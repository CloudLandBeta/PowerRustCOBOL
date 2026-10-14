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
