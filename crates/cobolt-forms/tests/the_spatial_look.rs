// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! The Spatial look (operator, 2026-10-04): what a control on a Spatial form
//! wears by default — the look of PowerDemo3's Buttons example — and that a
//! switch away from Spatial takes all of it back off, except what the
//! developer chose.

use cobolt_forms::model::{
    Control, ControlType, Form, GlassStyle, PropValue, SPATIAL_FORM_BACKGROUND,
    SPATIAL_FORM_TRANSPARENCY,
};

fn s(c: &Control, k: &str) -> String {
    c.get_prop(k).map(|v| v.to_xml_string()).unwrap_or_default()
}

#[test]
fn a_spatial_control_wears_the_buttons_example_look() {
    let mut b = Control::new("B", ControlType::Button, 0, 0);
    b.apply_spatial_defaults();
    for (k, v) in [
        ("BackgroundColor", "#36383EB3"),
        ("BackgroundGradientEnabled", "1"),
        ("BackgroundGradientStartColor", "#4E4E4EB3"),
        ("BackgroundGradientEndColor", "#000000B3"),
        ("BackgroundGradientDirection", "South"),
        ("ForegroundColor", "#FFFFFFFF"),
        ("BorderStyle", "None"),
        ("ShadowEnabled", "1"),
        ("ShadowOpacity", "6"),
        ("ShadowDirection", "SouthEast"),
        ("ShadowDistance", "7"),
        ("ShadowBlurStrength", "8"),
        ("Transparency", "0"),
    ] {
        assert_eq!(s(&b, k), v, "Button {k}");
    }
    let mut t = Control::new("T", ControlType::TextBox, 0, 0);
    t.apply_spatial_defaults();
    assert_eq!(s(&t, "BackgroundColor"), "#36383EFF", "only buttons and containers are see-through");
    let mut l = Control::new("L", ControlType::Label, 0, 0);
    l.apply_spatial_defaults();
    assert_eq!(s(&l, "ShadowEnabled"), "0", "a Label has no face to shade");
    assert_eq!(s(&l, "ForegroundColor"), "#FFFFFFFF");
}

#[test]
fn a_form_switched_away_from_spatial_keeps_nothing_of_it_but_the_developers_own() {
    let mut f = Form::new("F", "F", 400, 300);
    let mut p = Control::new("P", ControlType::Panel, 0, 0);
    let mut inner = Control::new("IN", ControlType::Button, 0, 0);
    inner.parent = Some("P".into());
    p.children.push(inner);
    f.controls.push(p);
    f.controls.push(Control::new("MINE", ControlType::Button, 0, 0));

    f.apply_look_defaults_with(true, GlassStyle::Classic, None);
    assert_eq!(f.background_color, SPATIAL_FORM_BACKGROUND);
    assert_eq!(i64::from(f.transparency), SPATIAL_FORM_TRANSPARENCY);
    assert_eq!(s(&f.controls[0].children[0], "BackgroundGradientEnabled"), "1", "children too");
    // The developer makes one button their own.
    f.controls[1].set_prop("BackgroundColor", PropValue::String("#123456FF".into()));

    f.apply_look_defaults_with(false, GlassStyle::Classic, None);
    let fresh = Form::new("_", "_", 400, 300);
    assert_eq!(f.background_color, fresh.background_color);
    assert_eq!(f.transparency, fresh.transparency);
    assert_eq!(s(&f.controls[0], "BackgroundGradientEnabled"), "0");
    assert_eq!(s(&f.controls[0].children[0], "BorderStyle"), s(&Control::new("x", ControlType::Button, 0, 0), "BorderStyle"));
    assert_eq!(s(&f.controls[1], "BackgroundColor"), "#123456FF", "the developer's value stays");
}

/// What a Spatial card holds keeps its own colours: the card's face is
/// see-through in its colours, so nothing fades what is inside it. It used to
/// be `Transparency` 30 on the card, which a Panel passes on to its contents —
/// a white bulb in a card came out `#C1C1C1` (operator, 2026-10-04).
#[test]
fn what_a_spatial_card_holds_is_drawn_at_full_strength() {
    let mut card = Control::new("CARD", ControlType::Panel, 0, 0);
    card.apply_spatial_defaults();
    let mut pic = Control::new("PIC", ControlType::PictureBox, 10, 10);
    pic.parent = Some("CARD".into());
    let controls = vec![card, pic];
    assert_eq!(cobolt_forms::containers::ancestor_opacity(&controls, 1), 1.0);
    let face = cobolt_forms::paint::parse_color(&s(&controls[0], "BackgroundGradientStartColor"));
    assert!(face.a() < 255 && face.a() > 150, "the card's face itself is see-through: {face:?}");
}
