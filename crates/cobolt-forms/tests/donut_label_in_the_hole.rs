// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! **A label in a donut's hole follows the donut when the form is laid out at another size** (2.0.413).
//!
//! PowerAnalyticsWeb's two donut cards write a figure and a caption in the hole of the ring ("43.701 / fundos"). The labels are
//! separate controls, centred on the chart in the design; at any other window size they stay on the ring only if the solver
//! moves them as it moves the chart. The first donut's chart is anchored `Top,Left,Bottom` (it takes the card's change in
//! height) while its two labels were anchored `Top,Left` (they kept their distance from the card's top), so on a window shorter
//! than the design the ring rose and the labels stayed: the figure sat on the ring's lower edge.
//!
//! This test lays the example's main form out at the design size and at smaller and larger ones, and asks where the chart and
//! its two labels are. The middle of the two labels together must stay on the middle of the chart's box. (The ring is drawn in
//! the middle of the box since 2.0.413; the browser test `page_chart_centre` measures that in Chrome.)
//!
//! Measured before the change, on a window like the one in the bug report (1100 x 760): the first card's labels were 24.5 px
//! below the ring's middle, the second card's 9.5 px at every size (the figure and its caption were designed 7 px low).
//! Now the second card's are within a pixel at every size; the first card's, which have no top or bottom anchor, keep
//! their centre's share of the card's height while the chart takes the whole change, so they agree with it to a few pixels
//! (the tolerance is the one in the table below), against 24.5 before.

use std::path::Path;

use cobolt_forms::layout::{solve, LayoutInput, LayoutOutput};
use cobolt_forms::load_form_from_str;

/// (chart, big label, caption label, the most the labels' middle may be off the chart's, in pixels) of each donut card of the
/// example.
const CARDS: &[(&str, &str, &str, f32)] = &[("CH-DONUT1", "LBL-D1-C", "LBL-D1-S", 4.5), ("CH-DONUT2", "LBL-D2-C", "LBL-D2-S", 1.5)];

fn mid_y(o: &LayoutOutput, id: &str) -> f32 {
    let r = o.rects[id];
    r.y + r.h / 2.0
}

fn mid_x(o: &LayoutOutput, id: &str) -> f32 {
    let r = o.rects[id];
    r.x + r.w / 2.0
}

#[test]
fn the_labels_in_a_donuts_hole_stay_on_the_chart_at_every_window_size() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/PowerAnalyticsWeb/webforms/main-form.wcfrm");
    let xml = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    let form = load_form_from_str(&xml).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    let design = (form.width as f32, form.height as f32);
    let mut report = Vec::new();
    let mut failures = Vec::new();
    for &(w, h) in &[(design.0, design.1), (1280.0, 800.0), (1100.0, 760.0), (1440.0, 1000.0), (1920.0, 1080.0)] {
        let o = solve(&LayoutInput::new(&form.controls, design, (w, h), &form.layout, &form.breakpoints));
        for &(chart, big, cap, tolerance) in CARDS {
            let (cy, cx) = (mid_y(&o, chart), mid_x(&o, chart));
            let (ly, lx) = ((mid_y(&o, big) + mid_y(&o, cap)) / 2.0, (mid_x(&o, big) + mid_x(&o, cap)) / 2.0);
            let (dx, dy) = (lx - cx, ly - cy);
            let line = format!("{w:.0}x{h:.0} {chart}: chart box {:.0}x{:.0}, labels' middle off by ({dx:+.1}, {dy:+.1}) px (allowed {tolerance})", o.rects[chart].w, o.rects[chart].h);
            if dx.abs() > tolerance || dy.abs() > tolerance {
                failures.push(line.clone());
            }
            report.push(line);
        }
    }
    println!("labels in the donuts' holes, {} window sizes x {} cards:\n  {}", report.len() / CARDS.len(), CARDS.len(), report.join("\n  "));
    assert!(failures.is_empty(), "a label left its donut's hole:\n  {}", failures.join("\n  "));
}
