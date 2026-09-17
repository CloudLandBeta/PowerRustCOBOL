#![cfg(feature = "render")]
//! A radio group holds exactly one selection, and the keyboard can choose it.
//!
//! Two reports, one control (operator, 2026-09-17):
//!
//!   * a group showed TWO buttons lit at once — the renderer cleared a group's
//!     other buttons when one was CLICKED, but a `SET Rad-PIX::Selected TO TRUE`
//!     never passes through the renderer, so the button already lit stayed lit;
//!   * and the group could not be driven from the keyboard at all: no arrows to
//!     walk it, no space bar to pick.
//!
//! The exclusivity half is asserted here for the click path and in the form host
//! for the code path — the two writers are in different crates, so neither test
//! can cover both.

use cobolt_forms::containers::ActiveTabs;
use cobolt_forms::model::{radio_group_key, Rect as MRect};
use cobolt_forms::render::{control_widget_id, DesignedState, RenderInput, RenderMode, RenderOutput};
use cobolt_forms::{Control, ControlType, PropValue};
use egui::{pos2, Key, Rect, Vec2};

/// A radio at a known place, in an optional named group.
fn radio(id: &str, y: i32, group: &str, selected: bool) -> Control {
    let mut c = Control::new(id, ControlType::RadioButton, 20, y);
    c.rect = MRect::new(20, y, 200, 24);
    c.set_prop("Selected", PropValue::Bool(selected));
    c.set_prop("GroupName", PropValue::String(group.into()));
    c.set_prop("Caption", PropValue::String(id.into()));
    c
}

fn centre(c: &Control) -> egui::Pos2 {
    pos2(
        c.rect.x as f32 + c.rect.w as f32 / 2.0,
        c.rect.y as f32 + c.rect.h as f32 / 2.0,
    )
}

/// Run one frame and hand back everything the engine produced.
fn frame(ctx: &egui::Context, controls: &[Control], events: Vec<egui::Event>) -> RenderOutput {
    let size = Vec2::new(600.0, 400.0);
    let active = ActiveTabs::new();
    let mut input = egui::RawInput::default();
    input.screen_rect = Some(Rect::from_min_size(pos2(0.0, 0.0), size));
    input.events = events;
    let mut produced = RenderOutput::default();
    let mut full = ctx.run_ui(input, |root| {
        egui::CentralPanel::default()
            .frame(egui::Frame::NONE)
            .show(root, |ui| {
                let inp = RenderInput {
                    controls,
                    state: &DesignedState,
                    form_size: size,
                    glass: true,
                    mode: RenderMode::Interactive,
                    active_tabs: &active,
                    backdrop: Default::default(),
                };
                produced = cobolt_forms::render::render_form(ui, &inp);
            });
    });
    full.textures_delta.clear();
    produced
}

fn key(k: Key) -> Vec<egui::Event> {
    vec![egui::Event::Key {
        key: k,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: Default::default(),
    }]
}

/// What `id` was written to under `key`, if this frame wrote it.
fn wrote(out: &RenderOutput, id: &str, k: &str) -> Option<String> {
    out.prop_updates
        .iter()
        .find(|(i, key, _)| i == id && key == k)
        .map(|(_, _, v)| v.clone())
}

fn on(v: &str) -> bool {
    matches!(v, "1" | "true")
}

/// Put the focus on `id` and let one frame pass.
///
/// The settling frame is not test scaffolding — it is the real sequence. A
/// focused radio claims the arrow keys through its egui event filter, and egui
/// reads that filter at the START of a pass, from what the widget registered
/// during the previous one. A key pressed on the very frame the focus lands is
/// therefore still egui's to spend; by the next frame the radio owns it. In use
/// the focus always arrives at least a frame before the key does.
fn focus(ctx: &egui::Context, controls: &[Control], id: &str) {
    ctx.memory_mut(|m| m.request_focus(control_widget_id(None, id)));
    frame(ctx, controls, vec![]);
}

// ── Grouping ──────────────────────────────────────────────────────────────────

/// The rule both writers share. It moved into `model` so there is one copy:
/// the renderer enforces exclusivity on a click and the host enforces it on a
/// write from code, and two copies of "which radios are a group" would drift.
#[test]
fn a_named_group_spans_containers_and_an_unnamed_one_follows_its_parent() {
    let mut a = radio("A", 10, "pay", false);
    let mut b = radio("B", 40, "pay", false);
    a.parent = Some("Panel-1".into());
    b.parent = Some("Panel-2".into());
    assert_eq!(
        radio_group_key(&a),
        radio_group_key(&b),
        "a NAME makes a group whatever contains the buttons"
    );

    let mut c = radio("C", 10, "", false);
    let mut d = radio("D", 40, "", false);
    c.parent = Some("Panel-1".into());
    d.parent = Some("Panel-2".into());
    assert_ne!(
        radio_group_key(&c),
        radio_group_key(&d),
        "with no name, what CONTAINS them decides"
    );

    let mut e = radio("E", 70, "", false);
    e.parent = Some("Panel-1".into());
    assert_eq!(
        radio_group_key(&c),
        radio_group_key(&e),
        "two unnamed radios in one container are one group"
    );
}

// ── Exclusivity, click path ───────────────────────────────────────────────────

/// Clicking one turns the other off — under every spelling its readers use, so
/// `IsSelected()` and the next frame's paint agree.
#[test]
fn clicking_one_radio_turns_its_group_mate_off() {
    let ctx = egui::Context::default();
    let controls = vec![
        radio("Rad-PIX", 40, "payment", false),
        radio("Rad-Boleto", 80, "payment", true),
    ];
    let at = centre(&controls[0]);
    frame(&ctx, &controls, vec![]);
    frame(&ctx, &controls, vec![egui::Event::PointerMoved(at)]);
    frame(
        &ctx,
        &controls,
        vec![egui::Event::PointerButton {
            pos: at,
            button: egui::PointerButton::Primary,
            pressed: true,
            modifiers: Default::default(),
        }],
    );
    let out = frame(
        &ctx,
        &controls,
        vec![egui::Event::PointerButton {
            pos: at,
            button: egui::PointerButton::Primary,
            pressed: false,
            modifiers: Default::default(),
        }],
    );

    for k in ["Selected", "Checked", "Value"] {
        let lit = wrote(&out, "Rad-PIX", k).unwrap_or_else(|| panic!("no `{k}` on the clicked one"));
        assert!(on(&lit), "the clicked radio was not turned on: {k}={lit}");
        let out_k =
            wrote(&out, "Rad-Boleto", k).unwrap_or_else(|| panic!("the mate kept its `{k}`"));
        assert!(!on(&out_k), "two radios lit at once: Rad-Boleto {k}={out_k}");
    }
}

/// A radio in a DIFFERENT group is none of this one's business.
#[test]
fn clicking_a_radio_leaves_another_group_alone() {
    let ctx = egui::Context::default();
    let controls = vec![
        radio("Rad-PIX", 40, "payment", false),
        radio("Rad-Express", 80, "shipping", true),
    ];
    let at = centre(&controls[0]);
    frame(&ctx, &controls, vec![]);
    frame(&ctx, &controls, vec![egui::Event::PointerMoved(at)]);
    frame(
        &ctx,
        &controls,
        vec![egui::Event::PointerButton {
            pos: at,
            button: egui::PointerButton::Primary,
            pressed: true,
            modifiers: Default::default(),
        }],
    );
    let out = frame(
        &ctx,
        &controls,
        vec![egui::Event::PointerButton {
            pos: at,
            button: egui::PointerButton::Primary,
            pressed: false,
            modifiers: Default::default(),
        }],
    );
    assert!(
        wrote(&out, "Rad-Express", "Selected").is_none(),
        "the shipping group was cleared by a payment click"
    );
}

// ── Keyboard ──────────────────────────────────────────────────────────────────

/// With focus on a radio, the space bar picks it — the same writes a click makes,
/// because it is the same act.
#[test]
fn the_space_bar_selects_the_focused_radio() {
    let ctx = egui::Context::default();
    let controls = vec![
        radio("Rad-Card", 40, "payment", false),
        radio("Rad-PIX", 80, "payment", true),
    ];
    frame(&ctx, &controls, vec![]);
    focus(&ctx, &controls, "Rad-Card");

    let out = frame(&ctx, &controls, key(Key::Space));
    let lit = wrote(&out, "Rad-Card", "Selected").expect("space did not select the focused radio");
    assert!(on(&lit), "space wrote {lit:?}");
    // …and the one it replaced goes out, or the group shows two again.
    let gone = wrote(&out, "Rad-PIX", "Selected").expect("the previous selection was left lit");
    assert!(!on(&gone), "Rad-PIX stayed on: {gone:?}");
}

/// Down and Right walk forward, Up and Left walk back, and both ends wrap so a
/// held key never dead-ends.
#[test]
fn the_arrows_walk_the_group_and_wrap() {
    let ctx = egui::Context::default();
    let controls = vec![
        radio("R1", 40, "g", true),
        radio("R2", 80, "g", false),
        radio("R3", 120, "g", false),
    ];
    let id = |name: &str| control_widget_id(None, name);

    for (from, k, want) in [
        ("R1", Key::ArrowDown, "R2"),
        ("R2", Key::ArrowRight, "R3"),
        ("R3", Key::ArrowDown, "R1"), // wraps forward
        ("R3", Key::ArrowUp, "R2"),
        ("R2", Key::ArrowLeft, "R1"),
        ("R1", Key::ArrowUp, "R3"), // wraps back
    ] {
        frame(&ctx, &controls, vec![]);
        focus(&ctx, &controls, from);
        frame(&ctx, &controls, key(k));
        let focused = ctx.memory(|m| m.focused());
        assert_eq!(
            focused,
            Some(id(want)),
            "{from} + {k:?} should focus {want}"
        );
    }
}

/// Arrowing through a group to look at it must not change the answer: the
/// focus moves, the selection does not. Space is what selects.
#[test]
fn arrowing_moves_the_focus_without_changing_the_selection() {
    let ctx = egui::Context::default();
    let controls = vec![
        radio("R1", 40, "g", true),
        radio("R2", 80, "g", false),
    ];
    frame(&ctx, &controls, vec![]);
    focus(&ctx, &controls, "R1");

    let out = frame(&ctx, &controls, key(Key::ArrowDown));
    assert_eq!(
        ctx.memory(|m| m.focused()),
        Some(control_widget_id(None, "R2")),
        "the focus did not move"
    );
    assert!(
        out.prop_updates.is_empty(),
        "arrowing changed the selection: {:?}",
        out.prop_updates
    );
}

/// The arrows stay inside the group — they are not a second Tab order.
#[test]
fn the_arrows_do_not_leave_the_group() {
    let ctx = egui::Context::default();
    let controls = vec![
        radio("Pay-1", 40, "payment", true),
        radio("Ship-1", 80, "shipping", false),
    ];
    frame(&ctx, &controls, vec![]);
    let start = control_widget_id(None, "Pay-1");
    focus(&ctx, &controls, "Pay-1");

    frame(&ctx, &controls, key(Key::ArrowDown));
    assert_eq!(
        ctx.memory(|m| m.focused()),
        Some(start),
        "the focus escaped into another group; a lone radio has nowhere to go"
    );
}
