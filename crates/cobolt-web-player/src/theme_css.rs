// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! **What a form theme does to a control** (WASM-016 R14): the look the desktop's painter gives a face under the theme —
//! a rim round it, the radius it falls back to, the fill it has when the developer named none — written as declarations
//! the property layer carries, after what the properties themselves say, so that a page draws a control as the Form
//! Designer draws it.
//!
//! The numbers are the theme's own (`cobolt_forms::theme::spatial`, which the painter's `SpatialTheme` is made from).
//! Only **Spatial** is here: it is the theme the products of this repository are built in, and the one whose look is a
//! whole look rather than a colour (a see-through window, a rim, large radii). Another theme's page is drawn from the
//! properties alone until its look is measured the same way (`specs/WASM-PARITY.md`).

use crate::css::Decls;
use crate::tree::{Ctl, Kind};
use crate::value::{css_color, fmt_number, parse_color};
use cobolt_forms::model::ControlType;
use cobolt_forms::theme::spatial;

fn push(d: &mut Decls, k: &str, v: impl Into<String>) {
    d.push((k.to_owned(), v.into()));
}

fn last<'a>(d: &'a Decls, k: &str) -> Option<&'a str> {
    d.iter().rev().find(|(x, _)| x == k).map(|(_, v)| v.as_str())
}

fn int(c: &Ctl, k: &str) -> Option<i64> {
    c.raw(k).and_then(crate::value::parse_f64).map(|v| v as i64)
}

/// The face a developer named: a `BackgroundColor` that is not the seeded default, nor clear.
fn named_background(c: &Ctl) -> Option<[u8; 4]> {
    let col = parse_color(c.raw("backgroundcolor")?)?;
    let seeded = [cobolt_forms::model::DEFAULT_BACKGROUND_COLOR, cobolt_forms::model::NEUMORPHIC_SURFACE_COLOR, cobolt_forms::model::NEUMORPHIC_DARK_SURFACE_COLOR];
    (col[3] > 0 && !seeded.iter().any(|s| parse_color(s).is_some_and(|d| d == col))).then_some(col)
}

/// What the painter's generic frame draws a control with, under the theme — `None` for a control that paints its own
/// artwork (charts, gauges, shapes, pictures) or whose face the properties take away.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Face {
    Card,
    Input,
    Button,
    /// A label: frameless unless the developer gave it a background.
    Label,
}

fn face_of(c: &Ctl) -> Option<Face> {
    let Kind::Control(t) = &c.kind else { return None };
    let gradient = c.flag("backgroundgradientenabled");
    match t {
        ControlType::Button => Some(Face::Button),
        // A container with no background shown has no face; a gradient is its own branch of the painter, with no rim.
        ControlType::Panel | ControlType::GroupBox => (!c.flag("hidebackground") && !gradient).then_some(Face::Card),
        ControlType::TextBox | ControlType::ComboBox | ControlType::NumericUpDown | ControlType::DateTimePicker => (!gradient).then_some(Face::Input),
        ControlType::Label => (named_background(c).is_some() && !gradient).then_some(Face::Label),
        _ => None,
    }
}

/// The radius the theme gives a control that has none of its own (`paint::themed_corner_radius`): the control's own
/// choice stands; the seeded default (a button's 3, nothing for the rest) is not one.
fn themed_radius(c: &Ctl) -> Option<f64> {
    let Kind::Control(t) = &c.kind else { return None };
    let seeded = if matches!(t, ControlType::Button) { 3 } else { 0 };
    if int(c, "cornerradius").or_else(|| int(c, "borderradius")).is_some_and(|r| r != seeded) {
        return None;
    }
    let card = matches!(t, ControlType::Panel | ControlType::GroupBox);
    let (w, h) = (c.num("width").unwrap_or(0.0), c.num("height").unwrap_or(0.0));
    Some((if card { spatial::CARD_RADIUS } else { spatial::CONTROL_RADIUS } as f64).clamp(0.0, (0.5 * w.min(h)).max(0.0)).floor())
}

/// Spatial's contribution to control `i`: appended to its declarations, so that what the properties said comes first and
/// a declaration here wins only where the properties are silent (a `box-shadow` is the one that is merged).
pub fn spatial_decls(ctls: &[Ctl], i: usize, d: &mut Decls) {
    let c = &ctls[i];
    let Some(face) = face_of(c) else { return };
    let transparency = int(c, "transparency").unwrap_or(0).clamp(0, 100);
    let m = (100 - transparency) as f64 / 100.0;
    let named = named_background(c);

    // The radius (a label's face is invisible more often than not, and the painter insets its text by the radius: left alone).
    if face != Face::Label {
        if let Some(r) = themed_radius(c) {
            push(d, "border-radius", format!("{}px", fmt_number(r)));
            if face == Face::Card {
                push(d, "overflow", "hidden");
            }
        }
    }

    // The fill: the developer's colour when they named one, faded as the painter fades a theme colour; else the theme's.
    if last(d, "background-image").is_none() {
        let fill = match (named, face) {
            (Some(col), _) => Some(spatial::faded(col, m)),
            (None, Face::Card) => Some(spatial::faded(spatial::CARD, m)),
            (None, Face::Input) => Some(spatial::faded(spatial::INPUT, m)),
            (None, Face::Button) => Some(spatial::faded(spatial::BUTTON, m)),
            (None, Face::Label) => None,
        };
        if let Some(([r, g, b], a)) = fill {
            if named.is_some() || last(d, "background-color").is_none() {
                push(d, "background-color", css_color([r, g, b, a]));
            }
        }
    }

    // The rim: one pixel inside the face, two layers of the faint dark line the painter draws, and — on a button — the face's own
    // colour at full strength under them (the painter strokes a button's outline opaque).
    //
    // A border the control draws itself is on the pixel the rim is on, and over it: the painter strokes the face's outline first
    // and the control's own border after, so under a border there is no rim to see. (A page cannot put a shadow under a border: an
    // inset shadow is inside it.)
    let own_border = last(d, "border").is_some_and(|b| !b.trim().is_empty() && b.trim() != "none" && !b.trim().starts_with("0px"));
    let ([gr, gg, gb], a) = spatial::faded(spatial::RIM, m);
    let mut rim: Vec<String> = Vec::new();
    if a > 0 && !own_border {
        let ring = format!("inset 0 0 0 1px {}", css_color([gr, gg, gb, a]));
        rim.push(ring.clone());
        rim.push(ring);
    }
    if face == Face::Button {
        if let Some(col) = named {
            if !rim.is_empty() {
                rim.push(format!("inset 0 0 0 1px {}", css_color([col[0], col[1], col[2], 255])));
            }
        }
    }
    if !rim.is_empty() {
        let mut list = rim;
        if let Some(existing) = last(d, "box-shadow") {
            if existing != "none" {
                list.push(existing.to_owned());
            }
        }
        push(d, "box-shadow", list.join(", "));
    }
}
