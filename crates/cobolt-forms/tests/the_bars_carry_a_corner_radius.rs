#![cfg(feature = "render")]
//! Spec 016 **Q4, settled** (operator, 2026-09-03): `MenuBar`, `ToolBar` and
//! `StatusBar` carry a corner radius like every other control.
//!
//! Q4's recommendation excluded them, and `Label`, for having "no real frame".
//! That reasoning had already failed once: a Label kept losing its corner
//! radius, and the fix was to seed the property so it exists. A bar has a frame
//! like anything else — at `Transparency = 100` that frame is invisible, not
//! absent, and its corners stay meaningful.
//!
//! Two halves, and the first is the one that was actually broken: the painter
//! always honoured `CornerRadius` on a bar, but `Control::new` never seeded it,
//! and the inspector shows a row only for a property that is present. So the
//! developer could not set what the renderer was ready to draw.

use cobolt_forms::model::{Control, ControlType, PropValue, Rect as MRect};
use cobolt_forms::paint::draw_control;
use egui::{Pos2, Vec2};

/// The three bars, and the radius each seeds. A ToolBar already had its own
/// seed of 10 and keeps it — the shared default only fills in where a control
/// has no opinion, so this change gives MenuBar and StatusBar a row without
/// re-flattening a bar that already had one.
const BARS: [(&str, ControlType, i64); 3] = [
    ("MenuBar", ControlType::MenuBar, 0),
    ("ToolBar", ControlType::ToolBar, 10),
    ("StatusBar", ControlType::StatusBar, 0),
];

/// The largest corner radius `draw_control` painted for `ctrl`.
fn painted_corner(ctrl: &Control) -> u8 {
    let ctx = egui::Context::default();
    let mut input = egui::RawInput::default();
    input.screen_rect = Some(egui::Rect::from_min_size(Pos2::ZERO, Vec2::new(400.0, 300.0)));
    fn walk(s: &egui::Shape, p: &mut Vec<u8>) {
        match s {
            egui::Shape::Vec(v) => v.iter().for_each(|s| walk(s, p)),
            egui::Shape::Rect(r) => {
                let c = r.corner_radius;
                p.push(c.nw.max(c.ne).max(c.sw).max(c.se));
            }
            _ => {}
        }
    }
    let mut radii = Vec::new();
    let mut full = ctx.run_ui(input, |root| {
        egui::CentralPanel::default()
            .frame(egui::Frame::NONE)
            .show_inside(root, |ui| {
                draw_control(&ui.painter().clone(), Pos2::ZERO, ctrl, false, true, 1.0, 1.0, None);
            });
    });
    full.textures_delta.clear();
    for cs in &full.shapes {
        walk(&cs.shape, &mut radii);
    }
    radii.into_iter().max().unwrap_or(0)
}

/// A bar with a background colour, which is what makes its frame visible at
/// all: a ToolBar ships at `Transparency = 100` so a bare one reads as buttons
/// on the form, and an invisible frame has no corners to look at.
fn bar(kind: ControlType, radius: Option<i64>) -> Control {
    let mut c = Control::new("C-1", kind, 10, 10);
    c.rect = MRect::new(10, 10, 200, 40);
    c.set_prop("BackgroundColor", PropValue::String("#3355AAFF".to_owned()));
    if let Some(r) = radius {
        c.set_prop("CornerRadius", PropValue::Int(r));
    }
    c
}

#[test]
fn a_new_bar_has_a_corner_radius_property() {
    // The half that was broken: no seed, so no inspector row, so the developer
    // could not set what the renderer was already willing to draw.
    for (name, kind, seed) in BARS {
        let c = Control::new("C-1", kind.clone(), 0, 0);
        assert_eq!(
            c.get_prop("CornerRadius").map(|v| v.as_i64()),
            Some(seed),
            "{name} must seed CornerRadius — at its own default, so no existing \
             bar changes shape and the row simply exists"
        );
    }
}

#[test]
fn setting_it_rounds_the_bar() {
    for (name, kind, _) in BARS {
        let square = painted_corner(&bar(kind.clone(), Some(0)));
        let round = painted_corner(&bar(kind.clone(), Some(14)));
        assert!(
            round > square,
            "{name}: CornerRadius = 14 must paint rounder corners than 0 \
             (got {round} vs {square})"
        );
    }
}

#[test]
fn a_bar_that_sets_nothing_is_unchanged() {
    // The point of the seed is the row, not a new look: an untouched bar paints
    // exactly as one explicitly set to its own seeded value.
    for (name, kind, seed) in BARS {
        assert_eq!(
            painted_corner(&bar(kind.clone(), None)),
            painted_corner(&bar(kind.clone(), Some(seed))),
            "{name}: seeding the property must not round anything by itself"
        );
    }
}
