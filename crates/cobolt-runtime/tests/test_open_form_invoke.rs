// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Spec 037 T8/T11 — `me::"OpenFormSync"/"OpenFormAsync"` end-to-end through
//! a live [`FormSupervisor`] on a host thread, headless: handles bind to
//! RETURNING data-items, omitted comma-form parameters travel as
//! host-defaulted (`None`), a modal Sync call blocks the COBOL flow until
//! the child closes (NULL handle on resume), and invoking through a
//! closed/NULL windowHandler raises the standard runtime error.

use std::sync::mpsc;
use std::time::Duration;

use cobolt_lexer::{tokenize, SourceFormat};
use cobolt_parser::{parse, Severity};
use cobolt_runtime::form_host::{FormRequest, FormSupervisor, HostAction, ROOT_HANDLE};
use cobolt_runtime::Interpreter;

/// Run `src` against a supervisor host thread. `auto_close_modal_after`
/// closes any modal spawn after the delay (the "user closed the dialog").
/// Returns (display lines, host actions, run result).
fn run_with_supervisor(
    src: &str,
    auto_close_modal_after: Option<Duration>,
) -> (Vec<String>, Vec<HostAction>, Result<(), String>) {
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
    let (req_tx, req_rx) = mpsc::channel::<FormRequest>();
    let (closed_tx, closed_rx) = mpsc::channel::<String>();

    // Host thread: a real supervisor answering requests, forwarding
    // NotifyClosed broadcasts, optionally auto-closing modal children.
    let host = std::thread::spawn(move || {
        let mut sup = FormSupervisor::new("MAIN-FORM", "MAIN-FORM");
        let mut actions: Vec<HostAction> = Vec::new();
        let (late_tx, late_rx) = mpsc::channel::<String>();
        loop {
            // Deferred modal auto-closes land as plain handle ids.
            while let Ok(h) = late_rx.try_recv() {
                let acts = sup.try_close(&h);
                for a in &acts {
                    if let HostAction::NotifyClosed { handle } = a {
                        let _ = closed_tx.send(handle.clone());
                    }
                }
                actions.extend(acts);
            }
            match req_rx.recv_timeout(Duration::from_millis(10)) {
                Ok(req) => {
                    let acts = sup.handle_request(req);
                    for a in &acts {
                        match a {
                            HostAction::NotifyClosed { handle } => {
                                let _ = closed_tx.send(handle.clone());
                            }
                            HostAction::SpawnWindow { handle, modal, .. } => {
                                if *modal {
                                    if let Some(delay) = auto_close_modal_after {
                                        let tx = late_tx.clone();
                                        let h = handle.clone();
                                        std::thread::spawn(move || {
                                            std::thread::sleep(delay);
                                            let _ = tx.send(h);
                                        });
                                    }
                                }
                            }
                            _ => {}
                        }
                    }
                    actions.extend(acts);
                }
                Err(mpsc::RecvTimeoutError::Timeout) => continue,
                Err(mpsc::RecvTimeoutError::Disconnected) => break,
            }
        }
        actions
    });

    let mut interp = Interpreter::new_with_channels(program, event_rx, state_tx, display_tx);
    interp.set_form_host(req_tx, ROOT_HANDLE, "MAIN-FORM", closed_rx);
    let run = interp.run().map_err(|e| e.to_string());
    drop(interp); // drops req_tx → host loop ends
    let actions = host.join().expect("host thread");
    (display_rx.try_iter().collect(), actions, run)
}

fn spawns(actions: &[HostAction]) -> Vec<&HostAction> {
    actions
        .iter()
        .filter(|a| matches!(a, HostAction::SpawnWindow { .. }))
        .collect()
}

#[test]
fn open_form_invoke_binds_handles_and_defaults_omitted_params() {
    let src = r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. DEMO.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 WS-H1 PIC X(8).
       01 WS-H2 PIC X(8).
       PROCEDURE DIVISION.
           INVOKE ME::"OpenFormAsync"("DETAIL", "Maximized", 10, 20)
               RETURNING WS-H1.
           DISPLAY "H1=" WS-H1.
           INVOKE ME::"OpenFormAsync"("DETAIL") RETURNING WS-H2.
           DISPLAY "H2=" WS-H2.
           STOP RUN.
"#;
    let (display, actions, run) = run_with_supervisor(src, None);
    run.expect("clean run");
    let sp = spawns(&actions);
    assert_eq!(sp.len(), 2, "two instances spawned (R11): {actions:?}");
    // First call: provided params travel as-is; omitted width/height default
    // to the HOST's RAD lookup (None).
    let HostAction::SpawnWindow {
        handle: h1,
        window_state,
        x,
        y,
        width,
        height,
        modal,
        ..
    } = sp[0]
    else {
        unreachable!()
    };
    assert_eq!(window_state.as_deref(), Some("Maximized"));
    assert_eq!((*x, *y), (Some(10), Some(20)));
    assert_eq!((*width, *height), (None, None), "omitted ⇒ RAD default (R21)");
    assert!(!modal, "async is never modal (R20)");
    // Second call: everything omitted ⇒ everything defaulted.
    let HostAction::SpawnWindow {
        handle: h2,
        window_state: ws2,
        ..
    } = sp[1]
    else {
        unreachable!()
    };
    assert_eq!(ws2.as_deref(), None);
    assert_ne!(h1, h2, "distinct handles per instance (R11)");
    let joined = display.join("\n");
    assert!(
        joined.contains(&format!("H1={h1}")) && joined.contains(&format!("H2={h2}")),
        "RETURNING bound the handles: {joined}"
    );
    println!("spawned {h1} (Maximized,10,20,defaults) + {h2} (all defaults); DISPLAY: {joined}");
}

#[test]
fn open_form_invoke_modal_sync_blocks_until_child_closes() {
    let src = r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. DEMO.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 WS-H PIC X(8).
       PROCEDURE DIVISION.
           MOVE "X" TO WS-H.
           INVOKE ME::"OpenFormSync"("DIALOG") RETURNING WS-H.
           DISPLAY "AFTER=[" WS-H "]".
           STOP RUN.
"#;
    let start = std::time::Instant::now();
    let (display, actions, run) =
        run_with_supervisor(src, Some(Duration::from_millis(120)));
    run.expect("clean run");
    let elapsed = start.elapsed();
    assert!(
        elapsed >= Duration::from_millis(120),
        "the caller's flow blocked until the modal child closed (R28): {elapsed:?}"
    );
    let sp = spawns(&actions);
    assert_eq!(sp.len(), 1);
    let HostAction::SpawnWindow { modal, .. } = sp[0] else {
        unreachable!()
    };
    assert!(*modal, "comma form defaults modal=true (R21)");
    let joined = display.join("\n");
    assert!(
        joined.contains("AFTER=[") && !joined.contains("AFTER=[W"),
        "the handle is NULL after the modal child closed (R24): {joined}"
    );
    println!("modal block: {elapsed:?} ≥ 120ms; resumed with NULL handle: {joined}");
}

#[test]
fn open_form_invoke_null_handle_method_raises_runtime_error() {
    let src = r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. DEMO.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 WS-H PIC X(8).
       PROCEDURE DIVISION.
           INVOKE ME::"OpenFormAsync"("A") RETURNING WS-H.
           INVOKE WS-H "Close".
           INVOKE WS-H "Focus".
           STOP RUN.
"#;
    let (_display, actions, run) = run_with_supervisor(src, None);
    let err = run.expect_err("Focus through a closed handle must error (AC13)");
    assert!(
        err.to_ascii_lowercase().contains("closed")
            || err.to_ascii_lowercase().contains("null"),
        "standard runtime error names the closed/NULL handle: {err}"
    );
    assert!(
        actions
            .iter()
            .any(|a| matches!(a, HostAction::CloseWindow { .. })),
        "the Close itself was honoured first"
    );
    println!("NULL-handle invoke → runtime error: {err}");
}
