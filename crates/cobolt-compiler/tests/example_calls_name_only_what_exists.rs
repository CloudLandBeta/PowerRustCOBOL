// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! The CALL-target check (2026-10-08) against every project we ship: each
//! example form's generated program and each hand-written source is analysed
//! with the names Run Form, Build and the form check pass — the project's
//! Common Code — and must come out with no `CALL` that names nothing.
//!
//! This is what proves the check accepts what real programs do (a handler
//! calling a procedure its form contains, a call into Common Code, a paragraph
//! reached with `CALL`, the generated event loop's own calls) and refuses only
//! a name that reaches nothing. The count at the end is the number of literal
//! targets the run resolved, so a check that quietly saw none could not pass.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use cobolt_ast::expr::{Expr, Literal};
use cobolt_ast::program::{ProcedureBody, Program};
use cobolt_ast::stmt::Stmt;
use cobolt_semantic::{analyze_with, AnalyzeOptions, Severity};

fn repo() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).ancestors().nth(2).unwrap().to_path_buf()
}

/// Every file under `dir` whose extension is `ext`, skipping build output.
fn files_with(dir: &Path, ext: &str, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    for e in entries.flatten() {
        let p = e.path();
        let name = e.file_name().to_string_lossy().to_string();
        if p.is_dir() {
            if name != "target" && name != "generated" && name != ".git" {
                files_with(&p, ext, out);
            }
        } else if p.extension().is_some_and(|x| x == ext) {
            out.push(p);
        }
    }
}

/// The literal, non-built-in `CALL` targets in `program` and the programs it contains.
fn literal_targets(program: &Program) -> usize {
    let mut n = 0usize;
    let mut count = |s: &Stmt| {
        if let Stmt::Call { program: Expr::Literal(Literal::String(t), _), .. } = s {
            let up = t.trim().to_ascii_uppercase();
            if !up.starts_with("COBOL-") && !up.starts_with("COBOLT-") {
                n += 1;
            }
        }
    };
    let paras: Vec<&cobolt_ast::program::Paragraph> = match &program.procedure.body {
        ProcedureBody::Sections(secs) => secs.iter().flat_map(|s| s.paragraphs.iter()).collect(),
        ProcedureBody::Paragraphs(p) => p.iter().collect(),
    };
    for para in paras {
        para.stmts.iter().for_each(|s| s.walk(&mut count));
    }
    for nested in &program.nested_programs {
        n += literal_targets(nested);
    }
    n
}

fn analyse(program: &Program, known: &HashSet<String>) -> Vec<(u32, String)> {
    analyze_with(
        program,
        &AnalyzeOptions { known_programs: Some(known.clone()), ..Default::default() },
    )
    .diagnostics
    .into_iter()
    .filter(|d| d.severity == Severity::Error && d.message.starts_with("CALL \""))
    .map(|d| (d.span.line, d.message))
    .collect()
}

#[test]
fn every_example_call_names_something_that_exists() {
    let examples = repo().join("examples");
    let mut projects: Vec<PathBuf> = std::fs::read_dir(&examples)
        .unwrap()
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.is_dir())
        .collect();
    projects.sort();

    let started = std::time::Instant::now();
    let (mut forms, mut sources, mut skipped, mut targets, mut common_names) = (0usize, 0usize, 0usize, 0usize, 0usize);
    let mut found: Vec<String> = Vec::new();

    for project in &projects {
        let Some(known) = cobolt_compiler::common_code_names(project) else { continue };
        let label = project.file_name().unwrap().to_string_lossy().to_string();
        common_names += known.len();

        let mut cfrms = Vec::new();
        files_with(&project.join("forms"), "cfrm", &mut cfrms);
        cfrms.sort();
        for cfrm in cfrms {
            let Ok(xml) = std::fs::read_to_string(&cfrm) else { continue };
            let Ok(form) = cobolt_forms::load_form_from_str(&xml) else {
                skipped += 1;
                continue;
            };
            let src = cobolt_codegen::generate(&form);
            let fmt = cobolt_lexer::SourceFormat::detect(&src);
            let Some(program) = cobolt_parser::parse(cobolt_lexer::tokenize(&src, fmt)).program else {
                skipped += 1;
                continue;
            };
            targets += literal_targets(&program);
            for (line, msg) in analyse(&program, &known) {
                found.push(format!("{label}/{}:{line}: {msg}", cfrm.file_name().unwrap().to_string_lossy()));
            }
            forms += 1;
        }

        let mut cbls = Vec::new();
        files_with(project, "cbl", &mut cbls);
        cbls.sort();
        for cbl in cbls {
            let Ok(src) = cobolt_lexer::read_source_file(&cbl) else { continue };
            let fmt = cobolt_lexer::SourceFormat::detect(&src);
            let tokens = match cobolt_lexer::preprocess_program(&src, &cbl, fmt) {
                Some(exp) if exp.errors.is_empty() => cobolt_lexer::tokenize_expansion(&exp),
                Some(_) => {
                    skipped += 1;
                    continue;
                }
                None => cobolt_lexer::tokenize(&src, fmt),
            };
            let Some(program) = cobolt_parser::parse(tokens).program else {
                skipped += 1;
                continue;
            };
            targets += literal_targets(&program);
            for (line, msg) in analyse(&program, &known) {
                found.push(format!("{label}/{}:{line}: {msg}", cbl.file_name().unwrap().to_string_lossy()));
            }
            sources += 1;
        }
    }

    println!(
        "CALL-target check over the shipped examples: {} projects, {forms} form programs and {sources} sources \
         ({skipped} skipped: unreadable or unparsable), {common_names} Common Code names, \
         {targets} literal CALL targets resolved, {} unresolved, {:.0} ms",
        projects.len(),
        found.len(),
        started.elapsed().as_secs_f64() * 1000.0
    );
    assert!(forms > 60, "the example forms were found ({forms})");
    assert!(targets > 0, "the sweep saw literal CALL targets, so it can fail ({targets})");
    assert!(found.is_empty(), "CALLs that name nothing:\n{}", found.join("\n"));
}
