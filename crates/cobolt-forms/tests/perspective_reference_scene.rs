// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Spec 096, task T15 — **the reference scene, pictured.**
//!
//! The operator's reference is a *Revenue by quarter* bar chart drawn as a card
//! tilted away from the viewer — its left edge farther than its right — over a dark
//! backdrop. This builds that scene, pictures it with the engine that draws Preview,
//! Run Form and a built program (`snapshot::render_form_image`, the CPU rasteriser that
//! fills egui's own triangles), and checks in numbers what a person looking at it sees:
//!
//! * the picture differs from the flat one;
//! * on the screen, the chart's left edge is shorter than its right, by the ratio the
//!   projection promises;
//! * the picture is saved when `PRC_PERSPECTIVE_PICTURES=<folder>` names one, so the
//!   author can look at it (the Guide's picture is made this way).
//!
//! Run with:
//! `PRC_PERSPECTIVE_PICTURES=/some/folder cargo test -p cobolt-forms --features render --test perspective_reference_scene -- --nocapture`
//!
//! Spec 096 AC12 (R7, R17) in part.

#![cfg(feature = "render")]

use cobolt_forms::model::{Control, ControlType, Form, PropValue};
use cobolt_forms::perspective::{edge_lengths, Perspective};
use cobolt_forms::snapshot::{render_form_image, SnapshotOptions};
use egui::{pos2, Color32, ColorImage};

const BACKDROP: &str = "#0E1522";

/// The reference scene, tilted (`Some`) or flat (`None`).
fn scene(tilt: Option<(i64, i64, i64)>) -> Form {
    let mut form = Form::new("PERSPECTIVE-REVENUE", "Revenue", 760, 460);
    form.background_color = BACKDROP.into();
    let mut chart = Control::new("CHART-REVENUE", ControlType::BarChart, 130, 50);
    chart.rect = cobolt_forms::Rect::new(130, 50, 500, 340);
    chart.set_prop("Title", PropValue::String("Revenue by quarter".into()));
    chart.set_prop(
        "__ChartData",
        PropValue::String("Q1\t120\t96\nQ2\t150\t128\nQ3\t135\t172\nQ4\t190\t204".into()),
    );
    chart.set_prop("SeriesLabels", PropValue::String("2025,2026".into()));
    chart.set_prop("ShadowEnabled", PropValue::Bool(true));
    chart.set_prop("ShadowDistance", PropValue::Int(14));
    chart.set_prop("ShadowOpacity", PropValue::Int(35));
    if let Some((rx, ry, d)) = tilt {
        chart.set_prop("RotateX", PropValue::Int(rx));
        chart.set_prop("RotateY", PropValue::Int(ry));
        chart.set_prop("PerspectiveDistance", PropValue::Int(d));
    }
    form.controls.push(chart);
    form
}

fn px(img: &ColorImage, x: usize, y: usize) -> Color32 {
    img.pixels[y * img.size[0] + x]
}

/// Whether a pixel is the backdrop (within a few levels), i.e. nothing is drawn there.
fn is_backdrop(c: Color32) -> bool {
    let b = [0x0E, 0x15, 0x22];
    c.r().abs_diff(b[0]) < 10 && c.g().abs_diff(b[1]) < 10 && c.b().abs_diff(b[2]) < 10
}

/// The vertical extent of the chart's face in column `x`: the first and last row whose pixel is not
/// the backdrop and is not the faint drop shadow (a shadow stays near the backdrop's darkness).
fn face_extent(img: &ColorImage, x: usize) -> Option<(usize, usize)> {
    let bright = |c: Color32| u32::from(c.r()) + u32::from(c.g()) + u32::from(c.b()) > 3 * 60;
    let rows: Vec<usize> = (0..img.size[1]).filter(|&y| !is_backdrop(px(img, x, y)) && bright(px(img, x, y))).collect();
    Some((*rows.first()?, *rows.last()?))
}

fn save(img: &ColorImage, name: &str) {
    let Some(dir) = std::env::var_os("PRC_PERSPECTIVE_PICTURES") else {
        return;
    };
    let dir = std::path::PathBuf::from(dir);
    std::fs::create_dir_all(&dir).unwrap();
    let png = cobolt_forms::raster::to_png(img).expect("the picture encodes");
    std::fs::write(dir.join(name), png).unwrap();
    println!("  picture written: {}", dir.join(name).display());
}

#[test]
fn the_reference_scene_is_pictured_tilted_with_its_left_edge_farther() {
    let opts = SnapshotOptions::default();
    let flat = render_form_image(&scene(None), &opts);
    let tilted_form = scene(Some((8, -30, 60)));
    let turned = render_form_image(&tilted_form, &opts);
    assert_eq!(flat.size, turned.size);
    save(&flat, "perspective-flat.png");
    save(&turned, "perspective-revenue.png");

    // The picture changed where the chart is.
    let differing = flat
        .pixels
        .iter()
        .zip(&turned.pixels)
        .filter(|(a, b)| a.r().abs_diff(b.r()) > 24 || a.g().abs_diff(b.g()) > 24 || a.b().abs_diff(b.b()) > 24)
        .count();
    println!("\n  ── the reference scene: RotateX 8, RotateY −30, PerspectiveDistance 60 ──");
    println!("  {} × {} px; {} of {} pixels differ from the flat chart", flat.size[0], flat.size[1], differing, flat.pixels.len());
    assert!(differing * 100 / flat.pixels.len() >= 5, "the chart looks flat");

    // On the screen the left edge is shorter than the right, by the ratio the model promises.
    let rect = egui::Rect::from_min_size(pos2(130.0, 50.0), egui::vec2(500.0, 340.0));
    let quad = Perspective::new(8, -30, 60).quad(rect);
    let [_, right, _, left] = edge_lengths(&quad);
    println!("  projected outline: left edge {left:.1} px, right edge {right:.1} px, ratio {:.3}", left / right);
    assert!(left < right, "RotateY −30 moves the LEFT edge away");

    let (lx, rx) = (quad[0].x.max(quad[3].x) as usize + 6, quad[1].x.min(quad[2].x) as usize - 6);
    let (lt, lb) = face_extent(&turned, lx).expect("the chart is there on the left");
    let (rt, rb) = face_extent(&turned, rx).expect("the chart is there on the right");
    let (seen_left, seen_right) = ((lb - lt) as f32, (rb - rt) as f32);
    // The columns sampled sit a little inside each edge, so compare the ratio of what is seen with
    // the ratio of the edges, within a few percent.
    println!(
        "  seen in the picture: column x={lx} spans {seen_left:.0} px, column x={rx} spans {seen_right:.0} px, ratio {:.3}",
        seen_left / seen_right
    );
    assert!(seen_left < seen_right, "in the picture, the chart is shorter on the left");
    let model = left / right;
    let seen = seen_left / seen_right;
    assert!((seen - model).abs() < 0.08, "seen ratio {seen:.3} vs the projection's {model:.3}");
}

#[test]
fn the_scene_picture_is_deterministic() {
    let opts = SnapshotOptions::default();
    let form = scene(Some((8, -30, 60)));
    let (a, b) = (render_form_image(&form, &opts), render_form_image(&form, &opts));
    assert_eq!(a.pixels, b.pixels, "the same scene pictures the same bytes");
    println!("\n  the tilted scene pictures identically twice ({} px)", a.pixels.len());
}

// ── Spec 097: the three strengths side by side ───────────────────────────────

/// Three charts, one tilt, three strengths: 100 (the perspective the distance gives), 50 and 0 (none).
fn strengths_scene() -> Form {
    let mut form = Form::new("PERSPECTIVE-STRENGTH", "Strength", 1020, 470);
    form.background_color = BACKDROP.into();
    for (i, strength) in [100i64, 50, 0].into_iter().enumerate() {
        let mut chart = Control::new(format!("CHART-{i}"), ControlType::BarChart, 20 + i as i32 * 335, 110);
        chart.rect = cobolt_forms::Rect::new(20 + i as i32 * 335, 110, 300, 240);
        chart.set_prop("Title", PropValue::String(format!("Strength {strength}")));
        chart.set_prop("__ChartData", PropValue::String("Q1\t120\t96\nQ2\t150\t128\nQ3\t135\t172\nQ4\t190\t204".into()));
        chart.set_prop("SeriesLabels", PropValue::String("2025,2026".into()));
        chart.set_prop("AnimateOnLoad", PropValue::Bool(false));
        chart.set_prop("RotateX", PropValue::Int(8));
        chart.set_prop("RotateY", PropValue::Int(-38));
        chart.set_prop("PerspectiveDistance", PropValue::Int(65));
        chart.set_prop("PerspectiveStrength", PropValue::Int(strength));
        form.controls.push(chart);
    }
    form
}

#[test]
fn the_three_strengths_are_pictured_side_by_side_and_the_far_side_grows_with_the_strength() {
    let form = strengths_scene();
    let img = render_form_image(&form, &SnapshotOptions::default());
    save(&img, "perspective-strength.png");

    println!("\n  ── spec 097: RotateY −38, RotateX 8, distance 65, three strengths ──");
    println!("  strength   left edge   right edge   far/near   seen in the picture (column spans)");
    let mut model = Vec::new();
    let mut seen = Vec::new();
    for (i, strength) in [100i64, 50, 0].into_iter().enumerate() {
        let rect = egui::Rect::from_min_size(pos2(20.0 + i as f32 * 335.0, 110.0), egui::vec2(300.0, 240.0));
        let quad = Perspective::new(8, -38, 65).with_strength(strength).quad(rect);
        let [_, right, _, left] = edge_lengths(&quad);
        // What the picture shows: the vertical extent of the chart's face near each side of its outline.
        let (lx, rx) = (quad[0].x.max(quad[3].x) as usize + 6, quad[1].x.min(quad[2].x) as usize - 6);
        let (lt, lb) = face_extent(&img, lx).expect("chart on the left");
        let (rt, rb) = face_extent(&img, rx).expect("chart on the right");
        let (sl, sr) = ((lb - lt) as f32, (rb - rt) as f32);
        println!("  {strength:>8}   {left:>9.1}   {right:>10.1}   {:>8.3}   {sl:.0} / {sr:.0} = {:.3}", left / right, sl / sr);
        model.push(left / right);
        seen.push(sl / sr);
    }
    // The nearer the strength is to 0, the closer the two sides are to equal — in the model and in the picture.
    assert!(model[0] < model[1] && model[1] < model[2], "far/near must approach 1 as the strength falls: {model:?}");
    assert!(seen[0] < seen[1] && seen[1] < seen[2], "…and so must what is seen: {seen:?}");
    assert!((model[2] - 1.0).abs() < 1e-3, "at strength 0 the two sides are equal");
}
