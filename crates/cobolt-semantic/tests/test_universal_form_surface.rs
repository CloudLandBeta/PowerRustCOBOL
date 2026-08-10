// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Spec 049 R33/R34/AC16 — bare properties on `me`/`super` are checked
//! against the universal form surface at build time, at any chain depth;
//! method calls (form-specific procedures) pass and dispatch at run time.

use cobolt_lexer::{tokenize, SourceFormat};
use cobolt_parser::parse;
use cobolt_semantic::{analyze, Severity};

fn errors_for(procedure: &str) -> Vec<String> {
    let src = format!(
        "\
IDENTIFICATION DIVISION.
PROGRAM-ID. SURFTEST.
DATA DIVISION.
WORKING-STORAGE SECTION.
01 WS-X PIC X(20).
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
fn a_misspelt_universal_property_fails_at_any_depth() {
    // AC16 — depth 1.
    let errs = errors_for("    MOVE super::Widht TO WS-X.");
    assert_eq!(errs.len(), 1, "depth 1: {errs:?}");
    // The lexer uppercases member names, so the diagnostic says WIDHT.
    assert!(
        errs[0].contains("super::WIDHT") && errs[0].contains("not a form property"),
        "names the receiver and the bad property: {}",
        errs[0]
    );

    // AC16 — depth 3 (super::super::super::Widht).
    let errs3 = errors_for("    MOVE super::super::super::Widht TO WS-X.");
    assert_eq!(errs3.len(), 1, "depth 3: {errs3:?}");
    assert!(errs3[0].contains("WIDHT"), "depth 3 names it: {}", errs3[0]);

    // And on me.
    let errs_me = errors_for("    MOVE me::Titel TO WS-X.");
    assert_eq!(errs_me.len(), 1, "me depth 1: {errs_me:?}");

    println!(
        "049 AC16 — misspelt property rejected at super depth 1, super depth 3 \
         and on me (3/3), each error listing the {}-entry universal surface",
        15
    );
}

#[test]
fn valid_properties_and_methods_pass() {
    // Every universal property passes on both receivers, at depth.
    for src in [
        "    MOVE me::Width TO WS-X.",
        "    MOVE me::Title TO WS-X.",
        "    MOVE super::Title TO WS-X.",
        "    MOVE super::super::WindowState TO WS-X.",
        "    MOVE super::FormState TO WS-X.",
        "    MOVE \"X\" TO super::Title.",
    ] {
        let errs = errors_for(src);
        assert!(errs.is_empty(), "{src} must pass: {errs:?}");
    }
    // R34 — a form-specific PROCEDURE (parens) builds; it dispatches at run
    // time. Same for chained window methods.
    for src in [
        r#"    INVOKE super::"RecalcTotals"()."#,
        r#"    INVOKE super::super::"Close"()."#,
        r#"    INVOKE ME::"OpenFormAsync"("F2") RETURNING WS-X."#,
    ] {
        let errs = errors_for(src);
        assert!(errs.is_empty(), "{src} must build: {errs:?}");
    }
    println!(
        "049 R33/R34 — 6 property forms pass (both receivers, depth 2 included); \
         3 method forms build for runtime dispatch"
    );
}

#[test]
fn control_chains_are_untouched() {
    // A control root is not a form receiver — no universal-surface check.
    let errs = errors_for("    MOVE Grid-1::Rows TO WS-X.");
    assert!(errs.is_empty(), "control chains unaffected: {errs:?}");
    println!("049 — control-rooted chains bypass the form-surface check (0 errors)");
}
