// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! `GOBACK` and `EXIT PROGRAM` leave the program from wherever they run.
//!
//! The interpreter absorbed both at the end of every PERFORM, so a `GOBACK`
//! in a performed paragraph ended only the PERFORM and the program carried
//! on with the statement after it. Reported from a login program that
//! PERFORMed its exit paragraph (`DISCONNECT` and `GOBACK`) after refusing a
//! password: it went on and let the user in. `GO TO` the same paragraph
//! worked, which is what made the PERFORM the culprit.
//!
//! `tests/cobol/call/test-goback-in-perform.cbl` is the quantified COBOL
//! counterpart; this file pins the report's own program and the main-program
//! case the COBOL test cannot show (it would end the test itself).

use std::path::Path;
use std::sync::mpsc;

use cobolt_lexer::{tokenize, SourceFormat};
use cobolt_parser::{parse, Severity};
use cobolt_runtime::Interpreter;
use pretty_assertions::assert_eq;

/// Parse `src`, run it, and return what it DISPLAYed, trimmed line by line.
fn run_capture(src: &str, format: SourceFormat) -> Vec<String> {
    let result = parse(tokenize(src, format));
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
    display_rx
        .try_iter()
        .map(|s| s.trim_end().to_owned())
        .collect()
}

/// The report's reproduction, verbatim (fixed format, two sibling programs).
const REPORTED: &str = "       IDENTIFICATION DIVISION.
       PROGRAM-ID. TESTGOBACK.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01  WS-R                        PIC X(020) VALUE SPACES.
       PROCEDURE DIVISION.
           CALL \"SUBGB\" USING WS-R
           DISPLAY \"Principal recebeu: \" WS-R
           STOP RUN.
       END PROGRAM TESTGOBACK.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. SUBGB.
       DATA DIVISION.
       LINKAGE SECTION.
       01  LK-R                        PIC X(020).
       PROCEDURE DIVISION USING LK-R.
       INICIO.
           MOVE \"ANTES DO FIM\" TO LK-R
           PERFORM FIM
           DISPLAY \"SUBGB CONTINUOU DEPOIS DO PERFORM FIM\"
           MOVE \"CONTINUOU\" TO LK-R
           GOBACK.
       FIM.
           GOBACK.
       END PROGRAM SUBGB.
";

#[test]
fn the_reported_program_returns_from_the_performed_paragraph() {
    assert_eq!(
        run_capture(REPORTED, SourceFormat::Fixed),
        vec!["Principal recebeu: ANTES DO FIM".to_string()]
    );
}

/// The report's control: `GO TO FIM` instead of `PERFORM FIM` already worked.
#[test]
fn go_to_the_exit_paragraph_still_returns() {
    let src = REPORTED.replace("PERFORM FIM", "GO TO FIM");
    assert_eq!(
        run_capture(&src, SourceFormat::Fixed),
        vec!["Principal recebeu: ANTES DO FIM".to_string()]
    );
}

/// In the main program a `GOBACK` ends the run, as `STOP RUN` would, even
/// from a performed paragraph and from inside an inline loop.
#[test]
fn goback_in_a_performed_paragraph_of_the_main_program_ends_the_run() {
    let src = "
       IDENTIFICATION DIVISION.
       PROGRAM-ID. MAINGB.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01  WS-I  PIC 9 VALUE 0.
       PROCEDURE DIVISION.
       START-PARA.
           DISPLAY \"before\"
           PERFORM LEAVE-PARA
           DISPLAY \"after the PERFORM\"
           STOP RUN.
       LEAVE-PARA.
           PERFORM UNTIL WS-I > 5
               ADD 1 TO WS-I
               DISPLAY \"pass \" WS-I
               IF WS-I = 2
                   GOBACK
               END-IF
           END-PERFORM.
";
    assert_eq!(
        run_capture(src, SourceFormat::Free),
        vec!["before".to_string(), "pass 1".to_string(), "pass 2".to_string()]
    );
}

/// `EXIT PROGRAM` in a performed paragraph of a CALLed program returns to
/// the caller too.
#[test]
fn exit_program_in_a_performed_paragraph_returns_to_the_caller() {
    let src = REPORTED.replace(
        "       FIM.\n           GOBACK.",
        "       FIM.\n           EXIT PROGRAM.",
    );
    assert!(src.contains("EXIT PROGRAM."));
    assert_eq!(
        run_capture(&src, SourceFormat::Fixed),
        vec!["Principal recebeu: ANTES DO FIM".to_string()]
    );
}

// ── The quantified COBOL program ───────────────────────────────────────────────

/// `tests/cobol/call/test-goback-in-perform.cbl` prints a case table and a
/// tally; this runs it under `cargo test` so the table cannot rot unseen.
#[test]
fn the_quantified_cobol_program_passes() {
    let path =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/cobol/call/test-goback-in-perform.cbl");
    let src = std::fs::read_to_string(&path).expect("read the COBOL test");
    let out = run_capture(&src, SourceFormat::Free);
    let report = out.join("\n");
    let line = |label: &str| {
        out.iter()
            .find(|l| l.starts_with(label))
            .unwrap_or_else(|| panic!("no `{label}` line in:\n{report}"))
            .clone()
    };
    assert_eq!(line("CHECKS RUN"), "CHECKS RUN      : 0011", "{report}");
    assert_eq!(line("CHECKS FAILED"), "CHECKS FAILED   : 0000", "{report}");
    assert_eq!(line("OVERALL RESULT"), "OVERALL RESULT  : PASS", "{report}");
    println!("{report}");
}
