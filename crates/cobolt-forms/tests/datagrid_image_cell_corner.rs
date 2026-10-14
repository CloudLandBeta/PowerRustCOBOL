// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! A DataGrid image cell's corner radius is clamped to the box the image is drawn in.
//!
//! The clamp was `radius.clamp(0.0, dest.width().min(dest.height()) * 0.5)`, and `f32::clamp`
//! **panics when its maximum is below its minimum**: an image box with a negative size would crash the
//! IDE exactly as a chart with a tiny border did (the 1.91.142 fix: `min = 0.0, max = -1.0`).
//!
//! The box is the cell pulled in by 3 points on each side, and the grid keeps a cell larger than that:
//! a row is never shorter than 14 points (`RowHeight` is clamped to 14–120, a per-row override to
//! 14–400) and a column never narrower than `MIN_COLUMN_WIDTH`, 32 points. So no grid reaches the
//! panic today; this proves it through the real renderer, with the image actually drawn, and the unit
//! test in `render.rs` holds the guard itself for the day one of those floors moves.

#![cfg(feature = "render")]

use cobolt_forms::containers::ActiveTabs;
use cobolt_forms::model::{
    Control, ControlType, DataGridAdvanced, DataGridColumn, PropValue, DATAGRID_ADVANCED_PROP,
};
use cobolt_forms::render::{render_form, Backdrop, DesignedState, RenderInput, RenderMode};
use egui::epaint::Primitive;
use egui::{Pos2, Rect, TextureId};

/// A 24 × 16 PNG on disk, the thumbnail a cell shows.
fn thumbnail() -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("prc-grid-image-cell-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("thumb.png");
    image::RgbaImage::from_pixel(24, 16, image::Rgba([200, 60, 60, 255])).save(&path).unwrap();
    path
}

fn grid(row_height: &str, corner: f32, picture: &str) -> Control {
    let mut g = Control::new("GRID", ControlType::DataGrid, 10, 10);
    g.rect = cobolt_forms::Rect::new(10, 10, 360, 200);
    g.set_prop("Columns", PropValue::String("Photo:string\nName:string".into()));
    g.set_prop("Rows", PropValue::String(format!("{picture}\tAna\n{picture}\tBia\n{picture}\tCid")));
    g.set_prop("RowHeight", PropValue::String(row_height.into()));
    let mut advanced = DataGridAdvanced::default();
    for (id, kind) in [("Photo", "Image"), ("Name", "")] {
        advanced.columns.push(DataGridColumn {
            id: id.into(),
            title: id.into(),
            source_name: id.into(),
            value_type: "string".into(),
            width: 120.0,
            control_kind: kind.into(),
            image_corner_radius: if kind == "Image" { corner } else { 0.0 },
            image_shadow: kind == "Image",
            ..DataGridColumn::default()
        });
    }
    g.set_prop(DATAGRID_ADVANCED_PROP, PropValue::String(advanced.to_json().unwrap()));
    g
}

/// Draw one interactive frame pair and say how many meshes carry a texture of their own (the thumbnails).
fn thumbnails_drawn(controls: &[Control]) -> usize {
    let ctx = egui::Context::default();
    let active = ActiveTabs::new();
    let mut drawn = 0;
    for _ in 0..3 {
        let mut full = ctx.run_ui(
            egui::RawInput { screen_rect: Some(Rect::from_min_size(Pos2::ZERO, egui::vec2(480.0, 320.0))), ..Default::default() },
            |root| {
                egui::CentralPanel::default().frame(egui::Frame::NONE).show(root, |ui| {
                    let input = RenderInput {
                        controls,
                        state: &DesignedState,
                        form_size: egui::vec2(460.0, 300.0),
                        glass: true,
                        mode: RenderMode::Interactive,
                        active_tabs: &active,
                        backdrop: Backdrop::default(),
                    };
                    render_form(ui, &input);
                });
            },
        );
        let ppp = full.pixels_per_point;
        full.textures_delta.clear();
        drawn = ctx
            .tessellate(std::mem::take(&mut full.shapes), ppp)
            .iter()
            .filter(|p| matches!(&p.primitive, Primitive::Mesh(m) if m.texture_id != TextureId::default()))
            .count();
    }
    drawn
}

#[test]
fn an_image_cell_is_drawn_at_every_row_height_a_grid_can_be_given() {
    let path = thumbnail();
    let picture = path.to_string_lossy().into_owned();
    println!("\n  RowHeight written   thumbnails drawn (rounded corner 6, shadow on)");
    for written in ["1", "2", "5", "6", "13", "14", "22", "120", "500", "-3", "abc"] {
        let drawn = thumbnails_drawn(&[grid(written, 6.0, &picture)]);
        println!("  {written:>17}   {drawn}");
        // The image is really drawn — the clamp under test ran with a real box — and the frame did not panic.
        assert!(drawn >= 1, "RowHeight {written}: no thumbnail was drawn, so the corner clamp was never reached");
    }
    // A radius larger than any box, and none at all, through the same path.
    for corner in [0.0f32, 1.0, 40.0, 1000.0] {
        assert!(thumbnails_drawn(&[grid("22", corner, &picture)]) >= 1, "corner {corner}");
    }
    let _ = std::fs::remove_dir_all(path.parent().unwrap());
}
