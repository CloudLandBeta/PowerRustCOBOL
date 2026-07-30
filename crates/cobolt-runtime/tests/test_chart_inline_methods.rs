// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Inline chart methods — `Chart::AddPoint(label, value)`, `Chart::Clear()`
//! and `Chart::Refresh()` drive the same `__ChartData` path as the
//! `CALL "COBOL-CHART-*"` runtime calls, so the syntax the Knowledge Base
//! documents is executable as written.

use std::sync::mpsc;

use cobolt_lexer::{tokenize, SourceFormat};
use cobolt_parser::{parse, Severity};
use cobolt_runtime::{Interpreter, StateUpdate};

fn run_with_chart(src: &str) -> Vec<StateUpdate> {
    let result = parse(tokenize(src, SourceFormat::Free));
    assert!(
        result
            .diagnostics
            .iter()
            .all(|d| d.severity != Severity::Error),
        "parse errors: {:?}",
        result.diagnostics
    );
    let program = result.program.expect("no program");
    let (_event_tx, event_rx) = mpsc::channel();
    let (state_tx, state_rx) = mpsc::channel();
    let (display_tx, _display_rx) = mpsc::channel();
    let mut interp = Interpreter::new_with_channels(program, event_rx, state_tx, display_tx);
    interp.seed_objects(vec![(
        "LineChart-1".to_owned(),
        "LineChart".to_owned(),
        vec![("Title".to_owned(), "Sales".to_owned())],
    )]);
    interp.run().expect("run failed");
    state_rx.try_iter().collect()
}

fn chart_data_updates(updates: &[StateUpdate]) -> Vec<String> {
    updates
        .iter()
        .filter(|u| u.prop == "__ChartData")
        .map(|u| u.value.clone())
        .collect()
}

#[test]
fn inline_addpoint_appends_points_to_chart_data() {
    let src = r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. DEMO.
       PROCEDURE DIVISION.
           LINECHART-1::AddPoint("Jan", 150).
           LINECHART-1::AddPoint("Feb", 200).
           STOP RUN.
"#;
    let data = chart_data_updates(&run_with_chart(src));
    assert_eq!(data.len(), 2, "one __ChartData push per AddPoint: {data:?}");
    assert_eq!(data[0], "Jan\t150");
    assert_eq!(data[1], "Jan\t150\nFeb\t200");
}

#[test]
fn inline_clear_drops_chart_data() {
    let src = r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. DEMO.
       PROCEDURE DIVISION.
           LINECHART-1::AddPoint("Jan", 150).
           LINECHART-1::Clear().
           STOP RUN.
"#;
    let data = chart_data_updates(&run_with_chart(src));
    assert_eq!(data.len(), 2, "AddPoint push then Clear push: {data:?}");
    assert_eq!(data[1], "", "Clear sends an empty series");
}

#[test]
fn inline_refresh_resends_current_points() {
    let src = r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. DEMO.
       PROCEDURE DIVISION.
           LINECHART-1::AddPoint("Jan", 150).
           LINECHART-1::Refresh().
           STOP RUN.
"#;
    let data = chart_data_updates(&run_with_chart(src));
    assert_eq!(data.len(), 2, "AddPoint push then Refresh push: {data:?}");
    assert_eq!(data[1], "Jan\t150");
}

#[test]
fn inline_addpoint_interops_with_chart_runtime_calls() {
    // The CALL form and the inline form share one canonical (upper-cased)
    // data store — mixing them must not fork the series.
    let src = r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. DEMO.
       PROCEDURE DIVISION.
           CALL "COBOL-CHART-ADD-POINT" USING "LineChart-1" "Jan" 150.
           LINECHART-1::AddPoint("Feb", 200).
           STOP RUN.
"#;
    let data = chart_data_updates(&run_with_chart(src));
    assert_eq!(data.len(), 2, "{data:?}");
    assert_eq!(data[1], "Jan\t150\nFeb\t200");
}
