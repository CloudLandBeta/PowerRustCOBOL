// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Spec 037 R22/AC12 — the SPACE (COBOL-standard) form of
//! `INVOKE me "OpenFormSync"/"OpenFormAsync"` requires every parameter and
//! type-checks literal arguments at compile time; the comma form is exempt
//! (its trailing parameters default from the RAD design, R21).

use cobolt_lexer::{tokenize, SourceFormat};
use cobolt_parser::parse;
use cobolt_semantic::{analyze, Severity};

fn errors_for(procedure: &str) -> Vec<String> {
    let src = format!(
        "\
IDENTIFICATION DIVISION.
PROGRAM-ID. SIGTEST.
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
    let program = result.program.expect("program should parse");
    analyze(&program)
        .diagnostics
        .into_iter()
        .filter(|d| d.severity == Severity::Error)
        .map(|d| d.message)
        .collect()
}

#[test]
fn open_form_signature_space_form_missing_params_is_a_compile_error() {
    let errs = errors_for(r#"    INVOKE ME "OpenFormSync" USING "F2" RETURNING WS-H."#);
    assert_eq!(errs.len(), 1, "one signature error expected: {errs:?}");
    assert!(
        errs[0].contains("OPENFORMSYNC") && errs[0].contains("requires all 7"),
        "diagnostic names the method and the arity: {}",
        errs[0]
    );
    println!("space-form missing params → {}", errs[0]);
}

#[test]
fn open_form_signature_space_form_wrong_literal_type_is_a_compile_error() {
    // x (parameter 3) must be numeric — a string literal is a type error.
    let errs = errors_for(
        r#"    INVOKE ME "OpenFormAsync" USING "F2" "Normal" "ten" 20 300 200 RETURNING WS-H."#,
    );
    assert_eq!(errs.len(), 1, "one type error expected: {errs:?}");
    assert!(
        errs[0].contains("parameter 3") && errs[0].contains("OPENFORMASYNC"),
        "diagnostic names the parameter and method: {}",
        errs[0]
    );
    println!("space-form wrong type → {}", errs[0]);
}

#[test]
fn open_form_signature_space_form_complete_call_is_clean() {
    let errs = errors_for(
        r#"    INVOKE ME "OpenFormSync" USING "F2" "Normal" 10 20 300 200 "true" RETURNING WS-H."#,
    );
    assert!(errs.is_empty(), "complete space form is clean: {errs:?}");
    println!("space-form complete call → no diagnostics");
}

#[test]
fn open_form_signature_comma_form_omits_freely() {
    // The comma form's trailing parameters are optional (R21) — one arg is fine.
    let errs = errors_for(r#"    INVOKE ME::"OpenFormSync"("F2") RETURNING WS-H."#);
    assert!(errs.is_empty(), "comma form is exempt: {errs:?}");
    println!("comma-form single arg → no diagnostics");
}

// ── 051 R24 — the SideMenu pair obeys the same signature discipline ─────────

#[test]
fn standalone_signature_space_form_missing_params_is_a_compile_error() {
    let errs =
        errors_for(r#"    INVOKE SIDE-1 "OpenStandAloneFormSync" USING "F2" RETURNING WS-H."#);
    assert_eq!(errs.len(), 1, "one signature error expected: {errs:?}");
    assert!(
        errs[0].contains("OPENSTANDALONEFORMSYNC") && errs[0].contains("requires all 7"),
        "diagnostic names the method and the arity: {}",
        errs[0]
    );
    assert!(
        errs[0].contains("<side-menu-control>"),
        "the example call names the SideMenu receiver, not `me`: {}",
        errs[0]
    );
    println!("standalone space-form missing params → {}", errs[0]);
}

#[test]
fn standalone_signature_space_form_wrong_literal_type_is_a_compile_error() {
    // x (parameter 3) must be numeric — a string literal is a type error (AC14).
    let errs = errors_for(
        r#"    INVOKE SIDE-1 "OpenStandAloneFormAsync" USING "F2" "Normal" "ten" 20 300 200 RETURNING WS-H."#,
    );
    assert_eq!(errs.len(), 1, "one type error expected: {errs:?}");
    assert!(
        errs[0].contains("parameter 3") && errs[0].contains("OPENSTANDALONEFORMASYNC"),
        "diagnostic names the parameter and method: {}",
        errs[0]
    );
    println!("standalone space-form wrong type → {}", errs[0]);
}

#[test]
fn standalone_signature_complete_and_comma_forms_are_clean() {
    let errs = errors_for(
        r#"    INVOKE SIDE-1 "OpenStandAloneFormSync" USING "F2" "Normal" 10 20 300 200 "true" RETURNING WS-H."#,
    );
    assert!(errs.is_empty(), "complete space form is clean: {errs:?}");
    // The comma form with only the form id passes (AC14).
    let errs = errors_for(r#"    INVOKE SIDE-1::"OpenStandAloneFormAsync"("F2") RETURNING WS-H."#);
    assert!(errs.is_empty(), "comma form is exempt: {errs:?}");
    println!("standalone complete space form + single-arg comma form → 0 diagnostics");
}
