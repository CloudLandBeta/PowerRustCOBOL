// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Top-level program structure and division nodes.

use cobolt_lexer::Span;
use serde::{Deserialize, Serialize};

use crate::data::{DataDecl, FileDescription, ScreenItem};
use crate::stmt::Stmt;

// ── IDENTIFICATION DIVISION ───────────────────────────────────────────────────

/// IDENTIFICATION DIVISION (also accepted as ID DIVISION).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct IdentificationDivision {
    /// The value of the PROGRAM-ID paragraph.
    pub program_id: String,
    /// Optional AUTHOR paragraph.
    pub author: Option<String>,
    /// Optional INSTALLATION paragraph.
    pub installation: Option<String>,
    /// Optional DATE-WRITTEN paragraph.
    pub date_written: Option<String>,
    /// Optional DATE-COMPILED paragraph.
    pub date_compiled: Option<String>,
    /// Optional SECURITY paragraph.
    pub security: Option<String>,
    pub span: Span,
}

// ── ENVIRONMENT DIVISION ──────────────────────────────────────────────────────

/// ENVIRONMENT DIVISION.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EnvironmentDivision {
    pub configuration: Option<ConfigurationSection>,
    pub input_output: Option<InputOutputSection>,
    pub span: Span,
}

/// CONFIGURATION SECTION.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ConfigurationSection {
    /// SOURCE-COMPUTER paragraph value.
    pub source_computer: Option<String>,
    /// OBJECT-COMPUTER paragraph value.
    pub object_computer: Option<String>,
    /// SPECIAL-NAMES paragraph entries.
    pub special_names: Vec<SpecialName>,
    pub span: Span,
}

/// A single SPECIAL-NAMES entry mapping a mnemonic to a device or switch.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SpecialName {
    pub system_name: String,
    pub mnemonic: String,
    pub span: Span,
}

/// INPUT-OUTPUT SECTION.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct InputOutputSection {
    /// FILE-CONTROL paragraph entries.
    pub file_controls: Vec<FileControl>,
    pub span: Span,
}

/// A SELECT … ASSIGN entry in FILE-CONTROL.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FileControl {
    /// The logical file name used in COBOL statements.
    pub name: String,
    /// The ASSIGN TO value (device/path string).
    pub assign: String,
    /// File organisation: SEQUENTIAL, RELATIVE, INDEXED.
    pub organization: FileOrganization,
    /// ACCESS MODE.
    pub access: AccessMode,
    /// RECORD KEY data-item name (for INDEXED files).
    pub record_key: Option<String>,
    /// ALTERNATE RECORD KEY entries.
    pub alternate_keys: Vec<AlternateKey>,
    /// FILE STATUS data-item name.
    pub file_status: Option<String>,
    /// STORAGE IS MEMORY | DISK (INDEXED files; PowerRustCOBOL extension).
    pub storage_mode: StorageMode,
    /// WITH COMPRESSION — compress stored record data (memory + disk).
    pub data_compressing: bool,
    pub span: Span,
}

/// File organisation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FileOrganization {
    /// Record SEQUENTIAL — fixed-length records, no line terminators.
    Sequential,
    /// LINE SEQUENTIAL — text records terminated by a newline; trailing spaces
    /// in a written record are not stored.
    LineSequential,
    Relative,
    Indexed,
}

/// Access mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AccessMode {
    Sequential,
    Random,
    Dynamic,
}

/// Where an INDEXED file's data lives at runtime (PowerRustCOBOL extension):
/// `MEMORY` is the in-RAM `BTreeMap` engine (whole file in memory, persisted to
/// the ASSIGN path); `DISK` is the persistent paged B+tree engine (records stay
/// on disk, fetched on demand). **The default (no `STORAGE` clause) is `DISK`.**
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum StorageMode {
    Memory,
    #[default]
    Disk,
}

/// An ALTERNATE RECORD KEY clause.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AlternateKey {
    pub field: String,
    pub with_duplicates: bool,
}

// ── DATA DIVISION ─────────────────────────────────────────────────────────────

/// DATA DIVISION.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DataDivision {
    pub sections: Vec<DataSection>,
    pub span: Span,
}

/// A section within the DATA DIVISION.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum DataSection {
    FileSection(Vec<FileDescription>),
    WorkingStorage(Vec<DataDecl>),
    LocalStorage(Vec<DataDecl>),
    Linkage(Vec<DataDecl>),
    Screen(Vec<ScreenItem>),
}

// ── PROCEDURE DIVISION ────────────────────────────────────────────────────────

/// PROCEDURE DIVISION.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProcedureDivision {
    /// Optional USING clause for subprogram parameters.
    pub using: Vec<String>,
    /// Optional RETURNING data item.
    pub returning: Option<String>,
    /// `DECLARATIVES … END DECLARATIVES` error-handling procedures, parsed from
    /// the head of the division. Empty when the program has no declaratives.
    pub declaratives: Vec<UseProcedure>,
    /// Sections, or bare paragraphs when there are no sections.
    pub body: ProcedureBody,
    pub span: Span,
}

/// A single `USE AFTER STANDARD ERROR PROCEDURE` declarative and its handler
/// statements (the body of its declarative SECTION, flattened).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct UseProcedure {
    /// `ON file-1 [file-2 …]` — specific files this handler covers (uppercased).
    pub files: Vec<String>,
    /// `ON INPUT/OUTPUT/I-O/EXTEND` — open-modes this handler covers.
    pub modes: Vec<UseMode>,
    /// True when neither files nor modes were named (applies to every file).
    pub catch_all: bool,
    /// The handler statements (all paragraphs of the declarative section).
    pub stmts: Vec<Stmt>,
    pub span: Span,
}

/// The open-mode a `USE` declarative applies to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum UseMode {
    Input,
    Output,
    Io,
    Extend,
}

/// The body of a PROCEDURE DIVISION.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ProcedureBody {
    /// Programs that use sections.
    Sections(Vec<Section>),
    /// Programs that use only paragraphs (no sections).
    Paragraphs(Vec<Paragraph>),
}

/// A SECTION in the PROCEDURE DIVISION.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Section {
    pub name: String,
    pub paragraphs: Vec<Paragraph>,
    pub span: Span,
}

/// A named paragraph (label + statements).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Paragraph {
    pub name: String,
    pub stmts: Vec<Stmt>,
    pub span: Span,
}

// ── Top-level Program ─────────────────────────────────────────────────────────

/// A complete compiled COBOL program, optionally containing nested programs.
///
/// This is the root node produced by the parser and consumed by the
/// semantic analyzer and runtime.
///
/// COBOL-85 nested programs appear in `nested_programs`.  Each nested program
/// has its own IDENTIFICATION / DATA / PROCEDURE DIVISIONs and is terminated
/// by `END PROGRAM name.`.  Nested programs can access `GLOBAL` data items
/// declared in the enclosing program without redeclaring them.  `EXTERNAL`
/// items must be re-declared but share the same storage across all programs
/// in the run unit.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Program {
    pub identification:   IdentificationDivision,
    pub environment:      Option<EnvironmentDivision>,
    pub data:             Option<DataDivision>,
    pub procedure:        ProcedureDivision,
    /// COBOL-85 nested programs contained within this program's scope.
    /// Parsed from the region between the last paragraph and `END PROGRAM`.
    pub nested_programs:  Vec<Program>,
    /// The name from `END PROGRAM name.` — `None` for a top-level program
    /// that has no closing `END PROGRAM` statement.
    pub end_program_name: Option<String>,
    /// `SPECIAL-NAMES. DECIMAL-POINT IS COMMA` — comma is the decimal separator
    /// for numeric literals and edited PICs (period becomes grouping insertion).
    pub decimal_comma: bool,
    pub span: Span,
}
