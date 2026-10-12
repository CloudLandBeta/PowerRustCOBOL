// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! The "layout" golden rule the plugin gives a coding agent, run: a program
//! written exactly in that layout — SET in place of MOVE, a method called as a
//! statement without INVOKE, no periods inside the procedure, `END-<verb>`
//! scope terminators, `TO` in column 50 — parses and runs.
//! `tests/cobol/code-style/test-layout-style.cbl` is the program.

use std::path::PathBuf;
use std::sync::mpsc;

use cobolt_lexer::{expand_copybooks, tokenize, SourceFormat};
use cobolt_parser::{parse, Severity};
use cobolt_runtime::Interpreter;

#[test]
fn the_layout_style_program_parses_and_passes() {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/cobol/code-style");
    let src = std::fs::read_to_string(dir.join("test-layout-style.cbl")).expect("read .cbl");
    let expanded = expand_copybooks(&src, &dir, SourceFormat::Free);
    assert!(expanded.errors.is_empty(), "copybook errors: {:?}", expanded.errors);
    let result = parse(tokenize(&expanded.text, SourceFormat::Free));
    assert!(
        result.diagnostics.iter().all(|d| d.severity != Severity::Error),
        "parse errors: {:?}",
        result.diagnostics
    );
    let program = result.program.expect("no program");
    let (_event_tx, event_rx) = mpsc::channel();
    let (state_tx, _state_rx) = mpsc::channel();
    let (display_tx, display_rx) = mpsc::channel();
    let mut interp = Interpreter::new_with_channels(program, event_rx, state_tx, display_tx);
    interp.run().expect("the layout program runs");
    let out: Vec<String> = display_rx.try_iter().collect();
    let out = out.join("\n");
    println!("{out}");
    assert!(out.contains("TESTS RUN       : 0007"), "{out}");
    assert!(out.contains("TESTS FAILED    : 0000"), "{out}");
    assert!(out.contains("OVERALL RESULT  : PASS"), "{out}");
}
