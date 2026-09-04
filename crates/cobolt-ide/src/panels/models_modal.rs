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

/// `file_dialog` key for the credential-file Browse… picker.
const CREDENTIAL_FILE_DIALOG: &str = "models:credential-file";

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
    /// The model the in-flight test is actually using, so every line the
    /// developer reads names it (operator, 2026-09-04).
    test_model: Option<String>,
    /// The credential-file path being edited, and the last thing the store said
    /// about it — a refusal, or where the keys were written.
    store_path_buf: String,
    store_msg: Option<String>,
    store_refused: bool,
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
    /// A provider's catalogue came back **successfully and non-empty**:
    /// `(provider id, models)`. The app uses it to retire leaderboard rows for
    /// models that provider no longer offers.
    ///
    /// Only ever set from a successful listing. A failed request and a provider
    /// that genuinely offers nothing are indistinguishable here, and the second
    /// does not happen — so an empty or failed result reports nothing at all
    /// rather than something that would read as "everything was decommissioned".
    pub catalogue: Option<(String, Vec<String>)>,
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
            test_model: None,
            store_path_buf: String::new(),
            store_msg: None,
            store_refused: false,
            semantic_ready: None,
        }
    }

    /// Where the developer's keys are kept, and the one rule about it.
    ///
    /// Not per provider: this is where EVERY key goes. It sits at the foot of the
    /// manager because it is the answer to "will I have to paste this again
    /// tomorrow?", which is the question the key field above raises.
    fn storage_section(&mut self, ui: &mut egui::Ui, llm: &mut LlmConfig, tr: &Tr) -> bool {
        use crate::model_config_store::{self as store, Vault};

        let mut changed = false;
        ui.add_space(10.0);
        ui.separator();
        ui.label(egui::RichText::new(tr.creds_where).strong());
        ui.label(egui::RichText::new(tr.creds_where_hint).small().weak());
        ui.add_space(4.0);

        if self.store_path_buf.is_empty() {
            self.store_path_buf = llm.credential_file_path().display().to_string();
        }

        for vault in Vault::ALL.iter().copied() {
            let label = match vault {
                Vault::Session => tr.creds_session,
                Vault::LocalFile => tr.creds_local_file,
                Vault::OsVault => tr.creds_os_vault,
            };
            let available = vault.available();
            ui.horizontal(|ui| {
                let mut chosen = llm.credential_vault == vault;
                let response = ui.add_enabled(
                    available,
                    egui::RadioButton::new(chosen, label),
                );
                if response.clicked() && available {
                    chosen = true;
                    llm.credential_vault = vault;
                    self.store_msg = None;
                    self.store_refused = false;
                    changed = true;
                }
                let _ = chosen;
                if !available {
                    ui.label(
                        egui::RichText::new(
                            tr.creds_ships_in
                                .replacen("{}", store::OS_VAULT_SHIPS_IN, 1),
                        )
                        .small()
                        .weak(),
                    );
                }
            });
        }

        if llm.credential_vault != Vault::LocalFile {
            return changed;
        }

        ui.add_space(6.0);
        ui.horizontal(|ui| {
            ui.label(tr.creds_file);
            if ui
                .add(egui::TextEdit::singleline(&mut self.store_path_buf).desired_width(320.0))
                .changed()
            {
                self.store_msg = None;
                self.store_refused = false;
            }
            if ui.button(tr.creds_browse).clicked() {
                crate::file_dialog::begin(
                    ui.ctx(),
                    CREDENTIAL_FILE_DIALOG,
                    crate::file_dialog::DialogSpec::save()
                        .file_name("llm_config.json")
                        .filter("JSON", &["json"]),
                );
            }
        });

        // The suggested paths, as one-click buttons. `/tmp` first: nothing there
        // can be committed, and it does not survive a reboot (operator).
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new(tr.creds_suggested).small().weak());
            for path in store::suggested_paths() {
                let shown = path.display().to_string();
                if ui.small_button(&shown).clicked() {
                    self.store_path_buf = shown;
                    self.store_msg = None;
                    self.store_refused = false;
                }
            }
        });

        ui.add_space(4.0);
        ui.horizontal(|ui| {
            if ui.button(tr.creds_use_file).clicked() {
                let path = std::path::PathBuf::from(self.store_path_buf.trim());
                match store::validate(&path) {
                    Ok(()) => {
                        llm.credential_file = path.display().to_string();
                        self.store_refused = false;
                        self.store_msg = Some(
                            tr.creds_will_write
                                .replacen("{}", &path.display().to_string(), 1),
                        );
                        changed = true;
                    }
                    Err(refusal) => {
                        crate::error_log::record(refusal.message());
                        self.store_refused = true;
                        self.store_msg = Some(refusal.message().to_owned());
                    }
                }
            }
            if ui.button(tr.creds_forget_file).clicked() {
                let path = llm.credential_file_path();
                match store::forget(&path) {
                    Ok(()) => {
                        llm.credential_vault = Vault::Session;
                        self.store_refused = false;
                        self.store_msg =
                            Some(tr.creds_forgot.replacen("{}", &path.display().to_string(), 1));
                        changed = true;
                    }
                    Err(e) => {
                        self.store_refused = true;
                        self.store_msg = Some(e);
                    }
                }
            }
        });

        if let Some(msg) = &self.store_msg {
            ui.add_space(4.0);
            let text = egui::RichText::new(msg).small();
            ui.label(if self.store_refused {
                text.color(ui.visuals().error_fg_color)
            } else {
                text
            });
        }
        changed
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
        self.test_model = None;
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
                            if !models.is_empty() {
                                action.catalogue =
                                    Some((self.selected_provider().to_owned(), models.clone()));
                            }
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
                // The outcome names the model it is about: the test picks the
                // model itself, so a bare verdict is a verdict about nothing
                // the developer can see.
                let msg = match &self.test_model {
                    Some(m) => format!("{m}: {msg}"),
                    None => msg,
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
        let mut storage_changed = false;

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
                //
                // It SCROLLS. This pane is the only one here whose height is
                // decided by its content — the storage section alone is a
                // heading, a hint and four radio choices — while the space it
                // gets is whatever the window has left after the two bottom
                // panels and the left rail. Without a scroll area the surplus
                // was simply clipped: "Where keys are kept" was cut off
                // mid-sentence and no drag, wheel or resize could reach the
                // choices under it (operator, 2026-08-20).
                //
                // `auto_shrink([false, false])` is what keeps the fix from
                // becoming the other bug: the scroll area fills the rect it is
                // given instead of reporting its content's height back up, so a
                // tall provider can never push the window wider or taller —
                // the same rule the left rail already follows.
                egui::CentralPanel::default()
                    .frame(egui::Frame::NONE)
                    .show(ui, |ui| {
                        egui::ScrollArea::vertical()
                            .id_salt("provider_details_scroll")
                            .auto_shrink([false, false])
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
                                    ui.label(
                                        egui::RichText::new(tr.providers_local_no_key).small(),
                                    );
                                    ui.add_space(4.0);
                                } else if !llm.provider_is_configured(id) {
                                    ui.label(
                                        egui::RichText::new(tr.providers_unconfigured).small(),
                                    );
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
                                                    egui::TextEdit::singleline(
                                                        &mut self.endpoint_buf,
                                                    )
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
                                            ui.label(
                                                egui::RichText::new(tr.agents_key_hint).small(),
                                            );
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
                                                    egui::RichText::new(tr.settings_ai_model_empty)
                                                        .small(),
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
                                        egui::RichText::new(e)
                                            .color(ui.visuals().error_fg_color)
                                            .small(),
                                    );
                                }

                                // Where every key is kept — the answer to "will
                                // I have to paste this again tomorrow?".
                                if self.storage_section(ui, llm, tr) {
                                    storage_changed = true;
                                }
                                // Breathing room under the last radio so it is
                                // never flush against the panel separator when
                                // scrolled to the bottom.
                                ui.add_space(6.0);
                            });
                    });
            });

        // A Browse… choice arrives on a later frame (a synchronous dialog nests
        // the OS event loop, which aborts winit).
        if let Some(picked) = crate::file_dialog::take(CREDENTIAL_FILE_DIALOG) {
            if let Some(path) = picked {
                self.store_path_buf = path.display().to_string();
                self.store_msg = None;
                self.store_refused = false;
            }
        }

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
            // WHICH model the test uses: the one actually in use when it
            // belongs to this provider, else the provider's first.
            //
            // Until 1.64.21 it was unconditionally the first registered
            // model, so a provider was tested under a model the developer had
            // never chosen and every failure named THAT model — including the
            // 401 help, which then asked them to check whether a model they
            // had never selected was still offered (operator, 2026-09-04:
            // testing gemma4 reported nemotron-3-ultra).
            let model = test_model_for(llm.models_for(&id), &llm.model).map(str::to_string);
            match model {
                // An empty model id reaches the provider as a malformed
                // request and comes back as 401/400 — indistinguishable from
                // a bad key, and the developer goes hunting for a credential
                // that was never the problem.
                None => {
                    self.test_model = None;
                    self.test_msg = Some(tr.providers_test_no_models.to_string());
                }
                Some(model) => {
                    let mut cfg = llm.clone();
                    cfg.provider = id.clone();
                    cfg.endpoint = self.endpoint_buf.clone();
                    cfg.api_key = self.key_buf.clone();
                    cfg.model = model.clone();
                    self.test_rx = Some(crate::llm::spawn_test(&cfg));
                    self.test_msg = Some(tr.providers_test_model.replacen("{}", &model, 1));
                    self.test_model = Some(model);
                }
            }
        }
        if do_save {
            self.save_and_close(llm);
            action.applied = true;
            action.save_requested = true;
        }
        if do_semantic_download {
            action.semantic_download_requested = true;
        }
        // A storage choice must reach disk as soon as it is made: it decides
        // whether the key already typed above survives the session.
        if storage_changed {
            action.applied = true;
            action.save_requested = true;
        }

        if !open {
            self.open = false;
        }
        action
    }
}

/// Which model a provider's connection test should send to.
///
/// The model actually in use when this provider offers it, and otherwise the
/// provider's first. `None` when the provider has no models registered — the
/// caller must not send a request with an empty model id, which every provider
/// answers as an authorization or bad-request failure indistinguishable from a
/// wrong key.
fn test_model_for<'a>(models: &'a [String], in_use: &str) -> Option<&'a str> {
    models
        .iter()
        .find(|m| m.trim() == in_use.trim() && !m.trim().is_empty())
        .or_else(|| models.first())
        .map(String::as_str)
}

#[cfg(test)]
mod tests {
    /// **A connection test must test the model the developer is using.**
    ///
    /// The button lives on a provider row with no model picker, so before
    /// 1.64.21 it always sent the provider's FIRST registered model. Testing a
    /// provider configured for gemma4 therefore exercised nemotron-3-ultra,
    /// and the 401 help then told the developer to check whether a model they
    /// had never chosen was still offered (operator, 2026-09-04).
    #[test]
    fn the_test_uses_the_model_in_use_when_the_provider_offers_it() {
        let models = vec![
            "nemotron-3-ultra".to_string(),
            "gemma4".to_string(),
            "qwen3".to_string(),
        ];
        assert_eq!(test_model_for(&models, "gemma4"), Some("gemma4"));
    }

    /// The modal can be opened on a provider that has nothing to do with the
    /// model in use; that provider's first model is then the only sensible
    /// probe, and it is what the messages will name.
    #[test]
    fn a_provider_that_does_not_offer_the_model_in_use_falls_back_to_its_first() {
        let models = vec!["nemotron-3-ultra".to_string(), "qwen3".to_string()];
        assert_eq!(
            test_model_for(&models, "claude-opus-5"),
            Some("nemotron-3-ultra")
        );
    }

    /// An empty model id reaches the provider as a malformed request and comes
    /// back 401/400 — the failure that sends a developer hunting for a
    /// credential that was never the problem. There must be no request at all.
    #[test]
    fn a_provider_with_no_models_yields_no_model_to_test() {
        assert_eq!(test_model_for(&[], "gemma4"), None);
    }

    /// Both messages the developer reads must carry the placeholder the model
    /// name is substituted into, in every language.
    #[test]
    fn the_test_messages_name_the_model_in_every_language() {
        for &lang in crate::i18n::Language::ALL {
            let tr = lang.tr();
            assert_eq!(
                tr.providers_test_model.matches("{}").count(),
                1,
                "{lang:?}: providers_test_model must carry the model name"
            );
            assert!(
                !tr.providers_test_no_models.trim().is_empty(),
                "{lang:?}: providers_test_no_models"
            );
        }
    }

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

    /// **The provider pane scrolls, and it does not grow the window doing it.**
    ///
    /// The pane's content is content-sized (endpoint, key, model row, and the
    /// four "Where keys are kept" choices) while the room it gets is whatever
    /// the window has left. With no scroll area the surplus was simply clipped
    /// and the storage choices could not be reached by any means (operator,
    /// 2026-08-20). The obvious fix is also the classic way to reintroduce this
    /// codebase's oldest bug — a child sized from available space inside a
    /// window that sizes itself from its content — so this is what it pins: the
    /// rendered window is stable across frames of unchanged state, and it never
    /// grows past the screen it was given. Run in every language, because a
    /// longer translation is exactly what would tip it over — French is the
    /// widest and wants about 920 px of its own accord, so the test screen is
    /// wide enough to let it have that and short enough that the vertical
    /// overflow this fix is about is the binding constraint.
    ///
    /// What it does NOT prove is that the pane scrolls: from outside, a window
    /// clamped to the screen and clipping looks the same as one that scrolls,
    /// and the scroll area's own id is derived from its parent Ui and is not
    /// reconstructable here. That half is the operator's to see.
    #[test]
    fn the_provider_pane_scrolls_without_inflating_the_window() {
        let ctx = egui::Context::default();
        ctx.set_fonts(egui::FontDefinitions::default());
        let id = egui::Id::new("model_providers_manager_modal");
        let screen =
            egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1200.0, 560.0));

        for &lang in crate::i18n::Language::ALL {
            let tr = lang.tr();
            let mut llm = LlmConfig::load_defaults_for_test();
            let mut modal = ModelsModal::new();
            // No network from a unit test: the manager owes a fetch on open.
            modal.auto_fetch_pending = false;

            let mut frame = |modal: &mut ModelsModal, llm: &mut LlmConfig| {
                let input = egui::RawInput {
                    screen_rect: Some(screen),
                    ..Default::default()
                };
                ctx.run_ui(input, |ui| {
                    let ctx = ui.ctx().clone();
                    let _ = modal.show(&ctx, llm, &tr);
                })
                .textures_delta
                .clear();
            };

            // egui needs one frame to discover a window's content-driven size,
            // so the first frame an id is ever shown is not a fair baseline —
            // a real inflation loop keeps growing after it, not once.
            frame(&mut modal, &mut llm);
            frame(&mut modal, &mut llm);
            let first = ctx.memory(|m| m.area_rect(id)).expect("modal rendered");
            frame(&mut modal, &mut llm);
            let second = ctx.memory(|m| m.area_rect(id)).expect("modal rendered");

            assert!(
                (first.width() - second.width()).abs() < 0.5
                    && (first.height() - second.height()).abs() < 0.5,
                "{lang:?}: the manager's rendered rect changed between two \
                 frames of unchanged state ({first:?} -> {second:?}) — the \
                 details pane is sizing the window instead of scrolling inside it"
            );
            assert!(
                second.height() <= screen.height() + 0.5
                    && second.width() <= screen.width() + 0.5,
                "{lang:?}: the manager grew past its screen ({second:?} vs \
                 {screen:?}) — content that does not fit must scroll, never \
                 push the window off the display"
            );
        }
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
