// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! **Layout by CSS** (WASM-004 R8, decision D8): the four layout modes of spec 056 as the browser's
//! own layout. Flex, Grid and Flow map to `display: flex` / `grid`, Absolute to absolute positioning
//! with anchors and docks, and the breakpoints are chosen from the form element's own width
//! (`layout::breakpoints::select`, the desktop's function, run by the renderer on a `ResizeObserver`).
//!
//! The desktop's pure solver is not run in the browser; the difference a pixel of rounding or the
//! browser's text metrics make is what the parity check of WASM-004 R11 measures. Where CSS cannot say
//! what the solver says — sequential docking consuming space, merged grid areas — the entry is
//! *approximate* and listed in [`APPROXIMATIONS`].

use crate::css::Decls;
use crate::tree::Ctl;
use crate::value::{fmt_number, parse_f64};
use cobolt_forms::model::ControlType;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Absolute,
    Flex,
    Grid,
    Flow,
}

/// What this module approximates rather than reproduces.
pub const APPROXIMATIONS: &[&str] = &[
    "Dock: each docked control sits against its edge of the container; the desktop solver also shrinks the space for the docks that follow, which CSS positioning does not",
    "GridMerges: a merged area is not expressed; its cells lay out as separate cells",
    "FlowBreak: a forced line break in a Flow container is an empty full-width item the renderer inserts",
    "ObsoleteScalingStyle: a legacy form's font ratio is not read, as the page does not do the resizing and moving that style also asks for (FontScaling and MinFontSize/MaxFontSize are: `tree::scaled_font`)",
];

pub fn mode_of(c: &Ctl) -> Mode {
    match c.raw("layoutmode").map(|s| s.trim().to_ascii_lowercase()).as_deref() {
        Some("flex") => Mode::Flex,
        Some("grid") => Mode::Grid,
        Some("flow") => Mode::Flow,
        _ => Mode::Absolute,
    }
}

fn px(v: f64) -> String {
    format!("{}px", fmt_number(v))
}

fn push(d: &mut Decls, k: &str, v: impl Into<String>) {
    d.push((k.to_owned(), v.into()));
}

fn num(c: &Ctl, k: &str) -> Option<f64> {
    c.raw(k).and_then(parse_f64)
}

/// One grid track list (`GridColumns` / `GridRows`) as CSS. Tokens, any case, separated by spaces:
/// `Npx` (a bare number is pixels), `N%`, `Nfr`, `Auto`, `MinMax(a, b)`, `Repeat(N, tracks…)`,
/// `Repeat(AutoFill, tracks…)`. A token it does not understand is skipped, as the desktop's parser skips it.
pub fn grid_tracks(text: &str) -> Option<String> {
    fn tokens(s: &str) -> Vec<String> {
        let mut out = Vec::new();
        let mut cur = String::new();
        let mut depth = 0;
        for ch in s.chars() {
            match ch {
                '(' => {
                    depth += 1;
                    cur.push(ch);
                }
                ')' => {
                    depth -= 1;
                    cur.push(ch);
                }
                c if c.is_whitespace() && depth == 0 => {
                    if !cur.is_empty() {
                        out.push(std::mem::take(&mut cur));
                    }
                }
                _ => cur.push(ch),
            }
        }
        if !cur.is_empty() {
            out.push(cur);
        }
        out
    }
    fn args(inner: &str) -> Vec<String> {
        let mut out = Vec::new();
        let mut cur = String::new();
        let mut depth = 0;
        for ch in inner.chars() {
            match ch {
                '(' => {
                    depth += 1;
                    cur.push(ch);
                }
                ')' => {
                    depth -= 1;
                    cur.push(ch);
                }
                ',' if depth == 0 => out.push(std::mem::take(&mut cur)),
                _ => cur.push(ch),
            }
        }
        out.push(cur);
        out.into_iter().map(|s| s.trim().to_owned()).filter(|s| !s.is_empty()).collect()
    }
    fn one(tok: &str) -> Option<String> {
        let t = tok.trim();
        let lower = t.to_ascii_lowercase();
        if lower == "auto" {
            return Some("auto".into());
        }
        if let Some(n) = lower.strip_suffix("fr") {
            return parse_f64(n).map(|v| format!("{}fr", fmt_number(v)));
        }
        if let Some(n) = lower.strip_suffix('%') {
            return parse_f64(n).map(|v| format!("{}%", fmt_number(v)));
        }
        if let Some(n) = lower.strip_suffix("px") {
            return parse_f64(n).map(|v| px(v));
        }
        if let Some(inner) = lower.strip_prefix("minmax(").and_then(|r| r.strip_suffix(')')) {
            let a = args(inner);
            if a.len() == 2 {
                return Some(format!("minmax({}, {})", one(&a[0])?, one(&a[1])?));
            }
            return None;
        }
        if let Some(inner) = lower.strip_prefix("repeat(").and_then(|r| r.strip_suffix(')')) {
            let a = args(inner);
            if a.len() >= 2 {
                let count = if a[0].eq_ignore_ascii_case("autofill") || a[0].eq_ignore_ascii_case("auto-fill") {
                    "auto-fill".to_owned()
                } else {
                    parse_f64(&a[0]).map(|v| fmt_number(v.max(1.0)))?
                };
                let inner_tracks: Vec<String> = a[1..].iter().filter_map(|x| one(x)).collect();
                if inner_tracks.is_empty() {
                    return None;
                }
                return Some(format!("repeat({count}, {})", inner_tracks.join(" ")));
            }
            return None;
        }
        // A bare number is pixels.
        parse_f64(&lower).map(px)
    }
    let out: Vec<String> = tokens(text).iter().filter_map(|t| one(t)).collect();
    if out.is_empty() {
        None
    } else {
        Some(out.join(" "))
    }
}

/// `JustifyContent`, in the words the solver reads (`flex::flex_container`): anything else is `Start`.
fn justify(v: &str) -> &'static str {
    match v.trim().to_ascii_lowercase().as_str() {
        "center" => "center",
        "end" => "flex-end",
        "spacebetween" => "space-between",
        "spacearound" => "space-around",
        "spaceevenly" => "space-evenly",
        _ => "flex-start",
    }
}

/// `AlignItems` / `AlignSelf` in the words the solver reads (`flex::align_of`) as a CSS keyword;
/// `None` for anything else, which the solver treats as not set.
fn align_keyword(v: &str) -> Option<&'static str> {
    match v.trim().to_ascii_lowercase().as_str() {
        "stretch" => Some("stretch"),
        "start" => Some("flex-start"),
        "center" => Some("center"),
        "end" => Some("flex-end"),
        _ => None,
    }
}

/// The container's `AlignItems` as the solver resolves it: a word it does not know is `Stretch`.
fn align(v: &str) -> &'static str {
    align_keyword(v).unwrap_or("stretch")
}

/// What the desktop takes off a container's outer box before its own padding to get the client
/// area its children are laid out in: `Control::content_rect` insets a Panel and a GroupBox by 2
/// pixels on every side, whatever border they draw.
fn chrome_inset(c: &Ctl) -> f64 {
    match c.control_type() {
        Some(ControlType::Panel | ControlType::GroupBox) => 2.0,
        _ => 0.0,
    }
}

/// Does this Flex or Flow container read its children right to left? (The solver's reading order
/// then runs the other way along a row — `layout::reading_order`.)
pub fn reads_right_to_left(c: &Ctl) -> bool {
    let word = |k: &str| c.raw(k).map(|s| s.trim().to_ascii_lowercase()).unwrap_or_default();
    match mode_of(c) {
        Mode::Flex => word("flexdirection") == "rowreverse",
        Mode::Flow => word("flowdirection") == "righttoleft",
        _ => false,
    }
}

/// Is the main axis of a Flex or Flow container horizontal?
fn main_is_horizontal(c: &Ctl) -> bool {
    let word = |k: &str| c.raw(k).map(|s| s.trim().to_ascii_lowercase()).unwrap_or_default();
    match mode_of(c) {
        Mode::Flex => !matches!(word("flexdirection").as_str(), "column" | "columnreverse"),
        Mode::Flow => !matches!(word("flowdirection").as_str(), "topdown" | "bottomup"),
        _ => true,
    }
}

/// The declarations a container (the form or a Panel/GroupBox/TabControl page) needs for its own layout.
pub fn container(c: &Ctl) -> Decls {
    let mut d = Decls::new();
    let gap = num(c, "gap").unwrap_or(0.0);
    let row_gap = num(c, "rowgap").unwrap_or(gap);
    let col_gap = num(c, "columngap").unwrap_or(gap);
    let word = |k: &str| c.raw(k).map(|s| s.trim().to_ascii_lowercase()).unwrap_or_default();
    match mode_of(c) {
        Mode::Absolute => {
            push(&mut d, "position", "relative");
        }
        Mode::Flex => {
            push(&mut d, "display", "flex");
            let dir = match word("flexdirection").as_str() {
                "column" => "column",
                "rowreverse" => "row-reverse",
                "columnreverse" => "column-reverse",
                _ => "row",
            };
            push(&mut d, "flex-direction", dir);
            push(&mut d, "flex-wrap", match word("flexwrap").as_str() {
                "wrap" => "wrap",
                "wrapreverse" => "wrap-reverse",
                _ => "nowrap",
            });
            push(&mut d, "justify-content", justify(c.text_or("justifycontent", "Start")));
            push(&mut d, "align-items", align(c.text_or("alignitems", "Stretch")));
            push(&mut d, "align-content", match word("aligncontent").as_str() {
                "spacebetween" => "space-between",
                "spacearound" => "space-around",
                "center" => "center",
                "end" => "flex-end",
                "start" => "flex-start",
                _ => "stretch",
            });
            push(&mut d, "gap", format!("{} {}", px(row_gap), px(col_gap)));
        }
        Mode::Grid => {
            push(&mut d, "display", "grid");
            if let Some(cols) = c.raw("gridcolumns").and_then(grid_tracks) {
                push(&mut d, "grid-template-columns", cols);
            }
            if let Some(rows) = c.raw("gridrows").and_then(grid_tracks) {
                push(&mut d, "grid-template-rows", rows);
            }
            push(&mut d, "gap", format!("{} {}", px(row_gap), px(col_gap)));
            push(&mut d, "justify-items", match c.text_or("justifyitems", "Stretch").to_ascii_lowercase().as_str() {
                "start" => "start",
                "end" => "end",
                "center" => "center",
                _ => "stretch",
            });
            push(&mut d, "align-items", align(c.text_or("alignitems", "Stretch")));
        }
        Mode::Flow => {
            push(&mut d, "display", "flex");
            let dir = match word("flowdirection").as_str() {
                "righttoleft" => "row-reverse",
                "topdown" => "column",
                "bottomup" => "column-reverse",
                _ => "row",
            };
            push(&mut d, "flex-direction", dir);
            let wrap_contents = c.raw("wrapcontents").map(|v| crate::value::turned_on(v)).unwrap_or(true);
            push(&mut d, "flex-wrap", if wrap_contents { "wrap" } else { "nowrap" });
            // The solver's Flow container: items at the start of their line, lines at the start.
            push(&mut d, "align-items", "flex-start");
            push(&mut d, "align-content", "flex-start");
            push(&mut d, "gap", format!("{} {}", px(row_gap), px(col_gap)));
        }
    }
    // The client area the children are laid out in is the outer box less what the desktop takes off
    // a Panel or GroupBox (`chrome_inset`) less the layout padding. The element draws its own border
    // as CSS does, inside the box, so the padding gives back what the border already takes.
    let all = num(c, "padding").unwrap_or(0.0);
    let side = |k: &str| num(c, k).unwrap_or(all);
    let take = |k: &str| (side(k) + chrome_inset(c) - crate::css::border_width(c)).max(0.0);
    let (top, right, bottom, left) = (take("paddingtop"), take("paddingright"), take("paddingbottom"), take("paddingleft"));
    if top + right + bottom + left > 0.0 {
        push(&mut d, "padding", format!("{} {} {} {}", px(top), px(right), px(bottom), px(left)));
        push(&mut d, "box-sizing", "border-box");
    }
    d
}

/// The control among the children of container `parent` that is expanded (spec 090), if any: a Panel or GroupBox that is shown and has
/// `Expanded` on; where several are, the one highest in z-order wins (R7).
pub fn expanded_sibling(ctls: &[Ctl], parent: usize) -> Option<usize> {
    ctls[parent]
        .children
        .iter()
        .copied()
        .filter(|&k| {
            let c = &ctls[k];
            matches!(c.control_type(), Some(ControlType::Panel | ControlType::GroupBox)) && c.flag("expanded") && c.visible()
        })
        .max_by_key(|&k| (num(&ctls[k], "zorder").unwrap_or(0.0) as i64, k))
}

/// What puts an expanded card over the whole client area of its container: out of the flow, edge to edge of the area the
/// container's own padding leaves, above everything else in it (R4).
pub fn expanded_placement(parent: &Ctl) -> Decls {
    let all = num(parent, "padding").unwrap_or(0.0);
    let side = |k: &str| num(parent, k).unwrap_or(all);
    let take = |k: &str| (side(k) + chrome_inset(parent) - crate::css::border_width(parent)).max(0.0);
    let mut d = Decls::new();
    push(&mut d, "position", "absolute");
    push(&mut d, "inset", format!("{} {} {} {}", px(take("paddingtop")), px(take("paddingright")), px(take("paddingbottom")), px(take("paddingleft"))));
    push(&mut d, "width", "auto");
    push(&mut d, "height", "auto");
    push(&mut d, "min-width", "0");
    push(&mut d, "min-height", "0");
    push(&mut d, "max-width", "none");
    push(&mut d, "max-height", "none");
    push(&mut d, "margin", "0");
    push(&mut d, "z-index", "900");
    d
}

/// What a container's box takes of its height before its content starts, as [`container`] and the border write it: the two
/// borders and the top and bottom padding. A box that must be `H` tall holds `H` less this of content.
pub fn vertical_extent(c: &Ctl) -> f64 {
    let all = num(c, "padding").unwrap_or(0.0);
    let border = crate::css::border_width(c);
    let take = |k: &str| (num(c, k).unwrap_or(all) + chrome_inset(c) - border).max(0.0);
    take("paddingtop") + take("paddingbottom") + 2.0 * border
}

/// The height of the empty, in-flow first child a card needs to carry its floor, if it needs one.
///
/// A Panel or GroupBox in Absolute mode places its children against its own edges. When it is also an item of a Flex or Flow
/// parent, its height can come from nothing but a `min-height` floor — the desktop's rule for a wrapping, stretched line is that
/// the designed height is the floor — and Chrome then measures a child anchored to its bottom edge (or to top and bottom) against
/// the box's *content* height, not the floor: a chart anchored top and bottom came out 0 high inside a card 260 high, and a label
/// anchored to the bottom edge sat at the top. Carrying the floor as content too — an empty block that high — makes the box that
/// tall in every pass whatever the parent's stretch does, which is also what the specification says the children are measured
/// against. `d` is the card's own declarations; `parent` is the container it is an item of.
pub fn floor_spacer(parent: &Ctl, c: &Ctl, d: &Decls) -> Option<f64> {
    if !matches!(c.control_type(), Some(ControlType::Panel | ControlType::GroupBox)) || mode_of(c) != Mode::Absolute {
        return None;
    }
    if !matches!(mode_of(parent), Mode::Flex | Mode::Flow) {
        return None;
    }
    let floor = d.iter().rev().find(|(k, _)| k == "min-height").and_then(|(_, v)| v.strip_suffix("px")).and_then(parse_f64)?;
    let content = floor - vertical_extent(c);
    (content > 0.0).then_some(content)
}

/// Does `c` follow the parent's edge on this axis? `Anchor` is `Top,Left` by default.
fn anchors(c: &Ctl) -> (bool, bool, bool, bool) {
    // An empty string is the empty set (proportional); only an absent property takes the default.
    let a = cobolt_forms::layout::props::Edges::parse(c.raw("anchor").unwrap_or("Top,Left"));
    (a.left, a.right, a.top, a.bottom)
}

/// The declarations that place `child` in `parent`, given the parent's DESIGNED size (the anchors are
/// offsets from the designed edges).
pub fn item(parent: &Ctl, child: &Ctl, parent_size: (f64, f64), origin: (f64, f64), responsive: bool) -> Decls {
    let mut d = Decls::new();
    let (x, y) = (num(child, "x").unwrap_or(0.0) - origin.0, num(child, "y").unwrap_or(0.0) - origin.1);
    let (w, h) = (num(child, "width").unwrap_or(100.0), num(child, "height").unwrap_or(24.0));
    // Size limits apply in every mode.
    for (k, css) in [("minwidth", "min-width"), ("minheight", "min-height"), ("maxwidth", "max-width"), ("maxheight", "max-height")] {
        if let Some(v) = num(child, k).filter(|v| *v > 0.0) {
            push(&mut d, css, px(v));
        }
    }
    match mode_of(parent) {
        Mode::Absolute => {
            push(&mut d, "position", "absolute");
            let dock = child.text_or("dock", "None").trim().to_ascii_lowercase();
            if responsive && dock != "none" {
                match dock.as_str() {
                    "fill" => {
                        push(&mut d, "inset", "0");
                    }
                    "left" => {
                        push(&mut d, "left", "0");
                        push(&mut d, "top", "0");
                        push(&mut d, "bottom", "0");
                        push(&mut d, "width", px(w));
                    }
                    "right" => {
                        push(&mut d, "right", "0");
                        push(&mut d, "top", "0");
                        push(&mut d, "bottom", "0");
                        push(&mut d, "width", px(w));
                    }
                    "top" => {
                        push(&mut d, "top", "0");
                        push(&mut d, "left", "0");
                        push(&mut d, "right", "0");
                        push(&mut d, "height", px(h));
                    }
                    _ => {
                        push(&mut d, "bottom", "0");
                        push(&mut d, "left", "0");
                        push(&mut d, "right", "0");
                        push(&mut d, "height", px(h));
                    }
                }
                return d;
            }
            if !responsive {
                // Not responsive: the designed rectangle, as the desktop draws it at the designed size.
                push(&mut d, "left", px(x));
                push(&mut d, "top", px(y));
                push(&mut d, "width", px(w));
                push(&mut d, "height", px(h));
                return d;
            }
            let (l, r, t, b) = anchors(child);
            let (pw, ph) = parent_size;
            // Horizontal.
            match (l, r) {
                (true, true) => {
                    push(&mut d, "left", px(x));
                    push(&mut d, "right", px(pw - x - w));
                }
                (false, true) => {
                    push(&mut d, "right", px(pw - x - w));
                    push(&mut d, "width", px(w));
                }
                (true, false) => {
                    push(&mut d, "left", px(x));
                    push(&mut d, "width", px(w));
                }
                (false, false) => {
                    // Proportional: the centre stays at the same fraction of the parent.
                    let centre = if pw > 0.0 { (x + w / 2.0) / pw * 100.0 } else { 50.0 };
                    push(&mut d, "left", format!("calc({}% - {})", fmt_number(centre), px(w / 2.0)));
                    push(&mut d, "width", px(w));
                }
            }
            // Vertical.
            match (t, b) {
                (true, true) => {
                    push(&mut d, "top", px(y));
                    push(&mut d, "bottom", px(ph - y - h));
                }
                (false, true) => {
                    push(&mut d, "bottom", px(ph - y - h));
                    push(&mut d, "height", px(h));
                }
                (true, false) => {
                    push(&mut d, "top", px(y));
                    push(&mut d, "height", px(h));
                }
                (false, false) => {
                    let centre = if ph > 0.0 { (y + h / 2.0) / ph * 100.0 } else { 50.0 };
                    push(&mut d, "top", format!("calc({}% - {})", fmt_number(centre), px(h / 2.0)));
                    push(&mut d, "height", px(h));
                }
            }
        }
        Mode::Flex | Mode::Flow => {
            push(&mut d, "position", "relative");
            let flex_parent = mode_of(parent) == Mode::Flex;
            let grow = num(child, "flexgrow").unwrap_or(0.0);
            let shrink = num(child, "flexshrink").unwrap_or(1.0);
            let basis_text = child.text_or("flexbasis", "Auto").trim().to_ascii_lowercase();
            let basis = if basis_text == "auto" { "auto".to_owned() } else if let Some(n) = basis_text.strip_suffix('%') { format!("{}%", n.trim()) } else { parse_f64(basis_text.trim_end_matches("px")).map(px).unwrap_or_else(|| "auto".into()) };
            let self_align = if flex_parent { align_keyword(child.text_or("alignself", "Auto")) } else { None };
            if flex_parent {
                push(&mut d, "flex", format!("{} {} {}", fmt_number(grow), fmt_number(shrink), basis));
                if let Some(a) = self_align {
                    push(&mut d, "align-self", a);
                }
                if let Some(o) = num(child, "order") {
                    if o != 0.0 {
                        push(&mut d, "order", fmt_number(o));
                    }
                }
            } else {
                push(&mut d, "flex", "0 0 auto");
            }
            // The solver lets an item shrink to its limits, not to what its content needs.
            for (k, css) in [("minwidth", "min-width"), ("minheight", "min-height")] {
                if !num(child, k).is_some_and(|v| v > 0.0) {
                    push(&mut d, css, "0");
                }
            }
            // The designed size is the item's size on the main axis unless the basis names one, and
            // on the cross axis unless the item is stretched to its line (a Flow item never is).
            let horizontal = main_is_horizontal(parent);
            let ((main_prop, main_size), (cross_prop, cross_size)) = if horizontal { (("width", w), ("height", h)) } else { (("height", h), ("width", w)) };
            if basis == "auto" {
                push(&mut d, main_prop, px(main_size));
            }
            let stretched = flex_parent && self_align.unwrap_or_else(|| align(parent.text_or("alignitems", "Stretch"))) == "stretch";
            if !stretched {
                push(&mut d, cross_prop, px(cross_size));
            } else if parent.raw("flexwrap").is_some_and(|v| v.trim().to_ascii_lowercase().starts_with("wrap")) {
                // A line of a wrapping container is as tall as its tallest item's DESIGNED size, and the
                // stretch then only grows the others to it: the designed size is the floor, not the size.
                let floor = if horizontal { "min-height" } else { "min-width" };
                if !num(child, if horizontal { "minheight" } else { "minwidth" }).is_some_and(|v| v > 0.0) {
                    d.retain(|(k, _)| k != floor);
                    push(&mut d, floor, px(cross_size));
                }
            }
        }
        Mode::Grid => {
            push(&mut d, "position", "relative");
            let col = num(child, "gridcolumn").unwrap_or(0.0) as i64;
            let row = num(child, "gridrow").unwrap_or(0.0) as i64;
            let cspan = num(child, "columnspan").unwrap_or(1.0).max(1.0) as i64;
            let rspan = num(child, "rowspan").unwrap_or(1.0).max(1.0) as i64;
            // 0 is auto-placed (R56).
            push(&mut d, "grid-column", if col > 0 { format!("{col} / span {cspan}") } else { format!("span {cspan}") });
            push(&mut d, "grid-row", if row > 0 { format!("{row} / span {rspan}") } else { format!("span {rspan}") });
            let js = child.text_or("justifyself", "Auto");
            if !js.eq_ignore_ascii_case("auto") {
                push(&mut d, "justify-self", match js.to_ascii_lowercase().as_str() {
                    "start" => "start",
                    "end" => "end",
                    "center" => "center",
                    _ => "stretch",
                });
            }
            if let Some(a) = align_keyword(child.text_or("alignself", "Auto")) {
                push(&mut d, "align-self", a);
            }
            // A grid cell stretches by default; a control keeps its designed size only when the track is `auto`.
            push(&mut d, "min-width", "0");
        }
    }
    d
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tree::testing::tree;

    fn get<'a>(d: &'a Decls, k: &str) -> Option<&'a str> {
        d.iter().find(|(x, _)| x == k).map(|(_, v)| v.as_str())
    }

    #[test]
    fn grid_tracks_translate_to_css_and_skip_what_they_do_not_understand() {
        assert_eq!(grid_tracks("1fr 2fr 100px").unwrap(), "1fr 2fr 100px");
        assert_eq!(grid_tracks("50 25% Auto").unwrap(), "50px 25% auto");
        assert_eq!(grid_tracks("MinMax(100px, 1fr)").unwrap(), "minmax(100px, 1fr)");
        assert_eq!(grid_tracks("Repeat(3, 1fr)").unwrap(), "repeat(3, 1fr)");
        assert_eq!(grid_tracks("Repeat(AutoFill, MinMax(120, 1fr))").unwrap(), "repeat(auto-fill, minmax(120px, 1fr))");
        assert_eq!(grid_tracks("1fr banana 2fr").unwrap(), "1fr 2fr", "a half-typed value must not blank the form");
        assert_eq!(grid_tracks("banana"), None);
        assert_eq!(grid_tracks(""), None);
    }

    #[test]
    fn a_flex_container_and_its_items() {
        let mut t = tree();
        let p = t.by_name("PNL-1").unwrap();
        for (k, v) in [("LayoutMode", "Flex"), ("FlexDirection", "Column"), ("FlexWrap", "Wrap"), ("JustifyContent", "SpaceBetween"), ("AlignItems", "Center"), ("Gap", "8"), ("Padding", "4")] {
            t.set(p, k, v);
        }
        let c = container(t.ctl(p));
        assert_eq!(get(&c, "display"), Some("flex"));
        assert_eq!(get(&c, "flex-direction"), Some("column"));
        assert_eq!(get(&c, "flex-wrap"), Some("wrap"));
        assert_eq!(get(&c, "justify-content"), Some("space-between"));
        assert_eq!(get(&c, "align-items"), Some("center"));
        assert_eq!(get(&c, "gap"), Some("8px 8px"));
        assert_eq!(get(&c, "padding"), Some("5px 5px 5px 5px"), "4 of padding, plus the 2 px the desktop takes off a Panel, less the 1 px border the element draws itself");
        let child = t.by_name("LBL-IN").unwrap();
        t.set(child, "FlexGrow", "1");
        t.set(child, "AlignSelf", "End");
        t.set(child, "Order", "2");
        let i = item(t.ctl(p), t.ctl(child), (200.0, 200.0), (0.0, 0.0), true);
        assert_eq!(get(&i, "flex"), Some("1 1 auto"));
        assert_eq!(get(&i, "align-self"), Some("flex-end"));
        assert_eq!(get(&i, "order"), Some("2"));
    }

    #[test]
    fn a_grid_container_places_items_by_cell_and_span_and_auto_places_zero() {
        let mut t = tree();
        let p = t.by_name("PNL-1").unwrap();
        for (k, v) in [("LayoutMode", "Grid"), ("GridColumns", "1fr 1fr"), ("GridRows", "Auto 50px"), ("ColumnGap", "6"), ("RowGap", "2")] {
            t.set(p, k, v);
        }
        let c = container(t.ctl(p));
        assert_eq!(get(&c, "grid-template-columns"), Some("1fr 1fr"));
        assert_eq!(get(&c, "grid-template-rows"), Some("auto 50px"));
        assert_eq!(get(&c, "gap"), Some("2px 6px"));
        let child = t.by_name("LBL-IN").unwrap();
        t.set(child, "GridColumn", "2");
        t.set(child, "ColumnSpan", "2");
        let i = item(t.ctl(p), t.ctl(child), (200.0, 200.0), (0.0, 0.0), true);
        assert_eq!(get(&i, "grid-column"), Some("2 / span 2"));
        assert_eq!(get(&i, "grid-row"), Some("span 1"), "row 0 is auto-placed");
    }

    #[test]
    fn anchors_follow_the_parents_edges() {
        let mut t = tree();
        let form = 0;
        let b = t.by_name("BTN-GO").unwrap();
        // BTN-GO: x=10 y=10 w=80 h=30 in a 640x480 form.
        let designed = item(t.ctl(form), t.ctl(b), (640.0, 480.0), (0.0, 0.0), false);
        assert_eq!((get(&designed, "left"), get(&designed, "top"), get(&designed, "width"), get(&designed, "height")), (Some("10px"), Some("10px"), Some("80px"), Some("30px")));
        t.set(b, "Anchor", "Top,Left,Right");
        let i = item(t.ctl(form), t.ctl(b), (640.0, 480.0), (0.0, 0.0), true);
        assert_eq!(get(&i, "left"), Some("10px"));
        assert_eq!(get(&i, "right"), Some("550px"), "640 - 10 - 80");
        assert_eq!(get(&i, "width"), None, "the width follows the parent");
        t.set(b, "Anchor", "Bottom,Right");
        let i = item(t.ctl(form), t.ctl(b), (640.0, 480.0), (0.0, 0.0), true);
        assert_eq!((get(&i, "right"), get(&i, "bottom"), get(&i, "width"), get(&i, "height")), (Some("550px"), Some("440px"), Some("80px"), Some("30px")));
        t.set(b, "Anchor", "");
        let i = item(t.ctl(form), t.ctl(b), (640.0, 480.0), (0.0, 0.0), true);
        assert_eq!(get(&i, "left"), Some("calc(7.8125% - 40px)"), "proportional: the centre stays at 50/640 of the width");
        t.set(b, "Dock", "Fill");
        assert_eq!(get(&item(t.ctl(form), t.ctl(b), (640.0, 480.0), (0.0, 0.0), true), "inset"), Some("0"));
    }

    #[test]
    fn size_limits_are_css_min_and_max() {
        let mut t = tree();
        let b = t.by_name("BTN-GO").unwrap();
        t.set(b, "MinWidth", "60");
        t.set(b, "MaxHeight", "50");
        let i = item(t.ctl(0), t.ctl(b), (640.0, 480.0), (0.0, 0.0), false);
        assert_eq!((get(&i, "min-width"), get(&i, "max-height")), (Some("60px"), Some("50px")));
    }

    #[test]
    fn a_boxes_vertical_extent_is_its_borders_and_the_padding_the_container_rule_writes() {
        let mut t = tree();
        let p = t.by_name("PNL-1").unwrap();
        assert_eq!(vertical_extent(t.ctl(p)), 4.0, "no border, no padding: the 2 px the desktop takes off a Panel on each side");
        for (k, v) in [("BorderStyle", "Single"), ("BorderWidth", "3"), ("Padding", "5")] {
            t.set(p, k, v);
        }
        // 3 px of border on each side, and 5 + 2 - 3 = 4 px of padding on each side.
        assert_eq!(vertical_extent(t.ctl(p)), 14.0);
    }

    #[test]
    fn a_card_whose_height_is_only_a_floor_carries_it_as_content_too() {
        let mut t = tree();
        t.set(0, "LayoutMode", "Flex");
        t.set(0, "FlexWrap", "Wrap");
        let p = t.by_name("PNL-1").unwrap();
        let mut d = Decls::new();
        push(&mut d, "min-height", "260px");
        assert_eq!(floor_spacer(t.ctl(0), t.ctl(p), &d), Some(256.0), "260 less the 2 + 2 px the box takes before its content");

        let label = t.by_name("LBL-IN").unwrap();
        assert_eq!(floor_spacer(t.ctl(0), t.ctl(label), &d), None, "a Label has no children to measure");
        t.set(0, "LayoutMode", "Absolute");
        assert_eq!(floor_spacer(t.ctl(0), t.ctl(p), &d), None, "in an Absolute parent the card has a designed rectangle, not a floor");
        t.set(0, "LayoutMode", "Flow");
        assert_eq!(floor_spacer(t.ctl(0), t.ctl(p), &d), Some(256.0), "a Flow parent too");
        t.set(p, "LayoutMode", "Flex");
        assert_eq!(floor_spacer(t.ctl(0), t.ctl(p), &d), None, "a card that lays its children out in flow has the content to size it");

        t.set(p, "LayoutMode", "Absolute");
        let mut none = Decls::new();
        push(&mut none, "min-height", "0");
        assert_eq!(floor_spacer(t.ctl(0), t.ctl(p), &none), None, "no floor, no spacer");
        assert_eq!(floor_spacer(t.ctl(0), t.ctl(p), &Decls::new()), None);
        let mut small = Decls::new();
        push(&mut small, "min-height", "3px");
        assert_eq!(floor_spacer(t.ctl(0), t.ctl(p), &small), None, "a floor the box's own padding already covers");
    }
}
