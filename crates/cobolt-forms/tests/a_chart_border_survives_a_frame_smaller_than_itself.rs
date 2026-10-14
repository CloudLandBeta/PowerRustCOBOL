// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! A chart's gradient border must not panic when the chart is drawn smaller than its border is thick.
//!
//! **The report (operator, 2026-10-14):** adding an *entrance animation* to a chart on
//! `PowerDemo3/forms/Charts/charts-form.cfrm` and previewing it crashed the IDE:
//!
//! ```text
//! panic: min > max, or either was NaN. min = 0.0, max = -1.0
//!   cobolt_forms::paint::rounded_rect_outline_points_per_corner
//!   cobolt_forms::paint::draw_chart_preview
//!   cobolt_forms::render::render_faces_with
//! ```
//!
//! **The cause:** a *ZoomIn* entrance starts at scale 0.001, so for its first frames the chart is a
//! fraction of a pixel across. The gradient border pulls its inner outline in by the border's width
//! (`rect.shrink(width)`), which turns a rect narrower than twice that width **inside out**; the
//! corner-radius cap, `0.5 × min(width, height)`, went negative and `f32::clamp(0.0, cap)` panics when
//! its maximum is below its minimum. Nothing about it is specific to a tilted chart (the backtrace
//! has no perspective frame in it); the report came while both were being tried.
//!
//! What is asserted is the whole class, with numbers: every chart type, a border from 1 to 40 points
//! wide, a gradient on and off, drawn at every size from nothing to a few points, and the
//! reported flow itself through the Designer's `render_faces`.

#![cfg(feature = "render")]

use std::panic::{catch_unwind, AssertUnwindSafe};

use cobolt_forms::containers::ActiveTabs;
use cobolt_forms::model::{Control, ControlType, PropValue};
use cobolt_forms::paint::draw_control;
use cobolt_forms::render::{render_faces, Backdrop, FormState, RenderInput, RenderMode, RenderTransform};
use egui::{Pos2, Rect};

const CHARTS: [ControlType; 7] = [
    ControlType::BarChart,
    ControlType::LineChart,
    ControlType::PieChart,
    ControlType::AreaChart,
    ControlType::ScatterChart,
    ControlType::DonutChart,
    ControlType::RadarChart,
];

fn chart(ct: &ControlType, w: i32, h: i32, border: i64, gradient: bool, blur: i64, tilt: bool) -> Control {
    let mut c = Control::new("CH", ct.clone(), 10, 10);
    c.rect = cobolt_forms::Rect::new(0, 0, w, h);
    c.set_prop("BorderStyle", PropValue::String("Single".into()));
    c.set_prop("BorderWidth", PropValue::Int(border));
    c.set_prop("BorderGradientEnabled", PropValue::Bool(gradient));
    c.set_prop("BorderBlur", PropValue::Int(blur));
    if tilt {
        c.set_prop("RotateX", PropValue::Int(8));
        c.set_prop("RotateY", PropValue::Int(-30));
        c.set_prop("PerspectiveDistance", PropValue::Int(60));
    }
    c
}

fn message(e: Box<dyn std::any::Any + Send>) -> String {
    e.downcast_ref::<String>()
        .cloned()
        .or_else(|| e.downcast_ref::<&str>().map(|s| (*s).to_owned()))
        .unwrap_or_default()
}

/// `draw_control` of `c` in a frame of its own.
fn paint(c: &Control) {
    let ctx = egui::Context::default();
    let mut full = ctx.run_ui(
        egui::RawInput { screen_rect: Some(Rect::from_min_size(Pos2::ZERO, egui::vec2(200.0, 200.0))), ..Default::default() },
        |root| {
            egui::CentralPanel::default().frame(egui::Frame::NONE).show(root, |ui| {
                draw_control(ui.painter(), Pos2::new(10.0, 10.0), c, false, true, 1.0, 1.0, None);
            });
        },
    );
    full.textures_delta.clear();
}

#[test]
fn a_chart_border_is_drawn_at_every_size_from_nothing_up() {
    let sizes = [0, 1, 2, 3, 5, 8, 20];
    let (mut runs, mut failed) = (0usize, Vec::new());
    for ct in &CHARTS {
        for border in [1i64, 3, 10, 40] {
            for gradient in [false, true] {
                for blur in [0i64, 6] {
                    for &w in &sizes {
                        for &h in &sizes {
                            runs += 1;
                            let c = chart(ct, w, h, border, gradient, blur, false);
                            if let Err(e) = catch_unwind(AssertUnwindSafe(|| paint(&c))) {
                                failed.push(format!(
                                    "{ct:?} border {border} gradient {gradient} blur {blur} at {w}×{h}: {}",
                                    message(e)
                                ));
                            }
                        }
                    }
                }
            }
        }
    }
    let kinds: std::collections::BTreeSet<String> =
        failed.iter().map(|f| f.split(": ").last().unwrap_or("").to_owned()).collect();
    println!("\n  {runs} chart borders drawn at 0–20 points, {} panicked", failed.len());
    for f in failed.iter().take(5) {
        println!("    {f}");
    }
    assert!(failed.is_empty(), "{} of {runs} panicked ({kinds:?}); first: {}", failed.len(), failed[0]);
}

/// An entrance effect, frozen at one instant — what a Preview draws while it plays.
struct At(RenderTransform);
impl FormState for At {
    fn transform(&self, _b: &Control) -> RenderTransform {
        self.0
    }
}

/// The reported flow: a chart with a gradient border, a *ZoomIn* at its first frames (scale 0.001, then
/// growing), through the Designer's canvas — `render_faces`, the frame in the crash log — tilted or not.
#[test]
fn a_chart_zooming_in_is_drawn_on_the_designer_canvas_from_its_first_frame() {
    let active = ActiveTabs::new();
    let (mut runs, mut failed) = (0usize, Vec::new());
    for ct in &CHARTS {
        for tilt in [false, true] {
            for scale in [0.0f32, 0.001, 0.01, 0.02, 0.05, 0.1, 0.5, 1.0] {
                let mut c = chart(ct, 240, 160, 3, true, 0, tilt);
                c.rect = cobolt_forms::Rect::new(80, 40, 240, 160);
                let controls = vec![c];
                let state = At(RenderTransform { dx: 0.0, dy: 0.0, scale, alpha: scale });
                runs += 1;
                let r = catch_unwind(AssertUnwindSafe(|| {
                    let ctx = egui::Context::default();
                    let mut full = ctx.run_ui(
                        egui::RawInput { screen_rect: Some(Rect::from_min_size(Pos2::ZERO, egui::vec2(480.0, 320.0))), ..Default::default() },
                        |root| {
                            egui::CentralPanel::default().frame(egui::Frame::NONE).show(root, |ui| {
                                let input = RenderInput {
                                    controls: &controls,
                                    state: &state,
                                    form_size: egui::vec2(460.0, 300.0),
                                    glass: true,
                                    mode: RenderMode::Static,
                                    active_tabs: &active,
                                    backdrop: Backdrop::default(),
                                };
                                let origin = ui.min_rect().min;
                                render_faces(&ui.painter().clone(), origin, &input);
                            });
                        },
                    );
                    full.textures_delta.clear();
                }));
                if let Err(e) = r {
                    failed.push(format!("{ct:?} tilted={tilt} scale {scale}: {}", message(e)));
                }
            }
        }
    }
    println!("\n  {runs} zoom-in frames on the Designer canvas (7 charts × tilted/not × 8 scales), {} panicked", failed.len());
    for f in failed.iter().take(5) {
        println!("    {f}");
    }
    assert!(failed.is_empty(), "{} of {runs} panicked; first: {}", failed.len(), failed[0]);
}

/// The form from the report, every control of it zoomed in from nothing: the Designer's canvas
/// (`render_faces`) and the Preview's interactive path (`render_form`). What a ZoomIn does to a chart it does
/// to anything else on the form, so this also says whether any other control has a crash of this kind.
#[test]
fn the_charts_demo_form_zooms_in_from_nothing_on_the_canvas_and_in_the_preview() {
    use cobolt_forms::render::render_form;
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/PowerDemo3/forms/Charts/charts-form.cfrm");
    let Ok(form) = cobolt_forms::load_form(&path) else {
        println!("  (the PowerDemo3 charts form is not in this checkout — skipped)");
        return;
    };
    let active = ActiveTabs::new();
    let (mut runs, mut failed) = (0usize, Vec::new());
    for tilt in [false, true] {
        let mut controls = form.controls.clone();
        if tilt {
            for c in &mut controls {
                if c.control_type.has_perspective() {
                    c.set_prop("RotateX", PropValue::Int(8));
                    c.set_prop("RotateY", PropValue::Int(-30));
                    c.set_prop("PerspectiveDistance", PropValue::Int(60));
                }
            }
        }
        for scale in [0.0f32, 0.001, 0.01, 0.02, 0.05, 0.1, 0.25, 0.5, 1.0, 1.2] {
            for path in ["canvas", "preview"] {
                runs += 1;
                let state = At(RenderTransform { dx: 0.0, dy: 0.0, scale, alpha: scale.min(1.0) });
                let r = catch_unwind(AssertUnwindSafe(|| {
                    let ctx = egui::Context::default();
                    for _ in 0..2 {
                        let mut full = ctx.run_ui(
                            egui::RawInput { screen_rect: Some(Rect::from_min_size(Pos2::ZERO, egui::vec2(1400.0, 900.0))), ..Default::default() },
                            |root| {
                                egui::CentralPanel::default().frame(egui::Frame::NONE).show(root, |ui| {
                                    let input = RenderInput {
                                        controls: &controls,
                                        state: &state,
                                        form_size: egui::vec2(form.width as f32, form.height as f32),
                                        glass: true,
                                        mode: if path == "canvas" { RenderMode::Static } else { RenderMode::Interactive },
                                        active_tabs: &active,
                                        backdrop: Backdrop::default(),
                                    };
                                    if path == "canvas" {
                                        let origin = ui.min_rect().min;
                                        render_faces(&ui.painter().clone(), origin, &input);
                                    } else {
                                        render_form(ui, &input);
                                    }
                                });
                            },
                        );
                        full.textures_delta.clear();
                    }
                }));
                if let Err(e) = r {
                    failed.push(format!("tilted={tilt} {path} scale {scale}: {}", message(e)));
                }
            }
        }
    }
    println!(
        "\n  charts-form.cfrm ({} controls) zoomed in from nothing: {runs} renders (canvas and preview, flat and tilted, 10 scales), {} panicked",
        form.controls.len(),
        failed.len()
    );
    for f in failed.iter().take(6) {
        println!("    {f}");
    }
    assert!(failed.is_empty(), "{} of {runs} panicked; first: {}", failed.len(), failed[0]);
}
