// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! `onUnhandledException` — the form CONTINUES (operator ruling, 2026-09-06).

use std::sync::mpsc;

use cobolt_lexer::{tokenize, SourceFormat};
use cobolt_parser::{parse, Severity};
use cobolt_runtime::Interpreter;

/// Run `src` as a form program, returning (DISPLAY lines, state updates).
fn run_as_form(src: &str) -> (Vec<String>, Vec<cobolt_runtime::channels::StateUpdate>) {
    let parsed = parse(tokenize(src, SourceFormat::Free));
    assert!(
        parsed
            .diagnostics
            .iter()
            .all(|d| d.severity != Severity::Error),
        "parse errors: {:?}",
        parsed.diagnostics
    );
    let program = parsed.program.expect("program");
    let (_ev_tx, ev_rx) = mpsc::channel();
    let (state_tx, state_rx) = mpsc::channel();
    let (display_tx, display_rx) = mpsc::channel();
    let mut interp = Interpreter::new_with_channels(program, ev_rx, state_tx, display_tx);
    // Being "under a form" is what turns the ruling on; a console program that
    // fails must still fail loudly.
    let (form_tx, _form_rx) = mpsc::channel();
    let (_closed_tx, closed_rx) = mpsc::channel();
    interp.set_form_host(
        form_tx,
        cobolt_runtime::form_host::ROOT_HANDLE,
        "MAIN-FORM",
        closed_rx,
    );
    let _ = interp.run();
    drop(interp);
    (
        display_rx.try_iter().collect(),
        state_rx.try_iter().collect(),
    )
}

/// A handler that divides by zero, and a main flow that carries on after it.
fn program(with_handler: bool) -> String {
    let handler = if with_handler {
        r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. MAIN-FORM--ONUNHANDLEDEXCEPTION IS COMMON PROGRAM.
       PROCEDURE DIVISION.
           DISPLAY "CAUGHT".
       END PROGRAM MAIN-FORM--ONUNHANDLEDEXCEPTION.
"#
    } else {
        ""
    };
    format!(
        r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. MAIN-FORM.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 WS-A PIC 9(4) VALUE 1.
       01 WS-Z PIC 9(4) VALUE 0.
       PROCEDURE DIVISION.
       COBOL-MAIN.
           DISPLAY "BEFORE"
           CALL "MAIN-FORM--BOOM"
           DISPLAY "AFTER"
           STOP RUN.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. MAIN-FORM--BOOM IS COMMON PROGRAM.
       PROCEDURE DIVISION.
           GO TO NO-SUCH-PARAGRAPH
           DISPLAY "NOT REACHED".
       END PROGRAM MAIN-FORM--BOOM.
{handler}
       END PROGRAM MAIN-FORM.
"#
    )
}

/// **The form continues.** A handler that throws must not end the run — the
/// statement after the CALL still executes.
#[test]
fn an_exception_in_a_handler_does_not_end_the_form() {
    let (out, _) = run_as_form(&program(false));
    assert!(out.iter().any(|l| l.contains("BEFORE")), "{out:?}");
    assert!(
        out.iter().any(|l| l.contains("AFTER")),
        "the form must carry on past a handler that threw: {out:?}"
    );
    assert!(
        !out.iter().any(|l| l.contains("NOT REACHED")),
        "…but the failing handler itself is abandoned: {out:?}"
    );
}

/// **With no handler bound, the operator is told** — in the words the ruling
/// specifies, as a critical notification the host raises with no Snackbar
/// control needed.
#[test]
fn with_no_handler_the_operator_gets_the_critical_message() {
    let (_, updates) = run_as_form(&program(false));
    let critical = updates
        .iter()
        .find(|u| u.prop == "_CriticalException")
        .expect("a critical notification must be raised");
    assert_eq!(critical.ctrl_id, "MAIN-FORM", "raised on the form itself");
    assert!(
        critical.value.starts_with("A critical exception has occurred: "),
        "the wording is the ruling's: {:?}",
        critical.value
    );
    assert!(
        critical.value.ends_with(
            "Implement the event handler onUnhandledException to get better \
             control over the exception."
        ),
        "…including the sentence that names the way out: {:?}",
        critical.value
    );
}

/// **A bound handler is called instead**, and no notification is raised: the
/// developer took control, so the host must not talk over them.
#[test]
fn a_bound_handler_is_called_and_silences_the_notification() {
    let (out, updates) = run_as_form(&program(true));
    assert!(
        out.iter().any(|l| l.contains("CAUGHT")),
        "onUnhandledException must run: {out:?}"
    );
    assert!(
        out.iter().any(|l| l.contains("AFTER")),
        "and the form still continues: {out:?}"
    );
    assert!(
        !updates.iter().any(|u| u.prop == "_CriticalException"),
        "a form that handles its own exceptions is not talked over"
    );
    // The details are readable from the handler.
    assert!(
        updates
            .iter()
            .any(|u| u.ctrl_id == "MAIN-FORM" && u.prop == "LastException"),
        "the details are published as LastException on the form"
    );
}

/// **A console program still fails loudly.** The ruling is about FORMS: a
/// program with no window has nowhere to show a notification, and swallowing
/// its failure would hide it completely.
#[test]
fn a_console_program_still_fails() {
    let src = program(false);
    let parsed = parse(tokenize(&src, SourceFormat::Free));
    let interp_program = parsed.program.expect("program");
    // No form host attached — this is the console path.
    let mut interp = Interpreter::new(interp_program);
    let err = interp.run().expect_err("the failure must reach the caller");
    assert!(
        !err.is_exit_signal(),
        "a real failure, not STOP RUN: {err}"
    );
}

/// The message is reproduced here in full, so a change to its wording has to be
/// a deliberate edit to this test as well (operator specified it verbatim).
#[test]
fn the_critical_message_is_worded_exactly_as_specified() {
    let (_, updates) = run_as_form(&program(false));
    let text = updates
        .iter()
        .find(|u| u.prop == "_CriticalException")
        .map(|u| u.value.clone())
        .expect("a critical notification");
    let expected_head = "A critical exception has occurred: ";
    let expected_tail = ". Implement the event handler onUnhandledException \
to get better control over the exception.";
    assert!(text.starts_with(expected_head), "{text:?}");
    assert!(
        text.ends_with(expected_tail.trim_start()),
        "tail mismatch.\n got: {text:?}\nwant ending: {expected_tail:?}"
    );
    // …and the exception's own words sit between the two.
    let detail = &text[expected_head.len()..text.len() - expected_tail.trim_start().len()];
    assert!(!detail.trim().is_empty(), "the details must not be empty");
    println!("critical notification: {text}");
}

/// **An unguarded size error becomes an exception — under a form.**
///
/// COBOL-85 leaves the result undefined when `ON SIZE ERROR` is absent, and
/// leaving the receiver untouched is also how a wrong total reaches a report
/// with no sign anything went wrong. Under a form, nobody handling it means
/// `onUnhandledException` should hear about it (operator ruling, 2026-09-06).
#[test]
fn an_unguarded_size_error_is_an_exception_under_a_form() {
    let src = r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. MAIN-FORM.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 WS-A PIC 9(4) VALUE 1.
       01 WS-Z PIC 9(4) VALUE 0.
       PROCEDURE DIVISION.
       COBOL-MAIN.
           DISPLAY "BEFORE"
           CALL "MAIN-FORM--BOOM"
           DISPLAY "AFTER"
           STOP RUN.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. MAIN-FORM--BOOM IS COMMON PROGRAM.
       PROCEDURE DIVISION.
           DIVIDE WS-A BY WS-Z GIVING WS-A
           DISPLAY "NOT REACHED".
       END PROGRAM MAIN-FORM--BOOM.
       END PROGRAM MAIN-FORM.
"#;
    let (out, updates) = run_as_form(src);
    assert!(
        !out.iter().any(|l| l.contains("NOT REACHED")),
        "the unguarded DIVIDE must abandon its handler: {out:?}"
    );
    assert!(
        out.iter().any(|l| l.contains("AFTER")),
        "…and the form still continues: {out:?}"
    );
    let text = updates
        .iter()
        .find(|u| u.prop == "_CriticalException")
        .map(|u| u.value.clone())
        .expect("the operator is told");
    assert!(text.contains("size error"), "{text:?}");
}

/// **A declared `ON SIZE ERROR` is still the developer's to handle** — the two
/// error models stay apart, which is the whole point of the phrase.
#[test]
fn a_declared_on_size_error_is_not_an_exception() {
    let src = r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. MAIN-FORM.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 WS-A PIC 9(4) VALUE 1.
       01 WS-Z PIC 9(4) VALUE 0.
       PROCEDURE DIVISION.
       COBOL-MAIN.
           CALL "MAIN-FORM--GUARDED"
           DISPLAY "AFTER"
           STOP RUN.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. MAIN-FORM--GUARDED IS COMMON PROGRAM.
       PROCEDURE DIVISION.
           DIVIDE WS-A BY WS-Z GIVING WS-A
               ON SIZE ERROR DISPLAY "HANDLED"
           END-DIVIDE
           DISPLAY "CARRIED ON".
       END PROGRAM MAIN-FORM--GUARDED.
       END PROGRAM MAIN-FORM.
"#;
    let (out, updates) = run_as_form(src);
    assert!(out.iter().any(|l| l.contains("HANDLED")), "{out:?}");
    assert!(
        out.iter().any(|l| l.contains("CARRIED ON")),
        "the handler runs on past its own ON SIZE ERROR: {out:?}"
    );
    assert!(
        !updates.iter().any(|u| u.prop == "_CriticalException"),
        "a declared phrase must NOT also raise an exception"
    );
}
