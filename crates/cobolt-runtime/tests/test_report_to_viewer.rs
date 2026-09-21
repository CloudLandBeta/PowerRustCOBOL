// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Spec 062 T5–T8 — a report is opened, written, advanced and handed over.
//!
//! Everything here runs a real program through the interpreter with a state
//! channel attached, and reads the `StateUpdate`s the way a form host would.
//! That channel is the whole of the hand-over: `CLOSE` writes `View1Source`,
//! and a host turns that into a document.

use cobolt_lexer::{tokenize, SourceFormat};
use cobolt_parser::parse;
use cobolt_runtime::channels::StateUpdate;
use cobolt_runtime::interpreter::Interpreter;
use std::sync::mpsc;

/// Run `src` with a form's channels attached, and collect what it announced.
fn run_with_form(src: &str) -> (Vec<StateUpdate>, Vec<String>) {
    let program = parse(tokenize(src, SourceFormat::Free))
        .program
        .expect("program should parse");
    let (_event_tx, event_rx) = mpsc::channel();
    let (state_tx, state_rx) = mpsc::channel();
    let (display_tx, display_rx) = mpsc::channel();
    let mut interp = Interpreter::new_with_channels(program, event_rx, state_tx, display_tx);
    interp.run().expect("run failed");
    drop(interp);
    (state_rx.into_iter().collect(), display_rx.into_iter().collect())
}

/// …and without a form, which is what a console program is: the state channel
/// is what makes a form a form, so an interpreter built without one has no
/// Viewer to print into. Its FILE STATUS is read from the environment, since
/// there is no display channel either.
fn run_console_status(src: &str, item: &str) -> String {
    let program = parse(tokenize(src, SourceFormat::Free))
        .program
        .expect("program should parse");
    let mut interp = Interpreter::new(program);
    interp.run().expect("run failed");
    interp.env.get_string(item).unwrap_or_default().trim().to_owned()
}

fn source_of(updates: &[StateUpdate], prop: &str) -> Option<String> {
    updates
        .iter()
        .rev()
        .find(|u| u.prop.eq_ignore_ascii_case(prop))
        .map(|u| u.value.clone())
}

/// A report program: `select` clause, `body` statements.
fn program(select: &str, body: &str) -> String {
    format!(
        "IDENTIFICATION DIVISION.\n\
         PROGRAM-ID. REPORTER.\n\
         ENVIRONMENT DIVISION.\n\
         INPUT-OUTPUT SECTION.\n\
         FILE-CONTROL.\n\
         {select}\n\
         DATA DIVISION.\n\
         FILE SECTION.\n\
         FD  REPORT-FILE.\n\
         01  REPORT-LINE PIC X(40).\n\
         WORKING-STORAGE SECTION.\n\
         01  WS-STATUS PIC XX.\n\
         01  WS-I PIC 9(4).\n\
         PROCEDURE DIVISION.\n\
         MAIN-PARA.\n\
         {body}\n\
         STOP RUN.\n"
    )
}

const MD: &str = "SELECT REPORT-FILE ASSIGN TO VIEWER \"VWR-1\" ORGANIZATION IS MARKDOWN.";
const SEQ: &str = "SELECT REPORT-FILE ASSIGN TO VIEWER \"VWR-1\" ORGANIZATION IS SEQUENTIAL.";

#[test]
fn a_report_is_written_to_a_file_beside_the_form() {
    let (updates, _) = run_with_form(&program(
        MD,
        "OPEN OUTPUT REPORT-FILE\n\
         MOVE \"# Sales\" TO REPORT-LINE\n\
         WRITE REPORT-LINE\n\
         PERFORM VARYING WS-I FROM 1 BY 1 UNTIL WS-I > 39\n\
         MOVE \"a line with trailing spaces\" TO REPORT-LINE\n\
         WRITE REPORT-LINE\n\
         END-PERFORM\n\
         CLOSE REPORT-FILE",
    ));
    let path = source_of(&updates, "View1Source").expect("CLOSE must announce the source");
    println!("  report → {path}");
    assert!(path.ends_with(".md"), "a Markdown report is named .md: {path}");
    let text = std::fs::read_to_string(&path).expect("the report is a real file");
    let lines: Vec<&str> = text.lines().collect();
    println!("  {} lines, first {:?}", lines.len(), lines[0]);
    assert_eq!(lines.len(), 40, "40 records, 40 lines");
    assert_eq!(lines[0], "# Sales");
    assert!(
        !lines[1].ends_with(' '),
        "trailing spaces are not part of the report: {:?}",
        lines[1]
    );
    let _ = std::fs::remove_file(&path);
}

#[test]
fn a_closed_report_reaches_the_viewer_and_an_open_one_does_not() {
    // A report is announced ONCE, by the close — not per record. Forty writes
    // and one hand-over is what "it appears when it is finished" means here;
    // the ordering inside the close (flush, drop, then announce) is what keeps
    // a session from indexing half a document.
    let (many_writes, _) = run_with_form(&program(
        MD,
        "OPEN OUTPUT REPORT-FILE\n\
         PERFORM VARYING WS-I FROM 1 BY 1 UNTIL WS-I > 40\n\
         MOVE \"a line\" TO REPORT-LINE\n\
         WRITE REPORT-LINE\n\
         END-PERFORM\n\
         CLOSE REPORT-FILE",
    ));
    let announced = many_writes
        .iter()
        .filter(|u| u.prop.eq_ignore_ascii_case("View1Source"))
        .count();
    println!("  40 writes → {announced} hand-over(s)");
    assert_eq!(announced, 1, "one report, one hand-over");
    if let Some(p) = source_of(&many_writes, "View1Source") {
        let _ = std::fs::remove_file(&p);
    }

    // …and an explicit CLOSE announces it.
    let (closed, _) = run_with_form(&program(
        MD,
        "OPEN OUTPUT REPORT-FILE\n\
         MOVE \"# Done\" TO REPORT-LINE\n\
         WRITE REPORT-LINE\n\
         CLOSE REPORT-FILE",
    ));
    let path = source_of(&closed, "View1Source").expect("CLOSE announces it");
    println!("  on close:  {path}");
    // Nothing but the source: the developer's Layout, Zoom and split are theirs.
    for u in &closed {
        assert!(
            u.prop.eq_ignore_ascii_case("View1Source"),
            "a report must not touch {}",
            u.prop
        );
    }
    let _ = std::fs::remove_file(&path);
}

#[test]
fn a_second_report_replaces_the_first_and_leaves_its_file_alone() {
    let (updates, _) = run_with_form(&program(
        MD,
        "OPEN OUTPUT REPORT-FILE\n\
         MOVE \"# First\" TO REPORT-LINE\n\
         WRITE REPORT-LINE\n\
         CLOSE REPORT-FILE\n\
         OPEN OUTPUT REPORT-FILE\n\
         MOVE \"# Second\" TO REPORT-LINE\n\
         WRITE REPORT-LINE\n\
         CLOSE REPORT-FILE",
    ));
    let sources: Vec<&StateUpdate> = updates
        .iter()
        .filter(|u| u.prop.eq_ignore_ascii_case("View1Source"))
        .collect();
    assert_eq!(sources.len(), 2, "two reports, two hand-overs");
    let first = sources[0].value.clone();
    let second = sources[1].value.clone();
    println!("  first  {first}\n  second {second}");
    assert_ne!(first, second, "each report gets its own file");
    assert!(
        std::path::Path::new(&first).exists(),
        "the first report's file is still there — the OS owns the temp directory"
    );
    assert_eq!(std::fs::read_to_string(&second).unwrap().trim(), "# Second");
    let _ = std::fs::remove_file(&first);
    let _ = std::fs::remove_file(&second);
}

#[test]
fn an_empty_report_shows_an_empty_document() {
    let (updates, _) = run_with_form(&program(MD, "OPEN OUTPUT REPORT-FILE\nCLOSE REPORT-FILE"));
    let path = source_of(&updates, "View1Source").expect("an empty report is still a report");
    println!("  empty → {path}");
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "");
    let _ = std::fs::remove_file(&path);
}

#[test]
fn a_report_cannot_be_read_and_says_so() {
    let (updates, display) = run_with_form(&program(
        "SELECT REPORT-FILE ASSIGN TO VIEWER \"VWR-1\" ORGANIZATION IS SEQUENTIAL\n\
         FILE STATUS IS WS-STATUS.",
        "OPEN INPUT REPORT-FILE\n\
         DISPLAY \"status \" WS-STATUS",
    ));
    let said = display.join(" ");
    println!("  {said}");
    assert!(said.contains("37"), "OPEN INPUT of a report is refused: {said}");
    assert!(source_of(&updates, "View1Source").is_none());
}

#[test]
fn a_report_needs_a_form_and_a_console_program_is_told() {
    let status = run_console_status(
        &program(
            "SELECT REPORT-FILE ASSIGN TO VIEWER \"VWR-1\" ORGANIZATION IS SEQUENTIAL\n\
             FILE STATUS IS WS-STATUS.",
            "OPEN OUTPUT REPORT-FILE",
        ),
        "WS-STATUS",
    );
    println!("  console status {status:?}");
    assert_eq!(
        status, "93",
        "a console program has no Viewer to print into, and is told so"
    );
}

#[test]
fn a_viewer_with_no_control_id_is_refused() {
    let (_, display) = run_with_form(&program(
        "SELECT REPORT-FILE ASSIGN TO VIEWER\n\
         ORGANIZATION IS SEQUENTIAL\n\
         FILE STATUS IS WS-STATUS.",
        "OPEN OUTPUT REPORT-FILE\n\
         DISPLAY \"status \" WS-STATUS",
    ));
    let said = display.join(" ");
    println!("  {said}");
    assert!(said.contains("31"), "ASSIGN TO VIEWER names no control: {said}");
}

/// **The guard on plan D2.** The vertical movement is emitted for a report and
/// for nothing else: a disk file's bytes are exactly what they were before this
/// feature existed, because `advance_linage` still emits nothing for them.
#[test]
fn advancing_moves_the_page_only_for_a_viewer_report() {
    // The report: one blank line between records, and a form feed for PAGE.
    let (updates, _) = run_with_form(&program(
        SEQ,
        "OPEN OUTPUT REPORT-FILE\n\
         MOVE \"first\" TO REPORT-LINE\n\
         WRITE REPORT-LINE\n\
         MOVE \"double spaced\" TO REPORT-LINE\n\
         WRITE REPORT-LINE AFTER ADVANCING 2 LINES\n\
         MOVE \"new page\" TO REPORT-LINE\n\
         WRITE REPORT-LINE AFTER ADVANCING PAGE\n\
         CLOSE REPORT-FILE",
    ));
    let path = source_of(&updates, "View1Source").expect("a source");
    let report = std::fs::read_to_string(&path).expect("read the report");
    println!("  report bytes {:?}", report);
    assert_eq!(
        report, "first\n\ndouble spaced\n\u{000C}new page\n",
        "a blank line for ADVANCING 2, a form feed for PAGE"
    );
    let _ = std::fs::remove_file(&path);

    // The disk file, same program: untouched by any of it.
    let disk = std::env::temp_dir().join(format!("prc-062-disk-{}.txt", std::process::id()));
    let _ = std::fs::remove_file(&disk);
    let src = program(
        &format!(
            "SELECT REPORT-FILE ASSIGN TO \"{}\" ORGANIZATION IS LINE SEQUENTIAL.",
            disk.display()
        ),
        "OPEN OUTPUT REPORT-FILE\n\
         MOVE \"first\" TO REPORT-LINE\n\
         WRITE REPORT-LINE\n\
         MOVE \"double spaced\" TO REPORT-LINE\n\
         WRITE REPORT-LINE AFTER ADVANCING 2 LINES\n\
         MOVE \"new page\" TO REPORT-LINE\n\
         WRITE REPORT-LINE AFTER ADVANCING PAGE\n\
         CLOSE REPORT-FILE",
    );
    let _ = run_with_form(&src);
    let on_disk = std::fs::read_to_string(&disk).expect("read the disk file");
    println!("  disk bytes   {:?}", on_disk);
    assert_eq!(
        on_disk, "first\ndouble spaced\nnew page\n",
        "a disk file's bytes must be what they have always been — no blank \
         line, no form feed (plan D2)"
    );
    let _ = std::fs::remove_file(&disk);
}

/// **STOP RUN closes every open file**, and closing a report is what shows it.
/// A program that ends without `CLOSE` used to leave the document written and
/// never displayed — the sink flushed when it was dropped and nobody was told
/// where it went.
#[test]
fn a_report_left_open_is_closed_at_stop_run() {
    let (updates, _) = run_with_form(&program(
        MD,
        "OPEN OUTPUT REPORT-FILE\n\
         MOVE \"# Never closed\" TO REPORT-LINE\n\
         WRITE REPORT-LINE",
    ));
    let path = source_of(&updates, "View1Source")
        .expect("STOP RUN closes the report, and a closed report is shown");
    println!("  left open → {path}");
    assert_eq!(
        std::fs::read_to_string(&path).unwrap().trim(),
        "# Never closed"
    );
    let _ = std::fs::remove_file(&path);
}
