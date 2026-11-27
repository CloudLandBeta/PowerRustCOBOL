// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Spec 114 T36 (R5, R6, R8; ABI A60): the Backend's eight verbs, served by the interpreter and by the module, do the same.
//!
//! One small Backend, every program of it written to use a verb, is loaded twice: the interpreter's engine runs it from its
//! sources, the Backend runner's engine runs the module the generator made of the same sources. Each case calls a program
//! with the hooks a server gives a call (the caller's claims, the file service, the progress and the stream sinks, a cancel the
//! caller asks for, a stop the server enforces) and everything a caller could learn is recorded: the answer or the failure
//! (class, code, message, detail), every progress report, every item sent. The two records must be equal, case by case.
//!
//! What a verb is for is asserted too, not only that both agree: a `WS-FAIL` is the failure, with the parameters it changed
//! not sent back; the progress is clamped and cut; a claim is the caller's; a file id the caller never uploaded is `NOFILE`;
//! a cancel the caller asks for ends a loop that asks `WS-CANCELLED`; a stop the server enforces ends a `WS-PAUSE`.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use cobolt_backend_runner::payload::{self, Package};
use cobolt_program_package::backend::BackendContainer;
use cobolt_server::backend::Backend;
use cobolt_server::engine::{Engine, ExecOptions, ExecResult};
use cobolt_server::exec::InterpreterEngine;
use cobolt_server::testkit::backend_dir;
use cobolt_wasm::{compile_backend, BackendProgram, Options};
use cobolt_wire::call::BackendFiles;
use cobolt_wire::error::{codes, CallError};

const EXPORTS: &str = r#"
[contract]
version = "1.0.0"

[[export]]
program = "FAILER"

[[export]]
program = "PROGRESSER"

[[export]]
program = "CLAIMER"

[[export]]
program = "FILER"

[[export]]
program = "WATCHER"

[[export]]
program = "PAUSER"

[[export]]
program = "TICKER"
stream = true

[[export]]
program = "EMIT-IN-CALL"
"#;

const FAILER: &str = r#"
IDENTIFICATION DIVISION.
PROGRAM-ID. FAILER.
DATA DIVISION.
LINKAGE SECTION.
01 LK.
    05 CODE-IN PIC 9(5).
    05 NOTE    PIC X(30).
PROCEDURE DIVISION USING LK.
MAIN-PARA.
    MOVE "changed by the program" TO NOTE
    COBOL::"WS-FAIL" ( CODE-IN "declined by the bank   " "retry=no" )
    MOVE "after the failure" TO NOTE
    GOBACK.
"#;

const PROGRESSER: &str = r#"
IDENTIFICATION DIVISION.
PROGRAM-ID. PROGRESSER.
DATA DIVISION.
LINKAGE SECTION.
01 LK.
    05 PCT  PIC 9(3)V9.
    05 DONE PIC X(4).
PROCEDURE DIVISION USING LK.
MAIN-PARA.
    COBOL::"WS-PROGRESS" ( 0 "starting" )
    COBOL::"WS-PROGRESS" ( PCT "going on   " )
    COBOL::"WS-PROGRESS" ( 150 "past the end" )
    MOVE "done" TO DONE
    GOBACK.
"#;

const CLAIMER: &str = r#"
IDENTIFICATION DIVISION.
PROGRAM-ID. CLAIMER.
DATA DIVISION.
LINKAGE SECTION.
01 LK.
    05 WHO   PIC X(20).
    05 NOSUCH PIC X(20).
PROCEDURE DIVISION USING LK.
MAIN-PARA.
    COBOL::"WS-CLAIM" ( "sub" WHO )
    COBOL::"WS-CLAIM" ( "missing" NOSUCH )
    GOBACK.
"#;

const FILER: &str = r#"
IDENTIFICATION DIVISION.
PROGRAM-ID. FILER.
DATA DIVISION.
WORKING-STORAGE SECTION.
01 WS-PATH PIC X(300).
LINKAGE SECTION.
01 LK.
    05 FILE-ID PIC X(32).
    05 FOUND   PIC X(10).
    05 SHOWN   PIC X(40).
    05 NEW-ID  PIC X(32).
    05 PUB     PIC X(10).
PROCEDURE DIVISION USING LK.
MAIN-PARA.
    COBOL::"WS-FILE-PATH" ( FILE-ID WS-PATH FOUND )
    MOVE WS-PATH TO SHOWN
    COBOL::"WS-FILE-PUBLISH" ( WS-PATH NEW-ID PUB )
    GOBACK.
"#;

const WATCHER: &str = r#"
IDENTIFICATION DIVISION.
PROGRAM-ID. WATCHER.
DATA DIVISION.
WORKING-STORAGE SECTION.
01 ASKED PIC X VALUE "N".
01 N     PIC 9(5) VALUE 0.
LINKAGE SECTION.
01 LK.
    05 TURNS PIC 9(5).
    05 SEEN  PIC 9(5).
PROCEDURE DIVISION USING LK.
MAIN-PARA.
    PERFORM UNTIL ASKED = "Y" OR N >= TURNS
        ADD 1 TO N
        COBOL::"WS-PAUSE" ( 5 )
        COBOL::"WS-CANCELLED" ( ASKED )
    END-PERFORM
    MOVE N TO SEEN
    GOBACK.
"#;

const PAUSER: &str = r#"
IDENTIFICATION DIVISION.
PROGRAM-ID. PAUSER.
DATA DIVISION.
LINKAGE SECTION.
01 LK.
    05 MS PIC 9(4).
PROCEDURE DIVISION USING LK.
MAIN-PARA.
    COBOL::"WS-PAUSE" ( MS )
    GOBACK.
"#;

const TICKER: &str = r#"
IDENTIFICATION DIVISION.
PROGRAM-ID. TICKER.
DATA DIVISION.
WORKING-STORAGE SECTION.
01 I PIC 99.
01 TICK.
    05 SEQ PIC 9(4) VALUE 0.
LINKAGE SECTION.
01 LK.
    05 HOW-MANY PIC 99.
PROCEDURE DIVISION USING LK.
MAIN-PARA.
    PERFORM VARYING I FROM 1 BY 1 UNTIL I > HOW-MANY
        ADD 1 TO SEQ
        COBOL::"WS-EMIT" ( TICK )
    END-PERFORM
    GOBACK.
"#;

const EMIT_IN_CALL: &str = r#"
IDENTIFICATION DIVISION.
PROGRAM-ID. EMIT-IN-CALL.
DATA DIVISION.
WORKING-STORAGE SECTION.
01 TICK.
    05 SEQ PIC 9(4) VALUE 7.
LINKAGE SECTION.
01 LK.
    05 N PIC 99.
PROCEDURE DIVISION USING LK.
MAIN-PARA.
    COBOL::"WS-EMIT" ( TICK )
    GOBACK.
"#;

const SOURCES: [(&str, &str); 8] = [
    ("failer.cbl", FAILER),
    ("progresser.cbl", PROGRESSER),
    ("claimer.cbl", CLAIMER),
    ("filer.cbl", FILER),
    ("watcher.cbl", WATCHER),
    ("pauser.cbl", PAUSER),
    ("ticker.cbl", TICKER),
    ("emit-in-call.cbl", EMIT_IN_CALL),
];

// ── the two engines ─────────────────────────────────────────────────────────────────────────────────────────────────────────

/// The Backend the runner serves from the module, as a Build puts the programs in a copy of the runner and the runner reads
/// them back.
fn module_engine(source: &Backend) -> (Backend, Arc<dyn Engine>, usize) {
    let mut programs: Vec<_> = source.programs().collect();
    programs.sort_by(|a, b| a.sig.name.cmp(&b.sig.name));
    let list: Vec<BackendProgram<'_>> =
        programs.iter().map(|p| BackendProgram { name: p.sig.name.clone(), program: p.program.as_ref().expect("the sources of the Backend") }).collect();
    let wasm = compile_backend(&list, &Options { native: true, ..Options::default() }).unwrap_or_else(|e| panic!("the generator refused the Backend: {e}")).wasm;
    let size = wasm.len();
    let container = BackendContainer {
        state: "per-call".into(),
        interface: source.interface.to_json().into_bytes(),
        exports: serde_json::to_vec(&source.export_entries()).unwrap(),
        history: b"[]".to_vec(),
        slots: b"[]".to_vec(),
        module: wasm,
        name: "verbs".to_string(),
    };
    let loaded = payload::load(Package { container, code: None }, &cobolt_backend_runner::provided()).unwrap_or_else(|e| panic!("the runner refuses the Backend: {e:?}"));
    (loaded.backend, loaded.engine.expect("the runner's engine"), size)
}

// ── what a call is given, and what it leaves ───────────────────────────────────────────────────────────────────────────────

/// The file service of a call: the caller uploaded one file, `f1`, and may publish that file's path and nothing else.
struct Files;

impl BackendFiles for Files {
    fn resolve(&self, id: &str) -> Option<PathBuf> {
        (id == "f1").then(|| PathBuf::from("/data/uploads/f1.txt"))
    }
    fn publish(&self, path: &Path) -> Result<String, String> {
        if path == Path::new("/data/uploads/f1.txt") {
            Ok("pub-1".into())
        } else {
            Err("not a file of this caller".into())
        }
    }
}

#[derive(Clone)]
struct Case {
    name: &'static str,
    program: &'static str,
    args: Vec<String>,
    claims: Vec<(&'static str, &'static str)>,
    files: bool,
    /// The program is a stream: the sink is given, and says no after this many items (`None`: never).
    stream_stops_after: Option<usize>,
    /// The caller asks to cancel this long after the call starts.
    cancel_asked_after: Option<Duration>,
    /// The server stops the call this long after it starts.
    stopped_after: Option<Duration>,
}

fn case(name: &'static str, program: &'static str, arg: &str) -> Case {
    Case { name, program, args: vec![arg.to_string()], claims: Vec::new(), files: false, stream_stops_after: None, cancel_asked_after: None, stopped_after: None }
}

/// What a caller could learn from one call.
#[derive(Debug, PartialEq)]
struct Seen {
    answer: Result<(Vec<String>, Option<String>), CallError>,
    progress: Vec<(u8, String)>,
    items: Vec<String>,
}

fn run(engine: &dyn Engine, backend: &Backend, c: &Case) -> (Seen, Duration) {
    let prog = backend.program(c.program).unwrap_or_else(|| panic!("{}: the Backend has no program {}", c.name, c.program));
    let cancel = Arc::new(AtomicBool::new(false));
    let mut opts = ExecOptions::plain(cancel.clone(), Duration::from_millis(200));
    opts.claims = c.claims.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect::<HashMap<_, _>>();
    if c.files {
        opts.files = Some(Arc::new(Files));
    }
    let progress = Arc::new(Mutex::new(Vec::new()));
    let p = progress.clone();
    opts.progress = Some(Box::new(move |pct, msg| p.lock().unwrap().push((pct, msg.to_string()))));
    let items = Arc::new(Mutex::new(Vec::new()));
    if prog.sig.stream {
        let (i, stops) = (items.clone(), c.stream_stops_after);
        opts.emit = Some(Box::new(move |json| {
            let mut v = i.lock().unwrap();
            v.push(json.to_string());
            stops.map_or(true, |n| v.len() < n)
        }));
    }
    // The caller's cancel and the server's stop, from another thread, at the times the case names.
    let mut timers = Vec::new();
    for (after, flag) in [(c.cancel_asked_after, opts.cancel_requested.clone()), (c.stopped_after, cancel)] {
        if let Some(d) = after {
            timers.push(std::thread::spawn(move || {
                std::thread::sleep(d);
                flag.store(true, Ordering::SeqCst);
            }));
        }
    }
    let started = Instant::now();
    let result: Result<ExecResult, CallError> = engine.execute(prog, &c.args, opts);
    let took = started.elapsed();
    for t in timers {
        let _ = t.join();
    }
    let answer = result.map(|r| (r.args, r.returning));
    let progress = progress.lock().unwrap().clone();
    let items = items.lock().unwrap().clone();
    (Seen { answer, progress, items }, took)
}

fn cases() -> Vec<Case> {
    let mut v = Vec::new();
    // WS-FAIL: the failure, with the parameters it changed not sent back.
    v.push(case("WS-FAIL 4201 with a message and a detail", "FAILER", r#"{"CODE-IN":4201,"NOTE":"untouched"}"#));
    v.push(case("WS-FAIL 1, the lowest code", "FAILER", r#"{"CODE-IN":1,"NOTE":""}"#));
    v.push(case("WS-FAIL 7999, the highest code", "FAILER", r#"{"CODE-IN":7999,"NOTE":""}"#));
    v.push(case("WS-FAIL 0: a code outside the range is a run-time error", "FAILER", r#"{"CODE-IN":0,"NOTE":""}"#));
    v.push(case("WS-FAIL 8000: the platform's codes are not the program's", "FAILER", r#"{"CODE-IN":8000,"NOTE":""}"#));
    // WS-PROGRESS: clamped, cut, in order.
    v.push(case("WS-PROGRESS 33.5, rounded", "PROGRESSER", r#"{"PCT":33.5,"DONE":""}"#));
    v.push(case("WS-PROGRESS 0", "PROGRESSER", r#"{"PCT":0,"DONE":""}"#));
    v.push(case("WS-PROGRESS 99.9", "PROGRESSER", r#"{"PCT":99.9,"DONE":""}"#));
    // WS-CLAIM.
    let mut c = case("WS-CLAIM of a claim the caller has and one it has not", "CLAIMER", r#"{"WHO":"","NOSUCH":"x"}"#);
    c.claims = vec![("sub", "user-42"), ("role", "admin")];
    v.push(c);
    v.push(case("WS-CLAIM with no claims at all", "CLAIMER", r#"{"WHO":"","NOSUCH":"x"}"#));
    // WS-FILE-PATH and WS-FILE-PUBLISH.
    let mut c = case("the file the caller uploaded: path and publish are OK", "FILER", r#"{"FILE-ID":"f1","FOUND":"","SHOWN":"","NEW-ID":"","PUB":""}"#);
    c.files = true;
    v.push(c);
    let mut c = case("a file id the caller never uploaded is NOFILE", "FILER", r#"{"FILE-ID":"zz","FOUND":"","SHOWN":"","NEW-ID":"","PUB":""}"#);
    c.files = true;
    v.push(c);
    v.push(case("a call with no file service says NOBACKEND", "FILER", r#"{"FILE-ID":"f1","FOUND":"","SHOWN":"","NEW-ID":"","PUB":""}"#));
    // WS-CANCELLED and WS-PAUSE.
    v.push(case("WS-CANCELLED stays N and the loop runs its turns", "WATCHER", r#"{"TURNS":4,"SEEN":0}"#));
    let mut c = case("a cancel the caller asks for ends a loop that asks WS-CANCELLED", "WATCHER", r#"{"TURNS":600,"SEEN":0}"#);
    c.cancel_asked_after = Some(Duration::from_millis(80));
    v.push(c);
    let mut c = case("a stop the server enforces ends a WS-PAUSE loop as a time-out", "WATCHER", r#"{"TURNS":60000,"SEEN":0}"#);
    c.stopped_after = Some(Duration::from_millis(80));
    v.push(c);
    v.push(case("WS-PAUSE 60 waits", "PAUSER", r#"{"MS":60}"#));
    let mut c = case("a cancel the caller asks for ends a long WS-PAUSE early", "PAUSER", r#"{"MS":5000}"#);
    c.cancel_asked_after = Some(Duration::from_millis(80));
    v.push(c);
    let mut c = case("a stop the server enforces ends a long WS-PAUSE", "PAUSER", r#"{"MS":5000}"#);
    c.stopped_after = Some(Duration::from_millis(80));
    v.push(c);
    // WS-EMIT.
    v.push(case("a stream sends its items in order and ends", "TICKER", r#"{"HOW-MANY":5}"#));
    v.push(case("a stream that sends nothing", "TICKER", r#"{"HOW-MANY":0}"#));
    let mut c = case("a stream nobody listens to any more is cancelled", "TICKER", r#"{"HOW-MANY":9}"#);
    c.stream_stops_after = Some(3);
    v.push(c);
    v.push(case("WS-EMIT in a program that is not a stream is a run-time error", "EMIT-IN-CALL", r#"{"N":1}"#));
    v
}

#[test]
fn the_eight_verbs_do_the_same_on_the_interpreter_and_on_the_module() {
    let started = Instant::now();
    let dir = backend_dir("verbs", EXPORTS, &SOURCES);
    let source = Backend::load(&dir).unwrap_or_else(|e| panic!("the Backend does not load: {e:#?}"));
    let interp_engine = InterpreterEngine;
    let t = Instant::now();
    let (on_module, wasm_engine, module_bytes) = module_engine(&source);
    let build_ms = t.elapsed().as_millis();
    assert_eq!(wasm_engine.name(), "wasm");

    let (mut equal, mut different) = (0usize, Vec::new());
    let (mut interp_ms, mut module_ms) = (0u128, 0u128);
    let mut seen_by_case: Vec<(&'static str, Seen)> = Vec::new();
    for c in cases() {
        let (a, ta) = run(&interp_engine, &source, &c);
        let (b, tb) = run(wasm_engine.as_ref(), &on_module, &c);
        interp_ms += ta.as_millis();
        module_ms += tb.as_millis();
        if a == b {
            equal += 1;
        } else {
            different.push(format!("{}:\n  interpreter: {a:?}\n  module:      {b:?}", c.name));
        }
        seen_by_case.push((c.name, b));
    }
    assert!(different.is_empty(), "the engines answer differently:\n{}", different.join("\n"));

    // What the verbs are for, asserted on the module's own record (equal to the interpreter's, above).
    let by = |name: &str| &seen_by_case.iter().find(|(n, _)| *n == name).unwrap_or_else(|| panic!("no case {name}")).1;
    let failure = |name: &str| by(name).answer.as_ref().unwrap_err().clone();

    let f = failure("WS-FAIL 4201 with a message and a detail");
    assert_eq!((f.code, f.message.as_str(), f.detail.as_deref()), (4201, "declined by the bank", Some("retry=no")), "{f:?}");
    assert_eq!(failure("WS-FAIL 1, the lowest code").code, 1);
    assert_eq!(failure("WS-FAIL 7999, the highest code").code, 7999);
    for name in ["WS-FAIL 0: a code outside the range is a run-time error", "WS-FAIL 8000: the platform's codes are not the program's"] {
        assert_eq!(failure(name).code, codes::PROGRAM_FAULT, "{name}");
    }

    let p = by("WS-PROGRESS 33.5, rounded");
    assert_eq!(p.progress, vec![(0, "starting".into()), (34, "going on".into()), (100, "past the end".into())], "{p:?}");
    assert!(p.answer.is_ok());
    assert_eq!(by("WS-PROGRESS 99.9").progress[1].0, 100);

    let c = by("WS-CLAIM of a claim the caller has and one it has not").answer.as_ref().unwrap();
    assert!(c.0[0].contains("\"WHO\":\"user-42\"") && c.0[0].contains("\"NOSUCH\":\"\""), "{c:?}");
    let c = by("WS-CLAIM with no claims at all").answer.as_ref().unwrap();
    assert!(c.0[0].contains("\"WHO\":\"\""), "{c:?}");

    let f = by("the file the caller uploaded: path and publish are OK").answer.as_ref().unwrap().0[0].clone();
    assert!(f.contains("\"FOUND\":\"OK\"") && f.contains("/data/uploads/f1.txt") && f.contains("\"NEW-ID\":\"pub-1\"") && f.contains("\"PUB\":\"OK\""), "{f}");
    let f = by("a file id the caller never uploaded is NOFILE").answer.as_ref().unwrap().0[0].clone();
    assert!(f.contains("\"FOUND\":\"NOFILE\"") && f.contains("\"PUB\":\"NOFILE\""), "{f}");
    let f = by("a call with no file service says NOBACKEND").answer.as_ref().unwrap().0[0].clone();
    assert!(f.contains("\"FOUND\":\"NOBACKEND\"") && f.contains("\"PUB\":\"NOBACKEND\""), "{f}");

    assert!(by("WS-CANCELLED stays N and the loop runs its turns").answer.as_ref().unwrap().0[0].contains("\"SEEN\":4"));
    for name in ["a cancel the caller asks for ends a loop that asks WS-CANCELLED", "a cancel the caller asks for ends a long WS-PAUSE early"] {
        assert_eq!(failure(name).code, codes::CANCELLED, "{name}");
    }
    for name in ["a stop the server enforces ends a WS-PAUSE loop as a time-out", "a stop the server enforces ends a long WS-PAUSE"] {
        assert_eq!(failure(name).code, codes::CALL_TIMEOUT, "{name}");
    }
    assert!(by("WS-PAUSE 60 waits").answer.is_ok());

    let t = by("a stream sends its items in order and ends");
    assert_eq!(t.items, (1..=5).map(|n| format!("{{\"SEQ\":{n}}}")).collect::<Vec<_>>(), "{t:?}");
    assert!(t.answer.as_ref().unwrap().0.is_empty(), "a stream has no answer to read back");
    assert!(by("a stream that sends nothing").items.is_empty());
    let t = by("a stream nobody listens to any more is cancelled");
    assert_eq!(t.items.len(), 3, "it stops at the item nobody took: {t:?}");
    assert_eq!(failure("a stream nobody listens to any more is cancelled").code, codes::CANCELLED);
    assert_eq!(failure("WS-EMIT in a program that is not a stream is a run-time error").code, codes::PROGRAM_FAULT);

    let cases_run = seen_by_case.len();
    let answered = seen_by_case.iter().filter(|(_, s)| s.answer.is_ok()).count();
    let progress: usize = seen_by_case.iter().map(|(_, s)| s.progress.len()).sum();
    let items: usize = seen_by_case.iter().map(|(_, s)| s.items.len()).sum();
    println!(
        "=== BACKEND VERBS (spec 114 T36, ABI A60) ===\n  the eight verbs ({} programs, {} cases) on the interpreter and on the module: {equal} of {cases_run} cases equal\n  answered {answered}, failed {} (WS-FAIL, a code out of range, cancelled, timed out, a stream nobody listens to, a run-time error)\n  progress reports {progress}, stream items {items}; module {module_bytes} bytes, built in {build_ms} ms\n  time on the interpreter {interp_ms} ms, on the module {module_ms} ms\nPASS {equal:03} FAIL {:03} ({} ms)",
        SOURCES.len(),
        cases_run,
        cases_run - answered,
        different.len(),
        started.elapsed().as_millis()
    );
}
