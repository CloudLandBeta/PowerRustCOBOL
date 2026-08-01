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
use crate::panels::editor::{EditorPanel, KnownControl};
use crate::project_model::CoboltProject;

/// A flat, comparable snapshot of every editable setting. `PartialEq` powers the
/// dirty check (draft ≠ baseline → there are unsaved changes).
#[derive(Clone, PartialEq, Default)]
pub struct SettingsDraft {
    // ── Project ──
    pub name: String,
    pub ver_major: u32,
    pub ver_minor: u32,
    pub ver_fix: u32,
    pub main: String,
    pub copyright: String,
    pub destination_folder: String,
    pub debug_compilation: bool,
    // ── License ──
    pub license_model: String,
    pub license_text: String,
    // ── Appearance ──
    pub theme_id: String,
    pub bg_image: String,
    pub project_icon: String,
    pub bg_opacity: u8,
    /// Default **form** theme id (spec 007); empty ⇒ Liquid Glass.
    pub form_theme_id: String,
    // ── Window effects (spec 038, project-level) ──
    pub fx_entrance_effect: String,
    pub fx_entrance_ms: u32,
    pub fx_entrance_easing: String,
    pub fx_exit_effect: String,
    pub fx_exit_ms: u32,
    pub fx_exit_easing: String,
    pub fx_restore: bool,
    // ── Runtime ──
    pub fixed_format: bool,
    // ── Run-Form inspector ──
    pub insp_dump_enabled: bool,
    pub insp_dump_path: String,
    // ── AI assistant (project-scoped; credentials remain machine-local) ──
    /// Selected AI provider id (empty ⇒ "Select the AI Provider").
    pub llm_provider: String,
    pub llm_endpoint: String,
    pub llm_api_key: String,
    pub llm_model: String,
    pub llm_cobol_proficiency_prompt: String,
    /// Request timeout in seconds (spec 025).
    pub llm_timeout: u32,
    /// Max tokens for AI generation (spec 025).
    pub llm_max_tokens: u32,
    /// Verbose AI activity logging (model info + full context + timings).
    pub llm_verbose: bool,
    pub llm_inspection_port: u16,
    /// Per-model API keys (provider::model -> key), edited alongside the
    /// visible key field and written back on Apply.
    pub llm_api_keys: std::collections::HashMap<String, String>,
    pub llm_reviewer_provider: String,
    pub llm_reviewer_endpoint: String,
    pub llm_reviewer_model: String,
    pub llm_reviewer_api_key: String,
    // ── Integrations (spec 039): Maps + Web Search ──
    /// google_maps crate API key — Directions/Geocoding/Places/Distance-
    /// Matrix only, never the OpenStreetMap basemap tiles (which need none).
    pub maps_api_key: String,
    /// Google Custom Search JSON API key.
    pub custom_search_api_key: String,
    /// Google Custom Search Engine id ("cx") — not a secret (R31/R32).
    pub custom_search_engine_id: String,
}

impl SettingsDraft {
    /// Switch the license model, loading that license's canonical text for the
    /// developer to edit. Text they wrote themselves is kept — only stock text
    /// (or an empty box) is replaced.
    pub fn set_license_model(&mut self, id: &str) {
        if self.license_model == id {
            return;
        }
        if is_stock_license_text(&self.license_text) {
            self.license_text = stock_license_text(id).unwrap_or_default().to_owned();
        }
        self.license_model = id.to_owned();
    }

    pub fn from_project(p: &CoboltProject, llm: &LlmConfig) -> Self {
        let (major, minor, fix) = p.project.version_parts();
        Self {
            name: p.project.name.clone(),
            ver_major: major,
            ver_minor: minor,
            ver_fix: fix,
            main: p.project.main.clone(),
            copyright: p.project.copyright.clone(),
            destination_folder: p.project.destination_folder.clone(),
            debug_compilation: p.project.debug_compilation,
            license_model: p.project.license_model.clone(),
            // A project that names a license but carries no text (older projects,
            // or one whose text was never filled in) gets the canonical text, so
            // the box shows the licence it claims.
            license_text: if p.project.license_text.trim().is_empty() {
                stock_license_text(&p.project.license_model)
                    .unwrap_or_default()
                    .to_owned()
            } else {
                p.project.license_text.clone()
            },
            theme_id: p.ide.theme.clone(),
            bg_image: p.ide.background_image.clone(),
            project_icon: p.ide.project_icon.clone(),
            bg_opacity: p.ide.background_opacity,
            form_theme_id: p.forms.theme.clone(),
            fx_entrance_effect: p.forms.entrance_effect.clone(),
            fx_entrance_ms: p.forms.entrance_ms,
            fx_entrance_easing: p.forms.entrance_easing.clone(),
            fx_exit_effect: p.forms.exit_effect.clone(),
            fx_exit_ms: p.forms.exit_ms,
            fx_exit_easing: p.forms.exit_easing.clone(),
            fx_restore: p.forms.entrance_on_restore,
            fixed_format: p.runtime.fixed_format,
            insp_dump_enabled: p.ide.inspector_dump_enabled,
            insp_dump_path: p.ide.inspector_dump_path.clone(),
            llm_provider: llm.provider.clone(),
            llm_endpoint: llm.endpoint.clone(),
            llm_api_key: llm.api_key.clone(),
            llm_model: llm.model.clone(),
            llm_cobol_proficiency_prompt: if llm.cobol_proficiency_prompt.trim().is_empty() {
                crate::llm::default_cobol_proficiency_prompt()
            } else {
                llm.cobol_proficiency_prompt.clone()
            },
            llm_timeout: llm.timeout_secs,
            llm_max_tokens: llm.max_tokens,
            llm_verbose: llm.verbose_log,
            llm_inspection_port: llm.inspection_port,
            llm_api_keys: llm.api_keys.clone(),
            llm_reviewer_provider: llm.reviewer_provider.clone(),
            llm_reviewer_endpoint: llm.reviewer_endpoint.clone(),
            llm_reviewer_model: llm.reviewer_model.clone(),
            llm_reviewer_api_key: llm.reviewer_config().api_key,
            maps_api_key: llm
                .api_keys
                .get(crate::llm::GOOGLE_MAPS_API_KEY_SLOT)
                .cloned()
                .unwrap_or_default(),
            custom_search_api_key: llm
                .api_keys
                .get(crate::llm::GOOGLE_CUSTOM_SEARCH_API_KEY_SLOT)
                .cloned()
                .unwrap_or_default(),
            custom_search_engine_id: p.integrations.google_search_engine_id.clone(),
        }
    }

    /// Write the draft back into the project + global AI config.
    pub fn apply(&self, p: &mut CoboltProject, llm: &mut LlmConfig) {
        p.project.name = self.name.clone();
        p.project
            .set_version_parts(self.ver_major, self.ver_minor, self.ver_fix);
        p.project.main = self.main.clone();
        p.project.copyright = self.copyright.clone();
        p.project.destination_folder = self.destination_folder.clone();
        p.project.debug_compilation = self.debug_compilation;
        p.project.license_model = self.license_model.clone();
        p.project.license_text = self.license_text.clone();
        p.ide.theme = self.theme_id.clone();
        p.ide.background_image = self.bg_image.clone();
        p.ide.project_icon = self.project_icon.clone();
        p.ide.background_opacity = self.bg_opacity;
        p.forms.theme = self.form_theme_id.clone();
        p.forms.entrance_effect = self.fx_entrance_effect.clone();
        p.forms.entrance_ms = self.fx_entrance_ms;
        p.forms.entrance_easing = self.fx_entrance_easing.clone();
        p.forms.exit_effect = self.fx_exit_effect.clone();
        p.forms.exit_ms = self.fx_exit_ms;
        p.forms.exit_easing = self.fx_exit_easing.clone();
        p.forms.entrance_on_restore = self.fx_restore;
        p.runtime.fixed_format = self.fixed_format;
        p.ide.inspector_dump_enabled = self.insp_dump_enabled;
        p.ide.inspector_dump_path = self.insp_dump_path.clone();
        llm.provider = self.llm_provider.clone();
        llm.endpoint = self.llm_endpoint.clone();
        if !self.llm_api_key.trim().is_empty() {
            llm.api_key = self.llm_api_key.clone();
        }
        // This draft may have been opened before Models Manager saved another
        // profile. Merge non-empty credentials instead of replacing the live
        // map, so saving Project Settings can never erase those newer keys.
        llm.merge_api_keys(&self.llm_api_keys);
        if !self.llm_model.trim().is_empty() && !self.llm_api_key.trim().is_empty() {
            let profile_id = llm.find_or_create_profile(
                &self.llm_provider,
                &self.llm_endpoint,
                &self.llm_model,
                llm.temperature,
                self.llm_max_tokens.max(1),
                self.llm_timeout.max(1),
            );
            llm.store_api_key(
                crate::llm::profile_api_key_slot(&profile_id),
                &self.llm_api_key,
            );
        }
        llm.reviewer_provider = self.llm_reviewer_provider.clone();
        llm.reviewer_endpoint = self.llm_reviewer_endpoint.clone();
        llm.reviewer_model = self.llm_reviewer_model.clone();
        // Hard rule: the reviewer may not be the same provider+model pair as
        // the primary. Persist nothing that violates it.
        if llm.reviewer_provider.trim() == llm.provider.trim()
            && llm.reviewer_model.trim() == llm.model.trim()
        {
            llm.reviewer_model.clear();
        }
        if !self.llm_reviewer_model.trim().is_empty() {
            let profile_id = llm.find_or_create_profile(
                &self.llm_reviewer_provider,
                &self.llm_reviewer_endpoint,
                &self.llm_reviewer_model,
                llm.temperature,
                self.llm_max_tokens.max(1),
                self.llm_timeout.max(1),
            );
            llm.store_api_key(
                crate::llm::profile_api_key_slot(&profile_id),
                &self.llm_reviewer_api_key,
            );
        }
        llm.model = self.llm_model.clone();
        llm.cobol_proficiency_prompt = self.llm_cobol_proficiency_prompt.clone();
        llm.timeout_secs = self.llm_timeout.max(1);
        llm.max_tokens = self.llm_max_tokens.max(1);
        llm.verbose_log = self.llm_verbose;
        llm.inspection_port = if self.llm_inspection_port >= 1024 {
            self.llm_inspection_port
        } else {
            crate::llm::default_inspection_port()
        };
        // Spec 039: same "only overwrite a non-empty edit" rule the LLM key
        // field above uses — leaving the box blank never clears a
        // previously-stored key by accident.
        if !self.maps_api_key.trim().is_empty() {
            llm.store_api_key(
                crate::llm::GOOGLE_MAPS_API_KEY_SLOT.to_owned(),
                &self.maps_api_key,
            );
        }
        if !self.custom_search_api_key.trim().is_empty() {
            llm.store_api_key(
                crate::llm::GOOGLE_CUSTOM_SEARCH_API_KEY_SLOT.to_owned(),
                &self.custom_search_api_key,
            );
        }
        p.integrations.google_search_engine_id = self.custom_search_engine_id.clone();
    }
}

/// What the caller should do after a frame of the form.
#[derive(Default)]
pub struct SettingsFormAction {
    pub save: bool,
    pub test_connection: bool,
    pub test_connection_from_model_selection: bool,
    /// Auto-detect the LLM API/models from the endpoint host (spec 025).
    pub detect_api: bool,
    /// Reopen the read-only LLM debug modal with the last response (spec 025).
    pub show_debug: bool,
    pub browse_bg: bool,
    pub browse_project_icon: bool,
    /// Fetch the selected provider's model list (provider just changed, or the
    /// user clicked the refresh button).
    pub fetch_models: bool,
    pub fetch_reviewer_models: bool,
    pub manage_agents: bool,
    /// Open the Models Manager (spec 031).
    pub manage_models: bool,
}

/// Common license identifiers offered in the dropdown.
const LICENSES: &[&str] = &[
    "Proprietary",
    "MIT",
    "Apache-2.0",
    "GPL-3.0",
    "LGPL-3.0",
    "BSD-3-Clause",
    "MPL-2.0",
    "Unlicense",
    "CC0-1.0",
];

/// Height of the license-text box. Fixed on purpose: the stock texts are long,
/// and a height taken from the available space makes the settings form grow.
const LICENSE_BOX_HEIGHT: f32 = 180.0;

/// Canonical text for a dropdown entry, baked in at build time from the SPDX
/// license list (`assets/licenses/`). The texts are verbatim, placeholders and
/// all (`<year>`, `<copyright holders>`) — the developer fills those in, which
/// is exactly why the box is editable.
///
/// `Proprietary` has no stock text; neither has an unknown id from an older
/// `cobolt.toml`.
fn stock_license_text(id: &str) -> Option<&'static str> {
    Some(match id.trim() {
        "MIT" => include_str!("../../../../assets/licenses/MIT.txt"),
        "Apache-2.0" => include_str!("../../../../assets/licenses/Apache-2.0.txt"),
        "GPL-3.0" => include_str!("../../../../assets/licenses/GPL-3.0-only.txt"),
        "LGPL-3.0" => include_str!("../../../../assets/licenses/LGPL-3.0-only.txt"),
        "BSD-3-Clause" => include_str!("../../../../assets/licenses/BSD-3-Clause.txt"),
        "MPL-2.0" => include_str!("../../../../assets/licenses/MPL-2.0.txt"),
        "Unlicense" => include_str!("../../../../assets/licenses/Unlicense.txt"),
        "CC0-1.0" => include_str!("../../../../assets/licenses/CC0-1.0.txt"),
        _ => return None,
    })
}

/// `true` when the text is untouched stock — empty, or still verbatim equal to
/// one of the shipped licenses. Text the developer wrote or edited is never
/// overwritten by a license switch.
fn is_stock_license_text(text: &str) -> bool {
    let text = text.trim();
    text.is_empty()
        || LICENSES
            .iter()
            .filter_map(|id| stock_license_text(id))
            .any(|stock| stock.trim() == text)
}

/// Holds the live draft + the last-saved baseline for the dirty check.
pub struct SettingsForm {
    pub draft: SettingsDraft,
    baseline: SettingsDraft,
    /// COBOL-aware editor for the COBOL proficiency benchmark prompt. The prompt
    /// is stored as plain text, but this gives embedded RustCOBOL examples the
    /// same IntelliSense surface used by the event handler editor.
    cobol_proficiency_prompt_editor: EditorPanel,
    /// Width of the label column; user can drag the resizer to adjust.
    splitter: f32,
    /// Models offered in the Model picker for the selected provider. Transient
    /// (not part of the dirty check); (re)populated whenever a provider is
    /// chosen or the model list is refreshed. Empty until a provider is picked.
    pub available_models: Vec<String>,
    pub available_reviewer_models: Vec<String>,
    reviewer_same_model_error: bool,
    /// 038 R6 — when Some, the effect preview is playing; the value is the
    /// `ctx.input(|i| i.time)` second it started (entrance → hold → exit).
    fx_preview_started: Option<f64>,
}

impl SettingsForm {
    pub fn new(p: &CoboltProject, llm: &LlmConfig) -> Self {
        let draft = SettingsDraft::from_project(p, llm);
        let mut cobol_proficiency_prompt_editor = EditorPanel::new();
        cobol_proficiency_prompt_editor.open_buffer(
            std::path::PathBuf::from("agentic_ai/cobol-proficiency-prompt.md"),
            draft.llm_cobol_proficiency_prompt.clone(),
        );
        cobol_proficiency_prompt_editor.set_context_only_completions(true);
        Self {
            baseline: draft.clone(),
            draft,
            cobol_proficiency_prompt_editor,
            splitter: 200.0,
            available_models: Vec::new(),
            available_reviewer_models: Vec::new(),
            reviewer_same_model_error: false,
            fx_preview_started: None,
        }
    }

    /// Re-seed both draft and baseline (e.g. after loading a different project).
    pub fn reset_to(&mut self, p: &CoboltProject, llm: &LlmConfig) {
        self.draft = SettingsDraft::from_project(p, llm);
        self.baseline = self.draft.clone();
        self.sync_cobol_proficiency_prompt_editor_from_draft();
        self.available_models.clear();
        // keep user's preferred splitter position
    }

    /// Replace the offered model list (called after a background fetch resolves).
    pub fn set_available_reviewer_models(&mut self, models: Vec<String>) {
        self.available_reviewer_models = models;
    }

    pub fn set_available_models(&mut self, models: Vec<String>) {
        self.available_models = models;
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
        self.sync_cobol_proficiency_prompt_editor_from_draft();
    }

    /// Push the just-picked background-image path into the draft.
    pub fn set_bg_image(&mut self, path: String) {
        self.draft.bg_image = path;
    }

    /// Push the just-picked project icon path into the draft.
    pub fn set_project_icon(&mut self, path: String) {
        self.draft.project_icon = path;
    }

    /// Reload the embedded prompt editor after the draft changes outside it
    /// (Cancel, project switch, or prompt template seeding).
    pub fn sync_cobol_proficiency_prompt_editor_from_draft(&mut self) {
        if self.cobol_proficiency_prompt_editor.buffer_content()
            != Some(self.draft.llm_cobol_proficiency_prompt.as_str())
        {
            self.cobol_proficiency_prompt_editor.open_buffer(
                std::path::PathBuf::from("agentic_ai/cobol-proficiency-prompt.md"),
                self.draft.llm_cobol_proficiency_prompt.clone(),
            );
            self.cobol_proficiency_prompt_editor
                .set_context_only_completions(true);
        }
    }

    /// Render the form. Returns the action(s) the caller must perform.
    pub fn show(
        &mut self,
        ui: &mut Ui,
        tr: &Tr,
        themes: &[(&'static str, &'static str)], // (id, display name)
        test_busy: bool,
        test_status: Option<&str>,
        has_debug: bool,
        known_controls: &[KnownControl],
        fx_disabled: bool,
    ) -> SettingsFormAction {
        let mut action = SettingsFormAction::default();
        self.cobol_proficiency_prompt_editor.known_controls = known_controls.to_vec();
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
                ui.heading(tr.settings_pane_title);
            });
            ui.add_space(right_padding);
        });

        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
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
                            let left_rect = ui
                                .allocate_exact_size(
                                    egui::vec2(splitter, 0.0),
                                    egui::Sense::hover(),
                                )
                                .0;
                            ui.scope_builder(egui::UiBuilder::new().max_rect(left_rect), |ui| {
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
                            let left_rect = ui
                                .allocate_exact_size(
                                    egui::vec2(splitter, 0.0),
                                    egui::Sense::hover(),
                                )
                                .0;
                            ui.scope_builder(egui::UiBuilder::new().max_rect(left_rect), |ui| {
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
                                ui.add(
                                    egui::TextEdit::singleline(&mut self.draft.name)
                                        .desired_width(w),
                                );
                            });
                        });

                        // Version (the drag values row on right is treated as the "value" for the Version label)
                        ui.horizontal_top(|ui| {
                            let left_rect = ui
                                .allocate_exact_size(
                                    egui::vec2(splitter, 0.0),
                                    egui::Sense::hover(),
                                )
                                .0;
                            ui.scope_builder(egui::UiBuilder::new().max_rect(left_rect), |ui| {
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
                                    ui.add(
                                        egui::DragValue::new(&mut self.draft.ver_major)
                                            .range(0..=9999),
                                    );
                                    ui.label(".");
                                    ui.add(
                                        egui::DragValue::new(&mut self.draft.ver_minor)
                                            .range(0..=9999),
                                    );
                                    ui.label(".");
                                    ui.add(
                                        egui::DragValue::new(&mut self.draft.ver_fix)
                                            .range(0..=9999),
                                    );
                                });
                            });
                        });

                        // Main program
                        ui.horizontal_top(|ui| {
                            let left_rect = ui
                                .allocate_exact_size(
                                    egui::vec2(splitter, 0.0),
                                    egui::Sense::hover(),
                                )
                                .0;
                            ui.scope_builder(egui::UiBuilder::new().max_rect(left_rect), |ui| {
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
                                ui.add(
                                    egui::TextEdit::singleline(&mut self.draft.main)
                                        .desired_width(w),
                                );
                            });
                        });

                        // Copyright
                        ui.horizontal_top(|ui| {
                            let left_rect = ui
                                .allocate_exact_size(
                                    egui::vec2(splitter, 0.0),
                                    egui::Sense::hover(),
                                )
                                .0;
                            ui.scope_builder(egui::UiBuilder::new().max_rect(left_rect), |ui| {
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
                                ui.add(
                                    egui::TextEdit::singleline(&mut self.draft.copyright)
                                        .hint_text("© 2026 …")
                                        .desired_width(w),
                                );
                            });
                        });

                        // Destination Folder
                        ui.horizontal_top(|ui| {
                            let left_rect = ui
                                .allocate_exact_size(
                                    egui::vec2(splitter, 0.0),
                                    egui::Sense::hover(),
                                )
                                .0;
                            ui.scope_builder(egui::UiBuilder::new().max_rect(left_rect), |ui| {
                                ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Truncate);
                                ui.set_min_width(splitter);
                                ui.add_space(property_indent);
                                ui.add(egui::Label::new(tr.lbl_destination_folder).truncate());
                            });
                            ui.allocate_space(egui::vec2(resizer_width, 0.0));
                            ui.add_space(gap_after_resizer);
                            let right_w = ui.available_width();
                            ui.allocate_ui(egui::vec2(right_w, 0.0), |ui| {
                                let w = ui.available_width();
                                ui.add(
                                    egui::TextEdit::singleline(&mut self.draft.destination_folder)
                                        .desired_width(w),
                                );
                            });
                        });

                        // Debug Compilation
                        ui.horizontal_top(|ui| {
                            let left_rect = ui
                                .allocate_exact_size(
                                    egui::vec2(splitter, 0.0),
                                    egui::Sense::hover(),
                                )
                                .0;
                            ui.scope_builder(egui::UiBuilder::new().max_rect(left_rect), |ui| {
                                ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Truncate);
                                ui.set_min_width(splitter);
                                ui.add_space(property_indent);
                                ui.add(egui::Label::new(tr.lbl_debug_compilation).truncate());
                            });
                            ui.allocate_space(egui::vec2(resizer_width, 0.0));
                            ui.add_space(gap_after_resizer);
                            let right_w = ui.available_width();
                            ui.allocate_ui(egui::vec2(right_w, 0.0), |ui| {
                                ui.checkbox(&mut self.draft.debug_compilation, "");
                            });
                        });

                        ui.add_space(8.0);

                        // --- License section header (left only)
                        ui.horizontal_top(|ui| {
                            let left_rect = ui
                                .allocate_exact_size(
                                    egui::vec2(splitter, 0.0),
                                    egui::Sense::hover(),
                                )
                                .0;
                            ui.scope_builder(egui::UiBuilder::new().max_rect(left_rect), |ui| {
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
                            let left_rect = ui
                                .allocate_exact_size(
                                    egui::vec2(splitter, 0.0),
                                    egui::Sense::hover(),
                                )
                                .0;
                            ui.scope_builder(egui::UiBuilder::new().max_rect(left_rect), |ui| {
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
                                    } else {
                                        self.draft.license_model.clone()
                                    })
                                    .width(w)
                                    .show_ui(ui, |ui| {
                                        // Route the pick through set_license_model
                                        // so choosing a license also loads its text.
                                        let mut picked: Option<&str> = None;
                                        for &lic in LICENSES {
                                            if ui
                                                .selectable_label(
                                                    self.draft.license_model == lic,
                                                    lic,
                                                )
                                                .clicked()
                                            {
                                                picked = Some(lic);
                                            }
                                        }
                                        if let Some(lic) = picked {
                                            self.draft.set_license_model(lic);
                                        }
                                    });
                            });
                        });

                        // License text (multiline on right determines row height; label is top-aligned to it)
                        ui.horizontal_top(|ui| {
                            let left_rect = ui
                                .allocate_exact_size(
                                    egui::vec2(splitter, 0.0),
                                    egui::Sense::hover(),
                                )
                                .0;
                            ui.scope_builder(egui::UiBuilder::new().max_rect(left_rect), |ui| {
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
                                // A stock license runs to tens of thousands of
                                // lines (GPL-3.0), and a multiline TextEdit grows
                                // to its content — so it scrolls inside a box of
                                // fixed height (a constant, never derived from the
                                // available space, which would inflate the form).
                                egui::ScrollArea::vertical()
                                    .id_salt("license_text")
                                    .max_height(LICENSE_BOX_HEIGHT)
                                    .show(ui, |ui| {
                                        ui.add(
                                            egui::TextEdit::multiline(
                                                &mut self.draft.license_text,
                                            )
                                            .desired_rows(5)
                                            .desired_width(w)
                                            .font(egui::TextStyle::Monospace),
                                        );
                                    });
                            });
                        });

                        ui.add_space(8.0);

                        // --- Appearance section header (left only)
                        ui.horizontal_top(|ui| {
                            let left_rect = ui
                                .allocate_exact_size(
                                    egui::vec2(splitter, 0.0),
                                    egui::Sense::hover(),
                                )
                                .0;
                            ui.scope_builder(egui::UiBuilder::new().max_rect(left_rect), |ui| {
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
                            let left_rect = ui
                                .allocate_exact_size(
                                    egui::vec2(splitter, 0.0),
                                    egui::Sense::hover(),
                                )
                                .0;
                            ui.scope_builder(egui::UiBuilder::new().max_rect(left_rect), |ui| {
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
                                let cur = themes
                                    .iter()
                                    .find(|(id, _)| *id == self.draft.theme_id)
                                    .map(|(_, n)| *n)
                                    .unwrap_or(themes.first().map(|(_, n)| *n).unwrap_or(""));
                                egui::ComboBox::from_id_salt("theme_pick")
                                    .selected_text(cur)
                                    .width(w)
                                    .show_ui(ui, |ui| {
                                        for (id, name) in themes {
                                            ui.selectable_value(
                                                &mut self.draft.theme_id,
                                                (*id).to_owned(),
                                                *name,
                                            );
                                        }
                                    });
                            });
                        });

                        // Default form theme (spec 007) — the picker is **hidden for now**:
                        // only Liquid Glass ships as a finished look; the special asset
                        // packs (cobalt-steel, …) need more fidelity work before they are
                        // offered. The model field (`forms.theme`) and the theme engine are
                        // retained, so re-enabling is just restoring this row.

                        // ── Window effects (spec 038) — project-level, all forms ──────
                        {
                            use cobolt_forms::window_fx::{Easing, FxSpec, WindowEffect};
                            let mut fx_row = |label: &str,
                                              effect_id: &mut String,
                                              ms: &mut u32,
                                              easing_id: &mut String,
                                              salt: &str,
                                              ui: &mut Ui| {
                                ui.horizontal_top(|ui| {
                                    let left_rect = ui
                                        .allocate_exact_size(
                                            egui::vec2(splitter, 0.0),
                                            egui::Sense::hover(),
                                        )
                                        .0;
                                    ui.scope_builder(
                                        egui::UiBuilder::new().max_rect(left_rect),
                                        |ui| {
                                            ui.style_mut().wrap_mode =
                                                Some(egui::TextWrapMode::Truncate);
                                            ui.set_min_width(splitter);
                                            ui.add_space(property_indent);
                                            ui.add(egui::Label::new(label).truncate());
                                        },
                                    );
                                    ui.allocate_space(egui::vec2(resizer_width, 0.0));
                                    ui.add_space(gap_after_resizer);
                                    let right_w = ui.available_width();
                                    ui.allocate_ui(egui::vec2(right_w, 0.0), |ui| {
                                        ui.horizontal(|ui| {
                                            let cur = WindowEffect::from_str(effect_id);
                                            egui::ComboBox::from_id_salt(("fx_effect", salt))
                                                .selected_text(cur.as_str())
                                                .width(150.0)
                                                .show_ui(ui, |ui| {
                                                    for e in WindowEffect::ALL {
                                                        if ui
                                                            .selectable_label(
                                                                cur == e,
                                                                e.as_str(),
                                                            )
                                                            .clicked()
                                                        {
                                                            *effect_id =
                                                                e.as_str().to_owned();
                                                            // Effects own their duration
                                                            // bounds (matrix-rain runs
                                                            // 1500–4000 ms).
                                                            let (mn, mx) =
                                                                e.duration_bounds();
                                                            *ms = (*ms).clamp(mn, mx);
                                                        }
                                                    }
                                                });
                                            let (min_ms, max_ms) = cur.duration_bounds();
                                            ui.add(
                                                egui::DragValue::new(ms)
                                                    .range(min_ms..=max_ms)
                                                    .suffix(" ms"),
                                            );
                                            let cur_e = Easing::from_str(easing_id);
                                            egui::ComboBox::from_id_salt(("fx_easing", salt))
                                                .selected_text(cur_e.as_str())
                                                .width(110.0)
                                                .show_ui(ui, |ui| {
                                                    for e in [
                                                        Easing::Linear,
                                                        Easing::EaseIn,
                                                        Easing::EaseOut,
                                                        Easing::EaseInOut,
                                                    ] {
                                                        if ui
                                                            .selectable_label(
                                                                cur_e == e,
                                                                e.as_str(),
                                                            )
                                                            .clicked()
                                                        {
                                                            *easing_id =
                                                                e.as_str().to_owned();
                                                        }
                                                    }
                                                });
                                        });
                                    });
                                });
                            };
                            fx_row(
                                tr.set_fx_entrance,
                                &mut self.draft.fx_entrance_effect,
                                &mut self.draft.fx_entrance_ms,
                                &mut self.draft.fx_entrance_easing,
                                "ent",
                                ui,
                            );
                            fx_row(
                                tr.set_fx_exit,
                                &mut self.draft.fx_exit_effect,
                                &mut self.draft.fx_exit_ms,
                                &mut self.draft.fx_exit_easing,
                                "exit",
                                ui,
                            );
                            // Restore replay + Preview on one row.
                            ui.horizontal_top(|ui| {
                                let left_rect = ui
                                    .allocate_exact_size(
                                        egui::vec2(splitter, 0.0),
                                        egui::Sense::hover(),
                                    )
                                    .0;
                                ui.scope_builder(
                                    egui::UiBuilder::new().max_rect(left_rect),
                                    |ui| {
                                        ui.style_mut().wrap_mode =
                                            Some(egui::TextWrapMode::Truncate);
                                        ui.set_min_width(splitter);
                                        ui.add_space(property_indent);
                                        ui.add(
                                            egui::Label::new(tr.set_fx_restore).truncate(),
                                        );
                                    },
                                );
                                ui.allocate_space(egui::vec2(resizer_width, 0.0));
                                ui.add_space(gap_after_resizer);
                                ui.horizontal(|ui| {
                                    ui.checkbox(&mut self.draft.fx_restore, "");
                                    let resp = ui.add_enabled(
                                        !fx_disabled,
                                        egui::Button::new(tr.set_fx_preview),
                                    );
                                    if fx_disabled {
                                        resp.on_disabled_hover_text(tr.set_fx_disabled_hint);
                                    } else if resp.clicked() {
                                        self.fx_preview_started =
                                            Some(ui.ctx().input(|i| i.time));
                                    }
                                });
                            });
                            // Preview canvas: entrance → short hold → exit, over a
                            // placeholder card face (same window_fx engine as the host).
                            if let Some(started) = self.fx_preview_started {
                                // Clamp into each effect's bounds so a draft loaded
                                // from an out-of-band project previews what will run.
                                let ent_effect =
                                    WindowEffect::from_str(&self.draft.fx_entrance_effect);
                                let (ent_mn, ent_mx) = ent_effect.duration_bounds();
                                let ent = FxSpec {
                                    effect: ent_effect,
                                    duration_ms: self.draft.fx_entrance_ms.clamp(ent_mn, ent_mx),
                                    easing: Easing::from_str(&self.draft.fx_entrance_easing),
                                };
                                let exit_effect =
                                    WindowEffect::from_str(&self.draft.fx_exit_effect);
                                let (exit_mn, exit_mx) = exit_effect.duration_bounds();
                                let exit = FxSpec {
                                    effect: exit_effect,
                                    duration_ms: self.draft.fx_exit_ms.clamp(exit_mn, exit_mx),
                                    easing: Easing::from_str(&self.draft.fx_exit_easing),
                                };
                                let now = ui.ctx().input(|i| i.time);
                                let el = now - started;
                                // The preview card's width, known before the
                                // rect is allocated: MatrixRain schedules its
                                // lines in real milliseconds across it, and
                                // takes as long as that schedule needs.
                                let card_w =
                                    (content_w - splitter - 40.0).clamp(220.0, 460.0);
                                let fx_ms = |spec: &FxSpec| -> u32 {
                                    if spec.effect == WindowEffect::MatrixRain {
                                        cobolt_forms::window_fx::matrix_effective_duration_ms(
                                            card_w,
                                            spec.duration_ms,
                                        )
                                    } else {
                                        spec.duration_ms
                                    }
                                };
                                let ent_ms = fx_ms(&ent);
                                let exit_ms = fx_ms(&exit);
                                let ent_d = ent_ms.max(1) as f64 / 1000.0;
                                let hold = 0.4_f64;
                                let exit_d = exit_ms.max(1) as f64 / 1000.0;
                                let phase = if ent.is_active() && el < ent_d {
                                    Some((
                                        ent.effect,
                                        ent.effect.progress(ent.easing, (el / ent_d) as f32),
                                    ))
                                } else if el < ent_d + hold {
                                    Some((WindowEffect::None, 1.0))
                                } else if exit.is_active() && el < ent_d + hold + exit_d {
                                    let back = 1.0 - ((el - ent_d - hold) / exit_d) as f32;
                                    Some((exit.effect, exit.effect.progress(exit.easing, back)))
                                } else {
                                    None
                                };
                                match phase {
                                    None => self.fx_preview_started = None,
                                    Some((effect, t)) => {
                                        let (rect, _) = ui.allocate_exact_size(
                                            egui::vec2(card_w, 150.0),
                                            egui::Sense::hover(),
                                        );
                                        let painter = ui.painter().with_clip_rect(rect);
                                        let card_bg = egui::Color32::from_rgb(46, 52, 64);
                                        cobolt_forms::window_fx::paint_window_fx(
                                            &painter,
                                            rect,
                                            card_bg,
                                            t,
                                            effect,
                                            0xC0FFEE,
                                            now,
                                            // The preview card sits on the
                                            // settings pane, not on a
                                            // see-through window.
                                            false,
                                            if matches!(phase, Some((e, _)) if e == exit.effect) {
                                                exit_ms
                                            } else {
                                                ent_ms
                                            },
                                            &mut |p, r| {
                                                p.rect_filled(r, 4.0, card_bg);
                                                let title = egui::Rect::from_min_size(
                                                    r.min,
                                                    egui::vec2(r.width(), r.height() * 0.14),
                                                );
                                                p.rect_filled(
                                                    title,
                                                    4.0,
                                                    egui::Color32::from_rgb(59, 66, 82),
                                                );
                                                let btn = egui::Rect::from_center_size(
                                                    r.center() + egui::vec2(0.0, r.height() * 0.2),
                                                    egui::vec2(r.width() * 0.3, r.height() * 0.16),
                                                );
                                                p.rect_filled(
                                                    btn,
                                                    4.0,
                                                    egui::Color32::from_rgb(94, 129, 172),
                                                );
                                                p.rect_filled(
                                                    egui::Rect::from_center_size(
                                                        r.center()
                                                            - egui::vec2(0.0, r.height() * 0.12),
                                                        egui::vec2(
                                                            r.width() * 0.6,
                                                            r.height() * 0.1,
                                                        ),
                                                    ),
                                                    3.0,
                                                    egui::Color32::from_rgb(76, 86, 106),
                                                );
                                            },
                                        );
                                        ui.ctx().request_repaint();
                                    }
                                }
                            }
                        }

                        // Background image (the button row + shown path is the "value")
                        ui.horizontal_top(|ui| {
                            let left_rect = ui
                                .allocate_exact_size(
                                    egui::vec2(splitter, 0.0),
                                    egui::Sense::hover(),
                                )
                                .0;
                            ui.scope_builder(egui::UiBuilder::new().max_rect(left_rect), |ui| {
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
                                    if ui.button(tr.settings_bg_browse).clicked() {
                                        action.browse_bg = true;
                                    }
                                    let shown = if self.draft.bg_image.is_empty() {
                                        tr.settings_bg_none.to_owned()
                                    } else {
                                        self.draft.bg_image.clone()
                                    };
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
                            let left_rect = ui
                                .allocate_exact_size(
                                    egui::vec2(splitter, 0.0),
                                    egui::Sense::hover(),
                                )
                                .0;
                            ui.scope_builder(egui::UiBuilder::new().max_rect(left_rect), |ui| {
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
                                if ui
                                    .add(egui::Slider::new(&mut o, 0..=100).suffix("%"))
                                    .changed()
                                {
                                    self.draft.bg_opacity = o.clamp(0, 100) as u8;
                                }
                            });
                        });

                        // Project icon (used by Run Form / packaged app windows)
                        ui.horizontal_top(|ui| {
                            let left_rect = ui
                                .allocate_exact_size(
                                    egui::vec2(splitter, 0.0),
                                    egui::Sense::hover(),
                                )
                                .0;
                            ui.scope_builder(egui::UiBuilder::new().max_rect(left_rect), |ui| {
                                ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Truncate);
                                ui.set_min_width(splitter);
                                ui.add_space(property_indent);
                                ui.add(egui::Label::new("Project icon").truncate());
                            });
                            ui.allocate_space(egui::vec2(resizer_width, 0.0));
                            ui.add_space(gap_after_resizer);
                            let right_w = ui.available_width();
                            ui.allocate_ui(egui::vec2(right_w, 0.0), |ui| {
                                ui.horizontal(|ui| {
                                    if ui.button("Select image...").clicked() {
                                        action.browse_project_icon = true;
                                    }
                                    let shown = if self.draft.project_icon.is_empty() {
                                        "No icon".to_owned()
                                    } else {
                                        self.draft.project_icon.clone()
                                    };
                                    ui.label(RichText::new(shown).small().monospace());
                                    if !self.draft.project_icon.is_empty()
                                        && ui.button(tr.settings_bg_clear).clicked()
                                    {
                                        self.draft.project_icon.clear();
                                    }
                                });
                            });
                        });

                        ui.add_space(8.0);

                        // --- AI assistant section header (left only)
                        ui.horizontal_top(|ui| {
                            let left_rect = ui
                                .allocate_exact_size(
                                    egui::vec2(splitter, 0.0),
                                    egui::Sense::hover(),
                                )
                                .0;
                            ui.scope_builder(egui::UiBuilder::new().max_rect(left_rect), |ui| {
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
                            let left_rect = ui
                                .allocate_exact_size(
                                    egui::vec2(splitter, 0.0),
                                    egui::Sense::hover(),
                                )
                                .0;
                            ui.scope_builder(egui::UiBuilder::new().max_rect(left_rect), |ui| {
                                ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Truncate);
                                ui.set_min_width(splitter);
                                // empty left side for the hint row
                            });
                            ui.allocate_space(egui::vec2(resizer_width, 0.0));
                            ui.add_space(gap_after_resizer);
                            let right_w = ui.available_width();
                            ui.allocate_ui(egui::vec2(right_w, 0.0), |ui| {
                                ui.label(
                                    RichText::new(tr.settings_ai_hint)
                                        .small()
                                        .color(theme.text_dim),
                                );
                            });
                        });

                        // Provider (drives the default endpoint + the model list)
                        // ── Agents Manager (spec 028): the agent database UI ───
                        ui.horizontal_top(|ui| {
                            let left_rect = ui
                                .allocate_exact_size(
                                    egui::vec2(splitter, 0.0),
                                    egui::Sense::hover(),
                                )
                                .0;
                            ui.scope_builder(egui::UiBuilder::new().max_rect(left_rect), |ui| {
                                ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Truncate);
                                ui.set_min_width(splitter);
                                ui.add_space(property_indent);
                                ui.add(egui::Label::new(tr.agents_row_label).truncate());
                            });
                            ui.allocate_space(egui::vec2(resizer_width, 0.0));
                            ui.add_space(gap_after_resizer);
                            let right_w = ui.available_width();
                            ui.allocate_ui(egui::vec2(right_w, 0.0), |ui| {
                                ui.horizontal(|ui| {
                                    if ui.button(tr.agents_manage).clicked() {
                                        action.manage_agents = true;
                                    }
                                    if ui.button(tr.models_manage).clicked() {
                                        action.manage_models = true;
                                    }
                                });
                            });
                        });

                        ui.add_space(8.0);

                        // Legacy per-agent connection fields (provider, endpoint,
                        // model, API key, reviewer, proficiency prompt, verbose).
                        // Spec 028/029: this configuration now lives PER AGENT in
                        // the Agents Manager (seeded from any prior config on first
                        // open), so the AI section is just the "Agents Manager…"
                        // button above plus the non-agent inspection port below.
                        // The draft fields are still loaded/saved so nothing is
                        // orphaned; only the UI is retired.
                        const SHOW_LEGACY_AI_FIELDS: bool = false;
                        if SHOW_LEGACY_AI_FIELDS {
                        ui.horizontal_top(|ui| {
                            let left_rect = ui
                                .allocate_exact_size(
                                    egui::vec2(splitter, 0.0),
                                    egui::Sense::hover(),
                                )
                                .0;
                            ui.scope_builder(egui::UiBuilder::new().max_rect(left_rect), |ui| {
                                ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Truncate);
                                ui.set_min_width(splitter);
                                ui.add_space(property_indent);
                                ui.add(egui::Label::new(tr.settings_ai_provider).truncate());
                            });
                            ui.allocate_space(egui::vec2(resizer_width, 0.0));
                            ui.add_space(gap_after_resizer);
                            let right_w = ui.available_width();
                            ui.allocate_ui(egui::vec2(right_w, 0.0), |ui| {
                                let w = ui.available_width();
                                let prev = self.draft.llm_provider.clone();
                                let cur_label =
                                    crate::llm::Provider::from_id(&self.draft.llm_provider)
                                        .map(|p| p.label().to_owned())
                                        .unwrap_or_else(|| {
                                            tr.settings_ai_provider_select.to_owned()
                                        });
                                egui::ComboBox::from_id_salt("ai_provider")
                                    .selected_text(cur_label)
                                    .width(w)
                                    .show_ui(ui, |ui| {
                                        for p in crate::llm::PROVIDERS {
                                            ui.selectable_value(
                                                &mut self.draft.llm_provider,
                                                p.id().to_owned(),
                                                p.label(),
                                            );
                                        }
                                    });
                                // React to a provider change: fill the default
                                // endpoint + recommended prompt, clear the model,
                                // and kick off a live model-list fetch.
                                if self.draft.llm_provider != prev {
                                    if let Some(p) = crate::llm::Provider::from_id(
                                        &self.draft.llm_provider,
                                    ) {
                                        self.draft.llm_endpoint =
                                            p.default_endpoint().to_owned();
                                        // The system prompt is (re)loaded from the
                                        // project's agentic_ai/system-prompt.md by
                                        // the caller when empty — never seeded or
                                        // overwritten here, so a developer's edit is
                                        // preserved.
                                        // Remember the key for the provider/model we
                                        // are leaving, then clear the visible field —
                                        // the new provider has no model selected yet,
                                        // so no stored key applies (and a stale key
                                        // must not look valid).
                                        if !self.draft.llm_model.trim().is_empty()
                                            && !self.draft.llm_api_key.trim().is_empty()
                                        {
                                            self.draft.llm_api_keys.insert(
                                                crate::llm::api_key_slot(
                                                    &prev,
                                                    &self.draft.llm_model,
                                                ),
                                                self.draft.llm_api_key.clone(),
                                            );
                                        }
                                        self.draft.llm_api_key.clear();
                                        self.draft.llm_model.clear();
                                        self.available_models.clear();
                                        action.fetch_models = true;
                                    }
                                }
                            });
                        });

                        // Endpoint
                        ui.horizontal_top(|ui| {
                            let left_rect = ui
                                .allocate_exact_size(
                                    egui::vec2(splitter, 0.0),
                                    egui::Sense::hover(),
                                )
                                .0;
                            ui.scope_builder(egui::UiBuilder::new().max_rect(left_rect), |ui| {
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
                                ui.add(
                                    egui::TextEdit::singleline(&mut self.draft.llm_endpoint)
                                        .hint_text("https://…/v1/chat/completions or /v1/responses")
                                        .desired_width(w),
                                );
                            });
                        });

                        // API key
                        ui.horizontal_top(|ui| {
                            let left_rect = ui
                                .allocate_exact_size(
                                    egui::vec2(splitter, 0.0),
                                    egui::Sense::hover(),
                                )
                                .0;
                            ui.scope_builder(egui::UiBuilder::new().max_rect(left_rect), |ui| {
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
                                ui.add(
                                    egui::TextEdit::singleline(&mut self.draft.llm_api_key)
                                        .password(true)
                                        .desired_width(w),
                                );
                            });
                        });

                        // Model
                        ui.horizontal_top(|ui| {
                            let left_rect = ui
                                .allocate_exact_size(
                                    egui::vec2(splitter, 0.0),
                                    egui::Sense::hover(),
                                )
                                .0;
                            ui.scope_builder(egui::UiBuilder::new().max_rect(left_rect), |ui| {
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
                                ui.horizontal(|ui| {
                                    let has_provider = crate::llm::Provider::from_id(
                                        &self.draft.llm_provider,
                                    )
                                    .is_some();
                                    let combo_w = (w - 104.0).max(60.0);
                                    let models = self.available_models.clone();
                                    let prev_model = self.draft.llm_model.clone();
                                    egui::ComboBox::from_id_salt("ai_model")
                                        .selected_text(if self.draft.llm_model.trim().is_empty() {
                                            tr.settings_ai_model_empty.to_string()
                                        } else {
                                            self.draft.llm_model.clone()
                                        })
                                        .width(combo_w)
                                        .height(250.0)
                                        .show_ui(ui, |ui| {
                                            ui.set_min_width(combo_w);
                                            ui.spacing_mut().item_spacing.y = 4.0;
                                            ui.spacing_mut().interact_size.y = 32.0;
                                            if models.is_empty() {
                                                ui.weak(tr.settings_ai_model_empty);
                                            } else {
                                                for model in models {
                                                    ui.selectable_value(
                                                        &mut self.draft.llm_model,
                                                        model.clone(),
                                                        model,
                                                    );
                                                }
                                            }
                                        });
                                    if self.draft.llm_model != prev_model {
                                        // Remember the key typed for the model we
                                        // are leaving, then restore the stored key
                                        // for the newly selected model — or clear
                                        // the field, so a leftover key never looks
                                        // like a valid credential for this model.
                                        let provider = self.draft.llm_provider.clone();
                                        if !prev_model.trim().is_empty() {
                                            let prev_slot =
                                                crate::llm::api_key_slot(&provider, &prev_model);
                                            if self.draft.llm_api_key.trim().is_empty() {
                                                self.draft.llm_api_keys.remove(&prev_slot);
                                            } else {
                                                self.draft.llm_api_keys.insert(
                                                    prev_slot,
                                                    self.draft.llm_api_key.clone(),
                                                );
                                            }
                                        }
                                        let slot = crate::llm::api_key_slot(
                                            &provider,
                                            &self.draft.llm_model,
                                        );
                                        self.draft.llm_api_key = self
                                            .draft
                                            .llm_api_keys
                                            .get(&slot)
                                            .cloned()
                                            .unwrap_or_default();
                                        action.test_connection = true;
                                        action.test_connection_from_model_selection = true;
                                    }
                                    if ui
                                        .add_enabled(
                                            has_provider && !test_busy,
                                            egui::Button::new(tr.settings_ai_refresh),
                                        )
                                        .on_hover_text(tr.settings_ai_refresh_models)
                                        .clicked()
                                    {
                                        action.fetch_models = true;
                                    }
                                });
                            });
                        });

                        // Test button row (no paired left label) — placed directly
                        // above the system prompt so the connection controls sit
                        // with the endpoint/model fields they act on.
                        ui.horizontal_top(|ui| {
                            let left_rect = ui
                                .allocate_exact_size(
                                    egui::vec2(splitter, 0.0),
                                    egui::Sense::hover(),
                                )
                                .0;
                            ui.scope_builder(egui::UiBuilder::new().max_rect(left_rect), |ui| {
                                ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Truncate);
                                ui.set_min_width(splitter);
                            });
                            ui.allocate_space(egui::vec2(resizer_width, 0.0));
                            ui.add_space(gap_after_resizer);
                            let right_w = ui.available_width();
                            ui.allocate_ui(egui::vec2(right_w, 0.0), |ui| {
                                ui.horizontal(|ui| {
                                    if ui
                                        .add_enabled(
                                            !test_busy,
                                            egui::Button::new(tr.settings_ai_detect),
                                        )
                                        .on_hover_text(tr.settings_ai_detect_hint)
                                        .clicked()
                                    {
                                        action.detect_api = true;
                                    }
                                    if ui
                                        .add_enabled(
                                            !test_busy,
                                            egui::Button::new(tr.settings_ai_test),
                                        )
                                        .clicked()
                                    {
                                        action.test_connection = true;
                                    }
                                    if has_debug
                                        && ui.button(tr.agent_details).clicked()
                                    {
                                        action.show_debug = true;
                                    }
                                    ui.separator();
                                    ui.label(RichText::new("Timeout (s)").small());
                                    ui.add(
                                        egui::DragValue::new(&mut self.draft.llm_timeout)
                                            .speed(1.0)
                                            .range(1..=1200),
                                    )
                                    .on_hover_text(tr.settings_ai_timeout_hint);
                                    ui.add_space(8.0);
                                    ui.label(RichText::new("Max Tokens").small());
                                    ui.add(
                                        egui::DragValue::new(&mut self.draft.llm_max_tokens)
                                            .speed(100.0)
                                            .range(256..=128000),
                                    );
                                    if let Some(s) = test_status {
                                        ui.label(RichText::new(s).small());
                                    }
                                });
                            });
                        });

                        // ── Pedantic reviewer model (optional second model) ──
                        ui.add_space(10.0);
                        ui.horizontal_top(|ui| {
                            let left_rect = ui
                                .allocate_exact_size(
                                    egui::vec2(splitter, 0.0),
                                    egui::Sense::hover(),
                                )
                                .0;
                            ui.scope_builder(egui::UiBuilder::new().max_rect(left_rect), |ui| {
                                ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Truncate);
                                ui.set_min_width(splitter);
                                ui.add_space(property_indent);
                                ui.add(
                                    egui::Label::new(
                                        RichText::new(tr.settings_ai_reviewer_section).strong(),
                                    )
                                    .truncate(),
                                )
                                .on_hover_text(tr.settings_ai_reviewer_hint);
                            });
                            ui.allocate_space(egui::vec2(resizer_width, 0.0));
                            ui.add_space(gap_after_resizer);
                            let right_w = ui.available_width();
                            ui.allocate_ui(egui::vec2(right_w, 0.0), |ui| {
                                ui.vertical(|ui| {
                                    // Provider + endpoint
                                    ui.horizontal(|ui| {
                                        let prev_p = self.draft.llm_reviewer_provider.clone();
                                        egui::ComboBox::from_id_salt("ai_reviewer_provider")
                                            .selected_text(if prev_p.trim().is_empty() {
                                                "—".to_string()
                                            } else {
                                                prev_p.clone()
                                            })
                                            .width(140.0)
                                            .show_ui(ui, |ui| {
                                                ui.selectable_value(
                                                    &mut self.draft.llm_reviewer_provider,
                                                    String::new(),
                                                    "—",
                                                );
                                                for p in crate::llm::PROVIDERS.iter() {
                                                    ui.selectable_value(
                                                        &mut self.draft.llm_reviewer_provider,
                                                        p.id().to_owned(),
                                                        p.label(),
                                                    );
                                                }
                                            });
                                        if self.draft.llm_reviewer_provider != prev_p {
                                            if let Some(p) = crate::llm::Provider::from_id(
                                                &self.draft.llm_reviewer_provider,
                                            ) {
                                                self.draft.llm_reviewer_endpoint =
                                                    p.default_endpoint().to_owned();
                                                action.fetch_reviewer_models = true;
                                            }
                                            // Stash the outgoing model's key, then
                                            // clear model + key (no model chosen yet
                                            // under the new provider).
                                            if !self.draft.llm_reviewer_model.trim().is_empty()
                                                && !self
                                                    .draft
                                                    .llm_reviewer_api_key
                                                    .trim()
                                                    .is_empty()
                                            {
                                                self.draft.llm_api_keys.insert(
                                                    crate::llm::api_key_slot(
                                                        &prev_p,
                                                        &self.draft.llm_reviewer_model,
                                                    ),
                                                    self.draft.llm_reviewer_api_key.clone(),
                                                );
                                            }
                                            self.draft.llm_reviewer_api_key.clear();
                                            self.draft.llm_reviewer_model.clear();
                                            self.available_reviewer_models.clear();
                                        }
                                        ui.add(
                                            egui::TextEdit::singleline(
                                                &mut self.draft.llm_reviewer_endpoint,
                                            )
                                            .desired_width(ui.available_width().max(60.0))
                                            .hint_text(tr.settings_ai_endpoint),
                                        );
                                    });
                                    // Model + key
                                    ui.horizontal(|ui| {
                                        let models = self.available_reviewer_models.clone();
                                        let prev_m = self.draft.llm_reviewer_model.clone();
                                        egui::ComboBox::from_id_salt("ai_reviewer_model")
                                            .selected_text(if prev_m.trim().is_empty() {
                                                tr.settings_ai_model_empty.to_string()
                                            } else {
                                                prev_m.clone()
                                            })
                                            .width(220.0)
                                            .height(250.0)
                                            .show_ui(ui, |ui| {
                                                if models.is_empty() {
                                                    ui.weak(tr.settings_ai_model_empty);
                                                } else {
                                                    for model in models {
                                                        ui.selectable_value(
                                                            &mut self.draft.llm_reviewer_model,
                                                            model.clone(),
                                                            model,
                                                        );
                                                    }
                                                }
                                            });
                                        if self.draft.llm_reviewer_model != prev_m {
                                            // Hard rule: not the same provider+model
                                            // pair as the primary — reject and warn.
                                            if self.draft.llm_reviewer_provider.trim()
                                                == self.draft.llm_provider.trim()
                                                && self.draft.llm_reviewer_model.trim()
                                                    == self.draft.llm_model.trim()
                                            {
                                                self.draft.llm_reviewer_model = prev_m.clone();
                                                self.reviewer_same_model_error = true;
                                            } else {
                                                self.reviewer_same_model_error = false;
                                                // Per-model key stash/restore, same
                                                // contract as the primary field.
                                                let provider =
                                                    self.draft.llm_reviewer_provider.clone();
                                                if !prev_m.trim().is_empty() {
                                                    let prev_slot = crate::llm::api_key_slot(
                                                        &provider, &prev_m,
                                                    );
                                                    if self
                                                        .draft
                                                        .llm_reviewer_api_key
                                                        .trim()
                                                        .is_empty()
                                                    {
                                                        self.draft
                                                            .llm_api_keys
                                                            .remove(&prev_slot);
                                                    } else {
                                                        self.draft.llm_api_keys.insert(
                                                            prev_slot,
                                                            self.draft
                                                                .llm_reviewer_api_key
                                                                .clone(),
                                                        );
                                                    }
                                                }
                                                let slot = crate::llm::api_key_slot(
                                                    &provider,
                                                    &self.draft.llm_reviewer_model,
                                                );
                                                self.draft.llm_reviewer_api_key = self
                                                    .draft
                                                    .llm_api_keys
                                                    .get(&slot)
                                                    .cloned()
                                                    .unwrap_or_default();
                                            }
                                        }
                                        if ui
                                            .button(tr.settings_ai_refresh)
                                            .on_hover_text(tr.settings_ai_refresh_models)
                                            .clicked()
                                        {
                                            action.fetch_reviewer_models = true;
                                        }
                                        ui.add(
                                            egui::TextEdit::singleline(
                                                &mut self.draft.llm_reviewer_api_key,
                                            )
                                            .password(true)
                                            .desired_width(ui.available_width().max(60.0))
                                            .hint_text(tr.settings_ai_api_key),
                                        );
                                    });
                                    if self.reviewer_same_model_error {
                                        ui.label(
                                            RichText::new(tr.settings_ai_reviewer_same)
                                                .small()
                                                .color(Color32::from_rgb(240, 120, 120)),
                                        );
                                    }
                                });
                            });
                        });

                        ui.add_space(8.0);

                        // COBOL proficiency prompt (multiline)
                        ui.horizontal_top(|ui| {
                            let left_rect = ui
                                .allocate_exact_size(
                                    egui::vec2(splitter, 0.0),
                                    egui::Sense::hover(),
                                )
                                .0;
                            ui.scope_builder(egui::UiBuilder::new().max_rect(left_rect), |ui| {
                                ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Truncate);
                                ui.set_min_width(splitter);
                                ui.add_space(property_indent);
                                ui.add(egui::Label::new(tr.settings_ai_system_prompt).truncate());
                            });
                            ui.allocate_space(egui::vec2(resizer_width, 0.0));
                            ui.add_space(gap_after_resizer);
                            let right_w = ui.available_width();
                            ui.allocate_ui(egui::vec2(right_w, 154.0), |ui| {
                                ui.horizontal(|ui| {
                                    ui.label(
                                        RichText::new(
                                            "Tests applied to models to measure COBOL code generation.",
                                        )
                                        .small(),
                                    );
                                    ui.with_layout(
                                        egui::Layout::right_to_left(egui::Align::Center),
                                        |ui| {
                                            if ui.button("Restore").clicked() {
                                                self.draft.llm_cobol_proficiency_prompt =
                                                    crate::llm::default_cobol_proficiency_prompt();
                                                self.sync_cobol_proficiency_prompt_editor_from_draft();
                                            }
                                            if ui.button(tr.btn_save).clicked() {
                                                action.save = true;
                                            }
                                        },
                                    );
                                });
                                let w = ui.available_width();
                                let h = 120.0;
                                let frame = egui::Frame::NONE
                                    .fill(theme.bg_extreme)
                                    .stroke(egui::Stroke::new(1.0, theme.panel_border()))
                                    .corner_radius(egui::CornerRadius::same(6))
                                    .inner_margin(egui::Margin::same(2));
                                ui.set_min_height(h);
                                let ectx = ui.ctx().clone();
                                frame.show(ui, |ui| {
                                    ui.set_min_size(egui::vec2(w, h));
                                    self.cobol_proficiency_prompt_editor
                                        .render_code_area(&ectx, ui);
                                });
                                if let Some(text) =
                                    self.cobol_proficiency_prompt_editor.buffer_for_save()
                                {
                                    self.draft.llm_cobol_proficiency_prompt = text;
                                }
                            });
                        });

                        // Verbose AI log toggle
                        ui.horizontal_top(|ui| {
                            let left_rect = ui
                                .allocate_exact_size(
                                    egui::vec2(splitter, 0.0),
                                    egui::Sense::hover(),
                                )
                                .0;
                            ui.scope_builder(egui::UiBuilder::new().max_rect(left_rect), |ui| {
                                ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Truncate);
                                ui.set_min_width(splitter);
                                ui.add_space(property_indent);
                                ui.add(egui::Label::new(tr.settings_ai_verbose).truncate());
                            });
                            ui.allocate_space(egui::vec2(resizer_width, 0.0));
                            ui.add_space(gap_after_resizer);
                            let right_w = ui.available_width();
                            ui.allocate_ui(egui::vec2(right_w, 0.0), |ui| {
                                ui.checkbox(&mut self.draft.llm_verbose, "")
                                    .on_hover_text(tr.settings_ai_verbose_hint);
                            });
                        });
                        } // end SHOW_LEGACY_AI_FIELDS

                        // --- Verbose AI log (project-wide: applies to every
                        // agent and chat surface; persisted in cobolt.toml)
                        ui.horizontal_top(|ui| {
                            let left_rect = ui
                                .allocate_exact_size(
                                    egui::vec2(splitter, 0.0),
                                    egui::Sense::hover(),
                                )
                                .0;
                            ui.scope_builder(egui::UiBuilder::new().max_rect(left_rect), |ui| {
                                ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Truncate);
                                ui.set_min_width(splitter);
                                ui.add_space(property_indent);
                                ui.add(egui::Label::new(tr.settings_ai_verbose).truncate());
                            });
                            ui.allocate_space(egui::vec2(resizer_width, 0.0));
                            ui.add_space(gap_after_resizer);
                            let right_w = ui.available_width();
                            ui.allocate_ui(egui::vec2(right_w, 0.0), |ui| {
                                ui.checkbox(&mut self.draft.llm_verbose, "")
                                    .on_hover_text(tr.settings_ai_verbose_hint);
                            });
                        });

                        // --- Agent access (egui inspection / MCP) port
                        ui.horizontal_top(|ui| {
                            let left_rect = ui
                                .allocate_exact_size(
                                    egui::vec2(splitter, 0.0),
                                    egui::Sense::hover(),
                                )
                                .0;
                            ui.scope_builder(egui::UiBuilder::new().max_rect(left_rect), |ui| {
                                ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Truncate);
                                ui.set_min_width(splitter);
                                ui.add_space(property_indent);
                                ui.add(egui::Label::new(tr.ai_inspection_port).truncate());
                            });
                            ui.allocate_space(egui::vec2(resizer_width, 0.0));
                            ui.add_space(gap_after_resizer);
                            let right_w = ui.available_width();
                            ui.allocate_ui(egui::vec2(right_w, 0.0), |ui| {
                                ui.add(
                                    egui::DragValue::new(&mut self.draft.llm_inspection_port)
                                        .range(1024..=65535),
                                )
                                .on_hover_text(tr.ai_inspection_hint);
                            });
                        });

                        ui.add_space(8.0);

                        // --- Integrations section (spec 039): Maps + Web Search
                        ui.horizontal_top(|ui| {
                            let left_rect = ui
                                .allocate_exact_size(
                                    egui::vec2(splitter, 0.0),
                                    egui::Sense::hover(),
                                )
                                .0;
                            ui.scope_builder(egui::UiBuilder::new().max_rect(left_rect), |ui| {
                                ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Truncate);
                                ui.set_min_width(splitter);
                                section(ui, tr.set_sec_integrations, &theme);
                            });
                        });

                        // google_maps API key (Directions/Geocoding/Places/
                        // Distance-Matrix — never the OSM basemap tiles).
                        ui.horizontal_top(|ui| {
                            let left_rect = ui
                                .allocate_exact_size(
                                    egui::vec2(splitter, 0.0),
                                    egui::Sense::hover(),
                                )
                                .0;
                            ui.scope_builder(egui::UiBuilder::new().max_rect(left_rect), |ui| {
                                ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Truncate);
                                ui.set_min_width(splitter);
                                ui.add_space(property_indent);
                                ui.add(egui::Label::new(tr.settings_maps_api_key).truncate());
                            });
                            ui.allocate_space(egui::vec2(resizer_width, 0.0));
                            ui.add_space(gap_after_resizer);
                            let right_w = ui.available_width();
                            ui.allocate_ui(egui::vec2(right_w, 0.0), |ui| {
                                let w = ui.available_width();
                                ui.add(
                                    egui::TextEdit::singleline(&mut self.draft.maps_api_key)
                                        .password(true)
                                        .hint_text(tr.settings_maps_api_key_hint)
                                        .desired_width(w),
                                );
                            });
                        });

                        // Google Custom Search API key.
                        ui.horizontal_top(|ui| {
                            let left_rect = ui
                                .allocate_exact_size(
                                    egui::vec2(splitter, 0.0),
                                    egui::Sense::hover(),
                                )
                                .0;
                            ui.scope_builder(egui::UiBuilder::new().max_rect(left_rect), |ui| {
                                ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Truncate);
                                ui.set_min_width(splitter);
                                ui.add_space(property_indent);
                                ui.add(egui::Label::new(tr.settings_search_api_key).truncate());
                            });
                            ui.allocate_space(egui::vec2(resizer_width, 0.0));
                            ui.add_space(gap_after_resizer);
                            let right_w = ui.available_width();
                            ui.allocate_ui(egui::vec2(right_w, 0.0), |ui| {
                                let w = ui.available_width();
                                ui.add(
                                    egui::TextEdit::singleline(&mut self.draft.custom_search_api_key)
                                        .password(true)
                                        .hint_text(tr.settings_search_api_key_hint)
                                        .desired_width(w),
                                );
                            });
                        });

                        // Custom Search Engine id ("cx") — not a secret.
                        ui.horizontal_top(|ui| {
                            let left_rect = ui
                                .allocate_exact_size(
                                    egui::vec2(splitter, 0.0),
                                    egui::Sense::hover(),
                                )
                                .0;
                            ui.scope_builder(egui::UiBuilder::new().max_rect(left_rect), |ui| {
                                ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Truncate);
                                ui.set_min_width(splitter);
                                ui.add_space(property_indent);
                                ui.add(egui::Label::new(tr.settings_search_engine_id).truncate());
                            });
                            ui.allocate_space(egui::vec2(resizer_width, 0.0));
                            ui.add_space(gap_after_resizer);
                            let right_w = ui.available_width();
                            ui.allocate_ui(egui::vec2(right_w, 0.0), |ui| {
                                let w = ui.available_width();
                                ui.add(
                                    egui::TextEdit::singleline(
                                        &mut self.draft.custom_search_engine_id,
                                    )
                                    .desired_width(w),
                                );
                            });
                        });

                        ui.add_space(8.0);

                        // --- Runtime section header (left only)
                        ui.horizontal_top(|ui| {
                            let left_rect = ui
                                .allocate_exact_size(
                                    egui::vec2(splitter, 0.0),
                                    egui::Sense::hover(),
                                )
                                .0;
                            ui.scope_builder(egui::UiBuilder::new().max_rect(left_rect), |ui| {
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
                            let left_rect = ui
                                .allocate_exact_size(
                                    egui::vec2(splitter, 0.0),
                                    egui::Sense::hover(),
                                )
                                .0;
                            ui.scope_builder(egui::UiBuilder::new().max_rect(left_rect), |ui| {
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

                        // ── Run-Form inspector ────────────────────────────────
                        ui.horizontal_top(|ui| {
                            let left_rect = ui
                                .allocate_exact_size(
                                    egui::vec2(splitter, 0.0),
                                    egui::Sense::hover(),
                                )
                                .0;
                            ui.scope_builder(egui::UiBuilder::new().max_rect(left_rect), |ui| {
                                ui.set_min_width(splitter);
                                section(ui, "Run-Form inspector", &theme);
                            });
                            ui.allocate_space(egui::vec2(resizer_width, 0.0));
                            ui.add_space(gap_after_resizer);
                            let right_w = ui.available_width();
                            ui.allocate_ui(egui::vec2(right_w, 0.0), |_ui| {});
                        });
                        // Dump-on-anomaly toggle.
                        ui.horizontal_top(|ui| {
                            let left_rect = ui
                                .allocate_exact_size(
                                    egui::vec2(splitter, 0.0),
                                    egui::Sense::hover(),
                                )
                                .0;
                            ui.scope_builder(egui::UiBuilder::new().max_rect(left_rect), |ui| {
                                ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Truncate);
                                ui.set_min_width(splitter);
                                ui.add_space(property_indent);
                                ui.add(egui::Label::new("Dump on suspicious activity").truncate());
                            });
                            ui.allocate_space(egui::vec2(resizer_width, 0.0));
                            ui.add_space(gap_after_resizer);
                            let right_w = ui.available_width();
                            ui.allocate_ui(egui::vec2(right_w, 0.0), |ui| {
                                ui.checkbox(&mut self.draft.insp_dump_enabled, "")
                                    .on_hover_text(
                                        "When the inspector detects a memory leak, runaway CPU, or \
                                         rogue subprocesses while a form runs, write a process/memory \
                                         dump to the file below (console output is always on).",
                                    );
                            });
                        });
                        // Dump file path.
                        ui.horizontal_top(|ui| {
                            let left_rect = ui
                                .allocate_exact_size(
                                    egui::vec2(splitter, 0.0),
                                    egui::Sense::hover(),
                                )
                                .0;
                            ui.scope_builder(egui::UiBuilder::new().max_rect(left_rect), |ui| {
                                ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Truncate);
                                ui.set_min_width(splitter);
                                ui.add_space(property_indent);
                                ui.add(egui::Label::new("Dump file path").truncate());
                            });
                            ui.allocate_space(egui::vec2(resizer_width, 0.0));
                            ui.add_space(gap_after_resizer);
                            let right_w = ui.available_width();
                            ui.allocate_ui(egui::vec2(right_w, 0.0), |ui| {
                                ui.add_enabled(
                                    self.draft.insp_dump_enabled,
                                    egui::TextEdit::singleline(&mut self.draft.insp_dump_path)
                                        .desired_width(right_w - 8.0),
                                );
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
                    if active {
                        Color32::from_gray(140)
                    } else {
                        Color32::from_gray(75)
                    }
                } else {
                    if active {
                        Color32::from_gray(105)
                    } else {
                        Color32::from_gray(155)
                    }
                };
                ui.painter()
                    .vline(line_x, y_range, egui::Stroke::new(2.0, resizer_color));
            });

        ui.add_space(12.0);
        ui.separator();
        // ── Save / Cancel ─────────────────────────────────────────────────
        let dirty = self.is_dirty();
        ui.horizontal(|ui| {
            if ui
                .add_enabled(dirty, egui::Button::new(format!("💾 {}", tr.btn_save)))
                .clicked()
            {
                action.save = true;
            }
            if ui
                .add_enabled(dirty, egui::Button::new(format!("✖ {}", tr.btn_cancel)))
                .clicked()
            {
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

#[cfg(test)]
mod license_tests {
    use super::*;

    /// Every id in the dropdown except Proprietary ships a text, and each one is
    /// the real thing (a wrong `include_str!` path would silently pair a license
    /// id with another license's body).
    #[test]
    fn every_license_ships_its_own_text() {
        for &id in LICENSES {
            let text = stock_license_text(id);
            if id == "Proprietary" {
                assert!(text.is_none(), "Proprietary has no stock text");
                continue;
            }
            let text = text.unwrap_or_else(|| panic!("{id} has no text"));
            assert!(text.len() > 400, "{id} text looks truncated");
        }
        assert!(stock_license_text("MIT")
            .unwrap()
            .starts_with("MIT License"));
        assert!(stock_license_text("Apache-2.0")
            .unwrap()
            .contains("Apache License"));
        assert!(stock_license_text("GPL-3.0")
            .unwrap()
            .contains("GNU GENERAL PUBLIC LICENSE"));
        assert!(stock_license_text("LGPL-3.0")
            .unwrap()
            .contains("GNU LESSER GENERAL PUBLIC LICENSE"));
        assert!(stock_license_text("MPL-2.0")
            .unwrap()
            .contains("Mozilla Public License"));
        assert!(stock_license_text("CC0-1.0")
            .unwrap()
            .contains("CC0 1.0 Universal"));
        assert!(stock_license_text("Unlicense")
            .unwrap()
            .contains("public domain"));
        assert!(stock_license_text("BSD-3-Clause")
            .unwrap()
            .contains("Redistribution and use"));
    }

    fn draft(model: &str, text: &str) -> SettingsDraft {
        let mut d = SettingsDraft::default();
        d.license_model = model.into();
        d.license_text = text.into();
        d
    }

    #[test]
    fn selecting_a_license_loads_its_text_into_an_empty_box() {
        let mut d = draft("", "");
        d.set_license_model("MIT");
        assert_eq!(d.license_model, "MIT");
        assert_eq!(d.license_text, stock_license_text("MIT").unwrap());
    }

    #[test]
    fn switching_licenses_replaces_the_previous_stock_text() {
        let mut d = draft("MIT", stock_license_text("MIT").unwrap());
        d.set_license_model("Apache-2.0");
        assert_eq!(d.license_text, stock_license_text("Apache-2.0").unwrap());
    }

    /// The point of "if no custom text exists": hand-written terms survive.
    #[test]
    fn custom_text_is_never_overwritten() {
        let mut d = draft("Proprietary", "All rights reserved. Internal use only.");
        d.set_license_model("MIT");
        assert_eq!(d.license_model, "MIT");
        assert_eq!(d.license_text, "All rights reserved. Internal use only.");
    }

    /// An edited stock license counts as custom too.
    #[test]
    fn edited_stock_text_is_treated_as_custom() {
        let edited = stock_license_text("MIT").unwrap().replace("<year>", "2026");
        let mut d = draft("MIT", &edited);
        d.set_license_model("GPL-3.0");
        assert_eq!(d.license_text, edited);
    }

    /// Proprietary has no text to load, and must not wipe a stock body the
    /// developer is about to replace by hand — it just clears the stock text.
    #[test]
    fn proprietary_clears_stock_text() {
        let mut d = draft("MIT", stock_license_text("MIT").unwrap());
        d.set_license_model("Proprietary");
        assert!(d.license_text.is_empty());
    }

    #[test]
    fn re_selecting_the_same_license_is_a_no_op() {
        let mut d = draft("MIT", "my own words");
        d.set_license_model("MIT");
        assert_eq!(d.license_text, "my own words");
    }
}
