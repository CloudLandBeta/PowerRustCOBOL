// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! **The form's minimum size** (spec 056 R18).
//!
//! The smallest surface at which every docked control still receives its
//! minimum and no stretched control is clamped below its `MinWidth`/
//! `MinHeight` — a stretched CONTAINER's minimum being what its own children
//! need. A zero limit is no limit, so a form that sets none has only
//! `MinFormWidth` × `MinFormHeight` as its floor.

use crate::layout::limits::clamp;
use crate::layout::props::{self, Dock, FormBag, PropSource};
use crate::layout::{flex, grid, LayoutMode};
use crate::layout::{designed_rect, lays_out_children, AxisPlace, Insets, LayoutInput, Tree};
use crate::model::Control;

/// The smallest size the whole form lays out for.
pub(crate) fn form_min(input: &LayoutInput<'_>, tree: &Tree) -> (f32, f32) {
    let form = FormBag(input.form_props);
    let pad = props::padding(&form);
    let client = (
        input.designed_size.0 - pad.horizontal(),
        input.designed_size.1 - pad.vertical(),
    );
    let (w, h) = client_min(input, tree, None, &form, client);
    (
        (w + pad.horizontal()).max(form.number("MinFormWidth")),
        (h + pad.vertical()).max(form.number("MinFormHeight")),
    )
}

/// What a container's client area must hold: `designed_client` is its client
/// extent at the designed size.
fn client_min(
    input: &LayoutInput<'_>,
    tree: &Tree,
    parent: Option<&str>,
    src: &dyn PropSource,
    designed_client: (f32, f32),
) -> (f32, f32) {
    let kids: Vec<&Control> = tree
        .children(parent)
        .iter()
        .map(|&i| &input.controls[i])
        .filter(|c| !c.control_type.is_non_visual())
        .collect();

    // A flex, flow or grid container: what its items need at their minimums.
    let mode = props::layout_mode(src);
    if mode != LayoutMode::Absolute {
        let size = |c: &Control| {
            let r = designed_rect(input, c);
            (r.w, r.h)
        };
        let mins: Vec<(f32, f32)> = kids.iter().map(|c| own_min(input, tree, c)).collect();
        return match mode {
            LayoutMode::Grid => {
                let items: Vec<grid::Item> = kids.iter().map(|c| grid::item(c, size(c))).collect();
                grid::min_size(&grid::container(src), &items)
            }
            LayoutMode::Flow => {
                let items: Vec<flex::Item> = kids.iter().map(|c| flex::flow_item(c, size(c))).collect();
                flex::min_size(&flex::flow_container(src), &items, &mins)
            }
            _ => {
                let items: Vec<flex::Item> = kids.iter().map(|c| flex::flex_item(c, size(c))).collect();
                flex::min_size(&flex::flex_container(src), &items, &mins)
            }
        };
    }

    // Docks, solved from the last one back: each needs its own thickness plus
    // whatever the docks after it need beside it.
    let mut need = (0.0f32, 0.0f32);
    for c in kids.iter().rev() {
        let d = props::dock(c);
        if d == Dock::None {
            continue;
        }
        let r = designed_rect(input, c);
        let (min_w, min_h) = own_min(input, tree, c);
        need = match d {
            Dock::Left | Dock::Right => (clamp(r.w, props::width_limits(c)) + need.0, min_h.max(need.1)),
            Dock::Top | Dock::Bottom => (min_w.max(need.0), clamp(r.h, props::height_limits(c)) + need.1),
            _ => (min_w.max(need.0), min_h.max(need.1)),
        };
    }

    // A stretched axis grows and shrinks with the parent: the parent may not
    // shrink it below its own minimum.
    // Spec 081 — a control scaled with the window shrinks by the parent's
    // ratio: the parent may not shrink it below its own minimum either.
    let scaling = crate::layout::scale::style(&FormBag(input.form_props));
    for c in &kids {
        if props::dock(c) != Dock::None {
            continue;
        }
        if scaling != 0 && !crate::layout::scale::opted_out(c) {
            if crate::layout::scale::resizes(scaling) {
                let r = designed_rect(input, c);
                let (min_w, min_h) = own_min(input, tree, c);
                if min_w > 0.0 && r.w > 0.0 {
                    need.0 = need.0.max(designed_client.0 * min_w / r.w);
                }
                if min_h > 0.0 && r.h > 0.0 {
                    need.1 = need.1.max(designed_client.1 * min_h / r.h);
                }
            }
            continue;
        }
        let e = props::anchor(c);
        let r = designed_rect(input, c);
        let (min_w, min_h) = own_min(input, tree, c);
        if crate::layout::anchor::axis_place(e.left, e.right) == AxisPlace::Stretch && min_w > 0.0 {
            need.0 = need.0.max(designed_client.0 - r.w + min_w);
        }
        if crate::layout::anchor::axis_place(e.top, e.bottom) == AxisPlace::Stretch && min_h > 0.0 {
            need.1 = need.1.max(designed_client.1 - r.h + min_h);
        }
    }
    need
}

/// A control's own minimum: its `MinWidth`/`MinHeight`, or — for a container
/// that lays its children out — what they need plus its chrome and padding,
/// whichever is larger.
fn own_min(input: &LayoutInput<'_>, tree: &Tree, c: &Control) -> (f32, f32) {
    let lim = (props::width_limits(c).min, props::height_limits(c).min);
    if !lays_out_children(c) || tree.children(Some(&c.id)).is_empty() {
        return lim;
    }
    let frame = Insets::between(c.rect, c.content_rect()).plus(props::padding(c));
    let r = designed_rect(input, c);
    let inner = client_min(
        input,
        tree,
        Some(&c.id),
        c,
        (r.w - frame.horizontal(), r.h - frame.vertical()),
    );
    let content = |v: f32, f: f32| if v > 0.0 { v + f } else { 0.0 };
    (
        lim.0.max(content(inner.0, frame.horizontal())),
        lim.1.max(content(inner.1, frame.vertical())),
    )
}

#[cfg(test)]
mod tests {
    use crate::layout::test_support::*;
    use crate::layout::{solve, LayoutInput};
    use crate::model::{ControlType, PropValue};
    use std::collections::BTreeMap;

    /// AC7/AC8 (pure) — form 800×600 with:
    ///   a `Left` dock 200 wide with `MinHeight` 300,
    ///   a `Fill` panel with `MinWidth` 250 and `MinHeight` 120,
    ///   an anchored `Top,Left,Right` TextBox designed 500 wide in the 800
    ///   client with `MinWidth` 420.
    /// Docks, back to front: Fill needs (250, 120); Left adds its 200 beside
    /// it and raises the height to 300 → (450, 300).
    /// The TextBox: 800 − 500 + 420 = 720 wide.
    /// Minimum = (max(450, 720), 300) = (720, 300).
    #[test]
    fn the_form_minimum_is_what_docks_and_stretched_controls_need() {
        let mut left = with(ctrl("L", ControlType::Panel, (0, 0, 200, 10), None), "Dock", PropValue::String("Left".into()));
        left = with(left, "MinHeight", PropValue::Int(300));
        left.z_order = 1;
        let mut fill = with(ctrl("F", ControlType::Panel, (0, 0, 10, 10), None), "Dock", PropValue::String("Fill".into()));
        fill = with(with(fill, "MinWidth", PropValue::Int(250)), "MinHeight", PropValue::Int(120));
        fill.z_order = 2;
        let tb = with(
            with(ctrl("T", ControlType::TextBox, (100, 10, 500, 30), None), "Anchor", PropValue::String("Top,Left,Right".into())),
            "MinWidth",
            PropValue::Int(420),
        );
        let o = solve_at(&[left, fill, tb], (800.0, 600.0), (100.0, 100.0));
        assert_eq!(o.min_size, (720.0, 300.0));
        assert_eq!(o.laid_out_size, (720.0, 300.0), "a surface below the minimum lays out AT the minimum");
        println!("form minimum for Left+Fill docks and a stretched TextBox: {:?}", o.min_size);
    }

    /// `MinFormWidth`/`MinFormHeight` are read from the form, not assumed.
    #[test]
    fn the_form_floor_comes_from_its_properties() {
        let bag = BTreeMap::from([
            ("MinFormWidth".to_owned(), PropValue::Int(320)),
            ("MinFormHeight".to_owned(), PropValue::Int(240)),
        ]);
        let controls = [ctrl("B", ControlType::Button, (0, 0, 10, 10), None)];
        let o = solve(&LayoutInput::new(&controls, (800.0, 600.0), (10.0, 10.0), &bag, &[]));
        assert_eq!(o.min_size, (320.0, 240.0));
        let o = solve_at(&controls, (800.0, 600.0), (10.0, 10.0));
        assert_eq!(
            o.min_size,
            (crate::layout::defaults::MIN_FORM_WIDTH as f32, crate::layout::defaults::MIN_FORM_HEIGHT as f32)
        );
    }

    /// A stretched Panel's minimum is what its own children need: a Panel
    /// (inset 2 per side) holding a `Left,Right` child designed 300 of a 396
    /// client with `MinWidth` 280 needs a client of 396 − 300 + 280 = 376, so
    /// the Panel needs 380; the Panel itself is designed 400 of 600, so the
    /// form needs 600 − 400 + 380 = 580.
    #[test]
    fn a_stretched_container_needs_what_its_children_need() {
        let p = with(ctrl("P", ControlType::Panel, (100, 100, 400, 200), None), "Anchor", PropValue::String("Top,Left,Right".into()));
        let k = with(
            with(ctrl("K", ControlType::TextBox, (120, 110, 300, 30), Some("P")), "Anchor", PropValue::String("Top,Left,Right".into())),
            "MinWidth",
            PropValue::Int(280),
        );
        let o = solve_at(&[p, k], (600.0, 400.0), (100.0, 400.0));
        assert_eq!(o.min_size.0, 580.0);
    }
}
