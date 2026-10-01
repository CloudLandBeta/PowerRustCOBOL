// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! A notification is high-contrast unless the developer chose otherwise
//! (operator, 2026-10-01).
//!
//! The defect: a Snackbar dropped on a form carries the colours every control
//! is seeded with — `BackgroundColor #F0F0F0`, `ForegroundColor #FFFFFF` — and
//! those were taken as the developer's choice, so a *critical* notification in
//! PowerDemo3 painted white text on near-white. A seeded value means "not
//! chosen": the category's own high-contrast colours apply, and an ink the
//! developer did not choose is kept readable on whatever background is in
//! effect. Colours the developer did choose are left alone.

use cobolt_forms::model::{Control, ControlType, PropValue};
use cobolt_forms::paint::{contrast_ratio, parse_color, readable_ink_on};
use cobolt_forms::snackbar::{mint, SnackVisual};

fn snack(set: &[(&str, &str)]) -> SnackVisual {
    let mut c = Control::new("SNACK-1", ControlType::Snackbar, 0, 0);
    c.set_prop("Text", PropValue::String("Get a valid API Key".into()));
    for (k, v) in set {
        c.set_prop(*k, PropValue::String((*v).into()));
    }
    mint(&c).0
}

/// The ink the painter uses: the same rule `draw_snackbar` applies.
fn painted_ink(v: &SnackVisual) -> egui::Color32 {
    let fg = parse_color(&v.foreground);
    readable_ink_on(v.foreground_chosen.then_some(fg), fg, parse_color(&v.background))
}

#[test]
fn a_notification_is_high_contrast_unless_the_developer_chose_otherwise() {
    let mut rows = Vec::new();
    for category in ["info", "warning", "error", "critical", "success"] {
        // As dropped: the seeded colours, which nobody chose.
        let v = snack(&[("Category", category), ("BackgroundColor", "#F0F0F0"), ("ForegroundColor", "#FFFFFF")]);
        assert_ne!(v.background.to_uppercase(), "#F0F0F0", "{category}: the seeded background is not a choice");
        assert!(!v.foreground_chosen, "{category}: the seeded ink is not a choice");
        let ratio = contrast_ratio(painted_ink(&v), parse_color(&v.background));
        assert!(ratio >= 4.5, "{category}: {} on {} is {ratio:.2}:1", v.foreground, v.background);
        rows.push(format!("{category} {:.1}:1", ratio));
    }

    // The developer chose a light background but no ink: the ink is made to read.
    let v = snack(&[("Category", "critical"), ("BackgroundColor", "#FAFAFA")]);
    assert_eq!(v.background, "#FAFAFA");
    let light = contrast_ratio(painted_ink(&v), parse_color("#FAFAFA"));
    assert!(light >= 4.5, "chosen light background, ink not chosen: {light:.2}:1");

    // The developer chose both, low contrast and all: their choice stands.
    let v = snack(&[("Category", "critical"), ("BackgroundColor", "#FAFAFA"), ("ForegroundColor", "#EEEEEE")]);
    assert!(v.foreground_chosen);
    assert_eq!(painted_ink(&v), parse_color("#EEEEEE"), "an explicit ink is never overridden");

    println!(
        "Snackbar contrast: seeded colours → category colours, {}; chosen light background → ink {light:.1}:1; both chosen → kept",
        rows.join(", ")
    );
}
