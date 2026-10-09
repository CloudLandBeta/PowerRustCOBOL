// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! A DATA ITEM answers a closed set of value methods (Trim, UpperCase …, Replace,
//! Len, Length, Split — `cobolt_ast::methods::DATA_ITEM_METHODS`). Any other
//! name after a data item used to evaluate to an empty string and let the
//! program carry on with it (operator, 2026-10-09: "make unknown methods on
//! data items raise an error"); the analyser now refuses it, naming the item
//! and what it does answer.

use std::collections::HashSet;

use cobolt_lexer::{tokenize, SourceFormat};
use cobolt_parser::parse;
use cobolt_semantic::{analyze, analyze_with, AnalyzeOptions, Severity};

fn program_with(procedure: &str) -> cobolt_ast::program::Program {
    let src = format!(
        "\
IDENTIFICATION DIVISION.
PROGRAM-ID. DIMTEST.
ENVIRONMENT DIVISION.
CONFIGURATION SECTION.
REPOSITORY.
    CLASS RUST-STRING IS \"Rust.String\"
DATA DIVISION.
WORKING-STORAGE SECTION.
01 WS-TEXT PIC X(40).
01 WS-OUT  PIC X(40).
01 WS-N    PIC 9(4).
01 WS-TAB.
   05 WS-ROW OCCURS 3 TIMES PIC X(10).
01 WS-OBJ USAGE IS OBJECT REFERENCE RUST-STRING.
PROCEDURE DIVISION.
MAIN.
{procedure}
    STOP RUN.
"
    );
    parse(tokenize(&src, SourceFormat::Free)).program.expect("program should parse")
}

fn errors_of(procedure: &str, objects: Option<HashSet<String>>) -> Vec<String> {
    analyze_with(
        &program_with(procedure),
        &AnalyzeOptions { known_objects: objects, ..Default::default() },
    )
    .diagnostics
    .into_iter()
    .filter(|d| d.severity == Severity::Error)
    .map(|d| d.message)
    .collect()
}

fn errors(procedure: &str) -> Vec<String> {
    errors_of(procedure, None)
}

#[test]
fn a_method_a_data_item_does_not_have_is_an_error() {
    let mut shown = Vec::new();
    for (code, method) in [
        ("    DISPLAY WS-TEXT::Contains(\"x\").", "Contains"),
        ("    MOVE WS-TEXT::Reverse() TO WS-OUT.", "Reverse"),
        ("    MOVE WS-TEXT::Value TO WS-OUT.", "Value"),
        ("    MOVE WS-TEXT::Text TO WS-OUT.", "Text"),
        ("    COMPUTE WS-N = WS-TEXT::Size.", "Size"),
        ("    MOVE WS-TEXT::Trim()::Foo() TO WS-OUT.", "Foo"),
        ("    MOVE WS-ROW(2)::Bar() TO WS-OUT.", "Bar"),
        ("    MOVE WS-TEXT(1:3)::Baz() TO WS-OUT.", "Baz"),
        ("    IF WS-TEXT::StartsWith(\"a\") = \"y\" DISPLAY \"x\" END-IF.", "StartsWith"),
        ("    INVOKE WS-TEXT::Show().", "Show"),
    ] {
        let e = errors(code);
        assert_eq!(e.len(), 1, "{code}: {e:?}");
        // The lexer upper-cases names, so the message quotes them as the AST has them.
        assert!(
            e[0].to_ascii_uppercase().contains(&format!("NO METHOD OR PROPERTY '{}'", method.to_ascii_uppercase())),
            "{code}: {}",
            e[0]
        );
        assert!(e[0].contains("Trim, UpperCase"), "the message says what a data item does answer: {}", e[0]);
        shown.push(e[0].clone());
    }
    println!("data-item method check — {} unknown names refused, e.g.:\n  {}", shown.len(), shown[0]);
}

#[test]
fn every_value_method_a_data_item_has_passes_in_every_spelling() {
    let code = "\
    MOVE WS-TEXT::Trim() TO WS-OUT.
    MOVE WS-TEXT::UpperCase() TO WS-OUT.
    MOVE WS-TEXT::ToUpperCase() TO WS-OUT.
    MOVE WS-TEXT::Upper() TO WS-OUT.
    MOVE WS-TEXT::LowerCase() TO WS-OUT.
    MOVE WS-TEXT::ToLowerCase() TO WS-OUT.
    MOVE WS-TEXT::Lower() TO WS-OUT.
    MOVE WS-TEXT::Replace(\"a\" \"b\") TO WS-OUT.
    COMPUTE WS-N = WS-TEXT::Len().
    COMPUTE WS-N = WS-TEXT::Length().
    COMPUTE WS-N = WS-TEXT::Length.
    MOVE WS-TEXT::Split(\"-\") TO WS-OUT.
    MOVE WS-TEXT::Split(\"-\")(2) TO WS-OUT.
    MOVE WS-TEXT::Trim()::Len() TO WS-N.
    MOVE WS-TEXT::TRIM() TO WS-OUT.
    MOVE WS-ROW(2)::Upper() TO WS-OUT.
    MOVE WS-TEXT(1:3)::Lower() TO WS-OUT.
    INVOKE WS-TEXT::UpperCase().";
    assert_eq!(errors(code), Vec::<String>::new());
}

#[test]
fn the_shared_list_matches_what_the_analyser_accepts() {
    for m in cobolt_ast::methods::DATA_ITEM_METHODS {
        let e = errors(&format!("    MOVE WS-TEXT::{m}() TO WS-OUT."));
        assert!(e.is_empty(), "{m}: {e:?}");
    }
}

#[test]
fn a_control_an_object_reference_or_a_name_the_form_owns_is_not_touched() {
    // A control chain: the control's own member.
    let controls: HashSet<String> = ["BTN-SAVE", "GRD-ROWS", "WS-TEXT"].iter().map(|s| s.to_string()).collect();
    let code = "\
    INVOKE BTN-SAVE::Disable().
    MOVE GRD-ROWS::Rows(1)::Value TO WS-OUT.
    MOVE GRD-ROWS::Rows(1)::Columns(2)::Value::toUpperCase() TO WS-OUT.";
    assert_eq!(errors_of(code, Some(controls.clone())), Vec::<String>::new());
    // The handle of an OBJECT REFERENCE answers the methods of its own class.
    assert_eq!(errors("    INVOKE WS-OBJ::Push(\"x\")."), Vec::<String>::new());
    assert_eq!(errors("    MOVE WS-OBJ::Len() TO WS-N."), Vec::<String>::new());
    // A name that is both a data item and a control of the form is the control
    // (the same rule `check_receiver` follows).
    assert_eq!(errors_of("    MOVE WS-TEXT::Caption TO WS-OUT.", Some(controls)), Vec::<String>::new());
    // me / super / COBOL are not data items.
    assert_eq!(errors("    MOVE me::Width TO WS-N."), Vec::<String>::new());
}

#[test]
fn without_a_data_item_of_that_name_the_old_rules_stand() {
    let r = analyze(&program_with("    MOVE NOT-DECLARED::Contains(\"x\") TO WS-OUT."));
    assert!(
        !r.errors().any(|d| d.message.contains("no method or property")),
        "only a declared data item is judged by this rule: {:?}",
        r.diagnostics
    );
}
