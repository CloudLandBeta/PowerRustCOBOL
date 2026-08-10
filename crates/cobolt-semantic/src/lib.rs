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
pub mod external;
pub mod resolver;
pub mod symbol_table;
pub mod type_checker;

use crate::symbol_table::DataItemInfo;
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
            Severity::Info => write!(f, "info"),
            Severity::Warning => write!(f, "warning"),
            Severity::Error => write!(f, "error"),
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
            self.span.line, self.span.col, self.severity, self.message
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
        self.diagnostics
            .iter()
            .all(|d| d.severity < Severity::Error)
    }

    /// Return only error-severity diagnostics.
    pub fn errors(&self) -> impl Iterator<Item = &SemanticDiagnostic> {
        self.diagnostics
            .iter()
            .filter(|d| d.severity == Severity::Error)
    }

    /// Return only warning-severity diagnostics.
    pub fn warnings(&self) -> impl Iterator<Item = &SemanticDiagnostic> {
        self.diagnostics
            .iter()
            .filter(|d| d.severity == Severity::Warning)
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
    analyze_with(program, &AnalyzeOptions::default())
}

/// Context the analysis cannot read from the program itself (spec 044).
#[derive(Debug, Clone, Default)]
pub struct AnalyzeOptions {
    /// The `use`-line names of the project's registered External Crates
    /// (spec 044 R20; `serde-json` registers as `serde_json`).
    ///
    /// `Some(list)` = project context: blocks may name these crates, and an
    /// unregistered crate's error points at External Crates (R21). `None` =
    /// no project (single-file builds): the error says external crates
    /// require a project (R22). The default is `None`, which keeps every
    /// pre-044 caller's behaviour: only the always-linked crates pass.
    pub external_crates: Option<Vec<String>>,
    /// 049 R17 — how each of the project's forms may be loaded, keyed by
    /// UPPERCASE form id (the `.cfrm` file stem, and the form's own name when
    /// it differs). `Some(map)` = project context: `OpenFormSync`/`OpenFormAsync`
    /// targeting an `Embedded` form is a compile-time error. `None` = no
    /// project; the load-path check is skipped.
    pub form_formats: Option<std::collections::HashMap<String, FormLoadFormat>>,
}

/// 049 R1 — a project form's FormFormat, as the load-path check needs it.
/// A local mirror: this crate cannot depend on `cobolt-forms` (the dependency
/// runs the other way), so callers translate when they build the map.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FormLoadFormat {
    Standalone,
    Embedded,
    Both,
}

impl FormLoadFormat {
    /// May `OpenFormSync`/`OpenFormAsync` open this form as a window? (049 R17)
    pub fn allows_standalone(self) -> bool {
        matches!(self, FormLoadFormat::Standalone | FormLoadFormat::Both)
    }
    /// May a menu item load this form into the ContentPane? (049 R17)
    pub fn allows_embedded(self) -> bool {
        matches!(self, FormLoadFormat::Embedded | FormLoadFormat::Both)
    }
}

/// [`analyze`], with project context (spec 044).
pub fn analyze_with(program: &Program, opts: &AnalyzeOptions) -> SemanticResult {
    // The outermost program of a compilation unit inherits nothing: there is no
    // enclosing program to declare a GLOBAL item for it.
    analyze_contained(program, &[], opts)
}

/// [`analyze`], for a program CONTAINED in another, told which `GLOBAL` items
/// its ancestors declare.
///
/// COBOL-85 scopes a `GLOBAL` name to the declaring program AND every program
/// contained in it, however deeply. Pass 5 below gave each contained program a
/// symbol table built from its own DATA DIVISION alone, so a handler that read
/// a form-level item — the whole point of declaring it `GLOBAL` — was told the
/// name `is not declared in DATA DIVISION`. The code ran fine: the interpreter
/// keeps one shared environment and the outer program's items are in it. Only
/// the analyzer disagreed, which is the worst place for the disagreement to be,
/// because the agents write against it and correct code kept being rejected.
fn analyze_contained(
    program: &Program,
    inherited_globals: &[DataItemInfo],
    opts: &AnalyzeOptions,
) -> SemanticResult {
    let mut diagnostics = Vec::new();

    // Pass 1: build the symbol table from DATA + PROCEDURE divisions, plus the
    // GLOBAL items visible from the programs enclosing this one.
    let symbols = symbol_table::SymbolTable::build_contained(program, inherited_globals);

    // Pass 1b: reject redeclared unique procedure names (paragraphs/sections).
    duplicates::check(program, &mut diagnostics);

    // Pass 1c: EXTERNAL placement (spec 005) — only on 01/77/FD items.
    external::check(program, &mut diagnostics);

    // Pass 2: name resolution (carries the 049 R17 form-format map).
    resolver::resolve(program, &symbols, &mut diagnostics, opts.form_formats.as_ref());

    // Pass 3: type checking.
    type_checker::check(program, &symbols, &mut diagnostics);

    // Pass 4: EXEC RUST binding resolution.
    exec_rust::check_repository_classes(program, &mut diagnostics);
    exec_rust::resolve_bindings(program, &symbols, &mut diagnostics, opts);

    // Pass 5: every contained program, analyzed in its own right.
    //
    // Each nested program owns its DATA DIVISION and its PROCEDURE DIVISION, so
    // it needs its own symbol table — a name is resolved against the program
    // that declares it, not against the compilation unit. Without this the
    // outer program was the only thing ever analyzed, and in a RAD project that
    // is the one place agents never write: every event handler and every common
    // procedure is a contained program, so all of their code went unchecked
    // (measured 2026-08-02, and it is why a PERFORM with no target survived
    // three correction rounds).
    //
    // Diagnostics from a contained program are appended to the same list: they
    // carry their own spans, and the caller reports against the whole source.
    //
    // "In its own right" is not "in isolation". What this program declares
    // GLOBAL is visible to everything it contains, and so is whatever its own
    // ancestors declared GLOBAL — visibility accumulates down the nest. A name
    // this program declares itself shadows an ancestor's, so its own globals go
    // in first and the inherited ones only fill the gaps.
    let visible_globals = if program.nested_programs.is_empty() {
        Vec::new()
    } else {
        let mut visible = symbol_table::SymbolTable::global_data_items(program);
        let own: std::collections::HashSet<&str> =
            visible.iter().map(|i| i.cobol_name.as_str()).collect();
        let inherited: Vec<DataItemInfo> = inherited_globals
            .iter()
            .filter(|i| !own.contains(i.cobol_name.as_str()))
            .cloned()
            .collect();
        visible.extend(inherited);
        visible
    };
    for nested in &program.nested_programs {
        diagnostics.extend(analyze_contained(nested, &visible_globals, opts).diagnostics);
    }

    SemanticResult {
        diagnostics,
        symbols,
    }
}
