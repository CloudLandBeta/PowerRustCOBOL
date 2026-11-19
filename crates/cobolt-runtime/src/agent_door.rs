// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! **The AgentObject's own rules**, for the interpreter and the desktop runner alike
//! (DESKTOP-WASM-001 T47c). They moved here, unchanged, from the interpreter.
//!
//! An `Ask` is prepared from the control's properties (spec 076's model-list entry first), narrated
//! when `Verbose` is on, and sent on a worker as a STREAMED request ([`Job`]), whose reports
//! ([`Report`]) come back to the program's thread. What a report does is decided here: the reply,
//! the error, the text shown so far (`StreamReply`), and the continuation of an answer cut off by
//! `MaximumTokens`.
//!
//! An `Ask` that offers tools (spec 072) becomes a short conversation: the question, the model's tool
//! calls, their results, and so on until the model answers in text. Every round runs on a worker as
//! a plain `Ask` does, under the SAME generation and pending entry, so `Cancel`, `TimeoutSeconds` and
//! a late reply behave as they always have, for the whole question and not per round.
//!
//! Tools come from two places:
//! - tools the **runtime** answers itself (the consultable indexed files, spec 065, and the
//!   Knowledge Base collections, spec 068), which the host offers ([`AgentHost::runtime_tools`]);
//! - tools the **program** declared with `AddTool`, answered by the program's own `onToolCall`
//!   handler. Those are presented ONE AT A TIME, because a call's details ride in the control's
//!   `ToolCallId` / `ToolName` / `ToolArguments` properties, which a second call would overwrite. When
//!   the handler has returned ([`Conversation::dispatched`], then [`resume`]), its `SetToolResult`, or
//!   nothing, which the model receives as an empty result, is taken and the loop moves on.
//!
//! The host keeps what is the host's: the control's properties and events, the program's output,
//! the in-flight entry the timeout sweep reads ([`PendingOp`]), the generation a cancelled call is
//! told apart by, the workers, and the tools it answers itself ([`AgentHost`]).

use std::collections::VecDeque;
use std::time::Instant;

use crate::agent_runtime::{self as ag, AskRequest, Protocol};
use crate::agent_tools::{self as at, Reply, ToolCall, ToolSpec, Turn, Usage};
use crate::async_op::PendingOp;
use crate::http_runtime::{HttpClient, RequestConfig};

/// A tool the program declared with `AddTool` / `AddToolParameter`.
#[derive(Debug, Clone, Default)]
pub struct DeclaredTool {
    pub name: String,
    pub description: String,
    pub params: Vec<(String, String)>,
}

impl DeclaredTool {
    fn spec(&self) -> ToolSpec {
        let mut properties = serde_json::Map::new();
        for (name, description) in &self.params {
            properties.insert(name.clone(), serde_json::json!({"type": "string", "description": description}));
        }
        ToolSpec {
            name: self.name.clone(),
            description: self.description.clone(),
            parameters: serde_json::json!({"type": "object", "properties": properties}),
        }
    }
}

/// `AddTool(name, description)`: the tool, replacing one of the same name. Answers `"1"`.
pub fn add_tool(list: &mut Vec<DeclaredTool>, name: String, description: String) -> String {
    list.retain(|t| !t.name.eq_ignore_ascii_case(&name));
    list.push(DeclaredTool { name, description, params: Vec::new() });
    "1".into()
}

/// `AddToolParameter(tool, name, description)`: whether the tool was declared and now has it.
pub fn add_tool_parameter(list: &mut [DeclaredTool], tool: &str, name: String, description: String) -> bool {
    match list.iter_mut().find(|t| t.name.eq_ignore_ascii_case(tool)) {
        Some(t) => {
            t.params.retain(|(n, _)| !n.eq_ignore_ascii_case(&name));
            t.params.push((name, description));
            true
        }
        None => false,
    }
}

/// `RemoveTool(name)`: whether a tool of that name was declared.
pub fn remove_tool(list: &mut Vec<DeclaredTool>, name: &str) -> bool {
    let before = list.len();
    list.retain(|t| !t.name.eq_ignore_ascii_case(name));
    before != list.len()
}

/// Where a program-answered call stands.
#[derive(Debug, Clone, PartialEq)]
pub enum Waiting {
    Nothing,
    /// `onToolCall` is queued; the handler has not run yet.
    Queued(ToolCall),
    /// The event was handed to the program; when its handler has returned the loop moves on.
    Dispatched(ToolCall),
    /// Spec 068 — a KnowledgeBase search is running on a worker (the query needed the embedding
    /// server); its result moves the loop on ([`tool_worker_delivered`]).
    KbSearch(ToolCall),
}

/// One `Ask` in progress that offers tools.
#[derive(Debug, Clone)]
pub struct ToolLoop {
    req: AskRequest,
    protocol: Protocol,
    fenced: bool,
    url: String,
    headers: Vec<(String, String)>,
    timeout_ms: u64,
    /// `StartTimeoutSeconds` in ms — every round is a new request, and each one's answer must
    /// begin within it.
    start_timeout_ms: u64,
    started_at: Instant,
    generation: u64,
    tools: Vec<ToolSpec>,
    turns: Vec<Turn>,
    rounds: u32,
    max_rounds: u32,
    usage: Usage,
    calls_made: u32,
    pending: VecDeque<ToolCall>,
    results: Vec<(ToolCall, String)>,
    waiting: Waiting,
    /// The answer so far, when the model was cut off by `MaximumTokens` and was asked to go on:
    /// every piece, in order, without a seam.
    partial: String,
    /// How many times it was asked to go on, and how many it may be (`MaximumContinuations`, 4
    /// unless set; 0 turns it off).
    continuations: u32,
    max_continuations: u32,
    /// A `StreamReply` Ask: the continuation's text is shown too, after what was shown already.
    show_partials: bool,
}

/// What the model is told when its answer was cut off by its output limit. It goes on from where it
/// stopped; the pieces are joined as they come.
pub const CONTINUE_PROMPT: &str = "Your previous answer was cut off by the length limit. \
Continue it exactly where it stopped - mid-sentence if that is where it stopped. Do not repeat \
anything already written, do not start over, and do not add any introduction or comment.";

/// A plain `Ask` (no tools) in flight: what a continuation needs to send its next request, kept
/// until the reply is read.
#[derive(Debug, Clone)]
pub struct PlainAsk {
    req: AskRequest,
    protocol: Protocol,
    url: String,
    headers: Vec<(String, String)>,
    timeout_ms: u64,
    start_timeout_ms: u64,
    generation: u64,
    show_partials: bool,
}

/// An agent's `Ask` in progress: a plain one, or one that offers tools (or a plain one being
/// continued, which is a loop with no tools).
#[derive(Debug, Clone)]
pub enum Conversation {
    Plain(PlainAsk),
    Loop(ToolLoop),
}

impl Conversation {
    /// Is it a tool loop (a plain `Ask` being continued is one)?
    pub fn is_loop(&self) -> bool {
        matches!(self, Conversation::Loop(_))
    }

    /// Is a program-answered call's handler done with, so the loop can take its result ([`resume`])?
    pub fn awaits_resume(&self) -> bool {
        matches!(self, Conversation::Loop(l) if matches!(l.waiting, Waiting::Dispatched(_)))
    }

    /// The queued `onToolCall` was handed to the program.
    pub fn dispatched(&mut self) {
        if let Conversation::Loop(l) = self {
            if let Waiting::Queued(call) = &l.waiting {
                l.waiting = Waiting::Dispatched(call.clone());
            }
        }
    }
}

fn lp(conv: &mut Option<Conversation>) -> Option<&mut ToolLoop> {
    match conv {
        Some(Conversation::Loop(l)) => Some(l),
        _ => None,
    }
}

/// Which model-list entry an agent now uses (spec 076 R16), so a change to it raises
/// `onModelChanged` once.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Seen {
    /// The agent uses no entry any more.
    Forget,
    /// The agent uses this entry (its name in capitals) at this generation.
    Watch(String, u64),
}

/// What the host gives the AgentObject's rules: one agent's properties and events, the program's
/// output, its in-flight entry and generation, its workers, and its tools.
pub trait AgentHost {
    /// The agent's name, as the program spelt it.
    fn name(&self) -> &str;
    /// One of its properties, as the program would read it.
    fn prop(&self, name: &str) -> String;
    /// Write one of its properties.
    fn set(&mut self, name: &str, value: String);
    /// Raise one of its events.
    fn event(&mut self, name: &str);
    /// One line of the program's output (the `Verbose` narration).
    fn log(&mut self, line: String);
    /// `ResponseDataItem`: write `text` into the program's data item `item`, when it declares one.
    fn set_item(&mut self, item: &str, text: &str);
    /// The tools the runtime answers itself, offered to every agent: the consultable indexed files,
    /// then this agent's Knowledge Base collections.
    fn runtime_tools(&self) -> Vec<ToolSpec>;
    /// The tools the program declared on this agent.
    fn declared_tools(&self) -> Vec<DeclaredTool>;
    /// What kind of runtime tool `name` is, as `onToolUse`'s `ToolKind` names it.
    fn tool_kind(&self, name: &str) -> &'static str;
    /// Answer a runtime tool's call; `None` when a worker answers it later, reporting with
    /// `generation` ([`tool_worker_delivered`]).
    fn run_tool(&mut self, call: &ToolCall, generation: u64) -> Option<String>;
    /// The program's `SetToolResult` for this call, taken.
    fn take_tool_result(&mut self, call_id: &str) -> Option<String>;
    /// The generation the agent is at: a call from another is stale.
    fn live_generation(&self) -> u64;
    /// Move the agent to its next generation, and answer it.
    fn next_generation(&mut self) -> u64;
    /// The agent's in-flight entry, which the timeout sweep reads.
    fn pending(&self) -> Option<PendingOp>;
    fn set_pending(&mut self, op: Option<PendingOp>);
    /// The model-list entry the agent now uses.
    fn watch(&mut self, seen: Seen);
    /// Send one request on a worker.
    fn send(&mut self, job: Job);
}

fn is_verbose(h: &dyn AgentHost) -> bool {
    h.prop("Verbose").eq_ignore_ascii_case("true")
}

/// Print a block of text under one label, one line per line of the text: verbatim, no clipping, no
/// flattening, no summary (operator, 2026-09-07: "the url, the payload, the headers, all of it, and
/// then response or the error, verbatim not summarized").
fn log_block(h: &mut dyn AgentHost, label: &str, text: &str) {
    let obj = h.name().to_owned();
    for line in crate::narration::block("agent", &obj, label, text) {
        h.log(line);
    }
}

fn log(h: &mut dyn AgentHost, line: String) {
    h.log(line);
}

/// `StartTimeoutSeconds`, in milliseconds: the longest an agent request may wait for the first piece
/// of its answer. Unset reads as the default, 60 s; `0` turns the limit off.
pub fn start_timeout_ms(h: &dyn AgentHost) -> u64 {
    match h.prop("StartTimeoutSeconds").trim() {
        "" => 60_000,
        v => v.parse::<u64>().unwrap_or(60).saturating_mul(1000),
    }
}

/// `MaximumContinuations`: how many times an answer cut off by `MaximumTokens` is asked to go on.
/// Unset reads as 4; `0` turns it off.
fn max_continuations(h: &dyn AgentHost) -> u32 {
    match h.prop("MaximumContinuations").trim() {
        "" => 4,
        v => v.parse::<u32>().unwrap_or(4),
    }
}

/// The tools this agent offers: every runtime tool (065, for every agent — 072 Q2) and the ones the
/// program declared on it.
fn offered_tools(h: &dyn AgentHost) -> Vec<ToolSpec> {
    // `ToolProtocol = None`: this agent is offered no tool at all, whatever the program allowed —
    // for a model that cannot call tools (spec 071, 063 R55/R66). Files and Knowledge Bases are
    // allowed program-wide, so this is the one per-agent switch.
    if h.prop("ToolProtocol").trim().eq_ignore_ascii_case("none") {
        return Vec::new();
    }
    let mut tools = h.runtime_tools();
    tools.extend(h.declared_tools().iter().map(DeclaredTool::spec));
    tools
}

/// `Ask(prompt)`: the request is prepared and sent on a worker; the control is busy until its reply,
/// its error or its time-out.
pub fn ask(h: &mut dyn AgentHost, conv: &mut Option<Conversation>, prompt: &str) {
    let obj = h.name().to_owned();
    let mut req = AskRequest {
        api: h.prop("AgentAPI"),
        url: h.prop("AgentURL"),
        endpoint: h.prop("AgentEndpoint"),
        model: h.prop("AgentModel"),
        api_key: h.prop("AgentAPIKey"),
        system_prompt: h.prop("SystemPrompt"),
        prompt: prompt.to_owned(),
        temperature: h.prop("Temperature").trim().parse().unwrap_or(70),
        max_tokens: h.prop("MaximumTokens").trim().parse().unwrap_or(1024),
    };
    // Spec 076 — a model-list entry, named at run time, wins over the design-time `Configuration`
    // and the agent's own settings (R14). It brings the API, the endpoint and the key; its model when
    // it names one; temperature, token limit and timeout stay the agent's (R17).
    let entry_name = h.prop("ModelEntry").trim().to_string();
    let key_from_store = !entry_name.is_empty();
    if key_from_store {
        let Some((entry, generation)) = crate::model_list::get(&entry_name) else {
            h.watch(Seen::Forget);
            failed(
                h,
                &format!(
                    "model entry '{entry_name}' does not exist — the program has not \
                     handed it over with COBOL-MODEL-SET"
                ),
            );
            return;
        };
        let key = crate::key_store::key_store().get(&entry_name);
        if entry.needs_key() && key.as_deref().map(str::trim).unwrap_or("").is_empty() {
            failed(
                h,
                &format!(
                    "model entry '{entry_name}' has no key stored, and its API ({}) needs \
                     one — store it with COBOL-KEY-SET",
                    entry.api
                ),
            );
            return;
        }
        h.watch(Seen::Watch(entry_name.to_ascii_uppercase(), generation));
        req.api = entry.api;
        req.url = entry.url;
        req.endpoint = String::new();
        if !entry.model.trim().is_empty() {
            req.model = entry.model;
        }
        req.api_key = key.unwrap_or_default();
    }
    // A provider from the catalogue is addressed the way the IDE addresses it (its API root plus
    // `/messages` or `/chat/completions`); anything else — `LMStudio`, `Custom`, an explicit
    // `AgentEndpoint` — keeps the older reading of the URL.
    let (protocol, url) = match crate::providers::find(&req.api) {
        Some(p) if req.endpoint.trim().is_empty() && !req.url.trim().is_empty() => crate::providers::chat_target(p.id, &req.url),
        _ => {
            let protocol = ag::protocol_for(&req.api, &req.url);
            (protocol, ag::endpoint_for(&req, protocol))
        }
    };
    if url.trim().is_empty() {
        failed(h, "AgentURL is not set — there is nowhere to send the prompt");
        return;
    }
    // Spec 072 — offering no tools keeps today's request byte-for-byte (R5); offering any makes this
    // the first round of a tool loop.
    let tools = offered_tools(h);
    let fenced = h.prop("ToolProtocol").trim().eq_ignore_ascii_case("fenced");
    let body = if tools.is_empty() {
        ag::body_for(&req, protocol)
    } else {
        at::body_for_turns(&req, protocol, &[Turn::User(req.prompt.clone())], &tools, fenced)
    };
    let cfg = RequestConfig {
        timeout_ms: h.prop("TimeoutSeconds").trim().parse::<u64>().unwrap_or(30).saturating_mul(1000),
        follow_redirects: true,
        verify_tls: true,
        headers: ag::headers_for(&req, protocol),
    };
    let verbose = is_verbose(h);
    if verbose {
        // Everything that goes on the wire, exactly as it goes. The header list includes the
        // Authorization line WITH the key: a key that is wrong by a character is invisible once
        // masked, and finding that is most of what this switch is for. It is opt-in and off by
        // default; the warning says what the output now contains.
        log(h, format!("[agent {obj}] ── request ──────────────────────"));
        log(h, format!("[agent {obj}] POST {url}"));
        log(h, format!("[agent {obj}] protocol: {protocol:?}"));
        log(h, format!("[agent {obj}] timeout: {} ms", cfg.timeout_ms));
        for (name, value) in &cfg.headers {
            // A key from the key store is never shown, not even here (spec 076 R7).
            let secret = name.eq_ignore_ascii_case("authorization") || name.eq_ignore_ascii_case("x-api-key");
            if key_from_store && secret {
                log(h, format!("[agent {obj}] header: {name}: ****"));
            } else {
                log(h, format!("[agent {obj}] header: {name}: {value}"));
            }
        }
        if !key_from_store
            && cfg
                .headers
                .iter()
                .any(|(n, _)| n.eq_ignore_ascii_case("authorization") || n.eq_ignore_ascii_case("x-api-key"))
        {
            log(h, format!("[agent {obj}] (this log contains the API key — do not paste it into a bug report)"));
        }
        log_block(h, "payload", &body);
    }
    // One question at a time per control (the spec-032 rule every async control follows): a second
    // Ask while one is in flight is ignored rather than racing the first one's answer onto the same
    // properties.
    if h.pending().is_some() {
        if verbose {
            log(h, format!("[agent {obj}] an Ask is already in flight — this one is ignored"));
        }
        return;
    }
    let generation = h.next_generation();
    let timeout_ms = cfg.timeout_ms;
    let offers_tools = !tools.is_empty();
    *conv = None;
    if offers_tools {
        let start = start_timeout_ms(h);
        *conv = Some(Conversation::Loop(begin_loop(
            h,
            req.clone(),
            protocol,
            fenced,
            url.clone(),
            cfg.headers.clone(),
            timeout_ms,
            start,
            generation,
            tools,
        )));
    }
    h.set("Busy", "1".to_owned());
    let start_ms = start_timeout_ms(h);
    h.set_pending(Some(PendingOp {
        generation,
        started_at: Instant::now(),
        timeout_ms,
        awaiting_start: (start_ms > 0).then(|| (Instant::now(), start_ms)),
    }));
    if offers_tools {
        // The loop sends its own rounds, each streamed the same way.
        loop_send(h, conv);
        return;
    }
    // The socket's per-read limit. With a start limit it is that limit (and 2 s), so a model that
    // sends nothing has its connection CLOSED just after the sweep reports it; the same bound then
    // applies to any silence mid-answer. Without one it is the older backstop, a little longer than
    // the sweep that owns `onTimeout`.
    let mut cfg = cfg;
    cfg.timeout_ms = if start_ms > 0 {
        start_ms.saturating_add(2_000)
    } else if timeout_ms > 0 {
        timeout_ms.saturating_add(5_000)
    } else {
        0
    };
    // `StreamReply` — the reply is also SHOWN as the provider writes it (`PartialReply` +
    // `onPartialReply`). A tool-offering Ask is never shown on the way: its rounds are answered by
    // the program. `Stream`, the retired switch seeded on older forms, is NOT this — it stays a
    // no-op. Every call is streamed on the wire regardless (see [`Job::run`]); `TimeoutSeconds`
    // counts the silence between pieces, so a long answer that keeps coming is never cut off.
    let show_partials = matches!(h.prop("StreamReply").trim().to_ascii_lowercase().as_str(), "1" | "true" | "yes");
    h.set("PartialReply", String::new());
    h.set("ReplyPiece", String::new());
    *conv = Some(Conversation::Plain(PlainAsk {
        req: req.clone(),
        protocol,
        url: url.clone(),
        headers: cfg.headers.clone(),
        timeout_ms,
        start_timeout_ms: start_ms,
        generation,
        show_partials,
    }));
    h.send(Job::new(url, &body, protocol, cfg, generation, show_partials));
}

/// A new tool-offering `Ask`, with the generation and timeout it registered.
#[allow(clippy::too_many_arguments)]
fn begin_loop(
    h: &dyn AgentHost,
    req: AskRequest,
    protocol: Protocol,
    fenced: bool,
    url: String,
    headers: Vec<(String, String)>,
    timeout_ms: u64,
    start_timeout_ms: u64,
    generation: u64,
    tools: Vec<ToolSpec>,
) -> ToolLoop {
    let max_rounds = h.prop("MaximumToolRounds").trim().parse::<u32>().ok().filter(|n| *n > 0).unwrap_or(8);
    let prompt = req.prompt.clone();
    ToolLoop {
        req,
        protocol,
        fenced,
        url,
        headers,
        timeout_ms,
        start_timeout_ms,
        started_at: Instant::now(),
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
        max_continuations: max_continuations(h),
        show_partials: false,
    }
}

/// A plain `Ask` came back cut off by its output limit: it becomes a loop with no tools, whose next
/// round asks the model to go on. False when it may not (no continuations allowed, or the Ask is no
/// longer current).
fn continue_plain(h: &mut dyn AgentHost, conv: &mut Option<Conversation>, text: &str, usage: Usage) -> bool {
    let Some(Conversation::Plain(p)) = conv.clone() else {
        return false;
    };
    if h.live_generation() != p.generation || max_continuations(h) == 0 || text.is_empty() {
        return false;
    }
    let obj = h.name().to_owned();
    let prompt = p.req.prompt.clone();
    *conv = Some(Conversation::Loop(begin_loop(
        h,
        p.req,
        p.protocol,
        false,
        p.url,
        p.headers,
        p.timeout_ms,
        p.start_timeout_ms,
        p.generation,
        Vec::new(),
    )));
    let Some(l) = lp(conv) else {
        return false;
    };
    l.usage.add(usage);
    l.rounds = 1;
    l.show_partials = p.show_partials;
    l.partial.push_str(text);
    l.continuations = 1;
    l.turns = vec![Turn::User(prompt), Turn::Assistant(Reply::Text(text.to_owned())), Turn::User(CONTINUE_PROMPT.to_owned())];
    if is_verbose(h) {
        log(h, format!("[agent {obj}] the answer was cut off by MaximumTokens — asking the model to go on (1)"));
    }
    h.set("Busy", "1".to_owned());
    loop_send(h, conv);
    true
}

/// Why the last answer stopped, read by the program: `StopReason` in the provider's words,
/// `Truncated` when it was still cut off after every continuation, `ContinuationCount` how many it
/// took.
fn publish_stop(h: &mut dyn AgentHost, reason: Option<&str>, continuations: u32) {
    let reason = reason.unwrap_or("").to_owned();
    let cut = ag::is_cut_by_length(&reason);
    h.set("StopReason", reason);
    h.set("Truncated", if cut { "1" } else { "0" }.to_owned());
    h.set("ContinuationCount", continuations.to_string());
}

/// Is this loop still the one its control is waiting on? A `Cancel`, a timeout or a newer `Ask` bumps
/// the generation, and a loop from before that is dropped without a word.
fn loop_live(h: &dyn AgentHost, conv: &Option<Conversation>) -> bool {
    match conv {
        Some(Conversation::Loop(l)) => h.live_generation() == l.generation,
        _ => false,
    }
}

/// A streamed Ask's reply so far: `PartialReply` holds it, `ReplyPiece` what is new since the last
/// `onPartialReply` (what a Viewer's `AppendToMessage` wants), and `onPartialReply` fires. The Ask
/// stays in flight — `Busy` stays 1 — and its timeout restarts, because text is still arriving.
pub fn partial(h: &mut dyn AgentHost, conv: &Option<Conversation>, text: String) {
    if let Some(mut op) = h.pending() {
        op.started_at = Instant::now();
        op.awaiting_start = None;
        h.set_pending(Some(op));
    }
    // A continued answer streams from empty again: what was shown before it comes first, so
    // `PartialReply` keeps growing and `ReplyPiece` stays only what is new.
    let text = match conv {
        Some(Conversation::Loop(l)) if l.show_partials => format!("{}{text}", l.partial),
        _ => text,
    };
    let before = h.prop("PartialReply");
    let piece = text.strip_prefix(before.as_str()).unwrap_or(&text).to_owned();
    h.set("ReplyPiece", piece);
    h.set("PartialReply", text);
    h.event("onPartialReply");
}

/// The model has begun answering — and, repeated, is still answering. The wait for the answer to
/// START is over; a plain `Ask`'s silence limit restarts. A tool loop keeps its clock:
/// `TimeoutSeconds` bounds its whole question, handler waits included (072 Q3).
pub fn alive(h: &mut dyn AgentHost, conv: &Option<Conversation>) {
    // A loop with no tools is a plain Ask being continued, timed like one.
    let tool_loop = matches!(conv, Some(Conversation::Loop(l)) if !l.tools.is_empty());
    if let Some(mut op) = h.pending() {
        op.awaiting_start = None;
        if !tool_loop {
            op.started_at = Instant::now();
        }
        h.set_pending(Some(op));
    }
}

/// Read one delivered reply onto its control and raise the event it earned, with the raw status
/// and body — so the narration and the parse are the same ones the call has always used.
pub fn delivered(h: &mut dyn AgentHost, conv: &mut Option<Conversation>, status: u16, body: &str) {
    // Spec 072 — a round of a tool-offering Ask.
    if matches!(conv, Some(Conversation::Loop(_))) {
        loop_delivered(h, conv, status, body);
        return;
    }
    let obj = h.name().to_owned();
    h.set("Busy", "0".to_owned());
    if is_verbose(h) {
        log(h, format!("[agent {obj}] ── response ─────────────────────"));
        log(h, format!("[agent {obj}] status: {status}"));
        // The raw body, before anything is read out of it. When the parse disagrees with what the
        // provider sent, this is the only place the disagreement is visible.
        log_block(h, "body", body);
    }
    // Status 0 is the transport convention: no HTTP response happened at all and the body IS the
    // failure. Handing that to `parse_reply` would dress a dead socket up as a malformed document.
    if status == 0 {
        failed(h, body.trim());
        return;
    }
    // R17 — the usage the provider reported, even with no tools.
    let usage = at::usage_from_body(body);
    h.set("LastInputTokens", usage.input.to_string());
    h.set("LastOutputTokens", usage.output.to_string());
    h.set("LastToolCallCount", "0".to_owned());
    match ag::parse_reply(status, body) {
        Ok(text) => {
            // Cut off by `MaximumTokens`: the model is asked to go on, and `onResponse` waits for
            // the whole answer.
            let reason = ag::stop_reason_of(body);
            if reason.as_deref().is_some_and(ag::is_cut_by_length) && continue_plain(h, conv, &text, usage) {
                return;
            }
            *conv = None;
            publish_stop(h, reason.as_deref(), 0);
            answered(h, text)
        }
        Err(message) => {
            *conv = None;
            failed(h, &message)
        }
    }
}

/// A reply arrived: `LastReply`, `ResponseDataItem`, `Result`, and `onResponse`. Shared by a plain
/// `Ask` and the last round of a tool loop.
fn answered(h: &mut dyn AgentHost, text: String) {
    let obj = h.name().to_owned();
    if is_verbose(h) {
        log_block(h, "reply", &text);
    }
    h.set("LastReply", text.clone());
    // `ResponseDataItem` names a WORKING-STORAGE item that receives the reply, written here, when
    // the reply exists, and only into an item the program actually declares.
    let target = h.prop("ResponseDataItem");
    let target = target.trim();
    if !target.is_empty() {
        h.set_item(target, &text);
    }
    h.set("Result", text);
    h.set("LastError", String::new());
    if is_verbose(h) {
        log(h, format!("[agent {obj}] LastReply set — onResponse fires next"));
    }
    h.event("onResponse");
}

/// Record a failed `Ask` and raise `onError`.
///
/// `LastReply` is cleared rather than left holding the previous answer: a stale reply after a failed
/// call is how a handler reports success it did not have.
fn failed(h: &mut dyn AgentHost, message: &str) {
    let obj = h.name().to_owned();
    h.set("LastReply", String::new());
    h.set("LastError", message.to_owned());
    if is_verbose(h) {
        log_block(h, "error", message);
        log(h, format!("[agent {obj}] LastError set, LastReply cleared — onError will fire"));
    }
    h.event("onError");
}

/// One round's reply of a tool loop arrived.
fn loop_delivered(h: &mut dyn AgentHost, conv: &mut Option<Conversation>, status: u16, body: &str) {
    let obj = h.name().to_owned();
    if is_verbose(h) {
        log(h, format!("[agent {obj}] ── response (tool loop) ─────────"));
        log(h, format!("[agent {obj}] status: {status}"));
        log_block(h, "body", body);
    }
    if status == 0 {
        loop_fail(h, conv, body.trim());
        return;
    }
    let fenced = lp(conv).map(|l| l.fenced).unwrap_or(false);
    match at::parse_turn(status, body, fenced) {
        Err(message) => loop_fail(h, conv, &message),
        Ok((reply, usage)) => {
            let Some(l) = lp(conv) else {
                return;
            };
            l.usage.add(usage);
            l.rounds += 1;
            let reason = ag::stop_reason_of(body);
            let cut = reason.as_deref().is_some_and(ag::is_cut_by_length);
            match reply {
                // Cut off by the output limit: asked to go on, the pieces joined (operator,
                // 2026-09-30: "the agent must interact with the model to retrieve a complete
                // response").
                Reply::Text(text) if cut && l.continuations < l.max_continuations && !text.is_empty() => {
                    l.partial.push_str(&text);
                    l.continuations += 1;
                    let n = l.continuations;
                    l.turns.push(Turn::Assistant(Reply::Text(text)));
                    l.turns.push(Turn::User(CONTINUE_PROMPT.to_owned()));
                    if is_verbose(h) {
                        log(h, format!("[agent {obj}] the answer was cut off by MaximumTokens — asking the model to go on ({n})"));
                    }
                    loop_send(h, conv);
                }
                Reply::Text(text) => {
                    let whole = std::mem::take(&mut l.partial) + &text;
                    let n = l.continuations;
                    publish_stop(h, reason.as_deref(), n);
                    loop_answer(h, conv, whole)
                }
                Reply::Calls { text, calls, raw_content } => {
                    if l.rounds >= l.max_rounds {
                        let max = l.max_rounds;
                        loop_fail(
                            h,
                            conv,
                            &format!(
                                "the model was still calling tools after {max} rounds \
                                 (MaximumToolRounds) — no answer"
                            ),
                        );
                        return;
                    }
                    l.calls_made += calls.len() as u32;
                    l.turns.push(Turn::Assistant(Reply::Calls { text, calls: calls.clone(), raw_content }));
                    l.pending = calls.into_iter().collect();
                    l.results.clear();
                    loop_advance(h, conv);
                }
            }
        }
    }
}

/// Tell the program that the model is using one of the tools the runtime answers itself — a
/// Knowledge Base search, a registered indexed file — which no program handler sees otherwise
/// (`onToolCall` is for the tools the program declared and answers). A chat can say "searching the
/// Knowledge Base…" while it happens (operator, 2026-09-27). Raised before the tool runs; the answer
/// still goes straight to the model.
fn announce_tool_use(h: &mut dyn AgentHost, call: &ToolCall, kind: &str) {
    h.set("ToolName", call.name.clone());
    h.set("ToolArguments", call.arguments.as_ref().map(|a| a.to_string()).unwrap_or_default());
    h.set("ToolKind", kind.to_string());
    h.event("onToolUse");
}

/// Answer the pending calls in order until one needs the program, or all are answered — then send
/// the next round.
fn loop_advance(h: &mut dyn AgentHost, conv: &mut Option<Conversation>) {
    loop {
        let Some(l) = lp(conv) else {
            return;
        };
        let Some(call) = l.pending.pop_front() else {
            break;
        };
        let generation = l.generation;
        let known: Vec<String> = l.tools.iter().map(|t| t.name.clone()).collect();
        let declared = h.declared_tools().iter().any(|t| t.name.eq_ignore_ascii_case(&call.name));

        let result = if !known.iter().any(|n| n.eq_ignore_ascii_case(&call.name)) {
            // R10 — told to the model, not raised to the program.
            Some(format!("error: no tool named '{}' is offered", call.name))
        } else if call.arguments.is_none() {
            Some(format!("error: the arguments were not a JSON object: {}", call.raw_arguments))
        } else if declared {
            None
        } else {
            // A tool the runtime answers: a Knowledge Base collection (spec 068), answered here or
            // on a worker when the query needs the embedding server, or a consultable indexed file
            // (spec 065), read-only.
            let kind = h.tool_kind(&call.name);
            announce_tool_use(h, &call, kind);
            match h.run_tool(&call, generation) {
                Some(text) => Some(text),
                None => {
                    if let Some(l) = lp(conv) {
                        l.waiting = Waiting::KbSearch(call);
                    }
                    keep_pending(h, conv);
                    return;
                }
            }
        };

        match result {
            Some(text) => {
                if is_verbose(h) {
                    log_block(h, &format!("tool {} → result", call.name), &text);
                }
                if let Some(l) = lp(conv) {
                    l.results.push((call, text));
                }
            }
            None => {
                // The program answers this one, in its own handler.
                h.set("ToolCallId", call.id.clone());
                h.set("ToolName", call.name.clone());
                h.set("ToolArguments", call.arguments.as_ref().map(|a| a.to_string()).unwrap_or_default());
                let _ = h.take_tool_result(&call.id);
                if let Some(l) = lp(conv) {
                    l.waiting = Waiting::Queued(call);
                }
                keep_pending(h, conv);
                h.event("onToolCall");
                return;
            }
        }
    }
    // Every call answered: back to the model.
    if let Some(l) = lp(conv) {
        let results = std::mem::take(&mut l.results);
        l.turns.push(Turn::ToolResults(results));
    }
    loop_send(h, conv);
}

/// Keep the `Ask` registered as in flight across the loop, so the timeout sweep bounds the WHOLE
/// question — handler waits included (Q3) — and a `Cancel` finds something to cancel.
fn keep_pending(h: &mut dyn AgentHost, conv: &Option<Conversation>) {
    if let Some(Conversation::Loop(l)) = conv {
        // A loop with no tools is a plain Ask being continued: like a plain Ask, `TimeoutSeconds` is
        // the silence between pieces, so its clock restarts with each request.
        let started_at = if l.tools.is_empty() { Instant::now() } else { l.started_at };
        h.set_pending(Some(PendingOp { generation: l.generation, started_at, timeout_ms: l.timeout_ms, awaiting_start: None }));
    }
}

/// Send the next round on a worker.
fn loop_send(h: &mut dyn AgentHost, conv: &Option<Conversation>) {
    let Some(Conversation::Loop(l)) = conv.clone() else {
        return;
    };
    let obj = h.name().to_owned();
    let body = at::body_for_turns(&l.req, l.protocol, &l.turns, &l.tools, l.fenced);
    if is_verbose(h) {
        log(h, format!("[agent {obj}] ── request, round {} ─────────────────", l.rounds + 1));
        log_block(h, "payload", &body);
    }
    keep_pending(h, conv);
    // This round's answer must BEGIN within the start limit, counted from now — never while a
    // handler or a search had the loop waiting.
    if let Some(mut op) = h.pending() {
        op.awaiting_start = (l.start_timeout_ms > 0).then(|| (Instant::now(), l.start_timeout_ms));
        h.set_pending(Some(op));
    }
    // The socket's per-read limit, as for a plain `Ask`: the start limit when there is one, so a
    // model that sends nothing has its connection closed; otherwise the backstop a little past
    // `TimeoutSeconds`.
    let cfg = RequestConfig {
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
    // Streamed like every agent request, so the loop can tell a model that has begun its answer from
    // one that never will. A tool round's text is not shown on the way (the program answers a round,
    // not the reader); a `StreamReply` answer that is being continued is.
    h.send(Job::new(l.url.clone(), &body, l.protocol, cfg, l.generation, l.show_partials));
}

/// The handler of a program-answered call has returned: its result is taken, and the loop moves on.
/// A loop that was cancelled or timed out while the handler ran is dropped.
pub fn resume(h: &mut dyn AgentHost, conv: &mut Option<Conversation>) {
    if !conv.as_ref().is_some_and(Conversation::awaits_resume) {
        return;
    }
    if !loop_live(h, conv) || h.pending().is_none() {
        // Cancelled or timed out while the handler ran.
        *conv = None;
        return;
    }
    let Some(l) = lp(conv) else {
        return;
    };
    let Waiting::Dispatched(call) = std::mem::replace(&mut l.waiting, Waiting::Nothing) else {
        return;
    };
    // Q3 — a handler that set nothing sends an empty result.
    let result = h.take_tool_result(&call.id).unwrap_or_default();
    if is_verbose(h) {
        log_block(h, &format!("tool {} → result (COBOL)", call.name), &result);
    }
    if let Some(l) = lp(conv) {
        l.results.push((call, result));
    }
    loop_advance(h, conv);
}

/// Spec 068 — a runtime tool's worker (a KnowledgeBase search) finished for this agent's tool loop:
/// answer the call and move the loop on.
pub fn tool_worker_delivered(h: &mut dyn AgentHost, conv: &mut Option<Conversation>, call_id: &str, text: String) {
    let Some(l) = lp(conv) else {
        return;
    };
    let call = match &l.waiting {
        Waiting::KbSearch(call) if call.id == call_id => call.clone(),
        _ => return,
    };
    l.waiting = Waiting::Nothing;
    l.results.push((call, text));
    loop_advance(h, conv);
}

fn loop_publish_usage(h: &mut dyn AgentHost, conv: &Option<Conversation>) {
    if let Some(Conversation::Loop(l)) = conv {
        let (i, o, c) = (l.usage.input, l.usage.output, l.calls_made);
        h.set("LastInputTokens", i.to_string());
        h.set("LastOutputTokens", o.to_string());
        h.set("LastToolCallCount", c.to_string());
    }
}

fn loop_answer(h: &mut dyn AgentHost, conv: &mut Option<Conversation>, text: String) {
    loop_publish_usage(h, conv);
    *conv = None;
    h.set_pending(None);
    h.set("Busy", "0".to_owned());
    answered(h, text);
}

fn loop_fail(h: &mut dyn AgentHost, conv: &mut Option<Conversation>, message: &str) {
    loop_publish_usage(h, conv);
    *conv = None;
    h.set_pending(None);
    h.set("Busy", "0".to_owned());
    failed(h, message);
}

/// What a request's worker reports to the program's thread.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Report {
    /// The model has begun its answer, or is still answering (at most every quarter of a second).
    Alive,
    /// A `StreamReply` Ask's text so far (at most ten a second).
    Partial(String),
    /// The finished reply, raw: the status and the body (status 0: no HTTP response, the body says
    /// why).
    Reply { status: u16, body: String },
}

impl Report {
    /// Does it end its request?
    pub fn is_final(&self) -> bool {
        matches!(self, Report::Reply { .. })
    }
}

/// One agent request to send on a worker: streamed, as every agent request is, tool rounds included —
/// only a streamed answer shows the moment a model BEGINS, which is what `StartTimeoutSeconds`
/// measures.
#[derive(Debug, Clone)]
pub struct Job {
    url: String,
    /// The body as it goes on the wire, asking for a stream.
    body: String,
    cfg: RequestConfig,
    generation: u64,
    show_partials: bool,
}

impl Job {
    fn new(url: String, body: &str, protocol: Protocol, cfg: RequestConfig, generation: u64, show_partials: bool) -> Job {
        Job { url, body: ag::streaming_body_for(body, protocol), cfg, generation, show_partials }
    }

    /// The generation the request was sent under; its reports carry it.
    pub fn generation(&self) -> u64 {
        self.generation
    }

    /// Send the request with `http` and report on it, on the worker's thread.
    ///
    /// * The first piece, then one every quarter of a second while pieces keep coming, is reported
    ///   as [`Report::Alive`].
    /// * `StreamReply` (`show_partials`) also reports the text so far.
    /// * The finished stream is rebuilt as the provider's unstreamed reply
    ///   (`StreamAssembler::into_body`) and reported as [`Report::Reply`], so it is read exactly as
    ///   before.
    /// * When the call is cancelled or times out, `keep_going` turns false and the worker stops at
    ///   the next piece, DROPPING the connection — a local model is not left generating for nobody. A
    ///   model that sends nothing at all is cut off by the socket's own limit, `cfg.timeout_ms`.
    pub fn run(self, http: &HttpClient, keep_going: &dyn Fn() -> bool, send: &dyn Fn(Report)) {
        let mut asm = ag::StreamAssembler::default();
        let mut last_alive: Option<Instant> = None;
        let mut last_partial = Instant::now();
        let mut unsent = false;
        let show_partials = self.show_partials;
        let (err, status) = http.post_streaming_while(&self.url, &self.body, &self.cfg, keep_going, &mut |line| {
            unsent |= asm.feed(line);
            // A quarter of a second: well inside any silence limit worth setting, so
            // `TimeoutSeconds` measures the model's silence and not this throttle's.
            if asm.started && last_alive.is_none_or(|t| t.elapsed() >= std::time::Duration::from_millis(250)) {
                last_alive = Some(Instant::now());
                send(Report::Alive);
            }
            // Throttled: a handler per token would drown the event loop; ten a second reads as live.
            if show_partials && unsent && last_partial.elapsed() >= std::time::Duration::from_millis(100) {
                unsent = false;
                last_partial = Instant::now();
                send(Report::Partial(asm.text.clone()));
            }
        });
        // A transport error AFTER the provider said "done" is the end of the stream, not a failure
        // of it.
        let (body, status) = if (200..300).contains(&status) || (status == 0 && asm.finished) {
            (asm.into_body(), if status == 0 { 200 } else { status })
        } else {
            (err, status)
        };
        send(Report::Reply { status, body });
    }
}

/// What a report does to the agent it belongs to, on the program's thread: the reply, the text so
/// far, or the news that the model is answering.
pub fn reported(h: &mut dyn AgentHost, conv: &mut Option<Conversation>, report: Report) {
    match report {
        Report::Alive => alive(h, conv),
        Report::Partial(text) => partial(h, conv, text),
        Report::Reply { status, body } => delivered(h, conv, status, &body),
    }
}

/// The methods of an AgentObject the door answers for a host that holds no interpreter: `Ask` and the
/// declared tools' methods (spec 072), and the general ones the interpreter answers for any control
/// (`SetPrompt`, `SetModel`, `GetResult` with no argument). `None` for any other.
pub enum Method {
    /// `Ask(prompt)`: the prompt is written to `Prompt`, the request goes ([`ask`]), the answer is empty.
    Ask(String),
    /// `AddTool`, `AddToolParameter`, `RemoveTool` on the declared tools; the answer.
    Tools(String),
    /// `SetToolResult(call-id, text)`: the result the handler gives the call.
    ToolResult(String, String),
    /// A property written, and the answer.
    Set(&'static str, String),
    /// An answer read from the control.
    Answer(String),
}

/// `method` (upper case) with the argument texts `args`, on an agent whose declared tools are
/// `tools` and whose properties `prop` reads: what it does, or `None` when it is not one of these.
pub fn method(prop: &dyn Fn(&str) -> String, tools: &mut Vec<DeclaredTool>, method: &str, args: &[String]) -> Option<Method> {
    let arg = |i: usize| args.get(i).cloned().unwrap_or_default();
    Some(match method {
        "ASK" => Method::Ask(arg(0)),
        "ADDTOOL" => Method::Tools(add_tool(tools, arg(0), arg(1))),
        "ADDTOOLPARAMETER" => Method::Tools(if add_tool_parameter(tools, &arg(0), arg(1), arg(2)) { "1" } else { "0" }.into()),
        "REMOVETOOL" => Method::Tools(if remove_tool(tools, &arg(0)) { "1" } else { "0" }.into()),
        "SETTOOLRESULT" => Method::ToolResult(arg(0), arg(1)),
        "SETPROMPT" => Method::Set("SystemPrompt", arg(0)),
        // `AgentModel` — the property `Ask` sends.
        "SETMODEL" => Method::Set("AgentModel", arg(0)),
        "GETRESULT" if args.is_empty() => Method::Answer(prop("Result")),
        _ => return None,
    })
}
