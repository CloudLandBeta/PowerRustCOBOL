// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Spec 055 T10 — the stack's geometry: nine anchors, vertical only.
//!
//! `stack_layout` is pure, so every case here is exact arithmetic on a fabricated
//! surface. `sizes` is always **oldest-first**, and the returned rects come back
//! in that same order — so "where did the newest go" is a question about
//! `result.last()`, and the tests say so explicitly rather than relying on the
//! reader to track an index.

use cobolt_forms::model::Rect;
use cobolt_forms::snackbar::{stack_layout, SnackAnchor, StackOrder};

const SURFACE: Rect = Rect { x: 0, y: 0, w: 1000, h: 700 };
const MARGIN: f32 = 16.0;
const SPACING: f32 = 8.0;

/// Three notifications, oldest first, all the same size for legibility.
fn three() -> Vec<(f32, f32)> {
    vec![(300.0, 56.0), (300.0, 56.0), (300.0, 56.0)]
}

#[test]
fn a_stack_grows_away_from_its_anchor_with_the_newest_nearest_it() {
    eprintln!("\n  anchor         oldest y   middle y   newest y   growth     newest is");
    eprintln!("  ------------   --------   --------   --------   --------   ---------");
    let mut checked = 0usize;
    for anchor in SnackAnchor::ALL {
        let r = stack_layout(anchor, MARGIN, SPACING, StackOrder::Auto, SURFACE, &three());
        let (oldest, middle, newest) = (r[0].y, r[1].y, r[2].y);

        let (growth, where_newest) = if anchor.is_top() {
            // R12 — Top: the stack grows DOWNWARD, newest nearest the anchor,
            // so the newest sits at the top and the oldest is pushed down.
            assert!(newest < middle && middle < oldest, "{:?}: newest must be topmost", anchor);
            assert_eq!(newest as f32, SURFACE.y as f32 + MARGIN, "{:?}: newest on the margin", anchor);
            ("downward", "topmost")
        } else if anchor.is_bottom() {
            // R12 — Bottom: grows UPWARD, newest nearest the anchor, so the
            // newest sits at the bottom.
            assert!(newest > middle && middle > oldest, "{:?}: newest must be bottommost", anchor);
            let newest_bottom = r[2].y + r[2].h;
            assert_eq!(
                newest_bottom as f32,
                SURFACE.y as f32 + SURFACE.h as f32 - MARGIN,
                "{:?}: newest on the bottom margin", anchor
            );
            ("upward", "bottommost")
        } else {
            // R13 — Centre row: newest first, growing down.
            assert!(newest < middle && middle < oldest, "{:?}: newest first, growing down", anchor);
            ("downward", "topmost")
        };

        eprintln!(
            "  {:<12}   {oldest:>8}   {middle:>8}   {newest:>8}   {growth:<8}   {where_newest}",
            anchor.as_str()
        );
        checked += 1;
    }
    assert_eq!(checked, 9, "all nine anchors");
    eprintln!("  → AC4/R12/R13: 9 anchors, growth direction and newest-nearest verified\n");
}

#[test]
fn the_stack_is_vertical_only_and_spaced_by_stack_spacing() {
    for anchor in SnackAnchor::ALL {
        let r = stack_layout(anchor, MARGIN, SPACING, StackOrder::Auto, SURFACE, &three());
        // R11 — vertical only. Every notification shares one x; there is no
        // horizontal stacking, by contract (§2 non-goals).
        assert_eq!(r[0].x, r[1].x, "{:?}: no horizontal stacking", anchor);
        assert_eq!(r[1].x, r[2].x, "{:?}: no horizontal stacking", anchor);

        // Sorted top-to-bottom, consecutive gaps are exactly StackSpacing.
        let mut sorted = r.clone();
        sorted.sort_by_key(|a| a.y);
        for w in sorted.windows(2) {
            let gap = w[1].y - (w[0].y + w[0].h);
            assert_eq!(gap as f32, SPACING, "{:?}: gap must be StackSpacing", anchor);
        }
    }
    eprintln!("\n  R11 — 9 anchors: one shared x each, every gap exactly {SPACING} pt\n");
}

#[test]
fn each_anchor_puts_the_stack_on_its_own_side_of_the_surface() {
    eprintln!("\n  anchor         x      right edge   top edge   bottom edge");
    eprintln!("  ------------   ----   ----------   --------   -----------");
    for anchor in SnackAnchor::ALL {
        let r = stack_layout(anchor, MARGIN, SPACING, StackOrder::Auto, SURFACE, &three());
        let x = r[0].x;
        let right = r[0].x + r[0].w;
        let top = r.iter().map(|a| a.y).min().unwrap();
        let bottom = r.iter().map(|a| a.y + a.h).max().unwrap();
        eprintln!("  {:<12}   {x:>4}   {right:>10}   {top:>8}   {bottom:>11}", anchor.as_str());

        match anchor {
            SnackAnchor::TopLeft | SnackAnchor::CenterLeft | SnackAnchor::BottomLeft => {
                assert_eq!(x as f32, SURFACE.x as f32 + MARGIN, "{:?} hugs the left margin", anchor);
            }
            SnackAnchor::TopRight | SnackAnchor::CenterRight | SnackAnchor::BottomRight => {
                assert_eq!(
                    right as f32,
                    SURFACE.x as f32 + SURFACE.w as f32 - MARGIN,
                    "{:?} hugs the right margin", anchor
                );
            }
            _ => {
                let left_gap = x - SURFACE.x;
                let right_gap = SURFACE.x + SURFACE.w - right;
                assert!((left_gap - right_gap).abs() <= 1, "{:?} is horizontally centred", anchor);
            }
        }
        // R16/R26 — the run always lands inside the surface; nothing ever asks
        // the surface to grow to hold it.
        assert!(top >= SURFACE.y, "{:?} stays inside the surface (top)", anchor);
        assert!(bottom <= SURFACE.y + SURFACE.h, "{:?} stays inside the surface (bottom)", anchor);
        assert!(x >= SURFACE.x && right <= SURFACE.x + SURFACE.w, "{:?} stays inside horizontally", anchor);
    }
    eprintln!("  → 9 anchors placed on their own side, all inside the surface\n");
}

#[test]
fn stack_order_overrides_the_anchors_own_rule() {
    // Centre defaults newest-first (R13) — and `StackOrder` overrides it, which
    // is the clause that makes the property mean anything.
    let auto = stack_layout(SnackAnchor::Center, MARGIN, SPACING, StackOrder::Auto, SURFACE, &three());
    let first = stack_layout(SnackAnchor::Center, MARGIN, SPACING, StackOrder::NewestFirst, SURFACE, &three());
    let last = stack_layout(SnackAnchor::Center, MARGIN, SPACING, StackOrder::NewestLast, SURFACE, &three());

    assert_eq!(auto, first, "Centre's Auto IS newest-first (R13)");
    // NewestLast reverses it: the newest goes furthest from the anchor.
    assert!(last[2].y > last[1].y && last[1].y > last[0].y, "NewestLast puts the newest last");
    assert_ne!(auto, last, "NewestLast must differ from Auto");

    // And on a Bottom anchor, NewestLast flips the default too.
    let b_auto = stack_layout(SnackAnchor::BottomRight, MARGIN, SPACING, StackOrder::Auto, SURFACE, &three());
    let b_last = stack_layout(SnackAnchor::BottomRight, MARGIN, SPACING, StackOrder::NewestLast, SURFACE, &three());
    assert!(b_auto[2].y > b_auto[0].y, "Bottom+Auto: newest is bottommost");
    assert!(b_last[2].y < b_last[0].y, "Bottom+NewestLast: newest is topmost");

    eprintln!(
        "\n  StackOrder — Center Auto y {:?}; NewestLast y {:?}; BottomRight Auto y {:?} vs NewestLast y {:?}\n",
        auto.iter().map(|r| r.y).collect::<Vec<_>>(),
        last.iter().map(|r| r.y).collect::<Vec<_>>(),
        b_auto.iter().map(|r| r.y).collect::<Vec<_>>(),
        b_last.iter().map(|r| r.y).collect::<Vec<_>>(),
    );
}

#[test]
fn dismissing_the_middle_closes_the_gap_immediately() {
    // R14 — the layout is recomputed from whoever is left, so there is no gap
    // to close: the arithmetic cannot leave one. Shown before/after.
    let before = stack_layout(SnackAnchor::BottomRight, MARGIN, SPACING, StackOrder::Auto, SURFACE, &three());
    // Drop the middle (index 1 of the oldest-first list).
    let remaining = vec![(300.0, 56.0), (300.0, 56.0)];
    let after = stack_layout(SnackAnchor::BottomRight, MARGIN, SPACING, StackOrder::Auto, SURFACE, &remaining);

    eprintln!("\n  before — {:?}", before.iter().map(|r| (r.y, r.h)).collect::<Vec<_>>());
    eprintln!("  after  — {:?}", after.iter().map(|r| (r.y, r.h)).collect::<Vec<_>>());

    assert_eq!(after.len(), 2);
    let gap = after[1].y - (after[0].y + after[0].h);
    assert_eq!(gap as f32, SPACING, "the survivors are exactly StackSpacing apart — no hole");
    // The stack still hugs the anchor: the bottom one is still on the margin.
    assert_eq!(
        (after[1].y + after[1].h) as f32,
        SURFACE.y as f32 + SURFACE.h as f32 - MARGIN,
        "the run re-hugs the bottom margin after the dismissal"
    );
    // And it moved: two notifications occupy less height than three did.
    let before_top = before.iter().map(|r| r.y).min().unwrap();
    let after_top = after.iter().map(|r| r.y).min().unwrap();
    assert!(after_top > before_top, "the run shrank toward its anchor ({before_top} → {after_top})");
    eprintln!(
        "  → AC5/R14: middle dismissed, gap {gap} pt (= StackSpacing), run top {before_top} → {after_top}\n"
    );
}

#[test]
fn an_empty_stack_and_a_single_notification_are_both_well_formed() {
    assert!(stack_layout(SnackAnchor::BottomRight, MARGIN, SPACING, StackOrder::Auto, SURFACE, &[]).is_empty());
    let one = stack_layout(SnackAnchor::BottomRight, MARGIN, SPACING, StackOrder::Auto, SURFACE, &[(300.0, 56.0)]);
    assert_eq!(one.len(), 1);
    assert_eq!((one[0].y + one[0].h) as f32, SURFACE.y as f32 + SURFACE.h as f32 - MARGIN);
    assert_eq!((one[0].x + one[0].w) as f32, SURFACE.x as f32 + SURFACE.w as f32 - MARGIN);
    eprintln!("\n  degenerate cases — 0 notifications → empty; 1 → {:?} on both margins\n", one[0]);
}

#[test]
fn the_stack_is_placed_against_the_surface_it_is_given_not_the_window() {
    // R16/D3 — an Embedded form's surface is its ContentPane, which does NOT
    // start at the origin. The same call with an offset surface must produce
    // rects inside THAT rect, which is the whole of what "anchored to the form's
    // surface" buys.
    let pane = Rect { x: 296, y: 64, w: 704, h: 600 };
    let r = stack_layout(SnackAnchor::BottomRight, MARGIN, SPACING, StackOrder::Auto, pane, &three());
    for n in &r {
        assert!(n.x >= pane.x, "left of the pane: {} < {}", n.x, pane.x);
        assert!(n.y >= pane.y, "above the pane: {} < {}", n.y, pane.y);
        assert!(n.x + n.w <= pane.x + pane.w, "right of the pane");
        assert!(n.y + n.h <= pane.y + pane.h, "below the pane");
    }
    assert_eq!((r[2].y + r[2].h) as f32, (pane.y + pane.h) as f32 - MARGIN);
    assert_eq!((r[0].x + r[0].w) as f32, (pane.x + pane.w) as f32 - MARGIN);
    eprintln!(
        "\n  R16/D3 — pane {:?}: 3 notifications all inside, newest bottom-right at {:?}\n",
        (pane.x, pane.y, pane.w, pane.h), (r[2].x, r[2].y, r[2].w, r[2].h)
    );
}
