// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! The Splitter: a themed panel divided into two panes by a draggable line.
//!
//! # What a Splitter is
//!
//! One control that owns three things:
//!
//! * **pane 1** and **pane 2** — real `Panel` controls, children of the
//!   splitter, borderless and transparent by default. They are ordinary
//!   containers: selectable, styleable, and drop targets. What the developer
//!   does NOT own is where they sit — the division decides that, so their rects
//!   are derived here every frame rather than dragged around.
//! * the **division line**, always visible, carrying a **grip** the developer
//!   drags to redistribute the room between the two panes.
//!
//! `Orientation` names how the PANES are arranged, not how the line runs:
//!
//! | Orientation  | pane 1 | pane 2 | the line |
//! |--------------|--------|--------|----------|
//! | `Horizontal` | left   | right  | vertical |
//! | `Vertical`   | top    | bottom | horizontal |
//!
//! # Why the geometry lives here
//!
//! Three surfaces have to agree about where the line is: the designer canvas
//! (which paints it and lets the developer drag it), the running form (same),
//! and the model (which pins the two pane Panels). A splitter whose line is
//! computed three times is a splitter that eventually draws in one place and
//! drags in another, so it is computed ONCE — [`geometry`] — and everything
//! else reads that.

use crate::model::{Control, ControlType, Rect};

/// Marks a Panel a Splitter owns as one of its two panes: `1` or `2`.
///
/// A property rather than an id convention, so the pane survives a rename.
pub const PANE_PROP: &str = "IsSplitterPane";

/// Where the division sits when the developer has not moved it: the middle.
pub const DEFAULT_SPLIT_PERCENT: i32 = 50;

/// The division line's drawn thickness, in points, when `LineSize` is unset.
pub const DEFAULT_LINE_SIZE: i32 = 2;

/// The grip's extent ALONG the line (pill length / circle diameter), in points.
pub const DEFAULT_GRIP_SIZE: i32 = 28;

/// How far either side of the line the pointer still counts as "on it", in
/// points. The line itself is 2pt by default and nobody can hit 2pt reliably.
pub const GRAB_TOLERANCE: i32 = 4;

/// Double-clicking the grip puts the division back here.
pub const CENTRE_PERCENT: i32 = 50;

/// How the grip is drawn on the division line.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GripStyle {
    /// An outlined lozenge, elongated along the line.
    HollowPill,
    /// A solid lozenge, elongated along the line.
    FilledPill,
    /// An outlined circle.
    HollowCircle,
    /// A solid circle.
    FilledCircle,
}

impl GripStyle {
    /// The four styles, in the order the inspector offers them.
    pub const ALL: [&'static str; 4] = ["FilledPill", "HollowPill", "FilledCircle", "HollowCircle"];

    /// Parse a `GripStyle` property value. Anything unrecognised — including
    /// the empty string a form saved before the property existed carries — is
    /// the default, so an old `.cfrm` opens with a visible grip.
    pub fn parse(s: &str) -> Self {
        match s.trim().to_ascii_lowercase().replace([' ', '-', '_'], "").as_str() {
            "hollowpill" | "outlinepill" => GripStyle::HollowPill,
            "hollowcircle" | "outlinecircle" => GripStyle::HollowCircle,
            "filledcircle" | "circle" => GripStyle::FilledCircle,
            _ => GripStyle::FilledPill,
        }
    }

    /// `true` when the grip is drawn as a round dot rather than a lozenge.
    pub fn is_circle(self) -> bool {
        matches!(self, GripStyle::HollowCircle | GripStyle::FilledCircle)
    }

    /// `true` when only the outline is drawn.
    pub fn is_hollow(self) -> bool {
        matches!(self, GripStyle::HollowPill | GripStyle::HollowCircle)
    }
}

/// The id of the Panel a Splitter owns as pane `n` (1 or 2).
pub fn pane_id(splitter_id: &str, pane: u8) -> String {
    format!("{splitter_id}-Pane{pane}")
}

/// `true` when the panes sit SIDE BY SIDE (pane 1 left, pane 2 right) and the
/// division line therefore runs vertically.
///
/// `Orientation` describes the pane arrangement, which is the way a developer
/// reads it: "a horizontal splitter" is two panes across the form.
pub fn is_horizontal(ctrl: &Control) -> bool {
    !ctrl
        .get_prop("Orientation")
        .map(|v| v.as_str().trim().to_ascii_uppercase())
        .unwrap_or_default()
        .starts_with('V')
}

/// Where the division sits, as a percentage of the splitter's inner width
/// (Horizontal) or height (Vertical). Always within 0–100: the extremes are
/// legal positions — one pane closed, the other holding everything.
pub fn split_percent(ctrl: &Control) -> i32 {
    ctrl.get_prop("SplitPosition")
        .map(|v| v.as_i64() as i32)
        .unwrap_or(DEFAULT_SPLIT_PERCENT)
        .clamp(0, 100)
}

/// The division line's drawn thickness in points (at least 1: a line nobody can
/// see is a splitter nobody can find).
pub fn line_size(ctrl: &Control) -> i32 {
    ctrl.get_prop("LineSize")
        .map(|v| v.as_i64() as i32)
        .unwrap_or(DEFAULT_LINE_SIZE)
        .clamp(1, 40)
}

/// The grip's extent along the line, in points. `0` is honoured — a splitter
/// with no grip is still draggable by its line.
pub fn grip_size(ctrl: &Control) -> i32 {
    ctrl.get_prop("GripSize")
        .map(|v| v.as_i64() as i32)
        .unwrap_or(DEFAULT_GRIP_SIZE)
        .clamp(0, 400)
}

/// The grip's thickness ACROSS the line, derived from its length so the two
/// stay in proportion however big the developer makes it. Circles are round, so
/// their thickness is their diameter.
pub fn grip_thickness(ctrl: &Control) -> i32 {
    let size = grip_size(ctrl);
    if size == 0 {
        return 0;
    }
    if GripStyle::parse(sv(ctrl, "GripStyle")).is_circle() {
        size
    } else {
        (size / 3).clamp(6, 24).max(line_size(ctrl) + 2)
    }
}

/// Everything the three surfaces need to draw and hit-test a splitter, in the
/// same coordinate space as the `rect` handed in.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Geometry {
    /// Pane 1: left (Horizontal) or top (Vertical).
    pub pane1: Rect,
    /// Pane 2: right (Horizontal) or bottom (Vertical).
    pub pane2: Rect,
    /// The division line itself, at its drawn thickness.
    pub line: Rect,
    /// The grip, centred on the line. May reach outside `rect` at 0 % / 100 %,
    /// where the splitter's own edge clips it to the half that is inside.
    pub grip: Rect,
    /// Line and grip together, widened by [`GRAB_TOLERANCE`]: what the pointer
    /// has to be inside for the cursor to become a hand.
    pub band: Rect,
    /// `true` when the panes are side by side.
    pub horizontal: bool,
}

/// The inner area a Splitter divides: its rect less the border it draws, so a
/// pane never sits on top of the splitter's own frame.
pub fn content_rect(rect: Rect) -> Rect {
    let b = 2;
    Rect::new(
        rect.x + b,
        rect.y + b,
        (rect.w - 2 * b).max(0),
        (rect.h - 2 * b).max(0),
    )
}

/// Lay a splitter out inside `rect` — the splitter's own rectangle, in whatever
/// space the caller is working in (form space for the model, screen space for a
/// painter).
pub fn geometry(ctrl: &Control, rect: Rect) -> Geometry {
    let inner = content_rect(rect);
    let horizontal = is_horizontal(ctrl);
    let thick = line_size(ctrl);
    let half = thick / 2;
    // The span the percentage divides, and where along it the line lands.
    let span = if horizontal { inner.w } else { inner.h };
    let start = if horizontal { inner.x } else { inner.y };
    let centre = start + ((span as f32) * (split_percent(ctrl) as f32) / 100.0).round() as i32;

    // Pane 1 runs from the start of the span to the near face of the line;
    // pane 2 from its far face to the end. At 0 % and 100 % one of them is
    // empty — a legal, and useful, position.
    let a_end = centre - half;
    // Clamped to the far edge so a pane closed at 100 % sits exactly ON the
    // edge rather than a hair past it: an empty rect still has a position, and
    // anything reading `pane2.x` deserves one inside the splitter.
    let b_start = (centre + (thick - half)).min(start + span);
    let a_len = (a_end - start).max(0);
    let b_len = (start + span - b_start).max(0);

    let grip_len = grip_size(ctrl).min(span.max(0));
    let grip_thick = grip_thickness(ctrl);
    // Centred on the line, halfway down (or across) the splitter.
    let cross_centre = if horizontal {
        inner.y + inner.h / 2
    } else {
        inner.x + inner.w / 2
    };
    let band_thick = (thick + 2 * GRAB_TOLERANCE).max(grip_thick);
    let band_start = centre - band_thick / 2;

    let (pane1, pane2, line, grip, band) = if horizontal {
        (
            Rect::new(start, inner.y, a_len, inner.h),
            Rect::new(b_start, inner.y, b_len, inner.h),
            Rect::new(centre - half, inner.y, thick, inner.h),
            Rect::new(
                centre - grip_thick / 2,
                cross_centre - grip_len / 2,
                grip_thick,
                grip_len,
            ),
            Rect::new(band_start, inner.y, band_thick, inner.h),
        )
    } else {
        (
            Rect::new(inner.x, start, inner.w, a_len),
            Rect::new(inner.x, b_start, inner.w, b_len),
            Rect::new(inner.x, centre - half, inner.w, thick),
            Rect::new(
                cross_centre - grip_len / 2,
                centre - grip_thick / 2,
                grip_len,
                grip_thick,
            ),
            Rect::new(inner.x, band_start, inner.w, band_thick),
        )
    };

    Geometry {
        pane1,
        pane2,
        line,
        grip,
        band,
        horizontal,
    }
}

/// The percentage a pointer at `(px, py)` puts the division at — what a drag
/// writes back to `SplitPosition`.
///
/// `rect` is the splitter's rectangle in the same space as the pointer. A
/// zero-width span cannot be divided, so it answers the current position.
pub fn percent_at(ctrl: &Control, rect: Rect, px: i32, py: i32) -> i32 {
    let inner = content_rect(rect);
    let (start, span, p) = if is_horizontal(ctrl) {
        (inner.x, inner.w, px)
    } else {
        (inner.y, inner.h, py)
    };
    if span <= 0 {
        return split_percent(ctrl);
    }
    (((p - start) as f32 / span as f32) * 100.0).round().clamp(0.0, 100.0) as i32
}

/// Is this control one of a Splitter's two panes, and which?
pub fn pane_index(ctrl: &Control) -> Option<u8> {
    if ctrl.control_type != ControlType::Panel {
        return None;
    }
    match ctrl.get_prop(PANE_PROP).map(|v| v.as_i64()) {
        Some(1) => Some(1),
        Some(2) => Some(2),
        _ => None,
    }
}

fn sv<'a>(ctrl: &'a Control, key: &str) -> &'a str {
    ctrl.get_prop(key).map(|v| v.as_str()).unwrap_or("")
}

// ── Painting ─────────────────────────────────────────────────────────────────

#[cfg(feature = "render")]
mod painting {
    use super::*;
    use egui::{Color32, Painter, Pos2, Rect as ERect, Stroke, StrokeKind, Vec2};

    /// What the pointer is doing to the splitter this frame — the grip lights
    /// up under the hand, which is how the developer discovers it is draggable.
    #[derive(Debug, Clone, Copy, Default)]
    pub struct SplitState {
        pub hovered: bool,
        pub dragging: bool,
        /// Fade applied by the container ancestry / Transparency.
        pub alpha: f32,
    }

    impl SplitState {
        pub fn still(alpha: f32) -> Self {
            Self {
                hovered: false,
                dragging: false,
                alpha,
            }
        }
    }

    /// A splitter's geometry on screen, in egui coordinates.
    #[derive(Debug, Clone, Copy, PartialEq)]
    pub struct ScreenGeometry {
        pub pane1: ERect,
        pub pane2: ERect,
        pub line: ERect,
        pub grip: ERect,
        /// What the pointer must be inside for the cursor to become a hand and
        /// a press to start moving the division.
        pub band: ERect,
        pub horizontal: bool,
    }

    fn erect(r: Rect, origin: Pos2) -> ERect {
        ERect::from_min_size(
            origin + Vec2::new(r.x as f32, r.y as f32),
            Vec2::new(r.w as f32, r.h as f32),
        )
    }

    /// Lay a splitter out over the on-screen `rect` it occupies.
    ///
    /// The one call both surfaces hit-test and paint through, so the line the
    /// pointer can grab is by construction the line that was drawn.
    pub fn screen_geometry(ctrl: &Control, rect: ERect) -> ScreenGeometry {
        let local = Rect::new(
            0,
            0,
            rect.width().round() as i32,
            rect.height().round() as i32,
        );
        let g = geometry(ctrl, local);
        ScreenGeometry {
            pane1: erect(g.pane1, rect.min),
            pane2: erect(g.pane2, rect.min),
            line: erect(g.line, rect.min),
            grip: erect(g.grip, rect.min),
            band: erect(g.band, rect.min),
            horizontal: g.horizontal,
        }
    }

    /// The percentage a pointer at `pos` puts the division at, for a splitter
    /// drawn over `rect` on screen.
    pub fn percent_at_screen(ctrl: &Control, rect: ERect, pos: Pos2) -> i32 {
        let local = Rect::new(
            0,
            0,
            rect.width().round() as i32,
            rect.height().round() as i32,
        );
        percent_at(
            ctrl,
            local,
            (pos.x - rect.min.x).round() as i32,
            (pos.y - rect.min.y).round() as i32,
        )
    }

    fn faded(c: Color32, alpha: f32) -> Color32 {
        Color32::from_rgba_unmultiplied(c.r(), c.g(), c.b(), (c.a() as f32 * alpha) as u8)
    }

    /// The colour a `LineColor` / `GripColor` property asks for, or the theme's
    /// own rule colour when it is empty — the splitter follows the form theme
    /// until the developer overrides it.
    fn resolved(ctrl: &Control, key: &str, ctx: &egui::Context, fallback: Color32) -> Color32 {
        let raw = sv(ctrl, key).trim();
        if raw.is_empty() {
            return crate::paint::theme_token(ctx, crate::surface_theme::ColorToken::Border)
                .unwrap_or(fallback);
        }
        crate::paint::parse_color(raw)
    }

    /// Draw the division line and its grip on top of an already-painted
    /// splitter face.
    ///
    /// `origin` is where the splitter's `rect` starts on screen, and `rect` is
    /// the splitter's own rectangle in the space `geometry` was asked about —
    /// pass the two consistently and the line lands where the hit-test says.
    ///
    /// **The grip is clipped to the splitter.** At 0 % and 100 % it straddles
    /// the splitter's edge, and only the half inside stays visible. That falls
    /// out of clipping rather than being special-cased, so there is no position
    /// at which it can slip out over a neighbouring control.
    pub fn paint(painter: &Painter, ctrl: &Control, rect: ERect, state: SplitState) {
        let g = screen_geometry(ctrl, rect);
        let ctx = painter.ctx();
        let alpha = state.alpha.clamp(0.0, 1.0);

        let line_col = faded(
            resolved(ctrl, "LineColor", ctx, Color32::from_rgb(150, 156, 172)),
            alpha,
        );
        // The grip defaults to the line's own colour, one step stronger so it
        // reads as a handle rather than a bulge in the line.
        let grip_raw = sv(ctrl, "GripColor").trim();
        let grip_col = if grip_raw.is_empty() {
            line_col
        } else {
            faded(crate::paint::parse_color(grip_raw), alpha)
        };
        // Under the hand the grip brightens; while dragging it stays bright.
        let lit = state.hovered || state.dragging;
        let grip_col = if lit {
            Color32::from_rgba_unmultiplied(
                grip_col.r().saturating_add(45),
                grip_col.g().saturating_add(45),
                grip_col.b().saturating_add(45),
                grip_col.a(),
            )
        } else {
            grip_col
        };

        // Everything the splitter draws stays inside the splitter.
        let clip = painter.clip_rect().intersect(rect);
        let p = painter.with_clip_rect(clip);

        p.rect_filled(g.line, 0.0, line_col);

        if g.grip.width() <= 0.0 || g.grip.height() <= 0.0 {
            return;
        }
        let grip = g.grip;
        let style = GripStyle::parse(sv(ctrl, "GripStyle"));
        let stroke = Stroke::new(1.5, grip_col);
        if style.is_circle() {
            let r = grip.width().min(grip.height()) * 0.5;
            if style.is_hollow() {
                // A hollow grip would show the line straight through it, so the
                // face is knocked out with the splitter's own backdrop first.
                p.circle_filled(grip.center(), r, backdrop(ctrl, ctx, alpha));
                p.circle_stroke(grip.center(), r, stroke);
            } else {
                p.circle_filled(grip.center(), r, grip_col);
            }
        } else {
            let rounding = grip.width().min(grip.height()) * 0.5;
            if style.is_hollow() {
                p.rect_filled(grip, rounding, backdrop(ctrl, ctx, alpha));
                p.rect_stroke(grip, rounding, stroke, StrokeKind::Middle);
            } else {
                p.rect_filled(grip, rounding, grip_col);
            }
        }
    }

    /// What a hollow grip is knocked out with: the splitter's own face, so the
    /// line does not run visibly through the middle of it.
    fn backdrop(ctrl: &Control, ctx: &egui::Context, alpha: f32) -> Color32 {
        let raw = sv(ctrl, "BackgroundColor").trim();
        let c = if raw.is_empty() {
            crate::paint::theme_token(ctx, crate::surface_theme::ColorToken::Card)
                .unwrap_or(Color32::from_rgb(240, 240, 240))
        } else {
            crate::paint::parse_color(raw)
        };
        faded(c, alpha)
    }
}

#[cfg(feature = "render")]
pub use painting::{paint, percent_at_screen, screen_geometry, ScreenGeometry, SplitState};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::PropValue;

    fn splitter(w: i32, h: i32) -> Control {
        let mut c = Control::new("SPLIT-1", ControlType::Splitter, 0, 0);
        c.rect = Rect::new(0, 0, w, h);
        c
    }

    /// `Horizontal` means the PANES are side by side — pane 1 left, pane 2
    /// right — which is the operator's definition and the opposite of what the
    /// bar-between-siblings splitter did before this.
    #[test]
    fn horizontal_puts_pane1_left_and_pane2_right() {
        let c = splitter(300, 200);
        let g = geometry(&c, c.rect);
        assert!(g.horizontal);
        assert!(
            g.pane1.x < g.pane2.x,
            "pane 1 is the LEFT pane: {:?} vs {:?}",
            g.pane1,
            g.pane2
        );
        assert_eq!(g.pane1.y, g.pane2.y, "side by side ⇒ same top edge");
        assert_eq!(g.pane1.h, g.pane2.h, "side by side ⇒ same height");
        assert!(g.line.h > g.line.w, "a vertical rule divides them");
    }

    #[test]
    fn vertical_puts_pane1_on_top_and_pane2_below() {
        let mut c = splitter(300, 200);
        c.set_prop("Orientation", "Vertical");
        let g = geometry(&c, c.rect);
        assert!(!g.horizontal);
        assert!(g.pane1.y < g.pane2.y, "pane 1 is the TOP pane");
        assert_eq!(g.pane1.x, g.pane2.x);
        assert!(g.line.w > g.line.h, "a horizontal rule divides them");
    }

    /// The two panes plus the visible part of the line account for the whole
    /// inner width — no gap the developer can see, and no overlap that would
    /// double-clip a child.
    ///
    /// "Visible part" is the point: at 0 % and 100 % the line straddles the
    /// splitter's edge, so only half of it is inside. That is the same rule
    /// that leaves half a grip showing at the extremes.
    #[test]
    fn the_panes_and_the_line_tile_the_inner_area() {
        for pct in [0, 15, 50, 83, 100] {
            let mut c = splitter(400, 200);
            c.set_prop("SplitPosition", pct as i64);
            let g = geometry(&c, c.rect);
            let inner = content_rect(c.rect);
            let seen = ((g.line.x + g.line.w).min(inner.x + inner.w) - g.line.x.max(inner.x)).max(0);
            assert_eq!(
                g.pane1.w + seen + g.pane2.w,
                inner.w,
                "at {pct}% the panes and the visible line must tile the inner width"
            );
            // An empty pane cannot overlap anything, and at an extreme the line
            // sits half outside it — so this is asked of the panes that exist.
            if g.pane1.w > 0 {
                assert!(
                    g.line.x >= g.pane1.x + g.pane1.w,
                    "at {pct}% the line must start where pane 1 ends: {:?} | {:?}",
                    g.pane1,
                    g.line
                );
            }
            if g.pane2.w > 0 {
                assert!(
                    g.pane2.x >= g.line.x + g.line.w,
                    "at {pct}% pane 2 must start where the line ends: {:?} | {:?}",
                    g.line,
                    g.pane2
                );
            }
            assert_eq!(g.pane1.x, inner.x, "pane 1 starts at the inner edge");
            assert_eq!(
                g.pane2.x + g.pane2.w,
                inner.x + inner.w,
                "pane 2 ends at the inner edge"
            );
        }
    }

    /// 0 % and 100 % are legal: one pane closes completely and the other holds
    /// everything. Nothing goes negative on the way.
    #[test]
    fn the_extremes_close_one_pane_without_going_negative() {
        let mut c = splitter(400, 200);
        c.set_prop("SplitPosition", 0i64);
        let g = geometry(&c, c.rect);
        assert_eq!(g.pane1.w, 0, "0 % closes pane 1");
        assert!(g.pane2.w > 0, "…and pane 2 holds the rest");

        c.set_prop("SplitPosition", 100i64);
        let g = geometry(&c, c.rect);
        assert_eq!(g.pane2.w, 0, "100 % closes pane 2");
        assert!(g.pane1.w > 0);
    }

    /// At an extreme the grip straddles the splitter's edge: half of it is
    /// outside, and the paint clip is what keeps the other half visible.
    #[test]
    fn at_an_extreme_the_grip_straddles_the_edge() {
        let mut c = splitter(400, 200);
        c.set_prop("SplitPosition", 0i64);
        let g = geometry(&c, c.rect);
        assert!(
            g.grip.x < content_rect(c.rect).x,
            "the grip reaches outside the inner area at 0 %: {:?}",
            g.grip
        );
        assert!(
            g.grip.x + g.grip.w > content_rect(c.rect).x,
            "…and half of it is still inside"
        );
    }

    /// A property nobody set must not produce an invisible splitter.
    #[test]
    fn a_fresh_splitter_divides_in_half() {
        let c = splitter(300, 200);
        let g = geometry(&c, c.rect);
        assert_eq!(split_percent(&c), 50);
        assert!(
            (g.pane1.w - g.pane2.w).abs() <= 2,
            "half and half: {:?} vs {:?}",
            g.pane1,
            g.pane2
        );
    }

    /// Out-of-range percentages are clamped, not honoured: a form hand-edited
    /// to `SplitPosition = 500` opens with pane 2 closed, not with a line
    /// painted somewhere off the form.
    #[test]
    fn a_percentage_out_of_range_is_clamped() {
        let mut c = splitter(300, 200);
        c.set_prop("SplitPosition", 500i64);
        assert_eq!(split_percent(&c), 100);
        c.set_prop("SplitPosition", -40i64);
        assert_eq!(split_percent(&c), 0);
    }

    #[test]
    fn the_pointer_maps_back_to_a_percentage() {
        let c = splitter(400, 200);
        let inner = content_rect(c.rect);
        let mid = inner.x + inner.w / 2;
        assert_eq!(percent_at(&c, c.rect, mid, 100), 50);
        assert_eq!(percent_at(&c, c.rect, inner.x, 100), 0);
        assert_eq!(percent_at(&c, c.rect, inner.x + inner.w, 100), 100);
        assert_eq!(
            percent_at(&c, c.rect, inner.x - 500, 100),
            0,
            "dragging past the edge stops at the edge"
        );
    }

    /// The band is what the pointer has to hit, and a 2pt line is not hittable
    /// on its own.
    #[test]
    fn the_grab_band_is_wider_than_the_line() {
        let c = splitter(400, 200);
        let g = geometry(&c, c.rect);
        assert!(
            g.band.w >= g.line.w + 2 * GRAB_TOLERANCE,
            "band {:?} must be grabbable around line {:?}",
            g.band,
            g.line
        );
        assert!(g.band.contains(g.line.x, g.line.y + 1));
    }

    #[test]
    fn grip_styles_parse_and_default() {
        assert_eq!(GripStyle::parse("HollowPill"), GripStyle::HollowPill);
        assert_eq!(GripStyle::parse("hollow circle"), GripStyle::HollowCircle);
        assert_eq!(GripStyle::parse("FilledCircle"), GripStyle::FilledCircle);
        assert_eq!(
            GripStyle::parse(""),
            GripStyle::FilledPill,
            "an unset property is the default grip, never no grip"
        );
    }

    #[test]
    fn pane_ids_and_marker() {
        assert_eq!(pane_id("SPLIT-1", 1), "SPLIT-1-Pane1");
        assert_eq!(pane_id("SPLIT-1", 2), "SPLIT-1-Pane2");
        let mut p = Control::new("SPLIT-1-Pane2", ControlType::Panel, 0, 0);
        assert_eq!(pane_index(&p), None);
        p.set_prop(PANE_PROP, PropValue::Int(2));
        assert_eq!(pane_index(&p), Some(2));
    }
}
