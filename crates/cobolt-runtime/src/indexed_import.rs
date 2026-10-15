// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Build `.cidx` definitions from on-disk schema (import flow).
//!
//! A file written by container version 4 carries the `FD` that describes its
//! records, and the import reads the field tree from it — names, PICTUREs, USAGE,
//! REDEFINES, OCCURS and where each field's bytes are. A file without one (an
//! older container, or a writer that declared none) has only its key schema, and
//! the import falls back to the keys with every other byte a `FILLER`.

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

/// The field tree an indexed file's own record definition describes.
///
/// `text` is the FD text a version 4 container carries (`cobolt_ast::fd_text`).
/// Returns `None` when it does not parse or holds no group record — the caller
/// then falls back to what the key schema alone can give.
///
/// Each field's bytes come from the `*> @offset+length` comment the writer put
/// beside it, which is the layout that file really uses; a field with none gets
/// the position the runtime's own layout rules give it. A condition name (`88`)
/// has no field of its own in a `.cidx` and is left out.
pub fn fields_from_definition(text: &str) -> Option<Vec<IndexedField>> {
    use cobolt_ast::program::DataSection;

    let source = format!(
        "IDENTIFICATION DIVISION.\nPROGRAM-ID. DEFINITION.\nENVIRONMENT DIVISION.\n\
         INPUT-OUTPUT SECTION.\nFILE-CONTROL.\n    SELECT F ASSIGN TO \"x\"\n\
         \x20       ORGANIZATION IS INDEXED RECORD KEY IS K.\nDATA DIVISION.\nFILE SECTION.\n\
         {text}\nPROCEDURE DIVISION.\n    STOP RUN.\n"
    );
    let parsed = cobolt_parser::parse(cobolt_lexer::tokenize(&source, cobolt_lexer::SourceFormat::Free));
    if parsed.diagnostics.iter().any(|d| d.is_error()) {
        return None;
    }
    let program = parsed.program?;
    let record = program.data?.sections.into_iter().find_map(|section| match section {
        DataSection::FileSection(fds) => fds.into_iter().next().and_then(|fd| fd.records.into_iter().next()),
        _ => None,
    })?;
    if record.children.iter().all(|c| c.level == 88) {
        return None; // an elementary 01: nothing a field tree can hold
    }

    // Extents in listing order: the writer's own, else the runtime's rules.
    let computed = crate::files::item_extents(&record);
    let written = extents_written_in(text);
    let extents: Vec<Option<(usize, usize)>> = if written.len() == computed.len() { written } else { computed };

    let mut listing = extents.iter();
    let mut filler = 0u32;
    let root = field_from_decl(&record, &mut listing, &mut filler)?;
    Some(vec![root])
}

/// The `*> @offset+length` comments of the first record of `text`, one entry per
/// data description in listing order (`None` where a line has none).
fn extents_written_in(text: &str) -> Vec<Option<(usize, usize)>> {
    let mut out = Vec::new();
    let mut records = 0;
    for line in text.lines() {
        let code = line.trim_start();
        let level: Option<u8> = code.split_whitespace().next().and_then(|w| w.parse().ok());
        let Some(level) = level else { continue };
        if level == 1 {
            records += 1;
            if records > 1 {
                break;
            }
        }
        if records == 0 {
            continue;
        }
        let extent = line.find("*> @").and_then(|at| {
            let (offset, len) = line[at + 4..].trim().split_once('+')?;
            Some((offset.parse().ok()?, len.split_whitespace().next()?.parse().ok()?))
        });
        out.push(extent);
    }
    out
}

fn field_from_decl(
    d: &cobolt_ast::data::DataDecl,
    listing: &mut std::slice::Iter<'_, Option<(usize, usize)>>,
    filler: &mut u32,
) -> Option<IndexedField> {
    use cobolt_ast::data::Usage;
    let extent = *listing.next()?;
    let members = d.children.iter().any(|c| c.level != 88);
    let mut children = Vec::new();
    for c in &d.children {
        if c.level == 88 {
            listing.next()?; // it holds its place in the listing, and no field
        } else if let Some(f) = field_from_decl(c, listing, filler) {
            children.push(f);
        }
    }
    let name = match &d.name {
        Some(n) => n.clone(),
        None => {
            *filler += 1;
            format!("FILLER-{filler}")
        }
    };
    let times = d.occurs.as_ref().map(|o| o.max.max(1) as usize).unwrap_or(1);
    let (offset, length) = match (members, extent) {
        (false, Some((o, l))) => (Some(o as u32), Some((l / times) as u32)),
        (false, None) => return None,
        (true, _) => (None, None),
    };
    Some(IndexedField {
        level: d.level,
        name,
        pic: if members { String::new() } else { d.picture.as_ref().map(|p| p.template.clone()).unwrap_or_default() },
        usage: match d.usage {
            Usage::Binary => FieldUsage::Binary,
            Usage::Comp | Usage::Comp5 => FieldUsage::Comp,
            Usage::Comp3 => FieldUsage::Comp3,
            Usage::PackedDecimal => FieldUsage::PackedDecimal,
            Usage::Index => FieldUsage::Index,
            Usage::Pointer => FieldUsage::Pointer,
            _ => FieldUsage::Display,
        },
        offset,
        length,
        comment: String::new(),
        grid_control: None,
        occurs: d.occurs.as_ref().map(|o| o.max),
        redefines: d.redefines.clone(),
        synchronized: false,
        children,
    })
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

/// A key part that spans exactly the bytes of one field takes that field's name,
/// so the `.cidx` says `RECORD KEY IS CUST-ID` rather than `KEY`.
fn name_key_parts_after_fields(def: &mut IndexedDefinition) {
    fn spans(f: &IndexedField, out: &mut Vec<(u32, u32, String)>) {
        if let (Some(o), Some(l)) = (f.offset, f.length) {
            out.push((o, l, f.name.clone()));
        }
        for c in &f.children {
            spans(c, out);
        }
    }
    let mut known = Vec::new();
    for f in &def.fields {
        spans(f, &mut known);
    }
    let name_of = |offset: u32, length: u32| {
        known
            .iter()
            .find(|(o, l, n)| *o == offset && *l == length && !n.starts_with("FILLER"))
            .map(|(_, _, n)| n.clone())
    };
    for key in std::iter::once(&mut def.keys.primary).chain(def.keys.alternates.iter_mut()) {
        for part in &mut key.parts {
            if let Some(n) = name_of(part.offset, part.length) {
                part.field_name = n;
            }
        }
    }
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
    // The record definition the file carries, when it carries one, names every
    // field; without it only the keys can be named.
    let from_definition = info.definition.as_deref().and_then(fields_from_definition);
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
        fields: from_definition.unwrap_or_else(|| fields_from_schema(info, record_len)),
        finalized: true,
    };
    name_key_parts_after_fields(&mut def);
    apply_default_controls(&mut def.fields);
    def
}
