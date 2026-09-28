// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! A page's CSS reaches the screen (operator, 2026-09-27: "the page's CSS must
//! be applied too"). The page goes into a conversation the way PowerChat puts
//! an HTML answer there — `AppendHtml`, no role — and one frame is painted:
//! the gradient header, the table's header colour, the white heading and the
//! card's white box must be among the shapes, not just in the model.

use cobolt_forms::containers::ActiveTabs;
use cobolt_forms::model::Rect as MRect;
use cobolt_forms::render::{DesignedState, RenderInput, RenderMode};
use cobolt_forms::viewer::{AppendMode, Conversation, MessageRole};
use cobolt_forms::{Control, ControlType, PropValue};
use egui::{pos2, Color32, Rect, Vec2};

const PAGE: &str = r#"<html><head><style>
  :root { --primary: #667eea; }
  body { background: #f4f6fb; color: #333; }
  header { background: linear-gradient(135deg, var(--primary), #764ba2); color: white; padding: 30px; border-radius: 12px; text-align: center }
  .card { background: #ffffff; border-left: 4px solid var(--primary); padding: 16px; margin: 16px 0 }
  th { background: var(--primary); color: #fff; padding: 12px }
  td { padding: 10px; border-bottom: 1px solid #dddddd }
</style></head><body>
  <header><h1>Contracts</h1></header>
  <div class="card"><p>A card</p></div>
  <table><tr><th>Value</th><th>Approval</th></tr><tr><td>A</td><td>1</td></tr></table>
</body></html>"#;

#[test]
fn a_pages_css_is_painted_in_the_conversation() {
    let mut conv = Conversation::new();
    conv.append_as(AppendMode::Html, PAGE, MessageRole::None);
    let mut c = Control::new("VWR-CHAT", ControlType::Viewer, 20, 20);
    c.rect = MRect::new(20, 20, 800, 640);
    c.set_prop("Layout", PropValue::String("Streamed".into()));
    c.set_prop("_ConversationHtml", PropValue::String(conv.to_html()));
    let form = Vec2::new(860.0, 700.0);
    let ctx = egui::Context::default();
    let controls = [c];
    let mut shapes = Vec::new();
    for _ in 0..2 {
        let active = ActiveTabs::new();
        let mut input = egui::RawInput::default();
        input.screen_rect = Some(Rect::from_min_size(pos2(0.0, 0.0), form));
        let mut out = ctx.run_ui(input, |root| {
            egui::CentralPanel::default().frame(egui::Frame::NONE).show(root, |ui| {
                let inp = RenderInput {
                    controls: &controls,
                    state: &DesignedState,
                    form_size: form,
                    glass: true,
                    mode: RenderMode::Interactive,
                    active_tabs: &active,
                    backdrop: Default::default(),
                };
                cobolt_forms::render::render_form(ui, &inp);
            });
        });
        out.textures_delta.clear();
        shapes = out.shapes.into_iter().map(|c| c.shape).collect();
    }

    fn walk(s: &egui::Shape, fills: &mut Vec<Color32>, meshes: &mut Vec<usize>, inks: &mut Vec<Color32>) {
        match s {
            egui::Shape::Vec(v) => v.iter().for_each(|s| walk(s, fills, meshes, inks)),
            egui::Shape::Rect(r) => fills.push(r.fill),
            egui::Shape::Mesh(m) => meshes.push(m.vertices.len()),
            egui::Shape::Text(t) => inks.extend(t.galley.job.sections.iter().map(|s| s.format.color)),
            _ => {}
        }
    }
    let (mut fills, mut meshes, mut inks) = (Vec::new(), Vec::new(), Vec::new());
    for s in &shapes {
        walk(s, &mut fills, &mut meshes, &mut inks);
    }
    let primary = Color32::from_rgb(0x66, 0x7e, 0xea);
    let gradient = meshes.iter().filter(|v| **v > 100).count();
    println!("\n  ── a page's CSS, painted ──");
    println!("  gradient meshes: {gradient} (vertices {meshes:?})");
    println!("  #667eea fills: {}; white fills: {}; body fill: {}", 
        fills.iter().filter(|f| **f == primary).count(),
        fills.iter().filter(|f| **f == Color32::WHITE).count(),
        fills.contains(&Color32::from_rgb(0xf4, 0xf6, 0xfb)));
    println!("  white ink runs: {}\n", inks.iter().filter(|c| **c == Color32::WHITE).count());
    assert!(gradient >= 1, "the header's linear-gradient is painted as a mesh");
    assert!(fills.contains(&primary), "the table header's background, from var(--primary)");
    assert!(fills.contains(&Color32::WHITE), "the card's white box");
    assert!(fills.contains(&Color32::from_rgb(0xf4, 0xf6, 0xfb)), "the body's background");
    assert!(inks.contains(&Color32::WHITE), "the heading's white text");
}
