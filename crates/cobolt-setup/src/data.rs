// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors

//! What a custom action is told to run: the text of its `CustomActionData`.
//!
//! A deferred custom action cannot read the install's properties, only this one
//! string, so everything the supervisor needs travels in it. It is four fields
//! separated by `|`, the last of which is the command line and may itself
//! contain `|`:
//!
//! ```text
//! asuser|900|0|"C:\Program Files\PowerRustCOBOL\PowerRustCOBOL.exe" --prepare-platform ...
//! ^      ^   ^ ^
//! |      |   | the command line, handed to CreateProcess as it is
//! |      |   kill the child when the developer cancels? 1 = yes, anything else = wait
//! |      seconds without a line of output before the step is given up (0 = never)
//! who the child runs as: `plain` (as the action itself) or `asuser` (the user's token)
//! ```

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
    /// The whole command line.
    pub command: String,
}

impl RunData {
    /// Parse the text. A mistake is a sentence, never a panic: the caller reports it and the install goes on.
    pub fn parse(text: &str) -> Result<RunData, String> {
        let mut parts = text.splitn(4, '|');
        let who = parts.next().unwrap_or("").trim();
        let watchdog = parts
            .next()
            .ok_or("the custom action data has no watchdog field")?
            .trim();
        let kill = parts
            .next()
            .ok_or("the custom action data has no kill field")?
            .trim();
        let command = parts
            .next()
            .ok_or("the custom action data has no command")?
            .trim();

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
        if command.is_empty() {
            return Err("the custom action data has an empty command".to_owned());
        }
        Ok(RunData {
            spawn,
            watchdog_secs,
            kill_on_cancel: kill == "1",
            command: command.to_owned(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_full_line_is_read() {
        let d = RunData::parse(r#"asuser|900|1|"C:\Program Files\X\x.exe" --step a|b"#).unwrap();
        assert_eq!(d.spawn, Spawn::AsUser);
        assert_eq!(d.watchdog_secs, 900);
        assert!(d.kill_on_cancel);
        assert_eq!(
            d.command, r#""C:\Program Files\X\x.exe" --step a|b"#,
            "a | inside the command is kept"
        );
    }

    #[test]
    fn empty_fields_mean_off() {
        // `[SPIKE_KILL]` expands to nothing when the property is not set, which must read as "wait".
        let d = RunData::parse("plain|||cmd.exe /c echo hi").unwrap();
        assert_eq!(d.spawn, Spawn::Plain);
        assert_eq!(d.watchdog_secs, 0);
        assert!(!d.kill_on_cancel);
    }

    #[test]
    fn a_mistake_is_a_sentence() {
        let cases = [
            ("", "no watchdog"),
            ("plain|10", "no kill"),
            ("plain|10|0", "no command"),
            ("nobody|10|0|x", "unknown identity"),
            ("plain|soon|0|x", "not a number"),
            ("plain|10|0| ", "empty command"),
        ];
        for (text, wants) in cases {
            let err = RunData::parse(text).expect_err(text);
            assert!(
                err.contains(wants),
                "'{text}' gave '{err}', expected it to mention '{wants}'"
            );
        }
    }
}
