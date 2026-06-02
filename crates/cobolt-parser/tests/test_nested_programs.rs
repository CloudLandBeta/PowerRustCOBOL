// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Tests: multi-program-unit source files.
//!
//! A COBOL source file can hold several program units in two shapes, both of
//! which the parser must collect into the first program's `nested_programs`
//! (the runtime dispatches them via one flat program registry):
//!
//!  1. **Sequential siblings** — each unit ends with its own `END PROGRAM name.`
//!  2. **True nesting** — inner units appear *before* the outer's `END PROGRAM`.

use cobolt_lexer::{tokenize, SourceFormat};
use cobolt_parser::{parse, Severity};

fn parse_ok(src: &str) -> cobolt_ast::program::Program {
    let result = parse(tokenize(src, SourceFormat::Free));
    assert!(
        result.diagnostics.iter().all(|d| d.severity != Severity::Error),
        "unexpected parse errors: {:?}",
        result.diagnostics
    );
    result.program.expect("no program returned")
}

#[test]
fn sequential_sibling_programs_are_collected() {
    // OUTER closes with its own END PROGRAM *before* SET-RESULT begins.
    let src = r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. OUTER.
       PROCEDURE DIVISION.
       MAIN.
           CALL "SET-RESULT".
           STOP RUN.
       END PROGRAM OUTER.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. SET-RESULT.
       PROCEDURE DIVISION.
           GOBACK.
       END PROGRAM SET-RESULT.
    "#;
    let prog = parse_ok(src);
    assert_eq!(prog.identification.program_id, "OUTER");
    assert_eq!(prog.nested_programs.len(), 1, "sibling SET-RESULT must be collected");
    assert_eq!(prog.nested_programs[0].identification.program_id, "SET-RESULT");
}

#[test]
fn three_sequential_siblings_are_all_collected() {
    let src = r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. OUTER.
       PROCEDURE DIVISION.
       MAIN.
           STOP RUN.
       END PROGRAM OUTER.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. A.
       PROCEDURE DIVISION.
           GOBACK.
       END PROGRAM A.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. B.
       PROCEDURE DIVISION.
           GOBACK.
       END PROGRAM B.
    "#;
    let prog = parse_ok(src);
    let ids: Vec<&str> = prog.nested_programs.iter()
        .map(|p| p.identification.program_id.as_str()).collect();
    assert_eq!(ids, vec!["A", "B"]);
}

#[test]
fn last_sibling_without_end_program_is_collected() {
    // The final unit omits its END PROGRAM terminator — still parsed.
    let src = r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. OUTER.
       PROCEDURE DIVISION.
       MAIN.
           STOP RUN.
       END PROGRAM OUTER.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. NO-TERM.
       PROCEDURE DIVISION.
           GOBACK.
    "#;
    let prog = parse_ok(src);
    assert_eq!(prog.nested_programs.len(), 1);
    assert_eq!(prog.nested_programs[0].identification.program_id, "NO-TERM");
}

#[test]
fn true_nested_program_before_terminator_still_works() {
    // Handler nested *inside* OUTER, before END PROGRAM OUTER (codegen shape).
    let src = r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. OUTER.
       PROCEDURE DIVISION.
       MAIN.
           STOP RUN.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. HANDLER.
       PROCEDURE DIVISION.
           GOBACK.
       END PROGRAM HANDLER.

       END PROGRAM OUTER.
    "#;
    let prog = parse_ok(src);
    assert_eq!(prog.identification.program_id, "OUTER");
    assert_eq!(prog.nested_programs.len(), 1);
    assert_eq!(prog.nested_programs[0].identification.program_id, "HANDLER");
    assert_eq!(prog.end_program_name.as_deref(), Some("OUTER"));
}
