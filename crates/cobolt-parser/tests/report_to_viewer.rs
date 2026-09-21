// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Spec 062 T2 — a report declares the Viewer it prints into, and how its lines
//! are to be read.
//!
//! ```cobol
//!        SELECT REPORT-FILE ASSIGN TO VIEWER "VWR-1"
//!            ORGANIZATION IS MARKDOWN.
//! ```
//!
//! Neither `VIEWER` nor `MARKDOWN` nor `HTML` is a lexer keyword: all three
//! arrive as ordinary identifiers and are recognised by name, the way `PAGE`
//! already is in an `ADVANCING` clause. That is deliberate — making them tokens
//! would reserve them, and a COBOL program is entitled to a data item called
//! `HTML`.

use cobolt_ast::program::FileOrganization;
use cobolt_lexer::{tokenize, SourceFormat};
use cobolt_parser::parse;

/// The one `SELECT` of a minimal program, parsed.
fn select_of(clause: &str) -> cobolt_ast::program::FileControl {
    let src = format!(
        "IDENTIFICATION DIVISION.\n\
         PROGRAM-ID. REPORTER.\n\
         ENVIRONMENT DIVISION.\n\
         INPUT-OUTPUT SECTION.\n\
         FILE-CONTROL.\n\
             {clause}\n\
         DATA DIVISION.\n\
         FILE SECTION.\n\
         FD  REPORT-FILE.\n\
         01  REPORT-LINE PIC X(80).\n\
         PROCEDURE DIVISION.\n\
         MAIN-PARA.\n\
             STOP RUN.\n"
    );
    let result = parse(tokenize(&src, SourceFormat::Free));
    assert!(
        result.diagnostics.is_empty(),
        "unexpected diagnostics for {clause}: {:?}",
        result.diagnostics
    );
    result
        .program
        .expect("a program")
        .environment
        .expect("an ENVIRONMENT DIVISION")
        .input_output
        .expect("an INPUT-OUTPUT SECTION")
        .file_controls
        .first()
        .cloned()
        .expect("one SELECT")
}

#[test]
fn a_report_declares_its_viewer_and_its_organization() {
    let cases: &[(&str, FileOrganization, Option<&str>)] = &[
        (
            "SELECT REPORT-FILE ASSIGN TO VIEWER \"VWR-1\" ORGANIZATION IS MARKDOWN.",
            FileOrganization::Markdown,
            Some("VWR-1"),
        ),
        (
            "SELECT REPORT-FILE ASSIGN TO VIEWER \"VWR-1\" ORGANIZATION IS HTML.",
            FileOrganization::Html,
            Some("VWR-1"),
        ),
        (
            "SELECT REPORT-FILE ASSIGN TO VIEWER \"VWR-1\" ORGANIZATION IS SEQUENTIAL.",
            FileOrganization::Sequential,
            Some("VWR-1"),
        ),
        // The words ORGANIZATION IS are optional — IX103A writes the bare form
        // and says so, and a report is entitled to the same shorthand.
        (
            "SELECT REPORT-FILE ASSIGN TO VIEWER \"VWR-1\" MARKDOWN.",
            FileOrganization::Markdown,
            Some("VWR-1"),
        ),
        // COBOL does not care about case; the control id does, and is kept
        // exactly as the literal spelled it.
        (
            "select report-file assign to viewer \"VWR-1\" organization is html.",
            FileOrganization::Html,
            Some("VWR-1"),
        ),
        // An ordinary file is untouched by any of this.
        (
            "SELECT REPORT-FILE ASSIGN TO \"out.txt\" ORGANIZATION IS LINE SEQUENTIAL.",
            FileOrganization::LineSequential,
            None,
        ),
    ];

    for (clause, want_org, want_target) in cases {
        let fc = select_of(clause);
        println!(
            "  {:<62} → assign {:<10} target {:<8} org {:?}",
            clause.trim_end_matches('.'),
            fc.assign,
            format!("{:?}", fc.viewer_target),
            fc.organization
        );
        assert_eq!(fc.organization, *want_org, "organization for {clause}");
        assert_eq!(
            fc.viewer_target.as_deref(),
            *want_target,
            "viewer target for {clause}"
        );
    }
}

/// `VIEWER` with no literal after it is a declaration missing its control, not
/// a different device: the device is still recognised, the target stays `None`,
/// and the runtime is what reports it — with the id it was not given.
#[test]
fn a_viewer_with_no_control_id_parses_and_names_nothing() {
    let fc = select_of("SELECT REPORT-FILE ASSIGN TO VIEWER ORGANIZATION IS MARKDOWN.");
    assert_eq!(fc.assign.to_uppercase(), "VIEWER");
    assert_eq!(fc.viewer_target, None);
    assert_eq!(fc.organization, FileOrganization::Markdown);
}

/// A data item called `HTML` still works, which is the whole reason these are
/// words rather than tokens.
#[test]
fn html_is_still_available_as_a_data_name() {
    let src = "IDENTIFICATION DIVISION.\n\
               PROGRAM-ID. NAMER.\n\
               DATA DIVISION.\n\
               WORKING-STORAGE SECTION.\n\
               01  HTML PIC X(10).\n\
               01  MARKDOWN PIC 9(4).\n\
               PROCEDURE DIVISION.\n\
               MAIN-PARA.\n\
                   MOVE \"x\" TO HTML.\n\
                   STOP RUN.\n";
    let result = parse(tokenize(src, SourceFormat::Free));
    assert!(
        result.diagnostics.is_empty(),
        "HTML and MARKDOWN must still be ordinary data names: {:?}",
        result.diagnostics
    );
}
