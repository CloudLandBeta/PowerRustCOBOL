// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! A **subscripted** (or qualified) data item as an EVALUATE subject.
//!
//! `EVALUATE CODE-X(N) WHEN "D" …` always took WHEN OTHER. The parser reads an
//! EVALUATE subject as a condition first and rewinds only when the result is a
//! bare condition-name; a subscripted reference comes back as
//! `Condition::ConditionRef`, which it kept as a *conditional* subject. At run
//! time a `ConditionRef` that names no 88-level means "the item holds something
//! non-zero", so the subject was the truth value TRUE and no literal WHEN could
//! equal it.
//!
//! `tests/cobol/conditionals/test-evaluate-subscript.cbl` is the quantified
//! program (29 cases); this runs it. The one-liners below pin the reproduction
//! from the bug report so a regression names the form that broke.

use std::path::PathBuf;
use std::sync::mpsc;

use cobolt_lexer::{expand_copybooks, tokenize, SourceFormat};
use cobolt_parser::{parse, Severity};
use cobolt_runtime::Interpreter;

fn detect_format(src: &str) -> SourceFormat {
    let looks_fixed = src.lines().any(|line| {
        let b = line.as_bytes();
        b.len() > 6 && b[6] != b' ' && b[..6].iter().all(|&c| c == b' ' || c.is_ascii_digit())
    });
    if looks_fixed {
        SourceFormat::Fixed
    } else {
        SourceFormat::Free
    }
}

/// Parse `src` (failing loudly on any error) and return its DISPLAY lines.
fn run(src: &str, dir: &std::path::Path) -> Vec<String> {
    let expanded = expand_copybooks(src, dir, detect_format(src));
    assert!(expanded.errors.is_empty(), "copybook errors: {:?}", expanded.errors);
    let result = parse(tokenize(&expanded.text, SourceFormat::Free));
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
    interp.run().expect("run failed");
    drop(interp);
    display_rx.try_iter().map(|l| l.trim().to_string()).collect()
}

#[test]
fn the_subscripted_subject_program_passes() {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/cobol/conditionals");
    let src = std::fs::read_to_string(dir.join("test-evaluate-subscript.cbl")).expect("read .cbl");
    let out = run(&src, &dir).join("\n");
    println!("{out}");
    assert!(out.contains("CASES EXERCISED : 29"), "{out}");
    assert!(out.contains("TESTS RUN       : 0029"), "{out}");
    assert!(out.contains("TESTS PASSED    : 0029"), "{out}");
    assert!(out.contains("TESTS FAILED    : 0000"), "{out}");
    assert!(out.contains("OVERALL RESULT  : PASS"), "{out}");
}

/// The bug report's reproduction, verbatim: six EVALUATEs, all must match.
#[test]
fn the_reported_reproduction_matches_in_all_six_forms() {
    let src = r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. EV2.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 TBL.
           05 ROWX OCCURS 3.
               10 ST PIC X.
       01 ONE PIC X VALUE "D".
       01 SIMPLE-TBL.
           05 CODE-X PIC X OCCURS 3.
       01 N PIC 99 VALUE 2.
       PROCEDURE DIVISION.
           MOVE "D" TO ST(2)
           MOVE "D" TO CODE-X(2)
           EVALUATE ONE   WHEN "D" DISPLAY "plain item: matched"          WHEN OTHER DISPLAY "plain item: OTHER" END-EVALUATE
           EVALUATE CODE-X(2) WHEN "D" DISPLAY "1-level literal: matched"  WHEN OTHER DISPLAY "1-level literal: OTHER" END-EVALUATE
           EVALUATE CODE-X(N) WHEN "D" DISPLAY "1-level variable: matched" WHEN OTHER DISPLAY "1-level variable: OTHER" END-EVALUATE
           EVALUATE ST(2)     WHEN "D" DISPLAY "2-level literal: matched"  WHEN OTHER DISPLAY "2-level literal: OTHER" END-EVALUATE
           EVALUATE ST(N)     WHEN "D" DISPLAY "2-level variable: matched" WHEN OTHER DISPLAY "2-level variable: OTHER" END-EVALUATE
           EVALUATE TRUE WHEN ST(N) = "D" DISPLAY "EVALUATE TRUE: matched" WHEN OTHER DISPLAY "EVALUATE TRUE: OTHER" END-EVALUATE
           STOP RUN.
"#;
    let out = run(src, std::path::Path::new("."));
    assert_eq!(
        out,
        vec![
            "plain item: matched",
            "1-level literal: matched",
            "1-level variable: matched",
            "2-level literal: matched",
            "2-level variable: matched",
            "EVALUATE TRUE: matched",
        ]
    );
}
