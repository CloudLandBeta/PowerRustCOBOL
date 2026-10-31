// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Spec 110, the designer's half of the TextBox caption: a press on the caption is a press on the
//! box, the lasso reaches the box through its caption, "Restore default" is one undo step that
//! returns an attribute to inherited, and snap keeps using the box alone.

use super::*;
use cobolt_forms::model::PropValue;
use cobolt_forms::{Control, ControlType, Form};
use std::time::Instant;

fn panel() -> DesignerPanel {
    let mut d = DesignerPanel::new(Form::new("F", "F", 640, 480));
    let mut tb = Control::new("TXT-NOME", ControlType::TextBox, 100, 100);
    tb.rect.w = 200;
    tb.rect.h = 22;
    tb.set_prop("FontSize", PropValue::Int(14));
    tb.set_prop("CaptionText", PropValue::String("&Nome:".into()));
    d.form.controls.push(tb);
    d
}

#[test]
fn caption_designer_report() {
    let t0 = Instant::now();
    let mut passed = 0;
    let mut d = panel();
    let cap = d.form.controls[0].caption_layout().unwrap().rect;
    let (cx, cy) = ((cap.x + cap.w / 2.0) as i32, (cap.y + cap.h / 2.0) as i32);

    // A press on the caption, which lies above the box, hits the box.
    assert!(!d.form.controls[0].rect.contains(cx, cy), "the point is outside the box");
    assert_eq!(d.hit_top_id(cx, cy).as_deref(), Some("TXT-NOME"));
    passed += 1;

    // The lasso rectangle: outer bounds reach a rubber band that only touches the caption.
    let rects = d.active_tab_rects();
    let outer = rects.iter().find(|(id, _)| id == "TXT-NOME").unwrap().1;
    assert!(outer.y < d.form.controls[0].rect.y && outer.h > d.form.controls[0].rect.h);
    passed += 1;

    // Snap and align keep reading the inner bounds: `rect` is still the box.
    assert_eq!((d.form.controls[0].rect.y, d.form.controls[0].rect.h), (100, 22));
    passed += 1;

    // "Restore default": set, then back to inherited, as one undo step each way.
    d.set_property("TXT-NOME", "CaptionFontSize", PropValue::Int(20));
    assert_eq!(d.form.controls[0].caption_is_set("CaptionFontSize"), Some(true));
    d.set_property(
        "TXT-NOME",
        "CaptionFontSize",
        PropValue::String(cobolt_forms::caption::INHERIT.into()),
    );
    assert_eq!(d.form.controls[0].caption_is_set("CaptionFontSize"), Some(false));
    assert_eq!(d.form.controls[0].caption_font().size, 14.0, "inherited again: the box's size");
    d.undo();
    assert_eq!(d.form.controls[0].caption_is_set("CaptionFontSize"), Some(true), "undo restores the set value");
    d.undo();
    assert_eq!(d.form.controls[0].caption_is_set("CaptionFontSize"), Some(false), "and back to never set");
    passed += 1;

    let ms = t0.elapsed().as_secs_f64() * 1000.0;
    println!("TextBox caption in the designer (spec 110)");
    println!("  cases: caption hit selects the box, lasso on outer bounds, inner bounds for snap, restore default with undo");
    println!("  {ms:.2} ms; passed {passed} / failed 0");
}
