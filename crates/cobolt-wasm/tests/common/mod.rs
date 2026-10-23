// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Shared test plumbing: compile a COBOL source to a module, run it on the
//! test host, and run the same source on the interpreter as the oracle.

#![allow(dead_code)]

use cobolt_lexer::{tokenize, SourceFormat};
use cobolt_wasm::host::{run_program, Outcome};
use cobolt_wasm::{compile_program, Compiled, Options};

pub fn parse(src: &str) -> cobolt_ast::program::Program {
    let parsed = cobolt_parser::parse(tokenize(src, SourceFormat::Free));
    let errors: Vec<String> = parsed
        .diagnostics
        .iter()
        .filter(|d| d.is_error())
        .map(|d| format!("line {}: {}", d.span.line, d.message))
        .collect();
    assert!(errors.is_empty(), "the source does not parse: {errors:?}");
    parsed.program.expect("a program")
}

pub fn compile(src: &str, opts: &Options) -> Compiled {
    let prog = parse(src);
    compile_program(&prog, opts).unwrap_or_else(|e| panic!("compile failed: {e}"))
}

pub fn run_wasm(src: &str, opts: &Options) -> Outcome {
    let c = compile(src, opts);
    run_program(&c.wasm).unwrap_or_else(|e| panic!("host failed: {e}"))
}

/// What the interpreter DISPLAYs, one entry per line.
pub fn run_interpreter(src: &str) -> Vec<String> {
    let prog = parse(src);
    let (_event_tx, event_rx) = std::sync::mpsc::channel();
    let (state_tx, _state_rx) = std::sync::mpsc::channel();
    let (display_tx, display_rx) = std::sync::mpsc::channel();
    let mut interp = cobolt_runtime::interpreter::Interpreter::new_with_channels(
        prog, event_rx, state_tx, display_tx,
    );
    let _ = interp.run();
    display_rx.try_iter().collect()
}

pub fn wasm_lines(o: &Outcome) -> Vec<String> {
    let mut v: Vec<String> = o.output.split('\n').map(|s| s.to_string()).collect();
    if v.last().is_some_and(|l| l.is_empty()) {
        v.pop();
    }
    v
}

/// Run on both engines and require identical output.
pub fn same_output(src: &str) -> Vec<String> {
    for multi in [true, false] {
        let opts = Options {
            multi_memory: multi,
            ..Options::default()
        };
        let o = run_wasm(src, &opts);
        assert!(
            o.trapped.is_none() && o.errors.is_empty(),
            "multi={multi}: trapped {:?} errors {:?}",
            o.trapped,
            o.errors
        );
        let want = run_interpreter(src);
        assert_eq!(wasm_lines(&o), want, "multi_memory={multi}\n{src}");
    }
    run_interpreter(src)
}
