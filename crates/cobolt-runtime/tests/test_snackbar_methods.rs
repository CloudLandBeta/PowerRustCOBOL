// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Spec 055 T8 — `Show()` and `DismissAll()`, driven from real COBOL.
//!
//! The interpreter cannot see the host crate, so a control method reaches the
//! stack the way `PlayAnimation` already does: `obj_set` writes a pseudo-property
//! and the `StateUpdate` crosses the channel. What these tests pin is the half
//! that lives here — that each `Show()` produces its **own** request, which is
//! what makes `Show()` a factory (D2) rather than a flag.
//!
//! A bare flag would have coalesced two calls into one notification, and AC3 —
//! "`Show()` twice in one handler yields two" — would have failed silently in
//! the running form with nothing here to catch it.

use std::sync::mpsc;

use cobolt_lexer::{tokenize, SourceFormat};
use cobolt_parser::{parse, Severity};
use cobolt_runtime::channels::StateUpdate;
use cobolt_runtime::Interpreter;

/// Run `src` and return (DISPLAY lines, state updates the host would receive).
fn run(src: &str) -> (Vec<String>, Vec<StateUpdate>) {
    let result = parse(tokenize(src, SourceFormat::Free));
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
        "SNACK-1".to_owned(),
        // The host registers each control under its real ControlType.
        "Snackbar".to_owned(),
        vec![
            ("Text".to_owned(), String::new()),
            ("Category".to_owned(), "Info".to_owned()),
        ],
    )]);
    interp.run().expect("run failed");
    (display_rx.try_iter().collect(), state_rx.try_iter().collect())
}

fn requests<'a>(ups: &'a [StateUpdate], prop: &str) -> Vec<&'a StateUpdate> {
    ups.iter().filter(|u| u.prop.eq_ignore_ascii_case(prop)).collect()
}

#[test]
fn show_raises_one_notification_per_call() {
    let (_out, ups) = run(
        r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. T.
       PROCEDURE DIVISION.
           MOVE "Saved" TO SNACK-1::Text
           INVOKE SNACK-1::Show()
           MOVE "Saved again" TO SNACK-1::Text
           INVOKE SNACK-1::Show()
           STOP RUN.
"#,
    );
    let shows = requests(&ups, "_ShowSnackbar");
    eprintln!(
        "\n  Show() x2 → {} request(s): {:?}",
        shows.len(),
        shows.iter().map(|u| (&u.ctrl_id, &u.value)).collect::<Vec<_>>()
    );
    assert_eq!(shows.len(), 2, "AC3/D2: each Show() is its own request");
    // Distinct values — a repeated identical write is what would let a host
    // coalesce the two into one notification.
    assert_ne!(shows[0].value, shows[1].value, "each request must be distinguishable");
    assert_eq!(shows[0].value, "1");
    assert_eq!(shows[1].value, "2");
    for u in &shows {
        assert_eq!(u.ctrl_id, "SNACK-1");
    }

    // And the Text writes reached the control, so the SECOND notification mints
    // from the value that was current when its Show() ran (D2).
    let texts: Vec<&String> = ups
        .iter()
        .filter(|u| u.prop.eq_ignore_ascii_case("Text"))
        .map(|u| &u.value)
        .collect();
    assert_eq!(texts, vec!["Saved", "Saved again"]);
    eprintln!("  → AC3: 2 requests, values \"1\"/\"2\"; Text was {texts:?} at each call\n");
}

#[test]
fn dismiss_all_is_its_own_request() {
    let (_out, ups) = run(
        r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. T.
       PROCEDURE DIVISION.
           INVOKE SNACK-1::Show()
           INVOKE SNACK-1::Show()
           INVOKE SNACK-1::DismissAll()
           STOP RUN.
"#,
    );
    assert_eq!(requests(&ups, "_ShowSnackbar").len(), 2);
    let clears = requests(&ups, "_DismissAllSnackbar");
    assert_eq!(clears.len(), 1, "R9: one DismissAll, one request");
    assert_eq!(clears[0].ctrl_id, "SNACK-1");
    eprintln!(
        "\n  R9 — 2 Show() + 1 DismissAll() → {} show request(s), {} dismiss request(s)\n",
        2,
        clears.len()
    );
}

#[test]
fn show_parses_as_a_call_not_a_subscript() {
    // The known-method list exists for exactly this: an unlisted name has its
    // parens read as a collection subscript, so `SNACK-1::Show()` would have
    // meant "element 0 of Show" and the program would have run doing nothing.
    let (out, ups) = run(
        r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. T.
       PROCEDURE DIVISION.
           INVOKE SNACK-1::Show()
           DISPLAY "after-show"
           INVOKE SNACK-1::DismissAll()
           DISPLAY "after-dismiss"
           STOP RUN.
"#,
    );
    assert!(out.iter().any(|l| l.contains("after-show")), "program ran: {out:?}");
    assert!(out.iter().any(|l| l.contains("after-dismiss")), "program ran: {out:?}");
    assert_eq!(requests(&ups, "_ShowSnackbar").len(), 1, "the call dispatched");
    assert_eq!(requests(&ups, "_DismissAllSnackbar").len(), 1, "the call dispatched");
    eprintln!("\n  both methods dispatched and the program ran on: {out:?}\n");
}

#[test]
fn a_handler_may_set_properties_and_show_in_one_breath() {
    // The shape a real handler takes: build the message in COBOL (spec Q3 — no
    // value substitution in `Text`), set the category, then raise it.
    let (_out, ups) = run(
        r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. T.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 WS-MSG  PIC X(40).
       01 WS-NAME PIC X(10) VALUE "CUSTOMER".
       PROCEDURE DIVISION.
           STRING "Saved " DELIMITED BY SIZE
                  FUNCTION TRIM(WS-NAME) DELIMITED BY SIZE
                  INTO WS-MSG
           MOVE FUNCTION TRIM(WS-MSG) TO SNACK-1::Text
           MOVE "Error" TO SNACK-1::Category
           MOVE 2500 TO SNACK-1::Timeout
           INVOKE SNACK-1::Show()
           STOP RUN.
"#,
    );
    let get = |p: &str| -> Option<String> {
        ups.iter()
            .filter(|u| u.prop.eq_ignore_ascii_case(p))
            .next_back()
            .map(|u| u.value.clone())
    };
    assert_eq!(get("Text").as_deref(), Some("Saved CUSTOMER"));
    assert_eq!(get("Category").as_deref(), Some("Error"));
    assert_eq!(get("Timeout").as_deref(), Some("2500"));
    assert_eq!(requests(&ups, "_ShowSnackbar").len(), 1);

    // Order matters: every property write must reach the host BEFORE the
    // request, or the notification mints from stale values.
    let show_at = ups.iter().position(|u| u.prop.eq_ignore_ascii_case("_ShowSnackbar")).unwrap();
    for p in ["Text", "Category", "Timeout"] {
        let at = ups.iter().position(|u| u.prop.eq_ignore_ascii_case(p)).unwrap();
        assert!(at < show_at, "{p} must be written before the Show() request");
    }
    eprintln!(
        "\n  D2 — Text/Category/Timeout all written before the request (positions vs {show_at})\n"
    );
}

#[test]
fn show_still_means_make_visible_for_every_other_control() {
    // The collision guard. `Show()` was already the UNIVERSAL "make this control
    // visible" verb before the Snackbar existed, and spec §6 reuses the name for
    // something else entirely. Diverting it by class is only safe if the old
    // meaning survives untouched for every control that is not a Snackbar —
    // otherwise this feature silently breaks `BTN-1::Show()` in every existing
    // project.
    let result = parse(tokenize(
        r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. T.
       PROCEDURE DIVISION.
           INVOKE BTN-1::Show()
           INVOKE LBL-1::Show()
           INVOKE SNACK-1::Show()
           STOP RUN.
"#,
        SourceFormat::Free,
    ));
    let program = result.program.expect("no program");
    let (_event_tx, event_rx) = mpsc::channel();
    let (state_tx, state_rx) = mpsc::channel();
    let (display_tx, _display_rx) = mpsc::channel();
    let mut interp = Interpreter::new_with_channels(program, event_rx, state_tx, display_tx);
    interp.seed_objects(vec![
        ("BTN-1".to_owned(), "Button".to_owned(), vec![("Visible".to_owned(), "0".to_owned())]),
        ("LBL-1".to_owned(), "Label".to_owned(), vec![("Visible".to_owned(), "0".to_owned())]),
        ("SNACK-1".to_owned(), "Snackbar".to_owned(), vec![("Text".to_owned(), String::new())]),
    ]);
    interp.run().expect("run failed");
    let ups: Vec<StateUpdate> = state_rx.try_iter().collect();

    let visible_writes: Vec<(&String, &String)> = ups
        .iter()
        // `Visible` is canonicalised on the way out — "true", not "1".
        .filter(|u| {
            u.prop.eq_ignore_ascii_case("Visible")
                && matches!(u.value.as_str(), "1" | "true" | "TRUE")
        })
        .map(|u| (&u.ctrl_id, &u.value))
        .collect();
    eprintln!("\n  control    class      Show() did");
    eprintln!("  --------   --------   ----------------------");
    eprintln!("  BTN-1      Button     Visible := 1");
    eprintln!("  LBL-1      Label      Visible := 1");
    eprintln!("  SNACK-1    Snackbar   raised a notification");

    assert!(
        visible_writes.iter().any(|(id, _)| *id == "BTN-1"),
        "BTN-1::Show() must still set Visible: {ups:?}"
    );
    assert!(
        visible_writes.iter().any(|(id, _)| *id == "LBL-1"),
        "LBL-1::Show() must still set Visible: {ups:?}"
    );
    // …and the Snackbar wrote no `Visible` at all — it is non-visual, and a
    // `Visible` on it would be a property nothing reads.
    assert!(
        !ups.iter().any(|u| u.ctrl_id == "SNACK-1" && u.prop.eq_ignore_ascii_case("Visible")),
        "a Snackbar has no Visible to set"
    );
    assert_eq!(requests(&ups, "_ShowSnackbar").len(), 1);
    eprintln!("  → 2 controls kept the universal meaning; 1 Snackbar raised instead\n");
}
