// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! A file's `FILE STATUS` item belongs to the program that performed the I/O.
//!
//! Motivated by CCVS85 **IC227A** (Inter-program communication), which failed
//! ten assertions on this in one program. It shares one file between a caller
//! and a called program — the FD is declared `IS EXTERNAL` in both — and each
//! names its own status item: `EXTERNAL-FILE-FS` in the caller's
//! WORKING-STORAGE, `LINKAGE-FS` in the callee's LINKAGE SECTION, which is the
//! caller's third argument.
//!
//! The *file* is genuinely shared: that is what `IS EXTERNAL` means, and its
//! open state and position are one thing across the run unit. The *status item*
//! is not — it is named by each program's own `SELECT`, out of that program's
//! own storage.
//!
//! Two separate defects made this fail, and each test below pins one:
//!
//! 1. `build_file_specs` read the outermost program only, so the status item
//!    was permanently the outer program's. Every operation, by either program,
//!    reported into the caller's storage — IC227A's report said
//!    `MAIN PROGRAM FILE STATUS UPDATED` five times.
//! 2. `set_file_status` wrote through `set_str`, which keys by `storage_key`
//!    and follows REDEFINES but **not** the parameter aliases that
//!    `resolve_name` owns. So once (1) was fixed the write landed in a LINKAGE
//!    slot nobody reads, and the caller's argument still never moved —
//!    `UNEXPECTED FILE STATUS VALUE RETURNED`, five more times.

use std::sync::mpsc;

use cobolt_lexer::{tokenize, SourceFormat};
use cobolt_parser::{parse, Severity};
use cobolt_runtime::Interpreter;

fn run_capture(src: &str) -> Vec<String> {
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
    interp.run().expect("run failed");
    display_rx.try_iter().map(|s| s.trim().to_owned()).collect()
}

/// The fixture, with the data file placed under the system temp directory.
///
/// `ASSIGN TO` takes a literal, so the path is woven in rather than declared —
/// otherwise the file lands in whatever directory `cargo test` happened to run
/// from, which is the crate root.
fn shared_file_program() -> String {
    let path = std::env::temp_dir().join("prc-fsscope.dat");
    let _ = std::fs::remove_file(&path);
    SHARED_FILE.replace("@PATH@", &path.display().to_string())
}

/// IC227A's shape, reduced: the callee writes and its status must come back
/// through the caller's argument, while the caller's own item stays untouched.
///
/// `CALLER-FS` is seeded with the sentinel `<>` exactly as IC227A seeds
/// `EXTERNAL-FILE-FS`, so a write into the wrong item is visible rather than
/// merely coincidentally equal.
const SHARED_FILE: &str = r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. FSMAIN.
       ENVIRONMENT DIVISION.
       INPUT-OUTPUT SECTION.
       FILE-CONTROL.
           SELECT WORK-FILE ASSIGN TO "@PATH@"
               ORGANIZATION IS LINE SEQUENTIAL
               FILE STATUS IS CALLER-FS.
       DATA DIVISION.
       FILE SECTION.
       FD  WORK-FILE.
       01  WORK-REC        PIC X(20).
       WORKING-STORAGE SECTION.
       01  CALLER-FS       PIC XX.
       01  PARAM-FS        PIC XX.
       PROCEDURE DIVISION.
       MAIN.
           MOVE "<>" TO CALLER-FS
           MOVE "**" TO PARAM-FS
           OPEN OUTPUT WORK-FILE
           MOVE "<>" TO CALLER-FS
           CALL "FSSUB" USING PARAM-FS
           DISPLAY "CALLER=" CALLER-FS
           DISPLAY "PARAM=" PARAM-FS
           CLOSE WORK-FILE
           STOP RUN.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. FSSUB.
       ENVIRONMENT DIVISION.
       INPUT-OUTPUT SECTION.
       FILE-CONTROL.
           SELECT WORK-FILE ASSIGN TO "@PATH@"
               ORGANIZATION IS LINE SEQUENTIAL
               FILE STATUS IS SUB-FS.
       DATA DIVISION.
       FILE SECTION.
       FD  WORK-FILE.
       01  WORK-REC        PIC X(20).
       LINKAGE SECTION.
       01  SUB-FS          PIC XX.
       PROCEDURE DIVISION USING SUB-FS.
       SUB-MAIN.
           MOVE "HELLO FROM THE SUB" TO WORK-REC
           WRITE WORK-REC
           EXIT PROGRAM.
       END PROGRAM FSSUB.
       END PROGRAM FSMAIN.
"#;

/// Defect (2): the callee's status reaches the caller's argument.
///
/// `SUB-FS` is a LINKAGE item, so it IS the caller's `PARAM-FS` — writing the
/// raw name filled a slot nobody reads and this stayed at its `**` sentinel.
#[test]
fn a_callee_reports_its_status_through_its_linkage_item() {
    let out = run_capture(&shared_file_program());
    let param = out
        .iter()
        .find_map(|l| l.strip_prefix("PARAM="))
        .expect("the program displays PARAM=");
    assert_eq!(
        param, "00",
        "the callee's WRITE succeeded, so its status item — which is the \
         caller's argument — must hold 00, not the {param:?} sentinel"
    );
}

/// Defect (1): the caller's own status item is left alone.
///
/// The caller performed no I/O between seeding the sentinel and reading it
/// back, so anything else means the callee reported into the wrong program's
/// storage. This is IC227A's `MAIN PROGRAM FILE STATUS UPDATED`.
#[test]
fn a_callees_io_does_not_touch_the_callers_status_item() {
    let out = run_capture(&shared_file_program());
    let caller = out
        .iter()
        .find_map(|l| l.strip_prefix("CALLER="))
        .expect("the program displays CALLER=");
    assert_eq!(
        caller, "<>",
        "the caller did no I/O after seeding the sentinel, so its own status \
         item must still hold it — {caller:?} means the callee's WRITE \
         reported into the caller's storage"
    );
}
