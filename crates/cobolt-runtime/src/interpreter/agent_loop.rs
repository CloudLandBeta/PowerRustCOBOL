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
    /// `StartTimeoutSeconds` in ms — every round is a new request, and each
    /// one's answer must begin within it.
    start_timeout_ms: u64,
    started_at: std::time::Instant,
    generation: u64,
    pub(super) tools: Vec<ToolSpec>,
    turns: Vec<Turn>,
    rounds: u32,
    max_rounds: u32,
    usage: Usage,
    calls_made: u32,
    pending: std::collections::VecDeque<ToolCall>,
    results: Vec<(ToolCall, String)>,
    pub(super) waiting: Waiting,
    /// The answer so far, when the model was cut off by `MaximumTokens` and
    /// was asked to go on: every piece, in order, without a seam.
    pub(super) partial: String,
    /// How many times it was asked to go on, and how many it may be
    /// (`MaximumContinuations`, 4 unless set; 0 turns it off).
    continuations: u32,
    max_continuations: u32,
    /// A `StreamReply` Ask: the continuation's text is shown too, after
    /// what was shown already.
    pub(super) show_partials: bool,
}

/// What the model is told when its answer was cut off by its output limit.
/// It goes on from where it stopped; the pieces are joined as they come.
pub(super) const CONTINUE_PROMPT: &str = "Your previous answer was cut off by the length limit. \
Continue it exactly where it stopped - mid-sentence if that is where it stopped. Do not repeat \
anything already written, do not start over, and do not add any introduction or comment.";

/// A plain `Ask` (no tools) in flight: what a continuation needs to send its
/// next request, kept until the reply is read.
#[derive(Debug, Clone)]
pub(crate) struct PlainAsk {
    pub(crate) req: AskRequest,
    pub(crate) protocol: Protocol,
    pub(crate) url: String,
    pub(crate) headers: Vec<(String, String)>,
    pub(crate) timeout_ms: u64,
    pub(crate) start_timeout_ms: u64,
    pub(crate) generation: u64,
    pub(crate) show_partials: bool,
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
        tools.extend(self.kb_tool_specs(obj));
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
        start_timeout_ms: u64,
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
                start_timeout_ms,
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
                partial: String::new(),
                continuations: 0,
                max_continuations: self.agent_max_continuations(obj),
                show_partials: false,
            },
        );
    }

    /// `MaximumContinuations`: how many times an answer cut off by
    /// `MaximumTokens` is asked to go on. Unset reads as 4; `0` turns it off.
    pub(super) fn agent_max_continuations(&self, obj: &str) -> u32 {
        match self.obj_get(obj, "MaximumContinuations").trim() {
            "" => 4,
            v => v.parse::<u32>().unwrap_or(4),
        }
    }

    /// A plain `Ask` came back cut off by its output limit: it becomes a loop
    /// with no tools, whose next round asks the model to go on. False when it
    /// may not (no continuations allowed, or the Ask is no longer current).
    pub(super) fn agent_continue_plain(&mut self, obj: &str, text: &str, usage: Usage) -> bool {
        let Some(p) = self.plain_asks.get(obj).cloned() else {
            return false;
        };
        let live = self
            .async_generations
            .get(obj)
            .map(|g| g.load(std::sync::atomic::Ordering::Relaxed))
            .unwrap_or(0);
        if live != p.generation || self.agent_max_continuations(obj) == 0 || text.is_empty() {
            return false;
        }
        let prompt = p.req.prompt.clone();
        self.agent_begin_tool_loop(
            obj,
            p.req,
            p.protocol,
            false,
            p.url,
            p.headers,
            p.timeout_ms,
            p.start_timeout_ms,
            p.generation,
            Vec::new(),
        );
        let Some(l) = self.tool_loops.get_mut(obj) else {
            return false;
        };
        l.usage.add(usage);
        l.rounds = 1;
        l.show_partials = p.show_partials;
        l.partial.push_str(text);
        l.continuations = 1;
        l.turns = vec![
            Turn::User(prompt),
            Turn::Assistant(Reply::Text(text.to_owned())),
            Turn::User(CONTINUE_PROMPT.to_owned()),
        ];
        if self.agent_is_verbose(obj) {
            self.agent_log(format!(
                "[agent {obj}] the answer was cut off by MaximumTokens — asking the model to go on (1)"
            ));
        }
        self.obj_set(obj, "Busy", "1".to_owned());
        self.tool_loop_send(obj);
        true
    }

    /// Why the last answer stopped, read by the program: `StopReason` in the
    /// provider's words, `Truncated` when it was still cut off after every
    /// continuation, `ContinuationCount` how many it took.
    pub(super) fn agent_publish_stop(&mut self, obj: &str, reason: Option<&str>, continuations: u32) {
        let reason = reason.unwrap_or("").to_owned();
        let cut = crate::agent_runtime::is_cut_by_length(&reason);
        self.obj_set(obj, "StopReason", reason);
        self.obj_set(obj, "Truncated", if cut { "1" } else { "0" }.to_owned());
        self.obj_set(obj, "ContinuationCount", continuations.to_string());
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
                let reason = crate::agent_runtime::stop_reason_of(body);
                let cut = reason.as_deref().is_some_and(crate::agent_runtime::is_cut_by_length);
                match reply {
                    // Cut off by the output limit: asked to go on, the pieces
                    // joined (operator, 2026-09-30: "the agent must interact
                    // with the model to retrieve a complete response").
                    Reply::Text(text) if cut && l.continuations < l.max_continuations && !text.is_empty() => {
                        l.partial.push_str(&text);
                        l.continuations += 1;
                        let n = l.continuations;
                        l.turns.push(Turn::Assistant(Reply::Text(text)));
                        l.turns.push(Turn::User(CONTINUE_PROMPT.to_owned()));
                        if self.agent_is_verbose(obj) {
                            self.agent_log(format!(
                                "[agent {obj}] the answer was cut off by MaximumTokens — asking the model to go on ({n})"
                            ));
                        }
                        self.tool_loop_send(obj);
                    }
                    Reply::Text(text) => {
                        let whole = std::mem::take(&mut l.partial) + &text;
                        let n = l.continuations;
                        self.agent_publish_stop(obj, reason.as_deref(), n);
                        self.tool_loop_answer(obj, whole)
                    }
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

    /// Tell the program that the model is using one of the tools the runtime
    /// answers itself — a Knowledge Base search, a registered indexed file —
    /// which no program handler sees otherwise (`onToolCall` is for the tools
    /// the program declared and answers). A chat can say "searching the
    /// Knowledge Base…" while it happens (operator, 2026-09-27). Raised
    /// before the tool runs; the answer still goes straight to the model.
    fn announce_tool_use(&mut self, obj: &str, call: &ToolCall, kind: &str) {
        self.obj_set(obj, "ToolName", call.name.clone());
        self.obj_set(
            obj,
            "ToolArguments",
            call.arguments.as_ref().map(|a| a.to_string()).unwrap_or_default(),
        );
        self.obj_set(obj, "ToolKind", kind.to_string());
        self.queue_control_event(obj, "onToolUse");
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
            } else if self.kb_tool_is(obj, &call.name) {
                self.announce_tool_use(obj, &call, "KnowledgeBase");
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
                self.announce_tool_use(obj, &call, "IndexedFile");
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
            // A loop with no tools is a plain Ask being continued: like a
            // plain Ask, `TimeoutSeconds` is the silence between pieces, so
            // its clock restarts with each request.
            let started_at = if l.tools.is_empty() { std::time::Instant::now() } else { l.started_at };
            self.async_pending.insert(
                obj.to_string(),
                crate::async_op::PendingOp {
                    generation: l.generation,
                    started_at,
                    timeout_ms: l.timeout_ms,
                    awaiting_start: None,
                },
            );
        }
    }

    /// Send the next round on a worker thread.
    pub(super) fn tool_loop_send(&mut self, obj: &str) {
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
        // This round's answer must BEGIN within the start limit, counted from
        // now — never while a handler or a search had the loop waiting.
        if let Some(op) = self.async_pending.get_mut(obj) {
            op.awaiting_start =
                (l.start_timeout_ms > 0).then(|| (std::time::Instant::now(), l.start_timeout_ms));
        }
        // The socket's per-read limit, as for a plain `Ask`: the start limit
        // when there is one, so a model that sends nothing has its connection
        // closed; otherwise the backstop a little past `TimeoutSeconds`.
        let cfg = crate::http_runtime::RequestConfig {
            timeout_ms: if l.start_timeout_ms > 0 {
                l.start_timeout_ms.saturating_add(2_000)
            } else if l.timeout_ms > 0 {
                l.timeout_ms.saturating_add(5_000)
            } else {
                0
            },
            follow_redirects: true,
            verify_tls: true,
            headers: l.headers.clone(),
        };
        // Streamed like every agent request, so the loop can tell a model that
        // has begun its answer from one that never will. A tool round's text
        // is not shown on the way (the program answers a round, not the
        // reader); a `StreamReply` answer that is being continued is.
        self.agent_spawn_streamed(obj, l.url.clone(), body, l.protocol, cfg, l.generation, l.show_partials);
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
