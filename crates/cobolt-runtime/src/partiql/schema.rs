// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! The record as an item (spec 108, §5.2): the fields of a record layout with
//! the facts a statement needs — qualifying path, occurrences, scale, sign,
//! class — and the keys of the file.
//!
//! One result from three sources: the FD text a data file carries, a `.cidx`
//! definition (which renders the same FD text), and a [`RecordLayout`] a
//! program's own `FD` produced. Offsets and widths are the runtime's own
//! (`files::compute_layout`), never a second set of rules.

use std::collections::HashSet;

use cobolt_ast::data::DataDecl;
use cobolt_indexed::IndexedDefinition;

use crate::files::{compute_layout, FieldPos, RecordLayout};
use crate::national::CharClass;

/// What a field holds, in the terms of §5.2.
#[derive(Clone, Debug, PartialEq)]
pub enum Class {
    /// A numeric PICTURE, in any usage: a Number with its scale.
    Number { decimals: u8, signed: bool },
    /// `X` or `A`: a String without trailing spaces.
    Text,
    /// `N` or `U`: a String held as a national or UTF-8 image.
    Char(CharClass),
}

impl Class {
    pub fn is_number(&self) -> bool {
        matches!(self, Class::Number { .. })
    }
}

/// One elementary item of the record (all its occurrences).
#[derive(Clone, Debug, PartialEq)]
pub struct Field {
    /// Upper-case, as the layout holds it.
    pub name: String,
    /// The enclosing groups, innermost first, the `01` excluded.
    pub quals: Vec<String>,
    /// One entry per occurrence; a plain item has exactly one.
    pub occ: Vec<FieldPos>,
    pub class: Class,
    /// The item sits under a group that has `OCCURS`: not addressable (O3).
    pub in_occurs_group: bool,
}

impl Field {
    /// The path from the outermost group: `BILLING.CITY`.
    pub fn path(&self) -> String {
        let mut s = String::new();
        for q in self.quals.iter().rev() {
            s.push_str(q);
            s.push('.');
        }
        s.push_str(&self.name);
        s
    }

    /// Whether the item itself has `OCCURS`.
    pub fn is_table(&self) -> bool {
        self.occ.len() > 1
    }
}

/// A group item: its path and how many elementary items it holds.
#[derive(Clone, Debug, PartialEq)]
pub struct Group {
    pub name: String,
    pub quals: Vec<String>,
    pub fields: usize,
}

/// One part of a key: where its bytes are and which field they are.
#[derive(Clone, Debug, PartialEq)]
pub struct KeyPart {
    pub field_name: String,
    pub offset: usize,
    pub len: usize,
    /// The field whose first occurrence is exactly these bytes; `None` when
    /// the part spans a group or part of a field (such a part is never used
    /// to plan a read, only to filter).
    pub field: Option<usize>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Key {
    /// An alternate's name (its `.cidx` name, else its first field's name,
    /// upper-case); `None` for the RECORD KEY, which has no index name.
    pub name: Option<String>,
    pub parts: Vec<KeyPart>,
    pub duplicates: bool,
}

/// A key as the caller knows it: parts as `(field name, offset, length)`.
#[derive(Clone, Debug, Default)]
pub struct KeyInput {
    pub name: Option<String>,
    pub parts: Vec<(String, usize, usize)>,
    pub duplicates: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Schema {
    /// The upper-cased `.cidx` stem: what `FROM "TABLE"` names.
    pub table: String,
    pub record_len: usize,
    pub fields: Vec<Field>,
    pub groups: Vec<Group>,
    pub primary: Key,
    pub alternates: Vec<Key>,
}

impl Schema {
    /// From a record description (the `01` item) and the file's keys.
    pub fn from_record(table: &str, record: &DataDecl, primary: KeyInput, alternates: Vec<KeyInput>) -> Schema {
        let layout = compute_layout(record);
        let mut in_table = HashSet::new();
        collect_table_members(&record.children, &mut Vec::new(), false, &mut in_table);
        Schema::from_layout_with(table, &layout, &in_table, primary, alternates)
    }

    /// From a layout a program's `FD` produced. It cannot tell an `OCCURS`
    /// group's members from an `OCCURS` item, so the refusal of O3 does not
    /// apply to this source.
    pub fn from_layout(table: &str, layout: &RecordLayout, primary: KeyInput, alternates: Vec<KeyInput>) -> Schema {
        Schema::from_layout_with(table, layout, &HashSet::new(), primary, alternates)
    }

    /// From the FD text a data file carries (or a `.cidx` renders).
    pub fn from_fd_text(
        table: &str,
        text: &str,
        primary: KeyInput,
        alternates: Vec<KeyInput>,
    ) -> Result<Schema, String> {
        let record = parse_fd_text(text)?;
        Ok(Schema::from_record(table, &record, primary, alternates))
    }

    /// The keys of a `.cidx` definition, as `Schema` constructors take them.
    pub fn keys_of_definition(def: &IndexedDefinition) -> (KeyInput, Vec<KeyInput>) {
        let key_of = |k: &cobolt_indexed::KeyDef| KeyInput {
            name: k.name.clone(),
            parts: k
                .parts
                .iter()
                .map(|p| (p.field_name.clone(), p.offset as usize, p.length as usize))
                .collect(),
            duplicates: k.duplicates_allowed,
        };
        (key_of(&def.keys.primary), def.keys.alternates.iter().map(key_of).collect())
    }

    /// From a `.cidx` definition: its FD text and its keys.
    pub fn from_definition(table: &str, def: &IndexedDefinition) -> Result<Schema, String> {
        let text = crate::indexed_ide::definition_text_of(def);
        let (primary, alternates) = Schema::keys_of_definition(def);
        Schema::from_fd_text(table, &text, primary, alternates)
    }

    fn from_layout_with(
        table: &str,
        layout: &RecordLayout,
        in_table: &HashSet<(String, Vec<String>)>,
        primary: KeyInput,
        alternates: Vec<KeyInput>,
    ) -> Schema {
        let mut fields: Vec<Field> = Vec::new();
        for f in &layout.fields {
            if f.is_group {
                continue;
            }
            match fields.iter_mut().find(|x| x.name == f.name && x.quals == f.quals) {
                Some(x) => x.occ.push(f.clone()),
                None => fields.push(Field {
                    name: f.name.clone(),
                    quals: f.quals.clone(),
                    class: match (f.class, f.numeric) {
                        (Some(c), _) => Class::Char(c),
                        (None, true) => Class::Number { decimals: f.decimals, signed: f.signed || f.sign_separate },
                        (None, false) => Class::Text,
                    },
                    occ: vec![f.clone()],
                    in_occurs_group: in_table.contains(&(f.name.clone(), f.quals.clone())),
                }),
            }
        }
        let groups = layout
            .groups
            .iter()
            .map(|g| {
                let mut chain = vec![g.name.clone()];
                chain.extend(g.quals.iter().cloned());
                Group {
                    name: g.name.clone(),
                    quals: g.quals.clone(),
                    fields: fields.iter().filter(|f| f.quals.ends_with(&chain)).count(),
                }
            })
            .collect();
        let make_key = |k: KeyInput, is_alt: bool| -> Key {
            let parts: Vec<KeyPart> = k
                .parts
                .iter()
                .map(|(name, offset, len)| {
                    let up = name.to_ascii_uppercase();
                    let field = fields
                        .iter()
                        .position(|f| f.name == up && f.occ[0].offset == *offset && f.occ[0].len == *len);
                    KeyPart { field_name: up, offset: *offset, len: *len, field }
                })
                .collect();
            let name = if is_alt {
                k.name
                    .filter(|n| !n.trim().is_empty())
                    .map(|n| n.to_ascii_uppercase())
                    .or_else(|| parts.first().map(|p| p.field_name.clone()))
            } else {
                None
            };
            Key { name, parts, duplicates: k.duplicates }
        };
        let primary = make_key(primary, false);
        let alternates = alternates.into_iter().map(|k| make_key(k, true)).collect();
        Schema { table: table.to_ascii_uppercase(), record_len: layout.len, fields, groups, primary, alternates }
    }

    /// The key of reference a table reference names: `None` → the RECORD KEY
    /// (0), an index name → that alternate (1 + its position).
    pub fn key_of_reference(&self, index: Option<&str>) -> Option<usize> {
        match index {
            None => Some(0),
            Some(n) => self
                .alternates
                .iter()
                .position(|k| k.name.as_deref().is_some_and(|x| x.eq_ignore_ascii_case(n)))
                .map(|i| i + 1),
        }
    }

    /// The key `kor` names: 0 is the RECORD KEY.
    pub fn key(&self, kor: usize) -> &Key {
        if kor == 0 {
            &self.primary
        } else {
            &self.alternates[kor - 1]
        }
    }
}

/// Parse the FD text of a file into its first record description.
pub fn parse_fd_text(text: &str) -> Result<DataDecl, String> {
    use cobolt_ast::program::DataSection;
    let source = format!(
        "IDENTIFICATION DIVISION.\nPROGRAM-ID. DEFINITION.\nENVIRONMENT DIVISION.\n\
         INPUT-OUTPUT SECTION.\nFILE-CONTROL.\n    SELECT F ASSIGN TO \"x\"\n\
         \x20       ORGANIZATION IS INDEXED RECORD KEY IS K.\nDATA DIVISION.\nFILE SECTION.\n\
         {text}\nPROCEDURE DIVISION.\n    STOP RUN.\n"
    );
    let parsed = cobolt_parser::parse(cobolt_lexer::tokenize(&source, cobolt_lexer::SourceFormat::Free));
    if let Some(d) = parsed.diagnostics.iter().find(|d| d.is_error()) {
        return Err(format!("the record description does not parse: {}", d.message));
    }
    parsed
        .program
        .and_then(|p| p.data)
        .and_then(|d| {
            d.sections.into_iter().find_map(|section| match section {
                DataSection::FileSection(fds) => fds.into_iter().next().and_then(|fd| fd.records.into_iter().next()),
                _ => None,
            })
        })
        .ok_or_else(|| "the file carries no record description".to_string())
}

/// Elementary items that sit under a group with `OCCURS`, as `(name, quals)`
/// the way a layout names them.
fn collect_table_members(
    children: &[DataDecl],
    path: &mut Vec<String>,
    under_table: bool,
    out: &mut HashSet<(String, Vec<String>)>,
) {
    for d in children {
        if d.level == 88 {
            continue;
        }
        if d.children.iter().any(|c| c.level != 88) {
            let pushed = d.name.as_ref().map(|n| path.push(n.to_ascii_uppercase())).is_some();
            collect_table_members(&d.children, path, under_table || d.occurs.is_some(), out);
            if pushed {
                path.pop();
            }
        } else if under_table {
            if let Some(n) = &d.name {
                out.insert((n.to_ascii_uppercase(), path.iter().rev().cloned().collect()));
            }
        }
    }
}
