// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! `PERFORM … VARYING`, from NIST CCVS85 NC201A (VI-112/117 6.20.4 GR10(d)).
//!
//! Three rules of the statement, none of which is about the loop body:
//!
//! * `WITH TEST AFTER` runs the body once before anything is tested, and then
//!   tests **innermost first**. The phrase was parsed and discarded, so every
//!   `VARYING` ran test-before — and NC201A PFM-TEST-F4-14, whose body assigns
//!   both loop variables, then augmented the outer one past its terminating
//!   value on every pass and never finished. That one program was the whole of
//!   the module's "timed out" column.
//! * When an `AFTER` condition becomes true its identifier is set back to its
//!   `FROM` value before the next level out is augmented, so an inner variable
//!   does not survive its own loop. The outermost one does.
//! * A subscripted `VARYING` identifier names whichever occurrence its
//!   subscript selects *now*. Resolving it once meant the augment wrote one
//!   fixed occurrence while the `UNTIL` — evaluated fresh — read another.

use std::sync::mpsc;

use cobolt_lexer::{tokenize, SourceFormat};
use cobolt_parser::{parse, Severity};
use cobolt_runtime::Interpreter;

fn run(src: &str) -> Vec<String> {
    let result = parse(tokenize(src, SourceFormat::Free));
    let errors: Vec<String> = result
        .diagnostics
        .iter()
        .filter(|d| d.severity == Severity::Error)
        .map(|d| format!("{}:{}: {}", d.span.line, d.span.col, d.message))
        .collect();
    assert!(errors.is_empty(), "parse errors: {errors:#?}");
    let program = result.program.expect("no program");
    let (_event_tx, event_rx) = mpsc::channel();
    let (state_tx, _state_rx) = mpsc::channel();
    let (display_tx, display_rx) = mpsc::channel();
    let mut interp = Interpreter::new_with_channels(program, event_rx, state_tx, display_tx);
    interp.run().expect("run failed");
    drop(interp);
    display_rx.try_iter().map(|s| s.trim_end().to_owned()).collect()
}

#[test]
fn test_after_varying_runs_the_body_once_and_tests_afterwards() {
    // NC201A PFM-TEST-F4-14. The body sets `P3` to 1 and `P2` to 99 — the two
    // values that satisfy both conditions — so exactly one pass runs. Under the
    // test-before order the outer variable was augmented from 1 to 0 every time
    // round and never *equalled* 1 at the point it was tested: an endless loop,
    // and the reason this program never reached its own report.
    //
    // A `CYCLES` guard keeps the failure a wrong number rather than a hung
    // test run.
    let out = run(
        r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. TESTAFT.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 P2 PIC S999.
       01 P3 PIC 9.
       01 P4 PIC S999V9.
       01 CYCLES PIC 9(4) VALUE 0.
       01 TBL.
          02 ROW OCCURS 4 TIMES.
             03 CELL OCCURS 20 TIMES PIC 99V9.
       PROCEDURE DIVISION.
       MAIN.
           MOVE 1 TO P4.
           MOVE 2 TO P3.
           MOVE 20 TO P2.
           PERFORM BODY-I THRU BODY-J WITH TEST AFTER
                   VARYING P3 FROM 2 BY -1 UNTIL P3 EQUAL TO 1
                     AFTER P2 FROM 2 BY 6 UNTIL P2 GREATER THAN 19.
           DISPLAY "CYCLES=" CYCLES.
           DISPLAY "CELL22=" CELL (2, 2).
           DISPLAY "P4=" P4.
           STOP RUN.
       BODY-I.
           ADD 1 TO CYCLES.
           IF CYCLES > 20 DISPLAY "RUNAWAY" STOP RUN.
           MULTIPLY P4 BY 10 GIVING CELL (P3, P2).
       BODY-J.
           ADD .5 TO P4.
           MOVE 1 TO P3.
           MOVE 99 TO P2.
"#,
    );
    assert_eq!(out, vec!["CYCLES=0001", "CELL22=100", "P4=0015"]);
}

#[test]
fn an_after_variable_is_reset_to_its_from_value_when_its_loop_ends() {
    // NC201A PFM-TEST-F4-3 and PFM-TEST-F4-4. Two `AFTER` levels below the
    // outer one: after the whole PERFORM the two inner variables read their
    // FROM values (10 and 3), while the outer keeps the value that ended it
    // (6). The loop was leaving all three at whatever ended them — 0 and 7 for
    // the inner two.
    let out = run(
        r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. AFTRESET.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 P3 PIC S999.
       01 P2 PIC S999.
       01 P11 PIC S999.
       01 HITS PIC 9(4) VALUE 0.
       PROCEDURE DIVISION.
       MAIN.
           PERFORM COUNT-IT
                   VARYING P3 FROM 2 BY 2 UNTIL P3 GREATER THAN 4
                     AFTER P2 FROM 10 BY -5 UNTIL P2 EQUAL TO 0
                     AFTER P11 FROM 3 BY 2 UNTIL P11 GREATER THAN 5.
           DISPLAY "P3=" P3 " P2=" P2 " P11=" P11 " HITS=" HITS.
           STOP RUN.
       COUNT-IT.
           ADD 1 TO HITS.
"#,
    );
    // 2 outer × 2 middle × 2 inner = 8 body executions.
    assert_eq!(out, vec!["P3=006 P2=010 P11=003 HITS=0008"]);
}

#[test]
fn a_subscripted_varying_identifier_follows_its_subscript() {
    // NC201A PFM-TEST-F4-24, "MANIPULATING SUBSCRIPTS": the body advances both
    // subscripts, so each pass augments the *next* occurrence, by the *next*
    // increment. The receiver's subscript was frozen at its initial value while
    // the UNTIL read the moving one, so the loop augmented A(1) to 150 and
    // tested A(2), A(3), … — all zero — for ever.
    let out = run(
        r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. VARYSUB.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 S1 PIC S9(5) COMP.
       01 S2 PIC S9(5) COMP.
       01 HITS PIC S9(5) COMP.
       01 FILLER-A.
          03 TBL-A PIC S9(3) COMP OCCURS 10.
          03 TBL-C PIC S9(3) COMP OCCURS 10.
       PROCEDURE DIVISION.
       MAIN.
           INITIALIZE FILLER-A.
           MOVE 1 TO S1.
           MOVE 1 TO S2.
           MOVE 10 TO TBL-C (1). MOVE 20 TO TBL-C (2).
           MOVE 30 TO TBL-C (3). MOVE 40 TO TBL-C (4).
           MOVE 50 TO TBL-C (5). MOVE 60 TO TBL-C (6).
           MOVE 70 TO TBL-C (7). MOVE 80 TO TBL-C (8).
           MOVE 0 TO HITS.
           PERFORM STEP-IT
                   VARYING TBL-A (S1) FROM 10 BY TBL-C (S2)
                   UNTIL TBL-A (S1) > 70.
           DISPLAY "A=" TBL-A (S1) " S1=" S1 " HITS=" HITS.
           DISPLAY "A1=" TBL-A (1) " A2=" TBL-A (2) " A3=" TBL-A (3)
                   " A4=" TBL-A (4).
           STOP RUN.
       STEP-IT.
           ADD 1 TO HITS.
           MULTIPLY 2 BY S2.
           ADD 1 TO S1.
"#,
    );
    assert_eq!(
        out,
        vec!["A=080 S1=00004 HITS=00003", "A1=010 A2=020 A3=040 A4=080"]
    );
}

#[test]
fn test_before_varying_is_unchanged() {
    // The ordinary form, pinned alongside: the body runs only while the
    // condition is false, and an already-satisfied condition runs it not at all.
    let out = run(
        r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. TESTBEF.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 I PIC 9(3).
       01 HITS PIC 9(3) VALUE 0.
       PROCEDURE DIVISION.
       MAIN.
           PERFORM VARYING I FROM 1 BY 1 UNTIL I > 5
               ADD 1 TO HITS
           END-PERFORM.
           DISPLAY "A HITS=" HITS " I=" I.
           MOVE 0 TO HITS.
           PERFORM VARYING I FROM 9 BY 1 UNTIL I > 5
               ADD 1 TO HITS
           END-PERFORM.
           DISPLAY "B HITS=" HITS " I=" I.
           MOVE 0 TO HITS.
           PERFORM WITH TEST AFTER VARYING I FROM 9 BY 1 UNTIL I > 5
               ADD 1 TO HITS
           END-PERFORM.
           DISPLAY "C HITS=" HITS " I=" I.
           STOP RUN.
"#,
    );
    // C is the same loop under TEST AFTER: the body runs once even though the
    // condition was already true. The variable is augmented only when the test
    // comes out false, so the one that ends the loop leaves it untouched — `I`
    // is still 9, where the test-before form (B) never ran the body at all.
    assert_eq!(
        out,
        vec!["A HITS=005 I=006", "B HITS=000 I=009", "C HITS=001 I=009"]
    );
}
