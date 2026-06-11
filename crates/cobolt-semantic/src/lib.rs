// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Cobolt semantic analysis — the pass that runs after parsing and before
//! code generation or interpretation.
//!
//! # What this crate does
//!
//! | Sub-pass              | Module            | Purpose                                      |
//! |-----------------------|-------------------|----------------------------------------------|
//! | Symbol table build    | [`symbol_table`]  | Index every data item, paragraph, section    |
//! | Name resolution       | [`resolver`]      | Check identifiers exist; resolve qualifiers  |
//! | Type checking         | [`type_checker`]  | Catch numeric-vs-string mismatches, etc.     |
//! | EXEC RUST binding     | [`exec_rust`]     | Map snake_case names → COBOL data items      |
//!
//! # Entry point
//!
//! ```rust,no_run
//! use cobolt_ast::program::Program;
//! use cobolt_semantic::analyze;
//!
//! // `program` comes from cobolt_parser::parse()
//! # let program: Program = unimplemented!();
//! let result = analyze(&program);
//! for diag in &result.diagnostics {
//!     eprintln!("{}", diag);
//! }
//! ```

pub mod duplicates;
pub mod exec_rust;
pub mod resolver;
pub mod symbol_table;
pub mod type_checker;

use cobolt_ast::program::Program;
use cobolt_lexer::Span;

// ── Diagnostic ────────────────────────────────────────────────────────────────

/// Severity level of a semantic diagnostic.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Severity {
    /// Informational hint — does not prevent compilation.
    Info,
    /// Potential issue — program may behave unexpectedly.
    Warning,
    /// Definite error — program cannot be executed correctly.
    Error,
}

impl std::fmt::Display for Severity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Severity::Info    => write!(f, "info"),
            Severity::Warning => write!(f, "warning"),
            Severity::Error   => write!(f, "error"),
        }
    }
}

/// A diagnostic message produced by the semantic analyser.
#[derive(Debug, Clone)]
pub struct SemanticDiagnostic {
    pub severity: Severity,
    pub message: String,
    pub span: Span,
}

impl std::fmt::Display for SemanticDiagnostic {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "[{}:{}] {}: {}",
            self.span.line, self.span.col,
            self.severity, self.message
        )
    }
}

// ── SemanticResult ────────────────────────────────────────────────────────────

/// The output of [`analyze`].
#[derive(Debug)]
pub struct SemanticResult {
    /// All diagnostics (info, warnings, errors).
    pub diagnostics: Vec<SemanticDiagnostic>,
    /// The symbol table built from the DATA and PROCEDURE divisions.
    pub symbols: symbol_table::SymbolTable,
}

impl SemanticResult {
    /// `true` if there are no error-severity diagnostics.
    pub fn is_ok(&self) -> bool {
        self.diagnostics.iter().all(|d| d.severity < Severity::Error)
    }

    /// Return only error-severity diagnostics.
    pub fn errors(&self) -> impl Iterator<Item = &SemanticDiagnostic> {
        self.diagnostics.iter().filter(|d| d.severity == Severity::Error)
    }

    /// Return only warning-severity diagnostics.
    pub fn warnings(&self) -> impl Iterator<Item = &SemanticDiagnostic> {
        self.diagnostics.iter().filter(|d| d.severity == Severity::Warning)
    }
}

// ── Public entry point ────────────────────────────────────────────────────────

/// Run all semantic passes over a parsed COBOL program.
///
/// The passes run in order:
/// 1. Build the symbol table.
/// 2. Resolve all identifier references.
/// 3. Type-check statements.
/// 4. Resolve EXEC RUST bindings.
///
/// The returned [`SemanticResult`] always contains a symbol table (even on
/// error), allowing downstream tools to present partial information.
pub fn analyze(program: &Program) -> SemanticResult {
    let mut diagnostics = Vec::new();

    // Pass 1: build the symbol table from DATA + PROCEDURE divisions.
    let symbols = symbol_table::SymbolTable::build(program);

    // Pass 1b: reject redeclared unique procedure names (paragraphs/sections).
    duplicates::check(program, &mut diagnostics);

    // Pass 2: name resolution.
    resolver::resolve(program, &symbols, &mut diagnostics);

    // Pass 3: type checking.
    type_checker::check(program, &symbols, &mut diagnostics);

    // Pass 4: EXEC RUST binding resolution.
    exec_rust::resolve_bindings(program, &symbols, &mut diagnostics);

    SemanticResult { diagnostics, symbols }
}
