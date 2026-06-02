// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Source location types used throughout the Cobolt pipeline.
//!
//! Every token carries a [`Span`] so the IDE can underline diagnostics and
//! the parser can emit precise error messages.

use crate::Token;
use serde::{Deserialize, Serialize};

// ── Span ──────────────────────────────────────────────────────────────────────

/// A half-open byte range `[start, end)` inside the original source string,
/// plus the 1-based line and column of the *start* position.
///
/// Byte offsets are preferred over char offsets because they are O(1) to
/// produce from a logos lexer and sufficient for slicing `&str`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Span {
    /// Byte offset of the first character of the token (inclusive).
    pub start: usize,
    /// Byte offset one past the last character of the token (exclusive).
    pub end: usize,
    /// 1-based line number of `start`.
    pub line: u32,
    /// 1-based column number of `start` (in bytes, not chars).
    pub col: u32,
}

impl Span {
    /// Construct a span explicitly.
    pub fn new(start: usize, end: usize, line: u32, col: u32) -> Self {
        Self { start, end, line, col }
    }

    /// A dummy span used as a placeholder before positions are resolved.
    pub fn dummy() -> Self {
        Self { start: 0, end: 0, line: 0, col: 0 }
    }

    /// Length of the token in bytes.
    pub fn len(&self) -> usize {
        self.end.saturating_sub(self.start)
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Merge two spans into a span that covers both (used by the parser to
    /// attach source locations to multi-token AST nodes).
    pub fn merge(self, other: Span) -> Span {
        Span {
            start: self.start.min(other.start),
            end: self.end.max(other.end),
            line: self.line.min(other.line),
            col: if self.start <= other.start { self.col } else { other.col },
        }
    }

    /// Extract the original source text that this span covers.
    ///
    /// Returns `None` if the span is out of bounds (should not happen in
    /// normal operation).
    pub fn text<'src>(&self, source: &'src str) -> Option<&'src str> {
        source.get(self.start..self.end)
    }
}

impl std::fmt::Display for Span {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}:{}", self.line, self.col)
    }
}

// ── SpannedToken ──────────────────────────────────────────────────────────────

/// A [`Token`] paired with the [`Span`] that identifies its location in source.
///
/// This is the primary output type of the lexer — everything downstream
/// (parser, IDE diagnostics, syntax highlighter) consumes `SpannedToken`s.
#[derive(Debug, Clone, PartialEq)]
pub struct SpannedToken {
    pub token: Token,
    pub span: Span,
}

impl SpannedToken {
    pub fn new(token: Token, span: Span) -> Self {
        Self { token, span }
    }

    /// `true` if this token should be invisible to the parser (comments,
    /// whitespace-only tokens).  The lexer filters these by default; this
    /// method is provided for tools that want the raw stream.
    pub fn is_trivia(&self) -> bool {
        matches!(self.token, Token::Comment(_))
    }
}

// ── LineIndex ─────────────────────────────────────────────────────────────────

/// Precomputed line-start byte offsets for a source file.
///
/// Constructed once from the source text; allows O(log n) conversion between
/// a byte offset and a `(line, col)` pair.
#[derive(Debug, Clone)]
pub struct LineIndex {
    /// `starts[i]` is the byte offset of the first character of line `i+1`.
    starts: Vec<usize>,
}

impl LineIndex {
    /// Build a `LineIndex` from a complete source string.
    pub fn new(source: &str) -> Self {
        let mut starts = vec![0usize];
        for (i, b) in source.bytes().enumerate() {
            if b == b'\n' {
                starts.push(i + 1);
            }
        }
        Self { starts }
    }

    /// Convert a byte offset to a 1-based `(line, col)` pair.
    pub fn line_col(&self, offset: usize) -> (u32, u32) {
        // Binary search for the line that contains `offset`.
        let line_idx = match self.starts.binary_search(&offset) {
            Ok(i) => i,
            Err(i) => i.saturating_sub(1),
        };
        let line = (line_idx + 1) as u32;
        let col = (offset - self.starts[line_idx] + 1) as u32;
        (line, col)
    }

    /// Total number of lines in the source.
    pub fn line_count(&self) -> u32 {
        self.starts.len() as u32
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn line_index_basic() {
        let src = "MOVE A TO B.\nADD 1 TO C.\n";
        let idx = LineIndex::new(src);
        assert_eq!(idx.line_col(0), (1, 1));   // 'M'
        assert_eq!(idx.line_col(13), (2, 1));  // 'A' of ADD
        assert_eq!(idx.line_col(14), (2, 2));  // 'D'
    }

    #[test]
    fn span_merge() {
        let a = Span::new(0, 4, 1, 1);
        let b = Span::new(8, 12, 1, 9);
        let m = a.merge(b);
        assert_eq!(m.start, 0);
        assert_eq!(m.end, 12);
    }

    #[test]
    fn span_text() {
        let src = "MOVE WS-A TO WS-B.";
        let sp = Span::new(5, 9, 1, 6);
        assert_eq!(sp.text(src), Some("WS-A"));
    }
}
