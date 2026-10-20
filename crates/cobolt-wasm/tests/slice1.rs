// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! The first vertical slice: a program is compiled to a module, run on the
//! test host, and its output compared with the interpreter's.

mod common;
use common::*;

#[test]
fn display_a_literal() {
    let lines = same_output(
        "IDENTIFICATION DIVISION.\nPROGRAM-ID. HELLO.\nPROCEDURE DIVISION.\nMAIN.\n    DISPLAY \"HELLO, WORLD\".\n    STOP RUN.\n",
    );
    assert_eq!(lines, vec!["HELLO, WORLD"]);
}

#[test]
fn move_add_and_display() {
    let src = r#"
IDENTIFICATION DIVISION.
PROGRAM-ID. ARITH.
DATA DIVISION.
WORKING-STORAGE SECTION.
01 A PIC 9(4) VALUE 12.
01 B PIC S9(4)V99 VALUE -3.5.
01 C PIC X(10) VALUE "ABC".
01 D PIC 9(4).
PROCEDURE DIVISION.
MAIN.
    ADD 5 TO A.
    DISPLAY "A=" A.
    ADD A B GIVING D.
    DISPLAY "D=" D.
    MOVE "XYZ" TO C.
    DISPLAY "C=" C "!".
    COMPUTE B = A * 2 + 1.
    DISPLAY "B=" B.
    STOP RUN.
"#;
    let lines = same_output(src);
    assert_eq!(lines[0], "A=0017");
}

#[test]
fn if_and_perform() {
    let src = r#"
IDENTIFICATION DIVISION.
PROGRAM-ID. LOOPS.
DATA DIVISION.
WORKING-STORAGE SECTION.
01 I PIC 99 VALUE 0.
01 T PIC 999 VALUE 0.
PROCEDURE DIVISION.
MAIN.
    PERFORM VARYING I FROM 1 BY 1 UNTIL I > 5
        ADD I TO T
    END-PERFORM.
    IF T = 15
        DISPLAY "SUM OK " T
    ELSE
        DISPLAY "SUM BAD " T
    END-IF.
    PERFORM SHOW 3 TIMES.
    PERFORM SHOW THRU SHOW-EXIT.
    STOP RUN.
SHOW.
    DISPLAY "TICK".
SHOW-EXIT.
    EXIT.
"#;
    let lines = same_output(src);
    assert_eq!(lines.iter().filter(|l| *l == "TICK").count(), 4);
}

/// WASM-010 R15: the Build reports the module's size. This prints the figures
/// for the three programs above so a change that bloats the output is visible.
#[test]
fn module_sizes_are_reported() {
    let progs = [
        ("hello", "IDENTIFICATION DIVISION.\nPROGRAM-ID. H.\nPROCEDURE DIVISION.\nM.\n    DISPLAY \"HI\".\n    STOP RUN.\n"),
        (
            "loops",
            "IDENTIFICATION DIVISION.\nPROGRAM-ID. L.\nDATA DIVISION.\nWORKING-STORAGE SECTION.\n01 I PIC 99 VALUE 0.\n01 T PIC 999 VALUE 0.\nPROCEDURE DIVISION.\nM.\n    PERFORM VARYING I FROM 1 BY 1 UNTIL I > 5\n        ADD I TO T\n    END-PERFORM.\n    DISPLAY T.\n    STOP RUN.\n",
        ),
    ];
    for (name, src) in progs {
        for multi in [true, false] {
            let c = compile(src, &cobolt_wasm::Options { multi_memory: multi, ..Default::default() });
            println!(
                "SIZE {name:6} multi_memory={multi:5} module={:6} bytes  routines={:2} fns / {:5} bytes  blocks={}  data_pages={}",
                c.sizes.module_bytes, c.sizes.rt_functions, c.sizes.rt_bytes, c.sizes.blocks, c.sizes.data_pages
            );
            assert!(wasmparser::validate(&c.wasm).is_ok(), "{name}: the module does not validate");
        }
    }
}
