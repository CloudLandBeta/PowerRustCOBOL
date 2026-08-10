// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Spec 049 R28/R29/AC18 — the `super` receiver, headless through a live
//! supervisor: a child form reads its opener's published properties
//! (`super::Title`), writes them through the supervisor (`MOVE … TO
//! super::X` → write-through + HostAction), drives the parent window
//! (`super::"SetWindowState"`), and an unbound `super` raises the standard
//! NULL error.

use std::sync::mpsc;
use std::time::Duration;

use cobolt_lexer::{tokenize, SourceFormat};
use cobolt_parser::{parse, Severity};
use cobolt_runtime::form_host::{FormRequest, FormSupervisor, HostAction, ROOT_HANDLE};
use cobolt_runtime::Interpreter;

fn program(src: &str) -> cobolt_ast::program::Program {
    let result = parse(tokenize(src, SourceFormat::Free));
    assert!(
        result
            .diagnostics
            .iter()
            .all(|d| d.severity != Severity::Error),
        "parse errors: {:?}",
        result.diagnostics
    );
    result.program.expect("no program")
}

/// A supervisor host thread that lives across several interpreters. Returns
/// (request sender, join handle yielding the recorded HostActions).
fn spawn_host() -> (
    mpsc::Sender<FormRequest>,
    std::thread::JoinHandle<Vec<HostAction>>,
) {
    let (req_tx, req_rx) = mpsc::channel::<FormRequest>();
    let host = std::thread::spawn(move || {
        let mut sup = FormSupervisor::new("MAIN-FORM", "MAIN-FORM");
        let mut actions = Vec::new();
        loop {
            match req_rx.recv_timeout(Duration::from_millis(10)) {
                Ok(req) => actions.extend(sup.handle_request(req)),
                Err(mpsc::RecvTimeoutError::Timeout) => continue,
                Err(mpsc::RecvTimeoutError::Disconnected) => break,
            }
        }
        actions
    });
    (req_tx, host)
}

/// Run `src` as the form `form_object` under `handle`, with an optional
/// `super` binding, seeding `form_props` as its form entry.
fn run_form(
    src: &str,
    req_tx: mpsc::Sender<FormRequest>,
    handle: &str,
    form_object: &str,
    super_handle: Option<&str>,
    form_props: Vec<(&str, &str)>,
) -> (Vec<String>, Result<(), String>) {
    let (_event_tx, event_rx) = mpsc::channel();
    let (state_tx, _state_rx) = mpsc::channel();
    let (display_tx, display_rx) = mpsc::channel();
    let (_closed_tx, closed_rx) = mpsc::channel::<String>();
    let mut interp = Interpreter::new_with_channels(program(src), event_rx, state_tx, display_tx);
    interp.set_form_host(req_tx, handle, form_object, closed_rx);
    if let Some(sh) = super_handle {
        interp.set_super_form(sh);
    }
    interp.seed_objects(vec![(
        form_object.to_string(),
        "Form".to_string(),
        form_props
            .into_iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect::<Vec<_>>(),
    )]);
    let run = interp.run().map_err(|e| e.to_string());
    (display_rx.try_iter().collect(), run)
}

#[test]
fn super_reads_writes_and_drives_the_opener() {
    // AC18 — the parent (root) publishes its designed Title; an async child
    // bound to it reads super::Title, rewrites it (write-through), reads the
    // new value back, and minimizes the parent window.
    let (req_tx, host) = spawn_host();

    // Parent: seeds + publishes, then opens the child (mints W1).
    let parent_src = r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. MAIN-FORM.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 WS-H PIC X(8).
       PROCEDURE DIVISION.
           INVOKE ME::"OpenFormAsync"("DETAIL") RETURNING WS-H.
           DISPLAY "CHILD=" WS-H.
           STOP RUN.
"#;
    let (parent_display, parent_run) = run_form(
        parent_src,
        req_tx.clone(),
        ROOT_HANDLE,
        "MAIN-FORM",
        None,
        vec![("Title", "Root Title"), ("Width", "800")],
    );
    parent_run.expect("parent run");
    assert!(
        parent_display.iter().any(|l| l.contains("CHILD=W1")),
        "the child handle must be W1: {parent_display:?}"
    );

    // Child: bound to the opener (R29 — the load path binds super).
    let child_src = r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. DETAIL-FORM.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 WS-T PIC X(12).
       PROCEDURE DIVISION.
           MOVE super::Title TO WS-T.
           DISPLAY "T1=" WS-T.
           MOVE "From Child" TO super::Title.
           MOVE super::Title TO WS-T.
           DISPLAY "T2=" WS-T.
           INVOKE super::"SetWindowState"("Minimized").
           STOP RUN.
"#;
    let (child_display, child_run) = run_form(
        child_src,
        req_tx.clone(),
        "W1",
        "DETAIL-FORM",
        Some(ROOT_HANDLE),
        vec![("Title", "Child")],
    );
    child_run.expect("child run");
    assert!(
        child_display.iter().any(|l| l.contains("T1=Root Title")),
        "super::Title must read the parent's published title: {child_display:?}"
    );
    assert!(
        child_display.iter().any(|l| l.contains("T2=From Child")),
        "the write-through must be readable back: {child_display:?}"
    );

    drop(req_tx);
    let actions = host.join().expect("host thread");
    assert!(
        actions.iter().any(|a| matches!(
            a,
            HostAction::SetFormProperty { handle, key, .. }
                if handle == ROOT_HANDLE && key.eq_ignore_ascii_case("Title")
        )),
        "the super write must surface as a HostAction on the PARENT: {actions:?}"
    );
    assert!(
        actions.iter().any(|a| matches!(
            a,
            HostAction::SetWindowState { handle, state }
                if handle == ROOT_HANDLE && state == "Minimized"
        )),
        "super::\"SetWindowState\" must target the PARENT window: {actions:?}"
    );
    println!(
        "049 AC18 — child W1 read super::Title='Root Title', rewrote it \
         ('From Child' read back through the supervisor), minimized the \
         parent; {} host action(s) recorded, both targeting W0",
        actions.len()
    );
}

#[test]
fn async_child_super_goes_null_when_the_opener_closes() {
    // AC26/R46 — the child reads its opener's Title; the opener closes; the
    // SAME reference now raises the standard NULL error and the child keeps
    // running (the DISPLAY after the failed branch proves liveness via
    // a second program run).
    let (req_tx, req_rx) = mpsc::channel::<FormRequest>();
    let (closed_tx, closed_rx) = mpsc::channel::<String>();
    let host = std::thread::spawn(move || {
        let mut sup = FormSupervisor::new("MAIN-FORM", "MAIN-FORM");
        let mut actions = Vec::new();
        loop {
            match req_rx.recv_timeout(Duration::from_millis(10)) {
                Ok(req) => {
                    let acts = sup.handle_request(req);
                    for a in &acts {
                        if let HostAction::NotifyClosed { handle } = a {
                            let _ = closed_tx.send(handle.clone());
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

    // A (root) opens B; B opens C — so C's opener (B) is not the root and
    // can close without R27 closing everything.
    let (_d, r) = run_form(
        r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. MAIN-FORM.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 WS-H PIC X(8).
       PROCEDURE DIVISION.
           INVOKE ME::"OpenFormAsync"("FORM-B") RETURNING WS-H.
           STOP RUN.
"#,
        req_tx.clone(),
        ROOT_HANDLE,
        "MAIN-FORM",
        None,
        vec![],
    );
    r.expect("A runs");
    let (_d, r) = run_form(
        r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. FORM-B.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 WS-H PIC X(8).
       PROCEDURE DIVISION.
           INVOKE ME::"OpenFormAsync"("FORM-C") RETURNING WS-H.
           STOP RUN.
"#,
        req_tx.clone(),
        "W1",
        "FORM-B",
        Some(ROOT_HANDLE),
        vec![("Title", "B Alive")],
    );
    r.expect("B runs");

    // C, part 1 — the opener is alive: super::Title reads.
    let (_event_tx, event_rx) = mpsc::channel();
    let (state_tx, _state_rx) = mpsc::channel();
    let (display_tx, display_rx) = mpsc::channel();
    let src_read = r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. FORM-C.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 WS-T PIC X(12).
       PROCEDURE DIVISION.
           MOVE super::Title TO WS-T.
           DISPLAY "ALIVE=" WS-T.
           STOP RUN.
"#;
    let mut c1 = Interpreter::new_with_channels(program(src_read), event_rx, state_tx, display_tx);
    let (_ct1, cr1) = mpsc::channel::<String>();
    c1.set_form_host(req_tx.clone(), "W2", "FORM-C", cr1);
    c1.set_super_form("W1");
    c1.run().expect("C reads while B lives");
    // The interpreter holds a clone of req_tx — drop it, or the host loop
    // never sees the disconnect and join() hangs.
    drop(c1);
    let d: Vec<String> = display_rx.try_iter().collect();
    assert!(
        d.iter().any(|l| l.contains("ALIVE=B Alive")),
        "pre-close read works: {d:?}"
    );

    // B closes (the supervisor broadcasts NotifyClosed{W1}).
    {
        let (rtx, rrx) = mpsc::channel();
        req_tx
            .send(FormRequest::HandleMethod {
                handle: "W1".into(),
                method: "Close".into(),
                args: vec![],
                reply: rtx,
            })
            .unwrap();
        rrx.recv().expect("close reply").expect("close ok");
    }
    // Wait for the broadcast, then hand it to C's second run.
    let closed = closed_rx
        .recv_timeout(Duration::from_secs(2))
        .expect("NotifyClosed broadcast");
    assert_eq!(closed, "W1");

    // C, part 2 — the same reference now raises the standard NULL error,
    // and the interpreter itself is alive enough to raise it cleanly.
    let (_event_tx, event_rx) = mpsc::channel();
    let (state_tx, _state_rx) = mpsc::channel();
    let (display_tx, _display_rx) = mpsc::channel();
    let mut c2 = Interpreter::new_with_channels(program(src_read), event_rx, state_tx, display_tx);
    let (ct2, cr2) = mpsc::channel::<String>();
    ct2.send("W1".to_string()).unwrap(); // the broadcast reaches C
    c2.set_form_host(req_tx.clone(), "W2", "FORM-C", cr2);
    c2.set_super_form("W1");
    let err = c2.run().expect_err("post-close super must error");
    assert!(
        err.to_string().contains("super is NULL"),
        "the standard NULL error, not a stale-handle one: {err}"
    );
    drop(c2);

    drop(req_tx);
    let _ = host.join();
    println!(
        "049 AC26/R46 — pre-close super::Title='B Alive'; after Close(W1) the \
         broadcast NULLed super and the SAME read raised 'super is NULL' \
         (standard error, child interpreter alive to raise it)"
    );
}

#[test]
fn super_chain_walks_one_loader_per_step() {
    // AC14 — A opens B opens C. In C: super::Title reads B's,
    // super::super::Title reads A's, assigning super::Title changes B's, and
    // one step past the root raises the NULL error.
    let (req_tx, host) = spawn_host();

    let open = |src: &str,
                handle: &str,
                obj: &str,
                sup: Option<&str>,
                props: Vec<(&str, &str)>| {
        run_form(src, req_tx.clone(), handle, obj, sup, props)
    };

    // A (root): publishes Title, opens B.
    let (_d, run_a) = open(
        r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. MAIN-FORM.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 WS-H PIC X(8).
       PROCEDURE DIVISION.
           INVOKE ME::"OpenFormAsync"("FORM-B") RETURNING WS-H.
           STOP RUN.
"#,
        ROOT_HANDLE,
        "MAIN-FORM",
        None,
        vec![("Title", "A Title")],
    );
    run_a.expect("A runs");

    // B (W1): publishes Title, opens C.
    let (_d, run_b) = open(
        r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. FORM-B.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 WS-H PIC X(8).
       PROCEDURE DIVISION.
           INVOKE ME::"OpenFormAsync"("FORM-C") RETURNING WS-H.
           STOP RUN.
"#,
        "W1",
        "FORM-B",
        Some(ROOT_HANDLE),
        vec![("Title", "B Title")],
    );
    run_b.expect("B runs");

    // C (W2): the AC14 program.
    let (display_c, run_c) = open(
        r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. FORM-C.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 WS-T PIC X(12).
       PROCEDURE DIVISION.
           MOVE super::Title TO WS-T.
           DISPLAY "B=" WS-T.
           MOVE super::super::Title TO WS-T.
           DISPLAY "A=" WS-T.
           MOVE "B Changed" TO super::Title.
           MOVE super::Title TO WS-T.
           DISPLAY "B2=" WS-T.
           STOP RUN.
"#,
        "W2",
        "FORM-C",
        Some("W1"),
        vec![("Title", "C Title")],
    );
    run_c.expect("C runs");
    assert!(
        display_c.iter().any(|l| l.contains("B=B Title")),
        "super::Title must read B's: {display_c:?}"
    );
    assert!(
        display_c.iter().any(|l| l.contains("A=A Title")),
        "super::super::Title must read A's: {display_c:?}"
    );
    assert!(
        display_c.iter().any(|l| l.contains("B2=B Changed")),
        "assigning super::Title must change B's: {display_c:?}"
    );

    // One step past the root: super::super in B (whose super is the root).
    let (_d, run_past) = open(
        r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. FORM-B.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 WS-T PIC X(12).
       PROCEDURE DIVISION.
           MOVE super::super::Title TO WS-T.
           STOP RUN.
"#,
        "W1",
        "FORM-B",
        Some(ROOT_HANDLE),
        vec![],
    );
    let err = run_past.expect_err("walking past the root must error");
    assert!(err.contains("super is NULL"), "R32 error text: {err}");

    drop(req_tx);
    let actions = host.join().expect("host thread");
    assert!(
        actions.iter().any(|a| matches!(
            a,
            HostAction::SetFormProperty { handle, key, .. }
                if handle == "W1" && key.eq_ignore_ascii_case("Title")
        )),
        "the depth-1 write must land on B (W1): {actions:?}"
    );
    println!(
        "049 AC14 — chain A(W0)→B(W1)→C(W2): super::Title='B Title', \
         super::super::Title='A Title', write changed B (verified by re-read \
         + HostAction on W1), and one step past the root raised 'super is NULL'"
    );
}

#[test]
fn menu_open_collapse_drive_the_pane_through_super() {
    // AC24 (COBOL half) — `super::<menu-id>::Collapse()` / `Open()` reach the
    // supervisor as pane-wide state changes with HostActions the shell
    // applies and persists.
    let (req_tx, host) = spawn_host();

    let (_d, r) = run_form(
        r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. MAIN-FORM.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 WS-H PIC X(8).
       PROCEDURE DIVISION.
           INVOKE ME::"OpenFormAsync"("DETAIL") RETURNING WS-H.
           STOP RUN.
"#,
        req_tx.clone(),
        ROOT_HANDLE,
        "MAIN-FORM",
        None,
        vec![],
    );
    r.expect("parent runs");

    let (_d, r) = run_form(
        r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. DETAIL-FORM.
       PROCEDURE DIVISION.
           super::SIDE-1::Collapse().
           super::SIDE-1::Open().
           super::SIDE-1::Collapse().
           STOP RUN.
"#,
        req_tx.clone(),
        "W1",
        "DETAIL-FORM",
        Some(ROOT_HANDLE),
        vec![],
    );
    r.expect("child drives the pane");

    drop(req_tx);
    let actions = host.join().expect("host thread");
    let pane_actions: Vec<bool> = actions
        .iter()
        .filter_map(|a| match a {
            HostAction::SetMenuPaneCollapsed { collapsed } => Some(*collapsed),
            _ => None,
        })
        .collect();
    assert_eq!(
        pane_actions,
        vec![true, false, true],
        "Collapse/Open/Collapse in order: {actions:?}"
    );
    println!(
        "049 AC24 (COBOL half) — super::SIDE-1::Collapse/Open/Collapse produced \
         pane actions [true, false, true]; persistence rides the shell glue (R9)"
    );
}

#[test]
fn unbound_super_raises_the_null_error() {
    // R32 shape (main form / no parent): any reference through super errors.
    let (req_tx, host) = spawn_host();
    let src = r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. MAIN-FORM.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 WS-T PIC X(12).
       PROCEDURE DIVISION.
           MOVE super::Title TO WS-T.
           STOP RUN.
"#;
    let (_display, run) = run_form(
        src,
        req_tx.clone(),
        ROOT_HANDLE,
        "MAIN-FORM",
        None, // no super binding — the main form
        vec![],
    );
    let err = run.expect_err("an unbound super read must error");
    assert!(
        err.contains("super is NULL"),
        "the standard NULL-super error is raised: {err}"
    );

    // The write and the method call raise the same error.
    let src_w = r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. MAIN-FORM.
       PROCEDURE DIVISION.
           MOVE "X" TO super::Title.
           STOP RUN.
"#;
    let (_d, run_w) = run_form(src_w, req_tx.clone(), ROOT_HANDLE, "MAIN-FORM", None, vec![]);
    assert!(run_w.expect_err("write").contains("super is NULL"));

    let src_m = r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. MAIN-FORM.
       PROCEDURE DIVISION.
           INVOKE super::"Close"().
           STOP RUN.
"#;
    let (_d, run_m) = run_form(src_m, req_tx.clone(), ROOT_HANDLE, "MAIN-FORM", None, vec![]);
    assert!(run_m.expect_err("method").contains("super is NULL"));

    drop(req_tx);
    let _ = host.join();
    println!("049 R32 shape — unbound super: read, write and method all raise 'super is NULL' (3/3)");
}
