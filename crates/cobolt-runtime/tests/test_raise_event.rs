// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! `COBOL::"RAISE-EVENT"( control event [argument] )` (WASM-005 T14): the
//! event is queued as the control itself raises one — the next `WAIT-EVENT`
//! presents it, after the running handler, in the order raised, under the
//! control's form spelling.

use std::sync::mpsc;

use cobolt_lexer::{tokenize, SourceFormat};
use cobolt_parser::{parse, Severity};
use cobolt_runtime::Interpreter;

fn run(src: &str) -> Vec<String> {
    let result = parse(tokenize(src, SourceFormat::Free));
    assert!(result.diagnostics.iter().all(|d| d.severity != Severity::Error), "{:?}", result.diagnostics);
    let (event_tx, event_rx) = mpsc::channel();
    // No UI: once the raised events are presented the loop sees the channel closed.
    drop(event_tx);
    let (state_tx, _state_rx) = mpsc::channel();
    let (display_tx, display_rx) = mpsc::channel();
    let mut interp = Interpreter::new_with_channels(result.program.unwrap(), event_rx, state_tx, display_tx);
    interp.seed_objects(vec![("Rest-1".to_string(), "RestClient".to_string(), Vec::<(String, String)>::new())]);
    interp.set_control_ids(["Rest-1"]);
    interp.run().unwrap_or_else(|e| panic!("{e}"));
    display_rx.try_iter().map(|s| s.trim_end().to_owned()).collect()
}

#[test]
fn raised_events_are_presented_after_the_handler_in_order_under_the_forms_spelling() {
    let shown = run("       IDENTIFICATION DIVISION.
       PROGRAM-ID. RAISER.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 COBOL-QUIT PIC 9 VALUE 0.
       01 EV PIC X(32).
       01 CT PIC X(32).
       01 WS-NAME PIC X(32) VALUE \"onError\".
       PROCEDURE DIVISION.
       MAIN-PARA.
           COBOL::\"RAISE-EVENT\" ( \"REST-1\" \"onComplete\" )
           COBOL::\"RAISE-EVENT\" ( \"rest-1\" WS-NAME \"why\" )
           DISPLAY \"HANDLER-END\"
           PERFORM UNTIL COBOL-QUIT = 1
               COBOL::\"WAIT-EVENT\" ( EV CT )
               IF COBOL-QUIT NOT = 1
                   DISPLAY \"EVENT \" CT::Trim() \" \" EV::Trim()
               END-IF
           END-PERFORM
           STOP RUN.
");
    assert_eq!(shown, vec!["HANDLER-END", "EVENT Rest-1 onComplete", "EVENT Rest-1 onError"], "{shown:?}");
}

#[test]
fn an_event_raised_in_a_nested_handler_is_presented_once() {
    let shown = run("       IDENTIFICATION DIVISION.
       PROGRAM-ID. RAISER.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 COBOL-QUIT PIC 9 VALUE 0.
       01 EV PIC X(32).
       01 CT PIC X(32).
       PROCEDURE DIVISION.
       MAIN-PARA.
           CALL \"BTN--ONCLICK\"
           PERFORM UNTIL COBOL-QUIT = 1
               COBOL::\"WAIT-EVENT\" ( EV CT )
               IF COBOL-QUIT NOT = 1
                   DISPLAY \"EVENT \" CT::Trim() \" \" EV::Trim()
               END-IF
           END-PERFORM
           STOP RUN.
       IDENTIFICATION DIVISION.
       PROGRAM-ID. BTN--ONCLICK IS COMMON PROGRAM.
       PROCEDURE DIVISION.
           COBOL::\"RAISE-EVENT\" ( \"REST-1\" \"onComplete\" )
           DISPLAY \"HANDLER-END\"
           GOBACK.
       END PROGRAM BTN--ONCLICK.
       END PROGRAM RAISER.
");
    assert_eq!(shown, vec!["HANDLER-END", "EVENT Rest-1 onComplete"], "{shown:?}");
}
