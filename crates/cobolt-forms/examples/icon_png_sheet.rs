// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! One RASTER contact sheet of catalogue icons, so hand-drawn artwork can be
//! looked at rather than only compiled.
//!
//! ```text
//! cargo run -p cobolt-forms --features render --example icon_png_sheet -- <out.png> [category …]
//! ```
//!
//! The sibling [`icon_sheet`](icon_sheet) example writes per-category **SVG**
//! sheets and is the one to reach for when reviewing the whole catalogue. This
//! one exists for a narrower job: a single PNG, openable anywhere, drawing each
//! icon twice — at 56 px to judge the drawing and at 16 px to judge whether it
//! survives a menu row. An icon that reads at tile size and turns to mud at row
//! size is not finished, and only seeing both together tells you which you have.
//!
//! With no category argument, every category is drawn.

use cobolt_forms::icons::{icon_svg, MENU_ICON_CATEGORIES};

fn main() {
    let mut args = std::env::args().skip(1);
    let out = args.next().unwrap_or_else(|| "/tmp/icon_sheet.png".to_owned());
    let wanted: Vec<String> = args.map(|a| a.to_ascii_lowercase()).collect();

    let names: Vec<&str> = MENU_ICON_CATEGORIES
        .iter()
        .filter(|(cat, _)| {
            wanted.is_empty() || wanted.iter().any(|w| cat.to_ascii_lowercase().contains(w))
        })
        .flat_map(|(_, n)| n.iter().copied())
        .collect();
    if names.is_empty() {
        eprintln!("no icons matched");
        std::process::exit(1);
    }

    // Big enough to judge the drawing, plus a 16 px copy to judge legibility.
    const CELL: usize = 96;
    const COLS: usize = 10;
    let rows = names.len().div_ceil(COLS);
    let (w, h) = (COLS * CELL, rows * CELL);

    let mut body = String::new();
    body.push_str(&format!(
        r##"<rect width="{w}" height="{h}" fill="#101418"/>"##
    ));
    for (i, name) in names.iter().enumerate() {
        let (cx, cy) = ((i % COLS) * CELL, (i / COLS) * CELL);
        let Some(svg) = icon_svg(name, "#e6edf3") else {
            continue;
        };
        // The emitter returns a whole <svg viewBox="0 0 24 24">; nest it.
        let big = svg.replacen(
            "<svg ",
            &format!(r#"<svg x="{}" y="{}" width="56" height="56" "#, cx + 8, cy + 8),
            1,
        );
        body.push_str(&big);
        let small = svg.replacen(
            "<svg ",
            &format!(r#"<svg x="{}" y="{}" width="16" height="16" "#, cx + 70, cy + 10),
            1,
        );
        body.push_str(&small);
        body.push_str(&format!(
            r##"<text x="{}" y="{}" fill="#8b98a5" font-family="monospace" font-size="8" text-anchor="middle">{}</text>"##,
            cx + CELL / 2,
            cy + CELL - 8,
            name
        ));
    }
    let doc = format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="{w}" height="{h}" viewBox="0 0 {w} {h}">{body}</svg>"#
    );

    let opt = resvg::usvg::Options::default();
    let tree = resvg::usvg::Tree::from_str(&doc, &opt).expect("the sheet is valid SVG");
    let mut pixmap = resvg::tiny_skia::Pixmap::new(w as u32, h as u32).expect("pixmap");
    resvg::render(
        &tree,
        resvg::tiny_skia::Transform::identity(),
        &mut pixmap.as_mut(),
    );
    pixmap.save_png(&out).expect("write png");
    println!("{} icons → {out} ({w}×{h})", names.len());
}
