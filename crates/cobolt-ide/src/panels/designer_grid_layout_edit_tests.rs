// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Spec 088 — editing a Grid container's layout on the Form Designer's canvas: the
//! commands and the history here, the gestures further down.
//!
//! A child module of `designer`, so it reads the designer's private state the way the
//! other test modules there do.

use super::*;
use crate::llm::LlmConfig;
use cobolt_forms::layout::grid_edit::{Axis, Refusal};
use egui::{Event, PointerButton};

const SCREEN: Vec2 = Vec2::new(1400.0, 1000.0);

fn press(at: Pos2, pressed: bool) -> Event {
    Event::PointerButton { pos: at, button: PointerButton::Primary, pressed, modifiers: egui::Modifiers::NONE }
}

/// A designer in a window of its own, one real frame at a time, on a clock.
struct Rig {
    ctx: egui::Context,
    llm: LlmConfig,
    d: DesignerPanel,
    t: f64,
    /// The cursor the last frame asked for.
    cursor: egui::CursorIcon,
    /// What the last frame painted.
    shapes: Vec<egui::epaint::ClippedShape>,
}

impl Rig {
    fn new(form: Form) -> Self {
        let mut rig = Self {
            ctx: egui::Context::default(),
            llm: LlmConfig::load_defaults_for_test(),
            d: DesignerPanel::new(form),
            t: 0.0,
            cursor: egui::CursorIcon::Default,
            shapes: Vec::new(),
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
        self.cursor = out.platform_output.cursor_icon;
        self.shapes = std::mem::take(&mut out.shapes);
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

    fn prop(&self, id: &str, key: &str) -> String {
        prop(&self.d, id, key)
    }

    /// Hold Shift (or let it go): egui keeps the modifiers it was last told about.
    fn shift(&mut self, held: bool) {
        let m = egui::Modifiers { shift: held, ..Default::default() };
        self.frame(vec![Event::ModifiersChanged(m)]);
    }

    fn key(&mut self, k: egui::Key) {
        self.frame(vec![Event::Key { key: k, physical_key: None, pressed: true, repeat: false, modifiers: egui::Modifiers::NONE }]);
        self.settle();
    }

    /// Press toolbar button `tool`.
    fn tool(&mut self, tool: crate::panels::layout_grid_edit::Tool) {
        use crate::panels::layout_grid_edit as lg;
        let strip = self.d.layout_grid.toolbar.expect("the floating toolbar is up");
        let i = lg::TOOLS.iter().position(|t| *t == tool).unwrap();
        let x = strip.min.x + lg::TOOL_PAD + i as f32 * (lg::TOOL_SIZE + lg::TOOL_GAP) + lg::TOOL_SIZE / 2.0;
        self.click(Pos2::new(x, strip.min.y + lg::TOOL_PAD + lg::TOOL_SIZE / 2.0));
        self.settle();
    }

    /// A point on track line `line` of Grid container `id` (the line after that track),
    /// a quarter of the way along it, on screen. A quarter point is added so a coordinate that is a whole
    /// number on the canvas stays one when egui truncates it.
    fn line_point(&self, id: &str, axis: cobolt_forms::layout::grid_edit::Axis, line: usize) -> Pos2 {
        use cobolt_forms::layout::grid_edit::Axis;
        let g = crate::panels::layout_grid_edit::GridGeom::of(self.d.canvas_layout.as_ref().unwrap(), id).unwrap();
        let b = g.bounds();
        // A quarter of the way along, so it is not where a column line and a row line cross.
        match axis {
            Axis::Column => self.at(g.line_pos(axis, line).round() + 0.25, b.y + b.h / 4.0),
            Axis::Row => self.at(b.x + b.w / 4.0, g.line_pos(axis, line).round() + 0.25),
        }
    }

    /// The centre of control `id` as the canvas laid it out, on screen.
    fn centre_of(&self, id: &str) -> Pos2 {
        let r = self.d.canvas_layout.as_ref().unwrap().rects[id];
        self.at(r.x + r.w / 2.0, r.y + r.h / 2.0)
    }

    /// Where selection handle `h` of control `id` is, on screen: on the rectangle the canvas
    /// laid the control out in. A quarter point is added so a coordinate that is a whole number
    /// on the canvas stays one when egui truncates it.
    fn handle_point(&self, id: &str, h: Handle) -> Pos2 {
        let r = self.d.canvas_layout.as_ref().unwrap().rects[id];
        let rect = cobolt_forms::model::Rect::new(r.x.round() as i32, r.y.round() as i32, r.w.round() as i32, r.h.round() as i32);
        self.d.canvas_origin.unwrap() + handle_pos(&rect, h).to_vec2() + Vec2::splat(0.25)
    }

    /// A point inside control `id`, a fraction of the way across and down, on screen — off
    /// its selection handles, which on a small control cover the middle.
    fn point_in(&self, id: &str, fx: f32, fy: f32) -> Pos2 {
        let r = self.d.canvas_layout.as_ref().unwrap().rects[id];
        self.at(r.x + r.w * fx, r.y + r.h * fy)
    }

    /// The centre of cell `(row, column)` of Grid container `id`, on screen.
    fn cell_centre(&self, id: &str, row: usize, column: usize) -> Pos2 {
        let g = crate::panels::layout_grid_edit::GridGeom::of(self.d.canvas_layout.as_ref().unwrap(), id).unwrap();
        let c = g.cell_rect(row, column);
        self.at(c.x + c.w / 2.0, c.y + c.h / 2.0)
    }
}

/// PowerSpatial's main form, as committed — the spec's own example.
fn power_spatial() -> Form {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/PowerSpatial/forms/main-form.cfrm");
    cobolt_forms::load_form(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

/// A designer on a responsive form holding one Grid container `P` (`1fr 1fr` by
/// `1fr 1fr`) with `A` in cell (1, 1) and `B` in cell (2, 2).
fn designer() -> DesignerPanel {
    let mut f = Form::new("F", "F", 400, 300);
    f.responsive = true;
    let mut p = Control::new("P", ControlType::Panel, 0, 0);
    p.rect = cobolt_forms::model::Rect::new(0, 0, 300, 120);
    p.set_prop("LayoutMode", PropValue::String("Grid".into()));
    p.set_prop("GridColumns", PropValue::String("1fr 1fr".into()));
    p.set_prop("GridRows", PropValue::String("1fr 1fr".into()));
    p.set_prop("BorderStyle", PropValue::String("None".into()));
    p.set_prop("BorderWidth", PropValue::Int(0));
    f.controls.push(p);
    for (id, col, row) in [("A", 1, 1), ("B", 2, 2)] {
        let mut c = Control::new(id, ControlType::Button, 0, 0);
        c.rect = cobolt_forms::model::Rect::new(0, 0, 50, 30);
        c.parent = Some("P".into());
        c.set_prop("GridColumn", PropValue::Int(col));
        c.set_prop("GridRow", PropValue::Int(row));
        f.controls.push(c);
    }
    let mut d = DesignerPanel::new(f);
    lay_out(&mut d);
    d
}

/// What a frame does first: lay the form out, so the editor sees the grid as the
/// engine placed it.
fn lay_out(d: &mut DesignerPanel) {
    let ctx = egui::Context::default();
    let st = DesignerState { anim: &Default::default(), tabs: None };
    let pr = d.canvas_prepare(&ctx, &d.form.controls.clone(), &st);
    d.canvas_layout = Some(pr.layout);
}

fn prop(d: &DesignerPanel, id: &str, key: &str) -> String {
    d.form.find_control(id).and_then(|c| c.get_prop(key)).map(|v| v.to_xml_string()).unwrap_or_default()
}

/// R13, R25 — Undo and Restore are separate and neither leaves the other lying: a
/// removal is on the history until it is undone; Restore takes it off, and undoing the
/// Restore puts it back where it was.
#[test]
fn undo_and_restore_keep_the_history_true() {
    let mut d = designer();
    let undo_before = d.undo_stack.len();

    // Remove row 1: it is one undo step, and it is remembered.
    assert_eq!(d.grid_remove("P", Axis::Row, 0), Ok(vec!["A".to_owned()]));
    assert_eq!(d.undo_stack.len(), undo_before + 1, "one step for the whole removal");
    assert_eq!(prop(&d, "P", "GridRows"), "1fr");
    assert_eq!(d.layout_grid.history.len(), 1);

    // Undo the removal: the row is back, and there is nothing to restore.
    d.undo_unchecked();
    assert_eq!(prop(&d, "P", "GridRows"), "1fr 1fr");
    assert_eq!(prop(&d, "A", "GridRow"), "1", "A is where it was");
    assert!(d.layout_grid.history.is_empty(), "an undone removal is not on the history");
    lay_out(&mut d);
    assert_eq!(d.grid_restore("P"), Err(Refusal::NoSuchTrack), "nothing to restore, so no row is added twice");
    assert_eq!(prop(&d, "P", "GridRows"), "1fr 1fr");

    // Redo it: it is on the history again; Restore puts the row back.
    d.redo_unchecked();
    assert_eq!(d.layout_grid.history.len(), 1, "a redone removal is remembered again");
    lay_out(&mut d);
    assert_eq!(d.grid_restore("P"), Ok(()));
    assert_eq!(prop(&d, "P", "GridRows"), "1fr 1fr");
    assert_eq!(prop(&d, "A", "GridRow"), "1");
    assert_eq!(prop(&d, "B", "GridRow"), "2");
    assert!(d.layout_grid.history.is_empty(), "a restored removal leaves the history");

    // Undo the Restore: the row is gone again and the record is back to be restored.
    d.undo_unchecked();
    assert_eq!(prop(&d, "P", "GridRows"), "1fr");
    assert_eq!(d.layout_grid.history.len(), 1, "undoing the Restore gives the record back");
    println!("history: remove → undo → (nothing to restore) → redo → restore → undo-restore: {} record, rows `{}`", d.layout_grid.history.len(), prop(&d, "P", "GridRows"));
}

/// R13 — Restore is independent of what was done since: an unrelated edit made after
/// the removal is still there afterwards, and the most recent removal comes back first.
#[test]
fn restore_keeps_an_unrelated_edit_and_takes_the_latest_first() {
    let mut d = designer();
    d.grid_remove("P", Axis::Row, 0).unwrap();
    lay_out(&mut d);
    d.grid_remove("P", Axis::Column, 1).unwrap();
    assert_eq!((prop(&d, "P", "GridRows"), prop(&d, "P", "GridColumns")), ("1fr".into(), "1fr".into()));
    // An edit elsewhere, made after both removals.
    d.apply(Cmd::SetProperty {
        id: "B".into(),
        key: "BackgroundColor".into(),
        old: None,
        new: PropValue::String("#112233".into()),
    });
    lay_out(&mut d);
    // Most recent first: the column.
    assert_eq!(d.grid_restore("P"), Ok(()));
    assert_eq!(prop(&d, "P", "GridColumns"), "1fr 1fr", "the column came back first");
    assert_eq!(prop(&d, "P", "GridRows"), "1fr", "the row is still to come");
    lay_out(&mut d);
    assert_eq!(d.grid_restore("P"), Ok(()));
    assert_eq!(prop(&d, "P", "GridRows"), "1fr 1fr");
    assert_eq!(prop(&d, "B", "BackgroundColor"), "#112233", "the unrelated edit remains");
    println!("restore order: column first, then row; B's own colour kept");
}

/// A control renamed after a removal is still found by Restore: the history follows it.
#[test]
fn a_rename_is_followed_by_the_history() {
    let mut d = designer();
    d.grid_remove("P", Axis::Row, 0).unwrap();
    assert!(d.rename_control("P", "CARD"));
    assert!(d.rename_control("A", "ALPHA"));
    let r = &d.layout_grid.history[0];
    assert_eq!((r.container.as_str(), r.held[0].id.as_str()), ("CARD", "ALPHA"));
    // Undoing the two renames takes the names back — in the history too — and redoing them
    // brings the new ones back.
    d.undo_unchecked();
    d.undo_unchecked();
    let r = &d.layout_grid.history[0];
    assert_eq!((r.container.as_str(), r.held[0].id.as_str()), ("P", "A"), "undone renames");
    d.redo_unchecked();
    d.redo_unchecked();
    let r = &d.layout_grid.history[0];
    assert_eq!((r.container.as_str(), r.held[0].id.as_str()), ("CARD", "ALPHA"), "redone renames");
    lay_out(&mut d);
    assert_eq!(d.grid_restore("CARD"), Ok(()));
    assert_eq!(prop(&d, "CARD", "GridRows"), "1fr 1fr");
    assert_eq!(prop(&d, "ALPHA", "GridRow"), "1", "the held control came back by its new name");
    println!("rename: container CARD and control ALPHA found by Restore");
}

/// An add is one step and undoes whole: the track and every renumbered child go back
/// together.
#[test]
fn an_added_track_is_one_undo_step() {
    let mut d = designer();
    let before = (prop(&d, "P", "GridColumns"), prop(&d, "B", "GridColumn"));
    let steps = d.undo_stack.len();
    assert_eq!(d.grid_insert("P", Axis::Column, Some(0)), Ok(()));
    assert_eq!(d.undo_stack.len(), steps + 1);
    assert_eq!((prop(&d, "P", "GridColumns"), prop(&d, "B", "GridColumn")), ("1fr 1fr 1fr".into(), "3".into()));
    d.undo_unchecked();
    assert_eq!((prop(&d, "P", "GridColumns"), prop(&d, "B", "GridColumn")), before);
    println!("add column: `1fr 1fr` → `1fr 1fr 1fr` (B to column 3) in one step; one Ctrl+Z returns `{}` and column {}", before.0, before.1);
}

/// The mode is only on when nothing else owns the pointer and there is a grid to edit
/// (plan §1): each excluded state turns it off, and turning the toggle off does too.
#[test]
fn grid_editing_mode_is_on_only_where_it_can_be() {
    let mut d = designer();
    assert!(d.layout_grid_available() && d.layout_grid_mode(), "on a responsive form with a grid, by default");

    d.layout_grid.show = false;
    assert!(d.layout_grid_available() && !d.layout_grid_mode(), "Show Grid off: available, but not the mode");
    d.layout_grid.show = true;

    let mut check = |name: &str, change: &dyn Fn(&mut DesignerPanel), restore: &dyn Fn(&mut DesignerPanel)| {
        change(&mut d);
        assert!(!d.layout_grid_mode(), "{name}: the mode is off");
        restore(&mut d);
        assert!(d.layout_grid_mode(), "{name}: and back on after");
    };
    check("a form that does not lay out", &|d| d.form.responsive = false, &|d| d.form.responsive = true);
    check(
        "the Format Painter armed",
        &|d| {
            d.format_painter = FormatPainter::WaitingForSource;
        },
        &|d| d.format_painter = FormatPainter::Idle,
    );
    check(
        "a control being placed",
        &|d| d.drag = DragState::PlacingNew { ctrl_type: ControlType::Button, start_x: 0, start_y: 0, cur_x: 0, cur_y: 0 },
        &|d| d.drag = DragState::None,
    );
    check("the Non-Visuals tab", &|d| d.tabs.select_non_visuals(), &|d| d.tabs.select_form());
    check(
        "Visual Tab Order open",
        &|d| d.tab_order_modal = Some(super::super::tab_order::TabOrderModal::new(&d.form, None)),
        &|d| d.tab_order_modal = None,
    );
    check("DataGrid edit mode", &|d| d.grid_edit = Some(super::super::grid_edit::GridEditState::new("P")), &|d| d.grid_edit = None);

    // No grid to edit: nothing to act on.
    let mut plain = DesignerPanel::new({
        let mut f = Form::new("F", "F", 400, 300);
        f.responsive = true;
        f
    });
    lay_out(&mut plain);
    assert!(plain.layout_grid_available() && !plain.layout_grid_mode(), "a form with no Grid container has no grid to edit");
    println!("mode: on by default; off for Show Grid off, a non-laid-out form, Format Painter, placing, Non-Visuals, DataGrid edit; and with no grid");
}

// ── T11 — the Show Grid button and the track-line painter ────────────────────

/// The Designer toolbar drawn with Show Grid `on` or off: every shape it painted, as text.
fn toolbar_shapes(on: bool) -> String {
    let ctx = egui::Context::default();
    let mut input = egui::RawInput::default();
    input.screen_rect = Some(egui::Rect::from_min_size(Pos2::ZERO, Vec2::new(1800.0, 60.0)));
    let mut shapes = String::new();
    let mut out = ctx.run_ui(input, |root| {
        egui::CentralPanel::default().show_inside(root, |ui| {
            let _ = draw_icon_toolbar(
                ui, true, true, true, false, true, "Cut", "Copy", "Paste", "Duplicate", "Format painter", "Tab order", "Tab list",
                false, false, true, true, false, false, false, false, false, false, false, "Magnifier", on, "Show Grid",
                false, "Task Wizard", "Choose a task",
            );
        });
    });
    out.textures_delta.clear();
    for c in &out.shapes {
        shapes.push_str(&format!("{:?}\n", c.shape));
    }
    shapes
}

/// AC1 (R1, R2, R5) — a fresh designer has Show Grid on; the button reads as engaged
/// while it is, and plain when it is not; toggling it changes no byte of the form.
#[test]
fn show_grid_starts_on_reads_as_engaged_and_changes_no_byte_of_the_form() {
    let mut d = designer();
    assert!(d.layout_grid.show, "on by default (R2)");
    let (on, off) = (toolbar_shapes(true), toolbar_shapes(false));
    assert_ne!(on, off, "the button is painted differently when engaged (R2)");
    // Everything but that button is painted the same: the engaged one adds a shape or two
    // (its highlight) and recolours the icon, and nothing else changes.
    let (on_set, off_set): (std::collections::HashSet<&str>, std::collections::HashSet<&str>) = (on.lines().collect(), off.lines().collect());
    let diff = on_set.symmetric_difference(&off_set).count();
    // The highlight, and the icon's five strokes in the engaged ink: one button's worth.
    assert!((1..=16).contains(&diff), "only the one button differs: {diff} shapes");
    assert!(on.lines().count() > off.lines().count(), "the engaged button paints its highlight");
    let saved = cobolt_forms::form_to_string(&d.form).unwrap();
    d.toggle_layout_grid();
    assert!(!d.layout_grid.show);
    d.toggle_layout_grid();
    assert!(d.layout_grid.show);
    assert_eq!(cobolt_forms::form_to_string(&d.form).unwrap(), saved, "toggling twice, and once, leave every byte of the form as it was");
    d.toggle_layout_grid();
    assert_eq!(cobolt_forms::form_to_string(&d.form).unwrap(), saved);
    assert!(d.undo_stack.is_empty(), "a view setting is not an undo step");
    println!("Show Grid: on by default; the button differs by {diff} painted shapes when engaged; the form's {} bytes unchanged by toggling", saved.len());
}

/// AC2 (R3) — with Show Grid on and nothing selected, every Grid container's track lines
/// are drawn: on PowerSpatial's main form, every Grid container the layout reports.
#[test]
fn with_show_grid_on_every_grid_container_draws_its_lines() {
    let form = power_spatial();
    assert!(form.lays_out(), "PowerSpatial's main form lays out (spec 056)");
    let mut rig = Rig::new(form);
    rig.settle();
    assert!(rig.d.selected_ids.is_empty());
    let expected: Vec<String> = crate::panels::layout_grid_edit::GridGeom::all(rig.d.canvas_layout.as_ref().unwrap()).into_iter().map(|g| g.id).collect();
    assert!(expected.len() >= 5, "the form has several grids: {expected:?}");
    let mut drawn = rig.d.grid_overlay_ids.clone();
    drawn.sort();
    let mut want = expected.clone();
    want.sort();
    assert_eq!(drawn, want, "every Grid container's lines were drawn");
    assert!(expected.iter().any(|id| id == "Crd-AC"));

    // Show Grid off, nothing selected: no grid's lines.
    rig.d.toggle_layout_grid();
    rig.settle();
    assert!(rig.d.grid_overlay_ids.is_empty(), "Show Grid off draws none: {:?}", rig.d.grid_overlay_ids);
    // …until a Grid container is selected (spec 056 R45, unchanged): its own lines.
    rig.d.select_only("Crd-AC");
    rig.settle();
    // Its own lines (spec 056 R45), and — it being a control in a grid — that grid's (A1).
    let mut shown = rig.d.grid_overlay_ids.clone();
    shown.sort();
    assert_eq!(shown, vec!["Crd-AC".to_owned(), "Pnl-Left".to_owned()]);
    println!("AC2: PowerSpatial main-form — {} Grid containers, all {} drawn with Show Grid on; none with it off; Crd-AC with the grid it sits in when selected", expected.len(), drawn.len());
}

// ── T12 — grid-editing mode: cells and ranges ────────────────────────────────

/// AC3 (R6) — with Show Grid on, a press on `Lbl-AC-Sub` inside `Crd-AC` selects a cell
/// of `Crd-AC`, not the label, and dragging cannot move it: nothing in the form changes.
#[test]
fn a_press_on_a_control_in_a_grid_selects_a_cell_and_the_control_cannot_be_moved() {
    let mut rig = Rig::new(power_spatial());
    let saved = cobolt_forms::form_to_string(&rig.d.form).unwrap();
    let steps = rig.d.undo_stack.len();
    let label = rig.centre_of("Lbl-AC-Sub");
    rig.click(label);
    rig.settle();
    assert_eq!(rig.d.selected_ids, vec!["Crd-AC".to_owned()], "the container is selected, not the label");
    let sel = rig.d.layout_grid.selection.clone().expect("a cell is selected");
    assert_eq!((sel.container.as_str(), sel.anchor, sel.head), ("Crd-AC", (1, 0), (1, 0)), "the label is in column 1, row 2");
    assert_eq!(rig.d.layout_grid.history.len(), 0);

    // Dragging from the label does not move it — it selects cells.
    let (before_x, before_y) = (rig.prop("Lbl-AC-Sub", "X"), rig.prop("Lbl-AC-Sub", "Y"));
    let rect_before = rig.d.form.find_control("Lbl-AC-Sub").unwrap().rect;
    rig.drag(label, label + Vec2::new(90.0, 60.0));
    assert_eq!(rig.d.form.find_control("Lbl-AC-Sub").unwrap().rect, rect_before, "the label did not move");
    assert_eq!(rig.d.undo_stack.len(), steps, "no command was made");
    assert_eq!(cobolt_forms::form_to_string(&rig.d.form).unwrap(), saved, "the form is byte for byte as it was");
    assert_eq!(rig.d.selected_ids, vec!["Crd-AC".to_owned()]);
    let _ = (before_x, before_y);
    println!("AC3: a press on Lbl-AC-Sub selects Crd-AC and cell (row 2, column 1); a drag from it changes nothing ({} bytes unchanged, {} undo steps)", saved.len(), steps);
}

/// R7 — a drag across cells, or a Shift-press, selects a rectangle of them.
#[test]
fn cells_are_selected_by_a_drag_and_widened_by_shift() {
    let mut rig = Rig::new(power_spatial());
    let (a, b) = (rig.cell_centre("Crd-AC", 1, 0), rig.cell_centre("Crd-AC", 2, 1));
    rig.drag(a, b);
    let sel = rig.d.layout_grid.selection.clone().unwrap();
    assert_eq!((sel.anchor, sel.head), ((1, 0), (2, 1)));
    let r = sel.range();
    assert_eq!((r.row, r.column, r.rows, r.columns), (1, 0, 2, 2));
    assert_eq!(rig.d.selected_ids, vec!["Crd-AC".to_owned()]);

    // A fresh press selects one cell; Shift on a second widens it.
    let c = rig.cell_centre("Crd-AC", 0, 1);
    rig.click(c);
    assert_eq!(rig.d.layout_grid.selection.as_ref().unwrap().range().rows, 1);
    rig.shift(true);
    let d = rig.cell_centre("Crd-AC", 3, 0);
    rig.click(d);
    rig.shift(false);
    let r = rig.d.layout_grid.selection.as_ref().unwrap().range();
    assert_eq!((r.row, r.column, r.rows, r.columns), (0, 0, 4, 2), "from (0,1) to (3,0), the corners either way round");
    // A drag that leaves the grid still names the cell at its edge.
    let inside = rig.cell_centre("Crd-AC", 1, 0);
    rig.drag(inside, rig.at(5000.0f32.min(1300.0), 5000.0f32.min(900.0)));
    let sel = rig.d.layout_grid.selection.clone().unwrap();
    let g = crate::panels::layout_grid_edit::GridGeom::of(rig.d.canvas_layout.as_ref().unwrap(), "Crd-AC").unwrap();
    assert_eq!(sel.head, (g.rows.len() - 1, g.columns.len() - 1), "the far corner of the grid");
    println!("cells: drag (1,0)→(2,1) = rows 1–2 × columns 0–1; click + Shift-click = rows 0–3 × columns 0–1; a drag out of the grid ends at its last cell");
}

/// A cell selected in a Grid container is not the container: Delete, Cut and the arrow
/// keys leave the container where it is and as it is.
#[test]
fn delete_and_the_arrows_do_not_act_on_the_container_while_cells_are_selected() {
    let mut rig = Rig::new(power_spatial());
    rig.click(rig.cell_centre("Crd-AC", 1, 0));
    assert!(rig.d.layout_grid_cell_selected());
    let rect = rig.d.form.find_control("Crd-AC").unwrap().rect;
    let n = rig.d.form.controls.len();
    let saved = cobolt_forms::form_to_string(&rig.d.form).unwrap();
    for k in [egui::Key::Delete, egui::Key::Backspace, egui::Key::ArrowRight, egui::Key::ArrowDown] {
        rig.key(k);
    }
    assert!(rig.d.pending_delete.is_none(), "no deletion was even offered for the container");
    assert_eq!(rig.d.form.controls.len(), n, "nothing was deleted");
    assert_eq!(rig.d.form.find_control("Crd-AC").unwrap().rect, rect, "and nothing was nudged");
    assert_eq!(cobolt_forms::form_to_string(&rig.d.form).unwrap(), saved);
    println!("keys: Delete, Backspace and two arrows with cells selected — the container and its {} siblings untouched", n - 1);
}

/// With Show Grid off the pointer is the ordinary one: the same press selects the label.
#[test]
fn with_show_grid_off_a_press_selects_the_control_as_before() {
    let mut rig = Rig::new(power_spatial());
    rig.d.toggle_layout_grid();
    rig.settle();
    rig.click(rig.centre_of("Lbl-AC-Sub"));
    assert_eq!(rig.d.selected_ids, vec!["Lbl-AC-Sub".to_owned()]);
    assert!(rig.d.layout_grid.selection.is_none());
    println!("Show Grid off: a press on Lbl-AC-Sub selects Lbl-AC-Sub");
}

// ── T13 — the floating toolbar and its actions ───────────────────────────────

/// A responsive form holding Grid container `P` (the given track lists) with the children
/// `(id, column, row, column span, row span)`; 0 leaves a property unwritten.
fn grid_form(columns: &str, rows: &str, kids: &[(&str, i64, i64, i64, i64)]) -> Form {
    let mut f = Form::new("F", "F", 600, 400);
    f.responsive = true;
    let mut p = Control::new("P", ControlType::Panel, 0, 0);
    p.rect = cobolt_forms::model::Rect::new(40, 60, 300, 120);
    for (k, v) in [("LayoutMode", "Grid"), ("GridColumns", columns), ("GridRows", rows), ("BorderStyle", "None")] {
        p.set_prop(k, PropValue::String(v.into()));
    }
    p.set_prop("BorderWidth", PropValue::Int(0));
    f.controls.push(p);
    for (i, (id, col, row, cs, rs)) in kids.iter().enumerate() {
        let mut c = Control::new(*id, ControlType::Button, 0, 0);
        c.rect = cobolt_forms::model::Rect::new(0, 30 * i as i32, 40, 20);
        c.parent = Some("P".into());
        for (key, v) in [("GridColumn", col), ("GridRow", row), ("ColumnSpan", cs), ("RowSpan", rs)] {
            if *v > 0 {
                c.set_prop(key, PropValue::Int(*v));
            }
        }
        f.controls.push(c);
    }
    f
}

/// AC4 (R7, R8, R9) — selecting a cell brings up the toolbar beside the grid: outside it and
/// on the canvas; every button has a tip and every reason a sentence, in all six languages;
/// and Merge is disabled for a single empty cell.
#[test]
fn selecting_a_cell_brings_up_the_toolbar_beside_the_grid_with_tips_in_six_languages() {
    use crate::panels::layout_grid_edit as lg;
    let mut rig = Rig::new(grid_form("1fr 1fr", "1fr 1fr", &[("A", 1, 1, 0, 0)]));
    assert!(rig.d.layout_grid.toolbar.is_none(), "no container selected, no toolbar");
    rig.click(rig.cell_centre("P", 1, 1)); // an empty cell
    let strip = rig.d.layout_grid.toolbar.expect("the toolbar is up");
    let r = rig.d.canvas_layout.as_ref().unwrap().rects["P"];
    let container = Rect::from_min_size(rig.at(r.x, r.y), Vec2::new(r.w, r.h));
    assert!(!strip.intersects(container), "beside the grid, never over it: {strip:?} vs {container:?}");
    assert!(Rect::from_min_size(Pos2::ZERO, SCREEN).contains_rect(strip), "on the canvas");
    // Merge is disabled for a single empty cell; Add is offered.
    let view = rig.d.grid_view("P").unwrap();
    let sel = rig.d.layout_grid.selection.clone();
    assert_eq!(lg::availability(lg::Tool::Merge, &view, sel.as_ref(), None), Err(lg::Why::OneCell));
    assert_eq!(lg::availability(lg::Tool::AddColumn, &view, sel.as_ref(), None), Ok(()));

    // Six languages: every tip and every reason is said in each, with the code names kept.
    let mut texts = 0;
    for &lang in crate::i18n::Language::ALL {
        for t in lg::TOOLS {
            let tip = crate::ui_text::ide_localized(lang, t.tip());
            assert!(!tip.is_empty());
            if lang != crate::i18n::Language::English {
                assert_ne!(tip, t.tip(), "{lang:?}: {:?} is translated", t.tip());
            }
            texts += 1;
        }
        for w in [lg::Why::NoCell, lg::Why::SeveralTracks, lg::Why::Repeat, lg::Why::LastTrack, lg::Why::NothingToRestore, lg::Why::OneCell, lg::Why::NoControl, lg::Why::SeveralControls, lg::Why::NothingToSplit, lg::Why::Scaled] {
            let text = crate::ui_text::ide_localized(lang, w.text());
            if lang != crate::i18n::Language::English {
                assert_ne!(text, w.text(), "{lang:?}: {:?} is translated", w.text());
            }
            if w == lg::Why::Repeat {
                assert!(text.contains("Repeat(…)"), "{lang:?}: the code name is kept");
            }
            texts += 1;
        }
    }
    println!("AC4: toolbar {strip:?} beside the grid {container:?}; Merge disabled for one empty cell; {texts} tips and reasons present in six languages");
}

/// AC5 (R10) — on a 2×2 grid `1fr 1fr` / `1fr 1fr` with a child at (2, 2), Add column after
/// column 1 gives `1fr 1fr 1fr` and the child is at (3, 2), in the same cell as before.
#[test]
fn add_column_through_the_toolbar_renumbers_the_child() {
    use crate::panels::layout_grid_edit::Tool;
    let mut rig = Rig::new(grid_form("1fr 1fr", "1fr 1fr", &[("A", 2, 2, 0, 0)]));
    rig.click(rig.cell_centre("P", 0, 0));
    rig.tool(Tool::AddColumn);
    assert_eq!(rig.prop("P", "GridColumns"), "1fr 1fr 1fr");
    assert_eq!((rig.prop("A", "GridColumn"), rig.prop("A", "GridRow")), ("3".into(), "2".into()));
    assert_eq!(rig.d.undo_stack.len(), 1, "one step");
    assert_eq!(rig.d.selected_ids, vec!["P".to_owned()], "pressing a toolbar button leaves the container selected");
    rig.d.undo_unchecked();
    assert_eq!((rig.prop("P", "GridColumns"), rig.prop("A", "GridColumn")), ("1fr 1fr".into(), "2".into()));
    println!("AC5: `1fr 1fr` → `1fr 1fr 1fr` through the toolbar; A (2,2) → column 3, row 2; one Ctrl+Z puts it back");
}

/// AC6, AC7 (R11–R13) — Remove row 1 of a 2-row grid whose child sits only in it: the track
/// goes, the child stays with `GridRow = 0`, a notice names it, the remaining row fills the
/// container; after an unrelated edit, Restore puts the row and the placement back exactly.
#[test]
fn remove_row_keeps_the_child_names_it_and_restore_brings_it_back() {
    use crate::panels::layout_grid_edit::Tool;
    let mut rig = Rig::new(grid_form("1fr 1fr", "1fr 1fr", &[("A", 1, 1, 0, 0), ("B", 2, 2, 0, 0)]));
    rig.click(rig.cell_centre("P", 0, 0));
    rig.tool(Tool::RemoveRow);
    assert_eq!(rig.prop("P", "GridRows"), "1fr");
    assert_eq!(rig.prop("A", "GridRow"), "0", "A survives, auto-placed on rows");
    assert_eq!(rig.prop("B", "GridRow"), "1");
    assert!(rig.d.form.find_control("A").is_some());
    let notice = rig.d.layout_grid.notice.clone().expect("a notice names the affected controls");
    assert_eq!(notice.controls, vec!["A".to_owned()]);
    let g = crate::panels::layout_grid_edit::GridGeom::of(rig.d.canvas_layout.as_ref().unwrap(), "P").unwrap();
    assert_eq!(g.rows.len(), 1);
    assert!((g.rows[0] - g.client.h).abs() < 0.5, "the remaining row fills the container: {} of {}", g.rows[0], g.client.h);
    assert!(rig.d.layout_grid.selection.is_none(), "the selected cell is gone");

    // An unrelated edit elsewhere, then Restore (the container is selected, as it was).
    rig.d.apply(Cmd::SetProperty { id: "B".into(), key: "BackgroundColor".into(), old: None, new: PropValue::String("#112233".into()) });
    rig.settle();
    rig.tool(Tool::Restore);
    assert_eq!(rig.prop("P", "GridRows"), "1fr 1fr");
    assert_eq!((rig.prop("A", "GridRow"), rig.prop("B", "GridRow")), ("1".into(), "2".into()));
    assert_eq!(rig.prop("B", "BackgroundColor"), "#112233", "the unrelated edit remains");
    println!("AC6/7: row 1 removed (A auto, notice {:?}, row {} px = client {}); restored with the unrelated edit kept", notice.controls, g.rows[0], g.client.h);
}

/// AC8 (R14) — with `GridColumns` = `Repeat(3, 1fr)`, the column buttons are disabled with
/// the reason for it, and pressing them changes nothing.
#[test]
fn a_repeat_axis_disables_its_buttons_and_pressing_them_does_nothing() {
    use crate::panels::layout_grid_edit as lg;
    let mut rig = Rig::new(grid_form("Repeat(3, 1fr)", "1fr 1fr", &[("A", 1, 1, 0, 0)]));
    rig.click(rig.cell_centre("P", 0, 0));
    let view = rig.d.grid_view("P").unwrap();
    let sel = rig.d.layout_grid.selection.clone();
    for t in [lg::Tool::AddColumn, lg::Tool::RemoveColumn] {
        assert_eq!(lg::availability(t, &view, sel.as_ref(), None), Err(lg::Why::Repeat), "{t:?}");
    }
    assert!(lg::availability(lg::Tool::Restore, &view, sel.as_ref(), None).is_err(), "restore: nothing to restore, with its reason");
    assert_eq!(lg::availability(lg::Tool::AddRow, &view, sel.as_ref(), None), Ok(()), "the rows are fine");
    let saved = cobolt_forms::form_to_string(&rig.d.form).unwrap();
    rig.tool(lg::Tool::AddColumn);
    rig.tool(lg::Tool::RemoveColumn);
    assert_eq!(cobolt_forms::form_to_string(&rig.d.form).unwrap(), saved, "a disabled button does nothing");
    assert!(rig.d.undo_stack.is_empty());
    println!("AC8: `Repeat(3, 1fr)` — column add/remove disabled with the Repeat reason; pressing them changed 0 of {} bytes", saved.len());
}

/// AC9 (R15–R17) — a 2×1 range holding one control: Merge gives it `ColumnSpan = 2`; Split
/// returns it to 1; a range holding two controls has Merge disabled.
#[test]
fn merge_and_split_through_the_toolbar() {
    use crate::panels::layout_grid_edit as lg;
    let mut rig = Rig::new(grid_form("1fr 1fr 1fr", "1fr 1fr", &[("A", 1, 1, 0, 0), ("B", 3, 1, 0, 0)]));
    // A is in column 1; the range is columns 1–2 of row 1 — one control.
    rig.drag(rig.cell_centre("P", 0, 0), rig.cell_centre("P", 0, 1));
    let r = rig.d.layout_grid.selection.as_ref().unwrap().range();
    assert_eq!((r.rows, r.columns), (1, 2));
    rig.tool(lg::Tool::Merge);
    assert_eq!((rig.prop("A", "ColumnSpan"), rig.prop("A", "GridColumn")), ("2".into(), "1".into()));
    // Split from a cell A covers.
    rig.click(rig.cell_centre("P", 0, 1));
    rig.tool(lg::Tool::Split);
    assert_eq!(rig.prop("A", "ColumnSpan"), "1");
    // A range holding two controls: Merge is disabled and does nothing.
    rig.drag(rig.cell_centre("P", 0, 0), rig.cell_centre("P", 0, 2));
    let view = rig.d.grid_view("P").unwrap();
    let sel = rig.d.layout_grid.selection.clone();
    assert_eq!(lg::availability(lg::Tool::Merge, &view, sel.as_ref(), None), Err(lg::Why::SeveralControls));
    let steps = rig.d.undo_stack.len();
    rig.tool(lg::Tool::Merge);
    assert_eq!(rig.d.undo_stack.len(), steps, "a disabled Merge makes no step");
    println!("AC9: merge 2×1 → ColumnSpan 2; split → 1; a range with two controls has Merge disabled");
}

/// R11 — Remove acts on the selected cell's own row or column, not on the first: with the
/// cell at row 2, column 1 selected, Remove row takes row 2 and Remove column takes column 1.
#[test]
fn remove_takes_the_selected_cells_own_row_and_column() {
    use crate::panels::layout_grid_edit::Tool;
    let mut rig = Rig::new(grid_form("1fr 2fr 3fr", "10px 20px 30px", &[("A", 3, 3, 0, 0)]));
    rig.click(rig.cell_centre("P", 1, 0)); // row 2, column 1
    rig.tool(Tool::RemoveRow);
    assert_eq!((rig.prop("P", "GridRows"), rig.prop("P", "GridColumns")), ("10px 30px".into(), "1fr 2fr 3fr".into()), "row 2 went, no column did");
    assert_eq!(rig.prop("A", "GridRow"), "2", "A moved up with its row");
    rig.click(rig.cell_centre("P", 0, 2)); // row 1, column 3
    rig.tool(Tool::RemoveColumn);
    assert_eq!(rig.prop("P", "GridColumns"), "1fr 2fr", "column 3 went");
    assert_eq!(rig.prop("P", "GridRows"), "10px 30px");
    println!("remove: the cell at row 2 takes row 2 (`10px 20px 30px` → `10px 30px`); the cell at column 3 takes column 3");
}

// ── T14 — dragging a track line ──────────────────────────────────────────────

/// Drag the first column line of `P` by `dx` and answer the `GridColumns` it leaves.
fn drag_first_column_line(rig: &mut Rig, dx: f32) -> String {
    use cobolt_forms::layout::grid_edit::Axis;
    let from = rig.line_point("P", Axis::Column, 0);
    rig.drag(from, from + Vec2::new(dx, 0.0));
    rig.prop("P", "GridColumns")
}

/// AC10 (R19, R20) — through a real drag: `1fr 1fr` to the 25 % mark is `0.5fr 1.5fr` (the
/// total, 2, kept); `100px 200px` + 50 is `150px 150px`; an `Auto` track dragged is `Npx`.
#[test]
fn dragging_a_line_writes_each_track_in_its_own_unit() {
    // The two columns share the container's client (its width less the panel's inset): the
    // 25 % mark of the pair is half the first column to the left of the middle.
    let mut rig = Rig::new(grid_form("1fr 1fr", "1fr", &[]));
    let before = cobolt_forms::form_to_string(&rig.d.form).unwrap();
    let steps = rig.d.undo_stack.len();
    let first = crate::panels::layout_grid_edit::GridGeom::of(rig.d.canvas_layout.as_ref().unwrap(), "P").unwrap().columns[0];
    assert_eq!(first.fract(), 0.0, "a whole number of pixels, so the drag lands exactly: {first}");
    let t = drag_first_column_line(&mut rig, -(first / 2.0));
    assert_eq!(t, "0.5fr 1.5fr");
    assert_eq!(rig.d.undo_stack.len(), steps + 1, "one undo step for the whole drag");
    println!("AC10: `1fr 1fr` ({first} + {first} px) dragged {} to the left → `{t}` in one step", first / 2.0);

    rig.d.undo_unchecked();
    assert_eq!(cobolt_forms::form_to_string(&rig.d.form).unwrap(), before, "one Ctrl+Z returns the form byte for byte");

    let mut rig = Rig::new(grid_form("100px 200px", "1fr", &[]));
    let t = drag_first_column_line(&mut rig, 50.0);
    assert_eq!(t, "150px 150px");
    println!("AC10: `100px 200px` dragged 50 → `{t}`");

    // An Auto track holding a 40-px control, dragged 50: a 90-px track.
    let mut rig = Rig::new(grid_form("Auto 1fr", "1fr", &[("A", 1, 1, 0, 0)]));
    let t = drag_first_column_line(&mut rig, 50.0);
    assert_eq!(t, "90px 1fr", "Auto became pixels of the dragged size; the flexible track kept as typed");
    println!("AC10: `Auto 1fr` (Auto = 40) dragged 50 → `{t}`");
}

/// AC11 (R21) — no drag makes a track narrower than 8 px, in either direction.
#[test]
fn no_drag_makes_a_track_narrower_than_eight_pixels() {
    let mut rig = Rig::new(grid_form("100px 100px", "1fr", &[]));
    assert_eq!(drag_first_column_line(&mut rig, -500.0), "8px 192px");
    let g = crate::panels::layout_grid_edit::GridGeom::of(rig.d.canvas_layout.as_ref().unwrap(), "P").unwrap();
    assert!(g.columns.iter().all(|w| *w >= 8.0 - 0.01), "{:?}", g.columns);
    let mut rig = Rig::new(grid_form("100px 100px", "1fr", &[]));
    assert_eq!(drag_first_column_line(&mut rig, 500.0), "192px 8px");
    let g = crate::panels::layout_grid_edit::GridGeom::of(rig.d.canvas_layout.as_ref().unwrap(), "P").unwrap();
    assert!(g.columns.iter().all(|w| *w >= 8.0 - 0.01), "{:?}", g.columns);
    println!("AC11: `100px 100px` dragged −500 → `8px 192px`, +500 → `192px 8px`; every track ≥ 8 px");
}

/// R19 — the tracks are written live, so the canvas lays out under the pointer; the undo
/// step comes only at the release. R18 — the cursor says "resize" over a draggable line, and
/// not over a `MinMax` or `Repeat` one, nor off the line.
#[test]
fn the_drag_is_live_and_the_cursor_says_resize_only_where_a_line_can_be_dragged() {
    use cobolt_forms::layout::grid_edit::Axis;
    let mut rig = Rig::new(grid_form("1fr 1fr", "1fr 1fr", &[]));
    let line = rig.line_point("P", Axis::Column, 0);
    rig.frame(vec![Event::PointerMoved(line)]);
    assert_eq!(rig.cursor, egui::CursorIcon::ResizeColumn, "over a column line");
    let row_line = rig.line_point("P", Axis::Row, 0);
    rig.frame(vec![Event::PointerMoved(row_line)]);
    assert_eq!(rig.cursor, egui::CursorIcon::ResizeRow, "over a row line");
    rig.frame(vec![Event::PointerMoved(rig.cell_centre("P", 0, 0))]);
    assert_ne!(rig.cursor, egui::CursorIcon::ResizeColumn, "off the line");

    // Live: mid-drag the text has changed and no step has been made.
    let steps = rig.d.undo_stack.len();
    rig.frame(vec![Event::PointerMoved(line)]);
    rig.frame(vec![press(line, true)]);
    for i in 1..=4 {
        rig.frame(vec![Event::PointerMoved(line + Vec2::new(-10.0 * i as f32, 0.0))]);
    }
    let live = rig.prop("P", "GridColumns");
    assert_eq!(live, "0.73fr 1.27fr", "written live, mid-drag: 40 px of the pair's 296 (148 | 148 → 108 | 188)");
    assert_eq!(rig.d.undo_stack.len(), steps, "no step until the release");
    rig.frame(vec![press(line + Vec2::new(-40.0, 0.0), false)]);
    assert_eq!(rig.d.undo_stack.len(), steps + 1);

    // What cannot be dragged shows no cursor, and a press there does not drag.
    for cols in ["MinMax(50px, 1fr) 1fr", "Repeat(2, 1fr)"] {
        let mut rig = Rig::new(grid_form(cols, "1fr", &[]));
        let line = rig.line_point("P", Axis::Column, 0);
        rig.frame(vec![Event::PointerMoved(line)]);
        assert_ne!(rig.cursor, egui::CursorIcon::ResizeColumn, "{cols}");
        let before = rig.prop("P", "GridColumns");
        rig.drag(line, line + Vec2::new(30.0, 0.0));
        assert_eq!(rig.prop("P", "GridColumns"), before, "{cols}: nothing dragged");
    }
    println!("drag: cursor ResizeColumn/ResizeRow over a line, none off it or over MinMax/Repeat; live write (`{live}`) mid-drag with 0 steps, 1 step at the release");
}

/// R20 — a line on the grid's outer border is not a line: a press there is an ordinary
/// press (here, a cell's).
#[test]
fn the_outer_border_is_not_a_line() {
    let mut rig = Rig::new(grid_form("1fr 1fr", "1fr 1fr", &[]));
    let g = crate::panels::layout_grid_edit::GridGeom::of(rig.d.canvas_layout.as_ref().unwrap(), "P").unwrap();
    let b = g.bounds();
    let edge = rig.at(b.x + 1.25, b.y + b.h / 2.0);
    rig.frame(vec![Event::PointerMoved(edge)]);
    assert_ne!(rig.cursor, egui::CursorIcon::ResizeColumn);
    let before = rig.prop("P", "GridColumns");
    rig.drag(edge, edge + Vec2::new(20.0, 0.0));
    assert_eq!(rig.prop("P", "GridColumns"), before);
    println!("the grid's left border: no resize cursor, nothing dragged");
}

// ── T15 — selecting a control shows its grid (amendment A1) ──────────────────

/// AC15 (R27, R30, R31) — with Show Grid off, selecting `Lbl-AC-Sub` draws the lines of
/// `Crd-AC` — the innermost grid, though `Crd-AC` sits in a grid of its own — and no other
/// grid, with the label's cell highlighted; selecting a control outside any grid hides them;
/// moving the selection to a control in another grid moves the lines to it.
#[test]
fn selecting_a_control_shows_the_grid_it_sits_in_and_no_other() {
    let mut form = power_spatial();
    // A control that sits in no grid: a button on the form itself.
    let mut loose = Control::new("Loose-1", ControlType::Button, 0, 0);
    loose.rect = cobolt_forms::model::Rect::new(900, 10, 80, 24);
    form.controls.push(loose);
    let mut rig = Rig::new(form);
    rig.d.toggle_layout_grid();
    rig.settle();
    assert!(rig.d.grid_overlay_ids.is_empty(), "nothing selected, Show Grid off: no lines");

    rig.click(rig.centre_of("Lbl-AC-Sub"));
    assert_eq!(rig.d.selected_ids, vec!["Lbl-AC-Sub".to_owned()]);
    assert_eq!(rig.d.grid_overlay_ids, vec!["Crd-AC".to_owned()], "the label's own grid, and no other");
    let (grid, cell) = rig.d.grid_overlay_cell.clone().expect("the label's cell is highlighted");
    assert_eq!(grid, "Crd-AC");
    assert_eq!((cell.row, cell.column, cell.rows, cell.columns), (1, 0, 1, 1), "column 1, row 2");
    // Crd-AC is itself in a grid: the outer one is not shown (R31).
    let parent = rig.d.form.find_control("Crd-AC").unwrap().parent.clone().unwrap();
    let outer = rig.d.canvas_layout.as_ref().unwrap().containers.get(&parent).map(|g| g.mode);
    assert_eq!(outer, Some(cobolt_forms::layout::LayoutMode::Grid), "{parent} is a grid too");
    assert!(!rig.d.grid_overlay_ids.contains(&parent));

    // A control in another grid: that grid, alone.
    let other = rig
        .d
        .form
        .controls
        .iter()
        .find(|c| c.id != "Lbl-AC-Sub" && !c.control_type.is_non_visual() && c.parent.as_deref().is_some_and(|p| p != "Crd-AC" && rig.d.grid_view(p).is_some()) && rig.d.grid_view(&c.id).is_none())
        .map(|c| (c.id.clone(), c.parent.clone().unwrap()))
        .expect("another control in another grid");
    rig.d.select_only(&other.0);
    rig.settle();
    assert_eq!(rig.d.grid_overlay_ids, vec![other.1.clone()], "the lines moved with the selection");

    // A control outside any grid: none.
    let loose = "Loose-1".to_owned();
    rig.d.select_only(&loose);
    rig.settle();
    assert!(rig.d.grid_overlay_ids.is_empty() && rig.d.grid_overlay_cell.is_none(), "{loose}: no lines");
    rig.d.select_only("Lbl-AC-Sub");
    rig.settle();
    rig.d.set_selected_one(None);
    rig.settle();
    assert!(rig.d.grid_overlay_ids.is_empty(), "cleared: hidden");
    // With Show Grid on they are all drawn (R3), so the selected control adds nothing.
    rig.d.toggle_layout_grid();
    rig.d.select_only("Lbl-AC-Sub");
    rig.settle();
    assert!(rig.d.grid_overlay_ids.len() > 5 && rig.d.grid_overlay_cell.is_none());
    println!("AC15: Lbl-AC-Sub → Crd-AC's lines only, its cell (row 2, column 1) lit; {} → {}; {loose} → none; Show Grid on → all {}", other.0, other.1, rig.d.grid_overlay_ids.len());
}

/// AC16 (R28, R29) — with the label selected, dragging the line between two of `Crd-AC`'s
/// columns resizes those columns (units kept, one undo step); the label stays selected and
/// its own size is untouched; and a press on the label away from the line is the ordinary one:
/// it moves the label to another cell.
#[test]
fn the_selected_controls_grid_can_be_dragged_and_the_control_keeps_its_own_gestures() {
    use cobolt_forms::layout::grid_edit::Axis;
    let mut rig = Rig::new(power_spatial());
    rig.d.toggle_layout_grid();
    rig.settle();
    rig.click(rig.centre_of("Lbl-AC-Sub"));
    assert_eq!(rig.d.selected_ids, vec!["Lbl-AC-Sub".to_owned()]);
    let rect = rig.d.form.find_control("Lbl-AC-Sub").unwrap().rect;
    let before = rig.prop("Crd-AC", "GridColumns");
    assert_eq!(before, "1fr 1fr");
    let saved = cobolt_forms::form_to_string(&rig.d.form).unwrap();
    let steps = rig.d.undo_stack.len();

    let from = rig.line_point("Crd-AC", Axis::Column, 0);
    rig.frame(vec![Event::PointerMoved(from)]);
    assert_eq!(rig.cursor, egui::CursorIcon::ResizeColumn, "the line the selection shows can be taken");
    rig.drag(from, from + Vec2::new(-40.0, 0.0));
    let after = rig.prop("Crd-AC", "GridColumns");
    assert_ne!(after, before);
    assert!(after.ends_with("fr") && after.split(' ').count() == 2, "units kept: {after}");
    assert_eq!(rig.d.selected_ids, vec!["Lbl-AC-Sub".to_owned()], "the label stays selected");
    assert_eq!(rig.d.form.find_control("Lbl-AC-Sub").unwrap().rect, rect, "and its own size is untouched");
    assert_eq!(rig.d.undo_stack.len(), steps + 1, "one step");
    rig.d.undo_unchecked();
    assert_eq!(cobolt_forms::form_to_string(&rig.d.form).unwrap(), saved);

    // A press on the label, off the line: the ordinary gesture — a move to the other cell.
    assert_eq!(rig.prop("Lbl-AC-Sub", "GridColumn"), "1");
    let label = rig.point_in("Lbl-AC-Sub", 0.3, 0.5);
    let target = rig.cell_centre("Crd-AC", 1, 1);
    rig.drag(label, target);
    assert_eq!(rig.d.selected_ids, vec!["Lbl-AC-Sub".to_owned()]);
    assert_eq!(rig.prop("Lbl-AC-Sub", "GridColumn"), "2", "the label was moved to the other column by its own gesture");
    println!("AC16: `{before}` dragged → `{after}` (one step, undone exactly); the label stayed selected at {rect:?}; a press on it moved it to column 2");
}


// ── T16 — resizing a control that sits in a grid resizes its cell ────────────

/// Select `Lbl-AC-Sub` with Show Grid off, the way a developer does: a press on it.
fn label_selected() -> Rig {
    let mut rig = Rig::new(power_spatial());
    rig.d.toggle_layout_grid();
    rig.settle();
    rig.click(rig.point_in("Lbl-AC-Sub", 0.3, 0.5));
    assert_eq!(rig.d.selected_ids, vec!["Lbl-AC-Sub".to_owned()]);
    rig
}

/// AC12 (R23, R24) — widening `Lbl-AC-Sub` by its right edge changes `Crd-AC`'s
/// `GridColumns`, and the label's own width in the saved `.cfrm` is unchanged — nor is any
/// other property of the label's. One Ctrl+Z puts the tracks back exactly.
#[test]
fn widening_a_control_in_a_grid_widens_its_column_and_not_the_control() {
    let mut rig = label_selected();
    let saved = cobolt_forms::form_to_string(&rig.d.form).unwrap();
    let label_xml = |rig: &Rig| {
        let xml = cobolt_forms::form_to_string(&rig.d.form).unwrap();
        let at = xml.find("id=\"Lbl-AC-Sub\"").unwrap();
        xml[at..at + xml[at..].find("</Control>").unwrap()].to_owned()
    };
    let label_before = label_xml(&rig);
    let steps = rig.d.undo_stack.len();
    let (cols, rows) = (rig.prop("Crd-AC", "GridColumns"), rig.prop("Crd-AC", "GridRows"));

    let h = rig.handle_point("Lbl-AC-Sub", Handle::Right);
    rig.drag(h, h + Vec2::new(30.0, 0.0));
    let widened = rig.prop("Crd-AC", "GridColumns");
    assert_ne!(widened, cols, "the column was resized");
    assert!(widened.ends_with("fr") && widened.split(' ').count() == 2, "units kept: {widened}");
    assert_eq!(rig.prop("Crd-AC", "GridRows"), rows, "rows untouched");
    assert_eq!(label_xml(&rig), label_before, "the label's own properties — width included — are as saved");
    assert_eq!(rig.d.undo_stack.len(), steps + 1, "one step");
    rig.d.undo_unchecked();
    assert_eq!(cobolt_forms::form_to_string(&rig.d.form).unwrap(), saved, "one Ctrl+Z returns every byte");
    println!("AC12: Right handle dragged 30 → Crd-AC GridColumns `{cols}` → `{widened}`; the label's {} bytes of XML unchanged; one Ctrl+Z", label_before.len());
}

/// R23 — each edge is the line it stands on: bottom is the line after the control's last row,
/// top the one before its first; a corner moves one line each way, in one step; an edge on
/// the container's own border is no line and moves nothing; the control's size is never written.
#[test]
fn every_edge_and_corner_moves_the_line_it_stands_on() {
    let mut rig = label_selected();
    let rect = rig.d.form.find_control("Lbl-AC-Sub").unwrap().rect;
    let tracks = |rig: &Rig| (rig.prop("Crd-AC", "GridColumns"), rig.prop("Crd-AC", "GridRows"));
    let (c0, r0) = tracks(&rig);
    assert_eq!(r0, "30px 14px 1fr 46px 28px");
    let steps = rig.d.undo_stack.len();

    // Bottom: the label is in row 2 (`14px`); its bottom edge is the line after it.
    let h = rig.handle_point("Lbl-AC-Sub", Handle::Bot);
    rig.drag(h, h + Vec2::new(0.0, 10.0));
    let (c, r) = tracks(&rig);
    assert_eq!(c, c0);
    assert!(r.starts_with("30px 24px "), "row 2 grew by 10: {r}");
    // Top: the line before the label's row, between `30px` and what is now `24px`.
    let h = rig.handle_point("Lbl-AC-Sub", Handle::Top);
    rig.drag(h, h + Vec2::new(0.0, -6.0));
    let (_, r) = tracks(&rig);
    assert!(r.starts_with("24px 30px "), "dragged up: row 1 shrank 6, row 2 grew 6: {r}");
    assert_eq!(rig.d.undo_stack.len(), steps + 2, "one step each");

    // Left, on the first column: the container's border — nothing moves, no step is made.
    let before = tracks(&rig);
    let steps = rig.d.undo_stack.len();
    let h = rig.handle_point("Lbl-AC-Sub", Handle::Left);
    rig.drag(h, h + Vec2::new(-25.0, 0.0));
    assert_eq!(tracks(&rig), before, "the left edge is on the border");
    assert_eq!(rig.d.undo_stack.len(), steps);

    // A corner: both lines, one step.
    let h = rig.handle_point("Lbl-AC-Sub", Handle::BotRight);
    rig.drag(h, h + Vec2::new(20.0, 8.0));
    let (c, r) = tracks(&rig);
    assert_ne!(c, before.0, "the column line moved");
    assert_ne!(r, before.1, "the row line moved");
    assert_eq!(rig.d.undo_stack.len(), steps + 1, "both in one step");
    assert_eq!(rig.d.form.find_control("Lbl-AC-Sub").unwrap().rect, rect, "the label's own rect: never written (R24)");
    println!("edges: bottom +10 → row 2 `24px`; top −6 → rows `24px 30px`; left on the border → no change, no step; corner → both lines in one step; rect {rect:?} never written");
}

/// A control that is not in a Grid container is resized as it always was.
#[test]
fn a_control_outside_a_grid_still_resizes_itself() {
    let mut form = power_spatial();
    let mut loose = Control::new("Loose-1", ControlType::Button, 0, 0);
    loose.rect = cobolt_forms::model::Rect::new(900, 10, 80, 24);
    form.controls.push(loose);
    let mut rig = Rig::new(form);
    rig.d.toggle_layout_grid();
    rig.settle();
    rig.d.select_only("Loose-1");
    rig.settle();
    let h = rig.handle_point("Loose-1", Handle::Right);
    rig.drag(h, h + Vec2::new(24.0, 0.0));
    assert!(rig.d.form.find_control("Loose-1").unwrap().rect.w > 80, "its own width grew: {:?}", rig.d.form.find_control("Loose-1").unwrap().rect);
    println!("Loose-1 (no grid): Right handle +24 → width {}", rig.d.form.find_control("Loose-1").unwrap().rect.w);
}

// ── T17 — every gesture is one undo step ─────────────────────────────────────

/// AC13 (R25) — each gesture the spec defines, performed through the canvas and the toolbar,
/// is exactly one history step: one Ctrl+Z returns the form byte for byte, one Ctrl+Y makes the
/// gesture again, byte for byte.
#[test]
fn every_gesture_is_one_undo_step_and_one_redo() {
    use crate::panels::layout_grid_edit::Tool;
    type Build = Box<dyn Fn() -> Rig>;
    type Gesture = Box<dyn Fn(&mut Rig)>;
    let two_by_two = || Rig::new(grid_form("1fr 1fr", "1fr 1fr", &[("A", 1, 1, 0, 0), ("B", 2, 2, 0, 0)]));
    let in_a_cell = |rig: &mut Rig| rig.click(rig.cell_centre("P", 0, 0));

    let table: Vec<(&str, Build, Gesture)> = vec![
        ("add column", Box::new(move || { let mut r = two_by_two(); r.click(r.cell_centre("P", 0, 0)); r }), Box::new(|r| r.tool(Tool::AddColumn))),
        ("add row", Box::new(move || { let mut r = two_by_two(); r.click(r.cell_centre("P", 0, 0)); r }), Box::new(|r| r.tool(Tool::AddRow))),
        ("remove column", Box::new(move || { let mut r = two_by_two(); r.click(r.cell_centre("P", 0, 0)); r }), Box::new(|r| r.tool(Tool::RemoveColumn))),
        ("remove row", Box::new(move || { let mut r = two_by_two(); r.click(r.cell_centre("P", 0, 0)); r }), Box::new(|r| r.tool(Tool::RemoveRow))),
        (
            "restore",
            Box::new(move || {
                let mut r = two_by_two();
                in_a_cell(&mut r);
                r.tool(Tool::RemoveRow);
                r
            }),
            Box::new(|r| r.tool(Tool::Restore)),
        ),
        (
            "merge",
            Box::new(|| {
                let mut r = Rig::new(grid_form("1fr 1fr 1fr", "1fr 1fr", &[("A", 1, 1, 0, 0), ("B", 3, 1, 0, 0)]));
                r.drag(r.cell_centre("P", 0, 0), r.cell_centre("P", 0, 1));
                r
            }),
            Box::new(|r| r.tool(Tool::Merge)),
        ),
        (
            "split",
            Box::new(|| {
                let mut r = Rig::new(grid_form("1fr 1fr 1fr", "1fr 1fr", &[("A", 1, 1, 2, 0)]));
                r.click(r.cell_centre("P", 0, 1));
                r
            }),
            Box::new(|r| r.tool(Tool::Split)),
        ),
        (
            "drag a column line",
            Box::new(|| Rig::new(grid_form("1fr 1fr", "1fr 1fr", &[]))),
            Box::new(|r| {
                let from = r.line_point("P", Axis::Column, 0);
                r.drag(from, from + Vec2::new(20.0, 0.0));
            }),
        ),
        (
            "drag a row line",
            Box::new(|| Rig::new(grid_form("1fr 1fr", "1fr 1fr", &[]))),
            Box::new(|r| {
                let from = r.line_point("P", Axis::Row, 0);
                r.drag(from, from + Vec2::new(0.0, 10.0));
            }),
        ),
        (
            "resize a control's cell by an edge",
            Box::new(label_selected),
            Box::new(|r| {
                let h = r.handle_point("Lbl-AC-Sub", Handle::Right);
                r.drag(h, h + Vec2::new(30.0, 0.0));
            }),
        ),
        (
            // A control that fills a roomy cell: its corner knob is clear of the edge knobs.
            "resize a control's cell by a corner (two lines)",
            Box::new(|| {
                let mut r = Rig::new(grid_form("1fr 1fr", "1fr 1fr", &[("A", 1, 1, 0, 0)]));
                r.d.toggle_layout_grid();
                r.settle();
                r.click(r.point_in("A", 0.3, 0.5));
                assert_eq!(r.d.selected_ids, vec!["A".to_owned()]);
                r
            }),
            Box::new(|r| {
                let (c, rows) = (r.prop("P", "GridColumns"), r.prop("P", "GridRows"));
                let h = r.handle_point("A", Handle::BotRight);
                r.drag(h, h + Vec2::new(20.0, 8.0));
                assert_ne!(r.prop("P", "GridColumns"), c, "the column line moved");
                assert_ne!(r.prop("P", "GridRows"), rows, "the row line moved");
            }),
        ),
    ];

    let mut report = Vec::new();
    for (name, build, gesture) in &table {
        let mut rig = build();
        let bytes = |rig: &Rig| cobolt_forms::form_to_string(&rig.d.form).unwrap();
        let (before, steps) = (bytes(&rig), rig.d.undo_stack.len());
        gesture(&mut rig);
        let after = bytes(&rig);
        assert_ne!(after, before, "{name}: the gesture changed the form");
        assert_eq!(rig.d.undo_stack.len(), steps + 1, "{name}: one history step");
        rig.d.undo();
        assert_eq!(rig.d.undo_stack.len(), steps, "{name}: one Ctrl+Z took it off");
        assert!(rig.d.pending_history_confirm.is_none(), "{name}: no question about COBOL procedures");
        assert_eq!(bytes(&rig), before, "{name}: one Ctrl+Z returns the form byte for byte");
        rig.d.redo();
        assert_eq!(bytes(&rig), after, "{name}: one Ctrl+Y makes it again, byte for byte");
        report.push(format!("{name} (+{} bytes of change)", after.len().abs_diff(before.len())));
    }
    println!("AC13: {} gestures, each one step, undone exactly and redone exactly: {}", table.len(), report.join("; "));
}

/// The stroke colours of every line segment a frame painted — the grid's dashed lines among
/// them.
fn segment_inks(shapes: &[egui::epaint::ClippedShape]) -> Vec<egui::Color32> {
    fn walk(s: &egui::Shape, out: &mut Vec<egui::Color32>) {
        match s {
            egui::Shape::LineSegment { stroke, .. } => out.push(stroke.color),
            egui::Shape::Vec(v) => v.iter().for_each(|s| walk(s, out)),
            _ => {}
        }
    }
    let mut out = Vec::new();
    shapes.iter().for_each(|c| walk(&c.shape, &mut out));
    out
}

/// Every text a frame painted.
fn painted_texts(shapes: &[egui::epaint::ClippedShape]) -> Vec<String> {
    fn walk(s: &egui::Shape, out: &mut Vec<String>) {
        match s {
            egui::Shape::Text(t) => out.push(t.galley.text().to_owned()),
            egui::Shape::Vec(v) => v.iter().for_each(|s| walk(s, out)),
            _ => {}
        }
    }
    let mut out = Vec::new();
    shapes.iter().for_each(|c| walk(&c.shape, &mut out));
    out
}

/// Spec 088 A3 (operator, 2026-10-17: "add a color selection button to change the color of
/// the grid (only applied during the designer, never in the preview or the runtime)"). The
/// toolbar's last cell is a swatch of the lines' colour:
/// - the lines are drawn in the colour the IDE hands in, else the theme's accent;
/// - a click opens the IDE's colour picker, and a right-click goes back to the theme's;
/// - none of it touches the form: not a byte of the `.cfrm`, not an undo step.
#[test]
fn the_toolbar_swatch_colours_the_grid_lines_and_never_the_form() {
    use crate::panels::layout_grid_edit as lg;
    let mut rig = Rig::new(grid_form("1fr 1fr", "1fr 1fr", &[("A", 1, 1, 0, 0)]));
    let saved = cobolt_forms::form_to_string(&rig.d.form).unwrap();
    rig.click(rig.cell_centre("P", 1, 1));
    let strip = rig.d.layout_grid.toolbar.expect("the toolbar is up");
    let swatch = lg::swatch_rect(strip.min);
    assert!(strip.contains_rect(swatch), "the swatch is inside the strip");
    let last_tool = strip.min.x + lg::TOOL_PAD + 7.0 * lg::TOOL_SIZE + 6.0 * lg::TOOL_GAP;
    assert!(swatch.min.x > last_tool, "after the seven tools, apart from them");

    // The theme's accent until a colour is chosen; the IDE's colour once one is.
    let dash = |c: egui::Color32| c.gamma_multiply(0.8);
    let accent = crate::theme::active().accent;
    rig.frame(vec![]);
    assert!(segment_inks(&rig.shapes).contains(&dash(accent)), "the theme's accent by default");
    let orange = egui::Color32::from_rgb(255, 136, 0);
    rig.d.layout_grid.color = Some(orange);
    rig.frame(vec![]);
    let inks = segment_inks(&rig.shapes);
    assert!(inks.contains(&dash(orange)), "the chosen colour");
    assert!(!inks.contains(&dash(accent)), "and the accent no longer");

    // A click opens the picker — its `#` hex field — and the form is untouched.
    let hashes = |rig: &Rig| painted_texts(&rig.shapes).iter().filter(|t| t.as_str() == "#").count();
    assert_eq!(hashes(&rig), 0, "no picker before the click");
    rig.click(swatch.center());
    rig.settle();
    assert!(hashes(&rig) > 0, "the colour picker is open");
    assert_eq!(rig.d.layout_grid.color, Some(orange), "opening it changes nothing");
    rig.key(egui::Key::Escape);
    assert_eq!(hashes(&rig), 0, "Esc closes it");

    // A right-click on the swatch: back to the theme's accent.
    let at = swatch.center();
    let secondary = |pressed| Event::PointerButton { pos: at, button: PointerButton::Secondary, pressed, modifiers: egui::Modifiers::NONE };
    rig.frame(vec![Event::PointerMoved(at)]);
    rig.frame(vec![secondary(true)]);
    rig.frame(vec![secondary(false)]);
    rig.settle();
    assert_eq!(rig.d.layout_grid.color, None, "right-click: the theme's colour");
    assert!(segment_inks(&rig.shapes).contains(&dash(accent)));

    assert_eq!(cobolt_forms::form_to_string(&rig.d.form).unwrap(), saved, "the .cfrm is not touched");
    assert!(rig.d.undo_stack.is_empty(), "a view colour is not an undo step");
    assert!(!rig.d.dirty, "nor an unsaved change");
    println!("A3: lines in the accent → orange → the accent again; picker opened and closed; form unchanged");
}
