// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Container/containment tree logic (spec 012), shared by the unified render
//! engine (spec 017) and the Form Designer.
//!
//! Controls live in one **flat** `form.controls` list; nesting is derived from
//! each control's `parent` link (and `tab` for `TabControl` pages). These pure
//! helpers compute draw order, per-control visibility (active-tab aware), the
//! clip rectangle (intersection of ancestor container content areas), the
//! composed ancestor opacity, drop-target resolution for reparenting, and the
//! cascade/cycle sets — all without an egui context, so they are unit-testable
//! and usable regardless of the `render` feature.

use std::collections::HashMap;

use crate::model::Rect;
use crate::{Control, ControlType};

/// Active tab page (from 1, like `SelectedTab`) per `TabControl` id, used to
/// hide controls that belong to a non-selected tab.
pub type ActiveTabs = HashMap<String, u32>;

/// Where a dragged control should be parented after a drop (spec 012 R7–R10).
#[derive(Debug, Clone, PartialEq)]
pub enum DropTarget {
    /// Directly on the form (no container).
    Form,
    /// Inside a container's content area; `tab` set when the container is a
    /// `TabControl` (the active page).
    Into { container: String, tab: Option<u32> },
}

/// A control's `Opacity` (0–100) as a 0.0–1.0 multiplier (default 1.0). Inlined
/// here so the containers logic stays free of the `render` feature.
fn opacity_of(ctrl: &Control) -> f32 {
    crate::model::alpha_multiplier(ctrl)
}

fn index_of(controls: &[Control], id: &str) -> Option<usize> {
    controls.iter().position(|c| c.id == id)
}

/// Indices of the direct children of `parent_id` (`None` = form roots), sorted by
/// `z_order` (ascending = drawn first / underneath).
fn children_sorted(controls: &[Control], parent_id: Option<&str>) -> Vec<usize> {
    let mut kids: Vec<usize> = controls
        .iter()
        .enumerate()
        .filter(|(_, c)| c.parent.as_deref() == parent_id)
        .map(|(i, _)| i)
        .collect();
    kids.sort_by_key(|&i| controls[i].z_order);
    kids
}

/// Pre-order draw list: a parent appears before its children, and siblings are
/// ordered by `z_order`, so children paint on top of their container.
pub fn render_order(controls: &[Control]) -> Vec<usize> {
    let mut out = Vec::with_capacity(controls.len());
    fn rec(controls: &[Control], parent: Option<&str>, out: &mut Vec<usize>) {
        for idx in children_sorted(controls, parent) {
            out.push(idx);
            rec(controls, Some(controls[idx].id.as_str()), out);
        }
    }
    rec(controls, None, &mut out);
    // Any control whose `parent` points at a missing id is orphaned — surface it
    // at the form level rather than dropping it.
    for (i, c) in controls.iter().enumerate() {
        if !out.contains(&i) {
            let _ = c;
            out.push(i);
        }
    }
    out
}

/// `true` if `idx` is inside `ancestor` (its parent chain reaches `ancestor`).
pub fn is_descendant(controls: &[Control], idx: usize, ancestor: usize) -> bool {
    let anc_id = controls[ancestor].id.as_str();
    let mut cur = idx;
    while let Some(pid) = controls[cur].parent.clone() {
        if pid == anc_id {
            return true;
        }
        match index_of(controls, &pid) {
            Some(p) => cur = p,
            None => break,
        }
    }
    false
}

/// All descendant indices of `idx` (its whole subtree, excluding `idx`).
pub fn collect_descendants(controls: &[Control], idx: usize) -> Vec<usize> {
    (0..controls.len())
        .filter(|&i| i != idx && is_descendant(controls, i, idx))
        .collect()
}

/// `true` when `idx` owns at least one descendant control.
pub fn has_descendants(controls: &[Control], idx: usize) -> bool {
    controls
        .iter()
        .enumerate()
        .any(|(child_idx, _)| child_idx != idx && is_descendant(controls, child_idx, idx))
}

/// `true` unless some ANCESTOR hides this control: a container that is itself
/// hidden, or a `TabControl` showing a different page than the branch this
/// control sits on.
///
/// `ancestor_shown` answers "is this container visible right now?" for each
/// ancestor in turn — the live state on a running surface, and a constant
/// `true` where there is none (the designer canvas paints a hidden control
/// anyway, or it could never be selected to be shown again).
///
/// It is a PARAMETER rather than a lookup because the two things that hide a
/// container live in different places: the designed `visible` flag travels with
/// the control, and `SET Group-1::Visible TO 0` lives in the interpreter's
/// state. Callers with state must pass it; the signature is what makes them
/// decide, because this function used to ask about tabs and nothing else — so
/// a hidden GroupBox went on painting everything inside it, its children not
/// members of the group as far as visibility was concerned (operator,
/// 2026-09-09: "Hiding a Groupbox does not hide its children").
pub fn is_visible(
    controls: &[Control],
    idx: usize,
    active: &ActiveTabs,
    ancestor_shown: &dyn Fn(&Control) -> bool,
) -> bool {
    let mut cur = idx;
    while let Some(pid) = controls[cur].parent.clone() {
        let Some(p) = index_of(controls, &pid) else {
            break;
        };
        // A container that is not on screen has no inside to be on screen in.
        // Both the designed flag and the live answer, because a form can be
        // saved with a hidden group AND hide one while it runs.
        if !controls[p].visible || !ancestor_shown(&controls[p]) {
            return false;
        }
        if controls[p].control_type == ControlType::TabControl {
            let act = active.get(&pid).copied().unwrap_or_else(|| {
                controls[p]
                    .get_prop("SelectedTab")
                    .map(|v| v.as_i64().max(1) as u32)
                    .unwrap_or(1)
            });
            if controls[cur].tab.unwrap_or(1) != act {
                return false;
            }
        }
        cur = p;
    }
    true
}

/// `true` unless some ANCESTOR disables this control.
///
/// The sibling of [`is_visible`], and the same walk for the same reason: a
/// container that is switched off switches off what is inside it. A GroupBox,
/// Panel, Splitter or TabControl with `Enabled = 0` looked disabled and every
/// control inside it went on taking clicks, because `enabled` was only ever
/// asked of the control itself — eight times over in the renderer, never once
/// of its ancestors (operator, 2026-09-09).
///
/// `ancestor_enabled` answers "is this container switched on right now?" for
/// each ancestor in turn: the live state on a running surface, a constant
/// `true` where there is none.
///
/// Nothing is WRITTEN to the children. Switching a container back on therefore
/// restores each child to its own `Enabled` — a control the developer disabled
/// individually stays disabled, which is what every RAD tool does and the only
/// behaviour that does not quietly destroy the developer's setting.
///
/// Tabs are deliberately not consulted here: an unselected page is not drawn at
/// all, so there is nothing on it to enable or disable.
pub fn is_enabled(
    controls: &[Control],
    idx: usize,
    ancestor_enabled: &dyn Fn(&Control) -> bool,
) -> bool {
    let mut cur = idx;
    while let Some(pid) = controls[cur].parent.clone() {
        let Some(p) = index_of(controls, &pid) else {
            break;
        };
        if !controls[p].enabled || !ancestor_enabled(&controls[p]) {
            return false;
        }
        cur = p;
    }
    true
}

fn intersect(a: Rect, b: Rect) -> Rect {
    let x0 = a.x.max(b.x);
    let y0 = a.y.max(b.y);
    let x1 = (a.x + a.w).min(b.x + b.w);
    let y1 = (a.y + a.h).min(b.y + b.h);
    Rect::new(x0, y0, (x1 - x0).max(0), (y1 - y0).max(0))
}

/// The clip rectangle (form-space) a control is confined to: the intersection of
/// every ancestor container's `content_rect`. `None` when the control has no
/// container ancestor (clip = the whole form).
pub fn clip_rect(controls: &[Control], idx: usize) -> Option<Rect> {
    let mut clip: Option<Rect> = None;
    let mut cur = idx;
    while let Some(pid) = controls[cur].parent.clone() {
        let Some(p) = index_of(controls, &pid) else {
            break;
        };
        let cr = controls[p].content_rect();
        clip = Some(match clip {
            Some(c) => intersect(c, cr),
            None => cr,
        });
        cur = p;
    }
    clip
}

/// Product of the opacities (0.0–1.0) of all ancestor containers — what a child's
/// `alpha_mul` should start from so a faded container dims its subtree.
/// GroupBox opacity applies only to its own frame (border/caption), never to the
/// controls placed inside it, so GroupBox ancestors are skipped here.
///
/// A **Splitter pane** is skipped for the same reason, and a stronger one: the
/// developer does not own its `Transparency` in the first place — the pane is
/// a layout region the splitter creates, and a transparent one must show what
/// is behind it, not erase what is inside it. Skipping the pane also repairs a
/// form saved by 1.61.164, whose panes carry the `Transparency = 100` that
/// blanked their contents.
pub fn ancestor_opacity(controls: &[Control], idx: usize) -> f32 {
    let mut o = 1.0_f32;
    let mut cur = idx;
    while let Some(pid) = controls[cur].parent.clone() {
        let Some(p) = index_of(controls, &pid) else {
            break;
        };
        let frame_only = matches!(controls[p].control_type, crate::ControlType::GroupBox)
            || crate::splitter::pane_index(&controls[p]).is_some();
        if !frame_only {
            o *= opacity_of(&controls[p]);
        }
        cur = p;
    }
    o
}

/// Resolve where a control dropped at form-space `(px, py)` should be parented
/// (spec 012 R7–R10). `dragged` is the index of the control being moved (it and
/// its descendants are never valid targets — cycle guard).
///
/// Rules, innermost/topmost first:
/// * a **StatusBar** always belongs to the form — see below;
/// * over a **container's content area** → `Into` that container (R8); for a
///   `TabControl`, the active page's `tab` (R9 — chrome / inactive pages are not
///   content and are skipped);
/// * over a **non-container control** → the same parent as that control (R10);
/// * otherwise → the form (R7).
pub fn resolve_drop_target(
    controls: &[Control],
    px: i32,
    py: i32,
    dragged: usize,
    active: &ActiveTabs,
) -> DropTarget {
    // A status bar reports on the window, so it is never a child of anything
    // (operator, 2026-09-09). Refusing it HERE rather than at the drop is what
    // makes the drop hint agree with the drop: the hint asks this same
    // question, so no container ever lights up for a status bar being dragged
    // over it.
    if controls
        .get(dragged)
        .is_some_and(|c| c.control_type == ControlType::StatusBar)
    {
        return DropTarget::Form;
    }
    for &idx in render_order(controls).iter().rev() {
        if idx == dragged || is_descendant(controls, idx, dragged) {
            continue;
        }
        // The canvas: a control hidden by the DESIGN is still a legal
        // drop target, so this asks only the tab question.
        if !is_visible(controls, idx, active, &|_| true) {
            continue;
        }
        // Must be inside the control's own clip (ancestor content areas).
        if let Some(clip) = clip_rect(controls, idx) {
            if !clip.contains(px, py) {
                continue;
            }
        }
        let c = &controls[idx];
        if !c.rect.contains(px, py) {
            continue;
        }
        if c.is_container() {
            // Valid drop only over the visible content area (R9). Over chrome
            // (caption / tab strip / border) keep searching for an outer target.
            let content_hit = c.content_rect().contains(px, py);
            if content_hit {
                let tab = if c.control_type == ControlType::TabControl {
                    Some(active.get(&c.id).copied().unwrap_or_else(|| {
                        c.get_prop("SelectedTab")
                            .map(|v| v.as_i64().max(1) as u32)
                            .unwrap_or(1)
                    }))
                } else {
                    None
                };
                return DropTarget::Into {
                    container: c.id.clone(),
                    tab,
                };
            }
            continue;
        }
        // Non-container: adopt its parent (R10). Parent-less ⇒ the form.
        return match &c.parent {
            Some(pid) => DropTarget::Into {
                container: pid.clone(),
                tab: c.tab,
            },
            None => DropTarget::Form,
        };
    }
    DropTarget::Form
}

/// What a TabControl's contents need when its tab strip moves to another side:
/// the controls to move (`id, old x, old y, new x, new y`) and, when the
/// content no longer fits, the TabControl's new `(w, h)`.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct TabReflow {
    pub moves: Vec<(String, i32, i32, i32, i32)>,
    pub size: Option<(i32, i32)>,
}

/// Lay a TabControl's contents out for its strip on `position` (`Top`,
/// `Bottom`, `Left`, `Right`).
///
/// Coordinates are form-absolute, so moving the strip moves the page out from
/// under the controls on it: a strip turned to the left leaves them sitting
/// under the tabs (operator, 2026-09-27). Three steps, the content moving as
/// one block so its own layout is never disturbed:
///
/// 1. every descendant keeps its offset from the page's corner;
/// 2. anything still above or left of the page (placed before the strip moved)
///    brings the whole block in by as much as it needs;
/// 3. content past the page's far edge grows the TabControl by the overflow
///    plus the margin the content keeps on the near side, so it sits inside
///    the page with the same room on both sides.
pub fn reflow_for_tab_position(controls: &[Control], tab_idx: usize, position: &str) -> TabReflow {
    let Some(tab) = controls.get(tab_idx).filter(|c| c.control_type == ControlType::TabControl) else {
        return TabReflow::default();
    };
    let descendants = collect_descendants(controls, tab_idx);
    if descendants.is_empty() {
        return TabReflow::default();
    }
    let old_page = tab.content_rect();
    let mut moved = tab.clone();
    moved.set_prop("TabPosition", crate::PropValue::String(position.to_owned()));
    let page = moved.content_rect();
    let children: Vec<&Control> = controls
        .iter()
        .filter(|c| c.parent.as_deref().is_some_and(|p| p.eq_ignore_ascii_case(&tab.id)))
        .collect();
    // 1. The same offset from the page's corner.
    let (mut dx, mut dy) = (page.x - old_page.x, page.y - old_page.y);
    // 2. Nothing before the page's near edges.
    let min_x = children.iter().map(|c| c.rect.x + dx).min().unwrap_or(page.x);
    let min_y = children.iter().map(|c| c.rect.y + dy).min().unwrap_or(page.y);
    dx += (page.x - min_x).max(0);
    dy += (page.y - min_y).max(0);
    // 3. Room for what reaches past the far edges.
    let margin_x = (children.iter().map(|c| c.rect.x + dx).min().unwrap_or(page.x) - page.x).clamp(0, 24);
    let margin_y = (children.iter().map(|c| c.rect.y + dy).min().unwrap_or(page.y) - page.y).clamp(0, 24);
    let max_x = children.iter().map(|c| c.rect.x + dx + c.rect.w).max().unwrap_or(0);
    let max_y = children.iter().map(|c| c.rect.y + dy + c.rect.h).max().unwrap_or(0);
    let grow_w = (max_x + margin_x - (page.x + page.w)).max(0);
    let grow_h = (max_y + margin_y - (page.y + page.h)).max(0);
    TabReflow {
        moves: if dx == 0 && dy == 0 {
            Vec::new()
        } else {
            descendants
                .iter()
                .map(|&i| {
                    let r = controls[i].rect;
                    (controls[i].id.clone(), r.x, r.y, r.x + dx, r.y + dy)
                })
                .collect()
        },
        size: (grow_w > 0 || grow_h > 0).then(|| (tab.rect.w + grow_w, tab.rect.h + grow_h)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::PropValue;

    fn ctrl(
        id: &str,
        t: ControlType,
        x: i32,
        y: i32,
        w: i32,
        h: i32,
        parent: Option<&str>,
    ) -> Control {
        let mut c = Control::new(id, t, x, y);
        c.rect.w = w;
        c.rect.h = h;
        c.parent = parent.map(|s| s.to_string());
        c
    }

    // Panel(0,0,300,300) ⊃ Button(inside), plus a top-level Label.
    fn sample() -> Vec<Control> {
        vec![
            ctrl("Pnl", ControlType::Panel, 0, 0, 300, 300, None),
            ctrl("Btn", ControlType::Button, 50, 50, 80, 24, Some("Pnl")),
            ctrl("Lbl", ControlType::Label, 400, 10, 80, 20, None),
        ]
    }

    #[test]
    fn render_order_parent_before_child() {
        let c = sample();
        let order = render_order(&c);
        let pos = |id: &str| order.iter().position(|&i| c[i].id == id).unwrap();
        assert!(pos("Pnl") < pos("Btn"), "container draws before its child");
        assert_eq!(order.len(), 3);
    }

    #[test]
    fn descendant_and_cascade() {
        let c = sample();
        assert!(is_descendant(&c, 1, 0)); // Btn inside Pnl
        assert!(!is_descendant(&c, 2, 0)); // Lbl not inside Pnl
        assert_eq!(collect_descendants(&c, 0), vec![1]);
        assert!(has_descendants(&c, 0));
        assert!(!has_descendants(&c, 1));
        assert!(!has_descendants(&c, 2));
    }

    #[test]
    fn clip_and_opacity_compose() {
        let mut c = sample();
        let clip = clip_rect(&c, 1).expect("child has a clip");
        assert_eq!(clip, c[0].content_rect());
        assert!(clip_rect(&c, 2).is_none(), "top-level control has no clip");
        c[0].set_prop("Transparency", PropValue::Int(50));
        assert!((ancestor_opacity(&c, 1) - 0.5).abs() < 1e-6);
        assert_eq!(ancestor_opacity(&c, 2), 1.0);
    }

    #[test]
    fn tab_visibility() {
        let mut c = vec![
            ctrl("Tabs", ControlType::TabControl, 0, 0, 300, 200, None),
            ctrl("A", ControlType::Button, 10, 40, 60, 20, Some("Tabs")),
            ctrl("B", ControlType::Button, 10, 70, 60, 20, Some("Tabs")),
        ];
        // Pages count from 1.
        c[1].tab = Some(1);
        c[2].tab = Some(2);
        let mut active = ActiveTabs::new();
        active.insert("Tabs".into(), 1);
        assert!(is_visible(&c, 1, &active, &|_| true));
        assert!(!is_visible(&c, 2, &active, &|_| true));
        active.insert("Tabs".into(), 2);
        assert!(is_visible(&c, 2, &active, &|_| true));
    }

    #[test]
    fn drop_target_rules() {
        let c = sample();
        let active = ActiveTabs::new();
        assert_eq!(
            resolve_drop_target(&c, 100, 100, 2, &active),
            DropTarget::Into {
                container: "Pnl".into(),
                tab: None
            }
        );
        assert_eq!(
            resolve_drop_target(&c, 600, 400, 2, &active),
            DropTarget::Form
        );
        assert_eq!(
            resolve_drop_target(&c, 60, 58, 2, &active),
            DropTarget::Into {
                container: "Pnl".into(),
                tab: None
            }
        );
        assert_eq!(
            resolve_drop_target(&c, 100, 100, 0, &active),
            DropTarget::Form
        );
    }

    /// A status bar dropped straight onto a container still belongs to the
    /// form — the one control this is true of, and true wherever it is dropped
    /// (operator, 2026-09-09). The same question drives the drop HINT, so no
    /// container lights up for one either.
    #[test]
    fn a_status_bar_never_lands_in_a_container() {
        let mut c = vec![
            ctrl("Pnl", ControlType::Panel, 0, 0, 400, 300, None),
            ctrl("Tabs", ControlType::TabControl, 0, 0, 300, 200, None),
            ctrl("SB", ControlType::StatusBar, 20, 20, 200, 22, None),
        ];
        c[1].tab = Some(1);
        let mut active = ActiveTabs::new();
        active.insert("Tabs".into(), 1);

        // Over the panel's content, over the tab page's content, over both.
        for (x, y) in [(100, 100), (150, 150), (20, 250)] {
            assert_eq!(
                resolve_drop_target(&c, x, y, 2, &active),
                DropTarget::Form,
                "a status bar dropped at ({x}, {y}) must still belong to the form"
            );
        }

        // The rule is about the STATUS BAR, not the point: an ordinary control
        // dropped at the same place is still adopted.
        c.push(ctrl("Btn", ControlType::Button, 0, 0, 60, 20, None));
        assert_ne!(
            resolve_drop_target(&c, 100, 100, 3, &active),
            DropTarget::Form,
            "the refusal must not have disabled containment for everything else"
        );
    }

    #[test]
    fn drop_rejects_inactive_tab_and_chrome() {
        let mut c = vec![
            ctrl("Tabs", ControlType::TabControl, 0, 0, 300, 200, None),
            ctrl("A", ControlType::Button, 10, 40, 60, 20, Some("Tabs")),
        ];
        c[1].tab = Some(1);
        let mut active = ActiveTabs::new();
        active.insert("Tabs".into(), 1);
        assert_eq!(
            resolve_drop_target(&c, 100, 10, 1, &active),
            DropTarget::Form
        );
        // Inside the tab strip (26 px tall; the page starts right below it).
        assert_eq!(
            resolve_drop_target(&c, 100, 20, 1, &active),
            DropTarget::Form
        );
        assert_eq!(
            resolve_drop_target(&c, 150, 100, 1, &active),
            DropTarget::Into {
                container: "Tabs".into(),
                tab: Some(1)
            }
        );
    }

    /// The strip moves, the content follows: turned to the left, the page's
    /// contents move right by the strip's width and keep their layout; a
    /// block that then reaches past the page's right edge grows the
    /// TabControl; turned back to the top, they return (operator, 2026-09-27).
    #[test]
    fn the_contents_follow_the_tab_strip() {
        let mut tab = ctrl("Tabs", ControlType::TabControl, 20, 20, 500, 300, None);
        tab.set_prop("Tabs", crate::PropValue::String("Browse\nCreate/Update".into()));
        tab.set_prop("TabPosition", crate::PropValue::String("Top".into()));
        let page_top = tab.content_rect();
        let mut grid = ctrl("Grid", ControlType::DataGrid, page_top.x + 8, page_top.y + 40, 460, 150, Some("Tabs"));
        grid.tab = Some(1);
        let mut new_btn = ctrl("New", ControlType::Button, page_top.x + 380, page_top.y + 8, 88, 28, Some("Tabs"));
        new_btn.tab = Some(1);
        let controls = vec![tab.clone(), grid.clone(), new_btn.clone()];

        let r = reflow_for_tab_position(&controls, 0, "Left");
        let mut left = tab.clone();
        left.set_prop("TabPosition", crate::PropValue::String("Left".into()));
        let page = left.content_rect();
        let at = |id: &str| r.moves.iter().find(|m| m.0 == id).map(|m| (m.3, m.4)).expect(id);
        // Same offset from the page's corner, so the layout is kept.
        assert_eq!(at("Grid"), (page.x + 8, page.y + 40));
        assert_eq!(at("New").0 - at("Grid").0, 372, "the block moves as one");
        // 8 + 460 wide no longer fits beside the strip: the control grows by
        // the overflow plus the 8 px the grid keeps on its left.
        let (w, h) = r.size.expect("grows");
        assert_eq!(h, 300, "tall enough already");
        let mut grown = left.clone();
        grown.rect.w = w;
        let gp = grown.content_rect();
        assert_eq!(
            gp.x + gp.w,
            at("Grid").0 + 460 + 8,
            "grown just enough: the grid keeps 8 px on both sides"
        );

        // Content left under a side strip (placed before the strip moved) is
        // brought in: here the strip is already Left and the grid sits at the
        // old, top-strip position.
        let mut already = left.clone();
        already.rect.w = w;
        let stuck = vec![already, grid.clone(), new_btn.clone()];
        let r = reflow_for_tab_position(&stuck, 0, "Left");
        let gx = r.moves.iter().find(|m| m.0 == "Grid").map(|m| m.3).expect("moved in");
        assert!(gx >= gp.x, "no longer under the strip: {gx} vs page {}", gp.x);

        // A right strip: the content moves to the left of it.
        let r = reflow_for_tab_position(&controls, 0, "Right");
        let mut right = tab.clone();
        right.set_prop("TabPosition", crate::PropValue::String("Right".into()));
        if let Some((w, _)) = r.size {
            right.rect.w = w;
        }
        let rp = right.content_rect();
        let gx = r.moves.iter().find(|m| m.0 == "Grid").map(|m| m.3).expect("moved");
        assert!(gx >= rp.x && gx + 460 <= rp.x + rp.w, "inside the page left of the strip: {gx} {rp:?}");

        // No children, nothing to do.
        assert_eq!(reflow_for_tab_position(&[tab], 0, "Left"), TabReflow::default());
    }

}
