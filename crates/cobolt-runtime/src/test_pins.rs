// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! **The run clock and the random seed, pinned for tests** (DESKTOP-WASM-001 R28, T5).
//!
//! Two variables turn the pins on. Nothing sets them by default:
//! - `PRC_TEST_CLOCK=<RFC 3339>`, for example `2026-01-02T03:04:05.67-03:00`, pins the date and
//!   time a program sees. The offset in the value is the program's time zone, so a pinned run
//!   shows the same date on every machine. The pinned clock does not move by itself; in a
//!   deterministic headless run it follows the run's virtual time (T8, [`start_virtual_time`]).
//! - `PRC_TEST_SEED=<u64>` gives `FUNCTION RANDOM` its first state, as `FUNCTION RANDOM(seed)`
//!   would.
//!
//! Both are read once, at the first read of the clock or of the generator. A value that cannot be
//! read is named on stderr and pins nothing.
//!
//! [`set_for_test`] pins without touching the environment. It holds for the whole process until
//! [`clear_for_test`], so a test that uses it runs alone in its test binary.
//!
//! **Where the clock and the generator are read** (the audit of T5): `ACCEPT … FROM DATE`,
//! `TIME`, `DAY` and `DAY-OF-WEEK`, `FUNCTION CURRENT-DATE`, `WHEN-COMPILED` and
//! `YEAR-TO-YYYY`, and `FUNCTION RANDOM`, all in `interpreter.rs`. The runtime's other reads of
//! the time never become a value a program sees:
//! - timeouts and progress of asynchronous operations;
//! - the waits of file locks, `WS-PAUSE` and the AWS pool;
//! - the frame clock of animation sequences, which the form host drives (T8);
//! - AWS request signing and credential expiry, which need the real time;
//! - indexed-file log stamps, key-file nonces and temporary names;
//! - measurements inside tests.
//!
//! `cobolt-stdlib`'s intrinsic table has its own `CURRENT-DATE`, but no code calls the table.

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{OnceLock, RwLock};
use std::time::Duration;

use chrono::{DateTime, FixedOffset};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct Pins {
    clock: Option<DateTime<FixedOffset>>,
    seed: Option<u64>,
}

static FROM_ENV: OnceLock<Pins> = OnceLock::new();
static FOR_TEST: RwLock<Option<Pins>> = RwLock::new(None);

fn read_env() -> Pins {
    let clock = std::env::var("PRC_TEST_CLOCK").ok().and_then(|v| match DateTime::parse_from_rfc3339(v.trim()) {
        Ok(t) => Some(t),
        Err(e) => {
            eprintln!("PRC_TEST_CLOCK={v} is not an RFC 3339 date and time ({e}); the clock is not pinned");
            None
        }
    });
    let seed = std::env::var("PRC_TEST_SEED").ok().and_then(|v| match v.trim().parse::<u64>() {
        Ok(s) => Some(s),
        Err(e) => {
            eprintln!("PRC_TEST_SEED={v} is not a whole number from 0 to {} ({e}); RANDOM is not pinned", u64::MAX);
            None
        }
    });
    Pins { clock, seed }
}

fn pins() -> Pins {
    if let Some(p) = *FOR_TEST.read().unwrap_or_else(|e| e.into_inner()) {
        return p;
    }
    *FROM_ENV.get_or_init(read_env)
}

/// The date and time the program sees: the pinned instant, plus the virtual time while a
/// deterministic run moves it; or the local clock.
pub fn now() -> DateTime<FixedOffset> {
    match pins().clock {
        Some(pinned) => pinned + chrono::Duration::from_std(virtual_elapsed().unwrap_or_default()).unwrap_or_default(),
        None => chrono::Local::now().fixed_offset(),
    }
}

/// Whether a deterministic run's virtual time is running, and how far it has gone (ns).
static VIRTUAL_ON: AtomicBool = AtomicBool::new(false);
static VIRTUAL_NANOS: AtomicU64 = AtomicU64::new(0);

/// Starts a deterministic run's virtual time at zero (T8). The headless loop calls it when the
/// clock is pinned, before it builds the form. From then on the form host's clock is a fixed
/// instant plus this time, and the program's pinned clock moves by it too.
pub fn start_virtual_time() {
    VIRTUAL_NANOS.store(0, Ordering::SeqCst);
    VIRTUAL_ON.store(true, Ordering::SeqCst);
}

/// Moves the virtual time to `elapsed` since it started.
pub fn set_virtual_time(elapsed: Duration) {
    VIRTUAL_NANOS.store(u64::try_from(elapsed.as_nanos()).unwrap_or(u64::MAX), Ordering::SeqCst);
}

/// Ends the virtual time: the host's clock is real again, and the program's stays at the pinned
/// instant.
pub fn stop_virtual_time() {
    VIRTUAL_ON.store(false, Ordering::SeqCst);
    VIRTUAL_NANOS.store(0, Ordering::SeqCst);
}

/// How far the virtual time has gone, while it runs.
pub fn virtual_elapsed() -> Option<Duration> {
    VIRTUAL_ON.load(Ordering::SeqCst).then(|| Duration::from_nanos(VIRTUAL_NANOS.load(Ordering::SeqCst)))
}

/// Whether the clock is pinned: a headless run is then deterministic (T6).
pub fn clock_pinned() -> bool {
    pins().clock.is_some()
}

/// The pinned first state of `FUNCTION RANDOM`, if one is pinned.
pub fn seed() -> Option<u64> {
    pins().seed
}

/// Pins the clock (RFC 3339) and the seed for the whole process, over the environment's pins,
/// and restarts `FUNCTION RANDOM` from the pinned seed. For tests only.
///
/// # Panics
/// When `clock` is not an RFC 3339 date and time.
pub fn set_for_test(clock: Option<&str>, seed: Option<u64>) {
    let clock = clock.map(|c| DateTime::parse_from_rfc3339(c).unwrap_or_else(|e| panic!("set_for_test: {c} is not RFC 3339: {e}")));
    *FOR_TEST.write().unwrap_or_else(|e| e.into_inner()) = Some(Pins { clock, seed });
    crate::interpreter::restart_random();
}

/// Takes back [`set_for_test`]: the environment's pins, if any, hold again.
pub fn clear_for_test() {
    *FOR_TEST.write().unwrap_or_else(|e| e.into_inner()) = None;
    crate::interpreter::restart_random();
}
