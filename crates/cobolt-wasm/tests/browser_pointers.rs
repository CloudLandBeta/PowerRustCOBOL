// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! `USAGE POINTER`, `SET pointer TO ADDRESS OF item`, `SET ADDRESS OF linkage-item TO pointer` and
//! `NULL` in a browser module, as a desktop module computes them (`native_pointers.rs`, DESKTOP-WASM-001
//! T45): the interpreter's numbering of addresses and its alias by name, kept by the module. A
//! browser module used to refuse every one of them and send the program to the backend (C13: "lowered
//! or reclassified"). Each case runs on the interpreter and on a browser module in both memory
//! layouts; the lines `DISPLAY` writes must be identical. The uses whose answer the module could not
//! give as the interpreter does are refused at their line, naming the backend.

mod common;

use cobolt_wasm::{compile_program, Options};
use std::time::Instant;

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
fn pointers_compute_in_a_browser_module_as_the_interpreter_computes_them() {
    let (mut t_interp, mut t_module, mut lines) = (0.0, 0.0, 0usize);
    println!("== browser_pointers: POINTER, ADDRESS OF and NULL in a browser module, against the interpreter ==");
    for (name, items, linkage, procedure) in CASES {
        let src = program(items, linkage, procedure);
        let t = Instant::now();
        let want = interpreter(&src);
        t_interp += t.elapsed().as_secs_f64() * 1000.0;
        for multi in [true, false] {
            let t = Instant::now();
            let o = common::run_wasm(&src, &Options { multi_memory: multi, ..Options::default() });
            t_module += t.elapsed().as_secs_f64() * 1000.0;
            assert!(o.trapped.is_none() && o.errors.is_empty(), "{name} multi={multi}: trapped {:?} errors {:?} {:?}\n{src}", o.trapped, o.errors, o.error_texts);
            assert_eq!(common::wasm_lines(&o), want, "{name} multi={multi}\n{src}");
            lines += want.len();
        }
        println!("  pass {name}");
    }
    println!("compared: {lines} DISPLAY lines, all identical");
    println!("timings: interpreter {t_interp:.1} ms; browser module runs {t_module:.1} ms");
    println!("result: {} of {} cases identical (each in both memory layouts)", CASES.len(), CASES.len());
}

/// Where the interpreter's alias by name and the module's alias by address would give different
/// answers, the module refuses the statement at its line, and says where the interpreter's rules run.
#[test]
fn the_pointer_uses_a_browser_module_does_not_compute_are_refused_at_their_line() {
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
    println!("== browser_pointers: refusals ==");
    for (what, items, linkage, procedure, needle) in refused {
        let src = program(items, linkage, procedure);
        let line = line_of(&src, needle);
        let e = compile_program(&common::parse(&src), &Options::default()).err().unwrap_or_else(|| panic!("{what}: compiled"));
        assert_eq!(e.line, line, "{what}: {e}");
        assert!(e.message.contains("CALL … WITH WEBSOCKET LINKAGE"), "{what}: {e}");
        assert!(!e.message.contains("Cargo"), "{what}: a page has no Cargo path: {e}");
        println!("  pass {what}: line {line}");
        checks += 1;
    }
    println!("result: {checks} of {} refusals at their line, naming the backend", refused.len());
}

/// A pointer statement no longer stops a program's other statements: a handler that sets one in the
/// same program as a form's other work compiles.
#[test]
fn a_program_with_pointers_and_other_statements_compiles_as_one_module() {
    let src = program(
        "01 WS-A PIC X(5) VALUE \"HELLO\".\n01 WS-P USAGE POINTER.\n01 WS-N PIC 9(3) VALUE 7.\n",
        "01 LK-X PIC X(5).\n",
        "    SET WS-P TO ADDRESS OF WS-A\n    SET ADDRESS OF LK-X TO WS-P\n    MOVE \"WORLD\" TO LK-X\n    ADD 1 TO WS-N\n    DISPLAY WS-A \" \" WS-N\n",
    );
    let t = Instant::now();
    let out = common::run_wasm(&src, &Options::default());
    println!("== browser_pointers: one module ==\noutput {:?} in {:.1} ms\nresult: 1 of 1 check", common::wasm_lines(&out), t.elapsed().as_secs_f64() * 1000.0);
    assert_eq!(common::wasm_lines(&out), ["WORLD 008"]);
}
