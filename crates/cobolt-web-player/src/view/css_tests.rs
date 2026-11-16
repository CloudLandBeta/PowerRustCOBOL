// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! The tests of `css` (now in `cobolt-abi-host`) that read what this crate's view writes: they moved
//! here with DESKTOP-WASM-001 T27, unchanged but for the paths they read.

use crate::css::{appearance, border_ring};

/// The property layer has one name, the one `view/style.rs::wrap_layer` writes (WASM-007 plan C11).
#[test]
fn the_property_layer_is_named_once() {
    let stale = concat!("pr-", "properties");
    for (file, text) in [("css.rs", include_str!("../../../cobolt-abi-host/src/css.rs")), ("view/style.rs", include_str!("style.rs")), ("base_css.rs", include_str!("../../../cobolt-abi-host/src/base_css.rs"))] {
        assert!(!text.contains(stale), "{file} still names the property layer `{stale}`");
    }
    assert!(crate::base_css::LAYER_ORDER.contains("pr-props") && crate::view::style::wrap_layer(&[]).starts_with("@layer pr-props {"));
}

/// WASM-004 T13b: a chart's frame gradient is a ring over a transparent border, with the radius kept.
#[test]
fn a_charts_border_gradient_is_a_masked_ring_over_a_transparent_border() {
    let (t, i) = crate::tree::testing::tree_with(
        "BarChart",
        "CH-1",
        &[("BorderStyle", "Single"), ("BorderWidth", "3"), ("CornerRadius", "10"), ("BorderGradientEnabled", "1"), ("BorderGradientStartColor", "#FF0000"), ("BorderGradientEndColor", "#0000FF"), ("BorderGradientDirection", "East")],
    );
    let d = appearance(t.ctl(i));
    let get = |k: &str| d.iter().find(|(x, _)| x == k).map(|(_, v)| v.clone());
    assert_eq!(get("border").as_deref(), Some("3px solid transparent"), "{d:?}");
    assert_eq!(get("border-radius").as_deref(), Some("10px"));
    let ring = border_ring(t.ctl(i)).expect("a gradient frame has a ring");
    let rget = |k: &str| ring.iter().find(|(x, _)| x == k).map(|(_, v)| v.as_str());
    println!("ring: {ring:?}");
    assert_eq!(rget("background"), Some("linear-gradient(90deg, rgb(255 0 0), rgb(0 0 255))"));
    assert_eq!(rget("inset"), Some("-3px"));
    assert_eq!(rget("padding"), Some("3px"));
    assert_eq!(rget("border-radius"), Some("inherit"), "the corners keep the control's radius");
    assert_eq!(rget("mask-composite"), Some("exclude"));
    assert_eq!(rget("pointer-events"), Some("none"));
    // The rule reaches the control's stylesheet as its `::after`.
    let css = crate::view::style::control_css(0, &t.ctls, i, false);
    assert!(css.contains("[data-name=\"CH-1\"]::after {"), "{css}");
    // No gradient, no ring; a Button never has one.
    let (t, i) = crate::tree::testing::tree_with("BarChart", "CH-1", &[("BorderGradientEnabled", "0")]);
    assert!(border_ring(t.ctl(i)).is_none());
    let (t, i) = crate::tree::testing::tree_with("Button", "B-1", &[("BorderStyle", "Single"), ("BorderGradientEnabled", "1")]);
    assert!(border_ring(t.ctl(i)).is_none());
    // HideBackground takes the frame off, ring included.
    let (t, i) = crate::tree::testing::tree_with("PieChart", "CH-2", &[("BorderGradientEnabled", "1"), ("HideBackground", "1")]);
    assert!(border_ring(t.ctl(i)).is_none());
}

/// WASM-036 T4: the IDE's `props_layer` names the CSS properties this player writes from a control's
/// own properties (`control_decls`), over every visible control type, seeded, absent and edge values,
/// Spatial on and off. A difference is a row of the table that no longer says what the player does.
#[test]
fn props_layer_names_what_control_decls_writes() {
    use cobolt_forms::css_effective::props_layer;
    use std::collections::BTreeSet;
    let watched: BTreeSet<&str> = cobolt_forms::css_reverse::REVERSE.iter().map(|r| r.css).collect();
    let variants: &[&[(&str, &str)]] = &[
        &[],
        &[("BackgroundColor", "#F0F0F0FF")],
        &[("BackgroundColor", "#336699FF"), ("Transparency", "30")],
        &[("BackgroundColor", "#00000000")],
        &[("BackgroundGradientEnabled", "1"), ("BackgroundGradientStartColor", "#FF0000FF"), ("BackgroundGradientEndColor", "#0000FFFF")],
        &[("ForegroundColor", "#FFFFFFFF"), ("FontName", "Segoe UI"), ("FontSize", "14"), ("Italic", "1"), ("Underline", "1")],
        &[("FontName", ""), ("FontSize", "0"), ("Bold", "1"), ("Strikethrough", "1")],
        &[("CornerRadius", "0")],
        &[("CornerRadius", "8"), ("BorderStyle", "Single"), ("BorderWidth", "2"), ("BorderColor", "#FF0000FF")],
        &[("BorderStyle", "Raised")],
        &[("BorderStyle", "")],
        &[("ShadowEnabled", "1"), ("ShadowDistance", "6")],
        &[("Padding", "6"), ("PaddingLeft", "10")],
        &[("Padding", "0"), ("PaddingTop", "3"), ("PaddingBottom", "4")],
        &[("Cursor", "Hand"), ("ZOrder", "3")],
        &[("Cursor", ""), ("ZOrder", "0")],
        &[("Visible", "false")],
        &[("TextAlignment", "Right"), ("VerticalAlignment", "Bottom"), ("WordWrap", "1")],
    ];
    let (mut checked, mut diffs) = (0usize, Vec::new());
    for spatial in [false, true] {
        for t in cobolt_forms::ControlType::ALL {
            if t.is_non_visual() {
                continue;
            }
            for (vi, props) in variants.iter().enumerate() {
                let theme = if spatial { " theme=\"spatial\"" } else { "" };
                let body: String = props.iter().map(|(k, v)| format!("<Property name=\"{k}\">{v}</Property>")).collect();
                let xml = format!(
                    r##"<?xml version="1.0" encoding="UTF-8"?><Form name="main-form" title="Demo" width="640" height="480" index-base="1"{theme}><Control id="C-1" type="{}" x="10" y="10" w="300" h="200">{body}</Control></Form>"##,
                    t.as_str()
                );
                let tree = crate::tree::testing::tree_of(&xml[xml.find("<Control").unwrap_or(0)..xml.rfind("</Form>").unwrap_or(xml.len())]);
                let Some(i) = tree.by_name("C-1") else { continue };
                let mut tree = tree;
                if spatial {
                    tree.set(0, "theme", cobolt_forms::theme::SPATIAL);
                }
                let got: BTreeSet<String> = crate::view::style::control_decls(&tree.ctls, i, true).into_iter().map(|(k, _)| k).filter(|k| watched.contains(k.as_str())).collect();
                let form = cobolt_forms::xml::load_form_from_str(&xml).expect("form");
                let ours: BTreeSet<String> = props_layer(&form.controls[0], &form).into_iter().map(|(k, _)| k.to_owned()).collect();
                checked += 1;
                if got != ours {
                    diffs.push(format!("{} v{vi}{}: player-only {:?}, table-only {:?}", t.as_str(), if spatial { " spatial" } else { "" }, got.difference(&ours).collect::<Vec<_>>(), ours.difference(&got).collect::<Vec<_>>()));
                }
            }
        }
    }
    println!("WASM-036 props_layer: {checked} (type, values, theme) cases, {} differ", diffs.len());
    let mut grouped: std::collections::BTreeMap<String, Vec<String>> = Default::default();
    for d in &diffs {
        let (head, rest) = d.split_once(": ").unwrap_or((d, ""));
        let (ty, v) = head.split_once(" ").unwrap_or((head, ""));
        grouped.entry(rest.to_owned()).or_default().push(format!("{ty}:{v}"));
    }
    let summary: Vec<String> = grouped.iter().map(|(k, v)| format!("{k}  <- {}", v.join(" "))).collect();
    assert!(diffs.is_empty(), "{} of {checked}:\n{}", diffs.len(), summary.join("\n").replace(" ", " ").lines().map(|l| if l.len() > 1500 { l[..1500].to_owned() } else { l.to_owned() }).collect::<Vec<_>>().join("\n"));
}
