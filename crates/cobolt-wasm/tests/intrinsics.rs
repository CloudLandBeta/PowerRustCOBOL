// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Intrinsic functions on a generated module against the interpreter (the NIST
//! IF module is the gate; these are the small, readable cases).

mod common;
use common::*;

fn program(body: &str) -> String {
    format!(
        "IDENTIFICATION DIVISION.\nPROGRAM-ID. T.\nDATA DIVISION.\nWORKING-STORAGE SECTION.\n\
         01 N PIC S9(5)V9(5).\n01 I PIC S9(5).\n01 X PIC X(30).\n01 A PIC 9 VALUE 2.\n01 B PIC 9 VALUE 3.\n\
         01 TBL.\n   02 T PIC 99 OCCURS 4 TIMES.\n01 TBL2.\n   02 R OCCURS 2 TIMES.\n      03 C PIC 9 OCCURS 3 TIMES.\n\
         PROCEDURE DIVISION.\nMAIN.\n{body}\n    STOP RUN.\n"
    )
}

#[test]
fn mathematical_functions() {
    let src = program(
        r#"    COMPUTE N = FUNCTION SQRT(2).
    DISPLAY "sqrt " N.
    COMPUTE N = FUNCTION SIN(1).
    DISPLAY "sin " N.
    COMPUTE N = FUNCTION COS(1) + FUNCTION TAN(0.5).
    DISPLAY "cos+tan " N.
    COMPUTE N = FUNCTION ATAN(1) * 4.
    DISPLAY "pi " N.
    COMPUTE N = FUNCTION ACOS(0.5) + FUNCTION ASIN(0.5).
    DISPLAY "acos+asin " N.
    COMPUTE N = FUNCTION LOG(10) + FUNCTION LOG10(1000).
    DISPLAY "logs " N.
    COMPUTE I = FUNCTION FACTORIAL(7).
    DISPLAY "7! " I.
    COMPUTE N = FUNCTION ANNUITY(0.05, 12).
    DISPLAY "annuity " N.
    COMPUTE N = FUNCTION PRESENT-VALUE(0.1, 100, 200, 300).
    DISPLAY "pv " N."#,
    );
    let lines = same_output(&src);
    assert_eq!(lines[0], "sqrt 0000141421");
}

#[test]
fn integer_and_remainder_functions() {
    let src = program(
        r#"    COMPUTE I = FUNCTION INTEGER(-2.5).
    DISPLAY "int " I.
    COMPUTE I = FUNCTION INTEGER-PART(-2.5).
    DISPLAY "intpart " I.
    COMPUTE N = FUNCTION MOD(-11, 5).
    DISPLAY "mod " N.
    COMPUTE N = FUNCTION REM(-11, 5).
    DISPLAY "rem " N.
    COMPUTE N = FUNCTION MOD(A, B) + FUNCTION ABS(-3).
    DISPLAY "mixed " N."#,
    );
    let lines = same_output(&src);
    assert_eq!(lines[0], "int -00003");
}

#[test]
fn statistics_over_arguments_and_whole_tables() {
    let src = program(
        r#"    MOVE 4 TO T (1). MOVE 9 TO T (2). MOVE 1 TO T (3). MOVE 6 TO T (4).
    MOVE 1 TO C (1, 1). MOVE 2 TO C (1, 2). MOVE 3 TO C (1, 3).
    MOVE 4 TO C (2, 1). MOVE 5 TO C (2, 2). MOVE 9 TO C (2, 3).
    COMPUTE N = FUNCTION MAX(T (ALL)).
    DISPLAY "max " N.
    COMPUTE N = FUNCTION MIN(T (ALL)).
    DISPLAY "min " N.
    COMPUTE N = FUNCTION MEAN(T (ALL)).
    DISPLAY "mean " N.
    COMPUTE N = FUNCTION MEDIAN(T (ALL)).
    DISPLAY "median " N.
    COMPUTE N = FUNCTION RANGE(T (ALL)).
    DISPLAY "range " N.
    COMPUTE N = FUNCTION MIDRANGE(T (ALL)).
    DISPLAY "midrange " N.
    COMPUTE N = FUNCTION VARIANCE(T (ALL)).
    DISPLAY "variance " N.
    COMPUTE N = FUNCTION STANDARD-DEVIATION(T (ALL)).
    DISPLAY "sd " N.
    COMPUTE N = FUNCTION SUM(T (ALL), 10).
    DISPLAY "sum " N.
    COMPUTE I = FUNCTION ORD-MAX(T (ALL)).
    DISPLAY "ordmax " I.
    COMPUTE I = FUNCTION ORD-MIN(T (ALL)).
    DISPLAY "ordmin " I.
    COMPUTE N = FUNCTION MAX(C (ALL, ALL)).
    DISPLAY "max2d " N.
    COMPUTE N = FUNCTION SUM(C (2, ALL)).
    DISPLAY "row2 " N."#,
    );
    let lines = same_output(&src);
    assert_eq!(lines[0], "max 0000900000");
}

#[test]
fn character_functions() {
    let src = program(
        r#"    MOVE FUNCTION UPPER-CASE("giZZard") TO X.
    DISPLAY "[" X "]".
    MOVE FUNCTION LOWER-CASE("GiZZard") TO X.
    DISPLAY "[" X "]".
    MOVE FUNCTION REVERSE("Gizzard") TO X.
    DISPLAY "[" X "]".
    MOVE FUNCTION REVERSE(FUNCTION UPPER-CASE("abc")) TO X.
    DISPLAY "[" X "]".
    COMPUTE I = FUNCTION LENGTH(FUNCTION REVERSE("Gizzard")) + FUNCTION LENGTH(X).
    DISPLAY "len " I.
    COMPUTE I = FUNCTION ORD(FUNCTION CHAR(7)).
    DISPLAY "ord " I.
    MOVE FUNCTION MAX("R", "a", "I") TO X.
    DISPLAY "[" X "]".
    MOVE FUNCTION MIN("R", "a", "I") TO X.
    DISPLAY "[" X "]".
    IF FUNCTION UPPER-CASE("abc") = "ABC" DISPLAY "equal" ELSE DISPLAY "different"."#,
    );
    let lines = same_output(&src);
    assert_eq!(lines[0], "[GIZZARD                       ]");
}

#[test]
fn numval_forms() {
    let src = program(
        r#"    COMPUTE N = FUNCTION NUMVAL("  -  4929.0323").
    DISPLAY "a " N.
    COMPUTE N = FUNCTION NUMVAL("200.0002  - ").
    DISPLAY "b " N.
    COMPUTE N = FUNCTION NUMVAL("12.5CR").
    DISPLAY "c " N.
    COMPUTE N = FUNCTION NUMVAL-C("$1,234.50").
    DISPLAY "d " N.
    COMPUTE N = FUNCTION NUMVAL("7") + FUNCTION NUMVAL(" 8 ").
    DISPLAY "e " N."#,
    );
    let lines = same_output(&src);
    assert_eq!(lines[0], "a -0492903230");
}

#[test]
fn date_functions_agree() {
    let src = program(
        r#"    COMPUTE I = FUNCTION INTEGER-OF-DATE(20000229).
    DISPLAY "iod " I.
    COMPUTE I = FUNCTION DATE-OF-INTEGER(146000).
    DISPLAY "doi " I.
    COMPUTE I = FUNCTION INTEGER-OF-DAY(2024060).
    DISPLAY "iod2 " I.
    COMPUTE I = FUNCTION DAY-OF-INTEGER(150000).
    DISPLAY "doi2 " I."#,
    );
    same_output(&src);
}

#[test]
fn random_is_a_reproducible_sequence() {
    let src = program(
        r#"    COMPUTE N = FUNCTION RANDOM(7).
    DISPLAY "r1 " N.
    COMPUTE N = FUNCTION RANDOM.
    DISPLAY "r2 " N.
    COMPUTE N = FUNCTION RANDOM(7).
    DISPLAY "again " N.
    COMPUTE I = FUNCTION INTEGER(100 * FUNCTION RANDOM(3)).
    DISPLAY "scaled " I."#,
    );
    let lines = same_output(&src);
    assert_eq!(lines[0].replace("r1", "again"), lines[2]);
}

#[test]
fn the_clock_and_compile_time_functions_have_the_standard_shape() {
    let src = program(
        r#"    MOVE FUNCTION CURRENT-DATE TO X.
    DISPLAY X(1:4) "|" X(17:5).
    MOVE FUNCTION WHEN-COMPILED TO X.
    DISPLAY X(1:2)."#,
    );
    let o = run_wasm(&src, &cobolt_wasm::Options::default());
    let lines = wasm_lines(&o);
    assert_eq!(lines.len(), 2, "{lines:?}");
    // The year has four digits and the offset is spelled; the module's clock is UTC.
    assert!(lines[0].ends_with("|+0000"), "{:?}", lines[0]);
    assert!(lines[0][..4].chars().all(|c| c.is_ascii_digit()));
    assert_eq!(lines[1], "20");
    // A reproducible build names the time it was made.
    let fixed = cobolt_wasm::Options { when_compiled: Some("2026102012000000+0000".into()), ..Default::default() };
    let o = run_wasm("IDENTIFICATION DIVISION.\nPROGRAM-ID. W.\nDATA DIVISION.\nWORKING-STORAGE SECTION.\n01 X PIC X(21).\nPROCEDURE DIVISION.\n    MOVE FUNCTION WHEN-COMPILED TO X.\n    DISPLAY X.\n    STOP RUN.\n", &fixed);
    assert_eq!(wasm_lines(&o), vec!["2026102012000000+0000"]);
}
