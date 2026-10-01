// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Spec 056 R18, extended (operator, 2026-10-01): a responsive form's window
//! stops shrinking or growing where two controls that are apart in the design
//! would touch, or where a control inside its parent in the design would
//! cross the parent's edge.
//!
//! Checked on the four starting templates and on every form shipped in
//! `examples/`, each laid out responsively. At the computed limits nothing
//! that was apart touches. One pixel past a limit that the collision itself
//! set (not the declared floor), something does. The search's cost is
//! reported per form, because a window computes it when it opens and again
//! whenever its controls change.

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::time::Instant;

use cobolt_forms::layout::{size_limits_of, solve, window_min_size, LRect, LayoutInput, LayoutOutput};
use cobolt_forms::templates::{build, FormTemplate, Texts};
use cobolt_forms::{load_form_from_str, Form};

fn touching(a: &LRect, b: &LRect) -> bool {
    let gx = (b.x - a.right()).max(a.x - b.right());
    let gy = (b.y - a.bottom()).max(a.y - b.bottom());
    gx <= 0.01 && gy <= 0.01
}

fn solved(f: &Form, w: f32, h: f32) -> LayoutOutput {
    solve(&LayoutInput::new(&f.controls, (f.width as f32, f.height as f32), (w, h), &f.layout, &f.breakpoints))
}

/// The client area `parent` lays its children out in: the form's laid-out
/// area at top level, a laid-out container's client, or none.
fn client(o: &LayoutOutput, parent: Option<&str>) -> Option<LRect> {
    match parent {
        None => Some(LRect::new(0.0, 0.0, o.laid_out_size.0, o.laid_out_size.1)),
        Some(p) => o.containers.get(p).map(|g| g.client),
    }
}

fn inside(r: &LRect, c: &LRect) -> bool {
    r.x >= c.x - 0.01 && r.y >= c.y - 0.01 && r.right() <= c.right() + 0.01 && r.bottom() <= c.bottom() + 0.01
}

/// Pairs apart at the designed size that touch at (w, h), and controls
/// inside their parent at the designed size that cross its edge at (w, h) —
/// except an item of a `Flow` with `WrapContents` off, which is one line,
/// clipped. Siblings on different TabControl pages are never on screen
/// together.
fn collisions(f: &Form, w: f32, h: f32) -> Vec<String> {
    let (base_o, now_o) = (solved(f, f.width as f32, f.height as f32), solved(f, w, h));
    let (base, hidden) = (&base_o.rects, &base_o.hidden);
    let (now, now_hidden) = (&now_o.rects, &now_o.hidden);
    let ids: HashSet<&str> = f.controls.iter().map(|c| c.id.as_str()).collect();
    let is_tabs = |p: &Option<String>| {
        p.as_deref().is_some_and(|p| f.controls.iter().any(|c| c.id == p && c.control_type == cobolt_forms::ControlType::TabControl))
    };
    let shown: Vec<_> = f
        .controls
        .iter()
        .filter(|c| c.visible && !c.control_type.is_non_visual() && !hidden.contains(&c.id) && base.contains_key(&c.id))
        .collect();
    let mut out = Vec::new();
    for (i, a) in shown.iter().enumerate() {
        for b in shown.iter().skip(i + 1) {
            if a.parent != b.parent || (is_tabs(&a.parent) && a.tab.unwrap_or(0) != b.tab.unwrap_or(0)) || touching(&base[&a.id], &base[&b.id]) {
                continue;
            }
            if now_hidden.contains(&a.id) || now_hidden.contains(&b.id) {
                continue;
            }
            if touching(&now[&a.id], &now[&b.id]) {
                out.push(format!("{} / {}", a.id, b.id));
            }
        }
    }
    let one_line = |p: Option<&str>| {
        let (mode, wrap) = match p {
            None => (f.layout.get("LayoutMode").map(|v| v.as_str().to_owned()), f.layout.get("WrapContents").map(|v| v.as_bool())),
            Some(p) => match f.controls.iter().find(|c| c.id == p) {
                Some(c) => (c.get_prop("LayoutMode").map(|v| v.as_str().to_owned()), c.get_prop("WrapContents").map(|v| v.as_bool())),
                None => (None, None),
            },
        };
        mode.is_some_and(|m| m.trim().eq_ignore_ascii_case("flow")) && !wrap.unwrap_or(true)
    };
    for c in &shown {
        let parent = c.parent.clone().filter(|p| ids.contains(p.as_str()));
        if one_line(parent.as_deref()) {
            continue;
        }
        let (Some(bc), Some(nc)) = (client(&base_o, parent.as_deref()), client(&now_o, parent.as_deref())) else { continue };
        if inside(&base[&c.id], &bc) && !now_hidden.contains(&c.id) && now.get(&c.id).is_some_and(|r| !inside(r, &nc)) {
            out.push(format!("{} leaves its parent", c.id));
        }
    }
    out
}

fn example_forms() -> Vec<PathBuf> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).ancestors().nth(2).unwrap().join("examples");
    let mut out = Vec::new();
    let mut stack = vec![root];
    while let Some(d) = stack.pop() {
        let Ok(rd) = std::fs::read_dir(&d) else { continue };
        for e in rd.flatten() {
            let p = e.path();
            if p.is_dir() {
                stack.push(p);
            } else if p.extension().is_some_and(|x| x == "cfrm") {
                out.push(p);
            }
        }
    }
    out.sort();
    out
}

#[test]
fn a_resized_form_stops_before_its_controls_touch() {
    let mut forms: Vec<(String, Form)> = FormTemplate::ALL
        .iter()
        .map(|t| (format!("template {t:?}"), build(*t, "TPL", "Customers", None, &Texts::default())))
        .collect();
    for p in example_forms() {
        let Ok(text) = std::fs::read_to_string(&p) else { continue };
        let Ok(mut f) = load_form_from_str(&text) else { continue };
        f.responsive = true;
        let name = p.components().rev().take(3).collect::<Vec<_>>().into_iter().rev().map(|c| c.as_os_str().to_string_lossy().to_string()).collect::<Vec<_>>().join("/");
        forms.push((name, f));
    }
    assert!(forms.len() > 4, "the example forms were found");

    let mut rows = Vec::new();
    let (mut constrained, mut slowest) = (0usize, (0.0f64, String::new()));
    for (name, f) in &forms {
        let t = Instant::now();
        let l = size_limits_of(f).unwrap();
        let ms = t.elapsed().as_secs_f64() * 1000.0;
        if ms > slowest.0 {
            slowest = (ms, name.clone());
        }
        let designed = (f.width as f32, f.height as f32);
        let floor = window_min_size(&f.controls, designed, &f.layout, &f.breakpoints);

        // At every limit, nothing that was apart touches and nothing leaves
        // its parent.
        for (w, h) in [(l.min.0, designed.1), (designed.0, l.min.1), (l.max.0, designed.1), (designed.0, l.max.1)] {
            if w.is_finite() && h.is_finite() {
                let c = collisions(f, w, h);
                assert!(c.is_empty(), "{name} at {w}x{h} (limits {l:?}): {c:?}");
            }
        }
        // One step past a limit the collision set, something touches.
        if l.min.0 > floor.0 + 0.5 {
            assert!(!collisions(f, l.min.0 - 1.5, designed.1).is_empty(), "{name}: the width floor {l:?} is not tight");
        }
        if l.min.1 > floor.1 + 0.5 {
            assert!(!collisions(f, designed.0, l.min.1 - 1.5).is_empty(), "{name}: the height floor {l:?} is not tight");
        }
        if l.max.0.is_finite() {
            assert!(!collisions(f, l.max.0 + 1.5, designed.1).is_empty(), "{name}: the width ceiling {l:?} is not tight");
        }
        if l.max.1.is_finite() {
            assert!(!collisions(f, designed.0, l.max.1 + 1.5).is_empty(), "{name}: the height ceiling {l:?} is not tight");
        }
        let raised = l.min.0 > floor.0 + 0.5 || l.min.1 > floor.1 + 0.5 || l.max.0.is_finite() || l.max.1.is_finite();
        if raised {
            constrained += 1;
        }
        let fmt = |v: f32| if v.is_finite() { format!("{v:.0}") } else { "∞".into() };
        rows.push(format!(
            "  {name:<60} {:>4}x{:<4} {} controls  min {}x{} (declared {:.0}x{:.0})  max {}x{}  {ms:6.1} ms",
            f.width,
            f.height,
            f.controls.len(),
            fmt(l.min.0),
            fmt(l.min.1),
            floor.0,
            floor.1,
            fmt(l.max.0),
            fmt(l.max.1),
        ));
    }
    println!(
        "Resize limits — {} forms ({} templates + {} example forms laid out responsively):\n{}\n  \
         {constrained} forms gained a limit from a collision; every limit is collision-free and tight to 1.5 px; \
         slowest search {:.1} ms ({}).",
        forms.len(),
        FormTemplate::ALL.len(),
        forms.len() - FormTemplate::ALL.len(),
        rows.join("\n"),
        slowest.0,
        slowest.1
    );
}
