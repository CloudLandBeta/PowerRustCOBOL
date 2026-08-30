// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! A called program's own WORKING-STORAGE is its own — even when the caller
//! uses the same names.
//!
//! Every one of IC's eight final failures reduced to this one missing
//! primitive, each with its own proof in the ledger
//! (`build-the-activation-scope-primitive`). Three shapes are pinned here:
//! the private static counter (IC101A/IC102A), the counter chain with CANCEL
//! (IC203A/IC205A/IC206A), and the save-swap-restore through a colliding
//! work area (IC227A EXT-REC-TEST-01).

use std::sync::mpsc;

use cobolt_lexer::{tokenize, SourceFormat};
use cobolt_parser::{parse, Severity};
use cobolt_runtime::Interpreter;

fn run_capture(src: &str) -> Vec<String> {
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
    let (state_tx, _state_rx) = mpsc::channel();
    let (display_tx, display_rx) = mpsc::channel();
    let mut interp = Interpreter::new_with_channels(program, event_rx, state_tx, display_tx);
    interp.run().expect("run failed");
    display_rx.try_iter().map(|s| s.trim().to_owned()).collect()
}

fn field(out: &[String], name: &str) -> String {
    out.iter()
        .find_map(|l| {
            l.strip_prefix(&format!("{name}=["))
                .and_then(|r| r.strip_suffix(']'))
                .map(str::to_owned)
        })
        .unwrap_or_else(|| panic!("expected {name}=[…]; got {out:?}"))
}

/// CCVS85 **IC101A** CALL-TEST-05. The callee keeps `77 DN2` as a static call
/// counter; the caller declares a `DN2` of its own and resets it between
/// calls. The counter must survive the reset — it is not the caller's
/// variable.
const PRIVATE_COUNTER: &str = r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. PMAIN.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       77  DN1 PIC S9 VALUE ZERO.
       77  DN2 PIC S9 VALUE ZERO.
       PROCEDURE DIVISION.
       MAIN.
           CALL "PSUB" USING DN1
           CALL "PSUB" USING DN1
           CALL "PSUB" USING DN1
           CALL "PSUB" USING DN2
           MOVE 0 TO DN2
           CALL "PSUB" USING DN2
           DISPLAY "CNT=[" DN2 "]"
           STOP RUN.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. PSUB.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       77  DN2 PIC S9 VALUE ZERO.
       LINKAGE SECTION.
       77  DN1 PIC S9.
       PROCEDURE DIVISION USING DN1.
       S-MAIN.
           ADD 1 TO DN2
           MOVE DN2 TO DN1
           EXIT PROGRAM.
       END PROGRAM PSUB.
       END PROGRAM PMAIN.
"#;

#[test]
#[ignore = "KNOWN DEFECT until the activation-scope primitive lands: the \
            callee's counter IS the caller's variable. Ledger: \
            build-the-activation-scope-primitive."]
fn a_callees_static_counter_is_not_the_callers_variable() {
    let out = run_capture(PRIVATE_COUNTER);
    assert_eq!(
        field(&out, "CNT"),
        "5",
        "five calls happened, so the callee's private counter hands back 5 \
         whatever the caller did to its own DN2 in between"
    );
}

/// CCVS85 **IC203A** CNCL-TEST-05, the full chain. The counters were measured
/// correct at 1.62.100 — DN2 provably held 3 and then 1 — and both IFs still
/// misdecided, because the callee's `77 DN2 COMP` is really the caller's
/// `PIC XXX` child and the comparison ran alphanumeric.
const COUNTER_CHAIN: &str = r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. CMAIN.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       77  DN1 PICTURE S999.
       01  TABLE-1.
           02  DN2 PICTURE XXX.
           02  DN3 PICTURE 99.
           02  DN4 PICTURE X(5).
       01  TABLE-2.
           02  DN6 PICTURE X OCCURS 2 TIMES.
       PROCEDURE DIVISION.
       MAIN.
           MOVE SPACE TO DN2, DN4, TABLE-2
           MOVE ZERO TO DN1
           CALL "SUB5" USING TABLE-1, TABLE-2, DN1
           DISPLAY "T2=[" TABLE-2 "]"
           STOP RUN.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. SUB5.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       77  DN2 PICTURE S9(8) USAGE COMP VALUE ZERO.
       LINKAGE SECTION.
       01  TABLE-1.
           02  T-DN1 PIC XXX.
           02  T-DN2 PIC 99.
           02  T-DN3 PIC X(5).
       77  DN1 PICTURE S999.
       01  TABLE-2.
           02  TV-1 PIC X.
           02  TV-2 PIC X.
       PROCEDURE DIVISION USING TABLE-1, TABLE-2, DN1.
       S-MAIN.
           CALL "SUB6" USING DN2
           CALL "SUB6" USING DN2
           CALL "SUB6" USING DN2
           MOVE "X" TO TV-1
           IF DN2 EQUAL TO 3
               MOVE "A" TO TV-1
           END-IF
           CANCEL "SUB6"
           MOVE ZERO TO DN2
           CALL "SUB6" USING DN2
           IF DN2 NOT EQUAL TO 1
               MOVE "Y" TO TV-2
           ELSE
               MOVE "B" TO TV-2
           END-IF
           EXIT PROGRAM.
       END PROGRAM SUB5.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. SUB6.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       77  WS1 PICTURE S9(8) USAGE COMPUTATIONAL VALUE ZERO.
       LINKAGE SECTION.
       01  DN1 PICTURE S9(8) USAGE COMPUTATIONAL.
       PROCEDURE DIVISION USING DN1.
       S6-MAIN.
           ADD 1 TO WS1
           MOVE WS1 TO DN1
           EXIT PROGRAM.
       END PROGRAM SUB6.
       END PROGRAM CMAIN.
"#;

#[test]
#[ignore = "KNOWN DEFECT until the activation-scope primitive lands. Ledger: \
            build-the-activation-scope-primitive."]
fn the_cancel_counter_chain_decides_numerically() {
    let out = run_capture(COUNTER_CHAIN);
    assert_eq!(
        field(&out, "T2"),
        "AB",
        "the counter reaches exactly 3 and, after CANCEL, exactly 1 — both \
         comparisons must run against the callee's own COMP item"
    );
}

/// CCVS85 **IC227A** EXT-REC-TEST-01's save-swap-restore, reduced. The callee
/// saves the shared record into its own work area, overwrites the record from
/// its parameter, and hands the old record back — and its work area is
/// declared IDENTICALLY in the caller, so the save destroyed the caller's
/// argument before it was read.
const SAVE_SWAP_RESTORE: &str = r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. EMAIN.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01  SHARED-REC PIC X(6).
       01  WORK-AREA.
           02  W-1 PIC X(3).
           02  W-2 PIC X(3).
       01  HOLD-AREA PIC X(6).
       PROCEDURE DIVISION.
       MAIN.
           MOVE "******" TO SHARED-REC
           MOVE "ABC" TO W-1
           MOVE "DEF" TO W-2
           MOVE WORK-AREA TO HOLD-AREA
           CALL "ESUB" USING WORK-AREA SHARED-REC
           DISPLAY "REC=[" SHARED-REC "]"
           DISPLAY "BACK=[" WORK-AREA "]"
           STOP RUN.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. ESUB.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01  WORK-AREA.
           02  W-1 PIC X(3).
           02  W-2 PIC X(3).
       LINKAGE SECTION.
       01  L-WORK PIC X(6).
       01  L-REC  PIC X(6).
       PROCEDURE DIVISION USING L-WORK L-REC.
       S-MAIN.
           MOVE L-REC TO WORK-AREA
           MOVE L-WORK TO L-REC
           MOVE WORK-AREA TO L-WORK
           EXIT PROGRAM.
       END PROGRAM ESUB.
       END PROGRAM EMAIN.
"#;

#[test]
#[ignore = "KNOWN DEFECT until the activation-scope primitive lands. Ledger: \
            build-the-activation-scope-primitive."]
fn a_save_swap_restore_through_a_colliding_work_area() {
    let out = run_capture(SAVE_SWAP_RESTORE);
    assert_eq!(
        field(&out, "REC"),
        "ABCDEF",
        "the record must receive the argument's data; stars mean the callee's \
         save into its own work area destroyed the caller's argument first"
    );
    assert_eq!(
        field(&out, "BACK"),
        "******",
        "the old record travels back through the parameter"
    );
}

/// PAIRS WITH EVERYTHING ABOVE — the 1.62.83 lesson. A group parameter's
/// subordinates must KEEP resolving to the caller's storage: privacy is for
/// the callee's own declarations, never for what it was handed.
#[test]
fn a_group_parameters_fields_still_reach_the_caller() {
    let out = run_capture(
        r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. GMAIN.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01  GRP.
           02  F1 PIC X(3).
           02  F2 PIC X(3).
       PROCEDURE DIVISION.
       MAIN.
           MOVE "AAA" TO F1
           MOVE "BBB" TO F2
           CALL "GSUB" USING GRP
           DISPLAY "G=[" GRP "]"
           STOP RUN.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. GSUB.
       DATA DIVISION.
       LINKAGE SECTION.
       01  GRP.
           02  F1 PIC X(3).
           02  F2 PIC X(3).
       PROCEDURE DIVISION USING GRP.
       S-MAIN.
           MOVE "XYZ" TO F2
           EXIT PROGRAM.
       END PROGRAM GSUB.
       END PROGRAM GMAIN.
"#,
    );
    assert_eq!(field(&out, "G"), "AAAXYZ");
}

/// An edited LINKAGE parameter **edits**: the template belongs to the
/// description written through, and rides the binding onto the caller's slot
/// for exactly the length of the call.
///
/// CCVS85 **IC103A/IC235A** CALL-TEST-06-06. IC104A declares `EDITED-FIELD
/// PIC XXBX0X` over its caller's plain `ALPHA-EDITED PIC X(6)` — inside a
/// group whose trees differ in shape, so this also rides on the offset
/// pairing. `MOVE "ABCD" TO EDITED-FIELD` must store `AB C0D` where the
/// caller reads it. The alias sends the write to the caller's key, whose own
/// description correctly has no template — outside the call it is a plain
/// X(6) — so the parameter LENDS its template to the argument for the
/// activation, and takes it back at exit: the second assertion writes the
/// same slot after the call and must stay unedited.
const EDITED_THROUGH_THE_BINDING: &str = r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. EMAIN.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01  GROUP-02.
           02  NUM-ITEM     PIC S99.
           02  ALPHA-EDITED PIC X(6).
       PROCEDURE DIVISION.
       MAIN.
           MOVE 12 TO NUM-ITEM
           MOVE SPACE TO ALPHA-EDITED
           CALL "ESUB" USING GROUP-02
           DISPLAY "EDITED=[" ALPHA-EDITED "]"
           MOVE "WXYZ" TO ALPHA-EDITED
           DISPLAY "AFTER=[" ALPHA-EDITED "]"
           STOP RUN.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. ESUB.
       DATA DIVISION.
       LINKAGE SECTION.
       01  GRP-02.
           02  GRP-03.
               03  NUM-ITEM2     PIC S99.
               03  EDITED-FIELD  PIC XXBX0X.
       PROCEDURE DIVISION USING GRP-02.
       S-MAIN.
           MOVE "ABCD" TO EDITED-FIELD
           EXIT PROGRAM.
       END PROGRAM ESUB.
       END PROGRAM EMAIN.
"#;

#[test]
fn an_edited_linkage_parameter_edits_through_the_binding() {
    let out = run_capture(EDITED_THROUGH_THE_BINDING);
    assert_eq!(
        field(&out, "EDITED"),
        "AB C0D",
        "the parameter's PIC XXBX0X places its insertions before the write \
         crosses the alias"
    );
    assert_eq!(
        field(&out, "AFTER"),
        "WXYZ  ",
        "after the call the caller's item is a plain X(6) again — a template \
         left behind would edit the caller's own writes"
    );
}

/// The same argument passed BY CONTENT **and** BY REFERENCE in one CALL: the
/// reference write survives, and the content parameter is a genuine copy.
///
/// CCVS85 **IC225A** CALL-TEST-02-03: `CALL … USING CONTENT DN1 REFERENCE DN2
/// DN1 REFERENCE DN2`. The old treatment aliased the content parameter and
/// restored the argument's prior value at exit — which clobbered the +1 the
/// callee had sent through the REFERENCE binding of the very same item. A
/// content parameter now lives as a private copy on an activation key: no
/// restore exists to clobber anything, and a write through it reaches only
/// the copy, which is the clause's whole meaning.
const CONTENT_AND_REFERENCE_TWICE: &str = r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. DMAIN.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       77  DN1 PIC S99 VALUE 25.
       77  DN2 PIC S99 VALUE 10.
       PROCEDURE DIVISION.
       MAIN.
           CALL "DSUB" USING BY CONTENT DN1
                             BY REFERENCE DN2 DN1
           DISPLAY "D1=[" DN1 "]"
           DISPLAY "D2=[" DN2 "]"
           STOP RUN.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. DSUB.
       DATA DIVISION.
       LINKAGE SECTION.
       77  P-COPY PIC S99.
       77  P-TWO  PIC S99.
       77  P-ONE  PIC S99.
       PROCEDURE DIVISION USING P-COPY P-TWO P-ONE.
       S-MAIN.
           ADD 1 TO P-ONE
           SUBTRACT 4 TO-BE-DROPPED
           EXIT PROGRAM.
       END PROGRAM DSUB.
       END PROGRAM DMAIN.
"#;

#[test]
fn a_content_copy_does_not_clobber_a_reference_write_to_the_same_item() {
    // The SUBTRACT line above is deliberately absent from the real source —
    // build it here so the constant stays valid COBOL.
    let src = CONTENT_AND_REFERENCE_TWICE.replace(
        "           SUBTRACT 4 TO-BE-DROPPED\n",
        "           SUBTRACT 4 FROM P-TWO\n           MOVE 99 TO P-COPY\n",
    );
    let out = run_capture(&src);
    assert_eq!(
        field(&out, "D1"),
        "26",
        "the +1 through the REFERENCE binding must survive; +25 means a \
         content restore clobbered it, and 99 means the content write leaked"
    );
    assert_eq!(field(&out, "D2"), "06", "the reference write to DN2 lands");
}
