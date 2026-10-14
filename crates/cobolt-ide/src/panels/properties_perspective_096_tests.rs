// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Spec 096, task T12 — **the Properties pane offers the 3D-perspective section to the twelve
//! controls that only draw, and to no other, in every language the IDE speaks.**
//!
//! The pane is drawn for a control of each type in each of the six languages and the words it
//! paints are read back: the section's title and its three rows must be in the language of the
//! pane — not one of them left in English — for the twelve, and must not be there at all for a
//! control that takes input or holds children.
//!
//! Spec 096 AC12 (R17) in part, R1, R16.

use cobolt_forms::{Control, ControlType, Form};

use crate::i18n::Language;
use crate::panels::properties::PropertiesPanel;
use crate::ui_text::probe::texts;

/// The English wording of the section and of its three rows.
const SECTION: &str = "3D perspective";
const ROWS: [&str; 3] = ["Tilt top / bottom", "Tilt left / right", "Distance from observer"];

/// The twelve of spec 096 R1.
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

/// What the pane paints for one control of type `ct`, in `lang`.
fn painted(ct: &ControlType, lang: Language) -> Vec<String> {
    let make = || {
        let mut form = Form::new("F", "F", 800, 600);
        form.controls.push(Control::new("C-1", ct.clone(), 10, 10));
        (PropertiesPanel::new(), form)
    };
    texts(lang, &make, &|(panel, form), ui, tr| {
        let ctrl = form.controls[0].clone();
        let _ = panel.show(ui, form, Some(&ctrl), &[], tr);
    })
}

/// The wording of `english` in `lang`, from the pane's table.
fn said(lang: Language, english: &str) -> String {
    crate::ui_text::begin(lang);
    let t = crate::ui_text::tx(english).to_owned();
    crate::ui_text::begin(Language::English);
    t
}

#[test]
fn the_section_is_drawn_for_each_of_the_twelve_in_each_language_and_none_is_left_in_english() {
    println!("\n  ── Spec 096 T12: the 3D perspective section, 12 controls × 6 languages ──");
    println!("  {:<13} {:<11} {:<6} {}", "control", "language", "found", "title as drawn");
    let mut checked = 0usize;
    for ct in THE_TWELVE {
        for &lang in Language::ALL {
            let drawn = painted(&ct, lang);
            let has = |s: &str| drawn.iter().any(|d| d.eq_ignore_ascii_case(s) || d == s || d.contains(s));
            let title = said(lang, SECTION);
            let mut found = usize::from(has(&title));
            for row in ROWS {
                found += usize::from(has(&said(lang, row)));
            }
            println!("  {:<13} {:<11} {found}/4    {title}", format!("{ct:?}"), format!("{lang:?}"));
            assert_eq!(found, 4, "{ct:?} in {lang:?}: the section or one of its rows is missing: {drawn:?}");
            if lang != Language::English {
                for english in std::iter::once(SECTION).chain(ROWS) {
                    assert!(
                        !drawn.iter().any(|d| d.eq_ignore_ascii_case(english)),
                        "{ct:?} in {lang:?}: {english:?} is left in English"
                    );
                    assert_ne!(said(lang, english), english, "{lang:?}: {english:?} has no translation");
                }
            }
            checked += 1;
        }
    }
    assert_eq!(checked, 12 * 6);
}

#[test]
fn a_control_that_takes_input_or_holds_children_has_no_such_section() {
    let others = [
        ControlType::Button,
        ControlType::TextBox,
        ControlType::ComboBox,
        ControlType::CheckBox,
        ControlType::Slider,
        ControlType::DataGrid,
        ControlType::Panel,
        ControlType::GroupBox,
        ControlType::ProgressBar,
    ];
    for ct in others {
        let drawn = painted(&ct, Language::English);
        for english in std::iter::once(SECTION).chain(ROWS) {
            assert!(
                !drawn.iter().any(|d| d.eq_ignore_ascii_case(english)),
                "{ct:?} must not offer {english:?}: {drawn:?}"
            );
        }
    }
    println!("\n  {} other controls: none offers the 3D perspective section", 9);
}

/// IntelliSense after `CHART-1::` lists the three properties, and after `BTN-1::` does not — the
/// list the editor completes from is the one the save-time gate checks a handler against, so a
/// write of `RotateY` to a Button is refused while the developer is typing.
#[test]
fn intellisense_lists_the_three_properties_for_the_twelve_and_only_them() {
    use crate::panels::editor::build_known_controls;
    let mut form = Form::new("F", "F", 800, 600);
    for (n, ct) in THE_TWELVE.iter().enumerate() {
        form.controls.push(Control::new(format!("C-{n}"), ct.clone(), 0, 0));
    }
    form.controls.push(Control::new("BTN-1", ControlType::Button, 0, 0));
    form.controls.push(Control::new("TXT-1", ControlType::TextBox, 0, 0));
    let known = build_known_controls(&form);
    let mut with = 0;
    for k in &known {
        let lists = |p: &str| k.properties.iter().any(|x| x.eq_ignore_ascii_case(p));
        let twelve = THE_TWELVE.iter().any(|t| format!("{t:?}") == k.ctrl_type);
        for p in ["RotateX", "RotateY", "PerspectiveDistance"] {
            assert_eq!(lists(p), twelve, "{} ({}): {p}", k.id, k.ctrl_type);
        }
        with += usize::from(twelve);
    }
    println!("\n  IntelliSense: {with} of {} controls list RotateX, RotateY and PerspectiveDistance", known.len());
    assert_eq!(with, 12);
}
