// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Data model for `.cidx` indexed-file definitions.

use cobolt_forms::ControlType;

/// COBOL `ACCESS MODE` for an indexed file.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AccessMode {
    #[default]
    Dynamic,
    Sequential,
    Random,
}

impl AccessMode {
    pub fn as_str(self) -> &'static str {
        match self {
            AccessMode::Dynamic => "dynamic",
            AccessMode::Sequential => "sequential",
            AccessMode::Random => "random",
        }
    }

    pub fn from_str(s: &str) -> Self {
        match s.to_ascii_lowercase().as_str() {
            "sequential" => AccessMode::Sequential,
            "random" => AccessMode::Random,
            _ => AccessMode::Dynamic,
        }
    }
}

/// Fixed or variable record layout.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecordFormatDef {
    Fixed { length: u32 },
    Variable { min_length: u32, max_length: u32 },
}

impl RecordFormatDef {
    pub fn max_len(self) -> u32 {
        match self {
            RecordFormatDef::Fixed { length } => length,
            RecordFormatDef::Variable { max_length, .. } => max_length,
        }
    }
}

/// `STORAGE MODE IS MEMORY | DISK`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum StorageMode {
    #[default]
    Disk,
    Memory,
}

impl StorageMode {
    pub fn as_str(self) -> &'static str {
        match self {
            StorageMode::Disk => "disk",
            StorageMode::Memory => "memory",
        }
    }

    pub fn from_str(s: &str) -> Self {
        match s.to_ascii_lowercase().as_str() {
            "memory" => StorageMode::Memory,
            _ => StorageMode::Disk,
        }
    }
}

/// Field `USAGE` clause (subset used by the editor, including INDEX and POINTER
/// which are valid in COBOL-85 data descriptions and thus for indexed file
/// record items per the ANSI X3.23-1985 standard and common implementations).

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FieldUsage {
    #[default]
    Display,
    Comp,
    Comp3,
    Comp4,
    Binary,
    PackedDecimal,
    Index,
    Pointer,
}

impl FieldUsage {
    pub fn as_str(self) -> &'static str {
        match self {
            FieldUsage::Display => "display",
            FieldUsage::Comp => "comp",
            FieldUsage::Comp3 => "comp-3",
            FieldUsage::Comp4 => "comp-4",
            FieldUsage::Binary => "binary",
            FieldUsage::PackedDecimal => "packed-decimal",
            FieldUsage::Index => "index",
            FieldUsage::Pointer => "pointer",
        }
    }

    pub fn from_str(s: &str) -> Self {
        match s.to_ascii_lowercase().as_str() {
            "comp" => FieldUsage::Comp,
            "comp-3" | "comp3" => FieldUsage::Comp3,
            "comp-4" | "comp4" => FieldUsage::Comp4,
            "binary" => FieldUsage::Binary,
            "packed-decimal" => FieldUsage::PackedDecimal,
            "index" => FieldUsage::Index,
            "pointer" => FieldUsage::Pointer,
            _ => FieldUsage::Display,
        }
    }
}

/// Key encoding stored in `.cidx` (mirrors runtime `KeyEncoding`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum KeyEncodingDef {
    #[default]
    Bytes,
    DisplayAscii,
    DisplayUtf8,
    Ucs2Le,
    Ucs2Be,
    Utf32Le,
    Utf32Be,
    PackedDecimal,
    BinaryBigEndian,
    BinaryLittleEndian,
}

impl KeyEncodingDef {
    pub fn as_str(self) -> &'static str {
        match self {
            KeyEncodingDef::Bytes => "bytes",
            KeyEncodingDef::DisplayAscii => "display-ascii",
            KeyEncodingDef::DisplayUtf8 => "display-utf8",
            KeyEncodingDef::Ucs2Le => "ucs2-le",
            KeyEncodingDef::Ucs2Be => "ucs2-be",
            KeyEncodingDef::Utf32Le => "utf32-le",
            KeyEncodingDef::Utf32Be => "utf32-be",
            KeyEncodingDef::PackedDecimal => "packed-decimal",
            KeyEncodingDef::BinaryBigEndian => "binary-be",
            KeyEncodingDef::BinaryLittleEndian => "binary-le",
        }
    }

    pub fn from_str(s: &str) -> Self {
        match s.to_ascii_lowercase().as_str() {
            "display-ascii" => KeyEncodingDef::DisplayAscii,
            "display-utf8" => KeyEncodingDef::DisplayUtf8,
            "ucs2-le" => KeyEncodingDef::Ucs2Le,
            "ucs2-be" => KeyEncodingDef::Ucs2Be,
            "utf32-le" => KeyEncodingDef::Utf32Le,
            "utf32-be" => KeyEncodingDef::Utf32Be,
            "packed-decimal" => KeyEncodingDef::PackedDecimal,
            "binary-be" => KeyEncodingDef::BinaryBigEndian,
            "binary-le" => KeyEncodingDef::BinaryLittleEndian,
            _ => KeyEncodingDef::Bytes,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum KeyOrderingDef {
    #[default]
    Ascending,
    Descending,
}

impl KeyOrderingDef {
    pub fn as_str(self) -> &'static str {
        match self {
            KeyOrderingDef::Ascending => "ascending",
            KeyOrderingDef::Descending => "descending",
        }
    }

    pub fn from_str(s: &str) -> Self {
        match s.to_ascii_lowercase().as_str() {
            "descending" => KeyOrderingDef::Descending,
            _ => KeyOrderingDef::Ascending,
        }
    }
}

/// One byte range of a composite key.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeyPartDef {
    pub field_name: String,
    pub offset: u32,
    pub length: u32,
    pub encoding: KeyEncodingDef,
}

/// Primary or alternate key definition.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeyDef {
    pub name: Option<String>,
    pub parts: Vec<KeyPartDef>,
    pub duplicates_allowed: bool,
    pub ordering: KeyOrderingDef,
}

/// All keys for the file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeySchema {
    pub primary: KeyDef,
    pub alternates: Vec<KeyDef>,
}

/// One field in the record layout (tree node).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IndexedField {
    pub level: u8,
    pub name: String,
    pub pic: String,
    pub usage: FieldUsage,
    /// Byte offset from start of record (leaf fields only).
    pub offset: Option<u32>,
    /// Byte length (leaf fields only).
    pub length: Option<u32>,
    pub comment: String,
    pub grid_control: Option<ControlType>,
    /// `OCCURS n TIMES` on group items.
    pub occurs: Option<u32>,
    /// `REDEFINES` target field name.
    pub redefines: Option<String>,
    /// `SYNCHRONIZED` clause.
    pub synchronized: bool,
    pub children: Vec<IndexedField>,
}

impl IndexedField {
    pub fn is_group(&self) -> bool {
        self.offset.is_none()
    }

    pub fn is_leaf(&self) -> bool {
        self.offset.is_some()
    }
}

impl IndexedField {
    /// Stable id for tree selection (level + name path).
    pub fn id(&self) -> String {
        self.name.clone()
    }

    /// All leaf fields in preorder.
    pub fn leaves<'a>(&'a self, out: &mut Vec<&'a IndexedField>) {
        if self.children.is_empty() {
            if self.offset.is_some() {
                out.push(self);
            }
        } else {
            for c in &self.children {
                c.leaves(out);
            }
        }
    }

    pub fn all_leaves(&self) -> Vec<&IndexedField> {
        let mut v = Vec::new();
        self.leaves(&mut v);
        v
    }
}

/// Full indexed-file definition (`.cidx` root).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IndexedDefinition {
    pub name: String,
    pub assign_path: String,
    pub access_mode: AccessMode,
    pub record_format: RecordFormatDef,
    pub storage: StorageMode,
    pub compression: bool,
    pub persistence: bool,
    pub comment: String,
    pub keys: KeySchema,
    pub fields: Vec<IndexedField>,
    pub finalized: bool,
}

impl IndexedDefinition {
    pub fn new(name: impl Into<String>, assign_path: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            assign_path: assign_path.into(),
            access_mode: AccessMode::Dynamic,
            record_format: RecordFormatDef::Fixed { length: 80 },
            storage: StorageMode::Disk,
            compression: false,
            persistence: false,
            comment: String::new(),
            keys: KeySchema {
                primary: KeyDef {
                    name: None,
                    parts: Vec::new(),
                    duplicates_allowed: false,
                    ordering: KeyOrderingDef::Ascending,
                },
                alternates: Vec::new(),
            },
            fields: Vec::new(),
            finalized: false,
        }
    }

    /// Root 01-level record group (first top-level field).
    pub fn record_root(&self) -> Option<&IndexedField> {
        self.fields.first()
    }

    /// Record length from format or computed from leaf fields.
    pub fn record_length(&self) -> u32 {
        match self.record_format {
            RecordFormatDef::Fixed { length } => length,
            RecordFormatDef::Variable { max_length, .. } => max_length,
        }
    }

    /// Recursively recompute leaf field offsets sequentially, updating the record format length.
    pub fn recompute_offsets(&mut self) -> u32 {
        let Some(root) = self.fields.first_mut() else {
            return 0;
        };
        let mut off = 0u32;
        fn walk(node: &mut IndexedField, off: &mut u32) {
            if node.offset.is_some() {
                node.offset = Some(*off);
                *off += node.length.unwrap_or(0);
            }
            for child in &mut node.children {
                walk(child, off);
            }
        }
        walk(root, &mut off);
        match &mut self.record_format {
            RecordFormatDef::Fixed { length } => {
                *length = off;
            }
            RecordFormatDef::Variable { max_length, .. } => {
                *max_length = off;
            }
        }
        off
    }
}
