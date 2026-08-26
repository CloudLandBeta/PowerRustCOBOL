// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Continued literals — the **values**, not the parse.
//!
//! `NIST-spec-literal-continuation.md` AC6 asks for this explicitly, and
//! `NIST-spec-harness-and-baseline.md` R8 says why: a continued literal whose
//! stray quotation marks happen to balance can leave a program parsing cleanly
//! while holding the wrong data. A green parser test proves nothing here.
//!
//! The fixture is CCVS85's own — NC113M's `HYPHEN-LINE`, whose two fragments
//! must reassemble to exactly the 54 characters its `PICTURE X(54)` declares.
//! Getting the width wrong by one is invisible to the parser and obvious here.

use std::sync::mpsc;

use cobolt_lexer::{tokenize, SourceFormat};
use cobolt_parser::{parse, Severity};
use cobolt_runtime::Interpreter;

/// Run a **fixed-format** program and return its DISPLAY lines.
///
/// `SourceFormat::FixedStrict` is the point: continuation is a source-format
/// mechanism, so it only happens under the classic reference format.
fn run_fixed(src: &str) -> Vec<String> {
    let result = parse(tokenize(src, SourceFormat::FixedStrict));
    let errs: Vec<_> = result
        .diagnostics
        .iter()
        .filter(|d| d.severity == Severity::Error)
        .collect();
    assert!(errs.is_empty(), "parse errors: {errs:#?}");
    let program = result.program.expect("no program");

    let (_event_tx, event_rx) = mpsc::channel();
    let (state_tx, _state_rx) = mpsc::channel();
    let (display_tx, display_rx) = mpsc::channel();
    let mut interp = Interpreter::new_with_channels(program, event_rx, state_tx, display_tx);
    interp.run().expect("run failed");
    display_rx.try_iter().collect()
}

/// AC1's value half — NC113M's `HYPHEN-LINE`, verbatim from the suite.
///
/// The continued line is exactly 72 characters, so its literal fragment ends at
/// column 72 with no padding: 24 hyphens there and 30 on the continuation make
/// the 54 the PICTURE declares. A reassembly that is off by one still parses.
#[test]
fn nc113m_hyphen_line_is_exactly_54_characters() {
    let src = concat!(
        "000100 IDENTIFICATION DIVISION.\n",
        "000200 PROGRAM-ID. CONTVAL.\n",
        "000300 DATA DIVISION.\n",
        "000400 WORKING-STORAGE SECTION.\n",
        "000500 01  HYPHEN-LINE.\n",
        "000600     02 FILLER PICTURE IS X(54) VALUE IS \"------------------------\n",
        "000700-    \"------------------------------\".\n",
        "000800 PROCEDURE DIVISION.\n",
        "000900 MAIN.\n",
        "001000     DISPLAY HYPHEN-LINE.\n",
        "001100     STOP RUN.\n",
    );
    let out = run_fixed(src);
    assert_eq!(out.len(), 1, "{out:?}");
    assert_eq!(
        out[0],
        "-".repeat(54),
        "the two fragments did not reassemble to 54 hyphens (got {} chars)",
        out[0].chars().count()
    );
}

/// AC2's value half — a statement continued mid-way, from the probe that first
/// demonstrated the swallow.
///
/// The continued line ends with the word `MOVE` and no open literal, so this is
/// the *word* continuation rule: the halves meet with nothing between them, and
/// `MOVE"PRESENT INCORRECT"` tokenizes as the verb followed by the literal.
#[test]
fn a_statement_continued_mid_way_moves_the_right_text() {
    let src = concat!(
        "000100 IDENTIFICATION DIVISION.\n",
        "000200 PROGRAM-ID. CONTVAL2.\n",
        "000300 DATA DIVISION.\n",
        "000400 WORKING-STORAGE SECTION.\n",
        "000500 01  RE-MARK PIC X(17).\n",
        "000600 PROCEDURE DIVISION.\n",
        "000700 MAIN.\n",
        "000800     MOVE\n",
        "000900-             \"PRESENT INCORRECT\" TO RE-MARK.\n",
        "001000     DISPLAY RE-MARK.\n",
        "001100     STOP RUN.\n",
    );
    let out = run_fixed(src);
    assert_eq!(out, vec!["PRESENT INCORRECT".to_string()], "{out:?}");
}

/// A continued literal on a line **shorter** than column 72 keeps the spaces
/// between its text and column 72 — COBOL-85 makes them part of the literal.
///
/// This is the rule that makes a continued literal byte-exact only under the
/// classic reference format, and the one most likely to be quietly dropped.
#[test]
fn a_short_continued_line_keeps_its_padding() {
    // The line ends at column 41, so its literal content is `AB` plus the
    // spaces through column 72 — 31 of them — and `END` then continues it.
    // 2 + 31 + 3 = 36, which is what the PICTURE must hold; a field one
    // character short would truncate and hide the very thing being checked.
    let first = "000100     01  PADDED PIC X(36) VALUE \"AB";
    assert_eq!(first.chars().count(), 41, "fixture must end the line here");
    let src = format!(
        "000010 IDENTIFICATION DIVISION.\n\
         000020 PROGRAM-ID. CONTVAL3.\n\
         000030 DATA DIVISION.\n\
         000040 WORKING-STORAGE SECTION.\n\
         {first}\n\
         000200-    \"END\".\n\
         000300 PROCEDURE DIVISION.\n\
         000400 MAIN.\n\
         000500     DISPLAY PADDED.\n\
         000600     STOP RUN.\n"
    );
    let out = run_fixed(&src);
    assert_eq!(out.len(), 1, "{out:?}");
    // "AB" + spaces from column 43 through 72 + "END".
    let expected = format!("AB{}END", " ".repeat(72 - 41));
    assert_eq!(
        out[0].trim_end(),
        expected.trim_end(),
        "padding to column 72 was dropped: {:?}",
        out[0]
    );
}
