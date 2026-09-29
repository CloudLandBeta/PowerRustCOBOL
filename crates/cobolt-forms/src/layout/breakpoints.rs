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

use crate::model::PropValue;

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
