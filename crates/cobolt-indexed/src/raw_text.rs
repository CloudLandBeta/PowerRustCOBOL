// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! COBOL-like text serialization for record structures.

use crate::structure::{depth_from_level, flatten_record, rebuild_record, FlatEntry};
use crate::{FieldUsage, IndexedDefinition, IndexedField};

const INDENT: &str = "    ";

/// Serialize record structure as editable COBOL-like text.
pub fn record_to_text(def: &IndexedDefinition) -> String {
    let mut lines = Vec::new();
    for entry in flatten_record(def) {
        let pad = INDENT.repeat(entry.depth);
        let f = &entry.field;
        let mut line = format!("{pad}{:02} {}", f.level, f.name);
        if f.is_leaf() {
            if !f.pic.is_empty() {
                // Strip custom-entry marker (if present) so generated raw text / COBOL source is clean.
                let pic = f.pic.trim_start_matches('\u{200B}');
                line.push_str(&format!(" PIC {}", pic));
            }
            if f.usage != FieldUsage::Display {
                line.push_str(&format!(" USAGE {}", usage_token(f.usage)));
            }
        } else {
            if f.usage != FieldUsage::Display {
                line.push_str(&format!(" USAGE {}", usage_token(f.usage)));
            }
            if let Some(n) = f.occurs {
                line.push_str(&format!(" OCCURS {n} TIMES"));
            }
            if let Some(r) = &f.redefines {
                line.push_str(&format!(" REDEFINES {r}"));
            }
            if f.synchronized {
                line.push_str(" SYNCHRONIZED");
            }
        }
        line.push_str(".");
        if !f.comment.is_empty() {
            line.push_str(&format!(" *> {}", f.comment));
        }
        lines.push(line);
    }
    lines.join("\n")
}

fn usage_token(u: FieldUsage) -> &'static str {
    match u {
        FieldUsage::Display => "DISPLAY",
        FieldUsage::Comp => "COMP",
        FieldUsage::Comp3 => "COMP-3",
        FieldUsage::Comp4 => "COMP-4",
        FieldUsage::Binary => "BINARY",
        FieldUsage::PackedDecimal => "PACKED-DECIMAL",
        FieldUsage::Index => "INDEX",
        FieldUsage::Pointer => "POINTER",
    }
}

/// Parse COBOL-like record text into a definition's field tree.
pub fn text_to_record(def: &mut IndexedDefinition, text: &str) -> Result<(), String> {
    let flat = parse_record_text(text)?;
    def.fields = rebuild_record(&flat)?;
    def.recompute_offsets();
    Ok(())
}

/// Best-effort auto-repair of a raw record descriptor. Inserts an omitted
/// PIC keyword before a bare picture string (`FOO S9(9)V99` → `FOO PIC S9(9)V99`)
/// and appends a missing terminating period. Comment/blank lines are untouched, and
/// it does not invent picture clauses for elementary items that have none (that
/// needs the author's intent) — any such errors still surface from `parse_record_text`.
pub fn fix_record_text(text: &str) -> String {
    text.lines().map(fix_line).collect::<Vec<_>>().join("\n")
}

fn fix_line(raw: &str) -> String {
    let body = raw.trim();
    if body.is_empty() || body.starts_with('*') {
        return raw.to_string();
    }
    // Preserve a trailing `*>` comment; only repair the code portion.
    let (code_part, comment) = match raw.find("*>") {
        Some(p) => (&raw[..p], Some(&raw[p..])),
        None => (raw, None),
    };
    let indent_len = code_part.len() - code_part.trim_start().len();
    let indent = &code_part[..indent_len];
    let code = code_part
        .trim()
        .strip_suffix('.')
        .unwrap_or(code_part.trim());
    let mut parts = code.trim().splitn(2, char::is_whitespace);
    let level = parts.next().unwrap_or("");
    let rest = parts.next().unwrap_or("").trim();
    let fixed_rest = insert_missing_pic(rest);

    let mut line = format!("{indent}{level}");
    if !fixed_rest.is_empty() {
        line.push(' ');
        line.push_str(&fixed_rest);
    }
    line.push('.'); // ensure the terminating period
    if let Some(c) = comment {
        line.push(' ');
        line.push_str(c.trim_end());
    }
    line
}

fn insert_missing_pic(rest: &str) -> String {
    if rest.is_empty() {
        return rest.to_string();
    }
    // Leave alone if a clause keyword is already present.
    let has_keyword = rest.split_whitespace().any(|t| {
        matches!(
            t.to_ascii_uppercase().as_str(),
            "PIC" | "PICTURE" | "USAGE" | "OCCURS" | "REDEFINES"
        )
    });
    if has_keyword {
        return rest.to_string();
    }
    if let Some(pos) = rest.find(char::is_whitespace) {
        let name = &rest[..pos];
        let tail = rest[pos..].trim();
        if looks_like_picture(tail) {
            return format!("{name} PIC {tail}");
        }
    }
    rest.to_string()
}

pub fn parse_record_text(text: &str) -> Result<Vec<FlatEntry>, String> {
    let mut flat = Vec::new();
    for (lineno, raw) in text.lines().enumerate() {
        let line = raw.trim_end();
        if line.trim().is_empty() || line.trim_start().starts_with('*') {
            continue;
        }
        let depth = raw.chars().take_while(|c| *c == ' ').count() / INDENT.len();
        let mut line_no_comment = line;
        let mut comment = String::new();
        if let Some(pos) = line.find("*>") {
            comment = line[pos + 2..].trim().to_string();
            line_no_comment = &line[..pos];
        } else if let Some(pos) = line.find("/*") {
            comment = line[pos + 2..].trim().to_string();
            line_no_comment = &line[..pos];
        } else if let Some(pos) = line.find(" . ") {
            comment = line[pos + 3..].trim().to_string();
            line_no_comment = &line[..pos];
        }
        let line_no_comment = line_no_comment.trim();
        if !line_no_comment.ends_with('.') {
            return Err(err_line(
                lineno,
                "data item description must end with a period '.'",
            ));
        }
        let trimmed = line_no_comment.strip_suffix('.').unwrap().trim();
        let mut parts = trimmed.splitn(2, char::is_whitespace);
        let level_s = parts
            .next()
            .ok_or_else(|| err_line(lineno, "missing level"))?;
        let level: u8 = level_s
            .parse()
            .map_err(|_| err_line(lineno, "invalid level number"))?;
        if !(1..=49).contains(&level) && !matches!(level, 66 | 77 | 88) {
            return Err(err_line(
                lineno,
                "invalid level number (COBOL-85 allows 01-49, 66, 77, 88)",
            ));
        }
        let rest = parts.next().unwrap_or("").trim();
        if rest.is_empty() {
            return Err(err_line(lineno, "missing field name"));
        }
        let (name, clauses) = split_name_clauses(rest);
        // COBOL-85: a data-name is a single word. Embedded whitespace means a clause
        // keyword was omitted — most often the PIC/PICTURE keyword before a picture.
        if name.split_whitespace().count() != 1 {
            let mut toks = name.split_whitespace();
            let first = toks.next().unwrap_or("");
            let tail = toks.collect::<Vec<_>>().join(" ");
            if looks_like_picture(&tail) {
                return Err(err_line(
                    lineno,
                    &format!("'{first}' is missing its PIC/PICTURE clause before picture '{tail}'"),
                ));
            }
            return Err(err_line(lineno, &format!("invalid data-name '{name}'")));
        }
        validate_data_name(name, lineno)?;
        let mut field = IndexedField {
            level,
            name: name.to_ascii_uppercase(),
            pic: String::new(),
            usage: FieldUsage::Display,
            offset: None,
            length: None,
            comment,
            grid_control: None,
            occurs: None,
            redefines: None,
            synchronized: false,
            children: Vec::new(),
        };
        parse_clauses(&mut field, clauses, lineno)?;
        if field.pic.is_empty() && field.occurs.is_none() && field.redefines.is_none() {
            // group
        } else if !field.pic.is_empty() {
            field.offset = Some(0);
            field.length = field.length.or(Some(1));
        }
        flat.push(FlatEntry {
            depth: depth_from_level(level).max(depth),
            field,
        });
    }
    if flat.is_empty() {
        return Err("record structure is empty".into());
    }
    // COBOL-85: every elementary item (one with no subordinate items) must carry a
    // PICTURE clause. Exceptions: USAGE INDEX/POINTER take none, and level 66/88
    // entries are not elementary data. In source order a group's children follow it
    // at a deeper level, so "no deeper item follows" ⇒ this item is elementary.
    for i in 0..flat.len() {
        let f = &flat[i].field;
        let has_children = flat.get(i + 1).is_some_and(|n| n.field.level > f.level);
        let exempt = matches!(f.usage, FieldUsage::Index | FieldUsage::Pointer)
            || matches!(f.level, 66 | 88);
        if !has_children && f.pic.is_empty() && !exempt {
            return Err(format!(
                "'{}': elementary item is missing a PIC/PICTURE clause",
                f.name
            ));
        }
    }
    // Normalise depth from level numbers in source.
    for e in &mut flat {
        e.depth = depth_from_level(e.field.level);
    }
    Ok(flat)
}

fn split_name_clauses(rest: &str) -> (&str, &str) {
    if let Some(pos) = rest.find(" PIC ") {
        return (&rest[..pos], &rest[pos + 1..]);
    }
    if let Some(pos) = rest.find(" USAGE ") {
        return (&rest[..pos], &rest[pos + 1..]);
    }
    if let Some(pos) = rest.find(" OCCURS ") {
        return (&rest[..pos], &rest[pos + 1..]);
    }
    if let Some(pos) = rest.find(" REDEFINES ") {
        return (&rest[..pos], &rest[pos + 1..]);
    }
    if let Some(pos) = rest.find(" SYNCHRONIZED") {
        return (&rest[..pos], &rest[pos + 1..]);
    }
    if let Some(pos) = rest.find('.') {
        let name = rest[..pos].trim();
        return (name, "");
    }
    (rest, "")
}

/// Heuristic: does `s` look like a COBOL PICTURE string with the PIC/PICTURE keyword
/// omitted? True when it is non-empty, carries a data character (9/X/A/S), and every
/// character is a valid picture symbol. Used to give a precise "missing PIC clause"
/// diagnostic when a picture string trails a data-name with no keyword.
/// Reject a data-name that is not COBOL-85 conformant: 1–30 characters, letters,
/// digits and hyphens only, at least one letter, and no leading/trailing hyphen.
fn validate_data_name(name: &str, lineno: usize) -> Result<(), String> {
    if name.is_empty() {
        return Err(err_line(lineno, "missing field name"));
    }
    if name.chars().count() > 30 {
        return Err(err_line(
            lineno,
            &format!("data-name '{name}' exceeds 30 characters"),
        ));
    }
    if name.starts_with('-') || name.ends_with('-') {
        return Err(err_line(
            lineno,
            &format!("data-name '{name}' may not begin or end with a hyphen"),
        ));
    }
    if !name.chars().any(|c| c.is_ascii_alphabetic()) {
        return Err(err_line(
            lineno,
            &format!("data-name '{name}' must contain at least one letter"),
        ));
    }
    if let Some(bad) = name
        .chars()
        .find(|c| !(c.is_ascii_alphanumeric() || *c == '-'))
    {
        return Err(err_line(
            lineno,
            &format!("data-name '{name}' contains invalid character '{bad}'"),
        ));
    }
    Ok(())
}

fn looks_like_picture(s: &str) -> bool {
    let s = s.trim();
    if s.is_empty() {
        return false;
    }
    let mut has_data = false;
    for c in s.chars() {
        match c.to_ascii_uppercase() {
            '9' | 'X' | 'A' | 'S' => has_data = true,
            'V' | 'P' | 'Z' | 'B' | '0' | '/' | ',' | '.' | '+' | '-' | '*' | '$' | 'C' | 'R'
            | 'D' | '(' | ')' | ' ' => {}
            c if c.is_ascii_digit() => {}
            _ => return false,
        }
    }
    has_data
}

fn parse_clauses(field: &mut IndexedField, mut clauses: &str, lineno: usize) -> Result<(), String> {
    if let Some(dot) = clauses.rfind('.') {
        if field.comment.is_empty() {
            field.comment = clauses[dot + 1..].trim().to_string();
        }
        clauses = clauses[..dot].trim();
    }
    if clauses.ends_with('.') {
        clauses = clauses.strip_suffix('.').unwrap().trim();
    }
    let upper = clauses.to_ascii_uppercase();
    if let Some(pic) = extract_pic(&upper) {
        field.pic = pic;
        let p = crate::parse_pic(&field.pic);
        field.length = Some(p.width as u32);
    }
    if upper.contains("USAGE DISPLAY") {
        field.usage = FieldUsage::Display;
    } else if upper.contains("USAGE COMP-3") {
        field.usage = FieldUsage::Comp3;
    } else if upper.contains("USAGE COMP-4") {
        field.usage = FieldUsage::Comp4;
    } else if upper.contains("USAGE COMP") {
        field.usage = FieldUsage::Comp;
    } else if upper.contains("USAGE BINARY") {
        field.usage = FieldUsage::Binary;
    } else if upper.contains("USAGE PACKED-DECIMAL") {
        field.usage = FieldUsage::PackedDecimal;
    } else if upper.contains("USAGE INDEX") {
        field.usage = FieldUsage::Index;
    } else if upper.contains("USAGE POINTER") {
        field.usage = FieldUsage::Pointer;
    }
    if let Some(n) = extract_occurs(&upper) {
        field.occurs = Some(n);
    }
    if let Some(r) = extract_after(&upper, "REDEFINES") {
        field.redefines = Some(r);
    }
    if upper.contains("SYNCHRONIZED") {
        field.synchronized = true;
    }
    if field.name.is_empty() {
        return Err(err_line(lineno, "empty field name"));
    }
    Ok(())
}

fn extract_pic(upper: &str) -> Option<String> {
    let idx = upper.find("PIC ")?;
    let rest = &upper[idx + 4..];
    let end = rest
        .find(" USAGE ")
        .or_else(|| rest.find(" OCCURS "))
        .or_else(|| rest.find(" REDEFINES "))
        .unwrap_or(rest.len());
    let pic = rest[..end].trim();
    if pic.is_empty() {
        None
    } else {
        Some(pic.replace(" ", ""))
    }
}

fn extract_occurs(upper: &str) -> Option<u32> {
    let idx = upper.find("OCCURS ")?;
    let rest = upper[idx + 7..].trim_start();
    let num: String = rest.chars().take_while(|c| c.is_ascii_digit()).collect();
    num.parse().ok()
}

fn extract_after(upper: &str, keyword: &str) -> Option<String> {
    let idx = upper.find(keyword)?;
    let rest = upper[idx + keyword.len()..].trim_start();
    let name: String = rest
        .chars()
        .take_while(|c| c.is_ascii_alphanumeric() || *c == '-')
        .collect();
    if name.is_empty() {
        None
    } else {
        Some(name)
    }
}

fn err_line(line: usize, msg: &str) -> String {
    format!("line {}: {msg}", line + 1)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_assigns_depth_from_level() {
        let flat = parse_record_text("01 ROOT.\n    05 A PIC X(8).").unwrap();
        assert_eq!(flat.len(), 2);
        assert_eq!(flat[0].depth, 0);
        assert_eq!(flat[1].depth, 1);
        assert_eq!(flat[1].field.pic, "X(8)");
    }

    #[test]
    fn round_trip_text() {
        let mut def = IndexedDefinition::new("T", "t.idx");
        def.fields = vec![IndexedField {
            level: 1,
            name: "ROOT".into(),
            pic: String::new(),
            usage: FieldUsage::Display,
            offset: None,
            length: None,
            comment: String::new(),
            grid_control: None,
            occurs: None,
            redefines: None,
            synchronized: false,
            children: vec![IndexedField {
                level: 5,
                name: "A".into(),
                pic: "X(8)".into(),
                usage: FieldUsage::Display,
                offset: Some(0),
                length: Some(8),
                comment: String::new(),
                grid_control: None,
                occurs: None,
                redefines: None,
                synchronized: false,
                children: Vec::new(),
            }],
        }];
        let text = record_to_text(&def);
        let mut def2 = IndexedDefinition::new("T", "t.idx");
        text_to_record(&mut def2, &text).expect(&text);
        assert_eq!(def2.fields[0].children.len(), 1, "from:\n{text}");
        assert_eq!(def2.fields[0].children[0].pic, "X(8)");
    }

    #[test]
    fn parse_rejects_missing_pic_keyword() {
        // `05 FOOD-PRICE S9(9)V99.` omits the PIC keyword — non-conformant COBOL-85,
        // so it must be rejected with a clear "missing PIC" diagnostic, not accepted.
        let err = parse_record_text(
            "01 MENU-RECORD.\n    05 FOOD-ID PIC X(10).\n    05 FOOD-PRICE S9(9)V99.",
        )
        .unwrap_err();
        assert!(err.contains("PIC/PICTURE"), "got: {err}");
        assert!(err.contains("FOOD-PRICE"), "got: {err}");
    }

    #[test]
    fn parse_rejects_elementary_item_without_pic() {
        let err = parse_record_text("01 REC.\n    05 FOO.").unwrap_err();
        assert!(err.contains("PIC/PICTURE"), "got: {err}");
    }

    #[test]
    fn fix_inserts_missing_pic_and_period() {
        let fixed = fix_record_text(
            "01 MENU-RECORD.\n    05 FOOD-ID PIC X(10).\n    05 FOOD-PRICE S9(9)V99\n",
        );
        assert!(
            fixed.contains("05 FOOD-PRICE PIC S9(9)V99."),
            "got:\n{fixed}"
        );
        // And the repaired text now parses cleanly.
        assert!(parse_record_text(&fixed).is_ok(), "fixed text should parse");
    }

    #[test]
    fn fix_leaves_conformant_lines_untouched() {
        let src = "01 REC.\n    05 NM PIC X(10).";
        assert_eq!(fix_record_text(src), src);
    }

    #[test]
    fn parse_accepts_conformant_numeric_field() {
        let flat =
            parse_record_text("01 REC.\n    05 PRICE PIC S9(9)V99.\n    05 NM PIC X(10).").unwrap();
        assert_eq!(flat[1].field.pic, "S9(9)V99");
        assert_eq!(
            crate::parse_pic(&flat[1].field.pic).category,
            crate::PicCategory::Numeric
        );
    }

    #[test]
    fn parse_fails_on_missing_period() {
        let res = parse_record_text("01 ROOT\n    05 A PIC X(8).");
        assert!(res.is_err());
        assert!(res.unwrap_err().contains("must end with a period"));
    }
}
