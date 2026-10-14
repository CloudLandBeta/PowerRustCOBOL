// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Spec 096 (R11; AC9) — in the Form Designer a tilted control is drawn tilted, **live**,
//! and is still selected, moved and resized by the rectangle it was designed in.
//!
//! The canvas draws the control through `render_faces_with`, which projects what the control
//! painted; the selection rectangle, the eight knobs and the drag that resizes it read the
//! rectangle the engine reports for the control, and that is the designed one. A knob that sat
//! on the projected outline would move with every degree of `RotateY`, and a control tilted
//! strongly could not be grabbed where its handles are drawn.
//!
//! A child module of `designer`, so it reads the designer's private state the way the other test
//! modules there do. Real frames of the real `DesignerPanel::show`, with pointer events.

use super::*;
use crate::llm::LlmConfig;
use cobolt_forms::perspective::Perspective;
use cobolt_forms::{Control, ControlType, Form};
use egui::{pos2, vec2, Event, PointerButton};

const SCREEN: Vec2 = Vec2::new(1400.0, 1000.0);

fn press(at: Pos2, pressed: bool) -> Event {
    Event::PointerButton { pos: at, button: PointerButton::Primary, pressed, modifiers: egui::Modifiers::NONE }
}

/// A designer on a form holding one tilted chart.
struct Rig {
    ctx: egui::Context,
    llm: LlmConfig,
    d: DesignerPanel,
    t: f64,
}

impl Rig {
    fn new(rx: i64, ry: i64, distance: i64) -> Self {
        let mut form = Form::new("F", "F", 800, 600);
        let mut chart = Control::new("CH", ControlType::BarChart, 100, 100);
        chart.rect = cobolt_forms::model::Rect::new(100, 100, 240, 160);
        chart.set_prop("RotateX", cobolt_forms::PropValue::Int(rx));
        chart.set_prop("RotateY", cobolt_forms::PropValue::Int(ry));
        chart.set_prop("PerspectiveDistance", cobolt_forms::PropValue::Int(distance));
        form.controls.push(chart);
        let mut rig = Self {
            ctx: egui::Context::default(),
            llm: LlmConfig::load_defaults_for_test(),
            d: DesignerPanel::new(form),
            t: 0.0,
        };
        rig.d.selected_ids = vec!["CH".to_owned()];
        for _ in 0..3 {
            rig.frame(vec![]);
        }
        rig
    }

    fn frame(&mut self, events: Vec<Event>) {
        let mut input = egui::RawInput::default();
        input.screen_rect = Some(Rect::from_min_size(Pos2::ZERO, SCREEN));
        input.max_texture_side = Some(8192);
        input.focused = true;
        self.t += 0.05;
        input.time = Some(self.t);
        input.events = events;
        let (d, llm) = (&mut self.d, &self.llm);
        let mut out = self.ctx.run_ui(input, |root| {
            egui::CentralPanel::default().show_inside(root, |ui| {
                let _ = d.show(ui, &mut None, &[], llm, None, None);
            });
        });
        out.textures_delta.clear();
    }

    fn at(&self, x: f32, y: f32) -> Pos2 {
        self.d.canvas_origin.expect("the canvas has been drawn") + Vec2::new(x, y)
    }

    fn drag(&mut self, from: Pos2, to: Pos2) {
        self.frame(vec![Event::PointerMoved(from)]);
        self.frame(vec![press(from, true)]);
        for i in 1..=6 {
            self.frame(vec![Event::PointerMoved(from + (to - from) * (i as f32 / 6.0))]);
        }
        self.frame(vec![press(to, false)]);
        for _ in 0..3 {
            self.frame(vec![]);
        }
    }

    fn rect(&self) -> cobolt_forms::model::Rect {
        self.d.form.find_control("CH").unwrap().rect
    }
}

#[test]
fn the_selection_and_the_knobs_stay_on_the_designed_rectangle() {
    let rig = Rig::new(8, -50, 60);
    let designed = cobolt_forms::model::Rect::new(100, 100, 240, 160);
    let handles = rig.d.handle_rect_of("CH").expect("the control is selected");
    println!("\n  designed {designed:?}; the knobs are laid out on {handles:?}");
    assert_eq!(handles, designed, "the knobs sit on the designed rectangle");

    // …while what is DRAWN has moved: the projected outline is not that rectangle.
    let plane = Rect::from_min_size(pos2(100.0, 100.0), vec2(240.0, 160.0));
    let outline = Perspective::new(8, -50, 60).quad(plane);
    let moved = outline
        .iter()
        .zip([plane.left_top(), plane.right_top(), plane.right_bottom(), plane.left_bottom()])
        .map(|(a, b)| (*a - b).length())
        .fold(0.0f32, f32::max);
    println!("  the drawn outline's corners are up to {moved:.1} px from the designed ones");
    assert!(moved > 20.0, "the chart is drawn tilted on the canvas");
}

#[test]
fn dragging_a_knob_of_a_tilted_control_resizes_the_designed_rectangle() {
    let mut rig = Rig::new(8, -50, 60);
    let before = rig.rect();
    let knob = handle_pos(&rig.d.handle_rect_of("CH").unwrap(), Handle::BotRight);
    println!("\n  designed {before:?}; bottom-right knob at {knob:?}");
    let (from, to) = (rig.at(knob.x, knob.y), rig.at(knob.x + 30.0, knob.y + 20.0));
    rig.drag(from, to);
    let after = rig.rect();
    println!("  after dragging the knob by (+30, +20): {after:?}");
    assert_eq!((after.x, after.y), (before.x, before.y), "the corner opposite the knob did not move");
    // The designer snaps a resize to its 8-point grid, so the size lands on the grid line nearest the
    // drag (270 × 180 asked for → 264 × 176), whatever the control looks like on the canvas.
    assert!(
        (after.w - (before.w + 30)).abs() <= 8 && (after.h - (before.h + 20)).abs() <= 8,
        "the designed rectangle grew by the drag, to the nearest grid line: {after:?}"
    );
    assert!(after.w > before.w && after.h > before.h);
    assert_eq!((after.w % 8, after.h % 8), (0, 0), "snapped to the grid");

    // The tilt is untouched by the resize, and one undo puts the size back.
    let c = rig.d.form.find_control("CH").unwrap();
    assert_eq!(c.get_prop("RotateY").map(|v| v.as_i64()), Some(-50));
    rig.d.undo();
    assert_eq!(rig.rect(), before, "one undo restores the rectangle");
}

// ── Spec 098: the corner pin's handles ───────────────────────────────────────

fn pin(rig: &Rig, key: &str) -> i64 {
    rig.d.form.find_control("CH").unwrap().get_prop(key).map_or(0, |v| v.as_i64())
}

fn designed_rect() -> Rect {
    Rect::from_min_size(pos2(100.0, 100.0), vec2(240.0, 160.0))
}

#[test]
fn the_corner_handles_sit_on_the_projected_outline_once_the_control_is_tilted() {
    let rig = Rig::new(8, -50, 60);
    let (sid, quad) = rig.d.sel_pin_quad.clone().expect("a tilted control shows its corner handles");
    assert_eq!(sid, "CH");
    let want = Perspective::new(8, -50, 60).quad(designed_rect());
    let worst = quad.iter().zip(want).map(|(a, b)| (*a - b).length()).fold(0.0f32, f32::max);
    println!("\n  corner handles at {quad:.1?}; the model's outline differs by {worst:.4} px");
    assert!(worst < 1e-3);
    // …and the resize knobs are still on the designed rectangle.
    assert_eq!(rig.d.handle_rect_of("CH"), Some(cobolt_forms::model::Rect::new(100, 100, 240, 160)));

    let flat = Rig::new(0, 0, 50);
    assert!(flat.d.sel_pin_quad.is_none(), "a flat, unpinned control shows no corner handles (spec 098 Q1)");
}

#[test]
fn dragging_a_corner_handle_pins_that_corner_and_is_one_undo_step() {
    let mut rig = Rig::new(8, -30, 60);
    let (_, quad) = rig.d.sel_pin_quad.clone().unwrap();
    let br = quad[2];
    let steps = rig.d.undo_stack.len();
    let (from, to) = (rig.at(br.x, br.y), rig.at(br.x + 30.0, br.y - 20.0));
    rig.drag(from, to);
    let (x, y) = (pin(&rig, "PinBottomRightX"), pin(&rig, "PinBottomRightY"));
    println!("\n  bottom-right handle at ({:.1}, {:.1}) dragged by (+30, −20): PinBottomRightX = {x}, PinBottomRightY = {y}", br.x, br.y);
    assert_eq!((x, y), (30, -20), "the corner's two pins are exactly the drag");
    for key in ["PinTopLeftX", "PinTopLeftY", "PinTopRightX", "PinTopRightY", "PinBottomLeftX", "PinBottomLeftY"] {
        assert_eq!(pin(&rig, key), 0, "{key} is untouched");
    }
    // The handle now sits where it was dragged to.
    let (_, after) = rig.d.sel_pin_quad.clone().unwrap();
    assert!((after[2] - (br + vec2(30.0, -20.0))).length() < 1e-3);
    // The tilt is untouched, and the whole gesture is ONE undo step.
    assert_eq!(rig.d.form.find_control("CH").unwrap().get_prop("RotateY").map(|v| v.as_i64()), Some(-30));
    assert_eq!(rig.d.undo_stack.len(), steps + 1, "one undo step for the two pins");
    rig.d.undo();
    assert_eq!((pin(&rig, "PinBottomRightX"), pin(&rig, "PinBottomRightY")), (0, 0), "one undo puts both back");
}

#[test]
fn a_drag_that_would_fold_the_outline_is_refused() {
    let mut rig = Rig::new(8, -30, 60);
    let (_, quad) = rig.d.sel_pin_quad.clone().unwrap();
    let (br, tl) = (quad[2], quad[0]);
    // The bottom-right corner dragged past the top-left one: a bow tie, were it allowed.
    let (from, to) = (rig.at(br.x, br.y), rig.at(tl.x - 60.0, tl.y - 60.0));
    rig.drag(from, to);
    let ctrl = rig.d.form.find_control("CH").unwrap().clone();
    let per = Perspective::of(&ctrl).unwrap();
    let rect = designed_rect();
    let drawable = cobolt_forms::perspective::pin_is_drawable(&per.unpinned().quad(rect), &per.pinned_quad(rect));
    let (x, y) = (pin(&rig, "PinBottomRightX"), pin(&rig, "PinBottomRightY"));
    println!("\n  bottom-right dragged towards ({:.0}, {:.0}), past the top-left: pins ended at ({x}, {y}); outline drawable: {drawable}", tl.x - 60.0, tl.y - 60.0);
    assert!(drawable || (x, y) == (0, 0), "the outline is never left folded");
    let full = (((tl.x - 60.0) - br.x).round() as i64, ((tl.y - 60.0) - br.y).round() as i64);
    assert_ne!((x, y), full, "the corner did not follow the pointer into the fold");
}
