// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Spec 110: a TextBox caption survives save and reopen with exactly the same
//! attributes inherited and set, and a form with no caption is unchanged.

use cobolt_forms::{form_to_string, load_form_from_str, Control, ControlType, Form, PropValue};
use std::time::Instant;

fn form_with(f: impl FnOnce(&mut Control)) -> Form {
    let mut form = Form::new("F", "F", 400, 200);
    let mut tb = Control::new("TXT-NOME", ControlType::TextBox, 120, 40);
    tb.set_prop("FontName", PropValue::String("Segoe UI".into()));
    tb.set_prop("FontSize", PropValue::Int(9));
    f(&mut tb);
    form.add_control(tb);
    form
}

fn reopened(form: &Form) -> Control {
    let xml = form_to_string(form).unwrap();
    let back = load_form_from_str(&xml).unwrap();
    back.controls.into_iter().find(|c| c.id == "TXT-NOME").unwrap()
}

#[test]
fn caption_round_trip_report() {
    let mut cases = 0;
    let t0 = Instant::now();

    // Only Bold set: it stays set, every other attribute stays inherited.
    let c = reopened(&form_with(|t| {
        t.set_prop("CaptionText", PropValue::String("&Nome:".into()));
        t.set_prop("CaptionPosition", PropValue::String("Left".into()));
        t.set_prop("CaptionFontBold", PropValue::Bool(true));
    }));
    assert_eq!(c.caption().text, "&Nome:");
    assert_eq!(c.caption_is_set("CaptionFontBold"), Some(true));
    for n in ["CaptionFontName", "CaptionFontSize", "CaptionFontItalic", "CaptionFontUnderline", "CaptionForeColor"] {
        assert_eq!(c.caption_is_set(n), Some(false), "{n} must stay inherited");
    }
    assert!(c.caption().font.bold == Some(true));
    cases += 1;

    // A value equal to the box's, set on purpose, is still set after reopening.
    let c = reopened(&form_with(|t| t.set_prop("CaptionFontSize", PropValue::Int(9))));
    assert_eq!(c.caption_is_set("CaptionFontSize"), Some(true));
    cases += 1;

    // Everything set.
    let c = reopened(&form_with(|t| {
        t.set_prop("CaptionFontName", PropValue::String("Arial".into()));
        t.set_prop("CaptionFontSize", PropValue::Int(12));
        t.set_prop("CaptionFontBold", PropValue::Bool(false));
        t.set_prop("CaptionFontItalic", PropValue::Bool(true));
        t.set_prop("CaptionFontUnderline", PropValue::Bool(true));
        t.set_prop("CaptionForeColor", PropValue::String("#FF0000".into()));
    }));
    for n in cobolt_forms::caption::INHERITABLE {
        assert_eq!(c.caption_is_set(n), Some(true), "{n}");
    }
    cases += 1;

    // A form with no caption writes no Caption* and opens identical.
    let plain = form_with(|_| {});
    let xml = form_to_string(&plain).unwrap();
    assert!(!xml.contains("Caption"), "an old form must not gain caption keys");
    let c = reopened(&plain);
    assert!(c.caption().text.is_empty());
    assert!(cobolt_forms::caption::INHERITABLE.iter().all(|n| c.caption_is_set(n) == Some(false)));
    cases += 1;

    // Reset then save: back to inherited on disk.
    let mut form = form_with(|t| t.set_prop("CaptionFontSize", PropValue::Int(12)));
    form.controls[0].reset_caption(None);
    assert!(!form_to_string(&form).unwrap().contains("CaptionFontSize"));
    cases += 1;

    let ms = t0.elapsed().as_secs_f64() * 1000.0;
    println!("TextBox caption round trip (spec 110)");
    println!("  cases: only-Bold set, equal-to-host set, all set, old form, reset-then-save");
    println!("  save+reopen cycles: {} in {ms:.2} ms ({:.0} per second)", cases, f64::from(cases) / (ms / 1000.0).max(1e-9));
    println!("  passed {cases} / failed 0");
}
