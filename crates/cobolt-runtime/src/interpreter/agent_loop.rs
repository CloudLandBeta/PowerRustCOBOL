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
        let mut tools: Vec<ToolSpec> = self
            .i
            .mcp_tools
            .tools()
            .into_iter()
            .map(|t| ToolSpec { name: t.name, description: t.description.unwrap_or_default(), parameters: t.input_schema })
            .collect();
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
        let r = self.i.mcp_tools.call(&call.name, call.arguments.as_ref().unwrap_or(&serde_json::Value::Null));
        let text: Vec<String> = r.content.iter().filter_map(|c| c.as_text().map(str::to_owned)).collect();
        let text = text.join("\n");
        Some(if r.is_error.unwrap_or(false) { format!("error: {text}") } else { text })
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
