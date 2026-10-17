// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! **The spotlight**: the geometry and the paint shared by the IDE Walkthrough
//! (spec 059) and the Task Wizard (spec 104).
//!
//! Both dim the IDE, leave something lit, and point a comic speech balloon at it.
//! What they share lives here, as pure functions where it can be: a geometry that
//! returns rectangles can be asserted without a renderer.
//!
//! The Walkthrough's own items were moved here unchanged and are re-exported from
//! `walkthrough.rs`; `walkthrough::tests::the_tour_paints_exactly_what_it_painted`
//! pins what the tour paints, so a change made here for the wizard cannot change
//! the tour unnoticed.

use eframe::egui::{Color32, CornerRadius, Painter, Pos2, Rect, Stroke, Vec2};

use crate::theme::Theme;

/// The four rectangles that dim everything except `hole`.
///
/// Returned rather than painted so the geometry can be asserted without a
/// renderer. Together they cover `screen` minus `hole` exactly, and none of them
/// overlaps `hole` — which is the whole claim a spotlight makes.
pub fn scrim_bands(screen: Rect, hole: Rect) -> [Rect; 4] {
    let hole = hole.intersect(screen);
    [
        // Above.
        Rect::from_min_max(screen.min, Pos2::new(screen.max.x, hole.min.y)),
        // Below.
        Rect::from_min_max(Pos2::new(screen.min.x, hole.max.y), screen.max),
        // Left of the hole, between its top and bottom.
        Rect::from_min_max(
            Pos2::new(screen.min.x, hole.min.y),
            Pos2::new(hole.min.x, hole.max.y),
        ),
        // Right of it.
        Rect::from_min_max(
            Pos2::new(hole.max.x, hole.min.y),
            Pos2::new(screen.max.x, hole.max.y),
        ),
    ]
}

/// Which edge of the balloon the tail leaves from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Side {
    Right,
    Left,
    Below,
    Above,
}

/// A placed balloon: its body, and the three points of its tail.
#[derive(Debug, Clone, Copy)]
pub struct Balloon {
    pub body: Rect,
    pub tail: [Pos2; 3],
    pub side: Side,
}

/// Gap between the target and the balloon body.
pub(crate) const GAP: f32 = 18.0;
/// How far the balloon must stay from the window edge.
pub(crate) const MARGIN: f32 = 12.0;
/// Half-width of the tail where it meets the body.
pub(crate) const TAIL_HALF: f32 = 9.0;
/// How far the tail tip reaches *inside* the target, so it demonstrably touches.
pub(crate) const TAIL_BITE: f32 = 6.0;
/// The balloon body's corner radius. The tail's base keeps this far from a corner.
pub(crate) const BODY_RADIUS: f32 = 14.0;

/// Place a balloon of `size` so it points at `target` and stays on `screen`.
///
/// Sides are tried right, left, below, above. Right first is not arbitrary: five
/// of the Walkthrough's six targets live in the left-hand project tree, so the
/// balloon lands over the central pane with its tail pointing back at the tree —
/// the way the eye already travels. The sixth, the output pane, spans the full
/// width, so right and left both fail on space and `Above` wins, which is also
/// correct.
pub fn place_balloon(screen: Rect, target: Rect, size: Vec2) -> Balloon {
    let field = screen.shrink(MARGIN);
    let need = |space: f32| space >= size.x + GAP;
    let need_v = |space: f32| space >= size.y + GAP;

    let side = if need(field.max.x - target.max.x) {
        Side::Right
    } else if need(target.min.x - field.min.x) {
        Side::Left
    } else if need_v(field.max.y - target.max.y) {
        Side::Below
    } else if need_v(target.min.y - field.min.y) {
        Side::Above
    } else {
        // Nowhere fits. Centre it and keep aiming — a balloon that overlaps its
        // target still points at the right thing; a tailless one does not.
        Side::Right
    };

    let body = body_beside(field, target, target.center(), size, side);
    let tip = tip_inside(target, target.center(), side);
    Balloon {
        body,
        tail: tail_between(body, tip, side),
        side,
    }
}

/// Place a balloon of `size` **beside** `avoid`, with its tail aimed at `aim`.
///
/// The Task Wizard lights a whole window (its *live region*) but rings one widget
/// inside it: a balloon placed against the widget would sit on the window the
/// developer has to use. So the balloon keeps clear of `avoid`, and only its tail
/// crosses into it to touch `aim`.
///
/// When no side has room — a window nearly as large as the screen — the balloon
/// goes on the side with the **most** room and slides back on screen, overlapping
/// as little of `avoid` as it can. With `avoid == aim` and a side that fits, the
/// result equals [`place_balloon`]'s.
pub fn place_balloon_avoiding(screen: Rect, avoid: Rect, aim: Rect, size: Vec2) -> Balloon {
    let field = screen.shrink(MARGIN);
    let room = [
        (Side::Right, field.max.x - avoid.max.x - (size.x + GAP)),
        (Side::Left, avoid.min.x - field.min.x - (size.x + GAP)),
        (Side::Below, field.max.y - avoid.max.y - (size.y + GAP)),
        (Side::Above, avoid.min.y - field.min.y - (size.y + GAP)),
    ];
    let side = room
        .iter()
        .find(|(_, spare)| *spare >= 0.0)
        .or_else(|| {
            room.iter()
                .max_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal))
        })
        .map(|(s, _)| *s)
        .unwrap_or(Side::Right);

    // Centre the body on the widget, not on the window, so the tail stays short.
    let aim_c = Pos2::new(
        aim.center().x.clamp(avoid.min.x, avoid.max.x),
        aim.center().y.clamp(avoid.min.y, avoid.max.y),
    );
    let body = body_beside(field, avoid, aim_c, size, side);
    let tip = tip_inside(aim, aim.center(), side);
    Balloon {
        body,
        tail: tail_between(body, tip, side),
        side,
    }
}

/// The body on `side` of `beside`, centred across that side on `centre`, then
/// slid back inside `field`.
fn body_beside(field: Rect, beside: Rect, centre: Pos2, size: Vec2, side: Side) -> Rect {
    let body = match side {
        Side::Right => Rect::from_min_size(
            Pos2::new(beside.max.x + GAP, centre.y - size.y / 2.0),
            size,
        ),
        Side::Left => Rect::from_min_size(
            Pos2::new(beside.min.x - GAP - size.x, centre.y - size.y / 2.0),
            size,
        ),
        Side::Below => Rect::from_min_size(
            Pos2::new(centre.x - size.x / 2.0, beside.max.y + GAP),
            size,
        ),
        Side::Above => Rect::from_min_size(
            Pos2::new(centre.x - size.x / 2.0, beside.min.y - GAP - size.y),
            size,
        ),
    };
    // Slide it back inside the window. This is what keeps a balloon on screen
    // when its target is hard against an edge.
    let dx = (field.min.x - body.min.x).max(0.0) - (body.max.x - field.max.x).max(0.0);
    let dy = (field.min.y - body.min.y).max(0.0) - (body.max.y - field.max.y).max(0.0);
    body.translate(Vec2::new(dx, dy))
}

/// The tail's tip: *inside* `target`, so "the tail touches it" is true by
/// construction rather than by eye.
fn tip_inside(target: Rect, c: Pos2, side: Side) -> Pos2 {
    match side {
        Side::Right => Pos2::new(target.max.x - TAIL_BITE, c.y.clamp(target.min.y, target.max.y)),
        Side::Left => Pos2::new(target.min.x + TAIL_BITE, c.y.clamp(target.min.y, target.max.y)),
        Side::Below => Pos2::new(c.x.clamp(target.min.x, target.max.x), target.max.y - TAIL_BITE),
        Side::Above => Pos2::new(c.x.clamp(target.min.x, target.max.x), target.min.y + TAIL_BITE),
    }
}

/// The tail's three points: its base sits on the body edge facing the target,
/// centred on the tip and clamped clear of the rounded corners.
fn tail_between(body: Rect, tip: Pos2, side: Side) -> [Pos2; 3] {
    let r = BODY_RADIUS;
    match side {
        Side::Right | Side::Left => {
            let x = if side == Side::Right {
                body.min.x
            } else {
                body.max.x
            };
            let y = tip.y.clamp(body.min.y + r + TAIL_HALF, body.max.y - r - TAIL_HALF);
            [
                Pos2::new(x, y - TAIL_HALF),
                tip,
                Pos2::new(x, y + TAIL_HALF),
            ]
        }
        Side::Below | Side::Above => {
            let y = if side == Side::Below {
                body.min.y
            } else {
                body.max.y
            };
            let x = tip.x.clamp(body.min.x + r + TAIL_HALF, body.max.x - r - TAIL_HALF);
            [
                Pos2::new(x - TAIL_HALF, y),
                tip,
                Pos2::new(x + TAIL_HALF, y),
            ]
        }
    }
}

/// The balloon's colours.
#[derive(Debug, Clone, Copy)]
pub struct Palette {
    pub fill: Color32,
    pub text: Color32,
    pub dim_text: Color32,
    pub ring: Color32,
    pub border: Color32,
    pub button_fill: Color32,
    pub button_text: Color32,
}

/// The dim laid over everything but the target — the same value the Building
/// modal and the KB-reindex scrim use, so the spotlight looks like the IDE's other
/// modals on every theme.
pub(crate) const DIM: Color32 = Color32::from_black_alpha(150);

/// Colours for the balloon on `theme`.
///
/// **The fill and the text are a fixed pair, deliberately.** The balloon always
/// sits on the same `from_black_alpha(150)` field on all 32 themes, so there is
/// nothing theme-dependent for its legibility to adapt to — and deriving ink
/// from `ui.visuals()` is what renders dark-on-dark under the glass themes. What
/// *does* follow the theme is the decoration: the ring around the lit component
/// and the balloon's border.
pub fn balloon_palette(theme: &Theme) -> Palette {
    let accent = theme.accent;
    // The primary button borrows the accent only when white reads on it;
    // otherwise the accent stays an outline and the button keeps the charcoal.
    let accent_ok = crate::contrast::contrast_ratio(accent, Color32::WHITE) >= 4.5;
    Palette {
        fill: Color32::from_rgb(0x1E, 0x22, 0x2A),
        text: Color32::WHITE,
        dim_text: Color32::from_rgb(0xC2, 0xC9, 0xD4),
        ring: accent,
        border: accent,
        button_fill: if accent_ok {
            accent
        } else {
            Color32::from_rgb(0x2C, 0x33, 0x3E)
        },
        button_text: Color32::WHITE,
    }
}

/// The ring drawn around the lit component.
pub(crate) fn paint_ring(painter: &Painter, target: Rect, p: &Palette) {
    painter.rect_stroke(
        target.expand(2.0),
        CornerRadius::same(6),
        Stroke::new(2.0, p.ring),
        eframe::egui::StrokeKind::Outside,
    );
}

/// The balloon's body and tail, with their outline.
///
/// Body, then tail, then the outline over both, so the join between them is never
/// drawn as a seam.
pub(crate) fn paint_balloon_frame(painter: &Painter, placed: &Balloon, p: &Palette) {
    painter.rect_filled(placed.body, CornerRadius::same(BODY_RADIUS as u8), p.fill);
    painter.add(eframe::egui::Shape::convex_polygon(
        placed.tail.to_vec(),
        p.fill,
        Stroke::NONE,
    ));
    painter.rect_stroke(
        placed.body,
        CornerRadius::same(BODY_RADIUS as u8),
        Stroke::new(1.0, p.border),
        eframe::egui::StrokeKind::Inside,
    );
    painter.line_segment([placed.tail[0], placed.tail[1]], Stroke::new(1.0, p.border));
    painter.line_segment([placed.tail[1], placed.tail[2]], Stroke::new(1.0, p.border));
}

// ── Many holes ────────────────────────────────────────────────────────────────

/// The most holes [`blockers`] cuts. The Task Wizard needs: the live region, the
/// toggle (with its ▾) and up to two windows that opened during a step. Holes past
/// this are merged into the last one — a bigger hole blocks less, never more, so
/// the error is on the side of usability.
pub const MAX_HOLES: usize = 4;

/// The most rectangles [`blockers`] returns: `MAX_HOLES` holes cut at most
/// `2·MAX_HOLES + 1` horizontal slabs, each into at most `MAX_HOLES + 1` pieces.
pub const MAX_BLOCKERS: usize = (2 * MAX_HOLES + 1) * (MAX_HOLES + 1);

/// The screen minus some holes, as disjoint rectangles, in a fixed-size array.
#[derive(Debug, Clone, Copy)]
pub struct Blockers {
    rects: [Rect; MAX_BLOCKERS],
    len: usize,
}

impl Blockers {
    /// The rectangles, top to bottom, left to right.
    pub fn as_slice(&self) -> &[Rect] {
        &self.rects[..self.len]
    }

    fn push(&mut self, r: Rect) {
        if r.width() > 0.0 && r.height() > 0.0 && self.len < MAX_BLOCKERS {
            self.rects[self.len] = r;
            self.len += 1;
        }
    }
}

/// Everything of `screen` that is not in one of `holes`, as disjoint rectangles.
///
/// A slab decomposition: every hole's top and bottom cut the screen into
/// horizontal slabs; inside a slab every hole either spans it or misses it, so the
/// slab is the run of gaps between the (merged) holes that span it. Vertically
/// adjacent slabs whose gaps are identical are merged, which keeps the count — and
/// so the number of input-blocking Areas the wizard shows — small.
///
/// Holes are clipped to the screen; empty ones are ignored; holes past
/// [`MAX_HOLES`] are merged into the last.
pub fn blockers(screen: Rect, holes: &[Rect]) -> Blockers {
    let mut out = Blockers {
        rects: [Rect::NOTHING; MAX_BLOCKERS],
        len: 0,
    };
    // Clip, drop the empty, and fold the surplus into the last hole.
    let mut hs: [Rect; MAX_HOLES] = [Rect::NOTHING; MAX_HOLES];
    let mut n = 0usize;
    for h in holes {
        let h = h.intersect(screen);
        if !(h.width() > 0.0 && h.height() > 0.0) {
            continue;
        }
        if n < MAX_HOLES {
            hs[n] = h;
            n += 1;
        } else {
            hs[MAX_HOLES - 1] = hs[MAX_HOLES - 1].union(h);
        }
    }
    let hs = &hs[..n];

    // The cut lines.
    let mut ys: [f32; 2 * MAX_HOLES + 2] = [0.0; 2 * MAX_HOLES + 2];
    let mut ny = 0usize;
    for y in std::iter::once(screen.min.y)
        .chain(std::iter::once(screen.max.y))
        .chain(hs.iter().flat_map(|h| [h.min.y, h.max.y]))
    {
        ys[ny] = y;
        ny += 1;
    }
    let ys = &mut ys[..ny];
    ys.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

    // One slab at a time: its gaps, merged downwards with the slab above when equal.
    type Slab = (f32, f32, [(f32, f32); MAX_HOLES + 1], usize);
    fn flush(slab: Slab, out: &mut Blockers) {
        let (y0, y1, gaps, ng) = slab;
        for &(x0, x1) in &gaps[..ng] {
            out.push(Rect::from_min_max(Pos2::new(x0, y0), Pos2::new(x1, y1)));
        }
    }
    let mut prev: Option<Slab> = None;
    for w in ys.windows(2) {
        let (y0, y1) = (w[0], w[1]);
        if y1 <= y0 {
            continue;
        }
        // The holes that span this slab, as x-intervals, merged.
        let mut iv: [(f32, f32); MAX_HOLES] = [(0.0, 0.0); MAX_HOLES];
        let mut ni = 0usize;
        for h in hs {
            if h.min.y <= y0 && h.max.y >= y1 {
                iv[ni] = (h.min.x, h.max.x);
                ni += 1;
            }
        }
        let iv = &mut iv[..ni];
        iv.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
        let mut gaps: [(f32, f32); MAX_HOLES + 1] = [(0.0, 0.0); MAX_HOLES + 1];
        let mut ng = 0usize;
        let mut x = screen.min.x;
        for &(a, b) in iv.iter() {
            if a > x {
                gaps[ng] = (x, a);
                ng += 1;
            }
            x = x.max(b);
        }
        if x < screen.max.x {
            gaps[ng] = (x, screen.max.x);
            ng += 1;
        }
        match prev {
            Some((py0, py1, pg, png)) if py1 == y0 && png == ng && pg[..png] == gaps[..ng] => {
                prev = Some((py0, y1, pg, png));
            }
            Some(p) => {
                flush(p, &mut out);
                prev = Some((y0, y1, gaps, ng));
            }
            None => prev = Some((y0, y1, gaps, ng)),
        }
    }
    if let Some(p) = prev {
        flush(p, &mut out);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn screen() -> Rect {
        Rect::from_min_size(Pos2::ZERO, Vec2::new(1400.0, 900.0))
    }

    /// A deterministic pseudo-random sequence (xorshift), so the property tests
    /// are reproducible without a dependency.
    struct Rng(u64);
    impl Rng {
        fn next(&mut self) -> f32 {
            self.0 ^= self.0 << 13;
            self.0 ^= self.0 >> 7;
            self.0 ^= self.0 << 17;
            (self.0 % 10_000) as f32 / 10_000.0
        }
        fn rect(&mut self, s: Rect) -> Rect {
            let x = s.min.x + self.next() * s.width();
            let y = s.min.y + self.next() * s.height();
            let w = self.next() * s.width() * 0.6;
            let h = self.next() * s.height() * 0.6;
            Rect::from_min_size(Pos2::new(x, y), Vec2::new(w, h))
        }
    }

    /// Half-open containment — `[min, max)` on both axes — the convention for a
    /// tiling: a point on an edge two pieces share belongs to exactly one of them.
    /// (`Rect::contains` is closed on every edge, which would count it twice.)
    fn inside(r: Rect, p: Pos2) -> bool {
        p.x >= r.min.x && p.x < r.max.x && p.y >= r.min.y && p.y < r.max.y
    }

    /// Area of `r` covered by none of `holes`, sampled on a grid — independent of
    /// the decomposition it checks.
    fn sample(s: Rect, holes: &[Rect], blocks: &[Rect]) -> (usize, usize) {
        let (mut wrong, mut total) = (0usize, 0usize);
        let step = 7.0;
        let mut y = s.min.y + step / 2.0;
        while y < s.max.y {
            let mut x = s.min.x + step / 2.0;
            while x < s.max.x {
                let p = Pos2::new(x, y);
                let in_hole = holes.iter().any(|h| inside(h.intersect(s), p));
                let n_block = blocks.iter().filter(|b| inside(**b, p)).count();
                // Outside every hole: covered exactly once. Inside a hole: never.
                if (in_hole && n_block != 0) || (!in_hole && n_block != 1) {
                    wrong += 1;
                }
                total += 1;
                x += step;
            }
            y += step;
        }
        (wrong, total)
    }

    #[test]
    fn the_blockers_cover_the_screen_minus_the_holes_exactly_once() {
        let s = screen();
        let mut rng = Rng(0x9E37_79B9_7F4A_7C15);
        let mut worst = 0usize;
        for case in 0..300 {
            let k = 1 + case % MAX_HOLES;
            let holes: Vec<Rect> = (0..k).map(|_| rng.rect(s)).collect();
            let b = blockers(s, &holes);
            worst = worst.max(b.as_slice().len());
            let (wrong, total) = sample(s, &holes, b.as_slice());
            assert_eq!(
                wrong, 0,
                "case {case}: {wrong} of {total} sample points are covered wrongly; holes {holes:?}, blockers {:?}",
                b.as_slice()
            );
            for (i, a) in b.as_slice().iter().enumerate() {
                for c in &b.as_slice()[i + 1..] {
                    assert!(a.intersect(*c).area() <= 0.01, "blockers overlap: {a:?} {c:?}");
                }
            }
        }
        println!("300 random cases of 1–{MAX_HOLES} holes: exact cover, no overlap; at most {worst} blockers (bound {MAX_BLOCKERS})");
    }

    #[test]
    fn no_hole_is_one_blocker_and_one_hole_is_the_four_bands() {
        let s = screen();
        assert_eq!(blockers(s, &[]).as_slice(), &[s]);
        let hole = Rect::from_min_size(Pos2::new(300.0, 200.0), Vec2::new(220.0, 40.0));
        let b = blockers(s, &[hole]);
        assert_eq!(b.as_slice().len(), 4, "{:?}", b.as_slice());
        let bands: f32 = scrim_bands(s, hole).iter().map(|r| r.area()).sum();
        let ours: f32 = b.as_slice().iter().map(|r| r.area()).sum();
        assert!((bands - ours).abs() < 1.0, "the same area as the four bands");
    }

    #[test]
    fn surplus_holes_are_merged_never_dropped() {
        let s = screen();
        let holes: Vec<Rect> = (0..(MAX_HOLES + 3))
            .map(|i| Rect::from_min_size(Pos2::new(50.0 + 120.0 * i as f32, 300.0), Vec2::new(60.0, 60.0)))
            .collect();
        let b = blockers(s, &holes);
        for h in &holes {
            for r in b.as_slice() {
                assert!(r.intersect(*h).area() <= 0.01, "hole {h:?} is blocked by {r:?}");
            }
        }
    }

    #[test]
    fn a_balloon_beside_a_window_keeps_off_it_and_still_touches_its_widget() {
        let s = screen();
        let size = Vec2::new(340.0, 170.0);
        for (window, widget) in [
            // A manager on the left: the balloon goes right.
            (
                Rect::from_min_size(Pos2::new(40.0, 120.0), Vec2::new(780.0, 560.0)),
                Rect::from_min_size(Pos2::new(300.0, 300.0), Vec2::new(300.0, 28.0)),
            ),
            // On the right: it goes left.
            (
                Rect::from_min_size(Pos2::new(560.0, 120.0), Vec2::new(780.0, 560.0)),
                Rect::from_min_size(Pos2::new(900.0, 640.0), Vec2::new(200.0, 28.0)),
            ),
            // A narrow pane on the right edge.
            (
                Rect::from_min_size(Pos2::new(1100.0, 40.0), Vec2::new(300.0, 860.0)),
                Rect::from_min_size(Pos2::new(1120.0, 500.0), Vec2::new(260.0, 22.0)),
            ),
        ] {
            let b = place_balloon_avoiding(s, window, widget, size);
            assert!(b.body.intersect(window).area() <= 0.01, "{:?} sits on the window {window:?}", b.body);
            assert!(widget.expand(0.5).contains(b.tail[1]), "tail tip {:?} misses {widget:?}", b.tail[1]);
            assert!(s.contains_rect(b.body), "balloon {:?} left the screen", b.body);
        }
    }

    #[test]
    fn a_window_too_big_for_any_side_gets_the_side_with_most_room() {
        let s = screen();
        let size = Vec2::new(340.0, 170.0);
        let window = Rect::from_min_size(Pos2::new(60.0, 40.0), Vec2::new(1120.0, 820.0));
        let widget = Rect::from_min_size(Pos2::new(900.0, 400.0), Vec2::new(200.0, 28.0));
        let b = place_balloon_avoiding(s, window, widget, size);
        assert_eq!(b.side, Side::Right, "the right has the most room (220 px)");
        assert!(s.contains_rect(b.body));
        assert!(widget.expand(0.5).contains(b.tail[1]));
    }

    #[test]
    fn avoiding_the_target_itself_is_the_walkthrough_placement() {
        let s = screen();
        let size = Vec2::new(340.0, 160.0);
        for target in [
            Rect::from_min_size(Pos2::new(0.0, 400.0), Vec2::new(300.0, 30.0)),
            Rect::from_min_size(Pos2::new(1100.0, 400.0), Vec2::new(300.0, 30.0)),
            Rect::from_min_size(Pos2::new(500.0, 0.0), Vec2::new(300.0, 30.0)),
            Rect::from_min_size(Pos2::new(0.0, 860.0), Vec2::new(1400.0, 40.0)),
        ] {
            let a = place_balloon(s, target, size);
            let b = place_balloon_avoiding(s, target, target, size);
            assert_eq!((a.body, a.tail, a.side), (b.body, b.tail, b.side), "target {target:?}");
        }
    }
}
