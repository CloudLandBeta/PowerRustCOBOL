// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Objects list — every control already placed on the form being designed.
//!
//! The middle section of the Form Designer's left sidebar. The canvas can only
//! select what the pointer can reach, so a control sitting underneath another
//! one — or behind an opaque container — is unreachable there. This list names
//! all of them, and clicking a name selects that control exactly as clicking it
//! on the canvas would, properties pane included.
//!
//! Containment is shown by indentation. `Form::controls` is one flat list with
//! `Control::parent` links (spec 012), so the tree is derived here rather than
//! walked — and derived in the form's own control order, so the list does not
//! reshuffle itself while the developer works.

use cobolt_forms::model::{Control, ControlType, Form};
use egui::{RichText, Sense, Ui, Vec2};

/// One row of the list: a control, and how deep its container chain runs.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ObjectRow {
    pub id: String,
    pub control_type: ControlType,
    /// 0 for a control parented directly to the form, +1 per enclosing container.
    pub depth: usize,
}

/// Flatten the form's controls into display rows: parents before their
/// children, siblings in the form's own order, `depth` from the parent chain.
///
/// A control whose `parent` names something that is not in the form is treated
/// as a child of the form rather than dropped — GOLDEN RULE *user code is
/// sacred* applies to the developer's controls too: a broken link must still be
/// selectable so it can be fixed, never silently invisible.
pub fn object_rows(form: &Form) -> Vec<ObjectRow> {
    let known: std::collections::HashSet<&str> =
        form.controls.iter().map(|c| c.id.as_str()).collect();

    // Root = no parent, or a parent that no longer exists.
    let is_root = |c: &Control| match c.parent.as_deref() {
        None => true,
        Some(p) => !known.contains(p),
    };

    let mut rows = Vec::with_capacity(form.controls.len());
    let mut stack: Vec<(String, usize)> = form
        .controls
        .iter()
        .filter(|c| is_root(c))
        .rev()
        .map(|c| (c.id.clone(), 0usize))
        .collect();

    // Depth-first, children pushed in reverse so they pop in form order.
    // `seen` guards against a parent cycle in a hand-edited `.cfrm`: without it
    // a two-control loop would spin here forever.
    let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();
    while let Some((id, depth)) = stack.pop() {
        let Some(ctrl) = form.controls.iter().find(|c| c.id == id) else {
            continue;
        };
        if !seen.insert(id.clone()) {
            continue;
        }
        rows.push(ObjectRow {
            id: ctrl.id.clone(),
            control_type: ctrl.control_type.clone(),
            depth,
        });
        for child in form
            .controls
            .iter()
            .filter(|c| c.parent.as_deref() == Some(ctrl.id.as_str()))
            .rev()
        {
            stack.push((child.id.clone(), depth + 1));
        }
    }
    rows
}

/// Draw the list. Returns the id the developer clicked, if any.
///
/// `selected` is the designer's current selection, so the row highlight and the
/// canvas always agree.
pub fn show(
    ui: &mut Ui,
    form: &Form,
    selected: &[String],
    max_height: f32,
    tr: &crate::i18n::Tr,
) -> Option<String> {
    let rows = object_rows(form);
    if rows.is_empty() {
        ui.label(
            RichText::new(tr.objects_empty)
                .color(crate::theme::active().text_dim)
                .small(),
        );
        return None;
    }

    let mut clicked = None;
    egui::ScrollArea::vertical()
        .id_salt("designer_objects_scroll")
        .max_height(max_height)
        .show(ui, |ui| {
            for row in &rows {
                if object_row(ui, row, selected.iter().any(|s| s == &row.id)) {
                    clicked = Some(row.id.clone());
                }
            }
        });
    clicked
}

/// One clickable row: indent, type glyph, id. Returns true when clicked.
fn object_row(ui: &mut Ui, row: &ObjectRow, is_selected: bool) -> bool {
    const ICON: f32 = 14.0;
    const INDENT: f32 = 10.0;

    let theme = crate::theme::active();
    let resp = ui
        .horizontal(|ui| {
            ui.add_space(2.0 + row.depth as f32 * INDENT);
            // The same glyphs the toolbox paints, so a row reads as the control
            // the developer dragged out of it.
            let (icon_rect, _) = ui.allocate_exact_size(Vec2::splat(ICON), Sense::hover());
            if ui.is_rect_visible(icon_rect) {
                super::toolbox::paint_control_icon(
                    ui.painter(),
                    icon_rect,
                    row.control_type.clone(),
                    if is_selected {
                        theme.accent
                    } else {
                        theme.text_dim
                    },
                );
            }
            ui.selectable_label(
                is_selected,
                RichText::new(&row.id).color(if is_selected {
                    theme.accent
                } else {
                    theme.text_bright
                }),
            )
        })
        .inner;

    resp.on_hover_text(format!("{} — {}", row.id, row.control_type.as_str()))
        .clicked()
}

#[cfg(test)]
mod tests {
    use super::*;
    use cobolt_forms::model::Control;

    fn ctrl(id: &str, ct: ControlType, parent: Option<&str>) -> Control {
        let mut c = Control::new(id.to_owned(), ct, 0, 0);
        c.parent = parent.map(|p| p.to_owned());
        c
    }

    fn form_with(controls: Vec<Control>) -> Form {
        let mut f = Form::new("F".to_owned(), "F", 640, 480);
        f.controls = controls;
        f
    }

    #[test]
    fn a_flat_form_lists_every_control_at_depth_zero_in_form_order() {
        let form = form_with(vec![
            ctrl("BTN-1", ControlType::Button, None),
            ctrl("LBL-1", ControlType::Label, None),
            ctrl("TXT-1", ControlType::TextBox, None),
        ]);

        let rows = object_rows(&form);

        assert_eq!(
            rows.iter().map(|r| r.id.as_str()).collect::<Vec<_>>(),
            ["BTN-1", "LBL-1", "TXT-1"]
        );
        assert!(rows.iter().all(|r| r.depth == 0));
    }

    #[test]
    fn a_child_follows_its_container_and_is_indented_one_level_per_ancestor() {
        // GRP-1 { PNL-1 { BTN-2 } }, plus a sibling after the group.
        let form = form_with(vec![
            ctrl("GRP-1", ControlType::GroupBox, None),
            ctrl("LBL-9", ControlType::Label, None),
            ctrl("PNL-1", ControlType::Panel, Some("GRP-1")),
            ctrl("BTN-2", ControlType::Button, Some("PNL-1")),
        ]);

        let rows = object_rows(&form);

        assert_eq!(
            rows.iter()
                .map(|r| (r.id.as_str(), r.depth))
                .collect::<Vec<_>>(),
            [("GRP-1", 0), ("PNL-1", 1), ("BTN-2", 2), ("LBL-9", 0)]
        );
    }

    #[test]
    fn a_control_whose_parent_is_gone_is_still_listed_rather_than_dropped() {
        // The container was deleted but the link was left behind. The control
        // must stay reachable — an unlistable control cannot be repaired.
        let form = form_with(vec![
            ctrl("BTN-1", ControlType::Button, None),
            ctrl("ORPHAN", ControlType::Label, Some("GONE-1")),
        ]);

        let rows = object_rows(&form);

        assert_eq!(
            rows.iter()
                .map(|r| (r.id.as_str(), r.depth))
                .collect::<Vec<_>>(),
            [("BTN-1", 0), ("ORPHAN", 0)]
        );
    }

    #[test]
    fn a_parent_cycle_lists_each_control_once_instead_of_hanging() {
        // Only reachable from a hand-edited `.cfrm`; the walk must terminate.
        let form = form_with(vec![
            ctrl("A", ControlType::Panel, Some("B")),
            ctrl("B", ControlType::Panel, Some("A")),
            ctrl("C", ControlType::Button, None),
        ]);

        let rows = object_rows(&form);

        // A and B are each other's parent, so neither is a root: only the
        // genuine root is listed, and the walk returns.
        assert_eq!(
            rows.iter().map(|r| r.id.as_str()).collect::<Vec<_>>(),
            ["C"]
        );
    }

    #[test]
    fn every_control_appears_exactly_once_however_deep_the_nesting_runs() {
        let form = form_with(vec![
            ctrl("G1", ControlType::GroupBox, None),
            ctrl("G2", ControlType::GroupBox, Some("G1")),
            ctrl("G3", ControlType::GroupBox, Some("G2")),
            ctrl("B1", ControlType::Button, Some("G3")),
            ctrl("B2", ControlType::Button, Some("G1")),
            ctrl("B3", ControlType::Button, None),
        ]);

        let rows = object_rows(&form);

        assert_eq!(rows.len(), form.controls.len());
        let mut ids: Vec<&str> = rows.iter().map(|r| r.id.as_str()).collect();
        ids.sort_unstable();
        assert_eq!(ids, ["B1", "B2", "B3", "G1", "G2", "G3"]);
        assert_eq!(rows.iter().find(|r| r.id == "B1").unwrap().depth, 3);
    }
}
