// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Spec 114 T37 (R8, AC5; ABI A61): a connection that keeps its state, on the interpreter's engine and on the module's.
//!
//! `[backend] state = "per-connection"` gives a connection one run unit per exported program it has called, for as long as
//! the connection lasts: the program's `WORKING-STORAGE` and a file it left open are what its next call finds. The same
//! Backend is served by `cobolt_server::exec::Session` (the interpreter) and by this runner's session (one instance of the
//! module per connection and program, on a worker thread of the session), and the same script of calls is made to each:
//! what a caller learns must be equal. Then the isolation the model promises: two connections never see each other's
//! storage, a connection's concurrent calls take turns, and a call that is cut off, cancelled or that dies with a run-time
//! error ends that program's instance while a `WS-FAIL` does not.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use cobolt_backend_runner::payload::{self, Package};
use cobolt_program_package::backend::BackendContainer;
use cobolt_server::backend::{Backend, StateMode};
use cobolt_server::engine::{Engine, EngineSession, ExecOptions, ExecResult};
use cobolt_server::exec::InterpreterEngine;
use cobolt_server::testkit::backend_dir;
use cobolt_wasm::{compile_backend, BackendOptions, BackendProgram, Options};
use cobolt_wire::error::{codes, CallError};

const EXPORTS: &str = r#"
[contract]
version = "1.0.0"

[[export]]
program = "COUNTER"

[[export]]
program = "TALLY"

[[export]]
program = "KEEPER"

[[export]]
program = "RISKY"

[[export]]
program = "STOPPER"

[[export]]
program = "PULSER"
stream = true
"#;

/// Counts its own calls in `WORKING-STORAGE`, which a per-call Backend starts afresh at every call.
const COUNTER: &str = r#"
IDENTIFICATION DIVISION.
PROGRAM-ID. COUNTER.
DATA DIVISION.
WORKING-STORAGE SECTION.
01 SEEN-HERE PIC 9(5) VALUE 0.
LINKAGE SECTION.
01 LK.
    05 CALLS PIC 9(5).
PROCEDURE DIVISION USING LK.
MAIN-PARA.
    ADD 1 TO SEEN-HERE
    MOVE SEEN-HERE TO CALLS
    GOBACK.
"#;

/// The same, in a program of its own: each program has its own run unit.
const TALLY: &str = r#"
IDENTIFICATION DIVISION.
PROGRAM-ID. TALLY.
DATA DIVISION.
WORKING-STORAGE SECTION.
01 SEEN-HERE PIC 9(5) VALUE 0.
LINKAGE SECTION.
01 LK.
    05 CALLS PIC 9(5).
PROCEDURE DIVISION USING LK.
MAIN-PARA.
    ADD 10 TO SEEN-HERE
    MOVE SEEN-HERE TO CALLS
    GOBACK.
"#;

/// Opens a file at its first call and leaves it open: the next calls write to the same open file. A program whose state
/// was ended would open (and so empty) the file again.
fn keeper(path: &str) -> String {
    format!(
        r#"
IDENTIFICATION DIVISION.
PROGRAM-ID. KEEPER.
ENVIRONMENT DIVISION.
INPUT-OUTPUT SECTION.
FILE-CONTROL.
    SELECT LOG-FILE ASSIGN TO "{path}"
        ORGANIZATION IS LINE SEQUENTIAL.
DATA DIVISION.
FILE SECTION.
FD LOG-FILE.
01 LOG-LINE PIC X(20).
WORKING-STORAGE SECTION.
01 IS-OPEN PIC X VALUE "N".
LINKAGE SECTION.
01 LK.
    05 TEXT-IN PIC X(20).
    05 ACTION  PIC X.
PROCEDURE DIVISION USING LK.
MAIN-PARA.
    IF IS-OPEN = "N"
        OPEN OUTPUT LOG-FILE
        MOVE "Y" TO IS-OPEN
    END-IF
    MOVE TEXT-IN TO LOG-LINE
    WRITE LOG-LINE
    IF ACTION = "C"
        CLOSE LOG-FILE
        MOVE "N" TO IS-OPEN
    END-IF
    GOBACK.
"#
    )
}

/// Counts, and then ends in the way the caller asks: a failure it reports (`WS-FAIL`), a run-time error (a `WS-FAIL` code outside the range), a wait that the
/// caller cancels or the server stops.
const RISKY: &str = r#"
IDENTIFICATION DIVISION.
PROGRAM-ID. RISKY.
DATA DIVISION.
WORKING-STORAGE SECTION.
01 SEEN-HERE PIC 9(5) VALUE 0.
LINKAGE SECTION.
01 LK.
    05 HOW   PIC X.
    05 CALLS PIC 9(5).
PROCEDURE DIVISION USING LK.
MAIN-PARA.
    ADD 1 TO SEEN-HERE
    MOVE SEEN-HERE TO CALLS
    EVALUATE HOW
        WHEN "F"
            COBOL::"WS-FAIL" ( 4201 "declined" )
        WHEN "B"
            COBOL::"WS-FAIL" ( 0 "no such code" )
        WHEN "W"
            COBOL::"WS-PAUSE" ( 5000 )
    END-EVALUATE
    GOBACK.
"#;

/// A program that ends with `STOP RUN` rather than `GOBACK`: the run unit it ends is the connection's, which goes on.
const STOPPER: &str = r#"
IDENTIFICATION DIVISION.
PROGRAM-ID. STOPPER.
DATA DIVISION.
WORKING-STORAGE SECTION.
01 SEEN-HERE PIC 9(5) VALUE 0.
LINKAGE SECTION.
01 LK.
    05 CALLS PIC 9(5).
PROCEDURE DIVISION USING LK.
MAIN-PARA.
    ADD 100 TO SEEN-HERE
    MOVE SEEN-HERE TO CALLS
    STOP RUN.
"#;

/// A stream: it sends three items and ends. A stream ignores the state mode: every call of it runs on an instance of its own, so
/// its `WORKING-STORAGE` starts afresh whatever the Backend keeps for the connection.
const PULSER: &str = r#"
IDENTIFICATION DIVISION.
PROGRAM-ID. PULSER.
DATA DIVISION.
WORKING-STORAGE SECTION.
01 I PIC 99.
01 TICK.
    05 SEQ PIC 9(4) VALUE 0.
LINKAGE SECTION.
01 LK.
    05 UNUSED PIC X.
PROCEDURE DIVISION USING LK.
MAIN-PARA.
    PERFORM VARYING I FROM 1 BY 1 UNTIL I > 3
        ADD 1 TO SEQ
        COBOL::"WS-EMIT" ( TICK )
    END-PERFORM
    GOBACK.
"#;

// ── the two engines ─────────────────────────────────────────────────────────────────────────────────────────────────────────

/// The Backend and the engine that serves it. `module` is the module of the Backend as a Build makes it and the runner reads
/// it back (the module's own state mode, the one the generator was given), else the interpreter's engine on the sources.
fn served(dir: &std::path::Path, module: bool) -> (Backend, Arc<dyn Engine>) {
    let mut source = Backend::load(dir).unwrap_or_else(|e| panic!("the Backend does not load: {e:#?}"));
    source.state = StateMode::PerConnection;
    if !module {
        return (source, Arc::new(InterpreterEngine));
    }
    let mut programs: Vec<_> = source.programs().collect();
    programs.sort_by(|a, b| a.sig.name.cmp(&b.sig.name));
    let list: Vec<BackendProgram<'_>> =
        programs.iter().map(|p| BackendProgram { name: p.sig.name.clone(), program: p.program.as_ref().expect("the sources of the Backend") }).collect();
    let options = Options { native: true, backend: Some(BackendOptions { per_connection: true, ..BackendOptions::default() }), ..Options::default() };
    let wasm = compile_backend(&list, &options).unwrap_or_else(|e| panic!("the generator refused the Backend: {e}")).wasm;
    let container = BackendContainer {
        state: source.state.as_str().into(),
        interface: source.interface.to_json().into_bytes(),
        exports: serde_json::to_vec(&source.export_entries()).unwrap(),
        history: b"[]".to_vec(),
        slots: b"[]".to_vec(),
        module: wasm,
        name: "session".to_string(),
    };
    let loaded = payload::load(Package { container, code: None }, &cobolt_backend_runner::provided()).unwrap_or_else(|e| panic!("the runner refuses the Backend: {e:?}"));
    let engine = loaded.engine.expect("the runner's engine");
    assert_eq!(engine.name(), "wasm");
    (loaded.backend, engine)
}

// ── a call, and what a caller learns from it ───────────────────────────────────────────────────────────────────────────────

/// What a caller learns: the answer's parameters, or the class and code of the failure.
type Seen = Result<Vec<String>, (String, u32)>;

fn seen(r: Result<ExecResult, CallError>) -> Seen {
    r.map(|r| r.args).map_err(|e| (format!("{:?}", e.class), e.code))
}

/// Call `program` in `session`; `cancel_asked_after` is the time after which the caller asks to cancel, `stopped_after` the
/// time after which the server stops the call.
fn call(session: &mut dyn EngineSession, backend: &Backend, program: &str, arg: &str, cancel_asked_after: Option<Duration>, stopped_after: Option<Duration>) -> Seen {
    let prog = backend.program(program).unwrap_or_else(|| panic!("no program {program}"));
    let cancel = Arc::new(AtomicBool::new(false));
    let opts = ExecOptions::plain(cancel.clone(), Duration::from_millis(200));
    let mut timers = Vec::new();
    for (after, flag) in [(cancel_asked_after, opts.cancel_requested.clone()), (stopped_after, cancel)] {
        if let Some(d) = after {
            timers.push(std::thread::spawn(move || {
                std::thread::sleep(d);
                flag.store(true, Ordering::SeqCst);
            }));
        }
    }
    let r = session.execute(prog, &[arg.to_string()], opts);
    for t in timers {
        let _ = t.join();
    }
    seen(r)
}

/// The items a call of a stream sends, in order.
fn stream(engine: &dyn Engine, backend: &Backend, program: &str, arg: &str) -> Vec<String> {
    let prog = backend.program(program).unwrap_or_else(|| panic!("no program {program}"));
    let items = Arc::new(Mutex::new(Vec::new()));
    let mut opts = ExecOptions::plain(Arc::new(AtomicBool::new(false)), Duration::from_millis(200));
    let sink = items.clone();
    opts.emit = Some(Box::new(move |json| {
        sink.lock().unwrap().push(json.to_string());
        true
    }));
    engine.execute(prog, &[arg.to_string()], opts).unwrap_or_else(|e| panic!("the stream failed: {e:?}"));
    let sent = items.lock().unwrap().clone();
    sent
}

fn plain(session: &mut dyn EngineSession, backend: &Backend, program: &str, arg: &str) -> Seen {
    call(session, backend, program, arg, None, None)
}

/// The number `CALLS` holds in an answer's first parameter.
fn calls_of(r: &Seen) -> u32 {
    let args = r.as_ref().unwrap_or_else(|e| panic!("the call failed: {e:?}"));
    let at = args[0].find("\"CALLS\":").expect("CALLS") + 8;
    args[0][at..].trim_start().chars().take_while(|c| c.is_ascii_digit()).collect::<String>().parse().unwrap()
}

/// What one engine did with the script: everything a caller learns, the lines the open file ended with, and how many programs
/// the sessions held at three moments.
struct Record {
    seen: Vec<Seen>,
    file: Vec<String>,
    held: Vec<usize>,
    /// The items each of two calls of a stream sent.
    streams: Vec<Vec<String>>,
}

/// The script: two connections interleaved, a file left open, a failure the program reports, a run-time error, a cancel and a
/// stop.
fn script(dir: &std::path::Path, log: &std::path::Path, module: bool) -> Record {
    let (backend, engine) = served(dir, module);
    let mut a = engine.session();
    let mut b = engine.session();
    let mut seen_all: Vec<Seen> = Vec::new();
    let mut held: Vec<usize> = Vec::new();
    let ask = r#"{"CALLS":0}"#;

    // Connections A and B interleaved: each program counts its own calls, in each connection (0..6, then 6, 7, 8).
    for _ in 0..3 {
        seen_all.push(plain(a.as_mut(), &backend, "COUNTER", ask));
        seen_all.push(plain(b.as_mut(), &backend, "COUNTER", ask));
    }
    seen_all.push(plain(a.as_mut(), &backend, "TALLY", ask));
    seen_all.push(plain(a.as_mut(), &backend, "TALLY", ask));
    seen_all.push(plain(b.as_mut(), &backend, "COUNTER", ask));
    held.push(a.programs());
    held.push(b.programs());
    // A file left open is written by the next call, not opened again (9, 10, 11).
    for (text, action) in [("one", " "), ("two", " "), ("three", "C")] {
        seen_all.push(plain(a.as_mut(), &backend, "KEEPER", &format!(r#"{{"TEXT-IN":"{text}","ACTION":"{action}"}}"#)));
    }
    // A failure the program reports does not end its run unit; a run-time error, a cancel and a stop do (12..20).
    let risky = |how: &str| format!(r#"{{"HOW":"{how}","CALLS":0}}"#);
    seen_all.push(plain(a.as_mut(), &backend, "RISKY", &risky(" "))); // 12: 1
    seen_all.push(plain(a.as_mut(), &backend, "RISKY", &risky("F"))); // 13: fails, kept
    seen_all.push(plain(a.as_mut(), &backend, "RISKY", &risky(" "))); // 14: 3
    seen_all.push(plain(a.as_mut(), &backend, "RISKY", &risky("B"))); // 15: run-time error, ended
    seen_all.push(plain(a.as_mut(), &backend, "RISKY", &risky(" "))); // 16: 1 again
    seen_all.push(call(a.as_mut(), &backend, "RISKY", &risky("W"), Some(Duration::from_millis(60)), None)); // 17: cancelled, ended
    seen_all.push(plain(a.as_mut(), &backend, "RISKY", &risky(" "))); // 18: 1 again
    seen_all.push(call(a.as_mut(), &backend, "RISKY", &risky("W"), None, Some(Duration::from_millis(60)))); // 19: stopped, ended
    seen_all.push(plain(a.as_mut(), &backend, "RISKY", &risky(" "))); // 20: 1 again
    // STOP RUN ends the run, not the connection's state (21, 22).
    seen_all.push(plain(a.as_mut(), &backend, "STOPPER", ask));
    seen_all.push(plain(a.as_mut(), &backend, "STOPPER", ask));
    held.push(a.programs());
    // A stream ignores the state mode (the server calls `execute` for it, not the session): each call runs on an instance of its own,
    // from its initial storage, so the second call sends the same items as the first.
    let streams: Vec<Vec<String>> = (0..2).map(|_| stream(engine.as_ref(), &backend, "PULSER", r#"{"UNUSED":" "}"#)).collect();
    drop((a, b));
    let file = std::fs::read_to_string(log).unwrap_or_default().lines().map(|l| l.trim_end().to_string()).collect();
    Record { seen: seen_all, file, held, streams }
}

#[test]
fn a_connection_keeps_its_run_unit_per_program_and_both_engines_agree() {
    let started = Instant::now();
    let scratch = std::env::temp_dir().join(format!("prc-session-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&scratch);
    std::fs::create_dir_all(&scratch).unwrap();
    let mut passed = 0u32;
    macro_rules! ok {
        ($cond:expr, $($msg:tt)+) => {{
            assert!($cond, $($msg)+);
            passed += 1;
        }};
    }

    // The same script, on each engine, each with a log file of its own.
    let mut records = Vec::new();
    for (which, module) in [("interpreter", false), ("module", true)] {
        let log = scratch.join(format!("log-{which}.txt"));
        let keeper = keeper(&log.to_string_lossy());
        let dir = backend_dir("session", EXPORTS, &[("counter.cbl", COUNTER), ("tally.cbl", TALLY), ("keeper.cbl", &keeper), ("risky.cbl", RISKY), ("stopper.cbl", STOPPER), ("pulser.cbl", PULSER)]);
        let t = Instant::now();
        let record = script(&dir, &log, module);
        records.push((which, record, t.elapsed().as_millis()));
    }
    let (interp, module) = (&records[0].1, &records[1].1);
    let calls = module.seen.len();
    ok!(interp.seen == module.seen, "the engines answer the same script differently:\n  interpreter {:?}\n  module      {:?}", interp.seen, module.seen);
    ok!(interp.file == module.file, "the file the program kept open holds different lines: {:?} / {:?}", interp.file, module.file);
    ok!(interp.held == module.held, "the sessions hold a different number of run units: {:?} / {:?}", interp.held, module.held);
    ok!(interp.streams == module.streams, "the engines send different items: {:?} / {:?}", interp.streams, module.streams);

    // What the model promises, asserted on the module's record (equal to the interpreter's, above).
    let r = &module.seen;
    let counts: Vec<u32> = r[..6].iter().map(calls_of).collect();
    ok!(counts == vec![1, 1, 2, 2, 3, 3], "each connection counts its own calls: {counts:?}");
    ok!(calls_of(&r[6]) == 10, "TALLY, first call");
    ok!(calls_of(&r[7]) == 20, "TALLY is a run unit of its own: {:?}", r[7]);
    ok!(calls_of(&r[8]) == 4, "connection B's COUNTER goes on from its third: {:?}", r[8]);
    ok!(module.file == vec!["one", "two", "three"], "a file the program left open is written by its next calls: {:?}", module.file);
    ok!(calls_of(&r[12]) == 1, "RISKY, first call");
    ok!(r[13] == Err(("Application".to_string(), 4201)), "WS-FAIL is the failure: {:?}", r[13]);
    ok!(calls_of(&r[14]) == 3, "a failure the program reports leaves its run unit: the call after it is the third: {:?}", r[14]);
    ok!(r[15] == Err(("Server".to_string(), codes::PROGRAM_FAULT)), "a run-time error: {:?}", r[15]);
    ok!(calls_of(&r[16]) == 1, "a run-time error ends the run unit: {:?}", r[16]);
    ok!(r[17] == Err(("Application".to_string(), codes::CANCELLED)), "a cancel the caller asked for: {:?}", r[17]);
    ok!(calls_of(&r[18]) == 1, "a cancel ends the run unit: {:?}", r[18]);
    ok!(r[19] == Err(("Limit".to_string(), codes::CALL_TIMEOUT)), "a stop the server enforces: {:?}", r[19]);
    ok!(calls_of(&r[20]) == 1, "a stop ends the run unit: {:?}", r[20]);
    ok!(calls_of(&r[21]) == 100 && calls_of(&r[22]) == 200, "STOP RUN ends the run and keeps the connection's state: {:?} {:?}", r[21], r[22]);
    ok!(module.held == vec![2, 1, 5], "A holds COUNTER and TALLY, then also KEEPER, RISKY and STOPPER; B holds COUNTER: {:?}", module.held);
    ok!(module.streams.iter().all(|items| items.len() == 3 && items[0].contains("\"SEQ\":1") && items[2].contains("\"SEQ\":3")), "a stream runs on an instance of its own, from its initial storage, on every call: {:?}", module.streams);

    // Connections never meet, and the calls of one connection take turns.
    let dir = backend_dir("session-turns", EXPORTS, &[("counter.cbl", COUNTER), ("tally.cbl", TALLY), ("keeper.cbl", &keeper("/dev/null")), ("risky.cbl", RISKY), ("stopper.cbl", STOPPER), ("pulser.cbl", PULSER)]);
    let (backend, engine) = served(&dir, true);
    let (backend, engine) = (Arc::new(backend), engine);
    let t = Instant::now();
    let each: Vec<Vec<u32>> = (0..8)
        .map(|_| {
            let (b, e) = (backend.clone(), engine.clone());
            std::thread::spawn(move || {
                let mut session = e.session();
                (0..25).map(|_| calls_of(&plain(session.as_mut(), &b, "COUNTER", r#"{"CALLS":0}"#))).collect::<Vec<u32>>()
            })
        })
        .collect::<Vec<_>>()
        .into_iter()
        .map(|h| h.join().unwrap())
        .collect();
    let separate_ms = t.elapsed().as_millis();
    let collisions = each.iter().filter(|seq| **seq != (1..=25).collect::<Vec<u32>>()).count();
    ok!(collisions == 0, "a connection saw another's storage: {each:?}");

    let shared: Arc<Mutex<Box<dyn EngineSession>>> = Arc::new(Mutex::new(engine.session()));
    let t = Instant::now();
    let mut all: Vec<u32> = (0..8)
        .map(|_| {
            let (b, s) = (backend.clone(), shared.clone());
            // The server runs a connection's calls in turn (it holds the session's lock); so does this.
            std::thread::spawn(move || (0..25).map(|_| calls_of(&plain(s.lock().unwrap().as_mut(), &b, "COUNTER", r#"{"CALLS":0}"#))).collect::<Vec<u32>>())
        })
        .collect::<Vec<_>>()
        .into_iter()
        .flat_map(|h| h.join().unwrap())
        .collect();
    let turns_ms = t.elapsed().as_millis();
    all.sort_unstable();
    ok!(all == (1..=200).collect::<Vec<u32>>(), "calls of one connection did not take turns");
    drop(shared);

    let _ = std::fs::remove_dir_all(&scratch);
    println!(
        "=== PER-CONNECTION STATE (spec 114 T37, ABI A61) ===\n  the script ({calls} calls on connections A and B: counters of two programs, a file left open, a WS-FAIL kept, a run-time error, a cancel and a stop that end the run unit, STOP RUN, a stream called twice), on the interpreter ({} ms) and on the module ({} ms): every answer, the file and the run units held equal\n  8 connections x 25 calls each: {collisions} collisions ({separate_ms} ms); 8 threads x 25 calls on one connection: 200 of 200 counted once ({turns_ms} ms)\nPASS {passed:03} FAIL 000 ({} ms)",
        records[0].2,
        records[1].2,
        started.elapsed().as_millis()
    );
}
