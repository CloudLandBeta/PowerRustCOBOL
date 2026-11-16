// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! **What is inside an expanded card grows with it** (spec 090; the desktop's `layout::place_children`, `scale::place_zoomed`,
//! `scale::zoom_of` and `zoom_font`).
//!
//! An expanded card takes the whole client area of its container: the browser lays that out
//! ([`crate::layout_css::expanded_placement`]). The desktop then lays out what is INSIDE the card against its new rectangle by ONE
//! factor, so nothing is distorted: the smaller of the horizontal and the vertical growth of the card's client area, taken from the
//! size the card was DESIGNED at (not from the cell it had). Every control that follows its container's anchors takes its designed
//! place and size times that factor, the whole block centred in the room that is left over on the side the aspect ratio does not
//! fill, whatever its `Anchor` says; and the text of every control grows by the same factor, within its own `MinFontSize` and
//! `MaxFontSize`, unless it opted out with `ScaleFont`. A docked control, or an item of a Flex, Grid or Flow container, is placed by
//! its container's rule and keeps its size; its text still grows.
//!
//! The browser decides how big the card is, so the page measures it and tells the tree
//! ([`crate::tree::FormTree::set_card_box`]). This module then writes what the controls inside would have been laid out at — their
//! `X`, `Y`, `Width`, `Height` and `FontSize`, and an `Anchor` of the top left corner they are placed from — into the effective
//! controls ([`crate::tree::FormTree::effective_ctls`]), so that everything the page derives from a control's rectangle and size of
//! lettering (the anchors, a caption shrunk to fit, a chart's lettering) follows the zoomed values with no rule of its own.
//!
//! Not reproduced: the inside of a TabControl (its text grows, its pages do not move), and of a container inside the card that is
//! itself docked or an item of a Flex, Grid or Flow container (the browser places it; its text grows).

use std::collections::BTreeMap;

use cobolt_forms::layout::defaults::{FONT_SIZE_MAX, FONT_SIZE_MIN};
use cobolt_forms::layout::props::Limits;
use cobolt_forms::layout::{scale, Insets, LRect};
use cobolt_forms::model::ControlType;

use crate::layout_css::{self, Mode};
use crate::tree::Ctl;
use crate::value::{fmt_number, turned_on};

/// The rectangle a control was designed at, as the page holds it (the properties `layout_css::item` reads).
fn designed_rect(c: &Ctl) -> LRect {
    let n = |k: &str, d: f64| c.num(k).unwrap_or(d) as f32;
    LRect::new(n("x", 0.0), n("y", 0.0), n("width", 100.0), n("height", 24.0))
}

/// `Padding`, with any of `PaddingLeft`/`Top`/`Right`/`Bottom` that is set overriding its side (`layout::props::padding`).
fn padding(c: &Ctl) -> Insets {
    let all = c.num("padding").unwrap_or(0.0) as f32;
    let side = |k: &str| c.num(k).map(|v| v as f32).unwrap_or(all);
    Insets { left: side("paddingleft"), top: side("paddingtop"), right: side("paddingright"), bottom: side("paddingbottom") }
}

/// The client rectangle of container `c` laid at `at`: what the desktop takes off a Panel or a GroupBox on every side
/// (`Control::content_rect`), then the container's own padding (`layout::client_of`).
fn client_of(c: &Ctl, at: LRect) -> LRect {
    let chrome = layout_css::chrome_inset(c) as f32;
    let p = padding(c);
    at.deflate(Insets { left: chrome + p.left, top: chrome + p.top, right: chrome + p.right, bottom: chrome + p.bottom })
}

fn limits(c: &Ctl, min: &str, max: &str) -> Limits {
    Limits { min: c.num(min).unwrap_or(0.0) as f32, max: c.num(max).unwrap_or(0.0) as f32 }
}

/// The factor expanded card `w`, measured at `size` (its border box, in pixels), grows its contents by: `scale::zoom_of` of the
/// client area it was designed at and the one it has now.
pub fn factor(ctls: &[Ctl], w: usize, size: (f64, f64)) -> f32 {
    let c = &ctls[w];
    let designed = designed_rect(c);
    let laid = LRect::new(designed.x, designed.y, size.0 as f32, size.1 as f32);
    scale::zoom_of(client_of(c, designed), client_of(c, laid))
}

/// The expanded card control `i` is inside (or is), and the factor it grows its contents by, when the page has measured it.
///
/// The OUTERMOST expanded card decides: the desktop passes its factor down to everything inside, an expanded card in there
/// included (`zoom.or_else(...)` in `place_children`). An outer card the page has not measured yet gives no factor to anything
/// inside it, however many cards inside are measured.
pub fn origin_of(ctls: &[Ctl], boxes: &BTreeMap<usize, (f64, f64)>, i: usize) -> Option<(usize, f32)> {
    // An expansion is a layout: a form that is not laid out (not responsive) never expands a card, so there is nothing to follow.
    if !ctls[0].flag("responsive") {
        return None;
    }
    let mut chain = Vec::new();
    let mut at = i;
    while at != 0 && chain.len() <= ctls.len() {
        chain.push(at);
        match ctls[at].parent {
            Some(p) => at = p,
            None => break,
        }
    }
    let card = chain.into_iter().rev().find(|&a| layout_css::is_expanded_card(ctls, a))?;
    boxes.get(&card).map(|size| (card, factor(ctls, card, *size)))
}

/// A control's text in an expanded card (`zoom_font`): the size it would have had, `effective`, times `z`, within the control's own
/// `MinFontSize` and `MaxFontSize` and the engine's bounds. A control that opted out of font scaling keeps its size.
pub fn zoomed_size(c: &Ctl, effective: f64, z: f32) -> f64 {
    if !c.raw("scalefont").map(|v| turned_on(v) && v.trim() != "0").unwrap_or(true) {
        return effective;
    }
    let bound = |k: &str| c.num(k).unwrap_or(0.0) as f32;
    let (lo, hi) = (bound("minfontsize"), bound("maxfontsize"));
    let mut size = effective as f32 * z;
    if hi > 0.0 && size > hi {
        size = hi;
    }
    if lo > 0.0 && size < lo {
        size = lo;
    }
    size.clamp(FONT_SIZE_MIN, FONT_SIZE_MAX) as f64
}

fn zoom_font(c: &mut Ctl, z: f32) {
    // A control that states no size keeps each painter's own default, unscaled.
    let Some(effective) = c.num("fontsize").filter(|s| *s > 0.0) else { return };
    let size = zoomed_size(c, effective, z);
    c.props.insert("fontsize".into(), fmt_number((size * 1000.0).round() / 1000.0));
}

fn put_rect(c: &mut Ctl, r: LRect) {
    let round = |v: f32| fmt_number((v as f64 * 1000.0).round() / 1000.0);
    c.props.insert("x".into(), round(r.x));
    c.props.insert("y".into(), round(r.y));
    c.props.insert("width".into(), round(r.w));
    c.props.insert("height".into(), round(r.h));
    // Placed from its top left corner at the size written, whatever it was anchored to.
    c.props.insert("anchor".into(), "Top,Left".into());
}

/// Lay out the children of `parent`, whose client area was designed as `dclient` and is `lclient` now (`None` where nothing says), at
/// factor `z`. `absolute` is whether they follow their anchors (`place_zoomed`); where the browser's layout places them, only their
/// text grows.
fn zoom_children(ctls: &mut [Ctl], parent: usize, dclient: LRect, lclient: Option<LRect>, z: f32, absolute: bool) {
    for k in ctls[parent].children.clone() {
        if !ctls[k].control_type().is_some_and(|t| !crate::kinds::is_non_visual(t)) {
            continue;
        }
        zoom_font(&mut ctls[k], z);
        // The rectangle it was designed at, read before it is written over.
        let designed = designed_rect(&ctls[k]);
        let docked = !ctls[k].text_or("dock", "None").trim().eq_ignore_ascii_case("none");
        let laid = match lclient {
            // A card that is expanded in here takes the whole client area of its container, wherever its own anchors would put it
            // (`layout::expand`): its contents are placed against that, at the factor of the card the whole thing began with.
            Some(area) if layout_css::is_expanded_card(ctls, k) => Some(area),
            Some(area) if absolute && !docked => Some(scale::place_zoomed(designed, dclient, area, z, limits(&ctls[k], "minwidth", "maxwidth"), limits(&ctls[k], "minheight", "maxheight"))),
            _ => None,
        };
        if let Some(r) = laid {
            put_rect(&mut ctls[k], r);
        }
        let repeating = ctls[k].flag("isrepeatinggroup");
        match ctls[k].control_type() {
            Some(ControlType::Panel | ControlType::GroupBox) if !repeating => {
                let inside = layout_css::mode_of(&ctls[k]) == Mode::Absolute;
                let dc = client_of(&ctls[k], designed);
                let lc = laid.map(|r| client_of(&ctls[k], r));
                zoom_children(ctls, k, dc, lc, z, inside && lc.is_some());
            }
            // A TabControl lays its pages out itself, but the page cannot say where they sit: the text inside grows.
            Some(ControlType::TabControl) => zoom_children(ctls, k, designed, None, z, false),
            _ => {}
        }
    }
}

/// Write the zoom of every expanded card the page has measured into `ctls` (the effective controls): see the module's text.
/// `boxes` is the measured border box of each card, by control.
pub fn apply(ctls: &mut [Ctl], boxes: &BTreeMap<usize, (f64, f64)>) {
    if boxes.is_empty() {
        return;
    }
    let origins: Vec<(usize, f32)> = boxes
        .keys()
        .copied()
        .filter(|&w| w < ctls.len())
        .filter_map(|w| origin_of(ctls, boxes, w).filter(|(card, _)| *card == w))
        .collect();
    for (card, z) in origins {
        // The card itself is laid out by the browser; its own lettering grows, and its contents are placed against its new area.
        let designed = designed_rect(&ctls[card]);
        let dclient = client_of(&ctls[card], designed);
        let (bw, bh) = boxes[&card];
        let lclient = client_of(&ctls[card], LRect::new(designed.x, designed.y, bw as f32, bh as f32));
        zoom_font(&mut ctls[card], z);
        let absolute = layout_css::mode_of(&ctls[card]) == Mode::Absolute;
        zoom_children(ctls, card, dclient, Some(lclient), z, absolute);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tree::testing::tree_of;
    use cobolt_forms::layout::{solve, LayoutInput};
    use cobolt_forms::model::PropValue;
    use cobolt_forms::xml::load_form_from_str;

    /// A 2 x 2 grid of cards on a 640 x 480 form. Card `A` is the one that gets expanded: an Absolute container whose contents are
    /// anchored every way the desktop's zoom has to ignore, one with a width limit, one that opted out of font scaling, one docked,
    /// and a Panel with a Label of its own inside.
    const CARDS: &str = r##"
      <Control id="G" type="Panel" x="0" y="0" w="640" h="480"><Property name="LayoutMode">Grid</Property><Property name="GridColumns">1fr 1fr</Property><Property name="GridRows">1fr 1fr</Property><Property name="Gap">20</Property><Property name="Padding">10</Property><Property name="Anchor">Top,Left,Right,Bottom</Property></Control>
      <Control id="A" type="GroupBox" parent="G" x="10" y="10" w="300" h="220"><Property name="GridColumn">1</Property><Property name="GridRow">1</Property><Property name="Expandable">1</Property><Property name="Padding">3</Property></Control>
      <Control id="A-TL" type="Label" parent="A" x="20" y="20" w="100" h="20"><Property name="FontSize">14</Property></Control>
      <Control id="A-TR" type="Label" parent="A" x="200" y="20" w="100" h="20"><Property name="FontSize">10</Property><Property name="Anchor">Top,Right</Property></Control>
      <Control id="A-W" type="Label" parent="A" x="20" y="60" w="270" h="30"><Property name="FontSize">11</Property><Property name="Anchor">Top,Left,Right</Property><Property name="MaxWidth">400</Property></Control>
      <Control id="A-BL" type="Button" parent="A" x="20" y="190" w="80" h="24"><Property name="FontSize">12</Property><Property name="Anchor">Bottom,Left</Property></Control>
      <Control id="A-FIX" type="Label" parent="A" x="150" y="190" w="100" h="20"><Property name="FontSize">12</Property><Property name="ScaleFont">false</Property></Control>
      <Control id="A-CAP" type="Label" parent="A" x="150" y="140" w="100" h="20"><Property name="FontSize">12</Property><Property name="MaxFontSize">20</Property></Control>
      <Control id="A-DOCK" type="Label" parent="A" x="13" y="13" w="294" h="18"><Property name="FontSize">9</Property><Property name="Dock">Bottom</Property></Control>
      <Control id="A-IN" type="Panel" parent="A" x="30" y="100" w="200" h="80"><Property name="Anchor">Top,Left</Property></Control>
      <Control id="A-IN-L" type="Label" parent="A-IN" x="40" y="110" w="100" h="20"><Property name="FontSize">9</Property><Property name="Anchor">Bottom,Right</Property></Control>
      <Control id="B" type="GroupBox" parent="G" x="330" y="10" w="300" h="220"><Property name="GridColumn">2</Property><Property name="GridRow">1</Property><Property name="Expandable">1</Property></Control>
      <Control id="B-L" type="Label" parent="B" x="340" y="20" w="100" h="20"><Property name="FontSize">14</Property></Control>
      <Control id="C" type="GroupBox" parent="G" x="10" y="250" w="620" h="220"><Property name="GridColumn">1</Property><Property name="GridRow">2</Property><Property name="ColumnSpan">2</Property></Control>
      <Control id="C-L" type="Label" parent="C" x="20" y="260" w="100" h="20"><Property name="FontSize">14</Property></Control>"##;

    /// What the effective control states for a numeric property.
    fn written(c: &Ctl, k: &str) -> f64 {
        c.num(k).unwrap_or(f64::NAN)
    }

    fn wrap(controls: &str) -> String {
        format!(r##"<?xml version="1.0" encoding="UTF-8"?>
<Form name="main-form" title="Demo" width="640" height="480" index-base="1" responsive="true">{controls}</Form>"##)
    }

    /// The desktop's own answer for `controls` with `card` expanded: every rectangle and every font size of its solver.
    fn desktop(controls: &str, card: &str, avail: (f32, f32)) -> (std::collections::HashMap<String, LRect>, std::collections::HashMap<String, f32>) {
        let mut form = load_form_from_str(&wrap(controls)).expect("the form loads");
        for c in form.controls.iter_mut().filter(|c| c.id == card) {
            c.set_prop("Expanded", PropValue::Bool(true));
        }
        let out = solve(&LayoutInput::new(&form.controls, (form.width as f32, form.height as f32), avail, &form.layout, &form.breakpoints));
        assert!(out.expanded_away.contains("B") || card != "A", "the siblings of the expanded card give their room up");
        (out.rects, out.font_sizes)
    }

    #[test]
    fn the_contents_of_an_expanded_card_land_where_the_desktops_solver_puts_them_at_every_window_size() {
        let mut report = Vec::new();
        for avail in [(640.0_f32, 480.0_f32), (1000.0, 700.0), (1440.0, 900.0), (500.0, 900.0), (900.0, 300.0)] {
            let (rects, fonts) = desktop(CARDS, "A", avail);
            let mut t = tree_of(CARDS);
        t.set(0, "Responsive", "1");
            let a = t.by_name("A").unwrap();
            t.set(a, "Expanded", "1");
            let laid = rects["A"];
            assert!(t.set_card_box(a, Some((laid.w as f64, laid.h as f64))));
            let eff = t.effective_ctls(None);
            // The card is laid out by the browser at its designed x and y here; the solver's frame is the form's. One translation.
            let (dx, dy) = (laid.x - 10.0, laid.y - 10.0);
            let mut checked = 0;
            for id in ["A-TL", "A-TR", "A-W", "A-BL", "A-FIX", "A-CAP", "A-IN", "A-IN-L"] {
                let k = t.by_name(id).unwrap();
                let got = [written(&eff[k], "x") as f32 + dx, written(&eff[k], "y") as f32 + dy, written(&eff[k], "width") as f32, written(&eff[k], "height") as f32];
                let want = rects[id];
                for (g, w, what) in [(got[0], want.x, "x"), (got[1], want.y, "y"), (got[2], want.w, "width"), (got[3], want.h, "height")] {
                    assert!((g - w).abs() < 0.01, "{id} {what} at {avail:?}: page {g}, desktop {w}");
                }
                assert_eq!(eff[k].raw("anchor"), Some("Top,Left"), "{id}: placed from its corner at the size written");
                checked += 1;
            }
            // The text of every control inside grows by the same factor, a control that opted out keeps its size, a limit holds.
            for id in ["A-TL", "A-TR", "A-W", "A-BL", "A-FIX", "A-CAP", "A-DOCK", "A-IN-L"] {
                let k = t.by_name(id).unwrap();
                let got = eff[k].num("fontsize").unwrap() as f32;
                assert!((got - fonts[id]).abs() < 0.001, "{id} lettering at {avail:?}: page {got}, desktop {}", fonts[id]);
                assert!((t.font_size_of(k).unwrap() as f32 - fonts[id]).abs() < 0.001, "{id}: the views read the same size");
                checked += 1;
            }
            // A docked control is placed by its dock: its rectangle is not written.
            let dock = t.by_name("A-DOCK").unwrap();
            assert_eq!(eff[dock].raw("width"), Some("294"), "a docked control keeps its rule");
            // The siblings and what they hold are untouched.
            let bl = t.by_name("B-L").unwrap();
            assert_eq!(eff[bl].raw("fontsize"), Some("14"));
            report.push(format!("{}x{} z={:.4} ({checked} values)", avail.0, avail.1, fonts["A-TL"] / 14.0));
        }
        println!("zoom, differential against cobolt_forms::layout::solve: {}", report.join("; "));
    }

    /// `CARDS` with a card of its own inside `A`, expanded as well: the Panel `A-NEST` holds a label and a button.
    fn nested() -> String {
        format!(
            r##"{CARDS}
      <Control id="A-NEST" type="GroupBox" parent="A" x="40" y="100" w="120" h="60"><Property name="Expandable">1</Property><Property name="Expanded">1</Property></Control>
      <Control id="A-NEST-L" type="Label" parent="A-NEST" x="50" y="110" w="60" h="16"><Property name="FontSize">8</Property></Control>
      <Control id="A-NEST-B" type="Button" parent="A-NEST" x="100" y="140" w="50" h="14"><Property name="FontSize">7</Property><Property name="Anchor">Bottom,Right</Property></Control>"##
        )
    }

    #[test]
    fn a_card_expanded_inside_an_expanded_card_takes_its_containers_client_and_its_contents_follow_the_outer_factor() {
        let xml = nested();
        for avail in [(640.0_f32, 480.0_f32), (1000.0, 700.0), (900.0, 300.0)] {
            let (rects, fonts) = desktop(&xml, "A", avail);
            let mut t = tree_of(&xml);
            t.set(0, "Responsive", "1");
            let (a, nest) = (t.by_name("A").unwrap(), t.by_name("A-NEST").unwrap());
            t.set(a, "Expanded", "1");
            let laid = rects["A"];
            t.set_card_box(a, Some((laid.w as f64, laid.h as f64)));
            // Only the outer card is the origin: a measured box for the inner one is not read.
            t.set_card_box(nest, Some((rects["A-NEST"].w as f64 * 0.5, rects["A-NEST"].h as f64 * 0.5)));
            let eff = t.effective_ctls(None);
            let (dx, dy) = (laid.x - 10.0, laid.y - 10.0);
            for id in ["A-NEST", "A-NEST-L", "A-NEST-B", "A-TL", "A-IN-L"] {
                let k = t.by_name(id).unwrap();
                let got = [written(&eff[k], "x") as f32 + dx, written(&eff[k], "y") as f32 + dy, written(&eff[k], "width") as f32, written(&eff[k], "height") as f32];
                let want = rects[id];
                for (g, w, what) in [(got[0], want.x, "x"), (got[1], want.y, "y"), (got[2], want.w, "width"), (got[3], want.h, "height")] {
                    assert!((g - w).abs() < 0.01, "{id} {what} at {avail:?}: page {g}, desktop {w}");
                }
            }
            for id in ["A-NEST-L", "A-NEST-B", "A-TL"] {
                let k = t.by_name(id).unwrap();
                assert!((eff[k].num("fontsize").unwrap() as f32 - fonts[id]).abs() < 0.001, "{id} lettering at {avail:?}");
            }
            assert_eq!(origin_of(&t.ctls, &t.card_boxes, t.by_name("A-NEST-L").unwrap()).map(|(card, _)| card), Some(a), "the outer card decides");
        }
    }

    #[test]
    fn a_card_that_is_not_expanded_or_not_measured_leaves_everything_as_designed() {
        let mut t = tree_of(CARDS);
        t.set(0, "Responsive", "1");
        let a = t.by_name("A").unwrap();
        let tl = t.by_name("A-TL").unwrap();
        // Not expanded: a stale box means nothing.
        t.set_card_box(a, Some((620.0, 460.0)));
        assert_eq!(t.effective_ctls(None)[tl].raw("x"), Some("20"));
        assert_eq!(t.font_size_of(tl), Some(14.0));
        // Expanded but not yet measured: the browser has not said how big it is.
        t.set_card_box(a, None);
        t.set(a, "Expanded", "1");
        assert_eq!(t.effective_ctls(None)[tl].raw("x"), Some("20"));
        assert_eq!(t.font_size_of(tl), Some(14.0));
        // Measured at its designed size: the factor is one and nothing moves.
        t.set_card_box(a, Some((300.0, 220.0)));
        let eff = t.effective_ctls(None);
        assert_eq!((written(&eff[tl], "x"), written(&eff[tl], "width"), written(&eff[tl], "fontsize")), (20.0, 100.0, 14.0));
    }

    #[test]
    fn a_form_that_is_not_laid_out_expands_nothing_and_so_zooms_nothing() {
        let mut t = tree_of(CARDS);
        let (a, tl) = (t.by_name("A").unwrap(), t.by_name("A-TL").unwrap());
        t.set(a, "Expanded", "1");
        t.set_card_box(a, Some((620.0, 460.0)));
        t.set(0, "Responsive", "0");
        assert_eq!(t.effective_ctls(None)[tl].raw("x"), Some("20"), "a card's expansion is a layout: without one it changes nothing");
        assert_eq!(t.font_size_of(tl), Some(14.0));
        t.set(0, "Responsive", "1");
        assert_ne!(t.font_size_of(tl), Some(14.0), "and with one it does");
    }

    #[test]
    fn a_box_within_a_quarter_pixel_is_no_change_and_a_collapsed_card_forgets_its_box() {
        let mut t = tree_of(CARDS);
        t.set(0, "Responsive", "1");
        let a = t.by_name("A").unwrap();
        assert!(t.set_card_box(a, Some((620.0, 460.0))));
        assert!(!t.set_card_box(a, Some((620.2, 459.9))), "a layout that settles must not set the page going again");
        assert!(t.set_card_box(a, Some((640.0, 460.0))));
        assert!(t.set_card_box(a, None));
        assert!(!t.set_card_box(a, None), "nothing left to forget");
    }

    #[test]
    fn the_outermost_expanded_card_decides_and_an_unmeasured_one_gives_no_factor_to_what_is_inside_it() {
        let mut t = tree_of(
            r##"<Control id="OUT" type="Panel" x="0" y="0" w="400" h="300"><Property name="Expanded">1</Property></Control>
                <Control id="IN" type="GroupBox" parent="OUT" x="10" y="10" w="200" h="100"><Property name="Expanded">1</Property></Control>
                <Control id="L" type="Label" parent="IN" x="20" y="20" w="50" h="20"><Property name="FontSize">10</Property></Control>"##,
        );
        t.set(0, "Responsive", "1");
        let (out, inn, l) = (t.by_name("OUT").unwrap(), t.by_name("IN").unwrap(), t.by_name("L").unwrap());
        t.set_card_box(inn, Some((400.0, 200.0)));
        assert_eq!(t.font_size_of(l), Some(10.0), "the outer card has not been measured: no factor");
        t.set_card_box(out, Some((800.0, 600.0)));
        // OUT: designed client 396 x 296, laid 796 x 596: min(2.01, 2.01) ... the outer card's factor reaches the inner one.
        let z = (796.0_f32 / 396.0).min(596.0 / 296.0);
        assert!((t.font_size_of(l).unwrap() as f32 - 10.0 * z).abs() < 0.001, "{:?}", t.font_size_of(l));
        assert_eq!(origin_of(&t.ctls, &t.card_boxes, l).map(|(card, _)| card), Some(out));
    }
}
