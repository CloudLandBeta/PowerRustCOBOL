// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Rewriting the older spellings of a data item's value methods to the one
//! spelling that is written today.
//!
//! `UpperCase` and `ToUpperCase` are `Upper`; `LowerCase` and `ToLowerCase` are
//! `Lower`; `Length` is `Len` (operator, 2026-10-10). All of them keep running —
//! [`crate::methods::DATA_ITEM_METHODS`] still lists every spelling — so this
//! module changes how a program is *written*, never what it does. The IDE runs
//! it when the developer saves, which is why it has to be exact:
//!
//! * **Token-aware.** Only a word that comes straight after `::` is a candidate.
//!   A string, a hexadecimal or national literal, a comment (`*>`, a column-7
//!   `*` or `/`, a `D` debugging line), a `PICTURE`, an `EXEC … END-EXEC`
//!   block, a ```` ``` ```` block literal and `==pseudo-text==` are never
//!   looked into, and neither are COBOL names that merely contain the words
//!   (`WS-LENGTH`, `UPPERCASE-FLAG`) nor the intrinsic functions
//!   (`FUNCTION LENGTH(…)`, `FUNCTION UPPER-CASE(…)`), which do not follow `::`.
//! * **Receiver-aware.** A member name means something only relative to what it
//!   is a member OF. `super::Length` may be a property a form published under
//!   that name; `PLANK-1::Length` may be one a user control declares. So a link
//!   is rewritten only when what precedes it is certainly a *value*: a data
//!   item (the caller says which names are — the analyser's own rule), or the
//!   result of a method call that returns one (`TXT-A::GetText()::UpperCase()`).
//!   Anything else is left as typed — it still works. (A literal cannot be the
//!   receiver at all: `"abc"::Upper()` is a syntax error.)
//! * **Exact.** Only the candidate's own bytes are replaced; every other byte of
//!   the text — line endings, tabs, columns, accents — is copied untouched.
//! * **Idempotent.** The canonical spellings are not candidates.

use cobolt_lexer::SourceFormat;

use crate::methods::{canonical_data_item_method, is_known_method};

/// One spelling that was changed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MethodEdit {
    /// 1-based line of the original text.
    pub line: u32,
    /// 1-based column, counted in characters, where the old spelling began.
    pub column: u32,
    /// Byte offset of the old spelling in the ORIGINAL text.
    pub offset: usize,
    /// The spelling as the developer wrote it (`ToUpperCase`, `length` …).
    pub old: String,
    /// The canonical spelling written in its place.
    pub new: &'static str,
}

/// The result of [`canonicalize_value_methods`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Rewritten {
    /// The text with every edit applied; equal to the input when there are none.
    pub text: String,
    /// What was changed, in source order.
    pub edits: Vec<MethodEdit>,
}

impl Rewritten {
    /// `true` when at least one spelling was changed.
    pub fn changed(&self) -> bool {
        !self.edits.is_empty()
    }
}

/// Rewrite every older spelling of a value method in `source` to its canonical
/// spelling.
///
/// `format` says how the text is laid out: [`SourceFormat::Free`] for form
/// handlers and free-form programs, [`SourceFormat::Fixed`] for the relaxed
/// fixed form PowerRustCOBOL reads, [`SourceFormat::FixedStrict`] for the
/// classic reference format (where columns 73-80 are not source).
///
/// `is_data_item` is asked about the root name of each receiver chain
/// (`WS-NAME` in `WS-NAME::Trim()::UpperCase()`, as written) and must answer
/// `true` only for an ordinary data item: not a control, a layer, `me`,
/// `super`, `COBOL`, an `OBJECT REFERENCE` or a `POINTER`.
pub fn canonicalize_value_methods(
    source: &str,
    format: SourceFormat,
    is_data_item: &dyn Fn(&str) -> bool,
) -> Rewritten {
    let line_starts = line_starts(source);
    let toks = scan(source, format, &line_starts);

    let mut edits: Vec<MethodEdit> = Vec::new();
    for i in 1..toks.len() {
        let t = &toks[i];
        if t.kind != Kind::Word || toks[i - 1].kind != Kind::ColonColon || t.cuts_a_word {
            continue;
        }
        let old = &source[t.start..t.end];
        let Some(new) = canonical_data_item_method(old) else {
            continue;
        };
        if !receiver_is_a_value(source, &toks, i - 1, is_data_item) {
            continue;
        }
        let line_idx = line_starts.partition_point(|&s| s <= t.start) - 1;
        edits.push(MethodEdit {
            line: line_idx as u32 + 1,
            column: source[line_starts[line_idx]..t.start].chars().count() as u32 + 1,
            offset: t.start,
            old: old.to_owned(),
            new,
        });
    }

    let mut text = String::with_capacity(source.len());
    let mut at = 0;
    for e in &edits {
        text.push_str(&source[at..e.offset]);
        text.push_str(e.new);
        at = e.offset + e.old.len();
    }
    text.push_str(&source[at..]);
    Rewritten { text, edits }
}

/// The check that a rewrite did nothing else: `after` has the same tokens as
/// `before` — comments, literals and embedded blocks included, compared in
/// full — and every token that differs is an older spelling that became the
/// canonical one.
///
/// The IDE runs it on its own output before it writes anything down, so a
/// mistake in the scanner above can cost a missed rewrite but never a changed
/// program.
pub fn differs_only_by_spellings(before: &str, after: &str, format: SourceFormat) -> bool {
    use cobolt_lexer::{tokenize_with_comments, Token};
    let a = tokenize_with_comments(before, format);
    let b = tokenize_with_comments(after, format);
    a.len() == b.len()
        && a.iter().zip(&b).all(|(x, y)| match (&x.token, &y.token) {
            (Token::Identifier(p), Token::Identifier(q)) => {
                p == q || canonical_data_item_method(p).is_some_and(|c| c.eq_ignore_ascii_case(q))
            }
            (p, q) => p == q,
        })
}

// ── Scanning ─────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Kind {
    /// A COBOL word: letters, digits, `_`, and `-` between them.
    Word,
    /// `::`
    ColonColon,
    LParen,
    RParen,
    /// A quoted literal (with any prefix letter left as its own `Word`).
    Literal,
    /// Something that is not COBOL source to look into: an `EXEC` block's
    /// body, a block literal, pseudo-text.
    Opaque,
    /// Any other character: punctuation, operators, a lone `:`.
    Other,
}

#[derive(Debug, Clone, Copy)]
struct Tok {
    kind: Kind,
    start: usize,
    end: usize,
    /// A word that ends exactly at the end of its line's source area while the
    /// next line is a continuation (`-` in column 7): the word goes on over
    /// there, so what is seen here is only its beginning.
    cuts_a_word: bool,
}

fn line_starts(source: &str) -> Vec<usize> {
    let mut starts = vec![0];
    for (i, b) in source.bytes().enumerate() {
        if b == b'\n' {
            starts.push(i + 1);
        }
    }
    starts
}

/// The byte offset of the `n`th character (0-based) of `line`, or `line.len()`.
fn byte_of_char(line: &str, n: usize) -> usize {
    line.char_indices().nth(n).map_or(line.len(), |(b, _)| b)
}

/// The indicator column (column 7) of a fixed-form line, if the line has one.
fn indicator(line: &str) -> Option<char> {
    line.chars().nth(6)
}

/// The part of a fixed-form `line` that is source, as byte offsets into it.
/// `None` for a line that carries none: a comment, a debugging line, a line too
/// short to reach column 8.
fn fixed_source_area(line: &str, strict: bool) -> Option<(usize, usize)> {
    let ind = indicator(line)?;
    // A free-style comment marker typed into the sequence area still means
    // "this is a comment" to whoever wrote it.
    if line[..byte_of_char(line, 6)].contains("*>") {
        return None;
    }
    // `D` is a comment unless the program asks for debugging mode; leaving it
    // alone is right either way.
    if matches!(ind, '*' | '/' | 'D' | 'd') {
        return None;
    }
    let start = byte_of_char(line, 7);
    let end = if strict { byte_of_char(line, 72) } else { line.len() };
    (start <= end).then_some((start, end))
}

fn is_word_char(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

/// The offset in `rest` of an `END-EXEC` word, if there is one.
fn find_end_exec(rest: &str) -> Option<usize> {
    const NEEDLE: &[u8] = b"END-EXEC";
    let bytes = rest.as_bytes();
    let boundary = |b: Option<&u8>| b.map_or(true, |b| !(b.is_ascii_alphanumeric() || *b == b'_' || *b == b'-' || *b >= 0x80));
    (0..bytes.len().saturating_sub(NEEDLE.len() - 1)).find(|&p| {
        bytes[p..p + NEEDLE.len()].eq_ignore_ascii_case(NEEDLE)
            && boundary(p.checked_sub(1).and_then(|q| bytes.get(q)))
            && boundary(bytes.get(p + NEEDLE.len()))
    })
}

fn scan(source: &str, format: SourceFormat, line_starts: &[usize]) -> Vec<Tok> {
    let free = format == SourceFormat::Free;
    let strict = format == SourceFormat::FixedStrict;
    let mut toks: Vec<Tok> = Vec::new();
    let mut in_block_literal = false;
    let mut in_pseudo_text = false;
    let mut in_exec = false;

    let line_at = |li: usize| -> &str {
        let ls = line_starts[li];
        let mut le = if li + 1 < line_starts.len() { line_starts[li + 1] - 1 } else { source.len() };
        if le > ls && source.as_bytes()[le - 1] == b'\r' {
            le -= 1;
        }
        &source[ls..le]
    };

    for li in 0..line_starts.len() {
        let ls = line_starts[li];
        let line = line_at(li);

        // A ``` block literal swallows whole lines, up to and including the
        // line that carries the closing fence.
        if in_block_literal {
            if line.trim_start().starts_with("```") {
                in_block_literal = false;
            }
            continue;
        }

        let (rs, re) = if free {
            (ls, ls + line.len())
        } else {
            match fixed_source_area(line, strict) {
                Some((a, b)) => (ls + a, ls + b),
                None => continue,
            }
        };
        let continues_next = !free && li + 1 < line_starts.len() && indicator(line_at(li + 1)) == Some('-');

        let mut i = rs;
        while i < re {
            let rest = &source[i..re];
            if in_exec {
                match find_end_exec(rest) {
                    Some(p) => {
                        i += p + "END-EXEC".len();
                        in_exec = false;
                        toks.push(Tok { kind: Kind::Opaque, start: i, end: i, cuts_a_word: false });
                        continue;
                    }
                    None => break,
                }
            }
            if in_pseudo_text {
                match rest.find("==") {
                    Some(p) => {
                        i += p + 2;
                        in_pseudo_text = false;
                        continue;
                    }
                    None => break,
                }
            }
            let c = rest.chars().next().expect("rest is not empty");
            if c.is_whitespace() {
                i += c.len_utf8();
                continue;
            }
            // A comment to the end of the line.
            if rest.starts_with("*>") {
                break;
            }
            // A block literal opens; the rest of this line is not content.
            if free && rest.starts_with("```") {
                in_block_literal = true;
                toks.push(Tok { kind: Kind::Opaque, start: i, end: i, cuts_a_word: false });
                break;
            }
            match c {
                '"' | '\'' => {
                    let mut j = i + 1;
                    let mut closed = false;
                    let mut chars = source[j..re].char_indices().peekable();
                    while let Some((off, ch)) = chars.next() {
                        if ch == c {
                            if matches!(chars.peek(), Some((_, n)) if *n == c) {
                                chars.next(); // a doubled delimiter is one character of content
                                continue;
                            }
                            j += off + ch.len_utf8();
                            closed = true;
                            break;
                        }
                    }
                    if !closed {
                        // Unterminated on this line: the rest of it is the literal.
                        j = re;
                    }
                    toks.push(Tok { kind: Kind::Literal, start: i, end: j, cuts_a_word: false });
                    i = j;
                }
                ':' if rest.starts_with("::") => {
                    toks.push(Tok { kind: Kind::ColonColon, start: i, end: i + 2, cuts_a_word: false });
                    i += 2;
                }
                '(' => {
                    toks.push(Tok { kind: Kind::LParen, start: i, end: i + 1, cuts_a_word: false });
                    i += 1;
                }
                ')' => {
                    toks.push(Tok { kind: Kind::RParen, start: i, end: i + 1, cuts_a_word: false });
                    i += 1;
                }
                '=' if rest.starts_with("==") => {
                    // Pseudo-text (`COPY … REPLACING ==a== BY ==b==`): text, not code.
                    toks.push(Tok { kind: Kind::Opaque, start: i, end: i, cuts_a_word: false });
                    match rest[2..].find("==") {
                        Some(p) => i += 2 + p + 2,
                        None => {
                            in_pseudo_text = true;
                            break;
                        }
                    }
                }
                c if is_word_char(c) => {
                    let mut j = i;
                    let mut it = rest.char_indices().peekable();
                    while let Some((off, ch)) = it.next() {
                        let joins = is_word_char(ch)
                            || (ch == '-' && matches!(it.peek(), Some((_, n)) if is_word_char(*n)));
                        if !joins {
                            break;
                        }
                        j = i + off + ch.len_utf8();
                    }
                    let cuts_a_word = j == re && continues_next;
                    toks.push(Tok { kind: Kind::Word, start: i, end: j, cuts_a_word });
                    if source[i..j].eq_ignore_ascii_case("EXEC") {
                        in_exec = true;
                    }
                    i = j;
                }
                other => {
                    toks.push(Tok { kind: Kind::Other, start: i, end: i + other.len_utf8(), cuts_a_word: false });
                    i += other.len_utf8();
                }
            }
        }
    }
    toks
}

// ── Receivers ────────────────────────────────────────────────────────────────

/// Index of the `(` that closes the `)` at `close`.
fn matching_open(toks: &[Tok], close: usize) -> Option<usize> {
    let mut depth = 0usize;
    for j in (0..=close).rev() {
        match toks[j].kind {
            Kind::RParen => depth += 1,
            Kind::LParen => {
                depth -= 1;
                if depth == 0 {
                    return Some(j);
                }
            }
            _ => {}
        }
    }
    None
}

/// Is what stands before the `::` at `toks[cc]` certainly a value?
///
/// Walks the receiver chain backwards. The nearest link settles it at once if
/// it is a call to a method that returns a value (`GetText()`); otherwise the
/// chain is followed to its root, which must be a data item.
fn receiver_is_a_value(src: &str, toks: &[Tok], mut cc: usize, is_data_item: &dyn Fn(&str) -> bool) -> bool {
    let text = |t: &Tok| &src[t.start..t.end];
    let mut nearest = true;
    loop {
        if cc == 0 {
            return false;
        }
        // The receiver ends at `cc - 1`; step back over any `( … )` groups —
        // a subscript, a reference modification, or a call's arguments.
        let mut k = cc - 1;
        let mut grouped = false;
        while toks[k].kind == Kind::RParen {
            let Some(open) = matching_open(toks, k) else {
                return false;
            };
            grouped = true;
            if open == 0 {
                return false;
            }
            k = open - 1;
        }
        match toks[k].kind {
            Kind::Word => {
                let word = text(&toks[k]);
                if k >= 1 && toks[k - 1].kind == Kind::ColonColon {
                    // A member link of the chain. Called as a method that
                    // returns a value, it makes everything after it a value.
                    if nearest && grouped && is_known_method(word) {
                        return true;
                    }
                    nearest = false;
                    cc = k - 1;
                    continue;
                }
                // The root. `A OF B` / `A IN B` names the item `A`.
                let mut root = k;
                while root >= 2
                    && toks[root - 1].kind == Kind::Word
                    && (text(&toks[root - 1]).eq_ignore_ascii_case("OF") || text(&toks[root - 1]).eq_ignore_ascii_case("IN"))
                    && toks[root - 2].kind == Kind::Word
                {
                    root -= 2;
                }
                let name = text(&toks[root]);
                return is_data_item(name);
            }
            _ => return false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn always(_: &str) -> bool {
        true
    }
    fn items(names: &'static [&'static str]) -> impl Fn(&str) -> bool {
        move |n| names.iter().any(|x| x.eq_ignore_ascii_case(n))
    }
    fn free(src: &str) -> Rewritten {
        canonicalize_value_methods(src, SourceFormat::Free, &always)
    }
    fn fixed(src: &str) -> Rewritten {
        canonicalize_value_methods(src, SourceFormat::Fixed, &always)
    }
    fn strict(src: &str) -> Rewritten {
        canonicalize_value_methods(src, SourceFormat::FixedStrict, &always)
    }

    #[test]
    fn each_older_spelling_becomes_the_canonical_one() {
        for (old, new) in [
            ("UpperCase()", "Upper()"),
            ("ToUpperCase()", "Upper()"),
            ("LowerCase()", "Lower()"),
            ("ToLowerCase()", "Lower()"),
            ("Length()", "Len()"),
            ("Length", "Len"),
        ] {
            let r = free(&format!("    MOVE WS-A::{old} TO WS-B.\n"));
            assert_eq!(r.text, format!("    MOVE WS-A::{new} TO WS-B.\n"), "{old}");
            assert_eq!(r.edits.len(), 1, "{old}");
        }
    }

    #[test]
    fn the_canonical_spellings_and_the_other_methods_are_left_alone() {
        let src = "    MOVE WS-A::Trim()::Upper() TO WS-B.\n    MOVE WS-A::Lower() TO WS-B.\n    COMPUTE N = WS-A::Len().\n    MOVE WS-A::Replace(\"a\", \"b\") TO WS-B.\n    MOVE WS-A::Split(\",\")(2) TO WS-B.\n    MOVE WS-A::upper() TO WS-B.\n";
        let r = free(src);
        assert_eq!(r.text, src);
        assert!(!r.changed());
    }

    #[test]
    fn mixed_case_is_found_and_the_replacement_is_always_the_canonical_case() {
        let r = free("    MOVE WS-A::uppercase() TO X.\n    MOVE WS-A::UPPERCASE() TO X.\n    MOVE WS-A::tolowercase() TO X.\n    MOVE WS-A::toLowerCase() TO X.\n    COMPUTE N = WS-A::LENGTH.\n    COMPUTE N = WS-A::length().\n");
        assert_eq!(
            r.text,
            "    MOVE WS-A::Upper() TO X.\n    MOVE WS-A::Upper() TO X.\n    MOVE WS-A::Lower() TO X.\n    MOVE WS-A::Lower() TO X.\n    COMPUTE N = WS-A::Len.\n    COMPUTE N = WS-A::Len().\n"
        );
        assert_eq!(r.edits.iter().map(|e| e.old.as_str()).collect::<Vec<_>>(), ["uppercase", "UPPERCASE", "tolowercase", "toLowerCase", "LENGTH", "length"]);
    }

    #[test]
    fn a_chain_is_rewritten_link_by_link() {
        let r = free("    MOVE WS-NAME::Trim()::UpperCase() TO WS-OUT.\n    MOVE WS-NAME::ToLowerCase()::Trim()::Length() TO WS-N.\n");
        assert_eq!(r.text, "    MOVE WS-NAME::Trim()::Upper() TO WS-OUT.\n    MOVE WS-NAME::Lower()::Trim()::Len() TO WS-N.\n");
        assert_eq!(r.edits.len(), 3);
    }

    #[test]
    fn a_subscript_and_a_reference_modification_do_not_hide_the_receiver() {
        let pred = items(&["WS-ROW", "WS-TEXT"]);
        let src = "    MOVE WS-ROW(2)::UpperCase() TO X.\n    MOVE WS-TEXT(1:3)::LowerCase() TO X.\n    MOVE WS-ROW(I J)::Length TO N.\n    MOVE WS-ROW (2) :: Length TO N.\n";
        let r = canonicalize_value_methods(src, SourceFormat::Free, &pred);
        assert_eq!(r.edits.len(), 4, "{:?}", r.text);
        assert!(r.text.contains("WS-ROW(2)::Upper()") && r.text.contains("WS-TEXT(1:3)::Lower()"));
    }

    #[test]
    fn it_is_idempotent() {
        let once = free("    MOVE WS-A::Trim()::ToUpperCase() TO X.\n    COMPUTE N = WS-A::Length.\n");
        let twice = free(&once.text);
        assert_eq!(twice.text, once.text);
        assert!(twice.edits.is_empty());
    }

    #[test]
    fn a_string_a_hex_and_a_national_literal_are_never_looked_into() {
        let src = "    DISPLAY \"WS-A::UpperCase()\" 'WS-B::Length' X\"41\" N\"WS-C::LowerCase\" \"it\"\"s::ToUpperCase\".\n    MOVE WS-A::UpperCase() TO X.\n";
        let r = free(src);
        assert_eq!(r.edits.len(), 1);
        assert_eq!(&r.text[..src.find("MOVE").unwrap()], &src[..src.find("MOVE").unwrap()]);
        assert!(r.text.contains("WS-A::Upper() TO X"));
    }

    #[test]
    fn a_comment_is_never_looked_into_in_any_form() {
        let r = free("    *> WS-A::UpperCase() in a full-line comment\n    MOVE WS-A::UpperCase() TO X. *> and WS-A::Length here\n");
        assert_eq!(r.text, "    *> WS-A::UpperCase() in a full-line comment\n    MOVE WS-A::Upper() TO X. *> and WS-A::Length here\n");

        // Fixed form: a `*`, `/` or `D` in column 7 makes the whole line a comment.
        let src = "000100*    MOVE WS-A::UpperCase() TO X.\n000200/    MOVE WS-A::UpperCase() TO X.\n000300D    MOVE WS-A::UpperCase() TO X.\n000400     MOVE WS-A::UpperCase() TO X.\n";
        let r = fixed(src);
        assert_eq!(r.edits.len(), 1, "{}", r.text);
        assert_eq!(r.edits[0].line, 4);
        assert_eq!(&r.text[..src.rfind("000400").unwrap()], &src[..src.rfind("000400").unwrap()]);
    }

    #[test]
    fn cobol_names_that_contain_the_words_and_the_intrinsic_functions_are_not_touched() {
        let src = "    COMPUTE N = FUNCTION LENGTH(WS-A).\n    MOVE FUNCTION UPPER-CASE(WS-A) TO X.\n    MOVE FUNCTION LOWER-CASE(WS-A) TO X.\n    MOVE WS-LENGTH TO UPPERCASE-FLAG.\n    MOVE WS-A::Length-2 TO X.\n    MOVE WS-A::Upper_Case TO X.\n    PERFORM LENGTH-CHECK.\n";
        let r = free(src);
        assert_eq!(r.text, src);
    }

    #[test]
    fn a_picture_is_not_touched() {
        let src = "01 WS-A PIC X(10) VALUE \"x\".\n01 WS-LENGTH PIC 9(4).\n";
        assert_eq!(free(src).text, src);
    }

    #[test]
    fn only_a_receiver_that_is_certainly_a_value_is_rewritten() {
        let pred = items(&["WS-NAME"]);
        let src = "\
    MOVE WS-NAME::UpperCase() TO X.
    MOVE BTN-1::Length TO X.
    MOVE me::Length TO X.
    MOVE super::UpperCase() TO X.
    MOVE GRD::Rows(1)::Length TO X.
    MOVE TXT-A::GetText()::UpperCase() TO X.
    MOVE LST::GetSelected()::toUpperCase() TO X.
    MOVE LST::Items(3)::Length TO X.
";
        let r = canonicalize_value_methods(src, SourceFormat::Free, &pred);
        let want = "\
    MOVE WS-NAME::Upper() TO X.
    MOVE BTN-1::Length TO X.
    MOVE me::Length TO X.
    MOVE super::UpperCase() TO X.
    MOVE GRD::Rows(1)::Length TO X.
    MOVE TXT-A::GetText()::Upper() TO X.
    MOVE LST::GetSelected()::Upper() TO X.
    MOVE LST::Items(3)::Length TO X.
";
        assert_eq!(r.text, want);
        assert_eq!(r.edits.len(), 3);
    }

    #[test]
    fn a_qualified_receiver_is_judged_by_the_item_it_names() {
        let pred = items(&["WS-NAME"]);
        let r = canonicalize_value_methods("    MOVE WS-NAME OF WS-REC::UpperCase() TO X.\n    MOVE OTHER OF WS-REC::UpperCase() TO X.\n", SourceFormat::Free, &pred);
        assert_eq!(r.edits.len(), 1);
        assert!(r.text.starts_with("    MOVE WS-NAME OF WS-REC::Upper()"));
    }

    #[test]
    fn two_edits_on_one_line_get_their_own_columns() {
        let r = free("    MOVE WS-A::UpperCase() TO WS-B::LowerCase().\n");
        assert_eq!(r.text, "    MOVE WS-A::Upper() TO WS-B::Lower().\n");
        assert_eq!(r.edits.len(), 2);
        assert_eq!((r.edits[0].line, r.edits[0].column), (1, 16));
        assert_eq!((r.edits[1].line, r.edits[1].column), (1, 37));
        assert_eq!(r.edits[0].offset, 15);
    }

    #[test]
    fn utf8_text_before_an_edit_keeps_every_offset_exact() {
        let src = "    DISPLAY \"Ação ñ ✓\" WS-A::UpperCase().\n    MOVE \"é\" TO WS-ç. MOVE WS-A::Length TO N.\n";
        let r = free(src);
        assert_eq!(r.text, "    DISPLAY \"Ação ñ ✓\" WS-A::Upper().\n    MOVE \"é\" TO WS-ç. MOVE WS-A::Len TO N.\n");
        // The column counts characters, not bytes.
        let line1 = "    DISPLAY \"Ação ñ ✓\" WS-A::";
        assert_eq!(r.edits[0].column as usize, line1.chars().count() + 1);
        assert_eq!(&src[r.edits[0].offset..r.edits[0].offset + 9], "UpperCase");
        assert_eq!(r.edits[1].line, 2);
        // A name with an accented letter right after the alias is another name.
        let r = free("    MOVE WS-A::UpperCaseé TO X.\n");
        assert!(!r.changed());
    }

    #[test]
    fn line_endings_are_preserved_byte_for_byte() {
        let r = free("    MOVE WS-A::UpperCase() TO X.\r\n    MOVE WS-A::Length TO N.\r\n\r\n    STOP RUN.");
        assert_eq!(r.text, "    MOVE WS-A::Upper() TO X.\r\n    MOVE WS-A::Len TO N.\r\n\r\n    STOP RUN.");
        // A carriage return after the alias does not hide it.
        assert_eq!(free("MOVE WS-A::Length\r\n").text, "MOVE WS-A::Len\r\n");
    }

    #[test]
    fn fixed_form_leaves_columns_1_to_6_alone_and_reads_to_the_end_of_the_line() {
        // Columns 1-6 are the sequence area: whatever is there is not source.
        let src = "WS-A::      MOVE WS-A::UpperCase() TO X.\n";
        let r = fixed(src);
        assert_eq!(r.edits.len(), 1);
        assert!(r.text.starts_with("WS-A::      MOVE WS-A::Upper()"));
        // PowerRustCOBOL's relaxed fixed form reads past column 72; the classic
        // reference format does not: columns 73-80 are an identification area.
        let mut line = String::from("       MOVE WS-A::Trim() TO X.");
        while line.len() < 72 {
            line.push(' ');
        }
        line.push_str("WS-A::Length");
        line.push('\n');
        assert_eq!(fixed(&line).edits.len(), 1, "relaxed fixed form reads column 73 on");
        assert_eq!(strict(&line).text, line, "the classic reference format does not");
    }

    #[test]
    fn a_word_cut_by_a_fixed_form_continuation_is_not_rewritten_from_its_first_half() {
        let src = "       MOVE WS-A::Length\n      -    ening TO X.\n";
        let r = strict(src);
        assert!(!r.changed(), "{}", r.text);
    }

    #[test]
    fn a_literal_continued_in_fixed_form_stays_a_literal() {
        let src = "       DISPLAY \"see WS-A::\n      -    \"UpperCase() here\".\n       MOVE WS-A::UpperCase() TO X.\n";
        let r = strict(src);
        assert_eq!(r.edits.len(), 1);
        assert_eq!(r.edits[0].line, 3);
    }

    #[test]
    fn embedded_blocks_are_not_cobol() {
        let src = "\
    EXEC RUST
        let n = text::Length;
        let s = WS-A::UpperCase();
    END-EXEC.
    MOVE WS-A::UpperCase() TO X.
    EXEC SQL SELECT 1 END-EXEC.
    MOVE WS-A::Length TO N.
";
        let r = free(src);
        assert_eq!(r.edits.len(), 2, "{}", r.text);
        assert!(r.text.contains("let s = WS-A::UpperCase();"));
        assert!(r.text.contains("MOVE WS-A::Upper() TO X.") && r.text.contains("MOVE WS-A::Len TO N."));
    }

    #[test]
    fn a_block_literal_and_pseudo_text_are_not_cobol() {
        let src = "\
    MOVE
    ```
    WS-A::UpperCase() and WS-A::Length
    ```
    TO WS-TEXT.
    MOVE WS-A::UpperCase() TO X.
    COPY BOOK REPLACING ==WS-A::Length== BY ==WS-B::LowerCase()==.
    MOVE WS-A::Length TO N.
";
        let r = free(src);
        assert_eq!(r.edits.len(), 2, "{}", r.text);
        assert!(r.text.contains("WS-A::UpperCase() and WS-A::Length"));
        assert!(r.text.contains("==WS-A::Length== BY ==WS-B::LowerCase()=="));
    }

    #[test]
    fn the_checker_accepts_what_the_rewrite_does_and_nothing_else() {
        let src = "    MOVE WS-A::Trim()::UpperCase() TO X. *> WS-A::Length stays\n    COMPUTE N = WS-A::Length.\n    DISPLAY \"WS-A::LowerCase()\".\n";
        let r = free(src);
        assert_eq!(r.edits.len(), 2);
        for fmt in [SourceFormat::Free, SourceFormat::Fixed] {
            assert!(differs_only_by_spellings(src, &r.text, fmt), "{fmt:?}");
        }
        // Unchanged text trivially qualifies.
        assert!(differs_only_by_spellings(src, src, SourceFormat::Free));
        // Anything else does not: a literal, a comment, another name, a new token.
        for bad in [
            src.replace("\"WS-A::LowerCase()\"", "\"WS-A::Lower()\""),
            src.replace("WS-A::Length stays", "WS-A::Len stays"),
            r.text.replace("TO X", "TO Y"),
            r.text.replace("::Upper()", "::Foo()"),
            r.text.replace("::Upper()", "::Upper()::Trim()"),
            r.text.replace("::Len.", "::Len"),
        ] {
            assert!(!differs_only_by_spellings(src, &bad, SourceFormat::Free), "{bad}");
        }
    }

    #[test]
    fn nothing_to_do_returns_the_text_unchanged() {
        for src in ["", "\n", "    STOP RUN.", "    MOVE A TO B"] {
            let r = free(src);
            assert_eq!(r.text, src);
            assert!(r.edits.is_empty());
        }
    }
}
