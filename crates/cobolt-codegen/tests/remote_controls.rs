// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! WASM-005 T9: the backend programs of a web form's bound DataGrids.
//!
//! Golden files pin every generated program and the generated `exports.toml`
//! (`tests/golden/backend/`); regenerate with `UPDATE_GOLDEN=1` and review the
//! diff. Every program must parse and pass the analyser with no error — the
//! backend refuses to load one that does not.

use cobolt_codegen::remote_controls::{fixtures, generate_backend, is_generated, GeneratedBackend};
use std::path::PathBuf;

fn generated() -> GeneratedBackend {
    let load = |path: &str| {
        assert_eq!(path, fixtures::CIDX_PATH);
        Ok(fixtures::customers_cidx("data/customers.idx"))
    };
    generate_backend(&fixtures::grid_forms(), &load).unwrap_or_else(|e| panic!("generation failed: {e:#?}"))
}

fn golden_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests").join("golden").join("backend")
}

#[test]
fn one_read_per_bound_grid_and_a_save_only_when_writable() {
    let g = generated();
    let ids: Vec<&str> = g.programs.iter().map(|p| p.program_id.as_str()).collect();
    assert_eq!(
        ids,
        vec![
            "RC-6-ORDERS-9-GRID-CUST-READ",
            "RC-6-ORDERS-9-GRID-CUST-SAVE",
            "RC-6-ORDERS-10-GRID-ITEMS-READ",
            "RC-6-ORDERS-10-GRID-ITEMS-SAVE",
            "RC-6-REPORT-11-GRID-TOTALS-READ",
        ],
        "the read-only REPORT grid has no SAVE"
    );
    for p in &g.programs {
        assert!(is_generated(&p.source), "{}: no banner", p.program_id);
        assert_eq!(p.file_name, format!("{}.cbl", p.program_id));
        assert_eq!(p.idempotent, p.program_id.ends_with("-READ"));
    }
    // The grid's BackendRoles; an unset one is any authenticated caller.
    assert_eq!(g.programs[0].roles, vec!["clerk".to_string(), "manager".to_string()]);
    assert!(g.programs[2].roles.is_empty());
    let exports = g.exports_toml();
    assert!(is_generated(&exports));
    assert_eq!(exports.matches("[[export]]").count(), 5);
    assert_eq!(exports.matches("public = false").count(), 5, "a generated endpoint is never public");
    assert!(!exports.contains("public = true"));
    println!("{} programs, exports.toml {} bytes", g.programs.len(), exports.len());
}

#[test]
fn every_generated_program_parses_and_analyses_clean() {
    let g = generated();
    for p in &g.programs {
        let parsed = cobolt_parser::parse(cobolt_lexer::tokenize(&p.source, cobolt_lexer::SourceFormat::Free));
        let errors: Vec<String> =
            parsed.diagnostics.iter().filter(|d| d.is_error()).map(|d| format!("line {}: {}", d.span.line, d.message)).collect();
        assert!(errors.is_empty(), "{} does not parse:\n{}\n{}", p.program_id, errors.join("\n"), p.source);
        let program = parsed.program.unwrap_or_else(|| panic!("{}: no program", p.program_id));
        assert!(program.identification.program_id.eq_ignore_ascii_case(&p.program_id));
        assert_eq!(program.procedure.using, vec!["RC-PARAMS".to_string()]);
        assert_eq!(program.procedure.returning.as_deref(), Some("RC-RESULT"));
        let sem: Vec<String> = cobolt_semantic::analyze(&program)
            .errors()
            .map(|d| format!("line {}: {}", d.span.line, d.message))
            .collect();
        assert!(sem.is_empty(), "{} fails the analyser:\n{}\n{}", p.program_id, sem.join("\n"), p.source);
        println!("  {:<34} {:>5} lines  parse 0 errors, analyser 0 errors", p.program_id, p.source.lines().count());
    }
}

#[test]
fn no_value_is_spliced_into_sql_text() {
    // R17 and the save's host variables: every value reaches a statement as a
    // host variable; the only SQL text is the design-time query and table.
    let g = generated();
    let save = g.programs.iter().find(|p| p.program_id == "RC-6-ORDERS-10-GRID-ITEMS-SAVE").unwrap();
    assert!(save.source.contains("SET NAME = :RC-V2:RC-I2"), "{}", save.source);
    assert!(save.source.contains("WHERE ID = :RC-K1"));
    assert!(save.source.contains("VALUES (:RC-V1:RC-I1, :RC-V2:RC-I2, :RC-V3:RC-I3, :RC-V4:RC-I4)"));
    assert!(!save.source.contains("STRING "), "no statement is built from text");
    let read = g.programs.iter().find(|p| p.program_id == "RC-6-ORDERS-10-GRID-ITEMS-READ").unwrap();
    assert!(read.source.contains("CONNECT TO 'SALES' AS RC-CONN"), "the BackendConfig name, never a connection string");
    assert!(read.source.contains("ORDER BY ID\n"), "the query's trailing ';' is dropped");
    assert!(!read.source.contains("ORDER BY ID;"));
}

#[test]
fn a_grid_the_backend_cannot_serve_is_refused_naming_it() {
    let mut forms = fixtures::grid_forms();
    // No connection name anywhere: refused, never a guess.
    forms[1].controls[1].properties.shift_remove("SqlConnection");
    // A writable indexed grid that does not show the key.
    if let cobolt_forms::BindingSourceDescriptor::IndexedFile { fields, .. } = &mut forms[0].data_bindings[0].source {
        fields.remove(0);
    }
    let load = |_: &str| Ok(fixtures::customers_cidx("data/customers.idx"));
    let errs = generate_backend(&forms, &load).unwrap_err();
    assert_eq!(errs.len(), 2, "{errs:#?}");
    assert!(errs[0].starts_with("ORDERS: DataGrid GRID-CUST: the key field CUST-ID"), "{}", errs[0]);
    assert!(errs[1].starts_with("REPORT: DataGrid GRID-TOTALS: DB-SALES names no backend connection"), "{}", errs[1]);
}

#[test]
fn generated_backend_programs_match_their_goldens() {
    let update = std::env::var("UPDATE_GOLDEN").is_ok();
    let dir = golden_dir();
    let g = generated();
    let mut files: Vec<(String, String)> = g.programs.iter().map(|p| (p.file_name.clone(), p.source.clone())).collect();
    files.push(("exports.toml".into(), g.exports_toml()));
    let mut compared = 0;
    for (name, text) in &files {
        let path = dir.join(name);
        if update {
            std::fs::create_dir_all(&dir).unwrap();
            std::fs::write(&path, text).unwrap();
            println!("  {name:<38} WRITTEN ({} bytes)", text.len());
            continue;
        }
        let golden = std::fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("missing golden {path:?} ({e}); run once with UPDATE_GOLDEN=1 and review"));
        if *text != golden {
            let line = text.lines().zip(golden.lines()).position(|(a, b)| a != b).map(|i| i + 1);
            panic!("{name} moved; first differing line {line:?}. If deliberate, regenerate with UPDATE_GOLDEN=1 and review.");
        }
        compared += 1;
        println!("  {name:<38} {:>6} bytes identical", text.len());
    }
    if !update {
        assert_eq!(compared, 6, "every generated file is compared");
    }
}
