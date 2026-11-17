// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! `FUNCTION CONCATENATE` with a number among its arguments (WASM-PARITY #74): each argument is its
//! text as the interpreter's value of it, so a number is spelled from its value — `-` when negative, no
//! leading zeros, its scale's decimals — and not from its picture (`n=42`, not `n=0042`).

mod common;

fn program(stmts: &str) -> String {
    format!(
        "IDENTIFICATION DIVISION.\nPROGRAM-ID. CONCATN.\nDATA DIVISION.\nWORKING-STORAGE SECTION.\n\
         01 WS-NAME PIC X(6) VALUE \"Ana\".\n01 WS-NUM PIC 9(4) VALUE 42.\n01 WS-NEG PIC S9(3) VALUE -7.\n\
         01 WS-DEC PIC 9V99 VALUE 1.5.\n01 WS-ZERO PIC 9(3) VALUE 0.\n01 WS-SMALL PIC V99 VALUE .05.\n\
         01 WS-OUT PIC X(40).\nPROCEDURE DIVISION.\n{stmts}    STOP RUN.\n"
    )
}

#[test]
fn function_concatenate_spells_a_number_as_the_interpreter() {
    let show = |args: &str| format!("    MOVE FUNCTION CONCATENATE( {args} ) TO WS-OUT\n    DISPLAY \"[\" WS-OUT \"]\"\n");
    let rows: Vec<(&str, String, &str)> = vec![
        ("an unsigned item", show("\"n=\" WS-NUM"), "[n=42 "),
        ("a negative item", show("\"n=\" WS-NEG"), "[n=-7 "),
        ("a decimal item keeps its scale", show("\"n=\" WS-DEC"), "[n=1.50 "),
        ("a zero item", show("\"n=\" WS-ZERO"), "[n=0 "),
        ("a fraction below one", show("\"n=\" WS-SMALL"), "[n=0.05 "),
        ("a number literal", show("\"n=\" 5"), "[n=5 "),
        ("two numbers side by side", show("WS-NUM WS-NEG"), "[42-7 "),
        ("texts unchanged: an item keeps its spaces", show("WS-NAME \"|\""), "[Ana   |"),
        ("texts unchanged: two literals", show("\"ab\" \"cd\""), "[abcd "),
    ];
    let t = std::time::Instant::now();
    let mut equal = 0;
    let mut expected = 0;
    for (name, stmts, want) in &rows {
        let out = common::same_output(&program(stmts));
        let line = out.first().cloned().unwrap_or_default();
        eprintln!("  {name:<42} {line:?}");
        equal += 1;
        if line.starts_with(want) {
            expected += 1;
        } else {
            eprintln!("    expected to start with {want:?}");
        }
    }
    eprintln!("== function_concatenate_spells_a_number_as_the_interpreter (WASM-PARITY #74) ==");
    eprintln!(
        "rows {}: equal on both engines and both memory variants {equal}, as expected {expected}; {:.0} ms",
        rows.len(),
        t.elapsed().as_secs_f64() * 1000.0
    );
    assert_eq!(expected, rows.len(), "a row printed what it should not");
}
