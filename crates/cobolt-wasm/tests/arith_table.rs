// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! AC3 (R4): a table of arithmetic cases gives the interpreter's results on
//! the module, in both memory variants. The language has two rounding modes —
//! truncation and `ROUNDED` (half away from zero; the AST carries
//! `rounded: bool`, there is no `ROUNDED MODE` clause) — so every verb has a
//! row of each, on positive and negative operands; then `SIZE ERROR`, 18- and
//! 38-digit operands and `DIVIDE … REMAINDER`.

mod common;

fn program(stmts: &str) -> String {
    format!(
        "IDENTIFICATION DIVISION.\nPROGRAM-ID. AR.\nDATA DIVISION.\nWORKING-STORAGE SECTION.\n\
         01 A PIC S9(5)V999 VALUE 12.345.\n01 B PIC S9(5)V999 VALUE -7.006.\n01 C PIC S9(5)V999 VALUE 3.\n\
         01 R2 PIC S9(7)V99.\n01 R0 PIC S9(7).\n01 RQ PIC S9(5).\n01 RR PIC S9(5)V999.\n01 SMALL PIC S99.\n01 OUT2 PIC -(8)9.99.\n01 OUT0 PIC -(8)9.\n\
         01 BIG18A PIC S9(18) VALUE 999999999999999999.\n01 BIG18B PIC S9(18) VALUE -123456789012345678.\n01 BIG18R PIC S9(18).\n\
         01 BIG38A PIC S9(38) VALUE 12345678901234567890123456789012345678.\n01 BIG38B PIC S9(38) VALUE -2.\n01 BIG38R PIC S9(38).\n\
         01 F38 PIC S9(20)V9(18) VALUE 1.5.\n01 F38R PIC S9(20)V9(18).\n\
         PROCEDURE DIVISION.\n{stmts}    STOP RUN.\n"
    )
}

/// `(verb, rounding or topic, statements)`; each row displays what it computed.
fn rows() -> Vec<(&'static str, &'static str, String)> {
    let show2 = "    MOVE R2 TO OUT2\n    DISPLAY OUT2\n";
    let show0 = "    MOVE R0 TO OUT0\n    DISPLAY OUT0\n";
    let mut v = Vec::new();
    for (round, kw) in [("truncation", ""), ("ROUNDED", " ROUNDED")] {
        v.push((
            "ADD",
            round,
            format!("    ADD A B GIVING R2{kw}\n{show2}    ADD A TO B GIVING R0{kw}\n{show0}"),
        ));
        v.push(("SUBTRACT", round, format!("    SUBTRACT A FROM B GIVING R2{kw}\n{show2}    SUBTRACT B FROM A GIVING R0{kw}\n{show0}")));
        v.push(("MULTIPLY", round, format!("    MULTIPLY A BY B GIVING R2{kw}\n{show2}    MULTIPLY A BY C GIVING R0{kw}\n{show0}")));
        v.push(("DIVIDE", round, format!("    DIVIDE A BY B GIVING R2{kw}\n{show2}    DIVIDE B INTO A GIVING R0{kw}\n{show0}")));
        v.push(("COMPUTE", round, format!("    COMPUTE R2{kw} = A * B / C\n{show2}    COMPUTE R0{kw} = (A - B) * 1.5\n{show0}")));
    }
    v.push((
        "ADD",
        "SIZE ERROR",
        "    MOVE 0 TO SMALL\n    ADD 150 TO SMALL ON SIZE ERROR DISPLAY \"SIZE \" SMALL NOT ON SIZE ERROR DISPLAY \"OK \" SMALL END-ADD\n\
         \x20   ADD 5 TO SMALL ON SIZE ERROR DISPLAY \"SIZE \" SMALL NOT ON SIZE ERROR DISPLAY \"OK \" SMALL END-ADD\n\
         \x20   ADD 150 TO SMALL\n    DISPLAY \"CUT \" SMALL\n"
            .to_string(),
    ));
    v.push((
        "DIVIDE",
        "SIZE ERROR (by zero)",
        "    MOVE 0 TO C\n    MOVE 9 TO R0\n    DIVIDE A BY C GIVING R0 ON SIZE ERROR DISPLAY \"ZERO \" R0 END-DIVIDE\n    DIVIDE C INTO A GIVING R0 ON SIZE ERROR DISPLAY \"ZERO2 \" R0 END-DIVIDE\n".to_string(),
    ));
    v.push((
        "ADD",
        "18 digits",
        "    ADD BIG18A BIG18B GIVING BIG18R\n    DISPLAY BIG18R\n    ADD 1 TO BIG18A ON SIZE ERROR DISPLAY \"SIZE 18\" END-ADD\n    MULTIPLY BIG18B BY 2 GIVING BIG18R\n    DISPLAY BIG18R\n".to_string(),
    ));
    v.push((
        "MULTIPLY",
        "38 digits",
        "    COMPUTE BIG38A = 1234567890123456789 * 10000000000 * 1000000000 + 987654321\n    DISPLAY BIG38A\n\
         \x20   MULTIPLY BIG38A BY 8 GIVING BIG38R ON SIZE ERROR DISPLAY \"SIZE 38\" END-MULTIPLY\n    DISPLAY BIG38R\n\
         \x20   MULTIPLY BIG38A BY 9 GIVING BIG38R ON SIZE ERROR DISPLAY \"SIZE 39\" END-MULTIPLY\n    DISPLAY BIG38R\n\
         \x20   COMPUTE BIG38R = BIG38A / -3\n    DISPLAY BIG38R\n    SUBTRACT BIG38A FROM BIG38B GIVING BIG38R\n    DISPLAY BIG38R\n    COMPUTE F38R ROUNDED = F38 / 7\n    DISPLAY F38R\n"
            .to_string(),
    ));
    v.push((
        "DIVIDE",
        "REMAINDER",
        "    DIVIDE A BY C GIVING RQ REMAINDER RR\n    DISPLAY RQ \" \" RR\n    DIVIDE B BY C GIVING RQ REMAINDER RR\n    DISPLAY RQ \" \" RR\n\
         \x20   DIVIDE 17 INTO 100 GIVING RQ ROUNDED REMAINDER RR\n    DISPLAY RQ \" \" RR\n"
            .to_string(),
    ));
    v
}

#[test]
fn arithmetic_matches_the_interpreter() {
    let rows = rows();
    let mut equal = 0;
    let mut lines = 0;
    for (verb, topic, stmts) in &rows {
        let out = common::same_output(&program(stmts));
        eprintln!("  {verb:<9} {topic:<22} {out:?}");
        lines += out.len();
        equal += 1;
    }
    let verbs = ["ADD", "SUBTRACT", "MULTIPLY", "DIVIDE", "COMPUTE"];
    for verb in verbs {
        for round in ["truncation", "ROUNDED"] {
            assert!(
                rows.iter().any(|(v, r, _)| *v == verb && *r == round),
                "no row for {verb} {round}"
            );
        }
    }
    eprintln!("arithmetic table: {} rows, {equal} equal on both engines and both memory variants ({lines} lines)", rows.len());
}

/// `COMPUTE … / 0` with `ON SIZE ERROR` is a size error condition (COBOL-85:
/// division by zero), so the phrase runs and the receiver is left alone. The
/// interpreter stops with `DivisionByZero` instead (`WASM-PARITY.md` #18, a
/// 1.x.x fix); this pins the module's answer until the two agree.
#[test]
fn compute_division_by_zero_is_a_size_error() {
    let src = program(
        "    MOVE 0 TO C\n    MOVE 9 TO R0\n    COMPUTE R0 = A / C ON SIZE ERROR DISPLAY \"SIZE \" R0 NOT ON SIZE ERROR DISPLAY \"NO\" END-COMPUTE\n    DISPLAY \"AFTER \" R0\n",
    );
    for multi in [true, false] {
        let o = common::run_wasm(
            &src,
            &cobolt_wasm::Options {
                multi_memory: multi,
                ..cobolt_wasm::Options::default()
            },
        );
        assert_eq!(
            common::wasm_lines(&o),
            vec!["SIZE 0000009", "AFTER 0000009"],
            "multi={multi}"
        );
    }
    assert!(
        common::run_interpreter(&src).is_empty(),
        "the interpreter still stops here; when it is fixed, move this case into the table"
    );
}
