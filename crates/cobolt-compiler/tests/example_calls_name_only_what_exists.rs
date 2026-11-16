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
//!
//! A Web or Mobile project's Backend (`[files] backend`) adds its programs to
//! what its own files and its drivers (`tests/`) may call, never to what the
//! front end's Common Code and forms may call: see [`backend_names`].

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

/// `path` parsed with its COPY books expanded. `Err(false)`: it cannot be read;
/// `Err(true)`: it does not preprocess or parse.
fn parse_source(path: &Path) -> Result<Program, bool> {
    let src = cobolt_lexer::read_source_file(path).map_err(|_| false)?;
    let fmt = cobolt_lexer::SourceFormat::detect(&src);
    let tokens = match cobolt_lexer::preprocess_program(&src, path, fmt) {
        Some(exp) if exp.errors.is_empty() => cobolt_lexer::tokenize_expansion(&exp),
        Some(_) => return Err(true),
        None => cobolt_lexer::tokenize(&src, fmt),
    };
    cobolt_parser::parse(tokens).program.ok_or(true)
}

/// The PROGRAM-IDs, in capitals, of the programs in the project's Backend
/// (`[files] backend`, its `.cbl` and `.cob` files) and of the programs they
/// contain. The backend links every one of them into each other
/// (`cobolt-server` `backend.rs`: every other program rides inside each
/// exported one), and a driver under `tests/` is assembled with the programs it
/// calls, so a `CALL` from either reaches them. A `CALL` from the front end's
/// Common Code or a form does not reach the backend, so those files keep the
/// Common Code names only.
fn backend_names(project: &Path, backend: &[String]) -> HashSet<String> {
    fn walk(p: &Program, out: &mut HashSet<String>) {
        out.insert(p.identification.program_id.trim().to_ascii_uppercase());
        p.nested_programs.iter().for_each(|n| walk(n, out));
    }
    let mut out = HashSet::new();
    for rel in backend {
        let path = project.join(rel);
        if !path.extension().is_some_and(|x| x.eq_ignore_ascii_case("cbl") || x.eq_ignore_ascii_case("cob")) {
            continue;
        }
        if let Ok(program) = parse_source(&path) {
            walk(&program, &mut out);
        }
    }
    out
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
    let (mut backend_total, mut backend_sources) = (0usize, 0usize);
    let mut found: Vec<String> = Vec::new();
    let mut guards: Vec<String> = Vec::new();

    for project in &projects {
        let Some(known) = cobolt_compiler::common_code_names(project) else { continue };
        let label = project.file_name().unwrap().to_string_lossy().to_string();
        common_names += known.len();
        let view = cobolt_compiler::find_project_manifest(project)
            .and_then(|m| cobolt_compiler::project_manifest_view(&m).ok())
            .expect("a project whose Common Code names were read has a readable manifest");
        let backend = backend_names(project, &view.backend);
        backend_total += backend.len();
        let with_backend: HashSet<String> = known.union(&backend).cloned().collect();

        // The guard: with the backend's names a driver still has a wrong name reported, and the
        // front end's Common Code still has a backend name reported.
        if let Some(real) = backend.difference(&known).min() {
            let driver = format!(
                "       IDENTIFICATION DIVISION.\n       PROGRAM-ID. GUARD-DRIVER.\n       PROCEDURE DIVISION.\n           CALL \"{real}\"\n           CALL \"NO-SUCH-BACKEND-PROGRAM\"\n           GOBACK.\n"
            );
            let program = cobolt_parser::parse(cobolt_lexer::tokenize(&driver, cobolt_lexer::SourceFormat::detect(&driver)))
                .program
                .expect("the guard driver parses");
            let as_driver = analyse(&program, &with_backend);
            let as_front_end = analyse(&program, &known);
            let named = |found: &[(u32, String)], name: &str| found.iter().any(|(_, m)| m.starts_with(&format!("CALL \"{name}\"")));
            assert!(
                as_driver.len() == 1 && named(&as_driver, "NO-SUCH-BACKEND-PROGRAM"),
                "{label}: a driver's wrong CALL must still be reported, and only it: {as_driver:?}"
            );
            assert!(
                as_front_end.len() == 2 && named(&as_front_end, real),
                "{label}: the front end's CALL \"{real}\" must still be reported: {as_front_end:?}"
            );
            guards.push(format!("{label} (CALL \"{real}\" reaches the backend only from its own files and drivers)"));
        }

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
            let program = match parse_source(&cbl) {
                Ok(program) => program,
                Err(false) => continue,
                Err(true) => {
                    skipped += 1;
                    continue;
                }
            };
            // The front end's Common Code sees the Common Code names; everything else (the
            // backend's own files, the drivers) sees the backend's too.
            let rel = cbl.strip_prefix(project).unwrap_or(&cbl).to_string_lossy().replace('\\', "/");
            let names = if view.sources.iter().any(|s| s.replace('\\', "/") == rel) {
                &known
            } else {
                if !backend.is_empty() {
                    backend_sources += 1;
                }
                &with_backend
            };
            targets += literal_targets(&program);
            for (line, msg) in analyse(&program, names) {
                found.push(format!("{label}/{}:{line}: {msg}", cbl.file_name().unwrap().to_string_lossy()));
            }
            sources += 1;
        }
    }

    println!(
        "CALL-target check over the shipped examples: {} projects, {forms} form programs and {sources} sources \
         ({skipped} skipped: unreadable or unparsable), {common_names} Common Code names, \
         {backend_total} Backend program names (seen by {backend_sources} backend and driver sources), \
         {targets} literal CALL targets resolved, {} unresolved, {:.0} ms",
        projects.len(),
        found.len(),
        started.elapsed().as_secs_f64() * 1000.0
    );
    println!("guards passed: {} of {}: {}", guards.len(), guards.len(), guards.join("; "));
    assert!(forms > 60, "the example forms were found ({forms})");
    assert!(targets > 0, "the sweep saw literal CALL targets, so it can fail ({targets})");
    assert!(!guards.is_empty(), "a project with a Backend was found, so the guard ran");
    assert!(found.is_empty(), "CALLs that name nothing:\n{}", found.join("\n"));
}
