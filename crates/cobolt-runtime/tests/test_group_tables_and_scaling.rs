// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Storage semantics the NIST CCVS85 Nucleus module exercises on nearly every
//! program: an occurrence of a **table of groups**, high-order truncation on a
//! numeric MOVE, `P` decimal scaling positions, a live `REDEFINES` overlay, and
//! index-names mixed with signed literals as subscripts.

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

/// One occurrence of a table of groups is its subordinate items' **matching**
/// occurrences: writing `GRP-1 (2)` fills `ELEM1 (2,1) … ELEM1 (2,4)`, and
/// reading it back concatenates exactly those.
#[test]
fn group_occurrence_reads_and_writes_its_own_children() {
    let src = r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. GRPTAB.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 GRP-TAB.
          02 GRP-1 OCCURS 3 TIMES.
             03 ELEM1 PIC XXX OCCURS 2 TIMES.
       01 TEMP PIC XXX.
       PROCEDURE DIVISION.
       MAIN.
           MOVE "AAABBB" TO GRP-1 (1)
           MOVE "CCCDDD" TO GRP-1 (2)
           MOVE ELEM1 (1, 2) TO TEMP
           DISPLAY TEMP
           MOVE ELEM1 (2, 1) TO TEMP
           DISPLAY TEMP
           DISPLAY GRP-1 (2)
           DISPLAY GRP-TAB
           STOP RUN.
    "#;
    let out = run_capture(src);
    // The third occurrence was never written, so the record ends in its six
    // spaces — which `run_capture` trims off.
    assert_eq!(out, vec!["BBB", "CCC", "CCCDDD", "AAABBBCCCDDD"]);
}

/// Index-names, plain literals and **signed** literals mix freely as
/// subscripts. `ELEM1 (IN1 +2)` is two subscripts — the sign belongs to the
/// literal — while `ELEM1 (IN1 - 1)` with spaces around the operator is
/// relative indexing, one subscript.
#[test]
fn index_names_mix_with_signed_literals_and_relative_indexing() {
    let src = r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. IDXMIX.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 GRP-TAB.
          02 GRP-1 OCCURS 3 TIMES INDEXED BY IN1.
             03 ELEM1 PIC XXX OCCURS 3 TIMES INDEXED BY IN2.
       01 TEMP PIC XXX.
       PROCEDURE DIVISION.
       MAIN.
           MOVE "AAABBBCCC" TO GRP-1 (1)
           MOVE "DDDEEEFFF" TO GRP-1 (2)
           MOVE "GGGHHHIII" TO GRP-1 (3)
           SET IN1 TO 2
           SET IN2 TO 3
           MOVE ELEM1 (IN1, IN2) TO TEMP
           DISPLAY TEMP
           MOVE ELEM1 (IN1, +1) TO TEMP
           DISPLAY TEMP
           MOVE ELEM1 (IN1 +2) TO TEMP
           DISPLAY TEMP
           MOVE ELEM1 (IN1 - 1, 1) TO TEMP
           DISPLAY TEMP
           MOVE ELEM1 (3, IN2 - 1) TO TEMP
           DISPLAY TEMP
           STOP RUN.
    "#;
    let out = run_capture(src);
    assert_eq!(out, vec!["FFF", "DDD", "EEE", "AAA", "HHH"]);
}

/// A numeric receiver holds only its declared digits: the low-order end is cut
/// by the rescale and the **high-order** end by the receiver's capacity.
#[test]
fn numeric_move_truncates_high_order_digits() {
    let src = r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. TRUNC.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 SMALL-ONE PIC 99V999.
       01 WIDE-ONE  PIC 9999V9.
       PROCEDURE DIVISION.
       MAIN.
           MOVE 123.45 TO SMALL-ONE
           DISPLAY SMALL-ONE
           MOVE 123.45 TO WIDE-ONE
           DISPLAY WIDE-ONE
           STOP RUN.
    "#;
    let out = run_capture(src);
    assert_eq!(out, vec!["23450", "01234"]);
}

/// `P` positions are digit positions the item spans but does not store: they
/// only move the decimal point, and always read back as zero.
#[test]
fn p_scaling_positions_shift_the_decimal_point() {
    let src = r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. PSCALE.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 HUNDREDS PIC S999PP.
       01 SCALED   PIC 9(3)P(4).
       01 PLAIN    PIC 9(8).
       01 CMP      PIC 9(6)V9(4).
       PROCEDURE DIVISION.
       MAIN.
           MOVE 12345 TO HUNDREDS
           MOVE 12300 TO CMP
           IF HUNDREDS EQUAL TO CMP
              DISPLAY "SAME"
           ELSE
              DISPLAY "DIFFERENT"
           END-IF
           MOVE 8888888 TO SCALED
           MOVE SCALED TO PLAIN
           DISPLAY PLAIN
           STOP RUN.
    "#;
    let out = run_capture(src);
    assert_eq!(out, vec!["SAME", "08880000"]);
}

/// A `REDEFINES` item is a second reading of storage its target owns, not
/// storage of its own: it does not widen the group, and a write through either
/// description is visible through the other.
#[test]
fn redefines_shares_storage_with_its_target() {
    let src = r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. REDEF.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 REPORT-LINE.
          02 RESULT-X.
             03 RESULT-A PIC X(6) VALUE SPACE.
             03 RESULT-N REDEFINES RESULT-A PIC 9(6).
       PROCEDURE DIVISION.
       MAIN.
           MOVE 123456 TO RESULT-N
           DISPLAY RESULT-A
           DISPLAY REPORT-LINE
           MOVE "ABCDEF" TO RESULT-A
           DISPLAY RESULT-A
           STOP RUN.
    "#;
    let out = run_capture(src);
    // Six bytes, not twelve: the REDEFINES describes the same six.
    assert_eq!(out, vec!["123456", "123456", "ABCDEF"]);
}
