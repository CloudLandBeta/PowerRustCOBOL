// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! A report takes 80 % of the Viewer, up to 1600 px (operator, 2026-09-30:
//! "unless requested otherwise by the user or dictated by the content width,
//! set the default width in % (80%) up to 1600px"). PowerChat's reports were
//! a 820–960 px column in the middle of a wide Viewer: every template capped
//! its page with a fixed `max-width`, and the Viewer dropped a `max-width`
//! written as a percentage.
//!
//! Measured: a `max-width: N%` box, and the page of every shipped template,
//! painted in a narrow and a wide Viewer.

use cobolt_forms::containers::ActiveTabs;
use cobolt_forms::model::Rect as MRect;
use cobolt_forms::render::{DesignedState, RenderInput, RenderMode};
use cobolt_forms::viewer::{AppendMode, Conversation, MessageRole};
use cobolt_forms::{Control, ControlType, PropValue};
use egui::{pos2, Color32, Rect, Vec2};

const PAGE: Color32 = Color32::from_rgb(10, 11, 12);
const PROBE: Color32 = Color32::from_rgb(13, 14, 15);
const WIDE: Color32 = Color32::from_rgb(16, 17, 18);

/// Paint `page` in a Viewer `width` px wide; every filled rect, by colour.
fn paint(page: &str, width: i32) -> Vec<(Rect, Color32)> {
    let mut conv = Conversation::new();
    conv.append_as(AppendMode::Html, page, MessageRole::None);
    let mut c = Control::new("VWR-CHAT", ControlType::Viewer, 0, 0);
    c.rect = MRect::new(0, 0, width, 1200);
    c.set_prop("Layout", PropValue::String("Streamed".into()));
    c.set_prop("_ConversationHtml", PropValue::String(conv.to_html()));
    let form = Vec2::new(width as f32 + 20.0, 1220.0);
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
    fn walk(s: &egui::Shape, out: &mut Vec<(Rect, Color32)>) {
        match s {
            egui::Shape::Vec(v) => v.iter().for_each(|s| walk(s, out)),
            egui::Shape::Rect(r) => out.push((r.rect, r.fill)),
            _ => {}
        }
    }
    let mut rects = Vec::new();
    shapes.iter().for_each(|s| walk(s, &mut rects));
    rects
}

fn widest(rects: &[(Rect, Color32)], c: Color32) -> Option<Rect> {
    rects.iter().filter(|r| r.1 == c).map(|r| r.0).max_by(|a, b| a.width().total_cmp(&b.width()))
}

/// The probe and the full-width block in the same page tell the CSS-pixel
/// scale and the width a block may take.
fn page_with_probe(body: &str, style: &str) -> String {
    format!(
        "<html><head><style>{style} .probe {{ width: 100px; background: #0d0e0f; height: 4px; }} \
         .full {{ background: #101112; height: 4px; }}</style></head><body>\
         <div class=\"probe\">.</div><div class=\"full\">.</div>{body}</body></html>"
    )
}

#[test]
fn max_width_as_a_percentage_is_honoured() {
    let mut lines = Vec::new();
    for width in [900, 2400] {
        let page = page_with_probe(
            "<div class=\"box\">The report</div>",
            ".box { max-width: 80%; margin: 0 auto; background: #0a0b0c; }",
        );
        let rects = paint(&page, width);
        let avail = widest(&rects, WIDE).expect("the full-width block").width();
        let boxw = widest(&rects, PAGE).expect("the box").width();
        let share = boxw / avail;
        lines.push(format!("  viewer {width:>4} px: available {avail:>7.1}, box {boxw:>7.1} = {:.1} %", share * 100.0));
        assert!((share - 0.80).abs() < 0.01, "max-width: 80% gives 80 % ({share})");
    }
    println!("max-width: 80%\n{}", lines.join("\n"));
}

#[test]
fn every_shipped_template_is_80_percent_of_the_viewer_up_to_1600_px() {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../examples/PowerChat/samples/report-templates.txt");
    let text = std::fs::read_to_string(path).expect("the shipped templates");
    let mut templates: Vec<(String, String)> = Vec::new();
    for part in text.split("\n=== ").map(|p| p.trim_start_matches("=== ")) {
        let (name, rest) = part.split_once('\n').expect("a name line");
        let html = rest.split_once('\n').map(|(_suits, h)| h).unwrap_or("");
        templates.push((name.trim().to_string(), html.to_string()));
    }
    assert_eq!(templates.len(), 11, "the eleven shipped templates");

    let mut table = Vec::new();
    for (name, html) in &templates {
        assert!(html.contains("<div class=\"rp-page\">"), "{name}: the page wrapper");
        // Colour the page so it can be measured; the probe tells the scale.
        let marked = html.replacen(
            "</style>",
            ".rp-page { background: #0a0b0c; } .probe { width: 100px; background: #0d0e0f; height: 4px; } \
             .full { background: #101112; height: 4px; }</style>",
            1,
        );
        let marked = marked.replacen("<body>", "<body><div class=\"probe\">.</div><div class=\"full\">.</div>", 1);
        for width in [1000, 2600] {
            let rects = paint(&marked, width);
            let k = widest(&rects, PROBE).expect("the probe").width() / 100.0;
            let avail = widest(&rects, WIDE).expect("the full-width block").width();
            let pagew = widest(&rects, PAGE).unwrap_or_else(|| panic!("{name}: the page is painted")).width();
            let expect = (avail * 0.8).min(1600.0 * k);
            table.push(format!(
                "  {name:<14} viewer {width:>4}: page {pagew:>7.1} of {avail:>7.1} ({:>5.1} %), expected {expect:>7.1}",
                pagew / avail * 100.0
            ));
            assert!((pagew - expect).abs() < 2.0, "{name} at {width}: page {pagew}, expected {expect}");
        }
    }
    println!("Report page width, 11 templates x 2 viewer widths:\n{}", table.join("\n"));
}
