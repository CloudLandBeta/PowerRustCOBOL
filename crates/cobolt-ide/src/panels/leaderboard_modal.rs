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

/// Opening size. From here on the size is whatever the developer dragged it to
/// — see [`LeaderboardModal::size`].
const DEFAULT_W: f32 = 1180.0;
const DEFAULT_H: f32 = 560.0;
/// Hard stops for the grip, so the window cannot be dragged into uselessness.
const MIN_W: f32 = 860.0;
const MIN_H: f32 = 300.0;
const MAX_W: f32 = 2400.0;
const MAX_H: f32 = 1600.0;
/// Side of the resize grip, and its inset from the window's border.
const GRIP: f32 = 14.0;
const GRIP_INSET: f32 = 3.0;
/// Everything above the rows: the two header lines, the card's own title,
/// padding and the footnote. Subtracted from the window height to give the row
/// area — a constant, so the rows never measure themselves against the space
/// they were handed.
const HEADER_H: f32 = 160.0;
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
    /// Provider chosen in the "add a model to test" row (spec 048 R20).
    add_provider: String,
    /// Model chosen in that row.
    add_model: String,
    /// The row whose Remove is awaiting a yes. One at a time — this is a
    /// destructive, deliberately unhurried action.
    confirm_retire: Option<(String, String)>,
    /// The window's size, owned here rather than by egui.
    ///
    /// This is the whole defence against the self-inflating window: the size
    /// changes **only** when the developer drags the grip. Children are laid
    /// out from this stored number, never from `available_width()` or
    /// `max_rect()`, so nothing a child measures can feed back into the size
    /// and grow it again on the next frame.
    size: egui::Vec2,
}

#[derive(Default)]
pub struct LeaderboardAction {
    /// Re-run the proficiency test for this `(provider, model)`.
    pub run_tests: Option<(String, String)>,
    // `apply_to_grace` / `apply_to_judge` / `apply_to_specialists` are gone
    // (operator, 2026-08-09). Assigning a model belongs to the Agent × Model
    // table, which shows every agent at once and checks the separation rule as
    // you pick; three buttons that silently rewrote several agents from a
    // screen displaying none of them was the wrong home for it.
    /// Reopen the stored benchmark report for this `(provider, model)`.
    pub open_report: Option<(String, String)>,
    /// Open the Agents Manager at the COBOL Proficiency Judge so a model can be
    /// given to it.
    pub open_judge_setup: bool,
    /// Put this `(provider, model)` on the board so it can be tested, even
    /// though no agent runs it (spec 048 R20).
    pub add_model: Option<(String, String)>,
    /// Take this `(provider, model)` off the board for good — the provider
    /// decommissioned it. Tombstoned, so the archive replay cannot bring it
    /// back; testing it again would.
    pub retire: Option<(String, String)>,
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
            add_provider: String::new(),
            add_model: String::new(),
            confirm_retire: None,
            size: egui::vec2(DEFAULT_W, DEFAULT_H),
        }
    }


    /// The resize grip, in its own foreground [`egui::Area`] pinned to the
    /// window's **outer** bottom-right corner, [`GRIP_INSET`] in from it.
    ///
    /// It lives outside the content on purpose: allocated inside, it joined the
    /// layout, pushed the card around and moved as the content moved, so a drag
    /// fought the thing it was sizing.
    ///
    /// Its drag delta is the one and only writer of `self.size`.
    fn resize_grip(&mut self, ctx: &egui::Context, theme: &Theme, window: egui::Rect) {
        let corner = window.max - egui::vec2(GRIP_INSET, GRIP_INSET);
        let origin = corner - egui::vec2(GRIP, GRIP);
        egui::Area::new(egui::Id::new("leaderboard_resize_grip"))
            .order(egui::Order::Foreground)
            .fixed_pos(origin)
            .show(ctx, |ui| {
                let (rect, response) =
                    ui.allocate_exact_size(egui::vec2(GRIP, GRIP), egui::Sense::drag());
                if response.dragged() {
                    self.size += response.drag_delta();
                    self.size.x = self.size.x.clamp(MIN_W, MAX_W);
                    self.size.y = self.size.y.clamp(MIN_H, MAX_H);
                }
                if response.hovered() || response.dragged() {
                    ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeNorthWest);
                }
                let colour = if response.hovered() || response.dragged() {
                    theme.accent
                } else {
                    theme.text_dim
                };
                let painter = ui.painter();
                for (i, offset) in [2.0_f32, 6.0, 10.0].iter().enumerate() {
                    let len = GRIP - offset - 1.0;
                    if len <= 0.0 {
                        continue;
                    }
                    let stroke = egui::Stroke::new(if i == 0 { 1.6 } else { 1.2 }, colour);
                    painter.line_segment(
                        [
                            egui::pos2(rect.right() - len, rect.bottom()),
                            egui::pos2(rect.right(), rect.bottom() - len),
                        ],
                        stroke,
                    );
                }
            });
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
        llm: &crate::llm::LlmConfig,
        theme: &Theme,
        tr: &Tr,
    ) -> LeaderboardAction {
        let mut action = LeaderboardAction::default();
        let mut open = self.open;

        let window = egui::Window::new(
            egui::RichText::new(tr.leaderboard_title)
                .size(SZ_TITLE)
                .strong(),
        )
        .id(egui::Id::new("model_leaderboard"))
        .open(&mut open)
        .collapsible(false)
        // NEVER `resizable(true)` here. egui then negotiates the window
        // rectangle against its contents every frame, and this window's rows
        // are a scroll area that reports what it would like to be — so the two
        // push each other outward and the window walks to the screen edge on
        // its own. Debug Settings survives that only because its content is
        // pinned to a constant height and never asks for more.
        //
        // The size is ours, it is exact, and the ONLY thing that changes it is
        // the developer dragging the grip.
        .resizable(false)
        .fixed_size(self.size)
        .default_pos([40.0, 60.0])
        .show(ctx, |ui| {
            // `self.size` is the size of the RESIZE area, and egui puts the
            // window margin INSIDE that (window.rs: the title bar and the body
            // are both children of `Resize`, and the body is wrapped in
            // `Frame::NONE.inner_margin(window_margin)`). So the content that
            // fits is the stored width minus that margin — subtracting
            // `CARD_PAD` instead was subtracting the card's own padding, a
            // different number (10 against 12).
            //
            // Being 4 px too wide is what opened the gap at the corners: the
            // body overflowed the resize area, the window frame followed the
            // body, and the TITLE BAR — laid out at the width the resize area
            // offered — stayed 6 px short of it. Two rounded corners of the
            // same radius and the same stroke, one inside the other.
            let margin = ui.style().spacing.window_margin.sum().x;
            let stroke = 2.0 * ui.style().visuals.window_stroke.width;
            let width = self.size.x - margin - stroke;
            ui.set_width(width);
            self.header(ui, board, theme, tr);
            // Spec 048 R20 — a model can be benchmarked without any agent
            // using it. The board is where models are compared, so this is
            // where one is put forward for testing.
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                ui.label(tr.agents_tbl_provider_scope);
                let configured = llm.configured_providers();
                let current = if self.add_provider.trim().is_empty() {
                    "—".to_string()
                } else {
                    crate::llm::Provider::from_id(&self.add_provider)
                        .map(|p| p.label.to_string())
                        .unwrap_or_else(|| self.add_provider.clone())
                };
                egui::ComboBox::from_id_salt("lb_add_provider")
                    .selected_text(current)
                    .show_ui(ui, |ui| {
                        if configured.is_empty() {
                            ui.weak(tr.providers_unconfigured);
                        }
                        for id in &configured {
                            let label = crate::llm::Provider::from_id(id)
                                .map(|p| p.label.to_string())
                                .unwrap_or_else(|| id.clone());
                            ui.selectable_value(&mut self.add_provider, id.clone(), label);
                        }
                    });
                let offered = llm.models_for(&self.add_provider);
                egui::ComboBox::from_id_salt("lb_add_model")
                    .selected_text(if self.add_model.is_empty() {
                        tr.agents_tbl_model
                    } else {
                        &self.add_model
                    })
                    .width(240.0)
                    .show_ui(ui, |ui| {
                        for model in offered {
                            ui.selectable_value(&mut self.add_model, model.clone(), model);
                        }
                    });
                let ready = !self.add_provider.trim().is_empty()
                    && !self.add_model.trim().is_empty()
                    && !board.contains(&self.add_provider, &self.add_model);
                if ui
                    .add_enabled(ready, egui::Button::new(tr.leaderboard_add_model))
                    .clicked()
                {
                    action.add_model =
                        Some((self.add_provider.clone(), self.add_model.clone()));
                }
            });
            ui.add_space(8.0);
            // Everything below is measured from `self.size`, never from the Ui.
            let rows_h = (self.size.y - HEADER_H).max(120.0);
            egui::Frame::NONE
                .fill(theme.bg_panel)
                .stroke(egui::Stroke::new(1.0, theme.panel_border()))
                .corner_radius(egui::CornerRadius::same(10))
                .inner_margin(egui::Margin::same(CARD_PAD))
                .show(ui, |ui| {
                    let inner = width - 2.0 * CARD_PAD as f32 - 2.0;
                    ui.set_min_width(inner);
                    ui.set_max_width(inner);
                    self.table(ui, board, theme, tr, &mut action, rows_h);
                });
        });

        // The grip, pinned to the window's outer corner on its own layer. It
        // reads no size from the window — it only adds its drag delta to ours,
        // so nothing the content does can move the window.
        if let Some(window) = window {
            self.resize_grip(ctx, theme, window.response.rect);
        }

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
                // `truncate()`, not a plain label: this sits in a horizontal
                // row that nothing clips, and the Run tests status is the
                // longest string in the panel ("Running the test — the COBOL
                // Proficiency Judge (…) will re-score the result."). Left to
                // its natural width it pushed the header past the content
                // width, the window frame followed it, and the title bar —
                // fixed at the width `Resize` offered — stayed behind, which
                // is the corner gap again, this time appearing only once a
                // Run tests click had set a status.
                ui.add(
                    egui::Label::new(
                        egui::RichText::new(s).size(SZ_SMALL).color(theme.ed_data),
                    )
                    .truncate(),
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
        rows_h: f32,
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
        // `both`, not `vertical` — and the horizontal direction is what keeps
        // the window still. A scroll area whose horizontal direction is
        // DISABLED sizes itself to `available.max(content)` (egui
        // `scroll_area.rs`, "Expand to fit content"), so a row wider than the
        // window makes the scroll area wider than the window. That width is the
        // content's `min_rect`, and on a non-resizable axis egui reports
        // `last_content_size` as the window's size — `fixed_size` bounds only
        // the space OFFERED to the content, never the rect the window ends up
        // with. So the row pushed the window open from the inside, with nothing
        // touching the grip and `self.size` never moving.
        //
        // The row is genuinely wide: six columns ending in five buttons, and
        // the translations run half again as long as the English ("Utiliser
        // pour tous les spécialistes"). With the direction ENABLED the area
        // takes the width it is given and scrolls the overflow instead.
        egui::ScrollArea::both()
            .id_salt("leaderboard_rows")
            // Shrinks to the rows it has (so a short board is a short card)
            // but never past the height the window was dragged to.
            .max_height(rows_h)
            .auto_shrink([false, true])
            .show(ui, |ui| {
                egui::Grid::new("leaderboard_grid")
                    .num_columns(6)
                    .striped(true)
                    .spacing([14.0, 7.0])
                    .min_col_width(26.0)
                    .show(ui, |ui| {
                        for h in [
                            "",
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
                                // Gold, silver, bronze — and nothing at all for
                                // a row with no rank to be third of.
                                paint_trophy(ui, rank, SZ_BODY + 8.0);
                                ui.label(
                                    egui::RichText::new(format!("#{rank}"))
                                        .size(SZ_BODY)
                                        .strong()
                                        .color(theme.text_bright),
                                );
                            } else {
                                ui.label("");
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
                                // "Use for Grace / Judge / All Specialists"
                                // used to sit here. Assigning a model is the
                                // Agent × Model table's job, where you can see
                                // which agent gets what and the separation rule
                                // is checked as you choose. Three buttons that
                                // silently rewrote several agents from a screen
                                // that shows none of them was the wrong place
                                // for it (operator, 2026-08-09).
                                //
                                // Retiring a model by hand, for the provider
                                // that shut one down without the catalogue
                                // saying so yet. Confirmed, because a score is
                                // hours and tokens, and the row does not come
                                // back on its own once it goes.
                                if self.confirm_retire.as_ref() == Some(&id) {
                                    ui.label(
                                        egui::RichText::new(tr.leaderboard_retire_confirm)
                                            .size(SZ_BODY)
                                            .color(egui::Color32::from_rgb(220, 120, 120)),
                                    );
                                    if ui
                                        .button(
                                            egui::RichText::new(tr.leaderboard_retire_yes)
                                                .size(SZ_BODY),
                                        )
                                        .clicked()
                                    {
                                        action.retire = Some(id.clone());
                                        self.confirm_retire = None;
                                    }
                                    if ui
                                        .button(egui::RichText::new(tr.btn_cancel).size(SZ_BODY))
                                        .clicked()
                                    {
                                        self.confirm_retire = None;
                                    }
                                } else if ui
                                    .button(
                                        egui::RichText::new(tr.leaderboard_retire).size(SZ_BODY),
                                    )
                                    .on_hover_text(tr.leaderboard_retire_hint)
                                    .clicked()
                                {
                                    self.confirm_retire = Some(id.clone());
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

    /// Surface a test that could not run — and record it in the IDE console
    /// (operator, 2026-08-09), naming the model so the line still means
    /// something once this window is gone.
    pub fn show_error(&mut self, model: String, message: String) {
        crate::error_log::record(format!("{model}: {message}"));
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

// ── trophies ───────────────────────────────────────────────────────────────

/// Gold, silver and bronze for the top three **ranked** models.
///
/// `rank` is the position among rated rows only, so a model that could not be
/// tested is never in the running for one: it holds no rank, and a trophy is a
/// claim about a result it does not have.
fn trophy_colour(rank: usize) -> Option<(egui::Color32, egui::Color32)> {
    match rank {
        1 => Some((
            egui::Color32::from_rgb(255, 201, 74),
            egui::Color32::from_rgb(168, 122, 20),
        )),
        2 => Some((
            egui::Color32::from_rgb(205, 213, 222),
            egui::Color32::from_rgb(126, 137, 150),
        )),
        3 => Some((
            egui::Color32::from_rgb(205, 127, 50),
            egui::Color32::from_rgb(126, 74, 26),
        )),
        _ => None,
    }
}

/// A cup on a stem and base, painted rather than typed — 🏆 is not in every
/// font, and a missing glyph in the first column would read as a broken row.
fn paint_trophy(ui: &mut egui::Ui, rank: usize, size: f32) {
    let (rect, _) = ui.allocate_exact_size(egui::vec2(size, size), egui::Sense::hover());
    let Some((face, rim)) = trophy_colour(rank) else {
        return;
    };
    let painter = ui.painter();
    let w = size;
    let cx = rect.center().x;
    let top = rect.top() + w * 0.14;
    let cup_h = w * 0.40;
    let half_top = w * 0.24;
    let half_bot = w * 0.12;

    // Handles first, so the cup's own edge covers where they meet it.
    for dir in [-1.0_f32, 1.0] {
        painter.add(egui::Shape::line(
            vec![
                egui::pos2(cx + dir * half_top, top + w * 0.03),
                egui::pos2(cx + dir * (half_top + w * 0.15), top + w * 0.09),
                egui::pos2(cx + dir * (half_top + w * 0.09), top + w * 0.23),
                egui::pos2(cx + dir * half_top * 0.85, top + w * 0.27),
            ],
            egui::Stroke::new(1.6, rim),
        ));
    }
    // Cup: a bowl tapering to the stem.
    painter.add(egui::Shape::convex_polygon(
        vec![
            egui::pos2(cx - half_top, top),
            egui::pos2(cx + half_top, top),
            egui::pos2(cx + half_bot, top + cup_h),
            egui::pos2(cx - half_bot, top + cup_h),
        ],
        face,
        egui::Stroke::new(1.0, rim),
    ));
    // Stem, then the base it stands on.
    painter.rect_filled(
        egui::Rect::from_min_max(
            egui::pos2(cx - w * 0.05, top + cup_h),
            egui::pos2(cx + w * 0.05, top + cup_h + w * 0.13),
        ),
        0.0,
        rim,
    );
    painter.rect_filled(
        egui::Rect::from_min_max(
            egui::pos2(cx - w * 0.20, top + cup_h + w * 0.13),
            egui::pos2(cx + w * 0.20, top + cup_h + w * 0.21),
        ),
        2.0,
        face,
    );
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

    /// Trophies go to the top three **ranked** models. An untestable model
    /// holds no rank, so it can never take a trophy from a model that earned
    /// one — and with two rated rows, only two trophies are awarded.
    #[test]
    fn only_ranked_models_take_trophies() {
        assert!(trophy_colour(1).is_some());
        assert!(trophy_colour(2).is_some());
        assert!(trophy_colour(3).is_some());
        assert!(trophy_colour(4).is_none(), "fourth place gets no trophy");
        assert!(trophy_colour(0).is_none(), "an unranked row gets no trophy");

        let mut lb = Leaderboard::default();
        lb.record_success("anthropic", "first", "", run(92.0));
        lb.record_failure("anthropic", "unreachable", "", "connection refused");
        lb.record_success("anthropic", "second", "", run(80.0));
        let awarded: Vec<_> = displayed_ranks(&lb, Board::Overall)
            .into_iter()
            .filter(|(_, rank)| rank.map(|r| trophy_colour(r).is_some()).unwrap_or(false))
            .map(|(model, _)| model)
            .collect();
        assert_eq!(awarded, vec!["first", "second"]);
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

    /// The window opens at its default and stays there until dragged. This is
    /// the property that keeps a resizable egui window from inflating: nothing
    /// but a drag delta may move the stored size.
    #[test]
    fn the_size_only_changes_by_a_drag() {
        let mut m = LeaderboardModal::new(true);
        let opened = m.size;
        assert_eq!(opened, egui::vec2(DEFAULT_W, DEFAULT_H));

        // What `resize_grip` does with a drag delta.
        m.size += egui::vec2(120.0, 80.0);
        m.size.x = m.size.x.clamp(MIN_W, MAX_W);
        m.size.y = m.size.y.clamp(MIN_H, MAX_H);
        assert_eq!(m.size, egui::vec2(DEFAULT_W + 120.0, DEFAULT_H + 80.0));
    }

    /// Dragging inward stops at the minimum rather than collapsing the window,
    /// and outward at the maximum rather than running off the screen.
    #[test]
    fn the_grip_clamps_at_both_ends() {
        let mut m = LeaderboardModal::new(true);
        m.size += egui::vec2(-9000.0, -9000.0);
        m.size.x = m.size.x.clamp(MIN_W, MAX_W);
        m.size.y = m.size.y.clamp(MIN_H, MAX_H);
        assert_eq!(m.size, egui::vec2(MIN_W, MIN_H));

        m.size += egui::vec2(9000.0, 9000.0);
        m.size.x = m.size.x.clamp(MIN_W, MAX_W);
        m.size.y = m.size.y.clamp(MIN_H, MAX_H);
        assert_eq!(m.size, egui::vec2(MAX_W, MAX_H));
    }

    /// Renders the real panel for a run of frames and reports, per frame, the
    /// stored size and the rect egui actually gave the window.
    ///
    /// Every other size test in this file does arithmetic on `self.size` and
    /// never lays anything out, which is why they all passed while the window
    /// on screen kept growing. `fixed_size` bounds the space *offered* to the
    /// content; on a non-resizable axis egui reports `last_content_size` as the
    /// window's size (`Resize::end`), so content wider than the fixed size
    /// moves the window rect and nothing in the stored number ever shows it.
    fn frames(lang: crate::i18n::Language, n: usize) -> Vec<(egui::Vec2, egui::Rect)> {
        let ctx = egui::Context::default();
        let theme = crate::theme::default_theme();
        let tr = lang.tr();
        // Model names of the length the store actually holds — an Ollama tag
        // carries its quantisation, and those are the widest cells in the row.
        let mut lb = Leaderboard::default();
        lb.record_success("anthropic", "claude-opus-5", "", run(94.0));
        lb.record_success("openai", "gpt-5", "", run(88.0));
        lb.record_success("ollama", "qwen2.5-coder:32b-instruct-q4_K_M", "", run(61.0));
        lb.record_failure("ollama", "deepseek-coder-v2:16b", "", "connection refused");

        let mut m = LeaderboardModal::new(true);
        let mut seen = Vec::new();
        for _ in 0..n {
            let mut input = egui::RawInput::default();
            // A screen far larger than the window, so nothing here is the
            // screen clamping the growth out of sight.
            input.screen_rect = Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(3000.0, 2000.0),
            ));
            ctx.begin_pass(input);
            m.show(&ctx, &lb, &crate::llm::LlmConfig::load_defaults_for_test(), theme, &tr);
            // epaint 0.36 asserts on dropping unapplied texture deltas.
            ctx.end_pass().textures_delta.clear();
            let rect = ctx
                .memory(|mem| mem.area_rect(egui::Id::new("model_leaderboard")))
                .expect("the leaderboard window did not register an area");
            seen.push((m.size, rect));
        }
        seen
    }

    /// Every rounded rect the panel paints on a settled frame, under the app's
    /// real visuals (the default egui style has a different window radius and
    /// margin, so it hides exactly the mismatch we are looking for).
    fn painted_rects() -> Vec<egui::epaint::RectShape> {
        let ctx = egui::Context::default();
        let theme = crate::theme::default_theme();
        let tr = crate::i18n::Language::English.tr();
        let mut lb = Leaderboard::default();
        lb.record_success("anthropic", "claude-opus-5", "", run(94.0));
        lb.record_success("ollama", "qwen2.5-coder:32b", "", run(61.0));
        let mut m = LeaderboardModal::new(true);
        // A Run tests click puts this beside the board tabs, and it is by far
        // the widest thing in the header.
        m.set_status(
            "Running the test — the COBOL Proficiency Judge (claude-sonnet-5) \
             will re-score the result."
                .to_string(),
        );

        crate::app::apply_glass_visuals(&ctx, theme);
        let mut rects = Vec::new();
        for pass in 0..4 {
            let mut input = egui::RawInput::default();
            input.screen_rect = Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(2000.0, 1200.0),
            ));
            ctx.begin_pass(input);
            m.show(&ctx, &lb, &crate::llm::LlmConfig::load_defaults_for_test(), theme, &tr);
            let mut full = ctx.end_pass();
            // epaint 0.36 asserts on dropping unapplied texture deltas.
            full.textures_delta.clear();
            if pass < 3 {
                continue;
            }
            for cs in &full.shapes {
                collect(&cs.shape, &mut rects);
            }
        }
        rects
    }

    fn collect(shape: &egui::Shape, out: &mut Vec<egui::epaint::RectShape>) {
        match shape {
            egui::Shape::Rect(r) => out.push(r.clone()),
            egui::Shape::Vec(v) => {
                for s in v {
                    collect(s, out);
                }
            }
            _ => {}
        }
    }

    fn describe(r: &egui::epaint::RectShape) -> String {
        let c = r.corner_radius;
        format!(
            "[{:.0},{:.0} .. {:.0},{:.0}] r=({},{},{},{}) stroke={:.1}",
            r.rect.min.x,
            r.rect.min.y,
            r.rect.max.x,
            r.rect.max.y,
            c.nw,
            c.ne,
            c.sw,
            c.se,
            r.stroke.width
        )
    }

    /// Diagnostic: dump every rounded rect, so a concentric mismatch reads as
    /// numbers instead of a screenshot. This is what found the corner gap.
    #[test]
    #[ignore]
    fn dump_rounded_rects() {
        let dump: Vec<String> = painted_rects()
            .iter()
            .filter(|r| {
                let c = r.corner_radius;
                c.nw != 0 || c.ne != 0 || c.sw != 0 || c.se != 0
            })
            .map(describe)
            .collect();
        panic!("rounded rects:\n  {}", dump.join("\n  "));
    }

    /// The window frame and the title bar must be the SAME rectangle.
    ///
    /// They are drawn with the same stroke and the same 12px top radius, so any
    /// disagreement paints two concentric rounded corners with a gap between
    /// them — which is what showed at the top-right corner. The cause was the
    /// content width: `fixed_size` refers to the window's OUTER size, and egui
    /// fits the title bar and the body inside it after taking off the frame's
    /// total margin (`window.rs`: `resize.max_size -= window_frame
    /// .total_margin()`, itself `inner_margin + stroke`) and then the window
    /// margin around the body. Content that does not clear BOTH pushes the
    /// frame out past the title bar, which stays at the width `Resize` offered.
    #[test]
    fn the_title_bar_and_the_window_frame_are_the_same_rect() {
        let rects = painted_rects();
        // The frame: rounded on all four corners, and stroked (the drop shadow
        // shares its radius but carries no stroke).
        let frame = rects
            .iter()
            .find(|r| {
                let c = r.corner_radius;
                c.nw == 12 && c.ne == 12 && c.sw == 12 && c.se == 12 && r.stroke.width > 0.0
            })
            .expect("no window frame painted");
        // The title bar: top corners rounded, bottom square.
        let title = rects
            .iter()
            .find(|r| {
                let c = r.corner_radius;
                c.nw == 12 && c.ne == 12 && c.sw == 0 && c.se == 0
            })
            .expect("no title bar painted");

        for (edge, a, b) in [
            ("left", frame.rect.left(), title.rect.left()),
            ("right", frame.rect.right(), title.rect.right()),
        ] {
            assert!(
                (a - b).abs() < 0.5,
                "the window frame and the title bar disagree on their {edge} edge \
                 ({a:.1} vs {b:.1}) — that difference paints as a gap between two \
                 rounded corners.\n  frame: {}\n  title: {}",
                describe(frame),
                describe(title)
            );
        }
    }

    /// The window must be the size we asked for and must stay there while
    /// nobody touches it.
    ///
    /// Swept over every language: the row ends in five buttons, and the
    /// translations of "Use for All Specialists" run half again as long as the
    /// English, so a window that fits in one language is not evidence for any
    /// other.
    #[test]
    fn the_window_does_not_grow_while_nobody_touches_it() {
        for lang in crate::i18n::Language::ALL {
            check_one_language(*lang);
        }
    }

    fn check_one_language(lang: crate::i18n::Language) {
        let seen = frames(lang, 16);
        let report = |s: &[(egui::Vec2, egui::Rect)]| {
            s.iter()
                .enumerate()
                .map(|(i, (size, rect))| {
                    format!(
                        "  frame {i}: size {:.0}x{:.0}  rect {:.0}x{:.0}",
                        size.x,
                        size.y,
                        rect.width(),
                        rect.height()
                    )
                })
                .collect::<Vec<_>>()
                .join("\n")
        };

        // The stored size is the control: if this moved, something other than
        // the grip is writing it and the content is not the driver.
        for (size, _) in &seen {
            assert_eq!(
                *size,
                egui::vec2(DEFAULT_W, DEFAULT_H),
                "[{:?}] the stored size changed with no drag:\n{}",
                lang,
                report(&seen)
            );
        }

        // Settled: the rect must not still be climbing at the end of the run.
        let (_, first) = seen[2];
        for (_, rect) in &seen[2..] {
            assert!(
                (rect.width() - first.width()).abs() < 0.5,
                "[{:?}] the window kept growing frame over frame:\n{}",
                lang,
                report(&seen)
            );
        }

        // And it must be the size we asked for, not merely stable at some
        // other size the content dictated — in either direction.
        let (_, last) = seen[seen.len() - 1];
        assert!(
            (last.width() - DEFAULT_W).abs() < 0.5,
            "[{:?}] the window is wider than its fixed size — the content is the driver:\n{}",
            lang,
            report(&seen)
        );
    }

    /// The row area is derived from the stored size, never from the space the
    /// Ui handed back — the shape that inflates.
    #[test]
    fn the_row_area_follows_the_stored_height() {
        let mut m = LeaderboardModal::new(true);
        let rows = |h: f32| (h - HEADER_H).max(120.0);
        assert!(rows(m.size.y) > 120.0);
        m.size.y = MIN_H;
        assert_eq!(rows(m.size.y), MIN_H - HEADER_H);
        assert!(
            rows(m.size.y) >= 120.0,
            "even at the minimum the rows keep a usable floor instead of going negative"
        );
    }
}
