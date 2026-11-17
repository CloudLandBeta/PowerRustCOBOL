// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! The concatenation operator `&` (DESKTOP-WASM-001 T39c): `a & b` is the text of `a` followed by the
//! text of `b`, each as the interpreter displays it, wherever a text is read — a `MOVE`'s sender, a
//! `DISPLAY`, a comparison, a method's argument. PowerCloud builds a JSON request this way:
//! `'{"StackName":"' & TXT-STACK::Text::Trim() & '"}'`.

mod common;

fn program(stmts: &str) -> String {
    format!(
        "IDENTIFICATION DIVISION.\nPROGRAM-ID. CONCAT.\nDATA DIVISION.\nWORKING-STORAGE SECTION.\n\
         01 WS-NAME PIC X(6) VALUE \"Ana\".\n01 WS-NUM PIC 9(4) VALUE 42.\n01 WS-NEG PIC S9(3) VALUE -7.\n\
         01 WS-DEC PIC 9V99 VALUE 1.5.\n01 WS-SHORT PIC X(3).\n01 WS-OUT PIC X(40).\n\
         PROCEDURE DIVISION.\n{stmts}    STOP RUN.\n"
    )
}

#[test]
fn the_concatenation_operator_matches_the_interpreter() {
    let show = |e: &str| format!("    MOVE {e} TO WS-OUT\n    DISPLAY \"[\" WS-OUT \"]\"\n");
    let rows: Vec<(&str, String, &str)> = vec![
        ("two literals", show("\"ab\" & \"cd\""), "[abcd "),
        ("an item keeps its trailing spaces", show("WS-NAME & \"|\""), "[Ana   |"),
        ("a method's answer", show("WS-NAME::Trim() & \"|\""), "[Ana|"),
        ("a value method without parentheses", show("WS-NAME::Upper & \"!\""), "[ANA   !"),
        ("PowerCloud's request", show("'{\"StackName\":\"' & WS-NAME::Trim() & '\"}'"), "[{\"StackName\":\"Ana\"}"),
        ("three parts", show("\"a\" & \"b\" & \"c\""), "[abc "),
        ("an unsigned number", show("\"n=\" & WS-NUM"), "[n=42 "),
        ("a negative number", show("\"n=\" & WS-NEG"), "[n=-7 "),
        ("a decimal number", show("\"n=\" & WS-DEC"), "[n=1.50 "),
        ("a number literal", show("\"n=\" & 5"), "[n=5 "),
        ("a computed part", show("\"n=\" & WS-NUM + 1"), "[n=43 "),
        ("a figurative constant", show("\"x\" & SPACE & \"y\""), "[x y "),
        ("cut to a short receiver", "    MOVE \"abc\" & \"def\" TO WS-SHORT\n    DISPLAY \"[\" WS-SHORT \"]\"\n".to_string(), "[abc]"),
        ("in DISPLAY", "    DISPLAY \"<\" & WS-NAME::Trim() & \">\"\n".to_string(), "<Ana>"),
        (
            "in IF",
            "    IF WS-NAME::Trim() & \"!\" = \"Ana!\"\n        DISPLAY \"yes\"\n    ELSE\n        DISPLAY \"no\"\n    END-IF\n".to_string(),
            "yes",
        ),
        ("a method's argument", show("WS-NAME::Replace( \"Ana\", \"An\" & \"na\" )"), "[Anna"),
        ("in COMPUTE, the number the text spells", "    COMPUTE WS-NUM = \"1\" & \"2\"\n    DISPLAY WS-NUM\n".to_string(), "0012"),
    ];
    let t = std::time::Instant::now();
    let mut equal = 0;
    let mut expected = 0;
    for (name, stmts, want) in &rows {
        let out = common::same_output(&program(stmts));
        let line = out.first().cloned().unwrap_or_default();
        eprintln!("  {name:<36} {line:?}");
        equal += 1;
        if line.starts_with(want) {
            expected += 1;
        } else {
            eprintln!("    expected to start with {want:?}");
        }
    }
    eprintln!("== the_concatenation_operator_matches_the_interpreter (DESKTOP-WASM-001 T39c) ==");
    eprintln!(
        "rows {}: equal on both engines and both memory variants {equal}, as expected {expected}; {:.0} ms",
        rows.len(),
        t.elapsed().as_secs_f64() * 1000.0
    );
    assert_eq!(expected, rows.len(), "a row printed what it should not");
}
