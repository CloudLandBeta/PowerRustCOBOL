// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Pure graphical (paint-only) renderer for form control appearance.
//!
//! This module is the single source of truth for control look-and-feel (frosted
//! glass, custom Slider, charts, etc.). Extracted from the Form Designer per
//! specs/003-unified-control-rendering.
//!
//! All rendering paths (designer canvas with dev overlays, RAD preview, live
//! Run Form, and compiled binaries) must delegate graphical element drawing
//! here.
//!
//! Dev-only affordances (selection chrome, handles, geometry mutation) are
//! gated by the `selected` parameter (always pass `false` for production
//! / preview / binary) and by caller discipline. The paint functions themselves
//! perform **only reads** (`get_prop`, rect) + egui draw calls — zero side
//! effects or mutations.

use crate::model::PropValue;
use crate::theme_pack::{ControlState, Slice, ThemePack};
use crate::{Control, ControlType};
use egui::text::{LayoutJob, TextFormat};
use egui::{Color32, Pos2, Rect, Stroke, Vec2};
use std::collections::HashMap;
use std::f32::consts::TAU;
use std::sync::{Arc, Mutex, OnceLock};

// ── Public API (the designer-derived appearance) ─────────────────────────────

/// Convert a model-space `f32` corner radius to egui 0.31+'s `u8` unit,
/// rounding to the nearest pixel and clamping to the representable range.
/// The `.cfrm` model keeps radii as `f32`; the conversion happens only here,
/// at the paint edge.
pub fn cr8(v: f32) -> u8 {
    v.round().clamp(0.0, 255.0) as u8
}

/// Scale `base` uniformly about its centre by `scale` (1.0 = unchanged).
/// Shared by the designer canvas, the preview window and the run form so that
/// zoom/spin/flip animations resize controls identically everywhere.
pub fn scale_rect_about_center(base: Rect, scale: f32) -> Rect {
    if (scale - 1.0).abs() < 0.001 {
        base
    } else {
        Rect::from_center_size(base.center(), base.size() * scale)
    }
}

/// Computes the visual thumb (knob) rect for a slider, using the same geometry
/// as the glass drawing. Used for hit-testing "did the drag start over the knob".
pub fn slider_thumb_rect(screen_rect: Rect, min: f32, max: f32, val: f32, vertical: bool) -> Rect {
    let range = (max - min).max(1.0);
    let pct = ((val - min) / range).clamp(0.0, 1.0);

    if vertical {
        let track_half_w = (screen_rect.width() * 0.18).clamp(4.0, 12.0);
        let cx = screen_rect.center().x;
        let track_t = screen_rect.min.y + 10.0;
        let track_b = screen_rect.max.y - 10.0;
        let track_h = (track_b - track_t).max(1.0);
        let thumb_y = track_b - pct * track_h;
        let thumb_h = (track_half_w * 2.0 * 1.6).clamp(16.0, 32.0);
        let thumb_w = track_half_w * 2.0 + 6.0;
        Rect::from_center_size(Pos2::new(cx, thumb_y), Vec2::new(thumb_w, thumb_h))
    } else {
        let track_half_h = (screen_rect.height() * 0.18).clamp(4.0, 12.0);
        let cy = screen_rect.center().y;
        let track_l = screen_rect.min.x + 10.0;
        let track_r = screen_rect.max.x - 10.0;
        let track_w = (track_r - track_l).max(1.0);
        let thumb_x = track_l + pct * track_w;
        let thumb_w_half = (track_half_h * 1.6).clamp(8.0, 20.0);
        let thumb_h = track_half_h * 2.0 + 6.0;
        Rect::from_center_size(
            Pos2::new(thumb_x, cy),
            Vec2::new(thumb_w_half * 2.0, thumb_h),
        )
    }
}

/// Snapshot live/runtime string props into a transient Control so that
/// draw_control can be used for exact designed appearance (WYSIWYG).
/// Moved fully here (per plan) so both IDE runtime and compiler binary can use it.
pub fn live_control<'a>(
    id: &str,
    ct: ControlType,
    size: Vec2,
    props: impl IntoIterator<Item = (&'a String, &'a String)>,
) -> Control {
    let mut c = Control::new(id, ct, 0, 0);
    c.rect = crate::model::Rect::new(0, 0, size.x.round() as i32, size.y.round() as i32);
    for (k, v) in props {
        c.properties.insert(k.clone(), PropValue::String(v.clone()));
    }
    c
}

/// Map alignment string (used by labels). "Justified" lays out left-anchored;
/// justification itself is a layout-job flag (see `justified_halign`).
pub fn text_halign(value: &str) -> egui::Align {
    let v = value.trim();
    if v.eq_ignore_ascii_case("Center") || v.ends_with("Center") {
        egui::Align::Center
    } else if v.eq_ignore_ascii_case("Right") || v.ends_with("Right") {
        egui::Align::RIGHT
    } else {
        egui::Align::LEFT
    }
}

/// True when the alignment string asks for justified text (wrapped lines
/// stretched to the full width; the last line stays natural).
pub fn text_justified(value: &str) -> bool {
    value.trim().eq_ignore_ascii_case("Justified")
}

/// Map a VerticalAlignment string (Top / Middle / Bottom). Anything else —
/// including the empty string on forms that predate the property — is Middle,
/// preserving the historical centred single line.
pub fn text_valign(value: &str) -> egui::Align {
    let v = value.trim();
    if v.eq_ignore_ascii_case("Top") {
        egui::Align::TOP
    } else if v.eq_ignore_ascii_case("Bottom") {
        egui::Align::BOTTOM
    } else {
        egui::Align::Center
    }
}

fn styled_text_job(
    painter: &egui::Painter,
    ctrl: &Control,
    text: &str,
    font_name: &str,
    fsize: f32,
    color: Color32,
    max_width: f32,
    halign: egui::Align,
) -> LayoutJob {
    let font_id = crate::fonts::font_id(painter.ctx(), font_name, fsize);
    let underline = ctrl
        .get_prop("Underline")
        .map(|v| v.as_bool())
        .unwrap_or(false);
    let strikeout = ctrl
        .get_prop("Strikethrough")
        .map(|v| v.as_bool())
        .unwrap_or(false);

    let mut job = LayoutJob::default();
    job.halign = halign;
    job.wrap.max_width = max_width;
    job.wrap.break_anywhere = false;
    job.append(
        text,
        0.0,
        TextFormat {
            font_id,
            color,
            italics: ctrl
                .get_prop("Italic")
                .map(|v| v.as_bool())
                .unwrap_or(false),
            underline: if underline {
                Stroke::new(1.0, color)
            } else {
                Stroke::NONE
            },
            strikethrough: if strikeout {
                Stroke::new(1.0, color)
            } else {
                Stroke::NONE
            },
            ..Default::default()
        },
    );
    job
}

fn paint_styled_galley(
    painter: &egui::Painter,
    ctrl: &Control,
    pos: Pos2,
    galley: std::sync::Arc<egui::Galley>,
    color: Color32,
) {
    painter.galley(pos, galley.clone(), color);
    if ctrl.get_prop("Bold").map(|v| v.as_bool()).unwrap_or(false) {
        // Egui does not guarantee a registered bold face for arbitrary system
        // fonts. Repaint with a tiny offset to make the weight visibly heavier.
        painter.galley(pos + Vec2::new(0.5, 0.0), galley, color);
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ButtonImageAlignment {
    Left,
    Right,
    Top,
    Bottom,
}

fn button_image_alignment(value: &str) -> ButtonImageAlignment {
    let v = value.trim();
    if v.eq_ignore_ascii_case("Right") || v.ends_with("Right") {
        ButtonImageAlignment::Right
    } else if v.eq_ignore_ascii_case("Top") || v.starts_with("Top") {
        ButtonImageAlignment::Top
    } else if v.eq_ignore_ascii_case("Bottom") || v.starts_with("Bottom") {
        ButtonImageAlignment::Bottom
    } else {
        ButtonImageAlignment::Left
    }
}

fn button_image_padding(ctrl: &Control) -> f32 {
    ctrl.get_prop("IconPadding")
        .or_else(|| ctrl.get_prop("ImagePadding"))
        .map(|v| v.as_i64() as f32)
        .unwrap_or(10.0)
        .clamp(0.0, 64.0)
}

fn button_icon_size(ctrl: &Control) -> f32 {
    ctrl.get_prop("IconSize")
        .map(|v| v.as_i64() as f32)
        .unwrap_or(32.0)
        .clamp(16.0, 128.0)
}

fn button_content_anchor_with_inset(
    rect: Rect,
    content_size: Vec2,
    text_alignment: &str,
    x_inset: f32,
    y_inset: f32,
) -> Pos2 {
    let h = text_halign(text_alignment);
    let v = if text_alignment.starts_with("Top") || text_alignment.eq_ignore_ascii_case("Top") {
        egui::Align::Min
    } else if text_alignment.starts_with("Bottom") || text_alignment.eq_ignore_ascii_case("Bottom")
    {
        egui::Align::Max
    } else {
        egui::Align::Center
    };
    let x = match h {
        egui::Align::Center => rect.center().x - content_size.x * 0.5,
        egui::Align::Max => rect.right() - x_inset - content_size.x,
        _ => rect.left() + x_inset,
    };
    let y = match v {
        egui::Align::Center => rect.center().y - content_size.y * 0.5,
        egui::Align::Max => rect.bottom() - y_inset - content_size.y,
        _ => rect.top() + y_inset,
    };
    Pos2::new(
        x.clamp(
            rect.left() + x_inset,
            (rect.right() - x_inset - content_size.x).max(rect.left() + x_inset),
        ),
        y.clamp(
            rect.top() + y_inset,
            (rect.bottom() - y_inset - content_size.y).max(rect.top() + y_inset),
        ),
    )
}

fn button_content_anchor(rect: Rect, content_size: Vec2, text_alignment: &str, inset: f32) -> Pos2 {
    button_content_anchor_with_inset(rect, content_size, text_alignment, inset, inset)
}

fn button_image_size(_native: Vec2, slot: Vec2) -> Vec2 {
    if slot.x <= 0.0 || slot.y <= 0.0 {
        return Vec2::ZERO;
    }
    slot
}

fn button_image_slot(ctrl: &Control) -> Vec2 {
    let size = button_icon_size(ctrl);
    Vec2::new(size, size)
}

fn button_svg_icon_available(path: &str) -> bool {
    is_svg_path(path) && std::fs::metadata(path).is_ok()
}

fn button_content_layout(
    rect: Rect,
    text_size: Vec2,
    image_size: Option<Vec2>,
    image_alignment: ButtonImageAlignment,
    padding: f32,
    text_alignment: &str,
) -> (Pos2, Option<Rect>) {
    let inset = 6.0_f32.min(rect.width() * 0.25).min(rect.height() * 0.25);
    let text_pos = button_content_anchor(rect, text_size, text_alignment, inset);
    let Some(img_size) = image_size.filter(|s| s.x > 0.0 && s.y > 0.0) else {
        return (text_pos, None);
    };
    let has_text = text_size.x > 0.0 && text_size.y > 0.0;
    let gap = if has_text { padding } else { 0.0 };
    let (text_pos, image_rect) = match image_alignment {
        ButtonImageAlignment::Left => {
            let avail_w = (rect.width() - inset * 2.0).max(1.0);
            let avail_h = (rect.height() - inset * 2.0).max(1.0);
            let max_icon_w = (avail_w - gap - text_size.x).max(0.0);
            let img_size = Vec2::new(img_size.x.min(max_icon_w), img_size.y.min(avail_h));
            let pair_size = Vec2::new(img_size.x + gap + text_size.x, img_size.y.max(text_size.y));
            let origin = button_content_anchor(rect, pair_size, text_alignment, inset);
            (
                Pos2::new(
                    origin.x + img_size.x + gap,
                    origin.y + (pair_size.y - text_size.y) * 0.5,
                ),
                Rect::from_min_size(
                    Pos2::new(origin.x, origin.y + (pair_size.y - img_size.y) * 0.5),
                    img_size,
                ),
            )
        }
        ButtonImageAlignment::Right => {
            let avail_w = (rect.width() - inset * 2.0).max(1.0);
            let avail_h = (rect.height() - inset * 2.0).max(1.0);
            let max_icon_w = (avail_w - gap - text_size.x).max(0.0);
            let img_size = Vec2::new(img_size.x.min(max_icon_w), img_size.y.min(avail_h));
            let pair_size = Vec2::new(text_size.x + gap + img_size.x, text_size.y.max(img_size.y));
            let origin = button_content_anchor(rect, pair_size, text_alignment, inset);
            (
                Pos2::new(origin.x, origin.y + (pair_size.y - text_size.y) * 0.5),
                Rect::from_min_size(
                    Pos2::new(
                        origin.x + text_size.x + gap,
                        origin.y + (pair_size.y - img_size.y) * 0.5,
                    ),
                    img_size,
                ),
            )
        }
        ButtonImageAlignment::Top => {
            let image_rect = Rect::from_min_size(
                Pos2::new(rect.center().x - img_size.x * 0.5, rect.top() + inset),
                img_size,
            );
            let text_top = (image_rect.bottom() + gap).min(rect.bottom() - inset);
            let text_rect =
                Rect::from_min_max(Pos2::new(rect.left(), text_top), rect.right_bottom());
            (
                button_content_anchor_with_inset(text_rect, text_size, text_alignment, inset, 0.0),
                image_rect,
            )
        }
        ButtonImageAlignment::Bottom => {
            let image_rect = Rect::from_min_size(
                Pos2::new(
                    rect.center().x - img_size.x * 0.5,
                    rect.bottom() - inset - img_size.y,
                ),
                img_size,
            );
            let text_bottom = (image_rect.top() - gap).max(rect.top() + inset);
            let text_rect =
                Rect::from_min_max(rect.left_top(), Pos2::new(rect.right(), text_bottom));
            (
                button_content_anchor_with_inset(text_rect, text_size, text_alignment, inset, 0.0),
                image_rect,
            )
        }
    };
    (text_pos, Some(image_rect))
}

fn draw_control_border(
    painter: &egui::Painter,
    rect: Rect,
    rounding: egui::CornerRadius,
    style: &str,
    width: f32,
    color: Color32,
) {
    let bw = width.clamp(0.0, 20.0);
    if bw <= 0.5 || style.eq_ignore_ascii_case("None") {
        return;
    }
    let style_l = style.trim().to_ascii_lowercase();
    if style_l == "single" {
        // StrokeKind::Inside keeps the whole stroke within `rect` at the exact
        // integer face radius — no fractional concentric radius (inexpressible
        // in egui>=0.31's u8) and no outward spill (spec 027 corner bleed).
        painter.rect_stroke(
            rect,
            rounding,
            Stroke::new(bw, color),
            egui::StrokeKind::Inside,
        );
        return;
    }

    let light = shade(color, 0.35);
    let dark = shade(color, -0.35);
    let inset = bw * 0.5;
    let r = rect.shrink(inset);
    let top_left_light = style_l == "raised" || style_l == "fixed3d" || style_l == "3d";
    let (top_left, bottom_right) = if top_left_light {
        (light, dark)
    } else {
        (dark, light)
    };
    let stroke_tl = Stroke::new(bw, top_left);
    let stroke_br = Stroke::new(bw, bottom_right);
    painter.line_segment([r.left_top(), r.right_top()], stroke_tl);
    painter.line_segment([r.left_top(), r.left_bottom()], stroke_tl);
    painter.line_segment([r.right_top(), r.right_bottom()], stroke_br);
    painter.line_segment([r.left_bottom(), r.right_bottom()], stroke_br);
}

// ── Shared control renderer (moved here from the Form Designer) ────────────
// One renderer for designer, preview, run and compiled/web binaries (007 R5).

/// Draw a liquid-glass / glassmorphism rectangle that matches the reference aesthetic:
/// predominantly bright-white frosted glass with the background showing through clearly.
///
/// Key principles (from reference image):
///   • Base fill is near-white at very low opacity (~22 %) — not the control's dark colour.
///   • The `base` colour contributes only a faint tint so controls remain distinguishable.
///   • A strong top-to-transparent gradient (vertex mesh) simulates the specular reflection.
///   • A bright crisp inner rim at the very top edge reinforces the glass look.
///   • The bottom third darkens subtly (depth cue).
///   • A soft drop shadow underneath.
///   • A bright white/silver border.
///
/// All colour values are in egui **premultiplied** alpha space:
///   premult_rgb = straight_rgb × (alpha / 255).
/// Frosted-glass disc effect for **circular** controls (Circle shape).
///
/// Uses **radial-gradient polygon fans** (48-sided mesh, centre → edge colour)
/// for perfectly smooth gradients with zero banding.  `circle_filled` has a hard
/// perimeter edge that creates visible concentric rings when layered; fans avoid
/// that entirely because colour is interpolated per-vertex by the GPU.
///
/// Layer order (back → front):
///   1. Drop shadow
///   2. Nearly-transparent frosted body (cool blue-gray tint, ~20 % opacity)
///   3. Top-arc highlight fan   — gentle upper brightening
///   4. Bottom crescent fan     — characteristic glass-disc reflection at base
///   5. Rim stroke
pub fn draw_glass_circle(
    painter: &egui::Painter,
    center: Pos2,
    radius: f32,
    base: Color32,
    selected: bool,
    alpha_mul: f32,
) {
    if alpha_mul <= 0.0 {
        return;
    }
    let am = alpha_mul.clamp(0.0, 1.0);

    let white = |alpha: u8| -> Color32 {
        let a = (alpha as f32 * am) as u8;
        Color32::from_rgba_premultiplied(a, a, a, a)
    };
    let pm = |rv: u8, gv: u8, bv: u8, alpha: u8| -> Color32 {
        let a = (alpha as f32 * am) as u8;
        Color32::from_rgba_premultiplied(
            (rv as f32 * am * alpha as f32 / 255.0) as u8,
            (gv as f32 * am * alpha as f32 / 255.0) as u8,
            (bv as f32 * am * alpha as f32 / 255.0) as u8,
            a,
        )
    };

    // Radial-gradient fan: 48-gon, colour goes from `cc` at `origin`
    // to `ce` at the perimeter.  GPU interpolation = perfectly smooth gradient.
    let radial_fan = |origin: Pos2, rad: f32, cc: Color32, ce: Color32| -> egui::epaint::Mesh {
        let uv = egui::pos2(0.0, 0.0);
        let n = 48_u32;
        let mut m = egui::epaint::Mesh::default();
        m.vertices.push(egui::epaint::Vertex {
            pos: origin,
            uv,
            color: cc,
        });
        for i in 0..n {
            let a = i as f32 / n as f32 * std::f32::consts::TAU;
            m.vertices.push(egui::epaint::Vertex {
                pos: origin + Vec2::new(a.cos(), a.sin()) * rad,
                uv,
                color: ce,
            });
        }
        for i in 1..=n {
            let j = if i == n { 1 } else { i + 1 };
            m.indices.extend([0, i, j]);
        }
        m
    };

    // ── 1. Drop shadow ────────────────────────────────────────────────────────
    painter.circle_filled(
        center + Vec2::new(0.0, radius * 0.10),
        radius * 0.97,
        pm(0, 0, 0, 58),
    );

    // ── 2. Frosted body ───────────────────────────────────────────────────────
    // Barely-there tint so the canvas background shows through (real-glass feel).
    // 85 % cool-blue-white (200, 210, 220) + 15 % control base colour, at 20 % opacity.
    let t = 0.20_f32 * am;
    let fr = ((200.0 * 0.85 + base.r() as f32 * 0.15) * t) as u8;
    let fg = ((210.0 * 0.85 + base.g() as f32 * 0.15) * t) as u8;
    let fb = ((220.0 * 0.85 + base.b() as f32 * 0.15) * t) as u8;
    let fa = (255.0 * t) as u8;
    painter.circle_filled(
        center,
        radius,
        Color32::from_rgba_premultiplied(fr, fg, fb, fa),
    );

    // ── 3. Top-arc highlight ──────────────────────────────────────────────────
    // Subtle brightening in the upper third — centre at -30 % of radius.
    let top_c = center + Vec2::new(0.0, -radius * 0.30);
    painter.add(egui::Shape::mesh(radial_fan(
        top_c,
        radius * 0.65,
        white(52), // centre: soft white
        white(0),  // edge:   fully transparent
    )));

    // ── 4. Bottom crescent reflection ─────────────────────────────────────────
    // The defining glass-disc feature: a smooth bright oval near the bottom,
    // like light reflecting off the curved lower surface.
    let bot_c = center + Vec2::new(0.0, radius * 0.62);
    painter.add(egui::Shape::mesh(radial_fan(
        bot_c,
        radius * 0.50,
        white(100), // centre: bright reflection
        white(0),   // edge:   fades to transparent
    )));

    // ── 5. Rim ────────────────────────────────────────────────────────────────
    let (border_w, border_c) = if selected {
        (
            2.0,
            Color32::from_rgba_premultiplied(
                (140.0 * am) as u8,
                (190.0 * am) as u8,
                (255.0 * am) as u8,
                (255.0 * am) as u8,
            ),
        )
    } else {
        (1.5, white(150))
    };
    painter.circle_stroke(center, radius, Stroke::new(border_w, border_c));
}

fn glass_base_underlay(base: Color32, alpha_mul: f32) -> Option<Color32> {
    let base_alpha = ((base.a() as f32) * alpha_mul.clamp(0.0, 1.0)).clamp(0.0, 255.0) as u8;
    (base_alpha > 0)
        .then(|| Color32::from_rgba_unmultiplied(base.r(), base.g(), base.b(), base_alpha))
}

pub fn draw_glass(
    painter: &egui::Painter,
    rect: egui::Rect,
    base: Color32, // control's own colour — used only as a faint frost tint
    // Explicit user-chosen background painted as a solid, opacity-aware layer
    // *under* the frost. `None` for the default glass look (the `base` tint only
    // shifts the frost's hue by ~3.5%). Only set when the user actually picked a
    // BackgroundColor, so default controls stay fully translucent glass.
    bg_underlay: Option<Color32>,
    rounding: impl Into<egui::CornerRadius>, // uniform `f32` corner OR per-corner Rounding
    selected: bool,
    alpha_mul: f32,
) {
    if alpha_mul <= 0.0 {
        return;
    }
    let am = alpha_mul.clamp(0.0, 1.0);

    // Helper: premultiplied white at `alpha` (0-255), scaled by `am`.
    let white = |alpha: u8| -> Color32 {
        let a = (alpha as f32 * am) as u8;
        Color32::from_rgba_premultiplied(a, a, a, a)
    };

    // Helper: premultiplied arbitrary straight-alpha colour.
    let pm = |r: u8, g: u8, b: u8, alpha: u8| -> Color32 {
        let a = (alpha as f32 * am) as u8;
        Color32::from_rgba_premultiplied(
            (r as f32 * am * alpha as f32 / 255.0) as u8,
            (g as f32 * am * alpha as f32 / 255.0) as u8,
            (b as f32 * am * alpha as f32 / 255.0) as u8,
            a,
        )
    };

    let (x0, x1) = (rect.min.x, rect.max.x);
    let (y0, y1) = (rect.min.y, rect.max.y);
    let w = (x1 - x0).max(1.0);
    let h = (y1 - y0).max(1.0);
    // Per-corner radii (a uniform `f32` arrives as four equal corners), each
    // clamped to half the smaller side. `max_radius` drives the uniform-ish bits
    // (shadow spread, fill inset) so a control with one rounded corner still looks
    // right (spec 017 — clip children to a container's rounded border).
    let rnd0: egui::CornerRadius = rounding.into();
    let cap = (w * 0.5).min(h * 0.5);
    let rnd = round_map(rnd0, |c| c.max(0.0).min(cap));
    let max_radius = rnd.nw.max(rnd.ne).max(rnd.sw).max(rnd.se);

    // ── 1. Layered shadow ────────────────────────────────────────────────────
    painter.rect_filled(
        rect.translate(Vec2::new(0.0, 8.0)).expand(1.0),
        round_map(rnd, |c| c + 4.0),
        pm(0, 0, 0, 18),
    );
    painter.rect_filled(
        rect.translate(Vec2::new(0.0, 16.0)).expand(4.0),
        round_map(rnd, |c| c + 10.0),
        pm(0, 0, 0, 8),
    );

    if let Some(base_fill) = bg_underlay.and_then(|c| glass_base_underlay(c, am)) {
        painter.rect_filled(rect, rnd, base_fill);
    }

    // ── 2+3. Frosted field + depth tint via stacked rounded-rect bands ─────
    // Each 1px band is horizontally inset to follow the rounded corner arcs
    // (egui's rounding on a 1px-tall rect is capped to 0.5px, so we compute
    // the inset ourselves). This gives perfect rounded corners with no bleed.
    {
        let arc_inset = |y: f32, r: f32, edge: f32| -> f32 {
            let dy = (y - edge).abs();
            if dy >= r || r < 0.5 {
                return 0.0;
            }
            // Extra 0.5px so the fill sits under the border stroke's inner edge
            (r - (r * r - (r - dy) * (r - dy)).max(0.0).sqrt() + 0.5).max(0.0)
        };
        let band_count = h.ceil() as usize;
        for i in 0..band_count {
            let t = i as f32 / (band_count as f32 - 1.0).max(1.0);
            let y_top = y0 + i as f32;
            let y_bot = (y_top + 1.0).min(y1);
            if y_bot <= y_top {
                continue;
            }

            // Use the y closest to each corner edge for tightest inset
            let left_inset = arc_inset(y_top, f32::from(rnd.nw), y0).max(arc_inset(
                y_bot,
                f32::from(rnd.sw),
                y1,
            ));
            let right_inset = arc_inset(y_top, f32::from(rnd.ne), y0).max(arc_inset(
                y_bot,
                f32::from(rnd.se),
                y1,
            ));
            let bx0 = x0 + left_inset;
            let bx1 = x1 - right_inset;
            if bx1 <= bx0 {
                continue;
            }

            let band_rect = egui::Rect::from_min_max(Pos2::new(bx0, y_top), Pos2::new(bx1, y_bot));

            let u = t.clamp(0.0, 1.0);
            let smooth = u * u * (3.0 - 2.0 * u);
            let glass_alpha = 30.0 + 82.0 * (1.0 - smooth).powf(1.18);
            let lip = 10.0 * (1.0 - u).powf(5.2);
            let mix_base = 0.035;
            let gr = 255.0 * (1.0 - mix_base) + base.r() as f32 * mix_base;
            let gg = 255.0 * (1.0 - mix_base) + base.g() as f32 * mix_base;
            let gb = 255.0 * (1.0 - mix_base) + base.b() as f32 * mix_base;
            let ga = ((glass_alpha + lip) * am).clamp(0.0, 255.0);
            let da = (1.0 + 13.0 * smooth.powf(1.5)).clamp(0.0, 18.0);
            let dr = 28.0 * am * da / 255.0;
            let dg = 44.0 * am * da / 255.0;
            let db = 56.0 * am * da / 255.0;
            let d_alpha = da * am;
            let fr = (gr * ga / 255.0 + dr).clamp(0.0, 255.0) as u8;
            let fg = (gg * ga / 255.0 + dg).clamp(0.0, 255.0) as u8;
            let fb = (gb * ga / 255.0 + db).clamp(0.0, 255.0) as u8;
            let fa = (ga + d_alpha).clamp(0.0, 255.0) as u8;

            painter.rect_filled(
                band_rect,
                0.0,
                Color32::from_rgba_premultiplied(fr, fg, fb, fa),
            );
        }
    }

    // ── 4. Single rounded frame ───────────────────────────────────────────────
    let (border_w, border_c) = if selected {
        (
            2.0,
            Color32::from_rgba_premultiplied(
                (140.0 * am) as u8,
                (190.0 * am) as u8,
                (255.0 * am) as u8,
                (255.0 * am) as u8,
            ),
        )
    } else {
        (1.4, white(170))
    };
    // Inset the stroke by half its width so it is fully inside `rect` (egui
    // centres strokes on the path; a centred stroke spills half-a-pixel past
    // the rect, and that overhang is exactly the bright corner fringe).
    painter.rect_stroke(
        rect,
        rnd,
        Stroke::new(border_w, border_c),
        egui::StrokeKind::Inside,
    );
}

/// Liquid Glass Enhanced — a two-part stack per the Setproduct spec:
///
/// **Outer shell** (materiality cues):
///   1. Layered shadow (elevation)
///   2. Frosted fill (continuous gradient — same as Classic)
///   3. Depth tint
///   4. **Highlight band** — a bright top-edge strip implying a locked light direction
///   5. **Inner stroke** — a softer secondary border suggesting glass cross-section
///   6. Border edge (outer frame)
///
/// **Inner stabilized plate** (readability):
///   7. A denser scrim patch inset from the edges so content text never sits on
///      raw translucent glass. Subtle but always present.
///
/// The Classic version is untouched; this function adds layers 4, 5, 7 and
/// widens the highlight band to every control (Classic only has it on Buttons).
pub fn draw_glass_enhanced(
    painter: &egui::Painter,
    rect: egui::Rect,
    base: Color32,
    // See `draw_glass`: explicit user background painted solid under the frost;
    // `None` keeps the default translucent glass look.
    bg_underlay: Option<Color32>,
    rounding: impl Into<egui::CornerRadius>,
    selected: bool,
    alpha_mul: f32,
) {
    if alpha_mul <= 0.0 {
        return;
    }
    let am = alpha_mul.clamp(0.0, 1.0);

    let white = |alpha: u8| -> Color32 {
        let a = (alpha as f32 * am) as u8;
        Color32::from_rgba_premultiplied(a, a, a, a)
    };

    let pm = |r: u8, g: u8, b: u8, alpha: u8| -> Color32 {
        let a = (alpha as f32 * am) as u8;
        Color32::from_rgba_premultiplied(
            (r as f32 * am * alpha as f32 / 255.0) as u8,
            (g as f32 * am * alpha as f32 / 255.0) as u8,
            (b as f32 * am * alpha as f32 / 255.0) as u8,
            a,
        )
    };

    let (x0, x1) = (rect.min.x, rect.max.x);
    let (y0, y1) = (rect.min.y, rect.max.y);
    let w = (x1 - x0).max(1.0);
    let h = (y1 - y0).max(1.0);

    let rnd0: egui::CornerRadius = rounding.into();
    let cap = (w * 0.5).min(h * 0.5);
    let rnd = round_map(rnd0, |c| c.max(0.0).min(cap));
    let max_radius = rnd.nw.max(rnd.ne).max(rnd.sw).max(rnd.se);

    // ── 1. Layered shadow (same as Classic) ──────────────────────────────────
    painter.rect_filled(
        rect.translate(Vec2::new(0.0, 8.0)).expand(1.0),
        round_map(rnd, |c| c + 4.0),
        pm(0, 0, 0, 18),
    );
    painter.rect_filled(
        rect.translate(Vec2::new(0.0, 16.0)).expand(4.0),
        round_map(rnd, |c| c + 10.0),
        pm(0, 0, 0, 8),
    );

    if let Some(base_fill) = bg_underlay.and_then(|c| glass_base_underlay(c, am)) {
        painter.rect_filled(rect, rnd, base_fill);
    }

    // ── 2+3. Frosted field + depth tint via stacked rounded-rect bands ─────
    {
        let arc_inset = |y: f32, r: f32, edge: f32| -> f32 {
            let dy = (y - edge).abs();
            if dy >= r || r < 0.5 {
                return 0.0;
            }
            (r - (r * r - (r - dy) * (r - dy)).max(0.0).sqrt() + 0.5).max(0.0)
        };
        let band_count = h.ceil() as usize;
        for i in 0..band_count {
            let t = i as f32 / (band_count as f32 - 1.0).max(1.0);
            let y_top = y0 + i as f32;
            let y_bot = (y_top + 1.0).min(y1);
            if y_bot <= y_top {
                continue;
            }
            let left_inset = arc_inset(y_top, f32::from(rnd.nw), y0).max(arc_inset(
                y_bot,
                f32::from(rnd.sw),
                y1,
            ));
            let right_inset = arc_inset(y_top, f32::from(rnd.ne), y0).max(arc_inset(
                y_bot,
                f32::from(rnd.se),
                y1,
            ));
            let bx0 = x0 + left_inset;
            let bx1 = x1 - right_inset;
            if bx1 <= bx0 {
                continue;
            }
            let band_rect = egui::Rect::from_min_max(Pos2::new(bx0, y_top), Pos2::new(bx1, y_bot));
            let u = t.clamp(0.0, 1.0);
            let smooth = u * u * (3.0 - 2.0 * u);
            let glass_alpha = 30.0 + 82.0 * (1.0 - smooth).powf(1.18);
            let lip = 10.0 * (1.0 - u).powf(5.2);
            let mix_base = 0.035;
            let gr = 255.0 * (1.0 - mix_base) + base.r() as f32 * mix_base;
            let gg = 255.0 * (1.0 - mix_base) + base.g() as f32 * mix_base;
            let gb = 255.0 * (1.0 - mix_base) + base.b() as f32 * mix_base;
            let ga = ((glass_alpha + lip) * am).clamp(0.0, 255.0);
            let da = (1.0 + 13.0 * smooth.powf(1.5)).clamp(0.0, 18.0);
            let dr = 28.0 * am * da / 255.0;
            let dg = 44.0 * am * da / 255.0;
            let db = 56.0 * am * da / 255.0;
            let d_alpha = da * am;
            let fr = (gr * ga / 255.0 + dr).clamp(0.0, 255.0) as u8;
            let fg = (gg * ga / 255.0 + dg).clamp(0.0, 255.0) as u8;
            let fb = (gb * ga / 255.0 + db).clamp(0.0, 255.0) as u8;
            let fa = (ga + d_alpha).clamp(0.0, 255.0) as u8;
            painter.rect_filled(
                band_rect,
                0.0,
                Color32::from_rgba_premultiplied(fr, fg, fb, fa),
            );
        }
    }

    // ── 4. Highlight band (top edge, locked light direction) ─────────────────
    // A bright translucent strip along the top ~6-8 px, implying overhead light.
    // This is the key "thickness cue" that makes flat glass read as material.
    let band_h = (h * 0.08).clamp(3.0, 10.0);
    {
        let arc_hi = |y: f32, r: f32, edge: f32| -> f32 {
            let dy = (y - edge).abs();
            if dy >= r || r < 0.5 {
                return 0.0;
            }
            (r - (r * r - (r - dy) * (r - dy)).max(0.0).sqrt() + 0.5).max(0.0)
        };
        let band_rows = band_h.ceil() as usize;
        for i in 0..band_rows {
            let t = i as f32 / (band_rows as f32 - 1.0).max(1.0);
            let yt = y0 + i as f32;
            let yb = (yt + 1.0).min(y0 + band_h);
            if yb <= yt {
                continue;
            }
            let li = arc_hi(yt, f32::from(rnd.nw), y0);
            let ri = arc_hi(yt, f32::from(rnd.ne), y0);
            let bx0 = x0 + li;
            let bx1 = x1 - ri;
            if bx1 <= bx0 {
                continue;
            }
            let fade = (1.0 - t).powf(1.8);
            let c = white((38.0 * fade) as u8);
            painter.rect_filled(
                egui::Rect::from_min_max(Pos2::new(bx0, yt), Pos2::new(bx1, yb)),
                0.0,
                c,
            );
        }
    }

    // ── 5. Inner stroke (cross-section cue) ──────────────────────────────────
    // A softer, lighter border inset from the outer frame, suggesting the glass
    // has physical thickness. The inner rounding is reduced proportionally so
    // the inner stroke follows the same curvature as the outer border.
    let inner_inset = 2.4_f32.min(w * 0.1).min(h * 0.1);
    let inner_rect = rect.shrink(inner_inset);
    let inner_round = round_map(rnd, |c| {
        if c <= 0.0 {
            0.0
        } else {
            (c - inner_inset).max(1.0)
        }
    });
    painter.rect_stroke(
        inner_rect,
        inner_round,
        Stroke::new(0.6, white(55)),
        egui::StrokeKind::Middle,
    );

    // ── 6. Outer border edge ─────────────────────────────────────────────────
    let (border_w, border_c) = if selected {
        (
            2.0,
            Color32::from_rgba_premultiplied(
                (140.0 * am) as u8,
                (190.0 * am) as u8,
                (255.0 * am) as u8,
                (255.0 * am) as u8,
            ),
        )
    } else {
        (1.4, white(170))
    };
    painter.rect_stroke(
        rect,
        rnd,
        Stroke::new(border_w, border_c),
        egui::StrokeKind::Inside,
    );

    // ── 7. Stabilized plate (scrim under content) ────────────────────────────
    // A very subtle denser patch in the center area. This ensures text/icons
    // remain readable over hotspots and busy backgrounds. It is intentionally
    // barely visible on calm backgrounds — that subtlety is correct.
    let plate_inset = (4.0 + f32::from(max_radius) * 0.3)
        .min(w * 0.15)
        .min(h * 0.15);
    let plate_rect = rect.shrink(plate_inset);
    if plate_rect.width() > 4.0 && plate_rect.height() > 4.0 {
        let plate_round = round_map(rnd, |c| (c - plate_inset).max(2.0));
        painter.rect_filled(plate_rect, plate_round, pm(240, 244, 255, 18));
    }
}

/// Neumorphic ("soft UI") — depth through dual opposing soft shadows on a flat
/// matte surface.  No translucent frost, no gradient bands; the illusion of
/// physical form comes entirely from illumination.
///
/// Rendering stack:
///   1. **Light shadow** — white, offset up-left, blurred with Gaussian falloff
///   2. **Dark shadow**  — dark gray, offset down-right, blurred with Gaussian falloff
///   3. **Surface fill** — the control's BackgroundColor as a flat matte rect
///   4. **Asymmetric border** (optional) — light gray top/left, darker gray bottom/right
///
/// The blur uses a logarithmic scale: `spread = base × (1 + ln(1 + blur))`
/// so small blur values stay tight while high values bloom outward smoothly.
/// All shadow layers follow the control's `CornerRadius` curvature.
pub fn draw_glass_neumorphic(
    painter: &egui::Painter,
    rect: egui::Rect,
    base: Color32, // control's BackgroundColor — used as the literal surface fill
    bg_underlay: Option<Color32>, // explicit user background (overrides `base` when set)
    rounding: impl Into<egui::CornerRadius>,
    selected: bool,
    alpha_mul: f32,
) {
    if alpha_mul <= 0.0 {
        return;
    }
    let am = alpha_mul.clamp(0.0, 1.0);

    let (x0, x1) = (rect.min.x, rect.max.x);
    let (y0, y1) = (rect.min.y, rect.max.y);
    let w = (x1 - x0).max(1.0);
    let h = (y1 - y0).max(1.0);

    let rnd0: egui::CornerRadius = rounding.into();
    let cap = (w * 0.5).min(h * 0.5);
    let rnd = round_map(rnd0, |c| c.max(0.0).min(cap));

    // Retrieve shadow parameters from egui context (or fall back to defaults)
    let params = painter
        .ctx()
        .data(|d| d.get_temp::<NeumorphicShadowParams>(neumorphic_params_id()))
        .unwrap_or_default();

    // ── Shadow reach (blur spread) ────────────────────────────────────────────
    // Positive blur is the normal Neumorphic relief drawn behind the control.
    // Negative blur is handled after the surface fill as an inset/front-plane
    // relief, so it can project inward from the rounded border.
    let spread = (1.0_f32 + params.blur_strength.abs()).ln() * 8.0;
    let layers = 10_usize;

    if params.shadow_on && params.blur_strength >= 0.0 {
        let ux = params.shadow_dir[0];
        let uy = params.shadow_dir[1];
        let distance = params.distance;

        // Light shadow — opposite the user direction for raised relief.
        let light_offset = Vec2::new(-ux * distance, -uy * distance);
        let light_opac = (params.shadow_opac * 3.25).clamp(0.0, 1.0);
        for i in 0..=layers {
            let t = 1.0 - (i as f32 / layers as f32); // 1 (outer) → 0 (core)
            let expand = t * spread;
            let falloff = (-3.0 * t * t).exp();
            let a_val = (light_opac * am * falloff * 255.0) as u8;
            if a_val == 0 {
                continue;
            }
            let layer_rect = rect.translate(light_offset).expand(expand);
            let layer_round = round_map(rnd, |c| c + expand);
            let lc = params.light_color;
            painter.rect_filled(
                layer_rect,
                layer_round,
                Color32::from_rgba_premultiplied(
                    (lc.r() as f32 * (a_val as f32 / 255.0)) as u8,
                    (lc.g() as f32 * (a_val as f32 / 255.0)) as u8,
                    (lc.b() as f32 * (a_val as f32 / 255.0)) as u8,
                    a_val,
                ),
            );
        }

        // Dark shadow — user direction for raised relief.
        let dark_offset = Vec2::new(ux * distance, uy * distance);
        let sc = params.shadow_color;
        let dark_max_opac = params.shadow_opac;
        for i in 0..=layers {
            let t = 1.0 - (i as f32 / layers as f32);
            let expand = t * spread;
            let falloff = (-3.0 * t * t).exp();
            let a_val = (dark_max_opac * am * falloff * 255.0) as u8;
            if a_val == 0 {
                continue;
            }
            let layer_rect = rect.translate(dark_offset).expand(expand);
            let layer_round = round_map(rnd, |c| c + expand);
            painter.rect_filled(
                layer_rect,
                layer_round,
                Color32::from_rgba_premultiplied(
                    (sc.r() as f32 * (a_val as f32 / 255.0)) as u8,
                    (sc.g() as f32 * (a_val as f32 / 255.0)) as u8,
                    (sc.b() as f32 * (a_val as f32 / 255.0)) as u8,
                    a_val,
                ),
            );
        }
    }

    // ── 3. Surface fill (flat matte) ──────────────────────────────────────────
    // In Neumorphic the surface colour IS the control's BackgroundColor — not a
    // frost tint.  If a bg_underlay was explicitly set, prefer it; otherwise fall
    // back to `base`.  Default: warm neutral gray (#E0E0E0).
    let surface = bg_underlay.unwrap_or(base);
    painter.rect_filled(rect, rnd, surface_fill(surface, am));

    if neumorphic_shadow_overlays(&params) {
        draw_neumorphic_overlay_shadow(painter, rect, rnd, &params, alpha_mul);
    }

    // ── 4. Asymmetric border (only when selected) ─────────────────────────────
    // By default Neumorphic has NO border — relief comes from illumination.
    // Selection uses a soft blue outline so the user can see what's selected.
    if selected {
        let sel_color = Color32::from_rgba_premultiplied(
            (100.0 * am) as u8,
            (160.0 * am) as u8,
            (240.0 * am) as u8,
            (200.0 * am) as u8,
        );
        painter.rect_stroke(
            rect,
            rnd,
            Stroke::new(2.0, sel_color),
            egui::StrokeKind::Inside,
        );
    }
}

/// Draw **only** the Neumorphic dual-shadow halo for a control that has its own
/// custom paint path (Slider, ProgressBar, etc.) and therefore never reaches
/// `draw_glass_neumorphic`.  Shadow parameters are read from the egui temp
/// context exactly as `draw_glass_neumorphic` does, so the ShadowEnabled /
/// ShadowOpacity / … properties stored by `draw_control` are honoured.
///
/// Call this **before** the control draws its own content so the shadow sits
/// underneath the control's artwork.
pub fn draw_neumorphic_shadow_only(
    painter: &egui::Painter,
    rect: egui::Rect,
    rounding: impl Into<egui::CornerRadius>,
    alpha_mul: f32,
) {
    if alpha_mul <= 0.0 {
        return;
    }
    let am = alpha_mul.clamp(0.0, 1.0);
    let rnd0: egui::CornerRadius = rounding.into();
    let cap = (rect.width() * 0.5).min(rect.height() * 0.5);
    let rnd = round_map(rnd0, |c| c.max(0.0).min(cap));

    let params = painter
        .ctx()
        .data(|d| d.get_temp::<NeumorphicShadowParams>(neumorphic_params_id()))
        .unwrap_or_default();
    if !params.shadow_on {
        return;
    }
    if neumorphic_shadow_overlays(&params) {
        return;
    }

    let spread = (1.0_f32 + params.blur_strength.abs()).ln() * 8.0;
    let layers = 10_usize;
    let ux = params.shadow_dir[0];
    let uy = params.shadow_dir[1];
    let distance = params.distance;
    let sunken = params.blur_strength < 0.0;

    // Clip inside rect for sunken; let halo bleed outside for raised.
    let clipped_inset;
    let sp: &egui::Painter = if sunken {
        clipped_inset = painter.with_clip_rect(painter.clip_rect().intersect(rect));
        &clipped_inset
    } else {
        painter
    };

    // Light side: NW-outside when raised; SE-inside when sunken.
    let light_sign = if sunken { 1.0_f32 } else { -1.0_f32 };
    let light_offset = Vec2::new(light_sign * ux * distance, light_sign * uy * distance);
    let light_opac = (params.shadow_opac * 3.25).clamp(0.0, 1.0);
    for i in 0..=layers {
        let t = 1.0 - (i as f32 / layers as f32);
        let expand = t * spread;
        let falloff = (-3.0 * t * t).exp();
        let a_val = (light_opac * am * falloff * 255.0) as u8;
        if a_val == 0 {
            continue;
        }
        let layer_rect = rect.translate(light_offset).expand(expand);
        let layer_round = round_map(rnd, |c| c + expand);
        let lc = params.light_color;
        sp.rect_filled(
            layer_rect,
            layer_round,
            Color32::from_rgba_premultiplied(
                (lc.r() as f32 * (a_val as f32 / 255.0)) as u8,
                (lc.g() as f32 * (a_val as f32 / 255.0)) as u8,
                (lc.b() as f32 * (a_val as f32 / 255.0)) as u8,
                a_val,
            ),
        );
    }

    // Dark (user colour): SE-outside when raised; NW-inside when sunken.
    let dark_sign = if sunken { -1.0_f32 } else { 1.0_f32 };
    let dark_offset = Vec2::new(dark_sign * ux * distance, dark_sign * uy * distance);
    let sc = params.shadow_color;
    let dark_max_opac = params.shadow_opac;
    for i in 0..=layers {
        let t = 1.0 - (i as f32 / layers as f32);
        let expand = t * spread;
        let falloff = (-3.0 * t * t).exp();
        let a_val = (dark_max_opac * am * falloff * 255.0) as u8;
        if a_val == 0 {
            continue;
        }
        let layer_rect = rect.translate(dark_offset).expand(expand);
        let layer_round = round_map(rnd, |c| c + expand);
        sp.rect_filled(
            layer_rect,
            layer_round,
            Color32::from_rgba_premultiplied(
                (sc.r() as f32 * (a_val as f32 / 255.0)) as u8,
                (sc.g() as f32 * (a_val as f32 / 255.0)) as u8,
                (sc.b() as f32 * (a_val as f32 / 255.0)) as u8,
                a_val,
            ),
        );
    }
}

fn neumorphic_shadow_overlays(params: &NeumorphicShadowParams) -> bool {
    params.shadow_on && params.blur_strength < 0.0
}

fn draw_neumorphic_overlay_shadow_only(
    painter: &egui::Painter,
    rect: egui::Rect,
    rounding: impl Into<egui::CornerRadius>,
    alpha_mul: f32,
) {
    let params = painter
        .ctx()
        .data(|d| d.get_temp::<NeumorphicShadowParams>(neumorphic_params_id()))
        .unwrap_or_default();
    draw_neumorphic_overlay_shadow(painter, rect, rounding, &params, alpha_mul);
}

fn neumorphic_inset_shadow_metrics(params: &NeumorphicShadowParams) -> (f32, usize, f32, f32) {
    let strength = (params.blur_strength.abs() / 20.0).clamp(0.0, 1.0);
    let spread = 1.5 + 53.0 * strength.powf(0.85);
    let layers = (4.0 + 32.0 * strength).round() as usize;
    let opacity_scale = (0.36 + 1.64 * strength.powf(0.75)).clamp(0.0, 2.0);
    let stroke_w = 0.9 + 4.7 * strength.powf(0.8);
    (spread, layers.max(1), opacity_scale, stroke_w)
}

fn draw_neumorphic_overlay_shadow(
    painter: &egui::Painter,
    rect: egui::Rect,
    rounding: impl Into<egui::CornerRadius>,
    params: &NeumorphicShadowParams,
    alpha_mul: f32,
) {
    if alpha_mul <= 0.0 || !neumorphic_shadow_overlays(params) {
        return;
    }

    let am = alpha_mul.clamp(0.0, 1.0);
    let rnd0: egui::CornerRadius = rounding.into();
    let cap = (rect.width() * 0.5).min(rect.height() * 0.5);
    let rnd = round_map(rnd0, |c| c.max(0.0).min(cap));
    let (spread, layers, opacity_scale, _stroke_w) = neumorphic_inset_shadow_metrics(params);
    let steps = spread.ceil().max(layers as f32).max(1.0) as usize;

    let edge_inset = |distance: f32, r: f32| -> f32 {
        if r < 0.5 || distance >= r {
            0.0
        } else {
            (r - (r * r - (r - distance) * (r - distance)).max(0.0).sqrt()).max(0.0)
        }
    };
    let shade = |color: Color32, opacity: f32, t: f32| -> Color32 {
        let falloff = (-4.5 * t * t).exp();
        let a_val = (opacity * opacity_scale * am * falloff * 255.0).clamp(0.0, 255.0) as u8;
        Color32::from_rgba_premultiplied(
            (color.r() as f32 * (a_val as f32 / 255.0)) as u8,
            (color.g() as f32 * (a_val as f32 / 255.0)) as u8,
            (color.b() as f32 * (a_val as f32 / 255.0)) as u8,
            a_val,
        )
    };

    // Negative blur is an inset/front-plane Neumorphic relief. It is drawn as
    // narrow bands that start at the rounded inner border and fade inward. Each
    // band computes its own corner inset, so no square/rectangular overlay can
    // spill across the control's curved corners or into the center.
    let draw_horizontal = |top: bool, color: Color32, opacity: f32| {
        for i in 0..steps {
            let distance = i as f32;
            if distance > rect.height() * 0.5 {
                break;
            }
            let t = (distance / spread.max(1.0)).clamp(0.0, 1.0);
            let fill = shade(color, opacity, t);
            if fill.a() == 0 {
                continue;
            }
            let y0 = if top {
                rect.top() + distance
            } else {
                rect.bottom() - distance - 1.0
            };
            let y1 = (y0 + 1.0).min(rect.bottom());
            if y1 <= rect.top() || y1 <= y0 {
                continue;
            }
            let left_r = if top { rnd.nw } else { rnd.sw };
            let right_r = if top { rnd.ne } else { rnd.se };
            let left_inset = edge_inset(distance, f32::from(left_r));
            let right_inset = edge_inset(distance, f32::from(right_r));
            let band = egui::Rect::from_min_max(
                Pos2::new(rect.left() + left_inset, y0.max(rect.top())),
                Pos2::new(rect.right() - right_inset, y1),
            );
            if band.width() > 0.0 {
                painter.rect_filled(band, 0.0, fill);
            }
        }
    };

    let draw_vertical = |left: bool, color: Color32, opacity: f32| {
        for i in 0..steps {
            let distance = i as f32;
            if distance > rect.width() * 0.5 {
                break;
            }
            let t = (distance / spread.max(1.0)).clamp(0.0, 1.0);
            let fill = shade(color, opacity, t);
            if fill.a() == 0 {
                continue;
            }
            let x0 = if left {
                rect.left() + distance
            } else {
                rect.right() - distance - 1.0
            };
            let x1 = (x0 + 1.0).min(rect.right());
            if x1 <= rect.left() || x1 <= x0 {
                continue;
            }
            let top_r = if left { rnd.nw } else { rnd.ne };
            let bottom_r = if left { rnd.sw } else { rnd.se };
            let top_inset = edge_inset(distance, f32::from(top_r));
            let bottom_inset = edge_inset(distance, f32::from(bottom_r));
            let band = egui::Rect::from_min_max(
                Pos2::new(x0.max(rect.left()), rect.top() + top_inset),
                Pos2::new(x1, rect.bottom() - bottom_inset),
            );
            if band.height() > 0.0 {
                painter.rect_filled(band, 0.0, fill);
            }
        }
    };

    let user_opacity = params.shadow_opac;
    let white_opacity = (params.shadow_opac * 3.25).clamp(0.0, 1.0);
    draw_vertical(
        params.shadow_dir[0] >= 0.0,
        params.shadow_color,
        user_opacity,
    );
    draw_horizontal(
        params.shadow_dir[1] >= 0.0,
        params.shadow_color,
        user_opacity,
    );
    draw_vertical(
        params.shadow_dir[0] < 0.0,
        params.light_color,
        white_opacity,
    );
    draw_horizontal(
        params.shadow_dir[1] < 0.0,
        params.light_color,
        white_opacity,
    );
}

/// Draw the optional asymmetric neumorphic border when the user explicitly sets
/// `BorderStyle != None`.  Light gray on top/left edges, darker gray on
/// bottom/right edges — reinforces the top-left light source.
pub fn draw_neumorphic_user_border(
    painter: &egui::Painter,
    rect: egui::Rect,
    rounding: egui::CornerRadius,
    border_width: f32,
    alpha_mul: f32,
) {
    if border_width < 0.5 || alpha_mul <= 0.0 {
        return;
    }
    let am = alpha_mul.clamp(0.0, 1.0);
    let bw = border_width;
    // Inside strokes at the exact integer face radius (see draw_control_border).
    let inner = rect;
    let rnd = rounding;

    // Light edges (top + left) — clip to top-left half
    {
        let clip_tl = egui::Rect::from_min_max(
            Pos2::new(rect.min.x - bw, rect.min.y - bw),
            Pos2::new(rect.max.x + bw, rect.max.y + bw),
        );
        // We draw the full stroke but use light gray for top/left:
        // Top edge
        let light = Color32::from_rgba_premultiplied(
            (200.0 * am) as u8,
            (200.0 * am) as u8,
            (200.0 * am) as u8,
            (200.0 * am) as u8,
        );
        // Clip to upper-left triangle by drawing two separate half-strokes
        let top_clip = egui::Rect::from_min_max(
            Pos2::new(rect.min.x - bw, rect.min.y - bw),
            Pos2::new(rect.max.x + bw, rect.center().y),
        );
        let left_clip = egui::Rect::from_min_max(
            Pos2::new(rect.min.x - bw, rect.min.y - bw),
            Pos2::new(rect.center().x, rect.max.y + bw),
        );
        let p_top = painter.with_clip_rect(painter.clip_rect().intersect(top_clip));
        p_top.rect_stroke(inner, rnd, Stroke::new(bw, light), egui::StrokeKind::Inside);
        let p_left = painter.with_clip_rect(painter.clip_rect().intersect(left_clip));
        p_left.rect_stroke(inner, rnd, Stroke::new(bw, light), egui::StrokeKind::Inside);
    }
    // Dark edges (bottom + right)
    {
        let dark = Color32::from_rgba_premultiplied(
            (144.0 * am) as u8,
            (144.0 * am) as u8,
            (144.0 * am) as u8,
            (144.0 * am) as u8,
        );
        let bottom_clip = egui::Rect::from_min_max(
            Pos2::new(rect.min.x - bw, rect.center().y),
            Pos2::new(rect.max.x + bw, rect.max.y + bw),
        );
        let right_clip = egui::Rect::from_min_max(
            Pos2::new(rect.center().x, rect.min.y - bw),
            Pos2::new(rect.max.x + bw, rect.max.y + bw),
        );
        let p_bottom = painter.with_clip_rect(painter.clip_rect().intersect(bottom_clip));
        p_bottom.rect_stroke(inner, rnd, Stroke::new(bw, dark), egui::StrokeKind::Inside);
        let p_right = painter.with_clip_rect(painter.clip_rect().intersect(right_clip));
        p_right.rect_stroke(inner, rnd, Stroke::new(bw, dark), egui::StrokeKind::Inside);
    }
}

// ── Non-visual control rendering (standardised "liquid glass" icons) ─────────────
//
// All non-visual controls (Timer / AgentObject / RestClient / SqlDatabase /
// IndexedFile) share
// one dark glass card + a consistent light, stroke-drawn ("hand-drawn") icon and
// a larger label, so they look uniform on the canvas.

/// Shared glass-card colour for every non-visual control.
const NV_CARD: Color32 = Color32::from_rgb(40, 54, 84);

/// Light "glass" colour for the stroke icons + labels.
pub fn nv_icon_color(a: u8) -> Color32 {
    Color32::from_rgba_premultiplied(212, 226, 255, a)
}

/// Draw the shared non-visual card background.
pub fn nv_card(
    painter: &egui::Painter,
    rect: egui::Rect,
    selected: bool,
    glass: bool,
    alpha_mul: f32,
    a: u8,
) {
    if glass {
        draw_glass_auto(painter, rect, NV_CARD, 12.0, selected, alpha_mul);
    } else {
        let fill = Color32::from_rgba_premultiplied(NV_CARD.r(), NV_CARD.g(), NV_CARD.b(), a);
        let border = if selected {
            Color32::from_rgba_premultiplied(90, 160, 255, a)
        } else {
            Color32::from_rgba_premultiplied(110, 130, 180, a)
        };
        painter.rect_filled(rect, 12.0, fill);
        painter.rect_stroke(
            rect,
            12.0,
            Stroke::new(if selected { 2.0 } else { 1.0 }, border),
            egui::StrokeKind::Middle,
        );
    }
}

/// Centre / size / stroke for a non-visual icon within `rect`.
pub fn nv_icon_geom(rect: egui::Rect, a: u8) -> (Pos2, f32, Stroke) {
    let cen = Pos2::new(rect.center().x, rect.min.y + rect.height() * 0.40);
    let s = rect.height().min(rect.width()) * 0.22;
    let sw = (s * 0.18).clamp(1.6, 3.0);
    (cen, s, Stroke::new(sw, nv_icon_color(a)))
}

/// A larger label centred at the bottom of the card (≈2× the previous size).
pub fn nv_label(painter: &egui::Painter, rect: egui::Rect, text: &str, a: u8) {
    let t: String = text.chars().take(14).collect();
    painter.text(
        rect.center_bottom() - Vec2::new(0.0, 7.0),
        egui::Align2::CENTER_BOTTOM,
        t,
        // 20% smaller than 16px, and 25% darker label colour.
        egui::FontId::proportional(12.8),
        Color32::from_rgba_premultiplied(154, 165, 186, a),
    );
}

pub fn nv_ellipse(painter: &egui::Painter, cx: f32, cy: f32, rw: f32, rh: f32, st: Stroke) {
    let steps = 28u32;
    let pts: Vec<Pos2> = (0..=steps)
        .map(|i| {
            let t = i as f32 / steps as f32 * std::f32::consts::TAU;
            Pos2::new(cx + rw * t.cos(), cy + rh * t.sin())
        })
        .collect();
    painter.add(egui::Shape::closed_line(pts, st));
}

pub fn nv_icon_clock(painter: &egui::Painter, c: Pos2, s: f32, st: Stroke) {
    painter.circle_stroke(c, s, st);
    // top stem (stopwatch button)
    painter.line_segment(
        [c + Vec2::new(0.0, -s), c + Vec2::new(0.0, -s - s * 0.30)],
        st,
    );
    // hands
    painter.line_segment([c, c + Vec2::new(0.0, -s * 0.6)], st);
    painter.line_segment([c, c + Vec2::new(s * 0.45, s * 0.12)], st);
}

pub fn nv_icon_robot(painter: &egui::Painter, c: Pos2, s: f32, st: Stroke) {
    let head =
        egui::Rect::from_center_size(c + Vec2::new(0.0, s * 0.1), Vec2::new(s * 1.7, s * 1.5));
    painter.rect_stroke(head, s * 0.28, st, egui::StrokeKind::Middle);
    // antenna
    painter.line_segment(
        [
            Pos2::new(c.x, head.min.y),
            Pos2::new(c.x, head.min.y - s * 0.4),
        ],
        st,
    );
    painter.circle_filled(
        Pos2::new(c.x, head.min.y - s * 0.45),
        st.width * 1.1,
        st.color,
    );
    // eyes
    painter.circle_filled(c + Vec2::new(-s * 0.42, 0.0), st.width * 1.2, st.color);
    painter.circle_filled(c + Vec2::new(s * 0.42, 0.0), st.width * 1.2, st.color);
    // mouth
    painter.line_segment(
        [
            c + Vec2::new(-s * 0.4, s * 0.5),
            c + Vec2::new(s * 0.4, s * 0.5),
        ],
        st,
    );
}

pub fn nv_icon_globe(painter: &egui::Painter, c: Pos2, s: f32, st: Stroke) {
    painter.circle_stroke(c, s, st);
    // equator + two latitude lines
    painter.line_segment([c + Vec2::new(-s, 0.0), c + Vec2::new(s, 0.0)], st);
    painter.line_segment(
        [
            c + Vec2::new(-s * 0.86, -s * 0.5),
            c + Vec2::new(s * 0.86, -s * 0.5),
        ],
        st,
    );
    painter.line_segment(
        [
            c + Vec2::new(-s * 0.86, s * 0.5),
            c + Vec2::new(s * 0.86, s * 0.5),
        ],
        st,
    );
    // central meridian
    nv_ellipse(painter, c.x, c.y, s * 0.45, s, st);
}

pub fn nv_icon_database(painter: &egui::Painter, c: Pos2, s: f32, st: Stroke) {
    let rw = s;
    let rh = s * 0.40;
    let top = c.y - s * 0.72;
    let bot = c.y + s * 0.72;
    // top + middle rings
    nv_ellipse(painter, c.x, top, rw, rh, st);
    nv_ellipse(painter, c.x, c.y, rw, rh, st);
    // sides
    painter.line_segment([Pos2::new(c.x - rw, top), Pos2::new(c.x - rw, bot)], st);
    painter.line_segment([Pos2::new(c.x + rw, top), Pos2::new(c.x + rw, bot)], st);
    // front-bottom curve
    let steps = 18u32;
    let front: Vec<Pos2> = (0..=steps)
        .map(|i| {
            let t = i as f32 / steps as f32 * std::f32::consts::PI;
            Pos2::new(c.x + rw * t.cos(), bot + rh * t.sin())
        })
        .collect();
    painter.add(egui::Shape::line(front, st));
}

/// Document icon with a nested database glyph. IndexedFile is a keyed,
/// database-style file, so its deployed-on-canvas icon combines both: a
/// folded-corner document outline with the shared database cylinder glyph
/// (`nv_icon_database`) nested inside — matching the toolbox's IndexedFile
/// icon so the same control looks identical whether it's still in the
/// toolbox or already placed on the form.
pub fn nv_icon_indexed_file(painter: &egui::Painter, c: Pos2, s: f32, st: Stroke) {
    let pts = [
        Pos2::new(c.x - s * 0.75, c.y - s * 0.95),
        Pos2::new(c.x + s * 0.15, c.y - s * 0.95),
        Pos2::new(c.x + s * 0.75, c.y - s * 0.45),
        Pos2::new(c.x + s * 0.75, c.y + s * 0.95),
        Pos2::new(c.x - s * 0.75, c.y + s * 0.95),
    ];
    for i in 0..pts.len() {
        painter.line_segment([pts[i], pts[(i + 1) % pts.len()]], st);
    }
    // Folded-corner crease.
    painter.line_segment([pts[1], Pos2::new(c.x + s * 0.15, c.y - s * 0.45)], st);
    painter.line_segment([Pos2::new(c.x + s * 0.15, c.y - s * 0.45), pts[2]], st);

    // Nested database glyph in the lower half of the document.
    nv_icon_database(painter, Pos2::new(c.x, c.y + s * 0.30), s * 0.38, st);
}

/// A control's own `Opacity` (0–100) as a 0.0–1.0 multiplier (default 1.0). The
/// render walk multiplies a container's `opacity_of` into the `alpha_mul` it
/// passes to descendants, so a faded container dims its whole subtree (spec 012).
pub fn opacity_of(ctrl: &Control) -> f32 {
    ctrl.get_prop("Opacity")
        .map(|v| v.as_i64())
        .unwrap_or(100)
        .clamp(0, 100) as f32
        / 100.0
}

pub fn draw_control(
    painter: &egui::Painter,
    origin: Pos2,
    ctrl: &Control,
    selected: bool,
    glass: bool,
    alpha_mul: f32,
    scale: f32,                       // animation scale factor (1.0 = normal)
    pic_tex: Option<egui::TextureId>, // pre-loaded texture for PictureBox
) {
    use crate::ControlType as CT;

    let r = ctrl.rect;
    // Compute the base rect, then apply scale around the control center.
    let base_rect = egui::Rect::from_min_size(
        origin + Vec2::new(r.x as f32, r.y as f32),
        Vec2::new(r.w as f32, r.h as f32),
    );
    let rect = scale_rect_about_center(base_rect, scale);
    let frame_rect = if matches!(ctrl.control_type, CT::TabControl) {
        tabcontrol_page_rect(rect, ctrl)
    } else {
        rect
    };

    // Opacity (0–100) fades this control. Ancestor *container* opacities are
    // already folded into the incoming `alpha_mul` by the render walk, so a faded
    // container dims its whole subtree (spec 012). Default 100 ⇒ no change.
    let alpha_mul = alpha_mul * opacity_of(ctrl);

    let a = (alpha_mul.clamp(0.0, 1.0) * 255.0) as u8;
    let c_scale = |c: u8| -> u8 { ((c as f32) * alpha_mul) as u8 };
    let alpha_color =
        |c: Color32| Color32::from_rgba_premultiplied(c.r(), c.g(), c.b(), c_scale(c.a()));

    // Composite-frame diagnostics overlay (spec 017 corner-bleed hunt): traces every
    // frame a container draws — shadow, face, border, notch mask, restored outline —
    // in place with its real rounding. Toggled at runtime via COBOLT_FRAME_DIAGNOSTICS.
    let container_diag = frame_diagnostics_enabled();

    let is_neumorphic = active_glass_style(painter.ctx()).is_neumorphic();

    if is_neumorphic {
        let shadow_on = ctrl
            .get_prop("ShadowEnabled")
            .map(|v| v.as_bool())
            .unwrap_or(true); // Neumorphic default: ON
        let shadow_color = ctrl
            .get_prop("ShadowColor")
            .map(|v| parse_color(v.as_str()))
            .unwrap_or(Color32::BLACK);
        let light_color = ctrl
            .get_prop("ShadowLightColor")
            .map(|v| parse_color(v.as_str()))
            .unwrap_or(Color32::WHITE);
        let shadow_opac = ctrl
            .get_prop("ShadowOpacity")
            .map(|v| v.as_i64())
            .unwrap_or(6)
            .clamp(0, 100) as f32
            / 100.0;
        let shadow_dir = ctrl
            .get_prop("ShadowDirection")
            .map(|v| v.as_str().to_owned())
            .unwrap_or_else(|| "SouthEast".into()); // Neumorphic default: SE
        let distance = ctrl
            .get_prop("ShadowDistance")
            .map(|v| v.as_i64())
            .unwrap_or(7)
            .clamp(0, 60) as f32;
        let blur_enabled = ctrl
            .get_prop("ShadowBlur")
            .map(|v| v.as_bool())
            .unwrap_or(true);
        let blur_strength = if blur_enabled {
            ctrl.get_prop("ShadowBlurStrength")
                .map(|v| v.as_i64())
                .unwrap_or(8)
                .clamp(-20, 20) as f32 // negative → sunken / inset
        } else {
            0.0
        };

        // Direction → unit vector (ux, uy)
        let (ux, uy): (f32, f32) = match shadow_dir.as_str() {
            "North" => (0.0, -1.0),
            "NorthEast" => (0.707, -0.707),
            "East" => (1.0, 0.0),
            "SouthEast" => (0.707, 0.707),
            "South" => (0.0, 1.0),
            "SouthWest" => (-0.707, 0.707),
            "West" => (-1.0, 0.0),
            "NorthWest" => (-0.707, -0.707),
            _ => (0.0, 1.0),
        };

        let params = NeumorphicShadowParams {
            shadow_on,
            shadow_color,
            light_color,
            shadow_opac,
            shadow_dir: [ux, uy],
            distance,
            blur_strength,
        };
        painter
            .ctx()
            .data_mut(|d| d.insert_temp(neumorphic_params_id(), params));
    }

    // ── Drop shadow ───────────────────────────────────────────────────────────
    let regular_shadow = regular_drop_shadow(ctrl, frame_rect, is_neumorphic);
    if let Some(shadow) = regular_shadow.as_ref().filter(|shadow| !shadow.overlay) {
        if matches!(ctrl.control_type, CT::GroupBox | CT::Panel) {
            debug_frame(
                painter,
                frame_rect,
                egui::CornerRadius::same(crate::paint::cr8(corner_radius(ctrl))),
                0,
                "CONTAINER_SHADOW",
                container_diag,
            );
        }
        draw_regular_drop_shadow(painter, shadow, alpha_mul);
    }

    // ── Line control ──────────────────────────────────────────────────────────
    if matches!(ctrl.control_type, CT::Line) {
        let line_color = ctrl
            .get_prop("LineColor")
            .map(|v| parse_color(v.as_str()))
            .unwrap_or(Color32::BLACK);
        let thickness = ctrl
            .get_prop("LineThickness")
            .map(|v| v.as_i64() as f32)
            .unwrap_or(1.0);
        let dir = ctrl
            .get_prop("LineDirection")
            .map(|v| v.as_str().to_owned())
            .unwrap_or_else(|| "Horizontal".into());
        // Free rotation: `LineAngle` (degrees, 0 = horizontal) is the source of
        // truth when present; otherwise fall back to the legacy LineDirection
        // presets so existing forms are unchanged. The line is centred and as long
        // as the control's width, rotated about its centre.
        let (p1, p2) = if let Some(deg) = ctrl.get_prop("LineAngle").map(|v| v.as_i64() as f32) {
            let rad = deg.to_radians();
            let c = rect.center();
            let d = Vec2::new(rad.cos(), rad.sin()) * (rect.width().max(1.0) * 0.5);
            (c - d, c + d)
        } else {
            match dir.as_str() {
                "Vertical" => (rect.left_top(), rect.left_bottom()),
                "Diagonal" => (rect.left_top(), rect.right_bottom()),
                _ => (rect.left_center(), rect.right_center()),
            }
        };
        let col = alpha_color(line_color);
        let stroke = Stroke::new(thickness, col);
        let t = thickness.max(1.0);
        // DashStyle: Solid | Dash | Dot | DashDot (egui dashed-line shapes).
        match ctrl
            .get_prop("DashStyle")
            .map(|v| v.as_str().to_owned())
            .unwrap_or_else(|| "Solid".into())
            .as_str()
        {
            "Dash" => painter.extend(egui::Shape::dashed_line(
                &[p1, p2],
                stroke,
                t * 5.0,
                t * 4.0,
            )),
            "Dot" => painter.extend(egui::Shape::dashed_line(
                &[p1, p2],
                stroke,
                t * 1.2,
                t * 2.5,
            )),
            "DashDot" => painter.extend(egui::Shape::dashed_line_with_offset(
                &[p1, p2],
                stroke,
                &[t * 5.0, t * 1.2],
                &[t * 3.0, t * 3.0],
                0.0,
            )),
            _ => {
                painter.line_segment([p1, p2], stroke);
            }
        }
        // Rounded endings (round caps) — draw a disc at each end.
        if ctrl
            .get_prop("RoundedEnds")
            .map(|v| v.as_bool())
            .unwrap_or(false)
        {
            let r = (thickness * 0.5).max(0.5);
            painter.circle_filled(p1, r, col);
            painter.circle_filled(p2, r, col);
        }
        if selected {
            painter.circle_stroke(
                p1,
                4.0,
                Stroke::new(1.0, Color32::from_rgba_premultiplied(60, 120, 230, a)),
            );
            painter.circle_stroke(
                p2,
                4.0,
                Stroke::new(1.0, Color32::from_rgba_premultiplied(60, 120, 230, a)),
            );
        }
        return;
    }

    // ── Shape control ─────────────────────────────────────────────────────────
    if matches!(ctrl.control_type, CT::Shape) {
        // Face colour: the type-specific FillColor when the user set it;
        // otherwise the Appearance Back colour — each honoured only when it
        // differs from its default, the renderer-wide "still on the default
        // means the user has not chosen" convention.
        //
        // Both properties are *seeded* on every new Shape, so neither may be
        // read with a bare `get_prop`: taking FillColor whenever it is present
        // made the Appearance Back colour dead for Shapes (it is always
        // present). Setting a colour equal to its own default is therefore
        // indistinguishable from leaving it alone — the same trade the rest of
        // the renderer makes.
        let non_default = |prop: &str, default_hex: &str| -> Option<Color32> {
            let raw = ctrl.get_prop(prop).map(|v| v.as_str().to_owned())?;
            let t = raw.trim().trim_start_matches('#');
            if t.is_empty() || t.eq_ignore_ascii_case(default_hex.trim_start_matches('#')) {
                return None;
            }
            let c = parse_color(&raw);
            (c.a() > 0).then_some(c)
        };
        let appearance_back =
            non_default("BackgroundColor", crate::model::DEFAULT_BACKGROUND_COLOR);
        let fill_color = non_default("FillColor", crate::model::DEFAULT_SHAPE_FILL_COLOR)
            .or(appearance_back)
            .unwrap_or_else(|| parse_color(crate::model::DEFAULT_SHAPE_FILL_COLOR));
        let line_color = ctrl
            .get_prop("LineColor")
            .map(|v| parse_color(v.as_str()))
            .unwrap_or(Color32::BLACK);
        let thickness = ctrl
            .get_prop("LineThickness")
            .map(|v| v.as_i64() as f32)
            .unwrap_or(1.0);
        let fill_style = ctrl
            .get_prop("FillStyle")
            .map(|v| v.as_str().to_owned())
            .unwrap_or_else(|| "Solid".into());
        let shape_type = ctrl
            .get_prop("ShapeType")
            .map(|v| v.as_str().to_owned())
            .unwrap_or_else(|| "Rectangle".into());
        // LineStyle: None | Solid | Dash | Dot | DashDot — the shape's outline.
        let line_style = ctrl
            .get_prop("LineStyle")
            .map(|v| v.as_str().to_owned())
            .unwrap_or_else(|| "Solid".into());
        // FormStyle (default true): the shape follows the form's current style
        // (Classic/Enhanced glass, Neumorphic); off = flat classic fill.
        let glass = glass && ctrl.get_prop("FormStyle").map(|v| v.as_bool()).unwrap_or(true);

        let rr = match shape_type.as_str() {
            "Circle" => rect.width().min(rect.height()) / 2.0,
            "Ellipse" => rect.width().min(rect.height()) / 2.0, // backward compat
            "RoundRect" => 8.0, // legacy forms; the picker no longer offers it
            // Rectangle: the user's CornerRadius property controls the rounding.
            _ => corner_radius(ctrl).min(0.5 * rect.width().min(rect.height())),
        };
        let is_round = matches!(shape_type.as_str(), "Circle" | "Ellipse");
        let is_tri = shape_type == "Triangle";
        let circ_r = rect.width().min(rect.height()) / 2.0;
        let cc = rect.center();
        // Triangle — equilateral pointing up, filling the bounding rect.
        let tri_top = Pos2::new(rect.center().x, rect.min.y);
        let tri_bl = Pos2::new(rect.min.x, rect.max.y);
        let tri_br = Pos2::new(rect.max.x, rect.max.y);

        // Property-driven drop shadow. Rect silhouettes ride the shared regular
        // (pre-face) / Neumorphic (in-face) shadow paths; circle & triangle draw
        // their own silhouette-matching shadow here — the shared one is a
        // rectangle and would poke out around the shape.
        if is_round || is_tri {
            draw_shape_silhouette_shadow(painter, ctrl, rect, &shape_type, is_neumorphic, alpha_mul);
        }

        // ── Face ──────────────────────────────────────────────────────────────
        if fill_style != "None" {
            let flat_fill = alpha_color(fill_color);
            if glass && is_neumorphic && (is_round || is_tri) {
                // Neumorphic: flat matte surface — the relief comes from the
                // dual silhouette shadows drawn above.
                if is_round {
                    painter.circle_filled(cc, circ_r, flat_fill);
                } else {
                    painter.add(egui::Shape::convex_polygon(
                        vec![tri_top, tri_br, tri_bl],
                        flat_fill,
                        Stroke::NONE,
                    ));
                }
            } else if glass && is_round {
                draw_glass_circle(painter, cc, circ_r, fill_color, selected, alpha_mul);
            } else if glass && is_tri {
                // Frosted body — same tint math as draw_glass_circle: 85 % cool
                // blue-white + 15 % of the user's FillColor at 20 % opacity, so
                // the canvas shows through and the triangle matches its glass
                // siblings.
                let t = 0.20_f32 * alpha_mul.clamp(0.0, 1.0);
                let fr = ((200.0 * 0.85 + fill_color.r() as f32 * 0.15) * t) as u8;
                let fg = ((210.0 * 0.85 + fill_color.g() as f32 * 0.15) * t) as u8;
                let fb = ((220.0 * 0.85 + fill_color.b() as f32 * 0.15) * t) as u8;
                let fa = (255.0 * t) as u8;
                let frost = Color32::from_rgba_premultiplied(fr, fg, fb, fa);
                painter.add(egui::Shape::convex_polygon(
                    vec![tri_top, tri_br, tri_bl],
                    frost,
                    Stroke::NONE,
                ));
                // Soft highlight in the upper half: a smaller inner triangle
                // fading the frost brighter toward the apex.
                let c = Pos2::new(
                    (tri_top.x + tri_bl.x + tri_br.x) / 3.0,
                    (tri_top.y + tri_bl.y + tri_br.y) / 3.0,
                );
                let hi = |p: Pos2| c + (p - c) * 0.55;
                let ha = (52.0 * alpha_mul.clamp(0.0, 1.0)) as u8;
                painter.add(egui::Shape::convex_polygon(
                    vec![hi(tri_top), hi(tri_br), hi(tri_bl)],
                    Color32::from_rgba_premultiplied(ha, ha, ha, ha),
                    Stroke::NONE,
                ));
            } else if glass {
                // Rectangle / RoundRect — style-aware surface (Classic/Enhanced
                // frost or Neumorphic matte + relief) tinted by FillColor.
                draw_glass_auto(painter, rect, fill_color, rr, selected, alpha_mul);
            } else if is_round {
                painter.circle_filled(cc, circ_r, flat_fill);
            } else if is_tri {
                painter.add(egui::Shape::convex_polygon(
                    vec![tri_top, tri_br, tri_bl],
                    flat_fill,
                    Stroke::NONE,
                ));
            } else {
                painter.rect_filled(rect, rr, flat_fill);
            }
        }

        // ── Outline (LineStyle) ───────────────────────────────────────────────
        // "None" removes the outline; a selected shape still shows the thin blue
        // designer outline so selection stays visible.
        let user_stroke = line_style != "None" && thickness > 0.0;
        if user_stroke || selected {
            let border_c = if selected {
                Color32::from_rgba_premultiplied(60, 120, 230, a)
            } else {
                alpha_color(line_color)
            };
            let sw = if user_stroke { thickness.max(1.0) } else { 1.0 };
            let stroke = Stroke::new(sw, border_c);
            let style = if user_stroke { line_style.as_str() } else { "Solid" };
            match style {
                "Dash" | "Dot" | "DashDot" => {
                    // Closed silhouette path; the dash pattern follows the
                    // perimeter (same dash metrics as the Line control).
                    let pts: Vec<Pos2> = if is_round {
                        let n = 72;
                        (0..=n)
                            .map(|i| {
                                let ang = i as f32 / n as f32 * std::f32::consts::TAU;
                                cc + Vec2::new(ang.cos(), ang.sin()) * circ_r
                            })
                            .collect()
                    } else if is_tri {
                        vec![tri_top, tri_br, tri_bl, tri_top]
                    } else if rr > 0.0 {
                        rounded_rect_outline_points(rect, rr)
                    } else {
                        vec![
                            rect.left_top(),
                            rect.right_top(),
                            rect.right_bottom(),
                            rect.left_bottom(),
                            rect.left_top(),
                        ]
                    };
                    let t = sw;
                    match style {
                        "Dash" => painter.extend(egui::Shape::dashed_line(
                            &pts,
                            stroke,
                            t * 5.0,
                            t * 4.0,
                        )),
                        "Dot" => painter.extend(egui::Shape::dashed_line(
                            &pts,
                            stroke,
                            t * 1.2,
                            t * 2.5,
                        )),
                        _ => painter.extend(egui::Shape::dashed_line_with_offset(
                            &pts,
                            stroke,
                            &[t * 5.0, t * 1.2],
                            &[t * 3.0, t * 3.0],
                            0.0,
                        )),
                    }
                }
                _ => {
                    if is_round {
                        painter.circle_stroke(cc, circ_r, stroke);
                    } else if is_tri {
                        painter.add(egui::Shape::closed_line(
                            vec![tri_top, tri_br, tri_bl],
                            stroke,
                        ));
                    } else {
                        painter.rect_stroke(rect, rr, stroke, egui::StrokeKind::Middle);
                    }
                }
            }
        }

        if let Some(shadow) = regular_shadow.as_ref().filter(|shadow| shadow.overlay) {
            draw_regular_drop_shadow(painter, shadow, alpha_mul);
        }
        return;
    }

    // ── Non-visual controls — standardised glass card + stroke icon + label ─────
    if matches!(
        ctrl.control_type,
        CT::Timer | CT::AgentObject | CT::RestClient | CT::SqlDatabase | CT::IndexedFile
    ) {
        nv_card(painter, rect, selected, glass, alpha_mul, a);
        let (cen, s, st) = nv_icon_geom(rect, a);
        let label: String = match ctrl.control_type {
            CT::Timer => {
                nv_icon_clock(painter, cen, s, st);
                let iv = ctrl.get_prop("Interval").map(|v| v.as_i64()).unwrap_or(1000);
                format!("{iv}ms")
            }
            CT::AgentObject => {
                nv_icon_robot(painter, cen, s, st);
                ctrl.get_prop("AgentModel").map(|v| v.as_str().to_owned()).unwrap_or_else(|| "LLM".into())
            }
            CT::RestClient => {
                nv_icon_globe(painter, cen, s, st);
                ctrl.get_prop("DefaultMethod").map(|v| v.as_str().to_owned()).unwrap_or_else(|| "GET".into())
            }
            CT::SqlDatabase => {
                nv_icon_database(painter, cen, s, st);
                ctrl.get_prop("Driver").map(|v| v.as_str().to_owned()).unwrap_or_else(|| "sqlite".into())
            }
            _ /* IndexedFile */ => {
                nv_icon_indexed_file(painter, cen, s, st);
                ctrl.get_prop("OpenMode").map(|v| v.as_str().to_owned()).unwrap_or_else(|| "INPUT".into())
            }
        };
        nv_label(painter, rect, &label, a);
        return;
    }

    // ── Slider ────────────────────────────────────────────────────────────────
    if matches!(ctrl.control_type, CT::Slider) {
        let min_v = ctrl.get_prop("Minimum").map(|v| v.as_i64()).unwrap_or(0) as f32;
        let max_v = ctrl
            .get_prop("Maximum")
            .map(|v| v.as_i64())
            .unwrap_or(100)
            .max(1) as f32;
        let val = ctrl.get_prop("Value").map(|v| v.as_i64()).unwrap_or(0) as f32;
        let _step_v = ctrl
            .get_prop("Step")
            .map(|v| v.as_i64())
            .unwrap_or(10)
            .max(1) as f32;
        let tick_fr = ctrl
            .get_prop("TickFrequency")
            .map(|v| v.as_i64())
            .unwrap_or(10)
            .max(1) as f32;
        let tick_st = ctrl
            .get_prop("TickStyle")
            .map(|v| v.as_str().to_owned())
            .unwrap_or_else(|| "Bottom".into());
        let orient = ctrl
            .get_prop("Orientation")
            .map(|v| v.as_str().to_owned())
            .unwrap_or_else(|| "Horizontal".into());
        let vertical = orient.starts_with('V');

        let track_c = alpha_color(
            ctrl.get_prop("TrackColor")
                .map(|v| parse_color(v.as_str()))
                .unwrap_or(Color32::from_rgb(170, 170, 170)),
        );
        let thumb_c = alpha_color(
            ctrl.get_prop("ThumbColor")
                .map(|v| parse_color(v.as_str()))
                .unwrap_or(Color32::from_rgb(0, 120, 215)),
        );
        let fill_c = alpha_color(
            ctrl.get_prop("FillColor")
                .map(|v| parse_color(v.as_str()))
                .unwrap_or(Color32::from_rgb(0, 120, 215)),
        );
        let show_val = ctrl
            .get_prop("ShowValue")
            .map(|v| v.as_bool())
            .unwrap_or(false);

        let _ = (track_c, thumb_c, fill_c); // glass design uses its own colors

        let pct = ((val - min_v) / (max_v - min_v)).clamp(0.0, 1.0);
        let range_units = max_v - min_v;

        // ── Helper: draw a frosted-glass pill ─────────────────────────────────
        // pill_rect: the full bounding rect of the pill
        // body_rgba: base frosted color (r,g,b,a) – already alpha-premultiplied
        // sheen: if true, add a top-half white gradient sheen
        let draw_glass_pill = |painter: &egui::Painter,
                               pill: egui::Rect,
                               body: Color32,
                               sheen: bool,
                               rim: Color32| {
            // CornerRadius follows the SHORT axis so a tall/narrow pill (a vertical
            // slider track) stays a capsule instead of over-rounding on height.
            let r = pill.width().min(pill.height()) * 0.5;
            painter.rect_filled(pill, r, body);
            // Top-half white sheen. The inset by `r` is only meaningful for a wide
            // pill; on a narrow vertical track it collapses (left >= right), so the
            // sheen is skipped rather than smeared into a giant gradient quad.
            let left = pill.min.x + r;
            let right = pill.max.x - r;
            if sheen && right > left {
                // Top-half gradient mesh: opaque white → transparent
                let mut mesh = egui::epaint::Mesh::default();
                let top = pill.min.y;
                let mid = pill.min.y + pill.height() * 0.5;
                let w_hi =
                    Color32::from_rgba_premultiplied(120, 130, 150, (80.0 * alpha_mul) as u8);
                let w_lo = Color32::from_rgba_premultiplied(0, 0, 0, 0);
                // quad: 4 vertices
                let i = mesh.vertices.len() as u32;
                mesh.vertices.push(egui::epaint::Vertex {
                    pos: Pos2::new(left, top),
                    uv: egui::epaint::WHITE_UV,
                    color: w_hi,
                });
                mesh.vertices.push(egui::epaint::Vertex {
                    pos: Pos2::new(right, top),
                    uv: egui::epaint::WHITE_UV,
                    color: w_hi,
                });
                mesh.vertices.push(egui::epaint::Vertex {
                    pos: Pos2::new(right, mid),
                    uv: egui::epaint::WHITE_UV,
                    color: w_lo,
                });
                mesh.vertices.push(egui::epaint::Vertex {
                    pos: Pos2::new(left, mid),
                    uv: egui::epaint::WHITE_UV,
                    color: w_lo,
                });
                mesh.indices
                    .extend_from_slice(&[i, i + 1, i + 2, i, i + 2, i + 3]);
                painter.add(egui::Shape::mesh(mesh));
            }
            painter.rect_stroke(pill, r, Stroke::new(1.0, rim), egui::StrokeKind::Middle);
        };

        // ── Helper: draw radial lens highlight at bottom-center of thumb ──────
        let draw_lens = |painter: &egui::Painter, center: Pos2, rx: f32, ry: f32| {
            let mut mesh = egui::epaint::Mesh::default();
            let center_c = Color32::from_rgba_premultiplied(
                (200.0 * alpha_mul) as u8,
                (215.0 * alpha_mul) as u8,
                (255.0 * alpha_mul) as u8,
                (160.0 * alpha_mul) as u8,
            );
            let edge_c = Color32::from_rgba_premultiplied(0, 0, 0, 0);
            let ci = mesh.vertices.len() as u32;
            mesh.vertices.push(egui::epaint::Vertex {
                pos: center,
                uv: egui::epaint::WHITE_UV,
                color: center_c,
            });
            let n = 32u32;
            for i in 0..n {
                let angle = (i as f32 / n as f32) * TAU;
                mesh.vertices.push(egui::epaint::Vertex {
                    pos: Pos2::new(center.x + rx * angle.cos(), center.y + ry * angle.sin()),
                    uv: egui::epaint::WHITE_UV,
                    color: edge_c,
                });
            }
            for i in 0..n {
                mesh.indices
                    .extend_from_slice(&[ci, ci + 1 + i, ci + 1 + (i + 1) % n]);
            }
            painter.add(egui::Shape::mesh(mesh));
        };

        // Fore color drives the KNOB (thumb); Back color drives the track BODY
        // (along the scale). Only override the Liquid Glass defaults when the user
        // set a *non-default* colour; otherwise keep the frosted-glass look.
        let non_default = |prop: &str, default_hex: &str| -> Option<Color32> {
            let raw = ctrl.get_prop(prop).map(|v| v.as_str().to_owned())?;
            let t = raw.trim().trim_start_matches('#');
            if t.is_empty() || t.eq_ignore_ascii_case(default_hex.trim_start_matches('#')) {
                return None;
            }
            let c = parse_color(&raw);
            (c.a() > 0).then_some(c)
        };
        let user_track = non_default("BackgroundColor", crate::model::DEFAULT_BACKGROUND_COLOR);
        let user_thumb = non_default("ForegroundColor", crate::model::DEFAULT_FOREGROUND_COLOR);
        let tint = |c: Color32, a: f32| {
            Color32::from_rgba_unmultiplied(
                c.r(),
                c.g(),
                c.b(),
                (a * alpha_mul).clamp(0.0, 255.0) as u8,
            )
        };

        // Glass track/knob colours (defaults), overridden by Back/Fore colour.
        let track_body = user_track.map(|c| tint(c, 210.0)).unwrap_or_else(|| {
            Color32::from_rgba_premultiplied(
                (100.0 * alpha_mul) as u8,
                (110.0 * alpha_mul) as u8,
                (135.0 * alpha_mul) as u8,
                (90.0 * alpha_mul) as u8,
            )
        });
        let track_rim = user_track
            .map(|c| tint(shade(c, 0.45), 190.0))
            .unwrap_or_else(|| {
                Color32::from_rgba_premultiplied(
                    (180.0 * alpha_mul) as u8,
                    (185.0 * alpha_mul) as u8,
                    (210.0 * alpha_mul) as u8,
                    (120.0 * alpha_mul) as u8,
                )
            });
        let thumb_body = user_thumb.map(|c| tint(c, 235.0)).unwrap_or_else(|| {
            Color32::from_rgba_premultiplied(
                (150.0 * alpha_mul) as u8,
                (160.0 * alpha_mul) as u8,
                (195.0 * alpha_mul) as u8,
                (140.0 * alpha_mul) as u8,
            )
        });
        let thumb_rim = user_thumb
            .map(|c| tint(shade(c, 0.5), 210.0))
            .unwrap_or_else(|| {
                Color32::from_rgba_premultiplied(
                    (220.0 * alpha_mul) as u8,
                    (225.0 * alpha_mul) as u8,
                    (245.0 * alpha_mul) as u8,
                    (180.0 * alpha_mul) as u8,
                )
            });

        if vertical {
            // ── Vertical glass slider ────────────────────────────────────────
            let track_half_w = (rect.width() * 0.18).clamp(4.0, 12.0);
            let cx = rect.center().x;
            let track_t = rect.min.y + 10.0;
            let track_b = rect.max.y - 10.0;
            let track_h = (track_b - track_t).max(1.0);
            let thumb_y = track_b - pct * track_h;
            let thumb_h = (track_half_w * 2.0 * 1.6).clamp(16.0, 32.0);
            let thumb_w = track_half_w * 2.0 + 6.0;

            // Track pill — shadow follows the pill shape in Neumorphic mode
            let track_rect = egui::Rect::from_min_max(
                Pos2::new(cx - track_half_w, track_t),
                Pos2::new(cx + track_half_w, track_b),
            );
            if is_neumorphic {
                // Pill rounding = half the short axis — exactly matches draw_glass_pill
                draw_neumorphic_shadow_only(painter, track_rect, track_half_w, alpha_mul);
            }
            draw_glass_pill(painter, track_rect, track_body, true, track_rim);
            if is_neumorphic {
                draw_neumorphic_overlay_shadow_only(painter, track_rect, track_half_w, alpha_mul);
            }

            // Tick marks
            if tick_st != "None" && range_units > 0.0 {
                let mut tick_v = min_v;
                while tick_v <= max_v + 0.001 {
                    let ty = track_b - ((tick_v - min_v) / range_units).clamp(0.0, 1.0) * track_h;
                    let tick_color =
                        Color32::from_rgba_premultiplied(140, 145, 165, (80.0 * alpha_mul) as u8);
                    let tick_len = 5.0;
                    if tick_st == "Left" || tick_st == "Both" {
                        painter.line_segment(
                            [
                                Pos2::new(cx - track_half_w - tick_len, ty),
                                Pos2::new(cx - track_half_w - 1.0, ty),
                            ],
                            Stroke::new(1.0, tick_color),
                        );
                    }
                    if tick_st != "Left" || tick_st == "Both" {
                        painter.line_segment(
                            [
                                Pos2::new(cx + track_half_w + 1.0, ty),
                                Pos2::new(cx + track_half_w + tick_len, ty),
                            ],
                            Stroke::new(1.0, tick_color),
                        );
                    }
                    tick_v += tick_fr;
                }
            }

            // Thumb pill
            let thumb_rect =
                egui::Rect::from_center_size(Pos2::new(cx, thumb_y), Vec2::new(thumb_w, thumb_h));
            draw_glass_pill(painter, thumb_rect, thumb_body, true, thumb_rim);
            // Lens at bottom-center of thumb
            draw_lens(
                painter,
                Pos2::new(cx, thumb_rect.max.y - thumb_h * 0.28),
                thumb_w * 0.32,
                thumb_h * 0.18,
            );
        } else {
            // ── Horizontal glass slider ──────────────────────────────────────
            let track_half_h = (rect.height() * 0.18).clamp(4.0, 12.0);
            let cy = rect.center().y;
            let track_l = rect.min.x + 10.0;
            let track_r = rect.max.x - 10.0;
            let track_w = (track_r - track_l).max(1.0);
            let thumb_x = track_l + pct * track_w;
            let thumb_w_half = (track_half_h * 1.6).clamp(8.0, 20.0);
            let thumb_h = track_half_h * 2.0 + 6.0;

            // Track pill — shadow follows the pill shape in Neumorphic mode
            let track_rect = egui::Rect::from_min_max(
                Pos2::new(track_l, cy - track_half_h),
                Pos2::new(track_r, cy + track_half_h),
            );
            if is_neumorphic {
                // Pill rounding = half the short axis — exactly matches draw_glass_pill
                draw_neumorphic_shadow_only(painter, track_rect, track_half_h, alpha_mul);
            }
            draw_glass_pill(painter, track_rect, track_body, true, track_rim);
            if is_neumorphic {
                draw_neumorphic_overlay_shadow_only(painter, track_rect, track_half_h, alpha_mul);
            }

            // Tick marks
            if tick_st != "None" && range_units > 0.0 {
                let mut tick_v = min_v;
                while tick_v <= max_v + 0.001 {
                    let tx = track_l + ((tick_v - min_v) / range_units).clamp(0.0, 1.0) * track_w;
                    let tick_color =
                        Color32::from_rgba_premultiplied(140, 145, 165, (80.0 * alpha_mul) as u8);
                    let tick_len = 5.0;
                    if tick_st == "Top" || tick_st == "Both" {
                        painter.line_segment(
                            [
                                Pos2::new(tx, cy - track_half_h - tick_len),
                                Pos2::new(tx, cy - track_half_h - 1.0),
                            ],
                            Stroke::new(1.0, tick_color),
                        );
                    }
                    if tick_st != "Top" || tick_st == "Both" {
                        painter.line_segment(
                            [
                                Pos2::new(tx, cy + track_half_h + 1.0),
                                Pos2::new(tx, cy + track_half_h + tick_len),
                            ],
                            Stroke::new(1.0, tick_color),
                        );
                    }
                    tick_v += tick_fr;
                }
            }

            // Thumb pill
            let thumb_rect = egui::Rect::from_center_size(
                Pos2::new(thumb_x, cy),
                Vec2::new(thumb_w_half * 2.0, thumb_h),
            );
            draw_glass_pill(painter, thumb_rect, thumb_body, true, thumb_rim);
            // Lens at bottom-center of thumb
            draw_lens(
                painter,
                Pos2::new(thumb_x, thumb_rect.max.y - thumb_h * 0.28),
                thumb_w_half * 0.6,
                thumb_h * 0.18,
            );
        }

        // Step label (min / max corners)
        let font_s = egui::FontId::proportional(9.0);
        let lbl_c = Color32::from_rgba_premultiplied(80, 80, 80, a);
        if vertical {
            painter.text(
                Pos2::new(rect.center().x, rect.max.y - 2.0),
                egui::Align2::CENTER_BOTTOM,
                format!("{}", min_v as i64),
                font_s.clone(),
                lbl_c,
            );
            painter.text(
                Pos2::new(rect.center().x, rect.min.y + 2.0),
                egui::Align2::CENTER_TOP,
                format!("{}", max_v as i64),
                font_s.clone(),
                lbl_c,
            );
        } else {
            painter.text(
                Pos2::new(rect.min.x + 2.0, rect.max.y - 1.0),
                egui::Align2::LEFT_BOTTOM,
                format!("{}", min_v as i64),
                font_s.clone(),
                lbl_c,
            );
            painter.text(
                Pos2::new(rect.max.x - 2.0, rect.max.y - 1.0),
                egui::Align2::RIGHT_BOTTOM,
                format!("{}", max_v as i64),
                font_s.clone(),
                lbl_c,
            );
        }

        // Optional current value label
        if show_val {
            painter.text(
                rect.center(),
                egui::Align2::CENTER_CENTER,
                format!("{}", val as i64),
                egui::FontId::proportional(ctrl_font_size(ctrl)),
                Color32::from_rgba_premultiplied(0, 0, 0, a),
            );
        }

        // Selection border
        if selected {
            painter.rect_stroke(
                rect,
                3.0,
                Stroke::new(2.0, Color32::from_rgba_premultiplied(60, 120, 230, a)),
                egui::StrokeKind::Middle,
            );
        }
        if let Some(shadow) = regular_shadow.as_ref().filter(|shadow| shadow.overlay) {
            draw_regular_drop_shadow(painter, shadow, alpha_mul);
        }
        return;
    }

    // ── Knob / Gauge / Switch / FileDropZone (spec 039) ────────────────────────
    //
    // These four render as REAL egui-elegance widgets on the interactive
    // surfaces (render_form → render_interactive, which has a live `Ui`).
    // `draw_control` only ever gets a bare `Painter` — no `Ui` — because it
    // is also the designer canvas's static-face renderer (`render_faces`),
    // which has no live widget tree at all. A crate `Widget` cannot run
    // without a `Ui`, so the designer canvas gets a simplified, hand-painted
    // proxy here instead of the real widget: recognisable and value-accurate,
    // not a pixel match for egui-elegance's own (considerably more elaborate)
    // paint job — that fidelity is what the real widget is for, and the
    // designer canvas only ever needs to communicate "this is a Knob, its
    // value is 42" at a glance, the same way every other custom-painted
    // control's static face here is a simplified stand-in for its live self.
    if matches!(
        ctrl.control_type,
        CT::Knob | CT::Gauge | CT::Switch | CT::FileDropZone
    ) {
        // Approximate egui-elegance's fixed Accent palette (theme.rs) — a
        // hand-picked visual match, not an import (that palette is private
        // to the crate's Theme type, which needs a live `Ui`/`Context` to
        // resolve dark/light variants; the designer canvas has neither).
        let accent_color = |name: &str| -> Color32 {
            match name {
                "Green" => Color32::from_rgb(46, 125, 50),
                "Red" => Color32::from_rgb(198, 40, 40),
                "Purple" => Color32::from_rgb(106, 27, 154),
                "Amber" => Color32::from_rgb(245, 124, 0),
                "Sky" => Color32::from_rgb(2, 136, 209),
                _ => Color32::from_rgb(0, 120, 215), // Blue (default)
            }
        };
        // A 270° arc (Knob's own sweep) or a full circle (Gauge Radial/Donut),
        // filled from the start up to `frac` (0..1) of `sweep_deg`, plus the
        // unfilled remainder as a dim track. `start_deg`/`sweep_deg` follow
        // egui's angle convention (0 = east, clockwise with +y down).
        let draw_ring = |painter: &egui::Painter,
                          center: Pos2,
                          radius: f32,
                          stroke_w: f32,
                          start_deg: f32,
                          sweep_deg: f32,
                          frac: f32,
                          fill: Color32,
                          track: Color32| {
            let segments = 48.max((sweep_deg.abs() / 4.0) as usize);
            let pt = |deg: f32| -> Pos2 {
                let a = deg.to_radians();
                Pos2::new(center.x + radius * a.cos(), center.y + radius * a.sin())
            };
            let track_pts: Vec<Pos2> = (0..=segments)
                .map(|i| pt(start_deg + sweep_deg * (i as f32 / segments as f32)))
                .collect();
            painter.add(egui::Shape::line(track_pts, Stroke::new(stroke_w, track)));
            if frac > 0.001 {
                let fill_sweep = sweep_deg * frac.clamp(0.0, 1.0);
                let fill_segments = 48.max((fill_sweep.abs() / 4.0) as usize).max(1);
                let fill_pts: Vec<Pos2> = (0..=fill_segments)
                    .map(|i| pt(start_deg + fill_sweep * (i as f32 / fill_segments as f32)))
                    .collect();
                painter.add(egui::Shape::line(fill_pts, Stroke::new(stroke_w, fill)));
            }
        };

        match ctrl.control_type {
            CT::Knob => {
                let min_v = ctrl.get_prop("Minimum").map(|v| v.as_i64()).unwrap_or(0) as f32;
                let max_v = ctrl
                    .get_prop("Maximum")
                    .map(|v| v.as_i64())
                    .unwrap_or(100)
                    .max(min_v as i64 + 1) as f32;
                let val = ctrl.get_prop("Value").map(|v| v.as_i64()).unwrap_or(0) as f32;
                let frac = ((val - min_v) / (max_v - min_v)).clamp(0.0, 1.0);
                let accent = alpha_color(accent_color(
                    &ctrl
                        .get_prop("Accent")
                        .map(|v| v.as_str().to_owned())
                        .unwrap_or_else(|| "Blue".into()),
                ));
                let track = alpha_color(Color32::from_gray(140));
                let center = rect.center();
                let radius = (rect.width().min(rect.height()) * 0.5 - 4.0).max(6.0);
                // Knob's own 270° sweep, centred at the bottom (135°..405°,
                // i.e. start at south-west, clockwise to south-east).
                draw_ring(painter, center, radius, radius * 0.18, 135.0, 270.0, frac, accent, track);
                let show_value = ctrl
                    .get_prop("ShowValue")
                    .map(|v| v.as_bool())
                    .unwrap_or(true);
                if show_value {
                    painter.text(
                        center,
                        egui::Align2::CENTER_CENTER,
                        format!("{val:.0}"),
                        egui::FontId::proportional((radius * 0.5).max(9.0)),
                        Color32::from_rgba_premultiplied(230, 230, 230, a),
                    );
                }
            }
            CT::Gauge => {
                let style = ctrl
                    .get_prop("GaugeStyle")
                    .map(|v| v.as_str().to_owned())
                    .unwrap_or_else(|| "Radial".into());
                let min_v = ctrl.get_prop("Minimum").map(|v| v.as_i64()).unwrap_or(0) as f32;
                let max_v = ctrl
                    .get_prop("Maximum")
                    .map(|v| v.as_i64())
                    .unwrap_or(100)
                    .max(min_v as i64 + 1) as f32;
                let val = ctrl.get_prop("Value").map(|v| v.as_i64()).unwrap_or(0) as f32;
                let frac = ((val - min_v) / (max_v - min_v)).clamp(0.0, 1.0);
                let color_prop = ctrl
                    .get_prop("Color")
                    .map(|v| v.as_str().to_owned())
                    .unwrap_or_default();
                let fill = alpha_color(if color_prop.is_empty() {
                    accent_color("Blue")
                } else {
                    parse_color(&color_prop)
                });
                let track = alpha_color(Color32::from_gray(140));
                match style.as_str() {
                    "Linear" => {
                        let h = ctrl
                            .get_prop("BarHeight")
                            .map(|v| v.as_i64() as f32)
                            .unwrap_or(14.0)
                            .min(rect.height());
                        let bar =
                            egui::Rect::from_center_size(rect.center(), Vec2::new(rect.width() - 4.0, h));
                        let r = h * 0.5;
                        painter.rect_filled(bar, r, track);
                        let filled = egui::Rect::from_min_size(
                            bar.min,
                            Vec2::new(bar.width() * frac, bar.height()),
                        );
                        if frac > 0.001 {
                            painter.rect_filled(filled, r, fill);
                        }
                    }
                    "Donut" => {
                        let center = rect.center();
                        let radius = (rect.width().min(rect.height()) * 0.5 - 4.0).max(6.0);
                        let stroke_w = ctrl
                            .get_prop("StrokeWidth")
                            .map(|v| v.as_i64() as f32)
                            .unwrap_or(8.0);
                        draw_ring(painter, center, radius, stroke_w, -90.0, 360.0, frac, fill, track);
                    }
                    _ => {
                        // Radial: a half-circle speedometer, sweeping the top.
                        let center = Pos2::new(rect.center().x, rect.bottom() - 6.0);
                        let radius = (rect.width() * 0.5 - 4.0).min(rect.height() - 10.0).max(6.0);
                        draw_ring(painter, center, radius, radius * 0.18, 180.0, 180.0, frac, fill, track);
                    }
                }
                painter.text(
                    rect.center(),
                    egui::Align2::CENTER_CENTER,
                    format!("{val:.0}"),
                    egui::FontId::proportional((rect.height() * 0.22).clamp(9.0, 18.0)),
                    Color32::from_rgba_premultiplied(230, 230, 230, a),
                );
            }
            CT::Switch => {
                let checked = ctrl
                    .get_prop("Checked")
                    .map(|v| v.as_bool())
                    .unwrap_or(false);
                let accent = alpha_color(accent_color(
                    &ctrl
                        .get_prop("Accent")
                        .map(|v| v.as_str().to_owned())
                        .unwrap_or_else(|| "Blue".into()),
                ));
                let off = alpha_color(Color32::from_gray(110));
                let track_h = rect.height().min(18.0);
                let track = egui::Rect::from_center_size(
                    rect.center(),
                    Vec2::new((rect.width()).min(32.0), track_h),
                );
                let r = track_h * 0.5;
                painter.rect_filled(track, r, if checked { accent } else { off });
                let knob_d = track_h - 4.0;
                let knob_x = if checked {
                    track.max.x - r
                } else {
                    track.min.x + r
                };
                painter.circle_filled(
                    Pos2::new(knob_x, track.center().y),
                    knob_d * 0.5,
                    Color32::from_rgba_premultiplied(a, a, a, a),
                );
            }
            _ /* FileDropZone */ => {
                let stroke = Stroke::new(1.5, Color32::from_rgba_premultiplied(140, 140, 140, a));
                // A dashed rounded-rect border — egui has no built-in dashed
                // rect stroke, so it is a handful of short segments per edge.
                let r = rect.shrink(2.0);
                let dash = 6.0_f32;
                let gap = 4.0_f32;
                let mut edge = |p0: Pos2, p1: Pos2| {
                    let len = p0.distance(p1);
                    let dir = (p1 - p0) / len.max(0.001);
                    let mut d = 0.0_f32;
                    while d < len {
                        let seg_end = (d + dash).min(len);
                        painter.line_segment([p0 + dir * d, p0 + dir * seg_end], stroke);
                        d += dash + gap;
                    }
                };
                edge(r.left_top(), r.right_top());
                edge(r.right_top(), r.right_bottom());
                edge(r.right_bottom(), r.left_bottom());
                edge(r.left_bottom(), r.left_top());
                let hint = ctrl
                    .get_prop("Hint")
                    .map(|v| v.as_str().to_owned())
                    .unwrap_or_default();
                let label = if hint.is_empty() {
                    "Drop files here".to_owned()
                } else {
                    hint
                };
                painter.text(
                    rect.center(),
                    egui::Align2::CENTER_CENTER,
                    label,
                    egui::FontId::proportional(12.0),
                    Color32::from_rgba_premultiplied(180, 180, 180, a),
                );
            }
        }
        return;
    }

    // ── Maps (spec 039 T9) ──────────────────────────────────────────────────
    // Same shared `map_tiles::paint_map` the interactive path (render.rs)
    // uses — a `Painter` carries its own `Context` (`Painter::ctx()`), which
    // is all texture upload needs, so the designer canvas's static face
    // shows the SAME real OSM tiles the running form does, not a simplified
    // proxy like Knob/Gauge/Switch/FileDropZone's static faces above (those
    // stand in for a live *widget*; a basemap has no such off-the-shelf
    // widget to substitute for in the first place).
    if matches!(ctrl.control_type, CT::Maps) {
        let center_lat: f64 = ctrl
            .get_prop("CenterLat")
            .map(|v| v.as_str().to_owned())
            .unwrap_or_default()
            .parse()
            .unwrap_or(0.0);
        let center_lng: f64 = ctrl
            .get_prop("CenterLng")
            .map(|v| v.as_str().to_owned())
            .unwrap_or_default()
            .parse()
            .unwrap_or(0.0);
        let zoom = ctrl
            .get_prop("Zoom")
            .map(|v| v.as_i64())
            .unwrap_or(2)
            .clamp(crate::map_tiles::MIN_ZOOM as i64, crate::map_tiles::MAX_ZOOM as i64)
            as u8;
        let markers_raw = ctrl
            .get_prop("Markers")
            .map(|v| v.as_str().to_owned())
            .unwrap_or_default();
        let records = crate::parse_map_markers(&markers_raw);
        let markers: Vec<crate::map_tiles::MapMarker> = records
            .iter()
            .map(|m| crate::map_tiles::MapMarker {
                lat: m.lat,
                lng: m.lng,
                label: &m.label,
            })
            .collect();
        crate::map_tiles::paint_map(painter, rect, center_lat, center_lng, zoom, &markers, None);
        return;
    }

    // ── ProgressBar ───────────────────────────────────────────────────────────
    if matches!(ctrl.control_type, CT::ProgressBar) {
        // In Neumorphic mode draw the dual-shadow halo BEFORE the bar's own artwork.
        if is_neumorphic {
            let corner_r = ctrl
                .get_prop("CornerRadius")
                .map(|v| v.as_i64() as f32)
                .unwrap_or(4.0);
            draw_neumorphic_shadow_only(painter, rect, corner_r, alpha_mul);
        }
        let bg_c = Color32::from_rgba_premultiplied(220, 220, 220, a);
        let bar_c = alpha_color(
            ctrl.get_prop("BarColor")
                .map(|v| parse_color(v.as_str()))
                .unwrap_or(Color32::from_rgb(0, 170, 0)),
        );
        let val = ctrl.get_prop("Value").map(|v| v.as_i64()).unwrap_or(0) as f32;
        let min = ctrl.get_prop("Minimum").map(|v| v.as_i64()).unwrap_or(0) as f32;
        let max = ctrl
            .get_prop("Maximum")
            .map(|v| v.as_i64())
            .unwrap_or(100)
            .max(1) as f32;
        let pct = ((val - min) / (max - min)).clamp(0.0, 1.0);
        painter.rect_filled(rect, 2.0, bg_c);
        let bar = egui::Rect::from_min_size(rect.min, Vec2::new(rect.width() * pct, rect.height()));
        if glass {
            draw_glass_auto(
                painter,
                bar,
                Color32::from_rgb(0, 170, 0),
                2.0,
                false,
                alpha_mul * pct,
            );
        } else {
            painter.rect_filled(bar, 2.0, bar_c);
        }
        let border_c = if selected {
            Color32::from_rgba_premultiplied(60, 120, 230, a)
        } else {
            Color32::from_rgba_premultiplied(140, 140, 160, a)
        };
        painter.rect_stroke(
            rect,
            2.0,
            Stroke::new(if selected { 2.0 } else { 1.0 }, border_c),
            egui::StrokeKind::Middle,
        );
        if ctrl
            .get_prop("ShowValue")
            .map(|v| v.as_bool())
            .unwrap_or(false)
        {
            painter.text(
                rect.center(),
                egui::Align2::CENTER_CENTER,
                format!("{:.0}%", pct * 100.0),
                egui::FontId::proportional(ctrl_font_size(ctrl)),
                Color32::from_rgba_premultiplied(0, 0, 0, a),
            );
        }
        if is_neumorphic {
            draw_neumorphic_overlay_shadow_only(painter, rect, 2.0, alpha_mul);
        }
        if let Some(shadow) = regular_shadow.as_ref().filter(|shadow| shadow.overlay) {
            draw_regular_drop_shadow(painter, shadow, alpha_mul);
        }
        return;
    }

    // ── Generic rect-based controls ───────────────────────────────────────────

    let (default_fill, default_border, default_text) = control_colors(&ctrl.control_type, selected);

    let is_container = matches!(ctrl.control_type, CT::GroupBox | CT::Panel);
    // In Neumorphic, the BackgroundColor IS the surface fill for ALL controls
    // including containers. In Classic/Enhanced, containers ignore it (their
    // content comes from children).

    let fill = if is_container && !is_neumorphic {
        default_fill
    } else {
        ctrl.get_prop("BackgroundColor")
            .map(|v| parse_color(v.as_str()))
            .unwrap_or(if is_neumorphic {
                // Default neumorphic surface: soft lavender-blue (#E8EDFE)
                Color32::from_rgb(232, 237, 254)
            } else {
                default_fill
            })
    };
    let label_color = ctrl
        .get_prop("ForegroundColor")
        .map(|v| parse_color(v.as_str()))
        .unwrap_or(if is_neumorphic {
            Color32::BLACK // Neumorphic default: black text on light surface
        } else {
            default_text
        });
    let stroke_color = ctrl
        .get_prop("BorderColor")
        .map(|v| parse_color(v.as_str()))
        .unwrap_or(default_border);
    let border_style = ctrl
        .get_prop("BorderStyle")
        .map(|v| v.as_str().to_owned())
        .unwrap_or_else(|| "Single".into());
    let user_border_width = ctrl
        .get_prop("BorderWidth")
        .map(|v| v.as_i64() as f32)
        .unwrap_or(1.0);

    // Unified corner radius for every control (spec 016): canonical CornerRadius,
    // legacy BorderRadius alias, per-type default, clamped. 0 ⇒ square.
    let corner = corner_radius(ctrl);
    // Per-corner frame rounding: the control's own radius, lifted to the container
    // radius on any corner that lands on a rounded GroupBox/Panel border — so the
    // card/background is cut by the parent shape and never bleeds past its rounded
    // corner (spec 017). Equals `corner` on all four corners when free-standing.
    let frame_round = control_border_rounding(ctrl, frame_rect, corner);

    let is_label = matches!(ctrl.control_type, CT::Label);

    // The BackgroundColor the developer explicitly chose, if any. The universal
    // seeded default and the values the Neumorphic style appliers stamp on
    // every control all mean "not chosen" (the renderer-wide "still on the
    // default means the user has not picked" convention) — the styled face
    // keeps its unit unless the developer deliberately broke it. When Some,
    // the colour is painted as a solid, opacity-aware layer under the styled
    // face (spec 019's DataGrid underlay, generalised), and a Label gains a
    // face at all instead of staying frameless.
    let user_bg: Option<Color32> = user_background_color(ctrl);

    // A PictureBox with ShowFrame = false draws no card/background/border —
    // only the image (so transparent PNG areas reveal what's behind).
    let pic_frameless = matches!(ctrl.control_type, CT::PictureBox)
        && !ctrl
            .get_prop("ShowFrame")
            .map(|v| v.as_bool())
            .unwrap_or(true);

    // Charts own their full card/background in `draw_chart_preview`. Drawing the
    // generic glass frame first leaves an extra dark under-frame that can show
    // through rounded corner notches in preview/run surfaces.
    let chart_frameless = matches!(
        ctrl.control_type,
        CT::BarChart
            | CT::LineChart
            | CT::PieChart
            | CT::AreaChart
            | CT::ScatterChart
            | CT::DonutChart
    );

    // A container (GroupBox/Panel) with HideBackground draws no fill/border
    // (children stay visible); with a background gradient enabled it fills with
    // a directional gradient instead of the default glass/solid fill.
    let container_frameless = is_container
        && ctrl
            .get_prop("HideBackground")
            .map(|v| v.as_bool())
            .unwrap_or(false);
    let background_gradient = !container_frameless
        && ctrl
            .get_prop("BackgroundGradientEnabled")
            .map(|v| v.as_bool())
            .unwrap_or(false);

    // 007 Form themes — when an asset-pack theme is active and covers this
    // control kind, 9-slice its skin instead of the procedural glass; controls
    // the pack doesn't cover fall through to Liquid Glass (R6, R7, R11).
    let theme_skin = active_theme(painter.ctx()).and_then(|pack| {
        let key = control_kind_key(&ctrl.control_type);
        if key.is_empty() {
            return None;
        }
        pack.control(key).map(|skin| (pack.clone(), skin.clone()))
    });

    if (is_label && !background_gradient && user_bg.is_none())
        || pic_frameless
        || chart_frameless
        || container_frameless
    {
        // No visible frame. When selected, show a lightweight selection outline.
        if is_container {
            debug_frame(
                painter,
                frame_rect,
                frame_round,
                1,
                "CONTAINER_FRAMELESS",
                container_diag,
            );
        }
        if selected {
            let sel_c = Color32::from_rgba_premultiplied(60, 120, 230, a);
            painter.rect_stroke(rect, 0.0, Stroke::new(1.0, sel_c), egui::StrokeKind::Middle);
        }
    } else if background_gradient {
        // Shared eight-direction gradient background. The mesh follows the same
        // per-corner rounding as the border, so the fill never bleeds past it.
        let dir = ctrl
            .get_prop("BackgroundGradientDirection")
            .map(|v| v.as_str().to_owned())
            .unwrap_or_else(|| "South".into());
        let start = alpha_color(
            ctrl.get_prop("BackgroundGradientStartColor")
                .map(|v| parse_color(v.as_str()))
                .unwrap_or(fill),
        );
        let end = alpha_color(
            ctrl.get_prop("BackgroundGradientEndColor")
                .map(|v| parse_color(v.as_str()))
                .unwrap_or(fill),
        );
        if is_neumorphic {
            draw_neumorphic_shadow_only(painter, frame_rect, frame_round, alpha_mul);
        }
        let face_rect = debug_frame(
            painter,
            frame_rect,
            frame_round,
            1,
            "CONTROL_GRADIENT",
            container_diag,
        );
        painter.add(egui::Shape::mesh(background_gradient_mesh(
            face_rect,
            start,
            end,
            &dir,
            frame_round,
        )));
        if is_neumorphic {
            draw_neumorphic_overlay_shadow_only(painter, frame_rect, frame_round, alpha_mul);
        }
        let bc = if selected {
            Color32::from_rgba_premultiplied(60, 120, 230, a)
        } else {
            alpha_color(stroke_color)
        };
        if selected || (border_style != "None" && user_border_width >= 0.5) {
            let border_rect = debug_frame(
                painter,
                frame_rect,
                frame_round,
                2,
                "CONTROL_GRADIENT_BORDER",
                container_diag,
            );
            painter.rect_stroke(
                border_rect,
                frame_round,
                Stroke::new(if selected { 2.0 } else { user_border_width }, bc),
                egui::StrokeKind::Middle,
            );
        }
    } else if let Some((pack, skin)) = &theme_skin {
        let state = if selected {
            ControlState::Focused
        } else {
            ControlState::Normal
        };
        if let Some(tex) = load_pack_texture(painter.ctx(), pack, skin.image_for(state)) {
            // Explicit BackgroundColor (R12) tints the skin; otherwise white = as-authored.
            // Nine-slice skins are drawn square — overlay reflects that (ZERO).
            let skin_rect = if is_container {
                debug_frame(
                    painter,
                    frame_rect,
                    egui::CornerRadius::ZERO,
                    1,
                    "CONTAINER_THEME",
                    container_diag,
                )
            } else {
                frame_rect
            };
            let tint = Color32::from_white_alpha(a);
            draw_nine_slice(painter, skin_rect, &tex, skin.slice, tint);
            if selected {
                let sel_rect = if is_container {
                    debug_frame(
                        painter,
                        frame_rect,
                        egui::CornerRadius::same(crate::paint::cr8(corner)),
                        2,
                        "CONTAINER_SELECTED",
                        container_diag,
                    )
                } else {
                    frame_rect
                };
                painter.rect_stroke(
                    sel_rect,
                    corner,
                    Stroke::new(2.0, Color32::from_rgba_premultiplied(60, 120, 230, a)),
                    egui::StrokeKind::Middle,
                );
            }
        } else {
            // Image missing / undecodable → never fail; fall back to glass (R11).
            let fallback_rect = if is_container {
                debug_frame(
                    painter,
                    frame_rect,
                    frame_round,
                    1,
                    "CONTAINER_THEME_FALLBACK",
                    container_diag,
                )
            } else {
                frame_rect
            };
            draw_glass_auto(
                painter,
                fallback_rect,
                fill,
                frame_round,
                selected,
                alpha_mul,
            );
        }
    } else if glass {
        let glass_rect = if is_container {
            debug_frame(
                painter,
                frame_rect,
                frame_round,
                1,
                "CONTAINER_GLASS",
                container_diag,
            )
        } else {
            frame_rect
        };
        // An explicit user background rides under the styled face (solid in
        // Classic/Enhanced, the surface itself in Neumorphic) so "the
        // background selected" is actually visible on styled forms.
        draw_glass_auto_bg(
            painter,
            glass_rect,
            fill,
            user_bg,
            frame_round,
            selected,
            alpha_mul,
        );
        // When the control has an explicit BorderStyle + BorderWidth, draw the
        // user border on top of the glass frame so containers (Panel, GroupBox)
        // honour the same border properties as non-glass controls. Neumorphic
        // uses an asymmetric border (light top/left, dark bottom/right).
        if border_style != "None" && user_border_width > 0.5 {
            let border_rect = if is_container {
                debug_frame(
                    painter,
                    frame_rect,
                    frame_round,
                    2,
                    "CONTAINER_BORDER",
                    container_diag,
                )
            } else {
                frame_rect
            };
            if is_neumorphic {
                draw_neumorphic_user_border(
                    painter,
                    border_rect,
                    frame_round,
                    user_border_width,
                    alpha_mul,
                );
            } else {
                let bw = if selected {
                    2.0_f32.max(user_border_width)
                } else {
                    user_border_width
                };
                let bc = if selected {
                    Color32::from_rgba_premultiplied(60, 120, 230, a)
                } else {
                    alpha_color(stroke_color)
                };
                draw_control_border(painter, border_rect, frame_round, &border_style, bw, bc);
            }
        }
        // Buttons get a subtle top specular — a soft vertical light reflection
        // that visually separates a clickable Button from flat fields like a
        // TextBox. Two stacked translucent bands fading downward.
        // Suppressed under Neumorphic (the dual soft shadows + rims already give
        // the relief; the band would fight the top-left lighting assumption).
        if matches!(ctrl.control_type, CT::Button) && rect.height() > 10.0 && !is_neumorphic {
            let inset = (corner + 3.0).min(rect.width() * 0.25);
            let spec_h = (rect.height() * 0.30).clamp(3.0, 9.0);
            let band = |h: f32, alpha: u8| {
                let sa = (alpha as f32 * alpha_mul) as u8;
                painter.rect_filled(
                    egui::Rect::from_min_size(
                        rect.min + Vec2::new(inset, 2.0),
                        Vec2::new((rect.width() - 2.0 * inset).max(0.0), h),
                    ),
                    (corner - 1.0).max(2.0),
                    Color32::from_rgba_premultiplied(sa, sa, sa, sa),
                );
            };
            band(spec_h, 16); // wide soft glow
            band(spec_h * 0.45, 22); // narrower brighter core
        }
    } else {
        // For a container, `debug_frame` explodes the frame out of the stack (60px
        // per slot) and returns the shifted rect so the real fill lands on its own;
        // non-containers get `frame_rect` back untouched.
        let face_rect = if is_container {
            debug_frame(
                painter,
                frame_rect,
                frame_round,
                1,
                "CONTAINER_FACE",
                container_diag,
            )
        } else {
            frame_rect
        };
        painter.rect_filled(face_rect, frame_round, alpha_color(fill));
        if border_style != "None" {
            let bw = if selected {
                2.0_f32.max(user_border_width)
            } else {
                user_border_width
            };
            let bc = if selected {
                Color32::from_rgba_premultiplied(60, 120, 230, a)
            } else {
                alpha_color(stroke_color)
            };
            let border_rect = if is_container {
                debug_frame(
                    painter,
                    frame_rect,
                    frame_round,
                    2,
                    "CONTAINER_BORDER",
                    container_diag,
                )
            } else {
                frame_rect
            };
            draw_control_border(painter, border_rect, frame_round, &border_style, bw, bc);
        } else if selected {
            let sel_rect = if is_container {
                debug_frame(
                    painter,
                    frame_rect,
                    frame_round,
                    2,
                    "CONTAINER_SELECTED",
                    container_diag,
                )
            } else {
                frame_rect
            };
            painter.rect_stroke(
                sel_rect,
                frame_round,
                Stroke::new(2.0, Color32::from_rgba_premultiplied(60, 120, 230, a)),
                egui::StrokeKind::Middle,
            );
        }
    }

    // ── TabControl tab strip (spec 012) ────────────────────────────────────────
    // Draw a real strip of tabs across the top, highlighting the selected page.
    // The active page index is the `SelectedTab` property (the designer updates it
    // when a tab is clicked). Shared renderers defer this strip until after
    // children, so tab titles remain chrome/overlay rather than clipped content.
    if matches!(ctrl.control_type, CT::TabControl)
        && !ctrl
            .get_prop("_DeferTabs")
            .map(|v| v.as_bool())
            .unwrap_or(false)
    {
        draw_tabcontrol_tabs(painter, rect.min, ctrl, alpha_mul);
    }

    // ── GroupBox caption — a "legend" on the top-left border, just past the
    // rounded corner (classic GroupBox look), vertically centred on the border
    // line. Suppressed by HideCaption (spec 015). ─────────────────────────────
    if matches!(ctrl.control_type, CT::GroupBox)
        && !ctrl
            .get_prop("_DeferCaption")
            .map(|v| v.as_bool())
            .unwrap_or(false)
        && !ctrl
            .get_prop("HideCaption")
            .map(|v| v.as_bool())
            .unwrap_or(false)
    {
        draw_groupbox_caption(painter, rect.min, ctrl, alpha_mul);
    }

    // Label text — Caption is on Label, Button, CheckBox, RadioButton.
    let label: String = match ctrl.control_type {
        // The check glyph is a real drawn square + checkmark (below), not
        // text — the caption is JUST the caption here.
        CT::CheckBox => ctrl
            .get_prop("Caption")
            .map(|v| v.as_str().to_owned())
            .unwrap_or_else(|| ctrl.id.clone()),
        CT::RadioButton => {
            let checked = ctrl
                .get_prop("Checked")
                .map(|v| v.as_bool())
                .unwrap_or(false);
            let cap = ctrl
                .get_prop("Caption")
                .map(|v| v.as_str().to_owned())
                .unwrap_or_else(|| ctrl.id.clone());
            format!("{} {cap}", if checked { "(●)" } else { "( )" })
        }
        CT::ComboBox => {
            let items = ctrl
                .get_prop("Items")
                .map(|v| v.as_str().to_owned())
                .unwrap_or_default();
            format!("{} ▾", items.lines().next().unwrap_or(""))
        }
        CT::DateTimePicker => {
            let val = ctrl
                .get_prop("Value")
                .map(|v| v.as_str().to_owned())
                .filter(|s| !s.is_empty())
                .unwrap_or_else(|| "DD/MM/YYYY".into());
            format!("📅 {val}")
        }
        CT::NumericUpDown => {
            let v = ctrl.get_prop("Value").map(|v| v.as_i64()).unwrap_or(0);
            format!("{v} ▲▼")
        }
        CT::PictureBox => {
            let image_path = ctrl
                .get_prop("ImagePath")
                .map(|v| v.as_str().to_owned())
                .unwrap_or_default();
            let size_mode = ctrl
                .get_prop("SizeMode")
                .map(|v| v.as_str().to_owned())
                .unwrap_or_else(|| "Normal".into());
            if draw_picturebox_image(
                painter,
                rect,
                image_path.trim(),
                &size_mode,
                alpha_mul,
                corner,
            ) {
                if selected {
                    painter.rect_stroke(
                        rect,
                        corner,
                        Stroke::new(2.0, Color32::from_rgba_premultiplied(60, 120, 230, a)),
                        egui::StrokeKind::Middle,
                    );
                }
                if let Some(shadow) = regular_shadow.as_ref().filter(|shadow| shadow.overlay) {
                    draw_regular_drop_shadow(painter, shadow, alpha_mul);
                }
                return;
            }
            // Fallback for legacy callers that provide a ready raster texture.
            if let Some(tex_id) = pic_tex {
                let size_mode = ctrl
                    .get_prop("SizeMode")
                    .map(|v| v.as_str().to_owned())
                    .unwrap_or_else(|| "Normal".into());
                // Honour SizeMode with the image's native size so the aspect ratio
                // is preserved (Fit/Zoom/Center) identically to the run/preview —
                // the native size comes from the texture manager (spec 017 parity).
                let native = painter
                    .ctx()
                    .tex_manager()
                    .read()
                    .meta(tex_id)
                    .map(|m| Vec2::new(m.size[0] as f32, m.size[1] as f32))
                    .unwrap_or_else(|| rect.size());
                draw_media_image(
                    painter,
                    rect,
                    tex_id,
                    native,
                    pic_size_mode(&size_mode),
                    a,
                    ctrl,
                    corner,
                );
                // Selection border on top
                if selected {
                    painter.rect_stroke(
                        rect,
                        corner,
                        Stroke::new(2.0, Color32::from_rgba_premultiplied(60, 120, 230, a)),
                        egui::StrokeKind::Middle,
                    );
                }
                if let Some(shadow) = regular_shadow.as_ref().filter(|shadow| shadow.overlay) {
                    draw_regular_drop_shadow(painter, shadow, alpha_mul);
                }
                return; // skip generic text rendering below
            }
            // No image loaded — show placeholder text
            if ctrl
                .get_prop("ImagePath")
                .map(|v| !v.as_str().is_empty())
                .unwrap_or(false)
            {
                "🖼 [loading…]".into()
            } else {
                "🖼 (empty)".into()
            }
        }
        CT::Animator => {
            let source = ctrl
                .get_prop("Source")
                .map(|v| v.as_str().to_owned())
                .unwrap_or_default();
            let auto = ctrl
                .get_prop("AutoPlay")
                .map(|v| v.as_bool())
                .unwrap_or(true);
            let looping = ctrl.get_prop("Loop").map(|v| v.as_bool()).unwrap_or(true);
            let size_mode = ctrl
                .get_prop("SizeMode")
                .map(|v| v.as_str().to_owned())
                .unwrap_or_else(|| "Fit".into());
            let key = format!("{}|{}", ctrl.id, source.trim());
            draw_animator(
                painter,
                rect,
                ctrl,
                &key,
                source.trim(),
                auto,
                looping,
                &size_mode,
                alpha_mul,
                selected,
            );
            if let Some(shadow) = regular_shadow.as_ref().filter(|shadow| shadow.overlay) {
                draw_regular_drop_shadow(painter, shadow, alpha_mul);
            }
            return;
        }
        CT::TreeView => "🌲 [TreeView]".into(),
        CT::DataGrid => {
            let cols = ctrl
                .get_prop("Columns")
                .map(|v| v.as_str().to_owned())
                .unwrap_or_default();
            let col_count = cols.lines().count().max(1);
            format!("⊞ DataGrid ({col_count} cols)")
        }
        CT::Splitter => {
            let dir = ctrl
                .get_prop("Orientation")
                .map(|v| v.as_str().to_owned())
                .unwrap_or_else(|| "H".into());
            if dir.starts_with('V') {
                "║ Splitter".into()
            } else {
                "═ Splitter".into()
            }
        }
        // The tab strip is drawn above; no centered label.
        CT::TabControl => String::new(),
        CT::MenuBar => {
            if let Some(def) = get_menu_cache(painter.ctx(), &ctrl.id) {
                if def.menu.is_empty() {
                    "☰ MenuBar (empty)".into()
                } else {
                    String::new()
                }
            } else {
                "☰ MenuBar (empty)".into()
            }
        }
        CT::ToolBar => "⬛ ToolBar".into(),
        CT::StatusBar => "▬ StatusBar".into(),
        // GroupBox draws its caption as a "legend" on the top-left border (below),
        // never as centered text.
        CT::GroupBox => String::new(),
        // Controls with an intrinsic text label use their Caption property.
        CT::Label | CT::Button => ctrl
            .get_prop("Caption")
            .map(|v| v.to_string())
            .unwrap_or_else(|| ctrl.id.clone()),
        // TextBox shows its current text value.
        CT::TextBox => ctrl
            .get_prop("Text")
            .map(|v| v.to_string())
            .unwrap_or_default(),
        // Non-text controls (Panel, …) draw no caption — only GroupBox and Label
        // (and the text-bearing widgets above) carry one.
        _ => String::new(),
    };
    // An empty TextBox previews its HintText (faded) so the designer face
    // matches what the run form shows for an empty box.
    let textbox_hint = matches!(ctrl.control_type, CT::TextBox) && label.is_empty();
    let label = if textbox_hint {
        ctrl.get_prop("HintText")
            .map(|v| v.to_string())
            .unwrap_or_default()
    } else {
        label
    };

    // ── CheckBox: a real drawn box + checkmark, not "[ ]"/"[✓]" bracket text.
    // Runs OUTSIDE the `!label.is_empty()` gate below because the box must
    // show even when the developer left Caption empty. `checkbox_text_rect`
    // narrows the caption's own layout area so text never overlaps the box.
    let mut checkbox_text_rect = rect;
    if matches!(ctrl.control_type, CT::CheckBox) {
        let checked = ctrl
            .get_prop("Checked")
            .map(|v| v.as_bool())
            .unwrap_or(false);
        let box_fsize = ctrl_font_size(ctrl);
        let box_d = (box_fsize * 1.25).clamp(12.0, (rect.height() - 4.0).max(10.0));
        let box_round = (box_d * 0.22).clamp(2.0, 5.0);
        let pad = 4.0_f32.min(rect.width() * 0.08);
        let gap = 6.0_f32.min(rect.width() * 0.08);
        let right_aligned = ctrl
            .get_prop("CheckAlignment")
            .map(|v| v.as_str().eq_ignore_ascii_case("Right"))
            .unwrap_or(false);
        let box_x = if right_aligned {
            rect.right() - pad - box_d
        } else {
            rect.left() + pad
        };
        let box_rect = egui::Rect::from_min_size(
            egui::pos2(box_x, rect.center().y - box_d / 2.0),
            Vec2::splat(box_d),
        );
        // Same theme dispatch (Classic / Enhanced / Neumorphic Light /
        // Neumorphic Dark) every other control's face already uses, so the
        // check glyph always matches the active GlassStyle instead of being
        // flat monospace brackets regardless of theme.
        draw_glass_auto(painter, box_rect, fill, box_round, false, alpha_mul);
        if checked {
            let check_color = ctrl
                .get_prop("CheckColor")
                .map(|v| parse_color(v.as_str()))
                .unwrap_or(Color32::from_rgb(0, 120, 215));
            let cc = alpha_color(check_color);
            // CheckSize: 0-100, percentage of the box the checkmark fills.
            let check_pct = ctrl
                .get_prop("CheckSize")
                .map(|v| v.as_i64())
                .unwrap_or(70)
                .clamp(10, 100) as f32
                / 100.0;
            let stroke_w = (box_d * 0.16 * check_pct).clamp(1.5, 6.0);
            let pt = |ux: f32, uy: f32| -> Pos2 {
                Pos2::new(
                    box_rect.center().x + (ux - 0.5) * box_d * check_pct,
                    box_rect.center().y + (uy - 0.5) * box_d * check_pct,
                )
            };
            let stroke = Stroke::new(stroke_w, cc);
            painter.line_segment([pt(0.18, 0.52), pt(0.42, 0.76)], stroke);
            painter.line_segment([pt(0.42, 0.76), pt(0.84, 0.22)], stroke);
        }
        checkbox_text_rect = if right_aligned {
            egui::Rect::from_min_max(rect.min, egui::pos2(box_rect.left() - gap, rect.max.y))
        } else {
            egui::Rect::from_min_max(egui::pos2(box_rect.right() + gap, rect.min.y), rect.max)
        };
    }

    if !label.is_empty() {
        let txt_color =
            Color32::from_rgba_premultiplied(label_color.r(), label_color.g(), label_color.b(), a);
        let txt_color = if textbox_hint {
            txt_color.gamma_multiply(0.55)
        } else {
            txt_color
        };
        let fsize = ctrl_font_size(ctrl);
        let font_name = ctrl
            .get_prop("FontName")
            .map(|v| v.as_str())
            .unwrap_or_default();

        if matches!(ctrl.control_type, CT::Button) {
            let text_alignment = ctrl
                .get_prop("TextAlignment")
                .map(|v| v.as_str())
                .unwrap_or("MiddleCenter");
            let galley = painter.layout_job(styled_text_job(
                painter,
                ctrl,
                &label,
                &font_name,
                fsize,
                txt_color,
                f32::INFINITY,
                egui::Align::LEFT,
            ));
            let image_path = ctrl
                .get_prop("IconPath")
                .or_else(|| ctrl.get_prop("ImagePath"))
                .map(|v| v.as_str().trim().to_owned())
                .unwrap_or_default();
            let is_svg_icon = button_svg_icon_available(&image_path);
            let raster_texture = if is_svg_icon {
                None
            } else {
                picturebox_texture(painter.ctx(), &image_path)
            };
            let image_alignment = button_image_alignment(
                ctrl.get_prop("IconAlignment")
                    .or_else(|| ctrl.get_prop("ImageAlignment"))
                    .map(|v| v.as_str())
                    .unwrap_or("Left"),
            );
            let image_size = if is_svg_icon {
                Some(button_image_slot(ctrl))
            } else {
                raster_texture
                    .as_ref()
                    .map(|tex| button_image_size(tex.size_vec2(), button_image_slot(ctrl)))
            };
            let (text_pos, image_rect) = button_content_layout(
                rect,
                galley.size(),
                image_size,
                image_alignment,
                button_image_padding(ctrl),
                text_alignment,
            );
            let texture = image_rect.and_then(|img_rect| {
                if is_svg_icon {
                    picturebox_svg_texture(painter.ctx(), &image_path, img_rect.size())
                } else {
                    raster_texture.clone()
                }
                .map(|tex| (tex, img_rect))
            });
            if let Some((tex, img_rect)) = texture {
                if img_rect.width() > 0.5 && img_rect.height() > 0.5 {
                    painter.image(
                        tex.id(),
                        img_rect,
                        Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
                        Color32::from_white_alpha(a),
                    );
                }
            }
            paint_styled_galley(painter, ctrl, text_pos, galley, txt_color);
        } else if matches!(ctrl.control_type, CT::Label) {
            // Honour the Label's TextAlignment (Left / Center / Right /
            // Justified) and VerticalAlignment (Top / Middle / Bottom).
            let align_raw = ctrl
                .get_prop("TextAlignment")
                .map(|v| v.as_str().to_owned())
                .unwrap_or_default();
            let halign = text_halign(&align_raw);

            let mut job = styled_text_job(
                painter,
                ctrl,
                &label,
                &font_name,
                fsize,
                txt_color,
                rect.width(),
                halign,
            );
            job.justify = text_justified(&align_raw);
            let galley = painter.layout_job(job);
            // The galley's draw origin follows `halign`: top-left for LEFT,
            // top-centre for CENTER, top-right for RIGHT. Anchor x to the
            // matching edge of the rect (with a small inset off the border);
            // y follows VerticalAlignment (Middle on forms without it).
            let pad = 3.0_f32.min(rect.width() * 0.25);
            let anchor_x = match halign {
                egui::Align::Center => rect.center().x,
                egui::Align::RIGHT => rect.right() - pad,
                _ => rect.left() + pad,
            };
            let vpad = 2.0_f32.min(rect.height() * 0.25);
            let anchor_y = match text_valign(
                ctrl.get_prop("VerticalAlignment")
                    .map(|v| v.as_str())
                    .unwrap_or(""),
            ) {
                egui::Align::TOP => rect.top() + vpad,
                egui::Align::BOTTOM => rect.bottom() - vpad - galley.size().y,
                _ => rect.center().y - galley.size().y / 2.0,
            };
            let text_pos = egui::pos2(anchor_x, anchor_y);
            paint_styled_galley(painter, ctrl, text_pos, galley, txt_color);
        } else if matches!(ctrl.control_type, CT::TextBox) {
            // Inset by at least the corner radius so text stays inside the rounded
            // arc and never bleeds past the box's own rounded corners.
            let pad = textbox_inner_padding(ctrl)
                .max(corner_radius(ctrl))
                .min(rect.width() * 0.45);
            let multiline = ctrl
                .get_prop("Multiline")
                .map(|v| v.as_bool())
                .unwrap_or(false);
            // TextAlignment (Left / Center / Right / Justified) and
            // VerticalAlignment (Top / Middle / Bottom) — defaults preserve
            // the historical left / centred single line.
            let align_raw = ctrl
                .get_prop("TextAlignment")
                .map(|v| v.as_str().to_owned())
                .unwrap_or_default();
            let halign = text_halign(&align_raw);
            let valign = text_valign(
                ctrl.get_prop("VerticalAlignment")
                    .map(|v| v.as_str())
                    .unwrap_or(""),
            );
            let anchor_x = |inset: f32| match halign {
                egui::Align::Center => rect.center().x,
                egui::Align::RIGHT => rect.right() - inset,
                _ => rect.left() + inset,
            };
            if multiline {
                // Multiline preview: lay the value out from the top, wrapping to
                // the field width when WordWrap is on (matching the run-time
                // editor), clipped to the control so long text doesn't spill past
                // the border. Vertical alignment stays top-anchored — the runtime
                // editor scrolls, so Middle/Bottom have no stable meaning there.
                let word_wrap = ctrl
                    .get_prop("WordWrap")
                    .map(|v| v.as_bool())
                    .unwrap_or(true);
                let max_width = if word_wrap {
                    (rect.width() - 2.0 * pad).max(1.0)
                } else {
                    f32::INFINITY
                };
                let mut job = styled_text_job(
                    painter,
                    ctrl,
                    &label,
                    &font_name,
                    fsize,
                    txt_color,
                    max_width,
                    halign,
                );
                // Justification needs a finite wrap width to stretch lines to.
                job.justify = text_justified(&align_raw) && word_wrap;
                let galley = painter.layout_job(job);
                // Clip to the padded inner rect so wrapped lines stay clear of the
                // rounded corners (top/bottom) instead of spilling past the arc.
                let clipped = painter.with_clip_rect(rect.shrink(pad));
                paint_styled_galley(
                    &clipped,
                    ctrl,
                    egui::pos2(anchor_x(pad), rect.top() + pad),
                    galley,
                    txt_color,
                );
            } else {
                // Single line: the box has NO fixed top/bottom padding — the line
                // sits in the FULL height, so a larger font uses all of it before
                // anything else. If the line is still taller than the box, SHRINK
                // the font until it fits so the text is never clipped by the
                // top/bottom border (never truncated instead).
                let inner_w = (rect.width() - 2.0 * pad).max(1.0);
                let min_font = 6.0_f32;
                let mut fit = fsize.max(min_font);
                let layout = |size: f32| {
                    painter.layout_job(styled_text_job(
                        painter,
                        ctrl,
                        &label,
                        &font_name,
                        size,
                        txt_color,
                        inner_w,
                        halign,
                    ))
                };
                let mut galley = layout(fit);
                while galley.size().y > rect.height() && fit > min_font {
                    fit = (fit - 1.0).max(min_font);
                    galley = layout(fit);
                }
                // Mirror the runtime editor's vertical padding for Top/Bottom.
                let vpad = textbox_inner_padding(ctrl)
                    .min((rect.height() * 0.5 - 1.0).max(0.0));
                let text_y = match valign {
                    egui::Align::TOP => rect.top() + vpad,
                    egui::Align::BOTTOM => rect.bottom() - vpad - galley.size().y,
                    _ => rect.center().y - galley.size().y / 2.0,
                };
                let text_pos = egui::pos2(anchor_x(pad), text_y);
                // Clip as a final guard (e.g. at the `min_font` floor in a tiny box).
                let clipped = painter.with_clip_rect(rect);
                paint_styled_galley(&clipped, ctrl, text_pos, galley, txt_color);
            }
        } else if matches!(ctrl.control_type, CT::CheckBox) {
            // Caption sits in the space left after the check glyph (drawn
            // below, outside this `!label.is_empty()` gate), wrapped, clipped,
            // and shrunk exactly like TextBox's single-line box — so a long
            // caption never bleeds past the control's own border instead of
            // overflowing it (developer-reported bug: text used to spill past
            // the frame).
            let pad = 3.0_f32.min(checkbox_text_rect.width() * 0.2);
            let inner_w = (checkbox_text_rect.width() - 2.0 * pad).max(1.0);
            let min_font = 6.0_f32;
            let mut fit = fsize.max(min_font);
            let layout = |size: f32| {
                painter.layout_job(styled_text_job(
                    painter,
                    ctrl,
                    &label,
                    &font_name,
                    size,
                    txt_color,
                    inner_w,
                    egui::Align::LEFT,
                ))
            };
            let mut galley = layout(fit);
            while galley.size().y > checkbox_text_rect.height() && fit > min_font {
                fit = (fit - 1.0).max(min_font);
                galley = layout(fit);
            }
            let text_pos = egui::pos2(
                checkbox_text_rect.left() + pad,
                checkbox_text_rect.center().y - galley.size().y / 2.0,
            );
            let clipped = painter.with_clip_rect(checkbox_text_rect);
            paint_styled_galley(&clipped, ctrl, text_pos, galley, txt_color);
        } else {
            let galley = painter.layout_job(styled_text_job(
                painter,
                ctrl,
                &label,
                &font_name,
                fsize,
                txt_color,
                rect.width(),
                egui::Align::Center,
            ));
            let text_pos = egui::pos2(
                rect.center().x - galley.size().x / 2.0,
                rect.center().y - galley.size().y / 2.0,
            );
            paint_styled_galley(painter, ctrl, text_pos, galley, txt_color);
        }
    }

    // ── MenuBar: render top-level labels horizontally ────────────────────────
    if matches!(ctrl.control_type, CT::MenuBar) {
        if let Some(def) = get_menu_cache(painter.ctx(), &ctrl.id) {
            if !def.menu.is_empty() {
                let fg_base = ctrl
                    .get_prop("ForegroundColor")
                    .map(|v| parse_color(v.as_str()))
                    .unwrap_or(Color32::from_rgb(225, 230, 250));
                let fg = Color32::from_rgba_premultiplied(fg_base.r(), fg_base.g(), fg_base.b(), a);
                let fsize = ctrl_font_size(ctrl);
                let font_name = ctrl
                    .get_prop("FontName")
                    .map(|v| v.as_str())
                    .unwrap_or_default();
                let fid = crate::fonts::font_id(painter.ctx(), &font_name, fsize);
                let mut x = rect.min.x + 10.0;
                for entry in &def.menu {
                    if entry.item_type == crate::menu::MenuItemType::Separator {
                        continue;
                    }
                    let galley = painter.layout_no_wrap(entry.label.clone(), fid.clone(), fg);
                    let w = galley.size().x;
                    painter.galley(
                        Pos2::new(x, rect.center().y - galley.size().y * 0.5),
                        galley,
                        fg,
                    );
                    x += w + 18.0;
                }
            }
        }
    }

    // ── Charts ───────────────────────────────────────────────────────────────
    if matches!(
        ctrl.control_type,
        CT::BarChart
            | CT::LineChart
            | CT::PieChart
            | CT::AreaChart
            | CT::ScatterChart
            | CT::DonutChart
    ) {
        draw_chart_preview(
            painter,
            ctrl,
            rect,
            a,
            alpha_mul,
            glass,
            selected,
            frame_round,
        );
        if selected {
            painter.rect_stroke(
                rect,
                frame_round,
                Stroke::new(2.0, Color32::from_rgba_premultiplied(60, 120, 230, a)),
                egui::StrokeKind::Middle,
            );
        }
    }

    if let Some(shadow) = regular_shadow.as_ref().filter(|shadow| shadow.overlay) {
        draw_regular_drop_shadow(painter, shadow, alpha_mul);
    }
}

/// Compute the destination rect for an image of `native` size inside `rect`,
/// according to a PictureBox/Animator-style `size_mode`.
/// Map a PictureBox/Animator `SizeMode` property value to the canonical mode
/// understood by [`media_dest_rect`]. Shared so the designer, preview, run, and
/// compiled binary all size images identically.
pub fn pic_size_mode(m: &str) -> &'static str {
    match m {
        "Stretch" | "StretchImage" => "Stretch",
        "Zoom" | "Fit" => "Fit",
        "Fill" => "Fill",
        _ => "Center", // Normal / CenterImage / AutoSize
    }
}

pub fn media_dest_rect(rect: egui::Rect, native: Vec2, size_mode: &str) -> egui::Rect {
    if native.x <= 0.0 || native.y <= 0.0 {
        return rect;
    }
    match size_mode {
        "Stretch" => rect,
        "Fill" => {
            // Cover: scale up so the rect is fully covered (may overflow → clipped).
            let s = (rect.width() / native.x).max(rect.height() / native.y);
            egui::Rect::from_center_size(rect.center(), native * s)
        }
        "Center" | "Normal" => {
            // Native size centred, but never larger than the rect.
            let s = (rect.width() / native.x)
                .min(rect.height() / native.y)
                .min(1.0);
            egui::Rect::from_center_size(rect.center(), native * s)
        }
        // "Fit" (default): contain, preserving aspect ratio.
        _ => {
            let s = (rect.width() / native.x).min(rect.height() / native.y);
            egui::Rect::from_center_size(rect.center(), native * s)
        }
    }
}

/// Draw a GroupBox caption as a top overlay. The shared renderer defers captions
/// until after children are drawn, so child clipping can use the whole container
/// interior while the caption still sits above any overlapping child content.
pub fn draw_groupbox_caption(
    painter: &egui::Painter,
    origin: Pos2,
    ctrl: &Control,
    alpha_mul: f32,
) {
    if !matches!(ctrl.control_type, ControlType::GroupBox)
        || ctrl
            .get_prop("HideCaption")
            .map(|v| v.as_bool())
            .unwrap_or(false)
    {
        return;
    }
    let cap = match ctrl.get_prop("Caption") {
        Some(v) if !v.as_str().is_empty() => v.to_string(),
        _ => return,
    };
    if is_legacy_groupbox_generated_caption(&cap) {
        return;
    }
    let label_color = ctrl
        .get_prop("ForegroundColor")
        .map(|v| parse_color(v.as_str()))
        .unwrap_or_else(|| control_colors(&ctrl.control_type, false).2);
    let caption_enabled = ctrl
        .get_prop("CaptionEnabled")
        .map(|v| v.as_bool())
        .unwrap_or(true);
    let a = alpha_mul.clamp(0.0, 1.0) * if caption_enabled { 1.0 } else { 0.45 };
    let text = Color32::from_rgba_premultiplied(
        label_color.r(),
        label_color.g(),
        label_color.b(),
        ((label_color.a() as f32) * a) as u8,
    );
    let font_name = ctrl
        .get_prop("FontName")
        .map(|v| v.as_str())
        .unwrap_or_default();
    let font_id = crate::fonts::font_id(painter.ctx(), &font_name, ctrl_font_size(ctrl));
    let x = origin.x + corner_radius(ctrl).max(0.0) + 10.0;
    let pos = Pos2::new(x, origin.y);

    // BackgroundColor on a GroupBox paints a band behind the caption text.
    if let Some(bg_val) = ctrl.get_prop("BackgroundColor") {
        let bg = parse_color(bg_val.as_str());
        if bg.a() > 0 {
            let galley = painter.layout_no_wrap(cap.clone(), font_id.clone(), text);
            let pad = 4.0_f32;
            let bg_rect = egui::Rect::from_min_size(
                Pos2::new(x - pad, origin.y - galley.size().y * 0.5 - 1.0),
                egui::Vec2::new(galley.size().x + pad * 2.0, galley.size().y + 2.0),
            );
            let bg_color = Color32::from_rgba_premultiplied(
                bg.r(),
                bg.g(),
                bg.b(),
                ((bg.a() as f32) * a) as u8,
            );
            painter.rect_filled(bg_rect, 2.0, bg_color);
        }
    }

    painter.text(pos, egui::Align2::LEFT_CENTER, &cap, font_id, text);
}

fn is_legacy_groupbox_generated_caption(value: &str) -> bool {
    let value = value.trim();
    let lower = value.to_ascii_lowercase();
    let Some(suffix) = lower.strip_prefix("groupbox-") else {
        return false;
    };
    !suffix.is_empty() && suffix.bytes().all(|b| b.is_ascii_digit())
}

/// Draw a TabControl tab strip as top chrome. Renderers may defer this until
/// after children are drawn so child clipping can use the whole rounded interior
/// while tab titles stay above the clipped content.
pub fn draw_tabcontrol_tabs(painter: &egui::Painter, origin: Pos2, ctrl: &Control, alpha_mul: f32) {
    if !matches!(ctrl.control_type, ControlType::TabControl) {
        return;
    }

    let tab_rects = tabcontrol_tab_rects(origin, ctrl);
    if tab_rects.is_empty() {
        return;
    }

    let tabs: Vec<String> = ctrl
        .get_prop("Tabs")
        .map(|v| v.as_str().lines().map(|s| s.to_string()).collect())
        .unwrap_or_default();
    let sel = ctrl
        .get_prop("SelectedTab")
        .map(|v| v.as_i64())
        .unwrap_or(0)
        .max(0) as usize;
    let active_color = ctrl
        .get_prop("ActiveTabColor")
        .map(|v| parse_color(v.as_str()))
        .unwrap_or(Color32::from_rgb(44, 111, 210));
    for (i, (t, tr)) in tabs.iter().zip(tab_rects.iter()).enumerate() {
        let active = i == sel;
        let mut tab = Control::new(format!("{}__tab_{}", ctrl.id, i), ControlType::Button, 0, 0);
        tab.rect =
            crate::model::Rect::new(0, 0, tr.width().round() as i32, tr.height().round() as i32);
        tab.properties = ctrl.properties.clone();
        tab.set_prop("Caption", PropValue::String(t.to_owned()));
        tab.set_prop("CornerRadius", PropValue::Int(4));
        tab.set_prop("BorderStyle", PropValue::String("Single".into()));
        tab.set_prop("BorderWidth", PropValue::Int(1));
        if active {
            tab.set_prop(
                "BackgroundColor",
                PropValue::String(color_to_hex(active_color)),
            );
        }
        draw_control(painter, tr.min, &tab, active, true, alpha_mul, 1.0, None);
    }
}

pub fn tabcontrol_tab_rects(origin: Pos2, ctrl: &Control) -> Vec<egui::Rect> {
    if !matches!(ctrl.control_type, ControlType::TabControl) {
        return Vec::new();
    }
    let tabs: Vec<String> = ctrl
        .get_prop("Tabs")
        .map(|v| v.as_str().lines().map(|s| s.to_string()).collect())
        .unwrap_or_default();
    let rect = egui::Rect::from_min_size(
        origin,
        Vec2::new(ctrl.rect.w.max(0) as f32, ctrl.rect.h.max(0) as f32),
    );
    let strip_h = ctrl.tab_strip_height().max(0) as f32;
    let strip_w = ctrl.tab_strip_extent().max(0) as f32;
    let gap = ctrl.tab_padding().max(0) as f32;
    let pos = ctrl.tab_position();
    let mut out = Vec::new();
    match pos.as_str() {
        "bottom" => {
            let mut x = rect.min.x;
            let y = rect.max.y - strip_h;
            for tab in tabs {
                let w = tab_width(&tab);
                if x + w > rect.max.x {
                    break;
                }
                out.push(egui::Rect::from_min_size(
                    Pos2::new(x, y),
                    Vec2::new(w, strip_h),
                ));
                x += w + gap;
            }
        }
        "left" => {
            let mut y = rect.min.y;
            for _ in tabs {
                if y + strip_h > rect.max.y {
                    break;
                }
                out.push(egui::Rect::from_min_size(
                    Pos2::new(rect.min.x, y),
                    Vec2::new(strip_w - gap, strip_h),
                ));
                y += strip_h + gap;
            }
        }
        "right" => {
            let mut y = rect.min.y;
            let x = rect.max.x - strip_w;
            for _ in tabs {
                if y + strip_h > rect.max.y {
                    break;
                }
                out.push(egui::Rect::from_min_size(
                    Pos2::new(x, y),
                    Vec2::new(strip_w - gap, strip_h),
                ));
                y += strip_h + gap;
            }
        }
        _ => {
            let mut x = rect.min.x;
            for tab in tabs {
                let w = tab_width(&tab);
                if x + w > rect.max.x {
                    break;
                }
                out.push(egui::Rect::from_min_size(
                    Pos2::new(x, rect.min.y),
                    Vec2::new(w, strip_h),
                ));
                x += w + gap;
            }
        }
    }
    out
}

fn tab_width(tab: &str) -> f32 {
    (tab.chars().count() as f32 * 7.0 + 18.0).clamp(40.0, 160.0)
}

fn color_to_hex(c: Color32) -> String {
    format!("#{:02X}{:02X}{:02X}{:02X}", c.r(), c.g(), c.b(), c.a())
}

pub fn tabcontrol_page_rect(rect: egui::Rect, ctrl: &Control) -> egui::Rect {
    if !matches!(ctrl.control_type, ControlType::TabControl) {
        return rect;
    }
    let inset = ctrl.tab_strip_extent().max(0) as f32;
    match ctrl.tab_position().as_str() {
        "bottom" => egui::Rect::from_min_max(
            rect.min,
            Pos2::new(rect.max.x, (rect.max.y - inset).max(rect.min.y)),
        ),
        "left" => egui::Rect::from_min_max(
            Pos2::new((rect.min.x + inset).min(rect.max.x), rect.min.y),
            rect.max,
        ),
        "right" => egui::Rect::from_min_max(
            rect.min,
            Pos2::new((rect.max.x - inset).max(rect.min.x), rect.max.y),
        ),
        _ => egui::Rect::from_min_max(
            Pos2::new(rect.min.x, (rect.min.y + inset).min(rect.max.y)),
            rect.max,
        ),
    }
}

/// Load an image file into an egui texture. Caching of the returned handle is
/// the caller's responsibility (see [`picturebox_texture`]). Shared so every
/// surface (designer, preview, run, compiled) decodes images the same way.
pub fn load_image_texture(ctx: &egui::Context, path: &str) -> Option<egui::TextureHandle> {
    let bytes = std::fs::read(path).ok()?;
    let ci = decode_image_bytes(path, &bytes)?;
    // Repeat wrap (identical to clamp for in-bounds [0,1] UVs) so a Tiled backdrop
    // can also tile inside the corner-notch mask (spec 017).
    Some(ctx.load_texture(path, ci, egui::TextureOptions::LINEAR_REPEAT))
}

fn decode_image_bytes(path: &str, bytes: &[u8]) -> Option<egui::ColorImage> {
    if is_svg_path(path) || is_svg_bytes(bytes) {
        return decode_svg_bytes(bytes);
    }

    let img = image::load_from_memory(bytes).ok()?.into_rgba8();
    let (w, h) = (img.width() as usize, img.height() as usize);
    let pixels: Vec<egui::Color32> = img
        .pixels()
        .map(|p| egui::Color32::from_rgba_unmultiplied(p[0], p[1], p[2], p[3]))
        .collect();
    Some(egui::ColorImage {
        size: [w, h],
        source_size: egui::vec2(w as f32, h as f32),
        pixels,
    })
}

fn is_svg_path(path: &str) -> bool {
    path.rsplit_once('.')
        .map(|(_, ext)| ext.eq_ignore_ascii_case("svg"))
        .unwrap_or(false)
}

fn is_svg_bytes(bytes: &[u8]) -> bool {
    std::str::from_utf8(bytes)
        .map(|s| s.trim_start().starts_with("<svg"))
        .unwrap_or(false)
}

fn svg_fontdb() -> Arc<resvg::usvg::fontdb::Database> {
    static DB: OnceLock<Arc<resvg::usvg::fontdb::Database>> = OnceLock::new();
    DB.get_or_init(|| {
        let mut db = resvg::usvg::fontdb::Database::new();
        db.load_system_fonts();
        Arc::new(db)
    })
    .clone()
}

fn strip_svg_icc_color_fallbacks(svg: &str) -> String {
    let marker = " icc-color(";
    if !svg.contains(marker) {
        return svg.to_owned();
    }
    let mut out = String::with_capacity(svg.len());
    let mut rest = svg;
    while let Some(pos) = rest.find(marker) {
        out.push_str(&rest[..pos]);
        let after_marker = &rest[pos + marker.len()..];
        if let Some(end) = after_marker.find(')') {
            rest = &after_marker[end + 1..];
        } else {
            rest = after_marker;
            break;
        }
    }
    out.push_str(rest);
    out
}

fn decode_svg_bytes(bytes: &[u8]) -> Option<egui::ColorImage> {
    let svg = std::str::from_utf8(bytes).ok()?;
    let svg = strip_svg_icc_color_fallbacks(svg);
    let opt = resvg::usvg::Options {
        fontdb: svg_fontdb(),
        ..Default::default()
    };
    let tree = resvg::usvg::Tree::from_str(&svg, &opt).ok()?;
    let size = tree.size().to_int_size();
    let width = size.width().max(1);
    let height = size.height().max(1);
    let mut pixmap = resvg::tiny_skia::Pixmap::new(width, height)?;
    resvg::render(
        &tree,
        resvg::tiny_skia::Transform::identity(),
        &mut pixmap.as_mut(),
    );
    let pixels = pixmap
        .pixels()
        .iter()
        .map(|p| egui::Color32::from_rgba_premultiplied(p.red(), p.green(), p.blue(), p.alpha()))
        .collect();
    Some(egui::ColorImage {
        size: [width as usize, height as usize],
        source_size: egui::vec2(width as f32, height as f32),
        pixels,
    })
}

fn svg_native_size_from_bytes(bytes: &[u8]) -> Option<Vec2> {
    let svg = std::str::from_utf8(bytes).ok()?;
    let svg = strip_svg_icc_color_fallbacks(svg);
    let opt = resvg::usvg::Options {
        fontdb: svg_fontdb(),
        ..Default::default()
    };
    let tree = resvg::usvg::Tree::from_str(&svg, &opt).ok()?;
    let size = tree.size();
    Some(Vec2::new(size.width().max(1.0), size.height().max(1.0)))
}

fn decode_svg_bytes_at_size(bytes: &[u8], width: u32, height: u32) -> Option<egui::ColorImage> {
    let svg = std::str::from_utf8(bytes).ok()?;
    let svg = strip_svg_icc_color_fallbacks(svg);
    let opt = resvg::usvg::Options {
        fontdb: svg_fontdb(),
        ..Default::default()
    };
    let tree = resvg::usvg::Tree::from_str(&svg, &opt).ok()?;
    let native = tree.size();
    let width = width.max(1);
    let height = height.max(1);
    let sx = width as f32 / native.width().max(1.0);
    let sy = height as f32 / native.height().max(1.0);
    let mut pixmap = resvg::tiny_skia::Pixmap::new(width, height)?;
    resvg::render(
        &tree,
        resvg::tiny_skia::Transform::from_scale(sx, sy),
        &mut pixmap.as_mut(),
    );
    let pixels = pixmap
        .pixels()
        .iter()
        .map(|p| egui::Color32::from_rgba_premultiplied(p.red(), p.green(), p.blue(), p.alpha()))
        .collect();
    Some(egui::ColorImage {
        size: [width as usize, height as usize],
        source_size: egui::vec2(width as f32, height as f32),
        pixels,
    })
}

/// Load (and cache in egui memory) a PictureBox image texture, so it isn't
/// re-read from disk and re-uploaded every frame.
pub fn picturebox_texture(ctx: &egui::Context, path: &str) -> Option<egui::TextureHandle> {
    if path.trim().is_empty() {
        return None;
    }
    let id = egui::Id::new(("pb_img", path));
    if let Some(h) = ctx.memory(|m| m.data.get_temp::<egui::TextureHandle>(id)) {
        return Some(h);
    }
    let h = load_image_texture(ctx, path)?;
    ctx.memory_mut(|m| m.data.insert_temp(id, h.clone()));
    Some(h)
}

fn picturebox_svg_native_size(path: &str) -> Option<Vec2> {
    if !is_svg_path(path) {
        return None;
    }
    static CACHE: OnceLock<Mutex<HashMap<String, Option<Vec2>>>> = OnceLock::new();
    let cache = CACHE.get_or_init(|| Mutex::new(HashMap::new()));
    if let Some(size) = cache.lock().unwrap().get(path) {
        return *size;
    }
    let bytes = std::fs::read(path).ok()?;
    let size = svg_native_size_from_bytes(&bytes);
    cache.lock().unwrap().insert(path.to_owned(), size);
    size
}

fn picturebox_svg_texture(
    ctx: &egui::Context,
    path: &str,
    logical_size: Vec2,
) -> Option<egui::TextureHandle> {
    if !is_svg_path(path) || logical_size.x <= 0.0 || logical_size.y <= 0.0 {
        return None;
    }
    let ppp = ctx.pixels_per_point().max(1.0);
    let width = (logical_size.x * ppp).ceil().max(1.0) as u32;
    let height = (logical_size.y * ppp).ceil().max(1.0) as u32;
    let id = egui::Id::new(("pb_svg_img", path, width, height));
    if let Some(h) = ctx.memory(|m| m.data.get_temp::<egui::TextureHandle>(id)) {
        return Some(h);
    }
    let bytes = std::fs::read(path).ok()?;
    let image = decode_svg_bytes_at_size(&bytes, width, height)?;
    let handle = ctx.load_texture(
        format!("{path}@{width}x{height}"),
        image,
        egui::TextureOptions::LINEAR,
    );
    ctx.memory_mut(|m| m.data.insert_temp(id, handle.clone()));
    Some(handle)
}

/// Load an SVG texture at the requested logical size, rasterizing from vector
/// data for that size instead of magnifying a previously rasterized texture.
pub fn load_svg_texture_at_size(
    ctx: &egui::Context,
    path: &str,
    logical_size: Vec2,
) -> Option<egui::TextureHandle> {
    picturebox_svg_texture(ctx, path, logical_size)
}

fn picturebox_rounded_image_rect_uv(
    control_rect: egui::Rect,
    image_dest: egui::Rect,
    corner: f32,
) -> Option<(egui::Rect, egui::Rect, f32)> {
    let visible = image_dest.intersect(control_rect);
    if visible.width() <= 0.5 || visible.height() <= 0.5 {
        return None;
    }
    let dw = image_dest.width().max(1.0);
    let dh = image_dest.height().max(1.0);
    let uv = egui::Rect::from_min_max(
        egui::pos2(
            (visible.min.x - image_dest.min.x) / dw,
            (visible.min.y - image_dest.min.y) / dh,
        ),
        egui::pos2(
            (visible.max.x - image_dest.min.x) / dw,
            (visible.max.y - image_dest.min.y) / dh,
        ),
    );
    let clamped_corner = corner
        .max(0.0)
        .min(visible.width().min(visible.height()) * 0.5);
    Some((visible, uv, clamped_corner))
}

fn paint_picturebox_texture(
    painter: &egui::Painter,
    rect: egui::Rect,
    dest: egui::Rect,
    texture_id: egui::TextureId,
    alpha: u8,
    corner: f32,
) {
    if corner > 0.0 {
        let Some((visible, uv, clamped_corner)) =
            picturebox_rounded_image_rect_uv(rect, dest, corner)
        else {
            return;
        };
        painter.with_clip_rect(rect).add(egui::Shape::Rect(
            egui::epaint::RectShape::new(
                visible,
                egui::CornerRadius::same(cr8(clamped_corner)),
                Color32::from_white_alpha(alpha),
                Stroke::NONE,
                egui::StrokeKind::Middle,
            )
            .with_texture(texture_id, uv),
        ));
    } else {
        let uv = egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0));
        painter
            .with_clip_rect(rect)
            .image(texture_id, dest, uv, Color32::from_white_alpha(alpha));
    }
}

fn draw_picturebox_image(
    painter: &egui::Painter,
    rect: egui::Rect,
    image_path: &str,
    size_mode: &str,
    alpha_mul: f32,
    corner: f32,
) -> bool {
    let a = (alpha_mul.clamp(0.0, 1.0) * 255.0) as u8;
    if let Some(native) = picturebox_svg_native_size(image_path) {
        let dest = media_dest_rect(rect, native, pic_size_mode(size_mode));
        if let Some(tex) = picturebox_svg_texture(painter.ctx(), image_path, dest.size()) {
            paint_picturebox_texture(painter, rect, dest, tex.id(), a, corner);
            return true;
        }
        return false;
    }
    if let Some(tex) = picturebox_texture(painter.ctx(), image_path) {
        let dest = media_dest_rect(rect, tex.size_vec2(), pic_size_mode(size_mode));
        paint_picturebox_texture(painter, rect, dest, tex.id(), a, corner);
        return true;
    }
    false
}

/// Render a PictureBox into `rect`: an optional frame (card + border) plus the
/// image, honouring `SizeMode`, opacity (`alpha_mul`), and `ShowFrame`. When the
/// frame is hidden, transparent PNG areas reveal whatever is behind the control.
pub fn draw_picturebox(
    painter: &egui::Painter,
    rect: egui::Rect,
    image_path: &str,
    size_mode: &str,
    show_frame: bool,
    alpha_mul: f32,
    corner: f32,
) {
    if show_frame {
        draw_glass_auto(
            painter,
            rect,
            Color32::from_rgb(20, 30, 60),
            corner,
            false,
            alpha_mul * 0.7,
        );
    }
    if !draw_picturebox_image(painter, rect, image_path, size_mode, alpha_mul, corner) && show_frame
    {
        painter.text(
            rect.center(),
            egui::Align2::CENTER_CENTER,
            "🖼",
            egui::FontId::proportional(32.0),
            Color32::from_rgba_premultiplied(160, 160, 200, (160.0 * alpha_mul) as u8),
        );
    }
}

/// Parse `#RRGGBB` / `#RRGGBBAA` (or without `#`) into a Color32, returning
/// `None` when the string is too short. Distinct from [`parse_color`], which
/// always yields a colour; used where a missing value should fall back.
pub fn parse_hex(s: &str) -> Option<Color32> {
    let h = s.trim().trim_start_matches('#');
    if h.len() >= 6 {
        let r = u8::from_str_radix(&h[0..2], 16).ok()?;
        let g = u8::from_str_radix(&h[2..4], 16).ok()?;
        let b = u8::from_str_radix(&h[4..6], 16).ok()?;
        let a = if h.len() >= 8 {
            u8::from_str_radix(&h[6..8], 16).unwrap_or(255)
        } else {
            255
        };
        Some(Color32::from_rgba_unmultiplied(r, g, b, a))
    } else {
        None
    }
}

/// Short month names for DataGrid date cells and the DateTimePicker field.
pub const MONTH_ABBR: [&str; 12] = [
    "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
];

/// Format a DataGrid cell value by its declared column type.
/// Returns `(display_text, right_aligned)`.
pub fn format_cell(raw: &str, ty: &str) -> (String, bool) {
    match ty {
        "number" | "num" | "int" | "integer" | "float" | "decimal" => {
            match raw.trim().parse::<f64>() {
                Ok(n) if n.fract() == 0.0 => (format!("{}", n as i64), true),
                Ok(n) => (format!("{n}"), true),
                Err(_) => (raw.to_owned(), true),
            }
        }
        "datetime" | "date" => match parse_ymd(raw.trim()) {
            Some((y, m, d)) => (
                format!(
                    "{:02} {} {}",
                    d,
                    MONTH_ABBR[(m.clamp(1, 12) - 1) as usize],
                    y
                ),
                false,
            ),
            None => (raw.to_owned(), false),
        },
        _ => (raw.to_owned(), false),
    }
}

/// Format a DataGrid cell using a COBOL display mask when one is available.
/// The mask support is intentionally conservative and covers the common grid
/// binding masks: `9(n)`, `S9(n)V99`, `PIC ...`, and text `X(n)`.
pub fn format_cell_with_cobol_mask(raw: &str, ty: &str, cobol_mask: &str) -> (String, bool) {
    let mask = normalize_cobol_mask(cobol_mask);
    if mask.is_empty() || mask == "-" || mask == "—" {
        return format_cell(raw, ty);
    }
    if mask.contains('X') || mask.contains('A') {
        return (raw.to_owned(), false);
    }
    // COBOL *edited* pictures — zero-suppression (`Z`), check protection (`*`),
    // digit-group (`,`) and displayed decimal (`.`) insertion, e.g.
    // `ZZZ,ZZZ,ZZ9.99` → `3,000.00`. Handled before the plain-numeric path so a
    // bound `S9(9)V99` value renders through the column's display mask.
    if is_edited_picture(&mask) {
        if let Some(edited) = format_edited_numeric(raw, &mask) {
            return (edited, true);
        }
    }
    if !mask.contains('9') {
        return format_cell(raw, ty);
    }
    let Some((integer_digits, decimal_digits, signed)) = parse_numeric_cobol_mask(&mask) else {
        return format_cell(raw, ty);
    };
    let Ok(value) = raw.trim().parse::<f64>() else {
        return (raw.to_owned(), true);
    };
    let negative = value.is_sign_negative();
    let abs_value = value.abs();
    let should_zero_pad = !signed;
    let formatted = if decimal_digits == 0 {
        let mut integer = format!("{:.0}", abs_value);
        if should_zero_pad && integer_digits > integer.len() {
            integer = format!("{}{}", "0".repeat(integer_digits - integer.len()), integer);
        }
        integer
    } else {
        let fixed = format!("{abs_value:.decimal_digits$}");
        let mut parts = fixed.splitn(2, '.');
        let mut integer = parts.next().unwrap_or_default().to_owned();
        let decimal = parts.next().unwrap_or_default();
        if should_zero_pad && integer_digits > integer.len() {
            integer = format!("{}{}", "0".repeat(integer_digits - integer.len()), integer);
        }
        format!("{integer}.{decimal}")
    };
    let sign = if negative {
        "-"
    } else if signed && mask.starts_with('+') {
        "+"
    } else {
        ""
    };
    (format!("{sign}{formatted}"), true)
}

/// A normalized picture is an *edited* numeric picture when it carries any
/// insertion/suppression symbol (`Z` `*` `,` `$` `B`) or a displayed decimal
/// point (`.`). Plain `9`/`S`/`V` pictures keep the simpler legacy formatting.
fn is_edited_picture(mask: &str) -> bool {
    mask.contains('Z')
        || mask.contains('*')
        || mask.contains(',')
        || mask.contains('$')
        || mask.contains('B')
        || mask.contains('.')
}

/// Expand `9(3)` / `Z(4)` / `*(2)` repetition groups into individual symbols so
/// the picture can be walked one position at a time.
fn expand_picture(mask: &str) -> String {
    let chars: Vec<char> = mask.chars().collect();
    let mut out = String::new();
    let mut i = 0;
    while i < chars.len() {
        let ch = chars[i];
        if chars.get(i + 1) == Some(&'(') {
            if let Some(end) = chars[i + 2..].iter().position(|c| *c == ')') {
                let end = i + 2 + end;
                let count: usize = chars[i + 2..end]
                    .iter()
                    .collect::<String>()
                    .parse()
                    .unwrap_or(1);
                for _ in 0..count {
                    out.push(ch);
                }
                i = end + 1;
                continue;
            }
        }
        out.push(ch);
        i += 1;
    }
    out
}

fn count_digit_positions(pic: &str) -> usize {
    pic.chars().filter(|c| matches!(c, '9' | 'Z' | '*')).count()
}

/// Format `raw` through a COBOL edited numeric picture (`ZZZ,ZZZ,ZZ9.99`,
/// `**,**9.99`, `9(6)`, …). Zero-suppresses leading zeros, honours grouping and
/// the displayed decimal point, and prefixes a sign for negatives / `+` floats.
/// Returns `None` when `raw` isn't numeric (caller falls back).
fn format_edited_numeric(raw: &str, mask: &str) -> Option<String> {
    let pic = expand_picture(mask);
    let value: f64 = raw.trim().parse().ok()?;
    let negative = value.is_sign_negative() && value != 0.0;
    let plus = pic.contains('+');
    let abs = value.abs();

    // Split integer / fraction at the displayed decimal point or implied `V`.
    let (int_pic, dec_pic) = if let Some(idx) = pic.find('.') {
        (pic[..idx].to_string(), pic[idx + 1..].to_string())
    } else if let Some(idx) = pic.find('V') {
        (pic[..idx].to_string(), pic[idx + 1..].to_string())
    } else {
        (pic.clone(), String::new())
    };
    let dec_digits = count_digit_positions(&dec_pic);
    let int_positions = count_digit_positions(&int_pic);

    // Digit strings, scaled to the fraction width and padded to the picture.
    let scaled = format!("{abs:.dec_digits$}");
    let (int_str, dec_str) = scaled.split_once('.').unwrap_or((scaled.as_str(), ""));
    let mut int_digits = int_str.to_string();
    while int_digits.len() < int_positions {
        int_digits.insert(0, '0');
    }
    if int_digits.len() > int_positions {
        int_digits = int_digits[int_digits.len() - int_positions..].to_string();
    }
    let int_digits: Vec<char> = int_digits.chars().collect();

    let fill = if pic.contains('*') { '*' } else { ' ' };
    let mut out = String::new();
    let mut di = 0usize;
    let mut suppressing = true;
    for ch in int_pic.chars() {
        match ch {
            '9' => {
                out.push(int_digits[di]);
                di += 1;
                suppressing = false;
            }
            'Z' | '*' => {
                let d = int_digits[di];
                di += 1;
                if suppressing && d == '0' {
                    out.push(fill);
                } else {
                    out.push(d);
                    suppressing = false;
                }
            }
            ',' => out.push(if suppressing { fill } else { ',' }),
            'B' => out.push(' '),
            'S' | 'V' | '+' | '-' => {}
            other => out.push(other),
        }
    }

    if dec_digits > 0 {
        out.push('.');
        let mut dec_chars = dec_str.to_string();
        while dec_chars.len() < dec_digits {
            dec_chars.push('0');
        }
        out.push_str(&dec_chars[..dec_digits]);
    }

    // Leading fill is alignment padding for a fixed field; a grid cell reads
    // cleaner trimmed. Asterisk (check-protection) fill is meaningful — keep it.
    if fill == ' ' {
        out = out.trim_start().to_string();
    }
    if negative {
        out = format!("-{out}");
    } else if plus {
        out = format!("+{out}");
    }
    Some(out)
}

fn normalize_cobol_mask(mask: &str) -> String {
    let mut normalized = mask.trim().to_ascii_uppercase();
    if let Some(rest) = normalized.strip_prefix("PIC ") {
        normalized = rest.trim().to_owned();
    } else if let Some(rest) = normalized.strip_prefix("PICTURE ") {
        normalized = rest.trim().to_owned();
    }
    normalized.retain(|ch| !ch.is_whitespace());
    normalized
}

fn parse_numeric_cobol_mask(mask: &str) -> Option<(usize, usize, bool)> {
    let signed = mask.starts_with('S') || mask.starts_with('+') || mask.starts_with('-');
    let unsigned = mask.trim_start_matches(['S', '+', '-']);
    let (integer_mask, decimal_mask) = unsigned
        .split_once('V')
        .or_else(|| unsigned.split_once('.'))
        .map(|(left, right)| (left, right))
        .unwrap_or((unsigned, ""));
    let integer_digits = count_cobol_digit_positions(integer_mask)?;
    let decimal_digits = if decimal_mask.is_empty() {
        0
    } else {
        count_cobol_digit_positions(decimal_mask)?
    };
    Some((integer_digits, decimal_digits, signed))
}

fn count_cobol_digit_positions(mask: &str) -> Option<usize> {
    let chars: Vec<char> = mask.chars().collect();
    let mut index = 0;
    let mut count = 0;
    while index < chars.len() {
        match chars[index] {
            '9' | 'Z' | '*' => {
                if chars.get(index + 1) == Some(&'(') {
                    let mut end = index + 2;
                    while end < chars.len() && chars[end] != ')' {
                        end += 1;
                    }
                    if end >= chars.len() {
                        return None;
                    }
                    let repeat = chars[index + 2..end]
                        .iter()
                        .collect::<String>()
                        .parse::<usize>()
                        .ok()?;
                    count += repeat;
                    index = end + 1;
                } else {
                    count += 1;
                    index += 1;
                }
            }
            ',' | '.' | '-' | '+' | '$' | '/' => index += 1,
            _ => return None,
        }
    }
    Some(count)
}

// ── DateTimePicker calendar support ────────────────────────────────────────────
pub const CAL_CELL: f32 = 28.0;
pub const CAL_W: f32 = CAL_CELL * 7.0;
pub const CAL_NAV_H: f32 = 24.0;
pub const CAL_WK_H: f32 = 20.0;
pub const CAL_GRID_Y: f32 = CAL_NAV_H + CAL_WK_H; // area-top → first day row
pub const MONTHS: [&str; 12] = [
    "January",
    "February",
    "March",
    "April",
    "May",
    "June",
    "July",
    "August",
    "September",
    "October",
    "November",
    "December",
];

/// Open/viewed-month state for a DateTimePicker calendar popup, stashed in egui
/// temp memory keyed by the control id.
#[derive(Clone)]
pub struct CalState {
    pub open: bool,
    pub year: i32,
    pub month: u32, // 1-12
}
impl Default for CalState {
    fn default() -> Self {
        Self {
            open: false,
            year: 2026,
            month: 6,
        }
    }
}

fn is_leap(y: i32) -> bool {
    (y % 4 == 0 && y % 100 != 0) || y % 400 == 0
}

pub fn days_in_month(y: i32, m: u32) -> u32 {
    match m {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 => {
            if is_leap(y) {
                29
            } else {
                28
            }
        }
        _ => 30,
    }
}

/// Day of week for a date, 0 = Sunday (Sakamoto's algorithm).
pub fn day_of_week(y: i32, m: u32, d: u32) -> u32 {
    let t = [0, 3, 2, 5, 0, 3, 5, 1, 4, 6, 2, 4];
    let yy = if m < 3 { y - 1 } else { y };
    let v = (yy + yy / 4 - yy / 100 + yy / 400 + t[(m.clamp(1, 12) - 1) as usize] + d as i32) % 7;
    ((v + 7) % 7) as u32
}

pub fn parse_ymd(s: &str) -> Option<(i32, u32, u32)> {
    let p: Vec<&str> = s.split('-').collect();
    if p.len() == 3 {
        Some((p[0].parse().ok()?, p[1].parse().ok()?, p[2].parse().ok()?))
    } else {
        None
    }
}

/// Render an Animator control: plays its animated/still image (GIF/WebP/APNG/…)
/// at the current moment, or a placeholder when no source is set / decode fails.
#[allow(clippy::too_many_arguments)]
// ── ComboBox glass widgets (shared by designer preview, run, compiled) ─────────
// Two-pass combo: `glass_combo_header` draws the closed bar; `glass_combo_popup`
// draws the open dropdown after all controls so it floats on top. The caller
// stores open/closed state keyed by control id (spec 017 consolidation).

/// Result of a `glass_combo_popup` interaction.
#[derive(Debug)]
pub enum GlassComboAction {
    /// User selected this item.
    Select(String),
    /// User clicked outside the popup — close without changing value.
    Close,
}

/// Draw the ComboBox header bar (always visible). Returns `true` if clicked.
pub fn glass_combo_header(
    painter: &egui::Painter,
    ui: &mut egui::Ui,
    rect: egui::Rect,
    control_id: egui::Id,
    selected: &str,
    is_open: bool,
    enabled: bool,
    alpha: f32,
) -> bool {
    use egui::{Align2, FontId, Pos2};
    draw_glass_auto(
        painter,
        rect,
        Color32::from_rgb(25, 38, 80),
        6.0,
        false,
        alpha,
    );
    painter.rect_stroke(
        rect,
        6.0,
        Stroke::new(1.0, Color32::from_rgba_premultiplied(100, 140, 230, 150)),
        egui::StrokeKind::Middle,
    );
    painter.text(
        Pos2::new(rect.min.x + 8.0, rect.center().y),
        Align2::LEFT_CENTER,
        selected,
        FontId::proportional(12.0),
        Color32::from_rgb(220, 228, 255),
    );
    painter.text(
        Pos2::new(rect.max.x - 13.0, rect.center().y),
        egui::Align2::CENTER_CENTER,
        if is_open { "▲" } else { "▼" },
        FontId::proportional(9.0),
        Color32::from_rgba_premultiplied(160, 190, 255, 200),
    );
    enabled
        && ui
            .interact(rect, control_id, egui::Sense::click())
            .clicked()
}

/// Draw the ComboBox dropdown popup (call after all controls). Returns the user
/// action, if any.
pub fn glass_combo_popup(
    ui: &mut egui::Ui,
    ctrl_id_str: &str,
    header_rect: egui::Rect,
    items: &[String],
    selected_val: &str,
) -> Option<GlassComboAction> {
    use egui::{Align2, FontId, Pos2, Vec2};

    let item_h = 22.0_f32;
    let popup_h = (items.len() as f32 * item_h).min(180.0);
    let popup_rect = egui::Rect::from_min_size(
        Pos2::new(header_rect.min.x, header_rect.max.y + 1.0),
        Vec2::new(header_rect.width(), popup_h),
    );

    let pointer_pos = ui.input(|i| i.pointer.hover_pos());
    let any_click = ui.input(|i| i.pointer.any_click());
    if any_click {
        let inside = header_rect.contains(pointer_pos.unwrap_or(Pos2::ZERO))
            || popup_rect.contains(pointer_pos.unwrap_or(Pos2::ZERO));
        if !inside {
            return Some(GlassComboAction::Close);
        }
    }

    let pp = ui.painter_at(popup_rect);
    pp.rect_filled(popup_rect, 6.0, Color32::from_rgb(22, 30, 58));
    draw_glass_auto(
        &pp,
        popup_rect,
        Color32::from_rgb(30, 42, 80),
        6.0,
        false,
        0.35,
    );
    pp.rect_stroke(
        popup_rect,
        6.0,
        Stroke::new(1.0, Color32::from_rgba_premultiplied(90, 130, 220, 180)),
        egui::StrokeKind::Middle,
    );

    let mut action = None;
    for (i, item) in items.iter().enumerate() {
        let item_y = popup_rect.min.y + i as f32 * item_h;
        if item_y + item_h > popup_rect.max.y {
            break;
        }
        let item_rect = egui::Rect::from_min_size(
            Pos2::new(popup_rect.min.x, item_y),
            Vec2::new(popup_rect.width(), item_h),
        );
        let iid = egui::Id::new(("glass_combo_item", ctrl_id_str, i));
        let is_sel = item == selected_val;
        let hovered = pointer_pos.map(|p| item_rect.contains(p)).unwrap_or(false);
        if is_sel {
            pp.rect_filled(
                item_rect,
                4.0,
                Color32::from_rgba_premultiplied(60, 100, 200, 120),
            );
        } else if hovered {
            pp.rect_filled(
                item_rect,
                4.0,
                Color32::from_rgba_premultiplied(50, 70, 150, 80),
            );
        }
        pp.text(
            Pos2::new(item_rect.min.x + 10.0, item_rect.center().y),
            Align2::LEFT_CENTER,
            item,
            FontId::proportional(12.0),
            if is_sel {
                Color32::from_rgb(200, 220, 255)
            } else {
                Color32::from_rgb(210, 218, 245)
            },
        );
        if ui.interact(item_rect, iid, egui::Sense::click()).clicked() {
            action = Some(GlassComboAction::Select(item.clone()));
        }
    }
    action
}

#[allow(clippy::too_many_arguments)]
pub fn draw_animator(
    painter: &egui::Painter,
    rect: egui::Rect,
    ctrl: &Control,
    key: &str,
    source: &str,
    auto_play: bool,
    looping: bool,
    size_mode: &str,
    alpha_mul: f32,
    selected: bool,
) {
    let a = (alpha_mul.clamp(0.0, 1.0) * 255.0) as u8;
    // Round the film/placeholder corners that land on a container's rounded border
    // (spec 017), so the animation is cut by the parent shape like the PictureBox.
    let round = control_border_rounding(ctrl, rect, 6.0);

    let played = if source.is_empty() {
        None
    } else {
        let path = source.to_owned();
        cobolt_media::play(
            painter.ctx(),
            key,
            move || std::fs::read(&path).ok(),
            auto_play,
            looping,
        )
    };

    match played {
        Some((tex, native)) => {
            // Same clipped/rounded path as the PictureBox image (own = 0 — the film
            // is unrounded unless a container border clips it).
            draw_media_image(painter, rect, tex, native, size_mode, a, ctrl, 0.0);
        }
        None => {
            // Placeholder: a dark "film" panel with a play glyph.
            painter.rect_filled(rect, round, Color32::from_rgba_premultiplied(18, 24, 48, a));
            painter.rect_stroke(
                rect,
                round,
                Stroke::new(1.0, Color32::from_rgba_premultiplied(120, 150, 230, a)),
                egui::StrokeKind::Middle,
            );
            let label = if source.is_empty() {
                "▶ Animator"
            } else {
                "▶ (cannot load)"
            };
            painter.text(
                rect.center(),
                egui::Align2::CENTER_CENTER,
                label,
                egui::FontId::proportional(13.0),
                Color32::from_rgba_premultiplied(190, 205, 255, a),
            );
        }
    }

    if selected {
        painter.rect_stroke(
            rect,
            round,
            Stroke::new(2.0, Color32::from_rgba_premultiplied(60, 120, 230, a)),
            egui::StrokeKind::Middle,
        );
    }
}

// ── Monochrome chart palette (spec 013) ────────────────────────────────────

/// RGB (0–255) → HSL with `h` in [0,360), `s`/`l` in [0,1].
fn rgb_to_hsl(c: Color32) -> (f32, f32, f32) {
    let r = c.r() as f32 / 255.0;
    let g = c.g() as f32 / 255.0;
    let b = c.b() as f32 / 255.0;
    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let l = (max + min) / 2.0;
    let d = max - min;
    if d.abs() < 1e-6 {
        return (0.0, 0.0, l); // achromatic
    }
    let s = if l > 0.5 {
        d / (2.0 - max - min)
    } else {
        d / (max + min)
    };
    let h = if max == r {
        ((g - b) / d + if g < b { 6.0 } else { 0.0 }) * 60.0
    } else if max == g {
        ((b - r) / d + 2.0) * 60.0
    } else {
        ((r - g) / d + 4.0) * 60.0
    };
    (h, s, l)
}

/// HSL (`h` in [0,360), `s`/`l` in [0,1]) → opaque `Color32`.
fn hsl_to_rgb(h: f32, s: f32, l: f32) -> Color32 {
    let h = h.rem_euclid(360.0) / 360.0;
    let s = s.clamp(0.0, 1.0);
    let l = l.clamp(0.0, 1.0);
    if s.abs() < 1e-6 {
        let v = (l * 255.0).round() as u8;
        return Color32::from_rgb(v, v, v);
    }
    let q = if l < 0.5 {
        l * (1.0 + s)
    } else {
        l + s - l * s
    };
    let p = 2.0 * l - q;
    let hue = |mut t: f32| -> f32 {
        if t < 0.0 {
            t += 1.0;
        }
        if t > 1.0 {
            t -= 1.0;
        }
        if t < 1.0 / 6.0 {
            p + (q - p) * 6.0 * t
        } else if t < 1.0 / 2.0 {
            q
        } else if t < 2.0 / 3.0 {
            p + (q - p) * (2.0 / 3.0 - t) * 6.0
        } else {
            p
        }
    };
    let r = (hue(h + 1.0 / 3.0) * 255.0).round() as u8;
    let g = (hue(h) * 255.0).round() as u8;
    let b = (hue(h - 1.0 / 3.0) * 255.0).round() as u8;
    Color32::from_rgb(r, g, b)
}

/// `count` distinguishable tones of `base`, same hue family, lightness spread
/// across ~[0.30, 0.78] so none is pure black/white and adjacent tones differ.
pub fn monochrome_palette(base: Color32, count: usize) -> Vec<Color32> {
    let (h, s, _) = rgb_to_hsl(base);
    let s = s.max(0.40);
    let n = count.max(1);
    (0..n)
        .map(|i| {
            let t = if n == 1 {
                0.5
            } else {
                i as f32 / (n as f32 - 1.0)
            };
            let l = 0.30 + t * 0.48;
            let sat = (s * (0.80 + 0.20 * (1.0 - t))).clamp(0.0, 1.0);
            hsl_to_rgb(h, sat, l)
        })
        .collect()
}

/// Soft pastel of `base` for grid lines (high lightness, low saturation).
pub fn pastel_of(base: Color32) -> Color32 {
    let (h, s, _) = rgb_to_hsl(base);
    hsl_to_rgb(h, (s * 0.35).clamp(0.0, 0.50), 0.80)
}

/// Slightly stronger pastel of `base` for axis lines.
pub fn axis_variant(base: Color32) -> Color32 {
    let (h, s, _) = rgb_to_hsl(base);
    hsl_to_rgb(h, (s * 0.55).clamp(0.0, 0.65), 0.66)
}

/// Outline variant: lighter than `base` on a dark background, darker on a light
/// one, so borders stay visible (spec 013 R6).
pub fn border_variant(base: Color32, dark_bg: bool) -> Color32 {
    let (h, s, l) = rgb_to_hsl(base);
    let nl = if dark_bg {
        (l + 0.22).min(0.92)
    } else {
        (l - 0.22).max(0.10)
    };
    hsl_to_rgb(h, s, nl)
}

/// The fixed set of **256** selectable monochrome base colours (spec 013 R10): a
/// 16-hue × 16 saturation/lightness grid with lightness bounded to ~[0.24, 0.80]
/// so pure black/white (and near-extremes) are never offered.
pub fn chart_palette_256() -> Vec<Color32> {
    // One column (hue ~292°, magenta) is replaced by a ramp of 16 greys.
    const GREY_COL: u32 = 13;
    let mut out = Vec::with_capacity(256);
    for hi in 0..16u32 {
        let h = hi as f32 / 16.0 * 360.0;
        for li in 0..16u32 {
            let l = 0.24 + (li as f32 / 15.0) * 0.56; // 0.24 .. 0.80
            if hi == GREY_COL {
                let v = (l * 255.0).round() as u8; // grey, never pure black/white
                out.push(Color32::from_rgb(v, v, v));
            } else {
                let s = 0.45 + ((li % 4) as f32 / 3.0) * 0.50; // 0.45 .. 0.95
                out.push(hsl_to_rgb(h, s.clamp(0.0, 1.0), l));
            }
        }
    }
    out
}

/// Shade `base` by `delta` lightness (positive = lighter, negative = darker),
/// keeping hue/saturation — used for the diagonal monochrome gradient (spec 013).
pub fn shade(base: Color32, delta: f32) -> Color32 {
    let (h, s, l) = rgb_to_hsl(base);
    hsl_to_rgb(h, s, (l + delta).clamp(0.0, 1.0))
}

// ── Smoothing + gradient meshes (spec 013 gradient/smooth) ──────────────────

/// Catmull-Rom spline through `pts`, `seg` samples per segment — for smooth
/// line/area curves. Returns `pts` unchanged when there are < 3 points.
fn catmull_rom(pts: &[Pos2], seg: usize) -> Vec<Pos2> {
    if pts.len() < 3 || seg < 2 {
        return pts.to_vec();
    }
    let n = pts.len();
    let mut out = Vec::with_capacity((n - 1) * seg + 1);
    for i in 0..n - 1 {
        let p0 = pts[i.saturating_sub(1)];
        let p1 = pts[i];
        let p2 = pts[i + 1];
        let p3 = pts[(i + 2).min(n - 1)];
        for s in 0..seg {
            let t = s as f32 / seg as f32;
            let t2 = t * t;
            let t3 = t2 * t;
            let x = 0.5
                * ((2.0 * p1.x)
                    + (-p0.x + p2.x) * t
                    + (2.0 * p0.x - 5.0 * p1.x + 4.0 * p2.x - p3.x) * t2
                    + (-p0.x + 3.0 * p1.x - 3.0 * p2.x + p3.x) * t3);
            let y = 0.5
                * ((2.0 * p1.y)
                    + (-p0.y + p2.y) * t
                    + (2.0 * p0.y - 5.0 * p1.y + 4.0 * p2.y - p3.y) * t2
                    + (-p0.y + 3.0 * p1.y - 3.0 * p2.y + p3.y) * t3);
            out.push(Pos2::new(x, y));
        }
    }
    out.push(pts[n - 1]);
    out
}

/// Vertical-gradient rectangle (top colour → bottom colour) — one bar's own
/// gradient (spec 013).
fn grad_rect_mesh(rect: egui::Rect, top: Color32, bottom: Color32) -> egui::epaint::Mesh {
    let uv = egui::epaint::WHITE_UV;
    let mut m = egui::epaint::Mesh::default();
    m.vertices.push(egui::epaint::Vertex {
        pos: rect.left_top(),
        uv,
        color: top,
    });
    m.vertices.push(egui::epaint::Vertex {
        pos: rect.right_top(),
        uv,
        color: top,
    });
    m.vertices.push(egui::epaint::Vertex {
        pos: rect.right_bottom(),
        uv,
        color: bottom,
    });
    m.vertices.push(egui::epaint::Vertex {
        pos: rect.left_bottom(),
        uv,
        color: bottom,
    });
    m.indices.extend([0, 1, 2, 0, 2, 3]);
    m
}

/// Like [`grad_rect_mesh`] but with rounded corners (`radius` px, clamped to half
/// the smaller side). Triangle-fan from the centre to a rounded perimeter; each
/// vertex is coloured by its vertical position so the top→bottom gradient is
/// preserved. `radius == 0` degenerates to a plain rectangle.
fn grad_round_rect_mesh(
    rect: egui::Rect,
    top: Color32,
    bottom: Color32,
    radius: f32,
) -> egui::epaint::Mesh {
    use std::f32::consts::PI;
    let uv = egui::epaint::WHITE_UV;
    let mut m = egui::epaint::Mesh::default();
    let r = radius
        .max(0.0)
        .min(rect.width() * 0.5)
        .min(rect.height() * 0.5);
    let h = rect.height().max(1.0);
    let color_at = |y: f32| lerp_color(top, bottom, (y - rect.min.y) / h);

    let center = rect.center();
    m.vertices.push(egui::epaint::Vertex {
        pos: center,
        uv,
        color: color_at(center.y),
    });

    // Rounded perimeter (clockwise), a few segments per corner arc.
    let seg = 4usize;
    let mut perim: Vec<Pos2> = Vec::new();
    let mut arc = |cx: f32, cy: f32, a0: f32, a1: f32| {
        for k in 0..=seg {
            let t = a0 + (a1 - a0) * (k as f32 / seg as f32);
            perim.push(Pos2::new(cx + r * t.cos(), cy + r * t.sin()));
        }
    };
    arc(rect.min.x + r, rect.min.y + r, PI, 1.5 * PI); // top-left
    arc(rect.max.x - r, rect.min.y + r, 1.5 * PI, 2.0 * PI); // top-right
    arc(rect.max.x - r, rect.max.y - r, 0.0, 0.5 * PI); // bottom-right
    arc(rect.min.x + r, rect.max.y - r, 0.5 * PI, PI); // bottom-left

    let base = m.vertices.len() as u32;
    for p in &perim {
        m.vertices.push(egui::epaint::Vertex {
            pos: *p,
            uv,
            color: color_at(p.y),
        });
    }
    let count = perim.len() as u32;
    for k in 0..count {
        m.indices.extend([0, base + k, base + (k + 1) % count]);
    }
    m
}

/// Lerp two colours in straight component space (`t` clamped to 0..=1).
fn lerp_color(a: Color32, b: Color32, t: f32) -> Color32 {
    let t = t.clamp(0.0, 1.0);
    let l = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * t).round() as u8;
    Color32::from_rgba_premultiplied(
        l(a.r(), b.r()),
        l(a.g(), b.g()),
        l(a.b(), b.b()),
        l(a.a(), b.a()),
    )
}

/// Directional gradient fill of `rect`, `start`→`end` (spec 015 GroupBox
/// background). Linear directions (Vertical/Horizontal/DiagonalDown/DiagonalUp)
/// use a 4-vertex quad with GPU-interpolated corner colours; Radial uses a
/// centre→edge fan. Corners are square — the rounded border is stroked
/// separately (same trade-off as the other mesh fills here).
fn gradient_direction(dir: &str) -> egui::Vec2 {
    match dir {
        "North" => egui::vec2(0.0, -1.0),
        "NorthEast" | "DiagonalUp" => egui::vec2(1.0, -1.0).normalized(),
        "East" | "Horizontal" => egui::vec2(1.0, 0.0),
        "SouthEast" | "DiagonalDown" => egui::vec2(1.0, 1.0).normalized(),
        "South" | "Vertical" => egui::vec2(0.0, 1.0),
        "SouthWest" => egui::vec2(-1.0, 1.0).normalized(),
        "West" => egui::vec2(-1.0, 0.0),
        "NorthWest" => egui::vec2(-1.0, -1.0).normalized(),
        _ => egui::vec2(0.0, 1.0),
    }
}

pub(crate) fn gradient_color_at(
    rect: egui::Rect,
    start: Color32,
    end: Color32,
    dir: &str,
    pos: egui::Pos2,
) -> Color32 {
    if dir == "Radial" {
        let half = egui::vec2(rect.width() * 0.5, rect.height() * 0.5);
        let delta = pos - rect.center();
        let t = ((delta.x / half.x.max(1.0)).powi(2) + (delta.y / half.y.max(1.0)).powi(2)).sqrt();
        return lerp_color(start, end, t);
    }
    let vector = gradient_direction(dir);
    let half_extent = (vector.x.abs() * rect.width() + vector.y.abs() * rect.height()) * 0.5;
    let projected = (pos - rect.center()).dot(vector);
    lerp_color(start, end, 0.5 + projected / (2.0 * half_extent.max(1.0)))
}

/// Rounded background-gradient mesh. Its perimeter follows each corner radius,
/// while vertex colors remain an affine eight-direction gradient.
pub fn background_gradient_mesh(
    rect: egui::Rect,
    start: Color32,
    end: Color32,
    dir: &str,
    rounding: egui::CornerRadius,
) -> egui::epaint::Mesh {
    use std::f32::consts::PI;

    let cap = rect.width().min(rect.height()).max(0.0) * 0.5;
    let radii = [
        f32::from(rounding.nw).min(cap),
        f32::from(rounding.ne).min(cap),
        f32::from(rounding.se).min(cap),
        f32::from(rounding.sw).min(cap),
    ];
    let corners = [
        (
            egui::pos2(rect.left() + radii[0], rect.top() + radii[0]),
            PI,
            1.5 * PI,
        ),
        (
            egui::pos2(rect.right() - radii[1], rect.top() + radii[1]),
            1.5 * PI,
            2.0 * PI,
        ),
        (
            egui::pos2(rect.right() - radii[2], rect.bottom() - radii[2]),
            0.0,
            0.5 * PI,
        ),
        (
            egui::pos2(rect.left() + radii[3], rect.bottom() - radii[3]),
            0.5 * PI,
            PI,
        ),
    ];
    let square_points = [
        rect.left_top(),
        rect.right_top(),
        rect.right_bottom(),
        rect.left_bottom(),
    ];
    let mut perimeter = Vec::new();
    for (index, (center, begin, finish)) in corners.iter().enumerate() {
        let radius = radii[index];
        if radius < 0.5 {
            perimeter.push(square_points[index]);
            continue;
        }
        let segments = ((radius / 2.0).ceil() as usize).clamp(4, 20);
        for step in 0..=segments {
            let angle = begin + (finish - begin) * step as f32 / segments as f32;
            perimeter.push(egui::pos2(
                center.x + radius * angle.cos(),
                center.y + radius * angle.sin(),
            ));
        }
    }

    let mut mesh = egui::epaint::Mesh::default();
    let center = rect.center();
    mesh.vertices.push(egui::epaint::Vertex {
        pos: center,
        uv: egui::epaint::WHITE_UV,
        color: gradient_color_at(rect, start, end, dir, center),
    });
    for point in &perimeter {
        mesh.vertices.push(egui::epaint::Vertex {
            pos: *point,
            uv: egui::epaint::WHITE_UV,
            color: gradient_color_at(rect, start, end, dir, *point),
        });
    }
    for index in 0..perimeter.len() as u32 {
        let next = if index + 1 == perimeter.len() as u32 {
            1
        } else {
            index + 2
        };
        mesh.indices.extend([0, index + 1, next]);
    }
    mesh
}

fn grad_dir_mesh(rect: egui::Rect, start: Color32, end: Color32, dir: &str) -> egui::epaint::Mesh {
    if dir != "Radial" {
        return background_gradient_mesh(rect, start, end, dir, egui::CornerRadius::ZERO);
    }
    let uv = egui::epaint::WHITE_UV;
    let mut m = egui::epaint::Mesh::default();
    if dir == "Radial" {
        let c = rect.center();
        m.vertices.push(egui::epaint::Vertex {
            pos: c,
            uv,
            color: start,
        });
        let perim = [
            rect.left_top(),
            egui::pos2(c.x, rect.top()),
            rect.right_top(),
            egui::pos2(rect.right(), c.y),
            rect.right_bottom(),
            egui::pos2(c.x, rect.bottom()),
            rect.left_bottom(),
            egui::pos2(rect.left(), c.y),
        ];
        for p in perim {
            m.vertices.push(egui::epaint::Vertex {
                pos: p,
                uv,
                color: end,
            });
        }
        let n = perim.len() as u32;
        for i in 1..=n {
            let j = if i == n { 1 } else { i + 1 };
            m.indices.extend([0, i, j]);
        }
        return m;
    }
    let mid = lerp_color(start, end, 0.5);
    // (top-left, top-right, bottom-right, bottom-left)
    let (tl, tr, br, bl) = match dir {
        "Horizontal"   => (start, end, end, start),
        "DiagonalDown" => (start, mid, end, mid),   // TL → BR
        "DiagonalUp"   => (mid, end, start, mid),   // BL → TR
        _ /* Vertical */ => (start, start, end, end),
    };
    m.vertices.push(egui::epaint::Vertex {
        pos: rect.left_top(),
        uv,
        color: tl,
    });
    m.vertices.push(egui::epaint::Vertex {
        pos: rect.right_top(),
        uv,
        color: tr,
    });
    m.vertices.push(egui::epaint::Vertex {
        pos: rect.right_bottom(),
        uv,
        color: br,
    });
    m.vertices.push(egui::epaint::Vertex {
        pos: rect.left_bottom(),
        uv,
        color: bl,
    });
    m.indices.extend([0, 1, 2, 0, 2, 3]);
    m
}

/// Radial-gradient disc (centre colour → edge colour) — one scatter bubble's or
/// pie slice's own gradient (spec 013).
fn radial_disc_mesh(center: Pos2, rad: f32, cc: Color32, ce: Color32) -> egui::epaint::Mesh {
    let uv = egui::epaint::WHITE_UV;
    let n = 24_u32;
    let mut m = egui::epaint::Mesh::default();
    m.vertices.push(egui::epaint::Vertex {
        pos: center,
        uv,
        color: cc,
    });
    for i in 0..n {
        let a = i as f32 / n as f32 * std::f32::consts::TAU;
        m.vertices.push(egui::epaint::Vertex {
            pos: center + Vec2::new(a.cos(), a.sin()) * rad,
            uv,
            color: ce,
        });
    }
    for i in 1..=n {
        let j = if i == n { 1 } else { i + 1 };
        m.indices.extend([0, i, j]);
    }
    m
}

/// Vertical gradient area fill below a polyline: each column fades from `top_c`
/// at the line to `bot_c` at `baseline` — the line-chart gradient (spec 013).
fn grad_area_mesh(
    top: &[Pos2],
    baseline: f32,
    top_c: Color32,
    bot_c: Color32,
) -> egui::epaint::Mesh {
    let uv = egui::epaint::WHITE_UV;
    let mut m = egui::epaint::Mesh::default();
    for (i, p) in top.iter().enumerate() {
        let base = m.vertices.len() as u32;
        m.vertices.push(egui::epaint::Vertex {
            pos: *p,
            uv,
            color: top_c,
        });
        m.vertices.push(egui::epaint::Vertex {
            pos: egui::pos2(p.x, baseline),
            uv,
            color: bot_c,
        });
        if i > 0 {
            m.indices
                .extend([base - 2, base - 1, base, base - 1, base + 1, base]);
        }
    }
    m
}

/// Pie/donut slice with a radial gradient (inner `cc` → outer `ce`). `inner_r`
/// 0 ⇒ solid pie fan; > 0 ⇒ donut ring strip (spec 013).
fn grad_slice_mesh(
    center: Pos2,
    start: f32,
    sweep: f32,
    inner_r: f32,
    outer_r: f32,
    cc: Color32,
    ce: Color32,
) -> egui::epaint::Mesh {
    let uv = egui::epaint::WHITE_UV;
    let steps = ((sweep.abs() * outer_r).max(4.0) as u32).clamp(4, 40);
    let mut m = egui::epaint::Mesh::default();
    if inner_r <= 0.0 {
        m.vertices.push(egui::epaint::Vertex {
            pos: center,
            uv,
            color: cc,
        });
        for s in 0..=steps {
            let t = start + sweep * s as f32 / steps as f32;
            m.vertices.push(egui::epaint::Vertex {
                pos: Pos2::new(center.x + t.cos() * outer_r, center.y + t.sin() * outer_r),
                uv,
                color: ce,
            });
        }
        for s in 1..=steps {
            m.indices.extend([0, s, s + 1]);
        }
    } else {
        for s in 0..=steps {
            let t = start + sweep * s as f32 / steps as f32;
            let (ct, st) = (t.cos(), t.sin());
            m.vertices.push(egui::epaint::Vertex {
                pos: Pos2::new(center.x + ct * inner_r, center.y + st * inner_r),
                uv,
                color: cc,
            });
            m.vertices.push(egui::epaint::Vertex {
                pos: Pos2::new(center.x + ct * outer_r, center.y + st * outer_r),
                uv,
                color: ce,
            });
            if s > 0 {
                let b = (s * 2) as u32;
                m.indices.extend([b - 2, b - 1, b, b - 1, b + 1, b]);
            }
        }
    }
    m
}

/// Draw a rich glass chart preview on the canvas for all chart control types.
#[allow(clippy::too_many_arguments)]
pub fn draw_chart_preview(
    painter: &egui::Painter,
    ctrl: &Control,
    rect: egui::Rect,
    a: u8,
    alpha_mul: f32,
    glass: bool,
    selected: bool,
    rounding: egui::CornerRadius, // per-corner: own radius, lifted to a container border
) {
    use crate::model::ControlType as CT;

    let chart_diag = frame_diagnostics_enabled();

    let _ = selected; // selection border drawn by caller
    let control_rect = rect;

    // ── Background ────────────────────────────────────────────────────────────
    // `HideBackground` suppresses the panel fill + border frame so only the chart
    // content (grid, axes, labels, data) is visible, transparent over the form.
    let hide_bg = ctrl
        .get_prop("HideBackground")
        .map(|v| v.as_bool())
        .unwrap_or(false);
    // Per-corner rounding (spec 016 default 8, lifted to a container's border by the
    // caller so the chart card never bleeds past a rounded GroupBox/Panel corner).
    // Neumorphic charts sit on the light soft-UI surface: the face is the light
    // panel tone (draw_neumorphic uses it as its surface colour) — never the dark
    // navy glass face, which would punch a dark hole in the soft light card.
    let is_neumorphic = active_glass_style(painter.ctx()).is_neumorphic();
    let default_face = if is_neumorphic {
        Color32::from_rgb(232, 237, 254) // soft lavender-blue (#E8EDFE)
    } else {
        Color32::from_rgb(15, 20, 45)
    };
    let face = ctrl
        .get_prop("BackgroundColor")
        .map(|v| parse_color(v.as_str()))
        .unwrap_or(default_face);
    let bg = Color32::from_rgba_premultiplied(
        (face.r() as f32 * a as f32 / 255.0) as u8,
        (face.g() as f32 * a as f32 / 255.0) as u8,
        (face.b() as f32 * a as f32 / 255.0) as u8,
        a,
    );
    if !hide_bg {
        // Charts draw dense internal content and then repaint rounded-corner
        // notches. Using the generic glass card here also paints its own dark
        // depth layers under the chart face, which show as black square corner
        // bleed. Keep charts on a single rounded face; external drop shadows are
        // still handled by the normal shadow path in `draw_control`.
        if glass && is_neumorphic {
            let shadow_rect = debug_frame(
                painter,
                control_rect,
                rounding,
                0,
                "CHART_NEU_SHADOW",
                chart_diag,
            );
            draw_neumorphic_shadow_only(painter, shadow_rect, rounding, alpha_mul);
        }
        let face_rect = debug_frame(painter, control_rect, rounding, 1, "CHART_FACE", chart_diag);
        painter.rect_filled(face_rect, rounding, bg);
        if glass && is_neumorphic {
            draw_neumorphic_overlay_shadow_only(painter, face_rect, rounding, alpha_mul);
        }
        let border = Color32::from_rgba_premultiplied(60, 80, 160, a);
        let border_rect = debug_frame(
            painter,
            control_rect,
            rounding,
            2,
            "CHART_BORDER",
            chart_diag,
        );
        painter.rect_stroke(
            border_rect,
            rounding,
            Stroke::new(1.0, border),
            egui::StrokeKind::Middle,
        );
    }

    let frame_painter = painter;
    let rect = debug_frame(
        frame_painter,
        control_rect,
        rounding,
        3,
        "CHART_CONTENT",
        chart_diag,
    );

    // 007 chart-style hook — an asset-pack theme supplies the data palette and
    // stroke width for the data marks (pie slices / lines / bars), so charts take
    // on the theme like every other control (R7). Liquid Glass keeps the built-in
    // accent palette.
    let active = active_theme(painter.ctx());
    let pal_raw: &[(u8, u8, u8)] = &[
        (76, 155, 232),
        (232, 122, 76),
        (76, 232, 122),
        (232, 76, 155),
    ];
    let base_pal: Vec<Color32> = active
        .as_ref()
        .map(|p| &p.manifest.palette.chart)
        .filter(|v| !v.is_empty())
        .map(|v| v.iter().map(|s| parse_color(s)).collect::<Vec<_>>())
        .unwrap_or_else(|| {
            pal_raw
                .iter()
                .map(|&(r, g, b)| Color32::from_rgb(r, g, b))
                .collect()
        });
    let chart_stroke = active
        .as_ref()
        .map(|p| p.manifest.chart_style.stroke_width)
        .filter(|w| *w > 0.0)
        .unwrap_or(1.8);

    // ── Monochrome mode (spec 013) ─────────────────────────────────────────────
    // When on, the data palette is replaced by tonal variations of one base
    // colour, and support colours (grid/axis/border) become derived variants. The
    // chart face is dark, so borders take the *lighter* variant. Text/alpha are
    // left untouched (handled by the existing paths below).
    let mono = ctrl
        .get_prop("Monochrome")
        .map(|v| v.as_bool())
        .unwrap_or(false);
    let mono_base = parse_color(
        &ctrl
            .get_prop("MonochromeColor")
            .map(|v| v.as_str().to_owned())
            .unwrap_or_else(|| "#3F6FB5".into()),
    );
    let pal: Vec<Color32> = if mono {
        let k = if matches!(ctrl.control_type, CT::PieChart | CT::DonutChart) {
            4
        } else {
            2
        };
        monochrome_palette(mono_base, k)
    } else {
        base_pal
    };
    let mono_border = border_variant(mono_base, true);

    // Inner plot area (leave margin for axes / labels)
    let margin_l = rect.width() * 0.10;
    let margin_b = rect.height() * 0.12;
    let margin_t = rect.height() * 0.12;
    let margin_r = rect.width() * 0.04;
    let plot = egui::Rect::from_min_max(
        Pos2::new(rect.min.x + margin_l, rect.min.y + margin_t),
        Pos2::new(rect.max.x - margin_r, rect.max.y - margin_b),
    );
    let content_clip = plot.expand(8.0).intersect(rect.shrink(1.0));
    let painter = &painter.with_clip_rect(content_clip);

    // Monochrome gradient (spec 013): when on, each data element gets its OWN
    // tonal gradient (bars vertical, bubbles/slices radial) and line/area charts
    // get a vertical fill gradient — handled per-branch via mesh helpers below.
    let gradient = mono
        && ctrl
            .get_prop("MonochromeGradient")
            .map(|v| v.as_bool())
            .unwrap_or(false);

    // title
    let title = ctrl
        .get_prop("Title")
        .map(|v| v.as_str().to_owned())
        .unwrap_or_default();
    if !title.is_empty() {
        painter.text(
            Pos2::new(rect.center().x, rect.min.y + margin_t * 0.5),
            egui::Align2::CENTER_CENTER,
            &title,
            egui::FontId::proportional(10.0),
            // The design-time grid face is white — the title must be dark to
            // be readable (it was near-white and invisible on the face).
            Color32::DARK_GRAY,
        );
    }

    // ── Grid lines ────────────────────────────────────────────────────────────
    let show_grid = ctrl
        .get_prop("ShowGridLines")
        .map(|v| v.as_bool())
        .unwrap_or(true);
    if show_grid {
        // Monochrome: grid lines use a soft pastel of the base colour (spec 013 R5).
        // Neumorphic: faint gray-blue on the light face (strong blue reads harsh).
        let grid_c = if mono {
            pastel_of(mono_base)
        } else {
            Color32::from_rgb(118, 142, 225)
        };
        let n_h = 4u32;
        for i in 1..n_h {
            let y = plot.min.y + plot.height() * i as f32 / n_h as f32;
            painter.line_segment(
                [Pos2::new(plot.min.x, y), Pos2::new(plot.max.x, y)],
                Stroke::new(1.15, grid_c),
            );
        }
        if !matches!(ctrl.control_type, CT::PieChart | CT::DonutChart) {
            let n_v = 5u32;
            for i in 1..n_v {
                let x = plot.min.x + plot.width() * i as f32 / n_v as f32;
                painter.line_segment(
                    [Pos2::new(x, plot.min.y), Pos2::new(x, plot.max.y)],
                    Stroke::new(1.15, grid_c),
                );
            }
        }
    }

    // Axes (monochrome: a pastel/slightly-stronger variant of the base — spec 013 R5)
    let ax_c = if mono {
        axis_variant(mono_base)
    } else {
        Color32::from_rgb(84, 104, 190)
    };
    if !matches!(ctrl.control_type, CT::PieChart | CT::DonutChart) {
        // X/Y axis-line visibility is independently toggleable (default on).
        let show_x = ctrl
            .get_prop("ShowXAxis")
            .map(|v| v.as_bool())
            .unwrap_or(true);
        let show_y = ctrl
            .get_prop("ShowYAxis")
            .map(|v| v.as_bool())
            .unwrap_or(true);
        if show_x {
            painter.line_segment(
                [plot.left_bottom(), plot.right_bottom()],
                Stroke::new(1.45, ax_c),
            );
        }
        if show_y {
            painter.line_segment(
                [plot.left_bottom(), plot.left_top()],
                Stroke::new(1.45, ax_c),
            );
        }
    }

    // ── Data ──────────────────────────────────────────────────────────────────
    // Live data pushed from COBOL via the `COBOL-CHART-*` runtime calls arrives as
    // the control's `__ChartData` property: one `label<TAB>value` per line. When
    // present it is auto-scaled to the plot and drawn; otherwise a representative
    // sample is shown, so the designer canvas and an unpopulated chart still look
    // meaningful.
    let live: Vec<(String, f32)> = ctrl
        .get_prop("__ChartData")
        .map(|v| v.as_str().to_owned())
        .filter(|s| !s.trim().is_empty())
        .map(|s| {
            s.lines()
                .filter_map(|ln| {
                    let mut it = ln.splitn(2, '\t');
                    let label = it.next()?.to_owned();
                    let value: f32 = it.next()?.trim().parse().ok()?;
                    Some((label, value))
                })
                .collect()
        })
        .unwrap_or_default();

    // Sample fallback (normalised Y for 5 points, 2 series).
    let sample1: &[f32] = &[0.40, 0.70, 0.55, 0.85, 0.60];
    let sample2: &[f32] = &[0.25, 0.45, 0.70, 0.50, 0.80];

    // Auto-scale live values into the plot's 0..1 band (max → top).
    let live_norm: Vec<f32> = if live.is_empty() {
        Vec::new()
    } else {
        let maxv = live
            .iter()
            .map(|(_, v)| *v)
            .fold(0.0_f32, f32::max)
            .max(f32::EPSILON);
        live.iter()
            .map(|(_, v)| (v / maxv).clamp(0.0, 1.0))
            .collect()
    };
    let series1: &[f32] = if live.is_empty() { sample1 } else { &live_norm };
    let series2: &[f32] = if live.is_empty() { sample2 } else { &[] };
    let n = series1.len().max(1);

    let px_x = |i: usize| plot.min.x + (i as f32 + 0.5) / n as f32 * plot.width();
    let px_y = |v: f32| plot.max.y - v * plot.height();

    // Line/area curve smoothing (spec 013): the `Smooth` property now actually
    // bends the polyline into a Catmull-Rom spline. `ShowPoints` gates markers.
    let smooth = ctrl.get_prop("Smooth").map(|v| v.as_bool()).unwrap_or(true);
    let show_points = ctrl
        .get_prop("ShowPoints")
        .map(|v| v.as_bool())
        .unwrap_or(true);

    match ctrl.control_type {
        CT::BarChart => {
            let horizontal = ctrl
                .get_prop("Horizontal")
                .map(|v| v.as_bool())
                .unwrap_or(false);
            let bar_total = plot.width() / n as f32;
            let bar_w = bar_total * 0.38;
            let gap = bar_total * 0.05;
            // Per-bar corner radius from the BarCornerRadius property (default 3),
            // clamped per-bar to half the smaller side so short/thin bars stay sane.
            let bar_corner = ctrl
                .get_prop("BarCornerRadius")
                .map(|v| v.as_i64() as f32)
                .unwrap_or(3.0)
                .max(0.0);
            for (si, series) in [series1, series2].iter().enumerate() {
                for (i, &v) in series.iter().enumerate() {
                    let br = if horizontal {
                        let y = plot.min.y
                            + (i as f32 + 0.5 + si as f32 * (0.5 + gap)) / n as f32 * plot.height()
                            - bar_w * 0.5;
                        let w = v * plot.width();
                        egui::Rect::from_min_size(Pos2::new(plot.min.x, y), Vec2::new(w, bar_w))
                    } else {
                        let x =
                            plot.min.x + (i as f32 * bar_total) + si as f32 * (bar_w + gap) + gap;
                        let h = v * plot.height();
                        egui::Rect::from_min_size(Pos2::new(x, plot.max.y - h), Vec2::new(bar_w, h))
                    };
                    let r = bar_corner.min(br.width() * 0.5).min(br.height() * 0.5);
                    if gradient {
                        // Each bar gets its own light→dark vertical gradient, with
                        // the configured rounded corners.
                        painter.add(egui::Shape::mesh(grad_round_rect_mesh(
                            br,
                            shade(mono_base, 0.20),
                            shade(mono_base, -0.20),
                            r,
                        )));
                    } else {
                        painter.rect_filled(br, r, pal[si % pal.len()]);
                    }
                }
            }
        }
        CT::LineChart => {
            for (si, series) in [series1, series2].iter().enumerate() {
                let raw: Vec<Pos2> = series
                    .iter()
                    .enumerate()
                    .map(|(i, &v)| Pos2::new(px_x(i), px_y(v)))
                    .collect();
                // `Smooth` bends the line into a Catmull-Rom curve (spec 013).
                let line = if smooth {
                    catmull_rom(&raw, 14)
                } else {
                    raw.clone()
                };
                let c = pal[si % pal.len()];
                if gradient {
                    // Vertical gradient fill under the line: brightest at the line,
                    // fading to transparent at the baseline (spec 013, mockup look).
                    let top_c = shade(mono_base, 0.12);
                    let bot_c = Color32::from_rgba_unmultiplied(top_c.r(), top_c.g(), top_c.b(), 0);
                    painter.add(egui::Shape::mesh(grad_area_mesh(
                        &line, plot.max.y, top_c, bot_c,
                    )));
                }
                let line_c = if gradient { shade(mono_base, 0.10) } else { c };
                for w in line.windows(2) {
                    painter.line_segment([w[0], w[1]], Stroke::new(chart_stroke, line_c));
                }
                if show_points {
                    for &p in &raw {
                        painter.circle_filled(p, 3.0, line_c);
                    }
                }
            }
        }
        CT::AreaChart => {
            for (si, series) in [series1, series2].iter().enumerate() {
                let raw: Vec<Pos2> = series
                    .iter()
                    .enumerate()
                    .map(|(i, &v)| Pos2::new(px_x(i), px_y(v)))
                    .collect();
                let top = if smooth {
                    catmull_rom(&raw, 14)
                } else {
                    raw.clone()
                };
                // Fill via a per-column mesh (handles the concave smoothed edge).
                // Non-gradient keeps the existing alpha-80 translucency (R8);
                // gradient fades vertically from the line to transparent.
                let (top_c, bot_c, line_c) = if gradient {
                    let t = shade(mono_base, 0.12);
                    (
                        Color32::from_rgba_unmultiplied(t.r(), t.g(), t.b(), 150),
                        Color32::from_rgba_unmultiplied(t.r(), t.g(), t.b(), 0),
                        shade(mono_base, 0.10),
                    )
                } else {
                    let c = pal[si % pal.len()];
                    let f = Color32::from_rgba_unmultiplied(c.r(), c.g(), c.b(), 80);
                    (f, f, c)
                };
                painter.add(egui::Shape::mesh(grad_area_mesh(
                    &top, plot.max.y, top_c, bot_c,
                )));
                for w in top.windows(2) {
                    painter.line_segment([w[0], w[1]], Stroke::new(chart_stroke, line_c));
                }
            }
        }
        CT::ScatterChart => {
            // Live data → one point per (index, value); index spreads across X,
            // value (auto-scaled) is Y. Sample clusters shown when no data is set.
            let sample1: &[(f32, f32)] = &[
                (0.15, 0.65),
                (0.35, 0.40),
                (0.50, 0.78),
                (0.70, 0.30),
                (0.88, 0.55),
            ];
            let sample2: &[(f32, f32)] = &[(0.20, 0.30), (0.42, 0.72), (0.60, 0.45), (0.78, 0.85)];
            let live_pts: Vec<(f32, f32)> = live_norm
                .iter()
                .enumerate()
                .map(|(i, &vy)| ((i as f32 + 0.5) / n as f32, vy))
                .collect();
            let groups: Vec<(&[(f32, f32)], usize)> = if live.is_empty() {
                vec![(sample1, 0usize), (sample2, 1)]
            } else {
                vec![(live_pts.as_slice(), 0usize)]
            };
            for (pts, ci) in groups {
                let c = pal[ci % pal.len()];
                for &(fx, fy) in pts {
                    let p = Pos2::new(
                        plot.min.x + fx * plot.width(),
                        plot.max.y - fy * plot.height(),
                    );
                    if gradient {
                        // Each bubble: its own radial gradient (light centre → dark edge).
                        painter.add(egui::Shape::mesh(radial_disc_mesh(
                            p,
                            5.0,
                            shade(mono_base, 0.20),
                            shade(mono_base, -0.20),
                        )));
                    } else {
                        painter.circle_stroke(p, 4.5, Stroke::new(1.5, c));
                    }
                }
            }
        }
        CT::PieChart | CT::DonutChart => {
            let center = plot.center();
            let outer_r = plot.size().min_elem() * 0.44;
            let inner_r = if ctrl.control_type == CT::DonutChart {
                let pct = ctrl
                    .get_prop("InnerRadius")
                    .map(|v| v.as_i64())
                    .unwrap_or(40) as f32
                    / 100.0;
                outer_r * pct
            } else {
                0.0
            };

            // Neumorphic: the pie sits ON the soft surface, so give it a faint

            // Live data → each value becomes a slice proportional to the total.
            // Sample proportions shown when no data is set.
            let slice_vec: Vec<f32> = if live.is_empty() {
                vec![0.30, 0.20, 0.25, 0.25]
            } else {
                let sum: f32 = live
                    .iter()
                    .map(|(_, v)| v.max(0.0))
                    .sum::<f32>()
                    .max(f32::EPSILON);
                live.iter().map(|(_, v)| v.max(0.0) / sum).collect()
            };
            let slices: &[f32] = &slice_vec; // proportions
            let mut start = -std::f32::consts::FRAC_PI_2; // top
            for (i, &frac) in slices.iter().enumerate() {
                let sweep = frac * TAU;
                let end = start + sweep;
                let steps = ((sweep * outer_r).max(4.0) as u32).min(40).max(4);
                // Outline points (fan for pie, ring for donut) — used for the
                // slice border in both fill modes.
                let mut pts: Vec<Pos2> = Vec::with_capacity(steps as usize + 2);
                if inner_r > 0.0 {
                    for s in 0..=steps {
                        let t = start + sweep * s as f32 / steps as f32;
                        pts.push(Pos2::new(
                            center.x + t.cos() * outer_r,
                            center.y + t.sin() * outer_r,
                        ));
                    }
                    for s in (0..=steps).rev() {
                        let t = start + sweep * s as f32 / steps as f32;
                        pts.push(Pos2::new(
                            center.x + t.cos() * inner_r,
                            center.y + t.sin() * inner_r,
                        ));
                    }
                } else {
                    pts.push(center);
                    for s in 0..=steps {
                        let t = start + sweep * s as f32 / steps as f32;
                        pts.push(Pos2::new(
                            center.x + t.cos() * outer_r,
                            center.y + t.sin() * outer_r,
                        ));
                    }
                }
                // Monochrome: slice borders use a lighter variant of the base so
                // adjacent slices separate on the dark face (spec 013 R6).
                // Neumorphic: thin white separators — a soft "molded" sheen
                // between pastel sectors instead of the dark face colour.
                let slice_stroke = if mono { mono_border } else { bg };
                let sep_w = 0.8;
                if gradient {
                    // Each slice gets its own radial gradient (light inner → dark outer).
                    painter.add(egui::Shape::mesh(grad_slice_mesh(
                        center,
                        start,
                        sweep,
                        inner_r,
                        outer_r,
                        shade(mono_base, 0.20),
                        shade(mono_base, -0.20),
                    )));
                    painter.add(egui::Shape::closed_line(
                        pts,
                        Stroke::new(sep_w, slice_stroke),
                    ));
                } else {
                    let c = pal[i % pal.len()];
                    let fill = Color32::from_rgba_premultiplied(
                        c.r(),
                        c.g(),
                        c.b(),
                        (a as f32 * 0.85) as u8,
                    );
                    painter.add(egui::Shape::convex_polygon(
                        pts,
                        fill,
                        Stroke::new(sep_w, slice_stroke),
                    ));
                }
                start = end;
            }
        }
        _ => {}
    }

    // data source hint
    let ds = ctrl
        .get_prop("DataSource")
        .map(|v| v.as_str().to_owned())
        .unwrap_or_default();
    if !ds.is_empty() {
        let hint_c = Color32::from_rgba_premultiplied(130, 160, 220, a);
        painter.text(
            Pos2::new(rect.center().x, rect.max.y - margin_b * 0.4),
            egui::Align2::CENTER_CENTER,
            format!("⬡ {ds}"),
            egui::FontId::proportional(8.5),
            hint_c,
        );
    }

    // type badge
    let badge = match ctrl.control_type {
        CT::BarChart => "BAR",
        CT::LineChart => "LINE",
        CT::PieChart => "PIE",
        CT::AreaChart => "AREA",
        CT::ScatterChart => "SCATTER",
        CT::DonutChart => "DONUT",
        _ => "",
    };
    if !badge.is_empty() {
        let badge_c = Color32::from_rgba_premultiplied(80, 100, 180, a);
        painter.text(
            Pos2::new(rect.max.x - margin_r - 2.0, rect.min.y + margin_t * 0.45),
            egui::Align2::RIGHT_CENTER,
            badge,
            egui::FontId::proportional(8.0),
            badge_c,
        );
    }

    if !hide_bg {
        let outline = if glass {
            Color32::from_rgba_premultiplied(170, 170, 170, (170.0 * alpha_mul) as u8)
        } else {
            Color32::from_rgba_premultiplied(60, 80, 160, a)
        };
        let outline_rect = debug_frame(
            frame_painter,
            control_rect,
            rounding,
            4,
            "CHART_OUTLINE",
            chart_diag,
        );
        frame_painter.rect_stroke(
            outline_rect,
            rounding,
            Stroke::new(1.0, outline),
            egui::StrokeKind::Middle,
        );
    }
}

/// Unified corner radius (px) for a control's rounded fill/border and content
/// (spec 016). Reads the canonical `CornerRadius`, falls back to the legacy
/// container `BorderRadius` (spec 012), then a per-type default, and clamps to
/// half the smaller side so a large value can never produce a degenerate shape.
/// `0` ⇒ square corners (and no rounded clipping).
pub fn corner_radius(ctrl: &Control) -> f32 {
    let raw = ctrl
        .get_prop("CornerRadius")
        .or_else(|| ctrl.get_prop("BorderRadius"))
        .map(|v| v.as_i64() as f32)
        .unwrap_or_else(|| match ctrl.control_type {
            ControlType::Button => 3.0,
            ControlType::BarChart
            | ControlType::LineChart
            | ControlType::PieChart
            | ControlType::AreaChart
            | ControlType::ScatterChart
            | ControlType::DonutChart => 8.0,
            _ => 0.0,
        });
    let max_r = 0.5 * (ctrl.rect.w.min(ctrl.rect.h) as f32);
    raw.clamp(0.0, max_r.max(0.0))
}

pub fn textbox_inner_padding(ctrl: &Control) -> f32 {
    ctrl.get_prop("InnerPadding")
        .map(|v| v.as_i64())
        .unwrap_or(3)
        .clamp(0, 128) as f32
}

/// Corner radius used by regular drop-shadow layers.
///
/// The shadow is painted before the control body, so any part that sits under
/// the control must match the control's own rounded silhouette exactly. Using
/// the canonical helper keeps hard shadows, zero/disabled blur shadows, and soft
/// shadow cores aligned with `CornerRadius`, legacy `BorderRadius`, per-control
/// defaults, and size clamping.
fn drop_shadow_corner_radius(ctrl: &Control) -> f32 {
    // Rectangle shapes round via CornerRadius like everything else; legacy
    // RoundRect forms keep their fixed 8px face radius. (Circle/Triangle never
    // reach this path — they draw silhouette shadows in the Shape branch.)
    if matches!(ctrl.control_type, ControlType::Shape)
        && ctrl.get_prop("ShapeType").map(|v| v.as_str()) == Some("RoundRect")
    {
        return 8.0;
    }
    corner_radius(ctrl)
}

#[derive(Debug, Clone)]
struct RegularDropShadow {
    rect: Rect,
    color: Color32,
    opacity: f32,
    blur_strength: usize,
    corner_radius: f32,
    overlay: bool,
}

fn regular_drop_shadow(
    ctrl: &Control,
    rect: Rect,
    is_neumorphic: bool,
) -> Option<RegularDropShadow> {
    use crate::ControlType as CT;

    if is_neumorphic
        || !ctrl
            .get_prop("ShadowEnabled")
            .map(|v| v.as_bool())
            .unwrap_or(false)
        || matches!(
            ctrl.control_type,
            CT::Line
                | CT::Timer
                | CT::AgentObject
                | CT::RestClient
                | CT::SqlDatabase
                | CT::IndexedFile
        )
        // Circle/Ellipse/Triangle shapes draw their own silhouette-matching
        // shadow in the Shape branch — this rectangle would poke out around them.
        || (matches!(ctrl.control_type, CT::Shape)
            && matches!(
                ctrl.get_prop("ShapeType").map(|v| v.as_str()),
                Some("Circle" | "Ellipse" | "Triangle")
            ))
    {
        return None;
    }

    let shadow_color = ctrl
        .get_prop("ShadowColor")
        .map(|v| parse_color(v.as_str()))
        .unwrap_or(Color32::BLACK);
    let shadow_opac = ctrl
        .get_prop("ShadowOpacity")
        .map(|v| v.as_i64())
        .unwrap_or(20)
        .clamp(0, 100) as f32
        / 100.0;
    let shadow_dir = ctrl
        .get_prop("ShadowDirection")
        .map(|v| v.as_str().to_owned())
        .unwrap_or_else(|| "South".into());
    let distance = ctrl
        .get_prop("ShadowDistance")
        .map(|v| v.as_i64())
        .unwrap_or(7)
        .clamp(0, 60) as f32;
    let blur_enabled = ctrl
        .get_prop("ShadowBlur")
        .map(|v| v.as_bool())
        .unwrap_or(true);
    let signed_blur = if blur_enabled {
        ctrl.get_prop("ShadowBlurStrength")
            .map(|v| v.as_i64())
            .unwrap_or(8)
            .clamp(-20, 20)
    } else {
        0
    };

    let (ux, uy): (f32, f32) = match shadow_dir.as_str() {
        "North" => (0.0, -1.0),
        "NorthEast" => (0.707, -0.707),
        "East" => (1.0, 0.0),
        "SouthEast" => (0.707, 0.707),
        "South" => (0.0, 1.0),
        "SouthWest" => (-0.707, 0.707),
        "West" => (-1.0, 0.0),
        "NorthWest" => (-0.707, -0.707),
        _ => (0.0, 1.0),
    };

    Some(RegularDropShadow {
        rect: rect.translate(Vec2::new(ux * distance, uy * distance)),
        color: shadow_color,
        opacity: shadow_opac,
        blur_strength: signed_blur.unsigned_abs() as usize,
        corner_radius: drop_shadow_corner_radius(ctrl),
        overlay: signed_blur < 0,
    })
}

/// Closed outline path of a rounded rect, for dashed Shape outlines.
fn rounded_rect_outline_points(rect: Rect, r: f32) -> Vec<Pos2> {
    let r = r.clamp(0.0, 0.5 * rect.width().min(rect.height()));
    let seg = 6; // arc segments per corner
    let mut pts = Vec::with_capacity(4 * (seg + 1) + 1);
    // Clockwise from the top-right arc: NE, SE, SW, NW.
    let corners = [
        (Pos2::new(rect.max.x - r, rect.min.y + r), -90.0_f32),
        (Pos2::new(rect.max.x - r, rect.max.y - r), 0.0),
        (Pos2::new(rect.min.x + r, rect.max.y - r), 90.0),
        (Pos2::new(rect.min.x + r, rect.min.y + r), 180.0),
    ];
    for (c, start) in corners {
        for i in 0..=seg {
            let ang = (start + 90.0 * i as f32 / seg as f32).to_radians();
            pts.push(c + Vec2::new(ang.cos(), ang.sin()) * r);
        }
    }
    let first = pts[0];
    pts.push(first);
    pts
}

/// Property-driven drop shadow for non-rectangular Shape silhouettes
/// (Circle/Ellipse/Triangle). Rect-based shapes ride the shared regular /
/// Neumorphic shadow paths; a rectangle behind these silhouettes would poke
/// out, so the Shape branch calls this instead. Layer falloff matches
/// `draw_regular_drop_shadow`; Neumorphic styles get the dual (light + dark)
/// relief like `draw_glass_neumorphic`.
fn draw_shape_silhouette_shadow(
    painter: &egui::Painter,
    ctrl: &Control,
    rect: Rect,
    shape_type: &str,
    is_neumorphic: bool,
    alpha_mul: f32,
) {
    let enabled = ctrl
        .get_prop("ShadowEnabled")
        .map(|v| v.as_bool())
        .unwrap_or(is_neumorphic); // Neumorphic default: ON, like every control
    if !enabled || alpha_mul <= 0.0 {
        return;
    }
    let am = alpha_mul.clamp(0.0, 1.0);
    let shadow_color = ctrl
        .get_prop("ShadowColor")
        .map(|v| parse_color(v.as_str()))
        .unwrap_or(Color32::BLACK);
    let opacity = ctrl
        .get_prop("ShadowOpacity")
        .map(|v| v.as_i64())
        .unwrap_or(if is_neumorphic { 6 } else { 20 })
        .clamp(0, 100) as f32
        / 100.0;
    let dir = ctrl
        .get_prop("ShadowDirection")
        .map(|v| v.as_str().to_owned())
        .unwrap_or_else(|| if is_neumorphic { "SouthEast" } else { "South" }.into());
    let distance = ctrl
        .get_prop("ShadowDistance")
        .map(|v| v.as_i64())
        .unwrap_or(7)
        .clamp(0, 60) as f32;
    let blur_enabled = ctrl
        .get_prop("ShadowBlur")
        .map(|v| v.as_bool())
        .unwrap_or(true);
    let blur = if blur_enabled {
        ctrl.get_prop("ShadowBlurStrength")
            .map(|v| v.as_i64())
            .unwrap_or(8)
            .clamp(-20, 20)
            .unsigned_abs() as usize
    } else {
        0
    };
    let (ux, uy): (f32, f32) = match dir.as_str() {
        "North" => (0.0, -1.0),
        "NorthEast" => (0.707, -0.707),
        "East" => (1.0, 0.0),
        "SouthEast" => (0.707, 0.707),
        "South" => (0.0, 1.0),
        "SouthWest" => (-0.707, 0.707),
        "West" => (-1.0, 0.0),
        "NorthWest" => (-0.707, -0.707),
        _ => (0.0, 1.0),
    };
    let offset = Vec2::new(ux * distance, uy * distance);

    let circ_r = rect.width().min(rect.height()) / 2.0;
    let cc = rect.center();
    let tri = [
        Pos2::new(rect.center().x, rect.min.y),
        Pos2::new(rect.max.x, rect.max.y),
        Pos2::new(rect.min.x, rect.max.y),
    ];
    // The silhouette translated by `off` and grown by `expand`.
    let paint_sil = |off: Vec2, expand: f32, col: Color32| {
        if matches!(shape_type, "Circle" | "Ellipse") {
            painter.circle_filled(cc + off, circ_r + expand, col);
        } else {
            // Grow by scaling about the centroid — close enough for the soft
            // shadow layers of a triangle.
            let centroid = Pos2::new(
                (tri[0].x + tri[1].x + tri[2].x) / 3.0,
                (tri[0].y + tri[1].y + tri[2].y) / 3.0,
            );
            let k = 1.0 + expand / (0.5 * rect.width().min(rect.height())).max(1.0);
            let pts = tri
                .iter()
                .map(|p| centroid + (*p - centroid) * k + off)
                .collect::<Vec<_>>();
            painter.add(egui::Shape::convex_polygon(pts, col, Stroke::NONE));
        }
    };
    let tint = |c: Color32, a01: f32| {
        Color32::from_rgba_premultiplied(
            (c.r() as f32 * a01) as u8,
            (c.g() as f32 * a01) as u8,
            (c.b() as f32 * a01) as u8,
            (a01 * 255.0) as u8,
        )
    };
    // One layered soft shadow — same outer-to-core falloff as the rect path.
    let layered = |off: Vec2, col: Color32, max_opac: f32| {
        if blur == 0 {
            paint_sil(off, 0.0, tint(col, max_opac * am));
            return;
        }
        for i in 0..=blur {
            let t = 1.0 - (i as f32 / blur as f32);
            let falloff = (-3.0 * t * t).exp();
            paint_sil(off, t * blur as f32, tint(col, max_opac * am * falloff));
        }
    };
    if is_neumorphic {
        // Dual relief: light opposite the shadow direction, dark along it.
        let light = ctrl
            .get_prop("ShadowLightColor")
            .map(|v| parse_color(v.as_str()))
            .unwrap_or(Color32::WHITE);
        layered(-offset, light, (opacity * 3.25).clamp(0.0, 1.0));
        layered(offset, shadow_color, opacity);
    } else {
        layered(offset, shadow_color, opacity);
    }
}

fn draw_regular_drop_shadow(painter: &egui::Painter, shadow: &RegularDropShadow, alpha_mul: f32) {
    let sc = shadow.color;
    if shadow.blur_strength == 0 {
        let alpha = (shadow.opacity * alpha_mul * 255.0) as u8;
        painter.rect_filled(
            shadow.rect,
            shadow.corner_radius,
            Color32::from_rgba_premultiplied(
                (sc.r() as f32 * shadow.opacity * alpha_mul) as u8,
                (sc.g() as f32 * shadow.opacity * alpha_mul) as u8,
                (sc.b() as f32 * shadow.opacity * alpha_mul) as u8,
                alpha,
            ),
        );
        return;
    }

    // Draw from outermost (faintest) to innermost (darkest), so the painter's
    // back-to-front order gives the shadow a denser core.
    let layers = shadow.blur_strength;
    for i in 0..=layers {
        let t = 1.0 - (i as f32 / layers as f32);
        let expand = t * shadow.blur_strength as f32;
        let falloff = (-3.0 * t * t).exp();
        let alpha = (shadow.opacity * alpha_mul * falloff * 255.0) as u8;
        let layer_rect = shadow.rect.expand(expand);
        painter.rect_filled(
            layer_rect,
            shadow.corner_radius + expand,
            Color32::from_rgba_premultiplied(
                (sc.r() as f32 * (alpha as f32 / 255.0)) as u8,
                (sc.g() as f32 * (alpha as f32 / 255.0)) as u8,
                (sc.b() as f32 * (alpha as f32 / 255.0)) as u8,
                alpha,
            ),
        );
    }
}

/// Composite premultiplied `fg` over premultiplied `bg`.
///
/// egui stores [`Color32`] in premultiplied-alpha space. Rounded-container notch
/// masks need to repaint the *already visible* backdrop, not draw a translucent
/// colour a second time: repainting translucent form glass would double its alpha
/// and create darker wedges, while skipping it lets children bleed through. This
/// helper produces the single-pass effective colour to use for those masks.
pub fn composite_premultiplied_over(fg: Color32, bg: Color32) -> Color32 {
    let fa = fg.a() as u16;
    let inv = 255_u16.saturating_sub(fa);
    let over = |f: u8, b: u8| -> u8 { ((f as u16 + (b as u16 * inv + 127) / 255).min(255)) as u8 };
    Color32::from_rgba_premultiplied(
        over(fg.r(), bg.r()),
        over(fg.g(), bg.g()),
        over(fg.b(), bg.b()),
        over(fg.a(), bg.a()),
    )
}

/// WCAG relative luminance of a colour, 0.0 (black) … 1.0 (white).
pub fn relative_luminance(c: Color32) -> f32 {
    let lin = |v: u8| {
        let s = v as f32 / 255.0;
        if s <= 0.04045 {
            s / 12.92
        } else {
            ((s + 0.055) / 1.055).powf(2.4)
        }
    };
    0.2126 * lin(c.r()) + 0.7152 * lin(c.g()) + 0.0722 * lin(c.b())
}

/// WCAG contrast ratio between two colours, 1.0 (identical) … 21.0 (black on
/// white). AA text wants 4.5; a caret is a thin bar, so it needs at least as
/// much to stay findable.
pub fn contrast_ratio(a: Color32, b: Color32) -> f32 {
    let (la, lb) = (relative_luminance(a), relative_luminance(b));
    let (hi, lo) = if la >= lb { (la, lb) } else { (lb, la) };
    (hi + 0.05) / (lo + 0.05)
}

/// The BackgroundColor the developer explicitly chose for a control, if any.
/// The universal seeded default and the values the Neumorphic style appliers
/// stamp on every control all mean "not chosen" — the renderer-wide "still on
/// the default means the user has not picked" convention.
pub fn user_background_color(ctrl: &Control) -> Option<Color32> {
    ctrl.get_prop("BackgroundColor")
        .map(|v| v.as_str().to_owned())
        .filter(|raw| !raw.trim().is_empty())
        .map(|raw| parse_color(&raw))
        .filter(|c| c.a() > 0)
        .filter(|c| {
            [
                crate::model::DEFAULT_BACKGROUND_COLOR,
                crate::model::NEUMORPHIC_SURFACE_COLOR,
                crate::model::NEUMORPHIC_DARK_SURFACE_COLOR,
            ]
            .iter()
            .all(|default_hex| parse_color(default_hex) != *c)
        })
}

/// The tone a control's content actually sits on, as an opaque colour: the
/// developer's own BackgroundColor when set (composited over `under` if it is
/// translucent), otherwise the surface the active glass style paints — the
/// Neumorphic surfaces are solid, while Classic/Enhanced frost lets `under`
/// (the form's backdrop) show through. Used to pick colours that must stay
/// legible on the face, whatever theme and background the developer chose.
pub fn control_surface_tone(ctx: &egui::Context, ctrl: &Control, under: Color32) -> Color32 {
    let opaque_under = Color32::from_rgb(under.r(), under.g(), under.b());
    if let Some(c) = user_background_color(ctrl) {
        return composite_premultiplied_over(c, opaque_under);
    }
    match active_glass_style(ctx) {
        crate::model::GlassStyle::Neumorphic => parse_color(crate::model::NEUMORPHIC_SURFACE_COLOR),
        crate::model::GlassStyle::NeumorphicDark => {
            parse_color(crate::model::NEUMORPHIC_DARK_SURFACE_COLOR)
        }
        // Liquid Glass is a translucent frost: what the eye reads is mostly
        // whatever the form paints behind the field.
        crate::model::GlassStyle::Classic | crate::model::GlassStyle::Enhanced => {
            composite_premultiplied_over(Color32::from_white_alpha(38), opaque_under)
        }
    }
}

/// A text caret colour that is always legible in a field whose background is
/// `surface`: the field's own text colour while that already clears WCAG AA
/// (so the caret normally matches the text, as every desktop toolkit draws
/// it), and otherwise near-black or near-white — whichever the surface calls
/// for. egui's caret comes from the ambient visuals, which on a dark field
/// (or a dark form under Liquid Glass) left it dark-on-dark.
pub fn caret_color(surface: Color32, text: Color32) -> Color32 {
    const AA: f32 = 4.5;
    if contrast_ratio(text, surface) >= AA {
        return text;
    }
    // Otherwise the pole that reads better. It has to be pure black/white and
    // chosen by ratio, not by a luminance threshold: on a mid grey the near
    // poles fall short (white on #808080 is 3.5:1), while the better of pure
    // black and pure white clears AA on ANY colour — the worst possible
    // surface still yields ~4.6:1.
    if contrast_ratio(Color32::BLACK, surface) >= contrast_ratio(Color32::WHITE, surface) {
        Color32::BLACK
    } else {
        Color32::WHITE
    }
}

/// Decode the transient `_ContainerClip` prop the render engine seeds on a
/// PictureBox face that lives inside a rounded GroupBox/Panel. Returns the
/// container's screen-space **content** rect, its corner radius, and a per-corner
/// `[nw, ne, sw, se]` roundable flag. `None` when the prop is absent/malformed.
fn parse_container_clip(ctrl: &Control) -> Option<(egui::Rect, f32, [bool; 4])> {
    let v = ctrl.get_prop("_ContainerClip")?;
    let p: Vec<f32> = v
        .as_str()
        .split(',')
        .filter_map(|s| s.trim().parse().ok())
        .collect();
    if p.len() != 9 {
        return None;
    }
    let rect = egui::Rect::from_min_max(egui::pos2(p[0], p[1]), egui::pos2(p[2], p[3]));
    let flags = [p[5] != 0.0, p[6] != 0.0, p[7] != 0.0, p[8] != 0.0];
    Some((rect, p[4], flags))
}

/// Per-corner rounding for content clipped to its container border. A corner is
/// rounded with the container radius when the content reaches into that corner's
/// arc region — i.e. its visible edge comes within the radius of both container
/// edges meeting at that corner. This covers content that fills the container AND
/// content inset a few px from the border (which would otherwise keep its small own
/// radius and poke past the larger container arc). Otherwise the corner keeps the
/// control's own radius. The applied radius is clamped to half the visible rect.
fn container_image_rounding(
    visible: egui::Rect,
    border: egui::Rect,
    rad: f32,
    flags: [bool; 4],
    own: f32,
) -> egui::CornerRadius {
    let cap = 0.5 * visible.width().min(visible.height());
    let rr = |raw: f32| own.max(raw.max(0.0).min(cap));
    let chord_cut = |inset: f32| -> f32 {
        let d = (rad - inset.clamp(0.0, rad)).abs();
        rad - (rad * rad - d * d).max(0.0).sqrt()
    };

    // When a child is merely near a parent rounded corner, applying the full
    // parent radius to the child's own corner crops far too early. Calculate how
    // much of the parent's arc actually crosses the child's visible corner. The
    // returned radius is intentionally conservative for asymmetric insets: it may
    // under-approximate the parent arc slightly, but it will not erase pixels that
    // are still inside the parent's rounded border.
    let nw = || {
        if visible.min.x >= border.min.x + rad || visible.min.y >= border.min.y + rad {
            return own;
        }
        let inset_x = (visible.min.x - border.min.x).clamp(0.0, rad);
        let inset_y = (visible.min.y - border.min.y).clamp(0.0, rad);
        let dx = rad - inset_x;
        let dy = rad - inset_y;
        if dx * dx + dy * dy <= rad * rad {
            return own;
        }
        rr((chord_cut(inset_y) - inset_x).min(chord_cut(inset_x) - inset_y))
    };
    let ne = || {
        if visible.max.x <= border.max.x - rad || visible.min.y >= border.min.y + rad {
            return own;
        }
        let inset_x = (border.max.x - visible.max.x).clamp(0.0, rad);
        let inset_y = (visible.min.y - border.min.y).clamp(0.0, rad);
        let dx = rad - inset_x;
        let dy = rad - inset_y;
        if dx * dx + dy * dy <= rad * rad {
            return own;
        }
        rr((chord_cut(inset_y) - inset_x).min(chord_cut(inset_x) - inset_y))
    };
    let sw = || {
        if visible.min.x >= border.min.x + rad || visible.max.y <= border.max.y - rad {
            return own;
        }
        let inset_x = (visible.min.x - border.min.x).clamp(0.0, rad);
        let inset_y = (border.max.y - visible.max.y).clamp(0.0, rad);
        let dx = rad - inset_x;
        let dy = rad - inset_y;
        if dx * dx + dy * dy <= rad * rad {
            return own;
        }
        rr((chord_cut(inset_y) - inset_x).min(chord_cut(inset_x) - inset_y))
    };
    let se = || {
        if visible.max.x <= border.max.x - rad || visible.max.y <= border.max.y - rad {
            return own;
        }
        let inset_x = (border.max.x - visible.max.x).clamp(0.0, rad);
        let inset_y = (border.max.y - visible.max.y).clamp(0.0, rad);
        let dx = rad - inset_x;
        let dy = rad - inset_y;
        if dx * dx + dy * dy <= rad * rad {
            return own;
        }
        rr((chord_cut(inset_y) - inset_x).min(chord_cut(inset_x) - inset_y))
    };

    egui::CornerRadius {
        nw: if flags[0] {
            crate::paint::cr8(nw())
        } else {
            crate::paint::cr8(own)
        },
        ne: if flags[1] {
            crate::paint::cr8(ne())
        } else {
            crate::paint::cr8(own)
        },
        sw: if flags[2] {
            crate::paint::cr8(sw())
        } else {
            crate::paint::cr8(own)
        },
        se: if flags[3] {
            crate::paint::cr8(se())
        } else {
            crate::paint::cr8(own)
        },
    }
}

/// Apply `f` to each corner radius of a `Rounding`.
///
/// Every remaining caller derives radii for soft **fills** (shadow/glow
/// layers at `r + fractional expand`), where round-to-nearest is the best u8
/// approximation — flooring makes each layer systematically squarer, which
/// bands dark exactly on the corner diagonals. Concentric border STROKES no
/// longer come through here at all: they are painted with
/// `StrokeKind::Inside` at the exact integer face radius (egui>=0.31 cannot
/// express `face - half` in its u8 radii; see the spec-027 corner-bleed
/// post-mortem skill).
fn round_map(r: egui::CornerRadius, f: impl Fn(f32) -> f32) -> egui::CornerRadius {
    egui::CornerRadius {
        nw: cr8(f(f32::from(r.nw))),
        ne: cr8(f(f32::from(r.ne))),
        sw: cr8(f(f32::from(r.sw))),
        se: cr8(f(f32::from(r.se))),
    }
}

/// Runtime switch for the composite-frame diagnostics overlay. Enabled by setting
/// `COBOLT_FRAME_DIAGNOSTICS=1` (also accepts `true`/`on`) in the environment, so
/// the corner-bleed overlay can be turned on without a rebuild and never ships on
/// by default. Read once and cached.
/// Runtime state of the diagnostics overlay, tri-valued so the IDE's Project
/// Settings toggle can override the env var without a rebuild:
/// `0` = uninitialised (fall back to `COBOLT_FRAME_DIAGNOSTICS` on first read),
/// `1` = forced off, `2` = forced on.
static FRAME_DIAG: std::sync::atomic::AtomicU8 = std::sync::atomic::AtomicU8::new(0);

/// Turn the frame-diagnostics overlay on/off at runtime. The IDE calls this from
/// the Project Settings "Frame diagnostics" toggle (and each frame from the live
/// project setting), so the in-process design canvas responds without a rebuild
/// or an env var. Run-Form runs in a child process, which still honours
/// `COBOLT_FRAME_DIAGNOSTICS` — the IDE passes it through when launching.
///
/// An explicit call always wins over the env var: once set, the stored value is
/// authoritative.
pub fn set_frame_diagnostics(on: bool) {
    FRAME_DIAG.store(if on { 2 } else { 1 }, std::sync::atomic::Ordering::Relaxed);
}

/// `true` when the frame-diagnostics overlay is active (project setting or, until
/// the IDE overrides it, the `COBOLT_FRAME_DIAGNOSTICS` env var). Public so other
/// crates (e.g. the IDE's rounded-clip labelling) share this one source of truth
/// instead of re-reading the env.
pub fn frame_diagnostics_enabled() -> bool {
    diag_flag(&FRAME_DIAG, "COBOLT_FRAME_DIAGNOSTICS")
}

/// Runtime state of the DataGrid component-frame overlay (a diagnostic private to
/// the DataGrid: it outlines every internal sub-component — header, body, each
/// column, each visible row and cell, frozen panes, scrollbar — independently of
/// the global frame diagnostics). Tri-valued like [`FRAME_DIAG`].
static DATAGRID_DIAG: std::sync::atomic::AtomicU8 = std::sync::atomic::AtomicU8::new(0);

/// Turn the DataGrid component-frame overlay on/off at runtime. Mirrors
/// [`set_frame_diagnostics`]; the IDE drives it from the project setting.
pub fn set_datagrid_diagnostics(on: bool) {
    DATAGRID_DIAG.store(if on { 2 } else { 1 }, std::sync::atomic::Ordering::Relaxed);
}

/// `true` when the DataGrid component-frame overlay is active (project setting or,
/// until the IDE overrides it, the `COBOLT_DATAGRID_DIAGNOSTICS` env var).
pub fn datagrid_diagnostics_enabled() -> bool {
    diag_flag(&DATAGRID_DIAG, "COBOLT_DATAGRID_DIAGNOSTICS")
}

/// Shared tri-state reader for the diagnostic flags: `1` = off, `2` = on,
/// `0` = uninitialised → seed once from `env_var` (dev override) so a child
/// `rcrun run-form` process still lights up when it's exported.
fn diag_flag(cell: &std::sync::atomic::AtomicU8, env_var: &str) -> bool {
    use std::sync::atomic::Ordering;
    match cell.load(Ordering::Relaxed) {
        1 => false,
        2 => true,
        _ => {
            let on = std::env::var(env_var)
                .map(|v| {
                    let v = v.trim();
                    v == "1" || v.eq_ignore_ascii_case("true") || v.eq_ignore_ascii_case("on")
                })
                .unwrap_or(false);
            cell.store(if on { 2 } else { 1 }, Ordering::Relaxed);
            on
        }
    }
}

/// One distinct hue per composite layer ("slot"), so frames that overlap the same
/// rect stay individually identifiable in the diagnostics overlay.
fn debug_frame_color(slot: usize) -> Color32 {
    const PALETTE: [Color32; 6] = [
        Color32::from_rgb(255, 64, 64),  // 0 shadow  – red
        Color32::from_rgb(64, 220, 96),  // 1 face    – green
        Color32::from_rgb(80, 160, 255), // 2 border  – blue
        Color32::from_rgb(255, 200, 32), // 3 content – amber
        Color32::from_rgb(210, 96, 255), // 4 outline – magenta
        Color32::from_rgb(0, 220, 220),  // 5 spare   – cyan
    ];
    PALETTE[slot % PALETTE.len()]
}

/// Per-slot exploded offset for the diagnostics view: 60px LEFT and 60px DOWN for
/// every layer up the stack. All frames are otherwise painted at the same rect, so
/// they hide one another — fanning them out on a diagonal lets each be inspected in
/// isolation (its fill, its border, and its real corner rounding) side by side.
fn debug_frame_offset(slot: usize) -> Vec2 {
    let delta = 60.0 * slot as f32;
    Vec2::new(-delta, delta)
}

/// Diagnostics view for one composite frame that makes up a control (spec 017
/// rounded-corner bleed hunt). When `enabled`, it EXPLODES the layer out of the
/// stack — shifting `rect` 60px left + 60px down per slot — so the real fill/border
/// this call is about to paint lands on its own, no longer hidden behind the frames
/// above it. It also traces that shifted rect with the layer's **real** `rounding`
/// in the slot's colour on a top layer and tags it with `name`, then returns the
/// SHIFTED rect for the caller to draw into. When disabled, returns `rect` unchanged
/// so production geometry is byte-for-byte identical.
///
/// Fanned out this way the culprit is obvious: the layer whose corner stays square
/// while the ones above it round is exactly the wedge that bleeds into the notch.
fn debug_frame(
    painter: &egui::Painter,
    rect: egui::Rect,
    rounding: egui::CornerRadius,
    slot: usize,
    name: &str,
    enabled: bool,
) -> egui::Rect {
    if !enabled {
        return rect;
    }
    // Explode this layer out of the stack so it no longer hides / is hidden by the
    // other frames. The caller paints its real fill/border into the returned rect.
    let rect = rect.translate(debug_frame_offset(slot));
    let color = debug_frame_color(slot);
    // Foreground layer, unclipped: the overlay must sit above the control's own
    // fills and outside any content clip so every frame stays visible.
    let overlay = egui::Painter::new(
        painter.ctx().clone(),
        egui::LayerId::new(
            egui::Order::Foreground,
            egui::Id::new("cobolt_frame_diagnostics"),
        ),
        egui::Rect::EVERYTHING,
    );
    // The layer's true silhouette (its real rounding) at its exploded position, so a
    // square corner reads plainly against the rounded ones fanned alongside it.
    overlay.rect_stroke(
        rect,
        rounding,
        Stroke::new(1.25, color),
        egui::StrokeKind::Middle,
    );
    // Crosshairs on the four square (un-rounded) box corners — the exact leak points.
    let t = 4.0;
    for c in [
        rect.left_top(),
        rect.right_top(),
        rect.left_bottom(),
        rect.right_bottom(),
    ] {
        overlay.line_segment(
            [c - Vec2::new(t, 0.0), c + Vec2::new(t, 0.0)],
            Stroke::new(1.0, color),
        );
        overlay.line_segment(
            [c - Vec2::new(0.0, t), c + Vec2::new(0.0, t)],
            Stroke::new(1.0, color),
        );
    }
    // Name tag pinned to this layer's top-left corner.
    let tag = egui::Rect::from_min_size(
        rect.left_top() + Vec2::new(0.0, -13.0),
        Vec2::new(168.0, 12.0),
    );
    overlay.rect_filled(tag, 2.0, color);
    overlay.text(
        tag.left_center() + Vec2::new(4.0, 0.0),
        egui::Align2::LEFT_CENTER,
        format!("{slot}:{name}"),
        egui::FontId::monospace(9.0),
        Color32::BLACK,
    );
    rect
}

/// Append a triangle fan covering one rounded-corner **notch** — the slice of the
/// corner square that lies OUTSIDE the rounded arc — to `m`. `apex` is the square
/// (un-rounded) corner; the arc of radius `r` is centred at `center` and swept from
/// `t0` to `t1` radians. `uv_fn` maps a screen position to texture UV (`WHITE_UV`
/// for a solid fill). egui can't clip to a rounded rect, so we paint these notches
/// with what's behind the container to cut child bleed (spec 017).
fn push_notch_fan(
    m: &mut egui::epaint::Mesh,
    apex: egui::Pos2,
    center: egui::Pos2,
    r: f32,
    t0: f32,
    t1: f32,
    uv_fn: &dyn Fn(egui::Pos2) -> egui::Pos2,
    color_fn: &dyn Fn(egui::Pos2) -> Color32,
) {
    if r < 0.5 {
        return;
    }
    let segs = ((r as usize) / 2).clamp(6, 40);
    let base = m.vertices.len() as u32;
    m.vertices.push(egui::epaint::Vertex {
        pos: apex,
        uv: uv_fn(apex),
        color: color_fn(apex),
    });
    for i in 0..=segs {
        let t = t0 + (t1 - t0) * (i as f32 / segs as f32);
        let p = egui::pos2(center.x + r * t.cos(), center.y + r * t.sin());
        m.vertices.push(egui::epaint::Vertex {
            pos: p,
            uv: uv_fn(p),
            color: color_fn(p),
        });
    }
    for i in 0..segs as u32 {
        m.indices.extend([base, base + 1 + i, base + 2 + i]);
    }
}

/// Build the four corner-notch fans of `rect`/`rounding` into one mesh, colouring
/// each vertex via `uv_fn` (texture) + `color` (tint).
fn notch_mesh(
    rect: egui::Rect,
    rounding: egui::CornerRadius,
    uv_fn: &dyn Fn(egui::Pos2) -> egui::Pos2,
    color_fn: &dyn Fn(egui::Pos2) -> Color32,
) -> egui::epaint::Mesh {
    use std::f32::consts::PI;
    let mut m = egui::epaint::Mesh::default();
    let cap = 0.5 * rect.width().min(rect.height());
    let cl = |v: f32| v.max(0.0).min(cap);
    let (x0, y0, x1, y1) = (rect.min.x, rect.min.y, rect.max.x, rect.max.y);
    let nw = cl(f32::from(rounding.nw));
    let ne = cl(f32::from(rounding.ne));
    let sw = cl(f32::from(rounding.sw));
    let se = cl(f32::from(rounding.se));
    push_notch_fan(
        &mut m,
        egui::pos2(x0, y0),
        egui::pos2(x0 + nw, y0 + nw),
        nw,
        PI,
        1.5 * PI,
        uv_fn,
        color_fn,
    );
    push_notch_fan(
        &mut m,
        egui::pos2(x1, y0),
        egui::pos2(x1 - ne, y0 + ne),
        ne,
        1.5 * PI,
        2.0 * PI,
        uv_fn,
        color_fn,
    );
    push_notch_fan(
        &mut m,
        egui::pos2(x1, y1),
        egui::pos2(x1 - se, y1 - se),
        se,
        0.0,
        0.5 * PI,
        uv_fn,
        color_fn,
    );
    push_notch_fan(
        &mut m,
        egui::pos2(x0, y1),
        egui::pos2(x0 + sw, y1 - sw),
        sw,
        0.5 * PI,
        PI,
        uv_fn,
        color_fn,
    );
    m
}

/// Paint a rounded container's four corner notches with whatever sits BEHIND it,
/// so any child content that bled past the rounded corner (charts, grids, anything
/// egui's axis-aligned clip couldn't trim) is covered. `fill` is the solid backdrop
/// colour; `image` is an optional backdrop texture and the screen rect it's mapped
/// to (drawn on top of `fill`, matching how a form paints colour then image).
/// Spec 017 — the general fix for rounded-corner child bleed.
///
/// Pass an already-composited opaque `fill` when the underlying canvas/background
/// is translucent; otherwise repainting the translucent colour would double the
/// tint into a darker wedge.
pub fn draw_container_notch_mask(
    painter: &egui::Painter,
    rect: egui::Rect,
    rounding: egui::CornerRadius,
    fill: Color32,
    gradient: Option<(egui::Rect, Color32, Color32, &str)>,
    image: Option<(egui::TextureId, egui::Rect)>,
    img_alpha: u8,
) {
    if rounding.nw < crate::paint::cr8(0.5)
        && rounding.ne < crate::paint::cr8(0.5)
        && rounding.sw < crate::paint::cr8(0.5)
        && rounding.se < crate::paint::cr8(0.5)
    {
        return;
    }
    debug_frame(
        painter,
        rect,
        rounding,
        3,
        "CONTAINER_NOTCH_MASK",
        frame_diagnostics_enabled(),
    );
    let painter = painter.with_clip_rect(rect);
    if let Some((gradient_rect, start, end, direction)) = gradient {
        let color = |position| gradient_color_at(gradient_rect, start, end, direction, position);
        let m = notch_mesh(rect, rounding, &|_p| egui::epaint::WHITE_UV, &color);
        if !m.indices.is_empty() {
            painter.add(egui::Shape::mesh(m));
        }
    } else if fill.a() > 0 {
        let m = notch_mesh(rect, rounding, &|_p| egui::epaint::WHITE_UV, &|_p| fill);
        if !m.indices.is_empty() {
            painter.add(egui::Shape::mesh(m));
        }
    }
    if let Some((tex, dest)) = image {
        if img_alpha > 0 && dest.width() > 0.5 && dest.height() > 0.5 {
            let tint = Color32::from_white_alpha(img_alpha);
            let (dw, dh) = (dest.width(), dest.height());
            let uv = |p: egui::Pos2| egui::pos2((p.x - dest.min.x) / dw, (p.y - dest.min.y) / dh);
            let mut m = notch_mesh(rect, rounding, &uv, &|_p| tint);
            m.texture_id = tex;
            if !m.indices.is_empty() {
                painter.add(egui::Shape::mesh(m));
            }
        }
    }
}

/// Restore a rounded container's own outline on its four corner arcs after
/// [`draw_container_notch_mask`] repainted the backdrop over them. The notch mask
/// paints the backdrop right up to (and, with anti-aliased tessellation, over) the
/// corner edge, so a Panel/GroupBox otherwise loses its border/rim on every rounded
/// corner — the straight edges survive but the corners show the backdrop. This
/// redraws the same outline `draw_control` paints for a container — the glass rim
/// plus any explicit BorderColor/BorderWidth/BorderStyle border — clipped to each
/// corner square so the straight edges are not double-stroked.
pub fn restore_container_outline(
    painter: &egui::Painter,
    ctrl: &Control,
    rect: egui::Rect,
    radius: f32,
    glass: bool,
    masked: egui::CornerRadius,
) {
    if radius < 0.5 {
        return;
    }
    // Restore the rim ONLY on the corners the notch mask actually repainted
    // (`masked`, the per-corner rounding from `corner_notch_rounding`). The mask is
    // selective — it leaves clean corners (no child bled there) untouched — so
    // redrawing the rim on an unmasked corner double-strokes the face's own rim and
    // leaves a light spur/thickening at that corner (regressed when the per-corner
    // guardian replaced the old blanket all-corners mask). A corner whose masked
    // radius is ~0 was not repainted and must not be restored.
    let lo = crate::paint::cr8(0.5);
    let draw_nw = masked.nw >= lo;
    let draw_ne = masked.ne >= lo;
    let draw_se = masked.se >= lo;
    let draw_sw = masked.sw >= lo;
    if !(draw_nw || draw_ne || draw_se || draw_sw) {
        return;
    }
    debug_frame(
        painter,
        rect,
        egui::CornerRadius::same(crate::paint::cr8(radius)),
        4,
        "CONTAINER_RESTORE_OUTLINE",
        frame_diagnostics_enabled(),
    );
    let rnd = egui::CornerRadius::same(crate::paint::cr8(radius));

    // Neumorphic surfaces have no glass rim and no hard user border — their
    // edge accents (outer contour, inner bevel, tinted rim) live only on the
    // TR→BR→BL path. The rect-clipped notch mask can only have erased the parts
    // inside the rect at the two BOTTOM corners, so redraw the accents there,
    // clipped to those corner squares; the top corners carry no lines at all
    // (redrawing full-perimeter strokes here was re-introducing top/left arcs).

    // The explicit user border (Panel/GroupBox honour BorderColor/Width/Style).
    let border_style = ctrl
        .get_prop("BorderStyle")
        .map(|v| v.as_str().to_owned())
        .unwrap_or_else(|| "Single".into());
    let user_border_width = ctrl
        .get_prop("BorderWidth")
        .map(|v| v.as_i64() as f32)
        .unwrap_or(1.0);
    let user_border = (border_style != "None" && user_border_width > 0.5).then(|| {
        let (_, default_border, _) = control_colors(&ctrl.control_type, false);
        let bc = ctrl
            .get_prop("BorderColor")
            .map(|v| parse_color(v.as_str()))
            .unwrap_or(default_border);
        (user_border_width, bc)
    });

    // Draw the container outline (glass rim first, user border on top), matching
    // draw_control. Called once per corner, clipped to that corner's square.
    let draw_outline = |clip: egui::Rect| {
        let p = painter.with_clip_rect(clip);
        if glass {
            // Matches draw_glass's default Classic rim: 1.4px, white 170, inset by
            // half its width so it sits inside the rect like the original.
            let bw = 1.4_f32;
            let half = bw * 0.5;
            p.rect_stroke(
                rect,
                rnd,
                Stroke::new(bw, Color32::from_rgba_premultiplied(170, 170, 170, 170)),
                egui::StrokeKind::Inside,
            );
        }
        if let Some((bw, bc)) = user_border {
            let half = bw * 0.5;
            p.rect_stroke(rect, rnd, Stroke::new(bw, bc), egui::StrokeKind::Inside);
        }
    };

    let r = radius.min(rect.width() * 0.5).min(rect.height() * 0.5);
    // Corner squares in guardian order: NW, NE, SE, SW — paired with the matching
    // `masked` flag so only repainted corners are restored.
    let corners = [
        (draw_nw, egui::Rect::from_min_size(rect.min, egui::vec2(r, r))),
        (
            draw_ne,
            egui::Rect::from_min_size(egui::pos2(rect.max.x - r, rect.min.y), egui::vec2(r, r)),
        ),
        (
            draw_se,
            egui::Rect::from_min_size(
                egui::pos2(rect.max.x - r, rect.max.y - r),
                egui::vec2(r, r),
            ),
        ),
        (
            draw_sw,
            egui::Rect::from_min_size(egui::pos2(rect.min.x, rect.max.y - r), egui::vec2(r, r)),
        ),
    ];
    for (draw, clip) in corners {
        if draw {
            draw_outline(clip);
        }
    }
}

/// Per-corner rounding for a control's own rect: its uniform `own` radius, lifted
/// to the container radius on any corner that lands on the parent's rounded border
/// (when `ctrl` carries a `_ContainerClip`). Used for non-image fills/strokes that
/// must follow a container corner (e.g. the Animator placeholder/film) — spec 017.
fn control_border_rounding(ctrl: &Control, rect: egui::Rect, own: f32) -> egui::CornerRadius {
    if let Some((border, rad, flags)) = parse_container_clip(ctrl) {
        let visible = rect.intersect(border);
        container_image_rounding(visible, border, rad, flags, own)
    } else {
        egui::CornerRadius::same(crate::paint::cr8(own))
    }
}

/// Draw a texture into `rect` honouring `size_mode` (its native size), clipped to
/// the control rect and — when `ctrl` carries a `_ContainerClip` — to the parent
/// container's rounded BORDER path, so any overflow is cut by the container shape
/// instead of poking out square (spec 017). Shared by the PictureBox image and the
/// Animator frame so both clip identically on every surface. `own` is the control's
/// own corner radius, used only when it is free-standing (no container clip).
pub fn draw_media_image(
    painter: &egui::Painter,
    rect: egui::Rect,
    tex_id: egui::TextureId,
    native: Vec2,
    size_mode: &str,
    a: u8,
    ctrl: &Control,
    own: f32,
) {
    let dest = media_dest_rect(rect, native, size_mode);
    // The image keeps its own size; the visible area is `dest` trimmed to the
    // control rect (and the container border). We draw one textured rect over it
    // with the UV remapped from the full image, so the texture stays put.
    let mut visible = dest.intersect(rect);
    let mut rounding = egui::CornerRadius::same(crate::paint::cr8(own));
    if let Some((border, rad, flags)) = parse_container_clip(ctrl) {
        visible = visible.intersect(border);
        rounding = container_image_rounding(visible, border, rad, flags, own);
    }
    if visible.width() <= 0.5 || visible.height() <= 0.5 {
        return;
    }
    let dw = dest.width().max(1.0);
    let dh = dest.height().max(1.0);
    let uv = egui::Rect::from_min_max(
        egui::pos2(
            (visible.min.x - dest.min.x) / dw,
            (visible.min.y - dest.min.y) / dh,
        ),
        egui::pos2(
            (visible.max.x - dest.min.x) / dw,
            (visible.max.y - dest.min.y) / dh,
        ),
    );
    let tint = Color32::from_rgba_premultiplied(255, 255, 255, a);
    painter.with_clip_rect(rect).add(egui::Shape::Rect(
        egui::epaint::RectShape::new(
            visible,
            rounding,
            tint,
            Stroke::NONE,
            egui::StrokeKind::Middle,
        )
        .with_texture(tex_id, uv),
    ));
}

pub fn control_colors(ct: &ControlType, selected: bool) -> (Color32, Color32, Color32) {
    let border = if selected {
        Color32::from_rgb(60, 120, 230)
    } else {
        Color32::from_rgb(140, 140, 160)
    };
    match ct {
        ControlType::Button => (Color32::from_rgb(220, 220, 235), border, Color32::WHITE),
        ControlType::Label => (Color32::TRANSPARENT, border, Color32::WHITE),
        ControlType::TextBox => (Color32::WHITE, border, Color32::DARK_GRAY),
        ControlType::CheckBox | ControlType::RadioButton => {
            (Color32::TRANSPARENT, border, Color32::BLACK)
        }
        ControlType::GroupBox | ControlType::Panel => (
            Color32::from_rgba_premultiplied(200, 200, 210, 40),
            border,
            Color32::DARK_GRAY,
        ),
        ControlType::PictureBox => (Color32::from_rgb(180, 200, 220), border, Color32::DARK_GRAY),
        ControlType::DataGrid | ControlType::ListBox => {
            (Color32::WHITE, border, Color32::DARK_GRAY)
        }
        ControlType::MenuBar | ControlType::ToolBar | ControlType::StatusBar => {
            (Color32::from_rgb(200, 200, 215), border, Color32::BLACK)
        }
        ControlType::DateTimePicker | ControlType::NumericUpDown => {
            (Color32::WHITE, border, Color32::DARK_GRAY)
        }
        ControlType::TreeView => (Color32::WHITE, border, Color32::DARK_GRAY),
        ControlType::Splitter => (Color32::from_rgb(180, 180, 190), border, Color32::DARK_GRAY),
        ControlType::ComboBox => (Color32::WHITE, border, Color32::DARK_GRAY),
        ControlType::TabControl => (
            Color32::from_rgba_premultiplied(210, 215, 230, 120),
            border,
            Color32::BLACK,
        ),
        _ => (Color32::from_rgb(210, 210, 225), border, Color32::BLACK),
    }
}

pub fn ctrl_font_size(ctrl: &Control) -> f32 {
    ctrl.get_prop("FontSize")
        .map(|v| v.as_i64() as f32)
        .unwrap_or(11.0)
        .clamp(4.0, 200.0)
}

pub fn parse_color(s: &str) -> Color32 {
    let s = s.trim_start_matches('#');
    // 8-char RRGGBBAA — straight alpha
    if s.len() == 8 {
        if let (Ok(r), Ok(g), Ok(b), Ok(a)) = (
            u8::from_str_radix(&s[0..2], 16),
            u8::from_str_radix(&s[2..4], 16),
            u8::from_str_radix(&s[4..6], 16),
            u8::from_str_radix(&s[6..8], 16),
        ) {
            return Color32::from_rgba_unmultiplied(r, g, b, a);
        }
    }
    // 6-char RRGGBB — fully opaque
    if s.len() == 6 {
        if let (Ok(r), Ok(g), Ok(b)) = (
            u8::from_str_radix(&s[0..2], 16),
            u8::from_str_radix(&s[2..4], 16),
            u8::from_str_radix(&s[4..6], 16),
        ) {
            return Color32::from_rgb(r, g, b);
        }
    }
    Color32::TRANSPARENT
}

// ── 007 Form themes — active theme + 9-slice asset rendering ─────────────────
//
// The active asset-pack theme is stashed in egui's per-context temp storage so
// the (already 9-arg) `draw_control` needs no signature change: each surface
// (designer canvas, preview, run form, compiled binary) calls
// [`set_active_theme`] once before its control-draw loop. `None` ⇒ procedural
// Liquid Glass (the default), so existing forms are pixel-identical (R9).

#[derive(Clone)]
struct ActiveTheme(Option<Arc<ThemePack>>);

// ── Glass style context (per-frame, set once before the draw loop) ────────────

#[derive(Clone, Copy, Debug)]
pub struct NeumorphicShadowParams {
    pub shadow_on: bool,
    pub shadow_color: Color32,
    pub light_color: Color32,
    pub shadow_opac: f32,
    pub shadow_dir: [f32; 2],
    pub distance: f32,
    pub blur_strength: f32,
}

impl Default for NeumorphicShadowParams {
    fn default() -> Self {
        Self {
            shadow_on: true,
            shadow_color: Color32::from_rgb(0, 0, 255), // blue
            light_color: Color32::WHITE,
            shadow_opac: 0.07,          // 7%
            shadow_dir: [0.707, 0.707], // SouthEast
            distance: 6.0,
            blur_strength: 20.0,
        }
    }
}

fn neumorphic_params_id() -> egui::Id {
    egui::Id::new("cobolt-neumorphic-shadow-params")
}

fn glass_style_id() -> egui::Id {
    egui::Id::new("cobolt-active-glass-style")
}

/// Set the glass style for the current frame. Call once before the control-draw
/// loop on every rendering surface.
pub fn set_glass_style(ctx: &egui::Context, style: crate::model::GlassStyle) {
    ctx.data_mut(|d| d.insert_temp(glass_style_id(), style as u8));
}

/// Read the active glass style (defaults to Classic).
fn active_glass_style(ctx: &egui::Context) -> crate::model::GlassStyle {
    ctx.data(|d| d.get_temp::<u8>(glass_style_id()))
        .map(|v| match v {
            1 => crate::model::GlassStyle::Enhanced,
            2 => crate::model::GlassStyle::Neumorphic,
            3 => crate::model::GlassStyle::NeumorphicDark,
            _ => crate::model::GlassStyle::Classic,
        })
        .unwrap_or(crate::model::GlassStyle::Classic)
}

/// Dispatch to the correct glass renderer based on the active style.
pub fn draw_glass_auto(
    painter: &egui::Painter,
    rect: egui::Rect,
    base: Color32,
    rounding: impl Into<egui::CornerRadius>,
    selected: bool,
    alpha_mul: f32,
) {
    // Default glass: `base` tints the frost only, never a solid underlay.
    draw_glass_auto_bg(painter, rect, base, None, rounding, selected, alpha_mul);
}

/// Like [`draw_glass_auto`] but paints an explicit user-chosen background
/// (`bg_underlay`) as a solid, opacity-aware layer beneath the frost. Callers
/// pass `Some` only when the user actually set a BackgroundColor, so default
/// controls keep the fully translucent Liquid Glass look (spec 019).
/// The premultiplied fill for a Neumorphic surface at overall alpha `am`,
/// honouring the surface colour's OWN alpha.
///
/// The alpha used to be forced to `am * 255`, discarding `surface.a()`
/// entirely. A `BackgroundColor` of `#00000000` has all three channels at zero
/// and no alpha, so forcing it opaque painted a **solid black face** over the
/// control — the reported "no gradient at 100 % and it goes black". It also
/// meant a translucent background could never show anything through, because
/// the surface was always laid down fully opaque.
///
/// `Color32` already stores premultiplied channels, so the fade scales all four
/// components by the same factor — the only thing that was wrong was taking the
/// alpha from `255` instead of from the surface. An opaque surface is therefore
/// unaffected, which is why this changes nothing for a form that never set a
/// translucent background.
fn surface_fill(surface: Color32, am: f32) -> Color32 {
    let m = am.clamp(0.0, 1.0);
    Color32::from_rgba_premultiplied(
        (surface.r() as f32 * m) as u8,
        (surface.g() as f32 * m) as u8,
        (surface.b() as f32 * m) as u8,
        (surface.a() as f32 * m) as u8,
    )
}

pub fn draw_glass_auto_bg(
    painter: &egui::Painter,
    rect: egui::Rect,
    base: Color32,
    bg_underlay: Option<Color32>,
    rounding: impl Into<egui::CornerRadius>,
    selected: bool,
    alpha_mul: f32,
) {
    match active_glass_style(painter.ctx()) {
        crate::model::GlassStyle::Enhanced => {
            draw_glass_enhanced(
                painter,
                rect,
                base,
                bg_underlay,
                rounding,
                selected,
                alpha_mul,
            );
        }
        crate::model::GlassStyle::Neumorphic | crate::model::GlassStyle::NeumorphicDark => {
            draw_glass_neumorphic(
                painter,
                rect,
                base,
                bg_underlay,
                rounding,
                selected,
                alpha_mul,
            );
        }
        crate::model::GlassStyle::Classic => {
            draw_glass(
                painter,
                rect,
                base,
                bg_underlay,
                rounding,
                selected,
                alpha_mul,
            );
        }
    }
}

// ── Menu definition cache (per control ID, set by the designer) ──────────────

/// Store a loaded MenuDefinition in egui temp data so `draw_control` can read it.
pub fn set_menu_cache(
    ctx: &egui::Context,
    ctrl_id: &str,
    def: std::sync::Arc<crate::menu::MenuDefinition>,
) {
    let key = egui::Id::new("cobolt-menu-def").with(ctrl_id);
    ctx.data_mut(|d| d.insert_temp(key, def));
}

/// Retrieve the cached MenuDefinition for a control (if any).
pub fn get_menu_cache(
    ctx: &egui::Context,
    ctrl_id: &str,
) -> Option<std::sync::Arc<crate::menu::MenuDefinition>> {
    let key = egui::Id::new("cobolt-menu-def").with(ctrl_id);
    ctx.data(|d| d.get_temp::<std::sync::Arc<crate::menu::MenuDefinition>>(key))
}

fn active_theme_id() -> egui::Id {
    egui::Id::new("cobolt-active-theme-pack")
}

/// Set the asset-pack theme the next [`draw_control`] calls should skin with.
/// Pass `None` for procedural Liquid Glass. Call once per frame, before the
/// control-draw loop, on every rendering surface (R5).
pub fn set_active_theme(ctx: &egui::Context, pack: Option<Arc<ThemePack>>) {
    ctx.data_mut(|d| d.insert_temp(active_theme_id(), ActiveTheme(pack)));
}

/// Clear the active theme (revert to Liquid Glass).
pub fn clear_active_theme(ctx: &egui::Context) {
    ctx.data_mut(|d| d.insert_temp(active_theme_id(), ActiveTheme(None)));
}

fn active_theme(ctx: &egui::Context) -> Option<Arc<ThemePack>> {
    ctx.data(|d| d.get_temp::<ActiveTheme>(active_theme_id()))
        .and_then(|a| a.0)
}

#[derive(Clone, Default)]
struct ThemeTexCache(Arc<Mutex<HashMap<String, egui::TextureHandle>>>);

/// Load (and cache, per egui context) one of a theme pack's images as an egui
/// texture. The bytes come from the pack — a folder on disk under the IDE, or
/// the store embedded in a compiled binary — so every surface decodes the same
/// PNG into the same texture and paints it identically. Returns `None` when the
/// image is missing or undecodable, so the caller can fall back to glass (R11).
fn load_pack_texture(
    ctx: &egui::Context,
    pack: &ThemePack,
    rel: &str,
) -> Option<egui::TextureHandle> {
    let key = pack.asset_key(rel);
    let cache = ctx.data_mut(|d| {
        d.get_temp_mut_or_default::<ThemeTexCache>(egui::Id::new("cobolt-theme-tex"))
            .clone()
    });
    if let Some(h) = cache.0.lock().unwrap().get(&key) {
        return Some(h.clone());
    }
    let bytes = pack.asset_bytes(rel)?;
    let img = image::load_from_memory(&bytes).ok()?.to_rgba8();
    let (w, h) = img.dimensions();
    let color = egui::ColorImage::from_rgba_unmultiplied([w as usize, h as usize], img.as_raw());
    let handle = ctx.load_texture(&key, color, egui::TextureOptions::LINEAR);
    cache.0.lock().unwrap().insert(key, handle.clone());
    Some(handle)
}

/// Compute the nine `(dest_rect, uv_rect)` cell pairs for a 9-slice composite.
/// `tex` is the texture's pixel size; `slice` insets are `[l, t, r, b]` and are
/// clamped so opposite insets never exceed half the dest or the source. `uv`
/// rects are normalised to `0..1`. Corners keep their pixel size; edges and the
/// centre stretch (so the skin scales to any control size — R6).
pub fn nine_slice_cells(dst: Rect, tex: [f32; 2], slice: Slice) -> Vec<(Rect, Rect)> {
    let tw = tex[0].max(1.0);
    let th = tex[1].max(1.0);
    let maxx = (dst.width() * 0.5).min(tw * 0.5);
    let maxy = (dst.height() * 0.5).min(th * 0.5);
    let l = (slice[0] as f32).min(maxx).max(0.0);
    let t = (slice[1] as f32).min(maxy).max(0.0);
    let r = (slice[2] as f32).min(maxx).max(0.0);
    let b = (slice[3] as f32).min(maxy).max(0.0);

    let dxs = [dst.min.x, dst.min.x + l, dst.max.x - r, dst.max.x];
    let dys = [dst.min.y, dst.min.y + t, dst.max.y - b, dst.max.y];
    let sxs = [0.0, l, tw - r, tw];
    let sys = [0.0, t, th - b, th];

    let mut cells = Vec::with_capacity(9);
    for row in 0..3 {
        for col in 0..3 {
            let d = Rect::from_min_max(
                Pos2::new(dxs[col], dys[row]),
                Pos2::new(dxs[col + 1], dys[row + 1]),
            );
            let u = Rect::from_min_max(
                Pos2::new(sxs[col] / tw, sys[row] / th),
                Pos2::new(sxs[col + 1] / tw, sys[row + 1] / th),
            );
            cells.push((d, u));
        }
    }
    cells
}

/// 9-slice a texture into `dst`, tinted by `tint`.
fn draw_nine_slice(
    painter: &egui::Painter,
    dst: Rect,
    tex: &egui::TextureHandle,
    slice: Slice,
    tint: Color32,
) {
    let sz = tex.size_vec2();
    for (d, u) in nine_slice_cells(dst, [sz.x, sz.y], slice) {
        if d.width() > 0.5 && d.height() > 0.5 {
            painter.image(tex.id(), d, u, tint);
        }
    }
}

/// Paint the active asset-pack theme's background into `rect` when the form has
/// opted in (`use_theme_background`) and the pack provides one (R8). Returns
/// `true` if a themed background was drawn, so the caller skips the form's own
/// back-colour / background image; `false` otherwise.
pub fn draw_theme_background(
    painter: &egui::Painter,
    rect: Rect,
    use_theme_background: bool,
    alpha_mul: f32,
) -> bool {
    if !use_theme_background {
        return false;
    }
    let Some(pack) = active_theme(painter.ctx()) else {
        return false;
    };
    let Some(bg) = pack.manifest.background.as_ref() else {
        return false;
    };
    if bg.image.is_empty() {
        return false;
    }
    let Some(tex) = load_pack_texture(painter.ctx(), &pack, &bg.image) else {
        return false;
    };

    let a = (alpha_mul.clamp(0.0, 1.0) * 255.0) as u8;
    let tint = Color32::from_white_alpha(a);
    if bg.tile {
        let sz = tex.size_vec2().max(Vec2::new(1.0, 1.0));
        let mut y = rect.min.y;
        while y < rect.max.y {
            let mut x = rect.min.x;
            while x < rect.max.x {
                let cell = Rect::from_min_size(Pos2::new(x, y), sz).intersect(rect);
                let uv = Rect::from_min_max(
                    Pos2::new(0.0, 0.0),
                    Pos2::new(
                        (cell.width() / sz.x).min(1.0),
                        (cell.height() / sz.y).min(1.0),
                    ),
                );
                painter.image(tex.id(), cell, uv, tint);
                x += sz.x;
            }
            y += sz.y;
        }
    } else {
        let uv = Rect::from_min_max(Pos2::new(0.0, 0.0), Pos2::new(1.0, 1.0));
        painter.image(tex.id(), rect, uv, tint);
    }
    true
}

/// Map a control type to its asset-pack manifest key (lowercase). An empty
/// string means "no skin key" → always Liquid Glass.
fn control_kind_key(ct: &ControlType) -> &'static str {
    use ControlType as CT;
    match ct {
        CT::Button => "button",
        CT::Panel => "panel",
        CT::GroupBox => "groupbox",
        CT::TextBox => "textbox",
        CT::ComboBox => "combobox",
        CT::ListBox => "listbox",
        CT::CheckBox => "checkbox",
        CT::RadioButton => "radiobutton",
        CT::DataGrid => "datagrid",
        CT::Slider => "slider",
        CT::ProgressBar => "progressbar",
        CT::TabControl => "tabcontrol",
        CT::DateTimePicker => "datetimepicker",
        CT::NumericUpDown => "numericupdown",
        CT::TreeView => "treeview",
        CT::Splitter => "splitter",
        CT::MenuBar => "menubar",
        CT::ToolBar => "toolbar",
        CT::StatusBar => "statusbar",
        CT::PictureBox => "picturebox",
        _ => "",
    }
}

#[cfg(test)]
mod surface_fill_tests {
    use super::*;

    /// The reported bug. `#00000000` is "no background": all three channels
    /// zero and no alpha. Forcing the alpha opaque turned that into a solid
    /// black face over the control.
    #[test]
    fn a_fully_transparent_background_paints_nothing_not_black() {
        let clear = Color32::from_rgba_unmultiplied(0, 0, 0, 0);
        let out = surface_fill(clear, 1.0);
        assert_eq!(out.a(), 0, "a transparent surface must stay transparent");
        assert_eq!(out, Color32::TRANSPARENT, "and must not paint black: {out:?}");
    }

    /// Any colour at zero alpha is "not painted", whatever its channels say —
    /// a transparent white must not come out as a white face either.
    #[test]
    fn zero_alpha_hides_the_surface_whatever_its_channels() {
        for rgb in [(0, 0, 0), (255, 255, 255), (200, 30, 90)] {
            let c = Color32::from_rgba_unmultiplied(rgb.0, rgb.1, rgb.2, 0);
            assert_eq!(surface_fill(c, 1.0).a(), 0, "{rgb:?} leaked through");
        }
    }

    /// The overwhelmingly common case — an opaque background — must render
    /// exactly as before, or this fix would restyle every existing form.
    #[test]
    fn an_opaque_surface_is_unchanged() {
        let solid = Color32::from_rgb(232, 237, 254);
        let out = surface_fill(solid, 1.0);
        assert_eq!((out.r(), out.g(), out.b(), out.a()), (232, 237, 254, 255));
    }

    /// A half-transparent surface shows half of what is behind it, and its
    /// channels are premultiplied to match — egui expects premultiplied colour,
    /// so scaling alpha alone would render it too bright.
    #[test]
    fn a_translucent_surface_is_premultiplied() {
        let half = Color32::from_rgba_unmultiplied(200, 100, 50, 128);
        let out = surface_fill(half, 1.0);
        assert!((out.a() as i32 - 128).abs() <= 1, "alpha {}", out.a());
        assert!((out.r() as i32 - 100).abs() <= 2, "red {}", out.r());
        assert!((out.g() as i32 - 50).abs() <= 2, "green {}", out.g());
    }

    /// The control's own fade still applies on top, and the two compose: a
    /// half-faded control with a half-transparent background shows a quarter.
    #[test]
    fn the_controls_fade_composes_with_the_surface_alpha() {
        let solid = Color32::from_rgb(255, 255, 255);
        assert!((surface_fill(solid, 0.5).a() as i32 - 127).abs() <= 1);

        let half = Color32::from_rgba_unmultiplied(255, 255, 255, 128);
        let out = surface_fill(half, 0.5);
        assert!((out.a() as i32 - 64).abs() <= 2, "alpha {}", out.a());
    }

    /// A fully faded control paints nothing at all.
    #[test]
    fn zero_alpha_mul_paints_nothing() {
        let solid = Color32::from_rgb(10, 20, 30);
        assert_eq!(surface_fill(solid, 0.0).a(), 0);
    }
}

#[cfg(test)]
mod theme_render_tests {
    use super::*;

    /// Alignment strings map to egui aligns; unknown/empty values keep the
    /// historical defaults (Left horizontally, Middle vertically) so forms
    /// that predate the properties are unchanged. "Justified" is left-anchored
    /// with the justify flag reported separately.
    #[test]
    fn alignment_strings_map_with_backward_compatible_defaults() {
        assert_eq!(text_halign("Left"), egui::Align::LEFT);
        assert_eq!(text_halign("Center"), egui::Align::Center);
        assert_eq!(text_halign("Right"), egui::Align::RIGHT);
        assert_eq!(text_halign("Justified"), egui::Align::LEFT);
        assert_eq!(text_halign(""), egui::Align::LEFT);
        assert!(text_justified("Justified"));
        assert!(text_justified(" justified "));
        assert!(!text_justified("Left"));
        assert_eq!(text_valign("Top"), egui::Align::TOP);
        assert_eq!(text_valign("Middle"), egui::Align::Center);
        assert_eq!(text_valign("Bottom"), egui::Align::BOTTOM);
        assert_eq!(text_valign(""), egui::Align::Center);
        assert_eq!(text_valign("nonsense"), egui::Align::Center);
    }

    #[test]
    fn nine_slice_produces_nine_cells_covering_dest() {
        let dst = Rect::from_min_max(Pos2::new(10.0, 20.0), Pos2::new(110.0, 70.0)); // 100×50
        let cells = nine_slice_cells(dst, [64.0, 64.0], [12, 12, 12, 12]);
        assert_eq!(cells.len(), 9);
        // First cell is the top-left corner at the dest origin.
        assert_eq!(cells[0].0.min, dst.min);
        // Last cell is the bottom-right corner ending at the dest max.
        assert_eq!(cells[8].0.max, dst.max);
        // uv stays within 0..1.
        for (_d, u) in &cells {
            assert!(u.min.x >= 0.0 && u.max.x <= 1.0 && u.min.y >= 0.0 && u.max.y <= 1.0);
        }
    }

    #[test]
    fn nine_slice_clamps_oversized_insets() {
        // Insets larger than half the dest must not produce inverted rects.
        let dst = Rect::from_min_max(Pos2::new(0.0, 0.0), Pos2::new(20.0, 20.0));
        let cells = nine_slice_cells(dst, [64.0, 64.0], [40, 40, 40, 40]);
        for (d, _u) in &cells {
            assert!(
                d.width() >= 0.0 && d.height() >= 0.0,
                "no inverted dest rect"
            );
        }
    }

    #[test]
    fn tabcontrol_tab_rects_obey_tab_position() {
        let mut ctrl = Control::new("Tabs", ControlType::TabControl, 0, 0);
        ctrl.rect.w = 300;
        ctrl.rect.h = 200;
        ctrl.set_prop("Tabs", PropValue::String("Tab1\nTab2".into()));

        ctrl.set_prop("TabPosition", PropValue::String("Top".into()));
        let top = tabcontrol_tab_rects(Pos2::ZERO, &ctrl);
        assert_eq!(top[0].min, Pos2::new(0.0, 0.0));

        ctrl.set_prop("TabPosition", PropValue::String("Bottom".into()));
        let bottom = tabcontrol_tab_rects(Pos2::ZERO, &ctrl);
        assert_eq!(bottom[0].min.y, 174.0);

        ctrl.set_prop("TabPosition", PropValue::String("Left".into()));
        let left = tabcontrol_tab_rects(Pos2::ZERO, &ctrl);
        assert_eq!(left[0].min, Pos2::new(0.0, 0.0));
        assert!(left[0].width() > left[0].height());
        assert_eq!(left[1].min.y, 33.0);

        ctrl.set_prop("TabPosition", PropValue::String("Right".into()));
        let right = tabcontrol_tab_rects(Pos2::ZERO, &ctrl);
        assert!(right[0].min.x > 200.0);
        assert_eq!(right[0].min.y, 0.0);
    }

    #[test]
    fn tabcontrol_page_rect_reserves_navbar_space() {
        let mut ctrl = Control::new("Tabs", ControlType::TabControl, 0, 0);
        ctrl.rect.w = 300;
        ctrl.rect.h = 200;
        let rect = Rect::from_min_size(Pos2::ZERO, Vec2::new(300.0, 200.0));

        ctrl.set_prop("TabPosition", PropValue::String("Top".into()));
        assert_eq!(tabcontrol_page_rect(rect, &ctrl).min.y, 33.0);

        ctrl.set_prop("TabPosition", PropValue::String("Bottom".into()));
        assert_eq!(tabcontrol_page_rect(rect, &ctrl).max.y, 167.0);

        ctrl.set_prop("TabPosition", PropValue::String("Left".into()));
        assert!(tabcontrol_page_rect(rect, &ctrl).min.x > 0.0);

        ctrl.set_prop("TabPosition", PropValue::String("Right".into()));
        assert!(tabcontrol_page_rect(rect, &ctrl).max.x < 300.0);
    }

    #[test]
    fn svg_image_bytes_decode_for_picturebox_loader() {
        let svg =
            br##"<svg xmlns="http://www.w3.org/2000/svg" width="12" height="8" viewBox="0 0 12 8">
            <rect width="12" height="8" fill="#ff0000"/>
        </svg>"##;
        let image = decode_image_bytes("sample.svg", svg).expect("svg should rasterize");

        assert_eq!(image.size, [12, 8]);
        assert!(
            image.pixels.iter().any(|p| p.r() > 200 && p.a() > 200),
            "rasterized svg should contain the filled rectangle"
        );
    }

    #[test]
    fn svg_image_bytes_decode_at_destination_size() {
        let svg =
            br##"<svg xmlns="http://www.w3.org/2000/svg" width="12" height="8" viewBox="0 0 12 8">
            <circle cx="6" cy="4" r="3" fill="#000000"/>
        </svg>"##;
        let image = decode_svg_bytes_at_size(svg, 240, 160).expect("svg should scale as vector");

        assert_eq!(image.size, [240, 160]);
        assert!(
            image.pixels.iter().any(|p| p.a() > 200),
            "scaled svg should render visible pixels at the requested size"
        );
    }

    #[test]
    fn svg_picturebox_texture_uses_requested_destination_size() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("button-icon.svg");
        std::fs::write(
            &path,
            r##"<svg xmlns="http://www.w3.org/2000/svg" width="12" height="8" viewBox="0 0 12 8">
                <circle cx="6" cy="4" r="3" fill="#000000"/>
            </svg>"##,
        )
        .expect("write svg");

        let ctx = egui::Context::default();
        let tex = picturebox_svg_texture(
            &ctx,
            path.to_str().expect("utf8 path"),
            Vec2::new(48.0, 32.0),
        )
        .expect("svg texture at requested size");

        assert_eq!(tex.size(), [48, 32]);
    }

    #[test]
    fn svg_icc_color_fallbacks_are_stripped_before_parse() {
        let svg = r##"<svg xmlns="http://www.w3.org/2000/svg" width="4" height="4">
            <rect width="4" height="4" fill="#000000 icc-color(sRGB-IEC61966-2, 0.1, 0, 0, 0)"/>
        </svg>"##;
        let cleaned = strip_svg_icc_color_fallbacks(svg);

        assert!(cleaned.contains("fill=\"#000000\""));
        assert!(!cleaned.contains("icc-color("));
        assert!(decode_image_bytes("icc.svg", cleaned.as_bytes()).is_some());
    }

    #[test]
    fn control_kind_key_maps_core_controls() {
        assert_eq!(control_kind_key(&ControlType::Button), "button");
        assert_eq!(control_kind_key(&ControlType::Panel), "panel");
        // Line has no skin key → always glass (fallback).
        assert_eq!(control_kind_key(&ControlType::Line), "");
    }

    // ── Monochrome palette (spec 013) ──────────────────────────────────────────

    fn is_extreme(c: Color32) -> bool {
        let near0 = c.r() < 12 && c.g() < 12 && c.b() < 12;
        let near255 = c.r() > 243 && c.g() > 243 && c.b() > 243;
        near0 || near255
    }

    #[test]
    fn monochrome_palette_distinct_in_hue_no_extremes() {
        let base = Color32::from_rgb(0x3F, 0x6F, 0xB5);
        let (bh, _, _) = rgb_to_hsl(base);
        let pal = monochrome_palette(base, 5);
        assert_eq!(pal.len(), 5);
        for &c in &pal {
            assert!(!is_extreme(c), "tone too close to black/white: {c:?}");
            let (h, _, _) = rgb_to_hsl(c);
            let dh = (h - bh).abs().min(360.0 - (h - bh).abs());
            assert!(dh < 25.0, "tone left the base hue family: {dh} deg");
        }
        // Adjacent tones must be distinguishable (lightness differs).
        for w in pal.windows(2) {
            let l0 = rgb_to_hsl(w[0]).2;
            let l1 = rgb_to_hsl(w[1]).2;
            assert!((l0 - l1).abs() > 0.04, "adjacent tones not distinguishable");
        }
    }

    #[test]
    fn chart_palette_256_is_256_unique_no_black_white() {
        let pal = chart_palette_256();
        assert_eq!(pal.len(), 256);
        let uniq: std::collections::HashSet<(u8, u8, u8)> =
            pal.iter().map(|c| (c.r(), c.g(), c.b())).collect();
        assert_eq!(uniq.len(), 256, "256 colours must be unique");
        for &c in &pal {
            assert_ne!(
                (c.r(), c.g(), c.b()),
                (0, 0, 0),
                "pure black must be excluded"
            );
            assert_ne!(
                (c.r(), c.g(), c.b()),
                (255, 255, 255),
                "pure white must be excluded"
            );
            assert!(!is_extreme(c), "swatch too close to an extreme: {c:?}");
        }
    }

    #[test]
    fn catmull_rom_smooths_and_keeps_endpoints() {
        let pts = vec![
            Pos2::new(0.0, 0.0),
            Pos2::new(10.0, 20.0),
            Pos2::new(20.0, 5.0),
            Pos2::new(30.0, 25.0),
        ];
        let sm = catmull_rom(&pts, 12);
        assert!(
            sm.len() > pts.len(),
            "smoothing should add intermediate points"
        );
        assert_eq!(sm.first().copied(), Some(pts[0]), "keeps first point");
        assert_eq!(
            sm.last().copied(),
            Some(*pts.last().unwrap()),
            "keeps last point"
        );
        // Fewer than 3 points → unchanged.
        let two = vec![Pos2::new(0.0, 0.0), Pos2::new(1.0, 1.0)];
        assert_eq!(catmull_rom(&two, 12).len(), 2);
    }

    #[test]
    fn shade_lightens_and_darkens() {
        let base = Color32::from_rgb(0x3F, 0x6F, 0xB5);
        let bl = rgb_to_hsl(base).2;
        assert!(rgb_to_hsl(shade(base, 0.2)).2 > bl);
        assert!(rgb_to_hsl(shade(base, -0.2)).2 < bl);
    }

    #[test]
    fn grad_dir_mesh_endpoints_per_direction() {
        let rect = egui::Rect::from_min_max(Pos2::new(0.0, 0.0), Pos2::new(100.0, 50.0));
        let a = Color32::from_rgb(240, 240, 240);
        let b = Color32::from_rgb(40, 60, 90);
        // Linear aliases and compass directions carry start/end on opposite edges.
        for dir in [
            "North",
            "NorthEast",
            "East",
            "SouthEast",
            "South",
            "SouthWest",
            "West",
            "NorthWest",
            "Vertical",
            "Horizontal",
            "DiagonalDown",
            "DiagonalUp",
        ] {
            let m = grad_dir_mesh(rect, a, b, dir);
            let has_start = m.vertices.iter().any(|v| v.color == a);
            let has_end = m.vertices.iter().any(|v| v.color == b);
            assert!(
                has_start && has_end,
                "{dir} must carry both endpoint colours"
            );
        }
        // South: top edge = start, bottom edge = end.
        assert_eq!(gradient_color_at(rect, a, b, "South", rect.center_top()), a);
        assert_eq!(
            gradient_color_at(rect, a, b, "South", rect.center_bottom()),
            b
        );
        // East: left edge = start, right edge = end.
        assert_eq!(gradient_color_at(rect, a, b, "East", rect.left_center()), a);
        assert_eq!(
            gradient_color_at(rect, a, b, "East", rect.right_center()),
            b
        );
        // Radial: centre = start, all perimeter = end (fan = 1 + 8 verts).
        let r = grad_dir_mesh(rect, a, b, "Radial");
        assert!(r.vertices.len() >= 9);
        assert_eq!(r.vertices[0].color, a, "radial centre is the start colour");
        assert!(
            r.vertices[1..].iter().all(|v| v.color == b),
            "radial rim is the end colour"
        );
    }

    #[test]
    fn corner_radius_reads_alias_default_and_clamps() {
        use crate::model::{Control, ControlType, PropValue, Rect};
        let big = |t| {
            let mut c = Control::new("C", t, 0, 0);
            c.rect = Rect::new(0, 0, 200, 100);
            c
        };
        // Per-type defaults when no property is set.
        assert_eq!(corner_radius(&big(ControlType::TextBox)), 0.0);
        assert_eq!(corner_radius(&big(ControlType::Button)), 3.0);
        assert_eq!(corner_radius(&big(ControlType::BarChart)), 8.0);
        // Canonical CornerRadius is read.
        let mut c = big(ControlType::TextBox);
        c.set_prop("CornerRadius", PropValue::Int(20));
        assert_eq!(corner_radius(&c), 20.0);
        // Legacy BorderRadius is read as an alias when CornerRadius is absent
        // (an old .cfrm clears defaults and carries only BorderRadius).
        let mut c = big(ControlType::Panel);
        c.properties.shift_remove("CornerRadius");
        c.set_prop("BorderRadius", PropValue::Int(15));
        assert_eq!(corner_radius(&c), 15.0);
        // CornerRadius wins over BorderRadius when both are present.
        c.set_prop("CornerRadius", PropValue::Int(7));
        assert_eq!(corner_radius(&c), 7.0);
        // Clamp to half the smaller side (24×24 ⇒ max 12).
        let mut s = Control::new("S", ControlType::TextBox, 0, 0);
        s.rect = Rect::new(0, 0, 24, 24);
        s.set_prop("CornerRadius", PropValue::Int(40));
        assert_eq!(corner_radius(&s), 12.0);
        // Zero stays zero.
        let mut z = big(ControlType::Button);
        z.set_prop("CornerRadius", PropValue::Int(0));
        assert_eq!(corner_radius(&z), 0.0);
    }

    #[test]
    /// Operator rule (2026-07-30): the TextBox caret must ALWAYS read against
    /// the field it sits in. It keeps the text colour while that already
    /// clears WCAG AA, and flips to near-black / near-white when it would not.
    #[test]
    fn caret_always_contrasts_with_the_field() {
        let dark_field = Color32::from_rgb(24, 26, 40);
        let light_field = Color32::from_rgb(240, 240, 240);
        // Dark text in a dark field would vanish — the caret goes light.
        let caret = caret_color(dark_field, Color32::from_rgb(40, 40, 40));
        assert!(
            contrast_ratio(caret, dark_field) >= 4.5,
            "caret {caret:?} unreadable on a dark field"
        );
        assert!(relative_luminance(caret) > 0.5, "dark field ⇒ light caret");
        // Light text in a light field flips the other way.
        let caret = caret_color(light_field, Color32::from_rgb(225, 225, 225));
        assert!(contrast_ratio(caret, light_field) >= 4.5);
        assert!(relative_luminance(caret) < 0.5, "light field ⇒ dark caret");
        // Text that already reads is kept, so the caret matches the text as
        // every desktop toolkit draws it.
        let text = Color32::from_rgb(20, 20, 20);
        assert_eq!(caret_color(light_field, text), text);
        // Whatever the field, the result always clears AA.
        for bg in [
            Color32::BLACK,
            Color32::WHITE,
            Color32::from_rgb(128, 128, 128),
            Color32::from_rgb(20, 22, 45),
            parse_color(crate::model::NEUMORPHIC_DARK_SURFACE_COLOR),
            parse_color(crate::model::NEUMORPHIC_SURFACE_COLOR),
        ] {
            for text in [Color32::BLACK, Color32::WHITE, Color32::DARK_GRAY] {
                let c = caret_color(bg, text);
                assert!(
                    contrast_ratio(c, bg) >= 4.5,
                    "caret {c:?} on {bg:?} is only {:.1}:1",
                    contrast_ratio(c, bg)
                );
            }
        }
        println!(
            "caret: dark field ⇒ {:?}, light field ⇒ {:?}, AA held on every sampled field",
            caret_color(dark_field, Color32::from_rgb(40, 40, 40)),
            caret_color(light_field, Color32::from_rgb(225, 225, 225))
        );
    }

    #[test]
    fn textbox_inner_padding_defaults_and_clamps() {
        use crate::model::{Control, ControlType, PropValue};

        let mut c = Control::new("Text1", ControlType::TextBox, 0, 0);
        assert_eq!(textbox_inner_padding(&c), 3.0);

        c.set_prop("InnerPadding", PropValue::Int(24));
        assert_eq!(textbox_inner_padding(&c), 24.0);

        c.set_prop("InnerPadding", PropValue::Int(-8));
        assert_eq!(textbox_inner_padding(&c), 0.0);

        c.set_prop("InnerPadding", PropValue::Int(500));
        assert_eq!(textbox_inner_padding(&c), 128.0);
    }

    #[test]
    fn button_icon_alignment_aliases_padding_and_size_default() {
        use crate::model::{Control, ControlType};

        let c = Control::new("Button1", ControlType::Button, 0, 0);
        assert_eq!(button_image_padding(&c), 10.0);
        assert_eq!(button_icon_size(&c), 32.0);
        assert_eq!(
            button_image_alignment("MiddleRight"),
            ButtonImageAlignment::Right
        );
        assert_eq!(button_image_alignment("TopLeft"), ButtonImageAlignment::Top);
        assert_eq!(
            button_image_alignment("BottomLeft"),
            ButtonImageAlignment::Bottom
        );
        assert_eq!(button_image_alignment("Left"), ButtonImageAlignment::Left);

        let mut large = c.clone();
        large.set_prop("IconSize", PropValue::String("96".into()));
        assert_eq!(button_image_slot(&large), Vec2::new(96.0, 96.0));
        assert_eq!(
            button_image_size(Vec2::new(12.0, 200.0), button_image_slot(&large)),
            Vec2::new(96.0, 96.0)
        );
    }

    #[test]
    fn button_content_layout_places_image_around_text() {
        let rect = egui::Rect::from_min_size(Pos2::ZERO, Vec2::new(160.0, 60.0));
        let text = Vec2::new(48.0, 16.0);
        let image = Some(Vec2::new(20.0, 20.0));

        let (left_text, left_img) = button_content_layout(
            rect,
            text,
            image,
            ButtonImageAlignment::Left,
            10.0,
            "MiddleCenter",
        );
        let left_img = left_img.expect("left image rect");
        assert_eq!(left_text.x, left_img.right() + 10.0);
        assert!(
            (left_text.y + text.y * 0.5 - left_img.center().y).abs() < 0.1,
            "left image and text should be vertically centered"
        );

        let (right_text, right_img) = button_content_layout(
            rect,
            text,
            image,
            ButtonImageAlignment::Right,
            10.0,
            "MiddleCenter",
        );
        let right_img = right_img.expect("right image rect");
        assert_eq!(right_img.left(), right_text.x + text.x + 10.0);

        let (right_zero_text, right_zero_img) = button_content_layout(
            rect,
            text,
            image,
            ButtonImageAlignment::Right,
            0.0,
            "MiddleLeft",
        );
        let right_zero_img = right_zero_img.expect("right image rect with zero padding");
        assert_eq!(right_zero_text.x, rect.left() + 6.0);
        assert_eq!(right_zero_img.left(), right_zero_text.x + text.x);

        let tight_rect = egui::Rect::from_min_size(Pos2::ZERO, Vec2::new(90.0, 32.0));
        let (tight_text, tight_img) = button_content_layout(
            tight_rect,
            text,
            Some(Vec2::new(80.0, 80.0)),
            ButtonImageAlignment::Left,
            10.0,
            "MiddleCenter",
        );
        let tight_img = tight_img.expect("tight left image rect");
        assert_eq!(tight_text.x, tight_img.right() + 10.0);
        assert!(
            tight_img.left() >= tight_rect.left() + 6.0
                && tight_text.x + text.x <= tight_rect.right() - 6.0,
            "left icon/text block must stay inside the button without overlap"
        );

        let (top_text, top_img) = button_content_layout(
            rect,
            text,
            image,
            ButtonImageAlignment::Top,
            10.0,
            "MiddleRight",
        );
        let top_img = top_img.expect("top image rect");
        assert!((top_img.center().x - rect.center().x).abs() < 0.1);
        assert_eq!(top_img.top(), 6.0);
        assert_eq!(top_text.x, rect.right() - 6.0 - text.x);
        assert!(top_text.y >= top_img.bottom() + 10.0);

        let (top_zero_text, top_zero_img) =
            button_content_layout(rect, text, image, ButtonImageAlignment::Top, 0.0, "TopLeft");
        let top_zero_img = top_zero_img.expect("top image rect with zero padding");
        assert_eq!(top_zero_text.x, rect.left() + 6.0);
        assert_eq!(top_zero_text.y, top_zero_img.bottom());

        let (bottom_text, bottom_img) = button_content_layout(
            rect,
            text,
            image,
            ButtonImageAlignment::Bottom,
            10.0,
            "BottomLeft",
        );
        let bottom_img = bottom_img.expect("bottom image rect");
        assert!((bottom_img.center().x - rect.center().x).abs() < 0.1);
        assert_eq!(bottom_img.bottom(), rect.bottom() - 6.0);
        assert_eq!(bottom_text.x, rect.left() + 6.0);
        assert!(bottom_text.y + text.y <= bottom_img.top() - 10.0);

        let (bottom_zero_text, bottom_zero_img) = button_content_layout(
            rect,
            text,
            image,
            ButtonImageAlignment::Bottom,
            0.0,
            "BottomLeft",
        );
        let bottom_zero_img = bottom_zero_img.expect("bottom image rect with zero padding");
        assert_eq!(bottom_zero_text.x, rect.left() + 6.0);
        assert_eq!(bottom_zero_text.y + text.y, bottom_zero_img.top());
    }

    #[test]
    fn control_border_accepts_button_3d_styles_without_panic() {
        let rect = egui::Rect::from_min_size(Pos2::ZERO, Vec2::new(80.0, 28.0));
        let ctx = egui::Context::default();
        let _ = ctx.run_ui(Default::default(), |root_ui| {
            let ctx = root_ui.ctx().clone();
            let ctx = &ctx;
            egui::CentralPanel::default().show(root_ui, |ui| {
                let painter = ui.painter();
                for style in ["Single", "Fixed3D", "3D", "Raised", "Sunken", "None"] {
                    draw_control_border(
                        painter,
                        rect,
                        egui::CornerRadius::same(3),
                        style,
                        2.0,
                        Color32::from_rgb(80, 100, 140),
                    );
                }
            });
        });
    }

    #[test]
    fn drop_shadow_corner_radius_matches_control_silhouette() {
        use crate::model::{Control, ControlType, PropValue, Rect};

        let mut alias_only = Control::new("Panel1", ControlType::Panel, 0, 0);
        alias_only.rect = Rect::new(0, 0, 120, 80);
        alias_only.properties.shift_remove("CornerRadius");
        alias_only.set_prop("BorderRadius", PropValue::Int(22));
        assert_eq!(
            drop_shadow_corner_radius(&alias_only),
            corner_radius(&alias_only)
        );
        assert_eq!(drop_shadow_corner_radius(&alias_only), 22.0);

        let mut clamped = Control::new("Button1", ControlType::Button, 0, 0);
        clamped.rect = Rect::new(0, 0, 30, 16);
        clamped.set_prop("CornerRadius", PropValue::Int(40));
        assert_eq!(drop_shadow_corner_radius(&clamped), corner_radius(&clamped));
        assert_eq!(drop_shadow_corner_radius(&clamped), 8.0);

        let square = Control::new("Text1", ControlType::TextBox, 0, 0);
        assert_eq!(drop_shadow_corner_radius(&square), 0.0);
    }

    #[test]
    fn picturebox_rounded_image_uses_visible_dest_and_radius() {
        let control = egui::Rect::from_min_size(Pos2::ZERO, Vec2::new(200.0, 100.0));
        let dest = egui::Rect::from_min_size(egui::pos2(25.0, 10.0), Vec2::new(150.0, 80.0));

        let (visible, uv, radius) =
            picturebox_rounded_image_rect_uv(control, dest, 24.0).expect("visible image");

        assert_eq!(visible, dest);
        assert_eq!(uv.min, egui::pos2(0.0, 0.0));
        assert_eq!(uv.max, egui::pos2(1.0, 1.0));
        assert_eq!(radius, 24.0);
    }

    #[test]
    fn picturebox_rounded_image_clips_overflow_and_remaps_uv() {
        let control = egui::Rect::from_min_size(Pos2::ZERO, Vec2::new(100.0, 80.0));
        let dest = egui::Rect::from_min_size(egui::pos2(-50.0, -20.0), Vec2::new(200.0, 100.0));

        let (visible, uv, radius) =
            picturebox_rounded_image_rect_uv(control, dest, 80.0).expect("visible image");

        assert_eq!(visible, control);
        assert_eq!(uv.min, egui::pos2(0.25, 0.2));
        assert_eq!(uv.max, egui::pos2(0.75, 1.0));
        assert_eq!(radius, 40.0);
    }

    #[test]
    fn negative_regular_drop_shadow_blur_draws_as_overlay() {
        use crate::model::{Control, ControlType, PropValue, Rect};

        let mut c = Control::new("Button1", ControlType::Button, 0, 0);
        c.rect = Rect::new(0, 0, 100, 40);
        c.set_prop("ShadowEnabled", PropValue::Bool(true));
        c.set_prop("ShadowBlur", PropValue::Bool(true));
        c.set_prop("ShadowBlurStrength", PropValue::Int(-12));

        let rect = egui::Rect::from_min_size(Pos2::ZERO, Vec2::new(100.0, 40.0));
        let shadow = regular_drop_shadow(&c, rect, false).expect("shadow enabled");
        assert!(shadow.overlay, "negative blur must draw above the control");
        assert_eq!(shadow.blur_strength, 12);

        c.set_prop("ShadowBlurStrength", PropValue::Int(12));
        let shadow = regular_drop_shadow(&c, rect, false).expect("shadow enabled");
        assert!(!shadow.overlay, "positive blur remains behind the control");
        assert_eq!(shadow.blur_strength, 12);
    }

    #[test]
    fn negative_neumorphic_blur_is_front_plane_inset_relief() {
        let mut params = NeumorphicShadowParams {
            shadow_on: true,
            blur_strength: -8.0,
            ..Default::default()
        };
        assert!(
            neumorphic_shadow_overlays(&params),
            "negative Neumorphic blur must draw front-plane inset relief"
        );
        assert!(params.shadow_dir[0] > 0.0 && params.shadow_dir[1] > 0.0);
        params.blur_strength = -1.0;
        let weak = neumorphic_inset_shadow_metrics(&params);
        params.blur_strength = -20.0;
        let strong = neumorphic_inset_shadow_metrics(&params);
        assert!(strong.0 > weak.0, "spread must grow toward -20");
        assert!(strong.1 > weak.1, "layer count must grow toward -20");
        assert!(strong.2 > weak.2, "opacity must grow toward -20");
        assert!(strong.3 > weak.3, "stroke width must grow toward -20");
        assert!(
            strong.0 >= 54.0,
            "max negative blur should use the stronger inset spread"
        );
        assert!(
            strong.2 >= 2.0,
            "max negative blur should double inset opacity strength"
        );

        params.blur_strength = 8.0;
        assert!(
            !neumorphic_shadow_overlays(&params),
            "positive Neumorphic blur remains a behind-control relief shadow"
        );

        params.shadow_on = false;
        params.blur_strength = -8.0;
        assert!(
            !neumorphic_shadow_overlays(&params),
            "disabled shadows must not draw an overlay"
        );
    }

    #[test]
    fn datagrid_cobol_masks_format_common_bound_values() {
        assert_eq!(
            format_cell_with_cobol_mask("1", "number", "9(06)"),
            ("000001".to_owned(), true)
        );
        assert_eq!(
            format_cell_with_cobol_mask("30000000", "number", "PIC S9(9)V99"),
            ("30000000.00".to_owned(), true)
        );
        assert_eq!(
            format_cell_with_cobol_mask("-12.3", "decimal", "S9(4)V99"),
            ("-12.30".to_owned(), true)
        );
        assert_eq!(
            format_cell_with_cobol_mask("Leonardo DiCaprio", "string", "X(40)"),
            ("Leonardo DiCaprio".to_owned(), false)
        );
    }

    #[test]
    fn datagrid_edited_pictures_suppress_and_group() {
        // The reported case: S9(9)V99 value shown through a ZZZ,ZZZ,ZZ9.99 mask.
        assert_eq!(
            format_cell_with_cobol_mask("000003000.00", "decimal", "PIC ZZZ,ZZZ,ZZ9.99"),
            ("3,000.00".to_owned(), true)
        );
        assert_eq!(
            format_cell_with_cobol_mask("1200.00", "decimal", "ZZZ,ZZZ,ZZ9.99"),
            ("1,200.00".to_owned(), true)
        );
        // Zero keeps the forced `9` digit.
        assert_eq!(
            format_cell_with_cobol_mask("0", "decimal", "ZZZ,ZZZ,ZZ9.99"),
            ("0.00".to_owned(), true)
        );
        // Check protection keeps the asterisk fill.
        assert_eq!(
            format_cell_with_cobol_mask("42.5", "decimal", "**,**9.99"),
            ("****42.50".to_owned(), true)
        );
        // Negative gets a leading sign.
        assert_eq!(
            format_cell_with_cobol_mask("-1234.5", "decimal", "Z,ZZ9.99"),
            ("-1,234.50".to_owned(), true)
        );
        // Plain (non-edited) pictures keep the legacy behaviour.
        assert_eq!(
            format_cell_with_cobol_mask("1", "number", "9(06)"),
            ("000001".to_owned(), true)
        );
        assert_eq!(
            format_cell_with_cobol_mask("30000000", "number", "PIC S9(9)V99"),
            ("30000000.00".to_owned(), true)
        );
    }

    #[test]
    fn glass_base_underlay_allows_solid_background_at_full_opacity() {
        let base = Color32::from_rgb(20, 40, 80);
        let solid = glass_base_underlay(base, 1.0).expect("full opacity should draw base");
        assert_eq!(solid.a(), 255);
        assert_eq!((solid.r(), solid.g(), solid.b()), (20, 40, 80));

        let half = glass_base_underlay(base, 0.5).expect("half opacity should draw base");
        assert_eq!(half.a(), 127);

        assert!(glass_base_underlay(Color32::TRANSPARENT, 1.0).is_none());
        assert!(glass_base_underlay(base, 0.0).is_none());
    }

    #[test]
    fn container_image_rounding_only_crops_when_child_crosses_parent_arc() {
        let border = egui::Rect::from_min_max(Pos2::new(0.0, 0.0), Pos2::new(200.0, 200.0));
        let flags = [true, true, true, true];

        // This rectangle is inside the 60px top-left corner band, but its own
        // corner is still inside the parent's rounded path. It must not receive a
        // synthetic 60px crop just because it is near the parent corner.
        let inside_arc_band =
            egui::Rect::from_min_max(Pos2::new(25.0, 35.0), Pos2::new(100.0, 110.0));
        let r = container_image_rounding(inside_arc_band, border, 60.0, flags, 0.0);
        assert_eq!(r.nw, 0);

        // A child that actually crosses the parent corner still receives rounding,
        // but inset-aware rounding is much smaller than the full parent radius.
        let crossing_arc = egui::Rect::from_min_max(Pos2::new(0.0, 30.0), Pos2::new(100.0, 130.0));
        let r = container_image_rounding(crossing_arc, border, 60.0, flags, 0.0);
        assert!(r.nw > 0, "corner crossing parent arc should be clipped");
        assert!(r.nw < 60, "inset child must not get full parent radius");
    }

    #[test]
    fn support_variants_are_lighter_pastels() {
        let base = Color32::from_rgb(0x3F, 0x6F, 0xB5);
        // Grid pastel is lighter and less saturated than the base.
        let (_, bs, bl) = rgb_to_hsl(base);
        let (_, gs, gl) = rgb_to_hsl(pastel_of(base));
        assert!(gl > bl && gs < bs, "grid pastel should be lighter + softer");
        // Border on a dark background is lighter than the base.
        assert!(rgb_to_hsl(border_variant(base, true)).2 > bl);
    }

    // ── Shape control: property-driven drop shadow (all styles) ────────────────

    fn shape_leaf_count(style: crate::model::GlassStyle, shape_type: &str, shadow: bool) -> usize {
        use crate::model::{Control, ControlType, PropValue};

        let ctx = egui::Context::default();
        set_glass_style(&ctx, style);
        let mut c = Control::new("SHP", ControlType::Shape, 0, 0);
        c.rect = crate::model::Rect::new(60, 60, 120, 80);
        c.set_prop("ShapeType", PropValue::String(shape_type.into()));
        // Explicit on/off: Neumorphic styles default the shadow to ON, so the
        // baseline must disable it rather than rely on the absent-prop default.
        c.set_prop("ShadowEnabled", PropValue::Bool(shadow));
        if shadow {
            c.set_prop("ShadowDistance", PropValue::Int(20));
        }
        let mut input = egui::RawInput::default();
        input.screen_rect = Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(400.0, 300.0)));
        let full = ctx.run_ui(input, |root_ui| {
            egui::CentralPanel::default()
                .frame(egui::Frame::NONE)
                .show_inside(root_ui, |ui| {
                    draw_control(ui.painter(), Pos2::ZERO, &c, false, true, 1.0, 1.0, None);
                });
        });
        fn leaves(s: &egui::Shape) -> usize {
            match s {
                egui::Shape::Vec(v) => v.iter().map(leaves).sum(),
                _ => 1,
            }
        }
        full.shapes.iter().map(|cs| leaves(&cs.shape)).sum()
    }

    #[test]
    fn shape_shadow_properties_add_geometry_in_every_style() {
        use crate::model::GlassStyle as GS;
        for style in [GS::Classic, GS::Enhanced, GS::Neumorphic, GS::NeumorphicDark] {
            for st in ["Rectangle", "RoundRect", "Circle", "Triangle"] {
                let without = shape_leaf_count(style, st, false);
                let with = shape_leaf_count(style, st, true);
                assert!(
                    with > without,
                    "ShadowEnabled must add geometry for {st} in {style:?} \
                     (without: {without}, with: {with})"
                );
            }
        }
    }

    fn shape_flat_fill_colors(props: &[(&str, &str)]) -> Vec<Color32> {
        use crate::model::{Control, ControlType, PropValue};

        let ctx = egui::Context::default();
        set_glass_style(&ctx, crate::model::GlassStyle::Classic);
        let mut c = Control::new("SHP", ControlType::Shape, 0, 0);
        c.rect = crate::model::Rect::new(60, 60, 120, 80);
        // Flat classic fill: FormStyle off keeps the face a single filled rect.
        c.set_prop("FormStyle", PropValue::Bool(false));
        c.set_prop("ShadowEnabled", PropValue::Bool(false));
        for &(key, value) in props {
            c.set_prop(key, PropValue::String(value.into()));
        }
        let mut input = egui::RawInput::default();
        input.screen_rect = Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(400.0, 300.0)));
        let full = ctx.run_ui(input, |root_ui| {
            egui::CentralPanel::default()
                .frame(egui::Frame::NONE)
                .show_inside(root_ui, |ui| {
                    draw_control(ui.painter(), Pos2::ZERO, &c, false, true, 1.0, 1.0, None);
                });
        });
        fn collect(s: &egui::Shape, out: &mut Vec<Color32>) {
            match s {
                egui::Shape::Vec(v) => v.iter().for_each(|s| collect(s, out)),
                egui::Shape::Rect(r) => out.push(r.fill),
                _ => {}
            }
        }
        let mut out = Vec::new();
        for cs in &full.shapes {
            collect(&cs.shape, &mut out);
        }
        out
    }

    #[test]
    fn appearance_back_color_fills_a_shape_unless_fill_color_overrides() {
        // Untouched colours (BackgroundColor still on the universal default
        // every new control gets) → the legacy silver face.
        assert!(shape_flat_fill_colors(&[]).contains(&Color32::from_rgb(192, 192, 192)));
        // Appearance → Back colour changed from the default fills the shape.
        assert!(shape_flat_fill_colors(&[("BackgroundColor", "#FF0000")])
            .contains(&Color32::from_rgb(255, 0, 0)));
        // The type-specific FillColor stays authoritative over Appearance.
        let both = shape_flat_fill_colors(&[
            ("BackgroundColor", "#FF0000"),
            ("FillColor", "#00FF00"),
        ]);
        assert!(both.contains(&Color32::from_rgb(0, 255, 0)));
        assert!(!both.contains(&Color32::from_rgb(255, 0, 0)));
    }

    fn shape_line_style_leaf_count(shape_type: &str, line_style: &str) -> usize {
        use crate::model::{Control, ControlType, PropValue};

        let ctx = egui::Context::default();
        set_glass_style(&ctx, crate::model::GlassStyle::Classic);
        let mut c = Control::new("SHP", ControlType::Shape, 0, 0);
        c.rect = crate::model::Rect::new(60, 60, 120, 80);
        c.set_prop("ShapeType", PropValue::String(shape_type.into()));
        c.set_prop("LineStyle", PropValue::String(line_style.into()));
        c.set_prop("FillStyle", PropValue::String("None".into()));
        let mut input = egui::RawInput::default();
        input.screen_rect = Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(400.0, 300.0)));
        let full = ctx.run_ui(input, |root_ui| {
            egui::CentralPanel::default()
                .frame(egui::Frame::NONE)
                .show_inside(root_ui, |ui| {
                    draw_control(ui.painter(), Pos2::ZERO, &c, false, true, 1.0, 1.0, None);
                });
        });
        fn leaves(s: &egui::Shape) -> usize {
            match s {
                egui::Shape::Vec(v) => v.iter().map(leaves).sum(),
                _ => 1,
            }
        }
        full.shapes.iter().map(|cs| leaves(&cs.shape)).sum()
    }

    #[test]
    fn shape_line_style_none_removes_outline_and_dashes_add_segments() {
        for st in ["Rectangle", "RoundRect", "Circle", "Triangle"] {
            let none = shape_line_style_leaf_count(st, "None");
            let solid = shape_line_style_leaf_count(st, "Solid");
            let dash = shape_line_style_leaf_count(st, "Dash");
            assert!(
                none < solid,
                "LineStyle None must drop the outline for {st} (none: {none}, solid: {solid})"
            );
            assert!(
                dash > solid,
                "LineStyle Dash must tessellate into dash segments for {st} \
                 (dash: {dash}, solid: {solid})"
            );
        }
    }

    // ── Spec 039 T3: Knob/Gauge/Switch/FileDropZone static-preview paint ───

    fn leaf_count_for(ctrl: &crate::model::Control) -> usize {
        let ctx = egui::Context::default();
        let mut input = egui::RawInput::default();
        input.screen_rect = Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(400.0, 300.0)));
        let full = ctx.run_ui(input, |root_ui| {
            egui::CentralPanel::default()
                .frame(egui::Frame::NONE)
                .show_inside(root_ui, |ui| {
                    draw_control(ui.painter(), Pos2::ZERO, ctrl, false, true, 1.0, 1.0, None);
                });
        });
        fn leaves(s: &egui::Shape) -> usize {
            match s {
                egui::Shape::Vec(v) => v.iter().map(leaves).sum(),
                _ => 1,
            }
        }
        full.shapes.iter().map(|cs| leaves(&cs.shape)).sum()
    }

    #[test]
    fn knob_gauge_switch_file_drop_zone_static_preview_paints_without_panicking() {
        use crate::model::{Control, ControlType};
        let cases: &[(ControlType, &[(&str, &str)])] = &[
            (ControlType::Knob, &[("Value", "42")][..]),
            (
                ControlType::Gauge,
                &[("GaugeStyle", "Radial"), ("Value", "70")][..],
            ),
            (
                ControlType::Gauge,
                &[("GaugeStyle", "Linear"), ("Value", "30")][..],
            ),
            (
                ControlType::Gauge,
                &[("GaugeStyle", "Donut"), ("Value", "55")][..],
            ),
            (ControlType::Switch, &[("Checked", "true")][..]),
            (ControlType::FileDropZone, &[("Hint", "CSV or XLSX")][..]),
        ];
        for (ct, props) in cases {
            let mut c = Control::new("C1", ct.clone(), 0, 0);
            c.rect = crate::model::Rect::new(20, 20, 100, 80);
            for &(k, v) in *props {
                c.set_prop(k, PropValue::String(v.into()));
            }
            let leaves = leaf_count_for(&c);
            assert!(leaves > 0, "{ct:?} with {props:?} painted no geometry at all");
        }
    }

    #[test]
    fn maps_static_preview_paints_a_backdrop_without_panicking() {
        // `map_tiles::request_tile` spawns a real background thread that
        // makes a real HTTP request to tile.openstreetmap.org — this test
        // does NOT wait for it (the tile stays "Loading", drawn as the
        // plain grey backdrop, until a later frame polls it), so it stays
        // fast and deterministic regardless of network conditions; it only
        // proves the synchronous paint path — request kick-off, backdrop
        // fill, no panic — never blocks or depends on the download itself.
        use crate::model::{Control, ControlType};
        let mut c = Control::new("MAP-1", ControlType::Maps, 0, 0);
        c.rect = crate::model::Rect::new(0, 0, 320, 240);
        c.set_prop("CenterLat", PropValue::String("40.7128".into()));
        c.set_prop("CenterLng", PropValue::String("-74.0060".into()));
        c.set_prop("Zoom", PropValue::Int(10));
        let leaves = leaf_count_for(&c);
        assert!(leaves > 0, "Maps painted no geometry at all (not even the backdrop)");
    }

    #[test]
    fn knob_fill_arc_grows_with_value() {
        use crate::model::{Control, ControlType};

        // The track and fill arcs are each a single `Shape::Path` holding
        // many points internally — a `Shape::Vec`-recursing leaf counter
        // (as used elsewhere in this file) sees ONE opaque leaf per path
        // regardless of how many points are in it, so it cannot see this
        // change. Sum the actual point counts across every `Shape::Path`
        // instead — `draw_ring`'s fill segment count scales with the swept
        // angle, so a fuller knob really does produce more points.
        fn total_path_points(ctrl: &crate::model::Control) -> usize {
            let ctx = egui::Context::default();
            let mut input = egui::RawInput::default();
            input.screen_rect = Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(400.0, 300.0)));
            let full = ctx.run_ui(input, |root_ui| {
                egui::CentralPanel::default()
                    .frame(egui::Frame::NONE)
                    .show_inside(root_ui, |ui| {
                        draw_control(ui.painter(), Pos2::ZERO, ctrl, false, true, 1.0, 1.0, None);
                    });
            });
            fn count(s: &egui::Shape, total: &mut usize) {
                match s {
                    egui::Shape::Vec(v) => v.iter().for_each(|s| count(s, total)),
                    egui::Shape::Path(p) => *total += p.points.len(),
                    _ => {}
                }
            }
            let mut total = 0;
            for cs in &full.shapes {
                count(&cs.shape, &mut total);
            }
            total
        }

        let mut low = Control::new("K1", ControlType::Knob, 0, 0);
        low.rect = crate::model::Rect::new(0, 0, 100, 100);
        low.set_prop("Minimum", PropValue::Int(0));
        low.set_prop("Maximum", PropValue::Int(100));
        low.set_prop("Value", PropValue::Int(1));
        low.set_prop("ShowValue", PropValue::Bool(false));

        let mut high = low.clone();
        high.set_prop("Value", PropValue::Int(99));

        assert!(
            total_path_points(&high) > total_path_points(&low),
            "a higher Value must sweep a longer fill arc (more points), not \
             the same track-only ring ({} vs {})",
            total_path_points(&high),
            total_path_points(&low),
        );
    }

    #[test]
    fn switch_thumb_moves_from_off_side_to_on_side() {
        use crate::model::{Control, ControlType};
        let mut off = Control::new("S1", ControlType::Switch, 0, 0);
        off.rect = crate::model::Rect::new(0, 0, 60, 30);
        off.set_prop("Checked", PropValue::Bool(false));
        let mut on = off.clone();
        on.set_prop("Checked", PropValue::Bool(true));

        // Both states paint the same shape COUNT (track + thumb either way);
        // what differs is the thumb's x position, which the leaf-count check
        // can't see — assert on the actual painted position instead.
        fn thumb_center_x(ctrl: &crate::model::Control) -> f32 {
            let ctx = egui::Context::default();
            let mut input = egui::RawInput::default();
            input.screen_rect = Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(400.0, 300.0)));
            let full = ctx.run_ui(input, |root_ui| {
                egui::CentralPanel::default()
                    .frame(egui::Frame::NONE)
                    .show_inside(root_ui, |ui| {
                        draw_control(ui.painter(), Pos2::ZERO, ctrl, false, true, 1.0, 1.0, None);
                    });
            });
            // The thumb is the only filled circle this control paints.
            fn find_circle_x(s: &egui::Shape, out: &mut Option<f32>) {
                match s {
                    egui::Shape::Vec(v) => v.iter().for_each(|s| find_circle_x(s, out)),
                    egui::Shape::Circle(c) => *out = Some(c.center.x),
                    _ => {}
                }
            }
            let mut x = None;
            for cs in &full.shapes {
                find_circle_x(&cs.shape, &mut x);
            }
            x.expect("Switch must paint a circular thumb")
        }
        assert!(
            thumb_center_x(&on) > thumb_center_x(&off),
            "Checked=true must paint the thumb on the right, not the left"
        );
    }
}
