// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! What a Backend module does with a verb (spec 114 T13, T35): the Backend column of the support table.
//!
//! The eight verbs the runner serves (`backend` capability, ABI A60) compile into a module that requires the capability and
//! imports exactly the verb's import; the verbs it does not (`AUTH-OP`, the nine a page calls, `CALL … WITH WEBSOCKET
//! LINKAGE`) are refused at their line with the Backend's own reason, and the Desktop module keeps the reason it had for
//! all of them.
mod common;

use cobolt_wasm::support::{BACKEND_SERVED_VERBS, BACKEND_SIDE_VERBS, CLIENT_SIDE_VERBS};
use cobolt_wasm::{compile_backend, compile_program, BackendProgram, Options};
use std::time::Instant;

fn source(call: &str) -> String {
    format!(
        "IDENTIFICATION DIVISION.\nPROGRAM-ID. P.\nDATA DIVISION.\nWORKING-STORAGE SECTION.\n01 X PIC X(10).\n01 N PIC 9(4).\n01 TICK.\n   05 TN PIC 9(6).\nPROCEDURE DIVISION.\nMAIN-PARA.\n    {call}\n    GOBACK.\n"
    )
}

/// The statement that calls the verb `name` with arguments the interpreter accepts.
fn call_of(name: &str) -> String {
    let args = match name {
        "WS-FAIL" => "4201 \"declined\"",
        "WS-PROGRESS" => "50 \"half\"",
        "WS-LAST-ERROR" => "X N X",
        "WS-FILE-PATH" | "WS-FILE-PUBLISH" => "X X N",
        "AUTH-OP" => "X X",
        "WS-CANCELLED" | "WS-PAUSE" => "N",
        "WS-EMIT" => "TICK",
        "WS-CLAIM" => "\"sub\" X",
        _ => "X",
    };
    format!("COBOL::\"{name}\" ( {args} ).")
}

fn compiled(call: &str) -> Result<cobolt_wasm::Compiled, String> {
    let p = common::parse(&source(call));
    let list = [BackendProgram { name: "P".into(), program: &p }];
    compile_backend(&list, &Options { native: true, ..Options::default() }).map_err(|e| e.to_string())
}

fn refusal(call: &str) -> Option<String> {
    compiled(call).err()
}

/// The refusal of the Desktop path for the same source: a program the Backend mode refuses must still read as it did
/// there (the Desktop column does not move).
fn desktop_refusal(call: &str) -> Option<String> {
    let p = common::parse(&source(call));
    compile_program(&p, &Options { native: true, ..Options::default() }).err().map(|e| e.to_string())
}

/// The import each served verb makes (ABI A60).
fn import_of(name: &str) -> &'static str {
    match name {
        "WS-FAIL" => "ws_fail",
        "WS-PROGRESS" => "ws_progress",
        "WS-CANCELLED" => "ws_cancelled",
        "WS-PAUSE" => "ws_pause",
        "WS-EMIT" => "ws_emit",
        "WS-CLAIM" => "ws_claim",
        "WS-FILE-PATH" => "ws_file_path",
        _ => "ws_file_publish",
    }
}

#[test]
fn the_verbs_the_runner_serves_compile_and_every_other_is_refused_at_its_line_with_the_backends_reason() {
    let started = Instant::now();
    let mut served = 0;
    for name in BACKEND_SERVED_VERBS {
        let c = compiled(&call_of(name)).unwrap_or_else(|e| panic!("{name}: a Backend module refused it: {e}"));
        let scan = cobolt_wasm_abi::scan::scan_module(&c.wasm).unwrap();
        let import = import_of(name);
        assert!(scan.imports.iter().any(|i| i.module == "prn" && i.name == import), "{name}: the module does not import prn.{import}");
        let manifest = scan.embedded_manifest().unwrap();
        assert!(manifest.capabilities.iter().any(|c| c == "backend"), "{name}: the manifest does not require `backend`: {:?}", manifest.capabilities);
        // Nothing else of the Backend capability is imported: a verb is one import.
        let others = cobolt_wasm_abi::codes::BACKEND_IMPORTS.iter().filter(|i| **i != import && scan.imports.iter().any(|s| s.module == "prn" && s.name == **i)).count();
        assert_eq!(others, 0, "{name}: it imports another verb's routine too");
        // The Desktop module keeps the reason it had.
        let d = desktop_refusal(&call_of(name)).unwrap_or_else(|| panic!("{name}: a Desktop module compiled it"));
        assert!(d.contains("DESKTOP-WASM-001 M6"), "{name}: the Desktop reason moved: {d}");
        served += 1;
    }
    assert_eq!(served, 8, "the eight verbs of the `backend` capability");
    let mut refused = 0;
    for name in BACKEND_SIDE_VERBS.iter().filter(|n| !BACKEND_SERVED_VERBS.contains(n)).chain(CLIENT_SIDE_VERBS) {
        let call = call_of(name);
        let b = refusal(&call).unwrap_or_else(|| panic!("{name}: a Backend module compiled it"));
        assert!(b.starts_with("line 11: "), "{name}: not refused at its line: {b}");
        assert!(b.contains(&format!("COBOL::\"{name}\"")), "{name}: the message does not name the verb: {b}");
        let expected = if *name == "AUTH-OP" { "identity program" } else { "a verb a page calls" };
        assert!(b.contains(expected), "{name}: {b}");
        assert!(!b.contains("DESKTOP-WASM-001"), "{name}: the Desktop's reason leaked into a Backend: {b}");
        let d = desktop_refusal(&call).unwrap_or_else(|| panic!("{name}: a Desktop module compiled it"));
        assert!(d.contains("DESKTOP-WASM-001 M6"), "{name}: the Desktop reason moved: {d}");
        refused += 1;
    }
    // `CALL … WITH WEBSOCKET LINKAGE`.
    let call = "CALL \"OTHER\" WITH WEBSOCKET LINKAGE USING X.";
    let b = refusal(call).expect("refused");
    assert!(b.contains("does not call another Backend over a WebSocket"), "{b}");
    let d = desktop_refusal(call).expect("refused");
    assert!(d.contains("DESKTOP-WASM-001 M6"), "{d}");
    refused += 1;
    // Anything else of a Backend is the Desktop's decision: a program with a file, SQL and a plain CALL compiles.
    let ok = refusal("MOVE 1 TO N.").is_none();
    assert!(ok, "a plain program is a Backend module");
    eprintln!(
        "=== BACKEND SUPPORT COLUMN (spec 114 T13, T35) ===\n  verbs a Backend module compiles: {served} of {} Backend-side (each requires `backend`, imports its own prn.ws_… and no other, and the Desktop module still refuses it)\n  refused at their line with the Backend's reason: 1 identity + {} client-side + 1 remote CALL = {refused}\n  a plain program compiles; {} ms\nPASS {:03} FAIL 000",
        BACKEND_SIDE_VERBS.len(),
        CLIENT_SIDE_VERBS.len(),
        started.elapsed().as_millis(),
        served + refused
    );
}

/// Fewer arguments than a verb takes: the interpreter does nothing for all but `WS-FAIL` (it reads `using.get(n)`), which is
/// a run-time error there, so that one is refused and the Build takes the Cargo path.
#[test]
fn a_verb_with_fewer_arguments_does_what_the_interpreter_does_with_them() {
    let mut checked = 0;
    for call in ["COBOL::\"WS-PROGRESS\" ( ).", "COBOL::\"WS-CANCELLED\" ( ).", "COBOL::\"WS-PAUSE\" ( ).", "COBOL::\"WS-EMIT\" ( ).", "COBOL::\"WS-CLAIM\" ( \"sub\" ).", "COBOL::\"WS-FILE-PATH\" ( X X ).", "COBOL::\"WS-FILE-PUBLISH\" ( X )."] {
        let why = refusal(call);
        assert!(why.is_none(), "{call} should compile to nothing, as it does in the interpreter: {why:?}");
        checked += 1;
    }
    let b = refusal("COBOL::\"WS-FAIL\" ( 4201 ).").expect("WS-FAIL with no message is refused");
    assert!(b.contains("takes a code and a message"), "{b}");
    checked += 1;
    eprintln!("verbs with too few arguments: {checked} of {checked} read as the interpreter reads them\nPASS {checked:03} FAIL 000");
}
