// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! A relation whose **subject is an arithmetic expression**, after `AND`/`OR`.
//!
//! `IF A > 0 AND A + 0.3 > B` failed to compile with "unexpected token in
//! statement: Gt": after `AND`, the parser saw `A` followed by `+` and read
//! `A + 0.3` as the object of an abbreviated `A > …`, leaving `> B` unread.
//! COBOL-85 lets either side of a relation be an arithmetic expression, and an
//! abbreviation object is never followed by a relational operator.
//!
//! `tests/cobol/conditionals/test-arith-relation-after-and.cbl` is the
//! quantified program (14 cases); this runs it.

use std::path::PathBuf;
use std::sync::mpsc;

use cobolt_lexer::{expand_copybooks, tokenize, SourceFormat};
use cobolt_parser::{parse, Severity};
use cobolt_runtime::Interpreter;

/// Parse `src` (failing loudly on any error) and return its DISPLAY lines.
fn run(src: &str, dir: &std::path::Path) -> Vec<String> {
    let expanded = expand_copybooks(src, dir, SourceFormat::Fixed);
    assert!(expanded.errors.is_empty(), "copybook errors: {:?}", expanded.errors);
    let result = parse(tokenize(&expanded.text, SourceFormat::Free));
    let errors: Vec<String> = result
        .diagnostics
        .iter()
        .filter(|d| d.severity == Severity::Error)
        .map(|d| format!("{}:{}: {}", d.span.line, d.span.col, d.message))
        .collect();
    assert!(errors.is_empty(), "parse errors: {errors:#?}");
    let program = result.program.expect("no program");
    let (_event_tx, event_rx) = mpsc::channel();
    let (state_tx, _state_rx) = mpsc::channel();
    let (display_tx, display_rx) = mpsc::channel();
    let mut interp = Interpreter::new_with_channels(program, event_rx, state_tx, display_tx);
    interp.run().expect("run failed");
    drop(interp);
    display_rx.try_iter().map(|l| l.trim().to_string()).collect()
}

#[test]
fn the_arithmetic_relation_program_passes() {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/cobol/conditionals");
    let src = std::fs::read_to_string(dir.join("test-arith-relation-after-and.cbl")).expect("read .cbl");
    let out = run(&src, &dir).join("\n");
    println!("{out}");
    assert!(out.contains("CASES EXERCISED : 14"), "{out}");
    assert!(out.contains("TESTS RUN       : 0014"), "{out}");
    assert!(out.contains("TESTS PASSED    : 0014"), "{out}");
    assert!(out.contains("TESTS FAILED    : 0000"), "{out}");
    assert!(out.contains("OVERALL RESULT  : PASS"), "{out}");
}
