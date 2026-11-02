#![cfg(feature = "render")]
// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Spec 110: a TextBox caption's background band reaches the last character of the text that is drawn, with no padding
//! added. The band used to be sized from the built-in regular face's width table and stopped short of the last letter of a
//! caption drawn in the system's wider bold face.

use cobolt_forms::model::Rect as MRect;
use cobolt_forms::paint::draw_control;
use cobolt_forms::{Control, ControlType, PropValue};
use egui::{epaint::Shape, pos2, Rect, Vec2};

fn walk(s: &Shape, band: &mut Option<Rect>, text: &mut Option<Rect>) {
    match s {
        Shape::Vec(v) => v.iter().for_each(|s| walk(s, band, text)),
        Shape::Rect(r) if r.fill.r() > 200 && r.fill.g() < 50 && r.fill.b() < 50 => *band = Some(r.rect),
        Shape::Text(t) if t.galley.text().contains("Generated") => *text = Some(Rect::from_min_size(t.pos, t.galley.size())),
        _ => {}
    }
}

#[test]
fn a_caption_band_covers_every_character_and_adds_no_padding() {
    let (mut worst, mut cases) = (f32::MAX, 0);
    for (bold, size) in [(false, 14), (true, 14), (true, 28), (true, 40)] {
        let mut c = Control::new("TXT-NOTES", ControlType::TextBox, 20, 60);
        c.rect = MRect::new(20, 60, 400, 120);
        c.set_prop("CaptionText", PropValue::String("Generated notes".into()));
        c.set_prop("CaptionBackColor", PropValue::String("#FF0000".into()));
        c.set_prop("CaptionFontSize", PropValue::Int(size));
        c.set_prop("CaptionFontBold", PropValue::Bool(bold));
        let ctx = egui::Context::default();
        ctx.set_fonts(cobolt_forms::fonts::base_font_definitions());
        let mut input = egui::RawInput::default();
        input.screen_rect = Some(Rect::from_min_size(pos2(0.0, 0.0), Vec2::new(600.0, 300.0)));
        let mut out = ctx.run_ui(input, |ui| draw_control(ui.painter(), pos2(0.0, 0.0), &c, false, true, 1.0, 1.0, None));
        out.textures_delta.clear();
        let (mut band, mut text) = (None, None);
        for s in &out.shapes {
            walk(&s.shape, &mut band, &mut text);
        }
        let (band, text) = (band.expect("the band is painted"), text.expect("the caption is painted"));
        let past = band.max.x - text.max.x;
        assert!(past >= -0.01, "bold {bold} size {size}: the band stops {} px before the end of the text", -past);
        assert!(past < 1.0, "bold {bold} size {size}: the band is {past} px wider than the text, which is padding");
        assert!((band.min.x - text.min.x).abs() < 0.01, "bold {bold} size {size}: the band starts away from the text");
        worst = worst.min(past);
        cases += 1;
    }
    println!("TextBox caption band: {cases} cases (plain 14, bold 14, 28, 40 pt), least band past the last letter {worst:.2} px, no padding");
}
