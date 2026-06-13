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
//! and is disabled until the developer changes a field. Two columns with a
//! single continuous draggable resizer line (top to bottom of content): labels
//! on the left never wrap (truncated with … when they would overflow the chosen
//! split), controls on the right (elastic width, 10 px gap after the line). The
//! resizer can be dragged freely up to 80 % of the pane; its colour follows the
//! active theme and brightens on hover/drag. All property value controls stay
//! perfectly vertically aligned to the same x position.

use egui::{Color32, RichText, Ui};

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
    /// Width of the label column; user can drag the resizer to adjust.
    splitter: f32,
}

impl SettingsForm {
    pub fn new(p: &CoboltProject, llm: &LlmConfig) -> Self {
        let draft = SettingsDraft::from_project(p, llm);
        Self { baseline: draft.clone(), draft, splitter: 200.0 }
    }

    /// Re-seed both draft and baseline (e.g. after loading a different project).
    pub fn reset_to(&mut self, p: &CoboltProject, llm: &LlmConfig) {
        self.draft = SettingsDraft::from_project(p, llm);
        self.baseline = self.draft.clone();
        // keep user's preferred splitter position
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

        // With the settings glass card now using the exact same
        // CentralPanel.frame(card) construction as the widget properties
        // inspector (see show_settings_pane), the outer pane border reaches
        // the full extent of the available central area. No extra right
        // padding is needed here; the glass inner_margin + property_indent
        // provide breathing room. Content fills the full inner width so there
        // is no ~9px shortfall.
        let right_padding = 0.0;
        let full_avail = ui.available_width();
        let content_w = (full_avail - right_padding).max(50.0);

        ui.horizontal_top(|ui| {
            ui.allocate_ui(egui::vec2(content_w, 0.0), |ui| {
                ui.heading("Project Settings");
            });
            ui.add_space(right_padding);
        });

        egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
            let resizer_width = 5.0;
            let gap_after_resizer = 10.0;
            let total_w = ui.available_width();
            let right_padding = 0.0;
            let content_w = (total_w - right_padding).max(50.0);

            let mut splitter = self.splitter.clamp(50.0, content_w * 0.8);

            // Capture the starting geometry so the resizer line + drag target
            // can span the *exact* natural height of the form content (single
            // continuous vertical line) without affecting layout measurement.
            let content_left = ui.cursor().left();
            let content_top = ui.cursor().top();

            // The rows use the full inner width of the glass (matching how
            // property sections fill their inspector card). The glass frame's
            // inner margin keeps content from touching the right stroke.
            ui.allocate_ui(egui::vec2(content_w, 0.0), |ui| {
                // Layout the form content as a series of small horizontal rows.
                // Each property gets its own horizontal_top so the label (left) and
                // its value control (right) are siblings in the same horizontal and
                // therefore top-aligned by horizontal_top. The continuous resizer
                // line is still painted as one overlay across the full height afterwards.
                let property_indent = 12.0;

                ui.vertical(|ui| {
                // --- Project section header (left only)
                ui.horizontal_top(|ui| {
                    let left_rect = ui.allocate_exact_size(egui::vec2(splitter, 0.0), egui::Sense::hover()).0;
                    ui.allocate_ui_at_rect(left_rect, |ui| {
                        ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Truncate);
                        ui.set_min_width(splitter);
                        section(ui, tr.set_sec_project, &theme);
                    });
                    ui.allocate_space(egui::vec2(resizer_width, 0.0));
                    ui.add_space(gap_after_resizer);
                    let right_w = ui.available_width();
                    ui.allocate_ui(egui::vec2(right_w, 0.0), |_ui| {});
                });

                // Name
                ui.horizontal_top(|ui| {
                    let left_rect = ui.allocate_exact_size(egui::vec2(splitter, 0.0), egui::Sense::hover()).0;
                    ui.allocate_ui_at_rect(left_rect, |ui| {
                        ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Truncate);
                        ui.set_min_width(splitter);
                        ui.add_space(property_indent);
                        ui.add(egui::Label::new(tr.lbl_proj_name).truncate());
                    });
                    ui.allocate_space(egui::vec2(resizer_width, 0.0));
                    ui.add_space(gap_after_resizer);
                    let right_w = ui.available_width();
                    ui.allocate_ui(egui::vec2(right_w, 0.0), |ui| {
                        let w = ui.available_width();
                        ui.add(egui::TextEdit::singleline(&mut self.draft.name).desired_width(w));
                    });
                });

                // Version (the drag values row on right is treated as the "value" for the Version label)
                ui.horizontal_top(|ui| {
                    let left_rect = ui.allocate_exact_size(egui::vec2(splitter, 0.0), egui::Sense::hover()).0;
                    ui.allocate_ui_at_rect(left_rect, |ui| {
                        ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Truncate);
                        ui.set_min_width(splitter);
                        ui.add_space(property_indent);
                        ui.add(egui::Label::new(tr.lbl_version).truncate());
                    });
                    ui.allocate_space(egui::vec2(resizer_width, 0.0));
                    ui.add_space(gap_after_resizer);
                    let right_w = ui.available_width();
                    ui.allocate_ui(egui::vec2(right_w, 0.0), |ui| {
                        ui.horizontal(|ui| {
                            ui.add(egui::DragValue::new(&mut self.draft.ver_major).range(0..=9999));
                            ui.label(".");
                            ui.add(egui::DragValue::new(&mut self.draft.ver_minor).range(0..=9999));
                            ui.label(".");
                            ui.add(egui::DragValue::new(&mut self.draft.ver_fix).range(0..=9999));
                        });
                    });
                });

                // Main program
                ui.horizontal_top(|ui| {
                    let left_rect = ui.allocate_exact_size(egui::vec2(splitter, 0.0), egui::Sense::hover()).0;
                    ui.allocate_ui_at_rect(left_rect, |ui| {
                        ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Truncate);
                        ui.set_min_width(splitter);
                        ui.add_space(property_indent);
                        ui.add(egui::Label::new(tr.lbl_main_program).truncate());
                    });
                    ui.allocate_space(egui::vec2(resizer_width, 0.0));
                    ui.add_space(gap_after_resizer);
                    let right_w = ui.available_width();
                    ui.allocate_ui(egui::vec2(right_w, 0.0), |ui| {
                        let w = ui.available_width();
                        ui.add(egui::TextEdit::singleline(&mut self.draft.main).desired_width(w));
                    });
                });

                // Copyright
                ui.horizontal_top(|ui| {
                    let left_rect = ui.allocate_exact_size(egui::vec2(splitter, 0.0), egui::Sense::hover()).0;
                    ui.allocate_ui_at_rect(left_rect, |ui| {
                        ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Truncate);
                        ui.set_min_width(splitter);
                        ui.add_space(property_indent);
                        ui.add(egui::Label::new(tr.lbl_copyright).truncate());
                    });
                    ui.allocate_space(egui::vec2(resizer_width, 0.0));
                    ui.add_space(gap_after_resizer);
                    let right_w = ui.available_width();
                    ui.allocate_ui(egui::vec2(right_w, 0.0), |ui| {
                        let w = ui.available_width();
                        ui.add(egui::TextEdit::singleline(&mut self.draft.copyright)
                            .hint_text("© 2026 …").desired_width(w));
                    });
                });

                ui.add_space(8.0);

                // --- License section header (left only)
                ui.horizontal_top(|ui| {
                    let left_rect = ui.allocate_exact_size(egui::vec2(splitter, 0.0), egui::Sense::hover()).0;
                    ui.allocate_ui_at_rect(left_rect, |ui| {
                        ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Truncate);
                        ui.set_min_width(splitter);
                        section(ui, tr.set_sec_license, &theme);
                    });
                    ui.allocate_space(egui::vec2(resizer_width, 0.0));
                    ui.add_space(gap_after_resizer);
                    let right_w = ui.available_width();
                    ui.allocate_ui(egui::vec2(right_w, 0.0), |_ui| {});
                });

                // License model
                ui.horizontal_top(|ui| {
                    let left_rect = ui.allocate_exact_size(egui::vec2(splitter, 0.0), egui::Sense::hover()).0;
                    ui.allocate_ui_at_rect(left_rect, |ui| {
                        ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Truncate);
                        ui.set_min_width(splitter);
                        ui.add_space(property_indent);
                        ui.add(egui::Label::new(tr.lbl_license_model).truncate());
                    });
                    ui.allocate_space(egui::vec2(resizer_width, 0.0));
                    ui.add_space(gap_after_resizer);
                    let right_w = ui.available_width();
                    ui.allocate_ui(egui::vec2(right_w, 0.0), |ui| {
                        let w = ui.available_width();
                        egui::ComboBox::from_id_salt("license_model")
                            .selected_text(if self.draft.license_model.is_empty() {
                                "Proprietary".to_owned()
                            } else { self.draft.license_model.clone() })
                            .width(w)
                            .show_ui(ui, |ui| {
                                for &lic in LICENSES {
                                    ui.selectable_value(&mut self.draft.license_model, lic.to_owned(), lic);
                                }
                            });
                    });
                });

                // License text (multiline on right determines row height; label is top-aligned to it)
                ui.horizontal_top(|ui| {
                    let left_rect = ui.allocate_exact_size(egui::vec2(splitter, 0.0), egui::Sense::hover()).0;
                    ui.allocate_ui_at_rect(left_rect, |ui| {
                        ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Truncate);
                        ui.set_min_width(splitter);
                        ui.add_space(property_indent);
                        ui.add(egui::Label::new(tr.lbl_license_text).truncate());
                    });
                    ui.allocate_space(egui::vec2(resizer_width, 0.0));
                    ui.add_space(gap_after_resizer);
                    let right_w = ui.available_width();
                    ui.allocate_ui(egui::vec2(right_w, 0.0), |ui| {
                        let w = ui.available_width();
                        ui.add(egui::TextEdit::multiline(&mut self.draft.license_text)
                            .desired_rows(5).desired_width(w).font(egui::TextStyle::Monospace));
                    });
                });

                ui.add_space(8.0);

                // --- Appearance section header (left only)
                ui.horizontal_top(|ui| {
                    let left_rect = ui.allocate_exact_size(egui::vec2(splitter, 0.0), egui::Sense::hover()).0;
                    ui.allocate_ui_at_rect(left_rect, |ui| {
                        ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Truncate);
                        ui.set_min_width(splitter);
                        section(ui, tr.set_sec_appearance, &theme);
                    });
                    ui.allocate_space(egui::vec2(resizer_width, 0.0));
                    ui.add_space(gap_after_resizer);
                    let right_w = ui.available_width();
                    ui.allocate_ui(egui::vec2(right_w, 0.0), |_ui| {});
                });

                // Theme
                ui.horizontal_top(|ui| {
                    let left_rect = ui.allocate_exact_size(egui::vec2(splitter, 0.0), egui::Sense::hover()).0;
                    ui.allocate_ui_at_rect(left_rect, |ui| {
                        ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Truncate);
                        ui.set_min_width(splitter);
                        ui.add_space(property_indent);
                        ui.add(egui::Label::new(tr.settings_theme).truncate());
                    });
                    ui.allocate_space(egui::vec2(resizer_width, 0.0));
                    ui.add_space(gap_after_resizer);
                    let right_w = ui.available_width();
                    ui.allocate_ui(egui::vec2(right_w, 0.0), |ui| {
                        let w = ui.available_width();
                        let cur = themes.iter().find(|(id, _)| *id == self.draft.theme_id)
                            .map(|(_, n)| *n).unwrap_or(themes.first().map(|(_, n)| *n).unwrap_or(""));
                        egui::ComboBox::from_id_salt("theme_pick")
                            .selected_text(cur).width(w)
                            .show_ui(ui, |ui| {
                                for (id, name) in themes {
                                    ui.selectable_value(&mut self.draft.theme_id, (*id).to_owned(), *name);
                                }
                            });
                    });
                });

                // Background image (the button row + shown path is the "value")
                ui.horizontal_top(|ui| {
                    let left_rect = ui.allocate_exact_size(egui::vec2(splitter, 0.0), egui::Sense::hover()).0;
                    ui.allocate_ui_at_rect(left_rect, |ui| {
                        ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Truncate);
                        ui.set_min_width(splitter);
                        ui.add_space(property_indent);
                        ui.add(egui::Label::new(tr.settings_background).truncate());
                    });
                    ui.allocate_space(egui::vec2(resizer_width, 0.0));
                    ui.add_space(gap_after_resizer);
                    let right_w = ui.available_width();
                    ui.allocate_ui(egui::vec2(right_w, 0.0), |ui| {
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
                });

                // Background opacity (slider row)
                ui.horizontal_top(|ui| {
                    let left_rect = ui.allocate_exact_size(egui::vec2(splitter, 0.0), egui::Sense::hover()).0;
                    ui.allocate_ui_at_rect(left_rect, |ui| {
                        ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Truncate);
                        ui.set_min_width(splitter);
                        ui.add_space(property_indent);
                        ui.add(egui::Label::new(tr.settings_bg_opacity).truncate());
                    });
                    ui.allocate_space(egui::vec2(resizer_width, 0.0));
                    ui.add_space(gap_after_resizer);
                    let right_w = ui.available_width();
                    ui.allocate_ui(egui::vec2(right_w, 0.0), |ui| {
                        let mut o = self.draft.bg_opacity as i32;
                        if ui.add(egui::Slider::new(&mut o, 0..=100).suffix("%")).changed() {
                            self.draft.bg_opacity = o.clamp(0, 100) as u8;
                        }
                    });
                });

                ui.add_space(8.0);

                // --- AI assistant section header (left only)
                ui.horizontal_top(|ui| {
                    let left_rect = ui.allocate_exact_size(egui::vec2(splitter, 0.0), egui::Sense::hover()).0;
                    ui.allocate_ui_at_rect(left_rect, |ui| {
                        ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Truncate);
                        ui.set_min_width(splitter);
                        section(ui, tr.settings_ai_title, &theme);
                    });
                    ui.allocate_space(egui::vec2(resizer_width, 0.0));
                    ui.add_space(gap_after_resizer);
                    let right_w = ui.available_width();
                    ui.allocate_ui(egui::vec2(right_w, 0.0), |_ui| {});
                });

                // AI hint (small text on right, no paired left label)
                ui.horizontal_top(|ui| {
                    let left_rect = ui.allocate_exact_size(egui::vec2(splitter, 0.0), egui::Sense::hover()).0;
                    ui.allocate_ui_at_rect(left_rect, |ui| {
                        ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Truncate);
                        ui.set_min_width(splitter);
                        // empty left side for the hint row
                    });
                    ui.allocate_space(egui::vec2(resizer_width, 0.0));
                    ui.add_space(gap_after_resizer);
                    let right_w = ui.available_width();
                    ui.allocate_ui(egui::vec2(right_w, 0.0), |ui| {
                        ui.label(RichText::new(tr.settings_ai_hint).small().color(theme.text_dim));
                    });
                });

                // Endpoint
                ui.horizontal_top(|ui| {
                    let left_rect = ui.allocate_exact_size(egui::vec2(splitter, 0.0), egui::Sense::hover()).0;
                    ui.allocate_ui_at_rect(left_rect, |ui| {
                        ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Truncate);
                        ui.set_min_width(splitter);
                        ui.add_space(property_indent);
                        ui.add(egui::Label::new(tr.settings_ai_endpoint).truncate());
                    });
                    ui.allocate_space(egui::vec2(resizer_width, 0.0));
                    ui.add_space(gap_after_resizer);
                    let right_w = ui.available_width();
                    ui.allocate_ui(egui::vec2(right_w, 0.0), |ui| {
                        let w = ui.available_width();
                        ui.add(egui::TextEdit::singleline(&mut self.draft.llm_endpoint)
                            .hint_text("https://…/v1/chat/completions").desired_width(w));
                    });
                });

                // API key
                ui.horizontal_top(|ui| {
                    let left_rect = ui.allocate_exact_size(egui::vec2(splitter, 0.0), egui::Sense::hover()).0;
                    ui.allocate_ui_at_rect(left_rect, |ui| {
                        ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Truncate);
                        ui.set_min_width(splitter);
                        ui.add_space(property_indent);
                        ui.add(egui::Label::new(tr.settings_ai_api_key).truncate());
                    });
                    ui.allocate_space(egui::vec2(resizer_width, 0.0));
                    ui.add_space(gap_after_resizer);
                    let right_w = ui.available_width();
                    ui.allocate_ui(egui::vec2(right_w, 0.0), |ui| {
                        let w = ui.available_width();
                        ui.add(egui::TextEdit::singleline(&mut self.draft.llm_api_key)
                            .password(true).desired_width(w));
                    });
                });

                // Model
                ui.horizontal_top(|ui| {
                    let left_rect = ui.allocate_exact_size(egui::vec2(splitter, 0.0), egui::Sense::hover()).0;
                    ui.allocate_ui_at_rect(left_rect, |ui| {
                        ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Truncate);
                        ui.set_min_width(splitter);
                        ui.add_space(property_indent);
                        ui.add(egui::Label::new(tr.settings_ai_model).truncate());
                    });
                    ui.allocate_space(egui::vec2(resizer_width, 0.0));
                    ui.add_space(gap_after_resizer);
                    let right_w = ui.available_width();
                    ui.allocate_ui(egui::vec2(right_w, 0.0), |ui| {
                        let w = ui.available_width();
                        ui.add(egui::TextEdit::singleline(&mut self.draft.llm_model).desired_width(w));
                    });
                });

                // Standard system prompt (multiline)
                ui.horizontal_top(|ui| {
                    let left_rect = ui.allocate_exact_size(egui::vec2(splitter, 0.0), egui::Sense::hover()).0;
                    ui.allocate_ui_at_rect(left_rect, |ui| {
                        ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Truncate);
                        ui.set_min_width(splitter);
                        ui.add_space(property_indent);
                        ui.add(egui::Label::new(tr.settings_ai_system_prompt).truncate());
                    });
                    ui.allocate_space(egui::vec2(resizer_width, 0.0));
                    ui.add_space(gap_after_resizer);
                    let right_w = ui.available_width();
                    ui.allocate_ui(egui::vec2(right_w, 0.0), |ui| {
                        let w = ui.available_width();
                        ui.add(egui::TextEdit::multiline(&mut self.draft.llm_system_prompt)
                            .desired_rows(3).desired_width(w));
                    });
                });

                // Test button row (no paired left label)
                ui.horizontal_top(|ui| {
                    let left_rect = ui.allocate_exact_size(egui::vec2(splitter, 0.0), egui::Sense::hover()).0;
                    ui.allocate_ui_at_rect(left_rect, |ui| {
                        ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Truncate);
                        ui.set_min_width(splitter);
                    });
                    ui.allocate_space(egui::vec2(resizer_width, 0.0));
                    ui.add_space(gap_after_resizer);
                    let right_w = ui.available_width();
                    ui.allocate_ui(egui::vec2(right_w, 0.0), |ui| {
                        ui.horizontal(|ui| {
                            if ui.add_enabled(!test_busy, egui::Button::new(tr.settings_ai_test)).clicked() {
                                action.test_connection = true;
                            }
                            if let Some(s) = test_status {
                                ui.label(RichText::new(s).small());
                            }
                        });
                    });
                });

                ui.add_space(8.0);

                // --- Runtime section header (left only)
                ui.horizontal_top(|ui| {
                    let left_rect = ui.allocate_exact_size(egui::vec2(splitter, 0.0), egui::Sense::hover()).0;
                    ui.allocate_ui_at_rect(left_rect, |ui| {
                        ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Truncate);
                        ui.set_min_width(splitter);
                        section(ui, tr.set_sec_runtime, &theme);
                    });
                    ui.allocate_space(egui::vec2(resizer_width, 0.0));
                    ui.add_space(gap_after_resizer);
                    let right_w = ui.available_width();
                    ui.allocate_ui(egui::vec2(right_w, 0.0), |_ui| {});
                });

                // Fixed format checkbox
                ui.horizontal_top(|ui| {
                    let left_rect = ui.allocate_exact_size(egui::vec2(splitter, 0.0), egui::Sense::hover()).0;
                    ui.allocate_ui_at_rect(left_rect, |ui| {
                        ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Truncate);
                        ui.set_min_width(splitter);
                        ui.add_space(property_indent);
                        ui.add(egui::Label::new(tr.lbl_runtime_fixed).truncate());
                    });
                    ui.allocate_space(egui::vec2(resizer_width, 0.0));
                    ui.add_space(gap_after_resizer);
                    let right_w = ui.available_width();
                    ui.allocate_ui(egui::vec2(right_w, 0.0), |ui| {
                        ui.checkbox(&mut self.draft.fixed_format, "");
                    });
                });
            });
            });

            ui.add_space(right_padding);

            // Exact content height now known (after the two columns were laid out).
            let content_bottom = ui.cursor().top();
            let y_range = egui::Rangef::new(content_top, content_bottom);

            // Position the hit area based on the splitter value used for *this frame's*
            // column layout (so the drag handle is where the columns currently are).
            let layout_resizer_left = content_left + splitter;

            // Comfortable drag target (a little wider than the visible line)
            // so the developer can easily grab it anywhere along the form.
            let hit_width = (resizer_width + 4.0).max(8.0);
            let hit_left = layout_resizer_left + (resizer_width - hit_width) * 0.5;
            let hit_rect = egui::Rect::from_x_y_ranges(
                egui::Rangef::new(hit_left, hit_left + hit_width),
                y_range,
            );

            let resizer_resp = ui.interact(
                hit_rect,
                egui::Id::new("project_settings_resizer"),
                egui::Sense::drag() | egui::Sense::hover(),
            );

            if resizer_resp.dragged() {
                splitter += resizer_resp.drag_delta().x;
                splitter = splitter.clamp(50.0, content_w * 0.8);
            }

            // Write back so the *next* frame will layout the columns at the new split.
            self.splitter = splitter;

            // Paint the line using the live (post-drag) splitter for this frame.
            // This makes the visual line follow the mouse immediately while dragging.
            let paint_resizer_left = content_left + splitter;
            let line_x = paint_resizer_left + resizer_width * 0.5;
            let active = resizer_resp.hovered() || resizer_resp.dragged();
            let resizer_color = if theme.dark {
                if active { Color32::from_gray(140) } else { Color32::from_gray(75) }
            } else {
                if active { Color32::from_gray(105) } else { Color32::from_gray(155) }
            };
            ui.painter().vline(line_x, y_range, egui::Stroke::new(2.0, resizer_color));
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

        // Small padding below the buttons inside the glass card. The pane
        // (framed CentralPanel, identical to properties inspector for width)
        // is full 100% height above the output (grows/shrinks on resize).
        // Form content is placed in a shorter rect + reservation inside the
        // glass (see show_settings_pane) so Save/Cancel are fully visible;
        // the frame outer margin keeps the rounded bottom border clear.
        ui.add_space(12.0);

        action
    }
}

fn section(ui: &mut Ui, title: &str, theme: &crate::theme::Theme) {
    ui.add_space(10.0);
    ui.label(RichText::new(title).size(15.0).strong().color(theme.accent));
    ui.add_space(2.0);
}
