// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Tests: construct and compare AST nodes.
//!
//! cobolt-ast is pure data — these tests verify that every node type can be
//! constructed, cloned, compared, and debug-printed without panicking.

use cobolt_ast::{
    Span,
    data::{DataDecl, OccursClause, PicClause, PicKind, Usage},
    expr::{ArithOp, CmpOp, Condition, Expr, FigurativeConstant, Literal, UnaryOp},
    program::{
        DataDivision, DataSection, IdentificationDivision, Paragraph, ProcedureDivision,
        ProcedureBody, Program,
    },
    stmt::{
        CallArg, EvalSubject, OpenMode, PerformTarget, Stmt, WhenClause, WhenValue,
    },
};

fn dummy_span() -> Span {
    Span::dummy()
}

// ── Literal ───────────────────────────────────────────────────────────────────

#[test]
fn literal_variants() {
    let l1 = Literal::String("hello".into());
    let l2 = Literal::Integer(42);
    let l3 = Literal::Float(3.14);
    let l4 = Literal::Figurative(FigurativeConstant::Zero);
    let l5 = Literal::Figurative(FigurativeConstant::Space);

    assert_eq!(l1.clone(), Literal::String("hello".into()));
    assert_eq!(l2.clone(), Literal::Integer(42));
    assert_eq!(l3.clone(), Literal::Float(3.14));
    assert_eq!(l4.clone(), Literal::Figurative(FigurativeConstant::Zero));
    assert_eq!(l5.clone(), Literal::Figurative(FigurativeConstant::Space));
}

// ── Expr ──────────────────────────────────────────────────────────────────────

#[test]
fn expr_identifier() {
    let e = Expr::Identifier("WS-COUNTER".into(), dummy_span());
    assert_eq!(e.clone(), e);
    let _ = format!("{e:?}");
}

#[test]
fn expr_literal() {
    let e = Expr::Literal(Literal::Integer(1), dummy_span());
    assert_eq!(e.span(), dummy_span());
}

#[test]
fn expr_arithmetic() {
    let lhs = Expr::Identifier("A".into(), dummy_span());
    let rhs = Expr::Literal(Literal::Integer(1), dummy_span());
    let e = Expr::Arithmetic {
        op: ArithOp::Add,
        lhs: Box::new(lhs),
        rhs: Box::new(rhs),
        span: dummy_span(),
    };
    assert_eq!(e.span(), dummy_span());
    let _ = format!("{e:?}");
}

#[test]
fn expr_unary() {
    let operand = Expr::Identifier("X".into(), dummy_span());
    let e = Expr::Unary {
        op: UnaryOp::Neg,
        operand: Box::new(operand),
        span: dummy_span(),
    };
    let _ = format!("{e:?}");
}

#[test]
fn expr_subscript() {
    let base = Expr::Identifier("TABLE-ITEM".into(), dummy_span());
    let idx  = Expr::Literal(Literal::Integer(1), dummy_span());
    let e = Expr::Subscript {
        base: Box::new(base),
        indices: vec![idx],
        span: dummy_span(),
    };
    let _ = format!("{e:?}");
}

#[test]
fn expr_function_call() {
    let arg = Expr::Identifier("WS-NAME".into(), dummy_span());
    let e = Expr::FunctionCall {
        name: "LENGTH".into(),
        args: vec![arg],
        span: dummy_span(),
    };
    let _ = format!("{e:?}");
}

// ── Condition ─────────────────────────────────────────────────────────────────

#[test]
fn condition_comparison() {
    let lhs = Expr::Identifier("WS-FLAG".into(), dummy_span());
    let rhs = Expr::Literal(Literal::Integer(1), dummy_span());
    let c = Condition::Comparison { lhs, op: CmpOp::Eq, rhs, span: dummy_span() };
    assert_eq!(c.span(), dummy_span());
}

#[test]
fn condition_and_or_not() {
    let lhs = Condition::ConditionName("WS-ON".into(), dummy_span());
    let rhs = Condition::ConditionName("WS-OFF".into(), dummy_span());
    let and = Condition::And(Box::new(lhs.clone()), Box::new(rhs.clone()), dummy_span());
    let or  = Condition::Or(Box::new(lhs.clone()), Box::new(rhs.clone()), dummy_span());
    let not = Condition::Not(Box::new(lhs.clone()), dummy_span());
    let _ = format!("{and:?} {or:?} {not:?}");
}

// ── DataDecl ──────────────────────────────────────────────────────────────────

#[test]
fn data_decl_elementary() {
    let d = DataDecl {
        level: 1,
        name: Some("WS-COUNTER".into()),
        picture: Some(PicClause {
            template: "9(5)".into(),
            kind: PicKind::Numeric,
            digits: 5,
            decimals: 0,
            span: dummy_span(),
        }),
        value: Some(Literal::Figurative(FigurativeConstant::Zero)),
        usage: Usage::Display,
        occurs: None,
        redefines: None,
        renames: None,
        condition_values: vec![],
        is_global: false,
        is_external: false,
        blank_when_zero: false,
        children: vec![],
        span: dummy_span(),
    };
    assert_eq!(d.level, 1);
    assert_eq!(d.name.as_deref(), Some("WS-COUNTER"));
    let _ = format!("{d:?}");
}

#[test]
fn data_decl_group() {
    let child = DataDecl {
        level: 5,
        name: Some("WS-NAME".into()),
        picture: Some(PicClause {
            template: "X(30)".into(),
            kind: PicKind::Alphanumeric,
            digits: 30,
            decimals: 0,
            span: dummy_span(),
        }),
        value: None,
        usage: Usage::Display,
        occurs: None,
        redefines: None,
        renames: None,
        condition_values: vec![],
        is_global: false,
        is_external: false,
        blank_when_zero: false,
        children: vec![],
        span: dummy_span(),
    };
    let parent = DataDecl {
        level: 1,
        name: Some("WS-RECORD".into()),
        picture: None,
        value: None,
        usage: Usage::Display,
        occurs: None,
        redefines: None,
        renames: None,
        condition_values: vec![],
        is_global: false,
        is_external: false,
        blank_when_zero: false,
        children: vec![child],
        span: dummy_span(),
    };
    assert_eq!(parent.children.len(), 1);
    assert_eq!(parent.children[0].name.as_deref(), Some("WS-NAME"));
}

#[test]
fn data_decl_table() {
    let d = DataDecl {
        level: 5,
        name: Some("TABLE-ITEM".into()),
        picture: Some(PicClause {
            template: "X(10)".into(),
            kind: PicKind::Alphanumeric,
            digits: 10,
            decimals: 0,
            span: dummy_span(),
        }),
        value: None,
        usage: Usage::Display,
        occurs: Some(OccursClause {
            min: 0,
            max: 100,
            depending_on: Some("WS-ITEM-COUNT".into()),
            indexed_by: vec!["WS-IDX".into()],
            span: dummy_span(),
        }),
        redefines: None,
        renames: None,
        condition_values: vec![],
        is_global: false,
        is_external: false,
        blank_when_zero: false,
        children: vec![],
        span: dummy_span(),
    };
    assert!(d.occurs.is_some());
}

#[test]
fn usage_variants_default() {
    assert_eq!(Usage::default(), Usage::Display);
}

// ── Stmt ──────────────────────────────────────────────────────────────────────

#[test]
fn stmt_move() {
    let s = Stmt::Move {
        from: Expr::Identifier("WS-A".into(), dummy_span()),
        to: vec![Expr::Identifier("WS-B".into(), dummy_span())],
        span: dummy_span(),
    };
    assert_eq!(s.span(), dummy_span());
    let _ = format!("{s:?}");
}

#[test]
fn stmt_add() {
    let s = Stmt::Add {
        operands: vec![Expr::Literal(Literal::Integer(1), dummy_span())],
        to: vec![(Expr::Identifier("WS-TOTAL".into(), dummy_span()), false)],
        giving: vec![],
        on_size_error: vec![],
        not_on_size_error: vec![],
        span: dummy_span(),
    };
    let _ = format!("{s:?}");
}

#[test]
fn stmt_compute() {
    let s = Stmt::Compute {
        targets: vec![(Expr::Identifier("WS-RESULT".into(), dummy_span()), false)],
        expr: Expr::Arithmetic {
            op: ArithOp::Mul,
            lhs: Box::new(Expr::Identifier("WS-A".into(), dummy_span())),
            rhs: Box::new(Expr::Identifier("WS-B".into(), dummy_span())),
            span: dummy_span(),
        },
        on_size_error: vec![],
        not_on_size_error: vec![],
        span: dummy_span(),
    };
    let _ = format!("{s:?}");
}

#[test]
fn stmt_if_else() {
    let cond = Condition::Comparison {
        lhs: Expr::Identifier("WS-FLAG".into(), dummy_span()),
        op: CmpOp::Eq,
        rhs: Expr::Literal(Literal::Integer(1), dummy_span()),
        span: dummy_span(),
    };
    let s = Stmt::If {
        condition: cond,
        then_stmts: vec![Stmt::Continue { span: dummy_span() }],
        else_stmts: vec![Stmt::GoBack { span: dummy_span() }],
        span: dummy_span(),
    };
    let _ = format!("{s:?}");
}

#[test]
fn stmt_evaluate() {
    let s = Stmt::Evaluate {
        subjects: vec![EvalSubject::Expr(Expr::Identifier("WS-CODE".into(), dummy_span()))],
        whens: vec![WhenClause {
            values: vec![WhenValue::Literal(Literal::Integer(1))],
            stmts: vec![Stmt::Continue { span: dummy_span() }],
            span: dummy_span(),
        }],
        other_stmts: vec![Stmt::Continue { span: dummy_span() }],
        span: dummy_span(),
    };
    let _ = format!("{s:?}");
}

#[test]
fn stmt_perform_varying() {
    let cond = Condition::Comparison {
        lhs: Expr::Identifier("WS-IDX".into(), dummy_span()),
        op: CmpOp::Gt,
        rhs: Expr::Literal(Literal::Integer(10), dummy_span()),
        span: dummy_span(),
    };
    let s = Stmt::Perform {
        target: PerformTarget::Varying {
            var: Expr::Identifier("WS-IDX".into(), dummy_span()),
            from: Expr::Literal(Literal::Integer(1), dummy_span()),
            by: Expr::Literal(Literal::Integer(1), dummy_span()),
            until: cond,
            stmts: vec![Stmt::Continue { span: dummy_span() }],
            after: vec![],
        },
        span: dummy_span(),
    };
    let _ = format!("{s:?}");
}

#[test]
fn stmt_call() {
    let s = Stmt::Call {
        program: Expr::Literal(Literal::String("MY-SUBPROG".into()), dummy_span()),
        using: vec![
            CallArg::ByReference(Expr::Identifier("WS-PARAM".into(), dummy_span())),
        ],
        returning: None,
        on_exception: vec![],
        not_on_exception: vec![],
        span: dummy_span(),
    };
    let _ = format!("{s:?}");
}

#[test]
fn stmt_open_close() {
    let open = Stmt::Open {
        mode: OpenMode::Input,
        files: vec!["MY-FILE".into()],
        sharing: None,
        lock: false,
        span: dummy_span(),
    };
    let close = Stmt::Close {
        files: vec!["MY-FILE".into()],
        span: dummy_span(),
    };
    let _ = format!("{open:?} {close:?}");
}

#[test]
fn stmt_stop_run() {
    let s = Stmt::Stop { run: true, literal: None, span: dummy_span() };
    assert_eq!(s.span(), dummy_span());
}

// ── Program ───────────────────────────────────────────────────────────────────

#[test]
fn program_construction() {
    let prog = Program {
        identification: IdentificationDivision {
            program_id: "HELLO".into(),
            author: Some("Cobolt test".into()),
            installation: None,
            date_written: None,
            date_compiled: None,
            security: None,
            span: dummy_span(),
        },
        environment: None,
        data: Some(DataDivision {
            sections: vec![DataSection::WorkingStorage(vec![DataDecl {
                level: 1,
                name: Some("WS-MSG".into()),
                picture: Some(PicClause {
                    template: "X(20)".into(),
                    kind: PicKind::Alphanumeric,
                    digits: 20,
                    decimals: 0,
                    span: dummy_span(),
                }),
                value: Some(Literal::String("Hello, World!".into())),
                usage: Usage::Display,
                occurs: None,
                redefines: None,
                renames: None,
                condition_values: vec![],
                is_global: false,
                is_external: false,
                blank_when_zero: false,
                children: vec![],
                span: dummy_span(),
            }])],
            span: dummy_span(),
        }),
        procedure: ProcedureDivision {
            using: vec![],
            returning: None,
            body: ProcedureBody::Paragraphs(vec![Paragraph {
                name: "MAIN-PROC".into(),
                stmts: vec![
                    Stmt::Display {
                        operands: vec![Expr::Identifier("WS-MSG".into(), dummy_span())],
                        upon: None,
                        no_advancing: false,
                        screen: None,
                        span: dummy_span(),
                    },
                    Stmt::Stop { run: true, literal: None, span: dummy_span() },
                ],
                span: dummy_span(),
            }]),
            span: dummy_span(),
        },
        nested_programs: vec![],
        end_program_name: None,
        decimal_comma: false,
        span: dummy_span(),
    };

    assert_eq!(prog.identification.program_id, "HELLO");
    assert!(prog.data.is_some());
    let _ = format!("{prog:?}");
    // Clone and compare
    assert_eq!(prog.clone(), prog);
}
