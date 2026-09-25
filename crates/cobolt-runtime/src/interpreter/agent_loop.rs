// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Spec 072 — the `AgentObject` tool loop, on the interpreter.
//!
//! One `Ask` that offers tools becomes a short conversation: the question, the
//! model's tool calls, their results, and so on until the model answers in
//! text. The network part of every round runs on a worker thread exactly as a
//! plain `Ask` does (spec 032), under the SAME generation and pending entry, so
//! `Cancel`, `TimeoutSeconds` and a late reply behave as they always have —
//! for the whole question, not per round.
//!
//! Tools come from two places:
//! - the application's **consultable indexed files** (spec 065), searched here
//!   on the interpreter thread, read-only and bounded by 065's memory limit;
//! - tools the **program** declared with `AddTool`, answered by the program's
//!   own `onToolCall` handler. Those are presented ONE AT A TIME, because a
//!   call's details ride in the control's `ToolCallId` / `ToolName` /
//!   `ToolArguments` properties, which a second call would overwrite. The next
//!   `COBOL-WAIT-EVENT` after the handler was dispatched means it has returned:
//!   its `SetToolResult` — or nothing, which the model receives as an empty
//!   result — is taken there, and the loop moves on.

use super::Interpreter;
use crate::agent_runtime::{AskRequest, Protocol};
use crate::agent_tools::{self as at, Reply, ToolCall, ToolSpec, Turn, Usage};

/// A tool the program declared with `AddTool` / `AddToolParameter`.
#[derive(Debug, Clone, Default)]
pub(super) struct DeclaredTool {
    pub name: String,
    pub description: String,
    pub params: Vec<(String, String)>,
}

impl DeclaredTool {
    fn spec(&self) -> ToolSpec {
        let mut properties = serde_json::Map::new();
        for (name, description) in &self.params {
            properties.insert(
                name.clone(),
                serde_json::json!({"type": "string", "description": description}),
            );
        }
        ToolSpec {
            name: self.name.clone(),
            description: self.description.clone(),
            parameters: serde_json::json!({"type": "object", "properties": properties}),
        }
    }
}

/// Where a program-answered call stands.
#[derive(Debug, Clone, PartialEq)]
pub(super) enum Waiting {
    Nothing,
    /// `onToolCall` is queued; the handler has not run yet.
    Queued(ToolCall),
    /// The event was handed to the program; the next wait means it returned.
    Dispatched(ToolCall),
    /// Spec 068 — a KnowledgeBase search is running on a worker (the query
    /// needed the embedding server); its result moves the loop on.
    KbSearch(ToolCall),
}

/// One `Ask` in progress that offers tools.
#[derive(Debug, Clone)]
pub(super) struct ToolLoop {
    req: AskRequest,
    protocol: Protocol,
    fenced: bool,
    url: String,
    headers: Vec<(String, String)>,
    timeout_ms: u64,
    started_at: std::time::Instant,
    generation: u64,
    tools: Vec<ToolSpec>,
    turns: Vec<Turn>,
    rounds: u32,
    max_rounds: u32,
    usage: Usage,
    calls_made: u32,
    pending: std::collections::VecDeque<ToolCall>,
    results: Vec<(ToolCall, String)>,
    pub(super) waiting: Waiting,
}

impl Interpreter {
    /// The tools this agent offers: every consultable indexed file (065, for
    /// every agent — 072 Q2) and the ones the program declared on it.
    pub(super) fn agent_offered_tools(&self, obj: &str) -> Vec<ToolSpec> {
        // `ToolProtocol = None`: this agent is offered no tool at all, whatever
        // the program allowed — for a model that cannot call tools (spec 071,
        // 063 R55/R66). Files and Knowledge Bases are allowed program-wide, so
        // this is the one per-agent switch.
        if self.obj_get(obj, "ToolProtocol").trim().eq_ignore_ascii_case("none") {
            return Vec::new();
        }
        let mut tools: Vec<ToolSpec> = self
            .mcp_tools
            .tools()
            .into_iter()
            .map(|t| ToolSpec {
                name: t.name,
                description: t.description.unwrap_or_default(),
                parameters: t.input_schema,
            })
            .collect();
        tools.extend(self.kb_tool_specs());
        if let Some(declared) = self.agent_declared_tools.get(&obj.trim().to_ascii_uppercase()) {
            tools.extend(declared.iter().map(DeclaredTool::spec));
        }
        tools
    }

    /// The generation this agent's tool loop runs under — what a worker that
    /// answers one of its calls must report with.
    pub(super) fn tool_loop_generation(&self, obj: &str) -> Option<u64> {
        self.tool_loops.get(obj).map(|l| l.generation)
    }

    /// Spec 068 — a KnowledgeBase search a worker finished for this agent's
    /// tool loop: answer the call and move the loop on.
    pub(super) fn kb_tool_delivered(&mut self, obj: &str, call_id: &str, text: String) {
        let Some(l) = self.tool_loops.get_mut(obj) else {
            return;
        };
        let call = match &l.waiting {
            Waiting::KbSearch(call) if call.id == call_id => call.clone(),
            _ => return,
        };
        l.waiting = Waiting::Nothing;
        l.results.push((call, text));
        self.tool_loop_advance(obj);
    }

    /// Record a new tool-offering `Ask`; called by `agent_ask` once the
    /// request is on its way, with the generation and timeout it registered.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn agent_begin_tool_loop(
        &mut self,
        obj: &str,
        req: AskRequest,
        protocol: Protocol,
        fenced: bool,
        url: String,
        headers: Vec<(String, String)>,
        timeout_ms: u64,
        generation: u64,
        tools: Vec<ToolSpec>,
    ) {
        let max_rounds = self
            .obj_get(obj, "MaximumToolRounds")
            .trim()
            .parse::<u32>()
            .ok()
            .filter(|n| *n > 0)
            .unwrap_or(8);
        let prompt = req.prompt.clone();
        self.tool_loops.insert(
            obj.to_string(),
            ToolLoop {
                req,
                protocol,
                fenced,
                url,
                headers,
                timeout_ms,
                started_at: std::time::Instant::now(),
                generation,
                tools,
                turns: vec![Turn::User(prompt)],
                rounds: 0,
                max_rounds,
                usage: Usage::default(),
                calls_made: 0,
                pending: Default::default(),
                results: Vec::new(),
                waiting: Waiting::Nothing,
            },
        );
    }

    /// Is this loop still the one its control is waiting on? A `Cancel`, a
    /// timeout or a newer `Ask` bumps the generation, and a loop from before
    /// that is dropped without a word.
    fn tool_loop_live(&self, obj: &str) -> bool {
        let Some(l) = self.tool_loops.get(obj) else {
            return false;
        };
        let live = self
            .async_generations
            .get(obj)
            .map(|g| g.load(std::sync::atomic::Ordering::Relaxed))
            .unwrap_or(0);
        live == l.generation
    }

    /// One round's reply arrived (from `agent_delivered`).
    pub(super) fn tool_loop_delivered(&mut self, obj: &str, status: u16, body: &str) {
        let verbose = self.agent_is_verbose(obj);
        if verbose {
            self.agent_log(format!("[agent {obj}] ── response (tool loop) ─────────"));
            self.agent_log(format!("[agent {obj}] status: {status}"));
            self.agent_log_block(obj, "body", body);
        }
        if status == 0 {
            self.tool_loop_fail(obj, body.trim());
            return;
        }
        let fenced = self.tool_loops.get(obj).map(|l| l.fenced).unwrap_or(false);
        match at::parse_turn(status, body, fenced) {
            Err(message) => self.tool_loop_fail(obj, &message),
            Ok((reply, usage)) => {
                let Some(l) = self.tool_loops.get_mut(obj) else {
                    return;
                };
                l.usage.add(usage);
                l.rounds += 1;
                match reply {
                    Reply::Text(text) => self.tool_loop_answer(obj, text),
                    Reply::Calls { text, calls, raw_content } => {
                        if l.rounds >= l.max_rounds {
                            let max = l.max_rounds;
                            self.tool_loop_fail(
                                obj,
                                &format!(
                                    "the model was still calling tools after {max} rounds \
                                     (MaximumToolRounds) — no answer"
                                ),
                            );
                            return;
                        }
                        l.calls_made += calls.len() as u32;
                        l.turns.push(Turn::Assistant(Reply::Calls {
                            text,
                            calls: calls.clone(),
                            raw_content,
                        }));
                        l.pending = calls.into_iter().collect();
                        l.results.clear();
                        self.tool_loop_advance(obj);
                    }
                }
            }
        }
    }

    /// Answer the pending calls in order until one needs the program, or all
    /// are answered — then send the next round.
    fn tool_loop_advance(&mut self, obj: &str) {
        loop {
            let Some(l) = self.tool_loops.get_mut(obj) else {
                return;
            };
            let Some(call) = l.pending.pop_front() else {
                break;
            };
            let known: Vec<String> = l.tools.iter().map(|t| t.name.clone()).collect();
            let declared = self
                .agent_declared_tools
                .get(&obj.trim().to_ascii_uppercase())
                .map(|d| d.iter().any(|t| t.name.eq_ignore_ascii_case(&call.name)))
                .unwrap_or(false);

            let result = if !known.iter().any(|n| n.eq_ignore_ascii_case(&call.name)) {
                // R10 — told to the model, not raised to the program.
                Some(format!("error: no tool named '{}' is offered", call.name))
            } else if call.arguments.is_none() {
                Some(format!(
                    "error: the arguments were not a JSON object: {}",
                    call.raw_arguments
                ))
            } else if declared {
                None
            } else if self.kb_tool_is(&call.name) {
                // Spec 068 — a KnowledgeBase collection. Answered here, or on a
                // worker when the query needs the embedding server.
                match self.kb_tool_search(obj, &call) {
                    Some(text) => Some(text),
                    None => {
                        if let Some(l) = self.tool_loops.get_mut(obj) {
                            l.waiting = Waiting::KbSearch(call);
                        }
                        self.tool_loop_keep_pending(obj);
                        return;
                    }
                }
            } else {
                // A consultable indexed file (spec 065), read-only.
                let r = self
                    .mcp_tools
                    .call(&call.name, call.arguments.as_ref().unwrap_or(&serde_json::Value::Null));
                let text: Vec<String> = r
                    .content
                    .iter()
                    .map(|c| match c {
                        cobolt_mcp::types::Content::Text { text } => text.clone(),
                    })
                    .collect();
                let text = text.join("\n");
                Some(if r.is_error.unwrap_or(false) { format!("error: {text}") } else { text })
            };

            match result {
                Some(text) => {
                    if self.agent_is_verbose(obj) {
                        self.agent_log_block(obj, &format!("tool {} → result", call.name), &text);
                    }
                    if let Some(l) = self.tool_loops.get_mut(obj) {
                        l.results.push((call, text));
                    }
                }
                None => {
                    // The program answers this one, in its own handler.
                    self.obj_set(obj, "ToolCallId", call.id.clone());
                    self.obj_set(obj, "ToolName", call.name.clone());
                    self.obj_set(
                        obj,
                        "ToolArguments",
                        call.arguments
                            .as_ref()
                            .map(|a| a.to_string())
                            .unwrap_or_default(),
                    );
                    self.tool_results.remove(&call.id);
                    if let Some(l) = self.tool_loops.get_mut(obj) {
                        l.waiting = Waiting::Queued(call);
                    }
                    self.tool_loop_keep_pending(obj);
                    self.queue_control_event(obj, "onToolCall");
                    return;
                }
            }
        }
        // Every call answered: back to the model.
        if let Some(l) = self.tool_loops.get_mut(obj) {
            let results = std::mem::take(&mut l.results);
            l.turns.push(Turn::ToolResults(results));
        }
        self.tool_loop_send(obj);
    }

    /// Keep the `Ask` registered as in flight across the loop, so the timeout
    /// sweep bounds the WHOLE question — handler waits included (Q3) — and a
    /// `Cancel` finds something to cancel.
    pub(super) fn tool_loop_keep_pending(&mut self, obj: &str) {
        if let Some(l) = self.tool_loops.get(obj) {
            self.async_pending.insert(
                obj.to_string(),
                crate::async_op::PendingOp {
                    generation: l.generation,
                    started_at: l.started_at,
                    timeout_ms: l.timeout_ms,
                },
            );
        }
    }

    /// Send the next round on a worker thread.
    fn tool_loop_send(&mut self, obj: &str) {
        let Some(l) = self.tool_loops.get(obj).cloned() else {
            return;
        };
        let body = at::body_for_turns(&l.req, l.protocol, &l.turns, &l.tools, l.fenced);
        if self.agent_is_verbose(obj) {
            self.agent_log(format!(
                "[agent {obj}] ── request, round {} ─────────────────",
                l.rounds + 1
            ));
            self.agent_log_block(obj, "payload", &body);
        }
        self.tool_loop_keep_pending(obj);
        let cfg = crate::http_runtime::RequestConfig {
            timeout_ms: if l.timeout_ms > 0 { l.timeout_ms.saturating_add(5_000) } else { 0 },
            follow_redirects: true,
            verify_tls: true,
            headers: l.headers.clone(),
        };
        let tx = self.async_result_tx.clone();
        let http = self.http.clone();
        let ctrl_id = obj.to_string();
        let url = l.url.clone();
        let generation = l.generation;
        std::thread::spawn(move || {
            let (body, status) = http.send_configured("POST", &url, Some(&body), &cfg);
            let _ = tx.send(crate::async_op::AsyncOpResult {
                ctrl_id,
                generation,
                outcome: crate::async_op::AsyncOutcome::AgentReply { status, body },
            });
        });
    }

    /// At every `COBOL-WAIT-EVENT`: a program-answered call whose handler has
    /// run gets its result, and the loop moves on.
    pub(super) fn tool_loops_resume(&mut self) {
        let ready: Vec<String> = self
            .tool_loops
            .iter()
            .filter(|(_, l)| matches!(l.waiting, Waiting::Dispatched(_)))
            .map(|(obj, _)| obj.clone())
            .collect();
        for obj in ready {
            if !self.tool_loop_live(&obj) || !self.async_pending.contains_key(&obj) {
                // Cancelled or timed out while the handler ran.
                self.tool_loops.remove(&obj);
                continue;
            }
            let Some(l) = self.tool_loops.get_mut(&obj) else {
                continue;
            };
            let Waiting::Dispatched(call) = std::mem::replace(&mut l.waiting, Waiting::Nothing)
            else {
                continue;
            };
            // Q3 — a handler that set nothing sends an empty result.
            let result = self.tool_results.remove(&call.id).unwrap_or_default();
            if self.agent_is_verbose(&obj) {
                self.agent_log_block(&obj, &format!("tool {} → result (COBOL)", call.name), &result);
            }
            if let Some(l) = self.tool_loops.get_mut(&obj) {
                l.results.push((call, result));
            }
            self.tool_loop_advance(&obj);
        }
    }

    /// The event queue handed `onToolCall` to the program.
    /// `ctrl` is the form's spelling of the id; the loop is keyed by the
    /// caller's, so they are compared without regard to case.
    pub(super) fn tool_loop_dispatched(&mut self, ctrl: &str) {
        let hit = self
            .tool_loops
            .iter_mut()
            .find(|(obj, _)| obj.trim().eq_ignore_ascii_case(ctrl.trim()));
        if let Some((_, l)) = hit {
            if let Waiting::Queued(call) = &l.waiting {
                l.waiting = Waiting::Dispatched(call.clone());
            }
        }
    }

    fn tool_loop_publish_usage(&mut self, obj: &str) {
        if let Some(l) = self.tool_loops.get(obj) {
            let (i, o, c) = (l.usage.input, l.usage.output, l.calls_made);
            self.obj_set(obj, "LastInputTokens", i.to_string());
            self.obj_set(obj, "LastOutputTokens", o.to_string());
            self.obj_set(obj, "LastToolCallCount", c.to_string());
        }
    }

    fn tool_loop_answer(&mut self, obj: &str, text: String) {
        self.tool_loop_publish_usage(obj);
        self.tool_loops.remove(obj);
        self.async_pending.remove(obj);
        self.obj_set(obj, "Busy", "0".to_owned());
        self.agent_answered(obj, text);
    }

    fn tool_loop_fail(&mut self, obj: &str, message: &str) {
        self.tool_loop_publish_usage(obj);
        self.tool_loops.remove(obj);
        self.async_pending.remove(obj);
        self.obj_set(obj, "Busy", "0".to_owned());
        self.agent_failed(obj, message);
    }

    /// `AllowFile(fd-name [, cidx-path])` — let every agent search this
    /// indexed file. What the file MEANS comes from its `.cidx`; how its
    /// records are LAID OUT comes from this program's own `FD` — never the
    /// other way round (spec 065 R30). Answers whether the file is now offered.
    pub(super) fn agent_allow_file(&mut self, fd: &str, cidx: &str) -> bool {
        let key = fd.trim().to_ascii_uppercase();
        let Some(spec) = self.file_specs.get(&key).cloned() else {
            return false;
        };
        let path = self.resolve_assign_path(&spec.assign);
        let layout = &spec.layout;
        let primary = spec
            .record_key
            .as_deref()
            .and_then(|k| layout.key_spec_qualified(k, &spec.record_key_quals, false))
            .unwrap_or(crate::indexed::KeySpec {
                offset: 0,
                len: layout.len.max(1),
                duplicates: false,
            });
        let columns = layout
            .fields
            .iter()
            .filter(|f| !f.is_group)
            .map(|f| crate::mcp_tool::ColumnLayout {
                name: f.name.clone(),
                offset: f.offset,
                len: f.len,
            })
            .collect();
        let description = if cidx.trim().is_empty() {
            find_description(&key)
        } else {
            crate::mcp_tool::read_description(cidx.trim())
        };
        let Some(description) = description else {
            return false;
        };
        self.mcp_tools.deny(&description.name);
        self.mcp_tools.allow(
            description,
            crate::mcp_tool::FileAccess {
                path: path.into(),
                record_len: layout.len,
                primary,
                columns,
            },
        );
        true
    }
}

impl Interpreter {
    /// `RegisterFile(data-path, cidx-path [, name])` — let every agent search
    /// an indexed file named by its path, with no `FD` (spec 075). The outcome
    /// is written to `RegisterResult` (`MEMORY`, `DISK` or a refusal code),
    /// `RegisterMessage`, `RegisteredName`, `RegisterFileBytes` and
    /// `RegisterLimitBytes` — kept apart from `LastError`, which an `Ask` in
    /// flight may be about to write. Nothing is kept once the program ends
    /// (R6a), and no path reaches a model.
    pub(super) fn agent_register_file(&mut self, obj: &str, data: &str, cidx: &str, name: &str) -> bool {
        use crate::registered_file as reg;
        let limit = self.mcp_tools.memory_limit();
        let fixed;
        let system = reg::SystemMemory;
        let free: &dyn reg::FreeMemory = match self.free_memory_probe {
            Some(bytes) => {
                fixed = reg::FixedMemory(bytes);
                &fixed
            }
            None => &system,
        };
        let fetch = reg::Fetcher { smb: crate::smb_source::fetcher() };
        let outcome = reg::register(data, cidx, name, limit, free, &fetch);
        let shown = reg::Location::parse(data).map(|l| l.display()).unwrap_or_default();
        match outcome {
            Ok(r) => {
                let result = match r.fit {
                    reg::Fit::Memory => "MEMORY",
                    reg::Fit::InPlace => "DISK",
                };
                let message = match r.fit {
                    reg::Fit::Memory => format!("{} is held in memory ({} bytes)", r.description.name, r.size),
                    reg::Fit::InPlace => format!(
                        "{} is {} bytes, over what memory allows, and is read in place from disk",
                        r.description.name, r.size
                    ),
                };
                tracing::info!(target: "mcp", "registered {shown} as {}: {result}", r.description.name);
                self.obj_set(obj, "RegisterResult", result.into());
                self.obj_set(obj, "RegisterMessage", message);
                self.obj_set(obj, "RegisteredName", r.description.name.clone());
                self.obj_set(obj, "RegisterFileBytes", r.size.to_string());
                self.obj_set(obj, "RegisterLimitBytes", r.limit.to_string());
                self.mcp_tools.register(r.description, r.access, r.source);
                true
            }
            Err(refusal) => {
                tracing::warn!(target: "mcp", "{shown} not registered: {} {}", refusal.code, refusal.message);
                self.obj_set(obj, "RegisterResult", refusal.code.into());
                self.obj_set(obj, "RegisterMessage", refusal.message);
                self.obj_set(obj, "RegisteredName", String::new());
                self.obj_set(obj, "RegisterFileBytes", refusal.size.to_string());
                self.obj_set(obj, "RegisterLimitBytes", refusal.bound.to_string());
                false
            }
        }
    }
}

/// The delivered definition whose file is `fd` — searched in the `indexed/`
/// tree the application anchors on, which is where a build stages every
/// definition the project declares.
fn find_description(fd: &str) -> Option<crate::mcp_tool::FileDescription> {
    fn walk(dir: &std::path::Path, fd: &str, depth: usize) -> Option<crate::mcp_tool::FileDescription> {
        if depth > 4 {
            return None;
        }
        let entries = std::fs::read_dir(dir).ok()?;
        let mut subdirs = Vec::new();
        for e in entries.flatten() {
            let p = e.path();
            if p.is_dir() {
                subdirs.push(p);
            } else if p.extension().is_some_and(|x| x.eq_ignore_ascii_case("cidx")) {
                if let Some(d) = crate::mcp_tool::read_description_at(&p) {
                    if d.name.eq_ignore_ascii_case(fd) {
                        return Some(d);
                    }
                }
            }
        }
        subdirs.iter().find_map(|d| walk(d, fd, depth + 1))
    }
    walk(&cobolt_forms::assets::resolve("indexed"), fd, 0)
}
