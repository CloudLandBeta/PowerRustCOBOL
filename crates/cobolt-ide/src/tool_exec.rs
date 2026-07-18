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
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
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
        Self { ok: true, summary: summary.into(), detail: detail.into(), critical: false }
    }
    /// A recoverable failure (the tool ran but reported an error, e.g. a
    /// non-zero git exit). The agent sees it and may adjust.
    pub fn err(summary: impl Into<String>, detail: impl Into<String>) -> Self {
        Self { ok: false, summary: summary.into(), detail: detail.into(), critical: false }
    }
    /// A critical defect that must fail the whole task.
    pub fn critical(summary: impl Into<String>) -> Self {
        let s = summary.into();
        Self { ok: false, detail: s.clone(), summary: s, critical: true }
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
    out
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
        Self { project_dir, confirm }
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
            let approved = (self.confirm)(GitConfirmRequest { command: command.clone() });
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
                    out.status.map(|c| c.to_string()).unwrap_or_else(|| "signal".into()),
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
}

impl ToolBackend for IdeToolBackend<'_> {
    fn execute(&mut self, _agent: &str, call: &ToolCall) -> ToolResult {
        if call.tool.starts_with("git.") {
            self.exec_git(call)
        } else if call.tool.starts_with("egui.") {
            self.exec_egui(call)
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
            .map(|x| x.as_str().map(str::to_string).ok_or_else(|| "git arguments must be strings".to_string()))
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
        let out = inv.invoke("Version Control Agent", "", "commit please").unwrap();
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
        let err = inv.invoke("Version Control Agent", "", "push please").unwrap_err();
        drop(inv);
        assert!(err.contains("CRITICAL DEFECT"), "undeclared tool fails the task: {err}");
        assert_eq!(backend.executed, 0, "undeclared tool never reaches the backend");
        assert_eq!(evidence.lock().unwrap().len(), 1, "the critical defect is recorded");
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
        let res = be.execute("Version Control Agent", &git_call(&["status", "--porcelain"]));
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
        let res = be.execute("Version Control Agent", &git_call(&["push", "origin", "main"]));
        drop(be);
        assert!(!res.ok, "declined push does not succeed");
        assert!(res.summary.contains("declined"), "{res:?}");
        assert_eq!(asked.len(), 1, "the gated op prompted exactly once");
        assert_eq!(asked[0], "git push origin main", "confirm shows the exact command");
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
            &ToolCall { tool: "svn.commit".into(), args: serde_json::Value::Null },
        );
        drop(be);
        assert!(res.critical, "an unknown tool namespace is a critical defect");
    }

    #[test]
    fn contract_appendix_matches_declared_tools() {
        let git = tool_contract_appendix(&declared(&["git.run"]));
        assert!(git.contains("git.run") && git.contains("GATED"), "git contract present");
        assert!(!git.contains("egui.tree"), "no egui contract when not declared");

        let egui = tool_contract_appendix(&declared(&["egui.tree", "egui.rects"]));
        assert!(egui.contains("egui.tree") && egui.to_lowercase().contains("read-only"));
        assert!(!egui.contains("git.run"));

        assert!(tool_contract_appendix(&HashSet::new()).is_empty(), "no tools → no appendix");
    }

    #[test]
    fn end_to_end_only_declaring_agent_reaches_git() {
        // VC agent declares git.run → executes; another agent does not → critical.
        let repo = init_repo("e2e");
        for (agent, declares, expect_ok) in
            [("Version Control Agent", true, true), ("Form Designer Agent", false, false)]
        {
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
            let declared_set: HashSet<String> =
                if declares { declared(&["git.run"]) } else { HashSet::new() };
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
                assert!(out.unwrap_err().contains("CRITICAL DEFECT"), "{agent} must be blocked");
            }
        }
        let _ = std::fs::remove_dir_all(&repo);
    }
}
