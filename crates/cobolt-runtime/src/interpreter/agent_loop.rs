// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! The `AgentObject` on the interpreter: the host of its rules, and the tools only the interpreter
//! offers.
//!
//! What an `Ask` does — its request, its streamed reply, its continuations and its tool loop
//! (spec 072) — is `crate::agent_door`'s, which the desktop runner of a program built on the
//! WebAssembly path follows too (DESKTOP-WASM-001 T47c). [`AgentSite`] is the interpreter as the
//! host of those rules: the control's properties and events, the program's output, the in-flight
//! entry the timeout sweep reads, the workers, and the tools the runtime answers itself — the
//! application's **consultable indexed files** (spec 065), searched here on the interpreter thread,
//! read-only and bounded by 065's memory limit, and the Knowledge Base collections (spec 068).

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use super::Interpreter;
use crate::agent_door::{AgentHost, DeclaredTool, Job, Report, Seen};
use crate::agent_tools::{ToolCall, ToolSpec};
use crate::async_op::{AsyncOpResult, AsyncOutcome, PendingOp};

/// The interpreter as the host of one AgentObject's rules (`agent_door`).
pub(super) struct AgentSite<'a> {
    pub(super) i: &'a mut Interpreter,
    /// The agent, as the caller spelt it: the key of its `Ask`, its pending entry and its generation.
    pub(super) obj: &'a str,
}

impl AgentHost for AgentSite<'_> {
    fn name(&self) -> &str {
        self.obj
    }

    fn prop(&self, name: &str) -> String {
        self.i.obj_get(self.obj, name)
    }

    fn set(&mut self, name: &str, value: String) {
        self.i.obj_set(self.obj, name, value);
    }

    fn event(&mut self, name: &str) {
        self.i.queue_control_event(self.obj, name);
    }

    /// One verbose line, to wherever this program's DISPLAY goes — the IDE output panel under a GUI
    /// run, stdout under the CLI.
    fn log(&mut self, line: String) {
        self.i.agent_log(line);
    }

    /// The reply goes only into an item the program actually declares. The generated `<agent>-ASK`
    /// paragraph used to MOVE the answer straight after `Ask` — which has returned at once since
    /// 1.65.63, so it always moved spaces.
    fn set_item(&mut self, item: &str, text: &str) {
        if self.i.env.contains(item) {
            self.i.env.set_str(item, text);
        }
    }

    fn runtime_tools(&self) -> Vec<ToolSpec> {
        let mut tools: Vec<ToolSpec> = crate::mcp_tool::agent_tool_specs(&self.i.mcp_tools);
        tools.extend(self.i.kb_tool_specs(self.obj));
        tools
    }

    fn declared_tools(&self) -> Vec<DeclaredTool> {
        self.i.agent_declared_tools.get(&self.obj.trim().to_ascii_uppercase()).cloned().unwrap_or_default()
    }

    fn tool_kind(&self, name: &str) -> &'static str {
        if self.i.kb_tool_is(self.obj, name) {
            "KnowledgeBase"
        } else {
            "IndexedFile"
        }
    }

    fn run_tool(&mut self, call: &ToolCall, generation: u64) -> Option<String> {
        // Spec 068 — a KnowledgeBase collection. Answered here, or on a worker when the query needs
        // the embedding server.
        if self.i.kb_tool_is(self.obj, &call.name) {
            return self.i.kb_tool_search(self.obj, call, generation);
        }
        // A consultable indexed file (spec 065), read-only.
        Some(crate::mcp_tool::agent_tool_answer(&self.i.mcp_tools, call))
    }

    fn take_tool_result(&mut self, call_id: &str) -> Option<String> {
        self.i.tool_results.remove(call_id)
    }

    fn live_generation(&self) -> u64 {
        self.i.async_generations.get(self.obj).map(|g| g.load(Ordering::Relaxed)).unwrap_or(0)
    }

    fn next_generation(&mut self) -> u64 {
        let gen = self.i.async_generations.entry(self.obj.to_string()).or_insert_with(|| Arc::new(AtomicU64::new(0)));
        gen.fetch_add(1, Ordering::Relaxed) + 1
    }

    fn pending(&self) -> Option<PendingOp> {
        self.i.async_pending.get(self.obj).copied()
    }

    fn set_pending(&mut self, op: Option<PendingOp>) {
        match op {
            Some(op) => {
                self.i.async_pending.insert(self.obj.to_string(), op);
            }
            None => {
                self.i.async_pending.remove(self.obj);
            }
        }
    }

    fn watch(&mut self, seen: Seen) {
        match seen {
            Seen::Forget => {
                self.i.model_entry_seen.remove(self.obj);
            }
            Seen::Watch(name, generation) => {
                self.i.model_entry_seen.insert(self.obj.to_string(), (name, generation));
            }
        }
    }

    /// The request goes on a worker with a copy of this program's HTTP client; its reports come back
    /// through the async channel under its generation. When the call is cancelled or times out, its
    /// generation moves on and the worker stops at the next piece.
    fn send(&mut self, job: Job) {
        let tx = self.i.async_result_tx.clone();
        let http = self.i.http.clone();
        let ctrl_id = self.obj.to_string();
        let live = self.i.async_generations.get(self.obj).cloned();
        let generation = job.generation();
        std::thread::spawn(move || {
            let keep_going = || live.as_ref().is_none_or(|g| g.load(Ordering::Relaxed) == generation);
            job.run(&http, &keep_going, &|report| {
                let outcome = match report {
                    Report::Alive => AsyncOutcome::AgentAlive,
                    Report::Partial(text) => AsyncOutcome::AgentPartial { text },
                    Report::Reply { status, body } => AsyncOutcome::AgentReply { status, body },
                };
                let _ = tx.send(AsyncOpResult { ctrl_id: ctrl_id.clone(), generation, outcome });
            });
        });
    }
}

impl Interpreter {
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
            .and_then(|k| super::declared_key_spec(layout, k, &spec.record_key_quals, &spec.record_key_parts, false))
            .unwrap_or(crate::indexed::KeySpec {
                parts: Vec::new(),
                offset: 0,
                len: layout.len.max(1),
                duplicates: false,
            });
        let access = crate::mcp_tool::FileAccess::of_layout(path, layout, primary);
        crate::mcp_tool::allow_file(&mut self.mcp_tools, &key, cidx, access)
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
        let (ok, props) = crate::registered_file::register_for_agent(&mut self.mcp_tools, data, cidx, name, self.free_memory_probe);
        for (prop, value) in props {
            self.obj_set(obj, prop, value);
        }
        ok
    }
}
