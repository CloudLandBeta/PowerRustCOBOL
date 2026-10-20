// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! **Properties to CSS** (WASM-007 R1): the one table that turns a control's appearance
//! properties into CSS3 declarations, each entry marked *exact*, *approximate* or *not on the web*.
//!
//! The declarations this module computes are the **lowest layer** of the cascade (WASM-007 R8: the
//! theme's defaults, then the properties, then the form stylesheet, then the control's `CssClass`
//! rules, then its `Style`). The renderer puts them where CSS the developer wrote can override them
//! — as inline CSSOM declarations *under* `@layer`-less author rules is not possible, so the
//! property layer is applied as a constructed stylesheet rule in a lower cascade layer
//! (`@layer pr-properties`), and the developer's stylesheet and `Style` come after it. Nothing here
//! writes a `style` attribute: under the page's CSP (no `'unsafe-inline'` for styles) only CSSOM
//! declarations are allowed.
//!
//! What it does *not* reproduce — the egui painter's liquid glass, perspective tilt of a whole
//! control, the operating-system blur — is listed in the table as *approximate* or *not on the web*
//! and shown in a property's tooltip (WASM-004 R6); a property that has no effect on the web stays
//! editable, so a form round-trips between kinds.

use crate::tree::{Ctl, Kind};
use crate::value::{css_color, fmt_number, parse_color, parse_f64, truthy};
use cobolt_forms::model::ControlType;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Fidelity {
    Exact,
    Approximate,
    NotOnWeb,
}

/// One row of the table of WASM-007 R1.
#[derive(Debug, Clone, Copy)]
pub struct Mapping {
    pub property: &'static str,
    pub css: &'static str,
    pub fidelity: Fidelity,
    pub note: &'static str,
}

const fn m(property: &'static str, css: &'static str, fidelity: Fidelity, note: &'static str) -> Mapping {
    Mapping { property, css, fidelity, note }
}

use Fidelity::*;

/// Every appearance property and what it becomes. A test checks the model's theme-owned
/// properties are all here (WASM-007 AC1).
pub const TABLE: &[Mapping] = &[
    m("BackgroundColor", "background-color", Exact, ""),
    m("BackgroundGradientEnabled", "background-image: linear-gradient(…)", Exact, ""),
    m("BackgroundGradientStartColor", "linear-gradient start stop", Exact, ""),
    m("BackgroundGradientEndColor", "linear-gradient end stop", Exact, ""),
    m("BackgroundGradientDirection", "linear-gradient angle (eight directions)", Exact, ""),
    m("ForegroundColor", "color", Exact, ""),
    m("FontName", "font-family", Exact, "a name the browser does not have falls back to the system font"),
    m("FontSize", "font-size", Exact, "pixels"),
    m("Bold", "font-weight: 700", Exact, ""),
    m("Italic", "font-style: italic", Exact, ""),
    m("Underline", "text-decoration: underline", Exact, ""),
    m("Strikethrough", "text-decoration: line-through", Exact, ""),
    m("Tooltip", "title attribute", Exact, ""),
    m("Cursor", "cursor", Exact, "Default|Hand|Text|Wait|Crosshair|No|SizeAll|SizeNS|SizeWE"),
    m("Padding", "padding", Exact, ""),
    m("PaddingLeft", "padding-left", Exact, ""),
    m("PaddingTop", "padding-top", Exact, ""),
    m("PaddingRight", "padding-right", Exact, ""),
    m("PaddingBottom", "padding-bottom", Exact, ""),
    m("Transparency", "alpha of the background, border and shadow (not opacity)", Exact, "fades the frame, never the content"),
    m("CornerRadius", "border-radius", Exact, "drawn corners of spec 057 (children clipped to the arc) are overflow: hidden on the container"),
    m("BorderStyle", "border-style", Approximate, "Raised/Sunken/Inset/Outset become a two-tone border"),
    m("BorderColor", "border-color", Exact, ""),
    m("BorderWidth", "border-width", Exact, ""),
    m("ShadowEnabled", "box-shadow", Approximate, "the neumorphic dual shadow is two box-shadows"),
    m("ShadowColor", "box-shadow colour", Exact, ""),
    m("ShadowLightColor", "second box-shadow colour", Exact, ""),
    m("ShadowOpacity", "alpha of the shadow colours", Exact, "percent"),
    m("ShadowDirection", "box-shadow offset direction", Exact, "eight directions"),
    m("ShadowDistance", "box-shadow offset", Exact, "0-60"),
    m("ShadowBlur", "box-shadow blur on/off", Exact, ""),
    m("ShadowBlurStrength", "box-shadow blur radius; negative = inset", Approximate, "-20..20"),
    m("TextAlignment", "text-align / justify-content", Exact, ""),
    m("VerticalAlignment", "align-items", Exact, ""),
    m("WordWrap", "white-space", Exact, ""),
    m("RotateX", "transform: perspective() rotateX()", Approximate, "no per-pixel perspective of the painter's content"),
    m("RotateY", "transform: perspective() rotateY()", Approximate, ""),
    m("PerspectiveDistance", "perspective()", Approximate, ""),
    m("PerspectiveStrength", "perspective()", Approximate, ""),
    m("BorderGradientEnabled", "border-image", NotOnWeb, "charts only; the frame is a plain border"),
    m("BorderBlur", "—", NotOnWeb, "an operating-system effect"),
    m("BorderTransparency", "—", NotOnWeb, ""),
    m("ZOrder", "z-index", Exact, ""),
    m("Visible", "display / hidden", Exact, ""),
    m("Enabled", "disabled / aria-disabled", Exact, ""),
    m("OsBlur", "backdrop-filter", NotOnWeb, "the operating-system blur behind a window has no page equivalent; backdrop-filter is not applied by default"),
];

/// The marking of a property, for its tooltip (WASM-004 R6): `None` for a property not in the table.
pub fn fidelity_of(property: &str) -> Option<&'static Mapping> {
    TABLE.iter().find(|r| r.property.eq_ignore_ascii_case(property))
}

/// A declaration list, in the order they are applied.
pub type Decls = Vec<(String, String)>;

fn push(d: &mut Decls, k: &str, v: impl Into<String>) {
    d.push((k.to_owned(), v.into()));
}

fn int(ctl: &Ctl, k: &str) -> Option<i64> {
    ctl.raw(k).and_then(parse_f64).map(|v| v as i64)
}

/// Scale a colour's alpha by `1 - transparency/100`.
fn faded(c: [u8; 4], transparency: i64) -> [u8; 4] {
    let keep = (100 - transparency.clamp(0, 100)) as f64 / 100.0;
    [c[0], c[1], c[2], (c[3] as f64 * keep).round() as u8]
}

/// `(dx, dy)` unit vector of a direction name.
pub fn direction_vector(name: &str) -> (f64, f64) {
    match name.trim() {
        "North" => (0.0, -1.0),
        "NorthEast" => (0.707, -0.707),
        "East" => (1.0, 0.0),
        "SouthEast" => (0.707, 0.707),
        "South" => (0.0, 1.0),
        "SouthWest" => (-0.707, 0.707),
        "West" => (-1.0, 0.0),
        "NorthWest" => (-0.707, -0.707),
        _ => (0.0, 1.0),
    }
}

/// The CSS angle of a gradient direction (the direction the gradient RUNS toward).
pub fn gradient_angle(name: &str) -> &'static str {
    match name.trim() {
        "North" => "0deg",
        "NorthEast" => "45deg",
        "East" => "90deg",
        "SouthEast" => "135deg",
        "South" => "180deg",
        "SouthWest" => "225deg",
        "West" => "270deg",
        "NorthWest" => "315deg",
        _ => "180deg",
    }
}

/// The font stack for a font name: the name first, then the system font (the page never shows a missing face).
pub fn font_family(name: &str) -> String {
    let n = name.trim();
    if n.is_empty() {
        return "system-ui, sans-serif".into();
    }
    let quoted = n.replace('"', "");
    format!("\"{quoted}\", system-ui, sans-serif")
}

pub fn cursor(name: &str) -> &'static str {
    match name.trim().to_ascii_lowercase().as_str() {
        "hand" | "pointer" => "pointer",
        "text" | "ibeam" => "text",
        "wait" | "busy" => "wait",
        "crosshair" | "cross" => "crosshair",
        "no" | "notallowed" => "not-allowed",
        "sizeall" | "move" => "move",
        "sizens" => "ns-resize",
        "sizewe" => "ew-resize",
        _ => "default",
    }
}

/// The `box-shadow` of a control's Shadow* properties, or `None` when it has none.
///
/// A positive blur strength is a raised shadow (and, with a light colour, the neumorphic pair: a
/// dark shadow toward the direction and a light one away from it); a negative one is sunken (inset).
pub fn box_shadow(ctl: &Ctl) -> Option<String> {
    if !ctl.raw("shadowenabled").map(truthy).unwrap_or(false) {
        return None;
    }
    let transparency = int(ctl, "transparency").unwrap_or(0);
    let color = parse_color(ctl.text_or("shadowcolor", "#000000FF")).unwrap_or([0, 0, 0, 255]);
    let light = parse_color(ctl.text_or("shadowlightcolor", "#FFFFFFFF")).unwrap_or([255, 255, 255, 255]);
    let opacity = int(ctl, "shadowopacity").unwrap_or(6).clamp(0, 100);
    let dist = int(ctl, "shadowdistance").unwrap_or(7).clamp(0, 60) as f64;
    let blur_on = ctl.raw("shadowblur").map(truthy).unwrap_or(true);
    let strength = if blur_on { int(ctl, "shadowblurstrength").unwrap_or(8).clamp(-20, 20) } else { 0 };
    let (ux, uy) = direction_vector(ctl.text_or("shadowdirection", "SouthEast"));
    let with_opacity = |c: [u8; 4]| faded([c[0], c[1], c[2], ((c[3] as i64) * opacity / 100) as u8], transparency);
    let inset = if strength < 0 { "inset " } else { "" };
    let blur = (strength.abs() as f64) * 2.0;
    let (dx, dy) = (ux * dist, uy * dist);
    let dark = format!("{inset}{}px {}px {}px {}", fmt_number(dx), fmt_number(dy), fmt_number(blur), css_color(with_opacity(color)));
    // The pair (neumorphic): a light shadow on the opposite side, when the control names a light colour.
    let has_light = ctl.raw("shadowlightcolor").and_then(parse_color).is_some_and(|c| c[3] > 0);
    if has_light && opacity > 0 && strength != 0 {
        let lit = format!("{inset}{}px {}px {}px {}", fmt_number(-dx), fmt_number(-dy), fmt_number(blur), css_color(with_opacity(light)));
        return Some(format!("{dark}, {lit}"));
    }
    Some(dark)
}

/// The background: a gradient when enabled, else the colour; with the control's transparency
/// applied to the colour's alpha.
pub fn background(ctl: &Ctl) -> Option<(&'static str, String)> {
    let transparency = int(ctl, "transparency").unwrap_or(0);
    if ctl.raw("backgroundgradientenabled").map(truthy).unwrap_or(false) {
        let a = parse_color(ctl.text_or("backgroundgradientstartcolor", "#FFFFFFFF"))?;
        let b = parse_color(ctl.text_or("backgroundgradientendcolor", "#000000FF"))?;
        return Some((
            "background-image",
            format!("linear-gradient({}, {}, {})", gradient_angle(ctl.text_or("backgroundgradientdirection", "South")), css_color(faded(a, transparency)), css_color(faded(b, transparency))),
        ));
    }
    let c = parse_color(ctl.raw("backgroundcolor")?)?;
    // "Still on the default means the developer has not chosen" (the renderer-wide convention of
    // `model::DEFAULT_BACKGROUND_COLOR`): the theme's own face stays. A form's colour is always a choice.
    if matches!(ctl.kind, Kind::Control(_)) && is_default(c, cobolt_forms::model::DEFAULT_BACKGROUND_COLOR) {
        return None;
    }
    Some(("background-color", css_color(faded(c, transparency))))
}

/// Is `c` exactly the colour `default_hex` (`#RRGGBB`)? An alpha of 255 only.
fn is_default(c: [u8; 4], default_hex: &str) -> bool {
    parse_color(default_hex).is_some_and(|d| d[..3] == c[..3]) && c[3] == 255
}

/// The border declarations.
pub fn border(ctl: &Ctl, d: &mut Decls) {
    let style = ctl.raw("borderstyle").map(str::trim).unwrap_or("");
    let width = int(ctl, "borderwidth").unwrap_or(1).clamp(0, 32);
    let transparency = int(ctl, "transparency").unwrap_or(0);
    let color = parse_color(ctl.text_or("bordercolor", "#808080FF")).unwrap_or([128, 128, 128, 255]);
    let c = css_color(faded(color, transparency));
    match style.to_ascii_lowercase().as_str() {
        "" | "none" => push(d, "border", "none"),
        "dashed" | "dash" => push(d, "border", format!("{width}px dashed {c}")),
        "dotted" | "dot" => push(d, "border", format!("{width}px dotted {c}")),
        "double" => push(d, "border", format!("{}px double {c}", width.max(3))),
        "raised" | "outset" => {
            let light = css_color(faded([255, 255, 255, 220], transparency));
            let dark = css_color(faded([0, 0, 0, 130], transparency));
            push(d, "border", format!("{width}px solid"));
            push(d, "border-color", format!("{light} {dark} {dark} {light}"));
        }
        "sunken" | "inset" => {
            let light = css_color(faded([255, 255, 255, 220], transparency));
            let dark = css_color(faded([0, 0, 0, 130], transparency));
            push(d, "border", format!("{width}px solid"));
            push(d, "border-color", format!("{dark} {light} {light} {dark}"));
        }
        _ => push(d, "border", format!("{width}px solid {c}")),
    }
}

/// The text declarations (font, colour, alignment).
pub fn text(ctl: &Ctl, d: &mut Decls) {
    if let Some(c) = ctl.raw("foregroundcolor").and_then(parse_color) {
        // The seeded white means "not chosen"; so does the black a faceless data-input control is
        // seeded with (a check box has no face of its own for black text to read on).
        let faceless = matches!(
            ctl.control_type(),
            Some(ControlType::CheckBox | ControlType::RadioButton | ControlType::Switch | ControlType::Slider | ControlType::Knob)
        );
        let unset = matches!(ctl.kind, Kind::Control(_))
            && (is_default(c, cobolt_forms::model::DEFAULT_FOREGROUND_COLOR) || (faceless && is_default(c, "#000000")));
        if !unset {
            push(d, "color", css_color(c));
        }
    }
    if let Some(n) = ctl.raw("fontname") {
        if !n.trim().is_empty() {
            push(d, "font-family", font_family(n));
        }
    }
    if let Some(s) = int(ctl, "fontsize") {
        if s > 0 {
            push(d, "font-size", format!("{s}px"));
        }
    }
    if ctl.raw("bold").map(truthy).unwrap_or(false) {
        push(d, "font-weight", "700");
    }
    if ctl.raw("italic").map(truthy).unwrap_or(false) {
        push(d, "font-style", "italic");
    }
    let mut deco = Vec::new();
    if ctl.raw("underline").map(truthy).unwrap_or(false) {
        deco.push("underline");
    }
    if ctl.raw("strikethrough").map(truthy).unwrap_or(false) {
        deco.push("line-through");
    }
    if !deco.is_empty() {
        push(d, "text-decoration", deco.join(" "));
    }
}

/// `justify-content` / `text-align` for a horizontal alignment name.
pub fn h_align(name: &str) -> (&'static str, &'static str) {
    match name.trim().to_ascii_lowercase().as_str() {
        "center" | "centre" | "middle" => ("center", "center"),
        "right" | "end" => ("right", "flex-end"),
        _ => ("left", "flex-start"),
    }
}

pub fn v_align(name: &str) -> &'static str {
    match name.trim().to_ascii_lowercase().as_str() {
        "top" | "start" => "flex-start",
        "bottom" | "end" => "flex-end",
        _ => "center",
    }
}

/// Everything the properties say about a control's look — the property layer of the cascade.
pub fn appearance(ctl: &Ctl) -> Decls {
    let mut d = Decls::new();
    if let Some((k, v)) = background(ctl) {
        push(&mut d, k, v);
    }
    text(ctl, &mut d);
    if let Some(r) = int(ctl, "cornerradius") {
        if r > 0 {
            push(&mut d, "border-radius", format!("{r}px"));
            // The drawn corners of spec 057: a child at a rounded corner stays inside its arc.
            if ctl.is_container() {
                push(&mut d, "overflow", "hidden");
            }
        }
    }
    if ctl.raw("borderstyle").is_some() {
        border(ctl, &mut d);
    }
    if let Some(s) = box_shadow(ctl) {
        push(&mut d, "box-shadow", s);
    }
    // Padding: the all-sides value, then any side that is set.
    if let Some(p) = int(ctl, "padding") {
        if p > 0 {
            push(&mut d, "padding", format!("{p}px"));
        }
    }
    for (k, side) in [("paddingleft", "left"), ("paddingtop", "top"), ("paddingright", "right"), ("paddingbottom", "bottom")] {
        if let Some(raw) = ctl.raw(k) {
            if let Some(v) = parse_f64(raw) {
                push(&mut d, &format!("padding-{side}"), format!("{}px", fmt_number(v)));
            }
        }
    }
    if let Some(c) = ctl.raw("cursor") {
        if !c.trim().is_empty() {
            push(&mut d, "cursor", cursor(c));
        }
    }
    if let Some(z) = int(ctl, "zorder") {
        if z != 0 {
            push(&mut d, "z-index", z.to_string());
        }
    }
    // Perspective (spec 096): an approximation on the element's own box.
    let rx = int(ctl, "rotatex").unwrap_or(0);
    let ry = int(ctl, "rotatey").unwrap_or(0);
    if rx != 0 || ry != 0 {
        let dist = int(ctl, "perspectivedistance").unwrap_or(50).clamp(1, 100);
        push(&mut d, "transform", format!("perspective({}px) rotateX({rx}deg) rotateY({ry}deg)", 400 + dist * 12));
    }
    d
}

/// Does a CSS declaration value attempt to load something from outside the bundle? (WASM-007 R13:
/// `url()` is limited to the project's assets and data URIs; legacy script-in-CSS is refused.)
pub fn css_is_safe(css: &str, allowed_origins: &[String]) -> Result<(), String> {
    let lower = css.to_ascii_lowercase();
    for bad in ["expression(", "javascript:", "vbscript:", "-moz-binding", "behavior:", "@import"] {
        if lower.contains(bad) {
            return Err(format!("the stylesheet uses `{bad}`, which is refused"));
        }
    }
    let mut rest = lower.as_str();
    while let Some(i) = rest.find("url(") {
        let after = &rest[i + 4..];
        let end = after.find(')').unwrap_or(after.len());
        let arg = after[..end].trim().trim_matches(|c| c == '"' || c == '\'').trim();
        let internal = arg.is_empty() || arg.starts_with("data:") || arg.starts_with("blob:") || arg.starts_with('#') || (!arg.contains(':') && !arg.starts_with("//"));
        if !internal && !allowed_origins.iter().any(|o| arg.starts_with(&o.to_ascii_lowercase())) {
            return Err(format!("the stylesheet loads `{}`, which is outside the project's assets and allowed origins", &arg[..arg.len().min(60)]));
        }
        rest = &after[end..];
    }
    Ok(())
}

/// The class names of a control's element (WASM-007 R3): its type, then the developer's `CssClass`.
pub fn class_names(ctl: &Ctl) -> Vec<String> {
    let mut v = vec!["pr-control".to_owned()];
    match &ctl.kind {
        Kind::Form => v.push("pr-form".into()),
        Kind::Control(t) => {
            v.push(format!("pr-{}", type_class(t)));
        }
    }
    if let Some(c) = ctl.raw("cssclass") {
        for n in c.split_whitespace() {
            if n.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_') {
                v.push(n.to_owned());
            }
        }
    }
    v
}

/// `Button` → `button`, `DateTimePicker` → `datetimepicker` (the public contract of WASM-007 Q4).
pub fn type_class(t: &ControlType) -> String {
    // The series' own controls reach the player as the model's `Custom`; they keep the plain name.
    if let Some(k) = crate::kinds::player_kind_of(t) {
        return k.name().to_ascii_lowercase();
    }
    t.as_str().to_ascii_lowercase().replace(':', "-")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tree::testing::tree;
    use cobolt_forms::model::THEME_OWNED_PROPS;

    fn set(t: &mut crate::tree::FormTree, name: &str, props: &[(&str, &str)]) -> usize {
        let i = t.by_name(name).unwrap();
        for (k, v) in props {
            t.set(i, k, v);
        }
        i
    }

    #[test]
    fn the_table_covers_every_theme_owned_property_of_the_model() {
        // WASM-007 AC1: an unmapped appearance property fails the test.
        for p in THEME_OWNED_PROPS {
            assert!(fidelity_of(p).is_some(), "`{p}` is an appearance property with no entry in the table");
        }
        // And every entry is marked.
        assert!(TABLE.iter().all(|r| !r.css.is_empty()));
        let exact = TABLE.iter().filter(|r| r.fidelity == Exact).count();
        let approx = TABLE.iter().filter(|r| r.fidelity == Approximate).count();
        let none = TABLE.iter().filter(|r| r.fidelity == NotOnWeb).count();
        println!("WASM-007 table: {} properties - {exact} exact, {approx} approximate, {none} not on the web", TABLE.len());
    }

    #[test]
    fn colours_gradients_and_transparency_become_css_with_the_frame_faded_not_the_content() {
        let mut t = tree();
        let i = set(&mut t, "BTN-GO", &[("BackgroundColor", "#36383EB3"), ("Transparency", "50"), ("ForegroundColor", "#E0E0E0FF")]);
        let d = appearance(t.ctl(i));
        let bg = d.iter().find(|(k, _)| k == "background-color").unwrap();
        assert_eq!(bg.1, "rgb(54 56 62 / 0.353)", "alpha B3 (179/255) halved by Transparency 50 is 90/255, as 8-bit alpha rounds");
        assert!(d.iter().all(|(k, _)| k != "opacity"), "Transparency never fades the content");
        assert!(d.contains(&("color".into(), "rgb(224 224 224)".into())));
        set(&mut t, "BTN-GO", &[("BackgroundGradientEnabled", "1"), ("BackgroundGradientStartColor", "#4E4E4EB3"), ("BackgroundGradientEndColor", "#000000B3"), ("BackgroundGradientDirection", "South"), ("Transparency", "0")]);
        let d = appearance(t.ctl(i));
        let g = d.iter().find(|(k, _)| k == "background-image").unwrap();
        assert!(g.1.starts_with("linear-gradient(180deg, rgb(78 78 78 / 0.702), rgb(0 0 0 / 0.702))"), "{}", g.1);
    }

    #[test]
    fn a_colour_still_on_its_seeded_default_is_not_a_choice_and_leaves_the_theme_alone() {
        let t = tree();
        let i = t.by_name("BTN-GO").unwrap();
        // Seeded by the model: #F0F0F0 and white are "not chosen".
        let d = appearance(t.ctl(i));
        assert!(!d.iter().any(|(k, _)| k == "background-color" || k == "color"), "{d:?}");
        // The form's own colour is always a choice.
        let f = appearance(t.ctl(0));
        assert!(f.iter().any(|(k, _)| k == "background-color"), "{f:?}");
    }

    #[test]
    fn the_spatial_shadow_becomes_a_box_shadow_and_a_negative_strength_is_inset() {
        let mut t = tree();
        let i = set(
            &mut t,
            "BTN-GO",
            &[("ShadowEnabled", "1"), ("ShadowOpacity", "6"), ("ShadowColor", "#000000FF"), ("ShadowLightColor", "#4E4E4EFF"), ("ShadowDirection", "SouthEast"), ("ShadowDistance", "7"), ("ShadowBlur", "1"), ("ShadowBlurStrength", "8")],
        );
        let s = box_shadow(t.ctl(i)).unwrap();
        assert!(s.starts_with("4.949px 4.949px 16px rgb(0 0 0 / 0.059)"), "6 % of an opaque shadow is 15/255: {s}");
        assert!(s.contains(", -4.949px -4.949px 16px rgb(78 78 78 / 0.059)"), "the light shadow is on the opposite side: {s}");
        set(&mut t, "BTN-GO", &[("ShadowBlurStrength", "-12")]);
        assert!(box_shadow(t.ctl(i)).unwrap().starts_with("inset "), "sunken");
        set(&mut t, "BTN-GO", &[("ShadowEnabled", "0")]);
        assert_eq!(box_shadow(t.ctl(i)), None);
    }

    #[test]
    fn borders_fonts_padding_cursor_and_radius() {
        let mut t = tree();
        let i = set(
            &mut t,
            "TXT-IN",
            &[("BorderStyle", "Dashed"), ("BorderColor", "#FF0000FF"), ("BorderWidth", "2"), ("FontName", "Segoe UI"), ("FontSize", "14"), ("Bold", "1"), ("Underline", "1"), ("Strikethrough", "1"), ("Padding", "6"), ("PaddingLeft", "10"), ("Cursor", "Hand"), ("CornerRadius", "8")],
        );
        let d = appearance(t.ctl(i));
        let get = |k: &str| d.iter().find(|(x, _)| x == k).map(|(_, v)| v.as_str());
        assert_eq!(get("border"), Some("2px dashed rgb(255 0 0)"));
        assert_eq!(get("font-family"), Some("\"Segoe UI\", system-ui, sans-serif"));
        assert_eq!(get("font-size"), Some("14px"));
        assert_eq!(get("font-weight"), Some("700"));
        assert_eq!(get("text-decoration"), Some("underline line-through"));
        assert_eq!(get("padding"), Some("6px"));
        assert_eq!(get("padding-left"), Some("10px"));
        assert_eq!(get("cursor"), Some("pointer"));
        assert_eq!(get("border-radius"), Some("8px"));
    }

    #[test]
    fn raised_and_sunken_borders_are_two_tone() {
        let mut t = tree();
        let i = set(&mut t, "TXT-IN", &[("BorderStyle", "Sunken"), ("BorderWidth", "2")]);
        let d = appearance(t.ctl(i));
        let bc = d.iter().find(|(k, _)| k == "border-color").unwrap();
        assert!(bc.1.starts_with("rgb(0 0 0 / 0.51"), "dark top-left for sunken: {}", bc.1);
    }

    #[test]
    fn the_cascade_hooks_are_the_public_contract() {
        let mut t = tree();
        let i = set(&mut t, "BTN-GO", &[("CssClass", "primary big ev\"il")]);
        let names = class_names(t.ctl(i));
        assert_eq!(names, ["pr-control", "pr-button", "primary", "big"], "a class with a quote is dropped");
        assert_eq!(type_class(&ControlType::DateTimePicker), "datetimepicker");
        assert_eq!(type_class(&ControlType::from_str("WebLink")), "weblink", "a type the model carries as Custom keeps a stable class");
        assert_eq!(type_class(&ControlType::from_str("acme:thing")), "acme-thing", "a plug-in control is named after its plug-in");
    }

    #[test]
    fn a_stylesheet_cannot_load_what_the_project_did_not_ship() {
        let ok = vec!["https://fonts.example.com".to_owned()];
        assert!(css_is_safe(".a { background: url(assets/bg.png) }", &ok).is_ok());
        assert!(css_is_safe(".a { background: url(\"data:image/png;base64,AAAA\") }", &ok).is_ok());
        assert!(css_is_safe(".a { background: url(https://fonts.example.com/x.woff2) }", &ok).is_ok(), "an allowed origin");
        assert!(css_is_safe(".a { background: url(https://evil.example.org/x.png) }", &ok).is_err());
        assert!(css_is_safe("@import url(x.css);", &ok).is_err());
        assert!(css_is_safe(".a { width: expression(alert(1)) }", &ok).is_err());
        assert!(css_is_safe(".a { background: url(javascript:alert(1)) }", &ok).is_err());
    }

    #[test]
    fn alignment_and_cursor_names_map_to_css() {
        assert_eq!(h_align("Center"), ("center", "center"));
        assert_eq!(h_align("Right"), ("right", "flex-end"));
        assert_eq!(v_align("Top"), "flex-start");
        assert_eq!(v_align("Middle"), "center");
        assert_eq!(cursor("SizeWE"), "ew-resize");
        assert_eq!(cursor("whatever"), "default");
        assert_eq!(direction_vector("West"), (-1.0, 0.0));
    }
}
