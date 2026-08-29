// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Grammar the NIST CCVS85 Nucleus module writes and the parser used to reject:
//! the `ALTER` series and the altered `GO TO`, receiver series on `MULTIPLY` /
//! `DIVIDE` format 1, `PERFORM` phrase order and its inline form, conditions
//! that are not a bare identifier, and abbreviated combined relations.
//!
//! Plus, from the Relative I/O module: the `RELATIVE KEY` clause with the word
//! `KEY` left out, and an unterminated conditional phrase inside an `IF`.

use cobolt_ast::expr::Condition;
use cobolt_ast::program::ProcedureBody;
use cobolt_ast::stmt::{PerformTarget, Stmt};
use cobolt_lexer::{tokenize, SourceFormat};
use cobolt_parser::{parse, Severity};

/// Parse a whole program and return every statement, in order, flattened
/// across paragraphs and sections.
fn all_stmts(code: &str) -> Vec<Stmt> {
    let result = parse(tokenize(code, SourceFormat::Free));
    let errors: Vec<_> = result
        .diagnostics
        .iter()
        .filter(|d| d.severity == Severity::Error)
        .collect();
    assert!(errors.is_empty(), "parse errors: {errors:?}");
    let proc = result.program.expect("no program").procedure;
    match proc.body {
        ProcedureBody::Paragraphs(paras) => paras.into_iter().flat_map(|p| p.stmts).collect(),
        ProcedureBody::Sections(secs) => secs
            .into_iter()
            .flat_map(|s| s.paragraphs)
            .flat_map(|p| p.stmts)
            .collect(),
    }
}

fn prog(body: &str) -> String {
    format!("IDENTIFICATION DIVISION.\nPROGRAM-ID. T.\nPROCEDURE DIVISION.\n{body}")
}

// ── ALTER ────────────────────────────────────────────────────────────────────

/// `ALTER a TO PROCEED TO b, c TO PROCEED TO d` is a **series**. It stays one
/// `Stmt::Alter` per pair, so the serialized AST is untouched.
#[test]
fn alter_takes_a_series_of_pairs() {
    let stmts = all_stmts(&prog(
        "MAIN.\n    ALTER PARA-A TO PROCEED TO PARA-B, PARA-C TO PROCEED TO PARA-D.\n\
         PARA-A. GO TO PARA-B.\nPARA-B. STOP RUN.\nPARA-C. GO TO PARA-D.\nPARA-D. EXIT.\n",
    ));
    let alters: Vec<&Stmt> = stmts
        .iter()
        .filter(|s| matches!(s, Stmt::Alter { .. }))
        .collect();
    assert_eq!(alters.len(), 2, "both pairs of the series: {stmts:?}");
}

/// An all-digit procedure name is legal, and an `ALTER` may name one.
#[test]
fn alter_accepts_all_digit_procedure_names() {
    let stmts = all_stmts(&prog(
        "MAIN.\n    ALTER 10 TO PROCEED TO 20.\n10. GO TO.\n20. STOP RUN.\n",
    ));
    assert!(stmts.iter().any(|s| matches!(s, Stmt::Alter { .. })));
}

/// `GO TO.` with no target is the altered GO TO: it parses, and until an
/// `ALTER` names a destination it simply falls through.
#[test]
fn bare_go_to_parses_with_no_target() {
    let stmts = all_stmts(&prog("MAIN.\n    GO TO.\n    STOP RUN.\n"));
    assert!(
        matches!(stmts.first(), Some(Stmt::GoTo { .. })),
        "expected a GO TO: {stmts:?}"
    );
}

/// An all-digit procedure name keeps its **leading zeros**: `00001` and
/// `000001` are two different paragraphs, and both parse to the integer 1.
#[test]
fn all_digit_procedure_names_keep_their_width() {
    let src = prog("MAIN.\n    GO TO 00001.\n00001. STOP RUN.\n000001. STOP RUN.\n");
    let result = parse(tokenize(&src, SourceFormat::Free));
    let names: Vec<String> = match result.program.expect("no program").procedure.body {
        ProcedureBody::Paragraphs(paras) => paras.into_iter().map(|p| p.name).collect(),
        ProcedureBody::Sections(secs) => secs
            .into_iter()
            .flat_map(|s| s.paragraphs)
            .map(|p| p.name)
            .collect(),
    };
    assert!(names.contains(&"00001".to_string()), "{names:?}");
    assert!(names.contains(&"000001".to_string()), "{names:?}");
}

// ── Arithmetic receiver series ───────────────────────────────────────────────

/// `MULTIPLY a BY b ROUNDED c ROUNDED d` — format 1 takes a series of
/// receivers, each multiplied by its own current value.
#[test]
fn multiply_format_1_takes_a_receiver_series() {
    let stmts = all_stmts(&prog(
        "MAIN.\n    MULTIPLY WS-A BY WS-B ROUNDED WS-C ROUNDED WS-D.\n    STOP RUN.\n",
    ));
    let n = stmts
        .iter()
        .filter(|s| matches!(s, Stmt::Multiply { .. }))
        .count();
    assert_eq!(n, 3, "one MULTIPLY per receiver: {stmts:?}");
}

#[test]
fn divide_format_1_takes_a_receiver_series() {
    let stmts = all_stmts(&prog(
        "MAIN.\n    DIVIDE WS-A INTO WS-B WS-C ROUNDED.\n    STOP RUN.\n",
    ));
    let n = stmts
        .iter()
        .filter(|s| matches!(s, Stmt::Divide { .. }))
        .count();
    assert_eq!(n, 2, "one DIVIDE per receiver: {stmts:?}");
}

// ── PERFORM ──────────────────────────────────────────────────────────────────

/// `WITH TEST` may be written before the phrase it qualifies.
#[test]
fn perform_takes_with_test_before_varying() {
    let stmts = all_stmts(&prog(
        "MAIN.\n    PERFORM PARA-A THRU PARA-B WITH TEST BEFORE\n\
         \x20       VARYING WS-I FROM 1 BY 1 UNTIL WS-I EQUAL TO 5.\n    STOP RUN.\n\
         PARA-A. CONTINUE.\nPARA-B. EXIT.\n",
    ));
    assert!(
        stmts.iter().any(|s| matches!(
            s,
            Stmt::Perform {
                target: PerformTarget::Varying { .. },
                ..
            }
        )),
        "{stmts:?}"
    );
}

/// A repeat count may be a **subscripted** data item.
#[test]
fn perform_count_may_be_subscripted() {
    let stmts = all_stmts(&prog(
        "MAIN.\n    PERFORM PARA-A TABLE5-NUM (INDEX5) TIMES.\n    STOP RUN.\nPARA-A. CONTINUE.\n",
    ));
    assert!(
        stmts.iter().any(|s| matches!(
            s,
            Stmt::Perform {
                target: PerformTarget::Times { .. },
                ..
            }
        )),
        "{stmts:?}"
    );
}

/// `PERFORM imperative … END-PERFORM` with no TIMES/UNTIL/VARYING runs its
/// body once.
#[test]
fn perform_inline_without_a_phrase() {
    let stmts = all_stmts(&prog(
        "MAIN.\n    PERFORM MOVE 88 TO WS-A\n            MOVE 99 TO WS-B\n\
         \x20   END-PERFORM.\n    STOP RUN.\n",
    ));
    match stmts.first() {
        Some(Stmt::Perform {
            target: PerformTarget::Inline { stmts: body },
            ..
        }) => assert_eq!(body.len(), 2, "{body:?}"),
        other => panic!("expected an inline PERFORM, got {other:?}"),
    }
}

/// A paragraph-name may be qualified by its section.
#[test]
fn perform_accepts_a_qualified_procedure_name() {
    let stmts = all_stmts(&prog(
        "SECT-1 SECTION.\nPAR-1A. CONTINUE.\n\
         SECT-2 SECTION.\nMAIN.\n    PERFORM PAR-1A OF SECT-1.\n    STOP RUN.\n",
    ));
    assert!(
        stmts.iter().any(|s| matches!(s, Stmt::Perform { .. })),
        "{stmts:?}"
    );
}

// ── Conditions ───────────────────────────────────────────────────────────────

fn condition_of(stmts: &[Stmt]) -> &Condition {
    stmts
        .iter()
        .find_map(|s| match s {
            Stmt::If { condition, .. } => Some(condition),
            _ => None,
        })
        .expect("no IF statement")
}

/// A condition-name may be reached through a reference: subscripted, or
/// qualified by the group that holds its host item.
#[test]
fn condition_name_may_be_subscripted_or_qualified() {
    let stmts = all_stmts(&prog(
        "MAIN.\n    IF FIRSTZ (1) AND LASTA (26)\n       CONTINUE\n    END-IF.\n    STOP RUN.\n",
    ));
    let cond = condition_of(&stmts);
    assert!(
        matches!(cond, Condition::And(a, b, _)
            if matches!(**a, Condition::ConditionRef(..))
            && matches!(**b, Condition::ConditionRef(..))),
        "{cond:?}"
    );

    let stmts = all_stmts(&prog(
        "MAIN.\n    IF A OF IF-D32\n       CONTINUE\n    END-IF.\n    STOP RUN.\n",
    ));
    assert!(
        matches!(condition_of(&stmts), Condition::ConditionRef(..)),
        "{:?}",
        condition_of(&stmts)
    );
}

/// A parenthesised **arithmetic** expression is an operand when a relational
/// operator follows the closing parenthesis, not a nested condition.
#[test]
fn parenthesised_arithmetic_is_an_operand_of_the_comparison() {
    let stmts = all_stmts(&prog(
        "MAIN.\n    PERFORM PARA-A UNTIL (WS-A + 12) EQUAL TO 100.\n    STOP RUN.\n\
         PARA-A. CONTINUE.\n",
    ));
    assert!(
        stmts.iter().any(|s| matches!(
            s,
            Stmt::Perform {
                target: PerformTarget::Until { .. },
                ..
            }
        )),
        "{stmts:?}"
    );
}

/// `GREATER THAN OR EQUAL TO` is one operator; the `OR EQUAL` may be written
/// on either side of the optional `THAN`.
#[test]
fn greater_than_or_equal_to_is_one_operator() {
    let stmts = all_stmts(&prog(
        "MAIN.\n    IF WS-A GREATER THAN OR EQUAL TO WS-B\n       CONTINUE\n    END-IF.\n\
         \x20   STOP RUN.\n",
    ));
    assert!(
        matches!(condition_of(&stmts), Condition::Comparison { .. }),
        "{:?}",
        condition_of(&stmts)
    );
}

/// A literal that is followed by a relational operator is the **subject** of a
/// full relation, not the object of an abbreviation.
#[test]
fn a_literal_subject_is_not_an_abbreviation_object() {
    let stmts = all_stmts(&prog(
        "MAIN.\n    IF WS-B GREATER THAN WS-A AND 20 LESS THAN WS-C\n       CONTINUE\n\
         \x20   END-IF.\n    STOP RUN.\n",
    ));
    let cond = condition_of(&stmts);
    assert!(
        matches!(cond, Condition::And(_, b, _) if matches!(**b, Condition::Comparison { .. })),
        "{cond:?}"
    );
}

/// The `ELSE` of an enclosing `IF` is not swallowed by an `ON SIZE ERROR`
/// imperative, nor by a nested `IF`'s own ELSE branch.
#[test]
fn else_belongs_to_its_own_if() {
    let stmts = all_stmts(&prog(
        "MAIN.\n    IF WS-X EQUAL TO \"X\"\n       MOVE \"Z\" TO WS-X\n\
         \x20      ADD 1 TO WS-N ON SIZE ERROR\n       MOVE \"Y\" TO WS-X\n\
         \x20   ELSE\n       ADD 2 TO WS-N ON SIZE ERROR\n       MOVE \"W\" TO WS-X.\n\
         \x20   STOP RUN.\n",
    ));
    match condition_of(&stmts) {
        Condition::Comparison { .. } => {}
        other => panic!("{other:?}"),
    }
    let has_else = stmts.iter().any(|s| match s {
        Stmt::If { else_stmts, .. } => !else_stmts.is_empty(),
        _ => false,
    });
    assert!(has_else, "the ELSE branch was lost: {stmts:?}");
}

// ── Relative I/O grammar ─────────────────────────────────────────────────────

/// Parse a whole program and hand back its `FILE-CONTROL` entries.
fn file_controls(code: &str) -> Vec<cobolt_ast::program::FileControl> {
    let result = parse(tokenize(code, SourceFormat::Free));
    let errors: Vec<_> = result
        .diagnostics
        .iter()
        .filter(|d| d.severity == Severity::Error)
        .collect();
    assert!(errors.is_empty(), "parse errors: {errors:?}");
    result
        .program
        .expect("no program")
        .environment
        .expect("no ENVIRONMENT DIVISION")
        .input_output
        .expect("no INPUT-OUTPUT SECTION")
        .file_controls
}

fn select(clauses: &str) -> String {
    format!(
        "IDENTIFICATION DIVISION.\nPROGRAM-ID. T.\nENVIRONMENT DIVISION.\n\
         INPUT-OUTPUT SECTION.\nFILE-CONTROL.\n\
         SELECT F ASSIGN TO \"f.dat\"\n{clauses}\nDATA DIVISION.\n\
         PROCEDURE DIVISION.\nMAIN.\n    STOP RUN.\n"
    )
}

/// `RELATIVE KEY IS`, `RELATIVE KEY`, and plain `RELATIVE data-name` all name
/// the same thing.
///
/// The suite writes all three. Ten RL members spell it `RELATIVE RL-FD2-KEY`
/// with no `KEY` at all, and read as a bare organization clause the key was
/// consumed and silently dropped — the file then had no record number and every
/// random `WRITE` came back 24.
#[test]
fn relative_key_is_named_with_or_without_the_word_key() {
    for clauses in [
        "    ORGANIZATION IS RELATIVE\n    ACCESS MODE IS RANDOM\n    RELATIVE KEY IS RK.",
        "    ORGANIZATION IS RELATIVE\n    ACCESS MODE IS RANDOM\n    RELATIVE KEY RK.",
        "    ORGANIZATION RELATIVE\n    ACCESS RANDOM\n    RELATIVE RK.",
    ] {
        let fcs = file_controls(&select(clauses));
        let fc = fcs.first().expect("no SELECT parsed");
        assert_eq!(
            fc.organization,
            cobolt_ast::program::FileOrganization::Relative,
            "organization lost: {clauses}"
        );
        assert_eq!(
            fc.relative_key.as_deref().map(str::to_uppercase),
            Some("RK".to_string()),
            "RELATIVE KEY lost: {clauses}"
        );
    }
}

/// A bare `RELATIVE` with no data-name after it is still the organization.
#[test]
fn bare_relative_is_the_organization_not_a_key() {
    let fcs = file_controls(&select("    RELATIVE\n    ACCESS SEQUENTIAL."));
    let fc = fcs.first().expect("no SELECT parsed");
    assert_eq!(
        fc.organization,
        cobolt_ast::program::FileOrganization::Relative
    );
    assert_eq!(fc.relative_key, None);
}

/// An `INVALID KEY` phrase with no `END-WRITE` stops at the enclosing `ELSE`.
///
/// RL210A writes exactly this shape, and reading the phrase past `ELSE` meant
/// the program did not parse at all.
#[test]
fn an_unterminated_invalid_key_phrase_stops_at_else() {
    let stmts = all_stmts(&prog(
        "MAIN.\n    IF WS-N < 201\n            WRITE REC-A\n\
         \x20           INVALID KEY GO TO BAD\n    ELSE\n            MOVE 16 TO WS-N\n\
         \x20           WRITE REC-B\n            INVALID KEY GO TO BAD.\n    STOP RUN.\n",
    ));
    let Some(Stmt::If {
        then_stmts,
        else_stmts,
        ..
    }) = stmts.first()
    else {
        panic!("first statement is not an IF: {stmts:?}");
    };
    assert_eq!(then_stmts.len(), 1, "THEN branch: {then_stmts:?}");
    assert_eq!(else_stmts.len(), 2, "ELSE branch was lost: {else_stmts:?}");
}
