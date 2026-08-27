// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Editing, MOVE alignment and `UNSTRING` — the rules the NIST CCVS85 Nucleus
//! module measures in NC114M, NC124A, NC126A, NC176A/NC177A, NC218A, NC221A,
//! NC222A, NC224A, NC238A, NC242A/NC243A and NC253A.
//!
//! Each test is one rule of COBOL-85, written the way the suite writes it.

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
    display_rx.try_iter().map(|s| s.trim_end().to_owned()).collect()
}

/// A size error phrase — **either** half of it — leaves an overflowing receiver
/// unchanged. Only `ON SIZE ERROR` used to count, so a statement carrying just
/// `NOT ON SIZE ERROR` truncated into every receiver that overflowed.
#[test]
fn not_on_size_error_alone_protects_the_receivers() {
    let src = r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. SIZEERR.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01  BIG   PIC 9(9) VALUE 222222222.
       01  R1    PIC 99 VALUE 0.
       01  R2    PIC 99 VALUE 0.
       01  FLAG  PIC X VALUE "N".
       PROCEDURE DIVISION.
       MAIN-PARA.
           ADD BIG 6 GIVING R1 R2
               NOT ON SIZE ERROR MOVE "A" TO FLAG.
           DISPLAY "R1=" R1 " R2=" R2 " FLAG=" FLAG.
           STOP RUN.
"#;
    assert_eq!(run_capture(src), vec!["R1=00 R2=00 FLAG=N"]);
}

/// A paragraph name may repeat. Each occurrence runs **its own** statements:
/// keying bodies by name let the second definition overwrite the first's while
/// the first kept its place in the program.
#[test]
fn a_duplicated_paragraph_name_runs_its_own_statements() {
    let src = r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. DUPPARA.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01  TRACE-1 PIC X(4) VALUE SPACES.
       PROCEDURE DIVISION.
       MAIN-PARA.
           DISPLAY "START".
       SAME-NAME.
           DISPLAY "FIRST".
       MIDDLE-PARA.
           DISPLAY "MIDDLE".
       SAME-NAME.
           DISPLAY "SECOND".
           STOP RUN.
"#;
    assert_eq!(
        run_capture(src),
        vec!["START", "FIRST", "MIDDLE", "SECOND"]
    );
}

/// `MOVE ZERO TO <group>` fills the whole record. Distributing the single
/// character `"0"` set the first field and left every one after it untouched.
#[test]
fn a_figurative_constant_fills_a_whole_group() {
    let src = r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. GRPFILL.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01  REC-1.
           03  A-1 PIC 99.
           03  B-1 PIC 99.
           03  C-1 PIC 99.
       PROCEDURE DIVISION.
       MAIN-PARA.
           MOVE 11 TO A-1.
           MOVE 22 TO B-1.
           MOVE 33 TO C-1.
           MOVE ZERO TO REC-1.
           DISPLAY "REC=" REC-1.
           MOVE SPACE TO REC-1.
           DISPLAY "SPC=[" REC-1 "]".
           STOP RUN.
"#;
    assert_eq!(run_capture(src), vec!["REC=000000", "SPC=[      ]"]);
}

/// `MOVE ALL literal TO <group>` repeats the literal across the whole record,
/// including every occurrence of a subordinate table. Filling only once left
/// the second occurrence of the outermost OCCURS blank.
#[test]
fn move_all_fills_every_occurrence_of_a_nested_table() {
    let src = r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. ALLFILL.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01  TBL.
           02  G1 OCCURS 2 TIMES.
               03  E1 PIC XX.
               03  G2 OCCURS 2 TIMES.
                   04  E2 PIC XX.
       PROCEDURE DIVISION.
       MAIN-PARA.
           MOVE ALL "ABCDEFGHIJKLMNOPQRSTUVWXYZ" TO TBL.
           DISPLAY "E1=" E1(1) "/" E1(2).
           DISPLAY "E2=" E2(1 1) "/" E2(1 2) "/" E2(2 1) "/" E2(2 2).
           STOP RUN.
"#;
    // Six bytes per G1 entry: E1 then two E2s.
    assert_eq!(
        run_capture(src),
        vec!["E1=AB/GH", "E2=CD/EF/IJ/KL"]
    );
}

/// Zero suppression that covers every digit position blanks the whole item,
/// decimal point included; `*` protection fills it with asterisks instead,
/// leaving only the point.
#[test]
fn a_fully_suppressed_picture_blanks_at_zero() {
    let src = r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. SUPPRESS.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01  Z-1 PIC ZZZ.ZZ.
       01  S-1 PIC *,***.**.
       01  F-1 PIC ++++.++.
       PROCEDURE DIVISION.
       MAIN-PARA.
           MOVE 0 TO Z-1.
           MOVE 0 TO S-1.
           MOVE 0 TO F-1.
           DISPLAY "Z=[" Z-1 "]".
           DISPLAY "S=[" S-1 "]".
           DISPLAY "F=[" F-1 "]".
           MOVE 12 TO F-1.
           DISPLAY "F12=[" F-1 "]".
           STOP RUN.
"#;
    assert_eq!(
        run_capture(src),
        vec![
            "Z=[      ]",
            "S=[*****.**]",
            "F=[       ]",
            // Seven positions: four `+` (one reserved for the sign) then `.++`.
            "F12=[ +12.00]",
        ]
    );
}

/// The floating symbol sits immediately left of the first digit shown, counted
/// in **character** positions — a grouping comma between them takes the sign.
/// And a simple insertion inside the suppression zone becomes the suppression
/// character.
#[test]
fn floating_sign_and_suppressed_insertions() {
    let src = r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. FLOATSGN.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01  M-1 PIC --,---.--.
       01  B-1 PIC -*B*99.
       01  P-1 PIC ZZZPP.
       01  L-1 PIC 090909.
       PROCEDURE DIVISION.
       MAIN-PARA.
           MOVE -123 TO M-1.
           MOVE -42  TO B-1.
           MOVE 900  TO P-1.
           MOVE 918  TO L-1.
           DISPLAY "M=[" M-1 "]".
           DISPLAY "B=[" B-1 "]".
           DISPLAY "P=[" P-1 "]".
           DISPLAY "L=[" L-1 "]".
           STOP RUN.
"#;
    assert_eq!(
        run_capture(src),
        vec!["M=[  -123.00]", "B=[-***42]", "P=[  9]", "L=[090108]"]
    );
}

/// An alphanumeric-edited item owns its insertion characters; the sender fills
/// only the `X`/`A`/`9` positions. `PIC ABA` is three characters wide, not two.
#[test]
fn alphanumeric_editing_keeps_its_insertions() {
    let src = r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. ALNUMED.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01  E-1 PIC ABA VALUE "ABC".
       01  E-2 PIC A/AA.
       01  E-3 PIC XXBXX/XX.
       PROCEDURE DIVISION.
       MAIN-PARA.
           DISPLAY "V=[" E-1 "]".
           MOVE "DEF" TO E-1.
           DISPLAY "M=[" E-1 "]".
           MOVE "ABC" TO E-2.
           DISPLAY "S=[" E-2 "]".
           MOVE SPACES TO E-3.
           DISPLAY "I=[" E-3 "]".
           STOP RUN.
"#;
    assert_eq!(
        run_capture(src),
        vec!["V=[ABC]", "M=[D E]", "S=[A/BC]", "I=[     /  ]"]
    );
}

/// `JUSTIFIED RIGHT` aligns the sender at the receiver's right end: a short
/// sender is padded on the left, a long one loses its leftmost characters.
#[test]
fn justified_right_aligns_at_the_right_end() {
    let src = r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. JUSTRT.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01  J-1 PIC X(5) JUSTIFIED RIGHT.
       01  J-2 PIC X JUSTIFIED RIGHT.
       PROCEDURE DIVISION.
       MAIN-PARA.
           MOVE "AB" TO J-1.
           DISPLAY "SHORT=[" J-1 "]".
           MOVE "12" TO J-2.
           DISPLAY "LONG=[" J-2 "]".
           STOP RUN.
"#;
    assert_eq!(run_capture(src), vec!["SHORT=[   AB]", "LONG=[2]"]);
}

/// De-editing: a numeric-**edited** sender moved to a numeric receiver
/// transfers the value its characters spell out, sign included.
#[test]
fn a_numeric_edited_sender_de_edits() {
    let src = r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. DEEDIT.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01  A-1 PIC $(4)9.99CR.
       01  B-1 PIC S9(4)V99.
       PROCEDURE DIVISION.
       MAIN-PARA.
           MOVE -123.45 TO A-1.
           MOVE A-1 TO B-1.
           DISPLAY "B=" B-1.
           STOP RUN.
"#;
    assert_eq!(run_capture(src), vec!["B=-012345"]);
}

/// A numeric sender reaching an alphanumeric receiver de-edits to its digits,
/// **left**-aligned — including a subscripted item and a literal.
#[test]
fn numeric_senders_left_align_in_an_alphanumeric_receiver() {
    let src = r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. LEFTALGN.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01  T-1.
           03  F-1 PIC 9 OCCURS 3 TIMES.
       01  H-1 PIC X(4).
       PROCEDURE DIVISION.
       MAIN-PARA.
           MOVE 6 TO F-1(2).
           MOVE F-1(2) TO H-1.
           DISPLAY "SUB=[" H-1 "]".
           MOVE 2 TO H-1.
           DISPLAY "LIT=[" H-1 "]".
           STOP RUN.
"#;
    assert_eq!(run_capture(src), vec!["SUB=[6   ]", "LIT=[2   ]"]);
}

/// Reference modification of a **table element** addresses that occurrence.
/// Dropping the subscript read the table's base slot instead.
#[test]
fn reference_modification_follows_the_subscript() {
    let src = r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. REFMODT.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01  T-5.
           03  ROW-1 OCCURS 3 TIMES.
               05  CELL-1 PIC 9(8) OCCURS 2 TIMES.
       PROCEDURE DIVISION.
       MAIN-PARA.
           MOVE 12345678 TO CELL-1 (3 2).
           DISPLAY "SLICE=[" CELL-1 (3 2) (2: 5) "]".
           STOP RUN.
"#;
    assert_eq!(run_capture(src), vec!["SLICE=[23456]"]);
}

/// `UNSTRING` honours `DELIMITER IN`, `COUNT IN`, `WITH POINTER` and
/// `TALLYING`, and `ALL` delivers **one** occurrence of the delimiter while
/// consuming the whole run.
#[test]
fn unstring_reports_delimiter_count_pointer_and_tally() {
    let src = r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. UNSTR.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01  SRC-1 PIC X(7) VALUE "1200000".
       01  RCV-1 PIC X.
       01  DLM-1 PIC X(4).
       01  CNT-1 PIC 99.
       01  PTR-1 PIC 99 VALUE 1.
       01  TLY-1 PIC 99 VALUE 0.
       PROCEDURE DIVISION.
       MAIN-PARA.
           UNSTRING SRC-1 DELIMITED BY ALL ZERO
               INTO RCV-1 DELIMITER IN DLM-1 COUNT IN CNT-1
               WITH POINTER PTR-1 TALLYING TLY-1.
           DISPLAY "R=[" RCV-1 "] D=[" DLM-1 "] C=" CNT-1
                   " P=" PTR-1 " T=" TLY-1.
           STOP RUN.
"#;
    // "12" then a run of five zeros: the field is two characters, the delimiter
    // delivered is one "0", and the pointer lands past the whole run.
    assert_eq!(
        run_capture(src),
        vec!["R=[1] D=[0   ] C=02 P=08 T=01"]
    );
}

/// With no `DELIMITED BY`, each receiver takes exactly its own size.
#[test]
fn unstring_without_a_delimiter_splits_by_receiver_size() {
    let src = r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. UNSTRSZ.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01  SRC-1 PIC X(6) VALUE "ABCDEF".
       01  A-1   PIC X(5).
       01  B-1   PIC X.
       01  FLAG  PIC X VALUE "N".
       PROCEDURE DIVISION.
       MAIN-PARA.
           UNSTRING SRC-1 INTO A-1 B-1
               ON OVERFLOW MOVE "Y" TO FLAG.
           DISPLAY "A=[" A-1 "] B=[" B-1 "] OVF=" FLAG.
           STOP RUN.
"#;
    assert_eq!(run_capture(src), vec!["A=[ABCDE] B=[F] OVF=N"]);
}

/// `INSPECT … FOR LEADING` counts contiguous occurrences of the whole pattern,
/// not characters that happen to appear in it.
#[test]
fn inspect_leading_counts_whole_pattern_runs() {
    let src = r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. INSLEAD.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01  S-1 PIC X(14) VALUE "AH YES AH YES ".
       01  T-1 PIC 999 VALUE 0.
       01  R-1 PIC X(8) VALUE "AHAHXYZ ".
       PROCEDURE DIVISION.
       MAIN-PARA.
           INSPECT S-1 TALLYING T-1 FOR LEADING "AH".
           DISPLAY "T=" T-1.
           INSPECT R-1 REPLACING LEADING "AH" BY "--".
           DISPLAY "R=[" R-1 "]".
           STOP RUN.
"#;
    assert_eq!(run_capture(src), vec!["T=001", "R=[----XYZ ]"]);
}

/// `DIVIDE … REMAINDER` forms the remainder after the quotient reaches its
/// receiver, so the remainder receiver's own subscript sees the new quotient.
#[test]
fn divide_remainder_sees_the_stored_quotient() {
    let src = r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. DIVREM.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01  ANS-1 PIC 99 VALUE 0.
       01  CNT-1 PIC 999 VALUE 6.
       01  REMS.
           03  WS-REM PIC 99 OCCURS 20 TIMES.
       PROCEDURE DIVISION.
       MAIN-PARA.
           MOVE ZERO TO REMS.
           DIVIDE 100 BY CNT-1 GIVING ANS-1 REMAINDER WS-REM (ANS-1).
           DISPLAY "ANS=" ANS-1 " REM=" WS-REM (16).
           STOP RUN.
"#;
    assert_eq!(run_capture(src), vec!["ANS=16 REM=04"]);
}

/// `SEARCH … VARYING id` replaces the search index only when `id` is an index
/// **of that table**. Otherwise the table's own index still drives the search
/// and `id` is stepped alongside it — so a `WHEN` that subscripts by the
/// table's own index still advances.
#[test]
fn search_varying_a_foreign_index_still_drives_the_table() {
    let src = r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. SRCHVAR.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01  GRP-A.
           02  TBL-A OCCURS 10 TIMES INDEXED BY IX-A.
               03  ELMT-A PIC 99.
       01  GRP-B.
           02  TBL-B OCCURS 10 TIMES INDEXED BY IX-W.
               03  ELMT-B PIC 99.
       01  I-1 PIC 99.
       PROCEDURE DIVISION.
       MAIN-PARA.
           PERFORM VARYING I-1 FROM 1 BY 1 UNTIL I-1 > 10
               MOVE I-1 TO ELMT-A (I-1)
           END-PERFORM.
           SET IX-A TO 1.
           SET IX-W TO IX-A.
           SEARCH TBL-A VARYING IX-W
               AT END DISPLAY "NOT FOUND"
               WHEN ELMT-A (IX-A) EQUAL TO 5
                   DISPLAY "FOUND AT " ELMT-A (IX-A).
           STOP RUN.
"#;
    assert_eq!(run_capture(src), vec!["FOUND AT 05"]);
}

/// `PERFORM paragraph OF section` runs the copy inside **that** section. The
/// name is program-wide in the procedure map, which hands back the first
/// definition anywhere, so the qualifier has to pick the position.
#[test]
fn perform_qualified_by_section_runs_that_copy() {
    let src = r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. QPERF.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01  QT-1 PIC X(4) VALUE SPACES.
       PROCEDURE DIVISION.
       SEC-ONE SECTION.
       DRIVER.
           PERFORM PAR-2A OF SEC-TWO.
           DISPLAY "QT=[" QT-1 "]".
           STOP RUN.
       PAR-2A.
           MOVE "FAIL" TO QT-1.
       SEC-TWO SECTION.
       PAR-2A.
           MOVE "PASS" TO QT-1.
"#;
    assert_eq!(run_capture(src), vec!["QT=[PASS]"]);
}

/// An unsigned numeric item has no sign position: it stores the magnitude.
#[test]
fn an_unsigned_item_stores_the_absolute_value() {
    let src = r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. UNSIGNED.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01  A-1 PIC S9(4) VALUE -1234.
       01  U-1 PIC 9(4) VALUE 0.
       PROCEDURE DIVISION.
       MAIN-PARA.
           MOVE A-1 TO U-1.
           DISPLAY "U=" U-1.
           STOP RUN.
"#;
    assert_eq!(run_capture(src), vec!["U=1234"]);
}

/// `ROUNDED` into an item with `P` scaling positions rounds to that position:
/// `PIC S99P` holds tens, so -99 rounds to -100 rather than truncating to -90.
#[test]
fn rounded_respects_p_scaling_positions() {
    let src = r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. ROUNDP.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01  A-1 PIC S99 VALUE 99.
       01  R-1 PIC S99P VALUE 0.
       PROCEDURE DIVISION.
       MAIN-PARA.
           SUBTRACT A-1 FROM R-1 ROUNDED.
           DISPLAY "R=" R-1.
           STOP RUN.
"#;
    assert_eq!(run_capture(src), vec!["R=-10"]);
}

/// `BLANK WHEN ZERO` is not confined to an *edited* picture — the standard
/// allows it on any numeric DISPLAY item. `PIC 9(10) BLANK WHEN ZERO` holding
/// zero reads as ten spaces, and compares equal to a blank literal, while the
/// stored value stays numeric so arithmetic on it is untouched
/// (NC107A BZERO-TEST-1/BZERO-TEST-2).
#[test]
fn blank_when_zero_applies_to_an_unedited_picture() {
    let src = r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. BZERO.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       77  DATA-F PICTURE IS 9(10) BLANK WHEN ZERO.
       77  DATA-M PICTURE IS W9999 BLANK WHEN ZERO.
       PROCEDURE DIVISION.
       MAIN-PARA.
           MOVE 0000000000 TO DATA-F.
           DISPLAY "F=[" DATA-F "]".
           IF DATA-F EQUAL TO "          "
               DISPLAY "F-BLANK" ELSE DISPLAY "F-NOT-BLANK" END-IF.
           MOVE 0000 TO DATA-M.
           IF DATA-M EQUAL TO SPACE
               DISPLAY "M-BLANK" ELSE DISPLAY "M-NOT-BLANK" END-IF.
           ADD 7 TO DATA-F.
           DISPLAY "F=[" DATA-F "]".
           STOP RUN.
"#;
    assert_eq!(
        run_capture(src),
        vec![
            "F=[          ]",
            "F-BLANK",
            "M-BLANK",
            "F=[0000000007]",
        ]
    );
}
