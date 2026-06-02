// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Top toolbar panel — Run ▶, Stop ■, Build ⚙ buttons + language selector.

use egui::{Context, TopBottomPanel, Button, RichText, Color32};

use crate::runner::Runner;
use crate::i18n::{Language, Tr};

/// Render the toolbar and return the user's action (if any).
///
/// `lang` is updated in-place when the user picks a different language.
pub fn show(ctx: &Context, runner: &Runner, tr: &Tr, lang: &mut Language) -> ToolbarAction {
    let mut action = ToolbarAction::None;

    TopBottomPanel::top("toolbar").show(ctx, |ui| {
        ui.horizontal(|ui| {
            ui.add_space(4.0);

            // ── Run ──────────────────────────────────────────────────────────
            let run_btn = Button::new(
                RichText::new(tr.tb_run).color(
                    if runner.is_running() { Color32::GRAY } else { Color32::from_rgb(80, 200, 80) }
                )
            );
            if ui.add_enabled(!runner.is_running(), run_btn).clicked() {
                action = ToolbarAction::Run;
            }

            ui.add_space(4.0);

            // ── Stop ─────────────────────────────────────────────────────────
            let stop_btn = Button::new(
                RichText::new(tr.tb_stop).color(
                    if runner.is_running() { Color32::from_rgb(220, 80, 80) } else { Color32::GRAY }
                )
            );
            if ui.add_enabled(runner.is_running(), stop_btn).clicked() {
                action = ToolbarAction::Stop;
            }

            ui.separator();

            // ── Build (check only) ────────────────────────────────────────────
            if ui.button(tr.tb_check).clicked() {
                action = ToolbarAction::Check;
            }

            ui.separator();

            // ── Open file ─────────────────────────────────────────────────────
            if ui.button(tr.tb_open).clicked() {
                action = ToolbarAction::Open;
            }

            // ── Save ──────────────────────────────────────────────────────────
            if ui.button(tr.tb_save).clicked() {
                action = ToolbarAction::Save;
            }

            // ── Right side: spinner + language selector ────────────────────────
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                // Language selector ────────────────────────────────────────────
                ui.add_space(4.0);
                egui::ComboBox::from_id_salt("ide_lang_selector")
                    .selected_text(lang.native_name())
                    .width(140.0)
                    .show_ui(ui, |ui| {
                        for &l in Language::ALL {
                            ui.selectable_value(lang, l, l.native_name());
                        }
                    });
                ui.label("🌐").on_hover_text(tr.lang_btn_tooltip);
                ui.separator();

                if runner.is_running() {
                    ui.spinner();
                    ui.label(RichText::new(tr.tb_running).color(Color32::YELLOW));
                }
            });
        });
    });

    action
}

/// Action requested by the toolbar buttons.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToolbarAction {
    None,
    Run,
    Stop,
    Check,
    Open,
    Save,
}
