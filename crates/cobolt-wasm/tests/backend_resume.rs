// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! The Backend module of a Backend that keeps a connection's state (ABI A61, spec 114 T37).
//!
//! `BackendOptions::per_connection` makes the manifest say `per-connection` and gives the module a second entry,
//! `pr_backend_resume`, beside `pr_backend_call`; a per-call module has the first only. And because the module keeps one
//! instance per exported program, each with a memory of its own, two exported programs that declare the same `EXTERNAL`
//! item cannot share it: the generator refuses that Backend when it keeps state per connection, naming the item and both
//! programs, and accepts it per call (where every call starts from the initial image, so nothing is shared to lose).
//! What the resumed entry does when it runs is held by `cobolt-backend-runner`'s `session` test, against the interpreter.

mod common;

use cobolt_wasm::{compile_backend, BackendOptions, BackendProgram, Compiled, Options};
use std::time::Instant;

fn program(name: &str, external: Option<&str>) -> String {
    let item = external.map_or(String::new(), |n| format!("01 {n} PIC 9(4) IS EXTERNAL.\n"));
    format!(
        "IDENTIFICATION DIVISION.\nPROGRAM-ID. {name}.\nDATA DIVISION.\nWORKING-STORAGE SECTION.\n{item}01 OWN PIC 9(4) VALUE 0.\nLINKAGE SECTION.\n01 LK.\n    05 N PIC 9(4).\nPROCEDURE DIVISION USING LK.\nMAIN-PARA.\n    ADD 1 TO OWN\n    MOVE OWN TO N\n    GOBACK.\n"
    )
}

fn module(sources: &[String], per_connection: bool) -> Result<Compiled, cobolt_wasm::GenError> {
    let programs: Vec<cobolt_ast::program::Program> = sources.iter().map(|s| common::parse(s)).collect();
    let list: Vec<BackendProgram<'_>> = programs.iter().map(|p| BackendProgram { name: p.identification.program_id.to_ascii_uppercase(), program: p }).collect();
    let options = Options { native: true, backend: Some(BackendOptions { per_connection, ..BackendOptions::default() }), ..Options::default() };
    compile_backend(&list, &options)
}

/// The names of the functions a module exports.
fn exports(wasm: &[u8]) -> Vec<String> {
    let mut names = Vec::new();
    for payload in wasmparser::Parser::new(0).parse_all(wasm) {
        if let wasmparser::Payload::ExportSection(section) = payload.expect("a module") {
            for e in section {
                names.push(e.expect("an export").name.to_string());
            }
        }
    }
    names
}

#[test]
fn a_per_connection_module_has_the_resumed_entry_and_says_so_and_a_per_call_one_has_neither() {
    let started = Instant::now();
    let two = [program("ALPHA", None), program("BETA", None)];
    let mut passed = 0u32;
    for (per_connection, state) in [(false, "per-call"), (true, "per-connection")] {
        let m = module(&two, per_connection).unwrap_or_else(|e| panic!("{state}: {e}"));
        assert_eq!(m.manifest["backend"]["state"], state, "the manifest's state");
        let names = exports(&m.wasm);
        assert!(names.iter().any(|n| n == "pr_backend_call"), "{state}: the first entry is exported: {names:?}");
        assert_eq!(names.iter().any(|n| n == "pr_backend_resume"), per_connection, "{state}: the resumed entry is exported only per connection: {names:?}");
        passed += 3;
    }
    println!("per-call and per-connection modules of 2 programs: the manifest's state and the exports are as ABI A61 says; PASS {passed:03} FAIL 000 ({} ms)", started.elapsed().as_millis());
}

#[test]
fn an_external_item_two_exported_programs_share_is_refused_only_when_the_connection_keeps_state() {
    let started = Instant::now();
    let mut passed = 0u32;

    // Two programs declare SHARED-COUNT: refused per connection, naming the item and both programs; accepted per call.
    let sharing = [program("ALPHA", Some("SHARED-COUNT")), program("BETA", Some("SHARED-COUNT"))];
    let e = module(&sharing, true).err().expect("per-connection must refuse the shared EXTERNAL item");
    assert!(e.message.contains("SHARED-COUNT") && e.message.contains("ALPHA") && e.message.contains("BETA"), "the refusal names the item and both programs: {e}");
    assert_eq!(e.program, "BETA", "it is the second declaration that is refused: {e:?}");
    assert!(e.line > 0, "at the line of the declaration: {e:?}");
    passed += 3;
    module(&sharing, false).unwrap_or_else(|e| panic!("per-call keeps nothing between calls, so the Backend is fine: {e}"));
    passed += 1;

    // An EXTERNAL item only one program declares, and EXTERNAL items with different names, are not shared: accepted.
    module(&[program("ALPHA", Some("ONLY-ALPHA")), program("BETA", None)], true).unwrap_or_else(|e| panic!("one declaration is not a share: {e}"));
    module(&[program("ALPHA", Some("ITEM-A")), program("BETA", Some("ITEM-B"))], true).unwrap_or_else(|e| panic!("two names are two items: {e}"));
    passed += 2;

    // Every conflict is listed, not only the first.
    let many = [program("ALPHA", Some("ITEM-A")), program("BETA", Some("ITEM-A")), program("GAMMA", Some("ITEM-A"))];
    let e = module(&many, true).err().expect("refused");
    assert_eq!(e.all_kinded().len(), 2, "BETA and GAMMA each repeat ITEM-A: {e:?}");
    passed += 1;
    println!("EXTERNAL items shared by exported programs: refused per connection (named, at its line, all listed), accepted per call and when not shared; PASS {passed:03} FAIL 000 ({} ms)", started.elapsed().as_millis());
}
