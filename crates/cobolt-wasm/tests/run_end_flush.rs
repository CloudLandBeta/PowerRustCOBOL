// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! A program module whose main program goes past its last statement hands over what it
//! DISPLAYed, as `STOP RUN` and `GOBACK` already do: each program runs on the module (both
//! memory variants) and on the interpreter, and the lines must be equal. The programs that end
//! with `STOP RUN` or `GOBACK` show their lines once, not twice.

mod common;

use cobolt_wasm::host::run_program;
use cobolt_wasm::{compile_program, Options};
use std::time::Instant;

fn program(id: &str, body: &str) -> String {
    format!(
        "IDENTIFICATION DIVISION.\nPROGRAM-ID. {id}.\nDATA DIVISION.\nWORKING-STORAGE SECTION.\n\
01 WS-I PIC 99 VALUE 0.\nPROCEDURE DIVISION.\n{body}"
    )
}

/// (name, procedure division body): how the run ends.
const CASES: &[(&str, &str)] = &[
    ("one line, past the end", "    DISPLAY \"ONLY LINE\".\n"),
    (
        "paragraphs, past the end",
        "MAIN-PARA.\n    DISPLAY \"FIRST\"\n    PERFORM SHOW-I 3 TIMES.\nSHOW-I.\n    ADD 1 TO WS-I\n    DISPLAY \"I=\" WS-I.\n",
    ),
    (
        "GO TO the last paragraph",
        "MAIN-PARA.\n    DISPLAY \"BEFORE\"\n    GO TO LAST-PARA.\nSKIPPED.\n    DISPLAY \"NOT SHOWN\".\nLAST-PARA.\n    DISPLAY \"LAST\".\n",
    ),
    (
        "sections, past the end",
        "MAIN SECTION.\nM1.\n    DISPLAY \"IN MAIN\".\nOTHER-WORK SECTION.\nO1.\n    DISPLAY \"IN OTHER\".\n",
    ),
    ("no advancing, past the end", "    DISPLAY \"NO NEWLINE\" WITH NO ADVANCING.\n"),
    ("STOP RUN", "    DISPLAY \"ONCE\"\n    STOP RUN.\n"),
    ("GOBACK", "    DISPLAY \"ONCE\"\n    GOBACK.\n"),
    ("EXIT PROGRAM", "    DISPLAY \"ONCE\"\n    EXIT PROGRAM.\n    DISPLAY \"AFTER\".\n"),
];

#[test]
fn a_run_past_the_last_statement_hands_over_its_lines() {
    let (mut checks, mut passed) = (0usize, 0usize);
    let mut failures: Vec<String> = Vec::new();
    let mut rows: Vec<String> = Vec::new();
    let (mut compile_ms, mut run_ms, mut runs, mut lines) = (0f64, 0f64, 0usize, 0usize);
    for (k, (name, body)) in CASES.iter().enumerate() {
        let src = program(&format!("FLUSH{k}"), body);
        let want = common::run_interpreter(&src);
        let prog = common::parse(&src);
        for multi in [true, false] {
            let t = Instant::now();
            let c = compile_program(&prog, &Options { multi_memory: multi, ..Options::default() })
                .unwrap_or_else(|e| panic!("{name}: compile failed: {e}"));
            compile_ms += t.elapsed().as_secs_f64() * 1000.0;
            let t = Instant::now();
            let o = run_program(&c.wasm).unwrap_or_else(|e| panic!("{name}: host failed: {e}"));
            run_ms += t.elapsed().as_secs_f64() * 1000.0;
            runs += 1;
            let got = common::wasm_lines(&o);
            lines += got.len();
            checks += 1;
            if o.trapped.is_none() && o.errors.is_empty() && got == want {
                passed += 1;
            } else {
                failures.push(format!("{name} multi={multi}: module {got:?} (trapped {:?}, errors {:?}), interpreter {want:?}", o.trapped, o.errors));
            }
        }
        rows.push(format!("  {name:<28} {}", want.join(" | ")));
    }
    println!("── a_run_past_the_last_statement_hands_over_its_lines ──");
    println!("each run on the module (2 memory variants) and on the interpreter:");
    for r in &rows {
        println!("{r}");
    }
    println!(
        "{} programs × 2 memory variants: compile {compile_ms:.1} ms, {runs} runs {run_ms:.1} ms ({:.0} runs/s), {lines} lines handed over",
        CASES.len(),
        runs as f64 / (run_ms / 1000.0).max(1e-9)
    );
    println!("checks: {passed} of {checks} passed");
    for f in &failures {
        println!("  FAIL {f}");
    }
    assert!(failures.is_empty(), "{} of {checks} checks failed:\n{}", failures.len(), failures.join("\n"));
}
