// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! A `CALL "NAME"` must name something that exists (operator report,
//! 2026-10-08: "a CALL to a procedure that does not exist, the compiler does
//! not warn").
//!
//! The runtime resolves a literal target against the program-names of the
//! unit, the paragraphs and sections of its programs, and the built-in
//! `COBOL-…` calls; anything else is skipped with a log line and nothing else
//! — the handler carries on as if the call had been made. With the project's
//! Common Code names supplied (`AnalyzeOptions::known_programs`), a literal
//! target that none of those answers is an error, unless the `CALL` names
//! `ON EXCEPTION`, which is how a program says the target is optional. A
//! target held in a data item cannot be known, and without context
//! (`known_programs: None`, a lone file read by something that cannot see its
//! project) nothing is checked.

use std::collections::HashSet;

use cobolt_lexer::{tokenize, SourceFormat};
use cobolt_parser::parse;
use cobolt_semantic::{analyze_with, AnalyzeOptions, Severity};

/// One program, with the paragraph `HELPER` beside `MAIN`, whose `body` holds
/// the statements under test.
fn flat(body: &str) -> String {
    format!(
        "\
IDENTIFICATION DIVISION.
PROGRAM-ID. CALLTEST.
DATA DIVISION.
WORKING-STORAGE SECTION.
01 WS-PROG PIC X(20).
PROCEDURE DIVISION.
MAIN.
{body}
    STOP RUN.
HELPER.
    DISPLAY \"helper\".
"
    )
}

/// A form-shaped unit: an outer program that contains `UPDATE-RECEIPT`, the
/// way a form holds its handlers and its own procedures.
fn nested(body: &str) -> String {
    format!(
        "\
       IDENTIFICATION DIVISION.
       PROGRAM-ID. OUTER-FORM.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 WS-PROG PIC X(20).
       PROCEDURE DIVISION.
       MAIN SECTION.
{body}
           CONTINUE.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. UPDATE-RECEIPT.
       DATA DIVISION.
       PROCEDURE DIVISION.
       MAIN SECTION.
           CALL \"INNER-WORK\"
           EXIT PROGRAM.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. INNER-WORK.
       DATA DIVISION.
       PROCEDURE DIVISION.
       MAIN SECTION.
           EXIT PROGRAM.
       END PROGRAM INNER-WORK.
       END PROGRAM UPDATE-RECEIPT.
       END PROGRAM OUTER-FORM.
"
    )
}

fn known(names: &[&str]) -> Option<HashSet<String>> {
    Some(names.iter().map(|s| s.to_string()).collect())
}

fn diagnostics(src: &str, known_programs: Option<HashSet<String>>) -> Vec<(Severity, usize, String)> {
    let parsed = parse(tokenize(src, SourceFormat::Free));
    assert!(!parsed.has_errors(), "the source must parse cleanly: {:?}", parsed.diagnostics);
    let program = parsed.program.expect("program should parse");
    analyze_with(&program, &AnalyzeOptions { known_programs, ..Default::default() })
        .diagnostics
        .into_iter()
        .filter(|d| d.message.contains("CALL"))
        .map(|d| (d.severity, d.span.line as usize, d.message))
        .collect()
}

fn errors(src: &str, known_programs: Option<HashSet<String>>) -> Vec<(usize, String)> {
    diagnostics(src, known_programs)
        .into_iter()
        .filter(|(s, _, _)| *s == Severity::Error)
        .map(|(_, line, m)| (line, m))
        .collect()
}

#[test]
fn a_literal_target_that_does_not_exist_is_an_error() {
    let src = flat("    CALL \"NO-SUCH-PROC\" USING WS-PROG");
    let e = errors(&src, known(&[]));
    assert_eq!(e.len(), 1, "one error expected: {e:?}");
    assert!(e[0].1.contains("NO-SUCH-PROC"), "names the target: {}", e[0].1);
    let want = src.lines().position(|l| l.contains("NO-SUCH-PROC")).unwrap() + 1;
    assert_eq!(e[0].0, want, "points at the CALL's own line");
    println!("unknown target → line {}: {}", e[0].0, e[0].1);
}

#[test]
fn a_typo_in_a_real_target_is_an_error_whatever_the_letter_case() {
    // The unit holds INNER-WORK; the CALL says INNER-WROK.
    let e = errors(&nested("           CALL \"inner-wrok\""), known(&[]));
    assert_eq!(e.len(), 1, "{e:?}");
    assert!(e[0].1.to_ascii_uppercase().contains("INNER-WROK"), "{}", e[0].1);
}

#[test]
fn a_program_the_unit_contains_is_found_at_any_depth_and_in_any_case() {
    // UPDATE-RECEIPT is contained in OUTER-FORM; INNER-WORK is called from it.
    let src = nested("           CALL \"update-receipt\"\n           CALL \"INNER-WORK\"");
    assert_eq!(errors(&src, known(&[])), vec![], "contained programs resolve");
}

#[test]
fn a_paragraph_or_a_section_of_the_unit_is_found() {
    let src = flat("    CALL \"HELPER\"\n    CALL \"main\"");
    assert_eq!(errors(&src, known(&[])), vec![], "a paragraph resolves, in any case");
    // The section name of the nested sample.
    let src = nested("           CALL \"MAIN\"");
    assert_eq!(errors(&src, known(&[])), vec![], "a section resolves");
}

#[test]
fn the_projects_common_code_is_found() {
    let src = flat("    CALL \"common-proc\" USING WS-PROG");
    assert_eq!(errors(&src, known(&["COMMON-PROC"])), vec![], "a Common Code name resolves");
    // …and the same call is an error when the project does not have it.
    assert_eq!(errors(&src, known(&["OTHER-PROC"])).len(), 1);
}

#[test]
fn a_call_that_names_on_exception_says_the_target_is_optional() {
    let src = flat(
        "    CALL \"OPTIONAL-PROG\"\n        ON EXCEPTION DISPLAY \"absent\"\n    END-CALL",
    );
    assert_eq!(errors(&src, known(&[])), vec![], "ON EXCEPTION handles it");
}

#[test]
fn a_target_held_in_a_data_item_cannot_be_known_and_is_not_checked() {
    let src = flat("    MOVE \"WHATEVER\" TO WS-PROG\n    CALL WS-PROG");
    assert_eq!(errors(&src, known(&[])), vec![]);
}

#[test]
fn the_builtin_namespace_is_not_checked_here() {
    // The built-ins live in the runtime, which this crate cannot see; the
    // runtime has no prefix dispatch, so the names are exact, but only the
    // runtime's own table can say which exist.
    let src = flat("    CALL \"COBOL-OPEN-DB\" USING WS-PROG\n    CALL \"COBOLT-INIT-FORM\"");
    assert_eq!(errors(&src, known(&[])), vec![]);
}

#[test]
fn a_call_nested_in_other_statements_and_in_a_contained_program_is_found() {
    let src = flat("    IF WS-PROG = SPACES\n        PERFORM HELPER\n        CALL \"HIDDEN-IN-IF\"\n    END-IF");
    let e = errors(&src, known(&[]));
    assert_eq!(e.len(), 1, "{e:?}");
    assert!(e[0].1.contains("HIDDEN-IN-IF"));

    // …and one inside a contained program.
    let src = nested("           CONTINUE").replace("CALL \"INNER-WORK\"", "CALL \"LOST-IN-NEST\"");
    let e = errors(&src, known(&[]));
    assert_eq!(e.len(), 1, "{e:?}");
    assert!(e[0].1.contains("LOST-IN-NEST"));
}

#[test]
fn without_context_nothing_is_checked() {
    // `None`: a caller that cannot see the project — a lone file, a lint that
    // has not read the manifest — must not raise what it cannot know.
    let src = flat("    CALL \"NO-SUCH-PROC\"");
    assert_eq!(diagnostics(&src, None), vec![]);
}

#[test]
fn the_cases_in_one_run() {
    // One line per case, so a regression names itself.
    let cases: [(&str, String, Option<HashSet<String>>, usize); 6] = [
        ("unknown", flat("    CALL \"NO-SUCH\""), known(&[]), 1),
        ("paragraph", flat("    CALL \"HELPER\""), known(&[]), 0),
        ("common code", flat("    CALL \"CC-PROC\""), known(&["CC-PROC"]), 0),
        ("contained", nested("           CALL \"UPDATE-RECEIPT\""), known(&[]), 0),
        ("on exception", flat("    CALL \"MAYBE\" ON EXCEPTION CONTINUE END-CALL"), known(&[]), 0),
        ("no context", flat("    CALL \"NO-SUCH\""), None, 0),
    ];
    let mut passed = 0;
    for (label, src, ctx, want) in &cases {
        let got = errors(src, ctx.clone()).len();
        println!("  {label:<12} expected {want} error(s), got {got}");
        assert_eq!(got, *want, "{label}");
        passed += 1;
    }
    println!("CALL-target check: {passed}/{} cases as expected", cases.len());
}
