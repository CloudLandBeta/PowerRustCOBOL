#![cfg(feature = "render")]
//! A TextBox, a ComboBox and a CheckBox show their `Tooltip` while the pointer
//! rests on them, as a Button always has.
//!
//! Every control carries the property and the designer lets you type it, but
//! the running form only ever showed it on a Button — so the explanation of
//! what a field is for never appeared where it matters most (operator,
//! 2026-09-25: "each textbox must have a tooltip saying what it is for").

use cobolt_forms::containers::ActiveTabs;
use cobolt_forms::model::Rect as MRect;
use cobolt_forms::render::{DesignedState, RenderInput, RenderMode};
use cobolt_forms::{Control, ControlType, PropValue};
use egui::{pos2, Rect, Vec2};

/// Every text the frame painted.
fn texts(shapes: &[egui::epaint::ClippedShape]) -> Vec<String> {
    fn walk(shape: &egui::Shape, out: &mut Vec<String>) {
        match shape {
            egui::Shape::Text(t) => out.push(t.galley.text().to_owned()),
            egui::Shape::Vec(v) => v.iter().for_each(|s| walk(s, out)),
            _ => {}
        }
    }
    let mut out = Vec::new();
    for c in shapes {
        walk(&c.shape, &mut out);
    }
    out
}

/// Rest the pointer on the middle of the control for a second and a half,
/// frame by frame, and report whether its tooltip text was painted.
fn tooltip_painted(ctrl: Control, tip: &str) -> bool {
    let ctx = egui::Context::default();
    let size = Vec2::new(600.0, 400.0);
    let centre = pos2(
        ctrl.rect.x as f32 + ctrl.rect.w as f32 / 2.0,
        ctrl.rect.y as f32 + ctrl.rect.h as f32 / 2.0,
    );
    let controls = vec![ctrl];
    let active = ActiveTabs::new();
    let mut seen = false;
    for frame in 0..30 {
        let mut input = egui::RawInput::default();
        input.screen_rect = Some(Rect::from_min_size(pos2(0.0, 0.0), size));
        input.time = Some(frame as f64 * 0.05);
        // Moved once, then still: every move restarts egui's tooltip delay.
        if frame == 0 {
            input.events = vec![egui::Event::PointerMoved(centre)];
        }
        let mut full = ctx.run_ui(input, |root| {
            egui::CentralPanel::default()
                .frame(egui::Frame::NONE)
                .show(root, |ui| {
                    let inp = RenderInput {
                        controls: &controls,
                        state: &DesignedState,
                        form_size: size,
                        glass: true,
                        mode: RenderMode::Interactive,
                        active_tabs: &active,
                        backdrop: Default::default(),
                    };
                    let _ = cobolt_forms::render::render_form(ui, &inp);
                });
        });
        full.textures_delta.clear();
        if texts(&full.shapes).iter().any(|t| t == tip) {
            seen = true;
        }
    }
    seen
}

fn with_tip(id: &str, ty: ControlType, tip: &str) -> Control {
    let mut c = Control::new(id, ty, 40, 40);
    c.rect = MRect::new(40, 40, 240, 30);
    c.set_prop("Tooltip", PropValue::String(tip.into()));
    c
}

#[test]
fn an_input_control_shows_its_tooltip_as_a_button_does() {
    let t = std::time::Instant::now();
    let mut combo = with_tip("Cmb-Agent", ControlType::ComboBox, "Which agent to set up");
    combo.set_prop("Items", PropValue::String("Agent 1\nAgent 2".into()));
    let cases = [
        // The control case: a Button always showed its tooltip.
        ("Button", with_tip("Btn-X", ControlType::Button, "A button tip")),
        ("TextBox", with_tip("Txt-Url", ControlType::TextBox, "The address of the model's server")),
        ("ComboBox", combo),
        ("CheckBox", with_tip("Chk-Tools", ControlType::CheckBox, "Tick when the model can call tools")),
    ];
    let mut rows = Vec::new();
    for (name, ctrl) in cases {
        let tip = ctrl.get_prop("Tooltip").unwrap().as_str().to_owned();
        let shown = tooltip_painted(ctrl, &tip);
        rows.push(format!("  {name:<9} {:<36} {}", format!("\"{tip}\""), if shown { "shown" } else { "NOT shown" }));
        assert!(shown, "{name}: its Tooltip {tip:?} was never painted");
    }
    println!(
        "\n  ── tooltips on input controls ──\n{}\n  {:.0} ms\n",
        rows.join("\n"),
        t.elapsed().as_secs_f64() * 1000.0
    );
}
