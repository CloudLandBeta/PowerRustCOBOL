// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Agent Manager modal (spec 028 R6) — the operator-approved mockup, in egui.
//!
//! Master–detail over the project agent database: the left rail lists agents
//! (companions nested under their primary, selection emphasized, the rest
//! dimmed); the right side is one panel of collapsible sections mirroring
//! the agent structure. Interior is partitioned with embedded panels — never
//! estimated heights (see the egui-paint-regressions skill).

use std::path::Path;

use eframe::egui;

use crate::agents_db::AgentsDb;
use crate::i18n::Tr;
use crate::llm::{api_key_slot, LlmConfig};

pub struct AgentsModal {
    pub open: bool,
    pub db: AgentsDb,
    sel: usize,
    prompt_buf: String,
    key_buf: String,
    filter: String,
    /// `Some(name-in-progress)` while the ＋ New inline row is active.
    new_name: Option<String>,
    confirm_delete: bool,
    error: Option<String>,
    /// `true` once anything changed (enables Apply).
    dirty: bool,
    seeded: usize,
}

/// What the caller (app.rs) must do after a frame of the modal.
#[derive(Default)]
pub struct AgentsModalAction {
    /// Settings were applied — persist `LlmConfig` and refresh the designer
    /// agent resolution (spec 028 R8).
    pub applied: bool,
}

impl AgentsModal {
    /// Load (and first-time seed, R7) the project's agents and open.
    pub fn open_for(project_dir: &Path, llm: &LlmConfig) -> Self {
        let mut db = AgentsDb::load(project_dir);
        let seeded = db.seed_from_legacy(llm);
        let mut m = Self {
            open: true,
            db,
            sel: 0,
            prompt_buf: String::new(),
            key_buf: String::new(),
            filter: String::new(),
            new_name: None,
            confirm_delete: false,
            error: None,
            dirty: seeded > 0,
            seeded,
        };
        m.load_selected(llm);
        m
    }

    fn load_selected(&mut self, llm: &LlmConfig) {
        self.confirm_delete = false;
        let Some(a) = self.db.agents.get(self.sel) else {
            self.prompt_buf.clear();
            self.key_buf.clear();
            return;
        };
        self.prompt_buf = self.db.load_prompt(&a.name);
        self.key_buf = llm
            .api_keys
            .get(&api_key_slot(&a.provider, &a.model))
            .cloned()
            .unwrap_or_default();
    }

    /// Stash the selected agent's prompt + key before leaving it.
    fn stash_selected(&mut self, llm: &mut LlmConfig) {
        let Some(a) = self.db.agents.get(self.sel).cloned() else {
            return;
        };
        let _ = self.db.save_prompt(&a.name, &self.prompt_buf);
        if !a.model.trim().is_empty() {
            let slot = api_key_slot(&a.provider, &a.model);
            if self.key_buf.trim().is_empty() {
                llm.api_keys.remove(&slot);
            } else {
                llm.api_keys.insert(slot, self.key_buf.clone());
            }
        }
    }

    fn select(&mut self, i: usize, llm: &mut LlmConfig) {
        if i == self.sel {
            return;
        }
        self.stash_selected(llm);
        self.sel = i;
        self.load_selected(llm);
    }

    fn apply(&mut self, llm: &mut LlmConfig) -> bool {
        self.stash_selected(llm);
        if let Err(e) = self.db.save_all() {
            self.error = Some(e);
            return false;
        }
        if let Err(e) = llm.save() {
            self.error = Some(e);
            return false;
        }
        self.dirty = false;
        true
    }

    /// One frame. Call every frame while `open`.
    pub fn show(&mut self, ctx: &egui::Context, llm: &mut LlmConfig, tr: &Tr) -> AgentsModalAction {
        let mut action = AgentsModalAction::default();
        if !self.open {
            return action;
        }
        let mut open = self.open;
        egui::Window::new(format!("🤖 {}", tr.agents_title))
            .id(egui::Id::new("agents_manager_modal"))
            .collapsible(false)
            .resizable(true)
            .default_size([980.0, 620.0])
            .min_size([720.0, 420.0])
            .open(&mut open)
            .show(ctx, |ui| {
                // Footer first, rail second, detail last: embedded panels
                // partition the window body exactly (no estimated heights).
                let mut close = false;
                egui::Panel::bottom(ui.id().with("agents_footer"))
                    .resizable(false)
                    .show_separator_line(true)
                    .frame(egui::Frame::NONE)
                    .show(ui, |ui| {
                        ui.add_space(6.0);
                        ui.horizontal(|ui| {
                            let violation = self.db.pair_rule_violation();
                            let missing = self.db.missing_key(llm);
                            if let Some(e) = &self.error {
                                ui.colored_label(egui::Color32::from_rgb(224, 120, 120), format!("⚠ {e}"));
                            } else if let Some((p, c)) = &violation {
                                ui.colored_label(
                                    egui::Color32::from_rgb(230, 192, 106),
                                    tr.agents_pair_rule.replacen("{}", p, 1).replacen("{}", c, 1),
                                );
                            } else if let Some(name) = &missing {
                                ui.colored_label(
                                    egui::Color32::from_rgb(230, 192, 106),
                                    tr.agents_missing_key.replacen("{}", name, 1),
                                );
                            } else {
                                let active = self.db.agents.iter().filter(|a| a.enabled).count();
                                ui.colored_label(
                                    egui::Color32::from_rgb(125, 214, 160),
                                    tr.agents_valid
                                        .replacen("{}", &active.to_string(), 1)
                                        .replacen("{}", &self.db.agents.len().to_string(), 1),
                                );
                            }
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                let can_commit = violation.is_none();
                                if ui.add_enabled(can_commit, egui::Button::new("OK")).clicked() {
                                    if self.apply(llm) {
                                        action.applied = true;
                                        close = true;
                                    }
                                }
                                if ui
                                    .add_enabled(can_commit && self.dirty, egui::Button::new(tr.btn_apply_raw))
                                    .clicked()
                                    && self.apply(llm)
                                {
                                    action.applied = true;
                                }
                                if ui.button(tr.btn_cancel).clicked() {
                                    close = true;
                                }
                            });
                        });
                    });

                egui::Panel::left(ui.id().with("agents_rail"))
                    .resizable(true)
                    .default_size(280.0)
                    .min_size(200.0)
                    .show(ui, |ui| self.rail_ui(ui, llm, tr));

                egui::CentralPanel::default()
                    .frame(egui::Frame::NONE)
                    .show(ui, |ui| {
                        egui::ScrollArea::vertical()
                            .id_salt("agents_detail_scroll")
                            .auto_shrink([false, false])
                            .show(ui, |ui| self.detail_ui(ui, llm, tr));
                    });

                if close {
                    self.open = false;
                }
            });
        self.open &= open;
        action
    }

    // ── Left rail ─────────────────────────────────────────────────────────
    fn rail_ui(&mut self, ui: &mut egui::Ui, llm: &mut LlmConfig, tr: &Tr) {
        ui.add_space(6.0);
        ui.horizontal(|ui| {
            ui.add(
                egui::TextEdit::singleline(&mut self.filter)
                    .hint_text(tr.agents_filter)
                    .desired_width(ui.available_width() - 64.0),
            );
            if ui.button(format!("＋ {}", tr.agents_new)).clicked() {
                self.new_name = Some(String::new());
                self.error = None;
            }
        });
        // Inline "new agent" row: the name is asked once and is immutable.
        if let Some(name) = &mut self.new_name {
            ui.add_space(4.0);
            let mut create = false;
            let mut cancel = false;
            ui.horizontal(|ui| {
                let resp = ui.add(
                    egui::TextEdit::singleline(name)
                        .hint_text(tr.agents_new_name_hint)
                        .desired_width(ui.available_width() - 96.0),
                );
                create = (resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)))
                    || ui.button("✔").clicked();
                cancel = ui.button("✖").clicked();
            });
            if create {
                let name = self.new_name.take().unwrap_or_default();
                self.stash_selected(llm);
                match self.db.create(&name, "") {
                    Ok(id) => {
                        self.dirty = true;
                        self.error = None;
                        if let Some(i) = self.db.agents.iter().position(|a| a.id == id) {
                            self.sel = i;
                            self.load_selected(llm);
                        }
                    }
                    Err(e) => {
                        self.error = Some(e);
                        self.new_name = Some(name);
                    }
                }
            } else if cancel {
                self.new_name = None;
            }
        }
        ui.add_space(6.0);
        ui.separator();

        let filter = self.filter.to_lowercase();
        egui::ScrollArea::vertical()
            .id_salt("agents_rail_scroll")
            .auto_shrink([false, false])
            .show(ui, |ui| {
                let entries: Vec<(usize, String, String, String, bool, bool)> = self
                    .db
                    .agents
                    .iter()
                    .enumerate()
                    .filter(|(_, a)| filter.is_empty() || a.name.to_lowercase().contains(&filter))
                    .map(|(i, a)| {
                        (
                            i,
                            a.name.clone(),
                            a.model.clone(),
                            a.provider.clone(),
                            a.enabled,
                            self.db.is_companion(&a.id),
                        )
                    })
                    .collect();
                for (i, name, model, provider, enabled, is_comp) in entries {
                    let selected = i == self.sel;
                    ui.horizontal(|ui| {
                        if is_comp {
                            ui.add_space(18.0);
                            ui.colored_label(egui::Color32::from_rgb(201, 162, 232), "└");
                        }
                        // Dim everything but the selection (operator request).
                        let alpha = if selected { 1.0 } else { 0.5 };
                        ui.scope(|ui| {
                            ui.set_opacity(alpha);
                            let dot = if enabled { "●" } else { "○" };
                            let label = format!(
                                "{dot} {name}\n    {} · {}",
                                if model.is_empty() { "—" } else { &model },
                                if provider.is_empty() { "—" } else { &provider },
                            );
                            if ui
                                .selectable_label(selected, egui::RichText::new(label).size(12.5))
                                .clicked()
                            {
                                self.select(i, llm);
                            }
                        });
                    });
                }
            });
    }

    // ── Right detail (one scroll of collapsible sections) ────────────────
    fn detail_ui(&mut self, ui: &mut egui::Ui, llm: &mut LlmConfig, tr: &Tr) {
        if self.seeded > 0 {
            ui.colored_label(
                egui::Color32::from_rgb(125, 214, 160),
                tr.agents_seeded.replacen("{}", &self.seeded.to_string(), 1),
            );
            ui.add_space(4.0);
        }
        let Some(agent) = self.db.agents.get(self.sel).cloned() else {
            ui.weak(tr.agents_empty);
            return;
        };
        let sel = self.sel;
        let mut changed = false;

        egui::Frame::group(ui.style()).show(ui, |ui| {
            ui.set_min_width(ui.available_width() - 12.0);

            // Identity ------------------------------------------------------
            egui::CollapsingHeader::new(egui::RichText::new(tr.agents_sec_identity).strong())
                .default_open(true)
                .show(ui, |ui| {
                    egui::Grid::new("ag_identity").num_columns(2).spacing([14.0, 7.0]).show(ui, |ui| {
                        ui.label(tr.agents_id);
                        ui.monospace(&agent.id);
                        ui.end_row();
                        ui.label(tr.agents_name);
                        ui.vertical(|ui| {
                            ui.monospace(&agent.name);
                            ui.weak(tr.agents_name_hint);
                        });
                        ui.end_row();
                        ui.label(tr.agents_purpose);
                        {
                            let a = &mut self.db.agents[sel];
                            changed |= ui
                                .add(egui::TextEdit::singleline(&mut a.purpose).desired_width(f32::INFINITY))
                                .changed();
                        }
                        ui.end_row();
                        ui.label(tr.agents_enabled);
                        {
                            let a = &mut self.db.agents[sel];
                            changed |= ui.checkbox(&mut a.enabled, "").changed();
                        }
                        ui.end_row();
                    });
                    ui.add_space(2.0);
                    if !self.confirm_delete {
                        if ui.button(format!("🗑 {}", tr.agents_delete)).clicked() {
                            self.confirm_delete = true;
                        }
                    } else if ui
                        .button(
                            egui::RichText::new(format!("🗑 {}", tr.agents_delete_confirm))
                                .color(egui::Color32::from_rgb(224, 120, 120)),
                        )
                        .clicked()
                    {
                        let id = agent.id.clone();
                        let _ = self.db.delete(&id);
                        self.sel = 0;
                        self.load_selected(llm);
                        self.dirty = true;
                        return;
                    }
                });
            ui.separator();

            // Runtime configuration ------------------------------------------
            egui::CollapsingHeader::new(egui::RichText::new(tr.agents_sec_runtime).strong())
                .default_open(true)
                .show(ui, |ui| {
                    egui::Grid::new("ag_runtime").num_columns(2).spacing([14.0, 7.0]).show(ui, |ui| {
                        ui.label(tr.settings_ai_provider);
                        {
                            let a = &mut self.db.agents[sel];
                            let prev = a.provider.clone();
                            egui::ComboBox::from_id_salt("ag_provider")
                                .selected_text(if a.provider.is_empty() { "—" } else { &a.provider })
                                .show_ui(ui, |ui| {
                                    for p in crate::llm::PROVIDERS.iter() {
                                        ui.selectable_value(&mut a.provider, p.id().to_owned(), p.label());
                                    }
                                });
                            if a.provider != prev {
                                if let Some(p) = crate::llm::Provider::from_id(&a.provider) {
                                    a.endpoint = p.default_endpoint().to_owned();
                                }
                                changed = true;
                            }
                        }
                        ui.end_row();
                        ui.label(tr.settings_ai_endpoint);
                        {
                            let a = &mut self.db.agents[sel];
                            changed |= ui
                                .add(egui::TextEdit::singleline(&mut a.endpoint).desired_width(f32::INFINITY))
                                .changed();
                        }
                        ui.end_row();
                        ui.label(tr.settings_ai_model);
                        {
                            let a = &mut self.db.agents[sel];
                            let prev_model = a.model.clone();
                            let r = ui.add(
                                egui::TextEdit::singleline(&mut a.model)
                                    .font(egui::TextStyle::Monospace)
                                    .desired_width(f32::INFINITY),
                            );
                            if r.changed() {
                                changed = true;
                            }
                            // Model switch = per-model key contract: restore
                            // the stored key or clear the field (spec 028 R4).
                            if r.lost_focus() && a.model != prev_model {
                                let slot = api_key_slot(&a.provider, &a.model);
                                self.key_buf =
                                    llm.api_keys.get(&slot).cloned().unwrap_or_default();
                            }
                        }
                        ui.end_row();
                        ui.label(tr.settings_ai_api_key);
                        ui.vertical(|ui| {
                            changed |= ui
                                .add(
                                    egui::TextEdit::singleline(&mut self.key_buf)
                                        .password(true)
                                        .desired_width(f32::INFINITY),
                                )
                                .changed();
                            ui.weak(tr.agents_key_hint);
                        });
                        ui.end_row();
                        ui.label(tr.agents_sampling);
                        {
                            let a = &mut self.db.agents[sel];
                            ui.horizontal(|ui| {
                                changed |= ui
                                    .add(egui::DragValue::new(&mut a.temperature).range(0.0..=2.0).speed(0.05))
                                    .changed();
                                ui.label("·");
                                changed |= ui
                                    .add(egui::DragValue::new(&mut a.max_tokens).range(256..=128000).speed(100))
                                    .changed();
                                ui.label("·");
                                changed |= ui
                                    .add(egui::DragValue::new(&mut a.timeout_secs).range(1..=1200))
                                    .changed();
                                ui.label("s");
                            });
                        }
                        ui.end_row();
                        ui.label(tr.agents_routing);
                        {
                            let a = &mut self.db.agents[sel];
                            changed |= ui
                                .add(egui::TextEdit::singleline(&mut a.routing).desired_width(f32::INFINITY))
                                .changed();
                        }
                        ui.end_row();
                    });
                });
            ui.separator();

            // Core instructions ---------------------------------------------
            egui::CollapsingHeader::new(egui::RichText::new(tr.agents_sec_core).strong())
                .default_open(true)
                .show(ui, |ui| {
                    ui.label(tr.agents_prompt);
                    let r = ui.add(
                        egui::TextEdit::multiline(&mut self.prompt_buf)
                            .font(egui::TextStyle::Monospace)
                            .desired_rows(10)
                            .desired_width(f32::INFINITY),
                    );
                    changed |= r.changed();
                    ui.weak(format!(
                        "{} agentic_ai/{}/{}_prompt.md",
                        tr.agents_prompt_hint, agent.name, agent.name
                    ));
                    ui.add_space(4.0);
                    string_list_ui(ui, tr.agents_steering, &mut self.db.agents[sel].steering, &mut changed);
                    string_list_ui(ui, tr.agents_policies, &mut self.db.agents[sel].policies, &mut changed);
                });
            ui.separator();

            // Capabilities ----------------------------------------------------
            egui::CollapsingHeader::new(egui::RichText::new(tr.agents_sec_capabilities).strong())
                .default_open(false)
                .show(ui, |ui| {
                    string_list_ui(ui, tr.agents_skills, &mut self.db.agents[sel].skills, &mut changed);
                    string_list_ui(ui, tr.agents_tools, &mut self.db.agents[sel].tools, &mut changed);
                });
            ui.separator();

            // Knowledge -------------------------------------------------------
            egui::CollapsingHeader::new(egui::RichText::new(tr.agents_sec_knowledge).strong())
                .default_open(false)
                .show(ui, |ui| {
                    string_list_ui(ui, tr.agents_references, &mut self.db.agents[sel].knowledge, &mut changed);
                });
            ui.separator();

            // On disk ---------------------------------------------------------
            egui::CollapsingHeader::new(egui::RichText::new(tr.agents_sec_disk).strong())
                .default_open(false)
                .show(ui, |ui| {
                    let n = &agent.name;
                    ui.monospace(format!(
                        "agentic_ai/{n}/\n├── {n}_prompt.md\n├── steering/\n├── policies.md\n├── skills/\n├── mcp.json\n├── knowledge/\n└── agent.json"
                    ));
                    ui.weak(tr.agents_disk_hint);
                });
            ui.separator();

            // Companion -------------------------------------------------------
            if self.db.is_companion(&agent.id) {
                let owner = self
                    .db
                    .agents
                    .iter()
                    .find(|a| a.companion.as_deref() == Some(agent.id.as_str()))
                    .map(|a| a.name.clone())
                    .unwrap_or_default();
                ui.colored_label(
                    egui::Color32::from_rgb(201, 162, 232),
                    format!("🔍 {}", tr.agents_companion_of.replacen("{}", &owner, 1)),
                );
            } else {
                egui::CollapsingHeader::new(egui::RichText::new(tr.agents_sec_companion).strong())
                    .default_open(true)
                    .show(ui, |ui| {
                        let current = agent.companion.clone();
                        let current_name = current
                            .as_ref()
                            .and_then(|id| self.db.by_id(id))
                            .map(|a| a.name.clone());
                        let mut pick: Option<Option<String>> = None;
                        egui::ComboBox::from_id_salt("ag_companion")
                            .selected_text(current_name.unwrap_or_else(|| tr.agents_companion_none.to_string()))
                            .width(320.0)
                            .show_ui(ui, |ui| {
                                if ui
                                    .selectable_label(current.is_none(), tr.agents_companion_none)
                                    .clicked()
                                {
                                    pick = Some(None);
                                }
                                let others: Vec<(String, String)> = self
                                    .db
                                    .agents
                                    .iter()
                                    .filter(|x| x.id != agent.id)
                                    .map(|x| (x.id.clone(), format!("{} ({})", x.name, x.model)))
                                    .collect();
                                for (id, label) in others {
                                    if ui
                                        .selectable_label(current.as_deref() == Some(id.as_str()), label)
                                        .clicked()
                                    {
                                        pick = Some(Some(id));
                                    }
                                }
                            });
                        if let Some(p) = pick {
                            self.db.agents[sel].companion = p;
                            changed = true;
                        }
                        ui.weak(tr.agents_companion_hint);
                    });
            }
        });

        if changed {
            self.dirty = true;
            self.seeded = 0;
        }
    }
}

/// A label + editable string list (chips-lite): one row per entry with a
/// remove button, plus an add field.
fn string_list_ui(ui: &mut egui::Ui, label: &str, items: &mut Vec<String>, changed: &mut bool) {
    ui.horizontal_wrapped(|ui| {
        ui.label(label);
        let mut rm: Option<usize> = None;
        for (i, it) in items.iter().enumerate() {
            if ui.small_button(format!("{it} ✕")).clicked() {
                rm = Some(i);
            }
        }
        if let Some(i) = rm {
            items.remove(i);
            *changed = true;
        }
        let id = ui.id().with(label).with("add");
        let mut buf: String = ui.data_mut(|d| d.get_temp(id).unwrap_or_default());
        let r = ui.add(egui::TextEdit::singleline(&mut buf).hint_text("＋").desired_width(110.0));
        if r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) && !buf.trim().is_empty()
        {
            items.push(buf.trim().to_string());
            buf.clear();
            *changed = true;
        }
        ui.data_mut(|d| d.insert_temp(id, buf));
    });
}
