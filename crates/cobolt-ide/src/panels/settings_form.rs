// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! The project **Settings** form shown in the Main Pane (it replaces the old
//! modal Settings dialog). It opens when the IDE starts with a project, and any
//! time the project (top tree node) is clicked.
//!
//! The form edits a *draft* snapshot; **Save** writes it back to the project +
//! global AI config, **Cancel** resets the draft to the last-saved *baseline*
//! and is disabled until the developer changes a field. Two columns: labels on
//! the left, controls on the right.

use egui::{RichText, Ui};

use crate::i18n::Tr;
use crate::llm::LlmConfig;
use crate::project_model::CoboltProject;

/// A flat, comparable snapshot of every editable setting. `PartialEq` powers the
/// dirty check (draft ≠ baseline → there are unsaved changes).
#[derive(Clone, PartialEq)]
pub struct SettingsDraft {
    // ── Project ──
    pub name: String,
    pub ver_major: u32,
    pub ver_minor: u32,
    pub ver_fix: u32,
    pub main: String,
    pub copyright: String,
    // ── License ──
    pub license_model: String,
    pub license_text: String,
    // ── Appearance ──
    pub theme_id: String,
    pub bg_image: String,
    pub bg_opacity: u8,
    // ── Runtime ──
    pub fixed_format: bool,
    // ── AI assistant (global) ──
    pub llm_endpoint: String,
    pub llm_api_key: String,
    pub llm_model: String,
    pub llm_system_prompt: String,
}

impl SettingsDraft {
    pub fn from_project(p: &CoboltProject, llm: &LlmConfig) -> Self {
        let (major, minor, fix) = p.project.version_parts();
        Self {
            name: p.project.name.clone(),
            ver_major: major, ver_minor: minor, ver_fix: fix,
            main: p.project.main.clone(),
            copyright: p.project.copyright.clone(),
            license_model: p.project.license_model.clone(),
            license_text: p.project.license_text.clone(),
            theme_id: p.ide.theme.clone(),
            bg_image: p.ide.background_image.clone(),
            bg_opacity: p.ide.background_opacity,
            fixed_format: p.runtime.fixed_format,
            llm_endpoint: llm.endpoint.clone(),
            llm_api_key: llm.api_key.clone(),
            llm_model: llm.model.clone(),
            llm_system_prompt: llm.system_prompt.clone(),
        }
    }

    /// Write the draft back into the project + global AI config.
    pub fn apply(&self, p: &mut CoboltProject, llm: &mut LlmConfig) {
        p.project.name = self.name.clone();
        p.project.set_version_parts(self.ver_major, self.ver_minor, self.ver_fix);
        p.project.main = self.main.clone();
        p.project.copyright = self.copyright.clone();
        p.project.license_model = self.license_model.clone();
        p.project.license_text = self.license_text.clone();
        p.ide.theme = self.theme_id.clone();
        p.ide.background_image = self.bg_image.clone();
        p.ide.background_opacity = self.bg_opacity;
        p.runtime.fixed_format = self.fixed_format;
        llm.endpoint = self.llm_endpoint.clone();
        llm.api_key = self.llm_api_key.clone();
        llm.model = self.llm_model.clone();
        llm.system_prompt = self.llm_system_prompt.clone();
    }
}

/// What the caller should do after a frame of the form.
#[derive(Default)]
pub struct SettingsFormAction {
    pub save: bool,
    pub test_connection: bool,
    pub browse_bg: bool,
}

/// Common license identifiers offered in the dropdown.
const LICENSES: &[&str] = &[
    "Proprietary", "MIT", "Apache-2.0", "GPL-3.0", "LGPL-3.0",
    "BSD-3-Clause", "MPL-2.0", "Unlicense", "CC0-1.0",
];

/// Holds the live draft + the last-saved baseline for the dirty check.
pub struct SettingsForm {
    pub draft: SettingsDraft,
    baseline: SettingsDraft,
}

impl SettingsForm {
    pub fn new(p: &CoboltProject, llm: &LlmConfig) -> Self {
        let draft = SettingsDraft::from_project(p, llm);
        Self { baseline: draft.clone(), draft }
    }

    /// Re-seed both draft and baseline (e.g. after loading a different project).
    pub fn reset_to(&mut self, p: &CoboltProject, llm: &LlmConfig) {
        self.draft = SettingsDraft::from_project(p, llm);
        self.baseline = self.draft.clone();
    }

    /// There are unsaved edits.
    pub fn is_dirty(&self) -> bool {
        self.draft != self.baseline
    }

    /// Mark the current draft as saved (call after persisting).
    pub fn mark_saved(&mut self) {
        self.baseline = self.draft.clone();
    }

    /// Discard edits back to the last-saved values.
    pub fn cancel(&mut self) {
        self.draft = self.baseline.clone();
    }

    /// Push the just-picked background-image path into the draft.
    pub fn set_bg_image(&mut self, path: String) {
        self.draft.bg_image = path;
    }

    /// Render the form. Returns the action(s) the caller must perform.
    pub fn show(
        &mut self,
        ui: &mut Ui,
        tr: &Tr,
        themes: &[(&'static str, &'static str)], // (id, display name)
        test_busy: bool,
        test_status: Option<&str>,
    ) -> SettingsFormAction {
        let mut action = SettingsFormAction::default();
        let theme = crate::theme::active();

        ui.horizontal(|ui| {
            ui.heading(format!("⚙ {}", tr.settings_pane_title));
        });
        ui.label(RichText::new(&self.draft.name).strong().color(theme.accent));
        ui.separator();

        egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
            // ── Project ───────────────────────────────────────────────────────
            section(ui, tr.set_sec_project, &theme);
            two_col(ui, "set_project", |ui| {
                row(ui, tr.lbl_proj_name, |ui| {
                    ui.add(egui::TextEdit::singleline(&mut self.draft.name).desired_width(F));
                });
                row(ui, tr.lbl_version, |ui| {
                    ui.horizontal(|ui| {
                        ui.add(egui::DragValue::new(&mut self.draft.ver_major).range(0..=9999));
                        ui.label(".");
                        ui.add(egui::DragValue::new(&mut self.draft.ver_minor).range(0..=9999));
                        ui.label(".");
                        ui.add(egui::DragValue::new(&mut self.draft.ver_fix).range(0..=9999));
                    });
                });
                row(ui, tr.lbl_main_program, |ui| {
                    ui.add(egui::TextEdit::singleline(&mut self.draft.main).desired_width(F));
                });
                row(ui, tr.lbl_copyright, |ui| {
                    ui.add(egui::TextEdit::singleline(&mut self.draft.copyright)
                        .hint_text("© 2026 …").desired_width(F));
                });
            });

            // ── License ───────────────────────────────────────────────────────
            section(ui, tr.set_sec_license, &theme);
            two_col(ui, "set_license", |ui| {
                row(ui, tr.lbl_license_model, |ui| {
                    egui::ComboBox::from_id_salt("license_model")
                        .selected_text(if self.draft.license_model.is_empty() {
                            "Proprietary".to_owned()
                        } else { self.draft.license_model.clone() })
                        .width(220.0)
                        .show_ui(ui, |ui| {
                            for &lic in LICENSES {
                                ui.selectable_value(&mut self.draft.license_model, lic.to_owned(), lic);
                            }
                        });
                });
                row(ui, tr.lbl_license_text, |ui| {
                    ui.add(egui::TextEdit::multiline(&mut self.draft.license_text)
                        .desired_rows(5).desired_width(F).font(egui::TextStyle::Monospace));
                });
            });

            // ── Appearance ────────────────────────────────────────────────────
            section(ui, tr.set_sec_appearance, &theme);
            two_col(ui, "set_appearance", |ui| {
                row(ui, tr.settings_theme, |ui| {
                    let cur = themes.iter().find(|(id, _)| *id == self.draft.theme_id)
                        .map(|(_, n)| *n).unwrap_or(themes.first().map(|(_, n)| *n).unwrap_or(""));
                    egui::ComboBox::from_id_salt("theme_pick")
                        .selected_text(cur).width(220.0)
                        .show_ui(ui, |ui| {
                            for (id, name) in themes {
                                ui.selectable_value(&mut self.draft.theme_id, (*id).to_owned(), *name);
                            }
                        });
                });
                row(ui, tr.settings_background, |ui| {
                    ui.horizontal(|ui| {
                        if ui.button(tr.settings_bg_browse).clicked() { action.browse_bg = true; }
                        let shown = if self.draft.bg_image.is_empty() {
                            tr.settings_bg_none.to_owned()
                        } else { self.draft.bg_image.clone() };
                        ui.label(RichText::new(shown).small().monospace());
                        if !self.draft.bg_image.is_empty()
                            && ui.button(tr.settings_bg_clear).clicked()
                        {
                            self.draft.bg_image.clear();
                        }
                    });
                });
                row(ui, tr.settings_bg_opacity, |ui| {
                    let mut o = self.draft.bg_opacity as i32;
                    if ui.add(egui::Slider::new(&mut o, 0..=100).suffix("%")).changed() {
                        self.draft.bg_opacity = o.clamp(0, 100) as u8;
                    }
                });
            });

            // ── AI assistant ──────────────────────────────────────────────────
            section(ui, tr.settings_ai_title, &theme);
            ui.label(RichText::new(tr.settings_ai_hint).small().color(theme.text_dim));
            two_col(ui, "set_ai", |ui| {
                row(ui, tr.settings_ai_endpoint, |ui| {
                    ui.add(egui::TextEdit::singleline(&mut self.draft.llm_endpoint)
                        .hint_text("https://…/v1/chat/completions").desired_width(F));
                });
                row(ui, tr.settings_ai_api_key, |ui| {
                    ui.add(egui::TextEdit::singleline(&mut self.draft.llm_api_key)
                        .password(true).desired_width(F));
                });
                row(ui, tr.settings_ai_model, |ui| {
                    ui.add(egui::TextEdit::singleline(&mut self.draft.llm_model).desired_width(F));
                });
                row(ui, tr.settings_ai_system_prompt, |ui| {
                    ui.add(egui::TextEdit::multiline(&mut self.draft.llm_system_prompt)
                        .desired_rows(3).desired_width(F));
                });
            });
            ui.horizontal(|ui| {
                if ui.add_enabled(!test_busy, egui::Button::new(tr.settings_ai_test)).clicked() {
                    action.test_connection = true;
                }
                if let Some(s) = test_status {
                    ui.label(RichText::new(s).small());
                }
            });

            // ── Runtime ───────────────────────────────────────────────────────
            section(ui, tr.set_sec_runtime, &theme);
            two_col(ui, "set_runtime", |ui| {
                row(ui, tr.lbl_runtime_fixed, |ui| {
                    ui.checkbox(&mut self.draft.fixed_format, "");
                });
            });

            ui.add_space(12.0);
            ui.separator();
            // ── Save / Cancel ─────────────────────────────────────────────────
            let dirty = self.is_dirty();
            ui.horizontal(|ui| {
                if ui.add_enabled(dirty, egui::Button::new(format!("💾 {}", tr.btn_save))).clicked() {
                    action.save = true;
                }
                if ui.add_enabled(dirty, egui::Button::new(format!("✖ {}", tr.btn_cancel))).clicked() {
                    self.cancel();
                }
            });
        });

        action
    }
}

/// Desired width for full-width text fields.
const F: f32 = 360.0;

fn section(ui: &mut Ui, title: &str, theme: &crate::theme::Theme) {
    ui.add_space(10.0);
    ui.label(RichText::new(title).size(15.0).strong().color(theme.accent));
    ui.add_space(2.0);
}

/// A two-column grid (labels left, controls right).
fn two_col(ui: &mut Ui, id: &str, body: impl FnOnce(&mut Ui)) {
    egui::Grid::new(id)
        .num_columns(2)
        .spacing([14.0, 8.0])
        .min_col_width(120.0)
        .show(ui, body);
}

/// One labelled row inside a [`two_col`] grid.
fn row(ui: &mut Ui, label: &str, control: impl FnOnce(&mut Ui)) {
    ui.label(label);
    control(ui);
    ui.end_row();
}
