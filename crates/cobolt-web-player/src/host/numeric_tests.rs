// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! A NumericUpDown's `Value` on its `Step` ladder (judgement G3-1, D-G3-2): the desktop's field snaps it every
//! frame it is drawn, enabled and not ReadOnly (`render.rs` CT::NumericUpDown); the page snaps it when a handler
//! has finished, with the same rule (`fieldtext::numeric_updown_snap`).

use super::*;
use cobolt_forms::xml::load_form_from_str;
use cobolt_wasm_abi::batch::BatchWriter;
use cobolt_wasm_abi::manifest::{ControlEntry, FormEntry, HandlerEntry, Limits, VariantInfo};

fn host() -> Host {
    let nud = |id: &str, y: i32, extra: &str| {
        format!(
            r#"<Control id="{id}" type="NumericUpDown" x="10" y="{y}" w="100" h="26" {extra}><Property name="Minimum">0</Property><Property name="Maximum">300000</Property><Property name="Step">100</Property><Property name="Value">0</Property></Control>"#
        )
    };
    let xml = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<Form name="main-form" title="T" width="640" height="480" index-base="1">
  {}
  {}
  {}
  <Control id="PNL" type="Panel" x="10" y="200" w="200" h="100" visible="false">
  </Control>
  {}
</Form>"#,
        nud("NUD-A", 10, ""),
        nud("NUD-OFF", 50, r#"enabled="false""#),
        nud("NUD-RO", 90, ""),
        nud("NUD-IN", 130, r#"parent="PNL""#),
    );
    let form = load_form_from_str(&xml).expect("the form loads");
    let events: Vec<String> = ["onChange", "onValueChanged"].iter().map(|s| s.to_string()).collect();
    let controls = ["NUD-A", "NUD-OFF", "NUD-RO", "NUD-IN"]
        .iter()
        .enumerate()
        .map(|(k, n)| ControlEntry { id: 3 + k as u32, name: (*n).into(), kind: "NumericUpDown".into() })
        .collect();
    let manifest = Manifest {
        abi: 1,
        generator: "test".into(),
        project: "t".into(),
        variant: VariantInfo { multi_memory: true, conformance: false },
        limits: Limits::default(),
        names: vec!["Value".into(), "ReadOnly".into(), "Visible".into()],
        events,
        forms: vec![FormEntry {
            id: 0,
            name: "main-form".into(),
            main: true,
            instances: 1,
            region_size: 64,
            controls,
            // NUD-A's onValueChanged is bound.
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

fn value(h: &Host, name: &str) -> String {
    let t = &h.trees[&0];
    t.get(t.by_name(name).unwrap(), "Value")
}

#[test]
fn a_value_written_off_the_ladder_is_on_its_rung_once_the_handler_ends() {
    let mut h = host();
    let mut w = BatchWriter::new();
    w.set_named(0, 3, "Value", "1")
        .set_named(0, 4, "Value", "1")
        .set_named(0, 5, "ReadOnly", "true")
        .set_named(0, 5, "Value", "1")
        .set_named(0, 6, "Value", "151");
    h.apply_batch(w.as_bytes(), false);
    assert_eq!(value(&h, "NUD-A"), "1", "inside the handler the write stands, as on the desktop (no frame inside a handler)");
    h.after_status(Status::Done, 0, 0, false, None, None, None);
    assert_eq!(value(&h, "NUD-A"), "0", "a 1 on a Step-100 field is 0, as the desktop reads it after a frame");
    assert_eq!(value(&h, "NUD-OFF"), "1", "a disabled field is not snapped (desktop: `editable`)");
    assert_eq!(value(&h, "NUD-RO"), "1", "a ReadOnly field is not snapped");
    assert_eq!(value(&h, "NUD-IN"), "151", "a field inside a hidden panel is not drawn, so not snapped");
    // The program's write raised the bound onValueChanged (0 -> 1); the snap is the field's own change and raises it
    // again (1 -> 0), as the desktop's frame does after the write.
    let queued: Vec<(u32, u32)> = h.work.iter().filter_map(|w| if let Work::Event(e) = w { Some((e.ctrl, e.event)) } else { None }).collect();
    assert_eq!(queued, vec![(3, 1), (3, 1)]);
    // Shown later: snapped after the handler that showed it.
    let pnl = h.trees[&0].by_name("PNL").unwrap();
    h.trees.get_mut(&0).unwrap().set(pnl, "Visible", "true");
    h.after_status(Status::Done, 0, 0, false, None, None, None);
    assert_eq!(value(&h, "NUD-IN"), "200");
}
