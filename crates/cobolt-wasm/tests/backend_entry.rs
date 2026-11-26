// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! The Backend entry (ABI A59, spec 114 T12): a module that holds every program a Backend exports and is
//! called as `pr_backend_call(export)` with one JSON text per parameter.

mod common;

use cobolt_wasm::host::{run_backend, BackendReply};
use cobolt_wasm::{compile_backend, BackendProgram, Options};
use cobolt_wasm_abi::backend::{BackendSection, Refusal};
use std::time::Instant;

const ADD: &str = r#"
IDENTIFICATION DIVISION.
PROGRAM-ID. ADD-TWO.
DATA DIVISION.
LINKAGE SECTION.
01 REQ.
   05 A PIC 9(4).
   05 B PIC 9(4).
01 RES.
   05 TOTAL PIC 9(5).
PROCEDURE DIVISION USING REQ RETURNING RES.
MAIN-PARA.
    COMPUTE TOTAL = A + B.
    GOBACK.
"#;

const GREET: &str = r#"
IDENTIFICATION DIVISION.
PROGRAM-ID. GREET.
DATA DIVISION.
LINKAGE SECTION.
01 WHO PIC X(10).
01 MSG PIC X(20).
PROCEDURE DIVISION USING WHO MSG.
MAIN-PARA.
    STRING "HELLO " DELIMITED BY SIZE WHO DELIMITED BY SPACE INTO MSG.
    GOBACK.
"#;

fn backend_options() -> Options {
    Options { native: true, ..Options::default() }
}

/// Every program nested in each of the others, as `cobolt_server::Backend::build` nests them.
fn module(sources: &[&str]) -> cobolt_wasm::Compiled {
    let programs: Vec<cobolt_ast::program::Program> = sources.iter().map(|s| common::parse(s)).collect();
    let tops: Vec<cobolt_ast::program::Program> = programs
        .iter()
        .map(|p| {
            let mut top = p.clone();
            for other in &programs {
                if other.identification.program_id != p.identification.program_id {
                    top.nested_programs.push(other.clone());
                }
            }
            top
        })
        .collect();
    let list: Vec<BackendProgram<'_>> = tops.iter().map(|t| BackendProgram { name: t.identification.program_id.to_ascii_uppercase(), program: t }).collect();
    compile_backend(&list, &backend_options()).unwrap_or_else(|e| panic!("compile failed: {e}"))
}

#[test]
fn a_backend_module_has_the_entry_and_the_section_and_no_other_module_does() {
    let started = Instant::now();
    let c = module(&[ADD, GREET]);
    let scan = cobolt_wasm_abi::scan::scan_module(&c.wasm).unwrap();
    assert!(scan.has_export("pr_backend_call"), "the entry is exported");
    assert!(!scan.has_export("pr_run"), "a Backend module is not a program module");
    let text = std::str::from_utf8(scan.custom("prmanifest").unwrap()).unwrap();
    let section = BackendSection::from_manifest(text).unwrap().expect("the section is there");
    assert_eq!(section.exports.iter().map(|e| (e.name.as_str(), e.params, e.returning)).collect::<Vec<_>>(), vec![("ADD-TWO", 1, true), ("GREET", 2, false)]);
    // Any other module: neither the export nor the section.
    let plain =
        common::compile("IDENTIFICATION DIVISION.\nPROGRAM-ID. P.\nPROCEDURE DIVISION.\nMAIN-PARA.\n    DISPLAY \"X\".\n    STOP RUN.\n", &Options::default());
    let pscan = cobolt_wasm_abi::scan::scan_module(&plain.wasm).unwrap();
    assert!(!pscan.has_export("pr_backend_call"));
    assert_eq!(BackendSection::from_manifest(std::str::from_utf8(pscan.custom("prmanifest").unwrap()).unwrap()).unwrap(), None);
    // The Web bundler refuses a Backend module in words.
    let problems = cobolt_wasm_abi::scan::bundle_problems(&c.wasm, false);
    assert!(problems.iter().any(|p| p.contains("Backend module")), "{problems:?}");
    println!(
        "backend module: {} bytes, {} exports, entry and section present; a plain module has neither ({} ms)",
        c.wasm.len(),
        section.exports.len(),
        started.elapsed().as_millis()
    );
}

#[test]
fn a_valid_request_runs_and_an_over_limit_one_is_refused_before_the_program_runs() {
    let started = Instant::now();
    let c = module(&[ADD, GREET]);
    let reply = run_backend(&c.wasm, "ADD-TWO", &[r#"{"A":7,"B":35}"#]).unwrap();
    println!("ADD-TWO answered {reply:?}");
    let BackendReply::Answered(a) = reply else { panic!("not answered") };
    assert_eq!(a.args.len(), 1);
    assert!(a.args[0].contains("0007") || a.args[0].contains('7'), "{:?}", a.args);
    let ret = a.returning.expect("RETURNING");
    assert!(ret.contains("42"), "{ret}");

    let greet = run_backend(&c.wasm, "GREET", &[r#""WORLD""#, r#""""#]).unwrap();
    println!("GREET answered {greet:?}");
    let BackendReply::Answered(g) = greet else { panic!("not answered") };
    assert!(g.args[1].contains("HELLO WORLD"), "{:?}", g.args);
    assert_eq!(g.returning, None);

    // A string longer than its PICTURE is refused with the mapping's code and the member's path.
    let refused = run_backend(&c.wasm, "GREET", &[r#""THIS-IS-MUCH-TOO-LONG""#, r#""""#]).unwrap();
    println!("GREET refused {refused:?}");
    assert!(matches!(refused, BackendReply::Refused(Refusal { code: 6, .. })), "{refused:?}");
    // An unknown member of a group.
    let unknown = run_backend(&c.wasm, "ADD-TWO", &[r#"{"A":1,"Z":2}"#]).unwrap();
    println!("ADD-TWO refused {unknown:?}");
    assert!(matches!(unknown, BackendReply::Refused(Refusal { code: 3, .. })), "{unknown:?}");
    // More texts than parameters.
    let extra = run_backend(&c.wasm, "GREET", &[r#""A""#, r#""""#, r#""""#]).unwrap();
    assert_eq!(extra, BackendReply::BadRequest);
    println!("valid request answered, over-limit refused (6), unknown member refused (3), extra text bad request ({} ms)", started.elapsed().as_millis());
}
