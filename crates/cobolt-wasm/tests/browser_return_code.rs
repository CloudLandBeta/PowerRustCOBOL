// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! `RETURN-CODE` in a browser module: the whole-number register a desktop module keeps
//! (`native_return_code.rs`, DESKTOP-WASM-001 T15) without the hand-over to a process's exit, which a
//! page has none to hand it to. The module used to answer "RETURN-CODE is not defined". Each program
//! runs on the interpreter and on a browser module in both memory layouts: the same lines. The uses
//! a module cannot follow are refused, naming why, and the refusal no longer sends anyone to the
//! Cargo path, which a page has not.

mod common;

use cobolt_wasm::{compile_program, Options};
use std::time::Instant;

const WS: &str = "01 WS-N PIC S9(4) VALUE 12.\n01 WS-I PIC 9(3) VALUE 5.\n01 WS-D PIC 9V9 VALUE 2.5.\n01 WS-X PIC X(5) VALUE \"ABC\".\n01 WS-E PIC -9(5).\n01 WS-Z PIC ZZZ9.\n";

fn program(id: &str, procedure: &str) -> String {
    format!("IDENTIFICATION DIVISION.\nPROGRAM-ID. {id}.\nDATA DIVISION.\nWORKING-STORAGE SECTION.\n{WS}PROCEDURE DIVISION.\n{procedure}    STOP RUN.\n")
}

/// Programs a module compiles: (name, procedure).
const RUNS: &[(&str, &str)] = &[
    ("never set", "    DISPLAY RETURN-CODE\n"),
    ("move, then show", "    MOVE 4 TO RETURN-CODE\n    DISPLAY \"RC=\" RETURN-CODE\n"),
    ("negative", "    MOVE -1 TO RETURN-CODE\n    DISPLAY RETURN-CODE\n"),
    ("add, subtract, multiply", "    MOVE 3 TO RETURN-CODE\n    ADD 5 TO RETURN-CODE\n    SUBTRACT 2 FROM RETURN-CODE\n    MULTIPLY 7 BY RETURN-CODE\n    DISPLAY RETURN-CODE\n"),
    ("compute from items", "    COMPUTE RETURN-CODE = WS-N * 2 - WS-I\n    DISPLAY RETURN-CODE\n"),
    ("giving", "    ADD WS-I 1 GIVING RETURN-CODE\n    DISPLAY RETURN-CODE\n"),
    ("read as a number", "    MOVE -3 TO RETURN-CODE\n    MOVE RETURN-CODE TO WS-N\n    DISPLAY WS-N\n    COMPUTE WS-N = RETURN-CODE * 10\n    DISPLAY WS-N\n"),
    (
        "compared",
        "    MOVE 3 TO RETURN-CODE\n    IF RETURN-CODE > 2 DISPLAY \"GT\" END-IF\n    IF RETURN-CODE = ZERO DISPLAY \"ZERO\" ELSE DISPLAY \"NONZERO\" END-IF\n    EVALUATE RETURN-CODE\n      WHEN 3 DISPLAY \"THREE\"\n      WHEN OTHER DISPLAY \"OTHER\"\n    END-EVALUATE\n",
    ),
    ("above 255", "    MOVE 300 TO RETURN-CODE\n    DISPLAY RETURN-CODE\n"),
    ("above 32 bits", "    MOVE 9999999999999 TO RETURN-CODE\n    DISPLAY RETURN-CODE\n"),
    ("reset", "    MOVE 5 TO RETURN-CODE\n    MOVE 0 TO RETURN-CODE\n    DISPLAY RETURN-CODE\n"),
    ("into numeric-edited items", "    MOVE -42 TO RETURN-CODE\n    MOVE RETURN-CODE TO WS-E WS-Z\n    DISPLAY \"[\" WS-E \"][\" WS-Z \"]\"\n"),
    ("set in a paragraph, read in another", "    PERFORM SET-IT\n    PERFORM SHOW-IT\n    GO TO DONE.\nSET-IT.\n    MOVE 9 TO RETURN-CODE.\nSHOW-IT.\n    DISPLAY \"RC \" RETURN-CODE.\nDONE.\n    CONTINUE.\n"),
];

/// Uses a module does not compile, and what the refusal says.
const REFUSED: &[(&str, &str, &str)] = &[
    ("a fraction", "    MOVE 7.9 TO RETURN-CODE\n", "may carry a fraction"),
    ("characters", "    MOVE \"AB\" TO RETURN-CODE\n", "may carry a fraction"),
    ("an item with decimals", "    MOVE WS-D TO RETURN-CODE\n", "may carry a fraction"),
    ("a division", "    COMPUTE RETURN-CODE = 10 / 4\n", "may carry a fraction"),
    ("DIVIDE", "    MOVE 4 TO RETURN-CODE\n    DIVIDE 3 INTO RETURN-CODE\n", "may carry a fraction"),
    ("ON SIZE ERROR", "    ADD 1 TO RETURN-CODE ON SIZE ERROR DISPLAY \"E\" END-ADD\n", "no size to exceed"),
    ("into characters", "    MOVE RETURN-CODE TO WS-X\n", "other than as a number"),
    ("compared with characters", "    IF RETURN-CODE = \"AB\" DISPLAY \"X\" END-IF\n", "other than as a number"),
    ("in a STRING", "    STRING RETURN-CODE DELIMITED BY SIZE INTO WS-X\n", "other than as a number"),
];

#[test]
fn return_code_is_a_whole_number_register_in_a_browser_module() {
    let (mut t_interp, mut t_module, mut passed) = (0.0, 0.0, 0usize);
    println!("== browser_return_code: RETURN-CODE in a browser module, interpreter = module (both memory layouts) ==");
    for (k, (name, procedure)) in RUNS.iter().enumerate() {
        let src = program(&format!("BRC{k}"), procedure);
        let t = Instant::now();
        let want = common::run_interpreter(&src);
        t_interp += t.elapsed().as_secs_f64() * 1000.0;
        for multi in [true, false] {
            let t = Instant::now();
            let o = common::run_wasm(&src, &Options { multi_memory: multi, ..Options::default() });
            t_module += t.elapsed().as_secs_f64() * 1000.0;
            assert!(o.trapped.is_none() && o.errors.is_empty(), "{name} multi={multi}: trapped {:?} errors {:?}\n{src}", o.trapped, o.errors);
            assert_eq!(common::wasm_lines(&o), want, "{name} multi={multi}\n{src}");
        }
        println!("  pass {name}: {want:?}");
        passed += 1;
    }
    println!("timings: interpreter {t_interp:.1} ms; browser module (2 layouts) {t_module:.1} ms");
    println!("result: {passed} of {} programs identical on the interpreter and the browser module", RUNS.len());
    assert_eq!(passed, RUNS.len());
}

#[test]
fn the_uses_a_browser_module_cannot_follow_are_refused_with_the_reason() {
    let t = Instant::now();
    println!("== browser_return_code: refusals ==");
    for (k, (name, procedure, says)) in REFUSED.iter().enumerate() {
        let src = program(&format!("BRCR{k}"), procedure);
        let e = compile_program(&common::parse(&src), &Options::default()).err().unwrap_or_else(|| panic!("{name}: the module should be refused"));
        assert!(e.message.contains(says), "{name}: {}", e.message);
        assert!(e.message.contains("browser module"), "{name}: {}", e.message);
        assert!(!e.message.contains("Cargo"), "{name}: a page has no Cargo path: {}", e.message);
        println!("  pass {name}: line {}: {}", e.line, e.message);
    }
    println!("compiled {} refusals in {:.1} ms", REFUSED.len(), t.elapsed().as_secs_f64() * 1000.0);
    println!("result: {} of {} refusals name their reason and send no one to the Cargo path", REFUSED.len(), REFUSED.len());
}

/// A conformance build (the NIST gate) still reads the name as it did.
#[test]
fn a_conformance_build_does_not_hold_the_register() {
    let src = program("BRCC", "    MOVE 4 TO RETURN-CODE\n");
    let e = compile_program(&common::parse(&src), &Options { conformance: true, ..Options::default() }).err().map(|e| e.message).unwrap_or_default();
    println!("== browser_return_code: the conformance build ==\nrefused: {e}\nresult: 1 of 1 check (the NIST gate is unchanged)");
    assert!(e.contains("RETURN-CODE"), "{e}");
}
