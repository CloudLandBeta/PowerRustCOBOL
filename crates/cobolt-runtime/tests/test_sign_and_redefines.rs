// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! The `SIGN` clause, subordinate `REDEFINES` overlays and 66-level `RENAMES`
//! ranges — the rules the NIST CCVS85 Nucleus module measures in NC116A,
//! NC217A and NC252A.
//!
//! Each test is one rule of COBOL-85, written the way the suite writes it.

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
    display_rx
        .try_iter()
        .map(|s| s.trim_end().to_owned())
        .collect()
}

// ── SIGN IS … SEPARATE CHARACTER ────────────────────────────────────────────

/// `SIGN IS LEADING SEPARATE CHARACTER` reserves a character position of its
/// own, so the item is one wider than its digits and the position always holds
/// a literal `+` or `-` (NC116A SIG-TEST-GF-1).
#[test]
fn leading_separate_sign_occupies_its_own_position() {
    let out = run_capture(
        r#"
IDENTIFICATION DIVISION.
PROGRAM-ID. LEADSEP.
DATA DIVISION.
WORKING-STORAGE SECTION.
01 DS-LS-5 PICTURE S99999 SIGN IS LEADING SEPARATE CHARACTER VALUE +91275.
01 GRP-001 REDEFINES DS-LS-5.
   02 TEST1-AN-1 PICTURE X.
   02 TEST1-AN-5 PICTURE X(5).
01 DS-LS-4 PICTURE S9999 SIGN IS LEADING SEPARATE CHARACTER VALUE -9127.
01 GRP-002 REDEFINES DS-LS-4.
   02 TEST1N-AN-1 PICTURE X.
   02 TEST1N-AN-4 PICTURE X(4).
PROCEDURE DIVISION.
MAIN-PARA.
    DISPLAY "[" TEST1-AN-1 "]".
    DISPLAY "[" TEST1-AN-5 "]".
    DISPLAY "[" TEST1N-AN-1 "]".
    DISPLAY "[" TEST1N-AN-4 "]".
    STOP RUN.
"#,
    );
    assert_eq!(out, vec!["[+]", "[91275]", "[-]", "[9127]"]);
}

/// `SIGN IS TRAILING SEPARATE CHARACTER` puts the same position at the **end**
/// (NC116A SIG-TEST-GF-2).
#[test]
fn trailing_separate_sign_goes_after_the_digits() {
    let out = run_capture(
        r#"
IDENTIFICATION DIVISION.
PROGRAM-ID. TRAILSEP.
DATA DIVISION.
WORKING-STORAGE SECTION.
01 DS-TS-5 PICTURE S99999 SIGN IS TRAILING SEPARATE CHARACTER VALUE +80361.
01 GRP-003 REDEFINES DS-TS-5.
   02 TEST2-AN-5 PICTURE X(5).
   02 TEST2-AN-1 PICTURE X.
PROCEDURE DIVISION.
MAIN-PARA.
    DISPLAY "[" TEST2-AN-5 "]".
    DISPLAY "[" TEST2-AN-1 "]".
    STOP RUN.
"#,
    );
    assert_eq!(out, vec!["[80361]", "[+]"]);
}

/// A value that arrives **unsigned** still fills the sign position, with `+` —
/// the position is declared storage, not a marker that appears only when a
/// negative turns up (NC116A SIG-TEST-GF-15/GF-16).
#[test]
fn separate_sign_is_written_for_positive_senders_too() {
    let out = run_capture(
        r#"
IDENTIFICATION DIVISION.
PROGRAM-ID. SEPPOS.
DATA DIVISION.
WORKING-STORAGE SECTION.
01 DU-005 PICTURE 9(5) VALUE ZERO.
01 DS-005 PICTURE S9(5) VALUE 0.
01 WRK-DS-LS-5 PICTURE S99999 VALUE ZERO SIGN LEADING SEPARATE.
01 GRP-09 REDEFINES WRK-DS-LS-5 PICTURE X(6).
01 WRK-DS-TS-5 PICTURE S99999 VALUE ZERO SIGN TRAILING SEPARATE.
01 GRP-10 REDEFINES WRK-DS-TS-5 PICTURE X(6).
PROCEDURE DIVISION.
MAIN-PARA.
    MOVE 15759 TO DU-005.
    MOVE -15759 TO DS-005.
    MOVE DU-005 TO WRK-DS-LS-5.
    DISPLAY "[" GRP-09 "]".
    MOVE DS-005 TO WRK-DS-LS-5.
    DISPLAY "[" GRP-09 "]".
    MOVE DU-005 TO WRK-DS-TS-5.
    DISPLAY "[" GRP-10 "]".
    MOVE -15759 TO WRK-DS-TS-5.
    DISPLAY "[" GRP-10 "]".
    STOP RUN.
"#,
    );
    assert_eq!(out, vec!["[+15759]", "[-15759]", "[15759+]", "[15759-]"]);
}

/// A `SIGN` clause written on a **group** reaches every subordinate signed
/// numeric DISPLAY item that does not carry one of its own, and a nested group
/// overrides it for its own subtree (NC116A SIG-TEST-GF-17/GF-18).
#[test]
fn group_sign_clause_is_inherited_and_overridable() {
    let out = run_capture(
        r#"
IDENTIFICATION DIVISION.
PROGRAM-ID. SIGNINH.
DATA DIVISION.
WORKING-STORAGE SECTION.
01 TEST-17-DATA SIGN TRAILING.
   03 TEST-17-A PIC S9(4) VALUE +1234.
   03 TEST-17-GROUP SIGN LEADING SEPARATE.
      05 TEST-17-C PIC S9(4) VALUE +5678.
      05 FILLER REDEFINES TEST-17-C.
         07 TEST-17-C-SIGN PIC X.
         07 FILLER PIC X(4).
PROCEDURE DIVISION.
MAIN-PARA.
    DISPLAY "[" TEST-17-C-SIGN "]".
    STOP RUN.
"#,
    );
    assert_eq!(out, vec!["[+]"]);
}

/// An **embedded** sign is not a storage position: the item stays exactly its
/// digit positions wide, so a redefining overlay sees only digits. This is the
/// half of the SIGN clause that must NOT change (NC116A SIG-TEST-GF-5/GF-7).
#[test]
fn embedded_sign_does_not_widen_the_item() {
    let out = run_capture(
        r#"
IDENTIFICATION DIVISION.
PROGRAM-ID. EMBSIGN.
DATA DIVISION.
WORKING-STORAGE SECTION.
01 DS-L-5 PICTURE S99999 VALUE +91275 SIGN IS LEADING.
01 GRP-005 REDEFINES DS-L-5.
   02 TEST3-AN-1 PICTURE X.
   02 TEST3-AN-4 PICTURE X(4).
01 DS-T-5 PICTURE S99999 VALUE +83621 SIGN IS TRAILING.
01 GRP-007 REDEFINES DS-T-5.
   02 TEST4-AN-4 PICTURE X(4).
   02 TEST4-AN-1 PICTURE X.
PROCEDURE DIVISION.
MAIN-PARA.
    DISPLAY "[" TEST3-AN-4 "]".
    DISPLAY "[" TEST4-AN-4 "]".
    STOP RUN.
"#,
    );
    assert_eq!(out, vec!["[1275]", "[8362]"]);
}

// ── Subordinate REDEFINES ───────────────────────────────────────────────────

/// A `REDEFINES` entry inside a group is another reading of its sibling's
/// bytes, not more bytes. Emitting it as storage too pushed every later field
/// down by its width (NC252A RDF-TEST-003/RDF-TEST-5).
#[test]
fn subordinate_redefines_adds_no_bytes_to_the_group() {
    let out = run_capture(
        r#"
IDENTIFICATION DIVISION.
PROGRAM-ID. RDFSIZE.
DATA DIVISION.
WORKING-STORAGE SECTION.
01 REDEF10.
   02 RDFDATA1 PICTURE X(10) VALUE "ABC98765DE".
   02 RDFDATA2 PICTURE 9(4)V99 VALUE 9116.44.
   02 RDFDATA3.
      08 RDFDATA4 PICTURE X(6) VALUE "ALLDON".
      08 RDFDATA5 PICTURE XX99 VALUE "XX66".
   02 RDF3 REDEFINES RDFDATA3.
      03 RDF3-4 PICTURE X(8).
      03 RDF3-5 PIC 99.
   02 RDFDATA6 PICTURE A(20) VALUE "ZYXWVUTSRQPONMLKJIHG".
01 GRP-REDEF125 REDEFINES REDEF10.
   02 AN0020-X-0001 PIC X(26).
   02 AN0002-O036F-X-0002 PIC XX OCCURS 36 TIMES.
PROCEDURE DIVISION.
MAIN-PARA.
    DISPLAY "[" REDEF10 "]".
    DISPLAY "[" AN0002-O036F-X-0002 (8) "]".
    STOP RUN.
"#,
    );
    assert_eq!(
        out,
        vec!["[ABC98765DE911644ALLDONXX66ZYXWVUTSRQPONMLKJIHG]", "[LK]"]
    );
}

/// An elementary item carrying 88-level condition-names is still elementary:
/// the 88s name values of the item, not fields inside it. Counting them as
/// children dropped the item out of its parent's image (NC252A RDF-TEST-1).
#[test]
fn condition_names_do_not_make_an_item_a_group() {
    let out = run_capture(
        r#"
IDENTIFICATION DIVISION.
PROGRAM-ID. COND88.
DATA DIVISION.
WORKING-STORAGE SECTION.
01 REDEF10.
   02 RDFDATA3.
      08 RDFDATA4 PICTURE X(6) VALUE "ALLDON".
      08 RDFDATA5 PICTURE XX99 VALUE "XX66".
   02 RDF3 REDEFINES RDFDATA3.
      03 RDF3-4 PICTURE X(8).
      03 RDF3-5 PIC 99.
      03 RDF3-5-1 REDEFINES RDF3-5.
         04 RDF3-5-14 PIC 9.
         04 RDF3-5-15 PIC 9.
            88 HARD VALUE 0.
            88 SOFT VALUE 1.
PROCEDURE DIVISION.
MAIN-PARA.
    DISPLAY "[" RDF3-5-14 "]".
    DISPLAY "[" RDF3-5-15 "]".
    IF HARD
        DISPLAY "[HARD]"
    ELSE
        DISPLAY "[NOT-HARD]"
    END-IF.
    STOP RUN.
"#,
    );
    assert_eq!(out, vec!["[6]", "[6]", "[NOT-HARD]"]);
}

// ── 66-level RENAMES ────────────────────────────────────────────────────────

/// A `FILLER` holds bytes like any other elementary item, so a RENAMES range
/// spanning one has to carry it. Leaving it out closed the gap it occupies
/// (NC252A RENAM-TEST-5/RENAM-TEST-6).
#[test]
fn renames_range_spans_filler_items() {
    let out = run_capture(
        r#"
IDENTIFICATION DIVISION.
PROGRAM-ID. RNMFILL.
DATA DIVISION.
WORKING-STORAGE SECTION.
01 GRP-FOR-RENAMES.
   03 SUB-GRP-FOR-RENAMES-1.
   05 ELEM-FOR-RENAMES-1 PICTURE X VALUE "X".
   05 FILLER PICTURE XX VALUE SPACE.
   03 SUB-GRP-FOR-RENAMES-2.
   49 ELEM-FOR-RENAMES-2 PICTURE 999 VALUE 123.
   49 FILLER PICTURE 9 VALUE ZERO.
   49 ELEM-FOR-RENAMES-3 PICTURE XXXX VALUE ZERO.
   66 RENAMES-TEST-3 RENAMES SUB-GRP-FOR-RENAMES-1 THRU ELEM-FOR-RENAMES-2.
PROCEDURE DIVISION.
MAIN-PARA.
    DISPLAY "[" RENAMES-TEST-3 "]".
    STOP RUN.
"#,
    );
    assert_eq!(out, vec!["[X  123]"]);
}

/// `MOVE ALL literal` to a 66-level receiver fills every byte it spans, the
/// same way it fills a group. It used to write a single character and leave
/// the rest as they were (NC252A RENAM-TEST-3).
#[test]
fn move_all_fills_a_whole_renames_receiver() {
    let out = run_capture(
        r#"
IDENTIFICATION DIVISION.
PROGRAM-ID. RNMALL.
DATA DIVISION.
WORKING-STORAGE SECTION.
01 RENAMES-DATA.
   02 NAME1.
      03 NAME1A PICTURE XX VALUE SPACE.
      03 NAME1B PICTURE XXX VALUE SPACE.
   02 NAME2 PICTURE X(10) VALUE SPACE.
   02 NAME3.
      09 NAME3A PICTURE XXX VALUE SPACE.
      09 NAME3B PICTURE XX VALUE SPACE.
66 RENAME1 RENAMES NAME1 THRU NAME3.
PROCEDURE DIVISION.
MAIN-PARA.
    MOVE ALL "A" TO RENAMES-DATA.
    MOVE ALL "X" TO RENAME1.
    DISPLAY "[" NAME1 "]".
    DISPLAY "[" NAME2 "]".
    DISPLAY "[" NAME3 "]".
    STOP RUN.
"#,
    );
    assert_eq!(out, vec!["[XXXXX]", "[XXXXXXXXXX]", "[XXXXX]"]);
}
