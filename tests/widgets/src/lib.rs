// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Shared helpers for per-widget property tests.
//!
//! For a given widget (`ControlType`) the harness:
//!   1. Creates the control (which populates its full default property set).
//!   2. Sets **every** property to a distinct test value of the same kind.
//!   3. Verifies the in-memory read-back (`get_prop`) returns the set value.
//!   4. Saves the form to a `.cfrm`, reloads it, and verifies every property
//!      survived the XML round-trip with the same value.
//!
//! A property "works" here = it is defined on the widget, settable, and
//! persisted losslessly. Values are compared via `PropValue::to_xml_string()`
//! so a property that is re-typed during parsing (e.g. Bool→String) still
//! passes as long as the logical value is preserved.

use cobolt_forms::model::PropValue;
use cobolt_forms::{load_form, save_form, Control, ControlType, Form};

/// Produce a test value distinct from `cur`, of the same `PropValue` kind.
pub fn mutate(cur: &PropValue) -> PropValue {
    match cur {
        PropValue::String(_) => PropValue::String("RTtest-XYZ-123".to_owned()),
        PropValue::Int(n) => PropValue::Int(n.wrapping_add(1234).wrapping_add(1)),
        PropValue::Bool(b) => PropValue::Bool(!b),
    }
}

#[derive(Debug)]
pub struct PropResult {
    pub name: String,
    pub stage: &'static str, // "set" (in-memory) or "reload" (after save/load)
    pub ok: bool,
    pub expected: String,
    pub got: String,
}

/// Run the full property check for one widget. Returns one `PropResult` per
/// (property × stage).
pub fn check_widget(ct: ControlType, label: &str) -> Vec<PropResult> {
    let mut results = Vec::new();

    let mut ctrl = Control::new("W1", ct, 10, 10);
    let names: Vec<String> = ctrl.properties.keys().cloned().collect();

    // 1+2+3: set each property to a distinct value and verify in-memory read-back.
    let mut expected: std::collections::HashMap<String, String> = std::collections::HashMap::new();
    for name in &names {
        let cur = ctrl
            .get_prop(name)
            .expect("property present at creation")
            .clone();
        let newv = mutate(&cur);
        let exp = newv.to_xml_string();
        expected.insert(name.clone(), exp.clone());
        ctrl.set_prop(name.clone(), newv);

        let got = ctrl
            .get_prop(name)
            .map(|v| v.to_xml_string())
            .unwrap_or_else(|| "<MISSING>".to_owned());
        results.push(PropResult {
            name: name.clone(),
            stage: "set",
            ok: got == exp,
            expected: exp,
            got,
        });
    }

    // 4: save → reload and verify every property persisted.
    let mut form = Form::new("TEST-FORM", "Widget Test", 800, 600);
    form.controls.push(ctrl);

    let path = std::env::temp_dir().join(format!("rcobol_widgettest_{label}.cfrm"));
    save_form(&form, &path).expect("save_form failed");
    let loaded = load_form(&path).expect("load_form failed");
    let _ = std::fs::remove_file(&path);

    let lctrl = loaded
        .controls
        .first()
        .expect("loaded form has the control");

    for name in &names {
        let exp = expected.get(name).cloned().unwrap_or_default();
        let got = lctrl
            .get_prop(name)
            .map(|v| v.to_xml_string())
            .unwrap_or_else(|| "<MISSING>".to_owned());
        results.push(PropResult {
            name: name.clone(),
            stage: "reload",
            ok: got == exp,
            expected: exp,
            got,
        });
    }

    results
}

/// Run the check and assert 100% of properties pass, printing a per-property
/// report (use `-- --nocapture` to see it).
pub fn assert_widget(ct: ControlType, label: &str) {
    let results = check_widget(ct, label);
    let total = results.len();
    let mut failures = 0;

    println!("\n=== Widget '{label}' — {total} property checks ===");
    for r in &results {
        let mark = if r.ok { "PASS" } else { "FAIL" };
        println!(
            "  [{mark}] {:<16} ({:<7}) expected='{}' got='{}'",
            r.name, r.stage, r.expected, r.got
        );
        if !r.ok {
            failures += 1;
        }
    }
    println!("  → {} passed, {failures} failed", total - failures);

    assert_eq!(
        failures, 0,
        "widget '{label}' had {failures}/{total} failing property checks"
    );
}
