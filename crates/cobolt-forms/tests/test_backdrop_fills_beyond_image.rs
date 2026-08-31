#![cfg(feature = "render")]
//! A form may be wider than its background image without looking cut off
//! (operator, 2026-08-31).
//!
//! The preview window is resizable and its backdrop was pinned to the FORM, so
//! dragging the window wider left everything past the form's edge unpainted:
//! the title bar kept growing while the form below it simply stopped, with the
//! IDE showing through the gap.
//!
//! `Backdrop::image_extent` separates the two questions the backdrop used to
//! answer with one rectangle — how far the COLOUR reaches, and how far the
//! IMAGE reaches. The colour covers the whole backdrop; the image stays in the
//! designed extent and keeps obeying its Mode there.

use cobolt_forms::model::BgImageMode;
use cobolt_forms::render::{paint_backdrop, Backdrop};
use egui::{Color32, Rect, Vec2};

const FILL: Color32 = Color32::from_rgb(0x10, 0x10, 0x10);

struct Painted {
    /// Solid (untextured) rects — the background colour pass.
    solids: Vec<Rect>,
    /// Textured quads — the background image.
    images: Vec<Rect>,
}

fn paint(area: Vec2, tsize: Vec2, mode: BgImageMode, extent: Option<Vec2>) -> Painted {
    let ctx = egui::Context::default();
    let tex = egui::TextureId::Managed(1);
    let mut out = ctx.run_ui(Default::default(), |ui| {
        let painter = ui.painter().clone();
        paint_backdrop(
            &painter,
            Rect::from_min_size(egui::pos2(0.0, 0.0), area),
            &Backdrop {
                paint: true,
                color_hex: "#101010".into(),
                image: Some((tex, tsize)),
                image_mode: mode,
                image_extent: extent,
                ..Default::default()
            },
        );
    });
    out.textures_delta.clear();

    let mut p = Painted { solids: Vec::new(), images: Vec::new() };
    fn walk(s: &egui::Shape, p: &mut Painted) {
        match s {
            egui::Shape::Vec(v) => v.iter().for_each(|s| walk(s, p)),
            egui::Shape::Mesh(m) if m.texture_id == egui::TextureId::Managed(1) => {
                p.images.push(m.calc_bounds())
            }
            egui::Shape::Rect(r) if r.brush.is_some() => p.images.push(r.rect),
            egui::Shape::Rect(r) if r.fill == FILL => p.solids.push(r.rect),
            _ => {}
        }
    }
    for cs in &out.shapes {
        walk(&cs.shape, &mut p);
    }
    p
}

/// Within a pixel — the engine works in floats.
fn covers(outer: Rect, inner: Rect) -> bool {
    outer.min.x <= inner.min.x + 1.0
        && outer.min.y <= inner.min.y + 1.0
        && outer.max.x + 1.0 >= inner.max.x
        && outer.max.y + 1.0 >= inner.max.y
}

#[test]
fn the_colour_covers_the_whole_window_even_though_the_image_does_not() {
    // A 1200x700 window showing a 800x600 form whose image is 800x600.
    let window = Vec2::new(1200.0, 700.0);
    let form = Vec2::new(800.0, 600.0);
    let p = paint(window, Vec2::new(800.0, 600.0), BgImageMode::Fit, Some(form));

    let whole = Rect::from_min_size(egui::pos2(0.0, 0.0), window);
    assert!(
        p.solids.iter().any(|r| covers(*r, whole)),
        "the background colour must cover the WHOLE window — that is what stops \
         the form looking cut off when the title bar grows past the image. \
         Painted solids: {:?}",
        p.solids
    );

    let designed = Rect::from_min_size(egui::pos2(0.0, 0.0), form);
    for q in &p.images {
        assert!(
            covers(designed, *q),
            "the image must stay inside the designed extent {designed:?}, but a \
             quad landed at {q:?} — it grew with the window instead"
        );
    }
    assert!(!p.images.is_empty(), "the image must still be painted");
}

#[test]
fn fit_letterboxes_inside_the_form_not_the_window() {
    // A tall image in a wide window: under Fit it must letterbox against the
    // 800x600 FORM. Fitted against the 1200x700 window it would be taller than
    // the form and spill over the controls.
    let p = paint(
        Vec2::new(1200.0, 700.0),
        Vec2::new(300.0, 600.0),
        BgImageMode::Fit,
        Some(Vec2::new(800.0, 600.0)),
    );
    assert_eq!(p.images.len(), 1, "Fit paints one quad: {:?}", p.images);
    let q = p.images[0];
    assert!(
        q.height() <= 601.0,
        "fitted against the form, the quad is at most the form's 600 high; got \
         {} — it was fitted to the window instead",
        q.height()
    );
}

#[test]
fn tiles_stop_at_the_designed_extent() {
    let p = paint(
        Vec2::new(1200.0, 700.0),
        Vec2::new(100.0, 100.0),
        BgImageMode::Tile,
        Some(Vec2::new(400.0, 300.0)),
    );
    let designed = Rect::from_min_size(egui::pos2(0.0, 0.0), Vec2::new(400.0, 300.0));
    assert!(p.images.len() > 1, "Tile still repeats: {}", p.images.len());
    for q in &p.images {
        assert!(
            covers(designed, *q),
            "a tile escaped the designed extent: {q:?} outside {designed:?}"
        );
    }
}

#[test]
fn without_an_extent_the_image_still_covers_the_whole_backdrop() {
    // The run-form and compiled-binary path: there the picture and the colour
    // cover the same rectangle, and this change must not have touched it.
    let window = Vec2::new(1200.0, 700.0);
    let p = paint(window, Vec2::new(800.0, 600.0), BgImageMode::Stretch, None);
    assert_eq!(p.images.len(), 1, "Stretch paints one quad: {:?}", p.images);
    let whole = Rect::from_min_size(egui::pos2(0.0, 0.0), window);
    assert!(
        covers(p.images[0], whole) && covers(whole, p.images[0]),
        "with no extent, Stretch still covers the whole backdrop: got {:?}, \
         expected {whole:?}",
        p.images[0]
    );
}
