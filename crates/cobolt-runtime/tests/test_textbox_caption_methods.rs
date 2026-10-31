// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Spec 110: the caption of a TextBox from COBOL. Properties are written as
//! data items, `ResetCaption` puts attributes back to inherited, `CaptionIsSet`
//! answers 1 or 0, and an inherited attribute reads as the TextBox's own value.

use std::sync::mpsc;
use std::time::Instant;

use cobolt_lexer::{tokenize, SourceFormat};
use cobolt_parser::{parse, Severity};
use cobolt_runtime::{Interpreter, StateUpdate};

fn run(src: &str) -> (Vec<String>, Vec<StateUpdate>) {
    let result = parse(tokenize(src, SourceFormat::Free));
    assert!(
        result.diagnostics.iter().all(|d| d.severity != Severity::Error),
        "parse errors: {:?}",
        result.diagnostics
    );
    let program = result.program.expect("no program");
    let (_event_tx, event_rx) = mpsc::channel();
    let (state_tx, state_rx) = mpsc::channel();
    let (display_tx, display_rx) = mpsc::channel();
    let mut interp = Interpreter::new_with_channels(program, event_rx, state_tx, display_tx);
    interp.run().expect("run failed");
    (display_rx.try_iter().collect(), state_rx.try_iter().collect())
}

const SRC: &str = r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. T.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 WS-A PIC X(20).
       01 WS-B PIC X(20).
       01 WS-C PIC X(20).
       01 WS-D PIC X(20).
       PROCEDURE DIVISION.
           TXT-NOME::FontSize = 9.
           MOVE TXT-NOME::CaptionIsSet("CaptionFontSize") TO WS-A.
           MOVE TXT-NOME::CaptionFontSize TO WS-B.
           TXT-NOME::CaptionFontSize = 12.
           MOVE TXT-NOME::CaptionIsSet("CaptionFontSize") TO WS-C.
           TXT-NOME::ResetCaption("CaptionFontSize").
           MOVE TXT-NOME::CaptionIsSet("CaptionFontSize") TO WS-D.
           DISPLAY "A=[" WS-A "] B=[" WS-B "] C=[" WS-C "] D=[" WS-D "]".
           STOP RUN.
"#;

#[test]
fn caption_methods_report() {
    let t0 = Instant::now();
    let (shown, updates) = run(SRC);
    let ms = t0.elapsed().as_secs_f64() * 1000.0;
    let line = shown.join("\n");
    assert!(line.contains("A=[0 "), "inherited at first: {line}");
    assert!(line.contains("B=[9 "), "an inherited attribute reads the TextBox's value: {line}");
    assert!(line.contains("C=[1 "), "set once written: {line}");
    assert!(line.contains("D=[0 "), "inherited again after ResetCaption: {line}");
    assert!(
        updates.iter().any(|u| u.prop.eq_ignore_ascii_case("CaptionFontSize")
            && u.value == cobolt_forms::caption::INHERIT),
        "the host is told to drop the attribute"
    );
    println!("TextBox caption methods from COBOL (spec 110)");
    println!("  cases: CaptionIsSet before/after write/after reset, inherited read, reset reaches the host");
    println!("  one program run: {ms:.2} ms; passed 4 / failed 0");
}
