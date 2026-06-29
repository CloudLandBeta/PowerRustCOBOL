// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Form-theme picker choices, shared between the project Settings form and the
//! per-form Appearance pane (spec 007).
//!
//! The app discovers the asset packs once and **publishes** the catalog
//! (`liquid-glass` + discovered packs, as `(id, display_name)` pairs) into egui's
//! per-frame temp storage. Both pickers read it from there, so the discovered
//! "special" themes surface automatically with no extra `show()` parameters.

/// `(id, display_name)` pairs, Liquid Glass first.
#[derive(Clone, Default)]
pub struct FormThemeChoices(pub Vec<(String, String)>);

fn id() -> egui::Id {
    egui::Id::new("cobolt-form-theme-choices")
}

/// Publish the available form themes for this frame.
pub fn publish(ctx: &egui::Context, choices: Vec<(String, String)>) {
    ctx.data_mut(|d| d.insert_temp(id(), FormThemeChoices(choices)));
}

/// Read the published form themes; falls back to just Liquid Glass if the app
/// has not published yet this frame.
pub fn choices(ctx: &egui::Context) -> Vec<(String, String)> {
    ctx.data(|d| d.get_temp::<FormThemeChoices>(id()))
        .map(|c| c.0)
        .filter(|v| !v.is_empty())
        .unwrap_or_else(|| {
            vec![(
                cobolt_forms::theme::LIQUID_GLASS.to_owned(),
                "Liquid Glass".to_owned(),
            )]
        })
}

/// The display name for a theme id (falls back to the id, then Liquid Glass).
pub fn display_name(ctx: &egui::Context, theme_id: &str) -> String {
    let all = choices(ctx);
    all.iter()
        .find(|(id, _)| id == theme_id)
        .map(|(_, n)| n.clone())
        .unwrap_or_else(|| {
            if theme_id.is_empty() {
                "Liquid Glass".to_owned()
            } else {
                theme_id.to_owned()
            }
        })
}
