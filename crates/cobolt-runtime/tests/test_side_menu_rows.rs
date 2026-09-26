// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Spec 066 — a running program adds, changes and removes SideMenu rows, from
//! real COBOL.
//!
//! The rows travel as one property, `RuntimeRows`, through the same state
//! channel every property uses; what these tests pin is the half that lives
//! here — that each call leaves the right rows, that a designed row can never
//! be touched, and that the list-control names (`AddItem`, `Clear`, …) never
//! fall through to the `Items` property a SideMenu does not have.

use std::sync::mpsc;

use cobolt_forms::menu::runtime::{merge_rows, parse_rows, RUNTIME_ROWS_PROP};
use cobolt_forms::menu::{MenuDefinition, MenuItem};
use cobolt_lexer::{tokenize, SourceFormat};
use cobolt_parser::{parse, Severity};
use cobolt_runtime::channels::StateUpdate;
use cobolt_runtime::Interpreter;

/// Run `body` against a SideMenu whose designed menu is Home + Documents.
/// Returns the DISPLAY lines and every state update the host would receive.
fn run(body: &str) -> (Vec<String>, Vec<StateUpdate>) {
    let src = format!(
        r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. T.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 WS-I      PIC 9(3).
       01 WS-ID     PIC X(20).
       01 WS-OK     PIC X(4).
       PROCEDURE DIVISION.
{body}
           STOP RUN.
"#
    );
    let result = parse(tokenize(&src, SourceFormat::Free));
    assert!(
        result.diagnostics.iter().all(|d| d.severity != Severity::Error),
        "parse errors: {:?}",
        result.diagnostics
    );
    let program = result.program.expect("no program");
    let (_event_tx, event_rx) = mpsc::channel();
    let (state_tx, state_rx) = mpsc::channel();
    let (display_tx, display_rx) = mpsc::channel();
    let mut interp = Interpreter::new_with_channels(program, event_rx, state_tx, display_tx);
    interp.seed_objects(vec![(
        "MENU-1".to_owned(),
        "SideMenu".to_owned(),
        Vec::<(String, String)>::new(),
    )]);
    let mut docs = MenuItem::new_action("docs", "Documents");
    docs.items.push(MenuItem::new_action("docs-new", "New"));
    interp.set_designed_menu(
        "MENU-1",
        &MenuDefinition {
            menu: vec![MenuItem::new_action("home", "Home"), docs],
            hash: String::new(),
        },
    );
    interp.run().expect("run failed");
    (display_rx.try_iter().collect(), state_rx.try_iter().collect())
}

/// The rows as the host ends up holding them: the last `RuntimeRows` write.
fn final_rows(ups: &[StateUpdate]) -> Vec<cobolt_forms::menu::runtime::RuntimeRow> {
    ups.iter()
        .rev()
        .find(|u| u.prop.eq_ignore_ascii_case(RUNTIME_ROWS_PROP))
        .map(|u| parse_rows(&u.value))
        .unwrap_or_default()
}

/// AC1 — fifty rows, a section and a nested row, in order, after the designed
/// rows. Reported as a summary, not per row.
#[test]
fn a_program_adds_fifty_rows_a_section_and_a_nested_row() {
    let started = std::time::Instant::now();
    let (out, ups) = run(
        r#"
           PERFORM VARYING WS-I FROM 1 BY 1 UNTIL WS-I > 50
               STRING "chat-" WS-I DELIMITED BY SIZE INTO WS-ID
               MOVE MENU-1::AddItem(WS-ID, "Conversation", "chat") TO WS-OK
               MOVE SPACES TO WS-ID
           END-PERFORM.
           MOVE MENU-1::AddSection("Folders") TO WS-ID.
           DISPLAY "SECTION=" WS-ID.
           MOVE MENU-1::AddItem("inbox", "Inbox", "", "docs") TO WS-OK.
           DISPLAY "NESTED=" WS-OK.
           MOVE MENU-1::GetCount() TO WS-I.
           DISPLAY "COUNT=" WS-I.
"#,
    );
    let elapsed = started.elapsed();
    let joined = out.join("\n");
    let rows = final_rows(&ups);
    let designed = vec![MenuItem::new_action("home", "Home"), {
        let mut d = MenuItem::new_action("docs", "Documents");
        d.items.push(MenuItem::new_action("docs-new", "New"));
        d
    }];
    let merged = merge_rows(&designed, &rows);

    assert!(joined.contains("SECTION=section-1"), "{joined}");
    assert!(joined.contains("NESTED=1"), "{joined}");
    assert!(joined.contains("COUNT=052"), "50 rows + a section + a nested row: {joined}");
    assert_eq!(merged[0].id, "home");
    assert_eq!(merged[1].id, "docs");
    assert_eq!(merged[1].items.last().map(|i| i.id.as_str()), Some("inbox"));
    assert_eq!(merged[2].id, "chat-001", "run-time rows follow the designed ones");
    assert_eq!(merged[51].id, "chat-050");
    assert_eq!(merged[52].label, "Folders");

    println!("\n  ── SideMenu run-time rows ─────────────────────────────");
    println!("  forms:  AddItem(id, label, icon) ×50 · AddSection(title) ·");
    println!("          AddItem(id, label, icon, parent) under a designed row · GetCount()");
    println!("  result: {} top-level rows after 2 designed; 1 nested; count 52", merged.len() - 2);
    println!("  time:   {:.2} ms for 53 calls", elapsed.as_secs_f64() * 1000.0);
    println!("  ───────────────────────────────────────────────────────\n");
}

/// AC3/AC4 — setters change run-time rows; every attempt on a designed row is
/// refused with `0` and changes nothing; `Items` is never written.
#[test]
fn designed_rows_are_refused_and_items_is_never_written() {
    let (out, ups) = run(
        r#"
           MOVE MENU-1::AddItem("c1", "Chat") TO WS-OK.
           MOVE MENU-1::AddItem("c1a", "Part", "", "c1") TO WS-OK.
           MOVE MENU-1::SetItemLabel("c1", "Renamed") TO WS-OK.
           DISPLAY "RELABEL=" WS-OK.
           MOVE MENU-1::SetItemBadge("c1", "3") TO WS-OK.
           MOVE MENU-1::SetItemEnabled("c1", "0") TO WS-OK.
           MOVE MENU-1::SetItemIcon("home", "X") TO WS-OK.
           DISPLAY "ICON-DESIGNED=" WS-OK.
           MOVE MENU-1::RemoveItem("docs") TO WS-OK.
           DISPLAY "REMOVE-DESIGNED=" WS-OK.
           MOVE MENU-1::AddItem("home", "Shadow") TO WS-OK.
           DISPLAY "SHADOW-DESIGNED=" WS-OK.
           MOVE MENU-1::HasItem("home") TO WS-OK.
           DISPLAY "HAS-HOME=" WS-OK.
           MOVE MENU-1::RemoveItem("c1") TO WS-OK.
           DISPLAY "REMOVE=" WS-OK.
           MOVE MENU-1::HasItem("c1a") TO WS-OK.
           DISPLAY "CHILD-GONE=" WS-OK.
           MOVE MENU-1::AddItem("c2", "Other") TO WS-OK.
           INVOKE MENU-1 "Clear".
           MOVE MENU-1::GetCount() TO WS-I.
           DISPLAY "AFTER-CLEAR=" WS-I.
"#,
    );
    let joined = out.join("\n");
    for (line, want) in [
        ("RELABEL=1", "a run-time row can be renamed"),
        ("ICON-DESIGNED=0", "a designed row's icon cannot"),
        ("REMOVE-DESIGNED=0", "nor removed"),
        ("SHADOW-DESIGNED=0", "nor replaced by a run-time row"),
        ("HAS-HOME=1", "HasItem answers for designed rows too"),
        ("REMOVE=1", "a run-time row can be removed"),
        ("CHILD-GONE=0", "and its children go with it"),
        ("AFTER-CLEAR=000", "Clear() empties the program's rows"),
    ] {
        assert!(joined.contains(line), "{want}: {joined}");
    }
    assert!(
        ups.iter().all(|u| !u.prop.eq_ignore_ascii_case("Items")),
        "a SideMenu has no Items; nothing may write it"
    );
    assert!(final_rows(&ups).is_empty(), "Clear() left no rows behind");
}

/// A menu designed in the RAD shows in the designer and the preview; the
/// running program translates its labels and holds rows shut until the
/// application is set up. `Clear()` removes the program's own rows and leaves
/// what it set on the designed ones.
#[test]
fn a_designed_row_is_relabelled_and_disabled_and_clear_keeps_that() {
    let (out, ups) = run(
        r#"
           MOVE MENU-1::SetItemLabel("home", "Início") TO WS-OK.
           DISPLAY "LABEL-DESIGNED=" WS-OK.
           MOVE MENU-1::SetItemEnabled("docs-new", "0") TO WS-OK.
           DISPLAY "SHUT-NESTED=" WS-OK.
           MOVE MENU-1::SetItemAction("home", "event") TO WS-OK.
           DISPLAY "ACTION-DESIGNED=" WS-OK.
           MOVE MENU-1::AddItem("c1", "Chat") TO WS-OK.
           INVOKE MENU-1 "Clear".
           MOVE MENU-1::GetCount() TO WS-I.
           DISPLAY "AFTER-CLEAR=" WS-I.
"#,
    );
    let joined = out.join("\n");
    for (line, want) in [
        ("LABEL-DESIGNED=1", "a designed row takes a label"),
        ("SHUT-NESTED=1", "and a nested one can be shut"),
        ("ACTION-DESIGNED=0", "but its action stays the developer's"),
        ("AFTER-CLEAR=000", "the overlays are not counted as the program's rows"),
    ] {
        assert!(joined.contains(line), "{want}: {joined}");
    }
    let mut docs = MenuItem::new_action("docs", "Documents");
    docs.items.push(MenuItem::new_action("docs-new", "New"));
    let merged = merge_rows(&[MenuItem::new_action("home", "Home"), docs], &final_rows(&ups));
    assert_eq!(merged.len(), 2, "no row added, and Clear() took c1: {merged:?}");
    assert_eq!(merged[0].label, "Início", "the label survived Clear()");
    assert!(!merged[1].items[0].enabled, "so did the shut row");
}

/// `ActivateItem(id)` asks the shell to act on a row as a click would: a
/// known row answers 1 and writes a fresh request each time (so the same row
/// can be activated twice); an unknown one answers 0 and writes nothing.
#[test]
fn activate_item_writes_a_fresh_request_per_call() {
    let (display, ups) = run(r#"
           MOVE MENU-1::ActivateItem("docs-new") TO WS-OK
           DISPLAY "A=" WS-OK
           MOVE MENU-1::ActivateItem("docs-new") TO WS-OK
           MOVE MENU-1::ActivateItem("nope") TO WS-OK
           DISPLAY "B=" WS-OK"#);
    assert!(display.iter().any(|l| l.trim() == "A=1"), "{display:?}");
    assert!(display.iter().any(|l| l.trim() == "B=0"), "{display:?}");
    let requests: Vec<&str> = ups
        .iter()
        .filter(|u| u.prop == cobolt_forms::menu::runtime::ACTIVATE_ITEM_PROP)
        .map(|u| u.value.as_str())
        .collect();
    assert_eq!(requests, ["docs-new#1", "docs-new#2"]);
}
