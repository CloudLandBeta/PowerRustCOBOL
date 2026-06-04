// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Symbol table — built from a parsed `Program` and used by all subsequent
//! semantic passes.
//!
//! The table has three sections:
//!
//! * **Data items** — every named item from every DATA DIVISION section.
//!   Group items carry their children too, but each item is also indexed
//!   flat by name for O(1) lookup.
//! * **Paragraphs** — procedure paragraph definitions.
//! * **Sections** — procedure section definitions.

use indexmap::IndexMap;

use cobolt_ast::{
    data::{DataDecl, PicKind, Usage},
    program::{DataSection, ProcedureBody, Program},
};
use cobolt_lexer::Span;

// ── DataItemInfo ──────────────────────────────────────────────────────────────

/// Information about a single named data item in the DATA DIVISION.
#[derive(Debug, Clone)]
pub struct DataItemInfo {
    /// COBOL name in uppercase with hyphens preserved: `"WS-COUNT"`.
    pub cobol_name: String,
    /// Rust-friendly snake_case name: `"ws_count"`.
    pub rust_name: String,
    /// Level number (1–49, 66, 77, 78, 88).
    pub level: u8,
    /// PIC category, if present (absent for group items and 88-levels).
    pub pic_kind: Option<PicKind>,
    /// Usage / storage class.
    pub usage: Usage,
    /// `true` if this item is a group (no PIC clause and has children).
    pub is_group: bool,
    /// Source location of the declaration.
    pub span: Span,
}

impl DataItemInfo {
    /// Convert a COBOL data-item name to its Rust snake_case equivalent.
    ///
    /// `WS-MY-FIELD` → `ws_my_field`
    pub fn cobol_to_rust(cobol: &str) -> String {
        cobol.to_ascii_lowercase().replace('-', "_")
    }

    fn from_decl(decl: &DataDecl) -> Option<Self> {
        let cobol_name = decl.name.as_ref()?.to_ascii_uppercase();
        let rust_name = Self::cobol_to_rust(&cobol_name);
        Some(DataItemInfo {
            cobol_name,
            rust_name,
            level: decl.level,
            pic_kind: decl.picture.as_ref().map(|p| p.kind),
            usage: decl.usage,
            is_group: decl.picture.is_none() && !decl.children.is_empty(),
            span: decl.span,
        })
    }
}

// ── ParagraphInfo / SectionInfo ───────────────────────────────────────────────

/// A paragraph defined in the PROCEDURE DIVISION.
#[derive(Debug, Clone)]
pub struct ParagraphInfo {
    pub name: String,
    pub span: Span,
}

/// A section defined in the PROCEDURE DIVISION.
#[derive(Debug, Clone)]
pub struct SectionInfo {
    pub name: String,
    pub span: Span,
    pub paragraphs: Vec<String>,
}

// ── SymbolTable ───────────────────────────────────────────────────────────────

/// The complete symbol table for one COBOL program.
#[derive(Debug, Default)]
pub struct SymbolTable {
    /// Flat map from COBOL name (uppercase) → data item info.
    /// Populated depth-first from all DATA DIVISION sections.
    data_items: IndexMap<String, DataItemInfo>,
    /// Paragraphs, in definition order.
    paragraphs: IndexMap<String, ParagraphInfo>,
    /// Sections (if the PROCEDURE DIVISION uses sections).
    sections: IndexMap<String, SectionInfo>,
}

impl SymbolTable {
    /// Build a symbol table from a complete parsed program.
    pub fn build(program: &Program) -> Self {
        let mut table = SymbolTable::default();

        // ── DATA DIVISION ─────────────────────────────────────────────────────
        if let Some(data_div) = &program.data {
            for section in &data_div.sections {
                let items = match section {
                    DataSection::WorkingStorage(items)
                    | DataSection::LocalStorage(items)
                    | DataSection::Linkage(items) => items.as_slice(),
                    DataSection::FileSection(fds) => {
                        // Index records declared inside each FD.
                        for fd in fds {
                            for rec in &fd.records {
                                table.index_data_decl(rec);
                            }
                        }
                        continue;
                    }
                    // Screen items are not addressable as COBOL data items
                    // in the same way; skip for now.
                    DataSection::Screen(_) => continue,
                };
                for decl in items {
                    table.index_data_decl(decl);
                }
            }
        }

        // ── PROCEDURE DIVISION ────────────────────────────────────────────────
        match &program.procedure.body {
            ProcedureBody::Paragraphs(paras) => {
                for para in paras {
                    table.paragraphs.insert(
                        para.name.to_ascii_uppercase(),
                        ParagraphInfo {
                            name: para.name.clone(),
                            span: para.span,
                        },
                    );
                }
            }
            ProcedureBody::Sections(secs) => {
                for sec in secs {
                    let para_names: Vec<String> = sec
                        .paragraphs
                        .iter()
                        .map(|p| p.name.clone())
                        .collect();
                    for para in &sec.paragraphs {
                        table.paragraphs.insert(
                            para.name.to_ascii_uppercase(),
                            ParagraphInfo {
                                name: para.name.clone(),
                                span: para.span,
                            },
                        );
                    }
                    table.sections.insert(
                        sec.name.to_ascii_uppercase(),
                        SectionInfo {
                            name: sec.name.clone(),
                            span: sec.span,
                            paragraphs: para_names,
                        },
                    );
                }
            }
        }

        table
    }

    /// Recursively index a data declaration and all its children.
    fn index_data_decl(&mut self, decl: &DataDecl) {
        if let Some(info) = DataItemInfo::from_decl(decl) {
            self.data_items.insert(info.cobol_name.clone(), info);
        }
        // Register any INDEXED BY index-names as synthetic numeric items so
        // that `SET`/`SEARCH` references to them are recognised.
        if let Some(occurs) = &decl.occurs {
            for ix in &occurs.indexed_by {
                let cobol_name = ix.to_ascii_uppercase();
                let rust_name = DataItemInfo::cobol_to_rust(&cobol_name);
                self.data_items
                    .entry(cobol_name.clone())
                    .or_insert(DataItemInfo {
                        cobol_name,
                        rust_name,
                        level: 77,
                        pic_kind: Some(PicKind::Numeric),
                        usage: Usage::Index,
                        is_group: false,
                        span: decl.span,
                    });
            }
        }
        for child in &decl.children {
            self.index_data_decl(child);
        }
    }

    // ── Query API ─────────────────────────────────────────────────────────────

    /// Look up a data item by its COBOL name (case-insensitive).
    pub fn data_item(&self, name: &str) -> Option<&DataItemInfo> {
        self.data_items.get(&name.to_ascii_uppercase())
    }

    /// Look up a data item by its Rust snake_case name.
    pub fn data_item_by_rust_name(&self, rust_name: &str) -> Option<&DataItemInfo> {
        self.data_items
            .values()
            .find(|info| info.rust_name == rust_name)
    }

    /// Iterate all data items in declaration order.
    pub fn data_items(&self) -> impl Iterator<Item = (&String, &DataItemInfo)> {
        self.data_items.iter()
    }

    /// `true` if a data item with the given COBOL name is declared.
    pub fn has_data_item(&self, name: &str) -> bool {
        self.data_item(name).is_some()
    }

    /// Look up a paragraph by name (case-insensitive).
    pub fn paragraph(&self, name: &str) -> Option<&ParagraphInfo> {
        self.paragraphs.get(&name.to_ascii_uppercase())
    }

    /// Look up a section by name (case-insensitive).
    pub fn section(&self, name: &str) -> Option<&SectionInfo> {
        self.sections.get(&name.to_ascii_uppercase())
    }

    /// `true` if a paragraph or section with the given name is declared.
    pub fn has_procedure(&self, name: &str) -> bool {
        let upper = name.to_ascii_uppercase();
        self.paragraphs.contains_key(&upper) || self.sections.contains_key(&upper)
    }

    /// Total number of named data items indexed.
    pub fn data_item_count(&self) -> usize {
        self.data_items.len()
    }

    /// Total number of paragraphs indexed.
    pub fn paragraph_count(&self) -> usize {
        self.paragraphs.len()
    }
}
