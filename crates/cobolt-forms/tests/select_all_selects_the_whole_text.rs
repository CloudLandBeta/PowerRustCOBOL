#![cfg(feature = "render")]
//! `ctl::SelectAll()` focuses that TextBox and selects its whole text.
//!
//! It was accepted and did nothing (2026-10-01): the interpreter's arm returned
//! without writing anything, while the IntelliSense and the System KB both
//! offered it on a TextBox.
//!
//! The interpreter now writes a never-repeated sequence number to
//! [`SELECT_ALL_PROP`]; these tests write that number the way the host's live
//! state does and read the selection back from egui's own editor state.

use cobolt_forms::containers::ActiveTabs;
use cobolt_forms::model::Rect as MRect;
use cobolt_forms::render::{
    control_widget_id, DesignedState, RenderInput, RenderMode, SELECT_ALL_PROP,
};
use cobolt_forms::{Control, ControlType, PropValue};
use egui::{pos2, Rect, Vec2};

fn text_box(id: &str, y: i32, text: &str) -> Control {
    let mut c = Control::new(id, ControlType::TextBox, 20, y);
    c.rect = MRect::new(20, y, 200, 24);
    c.set_prop("Text", PropValue::String(text.to_owned()));
    c
}

/// A code field and a name field. `seq` is what the name field's
/// `SelectAll()` sequence currently reads (`None` — never called).
fn form(name_seq: Option<u64>) -> Vec<Control> {
    let code = text_box("EDT-CODIGO", 20, "0042");
    let mut name = text_box("EDT-NOME", 60, "José da Silva");
    if let Some(n) = name_seq {
        name.set_prop(SELECT_ALL_PROP, PropValue::String(n.to_string()));
    }
    vec![code, name]
}

fn frame(ctx: &egui::Context, controls: &[Control]) {
    let size = Vec2::new(400.0, 200.0);
    let active = ActiveTabs::new();
    let mut input = egui::RawInput::default();
    input.screen_rect = Some(Rect::from_min_size(pos2(0.0, 0.0), size));
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
                cobolt_forms::render::render_form(ui, &inp);
            });
    });
    full.textures_delta.clear();
}

fn focused(ctx: &egui::Context, id: &str) -> bool {
    ctx.memory(|m| m.has_focus(control_widget_id(None, id)))
}

/// The selected character range of the field's editor, sorted.
fn selection(ctx: &egui::Context, id: &str) -> Option<(usize, usize)> {
    let state = egui::text_edit::TextEditState::load(ctx, control_widget_id(None, id))?;
    let r = state.cursor.char_range()?.as_sorted_char_range();
    Some((r.start.0, r.end.0))
}

fn put_focus(ctx: &egui::Context, controls: &[Control], id: &str) {
    ctx.memory_mut(|m| m.request_focus(control_widget_id(None, id)));
    frame(ctx, controls);
}

/// The whole text — counted in characters, so the accented "é" is one.
const NAME_CHARS: usize = 13;

#[test]
fn select_all_focuses_the_text_box_and_selects_every_character() {
    let ctx = egui::Context::default();
    put_focus(&ctx, &form(None), "EDT-CODIGO");
    assert!(focused(&ctx, "EDT-CODIGO"));

    frame(&ctx, &form(Some(1)));
    // …and the selection is still whole on the frames after: the editor must
    // not collapse it to a caret once it has the focus.
    frame(&ctx, &form(Some(1)));
    frame(&ctx, &form(Some(1)));
    assert!(focused(&ctx, "EDT-NOME"), "SelectAll() must give the field the focus");
    assert_eq!(
        selection(&ctx, "EDT-NOME"),
        Some((0, NAME_CHARS)),
        "SelectAll() must select every character of the text"
    );
}

/// A request is acted on once — the operator can move away afterwards — and
/// a SECOND call on the same field is a new request that selects again.
#[test]
fn a_second_select_all_on_the_same_field_selects_again() {
    let ctx = egui::Context::default();
    frame(&ctx, &form(Some(1)));
    frame(&ctx, &form(Some(1)));
    assert_eq!(selection(&ctx, "EDT-NOME"), Some((0, NAME_CHARS)));

    // The operator clicks into the other field: the spent request must not
    // drag the focus back.
    put_focus(&ctx, &form(Some(1)), "EDT-CODIGO");
    frame(&ctx, &form(Some(1)));
    assert!(focused(&ctx, "EDT-CODIGO"), "a spent request must not pull the focus back");

    frame(&ctx, &form(Some(2)));
    frame(&ctx, &form(Some(2)));
    assert!(focused(&ctx, "EDT-NOME"), "a second SelectAll() must focus the field again");
    assert_eq!(
        selection(&ctx, "EDT-NOME"),
        Some((0, NAME_CHARS)),
        "a second SelectAll() must select the whole text again"
    );
}
