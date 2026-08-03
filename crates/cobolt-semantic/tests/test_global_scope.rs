// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Tests: `GLOBAL` items reach the programs their declarer contains.
//!
//! COBOL-85 scopes a `GLOBAL` name to the declaring program and to every
//! program contained in it, however deeply. A PowerRustCOBOL form is the
//! outermost program of the generated nest and every event handler and common
//! procedure is contained in it, so the form's `01 … GLOBAL.` record is the one
//! way shared application state reaches a handler at all.
//!
//! Contained programs are analyzed with their own symbol table, which is right
//! — a name resolves against the program that declares it — but the table was
//! built from that program's DATA DIVISION *alone*. Every handler that read a
//! form-level item was told the name `is not declared in DATA DIVISION`, and
//! nothing the agent could write would satisfy it: declaring the item locally
//! makes a second, unrelated copy. The code ran correctly regardless (the
//! interpreter keeps one shared environment), so this was the analyzer
//! rejecting programs the runtime was perfectly happy to execute.

use cobolt_lexer::{tokenize, SourceFormat};
use cobolt_parser::parse;
use cobolt_semantic::analyze;

fn analyze_src(src: &str) -> cobolt_semantic::SemanticResult {
    let parsed = parse(tokenize(src, SourceFormat::Free));
    // Guard the premise. A body the parser threw away cannot be resolved, so a
    // semantic test over it would report nothing and pass for the wrong reason.
    assert!(
        !parsed.has_errors(),
        "the source must parse cleanly: {:?}",
        parsed.diagnostics
    );
    let program = parsed.program.expect("program should parse");
    // Guard the premise. Every assertion below is about what a CONTAINED
    // program can see; if the source parsed as one flat program, or as two
    // siblings, the tests would pass without proving anything at all.
    assert!(
        !program.nested_programs.is_empty(),
        "these tests require a contained program — the source did not nest"
    );
    analyze(&program)
}

/// EVERY diagnostic, whatever its severity. An unresolved identifier is
/// reported as a warning, not an error, so filtering to errors would make these
/// tests pass whether or not `GLOBAL` items are inherited — which is the whole
/// thing under test.
fn messages(sem: &cobolt_semantic::SemanticResult) -> Vec<String> {
    sem.diagnostics.iter().map(|d| d.message.clone()).collect()
}

fn mentions(msgs: &[String], name: &str) -> bool {
    msgs.iter().any(|m| m.contains(name))
}

/// The shape the project's COBOL code standard mandates: application data in
/// one `01 … GLOBAL` record on the form, named from a contained handler.
#[test]
fn a_contained_program_sees_the_global_record_its_parent_declares() {
    let src = r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. CHECKBOXES-FORM.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01  MC-APPLICATION-DATA GLOBAL.
           05  CALCULATION-DATA.
               10  TOTAL-AMOUNT   PIC 999V99 COMP VALUE ZERO.
           05  FORMATTING-DATA.
               10  EDITED-TOTAL   PIC ZZ9,99.
       PROCEDURE DIVISION.
       MAIN SECTION.
           CONTINUE.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. UPDATE-RECEIPT.
       DATA DIVISION.
       PROCEDURE DIVISION.
       MAIN SECTION.
           MOVE 5 TO TOTAL-AMOUNT
           MOVE TOTAL-AMOUNT TO EDITED-TOTAL
           EXIT PROGRAM.
       END PROGRAM UPDATE-RECEIPT.
       END PROGRAM CHECKBOXES-FORM.
"#;
    let msgs = messages(&analyze_src(src));
    assert!(
        !mentions(&msgs, "TOTAL-AMOUNT") && !mentions(&msgs, "EDITED-TOTAL"),
        "a GLOBAL record must reach the contained program: {msgs:?}"
    );
}

/// `GLOBAL` is written on the `01`, never on the subordinates — so the whole
/// subtree travels, which is the only reason the standard's "declare it at
/// level 01 and organize related fields beneath it" rule works.
#[test]
fn the_whole_subtree_under_a_global_record_travels_not_just_its_name() {
    let src = r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. F.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01  MC-MENU GLOBAL.
           05  HAMBURGER-DATA.
               10  HAMBURGER-PRICE  PIC 99V99 COMP OCCURS 4 TIMES.
       PROCEDURE DIVISION.
       MAIN SECTION.
           CONTINUE.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. H.
       DATA DIVISION.
       PROCEDURE DIVISION.
       MAIN SECTION.
           MOVE 7 TO HAMBURGER-PRICE (1)
           EXIT PROGRAM.
       END PROGRAM H.
       END PROGRAM F.
"#;
    let msgs = messages(&analyze_src(src));
    assert!(
        !mentions(&msgs, "HAMBURGER-PRICE"),
        "a 10-level item under a GLOBAL 01 must resolve: {msgs:?}"
    );
}

/// Visibility is a privilege `GLOBAL` grants, not something every parent item
/// has. Without the clause the name stays private to the declaring program, and
/// a contained program naming it is still an error.
#[test]
fn an_item_that_is_not_global_stays_private_to_its_program() {
    let src = r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. F.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01  WS-PRIVATE-TOTAL   PIC 999V99 COMP VALUE ZERO.
       PROCEDURE DIVISION.
       MAIN SECTION.
           CONTINUE.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. H.
       DATA DIVISION.
       PROCEDURE DIVISION.
       MAIN SECTION.
           MOVE 1 TO WS-PRIVATE-TOTAL
           EXIT PROGRAM.
       END PROGRAM H.
       END PROGRAM F.
"#;
    let msgs = messages(&analyze_src(src));
    assert!(
        mentions(&msgs, "WS-PRIVATE-TOTAL"),
        "a non-GLOBAL item must not leak into a contained program: {msgs:?}"
    );
}

/// A contained program that declares the name itself gets its own item —
/// COBOL-85 resolves innermost-first, so the inherited GLOBAL is shadowed
/// rather than colliding with it.
#[test]
fn a_contained_program_shadows_an_inherited_global_with_its_own_declaration() {
    let src = r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. F.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01  SHARED-COUNT   PIC 9(4) COMP VALUE 0.
       01  MC-DATA GLOBAL.
           05  SHARED-COUNT-G   PIC 9(4) COMP VALUE 0.
       PROCEDURE DIVISION.
       MAIN SECTION.
           CONTINUE.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. H.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01  SHARED-COUNT-G   PIC 9(4) COMP VALUE 0.
       PROCEDURE DIVISION.
       MAIN SECTION.
           MOVE 1 TO SHARED-COUNT-G
           EXIT PROGRAM.
       END PROGRAM H.
       END PROGRAM F.
"#;
    let msgs = messages(&analyze_src(src));
    assert!(
        !mentions(&msgs, "SHARED-COUNT-G"),
        "redeclaring an inherited GLOBAL name locally is legal shadowing, \
         not a collision and not an unresolved name: {msgs:?}"
    );
}
