// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! ABI amendment A14 (WASM-005 T14): `SET_NAMED` sets a control's property named
//! by text, and `RAISE` queues a control's event as the control would raise it —
//! after the running task, in order, and only when a handler is bound. This is
//! how a lowered class C call applies the backend's answer in the browser.

use super::*;
use cobolt_forms::xml::load_form_from_str;
use cobolt_wasm_abi::batch::BatchWriter;
use cobolt_wasm_abi::manifest::{ControlEntry, FormEntry, HandlerEntry, Limits, VariantInfo};

fn host() -> Host {
    let form = load_form_from_str(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<Form name="main-form" title="Shop" width="640" height="480" index-base="1">
  <Control id="LBL-OUT" type="Label" x="10" y="10" w="200" h="20"></Control>
</Form>"#,
    )
    .expect("the form loads");
    let events: Vec<String> = ["onClick", "onComplete", "onError"].iter().map(|s| s.to_string()).collect();
    let manifest = Manifest {
        abi: 1,
        generator: "test".into(),
        project: "shop".into(),
        variant: VariantInfo { multi_memory: true, conformance: false },
        limits: Limits::default(),
        names: vec!["Caption".into()],
        events,
        forms: vec![FormEntry {
            id: 0,
            name: "main-form".into(),
            main: true,
            instances: 1,
            region_size: 64,
            controls: vec![ControlEntry { id: 3, name: "LBL-OUT".into(), kind: "Label".into() }],
            // onComplete is bound; onError is not.
            handlers: vec![HandlerEntry { ctrl: 3, event: 1, entry: 0 }],
            layers: vec![],
        }],
        builtins: vec![],
        imports: vec![],
        sizes: Default::default(),
    };
    let mut h = Host::new(manifest, HostOptions::default());
    h.add_form(form).unwrap();
    h.create_tree(0, 0, None, Vec::new()).expect("the tree builds");
    h
}

fn queued(h: &Host) -> Vec<(u32, u32, u32, Vec<String>)> {
    h.work
        .iter()
        .filter_map(|w| if let Work::Event(e) = w { Some((e.inst, e.ctrl, e.event, e.args.clone())) } else { None })
        .collect()
}

#[test]
fn a_named_set_writes_the_property_and_a_raise_queues_only_a_bound_event() {
    let mut h = host();
    let mut w = BatchWriter::new();
    w.set_named(0, 3, "Caption", "order 9 made").raise(0, 3, "onError", "").raise(0, 3, "onComplete", "").raise(0, 3, "onComplete", "again");
    assert!(h.apply_batch(w.as_bytes(), false).is_none());
    let idx = h.trees[&0].by_name("LBL-OUT").unwrap();
    assert_eq!(h.trees[&0].get(idx, "Caption"), "order 9 made", "the property the name named");
    // onError has no handler: not delivered. onComplete twice, in order, its argument kept.
    assert_eq!(queued(&h), vec![(0, 3, 1, vec![]), (0, 3, 1, vec!["again".to_string()])]);
    assert!(errors(&h).is_empty(), "{:?}", errors(&h));
}

#[test]
fn a_raise_on_something_that_is_not_a_control_is_refused() {
    let mut h = host();
    let mut w = BatchWriter::new();
    w.raise(0, ctrl::APP, "onComplete", "").raise(0, 99, "onComplete", "");
    h.apply_batch(w.as_bytes(), false);
    assert!(queued(&h).is_empty());
    assert_eq!(errors(&h).len(), 2, "{:?}", errors(&h));
}

fn errors(h: &Host) -> Vec<String> {
    h.effects.iter().filter_map(|e| if let Effect::RuntimeError(r) = e { Some(r.message.clone()) } else { None }).collect()
}
