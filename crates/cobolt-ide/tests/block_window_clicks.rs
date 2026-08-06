// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors

//! Reproduction harness for the `cobolt_windows` "always the last button" bug.
//!
//! The operator's report: a block opens a two-button dialog through
//! `cobolt_windows::open`; whichever button is clicked, the recorded value is
//! the SECOND button's. The only writer of that value is the second button's
//! `.clicked()`, so either both buttons fire from one click, or the click is
//! re-delivered across passes. This test drives the exact drawing pattern the
//! generated `cobolt_windows::show_all` uses — `show_viewport_deferred`,
//! re-registered every pass with a fresh wrapper closure, embed mode (the raw
//! `Context` default) — and synthesizes a click on the FIRST button.

use std::sync::{Arc, Mutex};

fn pass(
    ctx: &egui::Context,
    events: Vec<egui::Event>,
    clicked: &Arc<Mutex<i64>>,
    rects: &Arc<Mutex<Vec<egui::Rect>>>,
) {
    let input = egui::RawInput {
        screen_rect: Some(egui::Rect::from_min_size(
            egui::Pos2::ZERO,
            egui::vec2(800.0, 600.0),
        )),
        events,
        ..Default::default()
    };
    let clicked = Arc::clone(clicked);
    let rects = Arc::clone(rects);
    ctx.run_ui(input, |root_ui| {
        // Exactly what the generated show_all does each frame: re-register the
        // deferred viewport with a freshly created wrapper closure.
        let clicked = Arc::clone(&clicked);
        let rects = Arc::clone(&rects);
        root_ui.ctx().show_viewport_deferred(
            egui::ViewportId::from_hash_of("ask"),
            egui::ViewportBuilder::default()
                .with_title("Pick")
                .with_inner_size([240.0, 90.0]),
            move |ui, _class| {
                let mut seen = Vec::new();
                ui.horizontal(|ui| {
                    for caption in [1_i64, 2_i64] {
                        let resp = ui.button(caption.to_string());
                        seen.push(resp.rect);
                        if resp.clicked() {
                            *clicked.lock().unwrap() = caption;
                        }
                    }
                });
                *rects.lock().unwrap() = seen;
            },
        );
    })
    .textures_delta
    .clear();
}

#[test]
fn clicking_the_first_button_yields_the_first_value() {
    let ctx = egui::Context::default(); // embed_viewports = true in a raw Context
    let clicked: Arc<Mutex<i64>> = Arc::new(Mutex::new(0));
    let rects: Arc<Mutex<Vec<egui::Rect>>> = Arc::new(Mutex::new(Vec::new()));

    // Layout pass — find where the buttons are.
    pass(&ctx, vec![], &clicked, &rects);
    pass(&ctx, vec![], &clicked, &rects);
    let (b1, b2) = {
        let r = rects.lock().unwrap();
        assert_eq!(r.len(), 2, "two buttons must be drawn: {r:?}");
        (r[0], r[1])
    };
    assert!(
        b1.intersect(b2).area() <= 0.0,
        "buttons must not overlap: {b1:?} vs {b2:?}"
    );

    // Press + release on the FIRST button's centre.
    let at = b1.center();
    pass(
        &ctx,
        vec![egui::Event::PointerMoved(at)],
        &clicked,
        &rects,
    );
    pass(
        &ctx,
        vec![egui::Event::PointerButton {
            pos: at,
            button: egui::PointerButton::Primary,
            pressed: true,
            modifiers: egui::Modifiers::NONE,
        }],
        &clicked,
        &rects,
    );
    pass(
        &ctx,
        vec![egui::Event::PointerButton {
            pos: at,
            button: egui::PointerButton::Primary,
            pressed: false,
            modifiers: egui::Modifiers::NONE,
        }],
        &clicked,
        &rects,
    );
    // A few quiet passes — if a stale click re-fires on a later pass (the
    // "always the last button" mechanism), these surface it.
    pass(&ctx, vec![], &clicked, &rects);
    pass(&ctx, vec![], &clicked, &rects);

    assert_eq!(
        *clicked.lock().unwrap(),
        1,
        "clicking button 1 must record 1"
    );
}
