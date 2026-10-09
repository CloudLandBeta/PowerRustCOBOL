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
pub mod collide;
pub mod defaults;
pub mod dock;
pub mod flex;
pub mod fonts;
pub mod grid;
pub mod inverse;
pub mod limits;
pub mod minsize;
pub mod props;
pub mod scale;
pub mod tracks;

use std::collections::{BTreeMap, HashMap, HashSet};

use crate::model::{Control, ControlType, PropValue, Rect};
pub use collide::{layout_fingerprint, size_limits_of, window_size_limits, LimitsCache, SizeLimits};
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
    /// Scaled by the window ratio on a form with an obsolete scaling style
    /// (spec 081): `style` is its flags.
    Scaled {
        style: i64,
        designed_parent: LRect,
        parent: LRect,
    },
}

/// A laid-out container, for the designer's overlays (R45).
#[derive(Clone, Debug, PartialEq)]
pub struct ContainerGeom {
    pub mode: LayoutMode,
    /// The client rectangle at the designed size, less padding.
    pub designed_client: LRect,
    /// The client rectangle as laid out, less padding.
    pub client: LRect,
    /// A grid's column widths and row heights as laid out, and its column and
    /// row gaps — what the designer draws as track lines (R45). Empty for any
    /// other mode.
    pub columns: Vec<f32>,
    pub rows: Vec<f32>,
    pub gaps: (f32, f32),
}

/// Everything the solver reads.
#[derive(Clone)]
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
    /// Controls a breakpoint shows although they were designed hidden.
    pub shown: HashSet<String>,
    /// Spec 090 — the siblings of an expanded Panel or GroupBox: they give
    /// their room up, and nothing of them (or inside them) is drawn.
    pub expanded_away: HashSet<String>,
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

/// Spec 091 R21 — for each control, whether it sits in a layer. Only the base has
/// layout behaviour (`Dock`, `Anchor` and the flex, grid and flow layouts): a
/// control in a layer keeps its designed rectangle, and so does everything
/// inside a container in a layer (R8) — the answer is the outermost container's
/// `layer`.
///
/// A name other than `Form` counts as a layer here whether or not the form
/// defines it. A control that names an undefined layer is DRAWN with the base
/// (R40) but keeps the rectangle it was designed at, which is the safer thing to
/// do with a control whose layer is lost than to dock it into a layout it was
/// never designed for.
pub(crate) fn layered_flags(controls: &[Control]) -> Vec<bool> {
    if controls.iter().all(|c| c.layer.is_none()) {
        return vec![false; controls.len()];
    }
    let by_id: HashMap<&str, usize> = controls
        .iter()
        .enumerate()
        .map(|(i, c)| (c.id.as_str(), i))
        .collect();
    controls
        .iter()
        .map(|c| {
            // A hand-edited file can hold a parent cycle: stop after one lap.
            let mut cur = c;
            for _ in 0..=controls.len() {
                match cur.parent.as_deref().and_then(|p| by_id.get(p)) {
                    Some(&i) => cur = &controls[i],
                    None => break,
                }
            }
            cur.layer
                .as_deref()
                .is_some_and(|l| !l.eq_ignore_ascii_case(crate::model::BASE_LAYER_NAME))
        })
        .collect()
}

impl Tree {
    pub(crate) fn new(controls: &[Control]) -> Tree {
        let ids: HashSet<&str> = controls.iter().map(|c| c.id.as_str()).collect();
        let layered = layered_flags(controls);
        let mut kids: HashMap<Option<String>, Vec<usize>> = HashMap::new();
        for (i, c) in controls.iter().enumerate() {
            // A control in a layer is not laid out at all — and, being left out
            // of the tree, neither is anything inside it (R21).
            if layered[i] {
                continue;
            }
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

/// The TabControl `parent` is, or `None` for the form or any other parent.
fn tab_control<'c>(controls: &'c [Control], parent: Option<&str>) -> Option<&'c Control> {
    let p = parent?;
    controls
        .iter()
        .find(|c| c.id == p)
        .filter(|c| c.control_type == ControlType::TabControl)
}

/// A TabControl child's page: its `tab` (from 1), the first page when unset.
pub(crate) fn page_of(c: &Control) -> u32 {
    c.tab.unwrap_or(1)
}

/// The sibling sets `parent`'s children `kids` lay out as: one per page of a
/// TabControl, each page alone in the whole client area — a page's docks,
/// anchors, flex, grid and flow never see another page's controls — and one
/// set of all of them for any other parent. In the order given, per set.
pub(crate) fn page_sets(controls: &[Control], parent: Option<&str>, kids: &[usize]) -> Vec<Vec<usize>> {
    if tab_control(controls, parent).is_none() {
        return vec![kids.to_vec()];
    }
    let mut pages: BTreeMap<u32, Vec<usize>> = BTreeMap::new();
    for &i in kids {
        pages.entry(page_of(&controls[i])).or_default().push(i);
    }
    pages.into_values().collect()
}

/// Whether `a` and `b` are laid out in the same sibling set: the same
/// parent and, under a TabControl, the same page.
pub(crate) fn same_layout_set(controls: &[Control], a: &Control, b: &Control) -> bool {
    a.parent == b.parent && (tab_control(controls, a.parent.as_deref()).is_none() || page_of(a) == page_of(b))
}

/// Whether `c` is on the page its parent shows at the designed state
/// (`SelectedTab`) — always, for a parent that is not a TabControl. The
/// designer's grid track lines are that page's.
fn shown_page(controls: &[Control], parent: Option<&str>, c: &Control) -> bool {
    tab_control(controls, parent).is_none_or(|t| {
        let sel = t.get_prop("SelectedTab").map(|v| v.as_i64()).unwrap_or(1).max(1);
        page_of(c) as i64 == sel
    })
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
    form.lays_out().then(|| {
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
    let mut input = LayoutInput::new(controls, designed, designed, form_props, breakpoints);
    // The narrowest breakpoint's layout is the smallest the form can be.
    input.pinned_breakpoint = breakpoints::narrowest(breakpoints).map(|b| b.name.as_str());
    let (w, h) = solve(&input).min_size;
    (w.max(defaults::WINDOW_MIN_INNER), h.max(defaults::WINDOW_MIN_INNER))
}

/// Lay the form out for its surface (R22).
pub fn solve(input: &LayoutInput<'_>) -> LayoutOutput {
    // The active breakpoint first, and its overrides before anything reads a
    // control (R26 step 1, R59–R62).
    let bp = breakpoints::select(input.breakpoints, input.available.0, input.pinned_breakpoint);
    match breakpoints::apply_overrides(input.controls, bp) {
        Some(seen) => {
            let mut at = input.clone();
            at.controls = &seen.controls;
            let mut out = solve_at_breakpoint(&at, bp);
            out.hidden = seen.hidden;
            out.shown = seen.shown;
            out
        }
        None => solve_at_breakpoint(input, bp),
    }
}

fn solve_at_breakpoint(input: &LayoutInput<'_>, bp: Option<&Breakpoint>) -> LayoutOutput {
    let tree = Tree::new(input.controls);
    let min_size = minsize::form_min(input, &tree, bp);
    lay_out(input, &tree, bp, min_size)
}

/// Lay the form out for its surface, given the form's minimum size: the
/// surface is never laid out smaller. [`minsize::form_min`] also asks this
/// for the layout AT a candidate minimum.
pub(crate) fn lay_out(
    input: &LayoutInput<'_>,
    tree: &Tree,
    bp: Option<&Breakpoint>,
    min_size: (f32, f32),
) -> LayoutOutput {
    let form = FormBag(input.form_props);
    // Spec 081 — a form with an obsolete scaling style takes its font from
    // the window ratio instead of `FontScaling`.
    let scaling = scale::style(&form);
    let font_factor = if scale::scales_font(scaling) {
        scale::font_factor(
            &form,
            input.designed_size,
            input.available,
            input.system_text_factor,
            input.pinned_font_scale,
        )
    } else {
        fonts::form_factor(
            &form,
            input.designed_size.0,
            input.available.0,
            bp,
            input.system_text_factor,
            input.pinned_font_scale,
        )
    };

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
    // A control in a layer is laid out nowhere, so its text is not scaled with a
    // layout either: it keeps its designed size, as its rectangle does (R21).
    let layered = layered_flags(input.controls);
    for (c, &in_layer) in input.controls.iter().zip(&layered) {
        if !in_layer {
            out.font_sizes
                .insert(c.id.clone(), fonts::effective_size(c, font_factor));
        }
    }
    let mode = props::layout_mode(&form);
    let content = place_children(input, tree, None, &form, mode, designed_client, client, false, &mut out);
    // A flex, grid or flow form is as tall as its content when that is
    // taller than the window, as a document is (R53): it lays out again at
    // that height and the surface scrolls. Its width stays the window's.
    if let Some((_, ch)) = content {
        let need = ch + pad.vertical();
        if need > out.laid_out_size.1 + defaults::EPSILON {
            out.laid_out_size.1 = need;
            let client = LRect::new(0.0, 0.0, out.laid_out_size.0, need).deflate(pad);
            place_children(input, tree, None, &form, mode, designed_client, client, false, &mut out);
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
    proportional: bool,
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

    // A TabControl's pages are separate layout sets, each in the whole
    // client: a page's docks, anchors and items never see another page's.
    let mut content: Option<(f32, f32)> = None;
    for set in page_sets(input.controls, parent, &visual) {
        let tracks = set.first().is_none_or(|&i| shown_page(input.controls, parent, &input.controls[i]));
        if let Some((w, h)) = place_set(input, parent, src, mode, &set, designed_client, client, tracks, proportional, out) {
            let (cw, ch) = content.unwrap_or((0.0, 0.0));
            content = Some((cw.max(w), ch.max(h)));
        }
        expand(input, &set, client, out);
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
                    columns: Vec::new(),
                    rows: Vec::new(),
                    gaps: (0.0, 0.0),
                },
            );
            // Everything inside an expanded card grows with it: its contents
            // — and theirs — are scaled with the card, not left to their anchors.
            let stretched = proportional || (props::expanded(c) && !out.expanded_away.contains(&c.id));
            place_children(input, tree, Some(&c.id), c, cmode, dclient, lclient, stretched, out);
        } else if c.control_type == ControlType::Splitter {
            carry_splitter(input, tree, c, designed_rect(input, c), laid, out);
        } else {
            let d = designed_rect(input, c);
            carry_rigidly(input, tree, &c.id, laid.x - d.x, laid.y - d.y, out);
        }
    }
    content
}

/// Spec 090 — the Panel or GroupBox of one sibling set that is expanded takes
/// the whole client rectangle, the room all its siblings had between them, and
/// the siblings drop out. The contents of the expanded control are laid out
/// afterwards, against its new rectangle, like at any other size.
///
/// `set` is in z-order, so when several are expanded the topmost wins.
fn expand(input: &LayoutInput<'_>, set: &[usize], client: LRect, out: &mut LayoutOutput) {
    let Some(&winner) = set.iter().rev().find(|&&i| props::expanded(&input.controls[i])) else {
        return;
    };
    out.rects.insert(input.controls[winner].id.clone(), client);
    for &i in set.iter().filter(|&&i| i != winner) {
        out.expanded_away.insert(input.controls[i].id.clone());
    }
}

/// Place one sibling set of `parent`'s visual children — all of them, or one
/// TabControl page's — inside the client rectangle: by its flex, grid or flow
/// layout, or by docking and anchoring. `tracks` records a grid's tracks for
/// the designer. Returns what a flex, grid or flow set needs.
#[allow(clippy::too_many_arguments)]
fn place_set(
    input: &LayoutInput<'_>,
    parent: Option<&str>,
    src: &dyn PropSource,
    mode: LayoutMode,
    visual: &[usize],
    designed_client: LRect,
    client: LRect,
    tracks: bool,
    proportional: bool,
    out: &mut LayoutOutput,
) -> Option<(f32, f32)> {
    // A flex, grid or flow container places its children itself (R50).
    let content = (mode != LayoutMode::Absolute).then(|| place_items(input, parent, src, mode, visual, client, tracks, out));
    // Spec 081 — on a scaling form, every control the developer did not
    // anchor or dock on purpose follows the window ratio.
    //
    // Inside an expanded card (spec 090) every control does — position and size
    // — whatever its anchor says, so that all of the card's contents grow with it.
    let scaling = if proportional {
        defaults::SCALING_RESIZE | defaults::SCALING_REPOSITION
    } else {
        scale::style(&FormBag(input.form_props))
    };
    let scaled = |c: &Control| scaling != 0 && (proportional || !scale::opted_out(c));

    // Docked controls first, in z-order, each taking an edge of what remains
    // (R12); every other control is anchored against the FULL client rect
    // (R14).
    let docked: Vec<(usize, Dock)> = if content.is_some() { Vec::new() } else { visual.to_vec() }
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
    for &i in visual {
        let c = &input.controls[i];
        // Placed above, by its dock or its flex/grid/flow parent.
        if content.is_some() || docked.iter().any(|(d, _)| *d == i) {
            continue;
        }
        if scaled(c) {
            let r = scale::place(
                designed_rect(input, c),
                designed_client,
                client,
                scaling,
                props::width_limits(c),
                props::height_limits(c),
            );
            out.rects.insert(c.id.clone(), r);
            out.placement.insert(
                c.id.clone(),
                Placement::Scaled { style: scaling, designed_parent: designed_client, parent: client },
            );
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
    content
}

/// `kids` in reading order (R54): top to bottom, then left to right — right
/// to left for a right-to-left container — by their designed rectangles.
pub(crate) fn reading_order(input: &LayoutInput<'_>, kids: &[usize], rtl: bool) -> Vec<usize> {
    let mut order: Vec<usize> = kids.to_vec();
    order.sort_by(|&a, &b| {
        let (ra, rb) = (designed_rect(input, &input.controls[a]), designed_rect(input, &input.controls[b]));
        let x = if rtl { rb.x.total_cmp(&ra.x) } else { ra.x.total_cmp(&rb.x) };
        ra.y.total_cmp(&rb.y).then(x)
    });
    order
}

/// Place the children of a `Flex`, `Flow` or `Grid` container (R50): each at
/// its designed (or measured) size as its intrinsic size, in reading order —
/// top to bottom, then left to right, or right to left for a right-to-left
/// container (R54). `Anchor` and `Dock` are not read. Returns what the items
/// need, width and height.
fn place_items(
    input: &LayoutInput<'_>,
    parent: Option<&str>,
    src: &dyn PropSource,
    mode: LayoutMode,
    kids: &[usize],
    client: LRect,
    tracks: bool,
    out: &mut LayoutOutput,
) -> (f32, f32) {
    let flex_c = match mode {
        LayoutMode::Flex => Some(flex::flex_container(src)),
        LayoutMode::Flow => Some(flex::flow_container(src)),
        _ => None,
    };
    let rtl = flex_c.is_some_and(|f| f.direction == flex::Direction::RowReverse);
    let order = reading_order(input, kids, rtl);
    let size = |i: usize| {
        let r = designed_rect(input, &input.controls[i]);
        (r.w, r.h)
    };
    let (rects, content) = match (mode, flex_c) {
        (LayoutMode::Grid, _) => {
            let g = grid::container(src);
            let items: Vec<grid::Item> = order.iter().map(|&i| grid::item(&input.controls[i], size(i))).collect();
            let s = grid::solve(&g, &items, client);
            if let Some(geom) = parent.filter(|_| tracks).and_then(|p| out.containers.get_mut(p)) {
                geom.columns = s.columns.clone();
                geom.rows = s.rows.clone();
                geom.gaps = (g.column_gap, g.row_gap);
            }
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
/// takes its place in the LAID-OUT splitter's geometry — the splitter owns
/// the panes — and lays its own children out inside that rect as any
/// container does (anchors, docks, or its flex/grid/flow `LayoutMode`). The
/// render, which derives the panes from the splitter it is given, finds every
/// pane where the layout put it and moves nothing again; only a divider moved
/// at run time reflows a pane's contents, by the pane's `ResizeBehavior`.
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
        let (b, a) = (LRect::from_model(b), LRect::from_model(a));
        out.rects.insert(pane.id.clone(), a);
        out.placement.insert(pane.id.clone(), Placement::Rigid { dx: a.x - b.x, dy: a.y - b.y });
        if tree.children(Some(&pane.id)).is_empty() {
            continue;
        }
        // The pane is a container like any other: its children are anchored,
        // docked or placed by its `LayoutMode` from the DESIGNED pane to the
        // pane this splitter gives it.
        let (dclient, lclient) = (client_of(pane, b), client_of(pane, a));
        let mode = props::layout_mode(pane);
        out.containers.insert(
            pane.id.clone(),
            ContainerGeom {
                mode,
                designed_client: dclient,
                client: lclient,
                columns: Vec::new(),
                rows: Vec::new(),
                gaps: (0.0, 0.0),
            },
        );
        place_children(input, tree, Some(&pane.id), pane, mode, dclient, lclient, false, out);
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

    /// AC28 (R58–R62) — a form designed at 1280 × 800 (Expanded):
    ///   SIDE, a `Left` dock 200 wide; MAIN, the `Fill` rest, a grid `1fr 1fr`
    ///   holding C1, C2 and PNL (which holds PC), in that reading order.
    ///   Compact: SIDE `Dock = Top`, `Height = 60`; MAIN one column; PNL
    ///   hidden. Medium: MAIN one column.
    /// 1280: SIDE left; C1 and C2 side by side, PNL on row 2.
    /// 800:  SIDE left; C1, C2, PNL stacked.
    /// 480:  SIDE across the top, 60 high; C1, C2 stacked and sharing MAIN's
    ///       height — PNL takes no cell, and it and PC are reported hidden.
    #[test]
    fn breakpoint_overrides_change_docks_grids_and_visibility() {
        let s = |v: &str| PropValue::String(v.into());
        let mut side = with(ctrl("SIDE", ControlType::Panel, (0, 0, 200, 800), None), "Dock", s("Left"));
        side.z_order = 1;
        let mut main = with(ctrl("MAIN", ControlType::Panel, (200, 0, 1080, 800), None), "Dock", s("Fill"));
        main = with(with(main, "LayoutMode", s("Grid")), "GridColumns", s("1fr 1fr"));
        main.z_order = 2;
        let controls = [
            side,
            main,
            ctrl("C1", ControlType::Label, (210, 10, 100, 40), Some("MAIN")),
            ctrl("C2", ControlType::Label, (700, 10, 100, 40), Some("MAIN")),
            ctrl("PNL", ControlType::Panel, (210, 100, 100, 40), Some("MAIN")),
            ctrl("PC", ControlType::Label, (215, 105, 50, 20), Some("PNL")),
        ];
        let o = |c: &str, p: &str, v: PropValue| breakpoints::Override { control: c.into(), property: p.into(), value: v };
        let mut table = defaults::default_breakpoints();
        table[0].overrides = vec![
            o("SIDE", "Dock", s("Top")),
            o("SIDE", "Height", PropValue::Int(60)),
            o("MAIN", "GridColumns", s("1fr")),
            o("PNL", "Visible", PropValue::Bool(false)),
        ];
        table[1].overrides = vec![o("MAIN", "GridColumns", s("1fr"))];
        let bag = BTreeMap::new();
        let at = |w: f32, h: f32| solve(&LayoutInput::new(&controls, (1280.0, 800.0), (w, h), &bag, &table));

        let wide = at(1280.0, 800.0);
        assert_eq!(wide.breakpoint, "Expanded");
        assert_eq!(r(&wide, "SIDE"), (0.0, 0.0, 200.0, 800.0));
        let mc = wide.containers["MAIN"].client;
        assert_eq!((r(&wide, "C1").0, r(&wide, "C2").0), (mc.x, mc.x + mc.w / 2.0));
        assert_eq!(r(&wide, "PNL").0, mc.x);
        assert!(r(&wide, "PNL").1 > r(&wide, "C1").1 && wide.hidden.is_empty());

        let mid = at(800.0, 600.0);
        assert_eq!(mid.breakpoint, "Medium");
        assert_eq!(r(&mid, "SIDE"), (0.0, 0.0, 200.0, 600.0));
        let mc = mid.containers["MAIN"].client;
        assert_eq!((r(&mid, "C1").0, r(&mid, "C2").0, r(&mid, "PNL").0), (mc.x, mc.x, mc.x));
        assert!(r(&mid, "C1").1 < r(&mid, "C2").1 && r(&mid, "C2").1 < r(&mid, "PNL").1);

        let narrow = at(480.0, 600.0);
        assert_eq!(narrow.breakpoint, "Compact");
        assert_eq!(r(&narrow, "SIDE"), (0.0, 0.0, 480.0, 60.0));
        assert_eq!(r(&narrow, "MAIN"), (0.0, 60.0, 480.0, 540.0));
        let mc = narrow.containers["MAIN"].client;
        let c2 = r(&narrow, "C2");
        assert_eq!((c2.0, c2.1 + c2.3), (mc.x, mc.y + mc.h), "two rows share the height: PNL takes no cell");
        assert!(!narrow.rects.contains_key("PNL") && !narrow.rects.contains_key("PC"));
        assert_eq!(narrow.hidden, HashSet::from(["PNL".to_owned(), "PC".to_owned()]));

        // The window's floor is the narrowest breakpoint's layout (R18).
        let mut pinned = LayoutInput::new(&controls, (1280.0, 800.0), (1280.0, 800.0), &bag, &table);
        pinned.pinned_breakpoint = Some("Compact");
        let compact_min = solve(&pinned).min_size;
        assert_eq!(
            window_min_size(&controls, (1280.0, 800.0), &bag, &table),
            (compact_min.0.max(defaults::WINDOW_MIN_INNER), compact_min.1.max(defaults::WINDOW_MIN_INNER))
        );
        println!(
            "AC28: 1280 → Expanded (C2 beside C1), 800 → Medium (stacked, PNL row 3), 480 → Compact (SIDE {:?} on top, PNL + PC hidden)",
            r(&narrow, "SIDE")
        );
    }

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

    /// Spec 090 (AC1, AC2, R7) — a grid of four cards, two columns by two rows,
    /// at 800 × 600 with a 20 px gap. Expand B: it takes the whole client of the
    /// grid, its three siblings and what is inside them are given up, and the
    /// label inside B, anchored `Top,Left,Right,Bottom`, follows B's new size.
    /// Collapse it: everything is back in its cell. The topmost of two expanded
    /// siblings wins.
    #[test]
    fn an_expanded_card_takes_the_room_of_its_siblings() {
        let s = |v: &str| PropValue::String(v.into());
        let mut grid = with(ctrl("G", ControlType::Panel, (0, 0, 800, 600), None), "LayoutMode", s("Grid"));
        grid = with(with(with(grid, "GridColumns", s("1fr 1fr")), "GridRows", s("1fr 1fr")), "Gap", PropValue::Int(20));
        let card = |id: &str, x: i32, y: i32, z: i32| {
            let mut c = ctrl(id, ControlType::GroupBox, (x, y, 100, 100), Some("G"));
            c.z_order = z;
            c
        };
        let mut inner = ctrl("IN", ControlType::Label, (130, 20, 60, 20), Some("B"));
        inner.set_prop("Anchor", s("Top,Left,Right,Bottom"));
        let build = |expanded: &[&str]| {
            let mut v = vec![grid.clone(), card("A", 0, 0, 1), card("B", 120, 0, 2), card("C", 0, 120, 3), card("D", 120, 120, 4), inner.clone()];
            for c in v.iter_mut().filter(|c| expanded.contains(&c.id.as_str())) {
                c.set_prop("Expanded", PropValue::Bool(true));
            }
            v
        };

        let collapsed = solve_at(&build(&[]), (800.0, 600.0), (800.0, 600.0));
        assert!(collapsed.expanded_away.is_empty());
        let cell_b = r(&collapsed, "B");
        assert!(cell_b.2 < 400.0 && cell_b.3 < 300.0, "B is one cell: {cell_b:?}");

        let open = solve_at(&build(&["B"]), (800.0, 600.0), (800.0, 600.0));
        let client = open.containers["G"].client;
        assert_eq!(r(&open, "B"), (client.x, client.y, client.w, client.h), "B takes the whole client");
        let mut away: Vec<&str> = open.expanded_away.iter().map(String::as_str).collect();
        away.sort_unstable();
        assert_eq!(away, ["A", "C", "D"], "its three siblings give their room up");
        let g = &open.containers["B"];
        let (rx, ry) = (g.client.w / g.designed_client.w, g.client.h / g.designed_client.h);
        let after = r(&open, "IN");
        assert!(
            (after.2 - 60.0 * rx).abs() < 1.0 && (after.3 - 20.0 * ry).abs() < 1.0,
            "what is inside B follows its size, share for share: {after:?} for x{rx} y{ry}"
        );

        let two = solve_at(&build(&["A", "D"]), (800.0, 600.0), (800.0, 600.0));
        assert_eq!(r(&two, "D"), (client.x, client.y, client.w, client.h), "the topmost wins");
        assert!(two.expanded_away.contains("A") && !two.expanded_away.contains("D"));
    }

    /// Spec 090 — what is inside an expanded card grows WITH it, whatever each
    /// control's anchor says: position and size, at every depth (operator,
    /// 2026-10-08: "some elements resize when their container expands, others
    /// don't"). A card of 100 × 100 holds, at the default top-left anchor, a label
    /// and a panel with a label inside it; expanded to 800 × 600 they all follow
    /// the card's 8 × 6. The same card collapsed keeps them anchored.
    #[test]
    fn everything_inside_an_expanded_card_grows_with_it() {
        let s = |v: &str| PropValue::String(v.into());
        let mut grid = with(ctrl("G", ControlType::Panel, (0, 0, 800, 600), None), "LayoutMode", s("Grid"));
        grid = with(with(with(grid, "GridColumns", s("1fr 1fr")), "GridRows", s("1fr 1fr")), "Gap", PropValue::Int(0));
        let card = |id: &str, x: i32, y: i32| {
            let mut c = ctrl(id, ControlType::Panel, (x, y, 100, 100), Some("G"));
            c.set_prop("BorderStyle", s("None"));
            c
        };
        let build = |expanded: bool| {
            let mut b = card("B", 100, 0);
            b.set_prop("Expanded", PropValue::Bool(expanded));
            let mut label = ctrl("L", ControlType::Label, (110, 20, 40, 10), Some("B")); // top-left anchored
            label.set_prop("Anchor", s("Top,Left"));
            let mut inner = ctrl("P", ControlType::Panel, (150, 50, 40, 40), Some("B"));
            inner.set_prop("BorderStyle", s("None"));
            let mut deep = ctrl("Q", ControlType::Label, (160, 60, 20, 10), Some("P"));
            deep.set_prop("Anchor", s("Top,Left"));
            vec![grid.clone(), card("A", 0, 0), b, card("C", 0, 100), card("D", 100, 100), label, inner, deep]
        };
        let shut = solve_at(&build(false), (800.0, 600.0), (800.0, 600.0));
        let open = solve_at(&build(true), (800.0, 600.0), (800.0, 600.0));
        let near = |got: f32, want: f32| (got - want).abs() <= 1.0;
        let l0 = r(&shut, "L");
        assert_eq!((l0.2, l0.3), (40.0, 10.0), "collapsed, the anchored label keeps its size: {l0:?}");

        // The card's contents: each at its share of the card's client.
        let b = &open.containers["B"];
        let (rx, ry) = (b.client.w / b.designed_client.w, b.client.h / b.designed_client.h);
        assert!(rx > 5.0 && ry > 4.0, "the card really grew: x{rx} y{ry}");
        let l = r(&open, "L");
        let want = (b.client.x + (110.0 - b.designed_client.x) * rx, b.client.y + (20.0 - b.designed_client.y) * ry);
        assert!(near(l.0, want.0) && near(l.1, want.1), "the label sits at its share of the card: {l:?}, want {want:?}");
        assert!(near(l.2, 40.0 * rx) && near(l.3, 10.0 * ry), "…and is that much bigger: {l:?} (x{rx}, y{ry})");
        let p = r(&open, "P");
        assert!(near(p.2, 40.0 * rx) && near(p.3, 40.0 * ry), "the panel inside grows too: {p:?}");

        // …and what is inside THAT panel, a level deeper.
        let inner = &open.containers["P"];
        let (px, py) = (inner.client.w / inner.designed_client.w, inner.client.h / inner.designed_client.h);
        let q = r(&open, "Q");
        assert!(near(q.2, 20.0 * px) && near(q.3, 10.0 * py), "a level deeper still follows: {q:?} (x{px}, y{py})");
        assert!(px > 5.0 && py > 4.0, "…and that panel's own ratio is the card's: x{px} y{py}");
    }

    /// Only a Panel or a GroupBox expands; a control of any other type carrying
    /// the property is just placed.
    #[test]
    fn only_a_panel_or_a_groupbox_expands() {
        let mut grid = with(ctrl("G", ControlType::Panel, (0, 0, 400, 200), None), "LayoutMode", PropValue::String("Flex".into()));
        grid.set_prop("Gap", PropValue::Int(0));
        let mut label = ctrl("L", ControlType::Label, (0, 0, 100, 20), Some("G"));
        label.set_prop("Expanded", PropValue::Bool(true));
        let o = solve_at(&[grid, label, ctrl("M", ControlType::Label, (100, 0, 100, 20), Some("G"))], (400.0, 200.0), (400.0, 200.0));
        assert!(o.expanded_away.is_empty(), "a Label never expands");
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

    /// Each page of a TabControl is its own layout set: a `Fill` Panel on
    /// page 0 and another on page 1 both take the WHOLE client area (they
    /// used to share one docking space, so page 1's got 0 × 0); a `Flex`
    /// column TabControl starts every page's items at the client's top; and
    /// the form minimum takes the largest page, not the sum of the pages.
    #[test]
    fn every_tab_page_is_its_own_layout_set() {
        let s = |v: &str| PropValue::String(v.into());
        let tabs = |id: &str| {
            let t = with(ctrl(id, ControlType::TabControl, (0, 0, 400, 300), None), "Dock", s("Fill"));
            with(t, "Tabs", s("One\nTwo"))
        };
        let page = |mut c: Control, n: u32, z: i32| {
            c.tab = Some(n);
            c.z_order = z;
            c
        };
        let k = tabs("K");
        let d = k.content_rect();
        let a = page(with(ctrl("A", ControlType::Panel, (d.x, d.y, 50, 50), Some("K")), "Dock", s("Fill")), 1, 1);
        let b = page(with(ctrl("B", ControlType::Panel, (d.x, d.y, 50, 50), Some("K")), "Dock", s("Fill")), 2, 2);
        let o = solve_at(&[k, a, b], (400.0, 300.0), (600.0, 500.0));
        let kc = o.containers["K"].client;
        let full = (kc.x, kc.y, kc.w, kc.h);
        assert!(kc.w > 0.0 && kc.h > 0.0);
        assert_eq!((r(&o, "A"), r(&o, "B")), (full, full), "both pages fill the client");

        // Flex column: page 1's first item starts at the top, not below page 0's.
        let k = with(with(tabs("K"), "LayoutMode", s("Flex")), "FlexDirection", s("Column"));
        let i0 = page(ctrl("I0", ControlType::Button, (d.x, d.y, 80, 40), Some("K")), 0, 1);
        let i1 = page(ctrl("I1", ControlType::Button, (d.x, d.y + 40, 80, 40), Some("K")), 1, 2);
        let o = solve_at(&[k, i0, i1], (400.0, 300.0), (400.0, 300.0));
        let kc = o.containers["K"].client;
        assert_eq!((r(&o, "I0").1, r(&o, "I1").1), (kc.y, kc.y), "every page starts at the top");

        // The minimum is the largest page: two 300-wide Left docks on
        // different pages need 300, not 600.
        let k = tabs("K");
        let frame = Insets::between(k.rect, k.content_rect());
        let l0 = page(with(with(ctrl("L0", ControlType::Panel, (d.x, d.y, 300, 50), Some("K")), "Dock", s("Left")), "MinHeight", PropValue::Int(10)), 0, 1);
        let l1 = page(with(with(ctrl("L1", ControlType::Panel, (d.x, d.y, 300, 50), Some("K")), "Dock", s("Left")), "MinHeight", PropValue::Int(10)), 1, 2);
        let o = solve_at(&[k, l0, l1], (400.0, 300.0), (100.0, 100.0));
        assert_eq!(o.min_size.0, (300.0 + frame.horizontal()).max(defaults::MIN_FORM_WIDTH as f32));
        println!("tab pages: Fill on page 0 and page 1 both {full:?}; flex pages start at y {}; minimum width {}", kc.y, o.min_size.0);
    }

    /// R26/R27 — a Splitter places its panes; a pane lays out its OWN
    /// children inside the pane's actual rect. The splitter, designed at
    /// (20, 60, 300, 200) in a 600 × 400 form and anchored `Bottom,Left,Right`,
    /// moves down 100 and widens 200 at 800 × 500. In pane 1 a `Dock = Fill`
    /// panel takes the whole pane; in pane 2 a `Top,Left` label keeps its
    /// 10, 10 offset from the pane on BOTH axes, and a `Top,Left,Right` field
    /// stretches with the pane.
    #[test]
    fn a_splitter_panes_children_are_laid_out_in_the_pane() {
        let s = |v: &str| PropValue::String(v.into());
        let mut form = crate::model::Form::new("F", "F", 600, 400);
        form.controls.push(with(ctrl("S", ControlType::Splitter, (20, 60, 300, 200), None), "Anchor", s("Bottom,Left,Right")));
        form.sync_splitter_panes();
        let (n1, n2) = (crate::splitter::pane_id("S", 1), crate::splitter::pane_id("S", 2));
        let p1 = form.find_control(&n1).unwrap().rect;
        let p2 = form.find_control(&n2).unwrap().rect;
        form.controls.push(with(ctrl("FILL", ControlType::Panel, (p1.x + 5, p1.y + 5, 40, 40), Some(&n1)), "Dock", s("Fill")));
        form.controls.push(ctrl("LBL", ControlType::Label, (p2.x + 10, p2.y + 10, 60, 20), Some(&n2)));
        form.controls.push(with(
            ctrl("FLD", ControlType::TextBox, (p2.x + 10, p2.y + 40, p2.w - 20, 24), Some(&n2)),
            "Anchor",
            s("Top,Left,Right"),
        ));
        let o = solve_at(&form.controls, (600.0, 400.0), (800.0, 500.0));
        assert_eq!(r(&o, "S"), (20.0, 160.0, 500.0, 200.0));
        let pane1 = o.rects[&n1];
        let pane2 = o.rects[&n2];
        let c1 = client_of(form.find_control(&n1).unwrap(), pane1);
        assert_eq!(r(&o, "FILL"), (c1.x, c1.y, c1.w, c1.h), "Fill takes pane 1 ({pane1:?})");
        assert_eq!((r(&o, "LBL").0 - pane2.x, r(&o, "LBL").1 - pane2.y), (10.0, 10.0), "the label keeps its offset in pane 2 ({pane2:?})");
        let fld = r(&o, "FLD");
        assert_eq!((fld.0 - pane2.x, fld.1 - pane2.y, pane2.right() - (fld.0 + fld.2)), (10.0, 40.0, 10.0), "the field stretches with pane 2");
        // The render derives the panes from the laid-out splitter and finds
        // them already there: the geometry the solver used.
        let mut laid = form.controls[0].clone();
        laid.rect = Rect::new(20, 160, 500, 200);
        let g = crate::splitter::geometry(&laid, laid.rect);
        assert_eq!((pane1, pane2), (LRect::from_model(g.pane1), LRect::from_model(g.pane2)));
        println!("splitter at 800×500: pane 1 {pane1:?} FILL {:?}; pane 2 {pane2:?} LBL {:?} FLD {fld:?}", r(&o, "FILL"), r(&o, "LBL"));
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
