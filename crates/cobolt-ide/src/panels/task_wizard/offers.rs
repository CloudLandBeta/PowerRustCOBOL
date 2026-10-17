// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Offers: the chip beside a selected DataGrid, TreeView, SideMenu, MenuBar or Maps (spec 104 R10,
//! R30).
//!
//! An offer only offers. It starts nothing by itself, it is drawn only while the wizard is on
//! and no task runs, and ✕ dismisses it for **that** control of that form of that project —
//! never for every grid.

use eframe::egui::{self, Id, Order, Pos2, Rect, RichText};

use super::facts::{ControlKind, Selected};
use super::tasks::TaskDef;
use crate::i18n::Tr;

/// What the developer did with an offer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OfferPick {
    Start(&'static str),
    Dismiss,
}

/// The dismissal key of an offer (R30): project, form, control and task.
pub fn dismissal_key(project: &str, form: &str, control: &str, task: &str) -> String {
    format!("{project}|{form}|{control}|{task}")
}

/// The task a selection is offered, if any (R10): exactly one control, of a kind a task names.
pub fn task_for<'t>(table: &'t [TaskDef], selected: &Selected) -> Option<&'t TaskDef> {
    if matches!(selected.kind, ControlKind::Other { .. }) {
        return None;
    }
    table.iter().find(|t| t.offer_for == Some(selected.kind))
}

/// Draw the offer for `task` beside `control` (the control's rect on the canvas).
pub fn show(ctx: &egui::Context, control: Rect, task: &TaskDef, tr: &Tr) -> Option<OfferPick> {
    let mut pick = None;
    let at = Pos2::new(control.right() + 6.0, control.top());
    egui::Area::new(Id::new("task_wizard_offer"))
        .order(Order::Foreground)
        .fixed_pos(at)
        .interactable(true)
        .show(ctx, |ui| {
            egui::Frame::popup(ui.style()).show(ui, |ui| {
                ui.horizontal(|ui| {
                    let label = format!("🗺 {}", (task.title)(tr));
                    if ui
                        .button(RichText::new(label))
                        .on_hover_text(tr.task_wizard_offer_tip)
                        .clicked()
                    {
                        pick = Some(OfferPick::Start(task.key));
                    }
                    if ui
                        .small_button("✕")
                        .on_hover_text(tr.task_wizard_offer_dismiss)
                        .clicked()
                    {
                        pick = Some(OfferPick::Dismiss);
                    }
                });
            });
        });
    pick
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::panels::task_wizard::TASKS;

    fn sel(kind: ControlKind) -> Selected {
        Selected {
            form: "Main".into(),
            control: "C1".into(),
            kind,
        }
    }

    #[test]
    fn only_the_controls_a_task_is_about_get_an_offer_and_each_its_own_task() {
        for (kind, key) in [
            (ControlKind::DataGrid, "datagrid"),
            (ControlKind::TreeView, "treeview"),
            (ControlKind::SideMenu, "sidemenu"),
            (ControlKind::MenuBar, "menubar"),
            (ControlKind::Maps, "maps"),
        ] {
            assert_eq!(task_for(TASKS, &sel(kind)).map(|t| t.key), Some(key));
        }
        for bindable in [true, false] {
            assert!(
                task_for(TASKS, &sel(ControlKind::Other { bindable })).is_none(),
                "no other control gets an offer (R10)"
            );
        }
    }

    #[test]
    fn a_dismissal_names_one_control_of_one_form_of_one_project() {
        let a = dismissal_key("/p", "Main", "Grid1", "datagrid");
        assert_ne!(a, dismissal_key("/p", "Main", "Grid2", "datagrid"));
        assert_ne!(a, dismissal_key("/p", "Other", "Grid1", "datagrid"));
        assert_ne!(a, dismissal_key("/q", "Main", "Grid1", "datagrid"));
    }
}
