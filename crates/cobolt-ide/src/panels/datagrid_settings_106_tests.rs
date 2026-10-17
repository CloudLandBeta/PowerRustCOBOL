// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Spec 106 — the *Edit DataGrid settings* window, driven through real frames of the
//! real properties pane: its size holds (AC1), and **+**, **−** (with its question),
//! **▲**/**▼** and a drag in the column list each send the canvas edit mode's own edit as
//! one step (AC2, AC3).
//!
//! A child module of `properties`, so it reads the pane's private state.

use super::*;
use crate::panels::datagrid_settings as dgs;
use cobolt_forms::model::DataGridColumn;
use egui::{Event, PointerButton, Pos2};

const SCREEN: egui::Vec2 = egui::Vec2::new(1600.0, 1100.0);

/// A grid of three columns over two rows: a Checkbox column (the most details), a
/// plain one, and a Button column with a caption.
fn grid() -> (Form, Control) {
    let mut form = Form::new("ACTORS", "Actors", 900, 600);
    let mut g = Control::new("DG", ControlType::DataGrid, 0, 0);
    g.set_prop("Columns", PropValue::String("ACTIVE:string\nACTOR-NAME:string\nEDIT:string".into()));
    g.set_prop("Rows", PropValue::String("Y\tAnn\t\nN\tBob\t".into()));
    let col = |id: &str, kind: &str| DataGridColumn {
        id: id.into(),
        title: id.into(),
        source_name: id.into(),
        edit_control: kind.into(),
        ..Default::default()
    };
    let mut button = col("EDIT", "Button");
    button.button_caption = "Edit".into();
    let advanced = DataGridAdvanced {
        columns: vec![col("ACTIVE", "Checkbox"), col("ACTOR-NAME", "Textbox"), button],
        ..Default::default()
    };
    g.set_prop(DATAGRID_ADVANCED_PROP, PropValue::String(advanced.to_json().unwrap()));
    form.add_control(g.clone());
    (form, g)
}

fn press(at: Pos2, pressed: bool) -> Event {
    Event::PointerButton { pos: at, button: PointerButton::Primary, pressed, modifiers: egui::Modifiers::NONE }
}

/// The pane with the settings window open on the grid, one real frame at a time.
struct Rig {
    ctx: egui::Context,
    panel: PropertiesPanel,
    form: Form,
    grid: Control,
    lang: crate::i18n::Language,
    t: f64,
    /// Every text the last frame painted, and where.
    painted: Vec<(String, Rect)>,
}

impl Rig {
    fn new(lang: crate::i18n::Language) -> Self {
        let (form, grid) = grid();
        let mut panel = PropertiesPanel::new();
        panel.datagrid_editor = Some(grid.id.clone());
        let ctx = egui::Context::default();
        crate::i18n::set_language(&ctx, lang);
        Self { ctx, panel, form, grid, lang, t: 0.0, painted: Vec::new() }
    }

    fn frame(&mut self, events: Vec<Event>) -> InspectorAction {
        let tr = self.lang.tr();
        self.t += 0.05;
        let input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(Pos2::ZERO, SCREEN)),
            time: Some(self.t),
            events,
            focused: true,
            ..Default::default()
        };
        let mut action = InspectorAction::default();
        let (panel, form, grid) = (&mut self.panel, &self.form, &self.grid);
        // The pane in a narrow side panel, as in the IDE, so nothing of it lies under
        // the window.
        let mut full = self.ctx.run_ui(input, |ui| {
            egui::Panel::left("pane").resizable(false).exact_size(300.0).show_inside(ui, |ui| {
                action = panel.show(ui, form, Some(grid), &[], &tr);
            });
        });
        full.textures_delta.clear();
        fn walk(shape: &egui::Shape, out: &mut Vec<(String, Rect)>) {
            match shape {
                egui::Shape::Text(t) => out.push((t.galley.text().to_owned(), t.visual_bounding_rect())),
                egui::Shape::Vec(v) => v.iter().for_each(|s| walk(s, out)),
                other if std::env::var("DG106_DEBUG").is_ok() => {
                    out.push((format!("<{}>", match other { egui::Shape::Rect(_) => "rect", egui::Shape::LineSegment { .. } => "line", egui::Shape::Path(_) => "path", egui::Shape::Mesh(_) => "mesh", _ => "shape" }), other.visual_bounding_rect()))
                }
                _ => {}
            }
        }
        self.painted.clear();
        for cs in &full.shapes {
            walk(&cs.shape, &mut self.painted);
        }
        action
    }

    fn window(&self) -> Rect {
        self.ctx
            .memory(|m| m.area_rect(egui::Id::new(("datagrid_settings", "DG"))))
            .expect("the settings window is shown")
    }

    /// Turn the wheel over the window's left side until the COLUMNS band (its **+**) is
    /// near the top of the window, as a developer scrolls down to the section.
    fn scroll_to_columns(&mut self) {
        for _ in 0..3 {
            self.frame(vec![]);
        }
        for _ in 0..40 {
            let w = self.window();
            let plus = self.painted.iter().find(|(t, _)| t == "+").map(|(_, r)| *r);
            if plus.is_some_and(|r| r.top() < w.top() + 90.0) {
                break;
            }
            let at = egui::pos2(w.left() + 40.0, w.top() + 140.0);
            self.frame(vec![
                Event::PointerMoved(at),
                Event::MouseWheel {
                    unit: egui::MouseWheelUnit::Point,
                    delta: egui::vec2(0.0, -60.0),
                    phase: egui::TouchPhase::Move,
                    modifiers: Default::default(),
                },
            ]);
        }
        for _ in 0..3 {
            self.frame(vec![]);
        }
        let w = self.window();
        let plus = self.painted.iter().find(|(t, _)| t == "+").map(|(_, r)| *r);
        assert!(plus.is_some_and(|r| w.contains(r.center())), "the COLUMNS band is in view: {plus:?} in {w:?}");
    }

    /// Where the last frame painted `text` (the first such text).
    fn at(&self, text: &str) -> Pos2 {
        self.painted
            .iter()
            .find(|(t, _)| t == text)
            .map(|(_, r)| r.center())
            .unwrap_or_else(|| panic!("{text:?} is not painted; painted: {:?}", self.painted.iter().map(|p| &p.0).collect::<Vec<_>>()))
    }

    /// A click at `at`: over it, down, up — one frame each. The actions of the three.
    fn click(&mut self, at: Pos2) -> InspectorAction {
        let mut all = InspectorAction::default();
        for events in [vec![Event::PointerMoved(at)], vec![press(at, true)], vec![press(at, false)]] {
            let a = self.frame(events);
            all.set_props.extend(a.set_props);
            all.set_props_in_one_step.extend(a.set_props_in_one_step);
        }
        all
    }

    /// The grid as the designer would leave it after `action`.
    fn apply(&mut self, action: &InspectorAction) {
        for (_, edits) in &action.set_props_in_one_step {
            for (k, v) in edits {
                self.grid.set_prop(k, v.clone());
            }
        }
        for (_, k, v) in &action.set_props {
            self.grid.set_prop(k, v.clone());
        }
    }

    fn order(&self) -> Vec<String> {
        DataGridAdvanced::from_control(&self.grid).columns.into_iter().map(|c| c.id).collect()
    }
}

/// AC1 — the window keeps its size, frame after frame and column after column (a
/// Checkbox column's long details, a plain one's, a Button column's), in six languages;
/// only the grip changes it.
#[test]
fn the_window_keeps_its_size_whatever_column_is_shown() {
    for &lang in crate::i18n::Language::ALL {
        let mut rig = Rig::new(lang);
        rig.scroll_to_columns();
        let first = rig.window();
        let columns = DataGridAdvanced::from_control(&rig.grid).columns;
        let mut seen = Vec::new();
        for frame in 0..120 {
            if frame % 10 == 0 {
                rig.panel.datagrid_settings.select(&columns, (frame / 10) % 3);
            }
            rig.frame(vec![]);
            seen.push(rig.window());
        }
        for r in &seen {
            assert!(
                (r.height() - first.height()).abs() < 0.5 && (r.width() - first.width()).abs() < 0.5,
                "{lang:?}: the window went from {first:?} to {r:?}"
            );
        }
        let stored = rig.panel.datagrid_settings.size;
        assert!(
            first.height() <= stored.y + 2.0 && first.width() <= stored.x + 2.0,
            "{lang:?}: the window ({:?}) is larger than its stored size {stored:?}",
            first.size()
        );
        println!("{lang:?}: 120 frames, 3 columns in turn, window {:?} (stored {stored:?})", first.size());
    }
}

/// AC2 — **+** adds a column after the selected one, as the canvas's Add does, in one
/// step; **−** asks first, and only its **Delete** removes the column, in one step.
#[test]
fn add_and_a_confirmed_delete_are_the_canvas_edits_in_one_step() {
    let tr = crate::i18n::Language::English.tr();
    let mut rig = Rig::new(crate::i18n::Language::English);
    rig.scroll_to_columns();
    let before = rig.grid.clone();
    let add = rig.click(rig.at("+"));
    assert_eq!(add.set_props_in_one_step.len(), 1, "one click, one step");
    let (want, _, _) = dgs::add_column_after(&before, Some(0));
    assert_eq!(add.set_props_in_one_step[0], ("DG".to_owned(), want));
    rig.apply(&add);
    assert_eq!(rig.order(), ["ACTIVE", "COLUMN_4", "ACTOR-NAME", "EDIT"]);

    // The new column is selected; − asks before anything changes.
    rig.frame(vec![]);
    let ask = rig.click(rig.at("−"));
    assert!(ask.set_props_in_one_step.is_empty() && ask.set_props.is_empty(), "nothing yet");
    rig.frame(vec![]);
    let question = tr.dg_col_delete_ask.replace("{}", "Column 4");
    assert!(rig.painted.iter().any(|(t, _)| *t == question), "the window asks: {question:?}");
    let before = rig.grid.clone();
    let delete = rig.click(rig.at(tr.dg_col_delete_yes));
    assert_eq!(delete.set_props_in_one_step, [("DG".to_owned(), dgs::delete_column(&before, 1))]);
    rig.apply(&delete);
    assert_eq!(rig.order(), ["ACTIVE", "ACTOR-NAME", "EDIT"]);
    assert_eq!(rig.grid.get_prop("Rows").map(|v| v.as_str().to_owned()).unwrap(), "Y\tAnn\t\nN\tBob\t");
    println!("+ then − / Delete: two steps, each the canvas's own edit; the grid is back as it was");
}

/// AC3 — dragging column 3 above column 1 in the list gives 3, 1, 2; ▼ moves the
/// selected one down. Each is one step.
#[test]
fn dragging_in_the_list_and_the_arrows_reorder_the_columns() {
    let mut rig = Rig::new(crate::i18n::Language::English);
    rig.scroll_to_columns();
    let from = rig.at("3 — EDIT  [Button]");
    let to = rig.at("1 — ACTIVE") - egui::vec2(0.0, 10.0);
    let before = rig.grid.clone();
    let mut steps = Vec::new();
    let mut events = vec![vec![Event::PointerMoved(from)], vec![press(from, true)]];
    for k in 1..=8 {
        let y = from.y + (to.y - from.y) * k as f32 / 8.0;
        events.push(vec![Event::PointerMoved(egui::pos2(from.x, y))]);
    }
    events.push(vec![press(to, false)]);
    for e in events {
        steps.extend(rig.frame(e).set_props_in_one_step);
    }
    assert_eq!(steps, [("DG".to_owned(), dgs::move_column(&before, 2, 0))]);
    let action = InspectorAction { set_props_in_one_step: steps, ..Default::default() };
    rig.apply(&action);
    assert_eq!(rig.order(), ["EDIT", "ACTIVE", "ACTOR-NAME"], "AC3: 3, 1, 2");

    // The dragged column stays selected; ▼ takes it one down.
    rig.frame(vec![]);
    let before = rig.grid.clone();
    let down = rig.click(rig.at("▼"));
    assert_eq!(down.set_props_in_one_step, [("DG".to_owned(), dgs::move_column(&before, 0, 1))]);
    rig.apply(&down);
    assert_eq!(rig.order(), ["ACTIVE", "EDIT", "ACTOR-NAME"]);
    println!("drag 3 above 1 -> 3,1,2; ▼ -> 1,3,2; one step each");
}

/// R7 — what the window sends is ONE undo step in the Designer: one Undo puts
/// `Columns`, `Rows` and `AdvancedGrid` back together.
#[test]
fn one_undo_takes_back_an_added_column() {
    let (form, grid) = grid();
    let mut d = crate::panels::designer::DesignerPanel::new(form);
    let (edits, _, _) = dgs::add_column_after(&grid, Some(0));
    assert!(edits.len() >= 3, "Columns, Rows and AdvancedGrid: {edits:?}");
    d.set_properties_in_one_step("DG", edits);
    let after = d.form.find_control("DG").unwrap().clone();
    assert_ne!(after.get_prop("Columns"), grid.get_prop("Columns"));
    d.undo();
    let back = d.form.find_control("DG").unwrap();
    for key in ["Columns", "Rows", DATAGRID_ADVANCED_PROP] {
        assert_eq!(back.get_prop(key), grid.get_prop(key), "{key} is back after one Undo");
    }
}

/// R8–R9, R17 — a Button column's details: its Content, caption and icon, the hint, and
/// the warning while it has neither; a Data column's: its name in DATAGRID-ROW-CONTENT,
/// and a clash said as it is typed.
#[test]
fn the_details_say_what_a_column_is_and_what_is_wrong_with_its_name() {
    let tr = crate::i18n::Language::English.tr();
    let mut rig = Rig::new(crate::i18n::Language::English);
    rig.scroll_to_columns();
    let columns = DataGridAdvanced::from_control(&rig.grid).columns;
    rig.panel.datagrid_settings.select(&columns, 2);
    rig.frame(vec![]);
    rig.frame(vec![]);
    let has =|rig: &Rig, t: &str| rig.painted.iter().any(|(p, _)| p == t);
    for t in [tr.dg_col_content_button, tr.dg_col_button_caption, tr.dg_col_button_icon, tr.dg_col_button_hint] {
        let w = rig.window();
        let shown: Vec<String> = rig
            .painted
            .iter()
            .filter(|(_, r)| w.contains(r.center()))
            .map(|(t, r)| format!("{t} @{:.0},{:.0}-{:.0}", r.left(), r.top(), r.right()))
            .collect();
        assert!(has(&rig, t), "{t:?} on a Button column; the window shows {shown:?}");
    }
    assert!(!has(&rig, tr.dg_col_button_needs_face), "it has a caption");

    // Rename the plain column to a reserved word: the warning is there at once.
    let mut advanced = DataGridAdvanced::from_control(&rig.grid);
    advanced.columns[1].title = "MOVE".into();
    rig.grid.set_prop(DATAGRID_ADVANCED_PROP, PropValue::String(advanced.to_json().unwrap()));
    rig.panel.datagrid_settings.select(&advanced.columns, 1);
    rig.frame(vec![]);
    rig.frame(vec![]);
    assert!(has(&rig, tr.dg_col_cobol_name));
    let warning = format!("⚠ {}", tr.dg_col_warn_reserved.replace("{}", "MOVE"));
    assert!(has(&rig, &warning), "{warning:?}");
    assert!(has(&rig, "2 — MOVE"), "the list row is the renamed column");
}
