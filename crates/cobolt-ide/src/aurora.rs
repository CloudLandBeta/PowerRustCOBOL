// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! The Aurora IDE themes' painted surfaces: the multi-gradient window
//! backdrop, one tint per pane, the corner-glow pane border, the discrete 3D
//! toolbar pills, the inspector's tabs and section bands. Two palettes share
//! them: Aurora Pastel (`PASTEL`) and Aurora Prime (`PRIME`, the same surfaces
//! in live secondary and tertiary colours: no primaries). Their flat palettes
//! are `theme::AURORA_PASTEL` and `theme::AURORA_PRIME`; everything here is a
//! no-op under any other theme.
//!
//! The structure of the IDE is untouched: every pane keeps its place and
//! behaviour, only the way it is painted changes (operator, 2026-10-02, after
//! the mockup "IDE — Aurora Pastel").

use egui::epaint::{Mesh, Shadow};
use egui::{Color32, CornerRadius, Pos2, Rect, Response, Shape, Stroke, Ui};

/// Every colour an Aurora theme paints beyond its flat `Theme` palette.
pub struct Palette {
    /// The backdrop's base wash, top, middle and bottom.
    pub wash: [Color32; 3],
    /// The three radial glows: top-left, right, bottom.
    pub glows: [Color32; 3],
    /// The two bottom waves, each left to right.
    pub waves: [(Color32, Color32); 2],
    /// The two faint discs.
    pub discs: [Color32; 2],
    /// Pane fills: project tree, agent bar, main pane, Output.
    pub panes: [Color32; 4],
    /// Corner glows: top-left, top-right, bottom-right, bottom-left.
    pub corners: [Color32; 4],
    pub pills: Pills,
    /// Idle inspector tabs: Props, Events, Procs, Anim.
    pub tabs: [Color32; 4],
    /// Section header band and its title ink.
    pub band: Color32,
    pub band_ink: Color32,
    /// The selected inspector tab's fill and text.
    pub tab_selected: Color32,
    pub tab_selected_ink: Color32,
    /// The colour of the shadow under every pane card.
    pub shadow: Color32,
    /// The discreet border along a pane's edges, where no corner glows.
    pub edge: Color32,
    /// Pane card corner radius, and toolbar pill radius (a pill radius of
    /// 18 or more draws a full capsule).
    pub radius: u8,
    pub pill_radius: u8,
    /// `Some(tint)`: the window is see-through. Nothing opaque is painted
    /// behind the panes; the operating system blurs the desktop under the
    /// window and this tint is laid over it, dark enough that the panes'
    /// text stays readable over a bright wallpaper. `None`: the painted
    /// backdrop.
    pub see_through: Option<Color32>,
}

/// The toolbar's pill per button.
pub struct Pills {
    pub open: Pill,
    pub check: Pill,
    pub search: Pill,
    pub build: Pill,
    pub run: Pill,
    pub debug: Pill,
    pub stop: Pill,
    /// Every disabled button.
    pub disabled: Pill,
}

const fn rgb(r: u8, g: u8, b: u8) -> Color32 {
    Color32::from_rgb(r, g, b)
}

/// `rgb` at `a` (0-255), premultiplied so it can be a `const`.
const fn rgba8(r: u8, g: u8, b: u8, a: u8) -> Color32 {
    Color32::from_rgba_premultiplied(
        ((r as u16 * a as u16) / 255) as u8,
        ((g as u16 * a as u16) / 255) as u8,
        ((b as u16 * a as u16) / 255) as u8,
        a,
    )
}

const fn pill_of(top: Color32, bottom: Color32, ink: Color32) -> Pill {
    Pill { top, bottom, ink }
}

/// Aurora Pastel: blue, grey and green over pastel secondaries.
pub const PASTEL: Palette = Palette {
    wash: [rgb(233, 238, 246), rgb(227, 234, 242), rgb(230, 236, 238)],
    glows: [rgb(191, 220, 255), rgb(198, 241, 221), rgb(220, 214, 247)],
    waves: [
        (rgba8(91, 155, 240, 51), rgba8(47, 179, 138, 41)),
        (rgba8(168, 150, 255, 41), rgba8(91, 155, 240, 26)),
    ],
    discs: [rgba8(143, 227, 192, 36), rgba8(127, 182, 255, 36)],
    panes: [
        rgba8(236, 250, 243, 219),
        rgba8(255, 248, 232, 224),
        rgba8(236, 244, 255, 230),
        rgba8(242, 239, 255, 224),
    ],
    corners: [
        rgba8(96, 190, 255, 242),
        rgba8(88, 214, 160, 242),
        rgba8(168, 150, 255, 230),
        rgba8(255, 170, 140, 230),
    ],
    pills: Pills {
        open: pill_of(rgb(255, 255, 255), rgb(227, 233, 243), rgb(27, 42, 74)),
        check: pill_of(rgb(221, 246, 234), rgb(183, 235, 211), rgb(11, 74, 53)),
        search: pill_of(rgb(227, 238, 255), rgb(199, 218, 250), rgb(23, 62, 134)),
        build: pill_of(rgb(236, 230, 255), rgb(207, 196, 247), rgb(43, 33, 96)),
        run: pill_of(rgb(19, 116, 86), rgb(14, 90, 66), rgb(255, 255, 255)),
        debug: pill_of(rgb(255, 241, 201), rgb(255, 217, 138), rgb(90, 58, 18)),
        stop: pill_of(rgb(255, 227, 234), rgb(249, 185, 201), rgb(91, 36, 51)),
        disabled: Pill::DISABLED,
    },
    tabs: [rgb(220, 235, 255), rgb(205, 223, 252), rgb(203, 238, 222), rgb(224, 216, 252)],
    band: rgb(220, 235, 255),
    band_ink: rgb(23, 62, 134),
    tab_selected: rgb(18, 22, 27),
    tab_selected_ink: rgb(255, 255, 255),
    shadow: rgba8(28, 56, 110, 64),
    edge: rgba8(120, 140, 175, 71),
    radius: 16,
    pill_radius: 10,
    see_through: None,
};

/// Aurora Prime: the same surfaces in live secondary and tertiary colours
/// (teal, cyan, emerald, indigo-violet, plum-magenta, burnt orange, apricot),
/// never a primary. Saturation goes to what decorates: the backdrop, the
/// glows, the pills and tabs. The pane interiors stay light enough that dark
/// text keeps 7:1.
pub const PRIME: Palette = Palette {
    wash: [rgb(226, 234, 246), rgb(219, 229, 241), rgb(222, 236, 234)],
    glows: [rgb(110, 205, 255), rgb(84, 226, 170), rgb(176, 148, 255)],
    waves: [
        (rgba8(0, 179, 164, 82), rgba8(124, 77, 255, 64)),
        (rgba8(214, 64, 160, 51), rgba8(255, 138, 61, 46)),
    ],
    discs: [rgba8(0, 201, 167, 51), rgba8(108, 140, 255, 51)],
    panes: [
        rgba8(214, 247, 233, 224),
        rgba8(255, 236, 214, 230),
        rgba8(221, 236, 255, 235),
        rgba8(236, 226, 255, 230),
    ],
    corners: [
        rgba8(0, 200, 255, 245),
        rgba8(0, 214, 143, 245),
        rgba8(124, 77, 255, 240),
        rgba8(255, 122, 61, 240),
    ],
    pills: Pills {
        open: pill_of(rgb(255, 255, 255), rgb(222, 230, 242), rgb(27, 42, 74)),
        check: pill_of(rgb(11, 122, 110), rgb(7, 94, 85), rgb(255, 255, 255)),
        search: pill_of(rgb(91, 75, 214), rgb(69, 53, 184), rgb(255, 255, 255)),
        build: pill_of(rgb(142, 63, 196), rgb(113, 48, 158), rgb(255, 255, 255)),
        run: pill_of(rgb(19, 116, 86), rgb(14, 90, 66), rgb(255, 255, 255)),
        debug: pill_of(rgb(176, 74, 8), rgb(143, 60, 6), rgb(255, 255, 255)),
        stop: pill_of(rgb(184, 37, 106), rgb(149, 29, 86), rgb(255, 255, 255)),
        disabled: Pill::DISABLED,
    },
    tabs: [rgb(203, 230, 255), rgb(201, 211, 255), rgb(189, 242, 220), rgb(226, 204, 255)],
    band: rgb(191, 227, 255),
    band_ink: rgb(15, 58, 122),
    tab_selected: rgb(18, 22, 27),
    tab_selected_ink: rgb(255, 255, 255),
    shadow: rgba8(28, 56, 110, 64),
    edge: rgba8(120, 140, 175, 71),
    radius: 16,
    pill_radius: 10,
    see_through: None,
};

/// A slick two-colour palette: white and its variants, plus `dark` (text,
/// chrome, solid Build) and the accent (`accent_ink` for text on white,
/// `accent_top`/`accent_bottom` for the solid Run pill). No glows, waves or
/// tints: the backdrop is white to light grey, the panes white cards, and
/// only the corner glows, pills, tabs and bands carry the two colours.
const fn slick(
    dark: Color32,
    dark_top: Color32,
    accent_ink: Color32,
    accent_top: Color32,
    accent_bottom: Color32,
) -> Palette {
    const CLEAR: Color32 = Color32::TRANSPARENT;
    let white = rgb(255, 255, 255);
    let off = rgb(242, 244, 247);
    Palette {
        wash: [white, rgb(246, 247, 249), rgb(238, 240, 243)],
        glows: [CLEAR, CLEAR, CLEAR],
        waves: [(CLEAR, CLEAR), (CLEAR, CLEAR)],
        discs: [CLEAR, CLEAR],
        panes: [
            rgba8(250, 251, 252, 245),
            rgba8(255, 255, 255, 248),
            rgba8(255, 255, 255, 248),
            rgba8(246, 247, 249, 245),
        ],
        corners: [
            Color32::from_rgba_premultiplied(
                (dark.r() as u16 * 217 / 255) as u8,
                (dark.g() as u16 * 217 / 255) as u8,
                (dark.b() as u16 * 217 / 255) as u8,
                217,
            ),
            accent_top,
            Color32::from_rgba_premultiplied(
                (dark.r() as u16 * 217 / 255) as u8,
                (dark.g() as u16 * 217 / 255) as u8,
                (dark.b() as u16 * 217 / 255) as u8,
                217,
            ),
            accent_top,
        ],
        pills: Pills {
            open: pill_of(white, off, dark),
            check: pill_of(white, off, accent_ink),
            search: pill_of(white, off, dark),
            build: pill_of(dark_top, dark, white),
            run: pill_of(accent_top, accent_bottom, white),
            debug: pill_of(white, off, dark),
            stop: pill_of(white, off, accent_ink),
            disabled: Pill::DISABLED,
        },
        tabs: [off, rgb(236, 239, 243), off, rgb(236, 239, 243)],
        band: rgb(236, 239, 243),
        band_ink: dark,
        tab_selected: dark,
        tab_selected_ink: white,
        shadow: rgba8(15, 20, 30, 46),
        edge: rgba8(120, 128, 140, 70),
        radius: 16,
        pill_radius: 10,
        see_through: None,
    }
}

/// Spatial: dark frosted glass in the manner of a spatial-computing UI.
/// Warm grey translucent panes over a dim room, white text, large corner
/// radii, capsule buttons, no saturated colour; the light catches the
/// panes' top corners as a white specular glow. Selection is brighter glass.
pub const SPATIAL: Palette = Palette {
    wash: [rgb(62, 60, 56), rgb(48, 46, 43), rgb(38, 37, 35)],
    glows: [rgb(96, 92, 84), rgb(84, 86, 88), rgb(90, 86, 80)],
    waves: [(Color32::TRANSPARENT, Color32::TRANSPARENT), (Color32::TRANSPARENT, Color32::TRANSPARENT)],
    discs: [Color32::TRANSPARENT, Color32::TRANSPARENT],
    // Glass at 74 %: the blurred desktop shows through. Over the worst case,
    // a white wallpaper under the see-through tint, white text still reads
    // about 7:1.
    panes: [
        rgba8(66, 65, 62, 189),
        rgba8(70, 69, 66, 189),
        rgba8(66, 65, 62, 189),
        rgba8(60, 59, 56, 196),
    ],
    corners: [
        rgba8(255, 255, 255, 140),
        rgba8(255, 255, 255, 115),
        rgba8(255, 255, 255, 31),
        rgba8(255, 255, 255, 46),
    ],
    pills: Pills {
        open: pill_of(rgb(110, 109, 105), rgb(92, 91, 88), rgb(255, 255, 255)),
        check: pill_of(rgb(110, 109, 105), rgb(92, 91, 88), rgb(255, 255, 255)),
        search: pill_of(rgb(110, 109, 105), rgb(92, 91, 88), rgb(255, 255, 255)),
        build: pill_of(rgb(110, 109, 105), rgb(92, 91, 88), rgb(255, 255, 255)),
        run: pill_of(rgb(245, 244, 240), rgb(218, 216, 210), rgb(28, 28, 30)),
        debug: pill_of(rgb(110, 109, 105), rgb(92, 91, 88), rgb(255, 255, 255)),
        stop: pill_of(rgb(110, 109, 105), rgb(92, 91, 88), rgb(255, 255, 255)),
        disabled: pill_of(rgb(64, 63, 60), rgb(56, 55, 52), rgb(186, 184, 179)),
    },
    tabs: [rgb(72, 71, 68), rgb(72, 71, 68), rgb(72, 71, 68), rgb(72, 71, 68)],
    band: rgb(62, 61, 58),
    band_ink: rgb(255, 255, 255),
    tab_selected: rgb(232, 231, 226),
    tab_selected_ink: rgb(28, 28, 30),
    shadow: rgba8(0, 0, 0, 110),
    edge: rgba8(255, 255, 255, 36),
    radius: 26,
    pill_radius: 18,
    see_through: Some(rgba8(28, 27, 25, 128)),
};

/// Slick: deep navy and teal.
pub const SLICK_NAVY_TEAL: Palette =
    slick(rgb(18, 48, 92), rgb(65, 89, 125), rgb(10, 92, 92), rgb(14, 130, 130), rgb(11, 107, 107));
/// Slick: graphite and electric indigo.
pub const SLICK_GRAPHITE_INDIGO: Palette =
    slick(rgb(30, 36, 48), rgb(84, 89, 98), rgb(68, 60, 197), rgb(79, 70, 229), rgb(65, 57, 188));
/// Slick: slate blue and coral.
pub const SLICK_SLATE_CORAL: Palette =
    slick(rgb(46, 74, 125), rgb(63, 88, 135), rgb(141, 54, 29), rgb(198, 76, 40), rgb(162, 62, 33));
/// Slick: charcoal and emerald.
pub const SLICK_CHARCOAL_EMERALD: Palette =
    slick(rgb(35, 39, 47), rgb(86, 89, 95), rgb(9, 94, 73), rgb(13, 133, 102), rgb(11, 109, 84));
/// Slick: piano black and deep coral.
pub const SLICK_PIANO_CORAL: Palette =
    slick(rgb(11, 11, 13), rgb(84, 84, 86), rgb(139, 56, 38), rgb(195, 79, 54), rgb(160, 65, 44));

/// The active Aurora palette, or `None` under any other theme.
pub fn palette() -> Option<&'static Palette> {
    palette_for(crate::theme::active().id)
}

/// The Aurora palette of the theme `id`, or `None` for a theme that is not
/// in the family.
pub fn palette_for(id: &str) -> Option<&'static Palette> {
    match id {
        "aurora-pastel" => Some(&PASTEL),
        "aurora-prime" => Some(&PRIME),
        "slick-navy-teal" => Some(&SLICK_NAVY_TEAL),
        "slick-graphite-indigo" => Some(&SLICK_GRAPHITE_INDIGO),
        "slick-slate-coral" => Some(&SLICK_SLATE_CORAL),
        "slick-charcoal-emerald" => Some(&SLICK_CHARCOAL_EMERALD),
        "slick-piano-coral" => Some(&SLICK_PIANO_CORAL),
        "spatial" => Some(&SPATIAL),
        _ => None,
    }
}

/// Whether the active IDE theme is one of the Aurora themes.
pub fn active() -> bool {
    palette().is_some()
}

/// The active palette's toolbar pills (Pastel's when no Aurora theme is
/// active, which the toolbar never asks for).
pub fn pills() -> &'static Pills {
    &palette().unwrap_or(&PASTEL).pills
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

/// The pane's fill under an Aurora theme, else `default` unchanged.
pub fn pane_fill(pane: Pane, default: Color32) -> Color32 {
    let Some(p) = palette() else {
        return default;
    };
    match pane {
        Pane::Project => p.panes[0],
        Pane::Agent => p.panes[1],
        Pane::Main => p.panes[2],
        Pane::Output => p.panes[3],
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
    let pal = palette().unwrap_or(&PASTEL);
    let mut mesh = Mesh::default();
    // 1. The base wash, top to bottom.
    let stops = [(0.0, pal.wash[0]), (0.5, pal.wash[1]), (1.0, pal.wash[2])];
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
    radial(&mut mesh, at(0.08, -0.10), 0.50 * w, 0.40 * h, pal.glows[0]);
    radial(&mut mesh, at(1.05, 0.10), 0.375 * w, 0.47 * h, pal.glows[1]);
    radial(&mut mesh, at(0.60, 1.20), 0.42 * w, 0.47 * h, pal.glows[2]);
    // 3. Two waves along the bottom.
    wave(
        &mut mesh,
        rect,
        [
            [(0.0, 0.711), (0.181, 0.622), (0.292, 0.844), (0.5, 0.767)],
            [(0.5, 0.767), (0.708, 0.689), (0.819, 0.622), (1.0, 0.722)],
        ],
        pal.waves[0].0,
        pal.waves[0].1,
    );
    wave(
        &mut mesh,
        rect,
        [
            [(0.0, 0.800), (0.208, 0.756), (0.361, 0.933), (0.597, 0.856)],
            [(0.597, 0.856), (0.833, 0.778), (0.875, 0.778), (1.0, 0.844)],
        ],
        pal.waves[1].0,
        pal.waves[1].1,
    );
    painter.add(Shape::mesh(mesh));
    // 4. Two faint discs.
    painter.circle_filled(at(0.924, 0.167), 0.104 * w, pal.discs[0]);
    painter.circle_filled(at(0.132, 0.089), 0.083 * w, pal.discs[1]);
}

// ── Pane chrome ──────────────────────────────────────────────────────────

/// How far along each edge a corner's glow reaches before it is gone.
const GLOW_REACH: f32 = 46.0;

/// The shadow under every pane card: soft, slightly blue.
pub fn pane_shadow() -> Shadow {
    Shadow {
        offset: [0, 8],
        blur: 22,
        spread: 0,
        color: palette().unwrap_or(&PASTEL).shadow,
    }
}

/// The border colour along a pane's edges, where no corner glows.
pub fn pane_edge() -> Color32 {
    palette().unwrap_or(&PASTEL).edge
}

/// The see-through tint when the active theme wants the desktop to show,
/// blurred, behind the window (Spatial); `None` for a painted backdrop.
pub fn see_through() -> Option<Color32> {
    palette().and_then(|p| p.see_through)
}

/// The active palette's pane corner radius.
pub fn pane_radius() -> u8 {
    palette().unwrap_or(&PASTEL).radius
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
    let glow = palette().unwrap_or(&PASTEL).corners;
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
                    best = (w, glow[k].gamma_multiply(w));
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
    paint_corner_glow(&painter, card, pane_radius() as f32);
}

/// [`glow_card`] from outside: `panel` is the response of a panel whose frame
/// is `glass_panel_frame` (its rect includes the frame's 6 px outer margin).
pub fn glow_panel(panel: &Response) {
    if !active() {
        return;
    }
    let painter = panel.ctx.layer_painter(panel.layer_id);
    paint_corner_glow(&painter, panel.rect.shrink(6.0), pane_radius() as f32);
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
    /// Every disabled button, in both palettes.
    pub const DISABLED: Pill = Pill { top: Color32::from_rgb(244, 246, 250), bottom: Color32::from_rgb(227, 232, 240), ink: Color32::from_rgb(79, 92, 118) };
}

/// A toolbar button drawn as a raised gradient pill: a soft shadow under it,
/// the gradient, a lit top edge and a shaded bottom edge. The label keeps the
/// widget's own text and behaviour; only the face is painted.
pub fn pill(ui: &mut Ui, enabled: bool, label: &str, look: Pill) -> Response {
    let pal = palette().unwrap_or(&PASTEL);
    let look = if enabled { look } else { pal.pills.disabled };
    let pill_r = pal.pill_radius;
    let slot = ui.painter().add(Shape::Noop);
    let resp = ui.add_enabled(
        enabled,
        egui::Button::new(egui::RichText::new(label).color(look.ink))
            .fill(Color32::TRANSPARENT)
            .stroke(Stroke::NONE)
            .corner_radius(CornerRadius::same(pill_r)),
    );
    let r = resp.rect;
    // A radius of 18 or more means a capsule: round ends whatever the height.
    let radius = if pill_r >= 18 { r.height() / 2.0 } else { pill_r as f32 };
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
            .as_shape(r, CornerRadius::same(radius.round().min(255.0) as u8)),
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
        for (theme, pal) in [
            ("pastel", &PASTEL),
            ("prime", &PRIME),
            ("navy-teal", &SLICK_NAVY_TEAL),
            ("graphite-indigo", &SLICK_GRAPHITE_INDIGO),
            ("slate-coral", &SLICK_SLATE_CORAL),
            ("charcoal-emerald", &SLICK_CHARCOAL_EMERALD),
            ("piano-coral", &SLICK_PIANO_CORAL),
            ("spatial", &SPATIAL),
        ] {
            let ps = &pal.pills;
            for (name, p) in [
                ("open", ps.open),
                ("check", ps.check),
                ("search", ps.search),
                ("build", ps.build),
                ("run", ps.run),
                ("debug", ps.debug),
                ("stop", ps.stop),
                ("disabled", ps.disabled),
            ] {
                // White on a saturated face 4.5:1; dark ink on a pastel 7:1.
                let floor = if p.ink == Color32::WHITE || name == "disabled" { 4.5 } else { 7.0 };
                for face in [p.top, p.bottom] {
                    let r = contrast_ratio(p.ink, face);
                    assert!(r >= floor, "{theme} {name}: {r:.2}:1 on {face:?}");
                }
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
                clicked |= pill(ui, false, "Save", pills().open).clicked();
                let _ = pill(ui, true, "Run", pills().run);
            });
        });
        out.textures_delta.clear();
        let shapes = out.shapes.len();
        assert!(shapes >= 6, "backdrop, glow and pills painted: {shapes} shapes");
        assert!(!clicked);
        crate::theme::set_active(crate::theme::default_theme());
    }

    /// Spatial is see-through: over the worst desktop, pure white, the
    /// blur tint and then each glass pane composite to a grey that white
    /// text still reads on at 7:1.
    #[test]
    fn spatial_text_reads_over_a_white_desktop() {
        let over = |top: Color32, under: Color32| {
            let a = top.a() as f32 / 255.0;
            let ch = |t: u8, u: u8| (t as f32 + u as f32 * (1.0 - a)).round().min(255.0) as u8;
            Color32::from_rgb(ch(top.r(), under.r()), ch(top.g(), under.g()), ch(top.b(), under.b()))
        };
        let tint = SPATIAL.see_through.expect("Spatial is see-through");
        let room = over(tint, Color32::WHITE);
        for pane in SPATIAL.panes {
            let seen = over(pane, room);
            let r = contrast_ratio(Color32::WHITE, seen);
            assert!(r >= 7.0, "white on {seen:?}: {r:.2}:1");
        }
        assert!(PASTEL.see_through.is_none() && SLICK_NAVY_TEAL.see_through.is_none());
    }

    /// The pane tints are pastel and translucent, so the backdrop shows
    /// through and dark text reads on them.
    #[test]
    fn pane_text_reads_on_every_tint() {
        for (theme, pal) in [
            (crate::theme::AURORA_PASTEL, &PASTEL),
            (crate::theme::AURORA_PRIME, &PRIME),
            (crate::theme::SLICK_NAVY_TEAL, &SLICK_NAVY_TEAL),
            (crate::theme::SLICK_GRAPHITE_INDIGO, &SLICK_GRAPHITE_INDIGO),
            (crate::theme::SLICK_SLATE_CORAL, &SLICK_SLATE_CORAL),
            (crate::theme::SLICK_CHARCOAL_EMERALD, &SLICK_CHARCOAL_EMERALD),
            (crate::theme::SLICK_PIANO_CORAL, &SLICK_PIANO_CORAL),
            (crate::theme::SPATIAL, &SPATIAL),
        ] {
            // Judged on the tint's opaque colour: what dark text sits on.
            let opaque = |c: Color32| {
                let a = c.a() as f32 / 255.0;
                let un = |v: u8| ((v as f32 / a).round().min(255.0)) as u8;
                Color32::from_rgb(un(c.r()), un(c.g()), un(c.b()))
            };
            for tint in pal.panes.iter().chain(pal.tabs.iter()).chain([&pal.band]) {
                let t = opaque(*tint);
                assert!(contrast_ratio(theme.text_bright, t) >= 7.0, "{}: {t:?}", theme.id);
                assert!(contrast_ratio(theme.text_dim, t) >= 6.5, "{}: {t:?}", theme.id);
            }
            assert!(contrast_ratio(pal.band_ink, opaque(pal.band)) >= 7.0, "{} band", theme.id);
            assert!(contrast_ratio(pal.tab_selected_ink, pal.tab_selected) >= 7.0, "{} selected tab", theme.id);
            assert!(crate::aurora::palette_for(theme.id).is_some(), "{} has a palette", theme.id);
        }
    }
}
