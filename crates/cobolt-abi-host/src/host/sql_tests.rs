// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! DESKTOP-WASM-001 T41c (ABI A43): what the host does with a SqlDatabase's methods and results,
//! with a scripted port standing in for the runner's. The port decides; the host writes the
//! properties, then raises each event bound to a handler, in the port's order. A host with no port
//! (a page) answers as it did before.

use super::*;
use cobolt_forms::xml::load_form_from_str;
use cobolt_wasm_abi::batch::{BatchWriter, PathSpec};
use cobolt_wasm_abi::manifest::{ControlEntry, FormEntry, HandlerEntry, Limits, VariantInfo};
use std::cell::RefCell;

/// What the scripted port was asked, and what it answers next.
#[derive(Default)]
struct Script {
    /// `(control, method, arguments, its Mode as the port read it)` of each call.
    calls: Vec<(String, String, Vec<String>, String)>,
    answers: VecDeque<Option<SqlEffects>>,
    running: bool,
    cancelled: usize,
    released: Vec<u32>,
}

struct Port(Rc<RefCell<Script>>);

impl SqlPort for Port {
    fn call(&mut self, _inst: u32, c: &SqlControl<'_>, method: &str, args: &[&str]) -> Option<SqlEffects> {
        let mut s = self.0.borrow_mut();
        s.calls.push((c.name.to_owned(), method.to_owned(), args.iter().map(|a| (*a).to_owned()).collect(), (c.prop)("Mode")));
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

const NAMES: [&str; 3] = ["Query", "Cancel", "IsBusy"];

/// A desktop host with `DB-1` (Async), whose `onQueryComplete` and `onCancelled` are bound and
/// `onQueryError` is not.
fn host(port: Option<&Rc<RefCell<Script>>>) -> Host {
    let form = load_form_from_str(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<Form name="main-form" title="Data" width="640" height="480" index-base="1">
  <Control id="DB-1" type="SqlDatabase" x="0" y="0" w="64" h="64"><Property name="Mode">Async</Property></Control>
</Form>"#,
    )
    .expect("the form loads");
    let manifest = Manifest {
        contract: None,
        remote: Vec::new(),
        capabilities: vec!["sql".into()],
        abi: 1,
        generator: "test".into(),
        project: "data".into(),
        variant: VariantInfo { multi_memory: true, conformance: false },
        limits: Limits::default(),
        names: NAMES.iter().map(|n| n.to_string()).collect(),
        events: ["onClick", "onQueryComplete", "onQueryError", "onCancelled"].iter().map(|e| e.to_string()).collect(),
        forms: vec![FormEntry {
            id: 0,
            name: "main-form".into(),
            main: true,
            instances: 1,
            region_size: 64,
            controls: vec![ControlEntry { id: 3, name: "DB-1".into(), kind: "SqlDatabase".into() }],
            handlers: vec![HandlerEntry { ctrl: 3, event: 1, entry: 0 }, HandlerEntry { ctrl: 3, event: 3, entry: 1 }],
            layers: vec![],
        }],
        builtins: vec![],
        imports: vec![],
        sizes: Default::default(),
    };
    let mut h = Host::new(manifest, HostOptions { desktop: true, ..HostOptions::default() });
    if let Some(p) = port {
        h.sql = Some(Box::new(Port(Rc::clone(p))));
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
    h.control_property(0, "DB-1", p).unwrap()
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
fn a_sqldatabase_method_is_the_ports_and_its_effects_are_carried_out_in_order() {
    let script = Rc::new(RefCell::new(Script::default()));
    let mut h = host(Some(&script));
    let mut checks = 0;
    let mut check = |ok: bool, what: &str| {
        assert!(ok, "{what}");
        checks += 1;
    };

    // A synchronous answer: the properties written, the bound event raised, the unbound one not.
    script.borrow_mut().answers.push_back(Some(SqlEffects {
        props: vec![("LastError".into(), "near \"SELEC\": syntax error".into()), ("StatusCode".into(), "1".into())],
        events: vec!["onQueryError".into(), "onQueryComplete".into()],
        answer: "3".into(),
    }));
    check(answer(&mut h, 0, &["SELECT 1"]).as_deref() == Some("3"), "the method answers what the port says");
    {
        let s = script.borrow();
        let (ctl, method, args, mode) = &s.calls[0];
        check(ctl == "DB-1" && method == "QUERY" && args == &["SELECT 1"], "the port is asked with the control, the method (upper case) and the arguments");
        check(mode == "Async", "the port reads the control's properties");
    }
    check(prop(&h, "LastError") == "near \"SELEC\": syntax error" && prop(&h, "StatusCode") == "1", "the properties are written");
    check(queue(&h) == ["onQueryComplete"], "only the bound event is raised");

    // A method that is not the database's own goes to the general dispatch: IsBusy reads Busy.
    script.borrow_mut().answers.push_back(None);
    check(answer(&mut h, 2, &[]).as_deref() == Some("0"), "IsBusy answers from Busy");

    // An asynchronous result, delivered later.
    h.sql_delivered(0, "DB-1", SqlEffects { props: vec![("Busy".into(), "0".into()), ("ResultCount".into(), "7".into())], events: vec!["onQueryComplete".into()], answer: String::new() });
    check(prop(&h, "ResultCount") == "7", "a delivered result writes its properties");
    check(queue(&h) == ["onQueryComplete", "onQueryComplete"], "and raises its event");
    h.sql_delivered(0, "NO-SUCH", SqlEffects::default());
    check(queue(&h).len() == 2, "a result for a control that is not there does nothing");
    println!(
        "── a_sqldatabase_method_is_the_ports_and_its_effects_are_carried_out_in_order ──\n  call (port asked, properties, bound events only, answer), IsBusy, a delivered result: {checks} of {checks} checks"
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
    // Cancel while a statement runs: no longer busy, onCancelled.
    let _ = h.trees.get_mut(&0).unwrap().set(1, "Busy", "1");
    script.borrow_mut().running = true;
    let _ = answer(&mut h, 1, &[]);
    check(script.borrow().cancelled == 2, "the port is asked each time");
    check(prop(&h, "Busy") == "false" && queue(&h) == ["onCancelled"], "a running statement let go: Busy cleared, onCancelled raised");
    check(script.borrow().calls.is_empty(), "Cancel is not a call of the port's");

    // The instance ends: the port lets its statements go.
    h.destroy(0);
    check(script.borrow().released.contains(&0), "the port hears the instance end");

    // A host without a port (a page): a SqlDatabase's method is not the port's.
    let mut page = host(None);
    let idx = page.trees[&0].by_name("DB-1").unwrap();
    check(page.sql_call(0, idx, "Query", &["SELECT 1"]).is_none(), "with no port, the general dispatch answers as before");
    println!("── cancel_release_and_a_host_without_a_port ──\n  Cancel idle and running, release, no port: {checks} of {checks} checks");
}
