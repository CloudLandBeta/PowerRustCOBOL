// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Level-78 constants (`78 K VALUE 5.`) in a WebAssembly module, browser and desktop. The parser nests
//! a 78 under whatever item precedes it, so a module used to answer "K is not defined" (or "an
//! elementary item needs a PICTURE" when the constant came first). The generator now takes each one
//! out of its parent and holds it as an item of its own, with the picture its literal needs. Every case
//! runs on the interpreter, on a browser module (both memory layouts) and on a desktop module, and the
//! lines `DISPLAY` writes must be identical.

mod common;

use cobolt_wasm::host::{run_native, NativeInput};
use cobolt_wasm::{compile_program, Options};
use std::time::Instant;

fn program(ws: &str, body: &str) -> String {
    format!(
        "IDENTIFICATION DIVISION.\nPROGRAM-ID. K78.\nDATA DIVISION.\nWORKING-STORAGE SECTION.\n{ws}PROCEDURE DIVISION.\nMAIN-PARA.\n{body}    STOP RUN.\n"
    )
}

const PLAIN: &str = "01 WS-I PIC 9(3).\n01 WS-R PIC S9(5)V99.\n01 WS-X PIC X(8).\n";

/// (what the case measures, the data, the statements).
const CASES: &[(&str, &str, &str)] = &[
    ("a whole number, first in the section", "78 K VALUE 5.\n01 WS-I PIC 9(3).\n", "    MOVE K TO WS-I\n    DISPLAY WS-I\n    DISPLAY K\n"),
    ("a whole number after a PIC item", "01 WS-I PIC 9(3).\n78 K VALUE 5.\n", "    MOVE K TO WS-I\n    DISPLAY WS-I\n    DISPLAY K\n"),
    (
        "a constant under a group takes no room in it",
        "01 G.\n   05 A PIC 9.\n   05 B PIC 9.\n78 K VALUE 5.\n01 WS-I PIC 9(3).\n",
        "    MOVE 12 TO G\n    DISPLAY \"[\" G \"]\"\n    MOVE K TO WS-I\n    DISPLAY WS-I\n",
    ),
    (
        "a constant between two items of one group",
        "01 G.\n   05 A PIC 9 VALUE 1.\n   78 K VALUE 7.\n   05 B PIC 9 VALUE 2.\n01 WS-I PIC 9(3).\n",
        "    DISPLAY \"[\" G \"]\"\n    ADD K TO B\n    DISPLAY G\n",
    ),
    ("a negative whole number", "78 K VALUE -42.\n01 WS-R PIC S9(5)V99.\n", "    MOVE K TO WS-R\n    DISPLAY WS-R\n    DISPLAY K\n"),
    ("a decimal", "78 K VALUE 3.25.\n01 WS-R PIC S9(5)V99.\n", "    MOVE K TO WS-R\n    DISPLAY WS-R\n    DISPLAY K\n"),
    ("a decimal below one", "78 K VALUE 0.5.\n01 WS-R PIC S9(5)V99.\n", "    MOVE K TO WS-R\n    DISPLAY WS-R\n    DISPLAY K\n"),
    ("text", "78 K VALUE \"HELLO\".\n01 WS-X PIC X(8).\n", "    MOVE K TO WS-X\n    DISPLAY \"[\" WS-X \"]\"\n    DISPLAY K\n"),
    (
        "text in a STRING and a comparison",
        "78 K VALUE \"AB\".\n01 WS-X PIC X(8) VALUE SPACES.\n",
        "    STRING K \"-\" K DELIMITED BY SIZE INTO WS-X\n    DISPLAY \"[\" WS-X \"]\"\n    IF WS-X(1:2) = K DISPLAY \"same\" ELSE DISPLAY \"differ\" END-IF\n",
    ),
    (
        "in arithmetic",
        "78 K VALUE 6.\n01 WS-I PIC 9(3) VALUE 10.\n01 WS-R PIC S9(5)V99.\n",
        "    COMPUTE WS-R = K * 2 + WS-I\n    DISPLAY WS-R\n    ADD K TO WS-I\n    DISPLAY WS-I\n    SUBTRACT K FROM WS-I\n    DISPLAY WS-I\n    MULTIPLY K BY WS-I\n    DISPLAY WS-I\n",
    ),
    (
        "in conditions and EVALUATE",
        "78 K VALUE 3.\n01 WS-I PIC 9(3) VALUE 3.\n",
        "    IF WS-I = K DISPLAY \"eq\" END-IF\n    IF WS-I > K DISPLAY \"gt\" ELSE DISPLAY \"not gt\" END-IF\n    EVALUATE WS-I WHEN K DISPLAY \"when K\" WHEN OTHER DISPLAY \"other\" END-EVALUATE\n",
    ),
    (
        "as a loop bound",
        "78 LAST-ONE VALUE 4.\n01 WS-I PIC 9(3).\n",
        "    PERFORM VARYING WS-I FROM 1 BY 1 UNTIL WS-I > LAST-ONE\n      DISPLAY WS-I\n    END-PERFORM\n",
    ),
    (
        "several constants, one of them after another",
        "78 A1 VALUE 1.\n78 A2 VALUE 22.\n78 A3 VALUE \"ccc\".\n01 WS-I PIC 9(3).\n",
        "    COMPUTE WS-I = A1 + A2\n    DISPLAY WS-I\n    DISPLAY A3\n",
    ),
    ("a constant after a table", "01 T.\n   05 E OCCURS 3 PIC 9 VALUE 2.\n78 K VALUE 9.\n01 WS-I PIC 9(3).\n", "    MOVE K TO WS-I\n    DISPLAY WS-I\n    DISPLAY T\n"),
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
fn level_78_constants_run_in_a_module_as_the_interpreter_runs_them() {
    let (mut t_interp, mut t_browser, mut t_desktop) = (0.0, 0.0, 0.0);
    let mut passed = 0;
    println!("== level78_constants: 78 K VALUE …, interpreter = browser module = desktop module ==");
    for (what, ws, body) in CASES {
        let src = program(ws, body);
        let t = Instant::now();
        let i = common::run_interpreter(&src);
        t_interp += t.elapsed().as_secs_f64() * 1000.0;
        assert!(!i.is_empty(), "the interpreter showed nothing: {what}\n{src}");
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

/// A program that declares a constant and never names it runs exactly as the same program without it.
#[test]
fn plain_data_beside_a_constant_is_unchanged() {
    let with = program(&format!("{PLAIN}78 K VALUE 5.\n"), "    MOVE 7 TO WS-I\n    DISPLAY WS-I\n");
    let without = program(PLAIN, "    MOVE 7 TO WS-I\n    DISPLAY WS-I\n");
    let (a, b) = (common::run_wasm(&with, &Options::default()), common::run_wasm(&without, &Options::default()));
    println!("== level78_constants: a program that does not use its constant ==\nwith {:?}; without {:?}", common::wasm_lines(&a), common::wasm_lines(&b));
    assert_eq!(common::wasm_lines(&a), common::wasm_lines(&b));
    println!("result: 1 of 1 check (an unused constant changes nothing)");
}
