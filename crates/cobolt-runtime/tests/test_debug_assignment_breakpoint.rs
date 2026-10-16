// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Spec 093 R14 / AC6 — a breakpoint set on the assignment shorthand, `A = B * C`, stops the debuggee
//! there, as it does on a `COMPUTE`: the statement the parser builds from the shorthand carries its own
//! line, so the debugger needs nothing but the gutter rule (which the IDE tests pin).

use std::sync::mpsc;
use std::thread;
use std::time::Duration;

use cobolt_lexer::{tokenize, SourceFormat};
use cobolt_parser::{parse, Severity};
use cobolt_runtime::{new_breakpoints, DebugCmd, DebugEvent, Interpreter};

/// The loop body, line 10, is written once with the verb and once without.
fn lines(body: &str) -> Vec<String> {
    [
        "IDENTIFICATION DIVISION.",
        "PROGRAM-ID. BPTEST.",
        "DATA DIVISION.",
        "WORKING-STORAGE SECTION.",
        "01 WS-I PIC 9(3) VALUE 0.",
        "01 WS-T PIC 9(4) VALUE 0.",
        "PROCEDURE DIVISION.",
        "MAIN.",
        "    PERFORM VARYING WS-I FROM 1 BY 1 UNTIL WS-I > 10",
        body,
        "    END-PERFORM",
        "    STOP RUN.",
    ]
    .iter()
    .map(|s| (*s).to_owned())
    .collect()
}

/// The value of `WS-I` at every stop on line 10, continuing each time.
fn stops(body: &str) -> Vec<String> {
    let src = lines(body).join("\n");
    let result = parse(tokenize(&src, SourceFormat::Free));
    assert!(result.diagnostics.iter().all(|d| d.severity != Severity::Error), "parse errors: {:?}", result.diagnostics);
    let program = result.program.expect("a program");
    let (cmd_tx, cmd_rx) = mpsc::channel::<DebugCmd>();
    let (ev_tx, ev_rx) = mpsc::channel::<DebugEvent>();
    let bps = new_breakpoints();
    bps.lock().unwrap().insert(10);
    let handle = thread::spawn(move || {
        let mut interp = Interpreter::new_with_debug_channels(program, cmd_rx, ev_tx, bps);
        let _ = interp.run();
    });
    let (mut seen, mut first) = (Vec::new(), true);
    loop {
        match ev_rx.recv_timeout(Duration::from_secs(10)) {
            Ok(DebugEvent::Paused { vars, .. }) => {
                if first {
                    first = false; // the session starts stopped at the first statement
                } else {
                    seen.push(vars.iter().find(|v| v.name == "WS-I").map(|v| v.value.trim_start_matches('0').to_owned()).unwrap_or_default());
                }
                if cmd_tx.send(DebugCmd::Continue).is_err() {
                    break;
                }
            }
            Ok(DebugEvent::Finished) | Err(_) => break,
            Ok(_) => {}
        }
    }
    drop(cmd_tx);
    let _ = handle.join();
    seen
}

#[test]
fn a_breakpoint_on_the_shorthand_stops_on_every_pass_as_it_does_on_compute() {
    let shorthand = stops("        WS-T = WS-T + WS-I");
    let verbose = stops("        COMPUTE WS-T = WS-T + WS-I");
    assert_eq!(shorthand.len(), 10, "ten passes through line 10: {shorthand:?}");
    assert_eq!(shorthand, verbose, "the two spellings stop at the same places");
    assert_eq!(shorthand, ["1", "2", "3", "4", "5", "6", "7", "8", "9", "10"]);
}
