#![cfg(feature = "render")]
// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! **Every visual control is painted where its layer and z-order put it.**
//!
//! A control that paints some of its shapes in a pass of its own — after the other
//! controls — sits over whatever overlaps it, whatever the form says (the DataGrid
//! did, on the Designer's canvas). The probe is the same for every visual control
//! type: a `Label` is placed over the control, once with a higher z-order and once
//! in a layer above it; and the other way round, the control in a layer above a
//! `Label` of the base with a higher z-order. The label's text is one shape; every
//! shape the control paints must come before it (the label is over the control) or
//! after it (the control is over the label). The control's own shapes are known by
//! painting it alone.
//!
//! Three surfaces, as the engine has them: the run form (`render_form`), the preview
//! (`render_form_with_chrome`) and the Designer's faces pass (`render_faces`).

use cobolt_forms::containers::ActiveTabs;
use cobolt_forms::model::{Layer, PropValue};
use cobolt_forms::render::{
    render_faces, render_form, render_form_with_chrome, Backdrop, DesignedState, RenderInput, RenderMode,
};
use cobolt_forms::{Control, ControlType};
use egui::{Pos2, Rect, Vec2};

#[derive(Clone, Copy, Debug)]
enum Surface {
    Run,
    Preview,
    Faces,
}

const SURFACES: [Surface; 3] = [Surface::Run, Surface::Preview, Surface::Faces];

fn fnv64(s: &str) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in s.bytes() {
        h ^= b as u64;
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    h
}

/// One line per shape, in paint order. A text is its string's hash and its font; anything else is a
/// hash of the shape. (Positions are left out: a control painted with another beside it does not move.)
fn dump(out: &mut Vec<String>, shape: &egui::Shape) {
    use egui::Shape as S;
    match shape {
        S::Vec(v) => v.iter().for_each(|s| dump(out, s)),
        S::Noop => {}
        S::Text(t) => {
            let job = &t.galley.job;
            out.push(format!("TEXT {:016x} {:?}", fnv64(&job.text), job.sections.first().map(|s| s.format.font_id.size)));
        }
        other => out.push(format!("SHAPE {:016x}", fnv64(&format!("{other:?}")))),
    }
}

fn paint(surface: Surface, controls: &[Control], layers: &[Layer]) -> Vec<String> {
    let ctx = egui::Context::default();
    ctx.set_fonts(egui::FontDefinitions::default());
    let active = ActiveTabs::new();
    let mut shapes = Vec::new();
    for frame in 0..2 {
        let mut full = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(500.0, 400.0))),
                time: Some(f64::from(frame) * 0.05),
                ..Default::default()
            },
            |root_ui| {
                egui::CentralPanel::default().frame(egui::Frame::NONE).show_inside(root_ui, |ui| {
                    ui.set_min_size(Vec2::new(500.0, 400.0));
                    let input = RenderInput {
                        controls,
                        state: &DesignedState,
                        form_size: Vec2::new(500.0, 400.0),
                        glass: true,
                        mode: if matches!(surface, Surface::Faces) { RenderMode::Static } else { RenderMode::Interactive },
                        active_tabs: &active,
                        backdrop: Backdrop { layers: layers.to_vec(), ..Backdrop::default() },
                    };
                    match surface {
                        Surface::Run => {
                            render_form(ui, &input);
                        }
                        Surface::Preview => {
                            render_form_with_chrome(ui, &input, None);
                        }
                        Surface::Faces => {
                            let painter = ui.painter().clone();
                            render_faces(&painter, ui.min_rect().min, &input);
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
    if std::env::var("Z_DEBUG").is_ok() {
        println!("--- {surface:?} {} controls", controls.len());
        for l in &lines {
            println!("  {l}");
        }
    }
    lines
}

/// The control under test: at (40, 40), 300 × 200, on `layer` at `z`. A container comes with what its
/// chrome is for: a caption, a rounded corner, and a child that reaches that corner (the notch mask
/// repaints the backdrop there).
fn subject(ty: &ControlType, layer: Option<&str>, z: i32) -> Vec<Control> {
    let mut c = Control::new("SUBJECT", ty.clone(), 40, 40);
    c.rect.w = 300;
    c.rect.h = 200;
    c.layer = layer.map(str::to_owned);
    c.z_order = z;
    let container = matches!(
        ty,
        ControlType::Panel | ControlType::GroupBox | ControlType::TabControl | ControlType::Splitter
    );
    if !container {
        return vec![c];
    }
    if matches!(ty, ControlType::GroupBox) {
        c.set_prop("Caption", PropValue::String("CAPTION".into()));
    }
    if matches!(ty, ControlType::Panel | ControlType::GroupBox) {
        c.set_prop("CornerRadius", PropValue::Int(40));
    }
    // A DataGrid does not draw its frame inside a rounded parent's arc, so it is what makes the
    // notch mask paint; the other containers get a label.
    let mut child = if matches!(ty, ControlType::Panel | ControlType::GroupBox) {
        let mut g = Control::new("CHILD", ControlType::DataGrid, 1, 1);
        g.rect.w = 120;
        g.rect.h = 80;
        g
    } else {
        Control::new("CHILD", ControlType::Label, 1, 1)
    };
    child.set_prop("Caption", PropValue::String("CHILD".into()));
    child.parent = Some("SUBJECT".into());
    vec![c, child]
}

const MARKER: &str = "ZMARKERZ";

/// The marker: a label over the subject's first corner, on `layer` at `z`.
fn marker(layer: Option<&str>, z: i32) -> Control {
    let mut c = Control::new("MARKER", ControlType::Label, 60, 60);
    c.set_prop("Caption", PropValue::String(MARKER.into()));
    c.layer = layer.map(str::to_owned);
    c.z_order = z;
    c
}

/// Of the shapes painted with the marker, which are the subject's, in order, and where the marker's
/// text is: `(index of the marker's text, the indexes of the subject's shapes)`. The form's own shapes
/// (painted with no control at all) and the marker's (painted alone) are taken out, each once.
fn split(together: &[String], form_only: &[String], marker_alone: &[String]) -> (usize, Vec<usize>) {
    let at = together
        .iter()
        .position(|l| *l == format!("TEXT {:016x} {:?}", fnv64(MARKER), marker_text_size(marker_alone)))
        .expect("the marker's text was painted");
    let mut not_the_subjects: Vec<&String> = form_only.iter().chain(marker_alone.iter()).collect();
    let mut subject = Vec::new();
    for (i, line) in together.iter().enumerate() {
        match not_the_subjects.iter().position(|m| *m == line) {
            Some(p) => {
                not_the_subjects.remove(p);
            }
            None => subject.push(i),
        }
    }
    (at, subject)
}

fn marker_text_size(marker_alone: &[String]) -> Option<f32> {
    marker_alone
        .iter()
        .find(|l| l.starts_with(&format!("TEXT {:016x}", fnv64(MARKER))))
        .and_then(|l| l.rsplit(' ').next().map(|s| s.trim_start_matches("Some(").trim_end_matches(')').parse().ok()))
        .flatten()
}

fn visual_types() -> Vec<ControlType> {
    ControlType::ALL.iter().filter(|t| !t.is_non_visual()).cloned().collect()
}

/// Does `ty` paint at least one shape alone? (A control that paints nothing cannot be out of order.)
fn paints(surface: Surface, ty: &ControlType) -> bool {
    !paint(surface, &subject(ty, None, 50), &[]).is_empty()
}

#[test]
fn every_visual_control_is_painted_in_z_order_and_layer_order() {
    let layers = vec![Layer::new("L1")];
    let mut failures: Vec<String> = Vec::new();
    let mut checked = 0usize;
    for ty in visual_types() {
        if std::env::var("Z_ONLY").is_ok_and(|o| !format!("{ty:?}").eq_ignore_ascii_case(&o)) {
            continue;
        }
        for surface in SURFACES {
            if !paints(surface, &ty) {
                continue;
            }
            let form_only = paint(surface, &[], &layers);
            // The marker alone, less the form's own shapes (those are counted once, in `form_only`).
            let marker_alone = {
                let mut all = paint(surface, &[marker(None, 50)], &layers);
                for l in &form_only {
                    if let Some(p) = all.iter().position(|m| m == l) {
                        all.remove(p);
                    }
                }
                all
            };
            // (what the case is, the subject, the marker, whether the marker should be over the subject)
            let cases: Vec<(&str, Vec<Control>, Control, bool)> = vec![
                ("a label above it in z-order", subject(&ty, None, 50), marker(None, 90), true),
                ("a label below it in z-order", subject(&ty, None, 50), marker(None, 10), false),
                ("a label in a layer above it", subject(&ty, None, 80), marker(Some("L1"), 5), true),
                ("a layer's control over a label of the base", subject(&ty, Some("L1"), 5), marker(None, 80), false),
            ];
            for (what, sub, mark, marker_over) in cases {
                let mut all = sub;
                all.push(mark);
                let together = paint(surface, &all, &layers);
                let (at, subject_shapes) = split(&together, &form_only, &marker_alone);
                checked += 1;
                let out_of_order: Vec<usize> = subject_shapes
                    .iter()
                    .copied()
                    .filter(|&i| if marker_over { i > at } else { i < at })
                    .collect();
                if !out_of_order.is_empty() {
                    failures.push(format!(
                        "{ty:?} on {surface:?}, {what}: {} of its {} shapes are painted {} the label (the label's text is shape #{at})",
                        out_of_order.len(),
                        subject_shapes.len(),
                        if marker_over { "after" } else { "before" },
                    ));
                }
            }
        }
    }
    for f in &failures {
        println!("OUT OF ORDER: {f}");
    }
    println!("checked {checked} cases ({} visual control types × 3 surfaces × 4 cases; containers with a caption, a rounded corner and a child)", visual_types().len());
    assert!(failures.is_empty(), "{} cases are painted out of order", failures.len());
}
