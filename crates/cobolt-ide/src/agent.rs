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

use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use serde::Deserialize;

// ── Change-set data model (T1) ───────────────────────────────────────────────

/// One structured operation the agent may propose. `op` is the JSON discriminator
/// (`deploy_control`, `set_property`, `generate_event_handler`, `create_procedure`).
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum AgentOp {
    /// Deploy a new control onto the form.
    #[serde(alias = "add_control")]
    DeployControl {
        control_type: String,
        #[serde(default, alias = "control_id")]
        id: Option<String>,
        #[serde(default)]
        parent_id: Option<String>,
        #[serde(default, alias = "parent", alias = "Parent")]
        parent: Option<String>,
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
    /// Return a conversational message to the user.
    Message { message: String },
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
    let mut known: HashMap<String, ControlType> = form
        .controls
        .iter()
        .map(|c| (c.id.to_ascii_uppercase(), c.control_type.clone()))
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
            ..
        } => {
            let ct = ControlType::from_str(control_type);
            if matches!(ct, ControlType::Custom { .. }) {
                return Some(format!("Unknown control type '{control_type}'."));
            }
            // Property keys, if any, must be valid for the new control's type.
            for key in properties.keys() {
                if !deploy_property_valid(&ct, key) {
                    return Some(format!("'{control_type}' has no property '{key}'."));
                }
            }
            if let Some(id) = id {
                known.insert(id.to_ascii_uppercase(), ct);
            }
            None
        }
        AgentOp::SetProperty {
            control_id, key, ..
        } => match known.get(&control_id.to_ascii_uppercase()) {
            None => Some(format!("No control named '{control_id}'.")),
            Some(ct) if !property_valid(ct, key) => {
                Some(format!("Control '{control_id}' has no property '{key}'."))
            }
            _ => None,
        },
        AgentOp::GenerateEventHandler {
            control_id,
            event,
            code,
        } => {
            let base = match known.get(&control_id.to_ascii_uppercase()) {
                None => Some(format!("No control named '{control_id}'.")),
                Some(ct)
                    if !ct
                        .supported_events()
                        .iter()
                        .any(|e| e.eq_ignore_ascii_case(event)) =>
                {
                    Some(format!("Control '{control_id}' has no event '{event}'."))
                }
                _ => None,
            };
            base.or_else(|| handler_body_shape_error(code))
                .or_else(|| unknown_property_ref(code, known).map(bad_prop_msg))
        }
        AgentOp::CreateProcedure { name, code } => {
            if name.trim().is_empty() {
                Some("Procedure name is empty.".to_string())
            } else {
                handler_body_shape_error(code)
                    .or_else(|| unknown_property_ref(code, known).map(bad_prop_msg))
            }
        }
        AgentOp::Message { .. } => None,
    }
}

/// Ensure an agent-authored handler/procedure is the IDE-owned nested-program body,
/// not a partial fragment. The generator supplies IDENTIFICATION/PROGRAM-ID and
/// the footer, but the editable body must keep the three divisions so the model
/// cannot silently drop DATA/ENVIRONMENT during a round-trip.
pub(crate) fn handler_body_shape_error(code: &str) -> Option<String> {
    let has_line = |needle: &str| code.lines().any(|l| l.trim().eq_ignore_ascii_case(needle));
    if !has_line("ENVIRONMENT DIVISION.") {
        return Some(
            "Code must start from the nested-program body and include \
             ENVIRONMENT DIVISION."
                .to_string(),
        );
    }
    if !has_line("DATA DIVISION.") {
        return Some(
            "Code must include DATA DIVISION.; do not return a PROCEDURE-only \
             fragment."
                .to_string(),
        );
    }
    if !has_line("PROCEDURE DIVISION.") {
        return Some("Code must include PROCEDURE DIVISION.".to_string());
    }
    None
}

/// Message for a hallucinated property reference in generated code.
fn bad_prop_msg((ctrl, prop): (String, String)) -> String {
    format!(
        "Code references '{ctrl}::{prop}', but control '{ctrl}' has no property \
         '{prop}'. Use a real property of that control (e.g. depth ⇒ \
         ShadowBlurStrength) — see the control-properties skill."
    )
}

/// One `Control::member` reference found in generated code.
pub(crate) struct MemberRef {
    /// 1-based source line number.
    pub line: u32,
    /// Receiver identifier — the control id, with any `(index)` subscript stripped.
    pub recv: String,
    /// Member name (property or method), unquoted.
    pub member: String,
    /// `true` for a call/subscript `::member(...)` (a **method**), `false` for a
    /// bare `::member` (a **property**).
    pub is_call: bool,
}

/// Scan COBOL for simple, top-level `<control-id>[(index)]::<member>` references.
/// Deliberately conservative to avoid false positives: `::` inside string literals
/// and `*>` comments, chained members (`a::b::c`), and non-identifier receivers are
/// skipped. The caller decides validity (property vs method) against the schema.
pub(crate) fn scan_member_refs(code: &str) -> Vec<MemberRef> {
    let is_id = |c: u8| c.is_ascii_alphanumeric() || c == b'-';
    let mut out = Vec::new();
    for (lineno, raw) in code.lines().enumerate() {
        let line = raw.split("*>").next().unwrap_or(raw); // drop comment tail
        let b = line.as_bytes();
        let n = b.len();
        let mut i = 0usize;
        let mut in_str = false;
        while i + 1 < n {
            if b[i] == b'"' {
                in_str = !in_str;
                i += 1;
                continue;
            }
            if in_str || !(b[i] == b':' && b[i + 1] == b':') {
                i += 1;
                continue;
            }

            // ── receiver: scan left of `::` ──
            let mut k = i;
            while k > 0 && (b[k - 1] == b' ' || b[k - 1] == b'\t') {
                k -= 1;
            }
            if k > 0 && b[k - 1] == b')' {
                let mut depth = 0i32;
                while k > 0 {
                    k -= 1;
                    match b[k] {
                        b')' => depth += 1,
                        b'(' => {
                            depth -= 1;
                            if depth == 0 {
                                break;
                            }
                        }
                        _ => {}
                    }
                }
                while k > 0 && (b[k - 1] == b' ' || b[k - 1] == b'\t') {
                    k -= 1;
                }
            }
            let recv_end = k;
            while k > 0 && is_id(b[k - 1]) {
                k -= 1;
            }
            let recv_start = k;

            // ── member: scan right of `::` ──
            let mut m = i + 2;
            while m < n && (b[m] == b' ' || b[m] == b'\t') {
                m += 1;
            }
            let (mem_start, mem_end);
            if m < n && b[m] == b'"' {
                let s = m + 1;
                let mut e = s;
                while e < n && b[e] != b'"' {
                    e += 1;
                }
                mem_start = s;
                mem_end = e;
                m = if e < n { e + 1 } else { e };
            } else {
                let s = m;
                while m < n && is_id(b[m]) {
                    m += 1;
                }
                mem_start = s;
                mem_end = m;
            }
            let mut p = m;
            while p < n && (b[p] == b' ' || b[p] == b'\t') {
                p += 1;
            }
            let is_call = p < n && b[p] == b'(';

            i += 2; // past this `::`

            if recv_start == recv_end || mem_start == mem_end {
                continue;
            }
            if recv_start >= 2 && &line[recv_start - 2..recv_start] == "::" {
                continue; // chained member, not a top-level control
            }
            let recv = &line[recv_start..recv_end];
            if !recv.as_bytes()[0].is_ascii_alphabetic() {
                continue;
            }
            out.push(MemberRef {
                line: lineno as u32 + 1,
                recv: recv.to_string(),
                member: line[mem_start..mem_end].to_string(),
                is_call,
            });
        }
    }
    out
}

/// First `Control::Property` reference in `code` whose property does not exist on
/// its (known) control — a hallucinated property such as `TextBox-2::Depth`. Method
/// calls are ignored. Uses the same `property_valid` the `set_property` op trusts.
fn unknown_property_ref(
    code: &str,
    known: &HashMap<String, ControlType>,
) -> Option<(String, String)> {
    for r in scan_member_refs(code) {
        if r.is_call {
            continue; // methods handled elsewhere
        }
        if let Some(ct) = known.get(&r.recv.to_ascii_uppercase()) {
            if !property_valid(ct, &r.member) {
                return Some((r.recv, r.member));
            }
        }
    }
    None
}

/// A property key is valid for a control type when it is one of the canonical
/// settable keys (`property_names_for`), compared case-insensitively (RustCOBOL
/// property names are case-insensitive).
fn property_valid(ct: &ControlType, key: &str) -> bool {
    property_names_for(ct.as_str())
        .iter()
        .any(|k| k.eq_ignore_ascii_case(key))
}

fn deploy_property_valid(ct: &ControlType, key: &str) -> bool {
    matches!(
        key.to_ascii_lowercase().as_str(),
        "x" | "y" | "width" | "height" | "parent" | "parent_id" | "tab"
    ) || property_valid(ct, key)
}

// ── Default assets, scaffold & resolvers (T4) ────────────────────────────────

/// Relative locations under the IDE working directory.
const AGENTIC_DIR: &str = "agentic_ai";
const PROMPT_FILE: &str = "system-prompt.md";
/// The general code/event assistant's own prompt (separate from the dev agent's
/// `system-prompt.md`): it asks for COBOL in a fenced block, not JSON change-sets.
const ASSISTANT_PROMPT_FILE: &str = "assistant-prompt.md";
const SKILLS_DIR: &str = "skills";

/// Base IDE `agentic_ai` directory (always loaded).
pub fn agentic_dir() -> PathBuf {
    PathBuf::from(AGENTIC_DIR)
}

/// Project-specific overrides directory inside the IDE `agentic_ai` directory.
pub fn project_agentic_dir(project_dir: &Path) -> Option<PathBuf> {
    project_dir
        .file_name()
        .map(|name| agentic_dir().join("projects").join(name))
}

/// The effective system prompt for a project (R14).
pub fn effective_prompt(project_dir: &Path) -> String {
    let mut text = String::new();

    // Load global prompt
    let global_path = agentic_dir().join(PROMPT_FILE);
    if let Ok(content) = std::fs::read_to_string(&global_path) {
        text.push_str(&content);
    } else {
        text.push_str("You are an expert dev agent. No prompt found.");
    }

    // Append project-specific prompt
    if let Some(proj_dir) = project_agentic_dir(project_dir) {
        let proj_path = proj_dir.join(PROMPT_FILE);
        if let Ok(content) = std::fs::read_to_string(&proj_path) {
            if !text.is_empty() {
                text.push_str("\n\n");
            }
            text.push_str(&content);
        }
    }

    text
}

/// The effective **general assistant** prompt (code editor / event editor).
pub fn effective_assistant_prompt(project_dir: &Path) -> String {
    let mut text = String::new();

    // Load global prompt
    let global_path = agentic_dir().join(ASSISTANT_PROMPT_FILE);
    if let Ok(content) = std::fs::read_to_string(&global_path) {
        text.push_str(&content);
    } else {
        text.push_str(&crate::llm::DEFAULT_SYSTEM_PROMPT.to_string());
    }

    // Append project-specific prompt
    if let Some(proj_dir) = project_agentic_dir(project_dir) {
        let proj_path = proj_dir.join(ASSISTANT_PROMPT_FILE);
        if let Ok(content) = std::fs::read_to_string(&proj_path) {
            if !text.is_empty() {
                text.push_str("\n\n");
            }
            text.push_str(&content);
        }
    }

    text
}

/// Concatenated text of every `*.md` skill under `agentic_ai/skills/` (R21),
/// plus any project-specific skills under `agentic_ai/projects/<project>/skills/`.
pub fn load_skills(project_dir: &Path) -> String {
    let mut out = String::new();

    let mut append_skills_from_dir = |dir: &Path| {
        let mut files: Vec<PathBuf> = std::fs::read_dir(dir)
            .into_iter()
            .flatten()
            .flatten()
            .map(|e| e.path())
            .filter(|p| p.extension().map(|x| x == "md").unwrap_or(false))
            .collect();
        files.sort();

        for f in &files {
            if let Ok(text) = std::fs::read_to_string(f) {
                if !out.is_empty() {
                    out.push_str("\n\n");
                }
                out.push_str(&text);
            }
        }
    };

    // Load global skills
    append_skills_from_dir(&agentic_dir().join(SKILLS_DIR));

    // Load project-specific skills
    if let Some(proj_dir) = project_agentic_dir(project_dir) {
        append_skills_from_dir(&proj_dir.join(SKILLS_DIR));
    }

    out
}

// ── Request CONTEXT builder (T3) ─────────────────────────────────────────────

use cobolt_forms::{Control, PropValue};

use crate::project_model::{CoboltProject, ProjectFiles};

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

    out.push_str("AVAILABLE CONTROL TYPES (use these for 'deploy_control'):\n");
    out.push_str("  Button, TextBox, Label, CheckBox, RadioButton, ListBox, ComboBox, GroupBox, Panel, TabControl, DataGrid, PictureBox, ProgressBar, MenuBar, ToolBar, StatusBar, Line, DateTimePicker, NumericUpDown, TreeView, Splitter, Timer, Shape, Animator, AgentObject, RestClient, SqlDatabase, IndexedFile, Slider, BarChart, LineChart, PieChart, AreaChart, ScatterChart, DonutChart\n\n");

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

    let all_types = [
        "Button",
        "TextBox",
        "Label",
        "CheckBox",
        "RadioButton",
        "ListBox",
        "ComboBox",
        "GroupBox",
        "Panel",
        "TabControl",
        "DataGrid",
        "PictureBox",
        "ProgressBar",
        "MenuBar",
        "ToolBar",
        "StatusBar",
        "Line",
        "DateTimePicker",
        "NumericUpDown",
        "TreeView",
        "Splitter",
        "Timer",
        "Shape",
        "Animator",
        "AgentObject",
        "RestClient",
        "SqlDatabase",
        "Slider",
        "BarChart",
        "LineChart",
        "PieChart",
        "AreaChart",
        "ScatterChart",
        "DonutChart",
    ];

    out.push_str("PROPERTY KEYS BY TYPE (for all available controls):\n");
    for t in &all_types {
        out.push_str(&format!("  {}: {}\n", t, property_names_for(t).join(", ")));
    }
    out.push_str("EVENTS BY TYPE (for all available controls):\n");
    for t in &all_types {
        let evs = ControlType::from_str(t).supported_events().join(", ");
        out.push_str(&format!("  {}: {}\n", t, evs));
    }
    out.push_str("CONTROL API BY ID:\n");
    for c in &form.controls {
        let ty = c.control_type.as_str();
        let mut methods = crate::panels::editor::method_names_for_type(ty);
        if matches!(c.control_type, ControlType::GroupBox) {
            let is_array = form.data_bindings.iter().any(|b| {
                if let cobolt_forms::BindingTargetDescriptor::ControlArray { array_id, .. } =
                    &b.target
                {
                    c.explicit_control_array_id().as_deref() == Some(array_id.as_str())
                        || c.id.eq_ignore_ascii_case(array_id)
                } else {
                    false
                }
            });
            if is_array && !methods.iter().any(|m| m == "RefreshBinding") {
                methods.push("RefreshBinding".to_string());
            }
        }
        methods.sort();
        methods.dedup();
        out.push_str(&format!(
            "  {} ({}): properties [{}]; methods [{}]\n",
            c.id,
            ty,
            property_names_for(ty).join(", "),
            methods.join(", ")
        ));
    }
    out.push_str(
        "PROPERTY INTENT MAP: drop shadow/dropshadow/shadow on/sombra => \
         ShadowEnabled; depth/relief/elevation/profundidad => ShadowBlurStrength; \
         x/left => X; y/top => Y; text on Button/Label => Caption; text in TextBox \
         => Text. If no listed property matches, ask for clarification.\n",
    );

    let procs: Vec<&str> = form
        .user_procedures
        .iter()
        .map(|p| p.name.as_str())
        .collect();
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

/// Build the request context with both the current form API and the project tree
/// inventory. The project inventory is what lets the assistant discover indexed
/// files, generated/common COBOL, assets, documentation, and project-scoped
/// controls without the user spelling out exact filenames.
pub fn build_context_with_project(
    form: &Form,
    project: Option<&CoboltProject>,
    project_root: Option<&Path>,
) -> String {
    let mut out = build_context(form);
    out.push('\n');
    out.push_str(&build_project_tree_context(project, project_root));
    out
}

fn build_project_tree_context(
    project: Option<&CoboltProject>,
    project_root: Option<&Path>,
) -> String {
    let mut out = String::new();
    out.push_str("PROJECT TREE INVENTORY\n");
    out.push_str(
        "Use this inventory to discover project resources before proposing changes. \
         For CRUD forms over indexed files, inspect the INDEXED FILES section first. \
         If the request matches multiple resources, ask the user which one to use.\n",
    );

    let Some(project) = project else {
        out.push_str("  (no project is currently open)\n");
        return out;
    };

    let _ = writeln!(
        out,
        "PROJECT: {} version {} main {}",
        project.project.name, project.project.version, project.project.main
    );
    append_file_section(&mut out, "COMMON COBOL SOURCES", &project.files.sources);
    append_file_section(&mut out, "FORMS", &project.files.forms);
    append_indexed_section(&mut out, &project.files, project_root);
    append_file_section(&mut out, "GENERATED COBOL", &project.files.generated);
    append_file_section(&mut out, "ASSETS", &project.files.assets);
    append_file_section(&mut out, "DOCUMENTATION", &project.files.documentation);

    out.push_str("PROJECT USER CONTROLS:\n");
    if project.user_controls.is_empty() {
        out.push_str("  (none)\n");
    } else {
        for uc in &project.user_controls {
            let _ = writeln!(
                out,
                "  {} ({}x{}, {} child controls)",
                uc.name,
                uc.width,
                uc.height,
                uc.controls.len()
            );
        }
    }

    out
}

fn append_file_section(out: &mut String, title: &str, files: &[String]) {
    let _ = writeln!(out, "{title}:");
    if files.is_empty() {
        out.push_str("  (none)\n");
    } else {
        for rel in files {
            let _ = writeln!(out, "  - {rel}");
        }
    }
}

fn append_indexed_section(out: &mut String, files: &ProjectFiles, project_root: Option<&Path>) {
    out.push_str("INDEXED FILES (.cidx):\n");
    if files.indexed.is_empty() {
        out.push_str("  (none)\n");
        return;
    }

    for rel in &files.indexed {
        let _ = writeln!(out, "  - {rel}");
        let Some(abs) = project_root.map(|root| resolve_project_path(root, rel)) else {
            continue;
        };
        match cobolt_indexed::load_indexed(&abs) {
            Ok(def) => {
                let _ = writeln!(
                    out,
                    "      COBOL file name: {}; ASSIGN: {}; record length: {}",
                    def.name,
                    def.assign_path,
                    def.record_length()
                );
                if let Some(root) = def.record_root() {
                    let _ = writeln!(out, "      Record root: {}", root.name);
                }
                if !def.keys.primary.parts.is_empty() {
                    let _ = writeln!(
                        out,
                        "      Primary key: {}",
                        key_part_names(&def.keys.primary.parts)
                    );
                }
                if !def.keys.alternates.is_empty() {
                    let names: Vec<String> = def
                        .keys
                        .alternates
                        .iter()
                        .map(|k| k.name.clone().unwrap_or_else(|| key_part_names(&k.parts)))
                        .collect();
                    let _ = writeln!(out, "      Alternate keys: {}", names.join(", "));
                }
                let mut leaves = Vec::new();
                if let Some(root) = def.record_root() {
                    for leaf in root.all_leaves() {
                        leaves.push(format!(
                            "{} PIC {} offset {:?} length {:?}",
                            leaf.name, leaf.pic, leaf.offset, leaf.length
                        ));
                    }
                }
                if leaves.is_empty() {
                    out.push_str("      Fields: (none)\n");
                } else {
                    let visible: Vec<&str> = leaves.iter().take(24).map(String::as_str).collect();
                    let suffix = if leaves.len() > visible.len() {
                        format!(" ... +{} more", leaves.len() - visible.len())
                    } else {
                        String::new()
                    };
                    let _ = writeln!(out, "      Fields: {}{}", visible.join("; "), suffix);
                }
            }
            Err(err) => {
                let _ = writeln!(out, "      (definition could not be read: {err})");
            }
        }
    }
}

fn key_part_names(parts: &[cobolt_indexed::KeyPartDef]) -> String {
    parts
        .iter()
        .map(|p| p.field_name.as_str())
        .collect::<Vec<_>>()
        .join(" + ")
}

fn resolve_project_path(project_root: &Path, rel: &str) -> PathBuf {
    let path = Path::new(rel);
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        project_root.join(path)
    }
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
        assert!(matches!(
            cs.operations[2],
            AgentOp::GenerateEventHandler { .. }
        ));
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
        f.controls
            .push(Control::new("L1", ControlType::Label, 0, 0));
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
                    parent_id: None,
                    parent: None,
                    properties: Default::default(),
                },
                // error: unsupported control type
                AgentOp::DeployControl {
                    control_type: "Frobnicator".into(),
                    id: None,
                    parent_id: None,
                    parent: None,
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
        assert!(
            v[4].is_none(),
            "backgroundcolor is valid (case-insensitive)"
        );
        assert!(v[5].is_none(), "B1 was deployed earlier in the set");
        assert!(v[6].as_ref().unwrap().contains("no event"));
    }

    #[test]
    fn handler_code_flags_hallucinated_property() {
        let form = form_with_label();
        let handler = |code: &str| AgentChangeSet {
            operations: vec![AgentOp::GenerateEventHandler {
                control_id: "L1".into(),
                event: "onClick".into(),
                code: code.into(),
            }],
            note: None,
        };
        let body = |stmt: &str| {
            format!(
                "       ENVIRONMENT DIVISION.\n       DATA DIVISION.\n       PROCEDURE DIVISION.\n{stmt}\n"
            )
        };

        // Hallucinated property → error naming the control and property.
        let v = validate(&handler(&body("           MOVE 5 TO L1::Depth.")), &form);
        let msg = v[0].as_ref().expect("Depth should be flagged");
        assert!(
            msg.contains("Depth") && msg.contains("no property"),
            "{msg}"
        );

        // The real property is accepted (the depth-fix target).
        let v = validate(
            &handler(&body("           MOVE 5 TO L1::ShadowBlurStrength.")),
            &form,
        );
        assert!(
            v[0].is_none(),
            "ShadowBlurStrength must be valid: {:?}",
            v[0]
        );

        // Quoted property name is checked too.
        let v = validate(&handler(&body("           MOVE 5 TO L1::\"Nope\".")), &form);
        assert!(v[0].as_ref().is_some_and(|m| m.contains("Nope")));

        // A method / subscript call is not a property → never flagged.
        let v = validate(&handler(&body("           DISPLAY L1::GetText().")), &form);
        assert!(
            v[0].is_none(),
            "method call must not be flagged: {:?}",
            v[0]
        );

        // `::` inside a string literal or a comment is ignored.
        let v = validate(&handler(&body("           DISPLAY \"L1::Depth\".")), &form);
        assert!(
            v[0].is_none(),
            "string content must not be flagged: {:?}",
            v[0]
        );
        let v = validate(
            &handler(&body("           MOVE 5 TO L1::Depth.  *> L1::Zzz")),
            &form,
        );
        assert!(v[0].as_ref().is_some_and(|m| m.contains("Depth")));

        // A non-control receiver is left alone (no false positive on data items).
        let v = validate(&handler(&body("           MOVE WS-X::Foo TO WS-Y.")), &form);
        assert!(
            v[0].is_none(),
            "non-control receiver must not be flagged: {:?}",
            v[0]
        );
    }

    #[test]
    fn generated_code_must_keep_nested_body_divisions() {
        assert!(handler_body_shape_error(
            "       ENVIRONMENT DIVISION.\n       DATA DIVISION.\n       PROCEDURE DIVISION.\n           CONTINUE.\n"
        )
        .is_none());
        assert!(
            handler_body_shape_error("       PROCEDURE DIVISION.\n           CONTINUE.\n")
                .is_some_and(|m| m.contains("ENVIRONMENT"))
        );
        assert!(handler_body_shape_error(
            "       ENVIRONMENT DIVISION.\n       PROCEDURE DIVISION.\n           CONTINUE.\n"
        )
        .is_some_and(|m| m.contains("DATA DIVISION")));
    }

    #[test]
    fn context_lists_controls_and_legends() {
        let ctx = build_context(&form_with_label());
        assert!(ctx.contains("L1 (Label)"), "inventory: {ctx}");
        assert!(ctx.contains("PROPERTY KEYS BY TYPE (for all available controls):"));
        assert!(ctx.contains("Label:"));
        assert!(ctx.contains("EVENTS BY TYPE (for all available controls):"));
        assert!(ctx.contains("CONTROL API BY ID:"));
        assert!(ctx.contains("L1 (Label): properties ["));
        assert!(ctx.contains("PROPERTY INTENT MAP:"));
        assert!(ctx.contains("dropshadow"));
        assert!(ctx.contains("ShadowEnabled"));
        assert!(ctx.contains("PROCEDURES:"));
    }

    #[test]
    fn context_lists_project_tree_and_indexed_files() {
        let mut project = crate::project_model::CoboltProject::new("Demo", "src/main.cbl");
        project.files.sources.push("src/common.cbl".into());
        project.files.forms.push("forms/customer.cfrm".into());
        project.files.indexed.push("indexed/customer.cidx".into());
        project
            .files
            .generated
            .push("generated/customer.cbl".into());
        project.files.assets.push("assets/logo.png".into());
        project.files.documentation.push("docs/readme.md".into());
        project
            .user_controls
            .push(crate::project_model::UserControlDef {
                name: "AddressBlock".into(),
                width: 240,
                height: 80,
                controls: Vec::new(),
            });

        let ctx = build_context_with_project(&form_with_label(), Some(&project), None);
        assert!(ctx.contains("PROJECT TREE INVENTORY"), "inventory: {ctx}");
        assert!(ctx.contains("INDEXED FILES (.cidx):"), "inventory: {ctx}");
        assert!(ctx.contains("indexed/customer.cidx"), "inventory: {ctx}");
        assert!(ctx.contains("COMMON COBOL SOURCES:"), "inventory: {ctx}");
        assert!(ctx.contains("src/common.cbl"), "inventory: {ctx}");
        assert!(ctx.contains("PROJECT USER CONTROLS:"), "inventory: {ctx}");
        assert!(ctx.contains("AddressBlock"), "inventory: {ctx}");
    }

    #[test]
    fn deploy_control_accepts_structural_parent_and_tab_properties() {
        let reply = r#"```json
{ "operations": [
  { "op": "deploy_control", "control_type": "Button", "id": "B1",
    "properties": { "Caption": "OK", "X": 40, "Y": 80, "Parent": "Tab1", "Tab": 0 } }
] }
```"#;
        let cs = parse_change_set(reply).expect("structural deploy properties parse");
        let form = form_with_label();
        let v = validate(&cs, &form);
        assert_eq!(v, vec![None]);
    }
}
