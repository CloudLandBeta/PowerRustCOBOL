// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Statement node types for the PROCEDURE DIVISION.

use cobolt_lexer::Span;
use serde::{Deserialize, Serialize};

use crate::expr::{CmpOp, Condition, Expr, Literal};

// ── Supporting clause types ───────────────────────────────────────────────────

/// File open modes for the OPEN statement.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum OpenMode {
    Input,
    Output,
    InputOutput,
    Extend,
}

/// Direction of a sequential READ on an indexed/relative file.
///
/// `Default` is an unqualified `READ` — random (by RECORD KEY) under RANDOM or
/// DYNAMIC access, sequential under SEQUENTIAL access. `Next`/`Previous` force
/// sequential retrieval (the only forms valid for DYNAMIC sequential reads).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum ReadDirection {
    #[default]
    Default,
    Next,
    Previous,
}

/// How an argument is passed in a CALL statement.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum CallArg {
    ByReference(Expr),
    ByContent(Expr),
    ByValue(Expr),
}

/// The source of an ACCEPT statement (`FROM DATE`, `FROM TIME`, etc.).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum AcceptSource {
    Date,
    Time,
    Day,
    DayOfWeek,
    CommandLine,
    Environment(String),
}

/// WRITE … ADVANCING clause.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AdvancingClause {
    /// Number of lines to advance, or the mnemonic.
    pub lines: Expr,
    /// `true` = BEFORE ADVANCING, `false` = AFTER ADVANCING.
    pub before: bool,
}

/// A single WHEN clause inside EVALUATE.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WhenClause {
    /// One or more values / ranges / ANY / OTHER that match this arm.
    pub values: Vec<WhenValue>,
    pub stmts: Vec<Stmt>,
    pub span: Span,
}

/// A value entry inside a WHEN clause.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum WhenValue {
    Literal(Literal),
    /// A range: `WHEN 1 THRU 9`
    Range(Literal, Literal),
    /// `WHEN ANY`
    Any,
    /// `WHEN OTHER`
    Other,
    /// A condition used directly: `WHEN condition`
    Condition(Condition),
}

/// The subject of an EVALUATE statement.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum EvalSubject {
    Expr(Expr),
    /// `EVALUATE TRUE`
    True_,
    /// `EVALUATE FALSE`
    False_,
}

/// PERFORM target variants.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum PerformTarget {
    /// `PERFORM paragraph-name`
    Paragraph(String, Span),
    /// `PERFORM section-name`
    Section(String, Span),
    /// `PERFORM paragraph-name THRU paragraph-name`
    Thru { from: String, to: String, span: Span },
    /// Inline PERFORM … END-PERFORM
    Inline {
        stmts: Vec<Stmt>,
    },
    /// PERFORM … TIMES
    Times {
        count: Expr,
        stmts: Vec<Stmt>,
    },
    /// PERFORM … UNTIL
    Until {
        condition: Condition,
        test_before: bool, // true = TEST BEFORE (default), false = TEST AFTER
        stmts: Vec<Stmt>,
    },
    /// PERFORM VARYING … FROM … BY … UNTIL …
    Varying {
        var: Expr,
        from: Expr,
        by: Expr,
        until: Condition,
        stmts: Vec<Stmt>,
        /// Optional AFTER sub-varying clauses
        after: Vec<VaryingAfter>,
    },
}

/// An AFTER sub-clause for multi-dimensional PERFORM VARYING.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct VaryingAfter {
    pub var: Expr,
    pub from: Expr,
    pub by: Expr,
    pub until: Condition,
}

/// An INTO target for UNSTRING.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct UnstringTarget {
    pub target: Expr,
    /// DELIMITER IN
    pub delimiter: Option<Expr>,
    /// COUNT IN
    pub count: Option<Expr>,
}

/// A sort/merge key.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SortKey {
    pub ascending: bool,
    pub fields: Vec<Expr>,
}

/// INSPECT TALLYING spec for one counter.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TallySpec {
    pub counter: Expr,
    pub for_: Vec<TallyFor>,
}

/// What to tally.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum TallyFor {
    Characters,
    All(Expr),
    Leading(Expr),
    Trailing(Expr),
}

/// INSPECT REPLACING spec.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ReplaceSpec {
    pub what: ReplaceWhat,
    pub by: Expr,
}

/// What to replace in an INSPECT REPLACING clause.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ReplaceWhat {
    Characters,
    All(Expr),
    Leading(Expr),
    Trailing(Expr),
    First(Expr),
}

/// PowerCOBOL / Fujitsu window operations.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum WindowOperation {
    Show(String),
    Hide(String),
    Close(String),
}

/// A resolved binding between a COBOL data item and its Rust counterpart
/// inside an [`Stmt::ExecRust`] block.
///
/// Populated by the semantic pass; empty until then.
///
/// # Variable naming convention
///
/// | COBOL name    | Rust name      |
/// |---------------|----------------|
/// | `WS-COUNT`    | `ws_count`     |
/// | `WS-MY-FIELD` | `ws_my_field`  |
///
/// Hyphens are replaced with underscores and the name is lower-cased.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ExecRustBinding {
    /// The COBOL data-item name, uppercase with hyphens preserved.
    /// Example: `"WS-COUNT"`.
    pub cobol_name: String,
    /// The Rust variable name, snake_case.
    /// Example: `"ws_count"`.
    pub rust_name: String,
}

// ── Stmt ──────────────────────────────────────────────────────────────────────

/// A single COBOL statement.
///
/// Every variant carries a `span` so the runtime and IDE can map back to
/// the exact source location.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Stmt {
    // ── Data movement ────────────────────────────────────────────────────────

    /// `MOVE sending TO receiving …`
    Move {
        from: Expr,
        to: Vec<Expr>,
        span: Span,
    },

    /// `MOVE CORRESPONDING group TO group`
    MoveCorresponding {
        from: Expr,
        to: Expr,
        span: Span,
    },

    /// `ADD CORRESPONDING group TO group [ROUNDED]`
    AddCorresponding {
        from: Expr,
        to: Expr,
        rounded: bool,
        span: Span,
    },

    /// `SUBTRACT CORRESPONDING group FROM group [ROUNDED]`
    SubtractCorresponding {
        from: Expr,
        to: Expr,
        rounded: bool,
        span: Span,
    },

    /// `INITIALIZE item …` — category-aware reset (numeric → ZERO, others →
    /// SPACE), recursing into group items.
    Initialize {
        items: Vec<Expr>,
        span: Span,
    },

    // ── Arithmetic ───────────────────────────────────────────────────────────

    /// `ADD operand … TO receiving … [GIVING receiving]`
    Add {
        operands: Vec<Expr>,
        to: Vec<Expr>,
        giving: Option<Expr>,
        rounded: bool,
        /// Imperative run on ON SIZE ERROR (empty if no such clause).
        on_size_error: Vec<Stmt>,
        /// Imperative run on NOT ON SIZE ERROR (empty if no such clause).
        not_on_size_error: Vec<Stmt>,
        span: Span,
    },

    /// `SUBTRACT operand … FROM receiving … [GIVING receiving]`
    Subtract {
        operands: Vec<Expr>,
        from: Vec<Expr>,
        giving: Option<Expr>,
        rounded: bool,
        on_size_error: Vec<Stmt>,
        not_on_size_error: Vec<Stmt>,
        span: Span,
    },

    /// `MULTIPLY lhs BY rhs [GIVING receiving]`
    Multiply {
        lhs: Expr,
        by: Expr,
        giving: Option<Expr>,
        rounded: bool,
        on_size_error: Vec<Stmt>,
        not_on_size_error: Vec<Stmt>,
        span: Span,
    },

    /// `DIVIDE lhs BY rhs [GIVING receiving] [REMAINDER remainder]`
    Divide {
        lhs: Expr,
        by: Expr,
        giving: Option<Expr>,
        remainder: Option<Expr>,
        rounded: bool,
        on_size_error: Vec<Stmt>,
        not_on_size_error: Vec<Stmt>,
        span: Span,
    },

    /// `COMPUTE target = expr`
    Compute {
        /// Receiving fields, each with its own `ROUNDED` flag.
        targets: Vec<(Expr, bool)>,
        expr: Expr,
        on_size_error: Vec<Stmt>,
        not_on_size_error: Vec<Stmt>,
        span: Span,
    },

    // ── Control flow ─────────────────────────────────────────────────────────

    /// `IF condition … [ELSE …] END-IF`
    If {
        condition: Condition,
        then_stmts: Vec<Stmt>,
        else_stmts: Vec<Stmt>,
        span: Span,
    },

    /// `EVALUATE subject WHEN … [WHEN OTHER …] END-EVALUATE`
    Evaluate {
        subject: EvalSubject,
        whens: Vec<WhenClause>,
        other_stmts: Vec<Stmt>,
        span: Span,
    },

    /// `PERFORM …`
    Perform {
        target: PerformTarget,
        span: Span,
    },

    /// `SEARCH [ALL] table [VARYING idx] [AT END …] {WHEN cond …}… END-SEARCH`
    Search {
        all: bool,
        table: Expr,
        varying: Option<Expr>,
        at_end: Vec<Stmt>,
        whens: Vec<(Condition, Vec<Stmt>)>,
        span: Span,
    },

    /// `GO TO paragraph`
    GoTo {
        target: String,
        span: Span,
    },

    /// `GO TO paragraph … DEPENDING ON data-item`
    GoToDepending {
        targets: Vec<String>,
        depending: Expr,
        span: Span,
    },

    /// `CONTINUE`
    Continue { span: Span },

    /// `NEXT SENTENCE`
    NextSentence { span: Span },

    // ── I/O ──────────────────────────────────────────────────────────────────

    /// `OPEN mode file …`
    Open {
        mode: OpenMode,
        files: Vec<String>,
        span: Span,
    },

    /// `CLOSE file …`
    Close {
        files: Vec<String>,
        span: Span,
    },

    /// `READ file [NEXT|PREVIOUS] [INTO target] [KEY IS k]`
    /// `[AT END …] [NOT AT END …] [INVALID KEY …] [NOT INVALID KEY …]`
    Read {
        file: String,
        into: Option<Expr>,
        key: Option<Expr>,
        direction: ReadDirection,
        at_end: Vec<Stmt>,
        not_at_end: Vec<Stmt>,
        invalid_key: Vec<Stmt>,
        not_invalid_key: Vec<Stmt>,
        span: Span,
    },

    /// `WRITE record [FROM source] [ADVANCING …] [INVALID KEY …]`
    Write {
        record: Expr,
        from: Option<Expr>,
        advancing: Option<AdvancingClause>,
        invalid_key: Vec<Stmt>,
        not_invalid_key: Vec<Stmt>,
        span: Span,
    },

    /// `REWRITE record [FROM source] [INVALID KEY …]`
    Rewrite {
        record: Expr,
        from: Option<Expr>,
        invalid_key: Vec<Stmt>,
        not_invalid_key: Vec<Stmt>,
        span: Span,
    },

    /// `DELETE file [INVALID KEY …]`
    Delete {
        file: String,
        invalid_key: Vec<Stmt>,
        not_invalid_key: Vec<Stmt>,
        span: Span,
    },

    /// `START file [KEY op data-item] [INVALID KEY …]`
    Start {
        file: String,
        key: Option<(CmpOp, Expr)>,
        invalid_key: Vec<Stmt>,
        not_invalid_key: Vec<Stmt>,
        span: Span,
    },

    // ── User interaction ─────────────────────────────────────────────────────

    /// `ACCEPT target [FROM source]`
    Accept {
        target: Expr,
        from: Option<AcceptSource>,
        span: Span,
    },

    /// `DISPLAY operand … [UPON mnemonic] [NO ADVANCING]`
    Display {
        operands: Vec<Expr>,
        upon: Option<String>,
        no_advancing: bool,
        span: Span,
    },

    // ── String handling ──────────────────────────────────────────────────────

    /// `STRING src … DELIMITED BY delim … INTO target [WITH POINTER ptr]`
    String_ {
        /// (source, delimiter) pairs
        operands: Vec<(Expr, Option<Expr>)>,
        into: Expr,
        pointer: Option<Expr>,
        on_overflow: Vec<Stmt>,
        not_on_overflow: Vec<Stmt>,
        span: Span,
    },

    /// `UNSTRING src DELIMITED BY … INTO target …`
    Unstring {
        from: Expr,
        delimited_by: Vec<Expr>,
        all: bool,
        into: Vec<UnstringTarget>,
        pointer: Option<Expr>,
        tallying: Option<Expr>,
        on_overflow: Vec<Stmt>,
        not_on_overflow: Vec<Stmt>,
        span: Span,
    },

    /// `INSPECT target TALLYING / REPLACING / CONVERTING`
    Inspect {
        target: Expr,
        spec: InspectSpec,
        span: Span,
    },

    // ── Sorting ──────────────────────────────────────────────────────────────

    /// `SORT file ON KEY … [INPUT PROCEDURE] [OUTPUT PROCEDURE]`
    Sort {
        file: String,
        keys: Vec<SortKey>,
        duplicates: bool,
        input_proc: Option<String>,
        output_proc: Option<String>,
        span: Span,
    },

    /// `MERGE file ON KEY … OUTPUT PROCEDURE`
    Merge {
        file: String,
        keys: Vec<SortKey>,
        output_proc: Option<String>,
        span: Span,
    },

    // ── Subprogram linkage ───────────────────────────────────────────────────

    /// `CALL program [USING …] [RETURNING …] [ON EXCEPTION …]`
    Call {
        program: Expr,
        using: Vec<CallArg>,
        returning: Option<Expr>,
        on_exception: Vec<Stmt>,
        span: Span,
    },

    // ── Program termination ──────────────────────────────────────────────────

    /// `STOP RUN` or `STOP literal`
    Stop {
        run: bool,
        literal: Option<Literal>,
        span: Span,
    },

    /// `GOBACK`
    GoBack { span: Span },

    // ── PowerCOBOL / Fujitsu extensions ─────────────────────────────────────

    /// Form/window operation (SHOW, HIDE, CLOSE window).
    WindowOp {
        op: WindowOperation,
        span: Span,
    },

    /// Set a control property via COBOLT-SET-PROPERTY.
    ControlSet {
        control: Expr,
        property: String,
        value: Expr,
        span: Span,
    },

    // ── EXEC RUST inline Rust extension ─────────────────────────────────────

    /// `EXEC RUST … END-EXEC`
    ///
    /// Embeds verbatim Rust code inside a COBOL procedure.
    ///
    /// # Runtime binding
    ///
    /// Before the block executes the runtime generates a preamble that binds
    /// every DATA DIVISION item as a typed Rust variable:
    ///
    /// ```text
    /// EXEC RUST
    ///     ws_count += 1;
    ///     if ws_flag == b'Y' {
    ///         ws_result = ws_total / ws_count;
    ///     }
    ///     // PowerCOBOL object access:
    ///     cobolt_objects.get("FORM1")?.set_text("Hello from Rust!");
    /// END-EXEC.
    /// ```
    ///
    /// Variable naming: COBOL `WS-MY-FIELD` → Rust `ws_my_field` (`&mut T`).
    /// Always-available handles: `cobol_env: &mut CobolEnvironment`,
    /// `cobolt_objects: &mut ObjectRegistry`.
    ExecRust {
        /// The raw Rust source text captured between `EXEC RUST` and `END-EXEC`.
        source: String,
        /// COBOL data items referenced by this block.
        ///
        /// **Populated by the semantic pass** (empty at parse time).
        /// Each entry maps a COBOL name to the corresponding Rust snake_case name.
        referenced_data: Vec<ExecRustBinding>,
        span: Span,
    },

    // ── CoBolt exception handling extensions ─────────────────────────────────

    /// `TRY … CATCH EXCEPTION <name> … [ FINALLY … ] END-TRY`
    ///
    /// Non-standard CoBolt extension for structured exception handling.
    ///
    /// ```text
    /// TRY
    ///     MOVE 'hello' TO WS-TEXT
    /// CATCH EXCEPTION e
    ///     DISPLAY 'Error: ' e
    /// FINALLY
    ///     DISPLAY 'Done'
    /// END-TRY
    /// ```
    TryCatch {
        try_stmts:      Vec<Stmt>,
        /// Name of the exception variable in the CATCH clause (e.g. `"e"`).
        exception_var:  Option<String>,
        catch_stmts:    Vec<Stmt>,
        finally_stmts:  Vec<Stmt>,
        span: Span,
    },

    /// `THROW <expression>` / `RAISE <expression>`
    ///
    /// Raises an exception with the given string message or identifier.
    Throw {
        message: crate::expr::Expr,
        span: Span,
    },
}

impl Stmt {
    /// Return the source span of this statement.
    pub fn span(&self) -> Span {
        match self {
            Stmt::Move { span, .. }              => *span,
            Stmt::MoveCorresponding { span, .. } => *span,
            Stmt::AddCorresponding { span, .. } => *span,
            Stmt::SubtractCorresponding { span, .. } => *span,
            Stmt::Initialize { span, .. }        => *span,
            Stmt::Add { span, .. }               => *span,
            Stmt::Subtract { span, .. }          => *span,
            Stmt::Multiply { span, .. }          => *span,
            Stmt::Divide { span, .. }            => *span,
            Stmt::Compute { span, .. }           => *span,
            Stmt::If { span, .. }                => *span,
            Stmt::Evaluate { span, .. }          => *span,
            Stmt::Perform { span, .. }           => *span,
            Stmt::Search { span, .. }            => *span,
            Stmt::GoTo { span, .. }              => *span,
            Stmt::GoToDepending { span, .. }     => *span,
            Stmt::Continue { span }              => *span,
            Stmt::NextSentence { span }          => *span,
            Stmt::Open { span, .. }              => *span,
            Stmt::Close { span, .. }             => *span,
            Stmt::Read { span, .. }              => *span,
            Stmt::Write { span, .. }             => *span,
            Stmt::Rewrite { span, .. }           => *span,
            Stmt::Delete { span, .. }            => *span,
            Stmt::Start { span, .. }             => *span,
            Stmt::Accept { span, .. }            => *span,
            Stmt::Display { span, .. }           => *span,
            Stmt::String_ { span, .. }           => *span,
            Stmt::Unstring { span, .. }          => *span,
            Stmt::Inspect { span, .. }           => *span,
            Stmt::Sort { span, .. }              => *span,
            Stmt::Merge { span, .. }             => *span,
            Stmt::Call { span, .. }              => *span,
            Stmt::Stop { span, .. }              => *span,
            Stmt::GoBack { span }                => *span,
            Stmt::WindowOp { span, .. }          => *span,
            Stmt::ControlSet { span, .. }        => *span,
            Stmt::ExecRust { span, .. }          => *span,
            Stmt::TryCatch { span, .. }          => *span,
            Stmt::Throw { span, .. }             => *span,
        }
    }
}

/// INSPECT specification.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum InspectSpec {
    Tallying(Vec<TallySpec>),
    Replacing(Vec<ReplaceSpec>),
    TallyingReplacing(Vec<TallySpec>, Vec<ReplaceSpec>),
    Converting { from: Expr, to: Expr },
}
