// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! WASM-036 R20 / AC15: what the stylesheet engine costs on a big form. 200 controls and 500 rules
//! are generated here; the whole recompute (every control's effective values, nothing cached), a
//! second pass with the cache warm, and a pass after one control changed are measured. Budget:
//! the whole recompute in at most 100 ms in the debug build (16 ms in a release build, recorded
//! in WASM-PROGRESS.md).

use std::time::Instant;

use cobolt_forms::css::{MediaEnv, Scheme};
use cobolt_forms::css_effective::{EffectiveCache, Origin, SheetSource};
use cobolt_forms::{Control, ControlType, Form, PropValue};

fn fixture() -> (Form, String, String) {
    let mut form = Form::new("BIG", "Big", 1280, 800);
    let kinds = [ControlType::Button, ControlType::Label, ControlType::TextBox, ControlType::Panel, ControlType::CheckBox];
    for i in 0..200 {
        let mut c = Control::new(format!("CTL-{i}"), kinds[i % kinds.len()].clone(), (i as i32 % 10) * 100, (i as i32 / 10) * 30);
        c.properties.insert("CssClass".into(), PropValue::String(format!("c{} grp{}", i % 17, i % 5)));
        if i % 3 == 0 {
            c.properties.insert("ForegroundColor".into(), PropValue::String("#112233FF".into()));
        }
        if i % 7 == 0 {
            c.properties.insert("FontSize".into(), PropValue::String("13".into()));
        }
        form.controls.push(c);
    }
    // Project sheet: 250 rules; form sheet: 250 rules.
    let mut project = String::from(":host { --accent: #3366cc }\n");
    let mut sheet = String::new();
    for r in 0..250 {
        match r % 5 {
            0 => project.push_str(&format!(".c{} {{ color: var(--accent); padding: {}px }}\n", r % 17, r % 9)),
            1 => project.push_str(&format!(".pr-button.grp{} {{ border-radius: {}px; cursor: pointer }}\n", r % 5, r % 12)),
            2 => project.push_str(&format!("@media (min-width: {}px) {{ .grp{} {{ font-size: {}px }} }}\n", 400 + r, r % 5, 10 + r % 8)),
            3 => project.push_str(&format!("[data-name^=\"CTL-{}\"] {{ background-color: #{:02x}{:02x}{:02x} }}\n", r % 20, r % 255, (r * 3) % 255, (r * 7) % 255)),
            _ => project.push_str(&format!(".pr-surface .pr-label.c{} {{ font-weight: 600; text-align: center }}\n", r % 17)),
        }
        match r % 5 {
            0 => sheet.push_str(&format!("#CTL-{} {{ color: #{:06x}; margin: 0 }}\n", r % 200, r * 9973 % 0xFFFFFF)),
            1 => sheet.push_str(&format!(".grp{} > .pr-control {{ cursor: help }}\n", r % 5)),
            2 => sheet.push_str(&format!(".pr-textbox.c{} {{ border: {}px solid red }}\n", r % 17, 1 + r % 4)),
            3 => sheet.push_str(&format!(".pr-panel:hover {{ background-color: #{:06x} }}\n", r * 7919 % 0xFFFFFF)),
            _ => sheet.push_str(&format!(".c{}.grp{} {{ box-shadow: 1px 1px 2px #000 }}\n", r % 17, r % 5)),
        }
    }
    (form, project, sheet)
}

#[test]
fn the_cascade_of_a_big_form_stays_inside_its_budget() {
    let (form, project, sheet) = fixture();
    let sources = [
        SheetSource { label: "webforms/application.css", text: &project, origin: Origin::Project },
        SheetSource { label: "webforms/BIG.css", text: &sheet, origin: Origin::Form },
    ];
    let env = MediaEnv { width: 1280.0, height: 800.0, scheme: Scheme::Light };
    let ids: Vec<String> = form.controls.iter().map(|c| c.id.clone()).collect();
    let rules = project.lines().count() + sheet.lines().count();

    let mut cache = EffectiveCache::new();
    let t = Instant::now();
    let mut entries = 0;
    for id in &ids {
        entries += cache.effective(&form, id, &sources, "spatial", &env).entries.len();
    }
    let cold = t.elapsed().as_secs_f64() * 1000.0;
    let after_cold = cache.stats;

    let t = Instant::now();
    for id in &ids {
        entries += cache.effective(&form, id, &sources, "spatial", &env).entries.len();
    }
    let warm = t.elapsed().as_secs_f64() * 1000.0;
    let after_warm = cache.stats;

    // One control changes: only it is computed again.
    let mut edited = form.clone();
    edited.controls[10].properties.insert("ForegroundColor".into(), PropValue::String("#FF0000FF".into()));
    for id in &ids {
        cache.effective(&edited, id, &sources, "spatial", &env);
    }
    let after_edit = cache.stats;
    // One sheet changes: everything is computed again, the sheets parsed once.
    let changed = format!("{sheet}#CTL-1 {{ color: red }}\n");
    let sources2 = [sources[0], SheetSource { label: "webforms/BIG.css", text: &changed, origin: Origin::Form }];
    let t = Instant::now();
    for id in &ids {
        cache.effective(&edited, id, &sources2, "spatial", &env);
    }
    let after_sheet = t.elapsed().as_secs_f64() * 1000.0;

    assert_eq!(after_cold.sheet_parses, 1);
    assert_eq!(after_cold.computed, 200);
    assert_eq!(after_warm.computed, 200, "a lookup with nothing changed computes nothing");
    assert_eq!(after_warm.hits, 200);
    assert_eq!(after_edit.computed, 201, "one control changed: one control computed");
    assert_eq!(cache.stats.sheet_parses, 2, "one sheet changed: parsed once more");
    let budget = if cfg!(debug_assertions) { 100.0 } else { 16.0 };
    assert!(cold <= budget, "the whole recompute took {cold:.1} ms, the budget is {budget} ms");

    let per_sec = (ids.len() * rules) as f64 / (cold / 1000.0);
    println!("── css_effective_perf ─────────────────────────────────────────────");
    println!("  fixture:               {} controls x {} rules ({} entries computed)", ids.len(), rules, entries);
    println!("  whole recompute:       {cold:.2} ms   (budget {budget} ms, {} build)", if cfg!(debug_assertions) { "debug" } else { "release" });
    println!("  cache hit, 200 lookups: {warm:.3} ms   (computed 0)");
    println!("  sheet change, recompute: {after_sheet:.2} ms   (parsed once)");
    println!("  one control edited:     computed 1 of 200");
    println!("  throughput:             {per_sec:.0} controls x rules per second");
    println!("  result:                 5 of 5 checks passed");
}
