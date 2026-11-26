// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Spec 114 T21 (R9, R10, R11, R12): the headless host of a Backend module.
//!
//! A Backend is compiled by the generator into one module that answers `pr_backend_call(export)`. `BackendModule` loads it
//! on the Backend engine and calls it: each call has a store, an instance and a provider state of its own. What a test
//! of it must show: an answer carries the parameters and `RETURNING` as the interpreter writes them; a request the
//! mapping refuses is named with its path; a run-time error is a trap that ends the call and not the process; a call
//! nothing ends is stopped from outside, within a measured time; what a program `DISPLAY`s stays with the call and is
//! never printed; a capability the runner does not provide is refused by name before anything runs; the data
//! capabilities (SQL, files) are the Desktop runner's own, and what a call wrote is there for the next; and calls from
//! many threads at once do not meet.

use cobolt_runner::backend::{BackendModule, CallControl, Outcome};
use cobolt_runner::Msg;
use cobolt_wasm::{compile_backend, BackendProgram, Options};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

const ADD: &str = r#"
IDENTIFICATION DIVISION.
PROGRAM-ID. ADD-TWO.
DATA DIVISION.
LINKAGE SECTION.
01 REQ.
   05 A PIC 9(4).
   05 B PIC 9(4).
01 RES.
   05 TOTAL PIC 9(5).
PROCEDURE DIVISION USING REQ RETURNING RES.
MAIN-PARA.
    COMPUTE TOTAL = A + B.
    GOBACK.
"#;

/// `DISPLAY`s a line and counts the calls it has had in its own working storage, which a call starts afresh.
const SAYS: &str = r#"
IDENTIFICATION DIVISION.
PROGRAM-ID. SAYS.
DATA DIVISION.
WORKING-STORAGE SECTION.
01 SEEN PIC 9(4) VALUE 0.
LINKAGE SECTION.
01 REQ PIC 9(4).
01 RES.
   05 COUNTED PIC 9(4).
PROCEDURE DIVISION USING REQ RETURNING RES.
MAIN-PARA.
    ADD 1 TO SEEN.
    DISPLAY "SAYS saw " REQ.
    MOVE SEEN TO COUNTED.
    GOBACK.
"#;

const BOOM: &str = r#"
IDENTIFICATION DIVISION.
PROGRAM-ID. BOOM.
DATA DIVISION.
WORKING-STORAGE SECTION.
01 ZED PIC 9 VALUE 0.
01 R PIC 9(2).
LINKAGE SECTION.
01 P PIC 9(2).
PROCEDURE DIVISION USING P.
MAIN-PARA.
    COMPUTE R = FUNCTION MOD(7, ZED).
    GOBACK.
"#;

const SPIN: &str = r#"
IDENTIFICATION DIVISION.
PROGRAM-ID. SPIN.
DATA DIVISION.
WORKING-STORAGE SECTION.
01 N PIC 9(9) VALUE 0.
LINKAGE SECTION.
01 P PIC 9(2).
PROCEDURE DIVISION USING P.
MAIN-PARA.
    PERFORM UNTIL 1 = 2
        ADD 1 TO N
        IF N > 900000000
            MOVE 0 TO N
        END-IF
    END-PERFORM.
    GOBACK.
"#;

/// Counts the rows of a table in a database file, and adds one when asked: what a call writes is for the next call.
fn database(path: &str) -> String {
    format!(
        r#"
IDENTIFICATION DIVISION.
PROGRAM-ID. TALLY.
DATA DIVISION.
WORKING-STORAGE SECTION.
01 WS-N PIC 9(4).
01 WS-ROW PIC X(40).
LINKAGE SECTION.
01 REQ.
   05 ADDING PIC 9(1).
01 RES.
   05 ROWS PIC 9(4).
PROCEDURE DIVISION USING REQ RETURNING RES.
MAIN-PARA.
    MOVE COBOL::SqlDatabase::Open("{path}") TO WS-N
    MOVE COBOL::SqlDatabase::Execute("CREATE TABLE IF NOT EXISTS t (id INTEGER)") TO WS-N
    IF ADDING = 1
        MOVE COBOL::SqlDatabase::Execute("INSERT INTO t VALUES (1)") TO WS-N
    END-IF
    MOVE COBOL::SqlDatabase::Query("SELECT COUNT(*) FROM t") TO WS-N
    SET WS-ROW TO COBOL::SqlDatabase::Fetch()
    MOVE FUNCTION NUMVAL(WS-ROW) TO ROWS
    COBOL::SqlDatabase::Close()
    GOBACK.
"#
    )
}

fn parse(src: &str) -> cobolt_ast::program::Program {
    use cobolt_lexer::{tokenize, SourceFormat};
    let parsed = cobolt_parser::parse(tokenize(src, SourceFormat::Free));
    let errors: Vec<String> = parsed.diagnostics.iter().filter(|d| d.is_error()).map(|d| format!("line {}: {}", d.span.line, d.message)).collect();
    assert!(errors.is_empty(), "the source does not parse: {errors:?}");
    parsed.program.expect("a program")
}

/// The Backend module of `sources`, each program nested in each of the others as `cobolt_server::Backend::build` does.
fn module(sources: &[&str]) -> Vec<u8> {
    let programs: Vec<_> = sources.iter().map(|s| parse(s)).collect();
    let tops: Vec<_> = programs
        .iter()
        .map(|p| {
            let mut top = p.clone();
            for other in &programs {
                if other.identification.program_id != p.identification.program_id {
                    top.nested_programs.push(other.clone());
                }
            }
            top
        })
        .collect();
    let list: Vec<BackendProgram<'_>> = tops.iter().map(|t| BackendProgram { name: t.identification.program_id.to_ascii_uppercase(), program: t }).collect();
    compile_backend(&list, &Options { native: true, ..Options::default() }).unwrap_or_else(|e| panic!("compile failed: {e}")).wasm
}

/// What a Backend runner provides: everything a desktop console program does, as the data variants name it.
fn provided() -> Vec<&'static str> {
    cobolt_runner::PROVIDED_CAPABILITIES.to_vec()
}

fn load(wasm: &[u8]) -> BackendModule {
    BackendModule::load(wasm, None, &provided(), None).unwrap_or_else(|e| panic!("load failed: {}", cobolt_runner::messages::text(cobolt_runner::Lang::En, &e)))
}

fn answered(called: cobolt_runner::backend::Called) -> cobolt_wasm_abi::backend::Answer {
    match called.outcome {
        Outcome::Answered(a) => a,
        other => panic!("not answered: {other:?}"),
    }
}

#[test]
fn a_call_is_answered_refused_trapped_or_stopped_and_what_it_displays_stays_with_it() {
    let started = Instant::now();
    let wasm = module(&[ADD, SAYS, BOOM, SPIN]);
    let m = load(&wasm);
    let none = CallControl::none();
    let mut cases = 0u32;

    // Answered: the parameter and RETURNING, as JSON.
    let a = answered(m.call("ADD-TWO", &[r#"{"A":40,"B":2}"#], &none).unwrap());
    assert_eq!(a.args.len(), 1);
    assert!(a.args[0].contains("40") && a.args[0].contains("2"), "{:?}", a.args);
    assert!(a.returning.as_deref().is_some_and(|r| r.contains("42")), "{:?}", a.returning);
    cases += 1;

    // The program's DISPLAY stays with the call; its working storage starts afresh on every call.
    for n in 0..3u32 {
        let called = m.call("SAYS", &[&format!("{}", 10 + n)], &none).unwrap();
        assert_eq!(called.display.len(), 1, "{:?}", called.display);
        assert!(called.display[0].contains(&format!("{}", 10 + n)), "{:?}", called.display);
        let r = answered(called);
        assert!(r.returning.as_deref().is_some_and(|r| r.contains('1') && !r.contains('2')), "a call starts afresh: {:?}", r.returning);
        cases += 1;
    }

    // Refused: the request is not the program's record; the member is named.
    let refused = m.call("ADD-TWO", &[r#"{"A":"not a number","B":2}"#], &none).unwrap();
    match refused.outcome {
        Outcome::Refused(r) => assert!(r.path.contains('A'), "{r:?}"),
        other => panic!("not refused: {other:?}"),
    }
    cases += 1;

    // A run-time error of the program ends the call, not the process, and the module goes on answering.
    let trapped = m.call("BOOM", &["7"], &none).unwrap();
    assert!(matches!(trapped.outcome, Outcome::Trapped(_)), "{:?}", trapped.outcome);
    answered(m.call("ADD-TWO", &[r#"{"A":1,"B":1}"#], &none).unwrap());
    cases += 2;

    // A name the module does not export is the runner's refusal, not a call.
    assert!(m.call("NOPE", &[], &none).is_err());
    cases += 1;

    // A call nothing ends is stopped from outside: the epoch advances, the stop says yes, the call returns.
    let stop = Arc::new(AtomicBool::new(false));
    let engine = m.engine().clone();
    let flag = stop.clone();
    let ticker = std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(60));
        flag.store(true, Ordering::SeqCst);
        // Ticks until the call has returned: the first one after the flag is the one that stops it.
        for _ in 0..200 {
            engine.increment_epoch();
            std::thread::sleep(Duration::from_millis(5));
        }
    });
    let t0 = Instant::now();
    let spun = m.call("SPIN", &["1"], &CallControl::on(stop)).unwrap();
    let stopped_ms = t0.elapsed().as_millis();
    ticker.join().unwrap();
    assert_eq!(spun.outcome, Outcome::Interrupted);
    assert!((60..3_000).contains(&stopped_ms), "stopped after {stopped_ms} ms");
    answered(m.call("ADD-TWO", &[r#"{"A":2,"B":2}"#], &none).unwrap());
    cases += 2;

    println!(
        "ADD-TWO answered, SAYS displayed and started afresh 3 times, a bad member refused with its path, BOOM trapped and the module went on, a name it does not export refused, SPIN stopped {} ms after it began (stop raised at 60 ms): {cases} cases in {} ms",
        stopped_ms,
        started.elapsed().as_millis()
    );
    println!("PASS {cases:03} FAIL 000");
}

#[test]
fn a_capability_the_runner_does_not_provide_is_refused_by_name_before_anything_runs() {
    let started = Instant::now();
    let wasm = module(&[&database("/tmp/never-opened.db")]);
    let without: Vec<&str> = provided().into_iter().filter(|c| *c != "sql").collect();
    let refused = BackendModule::load(&wasm, None, &without, None);
    match refused {
        Err(Msg::Capabilities(names)) => assert_eq!(names, vec!["sql".to_string()]),
        Err(other) => panic!("another refusal: {other:?}"),
        Ok(_) => panic!("loaded without the capability it declares"),
    }
    assert!(BackendModule::load(&wasm, None, &provided(), None).is_ok());
    assert!(!std::path::Path::new("/tmp/never-opened.db").exists(), "nothing ran");
    println!("a Backend declaring `sql` refused by name when `sql` is not provided, loaded when it is ({} ms)", started.elapsed().as_millis());
    println!("PASS 002 FAIL 000");
}

#[test]
fn what_a_call_writes_to_a_database_is_there_for_the_next_one() {
    let started = Instant::now();
    let dir = std::env::temp_dir().join(format!("prc-backend-module-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let db = dir.join("tally.db");
    let wasm = module(&[&database(&db.to_string_lossy())]);
    let m = load(&wasm);
    let none = CallControl::none();
    let rows = |adding: u32| -> u32 {
        let a = answered(m.call("TALLY", &[&format!(r#"{{"ADDING":{adding}}}"#)], &none).unwrap());
        let text = a.returning.expect("RETURNING answered");
        let digits: String = text.chars().filter(|c| c.is_ascii_digit()).collect();
        digits.parse().unwrap_or_else(|_| panic!("rows in {text}"))
    };
    let counts = [rows(0), rows(1), rows(1), rows(0), rows(1)];
    assert_eq!(counts, [0, 1, 2, 2, 3], "each call sees what the calls before it committed");
    let _ = std::fs::remove_dir_all(&dir);
    println!("a database file read and written by 5 calls on 5 fresh instances: rows {counts:?} ({} ms)", started.elapsed().as_millis());
    println!("PASS 005 FAIL 000");
}

#[test]
fn calls_from_many_threads_at_once_do_not_meet() {
    let started = Instant::now();
    let wasm = module(&[ADD, SAYS]);
    let m = Arc::new(load(&wasm));
    let threads = 8u32;
    let each = 40u32;
    let handles: Vec<_> = (0..threads)
        .map(|t| {
            let m = m.clone();
            std::thread::spawn(move || {
                let none = CallControl::none();
                let mut ok = 0u32;
                for i in 0..each {
                    let (a, b) = (t * 100 + i, i);
                    let called = m.call("ADD-TWO", &[&format!(r#"{{"A":{a},"B":{b}}}"#)], &none).unwrap();
                    let Outcome::Answered(r) = called.outcome else { panic!("thread {t} call {i} not answered") };
                    let want = format!("{}", a + b);
                    assert!(r.returning.as_deref().is_some_and(|x| x.contains(&want)), "thread {t} call {i}: want {want} in {:?}", r.returning);
                    let says = m.call("SAYS", &[&format!("{}", t)], &none).unwrap();
                    assert_eq!(says.display.len(), 1);
                    assert!(says.display[0].contains(&format!("{t:04}")), "thread {t}: {:?}", says.display);
                    ok += 1;
                }
                ok
            })
        })
        .collect();
    let total: u32 = handles.into_iter().map(|h| h.join().unwrap()).sum();
    assert_eq!(total, threads * each);
    let ms = started.elapsed().as_millis().max(1);
    println!(
        "{threads} threads x {each} iterations (2 calls each) on one module: {} calls in {ms} ms, {} calls/sec",
        total * 2,
        u128::from(total * 2) * 1000 / ms
    );
    println!("PASS {:03} FAIL 000", total * 2);
}
