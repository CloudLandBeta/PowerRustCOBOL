// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! COBOL-85 separators: the comma, the semicolon, and the space between
//! subscripts.
//!
//! The standard defines a **separator comma** and a **separator semicolon** as
//! a comma or semicolon *followed by a space*. They are pure decoration: they
//! may appear anywhere a space may appear, and they mean exactly what a space
//! means. `MOVE ZERO TO A, B, C.` and `MOVE ZERO TO A B C.` are the same
//! statement and no conforming compiler can tell them apart.
//!
//! Symmetrically, the only *required* separator between subscripts is a space:
//! `TABLE (1 1 1)` and `TABLE (1, 1, 1)` are the same reference. CCVS85 writes
//! both spellings throughout, which is how these gaps were found — see
//! `specs/nist/NIST-spec-separators.md`.

use cobolt_lexer::{tokenize, SourceFormat};
use cobolt_parser::{parse, Severity};

/// Parse a whole program and return its blocking diagnostics.
fn errors(src: &str) -> Vec<String> {
    let toks = tokenize(src, SourceFormat::Free);
    let parsed = parse(toks);
    parsed
        .diagnostics
        .iter()
        .filter(|d| d.severity == Severity::Error)
        .map(|d| format!("L{} {}", d.span.line, d.message))
        .collect()
}

fn program(data: &str, body: &str) -> String {
    format!(
        "IDENTIFICATION DIVISION.\n\
         PROGRAM-ID. TESTPROG.\n\
         DATA DIVISION.\n\
         WORKING-STORAGE SECTION.\n\
         {data}\
         PROCEDURE DIVISION.\n\
         MAIN.\n\
         {body}\
             STOP RUN.\n"
    )
}

/// AC1 — a receiving-operand list separated by commas.
#[test]
fn ac1_a_comma_separated_receiver_list_parses() {
    let src = program(
        "01  A PIC 9.\n01  B PIC 9.\n01  C PIC 9.\n",
        "    MOVE ZERO TO A, B, C.\n",
    );
    assert!(errors(&src).is_empty(), "{:?}", errors(&src));
}

/// R7 — and it must not produce a *semantic* cascade either. The fragment
/// after the comma used to be re-read as a paragraph header, which reported
/// `paragraph 'C' is declared more than once` — a diagnostic with no
/// relationship to anything the developer wrote.
#[test]
fn r7_a_comma_separated_receiver_list_produces_no_paragraph_cascade() {
    let src = program(
        "01  A PIC 9.\n01  B PIC 9.\n01  C PIC 9.\n",
        "    MOVE ZERO TO A, B, C.\n",
    );
    let toks = tokenize(&src, SourceFormat::Free);
    let parsed = parse(toks);
    for d in &parsed.diagnostics {
        assert!(
            !d.message.contains("paragraph"),
            "separator punctuation produced a paragraph diagnostic: {}",
            d.message
        );
    }
}

/// AC2 — `PROCEDURE DIVISION USING A, B, C.` binds three parameters.
#[test]
fn ac2_procedure_division_using_accepts_separator_commas() {
    let src = "IDENTIFICATION DIVISION.\n\
               PROGRAM-ID. TESTPROG.\n\
               DATA DIVISION.\n\
               LINKAGE SECTION.\n\
               01  A PIC 9.\n\
               01  B PIC 9.\n\
               01  C PIC 9.\n\
               PROCEDURE DIVISION USING A, B, C.\n\
               MAIN.\n\
                   STOP RUN.\n";
    assert!(errors(src).is_empty(), "{:?}", errors(src));
}

/// AC3 — `CALL "SUB" USING A, B, C.` passes three arguments.
#[test]
fn ac3_call_using_accepts_separator_commas() {
    let src = program(
        "01  A PIC 9.\n01  B PIC 9.\n01  C PIC 9.\n",
        "    CALL \"SUB\" USING A, B, C.\n",
    );
    assert!(errors(&src).is_empty(), "{:?}", errors(&src));
}

/// AC4 — the spaced and comma-separated subscript forms produce identical
/// ASTs. Comparing the debug rendering of the whole program is deliberate: it
/// catches an index silently dropped, which an "it parsed" assertion would not.
#[test]
fn ac4_spaced_and_comma_separated_subscripts_are_identical() {
    let data = "01  CELL-TABLE.\n\
                \x20   05  ROW OCCURS 3 TIMES.\n\
                \x20       10  CELL PIC 9 OCCURS 4 TIMES.\n";

    let spaced = program(data, "    MOVE 1 TO CELL (1  2).\n");
    let comma = program(data, "    MOVE 1 TO CELL (1, 2).\n");
    assert!(errors(&spaced).is_empty(), "spaced: {:?}", errors(&spaced));
    assert!(errors(&comma).is_empty(), "comma: {:?}", errors(&comma));

    let ast = |s: &str| format!("{:?}", parse(tokenize(s, SourceFormat::Free)).program);
    assert_eq!(
        ast(&spaced),
        ast(&comma),
        "the two spellings must produce the same AST"
    );
}

/// AC4, three subscripts — NC134A's `IF ANIMAL (1  1  1) EQUAL TO 1`.
#[test]
fn ac4_three_space_separated_subscripts_parse() {
    let data = "01  ANIMAL-TABLE.\n\
                \x20   05  A1 OCCURS 2 TIMES.\n\
                \x20       10  A2 OCCURS 2 TIMES.\n\
                \x20           15  ANIMAL PIC 9 OCCURS 2 TIMES.\n";
    let src = program(data, "    IF ANIMAL (1  1  1) EQUAL TO 1 CONTINUE END-IF.\n");
    assert!(errors(&src).is_empty(), "{:?}", errors(&src));
}

/// AC5 — qualification and a space-separated subscript list together.
/// NC135A / the spec's `CELL OF COLS OF ROWS (IDX-A  IDX-B)`.
#[test]
fn ac5_a_qualified_reference_takes_space_separated_subscripts() {
    let data = "01  ROWS.\n\
                \x20   05  COLS OCCURS 3 TIMES.\n\
                \x20       10  CELL PIC 9 OCCURS 4 TIMES.\n\
                01  IDX-A PIC 9 VALUE 1.\n\
                01  IDX-B PIC 9 VALUE 2.\n";
    let spaced = program(data, "    MOVE 1 TO CELL OF COLS OF ROWS (IDX-A  IDX-B).\n");
    let comma = program(data, "    MOVE 1 TO CELL OF COLS OF ROWS (IDX-A, IDX-B).\n");
    assert!(errors(&spaced).is_empty(), "spaced: {:?}", errors(&spaced));

    let ast = |s: &str| format!("{:?}", parse(tokenize(s, SourceFormat::Free)).program);
    assert_eq!(ast(&spaced), ast(&comma));
}

/// AC6 — NC101A puts a separator comma AND a separator semicolon between a
/// data-name and its REDEFINES clause, precisely to prove they are ignorable.
#[test]
fn ac6_separators_inside_a_data_description_parse() {
    let src = program(
        "01  WRK-XN-18-1 PIC X(18).\n\
         01  WRK-AN-X-18-1, REDEFINES WRK-XN-18-1 PIC A(18).\n\
         01  WRK-DU-X-18V0-1; REDEFINES WRK-XN-18-1 PIC 9(18).\n",
        "    CONTINUE.\n",
    );
    assert!(errors(&src).is_empty(), "{:?}", errors(&src));
}

/// R8 — nothing became *required*. The comma-free spellings still parse.
#[test]
fn r8_the_comma_is_never_required() {
    let src = program(
        "01  A PIC 9.\n01  B PIC 9.\n01  C PIC 9.\n",
        "    MOVE ZERO TO A B C.\n",
    );
    assert!(errors(&src).is_empty(), "{:?}", errors(&src));
}

/// AC8 — the decimal comma is untouched. It is glued between digits, so it is
/// never a separator, and `DECIMAL-POINT IS COMMA` still reads `1,5` as one and
/// a half.
#[test]
fn ac8_the_decimal_comma_still_works() {
    let src = "IDENTIFICATION DIVISION.\n\
               PROGRAM-ID. TESTPROG.\n\
               ENVIRONMENT DIVISION.\n\
               CONFIGURATION SECTION.\n\
               SPECIAL-NAMES. DECIMAL-POINT IS COMMA.\n\
               DATA DIVISION.\n\
               WORKING-STORAGE SECTION.\n\
               01  WS-PRICE PIC 9(5)V99 VALUE 1,5.\n\
               PROCEDURE DIVISION.\n\
               MAIN.\n\
                   STOP RUN.\n";
    assert!(errors(src).is_empty(), "{:?}", errors(src));
}

/// A separator comma before a **sign** is the one that carries meaning.
///
/// COBOL-85 tells a sign from a binary operator by the space that follows it:
/// `A -3` is two operands, `A - 3` is one subtraction. Drop the comma from
/// `FUNCTION MOD(A, -3)` and what is left reads as a subtraction — the function
/// is called with one argument, and IF124A and IF133A crashed the interpreter
/// on the second.
#[test]
fn a_comma_before_a_sign_still_separates_two_arguments() {
    let data = "01  A       PIC S9(9)V9(9) VALUE 11.\n01  WS-NUM  PIC S9(9)V9(9).\n";
    let src = program(data, "    COMPUTE WS-NUM = FUNCTION MOD(A, -3).\n");
    assert!(errors(&src).is_empty(), "{:?}", errors(&src));
    let ast = format!("{:?}", parse(tokenize(&src, SourceFormat::Free)).program);
    assert!(
        !ast.contains("Subtract") && !ast.contains("Sub,"),
        "the two arguments were merged into a subtraction: {ast}"
    );
}

/// …and it does not become required. Without the comma the two operands are
/// still two, because the sign is glued to its literal.
#[test]
fn a_sign_after_a_comma_is_not_a_binary_operator() {
    let data = "01  A       PIC S9(9)V9(9) VALUE 11.\n01  WS-NUM  PIC S9(9)V9(9).\n";
    let with = program(data, "    COMPUTE WS-NUM = FUNCTION MAX(A, -3, 7).\n");
    let without = program(data, "    COMPUTE WS-NUM = FUNCTION MAX(A -3 7).\n");
    assert!(errors(&with).is_empty(), "{:?}", errors(&with));
    assert!(errors(&without).is_empty(), "{:?}", errors(&without));
}

/// The **semicolon** keeps no such exception: it is never a list separator,
/// only decoration, so it is dropped even before a sign.
///
/// NC245A writes exactly this to prove it — a subscript list mixing both
/// punctuations with signed subscripts — and keeping the semicolon is a parse
/// error rather than a separator.
#[test]
fn a_semicolon_before_a_sign_is_still_only_decoration() {
    let data = "01  ELEM3-TABLE.\n   05 ELEM3 PIC 9 OCCURS 20 TIMES.\n01  TEMP PIC 9.\n";
    for body in [
        "    MOVE ELEM3( +3; +5, +10) TO TEMP.\n",
        "    MOVE ELEM3 (+3, 5; 10) TO TEMP.\n",
        "    ADD ELEM3 (3; 5; 10) TO TEMP.\n",
    ] {
        let src = program(data, body);
        assert!(errors(&src).is_empty(), "{body}: {:?}", errors(&src));
    }
}

/// With **no separator at all**, the space still settles it.
///
/// COBOL-85 tells a sign from a binary operator by the space that follows it,
/// so `10.2 -0.2` is two operands and `10.2 - 0.2` is one subtraction. IF132A
/// writes `FUNCTION RANGE(10.2 -0.2, 5.6, -15.6)` with no comma between the
/// first two arguments and expects four of them.
#[test]
fn a_sign_glued_to_a_literal_starts_a_new_operand() {
    let data = "01  WS-NUM  PIC S9(9)V9(4).\n";
    let src = program(
        data,
        "    COMPUTE WS-NUM = FUNCTION RANGE(10.2 -0.2, 5.6, -15.6).\n",
    );
    assert!(errors(&src).is_empty(), "{:?}", errors(&src));
    let ast = format!("{:?}", parse(tokenize(&src, SourceFormat::Free)).program);
    // Four arguments, so the mantissa of -0.2 survives as its own literal.
    assert!(
        ast.contains("-2") && !ast.contains("Sub"),
        "the first two arguments merged into a subtraction: {ast}"
    );
}

/// …and a properly spaced operator is still an operator.
#[test]
fn a_spaced_sign_is_still_a_binary_operator() {
    let data = "01  WS-NUM  PIC S9(9)V9(4).\n";
    let spaced = program(data, "    COMPUTE WS-NUM = 10.2 - 0.2.\n");
    assert!(errors(&spaced).is_empty(), "{:?}", errors(&spaced));
    let ast = format!("{:?}", parse(tokenize(&spaced, SourceFormat::Free)).program);
    assert!(ast.contains("Sub"), "the subtraction was lost: {ast}");
}

/// The rule reaches only as far as a **literal** before the sign. An
/// identifier cannot be told from a keyword here, and both `PICTURE -9(9).9(9)`
/// and `VARYING … BY -1` are "operand, gap, glued sign, digits".
#[test]
fn a_glued_sign_after_a_word_is_left_alone() {
    let data = "01  N-EDIT  PICTURE -9(9).9(9).\n01  WS-I PIC S9(4).\n01 WS-N PIC S9(9).\n";
    let src = program(
        data,
        "    PERFORM VARYING WS-I FROM 60 BY -1 UNTIL WS-I < 1\n\
         \x20       CONTINUE\n    END-PERFORM.\n",
    );
    assert!(errors(&src).is_empty(), "{:?}", errors(&src));
}

/// The RustCOBOL member-call argument list is not COBOL-85, but it is real
/// syntax and its comma is followed by a space — so it went through the same
/// change and must not have regressed to a single argument.
#[test]
fn a_member_call_still_takes_every_argument() {
    let data = "01  WS-IN  PIC X(20) VALUE \"a:b:c\".\n01  WS-OUT PIC X(20).\n";
    let src = program(data, "    MOVE WS-IN::replace(\"a\", \"z\") TO WS-OUT.\n");
    let ast = format!("{:?}", parse(tokenize(&src, SourceFormat::Free)).program);
    assert!(
        ast.contains("\"a\"") && ast.contains("\"z\""),
        "the second argument was dropped: {ast}"
    );
}
