// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! A number that OPENS a line, where an operand was meant.
//!
//! The lexer classifies a number at the start of a line as a **level number** —
//! and it usually is one, which is why the classification exists. But a
//! statement whose operand list spills onto the next line puts an ordinary
//! integer in exactly that position:
//!
//! ```cobol
//!        SUBTRACT DNAME-1
//!                 1 FROM ERROR-COUNTER.
//!        WRITE PRINT-REC FROM X AFTER
//!                 000000000000000001 LINE.
//!        DISPLAY "…"
//!                 21 SPACE  35  I-DATA
//! ```
//!
//! The lexer cannot tell the two apart — a level number is only recognisable
//! from context — so the parser accepts both spellings wherever an *expression*
//! is expected. A real level number can never appear there: the DATA DIVISION
//! recognises its entries before any expression is parsed.

use std::sync::mpsc;

use cobolt_lexer::{tokenize, SourceFormat};
use cobolt_parser::{parse, Severity};
use cobolt_runtime::Interpreter;

fn run_capture(src: &str) -> Vec<String> {
    let result = parse(tokenize(src, SourceFormat::Free));
    let errs: Vec<&String> = result
        .diagnostics
        .iter()
        .filter(|d| d.severity == Severity::Error)
        .map(|d| &d.message)
        .collect();
    assert!(errs.is_empty(), "parse errors: {errs:?}");
    let program = result.program.expect("no program");
    let (_event_tx, event_rx) = mpsc::channel();
    let (state_tx, _state_rx) = mpsc::channel();
    let (display_tx, display_rx) = mpsc::channel();
    let mut interp = Interpreter::new_with_channels(program, event_rx, state_tx, display_tx);
    interp.run().expect("run failed");
    display_rx.try_iter().map(|s| s.trim().to_owned()).collect()
}

fn prog(body: &str) -> String {
    format!(
        "       IDENTIFICATION DIVISION.\n\
         \x20      PROGRAM-ID. CONTNUM.\n\
         \x20      DATA DIVISION.\n\
         \x20      WORKING-STORAGE SECTION.\n\
         \x20      01  DNAME-1 PIC S9(4) VALUE 10.\n\
         \x20      01  ERROR-COUNTER PIC S9(4) VALUE 30.\n\
         \x20      01  WS-R PIC S9(4) VALUE 0.\n\
         \x20      PROCEDURE DIVISION.\n\
         \x20      MAIN.\n\
         {body}\
         \x20          STOP RUN.\n"
    )
}

/// NC112A / NC107A: a `SUBTRACT` operand on the next line.
#[test]
fn a_subtract_operand_may_open_a_line() {
    let out = run_capture(&prog(
        "           SUBTRACT DNAME-1\n\
         \x20                   1 FROM ERROR-COUNTER.\n\
         \x20          DISPLAY \"R=\" ERROR-COUNTER.\n",
    ));
    // 30 - (10 + 1) = 19
    assert!(out.join("|").contains("0019"), "expected 19, got {out:?}");
}

/// NC109M: a `DISPLAY` operand on the next line.
#[test]
fn a_display_operand_may_open_a_line() {
    let out = run_capture(&prog(
        "           DISPLAY \"N=\"\n\
         \x20                   21.\n",
    ));
    assert!(out.join("|").contains("N=21"), "{out:?}");
}

/// NC177A / NC175A: an `ADD … GIVING` operand on the next line.
#[test]
fn an_add_operand_may_open_a_line() {
    let out = run_capture(&prog(
        "           ADD DNAME-1\n\
         \x20                   6 GIVING WS-R.\n\
         \x20          DISPLAY \"R=\" WS-R.\n",
    ));
    assert!(out.join("|").contains("0016"), "10 + 6 = 16, got {out:?}");
}

/// ST140A / ST144A: a `PERFORM VARYING … FROM … BY` bound on the next line.
#[test]
fn a_perform_varying_bound_may_open_a_line() {
    let out = run_capture(&prog(
        "           MOVE 0 TO WS-R.\n\
         \x20          PERFORM VARYING DNAME-1 FROM\n\
         \x20                  1 BY 1 UNTIL DNAME-1 > 4\n\
         \x20              ADD 1 TO WS-R\n\
         \x20          END-PERFORM.\n\
         \x20          DISPLAY \"R=\" WS-R.\n",
    ));
    assert!(out.join("|").contains("0004"), "four iterations, got {out:?}");
}

/// **A real level number is untouched.** The DATA DIVISION recognises its
/// entries before any expression is parsed, so accepting the spelling in
/// expression positions cannot disturb a declaration.
#[test]
fn a_real_level_number_still_declares_an_item() {
    let out = run_capture(
        "       IDENTIFICATION DIVISION.\n\
         \x20      PROGRAM-ID. LEVELS.\n\
         \x20      DATA DIVISION.\n\
         \x20      WORKING-STORAGE SECTION.\n\
         \x20      01  GRP.\n\
         \x20          05  A PIC 9(3) VALUE 7.\n\
         \x20          05  B PIC 9(3) VALUE 8.\n\
         \x20      PROCEDURE DIVISION.\n\
         \x20      MAIN.\n\
         \x20          DISPLAY \"A=\" A \" B=\" B.\n\
         \x20          STOP RUN.\n",
    );
    assert!(out.join("|").contains("A=007 B=008"), "{out:?}");
}
