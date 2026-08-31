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

use crate::source::{flatten_fixed, flatten_fixed_strict, SourceFormat};

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
        // Classic reference format: apply the column rules and join
        // continuation lines *before* looking for COPY/REPLACE, so a directive
        // or a copied literal split across lines is seen whole.
        SourceFormat::FixedStrict => flatten_fixed_strict(source),
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
    ColonColon,
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
        } else if c == b':' && i + 1 < b.len() && b[i + 1] == b':' {
            toks.push(PTok {
                kind: PKind::ColonColon,
                text: "::".to_string(),
                start: i,
                end: i + 2,
            });
            i += 2;
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
            toks.push(PTok {
                kind: PKind::Pseudo,
                text: inner,
                start,
                end: i,
            });
        } else if c == b'"' || c == b'\'' {
            let quote = c;
            let start = i;
            i += 1;
            // **A literal ends at its line, and a doubled delimiter is content.**
            // Both are the same rules the lexer applies (see token.rs); the
            // preprocessor had neither, and it only has to know *where the
            // literals are* so it does not read their contents as directives.
            //
            // Without the line limit, one unpaired quotation mark pairs with
            // the next quote several lines away and shifts the parity of every
            // literal after it. In CCVS85 that exposed the word `COPY` inside
            // the copyright banner —
            //   "CCVS74 NCC  COPY, NOT FOR DISTRIBUTION."
            // — as a directive, and the resulting expansion corrupted four
            // programs (NC215A, SG104A, SG105A, SG106A) that are otherwise
            // clean. Exactly the defect 1.62.12 fixed in the lexer, still here.
            while i < b.len() && b[i] != b'\n' {
                if b[i] == quote {
                    if i + 1 < b.len() && b[i + 1] == quote {
                        i += 2; // doubled delimiter — one character of content
                        continue;
                    }
                    break;
                }
                i += 1;
            }
            let inner = s[start + 1..i.min(b.len())].to_string();
            if i < b.len() && b[i] == quote {
                i += 1;
            }
            toks.push(PTok {
                kind: PKind::Str,
                text: inner,
                start,
                end: i,
            });
        } else if c == b'.' {
            toks.push(PTok {
                kind: PKind::Dot,
                text: ".".into(),
                start: i,
                end: i + 1,
            });
            i += 1;
        } else if is_word(c) {
            let start = i;
            while i < b.len() && is_word(b[i]) {
                i += 1;
            }
            toks.push(PTok {
                kind: PKind::Word,
                text: s[start..i].to_string(),
                start,
                end: i,
            });
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
            let is_method = i > 0 && toks[i - 1].kind == PKind::ColonColon;
            if is_method {
                i += 1;
                continue;
            }
            // Emit the gap before COPY (REPLACE-rewritten).
            out.push_str(&apply_pairs(&text[prev_end..t.start], &active));
            match parse_copy(&toks, i) {
                Some((name, replacing, end_idx, end_byte)) => {
                    let copy =
                        load_and_expand(&name, &replacing, base_dir, format, errors, stack, depth);
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
            let is_method = i > 0 && toks[i - 1].kind == PKind::ColonColon;
            if is_method {
                i += 1;
                continue;
            }
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
    let child_dir = path
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| base_dir.to_path_buf());
    let expanded = expand_text(&replaced, &child_dir, format, errors, stack, depth + 1);
    stack.pop();
    expanded
}

/// Resolve a copybook name to a file path under `base_dir`, trying common
/// extensions. Quotes around a literal name are stripped.
fn resolve(name: &str, base_dir: &Path) -> Option<PathBuf> {
    let name = name.trim_matches(|c| c == '"' || c == '\'');
    let exts = [
        "", ".cpy", ".CPY", ".cbl", ".CBL", ".cob", ".COB", ".cpb", ".cobol",
    ];
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
            // A multi-word operand is pseudo-text, and pseudo-text matches by
            // TEXT WORDS, not by bytes: the operand and the copybook wrap
            // their lines differently, so a verbatim `str::replace` can never
            // find `==02 TST-FLD-1  PICTURE 9(5). 02 FILLER\n PICTURE
            // X(115)==` inside K101A (CCVS85 SM201A COPY-TEST-11 — the
            // replacement silently never applied and TXT-FLD-1 stayed
            // undeclared). Compare whitespace-normalized word sequences,
            // case-insensitively; fall back to the verbatim replace only if
            // no sequence matched.
            let (replaced, n) = replace_token_seq(&out, from, to);
            out = if n > 0 {
                replaced
            } else {
                out.replace(from.as_str(), to)
            };
        }
    }
    out
}

/// The text-word positions of `s`: whitespace-split, with a trailing
/// separator (`.`, `,`, `;`) detached as a word of its own — a separator
/// period is its own text word, so the operand `…X(115)` must match the
/// copybook's `…X(115).` up to but not including the period.
fn seq_words(s: &str) -> Vec<(usize, usize)> {
    let b = s.as_bytes();
    let mut words = Vec::new();
    let mut i = 0usize;
    while i < b.len() {
        if b[i].is_ascii_whitespace() {
            i += 1;
            continue;
        }
        let start = i;
        while i < b.len() && !b[i].is_ascii_whitespace() {
            i += 1;
        }
        let end = i;
        // Detach trailing separators, innermost last: `9(5).` → `9(5)` `.`
        let mut cut = end;
        while cut > start + 1 && matches!(b[cut - 1], b'.' | b',' | b';') {
            cut -= 1;
        }
        if cut < end {
            words.push((start, cut));
            for k in cut..end {
                words.push((k, k + 1));
            }
        } else {
            words.push((start, end));
        }
    }
    words
}

/// Replace every occurrence of the text-word sequence `from` (compared
/// case-insensitively word by word, whitespace- and line-break-insensitive)
/// with `to`, returning the new text and how many sequences were replaced.
fn replace_token_seq(text: &str, from: &str, to: &str) -> (String, usize) {
    let fpos = seq_words(from);
    if fpos.is_empty() {
        return (text.to_string(), 0);
    }
    let fwords: Vec<&str> = fpos.iter().map(|&(s, e)| &from[s..e]).collect();
    let words = seq_words(text);
    let mut out = String::with_capacity(text.len());
    let mut copied = 0usize;
    let mut n = 0usize;
    let mut w = 0usize;
    while w + fwords.len() <= words.len() {
        let matched = fwords.iter().enumerate().all(|(k, fw)| {
            let (s, e) = words[w + k];
            text[s..e].eq_ignore_ascii_case(fw)
        });
        if matched {
            let (s, _) = words[w];
            let (_, e) = words[w + fwords.len() - 1];
            out.push_str(&text[copied..s]);
            out.push_str(to);
            copied = e;
            n += 1;
            w += fwords.len();
        } else {
            w += 1;
        }
    }
    out.push_str(&text[copied..]);
    (out, n)
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
        assert!(
            r.text.contains("01 FOO PIC 9."),
            "REPLACE OFF should stop rewriting: {}",
            r.text
        );
    }

    // ── The word COPY inside a literal is not a directive ────────────────────

    /// A literal ends at its line, so an unpaired quotation mark cannot pair
    /// with one several lines away and shift the parity of everything after it.
    ///
    /// This is what exposed `COPY` inside the CCVS85 copyright banner as a
    /// directive and corrupted four otherwise-clean programs (NC215A, SG104A,
    /// SG105A, SG106A). It is the same rule the lexer got in 1.62.12.
    #[test]
    fn the_word_copy_inside_a_literal_is_not_a_directive() {
        let d = tmp();
        write(&d, "K1PRA", "01 SHOULD-NOT-APPEAR PIC X.\n");
        let src = "       IDENTIFICATION DIVISION.\n\
                          PROGRAM-ID. BANNER.\n\
                          DATA DIVISION.\n\
                          WORKING-STORAGE SECTION.\n\
                          01  CCVS-BANNER.\n\
                   \x20          02 FILLER PIC X(40) VALUE\n\
                   \x20          \"CCVS74 NCC  COPY, NOT FOR DISTRIBUTION.\".\n\
                   \x20          02 FILLER PIC X(15) VALUE \" COPYRIGHT 1974\".\n\
                          PROCEDURE DIVISION.\n\
                          MAIN.\n\
                   \x20          STOP RUN.\n";
        let r = expand_copybooks(src, &d, SourceFormat::Free);
        assert!(
            !r.text.contains("SHOULD-NOT-APPEAR"),
            "the word COPY inside a literal was expanded as a directive:\n{}",
            r.text
        );
        assert!(
            r.text.contains("NOT FOR DISTRIBUTION"),
            "the banner literal must survive intact:\n{}",
            r.text
        );
    }

    /// An unpaired quotation mark must not swallow a REAL directive on a
    /// later line — the containment has to cut both ways.
    #[test]
    fn an_unpaired_quote_does_not_hide_a_later_copy() {
        let d = tmp();
        write(&d, "REAL", "01 REALLY-COPIED PIC X.\n");
        let src = "01 A PIC X(20) VALUE \"unpaired.\n\
                   COPY REAL.\n";
        let r = expand_copybooks(src, &d, SourceFormat::Free);
        assert!(
            r.text.contains("REALLY-COPIED"),
            "a stray quote hid the COPY on the next line:\n{}",
            r.text
        );
    }

    /// CCVS85 **SM201A** COPY-TEST-11: pseudo-text matches by TEXT WORDS —
    /// whitespace- and line-break-insensitive, with a separator period as a
    /// word of its own. The operand wraps its lines differently from the
    /// copybook and stops before the copybook's final glued period, so a
    /// verbatim string replace never matched and the substitution silently
    /// did not happen.
    #[test]
    fn pseudo_text_matches_by_text_words_across_line_breaks() {
        let d = tmp();
        write(
            &d,
            "K101A",
            "            .
    02 TST-FLD-1 PICTURE 9(5).
    02 FILLER    PICTURE X(115).
",
        );
        let src = "01 TEXT-TEST-1 COPY K101A
            REPLACING ==02 TST-FLD-1  PICTURE 9(5). 02 FILLER
                      PICTURE X(115)==
            BY        ==02 FILLER PICTURE X(115).  02 TXT-FLD-1
                      PIC 9(5)==.
";
        let r = expand_copybooks(src, &d, SourceFormat::Free);
        assert!(
            r.text.contains("TXT-FLD-1"),
            "the pseudo-text replacement did not apply:
{}",
            r.text
        );
        assert!(
            !r.text.contains("TST-FLD-1"),
            "the matched text words were left behind:
{}",
            r.text
        );
    }

    /// A doubled delimiter is content, so a literal containing `""` does not
    /// end early and expose what follows it.
    #[test]
    fn a_doubled_delimiter_does_not_end_the_literal_early() {
        let d = tmp();
        write(&d, "K9", "01 NOPE PIC X.\n");
        let src = "01 A PIC X(40) VALUE \"say \"\"COPY K9.\"\" please\".\n";
        let r = expand_copybooks(src, &d, SourceFormat::Free);
        assert!(
            !r.text.contains("NOPE"),
            "COPY inside a doubled-quote literal was expanded:\n{}",
            r.text
        );
    }
}
