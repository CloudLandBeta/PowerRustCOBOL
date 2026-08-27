// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! `OCCURS … DEPENDING ON` group length — NIST CCVS85 NC247A (VI-26 5.8).
//!
//! A group containing a variable-length table does not have one length, it has
//! two, and which one applies depends on the direction of the reference
//! (VI-26 5.8.3 SR5):
//!
//! * **Sending** — only the occurrences the depending item currently counts
//!   take part. A group with 3 of 9 entries active is 13 bytes, not 19, and
//!   that is what a comparison, `STRING`, `UNSTRING` and `INSPECT` see.
//! * **Receiving** — the declared **maximum** applies. The move is what writes
//!   the depending item, so the item's *old* value cannot bound the receiver:
//!   `MOVE ODO-RECORD TO NEW-RECORD` copies all nine occurrences into a record
//!   whose own depending item still reads 3.
//!
//! `SEARCH` and `SEARCH ALL` walk the active length too, so a value parked in a
//! dormant occurrence is not found.
//!
//! Reading the maximum in every direction made the sending cases wrong; reading
//! the current count in every direction made the receiving cases wrong. Both
//! halves are covered here.
//!
//! Separately, INSPECT read its operand through the plain value store, which a
//! group does not have — so it tallied **nothing at all** on any group, even a
//! fixed-length one. That is the first test below.

use std::sync::mpsc;

use cobolt_lexer::{tokenize, SourceFormat};
use cobolt_parser::{parse, Severity};
use cobolt_runtime::Interpreter;

/// Run `src` and return its DISPLAY lines. Panics with the diagnostics when the
/// program does not parse — an unsupported form must fail loudly here.
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
    display_rx.try_iter().map(|l| l.trim().to_string()).collect()
}

/// NC247A's two records plus its work area, with `body` as the procedure.
/// `ODO-RECORD` and `NEW-RECORD` are deliberately identical in shape — several
/// cases move one into the other.
fn prog(body: &str) -> String {
    format!(
        "       IDENTIFICATION DIVISION.
       PROGRAM-ID. ODOLEN.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01  ODO-RECORD.
           02  GRP-ODO.
               03  DOI-DU-01V00 PIC 9.
               03  ODO-XN-00009 PIC X(9).
               03  ODO-GRP-00009.
                   04  ODO-XN-00001-O009D OCCURS 0 TO 9 TIMES
                       DEPENDING ON DOI-DU-01V00
                       ASCENDING KEY ODO-XN-00001-O009D
                       INDEXED BY ODO-IX PIC X.
       01  NEW-RECORD.
           02  NEW-ODO.
               03  NEW-DU-01V00 PIC 9.
               03  NEW-XN-00009 PIC X(9).
               03  NEW-GRP-00009.
                   04  NEW-XN-00001-O009D OCCURS 0 TO 9 TIMES
                       DEPENDING ON NEW-DU-01V00
                       ASCENDING KEY NEW-XN-00001-O009D
                       INDEXED BY NEW-IX PIC X.
       01  WRK-GRP-00019.
           02  WRK-DU-01V00   PIC 9.
           02  WRK-XN-00009-1 PIC X(9).
           02  WRK-XN-00009-2 PIC X(9).
       01  WRK-XN-00020 PIC X(20).
       01  WRK-DU-05V00 PIC 9(5).
       PROCEDURE DIVISION.
       MAIN-PARA.
           PERFORM INIT-WRK-AREA.
{body}
           STOP RUN.
       INIT-WRK-AREA.
           MOVE \"9\" TO WRK-DU-01V00.
           MOVE \" ACTIVE: \" TO WRK-XN-00009-1.
           MOVE \"123456789\" TO WRK-XN-00009-2.
           MOVE 9 TO DOI-DU-01V00.
           MOVE \" ACTIVE: \" TO ODO-XN-00009.
           MOVE \"1\" TO ODO-XN-00001-O009D (1).
           MOVE \"2\" TO ODO-XN-00001-O009D (2).
           MOVE \"3\" TO ODO-XN-00001-O009D (3).
           MOVE \"4\" TO ODO-XN-00001-O009D (4).
           MOVE \"5\" TO ODO-XN-00001-O009D (5).
           MOVE \"6\" TO ODO-XN-00001-O009D (6).
           MOVE \"7\" TO ODO-XN-00001-O009D (7).
           MOVE \"8\" TO ODO-XN-00001-O009D (8).
           MOVE \"9\" TO ODO-XN-00001-O009D (9).
"
    )
}

// ── INSPECT on a group at all ──────────────────────────────────────────────

/// The general gap: a group has no value slot of its own, so INSPECT read the
/// empty string and tallied zero on *every* group operand. Nothing here is
/// variable-length — a fixed table shows the same bug.
#[test]
fn inspect_tallies_over_a_fixed_group() {
    let out = run(&prog(
        "           MOVE 0 TO WRK-DU-05V00.
           INSPECT WRK-GRP-00019 TALLYING WRK-DU-05V00 FOR ALL \"7\".
           DISPLAY WRK-DU-05V00.",
    ));
    assert_eq!(out, vec!["00001"]);
}

/// INSPECT REPLACING must write back through the group's subordinate items,
/// not into a slot the group does not own.
#[test]
fn inspect_replacing_writes_back_through_a_group() {
    let out = run(&prog(
        "           INSPECT WRK-GRP-00019 REPLACING ALL \"7\" BY \"Z\".
           DISPLAY WRK-XN-00009-2.",
    ));
    assert_eq!(out, vec!["123456Z89"]);
}

/// NC247A INS-TEST-F1-1: tallying over the *active* part of an ODO table. With
/// 9 active there is exactly one `"7"`.
#[test]
fn inspect_tallies_over_a_full_odo_group() {
    let out = run(&prog(
        "           MOVE 0 TO WRK-DU-05V00.
           INSPECT ODO-GRP-00009 TALLYING WRK-DU-05V00 FOR ALL \"7\".
           DISPLAY WRK-DU-05V00.",
    ));
    assert_eq!(out, vec!["00001"]);
}

/// With only 3 active the `"7"` is out of scope and must not be counted.
#[test]
fn inspect_skips_dormant_odo_occurrences() {
    let out = run(&prog(
        "           MOVE 3 TO DOI-DU-01V00.
           MOVE 0 TO WRK-DU-05V00.
           INSPECT ODO-GRP-00009 TALLYING WRK-DU-05V00 FOR ALL \"7\".
           DISPLAY WRK-DU-05V00.",
    ));
    assert_eq!(out, vec!["00000"]);
}

// ── sending: the current length applies ────────────────────────────────────

/// The whole group reads 1 + 9 + 9 with every occurrence active.
#[test]
fn full_odo_group_reads_its_maximum() {
    let out = run(&prog("           DISPLAY GRP-ODO."));
    assert_eq!(out, vec!["9 ACTIVE: 123456789"]);
}

/// The same group reads 1 + 9 + 3 once the depending item says 3. The dormant
/// occurrences still *hold* "4".."9" — they are simply not part of the group.
#[test]
fn partial_odo_group_reads_only_active_occurrences() {
    let out = run(&prog(
        "           MOVE 3 TO DOI-DU-01V00.
           DISPLAY GRP-ODO.",
    ));
    assert_eq!(out, vec!["3 ACTIVE: 123"]);
}

/// NC247A IF-TEST-GF-2: the shorter operand is space-padded, so a 13-byte
/// partial group compares equal to a 19-byte item holding the same prefix.
#[test]
fn partial_odo_group_compares_with_space_padding() {
    let out = run(&prog(
        "           MOVE 3 TO WRK-DU-01V00.
           MOVE 3 TO DOI-DU-01V00.
           MOVE \"123      \" TO WRK-XN-00009-2.
           IF GRP-ODO IS EQUAL TO WRK-GRP-00019
               DISPLAY \"EQUAL\"
           ELSE
               DISPLAY \"UNEQUAL\".",
    ));
    assert_eq!(out, vec!["EQUAL"]);
}

/// NC247A STR-TEST-GF-2: `STRING … DELIMITED BY SIZE` takes the sending
/// group's *current* size, so the literal follows immediately after 13 bytes.
#[test]
fn string_takes_the_partial_odo_size() {
    let out = run(&prog(
        "           MOVE 3 TO DOI-DU-01V00.
           MOVE SPACES TO WRK-XN-00020.
           STRING GRP-ODO DELIMITED BY SIZE
                  \"X\" DELIMITED BY SIZE
                  INTO WRK-XN-00020.
           DISPLAY WRK-XN-00020.",
    ));
    assert_eq!(out, vec!["3 ACTIVE: 123X"]);
}

// ── receiving: the declared maximum applies ────────────────────────────────

/// NC247A MOV-TEST-F1-3. The depending item is the group's own first byte, so
/// the move overwrites it partway through. The receiver is still measured at
/// its maximum — bounding it by the value the move itself had just written
/// truncated the table to whatever the new count said.
#[test]
fn receiving_group_uses_the_maximum_when_the_move_writes_its_own_count() {
    let out = run(&prog(
        "           MOVE \"3 ACTIVE: TEST PASS\" TO GRP-ODO.
           MOVE 9 TO DOI-DU-01V00.
           DISPLAY GRP-ODO.",
    ));
    assert_eq!(out, vec!["9 ACTIVE: TEST PASS"]);
}

/// NC247A MOV-TEST-F1-6 (VI-26 5.8.3 SR5): all nine occurrences move even
/// though the receiving record's depending item reads 3 when the move starts.
#[test]
fn record_move_fills_every_occurrence_of_the_receiver() {
    let out = run(&prog(
        "           MOVE \"P\" TO ODO-XN-00001-O009D (1).
           MOVE \"Q\" TO ODO-XN-00001-O009D (2).
           MOVE \"R\" TO ODO-XN-00001-O009D (3).
           MOVE \"S\" TO ODO-XN-00001-O009D (4).
           MOVE \"T\" TO ODO-XN-00001-O009D (5).
           MOVE \"U\" TO ODO-XN-00001-O009D (6).
           MOVE \"V\" TO ODO-XN-00001-O009D (7).
           MOVE \"W\" TO ODO-XN-00001-O009D (8).
           MOVE \"X\" TO ODO-XN-00001-O009D (9).
           MOVE 3 TO NEW-DU-01V00.
           MOVE ODO-RECORD TO NEW-RECORD.
           DISPLAY NEW-GRP-00009.",
    ));
    assert_eq!(out, vec!["PQRSTUVWX"]);
}

// ── SEARCH walks the active length ─────────────────────────────────────────

/// NC247A SCH-TEST-F1-2: `"7"` sits in occurrence 7, outside the 3 that are
/// active, so a sequential SEARCH must run off the end.
#[test]
fn search_stops_at_the_active_odo_length() {
    let out = run(&prog(
        "           MOVE 3 TO DOI-DU-01V00.
           SET ODO-IX TO 1.
           SEARCH ODO-XN-00001-O009D
               AT END DISPLAY \"AT-END\"
               WHEN ODO-XN-00001-O009D (ODO-IX) IS EQUAL TO \"7\"
                   DISPLAY \"FOUND\"
           END-SEARCH.",
    ));
    assert_eq!(out, vec!["AT-END"]);
}

/// NC247A SCH-TEST-4: the same bound applies to the binary `SEARCH ALL`.
#[test]
fn search_all_stops_at_the_active_odo_length() {
    let out = run(&prog(
        "           MOVE 3 TO DOI-DU-01V00.
           SEARCH ALL ODO-XN-00001-O009D
               AT END DISPLAY \"AT-END\"
               WHEN ODO-XN-00001-O009D (ODO-IX) IS EQUAL TO \"7\"
                   DISPLAY \"FOUND\"
           END-SEARCH.",
    ));
    assert_eq!(out, vec!["AT-END"]);
}

/// The guard against fixing the bound by simply breaking SEARCH: a value that
/// *is* inside the active range is still found.
#[test]
fn search_still_finds_an_active_occurrence() {
    let out = run(&prog(
        "           MOVE 3 TO DOI-DU-01V00.
           SET ODO-IX TO 1.
           SEARCH ODO-XN-00001-O009D
               AT END DISPLAY \"AT-END\"
               WHEN ODO-XN-00001-O009D (ODO-IX) IS EQUAL TO \"2\"
                   DISPLAY \"FOUND\"
           END-SEARCH.",
    ));
    assert_eq!(out, vec!["FOUND"]);
}
