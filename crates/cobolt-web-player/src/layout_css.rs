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
    "MinFontSize/MaxFontSize and FontScaling: the browser's own text scaling applies",
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

fn justify(v: &str) -> &'static str {
    match v.trim().to_ascii_lowercase().as_str() {
        "end" | "flexend" | "flex-end" => "flex-end",
        "center" | "centre" => "center",
        "spacebetween" | "space-between" => "space-between",
        "spacearound" | "space-around" => "space-around",
        "spaceevenly" | "space-evenly" => "space-evenly",
        _ => "flex-start",
    }
}

fn align(v: &str) -> &'static str {
    match v.trim().to_ascii_lowercase().as_str() {
        "start" | "flexstart" | "flex-start" => "flex-start",
        "end" | "flexend" | "flex-end" => "flex-end",
        "center" | "centre" => "center",
        "baseline" => "baseline",
        _ => "stretch",
    }
}

/// The declarations a container (the form or a Panel/GroupBox/TabControl page) needs for its own layout.
pub fn container(c: &Ctl) -> Decls {
    let mut d = Decls::new();
    let gap = num(c, "gap").unwrap_or(0.0);
    let row_gap = num(c, "rowgap").unwrap_or(gap);
    let col_gap = num(c, "columngap").unwrap_or(gap);
    match mode_of(c) {
        Mode::Absolute => {
            push(&mut d, "position", "relative");
        }
        Mode::Flex => {
            push(&mut d, "display", "flex");
            let dir = match c.raw("flexdirection").map(|s| s.trim().to_ascii_lowercase()).as_deref() {
                Some("column") => "column",
                Some("rowreverse") | Some("row-reverse") => "row-reverse",
                Some("columnreverse") | Some("column-reverse") => "column-reverse",
                _ => "row",
            };
            push(&mut d, "flex-direction", dir);
            push(&mut d, "flex-wrap", if c.raw("flexwrap").map(|s| s.trim().eq_ignore_ascii_case("wrap")).unwrap_or(false) { "wrap" } else { "nowrap" });
            push(&mut d, "justify-content", justify(c.text_or("justifycontent", "Start")));
            push(&mut d, "align-items", align(c.text_or("alignitems", "Stretch")));
            push(&mut d, "align-content", match c.text_or("aligncontent", "Stretch").to_ascii_lowercase().as_str() {
                "spacebetween" | "space-between" => "space-between",
                "spacearound" | "space-around" => "space-around",
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
            let (dir, wrap) = match c.raw("flowdirection").map(|s| s.trim().to_ascii_lowercase()).as_deref() {
                Some("righttoleft") | Some("rtl") => ("row-reverse", true),
                Some("topdown") | Some("top-down") => ("column", true),
                Some("bottomup") | Some("bottom-up") => ("column-reverse", true),
                _ => ("row", true),
            };
            push(&mut d, "flex-direction", dir);
            let wrap_contents = c.raw("wrapcontents").map(|v| crate::value::turned_on(v)).unwrap_or(wrap);
            push(&mut d, "flex-wrap", if wrap_contents { "wrap" } else { "nowrap" });
            push(&mut d, "align-content", "flex-start");
            push(&mut d, "gap", format!("{} {}", px(row_gap), px(col_gap)));
        }
    }
    // Container padding (layout padding, in form pixels). The appearance table applies the same
    // property to controls that paint a face; a container uses it for its content box.
    let all = num(c, "padding").unwrap_or(0.0);
    let side = |k: &str| num(c, k).unwrap_or(all);
    if (side("paddingleft") + side("paddingtop") + side("paddingright") + side("paddingbottom")) > 0.0 {
        push(&mut d, "padding", format!("{} {} {} {}", px(side("paddingtop")), px(side("paddingright")), px(side("paddingbottom")), px(side("paddingleft"))));
        push(&mut d, "box-sizing", "border-box");
    }
    d
}

/// Does `c` follow the parent's edge on this axis? `Anchor` is `Top,Left` by default.
fn anchors(c: &Ctl) -> (bool, bool, bool, bool) {
    // An empty string is the empty set (proportional); only an absent property takes the default.
    let a = cobolt_forms::layout::props::Edges::parse(c.raw("anchor").unwrap_or("Top,Left"));
    (a.left, a.right, a.top, a.bottom)
}

/// The declarations that place `child` in `parent`, given the parent's DESIGNED size (the anchors are
/// offsets from the designed edges).
pub fn item(parent: &Ctl, child: &Ctl, parent_size: (f64, f64), responsive: bool) -> Decls {
    let mut d = Decls::new();
    let (x, y) = (num(child, "x").unwrap_or(0.0), num(child, "y").unwrap_or(0.0));
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
            let grow = num(child, "flexgrow").unwrap_or(0.0);
            let shrink = num(child, "flexshrink").unwrap_or(1.0);
            let basis_text = child.text_or("flexbasis", "Auto").trim().to_ascii_lowercase();
            let basis = if basis_text == "auto" { "auto".to_owned() } else if let Some(n) = basis_text.strip_suffix('%') { format!("{}%", n.trim()) } else { parse_f64(basis_text.trim_end_matches("px")).map(px).unwrap_or_else(|| "auto".into()) };
            if mode_of(parent) == Mode::Flex {
                push(&mut d, "flex", format!("{} {} {}", fmt_number(grow), fmt_number(shrink), basis));
                let a = child.text_or("alignself", "Auto");
                if !a.eq_ignore_ascii_case("auto") {
                    push(&mut d, "align-self", align(a));
                }
                if let Some(o) = num(child, "order") {
                    if o != 0.0 {
                        push(&mut d, "order", fmt_number(o));
                    }
                }
            } else {
                push(&mut d, "flex", "0 0 auto");
            }
            // The designed size is the item's preferred size unless the basis names one.
            if basis == "auto" {
                push(&mut d, "width", px(w));
                push(&mut d, "height", px(h));
            }
            if mode_of(parent) == Mode::Flex && grow > 0.0 {
                // A growing item takes the free space on the main axis; its cross size stays designed.
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
            let a = child.text_or("alignself", "Auto");
            if !a.eq_ignore_ascii_case("auto") {
                push(&mut d, "align-self", align(a));
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
        assert_eq!(get(&c, "padding"), Some("4px 4px 4px 4px"));
        let child = t.by_name("LBL-IN").unwrap();
        t.set(child, "FlexGrow", "1");
        t.set(child, "AlignSelf", "End");
        t.set(child, "Order", "2");
        let i = item(t.ctl(p), t.ctl(child), (200.0, 200.0), true);
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
        let i = item(t.ctl(p), t.ctl(child), (200.0, 200.0), true);
        assert_eq!(get(&i, "grid-column"), Some("2 / span 2"));
        assert_eq!(get(&i, "grid-row"), Some("span 1"), "row 0 is auto-placed");
    }

    #[test]
    fn anchors_follow_the_parents_edges() {
        let mut t = tree();
        let form = 0;
        let b = t.by_name("BTN-GO").unwrap();
        // BTN-GO: x=10 y=10 w=80 h=30 in a 640x480 form.
        let designed = item(t.ctl(form), t.ctl(b), (640.0, 480.0), false);
        assert_eq!((get(&designed, "left"), get(&designed, "top"), get(&designed, "width"), get(&designed, "height")), (Some("10px"), Some("10px"), Some("80px"), Some("30px")));
        t.set(b, "Anchor", "Top,Left,Right");
        let i = item(t.ctl(form), t.ctl(b), (640.0, 480.0), true);
        assert_eq!(get(&i, "left"), Some("10px"));
        assert_eq!(get(&i, "right"), Some("550px"), "640 - 10 - 80");
        assert_eq!(get(&i, "width"), None, "the width follows the parent");
        t.set(b, "Anchor", "Bottom,Right");
        let i = item(t.ctl(form), t.ctl(b), (640.0, 480.0), true);
        assert_eq!((get(&i, "right"), get(&i, "bottom"), get(&i, "width"), get(&i, "height")), (Some("550px"), Some("440px"), Some("80px"), Some("30px")));
        t.set(b, "Anchor", "");
        let i = item(t.ctl(form), t.ctl(b), (640.0, 480.0), true);
        assert_eq!(get(&i, "left"), Some("calc(7.8125% - 40px)"), "proportional: the centre stays at 50/640 of the width");
        t.set(b, "Dock", "Fill");
        assert_eq!(get(&item(t.ctl(form), t.ctl(b), (640.0, 480.0), true), "inset"), Some("0"));
    }

    #[test]
    fn size_limits_are_css_min_and_max() {
        let mut t = tree();
        let b = t.by_name("BTN-GO").unwrap();
        t.set(b, "MinWidth", "60");
        t.set(b, "MaxHeight", "50");
        let i = item(t.ctl(0), t.ctl(b), (640.0, 480.0), false);
        assert_eq!((get(&i, "min-width"), get(&i, "max-height")), (Some("60px"), Some("50px")));
    }
}
