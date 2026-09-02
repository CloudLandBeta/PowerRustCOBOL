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
//! The file lives in the operator's demo project, not in this repository, so the
//! test SKIPS when it is absent.

use std::path::PathBuf;

fn demo_form() -> Option<PathBuf> {
    let p = PathBuf::from(std::env::var("HOME").ok()?)
        .join("Documents/PowerDemo3/forms/Charts/charts-form.cfrm");
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
