// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! A program that nests `PERFORM` or `CALL` deeper than its thread's stack
//! allows ends with a runtime error, whatever the thread and its stack. It used
//! to overflow the stack, which ends the whole process with no message — the
//! IDE with it, since an overflow cannot be caught. On the program stack every
//! host now gives a program, the limit of 512 `PERFORM` levels is what a
//! program meets first. The command-line half is `cobolt-cli/tests/perform_stack.rs`.

use std::sync::mpsc;
use std::time::Instant;

use cobolt_lexer::{tokenize, SourceFormat};
use cobolt_parser::parse;
use cobolt_runtime::stack_guard::PROGRAM_STACK_BYTES;
use cobolt_runtime::Interpreter;

/// A paragraph that performs itself until `WS-N` reaches `WS-MAX`.
fn perform_deep(max: u32) -> String {
    format!(
        "IDENTIFICATION DIVISION.
PROGRAM-ID. DEEP.
DATA DIVISION.
WORKING-STORAGE SECTION.
01 WS-MAX PIC 9(6) VALUE {max}.
01 WS-N PIC 9(6) VALUE 0.
PROCEDURE DIVISION.
MAIN-PARA.
    PERFORM DIVE
    DISPLAY \"REACHED \" WS-N
    STOP RUN.
DIVE.
    ADD 1 TO WS-N
    IF WS-N < WS-MAX
        PERFORM DIVE
    END-IF.
"
    )
}

/// A nested program that calls itself, with no end in sight.
const CALL_DEEP: &str = "IDENTIFICATION DIVISION.
PROGRAM-ID. OUTER.
DATA DIVISION.
WORKING-STORAGE SECTION.
01 WS-N PIC 9(6) VALUE 0 GLOBAL.
PROCEDURE DIVISION.
MAIN-PARA.
    CALL \"INNER\"
    DISPLAY \"REACHED \" WS-N
    STOP RUN.
IDENTIFICATION DIVISION.
PROGRAM-ID. INNER COMMON.
PROCEDURE DIVISION.
INNER-PARA.
    ADD 1 TO WS-N
    IF WS-N < 999999
        CALL \"INNER\"
    END-IF
    GOBACK.
END PROGRAM INNER.
END PROGRAM OUTER.
";

/// Run `src` on a thread of `stack` bytes: the run's error, if any, and what it displayed.
fn run_on(stack: usize, src: String) -> (Result<(), String>, Vec<String>) {
    std::thread::Builder::new()
        .stack_size(stack)
        .spawn(move || {
            let program = parse(tokenize(&src, SourceFormat::Free)).program.expect("parses");
            let (_event_tx, event_rx) = mpsc::channel();
            let (state_tx, _state_rx) = mpsc::channel();
            let (display_tx, display_rx) = mpsc::channel();
            let mut interp = Interpreter::new_with_channels(program, event_rx, state_tx, display_tx);
            let result = match interp.run() {
                Err(e) if !e.is_exit_signal() => Err(e.to_string()),
                _ => Ok(()),
            };
            drop(interp);
            (result, display_rx.try_iter().map(|l| l.trim().to_string()).collect())
        })
        .expect("spawn")
        .join()
        .expect("the program's thread panicked")
}

#[test]
fn nesting_deeper_than_the_stack_ends_with_a_runtime_error() {
    const SMALL: usize = 4 * 1024 * 1024;
    let mut lines = Vec::new();
    let mut failures = Vec::new();
    let started = Instant::now();
    let mut case = |what: &str, stack: usize, src: String, check: &dyn Fn(&Result<(), String>, &[String]) -> bool| {
        let t = Instant::now();
        let (result, shown) = run_on(stack, src);
        let ms = t.elapsed().as_secs_f64() * 1000.0;
        let outcome = match &result {
            Ok(()) => shown.last().cloned().unwrap_or_default(),
            Err(e) => e.clone(),
        };
        lines.push(format!("  {what} ({} MB stack, {ms:.0} ms): {outcome}", stack >> 20));
        if !check(&result, &shown) {
            failures.push(format!("{what}: {result:?}, shown {shown:?}"));
        }
    };

    // A small stack: the guard stops the recursion before the stack runs out. In
    // an optimised build a level is small enough that the limit of 512 may come first.
    case("PERFORM without end", SMALL, perform_deep(999_999), &|r, _| match r {
        Err(e) if e.contains("PERFORM nested too deeply") => true,
        Err(e) => !cfg!(debug_assertions) && e.contains("PERFORM nesting too deep (max 512)"),
        Ok(()) => false,
    });
    // CALL has no fixed limit: only the guard stands between it and the overflow.
    case("CALL without end", SMALL, CALL_DEEP.to_string(), &|r, _| {
        matches!(r, Err(e) if e.contains("CALL nested too deeply"))
    });
    // The program stack: 500 levels run, and 600 meet the fixed limit of 512 first.
    case("PERFORM 500 deep", PROGRAM_STACK_BYTES, perform_deep(500), &|r, shown| {
        r.is_ok() && shown.last().map(String::as_str) == Some("REACHED 000500")
    });
    case("PERFORM 600 deep", PROGRAM_STACK_BYTES, perform_deep(600), &|r, _| {
        matches!(r, Err(e) if e.contains("PERFORM nesting too deep (max 512)"))
    });
    case("CALL without end", PROGRAM_STACK_BYTES, CALL_DEEP.to_string(), &|r, _| {
        matches!(r, Err(e) if e.contains("CALL nested too deeply"))
    });

    let n = lines.len();
    println!("Stack guard: nested PERFORM and CALL end with a runtime error, never an overflow");
    println!("{}", lines.join("\n"));
    println!("  program stack {} MB; total {:.0} ms", PROGRAM_STACK_BYTES >> 20, started.elapsed().as_secs_f64() * 1000.0);
    println!("  PASS {} FAIL {}", n - failures.len(), failures.len());
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}
