// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Model Providers Manager (spec 048).
//!
//! A **provider** is configured once — its endpoint and its API key — and from
//! that moment every model that provider offers is selectable by any agent.
//! That is the whole surface: no per-model records, no sampling, no model
//! choice. Which model an agent runs on, and how hot, how long and how large
//! its answers may be, belongs to the agent and is edited in the Agents
//! Manager.
//!
//! This replaces the spec 031 "Models Manager", where a connection was defined
//! once per *model* as a [`ModelProfile`](crate::llm::ModelProfile) and agents
//! referenced it. A provider you had already paid for needed a whole second
//! record, with the same key pasted again, before a second of its models could
//! be used.
//!
//! The API key never lives in a project file: it is stored in the machine-local
//! secret store under [`provider_key_slot`](crate::llm::provider_key_slot).

use std::sync::mpsc::Receiver;

use crate::i18n::Tr;
use crate::llm::{provider_key_slot, LlmConfig, LlmResponse, Provider, PROVIDERS};

pub struct ModelsModal {
    pub open: bool,
    /// Index into [`PROVIDERS`], clamped each frame.
    sel: usize,
    /// Edited API key for the selected provider.
    key_buf: String,
    /// Edited endpoint for the selected provider.
    endpoint_buf: String,
    /// Whether the buffers have been hydrated for the current selection.
    loaded: bool,
    error: Option<String>,
    confirm_delete: bool,
    /// In-flight model-list fetch, and the provider that asked for it.
    models_rx: Option<Receiver<Result<Vec<String>, String>>>,
    models_request_provider: Option<String>,
    models_msg: Option<String>,
    /// A model-list fetch is owed for the provider now selected — armed when
    /// the manager opens and whenever the selection moves, so a configured
    /// provider fills its list instead of waiting for Refresh.
    auto_fetch_pending: bool,
    /// In-flight test-connection request.
    test_rx: Option<Receiver<LlmResponse>>,
    test_msg: Option<String>,
    /// Cached "semantic model on disk" probe — `None` re-probes next frame.
    semantic_ready: Option<bool>,
}

#[derive(Default)]
pub struct ModelsModalAction {
    /// A provider's configuration changed — persist `LlmConfig`.
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
}

/// Whether opening the manager on `provider` should fetch its model list
/// without being asked (spec 048 R4).
///
/// The automatic fetch must never produce an error the developer did not ask
/// for: a provider with no key would answer with a provider error the moment
/// the manager opened. A local Ollama needs no key, so it lists freely — which
/// is the case where an automatic fetch helps most. Refresh stays available in
/// every case.
fn provider_can_list_models(provider: &str, api_key: &str) -> bool {
    if Provider::from_id(provider).is_none() {
        return false;
    }
    !crate::llm::provider_requires_key(provider) || !api_key.trim().is_empty()
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
            endpoint_buf: String::new(),
            loaded: false,
            error: None,
            confirm_delete: false,
            models_rx: None,
            models_request_provider: None,
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

    /// The provider id the selection points at.
    fn selected_provider(&self) -> &'static str {
        PROVIDERS
            .get(self.sel)
            .map(|p| p.id)
            .unwrap_or(PROVIDERS[0].id)
    }

    /// Hydrate the edit buffers from the selected provider's stored
    /// configuration.
    fn load_selected(&mut self, llm: &LlmConfig) {
        let id = self.selected_provider();
        self.key_buf = llm.provider_api_key(id);
        self.endpoint_buf = llm.provider_endpoint(id);
        self.models_msg = None;
        self.test_msg = None;
        self.models_rx = None;
        self.models_request_provider = None;
        self.test_rx = None;
        self.confirm_delete = false;
        // The selection moved, so this provider owes a fetch.
        self.auto_fetch_pending = true;
        self.loaded = true;
    }

    /// Write the edit buffers back into the machine-wide configuration.
    fn commit(&mut self, llm: &mut LlmConfig) {
        let id = self.selected_provider().to_string();
        let endpoint = self.endpoint_buf.trim().to_string();
        {
            let cfg = llm.ensure_provider_config(&id);
            if endpoint.is_empty() {
                cfg.endpoint = Provider::from_id(&id)
                    .map(|p| p.default_endpoint().to_string())
                    .unwrap_or_default();
                cfg.endpoint_user_edited = false;
            } else {
                cfg.endpoint = endpoint.clone();
                cfg.endpoint_user_edited =
                    !crate::llm::endpoint_is_provider_default(&id, &endpoint);
            }
        }
        // An empty key field never erases a stored credential; deleting is an
        // explicit action with its own confirmation.
        llm.store_api_key(provider_key_slot(&id), &self.key_buf);
    }

    fn save_and_close(&mut self, llm: &mut LlmConfig) {
        self.commit(llm);
        self.open = false;
    }

    pub fn show(&mut self, ctx: &egui::Context, llm: &mut LlmConfig, tr: &Tr) -> ModelsModalAction {
        let mut action = ModelsModalAction::default();
        let mut open = self.open;
        if self.sel >= PROVIDERS.len() {
            self.sel = 0;
        }
        if !self.loaded {
            self.load_selected(llm);
        }

        // Drain an in-flight model-list fetch.
        if let Some(rx) = &self.models_rx {
            if let Ok(res) = rx.try_recv() {
                let requested = self.models_request_provider.clone();
                if requested.as_deref() == Some(self.selected_provider()) {
                    match res {
                        Ok(models) => {
                            let models = crate::llm::filter_retired_models(models);
                            let msg = tr
                                .providers_models_count
                                .replacen("{}", &models.len().to_string(), 1);
                            self.models_msg = Some(msg.clone());
                            llm.set_models_for(self.selected_provider(), models);
                            action.applied = true;
                            action.log_lines.push(format!(
                                "Model Providers: {} — {msg}",
                                self.selected_provider()
                            ));
                        }
                        Err(e) => {
                            self.models_msg = None;
                            self.set_error(tr.providers_models_error.replacen("{}", &e, 1));
                            action
                                .log_lines
                                .push(format!("Model Providers: failed to list models: {e}"));
                            action.alert_error = Some(e);
                        }
                    }
                }
                self.models_rx = None;
                self.models_request_provider = None;
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
                action
                    .log_lines
                    .push(format!("Model Providers: {msg}"));
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
            do_fetch = provider_can_list_models(self.selected_provider(), &self.key_buf);
        }
        let mut do_test = false;
        let mut do_save = false;
        let mut do_delete = false;
        let mut do_semantic_download = false;
        let mut select: Option<usize> = None;

        // Seeded size only: after opening, egui window state owns user resizing.
        egui::Window::new(tr.providers_title)
            .id(egui::Id::new("model_providers_manager_modal"))
            .open(&mut open)
            .collapsible(false)
            .resizable(true)
            .default_size([780.0, 560.0])
            .min_size([720.0, 460.0])
            .show(ctx, |ui| {
                // Footer.
                egui::Panel::bottom(ui.id().with("providers_footer"))
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
                // project and persist in cobolt.toml — they belong to no
                // provider, so they render in their own panel above the footer.
                egui::Panel::bottom(ui.id().with("providers_project_settings"))
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
                        // correction loop determinism matters more.
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
                                ui.label(egui::RichText::new(tr.models_semantic_ready).small());
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
                                ui.label(egui::RichText::new(tr.models_semantic_missing).small());
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

                // Left rail: every shipped provider, configured ones marked.
                egui::Panel::left(ui.id().with("providers_left_rail"))
                    .resizable(true)
                    .default_size(240.0)
                    .min_size(200.0)
                    .max_size(340.0)
                    .show(ui, |ui| {
                        ui.add_space(6.0);
                        egui::ScrollArea::vertical()
                            .auto_shrink([false, false])
                            .show(ui, |ui| {
                                for (i, provider) in PROVIDERS.iter().enumerate() {
                                    let configured = llm.provider_is_configured(provider.id);
                                    let dot = if configured { "●" } else { "○" };
                                    let label = format!("{dot} {}", provider.label);
                                    if ui
                                        .selectable_label(self.sel == i, label)
                                        .clicked()
                                        && self.sel != i
                                    {
                                        select = Some(i);
                                    }
                                }
                            });
                    });

                // Central: the selected provider's configuration.
                egui::CentralPanel::default()
                    .frame(egui::Frame::NONE)
                    .show(ui, |ui| {
                        ui.add_space(6.0);
                        let id = self.selected_provider();
                        let label = Provider::from_id(id)
                            .map(|p| p.label.to_string())
                            .unwrap_or_else(|| id.to_string());
                        ui.label(egui::RichText::new(label).strong().size(15.0));
                        ui.add_space(6.0);

                        let needs_key = crate::llm::provider_requires_key(id);
                        if !needs_key {
                            ui.label(egui::RichText::new(tr.providers_local_no_key).small());
                            ui.add_space(4.0);
                        } else if !llm.provider_is_configured(id) {
                            ui.label(egui::RichText::new(tr.providers_unconfigured).small());
                            ui.add_space(4.0);
                        }

                        egui::Grid::new("provider_grid")
                            .num_columns(2)
                            .spacing([14.0, 7.0])
                            .show(ui, |ui| {
                                ui.label(tr.providers_endpoint);
                                ui.horizontal(|ui| {
                                    if ui
                                        .add(
                                            egui::TextEdit::singleline(&mut self.endpoint_buf)
                                                .desired_width(360.0),
                                        )
                                        .changed()
                                    {
                                        self.error = None;
                                    }
                                    if ui.button(tr.providers_endpoint_reset).clicked() {
                                        self.endpoint_buf = Provider::from_id(id)
                                            .map(|p| p.default_endpoint().to_string())
                                            .unwrap_or_default();
                                    }
                                });
                                ui.end_row();

                                if needs_key {
                                    ui.label(tr.providers_key);
                                    ui.horizontal(|ui| {
                                        ui.add(
                                            egui::TextEdit::singleline(&mut self.key_buf)
                                                .password(true)
                                                .desired_width(360.0),
                                        );
                                        if ui.button(tr.models_delete).clicked() {
                                            self.confirm_delete = true;
                                        }
                                    });
                                    ui.end_row();
                                    ui.label("");
                                    ui.label(egui::RichText::new(tr.agents_key_hint).small());
                                    ui.end_row();
                                }

                                ui.label(tr.settings_ai_model);
                                ui.horizontal(|ui| {
                                    let count = llm.models_for(id).len();
                                    if count > 0 {
                                        ui.label(
                                            tr.providers_models_count
                                                .replacen("{}", &count.to_string(), 1),
                                        );
                                    } else {
                                        ui.label(
                                            egui::RichText::new(tr.settings_ai_model_empty).small(),
                                        );
                                    }
                                    if ui.button(tr.providers_refresh_models).clicked() {
                                        do_fetch = true;
                                    }
                                    if ui.button(tr.settings_ai_test).clicked() {
                                        do_test = true;
                                    }
                                });
                                ui.end_row();
                            });

                        if self.confirm_delete {
                            ui.add_space(6.0);
                            ui.label(
                                egui::RichText::new(tr.models_delete_warn)
                                    .color(ui.visuals().error_fg_color),
                            );
                            ui.horizontal(|ui| {
                                if ui.button(tr.models_delete).clicked() {
                                    do_delete = true;
                                }
                                if ui.button(tr.btn_cancel).clicked() {
                                    self.confirm_delete = false;
                                }
                            });
                        }

                        if let Some(m) = &self.models_msg {
                            ui.add_space(6.0);
                            ui.label(egui::RichText::new(m).small());
                        }
                        if let Some(e) = &self.error {
                            ui.add_space(6.0);
                            ui.label(
                                egui::RichText::new(e).color(ui.visuals().error_fg_color).small(),
                            );
                        }
                    });
            });

        if let Some(i) = select {
            // Keep whatever the developer typed for the provider they are
            // leaving, then move.
            self.commit(llm);
            action.applied = true;
            self.sel = i;
            self.load_selected(llm);
        }
        if do_delete {
            let slot = provider_key_slot(self.selected_provider());
            llm.forget_credential_slot(&slot);
            self.key_buf.clear();
            self.confirm_delete = false;
            action.applied = true;
        }
        if do_fetch {
            let id = self.selected_provider().to_string();
            if let Some(provider) = Provider::from_id(&id) {
                self.models_rx = Some(crate::llm::spawn_list_models(
                    provider,
                    &self.endpoint_buf,
                    &self.key_buf,
                ));
                self.models_request_provider = Some(id);
                self.models_msg = Some(tr.ai_detecting.to_string());
            }
        }
        if do_test {
            self.commit(llm);
            action.applied = true;
            let id = self.selected_provider().to_string();
            let mut cfg = llm.clone();
            cfg.provider = id.clone();
            cfg.endpoint = self.endpoint_buf.clone();
            cfg.api_key = self.key_buf.clone();
            cfg.model = llm.models_for(&id).first().cloned().unwrap_or_default();
            self.test_rx = Some(crate::llm::spawn_test(&cfg));
            self.test_msg = Some(tr.ai_testing.to_string());
        }
        if do_save {
            self.save_and_close(llm);
            action.applied = true;
            action.save_requested = true;
        }
        if do_semantic_download {
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

    /// A provider is configured once and every one of its models becomes
    /// available — the point of retiring model profiles (spec 048 R1).
    #[test]
    fn saving_a_provider_stores_its_key_and_endpoint() {
        let mut llm = LlmConfig::load_defaults_for_test();
        let mut modal = ModelsModal::new();
        modal.sel = PROVIDERS.iter().position(|p| p.id == "anthropic").unwrap();
        modal.load_selected(&llm);

        assert_eq!(
            modal.endpoint_buf,
            Provider::from_id("anthropic").unwrap().default_endpoint(),
            "a fresh provider offers its shipped endpoint"
        );

        modal.key_buf = "sk-test".into();
        modal.commit(&mut llm);

        assert_eq!(llm.provider_api_key("anthropic"), "sk-test");
        assert!(llm.provider_is_configured("anthropic"));
        println!("provider anthropic configured with a key and its default endpoint");
    }

    /// Blanking the key field must not erase a stored credential — deleting is
    /// an explicit action with its own confirmation.
    #[test]
    fn a_blank_key_field_does_not_erase_the_stored_key() {
        let mut llm = LlmConfig::load_defaults_for_test();
        llm.store_api_key(provider_key_slot("openai"), "keep-me");

        let mut modal = ModelsModal::new();
        modal.sel = PROVIDERS.iter().position(|p| p.id == "openai").unwrap();
        modal.load_selected(&llm);
        assert_eq!(modal.key_buf, "keep-me");

        modal.key_buf.clear();
        modal.commit(&mut llm);
        assert_eq!(llm.provider_api_key("openai"), "keep-me");
    }

    /// An edited endpoint is remembered as edited; clearing it returns to the
    /// shipped default rather than leaving the provider unreachable.
    #[test]
    fn an_emptied_endpoint_falls_back_to_the_shipped_default() {
        let mut llm = LlmConfig::load_defaults_for_test();
        let mut modal = ModelsModal::new();
        modal.sel = PROVIDERS.iter().position(|p| p.id == "alibaba").unwrap();
        modal.load_selected(&llm);

        modal.endpoint_buf = "https://dashscope.aliyuncs.com/compatible-mode/v1".into();
        modal.commit(&mut llm);
        let cfg = llm.provider_config("alibaba").unwrap();
        assert!(cfg.endpoint_user_edited, "a hand-typed host is remembered");

        modal.endpoint_buf.clear();
        modal.commit(&mut llm);
        let cfg = llm.provider_config("alibaba").unwrap();
        assert_eq!(
            cfg.endpoint,
            Provider::from_id("alibaba").unwrap().default_endpoint()
        );
        assert!(!cfg.endpoint_user_edited);
    }

    /// Opening the manager owes a fetch, so a configured provider's list is not
    /// empty until the developer clicks Refresh.
    #[test]
    fn opening_the_manager_owes_a_model_fetch() {
        assert!(ModelsModal::new().auto_fetch_pending);
    }

    /// Moving to another provider owes a fetch too.
    #[test]
    fn selecting_another_provider_owes_a_model_fetch() {
        let llm = LlmConfig::load_defaults_for_test();
        let mut modal = ModelsModal::new();
        modal.auto_fetch_pending = false;
        modal.load_selected(&llm);
        assert!(modal.auto_fetch_pending);
    }

    /// The automatic fetch must not manufacture an error nobody asked for.
    #[test]
    fn the_automatic_fetch_is_skipped_when_it_could_only_fail() {
        assert!(!provider_can_list_models("", "sk-key"));
        assert!(!provider_can_list_models("openai", ""));
        assert!(!provider_can_list_models("openai", "   "));
        assert!(provider_can_list_models("openai", "sk-key"));
        // A local Ollama lists without a key, and is where it helps most.
        assert!(provider_can_list_models("ollama", ""));
        // The hosted variant still needs one.
        assert!(!provider_can_list_models("ollama_cloud", ""));
    }

    /// Deleting a credential clears it from the store and the field.
    #[test]
    fn deleting_a_credential_forgets_it() {
        let mut llm = LlmConfig::load_defaults_for_test();
        llm.store_api_key(provider_key_slot("groq"), "sk-gone");
        let mut modal = ModelsModal::new();
        modal.sel = PROVIDERS.iter().position(|p| p.id == "groq").unwrap();
        modal.load_selected(&llm);
        assert_eq!(modal.key_buf, "sk-gone");

        llm.forget_credential_slot(&provider_key_slot("groq"));
        modal.key_buf.clear();
        assert!(llm.provider_api_key("groq").is_empty());
        assert!(!llm.provider_is_configured("groq"));
    }
}
