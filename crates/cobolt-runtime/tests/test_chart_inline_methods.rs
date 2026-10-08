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
    run_with_object(src, "LineChart-1", "LineChart")
}

fn run_with_object(src: &str, id: &str, class: &str) -> Vec<StateUpdate> {
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
        id.to_owned(),
        class.to_owned(),
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

/// Several series (operator, 2026-09-26: chart stacking): on a bar, line or
/// area chart each argument after the value is the point's value in the next
/// series, carried on the same line; a point given fewer is padded with 0,
/// and a later wider point pads the earlier ones, so the series stay aligned.
#[test]
fn inline_addpoint_takes_a_value_per_series() {
    let src = r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. DEMO.
       PROCEDURE DIVISION.
           LINECHART-1::AddPoint("Jan", 150, 90).
           LINECHART-1::AddPoint("Feb", 200).
           LINECHART-1::AddPoint("Mar", 120, 80, 40).
           STOP RUN.
"#;
    let data = chart_data_updates(&run_with_chart(src));
    assert_eq!(data[0], "Jan\t150\t90");
    assert_eq!(data[1], "Jan\t150\t90\nFeb\t200\t0");
    assert_eq!(data[2], "Jan\t150\t90\t0\nFeb\t200\t0\t0\nMar\t120\t80\t40");
}

/// A radar takes the same multi-series points as the bar, line and area
/// charts: each `AddPoint` is one AXIS (its label), and the arguments after
/// the label are that axis's value in series 1, 2, 3. `Clear` and `Refresh`
/// work as on every chart, and the built-in `CHART-ADD-POINT` call carries the
/// extra series too.
#[test]
fn a_radar_chart_takes_a_value_per_series_and_clears() {
    let src = r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. DEMO.
       PROCEDURE DIVISION.
           RADAR-1::AddPoint("Speed", 80, 55, 35).
           RADAR-1::AddPoint("Power", 60, 85).
           CALL "COBOL-CHART-ADD-POINT" USING "Radar-1" "Range" 70 45 90.
           RADAR-1::Refresh().
           RADAR-1::Clear().
           STOP RUN.
"#;
    let data = chart_data_updates(&run_with_object(src, "Radar-1", "RadarChart"));
    assert_eq!(data[0], "Speed\t80\t55\t35");
    assert_eq!(data[1], "Speed\t80\t55\t35\nPower\t60\t85\t0");
    assert_eq!(data[2], "Speed\t80\t55\t35\nPower\t60\t85\t0\nRange\t70\t45\t90");
    assert_eq!(data[3], data[2], "Refresh re-sends the same points");
    assert_eq!(data[4], "", "Clear sends an empty set, so the sample comes back");
    println!("RadarChart: 3 axes x 3 series through AddPoint and CHART-ADD-POINT, Refresh and Clear as on every chart");
}
