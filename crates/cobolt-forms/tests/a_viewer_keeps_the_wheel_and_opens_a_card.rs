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
