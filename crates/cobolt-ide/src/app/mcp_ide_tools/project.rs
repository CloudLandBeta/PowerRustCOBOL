// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! The project's tools (spec 084 R56–R61, R64): a form's change set, Project
//! Settings, the SQL connections, save / package / upgrade, the Project
//! Knowledge Base, an indexed file's records and Beautify — each through the
//! code the IDE's own window or menu runs.

use std::path::Path;

use serde_json::{json, Map, Value};

use super::super::{load_form, save_form, CoboltApp, DesignerPanel};
use super::arg_str;
use crate::agent::AgentOp;
use crate::panels::designer::McpStep;

/// Project Settings sections a tool does not touch: the file lists (the file
/// tools and `add_to_project` own them), the crates (`add_crate`) and the
/// User Controls (the designer owns them).
const NOT_SETTINGS: [&str; 3] = ["files", "crates", "user-controls"];

/// The secrets Project Settings holds, by the name a tool uses, and the slot
/// the IDE keeps each in.
fn secret_slot(name: &str) -> Option<String> {
    match name {
        "google_maps_api_key" => Some(crate::llm::GOOGLE_MAPS_API_KEY_SLOT.to_owned()),
        "google_custom_search_api_key" => Some(crate::llm::GOOGLE_CUSTOM_SEARCH_API_KEY_SLOT.to_owned()),
        _ => None,
    }
}

/// `value` with `patch` merged in, object by object.
fn merge(value: &mut Value, patch: &Value) {
    match (value, patch) {
        (Value::Object(v), Value::Object(p)) => {
            for (k, pv) in p {
                match v.get_mut(k) {
                    Some(slot) if slot.is_object() && pv.is_object() => merge(slot, pv),
                    _ => {
                        v.insert(k.clone(), pv.clone());
                    }
                }
            }
        }
        (v, p) => *v = p.clone(),
    }
}

impl CoboltApp {
    // ── Forms (R56, R57) ────────────────────────────────────────────────────

    pub(super) fn mcp_apply_change_set(&mut self, args: &Value) -> Result<Value, String> {
        let ops = args
            .get("operations")
            .and_then(Value::as_array)
            .filter(|a| !a.is_empty())
            .ok_or("'operations' is required: a list of operations, each an object with an `op`")?;
        let mut steps: Vec<McpStep> = Vec::new();
        let mut create: Option<AgentOp> = None;
        for (i, op) in ops.iter().enumerate() {
            let kind = op.get("op").and_then(Value::as_str).unwrap_or_default();
            let field = |k: &str| op.get(k).and_then(Value::as_str).map(str::trim).filter(|s| !s.is_empty()).map(str::to_owned);
            match kind {
                "delete_control" => {
                    let id = field("control_id").or_else(|| field("id")).ok_or(format!("operation {i} (delete_control) needs `control_id`"))?;
                    steps.push(McpStep::Delete(id));
                }
                "rename_control" => {
                    let from = field("control_id").or_else(|| field("id")).ok_or(format!("operation {i} (rename_control) needs `control_id`"))?;
                    let to = field("new_id").ok_or(format!("operation {i} (rename_control) needs `new_id`"))?;
                    steps.push(McpStep::Rename { from, to });
                }
                _ => {
                    let parsed: AgentOp =
                        serde_json::from_value(op.clone()).map_err(|e| format!("operation {i} cannot be read: {e}"))?;
                    if matches!(parsed, AgentOp::CreateForm { .. }) {
                        if i != 0 {
                            return Err(format!("operation {i}: create_form must be the first operation of its change set"));
                        }
                        create = Some(parsed);
                        continue;
                    }
                    steps.push(McpStep::Op(parsed));
                }
            }
        }
        let root = self.mcp_root()?;
        let abs = match &create {
            Some(AgentOp::CreateForm { name, title, width, height, folder }) => {
                // Whole or not at all: the rest is tried on the blank form first,
                // so a refused operation creates nothing.
                let blank = self.build_new_form(
                    name,
                    title.as_deref().unwrap_or(name),
                    width.unwrap_or(640),
                    height.unwrap_or(480),
                    cobolt_forms::templates::FormTemplate::Blank,
                    true,
                    "",
                    "Classic",
                );
                let mut dry = DesignerPanel::new(blank);
                dry.properties.web_project = self.is_web_project();
                dry.apply_mcp_change_set(&steps)?;
                self.create_form_for_grace(name, folder.as_deref(), title.as_deref(), *width, *height)?
            }
            _ => {
                let rel = arg_str(args, "path").ok_or("'path' is required: the form, project-relative")?;
                let abs = self.mcp_abs(rel)?;
                if !abs.is_file() {
                    return Err(format!("{rel} is not a form of this project"));
                }
                abs
            }
        };
        let account = if steps.is_empty() {
            String::new()
        } else if let Some(idx) = self.designers.iter().position(|(p, _)| p == &abs) {
            let account = self.designers[idx].1.apply_mcp_change_set(&steps)?;
            // The designer's own save: the form, then its COBOL regenerated.
            self.do_save_designer(idx);
            account
        } else if let Some(st) = self.inspect.as_mut().filter(|st| st.path == abs) {
            let account = st.designer.apply_mcp_change_set(&steps)?;
            save_form(&st.designer.form, &abs).map_err(|e| format!("the form cannot be saved: {e}"))?;
            st.designer.dirty = false;
            st.mtime = super::super::file_mtime(&abs);
            account
        } else {
            let form = load_form(&abs).map_err(|e| format!("the form cannot be read: {e}"))?;
            let mut d = DesignerPanel::new(form);
            d.cfrm_dir = abs.parent().map(Path::to_path_buf);
            d.properties.web_project = self.is_web_project();
            let account = d.apply_mcp_change_set(&steps)?;
            save_form(&d.form, &abs).map_err(|e| format!("the form cannot be saved: {e}"))?;
            account
        };
        self.project.refresh_form(&abs);
        let rel = abs.strip_prefix(&root).unwrap_or(&abs).to_string_lossy().replace('\\', "/");
        Ok(json!({
            "path": rel,
            "created": create.is_some(),
            "operations": steps.len(),
            "account": account,
            "next": "regenerate the form, then check it"
        }))
    }

    // ── Project Settings (R59) ──────────────────────────────────────────────

    pub(super) fn mcp_project_settings(&mut self) -> Result<Value, String> {
        let proj = self.cobolt_project.as_ref().ok_or("no project is open in the IDE")?;
        let mut v = serde_json::to_value(proj).map_err(|e| e.to_string())?;
        if let Some(map) = v.as_object_mut() {
            for k in NOT_SETTINGS {
                map.remove(k);
            }
            if let Some(Value::Object(p)) = map.get_mut("project") {
                p.retain(|k, _| !k.contains("seal"));
            }
            if let Some(Value::Object(f)) = map.get_mut("forms") {
                f.retain(|k, _| !k.contains("seal"));
            }
        }
        let secret = |slot: &str| if self.llm.api_keys.get(slot).is_some_and(|k| !k.is_empty()) { "set" } else { "not set" };
        Ok(json!({
            "settings": v,
            "secrets": {
                "google_maps_api_key": secret(crate::llm::GOOGLE_MAPS_API_KEY_SLOT),
                "google_custom_search_api_key": secret(crate::llm::GOOGLE_CUSTOM_SEARCH_API_KEY_SLOT),
            },
            "note": "Change a field with set_project_settings: `values` mirrors `settings` (only what changes); a secret goes in `secrets`."
        }))
    }

    pub(super) fn mcp_set_project_settings(&mut self, args: &Value) -> Result<Value, String> {
        if self.settings_form.as_ref().is_some_and(|f| f.is_dirty()) {
            return Err("Project Settings has unsaved edits in the IDE; ask the developer to save or cancel them".to_owned());
        }
        let values = args.get("values").filter(|v| v.is_object()).ok_or("'values' is required: the settings to change, shaped as project_settings answers them")?;
        let proj = self.cobolt_project.as_ref().ok_or("no project is open in the IDE")?;
        let mut v = serde_json::to_value(proj).map_err(|e| e.to_string())?;
        let mut patch = values.clone();
        let mut secrets: Vec<(String, String)> = Vec::new();
        if let Some(map) = patch.as_object_mut() {
            for k in NOT_SETTINGS {
                if map.remove(k).is_some() {
                    return Err(format!("'{k}' is not a setting: use the file tools, add_crate or apply_change_set"));
                }
            }
            if let Some(Value::Object(s)) = map.remove("secrets") {
                for (name, val) in s {
                    let slot = secret_slot(&name).ok_or(format!("'{name}' is not a secret of Project Settings"))?;
                    secrets.push((slot, val.as_str().unwrap_or_default().to_owned()));
                }
            }
        }
        merge(&mut v, &patch);
        let next: crate::project_model::CoboltProject =
            serde_json::from_value(v).map_err(|e| format!("those values do not fit Project Settings: {e}"))?;
        let changed: Vec<String> = patch.as_object().map(|m| m.keys().cloned().collect()).unwrap_or_default();
        self.cobolt_project = Some(next);
        for (slot, key) in &secrets {
            if key.is_empty() {
                self.llm.withdraw_api_key(slot);
            } else {
                self.llm.store_api_key(slot.clone(), key);
            }
        }
        // The Settings form's own save: it applies, writes the Backend's files,
        // saves the AI settings and the project (which re-seals it).
        let proj = self.cobolt_project.as_ref().ok_or("no project is open in the IDE")?;
        let form = crate::panels::settings_form::SettingsForm::new(proj, &self.llm);
        self.settings_form = Some(form);
        self.save_settings_form();
        Ok(json!({ "changed": changed, "secrets": secrets.len(), "saved": true }))
    }

    // ── SQL connections (R60) ───────────────────────────────────────────────

    pub(super) fn mcp_sql_connections(&mut self) -> Result<Value, String> {
        let proj = self.cobolt_project.as_ref().ok_or("no project is open in the IDE")?;
        let app = proj.project.name.clone();
        let list: Vec<Value> = proj
            .sql_connections
            .iter()
            .map(|c| {
                let slot = |part| cobolt_forms::connections::sql_credential_slot(&app, &c.name, part);
                let mut v = serde_json::to_value(c).unwrap_or(Value::Null);
                if let Some(m) = v.as_object_mut() {
                    m.insert("user".into(), json!(self.llm.api_keys.get(&slot("user")).cloned().unwrap_or_default()));
                    m.insert("password".into(), json!(if self.llm.api_keys.contains_key(&slot("password")) { "set" } else { "not set" }));
                }
                v
            })
            .collect();
        Ok(json!({ "connections": list }))
    }

    pub(super) fn mcp_set_sql_connection(&mut self, args: &Value) -> Result<Value, String> {
        let name = arg_str(args, "name").ok_or("'name' is required")?.to_owned();
        if name.is_empty() || cobolt_forms::connections::sql_env_name(&name).is_empty() {
            return Err(format!("'{name}' is not a connection name"));
        }
        let project = self.cobolt_project.as_mut().ok_or("no project is open in the IDE")?;
        let app = project.project.name.clone();
        let at = project.sql_connections.iter().position(|c| c.name.eq_ignore_ascii_case(&name));
        let mut v = match at {
            Some(i) => serde_json::to_value(&project.sql_connections[i]).map_err(|e| e.to_string())?,
            None => json!({ "name": name }),
        };
        let mut fields = args.get("fields").cloned().unwrap_or_else(|| json!({}));
        let user = fields.as_object_mut().and_then(|m| m.remove("user")).and_then(|u| u.as_str().map(str::to_owned));
        if let Some(m) = fields.as_object_mut() {
            m.remove("password");
            m.remove("name");
        }
        merge(&mut v, &fields);
        let def: cobolt_forms::connections::SqlConnection =
            serde_json::from_value(v).map_err(|e| format!("those fields do not make a SQL connection: {e}"))?;
        if !["sqlite", "postgresql", "mysql"].contains(&def.backend.as_str()) {
            return Err(format!("'{}' is not an engine: sqlite, postgresql or mysql", def.backend));
        }
        if def.default {
            for c in project.sql_connections.iter_mut() {
                c.default = false;
            }
        }
        match at {
            Some(i) => project.sql_connections[i] = def.clone(),
            None => project.sql_connections.push(def.clone()),
        }
        let slot = |part| cobolt_forms::connections::sql_credential_slot(&app, &def.name, part);
        if let Some(u) = user {
            if u.is_empty() {
                self.llm.withdraw_api_key(&slot("user"));
            } else {
                self.llm.store_api_key(slot("user"), &u);
            }
        }
        if let Some(pw) = arg_str(args, "password") {
            self.llm.store_api_key(slot("password"), pw);
        }
        if let Err(e) = self.llm.save() {
            self.output.push_status(crate::ui_text::idef("could not store the SQL connection's credentials: {}", &[&e]));
        }
        self.do_save_project();
        self.output.push_status(crate::ui_text::idef("Saved SQL connection {}", &[&def.name]));
        Ok(json!({ "saved": def.name, "created": at.is_none() }))
    }

    pub(super) fn mcp_remove_sql_connection(&mut self, args: &Value) -> Result<Value, String> {
        let name = arg_str(args, "name").ok_or("'name' is required")?.to_owned();
        let project = self.cobolt_project.as_mut().ok_or("no project is open in the IDE")?;
        let app = project.project.name.clone();
        let before = project.sql_connections.len();
        project.sql_connections.retain(|c| !c.name.eq_ignore_ascii_case(&name));
        if project.sql_connections.len() == before {
            return Err(format!("the project has no SQL connection '{name}'"));
        }
        crate::form_runtime::move_sql_credentials(&mut self.llm, &app, &name, None);
        if let Err(e) = self.llm.save() {
            self.output.push_status(crate::ui_text::idef("could not remove the SQL connection's credentials: {}", &[&e]));
        }
        self.do_save_project();
        self.output.push_status(crate::ui_text::idef("Removed SQL connection {}", &[&name]));
        Ok(json!({ "removed": name }))
    }

    /// The Test of the SQL Connections window, as a job for another thread:
    /// it may wait on the network.
    pub(super) fn mcp_sql_test_job(&mut self, args: &Value) -> Result<Box<dyn FnOnce() -> Result<Value, String> + Send>, String> {
        let name = arg_str(args, "name").ok_or("'name' is required")?.to_owned();
        let root = self.mcp_root()?;
        let project = self.cobolt_project.as_ref().ok_or("no project is open in the IDE")?;
        let def = project
            .sql_connections
            .iter()
            .find(|c| c.name.eq_ignore_ascii_case(&name))
            .cloned()
            .ok_or(format!("the project has no SQL connection '{name}'"))?;
        let app = project.project.name.clone();
        let slot = |part| cobolt_forms::connections::sql_credential_slot(&app, &def.name, part);
        let user = self.llm.api_keys.get(&slot("user")).cloned().unwrap_or_default();
        let password = self.llm.api_keys.get(&slot("password")).cloned().unwrap_or_default();
        Ok(Box::new(move || {
            match crate::panels::sql_connections::test_connection_with(&def, &root, &user, &password) {
                Ok(what) => Ok(json!({ "connection": def.name, "ok": true, "answer": what })),
                // The database's own words; a password never appears in them, but
                // make sure.
                Err(e) => Err(if password.is_empty() { e } else { e.replace(&password, "***") }),
            }
        }))
    }

    // ── Save, package, upgrade (R61) ────────────────────────────────────────

    pub(super) fn mcp_save_project(&mut self) -> Result<Value, String> {
        self.mcp_root()?;
        let had = self.has_unsaved_changes();
        self.save_all_unsaved();
        self.do_save_project();
        Ok(json!({ "saved": true, "had_unsaved": had }))
    }

    pub(super) fn mcp_package_project(&mut self, args: &Value) -> Result<Value, String> {
        let root = self.mcp_root()?;
        let name = self
            .cobolt_project
            .as_ref()
            .map(|p| p.project.name.to_ascii_lowercase().replace(' ', "_"))
            .ok_or("no project is open in the IDE")?;
        let dir = match arg_str(args, "to") {
            Some(to) => {
                let p = std::path::PathBuf::from(to);
                if !p.is_absolute() || !p.is_dir() {
                    return Err(format!("'{to}' is not an existing absolute folder"));
                }
                p
            }
            None => root,
        };
        let zip = dir.join(format!("{name}.zip"));
        self.package_project_to(zip.clone());
        if !zip.is_file() {
            return Err("the package was not written; read_output says why".to_owned());
        }
        Ok(json!({ "package": zip.display().to_string() }))
    }

    pub(super) fn mcp_upgrade_project(&mut self) -> Result<Value, String> {
        self.mcp_root()?;
        self.detect_project_upgrades();
        let upgrades = self.project_upgrades.clone();
        if upgrades.is_empty() {
            return Ok(json!({ "upgraded": [], "note": "the project is already in this version's layout" }));
        }
        let tr = self.lang.tr();
        self.apply_project_upgrades(&upgrades, &tr);
        self.detect_project_upgrades();
        let left = self.project_upgrades.len();
        Ok(json!({ "offered": upgrades.len(), "still_pending": left, "note": "read_output has the IDE's account" }))
    }

    // ── Project Knowledge Base (R64) ────────────────────────────────────────

    pub(super) fn mcp_project_kb_search(&mut self, args: &Value) -> Result<Value, String> {
        let root = self.mcp_root()?;
        let query = arg_str(args, "query").ok_or("'query' is required")?;
        let limit = args.get("limit").and_then(Value::as_u64).unwrap_or(8).clamp(1, 50) as usize;
        let store = cobolt_agents::chunked_knowledge::project_chunked_path(&root);
        let _ = cobolt_agents::chunked_knowledge::sync_tree(&root, &store);
        let hits = cobolt_agents::chunked_knowledge::search(&store, query, limit)?;
        let hits: Vec<Value> = hits
            .into_iter()
            .map(|h| json!({ "subject": h.subject, "kind": h.kind, "content": h.content, "score": h.score }))
            .collect();
        Ok(json!({ "query": query, "hits": hits }))
    }

    // ── An indexed file's records (R64) ─────────────────────────────────────

    pub(super) fn mcp_indexed_records(&mut self, args: &Value) -> Result<Value, String> {
        use crate::panels::indexed_field_control::{build_record, format_cell, EditCell};
        let rel = arg_str(args, "path").ok_or("'path' is required: the .cidx")?;
        let cidx = self.mcp_abs(rel)?;
        let def = cobolt_indexed::load_indexed(&cidx).map_err(|e| format!("{rel} cannot be read: {e}"))?;
        let data = self.resolve_assign_path(&def).ok_or("no project is open in the IDE")?;
        let action = arg_str(args, "action").ok_or("'action' is required")?;
        if action != "list" && action != "find" && self.indexed_grids.iter().any(|(p, _)| p == &cidx) {
            return Err(format!("{rel} is open in the IDE's record grid; ask the developer to close it first"));
        }
        let (drift, detail) = self.check_schema_drift(&def);
        if drift && action != "list" && action != "find" {
            return Err(format!("the data file does not match {rel} ({}); records are not changed", detail.unwrap_or_default()));
        }
        let tr = self.lang.tr();
        let leaves: Vec<&cobolt_indexed::IndexedField> = def.fields.first().map(|r| r.all_leaves()).unwrap_or_default();
        let record_len = def.record_length() as usize;
        let show = |row: &[u8]| -> Value {
            let mut m = Map::new();
            for f in &leaves {
                m.insert(f.name.clone(), Value::String(format_cell(f, row, &tr)));
            }
            Value::Object(m)
        };
        let key_field = leaves.first().map(|f| f.name.clone()).unwrap_or_default();
        let key_of = |row: &[u8]| leaves.first().map(|f| format_cell(f, row, &tr).trim().to_owned()).unwrap_or_default();
        if action == "list" && !data.exists() {
            return Ok(json!({ "records": [], "note": "the data file does not exist yet" }));
        }
        let mut session = cobolt_runtime::indexed_ide::GridSession::open(&def, &data)?;
        let want = arg_str(args, "key").map(str::to_owned);
        let find = |session: &cobolt_runtime::indexed_ide::GridSession| -> Option<usize> {
            let k = want.as_deref()?.trim().to_owned();
            session.rows().iter().position(|r| key_of(r) == k)
        };
        let build = |base: &[u8], record: &Value| -> Result<Vec<u8>, String> {
            let given = record.as_object().ok_or("'record' is required: field name → value")?;
            let cells: Vec<EditCell> = leaves
                .iter()
                .map(|f| match given.iter().find(|(k, _)| k.eq_ignore_ascii_case(&f.name)).map(|(_, v)| v) {
                    None => EditCell::from_field(f, base),
                    Some(Value::Bool(b)) => EditCell::Bool(*b),
                    Some(Value::Number(n)) => match EditCell::from_field(f, base) {
                        EditCell::Number(_) => EditCell::Number(n.as_i64().unwrap_or(0)),
                        _ => EditCell::Text(n.to_string()),
                    },
                    Some(Value::String(s)) => match EditCell::from_field(f, base) {
                        EditCell::Number(_) => EditCell::Number(s.trim().parse().unwrap_or(0)),
                        EditCell::Bool(_) => EditCell::Bool(matches!(s.trim().to_ascii_lowercase().as_str(), "true" | "y" | "yes" | "1")),
                        EditCell::Text(_) => EditCell::Text(s.clone()),
                    },
                    Some(other) => EditCell::Text(other.to_string()),
                })
                .collect();
            build_record(&cells, &leaves, record_len, &tr)
        };
        let result = match action {
            "list" => {
                let limit = args.get("limit").and_then(Value::as_u64).unwrap_or(50).clamp(1, 1000) as usize;
                let rows: Vec<Value> = session.rows().iter().take(limit).map(|r| show(r)).collect();
                json!({ "count": session.rows().len(), "records": rows, "key": key_field })
            }
            "find" => match find(&session) {
                Some(i) => json!({ "record": show(&session.rows()[i]) }),
                None => json!({ "record": Value::Null, "note": "no record has that key" }),
            },
            "add" => {
                let rec = build(&vec![b' '; record_len], args.get("record").unwrap_or(&Value::Null))?;
                session.write_row(&rec)?;
                session.commit();
                json!({ "added": show(&rec) })
            }
            "update" => {
                let i = find(&session).ok_or("no record has that key")?;
                let base = session.rows()[i].clone();
                let rec = build(&base, args.get("record").unwrap_or(&Value::Null))?;
                session.select_row(i)?;
                session.rewrite_row(&rec)?;
                session.commit();
                json!({ "updated": show(&rec) })
            }
            "delete" => {
                let i = find(&session).ok_or("no record has that key")?;
                let gone = show(&session.rows()[i]);
                session.select_row(i)?;
                session.delete_current()?;
                session.commit();
                json!({ "deleted": gone })
            }
            other => return Err(format!("'{other}' is not an action: list, find, add, update or delete")),
        };
        Ok(result)
    }

    // ── Beautify (R64) ──────────────────────────────────────────────────────

    pub(super) fn mcp_beautify(&mut self, args: &Value) -> Result<Value, String> {
        let rel = arg_str(args, "path").ok_or("'path' is required: a COBOL source, project-relative")?;
        let abs = self.mcp_abs(rel)?;
        let ext = abs.extension().and_then(|e| e.to_str()).unwrap_or_default().to_ascii_lowercase();
        if !matches!(ext.as_str(), "cbl" | "cob" | "cpy") {
            return Err(format!("{rel} is not a COBOL source; a form's handlers are beautified in the designer's code editor"));
        }
        if rel.replace('\\', "/").split('/').any(|c| c == "generated") {
            return Err("generated code is rewritten by regenerate; it is not beautified".to_owned());
        }
        let text = std::fs::read_to_string(&abs).map_err(|e| format!("{rel} cannot be read: {e}"))?;
        let (verb_case, align_comments, style) = crate::ui_prefs::load_beautify();
        let opts = crate::panels::beautify::BeautifyOptions { verb_case, align_comments, style };
        let formatted = match crate::panels::beautify::beautify_with_rules(&text, &opts) {
            crate::panels::beautify::Beautified::Formatted(t) => t,
            crate::panels::beautify::Beautified::Rejected(why) => {
                return Err(format!("Beautify left {rel} as it is: {}", why.join("; ")))
            }
        };
        // As the editor's Beautify: the value methods take their one spelling too.
        let formatted = crate::value_methods::rewrite_program(&formatted, &abs).map(|r| r.text).unwrap_or(formatted);
        if formatted == text {
            return Ok(json!({ "path": rel, "changed": false }));
        }
        std::fs::write(&abs, &formatted).map_err(|e| format!("{rel} cannot be written: {e}"))?;
        self.editor.reload_file(&abs);
        Ok(json!({ "path": rel, "changed": true }))
    }
}
