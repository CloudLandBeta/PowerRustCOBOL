// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! User procedures (spec 005 T6): an event handler (a contained program) can
//! `CALL` a user procedure (a sibling contained program declared `IS COMMON`),
//! and a `GLOBAL` data item the procedure updates is seen back in the caller.
//! This also exercises the parser's acceptance of `IS COMMON PROGRAM`.

use std::sync::mpsc;

use cobolt_lexer::{tokenize, SourceFormat};
use cobolt_parser::{parse, Severity};
use cobolt_runtime::Interpreter;

fn run_capture(src: &str) -> Vec<String> {
    let result = parse(tokenize(src, SourceFormat::Free));
    assert!(
        result.diagnostics.iter().all(|d| d.severity != Severity::Error),
        "parse errors: {:?}",
        result.diagnostics
    );
    let program = result.program.expect("no program");
    let (_event_tx, event_rx) = mpsc::channel();
    let (state_tx, _state_rx) = mpsc::channel();
    let (display_tx, display_rx) = mpsc::channel();
    let mut interp = Interpreter::new_with_channels(program, event_rx, state_tx, display_tx);
    interp.run().expect("run failed");
    display_rx.try_iter().map(|s| s.trim().to_owned()).collect()
}

// OUTER owns a GLOBAL item and CALLs its HANDLER; HANDLER CALLs the sibling
// user procedure USERPROC (IS COMMON), which updates the GLOBAL item.
const SRC: &str = r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. OUTER.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 WS-G PIC 9(4) GLOBAL.
       PROCEDURE DIVISION.
           MOVE 11 TO WS-G.
           CALL "HANDLER".
           DISPLAY WS-G.
           STOP RUN.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. HANDLER.
       PROCEDURE DIVISION.
           CALL "USERPROC".
           GOBACK.
       END PROGRAM HANDLER.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. USERPROC IS COMMON PROGRAM.
       PROCEDURE DIVISION.
           MOVE 99 TO WS-G.
           GOBACK.
       END PROGRAM USERPROC.

       END PROGRAM OUTER.
"#;

#[test]
fn handler_calls_common_user_procedure_updating_global() {
    let out = run_capture(SRC);
    assert_eq!(out, vec!["0099".to_string()], "GLOBAL update via user proc not seen");
}
