// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! WASM-012 T5: the device verbs of a Mobile application are never silently
//! skipped on the desktop. `COBOL::SecureStore`, `SecureLoad` and
//! `SecureDelete` belong to the phone shell; a desktop program that calls one
//! gets a run-time error naming where the verb is available, or runs its
//! `ON EXCEPTION`. Every desktop host — `rcrun run-form`, an embedded child
//! form and the compiled binary (`run_form_app` in `cobolt-compiler`) — runs
//! this interpreter, so one test of the interpreter covers the three.

use std::sync::mpsc;

use cobolt_lexer::{tokenize, SourceFormat};
use cobolt_parser::{parse, Severity};
use cobolt_runtime::Interpreter;

fn run(src: &str) -> (Result<(), String>, Vec<String>) {
    let result = parse(tokenize(src, SourceFormat::Free));
    assert!(result.diagnostics.iter().all(|d| d.severity != Severity::Error), "parse errors: {:?}", result.diagnostics);
    let program = result.program.expect("no program");
    let (_event_tx, event_rx) = mpsc::channel();
    let (state_tx, _state_rx) = mpsc::channel();
    let (display_tx, display_rx) = mpsc::channel();
    let mut interp = Interpreter::new_with_channels(program, event_rx, state_tx, display_tx);
    let outcome = interp.run().map_err(|e| e.to_string());
    (outcome, display_rx.try_iter().map(|s| s.trim().to_owned()).collect())
}

fn program(body: &str) -> String {
    format!(
        "       IDENTIFICATION DIVISION.
       PROGRAM-ID. DEVICEVERBS.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 WS-KEY   PIC X(20) VALUE \"session\".
       01 WS-VALUE PIC X(40) VALUE \"secret-value\".
       PROCEDURE DIVISION.
       MAIN-PARA.
{body}
           DISPLAY \"AFTER\"
           STOP RUN.
"
    )
}

#[test]
fn each_device_verb_is_refused_with_where_it_is_available() {
    let t = std::time::Instant::now();
    let mut cases = 0;
    for (verb, call) in [
        ("SecureStore", "           COBOL::SecureStore( WS-KEY WS-VALUE )"),
        ("SecureLoad", "           COBOL::SecureLoad( WS-KEY )"),
        ("SecureDelete", "           INVOKE COBOL::SecureDelete( WS-KEY )"),
    ] {
        let (outcome, shown) = run(&program(call));
        let why = outcome.expect_err("a desktop program cannot use the phone's secure store");
        assert!(why.contains(&format!("COBOL::{verb}")) && why.contains("available in a Mobile application only"), "{verb}: {why}");
        assert!(why.contains("line 9"), "{verb}: the line is named: {why}");
        assert!(!shown.iter().any(|l| l == "AFTER"), "{verb}: the program stops at the call, never skips it");
        cases += 1;
    }
    // With ON EXCEPTION the program decides: the refusal runs it, NOT ON EXCEPTION does not.
    let (outcome, shown) = run(&program(
        "           CALL \"COBOL-SECURELOAD\" USING WS-KEY
               ON EXCEPTION DISPLAY \"NO SECURE STORE\"
               NOT ON EXCEPTION DISPLAY \"LOADED\"
           END-CALL",
    ));
    assert!(outcome.is_ok(), "{outcome:?}");
    assert_eq!(shown, ["NO SECURE STORE", "AFTER"]);
    cases += 1;
    println!(
        "WASM-012 T5 device verbs on the desktop: {cases} cases (SecureStore, SecureLoad, SecureDelete refused with the line; \
         ON EXCEPTION taken, NOT ON EXCEPTION skipped) in {:.1} ms — 4 passed, 0 failed",
        t.elapsed().as_secs_f64() * 1000.0
    );
}
