// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! What the INTERPRETED event loop costs per dispatched event.
//!
//! Measurement for the "replace the generated COBOL event loop with a native
//! one" question. The loop the code generator emits is:
//!
//! ```text
//! PERFORM UNTIL COBOL-QUIT = 1
//!     CALL "COBOL-WAIT-EVENT" USING COBOL-EVENT-ID COBOL-CONTROL-ID
//!     EVALUATE COBOL-CONTROL-ID
//!         WHEN "CTRL-1"  EVALUATE COBOL-EVENT-ID
//!                            WHEN "onClick" CALL "HANDLER-1"
//!         ... one WHEN per control that has any handler
//! END-PERFORM
//! ```
//!
//! Every part of that runs through the tree-walking interpreter on each event.
//! These benchmarks separate the three costs that decide whether a native loop
//! is worth building:
//!
//! 1. **Dispatch overhead** — the PERFORM/EVALUATE/CALL scaffolding. This is
//!    what a native loop would delete.
//! 2. **Chain position** — the EVALUATE is a LINEAR scan of string compares, so
//!    the last control costs more than the first. A native dispatcher is a hash
//!    lookup: flat.
//! 3. **Handler body** — stays COBOL either way, so it BOUNDS the whole gain
//!    (Amdahl). Reported so the ratio is visible rather than assumed.
//!
//! Run with: `cargo test -p cobolt-runtime --test bench_event_loop -- --nocapture`

use std::sync::mpsc;
use std::time::Instant;

use cobolt_lexer::{tokenize, SourceFormat};
use cobolt_parser::{parse, Severity};
use cobolt_runtime::{FormEvent, Interpreter};

/// Events per measured run. Large enough to swamp start-up, small enough to
/// keep the suite quick.
const EVENTS: usize = 20_000;

/// Build a program shaped exactly like the generated event loop, with
/// `controls` entries in the EVALUATE chain. `body` is the statement(s) run for
/// the matched handler — inline, so the measurement is the loop itself unless
/// the caller asks for more.
fn event_loop_program(controls: usize, body: &str) -> String {
    let mut s = String::from(
        "       IDENTIFICATION DIVISION.\n\
         \x20      PROGRAM-ID. BENCH.\n\
         \x20      DATA DIVISION.\n\
         \x20      WORKING-STORAGE SECTION.\n\
         \x20      01 COBOL-EVENT-ID    PIC X(32).\n\
         \x20      01 COBOL-CONTROL-ID  PIC X(32).\n\
         \x20      01 COBOL-QUIT        PIC 9 VALUE 0.\n\
         \x20      01 WS-HITS           PIC 9(9) VALUE 0.\n\
         \x20      01 WS-WORK           PIC 9(9) VALUE 0.\n\
         \x20      PROCEDURE DIVISION.\n\
         \x20      COBOL-MAIN.\n\
         \x20          PERFORM COBOL-EVENT-LOOP.\n\
         \x20          DISPLAY \"HITS=\" WS-HITS.\n\
         \x20          STOP RUN.\n\
         \x20      COBOL-EVENT-LOOP.\n\
         \x20          PERFORM UNTIL COBOL-QUIT = 1\n\
         \x20              CALL \"COBOL-WAIT-EVENT\"\n\
         \x20                  USING COBOL-EVENT-ID COBOL-CONTROL-ID\n\
         \x20              EVALUATE COBOL-CONTROL-ID\n",
    );
    for i in 1..=controls {
        s.push_str(&format!("                   WHEN \"CTRL-{i}\"\n"));
        s.push_str("                       EVALUATE COBOL-EVENT-ID\n");
        s.push_str("                           WHEN \"onClick\"\n");
        for line in body.lines() {
            s.push_str(&format!("                               {line}\n"));
        }
        s.push_str("                       END-EVALUATE\n");
    }
    s.push_str("               END-EVALUATE\n");
    s.push_str("           END-PERFORM.\n");
    s
}

/// Drive `EVENTS` clicks at `target`, then quit. Returns nanoseconds per event.
fn ns_per_event(src: &str, target: &str) -> f64 {
    let result = parse(tokenize(src, SourceFormat::Free));
    assert!(
        result.diagnostics.iter().all(|d| d.severity != Severity::Error),
        "parse errors: {:?}",
        result.diagnostics
    );
    let program = result.program.expect("no program");

    let (event_tx, event_rx) = mpsc::channel();
    let (state_tx, _state_rx) = mpsc::channel();
    let (display_tx, _display_rx) = mpsc::channel();

    // Pre-queue everything so the interpreter never waits on the producer:
    // what we time is processing, not scheduling.
    for _ in 0..EVENTS {
        event_tx
            .send(FormEvent::new(target, "onClick"))
            .expect("queue event");
    }
    event_tx
        .send(FormEvent::new("__QUIT__", "onClose"))
        .expect("queue quit");

    let mut interp = Interpreter::new_with_channels(program, event_rx, state_tx, display_tx);
    let start = Instant::now();
    interp.run().expect("run failed");
    let elapsed = start.elapsed();
    elapsed.as_nanos() as f64 / EVENTS as f64
}

#[test]
fn interpreted_event_dispatch_cost() {
    // A trivial body, so this is the loop scaffolding itself.
    let body = "CONTINUE";

    // Chain position, on a form with a realistic number of wired controls.
    let src_40 = event_loop_program(40, body);
    let first = ns_per_event(&src_40, "CTRL-1");
    let last = ns_per_event(&src_40, "CTRL-40");
    let miss = ns_per_event(&src_40, "CTRL-NONE"); // no WHEN matches

    // Chain length, measured at the far end where the scan is longest.
    let src_5 = event_loop_program(5, body);
    let short_last = ns_per_event(&src_5, "CTRL-5");

    // A handler body of the size real handlers have: a few moves and a test.
    let real_body = "MOVE 1 TO WS-WORK\n\
                     ADD 1 TO WS-HITS\n\
                     IF WS-WORK > 0\n\
                         ADD 1 TO WS-WORK\n\
                     END-IF";
    let src_body = event_loop_program(40, real_body);
    let with_body = ns_per_event(&src_body, "CTRL-40");

    println!("\n  ── Interpreted event loop, ns per dispatched event ──");
    println!("  40 controls, first WHEN matches      {first:>9.0} ns");
    println!("  40 controls, last WHEN matches       {last:>9.0} ns");
    println!("  40 controls, NO WHEN matches         {miss:>9.0} ns");
    println!("   5 controls, last WHEN matches       {short_last:>9.0} ns");
    println!("  40 controls + real handler body      {with_body:>9.0} ns");
    println!(
        "  scaffolding share of a real dispatch  {:>8.1} %",
        last / with_body * 100.0
    );
    println!(
        "  cost of 35 extra WHENs                {:>8.0} ns  ({:.1} ns per WHEN)",
        last - short_last,
        (last - short_last) / 35.0
    );
    println!(
        "  events/sec one core sustains (real)   {:>8.0}\n",
        1e9 / with_body
    );
}
