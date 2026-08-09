// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Agents Manager modal (spec 028 R6) — the operator-approved mockup, in egui.
//!
//! Master–detail over the project agent database: the left rail lists agents
//! (companions nested under their primary, selection emphasized, the rest
//! dimmed); the right side is one panel of collapsible sections mirroring
//! the agent structure. Interior is partitioned with embedded panels — never
//! estimated heights (see the egui-paint-regressions skill).

use std::path::Path;

use eframe::egui;

use crate::agents_db::{AgentKind, AgentsDb};
use crate::i18n::Tr;
use crate::llm::{api_key_slot, LlmConfig};

/// Agent lifecycle controls remain implemented for future maintenance, but the
/// IDE currently provisions and repairs the complete built-in agent mesh.
const SHOW_NEW_AGENT_CONTROL: bool = false;
const SHOW_DELETE_AGENT_CONTROL: bool = false;

const PROMPT_EDITOR_MIN_ROWS: usize = 4;
const PROMPT_EDITOR_MAX_ROWS: usize = 20;
const PROMPT_EDITOR_VERTICAL_MARGIN: f32 = 4.0;

fn prompt_editor_height(ui: &egui::Ui, rows: usize) -> f32 {
    ui.text_style_height(&egui::TextStyle::Monospace) * rows as f32 + PROMPT_EDITOR_VERTICAL_MARGIN
}

struct PromptEditorOutput {
    response: egui::Response,
    #[cfg(test)]
    viewport_rect: egui::Rect,
    #[cfg(test)]
    content_height: f32,
}

fn prompt_editor(ui: &mut egui::Ui, id: egui::Id, prompt: &mut String) -> PromptEditorOutput {
    let width = ui.available_width();
    let min_height = prompt_editor_height(ui, PROMPT_EDITOR_MIN_ROWS);
    let max_height = prompt_editor_height(ui, PROMPT_EDITOR_MAX_ROWS);

    egui::Resize::default()
        .id(id.with("resize"))
        .resizable([false, true])
        .min_size(egui::vec2(width, min_height))
        .max_size(egui::vec2(width, max_height))
        .default_size(egui::vec2(width, max_height))
        .show(ui, |ui| {
            let size = ui.available_size().min(egui::vec2(width, max_height));
            ui.set_min_size(size);
            ui.set_max_size(size);
            let scroll = egui::ScrollArea::vertical()
                .id_salt(id.with("scroll"))
                .auto_shrink([false, false])
                .min_scrolled_height(size.y)
                .max_height(size.y)
                .show(ui, |ui| {
                    ui.add(
                        egui::TextEdit::multiline(prompt)
                            .id(id.with("text"))
                            .font(egui::TextStyle::Monospace)
                            .desired_rows(PROMPT_EDITOR_MIN_ROWS)
                            .desired_width(f32::INFINITY)
                            .margin(egui::Margin::symmetric(4, 2)),
                    )
                });
            PromptEditorOutput {
                response: scroll.inner,
                #[cfg(test)]
                viewport_rect: scroll.inner_rect,
                #[cfg(test)]
                content_height: scroll.content_size.y,
            }
        })
}

pub struct AgentsModal {
    pub open: bool,
    pub db: AgentsDb,
    sel: usize,
    prompt_buf: String,
    key_buf: String,
    filter: String,
    /// `Some(name-in-progress)` while the ＋ New inline row is active.
    new_name: Option<String>,
    /// Kind chosen for the agent being created in the ＋ New row (so a pedantic
    /// reviewer can be created directly, not created-then-converted).
    new_kind: crate::agents_db::AgentKind,
    confirm_delete: bool,
    error: Option<String>,
    /// `true` once anything changed (enables Apply).
    dirty: bool,
    seeded: usize,
    /// Which provider the runtime table's Model column offers (spec 048 R10).
    ///
    /// A **picker scope**, not a project switch: it changes what the dropdowns
    /// list while configuring, and never touches an agent's stored provider
    /// (R11). Grace can sit on a cloud provider while a specialist runs local
    /// Ollama; switching the scope to look at one does not disturb the other.
    provider_scope: String,
    /// Free-text filter for the Model column. One provider can offer several
    /// hundred models, and a flat dropdown that long hides the one you came for.
    model_filter: String,
}

/// What the caller (app.rs) must do after a frame of the modal.
#[derive(Default)]
pub struct AgentsModalAction {
    /// Settings were applied — persist `LlmConfig` and refresh the designer
    /// agent resolution (spec 028 R8).
    pub applied: bool,
    /// Run the COBOL proficiency check for this resolved agent config
    /// (specialist's model, reviewed by its pedantic companion when set).
    pub run_proficiency: Option<LlmConfig>,
}

impl AgentsModal {
    /// Show an error, and record it in the IDE console (operator, 2026-08-09).
    /// Closing the manager takes the message with it; the console keeps it.
    fn set_error(&mut self, message: impl Into<String>) {
        let message = message.into();
        crate::error_log::record(&message);
        self.error = Some(message);
    }

    /// Load (and first-time seed, R7) the project's agents and open.
    /// Open the manager with one agent already selected — used by the AI setup
    /// wizard's **Judge** button, which exists to take the developer straight to
    /// the COBOL Proficiency Judge rather than leaving them to find it in the
    /// rail (spec 040 R11).
    pub fn open_at(project_dir: &Path, llm: &mut LlmConfig, agent_name: &str) -> Self {
        let mut m = Self::open_for(project_dir, llm);
        if let Some(index) = m
            .db
            .agents
            .iter()
            .position(|a| a.name.eq_ignore_ascii_case(agent_name))
        {
            m.sel = index;
            m.load_selected(llm);
        }
        m
    }

    pub fn open_for(project_dir: &Path, llm: &mut LlmConfig) -> Self {
        let mut db = AgentsDb::load(project_dir);
        let mut seeded = db.ensure_fixed_agents(llm);
        // Spec 048: retire the profile layer, the opposite of what spec 031's
        // `migrate_to_profiles` used to do here. Leaving that call in place
        // would rebuild profiles out of the agents' own fields on every open,
        // undoing the migration project-open just performed.
        let report = db.migrate_profiles_to_providers(llm);
        if !report.is_empty() {
            let _ = db.save_all();
            let _ = llm.save();
            seeded += report.agents_migrated;
        }
        // Start the picker on a provider that is actually usable, so the Model
        // column is not empty on the first frame.
        let provider_scope = db
            .by_name(crate::agents_db::GRACE)
            .map(|g| g.provider.clone())
            .filter(|p| !p.trim().is_empty())
            .or_else(|| llm.configured_providers().first().cloned())
            .unwrap_or_default();
        let mut m = Self {
            open: true,
            db,
            sel: 0,
            prompt_buf: String::new(),
            key_buf: String::new(),
            filter: String::new(),
            new_name: None,
            new_kind: crate::agents_db::AgentKind::Specialist,
            confirm_delete: false,
            error: None,
            dirty: seeded > 0,
            seeded,
            provider_scope,
            model_filter: String::new(),
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
        self.key_buf = a
            .model_profile
            .as_deref()
            .and_then(|id| llm.profile(id))
            .map(|profile| profile.resolve(llm).api_key)
            .unwrap_or_else(|| {
                llm.api_keys
                    .get(&api_key_slot(&a.provider, &a.model))
                    .cloned()
                    .unwrap_or_default()
            });
    }

    /// Stash the selected agent's prompt + key before leaving it.
    fn stash_selected(&mut self, llm: &mut LlmConfig) {
        let Some(a) = self.db.agents.get(self.sel).cloned() else {
            return;
        };
        let _ = self.db.save_prompt(&a.name, &self.prompt_buf);
        if self.key_buf.trim().is_empty() {
            return;
        }
        if let Some(profile) = a
            .model_profile
            .as_deref()
            .and_then(|id| llm.profile(id))
            .cloned()
        {
            llm.store_api_key(crate::llm::profile_api_key_slot(&profile.id), &self.key_buf);
        } else if !a.model.trim().is_empty() {
            llm.store_api_key(api_key_slot(&a.provider, &a.model), &self.key_buf);
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
            self.set_error(e);
            return false;
        }
        if let Err(e) = llm.save() {
            self.set_error(e);
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
            .default_size([1120.0, 720.0])
            .min_size([820.0, 500.0])
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
                        // Compute the status message + colour once, so the
                        // footer layout is independent of which state fires.
                        let violation = self.db.pair_rule_violation();
                        let missing = self.db.missing_key(llm);
                        // Spec 040 R10: a specialist standing on Grace's or the
                        // judge's model. Ranked above the key/reviewer warnings
                        // because it silently invalidates a judgement rather
                        // than stopping a run.
                        let separation = self.db.model_separation(llm);
                        let (msg, msg_color) = if let Some(e) = &self.error {
                            (format!("⚠ {e}"), egui::Color32::from_rgb(224, 120, 120))
                        } else if let Some(clash) = separation.clashes.first() {
                            (
                                tr.agents_model_reserved
                                    .replacen("{}", &clash.agent, 1)
                                    .replacen("{}", clash.reserved_for, 1),
                                egui::Color32::from_rgb(230, 192, 106),
                            )
                        } else if separation.judge_shares_grace {
                            (
                                tr.agents_judge_shares_grace.to_string(),
                                egui::Color32::from_rgb(169, 206, 236),
                            )
                        } else if let Some((p, c)) = &violation {
                            (
                                tr.agents_pair_rule
                                    .replacen("{}", p, 1)
                                    .replacen("{}", c, 1),
                                egui::Color32::from_rgb(230, 192, 106),
                            )
                        } else if let Some(name) = &missing {
                            (
                                tr.agents_missing_key.replacen("{}", name, 1),
                                egui::Color32::from_rgb(230, 192, 106),
                            )
                        } else if let Some(name) =
                            crate::agents_db::unreviewed_primaries(&self.db).first()
                        {
                            (
                                tr.agents_unreviewed_warning.replacen("{}", name, 1),
                                egui::Color32::from_rgb(230, 192, 106),
                            )
                        } else {
                            let active = self.db.agents.iter().filter(|a| a.enabled).count();
                            (
                                tr.agents_valid
                                    .replacen("{}", &active.to_string(), 1)
                                    .replacen("{}", &self.db.agents.len().to_string(), 1),
                                egui::Color32::from_rgb(125, 214, 160),
                            )
                        };
                        let can_commit = violation.is_none();
                        ui.horizontal(|ui| {
                            // Message in a bounded LEFT region that WRAPS (never
                            // clipped by the buttons); buttons pinned bottom-right.
                            // Bottom-panel width is the window width, so reserving
                            // from available_width here does not self-inflate.
                            const BUTTON_AREA: f32 = 280.0;
                            let msg_w = (ui.available_width() - BUTTON_AREA).max(160.0);
                            ui.allocate_ui_with_layout(
                                egui::vec2(msg_w, 0.0),
                                egui::Layout::top_down(egui::Align::LEFT),
                                |ui| {
                                    ui.add(
                                        egui::Label::new(
                                            egui::RichText::new(&msg).color(msg_color),
                                        )
                                        .wrap(),
                                    );
                                },
                            );
                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    if ui
                                        .add_enabled(can_commit, egui::Button::new("OK"))
                                        .clicked()
                                    {
                                        if self.apply(llm) {
                                            action.applied = true;
                                            close = true;
                                        }
                                    }
                                    if ui
                                        .add_enabled(
                                            can_commit && self.dirty,
                                            egui::Button::new(tr.btn_apply_raw),
                                        )
                                        .clicked()
                                        && self.apply(llm)
                                    {
                                        action.applied = true;
                                    }
                                    if ui.button(tr.btn_cancel).clicked() {
                                        close = true;
                                    }
                                },
                            );
                        });
                        ui.add_space(4.0);
                    });

                egui::Panel::left(egui::Id::new("agents_rail_panel"))
                    .resizable(true)
                    // Wider default now the agent name is a large headline.
                    .default_size(360.0)
                    .min_size(260.0)
                    // Hard upper bound so the rail can never swallow the detail
                    // pane even if some content is unexpectedly wide.
                    .max_size(520.0)
                    .show(ui, |ui| self.rail_ui(ui, llm, tr));

                // The runtime table (spec 048 R9): every agent's model and
                // tuning, readable top to bottom, above the per-agent detail.
                // A FIXED height — never sized from the content — so a project
                // with thirty agents cannot ratchet the window taller each
                // frame (the egui self-inflation trap).
                egui::Panel::top(ui.id().with("agents_runtime_panel"))
                    .resizable(true)
                    .default_size(260.0)
                    .min_size(120.0)
                    .max_size(460.0)
                    .show_separator_line(true)
                    .show(ui, |ui| {
                        if self.runtime_table_ui(ui, llm, tr) {
                            self.dirty = true;
                            if let Err(e) = self.db.save_all() {
                                self.set_error(e);
                            }
                        }
                    });

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

    // ── Runtime table (spec 048 R9/R10/R14) ───────────────────────────────

    /// One row per agent — Grace, every specialist, every reviewer and the
    /// COBOL Proficiency Judge — carrying the four things that decide how that
    /// agent runs: its model, temperature, output-token cap and timeout.
    ///
    /// The provider combobox above scopes which models the Model column
    /// offers. It is a picker convenience and nothing more: it never rewrites
    /// an agent's stored provider (R11), so a table can hold agents on several
    /// providers at once and switching the scope to configure one leaves the
    /// others exactly as they were.
    ///
    /// Returns whether anything changed, so the caller persists once per frame
    /// rather than once per widget.
    fn runtime_table_ui(&mut self, ui: &mut egui::Ui, llm: &LlmConfig, tr: &Tr) -> bool {
        let mut changed = false;
        ui.add_space(6.0);

        // Provider scope + model search.
        ui.horizontal(|ui| {
            ui.label(tr.agents_tbl_provider_scope);
            let configured = llm.configured_providers();
            let current = if self.provider_scope.trim().is_empty() {
                "—".to_string()
            } else {
                crate::llm::Provider::from_id(&self.provider_scope)
                    .map(|p| p.label.to_string())
                    .unwrap_or_else(|| self.provider_scope.clone())
            };
            egui::ComboBox::from_id_salt("agents_provider_scope")
                .selected_text(current)
                .show_ui(ui, |ui| {
                    if configured.is_empty() {
                        ui.weak(tr.providers_unconfigured);
                    }
                    for id in &configured {
                        let label = crate::llm::Provider::from_id(id)
                            .map(|p| p.label.to_string())
                            .unwrap_or_else(|| id.clone());
                        ui.selectable_value(&mut self.provider_scope, id.clone(), label);
                    }
                });
            ui.add_space(12.0);
            ui.add(
                egui::TextEdit::singleline(&mut self.model_filter)
                    .hint_text(tr.agents_tbl_model_search)
                    .desired_width(200.0),
            );
        });
        ui.add_space(6.0);

        // The models on offer for the scoped provider, filtered.
        let scope = self.provider_scope.clone();
        let needle = self.model_filter.trim().to_ascii_lowercase();
        let offered: Vec<String> = llm
            .models_for(&scope)
            .iter()
            .filter(|m| needle.is_empty() || m.to_ascii_lowercase().contains(&needle))
            .cloned()
            .collect();

        // Who is standing on whose model, computed once for the whole table.
        let separation = self.db.model_separation(llm);

        egui::ScrollArea::vertical()
            .id_salt("agents_runtime_table")
            .auto_shrink([false, false])
            .show(ui, |ui| {
                egui::Grid::new("agents_runtime_grid")
                    .num_columns(5)
                    .spacing([12.0, 6.0])
                    .striped(true)
                    .show(ui, |ui| {
                        ui.label(egui::RichText::new(tr.agents_tbl_agent).strong());
                        ui.label(egui::RichText::new(tr.agents_tbl_model).strong());
                        ui.label(egui::RichText::new(tr.agents_tbl_temp).strong());
                        ui.label(egui::RichText::new(tr.agents_tbl_max_tokens).strong());
                        ui.label(egui::RichText::new(tr.agents_tbl_timeout).strong());
                        ui.end_row();

                        for i in 0..self.db.agents.len() {
                            let name = self.db.agents[i].name.clone();
                            let clash = separation
                                .clashes
                                .iter()
                                .find(|c| c.agent == name)
                                .map(|c| c.reserved_for);

                            ui.horizontal(|ui| {
                                ui.label(&name);
                                if let Some(reserved_for) = clash {
                                    ui.label(
                                        egui::RichText::new(
                                            tr.agents_tbl_clash.replacen("{}", reserved_for, 1),
                                        )
                                        .small()
                                        .color(ui.visuals().error_fg_color),
                                    );
                                }
                            });

                            // Model — (no model) plus the scoped provider's list.
                            let agent = &mut self.db.agents[i];
                            let shown = if agent.no_model {
                                tr.agents_tbl_no_model.to_string()
                            } else if agent.model.trim().is_empty() {
                                tr.agents_tbl_no_model.to_string()
                            } else {
                                agent.model.clone()
                            };
                            egui::ComboBox::from_id_salt(("agent_model", i))
                                .selected_text(shown)
                                .width(240.0)
                                .show_ui(ui, |ui| {
                                    if ui
                                        .selectable_label(agent.no_model, tr.agents_tbl_no_model)
                                        .clicked()
                                    {
                                        agent.no_model = true;
                                        agent.model.clear();
                                        agent.model_profile = None;
                                        changed = true;
                                    }
                                    for model in &offered {
                                        let selected =
                                            !agent.no_model && &agent.model == model;
                                        if ui.selectable_label(selected, model).clicked()
                                            && !selected
                                        {
                                            // The agent takes the model AND the
                                            // provider it was picked from (R12);
                                            // other rows keep theirs (R11).
                                            agent.no_model = false;
                                            agent.model_profile = None;
                                            agent.provider = scope.clone();
                                            agent.endpoint = llm.provider_endpoint(&scope);
                                            agent.model = model.clone();
                                            changed = true;
                                        }
                                    }
                                });

                            // Temperature / output tokens / timeout. The ranges
                            // ARE the validation (R14): a DragValue cannot leave
                            // its range, so a rejected value never replaces the
                            // one already there.
                            if ui
                                .add(
                                    egui::DragValue::new(&mut agent.temperature)
                                        .range(0.0..=2.0)
                                        .speed(0.01)
                                        .fixed_decimals(2),
                                )
                                .on_hover_text(tr.agents_val_temp_range)
                                .changed()
                            {
                                changed = true;
                            }
                            if ui
                                .add(
                                    egui::DragValue::new(&mut agent.max_tokens)
                                        .range(1..=200_000)
                                        .speed(64.0),
                                )
                                .on_hover_text(tr.agents_val_tokens_range)
                                .changed()
                            {
                                changed = true;
                            }
                            if ui
                                .add(
                                    egui::DragValue::new(&mut agent.timeout_secs)
                                        .range(1..=3600)
                                        .suffix(" s"),
                                )
                                .on_hover_text(tr.agents_val_timeout_range)
                                .changed()
                            {
                                changed = true;
                            }
                            ui.end_row();
                        }
                    });
            });
        changed
    }

    // ── Left rail ─────────────────────────────────────────────────────────
    fn rail_ui(&mut self, ui: &mut egui::Ui, llm: &mut LlmConfig, tr: &Tr) {
        ui.add_space(6.0);
        ui.horizontal(|ui| {
            // FIXED widths only — never size a child from `available_width` in
            // a resizable side panel, or the row's min-width chases the panel
            // width and ratchets it wider every frame (egui self-inflation;
            // see the egui-resize-autogrow memory / egui-paint-regressions).
            ui.add(
                egui::TextEdit::singleline(&mut self.filter)
                    .hint_text(tr.agents_filter)
                    .desired_width(190.0),
            );
            if SHOW_NEW_AGENT_CONTROL && ui.button(format!("＋ {}", tr.agents_new)).clicked() {
                self.new_name = Some(String::new());
                self.new_kind = crate::agents_db::AgentKind::Specialist;
                self.error = None;
            }
        });
        // Inline "new agent" row: the name is asked once and is immutable.
        if SHOW_NEW_AGENT_CONTROL && self.new_name.is_some() {
            use crate::agents_db::AgentKind;
            ui.add_space(4.0);
            let mut create = false;
            let mut cancel = false;
            ui.horizontal(|ui| {
                let name = self.new_name.as_mut().unwrap();
                let resp = ui.add(
                    egui::TextEdit::singleline(name)
                        .hint_text(tr.agents_new_name_hint)
                        .desired_width(160.0), // fixed — no available_width feedback
                );
                create = (resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)))
                    || ui.button("✔").clicked();
                cancel = ui.button("✖").clicked();
            });
            // Kind: create the agent directly as a Specialist or a Pedantic
            // reviewer (a Pedantic then appears in specialists' companion picker).
            ui.horizontal(|ui| {
                ui.label(tr.agents_kind);
                egui::ComboBox::from_id_salt("new_agent_kind")
                    .selected_text(if self.new_kind == AgentKind::Pedantic {
                        tr.agents_kind_pedantic
                    } else {
                        tr.agents_kind_specialist
                    })
                    .show_ui(ui, |ui| {
                        ui.selectable_value(
                            &mut self.new_kind,
                            AgentKind::Specialist,
                            tr.agents_kind_specialist,
                        );
                        ui.selectable_value(
                            &mut self.new_kind,
                            AgentKind::Pedantic,
                            tr.agents_kind_pedantic,
                        );
                    });
            });
            if create {
                let name = self.new_name.take().unwrap_or_default();
                let kind = self.new_kind;
                self.stash_selected(llm);
                match self.db.create_kinded(&name, "", kind, "") {
                    Ok(id) => {
                        self.dirty = true;
                        self.error = None;
                        if let Some(i) = self.db.agents.iter().position(|a| a.id == id) {
                            self.sel = i;
                            self.load_selected(llm);
                        }
                    }
                    Err(e) => {
                        self.set_error(e);
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
                let entries: Vec<(
                    usize,
                    String,
                    String,
                    String,
                    bool,
                    bool,
                    crate::agents_db::AgentKind,
                )> = self
                    .db
                    .agents
                    .iter()
                    .enumerate()
                    .filter(|(_, a)| filter.is_empty() || a.name.to_lowercase().contains(&filter))
                    .map(|(i, a)| {
                        // Show the resolved model profile (spec 031), falling back
                        // to the dormant embedded fields for un-migrated agents.
                        // An explicit "(no model)" shows as none, whatever those
                        // dormant fields still hold.
                        let (model, provider) = if a.no_model {
                            (String::new(), String::new())
                        } else {
                            a.model_profile
                                .as_ref()
                                .and_then(|id| llm.profile(id))
                                .map(|p| (p.model.clone(), p.provider.clone()))
                                .unwrap_or_else(|| (a.model.clone(), a.provider.clone()))
                        };
                        (
                            i,
                            a.name.clone(),
                            model,
                            provider,
                            a.enabled,
                            // Linked companion = some primary points at it.
                            self.db.is_companion(&a.id),
                            a.kind,
                        )
                    })
                    .collect();
                for (i, name, model, provider, enabled, is_linked_companion, kind) in entries {
                    let selected = i == self.sel;
                    // A pedantic reviewer not attached to any primary — flagged.
                    // The COBOL Proficiency Judge is the exception: it reviews
                    // the proficiency test, not another agent's output, so it
                    // has no primary BY DESIGN and being unpaired is correct.
                    // Flagging it painted a permanently red row nobody could
                    // ever clear.
                    let orphan_pedantic = kind == crate::agents_db::AgentKind::Pedantic
                        && !is_linked_companion
                        && !crate::agents_db::is_proficiency_judge(&name);
                    let dot = if enabled { "●" } else { "○" };
                    let badge = match kind {
                        crate::agents_db::AgentKind::Orchestrator => "👑 ",
                        crate::agents_db::AgentKind::Pedantic => "🔍 ",
                        crate::agents_db::AgentKind::Specialist => "",
                    };
                    // Truncate so a long model id can't widen the rail.
                    let ell = |s: &str, n: usize| -> String {
                        if s.chars().count() > n {
                            format!("{}…", s.chars().take(n - 1).collect::<String>())
                        } else {
                            s.to_string()
                        }
                    };
                    // 50% of the previous 37.5/25 sizes.
                    const NAME_PT: f32 = 18.75;
                    const SUB_PT: f32 = 12.5;
                    let name_line = format!("{dot} {badge}{}", ell(&name, 24));
                    let sub_line = format!(
                        "    {} · {}",
                        if model.is_empty() {
                            "—".into()
                        } else {
                            ell(&model, 22)
                        },
                        if provider.is_empty() {
                            "—".into()
                        } else {
                            ell(&provider, 16)
                        },
                    );
                    // Text colour: on the bright selected fill, pick dark or light
                    // for contrast; otherwise the theme colours. Inactive buttons
                    // are dimmed to a constant level (baked into the colours) — no
                    // hover response.
                    let sel_fill = ui.visuals().selection.bg_fill;
                    let (name_color, sub_color) = if selected {
                        let lum = 0.299 * sel_fill.r() as f32
                            + 0.587 * sel_fill.g() as f32
                            + 0.114 * sel_fill.b() as f32;
                        if lum > 140.0 {
                            (egui::Color32::from_gray(20), egui::Color32::from_gray(70))
                        } else {
                            (egui::Color32::WHITE, egui::Color32::from_gray(210))
                        }
                    } else if orphan_pedantic {
                        // Unassociated pedantic reviewer — red foreground.
                        (
                            egui::Color32::from_rgb(224, 120, 120),
                            egui::Color32::from_rgb(176, 96, 96),
                        )
                    } else {
                        (
                            ui.visuals().text_color().gamma_multiply(0.55),
                            ui.visuals().weak_text_color().gamma_multiply(0.55),
                        )
                    };
                    let mut job = egui::text::LayoutJob::default();
                    // Never wrap → the row height is a constant two lines, so it
                    // does NOT change as the pane gets narrower/wider. Overflow is
                    // clipped to the button rect below.
                    job.wrap.max_width = f32::INFINITY;
                    job.append(
                        &name_line,
                        0.0,
                        egui::TextFormat {
                            font_id: egui::FontId::proportional(NAME_PT),
                            color: name_color,
                            ..Default::default()
                        },
                    );
                    job.append(
                        &format!("\n{sub_line}"),
                        0.0,
                        egui::TextFormat {
                            font_id: egui::FontId::proportional(SUB_PT),
                            color: sub_color,
                            ..Default::default()
                        },
                    );
                    let galley = ui.painter().layout_job(job);
                    // FULL width via allocate_exact_size: it takes exactly the
                    // available width, so it fills the rail yet can never demand
                    // MORE than is available — the pane is never pushed wider.
                    const HPAD: f32 = 10.0;
                    const VPAD: f32 = 6.0;
                    // Linked companions are indented under their primary.
                    let indent = if is_linked_companion { 22.0 } else { 0.0 };
                    let full_w = ui.available_width();
                    let row_h = galley.size().y + 2.0 * VPAD;
                    let (outer, resp) =
                        ui.allocate_exact_size(egui::vec2(full_w, row_h), egui::Sense::click());
                    // The button box is inset by the indent; the row still spans
                    // the full width so vertical layout advances correctly.
                    let rect =
                        egui::Rect::from_min_max(outer.min + egui::vec2(indent, 0.0), outer.max);
                    if ui.is_rect_visible(rect) {
                        let p = ui.painter();
                        if selected {
                            p.rect_filled(rect, egui::CornerRadius::same(8), sel_fill);
                        } else {
                            let stroke_color = if orphan_pedantic {
                                egui::Color32::from_rgb(200, 96, 96)
                            } else {
                                ui.visuals().weak_text_color().gamma_multiply(0.5)
                            };
                            p.rect_stroke(
                                rect,
                                egui::CornerRadius::same(8),
                                egui::Stroke::new(1.0, stroke_color),
                                egui::StrokeKind::Inside,
                            );
                        }
                        // Clip the text to the button so a long line never spills
                        // past the rail edge.
                        ui.painter().with_clip_rect(rect).galley(
                            rect.min + egui::vec2(HPAD, VPAD),
                            galley,
                            egui::Color32::WHITE,
                        );
                    }
                    if resp.clicked() {
                        self.select(i, llm);
                    }
                    ui.add_space(4.0);
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
        let mut do_proficiency = false;
        let mut relationship_changed = false;

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
                        // Name + its hint on separate rows so the label stays
                        // aligned with the value (egui Grid centers a cell whose
                        // content is taller — a value+subtext stack).
                        ui.label(tr.agents_name);
                        ui.monospace(&agent.name);
                        ui.end_row();
                        ui.label("");
                        ui.weak(tr.agents_name_hint);
                        ui.end_row();
                        ui.label(tr.agents_kind);
                        {
                            use crate::agents_db::AgentKind;
                            let a = &mut self.db.agents[sel];
                            if a.kind == AgentKind::Orchestrator {
                                // Grace's kind is fixed.
                                ui.label(tr.agents_kind_orchestrator);
                            } else {
                                // Specialist ↔ Pedantic is editable, so a user
                                // can designate a pedantic reviewer that then
                                // appears in the companion picker.
                                let cur = if a.kind == AgentKind::Pedantic {
                                    tr.agents_kind_pedantic
                                } else {
                                    tr.agents_kind_specialist
                                };
                                egui::ComboBox::from_id_salt("ag_kind")
                                    .selected_text(cur)
                                    .show_ui(ui, |ui| {
                                        changed |= ui
                                            .selectable_value(
                                                &mut a.kind,
                                                AgentKind::Specialist,
                                                tr.agents_kind_specialist,
                                            )
                                            .changed();
                                        changed |= ui
                                            .selectable_value(
                                                &mut a.kind,
                                                AgentKind::Pedantic,
                                                tr.agents_kind_pedantic,
                                            )
                                            .changed();
                                    });
                            }
                        }
                        ui.end_row();
                        ui.label(tr.agents_specialization);
                        {
                            let a = &mut self.db.agents[sel];
                            changed |= ui
                                .add(egui::TextEdit::singleline(&mut a.specialization).desired_width(f32::INFINITY))
                                .changed();
                        }
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
                    if agent.kind == crate::agents_db::AgentKind::Orchestrator {
                        ui.weak(tr.agents_grace_protected);
                    }
                    if SHOW_DELETE_AGENT_CONTROL {
                        if crate::agents_db::is_fixed_agent_name(&agent.name) {
                            ui.add_enabled(
                                false,
                                egui::Button::new(format!("🗑 {}", tr.agents_delete)),
                            );
                        } else if !self.confirm_delete {
                            if ui.button(format!("🗑 {}", tr.agents_delete)).clicked() {
                                self.confirm_delete = true;
                            }
                        } else if ui
                            .button(
                                egui::RichText::new(format!(
                                    "🗑 {}",
                                    tr.agents_delete_confirm
                                ))
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
                    }
                });
            ui.separator();

            // Runtime configuration ------------------------------------------
            egui::CollapsingHeader::new(egui::RichText::new(tr.agents_sec_runtime).strong())
                .default_open(true)
                .show(ui, |ui| {
                    egui::Grid::new("ag_runtime").num_columns(2).spacing([14.0, 7.0]).show(ui, |ui| {
                        // Model profile (spec 031): pick a reusable profile defined
                        // in the Models Manager, instead of re-entering a connection.
                        ui.label(tr.agents_model_profile);
                        {
                            let a = &mut self.db.agents[sel];
                            let current_name = a
                                .model_profile
                                .as_ref()
                                .and_then(|id| llm.profile(id))
                                .map(|p| p.name.clone());
                            let mut pick: Option<Option<String>> = None;
                            ui.horizontal(|ui| {
                                egui::ComboBox::from_id_salt("ag_model_profile")
                                    .selected_text(
                                        current_name
                                            .clone()
                                            .unwrap_or_else(|| tr.agents_model_profile_none.to_string()),
                                    )
                                    .width(240.0)
                                    .show_ui(ui, |ui| {
                                        if ui
                                            .selectable_label(a.model_profile.is_none(), tr.agents_model_profile_none)
                                            .clicked()
                                        {
                                            pick = Some(None);
                                        }
                                        for p in &llm.model_profiles {
                                            let label =
                                                if p.name.is_empty() { p.model.clone() } else { p.name.clone() };
                                            if ui
                                                .selectable_label(
                                                    a.model_profile.as_deref() == Some(p.id.as_str()),
                                                    label,
                                                )
                                                .clicked()
                                            {
                                                pick = Some(Some(p.id.clone()));
                                            }
                                        }
                                    });
                            });
                            if let Some(p) = pick {
                                // Picking "(none)" is an explicit choice, not an
                                // unconfigured agent: record it so the built-in
                                // seeding does not hand a model back on the next
                                // project open.
                                a.no_model = p.is_none();
                                a.model_profile = p;
                                changed = true;
                            }
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
                    // The user can resize the editor vertically from four to
                    // twenty text rows. Long prompts scroll inside the editor
                    // and cannot inflate the surrounding detail pane.
                    let r = prompt_editor(
                        ui,
                        ui.make_persistent_id(("agents_prompt_editor", agent.id.as_str())),
                        &mut self.prompt_buf,
                    );
                    changed |= r.response.changed();
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
            // The Judge gets neither picker: it is paired with nothing (the
            // Test proficiency button invokes it directly), so offering to
            // attach it to a primary only invites a misconfiguration.
            if crate::agents_db::is_proficiency_judge(&agent.name) {
                egui::CollapsingHeader::new(
                    egui::RichText::new(tr.agents_sec_companion_for).strong(),
                )
                .default_open(true)
                .show(ui, |ui| {
                    ui.weak(tr.agents_judge_unpaired_hint);
                });
            } else if agent.kind == AgentKind::Pedantic {
                egui::CollapsingHeader::new(
                    egui::RichText::new(tr.agents_sec_companion_for).strong(),
                )
                .default_open(true)
                .show(ui, |ui| {
                    let current_owner = self
                        .db
                        .companion_owner(&agent.id)
                        .map(|owner| (owner.id.clone(), owner.name.clone()));
                    let mut pick: Option<Option<String>> = None;
                    egui::ComboBox::from_id_salt("ag_companion_for")
                        .selected_text(
                            current_owner
                                .as_ref()
                                .map(|(_, name)| name.clone())
                                .unwrap_or_else(|| tr.agents_companion_for_none.to_string()),
                        )
                        .width(320.0)
                        .show_ui(ui, |ui| {
                            if ui
                                .selectable_label(
                                    current_owner.is_none(),
                                    tr.agents_companion_for_none,
                                )
                                .clicked()
                            {
                                pick = Some(None);
                            }
                            let owners: Vec<(String, String)> = self
                                .db
                                .agents
                                .iter()
                                .filter(|candidate| {
                                    matches!(
                                        candidate.kind,
                                        AgentKind::Orchestrator | AgentKind::Specialist
                                    )
                                })
                                .map(|candidate| {
                                    (candidate.id.clone(), candidate.name.clone())
                                })
                                .collect();
                            for (id, name) in owners {
                                if ui
                                    .selectable_label(
                                        current_owner.as_ref().map(|(owner_id, _)| owner_id)
                                            == Some(&id),
                                        name,
                                    )
                                    .clicked()
                                {
                                    pick = Some(Some(id));
                                }
                            }
                        });
                    if let Some(selection) = pick {
                        let result = match selection {
                            Some(owner_id) => {
                                self.db.set_companion(&owner_id, Some(agent.id.as_str()))
                            }
                            None => current_owner
                                .as_ref()
                                .map(|(owner_id, _)| self.db.set_companion(owner_id, None))
                                .unwrap_or(Ok(false)),
                        };
                        match result {
                            Ok(did_change) => {
                                changed |= did_change;
                                relationship_changed |= did_change;
                            }
                            Err(error) => self.set_error(error),
                        }
                    }
                    ui.weak(tr.agents_companion_for_hint);
                });
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
                        ui.horizontal(|ui| {
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
                                        .filter(|x| {
                                            x.id != agent.id
                                                && x.kind == AgentKind::Pedantic
                                                // The Judge reviews the
                                                // proficiency test, never an
                                                // agent — it is not on offer
                                                // as anybody's companion.
                                                && !crate::agents_db::is_proficiency_judge(&x.name)
                                        })
                                        .map(|x| {
                                            // Show the companion's resolved model (spec 031),
                                            // falling back to its dormant embedded model.
                                            let model = x
                                                .model_profile
                                                .as_ref()
                                                .and_then(|id| llm.profile(id))
                                                .map(|p| p.model.clone())
                                                .unwrap_or_else(|| x.model.clone());
                                            (x.id.clone(), format!("{} ({})", x.name, model))
                                        })
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
                            // The COBOL proficiency check moved to the Models
                            // Manager (1.55.3). It scores what a MODEL writes, so
                            // it belongs to the profile, once, where two models can
                            // be compared — not to whichever agents happen to
                            // reference that profile, where the same model was
                            // benchmarked repeatedly and Grace, being an
                            // Orchestrator rather than a Specialist, could not be
                            // benchmarked at all.
                        });
                        if let Some(p) = pick {
                            match self.db.set_companion(&agent.id, p.as_deref()) {
                                Ok(did_change) => {
                                    changed |= did_change;
                                    relationship_changed |= did_change;
                                }
                                Err(error) => self.set_error(error),
                            }
                        }
                        ui.weak(tr.agents_companion_hint);
                    });
            }
        });

        if relationship_changed {
            self.db.sort_rail();
            if let Some(index) = self
                .db
                .agents
                .iter()
                .position(|candidate| candidate.id == agent.id)
            {
                self.sel = index;
            }
        }

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
        let r = ui.add(
            egui::TextEdit::singleline(&mut buf)
                .hint_text("＋")
                .desired_width(110.0),
        );
        if r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) && !buf.trim().is_empty()
        {
            items.push(buf.trim().to_string());
            buf.clear();
            *changed = true;
        }
        ui.data_mut(|d| d.insert_temp(id, buf));
    });
}

#[cfg(test)]
mod resize_tests {
    use super::*;
    use crate::agents_db::{AgentKind, AgentsDb};

    #[test]
    fn agent_lifecycle_controls_are_hidden() {
        assert!(!SHOW_NEW_AGENT_CONTROL);
        assert!(!SHOW_DELETE_AGENT_CONTROL);
    }

    #[test]
    fn long_prompt_editor_never_exceeds_twenty_rows() {
        let ctx = egui::Context::default();
        let mut prompt = (1..=80)
            .map(|line| format!("instruction line {line}"))
            .collect::<Vec<_>>()
            .join("\n");
        let mut measured_height = 0.0;
        let mut max_height = 0.0;
        let mut content_height = 0.0;

        for frame in 0..4 {
            let input = egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(900.0, 700.0),
                )),
                time: Some(frame as f64 / 60.0),
                ..Default::default()
            };
            ctx.run_ui(input, |ui| {
                max_height = prompt_editor_height(ui, PROMPT_EDITOR_MAX_ROWS);
                let id = ui.make_persistent_id("prompt_height_test");
                let output = prompt_editor(ui, id, &mut prompt);
                measured_height = output.viewport_rect.height();
                content_height = output.content_height;
            })
            .textures_delta
            .clear();
        }

        assert!(
            measured_height <= max_height + 0.5,
            "long content expanded prompt editor to {measured_height}px; 20 rows is {max_height}px"
        );
        assert!(
            content_height > measured_height,
            "the long prompt should scroll inside the bounded editor"
        );
    }

    /// Regression guard (egui self-inflation): the agent-list rail must hold a
    /// stable width across many frames. It used to size its filter/name fields
    /// from `available_width`, so the row's min-width chased the panel width
    /// and ratcheted it wider every frame until the detail pane vanished.
    #[test]
    fn agent_rail_width_is_stable_across_frames() {
        let proj =
            std::env::temp_dir().join(format!("prc_railtest_{}", crate::agents_db::new_uuid()));
        std::fs::create_dir_all(&proj).unwrap();
        // A few agents with long-ish model ids (worst case for width).
        let mut db = AgentsDb::load(&proj);
        db.ensure_grace();
        let a = db
            .create_kinded(
                "Form Designer Agent",
                "p",
                AgentKind::Specialist,
                "form-design",
            )
            .unwrap();
        if let Some(x) = db.agents.iter_mut().find(|x| x.id == a) {
            x.model = "some-vendor/a-fairly-long-model-identifier:latest".into();
            x.provider = "ollama_cloud".into();
        }
        db.save_all().unwrap();

        let mut llm = crate::llm::LlmConfig::load_defaults_for_test();
        let mut modal = AgentsModal::open_for(&proj, &mut llm);
        let tr = crate::i18n::Language::English.tr();

        let ctx = egui::Context::default();
        let rail_id = egui::Id::new("agents_rail_panel");
        let mut widths: Vec<f32> = Vec::new();
        for _ in 0..120 {
            let mut input = egui::RawInput::default();
            input.screen_rect = Some(egui::Rect::from_min_size(
                egui::pos2(0.0, 0.0),
                egui::vec2(1400.0, 800.0),
            ));
            ctx.run_ui(input, |root_ui| {
                let c = root_ui.ctx().clone();
                modal.show(&c, &mut llm, &tr);
            })
            .textures_delta
            .clear();
            if let Some(state) = egui::containers::panel::PanelState::load(&ctx, rail_id) {
                widths.push(state.outer_rect.width());
            }
        }
        assert!(widths.len() >= 100, "rail panel never materialised");
        let settled = widths[5];
        for (i, w) in widths.iter().enumerate().skip(5) {
            assert!(
                (w - settled).abs() < 0.5,
                "rail width drifted at frame {i}: {settled} -> {w} (self-inflation)"
            );
            // Bounded by the max_size cap — the ratchet cannot run away even
            // with the large headline font and a long model id.
            assert!(*w <= 520.5, "rail exceeded its max_size cap: {w}");
        }
        let _ = std::fs::remove_dir_all(proj);
        println!(
            "agent rail stable at {settled:.0}px across {} frames",
            widths.len()
        );
    }
}
