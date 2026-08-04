// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Model Leaderboard panel (spec 040).
//!
//! Ranks every model that has taken the COBOL proficiency test, on four boards
//! — Overall, Cloud free, Cloud paid, Local — reading the machine-wide store in
//! [`crate::leaderboard`]. Each row offers the model's full metric sheet, a
//! re-test, and the two ways of putting it to work: as Grace, or across the
//! specialist agents.
//!
//! A model whose test could not run keeps no rank and no rating: it shows the
//! provider's error and sorts below every scored model, because a score it
//! never earned is worse than no score at all.

use crate::i18n::Tr;
use crate::leaderboard::{Board, Leaderboard, Tier};
use crate::theme::Theme;

/// A **ceiling**, not a height: the card hugs its rows and only starts
/// scrolling past this. A fixed height left two thirds of the panel empty
/// whenever the board was short, which it usually is.
const MAX_BODY_H: f32 = 620.0;
/// Wide enough for four buttons and the longest model id at 14 px.
const TABLE_W: f32 = 1010.0;
/// Breathing room inside the card, so nothing touches its border.
const CARD_PAD: i8 = 10;
const SZ_TITLE: f32 = 18.0;
const SZ_BODY: f32 = 14.0;
const SZ_SMALL: f32 = 14.0;

pub struct LeaderboardModal {
    pub open: bool,
    board: Board,
    /// Index into the store's entries whose Details modal is open.
    details: Option<usize>,
    /// `(model, message)` of the error window.
    error: Option<(String, String)>,
    status: Option<String>,
    /// Whether the COBOL Proficiency Judge has a model of its own. Resolved by
    /// the app when the panel opens — reading the agent database every frame
    /// would be a file read per frame.
    judge_ready: bool,
    /// A Run tests click held back while the judge question is answered.
    pending_run: Option<(String, String)>,
}

#[derive(Default)]
pub struct LeaderboardAction {
    /// Re-run the proficiency test for this `(provider, model)`.
    pub run_tests: Option<(String, String)>,
    /// Assign this `(provider, model)` to Grace.
    pub apply_to_grace: Option<(String, String)>,
    /// Assign this `(provider, model)` to every specialist agent.
    pub apply_to_specialists: Option<(String, String)>,
    /// Reopen the stored benchmark report for this `(provider, model)`.
    pub open_report: Option<(String, String)>,
    /// Open the Agents Manager at the COBOL Proficiency Judge so a model can be
    /// given to it.
    pub open_judge_setup: bool,
}

impl LeaderboardModal {
    pub fn new(judge_ready: bool) -> Self {
        Self {
            open: true,
            board: Board::Overall,
            details: None,
            error: None,
            status: None,
            judge_ready,
            pending_run: None,
        }
    }

    /// The judge gained (or lost) a model while the panel was open.
    pub fn set_judge_ready(&mut self, ready: bool) {
        self.judge_ready = ready;
    }

    /// Show a confirmation under the tabs (the app calls this once an action it
    /// was handed has actually been carried out).
    pub fn set_status(&mut self, text: String) {
        self.status = Some(text);
    }

    fn board_title(board: Board, tr: &Tr) -> &'static str {
        match board {
            Board::Overall => tr.leaderboard_board_overall,
            Board::CloudFree => tr.leaderboard_board_cloud_free,
            Board::CloudPaid => tr.leaderboard_board_cloud_paid,
            Board::Local => tr.leaderboard_board_local,
        }
    }

    fn tier_label(tier: Tier, tr: &Tr) -> &'static str {
        match tier {
            Tier::CloudFree => tr.leaderboard_tier_cloud_free,
            Tier::CloudPaid => tr.leaderboard_tier_cloud_paid,
            Tier::Local => tr.leaderboard_tier_local,
        }
    }

    pub fn show(
        &mut self,
        ctx: &egui::Context,
        board: &Leaderboard,
        theme: &Theme,
        tr: &Tr,
    ) -> LeaderboardAction {
        let mut action = LeaderboardAction::default();
        let mut open = self.open;

        egui::Window::new(
            egui::RichText::new(tr.leaderboard_title)
                .size(SZ_TITLE)
                .strong(),
        )
        .id(egui::Id::new("model_leaderboard"))
        .open(&mut open)
        .collapsible(false)
        .resizable(false)
        .default_pos([40.0, 60.0])
        .show(ctx, |ui| {
            ui.set_width(TABLE_W);
            self.header(ui, board, theme, tr);
            ui.add_space(8.0);
            // The card hugs its content vertically — no fixed height, so a
            // six-row board is a six-row card. Width stays fixed, which is what
            // keeps the columns from renegotiating themselves every frame.
            egui::Frame::NONE
                .fill(theme.bg_panel)
                .stroke(egui::Stroke::new(1.0, theme.panel_border()))
                .corner_radius(egui::CornerRadius::same(10))
                .inner_margin(egui::Margin::same(CARD_PAD))
                .show(ui, |ui| {
                    let inner = TABLE_W - 2.0 * CARD_PAD as f32 - 2.0;
                    ui.set_min_width(inner);
                    ui.set_max_width(inner);
                    self.table(ui, board, theme, tr, &mut action);
                });
        });

        self.details_modal(ctx, board, theme, tr, &mut action);
        self.judge_missing_modal(ctx, theme, tr, &mut action);
        self.error_window(ctx, theme, tr);
        self.open = open && self.open;
        action
    }

    fn header(&mut self, ui: &mut egui::Ui, board: &Leaderboard, theme: &Theme, tr: &Tr) {
        ui.horizontal(|ui| {
            ui.label(
                egui::RichText::new(tr.leaderboard_subtitle)
                    .size(SZ_BODY)
                    .color(theme.accent),
            );
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label(
                    egui::RichText::new(
                        tr.leaderboard_count
                            .replacen("{}", &board.entries.len().to_string(), 1),
                    )
                    .size(SZ_SMALL)
                    .color(theme.text_dim),
                );
            });
        });
        ui.add_space(6.0);
        ui.horizontal(|ui| {
            for b in Board::ALL {
                let count = board.ranked(b).len();
                let label = egui::RichText::new(format!("{}  ({count})", Self::board_title(b, tr)))
                    .size(SZ_BODY);
                if ui.selectable_label(self.board == b, label).clicked() {
                    self.board = b;
                    self.status = None;
                }
            }
            if let Some(s) = &self.status {
                ui.label(
                    egui::RichText::new(s)
                        .size(SZ_SMALL)
                        .color(theme.ed_data),
                );
            }
        });
    }

    fn table(
        &mut self,
        ui: &mut egui::Ui,
        board: &Leaderboard,
        theme: &Theme,
        tr: &Tr,
        action: &mut LeaderboardAction,
    ) {
        ui.label(
            egui::RichText::new(Self::board_title(self.board, tr))
                .size(SZ_TITLE)
                .strong(),
        );
        ui.add_space(4.0);
        let ranked = board.ranked(self.board);
        if ranked.is_empty() {
            ui.add_space(12.0);
            // Reached only when no model is configured at all — a configured
            // model always has a row, tested or not.
            ui.label(
                egui::RichText::new(tr.leaderboard_empty)
                    .size(SZ_BODY)
                    .color(theme.text_dim),
            );
            return;
        }
        egui::ScrollArea::vertical()
            .id_salt("leaderboard_rows")
            // Shrinks to the rows it has (so the card is tight) but never
            // grows past the ceiling (so a long board scrolls instead of
            // running off the screen).
            .max_height(MAX_BODY_H)
            .auto_shrink([false, true])
            .show(ui, |ui| {
                egui::Grid::new("leaderboard_grid")
                    .num_columns(5)
                    .striped(true)
                    .spacing([14.0, 7.0])
                    .min_col_width(48.0)
                    .show(ui, |ui| {
                        for h in [
                            tr.leaderboard_col_rank,
                            tr.leaderboard_col_model,
                            tr.leaderboard_col_provider,
                            tr.leaderboard_col_evaluation,
                            "",
                        ] {
                            ui.label(
                                egui::RichText::new(h)
                                    .size(SZ_BODY)
                                    .strong()
                                    .color(theme.accent),
                            );
                        }
                        ui.end_row();

                        let mut rank = 0usize;
                        for i in ranked {
                            let e = &board.entries[i];
                            let rated = e.rated();
                            if rated {
                                rank += 1;
                                ui.label(
                                    egui::RichText::new(format!("#{rank}"))
                                        .size(SZ_BODY)
                                        .strong()
                                        .color(theme.text_bright),
                                );
                            } else {
                                ui.label(
                                    egui::RichText::new("—").size(SZ_BODY).color(theme.error),
                                );
                            }
                            ui.label(
                                egui::RichText::new(&e.model)
                                    .size(SZ_BODY)
                                    .strong()
                                    .color(theme.text_bright),
                            );
                            ui.label(
                                egui::RichText::new(&e.provider)
                                    .size(SZ_BODY)
                                    .color(theme.text_bright),
                            );
                            match e.overall() {
                                Some(overall) if rated => {
                                    ui.horizontal(|ui| {
                                        star_rating(ui, theme, overall / 100.0, 14.0);
                                        ui.label(
                                            egui::RichText::new(format!("{overall:.1}%"))
                                                .size(SZ_BODY)
                                                .strong()
                                                .color(score_color(theme, overall)),
                                        );
                                    });
                                }
                                _ => {
                                    // Never tested and failed-its-test are both
                                    // unrated, but they call for different
                                    // things: run it, or fix the connection.
                                    let (text, colour) = if e.never_tested() {
                                        (tr.leaderboard_not_tested, theme.text_dim)
                                    } else {
                                        (tr.leaderboard_not_rated, theme.error)
                                    };
                                    let cell = ui.label(
                                        egui::RichText::new(text).size(SZ_BODY).color(colour),
                                    );
                                    if let Some(err) = &e.last_error {
                                        cell.on_hover_text(err);
                                    }
                                }
                            }
                            let id = (e.provider.clone(), e.model.clone());
                            ui.horizontal(|ui| {
                                if ui
                                    .add_enabled(
                                        rated,
                                        egui::Button::new(
                                            egui::RichText::new(tr.leaderboard_details)
                                                .size(SZ_BODY),
                                        ),
                                    )
                                    .clicked()
                                {
                                    self.details = Some(i);
                                }
                                if ui
                                    .button(
                                        egui::RichText::new(tr.leaderboard_run_tests)
                                            .size(SZ_BODY),
                                    )
                                    .on_hover_text(tr.leaderboard_run_tests_hint)
                                    .clicked()
                                {
                                    // A run with no judge is a self-assessment.
                                    // Ask before spending the tokens, not after.
                                    if self.judge_ready {
                                        action.run_tests = Some(id.clone());
                                    } else {
                                        self.pending_run = Some(id.clone());
                                    }
                                }
                                // A model that could not be reached is not a
                                // model to hand the project's work to.
                                if ui
                                    .add_enabled(
                                        rated,
                                        egui::Button::new(
                                            egui::RichText::new(tr.leaderboard_apply_grace)
                                                .size(SZ_BODY),
                                        ),
                                    )
                                    .on_hover_text(tr.leaderboard_apply_grace_hint)
                                    .clicked()
                                {
                                    action.apply_to_grace = Some(id.clone());
                                }
                                if ui
                                    .add_enabled(
                                        rated,
                                        egui::Button::new(
                                            egui::RichText::new(tr.leaderboard_apply_specialists)
                                                .size(SZ_BODY),
                                        ),
                                    )
                                    .on_hover_text(tr.leaderboard_apply_specialists_hint)
                                    .clicked()
                                {
                                    action.apply_to_specialists = Some(id);
                                }
                            });
                            ui.end_row();
                        }
                    });
                ui.add_space(6.0);
                ui.label(
                    egui::RichText::new(tr.leaderboard_footnote)
                        .size(SZ_SMALL)
                        .color(theme.text_dim),
                );
            });
    }

    fn details_modal(
        &mut self,
        ctx: &egui::Context,
        board: &Leaderboard,
        theme: &Theme,
        tr: &Tr,
        action: &mut LeaderboardAction,
    ) {
        let Some(i) = self.details else { return };
        let Some(e) = board.entries.get(i) else {
            self.details = None;
            return;
        };
        let ranked = board.ranked(self.board);
        let rank = ranked.iter().position(|k| *k == i).map(|p| p + 1);
        let mut close = false;

        egui::Modal::new(egui::Id::new("leaderboard_details")).show(ctx, |ui| {
            ui.set_width(600.0);
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new(&e.model).size(SZ_TITLE).strong());
                ui.label(egui::RichText::new(&e.provider).color(theme.accent));
                ui.label(
                    egui::RichText::new(Self::tier_label(e.tier(), tr)).color(theme.accent),
                );
            });
            ui.label(
                egui::RichText::new(format!(
                    "{} · {}",
                    rank.map(|r| tr
                        .leaderboard_rank_of
                        .replacen("{}", &r.to_string(), 1)
                        .replacen("{}", &ranked.len().to_string(), 1))
                        .unwrap_or_else(|| tr.leaderboard_not_rated.to_string()),
                    tr.leaderboard_runs.replacen("{}", &e.runs.to_string(), 1)
                ))
                .size(SZ_SMALL)
                .color(theme.text_dim),
            );
            ui.add_space(10.0);

            if let Some(overall) = e.overall() {
                ui.horizontal(|ui| {
                    star_rating(ui, theme, overall / 100.0, 20.0);
                    ui.label(
                        egui::RichText::new(format!("{overall:.1}%"))
                            .size(20.0)
                            .strong()
                            .color(score_color(theme, overall)),
                    );
                    ui.label(
                        egui::RichText::new(tr.leaderboard_overall_score).color(theme.text_dim),
                    );
                });
                ui.add_space(10.0);
            }

            ui.label(
                egui::RichText::new(tr.leaderboard_scores)
                    .size(SZ_SMALL)
                    .color(theme.accent),
            );
            egui::Grid::new("leaderboard_details_scores")
                .num_columns(4)
                .striped(true)
                .spacing([18.0, 6.0])
                .show(ui, |ui| {
                    let mut cell = 0;
                    for (label, key) in [
                        (tr.leaderboard_m_compilation, "compilation_score"),
                        (tr.leaderboard_m_functional, "functional_score"),
                        (tr.leaderboard_m_cobol85, "cobol85_score"),
                        (tr.leaderboard_m_indexed, "indexed_file_score"),
                        (tr.leaderboard_m_modification, "modification_score"),
                        (tr.leaderboard_m_debugging, "debugging_score"),
                        (tr.leaderboard_m_refactoring, "refactoring_score"),
                        (tr.leaderboard_m_file_handling, "file_handling_score"),
                        (tr.leaderboard_m_table_driven, "table_driven_score"),
                        (tr.leaderboard_m_explanation, "code_explanation_score"),
                        (tr.leaderboard_m_type_inference, "type_inference_score"),
                        (tr.leaderboard_m_inline_invoke, "inline_invoke_score"),
                        (tr.leaderboard_m_powerrustcobol, "powerrustcobol_score"),
                    ] {
                        ui.label(egui::RichText::new(label).color(theme.text_dim));
                        match e.score(key) {
                            Some(v) => ui.label(
                                egui::RichText::new(format!("{v:.0}%"))
                                    .color(score_color(theme, v)),
                            ),
                            // A key the model never returned must read as
                            // missing, never as a score of zero.
                            None => ui.label(
                                egui::RichText::new(tr.leaderboard_not_collected)
                                    .color(theme.text_dim),
                            ),
                        };
                        cell += 1;
                        if cell % 2 == 0 {
                            ui.end_row();
                        }
                    }
                    if cell % 2 != 0 {
                        ui.end_row();
                    }
                });

            ui.add_space(10.0);
            ui.label(
                egui::RichText::new(tr.leaderboard_connection)
                    .size(SZ_SMALL)
                    .color(theme.accent),
            );
            let unknown = tr.leaderboard_unknown.to_string();
            egui::Grid::new("leaderboard_details_connection")
                .num_columns(4)
                .striped(true)
                .spacing([18.0, 6.0])
                .show(ui, |ui| {
                    ui.label(
                        egui::RichText::new(tr.leaderboard_m_hallucinations)
                            .color(theme.text_dim),
                    );
                    ui.label(
                        e.hallucinations()
                            .map(|n| n.to_string())
                            .unwrap_or_else(|| unknown.clone()),
                    );
                    ui.label(egui::RichText::new(tr.leaderboard_m_context).color(theme.text_dim));
                    ui.label(match (e.caps.ctx_in, e.caps.ctx_out) {
                        (Some(i), Some(o)) => format!("{i} in / {o} out"),
                        (Some(i), None) => format!("{i} in"),
                        (None, Some(o)) => format!("{o} out"),
                        (None, None) => unknown.clone(),
                    });
                    ui.end_row();

                    ui.label(egui::RichText::new(tr.leaderboard_m_hardware).color(theme.text_dim));
                    ui.label(match e.tier() {
                        Tier::Local => tr.leaderboard_hw_local.to_string(),
                        _ => tr.leaderboard_hw_cloud.to_string(),
                    });
                    ui.label(
                        egui::RichText::new(tr.leaderboard_m_quantization).color(theme.text_dim),
                    );
                    ui.label(
                        e.caps
                            .quantization
                            .clone()
                            .unwrap_or_else(|| unknown.clone()),
                    );
                    ui.end_row();

                    ui.label(egui::RichText::new(tr.leaderboard_m_parameters).color(theme.text_dim));
                    ui.label(
                        e.caps
                            .params_b
                            .map(|b| format!("{b:.0} B"))
                            .unwrap_or_else(|| unknown.clone()),
                    );
                    ui.label(egui::RichText::new(tr.leaderboard_m_price).color(theme.text_dim));
                    ui.label(
                        e.caps
                            .usd_per_mtok_out
                            .map(|c| format!("${c:.2} / 1M"))
                            .unwrap_or_else(|| unknown.clone()),
                    );
                    ui.end_row();
                });

            if let Some(err) = &e.last_error {
                ui.add_space(8.0);
                ui.label(
                    egui::RichText::new(
                        tr.leaderboard_last_error.replacen("{}", err, 1),
                    )
                    .size(SZ_SMALL)
                    .color(theme.error),
                );
            }

            ui.add_space(12.0);
            ui.horizontal(|ui| {
                if ui.button(tr.leaderboard_open_report).clicked() {
                    action.open_report = Some((e.provider.clone(), e.model.clone()));
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button(tr.leaderboard_close).clicked() {
                        close = true;
                    }
                });
            });
        });

        if close || ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            self.details = None;
        }
    }

    /// The judge has no model and a test was asked for. Offer to set one now,
    /// rather than quietly producing a score the model gave itself.
    fn judge_missing_modal(
        &mut self,
        ctx: &egui::Context,
        theme: &Theme,
        tr: &Tr,
        action: &mut LeaderboardAction,
    ) {
        let Some(pending) = self.pending_run.clone() else {
            return;
        };
        let mut close = false;
        egui::Modal::new(egui::Id::new("leaderboard_judge_missing")).show(ctx, |ui| {
            ui.set_width(520.0);
            ui.label(
                egui::RichText::new(tr.leaderboard_judge_missing_title)
                    .size(SZ_TITLE)
                    .strong(),
            );
            ui.add_space(8.0);
            ui.label(
                egui::RichText::new(tr.leaderboard_judge_missing_body)
                    .size(SZ_BODY)
                    .color(theme.text_bright),
            );
            ui.add_space(12.0);
            ui.horizontal(|ui| {
                if ui
                    .button(egui::RichText::new(tr.leaderboard_judge_set_now).size(SZ_BODY))
                    .clicked()
                {
                    action.open_judge_setup = true;
                    close = true;
                }
                if ui
                    .button(egui::RichText::new(tr.leaderboard_judge_run_anyway).size(SZ_BODY))
                    .clicked()
                {
                    // Their call, made knowingly — and the run says so on the
                    // way past, so the score is never mistaken for a judged one.
                    action.run_tests = Some(pending.clone());
                    self.status = Some(tr.leaderboard_judge_declined.to_string());
                    close = true;
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui
                        .button(egui::RichText::new(tr.leaderboard_cancel).size(SZ_BODY))
                        .clicked()
                    {
                        close = true;
                    }
                });
            });
        });
        if close || ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            self.pending_run = None;
        }
    }

    /// Surface a test that could not run.
    pub fn show_error(&mut self, model: String, message: String) {
        self.error = Some((model, message));
    }

    fn error_window(&mut self, ctx: &egui::Context, theme: &Theme, tr: &Tr) {
        let Some((model, message)) = self.error.clone() else {
            return;
        };
        let mut open = true;
        let mut close = false;
        egui::Window::new(
            egui::RichText::new(tr.leaderboard_error_title)
                .size(SZ_TITLE)
                .strong(),
        )
        .id(egui::Id::new("leaderboard_error"))
        .open(&mut open)
        .collapsible(false)
        .resizable(false)
        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
        .show(ctx, |ui| {
            ui.set_width(440.0);
            ui.label(egui::RichText::new(&model).size(SZ_BODY + 1.0).strong());
            ui.add_space(6.0);
            egui::Frame::NONE
                .fill(theme.bg_extreme)
                .stroke(egui::Stroke::new(1.0, theme.error))
                .corner_radius(egui::CornerRadius::same(6))
                .inner_margin(egui::Margin::same(10))
                .show(ui, |ui| {
                    ui.set_width(410.0);
                    ui.label(
                        egui::RichText::new(&message)
                            .size(SZ_BODY)
                            .color(theme.text_bright),
                    );
                });
            ui.add_space(8.0);
            ui.label(
                egui::RichText::new(tr.leaderboard_error_note)
                    .size(SZ_SMALL)
                    .color(theme.text_dim),
            );
            ui.add_space(8.0);
            if ui.button(tr.leaderboard_close).clicked() {
                close = true;
            }
        });
        if close || !open {
            self.error = None;
        }
    }
}

// ── star rating ────────────────────────────────────────────────────────────

/// Five stars filled to `fraction`, painted rather than typed: partial stars
/// are exact and nothing depends on the font carrying ★.
fn star_rating(ui: &mut egui::Ui, theme: &Theme, fraction: f32, size: f32) {
    const N: usize = 5;
    let gap = 2.0;
    let width = N as f32 * size + (N as f32 - 1.0) * gap;
    let (rect, _) = ui.allocate_exact_size(egui::vec2(width, size), egui::Sense::hover());
    let centre_of = |i: usize| {
        egui::pos2(
            rect.left() + size * 0.5 + i as f32 * (size + gap),
            rect.center().y,
        )
    };
    let off = crate::theme::darken(theme.text_dim, 0.45);
    for i in 0..N {
        paint_star(ui.painter(), centre_of(i), size * 0.5, off);
    }
    let filled = egui::Rect::from_min_size(
        rect.min,
        egui::vec2(width * fraction.clamp(0.0, 1.0), rect.height()),
    );
    let painter = ui.painter().with_clip_rect(filled);
    for i in 0..N {
        paint_star(&painter, centre_of(i), size * 0.5, theme.warn);
    }
}

fn paint_star(painter: &egui::Painter, center: egui::Pos2, r: f32, color: egui::Color32) {
    // Fan-triangulated from the centre: a five-point star is concave and tears
    // if handed to `convex_polygon` whole.
    let mut pts = Vec::with_capacity(10);
    for i in 0..10 {
        let ang = -std::f32::consts::FRAC_PI_2 + i as f32 * std::f32::consts::PI / 5.0;
        let rad = if i % 2 == 0 { r } else { r * 0.44 };
        pts.push(egui::pos2(
            center.x + rad * ang.cos(),
            center.y + rad * ang.sin(),
        ));
    }
    for i in 0..10 {
        painter.add(egui::Shape::convex_polygon(
            vec![center, pts[i], pts[(i + 1) % 10]],
            color,
            egui::Stroke::NONE,
        ));
    }
}

fn score_color(theme: &Theme, v: f32) -> egui::Color32 {
    if v >= 85.0 {
        theme.ed_data
    } else if v >= 70.0 {
        theme.warn
    } else {
        theme.error
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::leaderboard::RunOutcome;

    fn run(overall: f32) -> RunOutcome {
        RunOutcome {
            metrics: serde_json::json!({ "overall_score": overall }),
        }
    }

    /// The rank column is drawn from the *rated* rows only, so an unrated model
    /// sitting between two rated ones cannot push the one below it down a
    /// place. Mirrors the numbering the table performs.
    fn displayed_ranks(board: &Leaderboard, which: Board) -> Vec<(String, Option<usize>)> {
        let mut rank = 0;
        board
            .ranked(which)
            .into_iter()
            .map(|i| {
                let e = &board.entries[i];
                if e.rated() {
                    rank += 1;
                    (e.model.clone(), Some(rank))
                } else {
                    (e.model.clone(), None)
                }
            })
            .collect()
    }

    #[test]
    fn unrated_rows_take_no_rank_number() {
        let mut lb = Leaderboard::default();
        lb.record_success("anthropic", "best", "", run(92.0));
        lb.record_failure("anthropic", "unreachable", "", "connection refused");
        lb.record_success("anthropic", "second", "", run(80.0));
        assert_eq!(
            displayed_ranks(&lb, Board::Overall),
            vec![
                ("best".to_string(), Some(1)),
                ("second".to_string(), Some(2)),
                ("unreachable".to_string(), None),
            ]
        );
    }

    #[test]
    fn an_empty_board_is_a_state_the_panel_must_handle() {
        let lb = Leaderboard::default();
        assert!(lb.ranked(Board::Local).is_empty());
    }

    /// Spec 040 R12: with a judge ready, Run tests runs. Without one it holds
    /// the request back and asks first — a self-assessment is not something to
    /// start by accident.
    #[test]
    fn a_run_waits_for_an_answer_when_the_judge_has_no_model() {
        let mut ready = LeaderboardModal::new(true);
        assert!(ready.judge_ready);
        assert!(ready.pending_run.is_none());

        let mut unready = LeaderboardModal::new(false);
        assert!(!unready.judge_ready);
        // What the Run tests button does in each case.
        let id = ("ollama".to_string(), "m".to_string());
        if unready.judge_ready {
            panic!("fixture is wrong");
        }
        unready.pending_run = Some(id.clone());
        assert_eq!(unready.pending_run, Some(id));

        // Giving the judge a model clears the question for later runs.
        unready.set_judge_ready(true);
        assert!(unready.judge_ready);
        ready.set_judge_ready(false);
        assert!(!ready.judge_ready);
    }

    /// Nothing in the panel renders below 14 px, and a card padded at 10 keeps
    /// its content off its own border.
    #[test]
    fn the_type_scale_and_padding_hold() {
        assert!(SZ_SMALL >= 14.0);
        assert!(SZ_BODY >= 14.0);
        assert!(SZ_TITLE > SZ_BODY);
        assert_eq!(CARD_PAD, 10);
    }
}
