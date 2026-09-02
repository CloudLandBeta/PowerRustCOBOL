// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! The shipped Snackbar example's COBOL **compiles**, not merely parses as XML.
//!
//! Same gate, and for the same reason, as `maps_demo_compiles.rs`: a form whose
//! handlers do not compile still loads perfectly, and the developer finds out at
//! Run. This one matters more than most — it is the worked example for a control
//! whose whole surface is methods (`Show`, `DismissAll`, `Clear`, `AddButton`)
//! and runtime-only properties (`LastButtonId`, `LastButtonIndex`), so a
//! misspelling here is a misspelling someone will copy.
//!
//! The file lives in the operator's demo project, not in this repository, so the
//! test SKIPS when it is absent: a fresh clone must not fail for missing
//! something it was never given.

use std::path::PathBuf;

fn demo_form() -> Option<PathBuf> {
    let p = PathBuf::from(std::env::var("HOME").ok()?)
        .join("Documents/PowerDemo3/forms/Non-Visual/snackbar-form.cfrm");
    p.exists().then_some(p)
}

#[test]
fn the_snackbar_example_generates_a_program_that_compiles() {
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

    // Every capability the example exists to show, still in the code it
    // generates. A demo that quietly stops exercising one of these is worse
    // than no demo: it reads as coverage.
    let must_carry = [
        ("Clear()", "SNACKBAR-1::Clear()"),
        ("AddButton()", "SNACKBAR-1::AddButton("),
        ("Show()", "SNACKBAR-1::Show()"),
        ("DismissAll()", "SNACKBAR-1::DismissAll()"),
        ("which button", "SNACKBAR-1::LastButtonId"),
        ("its index", "SNACKBAR-1::LastButtonIndex"),
        ("the Critical category", "\"Critical\""),
        ("every anchor", "\"TopLeft\""),
        ("the overflow policies", "\"DiscardNewest\""),
        ("the size classes", "\"Large\""),
    ];
    eprintln!("\n  capability              present");
    eprintln!("  ---------------------   -------");
    let mut missing: Vec<&str> = Vec::new();
    for (what, needle) in must_carry {
        let ok = src.contains(needle);
        eprintln!("  {what:<21}   {}", if ok { "yes" } else { "NO" });
        if !ok {
            missing.push(what);
        }
    }
    assert!(missing.is_empty(), "the example stopped covering: {missing:?}");

    // The five Snackbar events each get a generated paragraph, so a handler
    // bound in the designer is actually reachable.
    let events = ["ONSHOWN", "ONTIMEOUT", "ONCLOSING", "ONCLOSED", "ONBUTTONCLICK"];
    let bound = events
        .iter()
        .filter(|e| src.contains(&format!("SNACKBAR-1--{e}")))
        .count();
    eprintln!("\n  {bound}/{} Snackbar events have a generated handler", events.len());
    assert_eq!(bound, events.len(), "every Snackbar event must be wired: {src:.0}");

    eprintln!(
        "  → {} lines generated, parsed and analysed with 0 errors\n",
        src.lines().count()
    );
}
