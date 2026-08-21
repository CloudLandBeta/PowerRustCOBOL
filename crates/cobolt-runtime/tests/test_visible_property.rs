// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! `SET <control>::VISIBLE TO 0 | 1` — the pair, from real COBOL.
//!
//! Reported by the operator (2026-08-20): hiding a control worked and showing
//! it again did not. Both halves travel the same road — `SET` is parsed as a
//! `MOVE`, `MOVE` to a `::` target is a member assignment, and a member
//! assignment publishes a `StateUpdate` the host applies — so this pins the
//! road itself: what the interpreter puts on the wire for each direction.

use std::sync::mpsc;

use cobolt_lexer::{tokenize, SourceFormat};
use cobolt_parser::{parse, Severity};
use cobolt_runtime::Interpreter;

/// Run `src` and return every published `(control, property, value)`.
fn updates(src: &str) -> Vec<(String, String, String)> {
    let result = parse(tokenize(src, SourceFormat::Free));
    assert!(
        result
            .diagnostics
            .iter()
            .all(|d| d.severity != Severity::Error),
        "parse errors: {:?}",
        result.diagnostics
    );
    let program = result.program.expect("no program");
    let (_event_tx, event_rx) = mpsc::channel();
    let (state_tx, state_rx) = mpsc::channel();
    let (display_tx, _display_rx) = mpsc::channel();
    let mut interp = Interpreter::new_with_channels(program, event_rx, state_tx, display_tx);
    interp.seed_objects(vec![(
        "SWITCH-1".to_owned(),
        "Control".to_owned(),
        vec![("Visible".to_owned(), "1".to_owned())],
    )]);
    interp.run().expect("run failed");
    drop(interp);
    state_rx
        .try_iter()
        .map(|u| (u.ctrl_id, u.prop, u.value))
        .collect()
}

/// Both directions must publish the value the host can act on: exactly `"0"`
/// to hide and something the host reads as truthy to show. The host's rule is
/// `value != "0" && value != "false"`, so a stray sign or padding on the ZERO
/// (`"+0"`, `" 0"`) silently means "visible" — which is why the value, not
/// just the fact of a write, is what this asserts.
#[test]
fn set_visible_publishes_both_directions() {
    let src = "\
IDENTIFICATION DIVISION.
PROGRAM-ID. VIS-TEST.
PROCEDURE DIVISION.
    SET SWITCH-1::VISIBLE TO 0
    SET SWITCH-1::VISIBLE TO 1
    STOP RUN.
";
    let sent = updates(src);
    let visible: Vec<&(String, String, String)> = sent
        .iter()
        .filter(|(_, prop, _)| prop.eq_ignore_ascii_case("VISIBLE"))
        .collect();

    assert_eq!(
        visible.len(),
        2,
        "both SETs must reach the UI, got {sent:?}"
    );
    assert_eq!(
        visible[0].2, "0",
        "hiding must publish exactly \"0\" — anything else reads as visible"
    );
    assert_ne!(
        visible[1].2, "0",
        "showing must publish a truthy value, got {:?}",
        visible[1].2
    );
    assert_ne!(
        visible[1].2, "false",
        "showing must publish a truthy value, got {:?}",
        visible[1].2
    );
}
