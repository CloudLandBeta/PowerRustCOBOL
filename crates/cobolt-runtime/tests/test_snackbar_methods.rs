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

// ── Which button was pressed, end to end (operator, 2026-09-01) ──────────────
//
// The operator reported that a Snackbar's buttons "must return which one was
// clicked". `LastButtonId` / `LastButtonIndex` are written by the HOST and are
// runtime-only — never seeded at design time — so the whole claim rests on a
// chain nothing tested end to end: host writes a `StateUpdate` → interpreter
// folds it in before the handler runs → COBOL reads it as a member. This drives
// that chain with the demo form's own two buttons, in the operator's own syntax.

/// The handler shape `datagrid-form.cfrm` actually carries: read the member,
/// branch on it, and say what came back.
const BUTTON_HANDLER: &str = r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. SNACKBUTTONS.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 COBOL-EVENT-ID    PIC X(30).
       01 COBOL-CONTROL-ID  PIC X(30).
       01 COBOL-QUIT        PIC 9 VALUE 0.
       PROCEDURE DIVISION.
       MAIN.
           PERFORM UNTIL COBOL-QUIT = 1
               CALL "COBOL-WAIT-EVENT"
                   USING COBOL-EVENT-ID COBOL-CONTROL-ID
               EVALUATE SNACKBAR-1::LastButtonId
                   WHEN "retry"
                       DISPLAY "id=retry index=" SNACKBAR-1::LastButtonIndex
                   WHEN "later"
                       DISPLAY "id=later index=" SNACKBAR-1::LastButtonIndex
                   WHEN OTHER
                       DISPLAY "id=? " SNACKBAR-1::LastButtonId
               END-EVALUATE
           END-PERFORM.
           STOP RUN.
"#;

#[test]
fn a_handler_reads_which_button_was_pressed() {
    use cobolt_runtime::FormEvent;

    let result = parse(tokenize(BUTTON_HANDLER, SourceFormat::Free));
    assert!(
        result.diagnostics.iter().all(|d| d.severity != Severity::Error),
        "parse errors: {:?}",
        result.diagnostics
    );
    let program = result.program.expect("no program");

    let (event_tx, event_rx) = mpsc::channel::<FormEvent>();
    let (input_tx, input_rx) = mpsc::channel::<StateUpdate>();
    let (state_tx, _state_rx) = mpsc::channel::<StateUpdate>();
    let (display_tx, display_rx) = mpsc::channel::<String>();

    let handle = std::thread::spawn(move || {
        let mut interp =
            Interpreter::new_with_channels(program, event_rx, state_tx, display_tx);
        interp.set_input_channel(input_rx);
        // Seeded exactly as the host seeds it: the design-time properties only.
        // Neither LastButtonId nor LastButtonIndex exists yet — there is no
        // answer until a button is clicked.
        interp.seed_objects(vec![(
            "SNACKBAR-1".to_owned(),
            "Snackbar".to_owned(),
            vec![("Text".to_owned(), "Record saved".to_owned())],
        )]);
        let _ = interp.run();
    });

    // Press each of the demo form's buttons, one at a time and WAITING for the
    // handler to answer before the next — a human clicking, not a burst.
    // host.rs's ordering: both properties first, THEN the event.
    let mut lines: Vec<String> = Vec::new();
    for (id, index) in [("retry", 0usize), ("later", 1usize)] {
        input_tx
            .send(StateUpdate::new("SNACKBAR-1", "LastButtonId", id))
            .unwrap();
        input_tx
            .send(StateUpdate::new("SNACKBAR-1", "LastButtonIndex", index.to_string()))
            .unwrap();
        event_tx
            .send(FormEvent::new("SNACKBAR-1", "onButtonClick"))
            .unwrap();
        lines.push(
            display_rx
                .recv_timeout(std::time::Duration::from_secs(5))
                .expect("the handler must answer this click before the next is sent")
                .trim()
                .to_owned(),
        );
    }
    event_tx.send(FormEvent::quit()).unwrap();
    handle.join().expect("interpreter thread panicked");
    lines.extend(display_rx.try_iter().map(|l| l.trim().to_owned()));
    eprintln!("\n  clicked   index   what the handler read back");
    eprintln!("  -------   -----   --------------------------");
    for (id, index) in [("retry", 0usize), ("later", 1usize)] {
        let want = format!("id={id} index={index}");
        let got = lines.iter().find(|l| l.starts_with(&format!("id={id}")));
        eprintln!("  {id:<7}   {index:>5}   {}", got.map(String::as_str).unwrap_or("NOTHING"));
        assert_eq!(
            got.map(String::as_str),
            Some(want.as_str()),
            "the handler must read back button {id} at index {index}; got {lines:?}"
        );
    }
    assert!(
        !lines.iter().any(|l| l.starts_with("id=?")),
        "no click may fall through to WHEN OTHER: {lines:?}"
    );
    eprintln!("  → 2/2 buttons identified by id AND index in the handler\n");
}

#[test]
fn two_buttons_can_be_built_from_cobol() {
    // `Buttons` is ONE LINE PER BUTTON, and a COBOL literal cannot carry a
    // newline — so the only way to declare a second button at run time is to
    // STRING the lines together around one. `FUNCTION CHAR(11)` is ordinal 11,
    // which is LF. Without this recipe a handler can only ever set one button,
    // and the operator asked for two per notification.
    let (_out, ups) = run(
        r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. TWOBUTTONS.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 WS-LF       PIC X.
       01 WS-BUTTONS  PIC X(120).
       PROCEDURE DIVISION.
           MOVE FUNCTION CHAR(11) TO WS-LF
           MOVE SPACES TO WS-BUTTONS
           STRING "undo|Undo|undo|Left|true"   DELIMITED BY SIZE
                  WS-LF                        DELIMITED BY SIZE
                  "later|Later||None|false"    DELIMITED BY SIZE
                  INTO WS-BUTTONS
           MOVE WS-BUTTONS TO SNACK-1::Buttons
           INVOKE SNACK-1::Show()
           STOP RUN.
"#,
    );
    let written = requests(&ups, "Buttons");
    assert_eq!(written.len(), 1, "the handler wrote Buttons once: {ups:?}");
    let spec = written[0].value.clone();

    // What the host would mint from it.
    let (buttons, diag) = cobolt_forms::snackbar::parse_buttons(&spec);
    assert!(diag.is_none(), "two buttons is under the limit of {}", cobolt_forms::snackbar::MAX_BUTTONS);

    eprintln!("\n  #   id      text    icon   position   dismiss");
    eprintln!("  -   -----   -----   ----   --------   -------");
    for (i, b) in buttons.iter().enumerate() {
        eprintln!(
            "  {i}   {:<5}   {:<5}   {:<4}   {:<8}   {}",
            b.id, b.text, if b.icon.is_empty() { "-" } else { &b.icon }, format!("{:?}", b.position), b.dismiss
        );
    }
    assert_eq!(buttons.len(), 2, "STRING around FUNCTION CHAR(11) yields TWO buttons, got {buttons:?}");
    assert_eq!((buttons[0].id.as_str(), buttons[0].dismiss), ("undo", true));
    assert_eq!((buttons[1].id.as_str(), buttons[1].dismiss), ("later", false));
    assert_eq!(buttons[0].icon, "undo", "field 3 is the catalogue icon name");
    eprintln!("  → 2 buttons declared from COBOL, the second one non-dismissing\n");
}
