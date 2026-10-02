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
//!
//! A WRAPPING container (a Flex or Flow that wraps, a Grid whose columns
//! `Repeat(AutoFill, …)`) is measured in two passes. Its cross size depends
//! on how many items fit on a line, and that depends on the main size it is
//! laid out at. The first pass asks, as if every item had a line of its own,
//! for the minimum on the main axis. The form is then laid out AT that
//! minimum, and each wrapping container's cross size becomes what the lines
//! it forms there need (operator ruling, 2026-10-01) — so content never
//! overflows at the window's minimum, and a gallery is not stacked one card
//! per row to get there.

use std::collections::HashMap;

use crate::layout::breakpoints::Breakpoint;
use crate::layout::limits::clamp;
use crate::layout::props::{self, Dock, FormBag, PropSource};
use crate::layout::{defaults, flex, grid, LayoutMode, LayoutOutput, LRect};
use crate::layout::{designed_rect, lays_out_children, page_sets, reading_order, AxisPlace, Insets, LayoutInput, Tree};
use crate::model::{Control, ControlType};

/// Each container's client rectangle as laid out at a candidate minimum, by
/// id — the form's under `None`.
type Laid = HashMap<Option<String>, LRect>;

/// The smallest size the whole form lays out for.
pub(crate) fn form_min(input: &LayoutInput<'_>, tree: &Tree, bp: Option<&Breakpoint>) -> (f32, f32) {
    let first = form_min_at(input, tree, None);
    if !wraps(input) {
        return first;
    }
    let laid = |size: (f32, f32)| {
        let mut at = input.clone();
        at.available = size;
        laid_clients(input, &crate::layout::lay_out(&at, tree, bp, size))
    };
    let lower = |a: (f32, f32), b: (f32, f32)| (a.0.min(b.0), a.1.min(b.1));
    // Second pass: every wrapping container measured at the main size it has
    // at the first pass's minimum.
    let second = lower(form_min_at(input, tree, Some(&laid(first))), first);
    if second.0 < first.0 - defaults::EPSILON {
        // A column-wrapping container narrowed the form, so a row-wrapping
        // one has less width than it was measured at: measure its height
        // again at the narrower width. More height only helps the columns.
        let third = form_min_at(input, tree, Some(&laid(second)));
        return (second.0, second.1.max(third.1.min(first.1)));
    }
    second
}

/// Whether any container of the form — the form included — wraps: a Flex or
/// Flow whose items may break into lines, or a Grid of AutoFill columns.
fn wraps(input: &LayoutInput<'_>) -> bool {
    wrapping(&FormBag(input.form_props)) || input.controls.iter().any(|c| lays_out_children(c) && wrapping(c))
}

/// Whether the container `src` wraps (see [`wraps`]).
fn wrapping(src: &dyn PropSource) -> bool {
    match props::layout_mode(src) {
        LayoutMode::Flex => flex::flex_container(src).wrap != flex::Wrap::NoWrap,
        LayoutMode::Flow => flex::flow_container(src).wrap != flex::Wrap::NoWrap,
        LayoutMode::Grid => grid::container(src).columns.auto_fills(),
        LayoutMode::Absolute => false,
    }
}

/// Every container's laid-out client rectangle in `out`.
fn laid_clients(input: &LayoutInput<'_>, out: &LayoutOutput) -> Laid {
    let pad = props::padding(&FormBag(input.form_props));
    let mut m: Laid = out.containers.iter().map(|(id, g)| (Some(id.clone()), g.client)).collect();
    m.insert(None, LRect::new(0.0, 0.0, out.laid_out_size.0, out.laid_out_size.1).deflate(pad));
    m
}

/// The form's minimum, each wrapping container measured at its client in
/// `at` when given (one item per line otherwise).
fn form_min_at(input: &LayoutInput<'_>, tree: &Tree, at: Option<&Laid>) -> (f32, f32) {
    let form = FormBag(input.form_props);
    let pad = props::padding(&form);
    let client = (
        input.designed_size.0 - pad.horizontal(),
        input.designed_size.1 - pad.vertical(),
    );
    let (w, h) = client_min(input, tree, None, &form, client, at);
    (
        (w + pad.horizontal()).max(form.number("MinFormWidth")),
        (h + pad.vertical()).max(form.number("MinFormHeight")),
    )
}

/// What a container's client area must hold: `designed_client` is its client
/// extent at the designed size. A TabControl's pages are laid out apart, each
/// in the whole client, so it must hold its largest page.
fn client_min(
    input: &LayoutInput<'_>,
    tree: &Tree,
    parent: Option<&str>,
    src: &dyn PropSource,
    designed_client: (f32, f32),
    at: Option<&Laid>,
) -> (f32, f32) {
    let visual: Vec<usize> = tree
        .children(parent)
        .iter()
        .copied()
        .filter(|&i| !input.controls[i].control_type.is_non_visual())
        .collect();
    page_sets(input.controls, parent, &visual)
        .iter()
        .map(|set| set_min(input, tree, parent, src, designed_client, set, at))
        .fold((0.0f32, 0.0f32), |a, b| (a.0.max(b.0), a.1.max(b.1)))
}

/// What one sibling set — all of a container's children, or one page's —
/// needs of the client area.
#[allow(clippy::too_many_arguments)]
fn set_min(
    input: &LayoutInput<'_>,
    tree: &Tree,
    parent: Option<&str>,
    src: &dyn PropSource,
    designed_client: (f32, f32),
    set: &[usize],
    at: Option<&Laid>,
) -> (f32, f32) {
    // A flex, flow or grid container: what its items need at their minimums.
    let mode = props::layout_mode(src);
    if mode != LayoutMode::Absolute {
        // A wrapping container measured where it is laid out (the second
        // pass): the items in the reading order the layout places them in,
        // as it breaks its lines. Any other is measured as before.
        let laid = at.filter(|_| wrapping(src)).and_then(|m| m.get(&parent.map(str::to_owned))).copied();
        let flex_c = match mode {
            LayoutMode::Flow => flex::flow_container(src),
            _ => flex::flex_container(src),
        };
        let order: Vec<usize> = match laid {
            Some(_) => reading_order(input, set, mode != LayoutMode::Grid && flex_c.direction == flex::Direction::RowReverse),
            None => set.to_vec(),
        };
        let kids: Vec<&Control> = order.iter().map(|&i| &input.controls[i]).collect();
        let size = |c: &Control| {
            let r = designed_rect(input, c);
            (r.w, r.h)
        };
        let mins: Vec<(f32, f32)> = kids.iter().map(|c| own_min(input, tree, c, at)).collect();
        let main = laid.map(|r| match flex_c.direction {
            flex::Direction::Row | flex::Direction::RowReverse => r.w,
            _ => r.h,
        });
        return match mode {
            LayoutMode::Grid => {
                let items: Vec<grid::Item> = kids.iter().map(|c| grid::item(c, size(c))).collect();
                grid::min_size_at(&grid::container(src), &items, laid.map(|r| r.w))
            }
            LayoutMode::Flow => {
                let items: Vec<flex::Item> = kids.iter().map(|c| flex::flow_item(c, size(c))).collect();
                flex::min_size_at(&flex_c, &items, &mins, main)
            }
            _ => {
                let items: Vec<flex::Item> = kids.iter().map(|c| flex::flex_item(c, size(c))).collect();
                flex::min_size_at(&flex_c, &items, &mins, main)
            }
        };
    }
    let kids: Vec<&Control> = set.iter().map(|&i| &input.controls[i]).collect();

    // Docks, solved from the last one back: each needs its own thickness plus
    // whatever the docks after it need beside it.
    let mut need = (0.0f32, 0.0f32);
    for c in kids.iter().rev() {
        let d = props::dock(c);
        if d == Dock::None {
            continue;
        }
        let r = designed_rect(input, c);
        let (min_w, min_h) = own_min(input, tree, c, at);
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
                let (min_w, min_h) = own_min(input, tree, c, at);
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
        let (min_w, min_h) = own_min(input, tree, c, at);
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
fn own_min(input: &LayoutInput<'_>, tree: &Tree, c: &Control, at: Option<&Laid>) -> (f32, f32) {
    let lim = (props::width_limits(c).min, props::height_limits(c).min);
    if c.control_type == ControlType::Splitter {
        let panes = splitter_min(input, tree, c, at);
        return (lim.0.max(panes.0), lim.1.max(panes.1));
    }
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
        at,
    );
    let content = |v: f32, f: f32| if v > 0.0 { v + f } else { 0.0 };
    (
        lim.0.max(content(inner.0, frame.horizontal())),
        lim.1.max(content(inner.1, frame.vertical())),
    )
}

/// What a Splitter needs so that each of its panes holds what the pane's own
/// children need (R26/R27): the panes split the splitter's inner span at
/// `SplitPosition` percent around the line, so a pane's minimum is reached
/// through that share — pane 1 needs a span of its minimum over the
/// percentage, pane 2 over the rest. A closed pane (0 % or 100 %) asks for
/// nothing. Checked against [`crate::splitter::geometry`], which the layout
/// and the render place the panes with.
fn splitter_min(input: &LayoutInput<'_>, tree: &Tree, s: &Control, at: Option<&Laid>) -> (f32, f32) {
    use crate::splitter;
    let horizontal = splitter::is_horizontal(s);
    let along = |v: (f32, f32)| if horizontal { v } else { (v.1, v.0) };
    let mut need = [(0.0f32, 0.0f32); 2];
    for &i in tree.children(Some(&s.id)) {
        let pane = &input.controls[i];
        if let Some(n) = splitter::pane_index(pane) {
            let m = along(own_min(input, tree, pane, at));
            let k = (n - 1) as usize;
            need[k] = (need[k].0.max(m.0), need[k].1.max(m.1));
        }
    }
    if need.iter().all(|m| m.0 <= 0.0 && m.1 <= 0.0) {
        return (0.0, 0.0);
    }
    let inner = splitter::content_rect(s, s.rect);
    let chrome = along(((s.rect.w - inner.w) as f32, (s.rect.h - inner.h) as f32));
    let p = splitter::split_percent(s) as f32;
    let thick = splitter::line_size(s) as f32;
    let half = (splitter::line_size(s) / 2) as f32;
    let mut span = 0.0f32;
    if p > 0.0 && need[0].0 > 0.0 {
        span = span.max((need[0].0 + half) * defaults::PERCENT / p);
    }
    if p < defaults::PERCENT && need[1].0 > 0.0 {
        span = span.max((need[1].0 + thick - half) * defaults::PERCENT / (defaults::PERCENT - p));
    }
    // Whole points, raised until the splitter's own rounding gives each pane
    // its minimum.
    let mut span = span.ceil();
    if span > 0.0 {
        for _ in 0..defaults::SPLITTER_FIT_ROUNDS {
            let len = (span + chrome.0) as i32;
            let r = if horizontal {
                crate::model::Rect::new(s.rect.x, s.rect.y, len, s.rect.h)
            } else {
                crate::model::Rect::new(s.rect.x, s.rect.y, s.rect.w, len)
            };
            let g = splitter::geometry(s, r);
            let got = |r: crate::model::Rect| if horizontal { r.w as f32 } else { r.h as f32 };
            let ok1 = p <= 0.0 || got(g.pane1) + defaults::EPSILON >= need[0].0;
            let ok2 = p >= defaults::PERCENT || got(g.pane2) + defaults::EPSILON >= need[1].0;
            if ok1 && ok2 {
                break;
            }
            span += 1.0;
        }
    }
    let main = if span > 0.0 { span + chrome.0 } else { 0.0 };
    let cross = need[0].1.max(need[1].1);
    let cross = if cross > 0.0 { cross + chrome.1 } else { 0.0 };
    if horizontal { (main, cross) } else { (cross, main) }
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

    /// Every laid-out control of `o` that ends outside its parent's client
    /// area (or the form's), by id.
    fn overflowing(controls: &[crate::model::Control], o: &crate::layout::LayoutOutput) -> Vec<String> {
        let eps = 0.01;
        controls
            .iter()
            .filter_map(|c| {
                let r = o.rects.get(&c.id)?;
                let cl = match c.parent.as_deref() {
                    None => crate::layout::LRect::new(0.0, 0.0, o.laid_out_size.0, o.laid_out_size.1),
                    Some(p) => o.containers.get(p)?.client,
                };
                let out = r.x < cl.x - eps || r.y < cl.y - eps || r.right() > cl.right() + eps || r.bottom() > cl.bottom() + eps;
                out.then(|| format!("{} {:?} in {:?}", c.id, r, cl))
            })
            .collect()
    }

    /// A `Fill` panel holding `n` items `(w, h)` designed in rows of five,
    /// laid out by `props`; the form is 900 × 600 with `MinFormWidth`
    /// `min_w`.
    fn wrapping_form(props: &[(&str, &str)], n: i32, item: (i32, i32), min_w: i64) -> (Vec<crate::model::Control>, BTreeMap<String, PropValue>) {
        let mut g = with(ctrl("G", ControlType::Panel, (0, 0, 900, 600), None), "Dock", PropValue::String("Fill".into()));
        for (k, v) in props {
            g = with(g, k, PropValue::String((*v).into()));
        }
        let c = g.content_rect();
        let mut v = vec![g];
        for k in 0..n {
            let (col, row) = (k % 5, k / 5);
            v.push(ctrl(&format!("I{k}"), ControlType::Panel, (c.x + col * (item.0 + 10), c.y + row * (item.1 + 10), item.0, item.1), Some("G")));
        }
        (v, BTreeMap::from([("MinFormWidth".to_owned(), PropValue::Int(min_w))]))
    }

    /// Operator ruling 2026-10-01 (two-pass minimum): a 12-card
    /// `Repeat(AutoFill, MinMax(160px, 1fr))` gallery and a 10-item wrapping
    /// Flow no longer ask for room to stack every item. The form's minimum
    /// width is 700 (gallery) / 560 (flow), as before; the gallery laid out
    /// there holds 4 columns (4 × 160 + 3 × 10 = 670), so its 12 cards need
    /// 3 rows: 3 × 100 + 2 × 10 = 320 high — not 12 × 100 + 11 × 10 = 1310.
    /// The flow holds 4 items a line (4 × 120 + 3 × 10 = 510), so its 10
    /// items need 3 lines: 3 × 40 + 2 × 10 = 140 — not 490.
    #[test]
    fn a_wrapping_container_needs_the_lines_it_forms_at_the_minimum_width() {
        let (gallery, gbag) = wrapping_form(
            &[("LayoutMode", "Grid"), ("GridColumns", "Repeat(AutoFill, MinMax(160px, 1fr))"), ("Gap", "10")],
            12,
            (160, 100),
            700,
        );
        let o = solve(&LayoutInput::new(&gallery, (900.0, 600.0), (10.0, 10.0), &gbag, &[]));
        let frame = crate::layout::Insets::between(gallery[0].rect, gallery[0].content_rect());
        assert_eq!(o.min_size.0, 700.0);
        assert_eq!(o.min_size.1, 320.0 + frame.vertical(), "three rows of four, not twelve rows");

        let (flow, fbag) = wrapping_form(&[("LayoutMode", "Flow"), ("WrapContents", "true"), ("Gap", "10")], 10, (120, 40), 560);
        let f = solve(&LayoutInput::new(&flow, (900.0, 600.0), (10.0, 10.0), &fbag, &[]));
        assert_eq!(f.min_size.0, 560.0);
        assert_eq!(f.min_size.1, 140.0 + frame.vertical(), "three lines of four, not ten lines");

        // Column-wrap, axes swapped: 9 items 150 × 60 in a wrapping Flex
        // column with `MinFormHeight` 300 hold 4 a column there
        // (4 × 60 + 3 × 8 = 264), so they need 3 columns: 3 × 150 + 2 × 8.
        let (col, mut cbag) = wrapping_form(
            &[("LayoutMode", "Flex"), ("FlexDirection", "Column"), ("FlexWrap", "Wrap"), ("AlignItems", "Start"), ("Gap", "8")],
            9,
            (150, 60),
            64,
        );
        cbag.insert("MinFormHeight".into(), PropValue::Int(300));
        let k = solve(&LayoutInput::new(&col, (900.0, 600.0), (10.0, 10.0), &cbag, &[]));
        assert_eq!(k.min_size, (466.0 + frame.horizontal(), 300.0), "three columns of four, not nine");
        println!(
            "gallery minimum {:?} (was 1310 + frame high); flow minimum {:?} (was 490 + frame high); column-wrap minimum {:?} (was 1414 + frame wide)",
            o.min_size, f.min_size, k.min_size
        );
    }

    /// At the window's minimum nothing overflows: the gallery, the flow and
    /// a Flex row that wraps, laid out at their own minimum size, keep every
    /// item inside its container.
    #[test]
    fn nothing_overflows_at_the_window_minimum() {
        let mut col_300 = wrapping_form(&[("LayoutMode", "Flex"), ("FlexDirection", "Column"), ("FlexWrap", "Wrap"), ("AlignItems", "Start"), ("Gap", "8")], 9, (150, 60), 64);
        col_300.1.insert("MinFormHeight".into(), PropValue::Int(300));
        let cases = [
            col_300,
            wrapping_form(&[("LayoutMode", "Grid"), ("GridColumns", "Repeat(AutoFill, MinMax(160px, 1fr))"), ("Gap", "10")], 12, (160, 100), 700),
            wrapping_form(&[("LayoutMode", "Flow"), ("WrapContents", "true"), ("Gap", "10")], 10, (120, 40), 560),
            wrapping_form(&[("LayoutMode", "Flex"), ("FlexWrap", "Wrap"), ("AlignItems", "Start"), ("Gap", "8")], 9, (150, 60), 480),
            wrapping_form(&[("LayoutMode", "Flex"), ("FlexDirection", "Column"), ("FlexWrap", "Wrap"), ("AlignItems", "Start"), ("Gap", "8")], 9, (150, 60), 64),
        ];
        for (controls, bag) in &cases {
            let min = solve(&LayoutInput::new(controls, (900.0, 600.0), (10.0, 10.0), bag, &[])).min_size;
            let o = solve(&LayoutInput::new(controls, (900.0, 600.0), min, bag, &[]));
            assert_eq!(o.laid_out_size, min);
            let bad = overflowing(controls, &o);
            assert!(bad.is_empty(), "{:?} at its minimum {min:?}: {bad:?}", controls[0].properties.get("LayoutMode"));
            println!("{:?}: minimum {min:?}, nothing overflows", controls[0].properties.get("LayoutMode").map(|v| v.to_xml_string()));
        }
    }

    /// A Splitter's panes count toward the form minimum through its geometry:
    /// a `Fill` horizontal splitter at 50 % whose pane 1 holds a `Left,Right`
    /// field with `MinWidth` 300 makes the form wide enough that pane 1 —
    /// half the splitter less half the line — still gives the field 300.
    #[test]
    fn a_splitter_panes_children_count_toward_the_minimum() {
        let s = |v: &str| PropValue::String(v.into());
        let mut form = crate::model::Form::new("F", "F", 800, 400);
        form.controls.push(with(ctrl("S", ControlType::Splitter, (0, 0, 800, 400), None), "Dock", s("Fill")));
        form.sync_splitter_panes();
        let n1 = crate::splitter::pane_id("S", 1);
        let p1 = form.find_control(&n1).unwrap().rect;
        let fld = with(
            with(ctrl("FLD", ControlType::TextBox, (p1.x + 10, p1.y + 10, p1.w - 20, 24), Some(&n1)), "Anchor", s("Top,Left,Right")),
            "MinWidth",
            PropValue::Int(300),
        );
        form.controls.push(fld);
        let o = solve_at(&form.controls, (800.0, 400.0), (10.0, 10.0));
        assert!(o.min_size.0 > 600.0, "both halves of the splitter: {:?}", o.min_size);
        let at_min = solve_at(&form.controls, (800.0, 400.0), o.min_size);
        assert!(at_min.rects["FLD"].w >= 300.0 - 0.01, "the field keeps its MinWidth at the minimum: {:?}", at_min.rects["FLD"]);
        let small = solve_at(&form.controls, (800.0, 400.0), (o.min_size.0 - 4.0, 400.0));
        assert_eq!(small.laid_out_size.0, o.min_size.0, "the form never lays out narrower");
        println!("splitter: form minimum {:?}; FLD at the minimum {:?}", o.min_size, at_min.rects["FLD"]);
    }
}
