// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! The reverse table (WASM-036 R2, R3; plan §1.2): what a CSS declaration says about the model's
//! properties, and the CSS text a property value is written as. The forward direction is the web
//! player's (`cobolt-web-player/src/css.rs`); this is its inverse, one row per CSS property of
//! [`crate::css_overrides::CSS_TO_PROPERTIES`]. No egui and no `render` feature.
//!
//! A value the table cannot turn into model values is [`Reading::Unknown`] and keeps its CSS text:
//! `%`, `vw`, `vh`, `calc()`, `env()`, an unresolved `var()`, `url()`, a gradient the model lacks,
//! several shadows, `inherit`, `unset`, `revert`. It is never guessed.

use crate::css::{color, color_name, length, split_top, tokens};
use crate::model::ControlType;

/// How faithfully a row turns CSS back into the model's properties.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Fidelity {
    /// The same value goes back and forth.
    Exact,
    /// The model keeps the part of the value it has a property for (the first family of a list,
    /// the nearest of eight directions of a shadow).
    Approximate,
    /// The model has nothing to hold it: read as `Unknown`, never written.
    NotReversible,
}

/// A model value read from CSS, in the units the model uses.
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Color([u8; 4]),
    /// A length in CSS pixels, or a plain number (`ShadowOpacity`, `ZOrder`).
    Num(f32),
    Bool(bool),
    /// A name: a font family, a cursor, an alignment, a border style, a direction.
    Text(String),
}

impl Value {
    /// The text the model stores for it (`#rrggbbaa`, `14`, `1`/`0`, the name).
    pub fn to_property_text(&self) -> String {
        match self {
            Value::Color([r, g, b, a]) => format!("#{r:02x}{g:02x}{b:02x}{a:02x}"),
            Value::Num(n) => format!("{}", (*n * 100.0).round() / 100.0),
            Value::Bool(b) => (if *b { "1" } else { "0" }).to_owned(),
            Value::Text(t) => t.clone(),
        }
    }
}

/// What reading one declaration gives.
#[derive(Debug, Clone, PartialEq)]
pub enum Reading {
    /// The model properties the declaration sets, with their values.
    Known(Vec<(&'static str, Value)>),
    /// The model cannot hold it; the CSS text, as written.
    Unknown(String),
}

/// What reading a value depends on.
#[derive(Debug, Clone)]
pub struct Ctx {
    pub control_type: ControlType,
    /// The control's effective font size in px: what `em` is.
    pub font_px: f32,
    /// The player's root font size: what `rem` is.
    pub root_px: f32,
}

impl Ctx {
    pub fn new(control_type: ControlType) -> Self {
        Ctx { control_type, font_px: 14.0, root_px: 16.0 }
    }
}

/// One row of the reverse table.
#[derive(Debug, Clone, Copy)]
pub struct Row {
    pub css: &'static str,
    pub fidelity: Fidelity,
    /// `true`: [`write`] / [`write_decoration`] / [`write_shadow`] can put the model's value back.
    /// Shorthands, `z-index` and `display` are never written (R10, R16).
    pub writable: bool,
}

const fn row(css: &'static str, fidelity: Fidelity, writable: bool) -> Row {
    Row { css, fidelity, writable }
}

/// One row per CSS property of `CSS_TO_PROPERTIES`, in the same order (a test holds them to it).
pub const REVERSE: &[Row] = &[
    row("background-color", Fidelity::Exact, true),
    row("background", Fidelity::Exact, false),
    row("background-image", Fidelity::Exact, false),
    row("color", Fidelity::Exact, true),
    row("font-family", Fidelity::Approximate, true),
    row("font-size", Fidelity::Exact, true),
    row("font-weight", Fidelity::Exact, true),
    row("font-style", Fidelity::Exact, true),
    row("font", Fidelity::Approximate, false),
    row("text-decoration", Fidelity::Exact, true),
    row("text-decoration-line", Fidelity::Exact, true),
    row("cursor", Fidelity::Exact, true),
    row("padding", Fidelity::Exact, true),
    row("padding-left", Fidelity::Exact, true),
    row("padding-top", Fidelity::Exact, true),
    row("padding-right", Fidelity::Exact, true),
    row("padding-bottom", Fidelity::Exact, true),
    row("border-radius", Fidelity::Exact, true),
    row("border-top-left-radius", Fidelity::Exact, true),
    row("border-top-right-radius", Fidelity::Exact, true),
    row("border-bottom-left-radius", Fidelity::Exact, true),
    row("border-bottom-right-radius", Fidelity::Exact, true),
    row("border", Fidelity::Exact, false),
    row("border-style", Fidelity::Exact, true),
    row("border-color", Fidelity::Exact, true),
    row("border-width", Fidelity::Exact, true),
    row("box-shadow", Fidelity::Approximate, true),
    row("text-align", Fidelity::Exact, true),
    row("justify-content", Fidelity::Approximate, false),
    row("align-items", Fidelity::Approximate, false),
    row("white-space", Fidelity::Exact, true),
    row("transform", Fidelity::NotReversible, false),
    row("z-index", Fidelity::Exact, false),
    row("display", Fidelity::Exact, false),
    row("backdrop-filter", Fidelity::NotReversible, false),
];

/// The row of `css`.
pub fn row_of(css: &str) -> Option<&'static Row> {
    REVERSE.iter().find(|r| r.css.eq_ignore_ascii_case(css))
}

impl Row {
    /// The fidelity on a control type: padding is only approximate where the player adds its own
    /// insets (Label, Button, TextBox), `text-align` on a Button, `white-space` off a Label.
    pub fn fidelity_on(&self, t: &ControlType) -> Fidelity {
        let insets = matches!(t, ControlType::Label | ControlType::Button | ControlType::TextBox);
        match self.css {
            "padding" | "padding-left" | "padding-top" | "padding-right" | "padding-bottom" if insets => Fidelity::Approximate,
            "text-align" if !matches!(t, ControlType::Label | ControlType::TextBox) => Fidelity::Approximate,
            "white-space" if !matches!(t, ControlType::Label) => Fidelity::Approximate,
            _ => self.fidelity,
        }
    }
}

// ── Reading ───────────────────────────────────────────────────────────────────────────────────

fn unknown(v: &str) -> Reading {
    Reading::Unknown(v.trim().to_owned())
}

/// A value no row reads: another function that needs the page, or a CSS-wide keyword.
fn never_known(v: &str) -> bool {
    let l = v.trim().to_ascii_lowercase();
    ["inherit", "unset", "revert", "revert-layer", "initial"].contains(&l.as_str()) || l.contains("var(") || l.contains("env(") || l.contains("url(")
}

fn px(v: &str, ctx: &Ctx) -> Option<f32> {
    // `rem` is the player's root size, not `length`'s fixed 16.
    let t = v.trim().to_ascii_lowercase();
    if let Some(n) = t.strip_suffix("rem") {
        return n.trim().parse::<f32>().ok().map(|n| n * ctx.root_px);
    }
    length(&t, ctx.font_px, None)
}

const DIRECTIONS: [(&str, i32, i32); 8] =
    [("North", 0, -1), ("NorthEast", 1, -1), ("East", 1, 0), ("SouthEast", 1, 1), ("South", 0, 1), ("SouthWest", -1, 1), ("West", -1, 0), ("NorthWest", -1, -1)];

/// The CSS angle of a gradient direction (the direction it runs toward), as the player writes it.
fn direction_of_angle(deg: f32) -> Option<&'static str> {
    let a = deg.rem_euclid(360.0);
    let i = (a / 45.0).round();
    ((a - i * 45.0).abs() < 0.01).then(|| ["North", "NorthEast", "East", "SouthEast", "South", "SouthWest", "West", "NorthWest"][(i as usize) % 8])
}

fn angle_of_direction(name: &str) -> f32 {
    ["North", "NorthEast", "East", "SouthEast", "South", "SouthWest", "West", "NorthWest"].iter().position(|d| d.eq_ignore_ascii_case(name)).map_or(180.0, |i| i as f32 * 45.0)
}

/// `linear-gradient(<angle>?, <colour>, <colour>)` of two plain stops at one of the eight angles.
fn gradient(v: &str) -> Option<Vec<(&'static str, Value)>> {
    let t = v.trim();
    let inner = t.strip_prefix("linear-gradient(").or_else(|| t.strip_prefix("LINEAR-GRADIENT("))?.strip_suffix(')')?;
    let parts: Vec<&str> = split_top(inner, ',').into_iter().map(str::trim).collect();
    let (dir, stops) = match parts.len() {
        2 => ("South", &parts[..]),
        3 => {
            let a = parts[0].to_ascii_lowercase();
            let dir = if let Some(d) = a.strip_suffix("deg") {
                direction_of_angle(d.trim().parse().ok()?)?
            } else {
                match a.as_str() {
                    "to top" => "North",
                    "to top right" | "to right top" => "NorthEast",
                    "to right" => "East",
                    "to bottom right" | "to right bottom" => "SouthEast",
                    "to bottom" => "South",
                    "to bottom left" | "to left bottom" => "SouthWest",
                    "to left" => "West",
                    "to top left" | "to left top" => "NorthWest",
                    _ => return None,
                }
            };
            (dir, &parts[1..])
        }
        _ => return None,
    };
    Some(vec![
        ("BackgroundGradientEnabled", Value::Bool(true)),
        ("BackgroundGradientStartColor", Value::Color(color(stops[0])?)),
        ("BackgroundGradientEndColor", Value::Color(color(stops[1])?)),
        ("BackgroundGradientDirection", Value::Text(dir.to_owned())),
    ])
}

/// 1–4 values as `[top, right, bottom, left]`.
fn four<T: Clone>(v: &[T]) -> Option<[T; 4]> {
    match v {
        [a] => Some([a.clone(), a.clone(), a.clone(), a.clone()]),
        [a, b] => Some([a.clone(), b.clone(), a.clone(), b.clone()]),
        [a, b, c] => Some([a.clone(), b.clone(), c.clone(), b.clone()]),
        [a, b, c, d] => Some([a.clone(), b.clone(), c.clone(), d.clone()]),
        _ => None,
    }
}

fn border_style(t: &str) -> Option<&'static str> {
    Some(match t.to_ascii_lowercase().as_str() {
        "solid" => "Single",
        "dashed" => "Dashed",
        "dotted" => "Dotted",
        "double" => "Double",
        "none" | "hidden" => "None",
        "outset" => "Raised",
        "inset" => "Sunken",
        _ => return None,
    })
}

fn align_name(v: &str) -> Option<&'static str> {
    Some(match v.trim().to_ascii_lowercase().as_str() {
        "left" | "start" | "flex-start" => "Left",
        "center" => "Center",
        "right" | "end" | "flex-end" => "Right",
        _ => return None,
    })
}

fn cursor_name(v: &str) -> Option<&'static str> {
    Some(match v.trim().to_ascii_lowercase().as_str() {
        "default" | "auto" => "Default",
        "pointer" => "Hand",
        "text" => "Text",
        "wait" => "Wait",
        "crosshair" => "Crosshair",
        "not-allowed" => "No",
        "move" => "SizeAll",
        "ns-resize" => "SizeNS",
        "ew-resize" => "SizeWE",
        "help" => "Help",
        _ => return None,
    })
}

fn weight(v: &str) -> Option<bool> {
    let l = v.trim().to_ascii_lowercase();
    match l.as_str() {
        "bold" | "bolder" => Some(true),
        "normal" | "lighter" => Some(false),
        n => n.parse::<f32>().ok().map(|n| n >= 600.0),
    }
}

/// One box-shadow: `[inset] <dx> <dy> [<blur> [<spread>]] [<colour>]`, the colour first or last.
fn shadow(v: &str, ctx: &Ctx) -> Option<Vec<(&'static str, Value)>> {
    let mut inset = false;
    let mut nums: Vec<f32> = Vec::new();
    let mut col: Option<[u8; 4]> = None;
    for t in split_top_ws(v) {
        if t.eq_ignore_ascii_case("inset") {
            inset = true;
        } else if let Some(c) = color(&t) {
            col = Some(c);
        } else {
            nums.push(px(&t, ctx)?);
        }
    }
    if nums.len() < 2 || nums.len() > 4 {
        return None;
    }
    let (dx, dy, blur) = (nums[0], nums[1], nums.get(2).copied().unwrap_or(0.0));
    let c = col.unwrap_or([0, 0, 0, 255]);
    let dir = DIRECTIONS
        .iter()
        .max_by(|a, b| {
            let score = |d: &(&str, i32, i32)| {
                let n = ((d.1 * d.1 + d.2 * d.2) as f32).sqrt();
                (d.1 as f32 * dx + d.2 as f32 * dy) / n
            };
            score(a).total_cmp(&score(b))
        })
        .map_or("South", |d| d.0);
    let strength = blur / 2.0;
    Some(vec![
        ("ShadowEnabled", Value::Bool(true)),
        ("ShadowDirection", Value::Text(dir.to_owned())),
        ("ShadowDistance", Value::Num((dx * dx + dy * dy).sqrt())),
        ("ShadowBlur", Value::Bool(blur != 0.0)),
        ("ShadowBlurStrength", Value::Num(if inset { -strength } else { strength })),
        ("ShadowColor", Value::Color([c[0], c[1], c[2], 255])),
        ("ShadowOpacity", Value::Num((c[3] as f32 / 255.0 * 100.0).round())),
    ])
}

/// Whitespace-separated tokens, keeping `rgb(…)` and the like whole.
fn split_top_ws(s: &str) -> Vec<String> {
    tokens(s).into_iter().map(str::to_owned).collect()
}

/// The model properties a declaration sets, or `Unknown`.
pub fn read(css: &str, value: &str, ctx: &Ctx) -> Reading {
    let css = css.trim().to_ascii_lowercase();
    let v = value.trim();
    let Some(r) = row_of(&css) else { return unknown(v) };
    if r.fidelity == Fidelity::NotReversible || never_known(v) {
        return unknown(v);
    }
    let known = |p: Option<Vec<(&'static str, Value)>>| p.map_or_else(|| unknown(v), Reading::Known);
    let lower = v.to_ascii_lowercase();
    known(match css.as_str() {
        "background-color" => color(v).map(|c| vec![("BackgroundColor", Value::Color(c))]),
        "color" => color(v).map(|c| vec![("ForegroundColor", Value::Color(c))]),
        "border-color" => {
            let cs: Option<Vec<[u8; 4]>> = tokens(v).into_iter().map(color).collect();
            cs.filter(|c| c.len() == 1 || c.iter().all(|x| *x == c[0])).map(|c| vec![("BorderColor", Value::Color(c[0]))])
        }
        "background" => {
            if let Some(g) = gradient(v) {
                Some(g)
            } else {
                color(v).map(|c| vec![("BackgroundColor", Value::Color(c)), ("BackgroundGradientEnabled", Value::Bool(false))])
            }
        }
        "background-image" => {
            if lower == "none" {
                Some(vec![("BackgroundGradientEnabled", Value::Bool(false))])
            } else {
                gradient(v)
            }
        }
        "font-family" => {
            let first = split_top(v, ',').into_iter().next().map(|f| f.trim().trim_matches(['"', '\'']).trim().to_owned()).filter(|f| !f.is_empty());
            first.map(|f| vec![("FontName", Value::Text(f))])
        }
        "font-size" => px(v, ctx).map(|n| vec![("FontSize", Value::Num(n))]),
        "font-weight" => weight(v).map(|b| vec![("Bold", Value::Bool(b))]),
        "font-style" => match lower.as_str() {
            "italic" | "oblique" => Some(vec![("Italic", Value::Bool(true))]),
            "normal" => Some(vec![("Italic", Value::Bool(false))]),
            _ => None,
        },
        "font" => font_shorthand(v, ctx),
        "text-decoration" | "text-decoration-line" => {
            let t = tokens(&lower);
            let (u, s) = (t.contains(&"underline"), t.contains(&"line-through"));
            (u || s || t.contains(&"none")).then(|| vec![("Underline", Value::Bool(u)), ("Strikethrough", Value::Bool(s))])
        }
        "cursor" => cursor_name(v).map(|n| vec![("Cursor", Value::Text(n.to_owned()))]),
        "padding" => {
            let vals: Option<Vec<f32>> = tokens(v).into_iter().map(|t| px(t, ctx)).collect();
            vals.and_then(|vals| {
                if vals.len() == 1 {
                    return Some(vec![("Padding", Value::Num(vals[0]))]);
                }
                let [t, r, b, l] = four(&vals)?;
                Some(vec![("PaddingTop", Value::Num(t)), ("PaddingRight", Value::Num(r)), ("PaddingBottom", Value::Num(b)), ("PaddingLeft", Value::Num(l))])
            })
        }
        "padding-left" | "padding-top" | "padding-right" | "padding-bottom" => {
            let p = match css.as_str() {
                "padding-left" => "PaddingLeft",
                "padding-top" => "PaddingTop",
                "padding-right" => "PaddingRight",
                _ => "PaddingBottom",
            };
            px(v, ctx).map(|n| vec![(p, Value::Num(n))])
        }
        "border-radius" => {
            let vals: Option<Vec<f32>> = tokens(v).into_iter().map(|t| px(t, ctx)).collect();
            vals.and_then(|vals| {
                if vals.len() == 1 {
                    return Some(vec![("CornerRadius", Value::Num(vals[0]))]);
                }
                if !matches!(ctx.control_type, ControlType::Snackbar) {
                    return None;
                }
                let [tl, tr, br, bl] = four(&vals)?;
                Some(vec![
                    ("CornerRadiusTopLeft", Value::Num(tl)),
                    ("CornerRadiusTopRight", Value::Num(tr)),
                    ("CornerRadiusBottomRight", Value::Num(br)),
                    ("CornerRadiusBottomLeft", Value::Num(bl)),
                ])
            })
        }
        "border-top-left-radius" | "border-top-right-radius" | "border-bottom-left-radius" | "border-bottom-right-radius" => {
            if !matches!(ctx.control_type, ControlType::Snackbar) {
                return unknown(v);
            }
            let p = match css.as_str() {
                "border-top-left-radius" => "CornerRadiusTopLeft",
                "border-top-right-radius" => "CornerRadiusTopRight",
                "border-bottom-left-radius" => "CornerRadiusBottomLeft",
                _ => "CornerRadiusBottomRight",
            };
            px(v, ctx).map(|n| vec![(p, Value::Num(n))])
        }
        "border" => {
            let (mut w, mut s, mut c) = (None, None, None);
            for t in tokens(v) {
                if let Some(st) = border_style(t) {
                    s = Some(st);
                } else if let Some(col) = color(t) {
                    c = Some(col);
                } else if let Some(n) = px(t, ctx) {
                    w = Some(n);
                } else {
                    return unknown(v);
                }
            }
            let mut out = Vec::new();
            if let Some(w) = w {
                out.push(("BorderWidth", Value::Num(w)));
            }
            if let Some(s) = s {
                out.push(("BorderStyle", Value::Text(s.to_owned())));
            }
            if let Some(c) = c {
                out.push(("BorderColor", Value::Color(c)));
            }
            (!out.is_empty()).then_some(out)
        }
        "border-style" => {
            let st: Option<Vec<&str>> = tokens(v).into_iter().map(border_style).collect();
            st.filter(|s| s.iter().all(|x| *x == s[0])).map(|s| vec![("BorderStyle", Value::Text(s[0].to_owned()))])
        }
        "border-width" => {
            let w: Option<Vec<f32>> = tokens(v).into_iter().map(|t| px(t, ctx)).collect();
            w.filter(|w| !w.is_empty() && w.iter().all(|x| *x == w[0])).map(|w| vec![("BorderWidth", Value::Num(w[0]))])
        }
        "box-shadow" => {
            if lower == "none" {
                Some(vec![("ShadowEnabled", Value::Bool(false))])
            } else if split_top(v, ',').len() > 1 {
                None
            } else {
                shadow(v, ctx)
            }
        }
        "text-align" => align_name(&lower).filter(|_| !matches!(lower.as_str(), "start" | "end")).map(|a| vec![("TextAlignment", Value::Text(a.to_owned()))]),
        "justify-content" => align_name(&lower).filter(|_| lower != "start" && lower != "end").map(|a| vec![("TextAlignment", Value::Text(a.to_owned()))]),
        "align-items" => match lower.as_str() {
            "flex-start" => Some(vec![("VerticalAlignment", Value::Text("Top".into()))]),
            "center" => Some(vec![("VerticalAlignment", Value::Text("Middle".into()))]),
            "flex-end" => Some(vec![("VerticalAlignment", Value::Text("Bottom".into()))]),
            _ => None,
        },
        "white-space" => match lower.as_str() {
            "pre-wrap" | "normal" => Some(vec![("WordWrap", Value::Bool(true))]),
            "pre" | "nowrap" => Some(vec![("WordWrap", Value::Bool(false))]),
            _ => None,
        },
        "z-index" => lower.parse::<i32>().ok().map(|n| vec![("ZOrder", Value::Num(n as f32))]),
        "display" => (lower == "none").then(|| vec![("Visible", Value::Bool(false))]),
        _ => None,
    })
}

/// `font: [style] [weight] size[/line-height] family`.
fn font_shorthand(v: &str, ctx: &Ctx) -> Option<Vec<(&'static str, Value)>> {
    let toks = tokens(v);
    let (mut italic, mut bold) = (false, false);
    for (i, t) in toks.iter().enumerate() {
        let size = t.split('/').next().unwrap_or(t);
        match t.to_ascii_lowercase().as_str() {
            "italic" | "oblique" => italic = true,
            "normal" | "small-caps" => {}
            _ if weight(t).is_some() && px(size, ctx).is_none() => bold = weight(t)?,
            _ => {
                let n = px(size, ctx)?;
                let family = toks[i + 1..].join(" ");
                let first = split_top(&family, ',').into_iter().next()?.trim().trim_matches(['"', '\'']).trim().to_owned();
                if first.is_empty() {
                    return None;
                }
                return Some(vec![("FontName", Value::Text(first)), ("FontSize", Value::Num(n)), ("Bold", Value::Bool(bold)), ("Italic", Value::Bool(italic))]);
            }
        }
    }
    None
}

// ── Writing ───────────────────────────────────────────────────────────────────────────────────

/// How a value was written, kept when it is written again.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Notation {
    Hex,
    Rgb,
    Hsl,
    Name,
    Px,
    Em,
    Rem,
    Pt,
}

/// The notation of the text a value was written in.
pub fn notation_of(text: &str) -> Notation {
    let t = text.trim().to_ascii_lowercase();
    if t.starts_with("rgb") {
        Notation::Rgb
    } else if t.starts_with("hsl") {
        Notation::Hsl
    } else if t.starts_with('#') {
        Notation::Hex
    } else if t.ends_with("rem") {
        Notation::Rem
    } else if t.ends_with("em") {
        Notation::Em
    } else if t.ends_with("pt") {
        Notation::Pt
    } else if t.ends_with("px") {
        Notation::Px
    } else if color(&t).is_some() {
        Notation::Name
    } else {
        Notation::Hex
    }
}

fn trim_num(n: f32, decimals: i32) -> String {
    let f = 10f32.powi(decimals);
    let s = format!("{}", (n * f).round() / f);
    s
}

/// A colour as CSS: `#rrggbb` (`#rrggbbaa` when translucent), `rgb()`, `hsl()` or a name.
pub fn write_color(c: [u8; 4], notation: Notation) -> String {
    let [r, g, b, a] = c;
    let alpha = || trim_num(a as f32 / 255.0, 3);
    match notation {
        Notation::Name => match color_name(c) {
            Some(n) => n.to_owned(),
            None => write_color(c, Notation::Hex),
        },
        Notation::Rgb if a == 255 => format!("rgb({r}, {g}, {b})"),
        Notation::Rgb => format!("rgba({r}, {g}, {b}, {})", alpha()),
        Notation::Hsl => {
            let (rf, gf, bf) = (r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0);
            let (mx, mn) = (rf.max(gf).max(bf), rf.min(gf).min(bf));
            let l = (mx + mn) / 2.0;
            let d = mx - mn;
            let (h, s) = if d == 0.0 {
                (0.0, 0.0)
            } else {
                let s = if l > 0.5 { d / (2.0 - mx - mn) } else { d / (mx + mn) };
                let h = if mx == rf {
                    (gf - bf) / d + if gf < bf { 6.0 } else { 0.0 }
                } else if mx == gf {
                    (bf - rf) / d + 2.0
                } else {
                    (rf - gf) / d + 4.0
                };
                (h * 60.0, s)
            };
            if a == 255 {
                format!("hsl({}, {}%, {}%)", trim_num(h, 3), trim_num(s * 100.0, 3), trim_num(l * 100.0, 3))
            } else {
                format!("hsla({}, {}%, {}%, {})", trim_num(h, 3), trim_num(s * 100.0, 3), trim_num(l * 100.0, 3), alpha())
            }
        }
        _ if a == 255 => format!("#{r:02x}{g:02x}{b:02x}"),
        _ => format!("#{r:02x}{g:02x}{b:02x}{a:02x}"),
    }
}

/// A length in px as CSS in the notation it was written in (`em` against the control's font size).
pub fn write_length(n: f32, notation: Notation, ctx: &Ctx) -> String {
    match notation {
        Notation::Em => format!("{}em", trim_num(n / ctx.font_px, 4)),
        Notation::Rem => format!("{}rem", trim_num(n / ctx.root_px, 4)),
        Notation::Pt => format!("{}pt", trim_num(n * 0.75, 3)),
        _ => format!("{}px", trim_num(n, 3)),
    }
}

/// The value text of a writable longhand: `value` of model property `property`, in `notation`.
/// `None` when the property has no longhand to write (the shorthands, `z-index`, `display`).
pub fn write(property: &str, value: &Value, notation: Notation, ctx: &Ctx) -> Option<String> {
    Some(match (property, value) {
        ("BackgroundColor" | "ForegroundColor" | "BorderColor", Value::Color(c)) => write_color(*c, notation),
        ("FontSize" | "PaddingLeft" | "PaddingTop" | "PaddingRight" | "PaddingBottom" | "Padding" | "CornerRadius" | "CornerRadiusTopLeft" | "CornerRadiusTopRight" | "CornerRadiusBottomLeft" | "CornerRadiusBottomRight" | "BorderWidth", Value::Num(n)) => {
            write_length(*n, notation, ctx)
        }
        ("Bold", Value::Bool(b)) => (if *b { "700" } else { "400" }).to_owned(),
        ("Italic", Value::Bool(b)) => (if *b { "italic" } else { "normal" }).to_owned(),
        ("FontName", Value::Text(t)) => {
            if t.chars().all(|c| c.is_ascii_alphanumeric() || c == '-') {
                t.clone()
            } else {
                format!("\"{}\"", t.replace('"', ""))
            }
        }
        ("Cursor", Value::Text(t)) => match t.as_str() {
            "Hand" => "pointer",
            "Text" => "text",
            "Wait" => "wait",
            "Crosshair" => "crosshair",
            "No" => "not-allowed",
            "SizeAll" => "move",
            "SizeNS" => "ns-resize",
            "SizeWE" => "ew-resize",
            "Help" => "help",
            _ => "default",
        }
        .to_owned(),
        ("BorderStyle", Value::Text(t)) => match t.as_str() {
            "Dashed" => "dashed",
            "Dotted" => "dotted",
            "Double" => "double",
            "None" => "none",
            "Raised" => "outset",
            "Sunken" => "inset",
            _ => "solid",
        }
        .to_owned(),
        ("TextAlignment", Value::Text(t)) => t.to_ascii_lowercase(),
        ("WordWrap", Value::Bool(b)) => (if *b { "pre-wrap" } else { "nowrap" }).to_owned(),
        _ => return None,
    })
}

/// The `text-decoration` text: the line tokens, `none` when there are none.
pub fn write_decoration(underline: bool, strikethrough: bool) -> String {
    match (underline, strikethrough) {
        (false, false) => "none".to_owned(),
        (true, false) => "underline".to_owned(),
        (false, true) => "line-through".to_owned(),
        (true, true) => "underline line-through".to_owned(),
    }
}

/// The `box-shadow` of the Shadow* properties, in the player's single-shadow form; `none` when off.
pub fn write_shadow(enabled: bool, direction: &str, distance: f32, blur_strength: f32, color: [u8; 3], opacity_pct: f32) -> String {
    if !enabled {
        return "none".to_owned();
    }
    let (_, ux, uy) = DIRECTIONS.iter().copied().find(|d| d.0.eq_ignore_ascii_case(direction)).unwrap_or(("South", 0, 1));
    let n = ((ux * ux + uy * uy) as f32).sqrt();
    let (dx, dy) = (ux as f32 / n * distance, uy as f32 / n * distance);
    let a = (opacity_pct / 100.0 * 255.0).round().clamp(0.0, 255.0) as u8;
    let inset = if blur_strength < 0.0 { "inset " } else { "" };
    format!("{inset}{}px {}px {}px {}", trim_num(dx, 3), trim_num(dy, 3), trim_num(blur_strength.abs() * 2.0, 3), write_color([color[0], color[1], color[2], a], Notation::Hex))
}

/// The CSS angle for a gradient direction name (`South` → `180deg`).
pub fn gradient_angle_deg(name: &str) -> f32 {
    angle_of_direction(name)
}


/// The CSS longhand that sets model property `property`, the one an edit writes when the winning
/// declaration is a shorthand it cannot rewrite in place.
pub fn longhand_of(property: &str) -> Option<&'static str> {
    Some(match property {
        "BackgroundColor" => "background-color",
        "ForegroundColor" => "color",
        "BorderColor" => "border-color",
        "BorderStyle" => "border-style",
        "BorderWidth" => "border-width",
        "FontName" => "font-family",
        "FontSize" => "font-size",
        "Bold" => "font-weight",
        "Italic" => "font-style",
        "Underline" | "Strikethrough" => "text-decoration",
        "Cursor" => "cursor",
        "Padding" => "padding",
        "PaddingLeft" => "padding-left",
        "PaddingTop" => "padding-top",
        "PaddingRight" => "padding-right",
        "PaddingBottom" => "padding-bottom",
        "CornerRadius" => "border-radius",
        "CornerRadiusTopLeft" => "border-top-left-radius",
        "CornerRadiusTopRight" => "border-top-right-radius",
        "CornerRadiusBottomLeft" => "border-bottom-left-radius",
        "CornerRadiusBottomRight" => "border-bottom-right-radius",
        "TextAlignment" => "text-align",
        "WordWrap" => "white-space",
        p if p.starts_with("Shadow") => "box-shadow",
        _ => return None,
    })
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::css_overrides::CSS_TO_PROPERTIES;

    fn ctx() -> Ctx {
        Ctx::new(ControlType::Panel)
    }

    fn known_in(css: &str, v: &str, c: &Ctx) -> Vec<(&'static str, Value)> {
        match read(css, v, c) {
            Reading::Known(k) => k,
            Reading::Unknown(u) => panic!("{css}: {v} read as Unknown({u})"),
        }
    }

    fn known(css: &str, v: &str, t: ControlType) -> Vec<(&'static str, Value)> {
        match read(css, v, &Ctx::new(t)) {
            Reading::Known(k) => k,
            Reading::Unknown(u) => panic!("{css}: {v} read as Unknown({u})"),
        }
    }

    /// AC1: a row for each CSS property of the forward table, in its order, with a fidelity.
    #[test]
    fn every_css_property_of_the_forward_table_has_a_row() {
        let rows: Vec<&str> = REVERSE.iter().map(|r| r.css).collect();
        let fwd: Vec<&str> = CSS_TO_PROPERTIES.iter().map(|(n, _)| *n).collect();
        assert_eq!(rows, fwd, "REVERSE and CSS_TO_PROPERTIES list the same properties in the same order");
        for (css, props) in CSS_TO_PROPERTIES {
            let r = row_of(css).expect(css);
            // Every longhand the forward table names is read by its row (or the row is not reversible).
            if r.fidelity != Fidelity::NotReversible && r.writable {
                assert!(!props.is_empty());
            }
        }
        eprintln!("css_reverse: {} rows = {} forward rows; {} writable", REVERSE.len(), fwd.len(), REVERSE.iter().filter(|r| r.writable).count());
    }

    /// AC1: property → CSS → the same property, in every notation.
    #[test]
    fn exact_rows_round_trip_in_every_notation() {
        let c = Ctx { control_type: ControlType::Panel, font_px: 16.0, root_px: 16.0 };
        let mut n = 0;
        let colours = [[255, 0, 0, 255], [0, 128, 0, 255], [18, 52, 86, 255], [200, 100, 50, 128], [255, 255, 255, 255], [0, 0, 0, 255]];
        for (css, prop) in [("background-color", "BackgroundColor"), ("color", "ForegroundColor"), ("border-color", "BorderColor")] {
            for col in colours {
                for nt in [Notation::Hex, Notation::Rgb, Notation::Hsl, Notation::Name] {
                    let text = write(prop, &Value::Color(col), nt, &c).expect("writer");
                    let back = known_in(css, &text, &c);
                    assert_eq!(back, vec![(prop, Value::Color(col))], "{css}: {col:?} as {nt:?} = {text}");
                    n += 1;
                }
            }
        }
        for (css, prop) in [("font-size", "FontSize"), ("padding-left", "PaddingLeft"), ("padding-top", "PaddingTop"), ("padding-right", "PaddingRight"), ("padding-bottom", "PaddingBottom"), ("border-width", "BorderWidth"), ("border-radius", "CornerRadius"), ("padding", "Padding")] {
            for v in [0.0, 4.0, 12.0, 20.0] {
                for nt in [Notation::Px, Notation::Em, Notation::Rem, Notation::Pt] {
                    let text = write(prop, &Value::Num(v), nt, &c).expect("writer");
                    assert_eq!(known_in(css, &text, &c), vec![(prop, Value::Num(v))], "{css}: {v} as {nt:?} = {text}");
                    n += 1;
                }
            }
        }
        for b in [true, false] {
            assert_eq!(known("font-weight", &write("Bold", &Value::Bool(b), Notation::Px, &c).unwrap(), ControlType::Panel), vec![("Bold", Value::Bool(b))]);
            assert_eq!(known("font-style", &write("Italic", &Value::Bool(b), Notation::Px, &c).unwrap(), ControlType::Panel), vec![("Italic", Value::Bool(b))]);
            assert_eq!(known("white-space", &write("WordWrap", &Value::Bool(b), Notation::Px, &c).unwrap(), ControlType::Label), vec![("WordWrap", Value::Bool(b))]);
            n += 3;
        }
        for (u, s) in [(false, false), (true, false), (false, true), (true, true)] {
            for css in ["text-decoration", "text-decoration-line"] {
                assert_eq!(known(css, &write_decoration(u, s), ControlType::Panel), vec![("Underline", Value::Bool(u)), ("Strikethrough", Value::Bool(s))]);
                n += 1;
            }
        }
        for cur in ["Default", "Hand", "Text", "Wait", "Crosshair", "No", "SizeAll", "SizeNS", "SizeWE", "Help"] {
            assert_eq!(known("cursor", &write("Cursor", &Value::Text(cur.into()), Notation::Px, &c).unwrap(), ControlType::Panel), vec![("Cursor", Value::Text(cur.into()))]);
            n += 1;
        }
        for st in ["Single", "Dashed", "Dotted", "Double", "None", "Raised", "Sunken"] {
            assert_eq!(known("border-style", &write("BorderStyle", &Value::Text(st.into()), Notation::Px, &c).unwrap(), ControlType::Panel), vec![("BorderStyle", Value::Text(st.into()))]);
            n += 1;
        }
        for al in ["Left", "Center", "Right"] {
            assert_eq!(known("text-align", &write("TextAlignment", &Value::Text(al.into()), Notation::Px, &c).unwrap(), ControlType::Label), vec![("TextAlignment", Value::Text(al.into()))]);
            n += 1;
        }
        assert_eq!(known("font-family", &write("FontName", &Value::Text("Segoe UI".into()), Notation::Px, &c).unwrap(), ControlType::Panel), vec![("FontName", Value::Text("Segoe UI".into()))]);
        // A shadow, through the player's single-shadow form.
        for dir in ["North", "NorthEast", "East", "SouthEast", "South", "SouthWest", "West", "NorthWest"] {
            let css = write_shadow(true, dir, 8.0, 6.0, [10, 20, 30], 40.0);
            let k = known("box-shadow", &css, ControlType::Panel);
            assert!(k.contains(&("ShadowDirection", Value::Text(dir.into()))), "{dir}: {css} -> {k:?}");
            assert!(k.contains(&("ShadowBlurStrength", Value::Num(6.0))) && k.contains(&("ShadowColor", Value::Color([10, 20, 30, 255]))), "{css} -> {k:?}");
            n += 1;
        }
        let inset = known("box-shadow", &write_shadow(true, "South", 4.0, -5.0, [0, 0, 0], 20.0), ControlType::Panel);
        assert!(inset.contains(&("ShadowBlurStrength", Value::Num(-5.0))), "{inset:?}");
        assert_eq!(known("box-shadow", "none", ControlType::Panel), vec![("ShadowEnabled", Value::Bool(false))]);
        eprintln!("css_reverse round trips: {n} property→CSS→property cases (hex, rgb(), hsl(), names; px, em, rem, pt; flags, enums, shadows)");
    }

    /// R3: each kind of value the model cannot hold stays Unknown with its text.
    #[test]
    fn each_kind_of_unknown_value_keeps_its_css_text() {
        let unknowns = [
            ("font-size", "120%"),
            ("padding", "calc(100% - 4px)"),
            ("padding-left", "5vw"),
            ("padding-top", "2vh"),
            ("padding-right", "env(safe-area-inset-right)"),
            ("color", "var(--missing)"),
            ("background", "url(a.png)"),
            ("background-image", "linear-gradient(red, green, blue)"),
            ("background-image", "radial-gradient(red, blue)"),
            ("box-shadow", "1px 1px red, 2px 2px blue"),
            ("color", "inherit"),
            ("font-weight", "unset"),
            ("cursor", "revert"),
            ("cursor", "grab"),
            ("border-style", "groove"),
            ("border-style", "solid dashed"),
            ("transform", "rotate(5deg)"),
            ("backdrop-filter", "blur(4px)"),
            ("border-radius", "4px 8px"),
            ("display", "flex"),
        ];
        for (css, v) in unknowns {
            assert_eq!(read(css, v, &ctx()), Reading::Unknown(v.to_owned()), "{css}: {v}");
        }
        eprintln!("css_reverse unknown: {} values of the R3 list stay Unknown, text kept", unknowns.len());
    }

    #[test]
    fn shorthands_read_into_their_longhands() {
        let k = known("background", "linear-gradient(90deg, #ff0000, rgb(0 0 255))", ControlType::Panel);
        assert_eq!(k[1], ("BackgroundGradientStartColor", Value::Color([255, 0, 0, 255])));
        assert_eq!(k[2], ("BackgroundGradientEndColor", Value::Color([0, 0, 255, 255])));
        assert_eq!(k[3], ("BackgroundGradientDirection", Value::Text("East".into())));
        assert_eq!(known("background-image", "linear-gradient(to top left, red, blue)", ControlType::Panel)[3].1, Value::Text("NorthWest".into()));
        assert_eq!(known("background-image", "none", ControlType::Panel), vec![("BackgroundGradientEnabled", Value::Bool(false))]);
        assert_eq!(known("background", "teal", ControlType::Panel)[0], ("BackgroundColor", Value::Color([0, 128, 128, 255])));
        let f = known("font", "italic 700 1.5em \"Segoe UI\", sans-serif", ControlType::Panel);
        assert_eq!(f, vec![("FontName", Value::Text("Segoe UI".into())), ("FontSize", Value::Num(21.0)), ("Bold", Value::Bool(true)), ("Italic", Value::Bool(true))]);
        let b = known("border", "2px dashed #ff0000", ControlType::Panel);
        assert_eq!(b, vec![("BorderWidth", Value::Num(2.0)), ("BorderStyle", Value::Text("Dashed".into())), ("BorderColor", Value::Color([255, 0, 0, 255]))]);
        let p = known("padding", "1px 2px 3px 4px", ControlType::Panel);
        assert_eq!(p, vec![("PaddingTop", Value::Num(1.0)), ("PaddingRight", Value::Num(2.0)), ("PaddingBottom", Value::Num(3.0)), ("PaddingLeft", Value::Num(4.0))]);
        assert_eq!(known("border-radius", "4px 8px", ControlType::Snackbar).len(), 4);
        assert_eq!(known("font-family", "\"Fira Code\", monospace", ControlType::Panel), vec![("FontName", Value::Text("Fira Code".into()))]);
        assert_eq!(known("font-size", "2rem", ControlType::Panel), vec![("FontSize", Value::Num(32.0))]);
        assert_eq!(known("font-size", "2em", ControlType::Panel), vec![("FontSize", Value::Num(28.0))], "em is the control's font size");
        assert_eq!(known("font-weight", "500", ControlType::Panel), vec![("Bold", Value::Bool(false))]);
        assert_eq!(known("font-weight", "600", ControlType::Panel), vec![("Bold", Value::Bool(true))]);
        assert_eq!(known("display", "none", ControlType::Panel), vec![("Visible", Value::Bool(false))]);
        assert_eq!(known("z-index", "3", ControlType::Panel), vec![("ZOrder", Value::Num(3.0))]);
    }

    #[test]
    fn fidelity_depends_on_the_control_where_the_player_adds_insets() {
        let padding = row_of("padding").unwrap();
        assert_eq!(padding.fidelity_on(&ControlType::Panel), Fidelity::Exact);
        assert_eq!(padding.fidelity_on(&ControlType::Button), Fidelity::Approximate);
        assert_eq!(row_of("text-align").unwrap().fidelity_on(&ControlType::Button), Fidelity::Approximate);
        assert_eq!(row_of("white-space").unwrap().fidelity_on(&ControlType::Label), Fidelity::Exact);
        assert_eq!(row_of("transform").unwrap().fidelity_on(&ControlType::Panel), Fidelity::NotReversible);
        assert!(!row_of("z-index").unwrap().writable && !row_of("display").unwrap().writable, "R16: never written back");
    }
}
