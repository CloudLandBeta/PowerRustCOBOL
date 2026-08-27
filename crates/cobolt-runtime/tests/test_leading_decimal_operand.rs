// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! A numeric literal that BEGINS with the decimal point, inside an operand list.
//!
//! COBOL-85 lets a numeric literal open with the decimal point — `.499` — and
//! requires a space after a sentence-ending period, so the two are told apart
//! by adjacency. The literal parser knew that; the operand lists of `ADD`,
//! `SUBTRACT` and `COMPUTE` did not, because they were bounded by
//! `Token::Period` directly:
//!
//! ```cobol
//!        SUBTRACT SUBTR-4 SUBTR-5 .499 FROM SUBTR-2 GIVING SUBTR-11.
//! ```
//!
//! stopped at the `.`, and `499 FROM …` was read as a new sentence — reported
//! as `unexpected token: IntegerLiteral(499)`, which points at the digits
//! rather than at the period that caused it. NC119A, NC175A, NC118A, NC177A,
//! NC201A and NC106A all rest on this.

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

fn arith(body: &str) -> Vec<String> {
    run_capture(&format!(
        "       IDENTIFICATION DIVISION.\n\
         \x20      PROGRAM-ID. LEADDEC.\n\
         \x20      DATA DIVISION.\n\
         \x20      WORKING-STORAGE SECTION.\n\
         \x20      01  A PIC 9(3)V999 VALUE 1.\n\
         \x20      01  B PIC 9(3)V999 VALUE 2.\n\
         \x20      01  C PIC 9(3)V999 VALUE 99.\n\
         \x20      01  D PIC 9(3)V999 VALUE 0.\n\
         \x20      PROCEDURE DIVISION.\n\
         \x20      MAIN.\n\
         {body}\
         \x20          DISPLAY \"D=\" D.\n\
         \x20          STOP RUN.\n"
    ))
}

/// NC119A's statement, computing the right answer: 99 − (1 + 2 + 0.499).
#[test]
fn subtract_takes_a_leading_decimal_literal_in_its_operand_list() {
    let out = arith("           SUBTRACT A B .499 FROM C GIVING D.\n");
    assert!(
        out.join("|").contains("095501"),
        "99 - (1 + 2 + .499) = 95.501, got {out:?}"
    );
}

/// The same shape on `ADD … GIVING`.
#[test]
fn add_takes_a_leading_decimal_literal_in_its_operand_list() {
    let out = arith("           ADD A B .5 GIVING D.\n");
    assert!(
        out.join("|").contains("003500"),
        "1 + 2 + .5 = 3.5, got {out:?}"
    );
}

/// And the plain two-operand form, which is what NC201A writes.
#[test]
fn add_a_leading_decimal_literal_to_a_receiver() {
    let out = arith("           MOVE 0 TO D.\n           ADD .3 TO D.\n");
    assert!(out.join("|").contains("000300"), "0 + .3 = .3, got {out:?}");
}

/// **The period must still end the sentence** when it is not glued to digits.
/// Without this the fix would swallow the next statement.
#[test]
fn a_real_sentence_period_still_terminates() {
    let out = arith(
        "           SUBTRACT A FROM C GIVING D.\n\
         \x20          ADD 1 TO D.\n",
    );
    // 99 - 1 = 98, then +1 = 99.
    assert!(
        out.join("|").contains("099000"),
        "both statements must run: {out:?}"
    );
}

/// A period followed by a space then digits is two sentences, not a literal —
/// adjacency is the whole rule.
#[test]
fn a_spaced_period_is_not_a_decimal_point() {
    let src = "       IDENTIFICATION DIVISION.\n\
               \x20      PROGRAM-ID. SPACED.\n\
               \x20      DATA DIVISION.\n\
               \x20      WORKING-STORAGE SECTION.\n\
               \x20      01  D PIC 9(3)V999 VALUE 0.\n\
               \x20      PROCEDURE DIVISION.\n\
               \x20      MAIN.\n\
               \x20          ADD 1 TO D.\n\
               \x20          DISPLAY \"D=\" D.\n\
               \x20          STOP RUN.\n";
    let out = run_capture(src);
    assert!(out.join("|").contains("001000"), "{out:?}");
}
