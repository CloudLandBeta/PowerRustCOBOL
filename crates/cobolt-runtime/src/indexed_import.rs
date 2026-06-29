// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Build `.cidx` definitions from on-disk schema (import flow).

use std::path::Path;

use cobolt_indexed::{
    apply_default_controls, store_path, AccessMode, FieldUsage, IndexedDefinition, IndexedField,
    KeyDef, KeyEncodingDef, KeyOrderingDef, KeyPartDef, KeySchema, RecordFormatDef, StorageMode,
};

use crate::indexed::{IndexedFile, IndexedFileInfo, KeyDescriptor, KeyEncoding, RecordFormat};
use crate::indexed_disk::DiskIndexedFile;

/// Read schema from an indexed data file without opening for I/O.
pub fn inspect_any_path(path: impl AsRef<Path>) -> std::io::Result<Option<IndexedFileInfo>> {
    let path = path.as_ref();
    if let Some(info) = IndexedFile::inspect_path(path)? {
        return Ok(Some(info));
    }
    DiskIndexedFile::inspect_path(path)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e.to_string()))
}

fn encoding_from_runtime(enc: KeyEncoding) -> KeyEncodingDef {
    match enc {
        KeyEncoding::Bytes => KeyEncodingDef::Bytes,
        KeyEncoding::DisplayAscii => KeyEncodingDef::DisplayAscii,
        KeyEncoding::DisplayUtf8 => KeyEncodingDef::DisplayUtf8,
        KeyEncoding::Ucs2Le => KeyEncodingDef::Ucs2Le,
        KeyEncoding::Ucs2Be => KeyEncodingDef::Ucs2Be,
        KeyEncoding::Utf32Le => KeyEncodingDef::Utf32Le,
        KeyEncoding::Utf32Be => KeyEncodingDef::Utf32Be,
        KeyEncoding::PackedDecimal => KeyEncodingDef::PackedDecimal,
        KeyEncoding::BinaryBigEndian => KeyEncodingDef::BinaryBigEndian,
        KeyEncoding::BinaryLittleEndian => KeyEncodingDef::BinaryLittleEndian,
    }
}

fn map_key(k: &KeyDescriptor) -> KeyDef {
    KeyDef {
        name: k.name.clone(),
        parts: k
            .parts
            .iter()
            .map(|p| KeyPartDef {
                field_name: k.name.clone().unwrap_or_else(|| "KEY".into()),
                offset: p.offset,
                length: p.length,
                encoding: encoding_from_runtime(p.encoding),
            })
            .collect(),
        duplicates_allowed: k.duplicates_allowed,
        ordering: match k.ordering {
            crate::indexed::KeyOrdering::Ascending => KeyOrderingDef::Ascending,
            crate::indexed::KeyOrdering::Descending => KeyOrderingDef::Descending,
        },
    }
}

/// Synthesize leaf fields from key parts + filler gaps covering `record_len` bytes.
pub fn fields_from_schema(info: &IndexedFileInfo, record_len: u32) -> Vec<IndexedField> {
    let mut spans: Vec<(u32, u32, String)> = Vec::new();

    let mut add_key = |k: &KeyDescriptor| {
        for p in &k.parts {
            let name = k
                .name
                .clone()
                .unwrap_or_else(|| format!("KEY-{}", p.offset));
            spans.push((p.offset, p.length, name));
        }
    };
    add_key(&info.primary);
    for alt in &info.alternates {
        add_key(alt);
    }
    spans.sort_by_key(|(o, _, _)| *o);

    let mut leaves = Vec::new();
    let mut pos = 0u32;
    let mut filler_n = 0u32;
    for (offset, length, name) in spans {
        if offset > pos {
            filler_n += 1;
            leaves.push(IndexedField {
                level: 5,
                name: format!("FILLER-{filler_n}"),
                pic: format!("X({})", offset - pos),
                usage: FieldUsage::Display,
                offset: Some(pos),
                length: Some(offset - pos),
                comment: String::new(),
                grid_control: None,
                occurs: None,
                redefines: None,
                synchronized: false,
                children: Vec::new(),
            });
        }
        if !leaves.iter().any(|f: &IndexedField| f.name == name) {
            let pic = if length == 1 {
                "X".into()
            } else {
                format!("X({length})")
            };
            leaves.push(IndexedField {
                level: 5,
                name,
                pic,
                usage: FieldUsage::Display,
                offset: Some(offset),
                length: Some(length),
                comment: String::new(),
                grid_control: None,
                occurs: None,
                redefines: None,
                synchronized: false,
                children: Vec::new(),
            });
        }
        pos = pos.max(offset + length);
    }
    if pos < record_len {
        filler_n += 1;
        leaves.push(IndexedField {
            level: 5,
            name: format!("FILLER-{filler_n}"),
            pic: format!("X({})", record_len - pos),
            usage: FieldUsage::Display,
            offset: Some(pos),
            length: Some(record_len - pos),
            comment: String::new(),
            grid_control: None,
            occurs: None,
            redefines: None,
            synchronized: false,
            children: Vec::new(),
        });
    }

    vec![IndexedField {
        level: 1,
        name: "IMPORTED-RECORD".into(),
        pic: String::new(),
        usage: FieldUsage::Display,
        offset: None,
        length: None,
        comment: String::new(),
        grid_control: None,
        occurs: None,
        redefines: None,
        synchronized: false,
        children: leaves,
    }]
}

/// Build a finalized definition from inspected schema + disk path.
pub fn definition_from_inspect(
    logical_name: &str,
    project_root: &Path,
    data_abs: &Path,
    info: &IndexedFileInfo,
) -> IndexedDefinition {
    let record_format = match info.record_format {
        RecordFormat::Fixed { length } => RecordFormatDef::Fixed { length },
        RecordFormat::Variable {
            min_length,
            max_length,
        } => RecordFormatDef::Variable {
            min_length,
            max_length,
        },
    };
    let record_len = info.record_format.max_len();
    let mut def = IndexedDefinition {
        name: logical_name.into(),
        assign_path: store_path(project_root, data_abs),
        access_mode: AccessMode::Dynamic,
        record_format,
        storage: StorageMode::Disk,
        compression: false,
        persistence: false,
        comment: String::new(),
        keys: KeySchema {
            primary: map_key(&info.primary),
            alternates: info.alternates.iter().map(map_key).collect(),
        },
        fields: fields_from_schema(info, record_len),
        finalized: true,
    };
    apply_default_controls(&mut def.fields);
    def
}
