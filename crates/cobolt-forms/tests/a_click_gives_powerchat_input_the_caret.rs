#![cfg(feature = "render")]
//! A click on PowerChat's question box gives it the caret.
//!
//! Operator report, 2026-10-02: under Run Form, PowerChat's buttons answered
//! clicks but `Txt-Input` never took the caret, so nothing could be typed.
//! This drives the real `chat-form.cfrm` through the render engine.

use cobolt_forms::containers::ActiveTabs;
use cobolt_forms::render::{control_widget_id, DesignedState, RenderInput, RenderMode};
use egui::{pos2, Rect, Vec2};

fn chat_form() -> cobolt_forms::Form {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../examples/PowerChat/forms/chat-form.cfrm");
    cobolt_forms::load_form(&path).expect("chat-form loads")
}

fn frame(ctx: &egui::Context, form: &cobolt_forms::Form, events: Vec<egui::Event>) {
    let size = Vec2::new(form.width as f32, form.height as f32);
    let active = ActiveTabs::new();
    let mut input = egui::RawInput::default();
    input.screen_rect = Some(Rect::from_min_size(pos2(0.0, 0.0), size));
    input.events = events;
    let mut full = ctx.run_ui(input, |root| {
        egui::CentralPanel::default().frame(egui::Frame::NONE).show(root, |ui| {
            let inp = RenderInput {
                controls: &form.controls,
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

fn click_at(p: egui::Pos2) -> [Vec<egui::Event>; 2] {
    let button = |pressed| egui::Event::PointerButton {
        pos: p,
        button: egui::PointerButton::Primary,
        pressed,
        modifiers: Default::default(),
    };
    [vec![egui::Event::PointerMoved(p), button(true)], vec![button(false)]]
}

/// Anywhere inside the box's text area takes the caret — the first line, the
/// middle and near the bottom. Before the fix only a band one text row high at
/// the top did, so a click in the middle of the 104-px box did nothing.
#[test]
fn a_click_anywhere_in_txt_input_gives_it_the_caret() {
    let form = chat_form();
    let input = form.controls.iter().find(|c| c.id == "Txt-Input").expect("Txt-Input");
    let r = input.rect;
    let want = control_widget_id(None, "Txt-Input");
    let heights = [12, r.h / 2, r.h - 20];
    for dy in heights {
        let p = pos2((r.x + r.w / 2) as f32, (r.y + dy) as f32);
        let ctx = egui::Context::default();
        frame(&ctx, &form, vec![]);
        frame(&ctx, &form, vec![]);
        let [down, up] = click_at(p);
        frame(&ctx, &form, down);
        frame(&ctx, &form, up);
        frame(&ctx, &form, vec![]);
        assert_eq!(ctx.memory(|m| m.focused()), Some(want), "a click {dy} px down must give Txt-Input the caret");
    }
    println!("PowerChat Txt-Input ({}x{} multiline): clicks at {heights:?} px down all take the caret", r.w, r.h);
}
