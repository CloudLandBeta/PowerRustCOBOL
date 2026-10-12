// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! The rewrite of the older value-method spellings, run over every COBOL file the
//! repository holds — the RustCOBOL-85 test programs, the generated programs of
//! the example projects, the NIST suite — with the MOST permissive receiver rule
//! there is (every name is a data item), which is the worst case: it rewrites as
//! much as it ever could.
//!
//! What it proves, file by file: the rewritten text tokenises to the same tokens
//! as the original — comments, literals and embedded blocks included — except
//! for the spellings themselves; and a second pass finds nothing. So whatever
//! the scanner decides, it cannot have changed anything else.

use std::path::{Path, PathBuf};

use cobolt_ast::value_methods::{canonicalize_value_methods, differs_only_by_spellings};
use cobolt_lexer::SourceFormat;

fn repo() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn cobol_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(rd) = std::fs::read_dir(dir) else { return };
    for e in rd.flatten() {
        let p = e.path();
        if p.is_dir() {
            if p.file_name().is_some_and(|n| n == "target" || n == ".git") {
                continue;
            }
            cobol_files(&p, out);
        } else if p
            .extension()
            .and_then(|e| e.to_str())
            .is_some_and(|e| ["cbl", "cob", "cpy"].iter().any(|x| e.eq_ignore_ascii_case(x)))
            // The NIST CCVS85 bundle: `newcob.val,cbl`, every program in classic card-image form.
            || p.file_name().is_some_and(|n| n.to_string_lossy().ends_with(",cbl"))
        {
            out.push(p);
        }
    }
}

/// The IDE's own guess (`cobolt_project_tools::validate_source::source_format`):
/// fixed when some line has a sequence area and an indicator, free otherwise.
fn format_of(source: &str) -> SourceFormat {
    if source.lines().any(|l| {
        let b = l.as_bytes();
        b.len() > 6 && b[6] != b' ' && b[..6].iter().all(|&c| c == b' ' || c.is_ascii_digit())
    }) {
        SourceFormat::Fixed
    } else {
        SourceFormat::Free
    }
}

#[test]
fn over_every_cobol_file_in_the_repository_the_rewrite_changes_nothing_but_the_spellings() {
    let mut files = Vec::new();
    for dir in ["tests/cobol", "crates/cobolt-codegen/tests/golden", "examples", "NIST"] {
        cobol_files(&repo().join(dir), &mut files);
    }
    files.sort();
    assert!(files.len() > 100, "the corpus is the point: {} files", files.len());

    let (mut bytes, mut with_edits, mut edits, mut formats_fixed) = (0usize, 0usize, 0usize, 0usize);
    let mut bad = Vec::new();
    for path in &files {
        let Ok(raw) = std::fs::read(path) else { continue };
        // Windows-1252 sources are decoded the way the editor reads them.
        let text = cobolt_lexer::decode_source(raw).0;
        bytes += text.len();
        for format in [format_of(&text), SourceFormat::Free, SourceFormat::FixedStrict] {
            if format == format_of(&text) && format == SourceFormat::Fixed {
                formats_fixed += 1;
            }
            let r = canonicalize_value_methods(&text, format, &|_| true);
            if format == format_of(&text) && r.changed() {
                with_edits += 1;
                edits += r.edits.len();
            }
            if !differs_only_by_spellings(&text, &r.text, format) {
                bad.push(format!("{} ({format:?}): the rewrite changed more than a spelling", path.display()));
            }
            if canonicalize_value_methods(&r.text, format, &|_| true).changed() {
                bad.push(format!("{} ({format:?}): not idempotent", path.display()));
            }
        }
    }
    println!(
        "  value-method rewrite over {} COBOL files ({} MB; {} fixed-form): {} file(s) hold older spellings, {} edit(s) in all; every rewrite checked token-for-token, twice-run idempotent",
        files.len(),
        bytes / 1_000_000,
        formats_fixed,
        with_edits,
        edits
    );
    assert!(bad.is_empty(), "{}", bad.join("\n"));
}
