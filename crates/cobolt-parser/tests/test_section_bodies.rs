// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Tests: sentences written straight under a SECTION header.
//!
//! COBOL-85 lets a section header be followed directly by sentences, with no
//! paragraph-name in between, and the project's COBOL code standard *requires*
//! that shape for handlers — `MAIN SECTION.` followed by its `PERFORM`s, then
//! `INITIALIZE-… SECTION.` followed by its `MOVE`s.
//!
//! The section parser only ever collected `Identifier Period` paragraph
//! headers. Anything else ended the collection and fell back to the caller,
//! which was looking for a section name and reported "expected section name,
//! found MOVE" before skipping to the next period. The section survived with an
//! EMPTY paragraph list, so the body was gone before anything downstream saw
//! it: no resolution, no PERFORM-target check, and nothing to execute. Every
//! handler written to the standard would have parsed to a program that does
//! nothing.

use cobolt_ast::program::ProcedureBody;
use cobolt_lexer::{tokenize, SourceFormat};
use cobolt_parser::{parse, Severity};

fn parse_ok(src: &str) -> cobolt_ast::program::Program {
    let result = parse(tokenize(src, SourceFormat::Free));
    assert!(
        result
            .diagnostics
            .iter()
            .all(|d| d.severity != Severity::Error),
        "unexpected parse errors: {:?}",
        result.diagnostics
    );
    result.program.expect("no program returned")
}

fn sections(program: &cobolt_ast::program::Program) -> Vec<(String, usize)> {
    match &program.procedure.body {
        ProcedureBody::Sections(secs) => secs
            .iter()
            .map(|s| {
                (
                    s.name.clone(),
                    s.paragraphs.iter().map(|p| p.stmts.len()).sum(),
                )
            })
            .collect(),
        ProcedureBody::Paragraphs(_) => panic!("expected a section-structured body"),
    }
}

/// The exact shape the code standard mandates, and the one that used to be
/// discarded: an orchestrating `MAIN SECTION` plus a dedicated initialization
/// section, neither of them using a paragraph-name.
#[test]
fn statements_under_a_section_header_survive_parsing() {
    let src = r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. H.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01  MC-DATA.
           05  ITEM-PRICE  PIC 99V99 COMP OCCURS 3 TIMES.
       PROCEDURE DIVISION.

       MAIN SECTION.

           PERFORM INITIALIZE-MENU-PRICES
           EXIT PROGRAM.

       INITIALIZE-MENU-PRICES SECTION.

           MOVE 7.49 TO ITEM-PRICE (1)
           MOVE 8.90 TO ITEM-PRICE (2)
           MOVE 6.50 TO ITEM-PRICE (3)

           EXIT.
"#;
    let program = parse_ok(src);
    let secs = sections(&program);
    assert_eq!(
        secs.len(),
        2,
        "both sections must be present, got {secs:?}"
    );
    assert_eq!(secs[0].0, "MAIN");
    assert!(
        secs[0].1 >= 2,
        "MAIN's PERFORM and EXIT PROGRAM must survive, got {secs:?}"
    );
    assert_eq!(secs[1].0, "INITIALIZE-MENU-PRICES");
    assert!(
        secs[1].1 >= 4,
        "the three MOVEs and the EXIT must survive, got {secs:?}"
    );
}

/// The unnamed body of each section is labelled after the section that owns it.
/// Two sections both yielding `<implicit>` would be flagged as a duplicate
/// procedure name, and the symbol table — keyed by name — would keep only one.
#[test]
fn each_sections_unnamed_body_gets_a_distinct_label() {
    let src = r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. H.
       PROCEDURE DIVISION.
       MAIN SECTION.
           CONTINUE.
       SECOND-THING SECTION.
           CONTINUE.
       THIRD-THING SECTION.
           CONTINUE.
"#;
    let program = parse_ok(src);
    let names: Vec<String> = match &program.procedure.body {
        ProcedureBody::Sections(secs) => secs
            .iter()
            .flat_map(|s| s.paragraphs.iter().map(|p| p.name.clone()))
            .collect(),
        ProcedureBody::Paragraphs(_) => panic!("expected a section-structured body"),
    };
    assert_eq!(names.len(), 3, "one unnamed body per section: {names:?}");
    let mut unique = names.clone();
    unique.sort();
    unique.dedup();
    assert_eq!(
        unique.len(),
        names.len(),
        "the labels must be distinct, got {names:?}"
    );
}

/// Named paragraphs inside a section still work, and mix with a leading
/// unnamed body — the sentences before the first paragraph-name belong to the
/// section, the rest to their paragraphs.
#[test]
fn a_section_may_hold_both_a_leading_body_and_named_paragraphs() {
    let src = r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. H.
       PROCEDURE DIVISION.
       MAIN SECTION.
           PERFORM CHECK-RANGE.
       CHECK-RANGE.
           CONTINUE.
"#;
    let program = parse_ok(src);
    match &program.procedure.body {
        ProcedureBody::Sections(secs) => {
            assert_eq!(secs.len(), 1, "one section: {:?}", secs.len());
            let names: Vec<&str> = secs[0].paragraphs.iter().map(|p| p.name.as_str()).collect();
            assert!(
                names.contains(&"CHECK-RANGE"),
                "the named paragraph must survive: {names:?}"
            );
            assert!(
                names.iter().any(|n| n.starts_with('<')),
                "the leading unnamed body must survive too: {names:?}"
            );
        }
        ProcedureBody::Paragraphs(_) => panic!("expected a section-structured body"),
    }
}
