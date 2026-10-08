// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! The shipped Charts example's COBOL **compiles**, not merely parses as XML.
//!
//! Same gate as `maps_demo_compiles.rs` and `snackbar_demo_compiles.rs`, for the
//! same reason: a form whose handlers do not compile still loads perfectly, and
//! the developer finds out at Run.
//!
//! Read from the REPOSITORY's own `examples/PowerDemo3` (spec 056 T0.5). It
//! used to read `~/Documents/PowerDemo3` and skip when that copy was absent —
//! on a machine without it the test had never run at all.

use std::path::PathBuf;

fn demo_form() -> Option<PathBuf> {
    // `CARGO_MANIFEST_DIR` is crates/<crate>; the example sits two levels up.
    let p = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../examples/PowerDemo3/forms/Charts/charts-form.cfrm");
    p.exists().then_some(p)
}

#[test]
fn the_charts_example_generates_a_program_that_compiles() {
    let Some(path) = demo_form() else {
        eprintln!("PowerDemo3 not present — skipping");
        return;
    };
    let form = cobolt_forms::load_form(&path).expect("the example form must parse");
    let src = cobolt_codegen::generate(&form);

    let parsed = cobolt_parser::parse(cobolt_lexer::tokenize(
        &src,
        cobolt_lexer::SourceFormat::Free,
    ));
    let parse_errors: Vec<String> = parsed
        .diagnostics
        .iter()
        .filter(|d| d.is_error())
        .map(|d| format!("  line {}: {}", d.span.line, d.message))
        .collect();
    assert!(
        parse_errors.is_empty(),
        "the example's generated program does not parse:\n{}",
        parse_errors.join("\n")
    );
    let program = parsed.program.expect("a parse with no errors yields a program");

    let sem = cobolt_semantic::analyze(&program);
    let errors: Vec<String> = sem
        .errors()
        .map(|d| format!("  line {}: {}", d.span.line, d.message))
        .collect();
    assert!(
        errors.is_empty(),
        "the example's generated program has {} semantic error(s):\n{}",
        errors.len(),
        errors.join("\n")
    );

    // All SIX chart types are present and all six are fed. A demo that quietly
    // drops one reads as coverage it no longer has.
    let types = [
        "BarChart", "LineChart", "PieChart", "AreaChart", "ScatterChart", "DonutChart",
    ];
    eprintln!("\n  chart          on the form   Clear()   AddPoint()");
    eprintln!("  ------------   -----------   -------   ----------");
    let mut gaps: Vec<String> = Vec::new();
    for t in types {
        let id = format!("{t}-1");
        let on_form = form
            .controls
            .iter()
            .any(|c| c.id.eq_ignore_ascii_case(&id));
        let cleared = src.to_uppercase().contains(&format!("{}::CLEAR()", id.to_uppercase()));
        let fed = src
            .to_uppercase()
            .contains(&format!("{}::ADDPOINT(", id.to_uppercase()));
        eprintln!(
            "  {t:<12}   {:<11}   {:<7}   {}",
            yn(on_form),
            yn(cleared),
            yn(fed)
        );
        if !(on_form && cleared && fed) {
            gaps.push(id);
        }
    }
    assert!(gaps.is_empty(), "chart types not fully demonstrated: {gaps:?}");

    // The per-type properties the example exists to show, still reachable.
    let switches = [
        ("Monochrome", "MONOCHROME"),
        ("MonochromeGradient", "MONOCHROMEGRADIENT"),
        ("ShowGridLines", "SHOWGRIDLINES"),
        ("ShowLegend", "SHOWLEGEND"),
        ("Horizontal (bar only)", "HORIZONTAL"),
        ("Smooth (line/area)", "SMOOTH"),
        ("LabelFormat (pie/donut)", "LABELFORMAT"),
        ("InnerRadius (donut)", "INNERRADIUS"),
        ("AnimateValues", "ANIMATEVALUES"),
        ("AnimationDuration", "ANIMATIONDURATION"),
    ];
    let upper = src.to_uppercase();
    let missing: Vec<&str> = switches
        .iter()
        .filter(|(_, needle)| !upper.contains(needle))
        .map(|(what, _)| *what)
        .collect();
    eprintln!("\n  {}/{} property switches exercised", switches.len() - missing.len(), switches.len());
    assert!(missing.is_empty(), "the example stopped exercising: {missing:?}");

    eprintln!(
        "  → {} lines generated, parsed and analysed with 0 errors\n",
        src.lines().count()
    );
}

fn yn(b: bool) -> &'static str {
    if b {
        "yes"
    } else {
        "NO"
    }
}

/// A RadarChart — which the PowerDemo3 example above predates — gets the same
/// proof the other six get: a form carrying one, fed several series from its
/// `onLoad` handler, generates a program that parses and analyses with no
/// error, and the generated COBOL carries the chart's usual paragraphs.
#[test]
fn a_radar_chart_form_generates_a_program_that_compiles() {
    use cobolt_forms::{Control, ControlType, Form, PropValue};

    let mut form = Form::new("RADAR-FORM", "Radar", 640, 480);
    let mut radar = Control::new("Radar-1", ControlType::RadarChart, 20, 20);
    radar.set_prop("Title", PropValue::String("Skills".into()));
    radar.set_prop("SeriesLabels", PropValue::String("Team A,Team B,Team C".into()));
    radar.set_prop("Transparency", PropValue::Int(40));
    form.controls.push(radar);
    let on_load = form
        .form_events
        .iter_mut()
        .find(|e| e.event == "onLoad")
        .expect("every form has an onLoad");
    on_load.code = "       ENVIRONMENT DIVISION.\n\
                    \x20      DATA DIVISION.\n\
                    \x20      WORKING-STORAGE SECTION.\n\
                    \x20      LINKAGE SECTION.\n\
                    \n\
                    \x20      PROCEDURE DIVISION.\n\
                    \x20          INVOKE Radar-1::Clear()\n\
                    \x20          INVOKE Radar-1::AddPoint(\"Speed\", 80, 55, 35)\n\
                    \x20          INVOKE Radar-1::AddPoint(\"Power\", 60, 85, 40)\n\
                    \x20          INVOKE Radar-1::AddPoint(\"Range\", 70, 45, 90)\n\
                    \x20          INVOKE Radar-1::AddPoint(\"Comfort\", 90, 60, 45)\n\
                    \x20          INVOKE Radar-1::Refresh().\n"
        .into();

    let src = cobolt_codegen::generate(&form);
    let parsed = cobolt_parser::parse(cobolt_lexer::tokenize(&src, cobolt_lexer::SourceFormat::Free));
    let parse_errors: Vec<String> = parsed
        .diagnostics
        .iter()
        .filter(|d| d.is_error())
        .map(|d| format!("  line {}: {}", d.span.line, d.message))
        .collect();
    assert!(parse_errors.is_empty(), "the radar form's program does not parse:\n{}", parse_errors.join("\n"));
    let program = parsed.program.expect("a parse with no errors yields a program");
    let errors: Vec<String> = cobolt_semantic::analyze(&program)
        .errors()
        .map(|d| format!("  line {}: {}", d.span.line, d.message))
        .collect();
    assert!(errors.is_empty(), "the radar form's program has semantic error(s):\n{}", errors.join("\n"));

    let upper = src.to_uppercase();
    for needle in [
        "RADAR-1-ADD-POINT.",
        "RADAR-1-SET-TABLE.",
        "RADAR-1-CLEAR.",
        "RADAR-1-REFRESH.",
        "RADAR-1::ADDPOINT(\"SPEED\", 80, 55, 35)",
        "WS-RADAR-1-SELECTED-LBL",
    ] {
        assert!(upper.contains(needle), "the radar form's program lacks {needle}");
    }
    eprintln!("\n  RadarChart: {} lines generated, parsed and analysed with 0 errors\n", src.lines().count());
}
