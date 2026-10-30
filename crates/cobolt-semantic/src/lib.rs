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

pub mod calls;
pub mod duplicates;
pub mod exec_rust;
pub mod exec_sql;
pub mod national;
pub mod flagging;
pub mod external;
pub mod file_keys;
pub mod remote_calls;
pub mod reports;
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
    /// Report an undeclared data-item reference as a WARNING instead of the
    /// error it is (1.70.77). **For the CCVS85 conformance census only**: its
    /// members are analysed untouched, so their X-cards (`XXXXX081`, an
    /// implementor name the installer substitutes) and column-7 selector
    /// lines (`P`, referencing items declared on other selector lines) are
    /// undeclared by construction, not by mistake. Every product gate — the
    /// IDE's Run Form and Build, `rcrun run-form`, `rcrun build`, `rcrun
    /// check` — leaves this `false`, which is the default.
    pub tolerate_undeclared: bool,
    /// The objects a `X::…` receiver may name besides `me`, `super`, `COBOL`
    /// and the program's own data items and REPOSITORY classes: the form's
    /// control ids (toolbar buttons included) and the form's own name, in
    /// UPPERCASE. `Some(set)` = a form program: a receiver outside it is an
    /// error, because the call would land on nothing and say nothing.
    /// `None` = no form context (a lone `.cbl`); receivers are not checked.
    pub known_objects: Option<std::collections::HashSet<String>>,
    /// The names of the form's LAYERS, in UPPERCASE (spec 091 R37) — receivers
    /// that are in `known_objects` too, but whose properties are checked as well:
    /// `LAYER-NAME::Visible` passes, `LAYER-NAME::Colour` is an error at build
    /// time, where a misspelt property would otherwise be refused at run time.
    /// `None` (the default) = no layers known; no layer property is checked.
    pub known_layers: Option<std::collections::HashSet<String>>,
    /// The user-defined functions (`FUNCTION-ID`, spec 109 §5.1) of the project's
    /// Common Code besides the ones the unit holds, in UPPERCASE: a
    /// `FUNCTION name(…)` that is neither an intrinsic nor in this set or the
    /// unit is an error. `None` = no project context: only the unit's own count.
    pub known_functions: Option<std::collections::HashSet<String>>,
    /// The program-names a literal `CALL "NAME"` may reach besides what the
    /// unit itself holds — its programs, paragraphs and sections: the
    /// project's Common Code, in UPPERCASE. `Some(set)` = a closed world (the
    /// caller can see everything a run or a build links): a literal target
    /// that the unit and this set do not answer is an error, because the
    /// runtime would skip the call and say nothing — unless the `CALL` names
    /// `ON EXCEPTION`, which is how a program declares the target optional.
    /// `None` = the caller cannot see the project (a lone file, a lint that
    /// has not read the manifest): nothing is checked. The built-in
    /// `COBOL-…` / `COBOLT-…` names are never checked here — their table
    /// lives in the runtime, which this crate cannot see.
    pub known_programs: Option<std::collections::HashSet<String>>,
    /// The backend's interface description, imported into the project
    /// (WASM-005 R4, R7). `Some` = the project has a backend: a
    /// `CALL "NAME" WITH WEBSOCKET LINKAGE` must name a program the
    /// description exports, with as many arguments as it takes, whose groups
    /// fit the parameters it declares. `None` = no description imported; the
    /// phrase is still checked for what the unit alone can show (the arguments
    /// are groups of WORKING-STORAGE) but not against any backend.
    pub remote_interface: Option<cobolt_wire::contract::InterfaceDescription>,
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

/// The names of the ordinary data items of a compilation unit — the receivers
/// of a data item's value methods (`WS-TEXT::Trim()`), UPPERCASE.
///
/// This is the analyser's own rule for "is this a data item" (the one behind
/// "a data item has no method …"): the name is declared by the unit's DATA
/// DIVISIONs, is not an `OBJECT REFERENCE` or a `POINTER`, and is not a name the
/// form's own objects share (`known_objects` — the control wins). The unit is
/// the program and every program contained in it, so a form's `GLOBAL` items
/// and its handlers' own are all seen. A name declared as a value in one program
/// and as a handle in another is ambiguous, and an ambiguous name is left out:
/// the caller uses this to decide what may be *rewritten*, and a wrong "yes"
/// there changes a program, where a wrong "no" changes nothing.
pub fn value_item_names(
    program: &Program,
    known_objects: Option<&std::collections::HashSet<String>>,
) -> std::collections::HashSet<String> {
    value_item_names_in(&[program], known_objects)
}

/// [`value_item_names`] over several units read together — a form's whole
/// generated program and the one handler being edited, which may declare items
/// the form does not hold yet.
pub fn value_item_names_in(
    programs: &[&Program],
    known_objects: Option<&std::collections::HashSet<String>>,
) -> std::collections::HashSet<String> {
    fn walk(
        program: &Program,
        values: &mut std::collections::HashSet<String>,
        handles: &mut std::collections::HashSet<String>,
    ) {
        for (name, info) in symbol_table::SymbolTable::build(program).data_items() {
            if info.holds_a_value() {
                values.insert(name.to_ascii_uppercase());
            } else {
                handles.insert(name.to_ascii_uppercase());
            }
        }
        for nested in &program.nested_programs {
            walk(nested, values, handles);
        }
    }
    let mut values = std::collections::HashSet::new();
    let mut handles = std::collections::HashSet::new();
    for program in programs {
        walk(program, &mut values, &mut handles);
    }
    values.retain(|n| !handles.contains(n) && !known_objects.is_some_and(|k| k.contains(n)));
    values
}

/// [`analyze`], with project context (spec 044).
pub fn analyze_with(program: &Program, opts: &AnalyzeOptions) -> SemanticResult {
    // The outermost program of a compilation unit inherits nothing: there is no
    // enclosing program to declare a GLOBAL item for it.
    let mut result = analyze_contained(program, &[], opts);
    // Program-names are unique across the whole unit, so this is checked once,
    // here, rather than per contained program.
    duplicates::check_program_names(program, &mut result.diagnostics);
    // Spec 087: embedded SQL is checked across the whole unit at once, since a
    // cursor or a GLOBAL host variable crosses program boundaries.
    exec_sql::check(program, &mut result.diagnostics);
    national::check(program, &mut result.diagnostics);
    // A literal CALL target the unit and the project do not answer (2026-10-08).
    if let Some(known) = &opts.known_programs {
        calls::check(program, known, &mut result.diagnostics);
    }
    // A CALL … RETURNING at a target the unit knows returns nothing (2026-10-19).
    calls::check_returning(program, &mut result.diagnostics);
    // WASM-005: the arguments of a CALL … WITH WEBSOCKET LINKAGE, and — when the
    // project imported the backend's interface description — its name and fit.
    remote_calls::check(program, opts.remote_interface.as_ref(), &mut result.diagnostics);
    result
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

    // Pass 1d: a report printed into a Viewer (spec 062) — a rendered
    // organization needs a Viewer to render it, and has no printed page.
    reports::check(program, &mut diagnostics);

    // Pass 1e: an INDEXED file's RECORD KEY and ALTERNATE RECORD KEYs are
    // fields of its own record.
    file_keys::check(program, &mut diagnostics);

    // Pass 2: name resolution (carries the 049 R17 form-format map).
    resolver::resolve(
        program,
        &symbols,
        &mut diagnostics,
        opts.form_formats.as_ref(),
        opts.tolerate_undeclared,
        opts.known_objects.as_ref(),
        opts.known_layers.as_ref(),
        opts.known_functions.as_ref(),
    );

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
