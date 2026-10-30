// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Spec 109 §5.1 (COBOL-2002 `FUNCTION-ID`): a function unit is called in an
//! expression, takes its arguments by value, answers its RETURNING item, may call
//! other functions and itself, and keeps no shared scratch between calls.

use std::sync::mpsc;
use std::time::Instant;

use cobolt_lexer::{tokenize, SourceFormat};
use cobolt_parser::{parse, Severity};
use cobolt_runtime::Interpreter;

const FUNCTIONS: &str = "
IDENTIFICATION DIVISION.
FUNCTION-ID. DOUBLE-AMOUNT.
DATA DIVISION.
LINKAGE SECTION.
01 ARG-AMOUNT PIC S9(5)V99.
01 RESULT-AMOUNT PIC S9(5)V99.
PROCEDURE DIVISION USING BY VALUE ARG-AMOUNT RETURNING RESULT-AMOUNT.
    COMPUTE RESULT-AMOUNT = ARG-AMOUNT * 2
    GOBACK.
END FUNCTION DOUBLE-AMOUNT.

IDENTIFICATION DIVISION.
FUNCTION-ID. QUADRUPLE.
DATA DIVISION.
LINKAGE SECTION.
01 ARG-A PIC S9(5)V99.
01 RES-A PIC S9(5)V99.
PROCEDURE DIVISION USING BY VALUE ARG-A RETURNING RES-A.
    COMPUTE RES-A = FUNCTION DOUBLE-AMOUNT(FUNCTION DOUBLE-AMOUNT(ARG-A))
    GOBACK.
END FUNCTION QUADRUPLE.

IDENTIFICATION DIVISION.
FUNCTION-ID. FACT.
DATA DIVISION.
LINKAGE SECTION.
01 ARG-N PIC 9(4).
01 RES-N PIC 9(9).
LOCAL-STORAGE SECTION.
01 LS-N PIC 9(4).
PROCEDURE DIVISION USING BY VALUE ARG-N RETURNING RES-N.
    IF ARG-N <= 1
        MOVE 1 TO RES-N
    ELSE
        COMPUTE LS-N = ARG-N - 1
        COMPUTE RES-N = ARG-N * FUNCTION FACT(LS-N)
    END-IF
    GOBACK.
END FUNCTION FACT.

IDENTIFICATION DIVISION.
FUNCTION-ID. GREET.
DATA DIVISION.
LINKAGE SECTION.
01 ARG-NAME PIC X(10).
01 RES-TEXT PIC X(20).
PROCEDURE DIVISION USING BY VALUE ARG-NAME RETURNING RES-TEXT.
    STRING \"Hello, \" DELIMITED BY SIZE ARG-NAME DELIMITED BY SPACE INTO RES-TEXT
    GOBACK.
END FUNCTION GREET.
";

fn run(procedure: &str) -> Result<Vec<String>, String> {
    let src = format!(
        "\
IDENTIFICATION DIVISION.
PROGRAM-ID. MAIN.
DATA DIVISION.
WORKING-STORAGE SECTION.
01 WS-A PIC S9(5)V99 VALUE 21.50.
01 WS-B PIC S9(5)V99.
01 WS-N PIC 9(4) VALUE 5.
01 WS-R PIC 9(9).
01 WS-T PIC X(20).
PROCEDURE DIVISION.
MAIN-PARA.
{procedure}
    STOP RUN.
{FUNCTIONS}"
    );
    let result = parse(tokenize(&src, SourceFormat::Free));
    assert!(
        result.diagnostics.iter().all(|d| d.severity != Severity::Error),
        "parse errors: {:?}",
        result.diagnostics
    );
    let program = result.program.expect("no program");
    let (_event_tx, event_rx) = mpsc::channel();
    let (state_tx, _state_rx) = mpsc::channel();
    let (display_tx, display_rx) = mpsc::channel();
    let mut interp = Interpreter::new_with_channels(program, event_rx, state_tx, display_tx);
    match interp.run() {
        Ok(_) => Ok(display_rx.try_iter().collect()),
        Err(e) => Err(e.to_string()),
    }
}

fn shown(procedure: &str) -> String {
    run(procedure).expect("the program runs").join("|").trim().to_string()
}

#[test]
fn user_functions_are_called_in_expressions() {
    let t0 = Instant::now();
    let cases: Vec<(&str, String, &str)> = vec![
        ("numeric result", shown("COMPUTE WS-B = FUNCTION DOUBLE-AMOUNT(WS-A)\nDISPLAY WS-B"), "0004300"),
        ("literal argument", shown("COMPUTE WS-B = FUNCTION DOUBLE-AMOUNT(10)\nDISPLAY WS-B"), "0002000"),
        ("expression argument", shown("COMPUTE WS-B = FUNCTION DOUBLE-AMOUNT(WS-A + 1)\nDISPLAY WS-B"), "0004500"),
        ("function in arithmetic", shown("COMPUTE WS-B = FUNCTION DOUBLE-AMOUNT(1) + FUNCTION DOUBLE-AMOUNT(2)\nDISPLAY WS-B"), "0000600"),
        ("function in a condition", shown("IF FUNCTION DOUBLE-AMOUNT(WS-A) = 43 DISPLAY \"yes\" END-IF"), "yes"),
        ("the caller's item is unchanged", shown("COMPUTE WS-B = FUNCTION DOUBLE-AMOUNT(WS-A)\nDISPLAY WS-A"), "0002150"),
        ("nested calls", shown("COMPUTE WS-B = FUNCTION QUADRUPLE(WS-A)\nDISPLAY WS-B"), "0008600"),
        ("recursion", shown("COMPUTE WS-R = FUNCTION FACT(WS-N)\nDISPLAY WS-R"), "000000120"),
        ("text result", shown("MOVE FUNCTION GREET(\"Ana\") TO WS-T\nDISPLAY WS-T"), "Hello, Ana"),
        ("called twice in one statement", shown("COMPUTE WS-R = FUNCTION FACT(3) + FUNCTION FACT(4)\nDISPLAY WS-R"), "000000030"),
    ];
    let mut failed = 0;
    for (name, got, want) in &cases {
        if got != want {
            failed += 1;
            eprintln!("FAIL {name}: got [{got}] want [{want}]");
        }
    }
    let refused: Vec<(&str, Result<Vec<String>, String>, &str)> = vec![
        ("too many arguments", run("COMPUTE WS-B = FUNCTION DOUBLE-AMOUNT(1 2)"), "takes 1 argument"),
        ("no arguments", run("COMPUTE WS-B = FUNCTION DOUBLE-AMOUNT()"), "takes 1 argument"),
    ];
    for (name, got, want) in &refused {
        match got {
            Err(e) if e.contains(want) => {}
            other => {
                failed += 1;
                eprintln!("FAIL {name}: got {other:?}, wanted an error containing [{want}]");
            }
        }
    }
    let total = cases.len() + refused.len();
    println!("================ test_user_functions ================");
    println!("cases: numeric/text results, literal and expression arguments, in arithmetic and conditions, BY VALUE isolation, nested calls, recursion (FACT 5), 2 arity refusals");
    println!("run: {total} cases in {} ms", t0.elapsed().as_millis());
    println!("tally: {} passed, {failed} failed", total - failed);
    assert_eq!(failed, 0);
}
