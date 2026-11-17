// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! DESKTOP-WASM-001 T48 (ABI A38): what the host does with an AWS control's methods and results,
//! with a scripted port standing in for the runner's. The port decides; the host carries out, in
//! the port's order: narration, properties, rows, then each event with its values written as the
//! queue reaches it, bound or not.

use super::*;
use cobolt_forms::xml::load_form_from_str;
use cobolt_wasm_abi::batch::{BatchWriter, PathSpec};
use cobolt_wasm_abi::manifest::{ControlEntry, FormEntry, HandlerEntry, Limits, VariantInfo};
use serde_json::{json, Value};
use std::cell::RefCell;

/// What the scripted port was asked, and what it answers next.
#[derive(Default)]
struct Script {
    /// `(control, method, arguments, the form's AWS controls)` of each call.
    calls: Vec<(String, String, Vec<String>, Vec<(String, bool)>)>,
    answers: VecDeque<Option<AwsEffects>>,
    rows: HashMap<String, Vec<Value>>,
    running: bool,
    cancelled: usize,
    released: Vec<u32>,
    /// The key verbs, with how many arguments each carried — never the arguments.
    keys: Vec<(String, usize)>,
}

struct Port(Rc<RefCell<Script>>);

impl AwsPort for Port {
    fn call(&mut self, _inst: u32, c: &AwsControl<'_>, method: &str, args: &[&str]) -> Option<AwsEffects> {
        let mut s = self.0.borrow_mut();
        s.calls.push((c.name.to_owned(), method.to_owned(), args.iter().map(|a| (*a).to_owned()).collect(), c.peers.to_vec()));
        s.answers.pop_front().flatten()
    }
    fn row_value(&self, _inst: u32, ctl: &str, i: &str, field: Option<&str>) -> String {
        let s = self.0.borrow();
        let row = i.trim().parse::<usize>().ok().and_then(|n| s.rows.get(ctl)?.get(n - 1).cloned());
        match (row, field) {
            (Some(r), None) => r.to_string(),
            (Some(r), Some(f)) => r.get(f).and_then(Value::as_str).unwrap_or_default().to_owned(),
            (None, _) => String::new(),
        }
    }
    fn set_rows(&mut self, _inst: u32, ctl: &str, rows: Vec<Value>) {
        self.0.borrow_mut().rows.insert(ctl.to_owned(), rows);
    }
    fn cancel(&mut self, _inst: u32, _ctl: &str) -> bool {
        let mut s = self.0.borrow_mut();
        s.cancelled += 1;
        std::mem::take(&mut s.running)
    }
    fn credentials(&mut self, verb: &str, args: &[&str]) -> String {
        self.0.borrow_mut().keys.push((verb.to_owned(), args.len()));
        if verb == "AWS-CREDENTIALS-SET" { "Y" } else { "OK" }.to_owned()
    }
    fn release(&mut self, inst: u32) {
        self.0.borrow_mut().released.push(inst);
    }
}

const NAMES: [&str; 6] = ["PutObject", "GetRow", "GetField", "Cancel", "AWS-SET-CREDENTIALS", "AWS-CREDENTIALS-SET"];

/// A desktop host with `S3-1` (Async, on the connection Demo) and `LAM-1` (on Demo, writes allowed);
/// `S3-1` has handlers for `onObjectPut`, `onComplete` and `onCancelled`, none for `onError`.
fn host(port: Option<&Rc<RefCell<Script>>>) -> Host {
    let form = load_form_from_str(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<Form name="main-form" title="Cloud" width="640" height="480" index-base="1">
  <Control id="S3-1" type="AwsS3" x="0" y="0" w="32" h="32"><Property name="Connection">Demo</Property><Property name="Mode">Async</Property></Control>
  <Control id="LAM-1" type="AwsLambda" x="40" y="0" w="32" h="32"><Property name="Connection">Demo</Property><Property name="AllowWrite">true</Property></Control>
</Form>"#,
    )
    .expect("the form loads");
    let manifest = Manifest {
        contract: None,
        remote: Vec::new(),
        capabilities: vec!["aws".into()],
        abi: 1,
        generator: "test".into(),
        project: "cloud".into(),
        variant: VariantInfo { multi_memory: true, conformance: false },
        limits: Limits::default(),
        names: NAMES.iter().map(|n| n.to_string()).collect(),
        events: ["onClick", "onObjectPut", "onComplete", "onError", "onCancelled"].iter().map(|e| e.to_string()).collect(),
        forms: vec![FormEntry {
            id: 0,
            name: "main-form".into(),
            main: true,
            instances: 1,
            region_size: 64,
            controls: vec![
                ControlEntry { id: 3, name: "S3-1".into(), kind: "AwsS3".into() },
                ControlEntry { id: 4, name: "LAM-1".into(), kind: "AwsLambda".into() },
            ],
            handlers: vec![HandlerEntry { ctrl: 3, event: 1, entry: 0 }, HandlerEntry { ctrl: 3, event: 2, entry: 1 }, HandlerEntry { ctrl: 3, event: 4, entry: 2 }],
            layers: vec![],
        }],
        builtins: vec![],
        imports: vec![],
        sizes: Default::default(),
    };
    let mut h = Host::new(manifest, HostOptions { desktop: true, ..HostOptions::default() });
    if let Some(p) = port {
        h.aws = Some(Box::new(Port(Rc::clone(p))));
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

fn prop(h: &Host, ctl: &str, p: &str) -> String {
    h.control_property(0, ctl, p).unwrap()
}

fn shown(h: &Host) -> Vec<String> {
    h.effects.iter().filter_map(|e| if let Effect::Display(t) = e { Some(t.clone()) } else { None }).collect()
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
fn an_aws_method_is_the_ports_and_its_results_are_carried_out_in_order() {
    let script = Rc::new(RefCell::new(Script::default()));
    let mut h = host(Some(&script));
    let mut checks = 0;
    let mut check = |ok: bool, what: &str| {
        assert!(ok, "{what}");
        checks += 1;
    };

    // An asynchronous start: the port's narration, Busy, and its answer.
    script.borrow_mut().answers.push_back(Some(AwsEffects {
        lines: vec!["[aws S3-1] request: put".into()],
        props: vec![("Busy".into(), "1".into())],
        answer: "1".into(),
        ..AwsEffects::default()
    }));
    script.borrow_mut().running = true;
    check(answer(&mut h, 0, &["photos/1.jpg", "hello"]) == "1", "the method answers what the port says");
    {
        let s = script.borrow();
        let (ctl, method, args, peers) = &s.calls[0];
        check(ctl == "S3-1" && method == "PUTOBJECT" && args == &["photos/1.jpg", "hello"], "the port is asked with the control, the method and the arguments");
        check(peers == &[("Demo".to_string(), false), ("Demo".to_string(), true)], "with the form's AWS controls: their connection and AllowWrite");
    }
    let seen = (prop(&h, "S3-1", "Busy"), shown(&h));
    check(seen.0 == "true" && seen.1 == ["[aws S3-1] request: put"], &format!("Busy written and the request narrated: {seen:?}"));

    // The result: its event's values wait in the queue, then onComplete.
    h.aws_delivered(
        0,
        "S3-1",
        AwsEffects {
            lines: vec!["[aws S3-1] response: done".into()],
            events: vec![
                AwsEvent { name: "onObjectPut".into(), props: vec![("ETag".into(), "e1".into()), ("Busy".into(), "0".into())], rows: Some(vec![json!({"Key": "photos/1.jpg"})]) },
                AwsEvent { name: "onComplete".into(), ..AwsEvent::default() },
            ],
            ..AwsEffects::default()
        },
    );
    check(queue(&h) == ["apply onObjectPut", "onComplete"], "the values wait behind the queue, then onComplete");
    check(prop(&h, "S3-1", "ETag").is_empty() && prop(&h, "S3-1", "Busy") == "true", "nothing is written before the queue reaches it");
    let call = h.next_call();
    check(matches!(call, Some(ModuleCall::Event { ctrl: 3, event: 1, .. })), "the bound onObjectPut follows its values");
    check(prop(&h, "S3-1", "ETag") == "e1" && prop(&h, "S3-1", "Busy") == "false", "its values are written as it leaves the queue");
    check(script.borrow().rows["S3-1"].len() == 1, "its rows are the control's row set");
    println!(
        "── an_aws_method_is_the_ports_and_its_results_are_carried_out_in_order ──\n  start (narration, Busy, answer), result (values queued, written on leaving, event, onComplete): {checks} of {checks} checks"
    );
}

#[test]
fn rows_cancel_keys_and_a_host_without_aws() {
    let script = Rc::new(RefCell::new(Script::default()));
    let mut h = host(Some(&script));
    let mut checks = 0;
    let mut check = |ok: bool, what: &str| {
        assert!(ok, "{what}");
        checks += 1;
    };
    script.borrow_mut().rows.insert("S3-1".into(), vec![json!({"Key": "a.jpg"}), json!({"Key": "b.jpg"})]);
    check(answer(&mut h, 1, &[" 2 "]) == r#"{"Key":"b.jpg"}"#, "GetRow reads the port's row set, its index trimmed");
    check(answer(&mut h, 2, &["1", "Key"]) == "a.jpg", "GetField too");
    check(script.borrow().calls.is_empty(), "neither is an operation");

    // Cancel while an operation runs: not busy, onCancelled (bound); with none running, nothing.
    script.borrow_mut().running = true;
    let idx = h.trees[&0].by_name("S3-1").unwrap();
    let _ = h.trees.get_mut(&0).unwrap().set(idx, "Busy", "1");
    let mut w = BatchWriter::new();
    w.call(&PathSpec::call(0, 3, 3, &[])).call(&PathSpec::call(0, 3, 3, &[]));
    h.flush(w.as_bytes());
    let seen = (script.borrow().cancelled, prop(&h, "S3-1", "Busy"));
    check(seen.0 == 2 && seen.1 == "false", &format!("Cancel asks the port and clears Busy: {seen:?}"));
    check(queue(&h) == ["onCancelled"], "onCancelled once, for the operation that was running");

    // The keys: the application's AwsStatus and AwsFlag; the port sees the arguments, nothing records them.
    let mut w = BatchWriter::new();
    w.call(&PathSpec::call(0, ctrl::APP, 4, &["Demo", "AKIA-TEST", "secret-test", " "])).call(&PathSpec::call(0, ctrl::APP, 5, &["Demo"]));
    h.flush(w.as_bytes());
    check(h.app.get("awsstatus").map(String::as_str) == Some("OK") && h.app.get("awsflag").map(String::as_str) == Some("Y"), "the answers are the application's");
    check(script.borrow().keys == [("AWS-SET-CREDENTIALS".to_string(), 4), ("AWS-CREDENTIALS-SET".to_string(), 1)], "the port gets them");
    let recorded = format!("{:?}", h.effects);
    check(!recorded.contains("secret-test") && !recorded.contains("AKIA-TEST"), "no effect records a key");

    // The form closes: the port lets its instance go.
    h.destroy(0);
    check(script.borrow().released == [0], "released with its instance");

    // A host with no AWS answers as an interpreter built without it.
    let mut bare = host(None);
    check(answer(&mut bare, 0, &["x"]) == "0", "an operation answers 0");
    check(queue(&bare) == ["apply onError"], "and raises onError with its reason");
    let _ = bare.next_call();
    check(prop(&bare, "S3-1", "LastError") == "this application was built without AWS support", "written as it leaves the queue, bound or not");
    let mut w = BatchWriter::new();
    w.call(&PathSpec::call(0, ctrl::APP, 5, &["Demo"]));
    bare.flush(w.as_bytes());
    check(bare.app.get("awsflag").map(String::as_str) == Some("N"), "and holds no keys");
    println!("── rows_cancel_keys_and_a_host_without_aws ──\n  GetRow/GetField, Cancel twice, the three key verbs, release, a host without AWS: {checks} of {checks} checks");
}
