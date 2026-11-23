// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors

//! What each step reports, and what the finish page says about it (R7, R12).
//!
//! A deferred custom action cannot set a Windows Installer property, so the
//! steps cannot hand the finish page their outcome directly. Each step appends
//! lines to a small **result file**; an immediate action reads it after the
//! work and turns it into the page's text. The producer (the headless
//! `--prepare-platform`, and the supervisor when a step ends without writing
//! anything) and the reader are both written against this module:
//!
//! ```text
//! step=rust status=ok text=Rust 1.95.0 is installed.
//! step=build-tools status=failed text=The Visual Studio Build Tools did not install ...
//! step=release status=skipped text=Not compiled: the Build Tools are missing.
//! ```
//!
//! A failure never fails the install (R12): the setup finishes a complete
//! install and says plainly what was not done and why.

use crate::progress::Part;

/// The longest sentence a line carries.
const MAX_TEXT: usize = 300;

/// How a step ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    /// Done and checked.
    Ok,
    /// It did not complete.
    Failed,
    /// It was not attempted, because a step before it failed or there was nothing to do.
    Skipped,
}

impl Status {
    /// The word in the file.
    pub fn name(self) -> &'static str {
        match self {
            Status::Ok => "ok",
            Status::Failed => "failed",
            Status::Skipped => "skipped",
        }
    }

    fn from_name(name: &str) -> Option<Status> {
        [Status::Ok, Status::Failed, Status::Skipped]
            .into_iter()
            .find(|s| s.name() == name)
    }

    /// The exit code a step process ends with: `0` for done or skipped, `1` for failed. The supervisor
    /// reads the file first and the code only when the file says nothing.
    pub fn exit_code(self) -> i32 {
        match self {
            Status::Ok | Status::Skipped => 0,
            Status::Failed => 1,
        }
    }
}

/// One line of the result file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Outcome {
    /// The part it is about.
    pub part: Part,
    /// How it ended.
    pub status: Status,
    /// A plain sentence, one line, ASCII, safe to show on an installer page.
    pub text: String,
}

/// Make a sentence safe for the file and for an installer page: one line, ASCII, no square brackets (Windows
/// Installer reads `[Name]` in a property as a reference), no longer than [`MAX_TEXT`].
pub fn clean(text: &str) -> String {
    let mut out = String::with_capacity(text.len().min(MAX_TEXT));
    for c in text.chars() {
        let c = match c {
            '[' => '(',
            ']' => ')',
            c if c.is_control() => ' ',
            c if !c.is_ascii() => '?',
            c => c,
        };
        if c == ' ' && out.ends_with(' ') {
            continue;
        }
        out.push(c);
    }
    let out = out.trim();
    if out.len() <= MAX_TEXT {
        return out.to_owned();
    }
    let mut cut = out[..MAX_TEXT - 3].trim_end().to_owned();
    cut.push_str("...");
    cut
}

impl Outcome {
    /// A done step.
    pub fn ok(part: Part, text: &str) -> Outcome {
        Outcome {
            part,
            status: Status::Ok,
            text: clean(text),
        }
    }

    /// A step that was not attempted.
    pub fn skipped(part: Part, text: &str) -> Outcome {
        Outcome {
            part,
            status: Status::Skipped,
            text: clean(text),
        }
    }

    /// A step that failed, with the sentence its failure gives (R12).
    pub fn failed(failure: &Failure) -> Outcome {
        Outcome {
            part: failure.part(),
            status: Status::Failed,
            text: clean(&failure.sentence()),
        }
    }
}

/// Why a step did not complete. Each case has one sentence that names what was not done, why, and what the
/// developer can do (R7, R12).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Failure {
    /// A download did not complete: no network, a proxy, a refusal.
    Download {
        /// The part that needed it.
        part: Part,
        /// What was being downloaded (`rustup-init`).
        what: String,
        /// Where it comes from.
        url: String,
    },
    /// A downloaded installer did not match the checksum that was expected, so it was not run.
    Checksum {
        /// The part that needed it.
        part: Part,
        /// What was downloaded.
        what: String,
    },
    /// A volume filled up.
    DiskFull {
        /// The part that was writing.
        part: Part,
        /// The volume (`C:`).
        volume: String,
    },
    /// An installer ran and ended with an error.
    InstallFailed {
        /// The part.
        part: Part,
        /// What it installs.
        what: String,
        /// Its exit code, when it gave one.
        code: Option<i32>,
    },
    /// The installs ended well but the check after them (the same detection, with the link probe) still finds
    /// the requirement missing or not working (R7).
    StillMissing {
        /// What is missing: `Rust 1.95.0`, `the C++ linker`.
        what: String,
        /// The next step for the developer.
        next: String,
    },
    /// A compile failed.
    CompileFailed {
        /// Release or Debug.
        part: Part,
        /// The variant being compiled (`forms-sql`).
        unit: String,
        /// The last line of cargo's complaint, if any.
        detail: String,
    },
    /// The step ran longer than its watchdog allows: nothing was heard from it.
    TimedOut {
        /// The part.
        part: Part,
        /// Minutes it was given.
        minutes: u32,
    },
    /// The step process ended without saying how.
    Ended {
        /// The part.
        part: Part,
        /// Its exit code.
        code: Option<u32>,
    },
    /// The step refused to start, because it would have done its work in the wrong place.
    Refused {
        /// The part.
        part: Part,
        /// Why, in a sentence.
        why: String,
    },
}

/// What the developer does next, the same for every failure: nothing is lost, the first Build does the work.
const THEN: &str =
    "Nothing is lost: the first Debug or EXEC RUST Build asks first and then does this work.";

impl Failure {
    /// The part the failure belongs to.
    pub fn part(&self) -> Part {
        match self {
            Failure::Download { part, .. }
            | Failure::Checksum { part, .. }
            | Failure::DiskFull { part, .. }
            | Failure::InstallFailed { part, .. }
            | Failure::CompileFailed { part, .. }
            | Failure::TimedOut { part, .. }
            | Failure::Ended { part, .. }
            | Failure::Refused { part, .. } => *part,
            Failure::StillMissing { .. } => Part::Check,
        }
    }

    /// The sentence, before [`clean`].
    pub fn sentence(&self) -> String {
        match self {
            Failure::Download { what, url, .. } => format!(
                "{what} could not be downloaded from {url}. Check the network connection and any proxy, then run the setup again. {THEN}"
            ),
            Failure::Checksum { what, .. } => format!(
                "{what} was downloaded but did not match the checksum this setup expects, so it was not run. Run the setup again; if it happens again, install it yourself from its own page. {THEN}"
            ),
            Failure::DiskFull { volume, .. } => format!(
                "The disk {volume} filled up. Free some space and run the setup again. {THEN}"
            ),
            Failure::InstallFailed { what, code, .. } => match code {
                Some(code) => format!(
                    "{what} did not install (its installer ended with code {code}). Install it yourself from its own page, then run the setup again. {THEN}"
                ),
                None => format!(
                    "{what} did not install. Install it yourself from its own page, then run the setup again. {THEN}"
                ),
            },
            Failure::StillMissing { what, next } => format!(
                "After the installation, {what} is still missing or does not work. {next} {THEN}"
            ),
            Failure::CompileFailed { part, unit, detail } => {
                let profile = if *part == Part::Debug { "Debug" } else { "Release" };
                let detail = detail.trim();
                if detail.is_empty() {
                    format!("The {unit} part of PowerRustCOBOL AI did not compile for {profile}. {THEN}")
                } else {
                    format!(
                        "The {unit} part of PowerRustCOBOL AI did not compile for {profile}: {detail} {THEN}"
                    )
                }
            }
            Failure::TimedOut { part, minutes } => format!(
                "{} stopped answering for {minutes} minutes, so it was ended. {THEN}",
                part.words()
            ),
            Failure::Ended { part, code } => match code {
                Some(code) => format!(
                    "{} ended without saying how (code {code}). {THEN}",
                    part.words()
                ),
                None => format!("{} ended without saying how. {THEN}", part.words()),
            },
            Failure::Refused { why, .. } => format!("{why} {THEN}"),
        }
    }
}

/// The text of a line, as it is written.
pub fn format(outcome: &Outcome) -> String {
    format!(
        "step={} status={} text={}",
        outcome.part.name(),
        outcome.status.name(),
        clean(&outcome.text)
    )
}

/// A line read back, or `None` when it is not one of ours.
pub fn parse(line: &str) -> Option<Outcome> {
    let rest = line.trim().strip_prefix("step=")?;
    let (part, rest) = rest.split_once(" status=")?;
    let (status, text) = rest.split_once(" text=")?;
    Some(Outcome {
        part: Part::from_name(part)?,
        status: Status::from_name(status)?,
        text: text.trim().to_owned(),
    })
}

/// The outcomes in a result file. A part that appears twice (a step run again) keeps its last word, in the
/// place of its first. Lines that are not ours are ignored.
pub fn read(file: &str) -> Vec<Outcome> {
    let mut out: Vec<Outcome> = Vec::new();
    for outcome in file.lines().filter_map(parse) {
        match out.iter_mut().find(|o| o.part == outcome.part) {
            Some(slot) => *slot = outcome,
            None => out.push(outcome),
        }
    }
    out
}

/// What the finish page says (R12). `asked` is whether the developer chose *Prepare now*: when nothing was asked,
/// nothing is said.
pub fn finish_text(asked: bool, outcomes: &[Outcome]) -> String {
    if !asked {
        return String::new();
    }
    let failed: Vec<&Outcome> = outcomes
        .iter()
        .filter(|o| o.status == Status::Failed)
        .collect();
    if failed.is_empty() {
        let done = outcomes.iter().any(|o| o.status == Status::Ok);
        return if done {
            "PowerRustCOBOL AI is prepared: Debug and EXEC RUST Builds start at once and run at full speed.".to_owned()
        } else {
            "There was nothing to prepare.".to_owned()
        };
    }
    let mut t = String::from(
        "PowerRustCOBOL AI is installed, but the preparation did not finish. What was not done:\r\n",
    );
    for o in failed {
        t.push_str(&format!("  - {}\r\n", o.text));
    }
    for o in outcomes.iter().filter(|o| o.status == Status::Skipped) {
        t.push_str(&format!("  - {}\r\n", o.text));
    }
    t
}

#[cfg(test)]
mod tests {
    use super::*;

    fn every_failure() -> Vec<Failure> {
        vec![
            Failure::Download {
                part: Part::Rust,
                what: "rustup-init".into(),
                url: "https://static.rust-lang.org/rustup".into(),
            },
            Failure::Checksum {
                part: Part::Rust,
                what: "rustup-init".into(),
            },
            Failure::DiskFull {
                part: Part::Release,
                volume: "C:".into(),
            },
            Failure::InstallFailed {
                part: Part::BuildTools,
                what: "The Visual Studio Build Tools".into(),
                code: Some(5007),
            },
            Failure::InstallFailed {
                part: Part::Rust,
                what: "Rust 1.95.0".into(),
                code: None,
            },
            Failure::StillMissing {
                what: "the C++ linker".into(),
                next:
                    "Install the Desktop development with C++ workload, then run the setup again."
                        .into(),
            },
            Failure::CompileFailed {
                part: Part::Release,
                unit: "forms-sql".into(),
                detail: "error: could not compile `rusqlite` [x]".into(),
            },
            Failure::CompileFailed {
                part: Part::Debug,
                unit: "forms".into(),
                detail: String::new(),
            },
            Failure::TimedOut {
                part: Part::Release,
                minutes: 30,
            },
            Failure::Ended {
                part: Part::Debug,
                code: Some(3221225477),
            },
            Failure::Ended {
                part: Part::Debug,
                code: None,
            },
            Failure::Refused {
                part: Part::Release,
                why: "The build folder would have been the system account's.".into(),
            },
        ]
    }

    #[test]
    fn every_failure_says_what_was_not_done_and_that_nothing_is_lost() {
        let failures = every_failure();
        for f in &failures {
            let outcome = Outcome::failed(f);
            assert_eq!(outcome.status, Status::Failed);
            assert_eq!(outcome.part, f.part());
            assert!(
                outcome.text.contains("first Debug or EXEC RUST Build"),
                "{f:?} does not say what happens next: {}",
                outcome.text
            );
            assert!(
                outcome.text.is_ascii()
                    && !outcome.text.contains(['[', ']', '\r', '\n'])
                    && outcome.text.len() <= MAX_TEXT,
                "{f:?}: {}",
                outcome.text
            );
        }
        println!(
            "result sentences: {} failures, each ASCII, one line, no brackets, <= {MAX_TEXT} characters, each says the first Build does the work",
            failures.len()
        );
    }

    #[test]
    fn the_sentences_name_their_cause() {
        let find = |f: Failure| Outcome::failed(&f).text;
        assert!(find(every_failure()[0].clone()).contains("https://static.rust-lang.org/rustup"));
        assert!(find(every_failure()[2].clone()).contains("C:"));
        assert!(find(every_failure()[3].clone()).contains("5007"));
        assert!(find(every_failure()[5].clone()).contains("still missing or does not work"));
        assert!(find(every_failure()[6].clone()).contains("forms-sql"));
        assert!(find(every_failure()[7].clone()).contains("for Debug"));
        assert_eq!(every_failure()[5].part(), Part::Check);
    }

    #[test]
    fn a_line_survives_the_round_trip_and_a_sentence_with_equals_signs_stays_whole() {
        let mut lines = Vec::new();
        for f in every_failure() {
            lines.push(Outcome::failed(&f));
        }
        lines.push(Outcome::ok(
            Part::Rust,
            "Rust 1.95.0 is installed. a=b text=c status=ok",
        ));
        lines.push(Outcome::skipped(
            Part::Debug,
            "Not compiled: the Release compile failed.",
        ));
        for o in &lines {
            let text = format(o);
            assert_eq!(parse(&text).as_ref(), Some(o), "{text}");
            assert!(!text.contains('\n'));
        }
        println!("result file: {} lines round-trip exactly", lines.len());
    }

    #[test]
    fn what_is_not_ours_is_not_an_outcome() {
        for line in [
            "",
            "PRC step=release unit=- crates=1 elapsed=1",
            "step=release",
            "step=release status=ok",
            "step=nowhere status=ok text=x",
            "step=release status=maybe text=x",
        ] {
            assert_eq!(parse(line), None, "'{line}'");
        }
    }

    #[test]
    fn clean_makes_a_sentence_safe_for_an_installer_page() {
        assert_eq!(clean("a\r\nb\tc  d"), "a b c d");
        assert_eq!(clean("[ProductName] caf\u{e9}"), "(ProductName) caf?");
        assert_eq!(clean("   "), "");
        let long = "x".repeat(1000);
        let c = clean(&long);
        assert!(c.len() <= MAX_TEXT && c.ends_with("..."), "{}", c.len());
    }

    #[test]
    fn a_part_that_ran_again_keeps_its_last_word_in_its_first_place() {
        let file = [
            format(&Outcome::ok(Part::Rust, "Rust is installed.")),
            "something the child printed".to_owned(),
            format(&Outcome::failed(&Failure::TimedOut {
                part: Part::Release,
                minutes: 30,
            })),
            format(&Outcome::ok(Part::Release, "Compiled for Release.")),
        ]
        .join("\r\n");
        let read = read(&file);
        assert_eq!(read.len(), 2);
        assert_eq!((read[0].part, read[0].status), (Part::Rust, Status::Ok));
        assert_eq!((read[1].part, read[1].status), (Part::Release, Status::Ok));
    }

    #[test]
    fn the_finish_page_says_nothing_unasked_and_the_whole_truth_when_asked() {
        // Not asked: nothing is said, whatever the file holds.
        assert_eq!(finish_text(false, &[Outcome::ok(Part::Rust, "done")]), "");

        // Asked, all well.
        let well = [
            Outcome::ok(Part::Rust, "Rust is installed."),
            Outcome::ok(Part::Release, "Release compiled."),
        ];
        assert!(finish_text(true, &well).contains("is prepared"));
        assert!(finish_text(true, &[]).contains("nothing to prepare"));

        // Asked, one failed and the next was skipped: both are named, and the install is still called installed.
        let sad = [
            Outcome::ok(Part::Rust, "Rust is installed."),
            Outcome::failed(&Failure::InstallFailed {
                part: Part::BuildTools,
                what: "The Visual Studio Build Tools".into(),
                code: Some(1603),
            }),
            Outcome::skipped(
                Part::Release,
                "Release was not compiled: the Build Tools are missing.",
            ),
        ];
        let page = finish_text(true, &sad);
        assert!(
            page.contains("is installed, but the preparation did not finish"),
            "{page}"
        );
        assert!(
            page.contains("1603") && page.contains("Release was not compiled"),
            "{page}"
        );
        assert!(!page.contains("is prepared"), "{page}");
        assert!(page.is_ascii());
    }

    #[test]
    fn a_step_that_failed_exits_with_a_code_and_one_that_did_not_exits_clean() {
        assert_eq!(
            [Status::Ok, Status::Skipped, Status::Failed].map(Status::exit_code),
            [0, 0, 1]
        );
    }
}
