// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! The shipped Maps example form loads, and is what it claims to be.
//!
//! `forms/maps/maps-demo.cfrm` in the demo project is the worked example for
//! markers, traced routes, sales regions and drive times. A worked example that
//! does not open is worse than none, and a form file is easy to get subtly
//! wrong by hand — so it is parsed here rather than trusted.
//!
//! The file lives in the operator's demo project, not in this repository, so
//! the test SKIPS when it is not present: a fresh clone must not fail for
//! missing something it was never given.

#![cfg(feature = "render")]

use std::path::PathBuf;

fn demo_form() -> Option<PathBuf> {
    let p = PathBuf::from(std::env::var("HOME").ok()?)
        .join("Documents/PowerDemo3/forms/Inner-Forms/maps-demo.cfrm");
    p.exists().then_some(p)
}

#[test]
fn the_maps_example_form_loads_and_is_embeddable() {
    let Some(path) = demo_form() else {
        eprintln!("PowerDemo3 not present — skipping");
        return;
    };
    let form = cobolt_forms::load_form(&path).expect("the example form must parse");

    assert_eq!(form.name, "MAPS-DEMO");
    assert_eq!(
        form.form_format,
        cobolt_forms::model::FormFormat::Embedded,
        "the example is loaded into a shell's ContentPane, so it must be Embedded"
    );
    assert_eq!(
        form.glass_style,
        cobolt_forms::model::GlassStyle::Neumorphic,
        "the operator asked for Neumorphic Light"
    );

    let map = form
        .controls
        .iter()
        .find(|c| c.control_type == cobolt_forms::ControlType::Maps)
        .expect("the example is about a Maps control");
    assert_eq!(map.id, "MAP-1");

    // Every capability the example claims to demonstrate has a button behind
    // it. A demo that silently loses one is the thing this guards.
    for id in [
        "BTN-MARKERS",
        "BTN-REGIONS",
        "BTN-ROUTE",
        "BTN-DRIVE",
        "BTN-CLEAR",
    ] {
        assert!(
            form.controls.iter().any(|c| c.id == id),
            "the example lost its {id} button"
        );
    }
}

/// The regions it draws are real geometry, and — the whole reason the fill is
/// triangulated — at least one of them is **concave**. A demo made entirely of
/// convex blobs would pass over the bug it exists to show working.
#[test]
fn the_example_regions_parse_and_at_least_one_is_concave() {
    let Some(path) = demo_form() else {
        eprintln!("PowerDemo3 not present — skipping");
        return;
    };
    let form = cobolt_forms::load_form(&path).expect("parses");
    let regions_code = form
        .controls
        .iter()
        .flat_map(|c| c.events.iter())
        .map(|e| e.code.as_str())
        .find(|code| code.contains("AddRegion"))
        .expect("the example draws regions");

    // Pull each geometry literal out of the COBOL and check it is a real ring.
    let mut rings = 0;
    let mut concave = 0;
    for line in regions_code.lines() {
        let t = line.trim();
        if !t.starts_with('"') || !t.contains(',') || !t.contains(';') {
            continue;
        }
        let geom = t.trim_matches('"');
        let pts = cobolt_forms::map_geometry::parse_geometry(geom);
        if pts.len() < 3 {
            continue;
        }
        rings += 1;
        let screen: Vec<(f32, f32)> = pts
            .iter()
            .map(|p| (p.lng as f32 * 1000.0, -p.lat as f32 * 1000.0))
            .collect();
        let tris = cobolt_forms::map_geometry::triangulate(&screen);
        assert_eq!(
            tris.len(),
            screen.len() - 2,
            "a {}-point territory must fill as {} triangles, got {}",
            screen.len(),
            screen.len() - 2,
            tris.len()
        );
        if !is_convex(&screen) {
            concave += 1;
        }
    }
    assert_eq!(rings, 5, "five salesmen, five territories");
    assert!(
        concave > 0,
        "every territory is convex — the example would not exercise the \
         triangulated fill at all"
    );
}

fn is_convex(pts: &[(f32, f32)]) -> bool {
    let n = pts.len();
    let mut sign = 0i32;
    for i in 0..n {
        let (a, b, c) = (pts[i], pts[(i + 1) % n], pts[(i + 2) % n]);
        let cross = (b.0 - a.0) * (c.1 - b.1) - (b.1 - a.1) * (c.0 - b.0);
        let s = if cross > 0.0 {
            1
        } else if cross < 0.0 {
            -1
        } else {
            0
        };
        if s == 0 {
            continue;
        }
        if sign == 0 {
            sign = s;
        } else if sign != s {
            return false;
        }
    }
    true
}
