// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Typed reads of the layout properties (spec 056), each falling back to the
//! defaults table when the property is absent or unreadable.
//!
//! Two sources carry them: a control, and the form's own layout bag
//! ([`FormBag`]). Both answer through [`PropSource`], so the solver reads the
//! form exactly as it reads a container.

use std::collections::BTreeMap;

use crate::layout::defaults;
use crate::layout::{Insets, LayoutMode};
use crate::model::{Control, PropValue};

/// Where layout properties are read from.
pub trait PropSource {
    /// The stored value, if any (case-insensitive key).
    fn raw(&self, key: &str) -> Option<&PropValue>;
    /// The seeded default for `key`.
    fn fallback(&self, key: &str) -> Option<PropValue>;

    fn value(&self, key: &str) -> Option<PropValue> {
        self.raw(key).cloned().or_else(|| self.fallback(key))
    }
    fn text(&self, key: &str) -> String {
        self.value(key).map(|v| v.to_xml_string()).unwrap_or_default()
    }
    /// A number: integers, decimals (`"1.5"`), padded text (`" 18 "`).
    fn number(&self, key: &str) -> f32 {
        self.raw(key)
            .and_then(number_of)
            .or_else(|| self.fallback(key).as_ref().and_then(number_of))
            .unwrap_or_default()
    }
    fn flag(&self, key: &str) -> bool {
        self.value(key).map(|v| v.as_bool()).unwrap_or_default()
    }
}

impl PropSource for Control {
    fn raw(&self, key: &str) -> Option<&PropValue> {
        self.get_prop(key)
    }
    fn fallback(&self, key: &str) -> Option<PropValue> {
        defaults::control_default(&self.control_type, key)
    }
}

/// The form's layout bag: `LayoutMode` and its container properties,
/// `Padding`, `FontScaling`, the font-scale limits and the smallest size.
pub struct FormBag<'a>(pub &'a BTreeMap<String, PropValue>);

impl PropSource for FormBag<'_> {
    fn raw(&self, key: &str) -> Option<&PropValue> {
        self.0.get(key).or_else(|| {
            self.0
                .iter()
                .find(|(k, _)| k.eq_ignore_ascii_case(key))
                .map(|(_, v)| v)
        })
    }
    fn fallback(&self, key: &str) -> Option<PropValue> {
        defaults::form_default(key)
    }
}

/// A property value as a number, when it reads as one.
pub fn number_of(v: &PropValue) -> Option<f32> {
    match v {
        PropValue::Int(n) => Some(*n as f32),
        PropValue::Bool(b) => Some(if *b { 1.0 } else { 0.0 }),
        PropValue::String(s) => s.trim().parse::<f32>().ok(),
    }
}

/// A set of parent edges a control follows (R6).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Edges {
    pub top: bool,
    pub bottom: bool,
    pub left: bool,
    pub right: bool,
}

impl Edges {
    /// Parse `"Top,Left,Right"` (any case, any spacing); unknown words are
    /// ignored and an empty string is the empty set.
    pub fn parse(s: &str) -> Self {
        let mut e = Edges::default();
        for w in s.split(',').map(|w| w.trim().to_ascii_lowercase()) {
            match w.as_str() {
                "top" => e.top = true,
                "bottom" => e.bottom = true,
                "left" => e.left = true,
                "right" => e.right = true,
                _ => {}
            }
        }
        e
    }

    /// The canonical text, in `Top,Bottom,Left,Right` order.
    pub fn to_text(self) -> String {
        [
            (self.top, "Top"),
            (self.bottom, "Bottom"),
            (self.left, "Left"),
            (self.right, "Right"),
        ]
        .iter()
        .filter(|(on, _)| *on)
        .map(|(_, n)| *n)
        .collect::<Vec<_>>()
        .join(",")
    }
}

/// A control's `Anchor` edges. A boolean or integer value is the pre-056
/// drag-lock the loader migrates to `Locked` (R35); read before migration it
/// means the default edges.
pub fn anchor(c: &Control) -> Edges {
    match c.get_prop("Anchor") {
        Some(PropValue::String(s)) => Edges::parse(s),
        _ => Edges::parse(defaults::anchor_default(&c.control_type)),
    }
}

/// `Dock` (R11).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Dock {
    None,
    Left,
    Top,
    Right,
    Bottom,
    Fill,
}

impl Dock {
    pub fn parse(s: &str) -> Self {
        match s.trim().to_ascii_lowercase().as_str() {
            "left" => Dock::Left,
            "top" => Dock::Top,
            "right" => Dock::Right,
            "bottom" => Dock::Bottom,
            "fill" => Dock::Fill,
            _ => Dock::None,
        }
    }
}

pub fn dock(c: &Control) -> Dock {
    Dock::parse(&c.text("Dock"))
}

pub fn layout_mode(src: &dyn PropSource) -> LayoutMode {
    match src.text("LayoutMode").trim().to_ascii_lowercase().as_str() {
        "flex" => LayoutMode::Flex,
        "grid" => LayoutMode::Grid,
        "flow" => LayoutMode::Flow,
        _ => LayoutMode::Absolute,
    }
}

/// `[Min, Max]` of one axis; 0 is "no limit" (R16).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Limits {
    pub min: f32,
    pub max: f32,
}

pub fn width_limits(c: &Control) -> Limits {
    Limits {
        min: c.number("MinWidth"),
        max: c.number("MaxWidth"),
    }
}

pub fn height_limits(c: &Control) -> Limits {
    Limits {
        min: c.number("MinHeight"),
        max: c.number("MaxHeight"),
    }
}

/// `Padding`, with any of `PaddingLeft/Top/Right/Bottom` that is set
/// overriding its side (R51).
pub fn padding(src: &dyn PropSource) -> Insets {
    let all = src.number("Padding");
    let side = |k: &str| match src.raw(k).and_then(number_of) {
        Some(v) => v,
        None => all,
    };
    Insets {
        left: side("PaddingLeft"),
        top: side("PaddingTop"),
        right: side("PaddingRight"),
        bottom: side("PaddingBottom"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::ControlType;

    #[test]
    fn edges_parse_any_case_and_order_and_print_canonically() {
        let e = Edges::parse(" right , TOP,left");
        assert_eq!(e, Edges { top: true, bottom: false, left: true, right: true });
        assert_eq!(e.to_text(), "Top,Left,Right");
        assert_eq!(Edges::parse(""), Edges::default());
        println!("anchor text: \" right , TOP,left\" → {}", e.to_text());
    }

    #[test]
    fn a_missing_or_boolean_anchor_reads_as_the_type_default() {
        let mut b = Control::new("B", ControlType::Button, 0, 0);
        b.properties.shift_remove("Anchor");
        assert_eq!(anchor(&b), Edges::parse(defaults::ANCHOR));
        b.set_prop("Anchor", PropValue::Bool(true));
        assert_eq!(anchor(&b), Edges::parse(defaults::ANCHOR), "the legacy lock is not an edge set");
        let s = Control::new("S", ControlType::StatusBar, 0, 0);
        let mut s = s;
        s.properties.shift_remove("Anchor");
        assert_eq!(anchor(&s).to_text(), "Bottom,Left,Right");
    }

    #[test]
    fn numbers_accept_decimals_and_padding_and_fall_back_to_defaults() {
        let mut c = Control::new("P", ControlType::Panel, 0, 0);
        c.set_prop("FlexGrow", PropValue::String(" 1.5 ".into()));
        assert_eq!(c.number("FlexGrow"), 1.5);
        assert_eq!(c.number("FlexShrink"), 1.0, "unset → the table's default");
        c.set_prop("Padding", PropValue::Int(8));
        c.set_prop("PaddingLeft", PropValue::String("2".into()));
        let p = padding(&c);
        assert_eq!((p.left, p.top, p.right, p.bottom), (2.0, 8.0, 8.0, 8.0));
        assert_eq!(layout_mode(&c), LayoutMode::Absolute);
        let bag = BTreeMap::from([("layoutmode".to_owned(), PropValue::String("Grid".into()))]);
        assert_eq!(layout_mode(&FormBag(&bag)), LayoutMode::Grid, "keys are case-insensitive");
        assert_eq!(FormBag(&bag).number("MinFormWidth"), defaults::MIN_FORM_WIDTH as f32);
    }
}
