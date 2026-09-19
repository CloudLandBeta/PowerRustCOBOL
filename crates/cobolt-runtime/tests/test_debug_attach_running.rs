// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! A debuggee can join a session **already running** (spec 061).
//!
//! `attach_debug_channels` starts a program paused at its first statement,
//! which is right for the one the developer pressed Debug on. It is wrong for
//! a form the application opens while the session is live: that program should
//! run and stop only where the developer put a breakpoint. Starting it paused
//! would halt the whole application every time any form opened, at a line
//! nobody asked about.
//!
//! The two are measured against the same program, so the only variable is
//! which `attach` was used.

use std::sync::mpsc;
use std::thread;
use std::time::Duration;

use cobolt_lexer::{tokenize, SourceFormat};
use cobolt_parser::{parse, Severity};
use cobolt_runtime::{new_breakpoints, DebugCmd, DebugEvent, Interpreter};

/// Three statements, so "stopped at the first" is distinguishable from
/// "stopped at the breakpoint on line 11".
const LINES: &[&str] = &[
    "IDENTIFICATION DIVISION.",   // 1
    "PROGRAM-ID. ATTACHRUN.",     // 2
    "DATA DIVISION.",             // 3
    "WORKING-STORAGE SECTION.",   // 4
    "01 WS-A PIC 9 VALUE 0.",     // 5
    "01 WS-B PIC 9 VALUE 0.",     // 6
    "01 WS-C PIC 9 VALUE 0.",     // 7
    "PROCEDURE DIVISION.",        // 8
    "MAIN.",                      // 9
    "    MOVE 1 TO WS-A",         // 10
    "    MOVE 2 TO WS-B",         // 11  <- the breakpoint
    "    MOVE 3 TO WS-C",         // 12
    "    STOP RUN.",              // 13
];
const BP_LINE: u32 = 11;

/// Run the program with the chosen attach, answering every stop with
/// `Continue`. Returns the line of each stop, in order.
fn stops(running: bool, breakpoint: Option<u32>) -> Vec<u32> {
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
    let bps = new_breakpoints();
    if let Some(line) = breakpoint {
        bps.lock().unwrap().insert(line);
    }

    let handle = thread::spawn(move || {
        let mut interp = Interpreter::new(program);
        if running {
            interp.attach_debug_channels_running(cmd_rx, ev_tx, bps);
        } else {
            interp.attach_debug_channels(cmd_rx, ev_tx, bps);
        }
        let _ = interp.run();
    });

    let mut seen = Vec::new();
    loop {
        match ev_rx.recv_timeout(Duration::from_secs(10)) {
            Ok(DebugEvent::Stopped { line, .. }) => {
                seen.push(line);
                if cmd_tx.send(DebugCmd::Continue).is_err() {
                    break;
                }
            }
            Ok(DebugEvent::Finished) => break,
            Ok(_) => {}
            // The program ended and dropped its sender.
            Err(_) => break,
        }
    }
    drop(cmd_tx);
    let _ = handle.join();
    seen
}

/// The form the developer pressed Debug on: stopped before anything runs.
#[test]
fn a_plain_attach_starts_the_program_paused() {
    let seen = stops(false, None);
    assert_eq!(
        seen.first().copied(),
        Some(10),
        "a plain attach stops at the first statement, got {seen:?}"
    );
}

/// A form opened while the session is live: it runs, and nothing stops it.
#[test]
fn a_running_attach_stops_nowhere_without_a_breakpoint() {
    assert_eq!(
        stops(true, None),
        Vec::<u32>::new(),
        "a form that joins a live session must not stop at its own first \
         statement — nothing asked it to"
    );
}

/// …and it still honours the developer's breakpoint, which is the whole
/// point of attaching it at all.
#[test]
fn a_running_attach_still_stops_at_a_breakpoint() {
    assert_eq!(
        stops(true, Some(BP_LINE)),
        vec![BP_LINE],
        "the breakpoint is reached, exactly once, and nothing else stops"
    );
}
