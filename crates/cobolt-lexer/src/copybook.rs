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
            match parse_copy(&toks, i, text) {
                Some((name, library, replacing, end_idx, end_byte)) => {
                    // A qualified COPY looks in `base_dir/<library>/` first,
                    // falling back to the flat directory.
                    let lib_dir = library.as_deref().map(|l| base_dir.join(l));
                    let dir = match &lib_dir {
                        Some(d) if d.is_dir() => d.as_path(),
                        _ => base_dir,
                    };
                    let copy =
                        load_and_expand(&name, &replacing, dir, format, errors, stack, depth);
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
            match parse_replace(&toks, i, text) {
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
/// One REPLACING/REPLACE operand starting at token `j`: pseudo-text keeps its
/// inner text; a **string literal keeps its quotes** (the pair text is spliced
/// into source, and `FALSE-DATA-2 BY " TWO$"` must land quoted or the `$`
/// reaches the parser bare — SM202A COPY-TEST-16); an identifier extends
/// through its `IN`/`OF` qualification chain and a parenthesized subscript
/// (SM206A's `BY WRK-DS-05V00-O005-001 IN … IN GRP-001 (1)`), all as one
/// source slice. Returns the operand text and the index just past it.
fn take_operand(toks: &[PTok], j: usize, src: &str) -> (String, usize) {
    let t = &toks[j];
    match t.kind {
        PKind::Pseudo => (t.text.clone(), j + 1),
        PKind::Str => (src[t.start..t.end].to_string(), j + 1),
        _ => {
            let mut end = t.end;
            let mut jj = j + 1;
            loop {
                // IN/OF qualification chain.
                if let (Some(a), Some(b)) = (toks.get(jj), toks.get(jj + 1)) {
                    if a.kind == PKind::Word
                        && (eqi(&a.text, "IN") || eqi(&a.text, "OF"))
                        && b.kind == PKind::Word
                    {
                        end = b.end;
                        jj += 2;
                        continue;
                    }
                }
                // Parenthesized subscript — the parens live in the gaps
                // between tokens, so they are read from the source.
                if let Some(n) = toks.get(jj) {
                    if src[end..n.start].contains('(') {
                        let mut k = jj;
                        while let Some(tk2) = toks.get(k) {
                            let gap_end = toks.get(k + 1).map(|t| t.start).unwrap_or(src.len());
                            if let Some(p) = src[tk2.end..gap_end].find(')') {
                                end = tk2.end + p + 1;
                                jj = k + 1;
                                break;
                            }
                            k += 1;
                        }
                        continue;
                    }
                }
                break;
            }
            (src[t.start..end].to_string(), jj)
        }
    }
}

#[allow(clippy::type_complexity)]
fn parse_copy(
    toks: &[PTok],
    i: usize,
    src: &str,
) -> Option<(String, Option<String>, Vec<(String, String)>, usize, usize)> {
    let mut j = i + 1;
    let name_tok = toks.get(j)?;
    if !matches!(name_tok.kind, PKind::Word | PKind::Str) {
        return None;
    }
    let name = name_tok.text.clone();
    j += 1;
    // Optional OF/IN library — the qualifier selects WHICH library the text
    // comes from. CCVS85 SM207A keeps a member named ALTLB in two libraries
    // with different contents; skipping the qualifier fetched the same text
    // for both and QUAL-TEST-02 reported TEXT COPIED FROM WRONG LIBRARY.
    let mut library = None;
    if let Some(t) = toks.get(j) {
        if t.kind == PKind::Word && (eqi(&t.text, "OF") || eqi(&t.text, "IN")) {
            library = toks.get(j + 1).map(|l| l.text.clone());
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
                let (from, nj) = take_operand(toks, j, src);
                j = nj;
                if let Some(by) = toks.get(j) {
                    if by.kind == PKind::Word && eqi(&by.text, "BY") {
                        j += 1;
                    }
                }
                let (to, nj) = match toks.get(j) {
                    Some(_) => take_operand(toks, j, src),
                    None => (String::new(), j),
                };
                j = nj;
                replacing.push((from, to));
            }
        }
    }
    // terminating dot
    let dot = toks.get(j)?;
    if dot.kind != PKind::Dot {
        return None;
    }
    Some((name, library, replacing, j, dot.end))
}

/// Parse `REPLACE op BY op … .` or `REPLACE OFF.`.
fn parse_replace(
    toks: &[PTok],
    i: usize,
    src: &str,
) -> Option<(Vec<(String, String)>, usize, usize)> {
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
        let (from, nj) = take_operand(toks, j, src);
        j = nj;
        if let Some(by) = toks.get(j) {
            if by.kind == PKind::Word && eqi(&by.text, "BY") {
                j += 1;
            }
        }
        let (to, nj) = match toks.get(j) {
            Some(_) => take_operand(toks, j, src),
            None => (String::new(), j),
        };
        j = nj;
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
    // A debugging line in LIBRARY TEXT participates in COPY/REPLACING
    // matching as ordinary text — the comment-or-source decision belongs to a
    // later phase, and the REPLACING is frequently what removes it: CCVS85
    // SM206A PST-TEST-009's KP008 is `PERFORM FAIL. / D THIS IS GARBAGE. /
    // SUBTRACT 1 FROM ERROR-COUNTER.` with the whole tail replaced by
    // `PASS. `. Flattened as a comment, the operand could never match. The
    // indicator is blanked here, in the copybook path only.
    let raw = if matches!(format, SourceFormat::Fixed | SourceFormat::FixedStrict) {
        raw.lines()
            .map(|l| {
                let mut cs: Vec<char> = l.chars().collect();
                if matches!(cs.get(6), Some('D') | Some('d')) {
                    cs[6] = ' ';
                }
                cs.into_iter().collect::<String>()
            })
            .collect::<Vec<_>>()
            .join("\n")
    } else {
        raw
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
/// Apply every pair in ONE pass over the text, matching by TEXT WORDS.
///
/// * By text words, not bytes: the operand and the copybook wrap their lines
///   differently, so a verbatim compare never matched a multi-word operand
///   (SM201A COPY-TEST-11); and a whole word `001` must NOT match inside
///   `1005` (SM206A PST-TEST-006).
/// * One pass, all pairs together, first match wins, and the replacement is
///   never rescanned: `==1== BY ==5== ==5== BY ==7==` turns `1` into `5` and
///   STOPS — applying the pairs sequentially over the whole text cascaded it
///   to `7` (SM206A PST-TEST-005, XII-6 3.3 GR12).
/// * A standalone `,`/`;` is a separator, interchangeable with space
///   (SM208A REP-TEST-8); the period stays a text word of its own.
fn apply_pairs(text: &str, pairs: &[(String, String)]) -> String {
    if pairs.is_empty() {
        return text.to_string();
    }
    // The `:TAG:` idiom replaces INSIDE words — `==:PFX:== BY ==WS==` turns
    // `:PFX:-NAME` into `WS-NAME` — so those pairs are substring passes, run
    // before the word walk (they cannot cascade with it: a tag never equals a
    // text word).
    let mut text_owned;
    let mut text = text;
    for (f, t) in pairs {
        let f = f.trim();
        if f.len() > 2 && f.starts_with(':') && f.ends_with(':') && text.contains(f) {
            text_owned = text.replace(f, t.trim());
            text = &text_owned;
        }
    }
    let separator = |w: &str| w == "," || w == ";";
    let fw: Vec<(Vec<&str>, &str)> = pairs
        .iter()
        .map(|(f, t)| {
            (
                seq_words(f)
                    .into_iter()
                    .map(|(s, e)| &f[s..e])
                    .filter(|w| !separator(w))
                    .collect(),
                t.as_str(),
            )
        })
        .collect();
    let words: Vec<(usize, usize)> = seq_words(text)
        .into_iter()
        .filter(|&(s, e)| !separator(&text[s..e]))
        .collect();
    let mut out = String::with_capacity(text.len());
    let mut copied = 0usize;
    let mut w = 0usize;
    'outer: while w < words.len() {
        for (fwords, to) in &fw {
            if fwords.is_empty() || w + fwords.len() > words.len() {
                continue;
            }
            let matched = fwords.iter().enumerate().all(|(k, f)| {
                let (s, e) = words[w + k];
                text[s..e].eq_ignore_ascii_case(f)
            });
            if matched {
                let (s, _) = words[w];
                let (_, e) = words[w + fwords.len() - 1];
                out.push_str(&text[copied..s]);
                out.push_str(to);
                copied = e;
                w += fwords.len();
                continue 'outer;
            }
        }
        w += 1;
    }
    out.push_str(&text[copied..]);
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

    /// CCVS85 **SM207A**: `COPY ALTLB OF <library>` — the same member name
    /// in two libraries with different contents. The qualifier selects the
    /// subdirectory; ignoring it fetched the same text for both.
    #[test]
    fn a_qualified_copy_reads_from_that_library() {
        let d = tmp();
        write(&d, "ALTLB", "FLAT-TEXT.
");
        std::fs::create_dir_all(d.join("LIB2")).unwrap();
        write(&d.join("LIB2"), "ALTLB", "LIB2-TEXT.
");
        let src = "COPY ALTLB IN LIB2.
COPY ALTLB.
";
        let r = expand_copybooks(src, &d, SourceFormat::Free);
        assert!(
            r.text.contains("LIB2-TEXT"),
            "the qualified COPY did not read its library:
{}",
            r.text
        );
        assert!(
            r.text.contains("FLAT-TEXT"),
            "the unqualified COPY lost the flat lookup:
{}",
            r.text
        );
    }

    /// CCVS85 **SM208A** REP-TEST-8 (XII-7 3.4 GR6(b)): a standalone comma
    /// or semicolon is a separator, interchangeable with space for matching —
    /// `REPLACE ==MOVE;  "FAIL"  , TO== BY ==MOVE "PASS" TO==.` must rewrite
    /// `MOVE  , "FAIL";      TO  P-OR-F.`
    #[test]
    fn separators_are_interchangeable_in_pseudo_text_matching() {
        let d = tmp();
        let src = "REPLACE ==MOVE;  \"FAIL\"  , TO== BY ==MOVE \"PASS\" TO==.\nMOVE  , \"FAIL\";      TO  P-OR-F.\nREPLACE OFF.\n";
        let r = expand_copybooks(src, &d, SourceFormat::Free);
        assert!(
            r.text.contains("\"PASS\""),
            "the separator-styled text was not matched:\n{}",
            r.text
        );
        assert!(
            !r.text.contains("\"FAIL\""),
            "the original text words were left behind:\n{}",
            r.text
        );
    }

    /// CCVS85 **SM206A** PST-TEST-005 (XII-6 3.3 GR12): pairs apply in ONE
    /// pass and a replacement is never rescanned — `==1== BY ==5== ==5== BY
    /// ==7==` turns `1` into `5` and stops; sequential application cascaded
    /// it to `7`.
    #[test]
    fn replacement_output_is_never_rescanned() {
        let d = tmp();
        write(&d, "KP005", "ADD 1 TO W.\n");
        let r = expand_copybooks(
            "COPY KP005 REPLACING == 1 == BY == 5 == == 5 == BY == 7 ==.\n",
            &d,
            SourceFormat::Free,
        );
        assert!(r.text.contains("ADD 5 TO W"), "cascaded: {}", r.text);
    }

    /// CCVS85 **SM202A** COPY-TEST-16: a string-literal operand keeps its
    /// quotes — `FALSE-DATA-2 BY " TWO$"` splices the QUOTED literal, or the
    /// `$` reaches the parser bare.
    #[test]
    fn a_literal_operand_keeps_its_quotes() {
        let d = tmp();
        write(&d, "K2PRA", "MOVE FALSE-DATA-2 TO OUT-1.\n");
        let r = expand_copybooks(
            "COPY K2PRA REPLACING FALSE-DATA-2 BY \" TWO$\".\n",
            &d,
            SourceFormat::Free,
        );
        assert!(
            r.text.contains("MOVE \" TWO$\" TO OUT-1"),
            "unquoted splice: {}",
            r.text
        );
    }

    /// CCVS85 **SM206A** PST-TEST-002: a replacement operand that is an
    /// identifier extends through its IN/OF chain and subscript.
    #[test]
    fn an_identifier_operand_spans_its_qualification() {
        let d = tmp();
        write(&d, "KP002", "ADD 1 TO OLD-NAME.\n");
        let r = expand_copybooks(
            "COPY KP002 REPLACING ==OLD-NAME== BY NEW-NAME IN GRP-A OF GRP-B (1).\n",
            &d,
            SourceFormat::Free,
        );
        assert!(
            r.text.contains("ADD 1 TO NEW-NAME IN GRP-A OF GRP-B (1)"),
            "chain cut short: {}",
            r.text
        );
    }

    /// CCVS85 **SM206A** PST-TEST-009: a debugging line in library text
    /// participates in matching as ordinary text — the REPLACING is what
    /// removes it.
    #[test]
    fn a_copybook_debugging_line_participates_in_matching() {
        let d = tmp();
        // Column-true fixed format: 6-char sequence area, then the indicator.
        write(
            &d,
            "KP008",
            "000100     PERFORM FAIL.\n000200D    GARBAGE.\n000300     ADD 1 TO C1.\n",
        );
        let src = "000400     COPY KP008 REPLACING\n\
                   000500     ==FAIL. GARBAGE. ADD 1 TO C1. == BY ==PASS. ==.\n";
        let r = expand_copybooks(src, &d, SourceFormat::Fixed);
        assert!(
            r.text.contains("PERFORM PASS"),
            "the debug line did not participate: {}",
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
