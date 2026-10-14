// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Spec 096, tasks T9/T10 — **the pointer finds a tilted control where it is drawn.**
//!
//! A control that has been turned in space is somewhere else on the screen than
//! the rectangle it was designed in. A click on the picture must count; a click
//! on the empty ground the picture used to cover must not (R10). The Form
//! Designer is the one exception — it keeps the designed rectangle so a tilted
//! control can always be grabbed (R11) — and that is tested in `perspective_parity`.
//!
//! What is asserted here is the table of pointer positions and what each one
//! raised, printed, so the answer can be read rather than trusted.
//!
//! Spec 096 AC8 (R10), AC11 (R14).

#![cfg(feature = "render")]

use cobolt_forms::containers::ActiveTabs;
use cobolt_forms::model::{Control, ControlType, PropValue};
use cobolt_forms::perspective::{quad_contains, Perspective};
use cobolt_forms::render::{render_form, DesignedState, RenderInput, RenderMode, RenderOutput};
use egui::{pos2, Pos2, Rect, Vec2};

const FORM: Vec2 = Vec2::new(600.0, 400.0);

fn control(id: &str, ct: ControlType, x: i32, y: i32, w: i32, h: i32) -> Control {
    let mut c = Control::new(id, ct, x, y);
    c.rect = cobolt_forms::Rect::new(x, y, w, h);
    c
}

fn tilt(c: &mut Control, rx: i64, ry: i64, d: i64) {
    c.set_prop("RotateX", PropValue::Int(rx));
    c.set_prop("RotateY", PropValue::Int(ry));
    c.set_prop("PerspectiveDistance", PropValue::Int(d));
}

fn designed(c: &Control) -> Rect {
    Rect::from_min_size(pos2(c.rect.x as f32, c.rect.y as f32), egui::vec2(c.rect.w as f32, c.rect.h as f32))
}

fn frame(ctx: &egui::Context, controls: &[Control], events: Vec<egui::Event>) -> RenderOutput {
    let active = ActiveTabs::new();
    let mut kept = None;
    let mut full = ctx.run_ui(
        egui::RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, FORM)),
            events,
            ..Default::default()
        },
        |root| {
            egui::CentralPanel::default().frame(egui::Frame::NONE).show(root, |ui| {
                let input = RenderInput {
                    controls,
                    state: &DesignedState,
                    form_size: FORM,
                    glass: true,
                    mode: RenderMode::Interactive,
                    active_tabs: &active,
                    backdrop: Default::default(),
                };
                kept = Some(render_form(ui, &input));
            });
        },
    );
    full.textures_delta.clear();
    kept.expect("a frame was drawn")
}

fn button(pos: Pos2, down: bool) -> egui::Event {
    egui::Event::PointerButton { pos, button: egui::PointerButton::Primary, pressed: down, modifiers: Default::default() }
}

/// Click at `p` in a fresh context; the ids of the controls that raised `event`.
fn click(controls: &[Control], p: Pos2, event: &str) -> Vec<String> {
    let ctx = egui::Context::default();
    frame(&ctx, controls, vec![]);
    frame(&ctx, controls, vec![egui::Event::PointerMoved(p)]);
    frame(&ctx, controls, vec![button(p, true)]);
    let out = frame(&ctx, controls, vec![button(p, false)]);
    out.events.iter().filter(|e| e.event == event).map(|e| e.ctrl_id.clone()).collect()
}

#[test]
fn a_click_counts_where_the_shape_is_seen_and_only_there() {
    let mut shape = control("Shp", ControlType::Shape, 200, 150, 160, 100);
    shape.ensure_event("onClick");
    tilt(&mut shape, 0, -50, 50);
    let controls = vec![shape.clone()];

    let out = frame(&egui::Context::default(), &controls, vec![]);
    let quad = out.hit_quads["Shp"];
    let rect = designed(&shape);

    // Positions chosen from the geometry, then their premises checked: each one is
    // really where the table says it is.
    let both = rect.center();
    let quad_only = pos2(quad[1].x - 3.0, quad[1].y + 4.0); // the near (right) side stands taller than it was designed: its top corner rises above the rectangle
    let rect_only = rect.left_top() + egui::vec2(3.0, 3.0); // the far (left) side has drawn in
    let neither = pos2(520.0, 340.0);

    let table: [(&str, Pos2, bool, bool); 4] = [
        ("inside the outline and the designed rectangle", both, true, true),
        ("inside the outline, OUTSIDE the designed rectangle", quad_only, true, false),
        ("inside the designed rectangle, OUTSIDE the outline", rect_only, false, true),
        ("outside both", neither, false, false),
    ];
    println!("\n  ── R10: Shape, RotateY −50, onClick bound ──");
    println!("  outline {quad:?}");
    println!("  {:<54} {:>8} {:>9}   {:>8}", "pointer", "outline", "designed", "onClick");
    for (what, p, in_quad, in_rect) in table {
        assert_eq!(quad_contains(&quad, p), in_quad, "premise: {what} ({p:?})");
        assert_eq!(rect.contains(p), in_rect, "premise: {what} ({p:?})");
        let raised = click(&controls, p, "onClick");
        println!("  {what:<54} {in_quad:>8} {in_rect:>9}   {:>8}", !raised.is_empty());
        assert_eq!(
            !raised.is_empty(),
            in_quad,
            "{what}: onClick must arise exactly where the control is drawn, got {raised:?}"
        );
    }
}

#[test]
fn an_untilted_shape_still_answers_over_its_whole_rectangle() {
    // The control of the experiment: nothing about R10 may change a control left at the defaults.
    let mut shape = control("Shp", ControlType::Shape, 200, 150, 160, 100);
    shape.ensure_event("onClick");
    let controls = vec![shape.clone()];
    let rect = designed(&shape);
    for (p, want) in [
        (rect.center(), true),
        (rect.left_top() + egui::vec2(3.0, 3.0), true),
        (rect.right_bottom() - egui::vec2(3.0, 3.0), true),
        (pos2(rect.right() + 6.0, rect.center().y), false),
    ] {
        assert_eq!(!click(&controls, p, "onClick").is_empty(), want, "{p:?}");
    }
    println!("\n  an untilted Shape: clicks inside its rectangle raise onClick, one outside does not");
}

#[test]
fn a_tilted_chart_over_a_button_gives_each_click_to_the_one_that_is_seen() {
    let mut btn = control("Btn", ControlType::Button, 180, 170, 160, 40);
    btn.ensure_event("onClick");
    let mut chart = control("Chart", ControlType::BarChart, 180, 100, 160, 160);
    chart.ensure_event("onClick");
    tilt(&mut chart, 0, -55, 50); // drawn after (over) the button; the left side draws in
    let controls = vec![btn.clone(), chart.clone()];

    let out = frame(&egui::Context::default(), &controls, vec![]);
    let quad = out.hit_quads["Chart"];
    let on_both = pos2(300.0, 190.0); // inside the chart's outline and on the button
    let on_button_only = pos2(184.0, 190.0); // on the button, in the chart's designed box but outside its outline
    assert!(quad_contains(&quad, on_both) && designed(&btn).contains(on_both), "premise: both");
    assert!(!quad_contains(&quad, on_button_only) && designed(&chart).contains(on_button_only) && designed(&btn).contains(on_button_only), "premise: button only");

    println!("\n  ── a tilted chart drawn over a Button ──");
    for (what, p, want) in [
        ("on the chart's outline AND the button", on_both, "Chart"),
        ("on the button, in the chart's designed box but outside its outline", on_button_only, "Btn"),
    ] {
        let got = click(&controls, p, "onClick");
        println!("  {what:<66} → {got:?}");
        assert_eq!(got, vec![want.to_string()], "{what}");
    }
}

#[test]
fn a_tilted_label_keeps_its_clicks_and_offers_no_text_selection() {
    // R14: the caption is painted with the projection, so it is not hosted as selectable
    // text; the Label keeps its pointer events, its tab stop and its onGotFocus.
    let mk = |ry: i64| {
        let mut l = control("Lbl", ControlType::Label, 200, 150, 200, 40);
        l.set_prop("Caption", PropValue::String("Revenue by quarter".into()));
        l.ensure_event("onClick");
        l.ensure_event("onGotFocus");
        if ry != 0 {
            tilt(&mut l, 0, ry, 50);
        }
        l
    };
    println!("\n  ── R14: a Label, untilted and at RotateY −30 ──");
    println!("  {:<10} {:>14} {:>16} {:>11} {:>12}", "label", "caption shapes", "caption hosted", "onClick", "onGotFocus");
    for (name, label) in [("untilted", mk(0)), ("tilted", mk(-30))] {
        let controls = vec![label.clone()];
        let p = pos2(300.0, 170.0);
        let ctx = egui::Context::default();
        frame(&ctx, &controls, vec![]);
        frame(&ctx, &controls, vec![egui::Event::PointerMoved(p)]);
        frame(&ctx, &controls, vec![button(p, true)]);
        let out = frame(&ctx, &controls, vec![button(p, false)]);
        let clicked = out.events.iter().any(|e| e.event == "onClick");
        let focus = out.events.iter().any(|e| e.event == "onGotFocus");
        // The label is "hosted" as selectable text when egui's selection state knows its galley:
        // after a drag across it, there is a selection range to copy.
        let hosted = {
            let ctx = egui::Context::default();
            frame(&ctx, &controls, vec![]);
            frame(&ctx, &controls, vec![egui::Event::PointerMoved(pos2(215.0, 170.0))]);
            frame(&ctx, &controls, vec![button(pos2(215.0, 170.0), true)]);
            frame(&ctx, &controls, vec![egui::Event::PointerMoved(pos2(380.0, 170.0))]);
            frame(&ctx, &controls, vec![button(pos2(380.0, 170.0), false)]);
            ctx.plugin_opt::<egui::text_selection::LabelSelectionState>()
                .is_some_and(|handle| handle.lock().has_selection())
        };
        let text_shapes: usize = {
            let ctx = egui::Context::default();
            let mut n = 0;
            for _ in 0..2 {
                let mut full = ctx.run_ui(
                    egui::RawInput { screen_rect: Some(Rect::from_min_size(Pos2::ZERO, FORM)), ..Default::default() },
                    |root| {
                        egui::CentralPanel::default().frame(egui::Frame::NONE).show(root, |ui| {
                            let active = ActiveTabs::new();
                            let input = RenderInput {
                                controls: &controls, state: &DesignedState, form_size: FORM, glass: true,
                                mode: RenderMode::Interactive, active_tabs: &active, backdrop: Default::default(),
                            };
                            render_form(ui, &input);
                            let layer = ui.layer_id();
                            n = ctx.graphics(|g| {
                                g.get(layer).map_or(0, |l| {
                                    l.all_entries().filter(|c| matches!(c.shape, egui::Shape::Text(_))).count()
                                })
                            });
                        });
                    },
                );
                full.textures_delta.clear();
            }
            n
        };
        // The tab stop: with a Button after it, Tab reaches the label first and announces it.
        let tab_announced = {
            let mut with_button = controls.clone();
            let mut b = control("Btn", ControlType::Button, 200, 220, 100, 28);
            b.tab_order = 1;
            with_button.push(b);
            with_button[0].tab_order = 0;
            let ctx = egui::Context::default();
            frame(&ctx, &with_button, vec![]);
            frame(&ctx, &with_button, vec![]);
            let tab = egui::Event::Key {
                key: egui::Key::Tab,
                physical_key: Some(egui::Key::Tab),
                pressed: true,
                repeat: false,
                modifiers: Default::default(),
            };
            let mut announced = false;
            for events in [vec![tab], vec![], vec![]] {
                let out = frame(&ctx, &with_button, events);
                announced |= out.events.iter().any(|e| e.ctrl_id == "Lbl" && e.event == "onGotFocus");
            }
            announced
        };
        println!("  {name:<10} {text_shapes:>14} {hosted:>16} {clicked:>11} {focus:>12}   Tab announces it: {tab_announced}");
        assert!(tab_announced, "{name}: Tab must still reach the label and raise onGotFocus");
        assert!(clicked, "{name}: the label keeps its pointer events");
        assert!(focus, "{name}: a click still raises onGotFocus");
        assert_eq!(hosted, name == "untilted", "{name}: selectable text is offered only while the label is flat");
    }
}
