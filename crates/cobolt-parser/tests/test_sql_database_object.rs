// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! `COBOL::SqlDatabase::<method>` (WASM-015 AC11): the program-level SQL
//! database. The parser reads the two-word receiver `COBOL::SqlDatabase` as the
//! one object `COBOL-SQLDATABASE`, so a call or a property on it is an ordinary
//! member of that object — in a statement, as a value, and in a condition — and
//! the built-in `COBOL::NAME(…)` CALL is untouched.

use cobolt_ast::expr::Expr;
use cobolt_ast::methods::SQL_DATABASE_OBJECT;
use cobolt_ast::program::ProcedureBody;
use cobolt_ast::stmt::Stmt;
use cobolt_lexer::{tokenize, SourceFormat};
use cobolt_parser::{parse, Severity};

struct Parsed {
    stmts: Vec<Stmt>,
    errors: Vec<String>,
}

fn parse_body(body: &str) -> Parsed {
    let src = format!(
        "IDENTIFICATION DIVISION.\nPROGRAM-ID. T.\nDATA DIVISION.\nWORKING-STORAGE SECTION.\n\
         01 WS-ROW PIC X(80).\n01 WS-N PIC 9(4).\n01 WS-SQL PIC X(80).\n01 WS-NAME PIC X(20).\n\
         PROCEDURE DIVISION.\nMAIN.\n{body}\n"
    );
    let result = parse(tokenize(&src, SourceFormat::Free));
    let errors = result
        .diagnostics
        .iter()
        .filter(|d| d.severity == Severity::Error)
        .map(|d| d.message.clone())
        .collect();
    let stmts = match result.program.expect("no program").procedure.body {
        ProcedureBody::Paragraphs(mut p) => p.pop().map(|p| p.stmts).unwrap_or_default(),
        ProcedureBody::Sections(s) => s
            .into_iter()
            .flat_map(|s| s.paragraphs)
            .flat_map(|p| p.stmts)
            .collect(),
    };
    // A period ends a sentence: it is a marker in the tree, not a statement of the program.
    let stmts = stmts
        .into_iter()
        .filter(|s| !matches!(s, Stmt::SentenceEnd { .. }))
        .collect();
    Parsed { stmts, errors }
}

/// The member call at the root of an expression: `(receiver name, member, argument count)`.
fn member_of(e: &Expr) -> Option<(String, String, usize)> {
    let Expr::Member {
        recv, member, args, ..
    } = e
    else {
        return None;
    };
    let Expr::Identifier(object, _) = recv.as_ref() else {
        return None;
    };
    Some((object.clone(), member.to_ascii_uppercase(), args.len()))
}

#[test]
fn a_statement_call_is_a_member_call_on_the_one_object() {
    let p = parse_body("COBOL::SqlDatabase::Open(\":memory:\").\nCOBOL::SqlDatabase::Execute(WS-SQL).\nCOBOL::SqlDatabase::Close().");
    assert!(p.errors.is_empty(), "{:?}", p.errors);
    let calls: Vec<_> = p
        .stmts
        .iter()
        .map(|s| match s {
            Stmt::InvokeExpr { expr, .. } => member_of(expr).expect("a member call"),
            other => panic!("not an inline call: {other:?}"),
        })
        .collect();
    assert_eq!(
        calls,
        vec![
            (SQL_DATABASE_OBJECT.to_string(), "OPEN".to_string(), 1),
            (SQL_DATABASE_OBJECT.to_string(), "EXECUTE".to_string(), 1),
            (SQL_DATABASE_OBJECT.to_string(), "CLOSE".to_string(), 0),
        ]
    );
}

#[test]
fn the_object_name_is_the_same_whatever_the_case_written() {
    let p = parse_body("COBOL::SQLDATABASE::Query(WS-SQL).\ncobol::sqldatabase::Query(WS-SQL).");
    assert!(p.errors.is_empty(), "{:?}", p.errors);
    assert_eq!(p.stmts.len(), 2);
    for s in &p.stmts {
        let Stmt::InvokeExpr { expr, .. } = s else {
            panic!("not an inline call: {s:?}")
        };
        assert_eq!(
            member_of(expr).expect("a member call").0,
            SQL_DATABASE_OBJECT
        );
    }
}

#[test]
fn a_call_is_a_value_in_set_move_and_display() {
    let p = parse_body(
        "SET WS-ROW TO COBOL::SqlDatabase::Fetch().\n\
         MOVE COBOL::SqlDatabase::ColumnCount() TO WS-N.\n\
         DISPLAY \"row: \" COBOL::SqlDatabase::Fetch().\n\
         MOVE 1 TO WS-N.",
    );
    assert!(p.errors.is_empty(), "{:?}", p.errors);
    assert_eq!(p.stmts.len(), 4, "{:?}", p.stmts);
    let text = format!("{:?}", p.stmts);
    assert_eq!(text.matches(SQL_DATABASE_OBJECT).count(), 3, "{text}");
    assert!(
        !text.to_ascii_uppercase().contains("\"SQLDATABASE\""),
        "the two-word receiver is left in the tree: {text}"
    );
}

#[test]
fn a_property_reads_in_a_condition_and_a_call_may_follow_a_move() {
    let p = parse_body(
        "MOVE 1 TO WS-N.\n\
         COBOL::SqlDatabase::Execute(WS-SQL).\n\
         IF COBOL::SqlDatabase::StatusCode NOT = 0\n   DISPLAY COBOL::SqlDatabase::LastError\nEND-IF.",
    );
    assert!(p.errors.is_empty(), "{:?}", p.errors);
    assert_eq!(p.stmts.len(), 3, "{:?}", p.stmts);
    assert!(
        matches!(p.stmts[1], Stmt::InvokeExpr { .. }),
        "{:?}",
        p.stmts[1]
    );
    assert!(matches!(p.stmts[2], Stmt::If { .. }), "{:?}", p.stmts[2]);
}

#[test]
fn a_chain_after_the_call_keeps_its_members() {
    let p = parse_body("SET WS-NAME TO COBOL::SqlDatabase::ColumnName(1)::Trim().");
    assert!(p.errors.is_empty(), "{:?}", p.errors);
    let text = format!("{:?}", p.stmts);
    assert!(
        text.contains("COLUMNNAME") && text.contains("TRIM") && text.contains(SQL_DATABASE_OBJECT),
        "{text}"
    );
}

#[test]
fn the_built_in_call_is_still_a_call() {
    let p = parse_body("COBOL::\"OPEN-DB\"(WS-SQL WS-N).\nCOBOL::Sleep(1).");
    assert!(p.errors.is_empty(), "{:?}", p.errors);
    assert_eq!(p.stmts.len(), 2);
    assert!(
        p.stmts.iter().all(|s| matches!(s, Stmt::Call { .. })),
        "{:?}",
        p.stmts
    );
}

#[test]
fn a_display_ends_before_a_call_on_the_next_line_and_keeps_a_value_on_its_own() {
    // The statement below the DISPLAY is a statement, not a second thing to display.
    let p = parse_body("DISPLAY \"a\"\nCOBOL::SqlDatabase::Execute(WS-SQL).");
    assert!(p.errors.is_empty(), "{:?}", p.errors);
    assert_eq!(p.stmts.len(), 2, "{:?}", p.stmts);
    assert!(
        matches!(&p.stmts[0], Stmt::Display { operands, .. } if operands.len() == 1),
        "{:?}",
        p.stmts[0]
    );
    assert!(
        matches!(p.stmts[1], Stmt::InvokeExpr { .. }),
        "{:?}",
        p.stmts[1]
    );
    // On the DISPLAY's own line, a call is a value; so is a property on a line of its own.
    let p = parse_body("DISPLAY \"a\" COBOL::SqlDatabase::Fetch() \"b\"\nDISPLAY \"e\"\n  COBOL::SqlDatabase::LastError.");
    assert!(p.errors.is_empty(), "{:?}", p.errors);
    assert_eq!(p.stmts.len(), 2, "{:?}", p.stmts);
    assert!(
        matches!(&p.stmts[0], Stmt::Display { operands, .. } if operands.len() == 3),
        "{:?}",
        p.stmts[0]
    );
    assert!(
        matches!(&p.stmts[1], Stmt::Display { operands, .. } if operands.len() == 2),
        "{:?}",
        p.stmts[1]
    );
}

#[test]
fn a_name_list_ends_before_the_call_on_the_next_line() {
    let p = parse_body("CLOSE F1\nCOBOL::SqlDatabase::Close().");
    assert_eq!(
        p.stmts
            .iter()
            .filter(|s| matches!(s, Stmt::InvokeExpr { .. }))
            .count(),
        1,
        "{:?}",
        p.stmts
    );
}
