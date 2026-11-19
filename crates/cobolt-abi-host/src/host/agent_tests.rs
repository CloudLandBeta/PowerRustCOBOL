// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! DESKTOP-WASM-001 T47c (ABI A50): what the host does with an AgentObject's methods and its
//! requests' reports, with a scripted port standing in for the runner's. The port decides; the host
//! writes the narration where `DISPLAY` goes, the properties at once, and queues the events bound to
//! a handler. `onToolCall` is followed past the queue: the port hears that its handler has returned
//! before the program takes another event, or at once when no handler is bound.

use super::*;
use cobolt_forms::xml::load_form_from_str;
use cobolt_wasm_abi::batch::{BatchWriter, PathSpec};
use cobolt_wasm_abi::manifest::{ControlEntry, FormEntry, HandlerEntry, Limits, VariantInfo};
use std::cell::RefCell;

/// What the scripted port was asked, and what it answers next.
#[derive(Default)]
struct Script {
    /// `(control, method, arguments)` of each call.
    calls: Vec<(String, String, Vec<String>)>,
    answers: VecDeque<Option<AgentEffects>>,
    /// What `tool_returned` answers, and the controls it was told about, with `Prompt` as it read then.
    returned_fx: AgentEffects,
    returned: Vec<(String, String)>,
    running: bool,
    cancelled: usize,
    released: Vec<u32>,
}

struct Port(Rc<RefCell<Script>>);

impl AgentPort for Port {
    fn call(&mut self, _inst: u32, c: &AgentControl<'_>, method: &str, args: &[&str]) -> Option<AgentEffects> {
        let mut s = self.0.borrow_mut();
        s.calls.push((c.name.to_owned(), method.to_owned(), args.iter().map(|a| (*a).to_owned()).collect()));
        s.answers.pop_front().flatten()
    }
    fn cancel(&mut self, _inst: u32, _ctl: &str) -> bool {
        let mut s = self.0.borrow_mut();
        s.cancelled += 1;
        std::mem::take(&mut s.running)
    }
    fn tool_returned(&mut self, _inst: u32, c: &AgentControl<'_>) -> AgentEffects {
        let mut s = self.0.borrow_mut();
        s.returned.push((c.name.to_owned(), (c.prop)("Prompt")));
        s.returned_fx.clone()
    }
    fn release(&mut self, inst: u32) {
        self.0.borrow_mut().released.push(inst);
    }
}

const NAMES: [&str; 4] = ["Ask", "Cancel", "IsBusy", "SetToolResult"];
const EVENTS: [&str; 6] = ["onClick", "onResponse", "onToolCall", "onCancelled", "onPartialReply", "onError"];

/// A desktop host with `AGT-1`, which has handlers for `onResponse`, `onCancelled` and, when
/// `tool_handler`, `onToolCall`; none for `onPartialReply` or `onError`.
fn host(port: Option<&Rc<RefCell<Script>>>, tool_handler: bool) -> Host {
    let form = load_form_from_str(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<Form name="main-form" title="Assistant" width="640" height="480" index-base="1">
  <Control id="AGT-1" type="AgentObject" x="0" y="0" w="32" h="32"><Property name="AgentModel">stub-model</Property></Control>
</Form>"#,
    )
    .expect("the form loads");
    let mut handlers = vec![HandlerEntry { ctrl: 3, event: 1, entry: 0 }, HandlerEntry { ctrl: 3, event: 3, entry: 1 }];
    if tool_handler {
        handlers.push(HandlerEntry { ctrl: 3, event: 2, entry: 2 });
    }
    let manifest = Manifest {
        contract: None,
        remote: Vec::new(),
        capabilities: Vec::new(),
        abi: 1,
        generator: "test".into(),
        project: "assistant".into(),
        variant: VariantInfo { multi_memory: true, conformance: false },
        limits: Limits::default(),
        names: NAMES.iter().map(|n| n.to_string()).collect(),
        events: EVENTS.iter().map(|e| e.to_string()).collect(),
        forms: vec![FormEntry {
            id: 0,
            name: "main-form".into(),
            main: true,
            instances: 1,
            region_size: 64,
            controls: vec![ControlEntry { id: 3, name: "AGT-1".into(), kind: "AgentObject".into() }],
            handlers,
            layers: vec![],
        }],
        builtins: vec![],
        imports: vec![],
        sizes: Default::default(),
    };
    let mut h = Host::new(manifest, HostOptions { desktop: true, ..HostOptions::default() });
    if let Some(p) = port {
        h.agent = Some(Box::new(Port(Rc::clone(p))));
    }
    h.add_form(form).unwrap();
    h.create_tree(0, 0, None, Vec::new()).expect("the tree builds");
    h
}

fn answer(h: &mut Host, name: u32, args: &[&str]) -> String {
    let mut w = BatchWriter::new();
    w.call_result(&PathSpec::call(0, 3, name, args));
    String::from_utf8(h.query(w.as_bytes()).expect("the call answers")).unwrap()
}

fn prop(h: &Host, p: &str) -> String {
    h.control_property(0, "AGT-1", p).unwrap()
}

/// The queue, as `event` (an event) or `tool call` (an `onToolCall` the host follows).
fn queue(h: &Host) -> Vec<String> {
    h.work
        .iter()
        .filter_map(|w| match w {
            Work::Event(e) => Some(h.manifest.events[e.event as usize].clone()),
            Work::ToolCall { .. } => Some("tool call".to_owned()),
            _ => None,
        })
        .collect()
}

fn displayed(h: &mut Host) -> Vec<String> {
    h.take_effects()
        .into_iter()
        .filter_map(|e| match e {
            Effect::Display(line) => Some(line),
            _ => None,
        })
        .collect()
}

fn event_of(h: &Host, call: &Option<ModuleCall>) -> Option<String> {
    match call {
        Some(ModuleCall::Event { ctrl: 3, event, .. }) => Some(h.manifest.events[*event as usize].clone()),
        _ => None,
    }
}

#[test]
fn an_agentobject_method_is_the_ports_and_its_reports_are_carried_out() {
    let script = Rc::new(RefCell::new(Script::default()));
    let mut h = host(Some(&script), true);
    let mut checks = 0;
    let mut check = |ok: bool, what: &str| {
        assert!(ok, "{what}");
        checks += 1;
    };

    // Ask: the port's narration, properties and answer.
    script.borrow_mut().answers.push_back(Some(AgentEffects {
        lines: vec!["[agent AGT-1] asking stub-model".into()],
        props: vec![("Prompt".into(), "What is COBOL?".into()), ("Busy".into(), "1".into())],
        events: Vec::new(),
        answer: Some(String::new()),
    }));
    check(answer(&mut h, 0, &["What is COBOL?"]).is_empty(), "Ask answers what the port says: the empty string");
    check(script.borrow().calls[0] == ("AGT-1".to_string(), "ASK".to_string(), vec!["What is COBOL?".to_string()]), "the port is asked with the control, the method and the arguments");
    check(prop(&h, "Prompt") == "What is COBOL?" && prop(&h, "Busy") == "true", "the properties it answered are written at once");
    check(displayed(&mut h) == ["[agent AGT-1] asking stub-model"], "the narration goes where DISPLAY goes");

    // A piece and then the answer: written at once; only the bound event is queued.
    h.agent_delivered(0, "AGT-1", AgentEffects { props: vec![("PartialReply".into(), "A lang".into())], events: vec!["onPartialReply".into()], ..AgentEffects::default() });
    check(prop(&h, "PartialReply") == "A lang" && queue(&h).is_empty(), "a piece is written; its unbound onPartialReply is not queued");
    h.agent_delivered(
        0,
        "AGT-1",
        AgentEffects {
            lines: vec!["[agent AGT-1] reply".into()],
            props: vec![("LastReply".into(), "A language.".into()), ("Busy".into(), "0".into())],
            events: vec!["onResponse".into()],
            answer: None,
        },
    );
    check(prop(&h, "LastReply") == "A language." && prop(&h, "Busy") == "false", "the answer's properties are written at once");
    check(queue(&h) == ["onResponse"], "the bound onResponse is queued");
    check(displayed(&mut h) == ["[agent AGT-1] reply"], "a report's narration goes where DISPLAY goes");
    let call = h.next_call();
    check(event_of(&h, &call).as_deref() == Some("onResponse"), "onResponse runs");
    h.complete(call.as_ref().unwrap(), CallResult::Status { status: Status::Done.code(), task: 0 });

    // A report for a control that is not there changes nothing.
    h.agent_delivered(0, "AGT-9", AgentEffects { events: vec!["onResponse".into()], ..AgentEffects::default() });
    h.agent_delivered(7, "AGT-1", AgentEffects { events: vec!["onResponse".into()], ..AgentEffects::default() });
    check(queue(&h).is_empty(), "a report for another control or a closed form is dropped");

    // A method that is not the control's own (the port leaves it) goes on to the general dispatch.
    script.borrow_mut().answers.push_back(None);
    check(answer(&mut h, 2, &[]) == "0" && script.borrow().calls.len() == 2, "the port is asked first, and IsBusy is the general dispatch's");
    println!(
        "── an_agentobject_method_is_the_ports_and_its_reports_are_carried_out ──\n  Ask (answer, properties, narration), a piece and an answer (written at once, bound events queued), a stray report, the general dispatch: {checks} of {checks} checks"
    );
}

#[test]
fn on_tool_call_waits_for_its_handler_and_cancel_is_the_hosts() {
    let script = Rc::new(RefCell::new(Script::default()));
    let mut h = host(Some(&script), true);
    let mut checks = 0;
    let mut check = |ok: bool, what: &str| {
        assert!(ok, "{what}");
        checks += 1;
    };
    let tool_call = AgentEffects {
        props: vec![("ToolCallId".into(), "call_1".into()), ("ToolName".into(), "get_balance".into())],
        events: vec!["onToolCall".into()],
        ..AgentEffects::default()
    };

    // A tool call: the properties at once, onToolCall followed past the queue.
    h.agent_delivered(0, "AGT-1", tool_call.clone());
    check(prop(&h, "ToolCallId") == "call_1" && prop(&h, "ToolName") == "get_balance", "the call's properties are written at once");
    check(queue(&h) == ["tool call"], "onToolCall waits in the queue as the step the host follows");
    // Something else arrives before the handler runs.
    h.agent_delivered(0, "AGT-1", AgentEffects { events: vec!["onResponse".into()], ..AgentEffects::default() });
    let call = h.next_call();
    check(event_of(&h, &call).as_deref() == Some("onToolCall"), "its bound handler runs first");
    check(script.borrow().returned.is_empty(), "the port is not told while the handler runs");

    // The handler yields: the instance is busy, so the port still waits.
    h.complete(call.as_ref().unwrap(), CallResult::Status { status: Status::Yielded.code(), task: 7 });
    check(h.next_call().is_none() && script.borrow().returned.is_empty(), "a handler that yielded has not returned: nothing runs, the port waits");
    h.allow_yielded();
    let resumed = h.next_call();
    check(matches!(resumed, Some(ModuleCall::Resume { task: 7, .. })), "the yielded handler resumes");
    let _ = h.trees.get_mut(&0).unwrap().set(0, "Title", "answered");
    let idx = h.trees[&0].by_name("AGT-1").unwrap();
    let _ = h.trees.get_mut(&0).unwrap().set(idx, "Prompt", "as the handler left it");
    script.borrow_mut().returned_fx = AgentEffects { props: vec![("Busy".into(), "1".into())], lines: vec!["[agent AGT-1] tool get_balance → result".into()], ..AgentEffects::default() };
    h.complete(resumed.as_ref().unwrap(), CallResult::Status { status: Status::Done.code(), task: 0 });
    let _ = h.take_effects();

    // It returned: the port hears before the program takes the next event, and reads the control as the handler left it.
    let next = h.next_call();
    check(script.borrow().returned == [("AGT-1".to_string(), "as the handler left it".to_string())], "the port hears once the handler has returned, reading the control as it left it");
    check(event_of(&h, &next).as_deref() == Some("onResponse"), "and only then does the program take the next event");
    check(prop(&h, "Busy") == "true" && displayed(&mut h) == ["[agent AGT-1] tool get_balance → result"], "what the port answers is carried out");
    h.complete(next.as_ref().unwrap(), CallResult::Status { status: Status::Done.code(), task: 0 });

    // With no onToolCall handler bound, the port hears as the step leaves the queue.
    let unbound = Rc::new(RefCell::new(Script::default()));
    let mut u = host(Some(&unbound), false);
    u.agent_delivered(0, "AGT-1", tool_call.clone());
    check(u.next_call().is_none() && unbound.borrow().returned.len() == 1, "no handler bound: nothing runs, the port hears at once");

    // Cancel with an Ask in flight: no longer busy, onCancelled; the port is not asked to call.
    let calls_before = script.borrow().calls.len();
    let _ = h.trees.get_mut(&0).unwrap().set(idx, "Busy", "1");
    script.borrow_mut().running = true;
    check(answer(&mut h, 1, &[]).is_empty(), "Cancel answers nothing");
    check(script.borrow().calls.len() == calls_before && script.borrow().cancelled == 1, "Cancel is the host's uniform one: the port lets the Ask go");
    check(prop(&h, "Busy") == "false" && queue(&h) == ["onCancelled"], "no longer busy, and onCancelled is raised");
    h.work.clear();
    check(answer(&mut h, 1, &[]).is_empty() && queue(&h).is_empty() && script.borrow().cancelled == 2, "Cancel with nothing in flight raises nothing");

    // An instance that ends: the port hears, and its tool-call steps and waits go.
    h.agent_delivered(0, "AGT-1", tool_call.clone());
    let waiting = h.next_call();
    check(event_of(&h, &waiting).as_deref() == Some("onToolCall") && h.tool_waits.len() == 1, "a handler handed out is waited for");
    h.complete(waiting.as_ref().unwrap(), CallResult::Status { status: Status::Yielded.code(), task: 8 });
    h.agent_delivered(0, "AGT-1", tool_call);
    h.destroy(0);
    check(script.borrow().released == [0], "the port hears an instance end");
    check(h.tool_waits.is_empty() && queue(&h).is_empty(), "its waits and its queued tool calls go with it");

    // A desktop host with no port: Ask is the general dispatch's, which does not know it, and nothing is sent.
    let mut bare = host(None, true);
    let mut w = BatchWriter::new();
    w.call_result(&PathSpec::call(0, 3, 0, &["anything"]));
    let refused = bare.query(w.as_bytes());
    check(refused == Err(err::UNKNOWN_TARGET) && prop(&bare, "Busy") != "true" && queue(&bare).is_empty(), "a host with no port refuses Ask as an unknown method, sends nothing and raises nothing");
    println!(
        "── on_tool_call_waits_for_its_handler_and_cancel_is_the_hosts ──\n  onToolCall (bound: after its handler returns, before the next event; yielded: still waits; unbound: at once), Cancel in flight and idle, an instance's end, a host with no port: {checks} of {checks} checks"
    );
}
