// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Spec 079 — the look a NEW form starts with, and text that stays readable
//! when a form changes theme.
//!
//! * [`MODERN`] is a control style: flat fields with a thin border, rounded
//!   corners, a light surface, dark text and one type size. It is applied
//!   when a control is CREATED on a form whose `control_style` is
//!   `"modern"` ([`style_control`]), never by `Control::new` — whose defaults
//!   double as "not chosen" markers the painters and the theme switch read, so
//!   changing them would change every form that exists.
//! * [`ensure_text_contrast`] runs after a theme or glass-style switch: a
//!   control whose text would fall below WCAG AA (4.5:1) on the background it
//!   now sits on gets black or white, whichever reads.

use crate::model::{Control, ControlType, Form, GlassStyle, PropValue};

/// The `Form::control_style` value that selects the modern look.
pub const MODERN: &str = "modern";

/// The modern palette. One place, so a template and a dropped control agree.
pub mod palette {
    pub const FORM: &str = "#F4F6F9FF";
    pub const SURFACE: &str = "#FFFFFFFF";
    pub const SURFACE_BORDER: &str = "#DDE3EAFF";
    pub const INPUT_BORDER: &str = "#C5CED8FF";
    pub const TEXT: &str = "#1F2933FF";
    pub const MUTED: &str = "#52606DFF";
    pub const ACCENT: &str = "#2563EBFF";
    pub const ON_ACCENT: &str = "#FFFFFFFF";
    pub const CLEAR: &str = "#00000000";
    pub const FONT: &str = "Arial";
    /// Body text, field text and buttons.
    pub const TEXT_SIZE: i64 = 13;
    /// A form's title.
    pub const TITLE_SIZE: i64 = 20;
    pub const FIELD_RADIUS: i64 = 6;
    pub const SURFACE_RADIUS: i64 = 10;
}

fn set(c: &mut Control, k: &str, v: &str) {
    c.properties.insert(k.into(), PropValue::String(v.into()));
}
fn set_i(c: &mut Control, k: &str, v: i64) {
    c.properties.insert(k.into(), PropValue::Int(v));
}
fn set_b(c: &mut Control, k: &str, v: bool) {
    c.properties.insert(k.into(), PropValue::Bool(v));
}

/// Give one control the modern look, by type. Only appearance: captions,
/// text, geometry, events and bindings are not touched.
pub fn style_control(c: &mut Control) {
    use palette::*;
    let has = |c: &Control, k: &str| c.properties.contains_key(k);
    if has(c, "FontName") {
        set(c, "FontName", FONT);
    }
    if has(c, "FontSize") {
        set_i(c, "FontSize", TEXT_SIZE);
    }
    set_b(c, "BackgroundGradientEnabled", false);
    set_b(c, "ShadowEnabled", false);
    match c.control_type {
        ControlType::Label | ControlType::CheckBox | ControlType::RadioButton => {
            set(c, "BackgroundColor", CLEAR);
            set(c, "ForegroundColor", TEXT);
        }
        ControlType::Button => {
            set(c, "BackgroundColor", ACCENT);
            set(c, "ForegroundColor", ON_ACCENT);
            set(c, "BorderStyle", "None");
            set_i(c, "CornerRadius", FIELD_RADIUS);
        }
        ControlType::TextBox
        | ControlType::ComboBox
        | ControlType::ListBox
        | ControlType::NumericUpDown
        | ControlType::DateTimePicker
        | ControlType::DataGrid => {
            set(c, "BackgroundColor", SURFACE);
            set(c, "ForegroundColor", TEXT);
            set(c, "BorderStyle", "Single");
            set(c, "BorderColor", INPUT_BORDER);
            set_i(c, "CornerRadius", FIELD_RADIUS);
            if has(c, "InnerPadding") {
                set_i(c, "InnerPadding", 6);
            }
        }
        ControlType::Panel | ControlType::GroupBox => {
            set(c, "BackgroundColor", SURFACE);
            set(c, "ForegroundColor", TEXT);
            set(c, "BorderStyle", "Single");
            set(c, "BorderColor", SURFACE_BORDER);
            set_i(c, "CornerRadius", SURFACE_RADIUS);
            // A soft lift, so a card reads as a card on the light form.
            set_b(c, "ShadowEnabled", true);
            set_i(c, "ShadowOpacity", 4);
            set_i(c, "ShadowDistance", 2);
            set_b(c, "ShadowBlur", true);
            set_i(c, "ShadowBlurStrength", 10);
        }
        _ => {
            // Charts, pictures and the rest keep their own faces; they only
            // take the type and the flat background flag above.
        }
    }
}

/// A secondary (outlined) button: white face, the accent's border and text.
pub fn style_secondary_button(c: &mut Control) {
    use palette::*;
    style_control(c);
    set(c, "BackgroundColor", SURFACE);
    set(c, "ForegroundColor", ACCENT);
    set(c, "BorderStyle", "Single");
    set(c, "BorderColor", INPUT_BORDER);
}

/// A form's title: bigger and bold, the body's colour.
pub fn style_title(c: &mut Control) {
    style_control(c);
    set_i(c, "FontSize", palette::TITLE_SIZE);
    set_b(c, "Bold", true);
}

/// A quieter label (a card's caption, a hint).
pub fn style_muted(c: &mut Control) {
    style_control(c);
    set(c, "ForegroundColor", palette::MUTED);
}

/// Make `form` a modern form: its own surface, and every control on it.
pub fn style_form(form: &mut Form) {
    form.control_style = MODERN.into();
    form.glass_style = GlassStyle::Classic;
    form.background_color = palette::FORM.into();
    form.background_gradient_enabled = false;
    form.transparency = 0;
    for c in &mut form.controls {
        style_control(c);
    }
}

// ── Contrast ──────────────────────────────────────────────────────────────

/// `#RRGGBB` or `#RRGGBBAA`, as (r, g, b, a).
fn rgba(hex: &str) -> Option<(u8, u8, u8, u8)> {
    let h = hex.trim().trim_start_matches('#');
    let byte = |i: usize| u8::from_str_radix(h.get(i..i + 2)?, 16).ok();
    match h.len() {
        6 => Some((byte(0)?, byte(2)?, byte(4)?, 255)),
        8 => Some((byte(0)?, byte(2)?, byte(4)?, byte(6)?)),
        _ => None,
    }
}

fn luminance((r, g, b): (u8, u8, u8)) -> f32 {
    let lin = |v: u8| {
        let s = v as f32 / 255.0;
        if s <= 0.04045 {
            s / 12.92
        } else {
            ((s + 0.055) / 1.055).powf(2.4)
        }
    };
    0.2126 * lin(r) + 0.7152 * lin(g) + 0.0722 * lin(b)
}

/// WCAG contrast ratio, 1.0 … 21.0.
pub fn contrast(a: (u8, u8, u8), b: (u8, u8, u8)) -> f32 {
    let (la, lb) = (luminance(a), luminance(b));
    let (hi, lo) = if la >= lb { (la, lb) } else { (lb, la) };
    (hi + 0.05) / (lo + 0.05)
}

/// The contrast WCAG AA asks of body text.
pub const MIN_TEXT_CONTRAST: f32 = 4.5;

/// Controls whose `ForegroundColor` is text the user reads.
fn shows_text(t: &ControlType) -> bool {
    matches!(
        t,
        ControlType::Label
            | ControlType::Button
            | ControlType::CheckBox
            | ControlType::RadioButton
            | ControlType::TextBox
            | ControlType::ComboBox
            | ControlType::ListBox
            | ControlType::NumericUpDown
            | ControlType::DateTimePicker
            | ControlType::GroupBox
    )
}

/// The opaque colour a control's text sits on, when the form says what it is:
/// the control's own background, else its container's (up the chain), else
/// the form's. `None` when it is a glass or themed surface no property names
/// — the painters keep those legible themselves.
pub fn text_background(form: &Form, ctrl: &Control, backdrop_known: bool) -> Option<(u8, u8, u8)> {
    let solid = |hex: &str| {
        let (r, g, b, a) = rgba(hex)?;
        // "#F0F0F0" is Control::new's "not chosen" marker: a glass face.
        (a >= 200 && !hex.trim().eq_ignore_ascii_case(crate::model::DEFAULT_BACKGROUND_COLOR))
            .then_some((r, g, b))
    };
    let mut at = Some(ctrl);
    let mut guard = 0;
    while let Some(c) = at {
        if let Some(PropValue::String(bg)) = c.properties.get("BackgroundColor") {
            if let Some(col) = solid(bg) {
                return Some(col);
            }
        }
        guard += 1;
        if guard > 64 {
            break;
        }
        at = c.parent.as_deref().and_then(|p| form.controls.iter().find(|k| k.id == p));
    }
    if !backdrop_known {
        return None;
    }
    if form.background_gradient_enabled {
        let (a, b) = (
            rgba(&form.background_gradient_start_color)?,
            rgba(&form.background_gradient_end_color)?,
        );
        let mid = |x: u8, y: u8| ((x as u16 + y as u16) / 2) as u8;
        return Some((mid(a.0, b.0), mid(a.1, b.1), mid(a.2, b.2)));
    }
    solid(&form.background_color)
}

/// After a theme or glass-style switch: every control whose text now reads
/// below [`MIN_TEXT_CONTRAST`] on its background takes black or white,
/// whichever reads better. `backdrop_known` is false under a theme that owns
/// the whole look (its surfaces are painted, not stated in properties). The
/// ids of the controls changed, in form order.
pub fn ensure_text_contrast(form: &mut Form, backdrop_known: bool) -> Vec<String> {
    let mut fixes: Vec<(usize, &'static str)> = Vec::new();
    for (i, c) in form.controls.iter().enumerate() {
        if !shows_text(&c.control_type) {
            continue;
        }
        let Some(PropValue::String(fg)) = c.properties.get("ForegroundColor") else { continue };
        let Some((r, g, b, _)) = rgba(fg) else { continue };
        let Some(bg) = text_background(form, c, backdrop_known) else { continue };
        if contrast((r, g, b), bg) >= MIN_TEXT_CONTRAST {
            continue;
        }
        let ink = if contrast((0, 0, 0), bg) >= contrast((255, 255, 255), bg) {
            "#000000FF"
        } else {
            "#FFFFFFFF"
        };
        fixes.push((i, ink));
    }
    let mut changed = Vec::new();
    for (i, ink) in fixes {
        let c = &mut form.controls[i];
        c.properties.insert("ForegroundColor".into(), PropValue::String(ink.into()));
        changed.push(c.id.clone());
    }
    changed
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn white_text_on_a_white_card_turns_black() {
        let mut f = Form::new("F", "F", 400, 300);
        f.background_color = "#FFFFFFFF".into();
        let mut l = Control::new("L", ControlType::Label, 10, 10);
        l.set_prop("BackgroundColor", PropValue::String("#00000000".into()));
        l.set_prop("ForegroundColor", PropValue::String("#FFFFFFFF".into()));
        f.controls.push(l);
        assert_eq!(ensure_text_contrast(&mut f, true), vec!["L".to_string()]);
        assert_eq!(f.controls[0].get_prop("ForegroundColor").unwrap().as_str(), "#000000FF");
    }

    #[test]
    fn a_readable_pair_and_an_unknown_surface_are_left_alone() {
        let mut f = Form::new("F", "F", 400, 300);
        f.background_color = "#00000000".into(); // glass: nothing stated
        let mut l = Control::new("L", ControlType::Label, 10, 10);
        l.set_prop("ForegroundColor", PropValue::String("#777777FF".into()));
        f.controls.push(l);
        assert!(ensure_text_contrast(&mut f, true).is_empty(), "unknown surface");
        let mut b = Control::new("B", ControlType::Button, 10, 50);
        b.set_prop("BackgroundColor", PropValue::String("#2563EBFF".into()));
        b.set_prop("ForegroundColor", PropValue::String("#FFFFFFFF".into()));
        f.controls.push(b);
        assert!(ensure_text_contrast(&mut f, true).is_empty(), "readable already");
    }

    #[test]
    fn the_modern_palette_reads_on_itself() {
        use palette::*;
        let c = |h: &str| {
            let (r, g, b, _) = rgba(h).unwrap();
            (r, g, b)
        };
        for (fg, bg) in [(TEXT, FORM), (TEXT, SURFACE), (MUTED, SURFACE), (MUTED, FORM), (ON_ACCENT, ACCENT), (ACCENT, SURFACE)] {
            let r = contrast(c(fg), c(bg));
            assert!(r >= MIN_TEXT_CONTRAST, "{fg} on {bg}: {r:.2}");
        }
    }
}
