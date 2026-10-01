// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Spec 079 — the starting templates: each is responsive and modern, lays out
//! cleanly at its own size and larger (nothing leaves its container, no two
//! controls start to overlap, nothing gets narrower than designed), survives a
//! save and a load, and keeps its text readable before and after a switch of
//! glass style.

use cobolt_forms::layout::{min_size_of, solve, LRect, LayoutInput};
use cobolt_forms::model::GlassStyle;
use cobolt_forms::style::{contrast, ensure_text_contrast, text_background, MIN_TEXT_CONTRAST};
use cobolt_forms::templates::{build, FormTemplate, Texts};
use cobolt_forms::{form_to_string, load_form_from_str, Form, PropValue};

fn overlap(a: &LRect, b: &LRect) -> bool {
    a.x + 1.0 < b.x + b.w && b.x + 1.0 < a.x + a.w && a.y + 1.0 < b.y + b.h && b.y + 1.0 < a.y + a.h
}

fn rgb(hex: &str) -> (u8, u8, u8) {
    let h = hex.trim_start_matches('#');
    let p = |i: usize| u8::from_str_radix(&h[i..i + 2], 16).unwrap();
    (p(0), p(2), p(4))
}

/// Every text control's contrast where the form states its background:
/// (checked, worst ratio).
fn text_contrast(f: &Form) -> (usize, f32) {
    let mut n = 0;
    let mut worst = 21.0f32;
    for c in &f.controls {
        let Some(PropValue::String(fg)) = c.properties.get("ForegroundColor") else { continue };
        if !matches!(
            c.control_type,
            cobolt_forms::ControlType::Label | cobolt_forms::ControlType::Button | cobolt_forms::ControlType::TextBox
        ) {
            continue;
        }
        let Some(bg) = text_background(f, c, true) else { continue };
        n += 1;
        worst = worst.min(contrast(rgb(fg), bg));
    }
    (n, worst)
}

#[test]
fn every_template_lays_out_reads_and_round_trips() {
    let mut report = Vec::new();
    for t in FormTemplate::ALL {
        let f = build(t, "TPL-FORM", "Customers", None, &Texts::default());
        assert!(f.responsive, "{t:?} is responsive");
        assert_eq!(f.control_style, "modern", "{t:?} is modern");

        // Save and load: the same form comes back.
        let xml = form_to_string(&f).unwrap();
        assert!(xml.contains(r#"control-style="modern""#) && xml.contains(r#"responsive="true""#));
        let back = load_form_from_str(&xml).unwrap();
        assert_eq!(back.controls.len(), f.controls.len(), "{t:?}");
        assert_eq!(back.control_style, "modern");

        // Lay out at the designed size and larger.
        let design = (f.width as f32, f.height as f32);
        let solve_at = |w: f32, h: f32| solve(&LayoutInput::new(&f.controls, design, (w, h), &f.layout, &f.breakpoints)).rects;
        let base = solve_at(design.0, design.1);
        for (w, h) in [design, (design.0 + 400.0, design.1 + 240.0), (1920.0, 1080.0)] {
            let r = solve_at(w, h);
            for c in &f.controls {
                let cr = &r[&c.id];
                let (px, py, pw, ph) = match c.parent.as_ref().and_then(|p| r.get(p)) {
                    Some(p) => (p.x, p.y, p.w, p.h),
                    None => (0.0, 0.0, w, h),
                };
                assert!(
                    cr.x >= px - 1.0 && cr.y >= py - 1.0 && cr.x + cr.w <= px + pw + 1.0 && cr.y + cr.h <= py + ph + 1.0,
                    "{t:?} at {w}x{h}: {} leaves its container ({cr:?} in {px},{py},{pw},{ph})",
                    c.id
                );
                assert!(cr.w >= base[&c.id].w - 0.5, "{t:?} at {w}x{h}: {} narrower than designed", c.id);
            }
            for (i, a) in f.controls.iter().enumerate() {
                for b in f.controls.iter().skip(i + 1) {
                    if a.parent == b.parent {
                        assert!(!overlap(&r[&a.id], &r[&b.id]), "{t:?} at {w}x{h}: {} overlaps {}", a.id, b.id);
                    }
                }
            }
        }
        let min = min_size_of(&f).unwrap();

        // Readable as built.
        let (checked, worst) = text_contrast(&f);
        assert!(worst >= MIN_TEXT_CONTRAST, "{t:?}: worst contrast {worst:.2}");

        // And after every glass style: the contrast guard keeps it so.
        let mut after = Vec::new();
        for style in [GlassStyle::NeumorphicDark, GlassStyle::Neumorphic, GlassStyle::Enhanced, GlassStyle::Classic] {
            let mut g = f.clone();
            g.apply_glass_style_defaults_with(style, None);
            let fixed = ensure_text_contrast(&mut g, true);
            let (_, w2) = text_contrast(&g);
            assert!(w2 >= MIN_TEXT_CONTRAST, "{t:?} under {style:?}: worst contrast {w2:.2}");
            after.push(format!("{style:?} {w2:.1} ({} fixed)", fixed.len()));
        }
        report.push(format!(
            "  {t:?}: {} controls, minimum window {:.0}x{:.0}, {checked} text controls, worst contrast {worst:.1}; after a style switch: {}",
            f.controls.len(),
            min.0,
            min.1,
            after.join(", ")
        ));
    }
    println!("Spec 079 templates:\n{}", report.join("\n"));
}
