// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! `edit_indexed` — create or change an indexed definition (`.cidx`) as the
//! IDE's Indexed File Editor does (spec 084 R58): the record written as its
//! COBOL text view writes it, the keys named by field, the file's
//! properties. The layout (offsets, lengths, record length) is the editor's
//! own computation, a finalized definition keeps its schema unless the
//! developer agreed to change it, and the facade and copybooks are
//! regenerated with it.

use std::path::{Component, Path};

use cobolt_indexed::{IndexedDefinition, IndexedField, KeyDef, KeyEncodingDef, KeyOrderingDef, KeyPartDef};
use cobolt_mcp::Tool;
use serde_json::{json, Value};

use crate::host::ProjectHost;
use crate::root::ProjectRoot;

pub fn tool() -> Tool {
    Tool {
        name: "edit_indexed".into(),
        description: Some(
            "Create or change an indexed definition (.cidx) as the Indexed File Editor does. `record` is the record \
             description as the editor's text view shows it (01/05/10 levels, PIC, USAGE, OCCURS, REDEFINES, one \
             item per line); `primary_key` and `alternate_keys` name elementary fields. Give only what changes: \
             absent parts are kept, and the keys follow the fields when the record moves them. A finalized \
             definition keeps its schema: a change to it is refused unless the developer agreed and \
             `confirm_recreate` is true. Regenerates the facade and copybooks; creates an empty data file when \
             there is none. Changes files."
                .into(),
        ),
        input_schema: json!({
            "type": "object",
            "properties": {
                "path": { "type": "string", "description": "The definition, project-relative, e.g. indexed/CUSTOMER.cidx." },
                "record": { "type": "string", "description": "The record description, as COBOL text." },
                "primary_key": { "type": "string", "description": "The primary key's field." },
                "alternate_keys": { "type": "array", "items": { "type": "object" },
                                    "description": "[{field, duplicates?}] — replaces every alternate key; [] removes them." },
                "name": { "type": "string", "description": "The file's COBOL name (a new definition: the file's stem)." },
                "assign_path": { "type": "string", "description": "The data file, project-relative (a new definition: data/<name>.dat)." },
                "comment": { "type": "string", "description": "What the file holds." },
                "access_mode": { "type": "string", "description": "sequential, random or dynamic." },
                "storage": { "type": "string", "description": "disk or memory." },
                "finalized": { "type": "boolean", "description": "Lock the schema, as the editor's Finalized box does." },
                "confirm_recreate": { "type": "boolean", "description": "Only when the developer agreed to change a finalized schema." }
            },
            "required": ["path"],
            "additionalProperties": false
        }),
        annotations: None,
    }
}

fn s<'a>(args: &'a Value, key: &str) -> Option<&'a str> {
    args.get(key).and_then(Value::as_str)
}

/// A key part for the elementary field `name`, where the layout puts it now.
fn part(leaves: &[&IndexedField], name: &str) -> Result<KeyPartDef, String> {
    let field = leaves
        .iter()
        .find(|f| f.name.eq_ignore_ascii_case(name))
        .ok_or_else(|| format!("the record has no elementary field '{name}' for a key"))?;
    let (Some(offset), Some(length)) = (field.offset, field.length) else {
        return Err(format!("the key field '{name}' must be an elementary item"));
    };
    Ok(KeyPartDef { field_name: field.name.clone(), offset, length, encoding: KeyEncodingDef::Bytes })
}

/// `key` again, each part where its field is now.
fn follow(leaves: &[&IndexedField], key: &KeyDef) -> Result<KeyDef, String> {
    let parts = key
        .parts
        .iter()
        .map(|p| part(leaves, &p.field_name).map(|n| KeyPartDef { encoding: p.encoding, ..n }))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(KeyDef { parts, ..key.clone() })
}

/// What a finalized definition locks: everything but its comment, its
/// description for AI and the box itself.
fn schema_changed(a: &IndexedDefinition, b: &IndexedDefinition) -> bool {
    a.name != b.name
        || a.assign_path != b.assign_path
        || a.access_mode != b.access_mode
        || a.record_format != b.record_format
        || a.storage != b.storage
        || a.compression != b.compression
        || a.persistence != b.persistence
        || a.keys != b.keys
        || a.fields != b.fields
}

fn key_json(k: &KeyDef) -> Value {
    json!({
        "fields": k.parts.iter().map(|p| p.field_name.clone()).collect::<Vec<_>>(),
        "duplicates": k.duplicates_allowed,
    })
}

pub fn run<H: ProjectHost>(host: &mut H, root: &ProjectRoot, args: &Value) -> Result<Value, String> {
    let rel = s(args, "path").map(str::trim).filter(|p| !p.is_empty()).ok_or("edit_indexed needs a 'path'")?.replace('\\', "/");
    let abs = root.resolve(&rel)?;
    if !abs.extension().and_then(|e| e.to_str()).is_some_and(|e| e.eq_ignore_ascii_case("cidx")) {
        return Err(format!("'{rel}' is not an indexed definition (.cidx)"));
    }
    let existing = if abs.is_file() {
        Some(cobolt_indexed::load_indexed(&abs).map_err(|e| format!("'{rel}' cannot be loaded: {e}"))?)
    } else {
        None
    };
    let stem = abs.file_stem().and_then(|s| s.to_str()).unwrap_or("FILE").to_owned();
    let mut def = existing.clone().unwrap_or_else(|| IndexedDefinition::new(stem.clone(), format!("data/{stem}.dat")));
    if existing.is_none() {
        def.finalized = false;
        if s(args, "record").is_none() || s(args, "primary_key").is_none() {
            return Err(format!("'{rel}' is new: give its 'record' and its 'primary_key'"));
        }
    }

    if let Some(name) = s(args, "name").map(str::trim) {
        let ok = name.chars().next().is_some_and(|c| c.is_ascii_alphabetic())
            && name.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_'));
        if !ok {
            return Err(format!("'{name}' is not a file name: a letter, then letters, digits, hyphens or underscores"));
        }
        def.name = name.to_owned();
    }
    if let Some(assign) = s(args, "assign_path").map(str::trim) {
        let p = Path::new(assign);
        if assign.is_empty() || p.is_absolute() || p.components().any(|c| matches!(c, Component::ParentDir | Component::RootDir | Component::Prefix(_))) {
            return Err(format!("'{assign}' is not a data file path inside the project"));
        }
        def.assign_path = assign.replace('\\', "/");
    }
    if let Some(comment) = s(args, "comment") {
        def.comment = comment.to_owned();
    }
    if let Some(mode) = s(args, "access_mode") {
        def.access_mode = cobolt_indexed::AccessMode::from_str(mode);
    }
    if let Some(storage) = s(args, "storage") {
        def.storage = cobolt_indexed::StorageMode::from_str(storage);
    }
    if let Some(on) = args.get("finalized").and_then(Value::as_bool) {
        def.finalized = on;
    }
    if let Some(record) = s(args, "record") {
        cobolt_indexed::text_to_record(&mut def, record).map_err(|e| format!("the record does not read as COBOL: {e}"))?;
    }

    // The keys: as given, else the ones there were, where their fields are now.
    let leaves = def.record_root().map(IndexedField::all_leaves).unwrap_or_default();
    let primary = match s(args, "primary_key") {
        Some(name) => KeyDef { name: None, parts: vec![part(&leaves, name)?], duplicates_allowed: false, ordering: KeyOrderingDef::Ascending },
        None => follow(&leaves, &def.keys.primary)?,
    };
    let alternates = match args.get("alternate_keys") {
        Some(list) => list
            .as_array()
            .ok_or("'alternate_keys' must be a list of {field, duplicates}")?
            .iter()
            .map(|k| {
                let field = s(k, "field").ok_or("each alternate key needs a 'field'")?;
                let p = part(&leaves, field)?;
                Ok(KeyDef {
                    name: Some(p.field_name.clone()),
                    parts: vec![p],
                    duplicates_allowed: k.get("duplicates").and_then(Value::as_bool).unwrap_or(false),
                    ordering: KeyOrderingDef::Ascending,
                })
            })
            .collect::<Result<Vec<_>, String>>()?,
        None => def.keys.alternates.iter().map(|k| follow(&leaves, k)).collect::<Result<Vec<_>, _>>()?,
    };
    drop(leaves);
    def.keys.primary = primary;
    def.keys.alternates = alternates;

    if let Some(before) = existing.as_ref().filter(|d| d.finalized) {
        let confirmed = args.get("confirm_recreate").and_then(Value::as_bool).unwrap_or(false);
        if schema_changed(before, &def) && !confirmed {
            return Err(format!(
                "'{rel}' is finalized: its schema is locked, and the records already stored follow it. Ask the \
                 developer; only when they agree, repeat this call with confirm_recreate: true"
            ));
        }
    }
    cobolt_indexed::validate_definition(&def).map_err(|e| format!("the definition is not valid: {e}"))?;

    let xml = cobolt_indexed::save_indexed_to_string(&def).map_err(|e| e.to_string())?;
    let (_, recorded) = super::files::write_checked(host, root, &rel, xml.as_bytes())?;
    let regenerated = super::regenerate::run(host, root, Some(&rel))?;

    let data = cobolt_indexed::resolve_path(root.dir(), &def.assign_path);
    let data_note = if data.exists() {
        match existing.as_ref() {
            Some(before) if before.fields != def.fields || before.keys != def.keys => {
                "kept as it was: its records follow the old layout until the developer rebuilds it"
            }
            _ => "kept",
        }
    } else {
        if let Some(parent) = data.parent() {
            std::fs::create_dir_all(parent).map_err(|e| format!("the data folder cannot be made ({e})"))?;
        }
        cobolt_runtime::indexed_ide::create_empty_from_definition(&def, &data)
            .map_err(|e| format!("the empty data file cannot be created ({e})"))?;
        host.written(std::slice::from_ref(&data));
        "created empty"
    };
    Ok(json!({
        "path": rel,
        "created": existing.is_none(),
        "recorded_in": recorded,
        "record": cobolt_indexed::record_to_text(&def),
        "record_length": def.record_length(),
        "primary_key": key_json(&def.keys.primary),
        "alternate_keys": def.keys.alternates.iter().map(key_json).collect::<Vec<_>>(),
        "finalized": def.finalized,
        "warnings": cobolt_indexed::finalize_warnings(&def),
        "regenerated": regenerated.get("written").cloned().unwrap_or(Value::Null),
        "data_file": { "path": def.assign_path, "state": data_note },
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::host::{FileList, HeadlessHost};

    fn project(name: &str) -> (std::path::PathBuf, ProjectRoot, HeadlessHost) {
        let dir = std::env::temp_dir().join(format!("prc-084d-cidx-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("forms")).unwrap();
        let mut form = cobolt_forms::Form::new("MAIN", "MAIN", 300, 200);
        form.main_form = true;
        cobolt_forms::save_form(&form, &dir.join("forms/MAIN.cfrm")).unwrap();
        std::fs::write(
            dir.join("Demo.project.toml"),
            "[project]\nname = \"Demo\"\nversion = \"1.0.0\"\n\n[files]\nforms = [\"forms/MAIN.cfrm\"]\nsources = []\n",
        )
        .unwrap();
        crate::host::record_in_manifest(&dir.join("Demo.project.toml"), "forms/MAIN.cfrm", FileList::Forms).unwrap();
        let root = ProjectRoot::open(&dir).unwrap();
        let host = HeadlessHost::new(dir.join("Demo.project.toml"), "test");
        (dir, root, host)
    }

    const RECORD: &str = "01 CUST-REC.\n   05 CUST-ID PIC 9(6).\n   05 CUST-NAME PIC X(30).\n";

    /// Spec 084 R58 / AC26: a definition made field by field loads, keeps its
    /// keys on their fields when the record moves them, regenerates its
    /// facade and gets an empty data file; a finalized one is guarded.
    #[test]
    fn an_indexed_definition_is_made_and_changed_field_by_field() {
        let (dir, root, mut host) = project("made");
        let err = run(&mut host, &root, &json!({ "path": "indexed/CUST.cidx", "record": RECORD })).unwrap_err();
        assert!(err.contains("primary_key"), "a new one needs its key: {err}");

        let v = run(&mut host, &root, &json!({ "path": "indexed/CUST.cidx", "record": RECORD, "primary_key": "CUST-ID",
            "alternate_keys": [{ "field": "CUST-NAME", "duplicates": true }] })).unwrap();
        assert_eq!(v["created"], true);
        assert_eq!(v["record_length"], 36);
        assert_eq!(v["data_file"]["state"], "created empty");
        assert!(dir.join("data/CUST.dat").exists());
        assert!(v["regenerated"].as_array().is_some_and(|w| !w.is_empty()), "the facade is written: {v}");
        assert!(std::fs::read_to_string(dir.join("Demo.project.toml")).unwrap().contains("indexed/CUST.cidx"));

        // A field before the key moves it; the keys follow.
        let moved = "01 CUST-REC.\n   05 CUST-BRANCH PIC X(4).\n   05 CUST-ID PIC 9(6).\n   05 CUST-NAME PIC X(30).\n";
        let v = run(&mut host, &root, &json!({ "path": "indexed/CUST.cidx", "record": moved })).unwrap();
        assert_eq!(v["record_length"], 40);
        let def = cobolt_indexed::load_indexed(dir.join("indexed/CUST.cidx")).unwrap();
        assert_eq!((def.keys.primary.parts[0].offset, def.keys.primary.parts[0].length), (4, 6));
        assert_eq!(def.keys.alternates[0].parts[0].offset, 10);
        assert!(def.keys.alternates[0].duplicates_allowed);

        // A key on a field that is gone is refused, and nothing is written.
        let err = run(&mut host, &root, &json!({ "path": "indexed/CUST.cidx", "record": RECORD, "primary_key": "NOPE" })).unwrap_err();
        assert!(err.contains("NOPE"));
        assert_eq!(cobolt_indexed::load_indexed(dir.join("indexed/CUST.cidx")).unwrap().record_length(), 40);

        // Finalized: the schema holds unless the developer agreed.
        run(&mut host, &root, &json!({ "path": "indexed/CUST.cidx", "finalized": true })).unwrap();
        let err = run(&mut host, &root, &json!({ "path": "indexed/CUST.cidx", "record": RECORD })).unwrap_err();
        assert!(err.contains("finalized") && err.contains("confirm_recreate"), "{err}");
        run(&mut host, &root, &json!({ "path": "indexed/CUST.cidx", "comment": "Customers" })).expect("a comment is not schema");
        let v = run(&mut host, &root, &json!({ "path": "indexed/CUST.cidx", "record": RECORD, "confirm_recreate": true })).unwrap();
        assert_eq!(v["record_length"], 36);
        assert!(v["data_file"]["state"].as_str().unwrap().starts_with("kept as it was"), "the data is never deleted");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
