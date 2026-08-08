// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! External Crates in `EXEC RUST` (spec 044 R20–R22): a project's registered
//! crates extend the allowlist; an unregistered crate's error names the
//! remedy for the context it happened in.

use cobolt_lexer::{tokenize, SourceFormat};
use cobolt_parser::parse;
use cobolt_semantic::{analyze, analyze_with, AnalyzeOptions, SemanticResult};

fn program_using(krate: &str) -> cobolt_ast::program::Program {
    let src = format!(
        "IDENTIFICATION DIVISION.\nPROGRAM-ID. TESTPROG.\n\
         PROCEDURE DIVISION.\nMAIN.\n    EXEC RUST\n        use {krate}::anything;\n    END-EXEC.\n    STOP RUN.\n"
    );
    let result = parse(tokenize(&src, SourceFormat::Free));
    assert!(
        result.diagnostics.is_empty(),
        "fixture must parse clean: {:?}",
        result.diagnostics
    );
    result.program.unwrap()
}

fn with_registered(registered: &[&str]) -> AnalyzeOptions {
    AnalyzeOptions {
        external_crates: Some(registered.iter().map(|s| s.to_string()).collect()),
    }
}

fn unlinked_messages(result: &SemanticResult) -> Vec<String> {
    result
        .errors()
        .map(|d| d.message.clone())
        .filter(|m| m.contains("does not link"))
        .collect()
}

/// Spec R20 — a registered crate's name passes the check.
#[test]
fn a_registered_crate_is_accepted() {
    let program = program_using("csv");
    let result = analyze_with(&program, &with_registered(&["csv"]));
    assert!(
        unlinked_messages(&result).is_empty(),
        "registered csv must pass, got {:?}",
        unlinked_messages(&result)
    );
}

/// Spec R20 — registration is by `use`-line name: `serde-json` registers as
/// `serde_json` and that is the name the block writes.
#[test]
fn registration_uses_the_lib_name() {
    let program = program_using("serde_json");
    let result = analyze_with(&program, &with_registered(&["serde_json"]));
    assert!(unlinked_messages(&result).is_empty());
}

/// Spec R21 — in a project, an unregistered crate still fails, and the
/// message points at External Crates as the remedy.
#[test]
fn an_unregistered_crate_in_a_project_points_at_external_crates() {
    let program = program_using("csv");
    let result = analyze_with(&program, &with_registered(&[]));
    let messages = unlinked_messages(&result);
    assert_eq!(messages.len(), 1, "exactly one refusal, got {messages:?}");
    assert!(messages[0].contains("`csv`"));
    assert!(messages[0].contains("Project's Crates"));
}

/// Spec R22 — with no project (plain `analyze`), the message says external
/// crates require a project. This is also the `analyze()` regression guard:
/// an unlinked crate still errors at the developer's block.
#[test]
fn a_single_file_build_says_a_project_is_required() {
    let program = program_using("csv");
    let result = analyze(&program);
    let messages = unlinked_messages(&result);
    assert_eq!(messages.len(), 1, "exactly one refusal, got {messages:?}");
    assert!(messages[0].contains("require a project"));
}

/// Spec 041 R16's floor is untouched: the always-linked crates need no
/// registration in either context.
#[test]
fn the_always_linked_floor_is_unchanged() {
    for krate in ["std", "egui", "eframe"] {
        let program = program_using(krate);
        assert!(
            unlinked_messages(&analyze(&program)).is_empty(),
            "`{krate}` must pass with no project"
        );
        assert!(
            unlinked_messages(&analyze_with(&program, &with_registered(&[]))).is_empty(),
            "`{krate}` must pass in an empty project"
        );
    }
}
