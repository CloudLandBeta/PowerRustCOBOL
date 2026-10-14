// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Spec 096, task T2 — **the three properties exist, and mean the same everywhere.**
//!
//! `RotateX`, `RotateY` and `PerspectiveDistance` belong to the twelve controls
//! that only *draw*. This file states, as numbers a person can read, what that
//! means for the model: who carries them, what they default to, how a write is
//! brought into range, and what a control that must not carry them does when a
//! hand-edited `.cfrm` names them anyway.
//!
//! Spec 096 AC1 (R1, R2, R13, R16), AC13 (R18).

use cobolt_forms::model::{
    clamp_perspective_value, property_names_for, property_runtime_text, Control, ControlType,
    Form, GlassStyle, PropValue, PERSPECTIVE_DISTANCE_PROP, PERSPECTIVE_DISTANCE_REFERENCE,
    PERSPECTIVE_STRENGTH_MAX, PERSPECTIVE_STRENGTH_PROP, ROTATE_X_PROP, ROTATE_Y_PROP,
};
use cobolt_forms::{form_to_string, load_form_from_str};

/// The twelve of spec 096 R1, written out so a thirteenth cannot slip in
/// without this list — and the spec — being touched.
const THE_TWELVE: [ControlType; 12] = [
    ControlType::BarChart,
    ControlType::LineChart,
    ControlType::PieChart,
    ControlType::AreaChart,
    ControlType::ScatterChart,
    ControlType::DonutChart,
    ControlType::RadarChart,
    ControlType::PictureBox,
    ControlType::Animator,
    ControlType::Label,
    ControlType::Line,
    ControlType::Shape,
];

/// The three of spec 096 and `PerspectiveStrength` of spec 097.
const THE_THREE: [&str; 4] = [ROTATE_X_PROP, ROTATE_Y_PROP, PERSPECTIVE_DISTANCE_PROP, PERSPECTIVE_STRENGTH_PROP];

fn int(c: &Control, key: &str) -> Option<i64> {
    c.get_prop(key).map(|v| v.as_i64())
}

#[test]
fn exactly_twelve_types_have_perspective_and_they_are_the_spec_twelve() {
    let with: Vec<ControlType> = ControlType::ALL
        .iter()
        .filter(|ct| ct.has_perspective())
        .cloned()
        .collect();
    println!(
        "\n  control types: {} · with perspective: {}",
        ControlType::ALL.len(),
        with.len()
    );
    for ct in &with {
        println!("    {ct:?}");
    }
    assert_eq!(with.len(), 12, "the spec names twelve types (R1)");
    for ct in THE_TWELVE {
        assert!(with.contains(&ct), "{ct:?} is one of the twelve of R1");
    }
}

#[test]
fn each_of_the_twelve_seeds_the_three_properties_neutral() {
    println!("\n  type            RotateX  RotateY  PerspectiveDistance  PerspectiveStrength");
    for ct in THE_TWELVE {
        let c = Control::new("X-1", ct.clone(), 0, 0);
        println!(
            "  {:<14} {:>8} {:>8} {:>20} {:>20}",
            format!("{ct:?}"),
            int(&c, ROTATE_X_PROP).unwrap(),
            int(&c, ROTATE_Y_PROP).unwrap(),
            int(&c, PERSPECTIVE_DISTANCE_PROP).unwrap(),
            int(&c, PERSPECTIVE_STRENGTH_PROP).unwrap()
        );
        assert_eq!(c.get_prop(ROTATE_X_PROP), Some(&PropValue::Int(0)), "{ct:?} RotateX");
        assert_eq!(c.get_prop(ROTATE_Y_PROP), Some(&PropValue::Int(0)), "{ct:?} RotateY");
        assert_eq!(
            c.get_prop(PERSPECTIVE_DISTANCE_PROP),
            Some(&PropValue::Int(PERSPECTIVE_DISTANCE_REFERENCE)),
            "{ct:?} PerspectiveDistance"
        );
        assert_eq!(
            c.get_prop(PERSPECTIVE_STRENGTH_PROP),
            Some(&PropValue::Int(PERSPECTIVE_STRENGTH_MAX)),
            "{ct:?} PerspectiveStrength is seeded at full strength (spec 097)"
        );
        // IntelliSense lists them: the names come from the seed.
        let names = property_names_for(&format!("{ct:?}"));
        for key in THE_THREE {
            assert!(names.iter().any(|n| n == key), "{ct:?}: IntelliSense lacks {key}");
        }
    }
}

#[test]
fn no_other_type_seeds_or_lists_them() {
    let mut checked = 0;
    for ct in ControlType::ALL.iter().filter(|ct| !ct.has_perspective()) {
        let c = Control::new("X-1", ct.clone(), 0, 0);
        let names = property_names_for(&format!("{ct:?}"));
        for key in THE_THREE {
            assert!(c.get_prop(key).is_none(), "{ct:?} must not seed {key}");
            assert!(!names.iter().any(|n| n == key), "{ct:?} must not list {key}");
        }
        checked += 1;
    }
    println!("\n  {checked} other control types: none seeds or lists the three properties");
    assert_eq!(checked + 12, ControlType::ALL.len());
}

#[test]
fn a_write_is_brought_into_range_for_every_one_of_the_twelve() {
    // The table printed is the contract of R12: nearest whole number inside the range.
    let cases: [(&str, &str, &str); 19] = [
        (ROTATE_X_PROP, "200", "60"),
        (ROTATE_X_PROP, "-99", "-60"),
        (ROTATE_X_PROP, "30", "30"),
        (ROTATE_X_PROP, "-30", "-30"),
        (ROTATE_Y_PROP, "61", "60"),
        (ROTATE_Y_PROP, "-61", "-60"),
        (ROTATE_Y_PROP, "12.6", "13"),
        (ROTATE_Y_PROP, "  7 ", "7"),
        (PERSPECTIVE_DISTANCE_PROP, "150", "100"),
        (PERSPECTIVE_DISTANCE_PROP, "-5", "0"),
        (PERSPECTIVE_DISTANCE_PROP, "75", "75"),
        (PERSPECTIVE_DISTANCE_PROP, "0", "0"),
        (PERSPECTIVE_DISTANCE_PROP, "100", "100"),
        // Spec 097 — the strength, 0 to 100.
        (PERSPECTIVE_STRENGTH_PROP, "150", "100"),
        (PERSPECTIVE_STRENGTH_PROP, "-5", "0"),
        (PERSPECTIVE_STRENGTH_PROP, "40", "40"),
        (PERSPECTIVE_STRENGTH_PROP, "100", "100"),
        (PERSPECTIVE_STRENGTH_PROP, "0", "0"),
        // Not a number: passed through untouched, as a boolean's unreadable text is.
        (ROTATE_X_PROP, "wobbly", "wobbly"),
    ];
    println!("\n  property              written  reads back");
    for (key, written, expect) in cases {
        for ct in THE_TWELVE {
            let got = property_runtime_text(&format!("{ct:?}"), key, written);
            assert_eq!(got, expect, "{ct:?}::{key} written {written:?}");
        }
        println!("  {key:<20} {written:>8}  {expect:>9}");
    }
    // The property name is not case-sensitive, like every COBOL name.
    assert_eq!(property_runtime_text("BarChart", "rotatex", "999"), "60");
    // Another type is left alone — a write to it is not a perspective write.
    assert_eq!(property_runtime_text("Button", "RotateX", "999"), "999");
    // A property that is not one of the three is not touched.
    assert_eq!(property_runtime_text("BarChart", "FontSize", "999"), "999");
    assert_eq!(clamp_perspective_value("FontSize", "5"), None);
}

#[test]
fn a_form_saved_before_the_feature_loads_neutral_and_saves_unchanged() {
    // A saved chart that never heard of the properties.
    let mut form = Form::new("F", "F", 640, 480);
    let mut chart = Control::new("CHART-1", ControlType::BarChart, 10, 10);
    for key in THE_THREE {
        chart.properties.shift_remove(key);
    }
    form.controls.push(chart);
    let xml = form_to_string(&form).unwrap();
    assert!(!xml.contains("RotateX"), "the old form must not name the properties");

    let loaded = load_form_from_str(&xml).unwrap();
    let c = &loaded.controls[0];
    for key in THE_THREE {
        assert!(c.get_prop(key).is_none(), "{key} must stay absent after load (R13)");
    }
    let resaved = form_to_string(&loaded).unwrap();
    assert_eq!(resaved, xml, "loading and saving an old form must not edit it");
    println!("\n  an old form: {} bytes in, {} bytes out, identical", xml.len(), resaved.len());
}

#[test]
fn the_three_properties_survive_a_save_and_a_load() {
    let mut form = Form::new("F", "F", 640, 480);
    let mut chart = Control::new("CHART-1", ControlType::BarChart, 10, 10);
    chart.set_prop(ROTATE_X_PROP, PropValue::Int(8));
    chart.set_prop(ROTATE_Y_PROP, PropValue::Int(-30));
    chart.set_prop(PERSPECTIVE_DISTANCE_PROP, PropValue::Int(60));
    chart.set_prop(PERSPECTIVE_STRENGTH_PROP, PropValue::Int(35));
    form.controls.push(chart);
    let loaded = load_form_from_str(&form_to_string(&form).unwrap()).unwrap();
    let c = &loaded.controls[0];
    assert_eq!(int(c, ROTATE_X_PROP), Some(8));
    assert_eq!(int(c, ROTATE_Y_PROP), Some(-30));
    assert_eq!(int(c, PERSPECTIVE_DISTANCE_PROP), Some(60));
    assert_eq!(int(c, PERSPECTIVE_STRENGTH_PROP), Some(35));
    println!("\n  RotateX 8 / RotateY -30 / PerspectiveDistance 60 / PerspectiveStrength 35 survive a save and a load");
}

#[test]
fn a_button_that_names_rotate_y_loads_and_is_unchanged() {
    // The loader keeps every <Property> it meets whatever the type (R16): "does
    // not have the property" can only mean "not offered, not read".
    let mut form = Form::new("F", "F", 640, 480);
    form.controls.push(Control::new("BTN-1", ControlType::Button, 10, 10));
    let plain = form_to_string(&form).unwrap();
    form.controls[0].set_prop(ROTATE_Y_PROP, PropValue::Int(30));
    let with = form_to_string(&form).unwrap();
    let loaded = load_form_from_str(&with).expect("a hand-edited form loads without an error");
    let b = &loaded.controls[0];
    assert_eq!(b.control_type, ControlType::Button);
    assert_eq!(int(b, ROTATE_Y_PROP), Some(30), "the loader keeps what it reads");
    assert_ne!(plain, with, "the property is in the file");
    // …and nothing about the button's geometry or events moved.
    let plain_form = load_form_from_str(&plain).unwrap();
    let p = &plain_form.controls[0];
    assert_eq!((b.rect.x, b.rect.y, b.rect.w, b.rect.h), (p.rect.x, p.rect.y, p.rect.w, p.rect.h));
    assert_eq!(b.tab_order, p.tab_order);
    assert_eq!(b.events.len(), p.events.len());
    println!("\n  a Button with RotateY=30 in its file: loads, keeps the text, geometry/tab/events equal");
}

#[test]
fn writing_the_properties_leaves_layout_tab_order_and_events_alone() {
    // AC13 / R18.
    let mut c = Control::new("CHART-1", ControlType::BarChart, 40, 50);
    c.tab_order = 3;
    c.events.push(cobolt_forms::model::EventBinding::new("onClick", "CHART-CLICK"));
    let before = (c.rect, c.tab_order, c.events.clone(), c.id.clone());
    c.set_prop(ROTATE_X_PROP, PropValue::Int(20));
    c.set_prop(ROTATE_Y_PROP, PropValue::Int(-45));
    c.set_prop(PERSPECTIVE_DISTANCE_PROP, PropValue::Int(90));
    assert_eq!((c.rect, c.tab_order, c.events.clone(), c.id.clone()), before);
    println!("\n  rect, tab order, event bindings and id are equal before and after the writes");
}

#[test]
fn a_theme_switch_leaves_the_three_properties_as_they_were() {
    for ct in THE_TWELVE {
        let mut c = Control::new("X-1", ct.clone(), 0, 0);
        c.set_prop(ROTATE_X_PROP, PropValue::Int(11));
        c.set_prop(ROTATE_Y_PROP, PropValue::Int(-22));
        c.set_prop(PERSPECTIVE_DISTANCE_PROP, PropValue::Int(33));
        c.set_prop(PERSPECTIVE_STRENGTH_PROP, PropValue::Int(44));
        c.apply_glass_style_defaults(GlassStyle::Neumorphic);
        c.apply_glass_style_defaults(GlassStyle::Classic);
        c.reset_theme_owned_props();
        assert_eq!(int(&c, ROTATE_X_PROP), Some(11), "{ct:?} RotateX after a theme switch");
        assert_eq!(int(&c, ROTATE_Y_PROP), Some(-22), "{ct:?} RotateY after a theme switch");
        assert_eq!(int(&c, PERSPECTIVE_DISTANCE_PROP), Some(33), "{ct:?} PerspectiveDistance after a theme switch");
        assert_eq!(int(&c, PERSPECTIVE_STRENGTH_PROP), Some(44), "{ct:?} PerspectiveStrength after a theme switch");
    }
    println!("\n  12 types: the four properties are not theme-owned — a theme switch and a reset left them");
}
