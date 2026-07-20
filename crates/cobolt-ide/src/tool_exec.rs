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

use cobolt_agents::grace::{last_json_block, AgentInvoker};

use crate::git_exec::{self, GitClass, GitConfirmRequest};

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
}

impl ToolResult {
    pub fn ok(summary: impl Into<String>, detail: impl Into<String>) -> Self {
        Self {
            ok: true,
            summary: summary.into(),
            detail: detail.into(),
            critical: false,
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
    let calls: Vec<ToolCall> = serde_json::from_value(tc.clone())
        .map_err(|e| format!("malformed tool_calls block: {e}"))?;
    if calls.is_empty() {
        return Ok(None);
    }
    Ok(Some(calls))
}

const GIT_TOOL_CONTRACT: &str = "\n\n--- Tool execution (git) — how you actually run git ---\nYou run git through one tool. To execute a command, END your reply with exactly one fenced JSON block and nothing after it:\n```json\n{\"tool_calls\":[{\"tool\":\"git.run\",\"args\":{\"argv\":[\"status\",\"--porcelain\"]}}]}\n```\n`argv` is the git argument vector WITHOUT the leading \"git\" (e.g. [\"commit\",\"-m\",\"message\"]). Do not pass -C, --git-dir, or --work-tree — the executor is already bound to the open project's repository and will reject them. Read and local-mutation ops run immediately; network and history-rewriting ops (push, fetch, pull, rebase, reset --hard) are GATED on explicit operator approval and will not run if declined. Each command returns its real exit status and output as a TOOL RESULTS block — a non-zero exit is a failure, never a success. When the task is complete, reply with your final result and DO NOT emit a tool_calls block.";

const EGUI_TOOL_CONTRACT: &str = "\n\n--- Tool execution (live UI — observe only) ---\nYou may inspect the rendered form to verify your work. To read the live widget tree, END your reply with exactly one fenced JSON block:\n```json\n{\"tool_calls\":[{\"tool\":\"egui.tree\",\"args\":{}}]}\n```\nThe UI tools (egui.tree, egui.rects) are READ-ONLY: they let you SEE the rendered UI; they do NOT change it. Every form edit must be expressed as change-set operations, never by driving the live UI. The observed tree returns as a TOOL RESULTS block. When finished, reply with your final result and DO NOT emit a tool_calls block.";

const DOCUMENTATION_TOOL_CONTRACT: &str = "\n\n--- Tool execution (project Knowledge Base) ---\nYou create and inspect project Knowledge Base documents through these tools. To write a Markdown document, END your reply with exactly one fenced JSON block and nothing after it:\n```json\n{\"tool_calls\":[{\"tool\":\"documentation.write\",\"args\":{\"path\":\"/Knowledge Base/Projects/example.md\",\"content\":\"# Example\\n\"}}]}\n```\nUse documentation.list with empty args and documentation.read with {\"path\":\"/Knowledge Base/...\"}. Writes are restricted to the open project's Knowledge Base/ folder and are automatically indexed in the project's SQLite vector database. A document exists only after a successful TOOL RESULTS response. When finished, reply with your final result and DO NOT emit a tool_calls block.";

const KNOWLEDGE_TOOL_CONTRACT: &str = "\n\n--- Tool execution (project knowledge retrieval) ---\nUse the project-local SQLite vector index before relying on prior plans, requirements, task lists, or other documentation. END your reply with exactly one fenced JSON block:\n```json\n{\"tool_calls\":[{\"tool\":\"knowledge.search\",\"args\":{\"query\":\"PowerRustERP approved implementation tasks\",\"limit\":5}}]}\n```\nUse only returned project paths and excerpts as evidence. When finished, reply with your final result and DO NOT emit a tool_calls block.";

const INDEXED_FILE_TOOL_CONTRACT: &str = r#"

--- Tool execution (PowerRustCOBOL Indexed File UI model) ---
Only Data (Indexed File) Agent may inspect or mutate indexed-file definitions through these tools.

List with `indexed_file.list` and empty args. Read with `indexed_file.read` and `{"path":"indexed/customers.cidx"}`.

Create or replace one complete definition with `indexed_file.write`:
```json
{"tool_calls":[{"tool":"indexed_file.write","args":{"path":"indexed/customers.cidx","name":"CUSTOMER-FILE","purpose":"Customer master data for invoicing","assign_path":"data/customers.idx","record":"       01 CUSTOMER-RECORD.\n          05 CUSTOMER-ID PIC X(36).\n          05 CUSTOMER-NAME PIC X(80).","primary_key":"CUSTOMER-ID","alternate_keys":[],"access_mode":"dynamic","storage":"disk","id_definitions":{"CUSTOMER-ID":"UUID"},"normalization":{"1nf":"Atomic customer attributes; no repeating groups.","2nf":"Single-field primary key; no partial dependencies.","3nf":"All non-key fields depend only on CUSTOMER-ID."}}}]}
```
`alternate_keys` entries use `{"field":"FIELD-NAME","duplicates":false}`. Set `finalized` explicitly when needed; new definitions default to finalized after validation. Every ID field requires an `id_definitions` entry whose value is either `UUID` or the exact `PIC ...` chosen by the developer. The `normalization` object must contain non-empty `1nf`, `2nf`, and `3nf` decisions from the approved Documentation Agent handoff. The write validates the COBOL record and key fields, saves the `.cidx`, and regenerates Indexed File UI COBOL/copybook artifacts. Finalized definitions preserve the Indexed File UI lock and reject structural changes. A helper normalized relation is a separate `indexed_file.write` call. Use TOOL RESULTS as evidence and never claim a file changed without a successful result."#;

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
    fn invoke(&mut self, agent: &str, system: &str, user: &str) -> Result<String, String> {
        let declared = (self.declared)(agent);
        let mut convo_user = user.to_string();
        let mut rounds = 0usize;
        loop {
            let reply = self.inner.invoke(agent, system, &convo_user)?;
            let calls = match parse_tool_calls(&reply)? {
                None => return Ok(reply), // final result — no tools requested
                Some(calls) => calls,
            };
            if rounds >= self.max_tool_rounds {
                return Err(format!(
                    "tool-call loop exceeded {} round(s) without a final result",
                    self.max_tool_rounds
                ));
            }
            rounds += 1;

            let mut rendered = String::new();
            for call in &calls {
                let res = if declared.contains(&call.tool) {
                    self.backend.execute(agent, call)
                } else {
                    ToolResult::critical(format!(
                        "Agent \u{201c}{agent}\u{201d} invoked undeclared tool \u{201c}{}\u{201d} — ungoverned/fabricated tool use is a critical defect (spec 029 R4).",
                        call.tool
                    ))
                };
                self.record(agent, call, &res);
                if res.critical {
                    // Abort the task: the engine records the Err as the failure
                    // reason (spec 030 R2/R3).
                    return Err(format!("CRITICAL DEFECT: {}", res.summary));
                }
                rendered.push_str(&format!(
                    "- {} [{}]: {}\n{}\n",
                    call.tool,
                    if res.ok { "ok" } else { "error" },
                    res.summary,
                    res.detail
                ));
            }
            convo_user = format!(
                "{convo_user}\n\n=== TOOL RESULTS (round {rounds}) ===\n{rendered}\nUse these real results. When the task is complete, reply with your final result and DO NOT emit a tool_calls block."
            );
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
}

impl<'a> IdeToolBackend<'a> {
    pub fn new(
        project_dir: PathBuf,
        confirm: &'a mut dyn FnMut(GitConfirmRequest) -> bool,
    ) -> Self {
        Self {
            project_dir,
            confirm,
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
        upper == "ID" || upper.ends_with("-ID") || upper.ends_with("_ID")
    }

    fn validate_id_definitions(
        call: &ToolCall,
        leaves: &[&cobolt_indexed::IndexedField],
    ) -> Result<String, ToolResult> {
        let id_fields: Vec<_> = leaves
            .iter()
            .filter(|field| Self::is_id_field(&field.name))
            .collect();
        if id_fields.is_empty() {
            return Ok("No ID fields are present in this record.".into());
        }
        let Some(definitions) = call
            .args
            .get("id_definitions")
            .and_then(|value| value.as_object())
        else {
            return Err(ToolResult::critical(
                "every ID field requires the developer's UUID or exact PIC choice in id_definitions",
            ));
        };
        let mut evidence = Vec::new();
        for field in id_fields {
            let choice = definitions
                .iter()
                .find(|(name, _)| name.eq_ignore_ascii_case(&field.name))
                .and_then(|(_, value)| value.as_str())
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .ok_or_else(|| {
                    ToolResult::critical(format!(
                        "ID field {} has no developer-approved UUID or PIC definition",
                        field.name
                    ))
                })?;
            if choice.eq_ignore_ascii_case("UUID") {
                if !field.pic.trim().eq_ignore_ascii_case("X(36)") {
                    return Err(ToolResult::critical(format!(
                        "ID field {} was approved as UUID and must use PIC X(36), but the record uses PIC {}",
                        field.name, field.pic
                    )));
                }
            } else if let Some(pic) = choice
                .strip_prefix("PIC ")
                .or_else(|| choice.strip_prefix("pic "))
            {
                if !field.pic.trim().eq_ignore_ascii_case(pic.trim()) {
                    return Err(ToolResult::critical(format!(
                        "ID field {} uses PIC {}, which does not match the developer-approved {}",
                        field.name, field.pic, choice
                    )));
                }
            } else {
                return Err(ToolResult::critical(format!(
                    "ID field {} must be defined as UUID or an exact PIC clause, not {}",
                    field.name, choice
                )));
            }
            evidence.push(format!("{}: {}", field.name, choice));
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
                        return ToolResult::critical(format!(
                            "{} is finalized; the Indexed File UI locks its schema and storage properties. The developer must explicitly unfinalize it in the UI before structural maintenance.",
                            relative.display()
                        ));
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
        } else {
            ToolResult::critical(format!(
                "unknown tool namespace for \u{201c}{}\u{201d}",
                call.tool
            ))
        }
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
        let mut inner = ScriptInvoker {
            replies: vec![
                "```json\n{\"tool_calls\":[{\"tool\":\"git.push\",\"args\":{}}]}\n```".into(),
            ],
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
        assert_eq!(
            backend.executed, 0,
            "undeclared tool never reaches the backend"
        );
        assert_eq!(
            evidence.lock().unwrap().len(),
            1,
            "the critical defect is recorded"
        );
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
        let mut be = IdeToolBackend::new(repo.clone(), &mut confirm);
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
        let mut be = IdeToolBackend::new(repo.clone(), &mut deny);
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
        let mut be = IdeToolBackend::new(repo.clone(), &mut confirm);
        let res = be.execute("Version Control Agent", &git_call(&["frobnicate"]));
        drop(be);
        assert!(!res.ok, "unrecognised op is rejected");
        assert!(!res.critical, "rejection is recoverable, not a task-killer");
        let _ = std::fs::remove_dir_all(&repo);
    }

    #[test]
    fn backend_unknown_namespace_is_critical() {
        let mut confirm = |_r: GitConfirmRequest| true;
        let mut be = IdeToolBackend::new(std::env::temp_dir(), &mut confirm);
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

    #[test]
    fn contract_appendix_matches_declared_tools() {
        let git = tool_contract_appendix(&declared(&["git.run"]));
        assert!(
            git.contains("git.run") && git.contains("GATED"),
            "git contract present"
        );
        assert!(
            !git.contains("egui.tree"),
            "no egui contract when not declared"
        );

        let egui = tool_contract_appendix(&declared(&["egui.tree", "egui.rects"]));
        assert!(egui.contains("egui.tree") && egui.to_lowercase().contains("read-only"));
        assert!(!egui.contains("git.run"));

        let docs = tool_contract_appendix(&declared(&["documentation.write", "knowledge.search"]));
        assert!(docs.contains("documentation.write"));
        assert!(docs.contains("knowledge.search"));
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
        let mut backend = IdeToolBackend::new(root.clone(), &mut confirm);
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
        let mut backend = IdeToolBackend::new(root.clone(), &mut confirm);
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
        let mut backend = IdeToolBackend::new(root.clone(), &mut confirm);
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
    fn indexed_file_write_generates_ui_artifacts_and_preserves_existing_data() {
        let root = indexed_tool_root("write");
        let mut confirm = |_request: GitConfirmRequest| false;
        let mut backend = IdeToolBackend::new(root.clone(), &mut confirm);
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
        let rejected = backend.execute(
            crate::agents_db::DATA_INDEXED_FILE_AGENT,
            &structural_change,
        );
        assert!(rejected.critical);
        assert!(rejected.detail.contains("is finalized"));
        assert_eq!(
            std::fs::read(root.join("indexed/customers.cidx")).unwrap(),
            before_definition,
            "a finalized schema rejection must leave the definition untouched"
        );
        let _ = take_indexed_files_changed(&root);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn end_to_end_only_declaring_agent_reaches_git() {
        // VC agent declares git.run → executes; another agent does not → critical.
        let repo = init_repo("e2e");
        for (agent, declares, expect_ok) in [
            ("Version Control Agent", true, true),
            ("Form Designer Agent", false, false),
        ] {
            let mut inner = ScriptInvoker {
                replies: vec![
                    "```json\n{\"tool_calls\":[{\"tool\":\"git.run\",\"args\":{\"argv\":[\"status\",\"--porcelain\"]}}]}\n```".into(),
                    "Working tree is clean.".into(),
                ],
                calls: 0,
            };
            let mut confirm = |_r: GitConfirmRequest| true;
            let mut backend = IdeToolBackend::new(repo.clone(), &mut confirm);
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
}
