// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Tests for DATA DIVISION parsing.

use cobolt_ast::data::{PicKind, Usage};
use cobolt_ast::program::DataSection;
use cobolt_lexer::{tokenize, SourceFormat};
use cobolt_parser::parse;

fn with_data(data_src: &str) -> String {
    format!(
        "IDENTIFICATION DIVISION.\nPROGRAM-ID. TESTPROG.\n{}\nPROCEDURE DIVISION.\nMAIN.\n    STOP RUN.\n",
        data_src
    )
}

fn parse_data(data_src: &str) -> cobolt_ast::program::DataDivision {
    let code = with_data(data_src);
    let result = parse(tokenize(&code, SourceFormat::Free));
    assert!(result.diagnostics.is_empty(), "Diagnostics: {:?}", result.diagnostics);
    result.program.unwrap().data.expect("expected DATA DIVISION")
}

// ── Working-storage basics ────────────────────────────────────────────────────

#[test]
fn working_storage_simple() {
    let data = parse_data(
        "DATA DIVISION.\nWORKING-STORAGE SECTION.\n01 WS-NAME PIC X(30).\n",
    );
    assert_eq!(data.sections.len(), 1);
    if let DataSection::WorkingStorage(items) = &data.sections[0] {
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].name.as_deref(), Some("WS-NAME"));
        assert_eq!(items[0].level, 1);
        let pic = items[0].picture.as_ref().expect("no PIC");
        assert_eq!(pic.kind, PicKind::Alphanumeric);
    } else {
        panic!("expected WORKING-STORAGE section");
    }
}

#[test]
fn working_storage_numeric() {
    let data = parse_data(
        "DATA DIVISION.\nWORKING-STORAGE SECTION.\n01 WS-AMT PIC 9(7)V99.\n",
    );
    if let DataSection::WorkingStorage(items) = &data.sections[0] {
        let pic = items[0].picture.as_ref().unwrap();
        assert_eq!(pic.kind, PicKind::Numeric);
        assert_eq!(items[0].usage, Usage::Display);
    } else {
        panic!("expected WORKING-STORAGE");
    }
}

#[test]
fn usage_comp3() {
    let data = parse_data(
        "DATA DIVISION.\nWORKING-STORAGE SECTION.\n01 WS-AMT PIC S9(7)V99 COMP-3.\n",
    );
    if let DataSection::WorkingStorage(items) = &data.sections[0] {
        assert_eq!(items[0].usage, Usage::Comp3);
    } else {
        panic!("expected WORKING-STORAGE");
    }
}

#[test]
fn filler_item() {
    let data = parse_data(
        "DATA DIVISION.\nWORKING-STORAGE SECTION.\n01 FILLER PIC X(10).\n",
    );
    if let DataSection::WorkingStorage(items) = &data.sections[0] {
        assert!(items[0].name.is_none(), "FILLER should have no name");
    } else {
        panic!("expected WORKING-STORAGE");
    }
}

#[test]
fn group_item_tree() {
    let data = parse_data(
        "DATA DIVISION.\nWORKING-STORAGE SECTION.\n\
         01 WS-RECORD.\n   05 WS-NAME PIC X(20).\n   05 WS-AGE  PIC 9(3).\n",
    );
    if let DataSection::WorkingStorage(roots) = &data.sections[0] {
        assert_eq!(roots.len(), 1, "expected one root item");
        let root = &roots[0];
        assert_eq!(root.name.as_deref(), Some("WS-RECORD"));
        assert_eq!(root.children.len(), 2, "expected two children");
        assert_eq!(root.children[0].name.as_deref(), Some("WS-NAME"));
        assert_eq!(root.children[1].name.as_deref(), Some("WS-AGE"));
    } else {
        panic!("expected WORKING-STORAGE");
    }
}

#[test]
fn value_clause() {
    let data = parse_data(
        "DATA DIVISION.\nWORKING-STORAGE SECTION.\n01 WS-FLAG PIC X VALUE 'N'.\n",
    );
    if let DataSection::WorkingStorage(items) = &data.sections[0] {
        assert!(items[0].value.is_some(), "expected VALUE clause");
    } else {
        panic!("expected WORKING-STORAGE");
    }
}

#[test]
fn occurs_clause() {
    let data = parse_data(
        "DATA DIVISION.\nWORKING-STORAGE SECTION.\n01 WS-TABLE PIC 9(3) OCCURS 10 TIMES.\n",
    );
    if let DataSection::WorkingStorage(items) = &data.sections[0] {
        let occ = items[0].occurs.as_ref().expect("expected OCCURS");
        assert_eq!(occ.max, 10);
    } else {
        panic!("expected WORKING-STORAGE");
    }
}

#[test]
fn redefines_clause() {
    let data = parse_data(
        "DATA DIVISION.\nWORKING-STORAGE SECTION.\n\
         01 WS-DATA PIC X(4).\n01 WS-NUM REDEFINES WS-DATA PIC 9(4).\n",
    );
    if let DataSection::WorkingStorage(items) = &data.sections[0] {
        assert_eq!(items[1].redefines.as_deref(), Some("WS-DATA"));
    } else {
        panic!("expected WORKING-STORAGE");
    }
}

#[test]
fn multiple_sections() {
    let data = parse_data(
        "DATA DIVISION.\n\
         WORKING-STORAGE SECTION.\n01 WS-A PIC X.\n\
         LINKAGE SECTION.\n01 LK-B PIC 9.\n",
    );
    assert_eq!(data.sections.len(), 2);
    assert!(matches!(data.sections[0], DataSection::WorkingStorage(_)));
    assert!(matches!(data.sections[1], DataSection::Linkage(_)));
}

#[test]
fn level_77_item() {
    let data = parse_data(
        "DATA DIVISION.\nWORKING-STORAGE SECTION.\n77 WS-SWITCH PIC X.\n",
    );
    if let DataSection::WorkingStorage(items) = &data.sections[0] {
        assert_eq!(items[0].level, 77);
    } else {
        panic!("expected WORKING-STORAGE");
    }
}
