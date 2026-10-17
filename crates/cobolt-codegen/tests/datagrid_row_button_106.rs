// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Spec 106 — what the generated program does with a DataGrid's `onRowButtonClick`: a
//! buffer as wide as the row, the handler called `USING` it, and the handler's `COPY
//! DATAGRID-ROW-CONTENT.` replaced by the group from the grid's columns as they are now.

use cobolt_codegen::{generate, generate_with_map};
use cobolt_forms::model::{DataGridAdvanced, DataGridColumn, EventBinding, DATAGRID_ADVANCED_PROP};
use cobolt_forms::{Control, ControlType, Form, PropValue};

fn column(title: &str, mask: &str, kind: &str) -> DataGridColumn {
    DataGridColumn {
        id: title.to_ascii_uppercase().replace(' ', "-"),
        title: title.into(),
        source_name: title.into(),
        cobol_mask: mask.into(),
        edit_control: kind.into(),
        button_caption: if kind == "Button" { "Edit".into() } else { String::new() },
        ..Default::default()
    }
}

/// The developer's handler: the template's lines, then their own statements.
const HANDLER: &str = "       ENVIRONMENT DIVISION.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       LINKAGE SECTION.
       COPY DATAGRID-ROW-CONTENT.

       PROCEDURE DIVISION USING DATAGRID-ROW-CONTENT.
           DISPLAY ROW-NUMBER.
           DISPLAY ACTOR-NAME.";

fn form(columns: Vec<DataGridColumn>, code: &str) -> Form {
    let mut form = Form::new("ACTORS", "Actors", 600, 400);
    let mut grid = Control::new("DG", ControlType::DataGrid, 0, 0);
    let advanced = DataGridAdvanced { columns, ..Default::default() };
    grid.set_prop(DATAGRID_ADVANCED_PROP, PropValue::String(advanced.to_json().unwrap()));
    let mut ev = EventBinding::for_control("DG", "onRowButtonClick");
    ev.code = code.to_owned();
    grid.events.push(ev);
    form.add_control(grid);
    form
}

fn actors(code: &str) -> Form {
    form(
        vec![column("Actor Id", "9(5)", "Textbox"), column("Actor Name", "", "Textbox"), column("Edit", "", "Button")],
        code,
    )
}

#[test]
fn the_row_reaches_the_handler_through_a_buffer_and_the_group() {
    let src = generate(&actors(HANDLER));
    // 9 (ROW-NUMBER) + 30 (ROW-NAME) + 5 (ACTOR-ID) + 30 (ACTOR-NAME); the Button column is
    // not data.
    assert!(src.contains("01 DATAGRID-ROW-BUFFER     PIC X(74)."), "{src}");
    assert!(src.contains("CALL \"DG--ONROWBUTTONCLICK\" USING DATAGRID-ROW-BUFFER"), "{src}");
    assert!(src.contains("10 ACTOR-ID PIC 9(5)."), "{src}");
    assert!(src.contains("10 ACTOR-NAME PIC X(30)."), "{src}");
    assert!(!src.contains("10 EDIT"), "a Button column is an action, not a field");
    assert!(!src.contains("COPY DATAGRID-ROW-CONTENT"), "the line became the group");
}

#[test]
fn the_program_parses_and_checks_clean() {
    for code in [HANDLER, ""] {
        let src = generate(&actors(code));
        let parsed = cobolt_parser::parse(cobolt_lexer::tokenize(&src, cobolt_lexer::SourceFormat::Free));
        let errors: Vec<String> = parsed
            .diagnostics
            .iter()
            .filter(|d| d.is_error())
            .map(|d| format!("line {}: {}", d.span.line, d.message))
            .collect();
        assert!(errors.is_empty(), "{errors:?}\n{src}");
        let program = parsed.program.expect("a program");
        let sem: Vec<String> = cobolt_semantic::analyze(&program).errors().map(|d| d.message.clone()).collect();
        assert!(sem.is_empty(), "{sem:?}\n{src}");
    }
}

/// R15 — the group follows the columns: rename one and the next generation describes the
/// new name, with the developer's text untouched.
#[test]
fn renaming_a_column_changes_the_group_not_the_handler() {
    let renamed = form(
        vec![column("Actor Id", "9(5)", "Textbox"), column("Full Name", "X(40)", "Textbox"), column("Edit", "", "Button")],
        HANDLER,
    );
    let src = generate(&renamed);
    assert!(src.contains("10 FULL-NAME PIC X(40)."), "{src}");
    assert!(!src.contains("10 ACTOR-NAME"), "{src}");
    assert!(src.contains("01 DATAGRID-ROW-BUFFER     PIC X(84)."), "{src}");
}

/// The debugger keeps its place: every line the developer wrote maps to its own line of
/// the handler, the group's lines to nothing.
#[test]
fn every_written_line_maps_to_itself_and_the_group_to_nothing() {
    let (src, map) = generate_with_map(&actors(HANDLER));
    let lines: Vec<&str> = src.lines().collect();
    // Searched from the handler's own PROGRAM-ID: the main program has lines like these too.
    let handler = lines.iter().position(|l| l.contains("PROGRAM-ID. DG--ONROWBUTTONCLICK")).expect("the handler");
    let gen_of = |needle: &str| {
        (handler + lines[handler..].iter().position(|l| l.contains(needle)).expect(needle)) as u32 + 1
    };
    let site_of = |needle: &str| HANDLER.lines().position(|l| l.contains(needle)).expect(needle) as u32 + 1;
    for needle in ["DISPLAY ACTOR-NAME", "DISPLAY ROW-NUMBER", "PROCEDURE DIVISION USING", "LINKAGE SECTION."] {
        let (_, site_line) = map.resolve(gen_of(needle)).unwrap_or_else(|| panic!("{needle} is not mapped"));
        assert_eq!(site_line, site_of(needle), "{needle}");
    }
    assert!(map.resolve(gen_of("10 ACTOR-ID PIC 9(5).")).is_none(), "the group is generated");
}

/// The System KB's example handler (cobolt-compiler, DataGrid usage notes) is real code:
/// with the grid it describes and a TextBox for the name, it parses and checks clean.
#[test]
fn the_kb_example_handler_checks_clean() {
    const KB_HANDLER: &str = "       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 WS-OK PIC 9.
       LINKAGE SECTION.
       COPY DATAGRID-ROW-CONTENT.
       PROCEDURE DIVISION USING DATAGRID-ROW-CONTENT.
           EVALUATE ROW-NAME
               WHEN \"EDIT\"   MOVE ACTOR-NAME TO TXT-NAME::Text
               WHEN \"DELETE\" MOVE DG-1::DeleteRow(ROW-NUMBER) TO WS-OK
           END-EVALUATE.";
    let mut f = form(
        vec![
            column("Actor Id", "9(5)", "Textbox"),
            column("Actor Name", "", "Textbox"),
            column("Edit", "", "Button"),
            column("Delete", "", "Button"),
        ],
        KB_HANDLER,
    );
    f.controls[0].id = "DG-1".into();
    let mut ev = EventBinding::for_control("DG-1", "onRowButtonClick");
    ev.code = KB_HANDLER.to_owned();
    f.controls[0].events = vec![ev];
    f.add_control(Control::new("TXT-NAME", ControlType::TextBox, 0, 300));
    let src = generate(&f);
    let parsed = cobolt_parser::parse(cobolt_lexer::tokenize(&src, cobolt_lexer::SourceFormat::Free));
    let errors: Vec<String> = parsed
        .diagnostics
        .iter()
        .filter(|d| d.is_error())
        .map(|d| format!("line {}: {}", d.span.line, d.message))
        .collect();
    assert!(errors.is_empty(), "{errors:?}\n{src}");
    let program = parsed.program.expect("a program");
    let sem: Vec<String> = cobolt_semantic::analyze(&program).errors().map(|d| d.message.clone()).collect();
    assert!(sem.is_empty(), "{sem:?}\n{src}");
    assert!(src.contains("CALL \"DG-1--ONROWBUTTONCLICK\" USING DATAGRID-ROW-BUFFER"), "{src}");
}

/// AC9 — a form that does not bind the event generates exactly as before: no buffer.
#[test]
fn a_grid_without_the_event_has_no_buffer() {
    let mut f = actors(HANDLER);
    f.controls[0].events.clear();
    let src = generate(&f);
    assert!(!src.contains("DATAGRID-ROW-BUFFER"), "{src}");
}
