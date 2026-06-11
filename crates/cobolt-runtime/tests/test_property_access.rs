// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Integration tests for PowerCOBOL-style visual-object property access:
//!   `MOVE "Hello!" TO "Caption" OF CmStatic1`
//!   `MOVE "Caption" OF CmStatic1 TO "Text" OF "ListItems" (4) OF Listview1`
//! including direct property-to-property moves with no temporary data item
//! (type inference).

use std::sync::mpsc;

use cobolt_lexer::{tokenize, SourceFormat};
use cobolt_parser::{parse, Severity};
use cobolt_runtime::Interpreter;

/// Tokenize, parse (asserting no errors), run, and return captured DISPLAY lines.
fn run_capture(src: &str) -> Vec<String> {
    let tokens = tokenize(src, SourceFormat::Free);
    let result = parse(tokens);
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
    interp.run().expect("run failed");
    display_rx.try_iter().collect()
}

const SRC: &str = r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. T.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 WS-X PIC X(20).
       PROCEDURE DIVISION.
           MOVE "Hello!" TO "Caption" OF CmStatic1.
           MOVE "Caption" OF CmStatic1 TO WS-X.
           DISPLAY "X=[" WS-X "]".
           MOVE "Caption" OF CmStatic1
               TO "Text" OF "ListItems" (4) OF Listview1.
           MOVE "Text" OF "ListItems" (4) OF Listview1 TO WS-X.
           DISPLAY "Y=[" WS-X "]".
           STOP RUN.
"#;

#[test]
fn property_move_round_trip_and_type_inference() {
    let out = run_capture(SRC);
    let joined = out.join("\n");
    // literal -> property -> data item (no temp; type inferred)
    assert!(joined.contains("X=[Hello!"), "simple property round-trip failed: {out:?}");
    // property -> nested indexed property -> read back (no temp data item)
    assert!(joined.contains("Y=[Hello!"), "nested property round-trip failed: {out:?}");
}

const VERBS_SRC: &str = r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. T.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 WS-A PIC X(10) VALUE "World".
       PROCEDURE DIVISION.
      *> a property as receiver across several verbs (not just MOVE)
           COMPUTE "Value" OF S1 = 5 * 2.
           ADD 3 TO "Value" OF S1.
           SUBTRACT 1 FROM "Value" OF S1.
           MULTIPLY 2 BY "Value" OF S1.
           DISPLAY "arith=[" "Value" OF S1 "]".
      *> STRING with a property as both a source and the INTO receiver
           MOVE "Hi" TO "Text" OF Lbl1.
           STRING "Text" OF Lbl1 DELIMITED BY SPACE
                  WS-A DELIMITED BY SPACE
                  INTO "Text" OF Lbl1.
           DISPLAY "string=[" "Text" OF Lbl1 "]".
           STOP RUN.
"#;

#[test]
fn property_receiver_works_with_any_verb() {
    let out = run_capture(VERBS_SRC);
    let joined = out.join("\n");
    // ((5*2)+3-1)*2 = 24
    assert!(joined.contains("arith=[24]"), "arithmetic receivers failed: {out:?}");
    assert!(joined.contains("string=[HiWorld]"), "STRING INTO property failed: {out:?}");
}

#[test]
fn property_reference_does_not_warn_on_control_names() {
    // Control names in property references are form objects, not DATA DIVISION
    // items, so they must not produce "not declared" warnings.
    let tokens = tokenize(SRC, SourceFormat::Free);
    let result = parse(tokens);
    let program = result.program.expect("no program");
    let analysis = cobolt_semantic::analyze(&program);
    assert!(
        !analysis.diagnostics.iter()
            .any(|d| d.message.contains("CmStatic1") || d.message.contains("Listview1")),
        "unexpected diagnostic for a control name: {:?}",
        analysis.diagnostics
    );
}
