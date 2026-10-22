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
//! The new controls of the series that the model does not carry yet — **WebLink**,
//! **Authentication**, **Canvas2D**, **Canvas3D** and the five E objects — reach the player as
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

/// Is this control a thing with no element (a non-visual service object)?
pub fn is_non_visual(t: &ControlType) -> bool {
    if let Some(k) = player_kind_of(t) {
        return !k.visual();
    }
    matches!(
        t,
        ControlType::Timer
            | ControlType::AgentObject
            | ControlType::RestClient
            | ControlType::SqlDatabase
            | ControlType::IndexedFile
            | ControlType::KnowledgeBase
            | ControlType::WebSearch
            | ControlType::Snackbar
    ) || t.is_aws()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_series_controls_are_found_by_name() {
        let t = ControlType::from_str("WebLink");
        assert_eq!(player_kind_of(&t), Some(PlayerKind::WebLink));
        assert_eq!(class_of(&t), Class::A);
        assert_eq!(class_of(&ControlType::from_str("Authentication")), Class::C);
        assert_eq!(class_of(&ControlType::from_str("Canvas3D")), Class::B);
        assert_eq!(class_of(&ControlType::from_str("Physics2D")), Class::E);
        assert_eq!(class_of(&ControlType::from_str("acme:thing")), Class::D, "a plug-in control is not available");
        assert!(!is_non_visual(&ControlType::from_str("WebLink")));
        assert!(is_non_visual(&ControlType::from_str("ImageBuffer")));
        assert!(is_non_visual(&ControlType::Timer));
        assert!(!is_non_visual(&ControlType::Button));
    }
}
