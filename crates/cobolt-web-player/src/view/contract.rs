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

/// The custom properties every web theme defines, which a host page can set on the mount element
/// (WASM-007 R9): one list, in `cobolt_forms::web_themes`, which its own tests check against each theme.
use cobolt_forms::web_themes::CUSTOM_PROPERTIES;

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
    // What the page adopts: the base sheet and the default theme (Liquid Glass) as `pr-theme`.
    let (defined, used) = custom_properties(&format!("{}\n{}", crate::base_css::base_layer(), crate::base_css::theme_layer("")));
    // The base layer and the theme define these and a few more for their own parts (`--pr-line`, the grid colours…):
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

/// The element of a view that the keyboard reaches: a native control, or one with `tabindex="0"`.
fn focus_target(v: &super::VNode) -> Option<&super::VNode> {
    v.find(&|n| matches!(n.tag, "button" | "input" | "select" | "textarea" | "a") || n.attr_of("tabindex") == Some("0"))
}

/// R3: `:hover`, `:focus-visible`, `:disabled`, `:checked` — or the ARIA state the base styles — work on every control a
/// person can use, because the element a selector addresses is the native one or carries that state.
#[test]
fn the_states_a_stylesheet_may_select_are_the_browsers_own_pseudo_classes() {
    let e = Empty::default();
    // The types that hold a checked or selected state, and where a selector finds it.
    let checked: &[(&str, &[(&str, &str)])] = &[
        ("CheckBox", &[("Checked", "1")]),
        ("RadioButton", &[]),
        ("Switch", &[]),
        ("TabControl", &[]),
        ("TreeView", &[]),
        ("ListBox", &[("Items", "one\ntwo")]),
    ];
    let (mut interactive, mut display) = (Vec::new(), Vec::new());
    for r in TABLE {
        let extra: &[(&str, &str)] = checked.iter().find(|(n, _)| *n == r.control).map_or(&[], |(_, p)| p);
        let (t, i) = tree_with(r.control, "X-1", extra);
        let on = controls::view(&e.cx(&t), i);
        let Some(target) = focus_target(&on) else {
            display.push(r.control);
            continue;
        };
        let mut states = vec![":hover", ":focus-visible"];
        // Disabled: the native element's `disabled` (`:disabled`), and on every control the root's `aria-disabled` twin.
        let mut off_props = extra.to_vec();
        off_props.push(("Enabled", "0"));
        let (t, i) = tree_with(r.control, "X-1", &off_props);
        let off = controls::view(&e.cx(&t), i);
        assert_eq!(off.attr_of("aria-disabled"), Some("true"), "{}: a disabled control says so to a selector: {}", r.control, off.describe());
        let native_off = focus_target(&off).is_some_and(|n| matches!(n.tag, "button" | "input" | "select" | "textarea") && (n.prop_of("disabled").is_some() || n.attr_of("disabled").is_some()));
        states.push(if native_off { ":disabled" } else { "[aria-disabled]" });
        if checked.iter().any(|(n, _)| *n == r.control) {
            let native = on.find(&|n| n.tag == "input" && matches!(n.attr_of("type"), Some("checkbox" | "radio")));
            let twin = on.find(&|n| n.attr_of("aria-checked").is_some() || n.attr_of("aria-selected").is_some());
            assert!(native.is_some() || twin.is_some(), "{}: no element answers :checked or its ARIA twin: {}", r.control, on.describe());
            states.push(if native.is_some() { ":checked" } else if twin.unwrap().attr_of("aria-checked").is_some() { "[aria-checked]" } else { "[aria-selected]" });
        }
        if r.control == "CheckBox" {
            assert!(on.find(&|n| n.tag == "input").and_then(|n| n.prop_of("checked")).is_some(), ":checked follows the model");
        }
        interactive.push(format!("{} <{}> {}", r.control, target.tag, states.join(" ")));
    }
    // Every control a person types into, clicks or picks from is in the interactive list.
    for must in ["Button", "TextBox", "CheckBox", "RadioButton", "ComboBox", "ListBox", "TabControl", "DataGrid", "DateTimePicker", "NumericUpDown", "TreeView", "Slider", "Switch", "Knob", "Hyperlink", "Splitter", "Maps", "Viewer", "ToolBar", "SideMenu"] {
        assert!(interactive.iter().any(|l| l.starts_with(&format!("{must} "))), "{must} has no element the keyboard reaches");
    }
    assert!(BASE_CSS.contains(":focus-visible"), "the base layer styles keyboard focus");
    println!("states per interactive control type ({}):", interactive.len());
    for l in &interactive {
        println!("  {l}");
    }
    println!("display only ({}): {}", display.len(), display.join(", "));
}
