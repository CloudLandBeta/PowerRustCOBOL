// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! The examples of the Developer's Guide section "Code style" run, and pass:
//! a property used as a data item, a method's answer used as an expression, a
//! returned record moved into a 01 group, value methods on an item and on a
//! property, expressions in sending positions, and the built-ins written inline.
//! `tests/cobol/code-style/test-code-style.cbl` is the program; this runs it.

use std::path::PathBuf;
use std::sync::mpsc;

use cobolt_lexer::{expand_copybooks, tokenize, SourceFormat};
use cobolt_parser::{parse, Severity};
use cobolt_runtime::Interpreter;

fn detect_format(src: &str) -> SourceFormat {
    let looks_fixed = src.lines().any(|line| {
        let b = line.as_bytes();
        b.len() > 6 && b[6] != b' ' && b[..6].iter().all(|&c| c == b' ' || c.is_ascii_digit())
    });
    if looks_fixed {
        SourceFormat::Fixed
    } else {
        SourceFormat::Free
    }
}

#[test]
fn the_code_style_examples_all_pass() {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/cobol/code-style");
    let src = std::fs::read_to_string(dir.join("test-code-style.cbl")).expect("read .cbl");
    let expanded = expand_copybooks(&src, &dir, detect_format(&src));
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
    interp.run().expect("the examples run");
    let out: Vec<String> = display_rx.try_iter().collect();
    let out = out.join("\n");
    println!("{out}");
    assert!(out.contains("FORMS EXERCISED : 12"), "{out}");
    assert!(out.contains("TESTS RUN       : 0026"), "{out}");
    assert!(out.contains("TESTS FAILED    : 0000"), "{out}");
    assert!(out.contains("OVERALL RESULT  : PASS"), "{out}");
}
