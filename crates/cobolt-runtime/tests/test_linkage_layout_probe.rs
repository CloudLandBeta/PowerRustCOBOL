// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! What a group's storage actually looks like, item by item.
//!
//! Written to settle the `pairing-linkage-leaves-by-byte-offset` dead end in
//! `NIST/progress.json`. A LINKAGE parameter is a view over the caller's bytes,
//! so binding it means matching the two descriptions by **byte offset** — and
//! the first attempt at that made CCVS85 IC203A worse rather than better, for
//! reasons no amount of reading settled.
//!
//! The shape that matters is IC203A's `TABLE-2` against IC205A's. The caller
//! declares two bytes as `02 DN6 PIC X OCCURS 2 TIMES` — one child, occurring
//! twice — and the callee declares the same two bytes as `02 TV-1 PIC X` plus
//! `02 TV-2 PIC X`, two children. Any binding by tree position can only ever
//! reach half of it; a binding by offset should reach all of it, and did not.
//!
//! These are **probes, not guards**. They assert the properties an offset walk
//! depends on, so that when one is false it is named rather than guessed at.

use std::sync::mpsc;

use cobolt_lexer::{tokenize, SourceFormat};
use cobolt_parser::{parse, Severity};
use cobolt_runtime::Interpreter;

/// Run `src` and collect what it DISPLAYed, trimmed.
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
    let (_event_tx, event_rx) = std::sync::mpsc::channel();
    let (state_tx, _state_rx) = std::sync::mpsc::channel();
    let (display_tx, display_rx) = std::sync::mpsc::channel();
    let mut interp = Interpreter::new_with_channels(program, event_rx, state_tx, display_tx);
    interp.run().expect("run failed");
    display_rx.try_iter().map(|s| s.trim().to_owned()).collect()
}

/// Build an interpreter over `src` and run it, then hand back the environment
/// so its storage can be inspected.
fn env_of(src: &str) -> Interpreter {
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
    let (display_tx, _display_rx) = mpsc::channel();
    let mut interp = Interpreter::new_with_channels(program, event_rx, state_tx, display_tx);
    interp.run().expect("run failed");
    interp
}

const OCCURS_SIDE: &str = r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. OCCSIDE.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01  TABLE-2.
           02  DN6 PIC X OCCURS 2 TIMES.
       PROCEDURE DIVISION.
       MAIN.
           MOVE "AB" TO TABLE-2
           STOP RUN.
"#;

const FLAT_SIDE: &str = r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. FLATSIDE.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01  TABLE-2.
           02  TV-1 PIC X.
           02  TV-2 PIC X.
       PROCEDURE DIVISION.
       MAIN.
           MOVE "AB" TO TABLE-2
           STOP RUN.
"#;

/// Both descriptions must agree on the group's width, or no offset mapping
/// between them can mean anything. This is the precondition the whole approach
/// rests on and it was never checked before the first attempt.
#[test]
fn the_two_descriptions_of_table_2_are_the_same_width() {
    let occ = env_of(OCCURS_SIDE);
    let flat = env_of(FLAT_SIDE);
    let a = occ.env.stored_width("TABLE-2");
    let b = flat.env.stored_width("TABLE-2");
    assert_eq!(a, 2, "OCCURS 2 of PIC X is two bytes, got {a}");
    assert_eq!(b, 2, "two PIC X items are two bytes, got {b}");
}

/// `item_width` ends in `display_string(key).map(len).unwrap_or(0)`, so an item
/// it cannot render reports **zero** — and in an offset walk a zero shifts every
/// item after it. Suspect (1) of the dead end.
///
/// The unsubscripted name of an OCCURS item is the interesting case: it is what
/// a naive walk reaches when it does not expand the table.
#[test]
fn no_leaf_reports_a_zero_width() {
    let occ = env_of(OCCURS_SIDE);
    let flat = env_of(FLAT_SIDE);
    for (env, name) in [
        (&occ, "DN6"),
        (&occ, "DN6(1)"),
        (&occ, "DN6(2)"),
        (&flat, "TV-1"),
        (&flat, "TV-2"),
    ] {
        let w = env.env.stored_width(name);
        assert_ne!(
            w, 0,
            "{name} reports width 0 — an offset walk built on this silently \
             mis-places every item after it"
        );
    }
}

/// The declared children of the two descriptions, which is what the reverted
/// attempt paired positionally. One entry against two is why position cannot
/// express this pair, and is the reason to want an offset mapping at all.
#[test]
fn the_declared_children_do_not_correspond() {
    let occ = env_of(OCCURS_SIDE);
    let flat = env_of(FLAT_SIDE);
    let occ_kids = occ
        .env
        .symbol("TABLE-2")
        .map(|s| s.layout_keys.len())
        .unwrap_or(0);
    let flat_kids = flat
        .env
        .symbol("TABLE-2")
        .map(|s| s.layout_keys.len())
        .unwrap_or(0);
    assert_eq!(occ_kids, 1, "OCCURS side declares one child");
    assert_eq!(flat_kids, 2, "flat side declares two");
    assert_ne!(
        occ_kids, flat_kids,
        "if these ever match, this pair stopped being the interesting case"
    );
}

/// An occurrence has to be addressable by its subscripted key for an offset
/// walk to name it — `DN6(1)` and `DN6(2)` are what such a walk emits, and an
/// alias onto a key the environment does not resolve would bind nothing at all.
/// Suspect (2) of the dead end.
#[test]
fn each_occurrence_is_addressable_and_holds_its_own_byte() {
    let occ = env_of(OCCURS_SIDE);
    let first = occ.env.display_string("DN6(1)");
    let second = occ.env.display_string("DN6(2)");
    assert_eq!(
        first.as_deref(),
        Some("A"),
        "DN6(1) should hold the first byte of \"AB\""
    );
    assert_eq!(
        second.as_deref(),
        Some("B"),
        "DN6(2) should hold the second"
    );
}

/// The direction the CALLEE writes in: a subprogram sets the leaves, and the
/// caller then reads the group. If a write to `DN6(1)` does not surface in
/// `TABLE-2`, an offset binding onto occurrences is unobservable from the
/// caller no matter how correctly the offsets were computed — and IC203A reads
/// `TABLE-2`, which is exactly what its failing assertion names.
#[test]
fn writing_an_occurrence_surfaces_in_the_group() {
    let mut occ = env_of(OCCURS_SIDE);
    occ.env.set_str("DN6(1)", "X");
    occ.env.set_str("DN6(2)", "Y");
    let group = occ.env.display_string("TABLE-2");
    assert_eq!(
        group.as_deref(),
        Some("XY"),
        "the group must read back what was written into its occurrences; \
         if it does not, binding a LINKAGE item onto DN6(1) is invisible to \
         the program that owns the table"
    );
}

/// **An alias is only ever looked up by BASE NAME**, so an alias entry written
/// against one occurrence is never consulted by anything.
///
/// This is the property that settles the `pairing-linkage-leaves-by-byte-offset`
/// dead end. `resolve_name` resolves the *unsubscripted* leaf and consults
/// `addr_aliases` with that; the subscript is appended by the caller
/// afterwards. So `DN6(1) -> TV-1` sits in the map and nothing reads it, and a
/// per-occurrence offset mapping binds strictly LESS than the positional
/// pairing it replaced — which is exactly the IC203A 1 -> 7 that was measured
/// and reverted at 1.62.90.
///
/// The consequence for the design: IC203A's shape (`02 DN6 PIC X OCCURS 2`
/// against `02 TV-1 PIC X` + `02 TV-2 PIC X`) cannot be expressed as an
/// item-to-item alias at all, so it needs the shared-buffer model rather than
/// a better offset walk.
#[test]
fn a_subscripted_alias_key_is_never_consulted() {
    let mut env = cobolt_runtime::CobolEnvironment::new();
    env.set_alias("DN6(1)", "TV-1");

    // Exactly what the interpreter does for a reference `DN6 (1)`: resolve the
    // leaf, then apply the subscript to whatever came back.
    let resolved = env.resolve_name("DN6", &[]);
    let key = cobolt_runtime::environment::subscript_key(&resolved, &[1]);

    assert_eq!(
        key, "DN6(1)",
        "the per-occurrence alias was consulted after all — if this ever fails, \
         the offset mapping in the ledger becomes viable and the dead end \
         pairing-linkage-leaves-by-byte-offset should be re-read"
    );
    assert_eq!(
        resolved, "DN6",
        "resolve_name must not have found an alias keyed by a subscripted name"
    );
}

/// Does the flat offset walk actually produce spans for IC103A's shape?
///
/// The offset pairing landed and moved nothing, so before theorising about the
/// pairing this asks the walker directly. IC103A describes ten bytes as
/// `02 ALPHA-NUM-FIELD X(5)` + `02 GROUP-LEV2 (03 NUMER-FIELD 99, 03
/// ALPHA-FIELD A(3))`, and every leaf should come back with its offset.
const FLAT_GROUP: &str = r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. FLATG.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01  GROUP-01.
           02  ALPHA-NUM-FIELD PIC X(5).
           02  GROUP-LEV2.
               03  NUMER-FIELD  PIC 99.
               03  ALPHA-FIELD  PIC A(3).
       PROCEDURE DIVISION.
       MAIN.
           MOVE "ABCDE" TO ALPHA-NUM-FIELD
           MOVE 42 TO NUMER-FIELD
           MOVE "XYZ" TO ALPHA-FIELD
           STOP RUN.
"#;

#[test]
fn the_flat_offset_walk_reaches_every_leaf() {
    let env = env_of(FLAT_GROUP);
    let spans = env.env.flat_leaf_spans("GROUP-01");
    assert!(
        spans.is_some(),
        "GROUP-01 has no OCCURS anywhere, so the flat walk must produce spans; \
         None means a leaf reported width 0 or a symbol was missing"
    );
    let spans = spans.unwrap();
    let got: Vec<(String, usize, usize)> = spans.clone();
    assert_eq!(
        got.len(),
        3,
        "three elementary leaves are expected; got {got:?}"
    );
    assert_eq!(
        got.iter().map(|(_, o, w)| (*o, *w)).collect::<Vec<_>>(),
        vec![(0, 5), (5, 2), (7, 3)],
        "offsets and widths must tile the ten bytes in order; got {got:?}"
    );
}

/// The same walk over a **LINKAGE** description, which is what the CALL path
/// actually asks about.
///
/// A LINKAGE item owns no storage of its own and has never been written when
/// the binding is computed, so if `item_width` measures the *rendered value*
/// rather than the declared PICTURE, every leaf reports width 0 and the walk
/// refuses. That would make the offset pairing silently inert — which is
/// exactly what was observed: it landed and moved nothing.
const LINKAGE_GROUP: &str = r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. LNKG.
       DATA DIVISION.
       LINKAGE SECTION.
       01  GRP-01.
           02  AN-FIELD     PIC X(5).
           02  NUM-DISPLAY  PIC 99.
           02  GRP-LEVEL.
               03  A-FIELD  PIC A(3).
       PROCEDURE DIVISION.
       MAIN.
           STOP RUN.
"#;

#[test]
fn the_flat_offset_walk_reaches_an_unwritten_linkage_description() {
    let env = env_of(LINKAGE_GROUP);
    let spans = env.env.flat_leaf_spans("GRP-01");
    assert!(
        spans.is_some(),
        "GRP-01 is flat, so the walk must produce spans even though nothing has \
         written to it — None means a leaf's width came from its (empty) value \
         instead of its PICTURE"
    );
    assert_eq!(
        spans
            .unwrap()
            .iter()
            .map(|(_, o, w)| (*o, *w))
            .collect::<Vec<_>>(),
        vec![(0, 5), (5, 2), (7, 3)],
        "an unwritten LINKAGE description must lay out by its PICTUREs"
    );
}

/// A group parameter whose callee describes the same bytes with a **different
/// tree** must still reach every field.
///
/// CCVS85 **IC103A**, reduced. Its own header states the case: "THE ITEM
/// DESCRIPTIONS ARE DIFFERENT IN THE SUBPROGRAM FROM THE MAIN PROGRAM, BUT THE
/// NUMBER OF CHARACTERS IS IDENTICAL." The caller describes ten bytes as two
/// top-level children, the callee as three. Pairing by tree position bound only
/// the first — `NUM-DISPLAY` got the caller's whole five-byte `GROUP-LEV2`
/// ("42XYZ") and `A-FIELD` got nothing at all.
const DIFFERING_TREES: &str = r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. GMAIN.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01  GROUP-01.
           02  ALPHA-NUM-FIELD PIC X(5).
           02  GROUP-LEV2.
               03  NUMER-FIELD  PIC 99.
               03  ALPHA-FIELD  PIC A(3).
       PROCEDURE DIVISION.
       MAIN.
           MOVE "ABCDE" TO ALPHA-NUM-FIELD
           MOVE 42 TO NUMER-FIELD
           MOVE "XYZ" TO ALPHA-FIELD
           CALL "GSUB" USING GROUP-01
           STOP RUN.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. GSUB.
       DATA DIVISION.
       LINKAGE SECTION.
       01  GRP-01.
           02  AN-FIELD     PIC X(5).
           02  NUM-DISPLAY  PIC 99.
           02  GRP-LEVEL.
               03  A-FIELD  PIC A(3).
       PROCEDURE DIVISION USING GRP-01.
       S-MAIN.
           DISPLAY "AN=["  AN-FIELD    "]"
           DISPLAY "NUM=[" NUM-DISPLAY "]"
           DISPLAY "A=["   A-FIELD     "]"
           EXIT PROGRAM.
       END PROGRAM GSUB.
       END PROGRAM GMAIN.
"#;

#[test]
fn a_group_parameter_pairs_its_leaves_by_offset_not_position() {
    let out = run_capture(DIFFERING_TREES);
    let field = |name: &str| -> String {
        out.iter()
            .find_map(|l| {
                l.strip_prefix(&format!("{name}=["))
                    .and_then(|r| r.strip_suffix(']'))
                    .map(str::to_owned)
            })
            .unwrap_or_else(|| panic!("the callee displays {name}=[…]; got {out:?}"))
    };
    assert_eq!(field("AN"), "ABCDE", "first child: position and offset agree");
    assert_eq!(
        field("NUM"),
        "42",
        "NUM-DISPLAY covers bytes 5..7, so it must bind to NUMER-FIELD; \
         \"42XYZ\" means it was paired with the caller's GROUP-LEV2 by position"
    );
    assert_eq!(
        field("A"),
        "XYZ",
        "A-FIELD covers bytes 7..10 and must bind to ALPHA-FIELD; blank means \
         the positional zip ran out of caller children and left it unbound"
    );
}

/// Pairs with the test above: a description containing a **table** must keep
/// the positional pairing, because an alias is only looked up by base name and
/// a per-occurrence entry would never be consulted. Binding by offset there
/// would leave the leaf bound to nothing — worse than the imperfect pairing it
/// replaced. This is IC203A's shape, which the 1.62.90 attempt took from 1
/// failure to 7.
#[test]
fn an_occurs_description_is_left_on_the_positional_pairing() {
    let env = env_of(FLAT_GROUP);
    assert!(
        env.env.flat_leaf_spans("TABLE-2").is_none(),
        "a description with OCCURS must refuse the flat walk"
    );
}
