// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! R16(b), AC8: the programs of `tests/cobol/` that fall inside the browser
//! subset print the same output on the module (both memory variants) and on
//! the interpreter. A program outside the subset is listed with the construct
//! that keeps it out. A difference is never skipped: every program that
//! differs must be in [`KNOWN`], tied to its open row of `specs/WASM-PARITY.md`,
//! and a known difference that has gone away fails too, so the list stays true.

mod common;

use cobolt_lexer::{tokenize, SourceFormat};
use cobolt_wasm::{compile_program, Options};
use std::path::{Path, PathBuf};

/// Programs whose output differs, with the `WASM-PARITY.md` row that says why.
const KNOWN: &[(&str, u32)] = &[];

fn programs(dir: &Path, out: &mut Vec<PathBuf>) {
    let mut entries: Vec<_> = std::fs::read_dir(dir)
        .unwrap()
        .flatten()
        .map(|e| e.path())
        .collect();
    entries.sort();
    for p in entries {
        if p.is_dir() {
            programs(&p, out);
        } else if p.extension().is_some_and(|e| e.eq_ignore_ascii_case("cbl")) {
            out.push(p);
        }
    }
}

/// A line that prints a measurement of the clock (an elapsed time, a rate):
/// two runs of one engine already differ there, so only its label is compared.
fn clock_free(l: &str) -> String {
    let low = l.to_ascii_lowercase();
    if low.contains("elapsed") || low.contains("per second") || low.contains("/second") {
        l.split(':').next().unwrap_or(l).to_string()
    } else {
        l.to_string()
    }
}

enum Verdict {
    Identical(usize),
    Differs(String),
    Excluded(String),
}

fn judge(path: &Path) -> Verdict {
    let raw = std::fs::read(path).unwrap();
    let src = String::from_utf8_lossy(&raw).into_owned();
    let src = match cobolt_lexer::preprocess_program(&src, path, SourceFormat::Free) {
        Some(x) if !x.errors.is_empty() => {
            return Verdict::Excluded(format!("COPY: {}", x.errors[0]))
        }
        Some(x) => x.text,
        None => src,
    };
    let parsed = cobolt_parser::parse(tokenize(&src, SourceFormat::Free));
    if let Some(d) = parsed.diagnostics.iter().find(|d| d.is_error()) {
        return Verdict::Excluded(format!(
            "does not parse: line {}: {}",
            d.span.line, d.message
        ));
    }
    let Some(prog) = parsed.program else {
        return Verdict::Excluded("no program".into());
    };
    if let Err(e) = compile_program(&prog, &Options::default()) {
        let why = format!("line {}: {}", e.line, e.message);
        return Verdict::Excluded(why);
    }
    let want: Vec<String> = common::run_interpreter(&src)
        .iter()
        .map(|l| clock_free(l))
        .collect();
    for multi in [true, false] {
        let c = compile_program(
            &prog,
            &Options {
                multi_memory: multi,
                ..Options::default()
            },
        )
        .unwrap();
        let o = cobolt_wasm::host::run_program(&c.wasm).unwrap();
        if o.trapped.is_some() || !o.errors.is_empty() {
            return Verdict::Differs(format!(
                "multi={multi}: trapped {:?} errors {:?}",
                o.trapped, o.errors
            ));
        }
        let got: Vec<String> = common::wasm_lines(&o)
            .iter()
            .map(|l| clock_free(l))
            .collect();
        if got != want {
            let at = got
                .iter()
                .zip(want.iter())
                .position(|(a, b)| a != b)
                .unwrap_or(got.len().min(want.len()));
            return Verdict::Differs(format!(
                "multi={multi}: line {}: module {:?}, interpreter {:?} ({} against {} lines)",
                at + 1,
                got.get(at),
                want.get(at),
                got.len(),
                want.len()
            ));
        }
    }
    Verdict::Identical(want.len())
}

#[test]
fn the_corpus_inside_the_subset_matches_the_interpreter() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/cobol");
    let mut files = Vec::new();
    programs(&root, &mut files);
    let (mut identical, mut excluded, mut lines) = (0, 0, 0);
    let mut differs: Vec<(String, String)> = Vec::new();
    for f in &files {
        let rel = f
            .strip_prefix(&root)
            .unwrap()
            .to_string_lossy()
            .replace('\\', "/");
        match judge(f) {
            Verdict::Identical(n) => {
                identical += 1;
                lines += n;
                eprintln!("  identical  {rel} ({n} lines)");
            }
            Verdict::Excluded(why) => {
                excluded += 1;
                eprintln!("  excluded   {rel}: {why}");
            }
            Verdict::Differs(why) => {
                eprintln!("  DIFFERS    {rel}: {why}");
                differs.push((rel, why));
            }
        }
    }
    eprintln!(
        "corpus: {} programs, {} compiled, {identical} identical ({lines} lines), {} differ, {excluded} excluded with the construct named",
        files.len(),
        identical + differs.len(),
        differs.len()
    );
    let unknown: Vec<_> = differs
        .iter()
        .filter(|(r, _)| !KNOWN.iter().any(|(k, _)| k == r))
        .collect();
    assert!(
        unknown.is_empty(),
        "differences with no WASM-PARITY.md row: {unknown:#?}"
    );
    let gone: Vec<_> = KNOWN
        .iter()
        .filter(|(k, _)| !differs.iter().any(|(r, _)| r == k))
        .collect();
    assert!(
        gone.is_empty(),
        "known differences that no longer differ (close their rows): {gone:?}"
    );
}
