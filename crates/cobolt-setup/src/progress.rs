// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors

//! The progress line: what the step prints and what the progress page shows (R13).
//!
//! The headless `PowerRustCOBOL.exe --prepare-platform` prints one line per
//! event on its standard output, and the supervisor, which started it, reads
//! them. They are `key=value` text in ASCII so that a person reading an install
//! log can read them too:
//!
//! ```text
//! PRC step=release unit=forms crates=120 elapsed=734
//! PRC step=build-tools unit=- crates=0 elapsed=65
//! PRC note=Downloading rustup-init.exe
//! ```
//!
//! The producer and the consumer are both written against this module, and the
//! round trip is tested here, so neither can change the grammar alone.

/// The part of the preparation a line is about.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Part {
    /// Installing Rust.
    Rust,
    /// Installing the Visual Studio Build Tools.
    BuildTools,
    /// Checking that the requirements work (the link probe).
    Check,
    /// Compiling the platform, release profile.
    Release,
    /// Compiling the platform, debug profile.
    Debug,
}

impl Part {
    /// The name in the grammar and in the result file.
    pub fn name(self) -> &'static str {
        match self {
            Part::Rust => "rust",
            Part::BuildTools => "build-tools",
            Part::Check => "check",
            Part::Release => "release",
            Part::Debug => "debug",
        }
    }

    /// The part a name stands for.
    pub fn from_name(name: &str) -> Option<Part> {
        [
            Part::Rust,
            Part::BuildTools,
            Part::Check,
            Part::Release,
            Part::Debug,
        ]
        .into_iter()
        .find(|p| p.name() == name)
    }

    /// How the progress page names it.
    pub fn words(self) -> &'static str {
        match self {
            Part::Rust => "Installing Rust",
            Part::BuildTools => "Installing the Visual Studio Build Tools",
            Part::Check => "Checking the requirements",
            Part::Release => "Compiling the platform for Release",
            Part::Debug => "Compiling the platform for Debug",
        }
    }
}

/// One line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Line {
    /// Where a part is: the variant being compiled, the crates compiled so far, the seconds spent on this part.
    At {
        /// The part.
        part: Part,
        /// The variant of the platform (`forms`, `forms-sql`, `forms-graphics`), when the part has one.
        unit: Option<String>,
        /// Crates compiled so far in this part.
        crates: u32,
        /// Seconds since this part started.
        elapsed_secs: u64,
    },
    /// A sentence about what the step is doing.
    Note(String),
}

/// The text of a line, as it is printed.
pub fn format(line: &Line) -> String {
    match line {
        Line::At {
            part,
            unit,
            crates,
            elapsed_secs,
        } => {
            format!(
                "PRC step={} unit={} crates={crates} elapsed={elapsed_secs}",
                part.name(),
                unit.as_deref().unwrap_or("-")
            )
        }
        Line::Note(text) => format!("PRC note={}", one_line(text)),
    }
}

fn one_line(text: &str) -> String {
    text.chars()
        .map(|c| if c.is_control() { ' ' } else { c })
        .collect::<String>()
        .trim()
        .to_owned()
}

/// A line read back, or `None` when it is not one of ours (the child's other output, a malformed line).
pub fn parse(text: &str) -> Option<Line> {
    let rest = text.trim().strip_prefix("PRC ")?;
    if let Some(note) = rest.strip_prefix("note=") {
        let note = note.trim();
        return (!note.is_empty()).then(|| Line::Note(note.to_owned()));
    }
    let (mut part, mut unit, mut crates, mut elapsed) = (None, None, None, None);
    for pair in rest.split_whitespace() {
        let (key, value) = pair.split_once('=')?;
        match key {
            "step" => part = Some(Part::from_name(value)?),
            "unit" => unit = Some((value != "-").then(|| value.to_owned())),
            "crates" => crates = Some(value.parse::<u32>().ok()?),
            "elapsed" => elapsed = Some(value.parse::<u64>().ok()?),
            _ => {} // a key a later version adds: ignored, so an older reader still works
        }
    }
    Some(Line::At {
        part: part?,
        unit: unit.unwrap_or(None),
        crates: crates?,
        elapsed_secs: elapsed?,
    })
}

/// Seconds as a person reads a clock: `45 s`, `12 min 4 s`, `1 h 02 min`.
pub fn clock(secs: u64) -> String {
    match secs {
        0..=59 => format!("{secs} s"),
        60..=3599 => format!("{} min {} s", secs / 60, secs % 60),
        _ => format!("{} h {:02} min", secs / 3600, secs % 3600 / 60),
    }
}

/// What the progress page shows for a line: the part, where it is, how long it has taken.
pub fn display(line: &Line) -> String {
    match line {
        Line::At {
            part,
            unit,
            crates,
            elapsed_secs,
        } => {
            let mut s = part.words().to_owned();
            if let Some(unit) = unit {
                s.push_str(&format!(" ({unit})"));
            }
            if *crates > 0 {
                s.push_str(&format!(": {crates} crates compiled"));
            }
            s.push_str(&format!(", {} so far", clock(*elapsed_secs)));
            s
        }
        Line::Note(text) => text.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_kind_of_line_survives_the_round_trip() {
        let mut lines = vec![
            Line::Note("Downloading rustup-init.exe".into()),
            Line::Note("x=1 and = signs stay".into()),
        ];
        let mut count = 2;
        for part in [
            Part::Rust,
            Part::BuildTools,
            Part::Check,
            Part::Release,
            Part::Debug,
        ] {
            for unit in [
                None,
                Some("forms".to_owned()),
                Some("forms-sql".to_owned()),
                Some("forms-graphics".to_owned()),
            ] {
                for (crates, elapsed_secs) in [(0, 0), (1, 5), (412, 3_661), (u32::MAX, u64::MAX)] {
                    lines.push(Line::At {
                        part,
                        unit: unit.clone(),
                        crates,
                        elapsed_secs,
                    });
                    count += 1;
                }
            }
        }
        for line in &lines {
            let text = format(line);
            assert!(text.is_ascii(), "{text}");
            assert_eq!(parse(&text).as_ref(), Some(line), "{text}");
        }
        println!("progress line: {count} lines (5 parts x 4 units x 4 counts, and notes) round-trip exactly and are ASCII");
    }

    #[test]
    fn what_is_not_ours_or_is_malformed_is_not_a_line() {
        for text in [
            "",
            "Compiling serde v1.0",
            "PRC",
            "PRC step=release",
            "PRC step=nowhere unit=- crates=1 elapsed=1",
            "PRC step=release unit=- crates=many elapsed=1",
            "PRC step=release unit=- crates=1 elapsed=-4",
            "PRC step=release unit=- crates=1 elapsed=1 stray",
            "PRC note=",
            "prc step=release unit=- crates=1 elapsed=1",
        ] {
            assert_eq!(parse(text), None, "'{text}'");
        }
    }

    #[test]
    fn a_key_a_later_version_adds_is_ignored() {
        let line = parse("PRC step=debug unit=forms crates=3 elapsed=9 eta=77").unwrap();
        assert_eq!(
            line,
            Line::At {
                part: Part::Debug,
                unit: Some("forms".into()),
                crates: 3,
                elapsed_secs: 9
            }
        );
    }

    #[test]
    fn a_note_with_a_line_break_stays_one_line() {
        let text = format(&Line::Note("first\r\nsecond\tthird".into()));
        assert!(
            !text.contains('\n') && !text.contains('\r') && !text.contains('\t'),
            "{text:?}"
        );
        assert_eq!(parse(&text), Some(Line::Note("first  second third".into())));
    }

    #[test]
    fn the_page_shows_the_part_where_it_is_and_how_long() {
        let cases = [
            (Line::At { part: Part::Release, unit: Some("forms".into()), crates: 120, elapsed_secs: 734 }, "Compiling the platform for Release (forms): 120 crates compiled, 12 min 14 s so far"),
            (Line::At { part: Part::BuildTools, unit: None, crates: 0, elapsed_secs: 65 }, "Installing the Visual Studio Build Tools, 1 min 5 s so far"),
            (Line::At { part: Part::Debug, unit: None, crates: 9, elapsed_secs: 3_725 }, "Compiling the platform for Debug: 9 crates compiled, 1 h 02 min so far"),
            (Line::Note("Downloading rustup-init.exe".into()), "Downloading rustup-init.exe"),
        ];
        for (line, words) in cases {
            assert_eq!(display(&line), words);
        }
        assert_eq!(
            [clock(0), clock(59), clock(60), clock(3_599), clock(3_600)],
            ["0 s", "59 s", "1 min 0 s", "59 min 59 s", "1 h 00 min"]
        );
    }
}
