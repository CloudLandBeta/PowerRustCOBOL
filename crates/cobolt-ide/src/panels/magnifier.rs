// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! The Form Designer magnifier (spec 089).
//!
//! It shows what the canvas painted, four times larger, around the pointer —
//! the canvas's own shapes, not a second rendering of the form, so it can
//! never disagree with the canvas. The canvas copies the shapes near the
//! pointer ([`capture`]); the toolbox paints them into its square view
//! ([`paint`]) through one transform. `epaint` scales positions, clips,
//! stroke widths, corner radii and blur; text is laid out again at the
//! magnified size rather than stretched, so it stays sharp. Nothing here
//! touches the form.

use egui::epaint::{ClippedShape, Shape, TextShape};
use egui::layers::ShapeIdx;
use egui::{emath::TSTransform, Context, LayerId, Pos2, Rect, Vec2};

/// How much larger the view shows the form (R3).
pub const MAGNIFICATION: f32 = 4.0;

/// How far from the pointer, in canvas pixels, shapes are copied for a view
/// `side` pixels wide: half of what the view shows, with room for a shape
/// that starts outside and reaches in.
pub fn capture_half(side: f32) -> f32 {
    side / (2.0 * MAGNIFICATION) + 16.0
}

/// What the canvas painted near the pointer, for the toolbox's view.
#[derive(Clone, Debug)]
pub struct MagnifierFeed {
    pub shapes: Vec<ClippedShape>,
    /// The pointer, on the canvas.
    pub pointer: Pos2,
    /// The form on the canvas: nothing outside it is shown (R5).
    pub form_rect: Rect,
}

/// The shape-list position the canvas pass starts at.
pub fn pass_start(ctx: &Context, layer: LayerId) -> ShapeIdx {
    ctx.graphics(|g| g.get(layer).map_or(ShapeIdx(0), |l| l.next_idx()))
}

/// Copy the shapes painted on `layer` since `start` within `half` canvas
/// pixels of `pointer` ([`capture_half`]).
pub fn capture(ctx: &Context, layer: LayerId, start: ShapeIdx, pointer: Pos2, form_rect: Rect, half: f32) -> MagnifierFeed {
    let near = Rect::from_center_size(pointer, Vec2::splat(2.0 * half));
    let shapes = ctx.graphics(|g| {
        g.get(layer).map_or_else(Vec::new, |list| {
            list.all_entries()
                .skip(start.0)
                .filter(|c| c.clip_rect.intersects(near) && c.shape.visual_bounding_rect().intersects(near))
                .cloned()
                .collect()
        })
    });
    MagnifierFeed { shapes, pointer, form_rect }
}

/// The transform that puts `pointer` at the centre of `view`, magnified.
pub fn view_transform(view: Rect, pointer: Pos2) -> TSTransform {
    TSTransform::new(view.center().to_vec2() - pointer.to_vec2() * MAGNIFICATION, MAGNIFICATION)
}

/// Paint `feed` into `view` on `layer`, magnified about the pointer and kept
/// inside the view, the form, and `clip` (what the host panel shows).
pub fn paint(ctx: &Context, layer: LayerId, view: Rect, clip: Rect, feed: &MagnifierFeed) {
    let transform = view_transform(view, feed.pointer);
    let bound = view.intersect(clip).intersect(transform.mul_rect(feed.form_rect));
    let shapes: Vec<ClippedShape> = feed
        .shapes
        .iter()
        .filter_map(|c| {
            let clip = transform.mul_rect(c.clip_rect).intersect(bound);
            if !clip.is_positive() {
                return None;
            }
            let mut shape = c.shape.clone();
            magnify_shape(ctx, &mut shape, transform);
            Some(ClippedShape { clip_rect: clip, shape })
        })
        .collect();
    ctx.graphics_mut(|g| {
        let list = g.entry(layer);
        for c in shapes {
            list.add(c.clip_rect, c.shape);
        }
    });
}

fn magnify_shape(ctx: &Context, shape: &mut Shape, transform: TSTransform) {
    match shape {
        Shape::Vec(shapes) => shapes.iter_mut().for_each(|s| magnify_shape(ctx, s, transform)),
        Shape::Text(text) => magnify_text(ctx, text, transform),
        other => other.transform(transform),
    }
}

/// Lay a text shape out again with every size in its job multiplied, so its
/// glyphs are rasterised at the magnified size (R6).
fn magnify_text(ctx: &Context, text: &mut TextShape, transform: TSTransform) {
    let z = transform.scaling;
    let mut job = (*text.galley.job).clone();
    for s in &mut job.sections {
        s.leading_space *= z;
        let f = &mut s.format;
        f.font_id.size *= z;
        f.extra_letter_spacing *= z;
        f.line_height = f.line_height.map(|h| h * z);
        f.expand_bg *= z;
        f.underline.width *= z;
        f.strikethrough.width *= z;
    }
    if job.wrap.max_width.is_finite() {
        job.wrap.max_width *= z;
    }
    job.first_row_min_height *= z;
    text.galley = ctx.fonts_mut(|f| f.layout_job(job));
    text.pos = transform * text.pos;
    text.underline.width *= z;
}

#[cfg(test)]
mod tests {
    use super::*;
    use egui::{Color32, FontId, Stroke};

    /// Paint `scene` as the canvas would, capture around `pointer`, then paint
    /// the feed into `view` and return what the view drew.
    fn magnified(pointer: Pos2, form: Rect, view: Rect, scene: impl Fn(&egui::Painter)) -> Vec<ClippedShape> {
        let ctx = Context::default();
        let mut drawn = Vec::new();
        let mut out = ctx.run_ui(egui::RawInput::default(), |ui| {
            let layer = ui.layer_id();
            let start = pass_start(ui.ctx(), layer);
            scene(ui.painter());
            let feed = capture(ui.ctx(), layer, start, pointer, form, capture_half(view.width()));
            let view_layer = LayerId::new(egui::Order::Foreground, egui::Id::new("magnifier-test"));
            paint(ui.ctx(), view_layer, view, view, &feed);
            drawn = ui.ctx().graphics(|g| g.get(view_layer).map(|l| l.all_entries().cloned().collect()).unwrap_or_default());
        });
        out.textures_delta.clear();
        drawn
    }

    const FORM: Rect = Rect { min: Pos2::new(0.0, 0.0), max: Pos2::new(400.0, 300.0) };
    const VIEW: Rect = Rect { min: Pos2::new(1000.0, 0.0), max: Pos2::new(1200.0, 200.0) };

    #[test]
    fn shapes_are_four_times_larger_about_the_pointer() {
        let drawn = magnified(Pos2::new(100.0, 100.0), FORM, VIEW, |p| {
            p.rect(
                Rect::from_min_size(Pos2::new(95.0, 95.0), Vec2::new(10.0, 10.0)),
                2,
                Color32::RED,
                Stroke::new(1.0, Color32::BLACK),
                egui::StrokeKind::Inside,
            );
        });
        let Shape::Rect(r) = &drawn[0].shape else { panic!("{:?}", drawn[0].shape) };
        assert_eq!(r.rect, Rect::from_center_size(VIEW.center(), Vec2::splat(40.0)), "centred on the view, 4x");
        assert_eq!((r.stroke.width, r.corner_radius.nw), (4.0, 8));
        assert!(VIEW.contains_rect(drawn[0].clip_rect));
    }

    /// AC3 — a 9 px label under the pointer is laid out at 36 px.
    #[test]
    fn text_is_laid_out_again_at_four_times() {
        let drawn = magnified(Pos2::new(60.0, 40.0), FORM, VIEW, |p| {
            p.text(Pos2::new(50.0, 35.0), egui::Align2::LEFT_TOP, "Label", FontId::proportional(9.0), Color32::WHITE);
        });
        let Shape::Text(t) = &drawn[0].shape else { panic!("{:?}", drawn[0].shape) };
        let size = t.galley.job.sections[0].format.font_id.size;
        println!("089 AC3: a 9 px label in the magnifier: galley font {size} px");
        assert_eq!(size, 36.0);
        assert_eq!(t.pos, VIEW.center() + Vec2::new(-40.0, -20.0));
    }

    /// AC4/R5 — near the form's edge, what lies outside the form is clipped
    /// away; far from the pointer, nothing is copied at all.
    #[test]
    fn only_the_form_is_shown() {
        let drawn = magnified(Pos2::new(395.0, 150.0), FORM, VIEW, |p| {
            p.rect_filled(Rect::from_min_size(Pos2::new(380.0, 140.0), Vec2::new(60.0, 20.0)), 0, Color32::RED);
            p.rect_filled(Rect::from_min_size(Pos2::new(0.0, 0.0), Vec2::new(5.0, 5.0)), 0, Color32::BLUE);
        });
        assert_eq!(drawn.len(), 1, "the far shape is not copied");
        let form_right_in_view = VIEW.center().x + (400.0 - 395.0) * MAGNIFICATION;
        assert!(drawn[0].clip_rect.max.x <= form_right_in_view + 1e-3, "clipped at the form's edge: {:?}", drawn[0].clip_rect);
    }
}
