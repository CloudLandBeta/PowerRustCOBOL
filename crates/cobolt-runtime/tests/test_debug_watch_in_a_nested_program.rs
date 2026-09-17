// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors

//! Watching a data item the **nested program** declares.
//!
//! Every RAD event handler is a nested program with its own DATA DIVISION, so
//! the items a handler works with are usually its own — not the form's. The
//! operator stopped inside `RADIOBUTTONS-FORM--ONLOAD`, on a line whose
//! neighbour reads `WS-LINE`, added `WS-LINE` as a watch, and was told
//! *"WS-LINE is not a data item in this frame"* (2026-09-17).
//!
//! This reproduces that shape: a nested program with its own WORKING-STORAGE,
//! stopped inside it, asked about one of its own items.

use std::sync::mpsc;
use std::thread;
use std::time::Duration;

use cobolt_lexer::{tokenize, SourceFormat};
use cobolt_parser::parse;
use cobolt_runtime::{
    new_breakpoints, DebugAnswer, DebugCmd, DebugEvent, DebugQuery, Interpreter,
};

/// A form-shaped program: an outer one with its own storage, and a nested
/// handler declaring `WS-LINE` — exactly how a generated handler is built.
const SRC: &str = "\
IDENTIFICATION DIVISION.
PROGRAM-ID. OUTER.
DATA DIVISION.
WORKING-STORAGE SECTION.
01 WS-OUTER       PIC X(4) VALUE \"HOST\".
PROCEDURE DIVISION.
MAIN.
    CALL \"HANDLER\"
    STOP RUN.

IDENTIFICATION DIVISION.
PROGRAM-ID. HANDLER IS COMMON PROGRAM.
DATA DIVISION.
WORKING-STORAGE SECTION.
01 WS-LINE        PIC X(20) VALUE \"the handler's own\".
01 WS-NL          PIC X VALUE \"N\".
PROCEDURE DIVISION.
    MOVE \"written here\" TO WS-LINE
    DISPLAY WS-LINE
    GOBACK.
END PROGRAM HANDLER.
END PROGRAM OUTER.
";

struct Session {
    cmd: mpsc::Sender<DebugCmd>,
    ev: mpsc::Receiver<DebugEvent>,
    next_id: u64,
    handle: Option<thread::JoinHandle<()>>,
}

impl Session {
    fn start() -> Self {
        let result = parse(tokenize(SRC, SourceFormat::Free));
        let program = result.program.expect("no program");
        let (cmd_tx, cmd_rx) = mpsc::channel::<DebugCmd>();
        let (ev_tx, ev_rx) = mpsc::channel::<DebugEvent>();
        let handle = thread::spawn(move || {
            let mut interp =
                Interpreter::new_with_debug_channels(program, cmd_rx, ev_tx, new_breakpoints());
            let _ = interp.run();
        });
        let mut s = Session {
            cmd: cmd_tx,
            ev: ev_rx,
            next_id: 1,
            handle: Some(handle),
        };
        s.wait_stopped();
        s
    }

    /// Drain until the interpreter reports a stop; hand back its paragraph.
    fn wait_stopped(&mut self) -> String {
        for _ in 0..200 {
            match self.ev.recv_timeout(Duration::from_secs(5)) {
                Ok(DebugEvent::Stopped { paragraph, .. }) => return paragraph,
                Ok(_) => continue,
                Err(e) => panic!("no stop: {e}"),
            }
        }
        panic!("no stop after 200 events");
    }

    /// Step until the program is stopped INSIDE the nested handler.
    ///
    /// A generated handler has no paragraph of its own, so its statements run
    /// in the implicit one — which is exactly what the operator's breadcrumb
    /// read: `RADIOBUTTONS-FORM--ONLOAD › <IMPLICIT>`.
    fn step_into_handler(&mut self) {
        for _ in 0..20 {
            self.cmd.send(DebugCmd::StepIn).unwrap();
            if self.wait_stopped() == "<IMPLICIT>" {
                return;
            }
        }
        panic!("never stepped into the handler");
    }

    fn ask(&mut self, query: DebugQuery) -> DebugAnswer {
        let id = self.next_id;
        self.next_id += 1;
        self.cmd.send(DebugCmd::Query { id, query }).unwrap();
        for _ in 0..200 {
            match self.ev.recv_timeout(Duration::from_secs(5)) {
                Ok(DebugEvent::Answer { id: got, answer }) if got == id => return answer,
                Ok(_) => continue,
                Err(e) => panic!("no answer to {id}: {e}"),
            }
        }
        panic!("no answer to {id}");
    }

    fn evaluate(&mut self, expr: &str) -> DebugAnswer {
        self.ask(DebugQuery::Evaluate {
            frame: 0,
            expression: expr.into(),
        })
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        let _ = self.cmd.send(DebugCmd::Terminate);
        if let Some(h) = self.handle.take() {
            let _ = h.join();
        }
    }
}

/// Stopped inside the handler, its own `WS-LINE` must answer — that is the
/// whole of the operator's report.
#[test]
fn a_watch_on_a_nested_programs_own_item_answers_while_stopped_inside_it() {
    let mut s = Session::start();
    s.step_into_handler();

    match s.evaluate("WS-LINE") {
        DebugAnswer::Evaluated { result, .. } => {
            assert!(
                !result.trim().is_empty(),
                "WS-LINE answered with nothing: {result:?}"
            );
        }
        other => panic!(
            "the handler's own WS-LINE was refused while stopped inside it: {other:?}"
        ),
    }
}

/// …and the OUTER program's storage is still reachable from in there, because a
/// nested program can see it.
#[test]
fn the_outer_programs_storage_is_still_visible_from_the_handler() {
    let mut s = Session::start();
    s.step_into_handler();

    match s.evaluate("WS-OUTER") {
        DebugAnswer::Evaluated { result, .. } => {
            assert_eq!(result.trim(), "HOST", "WS-OUTER read back wrong");
        }
        other => panic!("the outer program's WS-OUTER was refused: {other:?}"),
    }
}

/// A name that really is not there still says so, rather than evaluating to
/// zero — the reason the bare-name branch exists at all.
#[test]
fn a_genuinely_unknown_name_is_still_refused() {
    let mut s = Session::start();
    s.step_into_handler();

    match s.evaluate("WS-NOT-DECLARED-ANYWHERE") {
        DebugAnswer::Error(msg) => {
            assert!(
                msg.contains("not a data item"),
                "unexpected refusal: {msg}"
            );
        }
        other => panic!("an unknown name must be refused, got {other:?}"),
    }
}
