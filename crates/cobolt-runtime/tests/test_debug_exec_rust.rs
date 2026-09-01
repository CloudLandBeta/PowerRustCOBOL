// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! An `EXEC RUST` block is ONE debugger step.
//!
//! Debugging a program containing `EXEC RUST` used to be refused outright, and
//! the refusal was honest as far as it went: the debugger drives an interpreter
//! and a block is native code. What it got wrong was *which process to drive* —
//! the blocks are compiled into the BUILT binary, so that binary is the only
//! process that can execute one, and it is now the debuggee.
//!
//! What this file pins is the stepping contract the IDE promises for it:
//!
//! * the debugger stops on the `EXEC RUST` line — the block is a statement, and
//!   that is where the statement is;
//! * it NEVER stops on a line inside the block. Those lines are not interpreted
//!   at all; in a built binary they were compiled into a native function before
//!   the program ran;
//! * one step over the block lands on the **next COBOL sentence**;
//! * the block really executes while stepping — the whole point of building
//!   first is that the Rust runs for real.

use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use cobolt_lexer::{tokenize, SourceFormat};
use cobolt_parser::{parse, Severity};
use cobolt_runtime::{new_breakpoints, DebugCmd, DebugEvent, Interpreter};

/// One entry per source line, so the test can name lines exactly.
/// 1-based line numbers are the index in this list plus one.
const LINES: &[&str] = &[
    "IDENTIFICATION DIVISION.",   // 1
    "PROGRAM-ID. STEPPING.",      // 2
    "DATA DIVISION.",             // 3
    "WORKING-STORAGE SECTION.",   // 4
    "01 WS-N PIC 9(3) VALUE 0.",  // 5
    "PROCEDURE DIVISION.",        // 6
    "MAIN.",                      // 7
    "    ADD 1 TO WS-N",          // 8   before the block
    "    EXEC RUST",              // 9   <- the statement
    "        let _a = 1;",        // 10  |
    "        let _b = 2;",        // 11  | body — never a stop
    "        let _c = 3;",        // 12  |
    "    END-EXEC",               // 13  <- closes it
    "    ADD 1 TO WS-N",          // 14  the next COBOL sentence
    "    STOP RUN.",              // 15
];

/// The `EXEC RUST` line, and the body lines that must never be stopped on.
const EXEC_LINE: u32 = 9;
const BODY_LINES: [u32; 4] = [10, 11, 12, 13];
const NEXT_SENTENCE: u32 = 14;

/// Proof the compiled block actually ran — a real registered function, exactly
/// as the generated `main.rs` installs one.
///
/// One counter per test, not one shared by both: cargo runs the tests in this
/// file on separate threads, and a single static counted the OTHER test's run
/// too. That reads exactly like a double execution, which is a bug worth
/// catching for real — so the counters are kept apart and each stays honest.
static BLOCK_RAN_STEP_OVER: Mutex<u32> = Mutex::new(0);
static BLOCK_RAN_STEP_IN: Mutex<u32> = Mutex::new(0);

fn the_block(_ctx: &mut cobolt_runtime::exec_rust::ExecRustContext<'_>) {
    *BLOCK_RAN_STEP_OVER.lock().unwrap() += 1;
}

fn the_block_step_in(_ctx: &mut cobolt_runtime::exec_rust::ExecRustContext<'_>) {
    *BLOCK_RAN_STEP_IN.lock().unwrap() += 1;
}

/// Step from start to finish, returning every line the debugger stopped on.
fn stops_while_stepping() -> Vec<u32> {
    let src = LINES.join("\n");
    let result = parse(tokenize(&src, SourceFormat::Free));
    assert!(
        result
            .diagnostics
            .iter()
            .all(|d| d.severity != Severity::Error),
        "parse errors: {:?}",
        result.diagnostics
    );
    let program = result.program.expect("no program");

    let (cmd_tx, cmd_rx) = mpsc::channel::<DebugCmd>();
    let (ev_tx, ev_rx) = mpsc::channel::<DebugEvent>();
    let panic_slot: Arc<Mutex<Option<String>>> = Arc::new(Mutex::new(None));

    let slot = Arc::clone(&panic_slot);
    let handle = thread::spawn(move || {
        let mut interp =
            Interpreter::new_with_debug_channels(program, cmd_rx, ev_tx, new_breakpoints());
        // Exactly what the generated `main.rs` does before the run: a block
        // with nothing registered against it is a hard error, never a no-op.
        interp.register_exec_rust_blocks(|reg| reg.register(0, the_block));
        if let Err(e) = interp.run() {
            if !e.is_exit_signal() {
                *slot.lock().unwrap() = Some(e.to_string());
            }
        }
    });

    let mut stops = Vec::new();
    while let Ok(ev) = ev_rx.recv_timeout(Duration::from_secs(5)) {
        if let DebugEvent::Paused { line, .. } = ev {
            stops.push(line);
            if cmd_tx.send(DebugCmd::StepOver).is_err() {
                break;
            }
        }
    }
    drop(cmd_tx);
    handle.join().expect("interpreter thread panicked");
    if let Some(e) = panic_slot.lock().unwrap().clone() {
        panic!("the program failed under the debugger: {e}");
    }
    stops
}

#[test]
fn a_block_is_one_step_and_never_stops_inside_itself() {
    let stops = stops_while_stepping();

    assert!(
        !stops.is_empty(),
        "the session stopped nowhere at all — that is indistinguishable from a hang"
    );

    // 1. The statement itself IS a stop.
    assert!(
        stops.contains(&EXEC_LINE),
        "the debugger must stop on the EXEC RUST line ({EXEC_LINE}) — the block \
         is a statement and that is where it is. Stops: {stops:?}"
    );

    // 2. Its body never is.
    let inside: Vec<u32> = stops
        .iter()
        .copied()
        .filter(|l| BODY_LINES.contains(l))
        .collect();
    assert!(
        inside.is_empty(),
        "the debugger stopped INSIDE the block at {inside:?}. Those lines are \
         not interpreted — in a built binary they were compiled into a native \
         function before the program ran, so a stop there can never happen and \
         must never be offered. Stops: {stops:?}"
    );

    // 3. One step over the block lands on the next COBOL sentence.
    let after_exec = stops
        .iter()
        .position(|l| *l == EXEC_LINE)
        .map(|i| stops.get(i + 1).copied())
        .expect("EXEC RUST line was asserted present above");
    assert_eq!(
        after_exec,
        Some(NEXT_SENTENCE),
        "one step over the block must land on the next COBOL sentence \
         (line {NEXT_SENTENCE}), not on {after_exec:?}. Stops: {stops:?}"
    );

    // 4. And the block really ran — building first exists so the Rust executes.
    let ran = *BLOCK_RAN_STEP_OVER.lock().unwrap();
    assert_eq!(
        ran, 1,
        "the compiled block must actually execute while stepping; it ran {ran} times"
    );

    println!(
        "\n  EXEC RUST stepping\n\
         \x20   stops:            {stops:?}\n\
         \x20   block statement:  line {EXEC_LINE} (stopped)\n\
         \x20   block body:       lines {BODY_LINES:?} (never stopped, as required)\n\
         \x20   next sentence:    line {NEXT_SENTENCE} (reached in ONE step)\n\
         \x20   block executions: {ran}\n"
    );
}

#[test]
fn step_in_does_not_enter_a_block_either() {
    // StepIn is statement-level, and a block must not become the exception —
    // "no step-in" is the contract the IDE documents.
    let src = LINES.join("\n");
    let program = parse(tokenize(&src, SourceFormat::Free))
        .program
        .expect("no program");

    let (cmd_tx, cmd_rx) = mpsc::channel::<DebugCmd>();
    let (ev_tx, ev_rx) = mpsc::channel::<DebugEvent>();
    let handle = thread::spawn(move || {
        let mut interp =
            Interpreter::new_with_debug_channels(program, cmd_rx, ev_tx, new_breakpoints());
        interp.register_exec_rust_blocks(|reg| reg.register(0, the_block_step_in));
        let _ = interp.run();
    });

    let mut stops = Vec::new();
    while let Ok(ev) = ev_rx.recv_timeout(Duration::from_secs(5)) {
        if let DebugEvent::Paused { line, .. } = ev {
            stops.push(line);
            if cmd_tx.send(DebugCmd::StepIn).is_err() {
                break;
            }
        }
    }
    drop(cmd_tx);
    handle.join().expect("interpreter thread panicked");

    let inside: Vec<u32> = stops
        .iter()
        .copied()
        .filter(|l| BODY_LINES.contains(l))
        .collect();
    assert!(
        inside.is_empty(),
        "StepIn stepped INTO the block at {inside:?} — there is nothing there to \
         step into. Stops: {stops:?}"
    );
    println!("  StepIn stops: {stops:?} — body never entered");
}
