// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors

//! How far the installer's progress bar moves, and what its text says (R13).
//!
//! Windows Installer wants to be told the size of a custom action's work before
//! the install runs (an immediate action adds it to the bar's total) and then
//! told how much of it is done as it goes (the deferred action reports ticks).
//! The sizes are fixed here, in one place, and a step that ends early or skips
//! work still reports what is left, so the bar never stops short or runs past
//! its end.
//!
//! The bar is a **share**, not a clock: the page's text says where the step is
//! and how long it has taken, and the bar says how many of the units of work
//! are behind it. Every unit weighs the same until task T16 measures them on a
//! reference machine, when [`weight`] takes the measured shares.

use crate::progress::{self, Line, Part};

/// The ticks the Rust step may move the bar by.
pub const RUST: u32 = 50;
/// The ticks the compile step may move the bar by.
pub const COMPILE: u32 = 950;
/// What the immediate action `PrcTicks` adds to the bar's total: both steps.
pub const TOTAL: u32 = RUST + COMPILE;

/// The platform variants the compile builds for each profile, which is what its units are.
const VARIANTS: u32 = 3;
/// The profiles it builds: release and debug.
const PROFILES: u32 = 2;
/// The weight of all the compile's units together.
const UNITS: u32 = VARIANTS * PROFILES;

/// The share of the compile one unit of work is worth. Equal until T16 measures them.
pub fn weight(part: Part, unit: Option<&str>) -> u32 {
    match (part, unit) {
        (Part::Release | Part::Debug, Some(_)) => 1,
        _ => 0,
    }
}

/// The ticks one step has earned so far.
#[derive(Debug)]
pub struct Ticks {
    allotment: u32,
    done: u32,
    reported: u32,
    current: Option<(Part, Option<String>)>,
}

impl Ticks {
    /// A step that may move the bar by `allotment` ticks in all.
    pub fn new(allotment: u32) -> Ticks {
        Ticks {
            allotment,
            done: 0,
            reported: 0,
            current: None,
        }
    }

    /// A line of the step. The ticks to report now: a unit is done when the step moves on to another, so the unit
    /// that is running is never counted early.
    pub fn on_line(&mut self, line: &Line) -> u32 {
        let Line::At { part, unit, .. } = line else {
            return 0;
        };
        let now = (*part, unit.clone());
        if self.current.as_ref() == Some(&now) {
            return 0;
        }
        if let Some((part, unit)) = self.current.replace(now) {
            self.done += weight(part, unit.as_deref());
        }
        let due = u64::from(self.allotment) * u64::from(self.done.min(UNITS)) / u64::from(UNITS);
        self.take(due as u32)
    }

    /// The step has ended, well or not: the rest of its allotment, so the bar is where the install expects it.
    pub fn finish(&mut self) -> u32 {
        self.take(self.allotment)
    }

    fn take(&mut self, due: u32) -> u32 {
        let due = due.min(self.allotment);
        let now = due.saturating_sub(self.reported);
        self.reported += now;
        now
    }
}

/// What a batch of the step's lines means for the installer.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Seen {
    /// What the progress page shows: the last line of ours in the batch. Nothing when the batch had none.
    pub text: Option<String>,
    /// The ticks to report.
    pub ticks: u32,
}

/// Reads a step's lines for the page, the bar and the part it is in.
#[derive(Debug)]
pub struct Tracker {
    part: Option<Part>,
    ticks: Ticks,
}

impl Tracker {
    /// A step about `part` (when known), with `allotment` ticks.
    pub fn new(part: Option<Part>, allotment: u32) -> Tracker {
        Tracker {
            part,
            ticks: Ticks::new(allotment),
        }
    }

    /// Read a batch of lines. What is not one of ours (the child's own output) moves nothing and shows nothing.
    pub fn see(&mut self, lines: &[String]) -> Seen {
        let mut seen = Seen::default();
        for line in lines.iter().filter_map(|l| progress::parse(l)) {
            if let Line::At { part, .. } = &line {
                self.part = Some(*part);
            }
            seen.ticks += self.ticks.on_line(&line);
            seen.text = Some(progress::display(&line));
        }
        seen
    }

    /// The part the step is in now.
    pub fn part(&self) -> Option<Part> {
        self.part
    }

    /// The rest of the allotment (see [`Ticks::finish`]).
    pub fn finish(&mut self) -> u32 {
        self.ticks.finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(part: Part, unit: Option<&str>, crates: u32) -> String {
        progress::format(&Line::At {
            part,
            unit: unit.map(str::to_owned),
            crates,
            elapsed_secs: 1,
        })
    }

    /// What the compile step prints, in its order: the check, then each variant for release, then for debug,
    /// each with a few lines on its way.
    fn compile_lines() -> Vec<String> {
        let mut lines = vec![at(Part::Check, None, 0)];
        for part in [Part::Release, Part::Debug] {
            for unit in ["forms", "forms-sql", "forms-graphics"] {
                for crates in [0, 40, 80] {
                    lines.push(at(part, Some(unit), crates));
                }
            }
        }
        lines
    }

    #[test]
    fn the_bar_is_the_sum_of_what_the_steps_may_move() {
        assert_eq!(RUST + COMPILE, TOTAL);
        assert_eq!(UNITS, 6);
        assert_eq!(TOTAL, 1000);
    }

    #[test]
    fn the_compile_moves_the_bar_by_exactly_its_allotment_one_unit_at_a_time() {
        let mut tracker = Tracker::new(Some(Part::Check), COMPILE);
        let mut reported = Vec::new();
        for line in compile_lines() {
            let seen = tracker.see(&[line]);
            if seen.ticks > 0 {
                reported.push(seen.ticks);
            }
        }
        // Five units were left behind by a later one; the last is still running.
        assert_eq!(reported.len(), 5, "{reported:?}");
        let before_the_end: u32 = reported.iter().sum();
        assert_eq!(before_the_end, COMPILE * 5 / UNITS, "{reported:?}");
        let rest = tracker.finish();
        assert_eq!(before_the_end + rest, COMPILE, "the bar must end exactly full");
        assert_eq!(tracker.finish(), 0, "a second finish adds nothing");
        println!(
            "ticks: compile of {} lines moved the bar {:?} then {rest} at the end = {COMPILE}",
            compile_lines().len(),
            reported
        );
    }

    #[test]
    fn a_step_that_ends_early_still_reports_the_rest() {
        let mut tracker = Tracker::new(Some(Part::Check), COMPILE);
        let first = tracker.see(&[
            at(Part::Release, Some("forms"), 10),
            at(Part::Release, Some("forms-sql"), 1),
        ]);
        assert_eq!(first.ticks, COMPILE / UNITS);
        assert_eq!(first.ticks + tracker.finish(), COMPILE);
    }

    #[test]
    fn the_rust_step_reports_everything_at_its_end() {
        let mut tracker = Tracker::new(Some(Part::Rust), RUST);
        let seen = tracker.see(&[
            progress::format(&Line::Note("Downloading rustup-init.exe".into())),
            at(Part::Rust, None, 0),
            at(Part::Check, None, 0),
        ]);
        assert_eq!(seen.ticks, 0, "no unit of the compile was finished");
        assert_eq!(tracker.finish(), RUST);
    }

    #[test]
    fn a_variant_a_later_version_adds_never_pushes_the_bar_past_its_end() {
        let mut tracker = Tracker::new(None, COMPILE);
        let mut total = 0;
        for n in 0..20 {
            total += tracker
                .see(&[at(Part::Release, Some(&format!("variant-{n}")), 1)])
                .ticks;
            assert!(total <= COMPILE, "{total} after {n} units");
        }
        assert_eq!(total + tracker.finish(), COMPILE);
    }

    #[test]
    fn the_same_unit_again_is_not_a_new_unit() {
        let mut ticks = Ticks::new(COMPILE);
        let line = Line::At {
            part: Part::Release,
            unit: Some("forms".into()),
            crates: 5,
            elapsed_secs: 2,
        };
        assert_eq!(ticks.on_line(&line), 0);
        assert_eq!(ticks.on_line(&line), 0);
        assert_eq!(ticks.on_line(&Line::Note("anything".into())), 0);
    }

    #[test]
    fn the_page_shows_the_last_line_of_ours_and_the_childs_own_output_is_not_shown() {
        let mut tracker = Tracker::new(None, COMPILE);
        let seen = tracker.see(&[
            "Compiling serde v1.0.200".to_owned(),
            at(Part::Release, Some("forms"), 12),
            "warning: unused import".to_owned(),
        ]);
        assert_eq!(
            seen.text.as_deref(),
            Some("Compiling the platform for Release (forms): 12 crates compiled, 1 s so far")
        );
        assert_eq!(tracker.part(), Some(Part::Release));

        let none = tracker.see(&["Compiling serde v1.0.200".to_owned()]);
        assert_eq!(none, Seen::default());
        assert_eq!(tracker.part(), Some(Part::Release), "the part is kept");
    }
}
