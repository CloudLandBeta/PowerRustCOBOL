// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! **Definition for AI** over MCP: `describe`, `set_definition` and the Check warning.
//!
//! A definition is text a developer (or an agent) writes about a form, a control, a layer, an
//! animation, a data binding, an indexed file or one of its fields, as if talking to someone who does
//! not know the application: what it is for, in which unit or format, when it applies. `describe`
//! hands an external agent the whole picture of a project or one file, definitions included, in one
//! call; `set_definition` writes one back; Check says, once per file, which elements have none.

use serde_json::{json, Value};

use cobolt_forms::{Control, Form};

use crate::root::ProjectRoot;
use crate::tools::check::Finding;
use crate::validate_source::Severity;

/// A definition shorter than this many characters says too little to be one ("Discount").
pub const MIN_DEFINITION_CHARS: usize = 20;

/// How many ids a Check warning names before it says "and N more".
const NAMED_IN_WARNING: usize = 8;

fn walk<'a>(controls: &'a [Control], out: &mut Vec<&'a Control>) {
    for c in controls {
        out.push(c);
        walk(&c.children, out);
    }
}

fn controls_of(form: &Form) -> Vec<&Control> {
    let mut out = Vec::new();
    walk(&form.controls, &mut out);
    out
}

fn walk_mut<'a>(controls: &'a mut [Control], id: &str) -> Option<&'a mut Control> {
    for c in controls {
        if c.id.eq_ignore_ascii_case(id) {
            return Some(c);
        }
        if let Some(found) = walk_mut(&mut c.children, id) {
            return Some(found);
        }
    }
    None
}

/// A control's definition: the property `Definition`.
pub fn control_definition(c: &Control) -> String {
    c.get_prop("Definition").map(|v| v.as_str().to_owned()).unwrap_or_default()
}

/// Says too little, or nothing.
pub fn is_thin(text: &str) -> bool {
    text.trim().chars().count() < MIN_DEFINITION_CHARS
}

fn kind_of_form(rel: &str) -> &'static str {
    match std::path::Path::new(rel).extension().and_then(|e| e.to_str()).unwrap_or("").to_ascii_lowercase().as_str() {
        "wcfrm" => "web",
        "mcfrm" => "mobile",
        _ => "desktop",
    }
}

/// Every element of `form` that carries a definition, as `(kind, name)` pairs with the text.
fn elements(form: &Form) -> Vec<(&'static str, String, String)> {
    let mut v = vec![("form", form.name.clone(), form.definition.clone())];
    for l in &form.layers {
        v.push(("layer", l.name.clone(), l.definition.clone()));
    }
    for c in controls_of(form) {
        v.push(("control", c.id.clone(), control_definition(c)));
        for a in &c.animations {
            v.push(("animation", format!("{}::{}", c.id, a.name), a.definition.clone()));
        }
    }
    for a in &form.animations {
        v.push(("animation", format!("FORM::{}", a.name), a.definition.clone()));
    }
    for b in &form.data_bindings {
        v.push(("binding", b.id.clone(), b.definition.clone()));
    }
    v
}

/// The Check warning for one form: which elements have no definition or a thin one. One finding per
/// file, never one per control, so a large form does not drown the report.
pub fn missing_finding(form: &Form, rel: &str) -> Option<Finding> {
    let thin: Vec<String> = elements(form)
        .into_iter()
        .filter(|(_, _, text)| is_thin(text))
        .map(|(kind, name, _)| if kind == "control" { name } else { format!("{kind} {name}") })
        .collect();
    if thin.is_empty() {
        return None;
    }
    let shown: Vec<&str> = thin.iter().take(NAMED_IN_WARNING).map(String::as_str).collect();
    let more = thin.len().saturating_sub(NAMED_IN_WARNING);
    let mut f = Finding::at(
        rel,
        Severity::Warning,
        format!(
            "{} element(s) have no \"Definition for AI\", or one under {} characters: {}{} — write each one as if to someone who does not know the application (`describe` shows them, `set_definition` writes one)",
            thin.len(),
            MIN_DEFINITION_CHARS,
            shown.join(", "),
            if more > 0 { format!(" and {more} more") } else { String::new() },
        ),
    );
    f.form = Some(form.name.clone());
    Some(f)
}

// ── describe ────────────────────────────────────────────────────────────────

pub fn describe(root: &ProjectRoot, path: Option<&str>) -> Result<Value, String> {
    match path {
        None => describe_project(root),
        Some(rel) => describe_file(root, rel),
    }
}

fn describe_project(root: &ProjectRoot) -> Result<Value, String> {
    let view = cobolt_compiler::project_manifest_view(root.manifest())?;
    let mut forms = Vec::new();
    for rel in &view.forms {
        let Ok(abs) = root.resolve(rel) else { continue };
        let Ok(form) = cobolt_forms::load_form(&abs) else {
            forms.push(json!({ "path": rel, "error": "the form cannot be loaded" }));
            continue;
        };
        let all = elements(&form);
        forms.push(json!({
            "path": rel,
            "name": form.name,
            "title": form.title,
            "kind": kind_of_form(rel),
            "main_form": form.main_form,
            "definition": form.definition,
            "controls": controls_of(&form).len(),
            "layers": form.layers.len(),
            "elements": all.len(),
            "without_definition": all.iter().filter(|(_, _, t)| is_thin(t)).count(),
        }));
    }
    let mut indexed = Vec::new();
    for rel in &view.indexed {
        let Ok(abs) = root.resolve(rel) else { continue };
        match cobolt_indexed::load_indexed(&abs) {
            Ok(def) => {
                let flat = cobolt_indexed::flatten_record(&def);
                indexed.push(json!({
                    "path": rel,
                    "name": def.name,
                    "definition": def.comment,
                    "fields": flat.len(),
                    "without_definition": flat.iter().filter(|e| is_thin(&e.field.comment)).count() + usize::from(is_thin(&def.comment)),
                }));
            }
            Err(_) => indexed.push(json!({ "path": rel, "error": "the definition cannot be loaded" })),
        }
    }
    let sql: Vec<Value> = view
        .sql_connections
        .iter()
        .map(|c| json!({ "name": c.name, "backend": c.backend, "default": c.default, "definition": c.definition }))
        .collect();
    Ok(json!({
        "project": view.name,
        "kind": view.kind.as_str(),
        "min_definition_chars": MIN_DEFINITION_CHARS,
        "forms": forms,
        "indexed": indexed,
        "sql_connections": sql,
        "hint": "describe with a 'path' shows one file in full; set_definition writes a definition",
    }))
}

fn describe_file(root: &ProjectRoot, rel: &str) -> Result<Value, String> {
    let abs = root.resolve(rel)?;
    if !abs.is_file() {
        return Err(format!("'{rel}' does not exist"));
    }
    let ext = abs.extension().and_then(|e| e.to_str()).unwrap_or("").to_ascii_lowercase();
    if cobolt_compiler::project_kind::is_form_extension(&ext) {
        let form = cobolt_forms::load_form(&abs).map_err(|e| format!("the form cannot be loaded: {e}"))?;
        return Ok(describe_form(rel, &form));
    }
    if ext == "cidx" {
        let def = cobolt_indexed::load_indexed(&abs).map_err(|e| format!("the definition cannot be loaded: {e}"))?;
        return Ok(describe_indexed(rel, &def));
    }
    Err(format!("'{rel}' is not a form or an indexed definition"))
}

fn describe_form(rel: &str, form: &Form) -> Value {
    let controls: Vec<Value> = controls_of(form)
        .into_iter()
        .map(|c| {
            json!({
                "id": c.id,
                "type": c.control_type.as_str(),
                "visual": !c.control_type.is_non_visual(),
                "parent": c.parent,
                "layer": c.layer,
                "definition": control_definition(c),
                "events": c.events.iter().map(|e| e.event.clone()).collect::<Vec<_>>(),
                "animations": c.animations.iter().map(|a| json!({ "name": a.name, "definition": a.definition })).collect::<Vec<_>>(),
            })
        })
        .collect();
    json!({
        "path": rel,
        "form": {
            "name": form.name,
            "title": form.title,
            "kind": kind_of_form(rel),
            "main_form": form.main_form,
            "definition": form.definition,
        },
        "layers": form.layers.iter().map(|l| json!({ "name": l.name, "definition": l.definition })).collect::<Vec<_>>(),
        "controls": controls,
        "form_animations": form.animations.iter().map(|a| json!({ "name": a.name, "definition": a.definition })).collect::<Vec<_>>(),
        "data_bindings": form.data_bindings.iter().map(|b| json!({ "id": b.id, "name": b.display_name, "definition": b.definition })).collect::<Vec<_>>(),
        "min_definition_chars": MIN_DEFINITION_CHARS,
        "without_definition": elements(form).iter().filter(|(_, _, t)| is_thin(t)).count(),
    })
}

fn describe_indexed(rel: &str, def: &cobolt_indexed::IndexedDefinition) -> Value {
    let in_key = |name: &str| {
        let named = |k: &cobolt_indexed::KeyDef| k.name.as_deref() == Some(name) || k.parts.iter().any(|p| p.field_name == name);
        named(&def.keys.primary) || def.keys.alternates.iter().any(named)
    };
    let fields: Vec<Value> = cobolt_indexed::flatten_record(def)
        .iter()
        .map(|e| {
            json!({
                "name": e.field.name,
                "level": e.field.level,
                "group": e.field.is_group(),
                "pic": e.field.pic,
                "key": in_key(&e.field.name),
                "definition": e.field.comment,
            })
        })
        .collect();
    json!({
        "path": rel,
        "file": { "name": def.name, "assign": def.assign_path, "finalized": def.finalized, "definition": def.comment },
        "fields": fields,
        "min_definition_chars": MIN_DEFINITION_CHARS,
    })
}

// ── set_definition ──────────────────────────────────────────────────────────

/// Write one definition. `element` is `form`, `control`, `layer`, `animation`, `binding`, `indexed-file`
/// or `indexed-field`; `id` names it (a control's id, a layer's name, an animation as `CONTROL::name`
/// or `FORM::name`, a binding's id, a field's name). An empty `text` clears it. The caller holds the
/// project's write lock.
pub fn set_definition(root: &ProjectRoot, path: &str, element: &str, id: Option<&str>, text: &str) -> Result<Value, String> {
    if element == "sql-connection" {
        return set_sql_definition(root, id, text);
    }
    let abs = root.resolve(path)?;
    if !abs.is_file() {
        return Err(format!("'{path}' does not exist"));
    }
    let ext = abs.extension().and_then(|e| e.to_str()).unwrap_or("").to_ascii_lowercase();
    let text = text.trim_end().to_owned();
    let need = |what: &str| id.filter(|s| !s.trim().is_empty()).ok_or_else(|| format!("set_definition of a {what} needs its 'id'"));
    if cobolt_compiler::project_kind::is_form_extension(&ext) {
        let mut form = cobolt_forms::load_form(&abs).map_err(|e| format!("the form cannot be loaded: {e}"))?;
        match element {
            "form" => form.definition = text.clone(),
            "control" => {
                let id = need("control")?;
                let c = walk_mut(&mut form.controls, id).ok_or_else(|| format!("the form has no control '{id}'"))?;
                if text.is_empty() {
                    c.properties.shift_remove("Definition");
                } else {
                    c.set_prop("Definition", cobolt_forms::PropValue::String(text.clone()));
                }
            }
            "layer" => {
                let id = need("layer")?;
                let l = form.layers.iter_mut().find(|l| l.name.eq_ignore_ascii_case(id)).ok_or_else(|| format!("the form has no layer '{id}'"))?;
                l.definition = text.clone();
            }
            "animation" => {
                let id = need("animation")?;
                let (owner, name) = id.split_once("::").ok_or("an animation is named CONTROL::name, or FORM::name for the form's own")?;
                let anims = if owner.eq_ignore_ascii_case("FORM") {
                    &mut form.animations
                } else {
                    &mut walk_mut(&mut form.controls, owner).ok_or_else(|| format!("the form has no control '{owner}'"))?.animations
                };
                let a = anims.iter_mut().find(|a| a.name.eq_ignore_ascii_case(name)).ok_or_else(|| format!("'{owner}' has no animation '{name}'"))?;
                a.definition = text.clone();
            }
            "binding" => {
                let id = need("binding")?;
                let b = form.data_bindings.iter_mut().find(|b| b.id.eq_ignore_ascii_case(id)).ok_or_else(|| format!("the form has no data binding '{id}'"))?;
                b.definition = text.clone();
            }
            other => return Err(format!("'{other}' is not an element of a form (form, control, layer, animation, binding)")),
        }
        cobolt_forms::save_form(&form, &abs).map_err(|e| format!("the form cannot be saved: {e}"))?;
        return Ok(json!({ "path": path, "element": element, "id": id, "definition": text, "thin": is_thin(&text) }));
    }
    if ext == "cidx" {
        let mut def = cobolt_indexed::load_indexed(&abs).map_err(|e| format!("the definition cannot be loaded: {e}"))?;
        match element {
            "indexed-file" => def.comment = text.clone(),
            "indexed-field" => {
                let id = need("field")?;
                let f = find_field_mut(&mut def.fields, id).ok_or_else(|| format!("the record has no field '{id}'"))?;
                f.comment = text.clone();
            }
            other => return Err(format!("'{other}' is not an element of an indexed file (indexed-file, indexed-field)")),
        }
        cobolt_indexed::save_indexed(&abs, &def).map_err(|e| format!("the definition cannot be saved: {e}"))?;
        return Ok(json!({ "path": path, "element": element, "id": id, "definition": text, "thin": is_thin(&text) }));
    }
    Err(format!("'{path}' is not a form or an indexed definition"))
}

/// A SQL connection's definition lives in the project file's `[[sql-connections]]`. The edit goes through
/// `toml_edit`, so every other byte of the developer's project file is written back as it was.
fn set_sql_definition(root: &ProjectRoot, id: Option<&str>, text: &str) -> Result<Value, String> {
    use toml_edit::{value, DocumentMut};
    let name = id.filter(|s| !s.trim().is_empty()).ok_or("set_definition of a sql-connection needs its 'id' (the connection name)")?;
    let text = text.trim_end().to_owned();
    let manifest = root.manifest();
    let source = std::fs::read_to_string(manifest).map_err(|e| format!("the project file cannot be read ({e})"))?;
    let mut doc: DocumentMut = source.parse().map_err(|e| format!("the project file cannot be parsed ({e})"))?;
    let entry = doc
        .get_mut("sql-connections")
        .and_then(|i| i.as_array_of_tables_mut())
        .and_then(|a| a.iter_mut().find(|t| t.get("name").and_then(|n| n.as_str()).is_some_and(|n| n.eq_ignore_ascii_case(name))))
        .ok_or_else(|| format!("the project has no SQL connection '{name}'"))?;
    if text.is_empty() {
        entry.remove("definition");
    } else {
        entry["definition"] = value(text.clone());
    }
    let written = doc.to_string();
    // `toml_edit` hands the text back with `LF` line ends. A project file that is `CRLF` all through (the usual one on
    // Windows) is written back as it was; one that mixes the two keeps what `toml_edit` gives.
    let all_crlf = source.contains("\r\n") && !source.replace("\r\n", "").contains('\n');
    let written = if all_crlf { written.replace("\r\n", "\n").replace('\n', "\r\n") } else { written };
    std::fs::write(manifest, written).map_err(|e| format!("the project file cannot be written ({e})"))?;
    Ok(json!({ "element": "sql-connection", "id": name, "definition": text, "thin": is_thin(&text) }))
}

fn find_field_mut<'a>(fields: &'a mut [cobolt_indexed::IndexedField], id: &str) -> Option<&'a mut cobolt_indexed::IndexedField> {
    for f in fields {
        if f.name.eq_ignore_ascii_case(id) {
            return Some(f);
        }
        if let Some(found) = find_field_mut(&mut f.children, id) {
            return Some(found);
        }
    }
    None
}
