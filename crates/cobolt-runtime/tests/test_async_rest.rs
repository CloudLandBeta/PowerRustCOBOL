// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Async I/O for RestClient — spec 032.
//!
//! Drives the real `COBOL-WAIT-EVENT` loop with a `RestClient` control against
//! deliberately-controlled mock HTTP servers (a `TcpListener` on an ephemeral
//! `127.0.0.1` port — no external network). Verifies that:
//!
//! - the event loop keeps dispatching (`onTick`) while a request is in flight,
//!   then delivers `onComplete` with `Busy` cleared and `ResponseBody`/
//!   `StatusCode` written (AC1/AC2);
//! - a transport failure delivers `onError` (AC2);
//! - a stalled server fires `onTimeout` after `TimeoutMs`, no `Cancel()` (AC4);
//! - `Cancel()` mid-flight fires `onCancelled` and the late worker result is
//!   discarded — no property corruption (AC3);
//! - `Mode = Sync` restores the original blocking, same-statement behaviour
//!   (AC7 for REST).

use std::io::{Read, Write};
use std::net::TcpListener;
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

use cobolt_lexer::{tokenize, SourceFormat};
use cobolt_parser::{parse, Severity};
use cobolt_runtime::{FormEvent, Interpreter, StateUpdate};

// ── Mock servers ───────────────────────────────────────────────────────────────

/// Accept one connection, read the request, wait for `release` to signal, then
/// reply `200 OK` with `body`. Returns the bound port.
fn gated_server(body: &'static str, release: mpsc::Receiver<()>) -> u16 {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    thread::spawn(move || {
        if let Ok((mut stream, _)) = listener.accept() {
            let mut buf = [0u8; 2048];
            let _ = stream.read(&mut buf);
            let _ = release.recv(); // hold the request open until released
            let resp = format!(
                "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                body.len(),
                body
            );
            let _ = stream.write_all(resp.as_bytes());
            let _ = stream.flush();
        }
    });
    port
}

/// Accept one connection, read the request, reply `200 OK` immediately.
fn immediate_server(body: &'static str) -> u16 {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    thread::spawn(move || {
        if let Ok((mut stream, _)) = listener.accept() {
            let mut buf = [0u8; 2048];
            let _ = stream.read(&mut buf);
            let resp = format!(
                "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                body.len(),
                body
            );
            let _ = stream.write_all(resp.as_bytes());
            let _ = stream.flush();
        }
    });
    port
}

/// Accept one connection, read the request, then close it with no response
/// (a transport-level failure the client sees as a broken connection).
fn closing_server() -> u16 {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    thread::spawn(move || {
        if let Ok((mut stream, _)) = listener.accept() {
            let mut buf = [0u8; 2048];
            let _ = stream.read(&mut buf);
            drop(stream); // close without replying
        }
    });
    port
}

/// Accept one connection, read the request, then hold it open silently (never
/// reply) so the interpreter's timeout sweep is what ends the operation.
fn silent_server() -> u16 {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    thread::spawn(move || {
        if let Ok((stream, _)) = listener.accept() {
            let mut stream = stream;
            let mut buf = [0u8; 2048];
            let _ = stream.read(&mut buf);
            thread::sleep(Duration::from_secs(30)); // hold open, never respond
            drop(stream);
        }
    });
    port
}

// ── COBOL driver program ───────────────────────────────────────────────────────

/// A form event loop: fire `GET` on REST-1 at start, then dispatch lifecycle
/// events. `onClick` cancels the in-flight request (used by the cancel test).
/// `<URL>` is substituted per test. Seeded control props (Mode/TimeoutMs) are
/// injected via `seed_objects`.
fn driver_src(url: &str) -> String {
    format!(
        r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. ASYNCGET.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 COBOL-EVENT-ID    PIC X(30).
       01 COBOL-CONTROL-ID  PIC X(30).
       01 COBOL-QUIT        PIC 9 VALUE 0.
       01 WS-URL            PIC X(80) VALUE "{url}".
       PROCEDURE DIVISION.
       MAIN.
           INVOKE REST-1 "GET" USING WS-URL
           PERFORM UNTIL COBOL-QUIT = 1
               CALL "COBOL-WAIT-EVENT"
                   USING COBOL-EVENT-ID COBOL-CONTROL-ID
               EVALUATE COBOL-EVENT-ID
                   WHEN "onTick"
                       DISPLAY "TICK"
                   WHEN "onClick"
                       INVOKE REST-1 "CANCEL"
                   WHEN "onComplete"
                       DISPLAY "DONE"
                       MOVE 1 TO COBOL-QUIT
                   WHEN "onError"
                       DISPLAY "ERR"
                       MOVE 1 TO COBOL-QUIT
                   WHEN "onTimeout"
                       DISPLAY "TIMEOUT"
                       MOVE 1 TO COBOL-QUIT
                   WHEN "onCancelled"
                       DISPLAY "CANCELLED"
                       MOVE 1 TO COBOL-QUIT
               END-EVALUATE
           END-PERFORM.
           STOP RUN.
"#
    )
}

fn parse_program(src: &str) -> cobolt_ast::program::Program {
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

struct Harness {
    event_tx: mpsc::Sender<FormEvent>,
    state_rx: mpsc::Receiver<StateUpdate>,
    display_rx: mpsc::Receiver<String>,
    handle: thread::JoinHandle<()>,
}

/// Launch the driver in a background thread wired to fresh channels, with the
/// given seeded properties on REST-1 (type `RestClient`).
fn launch(url: &str, seed_props: Vec<(&str, &str)>) -> Harness {
    let program = parse_program(&driver_src(url));
    let (event_tx, event_rx) = mpsc::channel::<FormEvent>();
    let (state_tx, state_rx) = mpsc::channel::<StateUpdate>();
    let (display_tx, display_rx) = mpsc::channel::<String>();
    let seeded: Vec<(String, String)> = seed_props
        .into_iter()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect();
    let handle = thread::spawn(move || {
        let mut interp = Interpreter::new_with_channels(program, event_rx, state_tx, display_tx);
        if !seeded.is_empty() {
            interp.seed_objects(vec![(
                "REST-1".to_string(),
                "RestClient".to_string(),
                seeded,
            )]);
        }
        let _ = interp.run();
    });
    Harness {
        event_tx,
        state_rx,
        display_rx,
        handle,
    }
}

fn drain_state(rx: &mpsc::Receiver<StateUpdate>) -> Vec<(String, String, String)> {
    rx.try_iter()
        .map(|u| (u.ctrl_id, u.prop, u.value.trim().to_string()))
        .collect()
}

fn last_value(updates: &[(String, String, String)], ctrl: &str, prop: &str) -> Option<String> {
    updates
        .iter()
        .filter(|(c, p, _)| c == ctrl && p == prop)
        .map(|(_, _, v)| v.clone())
        .last()
}

// ── Tests ───────────────────────────────────────────────────────────────────────

#[test]
fn async_get_stays_responsive_then_completes() {
    let (release_tx, release_rx) = mpsc::channel();
    let port = gated_server("HELLO-BODY", release_rx);
    let url = format!("http://127.0.0.1:{port}/");
    let h = launch(&url, vec![]); // no Mode → async by default (legacy-form path)

    // While the request is in flight, three timer ticks must dispatch.
    for _ in 0..3 {
        h.event_tx.send(FormEvent::new("TIMER-1", "onTick")).unwrap();
    }
    let mut lines = Vec::new();
    let mut ticks = 0;
    while ticks < 3 {
        let l = h
            .display_rx
            .recv_timeout(Duration::from_secs(5))
            .expect("event loop did not dispatch onTick while request was in flight")
            .trim()
            .to_string();
        if l == "TICK" {
            ticks += 1;
        }
        lines.push(l);
    }

    // Now let the server answer; the completion must arrive as onComplete.
    release_tx.send(()).unwrap();
    loop {
        match h.display_rx.recv_timeout(Duration::from_secs(5)) {
            Ok(l) => {
                let l = l.trim().to_string();
                let done = l == "DONE";
                lines.push(l);
                if done {
                    break;
                }
            }
            Err(_) => panic!("never received onComplete; got: {lines:?}"),
        }
    }
    h.handle.join().expect("interpreter thread panicked");

    assert_eq!(
        lines,
        vec!["TICK", "TICK", "TICK", "DONE"],
        "ticks must dispatch before the response and completion arrives once"
    );

    let updates = drain_state(&h.state_rx);
    assert_eq!(
        last_value(&updates, "REST-1", "Busy").as_deref(),
        Some("false"),
        "Busy must return to false on completion; updates: {updates:?}"
    );
    assert!(
        updates
            .iter()
            .any(|(c, p, v)| c == "REST-1" && p == "Busy" && v == "true"),
        "Busy must have been set to true while in flight; updates: {updates:?}"
    );
    assert_eq!(
        last_value(&updates, "REST-1", "ResponseBody").as_deref(),
        Some("HELLO-BODY")
    );
    assert_eq!(
        last_value(&updates, "REST-1", "StatusCode").as_deref(),
        Some("200")
    );
}

#[test]
fn async_get_transport_failure_fires_on_error() {
    let port = closing_server();
    let url = format!("http://127.0.0.1:{port}/");
    let h = launch(&url, vec![]);

    let mut got = None;
    for _ in 0..50 {
        match h.display_rx.recv_timeout(Duration::from_secs(5)) {
            Ok(l) => {
                let l = l.trim().to_string();
                if l == "ERR" || l == "DONE" {
                    got = Some(l);
                    break;
                }
            }
            Err(_) => break,
        }
    }
    h.handle.join().expect("interpreter thread panicked");
    assert_eq!(got.as_deref(), Some("ERR"), "a closed connection → onError");

    let updates = drain_state(&h.state_rx);
    assert_eq!(
        last_value(&updates, "REST-1", "Busy").as_deref(),
        Some("false"),
        "Busy cleared after error; updates: {updates:?}"
    );
}

#[test]
fn async_get_times_out_without_cancel() {
    let port = silent_server();
    let url = format!("http://127.0.0.1:{port}/");
    // Short interpreter timeout; the transport backstop is much longer, so the
    // sweep is what ends the op → onTimeout (not onError).
    let h = launch(&url, vec![("Mode", "Async"), ("TimeoutMs", "200")]);

    let mut got = None;
    match h.display_rx.recv_timeout(Duration::from_secs(5)) {
        Ok(l) => got = Some(l.trim().to_string()),
        Err(_) => {}
    }
    h.handle.join().expect("interpreter thread panicked");
    assert_eq!(
        got.as_deref(),
        Some("TIMEOUT"),
        "a stalled server must fire onTimeout"
    );

    let updates = drain_state(&h.state_rx);
    assert_eq!(
        last_value(&updates, "REST-1", "Busy").as_deref(),
        Some("false"),
        "Busy cleared after timeout; updates: {updates:?}"
    );
}

#[test]
fn cancel_mid_flight_fires_on_cancelled_and_discards_late_result() {
    let (release_tx, release_rx) = mpsc::channel();
    let port = gated_server("LATE-BODY", release_rx);
    let url = format!("http://127.0.0.1:{port}/");
    let h = launch(&url, vec![]);

    // Give the worker a moment to connect (request in flight), then cancel via
    // an onClick the handler maps to INVOKE REST-1 "CANCEL".
    thread::sleep(Duration::from_millis(150));
    h.event_tx
        .send(FormEvent::new("BTN-CANCEL", "onClick"))
        .unwrap();

    let msg = h
        .display_rx
        .recv_timeout(Duration::from_secs(5))
        .expect("no cancellation event")
        .trim()
        .to_string();
    assert_eq!(msg, "CANCELLED", "Cancel() must fire onCancelled");

    // Release the server so the worker result arrives late; it must be discarded.
    release_tx.send(()).unwrap();
    h.handle.join().expect("interpreter thread panicked");

    let updates = drain_state(&h.state_rx);
    assert_eq!(
        last_value(&updates, "REST-1", "Busy").as_deref(),
        Some("false"),
        "Busy cleared by cancel; updates: {updates:?}"
    );
    assert!(
        last_value(&updates, "REST-1", "ResponseBody").is_none(),
        "a cancelled op's late body must never be written; updates: {updates:?}"
    );
}

#[test]
fn sync_mode_returns_body_in_same_statement() {
    // Mode = Sync restores blocking behaviour: GET RETURNING gets the body now.
    let port = immediate_server("SYNC-BODY");
    let url = format!("http://127.0.0.1:{port}/");
    let src = format!(
        r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. SYNCGET.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 WS-URL   PIC X(80) VALUE "{url}".
       01 WS-BODY  PIC X(64).
       PROCEDURE DIVISION.
       MAIN.
           INVOKE REST-1 "GET" USING WS-URL RETURNING WS-BODY
           DISPLAY WS-BODY
           STOP RUN.
"#
    );
    let program = parse_program(&src);
    let (_event_tx, event_rx) = mpsc::channel();
    let (state_tx, _state_rx) = mpsc::channel();
    let (display_tx, display_rx) = mpsc::channel::<String>();
    let mut interp = Interpreter::new_with_channels(program, event_rx, state_tx, display_tx);
    interp.seed_objects(vec![(
        "REST-1".to_string(),
        "RestClient".to_string(),
        vec![("Mode".to_string(), "Sync".to_string())],
    )]);
    interp.run().expect("run failed");

    let lines: Vec<String> = display_rx.try_iter().map(|l| l.trim().to_string()).collect();
    assert!(
        lines.iter().any(|l| l == "SYNC-BODY"),
        "sync GET must return the body in the same statement; got: {lines:?}"
    );
}
