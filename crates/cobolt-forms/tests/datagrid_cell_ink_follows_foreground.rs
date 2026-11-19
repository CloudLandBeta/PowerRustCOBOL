// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

#![cfg(feature = "render")]
//! **A DataGrid's `ForegroundColor` is the colour of its cells' text** — the
//! reference's "text / foreground drawing color" (the Cnd_PRC report, 2026-11-18).
//!
//! Every column carries a text colour of its own, seeded black (a designed
//! column and every column a `Columns` line makes), and the painter took the
//! column's before the grid's: a grid's chosen `ForegroundColor` never reached a
//! cell, on the desktop or in the browser. A column still on the seeded black now
//! takes the grid's chosen colour; a column that names another colour keeps it;
//! a grid whose `ForegroundColor` is still the default draws exactly as before.
//! What is asserted is the colour of the painted text, read from the shapes the
//! running form paints — not a predicted value.

use cobolt_forms::containers::ActiveTabs;
use cobolt_forms::model::{
    DataGridAdvanced, DataGridCellFrame, DataGridColumn, DataGridValueStyleRule, PropValue,
    DATAGRID_ADVANCED_PROP,
};
use cobolt_forms::render::{render_form, Backdrop, DesignedState, RenderInput, RenderMode};
use cobolt_forms::{Control, ControlType};
use egui::{Color32, Pos2, Rect, Vec2};

/// A grid with columns `A` (text colour `a_fg`) and `B` (the seeded black), one row `alpha / beta`.
fn grid(grid_fg: &str, a_fg: &str) -> Control {
    grid_with(grid_fg, |a| a.foreground_color = a_fg.into())
}

/// The same grid, with column `A` (still on the seeded black) changed by `edit_a`.
fn grid_with(grid_fg: &str, edit_a: impl Fn(&mut DataGridColumn)) -> Control {
    let mut advanced = DataGridAdvanced::default();
    for id in ["A", "B"] {
        let mut column = DataGridColumn {
            id: id.into(),
            title: id.into(),
            source_name: id.into(),
            value_type: "string".into(),
            width: 100.0,
            foreground_color: "#000000".into(),
            ..DataGridColumn::default()
        };
        if id == "A" {
            edit_a(&mut column);
        }
        advanced.columns.push(column);
    }
    let mut grid = Control::new("DG", ControlType::DataGrid, 20, 20);
    grid.rect.w = 260;
    grid.rect.h = 160;
    for (key, value) in [
        ("Columns", "A:string\nB:string".to_owned()),
        ("Rows", "alpha\tbeta".to_owned()),
        ("ShowCSVExportButton", "false".to_owned()),
        ("RowHeight", "22".to_owned()),
        ("ForegroundColor", grid_fg.to_owned()),
        ("BackgroundColor", "#FFFFFF".to_owned()),
        (DATAGRID_ADVANCED_PROP, advanced.to_json().unwrap()),
    ] {
        grid.set_prop(key, PropValue::String(value));
    }
    grid
}

fn collect(out: &mut Vec<(String, Color32)>, shape: &egui::Shape) {
    match shape {
        egui::Shape::Vec(v) => v.iter().for_each(|s| collect(out, s)),
        egui::Shape::Text(t) => {
            let colour = t.galley.job.sections.first().map(|s| s.format.color).unwrap_or(t.fallback_color);
            out.push((t.galley.job.text.clone(), colour));
        }
        _ => {}
    }
}

/// The colour the running form paints `alpha` and `beta` in (the second of two frames).
fn inks(grid: &Control) -> (Color32, Color32) {
    let ctx = egui::Context::default();
    ctx.set_fonts(egui::FontDefinitions::default());
    let controls = vec![grid.clone()];
    let active = ActiveTabs::new();
    let mut shapes = Vec::new();
    for frame in 0..2 {
        let mut full = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(400.0, 300.0))),
                time: Some(frame as f64 * 0.05),
                ..Default::default()
            },
            |root_ui| {
                egui::CentralPanel::default().frame(egui::Frame::NONE).show_inside(root_ui, |ui| {
                    ui.set_min_size(Vec2::new(400.0, 300.0));
                    let input = RenderInput {
                        css_final: None,
                        controls: &controls,
                        state: &DesignedState,
                        form_size: Vec2::new(400.0, 300.0),
                        glass: true,
                        mode: RenderMode::Interactive,
                        active_tabs: &active,
                        backdrop: Backdrop::default(),
                    };
                    render_form(ui, &input);
                });
            },
        );
        full.textures_delta.clear();
        shapes = full.shapes;
    }
    let mut texts = Vec::new();
    for clipped in &shapes {
        collect(&mut texts, &clipped.shape);
    }
    let ink_of = |s: &str| texts.iter().find(|(t, _)| t == s).map(|(_, c)| *c).unwrap_or_else(|| panic!("`{s}` was not painted: {texts:?}"));
    (ink_of("alpha"), ink_of("beta"))
}

#[test]
fn a_cell_takes_the_grids_foreground_unless_its_column_names_a_colour() {
    let red = Color32::from_rgb(0xC0, 0x39, 0x2B);
    let green = Color32::from_rgb(0x1E, 0x84, 0x49);
    let rule = |a: &mut DataGridColumn| {
        a.value_style_rules.push(DataGridValueStyleRule {
            value: "alpha".into(),
            foreground_color: "#1E8449".into(),
            ..DataGridValueStyleRule::default()
        })
    };
    let frame = |a: &mut DataGridColumn| {
        a.frame = Some(DataGridCellFrame {
            enabled: true,
            foreground_color: "#1E8449".into(),
            ..DataGridCellFrame::default()
        })
    };
    let cases = [
        ("grid #C0392B, both columns on the seeded black", grid("#C0392B", "#000000"), (red, red)),
        ("grid #C0392B, column A names #1E8449", grid("#C0392B", "#1E8449"), (green, red)),
        ("grid #C0392B, column A set to black (#000000FF)", grid("#C0392B", "#000000FF"), (Color32::BLACK, red)),
        ("grid #C0392B, a value rule inks `alpha` #1E8449", grid_with("#C0392B", rule), (green, red)),
        ("grid #C0392B, column A framed in #1E8449 ink", grid_with("#C0392B", frame), (green, red)),
        ("grid #FFFFFFFF (a Spatial applier's stamp)", grid("#FFFFFFFF", "#000000"), (Color32::WHITE, Color32::WHITE)),
        ("grid on the default ForegroundColor", grid("#FFFFFF", "#000000"), (Color32::BLACK, Color32::BLACK)),
    ];
    for (what, g, want) in cases {
        let got = inks(&g);
        println!("  {what:<50} -> alpha {:?}, beta {:?}", got.0, got.1);
        assert_eq!(got, want, "{what}");
    }
}
