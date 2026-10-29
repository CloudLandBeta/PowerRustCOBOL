// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Spec 108, T15 — `COBOL::"PARTIQL"`, `COBOL::"PARTIQL-FETCH"` and
//! `COBOL::"PARTIQL-ERROR"`: PartiQL on a file the program's own `FD` opened,
//! for a program with no control. Arguments are data items, typed by PICTURE.
//!
//! One result block at the end (GOLDEN RULE #7).

#[path = "support/partiql_fixtures.rs"]
mod fx;

use std::sync::mpsc;
use std::time::Instant;

use cobolt_lexer::{tokenize, SourceFormat};
use cobolt_parser::{parse, Severity};
use cobolt_runtime::Interpreter;
use fx::Scratch;

fn run(src: &str) -> Vec<String> {
    let parsed = parse(tokenize(src, SourceFormat::Free));
    assert!(parsed.diagnostics.iter().all(|d| d.severity != Severity::Error), "parse errors: {:?}\n{src}", parsed.diagnostics);
    let (_etx, erx) = mpsc::channel();
    let (stx, _srx) = mpsc::channel();
    let (dtx, drx) = mpsc::channel();
    let mut interp = Interpreter::new_with_channels(parsed.program.unwrap(), erx, stx, dtx);
    interp.run().expect("run");
    drx.try_iter().collect()
}

#[test]
fn partiql_builtins() {
    let t0 = Instant::now();
    let dir = Scratch::new("builtin");
    let data = dir.0.join("b.idx").to_string_lossy().to_string();
    let call = |args: &str| format!("           COBOL::\"PARTIQL\" ( \"CUSTOMERS\" WS-STMT WS-N WS-ST {args} )\n");
    let stmt = |s: &str| format!("           MOVE \"{s}\" TO WS-STMT\n");
    let src = format!(
        "       IDENTIFICATION DIVISION.\n       PROGRAM-ID. T.\n       ENVIRONMENT DIVISION.\n       INPUT-OUTPUT SECTION.\n       FILE-CONTROL.\n           SELECT CUSTOMERS ASSIGN TO \"{data}\"\n               ORGANIZATION IS INDEXED ACCESS MODE IS DYNAMIC\n               RECORD KEY IS CUST-ID.\n       DATA DIVISION.\n       FILE SECTION.\n       FD CUSTOMERS.\n       01 CUST-REC.\n           05 CUST-ID PIC 9(8).\n           05 CUST-NAME PIC X(20).\n           05 CUST-BALANCE PIC S9(6)V99.\n       WORKING-STORAGE SECTION.\n       01 WS-STMT PIC X(120).\n       01 WS-N PIC 9(4).\n       01 WS-ST PIC XX.\n       01 WS-ROW PIC X(60).\n       01 WS-ERR PIC X(100).\n       01 WS-ID PIC 9(8).\n       01 WS-BAL PIC S9(6)V99.\n       PROCEDURE DIVISION.\n           OPEN OUTPUT CUSTOMERS\n           CLOSE CUSTOMERS\n           OPEN I-O CUSTOMERS\n{}           MOVE 42 TO WS-ID\n           MOVE -123.45 TO WS-BAL\n{}           DISPLAY \"INSERT \" WS-N \" \" WS-ST\n{}{}           COBOL::\"PARTIQL-FETCH\" ( \"CUSTOMERS\" WS-ROW WS-ST )\n           DISPLAY \"ROW \" WS-ROW\n           COBOL::\"PARTIQL-FETCH\" ( \"CUSTOMERS\" WS-ROW WS-ST )\n           DISPLAY \"PAST \" WS-ST\n{}{}           DISPLAY \"DUP \" WS-N \" \" WS-ST\n           COBOL::\"PARTIQL-ERROR\" ( \"CUSTOMERS\" WS-ERR )\n           DISPLAY WS-ERR\n{}{}           DISPLAY \"BAD \" WS-ST\n           COBOL::\"PARTIQL-ERROR\" ( \"CUSTOMERS\" WS-ERR )\n           DISPLAY WS-ERR\n           CLOSE CUSTOMERS\n           STOP RUN.\n",
        stmt("INSERT INTO CUSTOMERS VALUE {'CUST-ID': ?, 'CUST-NAME': 'Ana', 'CUST-BALANCE': ?}"),
        call("WS-ID WS-BAL"),
        stmt("SELECT CUST-ID, CUST-NAME, CUST-BALANCE FROM CUSTOMERS WHERE CUST-ID = ?"),
        call("WS-ID"),
        stmt("INSERT INTO CUSTOMERS VALUE {'CUST-ID': ?, 'CUST-NAME': 'Bia'}"),
        call("WS-ID"),
        stmt("SELECT COUNT(*) FROM CUSTOMERS"),
        call(""),
    );
    let out = run(&src);
    let shown = out.iter().map(|l| l.replace('\t', "<TAB>")).collect::<Vec<_>>();
    let mut failures = Vec::new();
    let mut check = |ok: bool, what: &str| {
        if !ok {
            failures.push(format!("{what}: {shown:?}"));
        }
    };
    check(shown.iter().any(|l| l.starts_with("INSERT 0001 00")), "INSERT answers 1 with 00");
    check(shown.iter().any(|l| l.starts_with("ROW 00000042<TAB>Ana") && l.contains("-123.45")), "the row, with its decimal point");
    check(shown.iter().any(|l| l.starts_with("PAST 10")), "past the last item is 10");
    check(shown.iter().any(|l| l.starts_with("DUP 0000 22")), "a duplicate key is 22");
    check(shown.iter().any(|l| l.starts_with("DuplicateItemException")), "PARTIQL-ERROR names the exception");
    check(shown.iter().any(|l| l.starts_with("BAD 94")), "an invalid statement is 94");
    check(shown.iter().any(|l| l.starts_with("ValidationException") && l.contains("COUNT")), "PARTIQL-ERROR names the construct");

    println!("\n=== test_partiql_builtins ===");
    println!("forms exercised: COBOL::\"PARTIQL\" (INSERT, SELECT with ? from data items typed by PICTURE), COBOL::\"PARTIQL-FETCH\", COBOL::\"PARTIQL-ERROR\"; no control, the program's own FD (PRCIDXD1)");
    println!("PICTUREs: 9(8) key, X(20), S9(6)V99 (parameter -123.45 from a PIC S9(6)V99 item)");
    println!("checks: {} passed, {} failed; elapsed {} ms", 7 - failures.len(), failures.len(), t0.elapsed().as_millis());
    for f in &failures {
        println!("  FAIL {f}");
    }
    assert!(failures.is_empty());
}
