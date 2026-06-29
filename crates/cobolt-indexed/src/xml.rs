// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! XML serialization for `.cidx` files.

use std::fs;
use std::io::BufReader;
use std::path::Path;

use cobolt_forms::ControlType;
use quick_xml::{
    events::{BytesCData, BytesDecl, BytesEnd, BytesStart, BytesText, Event},
    Reader, Writer,
};
use thiserror::Error;

use crate::model::{
    AccessMode, FieldUsage, IndexedDefinition, IndexedField, KeyDef, KeyEncodingDef,
    KeyOrderingDef, KeyPartDef, RecordFormatDef, StorageMode,
};

#[derive(Debug, Error)]
pub enum IndexedError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("XML error: {0}")]
    Xml(String),
    #[error("Missing required element: <{0}>")]
    MissingElement(String),
}

pub fn load_indexed(path: impl AsRef<Path>) -> Result<IndexedDefinition, IndexedError> {
    let f = fs::File::open(path)?;
    load_indexed_from_reader(BufReader::new(f))
}

pub fn load_indexed_from_str(s: &str) -> Result<IndexedDefinition, IndexedError> {
    load_indexed_from_reader(s.as_bytes())
}

pub fn save_indexed(path: impl AsRef<Path>, def: &IndexedDefinition) -> Result<(), IndexedError> {
    let xml = save_indexed_to_string(def)?;
    fs::write(path, xml)?;
    Ok(())
}

pub fn save_indexed_to_string(def: &IndexedDefinition) -> Result<String, IndexedError> {
    let mut w = Writer::new(Vec::new());
    w.write_event(Event::Decl(BytesDecl::new("1.0", Some("UTF-8"), None)))
        .map_err(|e| IndexedError::Xml(e.to_string()))?;

    let mut root = BytesStart::new("IndexedFile");
    root.push_attribute(("name", def.name.as_str()));
    root.push_attribute(("finalized", if def.finalized { "true" } else { "false" }));
    root.push_attribute(("version", "1.0"));
    w.write_event(Event::Start(root)).map_err(xml_err)?;

    write_text_el(&mut w, "assign-path", &def.assign_path)?;
    write_text_el(&mut w, "access-mode", def.access_mode.as_str())?;

    match def.record_format {
        RecordFormatDef::Fixed { length } => {
            let mut rf = BytesStart::new("record-format");
            rf.push_attribute(("fixed-length", length.to_string().as_str()));
            w.write_event(Event::Empty(rf)).map_err(xml_err)?;
        }
        RecordFormatDef::Variable {
            min_length,
            max_length,
        } => {
            let mut rf = BytesStart::new("record-format");
            rf.push_attribute(("min", min_length.to_string().as_str()));
            rf.push_attribute(("max", max_length.to_string().as_str()));
            w.write_event(Event::Empty(rf)).map_err(xml_err)?;
        }
    }

    let mut st = BytesStart::new("storage");
    st.push_attribute(("mode", def.storage.as_str()));
    st.push_attribute((
        "compression",
        if def.compression { "true" } else { "false" },
    ));
    st.push_attribute((
        "persistence",
        if def.persistence { "true" } else { "false" },
    ));
    w.write_event(Event::Empty(st)).map_err(xml_err)?;

    if !def.comment.is_empty() {
        write_cdata_el(&mut w, "comment", &def.comment)?;
    }

    w.write_event(Event::Start(BytesStart::new("keys")))
        .map_err(xml_err)?;
    write_key(&mut w, "primary", &def.keys.primary, true)?;
    for alt in &def.keys.alternates {
        write_key(&mut w, "alternate", alt, false)?;
    }
    w.write_event(Event::End(BytesEnd::new("keys")))
        .map_err(xml_err)?;

    w.write_event(Event::Start(BytesStart::new("fields")))
        .map_err(xml_err)?;
    for f in &def.fields {
        write_field(&mut w, f)?;
    }
    w.write_event(Event::End(BytesEnd::new("fields")))
        .map_err(xml_err)?;

    w.write_event(Event::End(BytesEnd::new("IndexedFile")))
        .map_err(xml_err)?;
    String::from_utf8(w.into_inner()).map_err(|e| IndexedError::Xml(e.to_string()))
}

fn xml_err(e: quick_xml::Error) -> IndexedError {
    IndexedError::Xml(e.to_string())
}

fn write_text_el(w: &mut Writer<Vec<u8>>, tag: &str, text: &str) -> Result<(), IndexedError> {
    w.write_event(Event::Start(BytesStart::new(tag)))
        .map_err(xml_err)?;
    w.write_event(Event::Text(BytesText::new(text)))
        .map_err(xml_err)?;
    w.write_event(Event::End(BytesEnd::new(tag)))
        .map_err(xml_err)?;
    Ok(())
}

fn write_cdata_el(w: &mut Writer<Vec<u8>>, tag: &str, text: &str) -> Result<(), IndexedError> {
    w.write_event(Event::Start(BytesStart::new(tag)))
        .map_err(xml_err)?;
    w.write_event(Event::CData(BytesCData::new(text)))
        .map_err(xml_err)?;
    w.write_event(Event::End(BytesEnd::new(tag)))
        .map_err(xml_err)?;
    Ok(())
}

fn write_key(
    w: &mut Writer<Vec<u8>>,
    tag: &str,
    key: &KeyDef,
    is_primary: bool,
) -> Result<(), IndexedError> {
    let mut el = BytesStart::new(tag);
    if !is_primary {
        if let Some(n) = &key.name {
            el.push_attribute(("name", n.as_str()));
        }
    }
    el.push_attribute((
        "duplicates",
        if key.duplicates_allowed {
            "true"
        } else {
            "false"
        },
    ));
    el.push_attribute(("ordering", key.ordering.as_str()));
    w.write_event(Event::Start(el)).map_err(xml_err)?;
    for p in &key.parts {
        let mut part = BytesStart::new("part");
        part.push_attribute(("field", p.field_name.as_str()));
        part.push_attribute(("offset", p.offset.to_string().as_str()));
        part.push_attribute(("length", p.length.to_string().as_str()));
        part.push_attribute(("encoding", p.encoding.as_str()));
        w.write_event(Event::Empty(part)).map_err(xml_err)?;
    }
    w.write_event(Event::End(BytesEnd::new(tag)))
        .map_err(xml_err)?;
    Ok(())
}

fn write_field(w: &mut Writer<Vec<u8>>, f: &IndexedField) -> Result<(), IndexedError> {
    let mut el = BytesStart::new("Field");
    el.push_attribute(("level", f.level.to_string().as_str()));
    el.push_attribute(("name", f.name.as_str()));
    if !f.pic.is_empty() {
        el.push_attribute(("pic", f.pic.as_str()));
    }
    if f.offset.is_some() || !f.children.is_empty() {
        el.push_attribute(("usage", f.usage.as_str()));
    }
    if let Some(o) = f.offset {
        el.push_attribute(("offset", o.to_string().as_str()));
    }
    if let Some(l) = f.length {
        el.push_attribute(("length", l.to_string().as_str()));
    }
    if let Some(wid) = &f.grid_control {
        el.push_attribute(("grid-control", wid.as_str()));
    }
    if let Some(n) = f.occurs {
        el.push_attribute(("occurs", n.to_string().as_str()));
    }
    if let Some(r) = &f.redefines {
        el.push_attribute(("redefines", r.as_str()));
    }
    if f.synchronized {
        el.push_attribute(("synchronized", "true"));
    }
    if f.children.is_empty() && f.comment.is_empty() && f.grid_control.is_none() && f.pic.is_empty()
    {
        w.write_event(Event::Empty(el)).map_err(xml_err)?;
        return Ok(());
    }
    w.write_event(Event::Start(el)).map_err(xml_err)?;
    if !f.comment.is_empty() {
        write_cdata_el(w, "comment", &f.comment)?;
    }
    for c in &f.children {
        write_field(w, c)?;
    }
    w.write_event(Event::End(BytesEnd::new("Field")))
        .map_err(xml_err)?;
    Ok(())
}

fn load_indexed_from_reader<R: std::io::BufRead>(
    reader: R,
) -> Result<IndexedDefinition, IndexedError> {
    let mut r = Reader::from_reader(reader);
    r.config_mut().trim_text(true);
    let mut buf = Vec::new();
    let mut def = IndexedDefinition::new("UNNAMED", "data.idx");
    let mut key_stack: Vec<KeyDef> = Vec::new();
    let mut field_path: Vec<usize> = Vec::new();
    let mut text_target: Option<TextTarget> = None;

    loop {
        buf.clear();
        match r.read_event_into(&mut buf) {
            Ok(Event::Start(e)) => {
                let tag = String::from_utf8_lossy(e.local_name().as_ref()).into_owned();
                match tag.as_str() {
                    "IndexedFile" => {
                        def.name = attr(&e, "name").unwrap_or_else(|| "UNNAMED".into());
                        def.finalized = attr(&e, "finalized").as_deref() == Some("true");
                    }
                    "assign-path" => text_target = Some(TextTarget::AssignPath),
                    "access-mode" => text_target = Some(TextTarget::AccessMode),
                    "comment" => {
                        text_target = Some(if field_path.is_empty() {
                            TextTarget::FileComment
                        } else {
                            TextTarget::FieldComment
                        });
                    }
                    "primary" | "alternate" => key_stack.push(parse_key(&e)),
                    "part" => push_part(&mut key_stack, &e),
                    "Field" => {
                        let field = parse_field(&e);
                        push_field(&mut def, &mut field_path, field, true);
                    }
                    _ => {}
                }
            }
            Ok(Event::Empty(e)) => {
                let tag = String::from_utf8_lossy(e.local_name().as_ref()).into_owned();
                match tag.as_str() {
                    "record-format" => def.record_format = parse_record_format(&e),
                    "storage" => {
                        def.storage =
                            StorageMode::from_str(attr(&e, "mode").as_deref().unwrap_or("disk"));
                        def.compression = attr(&e, "compression").as_deref() == Some("true");
                        def.persistence = attr(&e, "persistence").as_deref() == Some("true");
                    }
                    "part" => push_part(&mut key_stack, &e),
                    "Field" => {
                        let field = parse_field(&e);
                        push_field(&mut def, &mut field_path, field, false);
                    }
                    _ => {}
                }
            }
            Ok(Event::End(e)) => {
                let tag = String::from_utf8_lossy(e.local_name().as_ref()).into_owned();
                match tag.as_str() {
                    "primary" => {
                        if let Some(k) = key_stack.pop() {
                            def.keys.primary = k;
                        }
                    }
                    "alternate" => {
                        if let Some(k) = key_stack.pop() {
                            def.keys.alternates.push(k);
                        }
                    }
                    "Field" => {
                        field_path.pop();
                    }
                    "assign-path" | "access-mode" | "comment" => text_target = None,
                    _ => {}
                }
            }
            Ok(Event::Text(t)) => {
                let text = t.unescape().map_err(|e| IndexedError::Xml(e.to_string()))?;
                apply_text(
                    &mut def,
                    &mut field_path,
                    &mut text_target,
                    text.into_owned(),
                );
            }
            Ok(Event::CData(c)) => {
                let text = String::from_utf8_lossy(&c).into_owned();
                apply_text(&mut def, &mut field_path, &mut text_target, text);
            }
            Ok(Event::Eof) => break,
            Err(e) => return Err(IndexedError::Xml(e.to_string())),
            _ => {}
        }
    }
    Ok(def)
}

enum TextTarget {
    AssignPath,
    AccessMode,
    FileComment,
    FieldComment,
}

fn parse_record_format(e: &BytesStart) -> RecordFormatDef {
    if let Some(len) = attr(e, "fixed-length") {
        RecordFormatDef::Fixed {
            length: len.parse().unwrap_or(80),
        }
    } else {
        RecordFormatDef::Variable {
            min_length: attr(e, "min").and_then(|s| s.parse().ok()).unwrap_or(1),
            max_length: attr(e, "max").and_then(|s| s.parse().ok()).unwrap_or(256),
        }
    }
}

fn push_part(key_stack: &mut Vec<KeyDef>, e: &BytesStart) {
    if let Some(k) = key_stack.last_mut() {
        k.parts.push(KeyPartDef {
            field_name: attr(e, "field").unwrap_or_default(),
            offset: attr(e, "offset").and_then(|s| s.parse().ok()).unwrap_or(0),
            length: attr(e, "length").and_then(|s| s.parse().ok()).unwrap_or(0),
            encoding: KeyEncodingDef::from_str(attr(e, "encoding").as_deref().unwrap_or("bytes")),
        });
    }
}

fn field_mut<'a>(def: &'a mut IndexedDefinition, path: &[usize]) -> &'a mut IndexedField {
    let mut f = &mut def.fields[path[0]];
    for &i in &path[1..] {
        f = &mut f.children[i];
    }
    f
}

fn push_field(
    def: &mut IndexedDefinition,
    field_path: &mut Vec<usize>,
    field: IndexedField,
    track_depth: bool,
) {
    if field_path.is_empty() {
        def.fields.push(field);
        if track_depth {
            field_path.push(def.fields.len() - 1);
        }
    } else {
        let parent = field_mut(def, field_path);
        parent.children.push(field);
        if track_depth {
            field_path.push(parent.children.len() - 1);
        }
    }
}

fn apply_text(
    def: &mut IndexedDefinition,
    field_path: &[usize],
    target: &mut Option<TextTarget>,
    text: String,
) {
    match target {
        Some(TextTarget::AssignPath) => def.assign_path = text,
        Some(TextTarget::AccessMode) => def.access_mode = AccessMode::from_str(&text),
        Some(TextTarget::FileComment) => def.comment = text,
        Some(TextTarget::FieldComment) if !field_path.is_empty() => {
            field_mut(def, field_path).comment = text;
        }
        _ => {}
    }
}

fn parse_key(e: &BytesStart) -> KeyDef {
    KeyDef {
        name: attr(e, "name"),
        parts: Vec::new(),
        duplicates_allowed: attr(e, "duplicates").as_deref() == Some("true"),
        ordering: KeyOrderingDef::from_str(attr(e, "ordering").as_deref().unwrap_or("ascending")),
    }
}

fn parse_field(e: &BytesStart) -> IndexedField {
    let grid = attr(e, "grid-control").map(|s| ControlType::from_str(&s));
    IndexedField {
        level: attr(e, "level").and_then(|s| s.parse().ok()).unwrap_or(1),
        name: attr(e, "name").unwrap_or_else(|| "UNNAMED".into()),
        pic: attr(e, "pic").unwrap_or_default(),
        usage: FieldUsage::from_str(attr(e, "usage").as_deref().unwrap_or("display")),
        offset: attr(e, "offset").and_then(|s| s.parse().ok()),
        length: attr(e, "length").and_then(|s| s.parse().ok()),
        comment: String::new(),
        grid_control: grid,
        occurs: attr(e, "occurs").and_then(|s| s.parse().ok()),
        redefines: attr(e, "redefines"),
        synchronized: attr(e, "synchronized").as_deref() == Some("true"),
        children: Vec::new(),
    }
}

fn attr(e: &BytesStart, key: &str) -> Option<String> {
    for a in e.attributes().flatten() {
        if a.key.as_ref() == key.as_bytes() {
            return Some(String::from_utf8_lossy(&a.value).into_owned());
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::*;

    fn sample_def() -> IndexedDefinition {
        let mut def = IndexedDefinition::new("CUSTOMER-FILE", "data/customers.idx");
        def.finalized = true;
        def.access_mode = AccessMode::Dynamic;
        def.record_format = RecordFormatDef::Fixed { length: 38 };
        def.compression = true;
        def.comment = "Customer master".into();
        def.keys.primary = KeyDef {
            name: Some("CUST-ID".into()),
            parts: vec![KeyPartDef {
                field_name: "CUST-ID".into(),
                offset: 0,
                length: 8,
                encoding: KeyEncodingDef::Bytes,
            }],
            duplicates_allowed: false,
            ordering: KeyOrderingDef::Ascending,
        };
        def.fields = vec![IndexedField {
            level: 1,
            name: "CUSTOMER-RECORD".into(),
            pic: String::new(),
            usage: FieldUsage::Display,
            offset: None,
            length: None,
            comment: String::new(),
            grid_control: None,
            occurs: None,
            redefines: None,
            synchronized: false,
            children: vec![
                IndexedField {
                    level: 5,
                    name: "CUST-ID".into(),
                    pic: "9(8)".into(),
                    usage: FieldUsage::Display,
                    offset: Some(0),
                    length: Some(8),
                    comment: "Primary key".into(),
                    grid_control: Some(ControlType::NumericUpDown),
                    occurs: None,
                    redefines: None,
                    synchronized: false,
                    children: Vec::new(),
                },
                IndexedField {
                    level: 5,
                    name: "CUST-NAME".into(),
                    pic: "X(30)".into(),
                    usage: FieldUsage::Display,
                    offset: Some(8),
                    length: Some(30),
                    comment: String::new(),
                    grid_control: Some(ControlType::TextBox),
                    occurs: None,
                    redefines: None,
                    synchronized: false,
                    children: Vec::new(),
                },
            ],
        }];
        def
    }

    #[test]
    fn round_trip_fixed() {
        let def = sample_def();
        let xml = save_indexed_to_string(&def).unwrap();
        let loaded = load_indexed_from_str(&xml).unwrap();
        assert_eq!(loaded.name, def.name);
        assert_eq!(loaded.assign_path, def.assign_path);
        assert_eq!(loaded.finalized, true);
        assert_eq!(loaded.keys.primary.parts.len(), 1);
        assert_eq!(loaded.fields[0].children.len(), 2);
        assert_eq!(loaded.fields[0].children[0].pic, "9(8)");
    }

    #[test]
    fn round_trip_variable() {
        let mut def = sample_def();
        def.record_format = RecordFormatDef::Variable {
            min_length: 10,
            max_length: 200,
        };
        let loaded = load_indexed_from_str(&save_indexed_to_string(&def).unwrap()).unwrap();
        assert!(matches!(
            loaded.record_format,
            RecordFormatDef::Variable {
                min_length: 10,
                max_length: 200
            }
        ));
    }
}
