// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Spec 094 (AC1, R1–R6) — the Form Designer shows a DataGrid's own fake data and
//! edits it where it is drawn, driven through real frames of the real
//! `DesignerPanel::show` with pointer and key events.
//!
//! A child module of `designer`, so it reads the designer's private state the way
//! the other test modules there do.

use super::*;
use crate::llm::LlmConfig;
use crate::panels::grid_edit;
use cobolt_forms::model::{DataGridAdvanced, DATAGRID_ADVANCED_PROP};
use cobolt_forms::{
    BindingDataType, BindingField, BindingSourceDescriptor, BindingTargetDescriptor, DataBindingDef,
};
use egui::{Event, Key, PointerButton};

const SCREEN: Vec2 = Vec2::new(1400.0, 1000.0);

fn press(at: Pos2, pressed: bool) -> Event {
    Event::PointerButton { pos: at, button: PointerButton::Primary, pressed, modifiers: egui::Modifiers::NONE }
}

fn key(k: Key, mods: egui::Modifiers) -> Event {
    Event::Key { key: k, physical_key: None, pressed: true, repeat: false, modifiers: mods }
}

/// A designer in a window of its own, one real frame at a time, on a clock.
struct Rig {
    ctx: egui::Context,
    llm: LlmConfig,
    d: DesignerPanel,
    t: f64,
    /// Every text the last frame painted, in paint order: the text, where, and
    /// the clip rect it was painted under.
    painted: Vec<(String, Rect, Rect)>,
    /// The colour and opacity each of those texts was painted with, same order.
    ink: Vec<(Color32, f32)>,
    /// Every filled rectangle the last frame painted, in paint order.
    fills: Vec<(Rect, Color32)>,
    /// The cursor the last frame asked for.
    cursor: egui::CursorIcon,
}

impl Rig {
    fn new(form: Form) -> Self {
        let mut rig = Self {
            ctx: egui::Context::default(),
            llm: LlmConfig::load_defaults_for_test(),
            d: DesignerPanel::new(form),
            t: 0.0,
            painted: Vec::new(),
            ink: Vec::new(),
            fills: Vec::new(),
            cursor: egui::CursorIcon::Default,
        };
        rig.settle();
        rig
    }

    fn frame(&mut self, events: Vec<Event>) {
        let mut input = egui::RawInput::default();
        input.screen_rect = Some(egui::Rect::from_min_size(Pos2::ZERO, SCREEN));
        input.max_texture_side = Some(8192);
        input.focused = true;
        self.t += 0.05;
        input.time = Some(self.t);
        input.events = events;
        let d = &mut self.d;
        let llm = &self.llm;
        let mut out = self.ctx.run_ui(input, |root| {
            egui::CentralPanel::default().show_inside(root, |ui| {
                let _ = d.show(ui, &mut None, &[], llm, None, None);
            });
        });
        out.textures_delta.clear();
        fn walk(
            shape: &egui::Shape,
            clip: Rect,
            texts: &mut Vec<(String, Rect, Rect)>,
            ink: &mut Vec<(Color32, f32)>,
            fills: &mut Vec<(Rect, Color32)>,
        ) {
            match shape {
                egui::Shape::Text(t) => {
                    texts.push((
                        t.galley.text().to_owned(),
                        t.galley.rect.translate(t.pos.to_vec2()),
                        clip,
                    ));
                    let color = t
                        .galley
                        .job
                        .sections
                        .first()
                        .map(|s| s.format.color)
                        .unwrap_or(t.fallback_color);
                    ink.push((color, t.opacity_factor));
                }
                egui::Shape::Rect(r) => fills.push((r.rect, r.fill)),
                egui::Shape::Vec(v) => v.iter().for_each(|s| walk(s, clip, texts, ink, fills)),
                _ => {}
            }
        }
        self.painted.clear();
        self.ink.clear();
        self.fills.clear();
        for clipped in &out.shapes {
            walk(&clipped.shape, clipped.clip_rect, &mut self.painted, &mut self.ink, &mut self.fills);
        }
        self.cursor = out.platform_output.cursor_icon;
    }

    /// A few quiet frames, so layout and the canvas origin are known.
    fn settle(&mut self) {
        for _ in 0..3 {
            self.frame(vec![]);
        }
    }

    /// Where a point of the form is on screen.
    fn at(&self, x: f32, y: f32) -> Pos2 {
        self.d.canvas_origin.expect("the canvas has been drawn") + Vec2::new(x, y)
    }

    fn click(&mut self, p: Pos2) {
        // Apart from the last click, so two in a row are not a double-click.
        self.t += 0.8;
        self.frame(vec![Event::PointerMoved(p)]);
        self.frame(vec![press(p, true)]);
        self.frame(vec![press(p, false)]);
    }

    fn double_click(&mut self, p: Pos2) {
        // Long enough after the last click that this one starts a new count.
        self.t += 0.8;
        self.frame(vec![Event::PointerMoved(p)]);
        self.frame(vec![press(p, true)]);
        self.frame(vec![press(p, false)]);
        self.frame(vec![press(p, true)]);
        self.frame(vec![press(p, false)]);
        self.settle();
    }

    fn drag(&mut self, from: Pos2, to: Pos2) {
        self.frame(vec![Event::PointerMoved(from)]);
        self.frame(vec![press(from, true)]);
        for i in 1..=6 {
            let p = from + (to - from) * (i as f32 / 6.0);
            self.frame(vec![Event::PointerMoved(p)]);
        }
        self.frame(vec![press(to, false)]);
        self.settle();
    }

    fn type_replacing(&mut self, text: &str) {
        self.frame(vec![key(Key::A, egui::Modifiers::COMMAND)]);
        self.frame(vec![Event::Text(text.to_owned())]);
    }

    fn press_key(&mut self, k: Key) {
        self.frame(vec![key(k, egui::Modifiers::NONE)]);
        self.settle();
    }

    fn prop(&self, id: &str, key: &str) -> String {
        self.d
            .form
            .find_control(id)
            .and_then(|c| c.get_prop(key))
            .map(|v| v.to_xml_string())
            .unwrap_or_default()
    }

    /// Where the text `s` was painted last frame.
    fn text_rect(&self, s: &str) -> Option<Rect> {
        self.painted.iter().find(|(t, _, _)| t == s).map(|(_, r, _)| *r)
    }
}

/// A plain two-column grid at (40, 40), 300 × 200: no caption row, titles on
/// 40..62, rows from 62 down, 22 px each.
fn grid_form() -> Form {
    let mut form = Form::new("F", "F", 800, 600);
    let mut g = Control::new("DG", ControlType::DataGrid, 40, 40);
    g.rect.w = 300;
    g.rect.h = 200;
    for (k, v) in [
        ("Columns", "Name:string\nAge:number"),
        ("Rows", "Bob\t30\nann\t9\nCid\t100"),
        ("ShowCSVExportButton", "false"),
        ("RowHeight", "22"),
    ] {
        g.set_prop(k, PropValue::String(v.into()));
    }
    form.controls.push(g);
    form
}

/// Where the first data cell (Bob) is, in form coordinates.
const BOB: (f32, f32) = (40.0 + 50.0, 40.0 + 22.0 + 11.0);

#[test]
fn spike_the_interactive_grid_is_hosted_on_the_canvas() {
    let mut rig = Rig::new(grid_form());
    // A double-click on the grid enters edit mode (and selects it).
    rig.double_click(rig.at(BOB.0, BOB.1));
    assert_eq!(rig.d.grid_edit.as_ref().map(|s| s.id.as_str()), Some("DG"), "double-click enters");
    assert_eq!(rig.d.selected_ids, ["DG"]);
    // Drawn once, by the interactive renderer: the cell texts are on screen.
    assert!(rig.text_rect("Bob").is_some(), "the grid is drawn: {:?}", rig.painted);

    // Double-click a cell, replace its text, Enter.
    rig.double_click(rig.at(BOB.0, BOB.1));
    rig.type_replacing("Zed");
    rig.press_key(Key::Enter);
    assert_eq!(rig.prop("DG", "Rows"), "Zed\t30\nann\t9\nCid\t100", "the cell edit landed in Rows");
    rig.d.undo();
    assert_eq!(rig.prop("DG", "Rows"), "Bob\t30\nann\t9\nCid\t100", "and one undo restores it");
}

// ── Bound grids: the binding seeds once and never rewrites (R1, R2, R5) ──────

/// A CobolTable binding of the grid `grid_id` over two fields, with display names.
fn actors_binding(grid_id: &str) -> DataBindingDef {
    let mut id = BindingField::new("ACTOR-ID", BindingDataType::Integer);
    id.display_name = "Actor Id".into();
    let mut caption = BindingField::new("ACTOR-CAPTION", BindingDataType::Text);
    caption.display_name = "Actor Caption".into();
    DataBindingDef::new(
        "BIND-ACTORS",
        "Actors",
        BindingSourceDescriptor::CobolTable {
            table_name: "WS-ACTOR-TABLE".into(),
            occurs_item: "WS-ACTOR-ROW".into(),
            fields: vec![id, caption],
            key_fields: vec!["ACTOR-ID".into()],
            writable: true,
        },
        BindingTargetDescriptor::DataGrid { control_id: grid_id.into() },
    )
}

/// A form holding a bare DataGrid `BG` at (40, 40) — no columns, no rows — and
/// the binding above on it.
fn bound_form() -> Form {
    let mut form = Form::new("F", "F", 800, 600);
    let mut g = Control::new("BG", ControlType::DataGrid, 40, 40);
    g.rect.w = 320;
    g.rect.h = 200;
    g.set_prop("ShowCSVExportButton", PropValue::String("false".into()));
    form.controls.push(g);
    form.data_bindings.push(actors_binding("BG"));
    form
}

#[test]
fn a_bound_grid_draws_its_own_rows_and_a_frame_later_has_not_changed_them() {
    let mut rig = Rig::new(bound_form());
    assert_eq!(rig.d.seed_bound_grids_without_columns(), 1, "the bare bound grid is seeded once");
    let seeded = rig.prop("BG", "Rows");
    assert!(seeded.starts_with("1\tActor Caption 1"), "the binding's fake rows: {seeded:?}");

    // The developer's own fake data, typed over what the binding seeded.
    rig.d.set_property("BG", "Rows", PropValue::String("7\tOwn row one\n8\tOwn row two".into()));
    let undo_depth = rig.d.undo_stack.len();
    for _ in 0..20 {
        rig.frame(vec![]);
    }
    assert_eq!(rig.prop("BG", "Rows"), "7\tOwn row one\n8\tOwn row two", "20 frames later: unchanged");
    assert_eq!(rig.d.undo_stack.len(), undo_depth, "no frame wrote anything");
    // …and the canvas draws THOSE rows, not the binding's.
    assert!(rig.text_rect("Own row one").is_some(), "drawn: {:?}", rig.painted.iter().map(|p| &p.0).collect::<Vec<_>>());
    assert!(rig.text_rect("Actor Caption 1").is_none(), "the binding's sample is not drawn");
    println!("094 AC1: a bound grid keeps its own 2 rows over 20 frames; {} undo steps added by frames", rig.d.undo_stack.len() - undo_depth);
}

#[test]
fn applying_a_binding_seeds_columns_advanced_grid_and_rows_once_and_one_undo_restores_the_grid() {
    let mut form = bound_form();
    form.data_bindings.clear();
    let mut rig = Rig::new(form);
    let before = cobolt_forms::form_to_string(&rig.d.form).unwrap();
    let steps = rig.d.undo_stack.len();
    rig.d.apply_data_binding(actors_binding("BG"));
    assert_eq!(rig.d.undo_stack.len(), steps + 1, "applying is one undo step");
    assert_eq!(rig.prop("BG", "Columns"), "Actor Id:number\nActor Caption:string");
    assert!(rig.prop("BG", "Rows").starts_with("1\tActor Caption 1\n2\tActor Caption 2"), "{}", rig.prop("BG", "Rows"));
    let advanced = DataGridAdvanced::from_control(rig.d.form.find_control("BG").unwrap());
    assert_eq!(advanced.columns.iter().map(|c| c.source_name.as_str()).collect::<Vec<_>>(), ["ACTOR-ID", "ACTOR-CAPTION"]);

    // Frames neither re-seed nor change a thing.
    let seeded = cobolt_forms::form_to_string(&rig.d.form).unwrap();
    for _ in 0..10 {
        rig.frame(vec![]);
    }
    assert_eq!(cobolt_forms::form_to_string(&rig.d.form).unwrap(), seeded);

    // One undo gives the grid back as it was.
    rig.d.undo();
    assert_eq!(cobolt_forms::form_to_string(&rig.d.form).unwrap(), before, "bytes of the saved form are as before");
    assert_eq!(rig.prop("BG", "Columns"), "");
    assert_eq!(rig.prop("BG", "Rows"), "");
    println!("094 AC1: apply = 1 undo step; seeded {} columns and {} rows; undo restores {} bytes of form", advanced.columns.len(), 2, before.len());
}

#[test]
fn a_bound_grid_that_opens_without_columns_is_seeded_once_and_one_with_columns_is_left_alone() {
    let mut rig = Rig::new(bound_form());
    assert_eq!(rig.d.seed_bound_grids_without_columns(), 1);
    assert_eq!(rig.d.undo_stack.len(), 1, "one undoable step");
    assert_eq!(rig.d.seed_bound_grids_without_columns(), 0, "…and never again: the grid has columns now");
    rig.d.undo();
    assert_eq!(rig.prop("BG", "Columns"), "", "undo takes the open-time seed away");

    // A grid the developer already shaped is theirs.
    let mut form = bound_form();
    form.find_control_mut("BG").unwrap().set_prop("Columns", PropValue::String("Mine:string".into()));
    form.find_control_mut("BG").unwrap().set_prop("Rows", PropValue::String("kept".into()));
    let mut rig = Rig::new(form);
    assert_eq!(rig.d.seed_bound_grids_without_columns(), 0);
    assert_eq!(rig.prop("BG", "Rows"), "kept");
    assert!(rig.d.undo_stack.is_empty());
}

#[test]
fn reset_sample_data_reseeds_a_bound_grid_in_one_undoable_step() {
    let mut rig = Rig::new(bound_form());
    rig.d.seed_bound_grids_without_columns();
    let seeded = rig.prop("BG", "Rows");
    rig.d.set_property("BG", "Rows", PropValue::String("9\tEdited".into()));
    let depth = rig.d.undo_stack.len();
    assert!(rig.d.reset_grid_sample_data("BG"));
    assert_eq!(rig.prop("BG", "Rows"), seeded, "the binding's fake data is back");
    assert_eq!(rig.d.undo_stack.len(), depth + 1);
    rig.d.undo();
    assert_eq!(rig.prop("BG", "Rows"), "9\tEdited", "one undo gives the developer's rows back");
    assert!(!rig.d.reset_grid_sample_data("NOT-BOUND"), "a grid with no binding has nothing to reset to");
}

#[test]
fn a_renamed_title_survives_applying_the_binding_again() {
    let mut rig = Rig::new(bound_form());
    rig.d.seed_bound_grids_without_columns();
    // The developer renames the second column on the canvas.
    rig.d.enter_grid_edit("BG");
    rig.settle();
    let cells = grid_edit::header_cells_live(
        &rig.ctx,
        rig.d.form.find_control("BG").unwrap(),
        rig.d.grid_edit.as_ref().unwrap().screen.unwrap(),
    );
    let second = cells[1].rect.center();
    rig.double_click(second);
    assert!(rig.d.grid_edit.as_ref().unwrap().rename.is_some(), "double-click on a title opens the rename box");
    rig.type_replacing("Who");
    rig.press_key(Key::Enter);
    let titles = |rig: &Rig| -> Vec<String> {
        DataGridAdvanced::from_control(rig.d.form.find_control("BG").unwrap())
            .columns
            .iter()
            .map(|c| c.title.clone())
            .collect()
    };
    assert_eq!(titles(&rig), ["Actor Id", "Who"]);

    // Applying the binding again — the Reset sample data action does exactly this.
    rig.d.apply_data_binding(actors_binding("BG"));
    assert_eq!(titles(&rig), ["Actor Id", "Who"], "the title the developer typed is kept");
    // …and it still finds its data: the Columns line follows the title.
    let shown = grid_edit::shown_columns(rig.d.form.find_control("BG").unwrap());
    assert_eq!(shown.iter().map(|c| c.source).collect::<Vec<_>>(), [Some(0), Some(1)]);
    assert_eq!(rig.prop("BG", "Columns"), "Actor Id:number\nWho:string");
    println!("094 AC1: titles after re-applying the binding: {:?}", titles(&rig));
}

// ── Edit mode on the canvas: edits land in the properties and undo (R3–R6) ───

fn enter(rig: &mut Rig, id: &str) {
    assert!(rig.d.enter_grid_edit(id));
    rig.settle();
}

#[test]
fn a_column_resized_on_the_canvas_lands_in_advanced_grid_and_undoes_in_one_step() {
    let mut rig = Rig::new(grid_form());
    enter(&mut rig, "DG");
    let width = |rig: &Rig| DataGridAdvanced::from_control(rig.d.form.find_control("DG").unwrap()).columns[0].width;
    let before = width(&rig);
    let depth = rig.d.undo_stack.len();
    // The first column's right edge is 120 in; grab it on the title row and pull 30 right.
    rig.drag(rig.at(40.0 + 120.0, 51.0), rig.at(40.0 + 150.0, 51.0));
    let after = width(&rig);
    assert!(
        (after - before - 30.0).abs() <= 6.0,
        "the column grew by about the drag: {before} -> {after}"
    );
    assert_eq!(rig.d.undo_stack.len(), depth + 1, "a whole drag is one undo step, not one per frame");
    assert!(rig.prop("DG", DATAGRID_ADVANCED_PROP).contains("\"width\""), "it is in AdvancedGrid");
    rig.d.undo();
    assert_eq!(width(&rig), before, "one undo puts the width back");
    println!("094 AC1: resize on the canvas {before} -> {after} (dragged 30), 1 undo step");
}

#[test]
fn a_cell_edited_on_the_canvas_lands_in_rows_and_undoes() {
    let mut rig = Rig::new(grid_form());
    enter(&mut rig, "DG");
    rig.double_click(rig.at(BOB.0, BOB.1));
    rig.type_replacing("Zed");
    rig.press_key(Key::Enter);
    assert_eq!(rig.prop("DG", "Rows"), "Zed\t30\nann\t9\nCid\t100");
    // Only the design property was written: nothing the grid reports at run time.
    for run_time in ["EditedRow", "EditedColumn", "EditedValue", "PreviousValue", "ClickedRow"] {
        assert!(rig.d.form.find_control("DG").unwrap().get_prop(run_time).is_none(), "{run_time} is not saved");
    }
    assert!(rig.d.dirty);
    rig.d.undo();
    assert_eq!(rig.prop("DG", "Rows"), "Bob\t30\nann\t9\nCid\t100");
    rig.d.redo();
    assert_eq!(rig.prop("DG", "Rows"), "Zed\t30\nann\t9\nCid\t100", "and redo replays it");
}

#[test]
fn escape_cancels_a_cell_edit_and_only_the_next_escape_leaves_the_mode() {
    let mut rig = Rig::new(grid_form());
    enter(&mut rig, "DG");
    rig.double_click(rig.at(BOB.0, BOB.1));
    rig.type_replacing("Zed");
    rig.press_key(Key::Escape);
    assert_eq!(rig.prop("DG", "Rows"), "Bob\t30\nann\t9\nCid\t100", "Escape drops the edit");
    assert!(rig.d.grid_edit.is_some(), "…and stays in the mode: the Escape was the cell editor's");
    rig.press_key(Key::Escape);
    assert!(rig.d.grid_edit.is_none(), "the next Escape leaves");
}

#[test]
fn a_title_renamed_on_the_canvas_lands_in_the_properties_and_undoes() {
    let mut rig = Rig::new(grid_form());
    enter(&mut rig, "DG");
    let title = |rig: &Rig, i: usize| {
        DataGridAdvanced::from_control(rig.d.form.find_control("DG").unwrap()).columns[i].title.clone()
    };
    assert_eq!(title(&rig, 0), "Name");
    rig.double_click(rig.at(40.0 + 30.0, 40.0 + 11.0));
    rig.type_replacing("Customer");
    rig.press_key(Key::Enter);
    assert_eq!(title(&rig, 0), "Customer");
    assert!(rig.prop("DG", DATAGRID_ADVANCED_PROP).contains("Customer"), "saved in AdvancedGrid");
    // The column still shows its data under the new title.
    assert!(
        rig.text_rect("Customer").is_some() && rig.text_rect("Bob").is_some(),
        "{:?}",
        rig.painted.iter().map(|p| &p.0).collect::<Vec<_>>()
    );
    rig.d.undo();
    assert_eq!(title(&rig, 0), "Name");
    // Escape in the rename box drops it.
    rig.double_click(rig.at(40.0 + 30.0, 40.0 + 11.0));
    rig.type_replacing("Nope");
    rig.press_key(Key::Escape);
    assert_eq!(title(&rig, 0), "Name");
    assert!(rig.d.grid_edit.is_some(), "the Escape was the rename box's");
}

#[test]
fn a_title_dragged_on_the_canvas_moves_its_column() {
    let mut rig = Rig::new(grid_form());
    enter(&mut rig, "DG");
    let order = |rig: &Rig| -> Vec<String> {
        grid_edit::shown_columns(rig.d.form.find_control("DG").unwrap())
            .into_iter()
            .map(|c| c.title)
            .collect()
    };
    assert_eq!(order(&rig), ["Name", "Age"]);
    let depth = rig.d.undo_stack.len();
    rig.drag(rig.at(40.0 + 30.0, 40.0 + 11.0), rig.at(40.0 + 120.0 + 30.0, 40.0 + 11.0));
    assert_eq!(order(&rig), ["Age", "Name"], "the title was dropped on the second column");
    assert_eq!(rig.d.undo_stack.len(), depth + 1);
    // The cells follow their columns: the data layout in Rows did not change.
    assert_eq!(rig.prop("DG", "Rows"), "Bob\t30\nann\t9\nCid\t100");
    assert!(rig.text_rect("30").is_some() && rig.text_rect("Bob").is_some());
    rig.d.undo();
    assert_eq!(order(&rig), ["Name", "Age"]);
}

#[test]
fn edit_mode_ends_on_a_press_away_and_on_a_double_click_on_the_frame() {
    let mut rig = Rig::new(grid_form());
    enter(&mut rig, "DG");
    // A press well away from the grid ends it.
    rig.click(rig.at(600.0, 500.0));
    assert!(rig.d.grid_edit.is_none(), "a click away leaves");

    enter(&mut rig, "DG");
    // The frame band is the 8 px outside the grid (it starts at x = 40).
    rig.double_click(rig.at(40.0 - 4.0, 40.0 + 100.0));
    assert!(rig.d.grid_edit.is_none(), "a double-click on the frame leaves");
    assert!(rig.d.form.find_control("DG").is_some(), "…and the grid is as it was");
    assert!(rig.d.undo_stack.is_empty(), "entering and leaving are not edits");
    assert!(!rig.d.dirty);
}

#[test]
fn a_press_away_keeps_the_cell_being_typed() {
    let mut rig = Rig::new(grid_form());
    enter(&mut rig, "DG");
    rig.double_click(rig.at(BOB.0, BOB.1));
    rig.type_replacing("Kept");
    rig.click(rig.at(600.0, 500.0));
    assert!(rig.d.grid_edit.is_none());
    assert_eq!(
        rig.prop("DG", "Rows"),
        "Kept\t30\nann\t9\nCid\t100",
        "clicking away keeps an edit, as the grid does at run time"
    );
}

#[test]
fn the_grids_handles_are_gone_and_its_edges_do_not_resize_it_while_it_is_edited() {
    let mut rig = Rig::new(grid_form());
    // Selected only: the knobs are there and the corner resizes it.
    rig.click(rig.at(BOB.0, BOB.1));
    rig.settle();
    assert!(rig.d.sel_handle_rect.is_some(), "a selected grid has its handles");
    rig.drag(rig.at(340.0, 240.0), rig.at(380.0, 280.0));
    let r = rig.d.form.find_control("DG").unwrap().rect;
    assert!(r.w > 300 && r.h > 200, "outside edit mode the corner handle resizes the grid: {}x{}", r.w, r.h);
    rig.d.undo();

    enter(&mut rig, "DG");
    assert!(rig.d.sel_handle_rect.is_none(), "no handles in edit mode");
    rig.drag(rig.at(340.0, 240.0), rig.at(380.0, 280.0));
    let r = rig.d.form.find_control("DG").unwrap().rect;
    assert_eq!((r.w, r.h, r.x, r.y), (300, 200, 40, 40), "the grid is not moved or resized in edit mode");
    // The arrow keys belong to the grid, not to the control.
    rig.press_key(Key::ArrowRight);
    assert_eq!(rig.d.form.find_control("DG").unwrap().rect.x, 40, "no nudge");
}

#[test]
fn the_toolbar_adds_and_removes_columns_and_rows_each_as_one_undo_step() {
    use grid_edit::GridAction as A;
    let mut rig = Rig::new(grid_form());
    enter(&mut rig, "DG");
    let ctx = rig.ctx.clone();
    let steps = |rig: &Rig| rig.d.undo_stack.len();

    let n = steps(&rig);
    assert!(rig.d.grid_edit_action(&ctx, A::AddColumn));
    assert_eq!(steps(&rig), n + 1, "add column: one step (Columns and Rows together)");
    assert_eq!(rig.prop("DG", "Columns"), "Name:string\nAge:number\nColumn 3:string");
    assert_eq!(rig.prop("DG", "Rows"), "Bob\t30\tText 1\nann\t9\tText 2\nCid\t100\tText 3");

    let n = steps(&rig);
    assert!(rig.d.grid_edit_action(&ctx, A::AddRow));
    assert_eq!(steps(&rig), n + 1);
    assert_eq!(rig.prop("DG", "Rows").lines().count(), 4);

    // Remove acts on the selected cell: select Ann's age (row 2, column 2).
    rig.click(rig.at(40.0 + 120.0 + 40.0, 40.0 + 22.0 + 22.0 + 11.0));
    let sel = grid_edit::selected_cell(&ctx, "DG").expect("a click selects a cell");
    assert_eq!((sel.row_index, sel.display_column_index), (1, 1));
    assert!(rig.d.grid_edit_action(&ctx, A::RemoveRow));
    assert_eq!(
        rig.prop("DG", "Rows"),
        "Bob\t30\tText 1\nCid\t100\tText 3\nText 4\t0\tText 4",
        "Ann's row is gone"
    );
    rig.click(rig.at(40.0 + 120.0 + 40.0, 40.0 + 22.0 + 11.0));
    assert!(rig.d.grid_edit_action(&ctx, A::RemoveColumn));
    assert_eq!(rig.prop("DG", "Columns"), "Name:string\nColumn 3:string", "the selected column (Age) is gone");
    assert_eq!(rig.prop("DG", "Rows"), "Bob\tText 1\nCid\tText 3\nText 4\tText 4");

    // Everything the toolbar did undoes, step by step.
    for _ in 0..4 {
        rig.d.undo();
    }
    assert_eq!(rig.prop("DG", "Columns"), "Name:string\nAge:number");
    assert_eq!(rig.prop("DG", "Rows"), "Bob\t30\nann\t9\nCid\t100");
    // Done leaves the mode.
    assert!(!rig.d.grid_edit_action(&ctx, A::Done));
    assert!(rig.d.grid_edit.is_none());
}

#[test]
fn the_toolbar_is_a_row_of_real_buttons_below_or_above_the_grid() {
    let mut rig = Rig::new(grid_form());
    enter(&mut rig, "DG");
    let bar = rig.d.grid_edit.as_ref().unwrap().toolbar.expect("the toolbar was drawn");
    let grid = rig.d.grid_edit.as_ref().unwrap().screen.unwrap();
    assert!(!bar.intersects(grid), "it never covers the grid: {bar:?} vs {grid:?}");
    // Press its first button (Add column) with the pointer.
    let first = Pos2::new(
        bar.min.x + grid_edit::TOOL_PAD + grid_edit::TOOL_SIZE / 2.0,
        bar.min.y + grid_edit::TOOL_PAD + grid_edit::TOOL_SIZE / 2.0,
    );
    rig.click(first);
    assert_eq!(
        rig.prop("DG", "Columns"),
        "Name:string\nAge:number\nColumn 3:string",
        "the real button added a column"
    );
    assert!(rig.d.grid_edit.is_some(), "a press on the toolbar does not leave the mode");
    // The reset button is disabled on an unbound grid: pressing it changes nothing.
    let reset = first + Vec2::new(4.0 * (grid_edit::TOOL_SIZE + grid_edit::TOOL_GAP), 0.0);
    let depth = rig.d.undo_stack.len();
    rig.click(reset);
    assert_eq!(rig.d.undo_stack.len(), depth);
}

#[test]
fn a_bound_and_an_unbound_grid_edit_alike() {
    let mut form = grid_form();
    let mut bound = form.find_control("DG").unwrap().clone();
    bound.id = "BG".into();
    bound.rect.y = 300;
    form.controls.push(bound);
    form.data_bindings.push(actors_binding("BG"));
    let mut rig = Rig::new(form);

    for (id, y) in [("DG", 40.0f32), ("BG", 300.0)] {
        enter(&mut rig, id);
        rig.double_click(rig.at(40.0 + 50.0, y + 22.0 + 11.0));
        rig.type_replacing("Zed");
        rig.press_key(Key::Enter);
        rig.d.grid_edit = None;
        rig.settle();
    }
    assert_eq!(rig.prop("DG", "Rows"), "Zed\t30\nann\t9\nCid\t100");
    assert_eq!(rig.prop("BG", "Rows"), rig.prop("DG", "Rows"), "the bound grid took the same edit, and kept it");
    // A binding on BG does not bring its rows back on the next frames.
    for _ in 0..10 {
        rig.frame(vec![]);
    }
    assert_eq!(rig.prop("BG", "Rows"), "Zed\t30\nann\t9\nCid\t100");
}

#[test]
fn the_edited_grid_saves_and_reloads_with_its_new_shape() {
    use grid_edit::GridAction as A;
    let mut rig = Rig::new(grid_form());
    enter(&mut rig, "DG");
    let ctx = rig.ctx.clone();
    rig.d.grid_edit_action(&ctx, A::AddColumn);
    rig.d.grid_edit_action(&ctx, A::AddRow);
    rig.double_click(rig.at(40.0 + 30.0, 40.0 + 11.0));
    rig.type_replacing("Customer");
    rig.press_key(Key::Enter);
    let xml = cobolt_forms::form_to_string(&rig.d.form).unwrap();
    let back = cobolt_forms::xml::load_form_from_str(&xml).expect("the saved form loads");
    let grid = back.find_control("DG").unwrap();
    assert_eq!(
        grid.get_prop("Columns").map(|v| v.to_xml_string()).as_deref(),
        Some("Name:string\nAge:number\nColumn 3:string")
    );
    assert_eq!(grid.get_prop("Rows").map(|v| v.to_xml_string()).unwrap_or_default().lines().count(), 4);
    let titles: Vec<String> = grid_edit::shown_columns(grid).into_iter().map(|c| c.title).collect();
    assert_eq!(titles, ["Customer", "Age", "Column 3"]);
    println!("094 T16: edited grid round-trips through the .cfrm: {titles:?}, 4 rows, {} bytes", xml.len());
}

// ── T13: the interactive grid on a static canvas — z-order, clipping, geometry

#[test]
fn the_edited_grid_is_drawn_last_and_clipped_to_its_container() {
    let mut form = Form::new("F", "F", 800, 600);
    let mut panel = Control::new("PNL", ControlType::Panel, 20, 20);
    panel.rect.w = 360;
    panel.rect.h = 260;
    panel.set_prop("CornerRadius", PropValue::Int(24));
    form.controls.push(panel);
    let mut g = grid_form().controls.remove(0);
    g.parent = Some("PNL".into());
    g.rect = cobolt_forms::model::Rect::new(30, 60, 340, 210);
    form.controls.push(g);
    // A button above the grid in z-order that overlaps it.
    let mut b = Control::new("BTN", ControlType::Button, 200, 100);
    b.set_prop("Caption", PropValue::String("OnTop".into()));
    b.z_order = 50;
    form.controls.push(b);
    let mut rig = Rig::new(form);
    enter(&mut rig, "DG");

    let order = |t: &str| rig.painted.iter().position(|p| p.0 == t);
    let (button, cell) = (order("OnTop").expect("button drawn"), order("Bob").expect("grid drawn"));
    assert!(
        cell > button,
        "the edited grid is painted after (over) a sibling above it: button #{button}, grid cell #{cell}"
    );

    // The frame the canvas hands the overlay: the panel's content rect as its
    // clip, and the rounded parent's arc as the grid's container clip.
    let origin = rig.d.canvas_origin.unwrap();
    let frames = rig.d.grid_frames(&rig.d.form.controls, origin, &cobolt_forms::render::DesignedState);
    let frame = frames.iter().find(|f| f.grid.id == "DG").expect("frame");
    let content = rig.d.form.find_control("PNL").unwrap().content_rect();
    let want = Rect::from_min_size(
        origin + Vec2::new(content.x as f32, content.y as f32),
        Vec2::new(content.w as f32, content.h as f32),
    );
    assert_eq!(frame.clip, Some(want), "clipped to the panel's content");
    let clip = frame.container_clip.clone().expect("a rounded parent gives the grid its arc");
    assert_eq!(clip.split(',').count(), 9, "the renderer's nine-number descriptor: {clip}");
    // Every shape of the grid is under a clip no bigger than that content rect.
    let (_, _, cell_clip) = rig.painted.iter().find(|p| p.0 == "Bob").unwrap();
    assert!(
        want.expand(1.0).contains_rect(*cell_clip),
        "cell text clipped to the panel content: {cell_clip:?} vs {want:?}"
    );
    println!("094 T13: edited grid painted after a sibling above it ({cell} > {button}); clip {cell_clip:?} inside panel content {want:?}");
}

#[test]
fn header_cells_follow_what_the_renderer_paints() {
    let cases: Vec<(&str, Vec<(&str, &str)>)> = vec![
        ("plain", vec![]),
        ("title and CSV caption row", vec![("Title", "Actors"), ("ShowCSVExportButton", "true")]),
        ("row numbers", vec![("ShowRowNumbers", "true")]),
        ("filters", vec![("ShowColumnFilters", "true")]),
        ("tall rows", vec![("RowHeight", "34")]),
    ];
    let mut checked = 0;
    for (name, extra) in cases {
        let mut form = grid_form();
        for (k, v) in extra {
            form.find_control_mut("DG").unwrap().set_prop(k, PropValue::String(v.into()));
        }
        let mut rig = Rig::new(form);
        enter(&mut rig, "DG");
        let grid = rig.d.form.find_control("DG").unwrap().clone();
        let screen = rig.d.grid_edit.as_ref().unwrap().screen.unwrap();
        let cells = grid_edit::header_cells_live(&rig.ctx, &grid, screen);
        assert_eq!(cells.len(), 2, "{name}");
        for (i, title) in ["Name", "Age"].iter().enumerate() {
            let painted = rig.text_rect(title).unwrap_or_else(|| panic!("{name}: title {title} painted"));
            assert!(
                cells[i].rect.expand(1.0).contains(painted.center()),
                "{name}: the title {title} is painted at {painted:?}, inside its header cell {:?}",
                cells[i].rect
            );
            checked += 1;
        }
    }
    println!("094 T15: {checked} header titles painted inside the header cells the Designer hit-tests (5 grid shapes)");
}

#[test]
fn the_selected_cell_is_read_back_through_the_renderers_own_id() {
    let mut rig = Rig::new(grid_form());
    enter(&mut rig, "DG");
    assert!(grid_edit::selected_cell(&rig.ctx, "DG").is_none(), "nothing selected yet");
    rig.click(rig.at(40.0 + 120.0 + 30.0, 40.0 + 22.0 + 22.0 + 22.0 + 11.0));
    let sel = grid_edit::selected_cell(&rig.ctx, "DG").expect("selected");
    assert_eq!((sel.row_index, sel.display_column_index), (2, 1), "the click on Cid's age");
    grid_edit::clear_selection(&rig.ctx, "DG");
    assert!(grid_edit::selected_cell(&rig.ctx, "DG").is_none());
}

#[test]
fn every_toolbar_string_is_translated_in_all_six_languages() {
    let fields = |t: &crate::i18n::Tr| {
        [
            t.dg_edit_add_column,
            t.dg_edit_remove_column,
            t.dg_edit_add_row,
            t.dg_edit_remove_row,
            t.dg_edit_reset_sample,
            t.dg_edit_done,
        ]
    };
    let en = fields(&crate::i18n::Language::English.tr());
    for &lang in crate::i18n::Language::ALL {
        let tr = lang.tr();
        for (i, s) in fields(&tr).iter().enumerate() {
            assert!(!s.trim().is_empty(), "{lang:?} string #{i} is empty");
            if lang != crate::i18n::Language::English {
                assert_ne!(*s, en[i], "{lang:?} string #{i} is still English");
            }
        }
    }
    println!(
        "094 T16: 6 toolbar strings x {} languages, none empty, none English outside English",
        crate::i18n::Language::ALL.len()
    );
}

// ── The canvas always draws the real grid (R1) — and only looks at it ───────

fn painted_texts(rig: &Rig) -> Vec<String> {
    rig.painted.iter().map(|p| p.0.clone()).collect()
}

#[test]
fn the_canvas_paints_the_real_grid_and_not_the_placeholder() {
    // Not in edit mode, nothing selected: a plain look at the form.
    let rig = Rig::new(grid_form());
    assert!(rig.d.grid_edit.is_none());
    let texts = painted_texts(&rig);
    for want in ["Name", "Age", "Bob", "30", "ann", "9", "Cid", "100"] {
        assert!(texts.iter().any(|t| t == want), "{want} is painted: {texts:?}");
    }
    assert!(
        !texts.iter().any(|t| t.contains("DataGrid (")),
        "the placeholder label is gone from the canvas: {texts:?}"
    );
    println!("094 R1: a 2-column, 3-row grid paints its header (Name, Age) and its rows (Bob 30 / ann 9 / Cid 100); no placeholder");
}

#[test]
fn the_powerdemo3_grid_is_painted_as_the_real_grid_inside_its_panel() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../examples/PowerDemo3/forms/Data/datagrid-form.cfrm");
    let form = cobolt_forms::load_form(&path).expect("the demo form loads");
    let mut rig = Rig::new(form);
    let texts = painted_texts(&rig);
    for want in ["Actor Id", "Actor Caption", "Actor Salary", "Actor Caption 1", "Actor Awards 2", "Filter..."] {
        assert!(texts.iter().any(|t| t == want), "{want} is painted: {texts:?}");
    }
    assert!(!texts.iter().any(|t| t.contains("DataGrid (")), "no placeholder: {texts:?}");
    // It is drawn inside Panel-1, clipped to it, like any child of that panel.
    let origin = rig.d.canvas_origin.unwrap();
    let frames = rig.d.grid_frames(&rig.d.form.controls, origin, &cobolt_forms::render::DesignedState);
    let frame = frames.iter().find(|f| f.grid.id == "DataGrid-1").expect("the grid is a frame");
    assert!(frame.clip.is_some(), "clipped to its panel");
    assert!(frame.container_clip.is_some(), "and given the panel's rounded arc");
    // Row numbers and a filter well per column: header + filters + 2 rows.
    let filters = texts.iter().filter(|t| *t == "Filter...").count();
    println!(
        "094 R1: PowerDemo3 Datagrid form -> {} texts painted, {filters} filter wells, 2 fake rows, grid clipped to Panel-1 with its arc",
        texts.len()
    );
    // Entering edit mode on the real form draws the same grid, now live.
    assert!(rig.d.enter_grid_edit("DataGrid-1"));
    rig.settle();
    assert!(painted_texts(&rig).iter().any(|t| t == "Actor Caption 1"));
}

fn selection_of(rig: &Rig, scope: egui::Id, id: &str) -> Option<cobolt_forms::datagrid::DataGridCellSelection> {
    let wid = cobolt_forms::render::control_widget_id(Some(scope), id).with("datagrid-selection");
    rig.ctx.memory(|m| m.data.get_temp(wid))
}

#[test]
fn a_grid_that_is_only_looked_at_takes_no_input_and_the_canvas_gestures_work_over_it() {
    let mut form = grid_form();
    {
        let g = form.find_control_mut("DG").unwrap();
        g.set_prop("ShowColumnFilters", PropValue::String("true".into()));
        g.set_prop("AllowCellEditing", PropValue::String("true".into()));
    }
    let mut rig = Rig::new(form);
    let grid = rig.d.form.find_control("DG").unwrap().clone();
    let screen = Rect::from_min_size(rig.at(40.0, 40.0), Vec2::new(300.0, 200.0));
    let cells = grid_edit::header_cells(&grid, screen, 0.0);
    let first_row_y = cells[0].rect.max.y - screen.min.y + 11.0; // into Bob's row
    let bob = rig.at(90.0, 40.0 + first_row_y);
    let rows_before = rig.prop("DG", "Rows");
    let advanced_before = rig.prop("DG", DATAGRID_ADVANCED_PROP);

    // 1. A click on a cell selects the CONTROL, not the grid's cell.
    rig.click(bob);
    assert_eq!(rig.d.selected_ids, ["DG"], "the designer's own selection works over the grid");
    assert!(selection_of(&rig, grid_edit::view_scope(), "DG").is_none(), "the grid took no cell selection");
    // 2. A double-click on a cell enters edit mode — it does NOT open a cell editor.
    rig.double_click(bob);
    assert!(rig.d.grid_edit.is_some());
    rig.d.grid_edit = None;
    rig.settle();
    assert_eq!(rig.prop("DG", "Rows"), rows_before, "no editor ran");

    // 3. Header sorting: a click on a title reorders nothing.
    let y_of = |rig: &Rig, t: &str| rig.text_rect(t).map(|r| r.center().y).unwrap();
    let order_before = (y_of(&rig, "Bob"), y_of(&rig, "ann"), y_of(&rig, "Cid"));
    rig.click(cells[0].rect.center() - Vec2::new(30.0, 0.0));
    rig.click(cells[0].rect.center() - Vec2::new(30.0, 0.0));
    let order_after = (y_of(&rig, "Bob"), y_of(&rig, "ann"), y_of(&rig, "Cid"));
    assert_eq!(order_before, order_after, "rows are in the order of Rows, whatever was clicked");

    // 4. A filter well takes no typing.
    let well = Pos2::new(cells[0].rect.center().x, cells[0].rect.min.y + 30.0);
    rig.click(well);
    rig.frame(vec![Event::Text("zzz".to_owned())]);
    rig.settle();
    assert_eq!(rig.prop("DG", "ColumnFilters"), "", "nothing typed into a filter");
    assert_eq!(rig.prop("DG", DATAGRID_ADVANCED_PROP), advanced_before);
    assert!(rig.text_rect("Bob").is_some(), "no row was filtered away");

    // 5. A column edge shows the canvas's cursor, not the grid's resize cursor.
    rig.frame(vec![Event::PointerMoved(rig.at(40.0 + 120.0, 51.0))]);
    rig.frame(vec![]);
    assert_ne!(rig.cursor, egui::CursorIcon::ResizeHorizontal, "no grid cursor over a grid that is only looked at");

    // 6. Dragging the grid moves it, like any control.
    rig.click(rig.at(600.0, 500.0));
    rig.drag(bob, bob + Vec2::new(48.0, 24.0));
    let r = rig.d.form.find_control("DG").unwrap().rect;
    assert!(r.x > 40 && r.y > 40, "the drag moved the grid to ({}, {})", r.x, r.y);
    rig.d.undo();

    // 7. A rubber band around it selects it.
    rig.click(rig.at(600.0, 500.0));
    assert!(rig.d.selected_ids.is_empty());
    rig.drag(rig.at(5.0, 5.0), rig.at(400.0, 300.0));
    assert_eq!(rig.d.selected_ids, ["DG"], "the rubber band took the grid");

    // 8. The wheel over it is the canvas's, not the grid's.
    let scroll_y = |rig: &Rig| -> f32 {
        let wid = cobolt_forms::render::control_widget_id(Some(grid_edit::view_scope()), "DG").with("datagrid-scroll-y");
        rig.ctx.memory(|m| m.data.get_temp::<f32>(wid)).unwrap_or(0.0)
    };
    let over = rig.at(90.0, 150.0);
    rig.frame(vec![Event::PointerMoved(over)]);
    rig.frame(vec![Event::MouseWheel {
        unit: egui::MouseWheelUnit::Point,
        delta: Vec2::new(0.0, -80.0),
        phase: egui::TouchPhase::Move,
        modifiers: egui::Modifiers::NONE,
    }]);
    rig.settle();
    assert_eq!(scroll_y(&rig), 0.0, "the grid did not scroll");
    assert_eq!(rig.prop("DG", "Rows"), rows_before);
    println!("094 R1: a grid only looked at takes no click, header sort, filter typing, resize cursor or wheel; select/move/rubber-band work over it");
}

#[test]
fn input_is_handed_back_after_a_grid_is_drawn_for_viewing() {
    let ctx = egui::Context::default();
    let mut input = egui::RawInput::default();
    input.screen_rect = Some(egui::Rect::from_min_size(Pos2::ZERO, SCREEN));
    input.events = vec![
        Event::Text("x".to_owned()),
        Event::MouseWheel { unit: egui::MouseWheelUnit::Point, delta: Vec2::new(0.0, -40.0), phase: egui::TouchPhase::Move, modifiers: egui::Modifiers::NONE },
    ];
    let mut seen = None;
    let mut out = ctx.run_ui(input, |_| {
        let before = ctx.input(|i| (i.events.len(), i.smooth_scroll_delta));
        let during = grid_edit::without_input(&ctx, || ctx.input(|i| (i.events.len(), i.smooth_scroll_delta)));
        let after = ctx.input(|i| (i.events.len(), i.smooth_scroll_delta));
        seen = Some((before, during, after));
    });
    out.textures_delta.clear();
    let (before, during, after) = seen.expect("the frame ran");
    assert!(before.0 >= 2 && before.1.y != 0.0, "the frame had input: {before:?}");
    assert_eq!((during.0, during.1), (0, Vec2::ZERO), "the grid saw none of it");
    assert_eq!(after, before, "and the canvas gets all of it back");
    println!("094 R1: input during the viewing draw: {} events, scroll {:?} -> taken away; handed back whole", before.0, before.1);
}

#[test]
fn a_grid_only_looked_at_is_not_dimmed() {
    // `Ui::disable` fades the painter; the grid is drawn at full opacity, so every
    // colour is the one the live (edit-mode) grid paints.
    let mut rig = Rig::new(grid_form());
    let screen = Rect::from_min_size(rig.at(40.0, 40.0), Vec2::new(300.0, 200.0));
    let inside = |rig: &Rig| -> Vec<(i32, i32, [u8; 4])> {
        let mut v: Vec<_> = rig
            .fills
            .iter()
            .filter(|(r, c)| screen.expand(0.5).contains_rect(*r) && c.a() > 0)
            .map(|(r, c)| (r.min.x as i32, r.min.y as i32, c.to_array()))
            .collect();
        v.sort();
        v
    };
    let ink_of = |rig: &Rig, t: &str| {
        let ix = rig.painted.iter().position(|p| p.0 == t).unwrap_or_else(|| panic!("{t} painted"));
        rig.ink[ix]
    };
    let (viewed_fills, viewed_name, viewed_bob) = (inside(&rig), ink_of(&rig, "Name"), ink_of(&rig, "Bob"));
    assert_eq!(viewed_name.1, 1.0, "full opacity");
    assert_eq!(viewed_bob.1, 1.0, "full opacity");

    enter(&mut rig, "DG");
    let (live_fills, live_name, live_bob) = (inside(&rig), ink_of(&rig, "Name"), ink_of(&rig, "Bob"));
    assert_eq!(viewed_name, live_name, "header ink");
    assert_eq!(viewed_bob, live_bob, "cell ink");
    assert!(!viewed_fills.is_empty());
    assert_eq!(viewed_fills, live_fills, "every fill inside the grid has the same colour viewed as live");
    println!(
        "094 R1: {} fills inside the grid and 2 inks identical viewed vs live (opacity {})",
        viewed_fills.len(),
        viewed_name.1
    );
}

#[test]
fn a_grid_is_painted_after_every_other_face_which_is_the_cost_of_a_separate_pass() {
    // The engine's static pass draws the faces in z-order but cannot draw the real
    // grid (it only has a painter), so the grids are drawn right after it. The cost:
    // a control ABOVE a grid in z-order that overlaps it is under it on the canvas.
    let mut form = grid_form();
    let mut label = Control::new("LBL", ControlType::Label, 60, 60);
    label.set_prop("Caption", PropValue::String("Above".into()));
    label.z_order = 90;
    form.controls.push(label);
    let rig = Rig::new(form);
    let order = |t: &str| rig.painted.iter().position(|p| p.0 == t).unwrap();
    assert!(order("Bob") > order("Above"), "the grid's rows are painted after the label that is above it in z-order");
}
