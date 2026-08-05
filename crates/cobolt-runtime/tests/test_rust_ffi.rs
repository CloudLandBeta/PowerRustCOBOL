// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Rust-FFI dispatch (spec 005 T10 / AC6): a `REPOSITORY` binding maps a COBOL
//! class to a Rust type, a `USAGE OBJECT REFERENCE` item (seeded from its VALUE)
//! holds a live Rust object, and `INVOKE obj "method"` / `obj::method()` calls
//! into the curated Rust bridge, marshaling arguments and results.

use std::sync::mpsc;

use cobolt_lexer::{tokenize, SourceFormat};
use cobolt_parser::{parse, Severity};
use cobolt_runtime::Interpreter;

fn run(src: &str) -> (Vec<String>, usize) {
    let result = parse(tokenize(src, SourceFormat::Free));
    assert!(
        result
            .diagnostics
            .iter()
            .all(|d| d.severity != Severity::Error),
        "parse errors: {:?}",
        result.diagnostics
    );
    let program = result.program.expect("no program");
    let (_event_tx, event_rx) = mpsc::channel();
    let (state_tx, _state_rx) = mpsc::channel();
    let (display_tx, display_rx) = mpsc::channel();
    let mut interp = Interpreter::new_with_channels(program, event_rx, state_tx, display_tx);
    interp.run().expect("run failed");
    let out = display_rx.try_iter().map(|s| s.trim().to_owned()).collect();
    (out, interp.rust_object_count())
}

#[test]
fn invoke_rust_string_len_and_uppercase() {
    let src = r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. FFIDEMO.
       ENVIRONMENT DIVISION.
       CONFIGURATION SECTION.
       REPOSITORY.
           CLASS RUST-STRING IS "Rust.String"
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 S USAGE IS OBJECT REFERENCE RUST-STRING VALUE "hello".
       01 N PIC 9(4).
       01 T PIC X(10).
       PROCEDURE DIVISION.
           INVOKE S "len" RETURNING N.
           DISPLAY N.
           INVOKE S "to_uppercase" RETURNING T.
           DISPLAY T.
           STOP RUN.
"#;
    let (out, live) = run(src);
    assert_eq!(out, vec!["0005".to_string(), "HELLO".to_string()]);
    assert!(
        live >= 1,
        "the Rust.String object should be live during the run"
    );
}

#[test]
fn invoke_with_using_argument_mutates() {
    let src = r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. FFIMUT.
       ENVIRONMENT DIVISION.
       CONFIGURATION SECTION.
       REPOSITORY.
           CLASS RUST-STRING IS "Rust.String"
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 S USAGE IS OBJECT REFERENCE RUST-STRING VALUE "ab".
       01 N PIC 9(4).
       PROCEDURE DIVISION.
           INVOKE S "push_str" USING "cde".
           INVOKE S "len" RETURNING N.
           DISPLAY N.
           STOP RUN.
"#;
    let (out, _live) = run(src);
    assert_eq!(out, vec!["0005".to_string()]); // "ab" + "cde" = 5 bytes
}
// NOTE: both the `INVOKE … RETURNING` form (tested above) and the inline
// `obj::method()` form are wired. Inline `::` as a **value operand** inside
// DISPLAY/MOVE/COMPUTE is covered by `test_inline_methodcall_009`
// (spec 009 R16 / AC9 — `DISPLAY S::len()`).

// ── Every shipped class gets a real object (spec 041 T12) ────────────────────

/// One row per shipped `CLASS RUST-*`: declare an item of it and check the
/// handle it receives.
///
/// The curated bridge only *constructs* a handful of types — `String`, the
/// integers, the floats, `bool`, `Vec`. Every other class used to fall through
/// to handle **0**, so a program declaring `RUST-HASHMAP` and `RUST-INSTANT`
/// gave both items the same dead id and neither had an object behind it. That
/// silence is the same family of defect spec 041 exists to remove, so this
/// asserts what the fix guarantees: **every** declared item gets a live, unique
/// handle, whether or not the bridge can build its type yet.
///
/// What a block then *does* with each class is compiled code, so it is proved
/// where it happens — `every_shipped_class_binds_inside_a_block` in
/// `cobolt-compiler` builds and runs a program that binds all of them.
#[test]
fn every_shipped_class_gets_a_live_unique_handle() {
    use cobolt_ast::rust_types::SHIPPED_RUST_TYPES;

    let mut repository = String::new();
    let mut items = String::new();
    let mut displays = String::new();
    for (class, path) in SHIPPED_RUST_TYPES {
        repository.push_str(&format!("           CLASS {class} IS \"{path}\"\n"));
        items.push_str(&format!(
            "       01 WS-{class} USAGE IS OBJECT REFERENCE {class}.\n"
        ));
        displays.push_str(&format!("           DISPLAY WS-{class}.\n"));
    }

    let src = format!(
        "       IDENTIFICATION DIVISION.\n\
         \x20      PROGRAM-ID. ALLCLASSES.\n\
         \x20      ENVIRONMENT DIVISION.\n\
         \x20      CONFIGURATION SECTION.\n\
         \x20      REPOSITORY.\n{repository}\
         \x20      DATA DIVISION.\n\
         \x20      WORKING-STORAGE SECTION.\n{items}\
         \x20      PROCEDURE DIVISION.\n{displays}\
         \x20          STOP RUN.\n"
    );

    let (out, live) = run(&src);
    assert_eq!(
        out.len(),
        SHIPPED_RUST_TYPES.len(),
        "every declared item should have been displayed"
    );

    let ids: Vec<i64> = out
        .iter()
        .map(|s| s.trim().parse().expect("a handle id is numeric"))
        .collect();
    for (id, (class, _)) in ids.iter().zip(SHIPPED_RUST_TYPES) {
        assert!(*id > 0, "{class} got handle {id} — no object behind it");
    }
    let mut unique = ids.clone();
    unique.sort_unstable();
    unique.dedup();
    assert_eq!(
        unique.len(),
        ids.len(),
        "two classes share a handle, so writing one would overwrite the other"
    );

    assert_eq!(
        live,
        SHIPPED_RUST_TYPES.len(),
        "each of the {} shipped classes should hold one live object",
        SHIPPED_RUST_TYPES.len()
    );
}
