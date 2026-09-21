// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Spec 062 T12 — the COBOL programs of `tests/cobol/report-viewer/`, run.
//!
//! Each one prints a **single result block** at the end (GOLDEN RULE #7): the
//! organization, the `WRITE` forms it exercised by name, how many records it
//! wrote, how many pages it produced, how long that took and at what rate, and
//! its own pass/fail tally. No per-record DISPLAY — a thousand lines of
//! "wrote record 417" tells a reader nothing a total does not.
//!
//! This driver asserts the block says PASS, and then checks the document the
//! program actually produced: the runtime's own bytes, not the program's
//! opinion of them.

use cobolt_lexer::{tokenize, SourceFormat};
use cobolt_parser::parse;
use cobolt_runtime::interpreter::Interpreter;
use std::sync::mpsc;

/// Run one of the pack's programs with a form's channels attached, and give
/// back what it displayed and the document it printed.
fn run_report(file: &str) -> (Vec<String>, String) {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/cobol/report-viewer")
        .join(file);
    let src = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    let result = parse(tokenize(&src, SourceFormat::Fixed));
    assert!(
        result.diagnostics.is_empty(),
        "{file} must parse clean: {:?}",
        result.diagnostics
    );
    let program = result.program.expect("a program");

    let (_event_tx, event_rx) = mpsc::channel();
    let (state_tx, state_rx) = mpsc::channel();
    let (display_tx, display_rx) = mpsc::channel();
    let mut interp = Interpreter::new_with_channels(program, event_rx, state_tx, display_tx);
    interp.run().expect("run failed");
    drop(interp);

    let said: Vec<String> = display_rx.into_iter().map(|s| s.trim_end().to_owned()).collect();
    let source = state_rx
        .into_iter()
        .filter(|u| u.prop.eq_ignore_ascii_case("View1Source"))
        .map(|u| u.value)
        .last()
        .unwrap_or_else(|| panic!("{file} never handed a report to the Viewer"));
    let document = std::fs::read_to_string(&source).expect("read the report back");
    let _ = std::fs::remove_file(&source);
    (said, document)
}

/// The block is the point: print it, then hold it to what it claims.
fn check_block(file: &str, said: &[String]) {
    println!("\n{}", said.join("\n"));
    let joined = said.join("\n");
    assert!(
        joined.contains("REPORT TO VIEWER"),
        "{file} must print its result block"
    );
    let fail_line = said
        .iter()
        .find(|l| l.contains("FAIL            :"))
        .unwrap_or_else(|| panic!("{file} must report a FAIL tally"));
    assert!(
        fail_line.trim_end().ends_with('0'),
        "{file} reported failures: {fail_line}"
    );
    assert!(
        !joined.contains("FAIL: "),
        "{file} reported a failing assertion: {joined}"
    );
}

#[test]
fn a_markdown_report_renders_as_markdown() {
    let (said, doc) = run_report("markdown-report.cbl");
    check_block("markdown-report.cbl", &said);
    assert!(doc.starts_with("# Quarterly Sales\n"), "a heading is a heading");
    assert!(
        doc.contains("\n\nWritten from COBOL"),
        "ADVANCING 2 LINES is what separates one Markdown paragraph from the next"
    );
    assert!(doc.contains("| North  | 1,204 |"), "the table rows are there");
    assert_eq!(
        doc.lines().filter(|l| l.starts_with("| North")).count(),
        500,
        "every record written is a line in the document"
    );
}

#[test]
fn an_html_report_renders_as_html() {
    let (said, doc) = run_report("html-report.cbl");
    check_block("html-report.cbl", &said);
    assert!(doc.starts_with("<h1>Quarterly Sales</h1>\n"));
    assert!(doc.trim_end().ends_with("</table>"));
    assert_eq!(doc.lines().filter(|l| l.starts_with("<tr><td>")).count(), 500);
}

#[test]
fn a_sequential_report_is_paged_text() {
    let (said, doc) = run_report("sequential-report.cbl");
    check_block("sequential-report.cbl", &said);
    assert!(doc.starts_with("SALES BY REGION"));
    assert_eq!(
        doc.matches('\u{000C}').count(),
        1,
        "one ADVANCING PAGE, one form feed"
    );
    // …and that form feed is a page boundary the Viewer finds.
    let source = cobolt_forms::viewer::DocumentSource::Bytes(doc.as_bytes().into());
    let index = cobolt_forms::viewer::index_text(&source).expect("index");
    println!("  the Viewer reads it as {} pages", index.page_count());
    assert_eq!(index.page_count(), 2, "two pages, as the program wrote them");
}
