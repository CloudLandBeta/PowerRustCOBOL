// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Spec 055 T6 — `draw_snackbar` paints in R20's order, and the background image
//! never moves content (R21).
//!
//! The shapes are collected from a real headless egui frame and inspected in
//! emission order. The standing trap this avoids: a shape COUNT proves nothing —
//! two paints can emit the same number of rectangles and still put them in the
//! wrong order or the wrong colour, so the assertions here compare **colours and
//! positions**, never a tally.

use cobolt_forms::model::{Control, ControlType, PropValue};
use cobolt_forms::paint::draw_snackbar;
use cobolt_forms::snackbar::mint;
use egui::{Color32, Pos2, Rect, Vec2};

/// Every filled rect in emission order, as (rect, fill).
fn painted(v: &cobolt_forms::snackbar::SnackVisual, at: Rect) -> Vec<(Rect, Color32)> {
    let ctx = egui::Context::default();
    let mut input = egui::RawInput::default();
    input.screen_rect = Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(900.0, 400.0)));
    let mut full = ctx.run_ui(input, |root_ui| {
        egui::CentralPanel::default()
            .frame(egui::Frame::NONE)
            .show_inside(root_ui, |ui| {
                draw_snackbar(ui.painter(), at, v, None, 1.0);
            });
    });
    full.textures_delta.clear();
    fn collect(s: &egui::Shape, out: &mut Vec<(Rect, Color32)>) {
        match s {
            egui::Shape::Vec(v) => v.iter().for_each(|s| collect(s, out)),
            egui::Shape::Rect(r) => out.push((r.rect, r.fill)),
            _ => {}
        }
    }
    let mut out = Vec::new();
    for cs in &full.shapes {
        collect(&cs.shape, &mut out);
    }
    out
}

/// Every shape in emission order as (kind, bounding rect) — including the text
/// and icon geometry that `painted` deliberately skips.
fn painted_kinds(v: &cobolt_forms::snackbar::SnackVisual, at: Rect) -> Vec<(&'static str, Rect)> {
    let ctx = egui::Context::default();
    let mut input = egui::RawInput::default();
    input.screen_rect = Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(900.0, 400.0)));
    let mut full = ctx.run_ui(input, |root_ui| {
        egui::CentralPanel::default()
            .frame(egui::Frame::NONE)
            .show_inside(root_ui, |ui| {
                draw_snackbar(ui.painter(), at, v, None, 1.0);
            });
    });
    full.textures_delta.clear();
    fn kind(s: &egui::Shape) -> &'static str {
        match s {
            egui::Shape::Rect(_) => "rect",
            egui::Shape::Text(_) => "text",
            egui::Shape::Circle(_) => "circle",
            egui::Shape::Path(_) => "path",
            egui::Shape::LineSegment { .. } => "line",
            egui::Shape::Mesh(_) => "mesh",
            _ => "other",
        }
    }
    fn collect(s: &egui::Shape, out: &mut Vec<(&'static str, Rect)>) {
        match s {
            egui::Shape::Vec(v) => v.iter().for_each(|s| collect(s, out)),
            other => out.push((kind(other), other.visual_bounding_rect())),
        }
    }
    let mut out = Vec::new();
    for cs in &full.shapes {
        collect(&cs.shape, &mut out);
    }
    out
}

fn snack(set: &[(&str, PropValue)]) -> cobolt_forms::snackbar::SnackVisual {
    let mut c = Control::new("SNACK-1", ControlType::Snackbar, 0, 0);
    c.set_prop("Text", PropValue::String("Record saved".into()));
    for (k, v) in set {
        c.set_prop(*k, v.clone());
    }
    mint(&c).0
}

const AT: Rect = Rect {
    min: Pos2 { x: 100.0, y: 100.0 },
    max: Pos2 { x: 500.0, y: 156.0 },
};

#[test]
fn the_background_colour_is_laid_down_before_the_content() {
    // R20's first two steps, on a category whose colour is unmistakable.
    let v = snack(&[("BackgroundColor", PropValue::String("#1E4E8C".into()))]);
    let shapes = painted(&v, AT);

    // Find the notification's own face: the first shape covering the whole rect.
    // Match the face by its exact rect, not merely its size: the shadow's
    // innermost step is the same size and would otherwise be found first
    // (it is translated by ShadowDistance, which is what distinguishes them).
    let face_idx = shapes
        .iter()
        .position(|(r, _)| r.min.distance(AT.min) < 0.5 && r.max.distance(AT.max) < 0.5)
        .expect("the notification face must be painted");
    let (_, face) = shapes[face_idx];
    assert_eq!(
        (face.r(), face.g(), face.b()),
        (0x1E, 0x4E, 0x8C),
        "R20: the face carries the resolved background colour, got {face:?}"
    );

    // Everything drawn INSIDE the face after it is content. Content is text and
    // icon geometry, NOT rectangles — collecting only rects would have made this
    // check vacuous, which is exactly what it did on the first attempt.
    let kinds = painted_kinds(&v, AT);
    let face_pos = kinds
        .iter()
        .position(|(k, r)| *k == "rect" && r.min.distance(AT.min) < 0.5 && r.max.distance(AT.max) < 0.5)
        .expect("the face is among all shapes too");
    let after: Vec<&str> = kinds[face_pos + 1..]
        .iter()
        .filter(|(_, r)| AT.contains(r.center()))
        .map(|(k, _)| *k)
        .collect();
    assert!(
        !after.is_empty(),
        "R20: content must be painted AFTER the face. Shapes were: {:?}",
        kinds.iter().map(|(k, _)| *k).collect::<Vec<_>>()
    );
    assert!(
        after.iter().any(|k| *k == "text"),
        "the message text must be painted after the face, got {after:?}"
    );

    eprintln!(
        "\n  R20 — {} filled rects, {} shapes total; face at index {face_pos} \
         fill #{:02X}{:02X}{:02X}; content after it: {after:?}\n",
        shapes.len(), kinds.len(), face.r(), face.g(), face.b()
    );
}

#[test]
fn the_category_supplies_the_colour_when_unset_and_yields_when_set() {
    // AC10 — the same assertion the pure test makes, but proved at the PAINTER:
    // a default is only a default if it reaches the pixels.
    let mut cases: Vec<(&str, cobolt_forms::snackbar::SnackVisual, (u8, u8, u8))> = Vec::new();
    cases.push(("Info (unset)", snack(&[]), (0x1E, 0x4E, 0x8C)));
    cases.push((
        "Error (unset)",
        snack(&[("Category", PropValue::String("Error".into()))]),
        (0x8C, 0x23, 0x23),
    ));
    cases.push((
        "Error + explicit",
        snack(&[
            ("Category", PropValue::String("Error".into())),
            ("BackgroundColor", PropValue::String("#00FF7F".into())),
        ]),
        (0x00, 0xFF, 0x7F),
    ));

    eprintln!("\n  case                painted face   expected");
    eprintln!("  -----------------   ------------   --------");
    for (name, v, want) in &cases {
        let shapes = painted(v, AT);
        let (_, face) = shapes
            .iter()
            .find(|(r, _)| r.min.distance(AT.min) < 0.5 && r.max.distance(AT.max) < 0.5)
            .copied()
            .expect("a face");
        eprintln!(
            "  {name:<17}   #{:02X}{:02X}{:02X}        #{:02X}{:02X}{:02X}",
            face.r(), face.g(), face.b(), want.0, want.1, want.2
        );
        assert_eq!((face.r(), face.g(), face.b()), *want, "{name}");
    }
    eprintln!("  → AC10/R23: the category paints when unset; an explicit colour wins\n");
}

#[test]
fn a_background_image_never_moves_the_content() {
    // R21 — the layout is computed from the rect alone. Proving it at the
    // painter: the content rects are IDENTICAL with and without an image set.
    // (No texture is supplied, so the image branch is skipped; what is being
    // pinned is that declaring one changes no geometry.)
    let plain = snack(&[]);
    let imaged = snack(&[
        ("BackgroundImage", PropValue::String("/tmp/whatever.png".into())),
        ("BackgroundImageMode", PropValue::String("Tile".into())),
        ("BackgroundImageOpacity", PropValue::Int(80)),
    ]);

    let a = painted(&plain, AT);
    let b = painted(&imaged, AT);
    let geo = |v: &Vec<(Rect, Color32)>| v.iter().map(|(r, _)| *r).collect::<Vec<_>>();
    assert_eq!(
        geo(&a),
        geo(&b),
        "R21: declaring a background image must not move a single rect"
    );

    // And the button rects the painter reports are the same too — those are
    // what the host hit-tests, so a shift here would misroute onButtonClick.
    let ctx = egui::Context::default();
    let mut input = egui::RawInput::default();
    input.screen_rect = Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(900.0, 400.0)));
    let mut got = Vec::new();
    let mut full = ctx.run_ui(input, |root_ui| {
        egui::CentralPanel::default().frame(egui::Frame::NONE).show_inside(root_ui, |ui| {
            got.push(draw_snackbar(ui.painter(), AT, &plain, None, 1.0));
            got.push(draw_snackbar(ui.painter(), AT, &imaged, None, 1.0));
        });
    });
    full.textures_delta.clear();
    assert_eq!(got[0].buttons, got[1].buttons, "R21: button rects unmoved");
    assert_eq!(got[0].rect, got[1].rect);

    eprintln!(
        "\n  R21 — {} rects identical with and without a background image declared\n",
        geo(&a).len()
    );
}

#[test]
fn the_painter_reports_a_rect_for_every_button_it_drew() {
    // The host dispatches `onButtonClick` from these; one missing rect is one
    // button that can never be clicked.
    let v = snack(&[(
        "Buttons",
        PropValue::String("retry|Retry|refresh|Left|true\nclose||x-mark|Left|true".into()),
    )]);
    assert_eq!(v.buttons.len(), 2);

    let ctx = egui::Context::default();
    let mut input = egui::RawInput::default();
    input.screen_rect = Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(900.0, 400.0)));
    let mut out = None;
    let mut full = ctx.run_ui(input, |root_ui| {
        egui::CentralPanel::default().frame(egui::Frame::NONE).show_inside(root_ui, |ui| {
            out = Some(draw_snackbar(ui.painter(), AT, &v, None, 1.0));
        });
    });
    full.textures_delta.clear();
    let p = out.expect("painted");
    assert_eq!(p.buttons.len(), 2, "one rect per button");
    for (i, b) in p.buttons.iter().enumerate() {
        assert!(AT.contains(b.center()), "button {i} must sit inside the notification");
        assert!(b.width() > 0.0 && b.height() > 0.0, "button {i} must be clickable");
    }
    assert!(p.buttons[0].max.x <= p.buttons[1].min.x + 0.5, "buttons run left to right");
    eprintln!(
        "\n  button rects — {:?} and {:?}, both inside {:?}\n",
        p.buttons[0], p.buttons[1], AT
    );
}

#[test]
fn a_snackbar_never_paints_outside_its_own_rect() {
    // R26's neighbour: a notification that painted past its rect would look
    // like the surface had grown. The shadow is deliberately exempt — it is
    // drawn outside by definition — so this checks the face and the content.
    let v = snack(&[
        ("ShadowEnabled", PropValue::Bool(false)),
        ("Buttons", PropValue::String("ok|OK|check|Left|true".into())),
    ]);
    let shapes = painted(&v, AT);
    let mut escapees = Vec::new();
    for (r, c) in &shapes {
        if c.a() == 0 {
            continue;
        }
        if !AT.expand(1.0).contains_rect(*r) {
            escapees.push(*r);
        }
    }
    assert!(escapees.is_empty(), "shape(s) painted outside the notification: {escapees:?}");
    eprintln!("\n  containment — {} shapes, all inside {:?}, 0 escapees\n", shapes.len(), AT);
}
