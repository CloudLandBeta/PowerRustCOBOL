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
    source::{flatten_fixed, SourceFormat},
    span::{LineIndex, Span, SpannedToken},
    token::{RawToken, Token},
};

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
            SourceFormat::Free  => source.to_string(),
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

            let span = self.make_span(range.start, range.end);

            let token = match result {
                Err(()) => {
                    // Unexpected character — extract slice from preprocessed source.
                    let text = self.preprocessed
                        .get(range)
                        .unwrap_or("?")
                        .to_string();
                    self.errors.push(LexError::UnexpectedChar { span, text: text.clone() });
                    self.at_line_start = false;
                    Token::Error(text)
                }
                Ok(raw) => {
                    let tok = self.classify(raw, span);
                    // Update line-start flag for the next token.
                    match &tok {
                        // A real newline (empty comment) or period resets the flag.
                        Token::Comment(s) if s.is_empty() => { self.at_line_start = true; }
                        Token::Period                      => { self.at_line_start = true; }
                        // Non-empty comments don't change the flag (they don't
                        // consume a "slot" on the logical line).
                        Token::Comment(_) => {}
                        // Any real token clears line-start.
                        _ => { self.at_line_start = false; }
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

            RawToken::StringDouble(s) | RawToken::StringSingle(s) => {
                Token::StringLiteral(s)
            }

            RawToken::Float(Some(v)) => Token::FloatLiteral(v),
            RawToken::Float(None) => {
                let text = self.preprocessed
                    .get(span.start..span.end)
                    .unwrap_or("?")
                    .to_string();
                self.errors.push(LexError::IntegerOverflow { span, text: text.clone() });
                Token::Error(text)
            }

            RawToken::Integer(Some(n)) => {
                // A number is a level-number only when it appears at the start
                // of a line (after Newline, Period, or at the beginning of input).
                // Everywhere else (e.g. `ADD 42 TO X`) it is an IntegerLiteral.
                if self.at_line_start && keywords::is_level_number(n) {
                    Token::LevelNumber(n as u8)
                } else {
                    Token::IntegerLiteral(n as i64)
                }
            }
            RawToken::Integer(None) => {
                let text = self.preprocessed
                    .get(span.start..span.end)
                    .unwrap_or("?")
                    .to_string();
                self.errors.push(LexError::IntegerOverflow { span, text: text.clone() });
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

            RawToken::Power    => Token::Power,
            RawToken::LtEq     => Token::LtEq,
            RawToken::GtEq     => Token::GtEq,
            RawToken::NotEq    => Token::NotEq,
            RawToken::Eq       => Token::Eq,
            RawToken::Lt       => Token::Lt,
            RawToken::Gt       => Token::Gt,
            RawToken::Plus     => Token::Plus,
            RawToken::Minus    => Token::Minus,
            RawToken::Star     => Token::Star,
            RawToken::Slash    => Token::Slash,

            RawToken::Period    => Token::Period,
            RawToken::Comma     => Token::Comma,
            RawToken::Semicolon => Token::Semicolon,
            RawToken::LParen    => Token::LParen,
            RawToken::RParen    => Token::RParen,
            RawToken::Colon     => Token::Colon,
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
                    let end_exec_end  = self.raw_tokens[look].1.end;

                    // Slice the raw Rust source from the preprocessed string.
                    let rust_source = self.preprocessed
                        .get(rust_src_start..rust_src_end)
                        .unwrap_or("")
                        .trim()
                        .to_string();

                    // Advance the lexer cursor past END-EXEC.
                    self.pos = look + 1;
                    self.at_line_start = false;

                    // Build a span that covers the whole EXEC RUST … END-EXEC.
                    let block_span = Span::new(
                        exec_span.start,
                        end_exec_end,
                        exec_span.line,
                        exec_span.col,
                    );
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

    fn make_span(&self, start: usize, end: usize) -> Span {
        let (line, col) = self.line_index.line_col(start);
        Span::new(start, end, line, col)
    }
}

// ── Iterator impl ─────────────────────────────────────────────────────────────

impl<'src> Iterator for Lexer<'src> {
    type Item = SpannedToken;

    fn next(&mut self) -> Option<Self::Item> {
        let st = self.next_token()?;
        if st.token == Token::Eof { None } else { Some(st) }
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
    Lexer::new(source, format).collect()
}

/// Like [`tokenize`] but includes comment tokens in the output.
pub fn tokenize_with_comments(source: &str, format: SourceFormat) -> Vec<SpannedToken> {
    Lexer::new(source, format).with_comments().collect()
}
