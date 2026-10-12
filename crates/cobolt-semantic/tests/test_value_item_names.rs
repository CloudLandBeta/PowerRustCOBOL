// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! `value_item_names` is the analyser's own answer to "is this name an ordinary
//! data item" — the receiver of a value method (`WS-TEXT::Trim()`). The editor
//! asks it before rewriting a method spelling, so it must say yes only where the
//! analyser's "a data item has no method …" rule says it too.

use std::collections::HashSet;

use cobolt_lexer::{tokenize, SourceFormat};
use cobolt_parser::parse;
use cobolt_semantic::{analyze_with, value_item_names, AnalyzeOptions, Severity};

const UNIT: &str = "\
IDENTIFICATION DIVISION.
PROGRAM-ID. OUTER.
ENVIRONMENT DIVISION.
CONFIGURATION SECTION.
REPOSITORY.
    CLASS RUST-STRING IS \"Rust.String\"
DATA DIVISION.
WORKING-STORAGE SECTION.
01 WS-TEXT PIC X(40) GLOBAL.
01 WS-REC GLOBAL.
   05 WS-REC-NAME PIC X(10).
   05 WS-ROW OCCURS 3 TIMES PIC X(5).
01 WS-OBJ USAGE IS OBJECT REFERENCE RUST-STRING.
01 WS-PTR USAGE IS POINTER.
01 BTN-OK PIC X(10).
01 BOTH-WAYS PIC X(4).
PROCEDURE DIVISION.
MAIN.
    STOP RUN.
IDENTIFICATION DIVISION.
PROGRAM-ID. INNER.
DATA DIVISION.
WORKING-STORAGE SECTION.
01 WS-LOCAL PIC X(8).
01 BOTH-WAYS USAGE IS OBJECT REFERENCE RUST-STRING.
PROCEDURE DIVISION.
RUN-IT.
    MOVE WS-TEXT::Trim() TO WS-LOCAL.
    EXIT PROGRAM.
END PROGRAM INNER.
END PROGRAM OUTER.
";

fn names(objects: Option<&HashSet<String>>) -> HashSet<String> {
    let program = parse(tokenize(UNIT, SourceFormat::Free)).program.expect("the unit parses");
    value_item_names(&program, objects)
}

#[test]
fn ordinary_items_are_value_items_wherever_the_unit_declares_them() {
    let n = names(None);
    for want in ["WS-TEXT", "WS-REC", "WS-REC-NAME", "WS-ROW", "WS-LOCAL", "BTN-OK"] {
        assert!(n.contains(want), "{want} is an ordinary data item: {n:?}");
    }
}

#[test]
fn a_handle_a_pointer_and_an_ambiguous_name_are_not() {
    let n = names(None);
    for not in ["WS-OBJ", "WS-PTR", "NOT-DECLARED", "ME", "SUPER", "COBOL"] {
        assert!(!n.contains(not), "{not} must not be a value item: {n:?}");
    }
    assert!(!n.contains("BOTH-WAYS"), "declared as a value in one program and as a handle in another: {n:?}");
}

#[test]
fn a_name_the_forms_objects_share_is_the_control() {
    let controls: HashSet<String> = ["BTN-OK".to_string()].into();
    let n = names(Some(&controls));
    assert!(!n.contains("BTN-OK"), "the control wins, as in check_receiver: {n:?}");
    assert!(n.contains("WS-TEXT"));
}

#[test]
fn it_agrees_with_the_analysers_own_data_item_method_rule() {
    // For each name: does the analyser refuse `NAME::Foo()` (a data item has no such method)?
    let program = parse(tokenize(UNIT, SourceFormat::Free)).program.expect("parses");
    let mut agreed = 0;
    for name in ["WS-TEXT", "WS-REC", "WS-ROW", "WS-OBJ", "WS-PTR", "BTN-OK"] {
        let src = UNIT.replace("MOVE WS-TEXT::Trim() TO WS-LOCAL.", &format!("MOVE {name}::Foo() TO WS-LOCAL."));
        let p = parse(tokenize(&src, SourceFormat::Free)).program.expect("parses");
        let refused = analyze_with(&p, &AnalyzeOptions::default())
            .diagnostics
            .iter()
            .any(|d| d.severity == Severity::Error && d.message.contains("a data item has no method or property"));
        let is_value = value_item_names(&program, None).contains(name);
        // WS-ROW/WS-REC/WS-TEXT are GLOBAL or inherited in INNER; the rest are visible only
        // where declared, so the analyser (which judges INNER's text) cannot see them.
        if matches!(name, "WS-TEXT" | "WS-REC" | "WS-ROW") {
            assert_eq!(refused, is_value, "{name}");
            agreed += 1;
        }
    }
    assert_eq!(agreed, 3);
    println!("value_item_names agrees with the analyser's data-item method rule on {agreed} names");
}
