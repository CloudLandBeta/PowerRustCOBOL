// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! A `X::…` receiver must name something the form or the program has
//! (operator report, 2026-10-02): `INVOKE btn-xxxxxxx::disable()` on a form
//! with no such button compiled silently and did nothing at run time.
//!
//! With the form's objects supplied, an unknown root — in `INVOKE`, in a
//! `MOVE`/`SET` target, or inside an expression — is an error that names the
//! receiver and suggests the nearest real one. `me`, `super`, `COBOL`, a data
//! item and a REPOSITORY class pass; without form context nothing is checked.

use std::collections::HashSet;

use cobolt_lexer::{tokenize, SourceFormat};
use cobolt_parser::parse;
use cobolt_semantic::{analyze, analyze_with, AnalyzeOptions, Severity};

fn program_with(procedure: &str) -> cobolt_ast::program::Program {
    let src = format!(
        "\
IDENTIFICATION DIVISION.
PROGRAM-ID. RECVTEST.
DATA DIVISION.
WORKING-STORAGE SECTION.
01 WS-TEXT PIC X(40).
01 WS-N    PIC 9(4).
PROCEDURE DIVISION.
MAIN.
{procedure}
    STOP RUN.
"
    );
    parse(tokenize(&src, SourceFormat::Free)).program.expect("program should parse")
}

fn form_objects() -> HashSet<String> {
    ["CUST-FORM", "BTN-SAVE", "LBL-STATUS", "GRD-ROWS"].iter().map(|s| s.to_string()).collect()
}

fn errors(procedure: &str) -> Vec<String> {
    analyze_with(
        &program_with(procedure),
        &AnalyzeOptions { known_objects: Some(form_objects()), ..Default::default() },
    )
    .diagnostics
    .into_iter()
    .filter(|d| d.severity == Severity::Error)
    .map(|d| d.message)
    .collect()
}

#[test]
fn a_receiver_the_form_does_not_have_is_an_error() {
    let mut rows = Vec::new();
    for (code, name) in [
        ("    INVOKE BTN-SAVX::Disable().", "BTN-SAVX"),
        ("    INVOKE btn-xxxxxxx::disable().", "btn-xxxxxxx"),
        ("    MOVE \"ok\" TO LBL-STATSU::Caption.", "LBL-STATSU"),
        ("    SET LBL-NOPE::Visible TO TRUE.", "LBL-NOPE"),
        ("    MOVE GRD-ROWZ::Rows(1)::Value TO WS-TEXT.", "GRD-ROWZ"),
    ] {
        let e = errors(code);
        assert_eq!(e.len(), 1, "{code}: {e:?}");
        assert!(
            e[0].to_ascii_uppercase().contains(&format!("NO CONTROL OR OBJECT NAMED '{}'", name.to_ascii_uppercase())),
            "{code}: {}",
            e[0]
        );
        rows.push(e[0].clone());
    }
    // A near miss names the real one; a wild one does not guess.
    assert!(rows[0].contains("Did you mean 'BTN-SAVE'?"), "{}", rows[0]);
    assert!(rows[2].contains("Did you mean 'LBL-STATUS'?"), "{}", rows[2]);
    assert!(!rows[1].contains("Did you mean"), "{}", rows[1]);
    println!("receiver check — 5 unknown receivers refused:\n  {}", rows.join("\n  "));
}

#[test]
fn every_real_receiver_passes() {
    let code = "\
    INVOKE BTN-SAVE::Disable().
    MOVE \"ok\" TO Lbl-Status::Caption.
    MOVE GRD-ROWS::Rows(1)::Value TO WS-TEXT.
    MOVE CUST-FORM::Width TO WS-N.
    MOVE me::Width TO WS-N.
    MOVE super::Height TO WS-N.
    COBOL::\"MODEL-LIST\" ( WS-TEXT WS-TEXT WS-TEXT WS-N ).
    MOVE WS-TEXT::UpperCase() TO WS-TEXT.";
    assert_eq!(errors(code), Vec::<String>::new());
}

#[test]
fn without_form_context_nothing_is_checked() {
    let r = analyze(&program_with("    INVOKE BTN-SAVX::Disable()."));
    assert!(r.errors().next().is_none(), "{:?}", r.diagnostics);
}
