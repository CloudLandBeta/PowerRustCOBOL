// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! The Snackbar's built-in close (055 follow-up, operator 2026-09-03).
//!
//! Every notification gets a close-X, on top of whatever buttons the developer
//! added. Before this, a Critical notification with no developer button
//! (`timeout_ms: 0`, §7 R6 — severe enough that it must not silently expire)
//! had no way to close individually: only `DismissAll()`, which takes every
//! live notification on the control at once.
//!
//! This file proves the PAINTED geometry, through the real `draw_snackbar`:
//! the close never lands on a developer button, and `SnackbarPaint::hit_test`
//! — the exact test the host runs against a click — answers each rect
//! correctly. The standing trap this file's neighbour
//! (`snackbar_button_feedback.rs`) names applies here too: a shape COUNT
//! proves nothing. Every assertion is against the rect the painter itself
//! reported.

use cobolt_forms::model::{Control, ControlType, PropValue};
use cobolt_forms::paint::{draw_snackbar, SnackHit, SnackPointer, SnackbarPaint};
use cobolt_forms::snackbar::{mint, SnackVisual};
use egui::{Pos2, Rect, Vec2};

const AT: Rect = Rect {
    min: Pos2 { x: 100.0, y: 100.0 },
    max: Pos2 { x: 500.0, y: 156.0 },
};

fn critical_no_buttons() -> SnackVisual {
    let mut c = Control::new("SNACK-1", ControlType::Snackbar, 0, 0);
    c.set_prop("Category", PropValue::String("Critical".into()));
    c.set_prop("Text", PropValue::String("Disk almost full".into()));
    mint(&c).0
}

fn two_button_snack() -> SnackVisual {
    let mut c = Control::new("SNACK-1", ControlType::Snackbar, 0, 0);
    c.set_prop("Text", PropValue::String("Record saved".into()));
    c.set_prop(
        "Buttons",
        PropValue::String("retry|Retry|refresh|Left|true\nclose|Close||Left|true".into()),
    );
    mint(&c).0
}

/// Paint once, against a real (offscreen) painter — the same call
/// `snackbar_button_feedback.rs` makes.
fn paint(v: &SnackVisual) -> SnackbarPaint {
    let ctx = egui::Context::default();
    let mut input = egui::RawInput::default();
    input.screen_rect = Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(900.0, 400.0)));

    let mut out = None;
    let mut full = ctx.run_ui(input, |root_ui| {
        egui::CentralPanel::default()
            .frame(egui::Frame::NONE)
            .show_inside(root_ui, |ui| {
                out = Some(draw_snackbar(ui.painter(), AT, v, None, 1.0, SnackPointer::inert()));
            });
    });
    full.textures_delta.clear();
    out.expect("draw_snackbar always returns a SnackbarPaint")
}

#[test]
fn a_critical_buttonless_notification_still_paints_a_close_the_host_can_hit_test() {
    // This is the exact case that had NO way to close individually before:
    // Critical's own category default is timeout_ms 0 (never auto-dismisses),
    // and with no developer button there was nothing to click either.
    let v = critical_no_buttons();
    assert!(v.buttons.is_empty(), "fixture: no developer buttons");
    assert_eq!(v.timeout_ms, 0, "fixture: Critical never expires on its own (R6)");

    let out = paint(&v);
    assert!(AT.contains(out.close.center()), "the close sits inside the notification");
    assert_eq!(out.hit_test(out.close.center()), Some(SnackHit::Close));
    assert_eq!(
        out.hit_test(AT.left_top() + Vec2::new(2.0, 2.0)),
        None,
        "a point away from the close hits nothing on a buttonless notification"
    );
    eprintln!("\n  Critical, no buttons — close painted at {:?}, hit-tests as Close\n", out.close);
}

#[test]
fn the_close_never_overlaps_or_is_shadowed_by_a_developer_button() {
    let v = two_button_snack();
    let out = paint(&v);
    assert_eq!(out.buttons.len(), 2, "fixture: two developer buttons");

    for (i, br) in out.buttons.iter().enumerate() {
        assert!(!out.close.intersects(*br), "close {:?} overlaps button {i} {:?}", out.close, br);
        assert_eq!(
            out.hit_test(br.center()),
            Some(SnackHit::Button(i)),
            "button {i}'s own centre must hit the button, not the close"
        );
    }
    assert_eq!(
        out.hit_test(out.close.center()),
        Some(SnackHit::Close),
        "the close's own centre must hit the close, not a button"
    );
    eprintln!(
        "\n  2 developer buttons — close {:?}, buttons {:?} — no overlap, each hit-tests to itself\n",
        out.close, out.buttons
    );
}

#[test]
fn the_close_sits_in_the_notifications_top_right_region() {
    // "top-right corner of the notification body" — proved by asking which of
    // the notification's own four corners the close is nearest.
    let v = two_button_snack();
    let out = paint(&v);
    let corners = [
        ("top-left", AT.left_top()),
        ("top-right", AT.right_top()),
        ("bottom-left", AT.left_bottom()),
        ("bottom-right", AT.right_bottom()),
    ];
    let c = out.close.center();
    let (nearest_name, _) = corners
        .iter()
        .min_by(|(_, a), (_, b)| (*a - c).length().total_cmp(&(*b - c).length()))
        .unwrap();
    eprintln!("\n  close centre {c:?} — nearest corner: {nearest_name}\n");
    assert_eq!(*nearest_name, "top-right", "the close must read as a top-right affordance");
}
