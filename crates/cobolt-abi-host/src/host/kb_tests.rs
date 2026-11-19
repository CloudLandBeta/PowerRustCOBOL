// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! DESKTOP-WASM-001 T47b (ABI A49): what the host does with a KnowledgeBase's methods and its
//! operations' reports, with a scripted port standing in for the runner's. The port decides; the
//! host writes what a method answers at once, and queues each event with its values, written as
//! the queue reaches them, bound or not; a search's results become the control's then.

use super::*;
use cobolt_forms::xml::load_form_from_str;
use cobolt_wasm_abi::batch::{BatchWriter, PathSpec};
use cobolt_wasm_abi::manifest::{ControlEntry, FormEntry, HandlerEntry, Limits, VariantInfo};
use serde_json::{json, Value};
use std::cell::RefCell;

/// What the scripted port was asked, and what it answers next.
#[derive(Default)]
struct Script {
    /// `(control, method, arguments)` of each call.
    calls: Vec<(String, String, Vec<String>)>,
    answers: VecDeque<Option<KbEffects>>,
    results: HashMap<String, Vec<Value>>,
    running: bool,
    cancelled: usize,
    released: Vec<u32>,
}

struct Port(Rc<RefCell<Script>>);

impl KbPort for Port {
    fn call(&mut self, _inst: u32, c: &KbControl<'_>, method: &str, args: &[&str]) -> Option<KbEffects> {
        let mut s = self.0.borrow_mut();
        s.calls.push((c.name.to_owned(), method.to_owned(), args.iter().map(|a| (*a).to_owned()).collect()));
        s.answers.pop_front().flatten()
    }
    fn set_results(&mut self, _inst: u32, ctl: &str, results: Vec<Value>) {
        self.0.borrow_mut().results.insert(ctl.to_owned(), results);
    }
    fn cancel(&mut self, _inst: u32, _ctl: &str) -> bool {
        let mut s = self.0.borrow_mut();
        s.cancelled += 1;
        std::mem::take(&mut s.running)
    }
    fn release(&mut self, inst: u32) {
        self.0.borrow_mut().released.push(inst);
    }
}

const NAMES: [&str; 4] = ["AddDocument", "Search", "Cancel", "IsBusy"];

/// A desktop host with `KB-1`, which has handlers for `onSearchComplete` and `onCancelled`, none
/// for `onProgress`.
fn host(port: Option<&Rc<RefCell<Script>>>) -> Host {
    let form = load_form_from_str(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<Form name="main-form" title="Documents" width="640" height="480" index-base="1">
  <Control id="KB-1" type="KnowledgeBase" x="0" y="0" w="32" h="32"><Property name="Collection">docs</Property></Control>
</Form>"#,
    )
    .expect("the form loads");
    let manifest = Manifest {
        contract: None,
        remote: Vec::new(),
        capabilities: Vec::new(),
        abi: 1,
        generator: "test".into(),
        project: "documents".into(),
        variant: VariantInfo { multi_memory: true, conformance: false },
        limits: Limits::default(),
        names: NAMES.iter().map(|n| n.to_string()).collect(),
        events: ["onClick", "onProgress", "onSearchComplete", "onCancelled"].iter().map(|e| e.to_string()).collect(),
        forms: vec![FormEntry {
            id: 0,
            name: "main-form".into(),
            main: true,
            instances: 1,
            region_size: 64,
            controls: vec![ControlEntry { id: 3, name: "KB-1".into(), kind: "KnowledgeBase".into() }],
            handlers: vec![HandlerEntry { ctrl: 3, event: 2, entry: 0 }, HandlerEntry { ctrl: 3, event: 3, entry: 1 }],
            layers: vec![],
        }],
        builtins: vec![],
        imports: vec![],
        sizes: Default::default(),
    };
    let mut h = Host::new(manifest, HostOptions { desktop: true, ..HostOptions::default() });
    if let Some(p) = port {
        h.kb = Some(Box::new(Port(Rc::clone(p))));
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
    h.control_property(0, "KB-1", p).unwrap()
}

/// The queue, as `event` (an event) or `apply event` (values waiting for theirs).
fn queue(h: &Host) -> Vec<String> {
    h.work
        .iter()
        .filter_map(|w| match w {
            Work::Event(e) => Some(h.manifest.events[e.event as usize].clone()),
            Work::Apply(a) => Some(format!("apply {}", a.event)),
            _ => None,
        })
        .collect()
}

#[test]
fn a_knowledgebase_method_is_the_ports_and_its_events_carry_their_values() {
    let script = Rc::new(RefCell::new(Script::default()));
    let mut h = host(Some(&script));
    let mut checks = 0;
    let mut check = |ok: bool, what: &str| {
        assert!(ok, "{what}");
        checks += 1;
    };

    // A start: the port's properties, and its answer.
    script.borrow_mut().answers.push_back(Some(KbEffects { props: vec![("Busy".into(), "1".into()), ("LastError".into(), String::new())], answer: "1".into() }));
    check(answer(&mut h, 1, &["salaries paid", "3"]) == "1", "the method answers what the port says");
    check(script.borrow().calls[0] == ("KB-1".to_string(), "SEARCH".to_string(), vec!["salaries paid".to_string(), "3".to_string()]), "the port is asked with the control, the method and the arguments");
    check(prop(&h, "Busy") == "true", "the properties it answered are written at once");

    // The reports: each event's values wait in the queue, bound or not.
    let hit = json!({"document": "pay.md", "heading": "pay", "passage": "Salaries are paid monthly.", "score": 0.5});
    h.kb_delivered(
        0,
        "KB-1",
        vec![
            KbEvent { name: "onProgress".into(), props: vec![("ProgressCurrent".into(), "1".into())], results: None },
            KbEvent { name: "onSearchComplete".into(), props: vec![("ResultCount".into(), "1".into()), ("Busy".into(), "0".into())], results: Some(vec![hit.clone()]) },
        ],
    );
    check(queue(&h) == ["apply onProgress", "apply onSearchComplete"], "every event waits in the queue with its values");
    check(prop(&h, "ProgressCurrent").is_empty() && prop(&h, "Busy") == "true", "nothing is written before the queue reaches it");
    let call = h.next_call();
    check(matches!(call, Some(ModuleCall::Event { ctrl: 3, event: 2, .. })), "the unbound onProgress is skipped, its values written; the bound onSearchComplete follows its own");
    check(prop(&h, "ProgressCurrent") == "1" && prop(&h, "ResultCount") == "1" && prop(&h, "Busy") == "false", "each event's values are written as it leaves the queue");
    check(script.borrow().results["KB-1"] == [hit], "the search's results are the control's then");
    println!(
        "── a_knowledgebase_method_is_the_ports_and_its_events_carry_their_values ──\n  start (properties, answer), reports (values queued, written on leaving, unbound skipped, results handed back): {checks} of {checks} checks"
    );
}

#[test]
fn cancel_and_a_host_without_the_knowledge_base() {
    let script = Rc::new(RefCell::new(Script::default()));
    let mut h = host(Some(&script));
    let mut checks = 0;
    let mut check = |ok: bool, what: &str| {
        assert!(ok, "{what}");
        checks += 1;
    };

    // Cancel while an operation runs: no longer busy, onCancelled; the port is not asked to call.
    let idx = h.trees[&0].by_name("KB-1").unwrap();
    let _ = h.trees.get_mut(&0).unwrap().set(idx, "Busy", "1");
    script.borrow_mut().running = true;
    check(answer(&mut h, 2, &[]) == "1", "Cancel answers 1");
    check(script.borrow().calls.is_empty() && script.borrow().cancelled == 1, "Cancel is the host's uniform one: the port lets the operation go");
    check(prop(&h, "Busy") == "false" && queue(&h) == ["onCancelled"], "no longer busy, and onCancelled is raised");
    // Cancel with nothing running: still 1, nothing raised.
    h.work.clear();
    check(answer(&mut h, 2, &[]) == "1" && queue(&h).is_empty(), "Cancel with nothing running raises nothing");

    // A method that is not the control's own (the port leaves it) goes on to the general dispatch.
    script.borrow_mut().answers.push_back(None);
    check(answer(&mut h, 3, &[]) == "0" && script.borrow().calls.len() == 1, "the port is asked first, and IsBusy is the general dispatch's");

    // A desktop host with no Knowledge Base answers as an interpreter built without one.
    let mut bare = host(None);
    check(answer(&mut bare, 0, &["a.md", "text"]) == "0", "a method answers 0");
    check(prop(&bare, "LastError").contains("not linked into this program"), "and LastError says why");
    h.destroy(0);
    check(script.borrow().released == [0], "the port hears an instance end");
    println!("── cancel_and_a_host_without_the_knowledge_base ──\n  Cancel running and idle, the general dispatch, a host with no Knowledge Base, release: {checks} of {checks} checks");
}
