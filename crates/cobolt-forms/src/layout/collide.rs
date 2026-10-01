// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! **The window sizes at which nothing collides** (spec 056 R18, extended).
//!
//! The form minimum ([`super::minsize`]) only counts the limits a control
//! declares — `MinWidth`, `MinHeight`. A form that declares none can be
//! shrunk until a `Right`-anchored button slides over a `Left`-anchored field,
//! and a `Left,Right` field can be stretched over a `Left`-anchored neighbour.
//! Both are layouts the developer never drew.
//!
//! So the window stops there. Two sibling controls that are **apart** at the
//! designed size must never **touch** at any size the window accepts: shrink
//! or grow past the first size where they would, and the window does not go.
//! Siblings that touch or overlap in the design (a label on its card, two
//! flush toolbar buttons) are the developer's choice and are not constrained.
//!
//! Likewise a visible control that is fully **inside** its parent's client
//! area (the form's, at top level) at the designed size must never cross
//! that edge: a `Top,Left` button near the right of a form stops the window
//! narrowing before it would be cut off. A control that already overflows its
//! parent in the design is not constrained.
//!
//! Searched, not derived: the solver is the single source of truth, so this
//! asks it at candidate sizes rather than re-deriving anchoring, docking,
//! flex and breakpoints a second time.

use std::collections::BTreeMap;

use crate::layout::breakpoints::Breakpoint;
use crate::layout::{same_layout_set, solve, LRect, LayoutInput};
use crate::model::{Control, PropValue};

use crate::layout::defaults::{
    COLLIDE_GROW_CAP as GROW_CAP, COLLIDE_GROW_FIRST_STEP, COLLIDE_REFINE_ROUNDS as REFINE,
    COLLIDE_SHRINK_STEP as SHRINK_STEP, COLLIDE_TOUCH_EPS,
};

/// The window sizes a responsive form accepts.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SizeLimits {
    /// The smallest inner size: the declared minimum, raised to where two
    /// apart controls would first touch.
    pub min: (f32, f32),
    /// The largest inner size per axis; `f32::INFINITY` where growing never
    /// makes two controls touch.
    pub max: (f32, f32),
}

/// The pairs of visible sibling controls that are apart at the designed size
/// — the ones that must stay apart. Controls on different pages of a
/// TabControl are never on screen together, so they are not siblings here.
fn apart_pairs(controls: &[Control], rects: &std::collections::HashMap<String, LRect>, hidden: &std::collections::HashSet<String>) -> Vec<(String, String)> {
    let shown: Vec<&Control> = controls
        .iter()
        .filter(|c| c.visible && !c.control_type.is_non_visual() && !hidden.contains(&c.id) && rects.contains_key(&c.id))
        .collect();
    let mut pairs = Vec::new();
    for (i, a) in shown.iter().enumerate() {
        for b in shown.iter().skip(i + 1) {
            if same_layout_set(controls, a, b) && !touching(&rects[&a.id], &rects[&b.id]) {
                pairs.push((a.id.clone(), b.id.clone()));
            }
        }
    }
    pairs
}

/// The client rectangle `parent` lays its children out in, in layout `o`:
/// the form's laid-out area for `None`, a laid-out container's client, or
/// `None` where no container geometry is recorded (a Splitter, a SideMenu, a
/// repeating group carry their contents themselves).
fn parent_client(o: &crate::layout::LayoutOutput, parent: Option<&str>) -> Option<LRect> {
    match parent {
        None => Some(LRect::new(0.0, 0.0, o.laid_out_size.0, o.laid_out_size.1)),
        Some(p) => o.containers.get(p).map(|g| g.client),
    }
}

/// Whether `r` lies inside `client`, edges included.
fn inside(r: &LRect, client: &LRect) -> bool {
    r.x >= client.x - COLLIDE_TOUCH_EPS
        && r.y >= client.y - COLLIDE_TOUCH_EPS
        && r.right() <= client.right() + COLLIDE_TOUCH_EPS
        && r.bottom() <= client.bottom() + COLLIDE_TOUCH_EPS
}

/// Whether `parent` (the form for `None`) is a `Flow` container with
/// `WrapContents` off: one line, clipped — its items running past its edge is
/// that layout's intended behaviour, not something the window must prevent.
fn clips_one_line(controls: &[Control], form_props: &BTreeMap<String, PropValue>, parent: Option<&str>) -> bool {
    use crate::layout::props::{self, FormBag, PropSource};
    let flow_nowrap = |src: &dyn PropSource| props::layout_mode(src) == crate::layout::LayoutMode::Flow && !src.flag("WrapContents");
    match parent {
        None => flow_nowrap(&FormBag(form_props)),
        Some(p) => controls.iter().find(|c| c.id == p).is_some_and(|c| flow_nowrap(c)),
    }
}

/// The visible controls that are fully inside their parent's client area (the
/// form's, for a top-level control) at the designed size — the ones the
/// window must never push past that edge. A control that already overflows in
/// the design is the developer's choice and is not constrained, and neither is
/// an item of a one-line, clipped Flow ([`clips_one_line`]).
fn contained(
    controls: &[Control],
    form_props: &BTreeMap<String, PropValue>,
    o: &crate::layout::LayoutOutput,
) -> Vec<(String, Option<String>)> {
    let ids: std::collections::HashSet<&str> = controls.iter().map(|c| c.id.as_str()).collect();
    controls
        .iter()
        .filter(|c| c.visible && !c.control_type.is_non_visual() && !o.hidden.contains(&c.id))
        .filter_map(|c| {
            // A parent that does not exist puts the control at form level.
            let parent = c.parent.clone().filter(|p| ids.contains(p.as_str()));
            if clips_one_line(controls, form_props, parent.as_deref()) {
                return None;
            }
            let r = o.rects.get(&c.id)?;
            let client = parent_client(o, parent.as_deref())?;
            inside(r, &client).then(|| (c.id.clone(), parent))
        })
        .collect()
}

/// Whether two rectangles touch or overlap: no gap between them on either
/// axis.
fn touching(a: &LRect, b: &LRect) -> bool {
    let gap_x = (b.x - a.right()).max(a.x - b.right());
    let gap_y = (b.y - a.bottom()).max(a.y - b.bottom());
    gap_x <= COLLIDE_TOUCH_EPS && gap_y <= COLLIDE_TOUCH_EPS
}

/// The window limits for these controls; see the module docs.
pub fn window_size_limits(
    controls: &[Control],
    designed: (f32, f32),
    form_props: &BTreeMap<String, PropValue>,
    breakpoints: &[Breakpoint],
) -> SizeLimits {
    let floor = super::window_min_size(controls, designed, form_props, breakpoints);
    let at = |w: f32, h: f32| solve(&LayoutInput::new(controls, designed, (w, h), form_props, breakpoints));
    let base = at(designed.0, designed.1);
    let pairs = apart_pairs(controls, &base.rects, &base.hidden);
    let held = contained(controls, form_props, &base);
    if pairs.is_empty() && held.is_empty() {
        return SizeLimits { min: floor, max: (f32::INFINITY, f32::INFINITY) };
    }
    let collides = |w: f32, h: f32| {
        let o = at(w, h);
        pairs.iter().any(|(a, b)| match (o.rects.get(a), o.rects.get(b)) {
            _ if o.hidden.contains(a) || o.hidden.contains(b) => false,
            (Some(ra), Some(rb)) => touching(ra, rb),
            _ => false,
        }) || held.iter().any(|(c, p)| match (o.rects.get(c), parent_client(&o, p.as_deref())) {
            _ if o.hidden.contains(c) => false,
            (Some(r), Some(client)) => !inside(r, &client),
            _ => false,
        })
    };
    let min_w = shrink(designed.0, floor.0, |w| collides(w, designed.1));
    let min_h = shrink(designed.1, floor.1, |h| collides(designed.0, h));
    let max_w = grow(designed.0, |w| collides(w, designed.1));
    let max_h = grow(designed.1, |h| collides(designed.0, h));
    SizeLimits { min: (min_w, min_h), max: (max_w, max_h) }
}

/// The properties the layout reads from a control; a change to any other
/// (a caption, a value) cannot move a control, so it does not invalidate a
/// [`LimitsCache`].
const LAYOUT_KEYS: &[&str] = &[
    "Anchor", "Dock", "MinWidth", "MinHeight", "MaxWidth", "MaxHeight", "LayoutMode", "FlexDirection",
    "FlexWrap", "FlexGrow", "FlexShrink", "FlexBasis", "Order", "AlignSelf", "AlignItems", "AlignContent",
    "JustifyContent", "JustifyItems", "JustifySelf", "Gap", "RowGap", "ColumnGap", "GridColumns", "GridRows",
    "GridColumn", "GridRow", "ColumnSpan", "RowSpan", "FlowDirection", "FlowBreak", "WrapContents", "Padding",
    "PaddingLeft", "PaddingTop", "PaddingRight", "PaddingBottom", "AutoSize", "TabPosition",
];

/// A hash of everything [`window_size_limits`] depends on.
pub fn layout_fingerprint(
    controls: &[Control],
    designed: (f32, f32),
    form_props: &BTreeMap<String, PropValue>,
    breakpoints: &[Breakpoint],
) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    designed.0.to_bits().hash(&mut h);
    designed.1.to_bits().hash(&mut h);
    format!("{form_props:?}{breakpoints:?}").hash(&mut h);
    for c in controls {
        c.id.hash(&mut h);
        (c.rect.x, c.rect.y, c.rect.w, c.rect.h, c.z_order, c.visible).hash(&mut h);
        c.parent.hash(&mut h);
        format!("{:?}", c.control_type).hash(&mut h);
        for k in LAYOUT_KEYS {
            if let Some(v) = c.properties.get(*k) {
                k.hash(&mut h);
                format!("{v:?}").hash(&mut h);
            }
        }
    }
    h.finish()
}

/// [`window_size_limits`], recomputed only when [`layout_fingerprint`]
/// changes — the search costs milliseconds, and a window asks every frame.
#[derive(Debug, Default)]
pub struct LimitsCache(std::sync::Mutex<Option<(u64, SizeLimits)>>);

impl Clone for LimitsCache {
    fn clone(&self) -> Self {
        Self(std::sync::Mutex::new(*self.0.lock().unwrap_or_else(|e| e.into_inner())))
    }
}

impl LimitsCache {
    pub fn get(
        &self,
        controls: &[Control],
        designed: (f32, f32),
        form_props: &BTreeMap<String, PropValue>,
        breakpoints: &[Breakpoint],
    ) -> SizeLimits {
        let key = layout_fingerprint(controls, designed, form_props, breakpoints);
        let mut slot = self.0.lock().unwrap_or_else(|e| e.into_inner());
        match *slot {
            Some((k, l)) if k == key => l,
            _ => {
                let l = window_size_limits(controls, designed, form_props, breakpoints);
                *slot = Some((key, l));
                l
            }
        }
    }
}

/// An OS window cannot be given an infinite maximum: an unbounded axis is
/// reported as this.
pub const UNBOUNDED: f32 = crate::layout::defaults::WINDOW_UNBOUNDED;

impl SizeLimits {
    /// The maximum as a window can take it: [`UNBOUNDED`] for an axis with
    /// no limit.
    pub fn window_max(&self) -> (f32, f32) {
        let f = |v: f32| if v.is_finite() { v } else { UNBOUNDED };
        (f(self.max.0), f(self.max.1))
    }
}

/// [`window_size_limits`] for a form the engine lays out — responsive, or with
/// an obsolete scaling style (spec 081) — or `None` for one it does not (its
/// window keeps no limits, as before).
pub fn size_limits_of(form: &crate::model::Form) -> Option<SizeLimits> {
    form.lays_out().then(|| {
        window_size_limits(
            &form.controls,
            (form.width as f32, form.height as f32),
            &form.layout,
            &form.breakpoints,
        )
    })
}

/// The smallest size between `floor` and `from` reached by shrinking from
/// `from` without a collision.
fn shrink(from: f32, floor: f32, collides: impl Fn(f32) -> bool) -> f32 {
    if from <= floor {
        return floor;
    }
    let mut good = from;
    let mut x = from - SHRINK_STEP;
    loop {
        let probe = x.max(floor);
        if collides(probe) {
            return refine(probe, good, &collides).ceil().min(from);
        }
        if probe <= floor {
            return floor;
        }
        good = probe;
        x -= SHRINK_STEP;
    }
}

/// The largest size reached by growing from `from` without a collision, or
/// infinity when none occurs up to [`GROW_CAP`].
fn grow(from: f32, collides: impl Fn(f32) -> bool) -> f32 {
    let mut good = from;
    let mut d = COLLIDE_GROW_FIRST_STEP;
    while from + d <= GROW_CAP.max(from) {
        let probe = from + d;
        if collides(probe) {
            return refine(probe, good, &collides).floor().max(from);
        }
        good = probe;
        d *= 2.0;
    }
    if from < GROW_CAP && collides(GROW_CAP) {
        return refine(GROW_CAP, good, &collides).floor().max(from);
    }
    f32::INFINITY
}

/// Narrow `bad` (collides) and `good` (does not) to the last good size.
fn refine(mut bad: f32, mut good: f32, collides: &impl Fn(f32) -> bool) -> f32 {
    for _ in 0..REFINE {
        let mid = (bad + good) / 2.0;
        if collides(mid) {
            bad = mid;
        } else {
            good = mid;
        }
    }
    good
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::layout::test_support::*;
    use crate::model::ControlType;

    fn anchored(id: &str, r: (i32, i32, i32, i32), anchor: &str) -> Control {
        with(ctrl(id, ControlType::Button, r, None), "Anchor", PropValue::String(anchor.into()))
    }

    fn limits(controls: &[Control], designed: (f32, f32)) -> SizeLimits {
        window_size_limits(controls, designed, &BTreeMap::new(), &[])
    }

    /// A `Left` field at x 20..220 and a `Right` button at 300..400 in a 420
    /// wide form: the button keeps 20 from the right edge, so it meets the
    /// field when the form is 220 + 100 + 20 = 340 wide. Vertically they share
    /// a row and never meet. Growing only pulls them apart.
    #[test]
    fn shrinking_stops_where_a_right_anchored_control_would_meet_its_neighbour() {
        let field = anchored("F", (20, 20, 200, 30), "Top,Left");
        let button = anchored("B", (300, 20, 100, 30), "Top,Right");
        let l = limits(&[field, button], (420.0, 200.0));
        assert!((l.min.0 - 341.0).abs() <= 1.0, "min width {:?}", l.min);
        assert!(l.max.0.is_infinite() && l.max.1.is_infinite(), "{l:?}");
        println!("Left field + Right button, 420 wide: window floor {:.0} wide (they would touch at 340)", l.min.0);
    }

    /// A `Left,Right` field (20..220 of 420) beside a `Left` button at
    /// 300..400: growing the form stretches the field into the button when
    /// 220 + dw reaches 300, i.e. at 500 wide.
    #[test]
    fn growing_stops_where_a_stretched_control_would_reach_a_fixed_one() {
        let field = anchored("F", (20, 20, 200, 30), "Top,Left,Right");
        let button = anchored("B", (300, 20, 100, 30), "Top,Left");
        let l = limits(&[field, button], (420.0, 200.0));
        assert!((l.max.0 - 499.0).abs() <= 1.0, "max width {:?}", l.max);
        assert!(l.max.1.is_infinite());
        println!("stretched field beside a fixed button, 420 wide: window ceiling {:.0} wide", l.max.0);
    }

    /// The same, vertically: a `Bottom` button below a `Top` list.
    #[test]
    fn shrinking_height_stops_before_a_bottom_button_meets_the_list() {
        let list = anchored("L", (20, 20, 200, 200), "Top,Left");
        let button = anchored("B", (20, 260, 100, 30), "Bottom,Left");
        let l = limits(&[list, button], (300.0, 320.0));
        // The button keeps 30 from the bottom: it meets the list at 220 + 30 + 30 = 280.
        assert!((l.min.1 - 281.0).abs() <= 1.0, "min height {:?}", l.min);
    }

    /// The cache answers again without searching, and searches again when a
    /// layout property changes — but not for a caption.
    #[test]
    fn the_cache_recomputes_only_when_the_layout_changes() {
        let field = anchored("F", (20, 20, 200, 30), "Top,Left");
        let mut button = anchored("B", (300, 20, 100, 30), "Top,Right");
        let cache = LimitsCache::default();
        let none = BTreeMap::new();
        let a = cache.get(&[field.clone(), button.clone()], (420.0, 200.0), &none, &[]);
        let key = layout_fingerprint(&[field.clone(), button.clone()], (420.0, 200.0), &none, &[]);
        button.properties.insert("Caption".into(), PropValue::String("OK".into()));
        assert_eq!(key, layout_fingerprint(&[field.clone(), button.clone()], (420.0, 200.0), &none, &[]));
        button.properties.insert("Anchor".into(), PropValue::String("Top,Left".into()));
        let b = cache.get(&[field, button], (420.0, 200.0), &none, &[]);
        // Top,Left, the button no longer meets the field; it now limits the
        // width only by its own right edge at 400 — a different answer, so
        // the cache searched again.
        assert!((a.min.0 - 341.0).abs() <= 1.0 && (b.min.0 - 400.0).abs() <= 1.0, "{a:?} → {b:?}");
    }

    /// Two controls on different pages of a TabControl are never on screen
    /// together: a `Left` field on page 0 and a `Right` button on page 1 limit
    /// the window only as each would alone (by its own page's edges); the
    /// same pair on one page stops the window where they meet.
    #[test]
    fn controls_on_different_tab_pages_never_collide() {
        let s = |v: &str| PropValue::String(v.into());
        let tabs = with(ctrl("K", ControlType::TabControl, (0, 0, 420, 200), None), "Anchor", s("Top,Bottom,Left,Right"));
        let tabs = with(tabs, "Tabs", s("One\nTwo"));
        let y = tabs.content_rect().y + 10;
        let mut f = with(ctrl("F", ControlType::TextBox, (20, y, 200, 30), Some("K")), "Anchor", s("Top,Left"));
        let mut b = with(ctrl("B", ControlType::Button, (300, y, 100, 30), Some("K")), "Anchor", s("Top,Right"));
        f.tab = Some(0);
        b.tab = Some(1);
        let apart = limits(&[tabs.clone(), f.clone(), b.clone()], (420.0, 200.0));
        let alone_f = limits(&[tabs.clone(), f.clone()], (420.0, 200.0)).min;
        let alone_b = limits(&[tabs.clone(), b.clone()], (420.0, 200.0)).min;
        let alone = (alone_f.0.max(alone_b.0), alone_f.1.max(alone_b.1));
        assert_eq!(apart.min, alone, "different pages: {apart:?}");
        b.tab = Some(0);
        let together = limits(&[tabs, f, b], (420.0, 200.0));
        assert!(together.min.0 > 300.0, "same page: {together:?}");
        println!("tab pages: floor {:?} apart, {:?} on one page", apart.min, together.min);
    }

    /// A control inside its parent at the designed size is never pushed past
    /// the parent's edge: a lone `Top,Left` button at x 760..790 of an 800
    /// wide form stops the window at 790 wide (it used to shrink to the
    /// floor and leave the button outside); a `Top,Left` label near the
    /// bottom of a Panel stretched on all four edges stops the height where
    /// the Panel's client would end above it.
    #[test]
    fn shrinking_stops_before_a_control_would_leave_its_parent() {
        let lone = anchored("B", (760, 20, 30, 30), "Top,Left");
        let l = limits(&[lone], (800.0, 600.0));
        assert!((l.min.0 - 790.0).abs() <= 1.0, "min width {:?}", l.min);
        assert!(l.max.0.is_infinite() && l.max.1.is_infinite(), "{l:?}");

        let panel = with(ctrl("P", ControlType::Panel, (20, 20, 360, 260), None), "Anchor", PropValue::String("Top,Bottom,Left,Right".into()));
        let client = panel.content_rect();
        let lbl = with(ctrl("L", ControlType::Label, (40, 230, 80, 20), Some("P")), "Anchor", PropValue::String("Top,Left".into()));
        // The Panel's client ends (client bottom − 250) above the label's
        // bottom at 250, so the form may lose that much height and no more.
        let room = (client.y + client.h - 250) as f32;
        let l = limits(&[panel, lbl], (400.0, 300.0));
        assert!((l.min.1 - (300.0 - room)).abs() <= 1.0, "min height {:?}, room {room}", l.min);
        println!("a lone Top,Left button stops the width at {:.0}; a label in a stretched panel stops the height at {:.0}", 790.0, l.min.1);
    }

    /// A `Flow` with `WrapContents` off is one line, clipped: its row running
    /// past the container's edge does not hold the window. The Flow sits in a
    /// `MinMax(100px, 1fr)` grid column, so the declared minimum lets it
    /// narrow well below its row. The same items in a wrapping Flow wrap onto
    /// a second line its fixed height cannot show, so they do hold the window.
    #[test]
    fn a_one_line_flows_overflow_does_not_hold_the_window() {
        let s = |v: &str| PropValue::String(v.into());
        let grid = with(ctrl("G", ControlType::Panel, (0, 0, 800, 300), None), "Dock", s("Fill"));
        let grid = with(with(grid, "LayoutMode", s("Grid")), "GridColumns", s("MinMax(100px, 1fr)"));
        let gc = grid.content_rect();
        let flow = |wrap: bool| {
            let f = ctrl("FL", ControlType::Panel, (gc.x, gc.y, gc.w, 60), Some("G"));
            with(with(with(f, "LayoutMode", s("Flow")), "WrapContents", PropValue::Bool(wrap)), "AlignSelf", s("Start"))
        };
        let fc = flow(false).content_rect();
        let form = |wrap: bool| {
            let mut v = vec![grid.clone(), flow(wrap)];
            v.extend((0..7).map(|k| ctrl(&format!("I{k}"), ControlType::Button, (fc.x + k * 90, fc.y, 90, 30), Some("FL"))));
            v
        };
        let at = |v: &[Control]| (limits(v, (800.0, 300.0)).min, crate::layout::window_min_size(v, (800.0, 300.0), &BTreeMap::new(), &[]));
        let (one_line, one_floor) = at(&form(false));
        assert_eq!(one_line.0, one_floor.0, "WrapContents off: clipped, the width is not held");
        assert!(one_line.0 < 300.0, "the declared minimum is the grid's, well below the row: {one_line:?}");
        let (wrapping, wrap_floor) = at(&form(true));
        assert!(wrapping.0 > wrap_floor.0 + 1.0 && wrapping.0 > 600.0, "a wrapping Flow holds its items: {wrapping:?} over {wrap_floor:?}");
        println!("one-line Flow: min width {:.0}; wrapping Flow: min width {:.0}", one_line.0, wrapping.0);
    }

    /// What already overflows its parent in the design, and what is hidden,
    /// constrains nothing.
    #[test]
    fn a_control_already_outside_or_hidden_does_not_limit_the_window() {
        let outside = anchored("O", (790, 20, 30, 30), "Top,Left"); // past 800
        let mut hidden = anchored("H", (760, 60, 30, 30), "Top,Left");
        hidden.visible = false;
        let l = limits(&[outside, hidden], (800.0, 600.0));
        let floor = crate::layout::window_min_size(&[], (800.0, 600.0), &BTreeMap::new(), &[]);
        assert_eq!(l.min, floor);
    }

    /// Spec 081 AC8 — the limits follow an obsolete scaling style. Two buttons
    /// 20 px apart in a row of a 400 × 300 form, A at 20..100 and B at
    /// 120..200:
    /// * style 1 (resize only): positions stay, widths grow by the width
    ///   ratio, so A's right edge 20 + 80·rx meets B at 120 when rx = 1.25 —
    ///   a 500-wide window. That is the ceiling;
    /// * style 3 (resize and reposition): the gap grows with the window too,
    ///   so they never meet — no ceiling;
    /// and a form that is NOT responsive but carries a style gets them.
    #[test]
    fn an_obsolete_scaling_style_takes_the_collision_limits_081() {
        let a = ctrl("A", ControlType::Button, (20, 20, 80, 30), None);
        let b = ctrl("B", ControlType::Button, (120, 20, 80, 30), None);
        let style = |v: i64| BTreeMap::from([("ObsoleteScalingStyle".to_owned(), PropValue::Int(v))]);
        let resize = window_size_limits(&[a.clone(), b.clone()], (400.0, 300.0), &style(1), &[]);
        assert!((resize.max.0 - 499.0).abs() <= 1.0, "style 1 ceiling {:?}", resize.max);
        let both = window_size_limits(&[a.clone(), b.clone()], (400.0, 300.0), &style(3), &[]);
        assert!(both.max.0.is_infinite() && both.max.1.is_infinite(), "style 3: {both:?}");

        let mut form = crate::model::Form::new("F", "F", 400, 300);
        form.controls = vec![a, b];
        assert!(size_limits_of(&form).is_none(), "neither responsive nor scaling: no limits");
        form.layout.insert("ObsoleteScalingStyle".into(), PropValue::Int(1));
        assert!(!form.responsive);
        let l = size_limits_of(&form).expect("a scaling form is laid out, so it has limits");
        assert!((l.max.0 - 499.0).abs() <= 1.0, "{l:?}");
        println!(
            "081 AC8: buttons 20 px apart, 400 wide — style 1 ceiling {:.0} (they meet at 500), style 3 ceiling ∞; non-responsive style-1 form limited too",
            resize.max.0
        );
    }

    /// Controls that touch or overlap in the design (a label on its card, a
    /// hidden control) constrain nothing as a pair: the window stops only
    /// where `B`, keeping 120 from the right, would cross the form's left
    /// edge (320 wide) — the overlap with `A` and the hidden `C` add nothing.
    #[test]
    fn what_touches_in_the_design_and_what_is_hidden_constrain_nothing() {
        let a = anchored("A", (20, 20, 200, 30), "Top,Left");
        let b = anchored("B", (100, 20, 200, 30), "Top,Right"); // overlaps A
        let mut c = anchored("C", (350, 20, 40, 30), "Top,Right");
        c.visible = false;
        let l = limits(&[a, b, c], (420.0, 200.0));
        let floor = crate::layout::window_min_size(&[], (420.0, 200.0), &BTreeMap::new(), &[]);
        assert!((l.min.0 - 320.0).abs() <= 1.0 && l.min.1 == floor.1, "{l:?}");
        assert!(l.max.0.is_infinite());
    }
}
