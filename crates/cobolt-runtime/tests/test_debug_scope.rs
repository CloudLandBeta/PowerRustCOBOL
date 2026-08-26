// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! "Only my code" — stepping crosses IDE-generated scaffolding.
//!
//! A form's generated `.cbl` is mostly plumbing the developer never wrote: the
//! `COBOL-EVENT-LOOP` above all. Single-stepping through it puts dozens of
//! statements between the developer and their own handler, which is what the
//! operator meant by *"do not animate the cobol event loop — this is an
//! internal construct"* (2026-08-24).
//!
//! The scope filters **stepping** only. A breakpoint is honoured wherever it
//! sits, because setting one is a deliberate act.

use std::sync::mpsc;
use std::thread;
use std::time::Duration;

use cobolt_lexer::{tokenize, SourceFormat};
use cobolt_parser::{parse, Severity};
use cobolt_runtime::{new_breakpoints, new_user_scope, DebugCmd, DebugEvent, Interpreter};

/// The program, one entry per source line so a test can name lines exactly.
/// 1-based line numbers are the index in this list plus one.
const LINES: &[&str] = &[
    "IDENTIFICATION DIVISION.",       // 1
    "PROGRAM-ID. SCOPE.",             // 2
    "DATA DIVISION.",                 // 3
    "WORKING-STORAGE SECTION.",       // 4
    "01 WS-N PIC 9(3) VALUE 0.",      // 5
    "PROCEDURE DIVISION.",            // 6
    "MAIN.",                          // 7
    "    ADD 1 TO WS-N",              // 8   scaffolding
    "    ADD 1 TO WS-N",              // 9   scaffolding
    "    ADD 1 TO WS-N",              // 10  USER
    "    ADD 1 TO WS-N",              // 11  scaffolding
    "    ADD 1 TO WS-N",              // 12  USER
    "    DISPLAY WS-N",               // 13  scaffolding
    "    STOP RUN.",                  // 14  scaffolding
];

/// Lines 10 and 12 stand in for the developer's handler bodies; everything else
/// stands in for what the IDE generated around them.
const USER_LINES: [u32; 2] = [10, 12];

/// Run to completion under the debugger, stepping at every pause, and return
/// the lines it actually stopped on.
fn stops_while_stepping(user_only: bool) -> Vec<u32> {
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
    let scope = new_user_scope();
    {
        let mut g = scope.lock().unwrap();
        g.user_only = user_only;
        g.user_lines = USER_LINES.into_iter().collect();
    }

    let handle = thread::spawn(move || {
        let mut interp =
            Interpreter::new_with_debug_channels(program, cmd_rx, ev_tx, new_breakpoints());
        interp.set_debug_user_scope(scope);
        let _ = interp.run();
    });

    let mut stops = Vec::new();
    while let Ok(ev) = ev_rx.recv_timeout(Duration::from_secs(5)) {
        if let DebugEvent::Paused { line, .. } = ev {
            stops.push(line);
            // Keep stepping; a closed channel just ends the loop.
            if cmd_tx.send(DebugCmd::StepOver).is_err() {
                break;
            }
        }
    }
    drop(cmd_tx);
    handle.join().expect("interpreter thread panicked");
    stops
}

/// With the scope on, stepping stops **only** on the developer's lines.
#[test]
fn stepping_crosses_generated_scaffolding() {
    let stops = stops_while_stepping(true);

    assert!(
        !stops.is_empty(),
        "the session stopped nowhere at all — an empty scope must never mean \
         `stop nowhere`, or a debug session looks like a hang"
    );
    let strays: Vec<u32> = stops
        .iter()
        .copied()
        .filter(|l| !USER_LINES.contains(l))
        .collect();
    assert!(
        strays.is_empty(),
        "stepping stopped on generated lines {strays:?}; stops were {stops:?}"
    );

    // Both user lines are reached — the loop is *crossed*, not disarmed: after
    // running through scaffolding, stepping is still armed for the next one.
    for want in USER_LINES {
        assert!(
            stops.contains(&want),
            "line {want} is the developer's code and was never reached: {stops:?}"
        );
    }

    println!(
        "user scope ON — stopped on {stops:?}; \
         {} generated lines crossed without a pause",
        LINES.len() - USER_LINES.len()
    );
}

/// With the scope off, stepping behaves exactly as it always did — every
/// statement, scaffolding included. The toggle is the only difference.
#[test]
fn stepping_without_the_scope_still_visits_everything() {
    let stops = stops_while_stepping(false);

    let generated: Vec<u32> = stops
        .iter()
        .copied()
        .filter(|l| !USER_LINES.contains(l))
        .collect();
    assert!(
        !generated.is_empty(),
        "with the scope off, stepping must still stop on generated lines: {stops:?}"
    );
    assert!(
        stops.len() > USER_LINES.len(),
        "expected more stops than user lines, got {stops:?}"
    );

    println!("user scope OFF — stopped on {stops:?} (every statement, as before)");
}
