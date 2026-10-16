// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! COMPUTE without its verb runs as COMPUTE: one program, written once with the
//! verb and once without, must display the same lines — and the lines must be
//! the right ones. Nothing in the runtime knows the second spelling exists.

use std::sync::mpsc;

use cobolt_lexer::{tokenize, SourceFormat};
use cobolt_parser::{parse, Severity};
use cobolt_runtime::Interpreter;

fn run_capture(src: &str) -> Vec<String> {
    let result = parse(tokenize(src, SourceFormat::Free));
    assert!(
        result.diagnostics.iter().all(|d| d.severity != Severity::Error),
        "parse errors: {:?}\n{src}",
        result.diagnostics
    );
    let program = result.program.expect("no program");
    let (_event_tx, event_rx) = mpsc::channel();
    let (state_tx, _state_rx) = mpsc::channel();
    let (display_tx, display_rx) = mpsc::channel();
    let mut interp = Interpreter::new_with_channels(program, event_rx, state_tx, display_tx);
    interp.run().expect("run failed");
    display_rx.try_iter().map(|s| s.trim().to_owned()).collect()
}

/// The program, with `{C}` where a statement opens: `COMPUTE ` or nothing.
const PROGRAM: &str = r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. SUGAR.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 A     PIC 9(4)   VALUE 0.
       01 B     PIC 9(4)   VALUE 7.
       01 C     PIC 9(4)   VALUE 6.
       01 R     PIC 9(2)V9 VALUE 0.
       01 N     PIC 9(3)   VALUE 0.
       01 SMALL PIC 9(2)   VALUE 0.
       01 I     PIC 9      VALUE 2.
       01 TBL.
          05 T  PIC 9(4)   OCCURS 3 VALUE 0.
       01 GRP.
          05 F  PIC 9(4)   VALUE 0.
       PROCEDURE DIVISION.
       MAIN.
           {C}A = B * C
           DISPLAY A
           {C}R ROUNDED = B / 4
           DISPLAY R
           {C}SMALL = B * C * 3
               ON SIZE ERROR
                   DISPLAY "SIZE"
               NOT ON SIZE ERROR
                   DISPLAY "FINE"
           END-COMPUTE
           DISPLAY SMALL
           {C}T(I) = A + 1
           {C}F OF GRP = T(I) + 1
           DISPLAY T(2) " " F OF GRP
           IF A > 40
               {C}N = A - 40
           ELSE
               {C}N = 0
           END-IF
           DISPLAY N
           DISPLAY A B
           {C}C = A + B
           DISPLAY C
           MOVE 1 TO N
           {C}N = N + 1
           PERFORM 3 TIMES
               {C}N = N * 2
           END-PERFORM
           DISPLAY N
           ADD 1 TO N ROUNDED
           {C}N = N + 1
           DISPLAY N
           STOP RUN.
"#;

fn program(verb: bool) -> String {
    PROGRAM.replace("{C}", if verb { "COMPUTE " } else { "" })
}

#[test]
fn a_program_without_the_verb_displays_what_the_program_with_it_does() {
    let with_verb = run_capture(&program(true));
    let without = run_capture(&program(false));
    assert_eq!(without, with_verb);
    // And they are the right lines, not two wrong ones that agree.
    assert_eq!(
        without,
        vec![
            "0042",          // A = B * C
            "018",           // R ROUNDED = B / 4 = 1.75 -> 1.8
            "SIZE",          // 126 does not fit PIC 9(2): ON SIZE ERROR
            "00",            // ... and the receiver is left as it was
            "0043 0044",     // T(I) = A + 1, F OF GRP = T(I) + 1
            "002",           // IF branch
            "00420007",      // DISPLAY A B ends in front of the next line, which is a new statement
            "0049",          // C = A + B
            "016",           // (1 + 1) * 2 * 2 * 2
            "018",           // ADD 1 TO N ROUNDED, then N = N + 1
        ]
    );
    println!(
        "\n  Assignment statement -- {} lines, the same with and without COMPUTE: receiver, ROUNDED, ON SIZE ERROR, subscript, qualified, IF/ELSE, PERFORM, after DISPLAY/MOVE/ADD lists\n",
        without.len()
    );
}

/// Spec 093 Q5: a control's member as the receiver. `Ctrl::Property = expr` is a `COMPUTE` whose receiver
/// is the property, and the runtime stores to it as it does for `COMPUTE`.
#[test]
fn a_controls_member_is_a_receiver_and_the_runtime_stores_to_it() {
    let program = |verb: &str| {
        format!(
            "       IDENTIFICATION DIVISION.\n       PROGRAM-ID. MEMBER.\n       PROCEDURE DIVISION.\n       MAIN.\n\
             \x20          {verb}NUD-1::Value = NUD-1::Value + 2\n\
             \x20          DISPLAY NUD-1::Value\n\
             \x20          {verb}NUD-1::Value ROUNDED = NUD-1::Value * 3\n\
             \x20          DISPLAY NUD-1::Value\n\
             \x20          STOP RUN.\n"
        )
    };
    let run = |src: String| {
        let parsed = parse(tokenize(&src, SourceFormat::Free));
        assert!(parsed.diagnostics.iter().all(|d| d.severity != Severity::Error), "{:?}\n{src}", parsed.diagnostics);
        let (_event_tx, event_rx) = mpsc::channel();
        let (state_tx, _state_rx) = mpsc::channel();
        let (display_tx, display_rx) = mpsc::channel();
        let mut interp = Interpreter::new_with_channels(parsed.program.expect("program"), event_rx, state_tx, display_tx);
        interp.seed_objects([("NUD-1".to_owned(), "NumericUpDown".to_owned(), vec![("Value".to_owned(), "5".to_owned())])]);
        interp.run().expect("the program runs");
        display_rx.try_iter().map(|s| s.trim().to_owned()).collect::<Vec<_>>()
    };
    let shorthand = run(program(""));
    assert_eq!(shorthand, run(program("COMPUTE ")), "the two spellings differ");
    assert_eq!(shorthand, ["7", "21"], "(5 + 2) then (7 * 3)");
}
