// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Spec 109 S1 on the WebAssembly line: a user-defined function (`FUNCTION-ID`) is not generated
//! yet, so a program that calls one is refused at build time with the function named — it is never
//! compiled into a module that answers something else (`specs/WASM-PARITY.md`, row 48).

mod common;
use common::*;

const SRC: &str = "\
IDENTIFICATION DIVISION.
PROGRAM-ID. MAIN.
DATA DIVISION.
WORKING-STORAGE SECTION.
01 WS-A PIC S9(5)V99 VALUE 21.50.
01 WS-B PIC S9(5)V99.
PROCEDURE DIVISION.
MAIN-PARA.
    COMPUTE WS-B = FUNCTION DOUBLE-AMOUNT(WS-A)
    DISPLAY WS-B
    STOP RUN.

IDENTIFICATION DIVISION.
FUNCTION-ID. DOUBLE-AMOUNT.
DATA DIVISION.
LINKAGE SECTION.
01 ARG-AMOUNT PIC S9(5)V99.
01 RESULT-AMOUNT PIC S9(5)V99.
PROCEDURE DIVISION USING BY VALUE ARG-AMOUNT RETURNING RESULT-AMOUNT.
    COMPUTE RESULT-AMOUNT = ARG-AMOUNT * 2
    GOBACK.
END FUNCTION DOUBLE-AMOUNT.
";

#[test]
fn a_program_that_calls_a_user_function_is_refused_with_its_name() {
    let prog = parse(SRC);
    match cobolt_wasm::compile_program(&prog, &cobolt_wasm::Options::default()) {
        Err(e) => {
            let text = e.to_string();
            assert!(text.to_ascii_uppercase().contains("DOUBLE-AMOUNT"), "the refusal must name the function: {text}");
            println!("refused: {text}");
        }
        Ok(c) => {
            let out = run_wasm(SRC, &cobolt_wasm::Options::default());
            panic!("compiled ({} bytes) instead of being refused; it printed {:?}", c.wasm.len(), wasm_lines(&out));
        }
    }
}
