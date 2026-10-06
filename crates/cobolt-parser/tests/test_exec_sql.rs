// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Spec 087 T6/T7 — parsing `EXEC SQL` blocks.

use cobolt_ast::program::{DataSection, ProcedureBody, Program};
use cobolt_ast::sql::*;
use cobolt_ast::stmt::Stmt;
use cobolt_lexer::{tokenize, SourceFormat};
use cobolt_parser::{parse, ParseResult, Severity};

fn parse_src(src: &str) -> ParseResult {
    parse(tokenize(src, SourceFormat::Free))
}

fn program(ws: &str, proc_: &str) -> String {
    format!(
        "IDENTIFICATION DIVISION.\nPROGRAM-ID. ESQL.\nDATA DIVISION.\nWORKING-STORAGE SECTION.\n{ws}\nPROCEDURE DIVISION.\nMAIN-PARA.\n{proc_}\n    STOP RUN.\n"
    )
}

fn errors(r: &ParseResult) -> Vec<(u32, String)> {
    r.diagnostics
        .iter()
        .filter(|d| d.severity == Severity::Error)
        .map(|d| (d.span.line, d.message.clone()))
        .collect()
}

/// The program's statements, without the sentence-end markers.
fn stmts(p: &Program) -> Vec<Stmt> {
    let all: Vec<Stmt> = match &p.procedure.body {
        ProcedureBody::Paragraphs(ps) => ps.iter().flat_map(|p| p.stmts.clone()).collect(),
        ProcedureBody::Sections(ss) => ss.iter().flat_map(|s| s.paragraphs.iter()).flat_map(|p| p.stmts.clone()).collect(),
    };
    all.into_iter().filter(|s| !matches!(s, Stmt::SentenceEnd { .. })).collect()
}

fn sql_kinds(p: &Program) -> Vec<SqlKind> {
    let mut out = Vec::new();
    for s in stmts(p) {
        s.walk(&mut |st| {
            if let Stmt::ExecSql(e) = st {
                out.push(e.kind.clone());
            }
        });
    }
    out
}

fn ws_names(p: &Program) -> Vec<String> {
    let mut out = Vec::new();
    for s in &p.data.as_ref().unwrap().sections {
        if let DataSection::WorkingStorage(items) = s {
            for i in items {
                out.extend(i.name.clone());
            }
        }
    }
    out
}

fn text_of(t: &SqlText) -> String {
    t.parts
        .iter()
        .map(|p| match p {
            SqlPart::Text(s) => s.clone(),
            SqlPart::Input(n) => format!("${n}"),
            SqlPart::CurrentOf(c) => format!("<CURRENT OF {c}>"),
        })
        .collect()
}

/// Every statement form parses to its kind.
#[test]
fn every_statement_form_parses_to_its_kind() {
    let r = parse_src(&program(
        "01 WS-ID PIC 9(5).\n01 WS-NAME PIC X(30).\n01 WS-Q PIC X(200).\n01 WS-U PIC X(20).\n01 WS-P PIC X(20).",
        "    EXEC SQL SELECT NAME INTO :WS-NAME FROM CUST WHERE ID = :WS-ID END-EXEC
    EXEC SQL INSERT INTO CUST (ID, NAME) VALUES (:WS-ID, :WS-NAME) END-EXEC
    EXEC SQL UPDATE CUST SET NAME = :WS-NAME WHERE ID = :WS-ID END-EXEC
    EXEC SQL DELETE FROM CUST WHERE ID = :WS-ID END-EXEC
    EXEC SQL CREATE TABLE T (A INT) END-EXEC
    EXEC SQL DECLARE C1 CURSOR FOR SELECT ID, NAME FROM CUST END-EXEC
    EXEC SQL OPEN C1 END-EXEC
    EXEC SQL FETCH C1 INTO :WS-ID, :WS-NAME END-EXEC
    EXEC SQL CLOSE C1 END-EXEC
    EXEC SQL COMMIT WORK END-EXEC
    EXEC SQL ROLLBACK END-EXEC
    EXEC SQL CONNECT TO 'SALES' AS SECOND USER :WS-U USING :WS-P END-EXEC
    EXEC SQL SET CONNECTION SECOND END-EXEC
    EXEC SQL DISCONNECT ALL END-EXEC
    EXEC SQL PREPARE S1 FROM :WS-Q END-EXEC
    EXEC SQL EXECUTE S1 USING :WS-ID END-EXEC
    EXEC SQL EXECUTE IMMEDIATE :WS-Q END-EXEC
    EXEC SQL DESCRIBE INPUT S1 INTO SQLDA END-EXEC",
    ));
    assert_eq!(errors(&r), vec![]);
    let k = sql_kinds(r.program.as_ref().unwrap());
    let names: Vec<&str> = k
        .iter()
        .map(|k| match k {
            SqlKind::SelectInto { .. } => "select-into",
            SqlKind::Execute(_) => "execute",
            SqlKind::Declarative => "declarative",
            SqlKind::Open { .. } => "open",
            SqlKind::Fetch { .. } => "fetch",
            SqlKind::Close { .. } => "close",
            SqlKind::Commit => "commit",
            SqlKind::Rollback => "rollback",
            SqlKind::Connect { .. } => "connect",
            SqlKind::SetConnection(_) => "set-connection",
            SqlKind::Disconnect(_) => "disconnect",
            SqlKind::Prepare { .. } => "prepare",
            SqlKind::ExecutePrepared { .. } => "execute-prepared",
            SqlKind::ExecuteImmediate(_) => "execute-immediate",
            SqlKind::Describe { .. } => "describe",
        })
        .collect();
    assert_eq!(
        names,
        [
            "select-into", "execute", "execute", "execute", "execute", "declarative", "open", "fetch", "close",
            "commit", "rollback", "connect", "set-connection", "disconnect", "prepare", "execute-prepared",
            "execute-immediate", "describe"
        ]
    );
    // Inputs became parameters; nothing is spliced.
    match &k[1] {
        SqlKind::Execute(t) => {
            assert_eq!(text_of(t), "INSERT INTO CUST (ID, NAME) VALUES ($0, $1)");
            assert_eq!(t.inputs.iter().map(|h| h.var.name.clone()).collect::<Vec<_>>(), ["WS-ID", "WS-NAME"]);
        }
        other => panic!("{other:?}"),
    }
    match &k[11] {
        SqlKind::Connect { target, alias, user, password } => {
            assert_eq!(target, &SqlValue::Literal("SALES".into()));
            assert_eq!(alias.as_deref(), Some("SECOND"));
            assert!(matches!(user, Some(SqlValue::Host(h)) if h.var.name == "WS-U"));
            assert!(matches!(password, Some(SqlValue::Host(h)) if h.var.name == "WS-P"));
        }
        other => panic!("{other:?}"),
    }
    let cursors = &r.program.as_ref().unwrap().sql_cursors;
    assert_eq!(cursors.len(), 1);
    assert_eq!((cursors[0].name.as_str(), cursors[0].owner.as_str()), ("C1", "ESQL"));
}

/// `INTO` is cut out wherever it stands — after FROM and after WHERE, the
/// shapes the migrated samples use.
#[test]
fn into_is_cut_out_wherever_it_stands() {
    for (sql, expect) in [
        ("SELECT NAME INTO :WS-NAME FROM CUST WHERE ID = :WS-ID", "SELECT NAME FROM CUST WHERE ID = $0"),
        ("SELECT NAME FROM CUST INTO :WS-NAME WHERE ID = :WS-ID", "SELECT NAME FROM CUST WHERE ID = $0"),
        ("SELECT NAME FROM CUST WHERE ID = :WS-ID INTO :WS-NAME", "SELECT NAME FROM CUST WHERE ID = $0"),
    ] {
        let r = parse_src(&program("01 WS-ID PIC 9(5).\n01 WS-NAME PIC X(30).", &format!("    EXEC SQL {sql} END-EXEC")));
        assert_eq!(errors(&r), vec![], "{sql}");
        match &sql_kinds(r.program.as_ref().unwrap())[0] {
            SqlKind::SelectInto { text, into } => {
                assert_eq!(text_of(text).split_whitespace().collect::<Vec<_>>().join(" "), expect, "{sql}");
                assert_eq!(into.len(), 1);
                assert_eq!(into[0].var.name, "WS-NAME");
            }
            other => panic!("{sql}: {other:?}"),
        }
    }
}

/// A period after END-EXEC ends the sentence; a block in an IF without one
/// stays in the IF (AC1).
#[test]
fn a_period_ends_the_sentence_and_an_if_keeps_its_block() {
    let r = parse_src(&program(
        "01 A PIC 9.\n01 WS-N PIC 9.",
        "    IF A = 1
        EXEC SQL DELETE FROM T END-EXEC
        MOVE 2 TO A
    END-IF
    IF A = 1
        EXEC SQL DELETE FROM T END-EXEC.
    MOVE 3 TO A",
    ));
    assert_eq!(errors(&r), vec![]);
    let s = stmts(r.program.as_ref().unwrap());
    match &s[0] {
        Stmt::If { then_stmts, .. } => {
            assert!(matches!(then_stmts[0], Stmt::ExecSql(_)));
            assert!(matches!(then_stmts[1], Stmt::Move { .. }), "the block did not end the IF");
        }
        other => panic!("{other:?}"),
    }
    match &s[1] {
        Stmt::If { then_stmts, .. } => assert_eq!(then_stmts.len(), 1, "the period ended the IF"),
        other => panic!("{other:?}"),
    }
    assert!(matches!(s[2], Stmt::Move { .. }), "MOVE 3 follows the period, outside the IF");
}

/// Each R14 error is reported once, on its own line; a literal password too.
#[test]
fn r14_errors_and_a_literal_password_each_once_on_their_line() {
    let src = program(
        "01 TBL PIC X OCCURS 3.\n01 NM PIC X(9).",
        "    EXEC SQL
       UPDATE T SET A = :TBL(2)
       WHERE B = :NM(1:3)
       AND C = SPACES
       AND \"ZERO\" = 1
    END-EXEC
    EXEC SQL CONNECT TO 'S' USER :NM USING 'secret' END-EXEC",
    );
    let r = parse_src(&src);
    let e = errors(&r);
    let line_of = |needle: &str| src.lines().position(|l| l.contains(needle)).unwrap() as u32 + 1;
    assert_eq!(e.len(), 4, "{e:?}");
    assert_eq!(e[0].0, line_of(":TBL(2)"));
    assert!(e[0].1.contains(":TBL"), "{e:?}");
    assert_eq!(e[1].0, line_of(":NM(1:3)"));
    assert_eq!(e[2].0, line_of("C = SPACES"));
    assert!(e[2].1.contains("SPACES"));
    assert!(e[3].1.contains("literal password"), "{e:?}");
}

/// Items after a declare section are declared (they were silently lost), and
/// DECLARE TABLE changes nothing.
#[test]
fn items_after_a_declare_section_are_declared() {
    let r = parse_src(&program(
        "01 BEFORE-ITEM PIC X.
EXEC SQL BEGIN DECLARE SECTION END-EXEC.
01 HV-ID PIC 9(5).
01 HV-NAME PIC X(30).
EXEC SQL END DECLARE SECTION END-EXEC.
EXEC SQL DECLARE CUST TABLE (ID INT, NAME VARCHAR(30)) END-EXEC.
01 AFTER-ITEM PIC X.",
        "    CONTINUE",
    ));
    assert_eq!(errors(&r), vec![]);
    assert_eq!(ws_names(r.program.as_ref().unwrap()), ["BEFORE-ITEM", "HV-ID", "HV-NAME", "AFTER-ITEM"]);
}

/// A block in the FILE SECTION, and an executable statement in
/// WORKING-STORAGE, are one error each.
#[test]
fn each_wrong_placement_is_one_error() {
    let src = "IDENTIFICATION DIVISION.\nPROGRAM-ID. P.\nENVIRONMENT DIVISION.\nINPUT-OUTPUT SECTION.\nFILE-CONTROL.\n    SELECT F ASSIGN TO 'f.dat'.\nDATA DIVISION.\nFILE SECTION.\nFD F.\n01 F-REC PIC X(10).\nEXEC SQL DECLARE X TABLE (A INT) END-EXEC.\nWORKING-STORAGE SECTION.\n01 A PIC X.\nEXEC SQL DELETE FROM T END-EXEC.\n01 B PIC X.\nPROCEDURE DIVISION.\nM.\n    STOP RUN.\n";
    let r = parse_src(src);
    let e = errors(&r);
    assert_eq!(e.len(), 2, "{e:?}");
    assert_eq!(e[0].0, 11);
    assert!(e[0].1.contains("FILE SECTION"), "{e:?}");
    assert_eq!(e[1].0, 14);
    assert!(e[1].1.contains("PROCEDURE DIVISION"), "{e:?}");
    assert_eq!(ws_names(r.program.as_ref().unwrap()), ["A", "B"]);
}

/// WHENEVER applies to the statements after it in the SOURCE (AC6, parse
/// side), whatever order they run in.
#[test]
fn whenever_follows_source_order() {
    let r = parse_src(&program(
        "01 A PIC 9.",
        "    PERFORM LATER
    EXEC SQL FETCH C1 INTO :A END-EXEC
    STOP RUN.
LATER.
    EXEC SQL WHENEVER NOT FOUND GO TO END-OF-DATA END-EXEC
    EXEC SQL FETCH C1 INTO :A END-EXEC.
END-OF-DATA.
    CONTINUE.
MORE.",
    ));
    let p = r.program.unwrap();
    let mut seen = Vec::new();
    for s in stmts(&p) {
        s.walk(&mut |st| {
            if let Stmt::ExecSql(e) = st {
                if let SqlKind::Fetch { .. } = e.kind {
                    seen.push(e.whenever.not_found.clone());
                }
            }
        });
    }
    assert_eq!(seen, [None, Some("END-OF-DATA".to_string())]);
}

/// A cursor named by CURRENT OF is marked positioned; one declared in the
/// DATA DIVISION is known to the programs inside (R28), and carries its owner.
#[test]
fn positioned_cursors_are_marked_and_owned() {
    let src = "IDENTIFICATION DIVISION.\nPROGRAM-ID. OUTER.\nDATA DIVISION.\nWORKING-STORAGE SECTION.\n01 A PIC 9 GLOBAL.\nEXEC SQL DECLARE CW CURSOR WITH HOLD FOR SELECT A FROM T FOR UPDATE END-EXEC.\nPROCEDURE DIVISION.\nM.\n    EXEC SQL DECLARE CP CURSOR FOR SELECT A FROM T END-EXEC\n    STOP RUN.\nIDENTIFICATION DIVISION.\nPROGRAM-ID. INNER.\nPROCEDURE DIVISION.\nH.\n    EXEC SQL UPDATE T SET A = 1 WHERE CURRENT OF CW END-EXEC\n    EXIT PROGRAM.\nEND PROGRAM INNER.\nEND PROGRAM OUTER.\n";
    let r = parse_src(src);
    assert_eq!(errors(&r), vec![]);
    let p = r.program.unwrap();
    let cw = p.sql_cursors.iter().find(|c| c.name == "CW").unwrap();
    assert!(cw.positioned && cw.with_hold && cw.for_update && cw.in_data_division);
    assert_eq!(cw.owner, "OUTER");
    let cp = p.sql_cursors.iter().find(|c| c.name == "CP").unwrap();
    assert!(!cp.positioned && !cp.in_data_division);
    let inner = &p.nested_programs[0];
    match &sql_kinds(inner)[0] {
        SqlKind::Execute(t) => assert_eq!(text_of(t), "UPDATE T SET A = 1 WHERE <CURRENT OF CW>"),
        other => panic!("{other:?}"),
    }
}
