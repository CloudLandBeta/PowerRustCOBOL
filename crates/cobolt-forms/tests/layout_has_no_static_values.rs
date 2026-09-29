// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! **Spec 056 R85 / AC42 — the layout has no static values.**
//!
//! Every number that shapes a responsive layout is a property with a seeded
//! default, and those defaults are written in exactly one place,
//! `src/layout/defaults.rs`. This scans every other file of `src/layout/` —
//! comments, string literals and `#[cfg(test)]` items excluded — and fails on
//! any numeric literal other than 0, 1, 2 and 0.5 (the arithmetic of halves
//! and wholes, not a layout choice).
//!
//! The second test shows the other half of the rule: a value the layout uses
//! comes from a property, so changing the property moves the layout.

use std::collections::BTreeMap;
use std::path::PathBuf;

use cobolt_forms::layout::{solve, LayoutInput};
use cobolt_forms::model::{Control, ControlType, PropValue, Rect};

const ALLOWED: [f64; 4] = [0.0, 1.0, 2.0, 0.5];

/// Source text with comments, strings, char literals and `#[cfg(test)]` items
/// blanked out (newlines kept, so line numbers still point at the source).
fn code_only(src: &str) -> String {
    let b: Vec<char> = src.chars().collect();
    let mut out = String::with_capacity(src.len());
    let mut i = 0;
    let blank = |c: char| if c == '\n' { '\n' } else { ' ' };
    while i < b.len() {
        let c = b[i];
        let next = b.get(i + 1).copied();
        if c == '/' && next == Some('/') {
            while i < b.len() && b[i] != '\n' {
                out.push(' ');
                i += 1;
            }
        } else if c == '/' && next == Some('*') {
            while i < b.len() && !(b[i] == '*' && b.get(i + 1) == Some(&'/')) {
                out.push(blank(b[i]));
                i += 1;
            }
            out.push_str("  ");
            i += 2;
        } else if c == 'r' && (next == Some('"') || next == Some('#')) && (i == 0 || !is_ident(b[i - 1])) {
            // Raw string r#"…"#.
            let mut j = i + 1;
            let mut hashes = 0;
            while b.get(j) == Some(&'#') {
                hashes += 1;
                j += 1;
            }
            if b.get(j) != Some(&'"') {
                out.push(c);
                i += 1;
                continue;
            }
            j += 1;
            loop {
                if j >= b.len() {
                    break;
                }
                if b[j] == '"' && (0..hashes).all(|k| b.get(j + 1 + k) == Some(&'#')) {
                    j += 1 + hashes;
                    break;
                }
                j += 1;
            }
            for k in i..j.min(b.len()) {
                out.push(blank(b[k]));
            }
            i = j;
        } else if c == '"' {
            out.push(' ');
            i += 1;
            while i < b.len() && b[i] != '"' {
                if b[i] == '\\' {
                    out.push(' ');
                    i += 1;
                }
                if i < b.len() {
                    out.push(blank(b[i]));
                    i += 1;
                }
            }
            out.push(' ');
            i += 1;
        } else if c == '\'' && (b.get(i + 2) == Some(&'\'') || (next == Some('\\') && b.get(i + 3) == Some(&'\''))) {
            // A char literal ('x' or '\n'); a lifetime has no closing quote.
            let len = if next == Some('\\') { 4 } else { 3 };
            for _ in 0..len {
                out.push(' ');
            }
            i += len;
        } else {
            out.push(c);
            i += 1;
        }
    }
    strip_cfg_test(&out)
}

fn is_ident(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_'
}

/// Blank every item marked `#[cfg(test)]`: from the attribute to the end of
/// the braced block (or `;`) that follows it.
fn strip_cfg_test(code: &str) -> String {
    let mut b: Vec<char> = code.chars().collect();
    let pat: Vec<char> = "#[cfg(test)]".chars().collect();
    let mut i = 0;
    while i + pat.len() <= b.len() {
        if b[i..i + pat.len()] == pat[..] {
            let mut j = i + pat.len();
            while j < b.len() && b[j] != '{' && b[j] != ';' {
                j += 1;
            }
            if j < b.len() && b[j] == '{' {
                let mut depth = 0;
                while j < b.len() {
                    if b[j] == '{' {
                        depth += 1;
                    } else if b[j] == '}' {
                        depth -= 1;
                        if depth == 0 {
                            break;
                        }
                    }
                    j += 1;
                }
            }
            for k in i..=j.min(b.len() - 1) {
                if b[k] != '\n' {
                    b[k] = ' ';
                }
            }
            i = j + 1;
        } else {
            i += 1;
        }
    }
    b.into_iter().collect()
}

/// Every numeric literal in `code`: (line, text, value). A digit run right
/// after a `.` is a tuple field (`size.0`), not a literal.
fn literals(code: &str) -> Vec<(usize, String, f64)> {
    let b: Vec<char> = code.chars().collect();
    let mut out = Vec::new();
    let mut line = 1;
    let mut i = 0;
    while i < b.len() {
        let c = b[i];
        if c == '\n' {
            line += 1;
            i += 1;
            continue;
        }
        let prev = if i > 0 { b[i - 1] } else { ' ' };
        // `x.0` is a field; the `16` of `0..16` is not.
        let field = prev == '.' && (i < 2 || b[i - 2] != '.');
        if c.is_ascii_digit() && !is_ident(prev) && !field {
            let start = i;
            while i < b.len() {
                let d = b[i];
                // Stop before a range operator `..`.
                if d == '.' && b.get(i + 1) == Some(&'.') {
                    break;
                }
                if d.is_ascii_alphanumeric() || d == '_' || (d == '.' && b.get(i + 1).is_some_and(|n| n.is_ascii_digit())) {
                    i += 1;
                } else {
                    break;
                }
            }
            let text: String = b[start..i].iter().collect();
            let numeric: String = strip_suffix(&text).replace('_', "");
            let value = numeric.parse::<f64>().unwrap_or(f64::NAN);
            out.push((line, text, value));
        } else {
            i += 1;
        }
    }
    out
}

fn strip_suffix(t: &str) -> &str {
    for s in ["f32", "f64", "i8", "i16", "i32", "i64", "isize", "u8", "u16", "u32", "u64", "usize"] {
        if let Some(stripped) = t.strip_suffix(s) {
            return stripped;
        }
    }
    t
}

#[test]
fn no_numeric_literal_lives_outside_the_defaults_table() {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/layout");
    let mut files: Vec<PathBuf> = std::fs::read_dir(&dir)
        .expect("src/layout exists")
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().and_then(|e| e.to_str()) == Some("rs"))
        .collect();
    files.sort();
    let mut scanned = Vec::new();
    let mut total = 0;
    let mut offenders = Vec::new();
    for f in &files {
        let name = f.file_name().unwrap().to_string_lossy().to_string();
        if name == "defaults.rs" {
            continue;
        }
        let src = std::fs::read_to_string(f).unwrap();
        let lits = literals(&code_only(&src));
        total += lits.len();
        for (line, text, v) in &lits {
            if !ALLOWED.iter().any(|a| (a - v).abs() < 1e-12) {
                offenders.push(format!("src/layout/{name}:{line}: `{text}`"));
            }
        }
        scanned.push(format!("{name} ({} literal(s))", lits.len()));
    }
    println!("── 056 R85: no static values in the layout ─────────────");
    println!("  scanned : {}", scanned.join(", "));
    println!("  literals: {total}, all in {{0, 1, 2, 0.5}} unless listed below");
    println!("  outside the allow-list: {}", offenders.len());
    assert!(
        offenders.is_empty(),
        "numeric literals outside src/layout/defaults.rs (make each a property with a seeded default):\n{}",
        offenders.join("\n")
    );
}

/// The scanner itself: it sees what it must and ignores what it must.
#[test]
fn the_scanner_finds_literals_and_skips_comments_strings_fields_and_tests() {
    let src = r##"
fn f(a: (f32, f32)) -> f32 {
    // 64 in a comment
    let s = "600 in a string";
    let r = r#"1024 raw"#;
    let c = '7';
    let t = a.0 + a.1;
    for i in 0..16 {}
    t * 0.5 + 12.0 + 3f32
}
#[cfg(test)]
mod tests { fn g() -> i32 { 99 } }
"##;
    let lits: Vec<String> = literals(&code_only(src)).into_iter().map(|(_, t, _)| t).collect();
    assert_eq!(lits, vec!["0", "16", "0.5", "12.0", "3f32"]);
}

/// A value the layout uses is a property: raising the form's `Padding` (whose
/// seeded default is 0) moves a Top,Left control by exactly that much.
#[test]
fn changing_a_seeded_default_moves_the_layout() {
    let mut c = Control::new("B", ControlType::Button, 0, 0);
    c.rect = Rect::new(10, 10, 80, 30);
    c.set_prop("Dock", PropValue::String("Top".into()));
    let controls = [c];
    let at = |bag: &BTreeMap<String, PropValue>| {
        let o = solve(&LayoutInput::new(&controls, (400.0, 300.0), (400.0, 300.0), bag, &[]));
        let r = o.rects["B"];
        (r.x, r.y, r.w)
    };
    let seeded = at(&BTreeMap::new());
    let padded = at(&BTreeMap::from([("Padding".to_owned(), PropValue::Int(12))]));
    assert_eq!(seeded, (0.0, 0.0, 400.0));
    assert_eq!(padded, (12.0, 12.0, 376.0));
    println!("form Padding 0 → docked Top at {seeded:?}; Padding 12 → {padded:?}");
}
