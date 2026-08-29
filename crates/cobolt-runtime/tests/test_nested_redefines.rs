// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Overlays that nest, from NIST CCVS85 NC252A.
//!
//! `REDEFINES` descriptions are kept in step by re-rendering one whole
//! description into another on every write inside either. Two things stopped
//! that at the first hop when overlays nested:
//!
//! * a key that lies inside more than one overlay kept only the **last** class
//!   built, so writing through the outer one could not reach the inner one;
//! * one global "a refresh is running" flag suppressed every further refresh,
//!   not just the write bouncing back into the description that started it.
//!
//! Together they meant `MOVE 11 TO RDFDATA16` — two bytes of a 01-level
//! redefinition — reached the redefined record and stopped, never reaching the
//! `RDF3 REDEFINES RDFDATA3` inside it, let alone the `RDF3-5-1 REDEFINES
//! RDF3-5` inside *that*, whose 88 the test asks about.

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
    interp.run().expect("run failed");
    drop(interp);
    display_rx.try_iter().map(|s| s.trim_end().to_owned()).collect()
}

/// NC252A's `REDEF10`, with the two overlays nested inside it, plus the 01-level
/// `REDEF12` that overlays the whole record. `RDFDATA16` and `RDF3-5` are the
/// same two bytes, reached through three different descriptions.
const RECORDS: &str = r#"
       01 REDEF10.
          02 RDFDATA1 PICTURE X(10) VALUE "ABC98765DE".
          02 RDFDATA2 PICTURE 9(4)V99 VALUE 9116.44.
          02 RDFDATA3.
             08 RDFDATA4 PICTURE X(6) VALUE "ALLDON".
             08 RDFDATA5 PICTURE XX99 VALUE "XX66".
          02 RDF3 REDEFINES RDFDATA3.
             03 RDF3-4 PICTURE X(8).
             03 RDF3-5 PIC 99.
             03 RDF3-5-1 REDEFINES RDF3-5.
                04 RDF3-5-14 PIC 9.
                04 RDF3-5-15 PIC 9.
                   88 HARD VALUE 0.
                   88 SOFT VALUE 1.
          02 RDFDATA6 PICTURE A(20) VALUE "ZYXWVUTSRQPONMLKJIHG".
       01 REDEF12 REDEFINES REDEF10.
          02 RDFDATA9 PICTURE A(3).
          02 RDFDATA10 PIC 9(5).
          02 RDFDATA11.
             03 RDFDATA12.
                04 RDFDATA13 PICTURE XX.
                04 RDFDATA14 OCCURS 6 TIMES PICTURE 9.
             03 RDFDATA15 PICTURE X(8).
          02 RDFDATA16 PICTURE 99.
"#;

#[test]
fn a_write_reaches_an_overlay_nested_two_deep() {
    // NC252A RDF-TEST-12. `RDFDATA16` is bytes 24-25 of `REDEF12`, which are
    // `RDF3-5` inside `RDF3` inside `REDEF10` — and `RDF3-5-15`, the 88's host,
    // inside `RDF3-5-1`. Writing 11 must make `SOFT` (VALUE 1) true.
    let out = run(&format!(
        r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. NESTRDF.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
{RECORDS}
       PROCEDURE DIVISION.
       MAIN.
           MOVE 11 TO RDFDATA16.
           DISPLAY "RDF3-5=" RDF3-5.
           DISPLAY "RDF3-5-14=" RDF3-5-14 " RDF3-5-15=" RDF3-5-15.
           DISPLAY "RDFDATA5=" RDFDATA5.
           IF SOFT DISPLAY "SOFT" ELSE DISPLAY "NOT SOFT".
           IF HARD DISPLAY "HARD" ELSE DISPLAY "NOT HARD".
           STOP RUN.
"#
    ));
    assert_eq!(
        out,
        vec![
            "RDF3-5=11",
            "RDF3-5-14=1 RDF3-5-15=1",
            "RDFDATA5=XX11",
            "SOFT",
            "NOT HARD",
        ]
    );
}

#[test]
fn the_chain_runs_the_other_way_too() {
    // The same bytes written through the *innermost* description have to reach
    // the outermost one. Nothing may loop: each overlay is refreshed once.
    let out = run(&format!(
        r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. NESTRDF2.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
{RECORDS}
       PROCEDURE DIVISION.
       MAIN.
           MOVE 7 TO RDF3-5-14.
           MOVE 3 TO RDF3-5-15.
           DISPLAY "RDFDATA16=" RDFDATA16.
           DISPLAY "RDFDATA5=" RDFDATA5.
           DISPLAY "RDF3-5=" RDF3-5.
           STOP RUN.
"#
    ));
    assert_eq!(
        out,
        vec!["RDFDATA16=73", "RDFDATA5=XX73", "RDF3-5=73"]
    );
}

#[test]
fn a_single_overlay_still_refreshes_once() {
    // The ordinary, un-nested case the mechanism was built for, pinned so the
    // per-description guard cannot quietly re-render a description twice or
    // stop rendering it at all. CCVS's own `COMPUTED-A` / `COMPUTED-N` pair is
    // this shape.
    let out = run(
        r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. ONERDF.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 COMPUTED-X.
          02 COMPUTED-A PIC X(20).
       01 COMPUTED-Y REDEFINES COMPUTED-X.
          02 COMPUTED-N PIC -9(9).9(9).
       PROCEDURE DIVISION.
       MAIN.
           MOVE 42 TO COMPUTED-N.
           DISPLAY "[" COMPUTED-A "]".
           MOVE "HELLO" TO COMPUTED-A.
           DISPLAY "[" COMPUTED-N "]".
           STOP RUN.
"#,
    );
    assert_eq!(out, vec!["[ 000000042.000000000]", "[HELLO               ]"]);
}
