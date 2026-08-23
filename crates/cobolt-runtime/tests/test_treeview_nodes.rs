// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Walking a TreeView from COBOL: `TV::NodeParent(N)`, `TV::NodeNextSibling(N)`
//! and the rest.
//!
//! A node event hands the handler `CONTROL-NODE-INDEX`; these methods take that
//! same index and answer with another one, so a handler can climb to a parent,
//! run along the siblings, or read what the node carries. Before this a handler
//! knew the node it fired on and nothing about where that node sat.
//!
//! The index is the HANDLE. There is no node object to hold — a held object
//! would go stale the moment `Items` changed, while an index is re-read against
//! whatever the tree holds now.

use std::sync::mpsc;

use cobolt_lexer::{tokenize, SourceFormat};
use cobolt_parser::{parse, Severity};
use cobolt_runtime::Interpreter;

fn run(src: &str) -> Vec<String> {
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
    display_rx.try_iter().collect()
}

/// The tree every test below builds, and the indexes it produces:
///
/// ```text
/// 0 Warehouse
/// 1   Inbound
/// 2     Dock A
/// 3   Outbound
/// 4 Office
/// ```
const BUILD: &str = r#"
           TREE-1::AddNode(0, "Warehouse").
           TREE-1::AddNode(1, "Inbound").
           TREE-1::AddNode(2, "Dock A").
           TREE-1::AddNode(1, "Outbound").
           TREE-1::AddNode(0, "Office").
"#;

fn program(body: &str) -> String {
    format!(
        r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. TREEWALK.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 WS-IDX      PIC S9(4).
       01 WS-TEXT     PIC X(40).
       PROCEDURE DIVISION.
{BUILD}
{body}
           STOP RUN.
"#
    )
}

/// The level is a NUMBER, not leading spaces in a literal — `AddItem` trims its
/// argument (it must: a PIC X field arrives space-padded), so an indented
/// literal could never have built a child.
#[test]
fn a_tree_is_built_by_level_and_counted() {
    let out = run(&program(
        r#"
           MOVE TREE-1::NodeCount() TO WS-IDX.
           DISPLAY "COUNT=" WS-IDX.
           MOVE TREE-1::NodeText(2) TO WS-TEXT.
           DISPLAY "N2=[" WS-TEXT "]".
           MOVE TREE-1::NodeLevel(2) TO WS-IDX.
           DISPLAY "L2=" WS-IDX.
"#,
    ));
    let joined = out.join("\n");
    assert!(joined.contains("COUNT=0005"), "five nodes: {joined}");
    assert!(joined.contains("N2=[Dock A"), "index 2 is Dock A: {joined}");
    assert!(joined.contains("L2=0002"), "Dock A sits two deep: {joined}");
}

/// Climbing: a handler that has the node an event fired on can reach the node
/// it hangs under, and keep going to the root.
#[test]
fn a_handler_climbs_from_a_node_to_its_root() {
    let out = run(&program(
        r#"
           MOVE TREE-1::NodeParent(2) TO WS-IDX.
           DISPLAY "P=" WS-IDX.
           MOVE TREE-1::NodeText(WS-IDX) TO WS-TEXT.
           DISPLAY "PT=[" WS-TEXT "]".
           MOVE TREE-1::NodeParent(WS-IDX) TO WS-IDX.
           DISPLAY "GP=" WS-IDX.
           MOVE TREE-1::NodeParent(WS-IDX) TO WS-IDX.
           DISPLAY "ROOT-PARENT=" WS-IDX.
"#,
    ));
    let joined = out.join("\n");
    assert!(joined.contains("P=0001"), "Dock A hangs under Inbound: {joined}");
    assert!(joined.contains("PT=[Inbound"), "and the index reads back: {joined}");
    assert!(joined.contains("GP=0000"), "Inbound hangs under Warehouse: {joined}");
    assert!(
        joined.contains("ROOT-PARENT=-0001"),
        "a root has no parent, and says so with -1: {joined}"
    );
}

/// Running along the siblings — the loop a handler writes to visit everything
/// under one parent. `-1` is what ends it.
#[test]
fn a_handler_runs_along_the_siblings() {
    let out = run(&program(
        r#"
           MOVE TREE-1::NodeFirstChild(0) TO WS-IDX.
           PERFORM UNTIL WS-IDX < 0
               MOVE TREE-1::NodeText(WS-IDX) TO WS-TEXT
               DISPLAY "CHILD=[" WS-TEXT "]"
               MOVE TREE-1::NodeNextSibling(WS-IDX) TO WS-IDX
           END-PERFORM.
           MOVE TREE-1::NodeChildCount(0) TO WS-IDX.
           DISPLAY "KIDS=" WS-IDX.
"#,
    ));
    let joined = out.join("\n");
    assert!(joined.contains("CHILD=[Inbound"), "first child: {joined}");
    assert!(joined.contains("CHILD=[Outbound"), "second child: {joined}");
    assert!(
        !joined.contains("CHILD=[Dock A"),
        "a GRANDchild is not a child — the walk must not descend: {joined}"
    );
    assert!(
        !joined.contains("CHILD=[Office"),
        "and it must not escape into the next root: {joined}"
    );
    assert!(joined.contains("KIDS=0002"), "two children: {joined}");
}

/// A node carries its own icon, colour and background, and they read back — the
/// fields `AddNode` writes after the label.
#[test]
fn a_node_carries_and_returns_its_own_dress() {
    let out = run(&program(
        r##"
           TREE-1::AddNode(0, "Overdue", "alert", "#C81E1E", "#202020").
           MOVE TREE-1::NodeIndexOf("Overdue") TO WS-IDX.
           DISPLAY "AT=" WS-IDX.
           MOVE TREE-1::NodeIcon(WS-IDX) TO WS-TEXT.
           DISPLAY "ICON=[" WS-TEXT "]".
           MOVE TREE-1::NodeColor(WS-IDX) TO WS-TEXT.
           DISPLAY "COLOR=[" WS-TEXT "]".
           MOVE TREE-1::NodeBackColor(WS-IDX) TO WS-TEXT.
           DISPLAY "BACK=[" WS-TEXT "]".
           MOVE TREE-1::NodePath(2) TO WS-TEXT.
           DISPLAY "PATH=[" WS-TEXT "]".
"##,
    ));
    let joined = out.join("\n");
    assert!(joined.contains("AT=0005"), "appended as node 5: {joined}");
    assert!(joined.contains("ICON=[alert"), "{joined}");
    assert!(joined.contains("COLOR=[#C81E1E"), "{joined}");
    assert!(joined.contains("BACK=[#202020"), "{joined}");
    assert!(
        joined.contains("PATH=[Warehouse/Inbound/Dock A"),
        "a path names a node that a label alone cannot: {joined}"
    );
}

/// Ticked and folded are LIVE state, held on the control by label — so they are
/// answered from `CheckedNodes` / `CollapsedNodes`, not from the node's line.
#[test]
fn checked_and_collapsed_read_the_controls_live_state() {
    let out = run(&program(
        r#"
           MOVE "Inbound" TO TREE-1::CheckedNodes.
           MOVE "Warehouse" TO TREE-1::CollapsedNodes.
           MOVE TREE-1::NodeChecked(1) TO WS-IDX.
           DISPLAY "CHK1=" WS-IDX.
           MOVE TREE-1::NodeChecked(3) TO WS-IDX.
           DISPLAY "CHK3=" WS-IDX.
           MOVE TREE-1::NodeCollapsed(0) TO WS-IDX.
           DISPLAY "COL0=" WS-IDX.
           MOVE TREE-1::NodeCollapsed(1) TO WS-IDX.
           DISPLAY "COL1=" WS-IDX.
"#,
    ));
    let joined = out.join("\n");
    assert!(joined.contains("CHK1=0001"), "Inbound is ticked: {joined}");
    assert!(joined.contains("CHK3=0000"), "Outbound is not: {joined}");
    assert!(joined.contains("COL0=0001"), "Warehouse is folded: {joined}");
    assert!(joined.contains("COL1=0000"), "Inbound is not: {joined}");
}

/// A walk runs off the end of a tree by design. Asking about a node that is not
/// there answers empty rather than raising — the walk is guarded by the `-1`
/// the traversal calls return, not by an error every loop would have to trap.
#[test]
fn asking_past_the_end_is_answered_not_raised() {
    let out = run(&program(
        r#"
           MOVE TREE-1::NodeText(99) TO WS-TEXT.
           DISPLAY "GONE=[" WS-TEXT "]".
           MOVE TREE-1::NodeParent(99) TO WS-IDX.
           DISPLAY "GONE-P=" WS-IDX.
           MOVE TREE-1::NodeIndexOf("Nowhere") TO WS-IDX.
           DISPLAY "MISSING=" WS-IDX.
"#,
    ));
    let joined = out.join("\n");
    assert!(joined.contains("GONE=[ "), "an absent node reads empty: {joined}");
    assert!(joined.contains("GONE-P=-0001"), "{joined}");
    assert!(joined.contains("MISSING=-0001"), "{joined}");
}
