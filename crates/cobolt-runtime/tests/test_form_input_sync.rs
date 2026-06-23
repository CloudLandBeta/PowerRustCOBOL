// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Regression test: a UI-driven value change (e.g. a dragged slider) must reach
//! the interpreter's object registry BEFORE the change handler runs, so a
//! handler reading `"Value" OF SLIDER-1` (here via `COBOL-GET-PROPERTY`) sees the
//! live value rather than the seeded default. Previously the value stayed at the
//! seeded `0` because `FormEvent` carried no value and nothing synced the UI's
//! `ctrl_state` back into the interpreter.

use std::sync::mpsc;
use std::thread;

use cobolt_lexer::{tokenize, SourceFormat};
use cobolt_parser::{parse, Severity};
use cobolt_runtime::{FormEvent, Interpreter, StateUpdate};

/// A minimal form event loop: on each event, read SLIDER-1's `Value` and DISPLAY
/// it — exactly what a user's `onChange` handler would do.
const SRC: &str = r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. SLIDERTEST.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 COBOL-EVENT-ID    PIC X(30).
       01 COBOL-CONTROL-ID  PIC X(30).
       01 COBOL-QUIT        PIC 9 VALUE 0.
       01 WS-VAL            PIC X(10).
       PROCEDURE DIVISION.
       MAIN.
           PERFORM UNTIL COBOL-QUIT = 1
               CALL "COBOL-WAIT-EVENT"
                   USING COBOL-EVENT-ID COBOL-CONTROL-ID
               CALL "COBOL-GET-PROPERTY"
                   USING "SLIDER-1" "Value" WS-VAL
               DISPLAY WS-VAL
           END-PERFORM.
           STOP RUN.
"#;

#[test]
fn dragged_slider_value_reaches_change_handler() {
    let result = parse(tokenize(SRC, SourceFormat::Free));
    assert!(
        result.diagnostics.iter().all(|d| d.severity != Severity::Error),
        "parse errors: {:?}",
        result.diagnostics
    );
    let program = result.program.expect("no program");

    let (event_tx, event_rx) = mpsc::channel::<FormEvent>();
    let (input_tx, input_rx) = mpsc::channel::<StateUpdate>();
    let (state_tx, _state_rx) = mpsc::channel::<StateUpdate>();
    let (display_tx, display_rx) = mpsc::channel::<String>();

    let handle = thread::spawn(move || {
        let mut interp = Interpreter::new_with_channels(program, event_rx, state_tx, display_tx);
        interp.set_input_channel(input_rx);
        // Seed the slider with its designed default (0) — what the user sees as
        // the stale value before the fix.
        interp.seed_objects(vec![(
            "SLIDER-1".to_string(),
            "Slider".to_string(),
            vec![("Value".to_string(), "0".to_string())],
        )]);
        let _ = interp.run();
    });

    // Simulate the UI: the drag pushes the new value, THEN fires the change event
    // (the ordering the running form uses).
    input_tx.send(StateUpdate::new("SLIDER-1", "Value", "75")).unwrap();
    event_tx.send(FormEvent::change("SLIDER-1", "75")).unwrap();
    // Close the form so the event loop exits.
    event_tx.send(FormEvent::quit()).unwrap();

    handle.join().expect("interpreter thread panicked");

    let lines: Vec<String> = display_rx.try_iter().map(|l| l.trim().to_string()).collect();
    assert!(
        lines.iter().any(|l| l == "75"),
        "change handler should read the dragged value 75, got: {lines:?}",
    );
    assert!(
        !lines.iter().any(|l| l == "0"),
        "handler must never see the stale seeded 0, got: {lines:?}",
    );
}
