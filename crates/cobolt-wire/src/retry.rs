// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! The retry policy of a broken connection (WASM-003 R25, plan D11).
//!
//! When the connection to the backend breaks, a frontend tries again a fixed
//! number of times, each attempt given a window `first + (n-1)·step` long:
//! 10, 20 and 30 s. An attempt that fails before its window ends waits the
//! window out, so the cycle takes the same time whatever the failure; after
//! the last one the frontend gives up and every waiting call fails with the
//! class *transport*. Both clients — the browser player and the Rust client
//! the desktop runtime and the compiled binary use — read this one value, so
//! they cannot drift apart. It is not a project setting: R25 fixes the
//! numbers, and only tests construct a shorter policy.

/// How many attempts a broken connection gets and how long each may take.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Policy {
    /// The window of the first attempt, in milliseconds.
    pub first_ms: u64,
    /// What each later attempt's window adds to the one before, in milliseconds.
    pub step_ms: u64,
    /// How many attempts a cycle makes before it gives up.
    pub attempts: u32,
}

impl Default for Policy {
    /// The spec's numbers (R25): 10 s, then 20 s, then 30 s, and no fourth.
    fn default() -> Self {
        Policy { first_ms: 10_000, step_ms: 10_000, attempts: 3 }
    }
}

impl Policy {
    /// The window of attempt `n` (counted from 1), or `None` when the cycle
    /// has no such attempt — `n` is 0 or past [`Policy::attempts`].
    pub fn window(&self, n: u32) -> Option<u64> {
        if n == 0 || n > self.attempts {
            return None;
        }
        Some(self.first_ms.saturating_add(self.step_ms.saturating_mul(u64::from(n - 1))))
    }

    /// The length of a whole cycle that gives up: every window, summed.
    pub fn cycle_ms(&self) -> u64 {
        (1..=self.attempts).filter_map(|n| self.window(n)).fold(0u64, u64::saturating_add)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_windows_are_10_20_30_and_there_is_no_fourth() {
        let p = Policy::default();
        let windows: Vec<Option<u64>> = (0..=4).map(|n| p.window(n)).collect();
        println!("windows 0..=4: {windows:?}; cycle {} ms", p.cycle_ms());
        assert_eq!(windows, vec![None, Some(10_000), Some(20_000), Some(30_000), None]);
        assert_eq!(p.cycle_ms(), 60_000);
    }

    #[test]
    fn a_shorter_policy_keeps_the_same_shape() {
        let p = Policy { first_ms: 100, step_ms: 50, attempts: 2 };
        assert_eq!((p.window(1), p.window(2), p.window(3)), (Some(100), Some(150), None));
        assert_eq!(p.cycle_ms(), 250);
        let none = Policy { first_ms: 100, step_ms: 50, attempts: 0 };
        assert_eq!((none.window(1), none.cycle_ms()), (None, 0));
    }
}
