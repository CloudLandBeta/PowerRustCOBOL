// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Numeric literals that begin with a decimal point — `.5`, `-.5`, `.00001`.
//!
//! COBOL-85 allows a numeric literal to start with the decimal point; it must
//! only not *end* with one. The NIST CCVS85 suite leans on this heavily (48 of
//! its 459 programs), especially the intrinsic-function module, which tests
//! every function with fractional arguments written exactly this way:
//! `FUNCTION ACOS(.999)`.
//!
//! The whole difficulty is telling that literal apart from punctuation, because
//! a period is also the sentence terminator and a leading dot also starts a
//! numeric-edited PICTURE. The rule that separates them is **adjacency**:
//! COBOL-85 requires a space after a sentence-ending period, so a period glued
//! to its digits can only be a decimal point.
//!
//! The `guard_*` tests below are the ones that matter most. They pin the
//! behaviour that must NOT change, and they were written and run *before* the
//! feature existed — a false positive here would silently merge two statements
//! or corrupt a PICTURE, which is far worse than the gap being fixed.

use cobolt_ast::data::DataDecl;
use cobolt_ast::expr::{Expr, Literal};
use cobolt_ast::program::{DataSection, ProcedureBody};
use cobolt_ast::stmt::Stmt;
use cobolt_lexer::{tokenize, SourceFormat};
use cobolt_parser::{parse, Severity};

// ── Harness ───────────────────────────────────────────────────────────────────

/// A minimal program with `body` as the whole PROCEDURE DIVISION and `decls`
/// as WORKING-STORAGE.
fn prog(decls: &str, body: &str) -> String {
    format!(
        "IDENTIFICATION DIVISION.\n\
         PROGRAM-ID. TESTPROG.\n\
         DATA DIVISION.\n\
         WORKING-STORAGE SECTION.\n\
         {decls}\
         PROCEDURE DIVISION.\n\
         MAIN.\n\
         {body}"
    )
}

fn errors(src: &str) -> Vec<String> {
    parse(tokenize(src, SourceFormat::Free))
        .diagnostics
        .iter()
        .filter(|d| d.severity == Severity::Error)
        .map(|d| d.message.clone())
        .collect()
}

fn parse_ok(src: &str) -> cobolt_ast::program::Program {
    let errs = errors(src);
    assert!(errs.is_empty(), "unexpected parse errors: {errs:#?}\n{src}");
    parse(tokenize(src, SourceFormat::Free))
        .program
        .expect("no program produced")
}

/// The first paragraph's real statements, with the parser's `SentenceEnd`
/// markers removed.
///
/// `parse_stmts` inserts one marker per sentence boundary so `NEXT SENTENCE`
/// can be faithful, so the raw list is longer than the statement count. Use
/// [`sentence_ends`] when the boundaries are what is being asserted.
fn statements(src: &str) -> Vec<Stmt> {
    raw_statements(src)
        .into_iter()
        .filter(|s| !matches!(s, Stmt::SentenceEnd { .. }))
        .collect()
}

/// How many sentence boundaries the parser found — the direct measure of "the
/// period still terminates a sentence".
fn sentence_ends(src: &str) -> usize {
    raw_statements(src)
        .iter()
        .filter(|s| matches!(s, Stmt::SentenceEnd { .. }))
        .count()
}

/// Every statement of the first paragraph, markers included.
fn raw_statements(src: &str) -> Vec<Stmt> {
    let p = parse_ok(src);
    match &p.procedure.body {
        ProcedureBody::Paragraphs(paras) => {
            paras.first().map(|x| x.stmts.clone()).unwrap_or_default()
        }
        ProcedureBody::Sections(secs) => secs
            .first()
            .and_then(|s| s.paragraphs.first())
            .map(|x| x.stmts.clone())
            .unwrap_or_default(),
    }
}

/// Flatten WORKING-STORAGE, nested children included.
fn items(src: &str) -> Vec<DataDecl> {
    fn walk(out: &mut Vec<DataDecl>, item: &DataDecl) {
        out.push(item.clone());
        for c in &item.children {
            walk(out, c);
        }
    }
    let p = parse_ok(src);
    let mut out = Vec::new();
    if let Some(data) = p.data.as_ref() {
        for sec in &data.sections {
            if let DataSection::WorkingStorage(decls) = sec {
                for it in decls {
                    walk(&mut out, it);
                }
            }
        }
    }
    out
}

fn item(src: &str, name: &str) -> DataDecl {
    items(src)
        .into_iter()
        .find(|i| i.name.as_deref() == Some(name))
        .unwrap_or_else(|| panic!("no data item named {name}"))
}

/// The `VALUE` of a data item, as a literal.
fn value_of(src: &str, name: &str) -> Literal {
    item(src, name)
        .value
        .unwrap_or_else(|| panic!("{name} has no VALUE"))
}

// ── Guards: what must NOT change ──────────────────────────────────────────────
//
// Written before the leading-decimal-point support existed and green against
// that code. If any of these fails, the adjacency assumption is wrong.

/// A sentence-ending period is followed by whitespace, so it can never be read
/// as the start of a literal. Two sentences must stay two sentences.
#[test]
fn guard_sentence_period_still_terminates() {
    let src = prog(
        "01  X PIC 9(3) VALUE 1.\n01  Y PIC 9(3) VALUE 2.\n",
        "    MOVE X TO Y.\n    MOVE Y TO X.\n    STOP RUN.\n",
    );
    let stmts = statements(&src);
    assert_eq!(
        stmts.len(),
        3,
        "the period stopped terminating sentences: {stmts:#?}"
    );
    assert!(
        sentence_ends(&src) >= 2,
        "sentence boundaries were lost: {:#?}",
        raw_statements(&src)
    );
}

/// `VALUE 1.` — the period ends the data description entry, it does not begin a
/// fraction. The value stays the integer 1.
#[test]
fn guard_integer_value_period_still_terminates() {
    let src = prog("77  N PIC 9 VALUE 1.\n", "    STOP RUN.\n");
    assert_eq!(value_of(&src, "N"), Literal::Integer(1));
}

/// A numeric-edited PICTURE may itself begin with a decimal point. This is
/// CCVS85's `WRK-NE-1`, which appears in 9 of its programs.
///
/// PICTURE text is assembled by `parse_pic_clause`, which never calls the
/// literal parser — so this is structurally safe. Pinned anyway, because a
/// lexer-level implementation of the feature *would* have broken it, and this
/// test is what says so.
#[test]
fn guard_leading_dot_picture_is_untouched() {
    let src = prog("01  WRK-NE-1 PIC .9999/99999,99999,99.\n", "    STOP RUN.\n");
    let it = item(&src, "WRK-NE-1");
    let pic = it.picture.expect("WRK-NE-1 has no PICTURE");
    assert_eq!(
        pic.template, ".9999/99999,99999,99",
        "the leading-dot PICTURE was corrupted"
    );
}

/// A space after the period keeps it a terminator, even when digits follow.
#[test]
fn guard_period_then_space_then_digits() {
    let src = prog(
        "01  X PIC 9(3) VALUE 1.\n01  Y PIC 9(3) VALUE 2.\n",
        "    MOVE X TO Y.\n    ADD 5 TO X.\n    STOP RUN.\n",
    );
    let stmts = statements(&src);
    assert_eq!(stmts.len(), 3, "statement split changed: {stmts:#?}");
}

// ── The feature ───────────────────────────────────────────────────────────────

/// Pull the literal out of `MOVE <literal> TO …`.
fn moved_literal(src: &str) -> Literal {
    match statements(src).first().cloned() {
        Some(Stmt::Move { from, .. }) => match from {
            Expr::Literal(l, _) => l,
            other => panic!("MOVE source is not a literal: {other:?}"),
        },
        other => panic!("first statement is not a MOVE: {other:?}"),
    }
}

/// AC1 — `.00001` is one ten-thousandth of a tenth, not the integer 1.
///
/// The scale cannot come from the token's value: `00001` has already been
/// parsed to `1`, losing its four leading zeros. It comes from the span width.
#[test]
fn leading_point_preserves_leading_zeros() {
    let src = prog(
        "77  WS-NUM PIC S9(5)V9(5).\n",
        "    MOVE .00001 TO WS-NUM.\n    STOP RUN.\n",
    );
    assert_eq!(moved_literal(&src), Literal::Decimal(1, 5));
}

/// AC4 — `.1` arrives from the lexer as `LevelNumber(1)`, because a period sets
/// the lexer's "at line start" flag and 1 is a valid level number. Accepting
/// only `IntegerLiteral` would fix `.999` and leave this broken.
#[test]
fn leading_point_accepts_the_level_number_path() {
    let src = prog(
        "77  WS-NUM PIC S9V9.\n",
        "    MOVE .1 TO WS-NUM.\n    STOP RUN.\n",
    );
    assert_eq!(moved_literal(&src), Literal::Decimal(1, 1));

    // `.09` → LevelNumber(9), scale 2.
    let src = prog(
        "77  WS-NUM PIC S9V99.\n",
        "    MOVE .09 TO WS-NUM.\n    STOP RUN.\n",
    );
    assert_eq!(moved_literal(&src), Literal::Decimal(9, 2));
}

/// `.999` is above the level-number range, so it arrives as `IntegerLiteral`.
#[test]
fn leading_point_accepts_the_integer_path() {
    let src = prog(
        "77  WS-NUM PIC S9V999.\n",
        "    MOVE .999 TO WS-NUM.\n    STOP RUN.\n",
    );
    assert_eq!(moved_literal(&src), Literal::Decimal(999, 3));
}

/// AC3 — a `VALUE` clause. CCVS85: `77 A05ONES-DS-00V05 PICTURE SV9(5) VALUE .11111.`
#[test]
fn leading_point_in_value_clause() {
    let src = prog(
        "77  A05ONES PICTURE SV9(5) VALUE .11111.\n",
        "    STOP RUN.\n",
    );
    assert_eq!(value_of(&src, "A05ONES"), Literal::Decimal(11111, 5));
}

/// AC8 (parse half) — nine fractional digits survive exactly, no rounding.
/// CCVS85: `77 A01ONE-DS-P0801 PICTURE SP(8)9 VALUE .000000001.`
#[test]
fn leading_point_keeps_full_scale() {
    let src = prog(
        "77  A01ONE PICTURE SP(8)9 VALUE .000000001.\n",
        "    STOP RUN.\n",
    );
    assert_eq!(value_of(&src, "A01ONE"), Literal::Decimal(1, 9));
}

/// AC2 — the case that blocks the whole NIST intrinsic-function module:
/// `COMPUTE WS-NUM = FUNCTION ACOS(.999).`
#[test]
fn leading_point_as_a_function_argument() {
    let src = prog(
        "77  WS-NUM PIC S9(5)V9(5).\n",
        "    COMPUTE WS-NUM = FUNCTION ACOS(.999).\n    STOP RUN.\n",
    );
    // Parsing clean is the assertion here: the argument list used to end at the
    // period, leaving `999)` stranded.
    let stmts = statements(&src);
    assert_eq!(stmts.len(), 2, "{stmts:#?}");
}

/// A leading-point literal on the right of a comparison.
/// CCVS85: `IF WRK-DU-5V1-1 = .1 PERFORM PASS …`
#[test]
fn leading_point_in_a_condition() {
    let src = prog(
        "77  WS-NUM PIC S9V9 VALUE .5.\n",
        "    IF WS-NUM = .1 CONTINUE END-IF.\n    STOP RUN.\n",
    );
    assert!(errors(&src).is_empty());
}

// ── Signed leading-point literals (R2) ────────────────────────────────────────
//
// The lexer emits the sign as its own token by design, so that
// `COMPUTE X = Y - .5` stays a subtraction. Folding it back onto the literal is
// the parser's job, and already existed for `-1.5`.

#[test]
fn signed_leading_point_in_value_clause() {
    let src = prog("77  A PICTURE S9V9 VALUE -.5.\n", "    STOP RUN.\n");
    assert_eq!(value_of(&src, "A"), Literal::Decimal(-5, 1));

    let src = prog("77  B PICTURE S9V9 VALUE +.5.\n", "    STOP RUN.\n");
    assert_eq!(value_of(&src, "B"), Literal::Decimal(5, 1));
}

#[test]
fn signed_leading_point_in_expression() {
    let src = prog(
        "77  WS-NUM PIC S9V9.\n",
        "    COMPUTE WS-NUM = -.5.\n    STOP RUN.\n",
    );
    assert!(errors(&src).is_empty(), "{:#?}", errors(&src));
}

/// Subtraction must still be subtraction: the space around the operator is what
/// tells `Y - .5` from the literal `-.5`.
#[test]
fn subtraction_of_a_leading_point_literal() {
    let src = prog(
        "77  WS-NUM PIC S9(3)V9.\n77  Y PIC S9(3)V9 VALUE 9.\n",
        "    COMPUTE WS-NUM = Y - .5.\n    STOP RUN.\n",
    );
    assert!(errors(&src).is_empty(), "{:#?}", errors(&src));
}

// ── Other literal sites (R1) ──────────────────────────────────────────────────

/// 88-level condition names take literals through the same path.
#[test]
fn leading_point_in_88_level_values() {
    let src = prog(
        "01  WS-RATE PIC S9V999 VALUE .5.\n    88  MID-RANGE VALUE .5 THRU .9.\n",
        "    STOP RUN.\n",
    );
    assert!(errors(&src).is_empty(), "{:#?}", errors(&src));
}

#[test]
fn leading_point_in_evaluate_when() {
    let src = prog(
        "77  WS-NUM PIC S9V9 VALUE .5.\n",
        "    EVALUATE WS-NUM\n        WHEN .5 CONTINUE\n        WHEN OTHER CONTINUE\n    END-EVALUATE.\n    STOP RUN.\n",
    );
    assert!(errors(&src).is_empty(), "{:#?}", errors(&src));
}

// ── Malformed source is an error, never a silent reinterpretation ─────────────

/// `MOVE X TO Y.5` — a period glued to digits where no literal belongs.
///
/// COBOL-85 has no reading for this: the period ends the sentence, and `5`
/// cannot start a statement. The operator's ruling is to follow the standard and
/// **raise an error at compile time** rather than warn or guess. What must not
/// happen is that the new leading-point rule quietly turns the tail into an
/// extra receiver, making a typo compile.
#[test]
fn malformed_period_digits_is_an_error() {
    let src = prog(
        "01  X PIC 9(3) VALUE 1.\n01  Y PIC 9(3) VALUE 2.\n",
        "    MOVE X TO Y.5\n    STOP RUN.\n",
    );
    let errs = errors(&src);
    assert!(
        !errs.is_empty(),
        "`MOVE X TO Y.5` compiled silently — the leading-point rule swallowed a typo"
    );
}

// ── Unterminated literals are reported where they are written ─────────────────

/// A literal that is never closed on its line produces a diagnostic naming the
/// cause, **and the statement after it still parses**.
///
/// Before literals were confined to a line, the opening quote ran to the next
/// quotation mark anywhere in the file — so the error surfaced hundreds of
/// lines away, or as an end-of-file "expected PROCEDURE DIVISION", and
/// everything in between vanished.
#[test]
fn unterminated_literal_is_reported_locally() {
    let src = prog(
        "01  X PIC X(10).\n01  Y PIC X(10).\n",
        "    MOVE \"abc TO X.\n    MOVE \"def\" TO Y.\n    STOP RUN.\n",
    );
    let errs = errors(&src);
    assert!(
        errs.iter().any(|m| m.contains("unterminated alphanumeric literal")),
        "the cause was not named: {errs:#?}"
    );
    // Recovery: the damage stops at the newline. logos merges the unmatched
    // run, so the rest of *that* line is lost — but the next line's literal is
    // intact, which is the property that matters. Before this, the opening
    // quote reached the next quotation mark anywhere in the file.
    let toks = tokenize(&src, SourceFormat::Free);
    let strings: Vec<&String> = toks
        .iter()
        .filter_map(|t| match &t.token {
            cobolt_lexer::Token::StringLiteral(s) => Some(s),
            _ => None,
        })
        .collect();
    assert!(
        strings.contains(&&"def".to_string()),
        "the next line's literal was consumed too: {strings:?}"
    );
}
