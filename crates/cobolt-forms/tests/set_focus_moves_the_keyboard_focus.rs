#![cfg(feature = "render")]
//! `ctl::SetFocus()` gives that control the keyboard focus.
//!
//! It did nothing (developer report, 2026-10-01): the interpreter stored
//! `Focused = 1` and no renderer read it. A validation handler that sent the
//! caret back to the field in error left it wherever Enter had taken it, and
//! `SetFocus()` on the NEXT field only looked as if it worked because Enter had
//! already gone there.
//!
//! The interpreter now writes a never-repeated sequence number to
//! [`SET_FOCUS_PROP`]; these tests write that number the way the host's live
//! state does and check where the focus lands.

use cobolt_forms::containers::ActiveTabs;
use cobolt_forms::model::Rect as MRect;
use cobolt_forms::render::{
    control_widget_id, DesignedState, RenderInput, RenderMode, SET_FOCUS_PROP,
};
use cobolt_forms::{Control, ControlType, PropValue};
use egui::{pos2, Key, Rect, Vec2};

fn text_box(id: &str, y: i32) -> Control {
    let mut c = Control::new(id, ControlType::TextBox, 20, y);
    c.rect = MRect::new(20, y, 200, 24);
    c
}

/// Two fields, as in the report: a code, then a name. `seq` is what each
/// one's `SetFocus()` sequence currently reads (`None` — never called).
fn form(code_seq: Option<u64>, name_seq: Option<u64>) -> Vec<Control> {
    let mut code = text_box("EDT-CODIGO", 20);
    let mut name = text_box("EDT-NOME", 60);
    for (c, seq) in [(&mut code, code_seq), (&mut name, name_seq)] {
        if let Some(n) = seq {
            c.set_prop(SET_FOCUS_PROP, PropValue::String(n.to_string()));
        }
    }
    vec![code, name]
}

fn frame_in(ctx: &egui::Context, controls: &[Control], events: Vec<egui::Event>, blocked: bool) {
    let size = Vec2::new(400.0, 200.0);
    let active = ActiveTabs::new();
    let mut input = egui::RawInput::default();
    input.screen_rect = Some(Rect::from_min_size(pos2(0.0, 0.0), size));
    input.events = events;
    let mut full = ctx.run_ui(input, |root| {
        egui::CentralPanel::default()
            .frame(egui::Frame::NONE)
            .show(root, |ui| {
                // What the host does while a modal child form is open.
                if blocked {
                    ui.disable();
                }
                let inp = RenderInput {
                    controls,
                    state: &DesignedState,
                    form_size: size,
                    glass: true,
                    mode: RenderMode::Interactive,
                    active_tabs: &active,
                    backdrop: Default::default(),
                };
                cobolt_forms::render::render_form(ui, &inp);
            });
    });
    full.textures_delta.clear();
}

fn frame(ctx: &egui::Context, controls: &[Control]) {
    frame_in(ctx, controls, vec![], false);
}

fn focused(ctx: &egui::Context, id: &str) -> bool {
    ctx.memory(|m| m.has_focus(control_widget_id(None, id)))
}

fn put_focus(ctx: &egui::Context, controls: &[Control], id: &str) {
    ctx.memory_mut(|m| m.request_focus(control_widget_id(None, id)));
    frame(ctx, controls);
}

#[test]
fn set_focus_moves_the_focus_to_that_control() {
    let ctx = egui::Context::default();
    put_focus(&ctx, &form(None, None), "EDT-CODIGO");
    assert!(focused(&ctx, "EDT-CODIGO"));

    frame(&ctx, &form(None, Some(1)));
    frame(&ctx, &form(None, Some(1)));
    assert!(focused(&ctx, "EDT-NOME"), "SetFocus() on the name field must move the caret there");
}

/// The case that was reported: Enter has already taken the caret to the next
/// field when the validation handler — which Enter woke — answers with
/// `SetFocus()` on the field in error. The handler has the last word.
#[test]
fn set_focus_brings_the_caret_back_after_enter_moved_it_on() {
    let ctx = egui::Context::default();
    put_focus(&ctx, &form(None, None), "EDT-CODIGO");
    frame(&ctx, &form(None, None)); // the field claims Enter from this frame on

    let enter = vec![egui::Event::Key {
        key: Key::Enter,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: Default::default(),
    }];
    frame_in(&ctx, &form(None, None), enter, false);
    frame(&ctx, &form(None, None));
    assert!(focused(&ctx, "EDT-NOME"), "precondition: Enter advanced to the next field");

    // The handler's reply arrives on a later frame.
    frame(&ctx, &form(Some(7), None));
    frame(&ctx, &form(Some(7), None));
    assert!(focused(&ctx, "EDT-CODIGO"), "SetFocus() must send the caret back to the field in error");
}

/// A request is acted on ONCE: the operator can click away afterwards and the
/// focus is not dragged back every frame. A NEW call on the same control is a
/// new request, and works again.
#[test]
fn a_request_is_spent_once_and_a_new_call_on_the_same_control_works_again() {
    let ctx = egui::Context::default();
    frame(&ctx, &form(Some(1), None));
    frame(&ctx, &form(Some(1), None));
    assert!(focused(&ctx, "EDT-CODIGO"));

    put_focus(&ctx, &form(Some(1), None), "EDT-NOME");
    frame(&ctx, &form(Some(1), None));
    assert!(focused(&ctx, "EDT-NOME"), "a spent request must not pull the focus back");

    frame(&ctx, &form(Some(2), None));
    frame(&ctx, &form(Some(2), None));
    assert!(focused(&ctx, "EDT-CODIGO"), "a second SetFocus() on the same control is a new request");
}

/// `SetFocus()` called while a modal child blocks the form waits for the form
/// to be released instead of being lost.
#[test]
fn a_request_made_while_the_form_is_blocked_lands_when_it_is_released() {
    let ctx = egui::Context::default();
    put_focus(&ctx, &form(None, None), "EDT-NOME");

    frame_in(&ctx, &form(Some(3), None), vec![], true);
    frame_in(&ctx, &form(Some(3), None), vec![], true);

    frame(&ctx, &form(Some(3), None));
    frame(&ctx, &form(Some(3), None));
    assert!(focused(&ctx, "EDT-CODIGO"), "the request must survive the modal");
}
