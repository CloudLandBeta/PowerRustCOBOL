// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Spec 090 — `Expand()` and `Collapse()` on a Panel or a GroupBox, driven from
//! real COBOL.
//!
//! The interpreter cannot see the layout, so the two methods are writes of the
//! `Expanded` property: the host puts the write into the design the layout
//! reads. What is pinned here is the half that lives in the interpreter — each
//! method writes the right value to the right control, the property reads back
//! what was last written, and the parser takes `X::Expand()` for a call rather
//! than for a collection subscript.

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
        ("CARD-1".to_owned(), "GroupBox".to_owned(), vec![("Expanded".to_owned(), "0".to_owned())]),
        ("CARD-2".to_owned(), "Panel".to_owned(), vec![("Expanded".to_owned(), "0".to_owned())]),
    ]);
    interp.run().expect("run failed");
    (display_rx.try_iter().collect(), state_rx.try_iter().collect())
}

fn writes(ups: &[StateUpdate]) -> Vec<(String, String)> {
    ups.iter()
        .filter(|u| u.prop.eq_ignore_ascii_case("Expanded"))
        .map(|u| (u.ctrl_id.clone(), u.value.clone()))
        .collect()
}

#[test]
fn expand_and_collapse_write_expanded_on_the_control() {
    let (out, ups) = run(
        r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. T.
       PROCEDURE DIVISION.
           INVOKE CARD-1::Expand()
           IF CARD-1::Expanded IS true
               DISPLAY "after Expand: open"
           ELSE
               DISPLAY "after Expand: closed"
           END-IF
           INVOKE CARD-1::Collapse()
           IF CARD-1::Expanded IS true
               DISPLAY "after Collapse: open"
           ELSE
               DISPLAY "after Collapse: closed"
           END-IF
           INVOKE CARD-2::Expand()
           MOVE 0 TO CARD-2::Expanded
           STOP RUN.
"#,
    );
    let w = writes(&ups);
    eprintln!("\n  Expanded writes: {w:?}\n  display: {out:?}");
    // The interpreter writes a Boolean property as `true` / `false`.
    let (on, off) = ("true".to_owned(), "false".to_owned());
    assert_eq!(
        w,
        [
            ("CARD-1".to_owned(), on.clone()),
            ("CARD-1".to_owned(), off.clone()),
            ("CARD-2".to_owned(), on),
            ("CARD-2".to_owned(), off),
        ],
        "Expand() turns it on, Collapse() off, and a MOVE to the property does too"
    );
    assert_eq!(
        out,
        ["after Expand: open", "after Collapse: closed"],
        "the property reads back what was last written"
    );
}
