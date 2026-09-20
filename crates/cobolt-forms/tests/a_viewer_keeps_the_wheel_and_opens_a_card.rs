#![cfg(feature = "render")]
//! Two reports about a Viewer's pointer, one afternoon (operator, 2026-09-20):
//!
//!   * "Scroll up/down the content is passing through the form itself. It
//!     should not." — the Viewer READ the wheel and never consumed it, so the
//!     same notch scrolled the document and then the form behind it;
//!   * "double clicking a card must show the selected page" — a contact sheet
//!     where two clicks did what one click did.
//!
//! Both are asserted through the real interactive renderer, driven with real
//! events, because both are about what the engine does with an event rather
//! than about what a function returns when called directly.

use cobolt_forms::containers::ActiveTabs;
use cobolt_forms::model::Rect as MRect;
use cobolt_forms::render::{DesignedState, RenderInput, RenderMode, RenderOutput};
use cobolt_forms::{Control, ControlType, PropValue};
use egui::{pos2, Rect, Vec2};

const FORM: Vec2 = Vec2::new(900.0, 700.0);

/// A side-by-side Viewer, optionally with a different document on each side.
fn split_viewer(left: &str, right: &str) -> Control {
    let mut c = Control::new("VWR-1", ControlType::Viewer, 40, 40);
    c.rect = MRect::new(40, 40, 700, 560);
    c.set_prop("Layout", PropValue::String("Web".into()));
    c.set_prop("SplitMode", PropValue::String("LeftRight".into()));
    c.set_prop("View1Source", PropValue::String(left.into()));
    c.set_prop("View2Source", PropValue::String(right.into()));
    c
}

/// Where the Split button sits in a view's own toolbar, in that view's rect.
fn split_button(view: egui::Rect) -> egui::Pos2 {
    let slots = cobolt_forms::viewer::toolbar_slots(cobolt_forms::viewer::ViewRect::new(
        view.min.x,
        view.min.y,
        view.width(),
        cobolt_forms::viewer::TOOLBAR_HEIGHT,
    ));
    let (_, slot) = slots
        .into_iter()
        .find(|(a, _)| *a == cobolt_forms::viewer::ToolbarAction::Split)
        .expect("every view's toolbar carries the Split action");
    pos2(slot.x + slot.w / 2.0, slot.y + slot.h / 2.0)
}

fn click_at(p: egui::Pos2) -> Vec<egui::Event> {
    vec![
        egui::Event::PointerMoved(p),
        egui::Event::PointerButton {
            pos: p,
            button: egui::PointerButton::Primary,
            pressed: true,
            modifiers: Default::default(),
        },
        egui::Event::PointerButton {
            pos: p,
            button: egui::PointerButton::Primary,
            pressed: false,
            modifiers: Default::default(),
        },
    ]
}

fn viewer(cards: bool) -> Control {
    let mut c = Control::new("VWR-1", ControlType::Viewer, 40, 40);
    c.rect = MRect::new(40, 40, 700, 560);
    c.set_prop("Layout", PropValue::String("Web".into()));
    if cards {
        c.set_prop("View1ViewMode", PropValue::String("Cards".into()));
    }
    c
}

/// One frame, and what leaked past the controls.
///
/// `leaked` is read from the SAME place an ancestor `ScrollArea` reads it —
/// `smooth_scroll_delta`, after the form's content has run — so this measures
/// the actual mechanism by which a notch reaches the form, not a proxy for it.
fn frame(
    ctx: &egui::Context,
    controls: &[Control],
    events: Vec<egui::Event>,
) -> (RenderOutput, Vec2) {
    let active = ActiveTabs::new();
    let mut input = egui::RawInput::default();
    input.screen_rect = Some(Rect::from_min_size(pos2(0.0, 0.0), FORM));
    input.events = events;
    let mut produced = RenderOutput::default();
    let mut leaked = Vec2::ZERO;
    let mut full = ctx.run_ui(input, |root| {
        egui::CentralPanel::default()
            .frame(egui::Frame::NONE)
            .show(root, |ui| {
                let inp = RenderInput {
                    controls,
                    state: &DesignedState,
                    form_size: FORM,
                    glass: true,
                    mode: RenderMode::Interactive,
                    active_tabs: &active,
                    backdrop: Default::default(),
                };
                produced = cobolt_forms::render::render_form(ui, &inp);
                leaked = ui.input(|i| i.smooth_scroll_delta);
            });
    });
    full.textures_delta.clear();
    (produced, leaked)
}

fn wheel_at(p: egui::Pos2) -> Vec<egui::Event> {
    vec![
        egui::Event::PointerMoved(p),
        egui::Event::MouseWheel {
            unit: egui::MouseWheelUnit::Point,
            delta: egui::vec2(0.0, -40.0),
            modifiers: Default::default(),
            phase: egui::TouchPhase::Move,
        },
    ]
}

/// **The Viewer keeps the wheel.** Over the control it is consumed; away from
/// it, it is not — and the second half is what stops the first from being
/// vacuously true. A Viewer that ate every notch on the form would pass a test
/// that only checked the first.
#[test]
fn a_wheel_notch_over_a_viewer_never_reaches_the_form() {
    let ctx = egui::Context::default();
    let controls = [viewer(false)];

    // Warm the context: the first pass of a fresh Context lays out and
    // registers, and a pointer that has never been anywhere is nowhere.
    frame(&ctx, &controls, Vec::new());

    let over = pos2(400.0, 300.0); // inside the Viewer's 40,40 700x560 rect
    let away = pos2(820.0, 660.0); // bare form, past the control
    let (_, leaked_over) = frame(&ctx, &controls, wheel_at(over));
    let (_, leaked_away) = frame(&ctx, &controls, wheel_at(away));

    println!("  pointer            leaked to the form");
    println!("  ---------------    ------------------");
    println!("  over the Viewer    {leaked_over:?}");
    println!("  off the Viewer     {leaked_away:?}");

    assert_eq!(
        leaked_over,
        Vec2::ZERO,
        "a notch over the Viewer must not reach the form"
    );
    assert_ne!(
        leaked_away,
        Vec2::ZERO,
        "a notch away from the Viewer must still reach the form — otherwise \
         the assertion above proves nothing"
    );
}

/// **One click selects a card; two open it.** The double-click must leave
/// `Cards` for `Full` on the page it hit, or it does exactly what the single
/// click already did.
#[test]
fn double_clicking_a_card_opens_that_page() {
    let ctx = egui::Context::default();
    let controls = [viewer(true)];
    frame(&ctx, &controls, Vec::new());

    // The first card sits at the top-left of the content area, inside the
    // control and below its toolbar band.
    let card = pos2(120.0, 200.0);
    let press = |pressed: bool| egui::Event::PointerButton {
        pos: card,
        button: egui::PointerButton::Primary,
        pressed,
        modifiers: Default::default(),
    };

    // Two complete presses in one frame is what egui reads as a double click.
    let (out, _) = frame(
        &ctx,
        &controls,
        vec![
            egui::Event::PointerMoved(card),
            press(true),
            press(false),
            press(true),
            press(false),
        ],
    );

    let wrote = |k: &str| {
        out.prop_updates
            .iter()
            .find(|(id, key, _)| id == "VWR-1" && key == k)
            .map(|(_, _, v)| v.clone())
    };
    let mode = wrote("View1ViewMode");
    println!("  View1ViewMode after a double-click on a card: {mode:?}");
    assert_eq!(
        mode.as_deref(),
        Some("Full"),
        "a double-clicked card must open the page, not stay on the contact sheet"
    );
}

/// **Closing a side closes the document that was on it.**
///
/// Side-by-side holding two different files is a reader comparing two things.
/// Asking for a single view on the LEFT means "I am done with the left one", so
/// the survivor must be the RIGHT document — closing the side while keeping
/// exactly the file just dismissed is what was reported (operator,
/// 2026-09-20). Two views of ONE file have no file to lose, and there the view
/// closes and nothing else.
#[test]
fn closing_the_left_side_leaves_the_right_document_behind() {
    let wrote = |c: &Control, at: egui::Pos2| -> Vec<(String, String)> {
        let ctx = egui::Context::default();
        let controls = [c.clone()];
        frame(&ctx, &controls, Vec::new());
        let (out, _) = frame(&ctx, &controls, click_at(at));
        out.prop_updates
            .iter()
            .map(|(_, k, v)| (k.clone(), v.clone()))
            .collect()
    };
    // The left view occupies the left half of the control's 700x560 rect.
    let left_view = Rect::from_min_size(pos2(40.0, 40.0), Vec2::new(350.0, 560.0));

    let two = split_viewer("left.md", "right.md");
    let updates = wrote(&two, split_button(left_view));
    let get = |k: &str| {
        updates
            .iter()
            .find(|(key, _)| key == k)
            .map(|(_, v)| v.clone())
    };
    println!("  two files, closing the LEFT side:");
    for (k, v) in &updates {
        println!("    {k} = {v:?}");
    }
    assert_eq!(get("SplitMode").as_deref(), Some("None"), "the split must close");
    assert_eq!(
        get("View1Source").as_deref(),
        Some("right.md"),
        "the surviving view must show the document that was NOT dismissed"
    );
    assert_eq!(
        get("View2Source").as_deref(),
        Some(""),
        "the closed side keeps no document"
    );

    // The carve-out: one file on both sides is one document seen twice.
    let same = split_viewer("only.md", "only.md");
    let updates = wrote(&same, split_button(left_view));
    println!("  one file on both sides, closing the LEFT side:");
    for (k, v) in &updates {
        println!("    {k} = {v:?}");
    }
    let moved = updates.iter().any(|(k, _)| k == "View2Source");
    assert!(
        !moved,
        "two views of one file lose no document — only the view closes"
    );
}

/// Apply what a frame wrote back, the way a host does. Without this the
/// control never changes and a multi-step gesture cannot be told from a
/// single one.
fn apply(c: &mut Control, out: &RenderOutput) {
    for (_, key, val) in &out.prop_updates {
        c.set_prop(key, PropValue::String(val.clone()));
    }
}

fn prop(c: &Control, k: &str) -> String {
    c.get_prop(k).map(|v| v.as_str().to_owned()).unwrap_or_default()
}

/// **A view that closed the split must not keep voting once it is gone.**
///
/// Each view reconciles the control-wide `SplitMode` through its own shared
/// value, which prefers what the VIEW decided unless the property changed since
/// the view last looked. A second view that closed the split then stopped
/// rendering never looked again — so the moment the split reopened it woke,
/// found no change against its frozen `seen`, re-asserted its stale "closed"
/// and wrote `SplitMode = None` straight back. Side-by-side could not be
/// reached again for the life of the form (operator, 2026-09-20).
#[test]
fn the_split_can_be_reopened_after_a_view_closed_it() {
    let ctx = egui::Context::default();
    let mut c = split_viewer("a.md", "b.md");
    frame(&ctx, &[c.clone()], Vec::new());

    // Close from the RIGHT side, which leaves the left document in place.
    let right = Rect::from_min_size(pos2(390.0, 40.0), Vec2::new(350.0, 560.0));
    let (out, _) = frame(&ctx, &[c.clone()], click_at(split_button(right)));
    apply(&mut c, &out);
    println!("  after closing from the right: SplitMode = {:?}", prop(&c, "SplitMode"));
    assert_eq!(prop(&c, "SplitMode"), "None", "the split must close");

    let (out, _) = frame(&ctx, &[c.clone()], Vec::new());
    apply(&mut c, &out);

    // Reopen from the one view that is left.
    let whole = Rect::from_min_size(pos2(40.0, 40.0), Vec2::new(700.0, 560.0));
    let (out, _) = frame(&ctx, &[c.clone()], click_at(split_button(whole)));
    apply(&mut c, &out);
    println!("  after asking for side-by-side: SplitMode = {:?}", prop(&c, "SplitMode"));
    assert_eq!(
        prop(&c, "SplitMode"),
        "LeftRight",
        "asking for side-by-side must open it"
    );

    // And it STAYS open. This is the half the bug failed: the second view woke
    // on the very next frame and wrote the split shut again.
    for n in 1..=3 {
        let (out, _) = frame(&ctx, &[c.clone()], Vec::new());
        apply(&mut c, &out);
        println!("  {n} frame(s) later: SplitMode = {:?}", prop(&c, "SplitMode"));
    }
    assert_eq!(
        prop(&c, "SplitMode"),
        "LeftRight",
        "a reopened split must stay open — no view may write it shut on its own"
    );
}

/// **Fullscreen moves the Viewer somewhere else, and the pointer with it.**
///
/// The control opens a window of its own (`R13/AC5` — "Fullscreen is not
/// working as it is supposed to. It is maximizing the view inside the viewer
/// instead of the entire screen", operator 2026-09-20). The copy left in the
/// form is still PAINTED — a form must not show a hole where a control is —
/// but it must not be SENSED, because one surface owns the pointer at a time
/// and it is the one the operator is looking at.
///
/// Asserted by clicking the in-form toolbar: windowed, that press lands and
/// writes a property; fullscreen, the same press at the same point writes
/// nothing, because the control is not there any more.
#[test]
fn a_fullscreen_viewer_stops_sensing_the_copy_left_in_the_form() {
    let at = |fullscreen: bool| -> Vec<String> {
        let mut c = viewer(false);
        c.set_prop("Fullscreen", PropValue::Bool(fullscreen));
        let ctx = egui::Context::default();
        let controls = [c];
        frame(&ctx, &controls, Vec::new());
        // The Find button in the IN-FORM toolbar, which toggles a property.
        let view = Rect::from_min_size(pos2(40.0, 40.0), Vec2::new(700.0, 560.0));
        let slots = cobolt_forms::viewer::toolbar_slots(cobolt_forms::viewer::ViewRect::new(
            view.min.x,
            view.min.y,
            view.width(),
            cobolt_forms::viewer::TOOLBAR_HEIGHT,
        ));
        let (_, slot) = slots
            .into_iter()
            .find(|(a, _)| *a == cobolt_forms::viewer::ToolbarAction::Find)
            .expect("the toolbar carries Find");
        let p = pos2(slot.x + slot.w / 2.0, slot.y + slot.h / 2.0);
        let (out, _) = frame(&ctx, &controls, click_at(p));
        out.prop_updates.iter().map(|(_, k, _)| k.clone()).collect()
    };

    let windowed = at(false);
    let full = at(true);
    println!("  in-form toolbar click, windowed:   {windowed:?}");
    println!("  in-form toolbar click, fullscreen: {full:?}");
    assert!(
        windowed.iter().any(|k| k == "View1FindOpen"),
        "windowed, the in-form toolbar must work: {windowed:?}"
    );
    assert!(
        !full.iter().any(|k| k == "View1FindOpen"),
        "fullscreen, the copy left in the form must not answer the pointer: {full:?}"
    );
}
