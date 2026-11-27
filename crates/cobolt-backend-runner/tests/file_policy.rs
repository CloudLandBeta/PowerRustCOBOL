// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Spec 114 T38 (R8, R20, AC3): the files two connections share, on the interpreter's engine and on the module's.
//!
//! A Backend serves several callers at once, so the server gives its programs a file policy (WASM-003 R19, the setting
//! `file_lock_wait_ms`): a program that opens a file for anything but `INPUT` holds it alone until its `CLOSE`, readers share
//! it, and an `OPEN` that cannot get the file within the wait is file status 93. It was found by the channel suites run on the
//! module (`loopback`: "calls that share a file take turns", "a file held open across calls"): the module's file host took
//! the lock without waiting and without the writer's exclusivity, so two writers met in the container and one got status 90.
//! This test holds the policy for every organisation a program can declare, with the same script of calls made to each engine.
//!
//! The script, per organisation, with the wait set to 250 ms: A opens for output and leaves the file open; B's `OPEN INPUT`
//! waits the 250 ms and is told 93; A closes; B reads; two readers share; a writer is refused while a reader holds the file;
//! and a reader that waits for a writer which closes in time gets the file, having waited.

use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use std::time::{Duration, Instant};

use cobolt_server::backend::{Backend, StateMode};
use cobolt_server::engine::{Engine, EngineSession, ExecOptions};
use cobolt_server::exec::InterpreterEngine;
use cobolt_server::testkit::backend_dir;

const WAIT: Duration = Duration::from_millis(250);

/// The four organisations a program can declare, with what each needs besides `ORGANIZATION`.
struct Org {
    name: &'static str,
    clause: &'static str,
    record: &'static str,
    extra_storage: &'static str,
}

const ORGS: [Org; 4] = [
    Org { name: "INDEXED", clause: "ORGANIZATION IS INDEXED\n        ACCESS MODE IS DYNAMIC\n        RECORD KEY IS K", record: "    05 K PIC X(4).\n    05 V PIC X(4).", extra_storage: "" },
    Org { name: "RELATIVE", clause: "ORGANIZATION IS RELATIVE\n        ACCESS MODE IS DYNAMIC\n        RELATIVE KEY IS RK", record: "    05 V PIC X(8).", extra_storage: "01 RK PIC 9(4) VALUE 1.\n" },
    Org { name: "SEQUENTIAL", clause: "ORGANIZATION IS SEQUENTIAL", record: "    05 V PIC X(8).", extra_storage: "" },
    Org { name: "LINE SEQUENTIAL", clause: "ORGANIZATION IS LINE SEQUENTIAL", record: "    05 V PIC X(8).", extra_storage: "" },
];

/// The program of one organisation: `OP` says what to do (`O` open output, `I` open input, `C` close) and `OUT` is the file status.
fn program(name: &str, org: &Org, path: &str) -> String {
    format!(
        "IDENTIFICATION DIVISION.\nPROGRAM-ID. {name}.\nENVIRONMENT DIVISION.\nINPUT-OUTPUT SECTION.\nFILE-CONTROL.\n    SELECT F ASSIGN TO \"{path}\"\n        {clause}\n        FILE STATUS IS ST.\nDATA DIVISION.\nFILE SECTION.\nFD F.\n01 REC.\n{record}\nWORKING-STORAGE SECTION.\n01 ST PIC XX.\n{extra}LINKAGE SECTION.\n01 LK.\n    05 OP PIC X.\n    05 OUT PIC XX.\nPROCEDURE DIVISION USING LK.\nMAIN-PARA.\n    EVALUATE OP\n        WHEN \"O\"\n            OPEN OUTPUT F\n        WHEN \"I\"\n            OPEN INPUT F\n        WHEN \"C\"\n            CLOSE F\n    END-EVALUATE\n    MOVE ST TO OUT\n    GOBACK.\n",
        clause = org.clause,
        record = org.record,
        extra = org.extra_storage,
    )
}

/// The Backend that keeps a connection's state, and the engine that serves it: the interpreter's, or the module as a Build makes it.
fn served(dir: &std::path::Path, module: bool) -> (Backend, Arc<dyn Engine>) {
    let mut backend = Backend::load(dir).unwrap_or_else(|e| panic!("the Backend does not load: {e:#?}"));
    backend.state = StateMode::PerConnection;
    if !module {
        return (backend, Arc::new(InterpreterEngine));
    }
    let compiled = cobolt_compiler::backend_module::generate(&backend, &[]).unwrap_or_else(|r| panic!("the generator refuses the Backend: {r:#?}"));
    let engine = cobolt_backend_runner::engine::WasmEngine::start(&compiled.wasm, None, &cobolt_backend_runner::provided()).unwrap_or_else(|e| panic!("the runner refuses the module: {e}"));
    (backend, Arc::new(engine))
}

/// One call of the organisation's program in a connection: the file status it answers, and how long the call took.
fn ask(session: &mut dyn EngineSession, backend: &Backend, program: &str, op: &str, wait: Duration) -> (String, Duration) {
    let prog = backend.program(program).unwrap_or_else(|| panic!("no program {program}"));
    let opts = ExecOptions::plain(Arc::new(AtomicBool::new(false)), wait);
    let started = Instant::now();
    let r = session.execute(prog, &[format!(r#"{{"OP":"{op}","OUT":"  "}}"#)], opts);
    let took = started.elapsed();
    let args = r.unwrap_or_else(|e| panic!("{program} {op}: the call failed: {e:?}")).args;
    let at = args[0].find("\"OUT\":\"").expect("OUT in the answer") + 7;
    (args[0][at..at + 2].to_string(), took)
}

/// What one engine did with the script for one organisation: the status of each step, and whether each wait was kept.
#[derive(Debug, PartialEq)]
struct Record {
    statuses: Vec<(&'static str, String)>,
    waited: Vec<(&'static str, bool)>,
}

fn script(engine: &dyn Engine, backend: &Backend, program: &str) -> Record {
    let mut a = engine.session();
    let mut b = engine.session();
    let mut statuses = Vec::new();
    let mut waited = Vec::new();
    let mut step = |label: &'static str, s: (String, Duration), expect_wait: Option<bool>| {
        statuses.push((label, s.0));
        if let Some(long) = expect_wait {
            // A call that is refused after the wait took the wait; one that is served at once did not.
            waited.push((label, (s.1 >= WAIT - Duration::from_millis(20)) == long));
        }
    };
    // A writes (and so creates the file) and keeps it open: B, who only reads, waits the whole 250 ms and is told 93.
    step("A opens for output", ask(a.as_mut(), backend, program, "O", WAIT), Some(false));
    step("B opens for input while A writes", ask(b.as_mut(), backend, program, "I", WAIT), Some(true));
    step("A closes", ask(a.as_mut(), backend, program, "C", WAIT), None);
    // The file is free: readers share it.
    step("B opens for input", ask(b.as_mut(), backend, program, "I", WAIT), Some(false));
    step("A opens for input too", ask(a.as_mut(), backend, program, "I", WAIT), Some(false));
    // …and a writer is kept out while a reader holds it.
    step("A closes the reader", ask(a.as_mut(), backend, program, "C", WAIT), None);
    step("A writes while B reads", ask(a.as_mut(), backend, program, "O", WAIT), Some(true));
    step("B closes", ask(b.as_mut(), backend, program, "C", WAIT), None);
    // A reader that waits for a writer who closes in time gets the file, having waited.
    step("A opens for output again", ask(a.as_mut(), backend, program, "O", WAIT), Some(false));
    let (late, closed) = std::thread::scope(|s| {
        let reader = s.spawn(|| ask(b.as_mut(), backend, program, "I", Duration::from_secs(5)));
        std::thread::sleep(Duration::from_millis(300));
        let closing = ask(a.as_mut(), backend, program, "C", WAIT);
        (reader.join().expect("the reader's thread"), closing)
    });
    step("A closes after 300 ms", closed, None);
    step("B, who waited for it, opens for input", late, Some(true));
    step("B closes at the end", ask(b.as_mut(), backend, program, "C", WAIT), None);
    Record { statuses, waited }
}

#[test]
fn writers_hold_a_file_alone_readers_share_and_an_open_waits_the_same_on_both_engines_for_every_organisation() {
    let started = Instant::now();
    let data = std::env::temp_dir().join(format!("prc-policy-{}", std::process::id()));
    std::fs::create_dir_all(&data).unwrap();
    let mut passed = 0u32;
    let mut failed = 0u32;
    let mut lines = Vec::new();
    for (i, org) in ORGS.iter().enumerate() {
        let name = format!("POLICY-{}", i + 1);
        let path = data.join(format!("f{}.dat", i + 1));
        let exports = format!("[contract]\nversion = \"1.0.0\"\n\n[[export]]\nprogram = \"{name}\"\n");
        let source = program(&name, org, &path.display().to_string());
        let dir = backend_dir(&format!("policy-{}", i + 1), &exports, &[("policy.cbl", source.as_str())]);
        let mut records = Vec::new();
        let mut times = Vec::new();
        for module in [false, true] {
            let _ = std::fs::remove_file(&path);
            let _ = std::fs::remove_file(format!("{}.lck", path.display()));
            let t = Instant::now();
            let (backend, engine) = served(&dir, module);
            records.push(script(engine.as_ref(), &backend, &name));
            times.push(t.elapsed().as_millis());
        }
        // What the policy says, held for both engines: the statuses (93 only where a lock is in the way) and the waits.
        let expected = ["00", "93", "00", "00", "00", "00", "93", "00", "00", "00", "00", "00"];
        for (engine, r) in ["interpreter", "module"].iter().zip(&records) {
            let got: Vec<&str> = r.statuses.iter().map(|(_, s)| s.as_str()).collect();
            if got == expected && r.waited.iter().all(|(_, ok)| *ok) {
                passed += 1;
            } else {
                failed += 1;
                println!("FAIL {} on the {engine}: statuses {:?}, waits {:?}", org.name, r.statuses, r.waited);
            }
        }
        if records[0] == records[1] {
            passed += 1;
        } else {
            failed += 1;
            println!("FAIL {}: the engines differ: {:?} against {:?}", org.name, records[0], records[1]);
        }
        lines.push(format!(
            "  {:<16} {} steps on each engine, 93 only while a lock was in the way, the waits kept; interpreter {} ms, module {} ms",
            org.name,
            records[0].statuses.len(),
            times[0],
            times[1]
        ));
    }
    let _ = std::fs::remove_dir_all(&data);
    println!("\nFILE POLICY (file_lock_wait = {} ms): 4 organisations, the same script on the interpreter and on the module", WAIT.as_millis());
    for l in &lines {
        println!("{l}");
    }
    println!("PASS {passed:03} FAIL {failed:03} ({} ms)", started.elapsed().as_millis());
    assert_eq!(failed, 0, "the engines must give the same file policy");
}
