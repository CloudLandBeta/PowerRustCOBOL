// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Spec 108, T29 — the debugger shows the statement: a PartiQL call writes one
//! `PARTIQL` line on the File I/O channel with the statement, the bound
//! parameter values, the read class, the status, the count and the error text;
//! the parameter value is on the channel and never in `LastError` (R29).
//!
//! One result block at the end (GOLDEN RULE #7).

#[path = "support/partiql_fixtures.rs"]
mod fx;

use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

use cobolt_lexer::{tokenize, SourceFormat};
use cobolt_parser::{parse, Severity};
use cobolt_runtime::{new_breakpoints, DebugCmd, DebugEvent, Interpreter, OutputChannel};
use fx::Scratch;

fn source(data: &str) -> String {
    let call = |args: &str| format!("           COBOL::\"PARTIQL\" ( \"CUSTOMERS\" WS-STMT WS-N WS-ST {args} )\n");
    let stmt = |s: &str| format!("           MOVE \"{s}\" TO WS-STMT\n");
    format!(
        "       IDENTIFICATION DIVISION.\n       PROGRAM-ID. T.\n       ENVIRONMENT DIVISION.\n       INPUT-OUTPUT SECTION.\n       FILE-CONTROL.\n           SELECT CUSTOMERS ASSIGN TO \"{data}\"\n               ORGANIZATION IS INDEXED ACCESS MODE IS DYNAMIC\n               RECORD KEY IS CUST-ID.\n       DATA DIVISION.\n       FILE SECTION.\n       FD CUSTOMERS.\n       01 CUST-REC.\n           05 CUST-ID PIC 9(8).\n           05 CUST-NAME PIC X(20).\n           05 CUST-BALANCE PIC S9(6)V99.\n       WORKING-STORAGE SECTION.\n       01 WS-STMT PIC X(120).\n       01 WS-N PIC 9(4).\n       01 WS-ST PIC XX.\n       01 WS-ID PIC 9(8).\n       PROCEDURE DIVISION.\n           OPEN OUTPUT CUSTOMERS\n           CLOSE CUSTOMERS\n           OPEN I-O CUSTOMERS\n           MOVE 777 TO WS-ID\n{}{}{}{}           CLOSE CUSTOMERS\n           STOP RUN.\n",
        stmt("INSERT INTO CUSTOMERS VALUE {'CUST-ID': ?, 'CUST-NAME': 'Ana', 'CUST-BALANCE': 1}"),
        call("WS-ID"),
        stmt("INSERT INTO CUSTOMERS VALUE {'CUST-ID': ?, 'CUST-NAME': 'Bia'}"),
        call("WS-ID"),
    )
}

/// The File I/O lines of a run under a debug listener, continuing every stop.
fn debugged(src: &str) -> Vec<String> {
    let parsed = parse(tokenize(src, SourceFormat::Free));
    assert!(parsed.diagnostics.iter().all(|d| d.severity != Severity::Error), "{:?}", parsed.diagnostics);
    let program = parsed.program.expect("a program");
    let (cmd_tx, cmd_rx) = mpsc::channel::<DebugCmd>();
    let (ev_tx, ev_rx) = mpsc::channel::<DebugEvent>();
    let handle = thread::spawn(move || {
        let mut interp = Interpreter::new_with_debug_channels(program, cmd_rx, ev_tx, new_breakpoints());
        let _ = interp.run();
    });
    let mut lines = Vec::new();
    loop {
        match ev_rx.recv_timeout(Duration::from_secs(20)) {
            Ok(DebugEvent::Paused { .. }) => {
                if cmd_tx.send(DebugCmd::Continue).is_err() {
                    break;
                }
            }
            Ok(DebugEvent::Output { text, channel: OutputChannel::FileIo }) => lines.push(text),
            Ok(DebugEvent::Finished) | Err(_) => break,
            Ok(_) => {}
        }
    }
    drop(cmd_tx);
    let _ = handle.join();
    lines
}

#[test]
fn a_partiql_call_writes_one_line_on_the_file_io_channel() {
    let t0 = Instant::now();
    let dir = Scratch::new("debug");
    let data = dir.0.join("d.idx").to_string_lossy().to_string();
    let src = source(&data);
    let lines = debugged(&src);
    let partiql: Vec<&String> = lines.iter().filter(|l| l.starts_with("PARTIQL")).collect();

    let mut failures = Vec::new();
    let mut check = |ok: bool, what: &str| {
        if !ok {
            failures.push(format!("{what}: {partiql:?}"));
        }
    };
    check(partiql.len() == 2, "one line per call");
    let first = partiql.first().map(|s| s.as_str()).unwrap_or("");
    check(first.contains("INSERT INTO CUSTOMERS") && first.contains("params [777]") && first.contains("status 00") && first.contains("count 1"), "the first line has statement, parameter, status and count");
    let second = partiql.get(1).map(|s| s.as_str()).unwrap_or("");
    check(second.contains("status 22") && second.contains("DuplicateItemException"), "the duplicate carries 22 and the exception");
    let error_text = second.rsplit("error [").next().unwrap_or("");
    check(!error_text.contains("777"), "LastError holds no parameter value (R29)");

    println!("\n=== test_partiql_debug ===");
    println!("forms exercised: COBOL::\"PARTIQL\" INSERT with a PIC 9(8) parameter, run under a debug listener; a duplicate key (22)");
    println!("checks: {} passed, {} failed; elapsed {} ms", 4 - failures.len(), failures.len(), t0.elapsed().as_millis());
    for f in &failures {
        println!("  FAIL {f}");
    }
    assert!(failures.is_empty());
}

#[test]
fn without_a_listener_the_run_is_unchanged() {
    let dir = Scratch::new("debug-off");
    let data = dir.0.join("d.idx").to_string_lossy().to_string();
    let parsed = parse(tokenize(&source(&data), SourceFormat::Free));
    let (_etx, erx) = mpsc::channel();
    let (stx, _srx) = mpsc::channel();
    let (dtx, drx) = mpsc::channel();
    let mut interp = Interpreter::new_with_channels(parsed.program.unwrap(), erx, stx, dtx);
    interp.run().expect("run");
    assert!(drx.try_iter().all(|l| !l.contains("PARTIQL")));
}
