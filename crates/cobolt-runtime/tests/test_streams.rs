// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! WASM-014 T14 (R14, AC8): a form subscribes to a stream program from COBOL, receives `onStreamItem` for each item and
//! `onStreamEnd` for the end, reads the item with `COBOL::"WS-STREAM-ITEM"`, sees the restart flag on the first item after a
//! re-subscription, and leaves nothing behind on the backend when it closes.

use std::collections::VecDeque;
use std::sync::{mpsc, Arc, Mutex};
use std::time::Instant;

use cobolt_lexer::{tokenize, SourceFormat};
use cobolt_parser::parse;
use cobolt_runtime::Interpreter;
use cobolt_wire::call::{CallReply, CallRequest, StreamNote, Transport};
use cobolt_wire::error::{codes, CallError, ErrorClass};

#[derive(Default)]
struct Wire {
    /// What the backend will say, in order.
    notes: VecDeque<StreamNote>,
    /// The subscriptions started (program, the JSON of each argument) and ended.
    started: Vec<(String, Vec<String>)>,
    ended: Vec<u64>,
    refuse: bool,
}

struct Rig(Arc<Mutex<Wire>>);

impl Transport for Rig {
    fn call(&mut self, _r: &CallRequest) -> Result<CallReply, CallError> {
        Err(CallError::transport(codes::NOT_CONFIGURED, "no calls here"))
    }
    fn subscribe(&mut self, r: &CallRequest) -> Result<u64, CallError> {
        let mut w = self.0.lock().unwrap();
        if w.refuse {
            return Err(CallError::new(ErrorClass::Application, codes::UNKNOWN_PROGRAM, format!("no program {}", r.program)));
        }
        w.started.push((r.program.clone(), r.args.iter().map(|a| a.json.clone()).collect()));
        Ok(7)
    }
    fn unsubscribe(&mut self, sub: u64) {
        self.0.lock().unwrap().ended.push(sub);
    }
    fn next_item(&mut self, _sub: u64) -> Option<StreamNote> {
        self.0.lock().unwrap().notes.pop_front()
    }
}

fn item(seq: u64, n: u32) -> StreamNote {
    StreamNote::Item { seq, json: format!("{{\"N\":{n},\"WHAT\":\"tick\"}}") }
}

/// A form's event loop that subscribes, shows each stream event, and quits at the end.
fn program(subscribe: &str, on_end: &str) -> cobolt_ast::program::Program {
    let src = format!(
        r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. STREAMS.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 COBOL-EVENT-ID    PIC X(30).
       01 COBOL-CONTROL-ID  PIC X(30).
       01 COBOL-QUIT        PIC 9 VALUE 0.
       01 WS-SUB            PIC X(10).
       01 WS-ST             PIC X(10).
       01 WS-RESTART        PIC X(1).
       01 WS-REASON         PIC X(20).
       01 WS-LIMIT.
          05 WS-EVERY       PIC 9(4) VALUE 30.
       01 TICK.
          05 N              PIC 9(4).
          05 WHAT           PIC X(8).
       PROCEDURE DIVISION.
       MAIN.
{subscribe}
           DISPLAY "subscribed " WS-ST
           PERFORM UNTIL COBOL-QUIT = 1
               CALL "COBOL-WAIT-EVENT"
                   USING COBOL-EVENT-ID COBOL-CONTROL-ID
               EVALUATE COBOL-EVENT-ID
                   WHEN "onStreamItem"
                       COBOL::"WS-STREAM-ITEM" ( WS-SUB TICK WS-RESTART )
                       DISPLAY "item " N " " WHAT " restart=" WS-RESTART
                   WHEN "onStreamEnd"
                       COBOL::"WS-STREAM-ITEM" ( WS-SUB TICK WS-RESTART WS-REASON )
                       DISPLAY "end " WS-REASON
{on_end}
                       MOVE 1 TO COBOL-QUIT
               END-EVALUATE
           END-PERFORM
           STOP RUN.
"#
    );
    let parsed = parse(tokenize(&src, SourceFormat::Free));
    parsed.program.expect("the program parses")
}

/// Run the loop on its own thread; return what it displayed and the transport's record.
fn run(subscribe: &str, on_end: &str, wire: Wire) -> (Vec<String>, Arc<Mutex<Wire>>, Interpreter) {
    let shared = Arc::new(Mutex::new(wire));
    let (_events, event_rx) = mpsc::channel();
    let (state_tx, _state_rx) = mpsc::channel();
    let (display_tx, display_rx) = mpsc::channel();
    let mut i = Interpreter::new_with_channels(program(subscribe, on_end), event_rx, state_tx, display_tx);
    i.set_remote_transport(Box::new(Rig(shared.clone())));
    i.run().expect("the loop ends when the stream does");
    let shown = display_rx.try_iter().map(|s| s.trim_end().to_string()).collect();
    (shown, shared, i)
}

const SUBSCRIBE: &str = "           COBOL::\"WS-SUBSCRIBE\" ( \"TICKER\" WS-SUB WS-ST WS-LIMIT )";

#[test]
fn a_form_receives_stream_items_and_the_end() {
    let t = Instant::now();
    let wire = Wire { notes: VecDeque::from([item(1, 1), item(2, 2), item(3, 3), StreamNote::Ended]), ..Wire::default() };
    let (shown, wire, i) = run(SUBSCRIBE, "", wire);
    let ms = t.elapsed().as_millis();
    drop(i);
    let w = wire.lock().unwrap();
    println!("== T14 streams: a form receives the items and the end ==");
    println!("shown: {shown:?}");
    println!("started: {:?}; ended: {:?}; {ms} ms", w.started, w.ended);
    assert_eq!(shown, ["subscribed OK", "item 0001 tick     restart=N", "item 0002 tick     restart=N", "item 0003 tick     restart=N", "end ended"]);
    assert_eq!(w.started, [("TICKER".to_string(), vec!["{\"WS-EVERY\":30}".to_string()])], "the group travels as the subscription's argument");
    assert_eq!(w.ended, [7], "the stream that ended is released");
}

#[test]
fn a_reconnect_resubscribes_and_sets_the_restart_flag() {
    let wire = Wire { notes: VecDeque::from([item(1, 1), item(2, 2), StreamNote::Restarted, item(1, 3), item(2, 4), StreamNote::Ended]), ..Wire::default() };
    let (shown, _wire, _i) = run(SUBSCRIBE, "", wire);
    println!("== T14 streams: the restart flag ==\nshown: {shown:?}");
    assert_eq!(&shown[1..5], ["item 0001 tick     restart=N", "item 0002 tick     restart=N", "item 0003 tick     restart=Y", "item 0004 tick     restart=N"], "Y only on the first item after the re-subscription");
}

#[test]
fn unsubscribing_ends_the_stream_with_a_reason() {
    let wire = Wire { notes: VecDeque::from([item(1, 1), item(2, 2)]), ..Wire::default() };
    // The handler of the first item unsubscribes; the end arrives with the reason `unsubscribed`.
    let (shown, wire, i) = run_unsubscribing(wire);
    drop(i);
    println!("== T14 streams: unsubscribe ==\nshown: {shown:?}");
    assert_eq!(shown.last().map(String::as_str), Some("end unsubscribed"));
    assert_eq!(wire.lock().unwrap().ended, [7]);
}

fn run_unsubscribing(wire: Wire) -> (Vec<String>, Arc<Mutex<Wire>>, Interpreter) {
    let src = r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. STREAMS.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 COBOL-EVENT-ID    PIC X(30).
       01 COBOL-CONTROL-ID  PIC X(30).
       01 COBOL-QUIT        PIC 9 VALUE 0.
       01 WS-SUB            PIC X(10).
       01 WS-ST             PIC X(10).
       01 WS-RESTART        PIC X(1).
       01 WS-REASON         PIC X(20).
       01 TICK.
          05 N              PIC 9(4).
          05 WHAT           PIC X(8).
       PROCEDURE DIVISION.
       MAIN.
           COBOL::"WS-SUBSCRIBE" ( "TICKER" WS-SUB WS-ST )
           PERFORM UNTIL COBOL-QUIT = 1
               CALL "COBOL-WAIT-EVENT"
                   USING COBOL-EVENT-ID COBOL-CONTROL-ID
               EVALUATE COBOL-EVENT-ID
                   WHEN "onStreamItem"
                       COBOL::"WS-STREAM-ITEM" ( WS-SUB TICK )
                       DISPLAY "item " N
                       COBOL::"WS-UNSUBSCRIBE" ( WS-SUB )
                   WHEN "onStreamEnd"
                       COBOL::"WS-STREAM-ITEM" ( WS-SUB TICK WS-RESTART WS-REASON )
                       DISPLAY "end " WS-REASON
                       MOVE 1 TO COBOL-QUIT
               END-EVALUATE
           END-PERFORM
           STOP RUN.
"#;
    let shared = Arc::new(Mutex::new(wire));
    let (_events, event_rx) = mpsc::channel();
    let (state_tx, _state_rx) = mpsc::channel();
    let (display_tx, display_rx) = mpsc::channel();
    let parsed = parse(tokenize(src, SourceFormat::Free));
    let mut i = Interpreter::new_with_channels(parsed.program.expect("parses"), event_rx, state_tx, display_tx);
    i.set_remote_transport(Box::new(Rig(shared.clone())));
    i.run().expect("runs");
    let shown = display_rx.try_iter().map(|s| s.trim_end().to_string()).collect();
    (shown, shared, i)
}

#[test]
fn closing_the_form_unsubscribes() {
    // The program quits while its stream is still running: dropping the run (the form closing) must release the subscription.
    let src = r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. STREAMS.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 COBOL-EVENT-ID    PIC X(30).
       01 COBOL-CONTROL-ID  PIC X(30).
       01 COBOL-QUIT        PIC 9 VALUE 0.
       01 WS-SUB            PIC X(10).
       01 WS-ST             PIC X(10).
       PROCEDURE DIVISION.
       MAIN.
           COBOL::"WS-SUBSCRIBE" ( "TICKER" WS-SUB WS-ST )
           PERFORM UNTIL COBOL-QUIT = 1
               CALL "COBOL-WAIT-EVENT"
                   USING COBOL-EVENT-ID COBOL-CONTROL-ID
               MOVE 1 TO COBOL-QUIT
           END-PERFORM
           STOP RUN.
"#;
    let shared = Arc::new(Mutex::new(Wire { notes: VecDeque::from([item(1, 1)]), ..Wire::default() }));
    let (events, event_rx) = mpsc::channel();
    let (state_tx, _state_rx) = mpsc::channel();
    let (display_tx, _display_rx) = mpsc::channel();
    let parsed = parse(tokenize(src, SourceFormat::Free));
    let mut i = Interpreter::new_with_channels(parsed.program.expect("parses"), event_rx, state_tx, display_tx);
    i.set_remote_transport(Box::new(Rig(shared.clone())));
    let _keep_open = events;
    i.run().expect("runs");
    let before = shared.lock().unwrap().ended.len();
    drop(i);
    let after = shared.lock().unwrap().ended.clone();
    println!("== T14 streams: closing the form ==\nsubscriptions ended before the form closed: {before}; after: {after:?}");
    assert_eq!(before, 0);
    assert_eq!(after, [7], "the form's subscription is released when the form closes");
}

#[test]
fn a_refused_subscription_says_so() {
    let (shown, wire, _i) = {
        let shared = Arc::new(Mutex::new(Wire { refuse: true, ..Wire::default() }));
        let src = "       IDENTIFICATION DIVISION.\n       PROGRAM-ID. S.\n       DATA DIVISION.\n       WORKING-STORAGE SECTION.\n       01 WS-SUB PIC X(10).\n       01 WS-ST PIC X(10).\n       PROCEDURE DIVISION.\n       MAIN.\n           COBOL::\"WS-SUBSCRIBE\" ( \"NOPE\" WS-SUB WS-ST )\n           DISPLAY \"status \" WS-ST\n           STOP RUN.\n";
        let (_e, event_rx) = mpsc::channel();
        let (state_tx, _s) = mpsc::channel();
        let (display_tx, display_rx) = mpsc::channel();
        let parsed = parse(tokenize(src, SourceFormat::Free));
        let mut i = Interpreter::new_with_channels(parsed.program.expect("parses"), event_rx, state_tx, display_tx);
        i.set_remote_transport(Box::new(Rig(shared.clone())));
        i.run().unwrap();
        let shown: Vec<String> = display_rx.try_iter().map(|s| s.trim_end().to_string()).collect();
        (shown, shared, i)
    };
    println!("== T14 streams: a refused subscription ==\nshown: {shown:?}");
    assert_eq!(shown, ["status REFUSED"]);
    assert!(wire.lock().unwrap().started.is_empty());
}
