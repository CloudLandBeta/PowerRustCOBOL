// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! The clock and the random source the runner answers the module with, and the test pins that fix
//! them (DESKTOP-WASM-001 R28, T15), read as `cobolt-runtime`'s `test_pins` reads them for the
//! interpreter, so a pinned run gives the same values on both paths:
//!
//! * `PRC_TEST_CLOCK=<RFC 3339>` pins the date and time; the offset in the value is the program's
//!   time zone. The pinned clock does not move.
//! * `PRC_TEST_SEED=<u64>` gives the `random` import its first state. `FUNCTION RANDOM` keeps its
//!   generator inside the module; a desktop module that uses it reads the same variable through
//!   `prn.env_get`, as the interpreter reads it.
//!
//! A value that cannot be read is named on stderr, in the interpreter's words, and pins nothing.
//!
//! **The clock is the local one.** `now` answers milliseconds since 1970-01-01 of the *local* date
//! and time (the instant plus the zone's offset): the module's routines read it as a calendar, so
//! `ACCEPT … FROM DATE` and the date and time of `FUNCTION CURRENT-DATE` are the desktop's, as the
//! interpreter shows them. The last five characters of `CURRENT-DATE` still read `+0000` (WASM-PARITY
//! #6, open for the desktop).

use chrono::{DateTime, FixedOffset};

/// The pins a run starts with.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Pins {
    pub clock: Option<DateTime<FixedOffset>>,
    pub seed: Option<u64>,
}

impl Pins {
    /// The pins from `PRC_TEST_CLOCK` and `PRC_TEST_SEED`, and what to say about values that
    /// cannot be read.
    pub fn from_env() -> (Pins, Vec<String>) {
        Pins::read(std::env::var("PRC_TEST_CLOCK").ok().as_deref(), std::env::var("PRC_TEST_SEED").ok().as_deref())
    }

    /// The pins these two values give.
    pub fn read(clock: Option<&str>, seed: Option<&str>) -> (Pins, Vec<String>) {
        let mut said = Vec::new();
        let clock = clock.and_then(|v| match DateTime::parse_from_rfc3339(v.trim()) {
            Ok(t) => Some(t),
            Err(e) => {
                said.push(format!("PRC_TEST_CLOCK={v} is not an RFC 3339 date and time ({e}); the clock is not pinned"));
                None
            }
        });
        let seed = seed.and_then(|v| match v.trim().parse::<u64>() {
            Ok(s) => Some(s),
            Err(e) => {
                said.push(format!("PRC_TEST_SEED={v} is not a whole number from 0 to {} ({e}); RANDOM is not pinned", u64::MAX));
                None
            }
        });
        (Pins { clock, seed }, said)
    }

    /// What `now` answers: the local date and time, as milliseconds since 1970-01-01 (see the
    /// module documentation).
    pub fn now_ms(&self) -> f64 {
        match self.clock {
            Some(t) => wall_ms(t.naive_local()),
            None => wall_ms(chrono::Local::now().naive_local()),
        }
    }

    /// The `random` import's first state.
    pub fn random_state(&self) -> u64 {
        match self.seed {
            Some(s) => mix_seed(s),
            None => {
                let nanos = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos() as u64).unwrap_or(0);
                mix_seed(nanos ^ u64::from(std::process::id()))
            }
        }
    }
}

/// Milliseconds since 1970-01-01T00:00 of a date and time on the wall clock, with the fraction.
fn wall_ms(t: chrono::NaiveDateTime) -> f64 {
    let u = t.and_utc();
    u.timestamp() as f64 * 1000.0 + f64::from(u.timestamp_subsec_nanos()) / 1_000_000.0
}

const LCG_MUL: u64 = 6364136223846793005;
const LCG_ADD: u64 = 1442695040888963407;

/// The generator's state for a seed: the interpreter's and the module's `mix_seed`.
pub fn mix_seed(seed: u64) -> u64 {
    seed.wrapping_mul(LCG_MUL).wrapping_add(LCG_ADD)
}

/// One step of the generator: the next state, and a value in `[0, 1)`.
pub fn lcg_step(state: u64) -> (u64, f64) {
    let next = state.wrapping_mul(LCG_MUL).wrapping_add(LCG_ADD);
    (next, (next >> 11) as f64 / (1u64 << 53) as f64)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_pinned_clock_answers_its_local_wall_time() {
        let (p, said) = Pins::read(Some("2026-01-02T03:04:05.67-03:00"), Some("20261114"));
        assert!(said.is_empty());
        // 2026-01-02T03:04:05.670 on the wall: 20 455 days after 1970-01-01 (1 767 312 000 s) and
        // 11 045 s into the day, then 670 ms; the offset is not added, it is the wall clock's own.
        assert_eq!(p.now_ms(), 1_767_323_045_670.0);
        assert_eq!(p.random_state(), mix_seed(20_261_114));
    }

    #[test]
    fn unreadable_pins_are_named_and_pin_nothing() {
        let (p, said) = Pins::read(Some("yesterday"), Some("-1"));
        assert_eq!(p, Pins::default());
        assert_eq!(said.len(), 2);
        assert!(said[0].starts_with("PRC_TEST_CLOCK=yesterday is not an RFC 3339 date and time"));
        assert!(said[1].starts_with("PRC_TEST_SEED=-1 is not a whole number"));
    }
}
