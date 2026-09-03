// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! `ACCEPT identifier FROM <mnemonic-name>` — NIST CCVS85 NC204M.
//!
//! NC204M is NC109M's twin: it reads the operator through a mnemonic instead of
//! a bare Format 1 `ACCEPT`. `SPECIAL-NAMES` associates the implementor's input
//! device with `ACCEPT-INPUT-DEVICE`, and every one of the program's fifteen
//! comparisons then reads through it.
//!
//! The mnemonic never reached the parser: the ordinary `<implementor-name> IS
//! <mnemonic>` clause was skipped a token at a time, so nothing recorded that
//! the name had been declared. `ACCEPT x FROM ACCEPT-INPUT-DEVICE` fell through
//! to the environment-variable extension, read a variable no one had set, and
//! stored nothing.
//!
//! Both readings still exist, and which one applies is decided by the
//! declaration, not by the spelling of the name:
//!
//! * declared in `SPECIAL-NAMES` → Format 1, read the hardware device;
//! * never declared → the non-standard extension, read the environment.

use std::sync::mpsc;

use cobolt_ast::program::ProcedureBody;
use cobolt_ast::stmt::{AcceptSource, Stmt};
use cobolt_lexer::{tokenize, SourceFormat};
use cobolt_parser::{parse, Severity};
use cobolt_runtime::Interpreter;

/// Parse `src` and return the `FROM` source of every `ACCEPT` in it, in order.
fn accept_sources(src: &str) -> Vec<Option<AcceptSource>> {
    let result = parse(tokenize(src, SourceFormat::Free));
    let errors: Vec<String> = result
        .diagnostics
        .iter()
        .filter(|d| d.severity == Severity::Error)
        .map(|d| format!("{}:{}: {}", d.span.line, d.span.col, d.message))
        .collect();
    assert!(errors.is_empty(), "parse errors: {errors:#?}");
    let program = result.program.expect("no program");
    let paragraphs: Vec<_> = match &program.procedure.body {
        ProcedureBody::Paragraphs(ps) => ps.iter().collect(),
        ProcedureBody::Sections(ss) => ss.iter().flat_map(|s| s.paragraphs.iter()).collect(),
    };
    paragraphs
        .iter()
        .flat_map(|p| p.stmts.iter())
        .filter_map(|stmt| match stmt {
            Stmt::Accept { from, .. } => Some(from.clone()),
            _ => None,
        })
        .collect()
}

/// Point this process's real stdin at `/dev/null`.
///
/// A Format 1 `ACCEPT` (no `FROM`, or `FROM` a device mnemonic) reads the
/// interpreter's actual stdin — see `Interpreter::exec_accept` — with no
/// timeout, because a real running program is correct to block there waiting
/// for the operator. This file's `run()` executes real ACCEPT statements
/// through that same path, and the comment two tests down used to bank on
/// "stdin closed under `cargo test`" — true only when the invocation itself
/// redirects or closes it (CI, an agent's non-interactive shell). Run `cargo
/// test -p cobolt-runtime` directly from a terminal, where stdin is the TTY,
/// and the read never returns: this test — and every test after it in this
/// binary, since Rust runs one process for the whole file — hangs forever
/// with no output and no failure (operator, 2026-09-03: two such processes
/// sat blocked for 16 and 18 hours).
///
/// Redirecting the real fd makes the read return EOF immediately regardless
/// of how the binary was invoked, matching the assumption the test always
/// intended rather than one it merely inherited from its environment. `Once`
/// because integration tests in one file share a process and may run on
/// different threads; the redirect only needs to happen, well, once.
fn close_stdin_for_this_process() {
    use std::sync::Once;
    static ONCE: Once = Once::new();
    ONCE.call_once(|| unsafe {
        let devnull = std::ffi::CString::new("/dev/null").expect("no NUL in a literal");
        let fd = libc::open(devnull.as_ptr(), libc::O_RDONLY);
        if fd >= 0 {
            libc::dup2(fd, libc::STDIN_FILENO);
            libc::close(fd);
        }
    });
}

/// Run `src` and return its DISPLAY lines.
fn run(src: &str) -> Vec<String> {
    close_stdin_for_this_process();
    let result = parse(tokenize(src, SourceFormat::Free));
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
    display_rx.try_iter().map(|s| s.trim_end().to_owned()).collect()
}

// ── the classification ───────────────────────────────────────────────────────

/// NC204M's own clause, written exactly as the suite writes it.
#[test]
fn a_declared_mnemonic_is_format_1_not_an_environment_variable() {
    let got = accept_sources(
        r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. ACCMNE.
       ENVIRONMENT DIVISION.
       CONFIGURATION SECTION.
       SPECIAL-NAMES.
           XXXXX057
           IS ACCEPT-INPUT-DEVICE
           XXXXX056
           IS DISPLAY-OUTPUT-DEVICE.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 WS-IN PIC X(10).
       PROCEDURE DIVISION.
       MAIN.
           ACCEPT WS-IN FROM ACCEPT-INPUT-DEVICE.
           STOP RUN.
"#,
    );
    assert_eq!(
        got,
        vec![Some(AcceptSource::Mnemonic("ACCEPT-INPUT-DEVICE".into()))],
        "a mnemonic SPECIAL-NAMES declared must read the device"
    );
}

/// The extension it must not swallow: a name no `SPECIAL-NAMES` clause declares
/// still reads the environment variable of that name.
#[test]
fn an_undeclared_name_still_reads_the_environment() {
    let got = accept_sources(
        r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. ACCENV.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 WS-IN PIC X(10).
       PROCEDURE DIVISION.
       MAIN.
           ACCEPT WS-IN FROM PRC-NOT-DECLARED.
           STOP RUN.
"#,
    );
    assert_eq!(
        got,
        vec![Some(AcceptSource::Environment("PRC-NOT-DECLARED".into()))],
        "an undeclared name is the environment-variable extension"
    );
}

/// `IS` is optional in the clause, as it is in every other SPECIAL-NAMES entry.
#[test]
fn the_is_of_the_mnemonic_clause_is_optional() {
    let got = accept_sources(
        r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. ACCNOIS.
       ENVIRONMENT DIVISION.
       CONFIGURATION SECTION.
       SPECIAL-NAMES.
           CONSOLE CRT-DEVICE.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 WS-IN PIC X(10).
       PROCEDURE DIVISION.
       MAIN.
           ACCEPT WS-IN FROM CRT-DEVICE.
           STOP RUN.
"#,
    );
    assert_eq!(
        got,
        vec![Some(AcceptSource::Mnemonic("CRT-DEVICE".into()))]
    );
}

/// Every built-in source keeps its own reading — the mnemonic arm sits at the
/// end of the chain and must not shadow the registers in front of it.
///
/// `FROM ENVIRONMENT "name"` is absent on purpose: `ENVIRONMENT` lexes as its
/// own keyword (the division opens with it), so `parse_accept_source` never
/// sees it as an identifier and that branch is unreachable — a pre-existing
/// defect this change neither causes nor repairs. No NC program writes it.
#[test]
fn the_built_in_sources_are_untouched() {
    let got = accept_sources(
        r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. ACCBUILT.
       ENVIRONMENT DIVISION.
       CONFIGURATION SECTION.
       SPECIAL-NAMES.
           XXXXX057 IS ACCEPT-INPUT-DEVICE.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 WS-D PIC X(8).
       01 WS-N PIC 9(4).
       PROCEDURE DIVISION.
       MAIN.
           ACCEPT WS-D FROM DATE.
           ACCEPT WS-D FROM TIME.
           ACCEPT WS-D FROM DAY.
           ACCEPT WS-N FROM DAY-OF-WEEK.
           ACCEPT WS-D FROM COMMAND-LINE.
           ACCEPT WS-D FROM ESCAPE KEY.
           STOP RUN.
"#,
    );
    assert_eq!(
        got,
        vec![
            Some(AcceptSource::Date),
            Some(AcceptSource::Time),
            Some(AcceptSource::Day),
            Some(AcceptSource::DayOfWeek),
            Some(AcceptSource::CommandLine),
            Some(AcceptSource::EscapeKey),
        ]
    );
}

// ── what the new clause must not claim ───────────────────────────────────────

/// A switch is `<name> IS <mnemonic> ON STATUS IS <name>` — the same opening
/// shape. It must still be read as a switch, so `SET`/`IF` keep working.
#[test]
fn a_switch_clause_is_not_taken_for_a_mnemonic() {
    let out = run(
        r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. ACCSW.
       ENVIRONMENT DIVISION.
       CONFIGURATION SECTION.
       SPECIAL-NAMES.
           SWITCH-1 IS SW-1
               ON STATUS IS SW-1-ON
               OFF STATUS IS SW-1-OFF.
       PROCEDURE DIVISION.
       MAIN.
           SET SW-1 TO ON.
           IF SW-1-ON DISPLAY "ON" ELSE DISPLAY "NOT-ON".
           SET SW-1 TO OFF.
           IF SW-1-OFF DISPLAY "OFF" ELSE DISPLAY "NOT-OFF".
           STOP RUN.
"#,
    );
    assert_eq!(out, vec!["ON", "OFF"], "{out:#?}");
}

/// The clauses that share the shape but name a facility rather than a device
/// must not be recorded, and must not consume the clause that follows them.
#[test]
fn a_neighbouring_clause_is_not_taken_for_a_mnemonic() {
    let got = accept_sources(
        r##"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. ACCNEIGH.
       ENVIRONMENT DIVISION.
       CONFIGURATION SECTION.
       SPECIAL-NAMES.
           CURRENCY SIGN IS "#"
           DECIMAL-POINT IS COMMA
           CLASS PRC-DIGIT IS "0" THRU "9".
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 WS-IN  PIC X(10).
       01 WS-AMT PIC #(4),99 VALUE 12,34.
       PROCEDURE DIVISION.
       MAIN.
           ACCEPT WS-IN FROM CURRENCY.
           DISPLAY "AMT=[" WS-AMT "]".
           IF "5" IS PRC-DIGIT DISPLAY "DIGIT" ELSE DISPLAY "NOT-DIGIT".
           STOP RUN.
"##,
    );
    assert_eq!(
        got,
        vec![Some(AcceptSource::Environment("CURRENCY".into()))],
        "CURRENCY opens its own clause; it never became a mnemonic"
    );
}

// ── the behaviour it buys ────────────────────────────────────────────────────

/// The observable change: a declared mnemonic no longer picks up an environment
/// variable that happens to carry its name. With stdin closed under `cargo
/// test` the Format 1 read yields nothing, so the field stays spaces — the
/// point is that the variable's value is *not* what lands in it.
#[test]
fn a_declared_mnemonic_ignores_a_same_named_environment_variable() {
    std::env::set_var("PRC-MNEMONIC-TEST", "LEAKED");
    let out = run(
        r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. ACCLEAK.
       ENVIRONMENT DIVISION.
       CONFIGURATION SECTION.
       SPECIAL-NAMES.
           XXXXX057 IS PRC-MNEMONIC-TEST.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 WS-IN PIC X(6) VALUE "......".
       PROCEDURE DIVISION.
       MAIN.
           ACCEPT WS-IN FROM PRC-MNEMONIC-TEST.
           DISPLAY "IN=[" WS-IN "]".
           STOP RUN.
"#,
    );
    assert_ne!(
        out[0], "IN=[LEAKED]",
        "a declared mnemonic must read the device, not the environment"
    );
}

/// …and the extension still works for a name that was never declared.
#[test]
fn an_undeclared_name_reads_the_variable_at_run_time() {
    std::env::set_var("PRC-UNDECLARED-TEST", "READ-ME");
    let out = run(
        r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. ACCEXT.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 WS-IN PIC X(7).
       PROCEDURE DIVISION.
       MAIN.
           ACCEPT WS-IN FROM PRC-UNDECLARED-TEST.
           DISPLAY "IN=[" WS-IN "]".
           STOP RUN.
"#,
    );
    assert_eq!(out[0], "IN=[READ-ME]", "{out:#?}");
}
