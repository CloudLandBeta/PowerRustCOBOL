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

/// `OPEN … SHARING WITH …` mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ShareMode {
    /// `SHARING WITH ALL OTHER`
    AllOther,
    /// `SHARING WITH NO OTHER`
    NoOther,
    /// `SHARING WITH READ ONLY`
    ReadOnly,
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
    /// `FROM COMMAND-LINE` — the whole command line (arguments joined by spaces).
    CommandLine,
    /// `FROM ENVIRONMENT "name"` — the named environment variable.
    Environment(String),
    /// `FROM ENVIRONMENT-VALUE` — the variable named by the most recent
    /// `DISPLAY … UPON ENVIRONMENT-NAME`.
    EnvironmentValue,
    /// `FROM ARGUMENT-NUMBER` — the count of command-line arguments.
    ArgumentNumber,
    /// `FROM ARGUMENT-VALUE` — the argument at the current argument pointer
    /// (set by `DISPLAY n UPON ARGUMENT-NUMBER`).
    ArgumentValue,
    /// `FROM ESCAPE KEY` — the key code that ended the last ACCEPT (`"00"`).
    EscapeKey,
    /// `FROM CRT STATUS` — the screen status of the last operation (`"0000"`).
    CrtStatus,
    /// `FROM <mnemonic-name>` where SPECIAL-NAMES associates the mnemonic with
    /// an implementor-name — Format 1 `ACCEPT`, reading the hardware device.
    ///
    /// The mnemonic is kept rather than collapsed to "console" so a device
    /// distinction remains available. A name SPECIAL-NAMES never declared is
    /// **not** this variant: it stays [`AcceptSource::Environment`], which is
    /// the non-standard extension that reads an environment variable.
    ///
    /// New variants go at the END — the AST is bincode-serialized by ordinal.
    Mnemonic(String),
}

/// WRITE … ADVANCING clause.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AdvancingClause {
    /// Number of lines to advance, or the mnemonic.
    pub lines: Expr,
    /// `true` = BEFORE ADVANCING, `false` = AFTER ADVANCING.
    pub before: bool,
}

/// A single WHEN clause inside EVALUATE. With `ALSO`, `values` holds one entry
/// per EVALUATE subject (matched positionally, AND-combined).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WhenClause {
    /// One selection object per subject column (AND-combined across columns).
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
    /// A negated selection object: `WHEN NOT value`.
    Not(Box<WhenValue>),
    /// An arithmetic **expression** as the selection object:
    /// `WHEN (33 + (99 - 43))`. Compared against the EVALUATE subject exactly
    /// as a literal object is; it is not a condition, and reading it as one is
    /// what made the parenthesis fail to parse.
    ///
    /// ⚠️ New variants go at the END — this enum is bincode-serialized into
    /// every compiled binary and a variant is identified by its ordinal.
    Expr(Expr),
    /// A range whose bounds are **data items** rather than literals:
    /// `WHEN WRK-A THRU WRK-B`. [`WhenValue::Range`] carries literals only.
    ExprRange(Expr, Expr),
}

/// The subject of an EVALUATE statement.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum EvalSubject {
    Expr(Expr),
    /// `EVALUATE TRUE`
    True_,
    /// `EVALUATE FALSE`
    False_,
    // ⚠️ New variants go at the END — this enum is bincode-serialized into every
    // compiled binary and a variant is identified by its ordinal. See
    // `Expr::AllSubscript` for what inserting one costs.
    /// A **conditional expression** as the subject: `EVALUATE X NUMERIC`,
    /// `EVALUATE A > B`. COBOL-85 allows it, and it is matched against
    /// `WHEN TRUE` / `WHEN FALSE`.
    Cond(Condition),
}

/// The flavour of an `EXIT` statement.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ExitKind {
    /// Plain `EXIT` — a no-op return point (used as a `THRU` paragraph end).
    Point,
    /// `EXIT PROGRAM` — return to the calling program.
    Program,
    /// `EXIT PERFORM` — terminate the nearest inline PERFORM loop.
    Perform,
    /// `EXIT PERFORM CYCLE` — continue with the next inline PERFORM iteration.
    PerformCycle,
    /// `EXIT PARAGRAPH` — return from the current paragraph.
    Paragraph,
    /// `EXIT SECTION` — return from the current section.
    Section,
}

/// PERFORM target variants.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum PerformTarget {
    /// `PERFORM paragraph-name`
    Paragraph(String, Span),
    /// `PERFORM section-name`
    Section(String, Span),
    /// `PERFORM paragraph-name THRU paragraph-name`
    Thru {
        from: String,
        to: String,
        span: Span,
    },
    /// Inline PERFORM … END-PERFORM
    Inline { stmts: Vec<Stmt> },
    /// PERFORM … TIMES
    Times { count: Expr, stmts: Vec<Stmt> },
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
        /// `true` = `WITH TEST BEFORE` (the default), `false` = `TEST AFTER`.
        ///
        /// `TEST AFTER` runs the body once before any condition is tested, and
        /// then tests innermost-first. It was parsed and thrown away, so
        /// `PERFORM … WITH TEST AFTER VARYING …` ran as `TEST BEFORE` — and a
        /// body that assigns the loop variables (NC201A PFM-TEST-F4-14 sets
        /// both of them) then never satisfied the outer condition at the point
        /// it was tested, and the program did not terminate.
        ///
        /// ⚠️ New fields go at the END — this enum is bincode-serialized and a
        /// variant's fields are written in declaration order.
        test_before: bool,
    },
    /// `PERFORM paragraph-name {OF|IN} section-name` — a paragraph name that
    /// repeats across sections, qualified by the one that owns the copy meant.
    ///
    /// 🔴 New variants belong at the END of this enum. `PerformTarget` is
    /// bincode-serialized and a variant is identified by its **ordinal**.
    QualifiedParagraph {
        name: String,
        section: String,
        span: Span,
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

/// A `BEFORE/AFTER INITIAL delimiter` region qualifier for an INSPECT phrase.
/// Both `None` means the whole field.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct InspectRegion {
    /// `AFTER INITIAL delimiter` — start the scan after the first delimiter.
    pub after: Option<Expr>,
    /// `BEFORE INITIAL delimiter` — stop the scan before the first delimiter.
    pub before: Option<Expr>,
}

/// INSPECT TALLYING spec for one counter.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TallySpec {
    pub counter: Expr,
    /// Each FOR phrase plus its optional BEFORE/AFTER INITIAL region.
    pub for_: Vec<(TallyFor, InspectRegion)>,
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
    /// Optional BEFORE/AFTER INITIAL region this replacement is confined to.
    pub region: InspectRegion,
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

/// Extended `ACCEPT`/`DISPLAY` screen phrase: a cursor position (`AT nnnn` or
/// `AT LINE n [COLUMN n]`) plus display attributes.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct ScreenPhrase {
    /// `AT LINE n` — the row.
    pub line: Option<Expr>,
    /// `AT … COLUMN n` — the column.
    pub col: Option<Expr>,
    /// `AT nnnn` — a combined row*100+col position.
    pub at: Option<Expr>,
    /// `WITH HIGHLIGHT` / `BOLD`.
    pub highlight: bool,
    /// `WITH REVERSE-VIDEO`.
    pub reverse: bool,
    /// `WITH UNDERLINE`.
    pub underline: bool,
}

/// The source of a pointer assignment (`SET … TO …`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum PointerSource {
    /// `NULL` / `NULLS`.
    Null,
    /// `ADDRESS OF item`.
    AddressOf(Expr),
    /// Another pointer data item.
    Pointer(Expr),
}

/// A data category for `INITIALIZE … REPLACING`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum InitCategory {
    Alphabetic,
    Alphanumeric,
    Numeric,
    AlphanumericEdited,
    NumericEdited,
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
    MoveCorresponding { from: Expr, to: Expr, span: Span },

    /// `ADD CORRESPONDING group TO group [ROUNDED] [ON SIZE ERROR …]`
    AddCorresponding {
        from: Expr,
        to: Expr,
        rounded: bool,
        span: Span,
        // ⚠️ New fields go at the END: bincode writes a variant's fields in
        // declaration order, so inserting one above renumbers nothing but
        // silently changes the encoding of every AST already built.
        /// `[ON] SIZE ERROR imperative`, run once for the whole statement.
        on_size_error: Vec<Stmt>,
        /// `NOT [ON] SIZE ERROR imperative`.
        not_on_size_error: Vec<Stmt>,
    },

    /// `SUBTRACT CORRESPONDING group FROM group [ROUNDED] [ON SIZE ERROR …]`
    SubtractCorresponding {
        from: Expr,
        to: Expr,
        rounded: bool,
        span: Span,
        /// `[ON] SIZE ERROR imperative`, run once for the whole statement.
        on_size_error: Vec<Stmt>,
        /// `NOT [ON] SIZE ERROR imperative`.
        not_on_size_error: Vec<Stmt>,
    },

    /// `INITIALIZE item … [REPLACING category DATA BY value …]` — category-aware
    /// reset (numeric → ZERO, others → SPACE), recursing into group items;
    /// `REPLACING` overrides the value for subordinate items of each category.
    Initialize {
        items: Vec<Expr>,
        /// `REPLACING category [DATA] BY value` overrides (empty = plain reset).
        replacing: Vec<(InitCategory, Expr)>,
        span: Span,
    },

    // ── Arithmetic ───────────────────────────────────────────────────────────
    /// `ADD operand … TO receiving … [GIVING receiving]`
    Add {
        operands: Vec<Expr>,
        /// `TO` receivers (each with its own `ROUNDED` flag) — also addends.
        to: Vec<(Expr, bool)>,
        /// `GIVING` receivers (each with its own `ROUNDED` flag).
        giving: Vec<(Expr, bool)>,
        /// Imperative run on ON SIZE ERROR (empty if no such clause).
        on_size_error: Vec<Stmt>,
        /// Imperative run on NOT ON SIZE ERROR (empty if no such clause).
        not_on_size_error: Vec<Stmt>,
        span: Span,
    },

    /// `SUBTRACT operand … FROM receiving … [GIVING receiving]`
    Subtract {
        operands: Vec<Expr>,
        /// `FROM` receivers (each with its own `ROUNDED` flag) — also minuends.
        from: Vec<(Expr, bool)>,
        /// `GIVING` receivers (each with its own `ROUNDED` flag).
        giving: Vec<(Expr, bool)>,
        on_size_error: Vec<Stmt>,
        not_on_size_error: Vec<Stmt>,
        span: Span,
    },

    /// `MULTIPLY lhs BY rhs [ROUNDED] [GIVING receiving …]`
    Multiply {
        lhs: Expr,
        by: Expr,
        /// `GIVING` receivers (each with its own `ROUNDED` flag); empty form
        /// stores the product back into `by` honouring `rounded`.
        giving: Vec<(Expr, bool)>,
        rounded: bool,
        on_size_error: Vec<Stmt>,
        not_on_size_error: Vec<Stmt>,
        span: Span,
    },

    /// `DIVIDE lhs BY rhs [ROUNDED] [GIVING receiving …] [REMAINDER remainder]`
    Divide {
        lhs: Expr,
        by: Expr,
        /// `GIVING` receivers (each with its own `ROUNDED` flag); empty form
        /// stores the quotient back into `by` honouring `rounded`.
        giving: Vec<(Expr, bool)>,
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
    /// `ALTER paragraph-1 TO [PROCEED TO] paragraph-2` — redirect the `GO TO`
    /// in `from` to target `to` (deprecated).
    Alter {
        from: String,
        to: String,
        span: Span,
    },

    /// `UNLOCK file [RECORD[S]]` — release record locks on `file`.
    Unlock { file: String, span: Span },

    /// `COMMIT` — make all uncommitted INDEXED-file changes durable and start a
    /// new transaction.
    Commit { span: Span },

    /// `ROLLBACK` — undo all INDEXED-file changes since the last `COMMIT`
    /// (or since `OPEN`).
    Rollback { span: Span },

    /// Pointer assignment:
    /// `SET ptr … TO ADDRESS OF item` (`address_of` = None), or
    /// `SET ADDRESS OF item TO {ADDRESS OF x | ptr | NULL}` (`address_of` = item).
    SetPointer {
        /// `Some(item)` for `SET ADDRESS OF item TO …`; `None` for pointer LHS.
        address_of: Option<Expr>,
        /// Pointer receivers when `address_of` is `None`.
        targets: Vec<Expr>,
        source: PointerSource,
        span: Span,
    },

    /// `IF condition … [ELSE …] END-IF`
    If {
        condition: Condition,
        then_stmts: Vec<Stmt>,
        else_stmts: Vec<Stmt>,
        span: Span,
    },

    /// `EVALUATE subject [ALSO subject …] WHEN … [WHEN OTHER …] END-EVALUATE`
    Evaluate {
        /// One or more subjects (more than one when `ALSO` is used).
        subjects: Vec<EvalSubject>,
        whens: Vec<WhenClause>,
        other_stmts: Vec<Stmt>,
        span: Span,
    },

    /// `PERFORM …`
    Perform { target: PerformTarget, span: Span },

    /// `SEARCH [ALL] table [VARYING idx] [AT END …] {WHEN cond …}… END-SEARCH`
    Search {
        all: bool,
        table: Expr,
        varying: Option<Expr>,
        at_end: Vec<Stmt>,
        whens: Vec<(Condition, Vec<Stmt>)>,
        span: Span,
    },

    /// `GO TO paragraph [{OF|IN} section]`
    GoTo {
        target: String,
        span: Span,
        /// `GO TO paragraph {OF|IN} section` — the qualifier picks which of two
        /// like-named paragraphs is meant. NC208A declares `PAR-4B` in both
        /// `QUAL-SECTION-1` and `QUAL-SECTION-2`, and unqualified resolution
        /// takes the first definition anywhere in the program. `None` = the
        /// ordinary unqualified form.
        ///
        /// New fields go at the END — the AST is bincode-serialized by
        /// declaration order.
        section: Option<String>,
    },

    /// `GO TO paragraph … DEPENDING ON data-item`
    GoToDepending {
        targets: Vec<String>,
        depending: Expr,
        span: Span,
    },

    /// `CONTINUE`
    Continue { span: Span },

    /// `EXIT [PROGRAM | PERFORM [CYCLE] | PARAGRAPH | SECTION]`
    Exit { kind: ExitKind, span: Span },

    /// `NEXT SENTENCE`
    NextSentence { span: Span },

    /// Synthetic marker inserted by the parser at each sentence boundary (the
    /// period between sentences of a paragraph). A no-op at execution; used to
    /// implement `NEXT SENTENCE` (skip to the statement after the next marker).
    SentenceEnd { span: Span },

    // ── I/O ──────────────────────────────────────────────────────────────────
    /// `OPEN mode file … [SHARING WITH …] [WITH LOCK]`
    Open {
        mode: OpenMode,
        files: Vec<String>,
        /// `SHARING WITH {ALL OTHER | NO OTHER | READ ONLY}` (advisory in the
        /// single-run-unit model; `None` = default).
        sharing: Option<ShareMode>,
        /// `WITH LOCK` — open the file exclusively.
        lock: bool,
        /// `WITH REGISTERED USER {literal | data-item}` (PowerRustCOBOL
        /// extension): the operator/user opening the file, recorded in the
        /// per-file observability log. `None` = not supplied.
        registered_user: Option<Expr>,
        span: Span,
        /// The second and later `mode files…` groups of a multi-phrase `OPEN`
        /// (`OPEN INPUT SQ-FS1 OUTPUT SQ-FS3.`). Appended rather than folded
        /// into `mode`/`files` because the AST is bincode-serialized by field
        /// declaration order. Empty for the ordinary single-mode form.
        extra_modes: Vec<(OpenMode, Vec<String>)>,
    },

    /// `CLOSE file …`
    /// `CLOSE file [{REEL|UNIT} [FOR REMOVAL]] [WITH {NO REWIND|LOCK}]`.
    ///
    /// `locked` names the subset of `files` closed `WITH LOCK`, which COBOL-85
    /// says may not be reopened in the same run unit. The reel/unit phrases are
    /// multi-volume tape positioning; they parse and are accepted as no-ops on
    /// disk, so nothing is recorded for them.
    Close {
        files: Vec<String>,
        locked: Vec<String>,
        span: Span,
        /// Files closed with the `REEL` / `UNIT` phrase. On a disk file that is
        /// not the end of the file — it ends a *volume* of a multi-volume tape,
        /// and the file stays open — so it is a distinct outcome from a plain
        /// `CLOSE` rather than a noise word.
        ///
        /// 🔴 A new field goes at the END of the variant — `Stmt` is
        /// bincode-serialized field-by-field in declaration order.
        #[serde(default)]
        reel: Vec<String>,
    },

    /// `READ file [NEXT|PREVIOUS] [INTO target] [KEY IS k]`
    /// `[AT END …] [NOT AT END …] [INVALID KEY …] [NOT INVALID KEY …]`
    Read {
        file: String,
        into: Option<Expr>,
        key: Option<Expr>,
        direction: ReadDirection,
        /// `WITH LOCK` → `Some(true)`, `WITH NO LOCK` → `Some(false)`,
        /// unspecified → `None` (the file's default).
        lock: Option<bool>,
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
        /// `AT END-OF-PAGE` / `AT EOP` — run when writing this record reaches
        /// the FOOTING line of a LINAGE file's page body.
        #[serde(default)]
        at_eop: Vec<Stmt>,
        /// `NOT AT END-OF-PAGE` — run when it does not.
        #[serde(default)]
        not_at_eop: Vec<Stmt>,
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
    /// `ACCEPT target [AT …] [FROM source] [WITH …]`
    Accept {
        target: Expr,
        from: Option<AcceptSource>,
        /// Extended screen position / attributes (`AT`/`WITH`), if any.
        screen: Option<ScreenPhrase>,
        span: Span,
    },

    /// `DISPLAY operand … [AT …] [WITH …] [UPON mnemonic] [NO ADVANCING]`
    Display {
        operands: Vec<Expr>,
        upon: Option<String>,
        no_advancing: bool,
        /// Extended screen position / attributes (`AT`/`WITH`), if any.
        screen: Option<ScreenPhrase>,
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
    /// `SORT file ON KEY … {USING f… | INPUT PROCEDURE p} {GIVING f… | OUTPUT PROCEDURE p}`
    Sort {
        file: String,
        keys: Vec<SortKey>,
        duplicates: bool,
        /// Input files (`USING`) — mutually exclusive with `input_proc`.
        using: Vec<String>,
        /// Output files (`GIVING`) — mutually exclusive with `output_proc`.
        giving: Vec<String>,
        /// `INPUT PROCEDURE a [THRU b]` — the RELEASE loop may span a RANGE
        /// of sections, and CCVS85 ST106A writes exactly that
        /// (`INPUT PROCEDURE INPROC THRU INPROC-EXIT`). Dropping the THRU end
        /// performed one section of the range and released nothing.
        input_proc: Option<(String, Option<String>)>,
        output_proc: Option<(String, Option<String>)>,
        span: Span,
    },

    /// `MERGE file ON KEY … USING f… {GIVING f… | OUTPUT PROCEDURE p}`
    Merge {
        file: String,
        keys: Vec<SortKey>,
        using: Vec<String>,
        giving: Vec<String>,
        output_proc: Option<(String, Option<String>)>,
        span: Span,
    },

    /// `RELEASE record [FROM identifier]` — hand a record to a SORT.
    Release {
        record: Expr,
        from: Option<Expr>,
        span: Span,
    },

    /// `RETURN file [INTO identifier] AT END … [NOT AT END …] [END-RETURN]`
    Return {
        file: String,
        into: Option<Expr>,
        at_end: Vec<Stmt>,
        not_at_end: Vec<Stmt>,
        span: Span,
    },

    // ── Subprogram linkage ───────────────────────────────────────────────────
    /// `CALL program [USING …] [RETURNING …] [ON EXCEPTION …] [NOT ON EXCEPTION …]`
    Call {
        program: Expr,
        using: Vec<CallArg>,
        returning: Option<Expr>,
        /// Imperative run when the called program is unresolved.
        on_exception: Vec<Stmt>,
        /// Imperative run when the call resolved successfully (`NOT ON
        /// EXCEPTION` / `NOT ON OVERFLOW`).
        not_on_exception: Vec<Stmt>,
        span: Span,
    },

    /// Visual-object **method invocation** (PowerCOBOL OO):
    ///   `INVOKE Label-1 "SetCaption" USING "Hi"`   (space form)
    ///   `INVOKE me::"OpenFormSync"("F2", 10)`      (comma form, spec 037)
    ///   `Label-1::SetCaption("Hi")`
    /// `args` are the operands; `returning` receives a getter's result.
    /// `comma_form` records the `obj::"Method"(a, b)` spelling: its trailing
    /// parameters are OPTIONAL (defaulted at runtime, 037 R21), while the
    /// classic space form requires every parameter of a checked signature
    /// (037 R22 — enforced by `cobolt-semantic`).
    Invoke {
        object: String,
        method: String,
        args: Vec<Expr>,
        returning: Option<Expr>,
        comma_form: bool,
        span: Span,
    },

    /// An inline member-access **chain used as a statement**, evaluated for its
    /// effect: `Grid-1::Rows(I)::Delete()`, `Label-1::SetCaption("Hi")`,
    /// `obj::UpperCase()` (the last has no effect — the result is discarded).
    /// `expr` is always an [`Expr::Member`]; its value (if any) is dropped.
    InvokeExpr { expr: Expr, span: Span },

    /// `CANCEL program …` — drop the program(s) from memory so the next `CALL`
    /// re-initialises their storage.
    Cancel { programs: Vec<Expr>, span: Span },

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
    WindowOp { op: WindowOperation, span: Span },

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
        /// Identifies this block's compiled function (spec 041 R1/R2).
        ///
        /// Assigned in source order at parse time, and used by codegen to name
        /// the generated function and by the runtime to find it. A block whose
        /// id has nothing registered against it is a hard error, never a no-op:
        /// the executor this replaced logged unrecognised input at `debug!` and
        /// carried on, so a block of real Rust "succeeded" while doing nothing.
        block_id: u32,
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
        try_stmts: Vec<Stmt>,
        /// Name of the exception variable in the CATCH clause (e.g. `"e"`).
        exception_var: Option<String>,
        catch_stmts: Vec<Stmt>,
        /// Name bound by `CATCH RUST-EXCEPTION <name>` (spec 041 R23).
        ///
        /// A SEPARATE clause rather than a variant of the one above, because a
        /// `TRY` may carry both and each catches only its own class (R24): a
        /// contained Rust panic must never be swallowed by a plain
        /// `CATCH EXCEPTION`, which would report a memory-safety or logic fault
        /// as a business error.
        rust_exception_var: Option<String>,
        /// Body of the `CATCH RUST-EXCEPTION` clause. Empty when absent — and
        /// when absent a panic propagates after `FINALLY` (R25).
        rust_catch_stmts: Vec<Stmt>,
        finally_stmts: Vec<Stmt>,
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
    /// Every statement nested directly inside this one, in source order.
    ///
    /// # Why this lives here, with no wildcard arm
    ///
    /// Walking the statement tree used to mean writing a `match` in the walker
    /// with a `_ => {}` at the end. That compiles forever, including for the
    /// statements nobody remembered: an `EXEC RUST` block inside
    /// `TRY … END-TRY` — the *documented* way to catch a `RUST-EXCEPTION` —
    /// was invisible to codegen, so it compiled into nothing and failed at run
    /// time as "no compiled function". `ON SIZE ERROR`, `INVALID KEY`,
    /// `ON OVERFLOW`, `SEARCH … WHEN` and `AT END` had the same hole.
    ///
    /// This match is **exhaustive on purpose**. Adding a statement that carries
    /// other statements will not compile until it is listed here, next to the
    /// definition, where the omission is obvious.
    pub fn child_stmts(&self) -> Vec<&Stmt> {
        let mut out: Vec<&Stmt> = Vec::new();
        match self {
            // Arithmetic — ON SIZE ERROR / NOT ON SIZE ERROR.
            Stmt::Add {
                on_size_error,
                not_on_size_error,
                ..
            }
            | Stmt::Subtract {
                on_size_error,
                not_on_size_error,
                ..
            }
            | Stmt::Multiply {
                on_size_error,
                not_on_size_error,
                ..
            }
            | Stmt::Divide {
                on_size_error,
                not_on_size_error,
                ..
            }
            | Stmt::Compute {
                on_size_error,
                not_on_size_error,
                ..
            } => {
                out.extend(on_size_error.iter());
                out.extend(not_on_size_error.iter());
            }

            Stmt::If {
                then_stmts,
                else_stmts,
                ..
            } => {
                out.extend(then_stmts.iter());
                out.extend(else_stmts.iter());
            }

            Stmt::Evaluate {
                whens, other_stmts, ..
            } => {
                for w in whens {
                    out.extend(w.stmts.iter());
                }
                out.extend(other_stmts.iter());
            }

            Stmt::Perform { target, .. } => match target {
                PerformTarget::Inline { stmts }
                | PerformTarget::Times { stmts, .. }
                | PerformTarget::Until { stmts, .. }
                | PerformTarget::Varying { stmts, .. } => out.extend(stmts.iter()),
                PerformTarget::Paragraph(..)
                | PerformTarget::Section(..)
                | PerformTarget::QualifiedParagraph { .. }
                | PerformTarget::Thru { .. } => {}
            },

            Stmt::Search { at_end, whens, .. } => {
                out.extend(at_end.iter());
                for (_, stmts) in whens {
                    out.extend(stmts.iter());
                }
            }

            Stmt::Read {
                at_end,
                not_at_end,
                invalid_key,
                not_invalid_key,
                ..
            } => {
                out.extend(at_end.iter());
                out.extend(not_at_end.iter());
                out.extend(invalid_key.iter());
                out.extend(not_invalid_key.iter());
            }

            Stmt::Write {
                invalid_key,
                not_invalid_key,
                ..
            }
            | Stmt::Rewrite {
                invalid_key,
                not_invalid_key,
                ..
            }
            | Stmt::Delete {
                invalid_key,
                not_invalid_key,
                ..
            }
            | Stmt::Start {
                invalid_key,
                not_invalid_key,
                ..
            } => {
                out.extend(invalid_key.iter());
                out.extend(not_invalid_key.iter());
            }

            Stmt::String_ {
                on_overflow,
                not_on_overflow,
                ..
            }
            | Stmt::Unstring {
                on_overflow,
                not_on_overflow,
                ..
            } => {
                out.extend(on_overflow.iter());
                out.extend(not_on_overflow.iter());
            }

            Stmt::Return {
                at_end,
                not_at_end,
                ..
            } => {
                out.extend(at_end.iter());
                out.extend(not_at_end.iter());
            }

            Stmt::Call {
                on_exception,
                not_on_exception,
                ..
            } => {
                out.extend(on_exception.iter());
                out.extend(not_on_exception.iter());
            }

            Stmt::TryCatch {
                try_stmts,
                catch_stmts,
                rust_catch_stmts,
                finally_stmts,
                ..
            } => {
                out.extend(try_stmts.iter());
                out.extend(catch_stmts.iter());
                out.extend(rust_catch_stmts.iter());
                out.extend(finally_stmts.iter());
            }

            // Leaves — no nested statements. Listed rather than wildcarded so a
            // new statement with a body cannot slip in unnoticed.
            Stmt::Move { .. }
            | Stmt::MoveCorresponding { .. }
            | Stmt::AddCorresponding { .. }
            | Stmt::SubtractCorresponding { .. }
            | Stmt::Initialize { .. }
            | Stmt::Alter { .. }
            | Stmt::Unlock { .. }
            | Stmt::Commit { .. }
            | Stmt::Rollback { .. }
            | Stmt::SetPointer { .. }
            | Stmt::GoTo { .. }
            | Stmt::GoToDepending { .. }
            | Stmt::Continue { .. }
            | Stmt::Exit { .. }
            | Stmt::NextSentence { .. }
            | Stmt::SentenceEnd { .. }
            | Stmt::Open { .. }
            | Stmt::Close { .. }
            | Stmt::Accept { .. }
            | Stmt::Display { .. }
            | Stmt::Inspect { .. }
            | Stmt::Sort { .. }
            | Stmt::Merge { .. }
            | Stmt::Release { .. }
            | Stmt::Invoke { .. }
            | Stmt::InvokeExpr { .. }
            | Stmt::Cancel { .. }
            | Stmt::Stop { .. }
            | Stmt::GoBack { .. }
            | Stmt::WindowOp { .. }
            | Stmt::ControlSet { .. }
            | Stmt::ExecRust { .. }
            | Stmt::Throw { .. } => {}
        }
        out
    }

    /// Visit this statement and every statement nested inside it, depth-first
    /// in source order.
    pub fn walk(&self, f: &mut impl FnMut(&Stmt)) {
        f(self);
        for child in self.child_stmts() {
            child.walk(f);
        }
    }

    /// Return the source span of this statement.
    pub fn span(&self) -> Span {
        match self {
            Stmt::Move { span, .. } => *span,
            Stmt::MoveCorresponding { span, .. } => *span,
            Stmt::AddCorresponding { span, .. } => *span,
            Stmt::SubtractCorresponding { span, .. } => *span,
            Stmt::Initialize { span, .. } => *span,
            Stmt::Add { span, .. } => *span,
            Stmt::Subtract { span, .. } => *span,
            Stmt::Multiply { span, .. } => *span,
            Stmt::Divide { span, .. } => *span,
            Stmt::Compute { span, .. } => *span,
            Stmt::If { span, .. } => *span,
            Stmt::Evaluate { span, .. } => *span,
            Stmt::Perform { span, .. } => *span,
            Stmt::Search { span, .. } => *span,
            Stmt::GoTo { span, .. } => *span,
            Stmt::GoToDepending { span, .. } => *span,
            Stmt::Continue { span } => *span,
            Stmt::Alter { span, .. } => *span,
            Stmt::Unlock { span, .. } => *span,
            Stmt::Commit { span } => *span,
            Stmt::Rollback { span } => *span,
            Stmt::SetPointer { span, .. } => *span,
            Stmt::Exit { span, .. } => *span,
            Stmt::NextSentence { span } => *span,
            Stmt::SentenceEnd { span } => *span,
            Stmt::Open { span, .. } => *span,
            Stmt::Close { span, .. } => *span,
            Stmt::Read { span, .. } => *span,
            Stmt::Write { span, .. } => *span,
            Stmt::Rewrite { span, .. } => *span,
            Stmt::Delete { span, .. } => *span,
            Stmt::Start { span, .. } => *span,
            Stmt::Accept { span, .. } => *span,
            Stmt::Display { span, .. } => *span,
            Stmt::String_ { span, .. } => *span,
            Stmt::Unstring { span, .. } => *span,
            Stmt::Inspect { span, .. } => *span,
            Stmt::Sort { span, .. } => *span,
            Stmt::Merge { span, .. } => *span,
            Stmt::Release { span, .. } => *span,
            Stmt::Return { span, .. } => *span,
            Stmt::Call { span, .. } => *span,
            Stmt::Invoke { span, .. } => *span,
            Stmt::InvokeExpr { span, .. } => *span,
            Stmt::Cancel { span, .. } => *span,
            Stmt::Stop { span, .. } => *span,
            Stmt::GoBack { span } => *span,
            Stmt::WindowOp { span, .. } => *span,
            Stmt::ControlSet { span, .. } => *span,
            Stmt::ExecRust { span, .. } => *span,
            Stmt::TryCatch { span, .. } => *span,
            Stmt::Throw { span, .. } => *span,
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
    /// `CONVERTING from TO to [BEFORE|AFTER INITIAL delimiter]` — the same
    /// conversion restricted to a region of the item.
    ///
    /// ⚠️ New variants go at the END — this enum is bincode-serialized into
    /// every compiled binary and a variant is identified by its ordinal.
    ConvertingIn {
        from: Expr,
        to: Expr,
        region: InspectRegion,
    },
}
