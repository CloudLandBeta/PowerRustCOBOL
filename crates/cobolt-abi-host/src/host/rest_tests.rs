// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! DESKTOP-WASM-001 T42b (ABI A46): what the host does with a RestClient's methods and endings, with
//! a scripted port standing in for the runner's. The port decides; the host writes the properties,
//! then raises each event bound to a handler, in the port's order. A host with no port (a page)
//! answers as it did before.

use super::*;
use cobolt_forms::xml::load_form_from_str;
use cobolt_wasm_abi::batch::{BatchWriter, PathSpec};
use cobolt_wasm_abi::manifest::{ControlEntry, FormEntry, HandlerEntry, Limits, VariantInfo};
use std::cell::RefCell;

/// What the scripted port was asked, and what it answers next.
#[derive(Default)]
struct Script {
    /// `(control, method, arguments, its BaseURL as the port read it)` of each call.
    calls: Vec<(String, String, Vec<String>, String)>,
    answers: VecDeque<Option<RestEffects>>,
    running: bool,
    cancelled: usize,
    released: Vec<u32>,
}

struct Port(Rc<RefCell<Script>>);

impl RestPort for Port {
    fn call(&mut self, _inst: u32, c: &RestControl<'_>, method: &str, args: &[&str]) -> Option<RestEffects> {
        let mut s = self.0.borrow_mut();
        s.calls.push((c.name.to_owned(), method.to_owned(), args.iter().map(|a| (*a).to_owned()).collect(), (c.prop)("BaseURL")));
        s.answers.pop_front().flatten()
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

const NAMES: [&str; 3] = ["Get", "Cancel", "IsBusy"];

/// A desktop host with `REST-1`, whose `onComplete`, `onCancelled` and `onTimeout` are bound and
/// `onError` is not.
fn host(port: Option<&Rc<RefCell<Script>>>) -> Host {
    let form = load_form_from_str(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<Form name="main-form" title="Web" width="640" height="480" index-base="1">
  <Control id="REST-1" type="RestClient" x="0" y="0" w="64" h="64"><Property name="BaseURL">https://api.example.com/v1</Property></Control>
</Form>"#,
    )
    .expect("the form loads");
    let manifest = Manifest {
        contract: None,
        remote: Vec::new(),
        capabilities: vec!["http".into()],
        abi: 1,
        generator: "test".into(),
        project: "web".into(),
        variant: VariantInfo { multi_memory: true, conformance: false },
        limits: Limits::default(),
        names: NAMES.iter().map(|n| n.to_string()).collect(),
        events: ["onClick", "onComplete", "onError", "onCancelled", "onTimeout"].iter().map(|e| e.to_string()).collect(),
        forms: vec![FormEntry {
            id: 0,
            name: "main-form".into(),
            main: true,
            instances: 1,
            region_size: 64,
            controls: vec![ControlEntry { id: 3, name: "REST-1".into(), kind: "RestClient".into() }],
            handlers: vec![
                HandlerEntry { ctrl: 3, event: 1, entry: 0 },
                HandlerEntry { ctrl: 3, event: 3, entry: 1 },
                HandlerEntry { ctrl: 3, event: 4, entry: 2 },
            ],
            layers: vec![],
        }],
        builtins: vec![],
        imports: vec![],
        sizes: Default::default(),
    };
    let mut h = Host::new(manifest, HostOptions { desktop: true, ..HostOptions::default() });
    if let Some(p) = port {
        h.rest = Some(Box::new(Port(Rc::clone(p))));
    }
    h.add_form(form).unwrap();
    h.create_tree(0, 0, None, Vec::new()).expect("the tree builds");
    h
}

fn answer(h: &mut Host, name: u32, args: &[&str]) -> Option<String> {
    let mut w = BatchWriter::new();
    w.call_result(&PathSpec::call(0, 3, name, args));
    h.query(w.as_bytes()).ok().map(|b| String::from_utf8(b).unwrap())
}

fn prop(h: &Host, p: &str) -> String {
    h.control_property(0, "REST-1", p).unwrap()
}

/// The events in the queue, by name.
fn queue(h: &Host) -> Vec<String> {
    h.work
        .iter()
        .filter_map(|w| match w {
            Work::Event(e) => Some(h.manifest.events[e.event as usize].clone()),
            _ => None,
        })
        .collect()
}

#[test]
fn a_restclient_method_is_the_ports_and_its_effects_are_carried_out_in_order() {
    let script = Rc::new(RefCell::new(Script::default()));
    let mut h = host(Some(&script));
    let mut checks = 0;
    let mut check = |ok: bool, what: &str| {
        assert!(ok, "{what}");
        checks += 1;
    };

    // A request that ran now: the properties written, the method answers the body.
    script.borrow_mut().answers.push_back(Some(RestEffects {
        lines: Vec::new(),
        props: vec![("ResponseBody".into(), "{\"id\":7}".into()), ("StatusCode".into(), "200".into())],
        events: Vec::new(),
        answer: "{\"id\":7}".into(),
    }));
    check(answer(&mut h, 0, &["users/7"]).as_deref() == Some("{\"id\":7}"), "the method answers what the port says");
    {
        let s = script.borrow();
        let (ctl, method, args, base) = &s.calls[0];
        check(ctl == "REST-1" && method == "GET" && args == &["users/7"], "the port is asked with the control, the method (upper case) and the arguments");
        check(base == "https://api.example.com/v1", "the port reads the control's properties");
    }
    check(prop(&h, "ResponseBody") == "{\"id\":7}" && prop(&h, "StatusCode") == "200", "the properties are written");
    check(queue(&h).is_empty(), "a request that ran now raises nothing");

    // A request started on its own thread: Busy, then an error (unbound) and an ending (bound).
    script.borrow_mut().answers.push_back(Some(RestEffects { lines: Vec::new(), props: vec![("Busy".into(), "1".into())], events: Vec::new(), answer: String::new() }));
    check(answer(&mut h, 0, &[]).as_deref() == Some(""), "a request on its own thread answers nothing yet");
    check(answer(&mut h, 2, &[]).as_deref() == Some("1"), "IsBusy is not the port's: it reads Busy");
    h.rest_delivered(0, "REST-1", RestEffects { lines: Vec::new(), props: vec![("LastError".into(), "refused".into()), ("Busy".into(), "0".into())], events: vec!["onError".into()], answer: String::new() });
    check(prop(&h, "LastError") == "refused" && queue(&h).is_empty(), "an ending writes its properties; an unbound event is not raised");
    h.rest_delivered(0, "REST-1", RestEffects { lines: Vec::new(), props: vec![("StatusCode".into(), "404".into())], events: vec!["onComplete".into()], answer: String::new() });
    check(prop(&h, "StatusCode") == "404" && queue(&h) == ["onComplete"], "a bound event is raised");
    h.rest_delivered(0, "NO-SUCH", RestEffects::default());
    check(queue(&h).len() == 1, "an ending for a control that is not there does nothing");
    println!(
        "── a_restclient_method_is_the_ports_and_its_effects_are_carried_out_in_order ──\n  call (port asked, properties, answer), a request on its own thread, IsBusy, delivered endings: {checks} of {checks} checks"
    );
}

#[test]
fn cancel_release_and_a_host_without_a_port() {
    let script = Rc::new(RefCell::new(Script::default()));
    let mut h = host(Some(&script));
    let mut checks = 0;
    let mut check = |ok: bool, what: &str| {
        assert!(ok, "{what}");
        checks += 1;
    };

    // Cancel with nothing running: nothing happens.
    check(answer(&mut h, 1, &[]).is_some() && queue(&h).is_empty(), "Cancel with nothing running raises nothing");
    // Cancel while a request runs: no longer busy, onCancelled.
    let _ = h.trees.get_mut(&0).unwrap().set(1, "Busy", "1");
    script.borrow_mut().running = true;
    let _ = answer(&mut h, 1, &[]);
    check(script.borrow().cancelled == 2, "the port is asked each time");
    check(prop(&h, "Busy") == "false" && queue(&h) == ["onCancelled"], "a running request let go: Busy cleared, onCancelled raised");
    check(script.borrow().calls.is_empty(), "Cancel is not a call of the port's");

    // The instance ends: the port lets its requests go.
    h.destroy(0);
    check(script.borrow().released.contains(&0), "the port hears the instance end");

    // A host without a port (a page): a RestClient's method is not the port's.
    let mut page = host(None);
    let idx = page.trees[&0].by_name("REST-1").unwrap();
    check(page.rest_call(0, idx, "Get", &["users/7"]).is_none(), "with no port, the general dispatch answers as before");
    println!("── cancel_release_and_a_host_without_a_port (RestClient) ──\n  Cancel idle and running, release, no port: {checks} of {checks} checks");
}
