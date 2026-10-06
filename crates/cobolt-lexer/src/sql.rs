// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! The **embedded SQL scanner** (spec 087 R1, R4–R6, R9, R16).
//!
//! The text between `EXEC SQL` and `END-EXEC` belongs to the database, not to
//! COBOL: it is sent as written, and the only COBOL inside it is the host
//! variables (`:WS-NAME`). This module is the one place that knows how SQL text
//! is shaped, so the lexer, the copybook preprocessor, the parser, the runtime
//! (placeholders) and the IDE's highlighter all agree about it:
//!
//! * `'…'` strings, with `''` for an apostrophe inside;
//! * `"…"` and `` `…` `` quoted identifiers, with the delimiter doubled inside;
//! * `$tag$…$tag$` dollar quotes (PostgreSQL);
//! * `--` and `*>` comments to the end of the line, which are never sent;
//! * `/* … */` comments, which are sent, so optimizer hints survive (not
//!   nested: MySQL does not nest them, and a nesting reader would swallow the
//!   rest of a MySQL block);
//! * `?` parameter markers and `::` casts.
//!
//! A backslash is not an escape: standard SQL doubles the apostrophe, and the
//! scanner cannot know which database a block is for.

/// What a stretch of SQL text is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SqlKind {
    /// Keywords, names, operators, numbers and white space.
    Code,
    /// A `'…'` string.
    String,
    /// A `"…"` or `` `…` `` quoted identifier.
    QuotedIdent,
    /// A `$tag$…$tag$` dollar-quoted string.
    DollarQuoted,
    /// A `--` or `*>` comment, up to (not including) the end of its line.
    LineComment,
    /// A `/* … */` comment.
    BlockComment,
    /// A `?` parameter marker.
    Param,
    /// A `::` cast operator.
    Cast,
}

/// One classified stretch of SQL text, as a byte range of the scanned text.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SqlPiece {
    pub kind: SqlKind,
    pub start: usize,
    pub end: usize,
}

/// What a line of SQL ends inside, so the next line is read the same way.
///
/// Only the constructs that can cross a line need a state; a `--` comment ends
/// with its line.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum LineState {
    #[default]
    Code,
    String,
    QuotedIdent(u8),
    BlockComment,
    /// The tag between the dollars, without them (`""` for `$$`).
    Dollar(String),
}

/// The text of an `EXEC SQL … END-EXEC` block, as the lexer captured it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SqlBlock {
    /// Everything between the word `SQL` and the word `END-EXEC`, verbatim.
    pub text: String,
    /// The source line each line of `text` came from (1-based), so a
    /// diagnostic about the block names the line the developer wrote, even
    /// after a copybook expansion.
    pub lines: Vec<u32>,
    /// The column of the first character of `text` (1-based).
    pub first_col: u32,
    /// The line holding `END-EXEC`.
    pub end_line: u32,
}

impl SqlBlock {
    /// The source line of a byte offset inside [`SqlBlock::text`].
    pub fn line_of(&self, offset: usize) -> u32 {
        let n = self.text.as_bytes()[..offset.min(self.text.len())]
            .iter()
            .filter(|b| **b == b'\n')
            .count();
        self.lines
            .get(n)
            .or(self.lines.last())
            .copied()
            .unwrap_or(self.end_line)
    }
}

/// What `EXEC SQL INCLUDE SQLCA END-EXEC` inserts (R17): this project's own
/// SQL communication area. The runtime sets every field after each statement;
/// `SQLCODE` and `SQLSTATE` keep their conventional names so a program written
/// for the SQLCA style tests them as it always has.
pub const SQLCA_TEXT: &str = "\
       01  SQLCA.
           05  SQLCA-TAG               PIC X(8) VALUE \"RCSQLCA1\".
           05  SQLCODE                 PIC S9(9) COMP-5 VALUE 0.
           05  SQLSTATE                PIC X(5) VALUE \"00000\".
           05  SQLCA-ROWS              PIC S9(18) COMP-5 VALUE 0.
           05  SQLCA-MESSAGE-LENGTH    PIC S9(4) COMP-5 VALUE 0.
           05  SQLCA-MESSAGE           PIC X(512) VALUE SPACES.
           05  SQLCA-NATIVE-CODE       PIC S9(9) COMP-5 VALUE 0.
           05  SQLCA-WARNING           PIC X VALUE SPACE.
           05  SQLCA-TRUNCATED         PIC X VALUE SPACE.
           05  SQLCA-CONNECTION        PIC X(64) VALUE SPACES.
";

/// What `EXEC SQL INCLUDE SQLDA END-EXEC` inserts (R41): this project's own
/// SQL descriptor area, room for 100 columns or parameters. A program that
/// needs more declares its own copy with a larger `OCCURS` (and
/// `SQLDA-CAPACITY` to match) under the same tag.
pub const SQLDA_TEXT: &str = "\
       01  SQLDA.
           05  SQLDA-TAG               PIC X(8) VALUE \"RCSQLDA1\".
           05  SQLDA-CAPACITY          PIC S9(4) COMP-5 VALUE 100.
           05  SQLDA-NEEDED            PIC S9(4) COMP-5 VALUE 0.
           05  SQLDA-COUNT             PIC S9(4) COMP-5 VALUE 0.
           05  SQLDA-ENTRY OCCURS 100 TIMES.
               10  SQLDA-NAME          PIC X(128).
               10  SQLDA-TYPE          PIC S9(4) COMP-5.
               10  SQLDA-TYPE-NAME     PIC X(32).
               10  SQLDA-LENGTH        PIC S9(9) COMP-5.
               10  SQLDA-PRECISION     PIC S9(4) COMP-5.
               10  SQLDA-SCALE         PIC S9(4) COMP-5.
               10  SQLDA-NULLABLE      PIC S9(4) COMP-5.
               10  SQLDA-DATA          USAGE POINTER.
               10  SQLDA-IND-PTR       USAGE POINTER.
               10  SQLDA-IND           PIC S9(4) COMP-5.
               10  SQLDA-VALUE         PIC X(1024).
";

fn is_word_byte(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'-' || b == b'_'
}

/// Scan `text` from a known starting state. Returns the pieces, in order and
/// covering the whole text, and the state the text ends in.
pub fn scan_from(text: &str, start: LineState) -> (Vec<SqlPiece>, LineState) {
    let b = text.as_bytes();
    let mut pieces: Vec<SqlPiece> = Vec::new();
    let mut state = start;
    let mut i = 0usize;
    // Start of the current Code run, if one is open.
    let mut code_from: Option<usize> = None;

    fn flush(pieces: &mut Vec<SqlPiece>, code_from: &mut Option<usize>, at: usize) {
        if let Some(s) = code_from.take() {
            if at > s {
                pieces.push(SqlPiece { kind: SqlKind::Code, start: s, end: at });
            }
        }
    }

    while i < b.len() {
        match state.clone() {
            LineState::String => {
                let s = i;
                let mut closed = None;
                while i < b.len() {
                    if b[i] == b'\'' {
                        if b.get(i + 1) == Some(&b'\'') {
                            i += 2;
                            continue;
                        }
                        closed = Some(i + 1);
                        break;
                    }
                    i += 1;
                }
                match closed {
                    Some(e) => {
                        pieces.push(SqlPiece { kind: SqlKind::String, start: s, end: e });
                        i = e;
                        state = LineState::Code;
                    }
                    None => pieces.push(SqlPiece { kind: SqlKind::String, start: s, end: b.len() }),
                }
            }
            LineState::QuotedIdent(q) => {
                let s = i;
                let mut closed = None;
                while i < b.len() {
                    if b[i] == q {
                        if b.get(i + 1) == Some(&q) {
                            i += 2;
                            continue;
                        }
                        closed = Some(i + 1);
                        break;
                    }
                    i += 1;
                }
                match closed {
                    Some(e) => {
                        pieces.push(SqlPiece { kind: SqlKind::QuotedIdent, start: s, end: e });
                        i = e;
                        state = LineState::Code;
                    }
                    None => pieces.push(SqlPiece { kind: SqlKind::QuotedIdent, start: s, end: b.len() }),
                }
            }
            LineState::BlockComment => {
                let s = i;
                match text[i..].find("*/") {
                    Some(k) => {
                        let e = i + k + 2;
                        pieces.push(SqlPiece { kind: SqlKind::BlockComment, start: s, end: e });
                        i = e;
                        state = LineState::Code;
                    }
                    None => {
                        pieces.push(SqlPiece { kind: SqlKind::BlockComment, start: s, end: b.len() });
                        i = b.len();
                    }
                }
            }
            LineState::Dollar(tag) => {
                let s = i;
                let delim = format!("${tag}$");
                match text[i..].find(&delim) {
                    Some(k) => {
                        let e = i + k + delim.len();
                        pieces.push(SqlPiece { kind: SqlKind::DollarQuoted, start: s, end: e });
                        i = e;
                        state = LineState::Code;
                    }
                    None => {
                        pieces.push(SqlPiece { kind: SqlKind::DollarQuoted, start: s, end: b.len() });
                        i = b.len();
                    }
                }
            }
            LineState::Code => {
                let c = b[i];
                let next = b.get(i + 1).copied();
                match (c, next) {
                    (b'\'', _) | (b'"', _) | (b'`', _) => {
                        flush(&mut pieces, &mut code_from, i);
                        let kind = if c == b'\'' { SqlKind::String } else { SqlKind::QuotedIdent };
                        match close_quote(b, i + 1, c) {
                            Some(e) => {
                                pieces.push(SqlPiece { kind, start: i, end: e });
                                i = e;
                            }
                            None => {
                                pieces.push(SqlPiece { kind, start: i, end: b.len() });
                                i = b.len();
                                state = if c == b'\'' { LineState::String } else { LineState::QuotedIdent(c) };
                            }
                        }
                    }
                    (b'-', Some(b'-')) | (b'*', Some(b'>')) => {
                        flush(&mut pieces, &mut code_from, i);
                        let e = text[i..].find('\n').map(|k| i + k).unwrap_or(b.len());
                        pieces.push(SqlPiece { kind: SqlKind::LineComment, start: i, end: e });
                        i = e;
                    }
                    (b'/', Some(b'*')) => {
                        flush(&mut pieces, &mut code_from, i);
                        match text[i + 2..].find("*/") {
                            Some(k) => {
                                let e = i + 2 + k + 2;
                                pieces.push(SqlPiece { kind: SqlKind::BlockComment, start: i, end: e });
                                i = e;
                            }
                            None => {
                                pieces.push(SqlPiece { kind: SqlKind::BlockComment, start: i, end: b.len() });
                                i = b.len();
                                state = LineState::BlockComment;
                            }
                        }
                    }
                    (b'$', _) if dollar_tag(b, i).is_some() && (i == 0 || !is_word_byte(b[i - 1]) && b[i - 1] != b'$') => {
                        let tag = dollar_tag(b, i).unwrap_or_default();
                        flush(&mut pieces, &mut code_from, i);
                        let open_len = tag.len() + 2;
                        let delim = format!("${tag}$");
                        match text[i + open_len..].find(&delim) {
                            Some(k) => {
                                let e = i + open_len + k + delim.len();
                                pieces.push(SqlPiece { kind: SqlKind::DollarQuoted, start: i, end: e });
                                i = e;
                            }
                            None => {
                                pieces.push(SqlPiece { kind: SqlKind::DollarQuoted, start: i, end: b.len() });
                                i = b.len();
                                state = LineState::Dollar(tag);
                            }
                        }
                    }
                    (b'?', _) => {
                        flush(&mut pieces, &mut code_from, i);
                        pieces.push(SqlPiece { kind: SqlKind::Param, start: i, end: i + 1 });
                        i += 1;
                    }
                    (b':', Some(b':')) => {
                        flush(&mut pieces, &mut code_from, i);
                        pieces.push(SqlPiece { kind: SqlKind::Cast, start: i, end: i + 2 });
                        i += 2;
                    }
                    _ => {
                        if code_from.is_none() {
                            code_from = Some(i);
                        }
                        i += 1;
                    }
                }
            }
        }
    }
    flush(&mut pieces, &mut code_from, b.len());
    (pieces, state)
}

/// The tag of a dollar quote opening at `i` (`$$` → `""`, `$body$` → `"body"`).
/// A tag starts with a letter or `_`, so `$1` (a PostgreSQL parameter) is not
/// a quote.
/// The index just past the `q` that closes a quoted run starting at `i` (the
/// character after the opening quote); a doubled `q` stays inside.
fn close_quote(b: &[u8], mut i: usize, q: u8) -> Option<usize> {
    while i < b.len() {
        if b[i] == q {
            if b.get(i + 1) == Some(&q) {
                i += 2;
                continue;
            }
            return Some(i + 1);
        }
        i += 1;
    }
    None
}

fn dollar_tag(b: &[u8], i: usize) -> Option<String> {
    if b.get(i) != Some(&b'$') {
        return None;
    }
    let mut j = i + 1;
    if b.get(j) == Some(&b'$') {
        return Some(String::new());
    }
    let first = *b.get(j)?;
    if !(first.is_ascii_alphabetic() || first == b'_') {
        return None;
    }
    while j < b.len() && (b[j].is_ascii_alphanumeric() || b[j] == b'_') {
        j += 1;
    }
    (b.get(j) == Some(&b'$')).then(|| String::from_utf8_lossy(&b[i + 1..j]).into_owned())
}

/// Scan a whole block of SQL text.
pub fn scan(text: &str) -> Vec<SqlPiece> {
    scan_from(text, LineState::Code).0
}

/// Scan one line of SQL that starts in `state`, and advance `state` to where
/// the line ends — what an editor's line highlighter needs.
pub fn scan_line(line: &str, state: &mut LineState) -> Vec<SqlPiece> {
    let (pieces, end) = scan_from(line, state.clone());
    *state = end;
    pieces
}

/// The byte range of the `END-EXEC` that closes a block whose text starts at
/// the beginning of `text`: the first `END-EXEC` that is not inside a string,
/// a quoted identifier, a comment or a dollar quote (R5).
pub fn block_end(text: &str) -> Option<(usize, usize)> {
    const WORD: &str = "END-EXEC";
    let b = text.as_bytes();
    for p in scan(text) {
        if p.kind != SqlKind::Code {
            continue;
        }
        let seg = &text[p.start..p.end];
        let upper = seg.to_ascii_uppercase();
        let mut from = 0;
        while let Some(k) = upper[from..].find(WORD) {
            let s = p.start + from + k;
            let e = s + WORD.len();
            let before_ok = s == 0 || !is_word_byte(b[s - 1]);
            let after_ok = e >= b.len() || !is_word_byte(b[e]);
            if before_ok && after_ok {
                return Some((s, e));
            }
            from += k + 1;
        }
    }
    None
}

/// The SQL text that is sent to the database: `--` and `*>` comments removed
/// (R4), everything else — `/* */` hints included — kept as written.
pub fn strip_line_comments(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for p in scan(text) {
        if p.kind != SqlKind::LineComment {
            out.push_str(&text[p.start..p.end]);
        }
    }
    out
}

// ── Host variables ────────────────────────────────────────────────────────────

/// A COBOL name as written in a block: the item and the names that qualify it,
/// innermost first (`:CITY OF CUSTOMER` → `CITY`, `[CUSTOMER]`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HostName {
    pub name: String,
    pub quals: Vec<String>,
    /// The other reading of a period-qualified name (`:A.B` is `A OF B` or
    /// `B OF A`); `None` for every other form.
    pub swapped: Option<(String, Vec<String>)>,
}

/// One host-variable reference in a block (R6, R9).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HostRef {
    /// Byte range of the whole reference, indicator included.
    pub start: usize,
    pub end: usize,
    pub var: HostName,
    pub indicator: Option<HostName>,
    /// The reference carried a subscript or a reference modification
    /// (`:A(1)`, `:A(1:2)`), which R14 forbids.
    pub subscripted: bool,
}

/// Every host-variable reference in `text`, in order.
///
/// A host variable is a `:` that is not part of `::` or `:=`, followed by a
/// COBOL word containing a letter (so `arr[1:2]` is not one). A trailing
/// hyphen is not part of the name, and a hyphen anywhere else in the SQL is
/// the database's (R16): `QTY-1` is a subtraction, `:WS-QTY-LESS-ONE` is a
/// name. `OF` qualifies anywhere; `IN` only outside parentheses, because
/// inside them it is SQL (`POSITION(:A IN col)`).
pub fn host_variables(text: &str) -> Vec<HostRef> {
    let b = text.as_bytes();
    // Which bytes are SQL code (host variables live only there).
    let mut code = vec![false; b.len()];
    for p in scan(text) {
        if p.kind == SqlKind::Code {
            for c in &mut code[p.start..p.end] {
                *c = true;
            }
        }
    }
    let mut out = Vec::new();
    let mut depth: i32 = 0;
    let mut i = 0usize;
    while i < b.len() {
        if !code[i] {
            i += 1;
            continue;
        }
        match b[i] {
            b'(' => depth += 1,
            b')' => depth -= 1,
            b':' => {
                let prev_colon = i > 0 && b[i - 1] == b':';
                let next = b.get(i + 1).copied();
                if !prev_colon && !matches!(next, Some(b':') | Some(b'=')) {
                    if let Some((var, after)) = read_host_name(b, &code, i + 1, depth) {
                        let start = i;
                        let mut end = after;
                        let mut subscripted = false;
                        if b.get(end) == Some(&b'(') {
                            subscripted = true;
                            end = close_paren(b, end).unwrap_or(end);
                        }
                        // Indicator: `:H:I`, or `:H INDICATOR :I`.
                        let mut indicator = None;
                        if b.get(end) == Some(&b':') && b.get(end + 1) != Some(&b':') {
                            if let Some((ind, e)) = read_host_name(b, &code, end + 1, depth) {
                                indicator = Some(ind);
                                end = e;
                            }
                        } else {
                            let w = skip_ws(b, end);
                            if let Some(e) = keyword_at(b, w, "INDICATOR") {
                                let w2 = skip_ws(b, e);
                                if b.get(w2) == Some(&b':') {
                                    if let Some((ind, e2)) = read_host_name(b, &code, w2 + 1, depth) {
                                        indicator = Some(ind);
                                        end = e2;
                                    }
                                }
                            }
                        }
                        out.push(HostRef { start, end, var, indicator, subscripted });
                        i = end;
                        continue;
                    }
                }
            }
            _ => {}
        }
        i += 1;
    }
    out
}

fn skip_ws(b: &[u8], mut i: usize) -> usize {
    while i < b.len() && b[i].is_ascii_whitespace() {
        i += 1;
    }
    i
}

/// The end of `word` if it stands at `i` as a whole word (any case).
fn keyword_at(b: &[u8], i: usize, word: &str) -> Option<usize> {
    let e = i + word.len();
    if e > b.len() || !b[i..e].eq_ignore_ascii_case(word.as_bytes()) {
        return None;
    }
    if e < b.len() && is_word_byte(b[e]) {
        return None;
    }
    Some(e)
}

/// A COBOL word starting at `i`: letters, digits, `-` and `_`, containing a
/// letter, without a trailing hyphen. Returns the upper-cased word and its end.
fn read_word(b: &[u8], i: usize) -> Option<(String, usize)> {
    let mut j = i;
    while j < b.len() && is_word_byte(b[j]) {
        j += 1;
    }
    while j > i && b[j - 1] == b'-' {
        j -= 1;
    }
    if j == i || b[i] == b'-' || !b[i..j].iter().any(|c| c.is_ascii_alphabetic()) {
        return None;
    }
    Some((String::from_utf8_lossy(&b[i..j]).to_ascii_uppercase(), j))
}

/// A host name at `i` (just after its colon) with its qualification.
fn read_host_name(b: &[u8], code: &[bool], i: usize, depth: i32) -> Option<(HostName, usize)> {
    let (name, mut end) = read_word(b, i)?;
    let mut quals = Vec::new();
    // Period qualification, `:A.B` (the period glued on both sides).
    let mut dotted = false;
    while b.get(end) == Some(&b'.') {
        match read_word(b, end + 1) {
            Some((q, e)) => {
                quals.push(q);
                end = e;
                dotted = true;
            }
            None => break,
        }
    }
    if dotted {
        // `:A.B.C` reads A OF B OF C, or the other way round, C OF B OF A.
        let mut all = vec![name.clone()];
        all.extend(quals.iter().cloned());
        all.reverse();
        let swapped = Some((all[0].clone(), all[1..].to_vec()));
        return Some((HostName { name, quals, swapped }, end));
    }
    // `OF`/`IN` qualification, possibly repeated.
    loop {
        let w = skip_ws(b, end);
        if w >= b.len() || !code[w] {
            break;
        }
        let kw_end = keyword_at(b, w, "OF").or_else(|| if depth <= 0 { keyword_at(b, w, "IN") } else { None });
        let Some(kw_end) = kw_end else { break };
        let w2 = skip_ws(b, kw_end);
        if w2 >= b.len() || !code[w2] || b[w2] == b':' {
            break;
        }
        match read_word(b, w2) {
            Some((q, e)) => {
                quals.push(q);
                end = e;
            }
            None => break,
        }
    }
    Some((HostName { name, quals, swapped: None }, end))
}

/// The index just past the `)` matching the `(` at `i`.
fn close_paren(b: &[u8], i: usize) -> Option<usize> {
    let mut d = 0i32;
    for (k, c) in b.iter().enumerate().skip(i) {
        match c {
            b'(' => d += 1,
            b')' => {
                d -= 1;
                if d == 0 {
                    return Some(k + 1);
                }
            }
            _ => {}
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn kinds(text: &str) -> Vec<(SqlKind, &str)> {
        scan(text).into_iter().map(|p| (p.kind, &text[p.start..p.end])).collect()
    }

    fn names(text: &str) -> Vec<String> {
        host_variables(text).into_iter().map(|h| h.var.name).collect()
    }

    #[test]
    fn pieces_cover_the_text_in_order() {
        let t = "SELECT 'a''b', \"c\" -- x\n /* y */ ? x::int $q$z$q$";
        let ps = scan(t);
        let mut at = 0;
        for p in &ps {
            assert_eq!(p.start, at, "gap before {p:?} in {ps:?}");
            at = p.end;
        }
        assert_eq!(at, t.len());
        let k = kinds(t);
        assert!(k.contains(&(SqlKind::String, "'a''b'")));
        assert!(k.contains(&(SqlKind::QuotedIdent, "\"c\"")));
        assert!(k.contains(&(SqlKind::LineComment, "-- x")));
        assert!(k.contains(&(SqlKind::BlockComment, "/* y */")));
        assert!(k.contains(&(SqlKind::Param, "?")));
        assert!(k.contains(&(SqlKind::Cast, "::")));
        assert!(k.contains(&(SqlKind::DollarQuoted, "$q$z$q$")));
    }

    #[test]
    fn end_exec_inside_strings_identifiers_comments_and_dollar_quotes_does_not_end_the_block() {
        let cases = [
            "SELECT 'END-EXEC' FROM T",
            "SELECT 'it''s END-EXEC' FROM T",
            "SELECT \"END-EXEC\" FROM T",
            "SELECT `END-EXEC` FROM T",
            "SELECT 1 -- END-EXEC\n FROM T",
            "SELECT 1 *> END-EXEC\n FROM T",
            "SELECT 1 /* END-EXEC */ FROM T",
            "SELECT $x$ END-EXEC $x$ FROM T",
            "SELECT $$ END-EXEC $$ FROM T",
        ];
        for c in cases {
            let text = format!("{c}\n END-EXEC.");
            let (s, _) = block_end(&text).unwrap_or_else(|| panic!("no end in {text:?}"));
            assert_eq!(s, text.rfind("END-EXEC").unwrap(), "wrong END-EXEC chosen in {text:?}");
        }
        // A word that merely contains END-EXEC is not one.
        assert_eq!(block_end("X-END-EXEC-Y"), None);
        assert_eq!(block_end(" end-exec").map(|r| r.0), Some(1), "any case");
    }

    #[test]
    fn casts_slices_and_assignments_are_not_host_variables() {
        assert!(host_variables("SELECT x::int FROM t").is_empty());
        assert!(host_variables("SELECT arr[1:2] FROM t").is_empty());
        assert!(host_variables("SET @a := 1").is_empty());
        assert!(host_variables("SELECT ':WS-X' FROM t").is_empty(), "inside a string");
        assert!(host_variables("SELECT 1 -- :WS-X\n").is_empty(), "inside a comment");
    }

    #[test]
    fn a_hyphen_is_sql_except_inside_a_host_variable_name() {
        let t = "SELECT QTY-1 INTO :WS-QTY-LESS-ONE FROM T";
        let hv = host_variables(t);
        assert_eq!(hv.len(), 1);
        assert_eq!(hv[0].var.name, "WS-QTY-LESS-ONE");
        assert_eq!(&t[hv[0].start..hv[0].end], ":WS-QTY-LESS-ONE");
        // A trailing hyphen is the SQL operator, not part of the name.
        let t2 = "SELECT :A- 1";
        assert_eq!(names(t2), ["A"]);
        let h = &host_variables(t2)[0];
        assert_eq!(&t2[h.start..h.end], ":A");
    }

    #[test]
    fn every_qualification_form() {
        let of = &host_variables("WHERE C = :CITY OF CUSTOMER")[0].var;
        assert_eq!((of.name.as_str(), of.quals.clone()), ("CITY", vec!["CUSTOMER".to_string()]));
        let inq = &host_variables("WHERE C = :CITY IN CUSTOMER OF DB")[0].var;
        assert_eq!(inq.quals, ["CUSTOMER", "DB"]);
        // `IN` inside parentheses is SQL.
        let pos = host_variables("SELECT POSITION(:A IN COL) FROM T");
        assert_eq!(pos[0].var.quals, Vec::<String>::new());
        // Period form: both readings are kept.
        let dot = &host_variables("WHERE C = :CITY.CUSTOMER")[0].var;
        assert_eq!(dot.name, "CITY");
        assert_eq!(dot.quals, ["CUSTOMER"]);
        assert_eq!(dot.swapped, Some(("CUSTOMER".to_string(), vec!["CITY".to_string()])));
        // A following host variable is not a qualifier.
        let two = host_variables("VALUES (:A, :B)");
        assert_eq!(two.len(), 2);
    }

    #[test]
    fn every_indicator_form() {
        let a = &host_variables("INTO :PHONE:PHONE-IND")[0];
        assert_eq!(a.var.name, "PHONE");
        assert_eq!(a.indicator.as_ref().unwrap().name, "PHONE-IND");
        let b = &host_variables("INTO :PHONE INDICATOR :PHONE-IND, :X")[0];
        assert_eq!(b.indicator.as_ref().unwrap().name, "PHONE-IND");
        let c = &host_variables("INTO :P OF R:PI OF R")[0];
        assert_eq!(c.var.quals, ["R"]);
        let ci = c.indicator.as_ref().unwrap();
        assert_eq!((ci.name.as_str(), ci.quals.clone()), ("PI", vec!["R".to_string()]));
    }

    #[test]
    fn a_subscript_or_reference_modification_is_flagged() {
        let s = host_variables("WHERE A = :TBL(3) AND B = :NAME(1:4) AND C = :OK");
        assert_eq!(s.iter().map(|h| h.subscripted).collect::<Vec<_>>(), [true, true, false]);
    }

    #[test]
    fn strings_and_block_comments_that_span_lines_keep_their_state() {
        let mut st = LineState::Code;
        let l1 = scan_line("SELECT 'first", &mut st);
        assert_eq!(st, LineState::String);
        assert_eq!(l1.last().unwrap().kind, SqlKind::String);
        let l2 = scan_line("second' FROM T /* a", &mut st);
        assert_eq!(l2[0].kind, SqlKind::String);
        assert_eq!(st, LineState::BlockComment);
        let l3 = scan_line(" b */ WHERE X = $t$ c", &mut st);
        assert_eq!(l3[0].kind, SqlKind::BlockComment);
        assert_eq!(st, LineState::Dollar("t".into()));
        let l4 = scan_line(" d $t$ AND \"q", &mut st);
        assert_eq!(l4[0].kind, SqlKind::DollarQuoted);
        assert_eq!(st, LineState::QuotedIdent(b'"'));
        scan_line("x\"", &mut st);
        assert_eq!(st, LineState::Code);
    }

    #[test]
    fn line_comments_are_not_sent() {
        let t = "SELECT A -- why\n FROM T *> note\n /*+ HINT */ WHERE B = 'x--y'";
        assert_eq!(strip_line_comments(t), "SELECT A \n FROM T \n /*+ HINT */ WHERE B = 'x--y'");
    }

    #[test]
    fn a_dollar_parameter_is_not_a_quote() {
        let k = kinds("WHERE A = $1 AND B = $2");
        assert!(k.iter().all(|(kind, _)| *kind == SqlKind::Code), "{k:?}");
    }
}
