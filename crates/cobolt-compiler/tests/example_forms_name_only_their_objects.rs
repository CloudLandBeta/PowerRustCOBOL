// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! The receiver check (2026-10-02) against every form we ship: each example
//! form's generated program (the codegen goldens) is analysed with that form's
//! own objects — the set Run Form and Build now pass — and must come out with
//! no receiver error. This is what proves the check accepts everything real
//! programs do (`me`, `super`, `COBOL`, the form's own name, controls, toolbar
//! buttons, member chains, methods on data items) and refuses only names that
//! reach nothing. It found one such name in a shipped demo: `Lbl-Cfg`, whose
//! label had been deleted while its handler kept writing to it.

use std::path::{Path, PathBuf};

use cobolt_semantic::{analyze_with, AnalyzeOptions, Severity};

fn repo() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).ancestors().nth(2).unwrap().to_path_buf()
}

#[test]
fn every_example_form_names_only_its_own_objects() {
    let goldens = repo().join("crates/cobolt-codegen/tests/golden/corpus");
    let mut checked = 0usize;
    let mut receivers = 0usize;
    let mut found: Vec<String> = Vec::new();
    for project in ["PowerDemo3", "PowerChat"] {
        let mut files: Vec<PathBuf> = std::fs::read_dir(goldens.join(project))
            .unwrap()
            .flatten()
            .map(|e| e.path())
            .filter(|p| p.extension().is_some_and(|x| x == "cbl"))
            .collect();
        files.sort();
        for cbl in files {
            let stem = cbl.file_stem().unwrap().to_string_lossy().replace("__", "/");
            let cfrm = repo().join("examples").join(project).join("forms").join(format!("{stem}.cfrm"));
            let Ok(xml) = std::fs::read_to_string(&cfrm) else { continue };
            let form = cobolt_forms::load_form_from_str(&xml).unwrap();
            let src = std::fs::read_to_string(&cbl).unwrap();
            receivers += src.matches("::").count();
            let fmt = cobolt_lexer::SourceFormat::detect(&src);
            let program = cobolt_parser::parse(cobolt_lexer::tokenize(&src, fmt))
                .program
                .unwrap_or_else(|| panic!("{} parses", cbl.display()));
            let sem = analyze_with(
                &program,
                &AnalyzeOptions {
                    known_objects: Some(cobolt_forms::toolbar::object_names(&form)),
                    ..Default::default()
                },
            );
            for d in sem.diagnostics.iter().filter(|d| d.severity == Severity::Error) {
                if d.message.contains("no control or object named") {
                    found.push(format!("{project}/{stem}:{}: {}", d.span.line, d.message));
                }
            }
            checked += 1;
        }
    }
    assert!(checked > 60, "the example forms were found ({checked})");
    assert!(found.is_empty(), "receivers that name nothing:\n{}", found.join("\n"));
    println!("receiver check over the shipped examples: {checked} form programs, {receivers} `::` uses, 0 unknown receivers");
}
