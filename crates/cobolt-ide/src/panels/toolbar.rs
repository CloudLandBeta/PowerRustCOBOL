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
/// `compilable` gates the Run / Debug / Build actions (a project needs at least
/// one COBOL program or one form).
pub fn show(
    ctx: &Context,
    runner: &Runner,
    tr: &Tr,
    lang: &mut Language,
    compilable: bool,
    debuggable: bool,
    // `has_active`: true when there is an active project or open file — gates Save/Check.
    has_active: bool,
) -> ToolbarAction {
    let mut action = ToolbarAction::None;
    let busy = runner.is_running();

    TopBottomPanel::top("toolbar").show(ctx, |ui| {
        ui.horizontal(|ui| {
            ui.add_space(4.0);

            // Order: Open | Save | Check | Build | Debug | Run | Stop | ⚙

            // ── Open file ─────────────────────────────────────────────────────
            if ui.button(tr.tb_open).clicked() {
                action = ToolbarAction::Open;
            }

            // ── Save ──────────────────────────────────────────────────────────
            if ui.add_enabled(has_active, Button::new(tr.tb_save)).clicked() {
                action = ToolbarAction::Save;
            }

            ui.separator();

            // ── Check (parse/analyse only) ────────────────────────────────────
            if ui.add_enabled(has_active, Button::new(tr.tb_check)).clicked() {
                action = ToolbarAction::Check;
            }

            // ── Build binary ──────────────────────────────────────────────────
            let build_resp = ui.add_enabled(compilable, Button::new(tr.tb_build));
            if build_resp.clicked() { action = ToolbarAction::Build; }
            if !compilable { build_resp.on_hover_text(tr.tb_need_program); }

            ui.separator();

            // ── Run (interpreted) ─────────────────────────────────────────────
            let run_btn = Button::new(
                RichText::new(tr.tb_run).color(
                    if busy || !compilable { Color32::GRAY } else { Color32::from_rgb(80, 200, 80) }
                )
            );
            let run_resp = ui.add_enabled(!busy && compilable, run_btn);
            if run_resp.clicked() { action = ToolbarAction::Run; }
            if !compilable { run_resp.on_hover_text(tr.tb_need_program); }

            // ── Debug (right of Run; enabled when a Generated Code item is selected) ─
            let dbg_resp = ui.add_enabled(
                !busy && debuggable,
                Button::new(RichText::new(tr.tb_debug).color(
                    if busy || !debuggable { Color32::GRAY } else { Color32::from_rgb(200, 150, 80) }
                )),
            );
            if dbg_resp.clicked() { action = ToolbarAction::Debug; }
            if !debuggable { dbg_resp.on_hover_text(tr.tb_debug_hint); }

            // ── Stop ─────────────────────────────────────────────────────────
            let stop_btn = Button::new(
                RichText::new(tr.tb_stop).color(
                    if busy { Color32::from_rgb(220, 80, 80) } else { Color32::GRAY }
                )
            );
            if ui.add_enabled(busy, stop_btn).clicked() {
                action = ToolbarAction::Stop;
            }

            // Settings now live in the Main Pane (click the project node); the
            // toolbar no longer has a gear button.

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
    Debug,
    Build,
    Check,
    Open,
    Save,
}
