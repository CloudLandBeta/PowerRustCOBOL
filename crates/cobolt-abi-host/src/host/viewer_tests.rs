// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! DESKTOP-WASM-001 T50: what the host hands a Viewer's port, with a scripted port standing in for the
//! runner's. The port is asked first for a Viewer's own methods; a `SetProperty` that writes a Viewer's
//! property is a write with a load; a `MOVE` to its member a write without one; the window's answers go
//! to it; a quiet write keeps a pending write's value for the window. A host with no port answers as a
//! page does.

use super::*;
use cobolt_forms::xml::load_form_from_str;
use cobolt_wasm_abi::batch::{BatchWriter, PathSpec};
use cobolt_wasm_abi::manifest::{ControlEntry, FormEntry, HandlerEntry, Limits, VariantInfo};
use std::cell::RefCell;

/// What the scripted port was asked, one line each.
#[derive(Default)]
struct Script {
    asked: Vec<String>,
}

struct Port(Rc<RefCell<Script>>);

impl ViewerPort for Port {
    fn call(&mut self, _inst: u32, v: &mut dyn ViewerAccess, method: &str, args: &[&str]) -> Option<Option<String>> {
        self.0.borrow_mut().asked.push(format!("call {} {method} {args:?}", v.name()));
        if !method.eq_ignore_ascii_case("AppendMarkdown") {
            // Not one of the Viewer's own (`GetProperty`, `SetProperty`): the general dispatch answers it.
            return None;
        }
        v.set("Progress", "100");
        v.event("onLoaded");
        v.event("onError");
        Some(Some("m1".into()))
    }
    fn written(&mut self, _inst: u32, v: &mut dyn ViewerAccess, prop: &str, load: bool) {
        let value = v.get(prop);
        self.0.borrow_mut().asked.push(format!("written {} {prop}={value} load={load}", v.name()));
        if load {
            // As a failed load puts back the document still on screen.
            v.put_quiet(prop, "kept.md");
        }
    }
    fn input(&mut self, _inst: u32, v: &mut dyn ViewerAccess, prop: &str, value: &str) -> bool {
        self.0.borrow_mut().asked.push(format!("input {} {prop}={value}", v.name()));
        prop == "_SaveAsAnswer"
    }
    fn release(&mut self, inst: u32) {
        self.0.borrow_mut().asked.push(format!("release {inst}"));
    }
}

const NAMES: [&str; 5] = ["AppendMarkdown", "GetProperty", "SetProperty", "Zoom", "Caption"];

/// A desktop host with `VWR-1`, bound to `onLoaded` and not to `onError`, and `LBL-1`.
fn host(port: Option<&Rc<RefCell<Script>>>) -> Host {
    let form = load_form_from_str(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<Form name="main-form" title="Documents" width="640" height="480" index-base="1">
  <Control id="VWR-1" type="Viewer" x="0" y="0" w="320" h="240"></Control>
  <Control id="LBL-1" type="Label" x="0" y="250" w="80" h="32"><Property name="Caption">before</Property></Control>
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
        events: ["onClick", "onLoaded", "onError"].iter().map(|e| e.to_string()).collect(),
        forms: vec![FormEntry {
            id: 0,
            name: "main-form".into(),
            main: true,
            instances: 1,
            region_size: 64,
            controls: vec![ControlEntry { id: 3, name: "VWR-1".into(), kind: "Viewer".into() }, ControlEntry { id: 4, name: "LBL-1".into(), kind: "Label".into() }],
            handlers: vec![HandlerEntry { ctrl: 3, event: 1, entry: 0 }],
            layers: vec![],
        }],
        builtins: vec![],
        imports: vec![],
        sizes: Default::default(),
    };
    let mut h = Host::new(manifest, HostOptions { desktop: true, ..HostOptions::default() });
    if let Some(p) = port {
        h.viewer = Some(Box::new(Port(Rc::clone(p))));
    }
    h.add_form(form).unwrap();
    h.create_tree(0, 0, None, Vec::new()).expect("the tree builds");
    h
}

fn answer(h: &mut Host, ctrl: u32, name: u32, args: &[&str]) -> String {
    let mut w = BatchWriter::new();
    w.call_result(&PathSpec::call(0, ctrl, name, args));
    String::from_utf8(h.query(w.as_bytes()).expect("the call answers")).unwrap()
}

fn queue(h: &Host) -> Vec<String> {
    h.work.iter().filter_map(|w| if let Work::Event(e) = w { Some(h.manifest.events[e.event as usize].clone()) } else { None }).collect()
}

#[test]
fn a_viewer_is_the_ports_and_a_quiet_write_keeps_the_windows_value() {
    let script = Rc::new(RefCell::new(Script::default()));
    let mut h = host(Some(&script));
    let asked = |s: &Rc<RefCell<Script>>| s.borrow().asked.clone();
    let mut checks = 0;
    let mut check = |ok: bool, what: &str| {
        assert!(ok, "{what}");
        checks += 1;
    };
    let idx = h.trees[&0].by_name("VWR-1").unwrap();
    let _ = h.take_changes();

    // A Viewer's own method: the port's, its writes the control's, its events queued when bound.
    check(answer(&mut h, 3, 0, &["hi", "user"]) == "m1", "the method answers what the port says");
    check(asked(&script) == ["call VWR-1 AppendMarkdown [\"hi\", \"user\"]"], "the port is asked with the control, the method and the arguments");
    check(h.control_property(0, "VWR-1", "Progress").as_deref() == Some("100"), "what the port wrote is the control's");
    check(queue(&h) == ["onLoaded"], "the bound onLoaded is queued, the unbound onError is not");
    // One the port leaves goes on to the general dispatch.
    check(answer(&mut h, 3, 1, &["Progress"]) == "100", "GetProperty is the general dispatch's");
    check(asked(&script).len() == 2, "and the port was asked first");

    // SetProperty on the Viewer: a write with a load; a failed load's quiet restore keeps, for the
    // window, the value the program wrote, in its place among the pending writes.
    let _ = h.take_changes();
    answer(&mut h, 3, 2, &["Source", "doc.md"]);
    check(asked(&script).last().map(String::as_str) == Some("written VWR-1 Source=doc.md load=true"), "SetProperty is a write with a load");
    check(h.trees[&0].get(idx, "Source") == "kept.md", "the quiet write is what the program reads");
    let changes = h.take_changes();
    let carried: Vec<&Change> = changes.iter().map(|(_, c)| c).filter(|c| matches!(c, Change::Command { name, .. } if name == crate::tree::ORDERED_WRITES)).collect();
    check(
        matches!(carried.as_slice(), [Change::Command { args, .. }] if args == &["Source".to_string(), "doc.md".to_string()]),
        "the window still gets the value written: an ordered write of Source = doc.md",
    );
    check(!changes.iter().any(|(_, c)| matches!(c, Change::Prop { key, .. } if key == "source")), "and no second write of Source, with the quiet value");
    // SetProperty on another control naming the Viewer's property writes the Viewer's; one of its own does not.
    answer(&mut h, 4, 2, &["VWR-1.Zoom", "150"]);
    check(asked(&script).last().map(String::as_str) == Some("written VWR-1 Zoom=150 load=true"), "LBL-1's SetProperty of VWR-1.Zoom is the Viewer's write");
    let before = asked(&script).len();
    answer(&mut h, 4, 2, &["Caption", "after"]);
    check(asked(&script).len() == before, "LBL-1's own SetProperty never reaches the port");

    // A MOVE to the Viewer's member: a write without a load.
    let mut w = BatchWriter::new();
    w.set(&PathSpec::prop(0, 3, 3), "120");
    h.flush(w.as_bytes());
    check(asked(&script).last().map(String::as_str) == Some("written VWR-1 Zoom=120 load=false"), "a MOVE to VWR-1::Zoom is a write without a load");
    let mut w = BatchWriter::new();
    w.set(&PathSpec::prop(0, 4, 4), "label");
    h.flush(w.as_bytes());
    check(asked(&script).last().map(String::as_str) == Some("written VWR-1 Zoom=120 load=false"), "a MOVE to a Label's member never reaches the port");

    // The window's answers.
    check(h.viewer_input(0, idx, "_SaveAsAnswer", "/tmp/x.md"), "an answer the port carries out is reported carried out");
    check(!h.viewer_input(0, idx, "View1Page", "2"), "another write is not");
    let lbl = h.trees[&0].by_name("LBL-1").unwrap();
    let n = asked(&script).len();
    check(!h.viewer_input(0, lbl, "_SaveAsAnswer", "/tmp/x.md") && asked(&script).len() == n, "a Label's write never reaches the port");

    // The instance ends.
    h.destroy(0);
    check(asked(&script).last().map(String::as_str) == Some("release 0"), "the port hears the instance end");
    println!(
        "── a_viewer_is_the_ports_and_a_quiet_write_keeps_the_windows_value ──\n  methods (port's, general dispatch's), SetProperty (own, another control's, by name), MOVE to a member, quiet write, window answers, release: {checks} of {checks} checks"
    );
}

#[test]
fn a_desktop_host_with_no_viewer_port_answers_as_a_page() {
    let mut h = host(None);
    let idx = h.trees[&0].by_name("VWR-1").unwrap();
    let mut checks = 0;
    let mut check = |ok: bool, what: &str| {
        assert!(ok, "{what}");
        checks += 1;
    };
    check(answer(&mut h, 3, 0, &["hi", "user"]) == "m1", "AppendMarkdown answers the new message's id");
    check(h.trees[&0].conversation(idx).is_some_and(|c| !c.is_empty()), "the conversation is the page's, kept in the tree");
    check(!h.viewer_input(0, idx, "_SaveAsAnswer", "/tmp/x.md"), "no port carries out an answer");
    println!("── a_desktop_host_with_no_viewer_port_answers_as_a_page ──\n  AppendMarkdown, the tree's conversation, an answer with no port: {checks} of {checks} checks");
}
