// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! DESKTOP-WASM-001 T15, generator half (R27 console part; ABI A30): in a desktop module
//! `RETURN-CODE` is a whole-number register, and the run hands it to the runner's `prn.exit`.
//! Each program runs on the test host and on the interpreter: the same lines, and the code the
//! module hands over is the interpreter's exit code. The uses a module cannot follow are refused,
//! naming why, so the program takes the Cargo path. Without `native`, nothing changes.

mod common;

use cobolt_wasm::host::{run_native, NativeInput};
use cobolt_wasm::{compile_program, Options};
use std::time::Instant;

fn native(multi: bool) -> Options {
    Options { native: true, multi_memory: multi, ..Options::default() }
}

/// What the interpreter DISPLAYs, and the exit code it ends with.
fn interpreter(src: &str) -> (Vec<String>, i32) {
    let prog = common::parse(src);
    let (_events, event_rx) = std::sync::mpsc::channel();
    let (state_tx, _state_rx) = std::sync::mpsc::channel();
    let (display_tx, display_rx) = std::sync::mpsc::channel();
    let mut interp = cobolt_runtime::interpreter::Interpreter::new_with_channels(prog, event_rx, state_tx, display_tx);
    let _ = interp.run();
    (display_rx.try_iter().collect(), interp.return_code())
}

const WS: &str = "01 WS-N PIC S9(4) VALUE 12.\n01 WS-I PIC 9(3) VALUE 5.\n01 WS-D PIC 9V9 VALUE 2.5.\n01 WS-X PIC X(5) VALUE \"ABC\".\n01 WS-E PIC -9(5).\n01 WS-Z PIC ZZZ9.\n";

fn program(id: &str, procedure: &str, end: &str) -> String {
    format!("IDENTIFICATION DIVISION.\nPROGRAM-ID. {id}.\nDATA DIVISION.\nWORKING-STORAGE SECTION.\n{WS}PROCEDURE DIVISION.\n{procedure}{end}")
}

const STOP: &str = "    STOP RUN.\n";

/// Programs a module compiles: (name, procedure, how the run ends).
const RUNS: &[(&str, &str, &str)] = &[
    ("never set", "    DISPLAY RETURN-CODE\n", STOP),
    ("move, stop run", "    MOVE 4 TO RETURN-CODE\n    DISPLAY \"RC=\" RETURN-CODE\n", STOP),
    ("move, goback", "    MOVE 16 TO RETURN-CODE\n", "    GOBACK.\n"),
    ("move, last statement", "    MOVE 8 TO RETURN-CODE\n    DISPLAY \"RC=\" RETURN-CODE\n", "    ADD 1 TO RETURN-CODE.\n"),
    ("negative", "    MOVE -1 TO RETURN-CODE\n    DISPLAY RETURN-CODE\n", STOP),
    ("add, subtract, multiply", "    MOVE 3 TO RETURN-CODE\n    ADD 5 TO RETURN-CODE\n    SUBTRACT 2 FROM RETURN-CODE\n    MULTIPLY 7 BY RETURN-CODE\n    DISPLAY RETURN-CODE\n", STOP),
    ("compute from items", "    COMPUTE RETURN-CODE = WS-N * 2 - WS-I\n    DISPLAY RETURN-CODE\n", STOP),
    ("giving", "    ADD WS-I 1 GIVING RETURN-CODE\n    DISPLAY RETURN-CODE\n", STOP),
    ("read as a number", "    MOVE -3 TO RETURN-CODE\n    MOVE RETURN-CODE TO WS-N\n    DISPLAY WS-N\n    COMPUTE WS-N = RETURN-CODE * 10\n    DISPLAY WS-N\n    MOVE 0 TO RETURN-CODE\n", STOP),
    ("compared", "    MOVE 3 TO RETURN-CODE\n    IF RETURN-CODE > 2 DISPLAY \"GT\" END-IF\n    IF RETURN-CODE = ZERO DISPLAY \"ZERO\" ELSE DISPLAY \"NONZERO\" END-IF\n    EVALUATE RETURN-CODE\n      WHEN 3 DISPLAY \"THREE\"\n      WHEN OTHER DISPLAY \"OTHER\"\n    END-EVALUATE\n", STOP),
    ("above 255", "    MOVE 300 TO RETURN-CODE\n", STOP),
    ("above 32 bits", "    MOVE 9999999999999 TO RETURN-CODE\n    DISPLAY RETURN-CODE\n", STOP),
    ("below 32 bits", "    MOVE -9999999999999 TO RETURN-CODE\n", STOP),
    ("reset", "    MOVE 5 TO RETURN-CODE\n    MOVE 0 TO RETURN-CODE\n", STOP),
    ("into numeric-edited items", "    MOVE -42 TO RETURN-CODE\n    MOVE RETURN-CODE TO WS-E WS-Z\n    DISPLAY \"[\" WS-E \"][\" WS-Z \"]\"\n    MOVE 0 TO RETURN-CODE\n", STOP),
];

/// Results past 18 digits (WASM-PARITY #52, kept by the operator on 2026-11-15): the module keeps
/// the low 18 digits, as an item of 18 digits does; the interpreter's register has no size. The
/// module's side is checked against the interpreter's value; the interpreter's own exit code is
/// only shown.
const PAST_18: &[(&str, &str)] = &[
    ("19 digits", "    MOVE 90000000000 TO RETURN-CODE\n    MULTIPLY 100000000 BY RETURN-CODE\n    DISPLAY RETURN-CODE\n"),
    ("21 digits", "    MOVE 9999999999999 TO RETURN-CODE\n    MULTIPLY 100000000 BY RETURN-CODE\n    DISPLAY RETURN-CODE\n"),
    ("2^64", "    MOVE 4294967296 TO RETURN-CODE\n    MULTIPLY 4294967296 BY RETURN-CODE\n    DISPLAY RETURN-CODE\n"),
    ("-(2^64) + 5", "    MOVE -4294967296 TO RETURN-CODE\n    MULTIPLY 4294967296 BY RETURN-CODE\n    ADD 5 TO RETURN-CODE\n    DISPLAY RETURN-CODE\n"),
];

/// The low 18 digits of a whole number written as the interpreter shows it, sign kept.
fn low_18(shown: &str) -> i64 {
    let (neg, digits) = shown.strip_prefix('-').map_or((false, shown), |d| (true, d));
    let low = &digits[digits.len().saturating_sub(18)..];
    let n: i64 = low.parse().expect("a whole number");
    if neg { -n } else { n }
}

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
fn return_code_is_a_whole_number_register_handed_to_the_runner() {
    let mut checks = 0usize;
    let mut passed = 0usize;
    let mut failures: Vec<String> = Vec::new();
    let mut check = |ok: bool, what: String, failures: &mut Vec<String>| {
        checks += 1;
        if ok {
            passed += 1;
        } else {
            failures.push(what);
        }
    };
    let mut rows: Vec<String> = Vec::new();
    let (mut compile_ms, mut run_ms, mut runs) = (0f64, 0f64, 0usize);

    for (k, (name, procedure, end)) in RUNS.iter().enumerate() {
        let src = program(&format!("T15RC{k}"), procedure, end);
        let (want, want_code) = interpreter(&src);
        let prog = common::parse(&src);
        for multi in [true, false] {
            let t = Instant::now();
            let c = match compile_program(&prog, &native(multi)) {
                Ok(c) => c,
                Err(e) => {
                    check(false, format!("{name} multi={multi}: compile failed: {e}"), &mut failures);
                    continue;
                }
            };
            compile_ms += t.elapsed().as_secs_f64() * 1000.0;
            let scan = cobolt_wasm_abi::scan::scan_module(&c.wasm).expect("the module scans");
            let manifest = scan.embedded_manifest().expect("the module carries its manifest");
            let prn: Vec<&str> =
                scan.imports.iter().filter(|i| i.module == cobolt_wasm_abi::codes::NATIVE_IMPORT_MODULE).map(|i| i.name.as_str()).collect();
            check(
                manifest.capabilities == ["env"] && prn == ["exit"],
                format!("{name} multi={multi}: capabilities {:?}, prn imports {prn:?}; want [\"env\"], [\"exit\"]", manifest.capabilities),
                &mut failures,
            );
            let t = Instant::now();
            let o = run_native(&c.wasm, NativeInput::default()).unwrap_or_else(|e| panic!("{name}: host failed: {e}"));
            run_ms += t.elapsed().as_secs_f64() * 1000.0;
            runs += 1;
            check(o.trapped.is_none() && o.errors.is_empty(), format!("{name} multi={multi}: trapped {:?} errors {:?}", o.trapped, o.errors), &mut failures);
            let got = common::wasm_lines(&o);
            check(got == want, format!("{name} multi={multi}: module {got:?}, interpreter {want:?}"), &mut failures);
            let code = o.exit_code.unwrap_or(0);
            check(code == want_code, format!("{name} multi={multi}: module hands {:?}, interpreter ends with {want_code}", o.exit_code), &mut failures);
            if multi {
                rows.push(format!("  {:<26} exit {:>11}  {}", name, want_code, want.join(" | ")));
            }
        }
    }

    let mut refusals: Vec<String> = Vec::new();
    for (k, (name, procedure, says)) in REFUSED.iter().enumerate() {
        let prog = common::parse(&program(&format!("T15RX{k}"), procedure, STOP));
        let refused = compile_program(&prog, &native(true)).err().map(|e| e.to_string()).unwrap_or_default();
        let ok = refused.contains("RETURN-CODE") && refused.contains(says) && refused.contains("Cargo path") && !refused.to_lowercase().contains("cobolt");
        check(ok, format!("refused, {name}: {refused:?} does not say {says:?}"), &mut failures);
        refusals.push(format!("  {name:<26} {refused}"));
    }

    let mut past: Vec<String> = Vec::new();
    for (k, (name, procedure)) in PAST_18.iter().enumerate() {
        let src = program(&format!("T15RP{k}"), procedure, STOP);
        let (shown, interp_code) = interpreter(&src);
        let want = low_18(shown.first().map(String::as_str).unwrap_or("0"));
        let want_code = want.clamp(i64::from(i32::MIN), i64::from(i32::MAX)) as i32;
        let prog = common::parse(&src);
        for multi in [true, false] {
            let c = compile_program(&prog, &native(multi)).unwrap_or_else(|e| panic!("{name}: compile failed: {e}"));
            let o = run_native(&c.wasm, NativeInput::default()).unwrap_or_else(|e| panic!("{name}: host failed: {e}"));
            let got = common::wasm_lines(&o);
            check(got == [want.to_string()], format!("past 18, {name} multi={multi}: module {got:?}, want the low 18 digits {want}"), &mut failures);
            check(o.exit_code == Some(want_code), format!("past 18, {name} multi={multi}: module hands {:?}, want {want_code}", o.exit_code), &mut failures);
        }
        past.push(format!("  {name:<12} interpreter {:>22} exit {:>11} | module {:>19} exit {:>11}", shown.join(""), interp_code, want, want_code));
    }

    // A browser build holds the register too (`browser_return_code.rs`) and hands it to no exit; the
    // conformance build (the NIST gate) still refuses the name, as before T15. A desktop module that
    // never names it imports no `exit`, and its run ends as before.
    let prog = common::parse(&program("T15WEB", "    MOVE 4 TO RETURN-CODE\n", STOP));
    check(compile_program(&prog, &Options::default()).is_ok(), "without native: the browser build holds RETURN-CODE".to_string(), &mut failures);
    let conf = compile_program(&prog, &Options { conformance: true, ..Options::default() }).err().map(|e| e.to_string()).unwrap_or_default();
    check(conf.contains("RETURN-CODE is not defined"), format!("conformance build: {conf:?}"), &mut failures);
    let prog = common::parse(&program("T15NONE", "    DISPLAY \"HELLO\"\n", STOP));
    for multi in [true, false] {
        let c = compile_program(&prog, &native(multi)).expect("a plain program compiles");
        let w = compile_program(&prog, &Options { multi_memory: multi, ..Options::default() }).expect("and in a browser build");
        check(c.wasm == w.wasm, format!("multi={multi}: a desktop module that never names RETURN-CODE differs from the browser build"), &mut failures);
        let o = run_native(&c.wasm, NativeInput::default()).expect("it runs");
        check(o.exit_code.is_none(), format!("multi={multi}: it handed {:?} to exit", o.exit_code), &mut failures);
    }

    println!("── return_code_is_a_whole_number_register_handed_to_the_runner (DESKTOP-WASM-001 T15) ──");
    println!("compiled and run, each against the interpreter (lines and exit code):");
    for r in &rows {
        println!("{r}");
    }
    println!(
        "{} programs × 2 memory variants: compile {compile_ms:.1} ms, {runs} runs {run_ms:.1} ms ({:.0} runs/s)",
        RUNS.len(),
        runs as f64 / (run_ms / 1000.0).max(1e-9)
    );
    println!("refused (the program takes the Cargo path):");
    for r in &refusals {
        println!("{r}");
    }
    println!("past 18 digits (WASM-PARITY #52; the module keeps the low 18 digits, checked; the interpreter's exit only shown):");
    for r in &past {
        println!("{r}");
    }
    println!("without native: the browser build holds the register (the conformance build refuses it); a desktop module that never names it is the browser module, byte for byte");
    println!("checks: {passed} of {checks} passed");
    for f in &failures {
        println!("  FAIL {f}");
    }
    assert!(failures.is_empty(), "{} of {checks} checks failed:\n{}", failures.len(), failures.join("\n"));
}
