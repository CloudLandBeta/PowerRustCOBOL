// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Spec 058 T30 / **AC11** — a Viewer paints identically on the designer
//! canvas and in the running form, for **every format wave**.
//!
//! Spec 017's rule is that both surfaces go through one painter. For this
//! control that is `paint::draw_viewer`, reached from
//! `draw_control_body`'s `CT::Viewer` branch on the canvas and from
//! `render_interactive`'s own arm when running — with the state built by
//! **one** `ViewerPaintState::from_control` either way, which is what makes
//! parity a property of the design rather than something a test polices
//! after the fact.
//!
//! What can still drift is a surface deciding to paint something extra of
//! its own (the split divider did, briefly, and this is what would have
//! caught it). So the comparison is over **positions and fills**, never a
//! shape count: two paints can emit the same number of rectangles and put
//! them in different places in different colours.

use cobolt_forms::model::{Control, ControlType, PropValue, Rect as MRect};
use cobolt_forms::render::{render_form, Backdrop, FormState, RenderInput, RenderMode};
use egui::{Color32, Pos2, Rect, Vec2};

struct Designed;
impl FormState for Designed {}

fn fill_of(s: &egui::Shape) -> Color32 {
    match s {
        egui::Shape::Rect(r) => r.fill,
        egui::Shape::Circle(c) => c.fill,
        egui::Shape::Path(p) => p.fill,
        egui::Shape::Text(t) => t.fallback_color,
        egui::Shape::LineSegment { stroke, .. } => stroke.color,
        _ => Color32::TRANSPARENT,
    }
}

/// Every `(rect, fill)` a form paints, in order, on the given surface.
fn painted(controls: &[Control], mode: RenderMode) -> Vec<(Rect, Color32)> {
    let ctx = egui::Context::default();
    let active = cobolt_forms::containers::ActiveTabs::new();
    // Two frames: the first lays widgets out, and an interactive surface
    // needs that before it can report what it drew.
    let mut out_shapes = Vec::new();
    for _ in 0..2 {
        out_shapes.clear();
        let mut input = egui::RawInput::default();
        input.screen_rect = Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(760.0, 560.0)));
        let mut full = ctx.run_ui(input, |root| {
            egui::CentralPanel::default().frame(egui::Frame::NONE).show_inside(root, |ui| {
                let rin = RenderInput {
                    controls,
                    state: &Designed,
                    form_size: Vec2::new(700.0, 500.0),
                    glass: true,
                    mode,
                    active_tabs: &active,
                    backdrop: Backdrop::default(),
                };
                let _ = render_form(ui, &rin);
            });
        });
        full.textures_delta.clear();
        fn collect(s: &egui::Shape, out: &mut Vec<(Rect, Color32)>) {
            match s {
                egui::Shape::Vec(v) => v.iter().for_each(|s| collect(s, out)),
                other => out.push((other.visual_bounding_rect(), fill_of(other))),
            }
        }
        for cs in &full.shapes {
            collect(&cs.shape, &mut out_shapes);
        }
    }
    out_shapes
}

fn viewer(source: &str, layout: &str, extra: &[(&str, PropValue)]) -> Vec<Control> {
    let mut c = Control::new("VWR-1", ControlType::Viewer, 20, 20);
    c.rect = MRect::new(20, 20, 640, 440);
    c.set_prop("Source", PropValue::String(source.into()));
    c.set_prop("View1Source", PropValue::String(source.into()));
    c.set_prop("Layout", PropValue::String(layout.into()));
    for (k, v) in extra {
        c.set_prop(*k, v.clone());
    }
    vec![c]
}

fn write(dir: &tempfile::TempDir, name: &str, bytes: &[u8]) -> String {
    let path = dir.path().join(name);
    std::fs::write(&path, bytes).unwrap();
    path.to_string_lossy().into_owned()
}

/// A minimal real PDF, written by a real PDF writer.
fn sample_pdf(dir: &tempfile::TempDir) -> String {
    use lopdf::{dictionary, Document, Object, Stream};
    let mut doc = Document::with_version("1.5");
    let pages_id = doc.new_object_id();
    let font = doc.add_object(dictionary! { "Type" => "Font", "Subtype" => "Type1", "BaseFont" => "Helvetica" });
    let resources = doc.add_object(dictionary! { "Font" => dictionary! { "F1" => font } });
    let content = doc.add_object(Stream::new(
        dictionary! {},
        b"BT /F1 18 Tf 72 700 Td (Parity across surfaces) Tj ET\n".to_vec(),
    ));
    let page = doc.add_object(dictionary! {
        "Type" => "Page", "Parent" => pages_id, "Contents" => content, "Resources" => resources,
        "MediaBox" => vec![0.into(), 0.into(), 612.into(), 792.into()],
    });
    doc.objects.insert(
        pages_id,
        Object::Dictionary(dictionary! { "Type" => "Pages", "Kids" => vec![page.into()], "Count" => 1 }),
    );
    let catalog = doc.add_object(dictionary! { "Type" => "Catalog", "Pages" => pages_id });
    doc.trailer.set("Root", catalog);
    let mut bytes = Vec::new();
    doc.save_to(&mut bytes).unwrap();
    write(dir, "parity.pdf", &bytes)
}

/// A 4x3 PNG, so the image wave is a real decode rather than a placeholder.
fn sample_png(dir: &tempfile::TempDir) -> String {
    use image::{ImageBuffer, Rgba};
    let img: ImageBuffer<Rgba<u8>, Vec<u8>> =
        ImageBuffer::from_fn(4, 3, |x, y| Rgba([(x * 60) as u8, (y * 80) as u8, 128, 255]));
    let mut bytes = Vec::new();
    image::DynamicImage::ImageRgba8(img)
        .write_to(&mut std::io::Cursor::new(&mut bytes), image::ImageFormat::Png)
        .unwrap();
    write(dir, "parity.png", &bytes)
}

/// **AC11**, across every format wave that has landed: text, Markdown
/// (with a Mermaid diagram in it), an image, a PDF and the HTML subset —
/// each in a paged layout so the paper border and shadow are in the
/// comparison too.
#[test]
fn a_viewer_paints_the_same_on_the_designer_canvas_and_in_a_running_form() {
    let dir = tempfile::tempdir().unwrap();
    let text = write(&dir, "parity.txt", b"A plain text document.\nWith two lines.\n");
    let md = write(
        &dir,
        "parity.md",
        b"# Heading\n\nA **paragraph** with a [link](/x).\n\n| A | B |\n|---|---|\n| 1 | 2 |\n\n```mermaid\nflowchart LR\n  A-->B\n```\n",
    );
    let html = write(
        &dir,
        "parity.html",
        b"<h1>Title</h1><p>Body with <em>emphasis</em>.</p><ul><li>one</li></ul>",
    );
    let png = sample_png(&dir);
    let pdf = sample_pdf(&dir);

    let cases: Vec<(&str, String, &str)> = vec![
        ("text / Page", text, "Page"),
        ("text / Raw", md.clone(), "Raw"),
        ("Markdown+Mermaid / Web", md, "Web"),
        ("image / Print", png, "Print"),
        ("PDF / Page", pdf, "Page"),
        ("HTML subset / Web", html, "Web"),
    ];

    for (name, source, layout) in cases {
        let controls = viewer(&source, layout, &[]);
        let canvas = painted(&controls, RenderMode::Static);
        let running = painted(&controls, RenderMode::Interactive);
        println!(
            "{name:<24} canvas {:>4} shapes, running {:>4} shapes",
            canvas.len(),
            running.len()
        );
        assert!(!canvas.is_empty(), "{name}: the canvas must paint SOMETHING");
        assert_eq!(
            canvas.len(),
            running.len(),
            "{name}: the two surfaces painted different numbers of shapes"
        );
        let mut compared = 0usize;
        for (i, (c, r)) in canvas.iter().zip(running.iter()).enumerate() {
            assert_eq!(c.1, r.1, "{name}: shape {i} is {:?} on the canvas and {:?} when running", c.1, r.1);
            // An empty shape reports infinite bounds and has no position to
            // compare; its fill above is all there is to check.
            if !c.0.is_finite() || !r.0.is_finite() {
                assert_eq!(
                    c.0.is_finite(),
                    r.0.is_finite(),
                    "{name}: shape {i} is empty on one surface and not the other"
                );
                continue;
            }
            assert!(
                (c.0.min - r.0.min).length() < 0.5 && (c.0.max - r.0.max).length() < 0.5,
                "{name}: shape {i} is at {:?} on the canvas and {:?} when running",
                c.0,
                r.0
            );
            compared += 1;
        }
        assert!(compared > 4, "{name}: only {compared} positioned shapes — nothing was really compared");
    }
}

/// The same check with the control SPLIT — the divider is the one thing
/// each surface used to draw for itself, and this is what keeps it to one
/// drawing.
#[test]
fn a_split_viewer_paints_the_same_on_both_surfaces_divider_included() {
    let dir = tempfile::tempdir().unwrap();
    let a = write(&dir, "left.txt", b"The left-hand document.\n");
    let b = write(&dir, "right.txt", b"The right-hand one.\n");
    for mode in ["LeftRight", "TopBottom"] {
        let controls = viewer(
            &a,
            "Raw",
            &[
                ("SplitMode", PropValue::String(mode.into())),
                ("SplitPercent", PropValue::Int(40)),
                ("View2Source", PropValue::String(b.clone())),
            ],
        );
        let canvas = painted(&controls, RenderMode::Static);
        let running = painted(&controls, RenderMode::Interactive);
        println!("split {mode:<10} canvas {:>4} shapes, running {:>4} shapes", canvas.len(), running.len());
        assert_eq!(canvas.len(), running.len(), "split {mode}: shape counts differ");
        for (i, (c, r)) in canvas.iter().zip(running.iter()).enumerate() {
            assert_eq!(c.1, r.1, "split {mode}: shape {i} fill differs");
            if !c.0.is_finite() || !r.0.is_finite() {
                continue;
            }
            assert!(
                (c.0.min - r.0.min).length() < 0.5,
                "split {mode}: shape {i} at {:?} vs {:?}",
                c.0,
                r.0
            );
        }
    }
}
