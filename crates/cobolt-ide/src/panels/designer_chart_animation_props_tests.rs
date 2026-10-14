// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! **A chart's three animation properties are stored when the Properties pane writes them.**
//!
//! "Animate on load", "Animate value changes" and "Animation duration (ms)" are written by the
//! pane as `AnimateOnLoad`, `AnimateValues` and `AnimationDuration`. `DesignerPanel::set_property`
//! reserves keys that start with `Anim` for the animation LIST — `Anim0_Duration`, `Anim2_Kind`,
//! the field of animation number N — and its early `return` sat outside the check for the `_`,
//! so every key that merely started with `Anim` was taken for one of those and dropped before it
//! was stored. The checkbox could be clicked and nothing changed.
//!
//! The animation list's own keys must keep working, so this also pins that they do.

use super::*;
use cobolt_forms::model::{AnimationDef, PropValue};
use cobolt_forms::{Control, ControlType, Form};

const CHARTS: [ControlType; 7] = [
    ControlType::BarChart,
    ControlType::LineChart,
    ControlType::PieChart,
    ControlType::AreaChart,
    ControlType::ScatterChart,
    ControlType::DonutChart,
    ControlType::RadarChart,
];

fn panel_with(ty: &ControlType) -> DesignerPanel {
    let mut form = Form::new("F", "F", 800, 600);
    form.controls.push(Control::new("C", ty.clone(), 10, 10));
    DesignerPanel::new(form)
}

fn stored(d: &DesignerPanel, key: &str) -> Option<PropValue> {
    d.form.find_control("C").and_then(|c| c.get_prop(key).cloned())
}

#[test]
fn the_pane_can_turn_on_a_charts_value_animation_and_set_its_duration() {
    let writes = [
        ("AnimateValues", PropValue::Bool(true)),
        ("AnimateOnLoad", PropValue::Bool(false)),
        ("AnimationDuration", PropValue::Int(1200)),
    ];
    println!("\n  chart type      AnimateValues   AnimateOnLoad   AnimationDuration   undo restores");
    let (mut checked, mut failures) = (0usize, Vec::new());
    for ty in &CHARTS {
        let mut d = panel_with(ty);
        let before: Vec<_> = writes.iter().map(|(k, _)| stored(&d, k)).collect();
        for (key, value) in &writes {
            d.set_property("C", key, value.clone());
        }
        let after: Vec<_> = writes.iter().map(|(k, _)| stored(&d, k)).collect();
        for ((key, want), got) in writes.iter().zip(&after) {
            checked += 1;
            if got.as_ref() != Some(want) {
                failures.push(format!("{ty:?} {key}: wrote {want:?}, the control holds {got:?}"));
            }
        }
        // One undo step per write: three undos put every value back as it was.
        for _ in &writes {
            d.undo();
        }
        let undone: Vec<_> = writes.iter().map(|(k, _)| stored(&d, k)).collect();
        let restored = undone == before;
        if !restored {
            failures.push(format!("{ty:?}: three undos left {undone:?}, expected {before:?}"));
        }
        println!(
            "  {:<14}  {:<14}  {:<14}  {:<18}  {}",
            format!("{ty:?}"),
            format!("{:?}", after[0]),
            format!("{:?}", after[1]),
            format!("{:?}", after[2]),
            restored
        );
    }
    println!("  {checked} writes over {} chart types, {} failures", CHARTS.len(), failures.len());
    assert!(failures.is_empty(), "{failures:#?}");
}

#[test]
fn a_key_for_one_animation_of_the_list_is_still_routed_to_that_animation() {
    let mut d = panel_with(&ControlType::PictureBox);
    d.form.find_control_mut("C").unwrap().animations.push(AnimationDef::new("anim1"));
    let default_ms = d.form.find_control("C").unwrap().animations[0].duration_ms;
    d.set_property("C", "Anim0_Duration", PropValue::Int(777));
    let c = d.form.find_control("C").unwrap();
    println!("\n  animation 0 duration: {default_ms} ms -> {} ms (Anim0_Duration)", c.animations[0].duration_ms);
    assert_eq!(c.animations[0].duration_ms, 777, "the list's own key must keep editing the list");
    assert!(
        c.get_prop("Anim0_Duration").is_none(),
        "and it must not also be stored as a property of the control"
    );
    // An index that is out of range is consumed, not stored as a stray property.
    d.set_property("C", "Anim9_Duration", PropValue::Int(5));
    assert!(d.form.find_control("C").unwrap().get_prop("Anim9_Duration").is_none());
}
