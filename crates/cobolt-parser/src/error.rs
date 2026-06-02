// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Parser diagnostics and result type.

use cobolt_ast::program::Program;
use cobolt_lexer::Span;

/// Severity of a parser diagnostic.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Severity {
    Error,
    Warning,
}

/// A single parse diagnostic (error or warning).
#[derive(Debug, Clone, PartialEq)]
pub struct Diagnostic {
    pub severity: Severity,
    pub message: String,
    pub span: Span,
}

impl Diagnostic {
    pub fn error(message: impl Into<String>, span: Span) -> Self {
        Self { severity: Severity::Error, message: message.into(), span }
    }

    pub fn warning(message: impl Into<String>, span: Span) -> Self {
        Self { severity: Severity::Warning, message: message.into(), span }
    }

    pub fn is_error(&self) -> bool {
        self.severity == Severity::Error
    }
}

/// The result of parsing a complete COBOL program.
///
/// Even on error the parser attempts recovery and returns a partial AST
/// (useful for IDE tooling).  Check [`diagnostics`](ParseResult::diagnostics)
/// for errors; a non-empty list does **not** mean `program` is `None`.
#[derive(Debug)]
pub struct ParseResult {
    /// The parsed program, or `None` if parsing failed before any structure
    /// could be recovered.
    pub program: Option<Program>,
    /// All diagnostics emitted during parsing.
    pub diagnostics: Vec<Diagnostic>,
}

impl ParseResult {
    pub fn has_errors(&self) -> bool {
        self.diagnostics.iter().any(|d| d.is_error())
    }
}
