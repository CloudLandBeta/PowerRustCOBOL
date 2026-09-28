// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! A model asked for a process diagram wrote a TEXT drawing inside the
//! Mermaid element, and PowerChat showed "Mermaid '[' diagrams are not
//! supported" above it (operator, 2026-09-28). Not being Mermaid at all, it
//! is painted as the text it is — every line, no refusal.

use cobolt_forms::containers::ActiveTabs;
use cobolt_forms::model::Rect as MRect;
use cobolt_forms::render::{DesignedState, RenderInput, RenderMode};
use cobolt_forms::viewer::{AppendMode, Conversation, MessageRole};
use cobolt_forms::{Control, ControlType, PropValue};
use egui::{pos2, Rect, Vec2};

#[test]
fn a_text_drawing_is_painted_as_text_without_a_refusal() {
    let drawing = "[ Início ] → [ Elaboração ] → { Possui Cláusula Crítica? } → (Sim) → [ Assinatura ]\n\
                   → (Não) → [ Assinatura ]";
    let mut conv = Conversation::new();
    conv.append_as(AppendMode::Markdown, &format!("```mermaid\n{drawing}\n```"), MessageRole::None);
    conv.append_as(AppendMode::Html, &format!("<div class=\"mermaid\">{drawing}</div>"), MessageRole::None);
    let mut c = Control::new("VWR-CHAT", ControlType::Viewer, 20, 20);
    c.rect = MRect::new(20, 20, 900, 500);
    c.set_prop("Layout", PropValue::String("Streamed".into()));
    c.set_prop("_ConversationHtml", PropValue::String(conv.to_html()));
    let form = Vec2::new(960.0, 560.0);
    let ctx = egui::Context::default();
    let controls = [c];
    let mut texts: Vec<String> = Vec::new();
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
        fn walk(s: &egui::Shape, out: &mut Vec<String>) {
            match s {
                egui::Shape::Vec(v) => v.iter().for_each(|s| walk(s, out)),
                egui::Shape::Text(t) => out.push(t.galley.job.text.clone()),
                _ => {}
            }
        }
        texts.clear();
        for s in &out.shapes {
            walk(&s.shape, &mut texts);
        }
    }
    let all = texts.join("\n");
    let shown = all.matches("Possui Cláusula Crítica?").count();
    println!("  text drawing painted {shown} time(s); refusal present: {}", all.contains("not supported"));
    assert_eq!(shown, 2, "the fence and the element both show the drawing: {all}");
    assert!(!all.contains("not supported"), "no refusal for something that is not a diagram");
}
