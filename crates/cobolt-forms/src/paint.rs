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

/// Lay a caption out at the largest size up to `fsize` that fits `max_w` x
/// `max_h`, down to a 6 pt floor.
///
/// The TextBox has always done this; every other caption-bearing control laid
/// its text out at the requested size and let it spill over the border. This is
/// that behaviour, in one place, so the four caption branches cannot drift.
///
/// Both axes are tested. Height alone is enough only while the text can wrap,
/// and a caption is usually a single word ("Button-1") — a word cannot be
/// broken, so it overflows sideways at a height that fits perfectly well.
#[allow(clippy::too_many_arguments)]
fn fitted_caption_galley(
    painter: &egui::Painter,
    ctrl: &Control,
    text: &str,
    font_name: &str,
    fsize: f32,
    color: Color32,
    max_w: f32,
    max_h: f32,
    halign: egui::Align,
) -> std::sync::Arc<egui::Galley> {
    const MIN_FONT: f32 = 6.0;
    let wrap_w = if max_w.is_finite() { max_w.max(1.0) } else { max_w };
    let mut fit = fsize.max(MIN_FONT);
    let lay = |size: f32| {
        painter.layout_job(styled_text_job(
            painter, ctrl, text, font_name, size, color, wrap_w, halign,
        ))
    };
    let mut galley = lay(fit);
    while (galley.size().y > max_h || galley.size().x > max_w) && fit > MIN_FONT {
        fit = (fit - 1.0).max(MIN_FONT);
        galley = lay(fit);
    }
    galley
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
    let params = painter
        .ctx()
        .data(|d| d.get_temp::<NeumorphicShadowParams>(neumorphic_params_id()))
        .unwrap_or_default();
    neumorphic_shadow_stack(&params, rect, rounding.into(), alpha_mul).paint(painter);
}

/// One layer of a soft shadow: a rounded rect filled with a premultiplied colour.
#[derive(Debug, Clone, Copy)]
pub struct ShadowLayer {
    pub rect: egui::Rect,
    pub rounding: egui::CornerRadius,
    pub color: Color32,
}

/// The whole soft-shadow stack a control paints BEHIND its face — built once,
/// then either painted or sampled.
///
/// The corner-notch mask repaints the form backdrop over a rounded control's
/// notches, and that also erases the shadow which legitimately showed there: a
/// flat wedge bitten out of the halo at every corner (operator, 2026-08-21 — a
/// Maps control with an exaggerated shadow). Putting the shadow back means
/// knowing what colour it left at a point, and the only safe way to know is to
/// ask the SAME stack the painter drew. Deriving the geometry a second time is
/// exactly how this project keeps ending up with two painters that quietly
/// disagree.
#[derive(Debug, Clone, Default)]
pub struct ShadowStack {
    layers: Vec<ShadowLayer>,
    /// Sunken relief is clipped inside the control; a raised halo is not.
    clip: Option<egui::Rect>,
}

impl ShadowStack {
    pub fn is_empty(&self) -> bool {
        self.layers.is_empty()
    }

    /// Draw the stack back to front, exactly as the hand-rolled loops did.
    pub fn paint(&self, painter: &egui::Painter) {
        let clipped;
        let p: &egui::Painter = match self.clip {
            Some(c) => {
                clipped = painter.with_clip_rect(painter.clip_rect().intersect(c));
                &clipped
            }
            None => painter,
        };
        for l in &self.layers {
            p.rect_filled(l.rect, l.rounding, l.color);
        }
    }

    /// The premultiplied colour this stack leaves at `p` over a transparent base
    /// — i.e. what has to be re-composited on top of a repainted backdrop for the
    /// pixel to look the way it did before the repaint.
    pub fn sample(&self, p: Pos2) -> Color32 {
        if self.clip.is_some_and(|c| !c.contains(p)) {
            return Color32::TRANSPARENT;
        }
        let mut acc = Color32::TRANSPARENT;
        for l in &self.layers {
            if rounded_rect_contains(l.rect, l.rounding, p) {
                acc = composite_premultiplied_over(l.color, acc);
            }
        }
        acc
    }
}

/// Is `p` inside `rect` with `rounding`, as egui's tessellator would draw it?
///
/// Each corner radius is clamped to half the shorter side the way the tessellator
/// clamps it, so this answers about the shape that is actually drawn rather than
/// the one that was requested (CORNER-BLEED-PLAYBOOK §1.1: the stored radius lies).
pub fn rounded_rect_contains(rect: egui::Rect, rounding: egui::CornerRadius, p: Pos2) -> bool {
    if !rect.contains(p) {
        return false;
    }
    let cap = (rect.width() * 0.5).min(rect.height() * 0.5);
    let corners = [
        (rounding.nw, rect.left_top(), -1.0_f32, -1.0_f32),
        (rounding.ne, rect.right_top(), 1.0, -1.0),
        (rounding.se, rect.right_bottom(), 1.0, 1.0),
        (rounding.sw, rect.left_bottom(), -1.0, 1.0),
    ];
    for (stored, apex, sx, sy) in corners {
        let r = f32::from(stored).min(cap);
        if r <= 0.0 {
            continue;
        }
        let c = egui::pos2(apex.x - sx * r, apex.y - sy * r);
        if (p.x - c.x) * sx > 0.0 && (p.y - c.y) * sy > 0.0 && (p - c).length() > r {
            return false; // carved away by this corner's arc
        }
    }
    true
}

/// A control's Neumorphic shadow settings, read off its properties.
///
/// `draw_control` publishes these into the egui temp store for the branch
/// painters to pick up. The notch mask runs AFTER the whole control loop, when
/// that store holds whatever the last control published — so it reads the
/// control's own properties here instead of the store, and this is the function
/// that keeps the two answers the same.
pub(crate) fn neumorphic_shadow_params(ctrl: &Control) -> NeumorphicShadowParams {
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

    NeumorphicShadowParams {
        shadow_on,
        shadow_color,
        light_color,
        shadow_opac,
        shadow_dir: [ux, uy],
        distance,
        blur_strength,
    }
}

/// The soft-shadow stack `ctrl` paints behind its face at `rect`, for the
/// corner-notch mask to re-composite after it repaints the backdrop.
///
/// Mirrors what `draw_control` actually draws: the Neumorphic dual halo while
/// that register is on (`drop_shadow_spec` returns `None` for every control
/// then — the relief IS the shadow), and otherwise the ordinary drop shadow.
/// An OVERLAY shadow is excluded because it is painted on top of the face, not
/// behind it, so the mask never erased it.
pub fn control_shadow_stack(
    ctx: &egui::Context,
    ctrl: &Control,
    rect: egui::Rect,
    alpha_mul: f32,
) -> ShadowStack {
    let is_neumorphic = glass_config_applies(ctx) && active_glass_style(ctx).is_neumorphic();
    if is_neumorphic {
        return neumorphic_shadow_stack(
            &neumorphic_shadow_params(ctrl),
            rect,
            themed_corner_radius(ctx, ctrl).into(),
            alpha_mul,
        );
    }
    match regular_drop_shadow(ctrl, rect, false).filter(|s| !s.overlay) {
        Some(shadow) => regular_shadow_stack(&shadow, alpha_mul),
        None => ShadowStack::default(),
    }
}

/// The dual-halo stack that `draw_glass_neumorphic` and
/// [`draw_neumorphic_shadow_only`] both paint. One definition, so the sampler the
/// notch mask uses cannot drift from what was drawn.
fn neumorphic_shadow_stack(
    params: &NeumorphicShadowParams,
    rect: egui::Rect,
    rounding: egui::CornerRadius,
    alpha_mul: f32,
) -> ShadowStack {
    let mut stack = ShadowStack::default();
    if alpha_mul <= 0.0 || !params.shadow_on || neumorphic_shadow_overlays(params) {
        return stack;
    }
    let am = alpha_mul.clamp(0.0, 1.0);
    let cap = (rect.width() * 0.5).min(rect.height() * 0.5);
    let rnd = round_map(rounding, |c| c.max(0.0).min(cap));

    let spread = (1.0_f32 + params.blur_strength.abs()).ln() * 8.0;
    let layers = 10_usize;
    let ux = params.shadow_dir[0];
    let uy = params.shadow_dir[1];
    let distance = params.distance;
    let sunken = params.blur_strength < 0.0;
    // Clip inside rect for sunken; let the halo bleed outside for raised.
    stack.clip = sunken.then_some(rect);

    let mut push = |sign: f32, colour: Color32, opac: f32| {
        let offset = Vec2::new(sign * ux * distance, sign * uy * distance);
        for i in 0..=layers {
            let t = 1.0 - (i as f32 / layers as f32);
            let expand = t * spread;
            let falloff = (-3.0 * t * t).exp();
            let a_val = (opac * am * falloff * 255.0) as u8;
            if a_val == 0 {
                continue;
            }
            let f = a_val as f32 / 255.0;
            stack.layers.push(ShadowLayer {
                rect: rect.translate(offset).expand(expand),
                rounding: round_map(rnd, |c| c + expand),
                color: Color32::from_rgba_premultiplied(
                    (colour.r() as f32 * f) as u8,
                    (colour.g() as f32 * f) as u8,
                    (colour.b() as f32 * f) as u8,
                    a_val,
                ),
            });
        }
    };

    // Light side: NW-outside when raised; SE-inside when sunken.
    push(
        if sunken { 1.0 } else { -1.0 },
        params.light_color,
        (params.shadow_opac * 3.25).clamp(0.0, 1.0),
    );
    // Dark (user colour): SE-outside when raised; NW-inside when sunken.
    push(
        if sunken { -1.0 } else { 1.0 },
        params.shadow_color,
        params.shadow_opac,
    );
    stack
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

/// How far a Slider's tick marks reach beyond its track.
const TICK_LEN: f32 = 5.0;
/// Room a Slider's min/max labels need under the ticks — the 9pt label's line
/// height. Part of the assembly's own height, so the whole slider centres as
/// one block instead of the labels hanging off the rect's bottom edge.
const SLIDER_LABEL_H: f32 = 11.0;

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
        draw_surface_auto(
            painter,
            rect,
            NV_CARD,
            12.0,
            selected,
            alpha_mul,
            SurfaceRole::Card,
        );
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
    crate::model::alpha_multiplier(ctrl)
}

/// How opaque a control's own FACE is painted — [`opacity_of`], except that a
/// ToolBar the developer has given a colour is not left invisible.
///
/// A ToolBar ships at `Transparency = 100` so a bare one reads as buttons on
/// the form rather than a card. That default silently voided `BackgroundColor`:
/// the operator picked a colour in the inspector, nothing whatever happened,
/// and nothing said why (operator, 2026-08-22: "Toolbar, background color …
/// does not work"). Two properties, one quietly cancelling the other.
///
/// The seeded 100 is what EVERY toolbar carries, not something anyone chose —
/// the same "still on the default means the user has not picked" convention
/// [`user_background_color`] already applies to the colour itself. So choosing
/// a colour is what turns the frame on. A `Transparency` the developer actually
/// moved (anything but that seeded 100) still fades the face exactly as before,
/// and a toolbar whose colour is untouched stays invisible, as it always was.
pub fn face_opacity_of(ctrl: &Control) -> f32 {
    let seeded_transparent = matches!(ctrl.control_type, crate::ControlType::ToolBar)
        && crate::model::transparency_of(ctrl) == 100
        && user_background_color(ctrl).is_some();
    if seeded_transparent {
        1.0
    } else {
        opacity_of(ctrl)
    }
}

/// A caption already laid out, ready to be drawn — the galley, where its draw
/// origin sits, and the colour it falls back to.
///
/// Handed to a caller that wants to paint the text ITSELF (the run form, whose
/// Label text is selectable) instead of having it stamped on the canvas. It is
/// the painter's own layout, not a second one: two layouts of the same caption
/// drift the moment either side changes, and a Label that moves a pixel between
/// the designer and the running form is a bug report.
pub struct CaptionLayout {
    pub galley: std::sync::Arc<egui::Galley>,
    pub pos: Pos2,
    pub color: Color32,
}

/// What [`draw_control_body`] does with the control's caption.
enum CaptionMode<'a> {
    /// Stamp it — the designer canvas and every non-interactive surface.
    Paint,
    /// Draw the face only; the caller paints the live content itself.
    Skip,
    /// Hand a Label's caption back INSTEAD of stamping it, so the caller can
    /// host the very same galley as selectable text.
    Capture(&'a mut Option<CaptionLayout>),
}

/// Paint a control exactly as the designer canvas does — its face, its border
/// and the placeholder text the canvas stands in with for content the running
/// control supplies itself.
#[allow(clippy::too_many_arguments)]
pub fn draw_control(
    painter: &egui::Painter,
    origin: Pos2,
    ctrl: &Control,
    selected: bool,
    glass: bool,
    alpha_mul: f32,
    scale: f32,
    pic_tex: Option<egui::TextureId>,
) {
    draw_control_body(
        painter,
        origin,
        ctrl,
        selected,
        glass,
        alpha_mul,
        scale,
        pic_tex,
        &mut CaptionMode::Paint,
    );
}

/// [`draw_control`], except a **Label's caption is returned rather than
/// painted** — face, border and shadow are drawn as usual.
///
/// This is how the running form gives label text a selection: it takes the
/// caption the painter laid out and hosts it through egui's label-selection
/// machinery, which needs to own the draw call to paint the highlight under the
/// glyphs. Every other control type paints exactly as [`draw_control`] does and
/// returns `None`.
#[allow(clippy::too_many_arguments)]
pub fn draw_control_capturing_label(
    painter: &egui::Painter,
    origin: Pos2,
    ctrl: &Control,
    selected: bool,
    glass: bool,
    alpha_mul: f32,
    scale: f32,
    pic_tex: Option<egui::TextureId>,
) -> Option<CaptionLayout> {
    let mut caption = None;
    draw_control_body(
        painter,
        origin,
        ctrl,
        selected,
        glass,
        alpha_mul,
        scale,
        pic_tex,
        &mut CaptionMode::Capture(&mut caption),
    );
    caption
}

/// [`draw_control`] **without** the canvas placeholder text.
///
/// A handful of controls stand in for their content on the canvas — a ComboBox
/// letters its first item and a `▾` — because the canvas has no running value
/// to show. The interactive renderer *does*: it draws the real one, in the
/// control's own font and colour, with its own arrow. Calling `draw_control`
/// there would paint the stand-in underneath the real thing, so a combo showing
/// `Banana` would carry a ghost `Apple ▾` behind it.
///
/// This is the entry point for a live widget that wants the designed **face** —
/// `BackgroundColor`, the background gradient, the border, the corner radius —
/// and nothing else.
#[allow(clippy::too_many_arguments)]
pub fn draw_control_face(
    painter: &egui::Painter,
    origin: Pos2,
    ctrl: &Control,
    selected: bool,
    glass: bool,
    alpha_mul: f32,
    scale: f32,
    pic_tex: Option<egui::TextureId>,
) {
    draw_control_body(
        painter,
        origin,
        ctrl,
        selected,
        glass,
        alpha_mul,
        scale,
        pic_tex,
        &mut CaptionMode::Skip,
    );
}

#[allow(clippy::too_many_arguments)]
fn draw_control_body(
    painter: &egui::Painter,
    origin: Pos2,
    ctrl: &Control,
    selected: bool,
    glass: bool,
    alpha_mul: f32,
    scale: f32,                       // animation scale factor (1.0 = normal)
    pic_tex: Option<egui::TextureId>, // pre-loaded texture for PictureBox
    // What becomes of the canvas placeholder caption — see `draw_control_face`
    // and `draw_control_capturing_label`.
    caption_mode: &mut CaptionMode<'_>,
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

    // `Transparency` is about the control's FACE, not about erasing the control.
    // It answers "how much of what is behind shows through", so it fades the
    // background, the frame and its shadow — while the tick, the glyph, the
    // caption and the border stay exactly as legible as they were. Folding it
    // into `alpha_mul` instead made a fully transparent control invisible, which
    // is why a CheckBox at its new default of 100 vanished outright rather than
    // simply losing the card behind it.
    //
    // Ancestor *container* transparencies are still folded into the incoming
    // `alpha_mul` by the render walk, so a faded container dims its whole
    // subtree (spec 012) exactly as before.
    let face_alpha = alpha_mul * face_opacity_of(ctrl);

    let a = (alpha_mul.clamp(0.0, 1.0) * 255.0) as u8;
    let c_scale = |c: u8| -> u8 { ((c as f32) * alpha_mul) as u8 };
    let alpha_color =
        |c: Color32| Color32::from_rgba_premultiplied(c.r(), c.g(), c.b(), c_scale(c.a()));

    // Composite-frame diagnostics overlay (spec 017 corner-bleed hunt): traces every
    // frame a container draws — shadow, face, border, notch mask, restored outline —
    // in place with its real rounding. Toggled at runtime via COBOLT_FRAME_DIAGNOSTICS.
    let container_diag = frame_diagnostics_enabled();

    // 050 R4/R6 — THE gate. Neumorphic relief is Liquid Glass configuration, and
    // a self-contained theme owns the whole look, so it must not be applied on
    // top of one. This was read unconditionally, which is how a Liquid Glass
    // setting reached a flat theme: it painted neumorphic rims on a surface with
    // no relief AND — because `regular_drop_shadow` bails when neumorphic —
    // silently suppressed every drop shadow the developer had switched on.
    let is_neumorphic =
        glass_config_applies(painter.ctx()) && active_glass_style(painter.ctx()).is_neumorphic();

    if is_neumorphic {
        let params = neumorphic_shadow_params(ctrl);
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
        // SurfaceStyle (default true): the shape follows the form's current style
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
            // The designed background GRADIENT, which leads over every flat and
            // frosted face exactly as it does on any other control.
            //
            // A Shape paints its own face and returns long before the generic
            // frame code, which is the only place the gradient was ever read —
            // so a Shape's Background colour worked while its Background
            // gradient did nothing at all (operator, 2026-08-18).
            let gradient = ctrl
                .get_prop("BackgroundGradientEnabled")
                .map(|v| v.as_bool())
                .unwrap_or(false)
                .then(|| {
                    let colour = |key: &str| {
                        alpha_color(
                            ctrl.get_prop(key)
                                .map(|v| parse_color(v.as_str()))
                                .unwrap_or(fill_color),
                        )
                    };
                    (
                        colour("BackgroundGradientStartColor"),
                        colour("BackgroundGradientEndColor"),
                        ctrl.get_prop("BackgroundGradientDirection")
                            .map(|v| v.as_str().to_owned())
                            .unwrap_or_else(|| "South".into()),
                    )
                });
            if let Some((start, end, dir)) = gradient {
                // Every silhouette, not just the rectangle: a circle and a
                // triangle are filled as a fan from their centre, each vertex
                // taking the gradient's colour at its own position, so the
                // shading follows the shape instead of a box around it.
                if is_round {
                    painter.add(egui::Shape::mesh(gradient_fan(
                        rect,
                        cc,
                        &circle_perimeter(cc, circ_r),
                        start,
                        end,
                        &dir,
                    )));
                } else if is_tri {
                    painter.add(egui::Shape::mesh(gradient_fan(
                        rect,
                        rect.center(),
                        &polygon_perimeter(&[tri_top, tri_br, tri_bl]),
                        start,
                        end,
                        &dir,
                    )));
                } else {
                    painter.add(egui::Shape::mesh(background_gradient_mesh(
                        rect,
                        start,
                        end,
                        &dir,
                        egui::CornerRadius::same(rr.round().clamp(0.0, 255.0) as u8),
                    )));
                }
            } else if glass && is_neumorphic && (is_round || is_tri) {
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
                draw_surface_auto(
                    painter,
                    rect,
                    fill_color,
                    rr,
                    selected,
                    alpha_mul,
                    SurfaceRole::Shape,
                );
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
        // The designed background gradient, under the track and the thumb — a
        // custom painter returns before the generic frame code that would
        // otherwise have drawn it.
        paint_background_gradient(
            painter,
            rect,
            themed_corner_radius(painter.ctx(), ctrl).into(),
            ctrl,
            alpha_mul,
        );
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

        let show_val = ctrl
            .get_prop("ShowValue")
            .map(|v| v.as_bool())
            .unwrap_or(false);

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
        // A Slider's own colour properties outrank the generic Appearance pair,
        // the way a Shape's FillColor already does: TrackColor paints the rail,
        // ThumbColor the knob, FillColor the travelled part (Minimum → Value).
        // The defaults are model.rs's, so "still the default" reads as "the
        // developer left it alone" and the theme keeps the wheel. All three
        // used to be parsed and then thrown away — the glass painter drew its
        // own colours and nothing a developer picked here reached the screen.
        let user_track = non_default("TrackColor", "#AAAAAA")
            .or_else(|| non_default("BackgroundColor", crate::model::DEFAULT_BACKGROUND_COLOR));
        let user_fill = non_default("FillColor", "#0078D7");
        // 050 — a Slider is a data-input control, and `Control::new` seeds those
        // with a BLACK `ForegroundColor` (model.rs), not the `#FFFFFF` sentinel.
        // Comparing against the sentinel therefore read every untouched Slider
        // as "the developer chose black", so a theme's knob colour could never
        // apply. Correct the baseline only where a theme actually offers one:
        // Liquid Glass keeps the knob it has always drawn (R21).
        let theme_knob =
            theme_token(painter.ctx(), crate::surface_theme::ColorToken::SliderKnob);
        let seeded_fg = match theme_knob {
            Some(_) if ctrl.control_type.is_data_input_control() => "#000000",
            _ => crate::model::DEFAULT_FOREGROUND_COLOR,
        };
        let user_thumb = non_default("ThumbColor", "#0078D7")
            .or_else(|| non_default("ForegroundColor", seeded_fg));
        let tint = |c: Color32, a: f32| {
            Color32::from_rgba_unmultiplied(
                c.r(),
                c.g(),
                c.b(),
                (a * alpha_mul).clamp(0.0, 255.0) as u8,
            )
        };

        // Glass track/knob colours (defaults), overridden by Back/Fore colour.
        // 047/050 — a theme may supply the defaults instead; the built-ins here
        // are Liquid Glass's own, and Liquid Glass supplies nothing.
        use crate::surface_theme::ColorToken as Tok;
        // 050 — a theme may supply the FILLED part of the rail (start → knob).
        // Liquid Glass supplies none and keeps its frosted track exactly as it
        // was.
        let theme_fill = theme_token(painter.ctx(), Tok::SliderFill);
        // The rail is the part still to travel, so under a theme it takes the
        // muted structural colour and the fill marks the travelled part. It was
        // left bare here before, so with a coloured Back colour underneath it
        // the two sides read back to front: the travelled part came out muted
        // and the remainder came out coloured.
        let track_body = user_track.map(|c| tint(c, 210.0)).unwrap_or_else(|| {
            if let Some(c) = theme_fill.and(theme_token(painter.ctx(), Tok::Border)) {
                tint(c, 110.0)
            } else {
                Color32::from_rgba_premultiplied(
                    (100.0 * alpha_mul) as u8,
                    (110.0 * alpha_mul) as u8,
                    (135.0 * alpha_mul) as u8,
                    (90.0 * alpha_mul) as u8,
                )
            }
        });
        let track_rim = user_track
            .map(|c| tint(shade(c, 0.45), 190.0))
            .unwrap_or_else(|| {
                if theme_fill.is_some() {
                    // A flat themed rail carries no rim of its own.
                    Color32::TRANSPARENT
                } else {
                    Color32::from_rgba_premultiplied(
                        (180.0 * alpha_mul) as u8,
                        (185.0 * alpha_mul) as u8,
                        (210.0 * alpha_mul) as u8,
                        (120.0 * alpha_mul) as u8,
                    )
                }
            });
        // The travelled part, start → knob: the developer's FillColor when they
        // picked one, otherwise whatever the theme offers. Liquid Glass offers
        // none and keeps its frosted rail undivided, exactly as before.
        let range_fill = user_fill
            .map(|c| tint(c, 235.0))
            .or_else(|| theme_fill.map(|c| theme_alpha(c, alpha_mul)));
        let thumb_body = user_thumb.map(|c| tint(c, 235.0)).unwrap_or_else(|| {
            if let Some(c) = theme_knob {
                tint(c, 255.0)
            } else {
                Color32::from_rgba_premultiplied(
                    (150.0 * alpha_mul) as u8,
                    (160.0 * alpha_mul) as u8,
                    (195.0 * alpha_mul) as u8,
                    (140.0 * alpha_mul) as u8,
                )
            }
        });
        let thumb_rim = user_thumb
            .map(|c| tint(shade(c, 0.5), 210.0))
            .unwrap_or_else(|| {
                if let Some(c) = theme_token(painter.ctx(), Tok::Border) {
                    tint(c, 255.0)
                } else {
                    Color32::from_rgba_premultiplied(
                        (220.0 * alpha_mul) as u8,
                        (225.0 * alpha_mul) as u8,
                        (245.0 * alpha_mul) as u8,
                        (180.0 * alpha_mul) as u8,
                    )
                }
            });

        // Where the horizontal min/max labels sit. Set by the horizontal arm so
        // the labels ride with the assembly instead of being pinned to the
        // rect's bottom edge; the vertical arm keeps its own placement.
        let mut h_label_y = rect.max.y - 1.0;

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
            // 050 — the filled range, start → knob. A vertical slider fills from
            // the BOTTOM, which is where its zero is.
            if let Some(fill_c) = range_fill {
                let filled = egui::Rect::from_min_max(
                    Pos2::new(track_rect.min.x, thumb_y),
                    Pos2::new(track_rect.max.x, track_b),
                );
                if filled.height() > 0.5 {
                    painter.rect_filled(filled, track_half_w, fill_c);
                }
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
            if theme_fill.is_some() {
                // A themed knob is FLAT: a solid face with a rim. `draw_glass_pill`
                // is a glass primitive — frost and a lens — so a knob drawn
                // through it never actually shows the theme's colour.
                draw_flat_pill(painter, thumb_rect, thumb_body, thumb_rim);
            } else {
                draw_glass_pill(painter, thumb_rect, thumb_body, true, thumb_rim);
                // Lens at bottom-center of thumb
                draw_lens(
                    painter,
                    Pos2::new(cx, thumb_rect.max.y - thumb_h * 0.28),
                    thumb_w * 0.32,
                    thumb_h * 0.18,
                );
            }
        } else {
            // ── Horizontal glass slider ──────────────────────────────────────
            let track_half_h = (rect.height() * 0.18).clamp(4.0, 12.0);
            // The slider is ONE assembly — track, thumb, ticks and the min/max
            // labels — and it is centred as a whole. Centring the track alone
            // while the labels stayed pinned to `rect.max.y` split it in two:
            // the labels hugged the bottom edge, the track sat at the middle,
            // and every pixel of extra height opened a gap at the TOP only. A
            // 780x75 slider carried 22px of dead space above and 3 below.
            let tick_room = if tick_st != "None" { TICK_LEN + 1.0 } else { 0.0 };
            let label_room = SLIDER_LABEL_H;
            let above = (track_half_h * 2.0 + 6.0) * 0.5;
            let below = (above).max(track_half_h + tick_room) + label_room;
            let content_h = above + below;
            // Centre the assembly, and never push it off the top of a rect too
            // short to hold it.
            let cy = (rect.min.y + above + ((rect.height() - content_h) * 0.5).max(0.0))
                .min(rect.max.y - below.min(rect.height()));
            h_label_y = (cy + below - 1.0).min(rect.max.y - 1.0);
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
            // 050 — the filled range, start → knob. At zero there is nothing to
            // fill, which is what "transparent when 0" means.
            if let Some(fill_c) = range_fill {
                let filled = egui::Rect::from_min_max(
                    Pos2::new(track_l, track_rect.min.y),
                    Pos2::new(thumb_x, track_rect.max.y),
                );
                if filled.width() > 0.5 {
                    painter.rect_filled(filled, track_half_h, fill_c);
                }
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
            if theme_fill.is_some() {
                draw_flat_pill(painter, thumb_rect, thumb_body, thumb_rim);
            } else {
                draw_glass_pill(painter, thumb_rect, thumb_body, true, thumb_rim);
                // Lens at bottom-center of thumb
                draw_lens(
                    painter,
                    Pos2::new(thumb_x, thumb_rect.max.y - thumb_h * 0.28),
                    thumb_w_half * 0.6,
                    thumb_h * 0.18,
                );
            }
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
                Pos2::new(rect.min.x + 2.0, h_label_y),
                egui::Align2::LEFT_BOTTOM,
                format!("{}", min_v as i64),
                font_s.clone(),
                lbl_c,
            );
            painter.text(
                Pos2::new(rect.max.x - 2.0, h_label_y),
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
        // The designed background gradient, under the dial, the gauge, the
        // switch or the drop zone — these four paint their own artwork and
        // return before the generic frame code that reads the property.
        paint_background_gradient(
            painter,
            rect,
            themed_corner_radius(painter.ctx(), ctrl).into(),
            ctrl,
            alpha_mul,
        );
        // Under Elegance these read from the REAL palette — it turns out to be
        // public and constructible without a `Ui`/`Context`, so the designer
        // canvas can match the live widget exactly instead of approximating it
        // (spec 047). Liquid Glass keeps the original hand-picked values so
        // existing forms are untouched (AC8).
        use crate::surface_theme::{AccentName, ColorToken as Tok};
        let accent_color = |name: &str| -> Color32 {
            // A COLOUR, not just one of six names. The property is edited with a
            // colour picker now (the Switch's "Checked color" row), which writes
            // `#RRGGBB[AA]` — and a hex string used to fall through this whole
            // table to plain blue, so every colour the operator chose but the
            // six painted the same (operator, 2026-08-22). Read exactly as
            // `knob_accent` reads it, so the two cannot disagree about the same
            // property.
            if let Some(hex) = name.strip_prefix('#') {
                if matches!(hex.len(), 6 | 8) && hex.chars().all(|c| c.is_ascii_hexdigit()) {
                    return parse_color(name);
                }
            }
            if let Some(c) =
                theme_token(painter.ctx(), Tok::Accent(AccentName::parse(name)))
            {
                return c;
            }
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
                draw_knob(painter, rect, ctrl, alpha_mul, a);
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
                // The meter's own colours. `Color` names the needle/bar as it
                // always has; where the developer left it alone the control's
                // `ForegroundColor` paints the meter and `BackgroundColor` its
                // track — the two properties every other control honours, which
                // a Gauge simply ignored.
                let color_prop = ctrl
                    .get_prop("Color")
                    .map(|v| v.as_str().to_owned())
                    .unwrap_or_default();
                let user_fg = ctrl
                    .get_prop("ForegroundColor")
                    .map(|v| parse_color(v.as_str()))
                    .filter(|c| c.a() > 0 && *c != parse_color(crate::model::DEFAULT_FOREGROUND_COLOR));
                // Zones win when the developer set both thresholds — recolouring
                // the fill as the reading crosses them is the whole point of
                // asking for them, so they outrank a fixed `Color` (R8/AC2).
                let fill = alpha_color(match gauge_zone_color(ctrl, frac) {
                    Some(zone) => zone,
                    None if !color_prop.is_empty() => parse_color(&color_prop),
                    None => user_fg.unwrap_or_else(|| accent_color("Blue")),
                });
                let track = alpha_color(
                    user_background_color(ctrl).unwrap_or_else(|| {
                        theme_token(painter.ctx(), Tok::Border).unwrap_or(Color32::from_gray(140))
                    }),
                );
                // The needle's own colour, once the developer asks for one. Left
                // blank it is the meter's colour, which is the only ink the
                // needle has ever had — so an untouched gauge draws as before.
                let needle_ink = {
                    let named = ctrl
                        .get_prop("NeedleColor")
                        .map(|v| v.as_str().to_owned())
                        .unwrap_or_default();
                    if named.trim().is_empty() {
                        fill
                    } else {
                        alpha_color(parse_color(&named))
                    }
                };
                // What it reads out: the developer's own `Text` when they set
                // one, otherwise the value with `Unit` after it.
                let reading = {
                    let over = ctrl
                        .get_prop("Text")
                        .map(|v| v.as_str().to_owned())
                        .unwrap_or_default();
                    if over.trim().is_empty() {
                        let unit = ctrl
                            .get_prop("Unit")
                            .map(|v| v.as_str().to_owned())
                            .unwrap_or_default();
                        format!("{val:.0}{}{unit}", unit_gap(&unit))
                    } else {
                        over
                    }
                };
                // The reading is measured BEFORE the meter is laid out: a Radial
                // asked to print its number under the pivot has to give up that
                // much room at the bottom, and the text's own height is how much.
                let readout_font = crate::fonts::font_id(
                    painter.ctx(),
                    &ctrl
                        .get_prop("FontName")
                        .map(|v| v.as_str().to_owned())
                        .unwrap_or_default(),
                    ctrl_font_size(ctrl),
                );
                let readout_h = painter
                    .layout_no_wrap(reading.clone(), readout_font.clone(), Color32::WHITE)
                    .size()
                    .y;
                // Radial only (Donut reads out in its hole, Linear under its
                // bar — neither has a second place to put it). "Radial" is
                // everything the style match does not claim, exactly as below.
                let readout_down = !matches!(style.as_str(), "Linear" | "Donut")
                    && ctrl
                        .get_prop("ReadoutPosition")
                        .is_some_and(|v| v.as_str().eq_ignore_ascii_case("Down"));
                // Where the reading goes, per style. It used to be dropped at
                // the control's centre whatever the meter was doing — which on
                // a Radial is the middle of the sweep, so the number sat on the
                // band it was reporting.
                let (value_pos, value_anchor) = match style.as_str() {
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
                        // The thumb marks where the reading sits, riding proud
                        // of the bar so it reads against both halves of it.
                        if gauge_flag(ctrl, "ShowThumb") {
                            painter.circle(
                                Pos2::new(bar.min.x + bar.width() * frac, bar.center().y),
                                (h * 0.70).max(4.0),
                                lighten(fill, 0.35),
                                Stroke::new(1.5, fill),
                            );
                        }
                        // Above the bar when the control leaves room, otherwise
                        // on it — never half-covered by it.
                        if rect.top() + 4.0 < bar.top() - 2.0 {
                            (
                                Pos2::new(rect.center().x, bar.top() - 3.0),
                                egui::Align2::CENTER_BOTTOM,
                            )
                        } else {
                            (rect.center(), egui::Align2::CENTER_CENTER)
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
                        // The needle, on the hub it turns about — the same
                        // `ShowNeedle` the Radial honours, over the Donut's own
                        // sweep (from the top, clockwise). It reaches the band's
                        // inner edge, and the readout paints after it, so the
                        // number stays legible on top.
                        if gauge_flag(ctrl, "ShowNeedle") {
                            let a = (-90.0 + 360.0 * frac).to_radians();
                            let dir = Vec2::new(a.cos(), a.sin());
                            let reach = (radius - stroke_w * 0.5).max(radius * 0.3);
                            painter.line_segment(
                                [center, center + dir * reach],
                                Stroke::new((radius * 0.06).max(1.5), needle_ink),
                            );
                            painter.circle_filled(center, (radius * 0.10).max(2.5), needle_ink);
                        }
                        // The hole is exactly where a donut's reading belongs.
                        (center, egui::Align2::CENTER_CENTER)
                    }
                    _ => {
                        // Radial: a half-circle speedometer, sweeping the top.
                        // A readout asked to sit under the pivot needs room the
                        // dial is otherwise using, so the dial gives it up — its
                        // own height plus the 5 px gap — and the number lands
                        // below the needle instead of off the control's edge.
                        let reserve = if readout_down { readout_h + 5.0 } else { 0.0 };
                        let center = Pos2::new(rect.center().x, rect.bottom() - 6.0 - reserve);
                        let radius = (rect.width() * 0.5 - 4.0)
                            .min(rect.height() - 10.0 - reserve)
                            .max(6.0);
                        draw_ring(painter, center, radius, radius * 0.18, 180.0, 180.0, frac, fill, track);
                        // The scale: ten divisions across the sweep, just inside
                        // the band, with a longer mark at each end and the
                        // half-way point — the marks the needle is read against.
                        let ray = |deg: f32| -> Vec2 {
                            let a = deg.to_radians();
                            Vec2::new(a.cos(), a.sin())
                        };
                        if gauge_flag(ctrl, "ShowScale") {
                            let band_in = radius * 0.91;
                            for i in 0..=10 {
                                let dir = ray(180.0 + 18.0 * i as f32);
                                let major = i % 5 == 0;
                                let len = radius * if major { 0.16 } else { 0.09 };
                                painter.line_segment(
                                    [center + dir * (band_in - len), center + dir * band_in],
                                    Stroke::new(if major { 2.0 } else { 1.0 }, track),
                                );
                            }
                        }
                        // The needle points at the reading, on the hub it turns
                        // about. Drawn before the readout, so the number stays
                        // legible when the needle sweeps under it.
                        if gauge_flag(ctrl, "ShowNeedle") {
                            let dir = ray(180.0 + 180.0 * frac);
                            painter.line_segment(
                                [center, center + dir * radius * 0.78],
                                Stroke::new((radius * 0.06).max(1.5), needle_ink),
                            );
                            painter.circle_filled(center, (radius * 0.10).max(2.5), needle_ink);
                        }
                        if readout_down {
                            // Under the pivot, 5 px clear of it, where a
                            // speedometer prints its number.
                            (
                                Pos2::new(center.x, center.y + 5.0),
                                egui::Align2::CENTER_TOP,
                            )
                        } else {
                            // Inside the dial, centred on the sweep's own centre
                            // and well clear of the band at `radius`.
                            (
                                Pos2::new(center.x, center.y - radius * 0.30),
                                egui::Align2::CENTER_BOTTOM,
                            )
                        }
                    }
                };
                // The reading in the control's own font and colour, rescued so it
                // stays legible on whatever the gauge sits on.
                let tone = control_surface_tone(
                    painter.ctx(),
                    ctrl,
                    parse_color(crate::model::DEFAULT_BACKGROUND_COLOR),
                );
                let text_colour = caret_color(
                    tone,
                    user_fg.unwrap_or(Color32::from_rgb(230, 230, 230)),
                );
                painter.text(
                    value_pos,
                    value_anchor,
                    reading,
                    readout_font,
                    Color32::from_rgba_premultiplied(
                        text_colour.r(),
                        text_colour.g(),
                        text_colour.b(),
                        a,
                    ),
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
                let off = alpha_color(
                    theme_token(painter.ctx(), Tok::Border).unwrap_or(Color32::from_gray(110)),
                );
                // The switch FILLS its control, the way the real widget does
                // when Run Form hands it the same rect.
                //
                // The track used to be capped at 32x18 whatever the control's
                // size, so dragging a resize handle moved the grips and left the
                // switch exactly as it was — the operator's report — and a
                // switch sized in the designer did not match the one that ran.
                let track_h = rect.height().max(4.0);
                let track = egui::Rect::from_center_size(
                    rect.center(),
                    Vec2::new(rect.width().max(track_h), track_h),
                );
                let r = track_h * 0.5;
                // 050 — a switch is a TOGGLE, but its ON colour is the
                // developer's `Accent`, not the theme's.
                //
                // The theme supplied both states here for a while, which meant a
                // switch on the canvas was green whatever Accent said — while
                // Run Form, which uses the real widget, honoured it. The two
                // surfaces disagreed about the same control. A theme decides how
                // a control LOOKS where the developer expressed no preference;
                // `Accent` IS that preference, so it wins (R9). Only the OFF
                // track — which has no property of its own — takes the theme.
                match active_surface_theme(painter.ctx())
                    .surface(SurfaceRole::Toggle, SurfaceState { selected: false, on: checked })
                {
                    Some(spec) if !checked => {
                        // Themed OFF: an outline, not a fill — the state with no
                        // property of its own, so the theme decides it.
                        painter.rect_stroke(
                            track,
                            r,
                            Stroke::new(spec.border_width, theme_alpha(spec.border, alpha_mul)),
                            egui::StrokeKind::Inside,
                        );
                    }
                    // Themed ON, and every Liquid Glass state: exactly the one
                    // fill this has always drawn, so the glass rendering does
                    // not move (R21).
                    _ => {
                        painter.rect_filled(track, r, if checked { accent } else { off });
                    }
                }
                let knob_d = (track_h - 4.0).max(2.0);
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
                let stroke = Stroke::new(
                    1.5,
                    match theme_token(painter.ctx(), Tok::Border) {
                        Some(c) => theme_alpha(c, alpha_mul),
                        None => Color32::from_rgba_premultiplied(140, 140, 140, a),
                    },
                );
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
                    match theme_token(painter.ctx(), Tok::DimText) {
                        Some(c) => theme_alpha(c, alpha_mul),
                        None => Color32::from_rgba_premultiplied(180, 180, 180, a),
                    },
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
        // Under the Neumorphic register the relief IS the drop shadow —
        // `drop_shadow_spec` returns `None` for every control while it is on,
        // and each branch draws the halo instead. This branch drew neither and
        // returned, so a Maps control with `ShadowEnabled` ticked, a distance
        // and a blur set had no shadow of any kind: the one control on the form
        // sitting flat on the surface (operator, 2026-08-21).
        //
        // Before the tiles, like ProgressBar's: a basemap is opaque, so a halo
        // painted afterwards would be buried under it.
        if is_neumorphic {
            draw_neumorphic_shadow_only(
                painter,
                rect,
                themed_corner_radius(painter.ctx(), ctrl),
                alpha_mul,
            );
        }
        // The designed background gradient, under the tiles — it is what shows
        // while they load, and on the surfaces that draw no tiles at all.
        paint_background_gradient(
            painter,
            rect,
            themed_corner_radius(painter.ctx(), ctrl).into(),
            ctrl,
            alpha_mul,
        );
        // A caller drawing the LIVE map (the run form, which pans, zooms and
        // shows its own info window) wants this face and not the canvas's
        // stand-in basemap — the same contract `CaptionMode` carries for a
        // caption. Without it the run path could not draw the face at all, so a
        // map at run time had no shadow and no gradient, however they were set
        // (operator, 2026-08-21: "the dropshadow disappears when running the
        // form"). The tiles below are the stand-in; everything above is the
        // face, and both callers want the face.
        if matches!(*caption_mode, CaptionMode::Skip) {
            return;
        }
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
                id: &m.id,
                info: &m.info,
            })
            .collect();
        // The designer face draws routes and regions too — a territory map you
        // cannot see while laying it out is not a design surface.
        let routes = crate::model::parse_map_routes(
            &ctrl
                .get_prop("Routes")
                .map(|v| v.as_str().to_owned())
                .unwrap_or_default(),
        );
        let regions = crate::model::parse_map_regions(
            &ctrl
                .get_prop("Regions")
                .map(|v| v.as_str().to_owned())
                .unwrap_or_default(),
        );
        // The designer face has no pointer of its own and no open card: the
        // canvas is for laying a map out, not for driving it.
        crate::map_tiles::paint_map(
            painter,
            rect,
            center_lat,
            center_lng,
            zoom,
            &markers,
            &routes,
            &regions,
            crate::map_tiles::MapPointer::default(),
            "",
            "",
            &crate::map_tiles::InfoStyle::default(),
            // The canvas draws pins, routes and territories, so it reads the
            // map's colour properties too: a marker that is one colour while
            // you lay the form out and another while it runs is not a design
            // surface.
            &crate::map_tiles::MapColors::from_control(ctrl),
        );
        return;
    }

    // ── ProgressBar ───────────────────────────────────────────────────────────
    if matches!(ctrl.control_type, CT::ProgressBar) {
        // Spec 016's canonical radius drives the WHOLE control — trough, fill
        // and border. It used to reach only the neumorphic halo while the
        // artwork underneath was hard-wired to 2 px, so the property moved
        // nothing a developer could see.
        let corner = themed_corner_radius(painter.ctx(), ctrl);
        // In Neumorphic mode draw the dual-shadow halo BEFORE the bar's own artwork.
        if is_neumorphic {
            draw_neumorphic_shadow_only(painter, rect, corner, alpha_mul);
        }
        let bg_c = Color32::from_rgba_premultiplied(220, 220, 220, a);
        // The developer's BarColor is the fill in every style. The glass path
        // handed the frosted painter a hard-wired green, which is why the
        // property moved the swatch and left the bar untouched.
        //
        // Untouched, the bar takes the THEME's green rather than the built-in
        // one — the same "still on the seeded default means the developer has
        // not chosen" rule the background and foreground colours follow. A bar
        // was the one control that ignored the palette it sat in: under
        // Elegance every other control drew from the theme while the bar stayed
        // its own shade of green.
        let bar_base = ctrl
            .get_prop("BarColor")
            .filter(|v| {
                !v.as_str()
                    .trim()
                    .eq_ignore_ascii_case(crate::model::DEFAULT_BAR_COLOR)
            })
            .map(|v| parse_color(v.as_str()))
            .or_else(|| {
                theme_token(
                    painter.ctx(),
                    crate::surface_theme::ColorToken::Accent(
                        crate::surface_theme::AccentName::Green,
                    ),
                )
                .map(|c| theme_alpha(c, 1.0))
            })
            .unwrap_or(Color32::from_rgb(0, 170, 0));
        let bar_c = alpha_color(bar_base);
        let val = ctrl.get_prop("Value").map(|v| v.as_i64()).unwrap_or(0) as f32;
        let min = ctrl.get_prop("Minimum").map(|v| v.as_i64()).unwrap_or(0) as f32;
        let max = ctrl
            .get_prop("Maximum")
            .map(|v| v.as_i64())
            .unwrap_or(100)
            .max(1) as f32;
        let pct = ((val - min) / (max - min)).clamp(0.0, 1.0);
        let vertical = ctrl
            .get_prop("Orientation")
            .map(|v| v.as_str().starts_with(['V', 'v']))
            .unwrap_or(false);
        let blocks = ctrl
            .get_prop("Style")
            .map(|v| v.as_str().eq_ignore_ascii_case("Blocks"))
            .unwrap_or(false);
        // The trough is the part of the bar NOT yet travelled, and it is the
        // developer's BackgroundColor — the Appearance "Back colour" row was
        // dead here, because the trough only ever asked the theme. Same
        // precedence the fill above uses: a colour the developer actually
        // chose wins; still on the seeded default means they have not chosen,
        // so the theme's well colour answers; failing that, the built-in grey.
        let bg_c = ctrl
            .get_prop("BackgroundColor")
            .map(|v| v.as_str().to_owned())
            .filter(|raw| {
                let t = raw.trim().trim_start_matches('#');
                !t.is_empty()
                    && !t.eq_ignore_ascii_case(
                        crate::model::DEFAULT_BACKGROUND_COLOR.trim_start_matches('#'),
                    )
            })
            .map(|raw| parse_color(&raw))
            .filter(|c| c.a() > 0)
            .map(alpha_color)
            .or_else(|| {
                theme_token(painter.ctx(), crate::surface_theme::ColorToken::InputBg)
                    .map(|c| theme_alpha(c, alpha_mul))
            })
            .unwrap_or(bg_c);
        // The trough is this control's background face, so a designed gradient
        // paints it exactly as a designed Back colour does. Without this the
        // gradient rows sat in the inspector doing nothing, because the bar
        // returns long before the generic frame code that reads them.
        if !paint_background_gradient(painter, rect, corner.into(), ctrl, alpha_mul) {
            painter.rect_filled(rect, corner, bg_c);
        }
        // A vertical bar fills bottom → top, the way a column of liquid rises;
        // a horizontal one fills left → right.
        let filled = if vertical {
            egui::Rect::from_min_max(
                Pos2::new(rect.min.x, rect.max.y - rect.height() * pct),
                rect.max,
            )
        } else {
            egui::Rect::from_min_size(rect.min, Vec2::new(rect.width() * pct, rect.height()))
        };
        let block_len = blocks.then(|| progressbar_block_len(ctrl, rect, vertical));
        for seg in progressbar_segments(filled, vertical, block_len) {
            // A short block must not over-round into a lozenge.
            let seg_corner = corner.min(seg.width().min(seg.height()) * 0.5);
            if glass {
                draw_surface_auto(
                    painter,
                    seg,
                    bar_base,
                    seg_corner,
                    false,
                    alpha_mul,
                    SurfaceRole::Accent,
                );
            } else {
                painter.rect_filled(seg, seg_corner, bar_c);
            }
        }
        // The frame is the developer's, through the same three properties every
        // other bordered control carries and the same painter — so `None` really
        // draws nothing and Fixed3D/Raised/Sunken look here as they do there.
        // It used to be two constants, a grey and a width, that no property
        // could reach.
        let border_style = ctrl
            .get_prop("BorderStyle")
            .map(|v| v.as_str().to_owned())
            .unwrap_or_else(|| "Single".into());
        let user_bw = ctrl
            .get_prop("BorderWidth")
            .map(|v| v.as_i64() as f32)
            .unwrap_or(1.0);
        let border_c = if selected {
            Color32::from_rgba_premultiplied(60, 120, 230, a)
        } else {
            alpha_color(
                ctrl.get_prop("BorderColor")
                    .map(|v| parse_color(v.as_str()))
                    .unwrap_or(Color32::from_rgb(140, 140, 160)),
            )
        };
        draw_control_border(
            painter,
            rect,
            egui::CornerRadius::same(cr8(corner)),
            &border_style,
            if selected { 2.0_f32.max(user_bw) } else { user_bw },
            border_c,
        );
        if ctrl
            .get_prop("ShowValue")
            .map(|v| v.as_bool())
            .unwrap_or(false)
        {
            // The percentage is text, so it takes the developer's
            // ForegroundColor. Until they pick one it carries the universal
            // white sentinel, which is not a choice — and white on the light
            // default trough reads as nothing — so an untouched control falls
            // back to the trough's own text colour instead. (Hard-wiring that
            // fallback is what made the property look dead: legible, but never
            // the colour the developer asked for.)
            let txt_c = ctrl
                .get_prop("ForegroundColor")
                .filter(|v| {
                    !v.as_str()
                        .trim()
                        .eq_ignore_ascii_case(crate::model::DEFAULT_FOREGROUND_COLOR)
                })
                .map(|v| alpha_color(parse_color(v.as_str())))
                .unwrap_or_else(|| {
                    match theme_token(painter.ctx(), crate::surface_theme::ColorToken::Text) {
                        Some(c) => theme_alpha(c, alpha_mul),
                        None => Color32::from_rgba_premultiplied(0, 0, 0, a),
                    }
                });
            painter.text(
                rect.center(),
                egui::Align2::CENTER_CENTER,
                format!("{:.0}%", pct * 100.0),
                egui::FontId::proportional(ctrl_font_size(ctrl)),
                txt_c,
            );
        }
        if is_neumorphic {
            draw_neumorphic_overlay_shadow_only(painter, rect, corner, alpha_mul);
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
    let theme_text = theme_token(painter.ctx(), crate::surface_theme::ColorToken::Text);
    let label_color = ctrl
        .get_prop("ForegroundColor")
        // 047 — a control carries the `#FFFFFF` sentinel until the developer
        // picks a colour, so "absent" here means "still the sentinel", not
        // "no property". A theme that supplies a text colour resolves the
        // sentinel to it — that colour is the whole face of a frameless Label —
        // while an actually-chosen colour still wins (R9).
        .filter(|v| {
            !(theme_text.is_some()
                && v.as_str().trim().eq_ignore_ascii_case(
                    crate::model::DEFAULT_FOREGROUND_COLOR,
                ))
        })
        .map(|v| parse_color(v.as_str()))
        .unwrap_or_else(|| {
            if is_neumorphic {
                neumorphic_default_ink(painter.ctx(), ctrl, fill)
            } else if let Some(c) = theme_text {
                c
            } else {
                default_text
            }
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
    let corner = themed_corner_radius(painter.ctx(), ctrl);
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

    // Where a CheckBox's drop shadow belongs follows its transparency. With a
    // face solid enough to lift off the form (under 30 % transparent) the
    // shadow is the whole frame's, as for any other control. Once the
    // background is mostly or entirely see-through there is no card to raise,
    // and a frame shadow would hang in mid-air around nothing — so the control
    // draws no frame at all and the only thing casting is the tick box itself,
    // which takes its own relief from `draw_glass_auto` on `box_rect` below.
    let checkbox_frameless =
        matches!(ctrl.control_type, CT::CheckBox) && crate::model::transparency_of(ctrl) >= 30;

    // 049 — a SideMenu owns its whole face: `sidebar::paint` fills the rail
    // with the designed BackgroundColor over the form's backdrop. The generic
    // glass frame drew a second, differently-composited fill AND a border, so
    // the canvas showed a bordered grey rail where the preview and the shell
    // showed the designed colour.
    let sidemenu_frameless = matches!(ctrl.control_type, CT::SideMenu);

    // A RadioButton's frame follows its own `BorderStyle`, which is seeded
    // `None` — it is a selection circle and a caption, not a card. `BorderStyle`
    // only ever governed the *explicit* border stroke, so the themed card the
    // control sits on drew a rim around every radio regardless, under every
    // theme, with nothing in the properties pane able to turn it off. Tested in
    // `checkbox_and_radio_button_expose_border_properties`, which has always
    // said a radio "must not draw a frame by default".
    //
    // Ahead of the asset-pack skin and the themed surface below, so this holds
    // in every theme; set a BorderStyle and the frame comes back. (Its sibling
    // the CheckBox reaches the same place through its 100 % default
    // transparency.)
    let radio_frameless = matches!(ctrl.control_type, CT::RadioButton) && border_style == "None";

    let label_frameless = is_label && !background_gradient && user_bg.is_none();

    if label_frameless
        || pic_frameless
        || chart_frameless
        || container_frameless
        || checkbox_frameless
        || radio_frameless
        || sidemenu_frameless
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
        // A border is not a face. "Frameless" here means the control paints no
        // CARD — a Label has no background, a CheckBox is see-through — and that
        // said nothing about the rim, yet the branch returned before any border
        // was drawn. So `BorderStyle`, `BorderColor` and `BorderWidth` sat in the
        // properties pane doing nothing at all on the two controls whose frame is
        // frameless by default (operator, 2026-08-22). An explicitly asked-for
        // border is now drawn on the frame, over no face, exactly where the pane
        // says it will be.
        //
        // Only for those two. The others are frameless because a property SAYS
        // no border — a PictureBox's `ShowFrame`, a container's `HideBackground`
        // — or because the control paints its own whole face (charts, SideMenu);
        // drawing a second rim there would overrule the property that asked for
        // none. A RadioButton needs no entry: its framelessness IS
        // `BorderStyle == "None"`, so it never reaches here with a border to draw.
        if (label_frameless || checkbox_frameless)
            && border_style != "None"
            && user_border_width > 0.5
        {
            draw_control_border(
                painter,
                frame_rect,
                frame_round,
                &border_style,
                if selected {
                    2.0_f32.max(user_border_width)
                } else {
                    user_border_width
                },
                if selected {
                    Color32::from_rgba_premultiplied(60, 120, 230, a)
                } else {
                    alpha_color(stroke_color)
                },
            );
        } else if selected {
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
            draw_neumorphic_shadow_only(painter, frame_rect, frame_round, face_alpha);
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
            draw_neumorphic_overlay_shadow_only(painter, frame_rect, frame_round, face_alpha);
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
            // Through `draw_control_border`, like every other frame branch. This
            // was a bare `rect_stroke`, which draws ONE flat line whatever the
            // style says — so Fixed3D, Raised and Sunken all collapsed to Single
            // the moment a background gradient was switched on, and switching it
            // back restored them. That read exactly as the border style being
            // reset by the gradient (operator, 2026-08-22).
            if selected {
                painter.rect_stroke(
                    border_rect,
                    frame_round,
                    Stroke::new(2.0, bc),
                    egui::StrokeKind::Middle,
                );
            } else {
                draw_control_border(
                    painter,
                    border_rect,
                    frame_round,
                    &border_style,
                    user_border_width,
                    bc,
                );
            }
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
                face_alpha,
            );
        }
    } else if let Some(theme_face) = {
        // An explicit BackgroundColor is the developer's call and outranks the
        // theme (R9) — it takes the caller-led Shape role.
        let role = match user_bg {
            Some(_) => SurfaceRole::Shape,
            None => elegance_role_for(&ctrl.control_type),
        };
        active_surface_theme(painter.ctx()).surface(role, SurfaceState { selected, on: false })
    } {
        // ── 047/050 — the theme's own face ────────────────────────────────
        // Ordered after the asset-pack branch (a pack still wins where it
        // covers a control) and before glass. Flat fill + hairline border, at
        // the control's exact designed rect — which is why this is painted
        // rather than delegated to a crate widget: a widget would render at its
        // own intrinsic size and the designer canvas, which has no `Ui` at all,
        // could not run one anyway (spec 047 Q5).
        let eleg_rect = if is_container {
            debug_frame(
                painter,
                frame_rect,
                frame_round,
                1,
                "CONTAINER_ELEGANCE",
                container_diag,
            )
        } else {
            frame_rect
        };
        // `base` is what a caller-led role paints with: the developer's colour
        // when they set one, the computed fill otherwise.
        let base = user_bg.unwrap_or(fill);
        draw_theme_surface(
            painter,
            eleg_rect,
            base,
            frame_round,
            face_alpha,
            &theme_face,
        );
        // A user-set BorderStyle/BorderWidth still draws on top, as under glass
        // — same threshold and rect derivation the glass branch below uses.
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
            painter.rect_stroke(
                border_rect,
                frame_round,
                Stroke::new(
                    if selected {
                        2.0_f32.max(user_border_width)
                    } else {
                        user_border_width
                    },
                    stroke_color,
                ),
                egui::StrokeKind::Middle,
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
            face_alpha,
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
            // 050 — a radio's state was a CHARACTER in its caption, `(●)`/`( )`,
            // so there was no indicator to colour and a theme could not style it
            // at all. Every theme now draws the real circle (below), so no
            // caption anywhere carries a glyph.
            let _ = checked;
            cap
        }
        CT::ComboBox => {
            // What the RUNNING header shows: the chosen `Value`, or the first
            // item in the order the list actually displays them.
            //
            // It used to be the first item as TYPED, so a combo with `Sorted`
            // on read one thing on the canvas and another the moment the form
            // ran — which is how a working sort looked broken (operator,
            // 2026-08-18).
            let value = ctrl
                .get_prop("Value")
                .map(|v| v.as_str().to_owned())
                .unwrap_or_default();
            let shown = if value.is_empty() {
                let mut items: Vec<String> = ctrl
                    .get_prop("Items")
                    .map(|v| v.as_str().to_owned())
                    .unwrap_or_default()
                    .lines()
                    .map(|l| l.to_owned())
                    .collect();
                list_display_items(ctrl, &mut items);
                items.first().cloned().unwrap_or_default()
            } else {
                value
            };
            format!("{shown} ▾")
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
            // The value alone, centred in its field — the face the preview and
            // the running form show. The canvas used to letter "▲▼" into the
            // caption, so the RAD drew a control that existed nowhere else.
            let v = ctrl.get_prop("Value").map(|v| v.as_i64()).unwrap_or(0);
            format!("{v}")
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
        // The canvas draws the REAL tree (below), like every other control that
        // carries content — a placeholder here would be painted underneath it.
        CT::TreeView => String::new(),
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
        // A SideMenu centres NOTHING: its ☰, its items and its empty hint are
        // all painted top-anchored down the rail, further below.
        CT::SideMenu => String::new(),
        // A toolbar with buttons draws them (further below); only an empty one
        // needs to say what it is.
        CT::ToolBar => {
            if crate::toolbar::ToolbarDef::from_control(ctrl).is_empty() {
                "⬛ ToolBar (empty)".into()
            } else {
                String::new()
            }
        }
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
    // A caller drawing the control's own live content wants the face and not
    // the canvas's stand-in for it (`draw_control_face`). Dropped here, after
    // both branches, so the hint and the caption are silenced by one rule.
    let label = if matches!(*caption_mode, CaptionMode::Skip) {
        String::new()
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
        let (box_d, pad, gap) = toggle_indicator_metrics(rect, ctrl);
        let box_round = (box_d * 0.22).clamp(2.0, 5.0);
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
        // 050 — a check box is a TOGGLE, so a theme that distinguishes on from
        // off paints it accordingly (filled when checked, an empty rim when
        // not). Liquid Glass supplies no Toggle surface, so it keeps the
        // recessed Input well it has always drawn.
        let toggle_spec = active_surface_theme(painter.ctx())
            .surface(SurfaceRole::Toggle, SurfaceState { selected: false, on: checked });
        // `CheckBoxColor` is the BOX's own colour, and when the developer names
        // one it LEADS — the theme's toggle fill is a default, not a clamp.
        //
        // The box used to be painted from `fill`, the control's BackgroundColor,
        // which is the FRAME's colour and reached here only as a hint the
        // painters were free to ignore: a theme answered `spec.fill.unwrap_or
        // (base)` and never reached `base`, and Liquid Glass turned it into a
        // ~3.5 % frost tint. So one property drove two surfaces and was visible
        // on neither (operator, 2026-08-22).
        match (&toggle_spec, user_checkbox_color(ctrl)) {
            (_, Some(bg)) => draw_surface_auto_bg(
                painter,
                box_rect,
                bg,
                Some(bg),
                box_round,
                false,
                alpha_mul,
                SurfaceRole::Toggle,
            ),
            (Some(spec), None) => {
                draw_theme_surface(painter, box_rect, fill, box_round, alpha_mul, spec)
            }
            (None, None) => draw_surface_auto(
                painter,
                box_rect,
                fill,
                box_round,
                false,
                alpha_mul,
                SurfaceRole::Input,
            ),
        }
        // The box's own border, separate from the frame's: a tick box is a
        // surface in its own right, and the frame's `BorderStyle` is the rim
        // around the whole control. Seeded `None`, so the theme's own rim is
        // what an untouched check box shows.
        let (box_style, box_bw, box_bc) = checkbox_box_border(ctrl);
        draw_control_border(
            painter,
            box_rect,
            egui::CornerRadius::same(cr8(box_round)),
            &box_style,
            box_bw,
            alpha_color(box_bc),
        );
        if checked {
            let cc = alpha_color(toggle_mark_color(
                painter,
                ctrl,
                toggle_spec.as_ref().and_then(|spec| spec.fill),
            ));
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

    // 050 / 2026-08-22 — the RadioButton's real indicator, on EVERY theme.
    //
    // Its state used to be a character in the caption (`(●)` / `( )`), which no
    // theme could style. Elegance was then given a real drawn circle, and it was
    // drawn ONLY where a theme described a Toggle surface — so the same control
    // was a proper indicator on one theme and a pair of parentheses on all the
    // others (operator, 2026-08-22: "use Elegance's radio button on all themes,
    // circle empty/filled").
    //
    // The SHAPE is the platform's now, not a theme's: filled when on, an empty
    // rim when off, everywhere. A theme that describes a Toggle still supplies
    // the COLOURS (Elegance is unchanged, to the pixel); one that does not gets
    // the control's own `CheckColor` — the property that already colours a
    // CheckBox's tick, and a radio's dot is that tick.
    if matches!(ctrl.control_type, CT::RadioButton) {
        let checked = ctrl
            .get_prop("Checked")
            .map(|v| v.as_bool())
            .unwrap_or(false);
        let (d, pad, gap) = toggle_indicator_metrics(rect, ctrl);
        let c = Pos2::new(rect.left() + pad + d * 0.5, rect.center().y);
        let (fill, rim, rim_w) = radio_indicator_colors(painter.ctx(), ctrl, checked);
        // A radio is round: the same fill and rim, as a circle rather than a
        // rounded square — and the same two properties the CheckBox's box
        // carries, since the circle IS the radio's box. `CheckBoxColor` leads
        // over the fill; the box border rides on top of the rim, as a circle.
        painter.circle_filled(
            c,
            d * 0.5,
            match user_checkbox_color(ctrl) {
                Some(bg) => alpha_color(bg),
                None => theme_alpha(fill, alpha_mul),
            },
        );
        painter.circle_stroke(c, d * 0.5, Stroke::new(rim_w, theme_alpha(rim, alpha_mul)));
        let (box_style, box_bw, box_bc) = checkbox_box_border(ctrl);
        if box_style != "None" && box_bw > 0.5 {
            painter.circle_stroke(c, d * 0.5, Stroke::new(box_bw, alpha_color(box_bc)));
        }
        checkbox_text_rect =
            egui::Rect::from_min_max(egui::pos2(c.x + d * 0.5 + gap, rect.min.y), rect.max);
    }

    if !label.is_empty() {
        // A CheckBox has no face of its own by default, so its caption sits on
        // whatever it was dropped onto — a GroupBox, a Panel, a dark form, a
        // Neumorphic Dark surface. The seeded default is plain black, which is
        // unreadable on half of those. Keep the developer's colour whenever it
        // already clears WCAG AA against what is actually behind it, and
        // otherwise fall to the pole that reads: `caret_color` picks by ratio,
        // so it clears AA on ANY surface, where a luminance threshold would
        // still leave ~3.5:1 on a mid grey.
        // A RadioButton's caption is rescued by the same rule and for the same
        // reason: it is the other half of the same row, and a caption that
        // disappears on a dark theme is no more usable next to a circle than
        // next to a box.
        // A DateTimePicker's value (and its `DD/MM/YYYY` mask) is rescued too:
        // it is the control's whole content, and the seeded white foreground
        // disappears the moment the field's own surface is pale.
        let label_color = if matches!(
            ctrl.control_type,
            CT::CheckBox | CT::RadioButton | CT::DateTimePicker
        ) {
            // `draw_control` is not given the form's own backdrop (it is a
            // parameter of the render walk, not of the painter), so the neutral
            // default stands in for it. That only matters under Classic /
            // Enhanced, where the frost composites what is behind; the
            // Neumorphic surfaces are solid and answer exactly, which is the
            // case that was actually unreadable — black-on-dark.
            //
            // `caption_surface_tone`, not `control_surface_tone`: the caption
            // sits on the FRAME, and a toggle's BackgroundColor is its box. A
            // frame too transparent to read (a CheckBox at its 100 % default)
            // answers `None`, and the developer's colour is left alone.
            match caption_surface_tone(
                painter.ctx(),
                ctrl,
                parse_color(crate::model::DEFAULT_BACKGROUND_COLOR),
            ) {
                Some(behind) => caret_color(behind, label_color),
                None => label_color,
            }
        } else {
            label_color
        };
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
            // Shrink to fit the button rather than run past its border. It laid
            // out at an INFINITE wrap width, so a caption simply never fitted.
            let bpad = 3.0_f32.min(rect.width() * 0.2);
            let galley = fitted_caption_galley(
                painter,
                ctrl,
                &label,
                &font_name,
                fsize,
                txt_color,
                (rect.width() - 2.0 * bpad).max(1.0),
                (rect.height() - 2.0 * bpad).max(1.0),
                egui::Align::LEFT,
            );
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

            // A Label is frameless — its text IS the control — so an oversized
            // caption used to run straight over its neighbours. It shrinks now,
            // like every other caption.
            let lpad = 3.0_f32.min(rect.width() * 0.25);
            let galley = if text_justified(&align_raw) {
                // Justified text fills the width by construction; keep the job
                // path so `justify` is honoured.
                let mut job = styled_text_job(
                    painter, ctrl, &label, &font_name, fsize, txt_color, rect.width(), halign,
                );
                job.justify = true;
                painter.layout_job(job)
            } else {
                fitted_caption_galley(
                    painter,
                    ctrl,
                    &label,
                    &font_name,
                    fsize,
                    txt_color,
                    (rect.width() - 2.0 * lpad).max(1.0),
                    rect.height().max(1.0),
                    halign,
                )
            };
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
            // The run form takes the caption from here and draws it itself, so
            // the reader can select and copy it. Same galley, same position,
            // same colour — the only difference is who calls the painter.
            match caption_mode {
                CaptionMode::Capture(out) => {
                    **out = Some(CaptionLayout {
                        galley,
                        pos: text_pos,
                        color: txt_color,
                    });
                }
                _ => paint_styled_galley(painter, ctrl, text_pos, galley, txt_color),
            }
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
        } else if matches!(ctrl.control_type, CT::CheckBox | CT::RadioButton) {
            // Caption sits in the space left after the check glyph (drawn
            // below, outside this `!label.is_empty()` gate), wrapped, clipped,
            // and shrunk exactly like TextBox's single-line box — so a long
            // caption never bleeds past the control's own border instead of
            // overflowing it (developer-reported bug: text used to spill past
            // the frame).
            //
            // A RadioButton lays out by the same rule: its caption belongs to
            // the RIGHT of the selection circle, exactly as far from it as a
            // CheckBox's caption is from its box (`checkbox_text_rect` carries
            // the same `gap`). It used to fall through to the generic branch
            // below, which centres the caption in the WHOLE control rect — so
            // the circle was drawn on top of the first few characters. Where no
            // themed circle is drawn (Liquid Glass keeps its `(●)` glyph in the
            // caption) that rect is the full control, so the row reads the same
            // way: indicator first, caption after it.
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
            // Width as well as height: a single-word caption cannot wrap, so it
            // overflows sideways at a height that fits.
            while (galley.size().y > checkbox_text_rect.height()
                || galley.size().x > inner_w)
                && fit > min_font
            {
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
            // Every other caption-bearing control — Button, Label, RadioButton,
            // GroupBox and the rest — shrinks to fit, exactly as the TextBox and
            // the CheckBox already did. This branch laid the caption out at the
            // requested size and centred it, so a caption too big for its
            // control simply spilled past the border.
            //
            // BOTH axes, unlike the older loops above: those test height only,
            // which works when the text can wrap. A caption is usually one word
            // ("Button-1"), and a word cannot be broken — so it overflows
            // sideways at a height that fits perfectly well.
            let pad = 3.0_f32.min(rect.width() * 0.2);
            let inner_w = (rect.width() - 2.0 * pad).max(1.0);
            let inner_h = (rect.height() - 2.0 * pad).max(1.0);
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
                    egui::Align::Center,
                ))
            };
            let mut galley = layout(fit);
            while (galley.size().y > inner_h || galley.size().x > inner_w) && fit > min_font {
                fit = (fit - 1.0).max(min_font);
                galley = layout(fit);
            }
            // This job is laid out with `halign = Center`, so the position IS
            // the text's middle — subtracting half the galley's width from the
            // control's centre moved every caption half its own width to the
            // LEFT. On a roomy control the spill went unnoticed; on a narrow one
            // the text hung off the left edge and the clip cut its START, which
            // is how a narrowed DateTimePicker came to show `M/YYYY`.
            //
            // Text that still does not fit at the 6 pt floor is anchored so its
            // LEFT edge sits at the frame: what the clip takes is then the tail,
            // and a truncated value still reads as one.
            let text_x = if galley.size().x > inner_w {
                rect.left() + pad + galley.size().x / 2.0
            } else {
                rect.center().x
            };
            let text_pos = egui::pos2(text_x, rect.center().y - galley.size().y / 2.0);
            // Clipped as a last resort: at the 6pt floor a caption long enough
            // still cannot fit, and cutting it at the border beats bleeding over
            // the neighbouring controls.
            let clipped = painter.with_clip_rect(rect);
            paint_styled_galley(&clipped, ctrl, text_pos, galley, txt_color);
        }
    }

    // ── ToolBar: the real groups and buttons, not a placeholder ──────────────
    //
    // Through the SAME renderer the running form uses, so what the developer
    // arranges on the canvas is what they get. Inert: a canvas has no hover and
    // no press, and clicking a control there selects it.
    if matches!(ctrl.control_type, CT::ToolBar) {
        let def = crate::toolbar::ToolbarDef::from_control(ctrl);
        if !def.is_empty() {
            crate::toolbar_paint::draw(
                painter,
                rect,
                &def,
                alpha_mul,
                crate::toolbar_paint::Interaction::inert(),
            );
        }
    }

    // ── TreeView: the real nodes, by the SHARED renderer ─────────────────────
    //
    // The canvas used to draw `🌲 [TreeView]` and nothing else, so a developer
    // laying out a tree could not see the tree (operator, 2026-08-22: "content
    // not rendered"). It draws what the running form draws now, through the one
    // implementation both call — the same rule the toolbar above follows.
    //
    // No pointer here, so nothing is hot-tracked: a design surface is for
    // laying a tree out, not for driving it.
    if matches!(ctrl.control_type, CT::TreeView) {
        let rows = crate::treeview::layout(ctrl, rect);
        let selected = ctrl
            .get_prop("SelectedNode")
            .map(|v| v.as_str().to_owned())
            .unwrap_or_default();
        let checked: Vec<String> = ctrl
            .get_prop("CheckedNodes")
            .map(|v| v.as_str().lines().map(str::trim).map(str::to_owned).collect())
            .unwrap_or_default();
        crate::treeview::paint(
            painter,
            ctrl,
            rect,
            &rows,
            crate::treeview::TreeState {
                selected: &selected,
                checked: &checked,
                hovered: None,
                alpha: alpha_mul,
            },
        );
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

    // ── SideMenu: the ☰ toggle, then top-level labels down the rail (049) ────
    //
    // Everything a sidebar draws is anchored to its TOP and grows downward: a
    // sidebar is a rail, not a centred caption, and with `FullHeight` on it is
    // as tall as the form — content centred in that would float in the middle
    // of nothing. The ☰ is drawn whether or not the menu has items, mirroring
    // the running shell, where collapsing the pane is the operator's control
    // over the window and never a function of what the menu contains.
    if matches!(ctrl.control_type, CT::SideMenu) {
        let fg_base = ctrl
            .get_prop("ForegroundColor")
            .map(|v| parse_color(v.as_str()))
            .unwrap_or(Color32::from_rgb(225, 230, 250));
        let fg = Color32::from_rgba_premultiplied(fg_base.r(), fg_base.g(), fg_base.b(), a);
        let _ = fg;
        let def = get_menu_cache(painter.ctx(), &ctrl.id);
        let items: &[crate::menu::MenuItem] =
            def.as_ref().map(|d| d.menu.as_slice()).unwrap_or(&[]);
        // The canvas shows the tree OPEN: the developer should see the whole
        // menu while designing instead of running the app to find level two.
        let expanded = crate::sidebar::all_parent_ids(items);
        let mut state = crate::sidebar::state_for_control(painter.ctx(), ctrl, items, a, &expanded);
        // What the rail's (usually translucent) colour resolves against: the
        // form's own backdrop, the same base every other surface passes.
        state.backdrop = form_backdrop_of(painter.ctx());
        let rows = crate::sidebar::layout(rect, &state);
        crate::sidebar::paint(painter, rect, &rows, &state);
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
        .unwrap_or_else(|| {
            // 047 — a Label is frameless: its text IS its face, so the theme's
            // text colour has to reach it here or a label stays off-theme. It
            // takes `LabelText`, not `Text`: a theme may want a caption quieter
            // than the text the operator types into an input.
            theme_token(painter.ctx(), crate::surface_theme::ColorToken::LabelText)
                .unwrap_or_else(|| control_colors(&ctrl.control_type, false).2)
        });
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
/// [`load_image_texture`], but decoded ONCE per path and kept in the context's
/// own memory afterwards.
///
/// Decoding on every frame would re-read and re-upload the file sixty times a
/// second. Each surface used to cache this for itself, which is why a property
/// that resolves to a texture (the sidebar's `HeaderImage`) could be honoured
/// on one surface and silently ignored on the others. `None` is cached too: a
/// path that does not resolve must not be retried every frame either.
pub fn cached_image_texture(ctx: &egui::Context, path: &str) -> Option<egui::TextureHandle> {
    let path = path.trim();
    if path.is_empty() {
        return None;
    }
    let id = egui::Id::new(("cobolt_img_cache", path));
    if let Some(hit) = ctx.memory(|m| m.data.get_temp::<Option<egui::TextureHandle>>(id)) {
        return hit;
    }
    let loaded = load_image_texture(ctx, path);
    ctx.memory_mut(|m| m.data.insert_temp(id, loaded.clone()));
    loaded
}

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
        draw_surface_auto(
            painter,
            rect,
            Color32::from_rgb(20, 30, 60),
            corner,
            false,
            alpha_mul * 0.7,
            SurfaceRole::Card,
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

/// How far the dimmed highlight is lit relative to the active one, when the
/// developer has not named a colour of their own. Half-ish: enough that a
/// selected row is plainly selected, little enough that the ACTIVE row still
/// reads as the one the cursor is on.
pub const LIST_SELECTED_DIM: f32 = 0.45;

/// The two highlights a ListBox draws: the **active** row's, and the one every
/// other row of a multi-select set wears.
///
/// `ActiveItemColor` and `SelectedItemsColor` are the developer's; either left
/// empty falls back, so a list that names neither looks exactly as it did
/// before the properties existed:
///
/// * no `ActiveItemColor` → `theme_fill`, the palette's own selection colour;
/// * no `SelectedItemsColor` → the active colour at [`LIST_SELECTED_DIM`].
///
/// The dim follows whatever the active colour ended up being, so naming only
/// `ActiveItemColor` restyles the whole list and keeps the two related.
/// Resolved here rather than at each call site so the inspector's swatch and
/// the painted row can never disagree about what a list is showing.
pub fn list_selection_fills(ctrl: &Control, theme_fill: Color32) -> (Color32, Color32) {
    let named = |key: &str| ctrl.get_prop(key).and_then(|v| parse_hex(v.as_str()));
    let active = named("ActiveItemColor").unwrap_or(theme_fill);
    let selected =
        named("SelectedItemsColor").unwrap_or_else(|| active.gamma_multiply(LIST_SELECTED_DIM));
    (active, selected)
}

/// The items a list shows, in the order it shows them.
///
/// The developer's own order, or **alphabetical** when the control's `Sorted`
/// is on — a property every list-shaped control has carried, and shown in the
/// inspector, since before it did anything at all (operator, 2026-08-18).
///
/// Sorting is by TEXT, case-insensitively, which is what "alphabetically
/// sorted" means on every RAD a COBOL developer is likely to have used. Numbers
/// therefore sort as the strings they are — `10` before `9` — because a list's
/// items are text and nothing declares them otherwise.
///
/// Ties keep the order the developer typed them in: the sort is stable, so two
/// items differing only in case stay as authored rather than swapping about
/// between runs.
///
/// The stored `Items` is **never** rewritten. What the developer typed is
/// theirs; only the display order changes, so turning `Sorted` off again gives
/// back exactly the list they wrote.
pub fn list_display_items(ctrl: &Control, items: &mut [String]) {
    let sorted = ctrl
        .get_prop("Sorted")
        .map(|v| v.as_bool())
        .unwrap_or(false);
    if sorted {
        items.sort_by(|a, b| {
            let (a, b) = (a.to_lowercase(), b.to_lowercase());
            a.cmp(&b)
        });
    }
}

/// How far a selection band keeps off the border of the list it sits in.
///
/// The border's own width plus this, so the rim reads as one unbroken line with
/// a hairline of background between it and the highlight — rather than
/// something the selection has eaten into.
pub const HIGHLIGHT_INSET: f32 = 2.0;

/// The corner radii a selection band wears inside a rounded list.
///
/// Square, **except** where the band meets the container's own arc: there it is
/// cut by what is left of the radius once the band's inset is taken off, so the
/// highlight follows the border instead of poking straight through it. egui
/// clips to an axis-aligned rect, so a band left to itself paints past the arc
/// and out over the rim — which a short list shows most, because every row of
/// one IS a corner row.
///
/// `band` must already be clipped to `inner` (the container shrunk by its
/// border plus [`HIGHLIGHT_INSET`]), and `corner` is the container's own radius.
///
/// The radius asked for is never more than the band can hold. egui clamps a
/// fill's corner to half its shorter side, and a clamped arc is *smaller* than
/// the container's, so it pokes out past it — the corner-bleed rule that costs
/// this project the most time. Asking only for what fits keeps the stored
/// radius and the drawn one the same thing.
///
/// Shared by the ListBox and the ComboBox's dropdown so the two cannot drift:
/// the dropdown used to paint a flat 4 px round on every band, which leaked out
/// of both ends of a panel rounded any further than that (operator, 2026-08-18).
pub fn highlight_band_rounding(
    band: egui::Rect,
    inner: egui::Rect,
    corner: f32,
) -> egui::CornerRadius {
    let r = (corner - HIGHLIGHT_INSET)
        .max(0.0)
        .min(band.width() * 0.5)
        .min(band.height() * 0.5) as u8;
    let mut cr = egui::CornerRadius::ZERO;
    if band.top() <= inner.top() + 0.5 {
        cr.nw = r;
        cr.ne = r;
    }
    if band.bottom() >= inner.bottom() - 0.5 {
        cr.sw = r;
        cr.se = r;
    }
    cr
}

/// The fill an open ComboBox draws behind its SELECTED item when the developer
/// has not named one. Translucent, so the popup's own surface reads through it.
pub const COMBO_SELECTED_FILL: Color32 = Color32::from_rgba_premultiplied(60, 100, 200, 120);

/// The fill an open ComboBox draws behind the item the pointer is OVER when the
/// developer has not named one. Fainter than [`COMBO_SELECTED_FILL`], so
/// hovering a row never looks like selecting it.
pub const COMBO_HOVER_FILL: Color32 = Color32::from_rgba_premultiplied(50, 70, 150, 80);

/// The two highlights an open ComboBox draws: behind the **selected** item, and
/// behind the item the pointer is **over**.
///
/// `ActiveItemColor` and `HoverItemColor` are the developer's, on the same rule
/// as [`list_selection_fills`]: empty means "not chosen". The fallbacks are the
/// popup's own constants rather than the palette, because — unlike a list's
/// highlights — these two were never theme-derived, so that is what "unchanged"
/// means for a ComboBox already designed.
///
/// `ActiveItemColor` is deliberately the same property name a ListBox carries:
/// on both controls it is the highlight behind the item `Value` /
/// `SelectedIndex` reports. There is no `SelectedItemsColor` here — a ComboBox
/// selects one item or none, so the list's second selection has nothing to
/// colour.
pub fn combo_popup_fills(ctrl: &Control) -> (Color32, Color32) {
    let named = |key: &str| ctrl.get_prop(key).and_then(|v| parse_hex(v.as_str()));
    (
        named("ActiveItemColor").unwrap_or(COMBO_SELECTED_FILL),
        named("HoverItemColor").unwrap_or(COMBO_HOVER_FILL),
    )
}

/// The opaque base an undesigned dropdown lays down, so the list is readable
/// whatever is behind the form.
pub const COMBO_PANEL_BASE: Color32 = Color32::from_rgb(22, 30, 58);
/// The translucent card an undesigned dropdown frosts over [`COMBO_PANEL_BASE`].
pub const COMBO_PANEL_TINT: Color32 = Color32::from_rgb(30, 42, 80);
/// The rim an undesigned dropdown draws around itself.
pub const COMBO_PANEL_BORDER: Color32 = Color32::from_rgba_premultiplied(90, 130, 220, 180);

/// The panel an open dropdown paints for itself.
///
/// Resolved from the `Control` in the pass that still has one — the popup is
/// drawn later, when it is out of reach — on the same rule as
/// [`combo_popup_fills`]: what the developer designed leads, and *undesigned*
/// means **exactly what the dropdown drew before**, not the theme. A ComboBox
/// designed earlier must not restyle itself.
#[derive(Clone, Debug, PartialEq)]
pub struct ComboFace {
    /// The developer's `BackgroundColor`, when they named one.
    pub bg: Option<Color32>,
    /// The designed background gradient: `(start, end, direction)`.
    pub gradient: Option<(Color32, Color32, String)>,
    /// The designed border, `(colour, width)`. `None` = the dropdown's own rim;
    /// a width of `0` = the developer turned the border off.
    pub border: Option<(Color32, f32)>,
    /// The control's own corner radius — the panel hangs off the header and is
    /// cut to the same shape, so the two read as one control.
    pub corner: f32,
}

/// Resolve the panel an open dropdown paints, from the control it belongs to.
pub fn combo_popup_face(ctrl: &Control) -> ComboFace {
    let gradient = ctrl
        .get_prop("BackgroundGradientEnabled")
        .map(|v| v.as_bool())
        .unwrap_or(false)
        .then(|| {
            let colour = |key: &str, fallback: Color32| {
                ctrl.get_prop(key)
                    .map(|v| parse_color(v.as_str()))
                    .unwrap_or(fallback)
            };
            (
                colour("BackgroundGradientStartColor", COMBO_PANEL_BASE),
                colour("BackgroundGradientEndColor", COMBO_PANEL_BASE),
                ctrl.get_prop("BackgroundGradientDirection")
                    .map(|v| v.as_str().to_owned())
                    .unwrap_or_else(|| "South".into()),
            )
        });
    // A ComboBox seeds no border properties at all, so "absent" is the honest
    // reading of "the developer never said" — and that is the case that has to
    // keep the rim it always had.
    let border = match ctrl.get_prop("BorderStyle").map(|v| v.as_str().to_owned()) {
        Some(style) if style.eq_ignore_ascii_case("None") => Some((Color32::TRANSPARENT, 0.0)),
        _ => ctrl.get_prop("BorderColor").map(|v| {
            (
                parse_color(v.as_str()),
                ctrl.get_prop("BorderWidth")
                    .map(|w| w.as_i64() as f32)
                    .unwrap_or(1.0)
                    .clamp(0.0, 20.0),
            )
        }),
    };
    ComboFace {
        bg: user_background_color(ctrl),
        gradient,
        border,
        corner: corner_radius(ctrl),
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
#[derive(Debug, Clone, PartialEq)]
pub enum GlassComboAction {
    /// The operator committed this item — `(index, text)`. The popup closes.
    Select(usize, String),
    /// Dismissed without changing the value: a click outside, or Escape.
    Close,
}

/// A pointer gesture on a ComboBox, carried between the header pass and the
/// popup pass and between frames.
///
/// A drag through a dropdown is **one gesture with an anchor** — the header the
/// press landed on — and what it highlights is the item under the pointer *now*,
/// worked out afresh every frame, so reversing direction walks the highlight
/// back. There is no set of "every item this press has touched": a ComboBox
/// chooses one item, and the accumulating model is what made the ListBox go
/// deaf to a reversed drag (operator, 2026-08-17).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ComboGesture {
    /// No button is down on this control.
    #[default]
    None,
    /// The press landed on the header and has not left it. This is still the
    /// click that opens the dropdown, not a selection — releasing here leaves
    /// the list open rather than picking whatever is under the pointer.
    Header,
    /// The pointer has left the header downward. The highlight follows it from
    /// here on, clamped to the first and last item, and the release commits.
    List,
}

/// Everything an open dropdown needs to draw and answer with.
///
/// It is a struct rather than an argument list because **the popup is drawn in
/// a second pass**, when the `Control` it belongs to is out of reach: every
/// colour, font and measurement has to be resolved while the control is still
/// in hand and travel here.
pub struct ComboPopup<'a> {
    /// The control's id, for the popup's own widget ids.
    pub ctrl_id: &'a str,
    /// The header bar the panel hangs below.
    pub header: egui::Rect,
    pub items: &'a [String],
    /// The committed value — the item `Value` / `SelectedIndex` reports.
    pub selected: &'a str,
    /// The item the operator is on: the pointer's, the drag's, or the arrow
    /// keys'. Carried between frames by the caller.
    pub highlight: usize,
    /// The gesture in progress, carried from the header press.
    pub gesture: ComboGesture,
    /// `(selected fill, hovered fill)`, from [`combo_popup_fills`].
    pub fills: (Color32, Color32),
    /// The panel's own face, from [`combo_popup_face`].
    pub face: ComboFace,
    /// One item's height, and the font and colour its text is drawn in — the
    /// control's own, not the hardcoded 22 px and 12 pt this used to letter
    /// every dropdown in whatever `FontSize` said.
    pub item_h: f32,
    pub font: egui::FontId,
    pub text: Color32,
    /// The tallest the panel may be: the control's `DropDownHeight`. Items past
    /// it are reached by scrolling — they used to be dropped outright.
    pub max_h: f32,
    pub enabled: bool,
    /// Scroll this item into view this frame — set by the caller on the frame
    /// the dropdown opens, so it opens showing the current value.
    pub reveal: Option<usize>,
}

/// What an open dropdown did this frame, and the state to hand back to it next.
#[derive(Debug, Clone)]
pub struct GlassComboOutcome {
    pub action: Option<GlassComboAction>,
    /// The item the popup is highlighting now.
    pub highlight: usize,
    /// The gesture still in progress.
    pub gesture: ComboGesture,
    /// A press this frame landed **inside the panel**.
    ///
    /// The header pass cannot know this — the panel's rect is worked out here —
    /// and it matters: the header drops the keyboard on any press outside
    /// itself, so without this, clicking an item would leave the combo unable
    /// to answer the arrow keys until its header was clicked again.
    pub pressed_in_list: bool,
}

/// Draw the ComboBox header bar's **contents** — the value and the open/close
/// arrow. Returns `true` if it was clicked.
///
/// The **face is not drawn here**. It is the developer's, so the caller paints
/// it with [`draw_control_face`] first, exactly as a ListBox does: the header
/// used to lay a hardcoded navy surface and a blue rim over whatever the RAD
/// had designed, so a combo given a colour or a gradient came out blue the
/// moment the form ran (operator, 2026-08-18).
pub fn glass_combo_header(
    painter: &egui::Painter,
    ui: &mut egui::Ui,
    rect: egui::Rect,
    control_id: egui::Id,
    selected: &str,
    is_open: bool,
    enabled: bool,
    // The control's own typography and colour. `None` keeps the header's
    // built-in look, for callers that have no Control to hand.
    text: Option<(egui::FontId, Color32)>,
) -> bool {
    use egui::{Align2, FontId, Pos2};
    // The control's own font and colour when the caller has them: a ComboBox
    // used to paint its value at a hardcoded 12 pt in a fixed near-white,
    // whatever `FontSize` and `ForegroundColor` said — the one control on the
    // form whose text ignored both.
    let (font, text_color) = text.unwrap_or_else(|| {
        (
            FontId::proportional(12.0),
            Color32::from_rgb(220, 228, 255),
        )
    });
    painter.text(
        Pos2::new(rect.min.x + 8.0, rect.center().y),
        Align2::LEFT_CENTER,
        selected,
        font,
        text_color,
    );
    // The arrow follows the value's colour, dimmed. It was a fixed pale blue,
    // which read only because the face behind it was always the hardcoded navy;
    // now that the face is the developer's, a fixed pale blue is a glyph that
    // vanishes on the first light background anyone designs.
    painter.text(
        Pos2::new(rect.max.x - 13.0, rect.center().y),
        egui::Align2::CENTER_CENTER,
        if is_open { "▲" } else { "▼" },
        FontId::proportional(9.0),
        text_color.gamma_multiply(0.75),
    );
    enabled
        && ui
            .interact(rect, control_id, egui::Sense::click())
            .clicked()
}

/// Draw the ComboBox dropdown popup (call after all controls) and answer the
/// gesture on it: the drag that started on the header, the arrow keys, Enter
/// and Escape.
///
/// The panel is as tall as its items need up to the control's `DropDownHeight`,
/// and **scrolls** past that. It used to stop at 180 px and `break` out of the
/// item loop, so anything past about the eighth item was not clipped or
/// scrollable — it was simply never drawn, and unreachable.
pub fn glass_combo_popup(ui: &mut egui::Ui, p: ComboPopup<'_>) -> GlassComboOutcome {
    use egui::{Align2, Pos2, Sense, Vec2};

    let n = p.items.len();
    let mut highlight = p.highlight.min(n.saturating_sub(1));
    let mut gesture = p.gesture;
    let mut action: Option<GlassComboAction> = None;
    let mut reveal = p.reveal;
    let mut pressed_in_list = false;

    let item_h = p.item_h.max(1.0);
    // The panel holds its items AND the margin its scrolling pane keeps off the
    // border, so a list short enough to fit does not scroll for want of the six
    // pixels the margin costs it.
    let pad = crate::model::LIST_FRAME_PAD * 2.0;
    let content_h = n as f32 * item_h + pad;
    let popup_h = content_h.min(p.max_h.max(item_h + pad));
    let popup_rect = egui::Rect::from_min_size(
        Pos2::new(p.header.min.x, p.header.max.y + 1.0),
        Vec2::new(p.header.width(), popup_h),
    );

    let (pressed, held, released, pointer) = ui.input(|i| {
        (
            i.pointer.primary_pressed(),
            i.pointer.primary_down(),
            i.pointer.primary_released(),
            i.pointer.interact_pos(),
        )
    });

    // Where the FIRST item landed on the frame that drew it. The items live in
    // a scrolling pane, so this is what lets a pointer — including one past
    // either end of the list — be mapped onto an item before they are laid out
    // again. Absent (the frame the dropdown opens) the pane is still at the top.
    let geom_id = egui::Id::new(("glass_combo_top", p.ctrl_id));
    let first_top: f32 = ui
        .data(|d| d.get_temp(geom_id))
        .unwrap_or_else(|| popup_rect.top());
    // The item under a pointer, CLAMPED to the list: above the first it holds
    // at the first and below the last at the last, so a drag that leaves the
    // control stops on an item instead of choosing nothing.
    let item_at = |pt: Pos2| -> usize {
        (((pt.y - first_top) / item_h).floor().max(0.0) as usize).min(n.saturating_sub(1))
    };

    if n > 0 && p.enabled {
        // A press outside both the header and the panel dismisses the dropdown.
        // A press on the header belongs to the header pass, which has already
        // toggled it; a press inside the panel starts a selection gesture.
        if pressed {
            match pointer {
                Some(pt) if popup_rect.contains(pt) => {
                    gesture = ComboGesture::List;
                    pressed_in_list = true;
                }
                Some(pt) if p.header.contains(pt) => {}
                _ => action = Some(GlassComboAction::Close),
            }
        }

        // Plain hover moves the highlight, the way it always has — and now the
        // arrow keys carry on from wherever the pointer left it.
        if gesture == ComboGesture::None {
            if let Some(pt) = pointer.filter(|pt| popup_rect.contains(*pt)) {
                highlight = item_at(pt);
            }
        }

        // The drag. Once the pointer has left the header downward the gesture
        // belongs to the list, and from then on the highlight follows it —
        // clamped — however far outside the control it wanders.
        if held && gesture != ComboGesture::None {
            if let Some(pt) = pointer {
                if gesture == ComboGesture::Header && pt.y > p.header.bottom() {
                    gesture = ComboGesture::List;
                }
                if gesture == ComboGesture::List {
                    let idx = item_at(pt);
                    if idx != highlight {
                        highlight = idx;
                        reveal = Some(idx);
                    }
                }
            }
        }
        if released {
            if gesture == ComboGesture::List {
                action = Some(GlassComboAction::Select(highlight, p.items[highlight].clone()));
            }
            gesture = ComboGesture::None;
        }

        // An OPEN dropdown owns the keyboard — it is the thing in front of the
        // operator — so no focus bookkeeping is needed here, unlike the closed
        // combo the header pass has to arbitrate for. The keys are CONSUMED so
        // egui does not also walk focus to whatever lies in that direction, and
        // so Enter never reaches the form's default button behind the list.
        let (up, down, enter, escape) = ui.input_mut(|i| {
            (
                i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowUp),
                i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowDown),
                i.consume_key(egui::Modifiers::NONE, egui::Key::Enter),
                i.consume_key(egui::Modifiers::NONE, egui::Key::Escape),
            )
        });
        if up || down {
            let to = if up {
                highlight.saturating_sub(1)
            } else {
                (highlight + 1).min(n - 1)
            };
            if to != highlight {
                highlight = to;
                reveal = Some(to);
            }
        }
        if enter {
            action = Some(GlassComboAction::Select(highlight, p.items[highlight].clone()));
        } else if escape {
            action = Some(GlassComboAction::Close);
        }
    }

    // ── The panel's face: the developer's, or exactly what it drew before ────
    let corner = p.face.corner;
    let pp = ui.painter_at(popup_rect);
    match (&p.face.gradient, p.face.bg) {
        (Some((start, end, dir)), _) => {
            pp.rect_filled(popup_rect, corner, COMBO_PANEL_BASE);
            pp.add(egui::Shape::mesh(background_gradient_mesh(
                popup_rect,
                *start,
                *end,
                dir,
                egui::CornerRadius::same(corner.round() as u8),
            )));
        }
        // Opaque base under the chosen colour: a dropdown lies over the form's
        // own controls, so a translucent BackgroundColor must not let them show
        // through the list.
        (None, Some(bg)) => {
            pp.rect_filled(popup_rect, corner, COMBO_PANEL_BASE);
            pp.rect_filled(popup_rect, corner, bg);
        }
        (None, None) => {
            pp.rect_filled(popup_rect, corner, COMBO_PANEL_BASE);
            draw_surface_auto(
                &pp,
                popup_rect,
                COMBO_PANEL_TINT,
                corner,
                false,
                0.35,
                SurfaceRole::Card,
            );
        }
    }

    // ── The items, in a pane that scrolls ───────────────────────────────────
    let (selected_fill, hover_fill) = p.fills;
    let border_w = p.face.border.map(|(_, w)| w).unwrap_or(1.0);
    // The band keeps off the rim by the border plus a hairline, and is cut by
    // the panel's own arc where it meets one — a list's rule exactly, through
    // the list's own helper, so the two cannot drift apart.
    let inner = popup_rect.shrink(border_w + HIGHLIGHT_INSET);
    let mut first_row_top: Option<f32> = None;
    if n > 0 {
        // The pane the items scroll in sits INSIDE the panel's border, by the
        // same margin a ListBox keeps — so the scrollbar rests against the rim
        // from within instead of riding on top of it and out past the corner.
        let content = popup_rect.shrink(crate::model::LIST_FRAME_PAD);
        ui.scope_builder(egui::UiBuilder::new().max_rect(content), |ui| {
            egui::ScrollArea::vertical()
                .id_salt(("glass_combo_scroll", p.ctrl_id))
                .max_height(content.height())
                .auto_shrink([false, false])
                // A drag through a dropdown is a SELECTION, not a swipe. Were
                // egui's drag-to-scroll on as well, the list would slide under
                // the pointer while the highlight followed it, and the item
                // under the hand would run away from it. The wheel and the
                // scrollbar still scroll.
                .scroll_source(egui::containers::scroll_area::ScrollSource {
                    drag: egui::containers::scroll_area::DragScroll::Never,
                    ..Default::default()
                })
                .show(ui, |ui| {
                    ui.spacing_mut().item_spacing.y = 0.0;
                    // Clipped to bounds computed fresh from THIS frame's
                    // numbers: a ScrollArea floors its own clip at last frame's
                    // measured content, so the first frame the list grows it
                    // can paint past the panel.
                    let ip = ui.painter().with_clip_rect(popup_rect);
                    for (i, item) in p.items.iter().enumerate() {
                        let (row, _resp) = ui.allocate_exact_size(
                            Vec2::new(ui.available_width(), item_h),
                            Sense::click(),
                        );
                        if i == 0 {
                            first_row_top = Some(row.top());
                        }
                        // WITHOUT the animation: a list being dragged has to
                        // keep up with the hand, and egui's default eased
                        // scroll is still catching up several frames later —
                        // which is exactly the "I cannot see what is selected"
                        // this answers. `None` moves by the least it can, so
                        // an item already on screen is left where it is.
                        if reveal == Some(i) {
                            ui.scroll_to_rect_animation(
                                row,
                                None,
                                egui::style::ScrollAnimation::none(),
                            );
                        }
                        let band = egui::Rect::from_x_y_ranges(inner.x_range(), row.y_range())
                            .intersect(inner);
                        let fill = if item == p.selected {
                            Some(selected_fill)
                        } else if i == highlight {
                            Some(hover_fill)
                        } else {
                            None
                        };
                        if let Some(fill) = fill.filter(|_| band.is_positive()) {
                            ip.rect_filled(band, highlight_band_rounding(band, inner, corner), fill);
                        }
                        // Centred on the ROW, not on the band: the band is
                        // clipped by the rim at the first and last item, and
                        // hanging the text off that would nudge those two lines
                        // out of step with the rest.
                        ip.text(
                            Pos2::new(inner.left() + 10.0, row.center().y),
                            Align2::LEFT_CENTER,
                            item,
                            p.font.clone(),
                            match fill {
                                Some(fill) => caret_color(fill, p.text),
                                None => p.text,
                            },
                        );
                    }
                });
        });
    }

    // The rim LAST, and from the panel's own painter rather than from inside
    // the scrolling pane: a border drawn with the items is cut open the moment
    // the list is longer than the panel and scrolls away with them.
    match p.face.border {
        Some((_, w)) if w <= 0.0 => {}
        Some((colour, w)) => {
            pp.rect_stroke(popup_rect, corner, Stroke::new(w, colour), egui::StrokeKind::Middle);
        }
        None => {
            pp.rect_stroke(
                popup_rect,
                corner,
                Stroke::new(1.0, COMBO_PANEL_BORDER),
                egui::StrokeKind::Middle,
            );
        }
    }

    if let Some(top) = first_row_top {
        ui.data_mut(|d| d.insert_temp(geom_id, top));
    }
    GlassComboOutcome {
        action,
        highlight,
        gesture,
        pressed_in_list,
    }
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

/// Points around a circle, for [`gradient_fan`]. Enough of them that the rim
/// reads as a curve rather than a polygon at the sizes a Shape is drawn at.
fn circle_perimeter(centre: Pos2, radius: f32) -> Vec<Pos2> {
    const SEGMENTS: usize = 64;
    (0..SEGMENTS)
        .map(|i| {
            let t = std::f32::consts::TAU * (i as f32 / SEGMENTS as f32);
            Pos2::new(centre.x + radius * t.cos(), centre.y + radius * t.sin())
        })
        .collect()
}

/// The outline of a polygon, subdivided along each edge.
///
/// A triangle's three corners alone would carry a *linear* gradient exactly,
/// but not a radial one — the colour inside a triangle is interpolated between
/// its vertices, and a radial gradient is not linear. Subdividing makes both
/// right.
fn polygon_perimeter(corners: &[Pos2]) -> Vec<Pos2> {
    const PER_EDGE: usize = 16;
    let mut out = Vec::with_capacity(corners.len() * PER_EDGE);
    for i in 0..corners.len() {
        let (a, b) = (corners[i], corners[(i + 1) % corners.len()]);
        for step in 0..PER_EDGE {
            let t = step as f32 / PER_EDGE as f32;
            out.push(Pos2::new(a.x + (b.x - a.x) * t, a.y + (b.y - a.y) * t));
        }
    }
    out
}

/// Fill an arbitrary silhouette with a gradient, as a fan of triangles from
/// `centre` out to `perimeter`.
///
/// Each vertex takes the gradient's colour at its own position, so the shading
/// follows the shape. `rect` is the silhouette's bounding box — what the
/// gradient's direction and extent are measured against, so a circle and the
/// square around it shade identically.
fn gradient_fan(
    rect: egui::Rect,
    centre: Pos2,
    perimeter: &[Pos2],
    start: Color32,
    end: Color32,
    dir: &str,
) -> egui::epaint::Mesh {
    let mut mesh = egui::epaint::Mesh::default();
    if perimeter.len() < 3 {
        return mesh;
    }
    let uv = egui::epaint::WHITE_UV;
    mesh.vertices.push(egui::epaint::Vertex {
        pos: centre,
        uv,
        color: gradient_color_at(rect, start, end, dir, centre),
    });
    for p in perimeter {
        mesh.vertices.push(egui::epaint::Vertex {
            pos: *p,
            uv,
            color: gradient_color_at(rect, start, end, dir, *p),
        });
    }
    let n = perimeter.len() as u32;
    for i in 0..n {
        mesh.add_triangle(0, i + 1, (i + 1) % n + 1);
    }
    mesh
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

/// Paint the developer's background GRADIENT as this control's face, for the
/// controls that draw their own artwork and return before the generic frame
/// code ever runs. Returns whether it painted, so the caller can skip the solid
/// fill it would otherwise have drawn.
///
/// `BackgroundGradientEnabled` and its three companions are seeded on EVERY
/// control, but the only code that ever read them lived in the generic frame —
/// which a custom painter never reaches. So the property sat in the inspector
/// of a ProgressBar, a Slider, a Knob, a Gauge, a Switch, a FileDropZone and a
/// Maps, and moved nothing at all. A Shape hit exactly this wall and was fixed
/// on its own on 2026-08-18; this is that fix generalised, so a custom painter
/// inherits the behaviour instead of each one rediscovering the bug.
///
/// A control with no gradient enabled is untouched: the caller's own face is
/// drawn exactly as before.
pub fn paint_background_gradient(
    painter: &egui::Painter,
    rect: egui::Rect,
    rounding: egui::CornerRadius,
    ctrl: &Control,
    alpha_mul: f32,
) -> bool {
    if !ctrl
        .get_prop("BackgroundGradientEnabled")
        .map(|v| v.as_bool())
        .unwrap_or(false)
    {
        return false;
    }
    let shade = |key: &str, fallback: Color32| -> Color32 {
        let c = ctrl
            .get_prop(key)
            .map(|v| parse_color(v.as_str()))
            .unwrap_or(fallback);
        Color32::from_rgba_premultiplied(
            c.r(),
            c.g(),
            c.b(),
            (c.a() as f32 * alpha_mul).round().clamp(0.0, 255.0) as u8,
        )
    };
    let start = shade("BackgroundGradientStartColor", Color32::WHITE);
    let end = shade("BackgroundGradientEndColor", Color32::WHITE);
    let dir = ctrl
        .get_prop("BackgroundGradientDirection")
        .map(|v| v.as_str().to_owned())
        .unwrap_or_else(|| "South".into());
    painter.add(egui::Shape::mesh(background_gradient_mesh(
        rect, start, end, &dir, rounding,
    )));
    true
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

/// A chart value as a label: whole numbers plain, fractions to one decimal.
/// Chart labels are read at a glance, so `12` beats `12.000000`.
fn format_chart_number(v: f32) -> String {
    if (v - v.round()).abs() < 0.05 {
        format!("{}", v.round() as i64)
    } else {
        format!("{v:.1}")
    }
}
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
    // 050 R4/R6 — through the gate: a self-contained theme's chart card takes
    // the theme's own card colour, never the soft-UI light face.
    let is_neumorphic =
        glass_config_applies(painter.ctx()) && active_glass_style(painter.ctx()).is_neumorphic();
    let default_face = if let Some(c) =
        theme_token(painter.ctx(), crate::surface_theme::ColorToken::Card)
    {
        c
    } else if is_neumorphic {
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
            // 047 — a theme may supply its own data-mark palette, the same way
            // an asset pack does, so a chart reads as part of the theme rather
            // than keeping the built-in accents (spec 047 R4).
            active_surface_theme(painter.ctx())
                .data_marks()
                .unwrap_or_else(|| {
                    pal_raw
                        .iter()
                        .map(|&(r, g, b)| Color32::from_rgb(r, g, b))
                        .collect()
                })
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

    // -- The chart's own visual properties --------------------------------
    //
    // Seeded on every chart, shown in the inspector and documented in the
    // knowledge base, and read by NOTHING (operator, 2026-08-18, from the
    // dead-property audit). Each is honoured here at the value it was seeded
    // with, so a chart looks like what its properties have always claimed.
    let chart_str = |key: &str| -> String {
        ctrl.get_prop(key)
            .map(|v| v.as_str().trim().to_owned())
            .unwrap_or_default()
    };
    let is_pie = matches!(ctrl.control_type, CT::PieChart | CT::DonutChart);
    let x_caption = chart_str("XAxisLabel");
    let y_caption = chart_str("YAxisLabel");
    let show_legend = ctrl
        .get_prop("ShowLegend")
        .map(|v| v.as_bool())
        .unwrap_or(false);
    let show_labels = ctrl
        .get_prop("ShowLabels")
        .map(|v| v.as_bool())
        .unwrap_or(false);
    let label_format = {
        let s = chart_str("LabelFormat").to_ascii_lowercase();
        if s.is_empty() { "percent".to_owned() } else { s }
    };
    // Marker radius, and the opacity an area fill is laid down at.
    let point_r = ctrl
        .get_prop("PointRadius")
        .map(|v| v.as_i64() as f32)
        .unwrap_or(4.0)
        .clamp(0.5, 40.0);
    let fill_alpha = (ctrl
        .get_prop("FillAlpha")
        .map(|v| v.as_i64() as f32)
        .unwrap_or(40.0)
        .clamp(0.0, 100.0)
        / 100.0
        * 255.0) as u8;
    let cap_font = egui::FontId::proportional(9.0);
    let legend_font = egui::FontId::proportional(9.0);
    // Room reserved for whatever the properties asked to be drawn. Reserved
    // rather than overlaid: a caption written across the plot is worse than no
    // caption at all.
    let cap_h = if x_caption.is_empty() { 0.0 } else { 13.0 };
    let cap_w = if y_caption.is_empty() { 0.0 } else { 13.0 };
    // A pie's legend lists its slices, so it stands beside the chart; a
    // category chart's lists its series, so it sits under it.
    let legend_w = if show_legend && is_pie {
        (rect.width() * 0.26).min(120.0)
    } else {
        0.0
    };
    let legend_h = if show_legend && !is_pie { 13.0 } else { 0.0 };

    // Inner plot area (leave margin for axes / labels)
    let margin_l = rect.width() * 0.10 + cap_w;
    let margin_b = rect.height() * 0.12 + cap_h + legend_h;
    let margin_t = rect.height() * 0.12;
    let margin_r = rect.width() * 0.04 + legend_w;
    let plot = egui::Rect::from_min_max(
        Pos2::new(rect.min.x + margin_l, rect.min.y + margin_t),
        Pos2::new(rect.max.x - margin_r, rect.max.y - margin_b),
    );
    // Captions and the legend live in the MARGINS, outside the plot's own clip,
    // so they need a painter bounded by the whole control instead.
    let chrome = painter.with_clip_rect(rect.shrink(1.0));
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
                        painter.circle_filled(p, point_r, line_c);
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
                        Color32::from_rgba_unmultiplied(t.r(), t.g(), t.b(), fill_alpha),
                        Color32::from_rgba_unmultiplied(t.r(), t.g(), t.b(), 0),
                        shade(mono_base, 0.10),
                    )
                } else {
                    let c = pal[si % pal.len()];
                    let f = Color32::from_rgba_unmultiplied(c.r(), c.g(), c.b(), fill_alpha);
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
                            point_r + 1.0,
                            shade(mono_base, 0.20),
                            shade(mono_base, -0.20),
                        )));
                    } else {
                        painter.circle_stroke(p, point_r + 0.5, Stroke::new(1.5, c));
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
                // `ShowLabels` writes on each slice what `LabelFormat` asks
                // for: its share, its value, or its name. Both were seeded and
                // neither was ever read, so a pie has never carried a label.
                if show_labels && frac > 0.0 {
                    let text = match label_format.as_str() {
                        "value" => live
                            .get(i)
                            .map(|(_, v)| format_chart_number(*v))
                            .unwrap_or_default(),
                        "label" => live.get(i).map(|(l, _)| l.clone()).unwrap_or_default(),
                        _ => format!("{:.0}%", frac * 100.0),
                    };
                    if !text.is_empty() {
                        // Mid-way along the slice, and mid-way out from the
                        // hole so a donut's label sits on the ring rather than
                        // in the empty middle.
                        let mid = start + sweep * 0.5;
                        let r = inner_r + (outer_r - inner_r) * 0.62;
                        let at = Pos2::new(center.x + mid.cos() * r, center.y + mid.sin() * r);
                        let on = pal[i % pal.len()];
                        painter.text(
                            at,
                            egui::Align2::CENTER_CENTER,
                            text,
                            egui::FontId::proportional(9.0),
                            caret_color(on, Color32::WHITE),
                        );
                    }
                }
                start = end;
            }
        }
        _ => {}
    }

    // -- Axis captions and the legend, in the margins reserved for them ------
    let chrome_c = Color32::from_rgba_premultiplied(
        (170.0 * a as f32 / 255.0) as u8,
        (190.0 * a as f32 / 255.0) as u8,
        (225.0 * a as f32 / 255.0) as u8,
        a,
    );
    if !x_caption.is_empty() {
        chrome.text(
            Pos2::new(plot.center().x, rect.max.y - cap_h * 0.5 - legend_h),
            egui::Align2::CENTER_CENTER,
            &x_caption,
            cap_font.clone(),
            chrome_c,
        );
    }
    if !y_caption.is_empty() {
        // Turned a quarter to read up the axis, the way an axis caption does.
        let galley = chrome.layout_no_wrap(y_caption.clone(), cap_font.clone(), chrome_c);
        let at = Pos2::new(
            rect.min.x + cap_w * 0.5 + galley.size().y * 0.5,
            plot.center().y + galley.size().x * 0.5,
        );
        let mut shape = egui::epaint::TextShape::new(at, galley, chrome_c);
        shape.angle = -std::f32::consts::FRAC_PI_2;
        chrome.add(egui::Shape::Text(shape));
    }
    if show_legend {
        // A pie's legend names its SLICES, a category chart's its SERIES --
        // which is what there is to tell apart in each.
        let entries: Vec<(String, Color32)> = if is_pie {
            let names: Vec<String> = if live.is_empty() {
                (1..=4).map(|i| format!("Item {i}")).collect()
            } else {
                live.iter().map(|(l, _)| l.clone()).collect()
            };
            names
                .into_iter()
                .enumerate()
                .map(|(i, l)| (l, pal[i % pal.len()]))
                .collect()
        } else {
            let count = if live.is_empty() { 2 } else { 1 };
            (0..count)
                .map(|i| (format!("Series {}", i + 1), pal[i % pal.len()]))
                .collect()
        };
        let swatch = 7.0_f32;
        if is_pie {
            // Beside the chart, one entry per line.
            let x = rect.max.x - legend_w + 4.0;
            let line_h = 12.0_f32;
            let total = entries.len() as f32 * line_h;
            let mut y = (plot.center().y - total * 0.5).max(rect.min.y + 4.0);
            for (name, colour) in &entries {
                if y + line_h > rect.max.y - 2.0 {
                    break;
                }
                chrome.rect_filled(
                    egui::Rect::from_min_size(
                        Pos2::new(x, y + (line_h - swatch) * 0.5),
                        Vec2::new(swatch, swatch),
                    ),
                    1.0,
                    *colour,
                );
                chrome.text(
                    Pos2::new(x + swatch + 4.0, y + line_h * 0.5),
                    egui::Align2::LEFT_CENTER,
                    name,
                    legend_font.clone(),
                    chrome_c,
                );
                y += line_h;
            }
        } else {
            // Under the chart, entries laid out left to right and centred.
            let gap = 10.0_f32;
            let widths: Vec<f32> = entries
                .iter()
                .map(|(n, _)| {
                    swatch
                        + 4.0
                        + chrome
                            .layout_no_wrap(n.clone(), legend_font.clone(), chrome_c)
                            .size()
                            .x
                })
                .collect();
            let total: f32 = widths.iter().sum::<f32>() + gap * (entries.len() as f32 - 1.0).max(0.0);
            let mut x = plot.center().x - total * 0.5;
            let y = rect.max.y - legend_h * 0.5;
            for ((name, colour), w) in entries.iter().zip(widths) {
                chrome.rect_filled(
                    egui::Rect::from_min_size(
                        Pos2::new(x, y - swatch * 0.5),
                        Vec2::new(swatch, swatch),
                    ),
                    1.0,
                    *colour,
                );
                chrome.text(
                    Pos2::new(x + swatch + 4.0, y),
                    egui::Align2::LEFT_CENTER,
                    name,
                    legend_font.clone(),
                    chrome_c,
                );
                x += w + gap;
            }
        }
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
/// A control's corner radius, letting the active THEME supply the default when
/// the developer set none (050 R10).
///
/// [`corner_radius`] is ctx-free — sixty-odd callers reach it without one — so
/// it cannot ask a theme anything. This is the paint-time wrapper that can.
///
/// The developer's own `CornerRadius` always wins; a theme that offers nothing
/// leaves the per-control built-ins exactly where they were.
pub fn themed_corner_radius(ctx: &egui::Context, ctrl: &Control) -> f32 {
    // "Unset" cannot mean "absent": `Control::new` SEEDS `CornerRadius` on every
    // bordered control with that control's own default (Button 3, charts 8,
    // ProgressBar 10, everything else 0). Untouched therefore means "still equal
    // to that default" — the same still-on-the-default convention the background
    // and foreground colours use. Anything else is the developer's choice and
    // wins. This list must match `Control::new`'s, or a control's own default
    // reads as a deliberate choice and the theme stops being consulted.
    let seeded_default = match ctrl.control_type {
        ControlType::Button => 3.0,
        ControlType::BarChart
        | ControlType::LineChart
        | ControlType::PieChart
        | ControlType::AreaChart
        | ControlType::ScatterChart
        | ControlType::DonutChart => 8.0,
        ControlType::ProgressBar => 10.0,
        _ => 0.0,
    };
    let raw = ctrl
        .get_prop("CornerRadius")
        .or_else(|| ctrl.get_prop("BorderRadius"))
        .map(|v| v.as_i64() as f32);
    if raw.is_some_and(|r| r != seeded_default) {
        return corner_radius(ctrl);
    }
    let kind = if ctrl.is_container() {
        crate::surface_theme::RadiusKind::Card
    } else {
        crate::surface_theme::RadiusKind::Control
    };
    match active_surface_theme(ctx).radius(kind) {
        Some(r) => {
            let max_r = 0.5 * (ctrl.rect.w.min(ctrl.rect.h) as f32);
            r.clamp(0.0, max_r.max(0.0))
        }
        None => corner_radius(ctrl),
    }
}

/// Gap between two blocks of a `Blocks`-style ProgressBar, along the travel axis.
const PROGRESS_BLOCK_GAP: f32 = 2.0;

/// How long one block of a `Blocks`-style ProgressBar is, along the travel axis.
///
/// The developer's `BlockSize` when they set one. At its default of 0 the size
/// is automatic: derived from the bar's thickness, so the segmented look reads
/// as a row of near-square tiles whatever the bar's proportions.
pub(crate) fn progressbar_block_len(ctrl: &Control, rect: egui::Rect, vertical: bool) -> f32 {
    let chosen = ctrl
        .get_prop("BlockSize")
        .map(|v| v.as_i64())
        .unwrap_or(0)
        .max(0) as f32;
    if chosen > 0.0 {
        return chosen;
    }
    let thickness = if vertical {
        rect.width()
    } else {
        rect.height()
    };
    (thickness * 0.66).clamp(4.0, 40.0)
}

/// The runs of ink a ProgressBar's fill is made of.
///
/// `Continuous` is a single run — the filled part of the track. `Blocks` chops
/// that run into fixed-length segments separated by [`PROGRESS_BLOCK_GAP`],
/// growing from the origin edge: the left of a horizontal bar, the BOTTOM of a
/// vertical one. The last segment is clipped to the fill edge, so any non-zero
/// progress shows ink instead of waiting for a whole block to be earned.
pub(crate) fn progressbar_segments(
    filled: egui::Rect,
    vertical: bool,
    block_len: Option<f32>,
) -> Vec<egui::Rect> {
    let span = if vertical {
        filled.height()
    } else {
        filled.width()
    };
    if span < 0.5 {
        return Vec::new();
    }
    let Some(block) = block_len else {
        return vec![filled];
    };
    let block = block.max(1.0);
    let step = block + PROGRESS_BLOCK_GAP;
    let mut out = Vec::new();
    let mut off = 0.0;
    while off < span - 0.5 {
        let len = block.min(span - off);
        out.push(if vertical {
            egui::Rect::from_min_max(
                Pos2::new(filled.min.x, filled.max.y - off - len),
                Pos2::new(filled.max.x, filled.max.y - off),
            )
        } else {
            egui::Rect::from_min_max(
                Pos2::new(filled.min.x + off, filled.min.y),
                Pos2::new(filled.min.x + off + len, filled.max.y),
            )
        });
        off += step;
    }
    out
}

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
            ControlType::ProgressBar => 10.0,
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

/// A control's drop shadow with the geometry left out — everything the painter
/// needs except WHERE the control landed.
///
/// [`draw_control`] knows the rect and resolves both at once. A surface that
/// paints a control's whole face itself does not: the SideMenu's rail is handed
/// over to [`crate::sidebar::paint`], which runs on the shell and the preview
/// too, where `draw_control` is never called at all. Those surfaces resolve the
/// spec once with the rest of their shared state and place it when they know
/// their rect — so the rail's shadow exists on all four surfaces or on none,
/// rather than on whichever ones remembered to draw it.
#[derive(Debug, Clone, Copy)]
pub struct DropShadowSpec {
    /// Direction × distance, ready to translate the control's rect by.
    offset: Vec2,
    color: Color32,
    opacity: f32,
    blur_strength: usize,
    corner_radius: f32,
    /// A NEGATIVE `ShadowBlurStrength`: the shadow goes OVER the face instead
    /// of under it, which reads as sunken.
    overlay: bool,
}

impl DropShadowSpec {
    /// Is this the sunken variant, drawn after the face rather than before it?
    pub(crate) fn is_overlay(&self) -> bool {
        self.overlay
    }

    /// The same shadow at `k` of its strength — how a caller folds a control's
    /// own alpha in once, instead of at every paint site.
    pub(crate) fn faded(self, k: f32) -> Self {
        Self {
            opacity: self.opacity * k.clamp(0.0, 1.0),
            ..self
        }
    }

    /// Paint it for a control occupying `rect`.
    pub(crate) fn paint(&self, painter: &egui::Painter, rect: Rect, alpha_mul: f32) {
        draw_regular_drop_shadow(painter, &self.at(rect), alpha_mul);
    }

    fn at(&self, rect: Rect) -> RegularDropShadow {
        RegularDropShadow {
            rect: rect.translate(self.offset),
            color: self.color,
            opacity: self.opacity,
            blur_strength: self.blur_strength,
            corner_radius: self.corner_radius,
            overlay: self.overlay,
        }
    }
}

/// The shadow [`draw_control`]'s generic frame path draws, placed at `rect`.
///
/// 049 — a SideMenu is absent from it on purpose. The rail owns its whole face:
/// `draw_control` hands it to [`crate::sidebar::paint`], which draws the shadow
/// itself so the shell and the preview (which never call `draw_control`) get one
/// too. Resolving it here as well would simply paint it twice on the canvas.
fn regular_drop_shadow(
    ctrl: &Control,
    rect: Rect,
    is_neumorphic: bool,
) -> Option<RegularDropShadow> {
    if matches!(ctrl.control_type, crate::ControlType::SideMenu) {
        return None;
    }
    drop_shadow_spec(ctrl, is_neumorphic).map(|spec| spec.at(rect))
}

pub(crate) fn drop_shadow_spec(ctrl: &Control, is_neumorphic: bool) -> Option<DropShadowSpec> {
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

    Some(DropShadowSpec {
        offset: Vec2::new(ux * distance, uy * distance),
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

/// A drop shadow for something that is NOT a [`Control`] — a toolbar button,
/// say, which lives inside one control rather than being one.
///
/// The properties come in as values instead of being read off a control, but the
/// shadow is drawn by [`draw_regular_drop_shadow`], so a toolbar button's shadow
/// and a Button's are the same artwork with the same falloff. `opacity` is
/// 0-100 %, `blur_strength` is the layer count, `distance`/`direction_degrees`
/// place it exactly as `ShadowDistance`/`ShadowDirection` do.
#[allow(clippy::too_many_arguments)]
pub(crate) fn draw_loose_drop_shadow(
    painter: &egui::Painter,
    rect: Rect,
    color: Color32,
    opacity: i64,
    distance: i64,
    direction_degrees: f32,
    blur_strength: i64,
    corner_radius: f32,
    alpha_mul: f32,
) {
    let opacity = (opacity.clamp(0, 100) as f32) / 100.0;
    if opacity <= 0.0 {
        return;
    }
    let rad = direction_degrees.to_radians();
    let offset = Vec2::new(rad.cos(), rad.sin()) * distance.max(0) as f32;
    let shadow = RegularDropShadow {
        rect: rect.translate(offset),
        color,
        opacity,
        blur_strength: blur_strength.clamp(0, 20) as usize,
        corner_radius,
        overlay: false,
    };
    draw_regular_drop_shadow(painter, &shadow, alpha_mul);
}

fn draw_regular_drop_shadow(painter: &egui::Painter, shadow: &RegularDropShadow, alpha_mul: f32) {
    regular_shadow_stack(shadow, alpha_mul).paint(painter);
}

/// The layers [`draw_regular_drop_shadow`] paints — same reason as
/// [`neumorphic_shadow_stack`]: the notch mask has to be able to ask what colour
/// this shadow left at a point, and one definition is what keeps the answer true.
fn regular_shadow_stack(shadow: &RegularDropShadow, alpha_mul: f32) -> ShadowStack {
    let sc = shadow.color;
    let mut stack = ShadowStack::default();
    if shadow.blur_strength == 0 {
        let alpha = (shadow.opacity * alpha_mul * 255.0) as u8;
        stack.layers.push(ShadowLayer {
            rect: shadow.rect,
            rounding: shadow.corner_radius.into(),
            color: Color32::from_rgba_premultiplied(
                (sc.r() as f32 * shadow.opacity * alpha_mul) as u8,
                (sc.g() as f32 * shadow.opacity * alpha_mul) as u8,
                (sc.b() as f32 * shadow.opacity * alpha_mul) as u8,
                alpha,
            ),
        });
        return stack;
    }

    // Outermost (faintest) first, so the painter's back-to-front order gives the
    // shadow a denser core.
    let layers = shadow.blur_strength;
    for i in 0..=layers {
        let t = 1.0 - (i as f32 / layers as f32);
        let expand = t * shadow.blur_strength as f32;
        let falloff = (-3.0 * t * t).exp();
        let alpha = (shadow.opacity * alpha_mul * falloff * 255.0) as u8;
        stack.layers.push(ShadowLayer {
            rect: shadow.rect.expand(expand),
            rounding: (shadow.corner_radius + expand).into(),
            color: Color32::from_rgba_premultiplied(
                (sc.r() as f32 * (alpha as f32 / 255.0)) as u8,
                (sc.g() as f32 * (alpha as f32 / 255.0)) as u8,
                (sc.b() as f32 * (alpha as f32 / 255.0)) as u8,
                alpha,
            ),
        });
    }
    stack
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
    // 050 R4/R6 — through the gate. Under a self-contained theme the face IS
    // the theme's card, not frost over the backdrop, so asking the glass
    // register what the face looks like would be answering for a surface that
    // is not being painted.
    if !glass_config_applies(ctx) {
        let face = theme_token(ctx, crate::surface_theme::ColorToken::Card)
            .unwrap_or(opaque_under);
        return composite_premultiplied_over(face, opaque_under);
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

/// Above this `Transparency` a control's frame is too see-through to measure a
/// caption against: what the text really sits on is the form, a container, or a
/// background image — none of which the painter can read. The developer's own
/// foreground colour stands.
const CAPTION_RESCUE_MAX_TRANSPARENCY: i64 = 70;

/// The tone a control's **caption** sits on — the background of its *frame*,
/// not of its indicator.
///
/// A CheckBox paints its BackgroundColor into the tick box; a RadioButton into
/// its circle. The caption is beside that, on the frame. Measuring the caption
/// against `control_surface_tone` therefore asked the wrong surface: giving a
/// check box a dark BackgroundColor flipped its caption to white, and on a pale
/// form the caption vanished — the colour it was rescued from was the box's,
/// three pixels away, not the one under the text (operator, 2026-08-22).
///
/// So the frame's own transparency decides:
/// * more than `CAPTION_RESCUE_MAX_TRANSPARENCY` % see-through → `None`. There
///   is no frame background to read, so nothing is rescued and the developer's
///   colour is used exactly as given. (A CheckBox defaults to 100 %, which is
///   how the wrong rescue was reaching every default check box.)
/// * otherwise → the frame's face blended toward what is behind it by the same
///   alpha the frame is painted with, so a half-transparent frame is judged on
///   the half-transparent colour the eye actually sees.
///
/// A fully opaque control (a DateTimePicker, a check box the developer made
/// solid) returns exactly `control_surface_tone` — its face IS its frame.
pub fn caption_surface_tone(
    ctx: &egui::Context,
    ctrl: &Control,
    under: Color32,
) -> Option<Color32> {
    if crate::model::transparency_of(ctrl) > CAPTION_RESCUE_MAX_TRANSPARENCY {
        return None;
    }
    let face = control_surface_tone(ctx, ctrl, under);
    let alpha = crate::model::alpha_multiplier(ctrl).clamp(0.0, 1.0);
    if alpha >= 1.0 {
        return Some(face);
    }
    // What the frame lets through: the form's own backdrop when the render walk
    // published one, and otherwise the neutral stand-in the caller passed.
    let published = form_backdrop_of(ctx);
    let behind = if published.a() > 0 {
        Color32::from_rgb(published.r(), published.g(), published.b())
    } else {
        Color32::from_rgb(under.r(), under.g(), under.b())
    };
    let mix = |f: u8, b: u8| (f as f32 * alpha + b as f32 * (1.0 - alpha)).round() as u8;
    Some(Color32::from_rgb(
        mix(face.r(), behind.r()),
        mix(face.g(), behind.g()),
        mix(face.b(), behind.b()),
    ))
}

/// The default ink for a control under the **Neumorphic** register, derived
/// from the surface its text will actually land on.
///
/// This used to be a flat `Color32::BLACK`, commented "black text on light
/// surface" — an assumption about a surface the code never looked at. It holds
/// for a control that paints the register's own light face, and fails for the
/// one that paints no face at all: a Label is frameless, so its text lands on
/// the **form's backdrop**. Selecting an asset-pack theme on a dark form
/// therefore opened the neumorphic register over a dark ground and put black
/// ink on it — every caption on the form unreadable at once (operator,
/// 2026-08-21, switching Elegance → Neumorphic Light).
///
/// Elegance hid the bug rather than lacking it: it is self-contained, so the
/// glass register never applied and the ink came from its own palette.
///
/// Same rule as the map's info window (`map_tiles::readable_ink`): **derive the
/// ink from the resolved background, never inherit it from somewhere else.** An
/// explicit `ForegroundColor` still wins — this is the DEFAULT, not a clamp.
fn neumorphic_default_ink(ctx: &egui::Context, ctrl: &Control, fill: Color32) -> Color32 {
    // A Label with no background of its own paints nothing behind its text.
    // Everything else in this register paints `fill`.
    let paints_its_own_face =
        !matches!(ctrl.control_type, crate::model::ControlType::Label)
            || user_background_color(ctrl).is_some();
    let backdrop = form_backdrop_of(ctx);
    let ground = if paints_its_own_face {
        composite_premultiplied_over(
            fill,
            Color32::from_rgb(backdrop.r(), backdrop.g(), backdrop.b()),
        )
    } else {
        backdrop
    };
    if ground.a() == 0 {
        // Nothing published a backdrop (a bare painter, a preview surface):
        // keep the historical light-surface assumption rather than reading a
        // transparent ground as black and flipping every label to white.
        return Color32::BLACK;
    }
    crate::map_tiles::readable_ink(ground)
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

/// The colour an `Accent` property names: a `#RRGGBB`/`#RRGGBBAA` colour as
/// picked in the inspector, or one of the six presets by name.
///
/// The property held only those six names before it grew a colour picker, so
/// forms saved as `Blue`/`Sky`/… keep exactly the colour they were given.
pub fn knob_accent(name: &str) -> Color32 {
    if let Some(hex) = name.strip_prefix('#') {
        if matches!(hex.len(), 6 | 8) && hex.chars().all(|c| c.is_ascii_hexdigit()) {
            return parse_color(name);
        }
    }
    match name {
        "Green" => Color32::from_rgb(46, 125, 50),
        "Red" => Color32::from_rgb(198, 40, 40),
        "Purple" => Color32::from_rgb(106, 27, 154),
        "Amber" => Color32::from_rgb(245, 124, 0),
        "Sky" => Color32::from_rgb(2, 136, 209),
        _ => Color32::from_rgb(0, 120, 215),
    }
}

/// The colour a Gauge's fill takes at `frac`, once the developer has asked for
/// zones by setting BOTH thresholds — green below the warning, amber from it,
/// red from the critical one (spec 039 R8/AC2).
///
/// Thresholds are fractions of the `Minimum..Maximum` span, `0.0..1.0`, and
/// either one left blank turns zones off — `None` here, which leaves the gauge
/// its own `Color`/`ForegroundColor`/accent.
/// One of a Gauge's drawing switches (`ShowNeedle`/`ShowScale`/`ShowThumb`),
/// on unless the developer turned it off — the same default its model carries.
fn gauge_flag(ctrl: &Control, key: &str) -> bool {
    ctrl.get_prop(key).map(|v| v.as_bool()).unwrap_or(true)
}

/// What goes between a Gauge's value and its `Unit` — a space, or nothing.
///
/// A unit that starts with a letter or a digit is a WORD, and a word wants the
/// space a reader would type: `23 Parts`, `1450 rpm`. A symbol is welded to the
/// number the way it is written everywhere else: `23%`, `19°C`, `40$`. A unit
/// the developer already began with a space keeps exactly the spacing they
/// typed — the first character is not alphanumeric, so nothing is added.
///
/// This replaces "appended exactly as typed", which made `Parts` come out as
/// `23Parts` — legible only if you knew to type the space yourself.
fn unit_gap(unit: &str) -> &'static str {
    match unit.chars().next() {
        Some(c) if c.is_alphanumeric() => " ",
        _ => "",
    }
}

/// The built-in colour of each Gauge zone — what the meter painted before the
/// three became properties, and what an empty property still means.
pub const GAUGE_NORMAL_COLOR: Color32 = Color32::from_rgb(46, 125, 50);
pub const GAUGE_WARNING_COLOR: Color32 = Color32::from_rgb(245, 124, 0);
pub const GAUGE_CRITICAL_COLOR: Color32 = Color32::from_rgb(198, 40, 40);

/// Which colour a Gauge's fill takes at `frac`, once the developer has asked
/// for zones by setting BOTH thresholds.
///
/// The three colours were literals here — a green, an amber and a red nobody
/// could change, on a control whose every other colour is a property (operator,
/// 2026-08-22). Each is `NormalColor` / `WarningColor` / `CriticalColor` now,
/// and each defaults to exactly what was painted before, so a gauge that sets
/// none of them is unchanged.
pub fn gauge_zone_color(ctrl: &Control, frac: f32) -> Option<Color32> {
    let threshold = |key: &str| -> Option<f32> {
        let raw = ctrl.get_prop(key)?.as_str().trim().to_owned();
        raw.parse::<f32>().ok()
    };
    let zone = |key: &str, built_in: Color32| -> Color32 {
        ctrl.get_prop(key)
            .map(|v| v.as_str().to_owned())
            .filter(|s| !s.trim().is_empty())
            .map(|s| parse_color(&s))
            .filter(|c| c.a() > 0)
            .unwrap_or(built_in)
    };
    let warn = threshold("WarningThreshold")?;
    let crit = threshold("CriticalThreshold")?;
    Some(if frac >= crit {
        zone("CriticalColor", GAUGE_CRITICAL_COLOR)
    } else if frac >= warn {
        zone("WarningColor", GAUGE_WARNING_COLOR)
    } else {
        zone("NormalColor", GAUGE_NORMAL_COLOR)
    })
}

/// A 270° arc from `start_deg`, filled to `frac` over a dim track.
fn stroke_arc(
    painter: &egui::Painter,
    center: Pos2,
    radius: f32,
    stroke_w: f32,
    start_deg: f32,
    sweep_deg: f32,
    frac: f32,
    fill: Color32,
    track: Color32,
) {
    let pt = |deg: f32| -> Pos2 {
        let a = deg.to_radians();
        Pos2::new(center.x + radius * a.cos(), center.y + radius * a.sin())
    };
    let segments = 48.max((sweep_deg.abs() / 4.0) as usize);
    let track_pts: Vec<Pos2> = (0..=segments)
        .map(|i| pt(start_deg + sweep_deg * (i as f32 / segments as f32)))
        .collect();
    painter.add(egui::Shape::line(track_pts, Stroke::new(stroke_w, track)));
    if frac > 0.001 {
        let fill_sweep = sweep_deg * frac.clamp(0.0, 1.0);
        let n = 48.max((fill_sweep.abs() / 4.0) as usize).max(1);
        let fill_pts: Vec<Pos2> = (0..=n)
            .map(|i| pt(start_deg + fill_sweep * (i as f32 / n as f32)))
            .collect();
        painter.add(egui::Shape::line(fill_pts, Stroke::new(stroke_w, fill)));
    }
}

/// The Knob's dial and where its value sits: `(centre, radius, value baseline)`.
///
/// The dial is as big as the control allows, minus the room the value line needs
/// underneath it — so a knob is the size it was DRAWN, on every surface. (The
/// widget this replaced picked one of three fixed pixel sizes and ignored the
/// designed rect entirely, which is why the canvas and the preview disagreed.)
pub fn knob_layout(rect: egui::Rect, show_value: bool, value_h: f32) -> (Pos2, f32, f32) {
    let reserved = if show_value { value_h + 4.0 } else { 0.0 };
    let dial_h = (rect.height() - reserved).max(8.0);
    let radius = (rect.width().min(dial_h) * 0.5 - 2.0).max(6.0);
    let center = Pos2::new(rect.center().x, rect.top() + dial_h * 0.5);
    (center, radius, center.y + radius + 4.0)
}

/// The Knob: track, active arc, rim, face, inner ring, indicator, value.
///
/// One painter for the canvas, the preview, the running form and the compiled
/// binary — the proportions of the dial the preview always drew, but scaled to
/// the control's own rect and lettered in the control's own font.
pub fn draw_knob(painter: &egui::Painter, rect: egui::Rect, ctrl: &Control, alpha_mul: f32, a: u8) {
    use crate::surface_theme::ColorToken as Tok;
    let alpha_color =
        |c: Color32| Color32::from_rgba_premultiplied(c.r(), c.g(), c.b(), (c.a() as f32 * alpha_mul) as u8);

    let min_v = ctrl.get_prop("Minimum").map(|v| v.as_i64()).unwrap_or(0) as f32;
    let max_v = ctrl
        .get_prop("Maximum")
        .map(|v| v.as_i64())
        .unwrap_or(100)
        .max(min_v as i64 + 1) as f32;
    let val = knob_value(ctrl);
    let frac = ((val - min_v) / (max_v - min_v)).clamp(0.0, 1.0);

    let accent = alpha_color(knob_accent(
        &ctrl
            .get_prop("Accent")
            .map(|v| v.as_str().to_owned())
            .unwrap_or_else(|| "Blue".into()),
    ));
    let border = theme_token(painter.ctx(), Tok::Border).unwrap_or(Color32::from_gray(140));
    let card = theme_token(painter.ctx(), Tok::Card).unwrap_or(Color32::from_gray(40));

    // The dial's own parts. Empty — the default — means the developer chose
    // nothing, so the theme keeps painting precisely what it painted before
    // these properties existed. Accent still owns the arc and the indicator.
    let chosen = |prop: &str| -> Option<Color32> {
        let raw = ctrl.get_prop(prop).map(|v| v.as_str().to_owned())?;
        if raw.trim().is_empty() {
            return None;
        }
        let c = parse_color(&raw);
        (c.a() > 0).then_some(c)
    };
    let face = chosen("FaceColor").unwrap_or(card);
    let rim = alpha_color(chosen("RimColor").unwrap_or(border));
    let track = alpha_color(chosen("TrackColor").unwrap_or(border));

    let show_value = ctrl
        .get_prop("ShowValue")
        .map(|v| v.as_bool())
        .unwrap_or(true);
    let fsize = ctrl_font_size(ctrl);
    let (center, radius, value_y) = knob_layout(rect, show_value, fsize * 1.3);

    // Proportions taken from the dial the preview draws, expressed against the
    // arc radius so every knob keeps the same look at any size.
    let arc_stroke = (radius * 0.147).max(1.5);
    let rim_r = radius * 0.794;
    let face_r = radius * 0.647;
    let inner_r = radius * 0.529;
    let ind_inner = radius * 0.353;
    let ind_outer = radius * 0.706;
    let ind_w = (radius * 0.071).max(1.2);

    // Sweep: 270°, opening at the bottom — 135° round to 405°.
    stroke_arc(painter, center, radius, arc_stroke, 135.0, 270.0, frac, accent, track);

    let rim_fill = alpha_color(lighten(face, 0.12));
    painter.circle(center, rim_r, rim_fill, Stroke::new(1.0, rim));
    painter.circle_filled(center, face_r, alpha_color(face));
    if inner_r > 2.0 {
        painter.circle_stroke(center, inner_r, Stroke::new(1.0, rim));
    }

    // The indicator points at the value: 0 is bottom-left, 1 bottom-right.
    let angle = (135.0 + 270.0 * frac).to_radians();
    let dir = Vec2::new(angle.cos(), angle.sin());
    painter.line_segment(
        [center + dir * ind_inner, center + dir * ind_outer],
        Stroke::new(ind_w, accent),
    );

    if show_value {
        // Centred on the control, clear of the dial — and in the control's own
        // font and colour, rescued to stay legible on whatever it sits on.
        let fg = ctrl
            .get_prop("ForegroundColor")
            .map(|v| parse_color(v.as_str()))
            .filter(|c| c.a() > 0)
            .unwrap_or(Color32::from_rgb(230, 230, 230));
        let tone = control_surface_tone(
            painter.ctx(),
            ctrl,
            parse_color(crate::model::DEFAULT_BACKGROUND_COLOR),
        );
        let colour = caret_color(tone, fg);
        let font = crate::fonts::font_id(
            painter.ctx(),
            &ctrl
                .get_prop("FontName")
                .map(|v| v.as_str().to_owned())
                .unwrap_or_default(),
            fsize,
        );
        painter.text(
            Pos2::new(rect.center().x, value_y),
            egui::Align2::CENTER_TOP,
            format_knob_value(ctrl, val),
            font,
            Color32::from_rgba_premultiplied(colour.r(), colour.g(), colour.b(), a),
        );
    }
}

/// A Knob's current value, accepting the integer or decimal spellings a handler
/// may have written.
pub fn knob_value(ctrl: &Control) -> f32 {
    ctrl.get_prop("Value")
        .map(|v| v.as_str().trim().parse::<f32>().unwrap_or(v.as_i64() as f32))
        .unwrap_or(0.0)
}

/// How a Knob writes its value: whole numbers unless its `Step` is fractional.
pub fn format_knob_value(ctrl: &Control, val: f32) -> String {
    let step = ctrl
        .get_prop("Step")
        .map(|v| v.as_str().trim().parse::<f32>().unwrap_or(1.0))
        .unwrap_or(1.0);
    if step.fract().abs() > f32::EPSILON {
        format!("{val:.2}")
    } else {
        format!("{val:.0}")
    }
}

/// Lift a colour towards white by `t` — the rim tint that lets the dial read as
/// raised against its own face.
fn lighten(c: Color32, t: f32) -> Color32 {
    let mix = |v: u8| -> u8 { (v as f32 + (255.0 - v as f32) * t).round().clamp(0.0, 255.0) as u8 };
    Color32::from_rgba_premultiplied(mix(c.r()), mix(c.g()), mix(c.b()), c.a())
}

/// A toggle's indicator size and the two spacings around it: `(diameter, pad,
/// gap)` — `pad` from the control's edge to the indicator, `gap` from the
/// indicator to the caption.
///
/// ONE source for the CheckBox's square box and the RadioButton's circle, so a
/// radio's caption stands exactly as far from its selection circle as a check
/// box's caption stands from its box. They were computed twice from the same
/// constants, which is a coincidence, not a rule.
fn toggle_indicator_metrics(rect: egui::Rect, ctrl: &Control) -> (f32, f32, f32) {
    let d = (ctrl_font_size(ctrl) * 1.25).clamp(12.0, (rect.height() - 4.0).max(10.0));
    let pad = 4.0_f32.min(rect.width() * 0.08);
    let gap = 6.0_f32.min(rect.width() * 0.08);
    (d, pad, gap)
}

/// The colour the developer chose for a toggle's INDICATOR — the CheckBox's
/// tick box, the RadioButton's circle — or `None` while they have not chosen
/// one and the theme's own toggle surface leads.
///
/// Empty means "not chosen" here rather than the renderer-wide "still on the
/// seeded default" convention: the property is seeded empty precisely so the
/// theme keeps the box until someone names a colour, and so that naming white
/// stays possible.
pub fn user_checkbox_color(ctrl: &Control) -> Option<Color32> {
    ctrl.get_prop("CheckBoxColor")
        .map(|v| v.as_str().trim().to_owned())
        .filter(|raw| !raw.is_empty())
        .map(|raw| parse_color(&raw))
        .filter(|c| c.a() > 0)
}

/// The colour a toggle's indicator is actually painted with, resolved the same
/// way the painter resolves it. The inspector's swatch reads this, so what it
/// shows and what the canvas draws cannot disagree.
///
/// `under` stands in for the surface behind the control, used only when nothing
/// else answers.
pub fn checkbox_box_fill(ctx: &egui::Context, ctrl: &Control, under: Color32) -> Color32 {
    if let Some(c) = user_checkbox_color(ctrl) {
        return c;
    }
    let checked = ctrl
        .get_prop("Checked")
        .map(|v| v.as_bool())
        .unwrap_or(false);
    active_surface_theme(ctx)
        .surface(SurfaceRole::Toggle, SurfaceState { selected: false, on: checked })
        .and_then(|spec| spec.fill)
        .unwrap_or(under)
}

/// The indicator's own border — `(style, width, colour)`, seeded `None` so an
/// untouched toggle keeps whatever rim its theme draws.
pub fn checkbox_box_border(ctrl: &Control) -> (String, f32, Color32) {
    let style = ctrl
        .get_prop("CheckBoxBorderStyle")
        .map(|v| v.as_str().to_owned())
        .unwrap_or_else(|| "None".into());
    let width = ctrl
        .get_prop("CheckBoxBorderWidth")
        .map(|v| v.as_i64() as f32)
        .unwrap_or(1.0);
    let colour = ctrl
        .get_prop("CheckBoxBorderColor")
        .map(|v| parse_color(v.as_str()))
        .unwrap_or(Color32::from_rgb(140, 140, 160));
    (style, width, colour)
}

/// The colour a toggle's mark is drawn in — the CheckBox's tick, the
/// RadioButton's dot.
///
/// The developer's `CheckColor` is kept while it clears WCAG AA against the box
/// the THEME filled, and otherwise flips to the pole that reads: white on a dark
/// box, black on a light one. A form cannot know what its theme paints — the
/// seeded Windows blue is fine on a pale box and a smudge on the dark toggle
/// Elegance and its family draw — so the mark is rescued the same way the
/// caption already is.
///
/// `box_fill` is the theme's toggle fill, if it supplies one; the mark is
/// measured against that composited over whatever the control sits on.
fn toggle_mark_color(painter: &egui::Painter, ctrl: &Control, box_fill: Option<Color32>) -> Color32 {
    let behind = control_surface_tone(
        painter.ctx(),
        ctrl,
        parse_color(crate::model::DEFAULT_BACKGROUND_COLOR),
    );
    // The developer's own box colour outranks the theme's fill here exactly as
    // it does when the box is painted — otherwise the tick would be rescued
    // against a colour the box no longer wears.
    let tone = user_checkbox_color(ctrl)
        .or(box_fill)
        .map(|f| composite_premultiplied_over(f, behind))
        .unwrap_or(behind);
    let chosen = ctrl
        .get_prop("CheckColor")
        .map(|v| parse_color(v.as_str()))
        .unwrap_or(Color32::from_rgb(0, 120, 215));
    caret_color(tone, chosen)
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

/// Tessellate ONE corner notch as a radial grid instead of a fan.
///
/// `push_notch_fan` has the right silhouette but every triangle shares the bbox
/// corner, so a per-vertex colour gets exactly two samples along the radius.
/// That is enough for a flat repaint and not nearly enough for a drop shadow,
/// which falls off across the notch — the whole reason the mask has to repaint
/// one at all. The grid runs from the arc (ring 0) out to the two square edges
/// (ring `rings`), so the silhouette is identical to the fan's: the outer
/// boundary is where the ray from the arc centre leaves the corner square, which
/// traces those two edges through the bbox corner exactly.
#[allow(clippy::too_many_arguments)]
fn push_notch_rings(
    m: &mut egui::epaint::Mesh,
    center: egui::Pos2,
    r: f32,
    t0: f32,
    t1: f32,
    sx: f32, // outward sign toward the bbox corner, x
    sy: f32, // …and y
    rings: usize,
    uv_fn: &dyn Fn(egui::Pos2) -> egui::Pos2,
    color_fn: &dyn Fn(egui::Pos2) -> Color32,
) {
    if r < 0.5 {
        return;
    }
    // Finer than the fan's `r/2`: the outer boundary reaches `r·√2` at the bbox
    // corner, where the fan's angular step would put samples ~4px apart —
    // coarser than the gap between shadow layers, which is what the grid exists
    // to resolve. One sample per radius-pixel keeps the tangential step near a
    // pixel all the way out to the apex.
    let segs = (r as usize).clamp(12, 120);
    let rings = rings.max(1);
    let base = m.vertices.len() as u32;
    for i in 0..=segs {
        let t = t0 + (t1 - t0) * (i as f32 / segs as f32);
        let (dx, dy) = (t.cos(), t.sin());
        // Where this ray leaves the corner square: the nearer of the two edges.
        let hit = |d: f32, s: f32| if d.abs() < 1e-4 { f32::MAX } else { s * r / d };
        let outer = hit(dx, sx).min(hit(dy, sy)).max(r);
        for k in 0..=rings {
            let rho = r + (outer - r) * (k as f32 / rings as f32);
            let p = egui::pos2(center.x + rho * dx, center.y + rho * dy);
            m.vertices.push(egui::epaint::Vertex {
                pos: p,
                uv: uv_fn(p),
                color: color_fn(p),
            });
        }
    }
    let stride = (rings + 1) as u32;
    for i in 0..segs as u32 {
        for k in 0..rings as u32 {
            let a = base + i * stride + k;
            let b = a + 1;
            let c = base + (i + 1) * stride + k;
            let d = c + 1;
            m.indices.extend([a, b, d, a, d, c]);
        }
    }
}

/// Build the four corner notches of `rect`/`rounding` into one mesh, colouring
/// each vertex via `uv_fn` (texture) + `color_fn` (tint), with `rings` steps from
/// the arc out to the bbox edges (1 = the flat fan the backdrop repaint uses).
fn notch_mesh_ringed(
    rect: egui::Rect,
    rounding: egui::CornerRadius,
    rings: usize,
    uv_fn: &dyn Fn(egui::Pos2) -> egui::Pos2,
    color_fn: &dyn Fn(egui::Pos2) -> Color32,
) -> egui::epaint::Mesh {
    use std::f32::consts::PI;
    let mut m = egui::epaint::Mesh::default();
    let cap = 0.5 * rect.width().min(rect.height());
    let cl = |v: f32| v.max(0.0).min(cap);
    let (x0, y0, x1, y1) = (rect.min.x, rect.min.y, rect.max.x, rect.max.y);
    for (radius, cx, cy, t0, t1, sx, sy) in [
        (cl(f32::from(rounding.nw)), x0, y0, PI, 1.5 * PI, -1.0, -1.0),
        (cl(f32::from(rounding.ne)), x1, y0, 1.5 * PI, 2.0 * PI, 1.0, -1.0),
        (cl(f32::from(rounding.se)), x1, y1, 0.0, 0.5 * PI, 1.0, 1.0),
        (cl(f32::from(rounding.sw)), x0, y1, 0.5 * PI, PI, -1.0, 1.0),
    ] {
        push_notch_rings(
            &mut m,
            egui::pos2(cx - sx * radius, cy - sy * radius),
            radius,
            t0,
            t1,
            sx,
            sy,
            rings,
            uv_fn,
            color_fn,
        );
    }
    m
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
///
/// `shadow` is the control's OWN soft-shadow stack ([`control_shadow_stack`]).
/// The backdrop is not the whole truth about what sits behind a rounded control:
/// its drop shadow (or Neumorphic halo) is painted there too, and shows through
/// the notch, which is what makes a rounded corner look attached to the surface.
/// Repainting the flat backdrop erased it inside the bbox while it survived just
/// outside, leaving a hard-edged wedge at every corner. Pass the stack and it is
/// re-composited on top; pass `None` for a control that paints no shadow.
#[allow(clippy::too_many_arguments)]
pub fn draw_container_notch_mask(
    painter: &egui::Painter,
    rect: egui::Rect,
    rounding: egui::CornerRadius,
    fill: Color32,
    gradient: Option<(egui::Rect, Color32, Color32, &str)>,
    image: Option<(egui::TextureId, egui::Rect)>,
    img_alpha: u8,
    shadow: Option<&ShadowStack>,
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
    // The control's own shadow, back on top of the repainted backdrop. Ringed,
    // because the falloff is the whole point and a fan would interpolate it
    // between the arc and the bbox corner in one step.
    if let Some(stack) = shadow.filter(|s| !s.is_empty()) {
        let m = notch_mesh_ringed(
            rect,
            rounding,
            NOTCH_SHADOW_RINGS,
            &|_p| egui::epaint::WHITE_UV,
            &|p| stack.sample(p),
        );
        if !m.indices.is_empty() {
            painter.add(egui::Shape::mesh(m));
        }
    }
}

/// Steps from the arc to the bbox edge when re-compositing a shadow into a corner
/// notch.
///
/// The notch is at most `r·(√2−1)` deep, so 24 rings sample it under a pixel
/// apart at any radius a form uses. That matters because a soft shadow is a
/// STACK of layers — a staircase, not a ramp — and a vertex colour interpolated
/// across a cell cuts the corner off each step. Twelve rings left ~24 levels of
/// error at the dense core of a strong halo, which is small but measurable;
/// halving the cell halves it. (The whole notch is a few thousand vertices for
/// one mesh per masked control, so the resolution is cheap.)
const NOTCH_SHADOW_RINGS: usize = 24;

/// Restore a rounded container's own outline on its four corner arcs after
/// [`draw_container_notch_mask`] repainted the backdrop over them. The notch mask
/// paints the backdrop right up to (and, with anti-aliased tessellation, over) the
/// corner edge, so a Panel/GroupBox otherwise loses its border/rim on every rounded
/// corner — the straight edges survive but the corners show the backdrop. This
/// redraws the same outline `draw_control` paints for a container — the glass rim
/// plus any explicit BorderColor/BorderWidth/BorderStyle border — clipped to each
/// corner square so the straight edges are not double-stroked.
///
/// **Restore means restore.** A control whose face draws no outline has none to
/// put back, and drawing one here does not repair anything — it invents an edge
/// that exists on the four corner arcs and nowhere else. `draw_control`'s Maps
/// branch paints its halo, its background gradient and its tiles and returns
/// before any rim or border, so a map has no edge line at all; restoring one gave
/// it a hard 1px border on each corner — a dark hair at the corners of a running
/// map (operator, 2026-08-21). It showed in the run form only because the
/// designer canvas never calls this function.
pub fn restore_container_outline(
    painter: &egui::Painter,
    ctrl: &Control,
    rect: egui::Rect,
    radius: f32,
    glass: bool,
    masked: egui::CornerRadius,
) {
    // Stated positively, so a control type that later joins the notch mask has to
    // opt in deliberately rather than inherit an outline it never draws.
    if !matches!(
        ctrl.control_type,
        crate::ControlType::Panel | crate::ControlType::GroupBox
    ) {
        return;
    }
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

// ── 050 — the active form theme ──────────────────────────────────────────────
//
// `set_active_theme` carries `Option<Arc<ThemePack>>`, where `None` already
// means "procedural Liquid Glass" — so it has no room to express a *second*
// procedural theme. The theme rides its own channel, shaped exactly like
// `set_glass_style` above: the host publishes once per frame, the painter reads
// it per control.
//
// This used to carry a two-variant `SurfaceStyle` enum, which every painter
// tested against. It now carries the theme ITSELF (spec 050 R11–R15): painters
// ask it questions instead of asking who it is, so registering a theme touches
// no painter, and a theme that owns the whole look can say so once instead of
// eleven painters guessing.
//
// A context that was never told gets Liquid Glass, so any surface that forgets
// to publish keeps today's behaviour precisely — the same code path, not merely
// an equal one (R15).

#[derive(Clone)]
struct ActiveSurfaceTheme(Arc<dyn crate::surface_theme::SurfaceTheme>);

fn surface_theme_id() -> egui::Id {
    egui::Id::new("cobolt-active-surface-theme")
}

/// Set the form theme for the current frame. Call once before the control-draw
/// loop on every rendering surface, alongside [`set_glass_style`] and
/// [`set_active_theme`].
pub fn set_surface_theme(ctx: &egui::Context, theme: Arc<dyn crate::surface_theme::SurfaceTheme>) {
    ctx.data_mut(|d| d.insert_temp(surface_theme_id(), ActiveSurfaceTheme(theme)));
}

/// The theme painting this frame (Liquid Glass when nothing was published).
pub(crate) fn active_surface_theme(
    ctx: &egui::Context,
) -> Arc<dyn crate::surface_theme::SurfaceTheme> {
    ctx.data(|d| d.get_temp::<ActiveSurfaceTheme>(surface_theme_id()))
        .map(|a| a.0)
        .unwrap_or_else(crate::surface_theme::liquid_glass)
}

/// **The single gate** (spec 050 R6): may Liquid Glass's ambient configuration
/// — the `GlassStyle` register, its frost, its neumorphic relief — be applied
/// this frame?
///
/// `false` under a self-contained theme. Every painting read of
/// [`active_glass_style`] must pass through here; `glass_style_is_read_through_one_gate`
/// asserts it, because a single ungated read is exactly how this leaked in the
/// first place — suppressing drop shadows and painting neumorphic rims on a flat
/// surface that has neither.
///
/// This gates the ambient configuration ONLY. The developer's own explicit
/// control properties are never gated; they win under every theme (R9).
pub(crate) fn glass_config_applies(ctx: &egui::Context) -> bool {
    !active_surface_theme(ctx).is_self_contained()
}

/// The theme's colour for a property the developer has NOT set, or `None` to
/// keep the caller's built-in default.
///
/// This is how a painter defaults an unset property without ever asking which
/// theme is active (R13). The caller keeps its own literal in the `unwrap_or` —
/// that literal IS the Liquid Glass default, and Liquid Glass answers `None`, so
/// the historical value is reached by the historical code.
pub(crate) fn theme_token(
    ctx: &egui::Context,
    tok: crate::surface_theme::ColorToken,
) -> Option<Color32> {
    active_surface_theme(ctx).token(tok)
}

/// Does the theme supply its own face for `role`?
///
/// A flat theme has no frost to fall back on, so a surface Liquid Glass leaves
/// bare when the developer set no colour must still be laid down. Asking what
/// the theme provides — not which theme it is — is what keeps that decision
/// free of a name (R13).
pub(crate) fn theme_has_surface(ctx: &egui::Context, role: SurfaceRole) -> bool {
    active_surface_theme(ctx)
        .surface(role, SurfaceState::default())
        .is_some()
}

/// The active theme's own colours, for the colour picker's swatch grid.
///
/// Public because the IDE's inspector draws that grid, and it must offer the
/// colours of the theme the form is actually painted in.
pub fn active_theme_swatches(ctx: &egui::Context) -> Vec<Color32> {
    active_surface_theme(ctx).swatches()
}

/// Does the theme paint toggle indicators itself?
///
/// When it does, a RadioButton gets a real drawn dot and its caption carries no
/// `(●)`/`( )` glyph. Liquid Glass does not, and keeps the glyph.
pub(crate) fn theme_paints_toggles(ctx: &egui::Context) -> bool {
    theme_has_surface(ctx, SurfaceRole::Toggle)
}

/// The colour a TreeView writes its nodes in.
///
/// The developer's `ForegroundColor` when they chose one — the run form used to
/// ignore it entirely and write every tree in the theme's own text colour, so
/// the property was in the inspector and reached nothing. Otherwise the theme's
/// text token, and failing that a light ink for the dark face a tree has by
/// default.
pub fn treeview_ink(ctx: &egui::Context, ctrl: &Control) -> Color32 {
    if let Some(c) = ctrl
        .get_prop("ForegroundColor")
        .map(|v| v.as_str().to_owned())
        .filter(|s| {
            !s.trim().is_empty()
                && !s.trim().eq_ignore_ascii_case(crate::model::DEFAULT_FOREGROUND_COLOR)
        })
        .map(|s| parse_color(&s))
        .filter(|c| c.a() > 0)
    {
        return c;
    }
    theme_token(ctx, crate::surface_theme::ColorToken::Text)
        .unwrap_or(Color32::from_rgb(220, 226, 250))
}

/// A RadioButton's circle: `(fill, rim, rim width)` — **filled when on, an empty
/// rim when off**, on every theme.
///
/// The shape is the platform's and the colours are the theme's. A theme that
/// describes a `Toggle` surface answers with it, so Elegance paints exactly what
/// it always painted. A theme that describes none — Liquid Glass, and therefore
/// Classic, Enhanced and both Neumorphic styles — used to get no indicator at
/// all, only `(●)`/`( )` typed into the caption; it now gets the same circle,
/// coloured by the control's own `CheckColor`.
///
/// The OFF rim is picked by CONTRAST rather than fixed. An empty circle has to
/// be visible on whatever the control was dropped on — a dark form, a pale card,
/// a Neumorphic surface — and a fixed grey is exactly the thing that disappears
/// on half of them. This is the rule the caption beside it already follows.
pub(crate) fn radio_indicator_colors(
    ctx: &egui::Context,
    ctrl: &Control,
    checked: bool,
) -> (Color32, Color32, f32) {
    if let Some(spec) = active_surface_theme(ctx).surface(
        SurfaceRole::Toggle,
        SurfaceState {
            selected: false,
            on: checked,
        },
    ) {
        return (
            spec.fill.unwrap_or(Color32::TRANSPARENT),
            spec.border,
            spec.border_width,
        );
    }
    // `CheckColor` is seeded `#0078D7` on every CheckBox and RadioButton, so
    // there is always an answer here and a developer who wants another one
    // already has the property to say so.
    let on = ctrl
        .get_prop("CheckColor")
        .map(|v| parse_color(v.as_str()))
        .filter(|c| c.a() > 0)
        .unwrap_or(Color32::from_rgb(0, 120, 215));
    if checked {
        return (on, on, 1.0);
    }
    let behind = caption_surface_tone(ctx, ctrl, parse_color(crate::model::DEFAULT_BACKGROUND_COLOR))
        .unwrap_or_else(|| parse_color(crate::model::DEFAULT_BACKGROUND_COLOR));
    (Color32::TRANSPARENT, caret_color(behind, on), 1.0)
}

/// Set the glass style for the current frame. Call once before the control-draw
/// loop on every rendering surface.
pub fn set_glass_style(ctx: &egui::Context, style: crate::model::GlassStyle) {
    ctx.data_mut(|d| d.insert_temp(glass_style_id(), style as u8));
}

/// Read the active glass style (defaults to Classic).
pub(crate) fn active_glass_style(ctx: &egui::Context) -> crate::model::GlassStyle {
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

// ── 047 R13 — the shared sub-element seam ────────────────────────────────────
//
// `draw_control`'s FRAME choice is an if/else-if chain (frameless → asset-pack
// skin → glass), so a pack theme's frame correctly bypasses the glass style.
// Sub-elements never got that: the checkbox tick box, the progress fill, the
// combo header and friends called `draw_glass_auto` unconditionally, *after*
// the frame dispatch — which is why a steel-skinned checkbox still drew a glass
// tick box that changed with the glass style.
//
// All seven of those sites now route through `draw_surface_auto`, so the choice
// is made in ONE place. Spec 047 implements only the Elegance arm; Liquid Glass
// and asset packs pass straight through, byte-for-byte, so this refactor is
// invisible to them (AC8/AC10, guarded by `elegance_baseline_*`). Filling in the
// asset-pack arm is spec 007's Phase 6 (T15–T17) and deliberately not done here
// — the seam only gives that work a defined home.

// 050 — the role vocabulary moved to `crate::surface_theme`, where the trait
// that speaks it lives. Re-exported under the old names so the painters read
// unchanged.
pub(crate) use crate::surface_theme::role_for as elegance_role_for;
pub(crate) use crate::surface_theme::{SurfaceRole, SurfaceSpec, SurfaceState};

/// Like [`draw_surface_auto`] but carrying an explicit user-chosen background,
/// mirroring [`draw_glass_auto_bg`]. When the developer set a colour it leads
/// (R8) — under Elegance that means the caller-led `Shape` register, whatever
/// structural role the control would otherwise have taken.
pub(crate) fn draw_surface_auto_bg(
    painter: &egui::Painter,
    rect: egui::Rect,
    base: Color32,
    bg_underlay: Option<Color32>,
    rounding: impl Into<egui::CornerRadius>,
    selected: bool,
    alpha_mul: f32,
    role: SurfaceRole,
) {
    let (role, base) = match bg_underlay {
        Some(bg) => (SurfaceRole::Shape, bg),
        None => (role, base),
    };
    let theme = active_surface_theme(painter.ctx());
    match theme.surface(role, SurfaceState { selected, on: false }) {
        Some(spec) => draw_theme_surface(painter, rect, base, rounding, alpha_mul, &spec),
        // The theme has nothing to say — Liquid Glass, byte for byte.
        None => draw_glass_auto_bg(
            painter,
            rect,
            base,
            bg_underlay,
            rounding,
            selected,
            alpha_mul,
        ),
    }
}

/// Paint one themed sub-element surface from the theme's own description.
///
/// Flat fill + a one-pixel border, in the register the role calls for — no
/// frost, no relief, and **no dependence on `GlassStyle`** (spec 047 R12, and
/// now structurally true: nothing here can reach it).
///
/// `base` is the caller's colour, used when the theme leaves
/// [`SurfaceSpec::fill`] as `None` — a Shape whose colour the developer chose,
/// or an accent indicator carrying its own meaning.
fn draw_theme_surface(
    painter: &egui::Painter,
    rect: egui::Rect,
    base: Color32,
    rounding: impl Into<egui::CornerRadius>,
    alpha_mul: f32,
    spec: &SurfaceSpec,
) {
    let rounding = rounding.into();
    let fill = spec.fill.unwrap_or(base);
    painter.rect_filled(rect, rounding, theme_alpha(fill, alpha_mul));
    painter.rect_stroke(
        rect,
        rounding,
        Stroke::new(spec.border_width, theme_alpha(spec.border, alpha_mul)),
        egui::StrokeKind::Inside,
    );
}

/// A flat pill — solid face, one-pixel rim, fully rounded ends.
///
/// The themed counterpart of `draw_glass_pill`: a theme that paints flat
/// surfaces has no frost and no lens, so a knob drawn through the glass
/// primitive would never actually show the colour the theme chose.
fn draw_flat_pill(painter: &egui::Painter, rect: egui::Rect, face: Color32, rim: Color32) {
    let r = rect.width().min(rect.height()) * 0.5;
    painter.rect_filled(rect, r, face);
    painter.rect_stroke(rect, r, Stroke::new(1.0, rim), egui::StrokeKind::Inside);
}

/// Scale a theme colour by the caller's overall alpha, preserving its own.
pub(crate) fn theme_alpha(c: Color32, alpha_mul: f32) -> Color32 {
    let m = alpha_mul.clamp(0.0, 1.0);
    Color32::from_rgba_unmultiplied(c.r(), c.g(), c.b(), (c.a() as f32 * m).round() as u8)
}

/// Paint a control **sub-element** in the active form style.
///
/// The sub-element counterpart of [`draw_glass_auto`] — see the module note
/// above for why it exists.
pub(crate) fn draw_surface_auto(
    painter: &egui::Painter,
    rect: egui::Rect,
    base: Color32,
    rounding: impl Into<egui::CornerRadius>,
    selected: bool,
    alpha_mul: f32,
    role: SurfaceRole,
) {
    let theme = active_surface_theme(painter.ctx());
    match theme.surface(role, SurfaceState { selected, on: false }) {
        Some(spec) => draw_theme_surface(painter, rect, base, rounding, alpha_mul, &spec),
        // Liquid Glass (and every asset-pack form, which uses glass for the
        // sub-elements a pack does not cover) — unchanged.
        None => draw_glass_auto(painter, rect, base, rounding, selected, alpha_mul),
    }
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

#[derive(Clone, Copy)]
struct FormBackdrop(Color32);

fn form_backdrop_id() -> egui::Id {
    egui::Id::new("cobolt_form_backdrop")
}

/// Publish the form's resolved backdrop colour for this surface's frame, the
/// same way the active theme and glass style are published.
///
/// A control whose own background is TRANSLUCENT has to know what it is
/// translucent against, and `draw_control` is handed a control, not a form.
/// The SideMenu is the first control to need it: its rail colour is routinely
/// 20 %-opaque white, which reads as navy over a navy form and as near-white
/// over nothing.
pub fn set_form_backdrop(ctx: &egui::Context, color: Color32) {
    ctx.data_mut(|d| d.insert_temp(form_backdrop_id(), FormBackdrop(color)));
}

/// The backdrop [`set_form_backdrop`] published, or transparent if a surface
/// has not published one.
pub fn form_backdrop_of(ctx: &egui::Context) -> Color32 {
    ctx.data(|d| d.get_temp::<FormBackdrop>(form_backdrop_id()))
        .map(|b| b.0)
        .unwrap_or(Color32::TRANSPARENT)
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
        // A SideMenu reuses the MenuBar theme family — a pack that styles menus
        // styles both, and no pack needs a new key to support the shell.
        CT::MenuBar | CT::SideMenu => "menubar",
        CT::ToolBar => "toolbar",
        CT::StatusBar => "statusbar",
        CT::PictureBox => "picturebox",
        _ => "",
    }
}

#[cfg(test)]
mod checkbox_face_tests {
    use super::*;
    use crate::model::{transparency_of, Control, ControlType};

    /// **A radio is a circle — empty or filled — on every theme.**
    ///
    /// Operator, 2026-08-22: "use Elegance's radio button on all themes (circle
    /// empty/filled)". Elegance was the only theme describing a Toggle surface,
    /// and the indicator was drawn only where one existed, so everywhere else a
    /// radio was `(●)`/`( )` typed into its caption — nothing to colour and
    /// nothing that looked like a radio button.
    ///
    /// The two states must be told apart by the FILL, since that is the whole
    /// of what "empty or filled" means: a rim on both, a face on the chosen one
    /// only.
    #[test]
    fn a_radio_is_an_empty_or_filled_circle_on_every_theme() {
        let ctx = egui::Context::default();
        for (name, theme) in [
            ("liquid-glass", crate::surface_theme::liquid_glass()),
            ("elegance", crate::surface_theme::elegance()),
        ] {
            set_surface_theme(&ctx, theme);
            let radio = Control::new("RB-1", ControlType::RadioButton, 0, 0);
            let mut on = radio.clone();
            on.set_prop("Checked", crate::PropValue::Bool(true));

            let (off_fill, off_rim, off_w) = radio_indicator_colors(&ctx, &radio, false);
            let (on_fill, on_rim, on_w) = radio_indicator_colors(&ctx, &on, true);

            assert_eq!(
                off_fill,
                Color32::TRANSPARENT,
                "{name}: an unchosen radio must be an EMPTY circle"
            );
            assert!(
                on_fill.a() > 0,
                "{name}: the chosen radio must be FILLED, got {on_fill:?}"
            );
            assert!(
                off_rim.a() > 0 && on_rim.a() > 0,
                "{name}: both states need a visible rim ({off_rim:?} / {on_rim:?})"
            );
            assert!(
                off_w > 0.0 && on_w > 0.0,
                "{name}: a rim of zero width is no rim"
            );
        }
    }

    /// The theme LEADS where it speaks, and the control's own `CheckColor`
    /// answers where it does not — so Elegance keeps its green to the pixel
    /// while a glass form gets a radio it can colour.
    #[test]
    fn a_radios_fill_is_the_themes_where_it_has_one_and_check_colour_otherwise() {
        let ctx = egui::Context::default();
        let mut on = Control::new("RB-1", ControlType::RadioButton, 0, 0);
        on.set_prop("Checked", crate::PropValue::Bool(true));
        on.set_prop("CheckColor", crate::PropValue::String("#FF00FF".into()));

        set_surface_theme(&ctx, crate::surface_theme::liquid_glass());
        assert_eq!(
            radio_indicator_colors(&ctx, &on, true).0,
            Color32::from_rgb(255, 0, 255),
            "a theme with no Toggle surface must let CheckColor fill the circle"
        );

        set_surface_theme(&ctx, crate::surface_theme::elegance());
        let themed = radio_indicator_colors(&ctx, &on, true).0;
        assert_ne!(
            themed,
            Color32::from_rgb(255, 0, 255),
            "a theme that describes a Toggle still leads — Elegance is unchanged"
        );
        assert!(themed.a() > 0, "and it is a real colour: {themed:?}");
    }

    /// Where a CheckBox's drop shadow belongs follows its transparency. The
    /// threshold is the rule as specified: under 30 % there is a face solid
    /// enough to lift off the form, so the whole frame casts; at 30 % or above
    /// there is no card to raise and only the tick box does.
    fn casts_from_whole_frame(cb: &Control) -> bool {
        transparency_of(cb) < 30
    }

    #[test]
    fn a_default_checkbox_casts_from_the_tick_box_only() {
        let cb = Control::new("chk", ControlType::CheckBox, 0, 0);
        assert_eq!(transparency_of(&cb), 100);
        assert!(
            !casts_from_whole_frame(&cb),
            "with no background there is no card to lift"
        );
    }

    #[test]
    fn a_mostly_opaque_checkbox_casts_from_the_whole_frame() {
        let mut cb = Control::new("chk", ControlType::CheckBox, 0, 0);
        for t in [0, 10, 29] {
            cb.set_prop("Transparency", crate::model::PropValue::Int(t));
            assert!(casts_from_whole_frame(&cb), "{t}% must cast from the frame");
        }
        for t in [30, 60, 100] {
            cb.set_prop("Transparency", crate::model::PropValue::Int(t));
            assert!(
                !casts_from_whole_frame(&cb),
                "{t}% must cast from the tick box only"
            );
        }
    }

    /// A RadioButton's caption starts to the RIGHT of its selection circle, and
    /// exactly as far from it as a CheckBox's caption stands from its box. It
    /// used to be laid out in the whole control rect and centred there, so the
    /// circle was painted on top of the caption's first characters.
    #[test]
    fn a_radio_caption_clears_its_circle_by_the_checkboxs_own_gap() {
        let rect = egui::Rect::from_min_size(Pos2::new(0.0, 0.0), Vec2::new(240.0, 34.0));
        let check = Control::new("CheckBox-1", ControlType::CheckBox, 0, 0);
        let radio = Control::new("RadioButton-1", ControlType::RadioButton, 0, 0);

        // The box: caption starts one `gap` past the box's right edge.
        let (box_d, pad, gap) = toggle_indicator_metrics(rect, &check);
        let after_box = rect.left() + pad + box_d + gap;

        // The circle: same metrics, centred `pad + d/2` in, so its right edge is
        // at the same place — and the caption after it likewise.
        let (dot_d, dot_pad, dot_gap) = toggle_indicator_metrics(rect, &radio);
        let centre = rect.left() + dot_pad + dot_d * 0.5;
        let after_circle = centre + dot_d * 0.5 + dot_gap;

        assert_eq!(
            after_box, after_circle,
            "a radio's caption must start where a check box's does"
        );
        assert!(
            after_circle > centre + dot_d * 0.5,
            "and never on top of the circle"
        );

        println!(
            "\n  toggle captions — control {}x{}: indicator {}px at {}px in, \
             caption starts at {}px for BOTH the box and the circle\n",
            rect.width(),
            rect.height(),
            box_d,
            pad,
            after_box
        );
    }

    /// The check mark has to read on the box the THEME fills, which the form
    /// knows nothing about. The seeded `CheckColor` is a Windows blue: fine on a
    /// pale box, a smudge on the dark toggle Elegance and its family paint — so
    /// there it flips to white, and on a light box to black.
    #[test]
    fn the_check_mark_stays_legible_on_the_themes_toggle_box() {
        let seeded = parse_color("#0078D7");

        let dark = Color32::from_rgb(30, 41, 59); // an Elegance-family toggle
        let on_dark = caret_color(dark, seeded);
        assert_eq!(on_dark, Color32::WHITE, "white on a dark box");
        assert!(
            contrast_ratio(on_dark, dark) >= 4.5,
            "and it clears AA: {:.1}:1",
            contrast_ratio(on_dark, dark)
        );

        let light = Color32::from_rgb(245, 245, 248);
        let on_light = caret_color(light, seeded);
        assert_eq!(on_light, Color32::BLACK, "black on a light box");
        assert!(contrast_ratio(on_light, light) >= 4.5);

        println!(
            "\n  check mark — seeded #0078D7 reads {:.1}:1 on a dark toggle box (fails AA) \
             and is rescued to white at {:.1}:1; on a light box it becomes black at {:.1}:1\n",
            contrast_ratio(seeded, dark),
            contrast_ratio(on_dark, dark),
            contrast_ratio(on_light, light),
        );
    }

    /// The caption has to stay readable on whatever the CheckBox was dropped
    /// onto. The seeded default is plain black, which is unreadable on a dark
    /// surface — the case that sent developers hunting for a colour picker.
    #[test]
    fn the_caption_flips_to_stay_legible_on_a_dark_surface() {
        let dark = Color32::from_rgb(24, 26, 32);
        let chosen = caret_color(dark, Color32::BLACK);
        assert!(
            contrast_ratio(chosen, dark) >= 4.5,
            "black on a dark surface must be rescued, got {chosen:?}"
        );
    }

    /// A colour the developer picked that already reads is left alone — the
    /// rescue must not overrule a deliberate, legible choice.
    #[test]
    fn a_legible_chosen_colour_is_kept() {
        let light = Color32::from_rgb(240, 240, 240);
        assert_eq!(caret_color(light, Color32::BLACK), Color32::BLACK);

        let brand = Color32::from_rgb(10, 60, 140);
        assert_eq!(caret_color(light, brand), brand, "AA-clear brand colour");
    }

    /// Chosen by ratio rather than by a luminance threshold, so it clears AA on
    /// ANY surface — including the mid greys where a threshold leaves ~3.5:1.
    #[test]
    fn the_rescue_clears_aa_on_every_surface() {
        for v in (0..=255).step_by(15) {
            let surface = Color32::from_rgb(v, v, v);
            let chosen = caret_color(surface, Color32::from_rgb(128, 128, 128));
            assert!(
                contrast_ratio(chosen, surface) >= 4.5,
                "grey {v} only reached {:.2}",
                contrast_ratio(chosen, surface)
            );
        }
    }

    /// A check box's BackgroundColor paints its TICK BOX. The caption is beside
    /// the box, on the frame — and at the 100 % default the frame paints
    /// nothing at all. Rescuing the caption against the box therefore read the
    /// wrong surface: a dark BackgroundColor turned the caption white, and on a
    /// pale form it disappeared (operator, 2026-08-22).
    ///
    /// Over 70 % see-through there is no frame background to measure, so
    /// nothing is rescued and the developer's colour stands exactly as given.
    #[test]
    fn a_see_through_frame_leaves_the_developers_caption_colour_alone() {
        let ctx = egui::Context::default();
        let under = parse_color(crate::model::DEFAULT_BACKGROUND_COLOR);
        let mut cb = Control::new("chk", ControlType::CheckBox, 0, 0);
        cb.set_prop("BackgroundColor", crate::PropValue::String("#101018".into()));
        assert_eq!(transparency_of(&cb), 100, "the default this bug rode in on");

        assert!(
            caption_surface_tone(&ctx, &cb, under).is_none(),
            "a frame painting nothing cannot answer what the caption sits on"
        );

        // 71 % is still nothing to measure; 70 % is the last that answers.
        for t in [71_i64, 80, 99, 100] {
            cb.set_prop("Transparency", crate::PropValue::Int(t));
            assert!(
                caption_surface_tone(&ctx, &cb, under).is_none(),
                "{t} % transparent must leave the caption colour alone"
            );
        }
        cb.set_prop("Transparency", crate::PropValue::Int(70));
        assert!(
            caption_surface_tone(&ctx, &cb, under).is_some(),
            "70 % is the boundary and still has a frame to read"
        );
    }

    /// A frame solid enough to read IS what the caption sits on, so the rescue
    /// keeps working there: an opaque dark background still flips the seeded
    /// black caption to white.
    #[test]
    fn an_opaque_frame_still_rescues_the_caption() {
        let ctx = egui::Context::default();
        let under = parse_color(crate::model::DEFAULT_BACKGROUND_COLOR);
        let dark = Color32::from_rgb(16, 16, 24);
        let mut cb = Control::new("chk", ControlType::CheckBox, 0, 0);
        cb.set_prop("BackgroundColor", crate::PropValue::String("#101018".into()));
        cb.set_prop("Transparency", crate::PropValue::Int(0));

        let tone = caption_surface_tone(&ctx, &cb, under).expect("an opaque frame answers");
        assert_eq!(tone, dark, "an opaque frame IS its own background");
        let ink = caret_color(tone, Color32::BLACK);
        assert!(
            contrast_ratio(ink, dark) >= 4.5,
            "black on a dark frame must still be rescued, got {ink:?}"
        );
    }

    /// Between the two, the frame lets part of the form through — so that is
    /// what the caption is judged on, blended by the frame's own alpha. A dark
    /// background at 50 % over a pale form is a mid tone, not the dark colour
    /// three pixels away in the tick box.
    #[test]
    fn a_half_transparent_frame_is_judged_on_what_shows_through() {
        let ctx = egui::Context::default();
        let under = parse_color(crate::model::DEFAULT_BACKGROUND_COLOR);
        let pale = Color32::from_rgb(240, 240, 240);
        set_form_backdrop(&ctx, pale);

        let mut cb = Control::new("chk", ControlType::CheckBox, 0, 0);
        cb.set_prop("BackgroundColor", crate::PropValue::String("#000000".into()));
        cb.set_prop("Transparency", crate::PropValue::Int(50));

        let tone = caption_surface_tone(&ctx, &cb, under).expect("50 % still answers");
        let opaque = control_surface_tone(&ctx, &cb, under);
        assert_eq!(opaque, Color32::BLACK, "the box itself is the chosen black");
        assert!(
            relative_luminance(tone) > relative_luminance(opaque),
            "half a pale form showing through must lighten the frame: \
             tone {tone:?} vs box {opaque:?}"
        );
        assert!(
            relative_luminance(tone) < relative_luminance(pale),
            "and it must not read as the bare form either"
        );
    }
}

/// The two surfaces a toggle has, and the properties that reach each of them.
///
/// A CheckBox is a FRAME (the card behind caption and box, with its own
/// background and rim) and a BOX (the tick square, with its own). One
/// `BackgroundColor` used to answer for both and was visible on neither, and
/// `BorderStyle`/`BorderColor`/`BorderWidth` reached nothing at all, because a
/// see-through CheckBox took the frameless branch and returned before any
/// border was drawn (operator, 2026-08-22 — seven reports, of which these are
/// the paintable ones).
#[cfg(test)]
mod toggle_surface_tests {
    use super::*;
    use crate::model::{Control, ControlType};

    /// Everything one `draw_control` pass put on the screen, flattened.
    #[derive(Default)]
    struct Painted {
        fills: Vec<(Color32, egui::Rect)>,
        rect_strokes: Vec<(Color32, f32, egui::Rect)>,
        segments: Vec<(Color32, f32)>,
        circles: Vec<(Color32, f32)>,
    }

    impl Painted {
        fn has_rect_stroke(&self, rgb: (u8, u8, u8), width: f32) -> bool {
            self.rect_strokes.iter().any(|(c, w, _)| {
                (c.r(), c.g(), c.b()) == rgb && (*w - width).abs() < 0.01
            })
        }
        fn has_segment(&self, width: f32) -> bool {
            self.segments.iter().any(|(_, w)| (*w - width).abs() < 0.01)
        }
        fn fill_near(&self, rgb: (u8, u8, u8), tol: i32) -> Option<egui::Rect> {
            self.fills
                .iter()
                .find(|(c, _)| {
                    (c.r() as i32 - rgb.0 as i32).abs() <= tol
                        && (c.g() as i32 - rgb.1 as i32).abs() <= tol
                        && (c.b() as i32 - rgb.2 as i32).abs() <= tol
                })
                .map(|(_, r)| *r)
        }
    }

    fn paint(ctrl: &Control) -> Painted {
        let ctx = egui::Context::default();
        let mut input = egui::RawInput::default();
        input.screen_rect = Some(egui::Rect::from_min_size(
            Pos2::ZERO,
            Vec2::new(600.0, 400.0),
        ));
        let mut full = ctx.run_ui(input, |ui| {
            draw_control(ui.painter(), Pos2::ZERO, ctrl, false, true, 1.0, 1.0, None);
        });
        full.textures_delta.clear();
        fn walk(s: &egui::Shape, out: &mut Painted) {
            match s {
                egui::Shape::Vec(v) => v.iter().for_each(|s| walk(s, out)),
                egui::Shape::Rect(r) => {
                    if r.fill.a() > 0 {
                        out.fills.push((r.fill, r.rect));
                    }
                    if r.stroke.width > 0.0 && r.stroke.color.a() > 0 {
                        out.rect_strokes
                            .push((r.stroke.color, r.stroke.width, r.rect));
                    }
                }
                egui::Shape::LineSegment { stroke, .. } if stroke.width > 0.0 => {
                    out.segments.push((stroke.color, stroke.width));
                }
                egui::Shape::Circle(c) => {
                    if c.stroke.width > 0.0 && c.stroke.color.a() > 0 {
                        out.circles.push((c.stroke.color, c.stroke.width));
                    }
                }
                _ => {}
            }
        }
        let mut seen = Painted::default();
        for cs in &full.shapes {
            walk(&cs.shape, &mut seen);
        }
        seen
    }

    fn checkbox() -> Control {
        let mut cb = Control::new("chk", ControlType::CheckBox, 0, 0);
        cb.rect = crate::model::Rect { x: 0, y: 0, w: 160, h: 24 };
        cb
    }

    /// Reports 3, 5 and 6. A CheckBox is 100 % transparent by default, which
    /// sent it down the frameless branch — and that branch drew no border, so
    /// all three border properties were inert on the one control whose frame is
    /// see-through by design.
    #[test]
    fn the_frame_border_reaches_a_see_through_checkbox() {
        let mut cb = checkbox();
        cb.set_prop("BorderStyle", crate::PropValue::String("Single".into()));
        cb.set_prop("BorderColor", crate::PropValue::String("#FF00FF".into()));
        cb.set_prop("BorderWidth", crate::PropValue::Int(3));
        assert_eq!(crate::model::transparency_of(&cb), 100, "still see-through");

        let painted = paint(&cb);
        assert!(
            painted.has_rect_stroke((255, 0, 255), 3.0),
            "BorderStyle/Color/Width must paint the frame, got {:?}",
            painted.rect_strokes
        );
    }

    /// And the border is the FRAME's, not the box's (report 6): it rims the
    /// whole 160×24 control, not the ~17 px square the tick lives in.
    #[test]
    fn that_border_rims_the_frame_and_not_the_tick_box() {
        let mut cb = checkbox();
        cb.set_prop("BorderStyle", crate::PropValue::String("Single".into()));
        cb.set_prop("BorderColor", crate::PropValue::String("#FF00FF".into()));

        let painted = paint(&cb);
        let rimmed = painted
            .rect_strokes
            .iter()
            .find(|(c, _, _)| (c.r(), c.g(), c.b()) == (255, 0, 255))
            .map(|(_, _, r)| *r)
            .expect("the frame border must be painted");
        assert!(
            rimmed.width() > 100.0,
            "the frame is the whole control, got {rimmed:?}"
        );
    }

    /// Left alone, a CheckBox still paints no frame — the fix restores the
    /// border the developer asked for, it does not box every check box.
    #[test]
    fn an_untouched_checkbox_still_paints_no_frame() {
        let painted = paint(&checkbox());
        assert!(
            !painted.has_rect_stroke((140, 140, 160), 1.0)
                && !painted.has_rect_stroke((0x8C, 0x8C, 0xA0), 1.0),
            "a default CheckBox must stay frameless, got {:?}",
            painted.rect_strokes
        );
    }

    /// Report 4. The gradient branch stroked one flat rectangle whatever the
    /// style said, so turning a background gradient on flattened Fixed3D,
    /// Raised and Sunken to Single — and turning it off brought them back,
    /// which read as the gradient resetting the border style.
    #[test]
    fn a_three_d_border_survives_a_background_gradient() {
        let mut cb = checkbox();
        cb.set_prop("BorderStyle", crate::PropValue::String("Fixed3D".into()));
        cb.set_prop("BorderWidth", crate::PropValue::Int(2));
        cb.set_prop("Transparency", crate::PropValue::Int(0));

        // Fixed3D is four shaded edges, never a single rect stroke.
        let flat = paint(&cb);
        assert!(
            flat.has_segment(2.0),
            "Fixed3D must draw shaded edges, got {:?}",
            flat.segments
        );

        cb.set_prop("BackgroundGradientEnabled", crate::PropValue::Bool(true));
        cb.set_prop(
            "BackgroundGradientStartColor",
            crate::PropValue::String("#FFFFFF".into()),
        );
        cb.set_prop(
            "BackgroundGradientEndColor",
            crate::PropValue::String("#C0C0C0".into()),
        );
        let gradient = paint(&cb);
        assert!(
            gradient.has_segment(2.0),
            "the gradient must not flatten Fixed3D to one stroke, got {:?}",
            gradient.segments
        );
    }

    /// Report 2. `CheckBoxColor` paints the tick box. The box used to be drawn
    /// from `BackgroundColor`, which every painter was free to ignore — a theme
    /// answered with its own fill and Liquid Glass used it as a ~3.5 % tint, so
    /// the colour never actually landed.
    #[test]
    fn the_box_wears_the_colour_chosen_for_it() {
        let mut cb = checkbox();
        cb.set_prop("CheckBoxColor", crate::PropValue::String("#C81E1E".into()));

        let painted = paint(&cb);
        let box_rect = painted
            .fill_near((0xC8, 0x1E, 0x1E), 6)
            .expect("CheckBoxColor must paint the tick box");
        assert!(
            box_rect.width() < 40.0,
            "and it is the BOX that wears it, not the frame: {box_rect:?}"
        );
    }

    /// Report 7. The box carries its own border — style, width and colour —
    /// independent of the frame's.
    #[test]
    fn the_box_carries_its_own_border() {
        let mut cb = checkbox();
        cb.set_prop(
            "CheckBoxBorderStyle",
            crate::PropValue::String("Single".into()),
        );
        cb.set_prop(
            "CheckBoxBorderColor",
            crate::PropValue::String("#00A000".into()),
        );
        cb.set_prop("CheckBoxBorderWidth", crate::PropValue::Int(2));

        let painted = paint(&cb);
        let rimmed = painted
            .rect_strokes
            .iter()
            .find(|(c, w, _)| (c.r(), c.g(), c.b()) == (0, 0xA0, 0) && (*w - 2.0).abs() < 0.01)
            .map(|(_, _, r)| *r)
            .expect("the box border must be painted");
        assert!(
            rimmed.width() < 40.0,
            "the box border rims the BOX, got {rimmed:?}"
        );
    }

    /// The two borders are independent: setting one must not draw the other.
    #[test]
    fn the_two_borders_do_not_borrow_from_each_other() {
        let mut cb = checkbox();
        cb.set_prop(
            "CheckBoxBorderStyle",
            crate::PropValue::String("Single".into()),
        );
        cb.set_prop(
            "CheckBoxBorderColor",
            crate::PropValue::String("#00A000".into()),
        );
        let painted = paint(&cb);
        assert!(
            !painted
                .rect_strokes
                .iter()
                .any(|(_, _, r)| r.width() > 100.0),
            "a box border must not rim the frame, got {:?}",
            painted.rect_strokes
        );
    }

    /// The Label half of the operator's second list: frameless for the same
    /// reason (no background of its own), so its `BorderStyle` reached nothing
    /// either — and a Label with a border must gain a rim, not a glass card.
    #[test]
    fn a_labels_border_is_drawn_without_giving_it_a_face() {
        let mut label = Control::new("L", ControlType::Label, 0, 0);
        label.rect = crate::model::Rect { x: 0, y: 0, w: 120, h: 24 };
        label.set_prop("BorderStyle", crate::PropValue::String("Single".into()));
        label.set_prop("BorderColor", crate::PropValue::String("#FF00FF".into()));
        label.set_prop("BorderWidth", crate::PropValue::Int(2));

        let painted = paint(&label);
        assert!(
            painted.has_rect_stroke((255, 0, 255), 2.0),
            "a Label's BorderStyle must paint its frame, got {:?}",
            painted.rect_strokes
        );
        assert!(
            painted
                .fills
                .iter()
                .all(|(_, r)| r.width() < 119.0 || r.height() < 23.0),
            "and it must stay frameless — no full-size face, got {:?}",
            painted.fills
        );
    }

    /// A Label with no border keeps painting nothing, so this restores what the
    /// property promises without giving every Label a rim.
    #[test]
    fn an_untouched_label_still_paints_nothing() {
        let mut label = Control::new("L", ControlType::Label, 0, 0);
        label.rect = crate::model::Rect { x: 0, y: 0, w: 120, h: 24 };
        let painted = paint(&label);
        assert!(
            painted.rect_strokes.is_empty(),
            "a plain Label must paint no frame, got {:?}",
            painted.rect_strokes
        );
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
    /// The canvas face previews a TextBox's `HintText` **only while its `Text`
    /// is empty** — and a caller drawing the live content gets neither.
    ///
    /// The run path used to blank `Text` on a clone to silence the static
    /// caption, which switched the placeholder ON instead: it was painted under
    /// the live editor and stayed there however much was typed, crossed through
    /// the real characters (operator, 2026-08-21). `draw_control_face` is the
    /// right tool — it silences caption and hint together — and the editor
    /// supplies the placeholder itself, where egui hides it once the buffer has
    /// content.
    ///
    /// Shapes are counted rather than inspected: the face of an empty box draws
    /// strictly more than the face of a filled one (the hint galley), and a
    /// live-content face draws no text at all.
    #[test]
    fn a_textbox_face_previews_its_hint_only_while_it_is_empty() {
        let ctx = egui::Context::default();
        let rect = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(200.0, 28.0));

        // Count the TEXT shapes: the hint and the value are galleys, and every
        // other layer (face, border, shadow) is identical between the two
        // calls, so text is exactly the difference under test.
        let text_shapes = |ctrl: &Control, with_label: bool| -> usize {
            let mut out = ctx.run_ui(Default::default(), |root_ui| {
                let painter = root_ui.painter_at(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(600.0, 400.0),
                ));
                if with_label {
                    draw_control(&painter, rect.min, ctrl, false, false, 1.0, 1.0, None);
                } else {
                    draw_control_face(&painter, rect.min, ctrl, false, false, 1.0, 1.0, None);
                }
            });
            // Font atlas deltas panic on drop if nobody applies them.
            out.textures_delta.clear();
            out.shapes
                .iter()
                .filter(|cs| matches!(cs.shape, egui::Shape::Text(_)))
                .count()
        };
        let count = text_shapes;

        let mut empty = Control::new("T", crate::model::ControlType::TextBox, 0, 0);
        empty.set_prop("Text", crate::model::PropValue::String(String::new()));
        empty.set_prop(
            "HintText",
            crate::model::PropValue::String("paste it here".into()),
        );
        let mut filled = empty.clone();
        filled.set_prop(
            "Text",
            crate::model::PropValue::String("ACME Ltd".into()),
        );

        let empty_face = count(&empty, true);
        let filled_face = count(&filled, true);
        let live_face = count(&empty, false);
        assert!(
            empty_face > live_face,
            "an empty box previews its hint ({empty_face} vs {live_face} shapes)"
        );
        assert!(
            filled_face > live_face,
            "a filled box draws its text ({filled_face} vs {live_face} shapes)"
        );
        assert_eq!(
            live_face,
            count(&filled, false),
            "a face drawn for live content carries no text either way — which is \
             what stops the placeholder surviving underneath the editor"
        );
    }

    /// A Maps face carries the developer's appearance; the basemap is the
    /// canvas's stand-in for live content and is left to the caller.
    ///
    /// The run path draws the live, pannable map itself, so it could never call
    /// the face renderer — `draw_control` would have painted a second, static
    /// basemap underneath. It therefore drew no face at all, and everything
    /// set in Appearance and Drop Shadow applied on the canvas and vanished
    /// when the form ran (operator, 2026-08-21). `draw_control_face` now stops
    /// before the tiles, exactly as it stops before a caption, so both callers
    /// get the face and only the canvas gets the stand-in.
    #[test]
    fn a_maps_face_stops_before_the_basemap_so_the_run_path_can_use_it() {
        let ctx = egui::Context::default();
        let mut map = Control::new("MAP-1", crate::model::ControlType::Maps, 0, 0);
        map.rect.w = 300;
        map.rect.h = 200;
        map.set_prop("BackgroundGradientEnabled", crate::model::PropValue::Bool(true));

        let shapes = |with_label: bool| -> usize {
            let mut out = ctx.run_ui(Default::default(), |root_ui| {
                let painter = root_ui.painter_at(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(600.0, 400.0),
                ));
                if with_label {
                    draw_control(&painter, egui::Pos2::ZERO, &map, false, false, 1.0, 1.0, None);
                } else {
                    draw_control_face(&painter, egui::Pos2::ZERO, &map, false, false, 1.0, 1.0, None);
                }
            });
            out.textures_delta.clear();
            out.shapes.len()
        };

        let face_only = shapes(false);
        let with_basemap = shapes(true);
        assert!(
            face_only > 0,
            "the face must still paint the developer's appearance"
        );
        assert!(
            with_basemap > face_only,
            "the canvas draws the stand-in basemap on top of the face \
             ({with_basemap} vs {face_only} shapes)"
        );
    }

    /// A Maps control is shadow-eligible like any other visual control.
    ///
    /// Its own branch in `draw_control` returned before any shadow was drawn,
    /// so with the Neumorphic register on it got neither the relief every other
    /// control gets nor a drop shadow, and sat flat however its Drop Shadow
    /// section was filled in (operator, 2026-08-21). The branch now draws the
    /// halo; this pins the other half — that nothing excludes Maps from the
    /// shadow spec in the first place, which is what a future "non-visual
    /// controls have no shadow" list would quietly do.
    #[test]
    fn a_maps_control_is_not_excluded_from_drop_shadows() {
        let mut map = Control::new("MAP-1", crate::model::ControlType::Maps, 0, 0);
        map.set_prop("ShadowEnabled", crate::model::PropValue::Bool(true));
        map.set_prop("ShadowDistance", crate::model::PropValue::Int(7));
        assert!(
            drop_shadow_spec(&map, false).is_some(),
            "Maps must take a drop shadow like any other visual control"
        );
        // With the register on, NO control takes the rectangular shadow — the
        // relief replaces it. That is the design, and it is why the branch has
        // to draw the halo itself rather than rely on the shared path.
        assert!(
            drop_shadow_spec(&map, true).is_none(),
            "the neumorphic register replaces the drop shadow with its relief"
        );
        // Unticked stays unticked, whatever the register.
        map.set_prop("ShadowEnabled", crate::model::PropValue::Bool(false));
        assert!(drop_shadow_spec(&map, false).is_none());
    }

    /// Switching a dark form from Elegance to an asset-pack theme with the
    /// Neumorphic Light glass style turned every caption black on a dark
    /// ground — the whole panel unreadable at once (operator, 2026-08-21).
    ///
    /// The register's default ink was a flat `BLACK` justified as "on light
    /// surface", which is true of the face it paints and false of the Label
    /// that paints none: a Label's text lands on the FORM's backdrop.
    #[test]
    fn the_neumorphic_default_ink_reads_against_the_ground_it_lands_on() {
        let ctx = egui::Context::default();
        let label = Control::new("L", crate::model::ControlType::Label, 0, 0);
        let button = Control::new("B", crate::model::ControlType::Button, 0, 0);
        let light_face = Color32::from_rgb(232, 237, 254); // the register's own
        let dark_form = Color32::from_rgb(26, 31, 53); // what the operator had

        // A frameless Label over a DARK form: the case that was black-on-dark.
        set_form_backdrop(&ctx, dark_form);
        let ink = neumorphic_default_ink(&ctx, &label, light_face);
        assert!(
            contrast_ratio(ink, dark_form) >= 4.5,
            "label ink {ink:?} unreadable on {dark_form:?}"
        );
        assert!(relative_luminance(ink) > 0.5, "dark form ⇒ light ink");

        // The same Label over a LIGHT form keeps the historical dark ink, so
        // this is a repair and not a reversal.
        let light_form = Color32::from_rgb(240, 240, 240);
        set_form_backdrop(&ctx, light_form);
        let ink = neumorphic_default_ink(&ctx, &label, light_face);
        assert!(contrast_ratio(ink, light_form) >= 4.5);
        assert!(relative_luminance(ink) < 0.5, "light form ⇒ dark ink");

        // A control that DOES paint the register's light face keeps dark ink
        // even on a dark form — the face is what its text sits on, not the
        // backdrop.
        set_form_backdrop(&ctx, dark_form);
        let ink = neumorphic_default_ink(&ctx, &button, light_face);
        assert!(
            contrast_ratio(ink, light_face) >= 4.5,
            "button ink {ink:?} unreadable on its own face"
        );
        assert!(relative_luminance(ink) < 0.5, "light face ⇒ dark ink");

        // Whatever the ground, the answer clears AA — mid-tones included,
        // which is exactly where a fixed threshold picks the worse colour.
        for tone in [0u8, 40, 90, 128, 150, 200, 255] {
            let ground = Color32::from_gray(tone);
            set_form_backdrop(&ctx, ground);
            let ink = neumorphic_default_ink(&ctx, &label, light_face);
            assert!(
                contrast_ratio(ink, ground) >= 4.5,
                "ink {ink:?} fails AA on grey {tone}"
            );
        }

        // With no backdrop published at all nothing is known about the ground,
        // so the historical assumption stands rather than a guess from black.
        let bare = egui::Context::default();
        assert_eq!(
            neumorphic_default_ink(&bare, &label, light_face),
            Color32::BLACK
        );
    }

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
        ctx.run_ui(Default::default(), |root_ui| {
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
        }).textures_delta.clear();
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
        let mut full = ctx.run_ui(input, |root_ui| {
            egui::CentralPanel::default()
                .frame(egui::Frame::NONE)
                .show_inside(root_ui, |ui| {
                    draw_control(ui.painter(), Pos2::ZERO, &c, false, true, 1.0, 1.0, None);
                });
        });
        full.textures_delta.clear();
        fn leaves(s: &egui::Shape) -> usize {
            match s {
                egui::Shape::Vec(v) => v.iter().map(leaves).sum(),
                _ => 1,
            }
        }
        full.shapes.iter().map(|cs| leaves(&cs.shape)).sum()
    }

    /// Every control that paints its own artwork must still honour the
    /// UNIVERSAL background gradient.
    ///
    /// The property is seeded on all of them, but only the generic frame code
    /// ever read it — and a custom painter returns long before that. So the
    /// gradient rows sat in the inspector of a ProgressBar, a Slider, a Knob, a
    /// Gauge, a Switch, a FileDropZone and a Maps and moved nothing. This walks
    /// the painted output and demands the gradient mesh actually appear: a
    /// vertex carrying the start colour and one carrying the end colour, which
    /// a solid fill can never produce.
    #[test]
    fn a_background_gradient_reaches_every_self_painting_control() {
        use crate::model::{Control, ControlType as CT, PropValue};

        // Two colours nothing else in any style paints, so finding them proves
        // the developer's own gradient landed.
        const START: &str = "#FF00FF";
        const END: &str = "#00FF7F";

        for ct in [
            CT::ProgressBar,
            CT::Slider,
            CT::Knob,
            CT::Gauge,
            CT::Switch,
            CT::FileDropZone,
            CT::Maps,
            CT::Shape,
        ] {
            let ctx = egui::Context::default();
            let mut c = Control::new("C", ct.clone(), 0, 0);
            c.rect.w = 200;
            c.rect.h = 60;
            c.set_prop("BackgroundGradientEnabled", PropValue::Bool(true));
            c.set_prop("BackgroundGradientStartColor", PropValue::from(START));
            c.set_prop("BackgroundGradientEndColor", PropValue::from(END));
            c.set_prop("BackgroundGradientDirection", PropValue::from("South"));

            let mut input = egui::RawInput::default();
            input.screen_rect = Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(400.0, 300.0)));
            let mut full = ctx.run_ui(input, |root_ui| {
                egui::CentralPanel::default()
                    .frame(egui::Frame::NONE)
                    .show_inside(root_ui, |ui| {
                        draw_control(ui.painter(), Pos2::ZERO, &c, false, true, 1.0, 1.0, None);
                    });
            });
            full.textures_delta.clear();

            fn vertex_colors(s: &egui::Shape, out: &mut Vec<Color32>) {
                match s {
                    egui::Shape::Vec(v) => v.iter().for_each(|s| vertex_colors(s, out)),
                    egui::Shape::Mesh(m) => out.extend(m.vertices.iter().map(|v| v.color)),
                    _ => {}
                }
            }
            let mut colors = Vec::new();
            for cs in &full.shapes {
                vertex_colors(&cs.shape, &mut colors);
            }
            let want_start = parse_color(START);
            let want_end = parse_color(END);
            assert!(
                colors.contains(&want_start) && colors.contains(&want_end),
                "{ct:?}: the designed background gradient never reached the screen \
                 (looked for {want_start:?} and {want_end:?} among {} mesh vertices)",
                colors.len()
            );
        }
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
        // Flat classic fill: SurfaceStyle off keeps the face a single filled rect.
        c.set_prop("FormStyle", PropValue::Bool(false));
        c.set_prop("ShadowEnabled", PropValue::Bool(false));
        for &(key, value) in props {
            c.set_prop(key, PropValue::String(value.into()));
        }
        let mut input = egui::RawInput::default();
        input.screen_rect = Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(400.0, 300.0)));
        let mut full = ctx.run_ui(input, |root_ui| {
            egui::CentralPanel::default()
                .frame(egui::Frame::NONE)
                .show_inside(root_ui, |ui| {
                    draw_control(ui.painter(), Pos2::ZERO, &c, false, true, 1.0, 1.0, None);
                });
        });
        full.textures_delta.clear();
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
        let mut full = ctx.run_ui(input, |root_ui| {
            egui::CentralPanel::default()
                .frame(egui::Frame::NONE)
                .show_inside(root_ui, |ui| {
                    draw_control(ui.painter(), Pos2::ZERO, &c, false, true, 1.0, 1.0, None);
                });
        });
        full.textures_delta.clear();
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
        let mut full = ctx.run_ui(input, |root_ui| {
            egui::CentralPanel::default()
                .frame(egui::Frame::NONE)
                .show_inside(root_ui, |ui| {
                    draw_control(ui.painter(), Pos2::ZERO, ctrl, false, true, 1.0, 1.0, None);
                });
        });
        full.textures_delta.clear();
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
            let mut full = ctx.run_ui(input, |root_ui| {
                egui::CentralPanel::default()
                    .frame(egui::Frame::NONE)
                    .show_inside(root_ui, |ui| {
                        draw_control(ui.painter(), Pos2::ZERO, ctrl, false, true, 1.0, 1.0, None);
                    });
            });
            full.textures_delta.clear();
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
            let mut full = ctx.run_ui(input, |root_ui| {
                egui::CentralPanel::default()
                    .frame(egui::Frame::NONE)
                    .show_inside(root_ui, |ui| {
                        draw_control(ui.painter(), Pos2::ZERO, ctrl, false, true, 1.0, 1.0, None);
                    });
            });
            full.textures_delta.clear();
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

// ── Spec 047 — Elegance form theme ───────────────────────────────────────────

#[cfg(test)]
mod elegance_tests {
    use super::*;
    use crate::model::{Control, ControlType as CT, GlassStyle as GS};

    /// Every control family spec 047 R4 requires Elegance to cover, laid out on
    /// a notional form. Used by the regression baseline (T3) and the coverage
    /// gate (T13), so the two can never drift apart.
    pub(super) fn r4_families() -> Vec<(&'static str, CT)> {
        vec![
            ("PANEL", CT::Panel),
            ("GROUPBOX", CT::GroupBox),
            ("BUTTON", CT::Button),
            ("TEXTBOX", CT::TextBox),
            ("LABEL", CT::Label),
            ("CHECKBOX", CT::CheckBox),
            ("RADIOBUTTON", CT::RadioButton),
            ("LISTBOX", CT::ListBox),
            ("COMBOBOX", CT::ComboBox),
            ("SLIDER", CT::Slider),
            ("PROGRESSBAR", CT::ProgressBar),
            ("TABCONTROL", CT::TabControl),
            ("MENUBAR", CT::MenuBar),
            ("TOOLBAR", CT::ToolBar),
            ("STATUSBAR", CT::StatusBar),
            ("TREEVIEW", CT::TreeView),
            ("DATAGRID", CT::DataGrid),
            ("BARCHART", CT::BarChart),
            ("LINECHART", CT::LineChart),
            ("PIECHART", CT::PieChart),
            ("AREACHART", CT::AreaChart),
            ("SCATTERCHART", CT::ScatterChart),
            ("DONUTCHART", CT::DonutChart),
            ("KNOB", CT::Knob),
            ("GAUGE", CT::Gauge),
            ("SWITCH", CT::Switch),
            ("FILEDROPZONE", CT::FileDropZone),
        ]
    }

    /// The R4 fixture as real controls, tiled so none overlaps.
    pub(super) fn r4_fixture() -> Vec<Control> {
        r4_families()
            .into_iter()
            .enumerate()
            .map(|(i, (id, ct))| {
                let mut c = Control::new(id, ct, 0, 0);
                let col = (i % 5) as i32;
                let row = (i / 5) as i32;
                c.rect = crate::model::Rect::new(10 + col * 150, 10 + row * 90, 130, 70);
                c
            })
            .collect()
    }

    /// Count the tessellated shape leaves produced by painting `controls`
    /// under a given glass style / pack / surface style.
    ///
    /// A structural proxy for "what got drawn" — not a pixel diff. It is
    /// sensitive to geometry changes (a different number of rects, strokes or
    /// glyph runs) but blind to a pure colour swap, which is precisely the
    /// right sensitivity for proving a **refactor** moved nothing.
    pub(super) fn painted_leaf_count(
        controls: &[Control],
        glass_style: GS,
        pack: Option<std::sync::Arc<ThemePack>>,
        surface: Arc<dyn crate::surface_theme::SurfaceTheme>,
    ) -> usize {
        let ctx = egui::Context::default();
        set_glass_style(&ctx, glass_style);
        set_active_theme(&ctx, pack);
        set_surface_theme(&ctx, surface);

        let mut input = egui::RawInput::default();
        input.screen_rect = Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(900.0, 700.0)));
        let mut full = ctx.run_ui(input, |root_ui| {
            egui::CentralPanel::default()
                .frame(egui::Frame::NONE)
                .show_inside(root_ui, |ui| {
                    for c in controls {
                        draw_control(ui.painter(), Pos2::ZERO, c, false, true, 1.0, 1.0, None);
                    }
                });
        });
        full.textures_delta.clear();
        fn leaves(s: &egui::Shape) -> usize {
            match s {
                egui::Shape::Vec(v) => v.iter().map(leaves).sum(),
                _ => 1,
            }
        }
        full.shapes.iter().map(|cs| leaves(&cs.shape)).sum()
    }

    /// A minimal in-memory asset pack with real (decodable) art, so the
    /// pack-themed baseline row exercises the 9-slice path rather than silently
    /// falling back to glass on an undecodable image.
    pub(super) fn fixture_pack() -> std::sync::Arc<ThemePack> {
        const MANIFEST: &str = r#"
id = "baseline-fixture"
display_name = "Baseline Fixture"

[controls.button]
image = "art.png"
slice = [4, 4, 4, 4]

[controls.panel]
image = "art.png"
slice = [4, 4, 4, 4]
"#;
        // A real 16×16 RGBA PNG — encoded here so the texture actually loads.
        let img = image::RgbaImage::from_pixel(16, 16, image::Rgba([90, 100, 120, 255]));
        let mut png: Vec<u8> = Vec::new();
        image::DynamicImage::ImageRgba8(img)
            .write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png)
            .expect("encode fixture png");
        std::sync::Arc::new(
            ThemePack::from_embedded(MANIFEST, &[("art.png", &png)]).expect("fixture pack"),
        )
    }

    /// The four glass styles, in a stable order for reporting.
    pub(super) const ALL_GLASS_STYLES: [GS; 4] =
        [GS::Classic, GS::Enhanced, GS::Neumorphic, GS::NeumorphicDark];

    /// The Elegance theme, as the painters get it.
    pub(super) fn eleg() -> Arc<dyn crate::surface_theme::SurfaceTheme> {
        crate::surface_theme::elegance()
    }

    /// Liquid Glass, as the painters get it.
    pub(super) fn glass() -> Arc<dyn crate::surface_theme::SurfaceTheme> {
        crate::surface_theme::liquid_glass()
    }

    /// One Elegance token, unwrapped — the theme answers every one of them.
    pub(super) fn tok(t: crate::surface_theme::ColorToken) -> Color32 {
        eleg().token(t).expect("Elegance answers every token")
    }

    /// T6 — which Elegance colours are OURS and which still mirror the crate.
    ///
    /// This test used to assert that every token mirrored the palette crate, so
    /// a crate upgrade would fail loudly here rather than drifting the look
    /// silently. That guarantee now applies only to the colours we still take
    /// from it: the ones a developer actually notices — form and container
    /// backgrounds, buttons, labels, toggles, the slider — are **fixed by
    /// PowerRustCOBOL**, precisely so a crate upgrade *cannot* restyle a shipped
    /// application. Both halves are pinned below.
    ///
    /// This is the ONE test allowed to name the third-party crate — it is the
    /// mirror's other face. `no_user_facing_string_names_the_crate` guards
    /// everything the developer can actually see.
    #[test]
    fn elegance_palette_mirrors_the_crate_slate_theme() {
        use crate::surface_theme::{AccentName as A, ColorToken as Tok, RadiusKind as RK};
        let t = elegance::Theme::slate();
        let e = eleg();

        // ── Still the crate's ────────────────────────────────────────────────
        assert_eq!(tok(Tok::InputBg), t.palette.input_bg);
        assert_eq!(tok(Tok::Focus), t.palette.focus);
        assert_eq!(tok(Tok::Accent(A::Blue)), t.palette.accent_fill(elegance::Accent::Blue));
        assert_eq!(tok(Tok::Accent(A::Green)), t.palette.accent_fill(elegance::Accent::Green));
        assert_eq!(tok(Tok::Accent(A::Red)), t.palette.accent_fill(elegance::Accent::Red));
        assert_eq!(tok(Tok::Accent(A::Amber)), t.palette.accent_fill(elegance::Accent::Amber));
        assert_eq!(tok(Tok::Accent(A::Purple)), t.palette.accent_fill(elegance::Accent::Purple));
        assert_eq!(tok(Tok::Accent(A::Sky)), t.palette.accent_fill(elegance::Accent::Sky));
        assert!(t.palette.is_dark, "Elegance ships the dark slate palette");

        // ── Ours, and deliberately independent of the crate ──────────────────
        let hex = |c: Color32| format!("#{:02x}{:02x}{:02x}", c.r(), c.g(), c.b());
        for (name, got, want) in [
            ("form background", hex(tok(Tok::FormBackground)), "#0f172a"),
            ("container background", hex(tok(Tok::Card)), "#20293a"),
            ("input text", hex(tok(Tok::Text)), "#ffffff"),
            ("label text", hex(tok(Tok::LabelText)), "#8691a3"),
            ("border", hex(tok(Tok::Border)), "#8691a3"),
            // The travelled part of a rail is the highlighted one, so the fill
            // is the primary and the muted colour stays on the rail behind it.
            // It was the muted colour on both, which read back to front.
            ("slider fill", hex(tok(Tok::SliderFill)), "#3761e2"),
            ("slider knob", hex(tok(Tok::SliderKnob)), "#ffffff"),
        ] {
            assert_eq!(got, want, "{name} is PowerRustCOBOL's, not the crate's");
        }

        // 5 for EVERYTHING — controls and cards alike. The crate offers two
        // different radii; a form built from both reads as two design languages
        // sharing a window.
        assert_eq!(e.radius(RK::Control), Some(5.0));
        assert_eq!(e.radius(RK::Card), Some(5.0));
        assert_ne!(
            t.control_radius, t.card_radius,
            "the crate does differ here — which is why we do not take its values"
        );

        println!(
            "\n  Elegance — ours: form {} container {} text {} label {} \
             border {} slider {}/{}\n           crate's: input_bg {:?} focus {:?} \
             6 accents, radii {}/{}\n",
            hex(tok(Tok::FormBackground)),
            hex(tok(Tok::Card)),
            hex(tok(Tok::Text)),
            hex(tok(Tok::LabelText)),
            hex(tok(Tok::Border)),
            hex(tok(Tok::SliderFill)),
            hex(tok(Tok::SliderKnob)),
            t.palette.input_bg,
            t.palette.focus,
            t.control_radius,
            t.card_radius
        );
    }

    /// T6 — alpha scaling preserves a colour's own transparency.
    #[test]
    fn elegance_alpha_scales_without_discarding_source_alpha() {
        let opaque = Color32::from_rgb(10, 20, 30);
        assert_eq!(theme_alpha(opaque, 1.0).a(), 255);
        assert_eq!(theme_alpha(opaque, 0.5).a(), 128);
        let half = Color32::from_rgba_unmultiplied(10, 20, 30, 128);
        assert_eq!(theme_alpha(half, 1.0).a(), 128);
        assert_eq!(theme_alpha(half, 0.5).a(), 64);
    }

    /// T7/AC9 — under Elegance the glass style is inert.
    ///
    /// The whole point of the R13 seam: before it, a checkbox's tick box (and
    /// six other sub-elements) called `draw_glass_auto` unconditionally, so
    /// they still changed with `GlassStyle` under a non-glass theme. If this
    /// ever fails again, a sub-element has slipped back onto the glass path.
    #[test]
    fn elegance_is_unaffected_by_every_glass_style() {
        let fixture = r4_fixture();
        let counts: Vec<(crate::model::GlassStyle, usize)> = ALL_GLASS_STYLES
            .iter()
            .map(|gs| {
                (
                    *gs,
                    painted_leaf_count(&fixture, *gs, None, eleg()),
                )
            })
            .collect();

        println!("\n  Elegance under each glass style (must all match):");
        for (gs, n) in &counts {
            println!("    {:<16} {n}", format!("{gs:?}"));
        }

        let first = counts[0].1;
        for (gs, n) in &counts {
            assert_eq!(
                *n, first,
                "GlassStyle {gs:?} changed Elegance's painting ({n} vs {first}) \
                 — a sub-element is still on the glass path (R12/R13)"
            );
        }

        // And Elegance must actually differ from Liquid Glass, or this test
        // would pass trivially by never having switched theme at all.
        let glass = painted_leaf_count(
            &fixture,
            crate::model::GlassStyle::Classic,
            None,
            self::glass(),
        );
        assert_ne!(first, glass, "Elegance is painting exactly like Liquid Glass");
        println!("    (Liquid Glass, for contrast: {glass})\n");
    }

    /// 050 AC2/AC3 — **the reported defect.**
    ///
    /// A self-contained theme must ignore `GlassStyle` entirely: the same
    /// rendering under all four, and a `ShadowEnabled` control casting its drop
    /// shadow under every one of them.
    ///
    /// It did not. `is_neumorphic` was read unconditionally, so selecting
    /// Neumorphic (or Neumorphic Dark) while a flat theme was active painted
    /// neumorphic rims on a surface with no relief AND — because
    /// `regular_drop_shadow` bails when neumorphic — silently suppressed every
    /// drop shadow the developer had switched on. Nothing in the UI said so;
    /// the property simply stopped working.
    #[test]
    fn a_self_contained_theme_ignores_every_glass_style() {
        use crate::model::PropValue;

        /// Shapes painted OUTSIDE the control's own rect — where a drop shadow
        /// is, and where nothing else this fixture draws can be.
        fn shadow_shapes(
            theme: Arc<dyn crate::surface_theme::SurfaceTheme>,
            gs: GS,
            shadow: bool,
        ) -> usize {
            let ctx = egui::Context::default();
            set_glass_style(&ctx, gs);
            set_surface_theme(&ctx, theme);
            let mut c = Control::new("P", CT::Panel, 0, 0);
            c.rect = crate::model::Rect::new(60, 60, 160, 100);
            if shadow {
                c.set_prop("ShadowEnabled", PropValue::Bool(true));
                c.set_prop("ShadowDirection", PropValue::String("South".into()));
                c.set_prop("ShadowDistance", PropValue::Int(12));
            }
            let face = Rect::from_min_size(Pos2::new(60.0, 60.0), Vec2::new(160.0, 100.0));
            let mut input = egui::RawInput::default();
            input.screen_rect = Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(400.0, 300.0)));
            let mut full = ctx.run_ui(input, |root_ui| {
                egui::CentralPanel::default()
                    .frame(egui::Frame::NONE)
                    .show_inside(root_ui, |ui| {
                        draw_control(ui.painter(), Pos2::ZERO, &c, false, true, 1.0, 1.0, None);
                    });
            });
            full.textures_delta.clear();
            fn walk(s: &egui::Shape, face: Rect, n: &mut usize) {
                match s {
                    egui::Shape::Vec(v) => v.iter().for_each(|s| walk(s, face, n)),
                    other => {
                        let b = other.visual_bounding_rect();
                        if b.is_positive() && b.max.y > face.max.y + 1.0 {
                            *n += 1;
                        }
                    }
                }
            }
            let mut n = 0;
            for cs in &full.shapes {
                walk(&cs.shape, face, &mut n);
            }
            n
        }

        let mut rows = Vec::new();
        for gs in ALL_GLASS_STYLES {
            let with = shadow_shapes(eleg(), gs, true);
            let without = shadow_shapes(eleg(), gs, false);
            assert!(
                with > without,
                "{gs:?}: a self-contained theme must still cast the developer's \
                 drop shadow ({with} vs {without} shapes below the face)"
            );
            rows.push((gs, with, without));
        }

        // …and the whole rendering is identical across the four, not merely the
        // shadow (AC3: no neumorphic relief, no asymmetric rim).
        let counts: Vec<usize> = ALL_GLASS_STYLES
            .iter()
            .map(|gs| painted_leaf_count(&r4_fixture(), *gs, None, eleg()))
            .collect();
        assert!(
            counts.windows(2).all(|w| w[0] == w[1]),
            "the glass style still changes what a self-contained theme paints: {counts:?}"
        );

        // The gate is closed for it, and open for Liquid Glass.
        let ctx = egui::Context::default();
        set_surface_theme(&ctx, eleg());
        assert!(!glass_config_applies(&ctx));
        set_surface_theme(&ctx, glass());
        assert!(glass_config_applies(&ctx));

        println!(
            "\n  050 AC2/AC3 — self-contained theme vs GlassStyle\n  \
             {:<16} shadow on / off   full-fixture leaves",
            "glass style"
        );
        for ((gs, with, without), leaves) in rows.iter().zip(&counts) {
            println!("  {:<16} {with:>6} / {without:<5}   {leaves}", format!("{gs:?}"));
        }
        println!(
            "  → identical across all four ({}); shadows cast under every one\n",
            counts[0]
        );
    }

    /// 050 AC8 — a context that never published renders exactly like one that
    /// published Liquid Glass.
    ///
    /// The load-bearing default: four surfaces publish per frame, and any that
    /// misses the call must render as it does today rather than silently
    /// changing look.
    #[test]
    fn an_unpublished_theme_is_liquid_glass() {
        let fixture = r4_fixture();
        let mut rows = Vec::new();
        for gs in ALL_GLASS_STYLES {
            // `painted_leaf_count` always publishes, so the "never published"
            // case is rendered here by hand.
            let ctx = egui::Context::default();
            set_glass_style(&ctx, gs);
            let mut input = egui::RawInput::default();
            input.screen_rect = Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(900.0, 700.0)));
            let mut full = ctx.run_ui(input, |root_ui| {
                egui::CentralPanel::default()
                    .frame(egui::Frame::NONE)
                    .show_inside(root_ui, |ui| {
                        for c in &fixture {
                            draw_control(ui.painter(), Pos2::ZERO, c, false, true, 1.0, 1.0, None);
                        }
                    });
            });
            full.textures_delta.clear();
            fn leaves(s: &egui::Shape) -> usize {
                match s {
                    egui::Shape::Vec(v) => v.iter().map(leaves).sum(),
                    _ => 1,
                }
            }
            let silent: usize = full.shapes.iter().map(|cs| leaves(&cs.shape)).sum();
            let published = painted_leaf_count(&fixture, gs, None, glass());
            assert_eq!(
                silent, published,
                "{gs:?}: an unpublished context must render as Liquid Glass"
            );
            rows.push((gs, silent, published));
        }
        println!("\n  050 AC8 — {:<16} unpublished / published", "glass style");
        for (gs, a, b) in &rows {
            println!("             {:<16} {a} / {b}", format!("{gs:?}"));
        }
        println!();
    }

    /// 050 AC14/R22 — no user-facing string names the third-party crate.
    ///
    /// "Elegance" is the only name a developer ever sees. The palette crate it
    /// follows is an implementation detail, held privately by `EleganceTheme`
    /// so it cannot leak by accident.
    #[test]
    fn no_user_facing_string_names_the_crate() {
        let cat = crate::theme::ThemeCatalog::builtin();
        let mut checked = Vec::new();
        for t in cat.themes() {
            for s in [t.id.as_str(), t.display_name.as_str()] {
                assert!(
                    !s.to_ascii_lowercase().contains("elegance-ui")
                        && !s.to_ascii_lowercase().contains("egui_elegance"),
                    "catalogue string names the crate: {s:?}"
                );
                checked.push(s.to_owned());
            }
        }
        println!(
            "\n  050 AC14 — {} catalogue strings checked, none names the crate: {:?}\n",
            checked.len(),
            checked
        );
    }

    /// The Switch, as reported: it must grow with its control, and its ON
    /// colour is the developer's `Accent`, not the theme's.
    ///
    /// The track was capped at 32x18 whatever the control's size, so a resize
    /// moved the handles and nothing else. And the theme was supplying both
    /// toggle states here, so a switch on the canvas was green whatever Accent
    /// said — while Run Form, which uses the real widget, honoured it. Two
    /// surfaces disagreeing about one control.
    #[test]
    fn a_switch_grows_with_its_control_and_takes_the_developers_accent() {
        use crate::model::PropValue;

        fn fills(ct: &Control, theme: Arc<dyn crate::surface_theme::SurfaceTheme>) -> Vec<Color32> {
            let ctx = egui::Context::default();
            set_surface_theme(&ctx, theme);
            let mut input = egui::RawInput::default();
            input.screen_rect = Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(600.0, 400.0)));
            let mut full = ctx.run_ui(input, |ui| {
                draw_control(ui.painter(), Pos2::ZERO, ct, false, true, 1.0, 1.0, None);
            });
            full.textures_delta.clear();
            fn walk(s: &egui::Shape, out: &mut Vec<(Color32, Rect)>) {
                match s {
                    egui::Shape::Vec(v) => v.iter().for_each(|s| walk(s, out)),
                    egui::Shape::Rect(r) => out.push((r.fill, r.rect)),
                    _ => {}
                }
            }
            let mut seen = Vec::new();
            for cs in &full.shapes {
                walk(&cs.shape, &mut seen);
            }
            seen.into_iter().filter(|(c, _)| c.a() > 0).map(|(c, _)| c).collect()
        }

        fn track_width(ct: &Control) -> f32 {
            let ctx = egui::Context::default();
            set_surface_theme(&ctx, eleg());
            let mut input = egui::RawInput::default();
            input.screen_rect = Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(600.0, 400.0)));
            let mut full = ctx.run_ui(input, |ui| {
                draw_control(ui.painter(), Pos2::ZERO, ct, false, true, 1.0, 1.0, None);
            });
            full.textures_delta.clear();
            fn widest(s: &egui::Shape, out: &mut f32) {
                match s {
                    egui::Shape::Vec(v) => v.iter().for_each(|s| widest(s, out)),
                    other => {
                        let b = other.visual_bounding_rect();
                        if b.is_positive() {
                            *out = out.max(b.width());
                        }
                    }
                }
            }
            let mut w = 0.0;
            for cs in &full.shapes {
                widest(&cs.shape, &mut w);
            }
            w
        }

        // ── It grows with the control ────────────────────────────────────────
        let mut small = Control::new("S", CT::Switch, 0, 0);
        small.rect = crate::model::Rect::new(0, 0, 52, 28);
        let mut big = small.clone();
        big.rect = crate::model::Rect::new(0, 0, 200, 60);
        let (ws, wb) = (track_width(&small), track_width(&big));
        assert!(
            wb > ws * 2.0,
            "a switch must follow its control's size: 52pt ⇒ {ws:.0}, 200pt ⇒ {wb:.0}"
        );
        assert!(wb >= 190.0, "…and actually fill it, got {wb:.0}");

        // ── ON takes the developer's Accent, under every theme ───────────────
        let mut on = small.clone();
        on.set_prop("Checked", PropValue::Bool(true));
        on.set_prop("Accent", PropValue::String("Blue".into()));
        let same = |a: Color32, b: Color32| a.r() == b.r() && a.g() == b.g() && a.b() == b.b();
        let themed_green = eleg()
            .surface(
                crate::surface_theme::SurfaceRole::Toggle,
                crate::surface_theme::SurfaceState { selected: false, on: true },
            )
            .and_then(|s| s.fill)
            .expect("the theme has an on colour");

        for (name, theme) in [("Elegance", eleg()), ("Liquid Glass", glass())] {
            let painted = fills(&on, theme);
            assert!(
                !painted.iter().any(|c| same(*c, themed_green)),
                "{name}: the theme's toggle colour must not override Accent"
            );
        }

        // Changing Accent changes what is painted — the whole point.
        let mut red = on.clone();
        red.set_prop("Accent", PropValue::String("Red".into()));
        assert_ne!(
            format!("{:?}", fills(&on, eleg())),
            format!("{:?}", fills(&red, eleg())),
            "Accent must drive the ON colour"
        );

        println!(
            "\n  Switch — 52pt control ⇒ {ws:.0}pt track, 200pt ⇒ {wb:.0}pt; \
             ON follows Accent (Blue ≠ Red) under both themes\n"
        );
    }

    /// Every property the ProgressBar inspector offers must reach the paint.
    ///
    /// As reported: CornerRadius and ShowValue did nothing, Orientation was
    /// stuck on Horizontal, Style on Continuous, and BarColor on green. Four of
    /// the five never reached the painter at all; BarColor reached only the
    /// flat path, while the glass path — the one both the canvas and Run Form
    /// use — was handed a hard-wired green.
    #[test]
    fn a_progress_bars_back_colour_paints_the_untravelled_part() {
        use crate::model::PropValue;

        /// The fill of every full-size rect the control paints — the trough is
        /// the one that spans the whole control.
        fn trough_fills(ct: &Control) -> Vec<Color32> {
            let ctx = egui::Context::default();
            set_surface_theme(&ctx, glass());
            let mut input = egui::RawInput::default();
            input.screen_rect = Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(600.0, 400.0)));
            let mut full = ctx.run_ui(input, |ui| {
                draw_control(ui.painter(), Pos2::ZERO, ct, false, false, 1.0, 1.0, None);
            });
            full.textures_delta.clear();
            let want = Rect::from_min_size(Pos2::ZERO, Vec2::new(ct.rect.w as f32, ct.rect.h as f32));
            let mut out = Vec::new();
            fn walk(s: &egui::Shape, want: Rect, out: &mut Vec<Color32>) {
                match s {
                    egui::Shape::Vec(v) => v.iter().for_each(|s| walk(s, want, out)),
                    egui::Shape::Rect(r) => {
                        if (r.rect.width() - want.width()).abs() <= 0.5
                            && (r.rect.height() - want.height()).abs() <= 0.5
                            && r.fill.a() > 0
                        {
                            out.push(r.fill);
                        }
                    }
                    _ => {}
                }
            }
            for cs in &full.shapes {
                walk(&cs.shape, want, &mut out);
            }
            out
        }

        let mut bar = Control::new("PB", CT::ProgressBar, 0, 0);
        bar.rect = crate::model::Rect::new(0, 0, 200, 24);
        bar.set_prop("Value", PropValue::Int(50));

        // Untouched: the trough is whatever the theme says, never magenta.
        let before = trough_fills(&bar);
        assert!(
            !before.iter().any(|c| *c == Color32::from_rgb(0xFF, 0x00, 0xFF)),
            "nothing paints magenta until the developer asks for it"
        );

        // Chosen: the untravelled part takes the developer's Back colour. This
        // row was dead — the trough only ever asked the theme.
        bar.set_prop("BackgroundColor", PropValue::String("#FF00FF".into()));
        let after = trough_fills(&bar);
        assert!(
            after.iter().any(|c| *c == Color32::from_rgb(0xFF, 0x00, 0xFF)),
            "the trough must take BackgroundColor, got {after:?}"
        );

        // The seeded default is not a choice, so it must not override the theme.
        bar.set_prop(
            "BackgroundColor",
            PropValue::String(crate::model::DEFAULT_BACKGROUND_COLOR.into()),
        );
        assert_eq!(
            trough_fills(&bar),
            before,
            "still on the default means the developer has not chosen"
        );
    }

    #[test]
    fn a_progress_bar_honours_every_property_it_offers() {
        use crate::model::PropValue;

        /// `(fill, rect, corner radius)` of every rect the control paints, and
        /// the colour of every string.
        ///
        /// `glass` picks the surface: the frosted fill the canvas and Run Form
        /// use, or the flat one the thumbnails use. The bar's GEOMETRY is
        /// decided before that branch and is the same on both, so the layout
        /// assertions read it off the flat path — one rect per run of ink,
        /// instead of the two dozen layers frost stacks up.
        fn ink(
            ct: &Control,
            theme: Arc<dyn crate::surface_theme::SurfaceTheme>,
            glass: bool,
        ) -> (Vec<(Color32, Rect, f32)>, Vec<Color32>) {
            let ctx = egui::Context::default();
            set_surface_theme(&ctx, theme);
            let mut input = egui::RawInput::default();
            input.screen_rect = Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(600.0, 400.0)));
            let mut full = ctx.run_ui(input, |ui| {
                draw_control(ui.painter(), Pos2::ZERO, ct, false, glass, 1.0, 1.0, None);
            });
            full.textures_delta.clear();
            fn walk(
                s: &egui::Shape,
                rects: &mut Vec<(Color32, Rect, f32)>,
                texts: &mut Vec<Color32>,
            ) {
                match s {
                    egui::Shape::Vec(v) => v.iter().for_each(|s| walk(s, rects, texts)),
                    egui::Shape::Rect(r) => {
                        rects.push((r.fill, r.rect, r.corner_radius.nw as f32))
                    }
                    egui::Shape::Text(t) => {
                        texts.push(t.override_text_color.unwrap_or(t.fallback_color))
                    }
                    _ => {}
                }
            }
            let (mut rects, mut texts) = (Vec::new(), Vec::new());
            for cs in &full.shapes {
                walk(&cs.shape, &mut rects, &mut texts);
            }
            (rects, texts)
        }

        let bar = |w: i32, h: i32| {
            let mut c = Control::new("PB", CT::ProgressBar, 0, 0);
            c.rect = crate::model::Rect::new(0, 0, w, h);
            c.set_prop("Value", PropValue::Int(50));
            c
        };
        // The runs of bar ink: everything that is neither the full-size trough
        // nor invisible.
        let fills = |ct: &Control| {
            let (rects, _) = ink(ct, glass(), false);
            let full =
                Rect::from_min_size(Pos2::ZERO, Vec2::new(ct.rect.w as f32, ct.rect.h as f32));
            rects
                .into_iter()
                .filter(|(c, r, _)| {
                    c.a() > 0 && r.area() > 1.0 && (r.width() < full.width() - 0.5
                        || r.height() < full.height() - 0.5)
                })
                .map(|(_, r, _)| r)
                .collect::<Vec<_>>()
        };

        // ── Orientation ──────────────────────────────────────────────────────
        let horiz = fills(&bar(200, 24));
        let mut v = bar(24, 200);
        v.set_prop("Orientation", PropValue::String("Vertical".into()));
        let vert = fills(&v);
        assert_eq!(horiz.len(), 1, "a continuous bar is one run of ink");
        assert_eq!(vert.len(), 1);
        let (h0, v0) = (horiz[0], vert[0]);
        assert!(
            (h0.max.x - 100.0).abs() <= 1.0 && h0.height() >= 23.0,
            "Horizontal at 50% must fill the left half, got {h0:?}"
        );
        assert!(
            (v0.min.y - 100.0).abs() <= 1.0 && (v0.max.y - 200.0).abs() <= 1.0,
            "Vertical at 50% must fill the BOTTOM half, got {v0:?}"
        );

        // ── Style ────────────────────────────────────────────────────────────
        let mut blocky = bar(200, 24);
        blocky.set_prop("Style", PropValue::String("Blocks".into()));
        let segs = fills(&blocky);
        assert!(
            segs.len() >= 5,
            "Blocks must paint a row of segments, got {} run(s)",
            segs.len()
        );
        assert!(
            segs.iter().all(|r| r.width() < 20.0),
            "…each one shorter than the bar"
        );
        assert!(
            segs.iter().map(|r| r.max.x).fold(0.0_f32, f32::max) <= 101.0,
            "…and none past the 50% mark"
        );

        // ── BarColor, on the surface the IDE actually paints ──────────────────
        let mut green = bar(200, 24);
        green.set_prop("Value", PropValue::Int(100));
        let mut blue = green.clone();
        blue.set_prop("BarColor", PropValue::String("#2563EB".into()));
        for (name, theme) in [("Elegance", eleg()), ("Liquid Glass", glass())] {
            let (g, _) = ink(&green, theme.clone(), true);
            let (b, _) = ink(&blue, theme, true);
            assert_ne!(
                format!("{g:?}"),
                format!("{b:?}"),
                "{name}: BarColor must drive the frosted fill"
            );
        }
        // Flat surfaces carry the developer's colour literally.
        let (flat, _) = ink(&blue, glass(), false);
        assert!(
            flat.iter()
                .any(|(c, _, _)| c.b() > c.r() && c.b() > c.g() && c.a() > 0),
            "a blue bar must paint blue ink, got {flat:?}"
        );

        // ── An untouched bar belongs to the theme it sits in ─────────────────
        // Every other control treats "still on the seeded value" as "the
        // developer has not chosen" and lets the theme answer. The bar was the
        // exception: under Elegance everything around it drew from the palette
        // while it stayed its own built-in green.
        let mut untouched = bar(200, 24);
        untouched.set_prop("Value", PropValue::Int(100));
        let theme_green = eleg()
            .token(crate::surface_theme::ColorToken::Accent(
                crate::surface_theme::AccentName::Green,
            ))
            .expect("Elegance has a green accent");
        let (eleg_ink, _) = ink(&untouched, eleg(), false);
        assert!(
            eleg_ink.iter().any(|(c, _, _)| (c.r(), c.g(), c.b())
                == (theme_green.r(), theme_green.g(), theme_green.b())),
            "an untouched bar must fill with the THEME's green {theme_green:?}, got {eleg_ink:?}"
        );
        // With no theme to ask, the built-in green stands.
        let (glass_ink, _) = ink(&untouched, glass(), false);
        assert!(
            glass_ink
                .iter()
                .any(|(c, _, _)| (c.r(), c.g(), c.b()) == (0, 170, 0)),
            "without a theme the built-in green stands, got {glass_ink:?}"
        );

        // ── CornerRadius ─────────────────────────────────────────────────────
        let mut round = bar(200, 24);
        round.set_prop("CornerRadius", PropValue::Int(10));
        let (rects, _) = ink(&round, glass(), true);
        assert!(
            rects
                .iter()
                .any(|(_, r, cr)| r.width() >= 199.0 && (*cr - 10.0).abs() < 0.6),
            "the trough must follow CornerRadius, got {:?}",
            rects.iter().map(|(_, _, cr)| *cr).collect::<Vec<_>>()
        );

        // ── ShowValue ────────────────────────────────────────────────────────
        let mut shown = bar(200, 24);
        shown.set_prop("ShowValue", PropValue::Bool(true));
        assert!(ink(&bar(200, 24), eleg(), true).1.is_empty(), "off ⇒ no text");
        for (name, theme) in [("Elegance", eleg()), ("Liquid Glass", glass())] {
            let (rects, texts) = ink(&shown, theme, true);
            assert_eq!(texts.len(), 1, "{name}: the percentage must be painted");
            // Painted is not read: black on a dark well is what "ignored"
            // looked like. The text has to stand off the trough it sits on.
            let trough = rects
                .iter()
                .find(|(c, r, _)| r.width() >= 199.0 && c.a() > 0)
                .map(|(c, _, _)| *c)
                .expect("the trough is painted");
            let lum = |c: Color32| {
                0.299 * c.r() as f32 + 0.587 * c.g() as f32 + 0.114 * c.b() as f32
            };
            assert!(
                (lum(texts[0]) - lum(trough)).abs() > 40.0,
                "{name}: the percentage must be legible on the trough \
                 (text {:?} vs trough {trough:?})",
                texts[0]
            );
        }

        // ── ForegroundColor ──────────────────────────────────────────────────
        // The legible fallback above is for an UNTOUCHED control. A developer
        // who picks a colour gets that colour — the fallback was hard-wired,
        // which left ForegroundColor looking dead (locked on the theme's white).
        let mut red_text = shown.clone();
        red_text.set_prop("ForegroundColor", PropValue::String("#FF0000".into()));
        for (name, theme) in [("Elegance", eleg()), ("Liquid Glass", glass())] {
            let (_, texts) = ink(&red_text, theme, true);
            assert_eq!(
                (texts[0].r(), texts[0].g(), texts[0].b()),
                (255, 0, 0),
                "{name}: ForegroundColor must paint the percentage, got {:?}",
                texts[0]
            );
        }

        // ── Border ───────────────────────────────────────────────────────────
        // Three properties, not two constants: None paints no frame at all,
        // and a chosen colour is the one that lands.
        let mut framed = bar(200, 24);
        framed.set_prop("BorderColor", PropValue::String("#FF00FF".into()));
        framed.set_prop("BorderWidth", PropValue::Int(3));
        let strokes = |ct: &Control| -> Vec<(Color32, f32)> {
            let ctx = egui::Context::default();
            let mut input = egui::RawInput::default();
            input.screen_rect = Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(600.0, 400.0)));
            let mut full = ctx.run_ui(input, |ui| {
                draw_control(ui.painter(), Pos2::ZERO, ct, false, false, 1.0, 1.0, None);
            });
            full.textures_delta.clear();
            fn walk(s: &egui::Shape, out: &mut Vec<(Color32, f32)>) {
                match s {
                    egui::Shape::Vec(v) => v.iter().for_each(|s| walk(s, out)),
                    egui::Shape::Rect(r) if r.stroke.width > 0.0 => {
                        out.push((r.stroke.color, r.stroke.width))
                    }
                    _ => {}
                }
            }
            let mut seen = Vec::new();
            for cs in &full.shapes {
                walk(&cs.shape, &mut seen);
            }
            seen
        };
        let framed_strokes = strokes(&framed);
        assert!(
            framed_strokes
                .iter()
                .any(|(c, w)| (c.r(), c.g(), c.b()) == (255, 0, 255) && (*w - 3.0).abs() < 0.01),
            "BorderColor and BorderWidth must paint the frame, got {framed_strokes:?}"
        );
        let mut bare = bar(200, 24);
        bare.set_prop("BorderStyle", PropValue::String("None".into()));
        assert!(
            strokes(&bare).is_empty(),
            "BorderStyle None must paint no frame, got {:?}",
            strokes(&bare)
        );

        println!(
            "\n  ProgressBar — Orientation (H fills x≤{:.0}, V fills y≥{:.0}), \
             Style (Blocks ⇒ {} segments), BarColor, CornerRadius (10px trough), \
             ShowValue, ForegroundColor and the three border properties \
             all reach the paint\n",
            h0.max.x,
            v0.min.y,
            segs.len()
        );
    }

    /// `BlockSize` sets how long one block of a segmented bar is. At its
    /// default of 0 the size stays automatic — read off the bar's thickness —
    /// so a form that never touches it looks exactly as it did.
    #[test]
    fn a_segmented_progress_bar_takes_the_developers_block_size() {
        use crate::model::PropValue;

        /// The widths of the blocks a full bar paints, in order.
        fn widths(ct: &Control) -> Vec<f32> {
            let ctx = egui::Context::default();
            let mut input = egui::RawInput::default();
            input.screen_rect = Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(600.0, 400.0)));
            let mut full = ctx.run_ui(input, |ui| {
                draw_control(ui.painter(), Pos2::ZERO, ct, false, false, 1.0, 1.0, None);
            });
            full.textures_delta.clear();
            fn walk(s: &egui::Shape, out: &mut Vec<(Color32, Rect)>) {
                match s {
                    egui::Shape::Vec(v) => v.iter().for_each(|s| walk(s, out)),
                    egui::Shape::Rect(r) => out.push((r.fill, r.rect)),
                    _ => {}
                }
            }
            let mut seen = Vec::new();
            for cs in &full.shapes {
                walk(&cs.shape, &mut seen);
            }
            seen.into_iter()
                // Everything but the full-width trough and the hollow border.
                .filter(|(c, r)| c.a() > 0 && r.width() < 199.5 && r.area() > 1.0)
                .map(|(_, r)| r.width())
                .collect()
        }

        let mut auto = Control::new("PB", CT::ProgressBar, 0, 0);
        auto.rect = crate::model::Rect::new(0, 0, 200, 24);
        auto.set_prop("Value", PropValue::Int(100));
        auto.set_prop("Style", PropValue::String("Blocks".into()));
        let mut sized = auto.clone();
        sized.set_prop("BlockSize", PropValue::Int(8));

        let (a, s) = (widths(&auto), widths(&sized));
        // Automatic: two thirds of the bar's 24pt thickness.
        assert!(
            (a[0] - 15.84).abs() < 0.5,
            "0 must keep the automatic size, got {:.2}",
            a[0]
        );
        // Chosen: 8pt, every block but the clipped last one.
        assert!(
            s.iter().rev().skip(1).all(|w| (*w - 8.0).abs() < 0.01),
            "BlockSize must set the block length, got {s:?}"
        );
        assert!(
            s.len() > a.len(),
            "shorter blocks means more of them: {} at 8pt vs {} automatic",
            s.len(),
            a.len()
        );

        println!(
            "\n  ProgressBar Blocks — automatic ⇒ {} blocks of {:.2}pt, \
             BlockSize 8 ⇒ {} blocks of 8pt\n",
            a.len(),
            a[0],
            s.len()
        );
    }

    /// A Slider's rail reads the right way round — the travelled part is the
    /// highlighted one — and the three colour properties the inspector offers
    /// actually reach the paint.
    ///
    /// They used to be parsed and dropped on the floor (`let _ = (track_c,
    /// thumb_c, fill_c)`), and the theme's fill was its MUTED grey, so the
    /// travelled part came out duller than the part still to travel.
    #[test]
    fn a_slider_fills_the_travelled_side_and_takes_the_developers_colours() {
        use crate::model::PropValue;

        /// Every rect fill the control painted, un-premultiplied.
        fn fills(ct: &Control) -> Vec<[u8; 4]> {
            let ctx = egui::Context::default();
            set_surface_theme(&ctx, eleg());
            let mut input = egui::RawInput::default();
            input.screen_rect = Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(600.0, 400.0)));
            let mut full = ctx.run_ui(input, |ui| {
                draw_control(ui.painter(), Pos2::ZERO, ct, false, true, 1.0, 1.0, None);
            });
            full.textures_delta.clear();
            fn walk(s: &egui::Shape, out: &mut Vec<[u8; 4]>) {
                match s {
                    egui::Shape::Vec(v) => v.iter().for_each(|s| walk(s, out)),
                    egui::Shape::Rect(r) => out.push(r.fill.to_srgba_unmultiplied()),
                    _ => {}
                }
            }
            let mut out = Vec::new();
            for cs in &full.shapes {
                walk(&cs.shape, &mut out);
            }
            out
        }
        // Premultiplying and unmultiplying costs a unit either way.
        let has = |seen: &[[u8; 4]], rgb: [u8; 3]| -> bool {
            seen.iter().any(|c| {
                c[3] > 0
                    && (0..3).all(|i| (c[i] as i16 - rgb[i] as i16).abs() <= 2)
            })
        };

        let mut s = Control::new("S", CT::Slider, 0, 0);
        s.rect = crate::model::Rect::new(0, 0, 240, 40);
        s.set_prop("Value", PropValue::Int(50));

        // Untouched: the theme paints, and the travelled part is the PRIMARY —
        // not the muted grey that reads as "not yet travelled".
        let themed = fills(&s);
        assert!(
            has(&themed, [0x37, 0x61, 0xE2]),
            "the travelled part must take the theme's primary; painted {themed:?}"
        );
        assert!(
            has(&themed, [0x86, 0x91, 0xA3]),
            "…over a muted rail for the part still to travel; painted {themed:?}"
        );

        // Each property reaches the paint.
        let mut coloured = s.clone();
        coloured.set_prop("FillColor", PropValue::String("#FF00FF".into()));
        coloured.set_prop("TrackColor", PropValue::String("#00FF00".into()));
        let picked = fills(&coloured);
        assert!(
            has(&picked, [0xFF, 0x00, 0xFF]),
            "FillColor must paint the travelled part; painted {picked:?}"
        );
        assert!(
            has(&picked, [0x00, 0xFF, 0x00]),
            "TrackColor must paint the rail; painted {picked:?}"
        );

        println!(
            "\n  Slider — untouched: #3761e2 travelled over #8691a3 rail; \
             FillColor/TrackColor picked: #ff00ff over #00ff00\n"
        );
    }

    /// `Accent` takes a picked colour, and still answers to the six names the
    /// property was limited to before it grew a colour picker.
    #[test]
    fn accent_takes_any_colour_and_still_knows_the_six_names() {
        assert_eq!(knob_accent("#FF00FF"), Color32::from_rgb(255, 0, 255));
        // Eight digits carry alpha. The colour is premultiplied on the way in,
        // so the channels come back within a unit of what was typed.
        let picked = knob_accent("#12345678").to_srgba_unmultiplied();
        assert_eq!(picked[3], 0x78, "the alpha a developer typed is kept");
        assert!((picked[0] as i16 - 0x12).abs() <= 1, "got {picked:?}");
        // The presets a form saved before the picker existed.
        assert_eq!(knob_accent("Red"), Color32::from_rgb(198, 40, 40));
        assert_eq!(knob_accent("Sky"), Color32::from_rgb(2, 136, 209));
        // Anything else is still Blue — including a half-typed colour, which
        // must never be read as a colour.
        assert_eq!(knob_accent("Blue"), knob_accent("#GG00ZZ"));
        assert_eq!(knob_accent("#FF0"), knob_accent("nonsense"));

        println!("\n  Accent — #FF00FF paints magenta; Red/Sky keep their preset; junk falls back to Blue\n");
    }

    /// The dial's own parts answer to the developer, and an untouched Knob
    /// still leaves every one of them to the theme.
    ///
    /// `Accent` keeps the arc and the indicator; the face, the rim with its
    /// inner ring, and the part of the arc still to travel used to be the
    /// theme's alone — a developer could pick a colour for none of them.
    #[test]
    fn a_knob_paints_its_face_rim_and_track_in_the_developers_colours() {
        use crate::model::PropValue;

        /// `(circle fills, circle strokes, arc colours)`.
        fn ink(ct: &Control) -> (Vec<Color32>, Vec<Color32>, Vec<Color32>) {
            let ctx = egui::Context::default();
            set_surface_theme(&ctx, eleg());
            let mut input = egui::RawInput::default();
            input.screen_rect = Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(600.0, 400.0)));
            let mut full = ctx.run_ui(input, |ui| {
                draw_control(ui.painter(), Pos2::ZERO, ct, false, true, 1.0, 1.0, None);
            });
            full.textures_delta.clear();
            fn walk(
                s: &egui::Shape,
                faces: &mut Vec<Color32>,
                rims: &mut Vec<Color32>,
                arcs: &mut Vec<Color32>,
            ) {
                match s {
                    egui::Shape::Vec(v) => v.iter().for_each(|s| walk(s, faces, rims, arcs)),
                    egui::Shape::Circle(c) => {
                        faces.push(c.fill);
                        rims.push(c.stroke.color);
                    }
                    egui::Shape::Path(p) => {
                        if let egui::epaint::ColorMode::Solid(c) = p.stroke.color {
                            arcs.push(c);
                        }
                    }
                    _ => {}
                }
            }
            let (mut faces, mut rims, mut arcs) = (Vec::new(), Vec::new(), Vec::new());
            for cs in &full.shapes {
                walk(&cs.shape, &mut faces, &mut rims, &mut arcs);
            }
            (faces, rims, arcs)
        }

        let face = Color32::from_rgb(0x20, 0x30, 0x40);
        let rim = Color32::from_rgb(0xC0, 0x50, 0x10);
        let track = Color32::from_rgb(0x00, 0x88, 0x44);

        let mut k = Control::new("K", CT::Knob, 0, 0);
        k.rect = crate::model::Rect::new(0, 0, 80, 96);
        k.set_prop("Value", PropValue::Int(50));

        // Untouched: the theme paints, so none of the three can be on the dial.
        let (faces, rims, arcs) = ink(&k);
        for (seen, colour, part) in [
            (&faces, face, "face"),
            (&rims, rim, "rim"),
            (&arcs, track, "track"),
        ] {
            assert!(
                !seen.contains(&colour),
                "an untouched Knob must leave the {part} to the theme; painted {seen:?}"
            );
        }

        // Each property reaches the paint.
        k.set_prop("FaceColor", PropValue::String("#203040".into()));
        k.set_prop("RimColor", PropValue::String("#C05010".into()));
        k.set_prop("TrackColor", PropValue::String("#008844".into()));
        let (faces, rims, arcs) = ink(&k);
        assert!(
            faces.contains(&face),
            "FaceColor must fill the dial; painted {faces:?}"
        );
        assert!(
            rims.contains(&rim),
            "RimColor must stroke the rim and the inner ring; painted {rims:?}"
        );
        assert!(
            arcs.contains(&track),
            "TrackColor must paint the part still to travel; painted {arcs:?}"
        );

        println!(
            "\n  Knob — FaceColor #203040, RimColor #c05010 and TrackColor #008844 \
             all reach the paint; an untouched dial shows none of them\n"
        );
    }

    /// Zones own the Gauge's fill once BOTH thresholds are set, and are off
    /// entirely while either is blank (spec 039 R8/AC2).
    #[test]
    fn gauge_zones_need_both_thresholds_and_then_run_green_amber_red() {
        use crate::model::PropValue;

        let mut g = Control::new("G", CT::Gauge, 0, 0);
        // Blank by default — a Gauge nobody configured keeps its own colour.
        assert_eq!(gauge_zone_color(&g, 0.99), None);

        // One alone is not enough: zones need a warning AND a critical mark.
        g.set_prop("WarningThreshold", PropValue::String("0.6".into()));
        assert_eq!(gauge_zone_color(&g, 0.99), None, "one threshold is not zones");

        g.set_prop("CriticalThreshold", PropValue::String("0.85".into()));
        let (green, amber, red) = (
            Color32::from_rgb(46, 125, 50),
            Color32::from_rgb(245, 124, 0),
            Color32::from_rgb(198, 40, 40),
        );
        assert_eq!(gauge_zone_color(&g, 0.00), Some(green));
        assert_eq!(gauge_zone_color(&g, 0.59), Some(green));
        assert_eq!(gauge_zone_color(&g, 0.60), Some(amber), "at the mark, not past it");
        assert_eq!(gauge_zone_color(&g, 0.84), Some(amber));
        assert_eq!(gauge_zone_color(&g, 0.85), Some(red), "at the mark, not past it");
        assert_eq!(gauge_zone_color(&g, 1.00), Some(red));

        // Emptying either one puts the developer's own Color back in charge.
        g.set_prop("WarningThreshold", PropValue::String("".into()));
        assert_eq!(gauge_zone_color(&g, 0.99), None);

        println!(
            "\n  Gauge zones — off until both marks are set; then 0.00/0.59 green, \
             0.60/0.84 amber, 0.85/1.00 red\n"
        );
    }

    /// **Each zone is a colour the developer picks**, and each defaults to the
    /// literal it replaced.
    ///
    /// Operator, 2026-08-22: "gauge is using hard coded colors for gauge's
    /// value, warning and critical thresholds". They were three literals in
    /// `gauge_zone_color` on a control whose every other colour is a property.
    #[test]
    fn each_gauge_zone_takes_its_own_colour_and_defaults_to_the_old_one() {
        use crate::model::PropValue;

        let mut g = Control::new("G-1", crate::ControlType::Gauge, 0, 0);
        g.set_prop("WarningThreshold", PropValue::String("0.6".into()));
        g.set_prop("CriticalThreshold", PropValue::String("0.85".into()));

        // Untouched: exactly the greens, ambers and reds that were hard-coded.
        assert_eq!(gauge_zone_color(&g, 0.1), Some(GAUGE_NORMAL_COLOR));
        assert_eq!(gauge_zone_color(&g, 0.7), Some(GAUGE_WARNING_COLOR));
        assert_eq!(gauge_zone_color(&g, 0.9), Some(GAUGE_CRITICAL_COLOR));

        // …and each property moves only its own zone.
        g.set_prop("NormalColor", PropValue::String("#112233".into()));
        g.set_prop("WarningColor", PropValue::String("#445566".into()));
        g.set_prop("CriticalColor", PropValue::String("#778899".into()));
        assert_eq!(
            gauge_zone_color(&g, 0.1),
            Some(Color32::from_rgb(0x11, 0x22, 0x33))
        );
        assert_eq!(
            gauge_zone_color(&g, 0.7),
            Some(Color32::from_rgb(0x44, 0x55, 0x66))
        );
        assert_eq!(
            gauge_zone_color(&g, 0.9),
            Some(Color32::from_rgb(0x77, 0x88, 0x99))
        );

        // A malformed colour costs only itself — one typo must not drag the
        // whole meter back to stock.
        g.set_prop("WarningColor", PropValue::String("not a colour".into()));
        assert_eq!(gauge_zone_color(&g, 0.7), Some(GAUGE_WARNING_COLOR));
        assert_eq!(
            gauge_zone_color(&g, 0.9),
            Some(Color32::from_rgb(0x77, 0x88, 0x99)),
            "the critical zone is untouched by the warning zone's typo"
        );
    }

    /// The Gauge's own switches actually reach the paint: needle, scale and
    /// thumb each add marks, and turning one off takes its marks away.
    #[test]
    fn gauge_needle_scale_and_thumb_are_drawn_and_can_be_turned_off() {
        use crate::model::PropValue;

        /// Every line segment and circle the control painted.
        fn marks(ct: &Control) -> usize {
            let ctx = egui::Context::default();
            set_surface_theme(&ctx, eleg());
            let mut input = egui::RawInput::default();
            input.screen_rect = Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(600.0, 400.0)));
            let mut full = ctx.run_ui(input, |ui| {
                draw_control(ui.painter(), Pos2::ZERO, ct, false, true, 1.0, 1.0, None);
            });
            full.textures_delta.clear();
            fn walk(s: &egui::Shape, n: &mut usize) {
                match s {
                    egui::Shape::Vec(v) => v.iter().for_each(|s| walk(s, n)),
                    egui::Shape::LineSegment { .. } | egui::Shape::Circle(_) => *n += 1,
                    _ => {}
                }
            }
            let mut n = 0;
            for cs in &full.shapes {
                walk(&cs.shape, &mut n);
            }
            n
        }

        let mut radial = Control::new("G", CT::Gauge, 0, 0);
        radial.rect = crate::model::Rect::new(0, 0, 200, 120);
        radial.set_prop("Value", PropValue::Int(50));

        let all = marks(&radial);
        let mut no_scale = radial.clone();
        no_scale.set_prop("ShowScale", PropValue::Bool(false));
        let mut bare = no_scale.clone();
        bare.set_prop("ShowNeedle", PropValue::Bool(false));
        let (without_scale, without_either) = (marks(&no_scale), marks(&bare));

        assert!(
            all > without_scale,
            "ShowScale must draw ticks: on ⇒ {all} marks, off ⇒ {without_scale}"
        );
        assert!(
            without_scale > without_either,
            "ShowNeedle must draw the needle: on ⇒ {without_scale} marks, off ⇒ {without_either}"
        );

        let mut linear = radial.clone();
        linear.set_prop("GaugeStyle", PropValue::String("Linear".into()));
        let mut no_thumb = linear.clone();
        no_thumb.set_prop("ShowThumb", PropValue::Bool(false));
        let (with_thumb, sans_thumb) = (marks(&linear), marks(&no_thumb));
        assert!(
            with_thumb > sans_thumb,
            "ShowThumb must draw the thumb: on ⇒ {with_thumb} marks, off ⇒ {sans_thumb}"
        );

        println!(
            "\n  Gauge — Radial: {all} marks with needle+scale, {without_scale} without the \
             scale, {without_either} with neither; Linear: {with_thumb} with the thumb, \
             {sans_thumb} without\n"
        );
    }

    /// A Donut honours `ShowNeedle` exactly like the Radial — and the needle
    /// takes the developer's own `Color`, not a theme's. Mirrors the operator's
    /// report (2026-08-15): a Donut gauge saved with `ShowNeedle=true` and
    /// `Color=#16A34AFF` painted no needle at all.
    #[test]
    fn donut_needle_is_drawn_in_the_developers_colour_and_can_be_turned_off() {
        use crate::model::PropValue;

        /// `(needle stroke colours, hub fill colours)` painted for `ct`.
        fn needle_ink(ct: &Control) -> (Vec<Color32>, Vec<Color32>) {
            let ctx = egui::Context::default();
            set_surface_theme(&ctx, eleg());
            let mut input = egui::RawInput::default();
            input.screen_rect = Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(600.0, 400.0)));
            let mut full = ctx.run_ui(input, |ui| {
                draw_control(ui.painter(), Pos2::ZERO, ct, false, true, 1.0, 1.0, None);
            });
            full.textures_delta.clear();
            fn walk(s: &egui::Shape, segs: &mut Vec<Color32>, hubs: &mut Vec<Color32>) {
                match s {
                    egui::Shape::Vec(v) => v.iter().for_each(|s| walk(s, segs, hubs)),
                    egui::Shape::LineSegment { stroke, .. } => segs.push(stroke.color),
                    egui::Shape::Circle(c) => hubs.push(c.fill),
                    _ => {}
                }
            }
            let (mut segs, mut hubs) = (Vec::new(), Vec::new());
            for cs in &full.shapes {
                walk(&cs.shape, &mut segs, &mut hubs);
            }
            (segs, hubs)
        }

        // The operator's control, prop for prop: 160x144, Value 80 of 0..100,
        // StrokeWidth 15, Color #16A34AFF.
        let mut donut = Control::new("G", CT::Gauge, 0, 0);
        donut.rect = crate::model::Rect::new(0, 0, 160, 144);
        donut.set_prop("GaugeStyle", PropValue::String("Donut".into()));
        donut.set_prop("Value", PropValue::Int(80));
        donut.set_prop("StrokeWidth", PropValue::Int(15));
        donut.set_prop("Color", PropValue::String("#16A34AFF".into()));

        let green = Color32::from_rgb(0x16, 0xA3, 0x4A);
        let (segs, hubs) = needle_ink(&donut);
        assert!(
            segs.contains(&green),
            "ShowNeedle defaults on: the Donut must draw its needle in the \
             developer's #16A34A, got segments {segs:?}"
        );
        assert!(
            hubs.contains(&green),
            "the needle's hub must take the same colour, got circles {hubs:?}"
        );

        let mut bare = donut.clone();
        bare.set_prop("ShowNeedle", PropValue::Bool(false));
        let (segs_off, hubs_off) = needle_ink(&bare);
        assert!(
            segs_off.is_empty() && hubs_off.is_empty(),
            "ShowNeedle off must remove needle and hub: segments {segs_off:?}, \
             circles {hubs_off:?}"
        );

        println!(
            "\n  Gauge — Donut: needle + hub in the developer's #16A34A when on \
             ({} segments, {} circles), none when off\n",
            segs.len(),
            hubs.len()
        );
    }

    /// `NeedleColor` paints the needle and its hub, and nothing else on the
    /// gauge (operator, 2026-08-16: "the needle needs a property to define its
    /// colour"). Blank keeps the meter's own ink, which is all the needle ever
    /// had, so an untouched gauge is unchanged.
    #[test]
    fn the_needle_takes_its_own_colour_without_repainting_the_meter() {
        use crate::model::PropValue;

        /// `(line-segment colours, circle fills, ring/arc path colours)`.
        fn ink(ct: &Control) -> (Vec<Color32>, Vec<Color32>, Vec<Color32>) {
            let ctx = egui::Context::default();
            set_surface_theme(&ctx, eleg());
            let mut input = egui::RawInput::default();
            input.screen_rect = Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(600.0, 400.0)));
            let mut full = ctx.run_ui(input, |ui| {
                draw_control(ui.painter(), Pos2::ZERO, ct, false, true, 1.0, 1.0, None);
            });
            full.textures_delta.clear();
            fn walk(
                s: &egui::Shape,
                segs: &mut Vec<Color32>,
                hubs: &mut Vec<Color32>,
                bands: &mut Vec<Color32>,
            ) {
                match s {
                    egui::Shape::Vec(v) => v.iter().for_each(|s| walk(s, segs, hubs, bands)),
                    egui::Shape::LineSegment { stroke, .. } => segs.push(stroke.color),
                    egui::Shape::Circle(c) => hubs.push(c.fill),
                    egui::Shape::Path(p) => {
                        if let egui::epaint::ColorMode::Solid(c) = p.stroke.color {
                            bands.push(c);
                        }
                    }
                    _ => {}
                }
            }
            let (mut segs, mut hubs, mut bands) = (Vec::new(), Vec::new(), Vec::new());
            for cs in &full.shapes {
                walk(&cs.shape, &mut segs, &mut hubs, &mut bands);
            }
            (segs, hubs, bands)
        }

        let red = Color32::from_rgb(0xD9, 0x25, 0x25);
        let green = Color32::from_rgb(0x16, 0xA3, 0x4A);

        // Both styles that draw a needle answer to the property.
        for style in ["Radial", "Donut"] {
            let mut g = Control::new("G", CT::Gauge, 0, 0);
            g.rect = crate::model::Rect::new(0, 0, 200, 140);
            g.set_prop("GaugeStyle", PropValue::String(style.into()));
            g.set_prop("Value", PropValue::Int(70));
            g.set_prop("Color", PropValue::String("#16A34AFF".into()));
            // Scale marks are drawn as line segments too, in the TRACK colour —
            // off, so the only segments left belong to the needle.
            g.set_prop("ShowScale", PropValue::Bool(false));

            let (segs, hubs, _) = ink(&g);
            assert!(
                segs.contains(&green) && hubs.contains(&green),
                "{style}: a blank NeedleColor must keep the meter's #16A34A — \
                 segments {segs:?}, circles {hubs:?}"
            );

            g.set_prop("NeedleColor", PropValue::String("#D92525FF".into()));
            let (segs, hubs, bands) = ink(&g);
            assert!(
                segs.contains(&red) && hubs.contains(&red),
                "{style}: NeedleColor must paint the needle and its hub red — \
                 segments {segs:?}, circles {hubs:?}"
            );
            assert!(
                !segs.contains(&green) && !hubs.contains(&green),
                "{style}: nothing of the needle may stay on the meter's colour — \
                 segments {segs:?}, circles {hubs:?}"
            );
            assert!(
                bands.contains(&green) && !bands.contains(&red),
                "{style}: the meter's own band keeps #16A34A — the needle's \
                 colour is the needle's alone, got {bands:?}"
            );
        }

        println!(
            "\n  Gauge — NeedleColor #D92525 paints needle + hub in Radial and \
             Donut; the band stays on the meter's #16A34A; blank inherits it\n"
        );
    }

    /// `ReadoutPosition` moves a Radial's value+unit from inside the dial to
    /// under the needle's pivot, 5 px clear of it (operator, 2026-08-16). The
    /// dial gives up that much room, so the number never lands off the control.
    #[test]
    fn a_radial_gauge_can_print_its_reading_below_the_needle() {
        use crate::model::PropValue;

        /// `(text rect, hub centre)` for the gauge as painted.
        fn laid_out(ct: &Control) -> (Rect, Pos2) {
            let ctx = egui::Context::default();
            set_surface_theme(&ctx, eleg());
            let mut input = egui::RawInput::default();
            input.screen_rect = Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(600.0, 400.0)));
            let mut full = ctx.run_ui(input, |ui| {
                draw_control(ui.painter(), Pos2::ZERO, ct, false, true, 1.0, 1.0, None);
            });
            full.textures_delta.clear();
            // The galley's LAYOUT box, not its ink: `visual_bounding_rect` starts
            // at the first glyph's ink, a few px below the line's top, which is
            // not where the painter was told to put the text.
            fn walk(s: &egui::Shape, text: &mut Option<Rect>, hub: &mut Option<Pos2>) {
                match s {
                    egui::Shape::Vec(v) => v.iter().for_each(|s| walk(s, text, hub)),
                    egui::Shape::Text(t) => {
                        *text = Some(Rect::from_min_size(t.pos, t.galley.size()))
                    }
                    // The hub is the only filled circle a Radial draws.
                    egui::Shape::Circle(c) => *hub = Some(c.center),
                    _ => {}
                }
            }
            let (mut text, mut hub) = (None, None);
            for cs in &full.shapes {
                walk(&cs.shape, &mut text, &mut hub);
            }
            (
                text.expect("the gauge reads out"),
                hub.expect("the needle's hub"),
            )
        }

        let mut up = Control::new("G", CT::Gauge, 0, 0);
        up.rect = crate::model::Rect::new(0, 0, 240, 150);
        up.set_prop("Value", PropValue::Int(23));
        up.set_prop("Unit", PropValue::String("Parts".into()));
        let bottom = up.rect.y as f32 + up.rect.h as f32;

        // Up — the default, unchanged: inside the dial, above the pivot.
        let (text_up, hub_up) = laid_out(&up);
        assert!(
            text_up.bottom() < hub_up.y,
            "Up must keep the reading above the hub: text {text_up:?}, hub {hub_up:?}"
        );

        let mut down = up.clone();
        down.set_prop("ReadoutPosition", PropValue::String("Down".into()));
        let (text_down, hub_down) = laid_out(&down);
        assert!(
            (text_down.top() - (hub_down.y + 5.0)).abs() < 1.0,
            "Down must put the reading 5 px below the hub: text top {}, hub y {}",
            text_down.top(),
            hub_down.y
        );
        assert!(
            text_down.bottom() <= bottom,
            "the reading must stay inside the control: text bottom {} vs {bottom}",
            text_down.bottom()
        );
        assert!(
            hub_down.y < hub_up.y,
            "the dial rises to make the room: hub {} vs {}",
            hub_down.y,
            hub_up.y
        );

        // Radial only: a Donut reads out in its hole and a Linear under its bar,
        // and neither is moved by the property.
        for style in ["Donut", "Linear"] {
            let mut a = up.clone();
            a.set_prop("GaugeStyle", PropValue::String(style.into()));
            let mut b = a.clone();
            b.set_prop("ReadoutPosition", PropValue::String("Down".into()));
            let ctx = egui::Context::default();
            set_surface_theme(&ctx, eleg());
            let place = |c: &Control| -> Rect {
                let mut input = egui::RawInput::default();
                input.screen_rect = Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(600.0, 400.0)));
                let mut full = ctx.run_ui(input, |ui| {
                    draw_control(ui.painter(), Pos2::ZERO, c, false, true, 1.0, 1.0, None);
                });
                full.textures_delta.clear();
                fn walk(s: &egui::Shape, text: &mut Option<Rect>) {
                    match s {
                        egui::Shape::Vec(v) => v.iter().for_each(|s| walk(s, text)),
                        egui::Shape::Text(t) => {
                            *text = Some(Rect::from_min_size(t.pos, t.galley.size()))
                        }
                        _ => {}
                    }
                }
                let mut text = None;
                for cs in &full.shapes {
                    walk(&cs.shape, &mut text);
                }
                text.expect("the gauge reads out")
            };
            assert_eq!(
                place(&a),
                place(&b),
                "{style} has one place for its reading — ReadoutPosition must not move it"
            );
        }

        println!(
            "\n  Gauge — ReadoutPosition: Up leaves \"23 Parts\" at y {:.0} (above the \
             hub at {:.0}); Down puts it at y {:.0}, 5 px under the hub at {:.0}, \
             inside the control's {bottom:.0}; Donut and Linear unmoved\n",
            text_up.bottom(),
            hub_up.y,
            text_down.top(),
            hub_down.y
        );
    }

    /// What the Gauge reads out: `Unit` is appended to the value, and `Text`
    /// replaces the whole reading.
    #[test]
    fn gauge_readout_appends_unit_and_yields_to_a_text_override() {
        use crate::model::PropValue;

        fn reading(ct: &Control) -> String {
            let ctx = egui::Context::default();
            set_surface_theme(&ctx, eleg());
            let mut input = egui::RawInput::default();
            input.screen_rect = Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(600.0, 400.0)));
            let mut full = ctx.run_ui(input, |ui| {
                draw_control(ui.painter(), Pos2::ZERO, ct, false, true, 1.0, 1.0, None);
            });
            full.textures_delta.clear();
            fn walk(s: &egui::Shape, out: &mut Vec<String>) {
                match s {
                    egui::Shape::Vec(v) => v.iter().for_each(|s| walk(s, out)),
                    egui::Shape::Text(t) => out.push(t.galley.text().to_owned()),
                    _ => {}
                }
            }
            let mut out = Vec::new();
            for cs in &full.shapes {
                walk(&cs.shape, &mut out);
            }
            out.join("")
        }

        let mut g = Control::new("G", CT::Gauge, 0, 0);
        g.rect = crate::model::Rect::new(0, 0, 200, 120);
        g.set_prop("Value", PropValue::Int(42));
        assert_eq!(reading(&g), "42", "a bare Gauge reads out its value");

        g.set_prop("Unit", PropValue::String("%".into()));
        assert_eq!(reading(&g), "42%", "a symbol unit stays welded to the number");

        // …in every style, not just the two that once had an API for it.
        for style in ["Linear", "Donut"] {
            let mut styled = g.clone();
            styled.set_prop("GaugeStyle", PropValue::String(style.into()));
            assert_eq!(reading(&styled), "42%", "{style} must carry the Unit too");
        }

        // Reported (operator, 2026-08-16): `Unit` "Parts" read out as "42Parts".
        // "Appended exactly as typed" put the burden of the space on the
        // developer, and a word unit needs one every time.
        for (unit, want) in [
            ("Parts", "42 Parts"),
            ("rpm", "42 rpm"),
            ("3s", "42 3s"),
            ("%", "42%"),
            ("°C", "42°C"),
            ("$", "42$"),
            (" rpm", "42 rpm"),
            ("  rpm", "42  rpm"),
            ("", "42"),
        ] {
            let mut u = g.clone();
            u.set_prop("Unit", PropValue::String(unit.into()));
            assert_eq!(
                reading(&u),
                want,
                "Unit {unit:?} must read out as {want:?}"
            );
        }

        g.set_prop("Text", PropValue::String("OFFLINE".into()));
        assert_eq!(reading(&g), "OFFLINE", "Text replaces the whole reading");

        println!(
            "\n  Gauge readout — 42 ⇒ \"42\", \"%\" ⇒ \"42%\" in all 3 styles, \
             word units spaced (\"Parts\" ⇒ \"42 Parts\", \"rpm\" ⇒ \"42 rpm\"), \
             symbols tight (\"°C\" ⇒ \"42°C\", \"$\" ⇒ \"42$\"), \
             a typed space preserved (\" rpm\" ⇒ \"42 rpm\"), +Text ⇒ \"OFFLINE\"\n"
        );
    }

    /// A caption never escapes its control: the font shrinks to fit, the way
    /// the TextBox always has.
    ///
    /// Buttons, Labels and every other caption-bearing control fell to a branch
    /// that laid the text out at the requested size and centred it — no fitting
    /// at all — so an oversized caption spilled straight over the border.
    #[test]
    fn a_caption_shrinks_to_fit_instead_of_escaping_its_control() {
        use crate::model::PropValue;

        /// The widest text the control painted, and the control's own rect.
        fn text_extent(ct: &Control) -> (f32, f32, Rect) {
            let ctx = egui::Context::default();
            let mut input = egui::RawInput::default();
            input.screen_rect = Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(800.0, 400.0)));
            let mut full = ctx.run_ui(input, |ui| {
                draw_control(ui.painter(), Pos2::ZERO, ct, false, true, 1.0, 1.0, None);
            });
            full.textures_delta.clear();
            fn walk(s: &egui::Shape, out: &mut Vec<Rect>) {
                match s {
                    egui::Shape::Vec(v) => v.iter().for_each(|s| walk(s, out)),
                    egui::Shape::Text(t) => out.push(t.visual_bounding_rect()),
                    _ => {}
                }
            }
            let mut texts = Vec::new();
            for cs in &full.shapes {
                walk(&cs.shape, &mut texts);
            }
            let w = texts.iter().map(|r| r.width()).fold(0.0_f32, f32::max);
            let h = texts.iter().map(|r| r.height()).fold(0.0_f32, f32::max);
            let own = Rect::from_min_size(
                Pos2::new(ct.rect.x as f32, ct.rect.y as f32),
                Vec2::new(ct.rect.w as f32, ct.rect.h as f32),
            );
            (w, h, own)
        }

        // A caption far too big for its button — the reported case.
        for (name, kind) in [
            ("Button", CT::Button),
            ("Label", CT::Label),
            ("RadioButton", CT::RadioButton),
        ] {
            let mut c = Control::new("C", kind, 0, 0);
            c.rect = crate::model::Rect::new(0, 0, 90, 30);
            c.set_prop("Caption", PropValue::String("Button-1".into()));
            c.set_prop("FontSize", PropValue::Int(48));
            let (w, h, own) = text_extent(&c);
            assert!(w > 0.0, "{name}: nothing was drawn");
            assert!(
                w <= own.width() + 0.5,
                "{name}: caption {w:.0}pt wide escaped a {:.0}pt control",
                own.width()
            );
            assert!(
                h <= own.height() + 0.5,
                "{name}: caption {h:.0}pt tall escaped a {:.0}pt control",
                own.height()
            );
        }

        // A caption that already fits is NOT shrunk — fitting must not become a
        // silent restyle of every form that was fine.
        let mut roomy = Control::new("C", CT::Button, 0, 0);
        roomy.rect = crate::model::Rect::new(0, 0, 300, 80);
        roomy.set_prop("Caption", PropValue::String("OK".into()));
        roomy.set_prop("FontSize", PropValue::Int(14));
        let (small_w, _, _) = text_extent(&roomy);
        let mut same = roomy.clone();
        same.rect = crate::model::Rect::new(0, 0, 600, 160);
        let (bigger_box_w, _, _) = text_extent(&same);
        assert!(
            (small_w - bigger_box_w).abs() < 0.5,
            "a caption that fits must render identically whatever room is spare: \
             {small_w:.1} vs {bigger_box_w:.1}"
        );

        println!(
            "\n  captions — 'Button-1' at 48pt in a 90x30 control shrinks inside \
             the border on Button, Label and RadioButton; one that already fits \
             is untouched\n"
        );
    }

    /// 050 R10 — the theme's corner radius actually REACHES a control.
    ///
    /// `radius()` existed and was consulted by nothing but a test: every painter
    /// went through the ctx-free `corner_radius`, which cannot ask a theme
    /// anything. So the accessor was wired and the value still never applied —
    /// the dead-field problem one layer up. `themed_corner_radius` is the
    /// paint-time wrapper that closes it.
    #[test]
    fn the_themes_corner_radius_reaches_the_control() {
        use crate::model::PropValue;
        let ctx = egui::Context::default();

        let mut c = Control::new("P", CT::Panel, 0, 0);
        c.rect = crate::model::Rect::new(0, 0, 200, 100);

        // Liquid Glass says nothing ⇒ the built-in per-kind default stands.
        set_surface_theme(&ctx, glass());
        assert_eq!(
            themed_corner_radius(&ctx, &c),
            corner_radius(&c),
            "an unthemed control keeps the radius it always had (R21)"
        );

        // Elegance says 5 ⇒ 5, for a container and for an ordinary control.
        set_surface_theme(&ctx, eleg());
        assert_eq!(themed_corner_radius(&ctx, &c), 5.0, "container");
        let mut b = Control::new("B", CT::Button, 0, 0);
        b.rect = crate::model::Rect::new(0, 0, 90, 28);
        assert_eq!(
            themed_corner_radius(&ctx, &b),
            5.0,
            "ordinary control — and NOT the Button's built-in 3"
        );
        assert_eq!(corner_radius(&b), 3.0, "…which is what it would have been");

        // The developer's own value always wins (R9).
        b.set_prop("CornerRadius", PropValue::Int(12));
        assert_eq!(
            themed_corner_radius(&ctx, &b),
            12.0,
            "an explicit CornerRadius outranks the theme"
        );

        // And it is still clamped to half the short side, so a tiny control
        // cannot be rounded into a blob.
        let mut tiny = Control::new("T", CT::Panel, 0, 0);
        tiny.rect = crate::model::Rect::new(0, 0, 6, 6);
        assert_eq!(themed_corner_radius(&ctx, &tiny), 3.0, "clamped to w/2");

        println!(
            "\n  050 R10 — Elegance radius 5 applied: Panel 5, Button 5 \
             (built-in 3), explicit 12 wins, 6px control clamped to 3\n"
        );
    }

    /// 050 AC4 — `GlassStyle` is read for painting through **one** gate.
    ///
    /// Crude on purpose: it scans the source. It is also the only thing that
    /// stops the next painter reintroducing exactly this defect, which a
    /// behavioural test cannot do because the ungated read would be somewhere
    /// nobody thought to look.
    #[test]
    fn glass_style_is_read_through_one_gate() {
        const FILES: [(&str, &str); 3] = [
            ("paint.rs", include_str!("paint.rs")),
            ("render.rs", include_str!("render.rs")),
            ("sidebar.rs", include_str!("sidebar.rs")),
        ];
        /// The `fn …` line enclosing `idx`, and the lines from it to the read.
        fn enclosing_fn(lines: &[&str], idx: usize) -> (String, Vec<String>) {
            let start = (0..=idx)
                .rev()
                .find(|&i| {
                    let t = lines[i].trim_start();
                    (t.starts_with("fn ")
                        || t.starts_with("pub fn ")
                        || t.starts_with("pub(crate) fn "))
                        && lines[i].starts_with(|c: char| c == 'f' || c == 'p')
                })
                .unwrap_or(0);
            (
                lines[start].trim().to_owned(),
                lines[start..=idx].iter().map(|s| s.to_string()).collect(),
            )
        }

        let mut rows: Vec<(String, usize, String, bool)> = Vec::new();
        for (name, src) in FILES {
            let lines: Vec<&str> = src.lines().collect();
            // The scanner's own source is data, not a painting read.
            let scanner_start = lines
                .iter()
                .position(|l| l.contains("fn glass_style_is_read_through_one_gate"))
                .unwrap_or(lines.len());
            for (i, line) in lines.iter().enumerate() {
                if !line.contains("active_glass_style(") || i >= scanner_start {
                    continue;
                }
                let trimmed = line.trim();
                if trimmed.starts_with("//") || trimmed.starts_with("///") {
                    continue; // prose, not a read
                }
                let (sig, body) = enclosing_fn(&lines, i);
                // Its own definition is not a read.
                let is_decl = sig.contains("fn active_glass_style");
                // Legitimate when the gate guards it anywhere between the
                // function's start and the read…
                let gated = body.iter().any(|l| l.contains("glass_config_applies("));
                // …or when the read sits INSIDE a glass painter, which a
                // self-contained theme never reaches: the theme answered the
                // surface at the seam, and only its `None` arm calls these.
                let inside_glass_painter = sig.contains("fn draw_glass_");
                let ok = is_decl || gated || inside_glass_painter;
                rows.push((name.to_owned(), i + 1, trimmed.to_owned(), ok));
            }
        }

        println!("\n  050 AC4 — every painting read of the glass style:");
        for (file, line, text, ok) in &rows {
            let mark = if *ok { "✓" } else { "✗" };
            let short: String = text.chars().take(72).collect();
            println!("    {mark} {file}:{line}  {short}");
        }
        let ungated: Vec<String> = rows
            .iter()
            .filter(|(_, _, _, ok)| !ok)
            .map(|(f, l, _, _)| format!("{f}:{l}"))
            .collect();
        println!(
            "  → {}/{} pass through the gate\n",
            rows.len() - ungated.len(),
            rows.len()
        );
        assert!(
            ungated.is_empty(),
            "these read the glass style for painting without the gate — a \
             self-contained theme would be configured by it: {ungated:?}"
        );
    }

    /// T7/AC6 — the developer's own colour outranks the theme.
    #[test]
    fn elegance_honours_an_explicit_background_colour() {
        use crate::model::PropValue;

        fn face_colors(bg: Option<&str>) -> Vec<Color32> {
            let ctx = egui::Context::default();
            set_surface_theme(&ctx, eleg());
            let mut c = Control::new("P", CT::Panel, 0, 0);
            c.rect = crate::model::Rect::new(10, 10, 120, 80);
            if let Some(bg) = bg {
                c.set_prop("BackgroundColor", PropValue::String(bg.into()));
            }
            let mut input = egui::RawInput::default();
            input.screen_rect = Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(300.0, 200.0)));
            let mut full = ctx.run_ui(input, |root_ui| {
                egui::CentralPanel::default()
                    .frame(egui::Frame::NONE)
                    .show_inside(root_ui, |ui| {
                        draw_control(ui.painter(), Pos2::ZERO, &c, false, true, 1.0, 1.0, None);
                    });
            });
            full.textures_delta.clear();
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

        // A vivid colour no palette entry could coincidentally equal.
        let themed = face_colors(None);
        let custom = face_colors(Some("#FF00FFFF"));
        let magenta = Color32::from_rgb(0xFF, 0x00, 0xFF);
        println!(
            "themed panel fills: {:?}\n  custom-bg panel fills: {:?}",
            themed, custom
        );
        assert!(
            custom.iter().any(|c| c.r() > 200 && c.b() > 200 && c.g() < 60),
            "an explicit BackgroundColor must reach the face, got {custom:?}"
        );
        assert!(
            !themed.contains(&magenta),
            "the themed default must not be the custom colour"
        );
    }

    /// T8 — chart data marks take the Elegance palette, and only under Elegance.
    #[test]
    fn elegance_charts_use_the_theme_palette_for_data_marks() {
        use crate::model::PropValue;

        fn chart_fills(style: Arc<dyn crate::surface_theme::SurfaceTheme>) -> Vec<Color32> {
            let ctx = egui::Context::default();
            set_surface_theme(&ctx, style);
            let mut c = Control::new("CH", CT::BarChart, 0, 0);
            c.rect = crate::model::Rect::new(10, 10, 260, 160);
            c.set_prop("Series", PropValue::String("4,9,2,7".into()));
            let mut input = egui::RawInput::default();
            input.screen_rect = Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(400.0, 300.0)));
            let mut full = ctx.run_ui(input, |root_ui| {
                egui::CentralPanel::default()
                    .frame(egui::Frame::NONE)
                    .show_inside(root_ui, |ui| {
                        draw_control(ui.painter(), Pos2::ZERO, &c, false, true, 1.0, 1.0, None);
                    });
            });
            full.textures_delta.clear();
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

        // The theme's FIRST data mark — whatever a theme puts there.
        let first_mark = eleg().data_marks().expect("Elegance supplies data marks")[0];
        let themed = chart_fills(eleg());
        let plain = chart_fills(glass());
        let has = |v: &[Color32], c: Color32| {
            v.iter().any(|x| x.r() == c.r() && x.g() == c.g() && x.b() == c.b())
        };
        println!(
            "chart bars — themed first mark present: {}, under Liquid Glass: {}",
            has(&themed, first_mark),
            has(&plain, first_mark)
        );
        assert!(
            has(&themed, first_mark),
            "a themed chart must draw its first series in the theme's first data mark"
        );
        assert!(
            !has(&plain, first_mark),
            "Liquid Glass charts must keep the built-in accents (R21)"
        );
    }

    /// T13/AC5/R11 — every R4 family is actually painted by Elegance.
    ///
    /// The gate on the spec's "no partial-coverage ship point": a family that
    /// silently sits on the Liquid Glass fallback fails the build here rather
    /// than shipping looking wrong. Reports which families are covered by name,
    /// not just a count.
    #[test]
    fn elegance_covers_every_r4_control_family() {
        use crate::surface_theme::{AccentName as A, ColorToken as Tok};
        let mut palette = vec![
            tok(Tok::Card),
            tok(Tok::InputBg),
            tok(Tok::CardRaised),
            tok(Tok::Focus),
            tok(Tok::Border),
            // A frameless control (Label) has no surface at all — its themed
            // face IS its glyph colour.
            tok(Tok::Text),
            tok(Tok::LabelText),
            tok(Tok::DimText),
            // 050 — the toggles and the slider's own colours.
            tok(Tok::SliderFill),
            tok(Tok::SliderKnob),
            crate::surface_theme::elegance()
                .surface(
                    crate::surface_theme::SurfaceRole::Toggle,
                    crate::surface_theme::SurfaceState { selected: false, on: true },
                )
                .and_then(|s| s.fill)
                .expect("a toggle has an on colour"),
        ];
        for a in [A::Blue, A::Amber, A::Green, A::Purple, A::Red, A::Sky] {
            palette.push(tok(Tok::Accent(a)));
        }
        // The data marks are part of the theme's surface area too.
        palette.extend(eleg().data_marks().unwrap_or_default());
        let same = |a: Color32, b: Color32| a.r() == b.r() && a.g() == b.g() && a.b() == b.b();

        fn fills_for(ct: &ControlType) -> Vec<Color32> {
            let ctx = egui::Context::default();
            set_surface_theme(&ctx, eleg());
            let mut c = Control::new("C", ct.clone(), 0, 0);
            c.rect = crate::model::Rect::new(10, 10, 200, 110);
            let mut input = egui::RawInput::default();
            input.screen_rect = Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(400.0, 300.0)));
            let mut full = ctx.run_ui(input, |root_ui| {
                egui::CentralPanel::default()
                    .frame(egui::Frame::NONE)
                    .show_inside(root_ui, |ui| {
                        draw_control(ui.painter(), Pos2::ZERO, &c, false, true, 1.0, 1.0, None);
                    });
            });
            full.textures_delta.clear();
            // Elegance shows up as a fill on framed controls, but as a STROKE on
            // the ring/dial controls and as glyph colour on a bare label — so
            // every channel a colour can arrive through is collected.
            fn collect(s: &egui::Shape, out: &mut Vec<Color32>) {
                match s {
                    egui::Shape::Vec(v) => v.iter().for_each(|s| collect(s, out)),
                    egui::Shape::Rect(r) => {
                        out.push(r.fill);
                        out.push(r.stroke.color);
                    }
                    egui::Shape::Circle(c) => {
                        out.push(c.fill);
                        out.push(c.stroke.color);
                    }
                    egui::Shape::Path(p) => {
                        out.push(p.fill);
                        if let egui::epaint::ColorMode::Solid(c) = p.stroke.color {
                            out.push(c);
                        }
                    }
                    egui::Shape::LineSegment { stroke, .. } => out.push(stroke.color),
                    egui::Shape::Text(t) => {
                        out.push(t.override_text_color.unwrap_or(t.fallback_color));
                    }
                    _ => {}
                }
            }
            let mut out = Vec::new();
            for cs in &full.shapes {
                collect(&cs.shape, &mut out);
            }
            out
        }

        let mut covered: Vec<&str> = Vec::new();
        let mut missing: Vec<&str> = Vec::new();
        for (name, ct) in r4_families() {
            let fills = fills_for(&ct);
            // The palette border is on every Elegance face, so a themed control
            // always carries at least one palette colour.
            let hit = fills.iter().any(|f| palette.iter().any(|p| same(*f, *p)));
            if hit {
                covered.push(name);
            } else {
                // Say WHAT it painted instead — a bare "uncovered" sends the
                // next reader hunting through the painter by hand.
                let mut seen: Vec<String> = fills
                    .iter()
                    .filter(|c| c.a() > 0)
                    .map(|c| format!("#{:02x}{:02x}{:02x}", c.r(), c.g(), c.b()))
                    .collect();
                seen.sort();
                seen.dedup();
                println!("    {name} painted instead: {seen:?}");
                missing.push(name);
            }
        }

        println!(
            "\n  Elegance coverage: {}/{} R4 control families",
            covered.len(),
            covered.len() + missing.len()
        );
        println!("    covered: {}", covered.join(", "));
        if !missing.is_empty() {
            println!("    NOT covered: {}", missing.join(", "));
        }
        println!();

        assert!(
            missing.is_empty(),
            "spec 047 R11 forbids shipping with a family on the Liquid Glass \
             fallback — uncovered: {missing:?}"
        );
    }

    /// T13/R7 — the fallback still degrades gracefully.
    ///
    /// R11 says no *R4* family may rely on it, not that it should not exist:
    /// a control kind with no Elegance mapping must still render rather than
    /// fail. Timer is non-visual and deliberately outside R4's list.
    #[test]
    fn elegance_falls_back_without_failing_for_an_unmapped_kind() {
        let ctx = egui::Context::default();
        set_surface_theme(&ctx, eleg());
        let mut c = Control::new("T", CT::Timer, 0, 0);
        c.rect = crate::model::Rect::new(10, 10, 60, 60);
        let mut input = egui::RawInput::default();
        input.screen_rect = Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(200.0, 200.0)));
        // The assertion is that this paints at all rather than panicking.
        let mut full = ctx.run_ui(input, |root_ui| {
            egui::CentralPanel::default()
                .frame(egui::Frame::NONE)
                .show_inside(root_ui, |ui| {
                    draw_control(ui.painter(), Pos2::ZERO, &c, false, true, 1.0, 1.0, None);
                });
        });
        full.textures_delta.clear();
        println!("unmapped kind rendered {} shape group(s)", full.shapes.len());
    }

    /// T2 — the theme survives a round-trip through context storage, and the
    /// gate follows it (050 R6).
    #[test]
    fn elegance_wire_round_trips() {
        let ctx = egui::Context::default();
        assert_eq!(
            active_surface_theme(&ctx).id(),
            crate::theme::LIQUID_GLASS,
            "a context nobody published to must default to Liquid Glass"
        );

        set_surface_theme(&ctx, crate::surface_theme::elegance());
        assert_eq!(active_surface_theme(&ctx).id(), crate::theme::ELEGANCE);
        assert!(
            !glass_config_applies(&ctx),
            "a self-contained theme closes the gate"
        );

        set_surface_theme(&ctx, crate::surface_theme::liquid_glass());
        assert_eq!(active_surface_theme(&ctx).id(), crate::theme::LIQUID_GLASS);
        assert!(
            glass_config_applies(&ctx),
            "…and Liquid Glass opens it again"
        );
    }

    /// T2 — the "host forgot to publish" path keeps today's behaviour (R15).
    ///
    /// This is the load-bearing default: four separate surfaces publish per
    /// frame, and any that misses the call must render exactly as it does today
    /// rather than silently changing look. Note it is the SAME theme object, not
    /// merely an equal one — "unpublished" and "published Liquid Glass" are one
    /// code path.
    #[test]
    fn elegance_wire_defaults_to_liquid_glass_when_never_published() {
        let ctx = egui::Context::default();
        assert_eq!(active_surface_theme(&ctx).id(), crate::theme::LIQUID_GLASS);
        assert!(glass_config_applies(&ctx));
        assert!(!active_surface_theme(&ctx).is_self_contained());
    }

    /// T2 — id → theme mapping, through the registry. Only the exact Elegance
    /// id selects Elegance; pack ids and junk both fall to Liquid Glass (the
    /// correct base for a pack, which falls back to glass for the kinds it does
    /// not cover).
    #[test]
    fn elegance_wire_maps_theme_ids_to_styles() {
        use crate::surface_theme::for_theme_id;
        use crate::theme::{ELEGANCE, LIQUID_GLASS};
        let mut rows = Vec::new();
        for (id, want) in [
            (ELEGANCE, ELEGANCE),
            (" elegance ", ELEGANCE),
            (LIQUID_GLASS, LIQUID_GLASS),
            ("stainless-steel", LIQUID_GLASS),
            ("", LIQUID_GLASS),
            ("no-such-theme", LIQUID_GLASS),
        ] {
            let got = for_theme_id(id);
            assert_eq!(got.id(), want, "{id:?}");
            rows.push((id, got.id().to_owned(), got.is_self_contained()));
        }
        println!("\n  {:<18} {:<14} self-contained", "id", "→ theme");
        for (id, theme, sc) in &rows {
            println!("  {:<18} {:<14} {sc}", format!("{id:?}"), theme);
        }
    }
}

#[cfg(test)]
mod elegance_baseline_tests {
    use super::elegance_tests::*;
    use super::*;
    use crate::model::GlassStyle as GS;

    /// T3/AC10 — the regression baseline.
    ///
    /// Paints the full R4 fixture under Liquid Glass and under an asset pack,
    /// across all four glass styles, and reports the tessellated shape-leaf
    /// count for each. These eight numbers are the contract the R13 seam
    /// refactor (T4) must not move.
    #[test]
    fn elegance_baseline_reports_untouched_paths() {
        let fixture = r4_fixture();
        let pack = fixture_pack();

        let mut rows: Vec<(String, GS, usize)> = Vec::new();
        for gs in ALL_GLASS_STYLES {
            rows.push((
                "liquid-glass".to_owned(),
                gs,
                painted_leaf_count(&fixture, gs, None, glass()),
            ));
            rows.push((
                "asset-pack".to_owned(),
                gs,
                painted_leaf_count(&fixture, gs, Some(pack.clone()), glass()),
            ));
        }

        println!(
            "\n  R4 fixture: {} controls\n  {:<14} {:<16} {}",
            fixture.len(),
            "theme",
            "glass style",
            "shape leaves"
        );
        for (theme, gs, n) in &rows {
            println!("  {theme:<14} {:<16} {n}", format!("{gs:?}"));
        }
        println!();

        for (theme, gs, n) in &rows {
            assert!(
                *n > 0,
                "{theme} / {gs:?} painted nothing — the fixture is not exercising the painter"
            );
        }

        // The asset-pack row must actually differ from the glass row, or this
        // baseline would be silently testing the same path twice (an
        // undecodable image falls back to glass — see R11).
        let glass_classic = rows
            .iter()
            .find(|(t, gs, _)| t == "liquid-glass" && *gs == GS::Classic)
            .map(|(_, _, n)| *n)
            .unwrap();
        let pack_classic = rows
            .iter()
            .find(|(t, gs, _)| t == "asset-pack" && *gs == GS::Classic)
            .map(|(_, _, n)| *n)
            .unwrap();
        assert_ne!(
            glass_classic, pack_classic,
            "the pack fixture is not skinning anything — its art failed to decode"
        );

        // ── Golden values (spec 047 T3/AC10) ──────────────────────────────
        //
        // Captured before the R13 seam refactor. The seam rewires seven
        // sub-element paint sites shared with Liquid Glass and asset packs; if
        // any of these eight numbers moves, that refactor changed what gets
        // drawn and AC8/AC10 are broken.
        //
        // A deliberate change to Liquid Glass's own painting may legitimately
        // move them — re-bless only with that intent, never to get green.
        //
        // Re-blessed once, in 1.61.43: a RadioButton now honours its own
        // `BorderStyle` (seeded `None`) and so paints no card or rim by default.
        // The fixture holds exactly one radio, and every row moved by exactly
        // what that one control stopped drawing — measured on a radio-only
        // fixture: Classic 75 → 2 (−73, matching 1430 → 1357), Enhanced 83 → 2
        // (−81, 1546 → 1465), and both Neumorphic styles 3 → 2 (−1, 484 → 483).
        // The asset-pack rows moved by the same amounts, no pack here skinning a
        // radio. Nothing else in the seam changed.
        //
        // Re-blessed again in the same release: the Knob is now drawn by the
        // shared painter (rim, face, inner ring and indicator, in place of one
        // bare arc), which is 3 leaves → 7. Every row moved by exactly +4, and a
        // per-control sweep of all 27 fixture families showed the Knob as the
        // ONLY count that moved — the Gauge's colours and the NumericUpDown's
        // caption changed what is drawn, not how many shapes it takes.
        // Re-blessed in 1.61.49: the Gauge's own switches now reach the paint.
        // A Radial Gauge with the default `ShowScale`/`ShowNeedle` draws 11
        // scale ticks, a needle and its hub — 13 leaves — and the fixture holds
        // exactly one Gauge. Every row moved by exactly +13, which is that one
        // control and nothing else.
        //
        // Re-blessed in 1.61.69: the ToolBar. By operator decision a new toolbar
        // is fully transparent with NO border, so it stops drawing the card and
        // rim it used to — and instead draws the example it now ships with, one
        // group holding one folder-open button. Both halves show in the numbers:
        // the card it gave up costs a lot in Classic and Enhanced and almost
        // nothing in Neumorphic, while the button and its icon cost the same
        // everywhere. Hence Classic −62, Enhanced −70, and Neumorphic +7 — the
        // +7 being what is left when there was no expensive card to lose.
        //
        // Both themes in each style moved by the SAME amount, which is what says
        // one control moved rather than the seam: no pack here skins a toolbar.
        // And the only painting this release touched is the ToolBar's — the
        // `CT::ToolBar` label arm and the toolbar draw block in this file, plus
        // that control's seeded properties in the model. No other control type is
        // reachable from those changes.
        //
        // Re-blessed in 1.61.97: the charts. Seven of their own properties now
        // reach the paint (dead-property audit), and two are seeded ON --
        // `ShowLegend` on every chart and `ShowLabels` on pie/donut -- so each
        // chart in the fixture draws more than it did.
        //
        // Every row moved by exactly +40, and that number is fully accounted
        // for by the six chart controls and nothing else:
        //   Bar, Line, Area, Scatter -- a legend of the 2 sample series,
        //     one swatch and one name each                     4 x 4 = +16
        //   Pie, Donut -- a legend of the 4 sample slices (8) plus a label
        //     on each slice (4)                                2 x 12 = +24
        // Identical in every style and both themes, which is what says the
        // charts moved rather than the seam: no pack here skins a chart.
        // Re-blessed in 1.61.152: the RadioButton's circle, on every theme.
        // It was drawn only where a theme described a Toggle surface, so on
        // every theme here the radio was a pair of parentheses typed into its
        // caption instead of an indicator (operator, 2026-08-22). The shape is
        // the platform's now.
        //
        // Every row moved by exactly +2 — one circle fill and one circle
        // stroke — in both themes and all four styles. The fixture holds
        // exactly one radio, and +2 is what that one control started drawing;
        // both themes moving by the SAME amount is what says one control moved
        // rather than the seam, and it also says the asset-pack theme describes
        // no Toggle either, so it gains the same circle. The caption it stopped
        // typing was never a shape of its own — it was characters inside a
        // galley that is still one leaf.
        // Re-blessed again in 1.61.153: the TreeView draws its TREE. The canvas
        // drew the caption `[TreeView]` and no nodes at all, and the running
        // form drew a flat bulleted list; both now go through one renderer that
        // also honours `ShowLines`/`ShowRootLines` (operator, 2026-08-22).
        //
        // Every row moved by exactly +6, and the fixture's one TreeView (seeded
        // `Node 1 / Child 1 / Child 2 / Node 2`) accounts for all of it:
        //   was  1 placeholder caption + 4 bulleted labels            =  5
        //   now  4 labels
        //        + 2 children x (elbow + vertical)                    = +4
        //        + the root spine: 1 vertical + 2 elbows              = +3
        //                                                              = 11
        // 11 − 5 = +6, in both themes and all four styles — one control moved,
        // not the seam.
        let expected: [(&str, GS, usize); 8] = [
            ("liquid-glass", GS::Classic, 1360),
            ("asset-pack", GS::Classic, 1182),
            ("liquid-glass", GS::Enhanced, 1460),
            ("asset-pack", GS::Enhanced, 1256),
            ("liquid-glass", GS::Neumorphic, 555),
            ("asset-pack", GS::Neumorphic, 571),
            ("liquid-glass", GS::NeumorphicDark, 555),
            ("asset-pack", GS::NeumorphicDark, 571),
        ];
        for (theme, gs, want) in expected {
            let got = rows
                .iter()
                .find(|(t, g, _)| t == theme && *g == gs)
                .map(|(_, _, n)| *n)
                .unwrap_or_else(|| panic!("missing baseline row {theme}/{gs:?}"));
            assert_eq!(
                got, want,
                "baseline moved for {theme} / {gs:?}: expected {want}, got {got} \
                 — the seam refactor must not change what is painted"
            );
        }
    }
}
