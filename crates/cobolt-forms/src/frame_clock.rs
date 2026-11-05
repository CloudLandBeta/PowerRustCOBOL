// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! The pacing of a canvas's `onFrame` (WASM-018 T19, R14), with no window system in it: the desktop's
//! canvas (`canvas_input`) and the browser player both ask this clock which frames are due, so a program
//! sees the same elapsed times on every host. A minimised window or a hidden tab draws no frames, so it
//! calls [`FrameClock::pause`] and raises none.

/// The most fixed steps one display frame may carry.
pub const MAX_STEPS: usize = 5;

/// A frame deadline reached a whisker early still counts (repaint scheduling has millisecond granularity).
const TOLERANCE_MS: f64 = 1.0;

/// The pacing of one canvas's `onFrame`.
#[derive(Clone, Copy, Debug, Default)]
pub struct FrameClock {
    last_ms: Option<f64>,
    owed_ms: f64,
    due_ms: f64,
}

impl FrameClock {
    /// The elapsed times (ms) of the `onFrame` events due at `now_ms`. Variable: at most one, the time
    /// since the last. Fixed: as many whole steps of `1000 / fps` as have accrued.
    pub fn advance(&mut self, now_ms: f64, fps: f64, fixed: bool) -> Vec<f64> {
        let Some(last) = self.last_ms.replace(now_ms) else { return Vec::new() };
        if fps <= 0.0 {
            return Vec::new();
        }
        let step = 1000.0 / fps;
        let gap = (now_ms - last).max(0.0);
        if !fixed {
            // the schedule advances by exactly one step, so the mean rate is the target however the display
            // quantises it; a frame that lands late by more than a step resyncs instead of repaying
            if self.due_ms == 0.0 {
                self.due_ms = last + step;
            }
            if now_ms + TOLERANCE_MS < self.due_ms {
                self.last_ms = Some(last);
                return Vec::new();
            }
            self.due_ms = if now_ms - self.due_ms > step { now_ms + step } else { self.due_ms + step };
            return vec![gap];
        }
        self.owed_ms += gap;
        let mut out = Vec::new();
        while self.owed_ms + TOLERANCE_MS >= step && out.len() < MAX_STEPS {
            self.owed_ms = (self.owed_ms - step).max(0.0);
            out.push(step);
        }
        if out.len() == MAX_STEPS {
            self.owed_ms = 0.0;
        }
        out
    }

    /// The window was hidden: forget the past so showing it again does not repay it.
    pub fn pause(&mut self) {
        self.last_ms = None;
        self.owed_ms = 0.0;
        self.due_ms = 0.0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Variable pacing: the rate holds within a tolerance, the elapsed time is the real gap.
    #[test]
    fn on_frame_runs_at_the_target_rate_within_a_tolerance() {
        let mut c = FrameClock::default();
        let (fps, display_hz) = (30.0, 144.0);
        let mut t = 0.0;
        let mut frames = Vec::new();
        let started = std::time::Instant::now();
        while t < 10_000.0 {
            frames.extend(c.advance(t, fps, false));
            t += 1000.0 / display_hz;
        }
        let rate = frames.len() as f64 / 10.0;
        let mean = frames.iter().sum::<f64>() / frames.len() as f64;
        println!("T19: onFrame at TargetFps 30 on a 144 Hz display, 10 s: {} frames = {rate:.2}/s, mean elapsed {mean:.2} ms ({} us to compute)", frames.len(), started.elapsed().as_micros());
        assert!((rate - fps).abs() <= 3.0, "{rate}");
        assert!(frames.iter().all(|d| *d >= 1000.0 / fps - 1000.0 / display_hz - 0.01), "no delta is shorter than the display could make it");
        assert!((mean - 1000.0 / fps).abs() < 1.5, "the mean elapsed time is the period: {mean}");
    }

    /// Fixed step: every delta is exactly 1000/fps, and the count follows the elapsed time.
    #[test]
    fn fixed_step_deltas_are_exact_whatever_the_display_does() {
        let mut c = FrameClock::default();
        let fps = 60.0;
        let step = 1000.0 / fps;
        let mut t = 0.0;
        let mut steps = Vec::new();
        for gap in [7.0, 31.0, 5.0, 60.0, 16.0, 16.0, 2.0, 40.0].iter().cycle().take(400) {
            steps.extend(c.advance(t, fps, true));
            t += gap;
        }
        let expected = (t / step).floor();
        println!("T19: fixed step 60 fps over {t:.0} ms: {} steps (expected about {expected}), every delta {step:.4} ms", steps.len());
        assert!(steps.iter().all(|d| (*d - step).abs() < 1e-9));
        assert!((steps.len() as f64 - expected).abs() <= MAX_STEPS as f64 * 2.0 + 5.0, "{} vs {expected}", steps.len());
    }

    /// A stall never becomes a burst; hidden time is not repaid.
    #[test]
    fn a_stall_is_not_repaid_and_hidden_means_no_frames() {
        let mut c = FrameClock::default();
        c.advance(0.0, 60.0, true);
        let burst = c.advance(10_000.0, 60.0, true);
        assert_eq!(burst.len(), MAX_STEPS);
        c.pause();
        assert!(c.advance(20_000.0, 60.0, false).is_empty(), "the first frame after a pause only sets the mark");
        assert!(c.advance(20_005.0, 0.0, false).is_empty(), "TargetFps 0: no frames");
    }
}
