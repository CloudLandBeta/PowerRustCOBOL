// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Spec 009 R4 / AC2: every woven procedure — event handlers included — is
//! `IS COMMON`, so one handler can `CALL` a **sibling** handler in the same form
//! module. (A sibling contained program is only callable when it is COMMON.)

use std::sync::mpsc;

use cobolt_lexer::{tokenize, SourceFormat};
use cobolt_parser::{parse, Severity};
use cobolt_runtime::Interpreter;

fn run_capture(src: &str) -> Vec<String> {
    let result = parse(tokenize(src, SourceFormat::Free));
    assert!(
        result
            .diagnostics
            .iter()
            .all(|d| d.severity != Severity::Error),
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

// OUTER CALLs HANDLER-A; HANDLER-A (a sibling contained program) CALLs its
// SIBLING HANDLER-B — valid only because HANDLER-B is `IS COMMON` (009 R4).
// HANDLER-B updates the form's GLOBAL item, observed back in OUTER.
const SRC: &str = r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. OUTER.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 WS-G PIC 9(4) GLOBAL.
       PROCEDURE DIVISION.
           MOVE 1 TO WS-G.
           CALL "HANDLER-A".
           DISPLAY WS-G.
           STOP RUN.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. HANDLER-A IS COMMON PROGRAM.
       PROCEDURE DIVISION.
           CALL "HANDLER-B".
           GOBACK.
       END PROGRAM HANDLER-A.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. HANDLER-B IS COMMON PROGRAM.
       PROCEDURE DIVISION.
           MOVE 42 TO WS-G.
           GOBACK.
       END PROGRAM HANDLER-B.

       END PROGRAM OUTER.
"#;

#[test]
fn handler_calls_sibling_common_handler() {
    let out = run_capture(SRC);
    assert_eq!(
        out,
        vec!["0042".to_string()],
        "sibling-handler CALL (enabled by IS COMMON, 009 R4) did not run"
    );
}
