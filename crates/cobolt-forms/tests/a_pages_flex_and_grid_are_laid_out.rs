// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! A page's CSS LAYOUT reaches the screen (operator, 2026-09-28: an HTML
//! answer "ugly as hell" — a round step number came out as a full-width blue
//! bar). The infographic shapes a model writes are painted: a flex row puts a
//! 40 px circle BESIDE its text, centred on it, and a three-column grid puts
//! three cards side by side on one line.

use cobolt_forms::containers::ActiveTabs;
use cobolt_forms::model::Rect as MRect;
use cobolt_forms::render::{DesignedState, RenderInput, RenderMode};
use cobolt_forms::viewer::{AppendMode, Conversation, MessageRole};
use cobolt_forms::{Control, ControlType, PropValue};
use egui::{pos2, Color32, Rect, Vec2};

const PAGE: &str = r#"<html><head><style>
  .step { display: flex; align-items: center; gap: 16px; margin: 12px 0 }
  .num { width: 40px; height: 40px; border-radius: 50%; background: #e74c3c; color: #fff;
         display: flex; align-items: center; justify-content: center; font-weight: bold }
  .body { flex: 1 }
  .grid { display: grid; grid-template-columns: repeat(3, 1fr); gap: 20px }
  .card { background: #fef9e7; padding: 12px; border-radius: 8px }
</style></head><body>
  <div class="step"><div class="num">1</div><div class="body"><p>Define the contract</p></div></div>
  <div class="grid">
    <div class="card"><p>Legal</p></div>
    <div class="card"><p>Finance</p></div>
    <div class="card"><p>Management</p></div>
  </div>
</body></html>"#;

#[test]
fn a_flex_row_and_a_grid_are_laid_out_side_by_side() {
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

    type Rects = Vec<(Rect, Color32, f32)>;
    fn walk(s: &egui::Shape, rects: &mut Rects, texts: &mut Vec<(String, Rect)>) {
        match s {
            egui::Shape::Vec(v) => v.iter().for_each(|s| walk(s, rects, texts)),
            egui::Shape::Rect(r) => rects.push((r.rect, r.fill, r.corner_radius.nw as f32)),
            egui::Shape::Text(t) => texts.push((t.galley.job.text.clone(), t.galley.rect.translate(t.pos.to_vec2()))),
            _ => {}
        }
    }
    let (mut rects, mut texts) = (Vec::new(), Vec::new());
    for s in &shapes {
        walk(s, &mut rects, &mut texts);
    }
    let red = Color32::from_rgb(0xe7, 0x4c, 0x3c);
    let cream = Color32::from_rgb(0xfe, 0xf9, 0xe7);
    let circle = rects.iter().find(|r| r.1 == red).expect("the step number's red box is painted").clone();
    let text = |needle: &str| texts.iter().find(|t| t.0.contains(needle)).map(|t| t.1).expect(needle);
    let digit = text("1");
    let label = text("Define the contract");
    let mut cards: Vec<Rect> = rects.iter().filter(|r| r.1 == cream).map(|r| r.0).collect();
    cards.sort_by(|a, b| a.min.x.total_cmp(&b.min.x));

    println!("\n  ── a page's flex and grid, laid out ──");
    println!("  step number box: {:?}, corner radius {}", circle.0, circle.2);
    println!("  digit at {:?}; label at {:?}", digit, label);
    for (i, c) in cards.iter().enumerate() {
        println!("  card {}: {:?}", i + 1, c);
    }
    println!();

    let (r, _, radius) = circle;
    assert!((r.width() - r.height()).abs() < 1.5, "the number box is square: {r:?}");
    assert!(r.width() < 80.0, "the number box keeps its own width, not the row's: {r:?}");
    assert!(radius >= r.width() / 2.0 - 1.5, "border-radius: 50% rounds it into a circle");
    assert!(r.contains(digit.center()), "the digit sits inside its circle");
    assert!((digit.center().x - r.center().x).abs() < 3.0, "centred across (justify-content)");
    assert!((digit.center().y - r.center().y).abs() < 3.0, "centred down (align-items)");
    assert!(label.min.x > r.max.x, "the label is BESIDE the circle, not under it");
    assert!((label.center().y - r.center().y).abs() < 4.0, "and centred on it (align-items: center)");
    assert_eq!(cards.len(), 3, "three cards");
    assert!(cards.iter().all(|c| (c.min.y - cards[0].min.y).abs() < 0.5), "on one line");
    assert!(cards[1].min.x > cards[0].max.x && cards[2].min.x > cards[1].max.x, "side by side, apart");
    let w0 = cards[0].width();
    assert!(cards.iter().all(|c| (c.width() - w0).abs() < 1.0), "equal columns (repeat(3, 1fr))");
    assert!((cards[1].min.x - cards[0].max.x - cards[2].min.x + cards[1].max.x).abs() < 1.0, "an even gap");
}
