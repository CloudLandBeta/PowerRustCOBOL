// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Spec 062 T3 — what a report declaration gets wrong, said before it runs.
//!
//! Both of these fail **silently** without the pass. Every `match` on
//! `FileOrganization` in the product has a `_` arm, so a `MARKDOWN` file with
//! nobody to render it quietly becomes a record-sequential disk file; and page
//! control on a flowing document would simply be dropped, taking the program's
//! page breaks with it.

use cobolt_lexer::{tokenize, SourceFormat};
use cobolt_parser::parse;
use cobolt_semantic::{analyze, Severity};

fn errors_of(src: &str) -> Vec<String> {
    let program = parse(tokenize(src, SourceFormat::Free))
        .program
        .expect("program should parse");
    let sem = analyze(&program);
    let msgs: Vec<String> = sem
        .diagnostics
        .iter()
        .filter(|d| d.severity == Severity::Error)
        .map(|d| d.message.clone())
        .collect();
    for m in &msgs {
        println!("  error: {m}");
    }
    msgs
}

/// A program with one report file, parameterised on its SELECT, its FD and the
/// WRITE — everything else is the same in every case.
fn report_program(select: &str, fd_extra: &str, write: &str) -> String {
    format!(
        "IDENTIFICATION DIVISION.\n\
         PROGRAM-ID. REPORTER.\n\
         ENVIRONMENT DIVISION.\n\
         INPUT-OUTPUT SECTION.\n\
         FILE-CONTROL.\n\
         {select}\n\
         DATA DIVISION.\n\
         FILE SECTION.\n\
         FD  REPORT-FILE{fd_extra}.\n\
         01  REPORT-LINE PIC X(80).\n\
         PROCEDURE DIVISION.\n\
         MAIN-PARA.\n\
         OPEN OUTPUT REPORT-FILE.\n\
         MOVE \"# Title\" TO REPORT-LINE.\n\
         {write}\n\
         CLOSE REPORT-FILE.\n\
         STOP RUN.\n"
    )
}

#[test]
fn markdown_without_a_viewer_is_reported() {
    println!("── a rendered organization on an ordinary file ──");
    let errs = errors_of(&report_program(
        "SELECT REPORT-FILE ASSIGN TO \"report.md\" ORGANIZATION IS MARKDOWN.",
        "",
        "WRITE REPORT-LINE.",
    ));
    assert!(
        errs.iter().any(|m| m.contains("REPORT-FILE") && m.contains("ASSIGN TO VIEWER")),
        "the message must name the file and what it needs"
    );

    println!("── …and the same declaration with a Viewer is accepted ──");
    let ok = errors_of(&report_program(
        "SELECT REPORT-FILE ASSIGN TO VIEWER \"VWR-1\" ORGANIZATION IS MARKDOWN.",
        "",
        "WRITE REPORT-LINE.",
    ));
    assert!(ok.is_empty(), "a proper report must analyse clean: {ok:?}");
}

#[test]
fn page_control_on_a_markdown_report_is_reported() {
    println!("── ADVANCING PAGE ──");
    let page = errors_of(&report_program(
        "SELECT REPORT-FILE ASSIGN TO VIEWER \"VWR-1\" ORGANIZATION IS MARKDOWN.",
        "",
        "WRITE REPORT-LINE AFTER ADVANCING PAGE.",
    ));
    assert!(
        page.iter().any(|m| m.contains("ADVANCING PAGE") && m.contains("REPORT-FILE")),
        "ADVANCING PAGE on a rendered report must be reported, naming the file"
    );

    println!("── LINAGE ──");
    let linage = errors_of(&report_program(
        "SELECT REPORT-FILE ASSIGN TO VIEWER \"VWR-1\" ORGANIZATION IS HTML.",
        "\n    LINAGE IS 60 LINES",
        "WRITE REPORT-LINE.",
    ));
    assert!(
        linage.iter().any(|m| m.contains("LINAGE") && m.contains("REPORT-FILE")),
        "LINAGE on a rendered report must be reported"
    );

    println!("── AT END-OF-PAGE ──");
    let eop = errors_of(&report_program(
        "SELECT REPORT-FILE ASSIGN TO VIEWER \"VWR-1\" ORGANIZATION IS MARKDOWN.",
        "",
        "WRITE REPORT-LINE AT END-OF-PAGE MOVE \"x\" TO REPORT-LINE END-WRITE.",
    ));
    assert!(
        eop.iter().any(|m| m.contains("END-OF-PAGE")),
        "AT END-OF-PAGE on a rendered report must be reported"
    );
}

/// **Blank lines are not page control.** They mean something in all three
/// organizations — in Markdown they are what separates one paragraph from the
/// next — so `ADVANCING n LINES` is accepted everywhere and the runtime emits
/// them. Only `PAGE` is refused on a flowing document.
#[test]
fn advancing_lines_is_not_page_control() {
    let errs = errors_of(&report_program(
        "SELECT REPORT-FILE ASSIGN TO VIEWER \"VWR-1\" ORGANIZATION IS MARKDOWN.",
        "",
        "WRITE REPORT-LINE AFTER ADVANCING 2 LINES.",
    ));
    assert!(
        errs.is_empty(),
        "a blank line is meaningful in Markdown and must be allowed: {errs:?}"
    );
}

/// A paged report says `SEQUENTIAL` and keeps every bit of its page control.
#[test]
fn a_sequential_report_keeps_its_page_control() {
    let errs = errors_of(&report_program(
        "SELECT REPORT-FILE ASSIGN TO VIEWER \"VWR-1\" ORGANIZATION IS SEQUENTIAL.",
        "\n    LINAGE IS 60 LINES WITH FOOTING AT 55",
        "WRITE REPORT-LINE AFTER ADVANCING PAGE.",
    ));
    assert!(errs.is_empty(), "page control on a paged report is the point: {errs:?}");
}
