// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! A TabControl's tab strip: where each tab sits, and how a strip with more tabs than it has room for slides.
//!
//! # The rule that was here before 2.0.89
//!
//! Tabs sit edge to edge along their edge of the control, each as wide as its label (on a side strip: as tall as one
//! tab, all as wide as the widest). The first tab that would overrun the control was left out, and every tab after it:
//! neither painted nor clickable. With PowerExcel's 32 sheet names (FontSize 15) in a 1152 px strip, 9 were there and 23
//! could not be reached.
//!
//! # The rule now
//!
//! * **Everything fits:** nothing changes. The same rectangles, and nothing to scroll.
//! * **It does not fit:** the tabs are one long row that slides under the strip's box. `offset` is how far it has slid,
//!   in points: 0 puts the first tab at the start of the strip, `max_offset` puts the end of the last tab at the far end
//!   of it. A tab at either edge is cut by the box, as the tabs of the designer's layer bar are, and there are no buttons:
//!   a swipe, the wheel or a drag slides the row (`render` reads them), and pressing a tab that is cut selects it and
//!   brings it wholly into view.
//! * **A selection is brought into view, but only WHEN IT CHANGES** (a click, an arrow key, Home, End, Ctrl+Tab or a
//!   COBOL write of `SelectedTab`), with a sliver of the next tab showing beside it, so a tab brought in at an edge does
//!   not look like the last one. Whoever slides the strip away from the selected tab is not pulled back to it.
//!
//! # Why this is its own module
//!
//! The strip is laid out for five readers that must agree to the pixel: the painter that draws the tabs, the engine that
//! hit-tests them, the focus ring around the selected one, the designer canvas that selects a page by clicking a tab, and
//! the tests. A strip laid out five times is a strip that eventually draws in one place and answers clicks in another
//! (the same reason `splitter::geometry` exists), so it is laid out ONCE, here, from the control and an offset.
//!
//! This module is geometry and arithmetic. It reads no input and keeps no state: `render` owns the offset (view state, kept
//! in egui's memory like a Panel's scroll offset: it is not part of the form and is never saved) and calls in here.

use egui::{pos2, vec2, Pos2, Rect, Vec2};

use crate::model::Control;

/// One tab as placed: which tab it is (from 0, in `Tabs` order) and where it sits.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TabSlot {
    pub index: usize,
    /// The whole tab, where the strip's offset puts it: it can reach past the band at either end.
    pub rect: Rect,
    /// The part of the tab inside the band. This is what is painted (the painter clips to the band) and what a press can hit.
    pub shown: Rect,
}

/// A TabControl's strip, laid out for one offset.
#[derive(Debug, Clone, PartialEq)]
pub struct TabStrip {
    /// Every tab that has a part inside the band, in order. The first and the last can be cut by its edges.
    pub slots: Vec<TabSlot>,
    /// The band the strip occupies along its edge of the control: where tabs show and where a gesture slides them.
    pub band: Rect,
    /// How far the tabs have slid, held between 0 and `max_offset`.
    pub offset: f32,
    /// How far they can slide: 0 when everything fits.
    pub max_offset: f32,
    /// How many tabs there are, shown or not.
    pub count: usize,
    /// A side strip (Left / Right) runs down the control rather than across it.
    pub vertical: bool,
}

impl TabStrip {
    /// The tabs do not all fit, so the strip slides.
    pub fn overflows(&self) -> bool {
        self.max_offset > 0.0
    }

    /// Where tab `index` sits, when any of it is inside the band.
    pub fn rect_of(&self, index: usize) -> Option<Rect> {
        self.slots.iter().find(|s| s.index == index).map(|s| s.rect)
    }

    /// The part of tab `index` inside the band.
    pub fn shown_of(&self, index: usize) -> Option<Rect> {
        self.slots.iter().find(|s| s.index == index).map(|s| s.shown)
    }

    /// The tabs' shown parts, joined: the keyboard stop of the control. `None` when no tab is shown.
    pub fn tabs_rect(&self) -> Option<Rect> {
        self.slots.iter().map(|s| s.shown).reduce(|a, b| a.union(b))
    }

    /// How far a gesture of `v` (a drag, a swipe) goes along the strip: sideways on a top or bottom strip, down on a side one.
    pub fn along(&self, v: Vec2) -> f32 {
        if self.vertical {
            v.y
        } else {
            v.x
        }
    }

    /// The travel of a wheel or swipe delta that slides this strip. A side strip slides with the vertical wheel. On a top or
    /// bottom strip the vertical wheel slides it sideways too (a wheel only turns up and down), unless the gesture is
    /// plainly sideways.
    pub fn travel(&self, delta: Vec2) -> f32 {
        if self.vertical || delta.y.abs() >= delta.x.abs() {
            delta.y
        } else {
            delta.x
        }
    }
}

/// The tab titles, from `Tabs` (one per line, in order).
pub fn tab_titles(ctrl: &Control) -> Vec<String> {
    ctrl.get_prop("Tabs")
        .map(|v| v.as_str().lines().map(|s| s.to_string()).collect())
        .unwrap_or_default()
}

/// How many tabs the control has, shown or not.
pub fn tab_count(ctrl: &Control) -> usize {
    ctrl.get_prop("Tabs").map_or(0, |v| v.as_str().lines().count())
}

/// The strip position (from 0) of the page `SelectedTab` names (from 1); 0 or less reads as the first page. It may be past
/// the last tab: a program that writes one past the last name marks no tab at all.
pub fn selected_position(ctrl: &Control) -> usize {
    ctrl.get_prop("SelectedTab").map(|v| v.as_i64()).unwrap_or(1).max(1) as usize - 1
}

/// The strip's measurements along its own axis, from the control alone (no offset, no origin).
struct Layout {
    /// Each tab's extent along the strip.
    lens: Vec<f32>,
    /// A tab's extent across the strip.
    cross: f32,
    /// The strip runs down the control (Left / Right).
    vertical: bool,
    /// The strip's length: the control's width, or its height for a side strip.
    avail: f32,
    /// How far the tabs can slide: 0 when they fit.
    max_offset: f32,
    /// How much of the neighbouring tab `reveal` leaves showing beside the tab it brings into view: enough of it to say
    /// that there are more tabs that way, and to press.
    peek: f32,
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
        let lens: Vec<f32> = tab_titles(ctrl).iter().map(|t| if vertical { tab_h } else { ctrl.tab_width(t) as f32 }).collect();
        let cross = if vertical { strip } else { tab_h };
        let avail = if vertical { size.y } else { size.x };
        let total: f32 = lens.iter().sum();
        Layout { lens, cross, vertical, avail, max_offset: (total - avail).max(0.0), peek: tab_h * 0.75, band }
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

/// The strip of `ctrl` at `origin` (the control's top-left corner, in whatever space the caller paints in), with its tabs
/// slid by `offset`. `offset` is held between 0 and the furthest the strip slides, so any value is safe.
pub fn tab_strip(origin: Pos2, ctrl: &Control, offset: f32) -> TabStrip {
    let l = Layout::of(origin, ctrl);
    let offset = if offset.is_nan() { 0.0 } else { offset.clamp(0.0, l.max_offset) };
    // Edge to edge, accumulated in absolute coordinates so that one tab's far edge IS the next one's near edge, to the bit
    // (and, at offset 0, to the bit of the rule that was here before).
    let mut cursor = l.axis_min() - offset;
    let mut slots = Vec::new();
    for (index, &len) in l.lens.iter().enumerate() {
        let rect = l.at(cursor, len);
        let shown = rect.intersect(l.band);
        if shown.width() > 0.0 && shown.height() > 0.0 {
            slots.push(TabSlot { index, rect, shown });
        }
        cursor += len;
    }
    TabStrip { slots, band: l.band, offset, max_offset: l.max_offset, count: l.lens.len(), vertical: l.vertical }
}

/// How far the strip of `ctrl` slides: 0 when every tab fits.
pub fn max_offset(ctrl: &Control) -> f32 {
    Layout::of(Pos2::ZERO, ctrl).max_offset
}

/// Where the strip should be slid so that tab `selected` is wholly on show, moving it as little as possible from `offset`:
/// not at all when the tab is already wholly on show; otherwise just far enough that the tab is whole, and a little more,
/// so that a sliver of the neighbouring tab shows beside it on the side it came from (the sliver says there are more tabs
/// that way, and can be pressed to walk on). The strip stops at its ends, where there is no neighbour. A tab longer than
/// the strip starts at the strip's start. A `selected` past the last tab (nothing is selected) moves nothing.
pub fn reveal(ctrl: &Control, offset: f32, selected: usize) -> f32 {
    let l = Layout::of(Pos2::ZERO, ctrl);
    if l.max_offset <= 0.0 {
        return 0.0;
    }
    let offset = if offset.is_nan() { 0.0 } else { offset.clamp(0.0, l.max_offset) };
    let Some(&len) = l.lens.get(selected) else {
        return offset;
    };
    let start: f32 = l.lens[..selected].iter().sum();
    let end = start + len;
    if start >= offset && end <= offset + l.avail {
        return offset;
    }
    let to = if len >= l.avail {
        start
    } else if start < offset {
        // Cut or hidden at the start: the tab, and the end of the tab before it.
        (start - l.peek).max(end - l.avail)
    } else {
        // Cut or hidden at the end: the tab, and the start of the tab after it.
        (end - l.avail + l.peek).min(start)
    };
    to.clamp(0.0, l.max_offset)
}

/// Where the strip is when nothing has ever slid it: just far enough for the selected tab to be wholly on show. This is what
/// a surface with no scroll state (a still picture) draws, and what a running strip starts at.
pub fn auto_offset(ctrl: &Control) -> f32 {
    reveal(ctrl, 0.0, selected_position(ctrl))
}

/// The tab under `p` on `strip`: only the part of a tab that shows can be pressed. `None` off the strip.
pub fn hit(strip: &TabStrip, p: Pos2) -> Option<usize> {
    strip.slots.iter().find(|s| s.shown.contains(p)).map(|s| s.index)
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

    /// Strips that fit are laid out as they always were: the same rectangles to the bit, whole, nothing to slide, at any
    /// origin (a fractional one included: the far edge of a tab must still be the near edge of the next).
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
                        let strip = tab_strip(origin, &c, 0.0);
                        let got: Vec<Rect> = strip.slots.iter().map(|s| s.rect).collect();
                        assert_eq!(got, the_old_rule(origin, &c), "{position} {count} tabs {w}x{h} at {origin:?}");
                        assert!(strip.slots.iter().all(|s| s.shown == s.rect), "{position}: every tab whole");
                        assert!(!strip.overflows(), "{position} {count} tabs {w}x{h}: nothing to slide");
                        assert_eq!(strip.max_offset, 0.0);
                        assert_eq!(strip.slots.iter().map(|s| s.index).collect::<Vec<_>>(), (0..count).collect::<Vec<_>>());
                        // A fitting strip cannot be slid, whatever it is asked.
                        assert_eq!(tab_strip(origin, &c, 90.0).slots, strip.slots, "{position}: offset held at 0");
                        cases += 1;
                        tab_total += count;
                    }
                }
            }
        }
        assert!(cases > 100, "the sweep must cover many strips: {cases}");
        eprintln!("TabStrip fits — {cases} strips ({tab_total} tabs) laid out exactly as the old rule laid them, whole, with nothing to slide");
    }

    /// 32 sheet names in a 1152 px strip, the real case this feature exists for: from the start the tabs the old rule drew
    /// are there, whole, edge to edge, and one more is cut by the strip's far edge; at the far end the last tab ends at the
    /// strip's end and the first one showing is cut at its start.
    #[test]
    fn a_strip_that_overflows_slides_under_its_band_and_cuts_the_tab_at_the_edge() {
        let c = tabs(&sheets(32), "top", 1152, 560);
        let old = the_old_rule(Pos2::ZERO, &c);
        let strip = tab_strip(Pos2::ZERO, &c, 0.0);
        assert!(strip.overflows());
        let whole: Vec<Rect> = strip.slots.iter().filter(|s| s.shown == s.rect).map(|s| s.rect).collect();
        assert_eq!(whole, old, "the tabs that fit are the ones the old rule drew, to the bit");
        let cut = strip.slots.last().unwrap();
        assert_eq!(strip.slots.len(), old.len() + 1, "and one more, cut by the far edge");
        assert!(cut.shown.width() > 0.0 && cut.shown.width() < cut.rect.width());
        assert_eq!(cut.shown.max.x, 1152.0, "the cut tab runs to the strip's end");
        assert_eq!(cut.index, old.len());
        for w in strip.slots.windows(2) {
            assert_eq!(w[0].rect.max.x, w[1].rect.min.x, "edge to edge");
        }
        // At the far end.
        let end = tab_strip(Pos2::ZERO, &c, strip.max_offset);
        let last = end.slots.last().unwrap();
        assert_eq!(last.index, 31);
        assert!((last.rect.max.x - 1152.0).abs() < 0.01, "the last tab ends where the strip ends: {}", last.rect.max.x);
        assert_eq!(last.shown, last.rect, "whole");
        let first = end.slots.first().unwrap();
        assert!(first.shown.width() <= first.rect.width() && first.shown.min.x == 0.0, "the first one showing starts at the strip's start");
        assert_eq!(end.offset, strip.max_offset);
        // Past the range is held at the range, before 0 at 0.
        assert_eq!(tab_strip(Pos2::ZERO, &c, strip.max_offset + 500.0).offset, strip.max_offset);
        assert_eq!(tab_strip(Pos2::ZERO, &c, -40.0).offset, 0.0);
        assert_eq!(tab_strip(Pos2::ZERO, &c, f32::NAN).offset, 0.0);
        eprintln!(
            "TabStrip overflow — 32 tabs in 1152 px: {} whole at the start (as the old rule) + 1 cut at {:.0} of {:.0} px; slides {:.0} px; at the far end tab 32 ends at {:.2}",
            old.len(),
            cut.shown.width(),
            cut.rect.width(),
            strip.max_offset,
            last.rect.max.x
        );
    }

    /// Sliding one point at a time from the start to the end shows every tab wholly at some offset, on every edge and for
    /// strips of 9 to 120 tabs; no tab is ever placed outside its band's cross axis; the shown part never leaves the band.
    #[test]
    fn sliding_the_strip_shows_every_tab_whole_at_some_offset() {
        let mut checked = 0;
        let mut worst_cut = 0.0_f32;
        for position in ["top", "bottom", "left", "right"] {
            for (count, w, h) in [(32, 1152, 560), (120, 1152, 560), (40, 640, 300), (9, 330, 200)] {
                let c = tabs(&sheets(count), position, w, h);
                let max = max_offset(&c);
                assert!(max > 0.0, "{position} {count} tabs in {w}x{h}: must overflow");
                let mut whole = vec![false; count];
                let mut offset = 0.0;
                while offset <= max {
                    let s = tab_strip(pos2(3.0, 5.0), &c, offset);
                    assert_eq!(s.offset, offset);
                    for pair in s.slots.windows(2) {
                        assert_eq!(pair[1].index, pair[0].index + 1, "{position}: the tabs shown are in a run");
                    }
                    for slot in &s.slots {
                        assert!(s.band.contains_rect(slot.shown), "{position}: shown outside its band");
                        if slot.shown == slot.rect {
                            whole[slot.index] = true;
                        } else {
                            worst_cut = worst_cut.max(1.0 - slot.shown.area() / slot.rect.area());
                        }
                    }
                    checked += 1;
                    offset += 1.0;
                }
                assert!(whole.iter().all(|w| *w), "{position} {count} tabs: all of them shown whole by sliding");
                let end = tab_strip(Pos2::ZERO, &c, max);
                assert_eq!(end.slots.last().unwrap().index, count - 1, "{position}: the last tab at the furthest");
            }
        }
        eprintln!("TabStrip slide — {checked} offsets (1 pt apart) on 16 strips (4 edges x 4 sizes, up to 120 tabs): every tab shown whole at some offset, the last one at the furthest, never outside its band");
    }

    /// A side strip slides down the control: tabs stacked as tall as one tab, all as wide as the strip.
    #[test]
    fn a_side_strip_slides_vertically() {
        for position in ["left", "right"] {
            let c = tabs(&sheets(30), position, 400, 300);
            let s = tab_strip(pos2(10.0, 20.0), &c, 55.0);
            assert!(s.vertical && s.overflows());
            let tab_h = c.tab_strip_height() as f32;
            assert_eq!(s.slots[0].rect.min.y, 20.0 - 55.0 + tab_h * s.slots[0].index as f32, "the offset moves tabs up");
            for w in s.slots.windows(2) {
                assert_eq!(w[0].rect.max.y, w[1].rect.min.y);
                assert_eq!(w[0].rect.width(), w[1].rect.width());
                assert_eq!(w[0].rect.width(), c.tab_strip_extent() as f32, "as wide as the strip");
            }
            assert_eq!(s.slots[0].shown.min.y, 20.0, "cut at the top of the band");
            assert_eq!(s.slots.last().unwrap().shown.max.y, 320.0, "and at the bottom");
            // The gesture axes: down the strip.
            assert_eq!(s.along(vec2(7.0, 3.0)), 3.0);
            assert_eq!(s.travel(vec2(7.0, 3.0)), 3.0, "a side strip slides with the vertical wheel");
        }
        let top = tab_strip(Pos2::ZERO, &tabs(&sheets(30), "top", 400, 300), 0.0);
        assert_eq!(top.along(vec2(7.0, 3.0)), 7.0, "a top strip: sideways");
        assert_eq!(top.travel(vec2(0.0, 9.0)), 9.0, "the vertical wheel slides a top strip sideways");
        assert_eq!(top.travel(vec2(9.0, 2.0)), 9.0, "a plainly sideways swipe is itself");
    }

    /// `reveal`: not at all when the tab is wholly on show; to the start of the strip when it is cut or hidden at the start;
    /// to the end of the strip when it is cut or hidden at the end; and a selection past the last tab (none) moves nothing.
    #[test]
    fn a_selection_is_brought_into_view_with_the_least_movement() {
        let c = tabs(&sheets(32), "top", 1152, 560);
        let max = max_offset(&c);
        // Where each tab starts along the row of tabs, before any sliding.
        let widths: Vec<f32> = tab_titles(&c).iter().map(|t| c.tab_width(t) as f32).collect();
        let start = |k: usize| widths[..k].iter().sum::<f32>();
        // On show: nothing moves, wherever the strip is.
        assert_eq!(reveal(&c, 0.0, 0), 0.0);
        assert_eq!(reveal(&c, 0.0, 3), 0.0);
        assert_eq!(reveal(&c, 120.0, 4), 120.0);
        // Hidden or cut at the start: the tab comes to the strip's start, less a sliver of the tab before it.
        let peek = Layout::of(Pos2::ZERO, &c).peek;
        assert!(peek > 0.0 && peek < widths[0]);
        assert_eq!(reveal(&c, 900.0, 3), start(3) - peek);
        assert_eq!(reveal(&c, start(2) + 10.0, 2), start(2) - peek, "a tab cut at the start");
        assert_eq!(reveal(&c, 100.0, 0), 0.0, "the strip stops at its start: there is no tab before the first");
        // Hidden or cut at the end: the tab comes to the strip's end, less a sliver of the tab after it.
        let moved = reveal(&c, 0.0, 20);
        let s = tab_strip(Pos2::ZERO, &c, moved);
        let shown = s.rect_of(20).unwrap();
        assert!((shown.max.x - (1152.0 - peek)).abs() < 0.01, "the selected tab ends a sliver before the strip's end: {}", shown.max.x);
        assert_eq!(s.shown_of(20), Some(shown), "and is whole");
        let sliver = s.shown_of(21).expect("the next tab peeks out beside it");
        assert!((sliver.width() - peek).abs() < 0.01 && s.rect_of(21).unwrap().width() > peek, "a sliver of it, not all");
        assert!(moved > 0.0 && moved <= max);
        // The very last tab.
        assert!((reveal(&c, 0.0, 31) - max).abs() < 0.01);
        // Nothing is selected (one past the last name, or any larger number): nothing moves.
        assert_eq!(reveal(&c, 90.0, 32), 90.0);
        assert_eq!(reveal(&c, 90.0, 4000), 90.0);
        // A strip nobody has slid starts where the selection is wholly on show.
        let mut sel = c.clone();
        let mut every = 0;
        for k in 1..=32 {
            sel.set_prop("SelectedTab", PropValue::Int(k));
            let at = auto_offset(&sel);
            let s = tab_strip(Pos2::ZERO, &sel, at);
            assert_eq!(s.shown_of(k as usize - 1), s.rect_of(k as usize - 1), "tab {k} wholly on show at the auto offset");
            every += 1;
        }
        // A tab longer than the strip starts at its start.
        let tiny = tabs(&sheets(6), "top", 30, 100);
        let tiny_widths: Vec<f32> = tab_titles(&tiny).iter().map(|t| tiny.tab_width(t) as f32).collect();
        assert!(tiny_widths.iter().all(|w| *w > 30.0), "every tab is longer than the strip");
        assert_eq!(reveal(&tiny, 0.0, 3), tiny_widths[..3].iter().sum::<f32>());
        eprintln!("TabStrip into view — least-movement rule holds on 32 tabs in 1152 px; the auto offset shows the selection whole for all {every} of them");
    }

    /// What a point on the strip is over: the tab under it, by its own index, and only the part of a tab that shows.
    #[test]
    fn hit_finds_the_tab_under_a_point_and_only_its_shown_part() {
        let c = tabs(&sheets(32), "top", 1152, 560);
        let s = tab_strip(pos2(5.0, 9.0), &c, 130.0);
        for slot in &s.slots {
            assert_eq!(hit(&s, slot.shown.center()), Some(slot.index), "a tab is itself, wherever it sits");
        }
        assert_eq!(hit(&s, pos2(5.0 + 600.0, 200.0)), None, "the page below the strip is not the strip");
        let first = s.slots[0];
        assert!(first.rect.min.x < s.band.min.x, "the first tab is cut at the start");
        assert_eq!(hit(&s, pos2(first.rect.min.x + 1.0, 12.0)), None, "the part of a tab that is not shown cannot be pressed");
        assert_eq!(hit(&s, first.shown.center()), Some(first.index));
        assert!(first.index > 0, "the strip has slid, so the first tab shown is not tab 1");
    }

    /// The designer canvas keeps no scroll: it draws the strip slid just far enough for the selected tab to be on show,
    /// and a press selects the tab it lands on by its own index, cut or whole. Pressing the tab cut at the far edge, again
    /// and again, walks the whole strip, which is how a designer reaches tab 32 of 32 with nothing to scroll by.
    #[test]
    fn the_designer_walks_the_strip_by_pressing_the_tab_cut_at_its_edge() {
        let mut c = tabs(&sheets(32), "top", 400, 240);
        c.set_prop("SelectedTab", PropValue::Int(20));
        let strip = tab_strip(pos2(30.0, 40.0), &c, auto_offset(&c));
        assert!(strip.offset > 0.0, "tab 20 was not on show at the start: the strip is slid");
        for slot in &strip.slots {
            assert_eq!(hit(&strip, slot.shown.center()), Some(slot.index));
        }
        assert_eq!(strip.shown_of(19), strip.rect_of(19), "the selected tab is whole");
        // Walk from tab 1 to tab 32 by pressing the part of the tab that is cut at the far edge.
        let mut sel = 0usize;
        let mut presses = 0;
        while sel < 31 {
            c.set_prop("SelectedTab", PropValue::Int(sel as i64 + 1));
            let s = tab_strip(Pos2::ZERO, &c, auto_offset(&c));
            assert_eq!(s.shown_of(sel), s.rect_of(sel), "the selected tab {} is whole", sel + 1);
            // The last tab showing beyond the selected one: cut by the strip's far edge, until the strip's end is reached.
            let next = s.slots.iter().rev().find(|t| t.index > sel).expect("a tab after the selected one shows");
            if sel < 28 {
                assert!(next.shown != next.rect, "the last tab that shows is cut by the far edge");
            }
            sel = hit(&s, next.shown.center()).unwrap();
            presses += 1;
            assert!(presses <= 31);
        }
        assert_eq!(sel, 31);
        eprintln!("TabStrip designer — tab 20 of 32 selected: strip slid {:.0} px; {presses} presses on the tab cut at the far edge walk tab 1 to tab 32", strip.offset);
    }
}
