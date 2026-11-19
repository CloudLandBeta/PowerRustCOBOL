// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! DESKTOP-WASM-001 T45 (R11, R12): `USAGE POINTER`, `SET pointer TO ADDRESS OF item`, `SET
//! ADDRESS OF linkage-item TO pointer` and `NULL` computed in a desktop module as the interpreter
//! computes them. The interpreter's own pointer test (`cobolt-runtime/tests/test_pointers.rs`)
//! and the rules it relies on run on both engines in both memory layouts; the lines `DISPLAY`
//! writes must be identical. The interpreter numbers the addresses it hands out (1, 2, … in the
//! order it first meets each item) and aliases the LINKAGE item to the item by name; the module
//! does the same where the two readings agree, and refuses the rest at their line.

mod common;

use cobolt_wasm::host::{run_native, NativeInput};
use cobolt_wasm::{compile_program, Options};
use std::time::{Duration, Instant};

fn native(multi: bool) -> Options {
    Options { native: true, multi_memory: multi, ..Options::default() }
}

fn program(items: &str, linkage: &str, procedure: &str) -> String {
    format!(
        "IDENTIFICATION DIVISION.\nPROGRAM-ID. PTR.\nDATA DIVISION.\nWORKING-STORAGE SECTION.\n{items}\
         LINKAGE SECTION.\n{linkage}PROCEDURE DIVISION.\nMAIN-PARA.\n{procedure}    STOP RUN.\n"
    )
}

/// (name, working storage, linkage, procedure)
const CASES: &[(&str, &str, &str, &str)] = &[
    (
        "the interpreter's own test: NULL, ADDRESS OF, and reading and writing through the alias",
        "01 WS-A PIC X(5) VALUE \"HELLO\".\n01 WS-PTR USAGE POINTER.\n",
        "01 LK-X PIC X(5).\n",
        "    IF WS-PTR = NULL DISPLAY \"INIT-NULL\" END-IF\n    SET WS-PTR TO ADDRESS OF WS-A\n    SET ADDRESS OF LK-X TO WS-PTR\n\
         DISPLAY LK-X\n    MOVE \"WORLD\" TO LK-X\n    DISPLAY WS-A\n    SET WS-PTR TO NULL\n    IF WS-PTR = NULL DISPLAY \"PTR-NULL\" END-IF\n",
    ),
    (
        "addresses are numbered in the order items are first named, and a pointer copies another",
        "01 WS-A PIC X(5) VALUE \"AAAAA\".\n01 WS-B PIC X(5) VALUE \"BBBBB\".\n01 P1 USAGE POINTER.\n01 P2 USAGE POINTER.\n01 P3 USAGE POINTER.\n",
        "01 LK-X PIC X(5).\n",
        "    SET P1 TO ADDRESS OF WS-B\n    SET P2 TO ADDRESS OF WS-A\n    SET P3 TO ADDRESS OF WS-B\n    DISPLAY P1 \" \" P2 \" \" P3\n\
         IF P1 = P3 DISPLAY \"SAME\" ELSE DISPLAY \"DIFFERENT\" END-IF\n    IF P1 = P2 DISPLAY \"SAME\" ELSE DISPLAY \"DIFFERENT\" END-IF\n\
         SET P3 TO P2\n    DISPLAY P3\n    SET ADDRESS OF LK-X TO P3\n    DISPLAY LK-X\n    SET P1 P2 TO NULL\n    DISPLAY P1 \" \" P2\n",
    ),
    (
        "SET ADDRESS OF straight to ADDRESS OF, then to another item",
        "01 WS-A PIC X(5) VALUE \"FIRST\".\n01 WS-B PIC X(5) VALUE \"OTHER\".\n",
        "01 LK-X PIC X(5).\n",
        "    SET ADDRESS OF LK-X TO ADDRESS OF WS-A\n    DISPLAY LK-X\n    SET ADDRESS OF LK-X TO ADDRESS OF WS-B\n    DISPLAY LK-X\n\
         MOVE \"NEW B\" TO LK-X\n    DISPLAY WS-A \" \" WS-B\n",
    ),
    (
        "a numeric item read and changed through the alias",
        "01 WS-N PIC S9(5) COMP-3 VALUE -123.\n01 WS-P USAGE POINTER.\n",
        "01 LK-N PIC S9(5) COMP-3.\n",
        "    SET WS-P TO ADDRESS OF WS-N\n    SET ADDRESS OF LK-N TO WS-P\n    ADD 1000 TO LK-N\n    DISPLAY WS-N\n    DISPLAY LK-N\n",
    ),
    (
        "a pointer never set is NULL; the same address taken twice keeps its number; SET ADDRESS OF … TO NULL",
        "01 WS-A PIC X(3) VALUE \"ONE\".\n01 WS-B PIC X(3) VALUE \"TWO\".\n01 WS-P USAGE POINTER.\n01 WS-Q USAGE POINTER.\n",
        "01 LK-X PIC X(3).\n",
        "    IF WS-P = NULL DISPLAY \"NEVER SET\" END-IF\n    SET WS-Q TO ADDRESS OF WS-B\n\
         SET WS-P TO ADDRESS OF WS-A\n    SET WS-P TO ADDRESS OF WS-B\n    DISPLAY WS-P \" \" WS-Q\n    IF WS-P NOT = NULL DISPLAY \"SET\" END-IF\n\
         SET ADDRESS OF LK-X TO WS-P\n    DISPLAY LK-X\n    SET ADDRESS OF LK-X TO NULL\n    SET ADDRESS OF LK-X TO WS-Q\n    DISPLAY LK-X\n",
    ),
];

/// What the interpreter DISPLAYs.
fn interpreter(src: &str) -> Vec<String> {
    let prog = common::parse(src);
    let (_event_tx, event_rx) = std::sync::mpsc::channel();
    let (state_tx, _state_rx) = std::sync::mpsc::channel();
    let (display_tx, display_rx) = std::sync::mpsc::channel();
    let mut interp = cobolt_runtime::interpreter::Interpreter::new_with_channels(prog, event_rx, state_tx, display_tx);
    interp.run().expect("the interpreter runs it");
    display_rx.try_iter().map(|s: String| s.trim_end_matches(['\n', '\r']).to_owned()).collect()
}

#[test]
fn pointers_compute_as_the_interpreter_computes_them() {
    let (mut t_compile, mut t_module, mut t_interp) = (Duration::ZERO, Duration::ZERO, Duration::ZERO);
    let (mut lines, mut runs) = (0usize, 0usize);
    let mut results: Vec<(String, Result<(), String>)> = Vec::new();
    for (name, items, linkage, procedure) in CASES {
        let src = program(items, linkage, procedure);
        let t = Instant::now();
        let want = interpreter(&src);
        t_interp += t.elapsed();
        let mut outcome = Ok(());
        for multi in [true, false] {
            let t = Instant::now();
            let compiled = compile_program(&common::parse(&src), &native(multi));
            t_compile += t.elapsed();
            let c = match compiled {
                Ok(c) => c,
                Err(e) => {
                    outcome = Err(format!("multi={multi}: not compiled: {e}"));
                    break;
                }
            };
            let t = Instant::now();
            let o = run_native(&c.wasm, NativeInput::default()).expect("the host runs it");
            t_module += t.elapsed();
            runs += 1;
            let got = common::wasm_lines(&o);
            if o.trapped.is_some() || !o.errors.is_empty() {
                outcome = Err(format!("multi={multi}: trapped {:?} errors {:?} {:?}", o.trapped, o.errors, o.error_texts));
            } else if got != want {
                outcome = Err(format!("multi={multi}: DISPLAY differs\n  module      {got:?}\n  interpreter {want:?}"));
            } else {
                lines += got.len();
                continue;
            }
            break;
        }
        results.push((name.to_string(), outcome));
    }
    let passed = results.iter().filter(|(_, r)| r.is_ok()).count();
    let ms = |d: Duration| d.as_secs_f64() * 1000.0;
    println!("== native_pointers: POINTER, ADDRESS OF and NULL, module against interpreter (DESKTOP-WASM-001 T45) ==");
    println!("cases: {} (each in both memory layouts: {runs} module runs)", results.len());
    for (name, r) in &results {
        match r {
            Ok(()) => println!("  pass {name}"),
            Err(e) => println!("  FAIL {name}: {e}"),
        }
    }
    println!(
        "compared: {lines} DISPLAY lines, all identical where passed\n\
         timings: interpreter {:.1} ms; compile {:.1} ms; module runs {:.1} ms",
        ms(t_interp),
        ms(t_compile),
        ms(t_module),
    );
    println!("result: {passed} of {} cases identical", results.len());
    let failed: Vec<&String> = results.iter().filter(|(_, r)| r.is_err()).map(|(n, _)| n).collect();
    assert!(failed.is_empty(), "failed: {failed:?}");
}

/// Where the interpreter's alias by name and the module's alias by address would give different
/// answers, the module refuses the statement at its line and the Build takes the Cargo path.
#[test]
fn the_pointer_uses_a_module_does_not_compute_are_refused_at_their_line() {
    // (what, working storage, linkage, procedure, the text of the line the refusal names)
    let refused: &[(&str, &str, &str, &str, &str)] = &[
        (
            "a LINKAGE group with fields of its own",
            "01 WS-A PIC X(5).\n",
            "01 LK-G.\n   05 LK-G1 PIC X(2).\n   05 LK-G2 PIC X(3).\n",
            "    SET ADDRESS OF LK-G TO ADDRESS OF WS-A\n",
            "SET ADDRESS OF LK-G",
        ),
        (
            "an item described otherwise than the LINKAGE item",
            "01 WS-A PIC X(5).\n01 WS-B PIC X(8).\n01 WS-P USAGE POINTER.\n",
            "01 LK-X PIC X(5).\n",
            "    SET WS-P TO ADDRESS OF WS-A\n    SET WS-P TO ADDRESS OF WS-B\n    SET ADDRESS OF LK-X TO WS-P\n",
            "ADDRESS OF WS-B",
        ),
        (
            "an element of a table",
            "01 WS-T.\n   05 WS-E PIC X(2) OCCURS 3.\n01 WS-P USAGE POINTER.\n",
            "01 LK-X PIC X(2).\n",
            "    SET WS-P TO ADDRESS OF WS-E (2)\n",
            "ADDRESS OF WS-E",
        ),
        (
            "a receiver that is not a pointer",
            "01 WS-A PIC X(5).\n01 WS-N PIC 9(4).\n",
            "01 LK-X PIC X(5).\n",
            "    SET WS-N TO ADDRESS OF WS-A\n",
            "SET WS-N",
        ),
        (
            "the address of a LINKAGE item",
            "01 WS-P USAGE POINTER.\n",
            "01 LK-X PIC X(5).\n01 LK-Y PIC X(5).\n",
            "    SET WS-P TO ADDRESS OF LK-Y\n    SET ADDRESS OF LK-X TO WS-P\n",
            "ADDRESS OF LK-Y",
        ),
    ];
    let line_of = |src: &str, needle: &str| src.lines().position(|l| l.contains(needle)).map(|i| i as u32 + 1).expect("the line is in the source");
    let mut checks = 0;
    let mut failures: Vec<String> = Vec::new();
    for (what, items, linkage, procedure, needle) in refused {
        let src = program(items, linkage, procedure);
        let line = line_of(&src, needle);
        match compile_program(&common::parse(&src), &native(true)) {
            Ok(_) => failures.push(format!("{what}: compiled")),
            Err(e) if e.line == line && e.message.contains("Cargo path") => checks += 1,
            Err(e) => failures.push(format!("{what}: refused at line {} (want {line}): {e}", e.line)),
        }
    }
    println!("== native_pointers: refusals (DESKTOP-WASM-001 T45) ==");
    println!("result: {checks} of {} refusals at their line", refused.len());
    assert!(failures.is_empty(), "{failures:#?}");
}

/// The two differences WASM-PARITY records for pointers (#80, #81), measured: a pointer no statement
/// has given a value, and a LINKAGE item read after `SET ADDRESS OF … TO NULL`.
#[test]
fn the_recorded_pointer_differences_measure_as_recorded() {
    let unset = program("01 WS-P USAGE POINTER.\n", "", "    DISPLAY WS-P\n    IF WS-P = NULL DISPLAY \"NULL\" END-IF\n");
    let unbound = program(
        "01 WS-A PIC X(5) VALUE \"HELLO\".\n",
        "01 LK-X PIC X(5).\n",
        "    SET ADDRESS OF LK-X TO ADDRESS OF WS-A\n    DISPLAY LK-X\n    SET ADDRESS OF LK-X TO NULL\n    DISPLAY \"[\" LK-X \"]\"\n",
    );
    let module = |src: &str| {
        let c = compile_program(&common::parse(src), &native(true)).expect("the program compiles");
        let o = run_native(&c.wasm, NativeInput::default()).expect("the host runs it");
        (common::wasm_lines(&o), o.errors.iter().map(|e| e.code).collect::<Vec<u32>>())
    };
    let t = Instant::now();
    let (unset_i, unbound_i) = (interpreter(&unset), interpreter(&unbound));
    let t_interp = t.elapsed();
    let t = Instant::now();
    let ((unset_m, unset_e), (unbound_m, unbound_e)) = (module(&unset), module(&unbound));
    let t_module = t.elapsed();
    println!("== native_pointers: the recorded differences (WASM-PARITY #80, #81) ==");
    println!("#80 a pointer never given a value: interpreter {unset_i:?}; module {unset_m:?}, errors {unset_e:?}");
    println!("#81 LK-X read after SET ADDRESS OF LK-X TO NULL: interpreter {unbound_i:?}; module {unbound_m:?}, errors {unbound_e:?}");
    println!("timings: interpreter {:.1} ms; module {:.1} ms", t_interp.as_secs_f64() * 1000.0, t_module.as_secs_f64() * 1000.0);
    assert_eq!(unset_i, ["", "NULL"], "the interpreter shows a pointer with no value as nothing");
    assert_eq!(unset_m, ["0", "NULL"], "the module's pointer starts as NULL, 0");
    assert!(unset_e.is_empty());
    assert_eq!(unbound_i.len(), 2, "the interpreter reads the LINKAGE item's own storage again");
    assert_eq!(unbound_m, ["HELLO"], "the module ends the run at the read");
    assert_eq!(unbound_e, [1107], "with the unbound-LINKAGE error");
    println!("result: 6 of 6 checks; = NULL holds on both engines");
}
