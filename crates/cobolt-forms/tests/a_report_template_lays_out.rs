// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! The layouts PowerChat's report templates are built from reach the screen
//! as a browser draws them (operator, 2026-09-28: "all the templates are
//! horrible"): an empty grid cell keeps its place, a box stretched to its row
//! centres what it holds, a pyramid's tiers keep their stated widths, a
//! circle centres its column, a short tag is never squeezed to one letter per
//! line, and a big figure's unitless line-height is its own.

use cobolt_forms::containers::ActiveTabs;
use cobolt_forms::model::Rect as MRect;
use cobolt_forms::render::{DesignedState, RenderInput, RenderMode};
use cobolt_forms::viewer::{AppendMode, Conversation, MessageRole};
use cobolt_forms::{Control, ControlType, PropValue};
use egui::{pos2, Color32, Rect, Vec2};

type Rects = Vec<(Rect, Color32)>;
type Texts = Vec<(String, Rect)>;

fn render(page: &str) -> (Rects, Texts) {
    let mut conv = Conversation::new();
    conv.append_as(AppendMode::Html, page, MessageRole::None);
    let mut c = Control::new("VWR-CHAT", ControlType::Viewer, 20, 20);
    c.rect = MRect::new(20, 20, 800, 900);
    c.set_prop("Layout", PropValue::String("Streamed".into()));
    c.set_prop("_ConversationHtml", PropValue::String(conv.to_html()));
    let form = Vec2::new(860.0, 960.0);
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
    fn walk(s: &egui::Shape, rects: &mut Rects, texts: &mut Texts) {
        match s {
            egui::Shape::Vec(v) => v.iter().for_each(|s| walk(s, rects, texts)),
            egui::Shape::Rect(r) => rects.push((r.rect, r.fill)),
            egui::Shape::Text(t) => texts.push((t.galley.job.text.clone(), t.galley.rect.translate(t.pos.to_vec2()))),
            _ => {}
        }
    }
    let (mut rects, mut texts) = (Vec::new(), Vec::new());
    for s in &shapes {
        walk(s, &mut rects, &mut texts);
    }
    (rects, texts)
}

fn filled(rects: &Rects, rgb: (u8, u8, u8)) -> Vec<Rect> {
    let c = Color32::from_rgb(rgb.0, rgb.1, rgb.2);
    let mut v: Vec<Rect> = rects.iter().filter(|r| r.1 == c).map(|r| r.0).collect();
    v.sort_by(|a, b| a.min.y.total_cmp(&b.min.y).then(a.min.x.total_cmp(&b.min.x)));
    v
}

fn text(texts: &Texts, needle: &str) -> Rect {
    texts.iter().find(|t| t.0.contains(needle)).map(|t| t.1).unwrap_or_else(|| panic!("{needle} is drawn"))
}

#[test]
fn a_template_grid_keeps_its_cells_and_centres_them() {
    // A vertical timeline row: an EMPTY cell, a spine cell stretched to the
    // row and centring its node, a tall text cell.
    let page = r#"<style>
      .g { display: grid; grid-template-columns: 100px 40px 1fr; }
      .spine { background: #112233; display: flex; align-items: center; justify-content: center; }
      .node { width: 14px; height: 14px; background: #aa0000; }
      .t { padding: 30px 8px; }
    </style>
    <div class="g"><div></div><div class="spine"><div class="node"></div></div><div class="t"><p>2012 tall text</p></div></div>"#;
    let (rects, texts) = render(page);
    let spine = filled(&rects, (0x11, 0x22, 0x33))[0];
    let node = filled(&rects, (0xaa, 0, 0))[0];
    let label = text(&texts, "2012");
    println!("  spine {spine:?}\n  node {node:?}\n  text {label:?}");
    assert!(spine.min.x > 100.0 + 20.0 - 1.0, "the empty first cell keeps its 100 px: {spine:?}");
    assert!(label.min.x > spine.max.x, "the text is in the third column, not the second");
    assert!(spine.height() > 60.0, "the spine is stretched to the row: {spine:?}");
    assert!((node.center().y - spine.center().y).abs() < 2.0, "its node is centred in the stretched cell");
}

#[test]
fn a_pyramid_a_circle_and_a_tag() {
    let page = r#"<style>
      .p { display: flex; flex-direction: column; align-items: center; gap: 4px; }
      .t1 { width: 30%; height: 30px; background: #010203; }
      .t2 { width: 90%; height: 30px; background: #040506; }
      .c { display: flex; flex-direction: column; align-items: center; justify-content: center; height: 200px;
           width: 200px; border-radius: 50%; background: #070809; }
      .r { display: flex; align-items: center; gap: 10px; }
      .tag { background: #0a0b0c; color: #ffffff; padding: 2px 8px; }
      .fig { font-size: 64px; line-height: 1.2; }
    </style>
    <div class="p"><div class="t1">top</div><div class="t2">base</div></div>
    <div class="c"><p>ICON</p><p>Contract</p></div>
    <div class="r"><span class="tag">YES</span><p>A long explanation that takes far more than the room of the row it sits in, so the row must give something up somewhere.</p></div>
    <p class="fig">48%</p><p>under the figure</p>"#;
    let (rects, texts) = render(page);
    let top = filled(&rects, (1, 2, 3))[0];
    let base = filled(&rects, (4, 5, 6))[0];
    let circle = filled(&rects, (7, 8, 9))[0];
    let tag = filled(&rects, (0x0a, 0x0b, 0x0c))[0];
    let (icon, name) = (text(&texts, "ICON"), text(&texts, "Contract"));
    let (fig, under) = (text(&texts, "48%"), text(&texts, "under the figure"));
    println!("  tiers {top:?} / {base:?}\n  circle {circle:?}: {icon:?} {name:?}\n  tag {tag:?}\n  figure {fig:?} / {under:?}");
    assert!(base.width() > top.width() * 2.5, "the tiers keep their stated widths (30 % and 90 %)");
    assert!((top.center().x - base.center().x).abs() < 2.0, "both centred");
    let mid = (icon.min.y + name.max.y) / 2.0;
    assert!((mid - circle.center().y).abs() < 6.0, "the circle's column is centred in its height");
    assert!(tag.height() < 40.0, "the tag stays on one line: {tag:?}");
    assert!(under.min.y >= fig.max.y - 2.0, "a 64 px figure with line-height 1.2 does not run into the next line");
}
