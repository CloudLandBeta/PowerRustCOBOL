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

/// Active tab page (0-based) per `TabControl` id, used to hide controls that
/// belong to a non-selected tab.
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
    ctrl.get_prop("Opacity")
        .map(|v| v.as_i64())
        .unwrap_or(100)
        .clamp(0, 100) as f32
        / 100.0
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

/// `true` unless an ancestor `TabControl` has a different page selected than the
/// branch this control sits on (an inactive tab hides its children).
pub fn is_visible(controls: &[Control], idx: usize, active: &ActiveTabs) -> bool {
    let mut cur = idx;
    while let Some(pid) = controls[cur].parent.clone() {
        let Some(p) = index_of(controls, &pid) else {
            break;
        };
        if controls[p].control_type == ControlType::TabControl {
            let act = active.get(&pid).copied().unwrap_or_else(|| {
                controls[p]
                    .get_prop("SelectedTab")
                    .map(|v| v.as_i64() as u32)
                    .unwrap_or(0)
            });
            if controls[cur].tab.unwrap_or(0) != act {
                return false;
            }
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
pub fn ancestor_opacity(controls: &[Control], idx: usize) -> f32 {
    let mut o = 1.0_f32;
    let mut cur = idx;
    while let Some(pid) = controls[cur].parent.clone() {
        let Some(p) = index_of(controls, &pid) else {
            break;
        };
        if !matches!(controls[p].control_type, crate::ControlType::GroupBox) {
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
    for &idx in render_order(controls).iter().rev() {
        if idx == dragged || is_descendant(controls, idx, dragged) {
            continue;
        }
        if !is_visible(controls, idx, active) {
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
                            .map(|v| v.as_i64() as u32)
                            .unwrap_or(0)
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
        c[0].set_prop("Opacity", PropValue::Int(50));
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
        c[1].tab = Some(0);
        c[2].tab = Some(1);
        let mut active = ActiveTabs::new();
        active.insert("Tabs".into(), 0);
        assert!(is_visible(&c, 1, &active));
        assert!(!is_visible(&c, 2, &active));
        active.insert("Tabs".into(), 1);
        assert!(is_visible(&c, 2, &active));
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

    #[test]
    fn drop_rejects_inactive_tab_and_chrome() {
        let mut c = vec![
            ctrl("Tabs", ControlType::TabControl, 0, 0, 300, 200, None),
            ctrl("A", ControlType::Button, 10, 40, 60, 20, Some("Tabs")),
        ];
        c[1].tab = Some(0);
        let mut active = ActiveTabs::new();
        active.insert("Tabs".into(), 0);
        assert_eq!(
            resolve_drop_target(&c, 100, 10, 1, &active),
            DropTarget::Form
        );
        assert_eq!(
            resolve_drop_target(&c, 100, 30, 1, &active),
            DropTarget::Form
        );
        assert_eq!(
            resolve_drop_target(&c, 150, 100, 1, &active),
            DropTarget::Into {
                container: "Tabs".into(),
                tab: Some(0)
            }
        );
    }
}
