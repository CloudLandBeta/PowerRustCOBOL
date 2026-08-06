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
//! - Columns 73–80: Program identification — **not enforced here**
//!
//! ### The 72-column limit is deliberately not enforced
//!
//! Classic fixed format stops the source at column 72 and treats 73–80 as a
//! card-deck sequence area. PowerRustCOBOL reads the line to its end instead.
//! Nobody punches cards; what the limit actually did was delete code — silently,
//! mid-token, with the error surfacing somewhere else entirely. It also made
//! `EXEC RUST` unusable in a form, because every generated `.cbl` carries a
//! banner whose `*` sits in column 7, so the file was read as fixed and the
//! embedded Rust — which has no column rules at all — was chopped at 72.
//!
//! The sequence area (1–6) and the indicator column (7) are still honoured:
//! those carry meaning. Only the right-hand limit is gone.
//!
//! ## Free-form (COBOL 2002+, Fujitsu extension)
//!
//! No column restrictions.  `*>` starts a comment to end of line.
//! Continuation lines use `&` at the end of the continued line.

/// The source format of a COBOL file.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SourceFormat {
    /// Traditional punch-card column layout (default for Fujitsu COBOL).
    #[default]
    Fixed,
    /// Free-form layout (COBOL 2002, Fujitsu free-form option).
    Free,
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
        SourceFormat::Free => preprocess_free(source),
    }
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

pub fn flatten_fixed(source: &str) -> String {
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
            } else if matches!(indicator, '-' | 'D') {
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
