// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! The date/time registers follow the **local** clock.
//!
//! COBOL-85 defines `ACCEPT … FROM DATE / TIME / DAY / DAY-OF-WEEK` and
//! `FUNCTION CURRENT-DATE` on the local clock, and CURRENT-DATE reports the real
//! offset from GMT in its last five characters. They were all derived straight
//! from `UNIX_EPOCH`, which is UTC — a different time of day for most of the
//! world, and a different *date* either side of midnight. A clock built the
//! obvious way read three hours wrong in São Paulo (operator, 2026-08-24), and
//! CURRENT-DATE claimed `-0000` while sitting on `-0300`.
//!
//! These tests bracket the run between two `Local::now()` readings instead of
//! comparing against one instant, so they cannot flake on a second or midnight
//! boundary.

use std::sync::mpsc;

use chrono::{Datelike, Local, Offset, TimeZone, Timelike};
use cobolt_lexer::{tokenize, SourceFormat};
use cobolt_parser::{parse, Severity};
use cobolt_runtime::Interpreter;

fn run_capture(src: &str) -> Vec<String> {
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
    display_rx.try_iter().map(|s| s.trim().to_owned()).collect()
}

const PROG: &str = r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. CLK.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 WS-CD  PIC X(21).
       01 WS-DOW PIC 9.
       PROCEDURE DIVISION.
       MAIN.
           MOVE FUNCTION CURRENT-DATE TO WS-CD
           ACCEPT WS-DOW FROM DAY-OF-WEEK
           DISPLAY WS-CD
           DISPLAY WS-DOW
           STOP RUN.
"#;

/// `FUNCTION CURRENT-DATE` lands between two readings of the local clock.
///
/// If the register went back to UTC this fails everywhere the machine is not on
/// GMT — by exactly the offset.
#[test]
fn current_date_is_the_local_wall_clock() {
    let before = Local::now();
    let out = run_capture(PROG);
    let after = Local::now();

    let cd = out.first().expect("no CURRENT-DATE line");
    assert_eq!(cd.len(), 21, "CURRENT-DATE must be 21 chars, got {cd:?}");

    let num = |r: std::ops::Range<usize>| cd[r].parse::<i64>().expect("digits");
    let stamp = Local
        .with_ymd_and_hms(
            num(0..4) as i32,
            num(4..6) as u32,
            num(6..8) as u32,
            num(8..10) as u32,
            num(10..12) as u32,
            num(12..14) as u32,
        )
        .single()
        .expect("CURRENT-DATE is not a real local time");

    // One second of slack on each side: the register carries centiseconds, which
    // the reconstruction above truncates.
    assert!(
        stamp >= before - chrono::Duration::seconds(1)
            && stamp <= after + chrono::Duration::seconds(1),
        "CURRENT-DATE {cd:?} → {stamp} is outside [{before}, {after}] — \
         the register is not on the local clock"
    );

    println!("CURRENT-DATE {cd:?} sits inside [{before}, {after}]");
}

/// The last five characters are the machine's **real** offset from GMT.
///
/// They were the literal `-0000` regardless of where the machine was, which is
/// the one part of the register whose whole job is to say otherwise.
#[test]
fn current_date_reports_the_real_offset_from_gmt() {
    let out = run_capture(PROG);
    let cd = out.first().expect("no CURRENT-DATE line");
    let reported = &cd[16..21];

    let secs = Local::now().offset().fix().local_minus_utc();
    let sign = if secs < 0 { '-' } else { '+' };
    let expected = format!("{sign}{:02}{:02}", secs.abs() / 3600, (secs.abs() % 3600) / 60);

    assert_eq!(
        reported, expected,
        "CURRENT-DATE reported offset {reported:?}, machine is at {expected:?}"
    );

    if secs != 0 {
        assert_ne!(
            reported, "-0000",
            "the hardcoded -0000 is back on a machine that is not on GMT"
        );
    } else {
        println!("note: this machine IS on GMT, so the UTC/local distinction is invisible here");
    }
    println!("CURRENT-DATE offset {reported:?} matches the machine's {expected:?}");
}

/// `ACCEPT … FROM DAY-OF-WEEK` is the local day, 1 = Monday … 7 = Sunday.
///
/// Derived from UTC it flips a day either side of midnight — for a UTC-behind
/// zone on a Sunday evening, "today" was already Monday.
#[test]
fn day_of_week_is_the_local_day() {
    let before = Local::now().weekday().number_from_monday();
    let out = run_capture(PROG);
    let after = Local::now().weekday().number_from_monday();

    let dow: u32 = out.get(1).expect("no DAY-OF-WEEK line").parse().expect("digit");
    assert!((1..=7).contains(&dow), "out of range: {dow}");
    assert!(
        dow == before || dow == after,
        "DAY-OF-WEEK {dow} is neither {before} nor {after} (midnight crossing aside)"
    );

    println!("DAY-OF-WEEK {dow} matches the local day ({before})");
}
