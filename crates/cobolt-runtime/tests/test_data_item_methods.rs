// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! The value methods of a data item at run time: the worked examples in
//! `tests/cobol/string-ops/test-data-item-methods.cbl` all pass, every name in
//! `cobolt_ast::methods::DATA_ITEM_METHODS` evaluates, and any other name after
//! a data item is a run-time error instead of an empty string (operator,
//! 2026-10-09). A control's chain and an `OBJECT REFERENCE` are not touched.

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

fn run_source(src: &str, fmt: SourceFormat, dir: &std::path::Path) -> Result<Vec<String>, String> {
    let expanded = expand_copybooks(src, dir, fmt);
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
    match interp.run() {
        Ok(_) => Ok(display_rx.try_iter().collect()),
        Err(e) => Err(e.to_string()),
    }
}

/// A free-form program: the declarations, then `procedure` between MAIN and STOP RUN.
fn run(procedure: &str) -> Result<Vec<String>, String> {
    let src = format!(
        "\
IDENTIFICATION DIVISION.
PROGRAM-ID. DIM.
ENVIRONMENT DIVISION.
CONFIGURATION SECTION.
REPOSITORY.
    CLASS RUST-STRING IS \"Rust.String\"
DATA DIVISION.
WORKING-STORAGE SECTION.
01 WS-TEXT PIC X(20) VALUE \"  Hello  \".
01 WS-OUT  PIC X(40).
01 WS-N    PIC 9(4).
01 WS-TAB.
   05 WS-ROW OCCURS 2 TIMES PIC X(10) VALUE \"abc\".
01 S USAGE IS OBJECT REFERENCE RUST-STRING VALUE \"hello\".
PROCEDURE DIVISION.
MAIN.
{procedure}
    STOP RUN.
"
    );
    run_source(&src, SourceFormat::Free, std::path::Path::new("."))
}

#[test]
fn the_worked_examples_all_pass() {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/cobol/string-ops");
    let src = std::fs::read_to_string(dir.join("test-data-item-methods.cbl")).expect("read .cbl");
    let out = run_source(&src, detect_format(&src), &dir).expect("the examples run").join("\n");
    println!("{out}");
    assert!(out.contains("TESTS RUN       : 0032"), "{out}");
    assert!(out.contains("TESTS FAILED    : 0000"), "{out}");
    assert!(out.contains("OVERALL RESULT  : PASS"), "{out}");
}

#[test]
fn every_listed_value_method_evaluates() {
    for m in cobolt_ast::methods::DATA_ITEM_METHODS {
        let args = if m.eq_ignore_ascii_case("Replace") { "\"a\" \"b\"" } else { "" };
        let code = format!("    MOVE WS-TEXT::{m}({args}) TO WS-OUT.");
        run(&code).unwrap_or_else(|e| panic!("{m}: {e}"));
    }
    run("    MOVE WS-TEXT::Split(\"l\")(2) TO WS-OUT.").expect("Split(sep)(n)");
    run("    COMPUTE WS-N = WS-TEXT::Length.").expect("the bare Length property");
}

#[test]
fn a_name_a_data_item_does_not_answer_is_a_run_time_error() {
    let mut shown = Vec::new();
    for (code, method) in [
        ("    MOVE WS-TEXT::Contains(\"x\") TO WS-OUT.", "CONTAINS"),
        ("    MOVE WS-TEXT::Reverse() TO WS-OUT.", "REVERSE"),
        ("    MOVE WS-TEXT::Value TO WS-OUT.", "VALUE"),
        ("    MOVE WS-TEXT::Trim()::Foo() TO WS-OUT.", "FOO"),
        ("    MOVE WS-ROW(2)::Bar() TO WS-OUT.", "BAR"),
        ("    MOVE WS-TEXT(1:3)::Baz() TO WS-OUT.", "BAZ"),
        ("    DISPLAY WS-TEXT::StartsWith(\"x\").", "STARTSWITH"),
        ("    INVOKE WS-TEXT::Show().", "SHOW"),
    ] {
        let e = run(code).expect_err(code);
        assert!(
            e.to_ascii_uppercase().contains(&format!("NO METHOD OR PROPERTY '{method}'")),
            "{code}: {e}"
        );
        assert!(e.contains("A data item answers: Trim, UpperCase"), "{code}: {e}");
        shown.push(e);
    }
    println!("run-time data-item method check — {} unknown names stopped the run, e.g.:\n  {}", shown.len(), shown[0]);
}

#[test]
fn an_object_reference_still_answers_its_own_class() {
    // `S` is a Rust `String` handle: its methods are the class's, not a data item's.
    let out = run("    DISPLAY S::len().").expect("a handle's method still dispatches");
    assert!(out.iter().any(|l| l.contains('5')), "{out:?}");
}
