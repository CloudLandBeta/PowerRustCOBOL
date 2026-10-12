// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! The PowerRustCOBOL layout style for ✨ Beautify (spec 043 addendum,
//! operator, 2026-10-12).
//!
//! Switched on by [`BeautifyOptions::style`](super::beautify::BeautifyOptions);
//! with it off the engine behaves exactly as spec 043 dictated. What the style
//! adds, rule by rule (the numbers are the operator's):
//!
//! 1. 4 spaces per nesting level — data levels too (spec 043 had 3).
//! 2. A blank line before every DIVISION and SECTION, except ENVIRONMENT
//!    DIVISION.
//! 3. 4 spaces between a verb and its predicate (`SET` takes 5, so its
//!    predicate lines up with `MOVE`'s).
//! 4. `TO` of `MOVE`/`SET` in column 50; when the operand before it is too long
//!    it goes to column 50 of the next line, and a blank line follows.
//! 5. A comment sits in the column of the code below it …
//! 6. … and a blank line separates it from that code.
//! 7. A sentence period is written only before a SECTION / paragraph and at the
//!    end of the code; scopes are closed by their `END-<verb>`.
//! 8. A blank line before and after a block of repeated verbs.
//! 9. `INVOKE obj::method(…)` is written `obj::method(…)`.
//! 10. `SET` in place of `MOVE`.
//!
//! plus, for data entries, the first clause after the name (`PIC`, `OCCURS`,
//! `VALUE` of an 88, …) starts in column 50 — the same gutter as `TO`.
//!
//! Rules 7, 9 and 10 change tokens, not just whitespace. They are only kept
//! when [`programs_equivalent`] proves the result parses to the same program.

use super::beautify::{Kind, Out};

/// Column (1-based) of `TO` in `MOVE`/`SET`, and of the first clause of a data
/// entry.
const GUTTER: usize = 50;

/// Verbs that start a statement. Rule 3 applies to these; so does the
/// repeated-verb rule 8.
pub(super) const STATEMENT_VERBS: &[&str] = &[
    "ACCEPT", "ADD", "ALTER", "CALL", "CANCEL", "CLOSE", "COMMIT", "COMPUTE", "CONTINUE",
    "DELETE", "DISPLAY", "DIVIDE", "EVALUATE", "EXIT", "GO", "GOBACK", "IF", "INITIALIZE",
    "INSPECT", "INVOKE", "MERGE", "MOVE", "MULTIPLY", "OPEN", "PERFORM", "READ", "RELEASE",
    "RETURN", "REWRITE", "ROLLBACK", "SEARCH", "SET", "SORT", "START", "STOP", "STRING",
    "SUBTRACT", "THROW", "TRY", "UNLOCK", "UNSTRING", "WRITE",
];

/// Words that open a conditional phrase of a statement (`AT END`, `INVALID
/// KEY`, …). A sentence period after one of them closes it, so the period must
/// stay unless an explicit `END-<verb>` does.
pub(super) const PHRASE_WORDS: &[&str] = &["AT END", "INVALID KEY", "SIZE ERROR", "OVERFLOW", "EXCEPTION"];

// ── Scanning ──────────────────────────────────────────────────────────────────

/// Byte ranges of the words of `s` at the top level — whitespace inside a
/// string literal or inside parentheses does not split a word.
pub(super) fn top_level_words(s: &str) -> Vec<(usize, usize)> {
    let mut words = Vec::new();
    let mut start: Option<usize> = None;
    let mut quote: Option<char> = None;
    let mut depth = 0usize;
    for (i, c) in s.char_indices() {
        if let Some(q) = quote {
            if c == q {
                quote = None;
            }
            continue;
        }
        match c {
            '"' | '\'' => {
                quote = Some(c);
                start.get_or_insert(i);
            }
            '(' => {
                depth += 1;
                start.get_or_insert(i);
            }
            ')' => {
                depth = depth.saturating_sub(1);
                start.get_or_insert(i);
            }
            c if c.is_whitespace() && depth == 0 => {
                if let Some(st) = start.take() {
                    words.push((st, i));
                }
            }
            _ => {
                start.get_or_insert(i);
            }
        }
    }
    if let Some(st) = start {
        words.push((st, s.len()));
    }
    words
}

fn upper_word(s: &str, range: (usize, usize)) -> String {
    s[range.0..range.1].trim_end_matches('.').to_ascii_uppercase()
}

/// The statement verb a line starts with, as a key for rule 8 — `"INVOKE"` for
/// a bare `object::method(…)` call. `None` for continuation lines and markers.
pub(super) fn statement_verb(content: &str) -> Option<&'static str> {
    let words = top_level_words(content);
    let first = words.first()?;
    let w = upper_word(content, *first);
    if let Some(v) = STATEMENT_VERBS.iter().find(|v| **v == w) {
        return Some(v);
    }
    // `Txt-Log::AppendText(…)` and `COBOL::"NAME" ( … )`: a call on its own.
    if content[first.0..first.1].contains("::") {
        return Some("INVOKE");
    }
    None
}

/// Number of words that make up the verb: `EXIT PERFORM`, `STOP RUN` and
/// `GO TO` are one verb of two words.
fn verb_words(content: &str, words: &[(usize, usize)]) -> usize {
    let first = upper_word(content, words[0]);
    let second = words.get(1).map(|w| upper_word(content, *w)).unwrap_or_default();
    match (first.as_str(), second.as_str()) {
        ("EXIT", "PERFORM" | "PARAGRAPH" | "SECTION" | "PROGRAM") => 2,
        ("STOP", "RUN") | ("GO", "TO") => 2,
        _ => 1,
    }
}

fn is_word(content: &str, range: (usize, usize), what: &str) -> bool {
    content[range.0..range.1].eq_ignore_ascii_case(what)
}

/// A `MOVE`/`SET` with no `TO` yet: its `TO` is on the next line.
pub(super) fn lacks_to(content: &str) -> bool {
    let words = top_level_words(content);
    let Some(&first) = words.first() else { return false };
    let verb = upper_word(content, first);
    (verb == "MOVE" || verb == "SET") && !words.iter().skip(1).any(|w| is_word(content, *w, "TO"))
}

/// Is `next` a further target of the `MOVE`/`SET` in `prev` (`MOVE A TO B` /
/// `C` / `D`)? Only a line that cannot start anything else qualifies: it is not
/// a verb, a scope marker, a call, or a bare name that would be a paragraph.
pub(super) fn continues_targets(prev: &str, next: &str) -> bool {
    let words = top_level_words(prev);
    let Some(&first) = words.first() else { return false };
    let verb = upper_word(prev, first);
    if (verb != "MOVE" && verb != "SET") || lacks_to(prev) || prev.trim_end().ends_with('.') {
        return false;
    }
    let nw = top_level_words(next);
    let Some(&head) = nw.first() else { return false };
    let w = next[head.0..head.1].trim_end_matches('.').to_ascii_uppercase();
    let plain = w.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_');
    let name_like = w.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | ':'));
    !w.is_empty()
        && name_like
        && !STATEMENT_VERBS.contains(&w.as_str())
        && !matches!(w.as_str(), "ELSE" | "WHEN" | "CATCH" | "FINALLY" | "OTHER" | "NOT" | "AND" | "OR")
        && !w.starts_with("END-")
        // `NEXT-P.` alone is a paragraph header, not a target.
        && !(plain && next[head.0..head.1].ends_with('.') && nw.len() == 1)
}

/// `INVOKE obj::m(a)` / `RETURNING x` is one statement: join the clause so the
/// call is judged with it.
pub(super) fn continues_invoke(prev: &str, next: &str) -> bool {
    let words = top_level_words(prev);
    let Some(&first) = words.first() else { return false };
    let nw = top_level_words(next);
    let Some(&head) = nw.first() else { return false };
    upper_word(prev, first) == "INVOKE"
        && !prev.trim_end().ends_with('.')
        && matches!(upper_word(next, head).as_str(), "RETURNING" | "USING")
}

// ── Statement rewrites (rules 9 and 10 — token-changing) ──────────────────────

/// `INVOKE obj::method(…)` → `obj::method(…)` (rule 9). Only a plain call on a
/// control is touched: no `RETURNING`/`USING`, no quoted method name, no chain
/// after the call, no space form — and not a call on the window itself (`ME`,
/// `SUPER`), because the window supervisor runs `INVOKE ME::OpenFormSync(…)`
/// and its answer through a path the bare statement does not take.
fn drop_invoke(content: &str) -> Option<String> {
    let words = top_level_words(content);
    if words.len() < 2 || !is_word(content, words[0], "INVOKE") {
        return None;
    }
    let target = &content[words[1].0..words[1].1];
    let (object, member) = target.split_once("::")?;
    if matches!(object.to_ascii_uppercase().as_str(), "ME" | "SUPER") {
        return None;
    }
    let name = member.split('(').next().unwrap_or("");
    let ident = |s: &str| !s.is_empty() && s.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_');
    // `name(` … the matching `)`, and nothing after it but a period.
    let call_ends_at = (|| {
        let mut depth = 0usize;
        let mut quote: Option<char> = None;
        for (i, c) in member.char_indices().skip(name.len()) {
            match (quote, c) {
                (Some(q), c) if c == q => quote = None,
                (Some(_), _) => {}
                (None, '"' | '\'') => quote = Some(c),
                (None, '(') => depth += 1,
                (None, ')') => {
                    depth = depth.checked_sub(1)?;
                    if depth == 0 {
                        return Some(i + 1);
                    }
                }
                _ => {}
            }
        }
        None
    })();
    let plain = ident(object)
        && ident(name)
        && call_ends_at.is_some_and(|end| matches!(member[end..].trim(), "" | "."));
    let clauses = words[2..]
        .iter()
        .any(|w| is_word(content, *w, "RETURNING") || is_word(content, *w, "USING"));
    (plain && !clauses).then(|| content[words[1].0..].to_owned())
}

/// `MOVE src TO tgt` → `SET tgt TO src` (rule 10). The parser lowers both to
/// the same node, so the result is a pure respelling; `CORRESPONDING` has no
/// `SET` form and is left alone.
fn move_to_set(content: &str) -> Option<String> {
    let words = top_level_words(content);
    if words.len() < 4 || !is_word(content, words[0], "MOVE") {
        return None;
    }
    let to = words.iter().position(|w| is_word(content, *w, "TO"))?;
    if to < 2 || to + 1 >= words.len() {
        return None;
    }
    let first = upper_word(content, words[1]);
    if matches!(first.as_str(), "CORRESPONDING" | "CORR" | "ALL") {
        return None;
    }
    let src = content[words[1].0..words[to].0].trim();
    let mut tgt = content[words[to + 1].0..].trim().to_owned();
    // The sentence period belongs to the whole statement; keep it at the end.
    let ended = tgt.ends_with('.');
    if ended {
        tgt.pop();
    }
    let mut set = String::new();
    set.push_str(&match_case(&content[words[0].0..words[0].1], "SET"));
    set.push(' ');
    set.push_str(tgt.trim_end());
    set.push(' ');
    set.push_str(&match_case(&content[words[to].0..words[to].1], "TO"));
    set.push(' ');
    set.push_str(src);
    if ended {
        set.push('.');
    }
    Some(set)
}

/// Spell `word` in the case `like` is written in (all upper, all lower, or
/// capitalised).
fn match_case(like: &str, word: &str) -> String {
    if like.chars().all(|c| !c.is_ascii_lowercase()) {
        word.to_ascii_uppercase()
    } else if like.chars().all(|c| !c.is_ascii_uppercase()) {
        word.to_ascii_lowercase()
    } else {
        let lower = word.to_ascii_lowercase();
        let mut c = lower.chars();
        c.next().map(|f| f.to_ascii_uppercase().to_string() + c.as_str()).unwrap_or(lower)
    }
}

/// Rule 3: the verb, then 4 spaces (5 for `SET`), then the predicate.
fn space_verb(content: &str) -> String {
    let words = top_level_words(content);
    if words.is_empty() {
        return content.to_owned();
    }
    let verb = upper_word(content, words[0]);
    if !STATEMENT_VERBS.contains(&verb.as_str()) {
        return content.to_owned();
    }
    let n = verb_words(content, &words);
    let end = words[n - 1].1;
    let rest = content[end..].trim_start();
    if rest.is_empty() || rest == "." || n < words.len() && rest.starts_with('.') {
        return content.to_owned();
    }
    let gap = if verb == "SET" { 5 } else { 4 };
    format!("{}{}{}", &content[..end], " ".repeat(gap), rest)
}

/// Apply the style's statement rules to one procedure line. `rewrite` allows
/// the token-changing ones (rules 9 and 10); spacing (rule 3) is whitespace.
pub(super) fn restyle(content: &str, rewrite: bool) -> String {
    let mut text = content.to_owned();
    if rewrite {
        if let Some(t) = drop_invoke(&text) {
            text = t;
        }
        if let Some(t) = move_to_set(&text) {
            text = t;
        }
    }
    space_verb(&text)
}

// ── Column 50 (rule 4) ────────────────────────────────────────────────────────

fn width(s: &str) -> usize {
    s.chars().count()
}

/// Lay out `text` starting in column `col`. A `MOVE`/`SET` with a `TO` puts the
/// `TO` in column 50 (rule 4); anything else is a single line.
///
/// Returns `(line, is_continuation)` pairs, and `true` when a blank line must
/// follow because `TO` had to go to the next line.
pub(super) fn place(col: usize, text: &str) -> (Vec<(String, bool)>, bool) {
    let indent = " ".repeat(col.saturating_sub(1));
    let single = (vec![(format!("{indent}{text}"), false)], false);
    let words = top_level_words(text);
    let Some(&first) = words.first() else { return single };
    let verb = upper_word(text, first);
    if verb != "MOVE" && verb != "SET" {
        return single;
    }
    let Some(to) = words.iter().skip(2).position(|w| is_word(text, *w, "TO")).map(|p| p + 2) else {
        return single;
    };
    if to + 1 >= words.len() {
        return single;
    }
    let head = text[..words[to].0].trim_end();
    let tail = text[words[to].0..].trim_start();
    let tail = {
        // `TO` + 4 spaces + the operand(s).
        let (kw, rest) = tail.split_at(2);
        format!("{kw}    {}", rest.trim_start())
    };
    let used = width(&indent) + width(head);
    if used < GUTTER - 1 {
        let mut line = format!("{indent}{head}");
        line.push_str(&" ".repeat(GUTTER - 1 - used));
        line.push_str(&tail);
        (vec![(line, false)], false)
    } else {
        (
            vec![
                (format!("{indent}{head}"), false),
                (format!("{}{}", " ".repeat(GUTTER - 1), tail), true),
            ],
            true,
        )
    }
}

// ── Data entries (the gutter, for PIC) ────────────────────────────────────────

/// A clause of a data entry begins at one of these words.
fn starts_clause(word: &str, next: Option<&str>) -> bool {
    let w = word.to_ascii_uppercase();
    match w.as_str() {
        "PIC" | "PICTURE" | "VALUE" | "VALUES" | "OCCURS" | "REDEFINES" | "USAGE" | "BINARY"
        | "PACKED-DECIMAL" | "INDEX" | "POINTER" | "SYNC" | "SYNCHRONIZED" | "JUST" | "JUSTIFIED"
        | "BLANK" | "SIGN" | "EXTERNAL" | "GLOBAL" | "RENAMES" => true,
        "IS" => next.is_some_and(|n| matches!(n.to_ascii_uppercase().as_str(), "GLOBAL" | "EXTERNAL")),
        _ => {
            w.starts_with("COMP-") || w == "COMP" || w == "COMPUTATIONAL" || w.starts_with("COMPUTATIONAL-")
        }
    }
}

/// Lay out one data entry: level, name, then its clauses. The first clause
/// starts in column 50 — `KEYWORD`, 4 spaces, its operand, like `TO` — and
/// every later clause follows after 4 spaces. A name too long to reach the
/// gutter sends the clauses to column 50 of the next line, with a blank line
/// after. `body` is the entry without its period.
pub(super) fn data_entry(col: usize, body: &str, period: bool) -> (Vec<String>, bool) {
    let words = top_level_words(body);
    let text: Vec<&str> = words.iter().map(|(s, e)| &body[*s..*e]).collect();
    let indent = " ".repeat(col.saturating_sub(1));
    if text.is_empty() {
        return (vec![format!("{indent}{}", if period { "." } else { "" })], false);
    }
    // level [name] — the name is whatever follows the level, unless that is
    // already a clause (an unnamed FILLER).
    let mut head = text[0].to_owned();
    let mut i = 1;
    if i < text.len() && !starts_clause(text[i], text.get(i + 1).copied()) {
        head.push(' ');
        head.push_str(text[i]);
        i += 1;
    }
    // Group the rest into clauses.
    let mut clauses: Vec<Vec<&str>> = Vec::new();
    while i < text.len() {
        if clauses.is_empty() || starts_clause(text[i], text.get(i + 1).copied()) {
            clauses.push(vec![text[i]]);
        } else if let Some(c) = clauses.last_mut() {
            c.push(text[i]);
        }
        i += 1;
    }
    let dot = if period { "." } else { "" };
    if clauses.is_empty() {
        return (vec![format!("{indent}{head}{dot}")], false);
    }
    // First clause: keyword, 4 spaces, operand. `IS GLOBAL` / `IS EXTERNAL`
    // are the keyword.
    let first = &clauses[0];
    let kw_len = if first[0].eq_ignore_ascii_case("IS") && first.len() > 1 { 2 } else { 1 };
    let kw = first[..kw_len].join(" ");
    let operand = first[kw_len..].join(" ");
    let mut tail = if operand.is_empty() { kw } else { format!("{kw}    {operand}") };
    for c in &clauses[1..] {
        tail.push_str("    ");
        tail.push_str(&c.join(" "));
    }
    tail.push_str(dot);
    let used = width(&indent) + width(&head);
    if used < GUTTER - 1 {
        let pad = " ".repeat(GUTTER - 1 - used);
        (vec![format!("{indent}{head}{pad}{tail}")], false)
    } else {
        (vec![format!("{indent}{head}"), format!("{}{tail}", " ".repeat(GUTTER - 1))], true)
    }
}

// ── The layout post-pass (rules 2, 5, 6 and 8) ────────────────────────────────

fn col_of(line: &str) -> usize {
    line.chars().take_while(|c| *c == ' ').count() + 1
}

/// Place comments, add the blank lines the style asks for, and never leave two
/// in a row. Runs on the whole output before long lines are wrapped.
pub(super) fn finish(out: Out) -> Out {
    let Out { mut lines, kinds } = out;
    let n = lines.len();
    let mut blank_before = vec![false; n];
    let mut blank_after = vec![false; n];

    // Rules 5 and 6 — a comment takes the column of the code below it, and a
    // blank line separates it from that code.
    let mut i = 0;
    while i < n {
        if kinds[i] != Kind::Comment {
            i += 1;
            continue;
        }
        let start = i;
        while i < n && kinds[i] == Kind::Comment {
            i += 1;
        }
        let below = (i..n).find(|&k| !matches!(kinds[k], Kind::Blank | Kind::Gap | Kind::Comment | Kind::Pinned));
        let col = match below {
            Some(k) => col_of(&lines[k]),
            None => (0..start)
                .rev()
                .find(|&k| !matches!(kinds[k], Kind::Blank | Kind::Gap | Kind::Comment | Kind::Pinned))
                .map(|k| col_of(&lines[k]))
                .unwrap_or(12),
        };
        for k in start..i {
            let text = lines[k].trim().to_owned();
            lines[k] = format!("{}{}", " ".repeat(col - 1), text);
        }
        // Not before a scope marker (`END-IF`, `ELSE`, …): that is no code.
        if i < n && !matches!(kinds[i], Kind::Blank | Kind::Gap | Kind::Marker { .. }) {
            blank_after[i - 1] = true;
        }
    }

    // Rule 2 — a blank line before every DIVISION except ENVIRONMENT DIVISION.
    for k in 1..n {
        if kinds[k] == Kind::Division && !lines[k].trim_start().to_ascii_uppercase().starts_with("ENVIRONMENT") {
            blank_before[k] = true;
        }
    }

    // Rule 8 — a blank line before and after a block of repeated verbs.
    let mut k = 0;
    while k < n {
        let Kind::Stmt { depth, verb } = kinds[k] else {
            k += 1;
            continue;
        };
        let extent = |from: usize| {
            let mut e = from + 1;
            while e < n && matches!(kinds[e], Kind::Cont { .. }) {
                e += 1;
            }
            e
        };
        let mut end = extent(k);
        let mut count = 1;
        while end < n {
            match kinds[end] {
                Kind::Stmt { depth: d, verb: v } if d == depth && v == verb => {
                    count += 1;
                    end = extent(end);
                }
                _ => break,
            }
        }
        if count >= 2 {
            if k > 0 {
                let sibling = match kinds[k - 1] {
                    Kind::Stmt { depth: d, .. } | Kind::Cont { depth: d } | Kind::Marker { depth: d } => d == depth,
                    _ => false,
                };
                if sibling {
                    blank_before[k] = true;
                }
            }
            if end < n {
                let follows = match kinds[end] {
                    Kind::Stmt { depth: d, .. } => d == depth,
                    Kind::Comment | Kind::Pinned | Kind::Paragraph => true,
                    _ => false,
                };
                if follows {
                    blank_after[end - 1] = true;
                }
            }
        }
        k = end;
    }

    let mut done = Out::new();
    for k in 0..n {
        if blank_before[k] && !done.kinds.last().is_none_or(|x| *x == Kind::Blank) {
            done.push(String::new(), Kind::Blank);
        }
        // A gap is a blank line unless the next line closes a scope or the code
        // ends there.
        let kind = if kinds[k] == Kind::Gap {
            let next = (k + 1..n).map(|m| kinds[m]).find(|x| !matches!(x, Kind::Blank | Kind::Gap));
            if next.is_none_or(|x| matches!(x, Kind::Marker { .. })) {
                continue;
            }
            Kind::Blank
        } else {
            kinds[k]
        };
        if kind == Kind::Blank && done.kinds.last().is_none_or(|x| *x == Kind::Blank) {
            continue; // never two blank lines, and none at the very top
        }
        done.push(lines[k].clone(), kind);
        if blank_after[k] {
            done.push(String::new(), Kind::Blank);
        }
    }
    done
}

// ── The safety gate for rules 7, 9 and 10 ─────────────────────────────────────

/// The ways `text` can be read as a whole program. A whole text is itself; a
/// fragment gets a program header, and — if it has no PROCEDURE DIVISION, as an
/// event handler that starts at a paragraph does not — a division header too.
fn readings(text: &str) -> Vec<String> {
    let upper = text.to_ascii_uppercase();
    if upper.contains("IDENTIFICATION DIVISION") || upper.contains("ID DIVISION") {
        return vec![text.to_owned()];
    }
    let head = "       IDENTIFICATION DIVISION.\n       PROGRAM-ID. BEAUTIFY-CHECK.\n";
    let mut all = vec![format!("{head}{text}")];
    if !upper.contains("PROCEDURE DIVISION") {
        all.push(format!("{head}       PROCEDURE DIVISION.\n{text}"));
    }
    all
}

/// The program `source` parses to, as JSON with positions and sentence-end
/// markers removed. `None` if it does not parse cleanly in either source format
/// (the editor's text is column-fluid, like the error gate's).
fn normalized_program(source: &str) -> Option<serde_json::Value> {
    let detected = crate::runner::detect_format(source);
    let other = match detected {
        cobolt_lexer::SourceFormat::Fixed => cobolt_lexer::SourceFormat::Free,
        _ => cobolt_lexer::SourceFormat::Fixed,
    };
    for format in [detected, other] {
        let parsed = cobolt_parser::parse(cobolt_lexer::tokenize(source, format));
        if parsed.diagnostics.iter().any(|d| d.severity == cobolt_parser::Severity::Error) {
            continue;
        }
        let Some(program) = parsed.program else { continue };
        let mut value = serde_json::to_value(program).ok()?;
        strip_positions(&mut value);
        return Some(value);
    }
    None
}

/// A `Span` serializes as an object of exactly these four keys; it appears both
/// under a `span` field and as a bare element of a tuple variant.
fn is_span(map: &serde_json::Map<String, serde_json::Value>) -> bool {
    map.len() == 4 && ["start", "end", "line", "col"].iter().all(|k| map.contains_key(*k))
}

/// `INVOKE obj::m(args)` parses to `Invoke`, the bare `obj::m(args)` to
/// `InvokeExpr` over a member call. Rule 9 respells one as the other, so the
/// comparison reads a plain member call on a name as the `Invoke` it replaces.
fn canonical_invoke(v: &mut serde_json::Value) {
    use serde_json::{json, Value};
    let Some(expr) = v.get("InvokeExpr").and_then(|b| b.get("expr")).and_then(|e| e.get("Member")) else {
        return;
    };
    let recv = expr.get("recv").and_then(|r| r.get("Identifier")).and_then(|a| a.get(0)).cloned();
    let member = expr.get("member").cloned();
    let args = expr.get("args").cloned();
    let parens = expr.get("parens").and_then(Value::as_bool) == Some(true);
    if let (Some(object), Some(method), Some(args), true) = (recv, member, args, parens) {
        *v = json!({"Invoke": {"args": args, "comma_form": true, "method": method,
                               "object": object, "returning": null, "span": null}});
    }
}

fn strip_positions(v: &mut serde_json::Value) {
    match v {
        serde_json::Value::Object(map) => {
            if is_span(map) {
                *v = serde_json::Value::Null;
                return;
            }
            for child in map.values_mut() {
                strip_positions(child);
            }
            // After the children (their spans are gone) the shapes compare.
            canonical_invoke(v);
        }
        serde_json::Value::Array(items) => {
            // A sentence period leaves a marker in the statement list; the style
            // removes periods, so the markers are not part of the comparison.
            items.retain(|item| !item.as_object().is_some_and(|o| o.len() == 1 && o.contains_key("SentenceEnd")));
            for child in items.iter_mut() {
                strip_positions(child);
            }
        }
        _ => {}
    }
}

/// Do the two texts parse to the same program? `None` when `before` cannot be
/// parsed, so nothing can be proved about it.
pub(super) fn programs_equivalent(before: &str, after: &str) -> Option<bool> {
    let (before, after) = (readings(before), readings(after));
    for (b, a) in before.iter().zip(after.iter()) {
        if let Some(program) = normalized_program(b) {
            return Some(normalized_program(a).is_some_and(|x| x == program));
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn words_keep_literals_and_calls_whole() {
        let s = r#"MOVE "a to b" TO FUNCTION TRIM(X Y)"#;
        let w: Vec<&str> = top_level_words(s).iter().map(|(a, b)| &s[*a..*b]).collect();
        assert_eq!(w, ["MOVE", r#""a to b""#, "TO", "FUNCTION", "TRIM(X Y)"]);
    }

    #[test]
    fn invoke_is_dropped_only_for_a_plain_call() {
        assert_eq!(drop_invoke("INVOKE Txt-Log::AppendText(WS-NL)."), Some("Txt-Log::AppendText(WS-NL).".into()));
        assert_eq!(drop_invoke("INVOKE ME::\"OpenFormSync\"(\"X\")"), None);
        assert_eq!(drop_invoke("INVOKE ME::OpenFormSync(\"X\")"), None, "the window supervisor keeps INVOKE");
        assert_eq!(drop_invoke("INVOKE Super::SetProperty(\"A\", 1)"), None);
        assert_eq!(
            drop_invoke("INVOKE Txt-Log::AppendText(Txt-Name::Text)"),
            Some("Txt-Log::AppendText(Txt-Name::Text)".into()),
            "a `::` inside the arguments is no chain"
        );
        assert_eq!(drop_invoke("INVOKE Grid::Rows(1)::Delete()"), None, "a chain after the call");
        assert_eq!(drop_invoke("INVOKE Obj::Get() RETURNING WS-X"), None);
        assert_eq!(drop_invoke("INVOKE Obj \"Method\""), None);
    }

    #[test]
    fn move_becomes_set_with_the_period_at_the_end() {
        assert_eq!(move_to_set("MOVE 14 TO VWR-1::FontSize."), Some("SET VWR-1::FontSize TO 14.".into()));
        assert_eq!(move_to_set("MOVE 0 TO A B C"), Some("SET A B C TO 0".into()));
        assert_eq!(move_to_set("move x to y"), Some("set y to x".into()));
        assert_eq!(move_to_set("MOVE CORRESPONDING A TO B"), None);
    }

    #[test]
    fn verbs_get_four_spaces_and_set_five() {
        assert_eq!(space_verb("MOVE 1 TO X"), "MOVE    1 TO X");
        assert_eq!(space_verb("SET X TO 1"), "SET     X TO 1");
        assert_eq!(space_verb("EXIT PERFORM"), "EXIT PERFORM");
        assert_eq!(space_verb("STOP RUN."), "STOP RUN.");
        assert_eq!(space_verb("GO TO PARA-1"), "GO TO    PARA-1");
        assert_eq!(space_verb("ELSE"), "ELSE");
        assert_eq!(space_verb("Txt::Go()"), "Txt::Go()");
    }

    #[test]
    fn to_sits_in_column_fifty_or_drops_to_the_next_line() {
        let (lines, blank) = place(12, "MOVE    14 TO VWR-1::FontSize");
        assert!(!blank);
        assert_eq!(lines[0].0.find("TO").unwrap() + 1, 50, "{lines:?}");
        assert!(lines[0].0.ends_with("TO    VWR-1::FontSize"));

        let (lines, blank) = place(12, "MOVE    \"Ready. Pick a file, or press Start stream.\" TO Lbl-Status::Caption.");
        assert!(blank);
        assert_eq!(lines.len(), 2);
        assert_eq!(col_of(&lines[1].0), 50);
        assert!(lines[1].0.trim_start().starts_with("TO    Lbl-Status::Caption."));
    }

    #[test]
    fn data_clauses_start_in_the_gutter() {
        let (lines, blank) = data_entry(8, "01 WS-NL PIC X", true);
        assert!(!blank);
        assert_eq!(lines[0], format!("       01 WS-NL{}PIC    X.", " ".repeat(49 - 15)));
        assert_eq!(lines[0].find("PIC").unwrap() + 1, 50);
    }

    #[test]
    fn a_long_name_sends_its_clauses_to_the_next_line() {
        let (lines, blank) = data_entry(16, "10 WS-G-CUSTOMER-STATUS-LINE-CAPTION PIC X(60)", true);
        assert!(blank);
        assert_eq!(lines.len(), 2);
        assert_eq!(col_of(&lines[1]), 50);
        assert!(lines[1].trim_start().starts_with("PIC    X(60)."));
    }

    #[test]
    fn later_clauses_follow_after_four_spaces() {
        let (lines, _) = data_entry(12, "05 WS-REC-FLAG PIC X VALUE \"N\"", true);
        assert!(lines[0].ends_with("PIC    X    VALUE \"N\"."), "{lines:?}");
        let (lines, _) = data_entry(12, "05 WS-G OCCURS 12 TIMES", true);
        assert!(lines[0].ends_with("OCCURS    12 TIMES."), "{lines:?}");
        let (lines, _) = data_entry(8, "01 WS-RECORD", true);
        assert_eq!(lines[0].trim_end(), "       01 WS-RECORD.");
    }

    #[test]
    fn the_gate_accepts_a_respelling_and_refuses_a_change() {
        let before = "PROCEDURE DIVISION.\n    MOVE 1 TO WS-A.\n    INVOKE Txt::Go().\n";
        let same = "PROCEDURE DIVISION.\n    SET WS-A TO 1\n    Txt::Go().\n";
        let different = "PROCEDURE DIVISION.\n    SET WS-A TO 2\n    Txt::Go().\n";
        assert_eq!(programs_equivalent(before, same), Some(true));
        assert_eq!(programs_equivalent(before, different), Some(false));
        assert_eq!(programs_equivalent("PROCEDURE DIVISION.\n    MOVE TO .\n", same), None);
    }
}
