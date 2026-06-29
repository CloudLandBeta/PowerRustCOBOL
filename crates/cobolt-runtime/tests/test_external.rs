// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Run-unit-wide `EXTERNAL` data sharing (spec 005): an EXTERNAL item written by
//! one program activation is the same physical copy seen by another activation
//! in the same run unit, while a plain (non-EXTERNAL) item is private per
//! activation. Interpreters joined to one `ExternalStore` form a run unit.

use cobolt_ast::program::Program;
use cobolt_lexer::{tokenize, SourceFormat};
use cobolt_parser::{parse, Severity};
use cobolt_runtime::{new_external_store, Interpreter};

fn parse_prog(src: &str) -> Program {
    let result = parse(tokenize(src, SourceFormat::Free));
    assert!(
        result
            .diagnostics
            .iter()
            .all(|d| d.severity != Severity::Error),
        "parse errors: {:?}",
        result.diagnostics
    );
    result.program.expect("no program")
}

// Writer: sets the EXTERNAL item and a same-named non-EXTERNAL item.
const SRC_A: &str = r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. PA.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 WS-SHARED PIC 9(4) EXTERNAL.
       01 WS-LOCAL  PIC 9(4).
       PROCEDURE DIVISION.
           MOVE 1234 TO WS-SHARED.
           MOVE 5678 TO WS-LOCAL.
           STOP RUN.
"#;

// Reader: declares the same items; should see the run unit's EXTERNAL value.
const SRC_B: &str = r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. PB.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 WS-SHARED PIC 9(4) EXTERNAL.
       01 WS-LOCAL  PIC 9(4).
       PROCEDURE DIVISION.
           STOP RUN.
"#;

#[test]
fn external_item_is_registered() {
    let interp = Interpreter::new(parse_prog(SRC_A));
    let names = interp.env.external_names();
    assert!(
        names.contains("WS-SHARED"),
        "EXTERNAL item not registered: {names:?}"
    );
    assert!(
        !names.contains("WS-LOCAL"),
        "non-EXTERNAL item wrongly registered"
    );
}

#[test]
fn external_value_is_shared_across_run_unit() {
    let store = new_external_store();

    // Program A runs and writes both items.
    let mut a = Interpreter::with_external_store(parse_prog(SRC_A), store.clone());
    a.run().expect("A run failed");

    // Program B joins the same run unit: it adopts the EXTERNAL value A wrote…
    let mut b = Interpreter::with_external_store(parse_prog(SRC_B), store.clone());
    assert_eq!(
        b.env.get_i64("WS-SHARED"),
        Some(1234),
        "EXTERNAL not shared into B"
    );
    // …but the plain item is private — B keeps its own default, not A's 5678.
    assert_eq!(
        b.env.get_i64("WS-LOCAL"),
        Some(0),
        "non-EXTERNAL leaked across programs"
    );

    // Running B keeps the shared value (load at run start).
    b.run().expect("B run failed");
    assert_eq!(b.env.get_i64("WS-SHARED"), Some(1234));
}

#[test]
fn separate_run_units_do_not_share() {
    let store_a = new_external_store();
    let mut a = Interpreter::with_external_store(parse_prog(SRC_A), store_a);
    a.run().expect("A run failed");

    // A different store = a different run unit: B sees its own fresh default.
    let store_b = new_external_store();
    let b = Interpreter::with_external_store(parse_prog(SRC_B), store_b);
    assert_eq!(
        b.env.get_i64("WS-SHARED"),
        Some(0),
        "EXTERNAL leaked across run units"
    );
}
