// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Spec 109 §5.3: `Split`, `Map`, `Filter`, `SortByDescending`, `Take`, `Join`,
//! `Sum` and `Max` over a collection, with the expression-only lambda
//! `ITEM => …`. A collection is not a value, so `Split` keeps answering one part
//! wherever no collection is wanted (R38).

use std::sync::mpsc;
use std::time::Instant;

use cobolt_lexer::{tokenize, SourceFormat};
use cobolt_parser::{parse, Severity};
use cobolt_runtime::Interpreter;

fn run(procedure: &str) -> Result<Vec<String>, String> {
    let src = format!(
        "\
IDENTIFICATION DIVISION.
PROGRAM-ID. COLL.
DATA DIVISION.
WORKING-STORAGE SECTION.
01 WS-CSV  PIC X(40) VALUE \" b , a ,, c \".
01 WS-NUM  PIC X(40) VALUE \"3,10,7\".
01 WS-OUT  PIC X(60).
01 WS-TAB.
   05 WS-ROW OCCURS 3 TIMES PIC X(3).
PROCEDURE DIVISION.
MAIN.
{procedure}
    STOP RUN.
"
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
fn collection_methods_and_the_lambda_behave_as_specified() {
    let t0 = Instant::now();
    let cases: Vec<(&str, String, &str)> = vec![
        ("Map+Trim+Join", shown("DISPLAY WS-CSV::Split(\",\")::Map(ITEM => ITEM::Trim())::Join(\"/\")"), "b/a//c"),
        (
            "Filter drops the blank",
            shown("DISPLAY WS-CSV::Split(\",\")::Map(ITEM => ITEM::Trim())::Filter(ITEM => ITEM <> \" \")::Join(\"/\")"),
            "b/a/c",
        ),
        ("Sum", shown("DISPLAY WS-NUM::Split(\",\")::Sum()"), "20"),
        ("Max is numeric", shown("DISPLAY WS-NUM::Split(\",\")::Max()"), "10"),
        ("Take", shown("DISPLAY WS-NUM::Split(\",\")::Take(2)::Join(\"+\")"), "3+10"),
        (
            "SortByDescending",
            shown("DISPLAY WS-NUM::Split(\",\")::Map(ITEM => ITEM::Trim())::SortByDescending(ITEM => ITEM::Len())::Join(\",\")"),
            "10,3,7",
        ),
        ("Take 0 is empty", shown("DISPLAY WS-NUM::Split(\",\")::Take(0)::Sum()"), "0"),
        (
            "THROUGH a collection chain",
            shown("PERFORM VARYING EL THROUGH WS-NUM::Split(\",\")::Map(ITEM => ITEM::Trim())\n    DISPLAY EL\nEND-PERFORM"),
            "3|10|7",
        ),
        (
            "EXIT PERFORM leaves the loop early",
            shown("PERFORM VARYING EL THROUGH WS-NUM::Split(\",\")::Map(ITEM => ITEM::Trim())\n    IF EL = \"10\"\n        EXIT PERFORM\n    END-IF\n    DISPLAY EL\nEND-PERFORM"),
            "3",
        ),
        (
            "THROUGH an OCCURS table",
            shown("MOVE \"a\" TO WS-ROW(1)\nMOVE \"b\" TO WS-ROW(2)\nMOVE \"c\" TO WS-ROW(3)\nPERFORM VARYING EL THROUGH WS-ROW\n    DISPLAY EL\nEND-PERFORM"),
            "a  |b  |c",
        ),
        (
            "nested THROUGH",
            shown("PERFORM VARYING A THROUGH WS-NUM::Split(\",\")::Take(2)::Map(ITEM => ITEM::Trim())\n    PERFORM VARYING B THROUGH WS-NUM::Split(\",\")::Take(2)::Map(ITEM => ITEM::Trim())\n        DISPLAY A B\n    END-PERFORM\nEND-PERFORM"),
            "33|310|103|1010",
        ),
        // R38: outside a collection `Split` still answers one part.
        ("Split(sep)(2) unchanged", shown("DISPLAY WS-NUM::Split(\",\")(2)"), "10"),
        ("unsubscripted Split is part 1", shown("DISPLAY WS-NUM::Split(\",\")"), "3"),
    ];
    let mut failed = 0;
    for (name, got, want) in &cases {
        if got != want {
            failed += 1;
            eprintln!("FAIL {name}: got [{got}] want [{want}]");
        }
    }
    // The error cases of Q3, Q12 and R15.
    let errors: Vec<(&str, Result<Vec<String>, String>, &str)> = vec![
        ("Max of empty names the method", run("DISPLAY WS-NUM::Split(\",\")::Take(0)::Max()"), "Max of an empty collection"),
        ("negative Take is refused", run("DISPLAY WS-NUM::Split(\",\")::Take(-1)::Sum()"), "Take needs a count"),
        ("a collection is not a value", run("DISPLAY WS-NUM::Split(\",\")::Take(2)"), "not a value"),
    ];
    for (name, got, want) in &errors {
        match got {
            Err(e) if e.contains(want) => {}
            other => {
                failed += 1;
                eprintln!("FAIL {name}: got {other:?}, wanted an error containing [{want}]");
            }
        }
    }
    let total = cases.len() + errors.len();
    println!("================ test_collections ================");
    println!("cases: Map, Filter, Join, Sum, Max, Take, SortByDescending, Split (R38), PERFORM THROUGH (chain, EXIT PERFORM, OCCURS, nested), 3 error cases");
    println!("run: {total} cases in {} ms", t0.elapsed().as_millis());
    println!("tally: {} passed, {failed} failed", total - failed);
    assert_eq!(failed, 0);
}
