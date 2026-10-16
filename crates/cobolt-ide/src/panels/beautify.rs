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
//! 1.  `EXEC … END-EXEC` interiors pass through verbatim, and so does a
//!     ``` ``` ``` block literal — fences included (operator, 2026-09-07).
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
//!
//! On top of those, [`BeautifyOptions::style`] switches on the PowerRustCOBOL
//! layout style (operator, 2026-10-12; see `beautify_style`): 4-space data
//! levels, the first data clause and the `TO` of `MOVE`/`SET` in column 50,
//! blank lines around divisions, comments and blocks of repeated verbs, and —
//! only when proved harmless — `SET` for `MOVE`, no `INVOKE` on a plain call and
//! no sentence periods inside the procedure.

use std::collections::HashSet;

use super::beautify_style;
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
    /// The PowerRustCOBOL layout style (operator, 2026-10-12). Off keeps the
    /// spec-043 behaviour exactly.
    pub style: bool,
}

impl Default for BeautifyOptions {
    fn default() -> Self {
        Self {
            verb_case: VerbCase::Upper,
            align_comments: false,
            style: false,
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

/// What an emitted line is. The style's layout pass reads this rather than
/// guessing from the text, which cannot tell a `MOVE` inside a block literal
/// from a `MOVE` statement.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) enum Kind {
    Blank,
    /// A blank line the style wants *unless* the next line closes a scope
    /// (`END-IF`, `ELSE`) or ends the code: the one after a wrapped `TO`.
    Gap,
    /// A free-form `*>` comment the style may move to the code's column.
    Comment,
    /// A comment that stays where it is (classic column-7, or "leave as written").
    Pinned,
    Division,
    Section,
    Paragraph,
    Data,
    /// First line of a procedure statement; `verb` keys the repeated-verb rule.
    Stmt { depth: usize, verb: &'static str },
    /// A further line of the same statement (`INTO …`, `AT END …`, `TO …`).
    Cont { depth: usize },
    /// A scope marker: `ELSE`, `WHEN …`, `CATCH`, `END-IF`, …
    Marker { depth: usize },
    /// A line of a ``` block literal: byte for byte, never wrapped.
    Verbatim,
    Other,
}

/// The emitted lines, each with its [`Kind`].
pub(super) struct Out {
    pub lines: Vec<String>,
    pub kinds: Vec<Kind>,
}

impl Out {
    pub fn new() -> Self {
        Self { lines: Vec::new(), kinds: Vec::new() }
    }
    pub fn push(&mut self, line: String, kind: Kind) {
        self.lines.push(line);
        self.kinds.push(kind);
    }
    /// Push `content` starting in 1-based column `col`.
    fn put(&mut self, col: usize, content: &str, kind: Kind) {
        let mut line = " ".repeat(col.saturating_sub(1));
        line.push_str(content);
        self.push(line, kind);
    }
    fn len(&self) -> usize {
        self.lines.len()
    }
    fn is_empty(&self) -> bool {
        self.lines.is_empty()
    }
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
    /// A ``` ``` ``` block literal: every raw line verbatim, byte for byte,
    /// the two fence lines included (rule 1).
    ///
    /// This is a real language construct, not markdown in a comment — the
    /// lexer turns the lines between a pair of fences into a single
    /// `StringLiteral` (see `Lexer::capture_block_literal`). Its text is taken
    /// with no escaping, which is what makes it useful for JSON, SQL and HTML,
    /// and which is exactly why the beautifier must not go near it: re-indent
    /// a line and the literal's value changes.
    Fence(Vec<String>),
    /// One logical code line (column-7 continuations already joined).
    Code(String),
}

/// Whether this line opens a ``` ``` ``` block literal — that is, whether it
/// carries a fence OUTSIDE any string literal.
///
/// The quote walk is the point. `MOVE "```" TO WS-X` mentions a fence inside an
/// ordinary literal and opens nothing; treating it as an opener would swallow
/// the rest of the program as literal text.
fn opens_fence(s: &str) -> bool {
    let b = s.as_bytes();
    let mut i = 0usize;
    let mut quote: Option<u8> = None;
    while i < b.len() {
        let c = b[i];
        match quote {
            Some(q) => {
                // A doubled quote inside a literal is the escaped form.
                if c == q {
                    if i + 1 < b.len() && b[i + 1] == q {
                        i += 2;
                        continue;
                    }
                    quote = None;
                }
                i += 1;
            }
            None => {
                if c == b'"' || c == b'\'' {
                    quote = Some(c);
                } else if b[i..].starts_with(b"```") {
                    return true;
                }
                i += 1;
            }
        }
    }
    false
}

/// Whether this line CLOSES a block literal. Matches the lexer exactly: a line
/// whose first non-blank text is a fence.
fn closes_fence(s: &str) -> bool {
    s.trim_start().starts_with("```")
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
    let mut fence: Option<Vec<String>> = None;

    for (idx, raw) in text.lines().enumerate() {
        let line_no = idx + 1;
        let t = raw.trim();

        // An open block literal swallows every line until its closing fence —
        // blanks, comments and EXEC included. Checked before anything else so
        // that nothing inside the literal is read as code.
        if let Some(block) = fence.as_mut() {
            block.push(raw.to_owned());
            if closes_fence(raw) {
                units.push(Unit::Fence(fence.take().unwrap()));
            }
            continue;
        }

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
        // A fence opens the literal. EXEC is settled first, above: an EXEC
        // line is a standard construct and keeps precedence.
        if opens_fence(t) {
            fence = Some(vec![raw.to_owned()]);
            continue;
        }
        units.push(Unit::Code(t.to_owned()));
    }
    if exec.is_some() {
        errors.push("EXEC block is never closed by END-EXEC".to_owned());
    }
    if fence.is_some() {
        // Rule 8: reject rather than format. An unclosed fence means the rest
        // of the file is literal text, and guessing where it ends would
        // rewrite code the developer never meant to be code.
        errors.push("``` block literal is never closed by a matching ```".to_owned());
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

    let flush = |out: &mut String, word: &mut String, receiver: bool| {
        if word.is_empty() {
            return;
        }
        // A member name (`NUD-AGE::Value`, `Timer-1::Start()`) and a receiver
        // (`super::GetProperty`, `me::Close`) are the developer's own spelling,
        // never a verb, even when they match a reserved word.
        if receiver || out.ends_with("::") {
            out.push_str(word);
            word.clear();
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

    let chars: Vec<char> = s.chars().collect();
    for (i, &c) in chars.iter().enumerate() {
        if let Some(q) = quote {
            out.push(c);
            if c == q {
                quote = None;
            }
            continue;
        }
        if c == '"' || c == '\'' {
            flush(&mut out, &mut word, false);
            quote = Some(c);
            out.push(c);
        } else if c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == '.' {
            word.push(c);
        } else {
            let receiver = c == ':' && chars.get(i + 1) == Some(&':');
            flush(&mut out, &mut word, receiver);
            out.push(c);
        }
    }
    flush(&mut out, &mut word, false);
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
    /// A statement with conditional phrases, up to its `END-<verb>` (style only).
    Phrase(&'static str),
    /// The body of a phrase that stands alone on its line — `NOT AT END` (style only).
    PhraseBody,
}

/// Column of the first word of a data entry, per rule 3. `01`/`77`/`78`
/// reset to column 8; `66`/`88` sit one step under their item; everything
/// else nests +3 per depth step.
fn level_column(level: u32, levels: &mut Vec<u32>, step: usize) -> usize {
    if matches!(level, 1 | 77 | 78) {
        levels.clear();
        levels.push(level);
        return 8;
    }
    if matches!(level, 66 | 88) {
        return 8 + levels.len() * step;
    }
    while levels.last().is_some_and(|prev| *prev >= level) {
        levels.pop();
    }
    let col = 8 + levels.len() * step;
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
    /// The joined entry without its period (the style lays it out itself).
    body: String,
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
fn flush_data_run(run: &mut Vec<DataEntry>, out: &mut Out, style: bool) {
    if run.is_empty() {
        return;
    }
    if style {
        // The style needs no run: the first clause of every entry sits in
        // column 50, whatever its neighbours look like.
        for e in run.drain(..) {
            let (lines, blank_after) = beautify_style::data_entry(e.col, &e.body, e.period);
            for line in lines {
                out.push(line, Kind::Data);
            }
            if blank_after {
                out.push(String::new(), Kind::Gap);
            }
        }
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
        out.push(line, Kind::Data);
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

/// Wrap every emitted line to `MAX_LINE` chars (rule 6): outside literals at
/// a word boundary (continuation indented 4 deeper); inside a literal via a
/// column-7 `-` continuation with the remainder re-quoted.
fn wrap_lines(lines: Vec<String>, verbatim: &std::collections::HashSet<usize>, whole_literals: bool) -> Vec<String> {
    let mut out = Vec::with_capacity(lines.len());
    for (i, line) in lines.into_iter().enumerate() {
        // A block literal's line is never wrapped: splitting it with a
        // column-7 continuation would insert characters into the literal's
        // own text. A long line of JSON is long on purpose.
        if verbatim.contains(&i) {
            out.push(line);
            continue;
        }
        wrap_one(line, &mut out, whole_literals);
    }
    out
}

fn wrap_one(line: String, out: &mut Vec<String>, whole_literals: bool) {
    if line.chars().count() <= MAX_LINE {
        out.push(line);
        return;
    }
    let chars: Vec<char> = line.chars().collect();
    let mut quote: Option<char> = None;
    let mut literal_start = 0usize;
    let mut last_space_outside: Option<usize> = None;
    let mut quote_at_limit: Option<char> = None;
    for (i, &c) in chars.iter().enumerate().take(MAX_LINE) {
        match quote {
            Some(q) if c == q => quote = None,
            Some(_) => {}
            None if c == '"' || c == '\'' => {
                quote = Some(c);
                literal_start = i;
            }
            None if c == ' ' => last_space_outside = Some(i),
            None => {}
        }
        if i + 1 == MAX_LINE {
            quote_at_limit = quote;
        }
    }
    let indent: String = chars.iter().take_while(|c| **c == ' ').collect();
    // Style: a literal that fits on a line of its own is never cut in two —
    // the line breaks before it. A cut changes nothing the developer wrote only
    // if the lexer rejoins it exactly, and a value is not worth that risk.
    if let (true, Some(q)) = (whole_literals, quote_at_limit) {
        let end = chars[literal_start + 1..].iter().position(|c| *c == q).map(|p| literal_start + 1 + p);
        let fits = end.is_some_and(|e| e - literal_start + 1 + indent.chars().count() + 4 <= MAX_LINE);
        if let (true, Some(sp)) = (fits, last_space_outside.filter(|sp| *sp > indent.chars().count() && *sp < literal_start)) {
            let head: String = chars[..sp].iter().collect();
            let rest: String = chars[sp + 1..].iter().collect();
            out.push(head.trim_end().to_owned());
            let mut next = indent;
            next.push_str("    ");
            next.push_str(&rest);
            wrap_one(next, out, whole_literals);
            return;
        }
    }
    if let Some(q) = quote_at_limit {
        // Split inside the literal: the remainder re-opens on a column-7
        // continuation line, quote-enclosed (rule 6).
        let head: String = chars[..MAX_LINE].iter().collect();
        let rest: String = chars[MAX_LINE..].iter().collect();
        out.push(head);
        let mut cont = String::from("      -    ");
        cont.push(q);
        cont.push_str(&rest);
        wrap_one(cont, out, whole_literals);
    } else if let Some(sp) = last_space_outside.filter(|sp| *sp > indent.chars().count()) {
        let head: String = chars[..sp].iter().collect();
        let rest: String = chars[sp + 1..].iter().collect();
        out.push(head.trim_end().to_owned());
        let mut next = indent;
        next.push_str("    ");
        next.push_str(&rest);
        wrap_one(next, out, whole_literals);
    } else {
        out.push(chars.into_iter().collect());
    }
}

// ── The engine ────────────────────────────────────────────────────────────────

/// Beautify `text` per the spec-043 rules, or reject it with the list of
/// errors that make formatting unsafe (rule 8).
///
/// With [`BeautifyOptions::style`] the layout style is applied as well. Three of
/// its rules change tokens rather than whitespace — no sentence periods, no
/// `INVOKE` before a plain call, `SET` for `MOVE` — and those are kept only when
/// the formatted text parses to the same program as the original. Otherwise the
/// text is formatted again with the layout rules alone.
pub(crate) fn beautify_with_rules(text: &str, opts: &BeautifyOptions) -> Beautified {
    beautify_checked(text, opts).0
}

/// [`beautify_with_rules`], also saying whether the style's token-changing rules
/// were refused (`true`) because the result could not be proved to parse to the
/// same program, so only the layout was applied.
pub(crate) fn beautify_checked(text: &str, opts: &BeautifyOptions) -> (Beautified, bool) {
    let first = beautify_once(text, opts, opts.style);
    if !opts.style {
        return (first, false);
    }
    match &first {
        Beautified::Formatted(out) if beautify_style::programs_equivalent(text, out) != Some(true) => {
            (beautify_once(text, opts, false), true)
        }
        _ => (first, false),
    }
}

/// A lone dotted name that is no reserved word: a paragraph header. A lone
/// call such as `Txt-Log::AppendText(WS-NL).` also ends in a period but is no
/// name — a paragraph name is letters, digits, hyphens and underscores.
fn is_paragraph_header(words: &[&str], first: &str, reserved: &HashSet<&'static str>) -> bool {
    words.len() == 1
        && words[0].ends_with('.')
        && first.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
        && !reserved.contains(first)
        && !first.starts_with("END-")
        && !matches!(first, "ELSE" | "WHEN" | "CATCH" | "FINALLY")
}

/// Does this code line start a new division, section or paragraph?
fn is_boundary(code: &str, reserved: &HashSet<&'static str>) -> bool {
    let up = code.to_ascii_uppercase();
    let words: Vec<&str> = up.split_whitespace().collect();
    let first = words.first().copied().unwrap_or("").trim_end_matches('.');
    let second = words.get(1).copied().unwrap_or("").trim_end_matches('.');
    second == "DIVISION" || second == "SECTION" || is_paragraph_header(&words, first, reserved)
}

/// For every unit: is the next unit that matters (past blanks and comments) a
/// division, section or paragraph header, or the end of the text? A sentence
/// period right before one of those stays (style rule 7).
fn period_stays(units: &[Unit], reserved: &HashSet<&'static str>) -> Vec<bool> {
    let mut stays = vec![true; units.len()];
    let mut next_is_boundary = true;
    for (i, unit) in units.iter().enumerate().rev() {
        stays[i] = next_is_boundary;
        match unit {
            Unit::Blank | Unit::Comment(_) => {}
            Unit::Code(c) => next_is_boundary = is_boundary(c, reserved),
            Unit::Exec(_) | Unit::Fence(_) => next_is_boundary = false,
        }
    }
    stays
}

/// A `MOVE`/`SET` written over two lines (`MOVE x` / `TO y`) is one statement:
/// join it, so the style can place its `TO`.
fn join_split_statements(units: Vec<Unit>) -> Vec<Unit> {
    let mut out: Vec<Unit> = Vec::with_capacity(units.len());
    for unit in units {
        if let Unit::Code(next) = &unit {
            let starts_with_to = next.split_whitespace().next().is_some_and(|w| w.eq_ignore_ascii_case("TO"));
            if let Some(Unit::Code(prev)) = out.last_mut() {
                // `MOVE x` / `TO y`, and `MOVE x TO a` / `b` / `c`.
                if (starts_with_to && beautify_style::lacks_to(prev))
                    || beautify_style::continues_targets(prev, next)
                    || beautify_style::continues_invoke(prev, next)
                {
                    prev.push(' ');
                    prev.push_str(next);
                    continue;
                }
            }
        }
        out.push(unit);
    }
    out
}

fn beautify_once(text: &str, opts: &BeautifyOptions, rewrite: bool) -> Beautified {
    let style = opts.style;
    let (units, seg_errors) = segment(text);
    let errors = gate(text, &units, seg_errors);
    if !errors.is_empty() {
        return Beautified::Rejected(errors);
    }

    let reserved = reserved_set();
    let units = if style { join_split_statements(units) } else { units };
    let stays = if style { period_stays(&units, &reserved) } else { Vec::new() };
    let scoped = if style { phrase_scoped(&units, &reserved) } else { Vec::new() };
    let level_step = if style { 4 } else { 3 };
    let mut out = Out::new();
    let mut div = infer_initial_div(&units, &reserved);
    let mut scopes: Vec<Scope> = Vec::new();
    let mut data_levels: Vec<u32> = Vec::new();
    let mut data_run: Vec<DataEntry> = Vec::new();
    let mut data_pending: Option<String> = None;
    /// Index in `out` of the last emitted procedure code line.
    let mut last_code: Option<usize> = None;
    let mut prev_blank = false;
    // Style rule 7: a conditional phrase (`AT END`, `INVALID KEY`, …) is open
    // until its `END-<verb>`; a period would close it, so the period stays.
    let mut phrase_open = false;
    // Style: the clauses of a `SELECT` (`ORGANIZATION …`, `RECORD KEY …`) sit
    // one level under it.
    let mut in_select = false;

    let flush_pending_entry =
        |pending: &mut Option<String>,
         data_levels: &mut Vec<u32>,
         run: &mut Vec<DataEntry>| {
            if let Some(entry) = pending.take() {
                push_data_entry(&entry, true, data_levels, run, level_step);
            }
        };

    for (i, unit) in units.iter().enumerate() {
        match unit {
            Unit::Blank => {
                flush_pending_entry(&mut data_pending, &mut data_levels, &mut data_run);
                flush_data_run(&mut data_run, &mut out, style);
                if !prev_blank && !out.is_empty() {
                    out.push(String::new(), Kind::Blank);
                    prev_blank = true;
                }
                continue;
            }
            Unit::Comment(raw) => {
                flush_pending_entry(&mut data_pending, &mut data_levels, &mut data_run);
                flush_data_run(&mut data_run, &mut out, style);
                prev_blank = false;
                let t = raw.trim();
                if style {
                    // Style rule 5: a free-form comment sits in the column of
                    // the code below it (the layout pass places it). A classic
                    // column-7 comment stays pinned.
                    if t.starts_with("*>") {
                        out.push(t.to_owned(), Kind::Comment);
                    } else {
                        out.put(7, t, Kind::Pinned);
                    }
                } else if opts.align_comments && t.starts_with("*>") {
                    let col = match div {
                        Div::Proc => 12 + scopes.len() * 4,
                        Div::Data => 8 + data_levels.len() * 3,
                        _ => 8,
                    };
                    out.put(col, t, Kind::Other);
                } else if t.starts_with("*>") {
                    // Leave free-form comments exactly as authored.
                    out.push(raw.clone(), Kind::Other);
                } else {
                    out.put(7, t, Kind::Other);
                }
                continue;
            }
            Unit::Exec(lines) => {
                flush_pending_entry(&mut data_pending, &mut data_levels, &mut data_run);
                flush_data_run(&mut data_run, &mut out, style);
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
                        out.put(col, raw.trim(), Kind::Other);
                    } else {
                        out.push(raw.trim_end().to_owned(), Kind::Other);
                    }
                }
                continue;
            }
            Unit::Fence(lines) => {
                flush_pending_entry(&mut data_pending, &mut data_levels, &mut data_run);
                flush_data_run(&mut data_run, &mut out, style);
                prev_blank = false;
                // Byte for byte, fences included. Not even `trim_end`: trailing
                // spaces on a line inside a block literal are part of its
                // value, and re-indenting the opening line would move the fence
                // itself — which is what the operator asked be left alone.
                for raw in lines {
                    out.push(raw.clone(), Kind::Verbatim);
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
                    flush_data_run(&mut data_run, &mut out, style);
                    close_sentence(&mut out.lines, last_code.take());
                    div = match first {
                        "PROCEDURE" => Div::Proc,
                        "DATA" => Div::Data,
                        "ENVIRONMENT" => Div::Env,
                        _ => Div::Ident,
                    };
                    scopes.clear();
                    data_levels.clear();
                    phrase_open = false;
                    in_select = false;
                    out.put(8, &collapse_spaces(&cased), Kind::Division);
                    continue;
                }
                if second == "SECTION" {
                    flush_pending_entry(&mut data_pending, &mut data_levels, &mut data_run);
                    flush_data_run(&mut data_run, &mut out, style);
                    scopes.clear();
                    data_levels.clear();
                    in_select = false;
                    div = match first {
                        "WORKING-STORAGE" | "LOCAL-STORAGE" | "LINKAGE" | "FILE"
                        | "SCREEN" | "REPORT" | "COMMUNICATION" => Div::Data,
                        "CONFIGURATION" | "INPUT-OUTPUT" => Div::Env,
                        _ => div,
                    };
                    // A SECTION header is preceded by one blank line
                    // (operator rule, 2026-08-07) — added only when the
                    // previous line has content, so it never doubles up.
                    if out.lines.last().is_some_and(|l| !l.trim().is_empty()) {
                        out.push(String::new(), Kind::Blank);
                    }
                    out.put(8, &collapse_spaces(&cased), Kind::Section);
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
                            flush_data_run(&mut data_run, &mut out, style);
                            data_levels.clear();
                            out.put(8, &collapse_spaces(&cased), Kind::Data);
                            continue;
                        }
                        let starts_entry = leading_level(&cased).is_some();
                        if let Some(pending) = data_pending.as_mut() {
                            if starts_entry {
                                let entry = data_pending.take().unwrap();
                                push_data_entry(&entry, true, &mut data_levels, &mut data_run, level_step);
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
                                        level_step,
                                    );
                                }
                                continue;
                            }
                        }
                        if starts_entry {
                            if ends_with_period(&cased) {
                                push_data_entry(&cased, false, &mut data_levels, &mut data_run, level_step);
                            } else {
                                data_pending = Some(cased.clone());
                            }
                        } else {
                            // A stray clause with no open entry: emit as-is.
                            flush_data_run(&mut data_run, &mut out, style);
                            out.put(12, &collapse_spaces(&cased), Kind::Data);
                        }
                    }
                    Div::Proc => {
                        let content = collapse_spaces(&cased);
                        // A paragraph is a lone dotted name — never a scope
                        // terminator or phase marker that happens to close
                        // its sentence (`END-IF.`, `ELSE.`, `END-TRY.`).
                        if is_paragraph_header(&words, first, &reserved) {
                            // Rule 9: the sentence before a paragraph must
                            // close; rule 2: the header sits at column 8.
                            close_sentence(&mut out.lines, last_code.take());
                            scopes.clear();
                            phrase_open = false;
                            out.put(8, &content, Kind::Paragraph);
                            continue;
                        }

                        // Style: a phrase statement is a scope. `END-READ` closes it;
                        // a phrase marker (`AT END`) sits one level inside it.
                        let mut phrase_line = false;
                        if style {
                            if let Some(v) = first.strip_prefix("END-").and_then(phrase_verb) {
                                if let Some(pos) = scopes.iter().rposition(|s| *s == Scope::Phrase(v)) {
                                    scopes.truncate(pos);
                                }
                            } else if phrase_marker(&upper).is_some() {
                                if matches!(scopes.last(), Some(Scope::PhraseBody)) {
                                    scopes.pop();
                                }
                                phrase_line = matches!(scopes.last(), Some(Scope::Phrase(_)));
                            }
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
                            close_sentence(&mut out.lines, last_code);
                            while scopes.last().is_some_and(|s| *s != Scope::Try) {
                                scopes.pop();
                            }
                        }

                        let top_before = scopes.last().copied();
                        let mut depth = scopes.len();
                        if first == "ELSE" {
                            depth = depth.saturating_sub(1);
                        } else if matches!(first, "CATCH" | "FINALLY") {
                            depth = depth.saturating_sub(1);
                        }
                        // Style: a line that starts no statement and is no marker
                        // (`INTO …`, `WITH POINTER …`, `AND …`) belongs to the
                        // statement above and sits one level in — except inside a
                        // phrase scope, which already put it there. A condition
                        // continuation (`AND`/`OR`) sits two levels in from its
                        // header, clear of the body.
                        let marker_like = matches!(first, "ELSE" | "WHEN" | "CATCH" | "FINALLY")
                            || first.starts_with("END-")
                            || phrase_line;
                        let prev_code = units[..i]
                            .iter()
                            .rev()
                            .find_map(|u| if let Unit::Code(c) = u { Some(c.as_str()) } else { None })
                            .unwrap_or("");
                        let continuation = style
                            && !marker_like
                            && beautify_style::statement_verb_after(&content, prev_code).is_none();
                        let condition_line = continuation && matches!(first, "AND" | "OR");
                        if continuation && !condition_line && !matches!(top_before, Some(Scope::Phrase(_))) {
                            depth += 1;
                        }
                        let mut col = 12 + depth * 4;
                        if condition_line {
                            if let Some(j) = out
                                .kinds
                                .iter()
                                .rposition(|k| matches!(k, Kind::Stmt { .. } | Kind::Marker { .. }))
                            {
                                col = out.lines[j].chars().take_while(|c| *c == ' ').count() + 1 + 8;
                            }
                        }

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
                                v if style
                                    && scoped.get(i).copied().unwrap_or(false)
                                    && phrase_verb(v).is_some()
                                    && !upper.contains(&format!("END-{v}")) =>
                                {
                                    scopes.push(Scope::Phrase(phrase_verb(v).unwrap_or("READ")))
                                }
                                // Spec 093 R13: `A = B * C` with an `END-COMPUTE` of its own is a scope too.
                                _ if style
                                    && scoped.get(i).copied().unwrap_or(false)
                                    && beautify_style::statement_verb(&content).is_none()
                                    && beautify_style::is_assignment_start(&content)
                                    && !beautify_style::after_open_condition(prev_code)
                                    && !upper.contains("END-COMPUTE") =>
                                {
                                    scopes.push(Scope::Phrase("COMPUTE"))
                                }
                                _ if phrase_line && phrase_marker(&upper) == Some(true) => {
                                    scopes.push(Scope::PhraseBody)
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

                        if !style {
                            out.put(col, &content, Kind::Other);
                            last_code = Some(out.len() - 1);
                            let _ = i;
                            continue;
                        }

                        // ── the PowerRustCOBOL style ──
                        let mut text = beautify_style::restyle(&content, rewrite);
                        if beautify_style::PHRASE_WORDS.iter().any(|p| upper.contains(p)) {
                            phrase_open = true;
                        }
                        if first.starts_with("END-") && terminator_kind(first).is_none() {
                            phrase_open = false;
                        }
                        // Rule 7: the period goes when nothing depends on it —
                        // no scope or phrase left open by it, the line is not a
                        // one-line IF/EVALUATE/PERFORM that the period closes,
                        // and nothing (paragraph, section, end of code) follows.
                        let closes_itself = match first {
                            "IF" | "EVALUATE" | "SEARCH" | "TRY" => {
                                !upper.contains(&format!("END-{first}"))
                            }
                            "PERFORM" => {
                                is_inline_perform_words(&words) && !upper.contains("END-PERFORM")
                            }
                            _ => false,
                        };
                        if rewrite
                            && ends_period
                            && text.trim() != "."
                            && scopes.is_empty()
                            && !phrase_open
                            && !closes_itself
                            && !stays[i]
                        {
                            text = text.trim_end().trim_end_matches('.').trim_end().to_owned();
                        }
                        if ends_period {
                            phrase_open = false;
                        }

                        let stmt_verb = beautify_style::statement_verb_after(&text, prev_code);
                        let marker = marker_like;
                        let kind = if marker {
                            Kind::Marker { depth }
                        } else if let Some(verb) = stmt_verb {
                            Kind::Stmt { depth, verb }
                        } else {
                            Kind::Cont { depth }
                        };
                        let (pieces, blank_after) = beautify_style::place(col, &text);
                        for (n, (line, continuation)) in pieces.into_iter().enumerate() {
                            let k = if n == 0 || !continuation { kind } else { Kind::Cont { depth } };
                            out.push(line, k);
                        }
                        if blank_after {
                            out.push(String::new(), Kind::Gap);
                        }
                        last_code = out.lines.iter().rposition(|l| !l.trim().is_empty());
                    }
                    Div::Env | Div::Ident => {
                        let content = collapse_spaces(&cased);
                        if first.is_empty() {
                            continue;
                        }
                        if words[0].ends_with('.') && words.len() <= 2 {
                            in_select = false;
                            out.put(8, &content, Kind::Other);
                        } else if leading_level(&content).is_some() {
                            // A fragment that opens with data entries.
                            div = Div::Data;
                            flush_data_run(&mut data_run, &mut out, style);
                            if ends_with_period(&content) {
                                push_data_entry(&content, false, &mut data_levels, &mut data_run, level_step);
                            } else {
                                data_pending = Some(content);
                            }
                        } else {
                            if style && first == "SELECT" {
                                in_select = true;
                            }
                            let col = if style && in_select && first != "SELECT" { 16 } else { 12 };
                            out.put(col, &content, Kind::Other);
                        }
                    }
                }
                let _ = i;
            }
        }
    }
    flush_pending_entry(&mut data_pending, &mut data_levels, &mut data_run);
    flush_data_run(&mut data_run, &mut out, style);

    let out = if style { beautify_style::finish(out) } else { out };
    // The lines of a block literal reach the file exactly as they are.
    let verbatim: std::collections::HashSet<usize> = out
        .kinds
        .iter()
        .enumerate()
        .filter(|(_, k)| **k == Kind::Verbatim)
        .map(|(n, _)| n)
        .collect();
    let mut lines = wrap_lines(out.lines, &verbatim, style);
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
    step: usize,
) {
    let level = leading_level(entry).unwrap_or(1);
    let col = level_column(level, data_levels, step);
    let (left, pic, value, mut period) = split_entry(entry);
    if add_period {
        period = true; // rule 9: a data entry must close before the next one
    }
    let body = {
        let t = entry.trim_end();
        collapse_spaces(if ends_with_period(entry) { t.trim_end_matches('.').trim_end() } else { t })
    };
    run.push(DataEntry {
        col,
        body,
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

/// Verbs whose conditional phrases (`AT END`, `INVALID KEY`, `ON SIZE ERROR` …)
/// may hold statements; each closes with `END-<verb>`.
const PHRASE_VERBS: &[&str] = &[
    "ACCEPT", "ADD", "CALL", "COMPUTE", "DELETE", "DISPLAY", "DIVIDE", "MULTIPLY", "READ",
    "RETURN", "REWRITE", "SEARCH", "START", "STRING", "SUBTRACT", "UNSTRING", "WRITE",
];

fn phrase_verb(word: &str) -> Option<&'static str> {
    PHRASE_VERBS.iter().copied().find(|v| *v == word)
}

/// A conditional-phrase marker at the start of a line. `Some(true)` when the
/// line is the marker alone, so its body follows on the lines below.
fn phrase_marker(upper: &str) -> Option<bool> {
    const MARKERS: &[&str] = &[
        "NOT AT END", "AT END", "NOT INVALID KEY", "INVALID KEY", "NOT ON SIZE ERROR",
        "ON SIZE ERROR", "NOT ON OVERFLOW", "ON OVERFLOW", "NOT ON EXCEPTION", "ON EXCEPTION",
    ];
    let t = upper.trim().trim_end_matches('.').trim_end();
    MARKERS.iter().find_map(|m| {
        if t == *m {
            Some(true)
        } else if t.strip_prefix(m).is_some_and(|rest| rest.starts_with(' ')) {
            Some(false)
        } else {
            None
        }
    })
}

/// For every unit: is it the first line of a phrase statement that is closed by
/// its own `END-<verb>`? Only those open a scope — a plain `ADD 1 TO X` or
/// `STRING … INTO Y` has no terminator and holds nothing. The nearest opener of
/// the verb pairs with the terminator, which is how the compiler reads it too.
fn phrase_scoped(units: &[Unit], reserved: &HashSet<&'static str>) -> Vec<bool> {
    let mut scoped = vec![false; units.len()];
    let mut stack: Vec<(usize, &'static str)> = Vec::new();
    for (i, unit) in units.iter().enumerate() {
        let Unit::Code(c) = unit else { continue };
        if is_boundary(c, reserved) {
            stack.clear();
            continue;
        }
        let up = c.to_ascii_uppercase();
        let first = up.split_whitespace().next().unwrap_or("").trim_end_matches('.');
        // Spec 093 R13: `A = B * C` opens a `COMPUTE` scope, so its `ON SIZE ERROR` and `END-COMPUTE` align.
        let prev_code = units[..i].iter().rev().find_map(|u| if let Unit::Code(p) = u { Some(p.as_str()) } else { None }).unwrap_or("");
        let shorthand = beautify_style::statement_verb(c).is_none()
            && beautify_style::is_assignment_start(c)
            && !beautify_style::after_open_condition(prev_code);
        if let Some(v) = first.strip_prefix("END-").and_then(phrase_verb) {
            if let Some(pos) = stack.iter().rposition(|(_, sv)| *sv == v) {
                scoped[stack[pos].0] = true;
                stack.truncate(pos);
            }
        } else if let Some(v) = phrase_verb(first).or_else(|| shorthand.then_some("COMPUTE")) {
            if !up.contains(&format!("END-{v}")) {
                stack.push((i, v));
            }
        }
        if ends_with_period(c) {
            stack.clear();
        }
    }
    scoped
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

    // Rule 1 — a ``` block literal is untouchable, fences included.
    //
    // The lexer takes the text between the fences verbatim, with no escaping,
    // so every character between them is part of the literal's VALUE. Anything
    // the beautifier does in there — re-indent, collapse runs of spaces,
    // uppercase a word, wrap a long line — silently changes what the program
    // moves into a data item.
    #[test]
    fn a_block_literal_survives_byte_for_byte() {
        let src = concat!(
            "MAIN-P.\n",
            "    MOVE\n",
            "    ```\n",
            "  {  \"name\" :  \"ACME\",   \n",
            "       \"rows\": [1,2,3]   }\n",
            "\n",
            "        trailing indent kept\n",
            "    ```\n",
            "    TO WS-JSON.\n",
        );
        let out = fmt(src);
        for line in [
            "  {  \"name\" :  \"ACME\",   ",
            "       \"rows\": [1,2,3]   }",
            "        trailing indent kept",
        ] {
            assert!(
                out.lines().any(|l| l == line),
                "literal line was altered: {line:?}\n--- got ---\n{out}"
            );
        }
        // The blank line inside the literal is content too.
        let body: Vec<&str> = out.lines().collect();
        let open = body.iter().position(|l| l.trim() == "```").expect("open fence");
        let close = body[open + 1..]
            .iter()
            .position(|l| l.trim() == "```")
            .expect("close fence")
            + open
            + 1;
        assert!(
            body[open + 1..close].iter().any(|l| l.is_empty()),
            "the blank line inside the literal was dropped:\n{out}"
        );
    }

    #[test]
    fn the_fences_themselves_are_left_where_they_were() {
        let src = "MAIN-P.\n    MOVE\n      ```\nhello\n      ```\n    TO WS-X.\n";
        let out = fmt(src);
        assert_eq!(
            out.lines().filter(|l| *l == "      ```").count(),
            2,
            "both fences must keep their own column:\n{out}"
        );
    }

    // Rule 6 caps emitted lines at 256 characters by splitting them with a
    // column-7 continuation. Doing that to a block literal would insert the
    // continuation characters into the literal's own text.
    #[test]
    fn a_long_literal_line_is_never_wrapped() {
        let long = "x".repeat(400);
        let src = format!("MAIN-P.\n    MOVE\n    ```\n{long}\n    ```\n    TO WS-X.\n");
        let out = fmt(&src);
        assert!(
            out.lines().any(|l| l == long),
            "a {}-char literal line was wrapped:\n{out}",
            long.len()
        );
    }

    // A fence mentioned INSIDE an ordinary quoted literal is not a fence. If it
    // were treated as one, the rest of the program would be swallowed as
    // literal text and come back unformatted — or be rejected as unclosed.
    #[test]
    fn a_fence_inside_a_quoted_literal_opens_nothing() {
        let src = "MAIN-P.\n        MOVE   \"```\"   TO WS-X.\n        DISPLAY    \"AFTER\".\n";
        let out = fmt(src);
        assert!(
            out.contains("\"```\""),
            "the quoted fence must survive as a literal:\n{out}"
        );
        // The line after it was still formatted — proof nothing was swallowed.
        let after = out
            .lines()
            .find(|l| l.contains("AFTER"))
            .expect("the DISPLAY must still be there");
        assert_eq!(col_of(after), 12, "code after it is still formatted:\n{out}");
    }

    // Rule 8 — reject rather than guess. An unclosed fence means everything
    // below it is literal text, and picking an end would rewrite real code.
    #[test]
    fn an_unclosed_block_literal_is_rejected() {
        let errs = rejected("MAIN-P.\n    MOVE\n    ```\nstill open\n");
        assert!(
            errs.iter().any(|e| e.contains("never closed")),
            "expected an unclosed-fence error, got {errs:?}"
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
                style: false,
            },
        );
        assert!(upper.contains("DISPLAY \"Hi\""), "{upper}");
        let lower = fmt_opts(
            "MAIN-P.\n    DISPLAY \"Hi\".\n",
            BeautifyOptions {
                verb_case: VerbCase::Lower,
                align_comments: false,
                style: false,
            },
        );
        assert!(lower.contains("display \"Hi\""), "{lower}");
        let cap = fmt_opts(
            src,
            BeautifyOptions {
                verb_case: VerbCase::Capitalize,
                align_comments: false,
                style: false,
            },
        );
        assert!(cap.contains("Display \"Hi\""), "{cap}");
        let leave = fmt_opts(
            "main-p.\n    dIsPlAy \"Hi\".\n",
            BeautifyOptions {
                verb_case: VerbCase::Leave,
                align_comments: false,
                style: false,
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
                style: false,
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

    // ── The PowerRustCOBOL layout style (operator, 2026-10-12) ────────────────

    const STYLE: BeautifyOptions = BeautifyOptions {
        verb_case: VerbCase::Upper,
        align_comments: true,
        style: true,
    };

    fn styled(text: &str) -> String {
        fmt_opts(text, STYLE)
    }

    /// The operator's own snippet, in the order they wrote it.
    const OPERATORS_SNIPPET: &str = concat!(
        "ENVIRONMENT DIVISION.\n",
        "       DATA DIVISION.\n",
        "       WORKING-STORAGE SECTION.\n",
        "       01 WS-NL PIC X.\n",
        "      \n",
        "       PROCEDURE DIVISION.\n",
        "           MOVE FUNCTION CHAR(11) TO WS-NL.\n",
        "           MOVE \"Page\" TO VWR-1::Layout.\n",
        "           MOVE \"None\" TO VWR-1::SplitMode.\n",
        "           MOVE \"Full\" TO VWR-1::View1ViewMode.\n",
        "           MOVE 100 TO VWR-1::View1Zoom.\n",
        "           MOVE 14 TO VWR-1::FontSize.\n",
        "      *> Something to look at before anything is picked. The sample\n",
        "      *> ships with the project, so the demo does not depend on where\n",
        "      *> the repository happens to sit, and it exercises headings,\n",
        "      *> tables, lists, quotes and fenced code all at once.\n",
        "           MOVE \"assets/docs/viewer-sample.md\" TO VWR-1::View1Source.\n",
        "           MOVE \"Ready. Pick a file, or press Start stream.\" TO Lbl-Status::Caption.\n",
        "       SETTING-VIEWER.    \n",
        "           INVOKE Txt-Log::AppendText(\"Viewer demo ready.\").\n",
        "           INVOKE Txt-Log::AppendText(WS-NL).\n",
    );

    #[test]
    fn the_style_formats_the_operators_snippet() {
        let expected = concat!(
            "       ENVIRONMENT DIVISION.\n",
            "\n",
            "       DATA DIVISION.\n",
            "\n",
            "       WORKING-STORAGE SECTION.\n",
            "       01 WS-NL                                  PIC    X.\n",
            "\n",
            "       PROCEDURE DIVISION.\n",
            "           SET     WS-NL                         TO    FUNCTION CHAR(11)\n",
            "           SET     VWR-1::Layout                 TO    \"Page\"\n",
            "           SET     VWR-1::SplitMode              TO    \"None\"\n",
            "           SET     VWR-1::View1ViewMode          TO    \"Full\"\n",
            "           SET     VWR-1::View1Zoom              TO    100\n",
            "           SET     VWR-1::FontSize               TO    14\n",
            "\n",
            "           *> Something to look at before anything is picked. The sample\n",
            "           *> ships with the project, so the demo does not depend on where\n",
            "           *> the repository happens to sit, and it exercises headings,\n",
            "           *> tables, lists, quotes and fenced code all at once.\n",
            "\n",
            "           SET     VWR-1::View1Source            TO    \"assets/docs/viewer-sample.md\"\n",
            "           SET     Lbl-Status::Caption           TO    \"Ready. Pick a file, or press Start stream.\".\n",
            "\n",
            "       SETTING-VIEWER.\n",
            "           Txt-Log::AppendText(\"Viewer demo ready.\")\n",
            "           Txt-Log::AppendText(WS-NL).\n",
        );
        assert_eq!(styled(OPERATORS_SNIPPET), expected);
    }

    #[test]
    fn the_style_is_a_fixed_point() {
        for src in [
            OPERATORS_SNIPPET,
            "MAIN-P.\nIF A = 1\nMOVE 1 TO B\nMOVE 2 TO C\nELSE\nDISPLAY \"X\"\nEND-IF.\n",
            "01 REC.\n05 A PIC X.\n05 LONG-NAME-OF-A-FIELD-THAT-NEEDS-ROOM-TO-BREATHE PIC X(10) VALUE \"A\".\n88 A-ON VALUE \"Y\".\n",
            "MAIN-P.\nMOVE \"a long literal that cannot reach column fifty\" TO WS-TARGET-NAME\nMOVE 1 TO B\n",
            "01 A PIC X.\n*> note\n01 B PIC X.\nMAIN-P.\nDISPLAY \"A\"\n*> first\n*> second\nDISPLAY \"B\"\nIF A = 1\n*> inside\nDISPLAY \"C\"\nEND-IF.\n",
        ] {
            let once = styled(src);
            assert_eq!(styled(&once), once, "not a fixed point for:\n{src}\n--- first pass ---\n{once}");
        }
    }

    // Rule 1 — 4 spaces per data level (spec 043 had 3), 88s one step under.
    #[test]
    fn style_data_levels_nest_four_spaces() {
        let out = styled("01 A.\n05 B PIC X.\n10 C PIC X.\n88 C-ON VALUE \"Y\".\n05 D PIC X.\n");
        let cols: Vec<usize> = out.lines().map(col_of).collect();
        assert_eq!(cols, [8, 12, 16, 20, 12], "{out}");
    }

    // The gutter — the first clause after the name starts in column 50.
    #[test]
    fn style_puts_the_first_data_clause_in_column_fifty() {
        let out = styled("01 REC.\n05 A PIC X.\n05 LONG-NAME-HERE PIC X(10) VALUE \"A\".\n05 T OCCURS 12 TIMES.\n88 A-ON VALUE \"Y\".\n");
        for line in out.lines().skip(1) {
            let first = line.split_whitespace().nth(2).unwrap_or("");
            let at = line.find(&format!(" {first}")).unwrap() + 2;
            if ["PIC", "OCCURS", "VALUE"].contains(&first) {
                assert_eq!(at, 50, "{line:?} in\n{out}");
            }
        }
        assert!(out.contains("PIC    X(10)    VALUE \"A\"."), "{out}");
        assert!(out.contains("OCCURS    12 TIMES."), "{out}");
    }

    #[test]
    fn style_sends_clauses_of_a_long_name_to_the_next_line() {
        let out = styled("01 G.\n05 T OCCURS 3.\n10 WS-G-CUSTOMER-STATUS-LINE-CAPTION PIC X(60).\n");
        let lines: Vec<&str> = out.lines().collect();
        let at = lines.iter().position(|l| l.contains("WS-G-CUSTOMER-STATUS-LINE-CAPTION")).unwrap();
        assert_eq!(col_of(lines[at + 1]), 50, "{out}");
        assert!(lines[at + 1].trim_start().starts_with("PIC    X(60)."), "{out}");
        assert!(lines.get(at + 2).is_none_or(|l| l.trim().is_empty()), "blank line after:\n{out}");
    }

    // Rule 2 — a blank line before every DIVISION except ENVIRONMENT DIVISION.
    #[test]
    fn style_puts_a_blank_line_before_divisions_but_not_environment() {
        let out = styled("IDENTIFICATION DIVISION.\nPROGRAM-ID. P.\nENVIRONMENT DIVISION.\nDATA DIVISION.\nPROCEDURE DIVISION.\nMAIN-P.\nDISPLAY \"A\".\n");
        let lines: Vec<&str> = out.lines().collect();
        for (i, l) in lines.iter().enumerate() {
            if l.trim() == "DATA DIVISION." || l.trim() == "PROCEDURE DIVISION." {
                assert!(lines[i - 1].trim().is_empty(), "no blank before {l:?}:\n{out}");
            }
            if l.trim() == "ENVIRONMENT DIVISION." {
                assert!(!lines[i - 1].trim().is_empty(), "blank before ENVIRONMENT:\n{out}");
            }
        }
    }

    // Rule 3 — a verb, 4 spaces, its predicate (SET takes 5).
    #[test]
    fn style_separates_verb_and_predicate_by_four_spaces() {
        let out = styled("MAIN-P.\nDISPLAY \"A\"\nADD 1 TO WS-N\nIF WS-N > 1\nPERFORM SHOW\nEND-IF.\nEXIT PROGRAM.\n");
        assert!(out.contains("DISPLAY    \"A\""), "{out}");
        assert!(out.contains("ADD    1 TO WS-N"), "{out}");
        assert!(out.contains("IF    WS-N > 1"), "{out}");
        assert!(out.contains("PERFORM    SHOW"), "{out}");
        assert!(out.contains("EXIT PROGRAM."), "a verb with no predicate is left alone:\n{out}");
    }

    // Rule 4 — TO in column 50; too long → column 50 of the next line + blank.
    #[test]
    fn style_puts_to_in_column_fifty() {
        let out = styled("MAIN-P.\nMOVE 1 TO WS-A\nMOVE ALL \"-\" TO WS-LINE\nADD 1 TO WS-N\n");
        for line in out.lines().filter(|l| l.trim_start().starts_with("SET") || l.trim_start().starts_with("MOVE")) {
            assert_eq!(line.find("TO").unwrap() + 1, 50, "{line:?}\n{out}");
        }
        assert!(out.contains("ADD    1 TO WS-N"), "only MOVE and SET are aligned:\n{out}");
    }

    #[test]
    fn style_moves_a_far_to_to_the_next_line_and_keeps_it_on_a_second_pass() {
        let src = "MAIN-P.\nMOVE CORRESPONDING WS-A-GROUP-WITH-A-LONG-NAME TO WS-B-GROUP\nDISPLAY \"NEXT\"\n";
        let out = styled(src);
        let lines: Vec<&str> = out.lines().collect();
        let at = lines.iter().position(|l| l.contains("CORRESPONDING")).unwrap();
        assert_eq!(col_of(lines[at + 1]), 50, "{out}");
        assert!(lines[at + 1].trim_start().starts_with("TO    WS-B-GROUP"), "{out}");
        assert!(lines[at + 2].trim().is_empty(), "blank line after the statement:\n{out}");
        assert_eq!(styled(&out), out);
    }

    // Rules 5 and 6 — a comment sits in the column of the code below it, and a
    // blank line separates the two.
    #[test]
    fn style_aligns_a_comment_with_the_code_below_and_blanks_after_it() {
        let out = styled("MAIN-P.\nIF A = 1\n*> why this is done\nDISPLAY \"X\"\nEND-IF.\n");
        let lines: Vec<&str> = out.lines().collect();
        let at = lines.iter().position(|l| l.contains("why this")).unwrap();
        assert_eq!(col_of(lines[at]), 16, "{out}");
        assert!(lines[at + 1].trim().is_empty(), "{out}");
        assert_eq!(col_of(lines[at + 2]), 16, "{out}");
    }

    // Rule 11 — a blank line before a comment block (2026-10-12), and never a
    // second one: not when the line above is already blank, not at the top.
    #[test]
    fn style_puts_a_blank_line_before_a_comment_block() {
        let expected = concat!(
            "       MAIN-P.\n",
            "           DISPLAY    \"A\"\n",
            "\n",
            "           *> first\n",
            "           *> second\n",
            "\n",
            "           DISPLAY    \"B\"\n",
        );
        assert_eq!(styled("MAIN-P.\nDISPLAY \"A\"\n*> first\n*> second\nDISPLAY \"B\"\n"), expected);
    }

    #[test]
    fn style_adds_no_second_blank_line_before_a_comment() {
        let expected = styled("MAIN-P.\nDISPLAY \"A\"\n*> why\nDISPLAY \"B\"\n");
        assert_eq!(styled("MAIN-P.\nDISPLAY \"A\"\n\n*> why\nDISPLAY \"B\"\n"), expected);
        assert_eq!(styled("MAIN-P.\nDISPLAY \"A\"\n\n\n\n*> why\nDISPLAY \"B\"\n"), expected);
        assert!(!expected.contains("\n\n\n"), "{expected}");
        // Nothing above a comment that opens the text, so nothing to separate it from.
        assert!(!styled("*> note\nMAIN-P.\nDISPLAY \"A\"\n").starts_with('\n'));
    }

    #[test]
    fn style_sets_a_comment_apart_wherever_it_follows_code() {
        // After a header and after a scope's first line as well as after a statement.
        let out = styled("MAIN-P.\n*> one\nIF A = 1\n*> two\nDISPLAY \"X\"\nEND-IF.\n");
        let lines: Vec<&str> = out.lines().collect();
        for word in ["one", "two"] {
            let at = lines.iter().position(|l| l.contains(word)).unwrap();
            assert!(lines[at - 1].trim().is_empty(), "no blank before {word:?}:\n{out}");
        }
        assert_eq!(styled(&out), out);
    }

    // Rule 7 — periods only where necessary; scopes closed by END-<verb>.
    #[test]
    fn style_drops_a_sentence_period_that_nothing_depends_on() {
        let out = styled("MAIN-P.\nDISPLAY \"A\".\nDISPLAY \"B\".\nNEXT-P.\nDISPLAY \"C\".\n");
        assert!(out.contains("DISPLAY    \"A\"\n"), "{out}");
        assert!(out.contains("DISPLAY    \"B\".\n"), "the period before a paragraph stays:\n{out}");
        assert!(out.trim_end().ends_with("DISPLAY    \"C\"."), "the last element keeps its period:\n{out}");
    }

    #[test]
    fn style_keeps_a_period_that_closes_a_scope() {
        // A one-line IF closed by its period: dropping it would pull the next
        // statement into the IF.
        let src = "MAIN-P.\nIF A = 1 DISPLAY \"X\".\nDISPLAY \"ALWAYS\".\nREAD F AT END MOVE 1 TO WS-EOF.\nDISPLAY \"AFTER\".\n";
        let out = styled(src);
        assert!(out.contains("IF    A = 1 DISPLAY \"X\"."), "{out}");
        assert!(out.contains("AT END MOVE 1 TO WS-EOF."), "{out}");
    }

    // Rule 8 — a blank line before and after a block of repeated verbs.
    #[test]
    fn style_sets_a_block_of_repeated_verbs_apart() {
        let out = styled("MAIN-P.\nDISPLAY \"A\"\nMOVE 1 TO X\nMOVE 2 TO Y\nMOVE 3 TO Z\nDISPLAY \"B\"\n");
        let lines: Vec<&str> = out.lines().collect();
        let first = lines.iter().position(|l| l.contains("SET     X")).unwrap();
        assert!(lines[first - 1].trim().is_empty(), "blank before the block:\n{out}");
        assert!(lines[first + 3].trim().is_empty(), "blank after the block:\n{out}");
        // A single statement is no block.
        assert!(!lines[first - 2].trim().is_empty(), "{out}");
    }

    #[test]
    fn style_does_not_blank_the_start_or_end_of_a_scope() {
        let out = styled("MAIN-P.\nIF A = 1\nMOVE 1 TO X\nMOVE 2 TO Y\nEND-IF.\n");
        assert!(!out.contains("IF    A = 1\n\n"), "{out}");
        assert!(!out.contains("\n\n           END-IF"), "{out}");
    }

    // Rule 9 — no INVOKE on a plain call; the other spellings are kept.
    #[test]
    fn style_drops_invoke_only_from_a_plain_call() {
        let out = styled("MAIN-P.\nINVOKE Txt-Log::AppendText(WS-NL)\nINVOKE ME::\"OpenFormSync\"(\"X\")\nINVOKE Obj::Get() RETURNING WS-V\n");
        assert!(out.contains("\n           Txt-Log::AppendText(WS-NL)"), "{out}");
        assert!(out.contains("INVOKE    ME::\"OpenFormSync\"(\"X\")"), "{out}");
        assert!(out.contains("RETURNING WS-V"), "{out}");
    }

    // Rule 10 — SET in place of MOVE, except where SET has no equal.
    #[test]
    fn style_prefers_set_to_move() {
        let out = styled("MAIN-P.\nMOVE WS-A TO WS-B\nMOVE CORRESPONDING G1 TO G2\nMOVE ALL \"*\" TO WS-LINE\n");
        assert!(out.contains("SET     WS-B"), "{out}");
        assert!(out.contains("MOVE    CORRESPONDING"), "{out}");
        assert!(out.contains("MOVE    ALL"), "{out}");
    }

    // With the style off, nothing of the above happens.
    #[test]
    fn without_the_style_the_old_rules_hold() {
        let out = fmt("MAIN-P.\n    MOVE 1 TO X.\n    INVOKE Txt::Go().\n");
        assert!(out.contains("MOVE 1 TO X."), "{out}");
        assert!(out.contains("INVOKE Txt::Go()."), "a member name keeps its spelling:\n{out}");
    }

    // The token-changing rules are kept only when the program is unchanged.
    #[test]
    fn style_falls_back_to_layout_alone_when_it_cannot_prove_the_program_unchanged() {
        // Not parseable as a whole, so no rewrite can be proved: layout only.
        let out = styled("MAIN-P.\nMOVE 1 TO X.\nDISPLAY \"A\" +\nDISPLAY \"B\".\n");
        assert!(out.contains("MOVE    1"), "the MOVE stays a MOVE:\n{out}");
        assert!(out.contains("X."), "and its period stays:\n{out}");
    }

    // A bare call is not a paragraph header, with or without the style.
    #[test]
    fn a_lone_call_ending_in_a_period_is_no_paragraph() {
        let out = fmt("MAIN-P.\n    DISPLAY \"A\"\n    Txt-Log::AppendText(WS-NL).\n");
        let call = out.lines().find(|l| l.contains("AppendText")).unwrap();
        assert_eq!(col_of(call), 12, "{out}");
        assert!(out.contains("DISPLAY \"A\"\n"), "no period was added before it:\n{out}");
    }

    // A statement with conditional phrases is a scope: its phrases sit one level
    // in, their bodies two, and `END-READ` comes back to the verb's column.
    #[test]
    fn style_indents_a_phrase_statement_like_a_scope() {
        let src = "MAIN-P.\nREAD CONVS-FILE\nINVALID KEY CONTINUE\nNOT INVALID KEY\nADD WS-A TO WS-B\nIF WS-B > 1\nDISPLAY \"BIG\"\nEND-IF\nEND-READ\nCLOSE CONVS-FILE\n";
        let out = styled(src);
        let col = |needle: &str| col_of(out.lines().find(|l| l.contains(needle)).unwrap_or_else(|| panic!("{needle} in\n{out}")));
        assert_eq!(col("READ    CONVS"), 12, "{out}");
        assert_eq!(col("INVALID KEY CONTINUE"), 16, "{out}");
        assert_eq!(col("NOT INVALID KEY"), 16, "{out}");
        assert_eq!(col("ADD    WS-A"), 20, "{out}");
        assert_eq!(col("IF    WS-B"), 20, "{out}");
        assert_eq!(col("DISPLAY"), 24, "{out}");
        assert_eq!(col("END-READ"), 12, "{out}");
        assert_eq!(col("CLOSE"), 12, "the statement after the scope is back at the verb's column:\n{out}");
        assert_eq!(styled(&out), out, "fixed point");
    }

    #[test]
    fn style_indents_the_continuation_lines_of_a_plain_statement() {
        let out = styled("MAIN-P.\nSTRING WS-A DELIMITED BY SIZE\nINTO WS-B\nADD 1 TO WS-N\n");
        let col = |needle: &str| col_of(out.lines().find(|l| l.contains(needle)).unwrap());
        assert_eq!(col("STRING    WS-A"), 12, "{out}");
        assert_eq!(col("INTO WS-B"), 16, "{out}");
        assert_eq!(col("ADD"), 12, "{out}");
    }

    #[test]
    fn style_sets_a_condition_continuation_clear_of_the_body() {
        let out = styled("MAIN-P.\nIF WS-A = 1\nAND WS-B = 2\nDISPLAY \"BOTH\"\nEND-IF\n");
        let col = |needle: &str| col_of(out.lines().find(|l| l.contains(needle)).unwrap());
        assert_eq!(col("IF    WS-A"), 12, "{out}");
        assert_eq!(col("AND WS-B"), 20, "{out}");
        assert_eq!(col("DISPLAY"), 16, "{out}");
    }

    // Spec 093 R13 — `A = B * C` is a COMPUTE written without its verb. It is laid out as a statement of
    // its own, its size-error phrases sit one level in, and `END-COMPUTE` comes back under it.
    #[test]
    fn style_lays_out_the_assignment_shorthand_like_a_compute() {
        let src = "MAIN-P.\nIF A > 1\nA = B * C\nD ROUNDED = A / 3\nON SIZE ERROR\nDISPLAY \"X\"\nEND-COMPUTE\nCOMPUTE E = A + 1\nEND-IF.\nF = 1.\nDISPLAY \"Z\".\n";
        let out = styled(src);
        println!("{out}");
        let col = |needle: &str| col_of(out.lines().find(|l| l.contains(needle)).unwrap_or_else(|| panic!("{needle} in\n{out}")));
        assert_eq!(col("A = B * C"), 16, "a statement in the IF's body:\n{out}");
        assert_eq!(col("D ROUNDED = A / 3"), 16, "{out}");
        assert_eq!(col("ON SIZE ERROR"), 20, "the phrase sits one level in:\n{out}");
        assert_eq!(col("DISPLAY"), 24, "{out}");
        assert_eq!(col("END-COMPUTE"), 16, "END-COMPUTE comes back under its statement:\n{out}");
        assert_eq!(col("COMPUTE    E"), 16, "{out}");
        assert_eq!(col("F = 1"), 12, "a statement of its own after the IF closed:\n{out}");
        assert_eq!(col("DISPLAY    \"Z\""), 12, "{out}");
        assert_eq!(styled(&out), out, "a second pass changes nothing");
    }

    // The line above decides: the second line of a condition begins like an assignment and is not one.
    #[test]
    fn style_does_not_mistake_the_second_line_of_a_condition_for_an_assignment() {
        let out = styled("MAIN-P.\nIF A > 1 AND\nB = 2\nDISPLAY \"BOTH\"\nEND-IF\n");
        let col = |needle: &str| col_of(out.lines().find(|l| l.contains(needle)).unwrap());
        assert_eq!(col("IF    A > 1 AND"), 12, "{out}");
        assert!(col("B = 2") > 12, "the continuation sits in from its header:\n{out}");
        assert_eq!(col("DISPLAY"), 16, "{out}");
    }

    // An operand list ends in front of an assignment: `MOVE 1 TO A` / `B = 2` are two statements, and the
    // second is not one more target of the MOVE.
    #[test]
    fn style_does_not_take_an_assignment_for_one_more_move_target() {
        let out = styled("MAIN-P.\nMOVE 1 TO WS-A\nWS-B = 2\nMOVE 3 TO WS-C\n   WS-D\nDISPLAY \"X\"\n");
        assert!(out.contains("WS-B = 2"), "{out}");
        assert!(!out.contains("WS-A WS-B"), "the assignment was joined to the MOVE:\n{out}");
        assert!(out.contains("WS-C WS-D"), "a real extra target still joins:\n{out}");
    }

    // A RETURNING clause on the next line belongs to its INVOKE: with it the
    // call is not a plain one, and INVOKE stays.
    #[test]
    fn style_keeps_invoke_when_returning_is_on_the_next_line() {
        let out = styled("MAIN-P.\nINVOKE Obj::GetProperty(\"Answer\")\nRETURNING WS-ANSWER\n");
        assert!(out.contains("INVOKE    Obj::GetProperty(\"Answer\") RETURNING WS-ANSWER"), "{out}");
    }

    // A multi-line MOVE keeps all its targets.
    #[test]
    fn style_keeps_every_target_of_a_multi_line_move() {
        let out = styled("MAIN-P.\nMOVE WS-A TO WS-B\n   WS-C\n   WS-D\nDISPLAY \"X\"\n");
        assert!(out.contains("SET     WS-B WS-C WS-D"), "{out}");
        assert!(out.contains("TO    WS-A"), "{out}");
    }

    // Member names keep their spelling whatever the verb casing.
    #[test]
    fn member_names_are_never_recased() {
        let out = fmt("MAIN-P.\n    move nud-age::Value to lbl-1::Text\n    Timer-1::Start()\n");
        assert!(out.contains("nud-age::Value"), "{out}");
        assert!(out.contains("lbl-1::Text"), "{out}");
        assert!(out.contains("Timer-1::Start()"), "{out}");
        let out = fmt("MAIN-P.\n    invoke super::GetProperty(\"A\") returning ws-x\n    me::Close()\n");
        assert!(out.contains("super::GetProperty"), "a receiver keeps its spelling:\n{out}");
        assert!(out.contains("me::Close()"), "{out}");
    }

    // The blank line after a wrapped `TO` is for what follows, not for the end
    // of a scope.
    #[test]
    fn style_leaves_no_blank_line_between_a_wrapped_to_and_a_scope_end() {
        let src = "MAIN-P.\nIF A = 1\nMOVE CORRESPONDING WS-A-GROUP-WITH-A-LONG-NAME TO WS-B-GROUP\nEND-IF\nDISPLAY \"X\"\n";
        let out = styled(src);
        let lines: Vec<&str> = out.lines().collect();
        let at = lines.iter().position(|l| l.trim_start().starts_with("TO    WS-B-GROUP")).unwrap();
        assert_eq!(lines[at + 1].trim(), "END-IF", "{out}");
    }

    #[test]
    fn style_indents_the_clauses_of_a_select() {
        let out = styled("ENVIRONMENT DIVISION.\nINPUT-OUTPUT SECTION.\nFILE-CONTROL.\nSELECT F ASSIGN TO WS-P\nORGANIZATION IS INDEXED\nRECORD KEY IS K.\nSELECT G ASSIGN TO WS-Q.\n");
        let col = |needle: &str| col_of(out.lines().find(|l| l.contains(needle)).unwrap());
        assert_eq!(col("SELECT F"), 12, "{out}");
        assert_eq!(col("ORGANIZATION"), 16, "{out}");
        assert_eq!(col("RECORD KEY"), 16, "{out}");
        assert_eq!(col("SELECT G"), 12, "{out}");
        assert_eq!(styled(&out), out);
    }

    // A long VALUE keeps its literal whole: the line breaks before it.
    #[test]
    fn style_breaks_before_a_long_literal_instead_of_through_it() {
        let literal = format!("\"{}\"", "abc ".repeat(55).trim_end());
        let out = styled(&format!("01 WS-TYPES GLOBAL PIC X(300) VALUE {literal}.\n"));
        assert!(out.lines().all(|l| l.chars().count() <= 256), "{out}");
        assert!(out.contains(&literal), "the literal must be unbroken:\n{out}");
        assert!(!out.lines().any(|l| l.as_bytes().get(6) == Some(&b'-')), "no continuation line:\n{out}");
    }

    // The plugin tells a coding agent to write this layout, and shows a model
    // of it. ✨ Beautify must leave that model exactly as it is — one layout, in
    // the rule and in the IDE.
    #[test]
    fn beautify_leaves_the_plugins_model_untouched() {
        let (_, rule) = cobolt_project_tools::content::APPLICATION_RULES
            .iter()
            .find(|(id, _)| *id == "layout")
            .expect("a layout rule");
        let model = rule.split_once("The model:\n").expect("a model").1;
        assert_eq!(styled(model), model);
    }
}
