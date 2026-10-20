// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Colours.
//!
//! A colour is a straight (not premultiplied) RGBA value of four bytes. On the
//! wire - in the display list and in the COBOL methods - it is one unsigned
//! 32-bit number `0xRRGGBBAA`, which is also how the IDE writes colour
//! properties (`#RRGGBBAA`).

use std::fmt;

/// A straight-alpha RGBA colour.
#[derive(Copy, Clone, PartialEq, Eq, Hash, Default)]
pub struct Rgba {
    /// Red.
    pub r: u8,
    /// Green.
    pub g: u8,
    /// Blue.
    pub b: u8,
    /// Alpha (255 is opaque).
    pub a: u8,
}

impl fmt::Debug for Rgba {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "#{:02X}{:02X}{:02X}{:02X}", self.r, self.g, self.b, self.a)
    }
}

impl Rgba {
    /// Fully transparent black.
    pub const TRANSPARENT: Rgba = Rgba::new(0, 0, 0, 0);
    /// Opaque black.
    pub const BLACK: Rgba = Rgba::new(0, 0, 0, 255);
    /// Opaque white.
    pub const WHITE: Rgba = Rgba::new(255, 255, 255, 255);

    /// From the four channels.
    pub const fn new(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self { r, g, b, a }
    }

    /// From an opaque `0xRRGGBB` triple.
    pub const fn rgb(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b, a: 255 }
    }

    /// From the wire form `0xRRGGBBAA`.
    pub const fn from_u32(v: u32) -> Self {
        Self { r: (v >> 24) as u8, g: (v >> 16) as u8, b: (v >> 8) as u8, a: v as u8 }
    }

    /// To the wire form `0xRRGGBBAA`.
    pub const fn to_u32(self) -> u32 {
        ((self.r as u32) << 24) | ((self.g as u32) << 16) | ((self.b as u32) << 8) | self.a as u32
    }

    /// The same colour with its alpha multiplied by `factor` (clamped to 0..=1).
    pub fn with_alpha_factor(self, factor: f32) -> Self {
        let f = if factor.is_finite() { factor.clamp(0.0, 1.0) } else { 1.0 };
        Self { a: (self.a as f32 * f).round() as u8, ..self }
    }

    /// As the rasteriser's colour.
    pub fn to_skia(self) -> tiny_skia::Color {
        tiny_skia::Color::from_rgba8(self.r, self.g, self.b, self.a)
    }

    /// Parse a CSS-style colour: `#rgb`, `#rgba`, `#rrggbb`, `#rrggbbaa`,
    /// `rgb(r, g, b)`, `rgba(r, g, b, a)` (channels 0-255 or percentages, alpha
    /// 0-1 or a percentage) and the names listed in [`NAMED`]. Case-insensitive.
    pub fn parse(text: &str) -> Option<Rgba> {
        let t = text.trim();
        if t.is_empty() {
            return None;
        }
        if let Some(hex) = t.strip_prefix('#') {
            return parse_hex(hex);
        }
        let lower = t.to_ascii_lowercase();
        if let Some(rest) = lower.strip_prefix("rgba").or_else(|| lower.strip_prefix("rgb")) {
            return parse_functional(rest);
        }
        NAMED.iter().find(|(n, _)| *n == lower).map(|(_, c)| *c)
    }
}

fn parse_hex(hex: &str) -> Option<Rgba> {
    if !hex.bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    let nib = |i: usize| u8::from_str_radix(&hex[i..i + 1], 16).ok();
    let byte = |i: usize| u8::from_str_radix(&hex[i..i + 2], 16).ok();
    match hex.len() {
        3 => Some(Rgba::rgb(nib(0)? * 17, nib(1)? * 17, nib(2)? * 17)),
        4 => Some(Rgba::new(nib(0)? * 17, nib(1)? * 17, nib(2)? * 17, nib(3)? * 17)),
        6 => Some(Rgba::rgb(byte(0)?, byte(2)?, byte(4)?)),
        8 => Some(Rgba::new(byte(0)?, byte(2)?, byte(4)?, byte(6)?)),
        _ => None,
    }
}

fn parse_functional(rest: &str) -> Option<Rgba> {
    let inner = rest.trim().strip_prefix('(')?.strip_suffix(')')?;
    let parts: Vec<&str> = inner.split(|c| c == ',' || c == '/' || c == ' ').filter(|p| !p.is_empty()).collect();
    if parts.len() != 3 && parts.len() != 4 {
        return None;
    }
    let chan = |s: &str| -> Option<u8> {
        if let Some(p) = s.strip_suffix('%') {
            let v: f32 = p.parse().ok()?;
            Some((v.clamp(0.0, 100.0) * 2.55).round() as u8)
        } else {
            let v: f32 = s.parse().ok()?;
            Some(v.clamp(0.0, 255.0).round() as u8)
        }
    };
    let alpha = |s: &str| -> Option<u8> {
        if let Some(p) = s.strip_suffix('%') {
            let v: f32 = p.parse().ok()?;
            Some((v.clamp(0.0, 100.0) * 2.55).round() as u8)
        } else {
            let v: f32 = s.parse().ok()?;
            Some((v.clamp(0.0, 1.0) * 255.0).round() as u8)
        }
    };
    let a = if parts.len() == 4 { alpha(parts[3])? } else { 255 };
    Some(Rgba::new(chan(parts[0])?, chan(parts[1])?, chan(parts[2])?, a))
}

/// The colour names [`Rgba::parse`] knows (the CSS basic set plus a few common
/// extras). Lower-case.
pub const NAMED: &[(&str, Rgba)] = &[
    ("transparent", Rgba::TRANSPARENT),
    ("black", Rgba::rgb(0, 0, 0)),
    ("white", Rgba::rgb(255, 255, 255)),
    ("red", Rgba::rgb(255, 0, 0)),
    ("green", Rgba::rgb(0, 128, 0)),
    ("lime", Rgba::rgb(0, 255, 0)),
    ("blue", Rgba::rgb(0, 0, 255)),
    ("yellow", Rgba::rgb(255, 255, 0)),
    ("cyan", Rgba::rgb(0, 255, 255)),
    ("aqua", Rgba::rgb(0, 255, 255)),
    ("magenta", Rgba::rgb(255, 0, 255)),
    ("fuchsia", Rgba::rgb(255, 0, 255)),
    ("gray", Rgba::rgb(128, 128, 128)),
    ("grey", Rgba::rgb(128, 128, 128)),
    ("silver", Rgba::rgb(192, 192, 192)),
    ("maroon", Rgba::rgb(128, 0, 0)),
    ("olive", Rgba::rgb(128, 128, 0)),
    ("teal", Rgba::rgb(0, 128, 128)),
    ("navy", Rgba::rgb(0, 0, 128)),
    ("purple", Rgba::rgb(128, 0, 128)),
    ("orange", Rgba::rgb(255, 165, 0)),
    ("pink", Rgba::rgb(255, 192, 203)),
    ("brown", Rgba::rgb(165, 42, 42)),
    ("gold", Rgba::rgb(255, 215, 0)),
    ("indigo", Rgba::rgb(75, 0, 130)),
    ("violet", Rgba::rgb(238, 130, 238)),
    ("coral", Rgba::rgb(255, 127, 80)),
    ("crimson", Rgba::rgb(220, 20, 60)),
    ("darkgray", Rgba::rgb(169, 169, 169)),
    ("lightgray", Rgba::rgb(211, 211, 211)),
    ("skyblue", Rgba::rgb(135, 206, 235)),
    ("tomato", Rgba::rgb(255, 99, 71)),
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wire_form_round_trips() {
        for v in [0u32, 0xFF0000FF, 0x12345678, 0xFFFFFFFF] {
            assert_eq!(Rgba::from_u32(v).to_u32(), v);
        }
        assert_eq!(Rgba::from_u32(0xFF8000C0), Rgba::new(255, 128, 0, 192));
    }

    #[test]
    fn parses_the_css_forms() {
        assert_eq!(Rgba::parse("#f80"), Some(Rgba::rgb(255, 136, 0)));
        assert_eq!(Rgba::parse("#f808"), Some(Rgba::new(255, 136, 0, 136)));
        assert_eq!(Rgba::parse("#FF8000"), Some(Rgba::rgb(255, 128, 0)));
        assert_eq!(Rgba::parse("#FF800080"), Some(Rgba::new(255, 128, 0, 128)));
        assert_eq!(Rgba::parse("rgb(10, 20, 30)"), Some(Rgba::rgb(10, 20, 30)));
        assert_eq!(Rgba::parse("rgba(10,20,30,0.5)"), Some(Rgba::new(10, 20, 30, 128)));
        assert_eq!(Rgba::parse("rgb(100%, 0%, 50%)"), Some(Rgba::rgb(255, 0, 128)));
        assert_eq!(Rgba::parse("Orange"), Some(Rgba::rgb(255, 165, 0)));
        assert_eq!(Rgba::parse("transparent"), Some(Rgba::TRANSPARENT));
    }

    #[test]
    fn rejects_garbage() {
        for bad in ["", "#", "#12", "#12345", "#gg0000", "rgb(1,2)", "rgb(1,2,3,4,5)", "notacolour", "rgb 1 2 3"] {
            assert_eq!(Rgba::parse(bad), None, "{bad:?}");
        }
    }

    #[test]
    fn alpha_factor_is_clamped() {
        assert_eq!(Rgba::WHITE.with_alpha_factor(0.5).a, 128);
        assert_eq!(Rgba::WHITE.with_alpha_factor(2.0).a, 255);
        assert_eq!(Rgba::WHITE.with_alpha_factor(-1.0).a, 0);
        assert_eq!(Rgba::WHITE.with_alpha_factor(f32::NAN).a, 255);
    }
}
