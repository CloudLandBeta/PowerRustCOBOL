// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! The host's half of a Hyperlink click (WASM-006 T10, D1, D2): `Cancel` reset, `onClick` raised with the
//! modifier keys, `onBeforeNavigate` with `target⇥url` after its handler, and the navigation after that —
//! each step waiting behind the handler before it, and a `Cancel` left true by either stopping it. The
//! module is played by the test: each event call is completed by hand, after writing what its handler
//! would have written.

use super::*;
use crate::hyperlink::{plan, Click, LinkProps, NavPlan, Platform, Policy};
use cobolt_forms::xml::load_form_from_str;
use cobolt_wasm_abi::codes::status;
use cobolt_wasm_abi::manifest::{ControlEntry, FormEntry, HandlerEntry, Limits, VariantInfo};

const EVENTS: [&str; 3] = ["onClick", "onBeforeNavigate", "onNavigated"];

fn host(bound: &[&str]) -> Host {
    let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<Form name="main-form" title="Demo" width="640" height="480" index-base="1">
  <Control id="LNK-1" type="Hyperlink" x="10" y="10" w="120" h="20"><Property name="Url">https://example.com/</Property><Property name="Target">NewTab</Property></Control>
</Form>"#;
    let form = load_form_from_str(xml).expect("the form loads");
    let events: Vec<String> = EVENTS.iter().map(|s| s.to_string()).collect();
    let handlers = bound
        .iter()
        .enumerate()
        .map(|(n, e)| HandlerEntry {
            ctrl: 3,
            event: events.iter().position(|x| x == e).unwrap() as u32,
            entry: n as u32,
        })
        .collect();
    let manifest = Manifest {
        abi: 1,
        generator: "test".into(),
        project: "demo".into(),
        variant: VariantInfo {
            multi_memory: true,
            conformance: false,
        },
        limits: Limits::default(),
        names: vec![],
        events,
        forms: vec![FormEntry {
            id: 0,
            name: "main-form".into(),
            main: true,
            instances: 2,
            region_size: 64,
            controls: vec![ControlEntry {
                id: 3,
                name: "LNK-1".into(),
                kind: "Hyperlink".into(),
            }],
            handlers,
            layers: vec![],
        }],
        builtins: vec![],
        imports: vec![],
        sizes: Default::default(),
    };
    let mut h = Host::new(manifest, HostOptions::default());
    h.add_form(form).unwrap();
    h.create_tree(1, 0, None, Vec::new())
        .expect("the tree builds");
    h
}

fn the_plan() -> NavPlan {
    plan(
        &LinkProps {
            url: "https://example.com/".into(),
            target: "NewTab".into(),
            ..Default::default()
        },
        Click::default(),
        Platform::Other,
        &Policy::default(),
    )
    .unwrap()
    .unwrap()
}

/// Run the host until it has nothing to do; `handler` plays each event's handler (it gets the event's
/// name and arguments and may write the tree). Returns the events delivered, as `name(args)`.
fn run(h: &mut Host, mut handler: impl FnMut(&mut Host, &str, &[String])) -> Vec<String> {
    let mut seen = Vec::new();
    while let Some(call) = h.next_call() {
        if let ModuleCall::Event {
            event, args, argc, ..
        } = &call
        {
            let name = h.manifest.events[*event as usize].clone();
            let args: Vec<String> = cobolt_wasm_abi::batch::decode_event_args(args, *argc as usize)
                .unwrap()
                .into_iter()
                .map(str::to_owned)
                .collect();
            seen.push(format!("{name}({})", args.join("|")));
            handler(h, &name, &args);
        }
        h.complete(
            &call,
            CallResult::Status {
                status: status::DONE,
                task: 0,
            },
        );
    }
    seen
}

fn ctl(h: &Host) -> usize {
    h.trees[&1].by_name("LNK-1").unwrap()
}

fn followed(h: &mut Host) -> Vec<NavPlan> {
    h.take_effects()
        .into_iter()
        .filter_map(|e| {
            if let Effect::FollowLink { plan, .. } = e {
                Some(plan)
            } else {
                None
            }
        })
        .collect()
}

#[test]
fn no_cancel_raises_both_events_in_order_and_then_follows() {
    let mut h = host(&["onClick", "onBeforeNavigate"]);
    let c = ctl(&h);
    h.ui_set(1, c, "Cancel", "1");
    h.link_click(
        1,
        c,
        the_plan(),
        Click {
            ctrl: true,
            ..Default::default()
        },
    );
    let seen = run(&mut h, |_, _, _| {});
    println!("{seen:?}");
    assert_eq!(
        seen,
        [
            "onClick(Ctrl)",
            "onBeforeNavigate(NewTab\thttps://example.com/)"
        ]
    );
    assert_eq!(h.trees[&1].get(c, "Cancel"), "0", "reset before onClick");
    assert_eq!(followed(&mut h), vec![the_plan()]);
}

#[test]
fn cancel_set_in_on_click_stops_before_on_before_navigate() {
    let mut h = host(&["onClick", "onBeforeNavigate"]);
    let c = ctl(&h);
    h.link_click(1, c, the_plan(), Click::default());
    let seen = run(&mut h, |h, name, _| {
        if name == "onClick" {
            h.ui_set(1, c, "Cancel", "1");
        }
    });
    assert_eq!(seen, ["onClick()"]);
    assert!(followed(&mut h).is_empty());
}

#[test]
fn cancel_set_in_on_before_navigate_stops_the_navigation() {
    let mut h = host(&["onClick", "onBeforeNavigate"]);
    let c = ctl(&h);
    h.link_click(1, c, the_plan(), Click::default());
    let seen = run(&mut h, |h, name, _| {
        if name == "onBeforeNavigate" {
            h.ui_set(1, c, "Cancel", "1");
        }
    });
    assert_eq!(seen.len(), 2, "{seen:?}");
    assert!(followed(&mut h).is_empty());
}

#[test]
fn a_link_with_only_on_before_navigate_bound_still_walks_the_sequence() {
    let mut h = host(&["onBeforeNavigate"]);
    let c = ctl(&h);
    h.link_click(1, c, the_plan(), Click::default());
    let seen = run(&mut h, |_, _, _| {});
    assert_eq!(
        seen,
        ["onBeforeNavigate(NewTab\thttps://example.com/)"],
        "onClick has no handler and is not delivered"
    );
    assert_eq!(followed(&mut h).len(), 1);
}
