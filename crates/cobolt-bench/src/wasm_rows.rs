// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! The WebAssembly module beside the interpreter (WASM-015 T14, R8), behind the `wasm` feature.
//!
//! Every row here is measured **natively**, on `wasmtime`, in this process: it says how big the generated
//! module is and how the generated code compares with the interpreter on the same COBOL, never how long a
//! browser takes. No budget is asserted; the numbers are a baseline to read.

use super::{measure, run_cobol, Row};

fn module(src: &str, name: &str) -> Vec<u8> {
    use cobolt_lexer::{tokenize, SourceFormat};
    let parsed = cobolt_parser::parse(tokenize(src, SourceFormat::Free));
    if let Some(d) = parsed.diagnostics.iter().find(|d| d.severity == cobolt_parser::Severity::Error) {
        panic!("benchmark program failed to parse: {}", d.message);
    }
    let program = parsed.program.expect("benchmark program produced no AST");
    let opts = cobolt_wasm::Options { name: name.into(), ..Default::default() };
    match cobolt_wasm::compile_program(&program, &opts) {
        Ok(c) => c.wasm,
        Err(e) => panic!("the generator refused the benchmark program: {:?}", e.all()),
    }
}

fn run_module(wasm: &[u8]) {
    let out = cobolt_wasm::host::run_program(wasm).unwrap_or_else(|e| panic!("host: {e}"));
    assert!(out.trapped.is_none() && out.errors.is_empty(), "the module failed: {:?} {:?}", out.trapped, out.errors);
}

const MINIMAL: &str = "IDENTIFICATION DIVISION.\nPROGRAM-ID. MINIMAL.\nPROCEDURE DIVISION.\nMAIN-PARA.\n    STOP RUN.\n";

fn decimal(n: u64) -> String {
    format!(
        r#"
IDENTIFICATION DIVISION.
PROGRAM-ID. DECIMAL-BENCH.
DATA DIVISION.
WORKING-STORAGE SECTION.
01 I        PIC 9(9) COMP.
01 LIMIT-N  PIC 9(9) COMP VALUE {n}.
01 AMOUNT   PIC S9(13)V99 VALUE 1234.56.
01 RATE     PIC S9(3)V9(4) VALUE 1.0125.
01 TOTAL    PIC S9(15)V99 VALUE 0.
PROCEDURE DIVISION.
MAIN-PARA.
    PERFORM VARYING I FROM 1 BY 1 UNTIL I > LIMIT-N
        COMPUTE TOTAL = TOTAL + (AMOUNT * RATE)
    END-PERFORM
    STOP RUN.
"#
    )
}

fn strings(n: u64) -> String {
    format!(
        r#"
IDENTIFICATION DIVISION.
PROGRAM-ID. STRING-BENCH.
DATA DIVISION.
WORKING-STORAGE SECTION.
01 I        PIC 9(9) COMP.
01 LIMIT-N  PIC 9(9) COMP VALUE {n}.
01 SRC      PIC X(40) VALUE "THE QUICK BROWN FOX JUMPS OVER THE LAZY".
01 DST      PIC X(40).
01 JOINED   PIC X(80).
PROCEDURE DIVISION.
MAIN-PARA.
    PERFORM VARYING I FROM 1 BY 1 UNTIL I > LIMIT-N
        MOVE SRC TO DST
        MOVE SPACES TO JOINED
        STRING SRC DELIMITED BY SIZE DST DELIMITED BY SIZE INTO JOINED
    END-PERFORM
    STOP RUN.
"#
    )
}

pub fn rows(scale: f64, wanted: &dyn Fn(&str) -> bool) -> Vec<Row> {
    let mut rows = Vec::new();
    let n = ((500_000.0 * scale) as u64).max(500);
    let m = ((200_000.0 * scale) as u64).max(500);
    if wanted("module") {
        // The size of the generated module (ops = bytes, wall = the time the generator took), and the cost of
        // standing one up and running nothing: native, not a browser figure.
        rows.push(measure("module size, decimal program (native)", "bytes", || module(&decimal(n), "decimal").len() as u64));
        let wasm = module(MINIMAL, "minimal");
        let runs = ((200.0 * scale) as u64).max(5);
        rows.push(measure("module startup, empty program (native, not a browser figure)", "starts", move || {
            for _ in 0..runs {
                run_module(&wasm);
            }
            runs
        }));
    }
    if wanted("decimal") {
        let wasm = module(&decimal(n), "decimal");
        rows.push(measure("decimal COMPUTE, module (wasmtime)", "compute", move || {
            run_module(&wasm);
            n
        }));
    }
    if wanted("string") {
        let src = strings(m);
        let wasm = module(&src, "strings");
        rows.push(measure("string MOVE+STRING, interpreter", "iter", move || {
            run_cobol(&src);
            m
        }));
        rows.push(measure("string MOVE+STRING, module (wasmtime)", "iter", move || {
            run_module(&wasm);
            m
        }));
    }
    rows
}
