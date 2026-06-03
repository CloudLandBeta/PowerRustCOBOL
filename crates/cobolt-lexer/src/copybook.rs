// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! COPY / REPLACE source-text manipulation (the COBOL preprocessor).
//!
//! Runs before tokenization. `COPY name [OF lib] [REPLACING a BY b …].` splices
//! a copybook file in at the COPY point, applying any REPLACING substitutions.
//! `REPLACE a BY b … .` / `REPLACE OFF.` rewrite the following source text.
//!
//! Copybook text and the main source are flattened to free form first, so the
//! result is always free-form text that the lexer consumes with
//! [`SourceFormat::Free`](crate::SourceFormat).

use std::path::{Path, PathBuf};

use crate::source::{flatten_fixed, SourceFormat};

/// Result of preprocessing: the expanded free-form source plus any errors
/// (missing copybook, cyclic COPY, malformed directive).
#[derive(Debug, Clone, Default)]
pub struct CopyExpansion {
    pub text: String,
    pub errors: Vec<String>,
}

/// Expand all `COPY` / `REPLACE` directives in `source`, resolving copybooks
/// relative to `base_dir`. `format` is the source format of the program (and of
/// the copybooks); fixed-form text is flattened to free form.
pub fn expand_copybooks(source: &str, base_dir: &Path, format: SourceFormat) -> CopyExpansion {
    let mut errors = Vec::new();
    let mut stack: Vec<PathBuf> = Vec::new();
    let flat = flatten(source, format);
    let text = expand_text(&flat, base_dir, format, &mut errors, &mut stack, 0);
    CopyExpansion { text, errors }
}

fn flatten(source: &str, format: SourceFormat) -> String {
    match format {
        SourceFormat::Fixed => flatten_fixed(source),
        _ => source.to_string(),
    }
}

const MAX_DEPTH: usize = 50;

// ── Preprocessor token scan ─────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
enum PKind {
    Word,
    Str,
    Pseudo,
    Dot,
}

#[derive(Debug, Clone)]
struct PTok {
    kind: PKind,
    /// Significant content: word text, string contents, or pseudo-text inner.
    text: String,
    start: usize,
    end: usize,
}

/// Scan only the lexemes the preprocessor cares about (words, string literals,
/// `== … ==` pseudo-text, and `.`); everything else is left in the gaps between
/// tokens and copied verbatim.
fn scan(s: &str) -> Vec<PTok> {
    let b = s.as_bytes();
    let mut toks = Vec::new();
    let mut i = 0usize;
    let is_word = |c: u8| c.is_ascii_alphanumeric() || c == b'-' || c == b'_';
    while i < b.len() {
        let c = b[i];
        if c == b'*' && i + 1 < b.len() && b[i + 1] == b'>' {
            // Free-form comment `*>` (fixed-form column-7 comments are flattened
            // to this): skip to end of line so words inside comments — e.g. the
            // word COPY — are never mistaken for directives.
            while i < b.len() && b[i] != b'\n' {
                i += 1;
            }
        } else if c == b'=' && i + 1 < b.len() && b[i + 1] == b'=' {
            // pseudo-text  == ... ==
            let start = i;
            i += 2;
            let inner_start = i;
            while i + 1 < b.len() && !(b[i] == b'=' && b[i + 1] == b'=') {
                i += 1;
            }
            let inner = s[inner_start..i].trim().to_string();
            if i + 1 < b.len() {
                i += 2;
            } else {
                i = b.len();
            }
            toks.push(PTok { kind: PKind::Pseudo, text: inner, start, end: i });
        } else if c == b'"' || c == b'\'' {
            let quote = c;
            let start = i;
            i += 1;
            while i < b.len() && b[i] != quote {
                i += 1;
            }
            let inner = s[start + 1..i.min(b.len())].to_string();
            if i < b.len() {
                i += 1;
            }
            toks.push(PTok { kind: PKind::Str, text: inner, start, end: i });
        } else if c == b'.' {
            toks.push(PTok { kind: PKind::Dot, text: ".".into(), start: i, end: i + 1 });
            i += 1;
        } else if is_word(c) {
            let start = i;
            while i < b.len() && is_word(b[i]) {
                i += 1;
            }
            toks.push(PTok { kind: PKind::Word, text: s[start..i].to_string(), start, end: i });
        } else {
            i += 1;
        }
    }
    toks
}

fn eqi(a: &str, b: &str) -> bool {
    a.eq_ignore_ascii_case(b)
}

// ── Expansion ─────────────────────────────────────────────────────────────

#[allow(clippy::too_many_arguments)]
fn expand_text(
    text: &str,
    base_dir: &Path,
    format: SourceFormat,
    errors: &mut Vec<String>,
    stack: &mut Vec<PathBuf>,
    depth: usize,
) -> String {
    let toks = scan(text);
    let mut out = String::new();
    let mut prev_end = 0usize; // byte offset in `text` up to which we've emitted
    let mut active: Vec<(String, String)> = Vec::new(); // REPLACE pairs
    let mut i = 0usize;

    while i < toks.len() {
        let t = &toks[i];
        if t.kind == PKind::Word && eqi(&t.text, "COPY") {
            // Emit the gap before COPY (REPLACE-rewritten).
            out.push_str(&apply_pairs(&text[prev_end..t.start], &active));
            match parse_copy(&toks, i) {
                Some((name, replacing, end_idx, end_byte)) => {
                    let copy = load_and_expand(
                        &name, &replacing, base_dir, format, errors, stack, depth,
                    );
                    out.push_str(&apply_pairs(&copy, &active));
                    out.push('\n');
                    prev_end = end_byte;
                    i = end_idx + 1;
                }
                None => {
                    errors.push(format!("malformed COPY directive near byte {}", t.start));
                    i += 1;
                }
            }
        } else if t.kind == PKind::Word && eqi(&t.text, "REPLACE") {
            out.push_str(&apply_pairs(&text[prev_end..t.start], &active));
            match parse_replace(&toks, i) {
                Some((pairs, end_idx, end_byte)) => {
                    active = pairs; // REPLACE … replaces the active set; OFF clears it
                    prev_end = end_byte;
                    i = end_idx + 1;
                }
                None => {
                    errors.push(format!("malformed REPLACE directive near byte {}", t.start));
                    i += 1;
                }
            }
        } else {
            i += 1;
        }
    }
    out.push_str(&apply_pairs(&text[prev_end..], &active));
    out
}

/// Parse a `COPY name [OF/IN lib] [REPLACING op BY op …] .` directive.
/// `i` indexes the `COPY` word. Returns (name, replacing-pairs, last-tok-index,
/// byte offset just past the terminating `.`).
fn parse_copy(toks: &[PTok], i: usize) -> Option<(String, Vec<(String, String)>, usize, usize)> {
    let mut j = i + 1;
    let name_tok = toks.get(j)?;
    if !matches!(name_tok.kind, PKind::Word | PKind::Str) {
        return None;
    }
    let name = name_tok.text.clone();
    j += 1;
    // optional OF/IN library
    if let Some(t) = toks.get(j) {
        if t.kind == PKind::Word && (eqi(&t.text, "OF") || eqi(&t.text, "IN")) {
            j += 2; // skip OF + library word
        }
    }
    let mut replacing = Vec::new();
    if let Some(t) = toks.get(j) {
        if t.kind == PKind::Word && eqi(&t.text, "REPLACING") {
            j += 1;
            while let Some(tk) = toks.get(j) {
                if tk.kind == PKind::Dot {
                    break;
                }
                // operand BY operand
                let from = tk.text.clone();
                j += 1;
                // expect BY
                if let Some(by) = toks.get(j) {
                    if by.kind == PKind::Word && eqi(&by.text, "BY") {
                        j += 1;
                    }
                }
                let to = toks.get(j).map(|t| t.text.clone()).unwrap_or_default();
                j += 1;
                replacing.push((from, to));
            }
        }
    }
    // terminating dot
    let dot = toks.get(j)?;
    if dot.kind != PKind::Dot {
        return None;
    }
    Some((name, replacing, j, dot.end))
}

/// Parse `REPLACE op BY op … .` or `REPLACE OFF.`.
fn parse_replace(toks: &[PTok], i: usize) -> Option<(Vec<(String, String)>, usize, usize)> {
    let mut j = i + 1;
    // REPLACE OFF.
    if let Some(t) = toks.get(j) {
        if t.kind == PKind::Word && eqi(&t.text, "OFF") {
            j += 1;
            let dot = toks.get(j)?;
            if dot.kind != PKind::Dot {
                return None;
            }
            return Some((Vec::new(), j, dot.end));
        }
    }
    let mut pairs = Vec::new();
    while let Some(tk) = toks.get(j) {
        if tk.kind == PKind::Dot {
            break;
        }
        let from = tk.text.clone();
        j += 1;
        if let Some(by) = toks.get(j) {
            if by.kind == PKind::Word && eqi(&by.text, "BY") {
                j += 1;
            }
        }
        let to = toks.get(j).map(|t| t.text.clone()).unwrap_or_default();
        j += 1;
        pairs.push((from, to));
    }
    let dot = toks.get(j)?;
    if dot.kind != PKind::Dot {
        return None;
    }
    Some((pairs, j, dot.end))
}

#[allow(clippy::too_many_arguments)]
fn load_and_expand(
    name: &str,
    replacing: &[(String, String)],
    base_dir: &Path,
    format: SourceFormat,
    errors: &mut Vec<String>,
    stack: &mut Vec<PathBuf>,
    depth: usize,
) -> String {
    if depth >= MAX_DEPTH {
        errors.push(format!("COPY nesting too deep at '{name}'"));
        return String::new();
    }
    let path = match resolve(name, base_dir) {
        Some(p) => p,
        None => {
            errors.push(format!("copybook not found: '{name}'"));
            return String::new();
        }
    };
    let canon = path.canonicalize().unwrap_or_else(|_| path.clone());
    if stack.contains(&canon) {
        errors.push(format!("cyclic COPY of '{name}'"));
        return String::new();
    }
    let raw = match std::fs::read_to_string(&path) {
        Ok(s) => s,
        Err(e) => {
            errors.push(format!("cannot read copybook '{name}': {e}"));
            return String::new();
        }
    };
    let flat = flatten(&raw, format);
    let replaced = apply_pairs(&flat, replacing);
    // Recursively expand nested COPY/REPLACE inside this copybook.
    stack.push(canon);
    let child_dir = path.parent().map(Path::to_path_buf).unwrap_or_else(|| base_dir.to_path_buf());
    let expanded = expand_text(&replaced, &child_dir, format, errors, stack, depth + 1);
    stack.pop();
    expanded
}

/// Resolve a copybook name to a file path under `base_dir`, trying common
/// extensions. Quotes around a literal name are stripped.
fn resolve(name: &str, base_dir: &Path) -> Option<PathBuf> {
    let name = name.trim_matches(|c| c == '"' || c == '\'');
    let exts = ["", ".cpy", ".CPY", ".cbl", ".CBL", ".cob", ".COB", ".cpb", ".cobol"];
    for ext in exts {
        let cand = base_dir.join(format!("{name}{ext}"));
        if cand.is_file() {
            return Some(cand);
        }
    }
    // Case-insensitive fallback: scan the directory.
    if let Ok(entries) = std::fs::read_dir(base_dir) {
        for entry in entries.flatten() {
            let fname = entry.file_name();
            let fstr = fname.to_string_lossy();
            let stem = fstr.rsplit_once('.').map(|(s, _)| s).unwrap_or(&fstr);
            if stem.eq_ignore_ascii_case(name) {
                return Some(entry.path());
            }
        }
    }
    None
}

/// Apply REPLACING / REPLACE substitutions to `text`. A single COBOL word is
/// replaced on word boundaries; multi-token pseudo-text is replaced literally.
fn apply_pairs(text: &str, pairs: &[(String, String)]) -> String {
    if pairs.is_empty() {
        return text.to_string();
    }
    let mut out = text.to_string();
    for (from, to) in pairs {
        if from.is_empty() {
            continue;
        }
        if is_single_word(from) {
            out = replace_word(&out, from, to);
        } else {
            out = out.replace(from.as_str(), to);
        }
    }
    out
}

fn is_single_word(s: &str) -> bool {
    !s.is_empty()
        && s.bytes()
            .all(|c| c.is_ascii_alphanumeric() || c == b'-' || c == b'_')
}

/// Whole-word (COBOL word) case-insensitive replacement.
fn replace_word(text: &str, from: &str, to: &str) -> String {
    let is_word = |c: u8| c.is_ascii_alphanumeric() || c == b'-' || c == b'_';
    let b = text.as_bytes();
    let fb = from.as_bytes();
    let mut out = String::with_capacity(text.len());
    let mut i = 0usize;
    while i < b.len() {
        if i + fb.len() <= b.len()
            && text[i..i + fb.len()].eq_ignore_ascii_case(from)
            && (i == 0 || !is_word(b[i - 1]))
            && (i + fb.len() == b.len() || !is_word(b[i + fb.len()]))
        {
            out.push_str(to);
            i += fb.len();
        } else {
            out.push(b[i] as char);
            i += 1;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn tmp() -> PathBuf {
        let d = std::env::temp_dir().join(format!("copytest-{}", std::process::id()));
        std::fs::create_dir_all(&d).unwrap();
        d
    }
    fn write(dir: &Path, name: &str, body: &str) {
        let mut f = std::fs::File::create(dir.join(name)).unwrap();
        f.write_all(body.as_bytes()).unwrap();
    }

    #[test]
    fn plain_copy_splices_file() {
        let d = tmp();
        write(&d, "REC.cpy", "01 WS-NAME PIC X(10).\n");
        let r = expand_copybooks("       COPY REC.\n", &d, SourceFormat::Free);
        assert!(r.errors.is_empty(), "{:?}", r.errors);
        assert!(r.text.contains("01 WS-NAME PIC X(10)."));
        assert!(!r.text.to_uppercase().contains("COPY REC"));
    }

    #[test]
    fn copy_replacing_pseudo_text() {
        let d = tmp();
        write(&d, "TAGREC.cpy", "01 :PFX:-NAME PIC X(10).\n");
        let r = expand_copybooks(
            "       COPY TAGREC REPLACING ==:PFX:== BY ==WS==.\n",
            &d,
            SourceFormat::Free,
        );
        assert!(r.errors.is_empty(), "{:?}", r.errors);
        assert!(r.text.contains("01 WS-NAME PIC X(10)."), "got: {}", r.text);
    }

    #[test]
    fn copy_replacing_word() {
        let d = tmp();
        write(&d, "W.cpy", "01 AAA PIC 9(4).\n");
        let r = expand_copybooks("COPY W REPLACING AAA BY BBB.\n", &d, SourceFormat::Free);
        assert!(r.text.contains("01 BBB PIC 9(4)."), "got: {}", r.text);
    }

    #[test]
    fn nested_copy() {
        let d = tmp();
        write(&d, "INNER.cpy", "05 INNER-FLD PIC 9.\n");
        write(&d, "OUTER.cpy", "01 OUTER.\n   COPY INNER.\n");
        let r = expand_copybooks("COPY OUTER.\n", &d, SourceFormat::Free);
        assert!(r.errors.is_empty(), "{:?}", r.errors);
        assert!(r.text.contains("01 OUTER."));
        assert!(r.text.contains("05 INNER-FLD PIC 9."), "got: {}", r.text);
    }

    #[test]
    fn missing_copybook_reports_error() {
        let d = tmp();
        let r = expand_copybooks("COPY NOPE.\n", &d, SourceFormat::Free);
        assert!(r.errors.iter().any(|e| e.contains("not found")));
    }

    #[test]
    fn replace_directive_rewrites_following_text() {
        let d = tmp();
        let r = expand_copybooks(
            "REPLACE ==FOO== BY ==BAR==.\n01 FOO PIC X.\nREPLACE OFF.\n01 FOO PIC 9.\n",
            &d,
            SourceFormat::Free,
        );
        assert!(r.text.contains("01 BAR PIC X."), "got: {}", r.text);
        assert!(r.text.contains("01 FOO PIC 9."), "REPLACE OFF should stop rewriting: {}", r.text);
    }
}
