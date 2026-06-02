// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! # cobolt-lexer
//!
//! Fujitsu COBOL tokenizer for the Cobolt IDE.
//!
//! ## Quick start
//!
//! ```rust
//! use cobolt_lexer::{tokenize, SourceFormat, Token};
//!
//! let source = r#"
//!        IDENTIFICATION DIVISION.
//!        PROGRAM-ID. HELLO-WORLD.
//!        PROCEDURE DIVISION.
//!        MAIN-PROC.
//!            DISPLAY "Hello, World!"
//!            STOP RUN.
//! "#;
//!
//! let tokens = tokenize(source, SourceFormat::Fixed);
//! let kinds: Vec<&Token> = tokens.iter().map(|st| &st.token).collect();
//!
//! assert!(kinds.contains(&&Token::Identification));
//! assert!(kinds.contains(&&Token::ProgramId));
//! assert!(kinds.contains(&&Token::Stop));
//! ```
//!
//! ## Architecture
//!
//! The lexer operates in two passes:
//!
//! 1. **Source preprocessing** ([`source`] module) — strips fixed-form column
//!    areas (sequence numbers, identification columns 73-80), detects comment
//!    lines, and joins continuation lines.
//!
//! 2. **Tokenization** ([`lexer`] module) — a `logos`-powered inner lexer
//!    handles string/numeric literals and punctuation; a keyword lookup table
//!    ([`keywords`] module) classifies identifier-shaped words.
//!
//! See the [architecture plan](../../Cobolt_Architecture_Plan.md) for the
//! full picture.

// ── Public modules ────────────────────────────────────────────────────────────

pub mod keywords;
pub mod lexer;
pub mod source;
pub mod span;
pub mod token;

// ── Re-exports ────────────────────────────────────────────────────────────────

pub use lexer::{tokenize, tokenize_with_comments, LexError, Lexer};
pub use source::SourceFormat;
pub use span::{LineIndex, Span, SpannedToken};
pub use token::{RawToken, Token};
