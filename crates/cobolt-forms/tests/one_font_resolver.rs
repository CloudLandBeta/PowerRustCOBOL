// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

#![cfg(feature = "render")]
//! **Spec 056 R70 / AC33 — one font resolver.**
//!
//! Every paint site that reads a control's `FontSize` goes through
//! `layout::fonts::resolve_font_size`, which applies the laid-out effective
//! size and accepts decimals. Two halves:
//!
//! 1. a source scan: no `FontSize` READ is left in `cobolt-forms` outside the
//!    resolver, tests and the Viewer's export (`paint_viewer_export`, which
//!    lays out a document for a file, not a control for a surface — excluded
//!    by the plan);
//! 2. a render walk: every control type, given `FontSize = "18.5"` and an
//!    effective size of 23, paints no text at the old 4 pt floor a decimal
//!    used to fall to, and the text controls paint at 23.

use std::collections::BTreeSet;
use std::path::PathBuf;

use cobolt_forms::containers::ActiveTabs;
use cobolt_forms::layout::fonts::EFFECTIVE_FONT_SIZE;
use cobolt_forms::model::Rect as MRect;
use cobolt_forms::render::{render_form, Backdrop, DesignedState, RenderInput, RenderMode};
use cobolt_forms::{Control, ControlType, PropValue};
use egui::{pos2, vec2, Rect};

/// Source with `#[cfg(test)]` items blanked (brace-matched), line numbers kept.
fn without_tests(src: &str) -> String {
    let mut b: Vec<char> = src.chars().collect();
    let pat: Vec<char> = "#[cfg(test)]".chars().collect();
    let mut i = 0;
    while i + pat.len() <= b.len() {
        if b[i..i + pat.len()] == pat[..] {
            let mut j = i + pat.len();
            while j < b.len() && b[j] != '{' && b[j] != ';' {
                j += 1;
            }
            if j < b.len() && b[j] == '{' {
                let mut depth = 0;
                while j < b.len() {
                    if b[j] == '{' {
                        depth += 1;
                    } else if b[j] == '}' {
                        depth -= 1;
                        if depth == 0 {
                            break;
                        }
                    }
                    j += 1;
                }
            }
            for k in i..=j.min(b.len() - 1) {
                if b[k] != '\n' {
                    b[k] = ' ';
                }
            }
            i = j + 1;
        } else {
            i += 1;
        }
    }
    b.into_iter().collect()
}

/// The body of `fn name` in `src` (brace-matched), to exclude it.
fn fn_span(src: &str, name: &str) -> Option<(usize, usize)> {
    let start = src.find(&format!("fn {name}("))?;
    let open = start + src[start..].find('{')?;
    let mut depth = 0;
    for (k, ch) in src[open..].char_indices() {
        match ch {
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    return Some((start, open + k));
                }
            }
            _ => {}
        }
    }
    None
}

#[test]
fn no_font_size_read_is_left_outside_the_resolver() {
    let src_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut files = Vec::new();
    let mut stack = vec![src_dir.clone()];
    while let Some(d) = stack.pop() {
        for e in std::fs::read_dir(&d).unwrap().flatten() {
            let p = e.path();
            if p.is_dir() {
                stack.push(p);
            } else if p.extension().and_then(|x| x.to_str()) == Some("rs") {
                files.push(p);
            }
        }
    }
    files.sort();
    // A read: the property looked up, not written or listed.
    let read_patterns = [
        r#"get_prop("FontSize")"#,
        r#""FontSize").parse"#,
        r#"sv(ctrl, "FontSize")"#,
        r#"int("FontSize""#,
        r#"prop_i64(ctrl, "FontSize""#,
        r#"prop_str(ctrl, "FontSize""#,
    ];
    let mut offenders = Vec::new();
    let mut scanned = 0;
    for f in &files {
        let rel = f.strip_prefix(&src_dir).unwrap().display().to_string();
        if rel == "layout/fonts.rs" {
            continue;
        }
        let raw = std::fs::read_to_string(f).unwrap();
        let mut code = without_tests(&raw);
        if rel == "paint.rs" {
            if let Some((a, b)) = fn_span(&code, "paint_viewer_export") {
                let blank: String = code[a..=b].chars().map(|c| if c == '\n' { '\n' } else { ' ' }).collect();
                code.replace_range(a..=b, &blank);
            }
        }
        scanned += 1;
        for (n, line) in code.lines().enumerate() {
            if read_patterns.iter().any(|p| line.contains(p)) {
                offenders.push(format!("src/{rel}:{}: {}", n + 1, line.trim()));
            }
        }
    }
    println!("── 056 AC33: FontSize reads outside the resolver ──");
    println!("  files scanned: {scanned} (tests, layout/fonts.rs and paint_viewer_export excluded)");
    println!("  reads found  : {}", offenders.len());
    assert!(offenders.is_empty(), "read FontSize through layout::fonts::resolve_font_size:\n{}", offenders.join("\n"));
}

/// Every text shape's font size inside `rect`.
fn text_sizes(shapes: &[egui::epaint::ClippedShape], rect: Rect) -> BTreeSet<u32> {
    fn walk(s: &egui::Shape, rect: Rect, out: &mut BTreeSet<u32>) {
        match s {
            egui::Shape::Vec(v) => v.iter().for_each(|x| walk(x, rect, out)),
            egui::Shape::Text(t) if rect.expand2(vec2(4.0, 24.0)).contains(t.pos) => {
                for sec in &t.galley.job.sections {
                    out.insert((sec.format.font_id.size * 10.0).round() as u32);
                }
            }
            _ => {}
        }
    }
    let mut out = BTreeSet::new();
    for cs in shapes {
        walk(&cs.shape, rect, &mut out);
    }
    out
}

#[test]
fn every_control_paints_at_the_resolved_size() {
    // Controls whose own text is painted at the control's font — the ones the
    // walk requires to show 23.
    let text_controls = [
        ControlType::Label,
        ControlType::Button,
        ControlType::TextBox,
        ControlType::CheckBox,
        ControlType::RadioButton,
        ControlType::GroupBox,
        ControlType::ListBox,
    ];
    let mut report = Vec::new();
    let mut floor_hits = Vec::new();
    let mut missing = Vec::new();
    for ct in ControlType::ALL {
        if ct.is_non_visual() {
            continue;
        }
        let mut c = Control::new("C", ct.clone(), 0, 0);
        c.rect = MRect::new(20, 20, 260, 120);
        c.set_prop("FontSize", PropValue::String("18.5".into()));
        c.set_prop(EFFECTIVE_FONT_SIZE, PropValue::String("23".into()));
        for (k, v) in [("Caption", "Aa"), ("Text", "Aa"), ("Items", "Aa\nBb")] {
            if c.get_prop(k).is_some() {
                c.set_prop(k, PropValue::String(v.into()));
            }
        }
        let controls = vec![c];
        let ctx = egui::Context::default();
        let active = ActiveTabs::new();
        let mut input = egui::RawInput::default();
        input.screen_rect = Some(Rect::from_min_size(pos2(0.0, 0.0), vec2(400.0, 300.0)));
        input.max_texture_side = Some(8192);
        let mut full = ctx.run_ui(input, |root| {
            egui::CentralPanel::default().frame(egui::Frame::NONE).show(root, |ui| {
                let inp = RenderInput {
                    controls: &controls,
                    state: &DesignedState,
                    form_size: vec2(400.0, 300.0),
                    glass: true,
                    mode: RenderMode::Interactive,
                    active_tabs: &active,
                    backdrop: Backdrop::default(),
                };
                render_form(ui, &inp);
            });
        });
        full.textures_delta.clear();
        let sizes = text_sizes(&full.shapes, Rect::from_min_size(pos2(20.0, 20.0), vec2(260.0, 120.0)));
        if sizes.contains(&40) {
            floor_hits.push(format!("{ct:?}"));
        }
        if text_controls.contains(&ct) && !sizes.contains(&230) {
            missing.push(format!("{ct:?} {sizes:?}"));
        }
        report.push(format!("{ct:?}: {:?}", sizes.iter().map(|s| *s as f32 / 10.0).collect::<Vec<_>>()));
    }
    println!("── 056 AC33: text sizes painted with FontSize 18.5, effective 23 ──");
    for r in &report {
        println!("  {r}");
    }
    assert!(floor_hits.is_empty(), "text painted at the 4 pt floor: {floor_hits:?}");
    assert!(missing.is_empty(), "text controls not painted at the resolved 23: {missing:?}");
}
