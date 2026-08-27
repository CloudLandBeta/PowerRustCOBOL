// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Source format detection and preprocessing for COBOL source files.
//!
//! # COBOL source formats
//!
//! ## Fixed-form (traditional, pre-2002)
//!
//! ```text
//! Col:  1     6 7  8   11  12                                      72 73    80
//!       |-----| |  |---|   |--------------------------------------- | |------|
//!       SeqNum  I  AreaA   Area B (active source)                    Ident
//! ```
//!
//! - Columns 1–6:   Sequence number (ignored)
//! - Column 7:      Indicator area
//!   - `*` or `/`  → comment line
//!   - `-`          → continuation of previous line's non-terminated literal
//!   - `D`          → debugging line (treated as comment unless debug mode)
//!   - ` `          → normal source line
//! - Columns 8–11:  Area A (division/section/paragraph headers, FD, 01, 77)
//! - Columns 12–72: Area B (statements)
//! - Columns 73–80: Program identification — discarded by
//!                  [`SourceFormat::FixedStrict`], kept as source by
//!                  [`SourceFormat::Fixed`] (see below)
//!
//! ### The 72-column limit is not enforced *by [`SourceFormat::Fixed`]*
//!
//! Classic fixed format stops the source at column 72 and treats 73–80 as a
//! card-deck sequence area. [`SourceFormat::Fixed`] reads the line to its end
//! instead. Nobody punches cards; what the limit actually did was delete code —
//! silently, mid-token, with the error surfacing somewhere else entirely. It
//! also made `EXEC RUST` unusable in a form, because every generated `.cbl`
//! carries a banner whose `*` sits in column 7, so the file was read as fixed
//! and the embedded Rust — which has no column rules at all — was chopped at 72.
//!
//! The sequence area (1–6) and the indicator column (7) are still honoured:
//! those carry meaning. Only the right-hand limit is gone.
//!
//! ### …but real card-image source needs it: [`SourceFormat::FixedStrict`]
//!
//! Source that genuinely *is* in the classic reference format — a mainframe
//! export, or the NIST CCVS85 validation suite — carries a program stamp in
//! columns 73–80 of every line, and continues literals across lines with a `-`
//! in column 7. Reading it with the relaxed rules above fails completely: of
//! CCVS85's 459 programs, **none** parsed until [`SourceFormat::FixedStrict`]
//! existed, because the stamp glued itself onto every statement and an
//! unbalanced quotation mark swallowed whole programs.
//!
//! [`SourceFormat::FixedStrict`] applies every column rule and joins
//! continuation lines. It is chosen explicitly (`rcrun --source-format=fixed`),
//! never by detection — applying the column rules to source not written for
//! them is exactly the regression the relaxed reading exists to prevent.
//!
//! ## Free-form (COBOL 2002+, Fujitsu extension)
//!
//! No column restrictions.  `*>` starts a comment to end of line.
//! Continuation lines use `&` at the end of the continued line.

/// The source format of a COBOL file.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SourceFormat {
    /// Traditional punch-card column layout (default for Fujitsu COBOL).
    ///
    /// PowerRustCOBOL's *relaxed* reading: the sequence area and the indicator
    /// column are honoured, but the line runs as far as the developer typed
    /// (see the module docs on the 72-column limit). This is what generated
    /// form `.cbl` sources and `EXEC RUST` blocks need.
    #[default]
    Fixed,
    /// Free-form layout (COBOL 2002, Fujitsu free-form option).
    Free,
    /// **Classic COBOL-85 reference format** — the format the standard defines
    /// and that card-image source such as the NIST CCVS85 validation suite is
    /// written in. Unlike [`SourceFormat::Fixed`] it applies every column rule:
    ///
    /// - columns 1-6   sequence area, ignored;
    /// - column 7      indicator: `*` `/` comment, `-` continuation,
    ///                 `D` debugging line (a comment unless debugging mode);
    /// - columns 8-72  the source itself;
    /// - columns 73-80 identification area, **discarded**.
    ///
    /// Continuation lines are joined per the standard, including the
    /// alphanumeric-literal form where the continued fragment runs to column 72
    /// and the continuation line resumes after its opening quotation mark.
    ///
    /// Selected explicitly (`rcrun --source-format=fixed`), never by detection:
    /// applying these rules to source that was not written for them silently
    /// deletes code, which is exactly the 2026-08-05 regression the relaxed
    /// reading exists to prevent.
    FixedStrict,
}

impl SourceFormat {
    /// Guess the format by inspecting the first few non-empty lines.
    pub fn detect(source: &str) -> Self {
        for line in source.lines().take(20) {
            if line.starts_with("*>") || line.starts_with("      *>") {
                return Self::Free;
            }
        }
        Self::Fixed
    }
}

// ── Preprocessed source ───────────────────────────────────────────────────────

/// A single logical source line after preprocessing.
#[derive(Debug, Clone)]
pub struct SourceLine {
    pub content: String,
    pub line_number: u32,
    pub byte_offset: usize,
    pub is_comment: bool,
    pub comment_text: Option<String>,
}

/// Preprocess a complete COBOL source string into a vector of [`SourceLine`]s.
pub fn preprocess(source: &str, format: SourceFormat) -> Vec<SourceLine> {
    match format {
        SourceFormat::Fixed => preprocess_fixed(source),
        SourceFormat::FixedStrict => preprocess_fixed_strict(source),
        SourceFormat::Free => preprocess_free(source),
    }
}

/// Line view of the classic reference format.
///
/// Content comes from [`flatten_fixed_strict`] so this agrees exactly with what
/// the lexer sees — continuation lines joined the standard's way, not the
/// approximate way [`preprocess_fixed`] joins them. Comment text is recovered
/// from the original lines, which the flattener drops.
fn preprocess_fixed_strict(source: &str) -> Vec<SourceLine> {
    let flat = flatten_fixed_strict(source);
    let mut lines: Vec<SourceLine> = Vec::new();
    let mut byte_offset = 0usize;

    for (idx, (raw_line, flat_line)) in source.lines().zip(flat.lines()).enumerate() {
        let clipped = &raw_line[..char_boundary_at_col(raw_line, 72)];
        let indicator = clipped.chars().nth(6).unwrap_or(' ');
        let is_comment = matches!(indicator, '*' | '/');
        let is_debug = matches!(indicator, 'D' | 'd');
        let active = clipped
            .get(char_boundary_at_col(clipped, 7)..)
            .unwrap_or("");

        lines.push(SourceLine {
            content: flat_line.to_string(),
            line_number: (idx + 1) as u32,
            byte_offset,
            is_comment: is_comment || (is_debug && !requests_debugging_mode(source)),
            comment_text: if is_comment {
                Some(active.trim().to_string())
            } else if is_debug {
                Some(format!("(debug) {}", active.trim()))
            } else {
                None
            },
        });
        byte_offset += flat_line.len() + 1;
    }
    lines
}

/// Drop the identification area (columns 73-80) from every line.
///
/// Char-column based, so a multi-byte character never splits.
pub fn clip_to_col72(source: &str) -> String {
    let mut out = String::with_capacity(source.len());
    for line in source.lines() {
        let cut = char_boundary_at_col(line, 72);
        out.push_str(&line[..cut]);
        out.push('\n');
    }
    out
}

fn preprocess_fixed(source: &str) -> Vec<SourceLine> {
    let mut lines: Vec<SourceLine> = Vec::new();
    let mut byte_offset: usize = 0;

    for (line_number, raw_line) in source.lines().enumerate() {
        let line_number = (line_number + 1) as u32;
        let raw_bytes = raw_line.len();

        if raw_bytes < 7 {
            byte_offset += raw_bytes + 1;
            lines.push(SourceLine {
                content: String::new(),
                line_number,
                byte_offset,
                is_comment: false,
                comment_text: None,
            });
            continue;
        }

        let indicator = raw_line.chars().nth(6).unwrap_or(' ');
        let is_comment = matches!(indicator, '*' | '/');
        let is_continuation = indicator == '-';
        let is_debug = indicator == 'D';

        // Use char-column boundaries so multi-byte characters don't cause panics.
        // Active source runs from column 8 to the end of the line: the 72-column
        // limit is not enforced (see `flatten_fixed`).
        let col7_byte = char_boundary_at_col(raw_line, 7);
        let active = if raw_bytes > 7 {
            &raw_line[col7_byte..]
        } else {
            ""
        };
        let active_byte_offset = byte_offset + 7;

        if is_comment {
            lines.push(SourceLine {
                content: String::new(),
                line_number,
                byte_offset: active_byte_offset,
                is_comment: true,
                comment_text: Some(active.trim().to_string()),
            });
        } else if is_continuation {
            let cont_content = active.trim_start().to_string();
            if let Some(prev) = lines.iter_mut().rev().find(|l| !l.is_comment) {
                prev.content.push_str(&cont_content);
            }
        } else if is_debug {
            lines.push(SourceLine {
                content: String::new(),
                line_number,
                byte_offset: active_byte_offset,
                is_comment: true,
                comment_text: Some(format!("(debug) {}", active.trim())),
            });
        } else {
            lines.push(SourceLine {
                content: active.to_string(),
                line_number,
                byte_offset: active_byte_offset,
                is_comment: false,
                comment_text: None,
            });
        }

        byte_offset += raw_bytes + 1;
    }

    lines
}

fn preprocess_free(source: &str) -> Vec<SourceLine> {
    let mut lines: Vec<SourceLine> = Vec::new();
    let mut byte_offset: usize = 0;

    for (line_number, raw_line) in source.lines().enumerate() {
        let line_number = (line_number + 1) as u32;
        let raw_bytes = raw_line.len();
        let (active, comment) = strip_free_comment(raw_line);
        let is_comment = active.trim().is_empty() && comment.is_some();

        lines.push(SourceLine {
            content: active.to_string(),
            line_number,
            byte_offset,
            is_comment,
            comment_text: comment,
        });

        byte_offset += raw_bytes + 1;
    }

    lines
}

/// Split a free-form source line at the first `*>` comment marker.
fn strip_free_comment(line: &str) -> (&str, Option<String>) {
    let bytes = line.as_bytes();
    let mut i = 0;
    let mut in_string: Option<u8> = None;

    while i < bytes.len() {
        let b = bytes[i];
        match in_string {
            Some(q) if b == q => {
                if bytes.get(i + 1) == Some(&q) {
                    i += 2;
                } else {
                    in_string = None;
                    i += 1;
                }
            }
            Some(_) => {
                i += 1;
            }
            None => {
                if b == b'"' || b == b'\'' {
                    in_string = Some(b);
                    i += 1;
                } else if bytes.get(i..i + 2) == Some(b"*>") {
                    let active = &line[..i];
                    let comment = line[i + 2..].trim().to_string();
                    return (active, Some(comment));
                } else {
                    i += 1;
                }
            }
        }
    }
    (line, None)
}

// ── Flat source builder ───────────────────────────────────────────────────────

/// Produce a single flat string for the logos lexer, replacing fixed-form
/// dead zones (sequence numbers, identification area) with spaces to preserve
/// byte offsets for accurate span reporting.
/// Return the byte offset of the character boundary that is at or before
/// `char_col` *columns* (0-based) from the start of `s`.
/// Because COBOL fixed-format counts character positions (not bytes), we
/// advance by characters and return the corresponding byte index.
fn char_boundary_at_col(s: &str, char_col: usize) -> usize {
    let mut col = 0usize;
    for (byte_idx, _ch) in s.char_indices() {
        if col >= char_col {
            return byte_idx;
        }
        col += 1;
    }
    s.len() // past end
}

// ── Classic COBOL-85 reference format ─────────────────────────────────────────

/// Width of Area A + Area B: columns 8 through 72 inclusive.
const AREA_WIDTH: usize = 65;

/// Track whether a source fragment ends *inside* an alphanumeric literal.
///
/// `open` carries the delimiter of a literal that is still open when the
/// fragment starts. A doubled delimiter (`""`) is the COBOL escape and does not
/// close the literal.
fn literal_state(fragment: &str, open: Option<char>) -> Option<char> {
    let chars: Vec<char> = fragment.chars().collect();
    let mut state = open;
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        match state {
            Some(delim) => {
                if c == delim {
                    if chars.get(i + 1) == Some(&delim) {
                        i += 1; // escaped delimiter — literal stays open
                    } else {
                        state = None;
                    }
                }
            }
            None => {
                if c == '"' || c == '\'' {
                    state = Some(c);
                }
            }
        }
        i += 1;
    }
    state
}

/// True when the program asks for its debugging lines to be compiled.
///
/// COBOL-85 puts a `D` (or `d`) in the indicator area to mark a **debugging
/// line**, and the default is that such a line is a *comment*. It is compiled
/// only when the program says so:
///
/// ```cobol
///        SOURCE-COMPUTER. XYZ WITH DEBUGGING MODE.
/// ```
///
/// This is a source-format decision — the indicator area only exists in fixed
/// format — so it is answered here rather than in the parser, and it has to be
/// answered before any line is classified. Comment lines are skipped so that a
/// program merely *describing* debugging mode in its prose does not switch it
/// on; CCVS85 programs discuss it in their banners.
///
/// Free format has no indicator area, so it has no debugging lines at all: a
/// `D` there is an ordinary COBOL word and is left alone.
pub(crate) fn requests_debugging_mode(source: &str) -> bool {
    source.lines().any(|line| {
        let indicator = line.chars().nth(6).unwrap_or(' ');
        if matches!(indicator, '*' | '/') {
            return false;
        }
        let upper = line.to_ascii_uppercase();
        upper.contains("DEBUGGING MODE")
    })
}

/// Pad a fragment out to the full Area A+B width.
///
/// COBOL-85: when a line ends inside an alphanumeric literal, the spaces from
/// the end of the text through column 72 are part of that literal. A source
/// line that stops short of column 72 must therefore be padded before its
/// continuation is appended, or the literal comes out too short.
fn pad_area(fragment: &str) -> String {
    let n = fragment.chars().count();
    if n >= AREA_WIDTH {
        fragment.to_string()
    } else {
        let mut s = String::with_capacity(AREA_WIDTH);
        s.push_str(fragment);
        s.extend(std::iter::repeat(' ').take(AREA_WIDTH - n));
        s
    }
}

/// Flatten source written in the **classic COBOL-85 reference format**.
///
/// Applies every column rule (see [`SourceFormat::FixedStrict`]) and joins
/// continuation lines.
///
/// One output line is emitted per input line so spans keep pointing at the
/// physical line the developer wrote. A continuation line's text is appended to
/// the line it continues and its own slot is left blank; a diagnostic inside a
/// continued literal therefore reports the line the literal *started* on, which
/// is the line worth looking at.
pub fn flatten_fixed_strict(source: &str) -> String {
    // A `D` line is a comment unless the program asked for debugging mode.
    let debugging = requests_debugging_mode(source);
    let mut out: Vec<String> = Vec::new();
    // Index in `out` of the line a continuation should be appended to.
    let mut last_content: Option<usize> = None;
    // Delimiter of a literal left open by the last content line, if any.
    let mut open_lit: Option<char> = None;

    for raw_line in source.lines() {
        // Columns 73-80 are the identification area and never reach the parser.
        let clipped = &raw_line[..char_boundary_at_col(raw_line, 72)];
        let char_count = clipped.chars().count();

        // Nothing beyond the sequence area — an empty line.
        if char_count <= 6 {
            out.push(String::new());
            continue;
        }

        let indicator = clipped.chars().nth(6).unwrap_or(' ');
        let area = &clipped[char_boundary_at_col(clipped, 7)..];

        match indicator {
            // Comment lines carry no source and cannot close a literal.
            '*' | '/' => out.push(String::new()),

            // A debugging line is a comment unless the program requested
            // debugging mode with `SOURCE-COMPUTER. … WITH DEBUGGING MODE.`
            // — the standard's default, and the reason a `D` line must not
            // simply be compiled.
            'D' | 'd' if !debugging => out.push(String::new()),
            'D' | 'd' => {
                // Debugging mode is on: the line is ordinary source.
                out.push(area.to_string());
                last_content = Some(out.len() - 1);
                open_lit = literal_state(area, None);
            }

            '-' => {
                let target = match last_content {
                    Some(i) => i,
                    None => {
                        // A continuation with nothing to continue: keep the text
                        // rather than dropping it, and let the parser complain.
                        let mut s = " ".repeat(7);
                        s.push_str(area.trim_start());
                        out.push(s);
                        last_content = Some(out.len() - 1);
                        open_lit = literal_state(area, None);
                        continue;
                    }
                };

                if let Some(delim) = open_lit {
                    // Continuing an alphanumeric literal: the continuation line
                    // must reopen with the delimiter, and the literal resumes at
                    // the character after it.
                    let padded = pad_area(area);
                    let trimmed = padded.trim_start();
                    let lead = padded.chars().count() - trimmed.chars().count();
                    let rest = match trimmed.strip_prefix(delim) {
                        Some(r) => r,
                        // Tolerated: some sources omit the reopening quote.
                        None => trimmed,
                    };
                    // The fragment still runs to column 72, so keep exactly the
                    // spaces that survived the clip.
                    let consumed = lead + (trimmed.chars().count() - rest.chars().count());
                    let rest = if consumed >= AREA_WIDTH { "" } else { rest };
                    out[target].push_str(rest);
                    open_lit = literal_state(rest, Some(delim));
                } else {
                    // Continuing a word or a numeric literal: the continued
                    // line's trailing spaces are discarded and the two halves
                    // meet with nothing between them.
                    let joined = out[target].trim_end().to_string();
                    out[target] = joined;
                    let rest = area.trim_start();
                    out[target].push_str(rest);
                    open_lit = literal_state(rest, None);
                }
                out.push(String::new());
            }

            // A blank indicator is an ordinary source line. Anything else is
            // not a COBOL-85 indicator at all; rather than silently dropping the
            // line (or failing the whole compilation) it is read as ordinary
            // source, which is what lets card-image suites that use column 7 as
            // a selector — CCVS85 marks optional lines with `Y`, `P`, `C`, `S`
            // and others — compile as written.
            _ => {
                let fragment = area;
                open_lit = literal_state(fragment, None);
                let body = if open_lit.is_some() {
                    pad_area(fragment)
                } else {
                    fragment.to_string()
                };
                let mut s = " ".repeat(7);
                s.push_str(&body);
                out.push(s);
                last_content = Some(out.len() - 1);
            }
        }
    }

    let mut text = out.join("\n");
    text.push('\n');
    text
}

pub fn flatten_fixed(source: &str) -> String {
    // Same rule as the strict path — the two used to disagree about `D`.
    let debugging = requests_debugging_mode(source);
    let mut out = String::with_capacity(source.len());
    for raw_line in source.lines() {
        // Work in char-columns so multi-byte characters (e.g. '─') are handled safely.
        let char_count: usize = raw_line.chars().count();
        if char_count < 7 {
            out.push_str(&" ".repeat(char_count));
        } else {
            let indicator = raw_line.chars().nth(6).unwrap_or(' ');
            // Byte offsets for safe slicing
            let col7_byte = char_boundary_at_col(raw_line, 7);
            let col6_byte = char_boundary_at_col(raw_line, 6);
            // **The 72-column limit is not enforced** (operator, 2026-08-05).
            //
            // Classic fixed format reserves columns 73-80 for the identification
            // area — card-deck sequence numbers — and discards them. Nobody
            // punches cards, nobody puts sequence numbers there, and truncating
            // at 72 silently destroys code that runs past it.
            //
            // It destroyed more than long COBOL statements: every generated form
            // `.cbl` opens with a banner whose `*` sits in column 7, so the whole
            // file was classified fixed, and any `EXEC RUST` block in a form had
            // each line chopped mid-token — `eframe::Fram`, `{ clicke`. `rustc`
            // then reported a mismatched delimiter in code the developer never
            // wrote. Embedded Rust has no column rules at all.
            //
            // The sequence area (columns 1-6) and the indicator column are still
            // honoured, because those carry meaning. The line simply runs as far
            // as the developer typed.
            let line_end = raw_line.len();

            if matches!(indicator, '*' | '/') {
                out.push_str(&" ".repeat(6));
                out.push(' ');
                if char_count > 7 {
                    out.push_str("*> ");
                    out.push_str(&raw_line[col7_byte..line_end]);
                }
            } else if matches!(indicator, 'D' | 'd') && !debugging {
                // A debugging line is a comment unless the program asked for
                // debugging mode. This path used to compile it unconditionally
                // while `flatten_fixed_strict` treated it as a comment — the
                // two disagreed about the same text.
                out.push_str(&" ".repeat(6));
                out.push(' ');
                if char_count > 7 {
                    out.push_str("*> ");
                    out.push_str(&raw_line[col7_byte..line_end]);
                }
            } else if matches!(indicator, '-' | 'D' | 'd') {
                out.push_str(&raw_line[..col6_byte]);
                out.push(' ');
                if char_count > 7 {
                    out.push_str(&raw_line[col7_byte..line_end]);
                }
            } else {
                out.push_str(&" ".repeat(6));
                out.push_str(&raw_line[col6_byte..line_end]);
            }
        }
        out.push('\n');
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// **Nothing is cut at column 72** (operator, 2026-08-05: "this is archaic").
    ///
    /// The line below carries content well past column 72. Under the old rule
    /// everything from there on vanished, and the failure surfaced somewhere
    /// else — a half-token, an unbalanced delimiter, a statement that lost its
    /// period.
    #[test]
    fn fixed_format_does_not_truncate_at_column_72() {
        // 7 spaces of indent, then a statement whose tail sits beyond col 72.
        let tail = "END-OF-THE-VERY-LONG-NAME";
        let line = format!("       MOVE {} TO {}.", "X".repeat(60), tail);
        assert!(line.len() > 72, "the fixture must cross column 72");

        let flat = flatten_fixed(&line);
        assert!(
            flat.contains(tail),
            "content past column 72 was dropped:\n{flat}"
        );

        let pre = preprocess(&line, SourceFormat::Fixed);
        assert!(
            pre[0].content.contains(tail),
            "preprocess_fixed dropped it too:\n{}",
            pre[0].content
        );
    }

    // ── Classic COBOL-85 reference format (SourceFormat::FixedStrict) ─────────
    //
    // The relaxed reading above must keep working; these cover the strict one,
    // which is opt-in and *does* stop at column 72.

    /// The identification area (columns 73-80) never reaches the parser.
    ///
    /// Every line of the NIST CCVS85 suite carries a program stamp there — the
    /// reason the suite scored 0/459 before this format existed.
    #[test]
    fn strict_drops_the_identification_area() {
        let line = "000100 IDENTIFICATION DIVISION.                                         NC1014.2";
        assert_eq!(line.chars().count(), 80, "fixture must be a full card image");

        let flat = flatten_fixed_strict(line);
        assert!(
            flat.contains("IDENTIFICATION DIVISION."),
            "the statement was lost:\n{flat}"
        );
        assert!(
            !flat.contains("NC1014"),
            "the identification area leaked into the source:\n{flat}"
        );
    }

    /// A continued alphanumeric literal is reassembled from both fragments.
    ///
    /// This is NC113M's `HYPHEN-LINE`: 24 hyphens on the continued line and 30
    /// on the continuation, declared `PICTURE X(54)`.
    #[test]
    fn strict_joins_a_continued_alphanumeric_literal() {
        let src = concat!(
            "011700     02 FILLER PICTURE IS X(54) VALUE IS \"------------------------NC1134.2\n",
            "011800-    \"------------------------------\".                            NC1134.2\n",
        );
        let flat = flatten_fixed_strict(src);
        let hyphens = "-".repeat(54);
        assert!(
            flat.contains(&format!("\"{hyphens}\"")),
            "expected one 54-hyphen literal, got:\n{flat}"
        );
        assert!(!flat.contains("NC1134"), "stamp leaked:\n{flat}");
    }

    /// The continued fragment runs to column 72, trailing spaces included, even
    /// when the physical line stops short of it.
    #[test]
    fn strict_pads_a_short_continued_line_to_column_72() {
        // "AB" then the line ends at column 40; columns 41-72 are part of the
        // literal, so the result is "AB" + 32 spaces + "CD".
        let mut first = String::from("000100     01 X PIC X(36) VALUE \"AB");
        assert!(first.chars().count() < 72);
        let pad = 72 - first.chars().count();
        first.push('\n');
        let src = format!("{first}000200-    \"CD\".\n");

        let flat = flatten_fixed_strict(&src);
        let expected = format!("\"AB{}CD\"", " ".repeat(pad));
        assert!(
            flat.contains(&expected),
            "continued fragment was not padded to column 72:\n{flat}"
        );
    }

    /// A word split across lines joins with nothing between the halves, and the
    /// continued line's trailing spaces are discarded.
    #[test]
    fn strict_joins_a_continued_word() {
        let src = concat!(
            "004700 01  WRK-DS-18V00-CONTIN\n",
            "004800-    UED PICTURE X.\n",
        );
        let flat = flatten_fixed_strict(&src);
        assert!(
            flat.contains("WRK-DS-18V00-CONTINUED PICTURE X."),
            "word halves did not meet:\n{flat}"
        );
    }

    /// Line numbering survives joining: one output line per input line, so a
    /// span still points at the physical line the developer wrote.
    #[test]
    fn strict_preserves_one_output_line_per_input_line() {
        let src = concat!(
            "000100 IDENTIFICATION DIVISION.\n",
            "000200*a comment\n",
            "000300 PROGRAM-ID. X.\n",
            "000400     01 Y PIC X(4) VALUE \"AB\n",
            "000500-    \"CD\".\n",
        );
        let flat = flatten_fixed_strict(src);
        assert_eq!(
            flat.lines().count(),
            src.lines().count(),
            "line count changed:\n{flat}"
        );
    }

    /// Column 7 indicators: comment and debugging lines carry no source.
    #[test]
    fn strict_treats_comment_and_debug_lines_as_comments() {
        let src = concat!(
            "000100*    MOVE 1 TO SHOULD-NOT-APPEAR.\n",
            "000200/    MOVE 2 TO ALSO-NOT.\n",
            "000300D    MOVE 3 TO DEBUG-ONLY.\n",
            "000400     MOVE 4 TO REAL-ONE.\n",
        );
        let flat = flatten_fixed_strict(src);
        assert!(!flat.contains("SHOULD-NOT-APPEAR"), "{flat}");
        assert!(!flat.contains("ALSO-NOT"), "{flat}");
        assert!(!flat.contains("DEBUG-ONLY"), "{flat}");
        assert!(flat.contains("REAL-ONE"), "{flat}");
    }

    /// A doubled quotation mark is the COBOL escape and does not end the
    /// literal, so the line is *not* mistaken for one that needs continuing.
    #[test]
    fn strict_handles_a_doubled_delimiter() {
        let src = "000100     01 X PIC X(5) VALUE \"A\"\"B\".\n";
        let flat = flatten_fixed_strict(src);
        assert!(flat.contains("\"A\"\"B\""), "{flat}");
        assert_eq!(literal_state("\"A\"\"B\"", None), None);
        assert_eq!(literal_state("\"A\"\"B", None), Some('"'));
    }

    /// `preprocess` and the lexer must agree about the strict format. They
    /// disagree for the relaxed `Fixed` reading — two separate implementations,
    /// one of which joins continuations and one of which does not — and that
    /// divergence must not be inherited by the new variant.
    #[test]
    fn strict_preprocess_agrees_with_the_lexer() {
        let src = concat!(
            "000100 IDENTIFICATION DIVISION.                                         NC1134.2\n",
            "000200*    a comment                                                    NC1134.2\n",
            "000300     01 X PIC X(6) VALUE \"AB                                      NC1134.2\n",
            "000400-    \"CD\".                                                        NC1134.2\n",
        );
        let flat = flatten_fixed_strict(src);
        let pre = preprocess(src, SourceFormat::FixedStrict);

        assert_eq!(pre.len(), src.lines().count());
        let joined: Vec<&str> = flat.lines().collect();
        for (i, line) in pre.iter().enumerate() {
            assert_eq!(
                line.content, joined[i],
                "preprocess and the lexer disagree on line {}",
                i + 1
            );
        }
        assert!(pre[1].is_comment, "the comment line was not marked");
        assert!(!pre[0].is_comment, "a source line was marked as a comment");
    }

    /// A non-standard indicator letter is read as ordinary source rather than
    /// dropped. CCVS85 uses column 7 as a selector (`Y`, `P`, `C`, `S`, …) to
    /// mark optional lines, and dropping them would silently delete code.
    #[test]
    fn strict_reads_a_selector_indicator_as_source() {
        let src = "032700S    EXIT PROGRAM.                                                NC1014.2\n";
        let flat = flatten_fixed_strict(src);
        assert!(flat.contains("EXIT PROGRAM."), "{flat}");
        assert!(!flat.contains("NC1014"), "{flat}");
    }

    /// A line shorter than the identification area behaves exactly as before —
    /// there is nothing at column 73 to discard, and nothing is padded.
    #[test]
    fn strict_leaves_a_short_line_alone() {
        let src = "000100     MOVE 1 TO WS-X.\n";
        assert!(src.chars().count() < 73);
        let flat = flatten_fixed_strict(src);
        assert!(flat.contains("MOVE 1 TO WS-X."), "{flat}");
        // No padding: the fragment closes no literal, so nothing is appended.
        assert_eq!(flat.lines().count(), 1, "{flat:?}");

        // The property that matters is **column preservation**: the sequence
        // area and indicator are blanked, but everything from column 8 on stays
        // where the developer put it, so a diagnostic's column still points at
        // the right character. Assert that rather than a hand-counted string.
        let src_col = src.find("MOVE").expect("fixture");
        let out_col = flat.find("MOVE").expect("statement survived");
        assert_eq!(
            out_col, src_col,
            "the statement moved column: {src_col} → {out_col}\n{flat:?}"
        );
        assert!(
            flat.lines().next().unwrap()[..out_col]
                .chars()
                .all(|c| c == ' '),
            "the sequence area was not blanked: {flat:?}"
        );
    }

    /// Multi-byte characters near the cut must not split, and must not panic.
    ///
    /// The clip counts **char columns**, not bytes — a `─` is three bytes, so a
    /// byte-based cut would slice it in half and produce invalid UTF-8.
    #[test]
    fn strict_clips_multibyte_characters_on_a_boundary() {
        // Fill columns 8-71 with box-drawing characters so the cut at 72 lands
        // in the middle of the run.
        let mut line = String::from("000100 ");
        line.push_str(&"─".repeat(70));
        line.push_str("TAIL");
        let flat = flatten_fixed_strict(&line);
        // Valid UTF-8 and no panic is the assertion; `String` guarantees the
        // former, so reaching here at all is most of it.
        assert!(!flat.contains("TAIL"), "content past column 72 survived");
        let kept = flat.lines().next().unwrap().chars().count();
        assert!(kept <= 72, "clipped to {kept} columns, expected ≤ 72");
    }

    /// The relaxed `Fixed` reading is untouched by any of this: it still runs
    /// past column 72. This is the 2026-08-05 operator ruling, and the guard
    /// that generated form sources and `EXEC RUST` blocks depend on.
    #[test]
    fn strict_does_not_change_relaxed_fixed() {
        let tail = "END-OF-THE-VERY-LONG-NAME";
        let line = format!("       MOVE {} TO {}.", "X".repeat(60), tail);
        assert!(line.chars().count() > 72);

        assert!(flatten_fixed(&line).contains(tail), "relaxed Fixed regressed");
        assert!(
            !flatten_fixed_strict(&line).contains(tail),
            "strict is supposed to stop at column 72"
        );
    }

    /// The case that made this urgent: verbatim Rust inside a form's `.cbl`.
    /// A generated form file always begins with a banner whose `*` is in column
    /// 7, so the whole file reads as fixed — and the embedded Rust, which has no
    /// column rules, was being chopped mid-token.
    #[test]
    fn embedded_rust_survives_fixed_format() {
        // The real indentation from a form's generated `.cbl`.
        let line = "                               ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);";
        assert!(line.len() > 72, "the fixture must cross column 72");
        let flat = flatten_fixed(line);
        assert!(
            flat.contains("ViewportCommand::Close);"),
            "the Rust statement lost its tail:\n{flat}"
        );
    }

    #[test]
    fn detect_fixed_default() {
        let src = "000100 IDENTIFICATION DIVISION.\n000200 PROGRAM-ID. HELLO.\n";
        assert_eq!(SourceFormat::detect(src), SourceFormat::Fixed);
    }

    #[test]
    fn detect_free() {
        let src = "*> This is a free-form comment\nIDENTIFICATION DIVISION.\n";
        assert_eq!(SourceFormat::detect(src), SourceFormat::Free);
    }

    #[test]
    fn fixed_comment_line() {
        let src = "000100* This is a comment\n000200 MOVE A TO B.\n";
        let lines = preprocess(src, SourceFormat::Fixed);
        assert!(lines[0].is_comment);
        assert_eq!(lines[0].comment_text.as_deref(), Some("This is a comment"));
        assert!(!lines[1].is_comment);
    }

    #[test]
    fn fixed_active_area() {
        let src = "000100 MOVE WS-A TO WS-B.                                              \n";
        let lines = preprocess(src, SourceFormat::Fixed);
        assert!(!lines[0].is_comment);
        assert!(lines[0].content.contains("MOVE"));
    }

    #[test]
    fn free_comment_stripped() {
        let (active, comment) = strip_free_comment("MOVE A TO B. *> assign");
        assert_eq!(active, "MOVE A TO B. ");
        assert_eq!(comment, Some("assign".to_string()));
    }

    #[test]
    fn free_comment_in_string_not_stripped() {
        let (active, comment) = strip_free_comment(r#"MOVE "*> not a comment" TO B."#);
        assert!(comment.is_none());
        assert!(active.contains("*>"));
    }
}
