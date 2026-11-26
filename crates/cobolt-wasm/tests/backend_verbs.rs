// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! What a Backend module says to a verb it cannot hold (spec 114 T13): the Backend column of the support table.
//! Every verb of a Backend's two lists is refused at its line with the Backend's own reason, and the Desktop
//! module keeps the reason it had.
mod common;

use cobolt_wasm::support::{BACKEND_SIDE_VERBS, CLIENT_SIDE_VERBS};
use cobolt_wasm::{compile_backend, compile_program, BackendProgram, Options};
use std::time::Instant;

fn source(call: &str) -> String {
    format!(
        "IDENTIFICATION DIVISION.\nPROGRAM-ID. P.\nDATA DIVISION.\nWORKING-STORAGE SECTION.\n01 X PIC X(10).\n01 N PIC 9(4).\nPROCEDURE DIVISION.\nMAIN-PARA.\n    {call}\n    GOBACK.\n"
    )
}

/// The statement that calls the verb `name` with arguments the interpreter accepts.
fn call_of(name: &str) -> String {
    let args = match name {
        "WS-FAIL" => "4201 \"declined\"",
        "WS-PROGRESS" => "50 \"half\"",
        "WS-LAST-ERROR" => "X N X",
        "WS-FILE-PATH" | "WS-FILE-PUBLISH" | "AUTH-OP" => "X X",
        "WS-CANCELLED" | "WS-PAUSE" => "N",
        _ => "X",
    };
    format!("COBOL::\"{name}\" ( {args} ).")
}

fn refusal(call: &str) -> Option<String> {
    let p = common::parse(&source(call));
    let list = [BackendProgram { name: "P".into(), program: &p }];
    match compile_backend(&list, &Options { native: true, ..Options::default() }) {
        Ok(_) => None,
        Err(e) => Some(e.to_string()),
    }
}

/// The refusal of the Desktop path for the same source: a program the Backend mode refuses must still read as it did
/// there (the Desktop column does not move).
fn desktop_refusal(call: &str) -> Option<String> {
    let p = common::parse(&source(call));
    compile_program(&p, &Options { native: true, ..Options::default() }).err().map(|e| e.to_string())
}

#[test]
fn every_backend_verb_is_refused_at_its_line_with_the_backends_reason_and_the_desktop_keeps_its_own() {
    let started = Instant::now();
    let mut rows = 0;
    for (list, why, what) in [(BACKEND_SIDE_VERBS, "`backend` capability", "Backend-side"), (CLIENT_SIDE_VERBS, "a verb a page calls", "client-side")] {
        for name in list {
            let call = call_of(name);
            let b = refusal(&call).unwrap_or_else(|| panic!("{name}: a Backend module compiled it"));
            assert!(b.starts_with("line 9: "), "{name}: not refused at its line: {b}");
            assert!(b.contains(&format!("COBOL::\"{name}\"")), "{name}: the message does not name the verb: {b}");
            let expected = if *name == "AUTH-OP" { "identity program" } else { why };
            assert!(b.contains(expected), "{name} ({what}): {b}");
            assert!(!b.contains("DESKTOP-WASM-001"), "{name}: the Desktop's reason leaked into a Backend: {b}");
            // The Desktop module says what it said before this change.
            let d = desktop_refusal(&call).unwrap_or_else(|| panic!("{name}: a Desktop module compiled it"));
            assert!(d.contains("DESKTOP-WASM-001 M6"), "{name}: the Desktop reason moved: {d}");
            rows += 1;
        }
    }
    // `CALL … WITH WEBSOCKET LINKAGE`.
    let call = "CALL \"OTHER\" WITH WEBSOCKET LINKAGE USING X.";
    let b = refusal(call).expect("refused");
    assert!(b.contains("does not call another Backend over a WebSocket"), "{b}");
    let d = desktop_refusal(call).expect("refused");
    assert!(d.contains("DESKTOP-WASM-001 M6"), "{d}");
    rows += 1;
    // Anything else of a Backend is the Desktop's decision: a program with a file, SQL and a plain CALL compiles.
    let ok = refusal("MOVE 1 TO N.").is_none();
    assert!(ok, "a plain program is a Backend module");
    eprintln!(
        "=== BACKEND SUPPORT COLUMN (spec 114 T13) ===\n  verbs refused with the Backend's reason: {} Backend-side + {} client-side + 1 remote CALL = {rows}\n  the Desktop reason unchanged for all {rows}; a plain program compiles\n  {} ms\nPASS {rows:03} FAIL 000",
        BACKEND_SIDE_VERBS.len(),
        CLIENT_SIDE_VERBS.len(),
        started.elapsed().as_millis()
    );
}
