// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! **The AgentObject on the desktop** (DESKTOP-WASM-001 T47c, ABI A50).
//!
//! A method called on an `AgentObject` goes to the runner's [`AgentPort`], which decides it with the
//! interpreter's own rules (`cobolt_runtime::agent_door`) and sends its requests. The host carries
//! out what the port answers, as the interpreter does: the narration goes to the program's output,
//! the properties are written at once, and the events are queued for their handlers. A reply, a
//! piece of a streamed one, an error or a time-out comes back through [`Host::agent_delivered`] the
//! same way.
//!
//! `onToolCall` (spec 072) is the one event the host follows past the queue: the program answers the
//! call in that handler, and the conversation goes on only when it has returned. When the event
//! leaves the queue its handler runs, if one is bound, and once the instance is free again the port
//! hears it ([`AgentPort::tool_returned`]) before the program takes another event, as the
//! interpreter takes the handler's `SetToolResult` at its next wait. `Cancel` is the uniform one: an
//! `Ask` in flight is let go, the control is no longer busy and hears `onCancelled`. A browser module
//! never reaches this.

use super::*;

/// What the host knows of the AgentObject a method was called on.
pub struct AgentControl<'a> {
    /// The control's name in the form.
    pub name: &'a str,
    /// One of its properties, as the program would read it.
    pub prop: &'a dyn Fn(&str) -> String,
    /// A property of the form's KnowledgeBase named by the first argument, as the program would read
    /// it (`AllowKnowledgeBase`, and the search a model asks for): `None` when the form has no
    /// KnowledgeBase of that name.
    pub knowledge_base: &'a dyn Fn(&str, &str) -> Option<String>,
}

/// A property of the KnowledgeBase named `ctl` in `tree`: `None` when it has no KnowledgeBase of that
/// name.
fn knowledge_base_prop(tree: &FormTree, ctl: &str, prop: &str) -> Option<String> {
    let idx = tree.by_name(ctl)?;
    (tree.ctl(idx).kind.type_name() == "KnowledgeBase").then(|| tree.get(idx, prop))
}

/// What a method call, a report or a time-out does: the lines for the program's `DISPLAY` output
/// (the `Verbose` narration), then the properties written, in order, then the events raised, in
/// order, and the method's answer (`None`: the method answers nothing).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct AgentEffects {
    pub lines: Vec<String>,
    pub props: Vec<(String, String)>,
    pub events: Vec<String>,
    pub answer: Option<String>,
}

/// The desktop's AgentObject (DESKTOP-WASM-001 T47c), which the runner gives the host in
/// [`Host::agent`].
pub trait AgentPort {
    /// A method (`method`, upper case) of the AgentObject `c` of instance `inst`, with the argument
    /// texts `args`: what follows, or `None` when it is not one of the control's own methods, so the
    /// host's general dispatch answers it. `Cancel` never reaches it.
    fn call(&mut self, inst: u32, c: &AgentControl<'_>, method: &str, args: &[&str]) -> Option<AgentEffects>;
    /// `Cancel`: whether an `Ask` of the control was in flight. Its reports, when they come, are
    /// dropped.
    fn cancel(&mut self, inst: u32, ctl: &str) -> bool;
    /// The `onToolCall` the host queued for `c` was handed to the program, and its handler, when one
    /// is bound, has returned: the call's result is taken and the conversation goes on.
    fn tool_returned(&mut self, inst: u32, c: &AgentControl<'_>) -> AgentEffects;
    /// The instance `inst` ended: its conversations go, and reports still to come for it are dropped.
    fn release(&mut self, inst: u32);
    /// `COBOL::"MCP-SEARCH" ( tool arguments result )` called by instance `inst` (A54): the text the
    /// result receives, searched in the files the instance's AgentObjects allowed. `None`: the port
    /// does not search, and the result receives nothing.
    fn mcp_search(&mut self, inst: u32, tool: &str, arguments: &str) -> Option<String> {
        let _ = (inst, tool, arguments);
        None
    }
}

/// The application's model list, its keys and the model providers (DESKTOP-WASM-001 T48, A53), which
/// the runner gives the host in [`Host::models`].
pub trait ModelsPort {
    /// The built-in `verb` (upper case, one of `codes::MODEL_VERBS`) called by instance `inst` with the
    /// argument texts `args`: the arguments it writes, by 0-based place, with their texts.
    fn call(&mut self, inst: u32, verb: &str, args: &[&str]) -> Vec<(usize, String)>;
}

impl Host {
    /// A model-list built-in (A53): the answers go to the application's `BuiltinAnswer<n>`, `n` the
    /// argument's 1-based place, which the generated code moves into the arguments; an earlier call's
    /// answers are cleared first. A host with no port answers nothing.
    pub(super) fn models_call(&mut self, inst: u32, verb: &str, args: &[&str]) {
        let most = cobolt_wasm_abi::codes::MODEL_VERBS.iter().flat_map(|(_, _, w)| w.iter()).max().copied().unwrap_or(0);
        for i in 0..=most {
            self.app.remove(&format!("builtinanswer{}", i + 1));
        }
        let answers = match self.models.as_mut() {
            Some(port) => port.call(inst, verb, args),
            None => Vec::new(),
        };
        for (i, text) in answers {
            self.app.insert(format!("builtinanswer{}", i + 1), text);
        }
    }

    /// `COBOL::"MCP-SEARCH"` (A54): the result goes to the application's `BuiltinAnswer3`, which the
    /// generated code moves into the third argument; an earlier answer is cleared first. A host with no
    /// AgentObject port answers nothing.
    pub(super) fn mcp_search(&mut self, inst: u32, args: &[&str]) {
        self.app.remove("builtinanswer3");
        let arg = |i: usize| args.get(i).copied().unwrap_or("");
        if let Some(text) = self.agent.as_mut().and_then(|port| port.mcp_search(inst, arg(0), arg(1))) {
            self.app.insert("builtinanswer3".to_owned(), text);
        }
    }

    /// A method of the control `idx` when that control is an AgentObject and the host has a port:
    /// `None` otherwise, and for a method that is not the control's own.
    pub(super) fn agent_call(&mut self, inst: u32, idx: usize, name: &str, args: &[&str]) -> Option<Result<Option<String>, CmdError>> {
        let tree = self.trees.get(&inst)?;
        if tree.ctls[idx].kind.type_name() != "AgentObject" {
            return None;
        }
        let port = self.agent.as_mut()?;
        let m = name.trim().to_ascii_uppercase();
        let ctl = tree.ctl(idx).name.clone();
        if m == "CANCEL" {
            // The uniform asynchronous `Cancel` (spec 032): an `Ask` in flight is let go, its reports
            // dropped when they come; the control is no longer busy and hears `onCancelled`.
            if port.cancel(inst, &ctl) {
                let _ = self.trees.get_mut(&inst).expect("checked").set(idx, "Busy", "0");
                self.dom_event(inst, idx, "onCancelled", Vec::new());
            }
            return Some(Ok(None));
        }
        let prop = |p: &str| tree.get(idx, p);
        let kb = |k: &str, p: &str| knowledge_base_prop(tree, k, p);
        let fx = port.call(inst, &AgentControl { name: &ctl, prop: &prop, knowledge_base: &kb }, &m, args)?;
        Some(Ok(self.agent_apply(inst, idx, fx)))
    }

    /// Carry out `fx` on the control `idx` of instance `inst`: its lines go to the program's output
    /// first, then the properties are written and the events queued. `onToolCall` is queued as the
    /// step the host follows past the queue (see the module's notes).
    fn agent_apply(&mut self, inst: u32, idx: usize, fx: AgentEffects) -> Option<String> {
        for line in fx.lines {
            self.effects.push(Effect::Display(line));
        }
        if let Some(tree) = self.trees.get_mut(&inst) {
            for (k, v) in &fx.props {
                let _ = tree.set(idx, k, v);
            }
        }
        for e in &fx.events {
            if e == "onToolCall" {
                self.work.push_back(Work::ToolCall { inst, ctl: idx });
            } else {
                self.dom_event(inst, idx, e, Vec::new());
            }
        }
        fx.answer
    }

    /// A report of an `Ask` of the AgentObject named `ctl` of instance `inst` came, or its time ran
    /// out, and `fx` is what that does (the runner's port decided it). Nothing happens when the
    /// instance or the control is gone.
    pub fn agent_delivered(&mut self, inst: u32, ctl: &str, fx: AgentEffects) {
        let Some(idx) = self.trees.get(&inst).and_then(|t| t.by_name(ctl)) else { return };
        self.agent_apply(inst, idx, fx);
    }

    /// A property of the KnowledgeBase named `ctl` in instance `inst`, as the program would read it:
    /// `None` when the instance has no KnowledgeBase of that name. A report of an AgentObject's
    /// request reads its collections' settings through it.
    pub fn knowledge_base_property(&self, inst: u32, ctl: &str, prop: &str) -> Option<String> {
        knowledge_base_prop(self.trees.get(&inst)?, ctl, prop)
    }

    /// The `onToolCall` queued for control `ctl` of instance `inst` left the queue: its handler, when
    /// one is bound, is the event to run now (the port hears of it when it has returned); with none
    /// bound, the program has nothing to answer and the port hears at once.
    pub(super) fn agent_tool_call(&mut self, inst: u32, ctl: usize) -> Option<QueuedEvent> {
        let c = self.trees.get(&inst)?.ctls.get(ctl)?;
        let bound = c.has_handler("onToolCall").then_some(c.abi_id).flatten().zip(self.manifest.event_index("onToolCall"));
        match bound {
            Some((abi, ev)) => {
                self.tool_waits.push((inst, ctl));
                Some(QueuedEvent { inst, ctrl: abi, event: ev, args: Vec::new() })
            }
            None => {
                self.agent_tool_returned(inst, ctl);
                None
            }
        }
    }

    /// Every `onToolCall` handler handed out whose instance is free again has returned: the port
    /// takes each call's result, before the program takes another event.
    pub(super) fn agent_tools_returned(&mut self) {
        while let Some(at) = self.tool_waits.iter().position(|(inst, _)| !self.busy(*inst)) {
            let (inst, ctl) = self.tool_waits.remove(at);
            self.agent_tool_returned(inst, ctl);
        }
    }

    fn agent_tool_returned(&mut self, inst: u32, idx: usize) {
        let Some(tree) = self.trees.get(&inst) else { return };
        let Some(port) = self.agent.as_mut() else { return };
        let ctl = tree.ctl(idx).name.clone();
        let prop = |p: &str| tree.get(idx, p);
        let kb = |k: &str, p: &str| knowledge_base_prop(tree, k, p);
        let fx = port.tool_returned(inst, &AgentControl { name: &ctl, prop: &prop, knowledge_base: &kb });
        self.agent_apply(inst, idx, fx);
    }
}
