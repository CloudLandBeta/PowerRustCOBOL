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

use std::f32::consts::TAU;
use std::sync::{Arc, Mutex};
use std::collections::HashMap;
use egui::{Color32, Pos2, Rect, Stroke, Vec2};
use crate::{Control, ControlType};
use crate::model::PropValue;
use crate::theme_pack::{ThemePack, ControlState, Slice};

// ── Public API (the designer-derived appearance) ─────────────────────────────

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
        Rect::from_center_size(Pos2::new(thumb_x, cy), Vec2::new(thumb_w_half * 2.0, thumb_h))
    }
}

/// Snapshot live/runtime string props into a transient Control so that
/// draw_control can be used for exact designed appearance (WYSIWYG).
/// Moved fully here (per plan) so both IDE runtime and compiler binary can use it.
pub fn live_control<'a>(
    id:    &str,
    ct:    ControlType,
    size:  Vec2,
    props: impl IntoIterator<Item = (&'a String, &'a String)>,
) -> Control {
    let mut c = Control::new(id, ct, 0, 0);
    c.rect = crate::model::Rect::new(
        0, 0, size.x.round() as i32, size.y.round() as i32);
    for (k, v) in props {
        c.properties.insert(
            k.clone(), PropValue::String(v.clone()));
    }
    c
}

/// Map alignment string (used by labels).
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
    painter:   &egui::Painter,
    center:    Pos2,
    radius:    f32,
    base:      Color32,
    selected:  bool,
    alpha_mul: f32,
) {
    if alpha_mul <= 0.0 { return; }
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
    let radial_fan = |origin: Pos2, rad: f32, cc: Color32, ce: Color32|
        -> egui::epaint::Mesh
    {
        let uv = egui::pos2(0.0, 0.0);
        let n  = 48_u32;
        let mut m = egui::epaint::Mesh::default();
        m.vertices.push(egui::epaint::Vertex { pos: origin, uv, color: cc });
        for i in 0..n {
            let a = i as f32 / n as f32 * std::f32::consts::TAU;
            m.vertices.push(egui::epaint::Vertex {
                pos: origin + Vec2::new(a.cos(), a.sin()) * rad,
                uv, color: ce,
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
    let t  = 0.20_f32 * am;
    let fr = ((200.0 * 0.85 + base.r() as f32 * 0.15) * t) as u8;
    let fg = ((210.0 * 0.85 + base.g() as f32 * 0.15) * t) as u8;
    let fb = ((220.0 * 0.85 + base.b() as f32 * 0.15) * t) as u8;
    let fa = (255.0 * t) as u8;
    painter.circle_filled(center, radius,
        Color32::from_rgba_premultiplied(fr, fg, fb, fa));

    // ── 3. Top-arc highlight ──────────────────────────────────────────────────
    // Subtle brightening in the upper third — centre at -30 % of radius.
    let top_c = center + Vec2::new(0.0, -radius * 0.30);
    painter.add(egui::Shape::mesh(radial_fan(
        top_c, radius * 0.65,
        white(52),   // centre: soft white
        white(0),    // edge:   fully transparent
    )));

    // ── 4. Bottom crescent reflection ─────────────────────────────────────────
    // The defining glass-disc feature: a smooth bright oval near the bottom,
    // like light reflecting off the curved lower surface.
    let bot_c = center + Vec2::new(0.0, radius * 0.62);
    painter.add(egui::Shape::mesh(radial_fan(
        bot_c, radius * 0.50,
        white(100),  // centre: bright reflection
        white(0),    // edge:   fades to transparent
    )));

    // ── 5. Rim ────────────────────────────────────────────────────────────────
    let (border_w, border_c) = if selected {
        (2.0, Color32::from_rgba_premultiplied(
            (140.0 * am) as u8,
            (190.0 * am) as u8,
            (255.0 * am) as u8,
            (255.0 * am) as u8,
        ))
    } else {
        (1.5, white(150))
    };
    painter.circle_stroke(center, radius, Stroke::new(border_w, border_c));
}

pub fn draw_glass(
    painter:   &egui::Painter,
    rect:      egui::Rect,
    base:      Color32,   // control's own colour — used only as a faint frost tint
    corner:    f32,
    selected:  bool,
    alpha_mul: f32,
) {
    if alpha_mul <= 0.0 { return; }
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
    let radius = corner
        .max(0.0)
        .min(w * 0.5)
        .min(h * 0.5);

    // Build a rounded-rectangle mesh from horizontal strips.  This preserves a
    // true top-to-bottom gradient while following the exact rounded contour on
    // the left and right sides.  Unlike a centre-fan mesh, it does not create
    // side bands or corner warping inside the chart frame.
    let rounded_vertical_mesh = |area: egui::Rect,
                                 r: f32,
                                 rows: usize,
                                 color_at_t: &dyn Fn(f32) -> Color32|
        -> egui::epaint::Mesh
    {
        let uv = egui::pos2(0.0, 0.0);
        let mut m = egui::epaint::Mesh::default();
        let rr = r
            .max(0.0)
            .min(area.width() * 0.5)
            .min(area.height() * 0.5);

        let inset_at_y = |y: f32| -> f32 {
            if rr <= 0.0 { return 0.0; }
            let mut inset: f32 = 0.0;

            let top = (y - area.min.y).clamp(0.0, area.height());
            if top < rr {
                let dy = rr - top;
                inset = inset.max(rr - (rr * rr - dy * dy).max(0.0).sqrt());
            }

            let bottom = (area.max.y - y).clamp(0.0, area.height());
            if bottom < rr {
                let dy = rr - bottom;
                inset = inset.max(rr - (rr * rr - dy * dy).max(0.0).sqrt());
            }

            inset
        };

        let n = rows.max(32);
        for i in 0..=n {
            let t = i as f32 / n as f32;
            let y = area.min.y + area.height() * t;
            let inset = inset_at_y(y);
            let c = color_at_t(t);
            m.vertices.push(egui::epaint::Vertex {
                pos: Pos2::new(area.min.x + inset, y),
                uv,
                color: c,
            });
            m.vertices.push(egui::epaint::Vertex {
                pos: Pos2::new(area.max.x - inset, y),
                uv,
                color: c,
            });
        }

        for i in 0..n {
            let k = (i * 2) as u32;
            m.indices.extend([k, k + 1, k + 3, k, k + 3, k + 2]);
        }

        m
    };

    // ── 1. Layered shadow ────────────────────────────────────────────────────
    painter.rect_filled(
        rect.translate(Vec2::new(0.0, 8.0)).expand(1.0),
        radius + 4.0,
        pm(0, 0, 0, 18),
    );
    painter.rect_filled(
        rect.translate(Vec2::new(0.0, 16.0)).expand(4.0),
        radius + 10.0,
        pm(0, 0, 0, 8),
    );

    // ── 2. Continuous frosted field ───────────────────────────────────────────
    let glass_color = |t: f32| -> Color32 {
        let u = t.clamp(0.0, 1.0);
        let smooth = u * u * (3.0 - 2.0 * u);
        let alpha  = 30.0 + 82.0 * (1.0 - smooth).powf(1.18);
        let lip    = 10.0 * (1.0 - u).powf(5.2);

        let mix_base = 0.035;
        let r = 255.0 * (1.0 - mix_base) + base.r() as f32 * mix_base;
        let g = 255.0 * (1.0 - mix_base) + base.g() as f32 * mix_base;
        let b = 255.0 * (1.0 - mix_base) + base.b() as f32 * mix_base;

        let a = ((alpha + lip) * am).clamp(0.0, 255.0);
        Color32::from_rgba_premultiplied(
            (r * a / 255.0).clamp(0.0, 255.0) as u8,
            (g * a / 255.0).clamp(0.0, 255.0) as u8,
            (b * a / 255.0).clamp(0.0, 255.0) as u8,
            a as u8,
        )
    };
    // The gradient meshes are NOT anti-aliased by egui, so their hard rounded
    // contour pokes a few bright pixels past the (feathered) frame stroke at
    // the four corners. Inset the fill far enough that its whole edge sits
    // under the stroke, and curve its corners in a touch MORE than the stroke's
    // radius so the corner is decisively inside it. The frame stroke (drawn
    // last, over the outer rect) seals the resulting hairline on straight edges.
    let inset       = 1.4_f32.min(radius.max(2.0));
    let fill_rect   = rect.shrink(inset);
    let fill_radius = (radius - inset + 1.0).max(0.0);
    painter.add(egui::Shape::mesh(rounded_vertical_mesh(fill_rect, fill_radius, 220, &glass_color)));

    // ── 3. Very gentle depth tint ─────────────────────────────────────────────
    let depth_color = |t: f32| -> Color32 {
        let u = t.clamp(0.0, 1.0);
        let smooth = u * u * (3.0 - 2.0 * u);
        let a = (1.0 + 13.0 * smooth.powf(1.5)).clamp(0.0, 18.0) as u8;
        pm(28, 44, 56, a)
    };
    painter.add(egui::Shape::mesh(rounded_vertical_mesh(fill_rect, fill_radius, 220, &depth_color)));

    // ── 4. Single rounded frame ───────────────────────────────────────────────
    let (border_w, border_c) = if selected {
        (2.0, Color32::from_rgba_premultiplied(
            (140.0 * am) as u8,
            (190.0 * am) as u8,
            (255.0 * am) as u8,
            (255.0 * am) as u8,
        ))
    } else {
        (1.4, white(170))
    };
    // Inset the stroke by half its width so it is fully inside `rect` (egui
    // centres strokes on the path; a centred stroke spills half-a-pixel past
    // the rect, and that overhang is exactly the bright corner fringe).
    let half = border_w * 0.5;
    painter.rect_stroke(rect.shrink(half), (radius - half).max(0.0),
        Stroke::new(border_w, border_c));
}

// ── Non-visual control rendering (standardised "liquid glass" icons) ─────────────
//
// All non-visual controls (Timer / AgentObject / RestClient / SqlDatabase) share
// one dark glass card + a consistent light, stroke-drawn ("hand-drawn") icon and
// a larger label, so they look uniform on the canvas.

/// Shared glass-card colour for every non-visual control.
const NV_CARD: Color32 = Color32::from_rgb(40, 54, 84);

/// Light "glass" colour for the stroke icons + labels.
pub fn nv_icon_color(a: u8) -> Color32 { Color32::from_rgba_premultiplied(212, 226, 255, a) }

/// Draw the shared non-visual card background.
pub fn nv_card(painter: &egui::Painter, rect: egui::Rect, selected: bool, glass: bool, alpha_mul: f32, a: u8) {
    if glass {
        draw_glass(painter, rect, NV_CARD, 12.0, selected, alpha_mul);
    } else {
        let fill   = Color32::from_rgba_premultiplied(NV_CARD.r(), NV_CARD.g(), NV_CARD.b(), a);
        let border = if selected {
            Color32::from_rgba_premultiplied(90, 160, 255, a)
        } else {
            Color32::from_rgba_premultiplied(110, 130, 180, a)
        };
        painter.rect_filled(rect, 12.0, fill);
        painter.rect_stroke(rect, 12.0, Stroke::new(if selected { 2.0 } else { 1.0 }, border));
    }
}

/// Centre / size / stroke for a non-visual icon within `rect`.
pub fn nv_icon_geom(rect: egui::Rect, a: u8) -> (Pos2, f32, Stroke) {
    let cen = Pos2::new(rect.center().x, rect.min.y + rect.height() * 0.40);
    let s   = rect.height().min(rect.width()) * 0.22;
    let sw  = (s * 0.18).clamp(1.6, 3.0);
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
    let pts: Vec<Pos2> = (0..=steps).map(|i| {
        let t = i as f32 / steps as f32 * std::f32::consts::TAU;
        Pos2::new(cx + rw * t.cos(), cy + rh * t.sin())
    }).collect();
    painter.add(egui::Shape::closed_line(pts, st));
}

pub fn nv_icon_clock(painter: &egui::Painter, c: Pos2, s: f32, st: Stroke) {
    painter.circle_stroke(c, s, st);
    // top stem (stopwatch button)
    painter.line_segment([c + Vec2::new(0.0, -s), c + Vec2::new(0.0, -s - s * 0.30)], st);
    // hands
    painter.line_segment([c, c + Vec2::new(0.0, -s * 0.6)], st);
    painter.line_segment([c, c + Vec2::new(s * 0.45, s * 0.12)], st);
}

pub fn nv_icon_robot(painter: &egui::Painter, c: Pos2, s: f32, st: Stroke) {
    let head = egui::Rect::from_center_size(c + Vec2::new(0.0, s * 0.1), Vec2::new(s * 1.7, s * 1.5));
    painter.rect_stroke(head, s * 0.28, st);
    // antenna
    painter.line_segment([Pos2::new(c.x, head.min.y), Pos2::new(c.x, head.min.y - s * 0.4)], st);
    painter.circle_filled(Pos2::new(c.x, head.min.y - s * 0.45), st.width * 1.1, st.color);
    // eyes
    painter.circle_filled(c + Vec2::new(-s * 0.42, 0.0), st.width * 1.2, st.color);
    painter.circle_filled(c + Vec2::new(s * 0.42, 0.0), st.width * 1.2, st.color);
    // mouth
    painter.line_segment([c + Vec2::new(-s * 0.4, s * 0.5), c + Vec2::new(s * 0.4, s * 0.5)], st);
}

pub fn nv_icon_globe(painter: &egui::Painter, c: Pos2, s: f32, st: Stroke) {
    painter.circle_stroke(c, s, st);
    // equator + two latitude lines
    painter.line_segment([c + Vec2::new(-s, 0.0), c + Vec2::new(s, 0.0)], st);
    painter.line_segment([c + Vec2::new(-s * 0.86, -s * 0.5), c + Vec2::new(s * 0.86, -s * 0.5)], st);
    painter.line_segment([c + Vec2::new(-s * 0.86, s * 0.5), c + Vec2::new(s * 0.86, s * 0.5)], st);
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
    let front: Vec<Pos2> = (0..=steps).map(|i| {
        let t = i as f32 / steps as f32 * std::f32::consts::PI;
        Pos2::new(c.x + rw * t.cos(), bot + rh * t.sin())
    }).collect();
    painter.add(egui::Shape::line(front, st));
}

/// A control's own `Opacity` (0–100) as a 0.0–1.0 multiplier (default 1.0). The
/// render walk multiplies a container's `opacity_of` into the `alpha_mul` it
/// passes to descendants, so a faded container dims its whole subtree (spec 012).
pub fn opacity_of(ctrl: &Control) -> f32 {
    ctrl.get_prop("Opacity").map(|v| v.as_i64()).unwrap_or(100).clamp(0, 100) as f32 / 100.0
}

pub fn draw_control(
    painter:   &egui::Painter,
    origin:    Pos2,
    ctrl:      &Control,
    selected:  bool,
    glass:     bool,
    alpha_mul: f32,
    scale:     f32,                        // animation scale factor (1.0 = normal)
    pic_tex:   Option<egui::TextureId>,   // pre-loaded texture for PictureBox
) {
    use crate::ControlType as CT;

    let r = ctrl.rect;
    // Compute the base rect, then apply scale around the control center.
    let base_rect = egui::Rect::from_min_size(
        origin + Vec2::new(r.x as f32, r.y as f32),
        Vec2::new(r.w as f32, r.h as f32),
    );
    let rect = scale_rect_about_center(base_rect, scale);

    // Opacity (0–100) fades this control. Ancestor *container* opacities are
    // already folded into the incoming `alpha_mul` by the render walk, so a faded
    // container dims its whole subtree (spec 012). Default 100 ⇒ no change.
    let alpha_mul = alpha_mul * opacity_of(ctrl);

    let a = (alpha_mul.clamp(0.0, 1.0) * 255.0) as u8;
    let c_scale = |c: u8| -> u8 { ((c as f32) * alpha_mul) as u8 };
    let alpha_color = |c: Color32| Color32::from_rgba_premultiplied(c.r(), c.g(), c.b(), c_scale(c.a()));

    // ── Drop shadow ───────────────────────────────────────────────────────────
    let shadow_on = ctrl.get_prop("ShadowEnabled").map(|v| v.as_bool()).unwrap_or(false);
    if shadow_on && !matches!(ctrl.control_type, CT::Line | CT::Timer | CT::AgentObject | CT::RestClient | CT::SqlDatabase) {
        let shadow_color   = ctrl.get_prop("ShadowColor").map(|v| parse_color(v.as_str())).unwrap_or(Color32::BLACK);
        let shadow_opac    = ctrl.get_prop("ShadowOpacity").map(|v| v.as_i64()).unwrap_or(20).clamp(0, 100) as f32 / 100.0;
        let shadow_dir     = ctrl.get_prop("ShadowDirection").map(|v| v.as_str().to_owned()).unwrap_or_else(|| "South".into());
        let distance       = ctrl.get_prop("ShadowDistance").map(|v| v.as_i64()).unwrap_or(7).clamp(0, 60) as f32;
        let blur_enabled   = ctrl.get_prop("ShadowBlur").map(|v| v.as_bool()).unwrap_or(true);
        let blur_strength  = if blur_enabled {
            ctrl.get_prop("ShadowBlurStrength").map(|v| v.as_i64()).unwrap_or(8).clamp(0, 20) as usize
        } else { 0 };

        // Direction → unit vector (ux, uy)
        let (ux, uy): (f32, f32) = match shadow_dir.as_str() {
            "North"     => ( 0.0,   -1.0  ),
            "NorthEast" => ( 0.707, -0.707),
            "East"      => ( 1.0,    0.0  ),
            "SouthEast" => ( 0.707,  0.707),
            "South"     => ( 0.0,    1.0  ),
            "SouthWest" => (-0.707,  0.707),
            "West"      => (-1.0,    0.0  ),
            "NorthWest" => (-0.707, -0.707),
            _           => ( 0.0,    1.0  ),
        };
        let shadow_rect = rect.translate(Vec2::new(ux * distance, uy * distance));
        let corner_r    = ctrl.get_prop("CornerRadius").map(|v| v.as_i64() as f32).unwrap_or(3.0);
        let sc          = shadow_color;

        if blur_strength == 0 {
            // ── Hard shadow — single solid rect ───────────────────────────────
            let alpha = (shadow_opac * alpha_mul * 255.0) as u8;
            painter.rect_filled(
                shadow_rect,
                corner_r,
                Color32::from_rgba_premultiplied(
                    (sc.r() as f32 * shadow_opac * alpha_mul) as u8,
                    (sc.g() as f32 * shadow_opac * alpha_mul) as u8,
                    (sc.b() as f32 * shadow_opac * alpha_mul) as u8,
                    alpha,
                ),
            );
        } else {
            // ── Soft blur — concentric expanding rects with gaussian falloff ──
            // We draw `blur_strength + 1` layers from outermost (faintest) to
            // innermost (darkest), so the painter's back-to-front order gives the
            // right look: the core of the shadow is the most opaque.
            let layers = blur_strength;
            for i in 0..=layers {
                // i=0 → outer rim (t=1, faintest); i=layers → core (t=0, darkest)
                let t       = 1.0 - (i as f32 / layers as f32); // 1 → 0
                let expand  = t * blur_strength as f32;
                // Gaussian falloff: e^(-k·t²) where k controls how sharply the
                // shadow fades.  k=3 gives a natural soft shadow feel.
                let falloff = (-3.0 * t * t).exp();
                let alpha   = (shadow_opac * alpha_mul * falloff * 255.0) as u8;
                let layer_rect = shadow_rect.expand(expand);
                painter.rect_filled(
                    layer_rect,
                    corner_r + expand,
                    Color32::from_rgba_premultiplied(
                        (sc.r() as f32 * (alpha as f32 / 255.0)) as u8,
                        (sc.g() as f32 * (alpha as f32 / 255.0)) as u8,
                        (sc.b() as f32 * (alpha as f32 / 255.0)) as u8,
                        alpha,
                    ),
                );
            }
        }
    }

    // ── Line control ──────────────────────────────────────────────────────────
    if matches!(ctrl.control_type, CT::Line) {
        let line_color = ctrl.get_prop("LineColor").map(|v| parse_color(v.as_str())).unwrap_or(Color32::BLACK);
        let thickness  = ctrl.get_prop("LineThickness").map(|v| v.as_i64() as f32).unwrap_or(1.0);
        let dir        = ctrl.get_prop("LineDirection").map(|v| v.as_str().to_owned()).unwrap_or_else(|| "Horizontal".into());
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
                "Vertical" => (rect.left_top(),  rect.left_bottom()),
                "Diagonal" => (rect.left_top(),  rect.right_bottom()),
                _          => (rect.left_center(), rect.right_center()),
            }
        };
        let col = alpha_color(line_color);
        let stroke = Stroke::new(thickness, col);
        let t = thickness.max(1.0);
        // DashStyle: Solid | Dash | Dot | DashDot (egui dashed-line shapes).
        match ctrl.get_prop("DashStyle").map(|v| v.as_str().to_owned()).unwrap_or_else(|| "Solid".into()).as_str() {
            "Dash"    => painter.extend(egui::Shape::dashed_line(&[p1, p2], stroke, t * 5.0, t * 4.0)),
            "Dot"     => painter.extend(egui::Shape::dashed_line(&[p1, p2], stroke, t * 1.2, t * 2.5)),
            "DashDot" => painter.extend(egui::Shape::dashed_line_with_offset(
                &[p1, p2], stroke, &[t * 5.0, t * 1.2], &[t * 3.0, t * 3.0], 0.0)),
            _         => { painter.line_segment([p1, p2], stroke); }
        }
        // Rounded endings (round caps) — draw a disc at each end.
        if ctrl.get_prop("RoundedEnds").map(|v| v.as_bool()).unwrap_or(false) {
            let r = (thickness * 0.5).max(0.5);
            painter.circle_filled(p1, r, col);
            painter.circle_filled(p2, r, col);
        }
        if selected {
            painter.circle_stroke(p1, 4.0, Stroke::new(1.0, Color32::from_rgba_premultiplied(60,120,230, a)));
            painter.circle_stroke(p2, 4.0, Stroke::new(1.0, Color32::from_rgba_premultiplied(60,120,230, a)));
        }
        return;
    }

    // ── Shape control ─────────────────────────────────────────────────────────
    if matches!(ctrl.control_type, CT::Shape) {
        let fill_color = ctrl.get_prop("FillColor").map(|v| parse_color(v.as_str())).unwrap_or(Color32::from_rgb(192,192,192));
        let line_color = ctrl.get_prop("LineColor").map(|v| parse_color(v.as_str())).unwrap_or(Color32::BLACK);
        let thickness  = ctrl.get_prop("LineThickness").map(|v| v.as_i64() as f32).unwrap_or(1.0);
        let fill_style = ctrl.get_prop("FillStyle").map(|v| v.as_str().to_owned()).unwrap_or_else(|| "Solid".into());
        let shape_type = ctrl.get_prop("ShapeType").map(|v| v.as_str().to_owned()).unwrap_or_else(|| "Rectangle".into());

        let rr = match shape_type.as_str() {
            "Circle"    => rect.width().min(rect.height()) / 2.0,
            "Ellipse"   => rect.width().min(rect.height()) / 2.0, // backward compat
            "RoundRect" => 8.0,
            _           => 0.0,
        };

        let border_c = if selected {
            Color32::from_rgba_premultiplied(60, 120, 230, a)
        } else {
            alpha_color(line_color)
        };

        if shape_type == "Circle" || shape_type == "Ellipse" {
            // Circle / Ellipse — use circle primitives so the shape doesn't bleed.
            let circ_r = rect.width().min(rect.height()) / 2.0;
            let cc     = rect.center();
            if glass && fill_style != "None" {
                draw_glass_circle(painter, cc, circ_r, fill_color, selected, alpha_mul);
                if thickness > 0.0 {
                    painter.circle_stroke(cc, circ_r, Stroke::new(thickness, border_c));
                }
            } else {
                let fill = if fill_style == "None" { Color32::TRANSPARENT } else { alpha_color(fill_color) };
                painter.circle_filled(cc, circ_r, fill);
                painter.circle_stroke(cc, circ_r, Stroke::new(thickness, border_c));
            }
        } else if shape_type == "Triangle" {
            // Triangle — equilateral pointing up, filling the bounding rect.
            let top    = Pos2::new(rect.center().x, rect.min.y);
            let bot_l  = Pos2::new(rect.min.x, rect.max.y);
            let bot_r  = Pos2::new(rect.max.x, rect.max.y);
            let pts    = vec![top, bot_r, bot_l];
            let fill   = if fill_style == "None" { Color32::TRANSPARENT } else { alpha_color(fill_color) };
            painter.add(egui::Shape::convex_polygon(pts, fill, Stroke::new(thickness, border_c)));
        } else if glass && fill_style != "None" {
            // Rectangle / RoundRect — draw frosted glass using the user's FillColor as tint.
            draw_glass(painter, rect, fill_color, rr, selected, alpha_mul);
            if thickness > 0.0 {
                painter.rect_stroke(rect, rr, Stroke::new(thickness, border_c));
            }
        } else {
            let fill = if fill_style == "None" { Color32::TRANSPARENT } else { alpha_color(fill_color) };
            painter.rect_filled(rect, rr, fill);
            painter.rect_stroke(rect, rr, Stroke::new(thickness, border_c));
        }
        return;
    }

    // ── Non-visual controls — standardised glass card + stroke icon + label ─────
    if matches!(ctrl.control_type, CT::Timer | CT::AgentObject | CT::RestClient | CT::SqlDatabase) {
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
            _ /* SqlDatabase */ => {
                nv_icon_database(painter, cen, s, st);
                ctrl.get_prop("Driver").map(|v| v.as_str().to_owned()).unwrap_or_else(|| "sqlite".into())
            }
        };
        nv_label(painter, rect, &label, a);
        return;
    }

    // ── Slider ────────────────────────────────────────────────────────────────
    if matches!(ctrl.control_type, CT::Slider) {
        let min_v   = ctrl.get_prop("Minimum").map(|v| v.as_i64()).unwrap_or(0) as f32;
        let max_v   = ctrl.get_prop("Maximum").map(|v| v.as_i64()).unwrap_or(100).max(1) as f32;
        let val     = ctrl.get_prop("Value").map(|v| v.as_i64()).unwrap_or(0) as f32;
        let _step_v = ctrl.get_prop("Step").map(|v| v.as_i64()).unwrap_or(10).max(1) as f32;
        let tick_fr = ctrl.get_prop("TickFrequency").map(|v| v.as_i64()).unwrap_or(10).max(1) as f32;
        let tick_st = ctrl.get_prop("TickStyle").map(|v| v.as_str().to_owned()).unwrap_or_else(|| "Bottom".into());
        let orient  = ctrl.get_prop("Orientation").map(|v| v.as_str().to_owned()).unwrap_or_else(|| "Horizontal".into());
        let vertical = orient.starts_with('V');

        let track_c  = alpha_color(ctrl.get_prop("TrackColor").map(|v| parse_color(v.as_str())).unwrap_or(Color32::from_rgb(170,170,170)));
        let thumb_c  = alpha_color(ctrl.get_prop("ThumbColor").map(|v| parse_color(v.as_str())).unwrap_or(Color32::from_rgb(0,120,215)));
        let fill_c   = alpha_color(ctrl.get_prop("FillColor").map(|v| parse_color(v.as_str())).unwrap_or(Color32::from_rgb(0,120,215)));
        let show_val = ctrl.get_prop("ShowValue").map(|v| v.as_bool()).unwrap_or(false);

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
            let r = pill.height() / 2.0;
            painter.rect_filled(pill, r, body);
            if sheen {
                // Top-half gradient mesh: opaque white → transparent
                let mut mesh = egui::epaint::Mesh::default();
                let top    = pill.min.y;
                let mid    = pill.min.y + pill.height() * 0.5;
                let left   = pill.min.x + r;
                let right  = pill.max.x - r;
                let w_hi   = Color32::from_rgba_premultiplied(120,130,150, (80.0 * alpha_mul) as u8);
                let w_lo   = Color32::from_rgba_premultiplied(0,0,0,0);
                // quad: 4 vertices
                let i = mesh.vertices.len() as u32;
                mesh.vertices.push(egui::epaint::Vertex { pos: Pos2::new(left,  top), uv: egui::epaint::WHITE_UV, color: w_hi });
                mesh.vertices.push(egui::epaint::Vertex { pos: Pos2::new(right, top), uv: egui::epaint::WHITE_UV, color: w_hi });
                mesh.vertices.push(egui::epaint::Vertex { pos: Pos2::new(right, mid), uv: egui::epaint::WHITE_UV, color: w_lo });
                mesh.vertices.push(egui::epaint::Vertex { pos: Pos2::new(left,  mid), uv: egui::epaint::WHITE_UV, color: w_lo });
                mesh.indices.extend_from_slice(&[i,i+1,i+2, i,i+2,i+3]);
                painter.add(egui::Shape::mesh(mesh));
            }
            painter.rect_stroke(pill, r, Stroke::new(1.0, rim));
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
            let edge_c = Color32::from_rgba_premultiplied(0,0,0,0);
            let ci = mesh.vertices.len() as u32;
            mesh.vertices.push(egui::epaint::Vertex { pos: center, uv: egui::epaint::WHITE_UV, color: center_c });
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
                mesh.indices.extend_from_slice(&[ci, ci+1+i, ci+1+(i+1)%n]);
            }
            painter.add(egui::Shape::mesh(mesh));
        };

        // Glass track colors
        let track_body = Color32::from_rgba_premultiplied(
            (100.0 * alpha_mul) as u8, (110.0 * alpha_mul) as u8,
            (135.0 * alpha_mul) as u8, (90.0  * alpha_mul) as u8);
        let track_rim  = Color32::from_rgba_premultiplied(
            (180.0 * alpha_mul) as u8, (185.0 * alpha_mul) as u8,
            (210.0 * alpha_mul) as u8, (120.0 * alpha_mul) as u8);
        let thumb_body = Color32::from_rgba_premultiplied(
            (150.0 * alpha_mul) as u8, (160.0 * alpha_mul) as u8,
            (195.0 * alpha_mul) as u8, (140.0 * alpha_mul) as u8);
        let thumb_rim  = Color32::from_rgba_premultiplied(
            (220.0 * alpha_mul) as u8, (225.0 * alpha_mul) as u8,
            (245.0 * alpha_mul) as u8, (180.0 * alpha_mul) as u8);

        if vertical {
            // ── Vertical glass slider ────────────────────────────────────────
            let track_half_w = (rect.width() * 0.18).clamp(4.0, 12.0);
            let cx      = rect.center().x;
            let track_t = rect.min.y + 10.0;
            let track_b = rect.max.y - 10.0;
            let track_h = (track_b - track_t).max(1.0);
            let thumb_y = track_b - pct * track_h;
            let thumb_h = (track_half_w * 2.0 * 1.6).clamp(16.0, 32.0);
            let thumb_w = track_half_w * 2.0 + 6.0;

            // Track pill
            let track_rect = egui::Rect::from_min_max(
                Pos2::new(cx - track_half_w, track_t),
                Pos2::new(cx + track_half_w, track_b),
            );
            draw_glass_pill(painter, track_rect, track_body, true, track_rim);

            // Tick marks
            if tick_st != "None" && range_units > 0.0 {
                let mut tick_v = min_v;
                while tick_v <= max_v + 0.001 {
                    let ty = track_b - ((tick_v - min_v) / range_units).clamp(0.0, 1.0) * track_h;
                    let tick_color = Color32::from_rgba_premultiplied(140,145,165,(80.0*alpha_mul) as u8);
                    let tick_len = 5.0;
                    if tick_st == "Left" || tick_st == "Both" {
                        painter.line_segment([Pos2::new(cx - track_half_w - tick_len, ty), Pos2::new(cx - track_half_w - 1.0, ty)], Stroke::new(1.0, tick_color));
                    }
                    if tick_st != "Left" || tick_st == "Both" {
                        painter.line_segment([Pos2::new(cx + track_half_w + 1.0, ty), Pos2::new(cx + track_half_w + tick_len, ty)], Stroke::new(1.0, tick_color));
                    }
                    tick_v += tick_fr;
                }
            }

            // Thumb pill
            let thumb_rect = egui::Rect::from_center_size(
                Pos2::new(cx, thumb_y),
                Vec2::new(thumb_w, thumb_h),
            );
            draw_glass_pill(painter, thumb_rect, thumb_body, true, thumb_rim);
            // Lens at bottom-center of thumb
            draw_lens(painter,
                Pos2::new(cx, thumb_rect.max.y - thumb_h * 0.28),
                thumb_w * 0.32, thumb_h * 0.18);
        } else {
            // ── Horizontal glass slider ──────────────────────────────────────
            let track_half_h = (rect.height() * 0.18).clamp(4.0, 12.0);
            let cy      = rect.center().y;
            let track_l = rect.min.x + 10.0;
            let track_r = rect.max.x - 10.0;
            let track_w = (track_r - track_l).max(1.0);
            let thumb_x = track_l + pct * track_w;
            let thumb_w_half = (track_half_h * 1.6).clamp(8.0, 20.0);
            let thumb_h = track_half_h * 2.0 + 6.0;

            // Track pill
            let track_rect = egui::Rect::from_min_max(
                Pos2::new(track_l, cy - track_half_h),
                Pos2::new(track_r, cy + track_half_h),
            );
            draw_glass_pill(painter, track_rect, track_body, true, track_rim);

            // Tick marks
            if tick_st != "None" && range_units > 0.0 {
                let mut tick_v = min_v;
                while tick_v <= max_v + 0.001 {
                    let tx = track_l + ((tick_v - min_v) / range_units).clamp(0.0, 1.0) * track_w;
                    let tick_color = Color32::from_rgba_premultiplied(140,145,165,(80.0*alpha_mul) as u8);
                    let tick_len = 5.0;
                    if tick_st == "Top" || tick_st == "Both" {
                        painter.line_segment([Pos2::new(tx, cy - track_half_h - tick_len), Pos2::new(tx, cy - track_half_h - 1.0)], Stroke::new(1.0, tick_color));
                    }
                    if tick_st != "Top" || tick_st == "Both" {
                        painter.line_segment([Pos2::new(tx, cy + track_half_h + 1.0), Pos2::new(tx, cy + track_half_h + tick_len)], Stroke::new(1.0, tick_color));
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
            draw_lens(painter,
                Pos2::new(thumb_x, thumb_rect.max.y - thumb_h * 0.28),
                thumb_w_half * 0.6, thumb_h * 0.18);
        }

        // Step label (min / max corners)
        let font_s = egui::FontId::proportional(9.0);
        let lbl_c  = Color32::from_rgba_premultiplied(80,80,80,a);
        if vertical {
            painter.text(Pos2::new(rect.center().x, rect.max.y - 2.0), egui::Align2::CENTER_BOTTOM,
                format!("{}", min_v as i64), font_s.clone(), lbl_c);
            painter.text(Pos2::new(rect.center().x, rect.min.y + 2.0), egui::Align2::CENTER_TOP,
                format!("{}", max_v as i64), font_s.clone(), lbl_c);
        } else {
            painter.text(Pos2::new(rect.min.x + 2.0, rect.max.y - 1.0), egui::Align2::LEFT_BOTTOM,
                format!("{}", min_v as i64), font_s.clone(), lbl_c);
            painter.text(Pos2::new(rect.max.x - 2.0, rect.max.y - 1.0), egui::Align2::RIGHT_BOTTOM,
                format!("{}", max_v as i64), font_s.clone(), lbl_c);
        }

        // Optional current value label
        if show_val {
            painter.text(rect.center(), egui::Align2::CENTER_CENTER,
                format!("{}", val as i64), egui::FontId::proportional(ctrl_font_size(ctrl)),
                Color32::from_rgba_premultiplied(0,0,0,a));
        }

        // Selection border
        if selected {
            painter.rect_stroke(rect, 3.0, Stroke::new(2.0, Color32::from_rgba_premultiplied(60,120,230,a)));
        }
        return;
    }

    // ── ProgressBar ───────────────────────────────────────────────────────────
    if matches!(ctrl.control_type, CT::ProgressBar) {
        let bg_c  = Color32::from_rgba_premultiplied(220,220,220,a);
        let bar_c = alpha_color(ctrl.get_prop("BarColor").map(|v| parse_color(v.as_str())).unwrap_or(Color32::from_rgb(0,170,0)));
        let val   = ctrl.get_prop("Value").map(|v| v.as_i64()).unwrap_or(0) as f32;
        let min   = ctrl.get_prop("Minimum").map(|v| v.as_i64()).unwrap_or(0) as f32;
        let max   = ctrl.get_prop("Maximum").map(|v| v.as_i64()).unwrap_or(100).max(1) as f32;
        let pct   = ((val - min) / (max - min)).clamp(0.0, 1.0);
        painter.rect_filled(rect, 2.0, bg_c);
        let bar = egui::Rect::from_min_size(rect.min, Vec2::new(rect.width() * pct, rect.height()));
        if glass { draw_glass(painter, bar, Color32::from_rgb(0,170,0), 2.0, false, alpha_mul * pct); }
        else     { painter.rect_filled(bar, 2.0, bar_c); }
        let border_c = if selected { Color32::from_rgba_premultiplied(60,120,230,a) } else { Color32::from_rgba_premultiplied(140,140,160,a) };
        painter.rect_stroke(rect, 2.0, Stroke::new(if selected { 2.0 } else { 1.0 }, border_c));
        if ctrl.get_prop("ShowValue").map(|v| v.as_bool()).unwrap_or(false) {
            painter.text(rect.center(), egui::Align2::CENTER_CENTER, format!("{:.0}%", pct*100.0),
                egui::FontId::proportional(ctrl_font_size(ctrl)), Color32::from_rgba_premultiplied(0,0,0,a));
        }
        return;
    }

    // ── Generic rect-based controls ───────────────────────────────────────────

    let (default_fill, default_border, default_text) = control_colors(&ctrl.control_type, selected);

    let fill = ctrl.get_prop("BackgroundColor").map(|v| parse_color(v.as_str())).unwrap_or(default_fill);
    let label_color = ctrl.get_prop("ForegroundColor").map(|v| parse_color(v.as_str())).unwrap_or(default_text);
    let stroke_color = ctrl.get_prop("BorderColor").map(|v| parse_color(v.as_str())).unwrap_or(default_border);

    // Unified corner radius for every control (spec 016): canonical CornerRadius,
    // legacy BorderRadius alias, per-type default, clamped. 0 ⇒ square.
    let corner = corner_radius(ctrl);

    let is_label = matches!(ctrl.control_type, CT::Label);

    // A PictureBox with ShowFrame = false draws no card/background/border —
    // only the image (so transparent PNG areas reveal what's behind).
    let pic_frameless = matches!(ctrl.control_type, CT::PictureBox)
        && !ctrl.get_prop("ShowFrame").map(|v| v.as_bool()).unwrap_or(true);

    // A chart with HideBackground must draw NO card/glass frame here —
    // `draw_chart_preview` owns the chart's (suppressed) background, so the
    // generic frame drawn below would otherwise show through (spec 013 fix).
    let chart_frameless = matches!(ctrl.control_type,
        CT::BarChart | CT::LineChart | CT::PieChart
        | CT::AreaChart | CT::ScatterChart | CT::DonutChart)
        && ctrl.get_prop("HideBackground").map(|v| v.as_bool()).unwrap_or(false);

    // A GroupBox with HideBackground draws no fill/border (children stay visible);
    // with a background gradient enabled it fills with a directional gradient
    // instead of the solid BackgroundColor (spec 015).
    let group_frameless = matches!(ctrl.control_type, CT::GroupBox)
        && ctrl.get_prop("HideBackground").map(|v| v.as_bool()).unwrap_or(false);
    let group_gradient = matches!(ctrl.control_type, CT::GroupBox)
        && !group_frameless
        && ctrl.get_prop("BackgroundGradientEnabled").map(|v| v.as_bool()).unwrap_or(false);

    // 007 Form themes — when an asset-pack theme is active and covers this
    // control kind, 9-slice its skin instead of the procedural glass; controls
    // the pack doesn't cover fall through to Liquid Glass (R6, R7, R11).
    let theme_skin = active_theme(painter.ctx()).and_then(|pack| {
        let key = control_kind_key(&ctrl.control_type);
        if key.is_empty() { return None; }
        pack.control(key).map(|skin| (pack.clone(), skin.clone()))
    });

    if is_label || pic_frameless || chart_frameless || group_frameless {
        // No visible frame. When selected, show a lightweight selection outline.
        if selected {
            let sel_c = Color32::from_rgba_premultiplied(60, 120, 230, a);
            painter.rect_stroke(rect, 0.0, Stroke::new(1.0, sel_c));
        }
    } else if group_gradient {
        // Directional gradient background (spec 015). Fill via a per-vertex mesh,
        // then stroke the (rounded) border on top.
        let dir = ctrl.get_prop("BackgroundGradientDirection")
            .map(|v| v.as_str().to_owned()).unwrap_or_else(|| "Vertical".into());
        let start = alpha_color(ctrl.get_prop("BackgroundGradientStartColor")
            .map(|v| parse_color(v.as_str())).unwrap_or(fill));
        let end = alpha_color(ctrl.get_prop("BackgroundGradientEndColor")
            .map(|v| parse_color(v.as_str())).unwrap_or(fill));
        painter.add(egui::Shape::mesh(grad_dir_mesh(rect, start, end, &dir)));
        let bc = if selected { Color32::from_rgba_premultiplied(60,120,230,a) } else { alpha_color(stroke_color) };
        painter.rect_stroke(rect, corner, Stroke::new(if selected { 2.0 } else { 1.0 }, bc));
    } else if let Some((pack, skin)) = &theme_skin {
        let state = if selected { ControlState::Focused } else { ControlState::Normal };
        let img = pack.asset_path(skin.image_for(state));
        if let Some(tex) = load_theme_texture(painter.ctx(), &img.to_string_lossy()) {
            // Explicit BackgroundColor (R12) tints the skin; otherwise white = as-authored.
            let tint = Color32::from_white_alpha(a);
            draw_nine_slice(painter, rect, &tex, skin.slice, tint);
            if selected {
                painter.rect_stroke(rect, corner,
                    Stroke::new(2.0, Color32::from_rgba_premultiplied(60, 120, 230, a)));
            }
        } else {
            // Image missing / undecodable → never fail; fall back to glass (R11).
            draw_glass(painter, rect, fill, corner, selected, alpha_mul);
        }
    } else if glass {
        draw_glass(painter, rect, fill, corner, selected, alpha_mul);
        // Buttons get a subtle top specular — a soft vertical light reflection
        // that visually separates a clickable Button from flat fields like a
        // TextBox. Two stacked translucent bands fading downward.
        if matches!(ctrl.control_type, CT::Button) && rect.height() > 10.0 {
            let inset = (corner + 3.0).min(rect.width() * 0.25);
            let spec_h = (rect.height() * 0.30).clamp(3.0, 9.0);
            let band = |h: f32, alpha: u8| {
                let sa = (alpha as f32 * alpha_mul) as u8;
                painter.rect_filled(
                    egui::Rect::from_min_size(
                        rect.min + Vec2::new(inset, 2.0),
                        Vec2::new((rect.width() - 2.0 * inset).max(0.0), h)),
                    (corner - 1.0).max(2.0),
                    Color32::from_rgba_premultiplied(sa, sa, sa, sa));
            };
            band(spec_h, 16);          // wide soft glow
            band(spec_h * 0.45, 22);   // narrower brighter core
        }
    } else {
        painter.rect_filled(rect, corner, alpha_color(fill));
        let bc = if selected { Color32::from_rgba_premultiplied(60,120,230,a) } else { alpha_color(stroke_color) };
        painter.rect_stroke(rect, corner, Stroke::new(if selected { 2.0 } else { 1.0 }, bc));
    }

    // ── TabControl tab strip (spec 012) ────────────────────────────────────────
    // Draw a real strip of tabs across the top, highlighting the selected page.
    // The active page index is the `SelectedTab` property (the designer updates it
    // when a tab is clicked; the bounds here mirror `Control::content_rect`).
    if matches!(ctrl.control_type, CT::TabControl) {
        let tabs: Vec<String> = ctrl.get_prop("Tabs")
            .map(|v| v.as_str().lines().map(|s| s.to_string()).collect())
            .unwrap_or_default();
        let sel = ctrl.get_prop("SelectedTab").map(|v| v.as_i64()).unwrap_or(0).max(0) as usize;
        let strip_h = 24.0_f32;
        let mut tx = rect.min.x + 2.0;
        let ty = rect.min.y + 1.0;
        for (i, t) in tabs.iter().enumerate() {
            let tw = (t.chars().count() as f32 * 7.0 + 18.0).clamp(40.0, 160.0);
            if tx + tw > rect.max.x { break; }
            let tr = egui::Rect::from_min_size(Pos2::new(tx, ty), Vec2::new(tw, strip_h));
            let active = i == sel;
            let fill_c = if active { Color32::from_rgb(245, 246, 250) } else { Color32::from_rgb(208, 213, 224) };
            painter.rect_filled(tr, 3.0, alpha_color(fill_c));
            painter.rect_stroke(tr, 3.0, Stroke::new(1.0, alpha_color(stroke_color)));
            painter.text(tr.center(), egui::Align2::CENTER_CENTER, t,
                egui::FontId::proportional(11.0), alpha_color(Color32::from_rgb(40, 40, 50)));
            tx += tw + 2.0;
        }
    }

    // ── GroupBox caption — a "legend" on the top-left border, just past the
    // rounded corner (classic GroupBox look), vertically centred on the border
    // line. Suppressed by HideCaption (spec 015). ─────────────────────────────
    if matches!(ctrl.control_type, CT::GroupBox)
        && !ctrl.get_prop("HideCaption").map(|v| v.as_bool()).unwrap_or(false)
    {
        let cap = ctrl.get_prop("Caption").map(|v| v.to_string()).unwrap_or_else(|| ctrl.id.clone());
        if !cap.is_empty() {
            let font_name = ctrl.get_prop("FontName").map(|v| v.as_str()).unwrap_or_default();
            let font_id = crate::fonts::font_id(painter.ctx(), &font_name, ctrl_font_size(ctrl));
            let x = rect.min.x + corner.max(0.0) + 10.0;
            painter.text(Pos2::new(x, rect.min.y), egui::Align2::LEFT_CENTER, &cap,
                font_id, alpha_color(label_color));
        }
    }

    // Label text — Caption is on Label, Button, CheckBox, RadioButton.
    let label: String = match ctrl.control_type {
        CT::CheckBox => {
            let checked = ctrl.get_prop("Checked").map(|v| v.as_bool()).unwrap_or(false);
            let cap = ctrl.get_prop("Caption").map(|v| v.as_str().to_owned()).unwrap_or_else(|| ctrl.id.clone());
            format!("{} {cap}", if checked { "[✓]" } else { "[ ]" })
        }
        CT::RadioButton => {
            let checked = ctrl.get_prop("Checked").map(|v| v.as_bool()).unwrap_or(false);
            let cap = ctrl.get_prop("Caption").map(|v| v.as_str().to_owned()).unwrap_or_else(|| ctrl.id.clone());
            format!("{} {cap}", if checked { "(●)" } else { "( )" })
        }
        CT::ComboBox => {
            let items = ctrl.get_prop("Items").map(|v| v.as_str().to_owned()).unwrap_or_default();
            format!("{} ▾", items.lines().next().unwrap_or(""))
        }
        CT::DateTimePicker => {
            let val = ctrl.get_prop("Value").map(|v| v.as_str().to_owned()).filter(|s| !s.is_empty()).unwrap_or_else(|| "DD/MM/YYYY".into());
            format!("📅 {val}")
        }
        CT::NumericUpDown => {
            let v = ctrl.get_prop("Value").map(|v| v.as_i64()).unwrap_or(0);
            format!("{v} ▲▼")
        }
        CT::PictureBox => {
            // If we have a loaded texture, draw it directly and skip the text label.
            if let Some(tex_id) = pic_tex {
                let size_mode = ctrl.get_prop("SizeMode").map(|v| v.as_str().to_owned()).unwrap_or_else(|| "Normal".into());
                let tint = Color32::from_rgba_premultiplied(255, 255, 255, a);
                // Honour SizeMode with the image's native size so the aspect ratio
                // is preserved (Fit/Zoom/Center) identically to the run/preview —
                // the native size comes from the texture manager (spec 017 parity).
                let native = painter.ctx().tex_manager().read().meta(tex_id)
                    .map(|m| Vec2::new(m.size[0] as f32, m.size[1] as f32))
                    .unwrap_or_else(|| rect.size());
                let dest = media_dest_rect(rect, native, pic_size_mode(&size_mode));
                // Rounded image clipped to the corner radius (spec 016). When the
                // image is contained (Fit/Center) round the image rect; when it
                // overflows (Fill/Stretch) round the control rect with mapped UV.
                let contained = dest.width() <= rect.width() + 0.5 && dest.height() <= rect.height() + 0.5;
                let (shape_rect, uv) = if contained {
                    (dest, egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)))
                } else {
                    let dw = dest.width().max(1.0);
                    let dh = dest.height().max(1.0);
                    (rect, egui::Rect::from_min_max(
                        egui::pos2((rect.min.x - dest.min.x) / dw, (rect.min.y - dest.min.y) / dh),
                        egui::pos2((rect.max.x - dest.min.x) / dw, (rect.max.y - dest.min.y) / dh)))
                };
                painter.with_clip_rect(rect).add(egui::Shape::Rect(egui::epaint::RectShape {
                    rect: shape_rect,
                    rounding: egui::Rounding::same(corner),
                    fill: tint,                 // multiplies the texture (tint + alpha)
                    stroke: Stroke::NONE,
                    blur_width: 0.0,
                    fill_texture_id: tex_id,
                    uv,
                }));
                // Selection border on top
                if selected {
                    painter.rect_stroke(rect, corner, Stroke::new(2.0, Color32::from_rgba_premultiplied(60,120,230,a)));
                }
                return; // skip generic text rendering below
            }
            // No image loaded — show placeholder text
            if ctrl.get_prop("ImagePath").map(|v| !v.as_str().is_empty()).unwrap_or(false) {
                "🖼 [loading…]".into()
            } else {
                "🖼 (empty)".into()
            }
        }
        CT::Animator => {
            let source = ctrl.get_prop("Source").map(|v| v.as_str().to_owned()).unwrap_or_default();
            let auto    = ctrl.get_prop("AutoPlay").map(|v| v.as_bool()).unwrap_or(true);
            let looping = ctrl.get_prop("Loop").map(|v| v.as_bool()).unwrap_or(true);
            let size_mode = ctrl.get_prop("SizeMode").map(|v| v.as_str().to_owned())
                .unwrap_or_else(|| "Fit".into());
            let key = format!("{}|{}", ctrl.id, source.trim());
            draw_animator(painter, rect, &key, source.trim(), auto, looping, &size_mode, alpha_mul, selected);
            return;
        }
        CT::TreeView   => "🌲 [TreeView]".into(),
        CT::DataGrid   => {
            let cols = ctrl.get_prop("Columns").map(|v| v.as_str().to_owned()).unwrap_or_default();
            let col_count = cols.lines().count().max(1);
            format!("⊞ DataGrid ({col_count} cols)")
        }
        CT::Splitter   => {
            let dir = ctrl.get_prop("Orientation").map(|v| v.as_str().to_owned()).unwrap_or_else(|| "H".into());
            if dir.starts_with('V') { "║ Splitter".into() } else { "═ Splitter".into() }
        }
        // The tab strip is drawn above; no centered label.
        CT::TabControl => String::new(),
        CT::MenuBar    => "☰ MenuBar".into(),
        CT::ToolBar    => "⬛ ToolBar".into(),
        CT::StatusBar  => "▬ StatusBar".into(),
        // GroupBox draws its caption as a "legend" on the top-left border (below),
        // never as centered text.
        CT::GroupBox => String::new(),
        // Controls with an intrinsic text label use their Caption property.
        CT::Label | CT::Button =>
            ctrl.get_prop("Caption").map(|v| v.to_string()).unwrap_or_else(|| ctrl.id.clone()),
        // TextBox shows its current text value.
        CT::TextBox => ctrl.get_prop("Text").map(|v| v.to_string()).unwrap_or_default(),
        // Non-text controls (Panel, …) draw no caption — only GroupBox and Label
        // (and the text-bearing widgets above) carry one.
        _ => String::new(),
    };

    if !label.is_empty() {
        let txt_color = Color32::from_rgba_premultiplied(
            label_color.r(), label_color.g(), label_color.b(), a,
        );
        let fsize = ctrl_font_size(ctrl);
        let font_name = ctrl.get_prop("FontName").map(|v| v.as_str()).unwrap_or_default();

        // For Label controls, apply font-style properties via LayoutJob.
        if matches!(ctrl.control_type, CT::Label) {
            use egui::text::{LayoutJob, TextFormat};

            let bold        = ctrl.get_prop("Bold").map(|v| v.as_bool()).unwrap_or(false);
            let italic      = ctrl.get_prop("Italic").map(|v| v.as_bool()).unwrap_or(false);
            let underline   = ctrl.get_prop("Underline").map(|v| v.as_bool()).unwrap_or(false);
            let strikeout   = ctrl.get_prop("Strikethrough").map(|v| v.as_bool()).unwrap_or(false);

            // Egui doesn't have a separate bold typeface registered by default.
            // Simulate bold by painting the galley twice with a tiny x-offset.
            let font_id = crate::fonts::font_id(painter.ctx(), &font_name, fsize);

            // Honour the Label's TextAlignment (Left / Center / Right).
            let halign = text_halign(
                ctrl.get_prop("TextAlignment").map(|v| v.as_str()).unwrap_or(""));

            let mut job = LayoutJob::default();
            job.halign = halign;
            job.wrap.max_width = rect.width();
            job.wrap.break_anywhere = false;
            job.append(&label, 0.0, TextFormat {
                font_id: font_id.clone(),
                color: txt_color,
                italics: italic,
                underline: if underline {
                    Stroke::new(1.0, txt_color)
                } else {
                    Stroke::NONE
                },
                strikethrough: if strikeout {
                    Stroke::new(1.0, txt_color)
                } else {
                    Stroke::NONE
                },
                ..Default::default()
            });

            let galley = painter.layout_job(job);
            // The galley's draw origin follows `halign`: top-left for LEFT,
            // top-centre for CENTER, top-right for RIGHT. Anchor x to the
            // matching edge of the rect (with a small inset off the border);
            // y centres the wrapped block vertically.
            let pad = 3.0_f32.min(rect.width() * 0.25);
            let anchor_x = match halign {
                egui::Align::Center => rect.center().x,
                egui::Align::RIGHT  => rect.right() - pad,
                _                   => rect.left() + pad,
            };
            let text_pos = egui::pos2(
                anchor_x,
                rect.center().y - galley.size().y / 2.0,
            );
            painter.galley(text_pos, galley.clone(), txt_color);

            // Simulate bold: repaint shifted by 0.5 px
            if bold {
                painter.galley(text_pos + Vec2::new(0.5, 0.0), galley, txt_color);
            }
        } else {
            painter.text(
                rect.center(), egui::Align2::CENTER_CENTER, &label,
                crate::fonts::font_id(painter.ctx(), &font_name, fsize), txt_color,
            );
        }
    }

    // ── Charts ───────────────────────────────────────────────────────────────
    if matches!(ctrl.control_type,
        CT::BarChart | CT::LineChart | CT::PieChart |
        CT::AreaChart | CT::ScatterChart | CT::DonutChart)
    {
        draw_chart_preview(painter, ctrl, rect, a, alpha_mul, glass, selected);
        if selected {
            painter.rect_stroke(rect, 8.0, Stroke::new(2.0, Color32::from_rgba_premultiplied(60,120,230,a)));
        }
        // Animation indicator falls through to the shared badge below.
    }

    // Animation indicator badge
    if !ctrl.animations.is_empty() {
        let badge_pos = rect.right_top() + Vec2::new(-2.0, 2.0);
        painter.circle_filled(badge_pos, 5.0, Color32::from_rgba_premultiplied(255,180,0,180));
        painter.text(badge_pos, egui::Align2::CENTER_CENTER,
            "▶", egui::FontId::proportional(6.0), Color32::WHITE);
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
        "Zoom" | "Fit"             => "Fit",
        "Fill"                     => "Fill",
        _                          => "Center", // Normal / CenterImage / AutoSize
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
            let s = (rect.width() / native.x).min(rect.height() / native.y).min(1.0);
            egui::Rect::from_center_size(rect.center(), native * s)
        }
        // "Fit" (default): contain, preserving aspect ratio.
        _ => {
            let s = (rect.width() / native.x).min(rect.height() / native.y);
            egui::Rect::from_center_size(rect.center(), native * s)
        }
    }
}

/// Load an image file into an egui texture. Caching of the returned handle is
/// the caller's responsibility (see [`picturebox_texture`]). Shared so every
/// surface (designer, preview, run, compiled) decodes images the same way.
pub fn load_image_texture(ctx: &egui::Context, path: &str) -> Option<egui::TextureHandle> {
    let bytes = std::fs::read(path).ok()?;
    let img = image::load_from_memory(&bytes).ok()?.into_rgba8();
    let (w, h) = (img.width() as usize, img.height() as usize);
    let pixels: Vec<egui::Color32> = img
        .pixels()
        .map(|p| egui::Color32::from_rgba_unmultiplied(p[0], p[1], p[2], p[3]))
        .collect();
    let ci = egui::ColorImage { size: [w, h], pixels };
    Some(ctx.load_texture(path, ci, egui::TextureOptions::LINEAR))
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
        draw_glass(painter, rect, Color32::from_rgb(20, 30, 60), corner, false, alpha_mul * 0.7);
    }
    let a = (alpha_mul.clamp(0.0, 1.0) * 255.0) as u8;
    if let Some(tex) = picturebox_texture(painter.ctx(), image_path) {
        let dest = media_dest_rect(rect, tex.size_vec2(), pic_size_mode(size_mode));
        if corner > 0.0 {
            // Rounded image: a textured RectShape over the control bounds clips to
            // the corner radius (spec 016). UV maps the visible part of `dest`, so
            // Stretch/Fill/Zoom crop correctly; Fit margins are approximate.
            let dw = dest.width().max(1.0);
            let dh = dest.height().max(1.0);
            let uv = egui::Rect::from_min_max(
                egui::pos2((rect.min.x - dest.min.x) / dw, (rect.min.y - dest.min.y) / dh),
                egui::pos2((rect.max.x - dest.min.x) / dw, (rect.max.y - dest.min.y) / dh),
            );
            painter.add(egui::Shape::Rect(egui::epaint::RectShape {
                rect,
                rounding: egui::Rounding::same(corner),
                fill: Color32::from_white_alpha(a),
                stroke: Stroke::NONE,
                blur_width: 0.0,
                fill_texture_id: tex.id(),
                uv,
            }));
        } else {
            let uv = egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0));
            painter.with_clip_rect(rect).image(tex.id(), dest, uv, Color32::from_white_alpha(a));
        }
    } else if show_frame {
        painter.text(
            rect.center(), egui::Align2::CENTER_CENTER, "🖼",
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
        let a = if h.len() >= 8 { u8::from_str_radix(&h[6..8], 16).unwrap_or(255) } else { 255 };
        Some(Color32::from_rgba_unmultiplied(r, g, b, a))
    } else {
        None
    }
}

/// Short month names for DataGrid date cells and the DateTimePicker field.
pub const MONTH_ABBR: [&str; 12] = ["Jan", "Feb", "Mar", "Apr", "May", "Jun",
    "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"];

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
            Some((y, m, d)) => (format!("{:02} {} {}", d, MONTH_ABBR[(m.clamp(1, 12) - 1) as usize], y), false),
            None => (raw.to_owned(), false),
        },
        _ => (raw.to_owned(), false),
    }
}

// ── DateTimePicker calendar support ────────────────────────────────────────────
pub const CAL_CELL: f32 = 28.0;
pub const CAL_W: f32 = CAL_CELL * 7.0;
pub const CAL_NAV_H: f32 = 24.0;
pub const CAL_WK_H: f32 = 20.0;
pub const CAL_GRID_Y: f32 = CAL_NAV_H + CAL_WK_H; // area-top → first day row
pub const MONTHS: [&str; 12] = ["January", "February", "March", "April", "May", "June",
    "July", "August", "September", "October", "November", "December"];

/// Open/viewed-month state for a DateTimePicker calendar popup, stashed in egui
/// temp memory keyed by the control id.
#[derive(Clone)]
pub struct CalState {
    pub open: bool,
    pub year: i32,
    pub month: u32, // 1-12
}
impl Default for CalState {
    fn default() -> Self { Self { open: false, year: 2026, month: 6 } }
}

fn is_leap(y: i32) -> bool { (y % 4 == 0 && y % 100 != 0) || y % 400 == 0 }

pub fn days_in_month(y: i32, m: u32) -> u32 {
    match m {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 => if is_leap(y) { 29 } else { 28 },
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
    painter:     &egui::Painter,
    ui:          &mut egui::Ui,
    rect:        egui::Rect,
    control_id:  egui::Id,
    selected:    &str,
    is_open:     bool,
    enabled:     bool,
    alpha:       f32,
) -> bool {
    use egui::{Align2, FontId, Pos2};
    draw_glass(painter, rect, Color32::from_rgb(25, 38, 80), 6.0, false, alpha);
    painter.rect_stroke(rect, 6.0,
        Stroke::new(1.0, Color32::from_rgba_premultiplied(100, 140, 230, 150)));
    painter.text(Pos2::new(rect.min.x + 8.0, rect.center().y),
        Align2::LEFT_CENTER, selected, FontId::proportional(12.0),
        Color32::from_rgb(220, 228, 255));
    painter.text(Pos2::new(rect.max.x - 13.0, rect.center().y),
        Align2::CENTER_CENTER, if is_open { "▲" } else { "▼" },
        FontId::proportional(9.0), Color32::from_rgba_premultiplied(160, 190, 255, 200));
    enabled && ui.interact(rect, control_id, egui::Sense::click()).clicked()
}

/// Draw the ComboBox dropdown popup (call after all controls). Returns the user
/// action, if any.
pub fn glass_combo_popup(
    ui:           &mut egui::Ui,
    ctrl_id_str:  &str,
    header_rect:  egui::Rect,
    items:        &[String],
    selected_val: &str,
) -> Option<GlassComboAction> {
    use egui::{Align2, FontId, Pos2, Vec2};

    let item_h  = 22.0_f32;
    let popup_h = (items.len() as f32 * item_h).min(180.0);
    let popup_rect = egui::Rect::from_min_size(
        Pos2::new(header_rect.min.x, header_rect.max.y + 1.0),
        Vec2::new(header_rect.width(), popup_h),
    );

    let pointer_pos = ui.input(|i| i.pointer.hover_pos());
    let any_click   = ui.input(|i| i.pointer.any_click());
    if any_click {
        let inside = header_rect.contains(pointer_pos.unwrap_or(Pos2::ZERO))
            || popup_rect.contains(pointer_pos.unwrap_or(Pos2::ZERO));
        if !inside {
            return Some(GlassComboAction::Close);
        }
    }

    let pp = ui.painter_at(popup_rect);
    pp.rect_filled(popup_rect, 6.0, Color32::from_rgb(22, 30, 58));
    draw_glass(&pp, popup_rect, Color32::from_rgb(30, 42, 80), 6.0, false, 0.35);
    pp.rect_stroke(popup_rect, 6.0,
        Stroke::new(1.0, Color32::from_rgba_premultiplied(90, 130, 220, 180)));

    let mut action = None;
    for (i, item) in items.iter().enumerate() {
        let item_y = popup_rect.min.y + i as f32 * item_h;
        if item_y + item_h > popup_rect.max.y { break; }
        let item_rect = egui::Rect::from_min_size(
            Pos2::new(popup_rect.min.x, item_y),
            Vec2::new(popup_rect.width(), item_h));
        let iid = egui::Id::new(("glass_combo_item", ctrl_id_str, i));
        let is_sel  = item == selected_val;
        let hovered = pointer_pos.map(|p| item_rect.contains(p)).unwrap_or(false);
        if is_sel {
            pp.rect_filled(item_rect, 4.0, Color32::from_rgba_premultiplied(60, 100, 200, 120));
        } else if hovered {
            pp.rect_filled(item_rect, 4.0, Color32::from_rgba_premultiplied(50, 70, 150, 80));
        }
        pp.text(Pos2::new(item_rect.min.x + 10.0, item_rect.center().y),
            Align2::LEFT_CENTER, item, FontId::proportional(12.0),
            if is_sel { Color32::from_rgb(200, 220, 255) } else { Color32::from_rgb(210, 218, 245) });
        if ui.interact(item_rect, iid, egui::Sense::click()).clicked() {
            action = Some(GlassComboAction::Select(item.clone()));
        }
    }
    action
}

pub fn draw_animator(
    painter:   &egui::Painter,
    rect:      egui::Rect,
    key:       &str,
    source:    &str,
    auto_play: bool,
    looping:   bool,
    size_mode: &str,
    alpha_mul: f32,
    selected:  bool,
) {
    let a = (alpha_mul.clamp(0.0, 1.0) * 255.0) as u8;

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
            let dest = media_dest_rect(rect, native, size_mode);
            let clip = painter.with_clip_rect(rect);
            clip.image(
                tex,
                dest,
                egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                Color32::from_white_alpha(a),
            );
        }
        None => {
            // Placeholder: a dark "film" panel with a play glyph.
            painter.rect_filled(rect, 6.0, Color32::from_rgba_premultiplied(18, 24, 48, a));
            painter.rect_stroke(rect, 6.0,
                Stroke::new(1.0, Color32::from_rgba_premultiplied(120, 150, 230, a)));
            let label = if source.is_empty() { "▶ Animator" } else { "▶ (cannot load)" };
            painter.text(rect.center(), egui::Align2::CENTER_CENTER, label,
                egui::FontId::proportional(13.0),
                Color32::from_rgba_premultiplied(190, 205, 255, a));
        }
    }

    if selected {
        painter.rect_stroke(rect, 6.0,
            Stroke::new(2.0, Color32::from_rgba_premultiplied(60, 120, 230, a)));
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
    let s = if l > 0.5 { d / (2.0 - max - min) } else { d / (max + min) };
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
    let q = if l < 0.5 { l * (1.0 + s) } else { l + s - l * s };
    let p = 2.0 * l - q;
    let hue = |mut t: f32| -> f32 {
        if t < 0.0 { t += 1.0; }
        if t > 1.0 { t -= 1.0; }
        if t < 1.0 / 6.0 { p + (q - p) * 6.0 * t }
        else if t < 1.0 / 2.0 { q }
        else if t < 2.0 / 3.0 { p + (q - p) * (2.0 / 3.0 - t) * 6.0 }
        else { p }
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
            let t = if n == 1 { 0.5 } else { i as f32 / (n as f32 - 1.0) };
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
    let nl = if dark_bg { (l + 0.22).min(0.92) } else { (l - 0.22).max(0.10) };
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
            let l = 0.24 + (li as f32 / 15.0) * 0.56;            // 0.24 .. 0.80
            if hi == GREY_COL {
                let v = (l * 255.0).round() as u8;              // grey, never pure black/white
                out.push(Color32::from_rgb(v, v, v));
            } else {
                let s = 0.45 + ((li % 4) as f32 / 3.0) * 0.50;  // 0.45 .. 0.95
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
            let x = 0.5 * ((2.0 * p1.x) + (-p0.x + p2.x) * t
                + (2.0 * p0.x - 5.0 * p1.x + 4.0 * p2.x - p3.x) * t2
                + (-p0.x + 3.0 * p1.x - 3.0 * p2.x + p3.x) * t3);
            let y = 0.5 * ((2.0 * p1.y) + (-p0.y + p2.y) * t
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
    m.vertices.push(egui::epaint::Vertex { pos: rect.left_top(),     uv, color: top });
    m.vertices.push(egui::epaint::Vertex { pos: rect.right_top(),    uv, color: top });
    m.vertices.push(egui::epaint::Vertex { pos: rect.right_bottom(), uv, color: bottom });
    m.vertices.push(egui::epaint::Vertex { pos: rect.left_bottom(),  uv, color: bottom });
    m.indices.extend([0, 1, 2, 0, 2, 3]);
    m
}

/// Lerp two colours in straight component space (`t` clamped to 0..=1).
fn lerp_color(a: Color32, b: Color32, t: f32) -> Color32 {
    let t = t.clamp(0.0, 1.0);
    let l = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * t).round() as u8;
    Color32::from_rgba_premultiplied(l(a.r(), b.r()), l(a.g(), b.g()), l(a.b(), b.b()), l(a.a(), b.a()))
}

/// Directional gradient fill of `rect`, `start`→`end` (spec 015 GroupBox
/// background). Linear directions (Vertical/Horizontal/DiagonalDown/DiagonalUp)
/// use a 4-vertex quad with GPU-interpolated corner colours; Radial uses a
/// centre→edge fan. Corners are square — the rounded border is stroked
/// separately (same trade-off as the other mesh fills here).
fn grad_dir_mesh(rect: egui::Rect, start: Color32, end: Color32, dir: &str) -> egui::epaint::Mesh {
    let uv = egui::epaint::WHITE_UV;
    let mut m = egui::epaint::Mesh::default();
    if dir == "Radial" {
        let c = rect.center();
        m.vertices.push(egui::epaint::Vertex { pos: c, uv, color: start });
        let perim = [
            rect.left_top(), egui::pos2(c.x, rect.top()), rect.right_top(),
            egui::pos2(rect.right(), c.y), rect.right_bottom(),
            egui::pos2(c.x, rect.bottom()), rect.left_bottom(),
            egui::pos2(rect.left(), c.y),
        ];
        for p in perim { m.vertices.push(egui::epaint::Vertex { pos: p, uv, color: end }); }
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
    m.vertices.push(egui::epaint::Vertex { pos: rect.left_top(),     uv, color: tl });
    m.vertices.push(egui::epaint::Vertex { pos: rect.right_top(),    uv, color: tr });
    m.vertices.push(egui::epaint::Vertex { pos: rect.right_bottom(), uv, color: br });
    m.vertices.push(egui::epaint::Vertex { pos: rect.left_bottom(),  uv, color: bl });
    m.indices.extend([0, 1, 2, 0, 2, 3]);
    m
}

/// Radial-gradient disc (centre colour → edge colour) — one scatter bubble's or
/// pie slice's own gradient (spec 013).
fn radial_disc_mesh(center: Pos2, rad: f32, cc: Color32, ce: Color32) -> egui::epaint::Mesh {
    let uv = egui::epaint::WHITE_UV;
    let n = 24_u32;
    let mut m = egui::epaint::Mesh::default();
    m.vertices.push(egui::epaint::Vertex { pos: center, uv, color: cc });
    for i in 0..n {
        let a = i as f32 / n as f32 * std::f32::consts::TAU;
        m.vertices.push(egui::epaint::Vertex {
            pos: center + Vec2::new(a.cos(), a.sin()) * rad, uv, color: ce,
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
fn grad_area_mesh(top: &[Pos2], baseline: f32, top_c: Color32, bot_c: Color32) -> egui::epaint::Mesh {
    let uv = egui::epaint::WHITE_UV;
    let mut m = egui::epaint::Mesh::default();
    for (i, p) in top.iter().enumerate() {
        let base = m.vertices.len() as u32;
        m.vertices.push(egui::epaint::Vertex { pos: *p, uv, color: top_c });
        m.vertices.push(egui::epaint::Vertex { pos: egui::pos2(p.x, baseline), uv, color: bot_c });
        if i > 0 {
            m.indices.extend([base - 2, base - 1, base, base - 1, base + 1, base]);
        }
    }
    m
}

/// Pie/donut slice with a radial gradient (inner `cc` → outer `ce`). `inner_r`
/// 0 ⇒ solid pie fan; > 0 ⇒ donut ring strip (spec 013).
fn grad_slice_mesh(
    center: Pos2, start: f32, sweep: f32, inner_r: f32, outer_r: f32,
    cc: Color32, ce: Color32,
) -> egui::epaint::Mesh {
    let uv = egui::epaint::WHITE_UV;
    let steps = ((sweep.abs() * outer_r).max(4.0) as u32).clamp(4, 40);
    let mut m = egui::epaint::Mesh::default();
    if inner_r <= 0.0 {
        m.vertices.push(egui::epaint::Vertex { pos: center, uv, color: cc });
        for s in 0..=steps {
            let t = start + sweep * s as f32 / steps as f32;
            m.vertices.push(egui::epaint::Vertex {
                pos: Pos2::new(center.x + t.cos() * outer_r, center.y + t.sin() * outer_r), uv, color: ce,
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
                pos: Pos2::new(center.x + ct * inner_r, center.y + st * inner_r), uv, color: cc,
            });
            m.vertices.push(egui::epaint::Vertex {
                pos: Pos2::new(center.x + ct * outer_r, center.y + st * outer_r), uv, color: ce,
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
pub fn draw_chart_preview(
    painter:   &egui::Painter,
    ctrl:      &Control,
    rect:      egui::Rect,
    a:         u8,
    alpha_mul: f32,
    glass:     bool,
    selected:  bool,
) {
    use crate::model::ControlType as CT;

    let _ = selected; // selection border drawn by caller

    // ── Background ────────────────────────────────────────────────────────────
    // `HideBackground` suppresses the panel fill + border frame so only the chart
    // content (grid, axes, labels, data) is visible, transparent over the form.
    let hide_bg = ctrl.get_prop("HideBackground").map(|v| v.as_bool()).unwrap_or(false);
    // Unified corner radius (spec 016); default 8 preserves the prior chart look.
    let corner = corner_radius(ctrl);
    let bg = Color32::from_rgba_premultiplied(15,20,45,a);
    if !hide_bg {
        if glass {
            draw_glass(painter, rect, Color32::from_rgb(15,20,45), corner, false, alpha_mul);
        } else {
            painter.rect_filled(rect, corner, bg);
            let border = Color32::from_rgba_premultiplied(60,80,160,a);
            painter.rect_stroke(rect, corner, Stroke::new(1.0, border));
        }
    }

    // All chart content is drawn through a clipped painter so nothing bleeds
    // outside the rounded-corner frame.  We inset by 1 px so the border stroke
    // itself is never covered.
    let painter = &painter.with_clip_rect(rect.shrink(1.0));

    // 007 chart-style hook — an asset-pack theme supplies the data palette and
    // stroke width for the data marks (pie slices / lines / bars), so charts take
    // on the theme like every other control (R7). Liquid Glass keeps the built-in
    // accent palette.
    let active = active_theme(painter.ctx());
    let pal_raw: &[(u8,u8,u8)] = &[(76,155,232),(232,122,76),(76,232,122),(232,76,155)];
    let base_pal: Vec<Color32> = active.as_ref()
        .map(|p| &p.manifest.palette.chart)
        .filter(|v| !v.is_empty())
        .map(|v| v.iter().map(|s| parse_color(s)).collect::<Vec<_>>())
        .unwrap_or_else(|| pal_raw.iter().map(|&(r,g,b)| Color32::from_rgb(r, g, b)).collect());
    let chart_stroke = active.as_ref()
        .map(|p| p.manifest.chart_style.stroke_width)
        .filter(|w| *w > 0.0)
        .unwrap_or(1.8);

    // ── Monochrome mode (spec 013) ─────────────────────────────────────────────
    // When on, the data palette is replaced by tonal variations of one base
    // colour, and support colours (grid/axis/border) become derived variants. The
    // chart face is dark, so borders take the *lighter* variant. Text/alpha are
    // left untouched (handled by the existing paths below).
    let mono = ctrl.get_prop("Monochrome").map(|v| v.as_bool()).unwrap_or(false);
    let mono_base = parse_color(
        &ctrl.get_prop("MonochromeColor").map(|v| v.as_str().to_owned())
            .unwrap_or_else(|| "#3F6FB5".into()));
    let pal: Vec<Color32> = if mono {
        let k = if matches!(ctrl.control_type, CT::PieChart | CT::DonutChart) { 4 } else { 2 };
        monochrome_palette(mono_base, k)
    } else {
        base_pal
    };
    let mono_border = border_variant(mono_base, true);

    // Inner plot area (leave margin for axes / labels)
    let margin_l = rect.width()  * 0.10;
    let margin_b = rect.height() * 0.12;
    let margin_t = rect.height() * 0.12;
    let margin_r = rect.width()  * 0.04;
    let plot = egui::Rect::from_min_max(
        Pos2::new(rect.min.x + margin_l, rect.min.y + margin_t),
        Pos2::new(rect.max.x - margin_r, rect.max.y - margin_b),
    );

    // Monochrome gradient (spec 013): when on, each data element gets its OWN
    // tonal gradient (bars vertical, bubbles/slices radial) and line/area charts
    // get a vertical fill gradient — handled per-branch via mesh helpers below.
    let gradient = mono && ctrl.get_prop("MonochromeGradient").map(|v| v.as_bool()).unwrap_or(false);

    // title
    let title = ctrl.get_prop("Title").map(|v| v.as_str().to_owned()).unwrap_or_default();
    if !title.is_empty() {
        painter.text(
            Pos2::new(rect.center().x, rect.min.y + margin_t * 0.5),
            egui::Align2::CENTER_CENTER, &title,
            egui::FontId::proportional(10.0),
            // The design-time grid face is white — the title must be dark to
            // be readable (it was near-white and invisible on the face).
            Color32::DARK_GRAY);
    }

    // ── Grid lines ────────────────────────────────────────────────────────────
    let show_grid = ctrl.get_prop("ShowGridLines").map(|v| v.as_bool()).unwrap_or(true);
    if show_grid {
        // Monochrome: grid lines use a soft pastel of the base colour (spec 013 R5).
        let grid_c = if mono { pastel_of(mono_base) } else { Color32::from_rgb(118, 142, 225) };
        let n_h = 4u32;
        for i in 1..n_h {
            let y = plot.min.y + plot.height() * i as f32 / n_h as f32;
            painter.line_segment([Pos2::new(plot.min.x, y), Pos2::new(plot.max.x, y)],
                Stroke::new(1.15, grid_c));
        }
        if !matches!(ctrl.control_type, CT::PieChart | CT::DonutChart) {
            let n_v = 5u32;
            for i in 1..n_v {
                let x = plot.min.x + plot.width() * i as f32 / n_v as f32;
                painter.line_segment([Pos2::new(x, plot.min.y), Pos2::new(x, plot.max.y)],
                    Stroke::new(1.15, grid_c));
            }
        }
    }

    // Axes (monochrome: a pastel/slightly-stronger variant of the base — spec 013 R5)
    let ax_c = if mono { axis_variant(mono_base) } else { Color32::from_rgb(84, 104, 190) };
    if !matches!(ctrl.control_type, CT::PieChart | CT::DonutChart) {
        // X/Y axis-line visibility is independently toggleable (default on).
        let show_x = ctrl.get_prop("ShowXAxis").map(|v| v.as_bool()).unwrap_or(true);
        let show_y = ctrl.get_prop("ShowYAxis").map(|v| v.as_bool()).unwrap_or(true);
        if show_x {
            painter.line_segment([plot.left_bottom(), plot.right_bottom()], Stroke::new(1.45, ax_c));
        }
        if show_y {
            painter.line_segment([plot.left_bottom(), plot.left_top()], Stroke::new(1.45, ax_c));
        }
    }

    // ── Sample data (representative preview) ──────────────────────────────────
    // Normalised Y values for 5 data points, 2 series
    let series1: &[f32] = &[0.40, 0.70, 0.55, 0.85, 0.60];
    let series2: &[f32] = &[0.25, 0.45, 0.70, 0.50, 0.80];
    let n = series1.len();

    let px_x = |i: usize| plot.min.x + (i as f32 + 0.5) / n as f32 * plot.width();
    let px_y = |v: f32|   plot.max.y - v * plot.height();

    // Line/area curve smoothing (spec 013): the `Smooth` property now actually
    // bends the polyline into a Catmull-Rom spline. `ShowPoints` gates markers.
    let smooth = ctrl.get_prop("Smooth").map(|v| v.as_bool()).unwrap_or(true);
    let show_points = ctrl.get_prop("ShowPoints").map(|v| v.as_bool()).unwrap_or(true);

    match ctrl.control_type {
        CT::BarChart => {
            let horizontal = ctrl.get_prop("Horizontal").map(|v| v.as_bool()).unwrap_or(false);
            let bar_total  = plot.width() / n as f32;
            let bar_w      = bar_total * 0.38;
            let gap        = bar_total * 0.05;
            for (si, series) in [series1, series2].iter().enumerate() {
                for (i, &v) in series.iter().enumerate() {
                    let br = if horizontal {
                        let y  = plot.min.y + (i as f32 + 0.5 + si as f32 * (0.5 + gap)) / n as f32 * plot.height() - bar_w * 0.5;
                        let w  = v * plot.width();
                        egui::Rect::from_min_size(Pos2::new(plot.min.x, y), Vec2::new(w, bar_w))
                    } else {
                        let x  = plot.min.x + (i as f32 * bar_total) + si as f32 * (bar_w + gap) + gap;
                        let h  = v * plot.height();
                        egui::Rect::from_min_size(Pos2::new(x, plot.max.y - h), Vec2::new(bar_w, h))
                    };
                    if gradient {
                        // Each bar gets its own light→dark vertical gradient.
                        painter.add(egui::Shape::mesh(grad_rect_mesh(
                            br, shade(mono_base, 0.20), shade(mono_base, -0.20))));
                    } else {
                        painter.rect_filled(br, 2.0, pal[si % pal.len()]);
                    }
                }
            }
        }
        CT::LineChart => {
            for (si, series) in [series1, series2].iter().enumerate() {
                let raw: Vec<Pos2> = series.iter().enumerate()
                    .map(|(i, &v)| Pos2::new(px_x(i), px_y(v)))
                    .collect();
                // `Smooth` bends the line into a Catmull-Rom curve (spec 013).
                let line = if smooth { catmull_rom(&raw, 14) } else { raw.clone() };
                let c = pal[si % pal.len()];
                if gradient {
                    // Vertical gradient fill under the line: brightest at the line,
                    // fading to transparent at the baseline (spec 013, mockup look).
                    let top_c = shade(mono_base, 0.12);
                    let bot_c = Color32::from_rgba_unmultiplied(top_c.r(), top_c.g(), top_c.b(), 0);
                    painter.add(egui::Shape::mesh(grad_area_mesh(&line, plot.max.y, top_c, bot_c)));
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
                let raw: Vec<Pos2> = series.iter().enumerate()
                    .map(|(i, &v)| Pos2::new(px_x(i), px_y(v)))
                    .collect();
                let top = if smooth { catmull_rom(&raw, 14) } else { raw.clone() };
                // Fill via a per-column mesh (handles the concave smoothed edge).
                // Non-gradient keeps the existing alpha-80 translucency (R8);
                // gradient fades vertically from the line to transparent.
                let (top_c, bot_c, line_c) = if gradient {
                    let t = shade(mono_base, 0.12);
                    (Color32::from_rgba_unmultiplied(t.r(), t.g(), t.b(), 150),
                     Color32::from_rgba_unmultiplied(t.r(), t.g(), t.b(), 0),
                     shade(mono_base, 0.10))
                } else {
                    let c = pal[si % pal.len()];
                    let f = Color32::from_rgba_unmultiplied(c.r(), c.g(), c.b(), 80);
                    (f, f, c)
                };
                painter.add(egui::Shape::mesh(grad_area_mesh(&top, plot.max.y, top_c, bot_c)));
                for w in top.windows(2) {
                    painter.line_segment([w[0], w[1]], Stroke::new(chart_stroke, line_c));
                }
            }
        }
        CT::ScatterChart => {
            let pts1: &[(f32,f32)] = &[(0.15,0.65),(0.35,0.40),(0.50,0.78),(0.70,0.30),(0.88,0.55)];
            let pts2: &[(f32,f32)] = &[(0.20,0.30),(0.42,0.72),(0.60,0.45),(0.78,0.85)];
            for (pts, ci) in [(pts1, 0usize), (pts2, 1)] {
                let c = pal[ci % pal.len()];
                for &(fx, fy) in pts {
                    let p = Pos2::new(plot.min.x + fx*plot.width(), plot.max.y - fy*plot.height());
                    if gradient {
                        // Each bubble: its own radial gradient (light centre → dark edge).
                        painter.add(egui::Shape::mesh(radial_disc_mesh(
                            p, 5.0, shade(mono_base, 0.20), shade(mono_base, -0.20))));
                    } else {
                        painter.circle_stroke(p, 4.5, Stroke::new(1.5, c));
                    }
                }
            }
        }
        CT::PieChart | CT::DonutChart => {
            let center  = plot.center();
            let outer_r = plot.size().min_elem() * 0.44;
            let inner_r = if ctrl.control_type == CT::DonutChart {
                let pct = ctrl.get_prop("InnerRadius").map(|v| v.as_i64()).unwrap_or(40) as f32 / 100.0;
                outer_r * pct
            } else { 0.0 };

            let slices: &[f32] = &[0.30, 0.20, 0.25, 0.25]; // proportions
            let mut start = -std::f32::consts::FRAC_PI_2; // top
            for (i, &frac) in slices.iter().enumerate() {
                let sweep = frac * TAU;
                let end   = start + sweep;
                let steps = ((sweep * outer_r).max(4.0) as u32).min(40).max(4);
                // Outline points (fan for pie, ring for donut) — used for the
                // slice border in both fill modes.
                let mut pts: Vec<Pos2> = Vec::with_capacity(steps as usize + 2);
                if inner_r > 0.0 {
                    for s in 0..=steps {
                        let t = start + sweep * s as f32 / steps as f32;
                        pts.push(Pos2::new(center.x + t.cos()*outer_r, center.y + t.sin()*outer_r));
                    }
                    for s in (0..=steps).rev() {
                        let t = start + sweep * s as f32 / steps as f32;
                        pts.push(Pos2::new(center.x + t.cos()*inner_r, center.y + t.sin()*inner_r));
                    }
                } else {
                    pts.push(center);
                    for s in 0..=steps {
                        let t = start + sweep * s as f32 / steps as f32;
                        pts.push(Pos2::new(center.x + t.cos()*outer_r, center.y + t.sin()*outer_r));
                    }
                }
                // Monochrome: slice borders use a lighter variant of the base so
                // adjacent slices separate on the dark face (spec 013 R6).
                let slice_stroke = if mono { mono_border } else { bg };
                if gradient {
                    // Each slice gets its own radial gradient (light inner → dark outer).
                    painter.add(egui::Shape::mesh(grad_slice_mesh(
                        center, start, sweep, inner_r, outer_r,
                        shade(mono_base, 0.20), shade(mono_base, -0.20))));
                    painter.add(egui::Shape::closed_line(pts, Stroke::new(0.8, slice_stroke)));
                } else {
                    let c = pal[i % pal.len()];
                    let fill = Color32::from_rgba_premultiplied(c.r(), c.g(), c.b(), (a as f32 * 0.85) as u8);
                    painter.add(egui::Shape::convex_polygon(pts, fill, Stroke::new(0.8, slice_stroke)));
                }
                start = end;
            }
        }
        _ => {}
    }

    // data source hint
    let ds = ctrl.get_prop("DataSource").map(|v| v.as_str().to_owned()).unwrap_or_default();
    if !ds.is_empty() {
        painter.text(
            Pos2::new(rect.center().x, rect.max.y - margin_b * 0.4),
            egui::Align2::CENTER_CENTER,
            format!("⬡ {ds}"),
            egui::FontId::proportional(8.5),
            Color32::from_rgba_premultiplied(130,160,220,a));
    }

    // type badge
    let badge = match ctrl.control_type {
        CT::BarChart     => "BAR",
        CT::LineChart    => "LINE",
        CT::PieChart     => "PIE",
        CT::AreaChart    => "AREA",
        CT::ScatterChart => "SCATTER",
        CT::DonutChart   => "DONUT",
        _                => "",
    };
    if !badge.is_empty() {
        painter.text(
            Pos2::new(rect.max.x - margin_r - 2.0, rect.min.y + margin_t * 0.45),
            egui::Align2::RIGHT_CENTER,
            badge,
            egui::FontId::proportional(8.0),
            Color32::from_rgba_premultiplied(80,100,180,a));
    }
}

/// Unified corner radius (px) for a control's rounded fill/border and content
/// (spec 016). Reads the canonical `CornerRadius`, falls back to the legacy
/// container `BorderRadius` (spec 012), then a per-type default, and clamps to
/// half the smaller side so a large value can never produce a degenerate shape.
/// `0` ⇒ square corners (and no rounded clipping).
pub fn corner_radius(ctrl: &Control) -> f32 {
    let raw = ctrl.get_prop("CornerRadius")
        .or_else(|| ctrl.get_prop("BorderRadius"))
        .map(|v| v.as_i64() as f32)
        .unwrap_or_else(|| match ctrl.control_type {
            ControlType::Button => 3.0,
            ControlType::BarChart | ControlType::LineChart | ControlType::PieChart
            | ControlType::AreaChart | ControlType::ScatterChart | ControlType::DonutChart => 8.0,
            _ => 0.0,
        });
    let max_r = 0.5 * (ctrl.rect.w.min(ctrl.rect.h) as f32);
    raw.clamp(0.0, max_r.max(0.0))
}

pub fn control_colors(ct: &ControlType, selected: bool) -> (Color32, Color32, Color32) {
    let border = if selected { Color32::from_rgb(60,120,230) } else { Color32::from_rgb(140,140,160) };
    match ct {
        ControlType::Button         => (Color32::from_rgb(220,220,235), border, Color32::WHITE),
        ControlType::Label          => (Color32::TRANSPARENT, border, Color32::WHITE),
        ControlType::TextBox        => (Color32::WHITE, border, Color32::DARK_GRAY),
        ControlType::CheckBox | ControlType::RadioButton => (Color32::TRANSPARENT, border, Color32::BLACK),
        ControlType::GroupBox | ControlType::Panel => (Color32::from_rgba_premultiplied(200,200,210,40), border, Color32::DARK_GRAY),
        ControlType::PictureBox     => (Color32::from_rgb(180,200,220), border, Color32::DARK_GRAY),
        ControlType::DataGrid | ControlType::ListBox => (Color32::WHITE, border, Color32::DARK_GRAY),
        ControlType::MenuBar | ControlType::ToolBar | ControlType::StatusBar => (Color32::from_rgb(200,200,215), border, Color32::BLACK),
        ControlType::DateTimePicker | ControlType::NumericUpDown => (Color32::WHITE, border, Color32::DARK_GRAY),
        ControlType::TreeView       => (Color32::WHITE, border, Color32::DARK_GRAY),
        ControlType::Splitter       => (Color32::from_rgb(180,180,190), border, Color32::DARK_GRAY),
        ControlType::ComboBox       => (Color32::WHITE, border, Color32::DARK_GRAY),
        ControlType::TabControl     => (Color32::from_rgba_premultiplied(210,215,230,120), border, Color32::BLACK),
        _                           => (Color32::from_rgb(210,210,225), border, Color32::BLACK),
    }
}

pub fn ctrl_font_size(ctrl: &Control) -> f32 {
    ctrl.get_prop("FontSize").map(|v| v.as_i64() as f32).unwrap_or(11.0).clamp(4.0, 200.0)
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
        ) { return Color32::from_rgba_unmultiplied(r, g, b, a); }
    }
    // 6-char RRGGBB — fully opaque
    if s.len() == 6 {
        if let (Ok(r), Ok(g), Ok(b)) = (
            u8::from_str_radix(&s[0..2], 16),
            u8::from_str_radix(&s[2..4], 16),
            u8::from_str_radix(&s[4..6], 16),
        ) { return Color32::from_rgb(r, g, b); }
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

fn active_theme_id() -> egui::Id { egui::Id::new("cobolt-active-theme-pack") }

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
    ctx.data(|d| d.get_temp::<ActiveTheme>(active_theme_id())).and_then(|a| a.0)
}

#[derive(Clone, Default)]
struct ThemeTexCache(Arc<Mutex<HashMap<String, egui::TextureHandle>>>);

/// Load (and cache, per egui context) a theme image as an egui texture. Returns
/// `None` if the file is missing or undecodable so the caller can fall back.
fn load_theme_texture(ctx: &egui::Context, abs_path: &str) -> Option<egui::TextureHandle> {
    let cache = ctx.data_mut(|d|
        d.get_temp_mut_or_default::<ThemeTexCache>(egui::Id::new("cobolt-theme-tex")).clone());
    if let Some(h) = cache.0.lock().unwrap().get(abs_path) {
        return Some(h.clone());
    }
    let bytes = std::fs::read(abs_path).ok()?;
    let img = image::load_from_memory(&bytes).ok()?.to_rgba8();
    let (w, h) = img.dimensions();
    let color = egui::ColorImage::from_rgba_unmultiplied([w as usize, h as usize], img.as_raw());
    let handle = ctx.load_texture(abs_path, color, egui::TextureOptions::LINEAR);
    cache.0.lock().unwrap().insert(abs_path.to_owned(), handle.clone());
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
                Pos2::new(dxs[col + 1], dys[row + 1]));
            let u = Rect::from_min_max(
                Pos2::new(sxs[col] / tw, sys[row] / th),
                Pos2::new(sxs[col + 1] / tw, sys[row + 1] / th));
            cells.push((d, u));
        }
    }
    cells
}

/// 9-slice a texture into `dst`, tinted by `tint`.
fn draw_nine_slice(painter: &egui::Painter, dst: Rect, tex: &egui::TextureHandle, slice: Slice, tint: Color32) {
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
    if !use_theme_background { return false; }
    let Some(pack) = active_theme(painter.ctx()) else { return false; };
    let Some(bg) = pack.manifest.background.as_ref() else { return false; };
    if bg.image.is_empty() { return false; }
    let abs = pack.asset_path(&bg.image);
    let Some(tex) = load_theme_texture(painter.ctx(), &abs.to_string_lossy()) else { return false; };

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
                    Pos2::new((cell.width() / sz.x).min(1.0), (cell.height() / sz.y).min(1.0)));
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
        CT::Button         => "button",
        CT::Panel          => "panel",
        CT::GroupBox       => "groupbox",
        CT::TextBox        => "textbox",
        CT::ComboBox       => "combobox",
        CT::ListBox        => "listbox",
        CT::CheckBox       => "checkbox",
        CT::RadioButton    => "radiobutton",
        CT::DataGrid       => "datagrid",
        CT::Slider         => "slider",
        CT::ProgressBar    => "progressbar",
        CT::TabControl     => "tabcontrol",
        CT::DateTimePicker => "datetimepicker",
        CT::NumericUpDown  => "numericupdown",
        CT::TreeView       => "treeview",
        CT::Splitter       => "splitter",
        CT::MenuBar        => "menubar",
        CT::ToolBar        => "toolbar",
        CT::StatusBar      => "statusbar",
        CT::PictureBox     => "picturebox",
        _                  => "",
    }
}

#[cfg(test)]
mod theme_render_tests {
    use super::*;

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
            assert!(d.width() >= 0.0 && d.height() >= 0.0, "no inverted dest rect");
        }
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
            assert_ne!((c.r(), c.g(), c.b()), (0, 0, 0), "pure black must be excluded");
            assert_ne!((c.r(), c.g(), c.b()), (255, 255, 255), "pure white must be excluded");
            assert!(!is_extreme(c), "swatch too close to an extreme: {c:?}");
        }
    }

    #[test]
    fn catmull_rom_smooths_and_keeps_endpoints() {
        let pts = vec![
            Pos2::new(0.0, 0.0), Pos2::new(10.0, 20.0),
            Pos2::new(20.0, 5.0), Pos2::new(30.0, 25.0),
        ];
        let sm = catmull_rom(&pts, 12);
        assert!(sm.len() > pts.len(), "smoothing should add intermediate points");
        assert_eq!(sm.first().copied(), Some(pts[0]), "keeps first point");
        assert_eq!(sm.last().copied(), Some(*pts.last().unwrap()), "keeps last point");
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
        // Linear directions: a 4-vertex quad with start/end on opposite edges.
        for dir in ["Vertical", "Horizontal", "DiagonalDown", "DiagonalUp"] {
            let m = grad_dir_mesh(rect, a, b, dir);
            assert_eq!(m.vertices.len(), 4, "{dir} should be a quad");
            let has_start = m.vertices.iter().any(|v| v.color == a);
            let has_end   = m.vertices.iter().any(|v| v.color == b);
            assert!(has_start && has_end, "{dir} must carry both endpoint colours");
        }
        // Vertical: top edge = start, bottom edge = end.
        let v = grad_dir_mesh(rect, a, b, "Vertical");
        assert_eq!(v.vertices[0].color, a); // top-left
        assert_eq!(v.vertices[2].color, b); // bottom-right
        // Radial: centre = start, all perimeter = end (fan = 1 + 8 verts).
        let r = grad_dir_mesh(rect, a, b, "Radial");
        assert_eq!(r.vertices.len(), 9);
        assert_eq!(r.vertices[0].color, a, "radial centre is the start colour");
        assert!(r.vertices[1..].iter().all(|v| v.color == b), "radial rim is the end colour");
    }

    #[test]
    fn corner_radius_reads_alias_default_and_clamps() {
        use crate::model::{Control, ControlType, PropValue, Rect};
        let big = |t| { let mut c = Control::new("C", t, 0, 0); c.rect = Rect::new(0,0,200,100); c };
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
        s.rect = Rect::new(0,0,24,24);
        s.set_prop("CornerRadius", PropValue::Int(40));
        assert_eq!(corner_radius(&s), 12.0);
        // Zero stays zero.
        let mut z = big(ControlType::Button);
        z.set_prop("CornerRadius", PropValue::Int(0));
        assert_eq!(corner_radius(&z), 0.0);
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
}
