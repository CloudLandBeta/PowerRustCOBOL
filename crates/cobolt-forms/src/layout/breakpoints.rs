// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! **Breakpoints** (spec 056 §4.13, R58–R61).
//!
//! A breakpoint is a named range of AVAILABLE WIDTH, chosen afresh on every
//! layout pass from the width of the form's own surface (R61) — never a
//! screen width fixed at start-up.

use std::collections::HashSet;

use crate::model::{Control, PropValue};

/// One entry of a form's breakpoint table.
#[derive(Clone, Debug, PartialEq)]
pub struct Breakpoint {
    pub name: String,
    /// The smallest available width, in form pixels, this breakpoint covers.
    pub min_width: i64,
    /// Its font factor under `FontScaling = Stepped` (R67).
    pub font_factor: f32,
    /// Sparse `(control, property, value)` replacements while it is active
    /// (R59); the design breakpoint carries none.
    pub overrides: Vec<Override>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Override {
    pub control: String,
    pub property: String,
    pub value: PropValue,
}

/// The active breakpoint for `width`: the one COBOL pinned, when it names an
/// entry (R84); otherwise the entry with the largest minimum width not above
/// `width`, and the narrowest entry when every minimum is above it.
pub fn select<'a>(table: &'a [Breakpoint], width: f32, pin: Option<&str>) -> Option<&'a Breakpoint> {
    if let Some(p) = pin.map(str::trim).filter(|p| !p.is_empty()) {
        if let Some(b) = table.iter().find(|b| b.name.eq_ignore_ascii_case(p)) {
            return Some(b);
        }
    }
    table
        .iter()
        .filter(|b| b.min_width as f32 <= width)
        .max_by_key(|b| b.min_width)
        .or_else(|| table.iter().min_by_key(|b| b.min_width))
}

/// The breakpoint that contains the designed width — the base design, which
/// carries no overrides (R59).
pub fn design_breakpoint(table: &[Breakpoint], designed_width: f32) -> Option<&Breakpoint> {
    select(table, designed_width, None)
}

/// The narrowest entry of a table: the one whose layout sets the form's
/// minimum size (R18).
pub fn narrowest(table: &[Breakpoint]) -> Option<&Breakpoint> {
    table.iter().min_by_key(|b| b.min_width)
}

/// Whether a breakpoint may override `property` on a control of `c`'s type
/// (R60): `Visible`, the designed geometry, every layout property the
/// defaults table seeds (anchoring, docking, limits, container and item
/// properties, padding) and `FontSize`. Content never.
pub fn overridable(c: &Control, property: &str) -> bool {
    // `Expanded` (spec 090) is placement: an expanded card takes its siblings'
    // room, so a program's write to it must reach the design the layout reads.
    const GEOMETRY: &[&str] = &["Visible", "X", "Y", "Width", "Height", "FontSize", "Expanded"];
    GEOMETRY.iter().any(|k| k.eq_ignore_ascii_case(property))
        || crate::layout::defaults::control_default(&c.control_type, property).is_some()
}

/// The properties a running program has written on a control, as a list of
/// names. A breakpoint never overrides one of them (R64: a COBOL write, else
/// the active breakpoint, else the design). Never saved.
pub const WRITTEN: &str = "_Written";

/// Whether the program has written `property` on `c`.
pub fn is_written(c: &Control, property: &str) -> bool {
    c.get_prop(WRITTEN)
        .map(|v| v.to_xml_string().split(',').any(|p| p.trim().eq_ignore_ascii_case(property)))
        .unwrap_or(false)
}

/// Record that the program wrote `property` on `c`.
pub fn mark_written(c: &mut Control, property: &str) {
    if is_written(c, property) {
        return;
    }
    let mut list = c.get_prop(WRITTEN).map(|v| v.to_xml_string()).unwrap_or_default();
    if !list.is_empty() {
        list.push(',');
    }
    list.push_str(property);
    c.set_prop(WRITTEN, PropValue::String(list));
}

/// What layout sees while a breakpoint is active: see [`apply_overrides`].
pub struct Seen {
    pub controls: Vec<Control>,
    /// Hidden by an override, with everything inside them (R62).
    pub hidden: HashSet<String>,
    /// Shown by an override although designed hidden.
    pub shown: HashSet<String>,
}

/// What layout sees while `bp` is active (R59, R60, R62): each of its
/// overrides applied to a copy of its control, and every control an override
/// hides — with everything inside it — taken out. `None` when `bp` changes
/// nothing, so the common case copies nothing.
pub fn apply_overrides(controls: &[Control], bp: Option<&Breakpoint>) -> Option<Seen> {
    let bp = bp.filter(|b| !b.overrides.is_empty())?;
    let mut out: Vec<Control> = controls.to_vec();
    for o in &bp.overrides {
        let Some(c) = out.iter_mut().find(|c| c.id.eq_ignore_ascii_case(&o.control)) else { continue };
        if !overridable(c, &o.property) || is_written(c, &o.property) {
            continue;
        }
        let number = || crate::layout::props::number_of(&o.value).map(|v| v.round() as i32);
        match o.property.to_ascii_lowercase().as_str() {
            "visible" => c.visible = o.value.as_bool(),
            "x" => c.rect.x = number().unwrap_or(c.rect.x),
            "y" => c.rect.y = number().unwrap_or(c.rect.y),
            "width" => c.rect.w = number().unwrap_or(c.rect.w).max(0),
            "height" => c.rect.h = number().unwrap_or(c.rect.h).max(0),
            _ => c.set_prop(o.property.clone(), o.value.clone()),
        }
    }
    // Hidden by an override: the control and its descendants.
    let mut hidden: HashSet<String> = bp
        .overrides
        .iter()
        .filter(|o| o.property.eq_ignore_ascii_case("Visible") && !o.value.as_bool())
        .filter_map(|o| out.iter().find(|c| c.id.eq_ignore_ascii_case(&o.control)).map(|c| c.id.clone()))
        .filter(|id| out.iter().find(|c| &c.id == id).is_some_and(|c| !is_written(c, "Visible")))
        .collect();
    loop {
        let before = hidden.len();
        for c in &out {
            if c.parent.as_ref().is_some_and(|p| hidden.contains(p)) {
                hidden.insert(c.id.clone());
            }
        }
        if hidden.len() == before {
            break;
        }
    }
    let shown: HashSet<String> = bp
        .overrides
        .iter()
        .filter(|o| o.property.eq_ignore_ascii_case("Visible") && o.value.as_bool())
        .filter_map(|o| {
            controls
                .iter()
                .find(|c| c.id.eq_ignore_ascii_case(&o.control) && !c.visible && !is_written(c, "Visible"))
                .map(|c| c.id.clone())
        })
        .collect();
    out.retain(|c| !hidden.contains(&c.id));
    Some(Seen { controls: out, hidden, shown })
}

/// `me::Breakpoints` text: `Name:MinWidth:FontFactor;…` (R84).
pub fn to_text(table: &[Breakpoint]) -> String {
    table
        .iter()
        .map(|b| format!("{}:{}:{}", b.name, b.min_width, b.font_factor))
        .collect::<Vec<_>>()
        .join(";")
}

/// Parse `me::Breakpoints` text into a table, keeping the overrides of every
/// entry whose name survives. An unreadable entry is skipped; a missing
/// factor is the default.
pub fn from_text(text: &str, existing: &[Breakpoint]) -> Vec<Breakpoint> {
    text.split(';')
        .filter_map(|entry| {
            let mut parts = entry.split(':').map(str::trim);
            let name = parts.next().filter(|n| !n.is_empty())?.to_owned();
            let min_width = parts.next()?.parse::<i64>().ok()?;
            let font_factor = parts
                .next()
                .and_then(|f| f.parse::<f32>().ok())
                .unwrap_or(crate::layout::defaults::BREAKPOINT_FONT_FACTOR);
            let overrides = existing
                .iter()
                .find(|b| b.name.eq_ignore_ascii_case(&name))
                .map(|b| b.overrides.clone())
                .unwrap_or_default();
            Some(Breakpoint { name, min_width, font_factor, overrides })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::layout::defaults::default_breakpoints;

    /// Compact [0, 600), Medium [600, 1024), Expanded [1024, ∞).
    #[test]
    fn selection_changes_exactly_at_the_thresholds() {
        let t = default_breakpoints();
        let at = |w: f32| select(&t, w, None).unwrap().name.clone();
        let cases = [(0.0, "Compact"), (599.0, "Compact"), (599.9, "Compact"), (600.0, "Medium"),
                     (1023.0, "Medium"), (1024.0, "Expanded"), (4000.0, "Expanded")];
        for (w, want) in cases {
            assert_eq!(at(w), want, "at {w}");
        }
        println!("breakpoint selection: {} widths, each in the range the table names", cases.len());
    }

    #[test]
    fn a_pin_wins_and_an_unknown_pin_is_ignored() {
        let t = default_breakpoints();
        assert_eq!(select(&t, 1500.0, Some("compact")).unwrap().name, "Compact");
        assert_eq!(select(&t, 1500.0, Some("Nope")).unwrap().name, "Expanded");
        assert_eq!(select(&t, 1500.0, Some("  ")).unwrap().name, "Expanded");
        assert!(select(&[], 1500.0, None).is_none());
    }

    #[test]
    fn below_every_minimum_the_narrowest_entry_applies() {
        let t = from_text("Wide:800:1.2;Narrow:400:0.9", &[]);
        assert_eq!(select(&t, 100.0, None).unwrap().name, "Narrow");
    }

    #[test]
    fn the_text_form_round_trips_and_keeps_surviving_overrides() {
        let mut t = default_breakpoints();
        t[0].overrides.push(Override {
            control: "PNL-SIDE".into(),
            property: "Dock".into(),
            value: PropValue::String("Top".into()),
        });
        let text = to_text(&t);
        assert_eq!(text, "Compact:0:1;Medium:600:1;Expanded:1024:1");
        let back = from_text(&text, &t);
        assert_eq!(back, t);
        let renamed = from_text("Phone:0:0.9;Desktop:900:1.25", &t);
        assert_eq!(renamed.len(), 2);
        assert_eq!(renamed[1].font_factor, 1.25);
        assert!(renamed[0].overrides.is_empty(), "overrides follow the name, not the position");
        println!("me::Breakpoints: {text:?} round-trips; renamed entries start clean");
    }
}
