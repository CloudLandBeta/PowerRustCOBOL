// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! AI Development Agent (spec 025) — dev-time form-building assistant.
//!
//! The agent turns a developer's natural-language request into a **structured
//! change-set** over the current form/project: deploy controls, set any property,
//! generate COBOL event handlers, and create common procedures. The change-set is
//! previewed and only applied on explicit approval (as one undoable action).
//!
//! This module owns the change-set data model + JSON parsing (T1), schema
//! validation (T2), the request CONTEXT builder (T3), and the `agentic_ai/`
//! scaffold + prompt/skill resolvers (T4).

use serde::Deserialize;

// ── Change-set data model (T1) ───────────────────────────────────────────────

/// One structured operation the agent may propose. `op` is the JSON discriminator
/// (`deploy_control`, `set_property`, `generate_event_handler`, `create_procedure`).
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum AgentOp {
    /// Deploy a new control onto the form.
    DeployControl {
        control_type: String,
        #[serde(default)]
        id: Option<String>,
        #[serde(default)]
        properties: serde_json::Map<String, serde_json::Value>,
    },
    /// Set one property (any key) on an existing control.
    SetProperty {
        control_id: String,
        key: String,
        value: serde_json::Value,
    },
    /// Generate a COBOL event-handler body bound to a control's event.
    GenerateEventHandler {
        control_id: String,
        event: String,
        code: String,
    },
    /// Create a common (shared) procedure.
    CreateProcedure { name: String, code: String },
}

/// A parsed agent reply: an ordered list of operations, plus an optional note used
/// when the agent cannot express the request as operations.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct AgentChangeSet {
    pub operations: Vec<AgentOp>,
    #[serde(default)]
    pub note: Option<String>,
}

/// Parse the model reply into a change-set. The agent must reply with a single JSON
/// object; we accept it inside a ```json fence (preferred), a bare ``` fence, or as
/// the whole trimmed message. Any structural problem is a hard error — the caller
/// applies nothing (R3/R9/R12).
pub fn parse_change_set(reply: &str) -> Result<AgentChangeSet, String> {
    let json = extract_json(reply)
        .ok_or_else(|| "The agent reply did not contain a JSON change-set.".to_string())?;
    serde_json::from_str::<AgentChangeSet>(json)
        .map_err(|e| format!("The agent change-set was not valid: {e}"))
}

/// Pull the JSON body out of a reply: the contents of a ```json / ``` fence if
/// present, otherwise the message trimmed to its first `{` … last `}`.
fn extract_json(reply: &str) -> Option<&str> {
    if let Some(start) = reply.find("```") {
        let after = &reply[start + 3..];
        // Skip an optional language tag on the fence line (e.g. `json`).
        let body_start = after.find('\n').map(|n| n + 1).unwrap_or(0);
        let body = &after[body_start..];
        if let Some(end) = body.find("```") {
            let inner = body[..end].trim();
            if !inner.is_empty() {
                return Some(inner);
            }
        }
    }
    // No usable fence — take the outermost braces.
    let s = reply.trim();
    let open = s.find('{')?;
    let close = s.rfind('}')?;
    if close > open {
        Some(&s[open..=close])
    } else {
        None
    }
}

// ── Preview state (T9) ───────────────────────────────────────────────────────

/// A proposed change-set held **pending** between the agent's reply and the
/// developer's decision (spec 025 R5). Owns the parsed change-set and the per-op
/// validation status (`None` = ok, `Some(msg)` = an error shown in the preview and
/// blocked from apply). Nothing here is applied until the developer approves.
#[derive(Debug, Clone)]
pub struct AgentPreview {
    pub change_set: AgentChangeSet,
    /// Aligned with `change_set.operations`; `None` = valid, `Some` = error.
    pub statuses: Vec<Option<String>>,
    /// Optional agent note (when it returned no operations).
    pub note: Option<String>,
}

impl AgentPreview {
    /// Build a preview by validating a change-set against the current form.
    pub fn build(change_set: AgentChangeSet, form: &Form) -> Self {
        let statuses = validate(&change_set, form);
        let note = change_set.note.clone();
        Self {
            change_set,
            statuses,
            note,
        }
    }

    /// Operations that would actually apply (valid ones).
    pub fn valid_count(&self) -> usize {
        self.statuses.iter().filter(|s| s.is_none()).count()
    }

    /// Any operation failed validation.
    pub fn has_errors(&self) -> bool {
        self.statuses.iter().any(|s| s.is_some())
    }

    /// True when there is at least one applicable operation to approve.
    pub fn is_applicable(&self) -> bool {
        self.valid_count() > 0
    }
}

// ── Validation against the live schema (T2) ──────────────────────────────────

use cobolt_forms::model::property_names_for;
use cobolt_forms::{ControlType, Form};
use std::collections::HashMap;

/// Validate each operation against the real form + control/property/event schema.
/// Returns one entry per operation, aligned with `cs.operations`: `None` = valid,
/// `Some(msg)` = an error that must be shown in the preview and cannot be applied
/// (R9). Controls introduced by an earlier `deploy_control` in the same change-set
/// count as valid targets for later ops.
pub fn validate(cs: &AgentChangeSet, form: &Form) -> Vec<Option<String>> {
    // id → type for everything the change-set may legally target.
    let mut known: HashMap<String, ControlType> = form
        .controls
        .iter()
        .map(|c| (c.id.clone(), c.control_type.clone()))
        .collect();

    cs.operations
        .iter()
        .map(|op| validate_op(op, &mut known))
        .collect()
}

fn validate_op(op: &AgentOp, known: &mut HashMap<String, ControlType>) -> Option<String> {
    match op {
        AgentOp::DeployControl {
            control_type,
            id,
            properties,
        } => {
            let ct = ControlType::from_str(control_type);
            if matches!(ct, ControlType::Custom { .. }) {
                return Some(format!("Unknown control type '{control_type}'."));
            }
            // Property keys, if any, must be valid for the new control's type.
            for key in properties.keys() {
                if !property_valid(&ct, key) {
                    return Some(format!("'{control_type}' has no property '{key}'."));
                }
            }
            if let Some(id) = id {
                known.insert(id.clone(), ct);
            }
            None
        }
        AgentOp::SetProperty {
            control_id,
            key,
            ..
        } => match known.get(control_id) {
            None => Some(format!("No control named '{control_id}'.")),
            Some(ct) if !property_valid(ct, key) => {
                Some(format!("Control '{control_id}' has no property '{key}'."))
            }
            _ => None,
        },
        AgentOp::GenerateEventHandler {
            control_id, event, ..
        } => match known.get(control_id) {
            None => Some(format!("No control named '{control_id}'.")),
            Some(ct) if !ct.supported_events().iter().any(|e| e.eq_ignore_ascii_case(event)) => {
                Some(format!("Control '{control_id}' has no event '{event}'."))
            }
            _ => None,
        },
        AgentOp::CreateProcedure { name, .. } => {
            if name.trim().is_empty() {
                Some("Procedure name is empty.".to_string())
            } else {
                None
            }
        }
    }
}

/// A property key is valid for a control type when it is one of the canonical
/// settable keys (`property_names_for`), compared case-insensitively (RustCOBOL
/// property names are case-insensitive).
fn property_valid(ct: &ControlType, key: &str) -> bool {
    property_names_for(ct.as_str())
        .iter()
        .any(|k| k.eq_ignore_ascii_case(key))
}

// ── Default assets, scaffold & resolvers (T4) ────────────────────────────────

use std::path::{Path, PathBuf};

/// The built-in dev-agent system prompt (seed / reset default). Embedded so the
/// binary never depends on the `specs/` tree.
pub const AGENT_SYSTEM_PROMPT: &str =
    include_str!("assets/agentic_ai/system-prompt.md");

/// The default RustCOBOL-extensions skill, always injected into the agent context.
const DEFAULT_RUSTCOBOL_SKILL: &str =
    include_str!("assets/agentic_ai/skills/rustcobol-extensions.md");

/// Relative locations under a project directory.
const AGENTIC_DIR: &str = "agentic_ai";
const PROMPT_FILE: &str = "system-prompt.md";
/// The general code/event assistant's own prompt (separate from the dev agent's
/// `system-prompt.md`): it asks for COBOL in a fenced block, not JSON change-sets.
const ASSISTANT_PROMPT_FILE: &str = "assistant-prompt.md";
const SKILLS_DIR: &str = "skills";
const DEFAULT_SKILL_FILE: &str = "rustcobol-extensions.md";

/// `<project>/agentic_ai`.
pub fn agentic_dir(project_dir: &Path) -> PathBuf {
    project_dir.join(AGENTIC_DIR)
}

/// Ensure the `agentic_ai/` scaffold exists (R18/R19). Creates the folder and
/// writes **only the missing** default files — the prompt and the default skill —
/// never overwriting an existing file, so an edited prompt/skill is preserved.
/// Idempotent: safe to call on every project create and open.
pub fn ensure_agentic_ai_scaffold(project_dir: &Path) -> std::io::Result<()> {
    let base = agentic_dir(project_dir);
    let skills = base.join(SKILLS_DIR);
    std::fs::create_dir_all(&skills)?;

    let prompt = base.join(PROMPT_FILE);
    if !prompt.exists() {
        std::fs::write(&prompt, AGENT_SYSTEM_PROMPT)?;
    }
    let assistant_prompt = base.join(ASSISTANT_PROMPT_FILE);
    if !assistant_prompt.exists() {
        std::fs::write(&assistant_prompt, crate::llm::DEFAULT_SYSTEM_PROMPT)?;
    }
    let skill = skills.join(DEFAULT_SKILL_FILE);
    if !skill.exists() {
        std::fs::write(&skill, DEFAULT_RUSTCOBOL_SKILL)?;
    }
    Ok(())
}

/// The effective system prompt for a project (R14): the edited
/// `agentic_ai/system-prompt.md` when present and non-empty, otherwise the built-in
/// default.
pub fn effective_prompt(project_dir: &Path) -> String {
    let path = agentic_dir(project_dir).join(PROMPT_FILE);
    match std::fs::read_to_string(&path) {
        Ok(text) if !text.trim().is_empty() => text,
        _ => AGENT_SYSTEM_PROMPT.to_string(),
    }
}

/// The effective **general assistant** prompt (code editor / event editor): the
/// edited `agentic_ai/assistant-prompt.md` when present and non-empty, otherwise
/// the built-in COBOL code-assistant default. Kept separate from the dev agent's
/// `system-prompt.md` so the two never collide.
pub fn effective_assistant_prompt(project_dir: &Path) -> String {
    let path = agentic_dir(project_dir).join(ASSISTANT_PROMPT_FILE);
    match std::fs::read_to_string(&path) {
        Ok(text) if !text.trim().is_empty() => text,
        _ => crate::llm::DEFAULT_SYSTEM_PROMPT.to_string(),
    }
}

/// Concatenated text of every `*.md` skill under `agentic_ai/skills/` (R21). Always
/// includes the default RustCOBOL skill even if the file is missing.
pub fn load_skills(project_dir: &Path) -> String {
    let dir = agentic_dir(project_dir).join(SKILLS_DIR);
    let mut files: Vec<PathBuf> = std::fs::read_dir(&dir)
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().map(|x| x == "md").unwrap_or(false))
        .collect();
    files.sort();

    let mut out = String::new();
    let mut saw_default = false;
    for f in &files {
        if f.file_name().map(|n| n == DEFAULT_SKILL_FILE).unwrap_or(false) {
            saw_default = true;
        }
        if let Ok(text) = std::fs::read_to_string(f) {
            if !out.is_empty() {
                out.push_str("\n\n");
            }
            out.push_str(&text);
        }
    }
    if !saw_default {
        if !out.is_empty() {
            out.push_str("\n\n");
        }
        out.push_str(DEFAULT_RUSTCOBOL_SKILL);
    }
    out
}

// ── Request CONTEXT builder (T3) ─────────────────────────────────────────────

use cobolt_forms::{Control, PropValue};

/// Build the compact CONTEXT block appended to each agent request (R2): the form's
/// control inventory (id, type, geometry, non-default properties), a per-type
/// property/event legend, and existing procedure names. Kept terse to save tokens.
pub fn build_context(form: &Form) -> String {
    let mut out = String::new();
    out.push_str("CONTEXT\n");
    out.push_str(&format!(
        "FORM: {} ({}x{})\n",
        form.name, form.width, form.height
    ));

    out.push_str("CONTROLS:\n");
    if form.controls.is_empty() {
        out.push_str("  (none)\n");
    }
    for c in &form.controls {
        let ty = c.control_type.as_str();
        let mut line = format!(
            "  {} ({}) @({},{}) {}x{}",
            c.id, ty, c.rect.x, c.rect.y, c.rect.w, c.rect.h
        );
        let nd = non_default_props(c);
        if !nd.is_empty() {
            line.push_str("  ");
            line.push_str(&nd.join(" "));
        }
        out.push_str(&line);
        out.push('\n');
    }

    // Legends only for the types actually in use.
    let mut types: Vec<String> = form
        .controls
        .iter()
        .map(|c| c.control_type.as_str().to_string())
        .collect();
    types.sort();
    types.dedup();

    out.push_str("PROPERTY KEYS BY TYPE:\n");
    for t in &types {
        out.push_str(&format!("  {}: {}\n", t, property_names_for(t).join(", ")));
    }
    out.push_str("EVENTS BY TYPE:\n");
    for t in &types {
        let evs = ControlType::from_str(t).supported_events().join(", ");
        out.push_str(&format!("  {}: {}\n", t, evs));
    }

    let procs: Vec<&str> = form.user_procedures.iter().map(|p| p.name.as_str()).collect();
    out.push_str(&format!(
        "PROCEDURES: {}\n",
        if procs.is_empty() {
            "(none)".to_string()
        } else {
            procs.join(", ")
        }
    ));
    out
}

/// Properties whose value differs from a fresh control of the same type — the ones
/// worth showing the agent (defaults are implied by the type).
fn non_default_props(c: &Control) -> Vec<String> {
    let defaults = Control::new("_", c.control_type.clone(), 0, 0);
    let mut out = Vec::new();
    for (k, v) in &c.properties {
        if defaults.properties.get(k) != Some(v) {
            out.push(format!("{k}={}", prop_display(v)));
        }
    }
    out.sort();
    out
}

fn prop_display(v: &PropValue) -> String {
    match v {
        PropValue::String(s) => format!("\"{s}\""),
        PropValue::Int(n) => n.to_string(),
        PropValue::Bool(b) => b.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_well_formed_change_set_all_four_ops() {
        let reply = r#"Here you go:
```json
{ "operations": [
  { "op": "deploy_control", "control_type": "Button", "id": "SAVE",
    "properties": { "Caption": "Save", "X": 10 } },
  { "op": "set_property", "control_id": "L1", "key": "Bold", "value": true },
  { "op": "generate_event_handler", "control_id": "SAVE", "event": "onClick",
    "code": "       PROCEDURE DIVISION.\n           CONTINUE.\n" },
  { "op": "create_procedure", "name": "VALIDATE-INPUT", "code": "       PROCEDURE DIVISION.\n" }
] }
```"#;
        let cs = parse_change_set(reply).expect("should parse");
        assert_eq!(cs.operations.len(), 4);
        assert!(matches!(cs.operations[0], AgentOp::DeployControl { .. }));
        assert!(matches!(cs.operations[1], AgentOp::SetProperty { .. }));
        assert!(matches!(cs.operations[2], AgentOp::GenerateEventHandler { .. }));
        assert!(matches!(cs.operations[3], AgentOp::CreateProcedure { .. }));
    }

    #[test]
    fn parse_bare_json_object_without_fence() {
        let cs = parse_change_set(r#"{ "operations": [], "note": "nothing to do" }"#)
            .expect("bare object parses");
        assert!(cs.operations.is_empty());
        assert_eq!(cs.note.as_deref(), Some("nothing to do"));
    }

    #[test]
    fn malformed_and_unknown_op_are_errors() {
        assert!(parse_change_set("no json here at all").is_err());
        assert!(parse_change_set(r#"{ "operations": [ { "op": "explode" } ] }"#).is_err());
        assert!(parse_change_set(r#"{ "operations": [ { "op": "set_property" } ] }"#).is_err());
    }

    fn form_with_label() -> Form {
        use cobolt_forms::Control;
        let mut f = Form::new("F", "F", 400, 300);
        f.controls.push(Control::new("L1", ControlType::Label, 0, 0));
        f
    }

    #[test]
    fn validate_flags_unknown_id_type_key_and_event() {
        let form = form_with_label();
        let cs = AgentChangeSet {
            operations: vec![
                // ok: deploy a valid Button
                AgentOp::DeployControl {
                    control_type: "Button".into(),
                    id: Some("B1".into()),
                    properties: Default::default(),
                },
                // error: unsupported control type
                AgentOp::DeployControl {
                    control_type: "Frobnicator".into(),
                    id: None,
                    properties: Default::default(),
                },
                // error: unknown control id
                AgentOp::SetProperty {
                    control_id: "NOPE".into(),
                    key: "Caption".into(),
                    value: serde_json::json!("x"),
                },
                // error: unknown property key on a real control
                AgentOp::SetProperty {
                    control_id: "L1".into(),
                    key: "Wingspan".into(),
                    value: serde_json::json!(1),
                },
                // ok: valid property (case-insensitive) on L1
                AgentOp::SetProperty {
                    control_id: "L1".into(),
                    key: "backgroundcolor".into(),
                    value: serde_json::json!("#fff"),
                },
                // ok: set a property on the Button deployed earlier in this set
                AgentOp::SetProperty {
                    control_id: "B1".into(),
                    key: "Caption".into(),
                    value: serde_json::json!("Go"),
                },
                // error: unsupported event for a Label
                AgentOp::GenerateEventHandler {
                    control_id: "L1".into(),
                    event: "onNonsense".into(),
                    code: "x".into(),
                },
            ],
            note: None,
        };
        let v = validate(&cs, &form);
        assert!(v[0].is_none(), "valid Button deploy");
        assert!(v[1].as_ref().unwrap().contains("Unknown control type"));
        assert!(v[2].as_ref().unwrap().contains("No control named"));
        assert!(v[3].as_ref().unwrap().contains("no property"));
        assert!(v[4].is_none(), "backgroundcolor is valid (case-insensitive)");
        assert!(v[5].is_none(), "B1 was deployed earlier in the set");
        assert!(v[6].as_ref().unwrap().contains("no event"));
    }

    #[test]
    fn context_lists_controls_and_legends() {
        let ctx = build_context(&form_with_label());
        assert!(ctx.contains("L1 (Label)"), "inventory: {ctx}");
        assert!(ctx.contains("PROPERTY KEYS BY TYPE:"));
        assert!(ctx.contains("Label:"));
        assert!(ctx.contains("EVENTS BY TYPE:"));
        assert!(ctx.contains("PROCEDURES:"));
    }

    struct TmpDir(PathBuf);
    impl TmpDir {
        fn new(tag: &str) -> Self {
            let t = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos();
            let p = std::env::temp_dir().join(format!("prc-agent-{tag}-{t}"));
            std::fs::create_dir_all(&p).unwrap();
            TmpDir(p)
        }
    }
    impl Drop for TmpDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn scaffold_creates_seeds_and_is_non_destructive() {
        let tmp = TmpDir::new("scaffold");
        let dir = &tmp.0;
        let prompt = agentic_dir(dir).join(PROMPT_FILE);
        let skill = agentic_dir(dir).join(SKILLS_DIR).join(DEFAULT_SKILL_FILE);

        // First run: creates folder + both defaults.
        ensure_agentic_ai_scaffold(dir).unwrap();
        assert!(prompt.exists() && skill.exists());
        assert_eq!(std::fs::read_to_string(&prompt).unwrap(), AGENT_SYSTEM_PROMPT);

        // Edit the prompt; re-run must NOT overwrite it.
        std::fs::write(&prompt, "MY CUSTOM PROMPT").unwrap();
        // Delete the skill; re-run must re-seed ONLY it.
        std::fs::remove_file(&skill).unwrap();
        ensure_agentic_ai_scaffold(dir).unwrap();
        assert_eq!(std::fs::read_to_string(&prompt).unwrap(), "MY CUSTOM PROMPT");
        assert!(skill.exists(), "deleted skill re-seeded");

        // effective_prompt returns the edited file, and the default when absent.
        assert_eq!(effective_prompt(dir), "MY CUSTOM PROMPT");
        std::fs::remove_file(&prompt).unwrap();
        assert_eq!(effective_prompt(dir), AGENT_SYSTEM_PROMPT);

        // load_skills always includes the RustCOBOL skill content.
        assert!(load_skills(dir).contains("RustCOBOL"));
    }
}
