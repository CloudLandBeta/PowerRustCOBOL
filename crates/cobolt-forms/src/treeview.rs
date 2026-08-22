// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! The TreeView's layout and paint — **one implementation**, for the designer
//! canvas and the running form alike.
//!
//! Before this the two disagreed completely: the canvas drew the placeholder
//! caption `🌲 [TreeView]` and no nodes at all, while the running form drew a
//! flat bulleted list in a fixed 12pt font. Everything the inspector offered —
//! `ShowLines`, `ShowRootLines`, `LineColor`, `CheckBoxes`, `Sorted`,
//! `HotTracking` — reached neither: eight rows, and not one of them changed a
//! pixel (operator, 2026-08-22: "treeview not working / content not rendered").
//!
//! `Items` is the tree, one node per line, **two spaces per level**:
//!
//! ```text
//! Node 1
//!   Child 1
//!     Grandchild
//! Node 2
//! ```
//!
//! A row's index is its line's position in `Items` as WRITTEN — never its
//! position after sorting — so an event names the node the developer wrote and
//! `Sorted` cannot renumber anybody's handler.

use crate::model::Control;
use egui::{Color32, Pos2, Rect, Stroke, Vec2};

/// Row pitch, and where the first row's centre sits below the top edge. Both
/// are what the running form already used, so an existing tree does not move.
pub const ROW_H: f32 = 18.0;
const FIRST_ROW_Y: f32 = 12.0;
/// One level of indent.
pub const INDENT: f32 = 16.0;
/// The check box a node carries when `CheckBoxes` is on.
const CHECK: f32 = 12.0;

/// One laid-out node.
#[derive(Clone, Debug, PartialEq)]
pub struct TreeRow {
    /// Position of this node's line in `Items` as written — the id an event
    /// carries, unchanged by `Sorted`.
    pub index: usize,
    pub depth: usize,
    pub text: String,
    /// The full-width band: what a click, a selection and a hot-track use.
    pub rect: Rect,
    /// Where the label starts.
    pub label_x: f32,
    /// The check box, when `CheckBoxes` is on.
    pub check: Option<Rect>,
}

/// `Items` parsed into `(index, depth, text)`, blank lines dropped.
fn parse(items: &str) -> Vec<(usize, usize, String)> {
    items
        .lines()
        .enumerate()
        .filter_map(|(i, line)| {
            let text = line.trim();
            if text.is_empty() {
                return None;
            }
            // Two spaces per level, the shape the inspector's own hint teaches.
            // A tab counts as one level so a developer who typed one is not
            // punished for it.
            let lead = line.len() - line.trim_start().len();
            let tabs = line.chars().take_while(|c| *c == '\t').count();
            let depth = if tabs > 0 { tabs } else { lead / 2 };
            Some((i, depth, text.to_owned()))
        })
        .collect()
}

/// Sort SIBLINGS, leaving every node under the parent it was written under.
///
/// A flat `sort_by` would tear children away from their parents — the nodes
/// would be in order and the tree would be nonsense.
fn sort_siblings(nodes: Vec<(usize, usize, String)>) -> Vec<(usize, usize, String)> {
    // Each node's own subtree is the run of following nodes deeper than it.
    fn walk(nodes: &[(usize, usize, String)], depth: usize) -> Vec<(usize, usize, String)> {
        let mut groups: Vec<Vec<(usize, usize, String)>> = Vec::new();
        for node in nodes {
            if node.1 <= depth || groups.is_empty() {
                groups.push(vec![node.clone()]);
            } else {
                groups.last_mut().expect("just pushed").push(node.clone());
            }
        }
        groups.sort_by(|a, b| a[0].2.to_lowercase().cmp(&b[0].2.to_lowercase()));
        let mut out = Vec::new();
        for g in groups {
            out.push(g[0].clone());
            if g.len() > 1 {
                out.extend(walk(&g[1..], depth + 1));
            }
        }
        out
    }
    let base = nodes.first().map(|n| n.1).unwrap_or(0);
    walk(&nodes, base)
}

fn flag(ctrl: &Control, key: &str, default: bool) -> bool {
    ctrl.get_prop(key).map(|v| v.as_bool()).unwrap_or(default)
}

/// Lay the tree out inside `rect`. Rows past the bottom edge are dropped: a
/// node drawn outside its own control is worse than one the operator scrolls
/// to, and the control does not scroll yet.
pub fn layout(ctrl: &Control, rect: Rect) -> Vec<TreeRow> {
    let items = ctrl
        .get_prop("Items")
        .map(|v| v.as_str().to_owned())
        .unwrap_or_default();
    let mut nodes = parse(&items);
    if flag(ctrl, "Sorted", false) {
        nodes = sort_siblings(nodes);
    }
    let checks = flag(ctrl, "CheckBoxes", false);
    let mut y = rect.min.y + FIRST_ROW_Y;
    let mut rows = Vec::new();
    for (index, depth, text) in nodes {
        if y + ROW_H * 0.5 > rect.max.y {
            break;
        }
        let band = Rect::from_min_max(
            Pos2::new(rect.min.x + 2.0, y - ROW_H * 0.5),
            Pos2::new(rect.max.x - 2.0, y + ROW_H * 0.5),
        );
        let indent_x = rect.min.x + 8.0 + depth as f32 * INDENT;
        let check = checks.then(|| {
            Rect::from_center_size(
                Pos2::new(indent_x + CHECK * 0.5, y),
                Vec2::splat(CHECK),
            )
        });
        rows.push(TreeRow {
            index,
            depth,
            text,
            rect: band,
            label_x: match check {
                Some(c) => c.max.x + 6.0,
                None => indent_x,
            },
            check,
        });
        y += ROW_H;
    }
    rows
}

/// What the tree looks like right now, beyond its designed properties.
#[derive(Default, Clone, Copy)]
pub struct TreeState<'a> {
    /// `SelectedNode` — the node whose band is highlighted.
    pub selected: &'a str,
    /// `CheckedNodes`, one per line — which boxes are ticked.
    pub checked: &'a [String],
    /// Row under the pointer, for `HotTracking`. The canvas passes `None`: a
    /// design surface has no pointer of its own.
    pub hovered: Option<usize>,
    /// Fade from the render walk (a faded container dims its subtree).
    pub alpha: f32,
}

/// Paint the rows into `rect`. The FACE is the caller's business — both call
/// sites draw the control's designed face first, so this adds only the tree.
pub fn paint(
    painter: &egui::Painter,
    ctrl: &Control,
    rect: Rect,
    rows: &[TreeRow],
    state: TreeState<'_>,
) {
    let a = state.alpha.clamp(0.0, 1.0);
    let fade = |c: Color32| {
        Color32::from_rgba_premultiplied(c.r(), c.g(), c.b(), (c.a() as f32 * a) as u8)
    };
    let ink = crate::paint::treeview_ink(painter.ctx(), ctrl);
    let line_color = ctrl
        .get_prop("LineColor")
        .map(|v| crate::paint::parse_color(v.as_str()))
        .filter(|c| c.a() > 0)
        .unwrap_or(Color32::from_gray(170));
    let show_lines = flag(ctrl, "ShowLines", true);
    let show_root = flag(ctrl, "ShowRootLines", true);

    // ── The tree's own lines ────────────────────────────────────────────────
    //
    // Drawn from the ROWS rather than from the text, so what is joined is
    // exactly what is drawn. A node's elbow reaches left to its parent's
    // indent column; the verticals run from a parent down to its last child.
    if show_lines {
        let stroke = Stroke::new(1.0, fade(line_color));
        for (i, row) in rows.iter().enumerate() {
            if row.depth == 0 {
                continue; // a root has no parent to join
            }
            let x = rect.min.x + 8.0 + (row.depth - 1) as f32 * INDENT + INDENT * 0.5;
            let y = row.rect.center().y;
            let left = row.check.map(|c| c.min.x).unwrap_or(row.label_x) - 4.0;
            painter.line_segment([Pos2::new(x, y), Pos2::new(left, y)], stroke);
            // The vertical to the previous sibling (or the parent) at this
            // depth — walk back until something at this depth or shallower.
            let mut top = row.rect.min.y;
            for prev in rows[..i].iter().rev() {
                if prev.depth < row.depth {
                    top = prev.rect.center().y;
                    break;
                }
                if prev.depth == row.depth {
                    top = prev.rect.center().y;
                    break;
                }
            }
            painter.line_segment([Pos2::new(x, top), Pos2::new(x, y)], stroke);
        }
    }
    // The root spine: one vertical joining the top-level nodes, which is what
    // `ShowRootLines` means and why it is separate from `ShowLines`.
    if show_root {
        let roots: Vec<&TreeRow> = rows.iter().filter(|r| r.depth == 0).collect();
        if roots.len() > 1 {
            let x = rect.min.x + 8.0 - 4.0;
            let stroke = Stroke::new(1.0, fade(line_color));
            painter.line_segment([
                Pos2::new(x, roots[0].rect.center().y),
                Pos2::new(x, roots[roots.len() - 1].rect.center().y),
            ], stroke);
            for r in roots {
                painter.line_segment(
                    [
                        Pos2::new(x, r.rect.center().y),
                        Pos2::new(r.check.map(|c| c.min.x).unwrap_or(r.label_x) - 4.0, r.rect.center().y),
                    ],
                    stroke,
                );
            }
        }
    }

    // ── Bands, boxes and labels ─────────────────────────────────────────────
    let focus = crate::paint::theme_token(painter.ctx(), crate::surface_theme::ColorToken::Focus);
    let font = crate::fonts::font_id(
        painter.ctx(),
        &ctrl
            .get_prop("FontName")
            .map(|v| v.as_str().to_owned())
            .unwrap_or_default(),
        crate::paint::ctrl_font_size(ctrl),
    );
    for row in rows {
        let selected = !state.selected.is_empty() && state.selected == row.text;
        if selected {
            let c = focus.unwrap_or(Color32::from_rgb(70, 110, 200));
            painter.rect_filled(
                row.rect,
                3.0,
                fade(Color32::from_rgba_unmultiplied(c.r(), c.g(), c.b(), 70)),
            );
        } else if state.hovered == Some(row.index) {
            // HotTracking: the row under the pointer lifts, faintly — half the
            // selection's weight, so the two are never confused.
            painter.rect_filled(row.rect, 3.0, fade(Color32::from_white_alpha(18)));
        }
        if let Some(box_rect) = row.check {
            let ticked = state.checked.iter().any(|c| c == &row.text);
            painter.rect_filled(box_rect, 2.0, fade(Color32::from_black_alpha(40)));
            painter.rect_stroke(
                box_rect,
                2.0,
                Stroke::new(1.0, fade(ink)),
                egui::StrokeKind::Inside,
            );
            if ticked {
                let c = box_rect.center();
                let s = box_rect.width() * 0.28;
                let stroke = Stroke::new(2.0, fade(ink));
                painter.line_segment(
                    [Pos2::new(c.x - s, c.y), Pos2::new(c.x - s * 0.2, c.y + s)],
                    stroke,
                );
                painter.line_segment(
                    [Pos2::new(c.x - s * 0.2, c.y + s), Pos2::new(c.x + s, c.y - s)],
                    stroke,
                );
            }
        }
        painter.text(
            Pos2::new(row.label_x, row.rect.center().y),
            egui::Align2::LEFT_CENTER,
            &row.text,
            font.clone(),
            fade(ink),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ControlType, PropValue};

    fn tree(items: &str) -> Control {
        let mut c = Control::new("TV-1", ControlType::TreeView, 0, 0);
        c.rect = crate::model::Rect::new(0, 0, 200, 160);
        c.set_prop("Items", PropValue::String(items.into()));
        c
    }

    fn rect() -> Rect {
        Rect::from_min_size(Pos2::ZERO, Vec2::new(200.0, 160.0))
    }

    /// Indentation IS the tree: two spaces (or one tab) per level, and a node's
    /// row is indented by its depth.
    #[test]
    fn two_spaces_make_a_child() {
        let rows = layout(&tree("Root\n  Child\n    Grandchild\nOther"), rect());
        assert_eq!(
            rows.iter().map(|r| r.depth).collect::<Vec<_>>(),
            vec![0, 1, 2, 0]
        );
        assert!(
            rows[1].label_x > rows[0].label_x && rows[2].label_x > rows[1].label_x,
            "each level must sit further right: {:?}",
            rows.iter().map(|r| r.label_x).collect::<Vec<_>>()
        );
    }

    /// **`Sorted` orders SIBLINGS**, and leaves every child under the parent it
    /// was written under. A flat sort would put the nodes in order and the tree
    /// in ruins.
    #[test]
    fn sorted_orders_siblings_without_stealing_children() {
        let mut c = tree("Zebra\n  zulu\n  alpha\nApple\n  pear");
        c.set_prop("Sorted", PropValue::Bool(true));
        let rows = layout(&c, rect());
        assert_eq!(
            rows.iter().map(|r| r.text.as_str()).collect::<Vec<_>>(),
            vec!["Apple", "pear", "Zebra", "alpha", "zulu"],
            "roots sorted, and each child still under its own parent"
        );
        // The index is the line as WRITTEN, so an event names the node the
        // developer wrote whatever the sort did.
        let apple = rows.iter().find(|r| r.text == "Apple").expect("Apple");
        assert_eq!(apple.index, 3, "Apple is the fourth line of Items");
    }

    /// `CheckBoxes` gives every node a box, and the label steps aside for it.
    #[test]
    fn check_boxes_take_their_own_room() {
        let plain = layout(&tree("A\nB"), rect());
        let mut c = tree("A\nB");
        c.set_prop("CheckBoxes", PropValue::Bool(true));
        let checked = layout(&c, rect());
        assert!(plain[0].check.is_none());
        let box_rect = checked[0].check.expect("a box per node");
        assert!(
            checked[0].label_x > box_rect.max.x,
            "the label must clear its own box"
        );
        assert!(
            checked[0].label_x > plain[0].label_x,
            "…which moves it right of where it sits without one"
        );
    }

    /// A tree taller than its control stops at the bottom edge rather than
    /// painting nodes outside the control the developer sized.
    #[test]
    fn rows_stop_at_the_bottom_edge() {
        let items = (1..=40)
            .map(|i| format!("Node {i}"))
            .collect::<Vec<_>>()
            .join("\n");
        let rows = layout(&tree(&items), rect());
        assert!(rows.len() < 40, "40 rows cannot fit 160pt");
        assert!(
            rows.iter().all(|r| r.rect.max.y <= rect().max.y + ROW_H),
            "no row may hang below the control"
        );
    }
}
