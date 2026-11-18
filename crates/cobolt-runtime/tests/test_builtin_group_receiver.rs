// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! A built-in's text received into a GROUP item reaches the group's subordinate
//! items, as a group `MOVE` does (fixed in 2.0.600).
//!
//! Before the fix `FETCH-ROW`, `HTTP-GET` and every other built-in that stores
//! its answer by name wrote the text to a slot of the group's own name, which
//! nothing reads: the group and its subordinate items kept what they held.
//! The WebAssembly path always spread it, so the two paths disagreed.
//!
//! Each case runs one program through the interpreter and compares the
//! subordinate items, so a wrong split shows as well as a lost value. The HTTP
//! case answers from a `TcpListener` on `127.0.0.1` — no external network.

use std::io::{Read, Write};
use std::net::TcpListener;
use std::sync::mpsc;
use std::thread;
use std::time::Instant;

use cobolt_lexer::{tokenize, SourceFormat};
use cobolt_parser::{parse, Severity};
use cobolt_runtime::Interpreter;

fn run(src: &str) -> Vec<String> {
    let result = parse(tokenize(src, SourceFormat::Free));
    assert!(
        result.diagnostics.iter().all(|d| d.severity != Severity::Error),
        "parse errors: {:?}",
        result.diagnostics
    );
    let program = result.program.expect("no program");
    let (_event_tx, event_rx) = mpsc::channel();
    let (state_tx, _state_rx) = mpsc::channel();
    let (display_tx, display_rx) = mpsc::channel();
    let mut interp = Interpreter::new_with_channels(program, event_rx, state_tx, display_tx);
    interp.run().expect("run failed");
    display_rx.try_iter().map(|s| s.trim_end().to_owned()).collect()
}

/// A server that answers every request with `body`, status 200.
fn serve(body: &'static str) -> u16 {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut stream) = stream else { break };
            let mut head = Vec::new();
            let mut byte = [0u8; 1];
            while !head.ends_with(b"\r\n\r\n") {
                match stream.read(&mut byte) {
                    Ok(1) => head.push(byte[0]),
                    _ => break,
                }
            }
            let reply = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: text/plain\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            );
            let _ = stream.write_all(reply.as_bytes());
            let _ = stream.flush();
        }
    });
    port
}

const PROGRAM: &str = r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. GROUPRECV.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 WS-GRP.
          05 WS-G1    PIC X(4).
          05 WS-G2    PIC X(4).
       01 WS-REC.
          05 WS-A     PIC X(3).
          05 WS-SUB.
             10 WS-B  PIC X(2).
             10 WS-C  PIC X(3).
       01 WS-MIX.
          05 WS-CODE  PIC XX.
          05 WS-QTY   PIC 999.
       01 WS-ELEM     PIC X(6).
       01 WS-H        PIC 9(9).
       01 WS-ST       PIC X(40).
       01 WS-N        PIC 9(4).
       01 WS-URL      PIC X(60) VALUE "@URL@".
       PROCEDURE DIVISION.
       MAIN.
           MOVE "hello, world" TO WS-GRP
           DISPLAY "MOVE [" WS-G1 "|" WS-G2 "]"
           COBOL::"OPEN-DB" ( ":memory:" WS-H WS-ST )
           COBOL::"EXEC-SQL" ( WS-H "SELECT 'hello, world'" WS-N WS-ST )
           MOVE ALL "*" TO WS-GRP
           COBOL::"FETCH-ROW" ( WS-H 1 WS-GRP WS-ST )
           DISPLAY "FETCH [" WS-G1 "|" WS-G2 "]"
           COBOL::"EXEC-SQL" ( WS-H "SELECT 'hi'" WS-N WS-ST )
           MOVE ALL "*" TO WS-GRP
           COBOL::"FETCH-ROW" ( WS-H 1 WS-GRP WS-ST )
           DISPLAY "SHORT [" WS-G1 "|" WS-G2 "]"
           COBOL::"EXEC-SQL" ( WS-H "SELECT 'abcdefgh'" WS-N WS-ST )
           MOVE ALL "*" TO WS-REC
           COBOL::"FETCH-ROW" ( WS-H 1 WS-REC WS-ST )
           DISPLAY "NESTED [" WS-A "|" WS-B "|" WS-C "]"
           COBOL::"EXEC-SQL" ( WS-H "SELECT 'AB123'" WS-N WS-ST )
           MOVE ALL "*" TO WS-MIX
           COBOL::"FETCH-ROW" ( WS-H 1 WS-MIX WS-ST )
           ADD 1 TO WS-QTY
           DISPLAY "MIXED [" WS-CODE "|" WS-QTY "]"
           COBOL::"EXEC-SQL" ( WS-H "SELECT 'plain'" WS-N WS-ST )
           MOVE ALL "*" TO WS-ELEM
           COBOL::"FETCH-ROW" ( WS-H 1 WS-ELEM WS-ST )
           DISPLAY "ELEMENTARY [" WS-ELEM "]"
           MOVE ALL "*" TO WS-GRP
           COBOL::"HTTP-GET" ( WS-URL WS-GRP WS-N )
           DISPLAY "HTTP [" WS-G1 "|" WS-G2 "] " WS-N
           STOP RUN.
"#;

#[test]
fn a_builtins_text_reaches_a_groups_subordinate_items() {
    let port = serve("hello, world");
    let src = PROGRAM.replace("@URL@", &format!("http://127.0.0.1:{port}/"));

    let started = Instant::now();
    let lines = run(&src);
    let elapsed = started.elapsed();

    // (case, what the program displays)
    let expected: [(&str, &str); 7] = [
        ("MOVE into a group (unchanged)", "MOVE [hell|o, w]"),
        ("FETCH-ROW into a group", "FETCH [hell|o, w]"),
        ("FETCH-ROW, shorter than the group", "SHORT [hi  |    ]"),
        ("FETCH-ROW into a nested group", "NESTED [abc|de|fgh]"),
        ("FETCH-ROW into a group with a numeric item", "MIXED [AB|124]"),
        ("FETCH-ROW into an elementary item (unchanged)", "ELEMENTARY [plain ]"),
        ("HTTP-GET into a group", "HTTP [hell|o, w] 0200"),
    ];
    let mut passed = 0usize;
    let mut failures = Vec::new();
    for (i, (case, want)) in expected.iter().enumerate() {
        let got = lines.get(i).map(String::as_str).unwrap_or("<nothing>");
        if got == *want {
            passed += 1;
        } else {
            failures.push(format!("{case}: expected {want:?}, got {got:?}"));
        }
    }

    println!("── test_builtin_group_receiver ─────────────────────────────");
    println!("  receivers : 01 group of X(4) X(4); nested 01/05/10 group;");
    println!("              group of XX and 999; elementary X(6)");
    println!("  built-ins : FETCH-ROW (SQLite :memory:), HTTP-GET (local server)");
    for (case, _) in &expected {
        println!("  case      : {case}");
    }
    println!(
        "  run       : {:.1} ms for {} statements' worth of cases",
        elapsed.as_secs_f64() * 1000.0,
        expected.len()
    );
    println!("  result    : {passed} passed, {} failed", failures.len());
    println!("────────────────────────────────────────────────────────────");

    assert!(failures.is_empty(), "{failures:#?}\nall lines: {lines:#?}");
    assert_eq!(lines.len(), expected.len(), "{lines:#?}");
}
