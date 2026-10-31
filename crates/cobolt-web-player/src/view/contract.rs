// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! **The public styling contract, pinned** (WASM-007 Q4): the class every control's element carries,
//! the attributes a selector addresses it by, and the custom properties a host page may set. Once an
//! application ships, a developer's stylesheet depends on these names, so a change to one is a change
//! to a published interface: this module fails first, listing what differs, and the change is then made
//! on purpose (with the Developer's Guide, at the next release that may break a stylesheet).

use std::collections::BTreeSet;

use cobolt_forms::model::ControlType;

use super::{controls, Empty};
use crate::base_css::BASE_CSS;
use crate::css::{class_names, type_class};
use crate::kinds::{PlayerKind, TABLE};
use crate::tree::testing::tree_with;

/// The custom properties the base layer defines, which a host page can set on the mount element
/// (WASM-007 R9). Sorted.
const CUSTOM_PROPERTIES: &[&str] = &[
    "--pr-accent",
    "--pr-border",
    "--pr-danger",
    "--pr-field",
    "--pr-field-border",
    "--pr-field-text",
    "--pr-focus",
    "--pr-font",
    "--pr-radius",
    "--pr-surface",
    "--pr-surface-2",
    "--pr-text",
    "--pr-text-dim",
];

/// `--pr-…` names in `css`, with whether each is a definition (`--pr-x:`) or a use (`var(--pr-x)`).
fn custom_properties(css: &str) -> (BTreeSet<String>, BTreeSet<String>) {
    let (mut defined, mut used) = (BTreeSet::new(), BTreeSet::new());
    let bytes = css.as_bytes();
    let mut i = 0;
    while let Some(at) = css[i..].find("--pr-") {
        let start = i + at;
        let mut end = start + 5;
        while end < bytes.len() && (bytes[end].is_ascii_alphanumeric() || bytes[end] == b'-') {
            end += 1;
        }
        let name = css[start..end].to_owned();
        let after = css[end..].trim_start();
        let is_use = css[..start].ends_with("var(");
        if is_use {
            // `var(--pr-x, fallback)`: a property the views or the host page may set, with a default.
            if !after.starts_with(',') {
                used.insert(name);
            }
        } else if after.starts_with(':') {
            defined.insert(name);
        }
        i = end;
    }
    (defined, used)
}

#[test]
fn the_custom_properties_a_host_page_can_set_are_the_published_list() {
    let (defined, used) = custom_properties(BASE_CSS);
    // The base layer defines these and a few more for its own parts (`--pr-line`, the grid colours…):
    // the published ones must all be there; the others are the player's own business.
    let missing: Vec<&&str> = CUSTOM_PROPERTIES.iter().filter(|p| !defined.contains(**p)).collect();
    assert!(missing.is_empty(), "published custom properties the base layer no longer defines: {missing:?}");
    // Every property a rule reads with no fallback is defined: no `var(--pr-x)` resolves to nothing.
    let undefined: Vec<&String> = used.iter().filter(|u| !defined.contains(*u)).collect();
    assert!(undefined.is_empty(), "custom properties read with no fallback and never defined: {undefined:?}");
    println!("custom properties: {} published, {} defined, {} read", CUSTOM_PROPERTIES.len(), defined.len(), used.len());
}

/// What a control type's element is called: `pr-` and the type's name in lower case.
fn expected_class(type_name: &str) -> String {
    format!("pr-{}", type_name.to_ascii_lowercase().replace(':', "-"))
}

#[test]
fn every_control_type_has_its_class_its_id_and_its_data_name() {
    let mut checked = Vec::new();
    let names: Vec<String> = TABLE.iter().map(|r| r.control.to_owned()).collect();
    for type_name in &names {
        let (t, i) = tree_with(type_name, "X-1", &[]);
        let e = Empty::default();
        let cx = e.cx(&t);
        let v = controls::view(&cx, i);
        let want = expected_class(type_name);
        assert!(v.has_class("pr-control"), "{type_name}: pr-control");
        assert!(v.has_class(&want), "{type_name}: the class `{want}` is the contract; classes are {:?}", v.classes);
        assert_eq!(v.attr_of("id"), Some("X-1"), "{type_name}: the name is the id (R2)");
        assert_eq!(v.attr_of("data-name"), Some("X-1"), "{type_name}: the name is data-name (R2)");
        assert_eq!(v.key, "root");
        checked.push(want);
    }
    // The model's own types and the series' (Authentication, Canvas…) are all in the table.
    assert_eq!(names.len(), TABLE.len());
    let all: BTreeSet<String> = ControlType::ALL.iter().map(|t| t.as_str().to_owned()).collect();
    let in_table: BTreeSet<&String> = names.iter().collect();
    let unclassified: Vec<&String> = all.iter().filter(|a| !in_table.contains(a)).collect();
    assert!(unclassified.is_empty(), "ControlType::ALL entries with no row in the classification: {unclassified:?}");
    println!("class names pinned for {} control types; first few: {:?}", checked.len(), &checked[..6]);
}

#[test]
fn the_series_controls_keep_their_plain_names_whatever_the_model_calls_them() {
    for k in [PlayerKind::Authentication, PlayerKind::Canvas2D, PlayerKind::Canvas3D] {
        let name = k.name();
        let (t, i) = tree_with(name, "S-1", &[]);
        assert_eq!(type_class(&ControlType::from_str(name)), name.to_ascii_lowercase());
        let c = t.ctl(i);
        assert!(class_names(c).contains(&expected_class(name)), "{name}: {:?}", class_names(c));
    }
}

#[test]
fn css_class_adds_the_developers_classes_after_the_type_class_and_drops_what_is_not_a_class_name() {
    let (t, i) = tree_with("Button", "B-1", &[("CssClass", "primary  big_one\tx-1 bad;class url(x) a.b")]);
    let c = t.ctl(i);
    let classes = class_names(c);
    assert_eq!(&classes[..2], ["pr-control", "pr-button"], "the type's classes come first");
    assert_eq!(&classes[2..], ["primary", "big_one", "x-1"], "only names made of letters, digits, - and _ pass");
}

#[test]
fn the_states_a_stylesheet_may_select_are_the_browsers_own_pseudo_classes() {
    // R3: `:hover`, `:focus-visible`, `:disabled`, `:checked` work because the elements are the native ones.
    let e = Empty::default();
    let (t, i) = tree_with("Button", "B-1", &[("Enabled", "0")]);
    let v = controls::view(&e.cx(&t), i);
    assert_eq!(v.tag, "button");
    assert!(v.prop_of("disabled").is_some() || v.attr_of("disabled").is_some() || v.has_class("pr-disabled"), "a disabled button is disabled to the browser: {}", v.describe());
    let (t, i) = tree_with("CheckBox", "C-1", &[("Checked", "1")]);
    let v = controls::view(&e.cx(&t), i);
    let input = v.find(&|n| n.tag == "input").expect("a native check box");
    assert_eq!(input.attr_of("type"), Some("checkbox"));
    assert!(input.prop_of("checked").is_some(), ":checked follows the model");
    assert!(BASE_CSS.contains(":focus-visible"), "the base layer styles keyboard focus");
}
