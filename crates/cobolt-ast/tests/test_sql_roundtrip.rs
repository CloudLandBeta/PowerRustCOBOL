// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Spec 087 T5 — every embedded-SQL node survives bincode, the format a built
//! application carries its program in.

use cobolt_ast::sql::*;
use cobolt_ast::stmt::Stmt;
use cobolt_ast::Span;

fn name(n: &str) -> SqlHostName {
    SqlHostName { name: n.into(), quals: vec!["REC".into()], swapped: Some(("REC".into(), vec![n.into()])) }
}

fn host(n: &str) -> SqlHostRef {
    SqlHostRef { var: name(n), indicator: Some(name(&format!("{n}-IND"))), line: 7 }
}

fn text() -> SqlText {
    SqlText {
        parts: vec![SqlPart::Text("UPDATE T SET A = ".into()), SqlPart::Input(0), SqlPart::CurrentOf("C1".into())],
        inputs: vec![host("A")],
    }
}

#[test]
fn every_sql_kind_and_a_cursor_round_trip_through_bincode() {
    let kinds = vec![
        SqlKind::Declarative,
        SqlKind::Execute(text()),
        SqlKind::SelectInto { text: text(), into: vec![host("B"), host("C")] },
        SqlKind::Open { cursor: "C1".into(), using: SqlUsing::Hosts(vec![host("D")]) },
        SqlKind::Open { cursor: "C2".into(), using: SqlUsing::Descriptor(name("SQLDA")) },
        SqlKind::Open { cursor: "C3".into(), using: SqlUsing::None },
        SqlKind::Fetch { cursor: "C1".into(), into: SqlInto::Hosts(vec![host("E")]) },
        SqlKind::Fetch { cursor: "C2".into(), into: SqlInto::Descriptor(name("SQLDA")) },
        SqlKind::Close { cursor: "C1".into() },
        SqlKind::Commit,
        SqlKind::Rollback,
        SqlKind::Connect {
            target: SqlValue::Literal("SALES".into()),
            alias: Some("SECOND".into()),
            user: Some(SqlValue::Host(host("U"))),
            password: Some(SqlValue::Host(host("P"))),
        },
        SqlKind::SetConnection(SqlValue::Name("SECOND".into())),
        SqlKind::Disconnect(SqlDisconnect::Named(SqlValue::Name("SECOND".into()))),
        SqlKind::Disconnect(SqlDisconnect::Current),
        SqlKind::Disconnect(SqlDisconnect::All),
        SqlKind::Prepare { name: "S1".into(), from: SqlValue::Host(host("Q")) },
        SqlKind::ExecutePrepared { name: "S1".into(), using: SqlUsing::Hosts(vec![host("F")]) },
        SqlKind::ExecuteImmediate(SqlValue::Literal("CREATE TABLE X (A INT)".into())),
        SqlKind::Describe { name: "S1".into(), sqlda: name("SQLDA"), input: true },
    ];
    let n = kinds.len();
    let stmts: Vec<Stmt> = kinds
        .into_iter()
        .map(|kind| {
            Stmt::ExecSql(Box::new(ExecSql {
                kind,
                whenever: SqlWhenever { sqlerror: Some("ERR".into()), sqlwarning: None, not_found: Some("EOD".into()) },
                owner: "PROG".into(),
                span: Span::dummy(),
                last_line: 9,
            }))
        })
        .collect();
    let cursors = vec![
        SqlCursor {
            name: "C1".into(),
            owner: "PROG".into(),
            query: SqlCursorQuery::Static(text()),
            with_hold: true,
            for_update: true,
            positioned: true,
            in_data_division: true,
            line: 3,
        },
        SqlCursor {
            name: "C2".into(),
            owner: "PROG".into(),
            query: SqlCursorQuery::Prepared("S1".into()),
            with_hold: false,
            for_update: false,
            positioned: false,
            in_data_division: false,
            line: 4,
        },
    ];
    let bytes = bincode::serialize(&(stmts.clone(), cursors.clone())).expect("serialize");
    let back: (Vec<Stmt>, Vec<SqlCursor>) = bincode::deserialize(&bytes).expect("deserialize");
    assert_eq!(back.0, stmts);
    assert_eq!(back.1, cursors);
    println!("SQL AST round trip: {n} statement kinds and {} cursors, {} bytes", cursors.len(), bytes.len());
}
