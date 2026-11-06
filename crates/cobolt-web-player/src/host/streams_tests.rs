// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! WASM-014 T15 (R13, R14): the host shows a pending call's progress in the controls the form declared, answers a click on the cancel
//! control, raises `onStreamItem` / `onStreamEnd` on the form that subscribed (queued behind what the instance already has), and
//! lets go of an instance's subscriptions when it closes.

use super::*;
use cobolt_forms::xml::load_form_from_str;
use cobolt_wasm_abi::manifest::{ControlEntry, FormEntry, HandlerEntry, Limits, VariantInfo};

fn form() -> cobolt_forms::model::Form {
    load_form_from_str(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<Form name="feed-form" title="feed" width="640" height="480" index-base="1">
  <Control id="PRG-1" type="ProgressBar" x="10" y="10" w="200" h="20"></Control>
  <Control id="LBL-MSG" type="Label" x="10" y="40" w="200" h="20"></Control>
  <Control id="BTN-CANCEL" type="Button" x="10" y="70" w="100" h="24"></Control>
</Form>"#,
    )
    .expect("the form loads")
}

fn host() -> Host {
    let events: Vec<String> = ["onLoad", ON_STREAM_ITEM, ON_STREAM_END].iter().map(|s| s.to_string()).collect();
    let entry = FormEntry {
        id: 0,
        name: "feed-form".into(),
        main: true,
        instances: 2,
        region_size: 64,
        controls: vec![
            ControlEntry { id: 3, name: "PRG-1".into(), kind: "ProgressBar".into() },
            ControlEntry { id: 4, name: "LBL-MSG".into(), kind: "Label".into() },
            ControlEntry { id: 5, name: "BTN-CANCEL".into(), kind: "Button".into() },
        ],
        handlers: vec![
            HandlerEntry { ctrl: ctrl::FORM, event: 0, entry: 0 },
            HandlerEntry { ctrl: ctrl::FORM, event: 1, entry: 1 },
            HandlerEntry { ctrl: ctrl::FORM, event: 2, entry: 2 },
        ],
        layers: vec![],
    };
    let manifest = Manifest {
        contract: None,
        remote: Vec::new(),
        abi: 1,
        generator: "test".into(),
        project: "feed".into(),
        variant: VariantInfo { multi_memory: true, conformance: false },
        limits: Limits::default(),
        names: vec!["Caption".into()],
        events,
        forms: vec![entry],
        builtins: vec![],
        imports: vec![],
        sizes: Default::default(),
    };
    let mut h = Host::new(manifest, HostOptions::default());
    h.add_form(form()).unwrap();
    h
}

fn queued(h: &Host) -> Vec<(u32, u32, u32, Vec<String>)> {
    h.work.iter().filter_map(|w| if let Work::Event(e) = w { Some((e.inst, e.ctrl, e.event, e.args.clone())) } else { None }).collect()
}

fn prop(h: &Host, inst: u32, name: &str, prop: &str) -> Option<String> {
    let t = h.trees.get(&inst)?;
    t.ctls.iter().find(|c| c.name == name)?.props.get(prop).cloned()
}

#[test]
fn progress_frames_set_the_declared_controls() {
    let mut h = host();
    h.create_tree(4, 0, None, Vec::new()).unwrap();
    assert!(h.show_progress(9, 4, "PRG-1", "LBL-MSG", "BTN-CANCEL").is_ok());
    h.call_progress(9, 40, "reading the ledger");
    h.call_progress(9, 80, "posting");
    println!("bar {:?}, label {:?}", prop(&h, 4, "PRG-1", "value"), prop(&h, 4, "LBL-MSG", "caption"));
    assert_eq!(prop(&h, 4, "PRG-1", "value").as_deref(), Some("80"));
    assert_eq!(prop(&h, 4, "LBL-MSG", "caption").as_deref(), Some("posting"));
    assert!(queued(&h).is_empty(), "no COBOL runs for a progress frame");
    // A control the form does not have is refused, naming it.
    let e = h.show_progress(10, 4, "PRG-9", "", "").unwrap_err();
    assert!(e.contains("PRG-9"), "{e}");
    // The declaration is for that call alone.
    h.call_finished(9);
    h.call_progress(9, 99, "late");
    assert_eq!(prop(&h, 4, "PRG-1", "value").as_deref(), Some("80"));
}

#[test]
fn a_click_on_the_cancel_control_names_the_call_to_cancel() {
    let mut h = host();
    h.create_tree(4, 0, None, Vec::new()).unwrap();
    h.show_progress(9, 4, "PRG-1", "", "BTN-CANCEL").unwrap();
    let cancel = h.trees[&4].ctls.iter().position(|c| c.name == "BTN-CANCEL").unwrap();
    assert_eq!(h.cancel_clicked(4, cancel), Some(9));
    assert_eq!(h.cancel_clicked(4, 0), None);
    h.call_finished(9);
    assert_eq!(h.cancel_clicked(4, cancel), None, "spent with the call");
}

#[test]
fn stream_items_raise_on_stream_item_after_the_task() {
    let mut h = host();
    h.create_tree(4, 0, None, Vec::new()).unwrap();
    h.stream_owner(3, 4);
    // The instance already has work queued: the stream's event goes behind it.
    assert!(h.dom_event(4, 0, "onLoad", Vec::new()));
    assert!(h.stream_item(3, "{\"N\":1}", false));
    assert!(h.stream_item(3, "{\"N\":2}", true));
    assert!(h.stream_end(3, "ended"));
    let q = queued(&h);
    println!("queued: {q:?}");
    assert_eq!(q.len(), 4);
    assert_eq!((q[0].2, q[1].2, q[2].2, q[3].2), (0, 1, 1, 2), "onLoad first, then the items, then the end");
    assert_eq!(q[1].3, vec!["3".to_string(), "{\"N\":1}".into(), "N".into()]);
    assert_eq!(q[2].3[2], "Y", "the restart flag");
    assert_eq!(q[3].3, vec!["3".to_string(), "ended".into()]);
    assert!(!h.stream_item(3, "{}", false), "an ended stream raises nothing");
}

#[test]
fn closing_the_form_hands_back_its_subscriptions() {
    let mut h = host();
    h.create_tree(4, 0, None, Vec::new()).unwrap();
    h.stream_owner(3, 4);
    h.stream_owner(5, 4);
    h.destroy(4);
    let mut closed = h.take_closed_streams();
    closed.sort();
    println!("subscriptions to end: {closed:?}");
    assert_eq!(closed, [3, 5]);
    assert!(h.take_closed_streams().is_empty());
}
