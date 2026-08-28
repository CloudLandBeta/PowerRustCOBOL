// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! The main Cobolt COBOL lexer.
//!
//! [`Lexer`] preprocesses the source (stripping fixed-form dead zones,
//! converting column-7 comment indicators to `*>` markers), then runs
//! a `logos` tokenizer over the *preprocessed* text and applies a second
//! classification pass to produce the final [`Token`] / [`SpannedToken`] stream.
//!
//! # Usage
//!
//! ```rust
//! use cobolt_lexer::{Lexer, SourceFormat, Token};
//!
//! let src = "       MOVE WS-COUNT TO WS-TOTAL.\n";
//! let mut lexer = Lexer::new(src, SourceFormat::Fixed);
//! while let Some(st) = lexer.next_token() {
//!     if st.token == Token::Eof { break; }
//!     println!("{:?}", st);
//! }
//! ```

use logos::Logos;
use std::ops::Range;

use crate::{
    keywords,
    source::{flatten_fixed, flatten_fixed_strict, SourceFormat},
    span::{LineIndex, Span, SpannedToken},
    token::{RawToken, Token},
};

/// Parse a `digits.digits` literal into an exact `(mantissa, scale)` fixed-point
/// decimal. Returns `None` only if the combined digits overflow `i128`.
fn parse_decimal_token(text: &str) -> Option<(i128, u8)> {
    let (int_s, frac_s) = text.split_once('.')?;
    let scale = frac_s.len().min(u8::MAX as usize) as u8;
    let digits = format!("{int_s}{}", &frac_s[..scale as usize]);
    let mantissa: i128 = digits.parse().ok()?;
    Some((mantissa, scale))
}

// ── LexError ──────────────────────────────────────────────────────────────────

/// Errors that can be produced by the lexer.
///
/// The lexer never panics; unrecognised input is wrapped in [`Token::Error`]
/// and reported here for tools that want a separate error channel.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum LexError {
    #[error("unexpected character(s) at {span}: `{text}`")]
    UnexpectedChar { span: Span, text: String },

    #[error("unterminated string literal at {span}")]
    UnterminatedString { span: Span },

    #[error("integer literal out of range at {span}: `{text}`")]
    IntegerOverflow { span: Span, text: String },
}

// ── Lexer ─────────────────────────────────────────────────────────────────────

/// COBOL source tokenizer.
///
/// Implements `Iterator<Item = SpannedToken>` — collecting to `Vec` gives you
/// the full token stream.  Errors are embedded as [`Token::Error`] tokens;
/// check [`Lexer::errors`] after tokenization to inspect them separately.
pub struct Lexer<'src> {
    /// Preprocessed source (fixed-form flattened, or original for free-form).
    /// Stored so error recovery can extract text slices by byte range.
    preprocessed: String,
    /// Original raw source reference — kept only to satisfy the lifetime parameter.
    #[allow(dead_code)]
    original: &'src str,
    /// Line index built from the preprocessed source for offset → line/col.
    line_index: LineIndex,
    /// Raw tokens pre-collected from the logos lexer.
    ///
    /// We collect eagerly so that the logos lexer (which borrows from
    /// `preprocessed`) is dropped before `preprocessed` is moved into `Self`,
    /// avoiding a self-referential struct.  RawToken variants own their string
    /// data (String fields), so this is safe.
    raw_tokens: Vec<(Result<RawToken, ()>, Range<usize>)>,
    /// Cursor into `raw_tokens`.
    pos: usize,
    /// Errors accumulated during tokenization.
    errors: Vec<LexError>,
    /// Whether to include [`Token::Comment`] in the output stream.
    /// Default: `false` (parser doesn't need them; IDE tools set it to `true`).
    emit_comments: bool,
    /// Peeked-ahead token (used by `peek()`).
    peeked: Option<SpannedToken>,
    /// `true` after the first `Token::Eof` has been returned.
    done: bool,
    /// The source format this lexer was built for. The block-literal fence is
    /// a FREE-format construct: fixed format has an indicator column and a
    /// sequence area, so a line of backticks there means something else.
    format: SourceFormat,
    /// `true` at the very start of input, after a `Newline`, or after a
    /// `Period`.  Used to distinguish level-number literals (which appear
    /// only at the start of a data-description entry) from plain integers.
    at_line_start: bool,
}

impl<'src> Lexer<'src> {
    /// Create a new lexer for the given source text and format.
    pub fn new(source: &'src str, format: SourceFormat) -> Self {
        let preprocessed = match format {
            SourceFormat::Fixed => flatten_fixed(source),
            SourceFormat::FixedStrict => flatten_fixed_strict(source),
            SourceFormat::Free => source.to_string(),
        };
        let line_index = LineIndex::new(&preprocessed);

        // Tokenize the preprocessed source and eagerly collect into a Vec.
        // The logos lexer borrows `preprocessed` only for this block; after
        // `raw_tokens` is built the borrow ends and `preprocessed` can be
        // moved into the struct.
        let raw_tokens: Vec<(Result<RawToken, ()>, Range<usize>)> = {
            let mut lex = RawToken::lexer(&preprocessed);
            let mut v = Vec::new();
            while let Some(res) = lex.next() {
                v.push((res.map_err(|_| ()), lex.span()));
            }
            v
        };

        Self {
            preprocessed,
            original: source,
            line_index,
            raw_tokens,
            pos: 0,
            errors: Vec::new(),
            emit_comments: false,
            peeked: None,
            done: false,
            format,
            at_line_start: true,
        }
    }

    /// Enable comment tokens in the output stream.
    pub fn with_comments(mut self) -> Self {
        self.emit_comments = true;
        self
    }

    /// Return all lexer errors accumulated so far.
    pub fn errors(&self) -> &[LexError] {
        &self.errors
    }

    /// Peek at the next token without consuming it.
    pub fn peek(&mut self) -> Option<&SpannedToken> {
        if self.peeked.is_none() {
            self.peeked = self.next_token();
        }
        self.peeked.as_ref()
    }

    /// Advance and return the next [`SpannedToken`].
    ///
    /// Returns `Some(SpannedToken { token: Token::Eof, .. })` at end of input,
    /// then `None` on subsequent calls.
    pub fn next_token(&mut self) -> Option<SpannedToken> {
        if let Some(tok) = self.peeked.take() {
            return Some(tok);
        }

        if self.done {
            return None;
        }

        loop {
            // End of pre-collected token stream → emit Eof.
            if self.pos >= self.raw_tokens.len() {
                self.done = true;
                let len = self.preprocessed.len();
                let span = self.make_span(len, len);
                return Some(SpannedToken::new(Token::Eof, span));
            }

            // Clone the entry so we don't hold a borrow while calling classify.
            let (result, range) = self.raw_tokens[self.pos].clone();
            self.pos += 1;

            // ── Separator comma / semicolon (COBOL-85) ─────────────────────
            //
            // A `,` or `;` FOLLOWED BY A SPACE (or end of line) is a
            // *separator*: pure punctuation that may appear anywhere a space
            // may appear, and that means exactly what a space means. So
            // `MOVE ZERO TO DN3, DN4.` is the same statement as
            // `MOVE ZERO TO DN3 DN4.`, and a conforming compiler cannot tell
            // them apart.
            //
            // It is dropped HERE, before `classify`, so it never becomes a
            // token and never touches `at_line_start` — "means what a space
            // means" is stronger than "a token the parser ignores", and only
            // the first spelling keeps every future syntactic site correct
            // without listing them.
            //
            // The rule is one-sided on purpose. A comma that is NOT followed
            // by a space is never a separator, and that is precisely where the
            // two constructs that genuinely need a comma live: the decimal
            // comma (`1,5`, glued between digits, under `DECIMAL-POINT IS
            // COMMA`) and the PICTURE editing comma (`PIC ZZ,ZZ9`, glued
            // inside the template). Both keep their token untouched.
            if matches!(result, Ok(RawToken::Comma) | Ok(RawToken::Semicolon))
                && self.is_separator_punctuation(range.end)
            {
                continue;
            }

            let span = self.make_span(range.start, range.end);

            let token = match result {
                Err(()) => {
                    // Unexpected character — extract slice from preprocessed source.
                    let text = self.preprocessed.get(range).unwrap_or("?").to_string();
                    // A lone quotation mark is not a stray character: it is a
                    // literal that was never closed on its line. Naming it as
                    // such is what turns "unexpected character" into something
                    // the developer can act on — and a literal cannot span
                    // lines, so an unclosed one is always a mistake.
                    if text.starts_with('"') || text.starts_with('\'') {
                        self.errors.push(LexError::UnterminatedString { span });
                    } else {
                        self.errors.push(LexError::UnexpectedChar {
                            span,
                            text: text.clone(),
                        });
                    }
                    self.at_line_start = false;
                    Token::Error(text)
                }
                Ok(raw) => {
                    let tok = self.classify(raw, span);
                    // Update line-start flag for the next token.
                    match &tok {
                        // A real newline (empty comment) or period resets the flag.
                        Token::Comment(s) if s.is_empty() => {
                            self.at_line_start = true;
                        }
                        Token::Period => {
                            self.at_line_start = true;
                        }
                        // Non-empty comments don't change the flag (they don't
                        // consume a "slot" on the logical line).
                        Token::Comment(_) => {}
                        // Any real token clears line-start.
                        _ => {
                            self.at_line_start = false;
                        }
                    }
                    tok
                }
            };

            if matches!(token, Token::Comment(_)) && !self.emit_comments {
                continue;
            }

            // ── EXEC RUST … END-EXEC block capture ────────────────────────
            // When we see `EXEC`, look ahead in the raw token stream for the
            // word `RUST`.  If found, we slice the preprocessed source between
            // the end of `RUST` and the start of `END-EXEC` to capture the
            // verbatim Rust source, then return a single ExecRustBlock token
            // spanning the entire construct.
            if token == Token::Exec {
                if let Some(block) = self.try_capture_exec_rust(span) {
                    return Some(block);
                }
                // Not followed by RUST — emit standalone Exec token and
                // let the parser diagnose the error.
            }

            // ── Free-format block literal ─────────────────────────────────
            // A fence opens a literal whose text is the lines between the
            // fences, so the whole construct becomes one StringLiteral.
            if token == Token::Fence {
                return Some(self.capture_block_literal(span));
            }

            // ── A digit-leading word whose first letter is behind a hyphen ──
            //
            // `3-DEM-TBL` is a legal COBOL-85 user-defined word, but it cannot
            // be matched by the `Word` regex: a `logos` DFA does not backtrack,
            // so a pattern that allows a hyphen before the first letter eats
            // `9999-` out of `PIC 9999-.` and then fails. See token.rs.
            //
            // Here the pieces have already been lexed as `3` `-` `DEM-TBL`, and
            // their spans say whether the developer wrote them **glued**. Only
            // a glued run becomes one word, which is exactly COBOL-85's rule:
            // an arithmetic operator must have spaces around it, so `B - C` is
            // a subtraction and `B-C` is a name. `PIC 9999-.` is untouched
            // because no word follows its hyphen.
            if let Token::IntegerLiteral(..) | Token::LevelNumber(_) = token {
                if let Some(joined) = self.try_join_digit_leading_word(span) {
                    return Some(joined);
                }
            }

            return Some(SpannedToken::new(token, span));
        }
    }

    /// Classify a [`RawToken`] into the final [`Token`].
    fn classify(&mut self, raw: RawToken, span: Span) -> Token {
        match raw {
            RawToken::Newline => {
                // Newlines are consumed for line tracking via LineIndex.
                // Return an empty comment that gets filtered by emit_comments gate.
                Token::Comment(String::new())
            }

            RawToken::FreeComment(text) => Token::Comment(text),

            // A hex literal IS a character-string literal — it just spells its
            // characters in hex — so it becomes the same token and works
            // anywhere a quoted literal does (DELIMITED BY, MOVE, VALUE, …).
            RawToken::HexString(s) => Token::StringLiteral(s),

            RawToken::StringDouble(s) | RawToken::StringSingle(s) => Token::StringLiteral(s),

            RawToken::Float(Some(text)) => {
                // Parse the raw digits into an exact (mantissa, scale) fixed-point
                // decimal. The regex guarantees `digits.digits`, so this only
                // fails if the value overflows i128 (~38 digits).
                match parse_decimal_token(&text) {
                    Some((mantissa, scale)) => Token::DecimalLiteral { mantissa, scale },
                    None => {
                        self.errors.push(LexError::IntegerOverflow {
                            span,
                            text: text.clone(),
                        });
                        Token::Error(text)
                    }
                }
            }
            RawToken::Float(None) => {
                let text = self
                    .preprocessed
                    .get(span.start..span.end)
                    .unwrap_or("?")
                    .to_string();
                self.errors.push(LexError::IntegerOverflow {
                    span,
                    text: text.clone(),
                });
                Token::Error(text)
            }

            RawToken::Integer(Some(n)) => {
                // A number is a level-number only when it appears at the start
                // of a line (after Newline, Period, or at the beginning of input).
                // Everywhere else (e.g. `ADD 42 TO X`) it is an IntegerLiteral.
                if self.at_line_start && keywords::is_level_number(n) {
                    Token::LevelNumber(n as u8)
                } else {
                    // The raw token's regex is `[0-9]+`, so its span length is
                    // exactly how many digits were written — leading zeros
                    // included, which is the whole point of keeping it.
                    let digits = span.end.saturating_sub(span.start).min(u8::MAX as usize) as u8;
                    Token::IntegerLiteral(n as i64, digits)
                }
            }
            RawToken::Integer(None) => {
                let text = self
                    .preprocessed
                    .get(span.start..span.end)
                    .unwrap_or("?")
                    .to_string();
                self.errors.push(LexError::IntegerOverflow {
                    span,
                    text: text.clone(),
                });
                Token::Error(text)
            }

            RawToken::Word(w) => {
                let upper = w.to_ascii_uppercase();
                match keywords::lookup(&upper) {
                    Some(kw) => kw,
                    None => {
                        let name = upper.trim_end_matches('-').to_string();
                        Token::Identifier(name)
                    }
                }
            }

            RawToken::Power => Token::Power,
            RawToken::LtEq => Token::LtEq,
            RawToken::GtEq => Token::GtEq,
            RawToken::NotEq => Token::NotEq,
            RawToken::Eq => Token::Eq,
            RawToken::Lt => Token::Lt,
            RawToken::Gt => Token::Gt,
            RawToken::Plus => Token::Plus,
            RawToken::Minus => Token::Minus,
            RawToken::Star => Token::Star,
            RawToken::Slash => Token::Slash,
            RawToken::Ampersand => Token::Ampersand,

            RawToken::Period => Token::Period,
            RawToken::Fence => Token::Fence,
            RawToken::Comma => Token::Comma,
            RawToken::Semicolon => Token::Semicolon,
            RawToken::LParen => Token::LParen,
            RawToken::RParen => Token::RParen,
            RawToken::Colon => Token::Colon,
        }
    }

    /// Attempt to capture an `EXEC RUST … END-EXEC` block.
    ///
    /// Called immediately after the lexer has classified a [`Token::Exec`].
    /// `exec_span` is the span of the `EXEC` keyword itself.
    ///
    /// * Scans forward in `raw_tokens` (skipping newlines) for the word `RUST`.
    /// * If found, continues scanning until a `Word` that uppercases to
    ///   `"END-EXEC"` is encountered.
    /// * Slices `self.preprocessed` between the end of `RUST` and the start of
    ///   `END-EXEC` to obtain the verbatim Rust source.
    /// * Advances `self.pos` past `END-EXEC`.
    /// * Returns a [`SpannedToken`] carrying [`Token::ExecRustBlock`].
    ///
    /// Returns `None` if `EXEC` is NOT followed by `RUST`, leaving `self.pos`
    /// unchanged so the caller can emit a plain [`Token::Exec`].
    fn try_capture_exec_rust(&mut self, exec_span: Span) -> Option<SpannedToken> {
        let mut look = self.pos; // self.pos already points past EXEC

        // Skip leading newlines (horizontal whitespace is already consumed by logos)
        while look < self.raw_tokens.len() {
            match &self.raw_tokens[look].0 {
                Ok(RawToken::Newline) => look += 1,
                _ => break,
            }
        }

        // Next meaningful raw token must be Word("RUST")
        if look >= self.raw_tokens.len() {
            return None;
        }
        let is_rust = match &self.raw_tokens[look].0 {
            Ok(RawToken::Word(w)) => w.to_ascii_uppercase() == "RUST",
            _ => false,
        };
        if !is_rust {
            return None;
        }

        // The Rust source starts immediately after the "RUST" word.
        let rust_src_start = self.raw_tokens[look].1.end;
        look += 1; // advance past RUST

        // Scan forward for END-EXEC
        while look < self.raw_tokens.len() {
            if let Ok(RawToken::Word(w)) = &self.raw_tokens[look].0 {
                if w.to_ascii_uppercase() == "END-EXEC" {
                    let rust_src_end = self.raw_tokens[look].1.start;
                    let end_exec_end = self.raw_tokens[look].1.end;

                    // Slice the raw Rust source from the preprocessed string.
                    let rust_source = self
                        .preprocessed
                        .get(rust_src_start..rust_src_end)
                        .unwrap_or("")
                        .trim()
                        .to_string();

                    // Advance the lexer cursor past END-EXEC.
                    self.pos = look + 1;
                    self.at_line_start = false;

                    // Build a span that covers the whole EXEC RUST … END-EXEC.
                    let block_span =
                        Span::new(exec_span.start, end_exec_end, exec_span.line, exec_span.col);
                    return Some(SpannedToken::new(
                        Token::ExecRustBlock(rust_source),
                        block_span,
                    ));
                }
            }
            look += 1;
        }

        // Unterminated block — report an error and consume to EOF.
        self.errors.push(LexError::UnexpectedChar {
            span: exec_span,
            text: "unterminated EXEC RUST block (missing END-EXEC)".into(),
        });
        self.pos = self.raw_tokens.len();
        Some(SpannedToken::new(
            Token::Error("unterminated EXEC RUST block".into()),
            exec_span,
        ))
    }

    /// Join `<digits>` `-` `<word>` into one user-defined word when the three
    /// are written with no space between them.
    ///
    /// Returns `None` — consuming nothing — unless the whole run is glued, so
    /// `9999 - X` stays a subtraction and `PIC 9999-.` stays an integer
    /// followed by a hyphen.
    fn try_join_digit_leading_word(&mut self, num_span: Span) -> Option<SpannedToken> {
        let (minus, minus_range) = self.raw_tokens.get(self.pos)?.clone();
        if !matches!(minus, Ok(RawToken::Minus)) || minus_range.start != num_span.end {
            return None;
        }
        let (word, word_range) = self.raw_tokens.get(self.pos + 1)?.clone();
        let text = match word {
            Ok(RawToken::Word(w)) => w,
            _ => return None,
        };
        if word_range.start != minus_range.end {
            return None;
        }

        let digits = self.preprocessed.get(num_span.start..num_span.end)?.to_string();
        let end = word_range.end;
        self.pos += 2;
        self.at_line_start = false;
        let joined = format!("{digits}-{text}");
        let span = Span::new(num_span.start, end, num_span.line, num_span.col);
        Some(SpannedToken::new(Token::Identifier(joined), span))
    }

    /// Capture a **block literal**: the lines between a pair of ``` fences.
    ///
    /// ```text
    ///     MOVE
    ///     ```
    ///     Hello, World!
    ///     ```
    ///     TO WS-GREETING.
    /// ```
    ///
    /// The value is `Hello, World!`.
    ///
    /// # The rules, and why each one
    ///
    /// * **The text starts on the line after the opening fence.** Anything
    ///   typed after ``` on the opening line is not content — it is the place a
    ///   language tag would go in Markdown, which is the notation this borrows.
    /// * **The closing fence's own line is not content**, and neither is the
    ///   newline that precedes it. So a one-line block is exactly that line,
    ///   with no trailing newline: the common case behaves as if the developer
    ///   had typed a quoted literal.
    /// * **Interior newlines are kept**, which is the entire point — COBOL-85
    ///   has no multi-line literal at all, and fixed-format continuation is
    ///   unavailable in free format.
    /// * **No escaping.** The text is taken verbatim, so quotation marks and
    ///   apostrophes need no doubling. That is what makes it useful for JSON,
    ///   SQL and HTML.
    ///
    /// This is a **language extension**, not COBOL-85.
    fn capture_block_literal(&mut self, fence_span: Span) -> SpannedToken {
        // Fixed format has an indicator column and a sequence area, so a line
        // of backticks there is not this construct. Say so rather than
        // silently producing a literal the developer did not ask for.
        if self.format != SourceFormat::Free {
            self.errors.push(LexError::UnexpectedChar {
                span: fence_span,
                text: "a ``` block literal is a free-format construct; this source is fixed format"
                    .into(),
            });
            return SpannedToken::new(
                Token::Error("``` block literal requires free format".into()),
                fence_span,
            );
        }

        // Content begins after the opening fence's line ends.
        let after_fence = &self.preprocessed[fence_span.end..];
        let Some(nl) = after_fence.find('\n') else {
            return self.unterminated_block(fence_span);
        };
        let content_start = fence_span.end + nl + 1;

        // Find the closing fence: a line whose first non-blank text is ```.
        let mut cursor = content_start;
        let rest = &self.preprocessed[content_start..];
        for line in rest.split_inclusive('\n') {
            if line.trim_start().starts_with("```") {
                // Content ends at the newline before this line — excluded.
                let content_end = cursor.saturating_sub(1).max(content_start);
                let text = self.preprocessed[content_start..content_end].to_string();
                let close_end = cursor + line.trim_end_matches('\n').len();

                // Skip every raw token the block swallowed.
                while self.pos < self.raw_tokens.len()
                    && self.raw_tokens[self.pos].1.start < close_end
                {
                    self.pos += 1;
                }
                self.at_line_start = false;
                let span = Span::new(
                    fence_span.start,
                    close_end,
                    fence_span.line,
                    fence_span.col,
                );
                return SpannedToken::new(Token::StringLiteral(text), span);
            }
            cursor += line.len();
        }
        self.unterminated_block(fence_span)
    }

    /// An opening fence with no closing fence. Reported where it was opened —
    /// the end of the file is not the useful place to look.
    fn unterminated_block(&mut self, fence_span: Span) -> SpannedToken {
        self.errors.push(LexError::UnexpectedChar {
            span: fence_span,
            text: "unterminated ``` block literal (missing closing ```)".into(),
        });
        self.pos = self.raw_tokens.len();
        SpannedToken::new(
            Token::Error("unterminated ``` block literal".into()),
            fence_span,
        )
    }

    fn make_span(&self, start: usize, end: usize) -> Span {
        let (line, col) = self.line_index.line_col(start);
        Span::new(start, end, line, col)
    }

    /// True when the punctuation ending at `end` is a COBOL-85 **separator** —
    /// that is, when it is followed by a space or by the end of the line.
    ///
    /// End of input counts: there is no character after it, which is the same
    /// thing the standard's "followed by a space" is getting at.
    fn is_separator_punctuation(&self, end: usize) -> bool {
        match self.preprocessed[end..].chars().next() {
            None => true,
            Some(c) => c.is_whitespace(),
        }
    }
}

// ── Iterator impl ─────────────────────────────────────────────────────────────

impl<'src> Iterator for Lexer<'src> {
    type Item = SpannedToken;

    fn next(&mut self) -> Option<Self::Item> {
        let st = self.next_token()?;
        if st.token == Token::Eof {
            None
        } else {
            Some(st)
        }
    }
}

// ── Convenience functions ─────────────────────────────────────────────────────

/// Tokenize a complete COBOL source string and return all tokens (no comments).
///
/// # Example
/// ```rust
/// use cobolt_lexer::{tokenize, SourceFormat, Token};
///
/// let tokens = tokenize("       MOVE 1 TO WS-X.", SourceFormat::Fixed);
/// assert!(tokens.iter().any(|st| st.token == Token::Move));
/// ```
pub fn tokenize(source: &str, format: SourceFormat) -> Vec<SpannedToken> {
    let lexer = Lexer::new(source, format);
    let pre = lexer.preprocessed.clone();
    let mut toks: Vec<SpannedToken> = lexer.collect();
    reclassify_member_words(&mut toks, &pre);
    toks
}

/// Like [`tokenize`] but includes comment tokens in the output.
pub fn tokenize_with_comments(source: &str, format: SourceFormat) -> Vec<SpannedToken> {
    let lexer = Lexer::new(source, format).with_comments();
    let pre = lexer.preprocessed.clone();
    let mut toks: Vec<SpannedToken> = lexer.collect();
    reclassify_member_words(&mut toks, &pre);
    toks
}

/// A word following the member-access operator `::` is a control property or
/// method **name**, never a COBOL keyword — so `Grid::Rows(I)::Delete()` and
/// `obj::Value` must work even though `DELETE`/`VALUE` are keywords. `::` appears
/// in no other COBOL construct, so we reclassify any keyword token immediately
/// after a `:: ` pair back into a [`Token::Identifier`], recovering its spelling
/// from the (preprocessed) source. Identifier / string members are left as-is.
fn reclassify_member_words(toks: &mut [SpannedToken], pre: &str) {
    for i in 2..toks.len() {
        if toks[i - 1].token != Token::Colon || toks[i - 2].token != Token::Colon {
            continue;
        }
        if matches!(
            toks[i].token,
            Token::Identifier(_) | Token::StringLiteral(_)
        ) {
            continue;
        }
        let Some(text) = pre.get(toks[i].span.start..toks[i].span.end) else {
            continue;
        };
        let t = text.trim();
        let is_word = t.chars().next().is_some_and(|c| c.is_ascii_alphabetic())
            && t.chars().all(|c| c.is_ascii_alphanumeric() || c == '-');
        if is_word {
            let name = t.to_ascii_uppercase().trim_end_matches('-').to_string();
            toks[i].token = Token::Identifier(name);
        }
    }
}
