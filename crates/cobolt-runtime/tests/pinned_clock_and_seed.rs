// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! **DESKTOP-WASM-001 T5 (R28): the run clock and the random seed, pinned for tests.**
//!
//! With the clock pinned, `FUNCTION CURRENT-DATE` and `ACCEPT … FROM DATE`, `DAY`,
//! `DAY-OF-WEEK` and `TIME` give the pinned instant, the same in every run and on every machine.
//! With the seed pinned, `FUNCTION RANDOM` gives the same 1 000 values in every run, the values
//! `FUNCTION RANDOM(seed)` gives. With nothing pinned, the generator starts where it always has.
//!
//! The pins are set with `set_for_test`, never through the environment. They hold for the whole
//! process, so this file has one test. Restarting the pins starts the generator again, as a new
//! process does.

use std::sync::mpsc;
use std::time::Instant;

use cobolt_lexer::{tokenize, SourceFormat};
use cobolt_parser::{parse, Severity};
use cobolt_runtime::test_pins::{clear_for_test, set_for_test};
use cobolt_runtime::Interpreter;

/// A Friday, 2 January 2026, at 03:04:05.67 in São Paulo's offset.
const PIN: &str = "2026-01-02T03:04:05.67-03:00";
const SEED: u64 = 20_261_114;
const DRAWS: usize = 1000;

fn run(src: &str) -> Vec<String> {
    let result = parse(tokenize(src, SourceFormat::Free));
    assert!(result.diagnostics.iter().all(|d| d.severity != Severity::Error), "parse errors: {:?}", result.diagnostics);
    let program = result.program.expect("no program");
    let (_event_tx, event_rx) = mpsc::channel();
    let (state_tx, _state_rx) = mpsc::channel();
    let (display_tx, display_rx) = mpsc::channel();
    let mut interp = Interpreter::new_with_channels(program, event_rx, state_tx, display_tx);
    interp.run().expect("run failed");
    display_rx.try_iter().map(|s| s.trim().to_owned()).collect()
}

/// Every clock register, into the receivers COBOL-85 gives them.
const CLOCK: &str = r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. CLOCK.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 WS-NOW  PIC X(21).
       01 WS-DATE PIC 9(6).
       01 WS-DAY  PIC 9(5).
       01 WS-DOW  PIC 9.
       01 WS-TIME PIC 9(8).
       PROCEDURE DIVISION.
       MAIN.
           MOVE FUNCTION CURRENT-DATE TO WS-NOW
           ACCEPT WS-DATE FROM DATE
           ACCEPT WS-DAY FROM DAY
           ACCEPT WS-DOW FROM DAY-OF-WEEK
           ACCEPT WS-TIME FROM TIME
           DISPLAY WS-NOW
           DISPLAY WS-DATE
           DISPLAY WS-DAY
           DISPLAY WS-DOW
           DISPLAY WS-TIME
           STOP RUN.
"#;

/// `DRAWS` values of `FUNCTION RANDOM`, the first one seeded by the program when `seeded`.
fn random(seeded: bool) -> String {
    let first = if seeded {
        format!("COMPUTE WS-R = FUNCTION RANDOM({SEED})\n           DISPLAY WS-R\n           MOVE 2 TO WS-I")
    } else {
        "MOVE 1 TO WS-I".to_owned()
    };
    format!(
        r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. DRAWS.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 WS-R PIC 9V9(9).
       01 WS-I PIC 9(4).
       PROCEDURE DIVISION.
       MAIN.
           {first}
           PERFORM UNTIL WS-I > {DRAWS}
               COMPUTE WS-R = FUNCTION RANDOM
               DISPLAY WS-R
               ADD 1 TO WS-I
           END-PERFORM
           STOP RUN.
"#
    )
}

/// The first value of the generator from its unpinned first state, as `interpreter.rs` makes it.
fn unpinned_first_value() -> f64 {
    let next = 6_364_136_223_846_793_005u64.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1_442_695_040_888_963_407);
    (next >> 11) as f64 / (1u64 << 53) as f64
}

/// A `PIC 9V9(9)` value as displayed, back to a number.
fn value(shown: &str) -> f64 {
    shown.parse::<u64>().map_or(f64::NAN, |n| n as f64 / 1e9)
}

#[test]
fn pinned_clock_and_seed_fix_current_date_and_random() {
    let mut checks: Vec<(String, bool)> = Vec::new();
    let mut check = |name: &str, ok: bool| checks.push((name.to_owned(), ok));

    // The clock.
    set_for_test(Some(PIN), Some(SEED));
    let t = Instant::now();
    let clock_a = run(CLOCK);
    let clock_b = run(CLOCK);
    let clock_ms = t.elapsed().as_secs_f64() * 1000.0;
    let expected = ["2026010203040567-0300", "260102", "26002", "5", "03040567"];
    check("the clock: two runs give the same registers", clock_a == clock_b);
    check("the clock: every register is the pinned instant", clock_a == expected);

    // The generator, pinned: two runs, each from a fresh start.
    let t = Instant::now();
    set_for_test(Some(PIN), Some(SEED));
    let draws_a = run(&random(false));
    set_for_test(Some(PIN), Some(SEED));
    let draws_b = run(&random(false));
    let random_ms = t.elapsed().as_secs_f64() * 1000.0;
    let distinct = draws_a.iter().collect::<std::collections::BTreeSet<_>>().len();
    check("RANDOM: 1 000 values from the pinned seed", draws_a.len() == DRAWS);
    check("RANDOM: two runs give the same 1 000 values", draws_a == draws_b);
    check("RANDOM: the values vary (at least 990 distinct)", distinct >= 990);

    // Nothing pinned: the program's own RANDOM(seed) gives the same values, and an unseeded
    // generator starts where it always has.
    clear_for_test();
    let draws_c = run(&random(true));
    check("RANDOM: the pinned seed gives what FUNCTION RANDOM(seed) gives", draws_a == draws_c);
    clear_for_test();
    let unpinned = run(&random(false));
    let first = unpinned.first().map_or(f64::NAN, |s| value(s));
    check("RANDOM: unpinned, the first value is unchanged", (first - unpinned_first_value()).abs() < 1e-9);

    // The result block (GOLDEN RULE #7).
    let passed = checks.iter().filter(|(_, ok)| *ok).count();
    println!("== pinned_clock_and_seed (DESKTOP-WASM-001 T5, R28) ==");
    println!("pinned clock {PIN}, pinned seed {SEED}");
    println!("clock registers: CURRENT-DATE, DATE, DAY, DAY-OF-WEEK, TIME -> {clock_a:?}");
    println!("clock: 2 runs in {clock_ms:.1} ms");
    println!(
        "RANDOM: {} runs x {DRAWS} values in {random_ms:.1} ms ({:.0} values/s), {distinct} distinct; first {:?}, last {:?}",
        2,
        2.0 * DRAWS as f64 / (random_ms / 1000.0),
        draws_a.first(),
        draws_a.last()
    );
    println!("unpinned first value: shown {:?}, expected {:.9}", unpinned.first(), unpinned_first_value());
    for (name, ok) in &checks {
        println!("  [{}] {name}", if *ok { "pass" } else { "FAIL" });
    }
    println!("checks: {passed} of {} passed", checks.len());
    assert_eq!(passed, checks.len(), "a check failed: see the result block");
}
