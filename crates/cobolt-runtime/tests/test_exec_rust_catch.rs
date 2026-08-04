// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Spec 041 R23–R25 — how a contained Rust panic meets `TRY`/`CATCH`.
//!
//! Each clause catches only its own class: a COBOL exception never reaches
//! `CATCH RUST-EXCEPTION`, and a panic is never swallowed by a plain
//! `CATCH EXCEPTION`. These run a whole COBOL program with a *registered*
//! block that panics, which is why they live here rather than in T5 — until
//! blocks were dispatched there was no way to raise a panic from COBOL at all.

use std::sync::mpsc;

use cobolt_lexer::{tokenize, SourceFormat};
use cobolt_parser::{parse, Severity};
use cobolt_runtime::exec_rust::ExecRustContext;
use cobolt_runtime::Interpreter;

/// The compiled body stands in for what `cobolt-compiler` will emit: it panics.
fn panicking_block(_ctx: &mut ExecRustContext<'_>) {
    panic!("index out of range");
}

/// Run `src`, registering [`panicking_block`] as block 0, and return whatever
/// the program DISPLAYed plus the run's result.
fn run(src: &str) -> (Vec<String>, Result<(), String>) {
    let result = parse(tokenize(src, SourceFormat::Free));
    assert!(
        result
            .diagnostics
            .iter()
            .all(|d| d.severity != Severity::Error),
        "parse errors: {:?}",
        result.diagnostics
    );
    let program = result.program.expect("no program");
    let (_event_tx, event_rx) = mpsc::channel();
    let (state_tx, _state_rx) = mpsc::channel();
    let (display_tx, display_rx) = mpsc::channel();
    let mut interp = Interpreter::new_with_channels(program, event_rx, state_tx, display_tx);
    interp.exec_rust.register(0, panicking_block);
    let outcome = interp.run().map_err(|e| e.to_string());
    let out = display_rx.try_iter().map(|s| s.trim().to_owned()).collect();
    (out, outcome)
}

fn program_with(try_body: &str) -> String {
    format!(
        "\
       IDENTIFICATION DIVISION.
       PROGRAM-ID. PANICDEMO.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 E PIC X(80).
       01 R PIC X(80).
       PROCEDURE DIVISION.
       MAIN.
{try_body}
           STOP RUN.
"
    )
}

/// **AC11** — a panic is caught by `CATCH RUST-EXCEPTION`, the program carries
/// on, and `DISPLAY` of the bound name prints the message in plain text. No
/// substring inspection anywhere (R23).
#[test]
fn a_panic_is_caught_by_catch_rust_exception() {
    let (out, outcome) = run(&program_with(
        "           TRY\n\
         \x20              EXEC RUST\n\
         \x20                  anything();\n\
         \x20              END-EXEC\n\
         \x20          CATCH RUST-EXCEPTION R\n\
         \x20              DISPLAY R\n\
         \x20          END-TRY.\n\
         \x20          DISPLAY 'CONTINUED'",
    ));
    assert!(outcome.is_ok(), "the run should survive: {outcome:?}");
    assert!(
        out.iter().any(|l| l.contains("index out of range")),
        "the panic message did not reach DISPLAY: {out:?}"
    );
    assert!(
        out.iter().any(|l| l == "CONTINUED"),
        "execution did not continue past the TRY: {out:?}"
    );
}

/// **AC17** — the same panic with only a plain `CATCH EXCEPTION` is NOT caught.
/// Folding a memory-safety or logic fault into the general handler would let it
/// be reported as a business error (R24), so it propagates and ends the run
/// (R25).
#[test]
fn a_panic_is_not_caught_by_a_plain_catch() {
    let (out, outcome) = run(&program_with(
        "           TRY\n\
         \x20              EXEC RUST\n\
         \x20                  anything();\n\
         \x20              END-EXEC\n\
         \x20          CATCH EXCEPTION E\n\
         \x20              DISPLAY 'WRONG CLAUSE'\n\
         \x20          END-TRY.\n\
         \x20          DISPLAY 'CONTINUED'",
    ));
    assert!(
        outcome.is_err(),
        "the panic must propagate, not be swallowed: {out:?}"
    );
    assert!(
        !out.iter().any(|l| l == "WRONG CLAUSE"),
        "the plain clause ran for a Rust panic: {out:?}"
    );
    assert!(
        !out.iter().any(|l| l == "CONTINUED"),
        "execution continued after an uncaught panic: {out:?}"
    );
}

/// **AC18** — with both clauses present, the panic goes to the Rust one and the
/// COBOL clause stays untouched.
#[test]
fn both_clauses_route_to_their_own_class() {
    let (out, outcome) = run(&program_with(
        "           TRY\n\
         \x20              EXEC RUST\n\
         \x20                  anything();\n\
         \x20              END-EXEC\n\
         \x20          CATCH EXCEPTION E\n\
         \x20              DISPLAY 'COBOL CLAUSE'\n\
         \x20          CATCH RUST-EXCEPTION R\n\
         \x20              DISPLAY 'RUST CLAUSE'\n\
         \x20          END-TRY.",
    ));
    assert!(outcome.is_ok(), "the run should survive: {outcome:?}");
    assert!(
        out.iter().any(|l| l == "RUST CLAUSE"),
        "the Rust clause did not run: {out:?}"
    );
    assert!(
        !out.iter().any(|l| l == "COBOL CLAUSE"),
        "the COBOL clause ran for a Rust panic: {out:?}"
    );
}

/// A COBOL `THROW` still reaches the plain clause and never the Rust one — the
/// other direction of R24.
#[test]
fn a_cobol_exception_does_not_reach_the_rust_clause() {
    let (out, outcome) = run(&program_with(
        "           TRY\n\
         \x20              THROW 'business rule'\n\
         \x20          CATCH EXCEPTION E\n\
         \x20              DISPLAY 'COBOL CLAUSE'\n\
         \x20          CATCH RUST-EXCEPTION R\n\
         \x20              DISPLAY 'RUST CLAUSE'\n\
         \x20          END-TRY.",
    ));
    assert!(outcome.is_ok(), "the run should survive: {outcome:?}");
    assert!(
        out.iter().any(|l| l == "COBOL CLAUSE"),
        "the COBOL clause did not run: {out:?}"
    );
    assert!(
        !out.iter().any(|l| l == "RUST CLAUSE"),
        "a COBOL exception reached the Rust clause: {out:?}"
    );
}
