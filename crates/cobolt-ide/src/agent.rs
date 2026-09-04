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

use serde::{Deserialize, Serialize};

// ── Change-set data model (T1) ───────────────────────────────────────────────

/// One structured operation the agent may propose. `op` is the JSON discriminator
/// (`deploy_control`, `set_property`, `generate_event_handler`, `create_procedure`).
/// Serialize + JsonSchema so a malformed submission can be recovered through
/// provider-native typed extraction (Rig migration, phase 3) and re-encoded
/// canonically.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum AgentOp {
    /// Deploy a new control onto the form.
    #[serde(alias = "add_control")]
    DeployControl {
        control_type: String,
        /// Control id. It becomes a COBOL word in the generated program
        /// (`WS-<id>-TEXT`, `<id>-OPEN`), so it may hold ONLY letters, digits
        /// and hyphens and may neither begin nor end with a hyphen:
        /// `TEXTBOX-1`, never `TEXTBOX_1`. Omit to let the designer name it.
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
    /// Write one of the form's raw-COBOL structure blocks — the five the COBOL
    /// Structure panel edits. The form is the main program of the generated
    /// nesting, so this is the ONLY way an agent can reach `SPECIAL-NAMES`
    /// (`DECIMAL-POINT IS COMMA` lives here and nowhere else) or declare the
    /// form-level GLOBAL working-storage its handlers share.
    SetFormStructure { block: String, code: String },
    /// Return a conversational message to the user.
    Message { message: String },
}

/// The five raw-COBOL blocks a `set_form_structure` operation may target,
/// matching the COBOL Structure panel's own sections. Returns the canonical
/// spelling, or `None` when the name is not one of them.
pub fn form_structure_block(name: &str) -> Option<&'static str> {
    let key: String = name
        .chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .collect::<String>()
        .to_ascii_uppercase();
    Some(match key.as_str() {
        "SPECIALNAMES" => "SPECIAL-NAMES",
        "REPOSITORY" => "REPOSITORY",
        "FILECONTROL" => "FILE-CONTROL",
        "FILESECTION" => "FILE SECTION",
        "WORKINGSTORAGE" => "WORKING-STORAGE",
        _ => return None,
    })
}

/// Read/write access to the `Form` field one structure block is stored in.
/// `WORKING-STORAGE` is `Form::user_ws_source`; the other four live on
/// `Form::cobol_structure`.
pub fn form_structure_field<'a>(form: &'a mut Form, block: &str) -> Option<&'a mut String> {
    Some(match form_structure_block(block)? {
        "SPECIAL-NAMES" => &mut form.cobol_structure.special_names,
        "REPOSITORY" => &mut form.cobol_structure.repository,
        "FILE-CONTROL" => &mut form.cobol_structure.file_control,
        "FILE SECTION" => &mut form.cobol_structure.file_section,
        "WORKING-STORAGE" => &mut form.user_ws_source,
        _ => return None,
    })
}

/// A parsed agent reply: an ordered list of operations, plus an optional note used
/// when the agent cannot express the request as operations.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct AgentChangeSet {
    pub operations: Vec<AgentOp>,
    #[serde(default)]
    pub note: Option<String>,
}

/// The sentence to SHOW once a handler reply's code has been applied.
///
/// The handler is already on screen in the editor above, and the surface
/// re-sends it as CURRENT HANDLER on every turn, so repeating it in the
/// balloon is noise — and when the workflow summary is a bare
/// ``Grace: ```cobol ENVIRONMENT DIVISION. DATA DIVISION. …`` it is noise that
/// says nothing at all (operator, 2026-09-04). Dropping it also stops the
/// transcript from carrying a second copy of the code into every later turn.
///
/// Keeps whatever prose the agent wrote AROUND its code; falls back to a plain
/// line when what remains is only a label ("Grace:") or punctuation.
pub fn readable_handler_answer(reply: &str, fallback: &str) -> String {
    // Fences can open mid-line ("Grace: ```cobol"), so split on the marker
    // itself rather than testing line starts: the even segments are outside.
    let outside = reply
        .split("```")
        .step_by(2)
        .collect::<Vec<_>>()
        .join(" ");
    let trimmed = outside.trim().trim_end_matches(':').trim().to_string();
    // A dozen letters is the line between "an answer" and "a label".
    if trimmed.chars().filter(|c| c.is_alphanumeric()).count() < 12 {
        return fallback.to_string();
    }
    trimmed
}

/// What the developer should READ, and the handler body to APPLY, from a reply
/// that may be a change-set rather than prose.
///
/// A Grace workflow puts the specialist's `{"operations": …}` block on the
/// reply so the surface that receives it can apply the work. The RAD designer
/// parses that JSON and never shows it; the COBOL Event Editor looked only for
/// a ```` ```cobol ```` block, found none, concluded "Grace answered in prose"
/// and displayed the raw JSON as the assistant's answer — machinery where a
/// sentence belonged (operator, 2026-09-04).
///
/// Returns the text to show and, when the change-set carries a handler for
/// exactly this control and event, its code.
pub fn event_handler_reply(
    reply: &str,
    control_id: &str,
    event: &str,
    fallback: &str,
) -> (String, Option<String>) {
    let Ok(cs) = parse_change_set(reply) else {
        // Not a change-set at all: prose, a question, or a plain code block —
        // all of which the caller already handles correctly.
        return (reply.to_string(), None);
    };
    let handlers = cs
        .operations
        .iter()
        .filter_map(|op| match op {
            AgentOp::GenerateEventHandler {
                control_id: c,
                event: e,
                code,
            } => Some((c.as_str(), e.as_str(), code.as_str())),
            _ => None,
        })
        .collect::<Vec<_>>();
    // The handler for THIS control and event, so a change-set touching several
    // can never drop another control's body into the open one.
    //
    // When nothing matches exactly but the change-set carries exactly ONE
    // handler, that is the one this surface asked for: an agent that spells
    // the event `Click` for `onClick` would otherwise have its work silently
    // discarded — which is what `extract_code` (first-handler-wins) never did.
    let code = handlers
        .iter()
        .find(|(c, e, _)| c.eq_ignore_ascii_case(control_id) && e.eq_ignore_ascii_case(event))
        .or(if handlers.len() == 1 {
            handlers.first()
        } else {
            None
        })
        .map(|(_, _, code)| (*code).to_string());
    // A `message` operation IS the agent talking to the developer — its own
    // contract says "If you must ask a question or explain, use the `message`
    // operation" — so it outranks everything else here.
    //
    // 1.64.27 read only the note and fell through to the fallback, which turned
    // a clarifying question into "Updated this handler.": a claim that work was
    // done, in place of the question that was actually asked (operator,
    // 2026-09-04: "why is grace no longer making questions to clarify the
    // request?"). Nothing was wrong with Grace.
    let messages = cs
        .operations
        .iter()
        .filter_map(|op| match op {
            AgentOp::Message { message } if !message.trim().is_empty() => Some(message.trim()),
            _ => None,
        })
        .collect::<Vec<_>>();
    let text = if !messages.is_empty() {
        messages.join("\n\n")
    } else {
        match cs.note.as_deref().map(str::trim) {
            Some(note) if !note.is_empty() => note.to_string(),
            _ => fallback.to_string(),
        }
    };
    (text, code)
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
        .map(|op| validate_op(op, &mut known, &form.name))
        .collect()
}

/// The operations `cs` would NOT apply to `form`, each named with its reason.
///
/// The apply path skips invalid operations by design, and until now it reported
/// only how many it applied — so a change-set whose handlers all named an event
/// the control does not have produced "applied 1 change" and no events, with
/// nothing to tell the developer why (operator report, 2026-07-31). Call this
/// BEFORE applying: the verdict depends on the form's pre-state.
pub fn discarded_ops(cs: &AgentChangeSet, form: &Form) -> Vec<String> {
    validate(cs, form)
        .iter()
        .zip(cs.operations.iter())
        .filter_map(|(err, op)| err.as_ref().map(|e| format!("{}: {e}", op_ref(op))))
        .collect()
}

fn is_form_id(form_name: &str, id: &str) -> bool {
    id.is_empty() || id.eq_ignore_ascii_case("Form") || id.eq_ignore_ascii_case(form_name)
}

/// Whether `key` names a settable form-level property.
///
/// Case-insensitive, and it must accept exactly the set the designer can apply
/// (`panels::designer::canonical_form_prop_key`): a key accepted here but not
/// applied there validates, is reported to the developer as applied, and changes
/// nothing. `form_property_lists_agree` holds the two in step.
pub(crate) fn form_property_valid(key: &str) -> bool {
    matches!(
        key.to_ascii_lowercase().as_str(),
        "title"
            | "backgroundcolor"
            | "width"
            | "height"
            | "transparency"
            | "gridsize"
            | "snaptogrid"
            | "glassstyle"
            | "backgroundgradientenabled"
            | "backgroundgradientstartcolor"
            | "backgroundgradientendcolor"
            | "backgroundgradientdirection"
            | "target"
            | "backgroundimage"
            | "bgimagemode"
            | "theme"
            | "usethemebackground"
            // 037 main form & window lifecycle
            | "mainform"
            | "taskbaricon"
            | "canminimize"
            | "canmaximize"
            | "windowstate"
            | "fullscreen"
            | "titlevisible"
            // 038 window effects opt-out
            | "windoweffects"
            // 049 application shell
            | "formformat"
            | "menupanecustom"
            | "menupanecolor"
            | "menupanegradientenabled"
            | "menupanegradientstartcolor"
            | "menupanegradientendcolor"
            | "menupanegradientdirection"
            | "menupanetransparency"
            | "menupaneimage"
            | "menupaneimagemode"
            // Window start position
            | "x"
            | "y"
            | "startposition"
    )
}

/// The Form-independent half of [`validate_op`]: defects provable from the
/// operation alone, with no control inventory. Shared with the workflow lint
/// gate ([`lint_change_set_submission`]) so the two can never disagree — an
/// operation this rejects is exactly one the apply path would silently skip.
/// Every control type the designer can deploy. One list, used to build the
/// context block AND to prove an event name wrong without a form.
pub(crate) const ALL_CONTROL_TYPES: [&str; 35] = [
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
    "IndexedFile",
    "Slider",
    "BarChart",
    "LineChart",
    "PieChart",
    "AreaChart",
    "ScatterChart",
    "DonutChart",
];

/// The real event names that resemble `event` — what the agent probably meant.
/// `onFocus` finds `onGotFocus`/`onLostFocus`; `keyboard` finds nothing, which
/// is itself the answer.
fn event_name_suggestions(event: &str) -> Vec<&'static str> {
    let needle = event.trim().to_ascii_lowercase();
    let stem = needle.strip_prefix("on").unwrap_or(&needle);
    if stem.len() < 3 {
        return Vec::new();
    }
    let mut out: Vec<&'static str> = Vec::new();
    for ty in ALL_CONTROL_TYPES {
        for e in ControlType::from_str(ty).supported_events() {
            let lower = e.to_ascii_lowercase();
            if lower.contains(stem) && !out.contains(e) {
                out.push(e);
            }
        }
    }
    out.truncate(4);
    out
}

/// Whether ANY control type supports `event`.
fn event_exists_anywhere(event: &str) -> bool {
    ALL_CONTROL_TYPES.iter().any(|ty| {
        ControlType::from_str(ty)
            .supported_events()
            .iter()
            .any(|e| e.eq_ignore_ascii_case(event.trim()))
    })
}

pub(crate) fn op_form_free_error(op: &AgentOp) -> Option<String> {
    match op {
        AgentOp::DeployControl {
            control_type,
            properties,
            ..
        } => {
            let ct = ControlType::from_str(control_type);
            if matches!(ct, ControlType::Custom { .. }) {
                return Some(format!("Unknown control type '{control_type}'."));
            }
            // Property keys, if any, must be valid for the new control's type.
            properties.keys().find_map(|key| {
                (!deploy_property_valid(&ct, key))
                    .then(|| format!("'{control_type}' has no property '{key}'."))
            })
        }
        AgentOp::GenerateEventHandler { event, code, .. } => {
            // An event name no control type has is provably wrong whatever
            // control it targets — and it is the failure the workflow used to
            // discover one name at a time, a reviewer round each, or (before
            // the reviewer had the legend) not at all: the operation was
            // dropped at apply and no handler appeared. Naming every bad name
            // at once costs one round instead of three.
            if !event_exists_anywhere(event) {
                let hint = match event_name_suggestions(event).as_slice() {
                    [] => String::new(),
                    names => format!(" Did you mean {}?", names.join(", ")),
                };
                return Some(format!(
                    "No control type has an event '{event}'.{hint} Use the exact \
                     name from EVENTS BY TYPE in your context."
                ));
            }
            handler_body_shape_error(code)
        }
        AgentOp::CreateProcedure { name, code } => {
            if name.trim().is_empty() {
                return Some("Procedure name is empty.".to_string());
            }
            handler_body_shape_error(code)
        }
        AgentOp::SetFormStructure { block, code } => {
            if form_structure_block(block).is_none() {
                return Some(format!(
                    "'{block}' is not a form structure block. Use one of \
                     SPECIAL-NAMES, REPOSITORY, FILE-CONTROL, FILE SECTION, \
                     WORKING-STORAGE."
                ));
            }
            // The block is woven into the form — the OUTERMOST program — so the
            // division/section header belongs to codegen, not to the agent, in
            // exactly the way a handler body's scaffold does.
            let upper = code.to_ascii_uppercase();
            for header in [
                "IDENTIFICATION DIVISION",
                "ENVIRONMENT DIVISION",
                "DATA DIVISION",
                "PROCEDURE DIVISION",
                "CONFIGURATION SECTION",
            ] {
                if upper.contains(header) {
                    return Some(format!(
                        "Remove '{header}' — a structure block is woven into the \
                         form's own division; write only the block's contents."
                    ));
                }
            }
            None
        }
        AgentOp::SetProperty { .. } | AgentOp::Message { .. } => None,
    }
}

/// One-line label naming an operation in a validation message.
fn op_ref(op: &AgentOp) -> String {
    match op {
        AgentOp::DeployControl {
            control_type, id, ..
        } => format!(
            "deploy_control {control_type} {}",
            id.as_deref().unwrap_or("(auto id)")
        ),
        AgentOp::SetProperty {
            control_id, key, ..
        } => format!("set_property {control_id}.{key}"),
        AgentOp::GenerateEventHandler {
            control_id, event, ..
        } => format!("generate_event_handler {control_id}.{event}"),
        AgentOp::CreateProcedure { name, .. } => format!("create_procedure {name}"),
        AgentOp::SetFormStructure { block, .. } => format!("set_form_structure {block}"),
        AgentOp::Message { .. } => "message".into(),
    }
}

/// Split a defective change-set submission into the operations that stand and
/// the ones to redo, each serialized as its own change-set block.
///
/// `defect_refs` are the operation references a Pedantic reviewer attributed
/// its findings to (matched case-insensitively against [`op_ref`], and
/// tolerating a reviewer that names only the control — `txt8` matches
/// `generate_event_handler txt8.onChange`). When it is empty the machine
/// validator decides, since it can PROVE which operations are bad.
///
/// `None` when the submission has no parseable change-set, or when the split
/// would be degenerate (nothing defective, or nothing left to keep) — the
/// caller then falls back to asking for a full replacement.
pub fn split_change_set_submission(
    agent: &str,
    submission: &str,
    defect_refs: &[String],
) -> Option<(String, String, usize, usize)> {
    if !crate::agents_db::produces_form_change_set(agent) {
        return None;
    }
    let cs = parse_change_set(submission).ok()?;
    let attributed = |op: &AgentOp| -> bool {
        let r = op_ref(op).to_ascii_lowercase();
        defect_refs.iter().any(|d| {
            let d = d.trim().to_ascii_lowercase();
            !d.is_empty() && (r.contains(&d) || d.contains(&r))
        })
    };
    let (defective, accepted): (Vec<&AgentOp>, Vec<&AgentOp>) = cs.operations.iter().partition(
        |op| {
            if defect_refs.is_empty() {
                op_form_free_error(op).is_some()
            } else {
                attributed(op)
            }
        },
    );
    if defective.is_empty() || accepted.is_empty() {
        return None;
    }
    Some((
        change_set_block(&accepted)?,
        change_set_block(&defective)?,
        accepted.len(),
        defective.len(),
    ))
}

/// Merge a scoped correction back onto the kept operations: a corrected
/// operation SUPERSEDES the accepted one it targets (so a specialist that
/// ignored the "only the corrected ones" instruction and resubmitted
/// everything still merges cleanly), and a corrected `deploy_control` goes
/// first, since later operations may reference the control it creates.
pub fn merge_change_sets(accepted: &str, corrected_reply: &str) -> Option<String> {
    let kept = parse_change_set(accepted).ok()?;
    let fixed = parse_change_set(corrected_reply).ok()?;
    if fixed.operations.is_empty() {
        return None;
    }
    let superseded = |op: &AgentOp| -> bool {
        let r = op_ref(op);
        fixed.operations.iter().any(|f| op_ref(f) == r)
    };
    let mut merged: Vec<&AgentOp> = fixed
        .operations
        .iter()
        .filter(|op| matches!(op, AgentOp::DeployControl { .. }))
        .collect();
    merged.extend(kept.operations.iter().filter(|op| !superseded(op)));
    merged.extend(
        fixed
            .operations
            .iter()
            .filter(|op| !matches!(op, AgentOp::DeployControl { .. })),
    );
    change_set_block(&merged)
}

/// Serialize operations as the fenced change-set block the apply path reads.
fn change_set_block(ops: &[&AgentOp]) -> Option<String> {
    let json = serde_json::to_string_pretty(&serde_json::json!({ "operations": ops })).ok()?;
    Some(format!("```json\n{json}\n```"))
}

/// Machine validation of a specialist's change-set submission for the Grace
/// workflow lint gate. Runs only the **Form-independent** checks (see
/// [`op_form_free_error`]): those can never false-positive against a designer
/// holding unsaved changes, and every defect they prove is an operation the
/// apply-time validator would silently discard. Returns the numbered defect
/// list, or `None` when the agent's output is not a change-set, the change-set
/// does not parse deterministically (parse recovery is a separate, later
/// concern — see `normalize_form_change_sets`), or nothing is wrong.
pub fn lint_change_set_submission(agent: &str, submission: &str) -> Option<String> {
    if !crate::agents_db::produces_form_change_set(agent) {
        return None;
    }
    let cs = parse_change_set(submission).ok()?;
    let errors: Vec<String> = cs
        .operations
        .iter()
        .filter_map(|op| op_form_free_error(op).map(|e| format!("{}: {e}", op_ref(op))))
        .enumerate()
        .map(|(i, line)| format!("{}. {line}", i + 1))
        .collect();
    (!errors.is_empty()).then(|| errors.join("\n"))
}

fn validate_op(op: &AgentOp, known: &mut HashMap<String, ControlType>, form_name: &str) -> Option<String> {
    match op {
        AgentOp::DeployControl {
            control_type, id, ..
        } => {
            if let Some(error) = op_form_free_error(op) {
                return Some(error);
            }
            if let Some(id) = id {
                known.insert(id.to_ascii_uppercase(), ControlType::from_str(control_type));
            }
            None
        }
        AgentOp::SetProperty {
            control_id, key, ..
        } => {
            if is_form_id(form_name, control_id) {
                if !form_property_valid(key) {
                    Some(format!("Form has no property '{key}'."))
                } else {
                    None
                }
            } else {
                match known.get(&control_id.to_ascii_uppercase()) {
                    None => Some(format!("No control named '{control_id}'.")),
                    Some(ct) if !property_valid(ct, key) => {
                        Some(format!("Control '{control_id}' has no property '{key}'."))
                    }
                    _ => None,
                }
            }
        }
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
            base.or_else(|| op_form_free_error(op))
                .or_else(|| unknown_property_ref(code, known).map(bad_prop_msg))
        }
        AgentOp::CreateProcedure { name: _, code } => op_form_free_error(op)
            .or_else(|| unknown_property_ref(code, known).map(bad_prop_msg)),
        AgentOp::SetFormStructure { .. } => op_form_free_error(op),
        AgentOp::Message { .. } => None,
    }
}

// ── Grid & alignment normalisation ───────────────────────────────────────────

/// Snap `v` to the NEAREST multiple of `grid_px`, half-steps rounded away from
/// zero so the step stays symmetric either side of the origin. `grid_px <= 0`
/// leaves the value alone.
pub fn snap_nearest(v: i32, grid_px: i32) -> i32 {
    if grid_px <= 0 {
        return v;
    }
    let half = grid_px / 2;
    if v >= 0 {
        ((v + half) / grid_px) * grid_px
    } else {
        -(((-v + half) / grid_px) * grid_px)
    }
}

/// Read a coordinate the agent wrote as a number or as a numeric string —
/// matching what the applier's own `json_prop_i32` accepts, so this normalises
/// exactly the values that go on to take effect.
fn coord(v: &serde_json::Value) -> Option<i32> {
    v.as_i64()
        .or_else(|| v.as_str().and_then(|s| s.trim().parse::<i64>().ok()))
        .map(|n| n as i32)
}

/// One axis of a change-set's placement, resolved in two steps.
///
/// **Lanes** collapse coordinates that were meant to be one: the first control
/// to use a coordinate opens a lane, and anything within `tol` of it is that
/// same position, not a near-miss. This is what keeps a column a column.
///
/// **One delta** then moves every lane together. It is the shift that puts the
/// FIRST lane exactly on the grid, so the run is translated rather than
/// quantised: every distance the agent asked for — row pitch, column gap — is
/// preserved to the pixel, and the later lanes sit wherever that leaves them.
#[derive(Default)]
struct Axis {
    /// Raw lane anchors, in the order the change-set opened them.
    lanes: Vec<i32>,
    /// Shift derived from the first anchor; every coordinate takes this one.
    delta: Option<i32>,
}

impl Axis {
    fn place(&mut self, raw: i32, grid_px: i32, tol: i32) -> i32 {
        let anchor = match self.lanes.iter().copied().find(|a| (raw - a).abs() <= tol) {
            Some(a) => a,
            None => {
                self.lanes.push(raw);
                raw
            }
        };
        let delta = *self
            .delta
            .get_or_insert_with(|| snap_nearest(anchor, grid_px) - anchor);
        anchor + delta
    }
}

/// Put agent-placed geometry on the designer grid without breaking an
/// alignment the agent asked for.
///
/// Two things go wrong when agent geometry is quantised coordinate by
/// coordinate, and this fixes both by never quantising more than once per axis.
///
/// **Alignments split.** `X=19` and `X=21` are 2px apart before snapping and a
/// whole cell apart after, so the column the agent asked for comes out crooked.
/// Coordinates within half a cell of each other are therefore one lane — one
/// position, not a near-miss.
///
/// **Even spacing goes lumpy.** A 30px row pitch cannot be expressed on an 8px
/// grid, so snapping each row lands them 24, 32, 32 apart. Instead the run is
/// TRANSLATED: the shift that puts the first lane exactly on the grid is
/// applied to every lane, so a 30px pitch stays 30px and a 180px column gap
/// stays 180px.
///
/// The trade this makes, deliberately: only the first placement of each axis
/// lands on a grid point. Later ones sit wherever the requested distance from
/// it puts them, and a coordinate that was already on the grid can be carried
/// off it — that is the cost of keeping the agent's own spacing exact.
///
/// With the grid off (`snap_enabled` false) nothing moves at all.
pub fn normalize_geometry(cs: &AgentChangeSet, grid_px: i32, snap_enabled: bool) -> AgentChangeSet {
    let mut out = cs.clone();
    if !snap_enabled || grid_px <= 0 {
        return out;
    }
    let tol = (grid_px / 2).max(1);
    // [X, Y].
    let mut axes: [Axis; 2] = [Axis::default(), Axis::default()];
    for op in &mut out.operations {
        match op {
            AgentOp::DeployControl { properties, .. } => {
                for (axis, key) in [(0usize, "X"), (1usize, "Y")] {
                    let Some(raw) = properties.get(key).and_then(coord) else {
                        continue;
                    };
                    let placed = axes[axis].place(raw, grid_px, tol);
                    properties.insert(key.to_string(), serde_json::Value::from(placed));
                }
            }
            AgentOp::SetProperty { key, value, .. } => {
                let axis = match key.as_str() {
                    "X" => 0usize,
                    "Y" => 1,
                    _ => continue,
                };
                let Some(raw) = coord(value) else { continue };
                *value = serde_json::Value::from(axes[axis].place(raw, grid_px, tol));
            }
            _ => {}
        }
    }
    out
}

/// Ensure an agent-authored handler/procedure is the IDE-owned nested-program body,
/// not a partial fragment. The generator supplies IDENTIFICATION/PROGRAM-ID and
/// the footer, but the editable body must keep the three divisions so the model
/// cannot silently drop DATA/ENVIRONMENT during a round-trip.
pub(crate) fn handler_body_shape_error(code: &str) -> Option<String> {
    // A division header may carry a phrase — `PROCEDURE DIVISION USING
    // KEY-CODE.` is how every event with a payload is written — and may end
    // with an inline `*>` comment. Matching the whole line for equality
    // rejected those handlers with "Code must include PROCEDURE DIVISION.",
    // an instruction the agent had already followed: it rewrote the same
    // code, was rejected again, and the workflow burned its correction
    // budget without ever creating anything (operator report, 2026-07-30).
    let has_line = |division: &str| {
        code.lines().any(|l| {
            let text = l.split("*>").next().unwrap_or("").trim().to_ascii_uppercase();
            text.starts_with(division) && text.ends_with('.')
        })
    };
    if !has_line("ENVIRONMENT DIVISION") {
        return Some(
            "Code must start from the nested-program body and include \
             ENVIRONMENT DIVISION."
                .to_string(),
        );
    }
    if !has_line("DATA DIVISION") {
        return Some(
            "Code must include DATA DIVISION.; do not return a PROCEDURE-only \
             fragment."
                .to_string(),
        );
    }
    if !has_line("PROCEDURE DIVISION") {
        return Some(
            "Code must include PROCEDURE DIVISION. (a USING phrase is fine: \
             `PROCEDURE DIVISION USING KEY-CODE.`)"
                .to_string(),
        );
    }
    // The program wrapper is the IDE's: a body that writes it too declares a
    // second program inside the nest and the parser rejects it.
    //
    // `GOBACK` is NOT part of that ban. It is an ordinary COBOL-85 statement
    // and the only way to end a handler's main flow before the paragraphs it
    // declares for its own `PERFORM`s — without one, control falls through and
    // runs them a second time. It used to be banned because the lexer had no
    // `GOBACK` keyword, so a lone `GOBACK.` was read as a paragraph name and
    // collided with the generated closing one ("paragraph 'GOBACK' is declared
    // more than once", operator 2026-07-31). The keyword exists now, so the
    // collision cannot happen and the statement is legitimate.
    for line in code.lines() {
        let text = line
            .split("*>")
            .next()
            .unwrap_or("")
            .trim()
            .trim_end_matches('.')
            .trim()
            .to_ascii_uppercase();
        // The wrappers are caught by their opening words, which carry a name.
        let word = ["IDENTIFICATION DIVISION", "PROGRAM-ID", "END PROGRAM"]
            .into_iter()
            .find(|w| text.starts_with(w));
        if let Some(word) = word {
            return Some(format!(
                "Remove `{word}` from the body: the IDE generates IDENTIFICATION \
                 DIVISION, PROGRAM-ID, the closing GOBACK and END PROGRAM around \
                 it. A body that writes them too declares a second program inside \
                 the nest, and the form no longer compiles."
            ));
        }
    }
    None
}

/// The line of the first `EXEC RUST` block in `code` that `request` never asked
/// for, 1-based — `None` when there is no block, or when the developer did ask.
///
/// `EXEC RUST` is the developer's choice (see the agents' steering and the
/// extensions skill). The prompts say so, but a prompt is instruction and not
/// enforcement: asked to copy one value into fifteen controls, an agent still
/// answered with a Rust block and called it concise (operator, 2026-08-16).
/// This is the part that cannot be talked out of it.
///
/// "Asked for it" is deliberately generous — any mention of Rust in the request
/// counts, so a developer who wants a block gets one without fighting the
/// checker. The one word that must NOT count is the platform's own name: this
/// language is called RustCOBOL, so a request mentioning `RustCOBOL` (or
/// `PowerRustCOBOL`) is talking about the product, not asking for Rust.
pub fn unrequested_exec_rust(code: &str, request: &str) -> Option<u32> {
    let asked_for_rust = {
        let mut r = request.to_lowercase();
        for product_name in ["powerrustcobol", "rustcobol"] {
            r = r.replace(product_name, " ");
        }
        r.contains("rust")
    };
    if asked_for_rust {
        return None;
    }
    code.lines().enumerate().find_map(|(i, line)| {
        let code_part = line.split("*>").next().unwrap_or("");
        code_part
            .to_ascii_uppercase()
            .contains("EXEC RUST")
            .then_some(i as u32 + 1)
    })
}

/// What the developer is told when a block they never asked for comes back.
pub fn unrequested_exec_rust_msg() -> String {
    "This handler contains an `EXEC RUST` block, and the request did not ask for \
     Rust. Write it in COBOL: however many statements that takes is the correct \
     answer — fifteen controls are fifteen MOVE statements. A block is not free \
     either: it forces the program to be built, needs the Rust toolchain, and \
     cannot be stepped in the debugger. If the task genuinely cannot be done in \
     COBOL, say so and ask instead of choosing for the developer."
        .to_string()
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
/// calls are ignored.
///
/// Reading is judged by `property_readable`, not `property_valid`: a handler
/// legitimately reads what the runtime delivered (`Maps-1::ResponseBody`) even
/// though nothing can *set* it in the designer.
fn unknown_property_ref(
    code: &str,
    known: &HashMap<String, ControlType>,
) -> Option<(String, String)> {
    for r in scan_member_refs(code) {
        if r.is_call {
            continue; // methods handled elsewhere
        }
        if let Some(ct) = known.get(&r.recv.to_ascii_uppercase()) {
            if !property_readable(ct, &r.member) {
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

/// A property key a handler may **read**: everything [`property_valid`] accepts,
/// plus the ones the runtime delivers (`ResponseBody`, `SelectedMarkerId`, …).
///
/// Reading and writing are not the same question. `ResponseBody` cannot be set —
/// it is an answer, not a setting — but reading it is the only way a handler
/// ever sees what `Directions` or a `RestClient` verb returned, so judging a read
/// by the *settable* list rejected every correct async handler there is.
fn property_readable(ct: &ControlType, key: &str) -> bool {
    property_valid(ct, key)
        || cobolt_forms::model::runtime_property_names_for(ct.as_str())
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
const STEERING_FILE: &str = "steering.md";
pub const FORM_DESIGNER_AGENT_DIR: &str = "form-designer-agent";
pub const EVENT_HANDLER_AGENT_DIR: &str = "cobol-event-handler-agent";
pub const RUSTCOBOL_SKILL_FILE: &str = "rustcobol-extensions.md";
pub const FORM_DESIGNER_SKILL_FILE: &str = "form-designer.md";
pub const EVENT_HANDLER_SKILL_FILE: &str = "event-handler.md";
pub const MAPS_SKILL_FILE: &str = "maps.md";

const FORM_DESIGNER_STEERING: &str = r#"# Form Designer Agent Steering

- Build form changes as structured operations only; do not describe changes that are not present in the JSON change-set.
- Use the supplied project inventory before claiming a file, form, indexed file, control, data item, property, or event does not exist.
- Use exact control property names from the supplied schema. If the user uses a friendly name, map it to the real property before emitting an operation.
- Prefer inline PowerRustCOBOL object syntax for generated COBOL: `<control>::<method>(...)` and `<control>::<property>`.
- Write COBOL. Never emit an `EXEC RUST` block unless the developer asked for Rust in so many words. Repetition is not a reason: fifteen `MOVE` statements are the correct answer to fifteen controls.
- Never remove required COBOL divisions from generated handlers. If the correct change is unclear after validation feedback, ask the developer for directions.
"#;

const EVENT_HANDLER_STEERING: &str = r#"# COBOL Event Handler Script Agent Steering

- Return a complete event-handler body only when the user asks to write or change code.
- The editable body must include `ENVIRONMENT DIVISION.`, `DATA DIVISION.`, and `PROCEDURE DIVISION.`.
- Do not return `IDENTIFICATION DIVISION`, `PROGRAM-ID`, `GOBACK`, or `END PROGRAM`; the IDE owns that scaffold.
- Preserve existing declarations and code unless the user explicitly asks to change them.
- Use inline PowerRustCOBOL object syntax: `<control>::<method>(...)` and `<control>::<property>`. Do not use `CALL` for control methods or properties.
- Write COBOL. Never emit an `EXEC RUST` block unless the developer asked for Rust in so many words. Repetition is not a reason: fifteen `MOVE` statements are the correct answer to fifteen controls.
- If a property, method, data item, or intended behavior cannot be determined, ask the developer for directions instead of guessing.
"#;

const FORM_DESIGNER_SKILL: &str = r#"# Form Designer Agent Skill

The Form Designer Agent receives:

- A project tree inventory including forms, generated COBOL, indexed files, common code, assets, and documentation.
- The current form with controls, events, properties, methods, and data-binding information.
- A schema of valid operations: deploy controls, set properties, generate event handlers, create procedures, or answer with a message.

When creating controls inside containers, always set the correct parent/container target in the operation. For TabControl pages, target the requested tab page rather than the TabControl shell.

For property requests, use exact PowerRustCOBOL property names. Examples:

- dropshadow, drop shadow, shadow on -> `ShadowEnabled`
- selected tab color, active tab color -> `SelectedTabColor`
- icon image path -> `IconPath`

For indexed-file workflows, inspect the project indexed-file inventory and use IndexedFile controls and their methods when available.
"#;

const EVENT_HANDLER_SKILL: &str = r#"# COBOL Event Handler Script Agent Skill

Generate event-handler COBOL that is valid in the PowerRustCOBOL nested-program body edited by the IDE.

Expected shape:

```cobol
       ENVIRONMENT DIVISION.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 WS-VALUE PIC X(80).
       PROCEDURE DIVISION.
           *> code here
```

Use inline control access:

```cobol
       SET Button-1::Caption TO "Save".
       SET Panel-1::ShadowEnabled TO 1.
       TextBox-1::SetFocus().
```

Do not use `CALL` for a control's properties or methods — those are reached with `::` only.

`CALL` is, however, how one program reaches another, and every handler is a program. The form is the OUTERMOST program of a COBOL-85 nest; each event handler and each common procedure is a separate nested program inside it:

- A common procedure (`create_procedure`) is a nested program, NOT a paragraph of your body. Invoke it with `CALL "ITS-NAME"` — `CALL "VALIDATE-INPUT".`, `CALL "RECALC-TOTAL" USING WS-QTY WS-PRICE.`. `PERFORM` can never reach it.
- `PERFORM` reaches only a paragraph or section declared in the SAME body you are writing. A `PERFORM` naming anything outside it is a compile error.
- End your main flow with `GOBACK.` before any paragraph you declare, or control falls through and runs that paragraph a second time.
"#;

/// How to actually build a map, as opposed to what its properties are called.
///
/// The property/method reference reaches the agents through the System
/// Knowledge Base already. What it cannot teach is the SHAPE of a working
/// solution — which half needs a credential and which does not, that the data
/// methods answer on an event rather than returning, and that a distance is
/// two numbers and a string rather than one string. Every agent that got this
/// wrong got it wrong the same way, so the recipe is written down once here.
const MAPS_SKILL: &str = r##"# Maps Skill — building a real map solution

## The one thing to get right first

The Maps control is **two independent halves with different credential needs**:

| Half | Needs a key? | What it does |
|------|--------------|--------------|
| Basemap, markers, routes, regions | **No, never** | Draws OpenStreetMap tiles and whatever geometry the program holds |
| `Geocode`, `ReverseGeocode`, `Directions`, `DistanceMatrix`, `PlacesSearch` | **Yes** — Google Maps key in Settings → Integrations | Asks Google a question |

Build the drawing half FIRST. It works on a machine with nothing configured,
which is where most demos and most tests run. Never tell a developer they need
an API key to put a pin on a map, or to draw a route or a territory — they do
not.

## The data methods do NOT return the answer

All five are **asynchronous**. They return an EMPTY string immediately, set
`Busy` to `1`, and the answer arrives later on `onComplete` in `ResponseBody`.
There is no synchronous mode. This does not work, however much it reads like it
should:

```cobol
      *> WRONG — Directions returns "" here, always.
           MOVE MAP-1::Directions("Madrid", "Granada") TO WS-ANSWER
```

Write the call in one handler and the answer in `onComplete`:

```cobol
      *> In the button's onClick:
           INVOKE MAP-1 "Directions" USING "Madrid, Spain" "Granada, Spain"

      *> In the map's onComplete — SEVEN tab-separated fields:
      *>   text distance, text duration, summary, METRES, SECONDS, polyline,
      *>   and SECONDS WITH CURRENT TRAFFIC (0 when Google supplied none).
           UNSTRING MAP-1::ResponseBody DELIMITED BY X"09"
               INTO WS-DIST-TEXT WS-TIME-TEXT WS-SUMMARY
                    WS-METERS WS-SECONDS WS-POLYLINE WS-TRAFFIC-SECS
           COMPUTE WS-KM   = WS-METERS / 1000
           COMPUTE WS-COST = WS-KM * 0.62

      *> Prefer the traffic figure when there is one: it is the honest answer
      *> to "how long will this take, leaving now".
           IF WS-TRAFFIC-SECS > 0
               COMPUTE WS-MINUTES = WS-TRAFFIC-SECS / 60
           ELSE
               COMPUTE WS-MINUTES = WS-SECONDS / 60
           END-IF
```

Traffic is available as a NUMBER only. Google exposes its traffic *layer* through
its own JavaScript and mobile SDKs, never as map tiles, so there is no coloured
overlay to draw and asking for one is a dead end — but the drive time with
current traffic is right there in the last field, and a number is what a business
program can act on anyway.

`WS-METERS` and `WS-SECONDS` are the point. The text fields are for showing; the
numbers are what a business program computes with. Never parse `"72,4 km"` to
get a number back out of it — the number is already there, in the next field.

With no key configured the call fails on **`onError`** with `LastError`
explaining it. It never attempts a request, so handle `onError` and say what is
missing rather than leaving the form silent.

## Drawing: markers, routes, regions

Three collections, all the same shape — one TAB-separated record per line in a
string property — and all with the same rule: **re-using an id REPLACES that
record**, so a map that redraws itself as its data changes does not accumulate
invisible duplicates.

```cobol
      *> Pins. label shows on hover, info in the click card.
           INVOKE MAP-1 "AddMarker" USING
               "ANA" "40.4168" "-3.7038" "Ana - Centro" "27 accounts"

      *> A traced line. Geometry is EITHER an encoded polyline (exactly what
      *> Directions returned in its sixth field, so Google's own route traces
      *> with no conversion) OR an explicit lat,lng list you computed.
           INVOKE MAP-1 "AddRoute" USING "PLANNED" "#1E6EDC" "5"
               "40.4168,-3.7038;38.99,-3.37;37.1773,-3.5986"
           INVOKE MAP-1 "AddRoute" USING "DRIVEN" "#12A150" "6" WS-POLYLINE

      *> A filled territory. The fill takes an ALPHA (#RRGGBBAA) so the streets
      *> stay readable under it. The ring closes itself and MAY be concave.
           INVOKE MAP-1 "AddRegion" USING "NORTE" "#E5484D55" "#E5484D" "2"
               "43.79,-7.87;43.55,-5.66;42.60,-6.50;42.40,-8.87"
               "Norte - Elena" "18 accounts - 1.24M EUR YTD"
```

Also: `RemoveMarker`/`RemoveRoute`/`RemoveRegion` by id, and
`ClearRoutes`/`ClearRegions`.

## Positioning the view

`CenterLat`, `CenterLng` and `Zoom` are plain properties. Writing them moves the
map; the developer panning or zooming writes them back and fires
`onBoundsChanged`.

```cobol
           MOVE "40.0000" TO MAP-1::CenterLat
           MOVE "-3.7000" TO MAP-1::CenterLng
           MOVE 6         TO MAP-1::Zoom
```

Latitude and longitude are **strings**, not numerics — they carry more decimal
places than a PIC 9 would keep.

## The info window

Hovering a marker or region shows its `label`; clicking opens a card with the
`info` under it; clicking bare map closes the card. That is automatic — supply
`label`/`info` and it happens.

`SelectedMarkerId` / `SelectedRegionId` hold whichever card is open (write them
to open or close one from COBOL). `onMarkerHover`/`onRegionHover` fire beside
the native window, with `HoveredMarkerId`/`HoveredRegionId`, for a form that
wants to build its own panel or fetch something on hover.

Restyle it with `InfoBackgroundColor`, `InfoForegroundColor`,
`InfoBorderColor`, `InfoCornerRadius`, `InfoShadow`. Leave them EMPTY and the
window follows the form — which is the right default; do not set them unless
the developer asked for a specific look.

**Do not set `InfoForegroundColor` to "make it readable".** Left empty, the text
colour is derived from whichever background the window ended up with — black or
white, whichever contrasts more — so it is legible on any card. Setting it by
hand REPLACES that guarantee with your guess, which is how the window ended up
white-on-light in the first place.

## Checklist before claiming a map solution is done

- Does anything visual depend on an API key? It must not.
- Is every data-method result read in `onComplete`, never from the call?
- Is `onError` handled, so a missing key explains itself?
- Are distances computed from the METRES field, not parsed from text?
- Does redrawing re-use ids, so nothing accumulates?
"##;

const RUSTCOBOL_SKILL: &str = r#"# PowerRustCOBOL Extensions Skill

PowerRustCOBOL extends COBOL-85 with inline form/control access:

- Get a property with `<control>::<property>`.
- Set a property with `SET <control>::<property> TO <value>`.
- Invoke a method with `<control>::<method>(<parameters>)`.

## What a handler is, and how code reaches other code

A form is one compilation unit: the form itself is the OUTERMOST program, and every event handler and every common procedure is a separate NESTED program inside it. That structure decides which verb reaches what.

- `CALL "NAME"` is the only way to reach another program — that includes every common procedure created with `create_procedure`. Write `CALL "UPDATE-TOTAL".` or `CALL "RECALC" USING WS-QTY WS-PRICE.`.
- `PERFORM` reaches only a paragraph or section declared in the same body you are writing, and never crosses a program boundary. A `PERFORM` naming a procedure of another program is a compile error, not a style preference.
- The generated infrastructure paragraphs (`<id>-OPEN`, `<id>-READ-NEXT`, the timer, chart, CSV-export and data-binding helpers) live in the OUTER program, so form-level code may `PERFORM` them but a handler may not. From a handler, use the control's `::` methods.
- Do not use `CALL` for a control's own properties or methods — `::` is the only form for those.

## `EXEC RUST` is the developer's choice, never yours

The language of this platform is COBOL. `EXEC RUST` exists so a developer who
WANTS Rust — for a crate, an algorithm, something COBOL genuinely cannot reach —
can have it. It is not a shortcut for code you find repetitive.

Emit an `EXEC RUST` block ONLY when the developer asked for Rust in so many
words ("in Rust", "use EXEC RUST", "with the csv crate"). Absent that, write
COBOL, however long it comes out. Setting fifteen controls is fifteen `MOVE`
statements, and that is the CORRECT answer — not a reason to reach for Rust.

Never justify a block by concision, readability, elegance, or "the platform
supports it". The platform supporting a thing is not the developer asking for
it. A block also changes what the developer gets: a program with `EXEC RUST`
must be BUILT before it runs, needs the Rust toolchain installed, and cannot be
stepped in the debugger. Choosing that for someone who only asked to copy a
value is choosing badly on their behalf.

If a task truly cannot be done in COBOL, say so and ask — do not decide alone.

Generated COBOL must remain COBOL-85 compatible unless a documented PowerRustCOBOL extension is required. Preserve divisions, data declarations, the paragraphs a body declares for its own `PERFORM`s, and existing user code.
"#;

/// Base IDE `agentic_ai` directory (always loaded).
pub fn agentic_dir() -> PathBuf {
    PathBuf::from(AGENTIC_DIR)
}

/// Legacy project-specific overrides directory inside the IDE `agentic_ai`
/// directory. Kept so older projects keep working, but new projects use
/// `project_agentic_root()` directly inside the project.
pub fn project_agentic_dir(project_dir: &Path) -> Option<PathBuf> {
    project_dir
        .file_name()
        .map(|name| agentic_dir().join("projects").join(name))
}

/// Project-owned `agentic_ai` directory shown in the Project Tree.
pub fn project_agentic_root(project_dir: &Path) -> PathBuf {
    project_dir.join(AGENTIC_DIR)
}

pub fn project_agent_dir(project_dir: &Path, agent_dir: &str) -> PathBuf {
    project_agentic_root(project_dir).join(agent_dir)
}

fn default_form_designer_prompt() -> String {
    std::fs::read_to_string(agentic_dir().join(PROMPT_FILE))
        .unwrap_or_else(|_| "You are the PowerRustCOBOL Form Designer Agent.".to_string())
}

fn default_event_handler_prompt() -> String {
    let base = std::fs::read_to_string(agentic_dir().join(ASSISTANT_PROMPT_FILE))
        .unwrap_or_else(|_| crate::llm::DEFAULT_SYSTEM_PROMPT.to_string());
    format!("{base}\n\n{EVENT_HANDLER_STEERING}")
}

fn write_if_missing(path: &Path, content: &str) -> Result<(), String> {
    if path.exists() {
        return Ok(());
    }
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("Could not create {}: {e}", parent.display()))?;
    }
    std::fs::write(path, content).map_err(|e| format!("Could not write {}: {e}", path.display()))
}

/// Superseded `EVENT_HANDLER_SKILL` texts. The 1.55.6 one taught `CALL` as
/// something reserved for "runtime/library procedures", which is how a reviewer
/// came to reject a correct `CALL` to a common procedure as a hallucination.
const LEGACY_EVENT_HANDLER_SKILLS: &[&str] = &[r#"# COBOL Event Handler Script Agent Skill

Generate event-handler COBOL that is valid in the PowerRustCOBOL nested-program body edited by the IDE.

Expected shape:

```cobol
       ENVIRONMENT DIVISION.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 WS-VALUE PIC X(80).
       PROCEDURE DIVISION.
           *> code here
```

Use inline control access:

```cobol
       SET Button-1::Text TO "Save".
       SET Panel-1::ShadowEnabled TO 1.
       Button-1::Refresh().
```

Do not use `CALL` for form/control methods or properties. Keep `CALL` only for real runtime/library procedures that have no inline method equivalent.
"#];

/// Superseded `RUSTCOBOL_SKILL` texts — the first silent on the nest, so
/// nothing told an agent which verb reaches a common procedure; the second
/// silent on `EXEC RUST`, which is how an agent came to answer "copy this value
/// to fifteen controls" with a Rust block and defend it as concise.
const LEGACY_RUSTCOBOL_SKILLS: &[&str] = &[
    r#"# PowerRustCOBOL Extensions Skill

PowerRustCOBOL extends COBOL-85 with inline form/control access:

- Get a property with `<control>::<property>`.
- Set a property with `SET <control>::<property> TO <value>`.
- Invoke a method with `<control>::<method>(<parameters>)`.

Generated COBOL must remain COBOL-85 compatible unless a documented PowerRustCOBOL extension is required. Preserve divisions, data declarations, paragraph structure, and existing user code.
"#,
    r#"# PowerRustCOBOL Extensions Skill

PowerRustCOBOL extends COBOL-85 with inline form/control access:

- Get a property with `<control>::<property>`.
- Set a property with `SET <control>::<property> TO <value>`.
- Invoke a method with `<control>::<method>(<parameters>)`.

## What a handler is, and how code reaches other code

A form is one compilation unit: the form itself is the OUTERMOST program, and every event handler and every common procedure is a separate NESTED program inside it. That structure decides which verb reaches what.

- `CALL "NAME"` is the only way to reach another program — that includes every common procedure created with `create_procedure`. Write `CALL "UPDATE-TOTAL".` or `CALL "RECALC" USING WS-QTY WS-PRICE.`.
- `PERFORM` reaches only a paragraph or section declared in the same body you are writing, and never crosses a program boundary. A `PERFORM` naming a procedure of another program is a compile error, not a style preference.
- The generated infrastructure paragraphs (`<id>-OPEN`, `<id>-READ-NEXT`, the timer, chart, CSV-export and data-binding helpers) live in the OUTER program, so form-level code may `PERFORM` them but a handler may not. From a handler, use the control's `::` methods.
- Do not use `CALL` for a control's own properties or methods — `::` is the only form for those.

Generated COBOL must remain COBOL-85 compatible unless a documented PowerRustCOBOL extension is required. Preserve divisions, data declarations, the paragraphs a body declares for its own `PERFORM`s, and existing user code.
"#,
];

/// Superseded steering texts. Steering was seeded with `write_if_missing`, so
/// every project created before a rule existed kept steering that never carried
/// it — the `EXEC RUST` rule among them. Listed here, an untouched copy is
/// refreshed on project open; a developer's own edits are left alone.
const LEGACY_FORM_DESIGNER_STEERINGS: &[&str] = &[r#"# Form Designer Agent Steering

- Build form changes as structured operations only; do not describe changes that are not present in the JSON change-set.
- Use the supplied project inventory before claiming a file, form, indexed file, control, data item, property, or event does not exist.
- Use exact control property names from the supplied schema. If the user uses a friendly name, map it to the real property before emitting an operation.
- Prefer inline PowerRustCOBOL object syntax for generated COBOL: `<control>::<method>(...)` and `<control>::<property>`.
- Never remove required COBOL divisions from generated handlers. If the correct change is unclear after validation feedback, ask the developer for directions.
"#];

const LEGACY_EVENT_HANDLER_STEERINGS: &[&str] = &[r#"# COBOL Event Handler Script Agent Steering

- Return a complete event-handler body only when the user asks to write or change code.
- The editable body must include `ENVIRONMENT DIVISION.`, `DATA DIVISION.`, and `PROCEDURE DIVISION.`.
- Do not return `IDENTIFICATION DIVISION`, `PROGRAM-ID`, `GOBACK`, or `END PROGRAM`; the IDE owns that scaffold.
- Preserve existing declarations and code unless the user explicitly asks to change them.
- Use inline PowerRustCOBOL object syntax: `<control>::<method>(...)` and `<control>::<property>`. Do not use `CALL` for control methods or properties.
- If a property, method, data item, or intended behavior cannot be determined, ask the developer for directions instead of guessing.
"#];

/// Seed the file, and also replace it when it still holds a superseded default
/// verbatim.
///
/// `write_if_missing` alone means a project created before a correction keeps
/// the wrong text for the life of the project — and these skills are injected
/// into every request, so a stale one keeps teaching the dead paragraph model
/// no matter how many times the platform is fixed. A file that matches a
/// superseded default exactly was never touched by the developer, so replacing
/// it loses nothing; anything else is theirs and is left alone. This mirrors
/// how stored agent PROMPTS are upgraded on project open (`agents_db`'s
/// `prompt_is_unmodified_legacy`).
fn write_or_refresh(path: &Path, content: &str, superseded: &[&str]) -> Result<(), String> {
    match std::fs::read_to_string(path) {
        Ok(existing) => {
            let is_untouched_default = superseded.iter().any(|old| existing.trim() == old.trim());
            if is_untouched_default && existing.trim() != content.trim() {
                return std::fs::write(path, content)
                    .map_err(|e| format!("Could not write {}: {e}", path.display()));
            }
            Ok(())
        }
        Err(_) => write_if_missing(path, content),
    }
}

/// Create the project-local editable agent files if they do not exist yet.
pub fn ensure_project_agentic_files(project_dir: &Path) -> Result<(), String> {
    if project_dir.as_os_str().is_empty() {
        return Ok(());
    }

    let form_dir = project_agent_dir(project_dir, FORM_DESIGNER_AGENT_DIR);
    let form_skills = form_dir.join(SKILLS_DIR);
    write_if_missing(&form_dir.join(PROMPT_FILE), &default_form_designer_prompt())?;
    write_or_refresh(
        &form_dir.join(STEERING_FILE),
        FORM_DESIGNER_STEERING,
        LEGACY_FORM_DESIGNER_STEERINGS,
    )?;
    write_or_refresh(
        &form_skills.join(RUSTCOBOL_SKILL_FILE),
        RUSTCOBOL_SKILL,
        LEGACY_RUSTCOBOL_SKILLS,
    )?;
    write_if_missing(
        &form_skills.join(FORM_DESIGNER_SKILL_FILE),
        FORM_DESIGNER_SKILL,
    )?;
    // Both agents get the Maps skill: the designer places the control and its
    // properties, the event-handler writes the async/onComplete half. Getting
    // either wrong produces a map that looks built and does nothing.
    write_or_refresh(&form_skills.join(MAPS_SKILL_FILE), MAPS_SKILL, &[])?;

    let event_dir = project_agent_dir(project_dir, EVENT_HANDLER_AGENT_DIR);
    let event_skills = event_dir.join(SKILLS_DIR);
    write_if_missing(
        &event_dir.join(PROMPT_FILE),
        &default_event_handler_prompt(),
    )?;
    write_or_refresh(
        &event_dir.join(STEERING_FILE),
        EVENT_HANDLER_STEERING,
        LEGACY_EVENT_HANDLER_STEERINGS,
    )?;
    write_or_refresh(
        &event_skills.join(RUSTCOBOL_SKILL_FILE),
        RUSTCOBOL_SKILL,
        LEGACY_RUSTCOBOL_SKILLS,
    )?;
    write_or_refresh(
        &event_skills.join(EVENT_HANDLER_SKILL_FILE),
        EVENT_HANDLER_SKILL,
        LEGACY_EVENT_HANDLER_SKILLS,
    )?;
    write_or_refresh(&event_skills.join(MAPS_SKILL_FILE), MAPS_SKILL, &[])?;
    Ok(())
}

fn read_or_default(path: &Path, default: String) -> String {
    std::fs::read_to_string(path).unwrap_or(default)
}

fn load_agent_references(agent_dir: &Path) -> String {
    let mut out = String::new();
    if let Ok(text) = std::fs::read_to_string(agent_dir.join(STEERING_FILE)) {
        out.push_str(&text);
    }

    let mut files: Vec<PathBuf> = std::fs::read_dir(agent_dir.join(SKILLS_DIR))
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
    out
}

pub fn effective_form_designer_prompt(project_dir: &Path) -> String {
    let _ = ensure_project_agentic_files(project_dir);
    let path = project_agent_dir(project_dir, FORM_DESIGNER_AGENT_DIR).join(PROMPT_FILE);
    read_or_default(&path, default_form_designer_prompt())
}

pub fn load_form_designer_skills(project_dir: &Path) -> String {
    let _ = ensure_project_agentic_files(project_dir);
    load_agent_references(&project_agent_dir(project_dir, FORM_DESIGNER_AGENT_DIR))
}

pub fn effective_event_handler_prompt(project_dir: &Path) -> String {
    let _ = ensure_project_agentic_files(project_dir);
    let path = project_agent_dir(project_dir, EVENT_HANDLER_AGENT_DIR).join(PROMPT_FILE);
    read_or_default(&path, default_event_handler_prompt())
}

pub fn load_event_handler_skills(project_dir: &Path) -> String {
    let _ = ensure_project_agentic_files(project_dir);
    load_agent_references(&project_agent_dir(project_dir, EVENT_HANDLER_AGENT_DIR))
}

/// The effective system prompt for a project (R14).
pub fn effective_prompt(project_dir: &Path) -> String {
    effective_form_designer_prompt(project_dir)
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
    load_form_designer_skills(project_dir)
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

    // Form-level properties. Without these the agent cannot see the current
    // style or know which values are legal, and any reviewer demand for
    // evidence of a form-level change is unanswerable.
    out.push_str("FORM PROPERTIES (target these with 'set_property' using \"control_id\": \"Form\"):\n");
    out.push_str(&format!(
        "  GlassStyle={:?}  Theme={:?}  UseThemeBackground={}\n",
        form.glass_style.as_str(),
        form.theme.clone().unwrap_or_default(),
        form.use_theme_background
    ));
    out.push_str(&format!(
        "  SUPPORTED GlassStyle VALUES (exact spelling, no other value is accepted): {}\n",
        cobolt_forms::GlassStyle::ALL
            .iter()
            .map(|s| format!("{s:?}"))
            .collect::<Vec<_>>()
            .join(", ")
    ));
    out.push_str(
        "  GlassStyle is the visual style of the form and its controls (this is what \"neumorphic dark\", \"neumorphic light\", \"classic\", \"enhanced\" refer to).\n  Theme is a SEPARATE named asset-pack slot and is NOT how a GlassStyle is selected.\n\n",
    );
    out.push_str(&format!(
        "  X={}  Y={}  StartPosition={:?}\n",
        form.x,
        form.y,
        form.start_position.as_str()
    ));
    out.push_str(&format!(
        "  SUPPORTED StartPosition VALUES (exact spelling): {}\n",
        cobolt_forms::model::FormStartPosition::ALL
            .iter()
            .map(|s| format!("{:?}", s.as_str()))
            .collect::<Vec<_>>()
            .join(", ")
    ));
    out.push_str(
        "  X/Y are design-time window coordinates in screen pixels. They are ONLY applied at \
         launch when StartPosition is \"Custom\" — every other StartPosition computes its own \
         position (or, for \"System\", lets the OS place the window) and ignores X/Y. Setting X/Y \
         alone does not move the window unless StartPosition is also set to \"Custom\".\n\n",
    );

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

    let all_types = ALL_CONTROL_TYPES;

    out.push_str("PROPERTY KEYS BY TYPE (for all available controls):\n");
    for t in &all_types {
        out.push_str(&format!("  {}: {}\n", t, property_names_for(t).join(", ")));
    }
    // A bare name in the listing above tells the model a property EXISTS,
    // never what values are legal to set it to — the same gap GlassStyle and
    // StartPosition were already special-cased for above. `property_reference`
    // is the curated domain table cobolt-compiler already keeps for its own
    // generated docs; a property name means the same domain on every control
    // that has it, so this is listed once, not per type (operator, 2026-08-01:
    // Grace correctly flagged BorderStyle's context as listing the property
    // but not its supported values).
    let mut prop_names = std::collections::BTreeSet::new();
    for t in &all_types {
        prop_names.extend(property_names_for(t));
    }
    out.push_str("PROPERTY VALUE DOMAINS (same property name = same legal values on every control type that has it):\n");
    for name in &prop_names {
        if let Some((domain, _)) = cobolt_compiler::property_reference(name) {
            out.push_str(&format!("  {name}: {domain}\n"));
        }
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
    out.push_str(&event_handlers_context(form));
    out
}

/// The existing bound event handlers, verbatim — the only place an agent can
/// learn what a control's behavior actually IS, as opposed to what events its
/// TYPE merely supports (`EVENTS BY TYPE`, above, is a legend of names, not a
/// record of any control's real wiring).
///
/// Without this, a task that asks an agent to read existing behavior — "write
/// a caption describing what this handler does", "match this control's
/// effect on the others" — has nothing to read. Observed live: asked to
/// summarize each of 15 TextBoxes' handler into a Label caption, every
/// specialist that tried wrote the SAME caption on all 15 — first the
/// developer's own example, then a mangled copy of the task instruction
/// itself — because that was the only text available to copy from
/// (operator, 2026-07-31). Empty section markers are omitted entirely, so an
/// ordinary form with no handlers yet leaves the context unchanged.
fn event_handlers_context(form: &Form) -> String {
    let mut out = String::new();
    let mut control_lines: Vec<String> = Vec::new();
    for c in &form.controls {
        for ev in &c.events {
            if ev.has_code() {
                control_lines.push(format!("  {}::{}\n{}\n", c.id, ev.event, ev.code));
            }
        }
    }
    if !control_lines.is_empty() {
        out.push_str(
            "EVENT HANDLERS (existing bound code, verbatim — the ONLY source of \
             truth for what a control's event actually does; a task that reads or \
             describes existing behavior must be answered from this, never \
             invented or guessed):\n",
        );
        out.push_str(&control_lines.join("\n"));
    }
    let form_lines: Vec<String> = form
        .form_events
        .iter()
        .filter(|ev| ev.has_code())
        .map(|ev| format!("  {}\n{}\n", ev.event, ev.code))
        .collect();
    if !form_lines.is_empty() {
        out.push_str("FORM EVENT HANDLERS (existing bound code, verbatim):\n");
        out.push_str(&form_lines.join("\n"));
    }
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

/// The project-tree half of the request context on its own — used by the
/// project-wide Grace chatbot when no form is open, so a request can still name
/// real forms, indexed files and sources instead of inventing them.
pub(crate) fn build_project_tree_context(
    project: Option<&CoboltProject>,
    project_root: Option<&Path>,
) -> String {
    let mut out = String::new();
    out.push_str("PROJECT TREE INVENTORY\n");
    out.push_str(
        "Use this inventory to discover project resources before proposing changes. \
         For CRUD forms over indexed files, inspect the INDEXED FILES section first. \
         If the request matches multiple resources, ask the user which one to use. \
         Each section below (FORMS, INDEXED FILES, COMMON COBOL SOURCES, GENERATED COBOL, \
         ASSETS, DOCUMENTATION) is its canonical top-level folder, already listed \
         RECURSIVELY — every entry is a full path, so a resource nested inside a \
         subfolder (e.g. a form under \"forms/Common/\") is listed exactly like one \
         at the top level. Match a named resource against the FULL path of every \
         entry, not just its first path component, and never conclude a resource \
         is missing merely because it is not directly under the top-level folder.\n",
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
    out.push_str(&window_effects_context(project));
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

/// The PROJECT's own window entrance/exit effect settings — configured once,
/// in `[forms]` of the project file, and applied to every form whose own
/// `WindowEffects` property is true. Distinct from anything a `Form` carries:
/// a form only has that one on/off switch; the effect itself, its duration,
/// and its easing are project-wide and live on `CoboltProject`, which
/// `build_context` (given only a `&Form`) never sees.
///
/// Without this, a question like "what entrance effect does the project use,
/// and how long does it run" had no live value to answer from — the platform
/// Knowledge Base documents the effect CATALOGUE (what `matrix-rain` is, its
/// duration band, that it is a project setting) but cannot contain what THIS
/// project's `entrance-effect` / `entrance-ms` are actually set to; that is
/// data, not documentation (operator, 2026-07-31).
fn window_effects_context(project: &CoboltProject) -> String {
    let entrance = project.entrance_fx();
    let exit = project.exit_fx();
    format!(
        "WINDOW EFFECTS (project-level; applies to every form whose WindowEffects property is true):\n  \
         Entrance: {}, {} ms, {} easing\n  \
         Exit: {}, {} ms, {} easing\n  \
         Replay entrance when a window is restored after minimize: {}\n",
        entrance.effect.as_str(),
        entrance.duration_ms,
        entrance.easing.as_str(),
        exit.effect.as_str(),
        exit.duration_ms,
        exit.easing.as_str(),
        project.forms.entrance_on_restore,
    )
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
/// The complete drop-shadow property group. When a control casts a shadow we
/// surface every one of these — see the note in `non_default_props`.
const SHADOW_KEYS: &[&str] = &[
    "ShadowEnabled",
    "ShadowOpacity",
    "ShadowColor",
    "ShadowLightColor",
    "ShadowDirection",
    "ShadowDistance",
    "ShadowBlur",
    "ShadowBlurStrength",
];

fn non_default_props(c: &Control) -> Vec<String> {
    let defaults = Control::new("_", c.control_type.clone(), 0, 0);
    let mut seen = std::collections::HashSet::new();
    let mut out = Vec::new();
    for (k, v) in &c.properties {
        if defaults.properties.get(k) != Some(v) {
            seen.insert(k.clone());
            out.push(format!("{k}={}", prop_display(v)));
        }
    }
    // When a control actually casts a drop shadow, surface its COMPLETE shadow
    // configuration even where individual members are still at the type default.
    // Otherwise a member left at its default (e.g. ShadowDirection="SouthEast")
    // is invisible in the CONTEXT, and an instruction to "copy this control's
    // drop shadow onto the others" structurally cannot copy what it never sees.
    let casts_shadow = matches!(
        c.properties.get("ShadowEnabled"),
        Some(PropValue::Bool(true))
    );
    if casts_shadow {
        for key in SHADOW_KEYS {
            if !seen.contains(*key) {
                if let Some(v) = c.properties.get(*key) {
                    out.push(format!("{key}={}", prop_display(v)));
                }
            }
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

    /// A lone handler is the one this surface asked for, whatever the agent
    /// called the event — dropping it is a silent loss of real work.
    #[test]
    fn a_single_handler_applies_even_when_the_event_is_spelled_differently() {
        let reply = r#"```json
{"operations":[{"op":"generate_event_handler","control_id":"BTN-MARKERS","event":"Click","code":"       PROCEDURE DIVISION.\n           INVOKE MAP-1 \"AddMarker\"."}]}
```"#;
        let (_, code) = event_handler_reply(reply, "BTN-MARKERS", "onClick", "fallback");
        assert!(
            code.expect("the only handler must apply").contains("AddMarker"),
            "a lone handler must not be discarded over an event spelling"
        );
    }

    /// **A question must arrive as a question.**
    ///
    /// The event agent asks through a `message` operation — its contract says
    /// so. 1.64.27 read only the change-set note, found none, and showed the
    /// "Updated this handler." fallback: the developer was told work had been
    /// done instead of being asked what was meant (operator, 2026-09-04).
    #[test]
    fn a_message_operation_reaches_the_developer_as_itself() {
        let reply = r#"```json
{"operations":[{"op":"message","message":"Which label should the marker show — the control id BTN-MARKERS, or its caption?"}]}
```"#;
        let (text, code) =
            event_handler_reply(reply, "BTN-MARKERS", "onClick", "Updated this handler.");
        assert!(
            text.starts_with("Which label should the marker show"),
            "the question itself must be shown, got: {text}"
        );
        assert_ne!(text, "Updated this handler.", "never claim work that was not done");
        assert!(code.is_none(), "a question carries no handler");
    }

    /// Several messages all reach the developer, in order.
    #[test]
    fn every_message_operation_is_shown() {
        let reply = r#"```json
{"operations":[{"op":"message","message":"First question?"},{"op":"message","message":"Second question?"}]}
```"#;
        let (text, _) = event_handler_reply(reply, "X", "onClick", "fallback");
        assert!(text.contains("First question?") && text.contains("Second question?"), "got: {text}");
    }

    /// A message alongside real work is still the answer to read.
    #[test]
    fn a_message_outranks_the_note_when_both_are_present() {
        let reply = r#"```json
{"operations":[{"op":"generate_event_handler","control_id":"BTN-MARKERS","event":"onClick","code":"       PROCEDURE DIVISION.\n           CONTINUE."},{"op":"message","message":"I assumed metres; say the word if you meant feet."}],"note":"Handler rewritten."}
```"#;
        let (text, code) = event_handler_reply(reply, "BTN-MARKERS", "onClick", "fallback");
        assert!(text.contains("I assumed metres"), "got: {text}");
        assert!(code.is_some(), "the work still applies");
    }

    /// **The balloon shows the answer, never the code just applied.**
    ///
    /// With Grace's own summary empty, the workflow fell back to the
    /// specialist's submission, so the balloon read
    /// "Grace: ```cobol ENVIRONMENT DIVISION. DATA DIVISION. …" — the handler
    /// echoed back at the developer, who can see it in the editor above
    /// (operator, 2026-09-04).
    #[test]
    fn a_reply_that_is_only_code_shows_the_plain_line_instead() {
        let reply = "Grace: ```cobol\n       ENVIRONMENT DIVISION.\n       DATA DIVISION.\n\
                     \x20      WORKING-STORAGE SECTION.\n       LINKAGE SECTION.\n```";
        assert_eq!(
            readable_handler_answer(reply, "Updated this handler."),
            "Updated this handler."
        );
    }

    /// Prose the agent wrote around its code IS the answer, and survives.
    #[test]
    fn prose_around_the_code_is_kept_and_the_code_is_dropped() {
        let reply = "I added a comment before each INVOKE line describing the marker.\n\
                     ```cobol\n       PROCEDURE DIVISION.\n           INVOKE MAP-1 \"AddMarker\".\n```";
        let shown = readable_handler_answer(reply, "fallback");
        assert!(shown.starts_with("I added a comment before each INVOKE line"));
        // NOT "INVOKE": the prose legitimately names it. The CODE is what
        // must be gone.
        assert!(
            !shown.contains("PROCEDURE DIVISION") && !shown.contains("AddMarker"),
            "the code must not be echoed: {shown}"
        );
        assert!(!shown.contains("```"), "no fence reaches the balloon: {shown}");
    }

    /// A fence opened mid-line ("Grace: ```cobol") must still be recognised —
    /// testing only line starts is what let the code through.
    #[test]
    fn a_fence_that_opens_mid_line_is_still_a_fence() {
        let reply = "Done. ```cobol\n       PROCEDURE DIVISION.\n``` and that is all it needed.";
        let shown = readable_handler_answer(reply, "fallback");
        assert!(!shown.contains("PROCEDURE DIVISION"), "got: {shown}");
        assert!(shown.contains("and that is all it needed"), "got: {shown}");
    }

    /// **A change-set is machinery, not an answer.**
    ///
    /// A Grace workflow puts the specialist's `{"operations": …}` block on the
    /// reply so the surface can apply it. The COBOL Event Editor looked only
    /// for a ```cobol block, found none, and showed the developer raw JSON in
    /// the balloon (operator, 2026-09-04) — while the handler itself had been
    /// updated correctly.
    #[test]
    fn a_change_set_reply_yields_the_handler_and_never_shows_its_json() {
        let reply = r#"```json
{"operations":[{"op":"generate_event_handler","control_id":"BTN-MARKERS","event":"onClick",
"code":"       PROCEDURE DIVISION.\n      *> Add a marker for Ana\n           INVOKE MAP-1 \"AddMarker\"."}],
"note":"Added a comment before each INVOKE line."}
```"#;
        let (text, code) =
            event_handler_reply(reply, "BTN-MARKERS", "onClick", "Updated this handler.");

        assert_eq!(text, "Added a comment before each INVOKE line.");
        assert!(!text.contains("operations"), "the JSON must never be shown: {text}");
        assert!(!text.contains('{'), "no braces reach the balloon: {text}");
        let code = code.expect("the handler body must be extracted");
        assert!(code.contains("INVOKE MAP-1"));
        assert!(code.contains("*> Add a marker for Ana"));
    }

    /// With no note of its own, the developer still gets a sentence.
    #[test]
    fn a_change_set_without_a_note_falls_back_to_a_plain_line() {
        let reply = r#"```json
{"operations":[{"op":"generate_event_handler","control_id":"BTN-MARKERS","event":"onClick","code":"       PROCEDURE DIVISION.\n           CONTINUE."}]}
```"#;
        let (text, code) =
            event_handler_reply(reply, "BTN-MARKERS", "onClick", "Updated this handler.");
        assert_eq!(text, "Updated this handler.");
        assert!(code.is_some());
    }

    /// A change-set for ANOTHER control must not be applied to this handler,
    /// but its note is still the answer to show.
    #[test]
    fn a_change_set_for_another_control_yields_no_code_for_this_one() {
        // TWO handlers, neither this one: the lone-handler rescue must not
        // fire, because there is nothing to be sure about.
        let reply = r#"```json
{"operations":[
{"op":"generate_event_handler","control_id":"BTN-OTHER","event":"onClick","code":"       PROCEDURE DIVISION.\n           CONTINUE."},
{"op":"generate_event_handler","control_id":"BTN-THIRD","event":"onClick","code":"       PROCEDURE DIVISION.\n           CONTINUE."}],
"note":"Wired the other button."}
```"#;
        let (text, code) =
            event_handler_reply(reply, "BTN-MARKERS", "onClick", "Updated this handler.");
        assert_eq!(text, "Wired the other button.");
        assert!(
            code.is_none(),
            "another control's handler must never land in this one"
        );
    }

    /// Prose and plain code blocks are untouched — the paths that already
    /// worked must keep working.
    #[test]
    fn a_prose_reply_passes_through_unchanged() {
        let prose = "The INVOKE calls place five markers on the map.";
        let (text, code) = event_handler_reply(prose, "BTN-MARKERS", "onClick", "fallback");
        assert_eq!(text, prose);
        assert!(code.is_none());

        let with_code = "Here you go:\n```cobol\n       PROCEDURE DIVISION.\n```";
        let (text, code) = event_handler_reply(with_code, "BTN-MARKERS", "onClick", "fallback");
        assert_eq!(text, with_code, "a plain code reply is left for extract_code");
        assert!(code.is_none());
    }
    use super::*;

    /// Reading an async answer is not "a hallucinated property".
    ///
    /// `Directions`, `Geocode`, every `RestClient` verb and `WebSearch::Search`
    /// return an empty string at once and deliver through `ResponseBody` on
    /// `onComplete` — the documented, only way. Judging that read against the
    /// *settable* property list rejected the handler the platform's own KB tells
    /// developers to write (operator, 2026-08-21).
    #[test]
    fn an_async_answer_is_readable_even_though_nothing_can_set_it() {
        let mut known = HashMap::new();
        known.insert("MAP-1".to_string(), ControlType::Maps);
        known.insert("REST-1".to_string(), ControlType::RestClient);

        let reads = concat!(
            "       PROCEDURE DIVISION.\n",
            "           UNSTRING MAP-1::ResponseBody DELIMITED BY X\"09\"\n",
            "               INTO WS-A WS-B.\n",
            "           MOVE MAP-1::SelectedMarkerId TO WS-ID.\n",
            "           MOVE REST-1::StatusCode TO WS-ST.\n",
        );
        assert_eq!(
            unknown_property_ref(reads, &known),
            None,
            "the runtime's own delivery channel must pass the gate"
        );

        // The gate still catches an actually invented property.
        let invented = "       PROCEDURE DIVISION.\n           MOVE MAP-1::Depth TO WS-X.\n";
        assert_eq!(
            unknown_property_ref(invented, &known),
            Some(("MAP-1".to_string(), "Depth".to_string()))
        );

        // Reading and writing stay different questions: nothing SETS an answer.
        assert!(property_readable(&ControlType::Maps, "ResponseBody"));
        assert!(!property_valid(&ControlType::Maps, "ResponseBody"));
        // And a control with no async surface gains nothing either way.
        assert!(!property_readable(&ControlType::Button, "ResponseBody"));
    }

    /// `EXEC RUST` is the developer's choice, and every text an agent reads
    /// must say so.
    ///
    /// Asked to copy `Knob-1`'s value into fifteen controls, an agent answered
    /// with a Rust block and defended it as concise, readable and supported by
    /// the platform (operator, 2026-08-16). Nothing it had been given ruled
    /// that out, so the rule now lives in the steering BOTH code agents read
    /// and in the extensions skill injected into every request.
    /// The enforcement behind the prompts: a block nobody asked for is refused,
    /// and a block that was asked for goes through.
    #[test]
    fn exec_rust_is_refused_unless_the_request_asked_for_rust() {
        let with_block = concat!(
            "       PROCEDURE DIVISION.\n",
            "           EXEC RUST\n",
            "               let x = 1;\n",
            "           END-EXEC.\n"
        );
        let plain = "       PROCEDURE DIVISION.\n           MOVE 1 TO WS-N.\n";

        // Not asked for ⇒ refused, at the block's own line.
        assert_eq!(
            unrequested_exec_rust(with_block, "copy Knob-1's value to every numeric control"),
            Some(2)
        );
        // Asked for ⇒ allowed, however it is worded.
        for asked in [
            "do this in Rust",
            "use an EXEC RUST block",
            "read it with the csv crate in rust",
        ] {
            assert_eq!(
                unrequested_exec_rust(with_block, asked),
                None,
                "'{asked}' asks for Rust"
            );
        }
        // No block ⇒ nothing to refuse.
        assert_eq!(unrequested_exec_rust(plain, "anything"), None);

        // The trap this product sets for itself: the LANGUAGE is called
        // RustCOBOL, so naming it must not read as asking for Rust — otherwise
        // half the requests in this IDE would authorise a block by accident.
        for naming_the_product in [
            "write this in RustCOBOL",
            "use PowerRustCOBOL syntax",
            "is this valid rustcobol?",
        ] {
            assert_eq!(
                unrequested_exec_rust(with_block, naming_the_product),
                Some(2),
                "'{naming_the_product}' names the product, it does not ask for Rust"
            );
        }
        // A comment mentioning the words is not a block.
        assert_eq!(
            unrequested_exec_rust("       *> no EXEC RUST here\n", "please"),
            None
        );

        println!(
            "\n  EXEC RUST — refused when unasked (incl. requests naming \
             RustCOBOL itself), allowed when asked\n"
        );
    }

    #[test]
    fn every_agent_text_reserves_exec_rust_for_an_explicit_request() {
        for (name, text) in [
            ("form designer steering", FORM_DESIGNER_STEERING),
            ("event handler steering", EVENT_HANDLER_STEERING),
            ("rustcobol skill", RUSTCOBOL_SKILL),
        ] {
            assert!(
                text.contains("EXEC RUST"),
                "{name} never mentions EXEC RUST, so nothing rules it out"
            );
            let lower = text.to_lowercase();
            assert!(
                lower.contains("unless the developer asked for rust")
                    || lower.contains("only when the developer asked for rust"),
                "{name} must reserve a block for an explicit request"
            );
        }
        // The excuses the agent actually gave are named, so the rule answers
        // the reasoning that produced the block rather than a generic ban.
        let skill = RUSTCOBOL_SKILL.to_lowercase();
        for excuse in ["concision", "readability", "platform supporting"] {
            assert!(
                skill.contains(excuse),
                "the skill must refuse '{excuse}' as a reason"
            );
        }
        println!("\n  EXEC RUST — reserved for an explicit request in all three agent texts\n");
    }

    /// A project seeded before a correction keeps the old text for the life of
    /// the project unless an untouched default is refreshed on open — which is
    /// how the `EXEC RUST` rule would have reached nobody who already had one.
    #[test]
    fn an_untouched_default_steering_is_refreshed_but_a_developer_edit_is_kept() {
        let dir = std::env::temp_dir().join(format!(
            "prc-steering-refresh-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();

        for (file, current, legacy) in [
            (
                "form-steering.md",
                FORM_DESIGNER_STEERING,
                LEGACY_FORM_DESIGNER_STEERINGS,
            ),
            (
                "event-steering.md",
                EVENT_HANDLER_STEERING,
                LEGACY_EVENT_HANDLER_STEERINGS,
            ),
        ] {
            let stale = dir.join(file);
            std::fs::write(&stale, legacy[0]).unwrap();
            write_or_refresh(&stale, current, legacy).unwrap();
            assert!(
                std::fs::read_to_string(&stale).unwrap().contains("EXEC RUST"),
                "{file}: an untouched steering must gain the new rule"
            );

            let mine = dir.join(format!("mine-{file}"));
            std::fs::write(&mine, "# My own steering\n\n- Do it my way.\n").unwrap();
            write_or_refresh(&mine, current, legacy).unwrap();
            assert_eq!(
                std::fs::read_to_string(&mine).unwrap(),
                "# My own steering\n\n- Do it my way.\n",
                "{file}: a developer's own steering is theirs"
            );
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// A project seeded before the correction still holds the skill that told
    /// agents to keep `CALL` "only for real runtime/library procedures" — the
    /// sentence behind a reviewer rejecting a correct `CALL` to a common
    /// procedure. `write_if_missing` would leave it there for the life of the
    /// project, so an untouched default has to be replaced on open.
    #[test]
    fn an_untouched_default_skill_is_refreshed_but_a_developer_edit_is_kept() {
        let dir = std::env::temp_dir().join(format!(
            "prc-skill-refresh-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();

        // An untouched superseded default is replaced…
        let stale = dir.join("event-handler.md");
        std::fs::write(&stale, LEGACY_EVENT_HANDLER_SKILLS[0]).unwrap();
        write_or_refresh(&stale, EVENT_HANDLER_SKILL, LEGACY_EVENT_HANDLER_SKILLS).unwrap();
        let refreshed = std::fs::read_to_string(&stale).unwrap();
        assert!(
            refreshed.contains("CALL \"VALIDATE-INPUT\"") && refreshed.contains("nested program"),
            "the superseded skill should have been replaced, got:\n{refreshed}"
        );
        assert!(
            !refreshed.contains("only for real runtime/library procedures"),
            "the sentence that caused the false rejection must be gone"
        );

        // …but anything the developer wrote is theirs and survives untouched.
        let owned = dir.join("mine.md");
        std::fs::write(&owned, "# My own skill\n\nHouse rules.\n").unwrap();
        write_or_refresh(&owned, EVENT_HANDLER_SKILL, LEGACY_EVENT_HANDLER_SKILLS).unwrap();
        assert_eq!(
            std::fs::read_to_string(&owned).unwrap(),
            "# My own skill\n\nHouse rules.\n",
            "a developer-edited skill must never be overwritten"
        );

        // A missing file is still seeded.
        let fresh = dir.join("new.md");
        write_or_refresh(&fresh, RUSTCOBOL_SKILL, LEGACY_RUSTCOBOL_SKILLS).unwrap();
        assert!(std::fs::read_to_string(&fresh).unwrap().contains("CALL \"NAME\""));

        let _ = std::fs::remove_dir_all(&dir);
    }

    /// `(X, Y)` of a deploy op, as the applier would read them back.
    fn xy(op: &AgentOp) -> (Option<i64>, Option<i64>) {
        match op {
            AgentOp::DeployControl { properties, .. } => (
                properties.get("X").and_then(|v| v.as_i64()),
                properties.get("Y").and_then(|v| v.as_i64()),
            ),
            _ => (None, None),
        }
    }

    fn deploys(json: &str) -> AgentChangeSet {
        parse_change_set(json).expect("change-set parses")
    }

    /// `SPECIAL-NAMES` is reserved to the outermost program by COBOL-85, so
    /// until this operation existed a request for comma currency could not be
    /// satisfied by any agent (operator, 2026-08-02).
    #[test]
    fn set_form_structure_accepts_the_five_blocks_and_rejects_anything_else() {
        for (spelling, canonical) in [
            ("SPECIAL-NAMES", "SPECIAL-NAMES"),
            ("special names", "SPECIAL-NAMES"),
            ("SpecialNames", "SPECIAL-NAMES"),
            ("REPOSITORY", "REPOSITORY"),
            ("FILE-CONTROL", "FILE-CONTROL"),
            ("FILE SECTION", "FILE SECTION"),
            ("WORKING-STORAGE", "WORKING-STORAGE"),
        ] {
            assert_eq!(form_structure_block(spelling), Some(canonical), "{spelling}");
        }
        for bad in ["PROCEDURE DIVISION", "LOCAL-STORAGE", "SCREEN SECTION", ""] {
            assert_eq!(form_structure_block(bad), None, "{bad} must be rejected");
        }
    }

    /// Codegen owns the division and section headers; a block body carrying one
    /// would weave a second header into the form's own division.
    #[test]
    fn set_form_structure_rejects_a_body_that_writes_its_own_scaffold() {
        let ok = AgentOp::SetFormStructure {
            block: "SPECIAL-NAMES".into(),
            code: "       DECIMAL-POINT IS COMMA.".into(),
        };
        assert_eq!(op_form_free_error(&ok), None);

        for scaffold in [
            "       CONFIGURATION SECTION.\n       DECIMAL-POINT IS COMMA.",
            "       ENVIRONMENT DIVISION.\n       DECIMAL-POINT IS COMMA.",
            "       DATA DIVISION.\n       01 WS-X PIC 9.",
        ] {
            let op = AgentOp::SetFormStructure {
                block: "SPECIAL-NAMES".into(),
                code: scaffold.into(),
            };
            assert!(
                op_form_free_error(&op).is_some(),
                "scaffold must be rejected: {scaffold}"
            );
        }

        let unknown = AgentOp::SetFormStructure {
            block: "PROCEDURE DIVISION".into(),
            code: "CONTINUE.".into(),
        };
        assert!(op_form_free_error(&unknown)
            .unwrap()
            .contains("not a form structure block"));
    }

    #[test]
    fn set_form_structure_parses_and_names_itself_in_diagnostics() {
        let cs = parse_change_set(
            r#"{"operations":[
  {"op":"set_form_structure","block":"WORKING-STORAGE","code":"       01  WS-TOTAL PIC 9(5)V99 GLOBAL."}
]}"#,
        )
        .expect("parses");
        match &cs.operations[0] {
            AgentOp::SetFormStructure { block, code } => {
                assert_eq!(block, "WORKING-STORAGE");
                assert!(code.contains("GLOBAL"));
            }
            other => panic!("expected set_form_structure, got {other:?}"),
        }
        assert_eq!(
            op_ref(&cs.operations[0]),
            "set_form_structure WORKING-STORAGE"
        );
    }

    #[test]
    fn snap_nearest_rounds_to_the_closer_grid_point() {
        assert_eq!(snap_nearest(19, 8), 16);
        assert_eq!(snap_nearest(21, 8), 24);
        assert_eq!(snap_nearest(24, 8), 24, "an on-grid value must not move");
        assert_eq!(snap_nearest(0, 8), 0);
        // Symmetric either side of the origin.
        assert_eq!(snap_nearest(-19, 8), -16);
        assert_eq!(snap_nearest(-21, 8), -24);
        // A disabled/degenerate grid leaves the value alone.
        assert_eq!(snap_nearest(19, 0), 19);
    }

    /// The shape that prompted the rule: a column of checkboxes the agent
    /// placed off-grid. Each column lands on a grid point AND stays a column.
    #[test]
    fn agent_columns_land_on_the_grid_and_stay_aligned() {
        let cs = deploys(
            r#"{"operations":[
  {"op":"deploy_control","control_type":"CheckBox","id":"B1","properties":{"X":19,"Y":20}},
  {"op":"deploy_control","control_type":"CheckBox","id":"B2","properties":{"X":21,"Y":50}},
  {"op":"deploy_control","control_type":"CheckBox","id":"B3","properties":{"X":20,"Y":80}},
  {"op":"deploy_control","control_type":"CheckBox","id":"D1","properties":{"X":380,"Y":20}}
]}"#,
        );
        let out = normalize_geometry(&cs, 8, true);

        // X=19 opens the lane and fixes the axis shift (-3); 21 and 20 are within
        // half a cell of it, so all three are that one column. The far column
        // takes the same shift, keeping the 361px gap the agent asked for.
        let xs: Vec<Option<i64>> = out.operations.iter().map(|op| xy(op).0).collect();
        assert_eq!(xs, vec![Some(16), Some(16), Some(16), Some(377)]);
        assert_eq!(xs[0].unwrap() % 8, 0, "the first placement is on the grid");

        // Rows are translated, not quantised: the 30px pitch survives intact and
        // the second column still shares the first column's rows.
        let ys: Vec<Option<i64>> = out.operations.iter().map(|op| xy(op).1).collect();
        assert_eq!(ys, vec![Some(24), Some(54), Some(84), Some(24)]);
        assert_eq!(ys[0].unwrap() % 8, 0, "the first row is on the grid");
    }

    /// Without the lane, `19` and `21` — 2px apart, straddling a cell edge —
    /// snap to 16 and 24 and the column the agent asked for is 8px crooked.
    #[test]
    fn a_lane_holds_coordinates_that_would_otherwise_snap_apart() {
        let cs = deploys(
            r#"{"operations":[
  {"op":"deploy_control","control_type":"Label","id":"A","properties":{"X":19,"Y":10}},
  {"op":"deploy_control","control_type":"Label","id":"B","properties":{"X":21,"Y":40}}
]}"#,
        );
        assert_ne!(
            snap_nearest(19, 8),
            snap_nearest(21, 8),
            "the hazard this closes must be real"
        );
        let out = normalize_geometry(&cs, 8, true);
        assert_eq!(xy(&out.operations[0]).0, xy(&out.operations[1]).0);
    }

    /// A column a whole cell away is a different column, not a stray pixel.
    #[test]
    fn distinct_columns_keep_their_own_lanes() {
        let cs = deploys(
            r#"{"operations":[
  {"op":"deploy_control","control_type":"Label","id":"A","properties":{"X":20,"Y":10}},
  {"op":"deploy_control","control_type":"Label","id":"B","properties":{"X":200,"Y":10}},
  {"op":"deploy_control","control_type":"Label","id":"C","properties":{"X":380,"Y":10}}
]}"#,
        );
        let out = normalize_geometry(&cs, 8, true);
        let xs: Vec<Option<i64>> = out.operations.iter().map(|op| xy(op).0).collect();
        // One shift (+4) for the axis: the 180px gaps the agent asked for are
        // still 180px, and only the first column lands on a grid point.
        assert_eq!(xs, vec![Some(24), Some(204), Some(384)]);
    }

    /// `set_property` moves an existing control and is snapped the same way,
    /// sharing lanes with the deploys around it.
    #[test]
    fn set_property_moves_are_snapped_too() {
        let cs = deploys(
            r#"{"operations":[
  {"op":"deploy_control","control_type":"Label","id":"A","properties":{"X":19,"Y":10}},
  {"op":"set_property","control_id":"OLD","key":"X","value":22},
  {"op":"set_property","control_id":"OLD","key":"Caption","value":"untouched"}
]}"#,
        );
        let out = normalize_geometry(&cs, 8, true);
        match &out.operations[1] {
            AgentOp::SetProperty { value, .. } => assert_eq!(value.as_i64(), Some(16)),
            other => panic!("expected set_property, got {other:?}"),
        }
        match &out.operations[2] {
            AgentOp::SetProperty { value, .. } => assert_eq!(value.as_str(), Some("untouched")),
            other => panic!("expected set_property, got {other:?}"),
        }
    }

    /// Grid off: nothing quantises, so nothing can split an alignment and the
    /// agent's own placement is left exactly as written.
    #[test]
    fn a_disabled_grid_leaves_placement_untouched() {
        let json = r#"{"operations":[
  {"op":"deploy_control","control_type":"Label","id":"A","properties":{"X":19,"Y":13}},
  {"op":"deploy_control","control_type":"Label","id":"B","properties":{"X":19,"Y":41}}
]}"#;
        let cs = deploys(json);
        let out = normalize_geometry(&cs, 8, false);
        assert_eq!(out, cs, "a disabled grid must be a no-op");
        // The alignment the agent defined still holds on its own.
        assert_eq!(xy(&out.operations[0]).0, xy(&out.operations[1]).0);
    }

    /// The agent may write a coordinate as a numeric string; the applier
    /// accepts that, so normalisation has to see it too.
    #[test]
    fn string_coordinates_are_snapped() {
        let cs = deploys(
            r#"{"operations":[
  {"op":"deploy_control","control_type":"Label","id":"A","properties":{"X":"19","Y":"21"}}
]}"#,
        );
        let out = normalize_geometry(&cs, 8, true);
        assert_eq!(xy(&out.operations[0]), (Some(16), Some(24)));
    }

    /// The agent cannot pick a style it cannot see. CONTEXT must carry the
    /// current form-level values and the exact legal GlassStyle spellings.
    #[test]
    fn context_exposes_form_properties_and_supported_styles() {
        let mut form = Form::new("ACTORS-FORM", "Actors", 1016, 808);
        form.glass_style = cobolt_forms::GlassStyle::NeumorphicDark;
        let ctx = build_context(&form);

        assert!(ctx.contains("FORM PROPERTIES"));
        assert!(ctx.contains("GlassStyle=\"Neumorphic Dark\""));
        for value in cobolt_forms::GlassStyle::ALL {
            assert!(
                ctx.contains(value),
                "CONTEXT must advertise GlassStyle {value:?}"
            );
        }
        // Every advertised value must also be one the applier accepts.
        assert!(form_property_valid("GlassStyle"));
    }

    /// Grace must be able to both READ where the window opens and SET a real
    /// coordinate for it, and must be told the one thing that trips a naive
    /// "just set X/Y" attempt: neither is applied unless StartPosition is
    /// also "Custom".
    #[test]
    fn context_exposes_window_start_position() {
        let mut form = Form::new("F", "T", 800, 600);
        form.x = 240;
        form.y = -30;
        form.start_position = cobolt_forms::model::FormStartPosition::BottomRight;
        let ctx = build_context(&form);

        assert!(ctx.contains("X=240  Y=-30"), "context: {ctx}");
        assert!(ctx.contains("StartPosition=\"BottomRight\""), "context: {ctx}");
        for value in cobolt_forms::model::FormStartPosition::ALL {
            assert!(
                ctx.contains(value.as_str()),
                "CONTEXT must advertise StartPosition {value:?}"
            );
        }
        assert!(
            ctx.contains("ignores X/Y") || ctx.to_ascii_lowercase().contains("ignore"),
            "the context must warn that X/Y alone do not move the window: {ctx}"
        );
        assert!(form_property_valid("X"));
        assert!(form_property_valid("Y"));
        assert!(form_property_valid("StartPosition"));
    }

    /// The context must carry the ACTUAL bound handler code, not merely which
    /// event names a control's TYPE supports — otherwise a task that reads or
    /// describes existing behavior has nothing to read. Observed live: asked
    /// to write a caption per TextBox from "the event handler logic", the
    /// specialist wrote the SAME text on all fifteen, because the code was
    /// never in its context to read from (operator, 2026-07-31).
    #[test]
    fn context_carries_the_actual_bound_handler_code() {
        let mut form = Form::new("F", "T", 400, 300);
        let mut txt1 = Control::new("TXT1", ControlType::TextBox, 0, 0);
        txt1.events.push(cobolt_forms::model::EventBinding {
            event: "onEnterPressed".into(),
            paragraph: "TXT1--ONENTERPRESSED".into(),
            code: "       PROCEDURE DIVISION.\n           MOVE \"#0000FF\" TO TXT1::BackgroundColor.".into(),
        });
        // An unwritten handler (empty code, the template state) must not
        // appear as if it were real behavior.
        txt1.events.push(cobolt_forms::model::EventBinding {
            event: "onClick".into(),
            paragraph: "TXT1--ONCLICK".into(),
            code: String::new(),
        });
        form.controls.push(txt1);
        form.form_events.push(cobolt_forms::model::EventBinding {
            event: "onLoad".into(),
            paragraph: "F--ONLOAD".into(),
            code: "       PROCEDURE DIVISION.\n           DISPLAY \"loaded\".".into(),
        });

        let ctx = build_context(&form);
        assert!(ctx.contains("EVENT HANDLERS"), "the section must be present");
        assert!(ctx.contains("TXT1::onEnterPressed"));
        assert!(ctx.contains("#0000FF"), "the actual code must be inlined, not summarized");
        assert!(!ctx.contains("TXT1::onClick"), "an unwritten handler is not behavior");
        assert!(ctx.contains("FORM EVENT HANDLERS"));
        assert!(ctx.contains("DISPLAY \"loaded\""));

        // A form with no real handlers yet must not gain an empty section —
        // ordinary forms outnumber ones under analysis, and an empty heading
        // is a red herring, not information.
        let bare = Form::new("F2", "T", 400, 300);
        let ctx2 = build_context(&bare);
        assert!(!ctx2.contains("EVENT HANDLERS"));
    }

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

    /// Phase 3: a change-set recovered by typed extraction is re-encoded
    /// canonically and appended as a submission — that encoding must parse
    /// back deterministically, or recovery would loop.
    #[test]
    fn canonical_serialization_round_trips_through_parse() {
        let reply = r#"```json
{ "operations": [
  { "op": "deploy_control", "control_type": "Button", "id": "SAVE",
    "properties": { "Caption": "Save", "X": 10 } },
  { "op": "set_property", "control_id": "L1", "key": "Bold", "value": true },
  { "op": "generate_event_handler", "control_id": "SAVE", "event": "onClick", "code": "x" },
  { "op": "create_procedure", "name": "P", "code": "y" },
  { "op": "message", "message": "hi" }
] }
```"#;
        let cs = parse_change_set(reply).expect("should parse");
        let json = serde_json::to_string_pretty(&cs).expect("serializes");
        let back = parse_change_set(&format!("```json\n{json}\n```")).expect("round-trips");
        assert_eq!(cs, back);
    }

    /// The extraction schema generates (guards the schemars derives).
    #[test]
    fn change_set_schema_generates() {
        let schema = schemars::schema_for!(AgentChangeSet);
        let text = serde_json::to_string(&schema).expect("schema serializes");
        assert!(text.contains("operations"));
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

    /// A control that casts a drop shadow must expose its COMPLETE shadow group
    /// in CONTEXT, including members still at the type default. Otherwise Grace
    /// cannot copy a value like ShadowDirection="SouthEast" that it never sees —
    /// the exact reason a "copy this control's drop shadow" request dropped the
    /// direction while every other shadow property was applied.
    #[test]
    fn shadowed_control_surfaces_full_shadow_group_even_at_default() {
        use cobolt_forms::{Control, PropValue};
        // A chart with a shadow turned on but its direction left at the default.
        let mut chart = Control::new("BarChart-1", ControlType::BarChart, 0, 0);
        chart.set_prop("ShadowEnabled", PropValue::Bool(true));
        chart.set_prop("ShadowDistance", PropValue::Int(11));
        assert_eq!(
            chart.get_prop("ShadowDirection").unwrap().as_str(),
            "SouthEast",
            "precondition: direction sits at the type default"
        );

        let props = non_default_props(&chart);
        assert!(
            props.iter().any(|p| p == "ShadowDirection=\"SouthEast\""),
            "default-valued ShadowDirection must be surfaced for a shadowed \
             control so it can be copied; got {props:?}"
        );

        // A control WITHOUT a shadow must not have the group forced in — the
        // context stays compact for the common shadow-off case.
        let plain = Control::new("L1", ControlType::Label, 0, 0);
        assert!(
            !non_default_props(&plain)
                .iter()
                .any(|p| p.starts_with("ShadowDirection=")),
            "no shadow ⇒ default shadow members stay hidden"
        );
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

    /// The program wrapper belongs to the IDE: a body that writes it declares a
    /// second program inside the nest and the form stops compiling.
    #[test]
    fn a_body_may_not_write_the_scaffold_the_ide_generates() {
        let body = |tail: &str| {
            format!(
                "       ENVIRONMENT DIVISION.\n       DATA DIVISION.\n       \
                 PROCEDURE DIVISION.\n           MOVE SPACES TO WS-X.\n{tail}"
            )
        };
        for tail in [
            "       END PROGRAM UPDATE-CONCATENATION.\n",
            "       IDENTIFICATION DIVISION.\n",
            "       PROGRAM-ID. UPDATE-CONCATENATION.\n",
        ] {
            let error = handler_body_shape_error(&body(tail))
                .unwrap_or_else(|| panic!("the scaffold must be rejected: {tail:?}"));
            assert!(error.contains("the IDE generates"), "{error}");
        }

        // …while a body that leaves the wrapper alone passes, and a GOBACK
        // inside a literal or a comment is just text.
        assert!(handler_body_shape_error(&body("           CONTINUE.\n")).is_none());
        assert!(handler_body_shape_error(&body(
            "           MOVE \"GOBACK\" TO WS-VERB.\n           *> GOBACK is the IDE's.\n"
        ))
        .is_none());
    }

    /// `GOBACK` is an ordinary statement, not scaffold. A handler that declares
    /// paragraphs for its own `PERFORM`s must end its main flow with one, or
    /// control falls through and runs them again — so the gate must let it
    /// past. It was rejected only while the lexer lacked the keyword and a lone
    /// `GOBACK.` was read as a duplicate paragraph name.
    #[test]
    fn a_body_may_end_its_main_flow_with_goback() {
        let with_own_paragraph = "       ENVIRONMENT DIVISION.\n       DATA DIVISION.\n       \
             WORKING-STORAGE SECTION.\n       01 WS-N PIC 9(3) VALUE 0.\n\n       \
             PROCEDURE DIVISION.\n           PERFORM CHECK-RANGE.\n           GOBACK.\n       \
             CHECK-RANGE.\n           ADD 1 TO WS-N.\n";
        assert!(
            handler_body_shape_error(with_own_paragraph).is_none(),
            "a GOBACK ending the main flow is valid COBOL-85, not scaffold"
        );
        assert!(handler_body_shape_error(
            "       ENVIRONMENT DIVISION.\n       DATA DIVISION.\n       \
             PROCEDURE DIVISION.\n           MOVE SPACES TO WS-X.\n           GOBACK.\n"
        )
        .is_none());
    }

    /// Operator report (2026-07-30): a keyboard handler binding its event
    /// payload — `PROCEDURE DIVISION USING KEY-CODE.` — was rejected with
    /// "Code must include PROCEDURE DIVISION.", an instruction it had already
    /// followed. The agent resubmitted the same code three times and the
    /// workflow burned its correction budget creating nothing. A division
    /// header may carry a phrase and an inline comment.
    #[test]
    fn division_headers_may_carry_a_phrase_or_a_comment() {
        let with_using = "       ENVIRONMENT DIVISION.\n       DATA DIVISION.\n       LINKAGE SECTION.\n       01  KEY-CODE   PIC S9(4) COMP-5.\n\n       PROCEDURE DIVISION USING KEY-CODE.\n           CONTINUE.\n";
        assert!(
            handler_body_shape_error(with_using).is_none(),
            "USING phrase must be accepted: {:?}",
            handler_body_shape_error(with_using)
        );
        let with_comment = "       ENVIRONMENT DIVISION.\n       DATA DIVISION. *> sem itens\n       PROCEDURE DIVISION.\n           CONTINUE.\n";
        assert!(
            handler_body_shape_error(with_comment).is_none(),
            "inline comment must be accepted: {:?}",
            handler_body_shape_error(with_comment)
        );
        // A genuinely missing header is still caught, and the message now
        // shows the accepted phrase form.
        assert!(handler_body_shape_error(
            "       ENVIRONMENT DIVISION.\n       DATA DIVISION.\n           CONTINUE.\n"
        )
        .is_some_and(|m| m.contains("PROCEDURE DIVISION") && m.contains("USING")));
    }

    /// The workflow lint gate (spec: Grace correction loop) must catch exactly
    /// the operations the apply path would silently skip — observed live as 60
    /// placeholder hover handlers (`*> comment` + `CONTINUE.`, no division
    /// headers) that validated invalid, applied nothing, and told no one.
    #[test]
    fn lint_flags_placeholder_handlers_for_change_set_agents() {
        let placeholder = "Analysis prose here.\n```json\n{\"operations\":[\
            {\"op\":\"generate_event_handler\",\"control_id\":\"LBL-01\",\"event\":\"onHoverEnter\",\
             \"code\":\"*> Placeholder - implementação pendente\\n           CONTINUE.\"},\
            {\"op\":\"generate_event_handler\",\"control_id\":\"LBL-02\",\"event\":\"onHoverLeave\",\
             \"code\":\"*> Placeholder\\n           CONTINUE.\"}]}\n```";
        let errors = lint_change_set_submission("Form Designer Agent", placeholder)
            .expect("placeholder bodies must be flagged");
        assert!(errors.contains("1. generate_event_handler LBL-01.onHoverEnter"), "{errors}");
        assert!(errors.contains("2. generate_event_handler LBL-02.onHoverLeave"), "{errors}");
        assert!(errors.contains("ENVIRONMENT DIVISION"), "{errors}");

        // The event specialist's submissions are linted through the same gate.
        assert!(
            lint_change_set_submission("COBOL Event Handler Script Agent", placeholder).is_some()
        );

        // A complete nested-program body passes.
        let full = "```json\n{\"operations\":[\
            {\"op\":\"generate_event_handler\",\"control_id\":\"LBL-01\",\"event\":\"onHoverEnter\",\
             \"code\":\"       ENVIRONMENT DIVISION.\\n       DATA DIVISION.\\n       PROCEDURE DIVISION.\\n           CONTINUE.\"}]}\n```";
        assert!(lint_change_set_submission("Form Designer Agent", full).is_none());

        // Unknown deploy types and invalid deploy properties are Form-free too.
        let bad_deploy = "```json\n{\"operations\":[\
            {\"op\":\"deploy_control\",\"control_type\":\"HoloPanel\",\"id\":\"H1\",\"properties\":{}}]}\n```";
        let errors = lint_change_set_submission("Form Designer Agent", bad_deploy)
            .expect("unknown control type must be flagged");
        assert!(errors.contains("HoloPanel"), "{errors}");
    }

    /// Operator rule (2026-07-30): a correction round must fix ONLY what was
    /// found wrong and KEEP what was right. A specialist told to resubmit
    /// everything rewrites operations nobody complained about — observed
    /// live, where three malformed handlers were flagged and the model
    /// silently reimplemented a fourth, correct one on the way past.
    #[test]
    fn a_correction_keeps_the_accepted_operations_and_scopes_the_rest() {
        let body = |s: &str| {
            format!(
                "       ENVIRONMENT DIVISION.\\n       DATA DIVISION.\\n       PROCEDURE DIVISION.\\n           {s}"
            )
        };
        let submission = format!(
            "prose\n```json\n{{\"operations\":[\
             {{\"op\":\"generate_event_handler\",\"control_id\":\"A1\",\"event\":\"onClick\",\"code\":\"{good}\"}},\
             {{\"op\":\"generate_event_handler\",\"control_id\":\"A2\",\"event\":\"onClick\",\"code\":\"CONTINUE.\"}},\
             {{\"op\":\"generate_event_handler\",\"control_id\":\"A3\",\"event\":\"onClick\",\"code\":\"{good}\"}}]}}\n```",
            good = body("CONTINUE.")
        );

        // Machine validation attributes the defect itself: only A2 goes back.
        let (accepted, defective, kept, redo) =
            split_change_set_submission("Form Designer Agent", &submission, &[])
                .expect("a mixed change-set must be scoped");
        assert_eq!((kept, redo), (2, 1));
        assert!(defective.contains("A2") && !defective.contains("A1"));
        assert!(accepted.contains("A1") && accepted.contains("A3") && !accepted.contains("A2"));

        // A Pedantic reviewer can attribute it instead, naming the operation
        // loosely (just the control) — the match still lands.
        let (_, defective, kept, redo) = split_change_set_submission(
            "Form Designer Agent",
            &submission,
            &["a3".to_string()],
        )
        .expect("reviewer-attributed defects must be scoped");
        assert_eq!((kept, redo), (2, 1));
        assert!(defective.contains("A3") && !defective.contains("A1"));

        // The corrected operation is spliced back onto the kept ones.
        let corrected = format!(
            "fixed\n```json\n{{\"operations\":[\
             {{\"op\":\"generate_event_handler\",\"control_id\":\"A2\",\"event\":\"onClick\",\"code\":\"{good}\"}}]}}\n```",
            good = body("MOVE 1 TO WS-X.")
        );
        let (accepted, _, _, _) =
            split_change_set_submission("Form Designer Agent", &submission, &[]).unwrap();
        let merged = merge_change_sets(&accepted, &corrected).expect("merge");
        let cs = parse_change_set(&merged).expect("merged set parses");
        assert_eq!(cs.operations.len(), 3, "every operation survives the merge");
        assert!(merged.contains("MOVE 1 TO WS-X"), "the fix landed");
        assert!(
            lint_change_set_submission("Form Designer Agent", &merged).is_none(),
            "the merged set passes the gate"
        );

        // A specialist that ignores the instruction and resubmits everything
        // still merges: its version supersedes, nothing is duplicated.
        let full_resubmit = format!(
            "```json\n{{\"operations\":[\
             {{\"op\":\"generate_event_handler\",\"control_id\":\"A1\",\"event\":\"onClick\",\"code\":\"{good}\"}},\
             {{\"op\":\"generate_event_handler\",\"control_id\":\"A2\",\"event\":\"onClick\",\"code\":\"{good}\"}},\
             {{\"op\":\"generate_event_handler\",\"control_id\":\"A3\",\"event\":\"onClick\",\"code\":\"{good}\"}}]}}\n```",
            good = body("CONTINUE.")
        );
        let merged = merge_change_sets(&accepted, &full_resubmit).expect("merge");
        assert_eq!(parse_change_set(&merged).unwrap().operations.len(), 3);

        // Nothing to keep, or nothing wrong ⇒ no scoping, and the engine asks
        // for a full replacement exactly as before.
        let all_bad = "```json\n{\"operations\":[\
            {\"op\":\"generate_event_handler\",\"control_id\":\"B1\",\"event\":\"onClick\",\"code\":\"CONTINUE.\"}]}\n```";
        assert!(split_change_set_submission("Form Designer Agent", all_bad, &[]).is_none());
        let all_good = format!(
            "```json\n{{\"operations\":[\
             {{\"op\":\"generate_event_handler\",\"control_id\":\"B1\",\"event\":\"onClick\",\"code\":\"{good}\"}}]}}\n```",
            good = body("CONTINUE.")
        );
        assert!(split_change_set_submission("Form Designer Agent", &all_good, &[]).is_none());
        // And an agent that produces no change-set is never scoped.
        assert!(split_change_set_submission("Documentation Agent", &submission, &[]).is_none());
        println!("correction scoping: 3 operations, 1 defective, 2 kept, merged back to 3");
    }

    /// The gate never speaks for agents whose output is not a change-set, for
    /// submissions without a parseable change-set (recovery is a later,
    /// separate concern), or for Form-dependent doubts (unsaved designer state
    /// would make those false positives).
    #[test]
    fn lint_stays_silent_outside_its_proof() {
        let placeholder = "```json\n{\"operations\":[\
            {\"op\":\"generate_event_handler\",\"control_id\":\"L1\",\"event\":\"onClick\",\
             \"code\":\"CONTINUE.\"}]}\n```";
        assert!(
            lint_change_set_submission("Documentation Agent", placeholder).is_none(),
            "not a change-set producer"
        );
        assert!(
            lint_change_set_submission("Form Designer Agent", "prose only, no JSON").is_none(),
            "no parseable change-set"
        );
        // A set_property on a control the lint cannot see is a Form-dependent
        // question — never flagged here.
        let set_prop = "```json\n{\"operations\":[\
            {\"op\":\"set_property\",\"control_id\":\"GHOST-1\",\"key\":\"Width\",\"value\":150}]}\n```";
        assert!(lint_change_set_submission("Form Designer Agent", set_prop).is_none());
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

    /// A property listed by bare name only told the model it EXISTS, never
    /// what values are legal to set it to — Grace flagged exactly this during
    /// a prompt review: the context named `BorderStyle` as a TextBox property
    /// but did not say what values it accepts (operator, 2026-08-01). Anchored
    /// on `BorderStyle` specifically, since `border_rows` in
    /// `panels/properties.rs` and `draw_control_border` in
    /// `cobolt-forms::paint` are the ground truth this domain has to match:
    /// `None` | `Single` | `Fixed3D` | `Raised` | `Sunken`.
    #[test]
    fn context_lists_property_value_domains() {
        let ctx = build_context(&form_with_label());
        assert!(
            ctx.contains("PROPERTY VALUE DOMAINS"),
            "missing domains section: {ctx}"
        );
        assert!(
            ctx.contains("BorderStyle: one of: `None` | `Single` | `Fixed3D` | `Raised` | `Sunken`"),
            "BorderStyle domain not listed: {ctx}"
        );
    }

    /// The exact question this closes: "what entrance effect does the project
    /// use, and how long does it run?" had no live value to answer from — a
    /// form only carries the on/off `WindowEffects` switch; the effect,
    /// duration, and easing are project-wide, in `[forms]` of the project
    /// file, on `CoboltProject`, which the form-only `build_context` never
    /// sees. Values match a real project's `[forms]` section (operator,
    /// 2026-07-31) so this doubles as the regression anchor for that report.
    #[test]
    fn context_carries_the_projects_actual_configured_window_effects() {
        let mut project = crate::project_model::CoboltProject::new("Demo", "src/main.cbl");
        project.forms.entrance_effect = "matrix-rain".into();
        project.forms.entrance_ms = 4000;
        project.forms.entrance_easing = "ease-in-out".into();
        project.forms.exit_effect = "zoom".into();
        project.forms.exit_ms = 400;
        project.forms.exit_easing = "ease-in".into();
        project.forms.entrance_on_restore = true;

        let ctx = build_context_with_project(&form_with_label(), Some(&project), None);
        assert!(ctx.contains("WINDOW EFFECTS"), "context: {ctx}");
        assert!(ctx.contains("Entrance: matrix-rain, 4000 ms, ease-in-out easing"));
        assert!(ctx.contains("Exit: zoom, 400 ms, ease-in easing"));
        assert!(ctx.contains("Replay entrance when a window is restored after minimize: true"));

        // A project whose file predates spec 038 (no `[forms]` section at
        // all, so `FormsConfig::default()` is what serde falls back to) still
        // gets an honest, explicit "none" — not a section that silently
        // disappears and reads as "no data" rather than "no effect".
        let mut pre038 = crate::project_model::CoboltProject::new("Pre038", "src/main.cbl");
        pre038.forms = Default::default();
        let ctx2 = build_context_with_project(&form_with_label(), Some(&pre038), None);
        assert!(ctx2.contains("Entrance: none"), "context: {ctx2}");
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

    /// The workflow's machine gate must name EVERY bad event name at once: the
    /// reviewer finds them one per round, and a three-round correction budget
    /// spent on spelling is a workflow that never reaches the code.
    #[test]
    fn an_event_no_control_type_has_is_caught_before_the_reviewer() {
        let body = "       ENVIRONMENT DIVISION.\n       DATA DIVISION.\n       \
                    PROCEDURE DIVISION.\n           CONTINUE.\n";
        let handler = |ctrl: &str, event: &str| AgentOp::GenerateEventHandler {
            control_id: ctrl.into(),
            event: event.into(),
            code: body.into(),
        };
        let cs = AgentChangeSet {
            operations: vec![
                handler("TXT-2", "onFocus"),
                handler("TXT-4", "keyboard"),
                handler("TXT-1", "onChange"),
            ],
            note: None,
        };
        let report = lint_change_set_submission(crate::agents_db::EVENT_HANDLER, &change_set_json(&cs))
            .expect("the gate must reject");
        assert!(report.contains("onFocus"), "{report}");
        assert!(report.contains("keyboard"), "{report}");
        assert!(
            !report.contains("onChange"),
            "a real event must not be flagged: {report}"
        );
        // The suggestion is what turns a rejection into a fix.
        assert!(report.contains("onGotFocus"), "no suggestion offered: {report}");
    }

    fn change_set_json(cs: &AgentChangeSet) -> String {
        format!(
            "```json\n{}\n```",
            serde_json::to_string(&serde_json::json!({ "operations": cs.operations })).unwrap()
        )
    }

    /// A handler bound to an event the control does not have is skipped at
    /// apply. Silently, until now: the workflow reported success and the form
    /// gained nothing. `discarded_ops` is what the developer is told instead.
    #[test]
    fn a_handler_on_a_nonexistent_event_is_reported_not_swallowed() {
        let body = "       ENVIRONMENT DIVISION.\n       DATA DIVISION.\n       \
                    PROCEDURE DIVISION.\n           CONTINUE.\n";
        let cs = AgentChangeSet {
            operations: vec![
                AgentOp::DeployControl {
                    control_type: "TextBox".into(),
                    id: Some("TXT-1".into()),
                    parent_id: None,
                    parent: None,
                    properties: serde_json::Map::new(),
                },
                AgentOp::GenerateEventHandler {
                    control_id: "TXT-1".into(),
                    // The real name is onGotFocus — this is the guess that cost
                    // the developer a whole workflow's worth of events.
                    event: "onFocus".into(),
                    code: body.into(),
                },
                AgentOp::GenerateEventHandler {
                    control_id: "TXT-1".into(),
                    event: "onChange".into(),
                    code: body.into(),
                },
            ],
            note: None,
        };
        let form = form_with_label();
        let discarded = discarded_ops(&cs, &form);
        assert_eq!(discarded.len(), 1, "only the bad event is dropped: {discarded:?}");
        assert!(
            discarded[0].contains("TXT-1.onFocus") && discarded[0].contains("no event 'onFocus'"),
            "the report must name the operation and the reason: {discarded:?}"
        );
    }

    // ── The handler lint and runtime-only properties ─────────────────────────

    /// A property the runtime writes and the guide tells developers to read must
    /// not be reported as one the control does not have.
    ///
    /// `TOOLBAR-1::LastButton` is printed verbatim in the Developer's Guide, and
    /// the lint rejected it (operator, 2026-09-01). `property_readable` unions
    /// the *seeded* properties with `runtime_property_names_for`, and a
    /// runtime-only property is by definition never seeded — so when that list
    /// was empty for a control, every correct reference to one of its runtime
    /// answers read as a hallucination.
    #[test]
    fn a_runtime_only_property_is_readable_by_a_handler() {
        for (ct, prop) in [
            (ControlType::ToolBar, "LastButton"),
            (ControlType::FileDropZone, "DroppedFiles"),
            (ControlType::FileDropZone, "RejectedFiles"),
            (ControlType::FileDropZone, "StagedFiles"),
            (ControlType::FileDropZone, "CommitSummary"),
            (ControlType::Snackbar, "LastButtonId"),
            (ControlType::RestClient, "ResponseBody"),
            (ControlType::Maps, "SelectedMarkerId"),
        ] {
            assert!(
                property_readable(&ct, prop),
                "{ct:?}::{prop} is written by the runtime and documented, so reading \
                 it from a handler must not be reported as a missing property"
            );
        }
    }

    /// The union must not become a rubber stamp: a name nothing writes is still
    /// a mistake worth reporting.
    #[test]
    fn an_invented_property_is_still_rejected() {
        assert!(
            !property_readable(&ControlType::ToolBar, "LastBanana"),
            "the lint must still catch a property that does not exist"
        );
        assert!(
            !property_readable(&ControlType::Button, "LastButton"),
            "LastButton belongs to ToolBar; a Button has no such property"
        );
    }
}
