// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Spec 087 T9 (AC12) — Check reports every embedded-SQL error R46 lists,
//! once each and on its line, without any database: through `validate_text`,
//! the path the coding-agent `check` tool, `rcrun check` and the IDE share.

use cobolt_lexer::SourceFormat;
use cobolt_project_tools::validate_source::{validate_text, Severity};

/// One program holding every R46 error, each marked by a `*> E:<tag>` comment
/// on the line it must be reported on. The unterminated block is last: it
/// swallows whatever follows it. `rcrun check` reads the same file
/// (`cobolt-cli/tests/check_exec_sql.rs`).
const SOURCE: &str = include_str!("fixtures/esql_r46.cbl");

#[test]
fn every_r46_error_is_one_diagnostic_on_its_line() {
    let dir = std::env::temp_dir().join(format!("esql-check-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("checkme.cbl");
    std::fs::write(&path, SOURCE).unwrap();

    let diags = validate_text(SOURCE, &path, SourceFormat::Free, None);
    let errors: Vec<(u32, String)> =
        diags.iter().filter(|d| d.severity == Severity::Error).map(|d| (d.line, d.message.clone())).collect();

    let expected: Vec<(u32, &str)> = SOURCE
        .lines()
        .enumerate()
        .filter_map(|(i, l)| l.split("*> E:").nth(1).map(|tag| (i as u32 + 1, tag.trim())))
        .collect();
    println!("R46 diagnostics ({} expected):", expected.len());
    for (line, msg) in &errors {
        println!("  line {line:>3}: {msg}");
    }
    for (line, tag) in &expected {
        let n = errors.iter().filter(|(l, _)| l == line).count();
        assert_eq!(n, 1, "{tag} on line {line}: expected one diagnostic, got {n}: {errors:#?}");
    }
    assert_eq!(errors.len(), expected.len(), "no other diagnostic: {errors:#?}");
    let last = errors.iter().find(|(l, _)| *l == expected.last().unwrap().0).unwrap();
    assert_eq!(last.1, "unterminated EXEC SQL block (missing END-EXEC)");
}

/// Spec 087 T10 (AC15, Check part) — on the operator's own PowerCOBOL
/// programs, read from `PRC_LEGACY_CBL_DIR` and never copied into the
/// repository, no diagnostic comes from an `EXEC SQL` block.
///
/// The files are PowerCOBOL compiler listings: their `#FILE` / `#LINE`
/// directive lines are blanked (keeping every line number) before Check.
/// Their non-SQL PowerCOBOL constructs (`POW-…` items, `CALL … WITH
/// STDCALL`, `BY VALUE`) still fail Check — converting them is out of this
/// spec's scope, which is why this asserts only on the SQL blocks.
#[test]
fn the_operators_samples_have_no_diagnostic_inside_an_sql_block() {
    let Some(dir) = std::env::var_os("PRC_LEGACY_CBL_DIR") else {
        println!("SKIPPED: PRC_LEGACY_CBL_DIR is not set");
        return;
    };
    let dir = std::path::PathBuf::from(dir);
    for rel in ["M-ARTICULOS/Debug/F-ART-PURGA.cob", "TyC/Debug/TyC.cob"] {
        let path = dir.join(rel);
        let bytes = std::fs::read(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
        let text = cobolt_lexer::decode_source_bytes(bytes);
        let source: String = text
            .lines()
            .map(|l| if l.starts_with('#') { "" } else { l })
            .collect::<Vec<_>>()
            .join("\n");
        // The blocks' line ranges, from the source itself.
        let mut ranges = Vec::new();
        let mut start = None;
        for (i, l) in source.lines().enumerate() {
            let body = l.get(6..).unwrap_or("");
            if body.starts_with('*') || body.starts_with('/') {
                continue;
            }
            let up = body.to_ascii_uppercase();
            if start.is_none() && up.split_whitespace().collect::<Vec<_>>().windows(2).any(|w| w == ["EXEC", "SQL"]) {
                start = Some(i as u32 + 1);
            }
            if let Some(s) = start {
                if up.contains("END-EXEC") {
                    ranges.push((s, i as u32 + 1));
                    start = None;
                }
            }
        }
        let diags = validate_text(&source, &path, SourceFormat::Fixed, None);
        let errors: Vec<_> = diags.iter().filter(|d| d.severity == Severity::Error).collect();
        let inside: Vec<_> =
            errors.iter().filter(|d| ranges.iter().any(|(a, b)| d.line >= *a && d.line <= *b)).collect();
        println!(
            "{rel}: {} SQL blocks, {} diagnostics in all (non-SQL PowerCOBOL constructs), {} inside an SQL block",
            ranges.len(),
            errors.len(),
            inside.len()
        );
        assert!(!ranges.is_empty(), "{rel}: no EXEC SQL block found");
        assert!(inside.is_empty(), "{rel}: diagnostics inside SQL blocks: {inside:#?}");
    }
}
