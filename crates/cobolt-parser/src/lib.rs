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
