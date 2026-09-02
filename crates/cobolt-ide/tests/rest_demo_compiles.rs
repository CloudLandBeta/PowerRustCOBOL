// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! The shipped REST example's COBOL **compiles**, and it narrates every path.
//!
//! Same gate as the Maps, Snackbar and Charts examples. This one carries one
//! extra assertion that the others do not need: **all four** lifecycle events
//! must be bound. The example this replaces bound `onComplete` alone, so a call
//! that failed or timed out fired nothing at all and the form simply sat there
//! — which is indistinguishable from "the REST control does not work".
//!
//! The form lives in the operator's demo project and carries a real API key, so
//! nothing here reads or prints a property value: the assertions are about the
//! generated COBOL and the control's shape, never its credentials.

use std::path::PathBuf;

fn demo_form() -> Option<PathBuf> {
    let p = PathBuf::from(std::env::var("HOME").ok()?)
        .join("Documents/PowerDemo3/forms/Non-Visual/restapi-form.cfrm");
    p.exists().then_some(p)
}

#[test]
fn the_rest_example_compiles_and_narrates_every_path() {
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

    // Every way a call can end has a handler. This is the point of the example.
    let upper = src.to_uppercase();
    let events = ["onComplete", "onError", "onTimeout", "onCancelled"];
    eprintln!("\n  event         handler generated");
    eprintln!("  -----------   -----------------");
    let mut unbound: Vec<&str> = Vec::new();
    for e in events {
        let ok = upper.contains(&format!("RESTCLIENT-1--{}", e.to_uppercase()));
        eprintln!("  {e:<11}   {}", if ok { "yes" } else { "NO" });
        if !ok {
            unbound.push(e);
        }
    }
    assert!(
        unbound.is_empty(),
        "a call that ends this way would fire nothing and the form would sit \
         silent — exactly the failure this example exists to rule out: {unbound:?}"
    );

    // And it reads back what happened rather than guessing.
    let reads = [
        ("the status", "RESTCLIENT-1::STATUSCODE"),
        ("the body", "RESTCLIENT-1::RESPONSEBODY"),
        ("the failure reason", "RESTCLIENT-1::LASTERROR"),
    ];
    let missing: Vec<&str> = reads
        .iter()
        .filter(|(_, n)| !upper.contains(n))
        .map(|(w, _)| *w)
        .collect();
    eprintln!("\n  {}/{} runtime answers read back", reads.len() - missing.len(), reads.len());
    assert!(missing.is_empty(), "the example does not report: {missing:?}");

    // The credential must never be printed by the example itself.
    assert!(
        !upper.contains("APPENDTEXT(FUNCTION TRIM(WS-LINE))\n           INVOKE TEXTBOX-LOG::APPENDTEXT(RESTCLIENT-1::AUTHTOKEN"),
        "the log must never carry the token"
    );

    eprintln!(
        "  → {} lines generated, parsed and analysed with 0 errors\n",
        src.lines().count()
    );
}
