// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Spec 108 R3 — a `?` parameter of `COBOL::"PARTIQL"` is typed by the data
//! item's PICTURE, not by the kind of value its storage holds. A group `MOVE`
//! leaves characters in a `PIC 9` child, and such a key used to fail with
//! `94 ValidationException … argument n is a String`.
//!
//! Runs `tests/cobol/fileio/partiql-param-picture.cbl` on the interpreter, its
//! two indexed files in a scratch folder, and passes its result block through.
//! One result block (GOLDEN RULE #7): the program's.

#[path = "support/partiql_fixtures.rs"]
mod fx;

use std::sync::mpsc;
use std::time::Instant;

use cobolt_lexer::{tokenize, SourceFormat};
use cobolt_parser::{parse, Severity};
use cobolt_runtime::Interpreter;
use fx::Scratch;

const PROGRAM: &str = include_str!("../../../tests/cobol/fileio/partiql-param-picture.cbl");

#[test]
fn partiql_parameters_are_typed_by_picture() {
    let t0 = Instant::now();
    let dir = Scratch::new("param-picture");
    let src = PROGRAM
        .replace("partiql-param-quotes.idx", &dir.0.join("quotes.idx").to_string_lossy())
        .replace("partiql-param-seqs.idx", &dir.0.join("seqs.idx").to_string_lossy());
    let parsed = parse(tokenize(&src, SourceFormat::Free));
    assert!(parsed.diagnostics.iter().all(|d| d.severity != Severity::Error), "parse errors: {:?}", parsed.diagnostics);
    let (_etx, erx) = mpsc::channel();
    let (stx, _srx) = mpsc::channel();
    let (dtx, drx) = mpsc::channel();
    let mut interp = Interpreter::new_with_channels(parsed.program.expect("no program"), erx, stx, dtx);
    interp.run().expect("run");
    let out: Vec<String> = drx.try_iter().map(|l| l.trim_end().to_owned()).collect();
    println!();
    for l in &out {
        println!("{l}");
    }
    println!("(interpreter, {} ms in all)", t0.elapsed().as_millis());
    let failed: Vec<&String> = out.iter().filter(|l| l.starts_with("FAIL ")).collect();
    assert!(failed.is_empty(), "failing cases: {failed:#?}");
    assert!(out.iter().any(|l| l == "Cases exercised : 011 (9 forms + the load + the bulk phase)"), "the program did not run every case");
    assert!(out.iter().any(|l| l == "OVERALL RESULT  : PASS"), "the program did not pass");
}
