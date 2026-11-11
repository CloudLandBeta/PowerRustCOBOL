// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! 2.0.413: a call to the Backend made by a Timer's `onTick` handler is an automatic refresh, which the loading effect
//! ignores; a call made by a user's click, or by the form's load, is not.

use super::*;
use cobolt_forms::xml::load_form_from_str;
use cobolt_wasm_abi::manifest::{ControlEntry, FormEntry, HandlerEntry, Limits, VariantInfo};

/// One form with a Timer (`onTick`), a Button (`onClick`) and a form-level `onLoad`, open as instance 4.
fn host() -> Host {
    let form = load_form_from_str(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<Form name="main-form" title="main-form" width="640" height="480" index-base="1">
  <Control id="TMR-REFRESH" type="Timer" x="10" y="10" w="48" h="48"></Control>
  <Control id="BTN-GO" type="Button" x="10" y="80" w="100" h="28"></Control>
</Form>"#,
    )
    .expect("the form loads");
    let manifest = Manifest {
        contract: None,
        remote: Vec::new(),
        abi: 1,
        generator: "test".into(),
        project: "shop".into(),
        variant: VariantInfo { multi_memory: true, conformance: false },
        limits: Limits::default(),
        names: vec!["Caption".into()],
        events: ["onLoad", "onTick", "onClick"].iter().map(|s| s.to_string()).collect(),
        forms: vec![FormEntry {
            id: 0,
            name: "main-form".into(),
            main: true,
            instances: 1,
            region_size: 64,
            controls: vec![
                ControlEntry { id: 3, name: "TMR-REFRESH".into(), kind: "Timer".into() },
                ControlEntry { id: 4, name: "BTN-GO".into(), kind: "Button".into() },
            ],
            handlers: vec![
                HandlerEntry { ctrl: 3, event: 1, entry: 0 },
                HandlerEntry { ctrl: 4, event: 2, entry: 1 },
                HandlerEntry { ctrl: ctrl::FORM, event: 0, entry: 2 },
            ],
            layers: vec![],
        }],
        builtins: vec![],
        imports: vec![],
        sizes: Default::default(),
    };
    let mut h = Host::new(manifest, HostOptions::default());
    h.add_form(form).unwrap();
    h.create_tree(4, 0, None, Vec::new()).unwrap();
    h
}

/// Make a request from inside the handler of (`ctrl_id`, `event`), the way a module call does: the flight names the event.
fn request_from(h: &mut Host, ctrl_id: u32, event: u32) -> u32 {
    let desc = h.describe_event(4, ctrl_id, event);
    h.inflight = Some(InFlight { inst: Some(4), opening: None, wait: None, task: None, event_desc: Some(desc) });
    let id = h.remote(b"{}").expect("a small frame fits");
    h.inflight = None;
    id
}

#[test]
fn a_call_made_by_a_timers_tick_is_automatic_and_one_made_by_a_click_or_the_load_is_not() {
    let mut h = host();
    let tick = request_from(&mut h, 3, 1);
    let click = request_from(&mut h, 4, 2);
    let load = request_from(&mut h, ctrl::FORM, 0);
    assert_eq!(h.describe_event(4, 3, 1).1, "TMR-REFRESH onTick", "the text a tick's handler run is described by");
    assert!(h.take_automatic(tick), "the Timer's tick is an automatic refresh");
    assert!(!h.take_automatic(tick), "it is asked once, when the request goes out");
    assert!(!h.take_automatic(click), "a click is the user's");
    assert!(!h.take_automatic(load), "the form's load is the initial load");
    println!("automatic: tick request {tick} yes (once), click request {click} no, load request {load} no");
}

#[test]
fn only_the_word_ontick_ends_a_timers_description() {
    let cases = [
        ("onTick", true),
        ("TMR-REFRESH onTick", true),
        ("tmr ONTICK", true),
        ("BTN-GO onClick", false),
        ("onLoad", false),
        ("TMR-REFRESH onTickle", false),
        ("onTick BTN-GO", false),
        ("", false),
    ];
    for (text, want) in cases {
        assert_eq!(is_timer_tick(text), want, "{text:?}");
    }
    println!("is_timer_tick: {} descriptions read as expected", cases.len());
}
