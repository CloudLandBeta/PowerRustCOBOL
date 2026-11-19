// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! The desktop's AgentObject (DESKTOP-WASM-001 T47c, ABI A50).
//!
//! Every decision is the interpreter's own ([`agent_door`]): how an `Ask` is prepared and narrated,
//! what a reply, a piece of a streamed one or an error does, when an answer cut off by
//! `MaximumTokens` is continued, and how a tool loop goes round. A request runs on a thread of its
//! own with a copy of its window's HTTP client, as the interpreter's worker runs with a copy of its
//! client, and its reports come back to the program thread as [`AgentNews`]. A report from a request
//! that was cancelled, timed out, superseded or whose form closed is dropped, as the interpreter
//! drops a stale one. While an `Ask` is in flight the program thread looks at the clock often enough
//! ([`Agents::polling`], [`Agents::sweep`]) to say when it ran out of time, with the interpreter's
//! own rule (`PendingOp::expired`).
//!
//! The tools the runtime answers itself (indexed files, Knowledge Base collections) are not offered
//! here yet: a program that allows one takes the Cargo path (the Build refuses the method).

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use cobolt_abi_host::host::{AgentControl, AgentEffects, AgentPort};
use cobolt_runtime::agent_door::{self, AgentHost, Conversation, DeclaredTool, Job, Method, Report, Seen};
use cobolt_runtime::agent_tools::{ToolCall, ToolSpec};
use cobolt_runtime::async_op::PendingOp;
use cobolt_runtime::HttpClient;

use crate::http::Clients;

/// A request's report, on its way to the program thread.
pub(crate) struct AgentNews {
    pub inst: u32,
    /// The control, as the form writes it.
    pub ctl: String,
    pub generation: u64,
    pub report: Report,
}

/// One AgentObject of one form instance.
struct Agent {
    /// Its name, as the form writes it.
    name: String,
    conv: Option<Conversation>,
    pending: Option<PendingOp>,
    /// The generation it is at, which its requests' threads read to know they are still wanted.
    generation: Arc<AtomicU64>,
    /// The tools the program declared on it.
    tools: Vec<DeclaredTool>,
    /// The model-list entry it uses, and its generation then (spec 076 R16).
    seen: Option<(String, u64)>,
}

impl Agent {
    fn new(name: &str) -> Agent {
        Agent { name: name.to_owned(), conv: None, pending: None, generation: Arc::default(), tools: Vec::new(), seen: None }
    }
}

/// What the program thread and the host share.
#[derive(Default)]
struct State {
    /// `(instance, CONTROL)` → the agent.
    agents: HashMap<(u32, String), Agent>,
    /// Each instance's `SetToolResult(call-id, text)`, waiting to be collected: the interpreter keeps
    /// them per program.
    results: HashMap<u32, HashMap<String, String>>,
}

fn key(inst: u32, ctl: &str) -> (u32, String) {
    (inst, ctl.trim().to_ascii_uppercase())
}

/// The runner's AgentObjects: the host's [`AgentPort`], and (a clone) the program thread's handle on
/// the same state.
#[derive(Clone)]
pub(crate) struct Agents {
    state: Rc<RefCell<State>>,
    http: Rc<RefCell<Clients>>,
    /// Where a report goes: the program thread's own channel, from the request's thread.
    send: Arc<dyn Fn(AgentNews) + Send + Sync>,
}

/// One agent as the host of the door's rules: what they write and raise is gathered as the host's
/// effects, and what they send goes on a thread of its own.
struct Site<'a> {
    inst: u32,
    agent: &'a mut Agent,
    results: &'a mut HashMap<String, String>,
    prop: &'a dyn Fn(&str) -> String,
    /// The HTTP client a request is sent with.
    client: &'a dyn Fn() -> HttpClient,
    send: &'a Arc<dyn Fn(AgentNews) + Send + Sync>,
    fx: AgentEffects,
}

impl AgentHost for Site<'_> {
    fn name(&self) -> &str {
        &self.agent.name
    }

    /// What the rules wrote in this call, then the control as the program would read it.
    fn prop(&self, name: &str) -> String {
        match self.fx.props.iter().rev().find(|(k, _)| k.eq_ignore_ascii_case(name)) {
            Some((_, v)) => v.clone(),
            None => (self.prop)(name),
        }
    }

    fn set(&mut self, name: &str, value: String) {
        self.fx.props.push((name.to_owned(), value));
    }

    fn event(&mut self, name: &str) {
        self.fx.events.push(name.to_owned());
    }

    fn log(&mut self, line: String) {
        self.fx.lines.push(line);
    }

    /// A module holds no data item a host can name: the Build sends an AgentObject that names one
    /// (`ResponseDataItem`) down the Cargo path.
    fn set_item(&mut self, _item: &str, _text: &str) {}

    fn runtime_tools(&self) -> Vec<ToolSpec> {
        Vec::new()
    }

    fn declared_tools(&self) -> Vec<DeclaredTool> {
        self.agent.tools.clone()
    }

    fn tool_kind(&self, _name: &str) -> &'static str {
        "IndexedFile"
    }

    /// Never reached: no runtime tool is offered, and a call to a tool not offered is told so first.
    fn run_tool(&mut self, call: &ToolCall, _generation: u64) -> Option<String> {
        Some(format!("error: no tool named '{}' is offered", call.name))
    }

    fn take_tool_result(&mut self, call_id: &str) -> Option<String> {
        self.results.remove(call_id)
    }

    fn live_generation(&self) -> u64 {
        self.agent.generation.load(Ordering::Relaxed)
    }

    fn next_generation(&mut self) -> u64 {
        self.agent.generation.fetch_add(1, Ordering::Relaxed) + 1
    }

    fn pending(&self) -> Option<PendingOp> {
        self.agent.pending
    }

    fn set_pending(&mut self, op: Option<PendingOp>) {
        self.agent.pending = op;
    }

    fn watch(&mut self, seen: Seen) {
        self.agent.seen = match seen {
            Seen::Forget => None,
            Seen::Watch(name, generation) => Some((name, generation)),
        };
    }

    fn send(&mut self, job: Job) {
        let http = (self.client)();
        let live = Arc::clone(&self.agent.generation);
        let send = Arc::clone(self.send);
        let (inst, ctl, generation) = (self.inst, self.agent.name.clone(), job.generation());
        std::thread::spawn(move || {
            let keep_going = || live.load(Ordering::Relaxed) == generation;
            job.run(&http, &keep_going, &|report| send(AgentNews { inst, ctl: ctl.clone(), generation, report }));
        });
    }
}

impl Agents {
    pub(crate) fn new(http: Rc<RefCell<Clients>>, send: impl Fn(AgentNews) + Send + Sync + 'static) -> Agents {
        Agents { state: Rc::default(), http, send: Arc::new(send) }
    }

    /// Run the door's rules `f` for the agent `ctl` of instance `inst`, whose properties `prop` reads,
    /// sending with the client `client` gives: what they did, as the host's effects. `None` when the
    /// agent is unknown and `create` is false.
    fn with<R>(
        &self,
        inst: u32,
        ctl: &str,
        create: bool,
        prop: &dyn Fn(&str) -> String,
        client: &dyn Fn() -> HttpClient,
        f: impl FnOnce(&mut Site<'_>, &mut Option<Conversation>) -> R,
    ) -> Option<(R, AgentEffects)> {
        let mut st = self.state.borrow_mut();
        let st = &mut *st;
        let k = key(inst, ctl);
        if create {
            st.agents.entry(k.clone()).or_insert_with(|| Agent::new(ctl));
        }
        let agent = st.agents.get_mut(&k)?;
        let results = st.results.entry(inst).or_default();
        let mut conv = agent.conv.take();
        let mut site = Site { inst, agent, results, prop, client, send: &self.send, fx: AgentEffects::default() };
        let r = f(&mut site, &mut conv);
        let fx = std::mem::take(&mut site.fx);
        drop(site);
        if let Some(a) = st.agents.get_mut(&k) {
            a.conv = conv;
        }
        Some((r, fx))
    }

    /// The client of form instance `inst`'s window, for a request sent outside a call into the module.
    fn client_of(&self, inst: u32) -> impl Fn() -> HttpClient + '_ {
        move || self.http.borrow().of(inst)
    }

    /// A report of a request of agent `ctl` of instance `inst`: what it does, or `None` when the
    /// request is no longer wanted (cancelled, timed out, superseded, or its form closed). `prop` reads
    /// the control's properties as they are now.
    pub(crate) fn delivered(&self, n: AgentNews, prop: &dyn Fn(&str) -> String) -> Option<AgentEffects> {
        {
            let mut st = self.state.borrow_mut();
            let agent = st.agents.get_mut(&key(n.inst, &n.ctl))?;
            if agent.generation.load(Ordering::Relaxed) != n.generation {
                return None;
            }
            // A progress report is not the end of its request: only the reply releases the control.
            if n.report.is_final() {
                agent.pending = None;
            }
        }
        let client = self.client_of(n.inst);
        let ((), fx) = self.with(n.inst, &n.ctl, false, prop, &client, |h, c| agent_door::reported(h, c, n.report))?;
        Some(fx)
    }

    /// Is an `Ask` in flight anywhere? The program thread then looks at the clock often enough to say
    /// when one ran out of time, as the interpreter's wait does.
    pub(crate) fn polling(&self) -> bool {
        self.state.borrow().agents.values().any(|a| a.pending.is_some())
    }

    /// The agents whose `Ask` ran out of time, and those whose model-list entry changed (spec 076
    /// R16): `(instance, control, what that does)`, in no particular order, as the interpreter sweeps.
    pub(crate) fn sweep(&self) -> Vec<(u32, String, AgentEffects)> {
        let now = std::time::Instant::now();
        let mut out = Vec::new();
        let mut st = self.state.borrow_mut();
        for ((inst, _), a) in st.agents.iter_mut() {
            if let Some((name, seen)) = &a.seen {
                let now = cobolt_runtime::model_list::generation(name);
                if now != *seen {
                    a.seen = Some((name.clone(), now));
                    out.push((*inst, a.name.clone(), AgentEffects { events: vec!["onModelChanged".into()], ..AgentEffects::default() }));
                }
            }
            let Some(why) = a.pending.and_then(|op| op.expired(now)) else { continue };
            // Say what happened; the request's thread stops at its next piece.
            a.pending = None;
            a.generation.fetch_add(1, Ordering::Relaxed);
            let props = vec![("LastError".to_owned(), why), ("Busy".to_owned(), "0".to_owned())];
            out.push((*inst, a.name.clone(), AgentEffects { props, events: vec!["onTimeout".into()], ..AgentEffects::default() }));
        }
        out
    }
}

impl AgentPort for Agents {
    fn call(&mut self, inst: u32, c: &AgentControl<'_>, method: &str, args: &[&str]) -> Option<AgentEffects> {
        let texts: Vec<String> = args.iter().map(|a| (*a).to_owned()).collect();
        let what = {
            let mut st = self.state.borrow_mut();
            let agent = st.agents.entry(key(inst, c.name)).or_insert_with(|| Agent::new(c.name));
            agent_door::method(c.prop, &mut agent.tools, method, &texts)?
        };
        match what {
            Method::Ask(prompt) => {
                // Sent from inside the call: the client of the window the call acts on, as the HTTP
                // built-ins use (a form still opening has its client before its instance is known).
                let client = || self.http.borrow_mut().client().clone();
                let ((), mut fx) = self.with(inst, c.name, true, c.prop, &client, |h, conv| {
                    // `Prompt` holds the question, written before the call is made.
                    h.set("Prompt", prompt.clone());
                    agent_door::ask(h, conv, &prompt)
                })?;
                fx.answer = Some(String::new());
                Some(fx)
            }
            Method::Tools(answer) | Method::Answer(answer) => Some(AgentEffects { answer: Some(answer), ..AgentEffects::default() }),
            Method::ToolResult(id, text) => {
                self.state.borrow_mut().results.entry(inst).or_default().insert(id, text);
                Some(AgentEffects::default())
            }
            Method::Set(prop, value) => Some(AgentEffects { props: vec![(prop.to_owned(), value)], ..AgentEffects::default() }),
        }
    }

    fn cancel(&mut self, inst: u32, ctl: &str) -> bool {
        let mut st = self.state.borrow_mut();
        let Some(a) = st.agents.get_mut(&key(inst, ctl)) else { return false };
        if a.pending.take().is_none() {
            return false;
        }
        a.generation.fetch_add(1, Ordering::Relaxed);
        true
    }

    fn tool_returned(&mut self, inst: u32, c: &AgentControl<'_>) -> AgentEffects {
        let client = self.client_of(inst);
        self.with(inst, c.name, false, c.prop, &client, |h, conv| {
            if let Some(cv) = conv.as_mut() {
                cv.dispatched();
            }
            agent_door::resume(h, conv)
        })
        .map(|((), fx)| fx)
        .unwrap_or_default()
    }

    fn release(&mut self, inst: u32) {
        // A request still running is let go: its thread stops at the next piece, so a model is not
        // left answering for a window that is gone.
        let mut st = self.state.borrow_mut();
        for ((i, _), a) in st.agents.iter() {
            if *i == inst {
                a.generation.fetch_add(1, Ordering::Relaxed);
            }
        }
        st.agents.retain(|(i, _), _| *i != inst);
        st.results.remove(&inst);
    }
}
