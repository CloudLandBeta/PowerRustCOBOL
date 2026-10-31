// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! WASM-016 R21: when the connection gives up, the player raises `onBackendUnreachable` on the form the page started
//! with — once, and only when a handler is bound to it.

use super::*;
use cobolt_forms::xml::load_form_from_str;
use cobolt_wasm_abi::manifest::{ControlEntry, FormEntry, HandlerEntry, Limits, VariantInfo};

fn form(name: &str) -> cobolt_forms::model::Form {
    load_form_from_str(&format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<Form name="{name}" title="{name}" width="640" height="480" index-base="1">
  <Control id="LBL-OUT" type="Label" x="10" y="10" w="200" h="20"></Control>
</Form>"#
    ))
    .expect("the form loads")
}

/// `main-form` and `child-form`; `bound` says whether the main form has a handler for `onBackendUnreachable`.
fn host(bound: bool) -> Host {
    let events: Vec<String> = ["onLoad", ON_BACKEND_UNREACHABLE].iter().map(|s| s.to_string()).collect();
    let entry = |id: u32, name: &str, main: bool, handlers: Vec<HandlerEntry>| FormEntry {
        id,
        name: name.into(),
        main,
        instances: 2,
        region_size: 64,
        controls: vec![ControlEntry { id: 3, name: "LBL-OUT".into(), kind: "Label".into() }],
        handlers,
        layers: vec![],
    };
    let main_handlers = if bound { vec![HandlerEntry { ctrl: ctrl::FORM, event: 1, entry: 0 }] } else { vec![] };
    let manifest = Manifest {
        abi: 1,
        generator: "test".into(),
        project: "shop".into(),
        variant: VariantInfo { multi_memory: true, conformance: false },
        limits: Limits::default(),
        names: vec!["Caption".into()],
        events,
        // The child binds the event too: it is still the main form's to hear.
        forms: vec![entry(0, "main-form", true, main_handlers), entry(1, "child-form", false, vec![HandlerEntry { ctrl: ctrl::FORM, event: 1, entry: 1 }])],
        builtins: vec![],
        imports: vec![],
        sizes: Default::default(),
    };
    let mut h = Host::new(manifest, HostOptions::default());
    h.add_form(form("main-form")).unwrap();
    h.add_form(form("child-form")).unwrap();
    h
}

fn queued(h: &Host) -> Vec<(u32, u32, u32)> {
    h.work.iter().filter_map(|w| if let Work::Event(e) = w { Some((e.inst, e.ctrl, e.event)) } else { None }).collect()
}

#[test]
fn the_event_goes_to_the_form_the_page_started_with_and_only_when_bound() {
    let mut h = host(true);
    h.create_tree(4, 0, None, Vec::new()).unwrap();
    h.create_tree(5, 1, Some(4), Vec::new()).unwrap();
    assert!(h.backend_unreachable(), "queued on the main form");
    assert_eq!(queued(&h), vec![(4, ctrl::FORM, 1)], "the main form's instance, not the child it opened");

    let mut unbound = host(false);
    unbound.create_tree(4, 0, None, Vec::new()).unwrap();
    unbound.create_tree(5, 1, Some(4), Vec::new()).unwrap();
    assert!(!unbound.backend_unreachable(), "no handler on the main form: nothing is raised");
    assert!(queued(&unbound).is_empty());

    let mut empty = host(true);
    assert!(!empty.backend_unreachable(), "no form open: nothing to raise it on");
    println!("raised on instance 4 (main-form) with child 5 open; not raised unbound or with no form");
}

/// A standalone window runs a form no instance of its page opened (WASM-011): it is the one that hears it.
#[test]
fn a_page_that_started_with_another_form_raises_it_there() {
    let mut h = host(true);
    h.create_tree(7, 1, None, Vec::new()).unwrap();
    assert!(h.backend_unreachable());
    assert_eq!(queued(&h), vec![(7, ctrl::FORM, 1)]);
}
