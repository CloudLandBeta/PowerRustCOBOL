// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Rule-driven COBOL beautifier (spec 043).
//!
//! One engine serves every ✨ Beautify surface (main editor tabs, the event
//! editor, the COBOL Structure block editors). The rules, dictated by the
//! operator on 2026-08-07, live in `specs/043-beautify-rules/spec.md`:
//!
//! 1.  `EXEC … END-EXEC` interiors pass through verbatim.
//! 2.  Paragraphs at column 8.
//! 3.  `01`/`77`/`78` at column 8; nested levels +3 per depth step
//!     (`66`/`88` indent one step under their item).
//! 4.  A data entry is joined onto one line.
//! 5.  `PIC` and `VALUE` aligned to shared columns per run of consecutive
//!     declarations.
//! 6.  Emitted lines capped at 256 chars; literals split via column-7 `-`
//!     with re-quoted pieces, everything else wraps at a word boundary.
//! 7.  Procedure code at column 12.
//! 8.  Python-style nesting (4 per level); `END-x`/`ELSE`/`WHEN`/`CATCH`/
//!     `FINALLY` align with their opener; **erroneous code is rejected, not
//!     formatted**.
//! 9.  Missing periods added only where necessary (previous sentence before
//!     a paragraph, before `CATCH`/`FINALLY`, data entry before the next
//!     entry); never duplicated.
//! 10. Verb casing and comment alignment are caller options (the modal).
//! 11. Undo restores the pre-beautify text (the text widget's own undo).

use std::collections::HashSet;

use super::editor::{COBOL2002_KEYWORDS, DATA_KEYWORDS, DIVISION_KEYWORDS, VERBS};

/// How reserved words are re-cased (rule 10a). `Leave` keeps the author's
/// spelling untouched.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum VerbCase {
    Leave,
    Upper,
    Lower,
    Capitalize,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct BeautifyOptions {
    pub verb_case: VerbCase,
    /// `true` moves free-form `*>` comments to the surrounding code column;
    /// `false` leaves every comment exactly as authored. Classic column-7
    /// `*` / `/` indicator comments stay pinned at column 7 in both modes.
    pub align_comments: bool,
}

impl Default for BeautifyOptions {
    fn default() -> Self {
        Self {
            verb_case: VerbCase::Upper,
            align_comments: false,
        }
    }
}

/// The outcome: either the formatted text, or the reasons the text was left
/// untouched (rule 8 — errors block the whole beautify).
#[derive(Debug, PartialEq)]
pub(crate) enum Beautified {
    Formatted(String),
    Rejected(Vec<String>),
}

/// Longest line the beautifier will emit (rule 6).
const MAX_LINE: usize = 256;

// ── Segmentation ──────────────────────────────────────────────────────────────

#[derive(Debug)]
enum Unit {
    Blank,
    /// A full-line comment, kept with its original spelling.
    Comment(String),
    /// An `EXEC … END-EXEC` block: every raw line verbatim (rule 1).
    Exec(Vec<String>),
    /// One logical code line (column-7 continuations already joined).
    Code(String),
}

/// True for a classic fixed-form continuation line: `-` in column 7 with a
/// blank (or sequence-number) area before it.
fn is_continuation(raw: &str) -> bool {
    let b = raw.as_bytes();
    b.len() >= 7
        && b[6] == b'-'
        && b[..6]
            .iter()
            .all(|c| *c == b' ' || c.is_ascii_digit())
}

/// Split `text` into blank / comment / exec / logical-code units, joining
/// column-7 continuations into their opening line (rule 6: the continued
/// text is the remainder of a string literal, re-opened by its first quote).
fn segment(text: &str) -> (Vec<Unit>, Vec<String>) {
    let mut units: Vec<Unit> = Vec::new();
    let mut errors: Vec<String> = Vec::new();
    let mut exec: Option<Vec<String>> = None;

    for (idx, raw) in text.lines().enumerate() {
        let line_no = idx + 1;
        let t = raw.trim();

        if let Some(block) = exec.as_mut() {
            block.push(raw.to_owned());
            if t.to_ascii_uppercase().contains("END-EXEC") {
                units.push(Unit::Exec(exec.take().unwrap()));
            }
            continue;
        }

        if t.is_empty() {
            units.push(Unit::Blank);
            continue;
        }
        if is_continuation(raw) {
            let cont = raw[7..].trim_start();
            match units.iter_mut().rev().find(|u| !matches!(u, Unit::Blank)) {
                Some(Unit::Code(prev)) => {
                    if let Some(q) = cont.chars().next().filter(|c| *c == '"' || *c == '\'') {
                        // Literal continuation: drop the re-opening quote and
                        // splice the rest onto the still-open literal.
                        let mut rest = cont.chars();
                        rest.next();
                        let _ = q;
                        prev.push_str(rest.as_str());
                    } else {
                        prev.push(' ');
                        prev.push_str(cont);
                    }
                }
                _ => errors.push(format!(
                    "line {line_no}: continuation with nothing to continue"
                )),
            }
            continue;
        }
        if t.starts_with("*>") || t.starts_with('*') || t.starts_with('/') {
            units.push(Unit::Comment(raw.trim_end().to_owned()));
            continue;
        }

        let first = t.split_whitespace().next().unwrap_or("");
        if first.eq_ignore_ascii_case("EXEC") {
            if t.to_ascii_uppercase().contains("END-EXEC") {
                units.push(Unit::Exec(vec![raw.to_owned()]));
            } else {
                exec = Some(vec![raw.to_owned()]);
            }
            continue;
        }
        units.push(Unit::Code(t.to_owned()));
    }
    if exec.is_some() {
        errors.push("EXEC block is never closed by END-EXEC".to_owned());
    }

    // Every logical code line must close its string literals (a legitimately
    // open literal is only valid immediately before a continuation line,
    // which segmentation has already joined).
    for u in &units {
        if let Unit::Code(c) = u {
            if literal_open_at_end(c) {
                errors.push(format!("unterminated string literal: {}", elide(c)));
            }
        }
    }
    (units, errors)
}

/// Whether `s` ends inside a string literal. Doubled quotes inside a literal
/// (`"AB""CD"`) are the escaped form and do not close it.
fn literal_open_at_end(s: &str) -> bool {
    let mut quote: Option<char> = None;
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        match quote {
            Some(q) if c == q => {
                if chars.peek() == Some(&q) {
                    chars.next(); // escaped quote — still inside
                } else {
                    quote = None;
                }
            }
            Some(_) => {}
            None if c == '"' || c == '\'' => quote = Some(c),
            None => {}
        }
    }
    quote.is_some()
}

fn elide(s: &str) -> String {
    let mut out: String = s.chars().take(40).collect();
    if s.chars().count() > 40 {
        out.push('…');
    }
    out
}

/// True when the line's final period sits outside any string literal.
fn ends_with_period(s: &str) -> bool {
    !literal_open_at_end(s) && s.trim_end().ends_with('.')
}

// ── The error gate (rule 8) ───────────────────────────────────────────────────

/// Errors that make formatting unsafe. Full programs go through the real
/// lexer + parser; fragments get the structural scan (segmentation already
/// contributed literal/EXEC findings).
fn gate(text: &str, units: &[Unit], mut errors: Vec<String>) -> Vec<String> {
    let upper = text.to_ascii_uppercase();
    let program_shaped =
        upper.contains("IDENTIFICATION DIVISION") || upper.contains("ID DIVISION");
    if program_shaped {
        // The editor's text is column-fluid (formatting is exactly what was
        // asked for), so a fixed-form misdetection must not condemn valid
        // code: gate on the detected format first, and accept if either
        // format parses clean. Reported findings come from the detected one.
        let detected = crate::runner::detect_format(text);
        let parse_errors = |fmt: cobolt_lexer::SourceFormat| -> Vec<String> {
            let parsed = cobolt_parser::parse(cobolt_lexer::tokenize(text, fmt));
            let mut errs: Vec<String> = parsed
                .diagnostics
                .iter()
                .filter(|d| d.severity == cobolt_parser::Severity::Error)
                .map(|d| format!("line {}: {}", d.span.line, d.message))
                .collect();
            if parsed.program.is_none() {
                errs.push("parse failed — no program recovered".to_owned());
            }
            errs
        };
        let first_try = parse_errors(detected);
        if first_try.is_empty() {
            return errors;
        }
        let other = match detected {
            cobolt_lexer::SourceFormat::Fixed => cobolt_lexer::SourceFormat::Free,
            _ => cobolt_lexer::SourceFormat::Fixed,
        };
        if parse_errors(other).is_empty() {
            return errors;
        }
        errors.extend(first_try);
        return errors;
    }

    // Fragment: balance END-x terminators against openers.
    let mut stack: Vec<&'static str> = Vec::new();
    for u in units {
        let Unit::Code(c) = u else { continue };
        let up = c.to_ascii_uppercase();
        let words: Vec<&str> = up.split_whitespace().collect();
        let first = words.first().copied().unwrap_or("").trim_end_matches('.');
        match first {
            "IF" if !up.contains("END-IF") => stack.push("END-IF"),
            "EVALUATE" if !up.contains("END-EVALUATE") => stack.push("END-EVALUATE"),
            "TRY" if !up.contains("END-TRY") => stack.push("END-TRY"),
            "PERFORM"
                if !up.contains("END-PERFORM") && is_inline_perform_words(&words) =>
            {
                stack.push("END-PERFORM")
            }
            t @ ("END-IF" | "END-EVALUATE" | "END-TRY" | "END-PERFORM") => {
                match stack.pop() {
                    Some(open) if open == t => {}
                    Some(open) => errors.push(format!("{t} closes an open {open}")),
                    None => errors.push(format!("{t} has no matching opener")),
                }
            }
            _ => {}
        }
        if ends_with_period(c) {
            // A sentence period closes every inline scope — except a TRY,
            // whose body is made of whole sentences (mirrors the emitter).
            match stack.iter().rposition(|s| *s == "END-TRY") {
                Some(t) => stack.truncate(t + 1),
                None => stack.clear(),
            }
        }
    }
    for open in stack {
        errors.push(format!("missing {open}"));
    }
    errors
}

fn is_inline_perform_words(words: &[&str]) -> bool {
    match words.get(1).copied() {
        None => true,
        Some("UNTIL") | Some("VARYING") | Some("WITH") | Some("FOREVER") => true,
        _ => words
            .last()
            .is_some_and(|w| w.trim_end_matches('.') == "TIMES"),
    }
}

// ── Reserved-word casing (rules 10a / 11) ─────────────────────────────────────

fn reserved_set() -> HashSet<&'static str> {
    VERBS
        .iter()
        .chain(DIVISION_KEYWORDS.iter())
        .chain(DATA_KEYWORDS.iter())
        .chain(COBOL2002_KEYWORDS.iter())
        .copied()
        .collect()
}

/// Re-case every reserved word outside string literals per `mode`; all other
/// words (identifiers, PIC masks, literal contents) keep their spelling.
fn case_reserved(s: &str, reserved: &HashSet<&'static str>, mode: VerbCase) -> String {
    if mode == VerbCase::Leave {
        return s.to_owned();
    }
    let mut out = String::with_capacity(s.len());
    let mut word = String::new();
    let mut quote: Option<char> = None;

    let flush = |out: &mut String, word: &mut String| {
        if word.is_empty() {
            return;
        }
        let trailing_period = word.ends_with('.');
        let core = if trailing_period {
            &word[..word.len() - 1]
        } else {
            word.as_str()
        };
        let upper = core.to_ascii_uppercase();
        if reserved.contains(upper.as_str()) {
            let cased = match mode {
                VerbCase::Upper => upper,
                VerbCase::Lower => upper.to_ascii_lowercase(),
                VerbCase::Capitalize => {
                    let lower = upper.to_ascii_lowercase();
                    let mut c = lower.chars();
                    match c.next() {
                        Some(f) => f.to_ascii_uppercase().to_string() + c.as_str(),
                        None => lower,
                    }
                }
                VerbCase::Leave => unreachable!(),
            };
            out.push_str(&cased);
            if trailing_period {
                out.push('.');
            }
        } else {
            out.push_str(word);
        }
        word.clear();
    };

    for c in s.chars() {
        if let Some(q) = quote {
            out.push(c);
            if c == q {
                quote = None;
            }
            continue;
        }
        if c == '"' || c == '\'' {
            flush(&mut out, &mut word);
            quote = Some(c);
            out.push(c);
        } else if c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == '.' {
            word.push(c);
        } else {
            flush(&mut out, &mut word);
            out.push(c);
        }
    }
    flush(&mut out, &mut word);
    out
}

/// Collapse runs of spaces to one, outside string literals.
fn collapse_spaces(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut quote: Option<char> = None;
    let mut prev_space = false;
    for c in s.chars() {
        if let Some(q) = quote {
            out.push(c);
            if c == q {
                quote = None;
            }
            continue;
        }
        match c {
            '"' | '\'' => {
                quote = Some(c);
                prev_space = false;
                out.push(c);
            }
            ' ' => {
                if !prev_space {
                    out.push(' ');
                }
                prev_space = true;
            }
            _ => {
                prev_space = false;
                out.push(c);
            }
        }
    }
    out.trim_end().to_owned()
}

// ── Emission ──────────────────────────────────────────────────────────────────

#[derive(Clone, Copy, PartialEq)]
enum Div {
    Ident,
    Env,
    Data,
    Proc,
}

#[derive(Clone, Copy, PartialEq)]
enum Scope {
    If,
    Evaluate,
    When,
    Perform,
    Try,
}

/// Column of the first word of a data entry, per rule 3. `01`/`77`/`78`
/// reset to column 8; `66`/`88` sit one step under their item; everything
/// else nests +3 per depth step.
fn level_column(level: u32, levels: &mut Vec<u32>) -> usize {
    if matches!(level, 1 | 77 | 78) {
        levels.clear();
        levels.push(level);
        return 8;
    }
    if matches!(level, 66 | 88) {
        return 8 + levels.len() * 3;
    }
    while levels.last().is_some_and(|prev| *prev >= level) {
        levels.pop();
    }
    let col = 8 + levels.len() * 3;
    levels.push(level);
    col
}

fn leading_level(s: &str) -> Option<u32> {
    let w = s.split_whitespace().next()?;
    let n = w.parse::<u32>().ok()?;
    (n <= 88).then_some(n)
}

/// One joined data entry, split for column alignment (rule 5).
struct DataEntry {
    col: usize,
    left: String,
    pic: Option<String>,
    value: Option<String>,
    period: bool,
}

/// Split a one-line entry into name-part / PIC-part / VALUE-part at word
/// boundaries outside literals (case-insensitive, rule 5).
fn split_entry(entry: &str) -> (String, Option<String>, Option<String>, bool) {
    let period = ends_with_period(entry);
    let body = if period {
        entry.trim_end().trim_end_matches('.').trim_end()
    } else {
        entry.trim_end()
    };
    let mut pic_at: Option<usize> = None;
    let mut value_at: Option<usize> = None;
    let mut quote: Option<char> = None;
    let mut word_start: Option<usize> = None;
    let bytes = body.char_indices().collect::<Vec<_>>();
    let mut i = 0;
    while i <= bytes.len() {
        let (at, c) = bytes
            .get(i)
            .copied()
            .unwrap_or((body.len(), ' '));
        if let Some(q) = quote {
            if c == q {
                quote = None;
            }
            i += 1;
            continue;
        }
        if c == '"' || c == '\'' {
            quote = Some(c);
            word_start = None;
            i += 1;
            continue;
        }
        if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
            if word_start.is_none() {
                word_start = Some(at);
            }
        } else if let Some(ws) = word_start.take() {
            let word = body[ws..at].to_ascii_uppercase();
            match word.as_str() {
                "PIC" | "PICTURE" if pic_at.is_none() => pic_at = Some(ws),
                "VALUE" | "VALUES" if value_at.is_none() && Some(ws) != pic_at => {
                    value_at = Some(ws)
                }
                _ => {}
            }
        }
        i += 1;
    }
    match (pic_at, value_at) {
        (Some(p), Some(v)) if v > p => (
            collapse_spaces(&body[..p]),
            Some(collapse_spaces(&body[p..v])),
            Some(collapse_spaces(&body[v..])),
            period,
        ),
        (Some(p), _) => (
            collapse_spaces(&body[..p]),
            Some(collapse_spaces(&body[p..])),
            None,
            period,
        ),
        (None, Some(v)) => (
            collapse_spaces(&body[..v]),
            None,
            Some(collapse_spaces(&body[v..])),
            period,
        ),
        (None, None) => (collapse_spaces(body), None, None, period),
    }
}

/// Emit an aligned run of consecutive data entries (rule 5): every PIC
/// starts on the run's shared PIC column, every VALUE on the shared VALUE
/// column. A row too wide for the shared column falls back to one space —
/// same line, never the next (the columns are maxima, so only the 256-char
/// cap can force that).
fn flush_data_run(run: &mut Vec<DataEntry>, out: &mut Vec<String>) {
    if run.is_empty() {
        return;
    }
    let pic_col = run
        .iter()
        .filter(|e| e.pic.is_some())
        .map(|e| e.col + e.left.chars().count() + 1)
        .max()
        .map(|c| c + 1);
    let value_col = run
        .iter()
        .filter_map(|e| {
            e.value.as_ref()?;
            let pc = pic_col?;
            e.pic
                .as_ref()
                .map(|p| pc + p.chars().count() + 1)
        })
        .max();

    for e in run.drain(..) {
        let mut line = " ".repeat(e.col - 1);
        line.push_str(&e.left);
        if let Some(pic) = &e.pic {
            let target = pic_col.unwrap_or(0);
            pad_to_col(&mut line, target);
            line.push_str(pic);
        }
        if let Some(value) = &e.value {
            let target = match (e.pic.is_some(), value_col) {
                (_, Some(vc)) => vc,
                _ => 0,
            };
            pad_to_col(&mut line, target);
            line.push_str(value);
        }
        if e.period {
            line.push('.');
        }
        out.push(line);
    }
}

/// Pad with spaces so the next char lands on 1-based `col` (≥1 space).
fn pad_to_col(line: &mut String, col: usize) {
    let cur = line.chars().count() + 1;
    let pad = if col > cur { col - cur } else { 1 };
    for _ in 0..pad {
        line.push(' ');
    }
}

fn put(out: &mut Vec<String>, col: usize, content: &str) {
    let mut line = " ".repeat(col.saturating_sub(1));
    line.push_str(content);
    out.push(line);
}

/// Wrap every emitted line to `MAX_LINE` chars (rule 6): outside literals at
/// a word boundary (continuation indented 4 deeper); inside a literal via a
/// column-7 `-` continuation with the remainder re-quoted.
fn wrap_lines(lines: Vec<String>) -> Vec<String> {
    let mut out = Vec::with_capacity(lines.len());
    for line in lines {
        wrap_one(line, &mut out);
    }
    out
}

fn wrap_one(line: String, out: &mut Vec<String>) {
    if line.chars().count() <= MAX_LINE {
        out.push(line);
        return;
    }
    let chars: Vec<char> = line.chars().collect();
    let mut quote: Option<char> = None;
    let mut last_space_outside: Option<usize> = None;
    let mut quote_at_limit: Option<char> = None;
    for (i, &c) in chars.iter().enumerate().take(MAX_LINE) {
        match quote {
            Some(q) if c == q => quote = None,
            Some(_) => {}
            None if c == '"' || c == '\'' => quote = Some(c),
            None if c == ' ' => last_space_outside = Some(i),
            None => {}
        }
        if i + 1 == MAX_LINE {
            quote_at_limit = quote;
        }
    }
    let indent: String = chars.iter().take_while(|c| **c == ' ').collect();
    if let Some(q) = quote_at_limit {
        // Split inside the literal: the remainder re-opens on a column-7
        // continuation line, quote-enclosed (rule 6).
        let head: String = chars[..MAX_LINE].iter().collect();
        let rest: String = chars[MAX_LINE..].iter().collect();
        out.push(head);
        let mut cont = String::from("      -    ");
        cont.push(q);
        cont.push_str(&rest);
        wrap_one(cont, out);
    } else if let Some(sp) = last_space_outside.filter(|sp| *sp > indent.chars().count()) {
        let head: String = chars[..sp].iter().collect();
        let rest: String = chars[sp + 1..].iter().collect();
        out.push(head.trim_end().to_owned());
        let mut next = indent;
        next.push_str("    ");
        next.push_str(&rest);
        wrap_one(next, out);
    } else {
        out.push(chars.into_iter().collect());
    }
}

// ── The engine ────────────────────────────────────────────────────────────────

/// Beautify `text` per the spec-043 rules, or reject it with the list of
/// errors that make formatting unsafe (rule 8).
pub(crate) fn beautify_with_rules(text: &str, opts: &BeautifyOptions) -> Beautified {
    let (units, seg_errors) = segment(text);
    let errors = gate(text, &units, seg_errors);
    if !errors.is_empty() {
        return Beautified::Rejected(errors);
    }

    let reserved = reserved_set();
    let mut out: Vec<String> = Vec::new();
    let mut div = infer_initial_div(&units, &reserved);
    let mut scopes: Vec<Scope> = Vec::new();
    let mut data_levels: Vec<u32> = Vec::new();
    let mut data_run: Vec<DataEntry> = Vec::new();
    let mut data_pending: Option<String> = None;
    /// Index in `out` of the last emitted procedure code line.
    let mut last_code: Option<usize> = None;
    let mut prev_blank = false;

    let flush_pending_entry =
        |pending: &mut Option<String>,
         data_levels: &mut Vec<u32>,
         run: &mut Vec<DataEntry>| {
            if let Some(entry) = pending.take() {
                push_data_entry(&entry, true, data_levels, run);
            }
        };

    for (i, unit) in units.iter().enumerate() {
        match unit {
            Unit::Blank => {
                flush_pending_entry(&mut data_pending, &mut data_levels, &mut data_run);
                flush_data_run(&mut data_run, &mut out);
                if !prev_blank && !out.is_empty() {
                    out.push(String::new());
                    prev_blank = true;
                }
                continue;
            }
            Unit::Comment(raw) => {
                flush_pending_entry(&mut data_pending, &mut data_levels, &mut data_run);
                flush_data_run(&mut data_run, &mut out);
                prev_blank = false;
                let t = raw.trim();
                if opts.align_comments && t.starts_with("*>") {
                    let col = match div {
                        Div::Proc => 12 + scopes.len() * 4,
                        Div::Data => 8 + data_levels.len() * 3,
                        _ => 8,
                    };
                    put(&mut out, col, t);
                } else if t.starts_with("*>") {
                    // Leave free-form comments exactly as authored.
                    out.push(raw.clone());
                } else {
                    put(&mut out, 7, t);
                }
                continue;
            }
            Unit::Exec(lines) => {
                flush_pending_entry(&mut data_pending, &mut data_levels, &mut data_run);
                flush_data_run(&mut data_run, &mut out);
                prev_blank = false;
                // Rule 1: the interior is untouchable. The opening line is
                // placed by context; the rest keep their own columns.
                for (j, raw) in lines.iter().enumerate() {
                    if j == 0 {
                        let col = if div == Div::Proc {
                            12 + scopes.len() * 4
                        } else {
                            12
                        };
                        put(&mut out, col, raw.trim());
                    } else {
                        out.push(raw.trim_end().to_owned());
                    }
                }
                continue;
            }
            Unit::Code(code) => {
                prev_blank = false;
                let cased = case_reserved(code, &reserved, opts.verb_case);
                let upper = cased.to_ascii_uppercase();
                let words: Vec<&str> = upper.split_whitespace().collect();
                let first = words.first().copied().unwrap_or("").trim_end_matches('.');
                let second = words.get(1).copied().unwrap_or("").trim_end_matches('.');

                // Division / section headers reset context.
                if second == "DIVISION"
                    && matches!(
                        first,
                        "IDENTIFICATION" | "ID" | "ENVIRONMENT" | "DATA" | "PROCEDURE"
                    )
                {
                    flush_pending_entry(&mut data_pending, &mut data_levels, &mut data_run);
                    flush_data_run(&mut data_run, &mut out);
                    close_sentence(&mut out, last_code.take());
                    div = match first {
                        "PROCEDURE" => Div::Proc,
                        "DATA" => Div::Data,
                        "ENVIRONMENT" => Div::Env,
                        _ => Div::Ident,
                    };
                    scopes.clear();
                    data_levels.clear();
                    put(&mut out, 8, &collapse_spaces(&cased));
                    continue;
                }
                if second == "SECTION" {
                    flush_pending_entry(&mut data_pending, &mut data_levels, &mut data_run);
                    flush_data_run(&mut data_run, &mut out);
                    scopes.clear();
                    data_levels.clear();
                    div = match first {
                        "WORKING-STORAGE" | "LOCAL-STORAGE" | "LINKAGE" | "FILE"
                        | "SCREEN" | "REPORT" | "COMMUNICATION" => Div::Data,
                        "CONFIGURATION" | "INPUT-OUTPUT" => Div::Env,
                        _ => div,
                    };
                    // A SECTION header is preceded by one blank line
                    // (operator rule, 2026-08-07) — added only when the
                    // previous line has content, so it never doubles up.
                    if out.last().is_some_and(|l| !l.trim().is_empty()) {
                        out.push(String::new());
                    }
                    put(&mut out, 8, &collapse_spaces(&cased));
                    continue;
                }

                match div {
                    Div::Data => {
                        // Join wrapped clauses into one entry (rule 4): an
                        // entry runs until its period; a new level number or
                        // FD/section ends it and earns the missing period
                        // (rule 9).
                        if matches!(first, "FD" | "SD" | "RD" | "CD") {
                            flush_pending_entry(
                                &mut data_pending,
                                &mut data_levels,
                                &mut data_run,
                            );
                            flush_data_run(&mut data_run, &mut out);
                            data_levels.clear();
                            put(&mut out, 8, &collapse_spaces(&cased));
                            continue;
                        }
                        let starts_entry = leading_level(&cased).is_some();
                        if let Some(pending) = data_pending.as_mut() {
                            if starts_entry {
                                let entry = data_pending.take().unwrap();
                                push_data_entry(&entry, true, &mut data_levels, &mut data_run);
                            } else {
                                pending.push(' ');
                                pending.push_str(cased.trim());
                                if ends_with_period(pending) {
                                    let entry = data_pending.take().unwrap();
                                    push_data_entry(
                                        &entry,
                                        false,
                                        &mut data_levels,
                                        &mut data_run,
                                    );
                                }
                                continue;
                            }
                        }
                        if starts_entry {
                            if ends_with_period(&cased) {
                                push_data_entry(&cased, false, &mut data_levels, &mut data_run);
                            } else {
                                data_pending = Some(cased.clone());
                            }
                        } else {
                            // A stray clause with no open entry: emit as-is.
                            flush_data_run(&mut data_run, &mut out);
                            put(&mut out, 12, &collapse_spaces(&cased));
                        }
                    }
                    Div::Proc => {
                        let content = collapse_spaces(&cased);
                        // A paragraph is a lone dotted name — never a scope
                        // terminator or phase marker that happens to close
                        // its sentence (`END-IF.`, `ELSE.`, `END-TRY.`).
                        let is_paragraph = words.len() == 1
                            && words[0].ends_with('.')
                            && !reserved.contains(first)
                            && !first.starts_with("END-")
                            && !matches!(first, "ELSE" | "WHEN" | "CATCH" | "FINALLY");
                        if is_paragraph {
                            // Rule 9: the sentence before a paragraph must
                            // close; rule 2: the header sits at column 8.
                            close_sentence(&mut out, last_code.take());
                            scopes.clear();
                            put(&mut out, 8, &content);
                            continue;
                        }

                        if let Some(kind) = terminator_kind(first) {
                            pop_through(&mut scopes, kind);
                        } else if first == "WHEN"
                            && matches!(scopes.last(), Some(Scope::When))
                        {
                            scopes.pop();
                        } else if matches!(first, "CATCH" | "FINALLY") {
                            // Rule 9: the last sentence of the previous
                            // phase closes; the marker aligns with TRY.
                            close_sentence(&mut out, last_code);
                            while scopes.last().is_some_and(|s| *s != Scope::Try) {
                                scopes.pop();
                            }
                        }

                        let mut depth = scopes.len();
                        if first == "ELSE" {
                            depth = depth.saturating_sub(1);
                        } else if matches!(first, "CATCH" | "FINALLY") {
                            depth = depth.saturating_sub(1);
                        }
                        put(&mut out, 12 + depth * 4, &content);
                        last_code = Some(out.len() - 1);

                        let ends_period = ends_with_period(&content);
                        if !ends_period {
                            match first {
                                "IF" if !upper.contains("END-IF") => scopes.push(Scope::If),
                                "EVALUATE" if !upper.contains("END-EVALUATE") => {
                                    scopes.push(Scope::Evaluate)
                                }
                                "WHEN" => scopes.push(Scope::When),
                                "TRY" if !upper.contains("END-TRY") => {
                                    scopes.push(Scope::Try)
                                }
                                "PERFORM"
                                    if !upper.contains("END-PERFORM")
                                        && is_inline_perform_words(&words) =>
                                {
                                    scopes.push(Scope::Perform)
                                }
                                _ => {}
                            }
                        }
                        if ends_period {
                            // A period closes inline scopes — but a TRY body
                            // holds whole sentences, so everything above the
                            // innermost TRY closes and the TRY survives.
                            match scopes.iter().rposition(|s| *s == Scope::Try) {
                                Some(t) => scopes.truncate(t + 1),
                                None => scopes.clear(),
                            }
                        }
                    }
                    Div::Env | Div::Ident => {
                        let content = collapse_spaces(&cased);
                        if first.is_empty() {
                            continue;
                        }
                        if words[0].ends_with('.') && words.len() <= 2 {
                            put(&mut out, 8, &content);
                        } else if leading_level(&content).is_some() {
                            // A fragment that opens with data entries.
                            div = Div::Data;
                            flush_data_run(&mut data_run, &mut out);
                            if ends_with_period(&content) {
                                push_data_entry(&content, false, &mut data_levels, &mut data_run);
                            } else {
                                data_pending = Some(content);
                            }
                        } else {
                            put(&mut out, 12, &content);
                        }
                    }
                }
                let _ = i;
            }
        }
    }
    flush_pending_entry(&mut data_pending, &mut data_levels, &mut data_run);
    flush_data_run(&mut data_run, &mut out);

    let mut lines = wrap_lines(out);
    while lines.last().is_some_and(|l| l.trim().is_empty()) {
        lines.pop();
    }
    let mut result = lines.join("\n");
    result.push('\n');
    Beautified::Formatted(result)
}

/// Rule 9 helper: append the sentence period to the previously emitted code
/// line when it is missing (never duplicated — `ends_with_period` guards).
fn close_sentence(out: &mut [String], last_code: Option<usize>) {
    if let Some(i) = last_code {
        if let Some(line) = out.get_mut(i) {
            if !line.trim().is_empty() && !ends_with_period(line) {
                line.push('.');
            }
        }
    }
}

fn push_data_entry(
    entry: &str,
    add_period: bool,
    data_levels: &mut Vec<u32>,
    run: &mut Vec<DataEntry>,
) {
    let level = leading_level(entry).unwrap_or(1);
    let col = level_column(level, data_levels);
    let (left, pic, value, mut period) = split_entry(entry);
    if add_period {
        period = true; // rule 9: a data entry must close before the next one
    }
    run.push(DataEntry {
        col,
        left,
        pic,
        value,
        period,
    });
}

fn terminator_kind(first: &str) -> Option<Scope> {
    match first {
        "END-IF" => Some(Scope::If),
        "END-EVALUATE" => Some(Scope::Evaluate),
        "END-PERFORM" => Some(Scope::Perform),
        "END-TRY" => Some(Scope::Try),
        _ => None,
    }
}

/// Pop up to and including the opener `kind` (a trailing WHEN body closes
/// with its EVALUATE, mirroring the old formatter).
fn pop_through(scopes: &mut Vec<Scope>, kind: Scope) {
    while let Some(top) = scopes.pop() {
        if top == kind {
            break;
        }
        if !matches!(top, Scope::When) {
            break; // mismatch — the gate should have caught it; stay safe
        }
    }
}

/// Fragments carry no division header: infer where we start so structure
/// blocks format correctly (level number → Data, a known verb → Proc).
fn infer_initial_div(units: &[Unit], reserved: &HashSet<&'static str>) -> Div {
    for u in units {
        let Unit::Code(c) = u else { continue };
        let up = c.to_ascii_uppercase();
        let words: Vec<&str> = up.split_whitespace().collect();
        let first = words.first().copied().unwrap_or("").trim_end_matches('.');
        let second = words.get(1).copied().unwrap_or("").trim_end_matches('.');
        if second == "DIVISION" || second == "SECTION" {
            return Div::Ident;
        }
        if leading_level(c).is_some() || matches!(first, "FD" | "SD" | "RD" | "CD") {
            return Div::Data;
        }
        if VERBS.contains(&first) {
            return Div::Proc;
        }
        // A lone `name.` that is no reserved word is a paragraph header — the
        // fragment is procedure code (event handlers start this way).
        if words.len() == 1 && words[0].ends_with('.') && !reserved.contains(first) {
            return Div::Proc;
        }
        return Div::Ident;
    }
    Div::Ident
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn fmt(text: &str) -> String {
        match beautify_with_rules(text, &BeautifyOptions::default()) {
            Beautified::Formatted(s) => s,
            Beautified::Rejected(e) => panic!("rejected: {e:?}"),
        }
    }
    fn fmt_opts(text: &str, opts: BeautifyOptions) -> String {
        match beautify_with_rules(text, &opts) {
            Beautified::Formatted(s) => s,
            Beautified::Rejected(e) => panic!("rejected: {e:?}"),
        }
    }
    fn rejected(text: &str) -> Vec<String> {
        match beautify_with_rules(text, &BeautifyOptions::default()) {
            Beautified::Formatted(s) => panic!("formatted instead of rejected:\n{s}"),
            Beautified::Rejected(e) => e,
        }
    }
    fn col_of(line: &str) -> usize {
        line.chars().take_while(|c| *c == ' ').count() + 1
    }

    // Rule 1 — EXEC interiors are untouchable.
    #[test]
    fn exec_blocks_pass_through_verbatim() {
        let src = "MAIN-P.\n    EXEC RUST\n   let x=  1;   // weird   spacing\n      END-EXEC\n    DISPLAY \"OK\".\n";
        let out = fmt(src);
        assert!(
            out.contains("   let x=  1;   // weird   spacing"),
            "interior line must be byte-identical:\n{out}"
        );
    }

    // Rules 2 + 7 — paragraphs col 8, code col 12.
    #[test]
    fn paragraphs_col8_code_col12() {
        let out = fmt("  MAIN-P.\n      DISPLAY \"HI\".\n");
        let lines: Vec<&str> = out.lines().collect();
        assert_eq!(col_of(lines[0]), 8, "{out}");
        assert_eq!(col_of(lines[1]), 12, "{out}");
    }

    // Rule 3 — 01 at col 8, then +3 per nested depth step.
    #[test]
    fn level_indent_three_per_depth() {
        let out = fmt("01 A.\n05 B PIC X.\n10 C PIC X.\n05 D PIC X.\n77 E PIC 9.\n");
        let lines: Vec<&str> = out.lines().collect();
        assert_eq!(col_of(lines[0]), 8, "{out}");
        assert_eq!(col_of(lines[1]), 11, "{out}");
        assert_eq!(col_of(lines[2]), 14, "{out}");
        assert_eq!(col_of(lines[3]), 11, "{out}");
        assert_eq!(col_of(lines[4]), 8, "{out}");
    }

    // Rule 3 — 88s indent under their item, not back at col 8.
    #[test]
    fn condition_names_nest_under_their_item() {
        let out = fmt("01 FLAG PIC X.\n88 FLAG-ON VALUE \"Y\".\n");
        let lines: Vec<&str> = out.lines().collect();
        assert_eq!(col_of(lines[1]), 11, "{out}");
    }

    // Rule 4 — wrapped clauses join into one line.
    #[test]
    fn data_entry_joined_to_one_line() {
        let out = fmt("05 COMPANY-NAME\n   PIC X(40)\n   VALUE \"IPSUM LOREM\".\n");
        let lines: Vec<&str> = out.lines().collect();
        assert_eq!(lines.len(), 1, "{out}");
        assert!(lines[0].contains("PIC X(40)"), "{out}");
        assert!(lines[0].ends_with("VALUE \"IPSUM LOREM\"."), "{out}");
    }

    // Rule 5 — PIC and VALUE columns align across a run.
    #[test]
    fn pic_and_value_columns_align_across_a_run() {
        let out = fmt(
            "01 REC.\n05 A PIC X.\n05 LONG-NAME-HERE PIC X(10) VALUE \"A\".\n05 B PIC 9 VALUE 1.\n",
        );
        let lines: Vec<&str> = out.lines().collect();
        let pic_cols: Vec<usize> = lines[1..]
            .iter()
            .map(|l| l.to_ascii_uppercase().find(" PIC ").unwrap() + 2)
            .collect();
        assert!(
            pic_cols.windows(2).all(|w| w[0] == w[1]),
            "PIC columns differ: {pic_cols:?}\n{out}"
        );
        let val_cols: Vec<usize> = lines[1..]
            .iter()
            .filter(|l| l.to_ascii_uppercase().contains(" VALUE "))
            .map(|l| l.to_ascii_uppercase().find(" VALUE ").unwrap() + 2)
            .collect();
        assert!(
            val_cols.windows(2).all(|w| w[0] == w[1]),
            "VALUE columns differ: {val_cols:?}\n{out}"
        );
    }

    // Rule 6 — a monster literal is split onto column-7 continuations,
    // re-quoted, and no emitted line exceeds 256 chars.
    #[test]
    fn overlong_literal_wraps_via_column7_continuation() {
        let big = "A".repeat(400);
        let src = format!("MAIN-P.\n    DISPLAY \"{big}\".\n");
        let out = fmt(&src);
        assert!(
            out.lines().all(|l| l.chars().count() <= 256),
            "a line exceeds 256 chars"
        );
        assert!(
            out.lines().any(|l| l.as_bytes().get(6) == Some(&b'-')),
            "no column-7 continuation emitted:\n{out}"
        );
        let joined: String = out
            .lines()
            .filter(|l| l.contains('"') || l.as_bytes().get(6) == Some(&b'-'))
            .collect::<Vec<_>>()
            .join("");
        assert!(joined.matches('A').count() >= 400, "literal content lost");
    }

    // Rule 6 input side — an existing continuation joins back into one literal.
    #[test]
    fn input_continuations_are_joined() {
        let src = "MAIN-P.\n    DISPLAY \"AB\n      -\"CD\".\n";
        let out = fmt(src);
        assert!(out.contains("\"ABCD\""), "{out}");
    }

    // Rule 8 — Python-style nesting; terminators align with their verb.
    #[test]
    fn nesting_and_terminator_alignment() {
        let src = "MAIN-P.\nIF A = 1\nDISPLAY \"ONE\"\nIF B = 2\nDISPLAY \"TWO\"\nEND-IF\nEND-IF.\n";
        let out = fmt(src);
        let lines: Vec<&str> = out.lines().collect();
        assert_eq!(col_of(lines[1]), 12, "IF: {out}");
        assert_eq!(col_of(lines[2]), 16, "body: {out}");
        assert_eq!(col_of(lines[3]), 16, "inner IF: {out}");
        assert_eq!(col_of(lines[4]), 20, "inner body: {out}");
        assert_eq!(col_of(lines[5]), 16, "inner END-IF: {out}");
        assert_eq!(col_of(lines[6]), 12, "outer END-IF: {out}");
    }

    // Rule 8 — TRY/CATCH/FINALLY: markers align with TRY, bodies indent.
    #[test]
    fn try_catch_finally_alignment() {
        let src = "MAIN-P.\nTRY\nDISPLAY \"A\".\nCATCH EXCEPTION E\nDISPLAY \"B\".\nFINALLY\nDISPLAY \"C\".\nEND-TRY.\n";
        let out = fmt(src);
        let lines: Vec<&str> = out.lines().collect();
        let try_col = col_of(lines[1]);
        assert_eq!(col_of(lines[3]), try_col, "CATCH aligns with TRY: {out}");
        assert_eq!(col_of(lines[5]), try_col, "FINALLY aligns with TRY: {out}");
        assert_eq!(col_of(lines[7]), try_col, "END-TRY aligns with TRY: {out}");
        assert_eq!(col_of(lines[2]), try_col + 4, "TRY body indents: {out}");
        assert_eq!(col_of(lines[4]), try_col + 4, "CATCH body indents: {out}");
    }

    // Rule 8 — erroneous code is rejected untouched.
    #[test]
    fn unbalanced_scopes_are_rejected() {
        let errs = rejected("MAIN-P.\nIF A = 1\nDISPLAY \"X\"\n");
        assert!(
            errs.iter().any(|e| e.contains("END-IF")),
            "expected a missing END-IF finding: {errs:?}"
        );
    }
    #[test]
    fn unterminated_literal_is_rejected() {
        let errs = rejected("MAIN-P.\n    DISPLAY \"OOPS.\n");
        assert!(
            errs.iter().any(|e| e.contains("unterminated")),
            "{errs:?}"
        );
    }
    #[test]
    fn parse_errors_reject_full_programs() {
        let src = "IDENTIFICATION DIVISION.\nPROGRAM-ID. BAD.\nPROCEDURE DIVISION.\nMAIN-P.\n    MOVE TO .\n    GOBACK.\n";
        let errs = rejected(src);
        assert!(!errs.is_empty());
    }

    // Rule 9 — the sentence before a paragraph earns its period, exactly once.
    #[test]
    fn missing_period_added_before_paragraph_never_duplicated() {
        let src = "MAIN-P.\n    DISPLAY \"A\"\nNEXT-P.\n    DISPLAY \"B\".\n";
        let out = fmt(src);
        assert!(out.contains("DISPLAY \"A\"."), "{out}");
        assert!(!out.contains("DISPLAY \"A\".."), "{out}");
        let again = fmt(&out);
        assert!(!again.contains("DISPLAY \"A\".."), "idempotent: {again}");
    }

    // Rule 9 — a data entry followed by a new entry closes with a period.
    #[test]
    fn data_entry_period_added_before_next_entry() {
        let out = fmt("01 A PIC X\n01 B PIC 9.\n");
        let lines: Vec<&str> = out.lines().collect();
        assert!(lines[0].ends_with('.'), "{out}");
        assert!(!lines[0].ends_with(".."), "{out}");
    }

    // Rule 10a — verb casing options.
    #[test]
    fn verb_casing_options() {
        let src = "main-p.\n    display \"Hi\".\n";
        let upper = fmt_opts(
            src,
            BeautifyOptions {
                verb_case: VerbCase::Upper,
                align_comments: false,
            },
        );
        assert!(upper.contains("DISPLAY \"Hi\""), "{upper}");
        let lower = fmt_opts(
            "MAIN-P.\n    DISPLAY \"Hi\".\n",
            BeautifyOptions {
                verb_case: VerbCase::Lower,
                align_comments: false,
            },
        );
        assert!(lower.contains("display \"Hi\""), "{lower}");
        let cap = fmt_opts(
            src,
            BeautifyOptions {
                verb_case: VerbCase::Capitalize,
                align_comments: false,
            },
        );
        assert!(cap.contains("Display \"Hi\""), "{cap}");
        let leave = fmt_opts(
            "main-p.\n    dIsPlAy \"Hi\".\n",
            BeautifyOptions {
                verb_case: VerbCase::Leave,
                align_comments: false,
            },
        );
        assert!(leave.contains("dIsPlAy \"Hi\""), "{leave}");
    }

    // Rule 10b — comment alignment option; leave keeps them verbatim.
    #[test]
    fn comment_alignment_option() {
        let src = "MAIN-P.\nIF A = 1\n*> inside the if\nDISPLAY \"X\"\nEND-IF.\n";
        let left = fmt(src);
        assert!(left.lines().any(|l| l == "*> inside the if"), "{left}");
        let aligned = fmt_opts(
            src,
            BeautifyOptions {
                verb_case: VerbCase::Upper,
                align_comments: true,
            },
        );
        let line = aligned
            .lines()
            .find(|l| l.contains("inside the if"))
            .unwrap();
        assert_eq!(col_of(line), 16, "{aligned}");
    }

    // Identifiers and literal contents are never re-cased.
    #[test]
    fn identifiers_and_literals_keep_their_case() {
        let out = fmt("MAIN-P.\n    move my-Var to Other-var\n    display \"keep Case\".\n");
        assert!(out.contains("my-Var"), "{out}");
        assert!(out.contains("Other-var"), "{out}");
        assert!(out.contains("\"keep Case\""), "{out}");
    }

    // Fragments (structure blocks) format without any division header.
    #[test]
    fn working_storage_fragment_formats_as_data() {
        let out = fmt("01 WS-A.\n   05 WS-B PIC X(3) VALUE \"abc\".\n");
        let lines: Vec<&str> = out.lines().collect();
        assert_eq!(col_of(lines[0]), 8, "{out}");
        assert_eq!(col_of(lines[1]), 11, "{out}");
    }
    #[test]
    fn procedure_fragment_formats_as_code() {
        let out = fmt("DISPLAY \"A\"\nDISPLAY \"B\".\n");
        assert!(out.lines().all(|l| col_of(l) == 12), "{out}");
    }

    // A SECTION header gets one blank line before it — never two.
    #[test]
    fn sections_get_one_blank_line_before() {
        let src = "DATA DIVISION.\nWORKING-STORAGE SECTION.\n01 A PIC X.\nLINKAGE SECTION.\n01 B PIC X.\n";
        let out = fmt(src);
        let lines: Vec<&str> = out.lines().collect();
        for (i, line) in lines.iter().enumerate() {
            if line.trim_start().starts_with(|c: char| c.is_ascii_uppercase())
                && line.contains(" SECTION.")
            {
                assert!(i > 0, "section cannot be the very first line here");
                assert!(
                    lines[i - 1].trim().is_empty(),
                    "no blank line before {line:?}:\n{out}"
                );
            }
        }
        let again = fmt(&out);
        assert_eq!(again, out, "blank-before-SECTION must be idempotent");
        assert!(!out.contains("\n\n\n"), "never two blank lines:\n{out}");
    }

    // ELSE aligns with its IF; the body under it indents again.
    #[test]
    fn else_aligns_with_if() {
        let src = "MAIN-P.\nIF A = 1\nDISPLAY \"T\"\nELSE\nDISPLAY \"F\"\nEND-IF.\n";
        let out = fmt(src);
        let lines: Vec<&str> = out.lines().collect();
        let if_col = col_of(lines[1]);
        assert_eq!(col_of(lines[3]), if_col, "ELSE: {out}");
        assert_eq!(col_of(lines[4]), if_col + 4, "else body: {out}");
        assert_eq!(col_of(lines[5]), if_col, "END-IF: {out}");
    }
}
