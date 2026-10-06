// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Spec 087 T18 — dynamic SQL and the SQL descriptor area on SQLite, through
//! `tests/cobol/esql/ac11_dynamic.cbl`.

use std::path::{Path, PathBuf};
use std::sync::mpsc;

use cobolt_lexer::{expand_copybooks, tokenize_expansion, SourceFormat};
use cobolt_parser::{parse, Severity};
use cobolt_runtime::Interpreter;

fn esql_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/cobol/esql")
}

fn run_file(name: &str) -> Vec<String> {
    let src = std::fs::read_to_string(esql_dir().join(name)).unwrap();
    let exp = expand_copybooks(&src, &esql_dir(), SourceFormat::Free);
    assert!(exp.errors.is_empty(), "{:?}", exp.errors);
    let result = parse(tokenize_expansion(&exp));
    let errors: Vec<_> = result.diagnostics.iter().filter(|d| d.severity == Severity::Error).collect();
    assert!(errors.is_empty(), "parse errors: {errors:?}");
    let (_event_tx, event_rx) = mpsc::channel();
    let (state_tx, _state_rx) = mpsc::channel();
    let (display_tx, display_rx) = mpsc::channel();
    let mut interp = Interpreter::new_with_channels(result.program.unwrap(), event_rx, state_tx, display_tx);
    interp.run().expect("run failed");
    let out: Vec<String> = display_rx.try_iter().map(|l| l.trim_end().to_string()).collect();
    println!("--- {name} ---\n{}", out.join("\n"));
    out
}

/// AC11 — PREPARE, DESCRIBE (too small, then enough), a cursor over the
/// prepared query read through pointers and inline, EXECUTE IMMEDIATE and
/// EXECUTE USING.
#[test]
fn ac11_dynamic_sql_through_the_descriptor() {
    let out = run_file("ac11_dynamic.cbl");
    let verdict = out.iter().rev().find(|l| l.starts_with("PASS ")).expect("no PASS/FAIL line");
    assert!(verdict.ends_with("FAIL 000"), "{}", out.join("\n"));
}
