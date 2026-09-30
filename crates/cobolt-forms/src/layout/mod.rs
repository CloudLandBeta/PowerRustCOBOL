// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! **Responsive layout** (spec 056) — one pure function, every surface.
//!
//! [`solve`] takes a form's designed controls, its designed size, its layout
//! properties and breakpoint table, and the size of the surface it is being
//! drawn on, and answers where every control lands and at what font size. It
//! knows nothing of egui, windows or frames (R22): the designer canvas, the
//! preview, the run-form window, a ContentPane occupant and the compiled
//! binary all ask it the same question and so draw the same answer (R23).
//!
//! It never writes the design (R10). Every placement is computed from the
//! DESIGNED rectangle and the DESIGNED parent — never from a previous frame —
//! so it is idempotent and cannot drift (R9, R24).
//!
//! Rectangles stay form-space absolute (R21), in `f32` because stretched,
//! proportional and fractional placements do not land on whole pixels.

pub mod anchor;
/// Render-side glue: surfaces prepare a responsive form before rendering it.
#[cfg(feature = "render")]
pub mod apply;
pub mod breakpoints;
pub mod defaults;
pub mod dock;
pub mod flex;
pub mod fonts;
pub mod grid;
pub mod inverse;
pub mod limits;
pub mod minsize;
pub mod props;
pub mod tracks;

use std::collections::{BTreeMap, HashMap, HashSet};

use crate::model::{Control, ControlType, PropValue, Rect};
use breakpoints::Breakpoint;
use props::{Dock, FormBag, PropSource};

/// A rectangle in form space, fractional.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct LRect {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

impl LRect {
    pub fn new(x: f32, y: f32, w: f32, h: f32) -> Self {
        LRect { x, y, w, h }
    }
    pub fn from_model(r: Rect) -> Self {
        LRect::new(r.x as f32, r.y as f32, r.w as f32, r.h as f32)
    }
    pub fn right(&self) -> f32 {
        self.x + self.w
    }
    pub fn bottom(&self) -> f32 {
        self.y + self.h
    }
    /// Shrunk by `i` on every side, never to a negative size.
    pub fn deflate(&self, i: Insets) -> LRect {
        LRect::new(
            self.x + i.left,
            self.y + i.top,
            (self.w - i.left - i.right).max(0.0),
            (self.h - i.top - i.bottom).max(0.0),
        )
    }
}

/// Per-side distances.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Insets {
    pub left: f32,
    pub top: f32,
    pub right: f32,
    pub bottom: f32,
}

impl Insets {
    /// How far `inner` sits inside `outer` on each side.
    pub fn between(outer: Rect, inner: Rect) -> Insets {
        Insets {
            left: (inner.x - outer.x) as f32,
            top: (inner.y - outer.y) as f32,
            right: ((outer.x + outer.w) - (inner.x + inner.w)) as f32,
            bottom: ((outer.y + outer.h) - (inner.y + inner.h)) as f32,
        }
    }
    pub fn plus(self, o: Insets) -> Insets {
        Insets {
            left: self.left + o.left,
            top: self.top + o.top,
            right: self.right + o.right,
            bottom: self.bottom + o.bottom,
        }
    }
    pub fn horizontal(&self) -> f32 {
        self.left + self.right
    }
    pub fn vertical(&self) -> f32 {
        self.top + self.bottom
    }
}

/// How a container places its children (R49).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LayoutMode {
    /// Anchoring (§4.2) and docking (§4.3).
    Absolute,
    Flex,
    Grid,
    Flow,
}

/// How one axis of an anchored control follows its parent (R8).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AxisPlace {
    /// Leading edge only: a fixed offset from `Left`/`Top`.
    Lead,
    /// Trailing edge only: a fixed offset from `Right`/`Bottom`.
    Trail,
    /// Both edges: the size follows the parent.
    Stretch,
    /// Neither: the centre stays at the same fraction of the parent.
    Proportional,
}

/// What placed a control — kept so the designer and COBOL writes can map an
/// on-screen rectangle back to the designed one (R33, R38).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Placement {
    /// Not laid out: a non-visual control stays where it was designed.
    Designed,
    /// Carried rigidly by the control that owns its position — a Splitter
    /// pane, a SideMenu footer, a repeating group's contents (R27) — by the
    /// same offset its owner moved.
    Rigid { dx: f32, dy: f32 },
    /// Anchored in an `Absolute` parent.
    Anchored {
        x: AxisPlace,
        y: AxisPlace,
        designed_parent: LRect,
        parent: LRect,
    },
    /// Docked in an `Absolute` parent.
    Docked(Dock),
    /// Placed by a flex, grid or flow parent.
    Item(LayoutMode),
}

/// A laid-out container, for the designer's overlays (R45).
#[derive(Clone, Debug, PartialEq)]
pub struct ContainerGeom {
    pub mode: LayoutMode,
    /// The client rectangle at the designed size, less padding.
    pub designed_client: LRect,
    /// The client rectangle as laid out, less padding.
    pub client: LRect,
}

/// Everything the solver reads.
pub struct LayoutInput<'a> {
    /// The designed controls: flat, with `parent` links (R21).
    pub controls: &'a [Control],
    /// The form's designed `Width`/`Height`.
    pub designed_size: (f32, f32),
    /// The form's layout bag (`LayoutMode`, container properties, `Padding`,
    /// `FontScaling`, font-scale limits, the smallest form).
    pub form_props: &'a BTreeMap<String, PropValue>,
    /// The breakpoint table (R58).
    pub breakpoints: &'a [Breakpoint],
    /// The surface's available size (R23).
    pub available: (f32, f32),
    /// Sizes `AutoSize` measured, by control id; a control not listed enters
    /// layout at its designed size (R22, R26 step 3).
    pub intrinsic: Option<&'a HashMap<String, (f32, f32)>>,
    /// The operating system's text-size factor (R68).
    pub system_text_factor: f32,
    /// `me::Breakpoint` written by COBOL (R84).
    pub pinned_breakpoint: Option<&'a str>,
    /// `me::FontScale` written by COBOL, > 0 (R84).
    pub pinned_font_scale: Option<f32>,
}

impl<'a> LayoutInput<'a> {
    /// The common case: no measured sizes, no pins, no system factor.
    pub fn new(
        controls: &'a [Control],
        designed_size: (f32, f32),
        available: (f32, f32),
        form_props: &'a BTreeMap<String, PropValue>,
        breakpoints: &'a [Breakpoint],
    ) -> Self {
        LayoutInput {
            controls,
            designed_size,
            form_props,
            breakpoints,
            available,
            intrinsic: None,
            system_text_factor: 1.0,
            pinned_breakpoint: None,
            pinned_font_scale: None,
        }
    }
}

/// The solver's answer.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct LayoutOutput {
    /// Every control's rectangle, form-space absolute.
    pub rects: HashMap<String, LRect>,
    /// Every control's effective font size (R69).
    pub font_sizes: HashMap<String, f32>,
    /// Controls a breakpoint hides — laid out as absent (R62).
    pub hidden: HashSet<String>,
    /// The active breakpoint's name (empty with no table).
    pub breakpoint: String,
    /// The form's font factor, system factor included (R67, R68).
    pub font_factor: f32,
    /// The smallest surface the form lays out for (R18).
    pub min_size: (f32, f32),
    /// The size the form was laid out at: the surface, never below
    /// `min_size`.
    pub laid_out_size: (f32, f32),
    pub placement: HashMap<String, Placement>,
    pub containers: HashMap<String, ContainerGeom>,
}

/// The form's children and every container's, in render (z-) order.
pub(crate) struct Tree {
    kids: HashMap<Option<String>, Vec<usize>>,
}

impl Tree {
    pub(crate) fn new(controls: &[Control]) -> Tree {
        let ids: HashSet<&str> = controls.iter().map(|c| c.id.as_str()).collect();
        let mut kids: HashMap<Option<String>, Vec<usize>> = HashMap::new();
        for (i, c) in controls.iter().enumerate() {
            // A parent that does not exist puts the control at form level, as
            // the engine's render order does.
            let key = c.parent.clone().filter(|p| ids.contains(p.as_str()));
            kids.entry(key).or_default().push(i);
        }
        for v in kids.values_mut() {
            v.sort_by_key(|&i| (controls[i].z_order, i));
        }
        Tree { kids }
    }
    pub(crate) fn children(&self, parent: Option<&str>) -> &[usize] {
        self.kids
            .get(&parent.map(str::to_owned))
            .map(Vec::as_slice)
            .unwrap_or(&[])
    }
}

/// A repeating group's template: the group itself is placed, its contents are
/// laid out by the instancing (R27).
pub(crate) fn is_repeating_template(c: &Control) -> bool {
    c.control_type == ControlType::GroupBox
        && c.get_prop("IsRepeatingGroup").map(|v| v.as_bool()).unwrap_or(false)
}

/// Whether `c` lays its children out itself (a container), rather than
/// carrying them rigidly (a Splitter, a SideMenu, a repeating template).
pub(crate) fn lays_out_children(c: &Control) -> bool {
    c.is_container() && !is_repeating_template(c)
}

/// A control's designed rectangle as it enters layout: the `.cfrm` rect, at
/// the size `AutoSize` measured when it did.
pub(crate) fn designed_rect(input: &LayoutInput<'_>, c: &Control) -> LRect {
    let mut r = LRect::from_model(c.rect);
    if let Some((w, h)) = input.intrinsic.and_then(|m| m.get(&c.id)) {
        r.w = *w;
        r.h = *h;
    }
    r
}

/// The client rectangle of container `c` placed at `at`: its engine client
/// area (`content_rect`, R19) — insets taken from the designed control, so a
/// tab strip keeps its size — less its padding (R51).
pub(crate) fn client_of(c: &Control, at: LRect) -> LRect {
    let chrome = Insets::between(c.rect, c.content_rect());
    at.deflate(chrome.plus(props::padding(c)))
}

/// The smallest window a responsive form lays out for (R18) — what its
/// run-form window takes as its minimum inner size — or `None` for a form that
/// is not responsive, whose window keeps no minimum, as before.
pub fn min_size_of(form: &crate::model::Form) -> Option<(f32, f32)> {
    form.responsive.then(|| {
        window_min_size(
            &form.controls,
            (form.width as f32, form.height as f32),
            &form.layout,
            &form.breakpoints,
        )
    })
}

/// A responsive form's minimum size as a window's minimum inner size: the
/// solver's minimum, never below [`defaults::WINDOW_MIN_INNER`] (R18). The
/// one computation behind every run-form window's floor.
pub fn window_min_size(
    controls: &[Control],
    designed: (f32, f32),
    form_props: &BTreeMap<String, PropValue>,
    breakpoints: &[Breakpoint],
) -> (f32, f32) {
    let input = LayoutInput::new(controls, designed, designed, form_props, breakpoints);
    let (w, h) = solve(&input).min_size;
    (w.max(defaults::WINDOW_MIN_INNER), h.max(defaults::WINDOW_MIN_INNER))
}

/// Lay the form out for its surface (R22).
pub fn solve(input: &LayoutInput<'_>) -> LayoutOutput {
    let form = FormBag(input.form_props);
    let tree = Tree::new(input.controls);

    let bp = breakpoints::select(input.breakpoints, input.available.0, input.pinned_breakpoint);
    let font_factor = fonts::form_factor(
        &form,
        input.designed_size.0,
        input.available.0,
        bp,
        input.system_text_factor,
        input.pinned_font_scale,
    );

    let min_size = minsize::form_min(input, &tree);
    let laid_out_size = (
        input.available.0.max(min_size.0),
        input.available.1.max(min_size.1),
    );
    let pad = props::padding(&form);
    let designed_client =
        LRect::new(0.0, 0.0, input.designed_size.0, input.designed_size.1).deflate(pad);
    let client = LRect::new(0.0, 0.0, laid_out_size.0, laid_out_size.1).deflate(pad);

    let mut out = LayoutOutput {
        breakpoint: bp.map(|b| b.name.clone()).unwrap_or_default(),
        font_factor,
        min_size,
        laid_out_size,
        ..Default::default()
    };
    for c in input.controls {
        out.font_sizes
            .insert(c.id.clone(), fonts::effective_size(c, font_factor));
    }
    let mode = props::layout_mode(&form);
    let content = place_children(input, &tree, None, &form, mode, designed_client, client, &mut out);
    // A flex, grid or flow form is as tall as its content when that is
    // taller than the window, as a document is (R53): it lays out again at
    // that height and the surface scrolls. Its width stays the window's.
    if let Some((_, ch)) = content {
        let need = ch + pad.vertical();
        if need > out.laid_out_size.1 + defaults::EPSILON {
            out.laid_out_size.1 = need;
            let client = LRect::new(0.0, 0.0, out.laid_out_size.0, need).deflate(pad);
            place_children(input, &tree, None, &form, mode, designed_client, client, &mut out);
        }
    }
    out
}

/// Place the children of `parent` (the form when `None`) inside its client
/// rectangle, then recurse (R20).
#[allow(clippy::too_many_arguments)]
fn place_children(
    input: &LayoutInput<'_>,
    tree: &Tree,
    parent: Option<&str>,
    src: &dyn PropSource,
    mode: LayoutMode,
    designed_client: LRect,
    client: LRect,
    out: &mut LayoutOutput,
) -> Option<(f32, f32)> {
    let kids = tree.children(parent);
    let visual: Vec<usize> = kids
        .iter()
        .copied()
        .filter(|&i| !input.controls[i].control_type.is_non_visual())
        .collect();
    for &i in kids {
        let c = &input.controls[i];
        if c.control_type.is_non_visual() {
            out.rects.insert(c.id.clone(), designed_rect(input, c));
            out.placement.insert(c.id.clone(), Placement::Designed);
        }
    }

    // A flex, grid or flow container places its children itself (R50).
    let content = (mode != LayoutMode::Absolute).then(|| place_items(input, src, mode, &visual, client, out));

    // Docked controls first, in z-order, each taking an edge of what remains
    // (R12); every other control is anchored against the FULL client rect
    // (R14).
    let docked: Vec<(usize, Dock)> = if content.is_some() { Vec::new() } else { visual.clone() }
        .into_iter()
        .map(|i| (i, props::dock(&input.controls[i])))
        .filter(|(_, d)| *d != Dock::None)
        .collect();
    let items: Vec<dock::DockItem> = docked
        .iter()
        .map(|&(i, d)| {
            let c = &input.controls[i];
            dock::DockItem {
                dock: d,
                designed: designed_rect(input, c),
                width: props::width_limits(c),
                height: props::height_limits(c),
            }
        })
        .collect();
    for ((i, d), r) in docked.iter().zip(dock::place(&items, client)) {
        let c = &input.controls[*i];
        out.rects.insert(c.id.clone(), r);
        out.placement.insert(c.id.clone(), Placement::Docked(*d));
    }
    for &i in &visual {
        let c = &input.controls[i];
        // Placed above, by its dock or its flex/grid/flow parent.
        if content.is_some() || docked.iter().any(|(d, _)| *d == i) {
            continue;
        }
        let (r, x, y) = anchor::place(
            designed_rect(input, c),
            designed_client,
            client,
            props::anchor(c),
            props::width_limits(c),
            props::height_limits(c),
        );
        out.rects.insert(c.id.clone(), r);
        out.placement.insert(
            c.id.clone(),
            Placement::Anchored {
                x,
                y,
                designed_parent: designed_client,
                parent: client,
            },
        );
    }

    for &i in &visual {
        let c = &input.controls[i];
        if tree.children(Some(&c.id)).is_empty() {
            continue;
        }
        let laid = out.rects[&c.id];
        if lays_out_children(c) {
            let dclient = client_of(c, designed_rect(input, c));
            let lclient = client_of(c, laid);
            let cmode = props::layout_mode(c);
            out.containers.insert(
                c.id.clone(),
                ContainerGeom {
                    mode: cmode,
                    designed_client: dclient,
                    client: lclient,
                },
            );
            place_children(input, tree, Some(&c.id), c, cmode, dclient, lclient, out);
        } else if c.control_type == ControlType::Splitter {
            carry_splitter(input, tree, c, designed_rect(input, c), laid, out);
        } else {
            let d = designed_rect(input, c);
            carry_rigidly(input, tree, &c.id, laid.x - d.x, laid.y - d.y, out);
        }
    }
    content
}

/// Place the children of a `Flex`, `Flow` or `Grid` container (R50): each at
/// its designed (or measured) size as its intrinsic size, in reading order —
/// top to bottom, then left to right, or right to left for a right-to-left
/// container (R54). `Anchor` and `Dock` are not read. Returns what the items
/// need, width and height.
fn place_items(
    input: &LayoutInput<'_>,
    src: &dyn PropSource,
    mode: LayoutMode,
    kids: &[usize],
    client: LRect,
    out: &mut LayoutOutput,
) -> (f32, f32) {
    let flex_c = match mode {
        LayoutMode::Flex => Some(flex::flex_container(src)),
        LayoutMode::Flow => Some(flex::flow_container(src)),
        _ => None,
    };
    let rtl = flex_c.is_some_and(|f| f.direction == flex::Direction::RowReverse);
    let mut order: Vec<usize> = kids.to_vec();
    order.sort_by(|&a, &b| {
        let (ra, rb) = (designed_rect(input, &input.controls[a]), designed_rect(input, &input.controls[b]));
        let x = if rtl { rb.x.total_cmp(&ra.x) } else { ra.x.total_cmp(&rb.x) };
        ra.y.total_cmp(&rb.y).then(x)
    });
    let size = |i: usize| {
        let r = designed_rect(input, &input.controls[i]);
        (r.w, r.h)
    };
    let (rects, content) = match (mode, flex_c) {
        (LayoutMode::Grid, _) => {
            let g = grid::container(src);
            let items: Vec<grid::Item> = order.iter().map(|&i| grid::item(&input.controls[i], size(i))).collect();
            let s = grid::solve(&g, &items, client);
            (s.rects, s.content)
        }
        (_, Some(fc)) => {
            let items: Vec<flex::Item> = order
                .iter()
                .map(|&i| match mode {
                    LayoutMode::Flow => flex::flow_item(&input.controls[i], size(i)),
                    _ => flex::flex_item(&input.controls[i], size(i)),
                })
                .collect();
            let s = flex::solve(&fc, &items, client);
            (s.rects, s.content)
        }
        _ => return (0.0, 0.0),
    };
    for (&i, r) in order.iter().zip(rects) {
        let c = &input.controls[i];
        out.rects.insert(c.id.clone(), r);
        out.placement.insert(c.id.clone(), Placement::Item(mode));
    }
    content
}

/// A laid-out Splitter's panes and their contents (R26 step 5, R27): each pane
/// takes its place in the LAID-OUT splitter's geometry, and each pane's
/// subtree reflows from the designed pane to that one exactly as a divider
/// drag reflows it (`splitter::reflow_in_subtree`, the render's own rule), so
/// the render — which derives the panes from the splitter it is given — finds
/// every control already where that splitter puts it.
fn carry_splitter(
    input: &LayoutInput<'_>,
    tree: &Tree,
    s: &Control,
    designed: LRect,
    laid: LRect,
    out: &mut LayoutOutput,
) {
    // The model's unit, rounded as `apply::laid_out_controls` rounds.
    let model = |r: LRect| Rect::new(r.x.round() as i32, r.y.round() as i32, r.w.round() as i32, r.h.round() as i32);
    let before = crate::splitter::geometry(s, model(designed));
    let after = crate::splitter::geometry(s, model(laid));
    let horizontal = crate::splitter::is_horizontal(s);
    for &i in tree.children(Some(&s.id)) {
        let pane = &input.controls[i];
        let Some(n) = crate::splitter::pane_index(pane) else {
            // Not one of its panes: carried with the splitter, as before.
            let d = designed_rect(input, pane);
            let (dx, dy) = (laid.x - designed.x, laid.y - designed.y);
            out.rects.insert(pane.id.clone(), LRect::new(d.x + dx, d.y + dy, d.w, d.h));
            out.placement.insert(pane.id.clone(), Placement::Rigid { dx, dy });
            carry_rigidly(input, tree, &pane.id, dx, dy, out);
            continue;
        };
        let (b, a) = if n == 1 { (before.pane1, after.pane1) } else { (before.pane2, after.pane2) };
        out.rects.insert(pane.id.clone(), LRect::from_model(a));
        out.placement.insert(
            pane.id.clone(),
            Placement::Rigid { dx: (a.x - b.x) as f32, dy: (a.y - b.y) as f32 },
        );
        let behavior = crate::splitter::PaneResize::of(pane);
        for &j in tree.children(Some(&pane.id)) {
            let root = model(designed_rect(input, &input.controls[j]));
            reflow_pane_subtree(input, tree, j, root, (behavior, n, b, a, horizontal), out);
        }
    }
}

/// One control of a pane's subtree and everything under it, reflowed with the
/// subtree's root (see [`carry_splitter`]).
fn reflow_pane_subtree(
    input: &LayoutInput<'_>,
    tree: &Tree,
    i: usize,
    root: Rect,
    how: (crate::splitter::PaneResize, u8, Rect, Rect, bool),
    out: &mut LayoutOutput,
) {
    let (behavior, n, before, after, horizontal) = how;
    let c = &input.controls[i];
    let d = designed_rect(input, c);
    let moved = crate::splitter::reflow_in_subtree(
        behavior,
        n,
        before,
        after,
        root,
        Rect::new(d.x.round() as i32, d.y.round() as i32, d.w.round() as i32, d.h.round() as i32),
        horizontal,
    );
    let (dx, dy) = (moved.x as f32 - d.x.round(), moved.y as f32 - d.y.round());
    out.rects.insert(c.id.clone(), LRect::new(d.x + dx, d.y + dy, d.w, d.h));
    out.placement.insert(c.id.clone(), Placement::Rigid { dx, dy });
    for &j in tree.children(Some(&c.id)) {
        reflow_pane_subtree(input, tree, j, root, how, out);
    }
}

/// Move `owner`'s whole subtree by the offset `owner` itself moved (R27).
fn carry_rigidly(
    input: &LayoutInput<'_>,
    tree: &Tree,
    owner: &str,
    dx: f32,
    dy: f32,
    out: &mut LayoutOutput,
) {
    for &i in tree.children(Some(owner)) {
        let c = &input.controls[i];
        let d = designed_rect(input, c);
        out.rects
            .insert(c.id.clone(), LRect::new(d.x + dx, d.y + dy, d.w, d.h));
        out.placement.insert(c.id.clone(), Placement::Rigid { dx, dy });
        carry_rigidly(input, tree, &c.id, dx, dy, out);
    }
}

#[cfg(test)]
pub(crate) mod test_support {
    use super::*;

    /// A visual control at `(x, y, w, h)`, optionally inside `parent`.
    pub fn ctrl(id: &str, ct: ControlType, r: (i32, i32, i32, i32), parent: Option<&str>) -> Control {
        let mut c = Control::new(id, ct, r.0, r.1);
        c.rect = Rect::new(r.0, r.1, r.2, r.3);
        c.parent = parent.map(str::to_owned);
        c
    }

    pub fn with(mut c: Control, key: &str, v: impl Into<PropValue>) -> Control {
        c.set_prop(key, v.into());
        c
    }

    /// Solve at `available` with no breakpoints and the default form bag.
    pub fn solve_at(controls: &[Control], designed: (f32, f32), available: (f32, f32)) -> LayoutOutput {
        let bag = BTreeMap::new();
        let input = LayoutInput::new(controls, designed, available, &bag, &[]);
        solve(&input)
    }

    pub fn r(o: &LayoutOutput, id: &str) -> (f32, f32, f32, f32) {
        let r = o.rects[id];
        (r.x, r.y, r.w, r.h)
    }
}

#[cfg(test)]
mod tests {
    use super::test_support::*;
    use super::*;

    /// T5.4 — nesting: a `Fill` panel laid out as a grid `200px 1fr`, whose
    /// first cell holds a flex column (a 30 px button, then one that grows)
    /// and whose second a label. Form 600×400 at 800×500:
    ///   P fills (0, 0, 800, 500); its grid columns are 200 and the rest of
    ///   its client; the one row stretches to the client height.
    ///   G1 fills cell 1, G2 cell 2 (both `Stretch`).
    ///   In G1's client: B1 is 30 high at the top, B2 grows into the rest;
    ///   both as wide as the client (`AlignItems = Stretch`).
    /// Each container's geometry is recorded, and the form's minimum is the
    /// grid's: 200 + the label's 100 wide, the flex panel's 400 high, plus
    /// P's frame.
    #[test]
    fn a_flex_column_in_a_grid_in_a_docked_panel() {
        let s = |v: &str| PropValue::String(v.into());
        let mut p = with(ctrl("P", ControlType::Panel, (0, 0, 600, 400), None), "Dock", s("Fill"));
        p = with(with(p, "LayoutMode", s("Grid")), "GridColumns", s("200px 1fr"));
        let g1 = with(with(ctrl("G1", ControlType::Panel, (0, 0, 200, 400), Some("P")), "LayoutMode", s("Flex")), "FlexDirection", s("Column"));
        let g2 = ctrl("G2", ControlType::Label, (200, 0, 100, 20), Some("P"));
        let b1 = ctrl("B1", ControlType::Button, (0, 0, 80, 30), Some("G1"));
        let b2 = with(ctrl("B2", ControlType::Button, (0, 40, 80, 40), Some("G1")), "FlexGrow", s("1"));
        let controls = [p, g1, g2, b1, b2];
        let o = solve_at(&controls, (600.0, 400.0), (800.0, 500.0));
        assert_eq!(r(&o, "P"), (0.0, 0.0, 800.0, 500.0));
        let pc = o.containers["P"].client;
        assert_eq!(o.containers["P"].mode, LayoutMode::Grid);
        assert_eq!(r(&o, "G1"), (pc.x, pc.y, 200.0, pc.h));
        assert_eq!(r(&o, "G2"), (pc.x + 200.0, pc.y, pc.w - 200.0, pc.h));
        let gc = o.containers["G1"].client;
        assert_eq!(o.containers["G1"].mode, LayoutMode::Flex);
        assert_eq!(r(&o, "B1"), (gc.x, gc.y, gc.w, 30.0));
        assert_eq!(r(&o, "B2"), (gc.x, gc.y + 30.0, gc.w, gc.h - 30.0));
        assert_eq!(o.placement["B2"], Placement::Item(LayoutMode::Flex));
        assert_eq!(o.placement["G2"], Placement::Item(LayoutMode::Grid));
        let frame = Insets::between(controls[0].rect, controls[0].content_rect());
        assert_eq!(o.min_size, (300.0 + frame.horizontal(), 400.0 + frame.vertical()));
        println!(
            "grid in a Fill panel at 800×500: G1 {:?}, G2 {:?}; flex column in G1: B1 {:?}, B2 {:?}; form minimum {:?}",
            r(&o, "G1"), r(&o, "G2"), r(&o, "B1"), r(&o, "B2"), o.min_size
        );
    }

    /// R53 — a flex-column form is as tall as its content when that is taller
    /// than the window: three 100 px items in a 200 px window lay out at 300
    /// (y 0, 100, 200), none shrunk, and the surface scrolls; the width stays
    /// the window's, every item stretched across it.
    #[test]
    fn a_flex_column_form_is_as_tall_as_its_content() {
        let items: Vec<Control> = (0..3)
            .map(|k| ctrl(&format!("I{k}"), ControlType::Button, (0, k * 100, 80, 100), None))
            .collect();
        let bag = BTreeMap::from([
            ("LayoutMode".to_owned(), PropValue::String("Flex".into())),
            ("FlexDirection".to_owned(), PropValue::String("Column".into())),
        ]);
        let o = solve(&LayoutInput::new(&items, (400.0, 300.0), (500.0, 200.0), &bag, &[]));
        assert_eq!(o.laid_out_size, (500.0, 300.0));
        assert_eq!((r(&o, "I0"), r(&o, "I2")), ((0.0, 0.0, 500.0, 100.0), (0.0, 200.0, 500.0, 100.0)));
    }

    /// T4.5 / R18 — the run-form window's floor: none for a form that is not
    /// responsive; the solver's minimum for one that is, read from the form's
    /// own `MinFormWidth`/`MinFormHeight`; never below 64 × 64, even when the
    /// form asks for less.
    #[test]
    fn a_responsive_forms_window_minimum_is_its_layout_minimum_never_below_64() {
        let mut form = crate::model::Form::new("F", "F", 400, 300);
        form.controls.push(ctrl("B", ControlType::Button, (10, 20, 80, 30), None));
        assert_eq!(min_size_of(&form), None, "not responsive: no minimum, as before");

        form.responsive = true;
        assert_eq!(min_size_of(&form), Some((64.0, 64.0)), "the defaults");

        form.layout.insert("MinFormWidth".into(), PropValue::Int(320));
        form.layout.insert("MinFormHeight".into(), PropValue::Int(240));
        assert_eq!(min_size_of(&form), Some((320.0, 240.0)), "the form's own floor");

        form.layout.insert("MinFormWidth".into(), PropValue::Int(1));
        form.layout.insert("MinFormHeight".into(), PropValue::Int(10));
        assert_eq!(min_size_of(&form), Some((64.0, 64.0)), "never below 64 × 64");
        println!("min_size_of: off → None; defaults → 64×64; 320×240 → 320×240; 1×10 → 64×64");
    }

    #[test]
    fn the_types_and_defaults_are_wired() {
        let controls = vec![ctrl("B", ControlType::Button, (10, 20, 80, 30), None)];
        let o = solve_at(&controls, (400.0, 300.0), (400.0, 300.0));
        assert_eq!(r(&o, "B"), (10.0, 20.0, 80.0, 30.0));
        assert_eq!(o.font_factor, 1.0);
        assert_eq!(o.min_size, (defaults::MIN_FORM_WIDTH as f32, defaults::MIN_FORM_HEIGHT as f32));
        assert_eq!(o.laid_out_size, (400.0, 300.0));
        println!(
            "one Button at the designed size: rect {:?}, factor {}, form minimum {:?}",
            r(&o, "B"),
            o.font_factor,
            o.min_size
        );
    }

    /// AC9 (pure) — a child of a TabControl (each `TabPosition`), a GroupBox
    /// and a Panel lays out inside the container's `content_rect()` computed
    /// from the container's LAID-OUT rectangle, less its `Padding`; outputs are
    /// form-space absolute.
    ///
    /// The container is designed at (50, 50, 300, 200) in a 400×300 form and
    /// anchored on all four edges, so at 500×400 it is (50, 50, 400, 300). Its
    /// child is designed 10 px inside the padded client on every side and
    /// anchored on all four edges too, so it must end 10 px inside the padded
    /// client of the laid-out container.
    #[test]
    fn children_lay_out_inside_the_laid_out_containers_client_less_padding() {
        let cases: Vec<(ControlType, &str)> = vec![
            (ControlType::TabControl, "Top"),
            (ControlType::TabControl, "Bottom"),
            (ControlType::TabControl, "Left"),
            (ControlType::TabControl, "Right"),
            (ControlType::GroupBox, ""),
            (ControlType::Panel, ""),
        ];
        let all = PropValue::String("Top,Bottom,Left,Right".into());
        for (ct, pos) in &cases {
            let mut cont = with(ctrl("K", ct.clone(), (50, 50, 300, 200), None), "Anchor", all.clone());
            cont = with(cont, "Padding", PropValue::Int(6));
            if !pos.is_empty() {
                cont = with(cont, "TabPosition", PropValue::String((*pos).into()));
                cont = with(cont, "Tabs", PropValue::String("General\nAdvanced".into()));
            }
            let padded = |c: &Control| {
                let r = c.content_rect();
                Rect::new(r.x + 6, r.y + 6, r.w - 12, r.h - 12)
            };
            let d = padded(&cont);
            let child = with(
                ctrl("C", ControlType::TextBox, (d.x + 10, d.y + 10, d.w - 20, d.h - 20), Some("K")),
                "Anchor",
                all.clone(),
            );
            let o = solve_at(&[cont.clone(), child], (400.0, 300.0), (500.0, 400.0));
            assert_eq!(r(&o, "K"), (50.0, 50.0, 400.0, 300.0));
            let mut laid = cont.clone();
            laid.rect = Rect::new(50, 50, 400, 300);
            let l = padded(&laid);
            let want = ((l.x + 10) as f32, (l.y + 10) as f32, (l.w - 20) as f32, (l.h - 20) as f32);
            assert_eq!(r(&o, "C"), want, "{ct:?} {pos}");
            assert_eq!(o.containers["K"].client, LRect::from_model(l));
        }
        println!("containers: {} (TabControl ×4 positions, GroupBox, Panel) place their child inside the laid-out client less padding", cases.len());
    }

    #[test]
    fn a_non_visual_control_stays_where_it_was_designed() {
        let controls = vec![with(
            ctrl("T", ControlType::Timer, (500, 5, 32, 32), None),
            "Anchor",
            PropValue::String("Right".into()),
        )];
        let o = solve_at(&controls, (600.0, 400.0), (900.0, 400.0));
        assert_eq!(r(&o, "T"), (500.0, 5.0, 32.0, 32.0));
        assert_eq!(o.placement["T"], Placement::Designed);
    }
}
