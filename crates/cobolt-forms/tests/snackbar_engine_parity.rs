// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Spec 055 T16 — one notification, drawn identically wherever it is drawn.
//!
//! `rcrun run-form` and a compiled binary both consume `cobolt-form-host`, which
//! paints through `cobolt_forms::paint::draw_snackbar` and nothing else. There is
//! deliberately no second path — no `egui::Area` per notification, no per-surface
//! copy — so parity is a property of the design rather than something a test has
//! to police after the fact.
//!
//! What CAN still drift is the input: two callers building the same notification
//! and disagreeing about it. That is what these assert — the same template mints
//! the same visual, and the same visual paints the same shapes, every time and on
//! any painter.
//!
//! The standing trap: a shape COUNT proves nothing (two paints can emit the same
//! number of rectangles and put them in different places in different colours),
//! so every comparison here is over positions AND fills.

use cobolt_forms::model::{Control, ControlType, PropValue};
use cobolt_forms::paint::draw_snackbar;
use cobolt_forms::snackbar::{mint, SnackVisual};
use egui::{Color32, Pos2, Rect, Vec2};

fn template() -> Control {
    let mut c = Control::new("SNACK-1", ControlType::Snackbar, 0, 0);
    c.set_prop("Text", PropValue::String("Record saved".into()));
    c.set_prop("Category", PropValue::String("Warning".into()));
    c.set_prop("StackAnchor", PropValue::String("BottomRight".into()));
    c.set_prop("Buttons", PropValue::String("retry|Retry|refresh|Left|true".into()));
    c
}

/// Paint on a fresh context and return (rect, fill) for every shape, in order.
fn paint(v: &SnackVisual, at: Rect) -> Vec<(Rect, Color32)> {
    let ctx = egui::Context::default();
    let mut input = egui::RawInput::default();
    input.screen_rect = Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(900.0, 500.0)));
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
            other => out.push((other.visual_bounding_rect(), fill_of(other))),
        }
    }
    fn fill_of(s: &egui::Shape) -> Color32 {
        match s {
            egui::Shape::Rect(r) => r.fill,
            egui::Shape::Circle(c) => c.fill,
            egui::Shape::Path(p) => p.fill,
            egui::Shape::Text(t) => t.fallback_color,
            _ => Color32::TRANSPARENT,
        }
    }
    let mut out = Vec::new();
    for cs in &full.shapes {
        collect(&cs.shape, &mut out);
    }
    out
}

const AT: Rect = Rect {
    min: Pos2 { x: 120.0, y: 200.0 },
    max: Pos2 { x: 520.0, y: 256.0 },
};

#[test]
fn one_template_mints_one_visual_however_often_it_is_asked() {
    // `Show()` twice must produce two IDENTICAL notifications from an unchanged
    // template. If minting were order- or state-dependent, the second message on
    // a stack would quietly differ from the first.
    let c = template();
    let (a, da) = mint(&c);
    let (b, db) = mint(&c);
    assert_eq!(a, b, "minting is a pure function of the template");
    assert_eq!(da, db);
    eprintln!(
        "\n  mint determinism — category {:?}, timeout {} ms, {} button(s), bg {}\n",
        a.category, a.timeout_ms, a.buttons.len(), a.background
    );
}

#[test]
fn the_same_visual_paints_the_same_shapes_on_any_painter() {
    // The parity assertion proper: two independent contexts, same input, same
    // output — positions AND fills, not a count.
    let v = mint(&template()).0;
    let first = paint(&v, AT);
    let second = paint(&v, AT);

    assert_eq!(first.len(), second.len(), "shape count differed");
    let mut mismatched = Vec::new();
    for (i, (a, b)) in first.iter().zip(&second).enumerate() {
        if a != b {
            mismatched.push(format!("#{i}: {a:?} vs {b:?}"));
        }
    }
    assert!(
        mismatched.is_empty(),
        "{} shape(s) differed between painters:\n  {}",
        mismatched.len(),
        mismatched.join("\n  ")
    );
    assert!(first.len() > 3, "a notification with an icon, text and a button draws more than 3 shapes");
    eprintln!(
        "\n  AC12 — {} shapes compared by rect AND fill across two contexts, 0 differences\n",
        first.len()
    );
}

#[test]
fn moving_a_notification_translates_it_and_changes_nothing_else() {
    // The stack reflows constantly, so the same notification is painted at a
    // different y every time one above it leaves. Its APPEARANCE must not depend
    // on where it landed — a size- or position-dependent fill is exactly the
    // kind of drift that shows up as "it looks different in the shell".
    let v = mint(&template()).0;
    let here = paint(&v, AT);
    let offset = Vec2::new(0.0, -64.0);
    let there = paint(&v, AT.translate(offset));

    assert_eq!(here.len(), there.len(), "shape count changed with position");
    let mut wrong = Vec::new();
    for (i, ((ra, ca), (rb, cb))) in here.iter().zip(&there).enumerate() {
        if ca != cb {
            wrong.push(format!("#{i} fill {ca:?} → {cb:?}"));
        } else {
            let moved = *rb;
            let expect = ra.translate(offset);
            if (moved.min - expect.min).length() > 0.6 || (moved.max - expect.max).length() > 0.6 {
                wrong.push(format!("#{i} rect {ra:?} → {rb:?}, expected {expect:?}"));
            }
        }
    }
    assert!(wrong.is_empty(), "{} shape(s) did not translate cleanly:\n  {}", wrong.len(), wrong.join("\n  "));
    eprintln!(
        "\n  reflow parity — {} shapes translated by {:?} with identical fills\n",
        here.len(), offset
    );
}

#[test]
fn every_category_paints_its_own_face_and_no_two_are_alike() {
    // A category that painted the same as another would make the whole scheme
    // decorative. Reported as a table so the colours can be read, not just
    // asserted to differ.
    use cobolt_forms::snackbar::SnackCategory;
    let mut faces = Vec::new();
    eprintln!("\n  category    painted face   timeout");
    eprintln!("  ---------   ------------   -------");
    for cat in SnackCategory::ALL {
        let mut c = template();
        c.set_prop("Category", PropValue::String(cat.as_str().into()));
        let v = mint(&c).0;
        let shapes = paint(&v, AT);
        let (_, face) = shapes
            .iter()
            .find(|(r, _)| r.min.distance(AT.min) < 0.5 && r.max.distance(AT.max) < 0.5)
            .copied()
            .expect("a face");
        eprintln!(
            "  {:<9}   #{:02X}{:02X}{:02X}        {}",
            cat.as_str(), face.r(), face.g(), face.b(), v.timeout_ms
        );
        faces.push((cat.as_str(), (face.r(), face.g(), face.b())));
    }
    for i in 0..faces.len() {
        for j in (i + 1)..faces.len() {
            assert_ne!(
                faces[i].1, faces[j].1,
                "{} and {} paint the same face",
                faces[i].0, faces[j].0
            );
        }
    }
    eprintln!("  → 5 categories, 5 distinct faces\n");
}
