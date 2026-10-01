// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! The last step of an AI configuration import: one API key per provider the
//! imported file names. The file carries no key (see [`crate::ai_bundle`]),
//! so this is where each developer puts their own.

use crate::i18n::Tr;
use crate::llm::{provider_key_slot, LlmConfig, Provider};
use crate::theme::Theme;

/// Fixed size: the window never sizes itself (CONVENTIONS.md). The rows
/// scroll inside it.
const SIZE: egui::Vec2 = egui::vec2(560.0, 360.0);

struct Row {
    provider: String,
    label: String,
    stored: bool,
    key: String,
}

pub struct ImportKeysModal {
    pub open: bool,
    rows: Vec<Row>,
}

impl ImportKeysModal {
    /// `needing`: `(provider id, a key is already stored)`, from
    /// [`crate::ai_bundle::providers_needing_keys`]. `None` when there is
    /// nothing to ask.
    pub fn new(needing: Vec<(String, bool)>) -> Option<Self> {
        if needing.is_empty() {
            return None;
        }
        let rows = needing
            .into_iter()
            .map(|(provider, stored)| Row {
                label: Provider::from_id(&provider)
                    .map(|p| p.label().to_string())
                    .unwrap_or_else(|| provider.clone()),
                provider,
                stored,
                key: String::new(),
            })
            .collect();
        Some(Self { open: true, rows })
    }

    /// Store every key typed. An empty field changes nothing. Returns how
    /// many were stored.
    pub fn store(&mut self, llm: &mut LlmConfig) -> usize {
        let mut n = 0;
        for r in &mut self.rows {
            if !r.key.trim().is_empty() {
                llm.store_api_key(provider_key_slot(&r.provider), r.key.trim());
                r.key.clear();
                n += 1;
            }
        }
        n
    }

    /// Draw the window. Returns `true` when the developer pressed Save — the
    /// caller then calls [`Self::store`] and saves the configuration.
    pub fn show(&mut self, ctx: &egui::Context, tr: &Tr, theme: &Theme) -> bool {
        let mut save = false;
        let mut close = false;
        let mut open = self.open;
        egui::Window::new(egui::RichText::new(tr.ai_keys_title).size(18.0).strong())
            .id(egui::Id::new("ai_import_keys_modal"))
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .fixed_size(SIZE)
            .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
            .show(ctx, |ui| {
                let margin = ui.style().spacing.window_margin.sum().x;
                let inner_w = SIZE.x - margin;
                ui.add_sized(
                    [inner_w, 0.0],
                    egui::Label::new(egui::RichText::new(tr.ai_keys_intro).color(theme.text_dim)).wrap(),
                );
                ui.add_space(8.0);
                egui::ScrollArea::vertical()
                    .id_salt("ai_import_keys_rows")
                    .max_height(SIZE.y - 130.0)
                    .min_scrolled_height(SIZE.y - 130.0)
                    .show(ui, |ui| {
                        egui::Grid::new("ai_import_keys_grid")
                            .num_columns(3)
                            .spacing([12.0, 8.0])
                            .show(ui, |ui| {
                                for r in &mut self.rows {
                                    ui.label(egui::RichText::new(&r.label).strong());
                                    ui.add_sized(
                                        [220.0, 24.0],
                                        egui::TextEdit::singleline(&mut r.key).password(true),
                                    );
                                    let (text, col) = if r.stored {
                                        (tr.ai_keys_stored, theme.text_dim)
                                    } else {
                                        (tr.ai_keys_missing, theme.accent)
                                    };
                                    ui.label(egui::RichText::new(text).size(12.0).color(col));
                                    ui.end_row();
                                }
                            });
                    });
                ui.separator();
                ui.horizontal(|ui| {
                    if ui.button(tr.btn_save).clicked() {
                        save = true;
                        close = true;
                    }
                    if ui.button(tr.ai_keys_later).clicked() {
                        close = true;
                    }
                });
            });
        self.open = open && !close;
        save
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn typed_keys_are_stored_per_provider_and_empty_fields_change_nothing() {
        let mut llm = LlmConfig::load_defaults_for_test();
        llm.store_api_key(provider_key_slot("groq"), "gsk-kept-0001");
        let mut m = ImportKeysModal::new(vec![("anthropic".into(), false), ("groq".into(), true)]).unwrap();
        m.rows[0].key = "  sk-ant-new  ".into();
        assert_eq!(m.store(&mut llm), 1);
        assert_eq!(llm.api_keys.get(&provider_key_slot("anthropic")).map(String::as_str), Some("sk-ant-new"));
        assert_eq!(llm.api_keys.get(&provider_key_slot("groq")).map(String::as_str), Some("gsk-kept-0001"));
        assert!(m.rows.iter().all(|r| r.key.is_empty()), "typed keys do not linger in the window");
        assert!(ImportKeysModal::new(Vec::new()).is_none());
    }
}
