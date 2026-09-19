// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Pause reaches a form that is idle in `COBOL-WAIT-EVENT`.
//!
//! The interpreter used to notice a Pause only at the top of the next
//! statement — which an idle form never reaches, so pressing Pause did nothing
//! until the developer clicked something on the form (operator, 2026-09-19).
//! Now the wait itself polls for it and stops the program at the **last
//! executed line**: the wait's own CALL when everything is in scope, and the
//! developer's last handler line when "only my code" is on — never the event
//! loop the developer did not write.

use std::sync::mpsc;
use std::thread;
use std::time::Duration;

use cobolt_lexer::{tokenize, SourceFormat};
use cobolt_parser::{parse, Severity};
use cobolt_runtime::{
    new_breakpoints, new_user_scope, DebugCmd, DebugEvent, FormEvent, Interpreter, StopReason,
};

const LINES: &[&str] = &[
    "IDENTIFICATION DIVISION.",                                            // 1
    "PROGRAM-ID. PAUSEWAIT.",                                              // 2
    "DATA DIVISION.",                                                      // 3
    "WORKING-STORAGE SECTION.",                                            // 4
    "01 COBOL-QUIT PIC X VALUE \"0\".",                                    // 5
    "01 COBOL-EVENT-ID PIC X(30).",                                        // 6
    "01 COBOL-CONTROL-ID PIC X(30).",                                      // 7
    "01 WS-X PIC 9 VALUE 0.",                                              // 8
    "PROCEDURE DIVISION.",                                                 // 9
    "MAIN.",                                                               // 10
    "    MOVE 1 TO WS-X",                                                  // 11 <- the developer's line
    "    PERFORM UNTIL COBOL-QUIT = \"1\"",                                // 12
    "        CALL \"COBOL-WAIT-EVENT\" USING COBOL-EVENT-ID COBOL-CONTROL-ID", // 13 <- the wait
    "    END-PERFORM",                                                     // 14
    "    STOP RUN.",                                                       // 15
];
const USER_LINE: u32 = 11;
const WAIT_LINE: u32 = 13;

/// Run the program under the debugger with a live (but silent) form channel,
/// let it reach the wait, press Pause, and return the line the stop reports.
fn pause_line(user_only: bool) -> u32 {
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

    // The form's event channel stays OPEN and silent: the program blocks in
    // the wait exactly as an idle form does.
    let (event_tx, event_rx) = mpsc::channel::<FormEvent>();
    let (state_tx, _state_rx) = mpsc::channel();
    let (display_tx, _display_rx) = mpsc::channel();
    let (cmd_tx, cmd_rx) = mpsc::channel::<DebugCmd>();
    let (ev_tx, ev_rx) = mpsc::channel::<DebugEvent>();
    let bps = new_breakpoints();
    let scope = new_user_scope();
    if user_only {
        let mut s = scope.lock().unwrap();
        s.user_only = true;
        s.user_lines = [USER_LINE].into_iter().collect();
    }

    let handle = thread::spawn(move || {
        let mut interp = Interpreter::new_with_channels(program, event_rx, state_tx, display_tx);
        interp.attach_debug_channels(cmd_rx, ev_tx, bps);
        interp.set_debug_user_scope(scope);
        let _ = interp.run();
    });

    // The session starts stopped at the first statement: continue past it.
    loop {
        match ev_rx.recv_timeout(Duration::from_secs(10)) {
            Ok(DebugEvent::Paused { .. }) => {
                cmd_tx.send(DebugCmd::Continue).expect("send Continue");
                break;
            }
            Ok(_) => {}
            Err(e) => panic!("no initial stop: {e:?}"),
        }
    }
    // Two statements later the program is parked in the wait. Give it far
    // longer than that, so the Pause lands on an idle program and not on a
    // statement still in flight.
    thread::sleep(Duration::from_millis(500));
    cmd_tx.send(DebugCmd::Pause).expect("send Pause");

    let mut line = None;
    loop {
        match ev_rx.recv_timeout(Duration::from_secs(10)) {
            Ok(DebugEvent::Stopped {
                line: at,
                reason: StopReason::Pause,
                ..
            }) => {
                line = Some(at);
                break;
            }
            Ok(_) => {}
            Err(e) => panic!("Pause produced no stop while the form was idle: {e:?}"),
        }
    }
    // End the session; the program leaves the wait the way a closed window
    // ends it.
    let _ = cmd_tx.send(DebugCmd::Terminate);
    drop(cmd_tx);
    drop(event_tx);
    let _ = handle.join();
    line.expect("a Pause stop")
}

#[test]
fn pause_on_an_idle_form_stops_at_the_wait_when_everything_is_in_scope() {
    assert_eq!(pause_line(false), WAIT_LINE);
}

#[test]
fn pause_on_an_idle_form_stops_at_the_developers_last_line_under_only_my_code() {
    assert_eq!(pause_line(true), USER_LINE);
}
