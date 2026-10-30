// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Spec 109 S8: the order and display format of a READ-ONLY bound grid. The form host
//! publishes `_BindingOrderBy` / `_BindingFormats` only for a read-only binding; the
//! interpreter applies them where it writes the grid's rows, and a grid without them
//! shows its rows exactly as before.

use std::sync::mpsc;
use std::time::Instant;

use cobolt_lexer::{tokenize, SourceFormat};
use cobolt_parser::{parse, Severity};
use cobolt_runtime::Interpreter;

fn rows_of(extra: &[(&str, &str)]) -> String {
    let src = "\
IDENTIFICATION DIVISION.
PROGRAM-ID. BV.
DATA DIVISION.
WORKING-STORAGE SECTION.
01 WS-T.
   05 WS-ROW OCCURS 4 TIMES.
      10 WS-NAME PIC X(6).
      10 WS-AMT  PIC 9(5)V99.
PROCEDURE DIVISION.
MAIN.
    MOVE \"ana\"   TO WS-NAME(1)
    MOVE 1234.5   TO WS-AMT(1)
    MOVE \"bruno\" TO WS-NAME(2)
    MOVE 99       TO WS-AMT(2)
    MOVE \"carla\" TO WS-NAME(3)
    MOVE 10000    TO WS-AMT(3)
    MOVE \"dora\"  TO WS-NAME(4)
    MOVE 99       TO WS-AMT(4)
    GRID-1::RefreshBinding()
    DISPLAY GRID-1::Rows
    STOP RUN.
";
    let result = parse(tokenize(src, SourceFormat::Free));
    assert!(result.diagnostics.iter().all(|d| d.severity != Severity::Error), "{:?}", result.diagnostics);
    let program = result.program.expect("program");
    let (_e, event_rx) = mpsc::channel();
    let (state_tx, _s) = mpsc::channel();
    let (display_tx, display_rx) = mpsc::channel();
    let mut interp = Interpreter::new_with_channels(program, event_rx, state_tx, display_tx);
    let mut props = vec![
        ("_BindingKind".to_string(), "CobolTable".to_string()),
        ("_BindingFields".to_string(), "WS-NAME\nWS-AMT".to_string()),
    ];
    props.extend(extra.iter().map(|(k, v)| (k.to_string(), v.to_string())));
    interp.seed_objects(vec![("GRID-1".to_string(), "DataGrid".to_string(), props)]);
    interp.run().expect("runs");
    display_rx.try_iter().collect::<Vec<_>>().join("")
}

#[test]
fn a_read_only_grid_is_ordered_and_formatted() {
    let t0 = Instant::now();
    let plain = rows_of(&[]);
    let cases: Vec<(&str, String, String)> = vec![
        ("no view options: the source's order and cells", rows_of(&[]), plain.clone()),
        (
            "descending by amount; the tie keeps the source order",
            rows_of(&[("_BindingOrderBy", "-WS-AMT")]),
            "carla\t10000.00\nana\t1234.50\nbruno\t99.00\ndora\t99.00".to_string(),
        ),
        (
            "ascending by name",
            rows_of(&[("_BindingOrderBy", "WS-NAME")]),
            "ana\t1234.50\nbruno\t99.00\ncarla\t10000.00\ndora\t99.00".to_string(),
        ),
        (
            "a display format on the amount",
            rows_of(&[("_BindingFormats", "WS-AMT=currency USD")]),
            "ana\t$1,234.50\nbruno\t$99.00\ncarla\t$10,000.00\ndora\t$99.00".to_string(),
        ),
        (
            "order then format: the order is of the numbers, not of the text",
            rows_of(&[("_BindingOrderBy", "-WS-AMT"), ("_BindingFormats", "WS-AMT=number")]),
            "carla\t10,000.00\nana\t1,234.50\nbruno\t99.00\ndora\t99.00".to_string(),
        ),
        (
            "an order on a column the grid lacks changes nothing",
            rows_of(&[("_BindingOrderBy", "NOPE")]),
            plain.clone(),
        ),
    ];
    let mut failed = 0;
    for (name, got, want) in &cases {
        if got != want {
            failed += 1;
            eprintln!("FAIL {name}:\n  got  [{got}]\n  want [{want}]");
        }
    }
    println!("================ test_binding_view ================");
    println!("cases: no options, descending numeric with tie, ascending text, currency format, order-then-format, unknown order column");
    println!("run: {} cases in {} ms", cases.len(), t0.elapsed().as_millis());
    println!("tally: {} passed, {failed} failed", cases.len() - failed);
    assert_eq!(failed, 0);
}
