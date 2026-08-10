// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Spec 049 R17/AC7 — the load-path check: `OpenFormSync`/`OpenFormAsync`
//! may not target a form whose FormFormat is `Embedded`; `Both` passes; the
//! check applies to the comma AND space spellings; with no project form map
//! it is silent.

use std::collections::HashMap;

use cobolt_lexer::{tokenize, SourceFormat};
use cobolt_parser::parse;
use cobolt_semantic::{analyze, analyze_with, AnalyzeOptions, FormLoadFormat, Severity};

fn program_with(procedure: &str) -> cobolt_ast::program::Program {
    let src = format!(
        "\
IDENTIFICATION DIVISION.
PROGRAM-ID. LOADTEST.
DATA DIVISION.
WORKING-STORAGE SECTION.
01 WS-H PIC X(8).
PROCEDURE DIVISION.
MAIN.
{procedure}
    STOP RUN.
"
    );
    let result = parse(tokenize(&src, SourceFormat::Free));
    result.program.expect("program should parse")
}

fn forms_map() -> HashMap<String, FormLoadFormat> {
    HashMap::from([
        ("CRM-PANEL".to_string(), FormLoadFormat::Embedded),
        ("CUST-LOOKUP".to_string(), FormLoadFormat::Both),
        ("MAIN-FORM".to_string(), FormLoadFormat::Standalone),
    ])
}

fn errors_with_map(procedure: &str) -> Vec<String> {
    analyze_with(
        &program_with(procedure),
        &AnalyzeOptions {
            external_crates: None,
            form_formats: Some(forms_map()),
        },
    )
    .diagnostics
    .into_iter()
    .filter(|d| d.severity == Severity::Error)
    .map(|d| d.message)
    .collect()
}

#[test]
fn open_form_on_an_embedded_form_is_a_compile_error_comma_form() {
    let errs = errors_with_map(r#"    INVOKE ME::"OpenFormSync"("CRM-PANEL") RETURNING WS-H."#);
    assert_eq!(errs.len(), 1, "one load-path error expected: {errs:?}");
    assert!(
        errs[0].contains("CRM-PANEL")
            && errs[0].contains("Embedded")
            && errs[0].contains("OpenFormSync"),
        "diagnostic names the form, its format, and the call: {}",
        errs[0]
    );
    println!("comma form on Embedded → {}", errs[0]);
}

#[test]
fn open_form_on_an_embedded_form_is_a_compile_error_space_form() {
    let errs = errors_with_map(
        r#"    INVOKE ME "OpenFormAsync" USING "CRM-PANEL" "Normal" 10 20 300 200 RETURNING WS-H."#,
    );
    assert_eq!(errs.len(), 1, "one load-path error expected: {errs:?}");
    assert!(
        errs[0].contains("CRM-PANEL") && errs[0].contains("OpenFormAsync"),
        "diagnostic names the form and the call: {}",
        errs[0]
    );
    println!("space form on Embedded → {}", errs[0]);
}

#[test]
fn open_form_on_a_both_form_passes() {
    let errs = errors_with_map(r#"    INVOKE ME::"OpenFormSync"("CUST-LOOKUP") RETURNING WS-H."#);
    assert!(errs.is_empty(), "Both must pass the standalone path: {errs:?}");
    let errs = errors_with_map(r#"    INVOKE ME::"OpenFormSync"("MAIN-FORM") RETURNING WS-H."#);
    assert!(errs.is_empty(), "Standalone must pass: {errs:?}");
    println!("Both + Standalone targets pass the load-path check (0 errors)");
}

#[test]
fn without_a_project_form_map_the_check_is_silent() {
    // Single-file builds have no form map — the same call must not error.
    let errs: Vec<String> =
        analyze(&program_with(
            r#"    INVOKE ME::"OpenFormSync"("CRM-PANEL") RETURNING WS-H."#,
        ))
        .diagnostics
        .into_iter()
        .filter(|d| d.severity == Severity::Error)
        .map(|d| d.message)
        .collect();
    assert!(errs.is_empty(), "no map ⇒ no load-path check: {errs:?}");
    println!("no project map ⇒ silent (0 errors)");
}

#[test]
fn dynamic_targets_and_unknown_forms_are_not_checked() {
    // A data-item target is dynamic; an unknown literal has no format entry.
    let errs = errors_with_map(r#"    INVOKE ME::"OpenFormSync"(WS-H) RETURNING WS-H."#);
    assert!(errs.is_empty(), "dynamic target stays runtime: {errs:?}");
    let errs = errors_with_map(r#"    INVOKE ME::"OpenFormSync"("NO-SUCH") RETURNING WS-H."#);
    assert!(errs.is_empty(), "unknown form is skipped: {errs:?}");
    println!("dynamic + unknown targets skipped (0 errors), 5/5 cases covered");
}
