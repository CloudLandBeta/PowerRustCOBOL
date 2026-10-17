// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! The catalogue: the popup under the Task Wizard toggle (spec 104 R3, R7–R9).
//!
//! One row per task, with its state — *new*, *in progress* in the open project, *done* on
//! this machine — and the buttons that state allows. With no project open every row is
//! disabled and the popup says why (R7). The popup is an `egui::Popup`, so it closes on a
//! click outside it and on `Esc`, like every other popup in the IDE.

use eframe::egui::{self, Id, Rect, RichText};

use super::tasks::TaskDef;
use crate::i18n::Tr;

/// A task's state, as the catalogue shows it (R8).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskState {
    New,
    InProgress,
    Done,
}

/// What the developer picked.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Pick {
    Start(&'static str),
    Resume(&'static str),
    Restart(&'static str),
}

/// The catalogue's popup id in the window it is drawn in.
pub fn popup_id() -> Id {
    Id::new("task_wizard_catalogue")
}

/// Open the catalogue in the window `ctx` draws (R3).
pub fn open(ctx: &egui::Context) {
    egui::Popup::open_id(ctx, popup_id());
}

/// Where this window's toolbar drew the switch, for the catalogue to hang under. Kept per
/// OS window: egui's temporary data is shared by every viewport of the context.
fn toggle_key(ctx: &egui::Context) -> Id {
    Id::new(("task_wizard_toggle_rect", ctx.viewport_id()))
}

// The key is taken BEFORE the data lock: `toggle_key` asks the context for its viewport, and
// asking the context anything while `data_mut` holds it is a deadlock.
pub fn remember_toggle(ctx: &egui::Context, rect: Rect) {
    let key = toggle_key(ctx);
    ctx.data_mut(|d| d.insert_temp(key, rect));
}

pub fn toggle_rect(ctx: &egui::Context) -> Option<Rect> {
    let key = toggle_key(ctx);
    ctx.data(|d| d.get_temp::<Rect>(key))
}

/// The state of `task` for the catalogue: in progress in the open project wins over done,
/// so a task restarted on a machine where it was once finished can still be resumed.
pub fn state_of(in_progress: bool, done: bool) -> TaskState {
    if in_progress {
        TaskState::InProgress
    } else if done {
        TaskState::Done
    } else {
        TaskState::New
    }
}

/// Draw the catalogue, if open, under `anchor` — the toggle that opened it.
pub fn show(
    ctx: &egui::Context,
    anchor: Rect,
    table: &[TaskDef],
    state: &dyn Fn(&str) -> TaskState,
    has_project: bool,
    tr: &Tr,
) -> Option<Pick> {
    if !egui::Popup::is_id_open(ctx, popup_id()) {
        return None;
    }
    let mut pick = None;
    let layer = egui::LayerId::new(egui::Order::Foreground, Id::new("task_wizard_catalogue_layer"));
    egui::Popup::new(popup_id(), ctx.clone(), anchor, layer)
        .open_memory(None)
        .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
        .width(CATALOGUE_W)
        .show(|ui| {
            ui.set_width(CATALOGUE_W);
            ui.label(RichText::new(tr.task_wizard_catalogue_title).strong().size(15.0));
            if !has_project {
                ui.label(RichText::new(tr.task_wizard_needs_project).italics());
            }
            ui.separator();
            for task in table {
                let st = state(task.key);
                ui.horizontal(|ui| {
                    ui.label(RichText::new((task.title)(tr)).strong());
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.label(RichText::new(match st {
                            TaskState::New => tr.task_wizard_state_new,
                            TaskState::InProgress => tr.task_wizard_state_in_progress,
                            TaskState::Done => tr.task_wizard_state_done,
                        }).small());
                    });
                });
                ui.label(RichText::new((task.blurb)(tr)).small());
                ui.horizontal(|ui| {
                    ui.add_enabled_ui(has_project, |ui| match st {
                        TaskState::New => {
                            if ui.button(tr.task_wizard_start).clicked() {
                                pick = Some(Pick::Start(task.key));
                            }
                        }
                        TaskState::InProgress => {
                            if ui.button(tr.task_wizard_resume).clicked() {
                                pick = Some(Pick::Resume(task.key));
                            }
                            if ui.button(tr.task_wizard_restart).clicked() {
                                pick = Some(Pick::Restart(task.key));
                            }
                        }
                        TaskState::Done => {
                            if ui.button(tr.task_wizard_restart).clicked() {
                                pick = Some(Pick::Restart(task.key));
                            }
                        }
                    })
                    .response
                    .on_disabled_hover_text(tr.task_wizard_needs_project);
                });
                ui.add_space(4.0);
            }
        });
    if pick.is_some() {
        egui::Popup::close_id(ctx, popup_id());
    }
    pick
}

/// The catalogue's width: a constant, never the space available (R34).
const CATALOGUE_W: f32 = 380.0;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::i18n::Language;
    use crate::panels::task_wizard::TASKS;

    /// Every text a frame painted.
    fn painted(ctx: &egui::Context, has_project: bool, state: &dyn Fn(&str) -> TaskState) -> Vec<String> {
        let tr = Language::English.tr();
        let input = egui::RawInput {
            screen_rect: Some(Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1280.0, 900.0))),
            ..Default::default()
        };
        let mut out = ctx.run_ui(input, |ui| {
            let _ = show(ui.ctx(), Rect::from_min_size(egui::pos2(900.0, 4.0), egui::vec2(120.0, 24.0)), TASKS, state, has_project, &tr);
        });
        out.textures_delta.clear();
        fn walk(s: &egui::Shape, out: &mut Vec<String>) {
            match s {
                egui::Shape::Text(t) => out.push(t.galley.text().to_owned()),
                egui::Shape::Vec(v) => v.iter().for_each(|s| walk(s, out)),
                _ => {}
            }
        }
        let mut texts = Vec::new();
        for c in &out.shapes {
            walk(&c.shape, &mut texts);
        }
        texts
    }

    /// R7, R8 — the eight tasks, each with its state and its buttons; with no project open,
    /// the reason is said.
    #[test]
    fn the_catalogue_lists_every_task_with_its_state() {
        let tr = Language::English.tr();
        let ctx = egui::Context::default();
        let state = |k: &str| match k {
            "models" => TaskState::Done,
            "binding" => TaskState::InProgress,
            _ => TaskState::New,
        };
        open(&ctx);
        let mut texts = Vec::new();
        for _ in 0..3 {
            texts = painted(&ctx, true, &state);
        }
        for task in TASKS {
            assert!(texts.iter().any(|t| t == (task.title)(&tr)), "{} is not listed", task.key);
        }
        for s in [tr.task_wizard_state_new, tr.task_wizard_state_in_progress, tr.task_wizard_state_done] {
            assert!(texts.iter().any(|t| t == s), "no {s:?} shown");
        }
        assert!(texts.iter().any(|t| t == tr.task_wizard_resume), "an in-progress task offers Resume");
        assert!(!texts.iter().any(|t| t == tr.task_wizard_needs_project));

        let ctx = egui::Context::default();
        open(&ctx);
        for _ in 0..3 {
            texts = painted(&ctx, false, &state);
        }
        assert!(texts.iter().any(|t| t == tr.task_wizard_needs_project), "R7: the reason is said");
    }

    #[test]
    fn in_progress_wins_over_done() {
        assert_eq!(state_of(true, true), TaskState::InProgress);
        assert_eq!(state_of(false, true), TaskState::Done);
        assert_eq!(state_of(false, false), TaskState::New);
    }
}
