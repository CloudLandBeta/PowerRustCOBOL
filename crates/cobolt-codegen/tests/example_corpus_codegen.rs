// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! **Spec 056 R80 / AC37 — the example projects' generated COBOL does not move.**
//!
//! Responsive layout is host-side: no engine change made for it may alter what
//! the generator writes. Every `.cfrm` of `examples/PowerDemo3/forms` and
//! `examples/PowerChat/forms` is generated and compared byte-for-byte against
//! a tracked snapshot under `tests/golden/corpus/<project>/` — tracked here
//! because `examples/*/generated/` is a gitignored build artefact.
//!
//! `UPDATE_GOLDEN=1` rewrites the snapshots after a deliberate codegen change;
//! review the diff — an unreviewed regeneration defeats the guard.

use std::path::{Path, PathBuf};
use std::time::Instant;

fn repo() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn forms_in(dir: &Path) -> Vec<PathBuf> {
    fn walk(d: &Path, out: &mut Vec<PathBuf>) {
        let Ok(rd) = std::fs::read_dir(d) else { return };
        for e in rd.flatten() {
            let p = e.path();
            if p.is_dir() {
                walk(&p, out);
            } else if p.extension().and_then(|e| e.to_str()) == Some("cfrm") {
                out.push(p);
            }
        }
    }
    let mut out = Vec::new();
    walk(dir, &mut out);
    out.sort();
    out
}

#[test]
fn every_example_form_generates_its_snapshot() {
    let started = Instant::now();
    let update = std::env::var("UPDATE_GOLDEN").is_ok();
    let golden = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/golden/corpus");
    let (mut forms, mut bytes, mut lines, mut written) = (0usize, 0usize, 0usize, 0usize);
    let mut differences = Vec::new();
    for project in ["PowerDemo3", "PowerChat"] {
        let forms_dir = repo().join("examples").join(project).join("forms");
        for path in forms_in(&forms_dir) {
            let rel = path.strip_prefix(&forms_dir).unwrap().display().to_string();
            let form = cobolt_forms::load_form(&path)
                .unwrap_or_else(|e| panic!("{} must parse: {e}", path.display()));
            let cbl = cobolt_codegen::generate(&form);
            forms += 1;
            bytes += cbl.len();
            lines += cbl.lines().count();
            let file = golden
                .join(project)
                .join(format!("{}.cbl", rel.trim_end_matches(".cfrm").replace(['/', '\\'], "__")));
            if update {
                std::fs::create_dir_all(file.parent().unwrap()).unwrap();
                std::fs::write(&file, &cbl).unwrap();
                written += 1;
                continue;
            }
            let expected = std::fs::read_to_string(&file).unwrap_or_else(|_| {
                panic!("snapshot {} missing — capture it with UPDATE_GOLDEN=1", file.display())
            });
            if expected != cbl {
                let e: Vec<_> = expected.lines().collect();
                let g: Vec<_> = cbl.lines().collect();
                let first = e.iter().zip(g.iter()).position(|(a, b)| a != b).unwrap_or(e.len().min(g.len()));
                differences.push(format!(
                    "{project}/{rel}: first difference at line {}:\n    expected: {}\n    got:      {}",
                    first + 1,
                    e.get(first).unwrap_or(&"<end>"),
                    g.get(first).unwrap_or(&"<end>")
                ));
            }
        }
    }
    println!("── 056 example-corpus generated COBOL ────────────────────");
    println!("  projects   : PowerDemo3, PowerChat");
    println!("  forms      : {forms}");
    println!("  generated  : {lines} lines, {bytes} bytes");
    if update {
        println!("  WROTE      : {written} snapshots under {}", golden.display());
    } else {
        println!("  identical  : {}/{forms}", forms - differences.len());
    }
    println!("  elapsed    : {:.2} s", started.elapsed().as_secs_f32());
    assert!(
        differences.is_empty(),
        "generated COBOL of the example corpus changed:\n{}",
        differences.join("\n")
    );
}
