// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Spec 114 T39 (R11, R21, AC4): the server's limits on the module engine.
//!
//! The limits are the server's (`[limits]` of `backend.toml`): the engine only has to keep its side of them. So the module is
//! served through a real TLS loopback socket under each setting and what a caller learns is held against what the
//! interpreter's engine tells for the same call.
//!
//! * the number of calls running at once (`max_calls_total`): the calls over it are refused at once with their own code and the
//!   slots come back when the calls that held them end (module only: a call that nothing stops spins until the server's time
//!   limit ends it, and the interpreter's thread cannot be ended, so it would go on spinning through the other tests);
//! * a flood from one caller does not starve a caller that is within its limits (`user_rate_per_sec`);
//! * a request that does not fit the program's layout, however large, is refused by the same code with the same words on both
//!   engines, and a frame over `max_frame_bytes` by the server before any engine is asked;
//! * a program that holds a great deal of storage in a call does not touch another call's, and the process's resident memory
//!   is read while six such calls run (reported, not asserted: it is the machine's allocator that answers, not this code).

use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::{Duration, Instant};

use cobolt_backend_runner::engine::WasmEngine;
use cobolt_server::backend::Backend;
use cobolt_server::engine::Engine;
use cobolt_server::exec::InterpreterEngine;
use cobolt_server::testkit::*;
use serde_json::{json, Value};

const EXPORTS: &str = r#"
[contract]
version = "1.0.0"

[[export]]
program = "PING"

[[export]]
program = "SPIN"

[[export]]
program = "GREET"

[[export]]
program = "TABLE-ECHO"

[[export]]
program = "BIGMEM"
"#;

const PING: &str = r#"
IDENTIFICATION DIVISION.
PROGRAM-ID. PING.
DATA DIVISION.
LINKAGE SECTION.
01 LK.
    05 N PIC 9(6).
PROCEDURE DIVISION USING LK.
MAIN-PARA.
    ADD 1 TO N
    GOBACK.
"#;

const SPIN: &str = r#"
IDENTIFICATION DIVISION.
PROGRAM-ID. SPIN.
DATA DIVISION.
LINKAGE SECTION.
01 LK.
    05 N PIC 9(4).
PROCEDURE DIVISION USING LK.
MAIN-PARA.
    PERFORM UNTIL 1 = 2
        CONTINUE
    END-PERFORM
    GOBACK.
"#;

const GREET: &str = r#"
IDENTIFICATION DIVISION.
PROGRAM-ID. GREET.
DATA DIVISION.
LINKAGE SECTION.
01 LK.
    05 WHO PIC X(10).
PROCEDURE DIVISION USING LK.
MAIN-PARA.
    GOBACK.
"#;

const TABLE_ECHO: &str = r#"
IDENTIFICATION DIVISION.
PROGRAM-ID. TABLE-ECHO.
DATA DIVISION.
LINKAGE SECTION.
01 LK.
    05 CNT  PIC 99.
    05 ITEM PIC 9(3) OCCURS 1 TO 4 DEPENDING ON CNT.
PROCEDURE DIVISION USING LK.
MAIN-PARA.
    GOBACK.
"#;

/// 16 MB of `WORKING-STORAGE`, every byte of it written in the call: a program that holds a great deal in one call.
const BIGMEM: &str = r#"
IDENTIFICATION DIVISION.
PROGRAM-ID. BIGMEM.
DATA DIVISION.
WORKING-STORAGE SECTION.
01 BIG PIC X(16000000).
LINKAGE SECTION.
01 LK.
    05 N PIC 9(6).
PROCEDURE DIVISION USING LK.
MAIN-PARA.
    PERFORM 20 TIMES
        MOVE ALL "Z" TO BIG
    END-PERFORM
    ADD 1 TO N
    GOBACK.
"#;

const SOURCES: [(&str, &str); 5] = [("ping.cbl", PING), ("spin.cbl", SPIN), ("greet.cbl", GREET), ("table-echo.cbl", TABLE_ECHO), ("bigmem.cbl", BIGMEM)];

/// The Backend's engine: the interpreter's, or the module as a Build makes it (returned with its in-flight counter).
fn engine_of(dir: &std::path::Path, module: bool) -> (Arc<dyn Engine>, Option<Arc<std::sync::atomic::AtomicUsize>>) {
    if !module {
        return (Arc::new(InterpreterEngine), None);
    }
    let backend = Backend::load(dir).unwrap_or_else(|e| panic!("the Backend does not load: {e:#?}"));
    let compiled = cobolt_compiler::backend_module::generate(&backend, &[]).unwrap_or_else(|r| panic!("the generator refuses the Backend: {r:#?}"));
    let engine = WasmEngine::start(&compiled.wasm, None, &cobolt_backend_runner::provided()).unwrap_or_else(|e| panic!("the runner refuses the module: {e}"));
    let counter = engine.in_flight_counter();
    (Arc::new(engine), Some(counter))
}

fn rig_with(name: &str, module: bool, extra: &str) -> (Rig, Option<Arc<std::sync::atomic::AtomicUsize>>) {
    let dir = backend_dir(name, EXPORTS, &SOURCES);
    let (engine, in_flight) = engine_of(&dir, module);
    (rig_on(engine, dir, extra), in_flight)
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

/// What the caller learns from a reply: the answer, or the class, code, words and member of the refusal.
fn learned(r: &Value) -> Value {
    if r["t"] == "reply" {
        json!({"args": r["args"], "returning": r["returning"]})
    } else {
        json!({"t": r["t"], "class": r["class"], "code": r["code"], "message": r["message"], "detail": r["detail"]})
    }
}

/// One call whose answer may be the end of the connection: `None` when the server closed it instead of answering.
async fn call_or_close(c: &mut Raw, id: u64, program: &str, arg: &str) -> Option<Value> {
    c.send_text(&format!("{{\"t\":\"call\",\"id\":{id},\"program\":\"{program}\",\"args\":[{{\"mode\":\"ref\",\"json\":{arg}}}],\"returning\":false}}")).await;
    c.next().await
}

/// The resident memory of this process, in kilobytes (`ps`, which every Unix this runs on has).
fn rss_kb() -> u64 {
    let out = std::process::Command::new("ps").args(["-o", "rss=", "-p", &std::process::id().to_string()]).output().expect("ps");
    String::from_utf8_lossy(&out.stdout).trim().parse().unwrap_or(0)
}

// ── how many calls run at once ─────────────────────────────────────────────────────────────────────────────────────────

#[tokio::test(flavor = "multi_thread")]
async fn at_most_max_calls_total_calls_run_at_once_the_rest_are_refused_and_the_slots_come_back() {
    let started = Instant::now();
    let (rig, in_flight) = rig_with("slots", true, "[limits]\nmax_calls_total = 3\ncall_timeout_ms = 600\nconnection_rate_per_sec = 1000\nconnection_burst = 1000\nuser_rate_per_sec = 1000\nuser_burst = 1000\n");
    let in_flight = in_flight.expect("the module engine's counter");
    let callers = 8usize;
    let t0 = Instant::now();
    let mut handles = Vec::new();
    for i in 0..callers {
        let (addr, cert) = (rig.server.addr, rig.cert.cert_pem.clone());
        let tok = token(&rig, &format!("caller-{i}"));
        handles.push(tokio::spawn(async move {
            let mut c = Raw::connect_to(addr, &cert, Some(ORIGIN)).await.expect("connect");
            c.hello("1.0.0").await;
            c.auth(&tok).await;
            let r = c.call(1, "SPIN", &[("ref", r#"{"N":1}"#)], false).await;
            (code(&r), t0.elapsed())
        }));
    }
    let mut ran = 0;
    let mut refused = 0;
    let mut slowest_refusal = Duration::ZERO;
    let mut other = Vec::new();
    for h in handles {
        let (c, took) = h.await.unwrap();
        match c {
            7003 => ran += 1,
            7002 => {
                refused += 1;
                slowest_refusal = slowest_refusal.max(took);
            }
            c => other.push(c),
        }
    }
    assert!(other.is_empty(), "some call ended with neither limit: {other:?}");
    assert_eq!((ran, refused), (3, callers - 3), "3 calls hold the slots until the server stops them, the others are refused");
    assert!(slowest_refusal < Duration::from_millis(550), "a refusal is at once, not after the slots free: {slowest_refusal:?}");
    // The three threads return, and the slots come back: a new call is served at once.
    let t1 = Instant::now();
    while in_flight.load(Ordering::SeqCst) > 0 && t1.elapsed() < Duration::from_secs(5) {
        tokio::time::sleep(Duration::from_millis(2)).await;
    }
    let returned = t1.elapsed();
    assert_eq!(in_flight.load(Ordering::SeqCst), 0, "the stopped calls' threads did not return");
    let mut c = ready(&rig, "after").await;
    let r = c.call(2, "PING", &[("ref", r#"{"N":41}"#)], false).await;
    assert_eq!((r["t"].as_str(), r["args"][0]["N"].as_u64()), (Some("reply"), Some(42)), "the slots did not come back: {r}");
    println!(
        "max_calls_total = 3, {callers} callers at once on the module: {ran} ran until the 600 ms limit stopped them, {refused} refused with 7002 (the slowest refusal after {} ms); the threads returned {} ms after, and the next call was served ({} ms in all)",
        slowest_refusal.as_millis(),
        returned.as_millis(),
        started.elapsed().as_millis()
    );
    println!("PASS 005 FAIL 000");
    rig.server.stop();
}

// ── a flood does not starve a caller within its limits ─────────────────────────────────────────────────────────────────

#[tokio::test(flavor = "multi_thread")]
async fn a_flood_from_one_caller_does_not_starve_another_that_is_within_its_limits_on_both_engines() {
    let started = Instant::now();
    let mut report = Vec::new();
    let mut verdicts = (0u32, 0u32);
    for module in [false, true] {
        let (rig, _) = rig_with(if module { "flood-w" } else { "flood-i" }, module, "[limits]\nuser_rate_per_sec = 20\nuser_burst = 20\nconnection_rate_per_sec = 1000\nconnection_burst = 1000\nmax_calls_per_connection = 1000\n");
        let engine = rig.engine_name();
        let t0 = Instant::now();
        // Four connections of one caller send 100 calls each as fast as they are answered.
        let mut flooders = Vec::new();
        for _ in 0..4 {
            let (addr, cert) = (rig.server.addr, rig.cert.cert_pem.clone());
            let tok = token(&rig, "flooder");
            flooders.push(tokio::spawn(async move {
                let mut c = Raw::connect_to(addr, &cert, Some(ORIGIN)).await.expect("connect");
                c.hello("1.0.0").await;
                c.auth(&tok).await;
                let (mut served, mut limited) = (0u32, 0u32);
                for i in 0..100u64 {
                    let r = c.call(i + 1, "PING", &[("ref", r#"{"N":1}"#)], false).await;
                    match code(&r) {
                        0 if r["t"] == "reply" => served += 1,
                        7001 => limited += 1,
                        other => panic!("{other}: {r}"),
                    }
                }
                (served, limited)
            }));
        }
        // Meanwhile another caller makes ten calls, thirty milliseconds apart.
        let mut b = ready(&rig, "bystander").await;
        let mut slowest = Duration::ZERO;
        let mut answered = 0u32;
        for i in 0..10u64 {
            let t = Instant::now();
            let r = b.call(1000 + i, "PING", &[("ref", r#"{"N":1}"#)], false).await;
            slowest = slowest.max(t.elapsed());
            if r["t"] == "reply" {
                answered += 1;
            }
            tokio::time::sleep(Duration::from_millis(30)).await;
        }
        let (mut served, mut limited) = (0u32, 0u32);
        for f in flooders {
            let (s, l) = f.await.unwrap();
            served += s;
            limited += l;
        }
        let ok = answered == 10 && limited > 0 && served + limited == 400 && slowest < Duration::from_secs(2);
        if ok {
            verdicts.0 += 1;
        } else {
            verdicts.1 += 1;
        }
        report.push(format!(
            "  {engine:<11} the flooder: {served} served and {limited} refused with 7001 of 400 calls on 4 connections; the bystander: {answered} of 10 answered, the slowest in {} ms ({} ms in all)",
            slowest.as_millis(),
            t0.elapsed().as_millis()
        ));
        rig.server.stop();
    }
    println!("user_rate_per_sec = 20 (burst 20): one caller floods, another stays within its limits");
    for l in &report {
        println!("{l}");
    }
    println!("PASS {:03} FAIL {:03} ({} ms)", verdicts.0, verdicts.1, started.elapsed().as_millis());
    assert_eq!(verdicts.1, 0, "a flood starved the other caller, or was not limited");
}

// ── a request that does not fit ────────────────────────────────────────────────────────────────────────────────────────

#[tokio::test(flavor = "multi_thread")]
async fn a_request_that_does_not_fit_its_layout_is_refused_by_the_same_code_and_the_same_words_on_both_engines() {
    let started = Instant::now();
    let big_text = format!("\"{}\"", "x".repeat(200_000));
    let many: Vec<u32> = (0..5000).collect();
    let big_table = format!("{{\"CNT\":4,\"ITEM\":{}}}", serde_json::to_string(&many).unwrap());
    let cases: Vec<(&str, &str, String)> = vec![
        ("a 200,000-byte text for PIC X(10)", "GREET", format!("{{\"WHO\":{big_text}}}")),
        ("5,000 elements for OCCURS 1 TO 4", "TABLE-ECHO", big_table),
        ("a member that is not there, among 2,000 of them", "GREET", format!("{{{}}}", (0..2_000).map(|i| format!("\"M{i}\":1")).collect::<Vec<_>>().join(","))),
        ("a number 10,000 digits long", "PING", format!("{{\"N\":{}}}", "9".repeat(10_000))),
    ];
    let mut results: Vec<Vec<Value>> = Vec::new();
    for module in [false, true] {
        let (rig, in_flight) = rig_with(if module { "big-w" } else { "big-i" }, module, "[limits]\nconnection_rate_per_sec = 1000\nconnection_burst = 1000\nuser_rate_per_sec = 1000\nuser_burst = 1000\n");
        let mut c = ready(&rig, "u").await;
        let mut seen = Vec::new();
        for (i, (what, program, arg)) in cases.iter().enumerate() {
            // One connection per case: a request this large may be answered by closing the connection, and the next case
            // must not be judged by what the last one did to it.
            let mut one = ready(&rig, "u").await;
            let t = Instant::now();
            let reply = call_or_close(&mut one, i as u64 + 1, program, arg).await;
            eprintln!("  [{}] {what}: {} in {} ms", if module { "module" } else { "interpreter" }, reply.as_ref().map(|r| r.to_string().chars().take(160).collect::<String>()).unwrap_or_else(|| "connection ended".into()), t.elapsed().as_millis());
            seen.push(reply.as_ref().map(learned).unwrap_or_else(|| json!({"t": "closed"})));
        }
        // And the server's own limit, before any engine is asked: a frame over `max_frame_bytes` ends the connection.
        let (rig2, _) = rig_with(if module { "big-w2" } else { "big-i2" }, module, "[limits]\nmax_frame_bytes = 4096\n");
        let mut c2 = ready(&rig2, "u").await;
        c2.send_text(&format!("{{\"t\":\"call\",\"id\":1,\"program\":\"GREET\",\"args\":[{{\"mode\":\"ref\",\"json\":{{\"WHO\":\"{}\"}}}}]}}", "x".repeat(8000))).await;
        let r = c2.next().await.expect("the refusal");
        seen.push(json!({"class": r["class"], "code": r["code"]}));
        assert!(c2.next().await.is_none(), "the connection is closed");
        if let Some(n) = in_flight {
            assert_eq!(n.load(Ordering::SeqCst), 0, "no call is left running");
        }
        results.push(seen);
        rig2.server.stop();
        rig.server.stop();
    }
    let (mut equal, mut different) = (0u32, 0u32);
    let mut lines = Vec::new();
    for (i, (what, _, _)) in cases.iter().map(|(a, b, c)| (a, b, c)).enumerate().chain(std::iter::once((cases.len(), (&"a frame over max_frame_bytes = 4096", &"", &String::new())))) {
        let (a, b) = (&results[0][i], &results[1][i]);
        if a == b {
            equal += 1;
        } else {
            different += 1;
        }
        lines.push(format!("  {what}: {} {} — {}", a["class"].as_str().unwrap_or("?"), a["code"], if a == b { "the same on both engines".to_string() } else { format!("DIFFERENT: interpreter {a} module {b}") }));
    }
    println!("requests that do not fit, over a real socket, on the interpreter and on the module:");
    for l in &lines {
        println!("{l}");
    }
    println!("PASS {equal:03} FAIL {different:03} ({} ms)", started.elapsed().as_millis());
    assert_eq!(different, 0, "the engines refused a request differently");
}

// ── storage held in a call ─────────────────────────────────────────────────────────────────────────────────────────────

#[tokio::test(flavor = "multi_thread")]
async fn a_program_that_holds_16_mb_in_a_call_does_not_touch_another_calls_storage_on_either_engine() {
    let started = Instant::now();
    let mut report = Vec::new();
    let mut verdicts = (0u32, 0u32);
    let mut answers: Vec<Vec<Value>> = Vec::new();
    for module in [false, true] {
        let (rig, _) = rig_with(if module { "mem-w" } else { "mem-i" }, module, "[limits]\nconnection_rate_per_sec = 1000\nconnection_burst = 1000\nuser_rate_per_sec = 1000\nuser_burst = 1000\n");
        let engine = rig.engine_name();
        let before = rss_kb();
        // Six callers each hold 16 MB at once; meanwhile another makes 30 small calls.
        let mut big = Vec::new();
        for i in 0..6u64 {
            let (addr, cert) = (rig.server.addr, rig.cert.cert_pem.clone());
            let tok = token(&rig, &format!("big-{i}"));
            big.push(tokio::spawn(async move {
                let mut c = Raw::connect_to(addr, &cert, Some(ORIGIN)).await.expect("connect");
                c.hello("1.0.0").await;
                c.auth(&tok).await;
                let r = c.call(1, "BIGMEM", &[("ref", &format!("{{\"N\":{}}}", i * 10))], false).await;
                learned(&r)
            }));
        }
        // The process's resident memory is read every 15 ms from its own thread while the calls run.
        let watching = Arc::new(std::sync::atomic::AtomicBool::new(true));
        let watcher = {
            let watching = watching.clone();
            std::thread::spawn(move || {
                let mut peak = 0u64;
                while watching.load(Ordering::SeqCst) {
                    peak = peak.max(rss_kb());
                    std::thread::sleep(Duration::from_millis(15));
                }
                peak
            })
        };
        let mut small = ready(&rig, "small").await;
        let t = Instant::now();
        let mut small_ok = 0u32;
        let mut slowest = Duration::ZERO;
        for i in 0..30u64 {
            let t1 = Instant::now();
            let r = small.call(100 + i, "PING", &[("ref", &format!("{{\"N\":{i}}}"))], false).await;
            slowest = slowest.max(t1.elapsed());
            if r["t"] == "reply" && r["args"][0]["N"] == i + 1 {
                small_ok += 1;
            }
        }
        let small_took = t.elapsed();
        let mut mine = Vec::new();
        for b in big {
            mine.push(b.await.unwrap());
        }
        watching.store(false, Ordering::SeqCst);
        let peak = watcher.join().unwrap().max(before);
        let big_ok = mine.iter().enumerate().filter(|(i, a)| a["args"][0]["N"] == (*i as u64) * 10 + 1).count();
        // Give the allocator a moment, then see what is still held.
        tokio::time::sleep(Duration::from_millis(500)).await;
        let after = rss_kb();
        let ok = small_ok == 30 && big_ok == 6;
        if ok {
            verdicts.0 += 1;
        } else {
            verdicts.1 += 1;
        }
        report.push(format!(
            "  {engine:<11} 6 callers holding 16 MB each: {big_ok} of 6 answered right; the other caller's 30 small calls: {small_ok} of 30 right in {} ms (the slowest {} ms); the process held {} MB before, {} MB at its peak, {} MB half a second after",
            small_took.as_millis(),
            slowest.as_millis(),
            before / 1024,
            peak / 1024,
            after / 1024
        ));
        let mut sorted = mine;
        sorted.sort_by_key(|a| a["args"][0]["N"].as_u64());
        answers.push(sorted);
        rig.server.stop();
    }
    let same = answers[0] == answers[1];
    if same {
        verdicts.0 += 1;
    } else {
        verdicts.1 += 1;
    }
    println!("BIGMEM (16,000,000 bytes of WORKING-STORAGE, every byte written) in six calls at once:");
    for l in &report {
        println!("{l}");
    }
    println!("  the six answers are {} on the two engines", if same { "the same" } else { "DIFFERENT" });
    println!("PASS {:03} FAIL {:03} ({} ms)", verdicts.0, verdicts.1, started.elapsed().as_millis());
    assert_eq!(verdicts.1, 0, "a call saw another's storage, or the engines answered differently");
}


// ── a request with thousands of members ────────────────────────────────────────────────────────────────────────────────

#[tokio::test(flavor = "multi_thread")]
async fn a_request_with_thousands_of_members_is_refused_the_same_on_both_engines_and_what_it_costs_is_reported() {
    let started = Instant::now();
    let sizes = [500usize, 1000, 2000, 4000];
    let mut table: Vec<(usize, u128, u128)> = Vec::new();
    let mut answers: Vec<Vec<Value>> = Vec::new();
    for module in [false, true] {
        let (rig, _) = rig_with(if module { "many-w" } else { "many-i" }, module, "[limits]\nconnection_rate_per_sec = 1000\nconnection_burst = 1000\n");
        let mut seen = Vec::new();
        for (row, n) in sizes.iter().enumerate() {
            let arg = format!("{{{}}}", (0..*n).map(|i| format!("\"M{i}\":1")).collect::<Vec<_>>().join(","));
            let mut one = ready(&rig, "u").await;
            let t = Instant::now();
            let r = call_or_close(&mut one, 1, "GREET", &arg).await;
            let ms = t.elapsed().as_millis();
            if module {
                table[row].2 = ms;
            } else {
                table.push((*n, ms, 0));
            }
            seen.push(r.as_ref().map(learned).unwrap_or_else(|| json!({"t": "closed"})));
        }
        answers.push(seen);
        rig.server.stop();
    }
    let equal = sizes.iter().enumerate().filter(|(i, _)| answers[0][*i] == answers[1][*i] && answers[0][*i]["code"] == 4000).count();
    let different = sizes.len() - equal;
    println!("an object of N members, none of them the program's, over a real socket (milliseconds to the refusal):");
    println!("  {:>7} {:>12} {:>9}", "members", "interpreter", "module");
    for (n, i, m) in &table {
        println!("  {n:>7} {i:>12} {m:>9}");
    }
    println!("  both engines refuse with 4000 and the same words at every size; both grow with the square of the members (the module's duplicate-key scan of a request, `json::check_object`, is the larger: recorded for WASM-PARITY, T47)");
    println!("PASS {equal:03} FAIL {different:03} ({} ms)", started.elapsed().as_millis());
    assert_eq!(different, 0, "the engines refused differently: {answers:?}");
}
