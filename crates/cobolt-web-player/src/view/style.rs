// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! **The property layer of the cascade** (WASM-007 R8, WASM-016 R5): the stylesheet text that carries
//! what the Properties panel says about every control of one form instance.
//!
//! ```text
//!   lowest                                                                      highest
//!   theme defaults  <  properties (this module)  <  form stylesheet  <  CssClass  <  Style
//!   @layer pr-base     @layer pr-props               unlayered rules                 CSSOM
//! ```
//!
//! The rules are written inside `@layer pr-props`, and the base stylesheet inside `@layer pr-base`
//! (declared first, so it is the lower layer). **Any unlayered rule beats any layered one whatever its
//! specificity**, so the developer's stylesheet wins over a property without a single `!important`
//! or a specificity contest — which is the operator's ruling "CSS wins" (2026-10-19) made structural.
//!
//! Nothing here writes a `style` attribute: the page's CSP has no `'unsafe-inline'` for styles, and a
//! stylesheet object (`adoptedStyleSheets`) is the one way to give the page rules that it allows.
//!
//! # Coordinates
//!
//! A control's `X`/`Y` in a `.cfrm` are **form-absolute** (`containers.rs`: "Coordinates are
//! form-absolute"). A control inside a Panel is positioned by the Panel's element, so the offset is
//! taken from the origin of the container's content area ([`child_space`]) — the same rectangle the
//! model's `content_rect` names, which is what keeps a form laid out as it was designed.

use std::collections::BTreeMap;

use cobolt_forms::model::{Control, ControlType, Rect};

use crate::css::{self, Decls};
use crate::kinds::{player_kind, PlayerKind};
use crate::layout_css;
use crate::tree::{Ctl, Kind};
use crate::value::{css_color, fmt_number, parse_color, parse_f64};

/// The rules for a CSS string literal: `"` and `\` escaped, control characters dropped.
pub fn css_string(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    for ch in s.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            c if c.is_control() => {}
            c => out.push(c),
        }
    }
    out
}

/// `[data-pr-inst="3"] [data-name="BTN-GO"]`
pub fn selector(inst: u32, name: &str) -> String {
    format!("[data-pr-inst=\"{inst}\"] [data-name=\"{}\"]", css_string(name))
}

pub fn rule(sel: &str, d: &Decls) -> String {
    if d.is_empty() {
        return String::new();
    }
    let mut s = String::new();
    s.push_str(sel);
    s.push_str(" {");
    for (k, v) in d {
        // A value never closes its own rule or starts another (a hostile property could try).
        let v: String = v.chars().filter(|c| !matches!(c, '{' | '}' | ';' | '<' | '>')).collect();
        s.push(' ');
        s.push_str(k);
        s.push_str(": ");
        s.push_str(&v);
        s.push(';');
    }
    s.push_str(" }\n");
    s
}

fn num(c: &Ctl, k: &str) -> Option<f64> {
    c.raw(k).and_then(parse_f64)
}

fn push(d: &mut Decls, k: &str, v: impl Into<String>) {
    d.push((k.to_owned(), v.into()));
}

fn px(v: f64) -> String {
    format!("{}px", fmt_number((v * 100.0).round() / 100.0))
}

/// The width of the border a control draws (0 when it draws none).
pub fn border_px(c: &Ctl) -> f64 {
    match c.raw("borderstyle").map(|s| s.trim().to_ascii_lowercase()).as_deref() {
        None | Some("") | Some("none") => 0.0,
        Some(_) => num(c, "borderwidth").unwrap_or(1.0).clamp(0.0, 32.0),
    }
}

fn model_control(c: &Ctl) -> Control {
    let t = match &c.kind {
        Kind::Control(t) => t.clone(),
        Kind::Form => ControlType::Panel,
    };
    let mut m = Control::new(c.name.clone(), t, num(c, "x").unwrap_or(0.0) as i32, num(c, "y").unwrap_or(0.0) as i32);
    m.rect = Rect::new(num(c, "x").unwrap_or(0.0) as i32, num(c, "y").unwrap_or(0.0) as i32, num(c, "width").unwrap_or(100.0) as i32, num(c, "height").unwrap_or(24.0) as i32);
    for key in ["TabPosition", "Tabs", "FontSize", "TabPadding", "FontName", "FontScaling"] {
        if let Some(v) = c.raw(key) {
            m.set_prop(key, v.to_owned());
        }
    }
    m
}

/// The tab strip's extent and the page area of a TabControl, in form space: `(strip, x, y, w, h)`.
pub fn tab_geometry(c: &Ctl) -> (i32, Rect) {
    let m = model_control(c);
    (m.tab_strip_extent(), m.content_rect())
}

/// The coordinate system the children of container `i` live in: the origin (in form space) their
/// local positions count from, and the size anchors are measured against.
pub fn child_space(ctls: &[Ctl], i: usize) -> ((f64, f64), (f64, f64)) {
    let c = &ctls[i];
    let (x, y) = (num(c, "x").unwrap_or(0.0), num(c, "y").unwrap_or(0.0));
    let (w, h) = (num(c, "width").unwrap_or(0.0), num(c, "height").unwrap_or(0.0));
    match &c.kind {
        Kind::Form => ((0.0, 0.0), (w, h)),
        Kind::Control(ControlType::TabControl) => {
            let (_, r) = tab_geometry(c);
            ((r.x as f64, r.y as f64), (r.w as f64, r.h as f64))
        }
        Kind::Control(ControlType::Panel | ControlType::GroupBox) => {
            let b = border_px(c);
            ((x + b, y + b), ((w - 2.0 * b).max(0.0), (h - 2.0 * b).max(0.0)))
        }
        _ => ((x, y), (w, h)),
    }
}

fn is_splitter_pane(ctls: &[Ctl], i: usize) -> bool {
    let c = &ctls[i];
    matches!(&c.kind, Kind::Control(ControlType::Panel))
        && matches!(c.raw(cobolt_forms::splitter::PANE_PROP).map(str::trim), Some("1") | Some("2"))
        && c.parent.is_some_and(|p| matches!(&ctls[p].kind, Kind::Control(ControlType::Splitter)))
}

/// Which splitter pane (1 or 2) a Panel is, if it is one.
pub fn splitter_pane(ctls: &[Ctl], i: usize) -> Option<u8> {
    if !is_splitter_pane(ctls, i) {
        return None;
    }
    match ctls[i].raw(cobolt_forms::splitter::PANE_PROP).map(str::trim) {
        Some("1") => Some(1),
        Some("2") => Some(2),
        _ => None,
    }
}

/// Where control `i` is mounted: the control that holds it (0 = the form's surface) and the slot of
/// that control it goes into.
pub fn mount_of(ctls: &[Ctl], i: usize) -> (usize, super::Slot) {
    use super::Slot;
    let c = &ctls[i];
    let p = c.parent.unwrap_or(0);
    let slot = match &ctls[p].kind {
        Kind::Control(ControlType::TabControl) => Slot::Page(c.tab.unwrap_or(1).max(1)),
        Kind::Control(ControlType::Splitter) => match splitter_pane(ctls, i) {
            Some(n) => Slot::Pane(n),
            None => Slot::Children,
        },
        _ => Slot::Children,
    };
    (p, slot)
}

/// The order to mount controls in: a parent before its children, and siblings by layer, then by
/// `ZOrder` (the desktop paints them in that order; the DOM paints later siblings on top). A layer
/// above the base stacks over it and over the layers below it as one group (spec 091 R9, D20).
pub fn mount_order(ctls: &[Ctl]) -> Vec<usize> {
    fn depth(ctls: &[Ctl], mut i: usize) -> usize {
        let mut d = 0;
        while let Some(p) = ctls[i].parent {
            d += 1;
            i = p;
            if d > 64 {
                break;
            }
        }
        d
    }
    let mut order: Vec<usize> = (1..ctls.len()).collect();
    order.sort_by_key(|&i| (depth(ctls, i), ctls[i].layer.map_or(0, |l| l + 1), num(&ctls[i], "zorder").unwrap_or(0.0) as i64, i));
    order
}

fn accent_of(name: &str) -> String {
    super::charts::accent(name)
}

fn color(c: &Ctl, k: &str) -> Option<String> {
    c.raw(k).and_then(parse_color).map(css_color)
}

/// What a control type adds to the look beyond the table of `css.rs`: the colours and metrics only
/// that type has (a progress bar's `BarColor`, a check box's `CheckColor`, a label's alignment).
pub fn kind_decls(ctls: &[Ctl], i: usize) -> Decls {
    let c = &ctls[i];
    let mut d = Decls::new();
    if let Some(k) = player_kind(c) {
        if k == PlayerKind::WebLink {
            if let Some(v) = color(c, "linkcolor") {
                push(&mut d, "--pr-link", v);
            }
        }
        return d;
    }
    let Kind::Control(t) = &c.kind else { return d };
    match t {
        ControlType::Label => {
            push(&mut d, "display", "flex");
            let (_, justify) = css::h_align(c.text_or("textalignment", "Left"));
            push(&mut d, "justify-content", justify);
            push(&mut d, "align-items", css::v_align(c.text_or("verticalalignment", "Middle")));
            push(&mut d, "text-align", css::h_align(c.text_or("textalignment", "Left")).0);
            push(&mut d, "white-space", if c.flag("wordwrap") { "pre-wrap" } else { "pre" });
        }
        ControlType::Button => {
            // TextAlignment is `<Vertical><Horizontal>` in one word: MiddleCenter, TopLeft, …
            let a = c.text_or("textalignment", "MiddleCenter").to_ascii_lowercase();
            let v = if a.starts_with("top") { "flex-start" } else if a.starts_with("bottom") { "flex-end" } else { "center" };
            let h = if a.ends_with("left") { "flex-start" } else if a.ends_with("right") { "flex-end" } else { "center" };
            push(&mut d, "display", "inline-flex");
            push(&mut d, "align-items", v);
            push(&mut d, "justify-content", h);
            if let Some(n) = num(c, "iconpadding") {
                push(&mut d, "gap", px(n));
            }
        }
        ControlType::TextBox => {
            if let Some(n) = num(c, "innerpadding") {
                push(&mut d, "padding", px(n));
            }
            push(&mut d, "text-align", css::h_align(c.text_or("textalignment", "Left")).0);
            let bars = c.text_or("scrollbars", "None").to_ascii_lowercase();
            push(&mut d, "--pr-overflow", if bars == "none" { "hidden" } else { "auto" });
        }
        ControlType::CheckBox | ControlType::RadioButton => {
            if let Some(v) = color(c, "checkcolor") {
                push(&mut d, "accent-color", v);
            }
            if c.text_or("checkalignment", "Left").eq_ignore_ascii_case("Right") {
                push(&mut d, "flex-direction", "row-reverse");
                push(&mut d, "justify-content", "flex-end");
            }
            if let Some(n) = num(c, "checkspacing") {
                push(&mut d, "gap", px(n));
            }
        }
        ControlType::ProgressBar => {
            if let Some(v) = color(c, "barcolor") {
                push(&mut d, "accent-color", v);
            }
            if c.text_or("orientation", "Horizontal").eq_ignore_ascii_case("Vertical") {
                push(&mut d, "--pr-vertical", "1");
            }
        }
        ControlType::Slider => {
            if let Some(v) = color(c, "fillcolor").or_else(|| color(c, "thumbcolor")) {
                push(&mut d, "accent-color", v);
            }
        }
        ControlType::Switch => {
            push(&mut d, "--pr-accent", accent_of(c.text_or("accent", "Blue")));
        }
        ControlType::TabControl => {
            if let Some(v) = color(c, "activetabcolor") {
                push(&mut d, "--pr-accent", v);
            }
            if let Some(v) = color(c, "inactivetabcolor") {
                push(&mut d, "--pr-tab-bg", v);
            }
            if let Some(v) = color(c, "activetabforegroundcolor") {
                push(&mut d, "--pr-tab-active-fg", v);
            }
            if let Some(v) = color(c, "inactivetabforegroundcolor") {
                push(&mut d, "--pr-tab-fg", v);
            }
            if let Some(n) = num(c, "tabpadding") {
                push(&mut d, "--pr-tab-pad", px(n));
            }
        }
        ControlType::Splitter => {
            if let Some(v) = color(c, "linecolor") {
                push(&mut d, "--pr-line", v);
            }
            if let Some(n) = num(c, "linesize") {
                push(&mut d, "--pr-line-size", px(n));
            }
            if let Some(v) = color(c, "gripcolor") {
                push(&mut d, "--pr-grip", v);
            }
        }
        ControlType::Line => {
            let thick = num(c, "linethickness").unwrap_or(1.0).clamp(0.0, 40.0);
            let style = match c.text_or("dashstyle", "Solid").to_ascii_lowercase().as_str() {
                "dash" => "dashed",
                "dot" => "dotted",
                _ => "solid",
            };
            let col = color(c, "linecolor").unwrap_or_else(|| "rgb(0 0 0)".into());
            let side = if c.text_or("linedirection", "Horizontal").eq_ignore_ascii_case("Vertical") { "border-left" } else { "border-top" };
            push(&mut d, side, format!("{} {style} {col}", px(thick)));
            if c.flag("roundedends") {
                push(&mut d, "border-radius", px(thick));
            }
        }
        ControlType::Panel | ControlType::GroupBox => {
            if c.flag("hidebackground") {
                push(&mut d, "background", "transparent");
                push(&mut d, "border", "none");
            }
            if c.flag("hscroll") || c.flag("vscroll") {
                push(&mut d, "overflow-x", if c.flag("hscroll") { "auto" } else { "hidden" });
                push(&mut d, "overflow-y", if c.flag("vscroll") { "auto" } else { "hidden" });
            }
        }
        ControlType::PictureBox | ControlType::Animator => {
            if !c.flag("showframe") && matches!(t, ControlType::PictureBox) {
                push(&mut d, "background", "transparent");
                push(&mut d, "border", "none");
            }
        }
        ControlType::DataGrid => {
            if let Some(v) = color(c, "headerbackgroundcolor") {
                push(&mut d, "--pr-grid-head-bg", v);
            }
            if let Some(v) = color(c, "headerforegroundcolor") {
                push(&mut d, "--pr-grid-head-fg", v);
            }
            if let Some(v) = color(c, "gridlinecolor") {
                push(&mut d, "--pr-grid-line", v);
            }
            if let Some(v) = color(c, "alternatingrowcolor") {
                push(&mut d, "--pr-grid-alt", v);
            }
            if let Some(n) = num(c, "rowheight") {
                push(&mut d, "--pr-row-h", px(n));
            }
        }
        ControlType::ListBox | ControlType::ComboBox => {
            if let Some(v) = color(c, "activeitemcolor") {
                push(&mut d, "--pr-accent", v);
            }
            if let Some(v) = color(c, "activeitemtextcolor") {
                push(&mut d, "--pr-active-fg", v);
            }
        }
        ControlType::MenuBar | ControlType::SideMenu => {
            if let Some(v) = color(c, "highlightbgcolor") {
                push(&mut d, "--pr-menu-hover", v);
            }
            if let Some(v) = color(c, "highlightfgcolor") {
                push(&mut d, "--pr-menu-hover-fg", v);
            }
            if let Some(v) = color(c, "selectedbgcolor") {
                push(&mut d, "--pr-accent", v);
            }
            if let Some(v) = color(c, "selectedfgcolor") {
                push(&mut d, "--pr-menu-selected-fg", v);
            }
            if matches!(t, ControlType::SideMenu) {
                if let Some(n) = num(c, "collapsedwidth") {
                    push(&mut d, "--pr-rail-collapsed", px(n));
                }
            }
        }
        _ => {}
    }
    d
}

/// Does the control have a place on the page at all (a Timer or a service object has not)?
pub fn has_a_box(c: &Ctl) -> bool {
    match &c.kind {
        Kind::Form => true,
        Kind::Control(t) => !crate::kinds::is_non_visual(t),
    }
}

/// The declarations of control `i` — layout, then appearance, then what only its type adds.
/// `shell_rail` is the width of the SideMenu rail when the application is a shell with one.
pub fn control_decls(ctls: &[Ctl], i: usize, responsive: bool) -> Decls {
    let c = &ctls[i];
    let mut d = Decls::new();
    if !has_a_box(c) {
        return d;
    }
    let parent = c.parent.unwrap_or(0);
    if let Some(_pane) = splitter_pane(ctls, i) {
        push(&mut d, "position", "absolute");
        push(&mut d, "inset", "0");
    } else {
        let (origin, size) = child_space(ctls, parent);
        d.extend(layout_css::item(&ctls[parent], c, size, origin, responsive));
    }
    if c.is_container() && !matches!(&c.kind, Kind::Form) {
        for (k, v) in layout_css::container(c) {
            if k != "position" {
                d.push((k, v));
            }
        }
    }
    d.extend(css::appearance(c));
    d.extend(kind_decls(ctls, i));
    d
}

/// The tab strip and page area of a TabControl, as rules relative to the control.
fn tab_rules(inst: u32, ctls: &[Ctl], i: usize) -> String {
    let c = &ctls[i];
    let (strip, page) = tab_geometry(c);
    let (x, y) = (num(c, "x").unwrap_or(0.0), num(c, "y").unwrap_or(0.0));
    let (w, h) = (num(c, "width").unwrap_or(0.0), num(c, "height").unwrap_or(0.0));
    let (px_, py) = (page.x as f64 - x, page.y as f64 - y);
    let mut pages = Decls::new();
    push(&mut pages, "position", "absolute");
    push(&mut pages, "left", px(px_));
    push(&mut pages, "top", px(py));
    push(&mut pages, "width", px(page.w as f64));
    push(&mut pages, "height", px(page.h as f64));
    let mut strip_d = Decls::new();
    push(&mut strip_d, "position", "absolute");
    let pos = c.text_or("tabposition", "Top").to_ascii_lowercase();
    let strip = strip as f64;
    match pos.as_str() {
        "bottom" => {
            push(&mut strip_d, "left", "0");
            push(&mut strip_d, "right", "0");
            push(&mut strip_d, "bottom", "0");
            push(&mut strip_d, "height", px(strip));
        }
        "left" => {
            push(&mut strip_d, "left", "0");
            push(&mut strip_d, "top", "0");
            push(&mut strip_d, "bottom", "0");
            push(&mut strip_d, "width", px(strip));
        }
        "right" => {
            push(&mut strip_d, "right", "0");
            push(&mut strip_d, "top", "0");
            push(&mut strip_d, "bottom", "0");
            push(&mut strip_d, "width", px(strip));
        }
        _ => {
            push(&mut strip_d, "left", "0");
            push(&mut strip_d, "right", "0");
            push(&mut strip_d, "top", "0");
            push(&mut strip_d, "height", px(strip));
        }
    }
    let _ = (w, h);
    let base = selector(inst, &c.name);
    format!("{}{}", rule(&format!("{base} > .pr-tablist"), &strip_d), rule(&format!("{base} > .pr-tabpages"), &pages))
}

/// What the form's window carries: its background, its translucency, its corners.
pub fn form_decls(form: &Ctl, assets: &BTreeMap<String, String>) -> Decls {
    let mut d = Decls::new();
    if let Some((k, v)) = css::background(form) {
        push(&mut d, k, v);
    }
    let bg = form.raw("backgroundimage").unwrap_or("").trim().to_ascii_lowercase().replace('\\', "/");
    if !bg.is_empty() {
        if let Some(url) = assets.get(&bg) {
            // The image goes UNDER a gradient, if there is one, and covers the window.
            push(&mut d, "background-image", format!("url(\"{}\")", css_string(url)));
            push(&mut d, "background-size", "cover");
            push(&mut d, "background-position", "center");
        }
    }
    if let Some(r) = num(form, "cornerradius").filter(|r| *r > 0.0) {
        push(&mut d, "border-radius", px(r));
        push(&mut d, "overflow", "hidden");
    }
    d
}

/// The whole property layer of one instance: a `@layer pr-props { … }` block.
///
/// * `responsive` — the form's `Responsive` property: anchors, docks and the container modes apply;
///   off, every control sits at its designed rectangle on a surface of the designed size.
pub fn instance_css(inst: u32, ctls: &[Ctl], responsive: bool, assets: &BTreeMap<String, String>) -> String {
    let mut parts = vec![form_css(inst, ctls, responsive, assets)];
    for i in 1..ctls.len() {
        parts.push(control_css(inst, ctls, i, responsive));
    }
    wrap_layer(&parts)
}

/// The rules of the form's window and its surface.
pub fn form_css(inst: u32, ctls: &[Ctl], responsive: bool, assets: &BTreeMap<String, String>) -> String {
    let form = &ctls[0];
    let root_sel = format!("[data-pr-inst=\"{inst}\"]");
    let mut out = rule(&root_sel, &form_decls(form, assets));
    // The surface: the form's own container mode applies to it, and a non-responsive form has the designed size.
    let mut surface = layout_css::container(form);
    surface.retain(|(k, _)| k != "position");
    if !responsive {
        let (w, h) = (num(form, "width").unwrap_or(640.0), num(form, "height").unwrap_or(480.0));
        surface.push(("width".into(), px(w)));
        surface.push(("height".into(), px(h)));
    }
    out.push_str(&rule(&format!("{root_sel} > .pr-surface"), &surface));
    out
}

/// The rules of one control (a TabControl adds the rules of its strip and its pages).
pub fn control_css(inst: u32, ctls: &[Ctl], i: usize, responsive: bool) -> String {
    let d = control_decls(ctls, i, responsive);
    let mut out = rule(&selector(inst, &ctls[i].name), &d);
    if matches!(&ctls[i].kind, Kind::Control(ControlType::TabControl)) {
        out.push_str(&tab_rules(inst, ctls, i));
    }
    out
}

/// Put rules in the property layer.
pub fn wrap_layer(parts: &[String]) -> String {
    let mut out = String::from("@layer pr-props {\n");
    for p in parts {
        out.push_str(p);
    }
    out.push_str("}\n");
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tree::testing::{tree, tree_of};

    fn decl<'a>(d: &'a Decls, k: &str) -> Option<&'a str> {
        d.iter().rev().find(|(x, _)| x == k).map(|(_, v)| v.as_str())
    }

    #[test]
    fn a_rule_is_written_in_the_property_layer_under_the_instances_selector() {
        let t = tree();
        let css = instance_css(7, &t.ctls, false, &BTreeMap::new());
        assert!(css.starts_with("@layer pr-props {"), "{css}");
        assert!(css.contains("[data-pr-inst=\"7\"] [data-name=\"BTN-GO\"] {"), "{css}");
        assert!(css.contains("position: absolute; left: 10px; top: 10px; width: 80px; height: 30px;"), "a designed rectangle: {css}");
        assert!(css.trim_end().ends_with('}'));
    }

    #[test]
    fn a_hostile_value_cannot_close_its_rule_or_open_another() {
        let mut t = tree();
        let i = t.by_name("BTN-GO").unwrap();
        t.set(i, "FontName", "x\"; } body { display: none } /*");
        let css = instance_css(0, &t.ctls, false, &BTreeMap::new());
        assert_eq!(css.matches('{').count(), css.matches('}').count(), "braces still balance: {css}");
        assert!(!css.contains("body {"), "{css}");
        assert_eq!(css_string("a\"b\\c"), "a\\\"b\\\\c");
    }

    #[test]
    fn a_control_in_a_panel_is_positioned_from_the_panels_content_origin() {
        // The panel is at (300,10) with a 1px border; the label is at (310,30) in FORM space.
        let t = tree_of(
            r#"<Control id="PNL" type="Panel" x="300" y="10" w="200" h="100"><Property name="BorderStyle">Single</Property><Property name="BorderWidth">1</Property></Control>
               <Control id="LBL" type="Label" x="310" y="30" w="50" h="20" parent="PNL"></Control>"#,
        );
        let l = t.by_name("LBL").unwrap();
        let d = control_decls(&t.ctls, l, false);
        assert_eq!((decl(&d, "left"), decl(&d, "top")), (Some("9px"), Some("19px")), "310-300-1, 30-10-1");
        let (origin, size) = child_space(&t.ctls, t.by_name("PNL").unwrap());
        assert_eq!((origin, size), ((301.0, 11.0), (198.0, 98.0)));
    }

    #[test]
    fn a_responsive_form_follows_anchors_and_a_fixed_one_keeps_the_designed_rectangle() {
        let mut t = tree();
        let b = t.by_name("BTN-GO").unwrap();
        t.set(b, "Anchor", "Top,Left,Right");
        let fixed = control_decls(&t.ctls, b, false);
        assert_eq!((decl(&fixed, "left"), decl(&fixed, "width")), (Some("10px"), Some("80px")));
        let live = control_decls(&t.ctls, b, true);
        assert_eq!((decl(&live, "left"), decl(&live, "right"), decl(&live, "width")), (Some("10px"), Some("550px"), None));
    }

    #[test]
    fn a_breakpoint_override_changes_the_design_but_never_a_property_a_program_wrote() {
        use cobolt_forms::layout::breakpoints::{Breakpoint, Override};
        use cobolt_forms::model::PropValue;
        let mut t = tree();
        let b = t.by_name("BTN-GO").unwrap();
        let narrow = Breakpoint {
            name: "narrow".into(),
            min_width: 0,
            font_factor: 1.0,
            overrides: vec![
                Override { control: "BTN-GO".into(), property: "Width".into(), value: PropValue::Int(40) },
                Override { control: "BTN-GO".into(), property: "Height".into(), value: PropValue::Int(50) },
            ],
        };
        let ctls = t.effective_ctls(Some(&narrow));
        assert_eq!(ctls[b].raw("width"), Some("40"));
        t.set(b, "Width", "120");
        let ctls = t.effective_ctls(Some(&narrow));
        assert_eq!(ctls[b].raw("width"), Some("120"), "the program's write outranks the breakpoint");
        assert_eq!(ctls[b].raw("height"), Some("50"), "an unwritten property still follows it");
        assert!(t.was_written(b, "WIDTH"));
        assert_eq!(t.effective_ctls(None)[b].raw("height"), Some("30"), "no breakpoint is the design");
    }

    #[test]
    fn type_specific_colours_become_the_properties_the_browser_understands() {
        let t = tree_of(
            r#"<Control id="PB" type="ProgressBar" x="0" y="0" w="100" h="20"><Property name="BarColor">#30A46C</Property></Control>
               <Control id="LB" type="Label" x="0" y="30" w="100" h="20"><Property name="TextAlignment">Right</Property><Property name="VerticalAlignment">Top</Property></Control>
               <Control id="LN" type="Line" x="0" y="60" w="100" h="4"><Property name="LineThickness">3</Property><Property name="DashStyle">Dash</Property><Property name="LineColor">#FF0000</Property></Control>"#,
        );
        let pb = control_decls(&t.ctls, t.by_name("PB").unwrap(), false);
        assert_eq!(decl(&pb, "accent-color"), Some("rgb(48 164 108)"));
        let lb = control_decls(&t.ctls, t.by_name("LB").unwrap(), false);
        assert_eq!((decl(&lb, "justify-content"), decl(&lb, "align-items")), (Some("flex-end"), Some("flex-start")));
        let ln = control_decls(&t.ctls, t.by_name("LN").unwrap(), false);
        assert_eq!(decl(&ln, "border-top"), Some("3px dashed rgb(255 0 0)"));
    }

    #[test]
    fn a_tab_control_places_its_strip_and_pages_where_the_model_puts_them() {
        let t = tree_of(
            r#"<Control id="TC" type="TabControl" x="20" y="20" w="300" h="200"><Property name="Tabs">One
Two</Property></Control>
               <Control id="IN" type="Label" x="30" y="60" w="40" h="20" parent="TC" tab="1"></Control>"#,
        );
        let tc = t.by_name("TC").unwrap();
        let (strip, page) = tab_geometry(&t.ctls[tc]);
        assert!(strip >= 26, "{strip}");
        assert_eq!((page.x, page.w), (22, 296), "inset 2 on each side");
        let (origin, _) = child_space(&t.ctls, tc);
        assert_eq!(origin, (page.x as f64, page.y as f64));
        let css = instance_css(0, &t.ctls, false, &BTreeMap::new());
        assert!(css.contains("[data-name=\"TC\"] > .pr-tabpages {"), "{css}");
        assert!(css.contains("[data-name=\"TC\"] > .pr-tablist {"), "{css}");
        let d = control_decls(&t.ctls, t.by_name("IN").unwrap(), false);
        assert_eq!(decl(&d, "left"), Some(&*px(30.0 - page.x as f64)));
    }

    #[test]
    fn the_form_window_carries_the_background_and_a_non_responsive_surface_the_designed_size() {
        let mut t = tree();
        t.set(0, "BackgroundColor", "#202020");
        t.set(0, "CornerRadius", "12");
        let css = instance_css(1, &t.ctls, false, &BTreeMap::new());
        assert!(css.contains("[data-pr-inst=\"1\"] { background-color: rgb(32 32 32); border-radius: 12px; overflow: hidden; }"), "{css}");
        assert!(css.contains("[data-pr-inst=\"1\"] > .pr-surface { width: 640px; height: 480px; }"), "{css}");
        let live = instance_css(1, &t.ctls, true, &BTreeMap::new());
        assert!(!live.contains("> .pr-surface { width"), "a responsive surface fills the window");
    }

    #[test]
    fn splitter_panes_fill_their_slots() {
        let t = tree_of(
            r#"<Control id="SP" type="Splitter" x="0" y="0" w="300" h="200"></Control>
               <Control id="SP-Pane1" type="Panel" x="2" y="2" w="148" h="196" parent="SP"><Property name="IsSplitterPane">1</Property></Control>"#,
        );
        let p = t.by_name("SP-Pane1").unwrap();
        assert_eq!(splitter_pane(&t.ctls, p), Some(1));
        let d = control_decls(&t.ctls, p, true);
        assert_eq!((decl(&d, "position"), decl(&d, "inset")), (Some("absolute"), Some("0")));
    }

    #[test]
    fn non_visual_controls_have_no_rule() {
        let t = tree_of(r#"<Control id="TM" type="Timer" x="0" y="0" w="48" h="48"></Control>"#);
        assert!(control_decls(&t.ctls, t.by_name("TM").unwrap(), false).is_empty());
    }
}
