// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! A control's method call on the line after a `CALL ... USING` with no
//! `END-CALL` runs as a statement of its own.
//!
//! The parser took `IMG-1::Create( 16 16 )` for one more BY REFERENCE argument
//! of the CALL, so the buffer was never created and stayed one pixel wide —
//! seen in PowerCobolCraft, whose texture procedure calls a subprogram and
//! then creates its tile buffer on the next line.

#![cfg(feature = "graphics")]

use std::sync::mpsc;

use cobolt_lexer::{tokenize, SourceFormat};
use cobolt_parser::{parse, Severity};
use cobolt_runtime::Interpreter;

fn run(src: &str) -> Vec<String> {
    let result = parse(tokenize(src, SourceFormat::Free));
    let errors: Vec<String> = result
        .diagnostics
        .iter()
        .filter(|d| d.severity == Severity::Error)
        .map(|d| format!("{}:{}: {}", d.span.line, d.span.col, d.message))
        .collect();
    assert!(errors.is_empty(), "parse errors: {errors:#?}");
    let program = result.program.expect("no program");
    let (_event_tx, event_rx) = mpsc::channel();
    let (state_tx, _state_rx) = mpsc::channel();
    let (display_tx, display_rx) = mpsc::channel();
    let mut interp = Interpreter::new_with_channels(program, event_rx, state_tx, display_tx);
    interp.seed_objects(
        [("IMG-1".to_string(), "ImageBuffer".to_string(), Vec::<(String, String)>::new())].into_iter(),
    );
    interp.run().expect("run failed");
    drop(interp);
    display_rx.try_iter().map(|l| l.trim().to_string()).collect()
}

/// The caller, with `CALL_LINE` spliced in before the method call, and the
/// subprogram that adds one to its argument (so the CALL is seen to run too).
fn program(call_line: &str) -> String {
    format!(
        r#"
IDENTIFICATION DIVISION.
PROGRAM-ID. MAIN-P.
DATA DIVISION.
WORKING-STORAGE SECTION.
01 WS-N PIC 99 VALUE 1.
PROCEDURE DIVISION.
    {call_line}
    IMG-1::Create( 16 16 )
    DISPLAY "W=" IMG-1::GetWidth() " N=" WS-N
    STOP RUN.
END PROGRAM MAIN-P.
IDENTIFICATION DIVISION.
PROGRAM-ID. SUB-P.
DATA DIVISION.
LINKAGE SECTION.
01 LK-N PIC 99.
PROCEDURE DIVISION USING LK-N.
    ADD 1 TO LK-N
    GOBACK.
END PROGRAM SUB-P.
"#
    )
}

#[test]
fn a_method_call_on_the_line_after_call_using_runs() {
    for call_line in [
        "CALL \"SUB-P\" USING WS-N",
        "CALL \"SUB-P\" USING BY REFERENCE WS-N",
        "CALL \"SUB-P\" USING WS-N\n    END-CALL",
    ] {
        let out = run(&program(call_line));
        assert_eq!(out.last().map(String::as_str), Some("W=16 N=02"), "{call_line}: {out:?}");
    }
}
