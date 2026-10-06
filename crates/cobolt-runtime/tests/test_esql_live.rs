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

/// F-ART-PURGA: stand-alone status items, `DELETE … LIMIT 1` (which SQLite
/// rejects), `SELECT COUNT … INTO`, cursors over a query and over a prepared
/// statement, `COMMIT` — every distinct statement of the listing, as written.
#[test]
fn f_art_purga_sql_runs_on_sqlite() {
    let Some(dir) = legacy_dir() else { return };
    let lines = code_area(&dir.join("M-ARTICULOS/Debug/F-ART-PURGA.cob"));
    let mut b = Blocks::new(sql_blocks(&lines));
    let decls = declare_sections(&lines).join("\n");

    let mut p = String::new();
    p.push_str("       IDENTIFICATION DIVISION.\n       PROGRAM-ID. AC15-PURGA.\n       DATA DIVISION.\n       WORKING-STORAGE SECTION.\n");
    p.push_str(&decls);
    p.push_str("\n       01  WS-N PIC 9(9).\n       PROCEDURE DIVISION.\n       MAIN-PARA.\n");
    p.push_str(
        "           EXEC SQL CONNECT TO ':memory:' END-EXEC
           EXEC SQL CREATE TABLE ARTICULOS (ARTCOD TEXT PRIMARY KEY, ARTDES TEXT, ARTFAM INTEGER, ARTOBS1 TEXT, ARTELI TEXT) END-EXEC
           EXEC SQL INSERT INTO ARTICULOS VALUES ('A1', 'ALPHA', 1, 'one', 'S'), ('A2', 'BETA', 2, 'two', 'S'), ('A3', 'GAMMA', 3, '', 'N') END-EXEC
           EXEC SQL CREATE TABLE IMGART (IMGNUM INTEGER, IMGCOD TEXT, IMGRUT TEXT, IMGDES TEXT) END-EXEC
           EXEC SQL INSERT INTO IMGART VALUES (1, 'A1', '/img/a1.png', 'front'), (2, 'A1', '/img/a1-back.png', 'back') END-EXEC
           EXEC SQL CREATE TABLE CODE13 (ART13 TEXT, COD13 TEXT) END-EXEC
           EXEC SQL INSERT INTO CODE13 VALUES ('A1', '7790000000011') END-EXEC
           EXEC SQL COMMIT END-EXEC
           MOVE \"A1\" TO ARTCOD
",
    );
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

/// TyC: `CONNECT TO 'JOSBER'` — a named SQL connection, resolved through the
/// project's catalog (R34) — a `SELECT … INTO … LIMIT 1` with `INTO` after
/// `WHERE`, and `DISCONNECT 'JOSBER'`.
#[test]
fn tyc_sql_runs_on_sqlite() {
    let Some(dir) = legacy_dir() else { return };
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

    let mut p = String::new();
    p.push_str("       IDENTIFICATION DIVISION.\n       PROGRAM-ID. AC15-TYC.\n       DATA DIVISION.\n       WORKING-STORAGE SECTION.\n");
    p.push_str(&decls);
    p.push_str("\n       PROCEDURE DIVISION.\n       MAIN-PARA.\n");
    p.push_str(&b.take("CONNECT TO 'JOSBER'"));
    p.push_str("           DISPLAY \"CONNECT \" SQLSTATE\n           MOVE 8 TO CERO\n");
    p.push_str(&b.take("SELECT CERO"));
    p.push_str("           DISPLAY \"SELECT \" SQLSTATE \" \" DOS\n");
    p.push_str(&b.take("DISCONNECT 'JOSBER'"));
    p.push_str("           DISPLAY \"DISCONNECT \" SQLSTATE\n           STOP RUN.\n");
    let out = run(&p, Some(catalog()));
    println!("{}", out.join("\n"));
    assert!(b.unused().is_empty(), "statements of the listing not run: {:?}", b.unused());
    assert_eq!(out, ["CONNECT 00000", "SELECT 00000 OCHO", "DISCONNECT 00000"]);
    let _ = std::fs::remove_dir_all(&db_dir);
    println!("AC15 TyC: {} distinct statements, all run; CONNECT TO 'JOSBER' reached the named SQL connection; SELECT … INTO … LIMIT 1 → OCHO", b.distinct());
}

/// The AC programs that run on any database.
const AC_PROGRAMS: [&str; 10] = [
    "ac2_host_structure.cbl",
    "ac3_null_truncation.cbl",
    "ac4_round_trips.cbl",
    "ac5_status_standalone.cbl",
    "ac5_status_sqlca.cbl",
    "ac6_whenever.cbl",
    "ac7_cursors.cbl",
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
    let mut summary = Vec::new();
    for name in AC_PROGRAMS {
        let original = std::fs::read_to_string(dir.join(name)).unwrap();
        let mut src = original.clone();
        for target in &connect {
            src = src.replace(target, &format!("CONNECT TO '{url}'"));
        }
        // A server keeps its tables between programs: drop what this one creates.
        let mut backend = cobolt_runtime::esql::backend::open(&url).unwrap_or_else(|e| panic!("{var}: {}", e.message));
        for table in created_tables(&original) {
            let _ = backend.execute(&format!("DROP TABLE IF EXISTS {table}"), &[]);
        }
        drop(backend);
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
        let verdict = out.iter().rev().find(|l| l.starts_with("PASS ")).cloned().unwrap_or_default();
        println!("--- {name} ---\n{}", out.join("\n"));
        summary.push(format!("{name}: {verdict}"));
        assert!(verdict.ends_with("FAIL 000"), "{name} on {var}:\n{}", out.join("\n"));
    }
    println!("{var}: {}", summary.join("; "));
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
