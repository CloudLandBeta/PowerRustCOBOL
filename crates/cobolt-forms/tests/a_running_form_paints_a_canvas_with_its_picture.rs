// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! A running form (the interactive renderer) paints a Canvas2D with the picture the host made from its
//! last `Present`, not with the dashed placeholder the Designer shows.
//!
//! **The report (operator, 2026-11-08):** in PowerGraphics no chart, no game and no 3D view appeared when
//! the form ran; each canvas showed its name, kind and size in a dashed frame. The picture was drawn,
//! published and turned into a texture by the host, but the interactive renderer reached a Canvas through
//! its catch-all arm, which painted every control with no texture.

#![cfg(feature = "render")]

use cobolt_forms::containers::ActiveTabs;
use cobolt_forms::model::{Control, ControlType, Rect as MRect};
use cobolt_forms::paint::canvas_texture_key;
use cobolt_forms::render::{DesignedState, RenderInput, RenderMode};
use egui::{pos2, Rect, Vec2};

fn walk(s: &egui::Shape, out: &mut Vec<String>) {
    match s {
        egui::Shape::Vec(v) => v.iter().for_each(|s| walk(s, out)),
        egui::Shape::Text(t) => out.push(t.galley.text().to_string()),
        egui::Shape::Mesh(m) => {
            if m.texture_id != egui::TextureId::default() {
                out.push("<picture>".to_string());
            }
        }
        _ => {}
    }
}

/// What a running form paints for one Canvas2D: its texts, and `<picture>` for every user texture drawn.
fn painted(with_picture: bool) -> Vec<String> {
    let mut canvas = Control::new("CV-1", ControlType::Canvas2D, 10, 10);
    canvas.rect = MRect::new(10, 10, 200, 100);
    let controls = vec![canvas];
    let size = Vec2::new(320.0, 240.0);
    let ctx = egui::Context::default();
    if with_picture {
        // What the host does for a `Present`: one texture per canvas, kept under the canvas's key.
        let image = egui::ColorImage::new([4, 4], vec![egui::Color32::from_rgb(200, 30, 30); 16]);
        let handle = ctx.load_texture("canvas2d:CV-1", image, egui::TextureOptions::LINEAR);
        ctx.data_mut(|d| d.insert_temp(canvas_texture_key("CV-1"), handle));
    }
    let active = ActiveTabs::new();
    let mut found = Vec::new();
    for _ in 0..2 {
        let mut input = egui::RawInput::default();
        input.screen_rect = Some(Rect::from_min_size(pos2(0.0, 0.0), size));
        let mut full = ctx.run_ui(input, |root| {
            egui::CentralPanel::default().frame(egui::Frame::NONE).show(root, |ui| {
                let inp = RenderInput {
                    controls: &controls,
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
        found.clear();
        for cs in &full.shapes {
            walk(&cs.shape, &mut found);
        }
    }
    found
}

#[test]
fn a_running_canvas_shows_its_picture_and_only_a_canvas_without_one_shows_the_placeholder() {
    let started = std::time::Instant::now();
    let without = painted(false);
    let with = painted(true);
    println!("canvas without a picture painted {without:?}");
    println!("canvas with a picture painted {with:?} ({} ms)", started.elapsed().as_millis());
    assert!(without.iter().any(|t| t.contains("CV-1") && t.contains("Canvas2D")), "no picture: the placeholder, {without:?}");
    assert!(with.iter().any(|t| t == "<picture>"), "the picture is drawn, {with:?}");
    assert!(!with.iter().any(|t| t.contains("Canvas2D")), "the placeholder is gone, {with:?}");
}
