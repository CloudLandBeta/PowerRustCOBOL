// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Runtime tool-execution layer for Grace's specialist agents (spec 030).
//!
//! Grace's [`GraceEngine`](cobolt_agents::grace::GraceEngine) drives every
//! specialist and reviewer through a single
//! [`AgentInvoker`](cobolt_agents::grace::AgentInvoker) call and only reads back
//! a final text submission. We keep that contract — and keep `cobolt-agents`
//! free of IDE/tool concerns — by making tool execution a **decorator** on the
//! host's invoker.
//!
//! [`ToolExecutingInvoker`] wraps any inner invoker and runs a bounded loop:
//! call the model → parse a trailing `{"tool_calls":[…]}` block → if none, the
//! reply is the final submission; otherwise execute each call through a
//! [`ToolBackend`], thread the real results back as a new turn, and loop.
//!
//! Governance (spec 029 R4 / spec 030 R2, R3): a call naming a tool **not
//! declared** for the invoking agent is a *critical defect* that fails the task
//! — never silently completed. Because every executed result is real backend
//! output threaded back into the conversation, "done without evidence" is
//! structurally impossible for tool work. Each executed call is recorded as
//! [`ToolEvidence`] for the workflow record (R11).

use std::collections::HashSet;
use std::path::{Component, Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use cobolt_agents::grace::{last_json_block, AgentInvoker, ReviewVerdict, WorkflowPlan};

use crate::git_exec::{self, GitClass, GitConfirmRequest};
use crate::target_select::{TargetChoice, TargetRequest};

/// A single tool invocation requested by an agent, parsed from its reply.
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct ToolCall {
    /// Fully-qualified tool name, e.g. `git.status` or `egui.tree`.
    pub tool: String,
    /// Free-form arguments (backend-specific). Absent → JSON null.
    #[serde(default)]
    pub args: serde_json::Value,
}

/// The outcome of executing one [`ToolCall`].
#[derive(Debug, Clone, PartialEq)]
pub struct ToolResult {
    /// The tool ran and succeeded.
    pub ok: bool,
    /// One-line human summary (goes into the evidence + the model's next turn).
    pub summary: String,
    /// Fuller detail threaded back to the agent (command output, tree census…).
    pub detail: String,
    /// A governance/critical defect (undeclared or fabricated tool use, etc.):
    /// the task must fail, not continue (spec 030 R2/R3).
    pub critical: bool,
    /// The operation is blocked pending an explicit developer confirmation
    /// (e.g. a destructive recreate of a finalized indexed file). Unlike a
    /// defect, this is not the agent's fault to fix: the tool loop stops at
    /// once and surfaces `summary` to the developer as Grace's reply, so the
    /// developer can confirm or cancel.
    pub needs_confirmation: bool,
}

impl ToolResult {
    pub fn ok(summary: impl Into<String>, detail: impl Into<String>) -> Self {
        Self {
            ok: true,
            summary: summary.into(),
            detail: detail.into(),
            critical: false,
            needs_confirmation: false,
        }
    }
    /// A recoverable failure (the tool ran but reported an error, e.g. a
    /// non-zero git exit). The agent sees it and may adjust.
    pub fn err(summary: impl Into<String>, detail: impl Into<String>) -> Self {
        Self {
            ok: false,
            summary: summary.into(),
            detail: detail.into(),
            critical: false,
            needs_confirmation: false,
        }
    }
    /// A critical defect that must fail the whole task.
    pub fn critical(summary: impl Into<String>) -> Self {
        let s = summary.into();
        Self {
            ok: false,
            detail: s.clone(),
            summary: s,
            critical: true,
            needs_confirmation: false,
        }
    }
    /// The task cannot proceed without the developer's explicit go-ahead. The
    /// tool loop returns `summary` straight to the chat as Grace's reply — no
    /// fix-and-repeat, no failure — so the developer decides.
    pub fn needs_confirmation(summary: impl Into<String>) -> Self {
        let s = summary.into();
        Self {
            ok: false,
            detail: s.clone(),
            summary: s,
            critical: false,
            needs_confirmation: true,
        }
    }
}

/// One recorded tool execution, persisted alongside the workflow record
/// (spec 030 R11 observability).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolEvidence {
    pub agent: String,
    pub tool: String,
    /// A truncated digest of the call arguments (keeps the record compact).
    pub args_digest: String,
    pub summary: String,
    pub ok: bool,
    /// Unix seconds.
    pub ts: u64,
}

/// Executes a declared tool call against a real backend. Declared-tools
/// governance is enforced by [`ToolExecutingInvoker`] *before* dispatch; the
/// backend routes by tool name and returns a [`ToolResult`].
pub trait ToolBackend {
    fn execute(&mut self, agent: &str, call: &ToolCall) -> ToolResult;
}

/// Parse a trailing fenced `{"tool_calls":[…]}` JSON block from an agent reply.
///
/// - `Ok(None)` — no tool-call block (the reply is a final answer / change-set).
/// - `Ok(Some(calls))` — a non-empty, well-formed tool-call list.
/// - `Err(_)` — a tool-call block was present but malformed (never silently
///   swallowed).
pub fn parse_tool_calls(reply: &str) -> Result<Option<Vec<ToolCall>>, String> {
    let Some(v) = last_json_block(reply) else {
        // No *valid* JSON block. If the agent clearly meant to emit tool calls
        // but produced invalid JSON, surface it rather than treating it as a
        // final answer.
        if reply.contains("\"tool_calls\"") {
            return Err("a tool_calls block was present but was not valid JSON".into());
        }
        return Ok(None);
    };
    let Some(tc) = v.get("tool_calls") else {
        return Ok(None); // valid JSON, but a final result — not a tool call
    };
    let mut calls: Vec<ToolCall> = serde_json::from_value(tc.clone())
        .map_err(|e| format!("malformed tool_calls block: {e}"))?;
    if calls.is_empty() {
        return Ok(None);
    }
    for call in &mut calls {
        unwrap_double_nested_args(call);
        strip_quoted_keys(&mut call.args);
    }
    Ok(Some(calls))
}

/// Models sometimes emit object keys wrapped in LITERAL quote characters —
/// observed live from gemma: `"normalization": {"\"1nf\"": "…"}` (the key is
/// `"1nf"`, quotes included), which fails the backend's `1nf/2nf/3nf`
/// validation. No host tool has argument keys containing quote characters, so
/// stripping them recursively is always safe.
fn strip_quoted_keys(value: &mut serde_json::Value) {
    match value {
        serde_json::Value::Object(map) => {
            let quoted: Vec<String> = map
                .keys()
                .filter(|key| {
                    let trimmed = key.trim_matches('"');
                    trimmed != key.as_str() && !trimmed.is_empty()
                })
                .cloned()
                .collect();
            for key in quoted {
                let trimmed = key.trim_matches('"').to_string();
                if !map.contains_key(&trimmed) {
                    if let Some(inner) = map.remove(&key) {
                        map.insert(trimmed, inner);
                    }
                }
            }
            for (_key, inner) in map.iter_mut() {
                strip_quoted_keys(inner);
            }
        }
        serde_json::Value::Array(items) => {
            for inner in items {
                strip_quoted_keys(inner);
            }
        }
        _ => {}
    }
}

/// Models sometimes double-wrap tool arguments — observed live from gemma:
/// `{"tool":"indexed_file.write","args":{"args":{…real payload…}}}`. No host
/// tool has a single `"args"` object as its own argument, so unwrapping one
/// level is always safe and saves an otherwise perfect call from failing as
/// malformed.
fn unwrap_double_nested_args(call: &mut ToolCall) {
    if let serde_json::Value::Object(map) = &call.args {
        if map.len() == 1 {
            if let Some(inner @ serde_json::Value::Object(_)) = map.get("args") {
                call.args = inner.clone();
            }
        }
    }
}

const GIT_TOOL_CONTRACT: &str = "\n\n--- Tool execution (git) — how you actually run git ---\nYou run git through one tool. To execute a command, END your reply with exactly one fenced JSON block and nothing after it:\n```json\n{\"tool_calls\":[{\"tool\":\"git.run\",\"args\":{\"argv\":[\"status\",\"--porcelain\"]}}]}\n```\n`argv` is the git argument vector WITHOUT the leading \"git\" (e.g. [\"commit\",\"-m\",\"message\"]). Do not pass -C, --git-dir, or --work-tree — the executor is already bound to the open project's repository and will reject them. Read and local-mutation ops run immediately; network and history-rewriting ops (push, fetch, pull, rebase, reset --hard) are GATED on explicit operator approval and will not run if declined. Each command returns its real exit status and output as a TOOL RESULTS block — a non-zero exit is a failure, never a success. When the task is complete, reply with your final result and DO NOT emit a tool_calls block.";

const EGUI_TOOL_CONTRACT: &str = "\n\n--- Live UI inspection (observe only) ---\nYou may inspect the rendered form to verify your work by calling the native function tools `egui_tree` (widget tree) or `egui_rects` (geometry). They are READ-ONLY: they let you SEE the rendered UI; they do NOT change it. Every form edit must be expressed as change-set operations, never by driving the live UI. Call the tool directly — do not describe the call in prose or emit any fenced tool_calls JSON block.";

const DOCUMENTATION_TOOL_CONTRACT: &str = "\n\n--- Tool execution (project Knowledge Base) ---\nYou create and inspect project Knowledge Base documents through these tools. To write a Markdown document, END your reply with exactly one fenced JSON block and nothing after it:\n```json\n{\"tool_calls\":[{\"tool\":\"documentation.write\",\"args\":{\"path\":\"/Knowledge Base/Projects/example.md\",\"content\":\"# Example\\n\"}}]}\n```\nUse documentation.list with empty args and documentation.read with {\"path\":\"/Knowledge Base/...\"}. Writes are restricted to the open project's Knowledge Base/ folder and are automatically indexed in the project's SQLite vector database. A document exists only after a successful TOOL RESULTS response. When finished, reply with your final result and DO NOT emit a tool_calls block.";

const KNOWLEDGE_TOOL_CONTRACT: &str = "\n\n--- Project knowledge retrieval ---\nUse the native function tool `knowledge_search` (arguments: query, optional limit 1-10) to consult the project-local SQLite vector index before relying on prior plans, requirements, task lists, or other documentation. Use only returned project paths and excerpts as evidence. Call the tool directly — do not describe the call in prose or emit any fenced tool_calls JSON block.";

const INDEXED_FILE_TOOL_CONTRACT: &str = r#"

--- Tool execution (PowerRustCOBOL Indexed File UI model) ---
Only Data (Indexed File) Agent may inspect or mutate indexed-file definitions through these tools.

List with `indexed_file.list` and empty args. Read with `indexed_file.read` and `{"path":"indexed/customers.cidx"}`.

Create or replace one complete definition with `indexed_file.write`:
```json
{"tool_calls":[{"tool":"indexed_file.write","args":{"path":"indexed/customers.cidx","name":"CUSTOMER-FILE","purpose":"Customer master data for invoicing","assign_path":"data/customers.idx","record":"       01 CUSTOMER-RECORD.\n          05 CUSTOMER-ID PIC X(36).\n          05 CUSTOMER-NAME PIC X(80).","primary_key":"CUSTOMER-ID","alternate_keys":[],"access_mode":"dynamic","storage":"disk","id_definitions":{"CUSTOMER-ID":"UUID"},"normalization":{"1nf":"Atomic customer attributes; no repeating groups.","2nf":"Single-field primary key; no partial dependencies.","3nf":"All non-key fields depend only on CUSTOMER-ID."}}}]}
```
`alternate_keys` entries use `{"field":"FIELD-NAME","duplicates":false}`. Set `finalized` explicitly when needed; new definitions default to finalized after validation. Every ID field requires an `id_definitions` entry whose value is either `UUID` or the exact `PIC ...` chosen by the developer. The `normalization` object must contain non-empty `1nf`, `2nf`, and `3nf` decisions from the approved Documentation Agent handoff. The write validates the COBOL record and key fields, saves the `.cidx`, and regenerates Indexed File UI COBOL/copybook artifacts. A helper normalized relation is a separate `indexed_file.write` call. Use TOOL RESULTS as evidence and never claim a file changed without a successful result.

A finalized definition is LOCKED: any structural change (new/removed field, changed PIC, changed keys or storage) is refused and the write returns a confirmation-required result asking the developer to authorize destroying and recreating the file. Do NOT retry such a write unchanged and do NOT set `confirm_recreate` on your own. Only after Grace relays that the DEVELOPER explicitly confirmed the destroy-and-recreate may you repeat the write with `"confirm_recreate": true`, which overwrites the locked file with the new schema (its stored data is lost)."#;

// spec 034 rollback: retained for a future redesign but no longer appended
// (see `tool_contract_appendix`). Forms are edited via change-sets on the open
// form, so name→path resolution does not fit that flow.
#[allow(dead_code)]
const PROJECT_TOOL_CONTRACT: &str = "\n\n--- Target selection (project tree, spec 034) ---\nBefore you CREATE or EDIT a named project element (a form, indexed file, common-code source, documentation file, or asset), you MUST first resolve WHICH target the developer means, because folders allow several elements to share a name. Call `project.select_target` with `{\"op\":\"create\"|\"edit\",\"kind\":\"form\"|\"indexed\"|\"source\"|\"documentation\"|\"asset\",\"name\":\"<the element name>\"}`. The TOOL RESULT returns one project-relative path: for `create` it is the destination FOLDER the developer picked (place the new element inside it); for `edit` it is the exact element FILE to modify. Use that returned path verbatim in your subsequent write. A `create` always prompts the developer; an `edit` prompts only when the name is ambiguous and otherwise returns the single match. If the result is a cancellation, STOP and do not create or edit anything.";

/// How a Form Designer submission must encode its edits. Only this JSON shape
/// is parsed and applied (`crate::agent::parse_change_set`); prose, tables, or
/// invented operation names apply nothing. Appended to the Form Designer's
/// system prompt at delegation time so the schema it is told to emit is the
/// schema the applier actually accepts.
pub const CHANGE_SET_CONTRACT: &str = "\n\n--- Change-set contract (how your edits are applied) ---\nYour edits take effect ONLY when your final reply ends with exactly one fenced JSON block of this shape:\n```json\n{\"operations\": [ /* zero or more operations, applied in order */ ]}\n```\nEach operation is exactly one of:\n- `{\"op\":\"deploy_control\",\"control_type\":\"Button\",\"id\":\"SAVE-BUTTON\",\"properties\":{\"Caption\":\"Save\",\"X\":300,\"Y\":240}}`\n- `{\"op\":\"set_property\",\"control_id\":\"TOTAL-LABEL\",\"key\":\"ForegroundColor\",\"value\":\"#008000\"}`\n- `{\"op\":\"generate_event_handler\",\"control_id\":\"SAVE-BUTTON\",\"event\":\"onClick\",\"code\":\"…\"}`\n- `{\"op\":\"create_procedure\",\"name\":\"VALIDATE-INPUT\",\"code\":\"…\"}`\n\nUse `\"control_id\":\"Form\"` to set a form-level property such as GlassStyle.\n\nThere are no other operation names. `UPDATE_FORM_PROPERTY`, `UPDATE_CONTROL_PROPERTIES`, and `UPDATE_FORM_STYLE` do not exist and are silently discarded. A description of a change — a table, a bullet list, prose — is NOT a change. Keep the change-set minimal: emit only the operations the developer asked for.";

/// The hard boundary between the developer's project (which agents build) and
/// the PowerRustCOBOL IDE itself (which they never touch). Appended to EVERY
/// delegated agent's system prompt, regardless of which tools it declares,
/// because the read-only `egui.*` inspection tools observe the live IDE window
/// and could otherwise be mistaken for a licence to restyle or reconfigure it.
pub const PROJECT_SCOPE_BOUNDARY: &str = "\n\n--- Project scope boundary (absolute) ---\nEverything you create or modify belongs to the DEVELOPER'S OPEN PROJECT. The PowerRustCOBOL IDE / RAD Form Designer is the tool you are running inside; it is NOT part of the application being built and is permanently off limits.\n\nYou MAY, within the open project: create and modify forms and their controls; set form and control properties; wire events for controls deployed on a project form; create and modify indexed-file definitions; write project documentation and Knowledge Base files; generate and edit project COBOL sources; run version-control operations on the project repository.\n\nYou MUST NOT, under any circumstances: change the IDE's own appearance, theme, layout, panels, fonts, or window chrome; change IDE settings, preferences, or configuration; add, remove, rename, retune, or reconfigure agents, their prompts, their tools, or their model profiles; modify the PowerRustCOBOL source repository, its build files, or its own documentation.\n\nWhen a form-level property such as GlassStyle or Theme is requested, it applies to the PROJECT FORM named in your task context — never to the IDE. The `egui.*` tools render the live IDE window for observation only; IDE widgets that appear in their output (toolbox buttons, project rails, chat controls, settings panels) are never valid targets and must never appear in a change-set.\n\nIf a request would require any prohibited change, do not attempt it and do not approximate it. Report to Grace that the request falls outside the project scope, and state plainly what was asked for.";

/// A machine-readable appendix describing HOW to call the tools an agent has
/// been granted (spec 030 R2 tooling contract). Appended to the agent's system
/// prompt at delegation time so the contract always matches the agent's actual
/// declared tools — an agent with no tools gets nothing.
pub fn tool_contract_appendix(declared: &HashSet<String>) -> String {
    let mut out = String::new();
    if declared.iter().any(|t| t.starts_with("git.")) {
        out.push_str(GIT_TOOL_CONTRACT);
    }
    if declared.iter().any(|t| t.starts_with("egui.")) {
        out.push_str(EGUI_TOOL_CONTRACT);
    }
    if declared.iter().any(|t| t.starts_with("documentation.")) {
        out.push_str(DOCUMENTATION_TOOL_CONTRACT);
    }
    if declared.iter().any(|t| t.starts_with("knowledge.")) {
        out.push_str(KNOWLEDGE_TOOL_CONTRACT);
    }
    if declared.iter().any(|t| t.starts_with("indexed_file.")) {
        out.push_str(INDEXED_FILE_TOOL_CONTRACT);
    }
    // NOTE (spec 034 rollback): the `project.select_target` contract is
    // deliberately NOT appended. Forcing agents to resolve a target by name
    // before every create/edit broke the Form Designer flow — that agent edits
    // the OPEN form via change-sets applied by the IDE, never a resolved file
    // path, so a name like "MAIN-FORM" fails to resolve and blocks the edit. The
    // tool + picker mechanism stay in the codebase but are no longer mandated.
    out
}

fn changed_indexed_file_roots() -> &'static Mutex<HashSet<PathBuf>> {
    static ROOTS: OnceLock<Mutex<HashSet<PathBuf>>> = OnceLock::new();
    ROOTS.get_or_init(|| Mutex::new(HashSet::new()))
}

fn mark_indexed_files_changed(project_root: &std::path::Path) {
    changed_indexed_file_roots()
        .lock()
        .unwrap()
        .insert(project_root.to_path_buf());
}

pub(crate) fn take_indexed_files_changed(project_root: &std::path::Path) -> bool {
    changed_indexed_file_roots()
        .lock()
        .unwrap()
        .remove(project_root)
}

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn args_digest(args: &serde_json::Value) -> String {
    if args.is_null() {
        return String::new();
    }
    let mut s = args.to_string();
    if s.len() > 200 {
        s.truncate(197);
        s.push_str("…");
    }
    s
}

/// A tool-capable [`AgentInvoker`] decorator. Wraps an inner invoker and a
/// [`ToolBackend`]; resolves each agent's *declared* tool set for governance.
pub struct ToolExecutingInvoker<'a> {
    inner: &'a mut dyn AgentInvoker,
    backend: &'a mut dyn ToolBackend,
    /// Resolves the declared tool names for an agent (spec 030 R2). Injected so
    /// the loop is testable without the agent database.
    declared: Box<dyn Fn(&str) -> HashSet<String> + 'a>,
    /// Shared, so the host can persist evidence after the run (R11).
    evidence: Arc<Mutex<Vec<ToolEvidence>>>,
    /// Bound on tool-execution rounds per task (guards against a model that
    /// never stops emitting tool calls).
    max_tool_rounds: usize,
}

impl<'a> ToolExecutingInvoker<'a> {
    pub fn new(
        inner: &'a mut dyn AgentInvoker,
        backend: &'a mut dyn ToolBackend,
        declared: impl Fn(&str) -> HashSet<String> + 'a,
        evidence: Arc<Mutex<Vec<ToolEvidence>>>,
        max_tool_rounds: usize,
    ) -> Self {
        Self {
            inner,
            backend,
            declared: Box::new(declared),
            evidence,
            max_tool_rounds,
        }
    }

    fn record(&self, agent: &str, call: &ToolCall, res: &ToolResult) {
        self.evidence.lock().unwrap().push(ToolEvidence {
            agent: agent.to_string(),
            tool: call.tool.clone(),
            args_digest: args_digest(&call.args),
            summary: res.summary.clone(),
            ok: res.ok,
            ts: now_secs(),
        });
    }
}

impl AgentInvoker for ToolExecutingInvoker<'_> {
    // Typed extraction (Rig migration phase 3) is a transport concern:
    // delegate straight to the wrapped invoker so its provider-native
    // recovery stays reachable through this decorator.
    fn extract_plan(&mut self, agent: &str, plan_reply: &str) -> Result<WorkflowPlan, String> {
        self.inner.extract_plan(agent, plan_reply)
    }

    fn extract_verdict(
        &mut self,
        reviewer: &str,
        review_reply: &str,
    ) -> Result<ReviewVerdict, String> {
        self.inner.extract_verdict(reviewer, review_reply)
    }

    fn invoke(&mut self, agent: &str, system: &str, user: &str) -> Result<String, String> {
        let declared = (self.declared)(agent);
        let mut convo_user = user.to_string();
        let mut rounds = 0usize;
        // Fallback contract: one fix-and-repeat round per task for a REJECTED
        // (critical) tool call — the error is relayed verbatim for the agent
        // itself to correct; the host never patches the call. A second
        // rejection fails the task and the error reaches the chat history.
        let mut critical_retry_used = false;
        loop {
            let reply = self.inner.invoke(agent, system, &convo_user)?;
            let calls = match parse_tool_calls(&reply) {
                Ok(None) => return Ok(reply), // final result — no tools requested
                Ok(Some(calls)) => calls,
                Err(cause) => {
                    // A malformed block is usually one broken bracket (observed
                    // live: a documentation.write missing its closing `]`
                    // failed the task and blocked the whole dependency chain).
                    // Give the agent a bounded correction round instead of
                    // failing the task on the spot.
                    if rounds >= self.max_tool_rounds {
                        return Err(format!(
                            "tool-call block still malformed after {rounds} correction round(s): {cause}"
                        ));
                    }
                    rounds += 1;
                    convo_user = format!(
                        "{convo_user}\n\n=== TOOL CALL ERROR (round {rounds}) ===\nYour previous reply ended with a tool_calls block that could not be parsed: {cause}\n\nYOUR PREVIOUS REPLY:\n{reply}\n\nRe-send the reply with the corrected fenced JSON tool_calls block — verify every bracket closes: {{\"tool_calls\":[{{\"tool\":\"...\",\"args\":{{...}}}}]}} — or reply with your final result and no tool_calls block."
                    );
                    continue;
                }
            };
            if rounds >= self.max_tool_rounds {
                return Err(format!(
                    "tool-call loop exceeded {} round(s) without a final result",
                    self.max_tool_rounds
                ));
            }
            rounds += 1;

            let mut rendered = String::new();
            let mut rejected = false;
            for call in &calls {
                // Native function names cannot contain `.`, so a model calling
                // a fenced tool natively says `indexed_file_write`; map the
                // last underscore back to the declared dotted name.
                let resolved: Option<ToolCall> = if declared.contains(&call.tool) {
                    Some(call.clone())
                } else {
                    call.tool.rsplit_once('_').and_then(|(ns, op)| {
                        let dotted = format!("{ns}.{op}");
                        declared.contains(&dotted).then(|| ToolCall {
                            tool: dotted,
                            args: call.args.clone(),
                        })
                    })
                };
                let res = if let Some(resolved) = &resolved {
                    self.backend.execute(agent, resolved)
                } else {
                    ToolResult::critical(format!(
                        "Agent \u{201c}{agent}\u{201d} invoked undeclared tool \u{201c}{}\u{201d} — ungoverned/fabricated tool use is a critical defect (spec 029 R4).",
                        call.tool
                    ))
                };
                self.record(agent, call, &res);
                if res.needs_confirmation {
                    // Blocked pending the developer's go-ahead (e.g. a
                    // destructive recreate of a finalized file). Not a defect
                    // to fix: stop now and return the message as Grace's reply
                    // so the developer confirms or cancels. Calls after this
                    // one were NOT executed.
                    return Ok(res.summary);
                }
                if res.critical {
                    if critical_retry_used {
                        // Second rejection: fail the task. The engine stores
                        // this as the failure reason, so the error message and
                        // the check-the-prompt request reach the chat history.
                        return Err(format!(
                            "CRITICAL DEFECT: {} — the tool call was rejected again after a fix-and-repeat round; check \u{201c}{agent}\u{201d}'s prompt and this task's instructions",
                            res.summary
                        ));
                    }
                    rendered.push_str(&format!(
                        "- {} [REJECTED]: {}\n{}\n",
                        call.tool, res.summary, res.detail
                    ));
                    rejected = true;
                    break; // calls after the rejected one are NOT executed
                }
                rendered.push_str(&format!(
                    "- {} [{}]: {}\n{}\n",
                    call.tool,
                    if res.ok { "ok" } else { "error" },
                    res.summary,
                    res.detail
                ));
            }
            convo_user = if rejected {
                critical_retry_used = true;
                format!(
                    "{convo_user}\n\n=== TOOL CALL REJECTED (round {rounds}) ===\n{rendered}\nAnalyse the rejection, fix your tool call, and repeat it. Any calls after the rejected one were NOT executed. This is your only correction round: a second rejection fails the task."
                )
            } else {
                format!(
                    "{convo_user}\n\n=== TOOL RESULTS (round {rounds}) ===\n{rendered}\nUse these real results. When the task is complete, reply with your final result and DO NOT emit a tool_calls block."
                )
            };
        }
    }
}

/// The IDE's real tool backend: routes `git.*` through the project-scoped git
/// executor (spec 030 R9–R14) and `egui.*` through the observe-only inspection
/// reader (R4/R5). A tool in neither namespace is a critical defect.
pub struct IdeToolBackend<'a> {
    /// The open user project's repository root — git ops are bound here (R9).
    pub project_dir: PathBuf,
    /// Confirmation for gated git ops (R12). Returns true to proceed.
    pub confirm: &'a mut dyn FnMut(GitConfirmRequest) -> bool,
    /// Target picker for ambiguous create/edit (spec 034). Returns the chosen
    /// project-relative target, or `None` to cancel.
    pub select_target: &'a mut dyn FnMut(TargetRequest) -> Option<TargetChoice>,
}

impl<'a> IdeToolBackend<'a> {
    pub fn new(
        project_dir: PathBuf,
        confirm: &'a mut dyn FnMut(GitConfirmRequest) -> bool,
        select_target: &'a mut dyn FnMut(TargetRequest) -> Option<TargetChoice>,
    ) -> Self {
        Self {
            project_dir,
            confirm,
            select_target,
        }
    }

    fn exec_git(&mut self, call: &ToolCall) -> ToolResult {
        let argv = match git_argv(call) {
            Ok(a) => a,
            // A malformed tool call is a contract violation (spec 030 R2).
            Err(e) => return ToolResult::critical(format!("malformed git tool call: {e}")),
        };
        let class = match git_exec::classify(&argv) {
            Ok(c) => c,
            // Unrecognised op: rejected but recoverable — the agent may choose a
            // supported operation (spec 030 R14).
            Err(e) => return ToolResult::err(e, "The git operation was rejected and did not run."),
        };
        if class == GitClass::Gated {
            let command = git_exec::command_string(&argv);
            let approved = (self.confirm)(GitConfirmRequest {
                command: command.clone(),
            });
            if !approved {
                return ToolResult::err(
                    format!("operator declined: {command}"),
                    "The gated git operation was not approved and did not run.",
                );
            }
        }
        match git_exec::run_git(&self.project_dir, &argv) {
            Ok(out) => {
                let detail = format!(
                    "$ {}\n[exit {}]\n{}{}",
                    git_exec::command_string(&argv),
                    out.status
                        .map(|c| c.to_string())
                        .unwrap_or_else(|| "signal".into()),
                    out.stdout,
                    out.stderr
                );
                if out.ok() {
                    ToolResult::ok(out.summary(), detail)
                } else {
                    ToolResult::err(out.summary(), detail)
                }
            }
            Err(e) => ToolResult::err(format!("git could not run: {e}"), e),
        }
    }

    fn exec_egui(&mut self, call: &ToolCall) -> ToolResult {
        crate::agent_inspection::observe(&call.tool)
    }

    /// `project.select_target` (spec 034): resolve where a create/edit acts by
    /// asking the developer to pick a target on the project tree, and return the
    /// chosen **project-relative** path for the agent to use in its write.
    fn exec_project(&mut self, call: &ToolCall) -> ToolResult {
        use crate::target_select::{create_request, edit_candidates, edit_request, TargetChoice};
        if call.tool != "project.select_target" {
            return ToolResult::critical(format!("unknown project tool “{}”", call.tool));
        }
        let op = match Self::string_arg(call, "op") {
            Ok(v) => v.to_ascii_lowercase(),
            Err(e) => return e,
        };
        let kind_s = match Self::string_arg(call, "kind") {
            Ok(v) => v.to_ascii_lowercase(),
            Err(e) => return e,
        };
        let name = match Self::string_arg(call, "name") {
            Ok(v) => v.to_string(),
            Err(e) => return e,
        };
        let Some(kind) = parse_file_kind(&kind_s) else {
            return ToolResult::err(
                "unknown element kind",
                "kind must be one of: form, indexed, source, documentation, asset.",
            );
        };

        let request = match op.as_str() {
            "create" => create_request(kind, &name),
            "edit" => {
                let project = crate::project_model::load_project(
                    &self.project_dir.join("cobolt.toml"),
                )
                .unwrap_or_else(|_| crate::project_model::CoboltProject::new("", ""));
                let candidates = edit_candidates(&project, kind, &name);
                match candidates.len() {
                    0 => {
                        return ToolResult::err(
                            format!("no “{name}” element to edit"),
                            "No project element of that name and kind exists.",
                        )
                    }
                    // Exactly one match resolves without a modal (spec 034, R4).
                    1 => {
                        return ToolResult::ok(
                            format!("target: {}", candidates[0]),
                            candidates[0].clone(),
                        )
                    }
                    _ => edit_request(&project, kind, &name).expect("2+ candidates ⇒ request"),
                }
            }
            _ => {
                return ToolResult::err(
                    "unknown op",
                    "op must be \"create\" or \"edit\".",
                )
            }
        };

        match (self.select_target)(request) {
            Some(TargetChoice { rel_path }) => {
                ToolResult::ok(format!("target: {rel_path}"), rel_path)
            }
            // Cancel: stop the workflow and tell the developer nothing was done
            // (spec 034, R5) — like the confirm-required flow.
            None => ToolResult::needs_confirmation(format!(
                "Target selection for “{name}” was cancelled; nothing was created or edited."
            )),
        }
    }

    fn string_arg<'b>(call: &'b ToolCall, name: &str) -> Result<&'b str, ToolResult> {
        call.args
            .get(name)
            .and_then(serde_json::Value::as_str)
            .filter(|value| !value.trim().is_empty())
            .ok_or_else(|| {
                ToolResult::critical(format!(
                    "malformed {} call: \"{name}\" must be a non-empty string",
                    call.tool
                ))
            })
    }

    fn exec_documentation(&mut self, agent: &str, call: &ToolCall) -> ToolResult {
        if !agent.eq_ignore_ascii_case(crate::agents_db::DOCUMENTATION_AGENT) {
            return ToolResult::critical(format!(
                "Agent “{agent}” attempted project-document mutation or access reserved for {}.",
                crate::agents_db::DOCUMENTATION_AGENT
            ));
        }
        match call.tool.as_str() {
            "documentation.write" => {
                let path = match Self::string_arg(call, "path") {
                    Ok(path) => path,
                    Err(error) => return error,
                };
                let content = match Self::string_arg(call, "content") {
                    Ok(content) => content,
                    Err(error) => return error,
                };
                match cobolt_agents::project_knowledge::write_document(
                    &self.project_dir,
                    path,
                    content,
                ) {
                    Ok(relative) => ToolResult::ok(
                        format!("wrote and indexed {}", relative.display()),
                        format!(
                            "Project document written and indexed in {}.",
                            cobolt_agents::project_knowledge::database_path(&self.project_dir)
                                .display()
                        ),
                    ),
                    Err(error) => ToolResult::err("documentation write failed", error),
                }
            }
            "documentation.read" => {
                let path = match Self::string_arg(call, "path") {
                    Ok(path) => path,
                    Err(error) => return error,
                };
                let relative = match cobolt_agents::project_knowledge::normalize_document_path(path)
                {
                    Ok(relative) => relative,
                    Err(error) => return ToolResult::err("documentation path rejected", error),
                };
                match std::fs::read_to_string(self.project_dir.join(&relative)) {
                    Ok(content) => ToolResult::ok(format!("read {}", relative.display()), content),
                    Err(error) => ToolResult::err(
                        format!("could not read {}", relative.display()),
                        error.to_string(),
                    ),
                }
            }
            "documentation.list" => {
                match cobolt_agents::project_knowledge::documentation_paths(&self.project_dir) {
                    Ok(paths) => ToolResult::ok(
                        format!("listed {} project document(s)", paths.len()),
                        paths.join("\n"),
                    ),
                    Err(error) => ToolResult::err("could not list project documents", error),
                }
            }
            _ => ToolResult::critical(format!("unknown documentation tool “{}”", call.tool)),
        }
    }

    fn exec_knowledge(&mut self, call: &ToolCall) -> ToolResult {
        let query = match Self::string_arg(call, "query") {
            Ok(query) => query,
            Err(error) => return error,
        };
        let limit = call
            .args
            .get("limit")
            .and_then(serde_json::Value::as_u64)
            .unwrap_or(5)
            .clamp(1, 10) as usize;
        if let Err(error) = cobolt_agents::project_knowledge::sync_documentation(&self.project_dir)
        {
            return ToolResult::err("project knowledge synchronization failed", error);
        }
        match cobolt_agents::project_knowledge::search(&self.project_dir, query, limit) {
            Ok(hits) => {
                let detail = hits
                    .iter()
                    .map(|hit| {
                        format!(
                            "PATH: {}\nSCORE: {:.4}\nEXCERPT:\n{}",
                            hit.path, hit.score, hit.excerpt
                        )
                    })
                    .collect::<Vec<_>>()
                    .join("\n\n---\n\n");
                ToolResult::ok(
                    format!("retrieved {} project document(s)", hits.len()),
                    detail,
                )
            }
            Err(error) => ToolResult::err("project knowledge search failed", error),
        }
    }

    fn indexed_path(&self, raw: &str) -> Result<(PathBuf, PathBuf), ToolResult> {
        let mut relative = PathBuf::new();
        for component in Path::new(raw.trim_start_matches(['/', '\\'])).components() {
            match component {
                Component::Normal(part) => relative.push(part),
                Component::CurDir => {}
                Component::ParentDir | Component::RootDir | Component::Prefix(_) => {
                    return Err(ToolResult::critical(format!(
                        "indexed-file path escapes the project: {raw}"
                    )))
                }
            }
        }
        if !relative
            .components()
            .next()
            .and_then(|part| match part {
                Component::Normal(part) => part.to_str(),
                _ => None,
            })
            .is_some_and(|part| part.eq_ignore_ascii_case("indexed"))
        {
            relative = Path::new("indexed").join(relative);
        }
        if relative.extension().is_none() {
            relative.set_extension("cidx");
        }
        if !relative
            .extension()
            .and_then(|extension| extension.to_str())
            .is_some_and(|extension| extension.eq_ignore_ascii_case("cidx"))
        {
            return Err(ToolResult::critical(
                "indexed-file definitions must use the .cidx extension",
            ));
        }
        Ok((relative.clone(), self.project_dir.join(relative)))
    }

    fn indexed_definition_paths(&self) -> Result<Vec<PathBuf>, String> {
        let indexed_dir = self.project_dir.join("indexed");
        if !indexed_dir.exists() {
            return Ok(Vec::new());
        }
        let mut paths = Vec::new();
        let entries = std::fs::read_dir(&indexed_dir).map_err(|error| error.to_string())?;
        for entry in entries {
            let entry = entry.map_err(|error| error.to_string())?;
            let path = entry.path();
            if path.is_file()
                && path
                    .extension()
                    .and_then(|extension| extension.to_str())
                    .is_some_and(|extension| extension.eq_ignore_ascii_case("cidx"))
            {
                paths.push(
                    path.strip_prefix(&self.project_dir)
                        .unwrap_or(&path)
                        .to_path_buf(),
                );
            }
        }
        paths.sort();
        Ok(paths)
    }

    fn indexed_key_part(
        field_name: &str,
        leaves: &[&cobolt_indexed::IndexedField],
    ) -> Result<cobolt_indexed::KeyPartDef, ToolResult> {
        let Some(field) = leaves
            .iter()
            .find(|field| field.name.eq_ignore_ascii_case(field_name))
        else {
            return Err(ToolResult::critical(format!(
                "indexed key field \"{field_name}\" does not exist in the record"
            )));
        };
        let (Some(offset), Some(length)) = (field.offset, field.length) else {
            return Err(ToolResult::critical(format!(
                "indexed key \"{field_name}\" must name an elementary field"
            )));
        };
        Ok(cobolt_indexed::KeyPartDef {
            field_name: field.name.clone(),
            offset,
            length,
            encoding: cobolt_indexed::KeyEncodingDef::Bytes,
        })
    }

    fn validate_normalization(call: &ToolCall) -> Result<String, ToolResult> {
        let Some(normalization) = call
            .args
            .get("normalization")
            .and_then(|value| value.as_object())
        else {
            return Err(ToolResult::critical(format!(
                "malformed {} call: normalization must contain approved 1nf, 2nf, and 3nf decisions",
                call.tool
            )));
        };
        let mut evidence = Vec::new();
        for form in ["1nf", "2nf", "3nf"] {
            let Some(decision) = normalization
                .get(form)
                .and_then(|value| value.as_str())
                .filter(|value| !value.trim().is_empty())
            else {
                return Err(ToolResult::critical(format!(
                    "malformed {} call: normalization.{form} must be a non-empty Documentation Agent decision",
                    call.tool
                )));
            };
            evidence.push(format!(
                "{}: {}",
                form.to_ascii_uppercase(),
                decision.trim()
            ));
        }
        Ok(evidence.join("\n"))
    }

    fn is_id_field(name: &str) -> bool {
        let upper = name.to_ascii_uppercase();
        if upper == "ID" || upper.ends_with("-ID") || upper.ends_with("_ID") {
            return true;
        }
        // camelCase identifiers (CompanyID, UserId): an "ID"/"Id" suffix right
        // after a lowercase letter. Whole words ending in "id" (VALID, MADRID,
        // Paid, Grid) stay excluded because their preceding letter is not a
        // lowercase-to-uppercase case break.
        let bytes = name.as_bytes();
        bytes.len() >= 3
            && (name.ends_with("ID") || name.ends_with("Id"))
            && bytes[bytes.len() - 3].is_ascii_lowercase()
    }

    fn validate_id_definitions(
        call: &ToolCall,
        leaves: &[&cobolt_indexed::IndexedField],
    ) -> Result<String, ToolResult> {
        // The record parser uppercases COBOL data-names, which collapses
        // camelCase spellings (CompanyID → COMPANYID) and would let camelCase
        // ID fields bypass this gate. Detect ID fields from their RAW spelling
        // in the submitted record text, and also govern every field the caller
        // explicitly listed in id_definitions.
        let record_text = call
            .args
            .get("record")
            .and_then(|value| value.as_str())
            .unwrap_or("");
        let raw_spelling = |field_name: &str| {
            record_text
                .split(|c: char| !(c.is_ascii_alphanumeric() || matches!(c, '-' | '_')))
                .find(|token| token.eq_ignore_ascii_case(field_name))
        };
        let declared = call
            .args
            .get("id_definitions")
            .and_then(|value| value.as_object());
        let id_fields: Vec<(&cobolt_indexed::IndexedField, String)> = leaves
            .iter()
            .filter_map(|field| {
                let spelling = raw_spelling(&field.name)
                    .unwrap_or(field.name.as_str())
                    .to_owned();
                let governed = Self::is_id_field(&spelling)
                    || declared.is_some_and(|definitions| {
                        definitions
                            .keys()
                            .any(|name| name.eq_ignore_ascii_case(&field.name))
                    });
                governed.then_some((*field, spelling))
            })
            .collect();
        if id_fields.is_empty() {
            return Ok("No ID fields are present in this record.".into());
        }
        let Some(definitions) = declared else {
            return Err(ToolResult::critical(
                "every ID field requires the developer's UUID or exact PIC choice in id_definitions",
            ));
        };
        let mut evidence = Vec::new();
        for (field, spelling) in id_fields {
            let choice = definitions
                .iter()
                .find(|(name, _)| name.eq_ignore_ascii_case(&field.name))
                .and_then(|(_, value)| value.as_str())
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .ok_or_else(|| {
                    ToolResult::critical(format!(
                        "ID field {spelling} has no developer-approved UUID or PIC definition"
                    ))
                })?;
            if choice.eq_ignore_ascii_case("UUID") {
                if !field.pic.trim().eq_ignore_ascii_case("X(36)") {
                    return Err(ToolResult::critical(format!(
                        "ID field {} was approved as UUID and must use PIC X(36), but the record uses PIC {}",
                        spelling, field.pic
                    )));
                }
            } else if let Some(pic) = choice
                .strip_prefix("PIC ")
                .or_else(|| choice.strip_prefix("pic "))
            {
                if !field.pic.trim().eq_ignore_ascii_case(pic.trim()) {
                    return Err(ToolResult::critical(format!(
                        "ID field {} uses PIC {}, which does not match the developer-approved {}",
                        spelling, field.pic, choice
                    )));
                }
            } else {
                return Err(ToolResult::critical(format!(
                    "ID field {spelling} must be defined as UUID or an exact PIC clause, not {choice}"
                )));
            }
            evidence.push(format!("{spelling}: {choice}"));
        }
        Ok(evidence.join(", "))
    }

    fn validate_indexed_name(name: &str) -> Result<(), ToolResult> {
        let valid = name
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || matches!(character, '-' | '_'))
            && name
                .chars()
                .next()
                .is_some_and(|character| character.is_ascii_alphabetic());
        if valid {
            Ok(())
        } else {
            Err(ToolResult::critical(
                "indexed-file name must begin with a letter and contain only letters, digits, hyphens, or underscores",
            ))
        }
    }

    fn validate_assign_path(&self, assign_path: &str) -> Result<PathBuf, ToolResult> {
        let path = Path::new(assign_path);
        if path.is_absolute()
            || path.components().any(|component| {
                matches!(
                    component,
                    Component::ParentDir | Component::RootDir | Component::Prefix(_)
                )
            })
        {
            return Err(ToolResult::critical(
                "automated indexed-file writes require a project-relative assign_path without parent traversal",
            ));
        }
        Ok(self.project_dir.join(path))
    }

    fn write_indexed_artifacts(
        &self,
        relative: &Path,
        absolute: &Path,
        def: &cobolt_indexed::IndexedDefinition,
        record: &str,
    ) -> Result<Vec<PathBuf>, String> {
        let stem = relative
            .file_stem()
            .and_then(|value| value.to_str())
            .ok_or_else(|| "indexed-file path has no valid file stem".to_string())?;
        let generated = self
            .project_dir
            .join("generated")
            .join(format!("{stem}-indexed.cbl"));
        let copybooks = self.project_dir.join("COPYBOOKS");
        let select = copybooks.join(format!("{stem}.SEL"));
        let fd = copybooks.join(format!("{stem}.FD"));
        let record_copybook = copybooks.join(format!("{}.fd.cpy", def.name));
        let xml = cobolt_indexed::save_indexed_to_string(def).map_err(|error| error.to_string())?;
        let artifacts = [
            (absolute.to_path_buf(), xml),
            (generated.clone(), cobolt_codegen::generate_indexed(def)),
            (select.clone(), cobolt_codegen::generate_indexed_select(def)),
            (fd.clone(), cobolt_codegen::generate_indexed_fd(def)),
            (record_copybook.clone(), record.to_string()),
        ];
        for (path, _) in &artifacts {
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent).map_err(|error| error.to_string())?;
            }
        }
        for (path, content) in &artifacts {
            std::fs::write(path, content).map_err(|error| {
                format!(
                    "could not write indexed-file artifact {}: {error}",
                    path.display()
                )
            })?;
        }
        Ok(vec![
            relative.to_path_buf(),
            generated
                .strip_prefix(&self.project_dir)
                .unwrap_or(&generated)
                .to_path_buf(),
            select
                .strip_prefix(&self.project_dir)
                .unwrap_or(&select)
                .to_path_buf(),
            fd.strip_prefix(&self.project_dir)
                .unwrap_or(&fd)
                .to_path_buf(),
            record_copybook
                .strip_prefix(&self.project_dir)
                .unwrap_or(&record_copybook)
                .to_path_buf(),
        ])
    }

    fn exec_indexed_file(&mut self, agent: &str, call: &ToolCall) -> ToolResult {
        if !agent.eq_ignore_ascii_case(crate::agents_db::DATA_INDEXED_FILE_AGENT) {
            return ToolResult::critical(format!(
                "Agent \"{agent}\" attempted indexed-file access reserved for {}.",
                crate::agents_db::DATA_INDEXED_FILE_AGENT
            ));
        }
        match call.tool.as_str() {
            "indexed_file.list" => match self.indexed_definition_paths() {
                Ok(paths) => ToolResult::ok(
                    format!("listed {} indexed-file definition(s)", paths.len()),
                    paths
                        .iter()
                        .map(|path| path.to_string_lossy())
                        .collect::<Vec<_>>()
                        .join("\n"),
                ),
                Err(error) => ToolResult::err("could not list indexed files", error),
            },
            "indexed_file.read" => {
                let path = match Self::string_arg(call, "path") {
                    Ok(path) => path,
                    Err(error) => return error,
                };
                let (relative, absolute) = match self.indexed_path(path) {
                    Ok(paths) => paths,
                    Err(error) => return error,
                };
                match cobolt_indexed::load_indexed(&absolute) {
                    Ok(def) => match cobolt_indexed::save_indexed_to_string(&def) {
                        Ok(xml) => ToolResult::ok(
                            format!("read {}", relative.display()),
                            format!(
                                "DEFINITION XML:\n{xml}\n\nRECORD DESCRIPTION:\n{}",
                                cobolt_indexed::record_to_text(&def)
                            ),
                        ),
                        Err(error) => ToolResult::err(
                            format!("could not serialize {}", relative.display()),
                            error.to_string(),
                        ),
                    },
                    Err(error) => ToolResult::err(
                        format!("could not read {}", relative.display()),
                        error.to_string(),
                    ),
                }
            }
            "indexed_file.write" => {
                let path = match Self::string_arg(call, "path") {
                    Ok(value) => value,
                    Err(error) => return error,
                };
                let name = match Self::string_arg(call, "name") {
                    Ok(value) => value,
                    Err(error) => return error,
                };
                if let Err(error) = Self::validate_indexed_name(name) {
                    return error;
                }
                let purpose = match Self::string_arg(call, "purpose") {
                    Ok(value) => value,
                    Err(error) => return error,
                };
                let assign_path = match Self::string_arg(call, "assign_path") {
                    Ok(value) => value,
                    Err(error) => return error,
                };
                let data_path = match self.validate_assign_path(assign_path) {
                    Ok(path) => path,
                    Err(error) => return error,
                };
                let record = match Self::string_arg(call, "record") {
                    Ok(value) => value,
                    Err(error) => return error,
                };
                let primary_key = match Self::string_arg(call, "primary_key") {
                    Ok(value) => value,
                    Err(error) => return error,
                };
                let normalization = match Self::validate_normalization(call) {
                    Ok(evidence) => evidence,
                    Err(error) => return error,
                };
                let (relative, absolute) = match self.indexed_path(path) {
                    Ok(paths) => paths,
                    Err(error) => return error,
                };

                let existing = if absolute.exists() {
                    match cobolt_indexed::load_indexed(&absolute) {
                        Ok(definition) => Some(definition),
                        Err(error) => {
                            return ToolResult::err(
                                format!("could not load existing {}", relative.display()),
                                error.to_string(),
                            )
                        }
                    }
                } else {
                    None
                };
                let locked_definition = existing
                    .as_ref()
                    .filter(|definition| definition.finalized)
                    .cloned();
                let mut def = existing
                    .clone()
                    .unwrap_or_else(|| cobolt_indexed::IndexedDefinition::new(name, assign_path));
                def.name = name.to_string();
                def.assign_path = assign_path.to_string();
                def.comment = purpose.to_string();
                if let Some(access_mode) = call
                    .args
                    .get("access_mode")
                    .and_then(|value| value.as_str())
                {
                    def.access_mode = cobolt_indexed::AccessMode::from_str(access_mode);
                }
                if let Some(storage) = call.args.get("storage").and_then(|value| value.as_str()) {
                    def.storage = cobolt_indexed::StorageMode::from_str(storage);
                }
                def.finalized = call
                    .args
                    .get("finalized")
                    .and_then(|value| value.as_bool())
                    .unwrap_or_else(|| {
                        existing
                            .as_ref()
                            .map(|definition| definition.finalized)
                            .unwrap_or(true)
                    });
                if let Err(error) = cobolt_indexed::text_to_record(&mut def, record) {
                    return ToolResult::err("indexed record definition is invalid", error);
                }
                let leaves = def
                    .record_root()
                    .map(cobolt_indexed::IndexedField::all_leaves)
                    .unwrap_or_default();
                let id_evidence = match Self::validate_id_definitions(call, &leaves) {
                    Ok(evidence) => evidence,
                    Err(error) => return error,
                };
                let primary_part = match Self::indexed_key_part(primary_key, &leaves) {
                    Ok(part) => part,
                    Err(error) => return error,
                };
                let primary = cobolt_indexed::KeyDef {
                    // The `.cidx` format identifies the primary key through
                    // its part and intentionally does not serialize a name.
                    name: None,
                    parts: vec![primary_part],
                    duplicates_allowed: false,
                    ordering: cobolt_indexed::KeyOrderingDef::Ascending,
                };
                let mut alternates = Vec::new();
                let alternate_keys = call
                    .args
                    .get("alternate_keys")
                    .and_then(|value| value.as_array())
                    .cloned()
                    .unwrap_or_default();
                for alternate in alternate_keys {
                    let Some(field_name) = alternate
                        .get("field")
                        .and_then(|value| value.as_str())
                        .filter(|value| !value.trim().is_empty())
                    else {
                        return ToolResult::critical(
                            "each alternate_keys entry requires a non-empty field",
                        );
                    };
                    let part = match Self::indexed_key_part(field_name, &leaves) {
                        Ok(part) => part,
                        Err(error) => return error,
                    };
                    alternates.push(cobolt_indexed::KeyDef {
                        name: Some(part.field_name.clone()),
                        parts: vec![part],
                        duplicates_allowed: alternate
                            .get("duplicates")
                            .and_then(|value| value.as_bool())
                            .unwrap_or(false),
                        ordering: cobolt_indexed::KeyOrderingDef::Ascending,
                    });
                }
                drop(leaves);
                def.keys.primary = primary;
                def.keys.alternates = alternates;
                if let Some(locked) = locked_definition {
                    let structure_changed = locked.name != def.name
                        || locked.assign_path != def.assign_path
                        || locked.access_mode != def.access_mode
                        || locked.record_format != def.record_format
                        || locked.storage != def.storage
                        || locked.compression != def.compression
                        || locked.persistence != def.persistence
                        || locked.keys != def.keys
                        || locked.fields != def.fields;
                    if structure_changed || !def.finalized {
                        // The developer may authorize the destructive rewrite by
                        // confirming; the Data agent then repeats the write with
                        // `confirm_recreate: true`. Absent that, stop and ask —
                        // this is not a defect the agent can fix on its own.
                        let confirmed = call
                            .args
                            .get("confirm_recreate")
                            .and_then(|value| value.as_bool())
                            .unwrap_or(false);
                        if !confirmed {
                            return ToolResult::needs_confirmation(format!(
                                "The task cannot be completed as a normal edit: {} is finalized (its schema and storage are locked by the Indexed File UI). Completing this change would DESTROY and RECREATE the file, erasing its stored data. Reply to confirm you want to destroy and recreate {}, or cancel to keep it unchanged.",
                                relative.display(),
                                relative.display()
                            ));
                        }
                        // Confirmed: fall through to overwrite (recreate) the
                        // file with the new schema.
                    }
                }
                if let Err(error) = cobolt_indexed::validate_definition(&def) {
                    return ToolResult::err("indexed-file definition is invalid", error);
                }
                let warnings = cobolt_indexed::finalize_warnings(&def);
                let data_existed = data_path.exists();
                let artifacts =
                    match self.write_indexed_artifacts(&relative, &absolute, &def, record) {
                        Ok(paths) => paths,
                        Err(error) => return ToolResult::err("indexed-file write failed", error),
                    };
                if !data_existed {
                    if let Some(parent) = data_path.parent() {
                        if let Err(error) = std::fs::create_dir_all(parent) {
                            return ToolResult::err(
                                "indexed data directory could not be created",
                                error.to_string(),
                            );
                        }
                    }
                    if let Err(error) =
                        cobolt_runtime::indexed_ide::create_empty_from_definition(&def, &data_path)
                    {
                        return ToolResult::err(
                            "empty indexed data file could not be created",
                            error.to_string(),
                        );
                    }
                }
                mark_indexed_files_changed(&self.project_dir);
                ToolResult::ok(
                    format!("saved {} through the Indexed File UI model", relative.display()),
                    format!(
                        "Purpose: {purpose}\nID definitions: {id_evidence}\nNormalization:\n{normalization}\nArtifacts:\n{}\nData: {}\nWarnings: {}",
                        artifacts
                            .iter()
                            .map(|path| path.to_string_lossy())
                            .collect::<Vec<_>>()
                            .join("\n"),
                        if data_existed {
                            "preserved existing indexed data"
                        } else {
                            "created an empty indexed data file"
                        },
                        if warnings.is_empty() {
                            "none".into()
                        } else {
                            warnings.join("; ")
                        }
                    ),
                )
            }
            _ => ToolResult::critical(format!("unknown indexed-file tool \"{}\"", call.tool)),
        }
    }
}

impl ToolBackend for IdeToolBackend<'_> {
    fn execute(&mut self, agent: &str, call: &ToolCall) -> ToolResult {
        if call.tool.starts_with("git.") {
            self.exec_git(call)
        } else if call.tool.starts_with("egui.") {
            self.exec_egui(call)
        } else if call.tool.starts_with("documentation.") {
            self.exec_documentation(agent, call)
        } else if call.tool.starts_with("knowledge.") {
            self.exec_knowledge(call)
        } else if call.tool.starts_with("indexed_file.") {
            self.exec_indexed_file(agent, call)
        } else if call.tool.starts_with("project.") {
            self.exec_project(call)
        } else {
            ToolResult::critical(format!(
                "unknown tool namespace for \u{201c}{}\u{201d}",
                call.tool
            ))
        }
    }
}

/// Map a `project.select_target` `kind` argument to a [`FileKind`].
fn parse_file_kind(kind: &str) -> Option<crate::project_model::FileKind> {
    use crate::project_model::FileKind;
    match kind {
        "form" => Some(FileKind::Form),
        "indexed" | "indexed_file" | "cidx" => Some(FileKind::Indexed),
        "source" | "common" | "common_code" | "cobol" => Some(FileKind::Source),
        "documentation" | "doc" | "knowledge" => Some(FileKind::Documentation),
        "asset" => Some(FileKind::Asset),
        _ => None,
    }
}

/// Build the git argv for a `git.*` tool call. `git.run` takes a full
/// `{"argv":[…]}`; `git.<sub>` takes `{"args":[…]}` appended after `<sub>`.
fn git_argv(call: &ToolCall) -> Result<Vec<String>, String> {
    let sub = call
        .tool
        .strip_prefix("git.")
        .ok_or_else(|| "not a git tool".to_string())?;
    let str_array = |v: &serde_json::Value| -> Result<Vec<String>, String> {
        v.as_array()
            .ok_or_else(|| "expected a JSON array of strings".to_string())?
            .iter()
            .map(|x| {
                x.as_str()
                    .map(str::to_string)
                    .ok_or_else(|| "git arguments must be strings".to_string())
            })
            .collect()
    };
    if sub == "run" {
        let argv = call
            .args
            .get("argv")
            .ok_or_else(|| "git.run requires an \"argv\" array".to_string())?;
        let argv = str_array(argv)?;
        if argv.is_empty() {
            return Err("git.run argv is empty".into());
        }
        Ok(argv)
    } else {
        let mut argv = vec![sub.to_string()];
        if let Some(extra) = call.args.get("args") {
            argv.extend(str_array(extra)?);
        }
        Ok(argv)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::Command;

    /// Scripted inner invoker: returns queued replies in order.
    struct ScriptInvoker {
        replies: Vec<String>,
        calls: usize,
    }
    impl AgentInvoker for ScriptInvoker {
        fn invoke(&mut self, _agent: &str, _s: &str, _u: &str) -> Result<String, String> {
            let i = self.calls;
            self.calls += 1;
            self.replies
                .get(i)
                .cloned()
                .ok_or_else(|| format!("script exhausted at call #{i}"))
        }
    }

    /// Backend that echoes the call as a successful result.
    struct EchoBackend {
        executed: usize,
    }
    impl ToolBackend for EchoBackend {
        fn execute(&mut self, _agent: &str, call: &ToolCall) -> ToolResult {
            self.executed += 1;
            ToolResult::ok(format!("ran {}", call.tool), "(echo)".to_string())
        }
    }

    fn declared(set: &[&str]) -> HashSet<String> {
        set.iter().map(|s| s.to_string()).collect()
    }

    /// Observed live: gemma wrapped the `normalization` keys in literal quote
    /// characters (`"\"1nf\"": …`) inside a double-nested args payload — the
    /// backend's 1nf/2nf/3nf validation would reject the otherwise-valid
    /// write. Both normalizations must compose.
    #[test]
    fn quoted_object_keys_are_stripped_recursively() {
        let reply = r#"```json
{"tool_calls":[{"tool":"indexed_file.write","args":{"args":{"path":"indexed/idx-company.cidx","normalization":{"\"1nf\"":"Atomic.","\"2nf\"":"No partial deps.","\"3nf\"":"No transitive deps."}}}}]}
```"#;
        let calls = parse_tool_calls(reply).unwrap().expect("calls");
        let normalization = calls[0]
            .args
            .get("normalization")
            .and_then(|v| v.as_object())
            .expect("normalization object");
        assert_eq!(
            normalization.get("1nf").and_then(|v| v.as_str()),
            Some("Atomic."),
            "quoted keys must be stripped: {normalization:?}"
        );
        assert!(normalization.get("2nf").is_some());
        assert!(normalization.get("3nf").is_some());
        assert_eq!(
            calls[0].args.get("path").and_then(|v| v.as_str()),
            Some("indexed/idx-company.cidx"),
            "double-nested unwrap still applies"
        );
    }

    /// Observed live: gemma emitted `"args":{"args":{…real payload…}}` on an
    /// otherwise perfect indexed_file.write. The parser unwraps the extra
    /// level so the backend sees path/record/keys at the top of `args`.
    #[test]
    fn double_nested_args_are_unwrapped() {
        let reply = "```json\n{\"tool_calls\":[{\"tool\":\"indexed_file.write\",\"args\":{\"args\":{\"path\":\"indexed/idx-company-legal.cidx\",\"primary_key\":\"COMPANY-ID\"}}}]}\n```";
        let calls = parse_tool_calls(reply).unwrap().expect("calls");
        assert_eq!(calls.len(), 1);
        assert_eq!(
            calls[0].args.get("path").and_then(|v| v.as_str()),
            Some("indexed/idx-company-legal.cidx"),
            "payload fields must surface at the top level"
        );
        assert_eq!(
            calls[0].args.get("primary_key").and_then(|v| v.as_str()),
            Some("COMPANY-ID")
        );
        // A legitimate single-level call is untouched.
        let plain = "```json\n{\"tool_calls\":[{\"tool\":\"documentation.write\",\"args\":{\"path\":\"/Knowledge Base/x.md\",\"content\":\"# X\"}}]}\n```";
        let calls = parse_tool_calls(plain).unwrap().expect("calls");
        assert_eq!(
            calls[0].args.get("path").and_then(|v| v.as_str()),
            Some("/Knowledge Base/x.md")
        );
    }

    #[test]
    fn parses_trailing_tool_call_block() {
        let reply = "Reasoning here.\n```json\n{\"tool_calls\":[{\"tool\":\"git.status\",\"args\":{}}]}\n```";
        let calls = parse_tool_calls(reply).unwrap().expect("should find calls");
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].tool, "git.status");
    }

    #[test]
    fn no_block_is_none_not_error() {
        assert_eq!(parse_tool_calls("just a final answer").unwrap(), None);
        // A valid change-set JSON (has "operations", not "tool_calls") is a
        // final result, not a tool call.
        let cs = "```json\n{\"operations\":[]}\n```";
        assert_eq!(parse_tool_calls(cs).unwrap(), None);
    }

    #[test]
    fn malformed_tool_calls_block_errors() {
        // Present but the wrong shape → a parse error, never a silent empty.
        let bad = "```json\n{\"tool_calls\":\"not-an-array\"}\n```";
        assert!(parse_tool_calls(bad).is_err());
    }

    /// Observed live: the Documentation Agent emitted a documentation.write
    /// whose tool_calls JSON was missing the closing `]` — the task failed
    /// immediately and every dependent Data-agent task was blocked ("why did
    /// it stop?"). A malformed block must get a correction round, and the
    /// corrected call must then execute.
    #[test]
    fn malformed_tool_call_block_gets_a_correction_round() {
        let mut inner = ScriptInvoker {
            replies: vec![
                // Missing `]` before the final `}` — the live failure shape.
                "The handoff.\n```json\n{\"tool_calls\":[{\"tool\":\"documentation.write\",\"args\":{\"path\":\"/Knowledge Base/x.md\",\"content\":\"# X\"}}}\n```".into(),
                // Corrected block on the retry.
                "The handoff.\n```json\n{\"tool_calls\":[{\"tool\":\"documentation.write\",\"args\":{\"path\":\"/Knowledge Base/x.md\",\"content\":\"# X\"}}]}\n```".into(),
                "Handoff written.".into(),
            ],
            calls: 0,
        };
        let mut backend = EchoBackend { executed: 0 };
        let evidence = Arc::new(Mutex::new(Vec::new()));
        let declared_tools = declared(&["documentation.write"]);
        let mut inv = ToolExecutingInvoker::new(
            &mut inner,
            &mut backend,
            move |_agent: &str| declared_tools.clone(),
            evidence.clone(),
            4,
        );
        let out = inv
            .invoke("Documentation Agent", "sys", "task")
            .expect("correction round must rescue the task");
        drop(inv);
        assert_eq!(out, "Handoff written.");
        assert_eq!(backend.executed, 1, "the corrected call must actually run");
    }

    /// Observed live: gemma invoked the fenced tool natively — the transport
    /// bridges it back as a fenced block whose name keeps the native
    /// underscore form ("indexed_file_write"). The executor must map it to the
    /// declared dotted name and run it, not flag fabricated tool use.
    #[test]
    fn native_underscore_tool_names_resolve_to_declared_dotted_tools() {
        let mut inner = ScriptInvoker {
            replies: vec![
                "Creating the file.\n```json\n{\"tool_calls\":[{\"tool\":\"indexed_file_write\",\"args\":{\"path\":\"indexed/x.cidx\"}}]}\n```".into(),
                "File created.".into(),
            ],
            calls: 0,
        };
        let mut backend = EchoBackend { executed: 0 };
        let evidence = Arc::new(Mutex::new(Vec::new()));
        let declared_tools = declared(&["indexed_file.write"]);
        let mut inv = ToolExecutingInvoker::new(
            &mut inner,
            &mut backend,
            move |_agent: &str| declared_tools.clone(),
            evidence.clone(),
            4,
        );
        let out = inv
            .invoke("Data (Indexed File) Agent", "sys", "task")
            .expect("underscore alias must execute");
        drop(inv);
        assert_eq!(out, "File created.");
        assert_eq!(backend.executed, 1, "the aliased call must actually run");
        // A genuinely unknown tool is still a critical defect — after the one
        // fix-and-repeat round, a repeated fabrication fails the task.
        let made_up =
            "```json\n{\"tool_calls\":[{\"tool\":\"made_up_tool\",\"args\":{}}]}\n```".to_string();
        let mut inner = ScriptInvoker {
            replies: vec![made_up.clone(), made_up],
            calls: 0,
        };
        let mut backend = EchoBackend { executed: 0 };
        let declared_tools = declared(&["indexed_file.write"]);
        let mut inv = ToolExecutingInvoker::new(
            &mut inner,
            &mut backend,
            move |_agent: &str| declared_tools.clone(),
            Arc::new(Mutex::new(Vec::new())),
            4,
        );
        let err = inv
            .invoke("Data (Indexed File) Agent", "sys", "task")
            .unwrap_err();
        assert!(err.contains("CRITICAL DEFECT"), "{err}");
    }

    #[test]
    fn declared_tool_executes_and_threads_result() {
        // Call 1 asks for a tool; call 2 (after seeing results) is the final answer.
        let mut inner = ScriptInvoker {
            replies: vec![
                "```json\n{\"tool_calls\":[{\"tool\":\"git.status\",\"args\":{}}]}\n```".into(),
                "All done — clean tree.".into(),
            ],
            calls: 0,
        };
        let mut backend = EchoBackend { executed: 0 };
        let evidence = Arc::new(Mutex::new(Vec::new()));
        let mut inv = ToolExecutingInvoker::new(
            &mut inner,
            &mut backend,
            |_| declared(&["git.status"]),
            evidence.clone(),
            4,
        );
        let out = inv
            .invoke("Version Control Agent", "", "commit please")
            .unwrap();
        drop(inv); // release the &mut borrows before inspecting inner/backend
        assert_eq!(out, "All done — clean tree.");
        assert_eq!(backend.executed, 1, "backend ran the declared tool once");
        let ev = evidence.lock().unwrap();
        assert_eq!(ev.len(), 1, "one tool call recorded as evidence");
        assert_eq!(ev[0].tool, "git.status");
        assert!(ev[0].ok);
    }

    #[test]
    fn undeclared_tool_fails_the_task() {
        let push = "```json\n{\"tool_calls\":[{\"tool\":\"git.push\",\"args\":{}}]}\n```".to_string();
        let mut inner = ScriptInvoker {
            replies: vec![push.clone(), push],
            calls: 0,
        };
        let mut backend = EchoBackend { executed: 0 };
        let evidence = Arc::new(Mutex::new(Vec::new()));
        let mut inv = ToolExecutingInvoker::new(
            &mut inner,
            &mut backend,
            |_| declared(&["git.status"]), // git.push NOT declared
            evidence.clone(),
            4,
        );
        let err = inv
            .invoke("Version Control Agent", "", "push please")
            .unwrap_err();
        drop(inv);
        assert!(
            err.contains("CRITICAL DEFECT"),
            "undeclared tool fails the task: {err}"
        );
        assert!(
            err.contains("check \u{201c}Version Control Agent\u{201d}'s prompt"),
            "the failure asks the developer to check the prompt: {err}"
        );
        assert_eq!(
            backend.executed, 0,
            "undeclared tool never reaches the backend"
        );
        assert_eq!(
            evidence.lock().unwrap().len(),
            2,
            "both rejections are recorded"
        );
    }

    /// Fallback contract: a REJECTED (critical) tool call is relayed back to
    /// the SAME agent for exactly one fix-and-repeat round; a corrected call
    /// then executes normally. The host never patches the call itself.
    #[test]
    fn rejected_tool_call_gets_one_fix_and_repeat_round() {
        /// Rejects calls without a "path" argument, accepts the rest.
        struct PickyBackend {
            executed: usize,
        }
        impl ToolBackend for PickyBackend {
            fn execute(&mut self, _agent: &str, call: &ToolCall) -> ToolResult {
                if call.args.get("path").is_none() {
                    ToolResult::critical("malformed indexed_file.write call: path is required")
                } else {
                    self.executed += 1;
                    ToolResult::ok("saved indexed/x.cidx", "")
                }
            }
        }
        let mut inner = ScriptInvoker {
            replies: vec![
                // Round 1: parameter error (no path) — rejected.
                "```json\n{\"tool_calls\":[{\"tool\":\"indexed_file.write\",\"args\":{\"name\":\"X\"}}]}\n```".into(),
                // Round 2: the agent fixed its own call — executes.
                "```json\n{\"tool_calls\":[{\"tool\":\"indexed_file.write\",\"args\":{\"path\":\"indexed/x.cidx\"}}]}\n```".into(),
                "File created.".into(),
            ],
            calls: 0,
        };
        let mut backend = PickyBackend { executed: 0 };
        let evidence = Arc::new(Mutex::new(Vec::new()));
        let declared_tools = declared(&["indexed_file.write"]);
        let mut inv = ToolExecutingInvoker::new(
            &mut inner,
            &mut backend,
            move |_agent: &str| declared_tools.clone(),
            evidence.clone(),
            6,
        );
        let out = inv
            .invoke("Data (Indexed File) Agent", "sys", "task")
            .expect("the fix-and-repeat round must rescue the task");
        drop(inv);
        assert_eq!(out, "File created.");
        assert_eq!(backend.executed, 1, "the corrected call actually ran");
        let ev = evidence.lock().unwrap();
        assert_eq!(ev.len(), 2, "the rejection and the corrected run are both evidence");
        assert!(!ev[0].ok && ev[1].ok);
    }

    #[test]
    fn bounded_loop_terminates() {
        // Inner ALWAYS asks for a tool → the cap must stop it.
        let looping = "```json\n{\"tool_calls\":[{\"tool\":\"git.status\",\"args\":{}}]}\n```";
        let mut inner = ScriptInvoker {
            replies: vec![looping.into(); 10],
            calls: 0,
        };
        let mut backend = EchoBackend { executed: 0 };
        let evidence = Arc::new(Mutex::new(Vec::new()));
        let mut inv = ToolExecutingInvoker::new(
            &mut inner,
            &mut backend,
            |_| declared(&["git.status"]),
            evidence.clone(),
            2,
        );
        let err = inv.invoke("A", "", "go").unwrap_err();
        drop(inv);
        assert!(err.contains("exceeded 2 round"), "loop cap enforced: {err}");
        // 2 executions, then a 3rd invoke that still asked → cap tripped.
        assert_eq!(backend.executed, 2);
        assert_eq!(inner.calls, 3);
    }

    // ── IdeToolBackend (T6) ─────────────────────────────────────────────────

    fn init_repo(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "prc-be-{tag}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let git = |args: &[&str]| {
            assert!(Command::new("git")
                .args(args)
                .current_dir(&dir)
                .output()
                .unwrap()
                .status
                .success());
        };
        git(&["init", "-q"]);
        git(&["config", "user.email", "t@example.com"]);
        git(&["config", "user.name", "Tester"]);
        std::fs::write(dir.join("f.txt"), "x\n").unwrap();
        git(&["add", "f.txt"]);
        git(&["commit", "-q", "-m", "seed"]);
        dir
    }

    fn git_call(argv: &[&str]) -> ToolCall {
        ToolCall {
            tool: "git.run".into(),
            args: serde_json::json!({ "argv": argv }),
        }
    }

    #[test]
    fn backend_runs_autonomous_git_without_confirm() {
        let repo = init_repo("auto");
        let mut confirms = 0;
        let mut confirm = |_r: GitConfirmRequest| {
            confirms += 1;
            true
        };
        let mut _pick = |_: TargetRequest| -> Option<TargetChoice> { None };
        let mut be = IdeToolBackend::new(repo.clone(), &mut confirm, &mut _pick);
        let res = be.execute(
            "Version Control Agent",
            &git_call(&["status", "--porcelain"]),
        );
        drop(be);
        assert!(res.ok, "autonomous status ran: {res:?}");
        assert_eq!(confirms, 0, "autonomous ops never ask for confirmation");
        let _ = std::fs::remove_dir_all(&repo);
    }

    #[test]
    fn backend_gates_push_behind_confirm() {
        let repo = init_repo("gate");
        // Deny path: the op must not run.
        let mut asked = Vec::new();
        let mut deny = |r: GitConfirmRequest| {
            asked.push(r.command);
            false
        };
        let mut _pick = |_: TargetRequest| -> Option<TargetChoice> { None };
        let mut be = IdeToolBackend::new(repo.clone(), &mut deny, &mut _pick);
        let res = be.execute(
            "Version Control Agent",
            &git_call(&["push", "origin", "main"]),
        );
        drop(be);
        assert!(!res.ok, "declined push does not succeed");
        assert!(res.summary.contains("declined"), "{res:?}");
        assert_eq!(asked.len(), 1, "the gated op prompted exactly once");
        assert_eq!(
            asked[0], "git push origin main",
            "confirm shows the exact command"
        );
        let _ = std::fs::remove_dir_all(&repo);
    }

    #[test]
    fn backend_rejects_unrecognised_op() {
        let repo = init_repo("rej");
        let mut confirm = |_r: GitConfirmRequest| true;
        let mut _pick = |_: TargetRequest| -> Option<TargetChoice> { None };
        let mut be = IdeToolBackend::new(repo.clone(), &mut confirm, &mut _pick);
        let res = be.execute("Version Control Agent", &git_call(&["frobnicate"]));
        drop(be);
        assert!(!res.ok, "unrecognised op is rejected");
        assert!(!res.critical, "rejection is recoverable, not a task-killer");
        let _ = std::fs::remove_dir_all(&repo);
    }

    #[test]
    fn backend_unknown_namespace_is_critical() {
        let mut confirm = |_r: GitConfirmRequest| true;
        let mut _pick = |_: TargetRequest| -> Option<TargetChoice> { None };
        let mut be = IdeToolBackend::new(std::env::temp_dir(), &mut confirm, &mut _pick);
        let res = be.execute(
            "Anyone",
            &ToolCall {
                tool: "svn.commit".into(),
                args: serde_json::Value::Null,
            },
        );
        drop(be);
        assert!(
            res.critical,
            "an unknown tool namespace is a critical defect"
        );
    }

    /// The boundary is what keeps agents inside the developer's project, so it
    /// must state both halves: what they own, and that the IDE is off limits.
    #[test]
    fn scope_boundary_forbids_touching_the_ide() {
        let b = PROJECT_SCOPE_BOUNDARY;
        assert!(b.contains("DEVELOPER'S OPEN PROJECT"));
        for allowed in ["forms", "indexed-file", "Knowledge Base", "events"] {
            assert!(b.contains(allowed), "boundary must permit {allowed} work");
        }
        for forbidden in ["off limits", "IDE settings", "model profiles"] {
            assert!(b.contains(forbidden), "boundary must forbid {forbidden}");
        }
        // The boundary is unconditional — it is not part of any tool contract,
        // so an agent with no declared tools still receives it.
        assert!(!tool_contract_appendix(&declared(&[])).contains("off limits"));
    }

    #[test]
    fn change_set_contract_teaches_only_real_operations() {
        let c = CHANGE_SET_CONTRACT;
        for op in [
            "deploy_control",
            "set_property",
            "generate_event_handler",
            "create_procedure",
        ] {
            assert!(c.contains(op), "contract must document {op}");
        }
        assert!(c.contains("\\\"control_id\\\":\\\"Form\\\"") || c.contains("control_id"));
        assert!(c.contains("UPDATE_FORM_PROPERTY"), "names the trap it closes");
    }

    #[test]
    fn contract_appendix_matches_declared_tools() {
        let git = tool_contract_appendix(&declared(&["git.run"]));
        assert!(
            git.contains("git.run") && git.contains("GATED"),
            "git contract present"
        );
        assert!(
            !git.contains("egui_tree"),
            "no egui contract when not declared"
        );

        // Native-tool contracts (Rig migration phase 2): knowledge/egui are
        // described as native function tools, not fenced tool_calls blocks.
        let egui = tool_contract_appendix(&declared(&["egui.tree", "egui.rects"]));
        assert!(egui.contains("egui_tree") && egui.to_lowercase().contains("read-only"));
        assert!(!egui.contains("git.run"));

        let docs = tool_contract_appendix(&declared(&["documentation.write", "knowledge.search"]));
        assert!(docs.contains("documentation.write"));
        assert!(docs.contains("knowledge_search"));
        assert!(docs.contains("SQLite vector"));

        let indexed = tool_contract_appendix(&declared(&["indexed_file.write"]));
        assert!(indexed.contains("indexed_file.write"));
        assert!(indexed.contains("1nf"));
        assert!(indexed.contains("UUID"));

        assert!(
            tool_contract_appendix(&HashSet::new()).is_empty(),
            "no tools → no appendix"
        );
    }

    #[test]
    fn only_documentation_agent_can_write_and_index_project_documents() {
        let root = std::env::temp_dir().join(format!(
            "prc-doc-tool-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&root).unwrap();
        let call = ToolCall {
            tool: "documentation.write".into(),
            args: serde_json::json!({
                "path": "/Knowledge Base/Projects/ERP/plan/plan.md",
                "content": "# ERP plan\n\nAccounts payable implementation tasks."
            }),
        };
        let mut confirm = |_request: GitConfirmRequest| false;
        let mut _pick = |_: TargetRequest| -> Option<TargetChoice> { None };
        let mut backend = IdeToolBackend::new(root.clone(), &mut confirm, &mut _pick);
        let rejected = backend.execute("Form Designer Agent", &call);
        assert!(rejected.critical);
        assert!(!root
            .join("Knowledge Base/Projects/ERP/plan/plan.md")
            .exists());

        let written = backend.execute(crate::agents_db::DOCUMENTATION_AGENT, &call);
        assert!(written.ok, "{written:?}");
        assert!(root
            .join("Knowledge Base/Projects/ERP/plan/plan.md")
            .exists());
        assert!(cobolt_agents::project_knowledge::database_path(&root).exists());

        let found = backend.execute(
            "Form Designer Agent",
            &ToolCall {
                tool: "knowledge.search".into(),
                args: serde_json::json!({"query": "accounts payable tasks", "limit": 3}),
            },
        );
        assert!(found.ok, "{found:?}");
        assert!(found
            .detail
            .contains("Knowledge Base/Projects/ERP/plan/plan.md"));
        let _ = std::fs::remove_dir_all(root);
    }

    fn indexed_tool_root(tag: &str) -> PathBuf {
        let root = std::env::temp_dir().join(format!(
            "prc-indexed-tool-{tag}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&root).unwrap();
        root
    }

    fn indexed_write_call() -> ToolCall {
        ToolCall {
            tool: "indexed_file.write".into(),
            args: serde_json::json!({
                "path": "indexed/customers.cidx",
                "name": "CUSTOMER-FILE",
                "purpose": "Customer master for billing",
                "assign_path": "data/customers.idx",
                "record": "01 CUSTOMER-RECORD.\n    05 CUSTOMER-ID PIC X(36).\n    05 CUSTOMER-NAME PIC X(80).",
                "primary_key": "CUSTOMER-ID",
                "alternate_keys": [{"field": "CUSTOMER-NAME", "duplicates": true}],
                "access_mode": "dynamic",
                "storage": "disk",
                "id_definitions": {"CUSTOMER-ID": "UUID"},
                "normalization": {
                    "1nf": "All attributes are atomic and there are no repeating groups.",
                    "2nf": "The primary key has one field, so no partial dependency exists.",
                    "3nf": "CUSTOMER-NAME depends only on CUSTOMER-ID."
                }
            }),
        }
    }

    #[test]
    fn indexed_file_tools_are_reserved_for_the_data_agent() {
        let root = indexed_tool_root("ownership");
        let mut confirm = |_request: GitConfirmRequest| false;
        let mut _pick = |_: TargetRequest| -> Option<TargetChoice> { None };
        let mut backend = IdeToolBackend::new(root.clone(), &mut confirm, &mut _pick);
        let result = backend.execute(crate::agents_db::DOCUMENTATION_AGENT, &indexed_write_call());
        assert!(
            result.critical,
            "another agent must not mutate indexed files"
        );
        assert!(!root.join("indexed/customers.cidx").exists());
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn indexed_file_write_requires_normalization_and_explicit_id_policy() {
        let root = indexed_tool_root("governance");
        let mut call = indexed_write_call();
        call.args.as_object_mut().unwrap().remove("normalization");
        let mut confirm = |_request: GitConfirmRequest| false;
        let mut _pick = |_: TargetRequest| -> Option<TargetChoice> { None };
        let mut backend = IdeToolBackend::new(root.clone(), &mut confirm, &mut _pick);
        let result = backend.execute(crate::agents_db::DATA_INDEXED_FILE_AGENT, &call);
        assert!(result.critical);
        assert!(result.detail.contains("normalization"));

        let mut call = indexed_write_call();
        call.args.as_object_mut().unwrap().remove("id_definitions");
        let result = backend.execute(crate::agents_db::DATA_INDEXED_FILE_AGENT, &call);
        assert!(result.critical);
        assert!(result.detail.contains("ID field"));
        assert!(!root.join("indexed/customers.cidx").exists());
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn indexed_file_write_governs_camel_case_id_fields() {
        let root = indexed_tool_root("camel-ids");
        let camel_record = "01 COMPANY-RECORD.\n    05 CompanyID PIC X(36).\n    05 CompanyName PIC X(80).\n    05 LegalRepID PIC X(36).";
        let mut call = indexed_write_call();
        {
            let args = call.args.as_object_mut().unwrap();
            args.insert("record".into(), serde_json::json!(camel_record));
            args.insert("primary_key".into(), serde_json::json!("CompanyID"));
            args.insert("alternate_keys".into(), serde_json::json!([]));
            args.remove("id_definitions");
        }
        let mut confirm = |_request: GitConfirmRequest| false;
        let mut _pick = |_: TargetRequest| -> Option<TargetChoice> { None };
        let mut backend = IdeToolBackend::new(root.clone(), &mut confirm, &mut _pick);
        let result = backend.execute(crate::agents_db::DATA_INDEXED_FILE_AGENT, &call);
        assert!(
            result.critical,
            "camelCase ID fields must demand id_definitions: {result:?}"
        );
        assert!(result.detail.contains("ID field"));

        // With the developer's UUID choices supplied, the same write succeeds
        // and the evidence names each governed camelCase ID field.
        call.args.as_object_mut().unwrap().insert(
            "id_definitions".into(),
            serde_json::json!({"CompanyID": "UUID", "LegalRepID": "UUID"}),
        );
        let result = backend.execute(crate::agents_db::DATA_INDEXED_FILE_AGENT, &call);
        assert!(result.ok, "{result:?}");
        assert!(result.detail.contains("CompanyID: UUID"), "{result:?}");
        assert!(result.detail.contains("LegalRepID: UUID"), "{result:?}");
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn indexed_file_write_generates_ui_artifacts_and_preserves_existing_data() {
        let root = indexed_tool_root("write");
        let mut confirm = |_request: GitConfirmRequest| false;
        let mut _pick = |_: TargetRequest| -> Option<TargetChoice> { None };
        let mut backend = IdeToolBackend::new(root.clone(), &mut confirm, &mut _pick);
        let result = backend.execute(
            crate::agents_db::DATA_INDEXED_FILE_AGENT,
            &indexed_write_call(),
        );
        assert!(result.ok, "{result:?}");
        for relative in [
            "indexed/customers.cidx",
            "generated/customers-indexed.cbl",
            "COPYBOOKS/customers.SEL",
            "COPYBOOKS/customers.FD",
            "COPYBOOKS/CUSTOMER-FILE.fd.cpy",
            "data/customers.idx",
        ] {
            assert!(root.join(relative).exists(), "missing {relative}");
        }
        let loaded = cobolt_indexed::load_indexed(root.join("indexed/customers.cidx")).unwrap();
        assert_eq!(loaded.comment, "Customer master for billing");
        assert_eq!(loaded.keys.primary.parts[0].field_name, "CUSTOMER-ID");
        assert!(take_indexed_files_changed(&root));

        std::fs::write(root.join("data/customers.idx"), b"existing-data-sentinel").unwrap();
        let second = backend.execute(
            crate::agents_db::DATA_INDEXED_FILE_AGENT,
            &indexed_write_call(),
        );
        assert!(second.ok, "{second:?}");
        assert_eq!(
            std::fs::read(root.join("data/customers.idx")).unwrap(),
            b"existing-data-sentinel",
            "definition maintenance must never truncate existing indexed data"
        );
        let before_definition = std::fs::read(root.join("indexed/customers.cidx")).unwrap();
        let mut structural_change = indexed_write_call();
        structural_change.args["record"] = serde_json::Value::String(
            "01 CUSTOMER-RECORD.\n    05 CUSTOMER-ID PIC X(36).\n    05 CUSTOMER-NAME PIC X(81)."
                .into(),
        );
        // A structural change to a finalized file is not a defect to fix: it
        // asks the developer to confirm a destroy-and-recreate, and leaves the
        // definition untouched until they do.
        let blocked = backend.execute(
            crate::agents_db::DATA_INDEXED_FILE_AGENT,
            &structural_change,
        );
        assert!(blocked.needs_confirmation, "{blocked:?}");
        assert!(!blocked.critical);
        assert!(blocked.detail.contains("DESTROY and RECREATE"));
        assert_eq!(
            std::fs::read(root.join("indexed/customers.cidx")).unwrap(),
            before_definition,
            "a confirmation-required result must leave the definition untouched"
        );

        // With the developer's explicit confirmation, the same change overwrites
        // (recreates) the locked file.
        structural_change.args["confirm_recreate"] = serde_json::Value::Bool(true);
        let recreated = backend.execute(
            crate::agents_db::DATA_INDEXED_FILE_AGENT,
            &structural_change,
        );
        assert!(recreated.ok, "{recreated:?}");
        let loaded = cobolt_indexed::load_indexed(root.join("indexed/customers.cidx")).unwrap();
        assert!(
            loaded
                .record_root()
                .map(cobolt_indexed::IndexedField::all_leaves)
                .unwrap_or_default()
                .iter()
                .any(|field| field.name == "CUSTOMER-NAME" && field.pic.contains("81")),
            "the confirmed recreate must apply the new schema"
        );
        let _ = take_indexed_files_changed(&root);
        let _ = std::fs::remove_dir_all(root);
    }

    /// The tool loop turns a confirmation-required result into Grace's reply —
    /// no fix-and-repeat, no task failure — so the developer decides.
    #[test]
    fn confirmation_required_tool_result_surfaces_to_the_developer() {
        struct LockedBackend;
        impl ToolBackend for LockedBackend {
            fn execute(&mut self, _agent: &str, _call: &ToolCall) -> ToolResult {
                ToolResult::needs_confirmation(
                    "conta-a-pagar.cidx is finalized; confirm destroy and recreate.",
                )
            }
        }
        let mut inner = ScriptInvoker {
            replies: vec![
                "```json\n{\"tool_calls\":[{\"tool\":\"indexed_file.write\",\"args\":{\"path\":\"indexed/conta-a-pagar.cidx\"}}]}\n```".into(),
            ],
            calls: 0,
        };
        let mut backend = LockedBackend;
        let evidence = Arc::new(Mutex::new(Vec::new()));
        let declared_tools = declared(&["indexed_file.write"]);
        let mut inv = ToolExecutingInvoker::new(
            &mut inner,
            &mut backend,
            move |_agent: &str| declared_tools.clone(),
            evidence.clone(),
            6,
        );
        let out = inv
            .invoke("Data (Indexed File) Agent", "sys", "add a field")
            .expect("a confirmation request is not a task failure");
        drop(inv);
        assert!(out.contains("confirm destroy and recreate"), "{out}");
        // The block was recorded once; the agent was never asked to retry.
        assert_eq!(inner.calls, 1, "no fix-and-repeat round for a confirmation");
    }

    #[test]
    fn end_to_end_only_declaring_agent_reaches_git() {
        // VC agent declares git.run → executes; another agent does not → critical.
        let repo = init_repo("e2e");
        for (agent, declares, expect_ok) in [
            ("Version Control Agent", true, true),
            ("Form Designer Agent", false, false),
        ] {
            let git_call = "```json\n{\"tool_calls\":[{\"tool\":\"git.run\",\"args\":{\"argv\":[\"status\",\"--porcelain\"]}}]}\n```".to_string();
            let mut inner = ScriptInvoker {
                // The declaring agent finishes after one call; the undeclared
                // one insists through its fix-and-repeat round and fails.
                replies: if declares {
                    vec![git_call, "Working tree is clean.".into()]
                } else {
                    vec![git_call.clone(), git_call]
                },
                calls: 0,
            };
            let mut confirm = |_r: GitConfirmRequest| true;
            let mut _pick = |_: TargetRequest| -> Option<TargetChoice> { None };
            let mut backend = IdeToolBackend::new(repo.clone(), &mut confirm, &mut _pick);
            let evidence = Arc::new(Mutex::new(Vec::new()));
            let declared_set: HashSet<String> = if declares {
                declared(&["git.run"])
            } else {
                HashSet::new()
            };
            let mut inv = ToolExecutingInvoker::new(
                &mut inner,
                &mut backend,
                move |_| declared_set.clone(),
                evidence.clone(),
                4,
            );
            let out = inv.invoke(agent, "", "check status");
            drop(inv);
            if expect_ok {
                assert_eq!(out.unwrap(), "Working tree is clean.");
                assert!(evidence.lock().unwrap()[0].ok);
            } else {
                assert!(
                    out.unwrap_err().contains("CRITICAL DEFECT"),
                    "{agent} must be blocked"
                );
            }
        }
        let _ = std::fs::remove_dir_all(&repo);
    }

    // ── project.select_target (spec 034) ────────────────────────────────────
    mod project_select_target {
        use super::*;
        use crate::project_model::{Category, CoboltProject};
        use crate::target_select::{TargetChoice, TargetOp, TargetRequest};
        use std::cell::RefCell;

        fn call(op: &str, kind: &str, name: &str) -> ToolCall {
            ToolCall {
                tool: "project.select_target".into(),
                args: serde_json::json!({ "op": op, "kind": kind, "name": name }),
            }
        }

        fn tmp() -> PathBuf {
            use std::sync::atomic::{AtomicU64, Ordering};
            static SEQ: AtomicU64 = AtomicU64::new(0);
            let nanos = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos();
            let d = std::env::temp_dir().join(format!(
                "prc_pst_{}_{}_{}",
                std::process::id(),
                nanos,
                SEQ.fetch_add(1, Ordering::Relaxed)
            ));
            std::fs::create_dir_all(&d).unwrap();
            d
        }

        #[test]
        fn create_always_asks_and_returns_chosen_folder() {
            let dir = tmp();
            let seen = RefCell::new(Vec::<TargetOp>::new());
            let mut confirm = |_: GitConfirmRequest| true;
            let mut pick = |req: TargetRequest| {
                seen.borrow_mut().push(req.op);
                Some(TargetChoice {
                    rel_path: "forms/customers".into(),
                })
            };
            let mut be = IdeToolBackend::new(dir.clone(), &mut confirm, &mut pick);
            let res = be.execute("Form Designer Agent", &call("create", "form", "invoice"));
            drop(be);
            assert!(res.ok);
            assert_eq!(res.detail, "forms/customers");
            assert_eq!(*seen.borrow(), vec![TargetOp::Create]);
            let _ = std::fs::remove_dir_all(&dir);
        }

        #[test]
        fn edit_single_match_resolves_without_asking() {
            let dir = tmp();
            let mut proj = CoboltProject::new("T", "src/main.cbl");
            proj.add_file_to("forms/a/order.cfrm", Category::Forms);
            crate::project_model::save_project(&proj, &dir.join("cobolt.toml")).unwrap();

            let asked = RefCell::new(false);
            let mut confirm = |_: GitConfirmRequest| true;
            let mut pick = |_req: TargetRequest| {
                *asked.borrow_mut() = true;
                None
            };
            let mut be = IdeToolBackend::new(dir.clone(), &mut confirm, &mut pick);
            let res = be.execute("Form Designer Agent", &call("edit", "form", "order"));
            drop(be);
            assert!(res.ok);
            assert_eq!(res.detail, "forms/a/order.cfrm");
            assert!(!*asked.borrow(), "a single match must not open the modal");
            let _ = std::fs::remove_dir_all(&dir);
        }

        #[test]
        fn edit_ambiguous_asks_with_candidates() {
            let dir = tmp();
            let mut proj = CoboltProject::new("T", "src/main.cbl");
            proj.add_file_to("forms/a/order.cfrm", Category::Forms);
            proj.add_file_to("forms/b/order.cfrm", Category::Forms);
            crate::project_model::save_project(&proj, &dir.join("cobolt.toml")).unwrap();

            let count = RefCell::new(0usize);
            let mut confirm = |_: GitConfirmRequest| true;
            let mut pick = |req: TargetRequest| {
                *count.borrow_mut() = req.candidates.len();
                Some(TargetChoice {
                    rel_path: "forms/b/order.cfrm".into(),
                })
            };
            let mut be = IdeToolBackend::new(dir.clone(), &mut confirm, &mut pick);
            let res = be.execute("Form Designer Agent", &call("edit", "form", "order"));
            drop(be);
            assert!(res.ok);
            assert_eq!(res.detail, "forms/b/order.cfrm");
            assert_eq!(*count.borrow(), 2, "the modal receives both candidates");
            let _ = std::fs::remove_dir_all(&dir);
        }

        #[test]
        fn cancel_stops_the_workflow_cleanly() {
            let dir = tmp();
            let mut confirm = |_: GitConfirmRequest| true;
            let mut pick = |_req: TargetRequest| None; // developer cancelled
            let mut be = IdeToolBackend::new(dir.clone(), &mut confirm, &mut pick);
            let res = be.execute("Form Designer Agent", &call("create", "form", "invoice"));
            drop(be);
            assert!(res.needs_confirmation, "cancel halts the loop, not a defect");
            assert!(!res.critical);
            let _ = std::fs::remove_dir_all(&dir);
        }
    }
}
