// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! DESKTOP-WASM-001 T48 (ABI A55): what the host does with an IndexedFile's PartiQL members, with a
//! scripted port standing in for the runner's. The port decides; the host writes the properties in
//! the port's order and answers the method. A method of another class never reaches the port, and a
//! host with no port (a page) answers as it did before.

use super::*;
use cobolt_forms::xml::load_form_from_str;
use cobolt_wasm_abi::batch::{BatchWriter, PathSpec};
use cobolt_wasm_abi::manifest::{ControlEntry, FormEntry, Limits, VariantInfo};
use std::cell::RefCell;

/// What the scripted port was asked, and what it answers next.
#[derive(Default)]
struct Script {
    /// `(control, method, arguments, its OpenMode as the port read it)` of each call.
    calls: Vec<(String, String, Vec<String>, String)>,
    answers: VecDeque<Option<PartiqlEffects>>,
    released: Vec<u32>,
}

struct Port(Rc<RefCell<Script>>);

impl PartiqlPort for Port {
    fn call(&mut self, _inst: u32, c: &PartiqlControl<'_>, method: &str, args: &[&str]) -> Option<PartiqlEffects> {
        let mut s = self.0.borrow_mut();
        s.calls.push((c.name.to_owned(), method.to_owned(), args.iter().map(|a| (*a).to_owned()).collect(), (c.prop)("OpenMode")));
        s.answers.pop_front().flatten()
    }
    fn release(&mut self, inst: u32) {
        self.0.borrow_mut().released.push(inst);
    }
}

const NAMES: [&str; 3] = ["Execute", "Fetch", "Get"];

/// A desktop host with `IDX-1` (an IndexedFile opened I-O) and `LBL-1` (a Label).
fn host(port: Option<&Rc<RefCell<Script>>>) -> Host {
    let form = load_form_from_str(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<Form name="main-form" title="Data" width="640" height="480" index-base="1">
  <Control id="IDX-1" type="IndexedFile" x="0" y="0" w="64" h="64"><Property name="OpenMode">I-O</Property></Control>
  <Control id="LBL-1" type="Label" x="0" y="80" w="64" h="24"><Property name="Text">x</Property></Control>
</Form>"#,
    )
    .expect("the form loads");
    let manifest = Manifest {
        contract: None,
        remote: Vec::new(),
        capabilities: vec!["partiql".into()],
        abi: 1,
        generator: "test".into(),
        project: "data".into(),
        variant: VariantInfo { multi_memory: true, conformance: false },
        limits: Limits::default(),
        names: NAMES.iter().map(|n| n.to_string()).collect(),
        events: vec!["onClick".to_string()],
        forms: vec![FormEntry {
            id: 0,
            name: "main-form".into(),
            main: true,
            instances: 1,
            region_size: 64,
            controls: vec![
                ControlEntry { id: 3, name: "IDX-1".into(), kind: "IndexedFile".into() },
                ControlEntry { id: 4, name: "LBL-1".into(), kind: "Label".into() },
            ],
            handlers: vec![],
            layers: vec![],
        }],
        builtins: vec![],
        imports: vec![],
        sizes: Default::default(),
    };
    let mut h = Host::new(manifest, HostOptions { desktop: true, ..HostOptions::default() });
    if let Some(p) = port {
        h.partiql = Some(Box::new(Port(Rc::clone(p))));
    }
    h.add_form(form).unwrap();
    h.create_tree(0, 0, None, Vec::new()).expect("the tree builds");
    h
}

fn answer(h: &mut Host, ctrl: u32, name: u32, args: &[&str]) -> Option<String> {
    let mut w = BatchWriter::new();
    w.call_result(&PathSpec::call(0, ctrl, name, args));
    h.query(w.as_bytes()).ok().map(|b| String::from_utf8(b).unwrap())
}

fn prop(h: &Host, ctl: &str, p: &str) -> String {
    h.control_property(0, ctl, p).unwrap()
}

#[test]
fn an_indexedfile_member_is_the_ports_and_its_properties_are_written_in_order() {
    let script = Rc::new(RefCell::new(Script::default()));
    let mut h = host(Some(&script));
    let mut checks = 0;
    let mut check = |ok: bool, what: &str| {
        assert!(ok, "{what}");
        checks += 1;
    };

    // A statement: the properties written in the port's order (a property written twice keeps the
    // last value), and the count answered.
    script.borrow_mut().answers.push_back(Some(PartiqlEffects {
        props: vec![
            ("CUST-NAME".into(), "Ana".into()),
            ("CUST-NAME".into(), String::new()),
            ("ResultCount".into(), "2".into()),
            ("StatusCode".into(), "00".into()),
            ("LastError".into(), String::new()),
        ],
        answer: "2".into(),
    }));
    check(answer(&mut h, 3, 0, &["SELECT * FROM CUSTOMERS WHERE CUST-ID > ?", "0", "N"]).as_deref() == Some("2"), "Execute answers what the port says");
    {
        let s = script.borrow();
        let (ctl, method, args, mode) = &s.calls[0];
        check(ctl == "IDX-1" && method == "EXECUTE", "the port is asked with the control and the method, upper case");
        check(args == &["SELECT * FROM CUSTOMERS WHERE CUST-ID > ?", "0", "N"], "with every argument the module sent, the kinds last");
        check(mode == "I-O", "the port reads the control's properties");
    }
    check(prop(&h, "IDX-1", "ResultCount") == "2" && prop(&h, "IDX-1", "StatusCode") == "00", "the properties are written");
    check(prop(&h, "IDX-1", "CUST-NAME").is_empty(), "in the port's order: the later write wins");

    // Fetch: a row answered.
    script.borrow_mut().answers.push_back(Some(PartiqlEffects { props: vec![("StatusCode".into(), "10".into())], answer: String::new() }));
    check(answer(&mut h, 3, 1, &[]).as_deref() == Some("") && prop(&h, "IDX-1", "StatusCode") == "10", "Fetch past the last: an empty row and 10");

    // A member the port does not take goes to the general dispatch.
    script.borrow_mut().answers.push_back(None);
    let asked = script.borrow().calls.len();
    let _ = answer(&mut h, 3, 2, &["1", "N"]);
    check(script.borrow().calls.len() == asked + 1, "the port is asked first, and may pass");

    // Another class never reaches the port.
    let asked = script.borrow().calls.len();
    let _ = answer(&mut h, 4, 2, &["1"]);
    check(script.borrow().calls.len() == asked, "a Label's Get is not the port's");

    // The instance ends: the port lets what it kept go.
    h.destroy(0);
    check(script.borrow().released.contains(&0), "the port hears the instance end");

    // A host without a port (a page): an IndexedFile's member is not the port's.
    let mut page = host(None);
    let idx = page.trees[&0].by_name("IDX-1").unwrap();
    check(page.partiql_call(0, idx, "Execute", &["SELECT 1"]).is_none(), "with no port, the general dispatch answers as before");
    println!(
        "── an_indexedfile_member_is_the_ports_and_its_properties_are_written_in_order ──\n  Execute (port asked, arguments, properties in order, answer), Fetch, a member the port passes, another class, release, no port: {checks} of {checks} checks"
    );
}
