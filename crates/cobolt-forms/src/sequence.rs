// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! **Animation sequences** (spec 109 S7, R33): a list of timed entries a program
//! creates with `COBOL::AnimationSequence("Create", name, script)` and runs with
//! `"Start"`.
//!
//! This module is pure: it reads a script, keeps the sequences of one form
//! instance and says which entries are due at a given time. It has no clock of
//! its own — the caller passes the time in whole milliseconds, so a test drives
//! it with exact numbers and the interpreter drives it with the wall clock.
//!
//! The script is data in a string, not COBOL syntax. One entry per line, or
//! separated by `;`:
//!
//! ```text
//! SHOW ctrl [AFTER n]
//! HIDE ctrl [AFTER n]
//! PLAY ctrl "animation" [AFTER n]
//! ```
//!
//! `AFTER n` counts whole milliseconds from the start of the sequence, never from
//! the previous entry, and is 0 when left out. Blank lines and `*>` comments are
//! ignored.

use std::collections::BTreeMap;

/// What an entry does to its control.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SeqAction {
    /// Set `Visible` on.
    Show,
    /// Set `Visible` off.
    Hide,
    /// Start the named animation on the control.
    Play(String),
}

/// One timed entry.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SeqEntry {
    /// Milliseconds after the sequence starts.
    pub at_ms: u64,
    pub control: String,
    pub action: SeqAction,
    /// 1-based line of the script, for diagnostics.
    pub line: usize,
}

/// A script that does not read: the line it stopped on and why.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SeqError {
    pub line: usize,
    pub text: String,
    pub message: String,
}

impl std::fmt::Display for SeqError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "line {}: {} (`{}`)", self.line, self.message, self.text)
    }
}

/// Split one script line into words; a quoted word keeps its spaces.
pub(crate) fn words(line: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut quote: Option<char> = None;
    let mut chars = line.chars().peekable();
    while let Some(c) = chars.next() {
        match quote {
            Some(q) if c == q => {
                quote = None;
                out.push(std::mem::take(&mut cur));
            }
            Some(_) => cur.push(c),
            None if c == '"' || c == '\'' => quote = Some(c),
            None if c == '*' && chars.peek() == Some(&'>') => break,
            None if c.is_whitespace() => {
                if !cur.is_empty() {
                    out.push(std::mem::take(&mut cur));
                }
            }
            None => cur.push(c),
        }
    }
    if !cur.is_empty() {
        out.push(cur);
    }
    out
}

/// The entries of one script line: split at `;` and cut at `*>`, neither of
/// which counts inside a quoted word.
pub(crate) fn entries_of(line: &str) -> Vec<String> {
    let mut out = vec![String::new()];
    let mut quote: Option<char> = None;
    let mut chars = line.chars().peekable();
    while let Some(c) = chars.next() {
        match quote {
            Some(q) => {
                if c == q {
                    quote = None;
                }
            }
            None if c == '"' || c == '\'' => quote = Some(c),
            None if c == '*' && chars.peek() == Some(&'>') => break,
            None if c == ';' => {
                out.push(String::new());
                continue;
            }
            None => {}
        }
        out.last_mut().expect("never empty").push(c);
    }
    out
}

/// Read a script into entries, in the order written.
pub fn parse_script(script: &str) -> Result<Vec<SeqEntry>, SeqError> {
    let mut entries = Vec::new();
    for (n, raw) in script.lines().enumerate() {
        let line = n + 1;
        for part in entries_of(raw) {
            let part = part.as_str();
            let w = words(part);
            if w.is_empty() {
                continue;
            }
            let fail = |message: &str| SeqError {
                line,
                text: part.trim().to_owned(),
                message: message.to_owned(),
            };
            let verb = w[0].to_ascii_uppercase();
            let (action, rest) = match verb.as_str() {
                "SHOW" | "HIDE" => {
                    let a = if verb == "SHOW" { SeqAction::Show } else { SeqAction::Hide };
                    (a, 1)
                }
                "PLAY" => {
                    let Some(anim) = w.get(2) else {
                        return Err(fail("PLAY needs a control and an animation name"));
                    };
                    (SeqAction::Play(anim.clone()), 2)
                }
                _ => return Err(fail("expected SHOW, HIDE or PLAY")),
            };
            let Some(control) = w.get(1) else {
                return Err(fail("the control is missing"));
            };
            let tail = &w[rest + 1..];
            let at_ms = match tail {
                [] => 0,
                [after, n] if after.eq_ignore_ascii_case("AFTER") => n
                    .parse::<u64>()
                    .map_err(|_| fail("AFTER takes a whole number of milliseconds"))?,
                _ => return Err(fail("expected AFTER <milliseconds> at the end")),
            };
            entries.push(SeqEntry { at_ms, control: control.clone(), action, line });
        }
    }
    Ok(entries)
}

#[derive(Clone, Debug)]
struct Sequence {
    /// Sorted by `at_ms`; equal times keep the order written.
    entries: Vec<SeqEntry>,
    /// Entries already fired in this run.
    next: usize,
    started_at: Option<u64>,
}

impl Sequence {
    fn playing(&self) -> bool {
        self.started_at.is_some() && self.next < self.entries.len()
    }
}

/// Why a call on the set was refused.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SeqCallError {
    /// No sequence of that name was created.
    Unknown(String),
}

/// The sequences of one form instance.
#[derive(Clone, Debug, Default)]
pub struct SequenceSet {
    sequences: BTreeMap<String, Sequence>,
}

fn key(name: &str) -> String {
    name.trim().to_ascii_uppercase()
}

impl SequenceSet {
    /// Register `entries` under `name`, replacing any earlier sequence of that
    /// name (a running one stops with it).
    pub fn create(&mut self, name: &str, mut entries: Vec<SeqEntry>) {
        entries.sort_by_key(|e| e.at_ms); // stable: equal times keep their order
        self.sequences.insert(key(name), Sequence { entries, next: 0, started_at: None });
    }

    fn get(&mut self, name: &str) -> Result<&mut Sequence, SeqCallError> {
        self.sequences.get_mut(&key(name)).ok_or_else(|| SeqCallError::Unknown(name.trim().to_owned()))
    }

    /// Run from the beginning at `now_ms`. A sequence that is already playing is
    /// left alone.
    pub fn start(&mut self, name: &str, now_ms: u64) -> Result<(), SeqCallError> {
        let s = self.get(name)?;
        if !s.playing() {
            s.next = 0;
            s.started_at = Some(now_ms);
        }
        Ok(())
    }

    /// Drop the entries not yet fired. What they already did stays done.
    pub fn stop(&mut self, name: &str) -> Result<(), SeqCallError> {
        let s = self.get(name)?;
        s.started_at = None;
        s.next = 0;
        Ok(())
    }

    /// `stop`, then `start` at `now_ms`.
    pub fn restart(&mut self, name: &str, now_ms: u64) -> Result<(), SeqCallError> {
        self.stop(name)?;
        self.start(name, now_ms)
    }

    /// Forget the sequence.
    pub fn delete(&mut self, name: &str) -> Result<(), SeqCallError> {
        self.sequences
            .remove(&key(name))
            .map(|_| ())
            .ok_or_else(|| SeqCallError::Unknown(name.trim().to_owned()))
    }

    /// `true` while entries remain to fire.
    pub fn is_playing(&mut self, name: &str) -> Result<bool, SeqCallError> {
        Ok(self.get(name)?.playing())
    }

    /// `true` when any sequence has entries left, so the caller knows to keep
    /// the clock running.
    pub fn any_playing(&self) -> bool {
        self.sequences.values().any(Sequence::playing)
    }

    /// Fire every entry due at `now_ms`, oldest first (by time, then by the
    /// order written, then by sequence name), and say which ones.
    pub fn advance(&mut self, now_ms: u64) -> Vec<SeqEntry> {
        let mut fired: Vec<(u64, usize, String, SeqEntry)> = Vec::new();
        for (name, s) in self.sequences.iter_mut() {
            let Some(t0) = s.started_at else { continue };
            while let Some(e) = s.entries.get(s.next) {
                if t0 + e.at_ms > now_ms {
                    break;
                }
                fired.push((t0 + e.at_ms, s.next, name.clone(), e.clone()));
                s.next += 1;
            }
            if s.next >= s.entries.len() {
                s.started_at = None;
                s.next = 0;
            }
        }
        fired.sort_by(|a, b| (a.0, &a.2, a.1).cmp(&(b.0, &b.2, b.1)));
        fired.into_iter().map(|f| f.3).collect()
    }

    /// Milliseconds from `now_ms` to the next entry due, or `None` when nothing
    /// is playing.
    pub fn next_due_in(&self, now_ms: u64) -> Option<u64> {
        self.sequences
            .values()
            .filter_map(|s| {
                let t0 = s.started_at?;
                s.entries.get(s.next).map(|e| (t0 + e.at_ms).saturating_sub(now_ms))
            })
            .min()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn set_with(script: &str) -> SequenceSet {
        let mut set = SequenceSet::default();
        set.create("INTRO", parse_script(script).unwrap());
        set
    }

    fn ids(v: &[SeqEntry]) -> Vec<&str> {
        v.iter().map(|e| e.control.as_str()).collect()
    }

    #[test]
    fn a_script_reads_in_both_separators_with_comments() {
        let e = parse_script("SHOW A AFTER 0\n\n*> a note\nplay B \"fade in\" after 50 ; HIDE C").unwrap();
        assert_eq!(e.len(), 3);
        assert_eq!(e[1].action, SeqAction::Play("fade in".into()));
        assert_eq!((e[1].at_ms, e[2].at_ms), (50, 0), "AFTER is optional and defaults to 0");
        assert_eq!(e[2].line, 4);
    }

    #[test]
    fn a_semicolon_in_a_comment_or_in_quotes_does_not_split() {
        let e = parse_script("SHOW A AFTER 5 *> then; HIDE B\nPLAY C \"a;b\"").unwrap();
        assert_eq!(e.len(), 2);
        assert_eq!(e[1].action, SeqAction::Play("a;b".into()));
    }

    #[test]
    fn a_bad_line_is_named_with_its_number() {
        let err = parse_script("SHOW A\nFLY B AFTER 1").unwrap_err();
        assert_eq!(err.line, 2);
        assert!(err.message.contains("SHOW, HIDE or PLAY"));
        assert!(parse_script("SHOW A AFTER soon").is_err());
        assert!(parse_script("SHOW A AFTER -5").is_err());
        assert!(parse_script("PLAY A").is_err());
        assert!(parse_script("SHOW").is_err());
    }

    #[test]
    fn offsets_are_absolute_and_equal_times_keep_the_order_written() {
        let mut set = set_with("SHOW LATE AFTER 100\nSHOW FIRST AFTER 0\nHIDE SECOND AFTER 0\nSHOW MID AFTER 50");
        set.start("intro", 1000).unwrap();
        assert_eq!(ids(&set.advance(1000)), ["FIRST", "SECOND"]);
        assert!(set.is_playing("INTRO").unwrap());
        assert!(set.advance(1049).is_empty());
        assert_eq!(ids(&set.advance(1050)), ["MID"]);
        assert_eq!(ids(&set.advance(5000)), ["LATE"]);
        assert!(!set.is_playing("INTRO").unwrap(), "stopped once the last entry fired");
        assert!(set.advance(9000).is_empty(), "nothing fires twice");
    }

    #[test]
    fn a_late_clock_fires_everything_due_in_time_order() {
        let mut set = set_with("SHOW B AFTER 20\nSHOW A AFTER 10");
        set.start("INTRO", 0).unwrap();
        assert_eq!(ids(&set.advance(500)), ["A", "B"]);
    }

    #[test]
    fn stop_drops_what_is_left_and_start_begins_again() {
        let mut set = set_with("SHOW A AFTER 0\nSHOW B AFTER 100");
        set.start("INTRO", 0).unwrap();
        assert_eq!(ids(&set.advance(10)), ["A"]);
        set.stop("INTRO").unwrap();
        assert!(!set.is_playing("INTRO").unwrap());
        assert!(set.advance(200).is_empty(), "B was dropped");
        set.start("INTRO", 300).unwrap();
        assert_eq!(ids(&set.advance(300)), ["A"], "a new run starts from the first entry");
    }

    #[test]
    fn start_on_a_running_sequence_is_left_alone_and_restart_begins_again() {
        let mut set = set_with("SHOW A AFTER 0\nSHOW B AFTER 100");
        set.start("INTRO", 0).unwrap();
        set.advance(10);
        set.start("INTRO", 50).unwrap();
        assert_eq!(ids(&set.advance(100)), ["B"], "the second start changed nothing");
        let mut set = set_with("SHOW A AFTER 0\nSHOW B AFTER 100");
        set.start("INTRO", 0).unwrap();
        set.advance(10);
        set.restart("INTRO", 60).unwrap();
        assert_eq!(ids(&set.advance(60)), ["A"]);
    }

    #[test]
    fn two_sequences_run_independently_and_an_unknown_name_is_refused() {
        let mut set = SequenceSet::default();
        set.create("ONE", parse_script("SHOW A AFTER 10").unwrap());
        set.create("TWO", parse_script("SHOW B AFTER 10").unwrap());
        set.start("ONE", 0).unwrap();
        set.start("TWO", 5).unwrap();
        assert_eq!(ids(&set.advance(10)), ["A"]);
        assert_eq!(set.next_due_in(10), Some(5));
        assert_eq!(ids(&set.advance(15)), ["B"]);
        assert!(!set.any_playing());
        assert_eq!(set.start("NOPE", 0), Err(SeqCallError::Unknown("NOPE".into())));
        assert!(set.is_playing("NOPE").is_err());
        assert!(set.delete("TWO").is_ok());
        assert!(set.delete("TWO").is_err());
    }

    #[test]
    fn create_replaces_a_running_sequence() {
        let mut set = set_with("SHOW A AFTER 100");
        set.start("INTRO", 0).unwrap();
        set.create("INTRO", parse_script("SHOW Z AFTER 0").unwrap());
        assert!(!set.is_playing("INTRO").unwrap());
    }
}
