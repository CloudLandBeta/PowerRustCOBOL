// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Form layers in the player (WASM-016 R18, R19, AC12; ABI A9; spec 091), as the view draws them: a hidden
//! layer's controls are neither drawn nor reachable, and the layers stack above the base. These two tests
//! moved here from the ABI host's `host/layer_tests.rs` with DESKTOP-WASM-001 T27, since they read what
//! this crate's view draws; the form and its manifest are that file's. The host's private steps are
//! reached through `host::testing`.

use crate::host::testing;
use crate::host::{Host, HostOptions};
use cobolt_forms::description::{describe, to_json};
use cobolt_forms::xml::load_form_from_str;
use cobolt_wasm_abi::batch::{Path, Seg};
use cobolt_wasm_abi::manifest::{ControlEntry, FormEntry, LayerEntry, Limits, Manifest, VariantInfo};

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
        contract: None,
        remote: Vec::new(),
        capabilities: Vec::new(),
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
        testing::create_tree(&mut h, 0, 0).unwrap();
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
        testing::cmd_set(&mut h, &on(7, 0), "1").unwrap();
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
        testing::destroy(&mut h, 0);
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
fn layer_controls_stack_by_layer_then_z_order() {
    let mut h = host();
    testing::create_tree(&mut h, 0, 0).unwrap();
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
