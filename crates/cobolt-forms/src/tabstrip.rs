// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! A TabControl's tab strip: where each tab sits, and how a strip with more tabs than it has room for scrolls.
//!
//! # The rule that was here before
//!
//! Tabs sit edge to edge along their edge of the control, each as wide as its label (on a side strip: as tall as one
//! tab, all as wide as the widest). The first tab that would overrun the control was left out, and every tab after it:
//! neither painted nor clickable. With 32 sheet names in a 1152 px strip, 9 were there and 23 could not be reached.
//!
//! # The rule now
//!
//! * **Everything fits:** nothing changes. The same rectangles, and no chevrons.
//! * **It does not fit:** a chevron button takes each end of the strip, and the tabs between them are drawn from
//!   `first` on, whole, until the next one would overrun. `first` is how far the strip is scrolled, counted in tabs. It
//!   runs from 0 to `last_first`, the first tab of the last run of tabs that all fit, so the last tab can always be
//!   brought into view. A tab is never drawn in part: the viewport holds whole tabs, as the strip always did.
//! * **A selection is brought into view, but only WHEN IT CHANGES** (a click, an arrow key, Home, End, Ctrl+Tab or a
//!   COBOL write of `SelectedTab`). Whoever scrolls away from the selected tab is not pulled back to it.
//!
//! # Why this is its own module
//!
//! The strip is laid out for five readers that must agree to the pixel: the painter that draws the tabs, the engine that
//! hit-tests them, the focus ring around the selected one, the designer canvas that selects a page by clicking a tab, and
//! the tests. A strip laid out five times is a strip that eventually draws in one place and answers clicks in another
//! (the same reason `splitter::geometry` exists), so it is laid out ONCE, here, from the control and a scroll position.
//!
//! This module is geometry and arithmetic. It reads no input and keeps no state: `render` owns `first` (view state, kept
//! in egui's memory like a Panel's scroll offset: it is not part of the form and is never saved) and calls in here.

use egui::{pos2, vec2, Pos2, Rect};

use crate::model::Control;

/// One tab as drawn: which tab it is (from 0, in `Tabs` order) and where it sits.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TabSlot {
    pub index: usize,
    pub rect: Rect,
}

/// What a point on the strip is over.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TabHit {
    /// Tab `index` (from 0).
    Tab(usize),
    /// The chevron toward the first tab.
    Prev,
    /// The chevron toward the last tab.
    Next,
}

/// A TabControl's strip, laid out for one scroll position.
#[derive(Debug, Clone, PartialEq)]
pub struct TabStrip {
    /// Every tab drawn, in order. Whole tabs only.
    pub slots: Vec<TabSlot>,
    /// The chevron toward the first tab, and the one toward the last: present together, and only when the tabs do not
    /// all fit.
    pub prev: Option<Rect>,
    pub next: Option<Rect>,
    /// The band the strip occupies along its edge of the control, chevrons included: where the wheel scrolls it.
    pub band: Rect,
    /// The first tab drawn, once held between 0 and `last_first`.
    pub first: usize,
    /// How far the strip scrolls: 0 when everything fits.
    pub last_first: usize,
    /// How many tabs there are, drawn or not.
    pub count: usize,
    /// A side strip (Left / Right) runs down the control rather than across it.
    pub vertical: bool,
}

impl TabStrip {
    /// The tabs do not all fit, so the strip has chevrons and scrolls.
    pub fn overflows(&self) -> bool {
        self.prev.is_some()
    }

    /// The chevron toward the first tab does something.
    pub fn can_scroll_back(&self) -> bool {
        self.overflows() && self.first > 0
    }

    /// The chevron toward the last tab does something.
    pub fn can_scroll_forward(&self) -> bool {
        self.overflows() && self.first < self.last_first
    }

    /// Where tab `index` sits, when it is drawn.
    pub fn rect_of(&self, index: usize) -> Option<Rect> {
        self.slots.iter().find(|s| s.index == index).map(|s| s.rect)
    }

    /// The tab strip's tabs, joined: the keyboard stop of the control. `None` when no tab is drawn.
    pub fn tabs_rect(&self) -> Option<Rect> {
        self.slots.iter().map(|s| s.rect).reduce(|a, b| a.union(b))
    }
}

/// The tab titles, from `Tabs` (one per line, in order).
pub fn tab_titles(ctrl: &Control) -> Vec<String> {
    ctrl.get_prop("Tabs")
        .map(|v| v.as_str().lines().map(|s| s.to_string()).collect())
        .unwrap_or_default()
}

/// How many tabs the control has, drawn or not.
pub fn tab_count(ctrl: &Control) -> usize {
    ctrl.get_prop("Tabs").map_or(0, |v| v.as_str().lines().count())
}

/// The strip position (from 0) of the page `SelectedTab` names (from 1); 0 or less reads as the first page. It may be past
/// the last tab: a program that writes one past the last name marks no tab at all.
pub fn selected_position(ctrl: &Control) -> usize {
    ctrl.get_prop("SelectedTab").map(|v| v.as_i64()).unwrap_or(1).max(1) as usize - 1
}

/// The strip's measurements along its own axis, from the control alone (no scroll position, no origin).
struct Layout {
    /// Each tab's extent along the strip.
    lens: Vec<f32>,
    /// A tab's extent across the strip.
    cross: f32,
    /// The strip runs down the control (Left / Right).
    vertical: bool,
    /// The strip's length: the control's width, or its height for a side strip.
    avail: f32,
    /// A chevron's extent along the strip: 0 when the tabs fit and there are none.
    chevron: f32,
    /// The room for tabs: everything between the chevrons, or all of `avail` when there are none.
    view: f32,
    /// The furthest `first` goes: 0 without chevrons.
    last_first: usize,
    /// The band the strip occupies, for `origin`.
    band: Rect,
}

impl Layout {
    fn of(origin: Pos2, ctrl: &Control) -> Layout {
        let position = ctrl.tab_position();
        let vertical = matches!(position.as_str(), "left" | "right");
        let size = vec2(ctrl.rect.w.max(0) as f32, ctrl.rect.h.max(0) as f32);
        let tab_h = ctrl.tab_strip_height().max(0) as f32;
        let strip = ctrl.tab_strip_extent().max(0) as f32;
        let band = match position.as_str() {
            "bottom" => Rect::from_min_size(pos2(origin.x, origin.y + size.y - tab_h), vec2(size.x, tab_h)),
            "left" => Rect::from_min_size(origin, vec2(strip, size.y)),
            "right" => Rect::from_min_size(pos2(origin.x + size.x - strip, origin.y), vec2(strip, size.y)),
            _ => Rect::from_min_size(origin, vec2(size.x, tab_h)),
        };
        let lens: Vec<f32> = tab_titles(ctrl)
            .iter()
            .map(|t| if vertical { tab_h } else { ctrl.tab_width(t) as f32 })
            .collect();
        let cross = if vertical { strip } else { tab_h };
        let avail = if vertical { size.y } else { size.x };
        let total: f32 = lens.iter().sum();
        let smallest = lens.iter().copied().fold(f32::INFINITY, f32::min);
        // A square button at each end, as large as a tab is tall. A control too short to hold the two and one tab keeps the
        // old rule (whole tabs from the start, the rest left out): there is no room to scroll in.
        let scrolls = !lens.is_empty() && total > avail && avail >= 2.0 * tab_h + smallest;
        let chevron = if scrolls { tab_h } else { 0.0 };
        let view = avail - 2.0 * chevron;
        let last_first = if scrolls {
            // The first tab of the last run that all fits: walk back from the end until the next one would overrun.
            let mut used = 0.0;
            let mut first = lens.len();
            for (i, len) in lens.iter().enumerate().rev() {
                if used + len > view {
                    break;
                }
                used += len;
                first = i;
            }
            first.min(lens.len() - 1)
        } else {
            0
        };
        Layout { lens, cross, vertical, avail, chevron, view, last_first, band }
    }

    /// A rectangle `len` long starting at `cursor` (an absolute coordinate along the strip).
    fn at(&self, cursor: f32, len: f32) -> Rect {
        if self.vertical {
            Rect::from_min_size(pos2(self.band.min.x, cursor), vec2(self.cross, len))
        } else {
            Rect::from_min_size(pos2(cursor, self.band.min.y), vec2(len, self.cross))
        }
    }

    /// Where the strip starts along its axis.
    fn axis_min(&self) -> f32 {
        if self.vertical {
            self.band.min.y
        } else {
            self.band.min.x
        }
    }
}

/// The strip of `ctrl` at `origin` (the control's top-left corner, in whatever space the caller paints in), scrolled to
/// `first`. `first` is held between 0 and the furthest the strip scrolls, so any value is safe.
pub fn tab_strip(origin: Pos2, ctrl: &Control, first: usize) -> TabStrip {
    let l = Layout::of(origin, ctrl);
    let scrolls = l.chevron > 0.0;
    let first = if scrolls { first.min(l.last_first) } else { 0 };
    // Edge to edge, accumulated in absolute coordinates so that one tab's far edge IS the next one's near edge, to the bit.
    let mut cursor = l.axis_min() + l.chevron;
    let end = cursor + l.view;
    let mut slots = Vec::new();
    for (index, &len) in l.lens.iter().enumerate().skip(first) {
        if cursor + len > end {
            break;
        }
        slots.push(TabSlot { index, rect: l.at(cursor, len) });
        cursor += len;
    }
    let (prev, next) = if scrolls {
        (Some(l.at(l.axis_min(), l.chevron)), Some(l.at(l.axis_min() + l.avail - l.chevron, l.chevron)))
    } else {
        (None, None)
    };
    TabStrip { slots, prev, next, band: l.band, first, last_first: l.last_first, count: l.lens.len(), vertical: l.vertical }
}

/// How far the strip of `ctrl` scrolls: 0 when every tab fits.
pub fn scroll_range(ctrl: &Control) -> usize {
    Layout::of(Pos2::ZERO, ctrl).last_first
}

/// Where the strip should start so that tab `selected` is among the ones drawn, moving it as little as possible from
/// `first`: not at all when the tab is already on show, back to it when it is to the left, and just far enough for it to be
/// the last one drawn when it is to the right. A `selected` past the last tab (nothing is selected) moves nothing.
pub fn scroll_to(ctrl: &Control, first: usize, selected: usize) -> usize {
    let l = Layout::of(Pos2::ZERO, ctrl);
    if l.chevron == 0.0 {
        return 0;
    }
    let first = first.min(l.last_first);
    if selected >= l.lens.len() {
        return first;
    }
    if selected < first {
        return selected.min(l.last_first);
    }
    // Is it on show? Tabs `first..=selected` all have to fit.
    let span: f32 = l.lens[first..=selected].iter().sum();
    if span <= l.view {
        return first;
    }
    // To the right: start as far back as keeps `selected` the last tab drawn.
    let mut start = selected;
    let mut used = l.lens[selected];
    while start > first + 1 && used + l.lens[start - 1] <= l.view {
        start -= 1;
        used += l.lens[start];
    }
    start.min(l.last_first)
}

/// Where the strip starts when nothing has ever scrolled it: just far enough for the selected tab to be on show. This is
/// what a surface with no scroll state (the designer canvas, a still picture) draws.
pub fn auto_first(ctrl: &Control) -> usize {
    scroll_to(ctrl, 0, selected_position(ctrl))
}

/// What a point on `strip` is over, if anything.
pub fn hit(strip: &TabStrip, p: Pos2) -> Option<TabHit> {
    if strip.prev.is_some_and(|r| r.contains(p)) {
        return Some(TabHit::Prev);
    }
    if strip.next.is_some_and(|r| r.contains(p)) {
        return Some(TabHit::Next);
    }
    strip.slots.iter().find(|s| s.rect.contains(p)).map(|s| TabHit::Tab(s.index))
}

/// The tab a click at `p` selects on a surface with no scroll of its own (the designer canvas): the tab under the pointer,
/// by its own index whatever its place in the strip; for a chevron, the neighbour of the selected tab on that side (the
/// strip is laid out to keep the selection on show, `auto_first`, so it follows). `None` off the strip. `strip` is the strip
/// of `ctrl` that was drawn.
pub fn click_target(ctrl: &Control, strip: &TabStrip, p: Pos2) -> Option<usize> {
    match hit(strip, p)? {
        TabHit::Tab(i) => Some(i),
        TabHit::Prev => Some(selected_position(ctrl).min(strip.count.saturating_sub(1)).saturating_sub(1)),
        TabHit::Next => Some((selected_position(ctrl) + 1).min(strip.count.saturating_sub(1))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ControlType, PropValue};

    /// A TabControl `w` x `h` with these tabs on `position`, tab 1 selected.
    fn tabs(names: &[String], position: &str, w: i32, h: i32) -> Control {
        let mut c = Control::new("TAB-1", ControlType::TabControl, 0, 0);
        c.rect.w = w;
        c.rect.h = h;
        c.set_prop("Tabs", PropValue::String(names.join("\n")));
        c.set_prop("TabPosition", PropValue::String(position.into()));
        c
    }

    fn sheets(n: usize) -> Vec<String> {
        (1..=n).map(|k| format!("Sheet {k:02}")).collect()
    }

    /// The strip exactly as it was laid out before scrolling existed (`paint::tabcontrol_tab_rects`, 2.0.88): tabs edge
    /// to edge from the start of the strip, the first one that would overrun the control left out with all that follow.
    /// Kept here, verbatim in effect, as the reference a strip that FITS must still equal.
    fn the_old_rule(origin: Pos2, ctrl: &Control) -> Vec<Rect> {
        let tabs = tab_titles(ctrl);
        let rect = Rect::from_min_size(origin, vec2(ctrl.rect.w.max(0) as f32, ctrl.rect.h.max(0) as f32));
        let tab_h = ctrl.tab_strip_height().max(0) as f32;
        let strip = ctrl.tab_strip_extent().max(0) as f32;
        let mut out = Vec::new();
        match ctrl.tab_position().as_str() {
            pos @ ("left" | "right") => {
                let x = if pos == "left" { rect.min.x } else { rect.max.x - strip };
                let mut y = rect.min.y;
                for _ in tabs {
                    if y + tab_h > rect.max.y {
                        break;
                    }
                    out.push(Rect::from_min_size(pos2(x, y), vec2(strip, tab_h)));
                    y += tab_h;
                }
            }
            pos => {
                let y = if pos == "bottom" { rect.max.y - tab_h } else { rect.min.y };
                let mut x = rect.min.x;
                for tab in tabs {
                    let w = ctrl.tab_width(&tab) as f32;
                    if x + w > rect.max.x {
                        break;
                    }
                    out.push(Rect::from_min_size(pos2(x, y), vec2(w, tab_h)));
                    x += w;
                }
            }
        }
        out
    }

    /// Strips that fit are laid out as they always were: the same rectangles to the bit, no chevrons, at any origin
    /// (a fractional one included: the far edge of a tab must still be the near edge of the next).
    #[test]
    fn a_strip_that_fits_is_laid_out_as_before() {
        let mut cases = 0;
        let mut tab_total = 0;
        for position in ["top", "bottom", "left", "right"] {
            for count in [0, 1, 2, 3, 5, 8] {
                for (w, h) in [(1200, 600), (700, 500), (520, 400)] {
                    let c = tabs(&sheets(count), position, w, h);
                    let all = the_old_rule(Pos2::ZERO, &c).len();
                    // Only the cases where the old rule dropped nothing: a strip that fits.
                    if all != count {
                        continue;
                    }
                    for origin in [Pos2::ZERO, pos2(13.0, 7.0), pos2(0.37, 91.61)] {
                        let strip = tab_strip(origin, &c, 0);
                        let got: Vec<Rect> = strip.slots.iter().map(|s| s.rect).collect();
                        assert_eq!(got, the_old_rule(origin, &c), "{position} {count} tabs {w}x{h} at {origin:?}");
                        assert!(!strip.overflows(), "{position} {count} tabs {w}x{h}: nothing to scroll");
                        assert_eq!((strip.first, strip.last_first), (0, 0));
                        assert_eq!(strip.slots.iter().map(|s| s.index).collect::<Vec<_>>(), (0..count).collect::<Vec<_>>());
                        cases += 1;
                        tab_total += count;
                    }
                }
            }
        }
        assert!(cases > 100, "the sweep must cover many strips: {cases}");
        eprintln!("TabStrip fits — {cases} strips ({tab_total} tabs) laid out exactly as the old rule laid them, none with chevrons");
    }

    /// 32 sheet names in a 1152 px strip, the real case this feature exists for: the tabs that fit are whole, edge to
    /// edge, between two chevrons; the last tab can be reached; nothing is drawn under a chevron.
    #[test]
    fn a_strip_that_overflows_has_two_chevrons_and_whole_tabs_between_them() {
        let c = tabs(&sheets(32), "top", 1152, 560);
        let strip = tab_strip(Pos2::ZERO, &c, 0);
        let (prev, next) = (strip.prev.expect("a chevron toward the first tab"), strip.next.expect("one toward the last"));
        let tab_h = c.tab_strip_height() as f32;
        assert_eq!((prev.width(), prev.height()), (tab_h, tab_h), "a square, as large as a tab is tall");
        assert_eq!(prev.min.x, 0.0);
        assert_eq!(next.max.x, 1152.0, "the far chevron ends at the control's edge");
        assert_eq!((prev.min.y, next.min.y), (0.0, 0.0), "both sit on the strip");
        assert!(strip.slots.len() > 5 && strip.slots.len() < 32, "some tabs drawn, not all: {}", strip.slots.len());
        assert_eq!(strip.slots[0].index, 0);
        assert_eq!(strip.slots[0].rect.min.x, prev.max.x, "the first tab starts where the chevron ends");
        for w in strip.slots.windows(2) {
            assert_eq!(w[0].rect.max.x, w[1].rect.min.x, "edge to edge");
            assert_eq!(w[1].index, w[0].index + 1);
        }
        let last = strip.slots.last().unwrap().rect;
        assert!(last.max.x <= next.min.x, "no tab under the far chevron");
        assert!(strip.can_scroll_forward() && !strip.can_scroll_back(), "at the start: only forward");
        assert!(strip.last_first > 0 && strip.last_first < 32);
    }

    /// Every position from the start to `last_first` draws whole contiguous tabs, between the chevrons; together they
    /// reach every tab; at `last_first` the LAST tab is drawn and one step earlier it is not (that is what makes it the
    /// furthest the strip needs to go).
    #[test]
    fn scrolling_one_tab_at_a_time_reaches_every_tab() {
        let mut worst = 0;
        let mut checked = 0;
        for position in ["top", "bottom", "left", "right"] {
            for (count, w, h) in [(32, 1152, 560), (120, 1152, 560), (40, 640, 300), (9, 330, 200)] {
                let c = tabs(&sheets(count), position, w, h);
                let range = scroll_range(&c);
                assert!(range > 0, "{position} {count} tabs in {w}x{h}: must overflow");
                let mut reached = vec![false; count];
                for first in 0..=range {
                    let s = tab_strip(Pos2::ZERO, &c, first);
                    assert_eq!(s.first, first);
                    assert_eq!(s.slots[0].index, first, "{position}: the strip starts at the tab it was told to");
                    for pair in s.slots.windows(2) {
                        assert_eq!(pair[1].index, pair[0].index + 1);
                    }
                    let (p, n) = (s.prev.unwrap(), s.next.unwrap());
                    // Overlap, not touching: tabs and chevrons meet edge to edge, which egui's `intersects` counts.
                    let overlaps = |a: Rect, b: Rect| a.min.x < b.max.x && b.min.x < a.max.x && a.min.y < b.max.y && b.min.y < a.max.y;
                    for slot in &s.slots {
                        reached[slot.index] = true;
                        assert!(!overlaps(slot.rect, p) && !overlaps(slot.rect, n), "{position}: a tab under a chevron");
                        assert!(s.band.contains_rect(slot.rect), "{position}: a tab outside its band");
                    }
                    worst = worst.max(s.slots.len());
                    checked += 1;
                }
                assert!(reached.iter().all(|r| *r), "{position} {count} tabs: all of them reached by scrolling");
                assert!(tab_strip(Pos2::ZERO, &c, range).rect_of(count - 1).is_some(), "{position}: the last tab at the furthest");
                assert!(tab_strip(Pos2::ZERO, &c, range - 1).rect_of(count - 1).is_none(), "{position}: and not one step earlier");
                // Past the range is held at the range.
                assert_eq!(tab_strip(Pos2::ZERO, &c, range + 50).first, range);
            }
        }
        eprintln!("TabStrip scroll — {checked} scroll positions on 16 strips (4 edges x 4 sizes, up to 120 tabs): every tab reached, the last one at the furthest, widest viewport {worst} tabs");
    }

    /// A side strip scrolls down the control: tabs stacked as tall as one tab, chevrons above and below.
    #[test]
    fn a_side_strip_scrolls_vertically() {
        for position in ["left", "right"] {
            let c = tabs(&sheets(30), position, 400, 300);
            let s = tab_strip(pos2(10.0, 20.0), &c, 3);
            assert!(s.vertical);
            let (p, n) = (s.prev.unwrap(), s.next.unwrap());
            let tab_h = c.tab_strip_height() as f32;
            assert_eq!(p.min.y, 20.0, "the near chevron is at the top");
            assert_eq!(n.max.y, 320.0, "the far one at the bottom");
            assert_eq!(p.height(), tab_h);
            assert_eq!(p.width(), c.tab_strip_extent() as f32, "as wide as the strip");
            assert_eq!(s.slots[0].index, 3);
            assert_eq!(s.slots[0].rect.min.y, p.max.y);
            for w in s.slots.windows(2) {
                assert_eq!(w[0].rect.max.y, w[1].rect.min.y);
                assert_eq!(w[0].rect.width(), w[1].rect.width());
            }
        }
    }

    /// A control too short to hold two chevrons and a tab has no room to scroll in: it keeps the old rule.
    #[test]
    fn a_strip_too_small_for_chevrons_keeps_the_old_rule() {
        let c = tabs(&sheets(10), "top", 90, 100);
        let s = tab_strip(Pos2::ZERO, &c, 0);
        assert!(!s.overflows());
        let got: Vec<Rect> = s.slots.iter().map(|s| s.rect).collect();
        assert_eq!(got, the_old_rule(Pos2::ZERO, &c));
        assert_eq!(s.last_first, 0);
        assert_eq!(scroll_to(&c, 0, 9), 0, "nothing to scroll to");
    }

    /// `scroll_to`: not at all when the tab is on show; back to it when it is to the left; just far enough for it to be the
    /// last tab drawn when it is to the right; and a selection past the last tab (none) moves nothing.
    #[test]
    fn a_selection_is_brought_into_view_with_the_least_movement() {
        let c = tabs(&sheets(32), "top", 1152, 560);
        let range = scroll_range(&c);
        let drawn = tab_strip(Pos2::ZERO, &c, 0).slots.len();
        // On show: nothing moves, wherever the strip is.
        assert_eq!(scroll_to(&c, 0, 0), 0);
        assert_eq!(scroll_to(&c, 0, drawn - 1), 0);
        assert_eq!(scroll_to(&c, 5, 5), 5);
        // To the left: the tab becomes the first one drawn.
        assert_eq!(scroll_to(&c, 12, 3), 3);
        // To the right: it becomes the LAST one drawn.
        let first = scroll_to(&c, 0, 20);
        let s = tab_strip(Pos2::ZERO, &c, first);
        assert_eq!(s.slots.last().unwrap().index, 20, "the selected tab is the last drawn");
        assert!(first > 0 && first <= range);
        // ...and one tab less would not have shown it, so the move is the least there is.
        assert!(tab_strip(Pos2::ZERO, &c, first - 1).rect_of(20).is_none() || first == 0);
        // The very last tab.
        assert_eq!(scroll_to(&c, 0, 31), range);
        // Nothing is selected (one past the last name, or any larger number): nothing moves.
        assert_eq!(scroll_to(&c, 9, 32), 9);
        assert_eq!(scroll_to(&c, 9, 4000), 9);
        // A strip nobody has scrolled starts where the selection is on show.
        let mut sel = c.clone();
        sel.set_prop("SelectedTab", PropValue::Int(28));
        let auto = auto_first(&sel);
        assert!(tab_strip(Pos2::ZERO, &sel, auto).rect_of(27).is_some());
        let mut every = 0;
        for k in 1..=32 {
            sel.set_prop("SelectedTab", PropValue::Int(k));
            let f = auto_first(&sel);
            assert!(tab_strip(Pos2::ZERO, &sel, f).rect_of(k as usize - 1).is_some(), "tab {k} on show");
            every += 1;
        }
        eprintln!("TabStrip into view — least-movement rule holds on 32 tabs; the auto start shows the selection for all {every} of them");
    }

    /// What a point on the strip is over.
    #[test]
    fn hit_finds_a_tab_or_a_chevron() {
        let c = tabs(&sheets(32), "top", 1152, 560);
        let s = tab_strip(pos2(5.0, 9.0), &c, 4);
        let p = s.prev.unwrap();
        let n = s.next.unwrap();
        assert_eq!(hit(&s, p.center()), Some(TabHit::Prev));
        assert_eq!(hit(&s, n.center()), Some(TabHit::Next));
        for slot in &s.slots {
            assert_eq!(hit(&s, slot.rect.center()), Some(TabHit::Tab(slot.index)));
        }
        assert_eq!(hit(&s, pos2(5.0, 200.0)), None, "the page below the strip is not the strip");
        // The page a click selects is the tab's own index, not its position among the ones drawn.
        assert_eq!(hit(&s, s.slots[0].rect.center()), Some(TabHit::Tab(4)));
    }

    /// The designer canvas keeps no scroll: a click selects the tab it lands on by its own index, and a chevron steps the
    /// selection one tab, the strip following it. Clicking one tab after another with the far chevron walks the whole
    /// strip, which is how a designer reaches tab 32 of 32.
    #[test]
    fn the_designer_selects_by_index_and_its_chevrons_step_the_selection() {
        let mut c = tabs(&sheets(32), "top", 400, 240);
        c.set_prop("SelectedTab", PropValue::Int(20));
        let strip = tab_strip(pos2(30.0, 40.0), &c, auto_first(&c));
        assert!(strip.slots[0].index > 0, "tab 20 was not on show at the start: the strip is scrolled");
        for slot in &strip.slots {
            assert_eq!(click_target(&c, &strip, slot.rect.center()), Some(slot.index), "a tab is itself, wherever it sits");
        }
        assert_eq!(click_target(&c, &strip, strip.prev.unwrap().center()), Some(18), "the near chevron: tab 19 (index 18)");
        assert_eq!(click_target(&c, &strip, strip.next.unwrap().center()), Some(20), "the far chevron: tab 21 (index 20)");
        assert_eq!(click_target(&c, &strip, pos2(30.0, 400.0)), None, "off the strip");
        // Walk from tab 1 to tab 32 by the far chevron alone: 31 clicks, each one tab on, every tab drawn at its turn.
        let mut reached = 0;
        let mut sel = 0usize;
        while sel < 31 {
            c.set_prop("SelectedTab", PropValue::Int(sel as i64 + 1));
            let s = tab_strip(Pos2::ZERO, &c, auto_first(&c));
            assert!(s.rect_of(sel).is_some(), "the selected tab {} is on show", sel + 1);
            sel = click_target(&c, &s, s.next.unwrap().center()).unwrap();
            reached += 1;
        }
        assert_eq!((sel, reached), (31, 31));
        // The ends hold: before the first and past the last stay put.
        c.set_prop("SelectedTab", PropValue::Int(1));
        let s = tab_strip(Pos2::ZERO, &c, auto_first(&c));
        assert_eq!(click_target(&c, &s, s.prev.unwrap().center()), Some(0));
        c.set_prop("SelectedTab", PropValue::Int(32));
        let s = tab_strip(Pos2::ZERO, &c, auto_first(&c));
        assert_eq!(click_target(&c, &s, s.next.unwrap().center()), Some(31));
        eprintln!("TabStrip designer clicks — tab 20 of 32 selected: {} tabs drawn from tab {}; 31 far-chevron clicks walk tab 1 to tab 32; the ends hold", strip.slots.len(), strip.slots[0].index + 1);
    }
}
