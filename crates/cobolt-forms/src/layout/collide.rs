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
    if pairs.is_empty() {
        return SizeLimits { min: floor, max: (f32::INFINITY, f32::INFINITY) };
    }
    let collides = |w: f32, h: f32| {
        let o = at(w, h);
        pairs.iter().any(|(a, b)| match (o.rects.get(a), o.rects.get(b)) {
            _ if o.hidden.contains(a) || o.hidden.contains(b) => false,
            (Some(ra), Some(rb)) => touching(ra, rb),
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

/// [`window_size_limits`] for a form, or `None` for a form that is not
/// responsive (its window keeps no limits, as before).
pub fn size_limits_of(form: &crate::model::Form) -> Option<SizeLimits> {
    form.responsive.then(|| {
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
        assert!(a.min.0 > b.min.0, "a Left button no longer limits the width: {a:?} → {b:?}");
    }

    /// Two controls on different pages of a TabControl are never on screen
    /// together: a `Left` field on page 0 and a `Right` button on page 1 limit
    /// nothing; the same pair on one page stops the window where they meet.
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
        let floor = crate::layout::window_min_size(&[], (420.0, 200.0), &BTreeMap::new(), &[]);
        assert_eq!(apart.min, floor, "different pages: {apart:?}");
        b.tab = Some(0);
        let together = limits(&[tabs, f, b], (420.0, 200.0));
        assert!(together.min.0 > 300.0, "same page: {together:?}");
        println!("tab pages: floor {:?} apart, {:?} on one page", apart.min, together.min);
    }

    /// Controls that touch or overlap in the design (a label on its card, a
    /// hidden control) constrain nothing, and a form with nothing apart keeps
    /// only its declared floor.
    #[test]
    fn what_touches_in_the_design_and_what_is_hidden_constrain_nothing() {
        let a = anchored("A", (20, 20, 200, 30), "Top,Left");
        let b = anchored("B", (100, 20, 200, 30), "Top,Right"); // overlaps A
        let mut c = anchored("C", (350, 20, 40, 30), "Top,Right");
        c.visible = false;
        let l = limits(&[a, b, c], (420.0, 200.0));
        let floor = crate::layout::window_min_size(&[], (420.0, 200.0), &BTreeMap::new(), &[]);
        assert_eq!(l.min, floor);
        assert!(l.max.0.is_infinite());
    }
}
