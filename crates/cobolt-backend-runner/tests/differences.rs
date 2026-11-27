// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Spec 114 T47 (R20, AC3): the differences between the interpreter's engine and the module's that the earlier tasks found and
//! left for the record, each measured on both engines, so that `specs/WASM-PARITY.md` states what happens and not what was
//! remembered.
//!
//! Each case prints what each engine answered and asserts what the record says. A case that stops differing fails here, and
//! the record is then to be corrected: a difference that closed is news, and so is one that was never there.
//!
//! * `DISPLAY` in a Backend program: what a call reports of it (`ExecResult::display`, the debug log only);
//! * an answer the program holds and cannot write (an `OCCURS DEPENDING ON` count it never set): the failure's code and words;
//! * a request refused halfway through a parameter, on a connection that keeps its state: whether the fields read before the
//!   refusal stay in the run unit;
//! * a refusal whose path is longer than the 1,024 bytes the runtime keeps of it.

use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use std::time::{Duration, Instant};

use cobolt_backend_runner::payload::{self, Package};
use cobolt_program_package::backend::BackendContainer;
use cobolt_server::backend::{Backend, StateMode};
use cobolt_server::engine::{Engine, ExecOptions};
use cobolt_server::exec::InterpreterEngine;
use cobolt_server::testkit::backend_dir;
use cobolt_wasm::{compile_backend, BackendOptions, BackendProgram, Options};

const EXPORTS: &str = r#"
[contract]
version = "1.0.0"

[[export]]
program = "SAY"

[[export]]
program = "UNSET"

[[export]]
program = "HALF"
"#;

const SAY: &str = r#"
IDENTIFICATION DIVISION.
PROGRAM-ID. SAY.
DATA DIVISION.
LINKAGE SECTION.
01 LK.
    05 N PIC 9(4).
PROCEDURE DIVISION USING LK.
MAIN-PARA.
    DISPLAY "HELLO FROM THE BACKEND"
    ADD 1 TO N
    GOBACK.
"#;

const UNSET: &str = r#"
IDENTIFICATION DIVISION.
PROGRAM-ID. UNSET.
DATA DIVISION.
LINKAGE SECTION.
01 REQ PIC 9(2).
01 RES.
    05 M PIC 9(2).
    05 CELL PIC X OCCURS 1 TO 5 TIMES DEPENDING ON M.
PROCEDURE DIVISION USING REQ RETURNING RES.
MAIN-PARA.
    IF REQ > 80
        MOVE 9 TO M
    ELSE
        IF REQ > 50
            MOVE 3 TO M
        END-IF
    END-IF.
    GOBACK.
"#;

/// A parameter of two numbers that the program hands back as it finds them.
const HALF: &str = r#"
IDENTIFICATION DIVISION.
PROGRAM-ID. HALF.
DATA DIVISION.
LINKAGE SECTION.
01 P.
    05 A PIC 9(4).
    05 B PIC 9(4).
PROCEDURE DIVISION USING P.
MAIN-PARA.
    GOBACK.
"#;

/// The Backend and the engine that serves it, as a Build makes it and the runner reads it back (the module), or the
/// interpreter's engine on the sources.
fn served(dir: &std::path::Path, per_connection: bool, module: bool) -> (Backend, Arc<dyn Engine>) {
    try_served(dir, per_connection, module).unwrap_or_else(|e| panic!("{e}"))
}

/// [`served`], with the runner's refusal to start the Backend returned instead of raised.
fn try_served(dir: &std::path::Path, per_connection: bool, module: bool) -> Result<(Backend, Arc<dyn Engine>), String> {
    let mut source = Backend::load(dir).unwrap_or_else(|e| panic!("the Backend does not load: {e:#?}"));
    if per_connection {
        source.state = StateMode::PerConnection;
    }
    if !module {
        return Ok((source, Arc::new(InterpreterEngine)));
    }
    let mut programs: Vec<_> = source.programs().collect();
    programs.sort_by(|a, b| a.sig.name.cmp(&b.sig.name));
    let list: Vec<BackendProgram<'_>> =
        programs.iter().map(|p| BackendProgram { name: p.sig.name.clone(), program: p.program.as_ref().expect("the sources of the Backend") }).collect();
    let options = Options { native: true, backend: per_connection.then(|| BackendOptions { per_connection: true, ..BackendOptions::default() }), ..Options::default() };
    let wasm = compile_backend(&list, &options).unwrap_or_else(|e| panic!("the generator refused the Backend: {e}")).wasm;
    let container = BackendContainer {
        state: source.state.as_str().into(),
        interface: source.interface.to_json().into_bytes(),
        exports: serde_json::to_vec(&source.export_entries()).unwrap(),
        history: b"[]".to_vec(),
        slots: b"[]".to_vec(),
        module: wasm,
        name: "differences".to_string(),
    };
    let loaded = payload::load(Package { container, code: None }, &cobolt_backend_runner::provided()).map_err(|e| format!("the runner refuses the Backend: {e:?}"))?;
    Ok((loaded.backend, loaded.engine.expect("the runner's engine")))
}

fn options() -> ExecOptions {
    ExecOptions::plain(Arc::new(AtomicBool::new(false)), Duration::from_millis(200))
}

/// What a caller learns of one call, written so that two of them can be compared and printed.
#[derive(Debug, PartialEq, Clone)]
struct Learned {
    answer: Result<(Vec<String>, Option<String>), String>,
    display: Vec<String>,
}

fn learn(r: Result<cobolt_server::engine::ExecResult, cobolt_wire::error::CallError>) -> Learned {
    match r {
        Ok(r) => Learned { answer: Ok((r.args, r.returning)), display: r.display },
        Err(e) => Learned { answer: Err(format!("{:?} {} {:?} detail {:?}", e.class, e.code, e.message, e.detail)), display: Vec::new() },
    }
}

#[test]
fn the_differences_between_the_two_engines_are_what_the_record_says() {
    let started = Instant::now();
    let (mut passed, mut failed) = (0u32, 0u32);
    let mut report: Vec<String> = Vec::new();
    let mut check = |name: &str, ok: bool, text: String| {
        if ok {
            passed += 1;
        } else {
            failed += 1;
        }
        report.push(format!("{} {name}: {text}", if ok { "PASS" } else { "FAIL" }));
    };

    // ── DISPLAY ──────────────────────────────────────────────────────────────────────────────────
    let dir = backend_dir("differences", EXPORTS, &[("say.cbl", SAY), ("unset.cbl", UNSET), ("half.cbl", HALF)]);
    let (b_i, e_i) = served(&dir, false, false);
    let (b_m, e_m) = served(&dir, false, true);
    let say = |engine: &dyn Engine, backend: &Backend| learn(engine.execute(backend.program("SAY").expect("SAY"), &[r#"{"N":5}"#.to_string()], options()));
    let (di, dm) = (say(e_i.as_ref(), &b_i), say(e_m.as_ref(), &b_m));
    check("DISPLAY: the answer", di.answer == dm.answer, format!("interpreter {:?}, module {:?}", di.answer, dm.answer));
    // Both engines hand the line to the call's record, which the server writes to its debug log and never sends to the caller.
    check(
        "DISPLAY: the line is reported alike (the debug log, never the caller)",
        di.display == vec!["HELLO FROM THE BACKEND".to_string()] && dm.display == di.display,
        format!("interpreter {:?}, module {:?}", di.display, dm.display),
    );

    // ── an answer the program cannot write ───────────────────────────────────────────────────────
    let unset = |engine: &dyn Engine, backend: &Backend, arg: &str| learn(engine.execute(backend.program("UNSET").expect("UNSET"), &[arg.to_string()], options()));
    let (ui, um) = (unset(e_i.as_ref(), &b_i, "5"), unset(e_m.as_ref(), &b_m, "5"));
    check("UNSET a count the program sets: equal", unset(e_i.as_ref(), &b_i, "60") == unset(e_m.as_ref(), &b_m, "60"), "60".to_string());
    let (ei, em) = (ui.answer.clone().err().unwrap_or_default(), um.answer.clone().err().unwrap_or_default());
    check("UNSET a count never set: both refuse it", !ei.is_empty() && !em.is_empty(), format!("interpreter {ei}; module {em}"));
    check("UNSET a count never set: the words differ, the code and class do not", ei != em && ei.split_whitespace().take(2).eq(em.split_whitespace().take(2)), format!("interpreter {ei}; module {em}"));

    // ── a request refused halfway through a parameter, on a connection that keeps its state ──────
    let dir = backend_dir("differences-state", EXPORTS, &[("say.cbl", SAY), ("unset.cbl", UNSET), ("half.cbl", HALF)]);
    let (b_i, e_i) = served(&dir, true, false);
    let (b_m, e_m) = served(&dir, true, true);
    let seq = |engine: &dyn Engine, backend: &Backend| {
        let mut s = engine.session();
        let prog = backend.program("HALF").expect("HALF");
        // A is written, B is refused; then a request that leaves A out.
        let refused = learn(s.execute(prog, &[r#"{"A":1111,"B":"xx"}"#.to_string()], options()));
        let after = learn(s.execute(prog, &[r#"{"B":22}"#.to_string()], options()));
        (refused, after)
    };
    let (si, sm) = (seq(e_i.as_ref(), &b_i), seq(e_m.as_ref(), &b_m));
    check("HALF: the refusal is the same", si.0 == sm.0, format!("interpreter {:?}; module {:?}", si.0.answer, sm.0.answer));
    check("HALF: the request that leaves A out", true, format!("interpreter {:?}; module {:?}", si.1.answer, sm.1.answer));
    // 2.1.61 feared that the fields read before a refusal stay in the kept run unit; for this shape neither engine keeps them.
    check("HALF: nothing read before the refusal is kept (A is 0 again)", si.1 == sm.1 && sm.1.answer == Ok((vec![r#"{"A":0,"B":22}"#.to_string()], None)), format!("interpreter {:?}; module {:?}", si.1.answer, sm.1.answer));

    // ── a parameter nested many groups deep, a leaf of the wrong type at the bottom ──────────────
    // Each case is a program of its own: `DEEPS<n>` has groups of three-character names (a short path), `DEEPL<n>` of thirty
    // (a path of 1 + 31 n + 5 bytes: the runtime keeps 1,024 bytes of a refusal's path).
    const CASES: [(char, usize); 8] = [('S', 4), ('S', 16), ('S', 36), ('L', 20), ('L', 32), ('L', 33), ('L', 34), ('L', 36)];
    let name = |long: bool, i: usize| if long { format!("G{i:02}-ABCDEFGHIJKLMNOPQRSTUVWXYZ") } else { format!("G{i:02}") };
    let mut exports = String::from(EXPORTS);
    let mut sources: Vec<(String, String)> = vec![("say.cbl".into(), SAY.into()), ("unset.cbl".into(), UNSET.into()), ("half.cbl".into(), HALF.into())];
    let mut requests: Vec<(String, usize, String, String)> = Vec::new();
    for (kind, n) in CASES {
        let long = kind == 'L';
        let program = format!("DEEP{kind}{n:02}");
        let mut src = format!("\nIDENTIFICATION DIVISION.\nPROGRAM-ID. {program}.\nDATA DIVISION.\nLINKAGE SECTION.\n01 P.\n");
        for i in 1..=n {
            src.push_str(&format!("    {:02} {}.\n", i + 1, name(long, i)));
        }
        src.push_str(&format!("    {:02} LEAF PIC 9(2).\nPROCEDURE DIVISION USING P.\nMAIN-PARA.\n    GOBACK.\n", n + 2));
        let mut json: String = (1..=n).map(|i| format!("{{\"{}\":", name(long, i))).collect();
        json.push_str("{\"LEAF\":\"xx\"}");
        json.push_str(&"}".repeat(n));
        exports.push_str(&format!("\n[[export]]\nprogram = \"{program}\"\n"));
        sources.push((format!("{}.cbl", program.to_lowercase()), src));
        requests.push((program, 1 + n * (if long { 31 } else { 4 }) + 5, name(long, 1), json));
    }
    let refs: Vec<(&str, &str)> = sources.iter().map(|(a, b)| (a.as_str(), b.as_str())).collect();
    let dir = backend_dir("differences-deep", &exports, &refs);
    let (b_i, e_i) = served(&dir, false, false);
    let (b_m, e_m) = served(&dir, false, true);
    let refused = |engine: &dyn Engine, backend: &Backend, program: &str, json: &str| {
        engine.execute(backend.program(program).expect("a DEEP program"), &[json.to_string()], options()).err().map(|e| (e.code, e.message))
    };
    let (mut short_same, mut short_all) = (0usize, 0usize);
    let mut long_rows: Vec<(usize, bool, String)> = Vec::new();
    for (program, path_bytes, _, json) in &requests {
        let (pi, pm) = (refused(e_i.as_ref(), &b_i, program, json), refused(e_m.as_ref(), &b_m, program, json));
        let same = pi.is_some() && pi == pm;
        if program.starts_with("DEEPS") {
            short_all += 1;
            short_same += same as usize;
        } else {
            let words = |p: &Option<(u32, String)>| p.as_ref().map(|(c, m)| format!("{c} {}", m.split(" at ").next().unwrap_or(""))).unwrap_or_else(|| "answered".into());
            long_rows.push((*path_bytes, same, format!("{path_bytes} bytes: interpreter {}; module {}", words(&pi), words(&pm))));
        }
    }
    check("DEEP, short names: the refusal is the same at 4, 16 and 36 groups", short_same == short_all, format!("{short_same} of {short_all}"));
    // The runtime keeps 1,024 bytes of a refusal's path: a path within them is refused alike, a path past them is refused with the
    // same code and another reason (the module names a key it did not find where the interpreter names the wrong type).
    let within_alike = long_rows.iter().filter(|(bytes, _, _)| *bytes <= 1024).all(|(_, same, _)| *same);
    let past_differs = long_rows.iter().filter(|(bytes, _, _)| *bytes > 1024).all(|(_, same, _)| !*same);
    check(
        "DEEP, long names: a path within 1,024 bytes is refused alike, a path past them is not",
        within_alike && past_differs,
        long_rows.iter().map(|(_, _, t)| t.clone()).collect::<Vec<_>>().join(" | "),
    );

    // ── a parameter nested 41 groups deep (COBOL allows 49 levels) ───────────────────────────────
    // It loads on the interpreter's side; the runner refuses the module at start, reading the interface description back.
    let deep_41 = {
        let mut src = String::from("\nIDENTIFICATION DIVISION.\nPROGRAM-ID. DEEP41.\nDATA DIVISION.\nLINKAGE SECTION.\n01 P.\n");
        for i in 1..=41 {
            src.push_str(&format!("    {:02} G{i:02}.\n", i + 1));
        }
        src.push_str("    43 LEAF PIC 9(2).\nPROCEDURE DIVISION USING P.\nMAIN-PARA.\n    GOBACK.\n");
        src
    };
    let dir = backend_dir("differences-deep41", "[contract]\nversion = \"1.0.0\"\n\n[[export]]\nprogram = \"DEEP41\"\n", &[("deep41.cbl", &deep_41)]);
    let on_interpreter = try_served(&dir, false, false).is_ok();
    let on_module = try_served(&dir, false, true).err();
    check(
        "DEEP41: the interpreter's side loads it, the runner refuses the module, reading the interface back",
        on_interpreter && on_module.as_deref().is_some_and(|e| e.contains("recursion limit exceeded")),
        format!("interpreter loads: {on_interpreter}; module: {}", on_module.as_deref().unwrap_or("loads")),
    );

    for line in &report {
        println!("  {line}");
    }
    println!(
        "spec 114 T47, the differences measured: {} checks on the interpreter's engine and on the module's in {} ms; PASS {passed:03} FAIL {failed:03}",
        passed + failed,
        started.elapsed().as_millis()
    );
    assert_eq!(failed, 0, "{} check(s) failed:\n{}", failed, report.join("\n"));
}
