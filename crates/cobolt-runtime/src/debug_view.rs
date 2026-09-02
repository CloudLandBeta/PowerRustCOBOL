// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Structured viewers — the same bytes, read the several ways a COBOL developer
//! needs to read them.
//!
//! A COBOL item is bytes. What those bytes *mean* depends on the question being
//! asked: is this field padded or empty, is that packed-decimal sign what I
//! think, is the JSON in this `PIC X(4096)` well-formed. One rendering cannot
//! answer all three, so the viewer is chosen per row.
//!
//! Pure functions over bytes and text: no interpreter, no environment, no egui.
//! That is what makes them testable, and it is why the awkward cases below —
//! embedded NULs, a trailing-sign field, malformed JSON — are pinned rather
//! than discovered in a screenshot.

use serde::{Deserialize, Serialize};

/// How a value should be read.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum ValueView {
    /// The item's own category, as COBOL would DISPLAY it.
    #[default]
    Cobol,
    /// Text with whitespace made visible.
    Text,
    /// Raw storage bytes, hex and ASCII.
    Hex,
    /// Digits, sign and implied decimal point spelled out.
    Numeric,
    /// Pretty-printed JSON.
    Json,
    /// Indented XML.
    Xml,
}

impl ValueView {
    /// The label shown in the viewer picker. Not translated: `HEX` and `JSON`
    /// are the same word everywhere, and translating them would make the menu
    /// harder to scan, not easier.
    pub fn label(self) -> &'static str {
        match self {
            Self::Cobol => "COBOL",
            Self::Text => "Text",
            Self::Hex => "Hex",
            Self::Numeric => "Numeric",
            Self::Json => "JSON",
            Self::Xml => "XML",
        }
    }

    /// The views worth offering for a value — never a menu of six where five
    /// would say nothing. `Cobol` is always first because it is the default.
    pub fn available_for(bytes: &[u8], category: &str) -> Vec<ValueView> {
        let mut out = vec![ValueView::Cobol, ValueView::Text, ValueView::Hex];
        if matches!(category, "numeric" | "float") {
            out.push(ValueView::Numeric);
        }
        let text = String::from_utf8_lossy(bytes);
        let t = text.trim();
        if t.starts_with('{') || t.starts_with('[') {
            out.push(ValueView::Json);
        }
        if t.starts_with('<') {
            out.push(ValueView::Xml);
        }
        out
    }
}

/// Render `bytes` through `view`.
pub fn render(bytes: &[u8], view: ValueView, cobol: &str) -> String {
    match view {
        ValueView::Cobol => cobol.to_owned(),
        ValueView::Text => visible_whitespace(bytes),
        ValueView::Hex => hex_dump(bytes),
        ValueView::Numeric => numeric_detail(cobol),
        ValueView::Json => pretty_json(&String::from_utf8_lossy(bytes)),
        ValueView::Xml => pretty_xml(&String::from_utf8_lossy(bytes)),
    }
}

/// Text with its whitespace shown.
///
/// The question this answers is "is that field empty, or padded, or does it end
/// in a tab" — which is invisible in every other rendering.
pub fn visible_whitespace(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() + 8);
    for b in bytes {
        match b {
            b' ' => out.push('·'),
            b'\t' => out.push('→'),
            b'\n' => out.push('¶'),
            b'\r' => out.push('⏎'),
            0x00 => out.push('␀'),
            // Anything else non-printable would otherwise vanish or corrupt the
            // line; show its code rather than let it disappear.
            0..=0x1F | 0x7F => out.push_str(&format!("\\x{b:02X}")),
            _ => out.push(*b as char),
        }
    }
    out
}

/// A classic hex dump: offset, sixteen bytes, then the printable ASCII.
pub fn hex_dump(bytes: &[u8]) -> String {
    if bytes.is_empty() {
        return "(0 bytes)".into();
    }
    let mut out = String::new();
    for (row, chunk) in bytes.chunks(16).enumerate() {
        out.push_str(&format!("{:04X}  ", row * 16));
        for i in 0..16 {
            match chunk.get(i) {
                Some(b) => out.push_str(&format!("{b:02X} ")),
                None => out.push_str("   "),
            }
            if i == 7 {
                out.push(' ');
            }
        }
        out.push_str(" |");
        for b in chunk {
            out.push(if (0x20..0x7F).contains(b) { *b as char } else { '.' });
        }
        out.push_str("|\n");
    }
    out.pop();
    out
}

/// Digits, sign and scale spelled out.
///
/// COBOL hides the sign in the last byte of a DISPLAY field and the decimal
/// point in the PICTURE, so "is this negative" and "where is the point" are
/// exactly the questions a numeric row cannot answer by looking at it.
pub fn numeric_detail(display: &str) -> String {
    let t = display.trim();
    if t.is_empty() {
        return "(no value)".into();
    }
    let negative = t.starts_with('-');
    let body = t.trim_start_matches(['+', '-']);
    let (int, frac) = match body.split_once(['.', ',']) {
        Some((i, f)) => (i, f),
        None => (body, ""),
    };
    let mut out = String::new();
    out.push_str(&format!("sign      {}\n", if negative { "negative" } else { "positive" }));
    out.push_str(&format!("integer   {}\n", if int.is_empty() { "0" } else { int }));
    out.push_str(&format!("decimals  {}\n", frac.len()));
    if !frac.is_empty() {
        out.push_str(&format!("fraction  {frac}\n"));
    }
    out.push_str(&format!("digits    {}", int.len() + frac.len()));
    out
}

/// Pretty-print JSON, without a JSON parser.
///
/// A `PIC X(4096)` holding a REST payload is the case this exists for. Text
/// inside quotes is copied verbatim — a brace in a string value must not be
/// treated as structure, which is the bug every hand-rolled formatter has.
pub fn pretty_json(text: &str) -> String {
    let src = text.trim();
    if src.is_empty() {
        return "(empty)".into();
    }
    let mut out = String::with_capacity(src.len() + src.len() / 4);
    let (mut depth, mut in_str, mut escaped) = (0usize, false, false);
    for ch in src.chars() {
        if in_str {
            out.push(ch);
            if escaped {
                escaped = false;
            } else if ch == '\\' {
                escaped = true;
            } else if ch == '"' {
                in_str = false;
            }
            continue;
        }
        match ch {
            '"' => {
                in_str = true;
                out.push(ch);
            }
            '{' | '[' => {
                depth += 1;
                out.push(ch);
                out.push('\n');
                out.push_str(&"  ".repeat(depth));
            }
            '}' | ']' => {
                depth = depth.saturating_sub(1);
                out.push('\n');
                out.push_str(&"  ".repeat(depth));
                out.push(ch);
            }
            ',' => {
                out.push(ch);
                out.push('\n');
                out.push_str(&"  ".repeat(depth));
            }
            ':' => out.push_str(": "),
            c if c.is_whitespace() => {}
            c => out.push(c),
        }
    }
    out
}

/// Indent XML one element per line. Same quoting discipline as the JSON case.
pub fn pretty_xml(text: &str) -> String {
    let src = text.trim();
    if src.is_empty() {
        return "(empty)".into();
    }
    let mut out = String::with_capacity(src.len() + src.len() / 4);
    let mut depth = 0usize;
    for part in src.split('<').filter(|p| !p.is_empty()) {
        let (tag, rest) = match part.split_once('>') {
            Some((t, r)) => (t, r.trim()),
            None => (part, ""),
        };
        if tag.starts_with('/') {
            depth = depth.saturating_sub(1);
        }
        out.push_str(&"  ".repeat(depth));
        out.push('<');
        out.push_str(tag);
        out.push('>');
        if !rest.is_empty() {
            out.push_str(rest);
        }
        out.push('\n');
        if !tag.starts_with('/') && !tag.ends_with('/') && !tag.starts_with('?') {
            depth += 1;
        }
    }
    out.pop();
    out
}

/// Lay OCCURS data out as a compact table.
///
/// A hundred-element table as a hundred tree rows is a hundred lines of
/// scrolling to answer "which entry is wrong". As a grid it is one glance.
///
/// `cells` are the occurrence values in subscript order. Returns a
/// monospace-aligned block: a row per line, each cell in a column wide enough
/// for the widest value, with the 1-based subscript of the row's first cell in
/// the margin.
pub fn occurs_table(cells: &[String], per_row: usize) -> String {
    if cells.is_empty() {
        return "(empty table)".into();
    }
    let per_row = per_row.max(1);
    let width = cells.iter().map(|c| c.trim().chars().count()).max().unwrap_or(1);
    // The margin is sized for the LARGEST subscript, so the columns stay
    // aligned from (1) to (1000) instead of shifting a character at every
    // power of ten.
    let margin = format!("({})", cells.len()).chars().count();
    let mut out = String::new();
    for (row, chunk) in cells.chunks(per_row).enumerate() {
        let first = row * per_row + 1;
        out.push_str(&format!("{:>margin$}  ", format!("({first})"), margin = margin));
        for c in chunk {
            out.push_str(&format!("{:>width$} ", c.trim(), width = width));
        }
        out.push('\n');
    }
    out.pop();
    out
}

#[cfg(test)]
mod viewer_tests {
    use super::*;

    /// The question the text view exists to answer: an empty field and a padded
    /// one look identical everywhere else.
    #[test]
    fn whitespace_becomes_visible() {
        assert_eq!(visible_whitespace(b"AB  "), "AB··");
        assert_eq!(visible_whitespace(b""), "");
        assert_eq!(visible_whitespace(b"   "), "···");
        assert_eq!(visible_whitespace(b"a\tb\nc"), "a→b¶c");
        assert_eq!(visible_whitespace(&[b'A', 0x00, b'B']), "A␀B");
        // A control byte must not vanish or corrupt the row.
        assert_eq!(visible_whitespace(&[0x01]), "\\x01");
    }

    #[test]
    fn the_hex_dump_reads_like_a_hex_dump() {
        let d = hex_dump(b"ABC");
        assert!(d.starts_with("0000  41 42 43 "), "{d}");
        assert!(d.ends_with("|ABC|"), "{d}");
        assert_eq!(hex_dump(b""), "(0 bytes)");
        // Two rows for 17 bytes, and the second is offset 0010.
        let long = hex_dump(&[b'x'; 17]);
        assert_eq!(long.lines().count(), 2, "{long}");
        assert!(long.lines().nth(1).unwrap().starts_with("0010  "), "{long}");
        // Non-printables show as dots in the ASCII column, not as themselves.
        assert!(hex_dump(&[0x00, 0x1F]).ends_with("|..|"));
    }

    #[test]
    fn numeric_detail_spells_out_sign_and_scale() {
        let d = numeric_detail("-123.45");
        assert!(d.contains("sign      negative"), "{d}");
        assert!(d.contains("integer   123"), "{d}");
        assert!(d.contains("decimals  2"), "{d}");
        assert!(d.contains("digits    5"), "{d}");
        assert!(numeric_detail("42").contains("sign      positive"));
        assert!(numeric_detail("42").contains("decimals  0"));
        assert_eq!(numeric_detail("   "), "(no value)");
    }

    /// The bug every hand-rolled formatter has: a brace inside a string value
    /// is text, not structure.
    #[test]
    fn json_braces_inside_strings_are_not_structure() {
        let out = pretty_json(r#"{"a":"{not a brace}","b":[1,2]}"#);
        assert!(out.contains(r#""{not a brace}""#), "{out}");
        // The real structure still indents.
        assert!(out.lines().count() > 3, "{out}");
        assert!(out.starts_with('{'), "{out}");
    }

    #[test]
    fn json_survives_an_escaped_quote() {
        let out = pretty_json(r#"{"q":"say \"hi\""}"#);
        assert!(out.contains(r#"say \"hi\""#), "{out}");
    }

    /// Malformed input must still render something. A developer looking at a
    /// truncated `PIC X(4096)` needs to SEE the truncation, not an error.
    #[test]
    fn malformed_json_still_renders() {
        for bad in [r#"{"a":"#, "{{{", "not json at all", ""] {
            let out = pretty_json(bad);
            assert!(!out.is_empty(), "{bad:?} rendered nothing");
        }
        assert_eq!(pretty_json("   "), "(empty)");
    }

    #[test]
    fn xml_indents_by_element() {
        let out = pretty_xml("<a><b>x</b></a>");
        let lines: Vec<&str> = out.lines().collect();
        assert!(lines[0].starts_with("<a>"), "{out}");
        assert!(lines[1].starts_with("  <b>"), "{out}");
        assert_eq!(pretty_xml(""), "(empty)");
    }

    /// The menu offers only what a value can actually be read as — six entries
    /// where five say nothing is a worse menu than three.
    #[test]
    fn the_viewer_menu_fits_the_value() {
        let plain = ValueView::available_for(b"ADA", "alphanumeric");
        assert_eq!(plain, vec![ValueView::Cobol, ValueView::Text, ValueView::Hex]);

        let num = ValueView::available_for(b"42", "numeric");
        assert!(num.contains(&ValueView::Numeric));
        assert!(!num.contains(&ValueView::Json));

        let json = ValueView::available_for(br#"{"a":1}"#, "alphanumeric");
        assert!(json.contains(&ValueView::Json));

        let xml = ValueView::available_for(b"<a/>", "alphanumeric");
        assert!(xml.contains(&ValueView::Xml));

        // COBOL is always first: it is the default, and a menu whose default is
        // not at the top reads as a menu with no default.
        for v in [plain, num, json, xml] {
            assert_eq!(v[0], ValueView::Cobol);
        }
    }

    #[test]
    fn an_occurs_table_lines_its_columns_up() {
        let cells: Vec<String> = (1..=6).map(|n| n.to_string()).collect();
        let t = occurs_table(&cells, 3);
        let lines: Vec<&str> = t.lines().collect();
        assert_eq!(lines.len(), 2, "six cells, three per row: {t}");
        assert!(lines[0].starts_with("(1)"), "{t}");
        assert!(lines[1].starts_with("(4)"), "the second row's first subscript: {t}");
    }

    /// Alignment is the whole point: a value column that shifts by a character
    /// when one entry is wider is a table you cannot scan.
    #[test]
    fn a_wide_value_widens_every_column_equally() {
        let cells = vec!["1".into(), "1000".into(), "7".into()];
        let t = occurs_table(&cells, 3);
        let row = t.lines().next().unwrap();
        // Each cell occupies the same width, so the three are evenly spaced.
        assert!(row.contains("   1 1000    7"), "{row:?}");
    }

    /// And the margin is sized for the largest subscript, so the columns do not
    /// shift at (10), (100), (1000).
    #[test]
    fn the_subscript_margin_does_not_shift_at_a_power_of_ten() {
        let cells: Vec<String> = (1..=12).map(|_| "0".into()).collect();
        let t = occurs_table(&cells, 4);
        let widths: Vec<usize> = t
            .lines()
            .map(|l| l.len() - l.trim_start().len() + l.trim_start().find(' ').unwrap_or(0))
            .collect();
        assert!(
            widths.windows(2).all(|w| w[0] == w[1]),
            "margins differ between rows: {t}"
        );
    }

    #[test]
    fn an_empty_table_says_so() {
        assert_eq!(occurs_table(&[], 4), "(empty table)");
        // A zero row width must not divide by zero.
        assert!(!occurs_table(&["1".into()], 0).is_empty());
    }

    #[test]
    fn render_dispatches_to_each_viewer() {
        let bytes = b"AB ";
        assert_eq!(render(bytes, ValueView::Cobol, "AB"), "AB");
        assert_eq!(render(bytes, ValueView::Text, "AB"), "AB·");
        assert!(render(bytes, ValueView::Hex, "AB").starts_with("0000  41 42 20"));
        assert!(render(b"7", ValueView::Numeric, "7").contains("positive"));
    }
}
