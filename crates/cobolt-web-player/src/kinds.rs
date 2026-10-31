// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! **The classification of every control for the web** (WASM-004 §2, R2) — one table.
//!
//! | Class | Meaning |
//! |---|---|
//! | **A** | becomes ordinary HTML/CSS, behaviour from the model |
//! | **B** | drawn by the desktop painter today; here an SVG, an `<img>` or a `<canvas>` inside its own element |
//! | **C** | works through the backend (WASM-005): disk, database, network, a secret |
//! | **D** | needs the local machine; not available |
//! | **E** | a client object that runs in the player with no backend |
//!
//! A control has one primary class; where some operations differ the note says which. A test
//! fails when [`ControlType::ALL`] and this table disagree (WASM-004 AC1), so a control added to
//! the model is classified or the build is red.
//!
//! The new controls of the series that the model does not carry yet — **Authentication**, **Canvas2D**, **Canvas3D** and the five E objects — reach the player as
//! `ControlType::Custom` (the model's own way of holding a type it does not know) and are
//! recognised here by name: [`player_kind`].

use cobolt_forms::model::ControlType;

use crate::tree::{Ctl, Kind};

// The table, the classes and the series kinds live in `cobolt-forms` (WASM-001 T14, plan D-T14a):
// the IDE's toolbox reads the same rows. No row is repeated here.
pub use cobolt_forms::web_class::{available_on_web, class_of, player_kind_of, Class, PlayerKind, Row, PLAYER_KINDS, TABLE};

pub fn player_kind(c: &Ctl) -> Option<PlayerKind> {
    match &c.kind {
        Kind::Control(t) => player_kind_of(t),
        Kind::Form => None,
    }
}

/// Is this control a thing with no element (a non-visual service object)? For a model type the
/// model's own predicate answers (WASM-004 R21: one set, the Designer's Non-Visuals tab's); for a
/// series kind the model carries as `Custom`, whether it has a place on the page.
pub fn is_non_visual(t: &ControlType) -> bool {
    match player_kind_of(t) {
        Some(k) => !k.visual(),
        None => t.is_non_visual(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

    #[test]
    fn the_series_controls_are_found_by_name() {
        let t = ControlType::from_str("WebLink");
        assert_eq!(t, ControlType::Hyperlink, "the interim name is the model's type (WASM-006 T6)");
        assert_eq!(player_kind_of(&t), None);
        assert_eq!(class_of(&t), Class::A);
        assert_eq!(TABLE.len(), 63, "the catalogue, Hyperlink included");
        assert_eq!(class_of(&ControlType::from_str("Authentication")), Class::C);
        assert_eq!(class_of(&ControlType::from_str("Canvas3D")), Class::B);
        assert_eq!(class_of(&ControlType::from_str("Physics2D")), Class::E);
        assert_eq!(class_of(&ControlType::from_str("acme:thing")), Class::D, "a plug-in control is not available");
        assert!(!is_non_visual(&ControlType::Hyperlink));
        assert!(is_non_visual(&ControlType::from_str("ImageBuffer")));
        assert!(is_non_visual(&ControlType::Timer));
        assert!(!is_non_visual(&ControlType::Button));
    }

    #[test]
    fn the_non_visual_set_is_the_models_and_every_client_object_is_non_visual() {
        // WASM-004 R21: the player's set equals `ControlType::is_non_visual` over the catalogue.
        let differ: Vec<&str> = ControlType::ALL.iter().filter(|t| is_non_visual(t) != t.is_non_visual()).map(|t| t.as_str()).collect();
        assert!(differ.is_empty(), "the player and the model disagree on {differ:?}");
        let n = ControlType::ALL.iter().filter(|t| is_non_visual(t)).count();
        println!("non-visual model types: {n} of {}", ControlType::ALL.len());
        for (k, name) in PLAYER_KINDS {
            if k.class() == Class::E {
                assert!(is_non_visual(&ControlType::from_str(name)), "{name} is a client object and has no element");
            }
        }
    }

    /// Every property name the player's view reads by name (`tree.get(i, "…")`).
    fn names_the_view_reads() -> BTreeSet<String> {
        const SOURCES: &[&str] = &[
            include_str!("view/mod.rs"),
            include_str!("view/controls.rs"),
            include_str!("view/interact.rs"),
            include_str!("view/style.rs"),
            include_str!("view/data.rs"),
            include_str!("view/shell.rs"),
            include_str!("view/charts.rs"),
            include_str!("view/maps.rs"),
            include_str!("view/viewer.rs"),
            include_str!("view/auth.rs"),
        ];
        let mut out = BTreeSet::new();
        for src in SOURCES {
            let mut rest = *src;
            while let Some(at) = rest.find("tree.get(") {
                rest = &rest[at + "tree.get(".len()..];
                let Some(comma) = rest.find(',') else { break };
                let after = rest[comma + 1..].trim_start();
                if let Some(lit) = after.strip_prefix('"') {
                    if let Some(end) = lit.find('"') {
                        out.insert(lit[..end].to_owned());
                    }
                }
            }
        }
        out
    }

    #[test]
    fn every_property_the_view_reads_is_a_model_or_a_layer_property() {
        // WASM-004 R1, R7: the web introduces no member by accident. A name is known when a
        // catalogue type has it (`property_names_for`, field-backed geometry included), lists it as
        // a runtime property, or it is a layer property. Two more are the desktop host's own
        // run-time state, read by its painter too: names with a leading `_` (`__ChartData`,
        // `paint.rs`) and the TreeView's `SelectedNode` (`render.rs`, the interpreter's
        // `GetSelectedNode`), which the model's runtime list does not name.
        use cobolt_forms::model::{property_names_for, runtime_property_names_for, LAYER_PROPS};
        let mut known: BTreeSet<String> = ["selectednode".to_owned()].into();
        for t in ControlType::ALL {
            known.extend(property_names_for(t.as_str()).iter().map(|k| k.to_ascii_lowercase()));
            known.extend(runtime_property_names_for(t.as_str()).iter().map(|k| k.to_ascii_lowercase()));
        }
        known.extend(LAYER_PROPS.iter().map(|k| k.to_ascii_lowercase()));
        let read = names_the_view_reads();
        let unknown: Vec<&String> = read.iter().filter(|n| !n.starts_with('_') && !known.contains(&n.to_ascii_lowercase())).collect();
        println!("the view reads {} names; {} known to the model", read.len(), read.len() - unknown.len());
        assert!(read.len() > 50, "the scan found the view's reads");
        assert!(unknown.is_empty(), "names the model does not have: {unknown:?}");
    }
}
