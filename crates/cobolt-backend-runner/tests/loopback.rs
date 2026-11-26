// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Spec 114 T21 and T22 (R8 per-call, R9, R10, R11; AC3 in part, AC5, AC6): the Backend's module as the server's engine,
//! over a real TLS loopback socket.
//!
//! The same Backend, from the same sources, is served twice, once by the interpreter and once by the module on this
//! runner's engine, and the same calls are made to each. What a caller learns must be the same: the JSON of every
//! parameter and of `RETURNING`, and for a call that fails the class, the code and the member it names. Then what only
//! the module does: a program in a loop with no statement the interpreter would stop at is ended by the server's time
//! limit, its thread returns, and the next call is served; and calls from many connections at once never see each
//! other's `WORKING-STORAGE`.

use cobolt_backend_runner::engine::{refusal_name, WasmEngine};
use cobolt_server::exec::InterpreterEngine;
use cobolt_server::testkit::*;
use cobolt_wasm::{compile_backend, BackendProgram, Options};
use serde_json::{json, Value};
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::{Duration, Instant};

const EXPORTS: &str = r#"
[contract]
version = "1.0.0"

[[export]]
program = "ADD-TWO"

[[export]]
program = "GREET"

[[export]]
program = "TABLE-ECHO"

[[export]]
program = "COUNTER"

[[export]]
program = "SPIN"

[[export]]
program = "BOOM"
"#;

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

const GREET: &str = r#"
IDENTIFICATION DIVISION.
PROGRAM-ID. GREET.
DATA DIVISION.
LINKAGE SECTION.
01 WHO PIC X(10).
01 MSG PIC X(20).
PROCEDURE DIVISION USING WHO MSG.
MAIN-PARA.
    STRING "HELLO " DELIMITED BY SIZE WHO DELIMITED BY SPACE INTO MSG.
    GOBACK.
"#;

const TABLE_ECHO: &str = r#"
IDENTIFICATION DIVISION.
PROGRAM-ID. TABLE-ECHO.
DATA DIVISION.
WORKING-STORAGE SECTION.
01 I PIC 99.
LINKAGE SECTION.
01 LK-T.
    05 N    PIC 99.
    05 ITEM PIC 9(3) OCCURS 1 TO 4 DEPENDING ON N.
PROCEDURE DIVISION USING LK-T.
MAIN-PARA.
    PERFORM VARYING I FROM 1 BY 1 UNTIL I > N
        COMPUTE ITEM(I) = ITEM(I) * 2
    END-PERFORM
    GOBACK.
"#;

const COUNTER: &str = r#"
IDENTIFICATION DIVISION.
PROGRAM-ID. COUNTER.
DATA DIVISION.
WORKING-STORAGE SECTION.
01 SEEN-HERE PIC 9(4) VALUE 0.
LINKAGE SECTION.
01 LK.
    05 WHO  PIC X(10).
    05 SEEN PIC 9(4).
PROCEDURE DIVISION USING LK.
MAIN-PARA.
    ADD 1 TO SEEN-HERE
    ADD SEEN-HERE TO SEEN
    GOBACK.
"#;

const SPIN: &str = r#"
IDENTIFICATION DIVISION.
PROGRAM-ID. SPIN.
DATA DIVISION.
WORKING-STORAGE SECTION.
01 N PIC 9(9) VALUE 0.
LINKAGE SECTION.
01 P.
    05 NN PIC 9(2).
PROCEDURE DIVISION USING P.
MAIN-PARA.
    PERFORM UNTIL 1 = 2
        ADD 1 TO N
        IF N > 900000000
            MOVE 0 TO N
        END-IF
    END-PERFORM
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
01 P.
    05 NN PIC 9(2).
PROCEDURE DIVISION USING P.
MAIN-PARA.
    COMPUTE R = 7 / ZED.
    GOBACK.
"#;

const SOURCES: [(&str, &str); 6] =
    [("add.cbl", ADD), ("greet.cbl", GREET), ("table-echo.cbl", TABLE_ECHO), ("counter.cbl", COUNTER), ("spin.cbl", SPIN), ("boom.cbl", BOOM)];

fn parse(src: &str) -> cobolt_ast::program::Program {
    use cobolt_lexer::{tokenize, SourceFormat};
    let parsed = cobolt_parser::parse(tokenize(src, SourceFormat::Free));
    let errors: Vec<String> = parsed.diagnostics.iter().filter(|d| d.is_error()).map(|d| format!("line {}: {}", d.span.line, d.message)).collect();
    assert!(errors.is_empty(), "the source does not parse: {errors:?}");
    parsed.program.expect("a program")
}

/// The Backend module of the sources, each program nested in each of the others as `Backend::build` nests them.
fn module() -> Vec<u8> {
    let programs: Vec<_> = SOURCES.iter().map(|(_, s)| parse(s)).collect();
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

fn wasm_engine() -> WasmEngine {
    WasmEngine::start(&module(), None, &cobolt_backend_runner::provided()).unwrap_or_else(|e| panic!("the engine does not start: {e}"))
}

fn rig_of(name: &str, engine: Arc<dyn cobolt_server::engine::Engine>, extra: &str) -> Rig {
    rig_on(engine, backend_dir(name, EXPORTS, &SOURCES), extra)
}

fn token(rig: &Rig, sub: &str) -> String {
    rig.issuer.access(sub, AUDIENCE, 600, json!({}))
}

async fn ready(rig: &Rig, sub: &str) -> Raw {
    let mut c = Raw::connect(rig, Some(ORIGIN)).await.expect("connect");
    let h = c.hello("1.0.0").await;
    assert_eq!(h["accepted"], "ok", "{h}");
    let a = c.auth(&token(rig, sub)).await;
    assert_eq!(a["t"], "auth-ok", "{a}");
    c
}

/// What a caller learns from a reply: the answer, or the class, code and member of the refusal. The sentence the
/// interpreter adds after the member is not compared (a recorded difference), nor the call's id.
fn learned(r: &Value) -> Value {
    if r["t"] == "reply" {
        json!({"args": r["args"], "returning": r["returning"]})
    } else {
        json!({"t": r["t"], "class": r["class"], "code": r["code"], "detail": r["detail"]})
    }
}

/// The calls both engines answer: ordinary, empty, at a limit, over a limit, a bad member, a run-time error.
fn calls() -> Vec<(&'static str, &'static str, Vec<(&'static str, &'static str)>, bool)> {
    vec![
        ("ordinary add", "ADD-TWO", vec![("ref", r#"{"A":40,"B":2}"#)], true),
        ("add at the picture limit", "ADD-TWO", vec![("ref", r#"{"A":9999,"B":9999}"#)], true),
        ("add with zeros", "ADD-TWO", vec![("ref", r#"{"A":0,"B":0}"#)], true),
        ("add over the picture", "ADD-TWO", vec![("ref", r#"{"A":10000,"B":1}"#)], true),
        ("add with a text where a number goes", "ADD-TWO", vec![("ref", r#"{"A":"x","B":1}"#)], true),
        ("add with a member it does not have", "ADD-TWO", vec![("ref", r#"{"A":1,"B":1,"C":9}"#)], true),
        ("add with a negative for an unsigned picture", "ADD-TWO", vec![("ref", r#"{"A":-1,"B":1}"#)], true),
        ("greeting", "GREET", vec![("ref", r#""BOB""#), ("ref", r#""""#)], false),
        ("greeting of a full name field", "GREET", vec![("ref", r#""ABCDEFGHIJ""#), ("ref", r#""""#)], false),
        ("greeting over the picture", "GREET", vec![("ref", r#""ABCDEFGHIJK""#), ("ref", r#""""#)], false),
        ("table of three", "TABLE-ECHO", vec![("ref", r#"{"N":3,"ITEM":[1,2,3]}"#)], false),
        ("table at its maximum", "TABLE-ECHO", vec![("ref", r#"{"N":4,"ITEM":[100,200,300,400]}"#)], false),
        ("table over its maximum", "TABLE-ECHO", vec![("ref", r#"{"N":5,"ITEM":[1,2,3,4,5]}"#)], false),
        ("table whose count and items disagree", "TABLE-ECHO", vec![("ref", r#"{"N":3,"ITEM":[1,2]}"#)], false),
        ("a counter starts afresh", "COUNTER", vec![("ref", r#"{"WHO":"x","SEEN":0}"#)], false),
        ("a counter starts afresh again", "COUNTER", vec![("ref", r#"{"WHO":"x","SEEN":0}"#)], false),
        ("a run-time error", "BOOM", vec![("ref", r#"{"NN":7}"#)], false),
        ("a program the contract does not export", "NOPE", vec![], false),
        ("too few arguments", "ADD-TWO", vec![], true),
    ]
}

#[tokio::test(flavor = "multi_thread")]
async fn the_module_answers_what_the_interpreter_answers() {
    let started = Instant::now();
    let interp = rig_of("parity-i", Arc::new(InterpreterEngine), "");
    let wasm = rig_of("parity-w", Arc::new(wasm_engine()), "");
    println!("engines: {} and {} (rigs started in {:.1} ms)", interp.engine_name(), wasm.engine_name(), started.elapsed().as_secs_f64() * 1000.0);
    assert_eq!((interp.engine_name(), wasm.engine_name()), ("interpreter", "wasm"));
    let mut ci = ready(&interp, "u").await;
    let mut cw = ready(&wasm, "u").await;
    let (mut equal, mut different) = (0u32, 0u32);
    let mut refused = 0u32;
    for (i, (what, program, args, returning)) in calls().into_iter().enumerate() {
        let a = ci.call(i as u64 + 1, program, &args, returning).await;
        let b = cw.call(i as u64 + 1, program, &args, returning).await;
        let (la, lb) = (learned(&a), learned(&b));
        if la == lb {
            equal += 1;
        } else {
            different += 1;
            println!("DIFFERENT {what}:\n  interpreter {a}\n  module      {b}");
        }
        if a["t"] != "reply" {
            refused += 1;
        }
    }
    let total = equal + different;
    let ms = started.elapsed().as_millis();
    println!(
        "{total} calls on the interpreter and on the module over a real loopback socket: {equal} equal, {different} different ({refused} of them refused or failed, with the same class, code and member) in {ms} ms"
    );
    assert_eq!(different, 0, "the two engines answered differently");
    println!("PASS {equal:03} FAIL {different:03}");
    interp.server.stop();
    wasm.server.stop();
}

#[tokio::test(flavor = "multi_thread")]
async fn a_call_nothing_stops_is_stopped_by_the_server_and_the_next_call_is_served() {
    let started = Instant::now();
    let engine = wasm_engine();
    let in_flight = engine.in_flight_counter();
    let rig = rig_of("spin", Arc::new(engine), "[limits]\ncall_timeout_ms = 700\nconnection_rate_per_sec = 100\nconnection_burst = 100\n");
    let mut c = ready(&rig, "u").await;
    let t0 = Instant::now();
    let r = c.call(1, "SPIN", &[("ref", r#"{"NN":1}"#)], false).await;
    let replied = t0.elapsed();
    assert_eq!((class(&r).as_str(), code(&r)), ("limit", 7003), "{r}");
    assert!(replied >= Duration::from_millis(600) && replied < Duration::from_secs(5), "{replied:?}");
    // The thread that ran it returns: the interpreter's could only be abandoned.
    let t1 = Instant::now();
    while in_flight.load(Ordering::SeqCst) > 0 && t1.elapsed() < Duration::from_secs(5) {
        tokio::time::sleep(Duration::from_millis(2)).await;
    }
    let ended = t1.elapsed();
    assert_eq!(in_flight.load(Ordering::SeqCst), 0, "the call's thread did not return");
    let next = c.call(2, "ADD-TWO", &[("ref", r#"{"A":2,"B":3}"#)], true).await;
    assert_eq!(next["t"], "reply", "{next}");
    assert_eq!(next["returning"]["TOTAL"], 5, "{next}");
    println!(
        "SPIN (a loop with no exit) ended by the server's limit of 700 ms: the caller was answered after {} ms, the thread had returned {} ms after that, and the next call was served ({} ms in all)",
        replied.as_millis(),
        ended.as_millis(),
        started.elapsed().as_millis()
    );
    println!("PASS 003 FAIL 000");
    rig.server.stop();
}

#[tokio::test(flavor = "multi_thread")]
async fn calls_from_many_connections_at_once_never_see_each_others_working_storage() {
    let started = Instant::now();
    let rig = rig_of(
        "parallel",
        Arc::new(wasm_engine()),
        "[limits]\nconnection_rate_per_sec = 1000\nconnection_burst = 1000\nuser_rate_per_sec = 5000\nuser_burst = 5000\nmax_calls_per_connection = 64\n",
    );
    let (addr, cert) = (rig.server.addr, rig.cert.cert_pem.clone());
    let (users, each) = (8u64, 25u64);
    let mut handles = Vec::new();
    for u in 0..users {
        let tok = token(&rig, &format!("user-{u}"));
        let cert = cert.clone();
        handles.push(tokio::spawn(async move {
            let mut c = Raw::connect_to(addr, &cert, Some(ORIGIN)).await.unwrap();
            c.hello("1.0.0").await;
            c.auth(&tok).await;
            let mut calls = 0u64;
            for k in 0..each {
                let id = 1000 * u + k;
                let (a, b) = (100 * u + k, k);
                let r = c.call(id, "ADD-TWO", &[("ref", &format!(r#"{{"A":{a},"B":{b}}}"#))], true).await;
                assert_eq!(r["returning"]["TOTAL"], a + b, "{r}");
                // A counter in WORKING-STORAGE: every call starts afresh, so the answer is always 1.
                let n = c.call(id + 500, "COUNTER", &[("ref", r#"{"WHO":"x","SEEN":0}"#)], false).await;
                assert_eq!(n["args"][0]["SEEN"], 1, "state survived a call: {n}");
                calls += 2;
            }
            calls
        }));
    }
    let mut total = 0;
    for h in handles {
        total += h.await.unwrap();
    }
    let ms = started.elapsed().as_millis().max(1);
    println!(
        "{users} connections x {each} iterations (2 calls each) on one module: {total} calls in {ms} ms ({} calls/sec); no call saw another's WORKING-STORAGE",
        u128::from(total as u64) * 1000 / ms
    );
    println!("PASS {total:03} FAIL 000");
    rig.server.stop();
}

#[test]
fn the_words_for_a_refused_request_are_the_interpreters() {
    // The module answers a code and a member; the runner words it as the interpreter does. A code the two disagree on
    // would change what a caller reads.
    for code in 1..=12 {
        assert_eq!(refusal_name(code), cobolt_wasm_rt::json::code_name(code), "code {code}");
    }
    assert_eq!(refusal_name(99), "internal");
    println!("12 codes of the mapping named alike by the runner and by the module's routines");
    println!("PASS 013 FAIL 000");
}
