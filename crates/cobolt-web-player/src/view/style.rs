// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! **The property layer of the cascade** (WASM-007 R8, WASM-016 R5): the stylesheet text that carries
//! what the Properties panel says about every control of one form instance.
//!
//! ```text
//!   lowest                                                                                         highest
//!   structure  <  theme defaults  <  properties (this module)  <  project sheet  <  form sheet, CssClass  <  CssStyle
//!   pr-base       pr-theme           pr-props                     pr-app            unlayered, scoped        unlayered, last
//! ```
//!
//! The rules are written inside `@layer pr-props`; the order of the layers is declared once, by the
//! base stylesheet's first statement (`base_css::LAYER_ORDER`). **Any unlayered rule beats any layered one whatever its
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
use crate::kinds::player_kind;
use crate::layout_css;
use crate::metrics;
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
    css::border_width(c)
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
    if let Some((x, y, w, h)) = side_menu_footer_band(ctls, i) {
        let b = border_px(c);
        return ((x + b, y + b), ((w - 2.0 * b).max(0.0), (h - 2.0 * b).max(0.0)));
    }
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

/// How tall a SideMenu's footer band is: its `FooterHeight`, or the desktop's default when unset.
fn side_menu_footer_height(side: &Ctl) -> f64 {
    num(side, "footerheight").filter(|h| *h >= 0.0).unwrap_or(cobolt_forms::model::DEFAULT_SIDE_MENU_FOOTER_H as f64)
}

/// Does SideMenu `i` hold its footer Panel?
fn has_footer_panel(ctls: &[Ctl], i: usize) -> bool {
    ctls[i].children.iter().any(|&k| side_menu_footer_band(ctls, k).is_some())
}

/// When control `i` is the Panel a SideMenu owns in its footer band, that band in form space
/// (`x, y, w, h`): the rail's width, `FooterHeight` tall, at the rail's foot. The desktop pins the Panel
/// there whatever rectangle the file gives it (`FormModel::sync_side_menu_footer_panels`), and the
/// controls inside it keep their form-space positions, so they count from the band, not from the file.
pub fn side_menu_footer_band(ctls: &[Ctl], i: usize) -> Option<(f64, f64, f64, f64)> {
    let c = &ctls[i];
    if !matches!(&c.kind, Kind::Control(ControlType::Panel)) || !c.flag(cobolt_forms::model::SIDE_MENU_FOOTER_PROP) {
        return None;
    }
    let side = &ctls[c.parent?];
    if !matches!(&side.kind, Kind::Control(ControlType::SideMenu)) {
        return None;
    }
    let full_height = !matches!(side.raw("fullheight").map(str::trim), Some("false" | "0"));
    let (x, w) = (num(side, "x").unwrap_or(0.0), num(side, "width").unwrap_or(0.0));
    let (y, h) = if full_height { (0.0, num(&ctls[0], "height").unwrap_or(0.0)) } else { (num(side, "y").unwrap_or(0.0), num(side, "height").unwrap_or(0.0)) };
    let fh = side_menu_footer_height(side).min(h.max(0.0));
    Some((x, y + h - fh, w, fh))
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
        _ => match (p, c.layer) {
            // A control of a layer is mounted in its layer's element, not on the base's surface.
            (0, Some(k)) => Slot::Layer(k),
            _ => Slot::Children,
        },
    };
    (p, slot)
}

/// The rules of a form's layers (WASM-004 R14, R18): layer `k` stacks at `z-index: k + 1` over the base, which has none;
/// each is its own stacking context (`base_css`: `.pr-form-layer`), so every control of a higher layer paints above every
/// control of a lower one whatever its `ZOrder`. Its backdrop is the first child, `.pr-layer-bg`, painted from the layer as
/// the program left it (`Layer::with_live`): `Transparency` is the alpha of its colour or gradient stops, never an
/// `opacity` on the layer (R18), so the controls keep their own. A picture is laid as the form's is (`form_decls`); it is
/// drawn at full alpha. Empty for a form without layers (R24).
pub fn layers_css(inst: u32, layers: &[crate::tree::LayerState], assets: &BTreeMap<String, String>) -> String {
    let mut out = String::new();
    for (k, l) in layers.iter().enumerate() {
        let sel = format!("[data-pr-inst=\"{inst}\"] > .pr-form-layer[data-layer-index=\"{k}\"]");
        let mut z = Decls::new();
        push(&mut z, "z-index", (k + 1).to_string());
        out.push_str(&rule(&sel, &z));
        let live = live_layer(l);
        let mut bg = layer_backdrop_decls(&live, assets);
        // An opaque backdrop keeps the pointer from what it covers (R16); its listener discards the event
        // (`dom::instance`). A see-through one is never a target: the event passes to the layers below.
        if live.is_opaque() {
            push(&mut bg, "pointer-events", "auto");
        }
        out.push_str(&rule(&format!("{sel} > .pr-layer-bg"), &bg));
    }
    out
}

/// Which parts of a form the user can reach this frame (R15, plan §1.4): `(base inert, [(shown, inert)] per layer)`.
/// A hidden layer is out of reach; while an opaque layer is shown, the base and every layer under it are `inert` too,
/// as the desktop shields them from the focus (`cobolt_forms::render::opaque_shield_rank`).
pub fn layer_reach(layers: &[crate::tree::LayerState]) -> (bool, Vec<(bool, bool)>) {
    let shield = layers.iter().rposition(|l| l.visible && live_layer(l).is_opaque()).map_or(0, |k| k + 1);
    (shield > 0, layers.iter().enumerate().map(|(k, l)| (l.visible, !l.visible || k + 1 < shield)).collect())
}

/// The model's layer, with what the program has written over the design (`Layer::with_live`).
pub fn live_layer(l: &crate::tree::LayerState) -> cobolt_forms::model::Layer {
    cobolt_forms::model::Layer::new(l.name.clone()).with_live(l.props.iter())
}

/// A layer's backdrop as declarations (R18).
pub fn layer_backdrop_decls(l: &cobolt_forms::model::Layer, assets: &BTreeMap<String, String>) -> Decls {
    let b = &l.backdrop;
    let keep = 1.0 - f64::from(b.transparency.min(100)) / 100.0;
    let fade = |hex: &str| parse_color(hex).map(|c| css_color([c[0], c[1], c[2], (f64::from(c[3]) * keep).round() as u8]));
    let mut d = Decls::new();
    let mut images: Vec<String> = Vec::new();
    let image = b.image.trim().to_ascii_lowercase().replace('\\', "/");
    if let Some(url) = (!image.is_empty()).then(|| assets.get(&image)).flatten() {
        images.push(format!("url(\"{}\")", css_string(url)));
        let (size, repeat, position) = match b.image_mode {
            cobolt_forms::model::BgImageMode::Stretch => ("100% 100%", "no-repeat", "center"),
            cobolt_forms::model::BgImageMode::Tile => ("auto", "repeat", "0 0"),
            cobolt_forms::model::BgImageMode::Center => ("auto", "no-repeat", "center"),
            cobolt_forms::model::BgImageMode::Fill => ("cover", "no-repeat", "center"),
            cobolt_forms::model::BgImageMode::Fit => ("contain", "no-repeat", "center"),
        };
        push(&mut d, "background-size", size);
        push(&mut d, "background-repeat", repeat);
        push(&mut d, "background-position", position);
    }
    if b.gradient_enabled {
        if let (Some(a), Some(z)) = (fade(&b.gradient_start_color), fade(&b.gradient_end_color)) {
            images.push(format!("linear-gradient({}, {a}, {z})", css::gradient_angle(&b.gradient_direction)));
        }
    } else if let Some(c) = fade(&b.color) {
        push(&mut d, "background-color", c);
    }
    if !images.is_empty() {
        // A picture over the gradient; each layer of the list its own size and repeat.
        if images.len() == 2 {
            for k in ["background-size", "background-repeat", "background-position"] {
                if let Some(slot) = d.iter_mut().find(|(x, _)| x == k) {
                    slot.1 = format!("{}, {}", slot.1, if k == "background-repeat" { "no-repeat" } else if k == "background-size" { "100% 100%" } else { "0 0" });
                }
            }
        }
        push(&mut d, "background-image", images.join(", "));
    }
    d
}

/// The order to mount controls in: a parent before its children, and siblings by layer, then by
/// `TabOrder` (WASM-004 R20): the document order is the keyboard order — the base's controls, then
/// each layer upward, each in its own tab order, as the desktop walks them (`render.rs`, sorted by
/// layer, then tab order) — so no positive `tabindex` is needed. The stacking is not the document's:
/// `ZOrder` is each control's `z-index` (`css::appearance`), and each layer a stacking context above
/// the one before (spec 091 R9, D20). Siblings of equal `ZOrder` that overlap stack in tab order.
///
/// The children of a container that lays them out (Flex, Flow, Grid) are mounted in the solver's
/// **reading order** instead — top to bottom by their designed rectangles, then left to right (right
/// to left in a right-to-left container) — because that is the order the desktop places them in
/// (`cobolt_forms::layout::reading_order`, spec 056 R54), and the browser's flex and grid layout
/// places its items in document order. The `Order` property then moves an item by CSS `order`, as
/// it does for the solver, which sorts by it first and keeps the reading order among equals.
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
    // The two keys that order a control among its siblings: its place in reading order when its
    // parent lays its children out, else its `TabOrder`.
    let among_siblings = |i: usize| -> (i64, i64) {
        let parent = ctls[i].parent.unwrap_or(0);
        match layout_css::mode_of(&ctls[parent]) {
            layout_css::Mode::Absolute => (num(&ctls[i], "taborder").unwrap_or(0.0) as i64, 0),
            _ => {
                let (x, y) = (num(&ctls[i], "x").unwrap_or(0.0) as i64, num(&ctls[i], "y").unwrap_or(0.0) as i64);
                (y, if layout_css::reads_right_to_left(&ctls[parent]) { -x } else { x })
            }
        }
    };
    let mut order: Vec<usize> = (1..ctls.len()).collect();
    order.sort_by_key(|&i| {
        let (a, b) = among_siblings(i);
        (depth(ctls, i), ctls[i].layer.map_or(0, |l| l + 1), a, b, i)
    });
    order
}

fn accent_of(name: &str) -> String {
    super::charts::accent(name)
}

fn color(c: &Ctl, k: &str) -> Option<String> {
    c.raw(k).and_then(parse_color).map(css_color)
}

/// The caption shrunk to fit a frame `max_w` by `max_h` (`paint::fitted_caption_galley_wrapping`): when the designed size
/// does not fit, the size it is shrunk to is written (with the row and the outline that go with it). Answers the size
/// the caption ends up at and the height of the block of rows it fills.
fn fit_caption(c: &Ctl, d: &mut Decls, max_w: f64, max_h: f64, wrap: bool) -> (f64, f64) {
    // Not rounded: the painter fits from the size it is handed, a fraction when the form's font factor made one.
    let designed = num(c, "fontsize").filter(|s| *s > 0.0).unwrap_or(11.0).max(1.0);
    let text = c.raw("caption").unwrap_or("");
    if text.trim().is_empty() {
        return (designed, metrics::row_height(designed));
    }
    let size = metrics::fitted_size(text, designed, max_w.max(1.0), max_h.max(1.0), wrap);
    if size < designed {
        let ink = c.raw("foregroundcolor").and_then(parse_color).unwrap_or([255, 255, 255, 255]);
        push(d, "font-size", px(size));
        push(d, "line-height", px(metrics::row_height(size)));
        push(d, "-webkit-text-stroke", px(metrics::stroke(size, ink)));
    }
    let (lines, _) = metrics::lay_out(text, size, max_w.max(1.0), wrap);
    (size, lines.max(1) as f64 * metrics::row_height(size))
}

/// The padding above and below a line of text that makes a browser put the line's baseline where the
/// desktop puts it: `nudge` pixels lower (or higher, when negative) than the line box alone would. Centred text
/// moves by half of what is added to one side; text held to an edge moves by all of it.
///
/// Centred text is also kept off the half pixel. When the frame is an odd number of pixels taller than the row,
/// centring leaves half a pixel above it. The desktop rounds the frame's own top to a whole pixel first and then
/// rounds that half up, so a frame whose top lies at .8 of a pixel draws its text a pixel lower than the same text
/// in a browser, which adds the half pixel to the fraction and rounds once. Adding it here, as a whole number of
/// pixels above the line, makes both round the same way wherever the frame falls.
fn nudged_padding(align: &str, base: f64, nudge: f64, frame: f64, row: f64) -> (f64, f64) {
    match align {
        "flex-start" => ((base + nudge).max(0.0), base),
        "flex-end" => (base, (base - nudge).max(0.0)),
        _ => {
            let (top, bottom) = if nudge >= 0.0 { (base + 2.0 * nudge, base) } else { (base, base + 2.0 * -nudge) };
            let odd = (frame.round() - row).round().rem_euclid(2.0) == 1.0;
            (if odd { top + 1.0 } else { top }, bottom)
        }
    }
}

/// What a control type adds to the look beyond the table of `css.rs`: the colours and metrics only
/// that type has (a progress bar's `BarColor`, a check box's `CheckColor`, a label's alignment).
pub fn kind_decls(ctls: &[Ctl], i: usize) -> Decls {
    let c = &ctls[i];
    let mut d = Decls::new();
    if player_kind(c).is_some() {
        return d;
    }
    let Kind::Control(t) = &c.kind else { return d };
    match t {
        // WASM-006 R3 — the state colours as the custom properties `base_css` reads; `:visited` takes
        // the colour only (Q5, plan D8: browsers apply nothing else to a visited link).
        ControlType::Hyperlink => {
            for (prop, var) in [("linkcolor", "--pr-link"), ("hovercolor", "--pr-hover"), ("activecolor", "--pr-active"), ("visitedcolor", "--pr-visited")] {
                if let Some(v) = color(c, prop) {
                    push(&mut d, var, v);
                }
            }
        }
        ControlType::Label => {
            push(&mut d, "display", "flex");
            let (_, justify) = css::h_align(c.text_or("textalignment", "Left"));
            push(&mut d, "justify-content", justify);
            let v = css::v_align(c.text_or("verticalalignment", "Middle"));
            push(&mut d, "align-items", v);
            push(&mut d, "text-align", css::h_align(c.text_or("textalignment", "Left")).0);
            push(&mut d, "white-space", if c.flag("wordwrap") { "pre-wrap" } else { "pre" });
            // The caption's inset from the frame (`paint`, the Label branch): 3 px across and 2 px
            // down, or a quarter of a side that is smaller, plus the control's own `Padding`, and
            // never more than 45 % of the side.
            let (w, h) = (num(c, "width").unwrap_or(0.0), num(c, "height").unwrap_or(0.0));
            let own = num(c, "padding").unwrap_or(0.0).clamp(0.0, 128.0);
            let (across, down) = ((3.0_f64.min(w * 0.25) + own).min(w * 0.45), (2.0_f64.min(h * 0.25) + own).min(h * 0.45));
            // The caption shrinks until it fits the frame — the width inside its inset, the whole height.
            let (size, block) = fit_caption(c, &mut d, w - 2.0 * across, h, c.flag("wordwrap"));
            let (top, bottom) = nudged_padding(v, down, metrics::baseline_nudge(size), h, block);
            push(&mut d, "padding", format!("{} {} {} {}", px(top), px(across), px(bottom), px(across)));
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
            // The caption shrinks until it fits the frame inside its padding, and sits `inset` from the edge it
            // is aligned to (`paint`, the Button branch and `button_content_layout`).
            let (w, hh) = (num(c, "width").unwrap_or(0.0), num(c, "height").unwrap_or(0.0));
            let own = num(c, "padding").unwrap_or(0.0).clamp(0.0, 128.0);
            let bpad = (3.0_f64.min(w * 0.2) + own).min(w * 0.45).min(hh * 0.45);
            let inset = 6.0_f64.min(w * 0.25).min(hh * 0.25);
            let (size, block) = fit_caption(c, &mut d, w - 2.0 * bpad, hh - 2.0 * bpad, true);
            let (top, bottom) = nudged_padding(v, inset, metrics::baseline_nudge(size), hh, block);
            push(&mut d, "padding", format!("{} {} {} {}", px(top), px(inset), px(bottom), px(inset)));
        }
        ControlType::TextBox => {
            // The text starts `pad` inside the frame — the box's inner padding and its own `Padding`, and never less than the
            // corner radius, so that it stays inside the arc (`paint`, the TextBox branch) — measured from the frame's
            // outer edge, so the border counts as part of it.
            let (w, h) = (num(c, "width").unwrap_or(0.0), num(c, "height").unwrap_or(0.0));
            let inner = num(c, "innerpadding").unwrap_or(3.0).clamp(0.0, 128.0) + num(c, "padding").unwrap_or(0.0).clamp(0.0, 128.0);
            let radius = num(c, "cornerradius").unwrap_or(0.0).clamp(0.0, (0.5 * w.min(h)).max(0.0));
            let pad = inner.min(128.0).max(radius).min(w * 0.45);
            let across = (pad - border_px(c)).max(0.0);
            let vertical = if c.flag("multiline") { across } else { 0.0 };
            push(&mut d, "padding", format!("{} {} {} {}", px(vertical), px(across), px(vertical), px(across)));
            push(&mut d, "text-align", css::h_align(c.text_or("textalignment", "Left")).0);
            let bars = c.text_or("scrollbars", "None").to_ascii_lowercase();
            push(&mut d, "--pr-overflow", if bars == "none" { "hidden" } else { "auto" });
        }
        ControlType::NumericUpDown | ControlType::DateTimePicker => {
            // The text at rest is centred in the frame, its baseline where the desktop puts it (`pr-field-face`).
            let size = num(c, "fontsize").filter(|s| *s > 0.0).unwrap_or(11.0);
            let (top, bottom) = nudged_padding("center", 0.0, metrics::baseline_nudge(size), num(c, "height").unwrap_or(0.0), metrics::row_height(size));
            push(&mut d, "--pr-nt", px(top));
            push(&mut d, "--pr-nb", px(bottom));
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
            // The stroke is the view's SVG (`controls::line`), painted as the desktop's Line block paints it
            // (`paint::draw_control_body`): dash 5t on 4t off, dot 1.2t on 2.5t off, dash-dot 5t 3t 1.2t 3t
            // (t the thickness, at least 1), round ends for RoundedEnds.
            let thick = num(c, "linethickness").unwrap_or(1.0).clamp(0.0, 40.0);
            let t = thick.max(1.0);
            let dashes: &[f64] = match c.text_or("dashstyle", "Solid").to_ascii_lowercase().as_str() {
                "dash" => &[5.0, 4.0],
                "dot" => &[1.2, 2.5],
                "dashdot" => &[5.0, 3.0, 1.2, 3.0],
                _ => &[],
            };
            push(&mut d, "--pr-line-color", color(c, "linecolor").unwrap_or_else(|| "rgb(0 0 0)".into()));
            push(&mut d, "--pr-line-width", px(thick));
            if !dashes.is_empty() {
                push(&mut d, "--pr-line-dash", dashes.iter().map(|k| px(k * t)).collect::<Vec<_>>().join(" "));
            }
            if c.flag("roundedends") {
                push(&mut d, "--pr-line-cap", "round");
            }
        }
        ControlType::Panel | ControlType::GroupBox => {
            let scrolls = c.flag("hscroll") || c.flag("vscroll");
            if c.flag("expandable") {
                // The expand icon sits inside the card's corner, clear of the arc (`paint::expand_icon_rect`).
                let radius = num(c, "cornerradius").unwrap_or(0.0).clamp(0.0, 0.5 * num(c, "width").unwrap_or(0.0).min(num(c, "height").unwrap_or(0.0)));
                push(&mut d, "--pr-exp-inset", px(6.0 + 0.25 * radius));
            }
            if c.flag("hidebackground") {
                push(&mut d, "background", "transparent");
                push(&mut d, "border", "none");
                // A container with no face has no edge of its own to stop a child's shadow against: the
                // shadow falls past it, to the nearest container that is a real boundary (a face, or a
                // scroll viewport) — `render::shadow_room_owner`. Where nothing inside it casts a shadow the
                // desktop's other rule is all that is left: a child is cut at its container's content rectangle
                // (the frame less the 2 px every Panel and GroupBox insets), so a button that is a few pixels too
                // wide for its row loses those pixels, and the page cuts it the same way.
                if !scrolls {
                    if casts_a_shadow_inside(ctls, i) {
                        push(&mut d, "overflow", "visible");
                    } else {
                        push(&mut d, "overflow", "clip");
                        push(&mut d, "overflow-clip-margin", "content-box");
                    }
                }
            }
            if scrolls {
                push(&mut d, "overflow-x", if c.flag("hscroll") { "auto" } else { "hidden" });
                push(&mut d, "overflow-y", if c.flag("vscroll") { "auto" } else { "hidden" });
            }
        }
        ControlType::BarChart | ControlType::LineChart | ControlType::PieChart | ControlType::AreaChart | ControlType::ScatterChart | ControlType::DonutChart | ControlType::RadarChart => {
            // `HideBackground` suppresses the painter's face and frame so only the chart's marks, axes and
            // captions are drawn, over what is behind it.
            if c.flag("hidebackground") {
                push(&mut d, "background", "transparent");
                push(&mut d, "border", "none");
            }
        }
        ControlType::PictureBox | ControlType::Animator => {
            if !c.flag("showframe") && matches!(t, ControlType::PictureBox) {
                push(&mut d, "background", "transparent");
                push(&mut d, "border", "none");
            }
        }
        ControlType::DataGrid => {
            // The painter draws the grid's outline itself, in its line colour, and no other border.
            push(&mut d, "border", "none");
            if let Some(v) = color(c, "headerforegroundcolor") {
                push(&mut d, "--pr-dg-head-ink", v);
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
                // The logo pane is as tall as `HeaderHeight`, as the desktop's header pane is (sidebar.rs `header_h`).
                push(&mut d, "--pr-sm-header-h", px(num(c, "headerheight").filter(|h| *h >= 0.0).unwrap_or(cobolt_forms::model::DEFAULT_SIDE_MENU_HEADER_H as f64)));
                // The footer band is as tall as `FooterHeight` while the rail holds its footer Panel; without one it takes no room.
                if has_footer_panel(ctls, i) {
                    push(&mut d, "--pr-sm-footer-h", px(side_menu_footer_height(c)));
                }
            }
        }
        _ => {}
    }
    d
}

/// Does any control inside container `i`, at any depth, cast a drop shadow?
fn casts_a_shadow_inside(ctls: &[Ctl], i: usize) -> bool {
    ctls[i].children.iter().any(|&c| ctls[c].flag("shadowenabled") || casts_a_shadow_inside(ctls, c))
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
    if splitter_pane(ctls, i).is_some() || side_menu_footer_band(ctls, i).is_some() {
        // A splitter's pane fills its slot; so does a SideMenu's footer Panel, whose slot is the rail's footer band.
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
    // Spec 090: while a card is expanded it takes the whole client area of its container, and its siblings are not drawn.
    if responsive {
        if let Some(winner) = layout_css::expanded_in_set(ctls, i) {
            if winner == i {
                d.extend(layout_css::expanded_placement(&ctls[parent]));
            } else if matches!(&c.kind, Kind::Control(_)) {
                push(&mut d, "display", "none");
            }
        }
    }
    // What the form's theme does to the control's face comes last: it fills in what the properties left to the theme.
    if ctls[0].text_or("theme", "") == cobolt_forms::theme::SPATIAL {
        crate::theme_css::spatial_decls(ctls, i, &mut d);
    }
    // A control that is not shown takes no place on the page, and its siblings close up over it, as the desktop's solver leaves it
    // out. The view says the same with its `hidden` attribute for what the design hides; what a breakpoint hides (its `Visible`
    // override) reaches the page only here, because the views are not rebuilt when the breakpoint changes.
    if !c.visible() {
        push(&mut d, "display", "none");
    }
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
    // Under a see-through theme (Spatial) the backdrop is the theme's translucent glass: a form's own colour, gradient or
    // picture would hide the blurred desktop the theme exists to show (`cobolt-form-host`, `Backdrop`). The form's
    // `Transparency` fades the glass, as it fades any other face.
    let see_through = cobolt_forms::theme::is_see_through(form.text_or("theme", ""));
    if see_through {
        let [r, g, b, a] = cobolt_forms::theme::SPATIAL_FORM_GLASS;
        let faded = (a as f64 * (1.0 - num(form, "transparency").unwrap_or(0.0).clamp(0.0, 100.0) / 100.0)).floor() / 255.0;
        push(&mut d, "background-color", format!("rgb({r} {g} {b} / {})", fmt_number((faded * 10000.0).round() / 10000.0)));
        push(&mut d, "background-image", "none");
    } else if let Some((k, v)) = css::background(form) {
        push(&mut d, k, v);
    }
    let bg = if see_through { String::new() } else { form.raw("backgroundimage").unwrap_or("").trim().to_ascii_lowercase().replace('\\', "/") };
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
    out.push_str(&rule(&format!("{root_sel} > .pr-form > .pr-surface"), &surface));
    out
}

/// The rules of one control (a TabControl adds the rules of its strip and its pages).
pub fn control_css(inst: u32, ctls: &[Ctl], i: usize, responsive: bool) -> String {
    let d = control_decls(ctls, i, responsive);
    let mut out = rule(&selector(inst, &ctls[i].name), &d);
    if matches!(&ctls[i].kind, Kind::Control(ControlType::TabControl)) {
        out.push_str(&tab_rules(inst, ctls, i));
    }
    // A chart's gradient frame is a ring over its transparent border (WASM-004 T13b).
    if let Some(ring) = css::border_ring(&ctls[i]) {
        out.push_str(&rule(&format!("{}::after", selector(inst, &ctls[i].name)), &ring));
    }
    // A card whose height is only a floor also holds that height as content, or a browser measures its children against nothing.
    if ctls[i].visible() {
        if let Some(h) = layout_css::floor_spacer(&ctls[ctls[i].parent.unwrap_or(0)], &ctls[i], &d) {
            let mut spacer = Decls::new();
            push(&mut spacer, "content", "\"\"");
            push(&mut spacer, "display", "block");
            push(&mut spacer, "width", "0");
            push(&mut spacer, "height", px(h));
            out.push_str(&rule(&format!("{}::before", selector(inst, &ctls[i].name)), &spacer));
        }
    }
    out
}

/// The inline CSS of one instance (WASM-007 R2, level 6 of R8) and what was dropped from it.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct InlineCss {
    /// Unlayered rules, one per control with a `CssStyle`.
    pub css: String,
    /// `(control, declaration, why)` for every declaration left out.
    pub dropped: Vec<(String, String, String)>,
}

/// `#NAME` with the characters a CSS identifier cannot hold escaped (a name may start with a digit).
fn id_selector(name: &str) -> String {
    let mut out = String::from("#");
    for (i, ch) in name.chars().enumerate() {
        let plain = ch.is_ascii_alphabetic() || ch == '_' || ch == '-' || (i > 0 && ch.is_ascii_digit()) || (!ch.is_ascii() && !ch.is_control());
        if plain {
            out.push(ch);
        } else if !ch.is_control() {
            out.push_str(&format!("\\{:x} ", ch as u32));
        }
    }
    out
}

/// A property name a declaration may carry: a standard one, or a custom property.
fn is_property_name(n: &str) -> bool {
    !n.is_empty() && n.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_') && !n.starts_with(|c: char| c.is_ascii_digit())
}

/// The controls' `CssStyle` as rules (WASM-007 R2, level 6 of R8): one **unlayered** rule per control,
/// selector `[data-pr-inst="n"] #NAME` — the instance's root, so it stays inside the form as the form
/// sheet does, and an id, so it beats a form-sheet rule that is not `!important`. The sheet carrying
/// it is adopted after the form sheets (`base_css::adoption_order`). The value is a declaration list,
/// never a `style` attribute (the page's CSP has no `'unsafe-inline'`): a declaration that does not
/// parse, or that the safety check refuses, is dropped and reported.
pub fn inline_css(inst: u32, ctls: &[Ctl], allowed_origins: &[String]) -> InlineCss {
    let mut out = InlineCss::default();
    for c in ctls.iter().skip(1) {
        let Some(text) = c.raw("cssstyle").filter(|t| !t.trim().is_empty()) else { continue };
        let mut d = Decls::new();
        for part in cobolt_forms::css::split_top(text, ';') {
            let part = part.trim();
            if part.is_empty() {
                continue;
            }
            let mut drop = |why: &str| out.dropped.push((c.name.clone(), part.to_owned(), why.to_owned()));
            if part.contains(['{', '}', '<', '>']) {
                drop("a declaration cannot open or close a rule");
                continue;
            }
            let parsed = cobolt_forms::css::parse_declarations(part);
            let Some(decl) = parsed.first().filter(|d| is_property_name(&d.name)) else {
                drop("not a declaration (`property: value`)");
                continue;
            };
            if let Err(e) = css::css_is_safe(part, allowed_origins) {
                drop(&e);
                continue;
            }
            let value = if decl.important { format!("{} !important", decl.value) } else { decl.value.clone() };
            push(&mut d, &decl.name, value);
        }
        out.css.push_str(&rule(&format!("[data-pr-inst=\"{inst}\"] {}", id_selector(&c.name)), &d));
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
    fn inline_css_is_one_unlayered_rule_per_control_with_only_what_parsed() {
        let mut t = tree();
        let go = t.by_name("BTN-GO").unwrap();
        t.set(go, "CssStyle", "background-color: red; nonsense; color: blue !important; --pr-accent: #123");
        let out = inline_css(7, &t.ctls, &[]);
        assert_eq!(out.css, "[data-pr-inst=\"7\"] #BTN-GO { background-color: red; color: blue !important; --pr-accent: #123; }\n");
        assert!(!out.css.contains("@layer"), "level 6 is unlayered");
        assert_eq!(out.dropped.len(), 1, "{:?}", out.dropped);
        assert_eq!((out.dropped[0].0.as_str(), out.dropped[0].1.as_str()), ("BTN-GO", "nonsense"));
        // A control with no CssStyle writes nothing.
        assert_eq!(out.css.matches(" { ").count(), 1);
        eprintln!("inline: {} rule(s), dropped {:?}", out.css.lines().count(), out.dropped);
    }

    #[test]
    fn a_hostile_inline_value_cannot_open_a_rule_or_load_anything() {
        let mut t = tree();
        let go = t.by_name("BTN-GO").unwrap();
        t.set(go, "CssStyle", "color: red } body { color: green; background: url(http://x/y.png); width: 1px</style><script>; a{b: c; height: 2px");
        let out = inline_css(1, &t.ctls, &[]);
        assert_eq!(out.css, "[data-pr-inst=\"1\"] #BTN-GO { height: 2px; }\n", "{out:?}");
        assert_eq!(out.css.matches('{').count(), 1, "one rule, never another");
        assert!(!out.css.contains("url(") && !out.css.contains("body") && !out.css.contains("<"));
        assert_eq!(out.dropped.len(), 4, "{:?}", out.dropped);
        // An allowed origin is kept.
        t.set(go, "CssStyle", "background: url(https://cdn.example/a.png)");
        assert!(inline_css(1, &t.ctls, &["https://cdn.example".into()]).css.contains("url(https://cdn.example/a.png)"));
        eprintln!("hostile inline CSS: {} declarations dropped, one rule kept", out.dropped.len());
    }

    #[test]
    fn an_id_selector_escapes_what_an_identifier_cannot_hold() {
        assert_eq!(id_selector("BTN-GO"), "#BTN-GO");
        assert_eq!(id_selector("1ST"), "#\\31 ST");
        assert_eq!(id_selector("A\"B"), "#A\\22 B");
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
    fn a_side_menus_footer_panel_fills_the_footer_band_and_its_controls_count_from_the_band() {
        // The rail runs the form's full height (480); its footer band is the bottom 96 points. The file gives the Panel a
        // rectangle of its own (y 900) that the desktop never uses: it pins the Panel to the band.
        let t = tree_of(
            r#"<Control id="SM" type="SideMenu" x="0" y="0" w="248" h="480"><Property name="FooterHeight">96</Property></Control>
               <Control id="SM-Footer" type="Panel" x="0" y="900" w="248" h="96" parent="SM"><Property name="IsSideMenuFooter">true</Property><Property name="BorderStyle">None</Property></Control>
               <Control id="CARD" type="Panel" x="16" y="396" w="216" h="72" parent="SM-Footer"><Property name="BorderStyle">None</Property></Control>"#,
        );
        let (sm, footer, card) = (t.by_name("SM").unwrap(), t.by_name("SM-Footer").unwrap(), t.by_name("CARD").unwrap());
        assert_eq!(side_menu_footer_band(&t.ctls, footer), Some((0.0, 384.0, 248.0, 96.0)));
        let d = control_decls(&t.ctls, footer, false);
        assert_eq!((decl(&d, "position"), decl(&d, "inset"), decl(&d, "top")), (Some("absolute"), Some("0"), None), "the Panel fills the band");
        let d = control_decls(&t.ctls, card, false);
        assert_eq!((decl(&d, "left"), decl(&d, "top")), (Some("16px"), Some("12px")), "16-0, 396-384");
        assert_eq!(decl(&kind_decls(&t.ctls, sm), "--pr-sm-footer-h"), Some("96px"), "the band is FooterHeight tall");
        assert!(base_css_has_footer_height(), "the footer element takes its height from the rail");

        // A rail without a footer Panel keeps a footer that takes no room, and a Panel that is not the footer is not pinned.
        let t = tree_of(r#"<Control id="SM" type="SideMenu" x="0" y="0" w="248" h="480"></Control><Control id="P" type="Panel" x="0" y="10" w="20" h="20" parent="SM"></Control>"#);
        assert_eq!(decl(&kind_decls(&t.ctls, t.by_name("SM").unwrap()), "--pr-sm-footer-h"), None);
        assert_eq!(side_menu_footer_band(&t.ctls, t.by_name("P").unwrap()), None);
    }

    fn base_css_has_footer_height() -> bool {
        crate::base_css::BASE_CSS.contains(".pr-sm-footer { flex: none; position: relative; height: var(--pr-sm-footer-h, auto); }")
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

    /// A responsive form: a Flex root holding a header and a Grid of two cards, one of them expandable, and a layer's card.
    fn dashboard() -> crate::tree::FormTree {
        tree_of(
            r##"<Layer name="POP" color="#00000000" />
               <Control id="ROOT" type="Panel" x="0" y="0" w="640" h="480"><Property name="LayoutMode">Flex</Property><Property name="FlexDirection">Column</Property></Control>
               <Control id="HEAD" type="Panel" parent="ROOT" x="0" y="0" w="640" h="60"></Control>
               <Control id="GRID" type="Panel" parent="ROOT" x="0" y="60" w="640" h="400"><Property name="LayoutMode">Grid</Property><Property name="GridColumns">1fr 1fr</Property><Property name="GridRows">1fr</Property><Property name="Gap">20</Property></Control>
               <Control id="C-ONE" type="GroupBox" parent="GRID" x="0" y="60" w="300" h="400"><Property name="GridColumn">1</Property><Property name="GridRow">1</Property><Property name="Expandable">1</Property></Control>
               <Control id="C-TWO" type="GroupBox" parent="GRID" x="320" y="60" w="300" h="400"><Property name="GridColumn">2</Property><Property name="GridRow">1</Property><Property name="Expandable">1</Property></Control>
               <Control id="L-ONE" type="Label" parent="C-ONE" x="10" y="70" w="50" h="20"></Control>
               <Control id="L-TWO" type="Label" parent="C-TWO" x="330" y="70" w="50" h="20"></Control>
               <Control id="POP-CARD" type="GroupBox" x="0" y="500" w="100" h="100" layer="POP"><Property name="Expandable">1</Property></Control>
               <Control id="POP-OTHER" type="Panel" x="110" y="500" w="100" h="100" layer="POP"></Control>"##,
        )
    }

    #[test]
    fn an_expanded_card_fills_its_grid_and_hides_its_siblings_and_nothing_above_it_changes() {
        let mut t = dashboard();
        t.set(0, "Responsive", "1");
        let id = |t: &crate::tree::FormTree, n: &str| t.by_name(n).unwrap();
        let (root, head, grid, one, two) = ["ROOT", "HEAD", "GRID", "C-ONE", "C-TWO"].map(|n| id(&t, n)).into();
        let before: Vec<Decls> = [root, head, grid, one, two].iter().map(|&i| control_decls(&t.ctls, i, true)).collect();
        assert_eq!(decl(&before[3], "grid-column"), Some("1 / span 1"), "in its cell");
        t.set(one, "Expanded", "1");
        let after: Vec<Decls> = [root, head, grid, one, two].iter().map(|&i| control_decls(&t.ctls, i, true)).collect();
        // The card takes the grid's whole area: out of the flow, stretched edge to edge, cell given up.
        assert_eq!(decl(&after[3], "position"), Some("absolute"));
        assert_eq!(decl(&after[3], "grid-column"), Some("auto"));
        assert_eq!(decl(&after[3], "grid-row"), Some("auto"));
        assert_eq!(decl(&after[3], "z-index"), Some("900"));
        // Its sibling is not drawn; the header, the grid and the root, above it, are exactly as they were.
        assert_eq!(decl(&after[4], "display"), Some("none"));
        for (k, name) in [(0, "ROOT"), (1, "HEAD"), (2, "GRID")] {
            assert_eq!(after[k], before[k], "{name} is not expanded by the card inside it");
        }
        assert_eq!(decl(&after[2], "z-index"), decl(&before[2], "z-index"));
        // Collapsed again: everything is back.
        t.set(one, "Expanded", "0");
        let back: Vec<Decls> = [root, head, grid, one, two].iter().map(|&i| control_decls(&t.ctls, i, true)).collect();
        assert_eq!(back, before);
    }

    #[test]
    fn a_form_that_is_not_laid_out_and_a_layer_keep_their_cards_in_place() {
        let mut t = dashboard();
        let (one, two, pop, other) = ["C-ONE", "C-TWO", "POP-CARD", "POP-OTHER"].map(|n| t.by_name(n).unwrap()).into();
        t.set(one, "Expanded", "1");
        t.set(pop, "Expanded", "1");
        assert_eq!(decl(&control_decls(&t.ctls, two, false), "display"), None, "a form that is not responsive keeps its designed rectangles");
        assert_eq!(decl(&control_decls(&t.ctls, pop, true), "z-index"), None, "a card in a layer is placed by the program: Expanded changes nothing");
        assert_eq!(decl(&control_decls(&t.ctls, other, true), "display"), None, "and takes nothing from its neighbours in the layer");
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
    fn a_control_a_breakpoint_hides_takes_no_place_on_the_page() {
        let mut t = tree_of(
            r#"<Breakpoints><Breakpoint name="Medium" min-width="600" font-factor="1"><Override control="LBL-B" property="Visible">0</Override></Breakpoint></Breakpoints>
               <Control id="LBL-A" type="Label" x="0" y="0" w="50" h="20"></Control>
               <Control id="LBL-B" type="Label" x="60" y="0" w="50" h="20"></Control>"#,
        );
        let (a, b) = (t.by_name("LBL-A").unwrap(), t.by_name("LBL-B").unwrap());
        let medium = t.breakpoints[0].clone();
        let shown = |t: &crate::tree::FormTree, bp: Option<&cobolt_forms::layout::breakpoints::Breakpoint>, i: usize| decl(&control_decls(&t.effective_ctls(bp), i, true), "display") != Some("none");
        assert!(!shown(&t, Some(&medium), b), "the breakpoint's Visible override hides it, in the stylesheet, since the view is not rebuilt");
        assert!(shown(&t, None, b), "the design shows it");
        assert!(shown(&t, Some(&medium), a), "and only it");
        t.set(b, "Visible", "1");
        assert!(shown(&t, Some(&medium), b), "a program that showed it keeps it shown, as with any other property");
    }

    #[test]
    fn a_fluid_form_letters_a_control_at_the_scaled_size_fraction_and_all() {
        let mut t = tree_of(
            r##"<FormLayout FontScaling="Fluid" />
               <Control id="LBL" type="Label" x="0" y="0" w="200" h="40"><Property name="Caption">Hi</Property><Property name="FontSize">28</Property></Control>"##,
        );
        let i = t.by_name("LBL").unwrap();
        assert!(control_css(0, &t.effective_ctls(None), i, true).contains("font-size: 28px;"), "the designed size at the designed width");
        t.set_font_factor(0.85);
        let css = control_css(0, &t.effective_ctls(None), i, true);
        assert!(css.contains("font-size: 23.8px;"), "28 at 0.85 keeps its fraction: {css}");
        let row = fmt_number(metrics::row_height(23.8));
        assert!(css.contains(&format!("line-height: {row}px;")), "the row is the desktop's for that size: {css}");
    }

    #[test]
    fn a_card_that_is_only_a_floor_gets_an_empty_first_child_that_carries_it() {
        let mut t = tree();
        t.set(0, "LayoutMode", "Flex");
        t.set(0, "FlexWrap", "Wrap");
        let p = t.by_name("PNL-1").unwrap();
        let css = control_css(0, &t.ctls, p, true);
        assert!(css.contains("min-height: 200px;"), "the designed height is the floor: {css}");
        assert!(
            css.contains("[data-pr-inst=\"0\"] [data-name=\"PNL-1\"]::before { content: \"\"; display: block; width: 0; height: 196px; }"),
            "the same height as content, less the 2 + 2 px the box takes first: {css}"
        );
        let label = t.by_name("LBL-IN").unwrap();
        assert!(!control_css(0, &t.ctls, label, true).contains("::before"), "only a card has children to measure");
        t.set(0, "LayoutMode", "Absolute");
        assert!(!control_css(0, &t.ctls, p, true).contains("::before"), "a card with a designed rectangle has no floor");
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
        assert_eq!(
            (decl(&ln, "--pr-line-color"), decl(&ln, "--pr-line-width"), decl(&ln, "--pr-line-dash")),
            (Some("rgb(255 0 0)"), Some("3px"), Some("15px 12px")),
            "the desktop's dash: 5t on, 4t off"
        );
        assert_eq!(decl(&ln, "border-top"), None, "the stroke is the SVG's, not a border");
    }

    /// Fix 2.0.298: a Line has no face on the page, as the desktop's Line block draws none — its
    /// BackgroundColor, gradient, corner radius and border stay off the page, whatever their values.
    #[test]
    fn a_line_paints_its_stroke_and_no_face() {
        let t = tree_of(
            r##"<Control id="LN" type="Line" x="0" y="160" w="344" h="4"><Property name="BackgroundColor">#36383EFF</Property><Property name="BackgroundGradientEnabled">true</Property><Property name="BackgroundGradientStartColor">#4E4E4EFF</Property><Property name="BackgroundGradientEndColor">#000000FF</Property><Property name="CornerRadius">10</Property><Property name="BorderStyle">Single</Property><Property name="LineColor">#FFFFFFFF</Property><Property name="LineAngle">137</Property><Property name="RoundedEnds">true</Property></Control>"##,
        );
        let ln = control_decls(&t.ctls, t.by_name("LN").unwrap(), false);
        for k in ["background", "background-color", "background-image", "border-radius", "border", "box-shadow"] {
            assert_eq!(decl(&ln, k), None, "{k} is the face's, and a Line has none");
        }
        assert_eq!((decl(&ln, "--pr-line-width"), decl(&ln, "--pr-line-cap")), (Some("1px"), Some("round")));
        println!("Line: no face (6 face declarations absent), stroke 1px with round ends");
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
        assert!(css.contains("[data-pr-inst=\"1\"] > .pr-form > .pr-surface { width: 640px; height: 480px; }"), "{css}");
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

    /// A form with a base control at `ZOrder` 209 and two layers: a translucent one holding a card (`ZOrder` 0, with a
    /// label inside it and a DataGrid), and an opaque one.
    fn layered() -> crate::tree::FormTree {
        layered_with("")
    }

    /// The layered fixture with `extra` controls added.
    fn layered_with(extra: &str) -> crate::tree::FormTree {
        let xml = format!(
            r##"<?xml version="1.0" encoding="UTF-8"?>
<Form name="main-form" title="Layers" width="640" height="480" index-base="1" corner-radius="16">
  <Layer name="LYR-POP" color="#10203080" transparency="0"/>
  <Layer name="LYR-TOP" color="#203040"/>
  <Control id="BTN-BASE" type="Button" x="10" y="10" w="80" h="30" z-order="209"></Control>
  <Control id="PNL-POP" type="Panel" x="100" y="100" w="200" h="200" z-order="0" layer="LYR-POP"></Control>
  <Control id="LBL-IN" type="Label" x="10" y="10" w="80" h="20" parent="PNL-POP"></Control>
  <Control id="DG-1" type="DataGrid" x="0" y="400" w="200" h="80" layer="LYR-POP"></Control>
{extra}</Form>"##
        );
        let form = cobolt_forms::xml::load_form_from_str(&xml).unwrap();
        let (_, mut manifest) = crate::tree::testing::demo();
        manifest.forms[0].controls.clear();
        manifest.forms[0].handlers.clear();
        crate::tree::FormTree::build(&form, &manifest.forms[0], &manifest, 0, None).unwrap()
    }

    /// The rule `layers_css` writes for layer `k`'s backdrop.
    fn backdrop_rule(css: &str, k: usize) -> &str {
        let sel = format!("[data-pr-inst=\"0\"] > .pr-form-layer[data-layer-index=\"{k}\"] > .pr-layer-bg {{");
        let at = css.find(&sel).unwrap_or_else(|| panic!("no backdrop rule for layer {k}: {css}"));
        let rest = &css[at..];
        &rest[..=rest.find('}').unwrap()]
    }

    /// WASM-004 T19, R16 and Q7 natively: only an opaque backdrop takes the pointer, by the model's `Layer::is_opaque`.
    #[test]
    fn layer_hit_an_opaque_backdrop_takes_the_pointer_and_a_see_through_one_does_not() {
        let mut t = layered();
        let css = layers_css(0, &t.layers, &BTreeMap::new());
        let (pop, top) = (backdrop_rule(&css, 0), backdrop_rule(&css, 1));
        println!("{pop}\n{top}");
        assert!(!pop.contains("pointer-events"), "LYR-POP is #10203080, see-through: the event passes below ({pop})");
        assert!(top.contains("pointer-events: auto"), "LYR-TOP is #203040, opaque: it keeps the event ({top})");
        assert!(crate::base_css::BASE_CSS.contains(".pr-layer-bg { position: absolute; inset: 0; pointer-events: none; }"), "a backdrop takes nothing unless it is opaque");
        // The program fades the opaque layer: it lets the pointer through from the next frame on.
        t.layer_set(1, "Transparency", "10").unwrap();
        let css = layers_css(0, &t.layers, &BTreeMap::new());
        assert!(!backdrop_rule(&css, 1).contains("pointer-events"), "{css}");
    }

    /// WASM-004 T19, R16 natively: in a layer, a Label that paints no face is hit on its text, its box lets the pointer
    /// through; a Label with a face, and any Label in the base, keeps its whole box (the desktop's `paints_at`).
    #[test]
    fn layer_hit_a_see_through_label_in_a_layer_is_hit_on_its_text_only() {
        let t = layered_with(
            r##"  <Control id="LBL-SEE" type="Label" x="400" y="10" w="80" h="20" layer="LYR-TOP"><Property name="BackgroundColor">#00000000</Property><Property name="Caption">see</Property></Control>
  <Control id="LBL-FACE" type="Label" x="400" y="40" w="80" h="20" layer="LYR-TOP"><Property name="BackgroundColor">#FFFFFF</Property></Control>
  <Control id="LBL-GONE" type="Label" x="400" y="70" w="80" h="20" layer="LYR-TOP"><Property name="Transparency">100</Property></Control>
  <Control id="LBL-BASE" type="Label" x="400" y="100" w="80" h="20"><Property name="BackgroundColor">#00000000</Property></Control>
"##,
        );
        let empty = crate::view::Empty::default();
        let view = |name: &str| crate::view::controls::view(&empty.cx(&t), t.by_name(name).unwrap());
        let see = view("LBL-SEE");
        assert_eq!(see.attr_of("data-hit"), Some("text"));
        let text = see.find(&|n| n.has_class("pr-label-text")).expect("the text is a span of its own");
        assert_eq!(text.text.as_deref(), Some("see"));
        assert_eq!(view("LBL-GONE").attr_of("data-hit"), Some("text"), "Transparency 100 paints no face either");
        for name in ["LBL-FACE", "LBL-BASE"] {
            let n = view(name);
            assert_eq!(n.attr_of("data-hit"), None, "{name} is hit on its whole box");
            assert!(n.find(&|c| c.has_class("pr-label-text")).is_none(), "{name}");
        }
        let sheet = crate::base_css::BASE_CSS;
        assert!(sheet.contains(".pr-form-layer .pr-label[data-hit=\"text\"] { pointer-events: none; }"));
        assert!(sheet.contains(".pr-form-layer .pr-label[data-hit=\"text\"] > .pr-label-text { pointer-events: auto; }"));
    }

    /// WASM-004 T19, plan §1.4's working assumption: while an opaque layer is shown, the base and the layers under it are
    /// `inert` (the desktop's `opaque_shield_rank`); a see-through layer shields nothing.
    #[test]
    fn layer_hit_the_shield_toggles_with_the_opaque_layers_visible() {
        let mut t = layered();
        assert_eq!(layer_reach(&t.layers), (false, vec![(false, true), (false, true)]), "at open: every layer hidden and out of reach");
        t.layer_set(0, "Visible", "true").unwrap();
        assert_eq!(layer_reach(&t.layers), (false, vec![(true, false), (false, true)]), "the see-through LYR-POP shields nothing");
        t.layer_set(1, "Visible", "true").unwrap();
        assert_eq!(layer_reach(&t.layers), (true, vec![(true, true), (true, false)]), "the opaque LYR-TOP shields the base and LYR-POP");
        t.layer_set(1, "Visible", "false").unwrap();
        assert_eq!(layer_reach(&t.layers), (false, vec![(true, false), (false, true)]), "hidden again, the shield goes with it");
    }

    /// WASM-004 T21, R24: a form without layers gets nothing of them — no layer rules, no shield, no control mounted in a
    /// layer, no Label hit on its text only — and its own rules differ from before only by the root wrapper's selector.
    #[test]
    fn layers_a_form_without_layers_is_drawn_as_before_but_for_the_root_wrapper() {
        let t = crate::tree::testing::tree();
        assert!(t.layers.is_empty());
        assert_eq!(layers_css(1, &t.layers, &BTreeMap::new()), "");
        assert_eq!(layer_reach(&t.layers), (false, vec![]));
        let e = crate::view::Empty::default();
        for i in 1..t.ctls.len() {
            assert!(!matches!(mount_of(&t.ctls, i).1, crate::view::Slot::Layer(_)), "{}", t.ctls[i].name);
            assert_eq!(crate::view::controls::view(&e.cx(&t), i).attr_of("data-hit"), None, "{}", t.ctls[i].name);
        }
        let css = form_css(1, &t.ctls, false, &BTreeMap::new());
        let before = css.replace("[data-pr-inst=\"1\"] > .pr-form > .pr-surface", "[data-pr-inst=\"1\"] > .pr-surface");
        println!("expectations changed by the wrapper:\n- [data-pr-inst=\"1\"] > .pr-surface\n+ [data-pr-inst=\"1\"] > .pr-form > .pr-surface");
        assert_ne!(css, before, "the surface is reached through the base");
        assert_eq!(css.matches(".pr-form").count(), 1, "and nothing else names the base: {css}");
    }

    /// WASM-004 T20, R20 and AC12 natively: the document order is the keyboard order — the base's controls by
    /// `TabOrder`, then each layer upward by its own — as the desktop walks them (layer, then tab order); the stacking
    /// stays `ZOrder`'s, by `z-index`.
    #[test]
    fn layers_the_focus_order_is_the_document_order_base_then_each_layer_by_tab_order() {
        let xml_extra = r##"  <Control id="TXT-B3" type="TextBox" x="10" y="60" w="80" h="24" tab-order="3" z-order="1"></Control>
  <Control id="TXT-B1" type="TextBox" x="10" y="90" w="80" h="24" tab-order="1" z-order="2"></Control>
  <Control id="TXT-B2" type="TextBox" x="10" y="120" w="80" h="24" tab-order="2" z-order="3"></Control>
  <Control id="TXT-T2" type="TextBox" x="400" y="60" w="80" h="24" tab-order="2" layer="LYR-TOP"></Control>
  <Control id="TXT-T1" type="TextBox" x="400" y="90" w="80" h="24" tab-order="1" layer="LYR-TOP"></Control>
  <Control id="TXT-P1" type="TextBox" x="10" y="200" w="80" h="24" tab-order="1" layer="LYR-POP"></Control>
"##;
        let t = layered_with(xml_extra);
        let names = |pick: &dyn Fn(&str) -> bool| -> Vec<String> {
            mount_order(&t.ctls).into_iter().filter(|&i| t.ctls[i].parent.unwrap_or(0) == 0).map(|i| t.ctls[i].name.clone()).filter(|n| pick(n)).collect()
        };
        let order = names(&|n| n.starts_with("TXT-"));
        assert_eq!(order, ["TXT-B1", "TXT-B2", "TXT-B3", "TXT-P1", "TXT-T1", "TXT-T2"], "the base by TabOrder, then LYR-POP, then LYR-TOP");
        // The same order as the model's: (layer rank, tab order, index), the desktop's sort.
        let form = cobolt_forms::xml::load_form_from_str(&format!(
            "<Form name=\"f\"><Layer name=\"LYR-POP\"/><Layer name=\"LYR-TOP\"/>{xml_extra}</Form>"
        ))
        .unwrap();
        let mut model: Vec<(usize, u32, usize, String)> =
            form.controls.iter().enumerate().map(|(k, c)| (form.layer_rank(form.layer_of(&c.id)), c.tab_order, k, c.id.clone())).collect();
        model.sort();
        assert_eq!(order, model.into_iter().map(|m| m.3).collect::<Vec<_>>());
        // The stacking is ZOrder's: TXT-B3 is first in ZOrder and last in tab order.
        let z = |n: &str| decl(&control_decls(&t.ctls, t.by_name(n).unwrap(), false), "z-index").map(str::to_owned);
        assert_eq!((z("TXT-B3"), z("TXT-B1"), z("TXT-B2")), (Some("1".into()), Some("2".into()), Some("3".into())));
    }

    /// WASM-004 T18, AC9 natively: a layer control with `ZOrder` 0 sits in a stacking context above a base control with
    /// `ZOrder` 209, whatever their own `z-index`.
    #[test]
    fn layers_a_layer_control_with_zorder_0_stacks_above_a_base_control_with_zorder_209() {
        let t = layered();
        let (base, pop, inside, grid) = (t.by_name("BTN-BASE").unwrap(), t.by_name("PNL-POP").unwrap(), t.by_name("LBL-IN").unwrap(), t.by_name("DG-1").unwrap());
        assert_eq!(mount_of(&t.ctls, base), (0, crate::view::Slot::Children), "the base control is on the base's surface");
        assert_eq!(mount_of(&t.ctls, pop), (0, crate::view::Slot::Layer(0)), "the card is in its layer's element");
        assert_eq!(mount_of(&t.ctls, grid), (0, crate::view::Slot::Layer(0)), "AC13: the DataGrid is inside the layer, inside the root");
        assert_eq!(mount_of(&t.ctls, inside).0, pop, "a container's contents stay in the container");
        assert!(control_css(0, &t.ctls, base, false).contains("z-index: 209"));
        let layers = layers_css(0, &t.layers, &BTreeMap::new());
        println!("{layers}");
        assert!(layers.contains("[data-pr-inst=\"0\"] > .pr-form-layer[data-layer-index=\"0\"] { z-index: 1; }"), "{layers}");
        assert!(layers.contains("[data-pr-inst=\"0\"] > .pr-form-layer[data-layer-index=\"1\"] { z-index: 2; }"), "{layers}");
        // The base is a stacking context at 0 and each layer one of its own, so 209 is compared only inside the base.
        let sheet = crate::base_css::BASE_CSS;
        assert!(sheet.contains(".pr-form-root > .pr-form { z-index: 0; }"));
        assert!(sheet.contains(".pr-form-layer { position: absolute; inset: 0; isolation: isolate; pointer-events: none; }"));
        let context = |i: usize| t.layer_of(i).map_or(0, |l| l + 1);
        assert!((context(pop), 0) > (context(base), 209), "(layer, ZOrder) of the card is above the base button's");
    }

    /// WASM-004 T18, AC10 and AC13 natively, R17, R18, Q8: every layer starts hidden and a `Visible` write shows it on the
    /// next pass; the root clips with the form's radius; the backdrop's alpha is its `Transparency`, never an opacity; the
    /// toast stack is above every layer.
    #[test]
    fn layers_start_hidden_clip_to_the_root_and_take_their_transparency_as_alpha() {
        let mut t = layered();
        assert!(t.layers.iter().all(|l| !l.visible), "hidden at the start (R15)");
        t.take_changes();
        t.layer_set(0, "Visible", "TRUE").unwrap();
        assert!(t.layers[0].visible);
        let changes = t.take_changes();
        assert!(changes.iter().any(|c| matches!(c, crate::tree::Change::Prop { ctl: 0, .. })), "the form's rules (and the layers' flags) are recomputed on the next pass");
        // The root: clips always, with the form's own radius (R17).
        let sheet = crate::base_css::BASE_CSS;
        assert!(sheet.contains(".pr-form-root { position: absolute; inset: 0; overflow: hidden; isolation: isolate; }"));
        assert!(sheet.contains("[hidden] { display: none !important; }"));
        let form = form_css(0, &t.ctls, false, &BTreeMap::new());
        assert!(form.contains("[data-pr-inst=\"0\"] {") && form.contains("border-radius: 16px"), "{form}");
        // The backdrop of a layer at 50 %: alpha 0x80 on its colour, and no opacity anywhere (R18).
        t.layer_set(1, "Transparency", "50").unwrap();
        let bg = layer_backdrop_decls(&live_layer(&t.layers[1]), &BTreeMap::new());
        println!("{bg:?}");
        assert_eq!(bg, vec![("background-color".to_owned(), "rgb(32 48 64 / 0.502)".to_owned())]);
        assert!(!layers_css(0, &t.layers, &BTreeMap::new()).contains("opacity"));
        // A gradient backdrop fades its stops the same way.
        t.layer_set(1, "BackgroundGradientEnabled", "TRUE").unwrap();
        t.layer_set(1, "BackgroundGradientStartColor", "#FF0000").unwrap();
        t.layer_set(1, "BackgroundGradientEndColor", "#0000FF").unwrap();
        let bg = layer_backdrop_decls(&live_layer(&t.layers[1]), &BTreeMap::new());
        assert_eq!(bg, vec![("background-image".to_owned(), "linear-gradient(180deg, rgb(255 0 0 / 0.502), rgb(0 0 255 / 0.502))".to_owned())]);
        // Q8: the notifications are above every layer: the root isolates its layers, and the toast host is the application's.
        assert!(sheet.contains(".pr-toasts-host { position: absolute; left: 12px; bottom: 12px; z-index: 40;"));
        // R24: a form without layers has no layer rules.
        assert_eq!(layers_css(0, &[], &BTreeMap::new()), "");
    }
}
