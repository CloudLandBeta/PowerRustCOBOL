// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Render the menu-icon catalogue as per-category SVG contact sheets, for
//! visual QA of the icon set (spec 018).
//!
//! ```bash
//! cargo run -p cobolt-forms --example icon_sheet -- /tmp/icon-sheets
//! ```
//!
//! Writes one `NN-<category>.svg` per catalogue category into the given
//! directory (default `./icon-sheets`). Feature-free: uses the same shape
//! data the egui renderer paints, through the crate's SVG emitter.

use cobolt_forms::icons::{icon_svg, icon_svg_styled, SvgIconEffect, MENU_ICON_CATEGORIES};

const COLS: usize = 8;
const CELL_W: f32 = 30.0;
const CELL_H: f32 = 36.0;
const MARGIN: f32 = 4.0;

fn main() {
    let out_dir = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "icon-sheets".to_string());
    std::fs::create_dir_all(&out_dir).expect("create output dir");

    for (idx, (cat, names)) in MENU_ICON_CATEGORIES.iter().enumerate() {
        let rows = names.len().div_ceil(COLS);
        let w = MARGIN * 2.0 + COLS as f32 * CELL_W;
        let h = MARGIN * 2.0 + rows as f32 * CELL_H + 8.0;
        let mut body = format!(
            r##"<rect x="0" y="0" width="{w}" height="{h}" fill="white"/><text x="{MARGIN}" y="7" font-size="4.5" font-family="Helvetica" fill="#333">{cat}</text>"##
        );
        let mut drawn = 0usize;
        for (i, name) in names.iter().enumerate() {
            let col = i % COLS;
            let row = i / COLS;
            let x = MARGIN + col as f32 * CELL_W + (CELL_W - 24.0) * 0.5;
            let y = MARGIN + 8.0 + row as f32 * CELL_H;
            match icon_svg(name, "#1a1a1a") {
                Some(svg) => {
                    drawn += 1;
                    let inner = svg
                        .replace("<svg ", &format!(r#"<svg x="{x}" y="{y}" width="24" height="24" "#));
                    body.push_str(&inner);
                }
                None => {
                    body.push_str(&format!(
                        r##"<rect x="{x}" y="{y}" width="24" height="24" fill="none" stroke="#d33" stroke-dasharray="2 2"/>"##
                    ));
                }
            }
            let cx = x + 12.0;
            let ty = y + 28.5;
            body.push_str(&format!(
                r##"<text x="{cx}" y="{ty}" font-size="2.5" font-family="Helvetica" text-anchor="middle" fill="#555">{name}</text>"##
            ));
        }
        let svg = format!(
            r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {w} {h}">{body}</svg>"#
        );
        let file = format!(
            "{out_dir}/{:02}-{}.svg",
            idx,
            cat.to_lowercase()
                .replace([' ', '/'], "-")
                .replace("--", "-")
        );
        std::fs::write(&file, svg).expect("write sheet");
        println!("{file}: {drawn}/{} drawn", names.len());
    }

    // Effects demo: the same VECTOR data at 128×128 with colours, accent,
    // drop shadow and neumorphic emboss — proving the styling engine.
    let demo_icons = ["gear", "heart", "truck", "dollar", "folder", "bell"];
    let styles: &[(&str, &str, Option<&str>, SvgIconEffect)] = &[
        ("plain", "#1a1a1a", None, SvgIconEffect::Plain),
        ("colored", "#1d6ae5", None, SvgIconEffect::Plain),
        ("accent", "#1d6ae5", Some("#e5701d"), SvgIconEffect::Plain),
        (
            "shadow",
            "#1a1a1a",
            None,
            SvgIconEffect::DropShadow { color: "#000000", opacity: 0.35, offset: 0.8, blur: 0.7 },
        ),
        (
            "neumorphic",
            "#5a6478",
            None,
            SvgIconEffect::Neumorphic { light: "#ffffff", dark: "#a3b1c6", offset: 0.7, blur: 0.6 },
        ),
    ];
    let cell = 148.0;
    let w = 40.0 + styles.len() as f32 * cell;
    let h = 60.0 + demo_icons.len() as f32 * cell;
    let mut body = format!(
        r##"<rect width="{w}" height="{h}" fill="#e6eaf2"/><text x="20" y="34" font-size="20" font-family="Helvetica" fill="#333">Icon styling — vectors at 128 px</text>"##
    );
    for (si, (label, _, _, _)) in styles.iter().enumerate() {
        let x = 20.0 + si as f32 * cell + cell * 0.5;
        body.push_str(&format!(
            r##"<text x="{x}" y="52" font-size="13" font-family="Helvetica" text-anchor="middle" fill="#555">{label}</text>"##
        ));
    }
    for (ii, icon) in demo_icons.iter().enumerate() {
        for (si, (_, color, accent, effect)) in styles.iter().enumerate() {
            let x = 20.0 + si as f32 * cell + (cell - 128.0) * 0.5;
            let y = 60.0 + ii as f32 * cell + (cell - 128.0) * 0.5;
            if let Some(svg) = icon_svg_styled(icon, color, *accent, effect) {
                let inner = svg.replace(
                    "<svg ",
                    &format!(r#"<svg x="{x}" y="{y}" width="128" height="128" "#),
                );
                body.push_str(&inner);
            }
        }
    }
    let file = format!("{out_dir}/zz-effects-demo.svg");
    std::fs::write(
        &file,
        format!(r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {w} {h}">{body}</svg>"#),
    )
    .expect("write demo");
    println!("{file}: effects demo ({} styles)", styles.len());
}
