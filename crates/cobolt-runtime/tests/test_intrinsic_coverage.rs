// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! The intrinsic-function list and the implementation must agree.
//!
//! `cobolt_ast::intrinsics::INTRINSIC_FUNCTIONS` is what the semantic analyser
//! checks a `FUNCTION` name against, and `Interpreter::eval_function` is what
//! actually computes it. They live in different crates because
//! `cobolt-semantic` cannot depend on `cobolt-runtime`, which means they can
//! drift — and a drifted list is worse than no list at all:
//!
//! * a name in the list but not in `eval_function` compiles and then fails at
//!   run time, which is exactly the silent-zero failure the list exists to end;
//! * a name in `eval_function` but not in the list is rejected at compile time
//!   even though it works.
//!
//! These tests close both directions.

use std::sync::mpsc;

use cobolt_ast::intrinsics::INTRINSIC_FUNCTIONS;
use cobolt_lexer::{tokenize, SourceFormat};
use cobolt_parser::parse;
use cobolt_runtime::Interpreter;

/// Run a program and return `Err(message)` if the interpreter refused it.
fn run(src: &str) -> Result<Vec<String>, String> {
    let result = parse(tokenize(src, SourceFormat::Free));
    let program = result.program.ok_or_else(|| "no program".to_string())?;
    let (_event_tx, event_rx) = mpsc::channel();
    let (state_tx, _state_rx) = mpsc::channel();
    let (display_tx, display_rx) = mpsc::channel();
    let mut interp = Interpreter::new_with_channels(program, event_rx, state_tx, display_tx);
    match interp.run() {
        Ok(()) => Ok(display_rx.try_iter().collect()),
        Err(e) => Err(format!("{e:?}")),
    }
}

/// A program that calls `name` with `args`, ignoring whether the *result* is
/// meaningful — only whether the function is recognised.
fn call(name: &str, args: &str) -> Result<Vec<String>, String> {
    run(&format!(
        "       IDENTIFICATION DIVISION.\n\
         \x20      PROGRAM-ID. FN.\n\
         \x20      DATA DIVISION.\n\
         \x20      WORKING-STORAGE SECTION.\n\
         \x20      01  WS-N PIC S9(9)V9(4).\n\
         \x20      01  WS-A PIC X(40) VALUE \"ABCDE\".\n\
         \x20      PROCEDURE DIVISION.\n\
         \x20      MAIN.\n\
         \x20          MOVE FUNCTION {name}({args}) TO WS-N.\n\
         \x20          STOP RUN.\n"
    ))
}

/// The message `eval_function` produces for a name it does not implement.
/// Matching on it is what makes "is this name implemented?" answerable at all.
fn is_unimplemented(err: &str) -> bool {
    err.contains("is not an intrinsic function RustCOBOL implements")
}

/// **Every listed name is implemented.** A name may still fail for another
/// reason here — a wrong argument type or count for the dummy call — and that
/// is fine; what must never happen is the *unimplemented* error.
#[test]
fn every_listed_intrinsic_is_implemented() {
    let mut missing = Vec::new();
    for name in INTRINSIC_FUNCTIONS {
        // Alphanumeric-only functions need a string; the rest take a number.
        // Getting this wrong costs nothing — a type error is not the error
        // this test looks for.
        let args = match *name {
            "UPPER-CASE" | "LOWER-CASE" | "REVERSE" | "TRIM" | "LENGTH" | "LENGTH-AN"
            | "BYTE-LENGTH" | "ORD" | "NUMVAL" | "NUMVAL-C" | "NUMVAL-F" | "TEST-NUMVAL"
            | "STORED-CHAR-LENGTH" | "CONCATENATE" => "WS-A",
            "CURRENT-DATE" | "WHEN-COMPILED" | "PI" | "RANDOM" => "",
            "ANNUITY" | "MOD" | "REM" | "PRESENT-VALUE" | "DATE-OF-INTEGER"
            | "INTEGER-OF-DATE" | "INTEGER-OF-DAY" | "DAY-OF-INTEGER" | "YEAR-TO-YYYY" => "1, 1",
            _ => "1",
        };
        if let Err(e) = call(name, args) {
            if is_unimplemented(&e) {
                missing.push(*name);
            }
        }
    }
    assert!(
        missing.is_empty(),
        "listed in cobolt_ast::intrinsics but NOT implemented by eval_function: {missing:?}\n\
         Either implement them or remove them from the list — a name in the list \
         compiles and then fails at run time."
    );
}

/// **An unknown name is refused, not answered with zero.** This is the defect
/// the list exists to end: a typo used to compute a confident wrong number.
#[test]
fn an_unknown_intrinsic_is_refused_at_compile_time() {
    let src = "       IDENTIFICATION DIVISION.\n\
               \x20      PROGRAM-ID. FN.\n\
               \x20      DATA DIVISION.\n\
               \x20      WORKING-STORAGE SECTION.\n\
               \x20      01  WS-N PIC S9(9).\n\
               \x20      PROCEDURE DIVISION.\n\
               \x20      MAIN.\n\
               \x20          MOVE FUNCTION NO-SUCH-THING(1) TO WS-N.\n\
               \x20          STOP RUN.\n";
    let result = parse(tokenize(src, SourceFormat::Free));
    let program = result.program.expect("should still parse");
    let diags = cobolt_semantic::analyze(&program).diagnostics;
    let named: Vec<&str> = diags
        .iter()
        .filter(|d| d.severity == cobolt_semantic::Severity::Error)
        .map(|d| d.message.as_str())
        .filter(|m| m.contains("NO-SUCH-THING"))
        .collect();
    assert!(
        !named.is_empty(),
        "an unimplemented FUNCTION must be a compile error naming it; got: {:?}",
        diags.iter().map(|d| &d.message).collect::<Vec<_>>()
    );
}

/// A near miss says what was probably meant. `SQRTT` is a typo, not a new
/// function, and saying so is most of the value of the diagnostic.
#[test]
fn a_misspelled_intrinsic_suggests_the_real_one() {
    let src = "       IDENTIFICATION DIVISION.\n\
               \x20      PROGRAM-ID. FN.\n\
               \x20      DATA DIVISION.\n\
               \x20      WORKING-STORAGE SECTION.\n\
               \x20      01  WS-N PIC S9(9).\n\
               \x20      PROCEDURE DIVISION.\n\
               \x20      MAIN.\n\
               \x20          MOVE FUNCTION SQRTT(4) TO WS-N.\n\
               \x20          STOP RUN.\n";
    let program = parse(tokenize(src, SourceFormat::Free))
        .program
        .expect("should still parse");
    let diags = cobolt_semantic::analyze(&program).diagnostics;
    assert!(
        diags.iter().any(|d| d.message.contains("did you mean FUNCTION SQRT")),
        "expected a suggestion; got: {:?}",
        diags.iter().map(|d| &d.message).collect::<Vec<_>>()
    );
}

/// An implemented function still compiles clean — the check must not become a
/// wall in front of working code.
#[test]
fn implemented_intrinsics_still_compile_clean() {
    for expr in [
        "FUNCTION SQRT(4)",
        "FUNCTION MAX(1, 2)",
        "FUNCTION LENGTH(WS-A)",
        "FUNCTION CURRENT-DATE",
    ] {
        let src = format!(
            "       IDENTIFICATION DIVISION.\n\
             \x20      PROGRAM-ID. FN.\n\
             \x20      DATA DIVISION.\n\
             \x20      WORKING-STORAGE SECTION.\n\
             \x20      01  WS-N PIC S9(9)V9(4).\n\
             \x20      01  WS-A PIC X(40) VALUE \"ABCDE\".\n\
             \x20      PROCEDURE DIVISION.\n\
             \x20      MAIN.\n\
             \x20          MOVE {expr} TO WS-N.\n\
             \x20          STOP RUN.\n"
        );
        let program = parse(tokenize(&src, SourceFormat::Free))
            .program
            .expect("should parse");
        let diags = cobolt_semantic::analyze(&program).diagnostics;
        let errors: Vec<&String> = diags
            .iter()
            .filter(|d| d.severity == cobolt_semantic::Severity::Error)
            .map(|d| &d.message)
            .collect();
        assert!(errors.is_empty(), "{expr} should compile clean: {errors:?}");
    }
}

/// **The other direction: implemented but NOT listed.**
///
/// This is the failure that actually happened. `VARIANCE` was implemented and
/// left out of the list when the list was first transcribed by hand, so two
/// working NIST programs (IF124A, IF134A) started failing to compile — the
/// check rejected code that ran correctly. The test above cannot see it,
/// because it only walks the list.
///
/// There is no way to reflect over a `match`, so this reads the interpreter's
/// own source and collects the names in `eval_function`'s arms. It is coupled
/// to the shape of that function on purpose: the coupling is what makes the
/// two lists impossible to desynchronise silently.
#[test]
fn every_implemented_intrinsic_is_listed() {
    const SRC: &str = include_str!("../src/interpreter.rs");

    let body = SRC
        .split_once("fn eval_function(")
        .expect("eval_function must exist — update this test if it is renamed")
        .1;
    // The arms end where the fallback arm begins.
    let body = body
        .split_once("other => Err(RuntimeError::General")
        .expect("the unimplemented-intrinsic fallback arm must exist")
        .0;

    let mut implemented: Vec<String> = Vec::new();
    for line in body.lines() {
        let t = line.trim_start();
        // Match-arm shapes only: `"NAME" => {`, `"A" | "B" => {`, `| "C"`.
        if !(t.starts_with('"') || t.starts_with("| \"")) {
            continue;
        }
        if !t.contains("=>") && !t.ends_with('|') && !t.starts_with("| \"") {
            continue;
        }
        let mut rest = t;
        while let Some(open) = rest.find('"') {
            let after = &rest[open + 1..];
            let Some(close) = after.find('"') else { break };
            let name = &after[..close];
            if !name.is_empty()
                && name
                    .chars()
                    .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == '-')
            {
                implemented.push(name.to_string());
            }
            rest = &after[close + 1..];
        }
    }
    implemented.sort();
    implemented.dedup();
    assert!(
        implemented.len() > 40,
        "the scraper found only {} names — eval_function's shape probably changed, \
         and this test is no longer guarding anything: {implemented:?}",
        implemented.len()
    );

    let unlisted: Vec<&String> = implemented
        .iter()
        .filter(|n| !cobolt_ast::intrinsics::is_intrinsic(n))
        .collect();
    assert!(
        unlisted.is_empty(),
        "implemented by eval_function but MISSING from cobolt_ast::intrinsics: {unlisted:?}\n\
         The semantic analyser will reject these as unknown, so working programs \
         stop compiling. Add them to INTRINSIC_FUNCTIONS."
    );
}
