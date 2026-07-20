// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Models Manager modal (spec 031).
//!
//! Model connections are defined **once per project** here as reusable
//! [`ModelProfile`](crate::llm::ModelProfile)s — provider, endpoint, model id,
//! sampling, and (via the machine-local secret store) an API key — and referenced by
//! agents in the Agents Manager. This modal owns the full connection UI that used
//! to be repeated per agent: provider → endpoint → key → model (+ fetch list),
//! sampling, test connection, and a proficiency check, plus profile CRUD.
//!
//! The API key never lives on the profile or in any project file: it is stored
//! in [`LlmConfig::api_keys`] keyed by the stable model-profile id (spec 031 R7).

use std::sync::mpsc::Receiver;

use crate::i18n::Tr;
use crate::llm::{api_key_slot, profile_api_key_slot, LlmConfig, LlmResponse, ModelProfile};

pub struct ModelsModal {
    pub open: bool,
    /// Index into `llm.model_profiles`, clamped each frame.
    sel: usize,
    /// Edited API key for the selected profile (persisted into `api_keys`).
    key_buf: String,
    /// Whether `key_buf` has been hydrated for the current modal selection.
    key_loaded: bool,
    /// Draft copy of the selected profile. New and duplicated profiles also
    /// live only here until the user clicks Save.
    draft_profile: Option<ModelProfile>,
    /// Source index for an existing saved profile. `None` means the draft is a
    /// new profile and must not appear in `llm.model_profiles` before Save.
    draft_sel: Option<usize>,
    confirm_delete: bool,
    error: Option<String>,
    /// In-flight model-list fetch for the selected profile.
    models_rx: Option<Receiver<Result<Vec<String>, String>>>,
    models_request_profile_id: Option<String>,
    available_models: Vec<String>,
    models_msg: Option<String>,
    /// In-flight test-connection request.
    test_rx: Option<Receiver<LlmResponse>>,
    test_msg: Option<String>,
}

#[derive(Default)]
pub struct ModelsModalAction {
    /// A profile connection changed — persist `LlmConfig`.
    pub applied: bool,
    /// User explicitly clicked Save.
    pub save_requested: bool,
    /// Lines to mirror into the IDE output pane.
    pub log_lines: Vec<String>,
    /// Complete error payload to show in the alert/debug modal.
    pub alert_error: Option<String>,
}

impl ModelsModal {
    pub fn new() -> Self {
        Self {
            open: true,
            sel: 0,
            key_buf: String::new(),
            key_loaded: false,
            draft_profile: None,
            draft_sel: None,
            confirm_delete: false,
            error: None,
            models_rx: None,
            models_request_profile_id: None,
            available_models: Vec::new(),
            models_msg: None,
            test_rx: None,
            test_msg: None,
        }
    }

    fn clear_transient_results(&mut self) {
        self.available_models.clear();
        self.models_msg = None;
        self.test_msg = None;
        self.error = None;
        self.models_rx = None;
        self.models_request_profile_id = None;
        self.test_rx = None;
        crate::llm::clear_connection_log();
    }

    /// Load the selected profile's stored key into the edit buffer.
    fn load_key(&mut self, llm: &LlmConfig) {
        self.available_models.clear();
        self.models_msg = None;
        self.test_msg = None;
        self.models_rx = None;
        self.models_request_profile_id = None;
        self.test_rx = None;
        self.confirm_delete = false;
        let selected = llm.model_profiles.get(self.sel);
        self.draft_profile = selected.cloned();
        self.draft_sel = selected.map(|_| self.sel);
        self.key_buf = llm
            .model_profiles
            .get(self.sel)
            .map(|p| {
                llm.api_keys
                    .get(&profile_api_key_slot(&p.id))
                    .or_else(|| llm.api_keys.get(&api_key_slot(&p.provider, &p.model)))
                    .cloned()
                    .unwrap_or_default()
            })
            .unwrap_or_default();
        self.key_loaded = true;
    }

    fn ensure_draft(&mut self, llm: &LlmConfig) {
        let saved_selection_changed = self
            .draft_sel
            .map(|draft_sel| draft_sel != self.sel)
            .unwrap_or(false);
        if saved_selection_changed || self.draft_profile.is_none() {
            self.load_key(llm);
        }
    }

    fn begin_new_draft(&mut self, name: String) {
        self.draft_profile = Some(ModelProfile {
            id: crate::agents_db::new_uuid(),
            name,
            provider: String::new(),
            endpoint: String::new(),
            endpoint_user_edited: false,
            model: String::new(),
            temperature: 0.7,
            max_tokens: 8192,
            timeout_secs: 30,
        });
        self.draft_sel = None;
        self.key_buf.clear();
        self.key_loaded = true;
        self.clear_transient_results();
    }

    fn commit_draft(&mut self, llm: &mut LlmConfig) -> bool {
        let Some(mut draft) = self.draft_profile.clone() else {
            return false;
        };
        if draft.endpoint.trim().is_empty() {
            if let Some(provider) = crate::llm::Provider::from_id(&draft.provider) {
                draft.endpoint = provider.default_endpoint().to_string();
                draft.endpoint_user_edited = false;
            }
        }
        let saved_index = llm
            .model_profiles
            .iter()
            .position(|profile| profile.id == draft.id)
            .unwrap_or_else(|| {
                llm.model_profiles.push(draft.clone());
                llm.model_profiles.len() - 1
            });
        llm.model_profiles[saved_index] = draft.clone();
        llm.store_api_key(profile_api_key_slot(&draft.id), &self.key_buf);
        self.sel = saved_index;
        self.draft_sel = Some(saved_index);
        self.draft_profile = Some(draft);
        true
    }

    /// Commit the edited model first, then dismiss the modal only after the
    /// commit succeeds.
    fn save_and_close(&mut self, llm: &mut LlmConfig) -> bool {
        if !self.commit_draft(llm) {
            return false;
        }
        self.open = false;
        true
    }

    fn select_first_model_if_empty(&mut self) {
        let Some(draft) = self.draft_profile.as_mut() else {
            return;
        };
        if draft.model.trim().is_empty() {
            if let Some(first) = self.available_models.first() {
                draft.model = first.clone();
            }
        }
    }

    pub fn show(&mut self, ctx: &egui::Context, llm: &mut LlmConfig, tr: &Tr) -> ModelsModalAction {
        let mut action = ModelsModalAction::default();
        let mut open = self.open;
        if self.sel >= llm.model_profiles.len() {
            self.sel = llm.model_profiles.len().saturating_sub(1);
        }
        if !self.key_loaded {
            self.load_key(llm);
        }
        self.ensure_draft(llm);

        // Drain an in-flight model-list fetch.
        if let Some(rx) = &self.models_rx {
            if let Ok(res) = rx.try_recv() {
                let current_profile_id = self
                    .draft_profile
                    .as_ref()
                    .map(|profile| profile.id.as_str());
                if self.models_request_profile_id.as_deref() == current_profile_id {
                    match res {
                        Ok(models) => {
                            let msg = format!("{} model(s) available", models.len());
                            self.models_msg = Some(msg.clone());
                            self.available_models = crate::llm::filter_retired_models(models);
                            self.select_first_model_if_empty();
                            action.log_lines.push(format!("Models Manager: {msg}"));
                        }
                        Err(e) => {
                            let msg = format!("Models Manager: failed to fetch models: {e}");
                            self.models_msg = None;
                            self.error = Some(e.clone());
                            action.log_lines.push(msg);
                            action.alert_error = Some(e);
                        }
                    }
                }
                self.models_rx = None;
                self.models_request_profile_id = None;
            }
        }
        // Drain an in-flight test-connection request.
        if let Some(rx) = &self.test_rx {
            if let Ok(res) = rx.try_recv() {
                let msg = match res {
                    LlmResponse::Ok(_) | LlmResponse::Chunk(_) => tr.ai_test_ok.to_string(),
                    LlmResponse::Err(e) => {
                        action.alert_error = Some(e.clone());
                        format!("{}: {e}", tr.ai_test_failed_title)
                    }
                };
                action.log_lines.push(format!("Models Manager: {msg}"));
                self.test_msg = Some(msg);
                self.test_rx = None;
            }
        }

        let mut do_fetch = false;
        let mut do_test = false;
        let mut do_new = false;
        let mut do_duplicate = false;
        let mut do_delete = false;
        let mut do_save = false;
        let mut do_clear = false;

        // Seeded size only: after opening, egui window state owns user resizing.
        // Keep bounded inner lists so long model/profile names do not self-inflate
        // the modal or make the title bar unusable.
        egui::Window::new(tr.models_title)
            .id(egui::Id::new("models_manager_modal"))
            .open(&mut open)
            .collapsible(false)
            .resizable(true)
            .default_size([780.0, 560.0])
            .min_size([720.0, 460.0])
            .show(ctx, |ui| {
                if self.sel >= llm.model_profiles.len() {
                    self.sel = llm.model_profiles.len().saturating_sub(1);
                }

                // Bottom Panel for footer. Provider errors are intentionally not
                // rendered here; they go to the IDE log pane and alert dialog.
                egui::Panel::bottom(ui.id().with("models_footer"))
                    .resizable(false)
                    .show_separator_line(true)
                    .frame(egui::Frame::NONE)
                    .show(ui, |ui| {
                        ui.add_space(6.0);
                        if let Some(m) = &self.test_msg {
                            ui.label(egui::RichText::new(m).small());
                            ui.add_space(4.0);
                        }
                        ui.horizontal(|ui| {
                            if ui.button(tr.btn_save).clicked() {
                                do_save = true;
                            }
                            if ui.button(tr.inspect_close).clicked() {
                                self.open = false;
                            }
                        });
                        ui.add_space(4.0);
                    });

                // Left Panel for profile list
                egui::Panel::left(ui.id().with("models_left_rail"))
                    .resizable(true)
                    .default_size(220.0)
                    .min_size(180.0)
                    .max_size(320.0)
                    .show(ui, |ui| {
                        ui.add_space(6.0);
                        ui.vertical(|ui| {
                            if ui.button(format!("＋ {}", tr.models_new)).clicked() {
                                do_new = true;
                            }
                            ui.add_space(4.0);
                            egui::ScrollArea::vertical()
                                .auto_shrink([false, false])
                                .show(ui, |ui| {
                                    if llm.model_profiles.is_empty() {
                                        ui.weak(tr.models_none);
                                    }
                                    for i in 0..llm.model_profiles.len() {
                                        let name = llm.model_profiles[i].name.clone();
                                        let model = llm.model_profiles[i].model.clone();
                                        if ui
                                            .selectable_label(
                                                self.draft_sel == Some(i),
                                                egui::RichText::new(if name.is_empty() {
                                                    "—".into()
                                                } else {
                                                    name
                                                }),
                                            )
                                            .on_hover_text(model)
                                            .clicked()
                                    && self.draft_sel != Some(i)
                                        {
                                            self.sel = i;
                                            self.key_loaded = false;
                                            self.load_key(llm);
                                        }
                                    }
                                });
                        });
                    });

                // Central Panel for selected profile editor
                egui::CentralPanel::default()
                    .frame(egui::Frame::NONE)
                    .show(ui, |ui| {
                        ui.add_space(6.0);
                        self.ensure_draft(llm);
                        let Some(draft) = self.draft_profile.as_mut() else {
                            ui.weak(tr.models_select);
                            return;
                        };
                        egui::ScrollArea::vertical()
                            .id_salt("models_editor_scroll")
                            .auto_shrink([false, false])
                            .show(ui, |ui| {
                                egui::Grid::new("mp_grid")
                                    .num_columns(2)
                                    .spacing([14.0, 7.0])
                                    .show(ui, |ui| {
                                        ui.label(tr.models_name);
                                        {
                                            if ui
                                                .add(
                                                    egui::TextEdit::singleline(&mut draft.name)
                                                        .desired_width(f32::INFINITY),
                                                )
                                                .changed()
                                            {
                                                self.error = None;
                                            }
                                        }
                                        ui.end_row();

                                        ui.label(tr.settings_ai_provider);
                                        {
                                            let prev = draft.provider.clone();
                                            egui::ComboBox::from_id_salt("mp_provider")
                                                .selected_text(if draft.provider.is_empty() {
                                                    "—"
                                                } else {
                                                    &draft.provider
                                                })
                                                .show_ui(ui, |ui| {
                                                    for prov in crate::llm::PROVIDERS.iter() {
                                                        ui.selectable_value(
                                                            &mut draft.provider,
                                                            prov.id().to_owned(),
                                                            prov.label(),
                                                        );
                                                    }
                                                });
                                            if draft.provider != prev {
                                                if let Some(prov) =
                                                    crate::llm::Provider::from_id(&draft.provider)
                                                {
                                                    draft.endpoint =
                                                        prov.default_endpoint().to_owned();
                                                    draft.endpoint_user_edited = false;
                                                }
                                                draft.model.clear();
                                                self.available_models.clear();
                                                self.models_msg = None;
                                                self.test_msg = None;
                                            }
                                        }
                                        ui.end_row();

                                        ui.label(tr.settings_ai_endpoint);
                                        {
                                            if ui
                                                .add(
                                                    egui::TextEdit::singleline(
                                                        &mut draft.endpoint,
                                                    )
                                                    .desired_width(f32::INFINITY),
                                                )
                                                .changed()
                                            {
                                                draft.endpoint_user_edited = true;
                                                self.models_msg = None;
                                            }
                                        }
                                        ui.end_row();

                                        ui.label(tr.settings_ai_api_key);
                                        if ui
                                            .add(
                                                egui::TextEdit::singleline(&mut self.key_buf)
                                                    .password(true)
                                                    .desired_width(f32::INFINITY),
                                            )
                                            .changed()
                                        {
                                            self.test_msg = None;
                                        }
                                        ui.end_row();
                                        ui.label("");
                                        ui.weak(tr.agents_key_hint);
                                        ui.end_row();

                                        ui.label(tr.settings_ai_model);
                                        {
                                            let prev_model = draft.model.clone();
                                            ui.horizontal(|ui| {
                                                let selected = if draft.model.trim().is_empty() {
                                                    tr.settings_ai_model_empty.to_string()
                                                } else {
                                                    draft.model.clone()
                                                };
                                                egui::ComboBox::from_id_salt("mp_model_pick")
                                                    .selected_text(selected)
                                                    .width(260.0)
                                                    .height(260.0)
                                                    .show_ui(ui, |ui| {
                                                        if self.available_models.is_empty() {
                                                            ui.weak(tr.settings_ai_model_empty);
                                                        } else {
                                                            for model in &self.available_models {
                                                                ui.selectable_value(
                                                                    &mut draft.model,
                                                                    model.clone(),
                                                                    model,
                                                                );
                                                            }
                                                        }
                                                    });
                                                if ui
                                                    .add_enabled(
                                                        self.models_rx.is_none(),
                                                        egui::Button::new(tr.settings_ai_refresh),
                                                    )
                                                    .on_hover_text(tr.settings_ai_refresh_models)
                                                    .clicked()
                                                {
                                                    do_fetch = true;
                                                }
                                                if self.models_rx.is_some() {
                                                    ui.add(egui::Spinner::new());
                                                }
                                                if let Some(m) = &self.models_msg {
                                                    ui.label(egui::RichText::new(m).small().weak());
                                                }
                                            });
                                            if draft.model != prev_model {
                                                self.test_msg = None;
                                            }
                                        }
                                        ui.end_row();

                                        ui.label(tr.agents_sampling);
                                        {
                                            ui.horizontal(|ui| {
                                                if ui
                                                    .add(
                                                        egui::DragValue::new(&mut draft.temperature)
                                                            .range(0.0..=2.0)
                                                            .speed(0.05),
                                                    )
                                                    .changed()
                                                {
                                                    self.test_msg = None;
                                                }
                                                ui.label("·");
                                                if ui
                                                    .add(
                                                        egui::DragValue::new(&mut draft.max_tokens)
                                                            .range(256..=128000)
                                                            .speed(100),
                                                    )
                                                    .changed()
                                                {
                                                    self.test_msg = None;
                                                }
                                                ui.label("·");
                                                if ui
                                                    .add(
                                                        egui::DragValue::new(
                                                            &mut draft.timeout_secs,
                                                        )
                                                            .range(1..=1200),
                                                    )
                                                    .changed()
                                                {
                                                    self.test_msg = None;
                                                }
                                                ui.label("s");
                                            });
                                        }
                                        ui.end_row();

                                        ui.label("Agentic AI:");
                                        if ui
                                            .checkbox(
                                                &mut llm.agentic_ai_enabled,
                                                "Enable assistant and agents",
                                            )
                                            .on_hover_text(
                                                "Turn off to hide AI assistant surfaces and keep a traditional programming workflow.",
                                            )
                                            .changed()
                                        {
                                            action.applied = true;
                                        }
                                        ui.end_row();

                                        ui.label(tr.settings_ai_verbose);
                                        if ui
                                            .checkbox(&mut llm.verbose_log, "")
                                            .on_hover_text(tr.settings_ai_verbose_hint)
                                            .changed()
                                        {
                                            action.applied = true;
                                        }
                                        ui.end_row();
                                    });

                                ui.add_space(8.0);
                                ui.horizontal(|ui| {
                                    if ui.button(format!("🔌 {}", tr.settings_ai_test)).clicked()
                                    {
                                        do_test = true;
                                    }
                                    if ui.button(tr.agent_clear_log).clicked() {
                                        do_clear = true;
                                    }
                                    if ui.button(format!("⧉ {}", tr.models_duplicate)).clicked() {
                                        do_duplicate = true;
                                    }
                                    if !self.confirm_delete {
                                        if ui
                                            .button(
                                                egui::RichText::new(format!(
                                                    "🗑 {}",
                                                    tr.models_delete
                                                ))
                                                .color(egui::Color32::from_rgb(224, 120, 120)),
                                            )
                                            .clicked()
                                        {
                                            self.confirm_delete = true;
                                        }
                                    } else {
                                        ui.label(
                                            egui::RichText::new(tr.models_delete_warn)
                                                .small()
                                                .color(egui::Color32::from_rgb(224, 160, 120)),
                                        );
                                        if ui.button(tr.models_delete).clicked() {
                                            do_delete = true;
                                        }
                                        if ui.button(tr.btn_cancel).clicked() {
                                            self.confirm_delete = false;
                                        }
                                    }
                                });
                            });
                    });
            });

        // ── Deferred mutations (avoid double-borrowing inside the UI) ────────
        if do_clear {
            self.clear_transient_results();
        }
        if do_new {
            self.begin_new_draft(tr.models_new_name.to_string());
        }
        if do_duplicate {
            if let Some(src) = self.draft_profile.clone() {
                let mut copy = src;
                copy.id = crate::agents_db::new_uuid();
                copy.name = format!("{} (copy)", copy.name);
                self.draft_profile = Some(copy);
                self.draft_sel = None;
                self.clear_transient_results();
            }
        }
        if do_delete {
            if let Some(saved_index) = self.draft_sel {
                let profile_id = self
                    .draft_profile
                    .as_ref()
                    .map(|profile| profile.id.clone())
                    .unwrap_or_default();
                if llm.delete_model_profile(&profile_id) {
                    self.sel = saved_index
                        .saturating_sub(1)
                        .min(llm.model_profiles.len().saturating_sub(1));
                    self.load_key(llm);
                    action.applied = true;
                }
            } else {
                // Delete on an unsaved New/Duplicate draft simply discards it;
                // there is no stored key or profile to remove.
                self.load_key(llm);
            }
            if self.confirm_delete {
                self.confirm_delete = false;
            }
        }
        if do_save {
            if self.save_and_close(llm) {
                action.applied = true;
                action.save_requested = true;
                action
                    .log_lines
                    .push("Models Manager: model settings saved.".to_string());
            }
        }
        if do_fetch {
            if let Some(p) = self.draft_profile.as_ref() {
                match crate::llm::Provider::from_id(&p.provider) {
                    Some(provider) => {
                        self.available_models.clear();
                        self.models_msg = Some(tr.ai_detecting.to_string());
                        self.models_request_profile_id = Some(p.id.clone());
                        self.models_rx = Some(crate::llm::spawn_list_models(
                            provider,
                            &p.endpoint,
                            &self.key_buf,
                        ));
                    }
                    None => self.models_msg = Some(tr.settings_ai_provider_select.to_string()),
                }
            }
        }
        if do_test {
            if let Some(p) = self.draft_profile.as_ref() {
                self.test_msg = Some(tr.ai_testing.to_string());
                let mut cfg = p.resolve(llm);
                cfg.api_key = self.key_buf.clone();
                self.test_rx = Some(crate::llm::spawn_test(&cfg));
            }
        }

        if !open {
            self.open = false;
        }
        action
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_profile_stays_draft_until_commit_and_saves_selected_model_id() {
        let mut llm = LlmConfig::load_defaults_for_test();
        let mut modal = ModelsModal::new();
        modal.begin_new_draft("Cloud model".into());
        assert!(llm.model_profiles.is_empty());

        let draft = modal.draft_profile.as_mut().unwrap();
        draft.provider = "ollama_cloud".into();
        draft.endpoint = "https://ollama.com/api/chat".into();
        draft.model = "qwen3.5:397b".into();
        modal.key_buf = "saved-key".into();

        assert!(modal.commit_draft(&mut llm));
        assert_eq!(llm.model_profiles.len(), 1);
        assert_eq!(llm.model_profiles[0].model, "qwen3.5:397b");
        assert_eq!(
            llm.api_keys
                .get(&profile_api_key_slot(&llm.model_profiles[0].id))
                .map(String::as_str),
            Some("saved-key")
        );
    }

    #[test]
    fn model_dropdown_defaults_once_and_preserves_saved_selection() {
        let mut modal = ModelsModal::new();
        modal.begin_new_draft("Cloud model".into());
        modal.available_models = vec!["first-id".into(), "second-id".into()];
        modal.select_first_model_if_empty();
        assert_eq!(modal.draft_profile.as_ref().unwrap().model, "first-id");

        modal.draft_profile.as_mut().unwrap().model = "saved-id".into();
        modal.select_first_model_if_empty();
        assert_eq!(modal.draft_profile.as_ref().unwrap().model, "saved-id");
    }

    #[test]
    fn blank_key_field_does_not_erase_saved_profile_key() {
        let mut llm = LlmConfig::load_defaults_for_test();
        llm.model_profiles.push(ModelProfile {
            id: "profile-1".into(),
            name: "Saved".into(),
            provider: "openai".into(),
            endpoint: "https://api.openai.com/v1".into(),
            endpoint_user_edited: false,
            model: "gpt-5".into(),
            temperature: 0.7,
            max_tokens: 8192,
            timeout_secs: 30,
        });
        llm.store_api_key(profile_api_key_slot("profile-1"), "keep-me");

        let mut modal = ModelsModal::new();
        modal.load_key(&llm);
        modal.key_buf.clear();
        assert!(modal.commit_draft(&mut llm));
        assert_eq!(
            llm.api_keys
                .get(&profile_api_key_slot("profile-1"))
                .map(String::as_str),
            Some("keep-me")
        );
    }

    #[test]
    fn successful_save_commits_model_before_closing_modal() {
        let mut llm = LlmConfig::load_defaults_for_test();
        let mut modal = ModelsModal::new();
        modal.begin_new_draft("Saved model".into());
        modal.draft_profile.as_mut().unwrap().model = "gpt-5".into();

        assert!(modal.open);
        assert!(modal.save_and_close(&mut llm));
        assert_eq!(llm.model_profiles[0].model, "gpt-5");
        assert!(!modal.open);
    }
}
