// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors

//! What `send_viewport_cmd(Close)` from inside a block-owned window actually
//! closes — and whether the same window id can be opened again afterwards.
//!
//! A block's dialog closes itself the way egui's own docs show:
//! `ui.ctx().send_viewport_cmd(ViewportCommand::Close)`. If that command lands
//! on the ROOT viewport instead of the child, closing the dialog quits the
//! whole application — which would race any COBOL statement that runs after
//! `win.wait()` returns, so a label set there would sometimes appear and
//! sometimes not.

use std::sync::{
    atomic::{AtomicBool, AtomicUsize, Ordering},
    Arc,
};

/// Drive one pass with a deferred viewport whose callback closes itself on the
/// pass where `close_now` is set. Returns (root_close_requested, child_ran).
fn pass(
    ctx: &egui::Context,
    close_now: &Arc<AtomicBool>,
    child_runs: &Arc<AtomicUsize>,
) -> bool {
    let close_now = Arc::clone(close_now);
    let child_runs = Arc::clone(child_runs);
    let mut full = ctx.run_ui(
        egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(800.0, 600.0),
            )),
            ..Default::default()
        },
        |root_ui| {
            let close_now = Arc::clone(&close_now);
            let child_runs = Arc::clone(&child_runs);
            root_ui.ctx().show_viewport_deferred(
                egui::ViewportId::from_hash_of("ask"),
                egui::ViewportBuilder::default().with_title("Pick"),
                move |ui, _class| {
                    child_runs.fetch_add(1, Ordering::SeqCst);
                    if close_now.swap(false, Ordering::SeqCst) {
                        ui.ctx()
                            .send_viewport_cmd(egui::ViewportCommand::Close);
                    }
                },
            );
        },
    );
    full.textures_delta.clear();
    // Did the ROOT viewport get told to close?
    full.viewport_output
        .get(&egui::ViewportId::ROOT)
        .map(|o| {
            o.commands
                .iter()
                .any(|c| matches!(c, egui::ViewportCommand::Close))
        })
        .unwrap_or(false)
}

/// **The trap this pins:** `send_viewport_cmd(Close)` from inside a block's
/// window closes the APPLICATION, not the window.
///
/// `Context::send_viewport_cmd` targets `self.viewport_id()` — the viewport
/// *current during the pass*. Inside a deferred viewport's callback that is
/// the parent when viewports are embedded, so the command reaches the ROOT and
/// the whole program quits. The visible symptom is a dialog that closes and
/// takes the form with it, racing any COBOL that runs after `win.wait()`
/// returns: a label set there appears or not depending on timing.
///
/// The supported way for a block's window to close itself is
/// `cobolt_windows::close("id")`, which ends only that window (covered by
/// `closing_through_the_registry_leaves_the_root_alone`).
///
/// Asserted as current behaviour, not as desired behaviour: if a future egui
/// routes this to the child, this test flips and the guidance can relax.
#[test]
fn send_viewport_cmd_close_from_a_child_hits_the_root() {
    let ctx = egui::Context::default();
    let close_now = Arc::new(AtomicBool::new(false));
    let runs = Arc::new(AtomicUsize::new(0));

    pass(&ctx, &close_now, &runs);
    assert!(runs.load(Ordering::SeqCst) > 0, "the child must be drawn");

    close_now.store(true, Ordering::SeqCst);
    let root_closed = pass(&ctx, &close_now, &runs);

    assert!(
        root_closed,
        "expected the documented trap: Close from inside the child reaches the \
         ROOT viewport. If this now fails, egui has started routing it to the \
         child — update the guide and the System KB, which currently tell \
         developers to use cobolt_windows::close instead"
    );
}

/// The supported close — `cobolt_windows::close(id)`, modelled here as the
/// registry simply not re-registering the window — leaves the root untouched.
#[test]
fn closing_through_the_registry_leaves_the_root_alone() {
    let ctx = egui::Context::default();
    let runs = Arc::new(AtomicUsize::new(0));
    let never = Arc::new(AtomicBool::new(false));

    pass(&ctx, &never, &runs);
    let drawn = runs.load(Ordering::SeqCst);

    // The window is "closed" by not showing it this pass — exactly what
    // `show_all` does once an entry's open flag goes false.
    let mut full = ctx.run_ui(
        egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(800.0, 600.0),
            )),
            ..Default::default()
        },
        |_root_ui| {},
    );
    full.textures_delta.clear();

    let root_closed = full
        .viewport_output
        .get(&egui::ViewportId::ROOT)
        .map(|o| {
            o.commands
                .iter()
                .any(|c| matches!(c, egui::ViewportCommand::Close))
        })
        .unwrap_or(false);
    assert!(!root_closed, "the application must stay open");
    assert_eq!(
        runs.load(Ordering::SeqCst),
        drawn,
        "the window must stop being drawn once it is no longer registered"
    );
}

/// The same window id must open again after it was closed — a second click on
/// the form's button calls `cobolt_windows::open(\"ask\", ..)` all over again.
#[test]
fn the_same_window_id_reopens_after_closing() {
    let ctx = egui::Context::default();
    let close_now = Arc::new(AtomicBool::new(false));
    let runs = Arc::new(AtomicUsize::new(0));

    pass(&ctx, &close_now, &runs);
    close_now.store(true, Ordering::SeqCst);
    pass(&ctx, &close_now, &runs);
    let before = runs.load(Ordering::SeqCst);

    // Re-register the same id, as a second `ask()` would.
    pass(&ctx, &close_now, &runs);
    pass(&ctx, &close_now, &runs);
    assert!(
        runs.load(Ordering::SeqCst) > before,
        "re-registering the id must draw the window again; it did not, so a \
         second dialog in one run would never appear"
    );
}
