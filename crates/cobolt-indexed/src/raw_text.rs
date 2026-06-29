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
        if !f.comment.is_empty() {
            line.push_str(&format!(" . {}", f.comment));
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
    Ok(())
}

pub fn parse_record_text(text: &str) -> Result<Vec<FlatEntry>, String> {
    let mut flat = Vec::new();
    for (lineno, raw) in text.lines().enumerate() {
        let line = raw.trim_end();
        if line.trim().is_empty() || line.trim_start().starts_with('*') {
            continue;
        }
        let depth = raw.chars().take_while(|c| *c == ' ').count() / INDENT.len();
        let trimmed = line.split("/*").next().unwrap_or("").trim();
        let trimmed = trimmed.trim_start();
        let mut parts = trimmed.splitn(2, char::is_whitespace);
        let level_s = parts
            .next()
            .ok_or_else(|| err_line(lineno, "missing level"))?;
        let level: u8 = level_s
            .parse()
            .map_err(|_| err_line(lineno, "invalid level number"))?;
        let rest = parts.next().unwrap_or("").trim();
        if rest.is_empty() {
            return Err(err_line(lineno, "missing field name"));
        }
        let (name, clauses) = split_name_clauses(rest);
        let mut field = IndexedField {
            level,
            name: name.to_ascii_uppercase(),
            pic: String::new(),
            usage: FieldUsage::Display,
            offset: None,
            length: None,
            comment: String::new(),
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

fn parse_clauses(field: &mut IndexedField, mut clauses: &str, lineno: usize) -> Result<(), String> {
    if let Some(dot) = clauses.rfind('.') {
        field.comment = clauses[dot + 1..].trim().to_string();
        clauses = clauses[..dot].trim();
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
        let flat = parse_record_text("01 ROOT\n    05 A PIC X(8)").unwrap();
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
}
