// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! What the NIST NC module taught the generator, kept as small programs.
//!
//! Each test names the CCVS85 member and assertion that exposed the behaviour,
//! runs a few lines of COBOL on the generated module **and** on the
//! interpreter, and requires the same output. The expected text is written
//! out where it is the point of the test.

mod common;
use common::*;

fn head(body: &str) -> String {
    format!("IDENTIFICATION DIVISION.\nPROGRAM-ID. T.\n{body}\n    STOP RUN.\n")
}

/// NC107A BZERO-TEST-1/2/3: `BLANK WHEN ZERO` blanks the item, reads back as
/// zero, compares equal to SPACE, and is not applied to the initial `VALUE`.
#[test]
fn blank_when_zero_numeric_and_edited() {
    let src = head(
        r#"DATA DIVISION.
WORKING-STORAGE SECTION.
01 N PIC 9(4) BLANK WHEN ZERO.
01 E PIC ZZ9 BLANK WHEN ZERO.
01 P PIC 999 VALUE "000" BLANK WHEN ZERO.
01 Q PIC 999 REDEFINES P.
01 S PIC X(4).
PROCEDURE DIVISION.
MAIN.
    MOVE 0 TO N.
    DISPLAY "[" N "]".
    IF N = SPACE DISPLAY "N is blank" ELSE DISPLAY "N is not blank".
    MOVE 7 TO N.
    ADD 1 TO N.
    DISPLAY "[" N "]".
    MOVE 0 TO E.
    DISPLAY "[" E "]".
    MOVE 5 TO E.
    DISPLAY "[" E "]".
    DISPLAY "[" Q "]"."#,
    );
    let lines = same_output(&src);
    assert_eq!(lines[0], "[    ]");
}

/// A `BLANK WHEN ZERO` item holding zero is still the number zero, so it equals
/// `ZERO` (COBOL-85 5.4: the clause changes how the item is shown, not what it
/// holds). The interpreter answers "not zero" here — a divergence recorded in
/// `specs/WASM-PARITY.md` — so this one is checked on the module alone.
#[test]
fn blank_when_zero_item_equals_zero() {
    let src = head(
        r#"DATA DIVISION.
WORKING-STORAGE SECTION.
01 N PIC 9(4) BLANK WHEN ZERO.
PROCEDURE DIVISION.
MAIN.
    MOVE 0 TO N.
    IF N = ZERO DISPLAY "zero" ELSE DISPLAY "not zero"."#,
    );
    let o = run_wasm(&src, &cobolt_wasm::Options::default());
    assert_eq!(wasm_lines(&o), vec!["zero"]);
}

/// NC107A JUST-TEST-01.02, NC104A MOVE-TEST-F1-29, NC108M FMT-TEST-GF-1: a
/// `VALUE` is not a move — it ignores JUSTIFIED, a group's value reaches its
/// children, and an edited item keeps a nonnumeric value as written.
#[test]
fn value_clause_initialisation() {
    let src = head(
        r#"DATA DIVISION.
WORKING-STORAGE SECTION.
01 J PIC X(3) JUSTIFIED RIGHT VALUE "XY".
01 G VALUE "$123.45".
   02 E PIC $999.99.
01 W PIC XBX VALUE "ABC".
01 T PIC X(7).
PROCEDURE DIVISION.
MAIN.
    DISPLAY "[" J "]".
    MOVE E TO T.
    DISPLAY "[" T "]".
    DISPLAY "[" W "]"."#,
    );
    let lines = same_output(&src);
    assert_eq!(lines, vec!["[XY ]", "[$123.45]", "[ABC]"]);
}

/// NC105A MOVE-TEST-F1-20 and NC252A: a group sender makes the move
/// alphanumeric — the edited receiver's picture gives its size and no editing.
#[test]
fn group_into_alphanumeric_edited() {
    let src = head(
        r#"DATA DIVISION.
WORKING-STORAGE SECTION.
01 G.
   02 A PIC X(3) VALUE "123".
   02 B PIC X(3) VALUE "ABC".
01 R PIC 0XXXXX0.
PROCEDURE DIVISION.
MAIN.
    MOVE G TO R.
    DISPLAY "[" R "]"."#,
    );
    let lines = same_output(&src);
    assert_eq!(lines, vec!["[123ABC ]"]);
}

/// NC222A MOV-TEST-F2-4-1: an edited item moved to a numeric one is de-edited
/// whatever insertions its picture carries.
#[test]
fn de_editing_with_insertions() {
    let src = head(
        r#"DATA DIVISION.
WORKING-STORAGE SECTION.
01 A PIC --9B.99B99/99.
01 B PIC S99V9(6).
PROCEDURE DIVISION.
MAIN.
    MOVE -42.9876 TO A.
    DISPLAY "[" A "]".
    MOVE A TO B.
    DISPLAY B.
    COMPUTE B = B + 1.
    DISPLAY B."#,
    );
    let lines = same_output(&src);
    assert_eq!(lines[0], "[-42 .98 76/00]");
}

/// NC114M MOVE-TEST-16: the positions of a `P` scale are zeros in the
/// character image of the item.
#[test]
fn p_scale_in_character_image() {
    let src = head(
        r#"DATA DIVISION.
WORKING-STORAGE SECTION.
01 P PIC 9P(3).
01 X PIC X(4).
PROCEDURE DIVISION.
MAIN.
    MOVE 3000 TO P.
    MOVE P TO X.
    DISPLAY "[" X "]".
    DISPLAY "[" P "]"."#,
    );
    let lines = same_output(&src);
    assert_eq!(lines[0], "[3000]");
}

/// NC126A LEV-TEST-GF-3-19 and NC103A IF-TEST-GF-4: figurative ZERO against an
/// edited item is the character '0'; a numeric literal written with leading
/// zeros keeps them against characters.
#[test]
fn zero_and_written_digits_against_characters() {
    let src = head(
        r#"DATA DIVISION.
WORKING-STORAGE SECTION.
01 E PIC 0009999.
01 D PIC X(10) VALUE "0123456789".
01 S PIC S9(5) VALUE -42.
01 C PIC X(5) VALUE "00042".
PROCEDURE DIVISION.
MAIN.
    MOVE ZERO TO E.
    IF E = ZERO DISPLAY "edited zero" ELSE DISPLAY "not zero".
    IF D = 0123456789 DISPLAY "written digits" ELSE DISPLAY "digits lost".
    IF S = C DISPLAY "sign ignored" ELSE DISPLAY "sign matters"."#,
    );
    let lines = same_output(&src);
    assert_eq!(lines, vec!["edited zero", "written digits", "sign ignored"]);
}

/// NC216A INS-TEST-F1-23: a signed numeric item is inspected as its digits.
#[test]
fn inspect_a_signed_numeric_item() {
    let src = head(
        r#"DATA DIVISION.
WORKING-STORAGE SECTION.
01 N PIC S9(5) VALUE -12345.
01 A PIC 999 VALUE 0.
01 B PIC 999 VALUE 0.
PROCEDURE DIVISION.
MAIN.
    INSPECT N TALLYING A FOR ALL "-" B FOR ALL "5".
    DISPLAY A " " B.
    INSPECT N REPLACING ALL "1" BY "7".
    DISPLAY N."#,
    );
    let lines = same_output(&src);
    assert_eq!(lines[0], "000 001");
}

/// NC247A: a group holding an OCCURS DEPENDING ON table is as long as its
/// count says; the receiver of a MOVE takes the declared maximum.
#[test]
fn group_with_a_variable_length_table() {
    let src = head(
        r#"DATA DIVISION.
WORKING-STORAGE SECTION.
01 R.
   02 CT PIC 9.
   02 T PIC X OCCURS 1 TO 9 TIMES DEPENDING ON CT.
01 R2.
   02 CT2 PIC 9.
   02 T2 PIC X OCCURS 1 TO 9 TIMES DEPENDING ON CT2.
01 DST PIC X(12).
PROCEDURE DIVISION.
MAIN.
    MOVE 9 TO CT.
    MOVE "A" TO T (1). MOVE "B" TO T (2). MOVE "C" TO T (3).
    MOVE "D" TO T (4). MOVE "E" TO T (5). MOVE "F" TO T (6).
    MOVE "G" TO T (7). MOVE "H" TO T (8). MOVE "I" TO T (9).
    MOVE 3 TO CT.
    MOVE R TO DST.
    DISPLAY "[" DST "]".
    MOVE 2 TO CT2.
    MOVE 9 TO CT.
    MOVE R TO R2.
    MOVE R2 TO DST.
    DISPLAY "[" DST "]"."#,
    );
    let lines = same_output(&src);
    assert_eq!(lines[0], "[3ABC        ]");
    assert_eq!(lines[1], "[9ABCDEFGHI  ]");
}

/// NC252A/NC209A: a 66-level item belongs to the record before it, so the
/// same name in two records is told apart by `OF`; `RENAMES` of one edited
/// item is an edited item.
#[test]
fn renames_belong_to_their_record() {
    let src = head(
        r#"DATA DIVISION.
WORKING-STORAGE SECTION.
01 A1.
   02 X1 PIC X(2) VALUE "AA".
   02 Y1 PIC X(2) VALUE "BB".
66 R1 RENAMES X1 THRU Y1.
01 A2.
   02 X2 PIC X(3) VALUE "CCC".
   02 Y2 PIC X(3) VALUE "DDD".
66 R1 RENAMES X2 THRU Y2.
01 W.
   02 E PIC ***,***.99.
66 RE RENAMES E.
PROCEDURE DIVISION.
MAIN.
    DISPLAY R1 OF A1 "/" R1 OF A2.
    MOVE 234.5 TO RE.
    DISPLAY E."#,
    );
    let lines = same_output(&src);
    assert_eq!(lines, vec!["AABB/CCCDDD", "****234.50"]);
}

/// NC201A PFM-TEST-F4-14: with TEST AFTER a variable is stepped only after
/// its own condition came out false.
#[test]
fn perform_test_after_with_after() {
    let src = head(
        r#"DATA DIVISION.
WORKING-STORAGE SECTION.
01 I PIC 9 VALUE 2.
01 J PIC S999 VALUE 20.
01 CNT PIC 99 VALUE 0.
PROCEDURE DIVISION.
MAIN.
    PERFORM BODY WITH TEST AFTER
        VARYING I FROM 2 BY -1 UNTIL I = 1
        AFTER J FROM 2 BY 6 UNTIL J > 19.
    DISPLAY CNT " " I " " J.
    GO TO FINISH.
BODY.
    ADD 1 TO CNT.
    MOVE 1 TO I.
    MOVE 99 TO J.
FINISH."#,
    );
    let lines = same_output(&src);
    assert_eq!(lines[0], "01 1 099");
}

/// NC201A PFM-TEST-F4-3: with TEST BEFORE the inner variable goes back to its
/// FROM as soon as the outer one is stepped, so it ends there.
#[test]
fn perform_test_before_with_after_ends_reset() {
    let src = head(
        r#"DATA DIVISION.
WORKING-STORAGE SECTION.
01 I PIC 9 VALUE 0.
01 J PIC S999 VALUE 0.
01 CNT PIC 99 VALUE 0.
PROCEDURE DIVISION.
MAIN.
    PERFORM BODY
        VARYING I FROM 3 BY -1 UNTIL I = 1
        AFTER J FROM 2 BY 6 UNTIL J > 20.
    DISPLAY CNT " " I " " J.
    GO TO FINISH.
BODY.
    ADD 1 TO CNT.
FINISH."#,
    );
    let lines = same_output(&src);
    assert_eq!(lines[0], "08 1 002");
}

/// NC225A EVA-TEST-GF-35-1: WHEN phrases written one after another share the
/// statements of the last.
#[test]
fn stacked_when_phrases() {
    let src = head(
        r#"DATA DIVISION.
WORKING-STORAGE SECTION.
01 N PIC 9(8) VALUE 81.
01 X PIC X.
PROCEDURE DIVISION.
MAIN.
    EVALUATE TRUE
        WHEN N NUMERIC
        WHEN N > 10
        WHEN N < 100
            MOVE "A" TO X
        WHEN OTHER
            MOVE "Z" TO X
    END-EVALUATE.
    DISPLAY X.
    MOVE 500 TO N.
    EVALUATE N
        WHEN 1
        WHEN 500
            DISPLAY "one of them"
        WHEN OTHER
            DISPLAY "none"
    END-EVALUATE."#,
    );
    let lines = same_output(&src);
    assert_eq!(lines, vec!["A", "one of them"]);
}

/// NC203A DIV-TEST-F4-8-1: the remainder is taken from the quotient as the
/// edited GIVING item holds it, decimals of its picture included.
#[test]
fn remainder_from_an_edited_quotient() {
    let src = head(
        r#"DATA DIVISION.
WORKING-STORAGE SECTION.
01 E PIC ZZ9.9.
01 R PIC S9(5).
PROCEDURE DIVISION.
MAIN.
    DIVIDE 16 INTO 174 GIVING E REMAINDER R.
    DISPLAY "[" E "] " R."#,
    );
    let lines = same_output(&src);
    assert_eq!(lines[0], "[ 10.8] 00001");
}

/// NC174A: an external switch the host sets reads through its `ON STATUS` /
/// `OFF STATUS` condition-names (a conformance build asks the host once, at
/// the start).
#[test]
fn external_switches_come_from_the_host() {
    let src = r#"IDENTIFICATION DIVISION.
PROGRAM-ID. SW.
ENVIRONMENT DIVISION.
CONFIGURATION SECTION.
SPECIAL-NAMES.
    SWITCH-1 IS SW1
        ON STATUS IS ON-SWITCH-1
        OFF STATUS IS OFF-SWITCH-1.
DATA DIVISION.
PROCEDURE DIVISION.
    IF ON-SWITCH-1 DISPLAY "on" ELSE DISPLAY "off".
    STOP RUN.
"#;
    let opts = cobolt_wasm::Options { conformance: true, ..Default::default() };
    let c = compile(src, &opts);
    for (state, want) in [(true, "on"), (false, "off")] {
        let conf = cobolt_wasm::host::Conformance {
            cwd: std::env::temp_dir(),
            switches: vec![("SWITCH-1".to_string(), state)],
        };
        let o = cobolt_wasm::host::run_conformance(&c.wasm, conf).unwrap();
        assert_eq!(wasm_lines(&o), vec![want]);
    }
}
