// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! `SET condition-name TO FALSE` in a WebAssembly module, browser and desktop, as the interpreter
//! does it (`Interpreter::set_condition`): with no `WHEN SET TO FALSE` clause in the language, the
//! host item takes the smallest whole number from 0 that satisfies none of the condition's values.
//! A module used to refuse the statement. Every case runs on the interpreter, on a browser module
//! (both memory layouts) and on a desktop module, and the lines `DISPLAY` writes must be identical.

mod common;

use cobolt_wasm::host::{run_native, NativeInput};
use cobolt_wasm::{compile_program, Options};
use std::time::Instant;

fn program(ws: &str, body: &str) -> String {
    format!(
        "IDENTIFICATION DIVISION.\nPROGRAM-ID. SETF.\nDATA DIVISION.\nWORKING-STORAGE SECTION.\n{ws}PROCEDURE DIVISION.\nMAIN-PARA.\n{body}    STOP RUN.\n"
    )
}

/// What `SET … TO FALSE` leaves in the host (`[host]`) and whether the condition then reads false.
const SHOW: &str = "    SET C TO FALSE\n    DISPLAY \"[\" H \"]\"\n    IF C DISPLAY \"on\" ELSE DISPLAY \"off\" END-IF\n";

/// (what the case measures, the data, the statements, what the interpreter shows).
const CASES: &[(&str, &str, &str, &[&str])] = &[
    ("a digit host, one value: 1 -> 0", "01 H PIC 9 VALUE 1.\n   88 C VALUE 1.\n", SHOW, &["[0]", "off"]),
    ("a digit host whose value is 0: -> 1", "01 H PIC 9 VALUE 0.\n   88 C VALUE 0.\n", SHOW, &["[1]", "off"]),
    ("a range 0 THRU 3: -> 4", "01 H PIC 9 VALUE 1.\n   88 C VALUE 0 THRU 3.\n", SHOW, &["[4]", "off"]),
    ("several values 0 1 2 in two digits: -> 03", "01 H PIC 99 VALUE 1.\n   88 C VALUE 0 1 2.\n", SHOW, &["[03]", "off"]),
    ("values 0 and 1 in one digit: -> 2", "01 H PIC 9 VALUE 0.\n   88 C VALUE 0 1.\n", SHOW, &["[2]", "off"]),
    ("a binary host: 5 -> 0000", "01 H PIC S9(4) COMP VALUE 5.\n   88 C VALUE 5.\n", SHOW, &["[0000]", "off"]),
    ("a decimal value on a decimal host", "01 H PIC 9V9 VALUE 0.5.\n   88 C VALUE 0.5.\n", SHOW, &["[00]", "off"]),
    ("an alphanumeric host, one character: Y -> 0", "01 H PIC X VALUE \"Y\".\n   88 C VALUE \"Y\".\n", SHOW, &["[0]", "off"]),
    ("an alphanumeric host of three: ABC -> digits at the right", "01 H PIC XXX VALUE \"ABC\".\n   88 C VALUE \"ABC\".\n", SHOW, &["[  0]", "off"]),
    ("an alphanumeric value that reads as 0: -> 1", "01 H PIC X VALUE \"0\".\n   88 C VALUE \"0\".\n", SHOW, &["[1]", "off"]),
    ("an alphanumeric value that reads as 1 leaves 0", "01 H PIC XX VALUE \"1\".\n   88 C VALUE \"1\".\n", SHOW, &["[ 0]", "off"]),
    ("the figurative SPACE: -> 0", "01 H PIC XX VALUE SPACE.\n   88 C VALUE SPACE.\n", SHOW, &["[ 0]", "off"]),
    (
        "TRUE, FALSE, TRUE again",
        "01 H PIC X VALUE \"N\".\n   88 C VALUE \"Y\".\n",
        "    SET C TO TRUE\n    DISPLAY H\n    SET C TO FALSE\n    DISPLAY H\n    SET C TO TRUE\n    IF C DISPLAY \"on\" END-IF\n",
        &["Y", "0", "on"],
    ),
    (
        "two condition-names over one host",
        "01 H PIC 9 VALUE 5.\n   88 A VALUE 1.\n   88 B VALUE 0.\n",
        "    SET A TO TRUE\n    SET A TO FALSE\n    IF B DISPLAY \"B\" ELSE DISPLAY \"not B\" END-IF\n    DISPLAY H\n",
        &["B", "0"],
    ),
];

fn browser(src: &str, multi: bool) -> Vec<String> {
    let o = common::run_wasm(src, &Options { multi_memory: multi, ..Options::default() });
    assert!(o.trapped.is_none() && o.errors.is_empty(), "browser multi={multi}: trapped {:?} errors {:?}\n{src}", o.trapped, o.errors);
    common::wasm_lines(&o)
}

fn desktop(src: &str) -> Vec<String> {
    let c = compile_program(&common::parse(src), &Options { native: true, ..Options::default() }).unwrap_or_else(|e| panic!("desktop: {e}\n{src}"));
    let o = run_native(&c.wasm, NativeInput::default()).expect("the host runs the module");
    assert!(o.trapped.is_none() && o.errors.is_empty(), "desktop: trapped {:?} errors {:?}\n{src}", o.trapped, o.errors);
    common::wasm_lines(&o)
}

#[test]
fn set_condition_to_false_runs_in_a_module_as_the_interpreter_runs_it() {
    let (mut t_interp, mut t_browser, mut t_desktop) = (0.0, 0.0, 0.0);
    let mut passed = 0;
    println!("== set_condition_false: SET condition-name TO FALSE, interpreter = browser module = desktop module ==");
    for (what, ws, body, want) in CASES {
        let src = program(ws, body);
        let t = Instant::now();
        let i = common::run_interpreter(&src);
        t_interp += t.elapsed().as_secs_f64() * 1000.0;
        assert_eq!(&i, want, "the interpreter's own answer for: {what}");
        let t = Instant::now();
        let (b1, b2) = (browser(&src, true), browser(&src, false));
        t_browser += t.elapsed().as_secs_f64() * 1000.0;
        let t = Instant::now();
        let d = desktop(&src);
        t_desktop += t.elapsed().as_secs_f64() * 1000.0;
        assert_eq!(b1, i, "browser module (multi-memory): {what}\n{src}");
        assert_eq!(b2, i, "browser module (single memory): {what}\n{src}");
        assert_eq!(d, i, "desktop module: {what}\n{src}");
        println!("  pass {what}: {i:?}");
        passed += 1;
    }
    println!("timings: interpreter {t_interp:.1} ms; browser module (2 layouts) {t_browser:.1} ms; desktop module {t_desktop:.1} ms");
    println!("result: {passed} of {} cases identical on the three engines", CASES.len());
    assert_eq!(passed, CASES.len());
}

/// A national host, which only a desktop module holds as characters, takes the digits as text; a
/// browser module says it cannot, where it used to refuse every `TO FALSE`.
#[test]
fn a_national_host_takes_the_digits_in_a_desktop_module_only() {
    let src = program("01 H PIC N(2) VALUE N\"ab\".\n   88 C VALUE N\"ab\".\n", SHOW);
    let i = common::run_interpreter(&src);
    let d = desktop(&src);
    let b = compile_program(&common::parse(&src), &Options::default()).err().map(|e| e.message).unwrap_or_default();
    println!("== set_condition_false: a national host ==");
    println!("interpreter {i:?}; desktop module {d:?}; browser module: {b}");
    assert_eq!(d, i);
    assert!(!b.is_empty(), "a browser module does not hold national text as characters");
    println!("result: 2 of 2 checks (desktop = interpreter; browser refuses, naming national data)");
}
