// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! The clock that ends a call nothing else ends (spec 114 T22, R10, AC6, D3).
//!
//! The Backend engine checks the epoch at every loop and every function entry of the module. One thread per runner
//! advances it every [`TICK`]; each call's store asks, at each tick, whether its call must stop (the server's hard flag
//! is set: the call ran over its time, the Backend is shutting down, or the grace after a cancel ran out) and ends the
//! call there when it must. A program in a loop with no statement the interpreter would stop at is stopped all the same,
//! within a few ticks, and the thread that ran it ends: the interpreter's could only be abandoned.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread::JoinHandle;
use std::time::Duration;

/// How often the epoch advances: the time a runaway call outlives its stop by, at most, plus the time to unwind.
pub const TICK: Duration = Duration::from_millis(10);

/// The thread that advances the epoch, for as long as it is held.
pub struct Ticker {
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

impl Ticker {
    /// Call `tick` every [`TICK`] on a thread of its own.
    pub fn start(tick: impl Fn() + Send + 'static) -> Ticker {
        Ticker::start_every(TICK, tick)
    }

    /// [`Ticker::start`] with another interval (a test's).
    pub fn start_every(every: Duration, tick: impl Fn() + Send + 'static) -> Ticker {
        let stop = Arc::new(AtomicBool::new(false));
        let flag = stop.clone();
        let thread = std::thread::Builder::new()
            .name("backend-epoch".into())
            .spawn(move || {
                while !flag.load(Ordering::SeqCst) {
                    std::thread::sleep(every);
                    tick();
                }
            })
            .expect("the epoch thread starts");
        Ticker { stop, thread: Some(thread) }
    }
}

impl Drop for Ticker {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}
