// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Cooperative cancellation: a looping / long-running handler (e.g. a Timer
//! tick) must abort promptly when the host raises the cancel flag, so the UI
//! thread can never hang on close, relaunch, or exit. `Cancelled` is a clean
//! exit (`run()` returns `Ok`), not a fault.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc};
use std::thread;
use std::time::{Duration, Instant};

use cobolt_lexer::{tokenize, SourceFormat};
use cobolt_parser::{parse, Severity};
use cobolt_runtime::Interpreter;

macro_rules! parse_program {
    ($src:expr) => {{
        let result = parse(tokenize($src, SourceFormat::Free));
        assert!(
            result
                .diagnostics
                .iter()
                .all(|d| d.severity != Severity::Error),
            "parse errors: {:?}",
            result.diagnostics
        );
        result.program.expect("no program")
    }};
}

/// A program that spins forever unless something aborts it.
const SPIN_FOREVER: &str = r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. SPIN.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 DONE PIC 9 VALUE 0.
       01 N    PIC 9(9) COMP-5 VALUE 0.
       PROCEDURE DIVISION.
       MAIN.
           PERFORM UNTIL DONE = 1
               ADD 1 TO N
           END-PERFORM.
           STOP RUN.
"#;

#[test]
fn cancel_flag_aborts_infinite_loop_and_returns_ok() {
    let program = parse_program!(SPIN_FOREVER);
    let (_event_tx, event_rx) = mpsc::channel();
    let (state_tx, _state_rx) = mpsc::channel();
    let (display_tx, _display_rx) = mpsc::channel();

    let cancel = Arc::new(AtomicBool::new(false));
    let cancel_thread = Arc::clone(&cancel);

    // `Interpreter` is constructed inside the thread (like `FormRuntime` does),
    // since it is not `Send`; the parsed program and channels move in.
    let handle = thread::spawn(move || {
        let mut interp = Interpreter::new_with_channels(program, event_rx, state_tx, display_tx);
        interp.set_cancel_flag(cancel_thread);
        interp.run()
    });

    // Let it spin, then request cancellation.
    thread::sleep(Duration::from_millis(50));
    cancel.store(true, Ordering::Relaxed);

    // It must stop promptly — poll rather than block forever if it doesn't.
    let start = Instant::now();
    while !handle.is_finished() {
        assert!(
            start.elapsed() < Duration::from_secs(5),
            "interpreter did not honour the cancel flag within 5s"
        );
        thread::sleep(Duration::from_millis(5));
    }

    let result = handle.join().expect("interpreter thread panicked");
    assert!(
        result.is_ok(),
        "cancellation must be a clean exit, got {result:?}"
    );
}

#[test]
fn no_cancel_flag_runs_normally() {
    // Same machinery, but the loop terminates on its own — proving the cancel
    // check between statements does not disturb a normal run.
    let program = parse_program!(
        r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. LOOP-DEMO.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 N PIC 9(4) VALUE 0.
       PROCEDURE DIVISION.
       MAIN.
           PERFORM UNTIL N = 100
               ADD 1 TO N
           END-PERFORM.
           DISPLAY N.
           STOP RUN.
"#
    );
    let (_event_tx, event_rx) = mpsc::channel();
    let (state_tx, _state_rx) = mpsc::channel();
    let (display_tx, display_rx) = mpsc::channel();
    let mut interp = Interpreter::new_with_channels(program, event_rx, state_tx, display_tx);
    // A flag that is never set must not affect a normal run.
    interp.set_cancel_flag(Arc::new(AtomicBool::new(false)));
    interp.run().expect("run failed");
    let out: Vec<String> = display_rx.try_iter().map(|s| s.trim().to_owned()).collect();
    assert!(
        out.iter().any(|l| l == "0100" || l == "100"),
        "expected final count in output, got {out:?}"
    );
}
