// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors

//! Spec 109 T-IDE: stepping into a user-defined function (`FUNCTION-ID`).
//!
//! A call written inside an expression, `COMPUTE X = FUNCTION DOUBLE-AMOUNT(Y)`, is one source line in
//! the caller; the debugger must stop on the function's own lines, show the function as a frame above
//! the caller's line, answer the function's arguments, and come back to the caller.

use std::sync::mpsc;
use std::thread;
use std::time::Duration;

use cobolt_lexer::{tokenize, SourceFormat};
use cobolt_parser::parse;
use cobolt_runtime::debug_session::{DebugFrame, FrameKind};
use cobolt_runtime::{new_breakpoints, DebugAnswer, DebugCmd, DebugEvent, DebugQuery, Interpreter};

/// Line 12 is the call; lines 22-23 are the function's body.
const SRC: &str = "\
IDENTIFICATION DIVISION.
PROGRAM-ID. MAIN.
DATA DIVISION.
WORKING-STORAGE SECTION.
01 WS-PRICE PIC S9(5)V99 VALUE 21.50.
01 WS-TOTAL PIC S9(5)V99.
PROCEDURE DIVISION.
MAIN-PARA.
    MOVE 0 TO WS-TOTAL
    DISPLAY WS-PRICE
    ADD 1 TO WS-TOTAL
    COMPUTE WS-TOTAL = FUNCTION DOUBLE-AMOUNT(WS-PRICE)
    DISPLAY WS-TOTAL
    STOP RUN.

IDENTIFICATION DIVISION.
FUNCTION-ID. DOUBLE-AMOUNT.
DATA DIVISION.
LINKAGE SECTION.
01 ARG-AMOUNT PIC S9(5)V99.
01 RESULT-AMOUNT PIC S9(5)V99.
PROCEDURE DIVISION USING BY VALUE ARG-AMOUNT RETURNING RESULT-AMOUNT.
    COMPUTE RESULT-AMOUNT = ARG-AMOUNT * 2
    GOBACK.
END FUNCTION DOUBLE-AMOUNT.
";
const CALL_LINE: u32 = 12;
const BODY: std::ops::RangeInclusive<u32> = 23..=24;

struct Stop {
    line: u32,
    frames: Vec<DebugFrame>,
}

struct Session {
    cmd: mpsc::Sender<DebugCmd>,
    ev: mpsc::Receiver<DebugEvent>,
    handle: Option<thread::JoinHandle<()>>,
}

impl Session {
    fn start() -> (Self, Stop) {
        let program = parse(tokenize(SRC, SourceFormat::Free)).program.expect("no program");
        let (cmd_tx, cmd_rx) = mpsc::channel::<DebugCmd>();
        let (ev_tx, ev_rx) = mpsc::channel::<DebugEvent>();
        let handle = thread::spawn(move || {
            let mut interp = Interpreter::new_with_debug_channels(program, cmd_rx, ev_tx, new_breakpoints());
            let _ = interp.run();
        });
        let mut s = Session { cmd: cmd_tx, ev: ev_rx, handle: Some(handle) };
        let first = s.wait_stopped();
        (s, first)
    }

    fn wait_stopped(&mut self) -> Stop {
        for _ in 0..200 {
            match self.ev.recv_timeout(Duration::from_secs(5)) {
                Ok(DebugEvent::Stopped { line, frames, .. }) => return Stop { line, frames },
                Ok(_) => continue,
                Err(e) => panic!("no stop: {e}"),
            }
        }
        panic!("no stop after 200 events");
    }

    fn step_in(&mut self) -> Stop {
        self.cmd.send(DebugCmd::StepIn).unwrap();
        self.wait_stopped()
    }

    fn evaluate(&mut self, expr: &str) -> DebugAnswer {
        self.cmd.send(DebugCmd::Query { id: 7, query: DebugQuery::Evaluate { frame: 0, expression: expr.into() } }).unwrap();
        for _ in 0..200 {
            match self.ev.recv_timeout(Duration::from_secs(5)) {
                Ok(DebugEvent::Answer { id: 7, answer }) => return answer,
                Ok(_) => continue,
                Err(e) => panic!("no answer: {e}"),
            }
        }
        panic!("no answer");
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

#[test]
fn a_step_into_the_call_stops_inside_the_function_and_comes_back() {
    let (mut s, mut stop) = Session::start();
    let mut trail = vec![stop.line];
    // Walk to the call line, then one step in.
    for _ in 0..10 {
        if stop.line == CALL_LINE {
            break;
        }
        stop = s.step_in();
        trail.push(stop.line);
    }
    assert_eq!(stop.line, CALL_LINE, "never reached the call: {trail:?}");
    let mut inside = None;
    for _ in 0..6 {
        stop = s.step_in();
        trail.push(stop.line);
        if BODY.contains(&stop.line) {
            inside = Some(Stop { line: stop.line, frames: stop.frames.clone() });
            break;
        }
    }
    let inside = inside.unwrap_or_else(|| panic!("never stopped on a line of the function body: {trail:?}"));
    // The function is a frame of its own, above the caller's.
    assert!(inside.frames.len() >= 2, "{:?}", inside.frames);
    let top = &inside.frames[0];
    assert!(top.program.eq_ignore_ascii_case("DOUBLE-AMOUNT"), "top frame is {top:?}");
    assert_eq!(top.kind, FrameKind::Call);
    // The caller's frame still points at the call line.
    assert!(inside.frames.iter().any(|f| f.line == CALL_LINE), "{:?}", inside.frames);
    // The argument arrived by value: the function sees 21.50.
    match s.evaluate("ARG-AMOUNT") {
        DebugAnswer::Evaluated { result, .. } => assert!(result.contains("21.5") || result.contains("2150"), "{result}"),
        other => panic!("{other:?}"),
    }
    // …and stepping on leaves the function for the caller's next line.
    let mut back = false;
    for _ in 0..8 {
        stop = s.step_in();
        if stop.frames.first().is_some_and(|f| f.program.eq_ignore_ascii_case("MAIN")) && stop.line > CALL_LINE - 1 {
            back = true;
            break;
        }
    }
    assert!(back, "never returned to the caller");
}
