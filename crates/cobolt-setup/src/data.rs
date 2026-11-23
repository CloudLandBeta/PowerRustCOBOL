// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors

//! What a custom action is told to run: the text of its `CustomActionData`.
//!
//! A deferred custom action cannot read the install's properties, only this one
//! string, so everything the supervisor needs travels in it. It is eight fields
//! separated by `|`, the last of which is the command line and may itself
//! contain `|`:
//!
//! ```text
//! asuser|900|1|release|950|C:\...\result.txt|C:\...\cancel.txt|"C:\Program Files\PowerRustCOBOL\PowerRustCOBOL.exe" --prepare-platform ...
//! ^      ^   ^ ^       ^   ^                ^                  ^
//! |      |   | |       |   |                |                  the command line, handed to CreateProcess as it is
//! |      |   | |       |   |                the file whose appearance asks the step to stop (empty = none)
//! |      |   | |       |   the result file the supervisor appends to when the step cannot (empty = none)
//! |      |   | |       the progress ticks this step may move the bar by (empty = 0)
//! |      |   | the part it is about until its own lines say otherwise (empty = unknown)
//! |      |   kill the child when the developer cancels? 1 = yes, anything else = wait
//! |      seconds without a line of output before the step is given up (0 = never)
//! who the child runs as: `plain` (as the action itself) or `asuser` (the user's token)
//! ```
//!
//! An empty field means "off", so a property that is not set (`[NAME]` expands
//! to nothing) never breaks the line.

use crate::progress::Part;

/// How many fields the text has.
const FIELDS: usize = 8;

/// Whose identity the child process has.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Spawn {
    /// The same identity as the custom action: `SYSTEM` for an action that is not impersonated.
    Plain,
    /// The installing user's primary token and that user's environment block.
    AsUser,
}

/// The parsed `CustomActionData` of one step.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunData {
    /// Whose identity the child has.
    pub spawn: Spawn,
    /// Seconds without a line of output after which the child is stopped. `0` disables the watchdog.
    pub watchdog_secs: u64,
    /// Whether a Cancel stops the child (`true`) or waits for it to finish (`false`).
    pub kill_on_cancel: bool,
    /// The part the step is about, until a line of its own says another: what a failure the supervisor writes
    /// itself is attributed to.
    pub part: Option<Part>,
    /// The progress ticks this step may move the installer's bar by.
    pub ticks: u32,
    /// The result file the supervisor appends to when the step could not (it timed out, crashed or never started).
    pub result_file: Option<String>,
    /// The file the supervisor creates to ask the step to stop before it kills it.
    pub cancel_file: Option<String>,
    /// The whole command line.
    pub command: String,
}

impl RunData {
    /// Parse the text. A mistake is a sentence, never a panic: the caller reports it and the install goes on.
    pub fn parse(text: &str) -> Result<RunData, String> {
        let mut parts = text.splitn(FIELDS, '|');
        let mut next = |what: &str| -> Result<&str, String> {
            parts
                .next()
                .map(str::trim)
                .ok_or_else(|| format!("the custom action data has no {what} field"))
        };
        let who = next("identity")?;
        let watchdog = next("watchdog")?;
        let kill = next("kill")?;
        let part = next("part")?;
        let ticks = next("ticks")?;
        let result = next("result file")?;
        let cancel = next("cancel file")?;
        let command = next("command")?;

        let spawn = match who {
            "plain" => Spawn::Plain,
            "asuser" => Spawn::AsUser,
            other => {
                return Err(format!(
                    "unknown identity '{other}' (expected plain or asuser)"
                ))
            }
        };
        let watchdog_secs = if watchdog.is_empty() {
            0
        } else {
            watchdog
                .parse::<u64>()
                .map_err(|_| format!("the watchdog '{watchdog}' is not a number of seconds"))?
        };
        let part = if part.is_empty() {
            None
        } else {
            Some(Part::from_name(part).ok_or_else(|| format!("unknown part '{part}'"))?)
        };
        let ticks = if ticks.is_empty() {
            0
        } else {
            ticks
                .parse::<u32>()
                .map_err(|_| format!("the ticks '{ticks}' are not a number"))?
        };
        if command.is_empty() {
            return Err("the custom action data has an empty command".to_owned());
        }
        let file = |path: &str| (!path.is_empty()).then(|| path.to_owned());
        Ok(RunData {
            spawn,
            watchdog_secs,
            kill_on_cancel: kill == "1",
            part,
            ticks,
            result_file: file(result),
            cancel_file: file(cancel),
            command: command.to_owned(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_full_line_is_read() {
        let d = RunData::parse(
            r#"asuser|900|1|release|950|C:\T\result.txt|C:\T\cancel.txt|"C:\Program Files\X\x.exe" --step a|b"#,
        )
        .unwrap();
        assert_eq!(d.spawn, Spawn::AsUser);
        assert_eq!(d.watchdog_secs, 900);
        assert!(d.kill_on_cancel);
        assert_eq!(d.part, Some(Part::Release));
        assert_eq!(d.ticks, 950);
        assert_eq!(d.result_file.as_deref(), Some(r"C:\T\result.txt"));
        assert_eq!(d.cancel_file.as_deref(), Some(r"C:\T\cancel.txt"));
        assert_eq!(
            d.command, r#""C:\Program Files\X\x.exe" --step a|b"#,
            "a | inside the command is kept"
        );
    }

    #[test]
    fn empty_fields_mean_off() {
        // `[SPIKE_KILL]` expands to nothing when the property is not set, which must read as "wait".
        let d = RunData::parse("plain|||||||cmd.exe /c echo hi").unwrap();
        assert_eq!(d.spawn, Spawn::Plain);
        assert_eq!(d.watchdog_secs, 0);
        assert!(!d.kill_on_cancel);
        assert_eq!((d.part, d.ticks), (None, 0));
        assert_eq!((d.result_file, d.cancel_file), (None, None));
        assert_eq!(d.command, "cmd.exe /c echo hi");
    }

    #[test]
    fn a_mistake_is_a_sentence() {
        let cases = [
            ("", "no watchdog"),
            ("plain", "no watchdog"),
            ("plain|10", "no kill"),
            ("plain|10|0", "no part"),
            ("plain|10|0|", "no ticks"),
            ("plain|10|0||", "no result file"),
            ("plain|10|0|||", "no cancel file"),
            ("plain|10|0||||", "no command"),
            ("nobody|10|0|||||x", "unknown identity"),
            ("plain|soon|0|||||x", "not a number"),
            ("plain|10|0|nowhere||||x", "unknown part"),
            ("plain|10|0||many|||x", "not a number"),
            ("plain|10|0||||| ", "empty command"),
        ];
        for (text, wants) in cases {
            let err = RunData::parse(text).expect_err(text);
            assert!(
                err.contains(wants),
                "'{text}' gave '{err}', expected it to mention '{wants}'"
            );
        }
    }

    #[test]
    fn every_part_name_the_headless_step_writes_is_understood() {
        let mut n = 0;
        for part in [
            Part::Rust,
            Part::BuildTools,
            Part::Check,
            Part::Release,
            Part::Debug,
        ] {
            let d = RunData::parse(&format!("plain|0|0|{}|0|||x", part.name())).unwrap();
            assert_eq!(d.part, Some(part));
            n += 1;
        }
        println!("custom action data: {n} part names read back");
    }
}
