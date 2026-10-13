// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

#![cfg(feature = "render")]
//! **Spec 095 AC9 — one DataGrid cell, three surfaces.**
//!
//! A grid with a Checkbox column, and one with a Dropdown column, are painted through the three *interactive*
//! entry points the product has: `render_form` (the running form),
//! `render_form_with_chrome` (the preview) and `render_form_scoped` (the
//! Designer's canvas, which hosts every grid through the interactive renderer —
//! its static face for a DataGrid is only a label). The shapes painted must be
//! the same on all three; a cell drawn differently on the canvas than at run
//! time is exactly the drift the one-engine rule (spec 017) exists to stop.
//!
//! Shapes are compared the way the example-corpus golden records them: a text
//! by size, family, colour and a hash of its string (never its width, which
//! depends on the machine's fonts); anything else by a hash of the whole shape.

use cobolt_forms::containers::ActiveTabs;
use cobolt_forms::model::{DataGridAdvanced, DataGridColumn, DataGridDropdown, PropValue, DATAGRID_ADVANCED_PROP};
use cobolt_forms::render::{
    render_form, render_form_scoped, render_form_with_chrome, Backdrop, DesignedState, RenderInput, RenderMode,
};
use cobolt_forms::{Control, ControlType};
use egui::{Color32, Pos2, Rect, Vec2};

#[derive(Clone, Copy, Debug)]
enum Surface {
    Run,
    Preview,
    DesignerCanvas,
}
const SURFACES: [Surface; 3] = [Surface::Run, Surface::Preview, Surface::DesignerCanvas];

fn fnv64(s: &str) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in s.bytes() {
        h ^= b as u64;
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    h
}

fn c8(c: Color32) -> String {
    format!("#{:02x}{:02x}{:02x}{:02x}", c.r(), c.g(), c.b(), c.a())
}

fn dump(out: &mut Vec<String>, shape: &egui::Shape) {
    use egui::Shape as S;
    match shape {
        S::Vec(v) => v.iter().for_each(|s| dump(out, s)),
        S::Noop => {}
        S::Text(t) => {
            let job = &t.galley.job;
            let secs: Vec<String> = job
                .sections
                .iter()
                .map(|s| format!("{}/{:?}/{}", s.format.font_id.size, s.format.font_id.family, c8(s.format.color)))
                .collect();
            out.push(format!("TEXT {} str={:016x} fb={}", secs.join(","), fnv64(&job.text), c8(t.fallback_color)));
        }
        S::Mesh(_) => out.push(format!("MESH h={:016x}", fnv64(&format!("{shape:?}")))),
        S::Callback(cb) => out.push(format!("CALLBACK {:?}", cb.rect)),
        other => out.push(format!("SHAPE h={:016x}", fnv64(&format!("{other:?}")))),
    }
}

/// A grid with a Checkbox column (`Done`) and a plain one (`Name`), one cell
/// checked, one not, one empty.
fn grid(done: &[&str]) -> Control {
    let mut advanced = DataGridAdvanced::default();
    for (id, kind) in [("Name", ""), ("Done", "Checkbox")] {
        advanced.columns.push(DataGridColumn {
            id: id.into(),
            title: id.into(),
            source_name: id.into(),
            value_type: "string".into(),
            width: 100.0,
            control_kind: kind.into(),
            ..DataGridColumn::default()
        });
    }
    let rows: Vec<String> = done.iter().enumerate().map(|(i, d)| format!("Row {}\t{d}", i + 1)).collect();
    let mut grid = Control::new("DG", ControlType::DataGrid, 20, 20);
    grid.rect.w = 260;
    grid.rect.h = 160;
    for (key, value) in [
        ("Columns", "Name:string\nDone:string".to_owned()),
        ("Rows", rows.join("\n")),
        ("ShowCSVExportButton", "false".to_owned()),
        ("RowHeight", "22".to_owned()),
        ("AllowCellEditing", "true".to_owned()),
        (DATAGRID_ADVANCED_PROP, advanced.to_json().unwrap()),
    ] {
        grid.set_prop(key, PropValue::String(value));
    }
    grid
}

/// A grid with a plain column (`Name`) and a Dropdown column (`Genre`) over the static
/// items `D=Drama`, `C=Crime`, `T=Thriller`; one row per value in `genres`.
fn dropdown_grid(genres: &[&str]) -> Control {
    let mut advanced = DataGridAdvanced::default();
    for (id, kind) in [("Name", ""), ("Genre", "Dropdown")] {
        advanced.columns.push(DataGridColumn {
            id: id.into(),
            title: id.into(),
            source_name: id.into(),
            value_type: "string".into(),
            width: 100.0,
            control_kind: kind.into(),
            dropdown: (!kind.is_empty())
                .then(|| DataGridDropdown { items: "D=Drama\nC=Crime\nT=Thriller".into(), ..Default::default() }),
            ..DataGridColumn::default()
        });
    }
    let rows: Vec<String> = genres.iter().enumerate().map(|(i, g)| format!("Row {}\t{g}", i + 1)).collect();
    let mut grid = Control::new("DG", ControlType::DataGrid, 20, 20);
    grid.rect.w = 260;
    grid.rect.h = 160;
    for (key, value) in [
        ("Columns", "Name:string\nGenre:string".to_owned()),
        ("Rows", rows.join("\n")),
        ("ShowCSVExportButton", "false".to_owned()),
        ("RowHeight", "22".to_owned()),
        ("AllowCellEditing", "true".to_owned()),
        (DATAGRID_ADVANCED_PROP, advanced.to_json().unwrap()),
    ] {
        grid.set_prop(key, PropValue::String(value));
    }
    grid
}

/// Every shape the second of two frames painted, one line each.
fn painted(surface: Surface, grid: &Control) -> Vec<String> {
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
                        controls: &controls,
                        state: &DesignedState,
                        form_size: Vec2::new(400.0, 300.0),
                        glass: true,
                        mode: RenderMode::Interactive,
                        active_tabs: &active,
                        backdrop: Backdrop::default(),
                    };
                    match surface {
                        Surface::Run => {
                            render_form(ui, &input);
                        }
                        Surface::Preview => {
                            render_form_with_chrome(ui, &input, None);
                        }
                        Surface::DesignerCanvas => {
                            render_form_scoped(ui, &input, None, egui::Id::new("designer-grid-edit"));
                        }
                    }
                });
            },
        );
        full.textures_delta.clear();
        shapes = full.shapes;
    }
    let mut lines = Vec::new();
    for clipped in &shapes {
        dump(&mut lines, &clipped.shape);
    }
    lines
}

/// The same cells, the same shapes, on the running form, the preview and the
/// Designer's canvas — and the painted box follows the value.
#[test]
fn a_checkbox_cell_is_painted_the_same_on_run_preview_and_the_designer_canvas() {
    let checked = grid(&["Y", "N", ""]);
    let started = std::time::Instant::now();
    let reference = painted(Surface::Run, &checked);
    assert!(reference.len() > 20, "the grid painted something: {} shapes", reference.len());
    for surface in SURFACES {
        assert_eq!(painted(surface, &checked), reference, "{surface:?} differs from the running form");
    }
    // The tick is part of the picture: flip every value and the picture changes.
    let flipped = painted(Surface::Run, &grid(&["N", "Y", "Y"]));
    assert_ne!(flipped, reference, "a checked cell is painted differently from an unchecked one");
    // …and a grid whose values are the same words in the same state paints alike.
    assert_eq!(painted(Surface::Run, &grid(&["yes", "no", "false"])), painted(Surface::Run, &grid(&["Y", "N", "N"])), "yes/no/false draw as Y/N/N");
    println!(
        "\n  DATAGRID CHECKBOX CELLS -- {} shapes per surface, identical on run / preview / designer canvas ({} ms)\n",
        reference.len(),
        started.elapsed().as_millis()
    );
}

/// The Dropdown cell: the same shapes on all three surfaces, the item's TEXT painted
/// where the cell holds its value, and the picture follows the value.
#[test]
fn a_dropdown_cell_is_painted_the_same_on_run_preview_and_the_designer_canvas() {
    let grid = dropdown_grid(&["D", "C", ""]);
    let started = std::time::Instant::now();
    let reference = painted(Surface::Run, &grid);
    assert!(reference.len() > 20, "the grid painted something: {} shapes", reference.len());
    for surface in SURFACES {
        assert_eq!(painted(surface, &grid), reference, "{surface:?} differs from the running form");
    }
    // The cell shows the item's text — `Drama`, not the `D` it holds — on every surface.
    let text_is_painted = |lines: &[String], text: &str| lines.iter().any(|l| l.contains(&format!("str={:016x}", fnv64(text))));
    for surface in SURFACES {
        let lines = painted(surface, &grid);
        assert!(text_is_painted(&lines, "Drama") && text_is_painted(&lines, "Crime"), "{surface:?}: the items' texts are painted");
        assert!(!text_is_painted(&lines, "D") && !text_is_painted(&lines, "C"), "{surface:?}: the values are not");
    }
    // The picture follows the value, and a value no item has is shown as it is.
    assert_ne!(painted(Surface::Run, &dropdown_grid(&["C", "D", ""])), reference, "another value, another picture");
    assert!(text_is_painted(&painted(Surface::Run, &dropdown_grid(&["Z"])), "Z"), "a value no item has shows as it is");
    println!(
        "\n  DATAGRID DROPDOWN CELLS -- {} shapes per surface, identical on run / preview / designer canvas; the item's text is painted ({} ms)\n",
        reference.len(),
        started.elapsed().as_millis()
    );
}

// ── The grid is drawn in the order of the layers and z-orders ───────────────────────────

/// Every shape painted by `controls` on `surface`, with the form's `layers`, one line each.
fn painted_layers(surface: Surface, controls: &[Control], layers: Vec<cobolt_forms::model::Layer>) -> Vec<String> {
    let ctx = egui::Context::default();
    ctx.set_fonts(egui::FontDefinitions::default());
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
                        controls,
                        state: &DesignedState,
                        form_size: Vec2::new(400.0, 300.0),
                        glass: true,
                        mode: RenderMode::Interactive,
                        active_tabs: &active,
                        backdrop: Backdrop { layers: layers.clone(), ..Backdrop::default() },
                    };
                    match surface {
                        Surface::Run => {
                            render_form(ui, &input);
                        }
                        Surface::Preview => {
                            render_form_with_chrome(ui, &input, None);
                        }
                        Surface::DesignerCanvas => {
                            render_form_scoped(ui, &input, None, egui::Id::new("designer-grid-edit"));
                        }
                    }
                });
            },
        );
        full.textures_delta.clear();
        shapes = full.shapes;
    }
    let mut lines = Vec::new();
    for clipped in &shapes {
        dump(&mut lines, &clipped.shape);
    }
    lines
}

/// Where a text is among the shapes painted.
fn text_at(lines: &[String], text: &str) -> usize {
    let hash = format!("str={:016x}", fnv64(text));
    lines.iter().position(|l| l.contains(&hash)).unwrap_or_else(|| panic!("{text:?} was not painted"))
}

/// A label over the grid's first rows, in `layer` at `z`.
fn label_over(caption: &str, layer: Option<&str>, z: i32) -> Control {
    let mut label = Control::new(caption, ControlType::Label, 40, 40);
    label.set_prop("Caption", PropValue::String(caption.into()));
    label.layer = layer.map(str::to_owned);
    label.z_order = z;
    label
}

/// A grid is drawn where its z-order and its layer put it, as every other control is: on the
/// running form and the preview, a control above it is over it, one below it is under it, a
/// layer's controls are over the base's and a grid in a layer is over the base's controls.
#[test]
fn a_grid_is_drawn_in_z_order_and_layer_order() {
    let mut plain = dropdown_grid(&["D", "C"]);
    plain.z_order = 50;
    let l1 = || vec![cobolt_forms::model::Layer::new("L1")];
    for surface in [Surface::Run, Surface::Preview] {
        // z-order, on the base.
        let controls = vec![plain.clone(), label_over("Above", None, 90), label_over("Below", None, 10)];
        let lines = painted_layers(surface, &controls, Vec::new());
        let grid = text_at(&lines, "Drama");
        assert!(text_at(&lines, "Above") > grid, "{surface:?}: above the grid in z-order, over it");
        assert!(text_at(&lines, "Below") < grid, "{surface:?}: below the grid in z-order, under it");
        // A layer's control is over a base grid even with the lower z-order.
        let mut base_grid = plain.clone();
        base_grid.z_order = 80;
        let controls = vec![base_grid, label_over("Above", Some("L1"), 5)];
        let lines = painted_layers(surface, &controls, l1());
        assert!(text_at(&lines, "Above") > text_at(&lines, "Drama"), "{surface:?}: a layer's control is over a base grid");
        // A grid in a layer is over the base's controls, even with the higher z-order.
        let mut layer_grid = plain.clone();
        layer_grid.layer = Some("L1".into());
        layer_grid.z_order = 5;
        let controls = vec![layer_grid, label_over("Above", None, 80)];
        let lines = painted_layers(surface, &controls, l1());
        assert!(text_at(&lines, "Above") < text_at(&lines, "Drama"), "{surface:?}: a grid in a layer is over the base's controls");
    }
}
