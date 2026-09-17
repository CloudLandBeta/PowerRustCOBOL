#![cfg(feature = "render")]
//! A toggle the operator clicked must read back as clicked — from COBOL.
//!
//! The CheckBox and RadioButton arms wrote the new state to `Value` alone. The
//! renderer reads `Value` first, so the control looked and behaved correctly on
//! screen — but COBOL never reads `Value`. `IsChecked()` and `IsSelected()`
//! both read `Checked` (`interpreter.rs`: `obj_get(obj, "Checked")`), which the
//! click never touched, so a box the operator had just ticked answered **0**
//! and a form branching on it took the wrong path (operator, 2026-09-16).
//!
//! The Switch is the control that was already right — it writes `Checked`
//! directly — so it is asserted here too, as the reference the other two now
//! match.
//!
//! These assert the **prop updates**, not the painting: the painting was never
//! wrong, and a test that only looked at pixels would have passed throughout.

use cobolt_forms::containers::ActiveTabs;
use cobolt_forms::model::Rect as MRect;
use cobolt_forms::render::{DesignedState, FormState, RenderInput, RenderMode};
use cobolt_forms::{Control, ControlType, PropValue};
use egui::{pos2, Rect, Vec2};

/// A form host's live state: designed control + whatever the running form has
/// since written onto it, exactly as `HostState` merges it.
struct LiveState(Vec<(String, String, String)>);

impl FormState for LiveState {
    fn live(&self, base: &Control) -> Control {
        let mut c = base.clone();
        for (id, key, val) in &self.0 {
            if *id == base.id {
                c.set_prop(key, PropValue::String(val.clone()));
            }
        }
        c
    }
}

/// One toggle at a known place, starting unticked.
fn toggle(id: &str, ty: ControlType, y: i32) -> Control {
    let mut c = Control::new(id, ty, 20, y);
    c.rect = MRect::new(20, y, 200, 24);
    // As a `.cfrm` stores it — the string spelling, not `Bool`.
    c.set_prop("Checked", PropValue::String("false".into()));
    c.set_prop("Caption", PropValue::String(id.into()));
    c
}

/// The centre of a control's rect, in screen points.
fn centre(c: &Control) -> egui::Pos2 {
    pos2(
        c.rect.x as f32 + c.rect.w as f32 / 2.0,
        c.rect.y as f32 + c.rect.h as f32 / 2.0,
    )
}

/// Run one frame and return every property update the engine produced.
fn frame_with(
    ctx: &egui::Context,
    controls: &[Control],
    state: &dyn FormState,
    events: Vec<egui::Event>,
) -> Vec<(String, String, String)> {
    let size = Vec2::new(600.0, 400.0);
    let active = ActiveTabs::new();
    let mut input = egui::RawInput::default();
    input.screen_rect = Some(Rect::from_min_size(pos2(0.0, 0.0), size));
    input.events = events;
    let mut updates = Vec::new();
    let mut full = ctx.run_ui(input, |root| {
        egui::CentralPanel::default()
            .frame(egui::Frame::NONE)
            .show(root, |ui| {
                let inp = RenderInput {
                    controls,
                    state,
                    form_size: size,
                    glass: true,
                    mode: RenderMode::Interactive,
                    active_tabs: &active,
                    backdrop: Default::default(),
                };
                updates = cobolt_forms::render::render_form(ui, &inp).prop_updates;
            });
    });
    full.textures_delta.clear();
    updates
}

/// Press and release the primary button at `p`; report that frame's updates.
fn click_at_with(
    ctx: &egui::Context,
    controls: &[Control],
    state: &dyn FormState,
    p: egui::Pos2,
) -> Vec<(String, String, String)> {
    frame_with(ctx, controls, state, vec![]);
    frame_with(ctx, controls, state, vec![egui::Event::PointerMoved(p)]);
    frame_with(
        ctx,
        controls,
        state,
        vec![egui::Event::PointerButton {
            pos: p,
            button: egui::PointerButton::Primary,
            pressed: true,
            modifiers: Default::default(),
        }],
    );
    frame_with(
        ctx,
        controls,
        state,
        vec![egui::Event::PointerButton {
            pos: p,
            button: egui::PointerButton::Primary,
            pressed: false,
            modifiers: Default::default(),
        }],
    )
}

fn click_at(
    ctx: &egui::Context,
    controls: &[Control],
    p: egui::Pos2,
) -> Vec<(String, String, String)> {
    click_at_with(ctx, controls, &DesignedState, p)
}

/// What `id` was set to under `key`, if the frame wrote it at all.
fn wrote(updates: &[(String, String, String)], id: &str, key: &str) -> Option<String> {
    updates
        .iter()
        .find(|(i, k, _)| i == id && k == key)
        .map(|(_, _, v)| v.clone())
}

/// `1` / `0` and `true` / `false` are both current spellings here; a state read
/// must accept either rather than pin the one in use today.
fn is_on(v: &str) -> bool {
    matches!(v, "1" | "true")
}

#[test]
fn ticking_a_checkbox_writes_the_property_cobol_reads() {
    let ctx = egui::Context::default();
    let controls = vec![toggle("Chk-Email", ControlType::CheckBox, 40)];
    let updates = click_at(&ctx, &controls, centre(&controls[0]));

    let checked = wrote(&updates, "Chk-Email", "Checked").expect(
        "the click wrote no `Checked` at all — IsChecked() would answer 0 on a box that is \
         visibly ticked, which is the whole defect",
    );
    assert!(is_on(&checked), "Checked was written as {checked:?}");

    // `Value` is still written: a handler reading `::Value` is as entitled to it.
    let value = wrote(&updates, "Chk-Email", "Value").expect("`Value` stopped being written");
    assert!(is_on(&value), "Value was written as {value:?}");
}

#[test]
fn selecting_a_radio_writes_both_spellings_its_readers_use() {
    let ctx = egui::Context::default();
    let controls = vec![toggle("Rad-Card", ControlType::RadioButton, 40)];
    let updates = click_at(&ctx, &controls, centre(&controls[0]));

    // `Selected` is canonical since 2026-08-31; `Checked` is what the
    // interpreter actually reads for ISSELECTED. Both, and they must agree.
    for key in ["Selected", "Checked", "Value"] {
        let v = wrote(&updates, "Rad-Card", key)
            .unwrap_or_else(|| panic!("the click wrote no `{key}`"));
        assert!(is_on(&v), "{key} was written as {v:?}");
    }
}

/// The sibling a radio turns OFF has to be turned off everywhere too, or it
/// reads back selected from COBOL while another button is visibly lit.
#[test]
fn the_radio_it_deselects_reads_back_deselected() {
    let ctx = egui::Context::default();
    let mut lit = toggle("Rad-Cash", ControlType::RadioButton, 80);
    lit.set_prop("Selected", PropValue::String("true".into()));
    let controls = vec![toggle("Rad-Card", ControlType::RadioButton, 40), lit];

    let updates = click_at(&ctx, &controls, centre(&controls[0]));

    for key in ["Selected", "Checked", "Value"] {
        let v = wrote(&updates, "Rad-Cash", key)
            .unwrap_or_else(|| panic!("the deselected sibling was left with no `{key}`"));
        assert!(!is_on(&v), "the sibling's {key} stayed on as {v:?}");
    }
}

/// The Switch never had the defect — it has always written `Checked`. Pinned so
/// a later tidy-up cannot "harmonise" it onto the broken shape.
#[test]
fn the_switch_still_writes_checked_as_it_always_did() {
    let ctx = egui::Context::default();
    let controls = vec![toggle("Sw-Dark", ControlType::Switch, 40)];
    let updates = click_at(&ctx, &controls, centre(&controls[0]));

    let checked = wrote(&updates, "Sw-Dark", "Checked").expect("the Switch stopped writing Checked");
    assert!(is_on(&checked), "Checked was written as {checked:?}");
}

/// The other direction: COBOL turning a box OFF must be believed, even after
/// the operator has clicked it.
///
/// `Value` and `Checked` are two properties holding one state. The click writes
/// both now — but a COBOL `SET Chk::Checked TO FALSE` writes only `Checked`
/// (the host applies a `StateUpdate` verbatim), and the CheckBox arm reads
/// `Value` FIRST whenever it is non-empty. So once the operator had clicked a
/// box, every later write from the program was painted over by the stale click.
///
/// Asserted behaviourally rather than by inspecting pixels: with the program
/// saying OFF and the stale click saying ON, the next click must turn the box
/// ON — which it can only do if it agreed the box was OFF.
#[test]
fn a_program_turning_a_box_off_is_believed_after_a_click() {
    let ctx = egui::Context::default();
    let controls = vec![toggle("Chk-Email", ControlType::CheckBox, 40)];
    // The operator clicked it on, then the program turned it off.
    let state = LiveState(vec![
        ("Chk-Email".into(), "Value".into(), "1".into()),
        ("Chk-Email".into(), "Checked".into(), "0".into()),
    ]);

    let updates = click_at_with(&ctx, &controls, &state, centre(&controls[0]));
    let checked = wrote(&updates, "Chk-Email", "Checked")
        .expect("the click wrote no `Checked`");
    assert!(
        is_on(&checked),
        "the box was treated as still ticked, so the program's `SET ... TO FALSE` \
         was ignored and the click turned it OFF again (wrote {checked:?})"
    );
}
