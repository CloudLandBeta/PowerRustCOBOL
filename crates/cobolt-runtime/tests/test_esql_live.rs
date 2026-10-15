// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Spec 087 gated suites — what needs something from outside the repository.
//!
//! **T38 (AC15, narrowed by Q14):** the operator's sample programs' embedded
//! SQL, run as written. The samples are read from `PRC_LEGACY_CBL_DIR` at test
//! time and never copied into the repository: their host-variable
//! declarations and every `EXEC SQL` block are lifted from the files, and a
//! test program runs each distinct statement against a SQLite copy of the
//! tables they use. The programs' non-SQL PowerCOBOL constructs are out of
//! scope and play no part. Without the variable the test reports `SKIPPED`.
//!
//! **T27/T28 (M7):** the AC2–AC8, AC11 and AC18 programs of `tests/cobol/esql/`
//! rerun against a live PostgreSQL (`PRC_TEST_PG_URL`) or MySQL
//! (`PRC_TEST_MYSQL_URL`) server: every `CONNECT TO` the program makes is
//! pointed at the server, the tables it creates are dropped first, and each
//! program must report `FAIL 000`. `SKIPPED` without the variable.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::sync::mpsc;

use cobolt_forms::connections::SqlConnection;
use cobolt_lexer::{expand_copybooks, tokenize, tokenize_expansion, SourceFormat};
use cobolt_parser::{parse, Severity};
use cobolt_runtime::esql::catalog::{SqlCatalog, Source};
use cobolt_runtime::Interpreter;

fn legacy_dir() -> Option<PathBuf> {
    match std::env::var_os("PRC_LEGACY_CBL_DIR") {
        Some(d) => Some(PathBuf::from(d)),
        None => {
            println!("SKIPPED: PRC_LEGACY_CBL_DIR is not set");
            None
        }
    }
}

/// A listing's code area as text: the sequence area and indicator column
/// dropped, comment lines and PowerCOBOL `#` marker lines left out. The files
/// are Latin-1; every byte becomes its character.
fn code_area(path: &Path) -> Vec<String> {
    let bytes = std::fs::read(path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    let text: String = bytes.iter().map(|&b| b as char).collect();
    text.lines()
        .filter(|l| !l.starts_with('#'))
        .filter(|l| !l.chars().nth(6).is_some_and(|c| c == '*' || c == '/'))
        .map(|l| l.chars().skip(7).collect::<String>().replace('\t', " "))
        .collect()
}

/// Every `EXEC SQL … END-EXEC` block's SQL text, in source order.
fn sql_blocks(lines: &[String]) -> Vec<String> {
    let text = lines.join("\n");
    let upper = text.to_ascii_uppercase();
    let mut out = Vec::new();
    let mut from = 0;
    while let Some(k) = upper[from..].find("EXEC SQL") {
        let body = from + k + "EXEC SQL".len();
        let (s, e) = cobolt_lexer::sql::block_end(&text[body..]).expect("every block is closed");
        out.push(text[body..body + s].trim().to_string());
        from = body + e;
    }
    out
}

/// The data entries between each `BEGIN DECLARE SECTION` and its `END`.
fn declare_sections(lines: &[String]) -> Vec<String> {
    let mut out = Vec::new();
    let mut inside = false;
    for l in lines {
        let u = l.to_ascii_uppercase();
        if u.contains("BEGIN DECLARE SECTION") || u.contains("BEGIN  DECLARE SECTION") {
            inside = true;
            continue;
        }
        if u.contains("END") && u.contains("DECLARE SECTION") {
            inside = false;
            continue;
        }
        let entry = l.split("*>").next().unwrap_or("").trim_end();
        if inside && !entry.trim().is_empty() && !u.contains("EXEC SQL") && u.trim() != "END-EXEC." {
            out.push(format!("       {}", entry.trim()));
        }
    }
    out
}

fn normalize(sql: &str) -> String {
    sql.split_whitespace().collect::<Vec<_>>().join(" ").to_ascii_uppercase()
}

/// The procedure-division blocks of a sample, normalized, with the one that
/// starts with `prefix` (followed by a space or the end) picked by `take`.
struct Blocks {
    all: Vec<String>,
    used: BTreeSet<String>,
}

impl Blocks {
    fn new(blocks: Vec<String>) -> Self {
        let all = blocks.into_iter().filter(|b| !normalize(b).contains("DECLARE SECTION")).collect();
        Self { all, used: BTreeSet::new() }
    }

    /// `EXEC SQL <the sample's block> END-EXEC`, the block found by the start
    /// of its normalized text.
    fn take(&mut self, prefix: &str) -> String {
        let want = normalize(prefix);
        let found: Vec<&String> = self
            .all
            .iter()
            .filter(|b| {
                let n = normalize(b);
                n.starts_with(&want) && n[want.len()..].chars().next().is_none_or(|c| !c.is_ascii_alphanumeric() && c != '_')
            })
            .collect();
        let first = found.first().unwrap_or_else(|| panic!("no block starts with {prefix}"));
        let n = normalize(first);
        assert!(found.iter().all(|b| normalize(b) == n), "'{prefix}' names more than one statement: {found:?}");
        self.used.insert(n);
        format!("           EXEC SQL\n{}\n           END-EXEC\n", indent(first))
    }

    /// Distinct statements of the sample not run by the program.
    fn unused(&self) -> Vec<String> {
        let all: BTreeSet<String> = self.all.iter().map(|b| normalize(b)).collect();
        all.difference(&self.used).cloned().collect()
    }

    fn distinct(&self) -> usize {
        self.all.iter().map(|b| normalize(b)).collect::<BTreeSet<_>>().len()
    }
}

fn indent(sql: &str) -> String {
    sql.lines().map(|l| format!("               {}", l.trim())).collect::<Vec<_>>().join("\n")
}

fn run(src: &str, catalog: Option<SqlCatalog>) -> Vec<String> {
    let result = parse(tokenize(src, SourceFormat::Free));
    let errors: Vec<_> = result.diagnostics.iter().filter(|d| d.severity == Severity::Error).collect();
    assert!(errors.is_empty(), "parse errors: {errors:?}\n{src}");
    let (_event_tx, event_rx) = mpsc::channel();
    let (state_tx, _state_rx) = mpsc::channel();
    let (display_tx, display_rx) = mpsc::channel();
    let mut interp = Interpreter::new_with_channels(result.program.unwrap(), event_rx, state_tx, display_tx);
    if let Some(c) = catalog {
        interp.set_sql_catalog(c);
    }
    interp.run().unwrap_or_else(|e| panic!("the program ends normally: {e}"));
    display_rx.try_iter().map(|l| l.trim_end().to_string()).collect()
}

/// F-ART-PURGA as a test program: the listing's host-variable declarations, the
/// tables it works on (`setup` creates and fills them), then every distinct
/// statement of the listing, as written, each followed by what it reported.
fn purga_program(b: &mut Blocks, decls: &str, setup: &str) -> String {
    let mut p = String::new();
    p.push_str("       IDENTIFICATION DIVISION.\n       PROGRAM-ID. AC15-PURGA.\n       DATA DIVISION.\n       WORKING-STORAGE SECTION.\n");
    p.push_str(decls);
    p.push_str("\n       01  WS-N PIC 9(9).\n       PROCEDURE DIVISION.\n       MAIN-PARA.\n");
    p.push_str(setup);
    // R22: the statement SQLite rejects reports and the program carries on.
    p.push_str(&b.take("DELETE FROM ARTICULOS"));
    p.push_str("           DISPLAY \"DELETE-LIMIT \" SQLSTATE\n");
    p.push_str(&b.take("COMMIT"));
    for (count, cursor) in [("SELECT COUNT(IMGCOD)", "CURSOR_IMG"), ("SELECT COUNT(IMGNUM)", "CURSOR_IMG1")] {
        p.push_str(&b.take(count));
        p.push_str(&format!("           MOVE CONTASQL1 TO WS-N\n           DISPLAY \"{count} \" SQLSTATE \" \" WS-N\n"));
        p.push_str(&b.take(&format!("DECLARE {cursor}")));
        p.push_str(&b.take(&format!("OPEN {cursor}")));
        p.push_str("           PERFORM UNTIL SQLSTATE NOT = \"00000\"\n");
        p.push_str(&b.take(&format!("FETCH {cursor}")));
        p.push_str(&format!("               IF SQLSTATE = \"00000\"\n                   DISPLAY \"{cursor} \" IMGRUT\n               END-IF\n           END-PERFORM\n           DISPLAY \"{cursor}-END \" SQLSTATE\n"));
        p.push_str(&b.take(&format!("CLOSE {cursor}")));
    }
    p.push_str(&b.take("DELETE FROM IMGART"));
    p.push_str("           DISPLAY \"DELETE-IMG \" SQLSTATE\n");
    // The prepared query, as the listing's STRING builds it for a LIKE search.
    p.push_str("           MOVE \"SELECT ARTCOD ,ARTDES ,ARTFAM ,ARTOBS1 FROM ARTICULOS WHERE ARTDES LIKE ('%A%') AND ARTELI = 'S' ORDER BY ARTDES ASC\" TO SQLSCRIPT\n");
    p.push_str(&b.take("DECLARE CURSOR_TABLA"));
    p.push_str(&b.take("PREPARE CONSULTA"));
    p.push_str("           DISPLAY \"PREPARE \" SQLSTATE\n");
    for cursor in ["CURSOR_TABLA", "CURSOR_TABLA1"] {
        if cursor == "CURSOR_TABLA1" {
            p.push_str(&b.take("SELECT COUNT(ARTCOD)"));
            p.push_str("           MOVE CONTASQL TO WS-N\n           DISPLAY \"COUNT-ART \" SQLSTATE \" \" WS-N\n");
            p.push_str(&b.take("DECLARE CURSOR_TABLA1"));
        }
        p.push_str(&b.take(&format!("OPEN {cursor}")));
        p.push_str("           PERFORM UNTIL SQLSTATE NOT = \"00000\"\n");
        p.push_str(&b.take(&format!("FETCH {cursor}")));
        p.push_str(&format!("               IF SQLSTATE = \"00000\"\n                   DISPLAY \"{cursor} \" ARTCOD \" \" ARTDES\n               END-IF\n           END-PERFORM\n"));
        p.push_str(&b.take(&format!("CLOSE {cursor}")));
    }
    p.push_str("           MOVE \"A1\" TO ART13\n");
    p.push_str(&b.take("DELETE FROM CODE13"));
    p.push_str("           DISPLAY \"DELETE-CODE13 \" SQLSTATE\n");
    p.push_str("           EXEC SQL SELECT COUNT(*) INTO :WS-N FROM ARTICULOS END-EXEC\n           DISPLAY \"ARTICULOS \" WS-N\n");
    p.push_str("           STOP RUN.\n");

    p
}

/// The SQLite tables F-ART-PURGA works on, with the rows its statements find.
const PURGA_SQLITE_SETUP: &str = "           EXEC SQL CONNECT TO ':memory:' END-EXEC
           EXEC SQL CREATE TABLE ARTICULOS (ARTCOD TEXT PRIMARY KEY, ARTDES TEXT, ARTFAM INTEGER, ARTOBS1 TEXT, ARTELI TEXT) END-EXEC
           EXEC SQL INSERT INTO ARTICULOS VALUES ('A1', 'ALPHA', 1, 'one', 'S'), ('A2', 'BETA', 2, 'two', 'S'), ('A3', 'GAMMA', 3, '', 'N') END-EXEC
           EXEC SQL CREATE TABLE IMGART (IMGNUM INTEGER, IMGCOD TEXT, IMGRUT TEXT, IMGDES TEXT) END-EXEC
           EXEC SQL INSERT INTO IMGART VALUES (1, 'A1', '/img/a1.png', 'front'), (2, 'A1', '/img/a1-back.png', 'back') END-EXEC
           EXEC SQL CREATE TABLE CODE13 (ART13 TEXT, COD13 TEXT) END-EXEC
           EXEC SQL INSERT INTO CODE13 VALUES ('A1', '7790000000011') END-EXEC
           EXEC SQL COMMIT END-EXEC
           MOVE \"A1\" TO ARTCOD
";

/// F-ART-PURGA on SQLite: stand-alone status items, `DELETE … LIMIT 1` (which
/// SQLite rejects), `SELECT COUNT … INTO`, cursors over a query and over a
/// prepared statement, `COMMIT` — every distinct statement of the listing, as
/// written. `dir` holds the sample (`M-ARTICULOS/Debug/F-ART-PURGA.cob`).
fn f_art_purga_on_sqlite(dir: &Path) {
    let lines = code_area(&dir.join("M-ARTICULOS/Debug/F-ART-PURGA.cob"));
    let mut b = Blocks::new(sql_blocks(&lines));
    let decls = declare_sections(&lines).join("\n");

    let p = purga_program(&mut b, &decls, PURGA_SQLITE_SETUP);
    let out = run(&p, None);
    println!("{}", out.join("\n"));
    let unused = b.unused();
    assert!(unused.is_empty(), "statements of the listing not run: {unused:?}");
    let line = |prefix: &str| out.iter().find(|l| l.starts_with(prefix)).cloned().unwrap_or_else(|| panic!("no {prefix} line:\n{}", out.join("\n")));
    let squash = |s: String| s.split_whitespace().collect::<Vec<_>>().join(" ");
    let delete_limit = line("DELETE-LIMIT ");
    assert!(delete_limit.starts_with("DELETE-LIMIT 42"), "SQLite rejects DELETE … LIMIT with a syntax error: {delete_limit}");
    assert_eq!(line("SELECT COUNT(IMGCOD) "), "SELECT COUNT(IMGCOD) 00000 000000002");
    assert_eq!(
        out.iter().filter(|l| l.starts_with("CURSOR_IMG ")).map(|l| squash(l.clone())).collect::<Vec<_>>(),
        ["CURSOR_IMG /img/a1.png", "CURSOR_IMG /img/a1-back.png"]
    );
    assert_eq!(line("CURSOR_IMG-END "), "CURSOR_IMG-END 02000");
    assert_eq!(line("DELETE-IMG "), "DELETE-IMG 00000");
    assert_eq!(line("PREPARE "), "PREPARE 00000");
    let rows = |c: &str| out.iter().filter(|l| l.starts_with(&format!("{c} "))).map(|l| squash(l.clone())).collect::<Vec<_>>();
    assert_eq!(rows("CURSOR_TABLA"), ["CURSOR_TABLA A1 ALPHA", "CURSOR_TABLA A2 BETA"], "the prepared query");
    assert_eq!(line("COUNT-ART "), "COUNT-ART 00000 000000002");
    assert_eq!(rows("CURSOR_TABLA1"), ["CURSOR_TABLA1 A1 ALPHA", "CURSOR_TABLA1 A2 BETA"]);
    assert_eq!(line("DELETE-CODE13 "), "DELETE-CODE13 00000");
    assert_eq!(line("ARTICULOS "), "ARTICULOS 000000003", "the rejected DELETE removed nothing");
    println!(
        "AC15 F-ART-PURGA: {} distinct statements of the listing, all run; DELETE … LIMIT 1 → {} and the program went on; \
         2 cursors over queries, 1 over a prepared statement, 3 counts, 2 deletes, COMMIT",
        b.distinct(),
        &delete_limit["DELETE-LIMIT ".len()..]
    );
}

/// F-ART-PURGA on MySQL, with the tables created there. `DELETE … LIMIT 1` is MySQL's own
/// syntax, so the statement SQLite refuses is accepted: what the listing's other statements
/// then find depends on which row it deleted, so what is asserted is what holds either way —
/// every statement ran, every cursor ended with no data, and the counts agree with the rows.
fn f_art_purga_on_mysql(dir: &Path, url: &str) {
    let lines = code_area(&dir.join("M-ARTICULOS/Debug/F-ART-PURGA.cob"));
    let mut b = Blocks::new(sql_blocks(&lines));
    let decls = declare_sections(&lines).join("\n");
    let setup = format!(
        "           EXEC SQL CONNECT TO '{url}' END-EXEC
           EXEC SQL DROP TABLE IF EXISTS ARTICULOS END-EXEC
           EXEC SQL DROP TABLE IF EXISTS IMGART END-EXEC
           EXEC SQL DROP TABLE IF EXISTS CODE13 END-EXEC
           EXEC SQL CREATE TABLE ARTICULOS (ARTCOD VARCHAR(20) PRIMARY KEY, ARTDES VARCHAR(60), ARTFAM INTEGER, ARTOBS1 VARCHAR(200), ARTELI CHAR(1)) END-EXEC
           EXEC SQL INSERT INTO ARTICULOS VALUES ('A1', 'ALPHA', 1, 'one', 'S'), ('A2', 'BETA', 2, 'two', 'S'), ('A3', 'GAMMA', 3, '', 'N') END-EXEC
           EXEC SQL CREATE TABLE IMGART (IMGNUM INTEGER, IMGCOD VARCHAR(20), IMGRUT VARCHAR(200), IMGDES VARCHAR(60)) END-EXEC
           EXEC SQL INSERT INTO IMGART VALUES (1, 'A1', '/img/a1.png', 'front'), (2, 'A1', '/img/a1-back.png', 'back') END-EXEC
           EXEC SQL CREATE TABLE CODE13 (ART13 VARCHAR(20), COD13 VARCHAR(20)) END-EXEC
           EXEC SQL INSERT INTO CODE13 VALUES ('A1', '7790000000011') END-EXEC
           EXEC SQL COMMIT END-EXEC
           MOVE \"A1\" TO ARTCOD
"
    );
    let p = purga_program(&mut b, &decls, &setup);
    let out = run(&p, None);
    println!("{}", out.join("\n"));
    let unused = b.unused();
    assert!(unused.is_empty(), "statements of the listing not run: {unused:?}");
    let line = |prefix: &str| out.iter().find(|l| l.starts_with(prefix)).cloned().unwrap_or_else(|| panic!("no {prefix} line:\n{}", out.join("\n")));
    let squash = |s: String| s.split_whitespace().collect::<Vec<_>>().join(" ");
    let delete_limit = line("DELETE-LIMIT ");
    let deleted = match delete_limit.as_str() {
        "DELETE-LIMIT 00000" => 1,
        "DELETE-LIMIT 02000" => 0,
        other => panic!("MySQL accepts DELETE … LIMIT: {other}"),
    };
    assert!(line("SELECT COUNT(IMGCOD) ").starts_with("SELECT COUNT(IMGCOD) 00000 "));
    assert_eq!(line("CURSOR_IMG-END "), "CURSOR_IMG-END 02000");
    assert_eq!(line("PREPARE "), "PREPARE 00000");
    let rows = |c: &str| out.iter().filter(|l| l.starts_with(&format!("{c} "))).map(|l| squash(l.clone())).collect::<Vec<_>>();
    let counted: usize = squash(line("COUNT-ART ")).rsplit(' ').next().unwrap().parse().unwrap();
    assert_eq!(rows("CURSOR_TABLA").len(), counted, "the cursor over the prepared query reads as many rows as the count says");
    assert_eq!(rows("CURSOR_TABLA1").len(), counted);
    assert_eq!(line("ARTICULOS "), format!("ARTICULOS {:09}", 3 - deleted), "the DELETE removed {deleted} row(s)");
    println!(
        "AC15 F-ART-PURGA on MySQL: {} distinct statements of the listing, all run; DELETE … LIMIT 1 → {} ({deleted} row deleted); \
         the prepared query read {counted} row(s), as counted",
        b.distinct(),
        &delete_limit["DELETE-LIMIT ".len()..]
    );
}

#[test]
fn f_art_purga_sql_runs_on_sqlite() {
    let Some(dir) = legacy_dir() else { return };
    f_art_purga_on_sqlite(&dir);
}

/// AC15's MySQL half: the operator's listing, against a MySQL server.
#[test]
#[ignore = "needs the samples (PRC_LEGACY_CBL_DIR) and a MySQL server (PRC_TEST_MYSQL_URL)"]
fn f_art_purga_sql_runs_on_mysql() {
    let Some(dir) = legacy_dir() else { return };
    let Some(url) = std::env::var("PRC_TEST_MYSQL_URL").ok().filter(|u| !u.trim().is_empty()) else {
        println!("SKIPPED: PRC_TEST_MYSQL_URL is not set");
        return;
    };
    f_art_purga_on_mysql(&dir, &url);
}

/// TyC: `CONNECT TO 'JOSBER'` — a named SQL connection, resolved through the
/// project's catalog (R34) — a `SELECT … INTO … LIMIT 1` with `INTO` after
/// `WHERE`, and `DISCONNECT 'JOSBER'`. `dir` holds the sample (`TyC/Debug/TyC.cob`).
fn tyc_on_sqlite(dir: &Path) {
    let lines = code_area(&dir.join("TyC/Debug/TyC.cob"));
    let mut b = Blocks::new(sql_blocks(&lines));
    let decls = declare_sections(&lines).join("\n");

    let db_dir = std::env::temp_dir().join(format!("prc087-tyc-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&db_dir);
    std::fs::create_dir_all(&db_dir).unwrap();
    let mut josber = SqlConnection::new("JOSBER");
    josber.path = "josber.db".into();
    josber.create_if_missing = true;
    let catalog = || SqlCatalog::new("TyC", vec![josber.clone()], db_dir.clone(), Source::Injected);
    run(
        "IDENTIFICATION DIVISION.\nPROGRAM-ID. SEED.\nPROCEDURE DIVISION.\nMAIN-PARA.\n\
             EXEC SQL CONNECT TO 'JOSBER' END-EXEC\n\
             EXEC SQL CREATE TABLE REGISTRO (CERO INTEGER, UNO TEXT, DOS TEXT) END-EXEC\n\
             EXEC SQL INSERT INTO REGISTRO VALUES (7, 'x', 'SIETE'), (8, 'y', 'OCHO') END-EXEC\n    STOP RUN.\n",
        Some(catalog()),
    );
    let out = run(&tyc_program(&mut b, &decls), Some(catalog()));
    println!("{}", out.join("\n"));
    assert!(b.unused().is_empty(), "statements of the listing not run: {:?}", b.unused());
    assert_eq!(out, ["CONNECT 00000", "SELECT 00000 OCHO", "DISCONNECT 00000"]);
    let _ = std::fs::remove_dir_all(&db_dir);
    println!("AC15 TyC: {} distinct statements, all run; CONNECT TO 'JOSBER' reached the named SQL connection; SELECT … INTO … LIMIT 1 → OCHO", b.distinct());
}

/// The listing's statements as a test program: connect, read the one row, disconnect.
fn tyc_program(b: &mut Blocks, decls: &str) -> String {
    let mut p = String::new();
    p.push_str("       IDENTIFICATION DIVISION.\n       PROGRAM-ID. AC15-TYC.\n       DATA DIVISION.\n       WORKING-STORAGE SECTION.\n");
    p.push_str(decls);
    p.push_str("\n       PROCEDURE DIVISION.\n       MAIN-PARA.\n");
    p.push_str(&b.take("CONNECT TO 'JOSBER'"));
    p.push_str("           DISPLAY \"CONNECT \" SQLSTATE\n           MOVE 8 TO CERO\n");
    p.push_str(&b.take("SELECT CERO"));
    p.push_str("           DISPLAY \"SELECT \" SQLSTATE \" \" DOS\n");
    p.push_str(&b.take("DISCONNECT 'JOSBER'"));
    p.push_str("           DISPLAY \"DISCONNECT \" SQLSTATE\n           STOP RUN.\n");
    p
}

/// `scheme://user:password@host:port/database` in its parts.
fn server_parts(url: &str) -> (String, String, String, Option<u16>, String) {
    let rest = url.split_once("://").map_or(url, |(_, r)| r);
    let (cred, tail) = rest.rsplit_once('@').unwrap_or(("", rest));
    let (user, password) = cred.split_once(':').unwrap_or((cred, ""));
    let (hostport, database) = tail.split_once('/').unwrap_or((tail, ""));
    let (host, port) = match hostport.rsplit_once(':') {
        Some((h, p)) => (h, p.parse().ok()),
        None => (hostport, None),
    };
    (user.into(), password.into(), host.into(), port, database.split('?').next().unwrap_or("").into())
}

/// TyC on MySQL: `JOSBER` is a MySQL server in the project's SQL connections, its password
/// supplied the way an application's is — through the environment.
fn tyc_on_mysql(dir: &Path, url: &str) {
    let lines = code_area(&dir.join("TyC/Debug/TyC.cob"));
    let mut b = Blocks::new(sql_blocks(&lines));
    let decls = declare_sections(&lines).join("\n");
    let (user, password, host, port, database) = server_parts(url);
    let mut josber = SqlConnection::new("JOSBER");
    josber.backend = "mysql".into();
    josber.host = host;
    josber.port = port;
    josber.database = database;
    josber.user = user;
    let var = cobolt_forms::connections::sql_env_var("TyC", "JOSBER", "PASSWORD");
    let catalog = || {
        let mut c = SqlCatalog::new("TyC", vec![josber.clone()], std::env::temp_dir(), Source::Injected);
        c.injected_env.push((var.clone(), cobolt_runtime::esql::catalog::Secret::new(password.clone())));
        c
    };
    run(
        "IDENTIFICATION DIVISION.\nPROGRAM-ID. SEED.\nPROCEDURE DIVISION.\nMAIN-PARA.\n\
             EXEC SQL CONNECT TO 'JOSBER' END-EXEC\n\
             EXEC SQL DROP TABLE IF EXISTS REGISTRO END-EXEC\n\
             EXEC SQL CREATE TABLE REGISTRO (CERO INTEGER, UNO VARCHAR(10), DOS VARCHAR(10)) END-EXEC\n\
             EXEC SQL INSERT INTO REGISTRO VALUES (7, 'x', 'SIETE'), (8, 'y', 'OCHO') END-EXEC\n\
             EXEC SQL COMMIT END-EXEC\n    STOP RUN.\n",
        Some(catalog()),
    );
    let out = run(&tyc_program(&mut b, &decls), Some(catalog()));
    println!("{}", out.join("\n"));
    assert!(b.unused().is_empty(), "statements of the listing not run: {:?}", b.unused());
    assert_eq!(out, ["CONNECT 00000", "SELECT 00000 OCHO", "DISCONNECT 00000"]);
    println!("AC15 TyC on MySQL: {} distinct statements, all run; CONNECT TO 'JOSBER' reached a MySQL server through the named SQL connection; SELECT … INTO … LIMIT 1 → OCHO", b.distinct());
}

#[test]
fn tyc_sql_runs_on_sqlite() {
    let Some(dir) = legacy_dir() else { return };
    tyc_on_sqlite(&dir);
}

/// AC15's MySQL half: the operator's listing, against a MySQL server.
#[test]
#[ignore = "needs the samples (PRC_LEGACY_CBL_DIR) and a MySQL server (PRC_TEST_MYSQL_URL)"]
fn tyc_sql_runs_on_mysql() {
    let Some(dir) = legacy_dir() else { return };
    let Some(url) = std::env::var("PRC_TEST_MYSQL_URL").ok().filter(|u| !u.trim().is_empty()) else {
        println!("SKIPPED: PRC_TEST_MYSQL_URL is not set");
        return;
    };
    tyc_on_mysql(&dir, &url);
}

// ── Stand-ins for the samples, so the machinery above is held to account without them ──
//
// The operator's PowerCOBOL listings are a client's code and stay outside the repository.
// What the tests above do with them — lift the declarations and the `EXEC SQL` blocks, build a
// program, run it — is exercised here on two small listings of our own that have the same shape:
// the same kinds of statement, in the same fixed-format layout. They prove the harness, not
// AC15: AC15 is the real listings.

/// A fixed-format listing: six columns of sequence area and an indicator blank, then the code.
fn listing(code: &[&str]) -> String {
    code.iter().map(|l| format!("       {l}\n")).collect()
}

fn write_stand_ins() -> PathBuf {
    let dir = std::env::temp_dir().join(format!("prc087-standin-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join("M-ARTICULOS/Debug")).unwrap();
    std::fs::create_dir_all(dir.join("TyC/Debug")).unwrap();
    std::fs::write(
        dir.join("M-ARTICULOS/Debug/F-ART-PURGA.cob"),
        listing(&[
            "EXEC SQL BEGIN DECLARE SECTION END-EXEC.",
            "01  ARTCOD    PIC X(20).",
            "01  ARTDES    PIC X(60).",
            "01  ARTFAM    PIC 9(4).",
            "01  ARTOBS1   PIC X(60).",
            "01  IMGRUT    PIC X(60).",
            "01  ART13     PIC X(20).",
            "01  SQLSCRIPT PIC X(500).",
            "01  CONTASQL  PIC S9(9) COMP-5.",
            "01  CONTASQL1 PIC S9(9) COMP-5.",
            "01  SQLSTATE  PIC X(5).",
            "EXEC SQL END DECLARE SECTION END-EXEC.",
            "EXEC SQL DELETE FROM ARTICULOS WHERE ARTCOD = :ARTCOD LIMIT 1 END-EXEC.",
            "EXEC SQL COMMIT END-EXEC.",
            "EXEC SQL SELECT COUNT(IMGCOD) INTO :CONTASQL1 FROM IMGART WHERE IMGCOD = :ARTCOD END-EXEC.",
            "EXEC SQL DECLARE CURSOR_IMG CURSOR FOR SELECT IMGRUT FROM IMGART WHERE IMGCOD = :ARTCOD ORDER BY IMGNUM END-EXEC.",
            "EXEC SQL OPEN CURSOR_IMG END-EXEC.",
            "EXEC SQL FETCH CURSOR_IMG INTO :IMGRUT END-EXEC.",
            "EXEC SQL CLOSE CURSOR_IMG END-EXEC.",
            "EXEC SQL SELECT COUNT(IMGNUM) INTO :CONTASQL1 FROM IMGART WHERE IMGCOD = :ARTCOD END-EXEC.",
            "EXEC SQL DECLARE CURSOR_IMG1 CURSOR FOR SELECT IMGRUT FROM IMGART WHERE IMGCOD = :ARTCOD ORDER BY IMGNUM END-EXEC.",
            "EXEC SQL OPEN CURSOR_IMG1 END-EXEC.",
            "EXEC SQL FETCH CURSOR_IMG1 INTO :IMGRUT END-EXEC.",
            "EXEC SQL CLOSE CURSOR_IMG1 END-EXEC.",
            "EXEC SQL DELETE FROM IMGART WHERE IMGCOD = :ARTCOD END-EXEC.",
            "EXEC SQL DECLARE CURSOR_TABLA CURSOR FOR CONSULTA END-EXEC.",
            "EXEC SQL PREPARE CONSULTA FROM :SQLSCRIPT END-EXEC.",
            "EXEC SQL OPEN CURSOR_TABLA END-EXEC.",
            "EXEC SQL FETCH CURSOR_TABLA INTO :ARTCOD, :ARTDES, :ARTFAM, :ARTOBS1 END-EXEC.",
            "EXEC SQL CLOSE CURSOR_TABLA END-EXEC.",
            "EXEC SQL SELECT COUNT(ARTCOD) INTO :CONTASQL FROM ARTICULOS WHERE ARTDES LIKE ('%A%') AND ARTELI = 'S' END-EXEC.",
            "EXEC SQL DECLARE CURSOR_TABLA1 CURSOR FOR CONSULTA END-EXEC.",
            "EXEC SQL OPEN CURSOR_TABLA1 END-EXEC.",
            "EXEC SQL FETCH CURSOR_TABLA1 INTO :ARTCOD, :ARTDES, :ARTFAM, :ARTOBS1 END-EXEC.",
            "EXEC SQL CLOSE CURSOR_TABLA1 END-EXEC.",
            "EXEC SQL DELETE FROM CODE13 WHERE ART13 = :ART13 END-EXEC.",
        ]),
    )
    .unwrap();
    std::fs::write(
        dir.join("TyC/Debug/TyC.cob"),
        listing(&[
            "EXEC SQL BEGIN DECLARE SECTION END-EXEC.",
            "01  CERO      PIC 9(4).",
            "01  UNO       PIC X(10).",
            "01  DOS       PIC X(10).",
            "01  SQLSTATE  PIC X(5).",
            "EXEC SQL END DECLARE SECTION END-EXEC.",
            "EXEC SQL CONNECT TO 'JOSBER' END-EXEC.",
            "EXEC SQL SELECT CERO, UNO, DOS FROM REGISTRO WHERE CERO = :CERO INTO :CERO, :UNO, :DOS LIMIT 1 END-EXEC.",
            "EXEC SQL DISCONNECT 'JOSBER' END-EXEC.",
        ]),
    )
    .unwrap();
    dir
}

/// The harness, on SQLite, with the stand-in listings: the same assertions the real ones get.
#[test]
fn the_samples_harness_runs_on_stand_in_listings_on_sqlite() {
    let dir = write_stand_ins();
    f_art_purga_on_sqlite(&dir);
    tyc_on_sqlite(&dir);
    let _ = std::fs::remove_dir_all(&dir);
}

/// …and on MySQL.
#[test]
#[ignore = "needs a MySQL server: PRC_TEST_MYSQL_URL"]
fn the_samples_harness_runs_on_stand_in_listings_on_mysql() {
    let Some(url) = std::env::var("PRC_TEST_MYSQL_URL").ok().filter(|u| !u.trim().is_empty()) else {
        println!("SKIPPED: PRC_TEST_MYSQL_URL is not set");
        return;
    };
    let dir = write_stand_ins();
    f_art_purga_on_mysql(&dir, &url);
    tyc_on_mysql(&dir, &url);
    let _ = std::fs::remove_dir_all(&dir);
}

/// The AC programs that run on any database.
const AC_PROGRAMS: [&str; 11] = [
    "ac2_host_structure.cbl",
    "ac3_null_truncation.cbl",
    "ac4_round_trips.cbl",
    "ac5_status_standalone.cbl",
    "ac5_status_sqlca.cbl",
    "ac6_whenever.cbl",
    "ac7_cursors.cbl",
    "ac7b_cursor_batches.cbl",
    "ac8_units_of_work.cbl",
    "ac11_dynamic.cbl",
    "ac18_mixed.cbl",
];

/// Rerun every AC program against the server at `url`.
fn rerun_on_server(var: &str) {
    let Some(url) = std::env::var(var).ok().filter(|u| !u.trim().is_empty()) else {
        println!("SKIPPED: {var} is not set");
        return;
    };
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/cobol/esql");
    let connect = regex_free_connects();
    // AC18 holds two connections to two databases: the second is the first's name
    // with a 2 on the end, created here when the server does not have it yet.
    let url2 = second_database_url(&url);
    {
        let mut first = cobolt_runtime::esql::backend::open(&url).unwrap_or_else(|e| panic!("{var}: {}", e.message));
        let db2 = url2.rsplit('/').next().unwrap_or_default().split('?').next().unwrap_or_default().to_string();
        let _ = first.execute(&format!("CREATE DATABASE {db2}"), &[]);
    }
    // The INDEXED files the programs write (AC8) go to a folder of their own.
    let work = std::env::temp_dir().join(format!("prc-esql-live-{}", std::process::id()));
    std::fs::create_dir_all(&work).unwrap();
    let mut summary = Vec::new();
    let mut failures = Vec::new();
    for name in AC_PROGRAMS {
        let original = std::fs::read_to_string(dir.join(name)).unwrap();
        let mut src = original.replace("@DIR@", &work.to_string_lossy());
        for target in &connect {
            let to = if target.ends_with("second.db'") { &url2 } else { &url };
            src = src.replace(&target.replace("@DIR@", &work.to_string_lossy()), &format!("CONNECT TO '{to}'"));
        }
        // A server keeps its tables between programs: drop what this one creates,
        // in both databases.
        for u in [&url, &url2] {
            let mut backend = cobolt_runtime::esql::backend::open(u).unwrap_or_else(|e| panic!("{var}: {}", e.message));
            for table in created_tables(&original) {
                let _ = backend.execute(&format!("DROP TABLE IF EXISTS {table}"), &[]);
            }
        }
        let exp = expand_copybooks(&src, &dir, SourceFormat::Free);
        assert!(exp.errors.is_empty(), "{name}: {:?}", exp.errors);
        let result = parse(tokenize_expansion(&exp));
        let program = result.program.unwrap_or_else(|| panic!("{name} parses"));
        let (_event_tx, event_rx) = mpsc::channel();
        let (state_tx, _state_rx) = mpsc::channel();
        let (display_tx, display_rx) = mpsc::channel();
        let mut interp = Interpreter::new_with_channels(program, event_rx, state_tx, display_tx);
        interp.run().unwrap_or_else(|e| panic!("{name}: {e}"));
        let out: Vec<String> = display_rx.try_iter().map(|l| l.trim_end().to_string()).collect();
        println!("--- {name} ---\n{}", out.join("\n"));
        let verdict = verdict_of(name, &out);
        summary.push(format!("{name}: {verdict}"));
        if !(verdict.ends_with("FAIL 000") || verdict == STATES_OK) {
            failures.push(format!("{name} on {var}: {verdict:?}\n{}", out.join("\n")));
        }
    }
    println!("{var}: {}", summary.join("; "));
    assert!(failures.is_empty(), "{} of {} programs failed:\n{}", failures.len(), AC_PROGRAMS.len(), failures.join("\n\n"));
}

/// The verdict of the AC5 programs, which print a line per statement rather than a tally.
const STATES_OK: &str = "STATES OK";

/// The five outcomes AC5 provokes, as SQLSTATEs: success, no data, truncation,
/// a duplicate key, a syntax error. A server words its messages its own way; the
/// standard states are what R18 promises.
const AC5_STATES: [&str; 5] = ["00000", "02000", "01004", "23505", "42601"];

/// What a program says of itself: its `PASS n FAIL m` tally, or — for the AC5
/// programs — whether their STATUS lines carry the standard states.
fn verdict_of(name: &str, out: &[String]) -> String {
    if name.starts_with("ac5_status") {
        let states: Vec<&str> = out
            .iter()
            .filter(|l| l.starts_with("STATUS "))
            .filter_map(|l| l.split_whitespace().nth(1))
            .collect();
        return if states == AC5_STATES { STATES_OK.into() } else { format!("STATES {states:?}") };
    }
    out.iter().rev().find(|l| l.starts_with("PASS ")).cloned().unwrap_or_default()
}

/// `url` with the name of its database followed by a 2: the server's second database.
fn second_database_url(url: &str) -> String {
    let (head, query) = match url.split_once('?') {
        Some((h, q)) => (h, format!("?{q}")),
        None => (url, String::new()),
    };
    format!("{head}2{query}")
}

/// The connection targets the AC programs name, as they are written.
fn regex_free_connects() -> Vec<String> {
    vec![
        "CONNECT TO ':memory:'".into(),
        "CONNECT TO 'sqlite:@DIR@/first.db'".into(),
        "CONNECT TO 'sqlite:@DIR@/second.db'".into(),
    ]
}

/// The tables a program creates (`CREATE TABLE name`).
fn created_tables(src: &str) -> Vec<String> {
    let upper = src.to_ascii_uppercase();
    let mut out = Vec::new();
    let mut from = 0;
    while let Some(k) = upper[from..].find("CREATE TABLE ") {
        let s = from + k + "CREATE TABLE ".len();
        let name: String = upper[s..].trim_start().chars().take_while(|c| c.is_ascii_alphanumeric() || *c == '_').collect();
        if !name.is_empty() && name != "IF" && !out.contains(&name) {
            out.push(name);
        }
        from = s;
    }
    out
}

/// T27 — the AC programs on PostgreSQL.
#[test]
#[ignore = "needs a PostgreSQL server: PRC_TEST_PG_URL"]
fn pg_runs_the_ac_programs() {
    rerun_on_server("PRC_TEST_PG_URL");
}

/// T28 — the AC programs on MySQL.
#[test]
#[ignore = "needs a MySQL server: PRC_TEST_MYSQL_URL"]
fn mysql_runs_the_ac_programs() {
    rerun_on_server("PRC_TEST_MYSQL_URL");
}

// ── The drivers, value by value, against a live server ──────────────────────

use cobolt_runtime::esql::backend::{self, Backend};
use cobolt_runtime::esql::value::SqlValue;

fn server(var: &str) -> Option<Box<dyn Backend>> {
    let url = std::env::var(var).ok().filter(|u| !u.trim().is_empty());
    let Some(url) = url else {
        println!("SKIPPED: {var} is not set");
        return None;
    };
    let mut b = backend::open(&url).unwrap_or_else(|e| panic!("{var}: {}", e.message));
    // A test that leaves a unit of work open fails in seconds, rather than waiting
    // on a lock for as long as the server allows.
    let _ = match b.kind() {
        backend::BackendKind::Postgres => b.execute("SET lock_timeout = '10s'", &[]),
        backend::BackendKind::MySql => b.execute("SET SESSION lock_wait_timeout = 10", &[]),
        backend::BackendKind::Sqlite => Ok(0),
    };
    Some(b)
}

/// One row of constants, read back as the driver decodes them.
fn one_row(b: &mut Box<dyn Backend>, select: &str) -> Vec<SqlValue> {
    let rows = b.query(select, &[]).unwrap_or_else(|e| panic!("{select}: {}", e.message));
    assert_eq!(rows.rows.len(), 1, "{select}");
    rows.rows.into_iter().next().unwrap()
}

fn dec(mantissa: i128, scale: u8) -> SqlValue {
    SqlValue::Decimal { mantissa, scale }
}

/// T27: every PostgreSQL type the binary decoders read, as the server sends it —
/// not as the unit tests hand-built it.
#[test]
#[ignore = "needs a PostgreSQL server: PRC_TEST_PG_URL"]
fn pg_decodes_every_type_it_reads() {
    let Some(mut b) = server("PRC_TEST_PG_URL") else { return };
    let got = one_row(
        &mut b,
        "SELECT 42::int2, (-7)::int4, 9223372036854775807::int8, 1.5::float4, 2.25::float8, \
         12345.67::numeric(10,2), (-0.0005)::numeric, 'NaN'::numeric, 100000000::numeric(12,0), true, false, \
         'héllo ✓'::text, 'ab'::varchar(5), 'x'::char(3), '\\x01ab'::bytea, \
         DATE '2026-03-01', TIME '13:01:36.5', TIMESTAMP '2026-03-01 13:01:36.000001', \
         TIMESTAMPTZ '2026-03-01 13:01:36+00', '00010203-0405-0607-0809-0a0b0c0d0e0f'::uuid, NULL::int4, \
         '{\"a\":1}'::jsonb, 'infinity'::date, 4294967295::oid",
    );
    let want = vec![
        SqlValue::Int(42),
        SqlValue::Int(-7),
        SqlValue::Int(i64::MAX),
        SqlValue::Float(1.5),
        SqlValue::Float(2.25),
        dec(1234567, 2),
        dec(-5, 4),
        SqlValue::Float(f64::NAN),
        SqlValue::Int(100000000),
        SqlValue::Bool(true),
        SqlValue::Bool(false),
        SqlValue::Text("héllo ✓".into()),
        SqlValue::Text("ab".into()),
        SqlValue::Text("x  ".into()),
        SqlValue::Bytes(vec![1, 0xAB]),
        SqlValue::Text("2026-03-01".into()),
        SqlValue::Text("13:01:36.5".into()),
        SqlValue::Text("2026-03-01 13:01:36.000001".into()),
        SqlValue::Text("2026-03-01 13:01:36+00".into()),
        SqlValue::Text("00010203-0405-0607-0809-0a0b0c0d0e0f".into()),
        SqlValue::Null,
        SqlValue::Text("{\"a\": 1}".into()),
        SqlValue::Text("infinity".into()),
        SqlValue::Int(4294967295),
    ];
    assert_eq!(got.len(), want.len());
    for (i, (g, w)) in got.iter().zip(&want).enumerate() {
        match (g, w) {
            (SqlValue::Float(a), SqlValue::Float(b)) if b.is_nan() => assert!(a.is_nan(), "column {i}"),
            _ => assert_eq!(g, w, "column {i}"),
        }
    }
    // A type with no host variable to hold it is 0A000, never a wrong value.
    let e = b.query("SELECT POINT(1,2)", &[]).unwrap_err();
    assert_eq!(e.sqlstate, "0A000", "{}", e.message);
    println!("PostgreSQL decoders: {} columns decoded exactly as the server sent them; POINT → 0A000", got.len());
}

/// T27: values go in as bound parameters and come back unchanged — text, quotes,
/// NUL-free binary, an exact 30-digit decimal, a date, booleans and NULL.
#[test]
#[ignore = "needs a PostgreSQL server: PRC_TEST_PG_URL"]
fn pg_parameters_round_trip() {
    let Some(mut b) = server("PRC_TEST_PG_URL") else { return };
    b.execute("DROP TABLE IF EXISTS prc_rt", &[]).unwrap();
    b.execute("CREATE TABLE prc_rt (i BIGINT, d NUMERIC(40,10), t TEXT, y BYTEA, o BOOLEAN, dt DATE, f DOUBLE PRECISION)", &[]).unwrap();
    let nasty = "Robert'); DROP TABLE prc_rt;-- \"quoted\" ünïcode ✓ \\ backslash";
    let params = [
        SqlValue::Int(i64::MIN),
        dec(123456789012345678901234567890i128, 10),
        SqlValue::Text(nasty.into()),
        SqlValue::Bytes(vec![0xFF, 0x00, 0x7F, 0x80]),
        SqlValue::Bool(true),
        SqlValue::Text("2026-03-01".into()),
        SqlValue::Float(0.1),
    ];
    let n = b.execute("INSERT INTO prc_rt VALUES ($1, $2, $3, $4, $5, $6, $7)", &params).unwrap();
    assert_eq!(n, 1);
    b.execute("INSERT INTO prc_rt VALUES ($1, $2, $3, $4, $5, $6, $7)", &[SqlValue::Null, SqlValue::Null, SqlValue::Null, SqlValue::Null, SqlValue::Null, SqlValue::Null, SqlValue::Null]).unwrap();
    let rows = b.query("SELECT i, d, t, y, o, dt::text, f FROM prc_rt ORDER BY i NULLS LAST", &[]).unwrap();
    assert_eq!(rows.rows.len(), 2);
    assert_eq!(rows.rows[0][0], SqlValue::Int(i64::MIN));
    assert_eq!(rows.rows[0][1], dec(123456789012345678901234567890i128, 10));
    assert_eq!(rows.rows[0][2], SqlValue::Text(nasty.into()));
    assert_eq!(rows.rows[0][3], SqlValue::Bytes(vec![0xFF, 0x00, 0x7F, 0x80]));
    assert_eq!(rows.rows[0][4], SqlValue::Bool(true));
    assert_eq!(rows.rows[0][5], SqlValue::Text("2026-03-01".into()));
    assert_eq!(rows.rows[0][6], SqlValue::Float(0.1));
    assert!(rows.rows[1].iter().all(|v| *v == SqlValue::Null), "NULL stays NULL in every type");
    // A bound value is a value, never SQL: the table is still there.
    assert_eq!(b.query("SELECT COUNT(*) FROM prc_rt", &[]).unwrap().rows[0][0], SqlValue::Int(2));
    b.execute("DROP TABLE prc_rt", &[]).unwrap();
    println!("PostgreSQL parameters: 7 typed values + 7 NULLs round-trip; the hostile text stayed text");
}

/// T28: every MySQL type the driver hands over, read as the server sends it.
#[test]
#[ignore = "needs a MySQL server: PRC_TEST_MYSQL_URL"]
fn mysql_decodes_every_type_it_reads() {
    let Some(mut b) = server("PRC_TEST_MYSQL_URL") else { return };
    let got = one_row(
        &mut b,
        "SELECT CAST(-7 AS SIGNED), CAST(18446744073709551615 AS UNSIGNED), CAST(12345.67 AS DECIMAL(10,2)), \
         CAST(-0.0005 AS DECIMAL(10,4)), CAST(100000000 AS DECIMAL(12,0)), CAST(1.5 AS DOUBLE), \
         'héllo ✓', CAST('ab' AS CHAR(5)), x'01ab', CAST(0x01ab AS BINARY(2)), \
         DATE '2026-03-01', CAST('13:01:36.5' AS TIME(6)), CAST('-26:03:04' AS TIME), \
         CAST('2026-03-01 13:01:36.000001' AS DATETIME(6)), NULL, TRUE, CAST('{\"a\":1}' AS JSON), 1 = 1",
    );
    let want = vec![
        SqlValue::Int(-7),
        dec(18446744073709551615i128, 0),
        dec(1234567, 2),
        dec(-5, 4),
        SqlValue::Int(100000000),
        SqlValue::Float(1.5),
        SqlValue::Text("héllo ✓".into()),
        SqlValue::Text("ab".into()),
        SqlValue::Bytes(vec![1, 0xAB]),
        SqlValue::Bytes(vec![1, 0xAB]),
        SqlValue::Text("2026-03-01".into()),
        SqlValue::Text("13:01:36.5".into()),
        SqlValue::Text("-26:03:04".into()),
        SqlValue::Text("2026-03-01 13:01:36.000001".into()),
        SqlValue::Null,
        SqlValue::Int(1),
        SqlValue::Text("{\"a\": 1}".into()),
        SqlValue::Int(1),
    ];
    assert_eq!(got.len(), want.len());
    for (i, (g, w)) in got.iter().zip(&want).enumerate() {
        assert_eq!(g, w, "column {i}");
    }
    println!("MySQL decoders: {} columns decoded exactly as the server sent them", got.len());
}

/// T28: bound parameters on MySQL — and `CLIENT_FOUND_ROWS`, an UPDATE that
/// changes nothing still reports the row it matched.
#[test]
#[ignore = "needs a MySQL server: PRC_TEST_MYSQL_URL"]
fn mysql_parameters_round_trip() {
    let Some(mut b) = server("PRC_TEST_MYSQL_URL") else { return };
    b.execute("DROP TABLE IF EXISTS prc_rt", &[]).unwrap();
    b.execute("CREATE TABLE prc_rt (i BIGINT, d DECIMAL(40,10), t TEXT, y BLOB, o BOOLEAN, dt DATE, f DOUBLE, k INT PRIMARY KEY AUTO_INCREMENT) CHARACTER SET utf8mb4", &[]).unwrap();
    let nasty = "Robert'); DROP TABLE prc_rt;-- \"quoted\" ünïcode ✓ \\ backslash";
    let params = [
        SqlValue::Int(i64::MIN),
        dec(123456789012345678901234567890i128, 10),
        SqlValue::Text(nasty.into()),
        SqlValue::Bytes(vec![0xFF, 0x00, 0x7F, 0x80]),
        SqlValue::Bool(true),
        SqlValue::Text("2026-03-01".into()),
        SqlValue::Float(0.1),
    ];
    let n = b.execute("INSERT INTO prc_rt (i, d, t, y, o, dt, f) VALUES (?, ?, ?, ?, ?, ?, ?)", &params).unwrap();
    assert_eq!(n, 1);
    b.execute("INSERT INTO prc_rt (i, d, t, y, o, dt, f) VALUES (?, ?, ?, ?, ?, ?, ?)", &[SqlValue::Null, SqlValue::Null, SqlValue::Null, SqlValue::Null, SqlValue::Null, SqlValue::Null, SqlValue::Null]).unwrap();
    let rows = b.query("SELECT i, d, t, y, o, CAST(dt AS CHAR), f FROM prc_rt ORDER BY k", &[]).unwrap();
    assert_eq!(rows.rows.len(), 2);
    assert_eq!(rows.rows[0][0], SqlValue::Int(i64::MIN));
    assert_eq!(rows.rows[0][1], dec(123456789012345678901234567890i128, 10));
    assert_eq!(rows.rows[0][2], SqlValue::Text(nasty.into()));
    assert_eq!(rows.rows[0][3], SqlValue::Bytes(vec![0xFF, 0x00, 0x7F, 0x80]));
    assert_eq!(rows.rows[0][4], SqlValue::Int(1));
    assert_eq!(rows.rows[0][5], SqlValue::Text("2026-03-01".into()));
    assert_eq!(rows.rows[0][6], SqlValue::Float(0.1));
    assert!(rows.rows[1][..7].iter().all(|v| *v == SqlValue::Null), "NULL stays NULL in every type");
    assert_eq!(b.query("SELECT COUNT(*) FROM prc_rt", &[]).unwrap().rows[0][0], SqlValue::Int(2));
    // CLIENT_FOUND_ROWS: the row matched, though nothing changed.
    let matched = b.execute("UPDATE prc_rt SET i = i WHERE k = 1", &[]).unwrap();
    assert_eq!(matched, 1, "an UPDATE that changes nothing still matched its row");
    b.execute("DROP TABLE prc_rt", &[]).unwrap();
    println!("MySQL parameters: 7 typed values + 7 NULLs round-trip; the hostile text stayed text; UPDATE matched = 1");
}

// ── What only a server does: units of work, plans, lost connections ─────────

use cobolt_runtime::esql::session::Session;

fn session(var: &str, name: &str) -> Option<Session> {
    server(var).map(|b| Session::new(name, b))
}

/// R22 on a server: a statement that fails does not take the unit of work with
/// it — PostgreSQL would otherwise refuse everything until a ROLLBACK — and what
/// the unit did before and after the failure is committed, and seen by another
/// connection.
fn failed_statement_keeps_the_unit(var: &str, serial: &str) {
    let (Some(mut a), Some(mut other)) = (session(var, "A"), session(var, "B")) else { return };
    a.execute("DROP TABLE IF EXISTS prc_uow", &[], true).unwrap();
    a.execute(&format!("CREATE TABLE prc_uow (id {serial} PRIMARY KEY)"), &[], true).unwrap();
    a.commit().unwrap();
    a.execute("INSERT INTO prc_uow VALUES (1)", &[], true).unwrap();
    let dup = a.execute("INSERT INTO prc_uow VALUES (1)", &[], true).unwrap_err();
    assert_eq!(dup.sqlstate, "23505", "{}", dup.message);
    let syntax = a.execute("SELEC 1", &[], true).unwrap_err();
    assert_eq!(syntax.sqlstate, "42601", "{}", syntax.message);
    let table = a.query("SELECT * FROM prc_no_such_table", &[]).unwrap_err();
    assert_eq!(table.sqlstate, "42P01", "{}", table.message);
    assert!(a.in_unit(), "the unit of work survived three failures");
    a.execute("INSERT INTO prc_uow VALUES (2)", &[], true).unwrap();
    assert_eq!(other.query("SELECT COUNT(*) FROM prc_uow", &[]).unwrap().rows[0][0], SqlValue::Int(0), "nothing is visible before COMMIT");
    a.commit().unwrap();
    // The reader's own unit of work first: MySQL reads a snapshot taken at the unit's
    // first statement (REPEATABLE READ), so only a new unit sees what was committed.
    other.commit().unwrap();
    assert_eq!(other.query("SELECT COUNT(*) FROM prc_uow", &[]).unwrap().rows[0][0], SqlValue::Int(2), "both inserts committed");
    // And ROLLBACK takes back what came after the failures too.
    a.execute("INSERT INTO prc_uow VALUES (3)", &[], true).unwrap();
    let _ = a.execute("INSERT INTO prc_uow VALUES (3)", &[], true).unwrap_err();
    a.rollback().unwrap();
    other.commit().unwrap();
    assert_eq!(other.query("SELECT COUNT(*) FROM prc_uow", &[]).unwrap().rows[0][0], SqlValue::Int(2));
    // The reader's unit of work holds the table; ending it lets the table go.
    other.commit().unwrap();
    a.execute("DROP TABLE prc_uow", &[], true).unwrap();
    a.commit().unwrap();
    println!("{var}: a duplicate key, a syntax error and a missing table left the unit alive; COMMIT kept 2 rows, ROLLBACK took back the third");
}

#[test]
#[ignore = "needs a PostgreSQL server: PRC_TEST_PG_URL"]
fn pg_a_failed_statement_keeps_the_unit_of_work() {
    failed_statement_keeps_the_unit("PRC_TEST_PG_URL", "INTEGER");
}

#[test]
#[ignore = "needs a MySQL server: PRC_TEST_MYSQL_URL"]
fn mysql_a_failed_statement_keeps_the_unit_of_work() {
    failed_statement_keeps_the_unit("PRC_TEST_MYSQL_URL", "INTEGER");
}

/// A statement prepared and reused goes stale when a table it reads changes
/// shape; the next run starts over and succeeds, as it would on any database.
#[test]
#[ignore = "needs a PostgreSQL server: PRC_TEST_PG_URL"]
fn pg_a_stale_plan_is_replaced() {
    let Some(mut b) = server("PRC_TEST_PG_URL") else { return };
    b.execute("DROP TABLE IF EXISTS prc_stale", &[]).unwrap();
    b.execute("CREATE TABLE prc_stale (a INTEGER)", &[]).unwrap();
    b.execute("INSERT INTO prc_stale VALUES (1)", &[]).unwrap();
    let q = "SELECT * FROM prc_stale";
    assert_eq!(b.query(q, &[]).unwrap().columns.len(), 1);
    b.execute("ALTER TABLE prc_stale ADD COLUMN b INTEGER DEFAULT 5", &[]).unwrap();
    let r = b.query(q, &[]).expect("the statement is prepared afresh, not refused");
    assert_eq!(r.rows, vec![vec![SqlValue::Int(1), SqlValue::Int(5)]]);
    b.execute("DROP TABLE prc_stale", &[]).unwrap();
    println!("PostgreSQL: SELECT * after ALTER TABLE ADD COLUMN → 1 row of 2 columns (the cached plan was replaced)");
}

/// MySQL ends the unit of work on its own after a data-definition statement
/// (R36); the session notices and opens a new one for the next statement.
#[test]
#[ignore = "needs a MySQL server: PRC_TEST_MYSQL_URL"]
fn mysql_ddl_commits_and_the_next_statement_opens_a_new_unit() {
    let (Some(mut a), Some(mut other)) = (session("PRC_TEST_MYSQL_URL", "A"), session("PRC_TEST_MYSQL_URL", "B")) else { return };
    a.execute("DROP TABLE IF EXISTS prc_ddl", &[], true).unwrap();
    a.execute("DROP TABLE IF EXISTS prc_ddl2", &[], true).unwrap();
    a.execute("CREATE TABLE prc_ddl (id INT)", &[], true).unwrap();
    assert!(!a.in_unit(), "DDL committed, so no unit is open");
    a.execute("INSERT INTO prc_ddl VALUES (1)", &[], true).unwrap();
    assert!(a.in_unit());
    a.execute("CREATE TABLE prc_ddl2 (id INT)", &[], true).unwrap();
    assert!(!a.in_unit(), "…and the DDL committed the INSERT before it");
    other.commit().unwrap();
    assert_eq!(other.query("SELECT COUNT(*) FROM prc_ddl", &[]).unwrap().rows[0][0], SqlValue::Int(1), "the INSERT was committed by the DDL after it");
    a.execute("INSERT INTO prc_ddl VALUES (2)", &[], true).unwrap();
    a.rollback().unwrap();
    other.commit().unwrap();
    assert_eq!(other.query("SELECT COUNT(*) FROM prc_ddl", &[]).unwrap().rows[0][0], SqlValue::Int(1), "ROLLBACK undid only the new unit");
    other.commit().unwrap();
    a.execute("DROP TABLE prc_ddl", &[], true).unwrap();
    a.execute("DROP TABLE prc_ddl2", &[], true).unwrap();
    println!("MySQL: DDL commits; the next statement opens a new unit; ROLLBACK undid only that one");
}

/// A connection the server ends does not hang or panic the program: the next
/// statement fails with a connection-class SQLSTATE (R18, R22) and says why.
fn lost_connection_is_an_error(var: &str, kill: impl Fn(&mut Box<dyn Backend>, i64)) {
    let (Some(mut victim), Some(mut killer)) = (server(var), server(var)) else { return };
    let id = match victim.query(if var.contains("PG") { "SELECT pg_backend_pid()::int8" } else { "SELECT CONNECTION_ID()" }, &[]).unwrap().rows[0][0].clone() {
        SqlValue::Int(i) => i,
        other => panic!("a connection id, got {other:?}"),
    };
    kill(&mut killer, id);
    std::thread::sleep(std::time::Duration::from_millis(300));
    let e = victim.query("SELECT 1", &[]).unwrap_err();
    println!("{var}: after the server ended the connection → SQLSTATE {} — {}", e.sqlstate, e.message);
    assert!(e.sqlstate.starts_with("08") || e.sqlstate.starts_with("57"), "a connection-class state, got {}", e.sqlstate);
    assert!(!e.message.is_empty());
}

#[test]
#[ignore = "needs a PostgreSQL server: PRC_TEST_PG_URL"]
fn pg_a_lost_connection_is_an_error() {
    lost_connection_is_an_error("PRC_TEST_PG_URL", |k, id| {
        k.query(&format!("SELECT pg_terminate_backend({id})"), &[]).unwrap();
    });
}

#[test]
#[ignore = "needs a MySQL server: PRC_TEST_MYSQL_URL"]
fn mysql_a_lost_connection_is_an_error() {
    lost_connection_is_an_error("PRC_TEST_MYSQL_URL", |k, id| {
        k.execute(&format!("KILL CONNECTION {id}"), &[]).unwrap();
    });
}

/// The harness's own pieces, which need no server.
#[test]
fn the_live_harness_finds_what_each_program_creates() {
    assert_eq!(
        created_tables("EXEC SQL CREATE TABLE ORDERS (ID INTEGER) END-EXEC\n EXEC SQL create table if not exists X (A INT) END-EXEC"),
        ["ORDERS"]
    );
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/cobol/esql");
    for name in AC_PROGRAMS {
        let src = std::fs::read_to_string(dir.join(name)).unwrap();
        assert!(regex_free_connects().iter().any(|c| src.contains(c.as_str())), "{name} connects in a form the harness redirects");
    }
}
