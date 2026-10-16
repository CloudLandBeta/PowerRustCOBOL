// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! COMPUTE written without its verb: `A = B * C`, `A ROUNDED = B / C ON SIZE
//! ERROR … END-COMPUTE`. The parser builds the very `Stmt::Compute` that
//! `COMPUTE` builds, so these tests pin two things — the shape that opens a
//! statement, and that no neighbouring statement changes how it ends.

use cobolt_ast::expr::Expr;
use cobolt_ast::program::ProcedureBody;
use cobolt_ast::stmt::Stmt;
use cobolt_lexer::{tokenize, SourceFormat};
use cobolt_parser::{parse, Severity};

fn all_stmts(body: &str) -> Vec<Stmt> {
    let code = format!("IDENTIFICATION DIVISION.\nPROGRAM-ID. T.\nPROCEDURE DIVISION.\nMAIN.\n{body}");
    let result = parse(tokenize(&code, SourceFormat::Free));
    let errors: Vec<_> = result.diagnostics.iter().filter(|d| d.severity == Severity::Error).collect();
    assert!(errors.is_empty(), "parse errors: {errors:?}\n{body}");
    match result.program.expect("no program").procedure.body {
        ProcedureBody::Paragraphs(paras) => paras.into_iter().flat_map(|p| p.stmts).collect(),
        ProcedureBody::Sections(secs) => {
            secs.into_iter().flat_map(|s| s.paragraphs).flat_map(|p| p.stmts).collect()
        }
    }
}

fn computes(stmts: &[Stmt]) -> Vec<&Stmt> {
    stmts.iter().filter(|s| matches!(s, Stmt::Compute { .. })).collect()
}

#[test]
fn a_receiver_and_an_equals_sign_open_a_compute() {
    let stmts = all_stmts("    A = B * C\n    STOP RUN.\n");
    let c = computes(&stmts);
    assert_eq!(c.len(), 1, "{stmts:?}");
    let Stmt::Compute { targets, on_size_error, not_on_size_error, .. } = c[0] else { unreachable!() };
    assert_eq!(targets.len(), 1);
    assert!(!targets[0].1, "not ROUNDED");
    assert!(on_size_error.is_empty() && not_on_size_error.is_empty());
}

#[test]
fn rounded_goes_after_the_receiver() {
    let stmts = all_stmts("    A ROUNDED = B / C\n    STOP RUN.\n");
    let Stmt::Compute { targets, .. } = computes(&stmts)[0] else { unreachable!() };
    assert!(targets[0].1, "ROUNDED");
}

#[test]
fn the_size_phrases_and_their_terminator_belong_to_it() {
    let stmts = all_stmts(
        "    A ROUNDED = B * C\n        ON SIZE ERROR\n            DISPLAY \"BIG\"\n        NOT ON SIZE ERROR\n            DISPLAY \"OK\"\n    END-COMPUTE\n    DISPLAY \"AFTER\"\n    STOP RUN.\n",
    );
    let Stmt::Compute { on_size_error, not_on_size_error, .. } = computes(&stmts)[0] else { unreachable!() };
    assert_eq!(on_size_error.len(), 1);
    assert_eq!(not_on_size_error.len(), 1);
    // END-COMPUTE closed it: DISPLAY "AFTER" is the next statement, not part of the phrase.
    assert!(matches!(stmts[1], Stmt::Display { .. }), "{stmts:?}");
}

#[test]
fn a_qualified_a_subscripted_and_a_member_receiver_are_receivers() {
    let stmts = all_stmts(
        "    F OF G = 1\n    T(I J) = T(I) + 1\n    NUD-AGE::Value = NUD-AGE::Value + 1\n    TXT-1::Items(3) = 4\n    STOP RUN.\n",
    );
    assert_eq!(computes(&stmts).len(), 4, "{stmts:?}");
    let Stmt::Compute { targets, .. } = computes(&stmts)[2] else { unreachable!() };
    assert!(matches!(targets[0].0, Expr::Member { .. }), "{:?}", targets[0].0);
}

/// An operand list ends in front of an assignment, as it does in front of a
/// method call: the identifier on the next line is a new statement.
#[test]
fn an_operand_list_ends_in_front_of_an_assignment() {
    let stmts = all_stmts(
        "    DISPLAY A B\n    C = A + B\n    MOVE A TO D E\n    F = D\n    INITIALIZE G H\n    I = 1\n    CANCEL \"P\"\n    J = 2\n    STOP RUN.\n",
    );
    assert_eq!(computes(&stmts).len(), 4, "{stmts:?}");
    let Some(Stmt::Display { operands, .. }) = stmts.first() else { panic!("{stmts:?}") };
    assert_eq!(operands.len(), 2, "DISPLAY keeps its own two operands");
}

#[test]
fn a_run_of_assignments_is_a_run_of_statements() {
    let stmts = all_stmts("    A = 1\n    B = A + 1\n    C = B * 2\n    STOP RUN.\n");
    assert_eq!(computes(&stmts).len(), 3, "{stmts:?}");
}

#[test]
fn an_assignment_inside_if_else_and_a_scoped_phrase() {
    let stmts = all_stmts(
        "    IF A = B\n        C = 1\n    ELSE\n        C = 2\n    END-IF\n    ADD 1 TO D ON SIZE ERROR E = 0 END-ADD\n    STOP RUN.\n",
    );
    let Stmt::If { condition: _, then_stmts, else_stmts, .. } = &stmts[0] else { panic!("{stmts:?}") };
    assert!(matches!(then_stmts[0], Stmt::Compute { .. }));
    assert!(matches!(else_stmts[0], Stmt::Compute { .. }));
    let Stmt::Add { on_size_error, .. } = &stmts[1] else { panic!("{stmts:?}") };
    assert!(matches!(on_size_error[0], Stmt::Compute { .. }));
}

/// What did not change: the receiver series of the arithmetic verbs (`ROUNDED`
/// follows a receiver there too), a COMPUTE with several receivers, the
/// condition of an IF, and a method call after any of them.
#[test]
fn nothing_else_changes() {
    let stmts = all_stmts(
        "    ADD 1 TO A ROUNDED B ROUNDED\n    MULTIPLY 2 BY C ROUNDED D\n    COMPUTE E ROUNDED F = 1 + 2\n    IF A = B DISPLAY \"EQ\" END-IF\n    PERFORM UNTIL A = B\n        ADD 1 TO A\n    END-PERFORM\n    MOVE 1 TO C\n    TXT-1::Clear()\n    STOP RUN.\n",
    );
    let Some(Stmt::Add { to, .. }) = stmts.first() else { panic!("{stmts:?}") };
    assert_eq!(to.len(), 2, "ADD keeps its receiver series");
    let multiplies = stmts.iter().filter(|s| matches!(s, Stmt::Multiply { .. })).count();
    assert_eq!(multiplies, 2, "MULTIPLY keeps its receiver series (one statement per receiver): {stmts:?}");
    assert_eq!(computes(&stmts).len(), 1, "no other statement became a compute: {stmts:?}");
    let Stmt::Compute { targets, .. } = computes(&stmts)[0] else { unreachable!() };
    assert_eq!(targets.len(), 2, "COMPUTE keeps its receiver series");
    assert!(stmts.iter().any(|s| matches!(s, Stmt::InvokeExpr { .. })), "the method call is still a call");
}

/// Spec 093 AC2 (R6, R7, R9): every verb whose operand list is a run of names ends it in front of an
/// assignment — the next line's name is a statement, never one operand more — and the receiver series of
/// the arithmetic verbs, where `ROUNDED` follows a receiver, are unchanged.
#[test]
fn every_operand_list_ends_in_front_of_an_assignment() {
    for (verb_line, label) in [
        ("    ADD 1 TO A B", "ADD … TO a list of receivers"),
        ("    SUBTRACT 1 FROM A B", "SUBTRACT"),
        ("    MULTIPLY 2 BY A B", "MULTIPLY"),
        ("    DIVIDE 2 INTO A B", "DIVIDE"),
        ("    STRING \"a\" DELIMITED BY SIZE INTO A", "STRING"),
        ("    UNSTRING A DELIMITED BY \",\" INTO B C", "UNSTRING"),
        ("    CALL \"P\" USING A B", "CALL … USING"),
        ("    INSPECT A TALLYING N FOR ALL \"x\"", "INSPECT … TALLYING"),
        ("    SET I J TO 1", "SET"),
        ("    MOVE 1 TO A B", "MOVE"),
        ("    INITIALIZE A B", "INITIALIZE"),
        ("    DISPLAY A B", "DISPLAY"),
    ] {
        let stmts = all_stmts(&format!("{verb_line}\n    Z = 9\n    STOP RUN.\n"));
        assert_eq!(computes(&stmts).len(), 1, "{label}: the assignment after it is a statement of its own: {stmts:?}");
        assert!(!matches!(stmts.first(), Some(Stmt::Compute { .. })), "{label}: the verb statement comes first");
    }
}

#[test]
fn the_receiver_series_of_the_arithmetic_verbs_and_computes_two_receivers_are_unchanged() {
    // ROUNDED follows a receiver in every arithmetic verb, so it alone never ends the list (R7).
    let stmts = all_stmts("    ADD 1 TO A ROUNDED B ROUNDED\n    Z = 9\n    STOP RUN.\n");
    let Some(Stmt::Add { to, .. }) = stmts.first() else { panic!("{stmts:?}") };
    assert_eq!(to.len(), 2, "both receivers of the ADD: {stmts:?}");
    assert!(to.iter().all(|(_, rounded)| *rounded), "each carries ROUNDED");
    assert_eq!(computes(&stmts).len(), 1);

    let stmts = all_stmts("    MULTIPLY 2 BY A ROUNDED B ROUNDED\n    Z = 9\n    STOP RUN.\n");
    assert_eq!(computes(&stmts).len(), 1, "{stmts:?}");

    // COMPUTE's own list still stops at `=` only (R9).
    let stmts = all_stmts("    COMPUTE A B = 7\n    STOP RUN.\n");
    let Some(Stmt::Compute { targets, .. }) = stmts.first() else { panic!("{stmts:?}") };
    assert_eq!(targets.len(), 2, "two receivers");
}
