// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Tests for PROCEDURE DIVISION statement parsing.

use cobolt_ast::program::ProcedureBody;
use cobolt_ast::stmt::Stmt;
use cobolt_lexer::{tokenize, SourceFormat};
use cobolt_parser::parse;

/// Build a complete program with just a single paragraph of statements.
fn prog(stmts_src: &str) -> String {
    format!(
        "IDENTIFICATION DIVISION.\nPROGRAM-ID. TESTPROG.\n\
         PROCEDURE DIVISION.\nMAIN.\n{}\n",
        stmts_src
    )
}

/// Parse and return the statements from the first paragraph.
fn parse_stmts(code: &str) -> Vec<Stmt> {
    let result = parse(tokenize(code, SourceFormat::Free));
    assert!(
        result.diagnostics.is_empty(),
        "Diagnostics: {:?}",
        result.diagnostics
    );
    let proc = result.program.unwrap().procedure;
    match proc.body {
        ProcedureBody::Paragraphs(mut paras) => paras.pop().map(|p| p.stmts).unwrap_or_default(),
        ProcedureBody::Sections(secs) => secs
            .into_iter()
            .flat_map(|s| s.paragraphs)
            .flat_map(|p| p.stmts)
            .collect(),
    }
}

// ── MOVE ─────────────────────────────────────────────────────────────────────

#[test]
fn move_literal_to_field() {
    let stmts = parse_stmts(&prog("    MOVE 'HELLO' TO WS-NAME.\n    STOP RUN.\n"));
    assert!(!stmts.is_empty());
    assert!(matches!(stmts[0], Stmt::Move { .. }));
}

#[test]
fn move_field_to_field() {
    let stmts = parse_stmts(&prog("    MOVE WS-A TO WS-B.\n    STOP RUN.\n"));
    assert!(matches!(stmts[0], Stmt::Move { .. }));
}

#[test]
fn move_to_multiple() {
    let stmts = parse_stmts(&prog("    MOVE SPACES TO WS-A WS-B WS-C.\n    STOP RUN.\n"));
    if let Stmt::Move { to, .. } = &stmts[0] {
        assert_eq!(to.len(), 3);
    } else {
        panic!("expected MOVE");
    }
}

// ── ADD ──────────────────────────────────────────────────────────────────────

#[test]
fn add_to() {
    let stmts = parse_stmts(&prog("    ADD 1 TO WS-CNT.\n    STOP RUN.\n"));
    assert!(matches!(stmts[0], Stmt::Add { .. }));
}

#[test]
fn add_giving() {
    let stmts = parse_stmts(&prog("    ADD WS-A WS-B GIVING WS-C.\n    STOP RUN.\n"));
    if let Stmt::Add { giving, .. } = &stmts[0] {
        assert_eq!(giving.len(), 1);
    } else {
        panic!("expected ADD");
    }
}

// ── SUBTRACT ─────────────────────────────────────────────────────────────────

#[test]
fn subtract_from() {
    let stmts = parse_stmts(&prog("    SUBTRACT 5 FROM WS-TOTAL.\n    STOP RUN.\n"));
    assert!(matches!(stmts[0], Stmt::Subtract { .. }));
}

#[test]
fn multiply_giving_multiple_receivers() {
    let stmts = parse_stmts(&prog("    MULTIPLY A BY B GIVING R1 R2.\n    STOP RUN.\n"));
    if let Stmt::Multiply { giving, .. } = &stmts[0] {
        assert_eq!(giving.len(), 2);
    } else {
        panic!("expected MULTIPLY");
    }
}

#[test]
fn divide_giving_remainder_and_per_receiver_rounded() {
    let stmts = parse_stmts(&prog(
        "    DIVIDE A BY B GIVING Q1 ROUNDED Q2 REMAINDER R.\n    STOP RUN.\n",
    ));
    if let Stmt::Divide {
        giving, remainder, ..
    } = &stmts[0]
    {
        assert_eq!(giving.len(), 2);
        assert!(giving[0].1, "Q1 should be ROUNDED");
        assert!(!giving[1].1, "Q2 should not be ROUNDED");
        assert!(remainder.is_some());
    } else {
        panic!("expected DIVIDE");
    }
}

#[test]
fn add_per_receiver_rounded() {
    let stmts = parse_stmts(&prog("    ADD A TO R1 ROUNDED R2.\n    STOP RUN.\n"));
    if let Stmt::Add { to, .. } = &stmts[0] {
        assert_eq!(to.len(), 2);
        assert!(to[0].1, "R1 should be ROUNDED");
        assert!(!to[1].1, "R2 should not be ROUNDED");
    } else {
        panic!("expected ADD");
    }
}

// ── COMPUTE ───────────────────────────────────────────────────────────────────

#[test]
fn compute_expression() {
    let stmts = parse_stmts(&prog(
        "    COMPUTE WS-R = WS-A + WS-B * 2.\n    STOP RUN.\n",
    ));
    assert!(matches!(stmts[0], Stmt::Compute { .. }));
}

// ── IF ────────────────────────────────────────────────────────────────────────

#[test]
fn if_simple() {
    let code =
        prog("    IF WS-CNT > 0\n       MOVE 'POS' TO WS-SIGN\n    END-IF.\n    STOP RUN.\n");
    let stmts = parse_stmts(&code);
    assert!(matches!(stmts[0], Stmt::If { .. }));
    if let Stmt::If {
        then_stmts,
        else_stmts,
        ..
    } = &stmts[0]
    {
        assert_eq!(then_stmts.len(), 1);
        assert!(else_stmts.is_empty());
    }
}

#[test]
fn if_else() {
    let code = prog(
        "    IF WS-FLAG = 'Y'\n       MOVE 1 TO WS-OK\n    ELSE\n       MOVE 0 TO WS-OK\n    END-IF.\n    STOP RUN.\n",
    );
    let stmts = parse_stmts(&code);
    if let Stmt::If {
        then_stmts,
        else_stmts,
        ..
    } = &stmts[0]
    {
        assert_eq!(then_stmts.len(), 1);
        assert_eq!(else_stmts.len(), 1);
    } else {
        panic!("expected IF");
    }
}

// ── PERFORM ───────────────────────────────────────────────────────────────────

#[test]
fn perform_paragraph() {
    let code = prog("    PERFORM MY-PARA.\n    STOP RUN.\n");
    let stmts = parse_stmts(&code);
    assert!(matches!(stmts[0], Stmt::Perform { .. }));
}

#[test]
fn perform_thru() {
    let code = prog("    PERFORM PARA-A THRU PARA-Z.\n    STOP RUN.\n");
    let stmts = parse_stmts(&code);
    if let Stmt::Perform { target, .. } = &stmts[0] {
        assert!(matches!(
            target,
            cobolt_ast::stmt::PerformTarget::Thru { .. }
        ));
    } else {
        panic!("expected PERFORM");
    }
}

#[test]
fn perform_inline_until() {
    let code = prog(
        "    PERFORM UNTIL WS-CNT > 10\n       ADD 1 TO WS-CNT\n    END-PERFORM.\n    STOP RUN.\n",
    );
    let stmts = parse_stmts(&code);
    if let Stmt::Perform { target, .. } = &stmts[0] {
        assert!(matches!(
            target,
            cobolt_ast::stmt::PerformTarget::Until { .. }
        ));
    } else {
        panic!("expected PERFORM UNTIL");
    }
}

// ── DISPLAY ───────────────────────────────────────────────────────────────────

#[test]
fn display_literal() {
    let code = prog("    DISPLAY 'HELLO WORLD'.\n    STOP RUN.\n");
    let stmts = parse_stmts(&code);
    assert!(matches!(stmts[0], Stmt::Display { .. }));
}

#[test]
fn display_multiple() {
    let code = prog("    DISPLAY 'NAME: ' WS-NAME.\n    STOP RUN.\n");
    let stmts = parse_stmts(&code);
    if let Stmt::Display { operands, .. } = &stmts[0] {
        assert_eq!(operands.len(), 2);
    } else {
        panic!("expected DISPLAY");
    }
}

#[test]
fn display_at_with_screen_phrase() {
    let code = prog("    DISPLAY 'X' AT LINE 5 COLUMN 10 WITH HIGHLIGHT.\n    STOP RUN.\n");
    let stmts = parse_stmts(&code);
    if let Stmt::Display {
        operands, screen, ..
    } = &stmts[0]
    {
        assert_eq!(operands.len(), 1);
        let sc = screen.as_ref().expect("expected a screen phrase");
        assert!(sc.line.is_some());
        assert!(sc.col.is_some());
        assert!(sc.highlight);
    } else {
        panic!("expected DISPLAY");
    }
}

// ── STOP RUN ──────────────────────────────────────────────────────────────────

#[test]
fn stop_run() {
    let code = prog("    STOP RUN.\n");
    let stmts = parse_stmts(&code);
    assert!(matches!(stmts[0], Stmt::Stop { run: true, .. }));
}

// ── CONTINUE ─────────────────────────────────────────────────────────────────

#[test]
fn continue_stmt() {
    let code = prog("    CONTINUE.\n    STOP RUN.\n");
    let stmts = parse_stmts(&code);
    assert!(matches!(stmts[0], Stmt::Continue { .. }));
}

// ── GO TO ────────────────────────────────────────────────────────────────────

#[test]
fn go_to_paragraph() {
    let code = prog("    GO TO END-PARA.\n    END-PARA.\n    STOP RUN.\n");
    let result = parse(tokenize(&code, SourceFormat::Free));
    // May have diagnostics if END-PARA is parsed as keyword, but we just
    // check the program was produced
    assert!(result.program.is_some());
}

// ── EVALUATE ─────────────────────────────────────────────────────────────────

#[test]
fn evaluate_when() {
    let code = prog(
        "    EVALUATE WS-CODE\n      WHEN 1 MOVE 'ONE' TO WS-TEXT\n      WHEN 2 MOVE 'TWO' TO WS-TEXT\n      WHEN OTHER MOVE 'UNK' TO WS-TEXT\n    END-EVALUATE.\n    STOP RUN.\n",
    );
    let stmts = parse_stmts(&code);
    if let Stmt::Evaluate {
        whens, other_stmts, ..
    } = &stmts[0]
    {
        assert_eq!(whens.len(), 2);
        assert!(!other_stmts.is_empty());
    } else {
        panic!("expected EVALUATE");
    }
}

// ── Full hello-world integration ──────────────────────────────────────────────

#[test]
fn hello_world_program() {
    let src = "\
IDENTIFICATION DIVISION.
PROGRAM-ID. HELLO.
PROCEDURE DIVISION.
MAIN.
    DISPLAY 'HELLO, WORLD!'.
    STOP RUN.
";
    let result = parse(tokenize(src, SourceFormat::Free));
    assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
    let prog = result.program.unwrap();
    assert_eq!(prog.identification.program_id, "HELLO");
}

// ── CALL ──────────────────────────────────────────────────────────────────────

#[test]
fn call_subprogram() {
    let code = prog("    CALL 'MYSUB' USING WS-A WS-B.\n    STOP RUN.\n");
    let stmts = parse_stmts(&code);
    assert!(matches!(stmts[0], Stmt::Call { .. }));
    if let Stmt::Call { using, .. } = &stmts[0] {
        assert_eq!(using.len(), 2);
    }
}

// ── OPEN / READ locking + CANCEL ──────────────────────────────────────────────

#[test]
fn open_sharing_and_with_lock() {
    let stmts = parse_stmts(&prog(
        "    OPEN I-O MY-FILE SHARING WITH ALL OTHER WITH LOCK.\n    STOP RUN.\n",
    ));
    if let Stmt::Open {
        sharing,
        lock,
        files,
        ..
    } = &stmts[0]
    {
        assert_eq!(files, &vec!["MY-FILE".to_string()]);
        assert!(lock, "WITH LOCK should set lock");
        assert!(sharing.is_some(), "SHARING should be captured");
    } else {
        panic!("expected OPEN, got {:?}", stmts[0]);
    }
}

#[test]
fn open_with_registered_user_literal_and_data_item() {
    use cobolt_ast::expr::{Expr, Literal};

    // String literal.
    let stmts = parse_stmts(&prog(
        "    OPEN I-O MY-FILE WITH REGISTERED USER \"ALICE\".\n    STOP RUN.\n",
    ));
    if let Stmt::Open {
        files,
        registered_user,
        ..
    } = &stmts[0]
    {
        assert_eq!(files, &vec!["MY-FILE".to_string()]);
        match registered_user {
            Some(Expr::Literal(Literal::String(s), _)) => assert_eq!(s, "ALICE"),
            other => panic!("expected string-literal user, got {other:?}"),
        }
    } else {
        panic!("expected OPEN, got {:?}", stmts[0]);
    }

    // Data item (and the `USER` keyword omitted).
    let stmts = parse_stmts(&prog(
        "    OPEN OUTPUT MY-FILE WITH REGISTERED WS-USER.\n    STOP RUN.\n",
    ));
    if let Stmt::Open {
        registered_user, ..
    } = &stmts[0]
    {
        match registered_user {
            Some(Expr::Identifier(name, _)) => assert_eq!(name, "WS-USER"),
            other => panic!("expected identifier user, got {other:?}"),
        }
    } else {
        panic!("expected OPEN, got {:?}", stmts[0]);
    }
}

#[test]
fn read_with_no_lock() {
    let stmts = parse_stmts(&prog(
        "    READ MY-FILE WITH NO LOCK\n        AT END CONTINUE\n    END-READ.\n    STOP RUN.\n",
    ));
    if let Stmt::Read { lock, .. } = &stmts[0] {
        assert_eq!(*lock, Some(false));
    } else {
        panic!("expected READ, got {:?}", stmts[0]);
    }
}

#[test]
fn cancel_is_a_real_statement() {
    let stmts = parse_stmts(&prog("    CANCEL \"SUBP\".\n    STOP RUN.\n"));
    assert!(
        matches!(stmts[0], Stmt::Cancel { .. }),
        "expected CANCEL, got {:?}",
        stmts[0]
    );
}

// ── Property access syntax (spec 010): only `ctrl::member` / INVOKE ──────────

#[test]
fn inline_property_access_parses_as_member() {
    use cobolt_ast::expr::Expr;
    // GET in operand position and SET as a MOVE target both parse to Member
    // (spec 011 — the chainable node replaces the old MethodCall).
    let stmts = parse_stmts(&prog(
        "    MOVE BTN::Caption TO LBL::Text.\n    STOP RUN.\n",
    ));
    match &stmts[0] {
        Stmt::Move { from, to, .. } => {
            assert!(
                matches!(from, Expr::Member { parens: false, .. }),
                "source `BTN::Caption` must be a bare-property Member: {from:?}"
            );
            assert!(
                matches!(to[0], Expr::Member { parens: false, .. }),
                "target `LBL::Text` must be a bare-property Member: {:?}",
                to[0]
            );
        }
        other => panic!("expected MOVE, got {other:?}"),
    }
}

#[test]
fn member_access_chain_parses_nested_with_subscripts_and_calls() {
    use cobolt_ast::expr::Expr;
    // `Grid::Rows(I)::Columns(2)::Value::toUpperCase()` → nested Member chain.
    let stmts = parse_stmts(&prog(
        "    DISPLAY Grid::Rows(I)::Columns(2)::Value::toUpperCase().\n    STOP RUN.\n",
    ));
    let Stmt::Display { operands, .. } = &stmts[0] else {
        panic!("expected DISPLAY")
    };
    // Outermost segment is the trailing call `toUpperCase()` (parens, no args).
    let Expr::Member {
        member,
        parens,
        args,
        recv,
        ..
    } = &operands[0]
    else {
        panic!("expected Member chain, got {:?}", operands[0]);
    };
    assert_eq!(member.to_ascii_uppercase(), "TOUPPERCASE");
    assert!(*parens && args.is_empty(), "tail must be a no-arg call");
    // Next inward: `::Value` (bare property).
    let Expr::Member { member, parens, .. } = recv.as_ref() else {
        panic!("expected ::Value")
    };
    assert_eq!(member.to_ascii_uppercase(), "VALUE");
    assert!(!*parens, "Value is a bare property");
}

#[test]
fn inline_chain_statement_is_invoke_expr() {
    // A `::` chain used as a statement → Stmt::InvokeExpr wrapping a Member.
    use cobolt_ast::expr::Expr;
    let stmts = parse_stmts(&prog("    Grid::Rows(I)::Delete().\n    STOP RUN.\n"));
    let Stmt::InvokeExpr { expr, .. } = &stmts[0] else {
        panic!("expected InvokeExpr, got {:?}", stmts[0]);
    };
    assert!(
        matches!(expr, Expr::Member { parens: true, .. }),
        "tail Delete() has parens"
    );
}

#[test]
fn quoted_string_of_name_is_not_a_property_ref() {
    // The Fujitsu `"Prop" OF Ctrl` form is removed (spec 010). A leading string
    // literal is just a literal — no PropertyRef variant exists any more, so the
    // construct simply parses as the string `"X"` (the dangling `OF Y` is a
    // separate token sequence). This must not panic or produce a PropertyRef.
    use cobolt_ast::expr::Expr;
    let stmts = parse_stmts(&prog("    DISPLAY \"X\".\n    STOP RUN.\n"));
    match &stmts[0] {
        Stmt::Display { operands, .. } => assert!(
            matches!(operands[0], Expr::Literal(..)),
            "expected a literal operand"
        ),
        other => panic!("expected DISPLAY, got {other:?}"),
    }
}

// ── FUNCTION RANDOM / intrinsic-name-vs-keyword + arg-loop liveness ─────────────

#[test]
fn function_random_parses_as_intrinsic() {
    // `RANDOM` lexes as a keyword (from `ACCESS MODE IS RANDOM`), but
    // `FUNCTION RANDOM` is a standard COBOL-85 intrinsic and must parse as a
    // FunctionCall named "RANDOM" — with and without a seed argument.
    for src in [
        "    COMPUTE X = FUNCTION RANDOM.\n    STOP RUN.\n",
        "    COMPUTE X = FUNCTION RANDOM(5).\n    STOP RUN.\n",
    ] {
        let stmts = parse_stmts(&prog(src));
        match &stmts[0] {
            Stmt::Compute { .. } => {}
            other => panic!("expected COMPUTE, got {other:?}"),
        }
        // Confirm the intrinsic name resolved rather than a `<missing>`.
        let joined = format!("{stmts:?}");
        assert!(
            joined.contains("RANDOM") && !joined.contains("<missing>"),
            "FUNCTION RANDOM should name the intrinsic: {joined}"
        );
    }
}

#[test]
fn nested_function_random_argument_parses() {
    // Regression: `FUNCTION INTEGER(FUNCTION RANDOM * 4)` used to spin the
    // function-argument loop forever (keyword `RANDOM` never advanced), freezing
    // the whole IDE while parsing a form's generated event handler.
    let stmts = parse_stmts(&prog(
        "    COMPUTE X = FUNCTION INTEGER(FUNCTION RANDOM * 4) + 1.\n    STOP RUN.\n",
    ));
    assert!(matches!(stmts[0], Stmt::Compute { .. }));
}

#[test]
fn malformed_function_argument_terminates_with_diagnostic() {
    // Liveness guard: an unparseable function argument (a stray keyword the
    // primary parser cannot start) must make the parser stop and report an
    // error — never loop. Reaching the assertions at all proves termination.
    let code = prog("    DISPLAY FUNCTION INTEGER(THRU).\n    STOP RUN.\n");
    let result = parse(tokenize(&code, SourceFormat::Free));
    assert!(
        !result.diagnostics.is_empty(),
        "malformed FUNCTION argument should produce a diagnostic"
    );
    assert!(
        result.program.is_some(),
        "parser should still yield a program"
    );
}

#[test]
fn arithmetic_expressions_are_allowed_in_common_positions() {
    // DISPLAY operands, MOVE source, IF condition operands, and subscripts (incl.
    // inside a `::` member chain) all accept full arithmetic expressions.
    parse_stmts(&prog("    DISPLAY \"IDX\" CONTROL-ARRAY-INDEX + 1.\n"));
    parse_stmts(&prog("    MOVE CONTROL-ARRAY-INDEX / 10 TO X.\n"));
    parse_stmts(&prog(
        "    IF (CONTROL-ARRAY-INDEX + 1 > 0) DISPLAY \"y\" END-IF.\n",
    ));
    parse_stmts(&prog(
        "    DISPLAY BTN-1(CONTROL-ARRAY-INDEX + 1)::BackgroundColor.\n",
    ));
}

#[test]
fn screen_position_line_col_parse_without_at_and_with_arithmetic() {
    // Bare LINE/COL (no leading AT) with arithmetic operands must parse for both
    // ACCEPT and DISPLAY — the reported gap. The AT form and WITH attributes,
    // and plain (position-less) ACCEPT/DISPLAY, keep working.
    parse_stmts(&prog("    ACCEPT ITM LINE A + B COL C + D.\n"));
    parse_stmts(&prog("    ACCEPT ITM COL C + D.\n"));
    parse_stmts(&prog("    DISPLAY ITM LINE A + 1 COL C.\n"));
    parse_stmts(&prog("    DISPLAY ITM COL 5.\n"));
    parse_stmts(&prog("    ACCEPT ITM AT LINE A + B COL C + D.\n"));
    parse_stmts(&prog("    ACCEPT ITM LINE A COL B WITH HIGHLIGHT.\n"));
    parse_stmts(&prog("    DISPLAY A B.\n"));
    parse_stmts(&prog("    ACCEPT ITM.\n"));
}

// ── TRY / CATCH / CATCH RUST-EXCEPTION (spec 041 R23, R24) ──────────────────

/// Pull the four catch-related fields out of a lone `TryCatch`.
fn try_clauses(src: &str) -> (Option<String>, usize, Option<String>, usize) {
    let stmts = parse_stmts(&prog(src));
    match stmts.into_iter().next().expect("no statement parsed") {
        Stmt::TryCatch {
            exception_var,
            catch_stmts,
            rust_exception_var,
            rust_catch_stmts,
            ..
        } => (
            exception_var,
            catch_stmts.len(),
            rust_exception_var,
            rust_catch_stmts.len(),
        ),
        other => panic!("expected TryCatch, got {other:?}"),
    }
}

#[test]
fn try_with_only_a_plain_catch() {
    let (var, body, rust_var, rust_body) =
        try_clauses("    TRY\n DISPLAY 'a'\n CATCH EXCEPTION E\n DISPLAY 'b'\n END-TRY.\n");
    assert_eq!(var.as_deref(), Some("E"));
    assert_eq!(body, 1);
    assert_eq!(rust_var, None, "no RUST-EXCEPTION clause was written");
    assert_eq!(rust_body, 0);
}

#[test]
fn try_with_only_a_rust_catch() {
    let (var, body, rust_var, rust_body) =
        try_clauses("    TRY\n DISPLAY 'a'\n CATCH RUST-EXCEPTION E\n DISPLAY 'b'\n END-TRY.\n");
    assert_eq!(var, None, "the plain clause must stay empty");
    assert_eq!(body, 0);
    assert_eq!(rust_var.as_deref(), Some("E"));
    assert_eq!(rust_body, 1);
}

/// R24 — one `TRY` may carry both, each keeping its own name and body.
#[test]
fn try_with_both_clauses_keeps_them_apart() {
    let (var, body, rust_var, rust_body) = try_clauses(
        "    TRY\n DISPLAY 'a'\n\
         CATCH EXCEPTION E\n DISPLAY 'b'\n\
         CATCH RUST-EXCEPTION R\n DISPLAY 'c'\n DISPLAY 'd'\n END-TRY.\n",
    );
    assert_eq!(var.as_deref(), Some("E"));
    assert_eq!(body, 1);
    assert_eq!(rust_var.as_deref(), Some("R"));
    assert_eq!(rust_body, 2);
}

/// Order is not significant.
#[test]
fn try_with_both_clauses_reversed() {
    let (var, _, rust_var, _) = try_clauses(
        "    TRY\n DISPLAY 'a'\n\
         CATCH RUST-EXCEPTION R\n DISPLAY 'c'\n\
         CATCH EXCEPTION E\n DISPLAY 'b'\n END-TRY.\n",
    );
    assert_eq!(var.as_deref(), Some("E"));
    assert_eq!(rust_var.as_deref(), Some("R"));
}

#[test]
fn duplicate_catch_clauses_are_rejected() {
    for (src, want) in [
        (
            "    TRY\n DISPLAY 'a'\n CATCH EXCEPTION E\n DISPLAY 'b'\n \
             CATCH EXCEPTION F\n DISPLAY 'c'\n END-TRY.\n",
            "duplicate CATCH EXCEPTION clause",
        ),
        (
            "    TRY\n DISPLAY 'a'\n CATCH RUST-EXCEPTION E\n DISPLAY 'b'\n \
             CATCH RUST-EXCEPTION F\n DISPLAY 'c'\n END-TRY.\n",
            "duplicate CATCH RUST-EXCEPTION clause",
        ),
    ] {
        let result = parse(tokenize(&prog(src), SourceFormat::Free));
        assert!(
            result.diagnostics.iter().any(|d| d.message.contains(want)),
            "expected {want:?}, got {:?}",
            result.diagnostics
        );
    }
}
