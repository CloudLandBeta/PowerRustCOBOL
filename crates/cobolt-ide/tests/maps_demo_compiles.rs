// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! The shipped Maps example's COBOL **compiles**, not merely parses as XML.
//!
//! `test_maps_demo_form.rs` in `cobolt-forms` proves the form file loads and is
//! structurally what it claims. That is a different question from whether the
//! handlers inside it are valid COBOL: a form whose `onComplete` does not
//! compile still loads perfectly, and the developer finds out at Run.
//!
//! So the whole program is generated from the form exactly as the IDE does it,
//! then lexed, parsed and analysed. A worked example is read as authoritative —
//! whatever it does, someone will copy — which is why its code is held to the
//! same gate as the developer's own.
//!
//! The file lives in the operator's demo project, not in this repository, so
//! the test SKIPS when it is absent: a fresh clone must not fail for missing
//! something it was never given.

use std::path::PathBuf;

fn demo_form() -> Option<PathBuf> {
    let p = PathBuf::from(std::env::var("HOME").ok()?)
        .join("Documents/PowerDemo3/forms/Inner-Forms/maps-demo.cfrm");
    p.exists().then_some(p)
}

#[test]
fn the_maps_example_generates_a_program_that_compiles() {
    let Some(path) = demo_form() else {
        eprintln!("PowerDemo3 not present — skipping");
        return;
    };
    let form = cobolt_forms::load_form(&path).expect("the example form must parse");
    let src = cobolt_codegen::generate(&form);

    let parsed = cobolt_parser::parse(cobolt_lexer::tokenize(
        &src,
        cobolt_lexer::SourceFormat::Free,
    ));
    let parse_errors: Vec<String> = parsed
        .diagnostics
        .iter()
        .filter(|d| d.is_error())
        .map(|d| format!("  line {}: {}", d.span.line, d.message))
        .collect();
    assert!(
        parse_errors.is_empty(),
        "the example's generated program does not parse:\n{}",
        parse_errors.join("\n")
    );
    let program = parsed.program.expect("a parse with no errors yields a program");

    let sem = cobolt_semantic::analyze(&program);
    let errors: Vec<String> = sem
        .errors()
        .map(|d| format!("  line {}: {}", d.span.line, d.message))
        .collect();
    assert!(
        errors.is_empty(),
        "the example's generated program has {} semantic error(s):\n{}",
        errors.len(),
        errors.join("\n")
    );

    // The two things the example exists to show, still in the code it
    // generates: the async answer is read from `ResponseBody`, and the road it
    // returns is traced with `AddRoute`.
    assert!(
        src.contains("MAP-1::ResponseBody"),
        "the example must read its async answer from ResponseBody"
    );
    assert!(
        src.contains("\"AddRoute\""),
        "the example must trace the route it received"
    );
    println!(
        "maps-demo generates {} lines of COBOL; 0 parse errors, 0 semantic errors",
        src.lines().count()
    );
}
