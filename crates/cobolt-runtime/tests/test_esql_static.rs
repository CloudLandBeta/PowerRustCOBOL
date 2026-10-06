// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Spec 087 T14 — static embedded SQL against SQLite, through COBOL programs
//! in `tests/cobol/esql/`. Each program checks itself and ends with a result
//! block (GOLDEN RULE #7); this harness runs it as a host does — `INCLUDE`
//! expanded — prints the block and requires `FAIL 000`.

use std::path::{Path, PathBuf};
use std::sync::mpsc;

use cobolt_lexer::{expand_copybooks, tokenize_expansion, SourceFormat};
use cobolt_parser::{parse, Severity};
use cobolt_runtime::{DebugEvent, Interpreter, OutputChannel};

fn esql_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/cobol/esql")
}

fn program(src: &str, dir: &Path) -> cobolt_ast::program::Program {
    let exp = expand_copybooks(src, dir, SourceFormat::Free);
    assert!(exp.errors.is_empty(), "copybook errors: {:?}", exp.errors);
    let result = parse(tokenize_expansion(&exp));
    let errors: Vec<_> = result.diagnostics.iter().filter(|d| d.severity == Severity::Error).collect();
    assert!(errors.is_empty(), "parse errors: {errors:?}");
    result.program.expect("no program")
}

/// Run a program and return what it displayed.
fn run_src(src: &str) -> Vec<String> {
    let prog = program(src, &esql_dir());
    let (_event_tx, event_rx) = mpsc::channel();
    let (state_tx, _state_rx) = mpsc::channel();
    let (display_tx, display_rx) = mpsc::channel();
    let mut interp = Interpreter::new_with_channels(prog, event_rx, state_tx, display_tx);
    interp.run().expect("run failed");
    display_rx.try_iter().map(|l| l.trim_end().to_string()).collect()
}

fn run_file(name: &str) -> Vec<String> {
    let src = std::fs::read_to_string(esql_dir().join(name)).unwrap();
    let out = run_src(&src);
    println!("--- {name} ---");
    for l in &out {
        println!("{l}");
    }
    out
}

/// The program's own verdict: its last `PASS n FAIL n` line says no failures.
fn assert_passed(out: &[String]) {
    let verdict = out.iter().rev().find(|l| l.starts_with("PASS ")).expect("no PASS/FAIL line");
    assert!(verdict.ends_with("FAIL 000"), "the program reported failures:\n{}", out.join("\n"));
}

/// AC2 — a host structure (FILLER and REDEFINES skipped) and bound values.
#[test]
fn ac2_host_structure_and_bound_values() {
    assert_passed(&run_file("ac2_host_structure.cbl"));
}

/// AC3 — NULL with and without an indicator, NULL written, truncation.
#[test]
fn ac3_null_and_truncation() {
    assert_passed(&run_file("ac3_null_truncation.cbl"));
}

/// AC4 — round trips through every usage, maxima, and 22003.
#[test]
fn ac4_values_round_trip_and_overflow_is_22003() {
    assert_passed(&run_file("ac4_round_trips.cbl"));
}

/// AC5 — the stand-alone items and the SQLCA report the same SQLSTATE,
/// SQLCODE and message, and each outcome gives its documented pair.
#[test]
fn ac5_both_status_styles_agree_on_the_documented_pairs() {
    let norm = |out: Vec<String>| -> Vec<String> {
        out.into_iter()
            .filter(|l| l.starts_with("STATUS "))
            .map(|l| l.split_whitespace().collect::<Vec<_>>().join(" "))
            .collect()
    };
    let standalone = norm(run_file("ac5_status_standalone.cbl"));
    let sqlca = norm(run_file("ac5_status_sqlca.cbl"));
    assert_eq!(standalone, sqlca, "both styles report the same");
    let pairs: Vec<(String, String)> = standalone
        .iter()
        .map(|l| {
            let w: Vec<&str> = l.split(' ').collect();
            (w[1].to_string(), w[2].to_string())
        })
        .collect();
    let want = [("00000", "0"), ("02000", "100"), ("01004", "1004"), ("23505", "-23505"), ("42601", "-42601")];
    assert_eq!(pairs, want.map(|(a, b)| (a.to_string(), b.to_string())));
    assert!(standalone[3].contains("UNIQUE"), "the database's message is kept: {}", standalone[3]);
    assert!(standalone[4].contains("syntax error"), "{}", standalone[4]);
}

/// AC6 — WHENEVER in source order; an error with none in force continues.
#[test]
fn ac6_whenever_follows_the_source() {
    assert_passed(&run_file("ac6_whenever.cbl"));
}

/// R48/R52 — the debugger's SQL line never shows a CONNECT password.
#[test]
fn the_sql_debug_line_masks_the_password() {
    const SECRET: &str = "s3cr3t-pass-87";
    let src = format!(
        "IDENTIFICATION DIVISION.
PROGRAM-ID. ESQL-MASK.
DATA DIVISION.
WORKING-STORAGE SECTION.
01 WS-USER PIC X(20) VALUE \"ana\".
01 WS-PASSWORD PIC X(20) VALUE \"{SECRET}\".
01 WS-N PIC 9(5).
PROCEDURE DIVISION.
MAIN-PARA.
    EXEC SQL CONNECT TO ':memory:' AS M USER :WS-USER USING :WS-PASSWORD END-EXEC
    EXEC SQL SELECT 42 INTO :WS-N END-EXEC
    DISPLAY \"N \" WS-N
    STOP RUN.
"
    );
    let prog = program(&src, &esql_dir());
    let (_event_tx, event_rx) = mpsc::channel();
    let (state_tx, _state_rx) = mpsc::channel();
    let (display_tx, display_rx) = mpsc::channel();
    let (_cmd_tx, cmd_rx) = mpsc::channel();
    let (ev_tx, ev_rx) = mpsc::channel();
    let mut interp = Interpreter::new_with_channels(prog, event_rx, state_tx, display_tx);
    interp.attach_debug_channels_running(cmd_rx, ev_tx, cobolt_runtime::new_breakpoints());
    interp.run().expect("run failed");
    let sql: Vec<String> = ev_rx
        .try_iter()
        .filter_map(|e| match e {
            DebugEvent::Output { text, channel } if channel == OutputChannel::Sql => Some(text),
            _ => None,
        })
        .collect();
    println!("SQL channel:\n{}", sql.join("\n"));
    assert_eq!(sql.len(), 2, "one line per statement");
    assert!(sql[0].contains("CONNECT TO :memory: USER ana USING ******"), "{}", sql[0]);
    assert!(sql[1].contains("SQLSTATE 00000 SQLCODE 0 rows 1"), "{}", sql[1]);
    assert!(sql.iter().all(|l| !l.contains(SECRET)), "the password leaked: {sql:?}");
    let shown: Vec<String> = display_rx.try_iter().collect();
    assert!(shown.iter().any(|l| l.trim() == "N 00042"), "{shown:?}");
}
