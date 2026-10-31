// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! The caption of a TextBox (spec 110): a label that belongs to the box.
//!
//! A caption has no coordinates of its own. Its rectangle is computed from the
//! box and the measured text every time, so moving or resizing the box keeps
//! the distance by construction. Everything here is pure: no window, no
//! renderer, no font atlas. The caller measures the text and passes the size.

/// Layout unit of the form. A type of its own so that a responsive shell that
/// later changes the unit changes it in one place (spec 110, risks).
pub type Unit = f32;

/// An axis-aligned rectangle: `x`, `y` is the top-left corner.
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct CapRect {
    pub x: Unit,
    pub y: Unit,
    pub w: Unit,
    pub h: Unit,
}

impl CapRect {
    pub const fn new(x: Unit, y: Unit, w: Unit, h: Unit) -> Self {
        Self { x, y, w, h }
    }

    /// The smallest rectangle holding both.
    pub fn union(self, other: CapRect) -> CapRect {
        let x0 = self.x.min(other.x);
        let y0 = self.y.min(other.y);
        let x1 = (self.x + self.w).max(other.x + other.w);
        let y1 = (self.y + self.h).max(other.y + other.h);
        CapRect::new(x0, y0, x1 - x0, y1 - y0)
    }
}

/// The measured size of the caption text.
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct CapSize {
    pub w: Unit,
    pub h: Unit,
}

/// The side of the box the caption sits on.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum CaptionPosition {
    #[default]
    Top,
    Bottom,
    Left,
    Right,
}

/// Horizontal alignment against the box (used by Top and Bottom).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum HAlign {
    #[default]
    Left,
    Center,
    Right,
}

/// Vertical alignment against the box (used by Left and Right).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum VAlign {
    Top,
    #[default]
    Middle,
    Bottom,
}

/// A resolved font.
#[derive(Clone, Debug, PartialEq)]
pub struct CapFont {
    pub name: String,
    pub size: f32,
    pub bold: bool,
    pub italic: bool,
    pub underline: bool,
}

/// `None` = inherited from the TextBox, `Some` = set. Inheritance is per
/// attribute and live: it is resolved against the host each time it is read.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct FontOverride {
    pub name: Option<String>,
    pub size: Option<f32>,
    pub bold: Option<bool>,
    pub italic: Option<bool>,
    pub underline: Option<bool>,
}

impl FontOverride {
    pub fn resolve(&self, host: &CapFont) -> CapFont {
        CapFont {
            name: self.name.clone().unwrap_or_else(|| host.name.clone()),
            size: self.size.unwrap_or(host.size),
            bold: self.bold.unwrap_or(host.bold),
            italic: self.italic.unwrap_or(host.italic),
            underline: self.underline.unwrap_or(host.underline),
        }
    }
}

/// The default distance between box and caption.
pub const DEFAULT_GAP: u16 = 4;

/// The caption state. Empty `text` = no caption: nothing is drawn or reserved.
#[derive(Clone, Debug, PartialEq)]
pub struct Caption {
    pub text: String,
    pub position: CaptionPosition,
    pub halign: HAlign,
    pub valign: VAlign,
    pub gap: u16,
    pub font: FontOverride,
    /// `None` = inherits the box's ForeColor.
    pub fore_color: Option<String>,
    /// `None` = transparent. The box's background is never inherited.
    pub back_color: Option<String>,
}

impl Default for Caption {
    fn default() -> Self {
        Self {
            text: String::new(),
            position: CaptionPosition::default(),
            halign: HAlign::default(),
            valign: VAlign::default(),
            gap: DEFAULT_GAP,
            font: FontOverride::default(),
            fore_color: None,
            back_color: None,
        }
    }
}

/// The caption rectangle from the box and the measured text; `None` when the
/// caption is empty. The caption is exactly the size of its text, and it
/// overflows the box (never clipped, never ellipsised) when wider.
pub fn caption_rect(c: &Caption, host: CapRect, text: CapSize) -> Option<CapRect> {
    if c.text.is_empty() {
        return None;
    }
    let g = Unit::from(c.gap);
    let (x, y) = match c.position {
        CaptionPosition::Top | CaptionPosition::Bottom => {
            let x = match c.halign {
                HAlign::Left => host.x,
                HAlign::Center => host.x + (host.w - text.w) / 2.0,
                HAlign::Right => host.x + host.w - text.w,
            };
            let y = if c.position == CaptionPosition::Top {
                host.y - g - text.h
            } else {
                host.y + host.h + g
            };
            (x, y)
        }
        CaptionPosition::Left | CaptionPosition::Right => {
            let y = match c.valign {
                VAlign::Top => host.y,
                VAlign::Middle => host.y + (host.h - text.h) / 2.0,
                VAlign::Bottom => host.y + host.h - text.h,
            };
            let x = if c.position == CaptionPosition::Left {
                host.x - g - text.w
            } else {
                host.x + host.w + g
            };
            (x, y)
        }
    };
    Some(CapRect::new(x, y, text.w, text.h))
}

/// The box and its caption together; equal to `host` without a caption.
pub fn outer_bounds(c: &Caption, host: CapRect, text: CapSize) -> CapRect {
    match caption_rect(c, host, text) {
        Some(r) => host.union(r),
        None => host,
    }
}

/// The mnemonic of a caption text: the text to draw (`&&` becomes `&`, a lone
/// `&` is dropped) and the char index of the underlined letter, if any.
pub fn parse_mnemonic(text: &str) -> (String, Option<usize>) {
    let mut out = String::new();
    let mut mark = None;
    let mut chars = text.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch == '&' {
            match chars.peek() {
                Some('&') => {
                    chars.next();
                    out.push('&');
                }
                Some(_) if mark.is_none() => mark = Some(out.chars().count()),
                _ => {}
            }
        } else {
            out.push(ch);
        }
    }
    (out, mark)
}

/// Property names that are inherited from the TextBox until set (spec 110 §4).
pub const INHERITABLE: [&str; 6] = [
    "CaptionFontName",
    "CaptionFontSize",
    "CaptionFontBold",
    "CaptionFontItalic",
    "CaptionFontUnderline",
    "CaptionForeColor",
];

/// The value a running program writes to put an attribute back to inherited
/// (`ResetCaption`). It travels the ordinary property channel; the live merge
/// turns it into the removal of the property.
pub const INHERIT: &str = "\u{1}inherit";

/// Every caption property of a TextBox, set or inherited.
pub const PROPERTY_NAMES: [&str; 12] = [
    "CaptionText",
    "CaptionPosition",
    "CaptionAlign",
    "CaptionVAlign",
    "CaptionGap",
    "CaptionFontName",
    "CaptionFontSize",
    "CaptionFontBold",
    "CaptionFontItalic",
    "CaptionFontUnderline",
    "CaptionForeColor",
    "CaptionBackColor",
];

/// The inheritable attribute `name` spells (any case), in its canonical form.
pub fn inheritable(name: &str) -> Option<&'static str> {
    INHERITABLE.iter().copied().find(|n| n.eq_ignore_ascii_case(name.trim()))
}

/// The TextBox property an inheritable caption attribute falls back to.
pub fn host_property(name: &str) -> Option<&'static str> {
    Some(match inheritable(name)? {
        "CaptionFontName" => "FontName",
        "CaptionFontSize" => "FontSize",
        "CaptionFontBold" => "Bold",
        "CaptionFontItalic" => "Italic",
        "CaptionFontUnderline" => "Underline",
        _ => "ForegroundColor",
    })
}

fn parse_position(s: &str) -> CaptionPosition {
    match s.to_ascii_lowercase().as_str() {
        "bottom" => CaptionPosition::Bottom,
        "left" => CaptionPosition::Left,
        "right" => CaptionPosition::Right,
        _ => CaptionPosition::Top,
    }
}

impl Caption {
    /// The caption as a form stores it, read through any property lookup (the
    /// desktop's `Control`, the web player's tree). A name the lookup does not
    /// know is an attribute that is not set.
    pub fn from_props(get: &dyn Fn(&str) -> Option<String>) -> Caption {
        let flag = |k: &str| get(k).map(|v| matches!(v.trim().to_ascii_lowercase().as_str(), "1" | "true" | "yes" | "on"));
        Caption {
            text: get("CaptionText").unwrap_or_default(),
            position: get("CaptionPosition").map_or_else(Default::default, |v| parse_position(&v)),
            halign: match get("CaptionAlign").unwrap_or_default().to_ascii_lowercase().as_str() {
                "center" => HAlign::Center,
                "right" => HAlign::Right,
                _ => HAlign::Left,
            },
            valign: match get("CaptionVAlign").unwrap_or_default().to_ascii_lowercase().as_str() {
                "top" => VAlign::Top,
                "bottom" => VAlign::Bottom,
                _ => VAlign::Middle,
            },
            gap: get("CaptionGap")
                .and_then(|v| v.trim().parse::<i64>().ok())
                .map_or(DEFAULT_GAP, |n| n.clamp(0, i64::from(u16::MAX)) as u16),
            font: FontOverride {
                name: get("CaptionFontName"),
                size: get("CaptionFontSize").and_then(|v| v.trim().parse::<f32>().ok()),
                bold: flag("CaptionFontBold"),
                italic: flag("CaptionFontItalic"),
                underline: flag("CaptionFontUnderline"),
            },
            fore_color: get("CaptionForeColor"),
            back_color: get("CaptionBackColor").filter(|c| !c.eq_ignore_ascii_case("transparent")),
        }
    }
}

impl crate::model::Control {
    /// The caption as stored: an attribute that is not set stays `None`.
    pub fn caption(&self) -> Caption {
        Caption::from_props(&|k| {
            self.get_prop(k).map(|v| match v {
                crate::model::PropValue::String(s) => s.clone(),
                crate::model::PropValue::Int(n) => n.to_string(),
                crate::model::PropValue::Bool(b) => (if *b { "1" } else { "0" }).to_string(),
            })
        })
    }

    /// The font the caption is drawn with: its own attributes over the box's.
    pub fn caption_font(&self) -> CapFont {
        let host = CapFont {
            name: self.get_prop("FontName").map_or_else(String::new, |v| v.as_str().to_string()),
            size: crate::layout::fonts::resolve_font_size(self, 14.0, 4.0, 200.0),
            bold: self.get_prop("Bold").is_some_and(|v| v.as_bool()),
            italic: self.get_prop("Italic").is_some_and(|v| v.as_bool()),
            underline: self.get_prop("Underline").is_some_and(|v| v.as_bool()),
        };
        self.caption().font.resolve(&host)
    }

    /// The colour the caption is drawn with: its own, else the box's.
    pub fn caption_fore_color(&self) -> String {
        self.caption().fore_color.unwrap_or_else(|| {
            self.get_prop("ForegroundColor").map_or_else(String::new, |v| v.as_str().to_string())
        })
    }

    /// Is this attribute set (true) or inherited (false)? `None` for a name
    /// that is not inheritable.
    pub fn caption_is_set(&self, name: &str) -> Option<bool> {
        INHERITABLE
            .iter()
            .find(|n| n.eq_ignore_ascii_case(name))
            .map(|n| self.get_prop(n).is_some())
    }

    /// Back to inherited: one attribute, or every font and colour attribute
    /// when `name` is `None`. Returns false for a name that is not inheritable.
    pub fn reset_caption(&mut self, name: Option<&str>) -> bool {
        match name {
            None => {
                for n in INHERITABLE {
                    self.properties.shift_remove(n);
                }
                true
            }
            Some(n) => match INHERITABLE.iter().find(|k| k.eq_ignore_ascii_case(n)) {
                Some(k) => {
                    self.properties.shift_remove(*k);
                    true
                }
                None => false,
            },
        }
    }
}

/// A caption ready to draw or hit-test, in the same coordinates as the box's
/// `rect` (the form's, or the host container's, local space).
#[derive(Clone, Debug, PartialEq)]
pub struct CaptionLayout {
    /// The text to draw: `&&` already `&`, the mnemonic marker removed.
    pub text: String,
    /// Char index of the underlined mnemonic letter.
    pub mnemonic: Option<usize>,
    pub rect: CapRect,
    pub font: CapFont,
    /// Resolved ink, before the disabled colour is applied.
    pub fore_color: String,
    pub back_color: Option<String>,
}

/// Lay a caption out against its box: the one routine the desktop painter, the
/// hit test and the web player share. The text is measured with
/// [`crate::textmetrics`]; `host_font` and `host_fore` are what the TextBox
/// itself uses, which an inherited attribute resolves to.
pub fn layout_caption(cap: &Caption, host_font: &CapFont, host_fore: &str, host: CapRect) -> Option<CaptionLayout> {
    let (text, mnemonic) = parse_mnemonic(&cap.text);
    if text.is_empty() {
        return None;
    }
    let font = cap.font.resolve(host_font);
    let size = f64::from(font.size.max(1.0));
    let measured = CapSize {
        w: crate::textmetrics::text_width(&text, size) as Unit,
        h: crate::textmetrics::row_height(size) as Unit,
    };
    let rect = caption_rect(cap, host, measured)?;
    Some(CaptionLayout {
        text,
        mnemonic,
        rect,
        font,
        fore_color: cap.fore_color.clone().unwrap_or_else(|| host_fore.to_string()),
        back_color: cap.back_color.clone(),
    })
}

impl crate::model::Control {
    /// The caption of a TextBox laid out, or `None` when there is none. The
    /// text is measured with [`crate::textmetrics`], the one measure the
    /// designer and the runtime share, so the two never disagree.
    pub fn caption_layout(&self) -> Option<CaptionLayout> {
        if self.control_type != crate::model::ControlType::TextBox {
            return None;
        }
        let host_font = CapFont {
            name: self.get_prop("FontName").map_or_else(String::new, |v| v.as_str().to_string()),
            size: crate::layout::fonts::resolve_font_size(self, 14.0, 4.0, 200.0),
            bold: self.get_prop("Bold").is_some_and(|v| v.as_bool()),
            italic: self.get_prop("Italic").is_some_and(|v| v.as_bool()),
            underline: self.get_prop("Underline").is_some_and(|v| v.as_bool()),
        };
        let host_fore = self.get_prop("ForegroundColor").map_or_else(String::new, |v| v.as_str().to_string());
        let r = self.rect;
        layout_caption(
            &self.caption(),
            &host_font,
            &host_fore,
            CapRect::new(r.x as Unit, r.y as Unit, r.w as Unit, r.h as Unit),
        )
    }

    /// The box and its caption together (spec 110 outer bounds).
    pub fn outer_rect(&self) -> CapRect {
        let r = self.rect;
        let inner = CapRect::new(r.x as Unit, r.y as Unit, r.w as Unit, r.h as Unit);
        match self.caption_layout() {
            Some(l) => inner.union(l.rect),
            None => inner,
        }
    }
}

/// A thing about a caption worth telling the developer; none of them stops a save.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CaptionWarning {
    /// The caption (or the box with it) reaches outside the form.
    OutsideForm,
    /// The caption lies over another control.
    Overlaps(String),
    /// Another caption on the form uses the same mnemonic letter.
    RepeatedMnemonic(char),
}

/// The warnings for every captioned TextBox of a form of `form_w` x `form_h`, as `(control id, warning)`.
pub fn caption_warnings(controls: &[crate::model::Control], form_w: Unit, form_h: Unit) -> Vec<(String, CaptionWarning)> {
    let mut out = Vec::new();
    let mut letters: Vec<(String, char)> = Vec::new();
    for c in controls {
        let Some(l) = c.caption_layout() else { continue };
        let r = l.rect;
        if r.x < 0.0 || r.y < 0.0 || r.x + r.w > form_w || r.y + r.h > form_h {
            out.push((c.id.clone(), CaptionWarning::OutsideForm));
        }
        // A container holding the box is the box's ground, not something the caption is over.
        let mut ancestors = Vec::new();
        let mut up = c.parent.clone();
        while let Some(p) = up {
            up = controls.iter().find(|x| x.id == p).and_then(|x| x.parent.clone());
            ancestors.push(p);
            if ancestors.len() > 64 {
                break;
            }
        }
        for o in controls {
            if o.id == c.id || ancestors.contains(&o.id) || o.control_type.is_non_visual() || !o.visible {
                continue;
            }
            let b = &o.rect;
            if r.x < (b.x + b.w) as Unit && r.x + r.w > b.x as Unit && r.y < (b.y + b.h) as Unit && r.y + r.h > b.y as Unit {
                out.push((c.id.clone(), CaptionWarning::Overlaps(o.id.clone())));
                break;
            }
        }
        if let Some(ch) = l.mnemonic.and_then(|i| l.text.chars().nth(i)) {
            letters.push((c.id.clone(), ch.to_ascii_lowercase()));
        }
    }
    for (id, ch) in &letters {
        if letters.iter().filter(|(_, o)| o == ch).count() > 1 {
            out.push((id.clone(), CaptionWarning::RepeatedMnemonic(*ch)));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const BOX: CapRect = CapRect::new(100.0, 50.0, 200.0, 22.0);
    const TEXT: CapSize = CapSize { w: 60.0, h: 15.0 };

    fn at(position: CaptionPosition, halign: HAlign, valign: VAlign) -> CapRect {
        let c = Caption {
            text: "x".into(),
            position,
            halign,
            valign,
            ..Caption::default()
        };
        caption_rect(&c, BOX, TEXT).unwrap()
    }

    #[test]
    fn the_acceptance_table() {
        use CaptionPosition::*;
        let cases = [
            (at(Top, HAlign::Left, VAlign::Middle), (100.0, 31.0)),
            (at(Top, HAlign::Center, VAlign::Middle), (170.0, 31.0)),
            (at(Top, HAlign::Right, VAlign::Middle), (240.0, 31.0)),
            (at(Bottom, HAlign::Left, VAlign::Middle), (100.0, 76.0)),
            (at(Left, HAlign::Left, VAlign::Middle), (36.0, 53.5)),
            (at(Left, HAlign::Left, VAlign::Top), (36.0, 50.0)),
            (at(Right, HAlign::Left, VAlign::Bottom), (304.0, 57.0)),
        ];
        for (r, (x, y)) in cases {
            assert_eq!((r.x, r.y, r.w, r.h), (x, y, 60.0, 15.0));
        }
    }

    #[test]
    fn no_text_no_caption() {
        let c = Caption::default();
        assert_eq!(caption_rect(&c, BOX, TEXT), None);
        assert_eq!(outer_bounds(&c, BOX, TEXT), BOX);
    }

    #[test]
    fn moving_the_box_moves_the_caption_by_the_same_vector() {
        use CaptionPosition::*;
        for p in [Top, Bottom, Left, Right] {
            let c = Caption { text: "x".into(), position: p, ..Caption::default() };
            let a = caption_rect(&c, BOX, TEXT).unwrap();
            let moved = CapRect::new(BOX.x + 13.0, BOX.y - 7.0, BOX.w, BOX.h);
            let b = caption_rect(&c, moved, TEXT).unwrap();
            assert_eq!((b.x - a.x, b.y - a.y), (13.0, -7.0));
        }
    }

    #[test]
    fn top_right_keeps_the_right_edge_when_the_box_is_resized() {
        let c = Caption {
            text: "x".into(),
            halign: HAlign::Right,
            ..Caption::default()
        };
        let wide = CapRect::new(BOX.x, BOX.y, 320.0, BOX.h);
        let r = caption_rect(&c, wide, TEXT).unwrap();
        assert_eq!(r.x + r.w, wide.x + wide.w);
    }

    #[test]
    fn wide_text_overflows_evenly_when_centred() {
        let c = Caption {
            text: "x".into(),
            halign: HAlign::Center,
            ..Caption::default()
        };
        let r = caption_rect(&c, BOX, CapSize { w: 300.0, h: 15.0 }).unwrap();
        assert_eq!((BOX.x - r.x, (r.x + r.w) - (BOX.x + BOX.w)), (50.0, 50.0));
    }

    #[test]
    fn outer_bounds_hold_both() {
        let c = Caption { text: "x".into(), ..Caption::default() };
        assert_eq!(outer_bounds(&c, BOX, TEXT), CapRect::new(100.0, 31.0, 200.0, 41.0));
    }

    #[test]
    fn font_inherits_per_attribute() {
        let host = CapFont {
            name: "Segoe UI".into(),
            size: 9.0,
            bold: false,
            italic: false,
            underline: false,
        };
        let o = FontOverride { bold: Some(true), ..FontOverride::default() };
        let bigger = CapFont { size: 11.0, ..host.clone() };
        let r = o.resolve(&bigger);
        assert_eq!((r.name.as_str(), r.size, r.bold), ("Segoe UI", 11.0, true));
        let fixed = FontOverride { size: Some(9.0), ..FontOverride::default() };
        assert_eq!(fixed.resolve(&bigger).size, 9.0);
    }

    #[test]
    fn mnemonics() {
        assert_eq!(parse_mnemonic("&Nome:"), ("Nome:".to_string(), Some(0)));
        assert_eq!(parse_mnemonic("R&&D"), ("R&D".to_string(), None));
        assert_eq!(parse_mnemonic("Cu&stomer"), ("Customer".to_string(), Some(2)));
    }
}

#[cfg(test)]
mod control_tests {
    use crate::model::{Control, ControlType, PropValue};

    fn tb() -> Control {
        let mut c = Control::new("TXT", ControlType::TextBox, 10, 10);
        c.set_prop("FontName", PropValue::String("Segoe UI".into()));
        c.set_prop("FontSize", PropValue::Int(9));
        c
    }

    #[test]
    fn nothing_is_set_on_a_new_textbox() {
        let c = tb();
        assert!(c.caption().text.is_empty());
        for n in super::INHERITABLE {
            assert_eq!(c.caption_is_set(n), Some(false), "{n}");
            assert!(c.get_prop(n).is_none(), "{n} must not be seeded");
        }
    }

    #[test]
    fn inheritance_is_live_and_per_attribute() {
        let mut c = tb();
        c.set_prop("CaptionFontBold", PropValue::Bool(true));
        c.set_prop("FontSize", PropValue::Int(11));
        let f = c.caption_font();
        assert_eq!((f.name.as_str(), f.size, f.bold), ("Segoe UI", 11.0, true));
        c.set_prop("CaptionFontSize", PropValue::Int(12));
        c.set_prop("FontSize", PropValue::Int(20));
        assert_eq!(c.caption_font().size, 12.0);
    }

    #[test]
    fn a_value_equal_to_the_hosts_is_still_set() {
        let mut c = tb();
        c.set_prop("CaptionFontSize", PropValue::Int(9));
        c.set_prop("FontSize", PropValue::Int(14));
        assert_eq!(c.caption_is_set("CaptionFontSize"), Some(true));
        assert_eq!(c.caption_font().size, 9.0);
    }

    #[test]
    fn reset_one_or_all() {
        let mut c = tb();
        c.set_prop("CaptionFontSize", PropValue::Int(12));
        c.set_prop("CaptionFontBold", PropValue::Bool(true));
        c.set_prop("CaptionForeColor", PropValue::String("#FF0000".into()));
        assert!(c.reset_caption(Some("CaptionFontSize")));
        assert_eq!(c.caption_is_set("CaptionFontSize"), Some(false));
        assert_eq!(c.caption_is_set("CaptionFontBold"), Some(true));
        assert!(c.reset_caption(None));
        assert!(super::INHERITABLE.iter().all(|n| c.caption_is_set(n) == Some(false)));
        assert!(!c.reset_caption(Some("CaptionText")));
        assert_eq!(c.caption_is_set("CaptionText"), None);
    }
}

#[cfg(test)]
mod layout_tests {
    use crate::model::{Control, ControlType, PropValue};

    #[test]
    fn the_caption_follows_the_box_and_the_outer_rect_holds_both() {
        let mut c = Control::new("T", ControlType::TextBox, 100, 50);
        c.rect.w = 200;
        c.rect.h = 22;
        assert!(c.caption_layout().is_none());
        c.set_prop("CaptionText", PropValue::String("&Nome:".into()));
        let a = c.caption_layout().unwrap();
        assert_eq!((a.text.as_str(), a.mnemonic), ("Nome:", Some(0)));
        assert!(a.rect.y + a.rect.h < 50.0 + 0.001, "Top: above the box");
        c.rect.x += 30;
        c.rect.y += 10;
        let b = c.caption_layout().unwrap();
        assert_eq!((b.rect.x - a.rect.x, b.rect.y - a.rect.y), (30.0, 10.0));
        let o = c.outer_rect();
        assert!(o.y < c.rect.y as f32 && o.w >= 200.0);
    }
}

#[cfg(test)]
mod warning_tests {
    use super::*;
    use crate::model::{Control, ControlType, PropValue};

    fn tb(id: &str, x: i32, y: i32, text: &str) -> Control {
        let mut c = Control::new(id, ControlType::TextBox, x, y);
        c.rect.w = 100;
        c.rect.h = 22;
        c.set_prop("CaptionText", PropValue::String(text.into()));
        c
    }

    #[test]
    fn the_three_warnings() {
        let a = tb("A", 10, 10, "&Nome:"); // Top caption at y < 10: outside the form
        let b = tb("B", 10, 100, "N&ome:"); // same mnemonic letter as A: 'o'? no: 'o' vs 'n'
        let c = tb("C", 10, 130, "&Outro:");
        let d = tb("D", 10, 160, "T&odo:"); // mnemonic 'o', as C's
        let w = caption_warnings(&[a, b, c, d], 640.0, 480.0);
        assert!(w.contains(&("A".into(), CaptionWarning::OutsideForm)));
        assert!(w.contains(&("C".into(), CaptionWarning::RepeatedMnemonic('o'))));
        assert!(w.contains(&("D".into(), CaptionWarning::RepeatedMnemonic('o'))));
        // B's caption lies over A's box? A is at y 10..32, B's caption at y ~81..96: no.
        assert!(!w.iter().any(|(id, k)| id == "B" && matches!(k, CaptionWarning::Overlaps(_))));
        // A caption over a neighbour's box.
        let n = tb("N", 10, 300, "");
        let mut m = tb("M", 10, 315, "&Mid:"); // Top caption around y 296..311, over N (300..322)
        m.rect.y = 315;
        let w = caption_warnings(&[n, m], 640.0, 480.0);
        assert!(w.contains(&("M".into(), CaptionWarning::Overlaps("N".into()))), "{w:?}");
        // The container that holds the box is not "overlapped".
        let mut p = Control::new("P", ControlType::Panel, 0, 200);
        p.rect.w = 400;
        p.rect.h = 200;
        let mut t = tb("T", 20, 240, "&Cep:");
        t.parent = Some("P".into());
        assert!(caption_warnings(&[p, t], 640.0, 480.0).is_empty());
    }
}
