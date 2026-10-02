// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! The Aurora Pastel IDE theme's painted surfaces: the multi-gradient window
//! backdrop, one pastel tint per pane, the corner-glow pane border, and the
//! discrete 3D toolbar pills. The flat palette is `theme::AURORA_PASTEL`;
//! everything here is a no-op under any other theme.
//!
//! The structure of the IDE is untouched: every pane keeps its place and
//! behaviour, only the way it is painted changes (operator, 2026-10-02, after
//! the mockup "IDE — Aurora Pastel").

use egui::epaint::{Mesh, Shadow};
use egui::{Color32, CornerRadius, Pos2, Rect, Response, Shape, Stroke, Ui};

/// Whether the active IDE theme is Aurora Pastel.
pub fn active() -> bool {
    crate::theme::active().is_aurora()
}

/// Card corner radius for a pane.
pub const PANE_RADIUS: u8 = 16;

/// The pane a frame belongs to; each takes its own pastel.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Pane {
    /// The left pane: the Grace button over the project tree.
    Project,
    /// The agent bar over the main pane.
    Agent,
    /// The main pane: inspector, editor, designer, settings.
    Main,
    /// The Output pane along the bottom.
    Output,
}

fn rgba(r: u8, g: u8, b: u8, a: f32) -> Color32 {
    Color32::from_rgba_unmultiplied(r, g, b, (a * 255.0).round() as u8)
}

/// The pane's fill under Aurora Pastel, else `default` unchanged.
pub fn pane_fill(pane: Pane, default: Color32) -> Color32 {
    if !active() {
        return default;
    }
    match pane {
        Pane::Project => rgba(236, 250, 243, 0.86),
        Pane::Agent => rgba(255, 248, 232, 0.88),
        Pane::Main => rgba(236, 244, 255, 0.90),
        Pane::Output => rgba(242, 239, 255, 0.88),
    }
}

// ── Gradient geometry ────────────────────────────────────────────────────

/// A rounded rectangle filled with a vertical gradient: a fan from its centre,
/// each vertex coloured by its height (the shape is convex, so a fan is exact).
pub fn gradient_rounded_rect(rect: Rect, radius: f32, top: Color32, bottom: Color32) -> Mesh {
    let mut outline = Vec::new();
    egui::epaint::tessellator::path::rounded_rectangle(
        &mut outline,
        rect,
        egui::epaint::CornerRadiusF32::same(radius),
    );
    let at = |y: f32| {
        let t = ((y - rect.top()) / rect.height().max(1.0)).clamp(0.0, 1.0);
        top.lerp_to_gamma(bottom, t)
    };
    let mut mesh = Mesh::default();
    mesh.colored_vertex(rect.center(), at(rect.center().y));
    for p in &outline {
        mesh.colored_vertex(*p, at(p.y));
    }
    let n = outline.len() as u32;
    for i in 0..n {
        mesh.add_triangle(0, 1 + i, 1 + (i + 1) % n);
    }
    mesh
}

/// An elliptical radial glow: `color` at `centre`, fading to nothing at the
/// ellipse's edge (`rx`, `ry`), with a soft knee half-way.
fn radial(mesh: &mut Mesh, centre: Pos2, rx: f32, ry: f32, color: Color32) {
    const N: u32 = 64;
    let base = mesh.vertices.len() as u32;
    mesh.colored_vertex(centre, color);
    let rings = [(0.5_f32, 0.45_f32), (1.0, 0.0)];
    for (scale, alpha) in rings {
        for i in 0..N {
            let a = i as f32 / N as f32 * std::f32::consts::TAU;
            let p = centre + egui::vec2(a.cos() * rx * scale, a.sin() * ry * scale);
            mesh.colored_vertex(p, color.gamma_multiply(alpha));
        }
    }
    for i in 0..N {
        let j = (i + 1) % N;
        mesh.add_triangle(base, base + 1 + i, base + 1 + j);
        let (a0, a1, b0, b1) = (base + 1 + i, base + 1 + j, base + 1 + N + i, base + 1 + N + j);
        mesh.add_triangle(a0, b0, b1);
        mesh.add_triangle(a0, b1, a1);
    }
}

/// A filled wave: the area under two joined cubic curves (in unit
/// coordinates of `rect`), coloured left to right from `left` to `right`.
fn wave(mesh: &mut Mesh, rect: Rect, segments: [[(f32, f32); 4]; 2], left: Color32, right: Color32) {
    let cubic = |s: &[(f32, f32); 4], t: f32| {
        let u = 1.0 - t;
        let x = u * u * u * s[0].0 + 3.0 * u * u * t * s[1].0 + 3.0 * u * t * t * s[2].0 + t * t * t * s[3].0;
        let y = u * u * u * s[0].1 + 3.0 * u * u * t * s[1].1 + 3.0 * u * t * t * s[2].1 + t * t * t * s[3].1;
        (x, y)
    };
    let mut points = Vec::new();
    for s in &segments {
        for k in 0..=40 {
            points.push(cubic(s, k as f32 / 40.0));
        }
    }
    let base = mesh.vertices.len() as u32;
    for (x, y) in &points {
        let c = left.lerp_to_gamma(right, *x);
        mesh.colored_vertex(Pos2::new(rect.left() + x * rect.width(), rect.top() + y * rect.height()), c);
        mesh.colored_vertex(Pos2::new(rect.left() + x * rect.width(), rect.bottom()), c);
    }
    for i in 0..(points.len() as u32 - 1) {
        let (a, b, c, d) = (base + 2 * i, base + 2 * i + 1, base + 2 * i + 2, base + 2 * i + 3);
        mesh.add_triangle(a, b, c);
        mesh.add_triangle(b, d, c);
    }
}

/// The window backdrop, under every pane: a soft three-stop wash, three large
/// radial glows (sky, mint, lavender), two waves along the bottom and two
/// faint discs. Proportional to the window, so it reads the same at any size.
pub fn paint_backdrop(painter: &egui::Painter, rect: Rect) {
    let mut mesh = Mesh::default();
    // 1. The base wash, top to bottom.
    let stops = [
        (0.0, Color32::from_rgb(233, 238, 246)),
        (0.5, Color32::from_rgb(227, 234, 242)),
        (1.0, Color32::from_rgb(230, 236, 238)),
    ];
    for w in stops.windows(2) {
        let (t0, c0) = w[0];
        let (t1, c1) = w[1];
        let y0 = rect.top() + t0 * rect.height();
        let y1 = rect.top() + t1 * rect.height();
        let base = mesh.vertices.len() as u32;
        mesh.colored_vertex(Pos2::new(rect.left(), y0), c0);
        mesh.colored_vertex(Pos2::new(rect.right(), y0), c0);
        mesh.colored_vertex(Pos2::new(rect.left(), y1), c1);
        mesh.colored_vertex(Pos2::new(rect.right(), y1), c1);
        mesh.add_triangle(base, base + 1, base + 2);
        mesh.add_triangle(base + 1, base + 3, base + 2);
    }
    let (w, h) = (rect.width(), rect.height());
    let at = |fx: f32, fy: f32| Pos2::new(rect.left() + fx * w, rect.top() + fy * h);
    // 2. The three glows (the mockup's 1200x600, 900x700 and 1000x700 ellipses
    //    at a 1440x900 window, fading out at 60 % of their size).
    radial(&mut mesh, at(0.08, -0.10), 0.50 * w, 0.40 * h, Color32::from_rgb(191, 220, 255));
    radial(&mut mesh, at(1.05, 0.10), 0.375 * w, 0.47 * h, Color32::from_rgb(198, 241, 221));
    radial(&mut mesh, at(0.60, 1.20), 0.42 * w, 0.47 * h, Color32::from_rgb(220, 214, 247));
    // 3. Two waves along the bottom.
    wave(
        &mut mesh,
        rect,
        [
            [(0.0, 0.711), (0.181, 0.622), (0.292, 0.844), (0.5, 0.767)],
            [(0.5, 0.767), (0.708, 0.689), (0.819, 0.622), (1.0, 0.722)],
        ],
        rgba(91, 155, 240, 0.20),
        rgba(47, 179, 138, 0.16),
    );
    wave(
        &mut mesh,
        rect,
        [
            [(0.0, 0.800), (0.208, 0.756), (0.361, 0.933), (0.597, 0.856)],
            [(0.597, 0.856), (0.833, 0.778), (0.875, 0.778), (1.0, 0.844)],
        ],
        rgba(168, 150, 255, 0.16),
        rgba(91, 155, 240, 0.10),
    );
    painter.add(Shape::mesh(mesh));
    // 4. Two faint discs.
    painter.circle_filled(at(0.924, 0.167), 0.104 * w, rgba(143, 227, 192, 0.14));
    painter.circle_filled(at(0.132, 0.089), 0.083 * w, rgba(127, 182, 255, 0.14));
}

// ── Pane chrome ──────────────────────────────────────────────────────────

/// The four corner glows: top-left sky, top-right mint, bottom-right
/// lavender, bottom-left peach.
const CORNERS: [(u8, u8, u8, f32); 4] = [
    (96, 190, 255, 0.95),
    (88, 214, 160, 0.95),
    (168, 150, 255, 0.90),
    (255, 170, 140, 0.90),
];

/// How far along each edge a corner's glow reaches before it is gone.
const GLOW_REACH: f32 = 46.0;

/// The shadow under every pane card: soft, slightly blue.
pub fn pane_shadow() -> Shadow {
    Shadow {
        offset: [0, 8],
        blur: 22,
        spread: 0,
        color: Color32::from_rgba_unmultiplied(28, 56, 110, 64),
    }
}

/// The border colour along a pane's edges, where no corner glows.
pub fn pane_edge() -> Color32 {
    rgba(120, 140, 175, 0.28)
}

/// Paint `card`'s corner glow and its inner top highlight. The edges keep the
/// frame's own discreet stroke; within [`GLOW_REACH`] of each corner the border
/// brightens into that corner's colour.
pub fn paint_corner_glow(painter: &egui::Painter, card: Rect, radius: f32) {
    let mut outline = Vec::new();
    egui::epaint::tessellator::path::rounded_rectangle(
        &mut outline,
        card.shrink(0.75),
        egui::epaint::CornerRadiusF32::same(radius),
    );
    let corners = [card.left_top(), card.right_top(), card.right_bottom(), card.left_bottom()];
    let n = outline.len();
    for i in 0..n {
        let (a, b) = (outline[i], outline[(i + 1) % n]);
        // Long straight edges are split so the fade is smooth.
        let steps = ((a.distance(b) / 3.0).ceil() as usize).max(1);
        for s in 0..steps {
            let p0 = a.lerp(b, s as f32 / steps as f32);
            let p1 = a.lerp(b, (s + 1) as f32 / steps as f32);
            let mid = p0.lerp(p1, 0.5);
            let mut best = (0.0_f32, Color32::TRANSPARENT);
            for (k, c) in corners.iter().enumerate() {
                let w = (1.0 - mid.distance(*c) / (GLOW_REACH + radius)).max(0.0);
                if w > best.0 {
                    let (r, g, bl, alpha) = CORNERS[k];
                    best = (w, rgba(r, g, bl, alpha * w));
                }
            }
            if best.0 > 0.0 {
                painter.line_segment([p0, p1], Stroke::new(1.6, best.1));
            }
        }
    }
    // The discrete 3D lift: a lit top edge.
    let y = card.top() + 1.0;
    painter.line_segment(
        [Pos2::new(card.left() + radius, y), Pos2::new(card.right() - radius, y)],
        Stroke::new(1.0, rgba(255, 255, 255, 0.85)),
    );
}

/// Call first thing inside a pane frame's content closure: paints the pane's
/// corner glow on its border. The content is inset by the frame's inner
/// margin, so it never covers the border this paints on. No-op unless Aurora
/// Pastel is active.
pub fn glow_card(ui: &Ui) {
    if !active() {
        return;
    }
    let card = ui.max_rect().expand(crate::theme::PANE_INNER_MARGIN as f32);
    let painter = ui.ctx().layer_painter(ui.layer_id());
    paint_corner_glow(&painter, card, PANE_RADIUS as f32);
}

/// [`glow_card`] from outside: `panel` is the response of a panel whose frame
/// is `glass_panel_frame` (its rect includes the frame's 6 px outer margin).
pub fn glow_panel(panel: &Response) {
    if !active() {
        return;
    }
    let painter = panel.ctx.layer_painter(panel.layer_id);
    paint_corner_glow(&painter, panel.rect.shrink(6.0), PANE_RADIUS as f32);
}

// ── Toolbar pills ─────────────────────────────────────────────────────────

/// A toolbar button's look under Aurora Pastel: its gradient and its ink.
#[derive(Clone, Copy, Debug)]
pub struct Pill {
    pub top: Color32,
    pub bottom: Color32,
    pub ink: Color32,
}

impl Pill {
    pub const OPEN: Pill = Pill { top: Color32::from_rgb(255, 255, 255), bottom: Color32::from_rgb(227, 233, 243), ink: Color32::from_rgb(27, 42, 74) };
    pub const CHECK: Pill = Pill { top: Color32::from_rgb(221, 246, 234), bottom: Color32::from_rgb(183, 235, 211), ink: Color32::from_rgb(11, 74, 53) };
    pub const SEARCH: Pill = Pill { top: Color32::from_rgb(227, 238, 255), bottom: Color32::from_rgb(199, 218, 250), ink: Color32::from_rgb(23, 62, 134) };
    pub const BUILD: Pill = Pill { top: Color32::from_rgb(236, 230, 255), bottom: Color32::from_rgb(207, 196, 247), ink: Color32::from_rgb(43, 33, 96) };
    pub const RUN: Pill = Pill { top: Color32::from_rgb(19, 116, 86), bottom: Color32::from_rgb(14, 90, 66), ink: Color32::from_rgb(255, 255, 255) };
    pub const DEBUG: Pill = Pill { top: Color32::from_rgb(255, 241, 201), bottom: Color32::from_rgb(255, 217, 138), ink: Color32::from_rgb(90, 58, 18) };
    pub const STOP: Pill = Pill { top: Color32::from_rgb(255, 227, 234), bottom: Color32::from_rgb(249, 185, 201), ink: Color32::from_rgb(91, 36, 51) };
    pub const DISABLED: Pill = Pill { top: Color32::from_rgb(244, 246, 250), bottom: Color32::from_rgb(227, 232, 240), ink: Color32::from_rgb(79, 92, 118) };
}

/// A toolbar button drawn as a raised gradient pill: a soft shadow under it,
/// the gradient, a lit top edge and a shaded bottom edge. The label keeps the
/// widget's own text and behaviour; only the face is painted.
pub fn pill(ui: &mut Ui, enabled: bool, label: &str, look: Pill) -> Response {
    let look = if enabled { look } else { Pill::DISABLED };
    let slot = ui.painter().add(Shape::Noop);
    let resp = ui.add_enabled(
        enabled,
        egui::Button::new(egui::RichText::new(label).color(look.ink))
            .fill(Color32::TRANSPARENT)
            .stroke(Stroke::NONE)
            .corner_radius(CornerRadius::same(10)),
    );
    let r = resp.rect;
    let radius = 10.0;
    let (top, bottom) = if enabled && resp.hovered() {
        (look.top.lerp_to_gamma(Color32::WHITE, 0.25), look.bottom.lerp_to_gamma(Color32::WHITE, 0.15))
    } else {
        (look.top, look.bottom)
    };
    let mut shapes = Vec::new();
    if enabled {
        shapes.push(Shape::from(
            Shadow {
                offset: [0, 3],
                blur: 8,
                spread: 0,
                color: Color32::from_rgba_unmultiplied(28, 56, 110, 70),
            }
            .as_shape(r, CornerRadius::same(10)),
        ));
    }
    shapes.push(Shape::mesh(gradient_rounded_rect(r, radius, top, bottom)));
    shapes.push(Shape::line_segment(
        [Pos2::new(r.left() + radius, r.top() + 1.0), Pos2::new(r.right() - radius, r.top() + 1.0)],
        Stroke::new(1.0, rgba(255, 255, 255, 0.7)),
    ));
    shapes.push(Shape::line_segment(
        [Pos2::new(r.left() + radius, r.bottom() - 1.0), Pos2::new(r.right() - radius, r.bottom() - 1.0)],
        Stroke::new(2.0, Color32::from_rgba_unmultiplied(0, 0, 0, 26)),
    ));
    ui.painter().set(slot, Shape::Vec(shapes));
    resp
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::contrast::contrast_ratio;

    /// Every pill's ink reads on both ends of its own gradient: 4.5:1 for
    /// white on a saturated face (Run), 7:1 for dark ink on a pastel one.
    #[test]
    fn every_pill_ink_reads_on_its_gradient() {
        for (name, p) in [
            ("open", Pill::OPEN),
            ("check", Pill::CHECK),
            ("search", Pill::SEARCH),
            ("build", Pill::BUILD),
            ("run", Pill::RUN),
            ("debug", Pill::DEBUG),
            ("stop", Pill::STOP),
            ("disabled", Pill::DISABLED),
        ] {
            let floor = if name == "run" { 4.5 } else if name == "disabled" { 4.5 } else { 7.0 };
            for face in [p.top, p.bottom] {
                let r = contrast_ratio(p.ink, face);
                assert!(r >= floor, "{name}: {r:.2}:1 on {face:?}");
            }
        }
    }

    /// Rendered in a real frame: the backdrop, a pane's glow and a pill all
    /// produce shapes, and a disabled pill is not clickable.
    #[test]
    fn aurora_surfaces_paint_in_a_frame() {
        crate::theme::set_active(&crate::theme::AURORA_PASTEL);
        assert!(active());
        let ctx = egui::Context::default();
        let input = egui::RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, egui::vec2(1440.0, 900.0))),
            ..Default::default()
        };
        let mut clicked = false;
        let mut out = ctx.run_ui(input, |ui| {
            paint_backdrop(&ui.ctx().layer_painter(egui::LayerId::background()), ui.ctx().content_rect());
            egui::Frame::NONE.inner_margin(egui::Margin::same(crate::theme::PANE_INNER_MARGIN)).show(ui, |ui| {
                glow_card(ui);
                clicked |= pill(ui, false, "Save", Pill::OPEN).clicked();
                let _ = pill(ui, true, "Run", Pill::RUN);
            });
        });
        out.textures_delta.clear();
        let shapes = out.shapes.len();
        assert!(shapes >= 6, "backdrop, glow and pills painted: {shapes} shapes");
        assert!(!clicked);
        crate::theme::set_active(crate::theme::default_theme());
    }

    /// The pane tints are pastel and translucent, so the backdrop shows
    /// through and dark text reads on them.
    #[test]
    fn pane_text_reads_on_every_tint() {
        let theme = crate::theme::AURORA_PASTEL;
        for (r, g, b) in [(236, 250, 243), (255, 248, 232), (236, 244, 255), (242, 239, 255)] {
            let tint = Color32::from_rgb(r, g, b);
            assert!(contrast_ratio(theme.text_bright, tint) >= 7.0);
            assert!(contrast_ratio(theme.text_dim, tint) >= 7.0);
        }
    }
}
