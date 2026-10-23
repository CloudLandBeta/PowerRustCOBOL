// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Form layers in the player (WASM-016 R18, R19, AC12; ABI A9; spec 091): a layer is addressed by the id
//! that follows the form's controls, its `Visible` is the state of one form instance — hidden when the
//! instance is created, gone with it — and a hidden layer's controls are neither drawn nor reachable.

use super::*;
use cobolt_forms::description::{describe, to_json};
use cobolt_forms::xml::load_form_from_str;
use cobolt_wasm_abi::batch::{Path, Seg};
use cobolt_wasm_abi::manifest::{ControlEntry, FormEntry, LayerEntry, Limits, VariantInfo};

const LAYERED: &str = r##"<?xml version="1.0" encoding="UTF-8"?>
<Form name="main-form" title="Layers" width="640" height="480" index-base="1">
  <Layer name="LYR-DATA" color="#00000000"/>
  <Layer name="LYR-POPUP" color="#10203040"/>
  <Control id="BTN-BASE" type="Button" x="10" y="10" w="80" h="30" z-order="9"><Property name="Caption">Base</Property></Control>
  <Control id="PNL-POP" type="Panel" x="100" y="100" w="200" h="200" z-order="0" layer="LYR-POPUP"></Control>
  <Control id="LBL-IN" type="Label" x="10" y="10" w="80" h="20" parent="PNL-POP"><Property name="Caption">in popup</Property></Control>
  <Control id="TXT-DATA" type="TextBox" x="10" y="60" w="200" h="26" layer="LYR-DATA"></Control>
</Form>"##;

/// The manifest the generator writes for [`LAYERED`]: three addressed controls, then the layers (`3 + n …`).
fn manifest(layers: bool) -> Manifest {
    let controls = vec![
        ControlEntry {
            id: 3,
            name: "BTN-BASE".into(),
            kind: "Button".into(),
        },
        ControlEntry {
            id: 4,
            name: "PNL-POP".into(),
            kind: "Panel".into(),
        },
        ControlEntry {
            id: 5,
            name: "TXT-DATA".into(),
            kind: "TextBox".into(),
        },
    ];
    let layers = if layers {
        vec![
            LayerEntry {
                id: 6,
                name: "LYR-DATA".into(),
                index: 0,
            },
            LayerEntry {
                id: 7,
                name: "LYR-POPUP".into(),
                index: 1,
            },
        ]
    } else {
        vec![]
    };
    Manifest {
        abi: 1,
        generator: "test".into(),
        project: "layers".into(),
        variant: VariantInfo {
            multi_memory: true,
            conformance: false,
        },
        limits: Limits::default(),
        names: vec!["Visible".into(), "Name".into(), "BackgroundColor".into()],
        events: vec!["onLoad".into()],
        forms: vec![FormEntry {
            id: 0,
            name: "main-form".into(),
            main: true,
            instances: 2,
            region_size: 64,
            controls,
            handlers: vec![],
            layers,
        }],
        builtins: vec![],
        imports: vec![],
        sizes: Default::default(),
    }
}

fn host() -> Host {
    let mut h = Host::new(manifest(true), HostOptions::default());
    // Through the description, as the Build writes it and the player reads it.
    h.add_description(&to_json(&describe(&load_form_from_str(LAYERED).unwrap())))
        .unwrap();
    h
}

fn on(ctrl: u32, name: u32) -> Path<'static> {
    Path {
        inst: 0,
        ctrl,
        segs: vec![Seg { name, args: vec![] }],
    }
}

fn hidden_flags(h: &Host, name: &str) -> (bool, bool) {
    let t = &h.trees[&0];
    let i = t.by_name(name).unwrap();
    let empty = crate::view::Empty::default();
    let n = crate::view::controls::view(&empty.cx(t), i);
    (n.attr_of("hidden").is_some(), n.attr_of("inert").is_some())
}

#[test]
fn layer_a_form_opened_twice_starts_with_every_layer_hidden_both_times() {
    let mut h = host();
    for round in 1..=2 {
        h.create_tree(0, 0, None, Vec::new()).unwrap();
        let t = &h.trees[&0];
        assert_eq!(t.layers.len(), 2);
        assert!(
            t.layers.iter().all(|l| !l.visible),
            "round {round}: every layer starts hidden"
        );
        assert_eq!(
            hidden_flags(&h, "PNL-POP"),
            (true, true),
            "round {round}: a hidden layer is not drawn and is inert"
        );
        assert_eq!(h.trees[&0].layer_get(1, "Visible").unwrap(), "false");

        // The program shows the popup layer (A9: SET on id 3 + n + 1).
        h.cmd_set(&on(7, 0), "1").unwrap();
        assert!(
            h.trees[&0].layers[1].visible && !h.trees[&0].layers[0].visible,
            "only the layer written"
        );
        assert_eq!(
            hidden_flags(&h, "PNL-POP"),
            (false, false),
            "shown on the next frame"
        );
        assert_eq!(
            hidden_flags(&h, "TXT-DATA"),
            (true, true),
            "the other layer stays hidden"
        );
        assert_eq!(
            hidden_flags(&h, "BTN-BASE"),
            (false, false),
            "the base is not a layer"
        );
        let changed = h.trees.get_mut(&0).unwrap().take_changes();
        assert!(changed.iter().any(|c| matches!(c, crate::tree::Change::Prop { ctl, .. } if *ctl == h.trees[&0].by_name("PNL-POP").unwrap())), "the layer's controls are redrawn");

        // Closed: the instance's layer state goes with it.
        h.destroy(0);
        assert!(
            !h.trees.contains_key(&0),
            "round {round}: no state survives the release of the instance"
        );
    }
    println!(
        "a layered form opened twice: 2 layers hidden at each open, released with the instance"
    );
}

#[test]
fn layer_ids_follow_the_controls_and_name_is_read_only() {
    let mut h = host();
    h.create_tree(0, 0, None, Vec::new()).unwrap();
    assert_eq!(h.trees[&0].layer_by_abi(6), Some(0));
    assert_eq!(h.trees[&0].layer_by_abi(7), Some(1));
    assert!(
        matches!(h.cmd_set(&on(6, 1), "OTHER"), Err(CmdError::Bad(m)) if m.contains("cannot be changed")),
        "Name is refused with -2"
    );
    h.cmd_set(&on(7, 2), "#FF000080").unwrap();
    assert_eq!(
        h.trees[&0].layer_get(1, "BackgroundColor").unwrap(),
        "#FF000080"
    );
    assert_eq!(h.trees[&0].layer_get(1, "Name").unwrap(), "LYR-POPUP");
    assert!(
        matches!(h.cmd_call(&on(7, 0)), Err(CmdError::Unknown(_))),
        "a layer has no methods"
    );
}

#[test]
fn layer_controls_stack_by_layer_then_z_order() {
    let mut h = host();
    h.create_tree(0, 0, None, Vec::new()).unwrap();
    let t = &h.trees[&0];
    let order: Vec<&str> = crate::view::style::mount_order(&t.ctls)
        .into_iter()
        .map(|i| t.ctls[i].name.as_str())
        .collect();
    // BTN-BASE has the highest ZOrder but is in the base; the layers stack above it in their order.
    assert_eq!(
        &order[..3],
        ["BTN-BASE", "TXT-DATA", "PNL-POP"],
        "{order:?}"
    );
    assert_eq!(
        t.layer_of(t.by_name("LBL-IN").unwrap()),
        Some(1),
        "a container's contents are in its layer"
    );
}

#[test]
fn layer_state_is_not_allocated_for_a_form_without_layers() {
    let xml = r##"<?xml version="1.0" encoding="UTF-8"?>
<Form name="main-form" title="Plain" width="640" height="480" index-base="1">
  <Control id="BTN-BASE" type="Button" x="10" y="10" w="80" h="30"></Control>
  <Control id="PNL-POP" type="Panel" x="100" y="100" w="200" h="200"></Control>
  <Control id="TXT-DATA" type="TextBox" x="10" y="60" w="200" h="26"></Control>
</Form>"##;
    let mut h = Host::new(manifest(false), HostOptions::default());
    h.add_description(&to_json(&describe(&load_form_from_str(xml).unwrap())))
        .unwrap();
    h.create_tree(0, 0, None, Vec::new()).unwrap();
    let t = &h.trees[&0];
    assert!(
        t.layers.is_empty() && t.layers.capacity() == 0,
        "no layer state for a form with none"
    );
    assert!(t.ctls.iter().all(|c| c.layer.is_none()));
}

#[test]
fn layer_a_program_built_with_a_layer_the_description_lacks_is_refused() {
    let xml = LAYERED
        .replace(r##"  <Layer name="LYR-DATA" color="#00000000"/>"##, "")
        .replace(r#" layer="LYR-DATA""#, "");
    let mut h = Host::new(manifest(true), HostOptions::default());
    h.add_description(&to_json(&describe(&load_form_from_str(&xml).unwrap())))
        .unwrap();
    let err = h.create_tree(0, 0, None, Vec::new()).unwrap_err();
    assert!(err.contains("layer `LYR-DATA`"), "{err}");
}
