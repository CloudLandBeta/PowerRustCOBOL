// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! WASM-014 T9 (R2, R6): the player raises `onBackendVersion` (a hello that is not ok) and `onBackendWarning` (a reply with a warning) on the form the page started
//! with, only when a handler is bound to it, carrying what the backend said.

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
    let events: Vec<String> = ["onLoad", ON_BACKEND_VERSION, ON_BACKEND_WARNING].iter().map(|s| s.to_string()).collect();
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
    let main_handlers = if bound { vec![HandlerEntry { ctrl: ctrl::FORM, event: 1, entry: 0 }, HandlerEntry { ctrl: ctrl::FORM, event: 2, entry: 2 }] } else { vec![] };
    let manifest = Manifest {
        contract: None,
        remote: Vec::new(),
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

fn queued(h: &Host) -> Vec<(u32, u32, u32, Vec<String>)> {
    h.work.iter().filter_map(|w| if let Work::Event(e) = w { Some((e.inst, e.ctrl, e.event, e.args.clone())) } else { None }).collect()
}

#[test]
fn a_hello_that_is_not_ok_and_a_warning_go_to_the_main_form_with_what_the_backend_said() {
    let mut h = host(true);
    h.create_tree(4, 0, None, Vec::new()).unwrap();
    h.create_tree(5, 1, Some(4), Vec::new()).unwrap();
    assert!(h.backend_version("warn", "this application is older than the backend", "1.2.0"));
    assert!(h.backend_warning("CHARGE", "CHARGE is deprecated: use CHARGE-V2"));
    let q = queued(&h);
    println!("queued on the main form: {q:?}");
    assert_eq!(q[0], (4, ctrl::FORM, 1, vec!["warn".to_string(), "this application is older than the backend".into(), "1.2.0".into()]), "onBackendVersion on the main form, not the child it opened");
    assert_eq!(q[1], (4, ctrl::FORM, 2, vec!["CHARGE".to_string(), "CHARGE is deprecated: use CHARGE-V2".into()]));
    assert_eq!(q.len(), 2);
}

#[test]
fn nothing_is_raised_unbound_or_with_no_form_open() {
    let mut unbound = host(false);
    unbound.create_tree(4, 0, None, Vec::new()).unwrap();
    assert!(!unbound.backend_version("refused", "update", "2.0.0"));
    assert!(!unbound.backend_warning("CHARGE", "w"));
    assert!(queued(&unbound).is_empty());
    let mut empty = host(true);
    assert!(!empty.backend_version("warn", "m", ""));
}
