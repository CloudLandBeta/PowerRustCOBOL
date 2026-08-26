// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Cobolt COBOL parser — converts a token stream into an AST.
//!
//! # Entry point
//!
//! ```rust
//! use cobolt_lexer::{tokenize, SourceFormat};
//! use cobolt_parser::parse;
//!
//! let src = "IDENTIFICATION DIVISION.\nPROGRAM-ID. HELLO.\nPROCEDURE DIVISION.\nMAIN.\n    STOP RUN.\n";
//! let tokens = tokenize(src, SourceFormat::Free);
//! let result = parse(tokens);
//! assert!(result.diagnostics.is_empty());
//! assert_eq!(result.program.unwrap().identification.program_id, "HELLO");
//! ```

mod data;
mod error;
mod expr;
mod identification;
mod parser;
mod procedure;
mod stmt;

pub use error::{Diagnostic, ParseResult, Severity};
pub use parser::Parser;

use cobolt_lexer::SpannedToken;

/// Parse a complete COBOL program from a pre-tokenized stream.
///
/// Always returns a [`ParseResult`].  When errors occur the parser
/// attempts recovery (skipping to the next `.`) and continues; partial
/// ASTs are common and useful for IDE tooling.
pub fn parse(tokens: Vec<SpannedToken>) -> ParseResult {
    Parser::new(tokens).parse_program()
}

/// Parse a program whose `EXEC RUST` block ids continue from `block_id_base`.
///
/// [`parse`] numbers blocks from zero, which is right for a source read on its
/// own. A build is not that: a form application compiles the main program and
/// every openable form's program into **one** binary, and every interpreter in
/// that process looks a block up in **one** registry. Numbering each file from
/// zero would give the main form's first block and a child form's first block
/// the same id, and the last one registered would silently answer for both.
///
/// So the caller parses each program with the previous one's
/// [`ParseResult::next_block_id`], and the ids stay unique across the whole
/// application.
pub fn parse_from(tokens: Vec<SpannedToken>, block_id_base: u32) -> ParseResult {
    Parser::with_block_id_base(tokens, block_id_base).parse_program()
}
