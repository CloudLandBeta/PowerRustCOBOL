// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! DATA DIVISION node types: data declarations, PIC clauses, and USAGE.

use cobolt_lexer::Span;
use serde::{Deserialize, Serialize};

use crate::expr::{Expr, Literal};

// ── PIC clause ────────────────────────────────────────────────────────────────

/// The category of a PICTURE clause.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PicKind {
    Alphabetic,           // PIC A(n)
    Numeric,              // PIC 9(n)
    Alphanumeric,         // PIC X(n)
    NumericEdited,        // PIC Z,9 / $,Z.99 / etc.
    AlphanumericEdited,   // PIC X(n)B / etc.
}

/// A parsed PICTURE clause.
///
/// The `template` field preserves the raw template string (e.g. `"9(5)V99"`)
/// so that code generators can reproduce it exactly.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PicClause {
    /// Raw template string as written in the source (`"9(5)V99"`, `"X(30)"`, …).
    pub template: String,
    /// High-level category of this picture.
    pub kind: PicKind,
    /// Number of integer digits (before the implied decimal point), or the total
    /// character width for alphanumeric/alphabetic pictures. `u16` so wide fields
    /// such as `PIC X(4096)` / `PIC X(32767)` are represented exactly.
    pub digits: u16,
    /// Number of decimal digits (after the implied decimal point V).
    pub decimals: u16,
    pub span: Span,
}

// ── USAGE clause ──────────────────────────────────────────────────────────────

/// The USAGE clause for a data item.
///
/// Determines the internal machine representation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum Usage {
    /// Character string representation (default).
    #[default]
    Display,
    /// Native binary integer (COMP / COMPUTATIONAL / COMP-4).
    Binary,
    /// Alias for `Binary` — accepted by Fujitsu COBOL.
    Comp,
    /// 32-bit IEEE floating point (COMP-1).
    Comp1,
    /// 64-bit IEEE floating point (COMP-2).
    Comp2,
    /// Packed-decimal / BCD (COMP-3 / COMPUTATIONAL-3 / PACKED-DECIMAL).
    Comp3,
    /// Native binary, no sign extension (COMP-5).
    Comp5,
    /// Synonym for `Comp3`.
    PackedDecimal,
    /// Table index register.
    Index,
    /// Memory address pointer.
    Pointer,
}

// ── OCCURS clause ─────────────────────────────────────────────────────────────

/// An OCCURS clause defining a table dimension.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OccursClause {
    /// Minimum number of occurrences (0 for fixed-length OCCURS n TIMES).
    pub min: u32,
    /// Maximum (and exact for fixed-length) number of occurrences.
    pub max: u32,
    /// The DEPENDING ON data item for variable-length tables.
    pub depending_on: Option<String>,
    /// Names of index items (INDEXED BY).
    pub indexed_by: Vec<String>,
    pub span: Span,
}

// ── Data item declaration ─────────────────────────────────────────────────────

/// A single data item declaration (one line of the DATA DIVISION).
///
/// Nested group items are represented as a tree via `children`.
///
/// ```text
/// 01 WS-RECORD.
///    05 WS-NAME    PIC X(30).
///    05 WS-AMOUNT  PIC 9(7)V99 COMP-3.
/// ```
///
/// `WS-RECORD` would have `children = [WS-NAME, WS-AMOUNT]`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DataDecl {
    /// Level number (1–49, 66, 77, 78, 88).
    pub level: u8,
    /// Item name.  `None` for `FILLER` items.
    pub name: Option<String>,
    /// PICTURE clause.  Absent for group items.
    pub picture: Option<PicClause>,
    /// VALUE clause initial value.
    pub value: Option<Literal>,
    /// USAGE clause (defaults to `Display`).
    pub usage: Usage,
    /// OCCURS clause for table items.
    pub occurs: Option<OccursClause>,
    /// REDEFINES clause — name of the item being redefined.
    pub redefines: Option<String>,
    /// 66-level `RENAMES item-1 [THRU item-2]` regrouping, if this is a 66 item.
    pub renames: Option<RenamesClause>,
    /// For 88-level condition names: the list of values that make it TRUE.
    pub condition_values: Vec<ConditionValue>,
    /// GLOBAL clause — item is visible to all nested programs in this compilation unit.
    pub is_global:   bool,
    /// EXTERNAL clause — item is shared across all programs in the run unit.
    pub is_external: bool,
    /// BLANK WHEN ZERO — an edited field is blanked when its value is zero.
    pub blank_when_zero: bool,
    /// Nested subordinate data items (group items only).
    pub children: Vec<DataDecl>,
    pub span: Span,
}

/// A 66-level `RENAMES item-1 [THRU item-2]` regrouping clause.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RenamesClause {
    pub from: String,
    pub thru: Option<String>,
}

/// A value entry in an 88-level condition-name declaration.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ConditionValue {
    /// `VALUE literal`
    Single(Literal),
    /// `VALUE literal THRU literal`
    Range(Literal, Literal),
}

// ── File description ──────────────────────────────────────────────────────────

/// An FD (File Description) entry in the FILE SECTION.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FileDescription {
    /// The file name as referenced in SELECT … ASSIGN.
    pub name: String,
    /// Record descriptions belonging to this file.
    pub records: Vec<DataDecl>,
    pub span: Span,
}

// ── Screen items ──────────────────────────────────────────────────────────────

/// A single item in the SCREEN SECTION.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ScreenItem {
    pub level: u8,
    pub name: Option<String>,
    pub picture: Option<PicClause>,
    pub from: Option<Expr>,
    pub to: Option<Expr>,
    pub using: Option<Expr>,
    pub foreground: Option<u8>,
    pub background: Option<u8>,
    pub highlight: bool,
    pub reverse: bool,
    pub blink: bool,
    pub children: Vec<ScreenItem>,
    pub span: Span,
}
