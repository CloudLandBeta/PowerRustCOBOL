// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! The room a form has, as its program is told it (the PowerAnalyticsWeb parity job, P4; scout D1): `me::Width`,
//! `me::Height` and `me::Breakpoint` follow the surface the page measures, and `onResizing`, `onBreakpointChanged` and
//! `onResize` are raised as the desktop host raises them (form-host `host.rs`: the size pass, `mirror_layout`; its test
//! `the_program_is_told_the_layout_before_on_resize_056` gives the order `onResizing`, `onBreakpointChanged`, `onResize`).

use super::*;
use cobolt_forms::description::{describe, to_json};
use cobolt_forms::xml::load_form_from_str;
use cobolt_wasm_abi::batch::{Path, Seg};
use cobolt_wasm_abi::manifest::{ControlEntry, FormEntry, HandlerEntry, Limits, VariantInfo};

/// The PowerAnalytics breakpoints (`Compact:0;Medium:600;Expanded:1024`), a 1440 × 900 responsive form.
const FORM: &str = r##"<?xml version="1.0" encoding="UTF-8"?>
<Form name="main-form" title="Room" width="1440" height="900" responsive="true">
  <Breakpoints>
    <Breakpoint name="Compact" min-width="0" font-factor="1"/>
    <Breakpoint name="Medium" min-width="600" font-factor="1"/>
    <Breakpoint name="Expanded" min-width="1024" font-factor="1"/>
  </Breakpoints>
  <Control id="BTN-A" type="Button" x="10" y="10" w="80" h="30"></Control>
</Form>"##;

fn manifest() -> Manifest {
    Manifest {
        abi: 1,
        generator: "test".into(),
        project: "room".into(),
        variant: VariantInfo { multi_memory: true, conformance: false },
        limits: Limits::default(),
        names: vec!["Width".into(), "Height".into(), "Breakpoint".into(), "FontScale".into()],
        events: vec!["onLoad".into(), "onResizing".into(), "onBreakpointChanged".into(), "onResize".into()],
        forms: vec![FormEntry {
            id: 0,
            name: "main-form".into(),
            main: true,
            instances: 2,
            region_size: 64,
            controls: vec![ControlEntry { id: 3, name: "BTN-A".into(), kind: "Button".into() }],
            handlers: vec![
                HandlerEntry { ctrl: ctrl::FORM, event: 1, entry: Default::default() },
                HandlerEntry { ctrl: ctrl::FORM, event: 2, entry: Default::default() },
                HandlerEntry { ctrl: ctrl::FORM, event: 3, entry: Default::default() },
            ],
            layers: vec![],
        }],
        builtins: vec![],
        imports: vec![],
        sizes: Default::default(),
    }
}

fn host() -> Host {
    let mut h = Host::new(manifest(), HostOptions::default());
    h.add_description(&to_json(&describe(&load_form_from_str(FORM).unwrap()))).unwrap();
    h
}

/// `GET me::<name>` as the module sends it.
fn read(h: &mut Host, name: u32) -> String {
    let p = Path { inst: 0, ctrl: ctrl::FORM, segs: vec![Seg { name, args: vec![] }] };
    h.eval_query(&Command::Get(p)).unwrap()
}

/// The events queued for the form, by name, in order (and taken off the queue).
fn raised(h: &mut Host) -> Vec<String> {
    let ev = h.manifest.events.clone();
    let out = h.work.iter().filter_map(|w| match w { Work::Event(e) if e.ctrl == ctrl::FORM => Some(ev[e.event as usize].clone()), _ => None }).collect();
    h.work.retain(|w| !matches!(w, Work::Event(_)));
    out
}

#[test]
fn the_window_the_page_gives_is_what_the_first_handler_reads() {
    let mut h = host();
    // Nothing measured (a headless run): the stored, designed values, as before.
    h.create_tree_as(0, 0, None, Vec::new(), OpenMode::Root).unwrap();
    assert_eq!((read(&mut h, 0), read(&mut h, 1), read(&mut h, 2)), ("1440".into(), "900".into(), String::new()));
    h.destroy(0);
    // The page's main area measured 1000.4 × 700: Medium, whole pixels, before onLoad runs.
    h.set_viewport(Some((1000.4, 700.0)), None);
    h.create_tree_as(0, 0, None, Vec::new(), OpenMode::Root).unwrap();
    assert_eq!((read(&mut h, 0), read(&mut h, 1), read(&mut h, 2), read(&mut h, 3)), ("1000".into(), "700".into(), "Medium".into(), "1".into()));
    // The first measurement of the same size raises nothing (the desktop never raises for the size and breakpoint it opens in).
    assert!(!h.surface_resized(0, 1000.4, 700.0));
    assert!(raised(&mut h).is_empty());
    println!("opened at 1000.4 x 700: me::Width 1000, me::Height 700, me::Breakpoint Medium; no event");
}

#[test]
fn a_resize_raises_resizing_then_the_breakpoint_then_resize_once_it_settles() {
    let mut h = host();
    h.set_viewport(Some((1440.0, 900.0)), None);
    h.create_tree_as(0, 0, None, Vec::new(), OpenMode::Root).unwrap();
    assert_eq!(read(&mut h, 2), "Expanded");
    // Dragged to 900: still moving.
    assert!(h.surface_resized(0, 900.0, 900.0));
    assert_eq!(raised(&mut h), ["onResizing", "onBreakpointChanged"]);
    assert_eq!((read(&mut h, 0), read(&mut h, 2)), ("900".into(), "Medium".into()), "both written before their events run");
    // …to 500 (Compact), then it stops.
    assert!(h.surface_resized(0, 500.0, 820.0));
    assert_eq!(raised(&mut h), ["onResizing", "onBreakpointChanged"]);
    h.surface_settled(0);
    assert_eq!(raised(&mut h), ["onResize"], "once, when the size settles");
    assert_eq!((read(&mut h, 0), read(&mut h, 1), read(&mut h, 2)), ("500".into(), "820".into(), "Compact".into()));
    h.surface_settled(0);
    assert!(raised(&mut h).is_empty(), "nothing owed twice");
    // A wobble that ends where it began reports no new size.
    assert!(h.surface_resized(0, 520.0, 820.0));
    assert!(h.surface_resized(0, 500.0, 820.0));
    assert_eq!(raised(&mut h), ["onResizing"], "two in a row while none has run are one (a flood event, coalesced)");
    h.surface_settled(0);
    assert!(raised(&mut h).is_empty(), "settled where it was reported");
    // Within half a pixel is no change.
    assert!(!h.surface_resized(0, 500.3, 820.2));
    println!("1440 -> 900 -> 500x820: onResizing, onBreakpointChanged (Medium), onResizing, onBreakpointChanged (Compact), onResize once");
}

#[test]
fn a_form_in_the_pane_or_a_dialog_is_told_its_breakpoint_only() {
    let mut h = host();
    h.set_viewport(Some((1440.0, 900.0)), Some((1200.0, 860.0)));
    h.create_tree_as(0, 0, None, Vec::new(), OpenMode::Embedded).unwrap();
    assert_eq!((read(&mut h, 0), read(&mut h, 2)), ("1440".into(), "Expanded".into()), "me::Width stays the designed width, as a desktop form body's");
    assert!(!h.surface_resized(0, 700.0, 860.0), "no onResize is owed");
    assert_eq!(raised(&mut h), ["onBreakpointChanged"]);
    assert_eq!(read(&mut h, 2), "Medium");
    h.destroy(0);
    // A dialog opens at its designed size.
    h.create_tree_as(0, 0, None, Vec::new(), OpenMode::Modal).unwrap();
    assert_eq!(read(&mut h, 2), "Expanded");
}

#[test]
fn a_form_that_is_not_responsive_is_told_no_breakpoint() {
    let mut h = Host::new(manifest(), HostOptions::default());
    h.add_description(&to_json(&describe(&load_form_from_str(&FORM.replace(r#" responsive="true""#, "")).unwrap()))).unwrap();
    h.set_viewport(Some((800.0, 600.0)), None);
    h.create_tree_as(0, 0, None, Vec::new(), OpenMode::Root).unwrap();
    assert_eq!((read(&mut h, 0), read(&mut h, 2)), ("800".into(), String::new()));
    assert!(h.surface_resized(0, 500.0, 600.0));
    assert_eq!(raised(&mut h), ["onResizing"], "the size, never a breakpoint (the desktop lays out only a responsive form)");
}
