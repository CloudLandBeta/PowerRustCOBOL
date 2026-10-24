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
//! control, the operating-system blur — is marked *approximate* or *not on the web* in the shared
//! fidelity rows (`cobolt_forms::web_class::FIDELITY`): the Properties panel of a web form marks an
//! approximate property in its tooltip and hides one with no effect on the web, whose value stays
//! in the form file, so a form round-trips between kinds (WASM-004 R6).

use crate::tree::{Ctl, Kind};
use crate::value::{css_color, fmt_number, parse_color, parse_f64, truthy};
use cobolt_forms::model::ControlType;

// The fidelity of each property (exact, approximate, not on the web) and its note live in
// `cobolt_forms::web_class::FIDELITY` (WASM-004 T13), which the IDE's Properties panel reads too.
// This table holds only what the browser does with each of those rows.
pub use cobolt_forms::web_class::{fidelity_of, Fidelity, PropertyFidelity};

/// One row of the table of WASM-007 R1: the CSS a shared fidelity row becomes.
#[derive(Debug, Clone, Copy)]
pub struct Mapping {
    pub property: &'static str,
    pub css: &'static str,
}

const fn m(property: &'static str, css: &'static str) -> Mapping {
    Mapping { property, css }
}

/// Every appearance property and what it becomes. A test checks the model's theme-owned
/// properties are all here (WASM-007 AC1), and another that these rows and the shared fidelity
/// rows name the same properties.
pub const TABLE: &[Mapping] = &[
    m("BackgroundColor", "background-color"),
    m("BackgroundGradientEnabled", "background-image: linear-gradient(…)"),
    m("BackgroundGradientStartColor", "linear-gradient start stop"),
    m("BackgroundGradientEndColor", "linear-gradient end stop"),
    m("BackgroundGradientDirection", "linear-gradient angle (eight directions)"),
    m("ForegroundColor", "color"),
    m("FontName", "font-family"),
    m("FontSize", "font-size"),
    m("Bold", "font-weight: 700"),
    m("Italic", "font-style: italic"),
    m("Underline", "text-decoration: underline"),
    m("Strikethrough", "text-decoration: line-through"),
    m("Tooltip", "title attribute"),
    m("Cursor", "cursor"),
    m("Padding", "padding"),
    m("PaddingLeft", "padding-left"),
    m("PaddingTop", "padding-top"),
    m("PaddingRight", "padding-right"),
    m("PaddingBottom", "padding-bottom"),
    m("Transparency", "alpha of the background, border and shadow (not opacity)"),
    m("CornerRadius", "border-radius"),
    m("BorderStyle", "border-style"),
    m("BorderColor", "border-color"),
    m("BorderWidth", "border-width"),
    m("ShadowEnabled", "box-shadow"),
    m("ShadowColor", "box-shadow colour"),
    m("ShadowLightColor", "second box-shadow colour"),
    m("ShadowOpacity", "alpha of the shadow colours"),
    m("ShadowDirection", "box-shadow offset direction"),
    m("ShadowDistance", "box-shadow offset"),
    m("ShadowBlur", "box-shadow blur on/off"),
    m("ShadowBlurStrength", "box-shadow blur radius; negative = inset"),
    m("TextAlignment", "text-align / justify-content"),
    m("VerticalAlignment", "align-items"),
    m("WordWrap", "white-space"),
    m("RotateX", "transform: perspective() rotateX()"),
    m("RotateY", "transform: perspective() rotateY()"),
    m("PerspectiveDistance", "perspective()"),
    m("PerspectiveStrength", "perspective()"),
    m("BorderGradientEnabled", "border-image"),
    m("BorderBlur", "—"),
    m("BorderTransparency", "—"),
    m("ZOrder", "z-index"),
    m("Visible", "display / hidden"),
    m("Enabled", "disabled / aria-disabled"),
    m("OsBlur", "backdrop-filter"),
];

/// The CSS row of a property: `None` for a property not in the table.
pub fn mapping_of(property: &str) -> Option<&'static Mapping> {
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

/// A gradient stop as the desktop paints it under a control's `Transparency`. The painter fades a gradient's vertex colour by
/// keeping its colour channels and scaling only its alpha (`face_color`), and the channels are premultiplied: the stop is
/// therefore `c / (1 - t)` at alpha `a * (1 - t)`, brighter than the colour named — a fade that shows `t` of what is
/// behind the card on top of the whole colour, not on top of `1 - t` of it. A solid face is faded the ordinary way.
fn gradient_stop(c: [u8; 4], transparency: i64) -> [u8; 4] {
    let keep = (100 - transparency.clamp(0, 100)) as f64 / 100.0;
    if keep <= 0.0 {
        return [c[0], c[1], c[2], 0];
    }
    let up = |v: u8| ((v as f64 / keep).round()).min(255.0) as u8;
    [up(c[0]), up(c[1]), up(c[2]), (c[3] as f64 * keep).round() as u8]
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

/// The family the desktop draws text in when a form names none of its own, or names Arial, Helvetica
/// or `sans-serif`: egui's built-in proportional face (`fonts::is_builtin`), Ubuntu Light. The player
/// ships it (`assets/web-fonts`) so the two surfaces letter the same text the same way; the
/// host page declares it as `@font-face` under this name.
pub const STAND_IN_FONT: &str = "PR Ubuntu Light";

/// The font stack for a font name, as the desktop resolves it (`fonts::font_id`): Arial, Helvetica,
/// `sans-serif` and an empty name are the stand-in face; any other name is that family if the machine
/// has it and the stand-in face if it has not — which is what the desktop does when a system font cannot
/// be loaded. The system font comes last, for the glyphs the stand-in lacks.
pub fn font_family(name: &str) -> String {
    let n = name.trim();
    let builtin = n.is_empty() || n.eq_ignore_ascii_case("Arial") || n.eq_ignore_ascii_case("Helvetica") || n.eq_ignore_ascii_case("sans-serif");
    if builtin {
        return format!("\"{STAND_IN_FONT}\", system-ui, sans-serif");
    }
    let quoted = n.replace('"', "");
    format!("\"{quoted}\", \"{STAND_IN_FONT}\", system-ui, sans-serif")
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

/// Does the desktop painter cast a drop shadow for this control at all, whatever its properties say?
/// (`paint::drop_shadow_spec`: a line, a non-visual service and a round or triangular shape draw
/// their own silhouette or none.)
fn can_cast_a_shadow(ctl: &Ctl) -> bool {
    match ctl.control_type() {
        Some(ControlType::Line | ControlType::Timer | ControlType::AgentObject | ControlType::KnowledgeBase | ControlType::RestClient | ControlType::SqlDatabase | ControlType::IndexedFile) => false,
        Some(ControlType::Shape) => !matches!(ctl.raw("shapetype").map(str::trim), Some("Circle" | "Ellipse" | "Triangle")),
        _ => true,
    }
}

/// The `box-shadow` of a control's Shadow* properties, or `None` when it has none.
///
/// **The desktop's own recipe** (`paint::regular_shadow_stack`, shared as `cobolt_forms::shadow`): the
/// shadow is a STACK of rounded rectangles, each the control's rectangle moved by `ShadowDistance` in
/// `ShadowDirection` and grown by a little less than the one before, filled with `ShadowColor` at a
/// faint alpha — so the core is the sum of them all, which a single blurred shadow of the same alpha
/// is not. A browser lays the same stack down with one `box-shadow` per ring and no blur. The face's
/// own `Transparency` fades the shadow with it, and a shadow is never drawn under the face itself
/// (CSS clips an outer shadow to outside the border box, as the painter cuts a hole in it).
///
/// A negative `ShadowBlurStrength` is the sunken variant (the shadow over the face): an inset shadow.
pub fn box_shadow(ctl: &Ctl) -> Option<String> {
    if !ctl.raw("shadowenabled").map(truthy).unwrap_or(false) || !can_cast_a_shadow(ctl) {
        return None;
    }
    let transparency = int(ctl, "transparency").unwrap_or(0).clamp(0, 100);
    let color = parse_color(ctl.text_or("shadowcolor", "#000000")).unwrap_or([0, 0, 0, 255]);
    let opacity = int(ctl, "shadowopacity").unwrap_or(20).clamp(0, 100) as f32 / 100.0;
    let dist = int(ctl, "shadowdistance").unwrap_or(7).clamp(0, 60) as f64;
    let blur_on = ctl.raw("shadowblur").map(truthy).unwrap_or(true);
    let strength = if blur_on { int(ctl, "shadowblurstrength").unwrap_or(8).clamp(-20, 20) } else { 0 };
    let (ux, uy) = direction_vector(ctl.text_or("shadowdirection", "South"));
    let (dx, dy) = (ux * dist, uy * dist);
    let face = 1.0 - transparency as f32 / 100.0;
    if strength < 0 {
        let (r, g, b) = (color[0], color[1], color[2]);
        // Sunken: the same colour at the opacity the face leaves, as one inset blur.
        let a = ((opacity * face * 255.0).round() / 255.0) as f64;
        return Some(format!("inset {}px {}px {}px rgb({r} {g} {b} / {})", fmt_number(dx), fmt_number(dy), fmt_number(strength.unsigned_abs() as f64 * 2.0), fmt_number((a * 1000.0).round() / 1000.0)));
    }
    // Innermost first: a CSS list paints its first shadow on top, the painter its last.
    let rings: Vec<String> = cobolt_forms::shadow::rings(strength as usize, opacity, face)
        .iter()
        .rev()
        .filter(|ring| ring.alpha > 0)
        .map(|ring| {
            let alpha = (ring.alpha as f64 / 255.0 * 10000.0).round() / 10000.0;
            // The colour the painter ends up with after premultiplying by truncation, not the one named.
            let [r, g, b] = ring.straight([color[0], color[1], color[2]]);
            format!("{}px {}px 0 {}px rgb({r} {g} {b} / {})", fmt_number(dx), fmt_number(dy), fmt_number((ring.expand as f64 * 100.0).round() / 100.0), fmt_number(alpha))
        })
        .collect();
    (!rings.is_empty()).then(|| rings.join(", "))
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
            format!("linear-gradient({}, {}, {})", gradient_angle(ctl.text_or("backgroundgradientdirection", "South")), css_color(gradient_stop(a, transparency)), css_color(gradient_stop(b, transparency))),
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

/// The width of the border the element ends up with, in pixels: what `border` writes (a double
/// border is at least 3), what the base sheet gives a GroupBox that names no border style, and
/// nothing for a Panel or GroupBox whose background is hidden (`kind_decls` takes its border off).
/// The one answer for the padding that gives a border back and for the origin of absolutely
/// positioned children, which count from inside it.
pub fn border_width(ctl: &Ctl) -> f64 {
    if matches!(ctl.control_type(), Some(ControlType::Panel | ControlType::GroupBox)) && ctl.flag("hidebackground") {
        return 0.0;
    }
    let width = int(ctl, "borderwidth").unwrap_or(1).clamp(0, 32) as f64;
    match ctl.raw("borderstyle").map(|s| s.trim().to_ascii_lowercase()) {
        None => {
            if matches!(ctl.control_type(), Some(ControlType::GroupBox)) {
                1.0
            } else {
                0.0
            }
        }
        Some(s) => match s.as_str() {
            "" | "none" => 0.0,
            "double" => width.max(3.0),
            _ => width,
        },
    }
}

/// The border declarations.
pub fn border(ctl: &Ctl, d: &mut Decls) {
    let style = ctl.raw("borderstyle").map(str::trim).unwrap_or("");
    let width = int(ctl, "borderwidth").unwrap_or(1).clamp(0, 32);
    let transparency = int(ctl, "transparency").unwrap_or(0);
    // The painter's border when none is chosen (`control_colors`): a light slate, not mid grey.
    let color = parse_color(ctl.text_or("bordercolor", "#8C8CA0FF")).unwrap_or([140, 140, 160, 255]);
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

/// How far to the right the desktop's second stamp of a bold caption lands: 1 px or none.
///
/// The caption is laid out once and put down twice, the second time half a pixel to the right, and each time its
/// position is rounded to a whole pixel (half away from zero). So the stamps coincide when the caption's own
/// position rounds the same both ways (its fraction is half or more) and are a pixel apart otherwise. The position is
/// where the control puts the caption (`paint`): the anchor its alignment names — the frame's inset edge, or its middle
/// less half the caption — from the control's designed rectangle.
fn bold_offset(ctl: &Ctl, size: f64) -> f64 {
    let (x, w) = (ctl.num("x").unwrap_or(0.0), ctl.num("width").unwrap_or(0.0));
    let own = ctl.num("padding").unwrap_or(0.0).clamp(0.0, 128.0);
    let text_w = |fit: f64| crate::metrics::text_width(ctl.raw("caption").unwrap_or(""), fit);
    let anchor = match ctl.control_type() {
        Some(ControlType::Label) => {
            let pad = (3.0_f64.min(w * 0.25) + own).min(w * 0.45);
            let a = ctl.text_or("textalignment", "Left").to_ascii_lowercase();
            if a.ends_with("center") || a == "centre" || a == "middle" {
                // The row is put at the whole pixel of the middle, less the whole pixel of half its width.
                return if (x + w / 2.0).fract() < 0.5 { 1.0 } else { 0.0 };
            } else if a.ends_with("right") {
                x + w - pad
            } else {
                x + pad
            }
        }
        Some(ControlType::Button) => {
            let a = ctl.text_or("textalignment", "MiddleCenter").to_ascii_lowercase();
            let inset = 6.0_f64.min(w * 0.25);
            let fit = crate::metrics::fitted_size(ctl.raw("caption").unwrap_or(""), size, (w - 2.0 * (3.0_f64.min(w * 0.2) + own)).max(1.0), 1.0e6, true);
            let tw = text_w(fit);
            if a.ends_with("left") {
                x + inset
            } else if a.ends_with("right") {
                x + w - inset - tw
            } else {
                x + w / 2.0 - tw / 2.0
            }
        }
        _ => return 1.0,
    };
    ((anchor + 0.5).round() - anchor.round()).max(0.0)
}

/// The text declarations (font, colour, alignment).
pub fn text(ctl: &Ctl, d: &mut Decls) {
    // The ink is the control's own `ForegroundColor` — always, and never inherited from the container it
    // sits in. The desktop paints each control's text in the colour that control names (the seeded white
    // included: a form theme that publishes a text colour answers it with white, and one that publishes none
    // keeps the white), so a label on a dark card is white whatever ink the card itself carries.
    let ink = ctl.raw("foregroundcolor").and_then(parse_color);
    if let Some(c) = ink {
        push(d, "color", css_color(c));
    }
    if let Some(n) = ctl.raw("fontname") {
        if !n.trim().is_empty() {
            push(d, "font-family", font_family(n));
        }
    }
    // A size is a number of pixels with a fraction when there is one: the desktop letters 18.5 at 18.5, and the form's font
    // factor makes 23.8 of a 28 (`tree::scaled_font`).
    let stated = ctl.raw("fontsize").and_then(parse_f64).filter(|s| *s > 0.0);
    if let Some(s) = stated {
        push(d, "font-size", format!("{}px", fmt_number(s)));
        // The row the desktop's text layout makes of this size (`metrics::row_height`), so that a line
        // sits where the desktop's sits and a block is as tall as the desktop's.
        push(d, "line-height", format!("{}px", crate::value::fmt_number(crate::metrics::row_height(s))));
    }
    // A browser's glyphs carry less ink than the desktop's at small sizes and more at large ones; an outline
    // of the measured width (`metrics::stroke`) brings the two to the same weight.
    let size = stated.unwrap_or(11.0);
    let bold = ctl.raw("bold").map(truthy).unwrap_or(false);
    push(d, "-webkit-text-stroke", format!("{}px", fmt_number(crate::metrics::stroke(size, ink.unwrap_or([255, 255, 255, 255])))));
    if bold {
        // The desktop has no bold face for a font it did not load as one: it stamps the caption a
        // second time half a point to the right (`paint::paint_styled_galley`) — and a galley is put on a whole
        // pixel, so the second stamp lands a whole pixel to the right or, when the caption's position
        // rounds the same way, exactly on the first. Which of the two it is is arithmetic on the caption's
        // place (`bold_offset`). The browser's own bold would be another weight of another shape, so the same
        // second stamp is a shadow of the text itself, and the weight stays regular.
        push(d, "font-weight", "400");
        let k = bold_offset(ctl, size);
        if k > 0.0 {
            push(d, "text-shadow", format!("{}px 0 0 currentColor", fmt_number(k)));
        }
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
    // Padding: the all-sides value, then any side that is set. A Panel's and a GroupBox's padding is
    // the layout padding of their client area, which `layout_css::container` writes (it also gives
    // back what the border and the desktop's content inset take).
    if !matches!(ctl.control_type(), Some(ControlType::Panel | ControlType::GroupBox)) {
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
        let n = |k: Fidelity| cobolt_forms::web_class::FIDELITY.iter().filter(|r| r.fidelity == k).count();
        println!(
            "WASM-007 table: {} properties - {} exact, {} approximate, {} not on the web",
            TABLE.len(),
            n(Fidelity::Exact),
            n(Fidelity::Approximate),
            n(Fidelity::NotOnWeb)
        );
    }

    #[test]
    fn the_css_rows_and_the_shared_fidelity_rows_name_the_same_properties() {
        // WASM-004 T13: a property in one table and not the other fails.
        use std::collections::BTreeSet;
        let css: Vec<&str> = TABLE.iter().map(|r| r.property).collect();
        let shared: Vec<&str> = cobolt_forms::web_class::FIDELITY.iter().map(|r| r.property).collect();
        let (css_set, shared_set): (BTreeSet<&str>, BTreeSet<&str>) = (css.iter().copied().collect(), shared.iter().copied().collect());
        assert_eq!(css.len(), css_set.len(), "a property has two CSS rows");
        assert_eq!(shared.len(), shared_set.len(), "a property has two fidelity rows");
        assert_eq!(css_set.difference(&shared_set).collect::<Vec<_>>(), Vec::<&&str>::new(), "CSS rows with no fidelity row");
        assert_eq!(shared_set.difference(&css_set).collect::<Vec<_>>(), Vec::<&&str>::new(), "fidelity rows with no CSS row");
        println!("{} properties in both tables", css.len());
        assert_eq!(mapping_of("osblur").map(|r| r.css), Some("backdrop-filter"));
        assert_eq!(fidelity_of("OsBlur").map(|r| r.fidelity), Some(Fidelity::NotOnWeb));
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
        // A gradient is faded the way the painter fades it: the channels are kept (they are premultiplied) and only the
        // alpha is scaled, so each stop is its colour over `1 - t`, at `1 - t` of its alpha — brighter than the colour named.
        set(&mut t, "BTN-GO", &[("BackgroundGradientStartColor", "#0A1B45"), ("BackgroundGradientEndColor", "#1F4A94"), ("Transparency", "12")]);
        let d = appearance(t.ctl(i));
        let g = d.iter().find(|(k, _)| k == "background-image").unwrap();
        assert_eq!(g.1, "linear-gradient(180deg, rgb(11 31 78 / 0.878), rgb(35 84 168 / 0.878))", "{}", g.1);
    }

    #[test]
    fn a_background_still_on_its_seeded_default_is_not_a_choice_but_the_ink_is_always_the_controls_own() {
        let t = tree();
        let i = t.by_name("BTN-GO").unwrap();
        // The seeded background (#F0F0F0) is "not chosen": the theme's face stays.
        let d = appearance(t.ctl(i));
        assert!(!d.iter().any(|(k, _)| k == "background-color"), "{d:?}");
        // The ink is never inherited from the container: the control's own colour, the seeded white included
        // (the desktop paints a label on a dark card in white whatever ink the card itself carries).
        assert!(d.iter().any(|(k, v)| k == "color" && v == "rgb(255 255 255)"), "{d:?}");
        // The form's own colour is always a choice.
        let f = appearance(t.ctl(0));
        assert!(f.iter().any(|(k, _)| k == "background-color"), "{f:?}");
    }

    #[test]
    fn a_shadow_is_the_painters_stack_of_rings_and_a_negative_strength_is_inset() {
        let mut t = tree();
        let i = set(
            &mut t,
            "BTN-GO",
            &[("ShadowEnabled", "1"), ("ShadowOpacity", "6"), ("ShadowColor", "#000000FF"), ("ShadowLightColor", "#4E4E4EFF"), ("ShadowDirection", "SouthEast"), ("ShadowDistance", "7"), ("ShadowBlur", "1"), ("ShadowBlurStrength", "8")],
        );
        let s = box_shadow(t.ctl(i)).unwrap();
        let layers: Vec<&str> = s.split(", ").collect();
        let rings: Vec<_> = cobolt_forms::shadow::rings(8, 0.06, 1.0).into_iter().filter(|r| r.alpha > 0).collect();
        assert_eq!(layers.len(), rings.len(), "one box-shadow per visible ring: {s}");
        assert!(layers.iter().all(|l| l.starts_with("4.949px 4.949px 0 ")), "each ring is the control's box moved 7 px south-east with no blur: {s}");
        assert!(layers.first().unwrap().starts_with("4.949px 4.949px 0 0px "), "innermost first: the CSS list paints its first shadow on top");
        assert!(layers.last().unwrap().contains(" 8px "), "the widest ring reaches the strength");
        assert!(!s.contains("78 78 78"), "the light colour is the Neumorphic pair's, which a regular shadow never draws");
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
        assert_eq!(get("font-family"), Some("\"Segoe UI\", \"PR Ubuntu Light\", system-ui, sans-serif"), "the named font, else the desktop's stand-in face");
        assert_eq!(get("font-size"), Some("14px"));
        assert_eq!(get("line-height"), Some("16px"), "the row the desktop's text layout makes of 14 px (its font height rounded)");
        assert_eq!(get("font-weight"), Some("400"), "bold is the desktop's second stamp, not another weight");
        assert_eq!(get("text-shadow"), Some("1px 0 0 currentColor"), "a control the player does not place a caption for puts the second stamp a whole pixel over");
        assert_eq!(get("text-decoration"), Some("underline line-through"));
        assert_eq!(get("padding"), Some("6px"));
        assert_eq!(get("padding-left"), Some("10px"));
        assert_eq!(get("cursor"), Some("pointer"));
        assert_eq!(get("border-radius"), Some("8px"));
    }

    #[test]
    fn a_bold_caption_is_stamped_a_whole_pixel_over_or_on_itself_by_where_it_is_put() {
        let mut t = tree();
        let tw = crate::metrics::text_width("Dados", 13.0);
        let mut seen = [false, false];
        // A button whose caption is centred: the stamps are a pixel apart when the caption's position (a float, the middle less
        // half its width) has a fraction under a half, and coincide otherwise. Widths 120 to 160 give both.
        for w in 120..160 {
            let i = set(&mut t, "BTN-GO", &[("Caption", "Dados"), ("Bold", "1"), ("FontSize", "13"), ("X", "100"), ("Width", &w.to_string()), ("TextAlignment", "MiddleCenter")]);
            let at = 100.0 + w as f64 / 2.0 - tw / 2.0;
            let apart = (at + 0.5).round() - at.round() > 0.0;
            let got = appearance(t.ctl(i)).iter().find(|(k, _)| k == "text-shadow").map(|(_, v)| v.clone());
            assert_eq!(got.as_deref(), apart.then_some("1px 0 0 currentColor"), "width {w}: the caption starts at {at}");
            seen[apart as usize] = true;
        }
        assert_eq!(seen, [true, true], "both outcomes were exercised");
        // A label held to its left edge starts on a whole pixel: always a pixel apart.
        let (lt, l) = crate::tree::testing::tree_with("Label", "LBL-A", &[("Caption", "Total"), ("Bold", "1"), ("X", "50"), ("Width", "193"), ("TextAlignment", "Left")]);
        assert!(appearance(lt.ctl(l)).iter().any(|(k, v)| k == "text-shadow" && v == "1px 0 0 currentColor"));
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
