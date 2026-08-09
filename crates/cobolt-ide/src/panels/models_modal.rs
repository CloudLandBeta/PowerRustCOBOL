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
    /// A model-list fetch is owed for the profile now selected — armed when the
    /// manager opens and whenever the selection moves to another profile, so
    /// the dropdown fills itself instead of sitting empty until Refresh.
    auto_fetch_pending: bool,
    /// In-flight test-connection request.
    test_rx: Option<Receiver<LlmResponse>>,
    test_msg: Option<String>,
    /// Cached "model on disk" probe — `None` re-probes on the next frame.
    /// Cached so an open modal does not stat the model files every frame.
    semantic_ready: Option<bool>,
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
    /// User asked for the semantic search model — the app opens its blocking
    /// download modal.
    pub semantic_download_requested: bool,
    /// "Check proficiency" on a profile: the resolved config to benchmark, with
    /// no reviewer, so the run scores this model alone.
    pub run_proficiency: Option<LlmConfig>,
}

/// Whether opening the manager on `profile` should fetch its model list without
/// being asked.
///
/// The automatic fetch must never produce an error the developer did not ask
/// for: a profile with no provider yet, or a remote provider with no key, would
/// answer with a provider error the moment the manager opened. A local Ollama
/// needs no key, so it lists freely — which is the case where an automatic fetch
/// helps most. The Refresh button stays available in every case.
fn profile_can_list_models(profile: &ModelProfile, api_key: &str) -> bool {
    if crate::llm::Provider::from_id(&profile.provider).is_none() {
        return false;
    }
    let local = matches!(profile.provider.as_str(), "ollama" | "ollama_cloud");
    local || !api_key.trim().is_empty()
}

impl ModelsModal {
    /// Show an error, and record it in the IDE console (operator, 2026-08-09).
    /// Closing the manager takes the message with it; the console keeps it.
    fn set_error(&mut self, message: impl Into<String>) {
        let message = message.into();
        crate::error_log::record(&message);
        self.error = Some(message);
    }

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
            auto_fetch_pending: true,
            test_rx: None,
            test_msg: None,
            semantic_ready: None,
        }
    }

    /// Forget the cached "semantic model on disk" probe — the app calls this
    /// after its download modal finishes, so the status row updates without
    /// waiting for the manager to be reopened.
    pub fn invalidate_semantic_probe(&mut self) {
        self.semantic_ready = None;
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
        // This clears the list, so the newly selected profile owes a fetch —
        // otherwise switching profiles lands on an empty dropdown again.
        self.auto_fetch_pending = true;
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
                            self.set_error(e.clone());
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
        let semantic_ready = *self
            .semantic_ready
            .get_or_insert_with(cobolt_agents::project_knowledge::semantic_model_is_ready);

        let mut do_fetch = false;
        if self.auto_fetch_pending && self.models_rx.is_none() {
            self.auto_fetch_pending = false;
            do_fetch = self
                .draft_profile
                .as_ref()
                .is_some_and(|p| profile_can_list_models(p, &self.key_buf));
        }
        let mut do_test = false;
        let mut do_proficiency = false;
        let mut do_new = false;
        let mut do_duplicate = false;
        let mut do_delete = false;
        let mut do_save = false;
        let mut do_clear = false;
        let mut do_semantic_download = false;

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

                // Project-wide AI settings. These govern EVERY agent in the
                // project and persist in cobolt.toml — they are not part of the
                // selected model profile, so they render in their own panel
                // (above the footer) instead of inside the profile editor,
                // where they used to read as per-agent settings.
                egui::Panel::bottom(ui.id().with("models_project_settings"))
                    .resizable(false)
                    .show_separator_line(true)
                    .frame(egui::Frame::NONE)
                    .show(ui, |ui| {
                        ui.add_space(6.0);
                        ui.label(egui::RichText::new(tr.models_project_scope).strong());
                        ui.add_space(2.0);
                        // The verbose-log toggle deliberately does NOT appear
                        // here: its single control is ⚙ Settings → AI
                        // Assistants (the settings draft applies on OK, so a
                        // second live-editing control could silently stomp it).
                        ui.horizontal(|ui| {
                            if ui
                                .checkbox(&mut llm.agentic_ai_enabled, tr.models_agentic_enable)
                                .on_hover_text(tr.models_agentic_enable_hint)
                                .changed()
                            {
                                action.applied = true;
                            }
                            ui.add_space(16.0);
                            ui.label(tr.models_max_revisions);
                            if ui
                                .add(
                                    egui::DragValue::new(&mut llm.max_review_revisions)
                                        .range(0..=10),
                                )
                                .on_hover_text(tr.models_max_revisions_hint)
                                .changed()
                            {
                                action.applied = true;
                            }
                        });
                        ui.add_space(4.0);
                        // Unreviewed-task temperature: skipping Pedantic
                        // reviewers is a legitimate economy, but without a
                        // correction loop determinism matters more — so those
                        // calls run colder, and the knob is right here where
                        // the review budget lives.
                        ui.horizontal(|ui| {
                            let mut lowered = llm.unreviewed_temperature.is_some();
                            if ui
                                .checkbox(&mut lowered, tr.models_unreviewed_temp)
                                .on_hover_text(tr.models_unreviewed_temp_hint)
                                .changed()
                            {
                                llm.unreviewed_temperature = if lowered {
                                    crate::llm::default_unreviewed_temperature()
                                } else {
                                    None
                                };
                                action.applied = true;
                            }
                            if let Some(mut temperature) = llm.unreviewed_temperature {
                                if ui
                                    .add(
                                        egui::DragValue::new(&mut temperature)
                                            .range(0.0..=1.0)
                                            .speed(0.01)
                                            .fixed_decimals(2),
                                    )
                                    .on_hover_text(tr.models_unreviewed_temp_hint)
                                    .changed()
                                {
                                    llm.unreviewed_temperature = Some(temperature);
                                    action.applied = true;
                                }
                            }
                        });
                        ui.add_space(4.0);
                        // Semantic Knowledge Base search model. Machine-wide
                        // (the model cache is per-user, not per-project), but
                        // surfaced here because this is where models live.
                        ui.horizontal(|ui| {
                            ui.label(tr.models_semantic_label);
                            if semantic_ready {
                                ui.label(
                                    egui::RichText::new(tr.models_semantic_ready).small(),
                                );
                                // Embedding device (one policy, both KBs):
                                // GPU full speed, CPU low-power. Only known
                                // once an embedder has actually loaded.
                                if let Some(dev) =
                                    cobolt_agents::bert_embedder::active_device_label()
                                {
                                    ui.label(
                                        egui::RichText::new(format!(
                                            "{} {dev}",
                                            tr.models_embed_device
                                        ))
                                        .small(),
                                    );
                                }
                            } else {
                                ui.label(
                                    egui::RichText::new(tr.models_semantic_missing).small(),
                                );
                                if ui
                                    .button(tr.models_semantic_download)
                                    .on_hover_text(tr.models_semantic_download_hint)
                                    .clicked()
                                {
                                    do_semantic_download = true;
                                }
                            }
                        });
                        ui.add_space(6.0);
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

                                    });

                                ui.add_space(8.0);
                                ui.horizontal(|ui| {
                                    if ui.button(format!("🔌 {}", tr.settings_ai_test)).clicked()
                                    {
                                        do_test = true;
                                    }
                                    // The COBOL proficiency check belongs to the
                                    // MODEL, not to an agent: it scores what this
                                    // model writes, so a developer comparing two
                                    // models needs it here, once per profile,
                                    // rather than reaching it through whichever
                                    // agent happens to reference the profile.
                                    if ui
                                        .add_enabled(
                                            !draft.model.trim().is_empty(),
                                            egui::Button::new(format!(
                                                "🎓 {}",
                                                tr.agents_check_proficiency
                                            )),
                                        )
                                        .on_hover_text(tr.models_proficiency_hint)
                                        .clicked()
                                    {
                                        do_proficiency = true;
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
        if do_proficiency {
            if let Some(p) = self.draft_profile.as_ref() {
                let mut cfg = p.resolve(llm);
                cfg.api_key = self.key_buf.clone();
                // Test THIS model on its own. A reviewer inherited from the
                // global config would silently turn the run into the tandem
                // benchmark and score two models as one — the opposite of what
                // "check this profile" means.
                cfg.reviewer_provider.clear();
                cfg.reviewer_endpoint.clear();
                cfg.reviewer_model.clear();
                action.run_proficiency = Some(cfg);
            }
        }
        if do_semantic_download {
            // The download itself is owned by the app: it runs under the
            // IDE-blocking progress modal, not inside this manager.
            action.semantic_download_requested = true;
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

    fn profile_with(provider: &str) -> ModelProfile {
        ModelProfile {
            id: "profile-1".into(),
            name: "Saved".into(),
            provider: provider.into(),
            endpoint: String::new(),
            endpoint_user_edited: false,
            model: String::new(),
            temperature: 0.7,
            max_tokens: 8192,
            timeout_secs: 30,
        }
    }

    /// Opening the manager owes a fetch, so the dropdown is not empty until the
    /// developer clicks Refresh.
    #[test]
    fn opening_the_manager_owes_a_model_fetch() {
        assert!(ModelsModal::new().auto_fetch_pending);
    }

    /// Switching profiles clears the list, so the newly selected profile owes a
    /// fetch too — otherwise the second profile lands on an empty dropdown.
    #[test]
    fn selecting_another_profile_owes_a_model_fetch() {
        let mut llm = LlmConfig::load_defaults_for_test();
        llm.model_profiles.push(profile_with("openai"));
        let mut modal = ModelsModal::new();
        modal.auto_fetch_pending = false;
        modal.load_key(&llm);
        assert!(modal.auto_fetch_pending);
    }

    /// The automatic fetch must not manufacture an error the developer did not
    /// ask for: no provider, or a remote provider with no key, would answer with
    /// a provider error the moment the manager opened.
    #[test]
    fn the_automatic_fetch_is_skipped_when_it_could_only_fail() {
        assert!(!profile_can_list_models(&profile_with(""), "sk-key"));
        assert!(!profile_can_list_models(&profile_with("openai"), ""));
        assert!(!profile_can_list_models(&profile_with("openai"), "   "));
        assert!(profile_can_list_models(&profile_with("openai"), "sk-key"));
        assert!(profile_can_list_models(&profile_with("anthropic"), "sk-ant"));
    }

    /// A local Ollama lists without a key, and is the case where filling the
    /// dropdown automatically helps most.
    #[test]
    fn a_local_ollama_lists_without_a_key() {
        assert!(profile_can_list_models(&profile_with("ollama"), ""));
        assert!(profile_can_list_models(&profile_with("ollama_cloud"), ""));
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
