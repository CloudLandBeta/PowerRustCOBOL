// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Models Manager modal (spec 031).
//!
//! Model connections are defined **once** here as reusable, global
//! [`ModelProfile`](crate::llm::ModelProfile)s — provider, endpoint, model id,
//! sampling, and (via the global secret store) an API key — and referenced by
//! agents in the Agent Manager. This modal owns the full connection UI that used
//! to be repeated per agent: provider → endpoint → key → model (+ fetch list),
//! sampling, test connection, and a proficiency check, plus profile CRUD.
//!
//! The API key never lives on the profile or in any project file: it is stored
//! in [`LlmConfig::api_keys`] keyed by `(provider, model)` (spec 031 R7).

use std::sync::mpsc::Receiver;

use crate::i18n::Tr;
use crate::llm::{api_key_slot, LlmConfig, LlmResponse, ModelProfile};

pub struct ModelsModal {
    pub open: bool,
    /// Index into `llm.model_profiles`, clamped each frame.
    sel: usize,
    /// Edited API key for the selected profile (persisted into `api_keys`).
    key_buf: String,
    confirm_delete: bool,
    error: Option<String>,
    /// In-flight model-list fetch for the selected profile.
    models_rx: Option<Receiver<Result<Vec<String>, String>>>,
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
}

impl ModelsModal {
    pub fn new() -> Self {
        Self {
            open: true,
            sel: 0,
            key_buf: String::new(),
            confirm_delete: false,
            error: None,
            models_rx: None,
            available_models: Vec::new(),
            models_msg: None,
            test_rx: None,
            test_msg: None,
        }
    }

    /// Load the selected profile's stored key into the edit buffer.
    fn load_key(&mut self, llm: &LlmConfig) {
        self.available_models.clear();
        self.models_msg = None;
        self.test_msg = None;
        self.confirm_delete = false;
        self.key_buf = llm
            .model_profiles
            .get(self.sel)
            .map(|p| llm.api_keys.get(&api_key_slot(&p.provider, &p.model)).cloned().unwrap_or_default())
            .unwrap_or_default();
    }

    pub fn show(&mut self, ctx: &egui::Context, llm: &mut LlmConfig, tr: &Tr) -> ModelsModalAction {
        let mut action = ModelsModalAction::default();
        let mut open = self.open;

        // Drain an in-flight model-list fetch.
        if let Some(rx) = &self.models_rx {
            if let Ok(res) = rx.try_recv() {
                match res {
                    Ok(models) => {
                        self.models_msg = Some(format!("{} model(s) available", models.len()));
                        self.available_models = crate::llm::filter_retired_models(models);
                    }
                    Err(e) => self.models_msg = Some(e),
                }
                self.models_rx = None;
            }
        }
        // Drain an in-flight test-connection request.
        if let Some(rx) = &self.test_rx {
            if let Ok(res) = rx.try_recv() {
                self.test_msg = Some(match res {
                    LlmResponse::Ok(_) | LlmResponse::Chunk(_) => tr.ai_test_ok.to_string(),
                    LlmResponse::Err(e) => format!("{}: {e}", tr.ai_test_failed_title),
                });
                self.test_rx = None;
            }
        }

        let mut do_fetch = false;
        let mut do_test = false;
        let mut do_new = false;
        let mut do_duplicate = false;
        let mut do_delete = false;

        // Fixed size: the modal must not self-inflate. It opens at a set size and
        // its inner lists scroll rather than pushing the window taller.
        egui::Window::new(format!("🧠 {}", tr.models_title))
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .fixed_size([780.0, 560.0])
            .show(ctx, |ui| {
                if self.sel >= llm.model_profiles.len() {
                    self.sel = llm.model_profiles.len().saturating_sub(1);
                }
                ui.horizontal_top(|ui| {
                    // ── Left rail: profile list ──────────────────────────────
                    ui.vertical(|ui| {
                        ui.set_min_width(220.0);
                        ui.set_max_width(220.0);
                        if ui.button(format!("＋ {}", tr.models_new)).clicked() {
                            do_new = true;
                        }
                        ui.add_space(4.0);
                        egui::ScrollArea::vertical()
                            .auto_shrink([false, false])
                            .max_height(430.0)
                            .show(ui, |ui| {
                                if llm.model_profiles.is_empty() {
                                    ui.weak(tr.models_none);
                                }
                                for i in 0..llm.model_profiles.len() {
                                    let name = llm.model_profiles[i].name.clone();
                                    let model = llm.model_profiles[i].model.clone();
                                    if ui
                                        .selectable_label(
                                            i == self.sel,
                                            egui::RichText::new(if name.is_empty() { "—".into() } else { name }),
                                        )
                                        .on_hover_text(model)
                                        .clicked()
                                        && i != self.sel
                                    {
                                        self.sel = i;
                                        self.load_key(llm);
                                    }
                                }
                            });
                    });

                    ui.separator();

                    // ── Right pane: selected profile editor ──────────────────
                    ui.vertical(|ui| {
                        let Some(_) = llm.model_profiles.get(self.sel) else {
                            ui.weak(tr.models_select);
                            return;
                        };
                        egui::Grid::new("mp_grid").num_columns(2).spacing([14.0, 7.0]).show(ui, |ui| {
                            ui.label(tr.models_name);
                            {
                                let p = &mut llm.model_profiles[self.sel];
                                if ui.add(egui::TextEdit::singleline(&mut p.name).desired_width(f32::INFINITY)).changed() {
                                    action.applied = true;
                                }
                            }
                            ui.end_row();

                            ui.label(tr.settings_ai_provider);
                            {
                                let p = &mut llm.model_profiles[self.sel];
                                let prev = p.provider.clone();
                                egui::ComboBox::from_id_salt("mp_provider")
                                    .selected_text(if p.provider.is_empty() { "—" } else { &p.provider })
                                    .show_ui(ui, |ui| {
                                        for prov in crate::llm::PROVIDERS.iter() {
                                            ui.selectable_value(&mut p.provider, prov.id().to_owned(), prov.label());
                                        }
                                    });
                                if p.provider != prev {
                                    if let Some(prov) = crate::llm::Provider::from_id(&p.provider) {
                                        p.endpoint = prov.default_endpoint().to_owned();
                                    }
                                    action.applied = true;
                                }
                            }
                            ui.end_row();

                            ui.label(tr.settings_ai_endpoint);
                            {
                                let p = &mut llm.model_profiles[self.sel];
                                if ui.add(egui::TextEdit::singleline(&mut p.endpoint).desired_width(f32::INFINITY)).changed() {
                                    action.applied = true;
                                }
                            }
                            ui.end_row();

                            ui.label(tr.settings_ai_api_key);
                            if ui
                                .add(egui::TextEdit::singleline(&mut self.key_buf).password(true).desired_width(f32::INFINITY))
                                .changed()
                            {
                                action.applied = true;
                            }
                            ui.end_row();
                            ui.label("");
                            ui.weak(tr.agents_key_hint);
                            ui.end_row();

                            ui.label(tr.settings_ai_model);
                            {
                                let prev_model;
                                {
                                    let p = &mut llm.model_profiles[self.sel];
                                    prev_model = p.model.clone();
                                    ui.horizontal(|ui| {
                                        if ui
                                            .add(
                                                egui::TextEdit::singleline(&mut p.model)
                                                    .font(egui::TextStyle::Monospace)
                                                    .desired_width(180.0),
                                            )
                                            .changed()
                                        {
                                            action.applied = true;
                                        }
                                        if !self.available_models.is_empty() {
                                            egui::ComboBox::from_id_salt("mp_model_pick")
                                                .selected_text("▾")
                                                .width(28.0)
                                                .show_ui(ui, |ui| {
                                                    for m in &self.available_models {
                                                        if ui.selectable_label(&p.model == m, m).clicked() {
                                                            p.model = m.clone();
                                                            action.applied = true;
                                                        }
                                                    }
                                                });
                                        }
                                        if ui
                                            .add_enabled(self.models_rx.is_none(), egui::Button::new(tr.settings_ai_refresh))
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
                                }
                                // Model switch = per-model key contract: restore
                                // the stored key or clear the field.
                                let p = &llm.model_profiles[self.sel];
                                if p.model != prev_model {
                                    let slot = api_key_slot(&p.provider, &p.model);
                                    self.key_buf = llm.api_keys.get(&slot).cloned().unwrap_or_default();
                                }
                            }
                            ui.end_row();

                            ui.label(tr.agents_sampling);
                            {
                                let p = &mut llm.model_profiles[self.sel];
                                ui.horizontal(|ui| {
                                    if ui.add(egui::DragValue::new(&mut p.temperature).range(0.0..=2.0).speed(0.05)).changed() {
                                        action.applied = true;
                                    }
                                    ui.label("·");
                                    if ui.add(egui::DragValue::new(&mut p.max_tokens).range(256..=128000).speed(100)).changed() {
                                        action.applied = true;
                                    }
                                    ui.label("·");
                                    if ui.add(egui::DragValue::new(&mut p.timeout_secs).range(1..=1200)).changed() {
                                        action.applied = true;
                                    }
                                    ui.label("s");
                                });
                            }
                            ui.end_row();
                        });

                        ui.add_space(8.0);
                        ui.horizontal(|ui| {
                            if ui.button(format!("🔌 {}", tr.settings_ai_test)).clicked() {
                                do_test = true;
                            }
                            if ui.button(format!("⧉ {}", tr.models_duplicate)).clicked() {
                                do_duplicate = true;
                            }
                            if !self.confirm_delete {
                                if ui
                                    .button(egui::RichText::new(format!("🗑 {}", tr.models_delete)).color(egui::Color32::from_rgb(224, 120, 120)))
                                    .clicked()
                                {
                                    self.confirm_delete = true;
                                }
                            } else {
                                ui.label(egui::RichText::new(tr.models_delete_warn).small().color(egui::Color32::from_rgb(224, 160, 120)));
                                if ui.button(tr.models_delete).clicked() {
                                    do_delete = true;
                                }
                                if ui.button(tr.btn_cancel).clicked() {
                                    self.confirm_delete = false;
                                }
                            }
                        });
                        if let Some(m) = &self.test_msg {
                            ui.add_space(4.0);
                            ui.label(egui::RichText::new(m).small());
                        }
                    });
                });

                if let Some(e) = &self.error {
                    ui.add_space(4.0);
                    ui.colored_label(egui::Color32::from_rgb(224, 120, 120), e);
                }
                ui.separator();
                ui.horizontal(|ui| {
                    if ui.button(tr.inspect_close).clicked() {
                        self.open = false;
                    }
                });
            });

        // ── Deferred mutations (avoid double-borrowing inside the UI) ────────
        if do_new {
            let id = crate::agents_db::new_uuid();
            llm.model_profiles.push(ModelProfile {
                id,
                name: tr.models_new_name.to_string(),
                provider: String::new(),
                endpoint: String::new(),
                model: String::new(),
                temperature: 0.7,
                max_tokens: 8192,
                timeout_secs: 30,
            });
            self.sel = llm.model_profiles.len() - 1;
            self.load_key(llm);
            action.applied = true;
        }
        if do_duplicate {
            if let Some(src) = llm.model_profiles.get(self.sel).cloned() {
                let mut copy = src.clone();
                copy.id = crate::agents_db::new_uuid();
                copy.name = format!("{} (copy)", src.name);
                llm.model_profiles.push(copy);
                self.sel = llm.model_profiles.len() - 1;
                self.load_key(llm);
                action.applied = true;
            }
        }
        if do_delete {
            if self.sel < llm.model_profiles.len() {
                llm.model_profiles.remove(self.sel);
                self.confirm_delete = false;
                self.sel = self.sel.saturating_sub(1);
                self.load_key(llm);
                action.applied = true;
            }
        }
        // Persist the edited key into the global secret store (spec 031 R7).
        if let Some(p) = llm.model_profiles.get(self.sel) {
            if !p.model.trim().is_empty() {
                let slot = api_key_slot(&p.provider, &p.model);
                if self.key_buf.is_empty() {
                    llm.api_keys.remove(&slot);
                } else {
                    llm.api_keys.insert(slot, self.key_buf.clone());
                }
            }
        }
        if do_fetch {
            if let Some(p) = llm.model_profiles.get(self.sel) {
                match crate::llm::Provider::from_id(&p.provider) {
                    Some(provider) => {
                        self.available_models.clear();
                        self.models_msg = Some(tr.ai_detecting.to_string());
                        self.models_rx = Some(crate::llm::spawn_list_models(provider, &p.endpoint, &self.key_buf));
                    }
                    None => self.models_msg = Some(tr.settings_ai_provider_select.to_string()),
                }
            }
        }
        if do_test {
            if let Some(p) = llm.model_profiles.get(self.sel) {
                self.test_msg = Some(tr.ai_testing.to_string());
                self.test_rx = Some(crate::llm::spawn_test(&p.resolve(llm)));
            }
        }

        if !open {
            self.open = false;
        }
        action
    }
}
