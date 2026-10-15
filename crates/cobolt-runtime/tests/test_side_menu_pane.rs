// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! `Collapse()` and `Open()` on the form's OWN SideMenu, driven from real COBOL.
//!
//! A child form folds and opens the rail with `super::<menu>::Collapse()` /
//! `::Open()` (spec 049 R44). The form that owns the SideMenu could not: its
//! `Collapse()` fell through to the Panel and GroupBox verb and wrote `Expanded`,
//! a property a SideMenu does not have, and its `Open()` was read as the verb
//! that opens an SQL database. Both now write `Collapsed`, the one property the
//! rail's state is — what the host turns into the shell's request. (A boolean
//! property travels as `true` / `false`.)

use std::sync::mpsc;

use cobolt_lexer::{tokenize, SourceFormat};
use cobolt_parser::{parse, Severity};
use cobolt_runtime::channels::StateUpdate;
use cobolt_runtime::Interpreter;

fn run(src: &str) -> (Vec<String>, Vec<StateUpdate>) {
    let result = parse(tokenize(src, SourceFormat::Free));
    assert!(
        result.diagnostics.iter().all(|d| d.severity != Severity::Error),
        "parse errors: {:?}",
        result.diagnostics
    );
    let program = result.program.expect("no program");
    let (_event_tx, event_rx) = mpsc::channel();
    let (state_tx, state_rx) = mpsc::channel();
    let (display_tx, display_rx) = mpsc::channel();
    let mut interp = Interpreter::new_with_channels(program, event_rx, state_tx, display_tx);
    interp.seed_objects(vec![
        ("SIDE-1".to_owned(), "SideMenu".to_owned(), vec![("Collapsed".to_owned(), "0".to_owned())]),
        // A Panel next to it: its own `Collapse()` must go on writing `Expanded`.
        ("CARD-1".to_owned(), "Panel".to_owned(), vec![("Expanded".to_owned(), "1".to_owned())]),
    ]);
    interp.run().expect("run failed");
    (display_rx.try_iter().collect(), state_rx.try_iter().collect())
}

fn writes(ups: &[StateUpdate], prop: &str) -> Vec<(String, String)> {
    ups.iter()
        .filter(|u| u.prop.eq_ignore_ascii_case(prop))
        .map(|u| (u.ctrl_id.clone(), u.value.clone()))
        .collect()
}

#[test]
fn collapse_and_open_write_collapsed_on_the_rail_and_a_property_write_does_too() {
    let (out, ups) = run(
        r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. T.
       PROCEDURE DIVISION.
           INVOKE SIDE-1::Collapse()
           IF SIDE-1::Collapsed IS true
               DISPLAY "after Collapse: folded"
           ELSE
               DISPLAY "after Collapse: open"
           END-IF
           INVOKE SIDE-1::Open()
           IF SIDE-1::Collapsed IS true
               DISPLAY "after Open: folded"
           ELSE
               DISPLAY "after Open: open"
           END-IF
           INVOKE SIDE-1::Collapse()
           MOVE 0 TO SIDE-1::Collapsed
           MOVE 1 TO SIDE-1::Collapsed
           STOP RUN.
"#,
    );
    let collapsed = writes(&ups, "Collapsed");
    let expected: Vec<(String, String)> = ["true", "false", "true", "false", "true"]
        .iter()
        .map(|v| ("SIDE-1".to_owned(), (*v).to_owned()))
        .collect();
    assert_eq!(collapsed, expected, "every verb and write puts Collapsed on the rail: {ups:?}");
    assert_eq!(out, vec!["after Collapse: folded".to_owned(), "after Open: open".to_owned()]);
    assert!(
        writes(&ups, "Expanded").is_empty(),
        "a SideMenu has no Expanded: {:?}",
        writes(&ups, "Expanded")
    );
    println!(
        "SideMenu own-form pane: Collapse/Open/Collapse + two property writes -> Collapsed writes {:?}",
        collapsed.iter().map(|(_, v)| v.as_str()).collect::<Vec<_>>()
    );
}

#[test]
fn a_panels_collapse_still_writes_expanded() {
    let (_out, ups) = run(
        r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. T.
       PROCEDURE DIVISION.
           INVOKE CARD-1::Collapse()
           STOP RUN.
"#,
    );
    assert_eq!(writes(&ups, "Expanded"), vec![("CARD-1".to_owned(), "false".to_owned())]);
    assert!(writes(&ups, "Collapsed").is_empty(), "a Panel has no Collapsed");
}
