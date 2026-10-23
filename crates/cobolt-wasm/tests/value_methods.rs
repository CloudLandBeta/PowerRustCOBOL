// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Value methods of data items (WASM-010 T11, R7): every spelling of
//! `cobolt_ast::methods::DATA_ITEM_METHODS` gives the interpreter's answer on
//! the module, in both memory variants.

mod common;

fn program(stmts: &str) -> String {
    format!(
        "IDENTIFICATION DIVISION.\nPROGRAM-ID. VM.\nDATA DIVISION.\nWORKING-STORAGE SECTION.\n\
         01 WS-NAME PIC X(12) VALUE \"  Ana Maria \".\n01 WS-CSV PIC X(16) VALUE \"red,green,blue\".\n\
         01 WS-NUM PIC 9(4) VALUE 42.\n01 WS-OUT PIC X(24).\n01 WS-N PIC 9(4).\n\
         PROCEDURE DIVISION.\n{stmts}    STOP RUN.\n"
    )
}

/// `(spelling or case, statements)`: each prints what it computed.
fn rows() -> Vec<(&'static str, String)> {
    let show = |e: &str| format!("    MOVE {e} TO WS-OUT\n    DISPLAY \"[\" WS-OUT \"]\"\n");
    let count = |e: &str| format!("    MOVE {e} TO WS-N\n    DISPLAY WS-N\n");
    vec![
        ("Trim", show("WS-NAME::Trim")),
        ("UpperCase", show("WS-NAME::UpperCase")),
        ("ToUpperCase", show("WS-NAME::ToUpperCase")),
        ("Upper", show("WS-NAME::Upper")),
        ("LowerCase", show("WS-NAME::LowerCase")),
        ("ToLowerCase", show("WS-NAME::ToLowerCase")),
        ("Lower", show("WS-NAME::Lower")),
        ("Replace", show("WS-NAME::Replace(\"a\", \"o\")")),
        ("Len", count("WS-NAME::Len")),
        ("Length", count("WS-NAME::Length")),
        ("Split", show("WS-CSV::Split(\",\")")),
        // Beyond the eleven spellings: a chain, a computation, a number, DISPLAY.
        ("Trim::Upper", show("WS-NAME::Trim::Upper")),
        ("Len in COMPUTE", "    COMPUTE WS-N = WS-NAME::Trim::Len + 100\n    DISPLAY WS-N\n".to_string()),
        ("Len of a number", count("WS-NUM::Len")),
        ("DISPLAY Upper", "    DISPLAY WS-CSV::Upper\n".to_string()),
    ]
}

#[test]
fn every_value_method_spelling_matches_the_interpreter() {
    for m in cobolt_ast::methods::DATA_ITEM_METHODS {
        assert!(rows().iter().any(|(n, _)| n == m), "no row for {m}");
    }
    let rows = rows();
    let mut equal = 0;
    for (name, stmts) in &rows {
        let src = program(stmts);
        let out = common::same_output(&src);
        assert!(!out.is_empty(), "{name}: printed nothing");
        eprintln!("  {name:<16} {out:?}");
        equal += 1;
    }
    eprintln!("value methods: {} rows, {equal} equal on both engines and both memory variants ({} spellings)", rows.len(), cobolt_ast::methods::DATA_ITEM_METHODS.len());
}

#[test]
fn a_method_a_data_item_does_not_have_is_refused_naming_the_list() {
    let src = program("    MOVE WS-NAME::Reverse TO WS-OUT\n");
    let prog = common::parse(&src);
    let err = match cobolt_wasm::compile_program(&prog, &cobolt_wasm::Options::default()) {
        Ok(_) => panic!("compiled"),
        Err(e) => e.to_string(),
    };
    assert!(err.to_ascii_uppercase().contains("REVERSE") && err.contains("Trim, Upper, Lower, Replace, Len and Split"), "{err}");
}
