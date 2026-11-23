// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors

//! What the supervisor decides about a step, apart from Windows (R12, R14, R15).
//!
//! Two decisions are made here so they are tested on every operating system:
//! when a step that has gone quiet is given up (the watchdog), and what the
//! finish page is told when a step ended in a way its own result file does not
//! say.

use crate::progress::Part;
use crate::result::{Failure, Outcome};
use std::time::{Duration, Instant};

/// Gives up on a step that has said nothing for too long. Every line the step prints is "something heard", the
/// heartbeat included, so a long compile that keeps printing is never given up and a step that hangs is.
#[derive(Debug)]
pub struct Watchdog {
    limit: Option<Duration>,
    last: Instant,
}

impl Watchdog {
    /// A watchdog of `secs` seconds starting `now`. Zero is no limit.
    pub fn new(secs: u64, now: Instant) -> Watchdog {
        Watchdog {
            limit: (secs > 0).then(|| Duration::from_secs(secs)),
            last: now,
        }
    }

    /// The step printed something.
    pub fn heard(&mut self, now: Instant) {
        self.last = now;
    }

    /// Whether the step has been silent for the whole limit.
    pub fn expired(&self, now: Instant) -> bool {
        self.limit
            .is_some_and(|limit| now.saturating_duration_since(self.last) >= limit)
    }
}

/// How a step ended, as the supervisor saw it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum End {
    /// The process ended by itself with this code (`None`: Windows would not say).
    Exited(Option<u32>),
    /// The developer cancelled and the step was stopped. The install is being rolled back; nothing is reported.
    Cancelled,
    /// The step was silent for its whole watchdog and was stopped.
    GaveUp {
        /// The seconds it was given.
        secs: u64,
    },
    /// The process could not be started.
    CouldNotStart(String),
}

/// What the supervisor must add to the result file for this end, if anything.
///
/// A step that did its work, or reported in the result file that it could not, ends with
/// [`REPORTED`](crate::result::exit::REPORTED) and has already written its own lines: the supervisor adds
/// nothing. Every other end is a step that did not get to say how it went, so the supervisor says it for the step
/// (R12): a failure the finish page names, never a failed install.
pub fn outcome_for(end: &End, part: Part) -> Option<Outcome> {
    use crate::result::exit::REPORTED;
    let failure = match end {
        End::Exited(Some(code)) if *code == REPORTED as u32 => return None,
        End::Cancelled => return None,
        End::Exited(code) => Failure::Ended { part, code: *code },
        End::GaveUp { secs } => Failure::TimedOut {
            part,
            minutes: u32::try_from(secs.div_ceil(60)).unwrap_or(u32::MAX),
        },
        End::CouldNotStart(why) => Failure::Refused {
            part,
            why: format!("PowerRustCOBOL could not start this step ({why})."),
        },
    };
    Some(Outcome::failed(&failure))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::result::Status;

    #[test]
    fn the_watchdog_gives_up_only_after_its_whole_limit_of_silence() {
        let t0 = Instant::now();
        let mut dog = Watchdog::new(900, t0);
        let at = |s: u64| t0 + Duration::from_secs(s);
        assert!(!dog.expired(at(0)));
        assert!(!dog.expired(at(899)));
        assert!(dog.expired(at(900)));
        assert!(dog.expired(at(5_000)));

        // A line resets the clock for another whole limit.
        dog.heard(at(899));
        assert!(!dog.expired(at(899 + 899)));
        assert!(dog.expired(at(899 + 900)));
    }

    #[test]
    fn a_watchdog_of_zero_never_gives_up_and_a_clock_that_runs_backwards_is_silence_of_nothing() {
        let t0 = Instant::now();
        let never = Watchdog::new(0, t0);
        assert!(!never.expired(t0 + Duration::from_secs(1_000_000)));

        let mut dog = Watchdog::new(10, t0 + Duration::from_secs(100));
        assert!(!dog.expired(t0), "now before the last line must not wrap around");
        dog.heard(t0 + Duration::from_secs(100));
        assert!(!dog.expired(t0 + Duration::from_secs(105)));
    }

    #[test]
    fn a_step_that_reported_or_was_cancelled_gets_nothing_added() {
        assert_eq!(outcome_for(&End::Exited(Some(0)), Part::Release), None);
        assert_eq!(outcome_for(&End::Cancelled, Part::Release), None);
    }

    #[test]
    fn every_other_end_is_a_failure_the_finish_page_names() {
        let cases = [
            (End::Exited(Some(2)), Part::Rust, "ended without saying how (code 2)"),
            (
                End::Exited(Some(3_221_225_477)),
                Part::Debug,
                "(code 3221225477)",
            ),
            (End::Exited(None), Part::Release, "ended without saying how."),
            (
                End::GaveUp { secs: 900 },
                Part::Release,
                "stopped answering for 15 minutes",
            ),
            (
                End::GaveUp { secs: 61 },
                Part::Release,
                "stopped answering for 2 minutes",
            ),
            (
                End::CouldNotStart("Windows error 5".into()),
                Part::Rust,
                "could not start this step (Windows error 5)",
            ),
        ];
        let count = cases.len();
        for (end, part, wants) in cases {
            let outcome = outcome_for(&end, part).unwrap_or_else(|| panic!("{end:?}"));
            assert_eq!(outcome.status, Status::Failed);
            assert_eq!(outcome.part, part);
            assert!(
                outcome.text.contains(wants),
                "{end:?} said '{}', wanted '{wants}'",
                outcome.text
            );
            assert!(
                outcome.text.contains("first Debug or EXEC RUST Build"),
                "{end:?} does not say what happens next"
            );
        }
        println!("supervisor ends: {count} ends, each a named failure that says nothing is lost");
    }
}
