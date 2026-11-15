// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! **DESKTOP-WASM-001 T8 (R28): the form host's clock.**
//!
//! Everything the host times on its own reads this clock instead of `Instant::now`:
//! - the moment a form opened, which gates its 450 ms input hold and `onShow`;
//! - the entrance and exit effects and the window effect;
//! - snackbars;
//! - the animation step;
//! - the timers of a parked form.
//!
//! **Real by default:** [`now`] is `Instant::now()`.
//!
//! **Virtual in a deterministic headless run:** with the clock pinned (`PRC_TEST_CLOCK`), the
//! headless loop starts the virtual time before it builds the form
//! (`cobolt_runtime::test_pins::start_virtual_time`) and moves it 1/60 s per tick. [`now`] is then
//! a fixed instant plus that time. Two runs see the same instants however fast the machine is, and
//! the program's own clock follows the same time.
//!
//! Reads that never reach a pixel stay on the real clock: placing a group of windows on the
//! screen, the OS blur's retry, time limits, and the measurements of tests.

use std::sync::OnceLock;
use std::time::{Duration, Instant};

/// The instant the virtual time counts from, taken the first time it is read.
static ORIGIN: OnceLock<Instant> = OnceLock::new();

/// Now, on the host's clock.
pub fn now() -> Instant {
    match cobolt_runtime::test_pins::virtual_elapsed() {
        Some(elapsed) => *ORIGIN.get_or_init(Instant::now) + elapsed,
        None => Instant::now(),
    }
}

/// The time since `earlier` on the host's clock: `Instant::elapsed` for this clock.
pub fn since(earlier: Instant) -> Duration {
    now().saturating_duration_since(earlier)
}
