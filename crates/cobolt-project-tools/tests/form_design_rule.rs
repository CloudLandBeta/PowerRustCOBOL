// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! **The design steps of `powerrustcobol-layout-and-themes`** (operator,
//! 2026-10-17: guide the form an agent writes — no overlapping controls, no
//! text too small, spacing, typography, soft shadows, consistency, a modern
//! look — without breaking the theme).
//!
//! A skill that states numbers must itself keep them, or the agent is taught one
//! thing and shown another. So the worked example the skill carries is read back
//! with the product's own `.cfrm` loader and held to every rule the steps state;
//! the checker is shown to catch each defect it names, on a form built to have
//! them all; and every property, border style and contrast figure the steps
//! mention is checked against the reference tables and the arithmetic.
//!
//! The shadow figures (a halo 1.4 times darker than the backdrop at opacity 6,
//! a black rim from 15) are pinned to the real paint in
//! `cobolt-forms/tests/design_guidance_bands.rs`.

use std::collections::{BTreeSet, HashMap};

use cobolt_forms::{load_form_from_str, Control, Form};
use cobolt_project_tools::content::{self, BuildInput, KitContent, RcrunLocation, ToolInfo, DESIGN_EXAMPLE_CFRM};

type Rgb = (f64, f64, f64);

/// What Spatial's see-through glass sits on in `render_form`: the dim stand-in
/// for the desktop, measured in the picture.
const BACKDROP: Rgb = (71.0, 68.0, 64.0);
/// The spacing scale of the "Space by role" step.
const SCALE: [i32; 7] = [4, 8, 12, 16, 24, 32, 40];
/// The faces a Button may have: Spatial's own, the one accent, the three states.
const FACES: [&str; 5] = ["#36383EB3", "#0A84FFFF", "#30D158FF", "#FF9F0AFF", "#FF453AFF"];
const TEXT_KINDS: [&str; 10] = [
    "Label", "Button", "TextBox", "ComboBox", "CheckBox", "RadioButton", "ListBox", "NumericUpDown", "DateTimePicker", "GroupBox",
];
const INPUT_KINDS: [&str; 4] = ["TextBox", "ComboBox", "NumericUpDown", "DateTimePicker"];
const SHADOW_KINDS: [&str; 3] = ["Panel", "GroupBox", "Button"];

// ── colour arithmetic ────────────────────────────────────────────────────────

/// `#RRGGBB` or `#RRGGBBAA` as `(rgb, alpha 0..1)`.
fn parse_color(s: &str) -> Option<(Rgb, f64)> {
    let h = s.trim().strip_prefix('#')?;
    let byte = |i: usize| u8::from_str_radix(h.get(i..i + 2)?, 16).ok().map(f64::from);
    match h.len() {
        6 => Some(((byte(0)?, byte(2)?, byte(4)?), 1.0)),
        8 => Some(((byte(0)?, byte(2)?, byte(4)?), byte(6)? / 255.0)),
        _ => None,
    }
}

fn composite(over: (Rgb, f64), under: Rgb) -> Rgb {
    let ((r, g, b), a) = over;
    (r * a + under.0 * (1.0 - a), g * a + under.1 * (1.0 - a), b * a + under.2 * (1.0 - a))
}

fn lum(c: Rgb) -> f64 {
    let f = |v: f64| {
        let v = v / 255.0;
        if v <= 0.03928 {
            v / 12.92
        } else {
            ((v + 0.055) / 1.055).powf(2.4)
        }
    };
    0.2126 * f(c.0) + 0.7152 * f(c.1) + 0.0722 * f(c.2)
}

fn ratio(a: Rgb, b: Rgb) -> f64 {
    let (la, lb) = (lum(a), lum(b));
    (la.max(lb) + 0.05) / (la.min(lb) + 0.05)
}

fn solid(hex: &str) -> Rgb {
    parse_color(hex).expect("a colour").0
}

// ── reading a form ───────────────────────────────────────────────────────────

fn kind(c: &Control) -> String {
    format!("{:?}", c.control_type).chars().take_while(|ch| ch.is_alphanumeric()).collect()
}

fn text(c: &Control, name: &str) -> String {
    c.get_prop(name).map(|v| v.to_xml_string()).unwrap_or_default()
}

fn int(c: &Control, name: &str) -> Option<i64> {
    c.get_prop(name).map(|v| v.as_i64())
}

fn flag(c: &Control, name: &str) -> bool {
    c.get_prop(name).is_some_and(|v| v.as_bool())
}

fn rect(c: &Control) -> (i32, i32, i32, i32) {
    (c.rect.x, c.rect.y, c.rect.w, c.rect.h)
}

/// Every control of the form, once, whether the loader lists it flat or nests it.
fn flat(form: &Form) -> Vec<&Control> {
    fn walk<'a>(c: &'a Control, out: &mut Vec<&'a Control>) {
        if out.iter().all(|seen| seen.id != c.id) {
            out.push(c);
        }
        for k in &c.children {
            walk(k, out);
        }
    }
    let mut out = Vec::new();
    for c in &form.controls {
        walk(c, &mut out);
    }
    out
}

/// The colours a control's own face is painted in (both ends of a gradient).
/// A Label paints none: Spatial draws it on whatever is under it (checked in
/// `render_form`, with an ink set and without `Transparency`).
fn face_colors(c: &Control) -> Vec<(Rgb, f64)> {
    if kind(c) == "Label" {
        return Vec::new();
    }
    if flag(c, "BackgroundGradientEnabled") {
        ["BackgroundGradientStartColor", "BackgroundGradientEndColor"]
            .iter()
            .filter_map(|n| parse_color(&text(c, n)))
            .collect()
    } else {
        parse_color(&text(c, "BackgroundColor")).into_iter().collect()
    }
}

/// Every surface a control's ink may sit on: its own face over what is under
/// it, all the way down to the backdrop.
fn surfaces(c: &Control, by_id: &HashMap<&str, &Control>) -> Vec<Rgb> {
    let under: Vec<Rgb> = match c.parent.as_deref().and_then(|p| by_id.get(p)) {
        Some(p) => surfaces(p, by_id),
        None => vec![BACKDROP],
    };
    let faces = face_colors(c);
    if faces.is_empty() {
        return under;
    }
    let mut out = Vec::new();
    for f in faces {
        for u in &under {
            out.push(composite(f, *u));
        }
    }
    out
}

// ── the design steps, as a checker ───────────────────────────────────────────

/// One message per breach of the design steps; empty means the form obeys them.
fn design_breaches(form: &Form) -> Vec<String> {
    let all = flat(form);
    let by_id: HashMap<&str, &Control> = all.iter().map(|c| (c.id.as_str(), *c)).collect();
    let visible: Vec<&Control> = all.iter().copied().filter(|c| c.visible && !c.control_type.is_non_visual()).collect();
    let (fw, fh) = (form.width as i32, form.height as i32);
    let mut out = Vec::new();

    if form.grid_size != 8 || !form.snap_to_grid {
        out.push("the form's grid-size is not 8 with snap-to-grid on".into());
    }

    // Geometry: form coordinates, on the 8 grid, inside the parent, inside the form.
    for c in &visible {
        let (x, y, w, h) = rect(c);
        if [x, y, w, h].iter().any(|v| v % 8 != 0) {
            out.push(format!("{}: x/y/w/h {x},{y},{w},{h} are off the 8 grid", c.id));
        }
        if x < 0 || y < 0 || x + w > fw || y + h > fh {
            out.push(format!("{}: outside the form", c.id));
        }
        if let Some(p) = c.parent.as_deref().and_then(|p| by_id.get(p)) {
            let (px, py, pw, ph) = rect(p);
            let pad = int(p, "Padding").unwrap_or(0) as i32;
            if x < px + pad || y < py + pad || x + w > px + pw - pad || y + h > py + ph - pad {
                out.push(format!("{}: outside its parent {} (or inside its Padding)", c.id, p.id));
            }
        }
    }

    // Overlap, touching and spacing: only for controls on screen together.
    let mut groups: HashMap<(Option<String>, Option<u32>), Vec<&Control>> = HashMap::new();
    for c in &visible {
        groups.entry((c.parent.clone(), c.tab)).or_default().push(c);
    }
    let backdrop = |k: &str| matches!(k, "Shape" | "PictureBox" | "Panel");
    for list in groups.values() {
        for (i, a) in list.iter().enumerate() {
            for b in &list[i + 1..] {
                let (ra, rb) = (rect(a), rect(b));
                let dx = (ra.0 - (rb.0 + rb.2)).max(rb.0 - (ra.0 + ra.2));
                let dy = (ra.1 - (rb.1 + rb.3)).max(rb.1 - (ra.1 + ra.3));
                if dx < 0 && dy < 0 {
                    if !(backdrop(&kind(a)) || backdrop(&kind(b))) {
                        out.push(format!("{} and {} overlap", a.id, b.id));
                    }
                    continue;
                }
                // Neighbours: they share a span on one axis; the gap is on the other.
                let gap = if dx < 0 { dy } else if dy < 0 { dx } else { continue };
                if gap == 0 {
                    out.push(format!("{} and {} touch", a.id, b.id));
                } else if gap < 48 && !SCALE.contains(&gap) {
                    out.push(format!("{} and {}: a gap of {gap} is not on the scale", a.id, b.id));
                }
            }
        }
    }
    // The margin between the window and its content.
    let top: Vec<&&Control> = visible.iter().filter(|c| c.parent.is_none()).collect();
    if let (Some(l), Some(t), Some(r), Some(b)) = (
        top.iter().map(|c| c.rect.x).min(),
        top.iter().map(|c| c.rect.y).min(),
        top.iter().map(|c| c.rect.x + c.rect.w).max(),
        top.iter().map(|c| c.rect.y + c.rect.h).max(),
    ) {
        if l.min(t).min(fw - r).min(fh - b) < 16 {
            out.push("the margin between the window and its content is under 16".into());
        }
    }

    // Type: FontName unset, FontSize at least 12, at most four sizes.
    let mut sizes = BTreeSet::new();
    for c in visible.iter().filter(|c| TEXT_KINDS.contains(&kind(c).as_str())) {
        match int(c, "FontSize") {
            Some(s) if s >= 12 => {
                sizes.insert(s);
            }
            Some(s) => out.push(format!("{}: text of {s} points is under 12", c.id)),
            None => out.push(format!("{}: no explicit FontSize (under 12 is not allowed, 14 is body)", c.id)),
        }
        if c.get_prop("FontName").is_some() {
            out.push(format!("{}: FontName is set; leave it unset", c.id));
        }
    }
    if sizes.len() > 4 {
        out.push(format!("{} sizes on one form: {sizes:?}", sizes.len()));
    }

    // Defaults are the dated look: an omitted face or ink shows as these exact values.
    for c in &visible {
        let k = kind(c);
        let faced = ["Button", "TextBox", "ComboBox", "NumericUpDown", "DateTimePicker", "ListBox", "Panel", "GroupBox"];
        if faced.contains(&k.as_str())
            && !flag(c, "BackgroundGradientEnabled")
            && text(c, "BackgroundColor").eq_ignore_ascii_case("#F0F0F0")
            && int(c, "Transparency").unwrap_or(0) < 100
        {
            out.push(format!("{}: the default grey face #F0F0F0 (set BackgroundColor or the gradient)", c.id));
        }
        if TEXT_KINDS.contains(&k.as_str()) && text(c, "ForegroundColor") == "#000000" {
            out.push(format!("{}: black ink #000000 is the default (set ForegroundColor, eight digits)", c.id));
        }
    }

    // Colour: an ink that passes the contrast rule on every surface it sits on.
    for c in visible.iter().filter(|c| TEXT_KINDS.contains(&kind(c).as_str())) {
        let Some(ink) = parse_color(&text(c, "ForegroundColor")) else {
            out.push(format!("{}: no ForegroundColor", c.id));
            continue;
        };
        let need = if int(c, "FontSize").unwrap_or(0) >= 18 { 3.0 } else { 4.5 };
        for s in surfaces(c, &by_id) {
            let seen = composite(ink, s);
            let r = ratio(seen, s);
            if r < need {
                out.push(format!("{}: contrast {r:.2}:1 is under {need}:1 against {s:?}", c.id));
            }
        }
    }
    for c in visible.iter().filter(|c| kind(c) == "Button") {
        let face = text(c, "BackgroundColor").to_ascii_uppercase();
        if !FACES.contains(&face.as_str()) {
            out.push(format!("{}: a second accent, the face {face} is not Spatial's, the accent or a state", c.id));
        }
    }

    // Depth: one soft shadow from one light, on a raised face only.
    for c in visible.iter().filter(|c| flag(c, "ShadowEnabled")) {
        let k = kind(c);
        if !SHADOW_KINDS.contains(&k.as_str()) {
            out.push(format!("{}: a {k} has no shadow", c.id));
        }
        if text(c, "ShadowDirection") != "SouthEast" {
            out.push(format!("{}: ShadowDirection {} is not SouthEast", c.id, text(c, "ShadowDirection")));
        }
        if text(c, "ShadowColor").to_ascii_uppercase() != "#000000FF" {
            out.push(format!("{}: ShadowColor is not black", c.id));
        }
        let opacity = int(c, "ShadowOpacity").unwrap_or(-1);
        if !(4..=8).contains(&opacity) {
            out.push(format!("{}: ShadowOpacity {opacity} is outside 4 to 8", c.id));
        }
        let distance = int(c, "ShadowDistance").unwrap_or(-1);
        if !(4..=10).contains(&distance) {
            out.push(format!("{}: ShadowDistance {distance} is outside 4 to 10", c.id));
        }
        let strength = int(c, "ShadowBlurStrength").unwrap_or(-99);
        if !flag(c, "ShadowBlur") || !(8..=12).contains(&strength) {
            out.push(format!("{}: ShadowBlur must be on with a ShadowBlurStrength of 8 to 12, was {strength}", c.id));
        }
    }

    // A modern look: no bevels, no outline around a card, rounded faces.
    for c in &visible {
        let k = kind(c);
        let style = text(c, "BorderStyle");
        let outline_ok = INPUT_KINDS.contains(&k.as_str())
            && style == "Single"
            && parse_color(&text(c, "BorderColor")).is_some_and(|(_, a)| a <= 0.25);
        if !style.is_empty() && style != "None" && !outline_ok {
            out.push(format!("{}: BorderStyle {style} is a dated border", c.id));
        }
        let radius = int(c, "CornerRadius").unwrap_or(0);
        let bad_radius = match k.as_str() {
            "Button" | "TextBox" | "ComboBox" | "NumericUpDown" | "DateTimePicker" => !(10..=12).contains(&radius),
            "Panel" | "GroupBox" if flag(c, "ShadowEnabled") => !(22..=28).contains(&radius),
            _ => false,
        };
        if bad_radius {
            out.push(format!("{}: CornerRadius {radius} is outside the range for a {k}", c.id));
        }
    }

    // Consistency: one height and one radius per class.
    for k in ["Button", "TextBox"] {
        let of: Vec<&&Control> = visible.iter().filter(|c| kind(c) == k).collect();
        let heights: BTreeSet<i32> = of.iter().map(|c| c.rect.h).collect();
        let radii: BTreeSet<i64> = of.iter().map(|c| int(c, "CornerRadius").unwrap_or(0)).collect();
        if heights.len() > 1 || radii.len() > 1 {
            out.push(format!("the {k} controls differ: heights {heights:?}, radii {radii:?}"));
        }
        if of.iter().any(|c| c.rect.h < 32) {
            out.push(format!("a {k} is under 32 high"));
        }
    }
    out
}

// ── the tests ────────────────────────────────────────────────────────────────

fn kit() -> KitContent {
    let tool = |n: &str| ToolInfo { name: n.into(), description: String::new() };
    content::build(BuildInput {
        ide_version: "1.test".into(),
        project_name: "Demo".into(),
        ide_url: "http://127.0.0.1:5720/mcp".into(),
        rcrun: RcrunLocation::OnPath,
        tools: ["list_files", "check", "regenerate", "add_to_project", "build", "validate", "kb_lookup", "render_form"]
            .into_iter()
            .map(tool)
            .collect(),
        reference: Vec::new(),
    })
}

fn design_skill_text(kit: &KitContent) -> String {
    let skill = kit.skills.iter().find(|s| s.name == "powerrustcobol-layout-and-themes").expect("the skill exists");
    skill.steps.join("\n")
}

#[test]
fn the_worked_example_obeys_every_design_step_it_teaches() {
    let form = load_form_from_str(DESIGN_EXAMPLE_CFRM).expect("the example is a form the product loads");
    assert!(flat(&form).len() >= 10, "the example has its controls");
    let breaches = design_breaches(&form);
    assert!(breaches.is_empty(), "the skill's own example breaks its own rules:\n{}", breaches.join("\n"));

    // The skill shows exactly this form, in a fenced block.
    let kit = kit();
    let skill = kit.skills.iter().find(|s| s.name == "powerrustcobol-layout-and-themes").unwrap();
    let shown = skill.example.as_deref().expect("an example");
    assert!(shown.starts_with("```xml\n") && shown.ends_with("```") && shown.contains(DESIGN_EXAMPLE_CFRM.trim()));
}

/// A form built to break every rule the checker names.
const BAD_FORM: &str = r##"<?xml version="1.0" encoding="UTF-8"?>
<Form name="BAD" title="Bad" width="640" height="424" grid-size="8" snap-to-grid="true">
  <Control id="A" type="Button" x="24" y="24" w="112" h="36">
    <Property name="FontSize">9</Property>
    <Property name="FontName">Verdana</Property>
    <Property name="ForegroundColor">#777777FF</Property>
    <Property name="BackgroundColor">#F0F0F0FF</Property>
    <Property name="BorderStyle">Fixed3D</Property>
    <Property name="CornerRadius">0</Property>
    <Property name="ShadowEnabled">true</Property>
    <Property name="ShadowOpacity">40</Property>
    <Property name="ShadowColor">#FFFFFFFF</Property>
    <Property name="ShadowDirection">NorthWest</Property>
    <Property name="ShadowDistance">16</Property>
    <Property name="ShadowBlur">false</Property>
  </Control>
  <Control id="B" type="Button" x="104" y="40" w="112" h="40">
    <Property name="FontSize">14</Property>
    <Property name="ForegroundColor">#FFFFFFFF</Property>
    <Property name="BackgroundColor">#FF00FFFF</Property>
    <Property name="CornerRadius">12</Property>
  </Control>
  <Control id="C" type="Label" x="24" y="96" w="96" h="24">
    <Property name="FontSize">14</Property>
    <Property name="ForegroundColor">#FFFFFFFF</Property>
    <Property name="Transparency">100</Property>
  </Control>
  <Control id="D" type="TextBox" x="24" y="120" w="200" h="40">
    <Property name="FontSize">14</Property>
    <Property name="ForegroundColor">#FFFFFFFF</Property>
    <Property name="BackgroundColor">#12131CFF</Property>
    <Property name="BorderStyle">Single</Property>
    <Property name="BorderColor">#FFFFFF33</Property>
    <Property name="CornerRadius">12</Property>
  </Control>
  <Control id="E" type="Label" x="24" y="170" w="96" h="24">
    <Property name="FontSize">14</Property>
    <Property name="ForegroundColor">#FFFFFFFF</Property>
    <Property name="Transparency">100</Property>
  </Control>
  <Control id="F" type="Button" x="600" y="24" w="80" h="40">
    <Property name="FontSize">14</Property>
    <Property name="ForegroundColor">#FFFFFFFF</Property>
    <Property name="BackgroundColor">#36383EB3</Property>
    <Property name="CornerRadius">12</Property>
  </Control>
  <Control id="CARD" type="Panel" x="248" y="200" w="200" h="104">
    <Property name="Padding">16</Property>
    <Property name="BorderStyle">Single</Property>
    <Property name="CornerRadius">0</Property>
    <Property name="BackgroundColor">#36383EB3</Property>
  </Control>
  <Control id="G" type="Label" x="256" y="208" w="96" h="24" parent="CARD">
    <Property name="FontSize">14</Property>
    <Property name="ForegroundColor">#FFFFFFFF</Property>
    <Property name="Transparency">100</Property>
  </Control>
  <Control id="H" type="Label" x="256" y="240" w="96" h="24" parent="CARD">
    <Property name="FontSize">14</Property>
  </Control>
  <Control id="T" type="TextBox" x="24" y="328" w="200" h="40">
  </Control>
</Form>"##;

#[test]
fn the_checker_finds_what_it_is_built_to_find() {
    let form = load_form_from_str(BAD_FORM).expect("the bad form loads");
    let breaches = design_breaches(&form).join("\n");
    for needle in [
        "A and B overlap",                      // overlap
        "C and D touch",                        // a gap of 0
        "D and E: a gap of 10 is not on the scale", // an off-scale gap
        "A: x/y/w/h 24,24,112,36 are off the 8 grid",
        "F: outside the form",
        "G: outside its parent CARD (or inside its Padding)",
        "A: text of 9 points is under 12",
        "A: FontName is set",
        "A: contrast",                          // grey ink on a light face
        "T: the default grey face #F0F0F0",     // a bare TextBox: the defaults are the dated look
        "T: black ink #000000 is the default",
        "T: BorderStyle Fixed3D is a dated border",
        "T: no explicit FontSize",
        "T: CornerRadius 0 is outside the range for a TextBox",
        "B: a second accent",
        "A: ShadowDirection NorthWest is not SouthEast",
        "A: ShadowColor is not black",
        "A: ShadowOpacity 40 is outside 4 to 8",
        "A: ShadowDistance 16 is outside 4 to 10",
        "A: ShadowBlur must be on",
    ] {
        assert!(breaches.contains(needle), "the checker missed `{needle}`:\n{breaches}");
    }
    // …and it does not cry wolf: a Panel without a shadow, a clean TextBox, a clean Label.
    assert!(!breaches.contains("CARD: a Panel has no shadow"), "a Panel without a shadow is fine:\n{breaches}");
    for clean in ["D: ", "C: ", "G: contrast", "G: text of"] {
        assert!(!breaches.lines().any(|l| l.starts_with(clean)), "the checker flagged `{clean}` wrongly:\n{breaches}");
    }
    assert!(breaches.contains("A: BorderStyle Fixed3D is a dated border"), "{breaches}");
    assert!(breaches.contains("CARD: BorderStyle Single is a dated border"), "an outline around a card: {breaches}");
    assert!(breaches.contains("A: CornerRadius 0 is outside the range for a Button"), "{breaches}");
    assert!(breaches.contains("the Button controls differ"), "buttons of different heights: {breaches}");
}

/// The value a control of `kind` opens with for `prop` when its element sets nothing.
fn bare(kind: &str, prop: &str) -> Option<String> {
    let xml = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<Form name="P" title="P" width="400" height="300"><Control id="C" type="{kind}" x="8" y="8" w="120" h="40"><Property name="Caption">c</Property></Control></Form>"#
    );
    let form = load_form_from_str(&xml).expect("a bare control loads");
    form.controls[0].get_prop(prop).map(|v| v.to_xml_string())
}

#[test]
fn the_defaults_the_steps_warn_about_are_the_defaults_the_product_has() {
    let steps = design_skill_text(&kit());
    assert!(steps.contains("Never leave the look to a default"));
    // An outline round a Button, Panel, GroupBox and ComboBox; a bevel on a TextBox.
    for k in ["Button", "Panel", "GroupBox", "ComboBox"] {
        assert_eq!(bare(k, "BorderStyle").as_deref(), Some("Single"), "{k}");
    }
    assert_eq!(bare("TextBox", "BorderStyle").as_deref(), Some("Fixed3D"));
    // Square corners (3 on a Button); the grey face; black ink where the steps say.
    assert_eq!(bare("Button", "CornerRadius").as_deref(), Some("3"));
    for k in ["TextBox", "Panel", "Button"] {
        assert_eq!(bare(k, "BackgroundColor").as_deref(), Some("#F0F0F0"), "{k}");
    }
    assert_eq!(bare("TextBox", "CornerRadius").as_deref(), Some("0"));
    for k in ["TextBox", "ComboBox", "CheckBox", "ListBox", "DataGrid"] {
        assert_eq!(bare(k, "ForegroundColor").as_deref(), Some("#000000"), "{k}");
    }
    // No FontSize is seeded: the paint falls back to 11 points (pinned in cobolt-forms).
    for k in ["Button", "Label", "TextBox"] {
        assert_eq!(bare(k, "FontSize"), None, "{k}");
    }
}

#[test]
fn every_property_and_border_the_design_steps_name_is_real() {
    let kit = kit();
    let steps = design_skill_text(&kit);

    // Every design property named in backticks exists in the reference tables.
    let prefixes = ["Shadow", "Font", "Border", "Corner", "Padding", "Gap", "TextAlign", "Foreground", "Background", "Transparency"];
    let mut named = BTreeSet::new();
    for token in steps.split('`').skip(1).step_by(2) {
        let t = token.trim();
        if t.chars().all(|c| c.is_ascii_alphanumeric()) && t.chars().next().is_some_and(|c| c.is_ascii_uppercase()) && prefixes.iter().any(|p| t.starts_with(p)) {
            named.insert(t.to_owned());
        }
    }
    assert!(named.len() >= 12, "the steps name their properties: {named:?}");
    for p in &named {
        assert!(cobolt_compiler::property_reference(p).is_some(), "`{p}` is named in the design steps but is not a property");
    }

    // The values the steps name exist in their domains.
    let (border, _) = cobolt_compiler::property_reference_for("Button", "BorderStyle").expect("BorderStyle");
    for v in ["None", "Single", "Fixed3D", "Raised", "Sunken", "Glow"] {
        assert!(border.contains(v), "BorderStyle has no `{v}`: {border}");
    }
    let (direction, _) = cobolt_compiler::property_reference_for("Button", "ShadowDirection").expect("ShadowDirection");
    assert!(direction.contains("SouthEast") && direction.contains("NorthWest"), "{direction}");
    let (opacity, _) = cobolt_compiler::property_reference_for("Button", "ShadowOpacity").expect("ShadowOpacity");
    assert!(opacity.contains("0-100"), "{opacity}");
    let (strength, _) = cobolt_compiler::property_reference_for("Button", "ShadowBlurStrength").expect("ShadowBlurStrength");
    assert!(strength.contains("-20..20"), "{strength}");

    // Every control class the steps give a rule to takes the property the rule sets.
    for (control, props) in [
        ("Button", &["FontSize", "CornerRadius", "BorderStyle", "ShadowOpacity", "ForegroundColor", "BackgroundColor"][..]),
        ("TextBox", &["FontSize", "CornerRadius", "BorderStyle", "BorderColor", "TextAlignment", "BackgroundColor"][..]),
        ("Panel", &["CornerRadius", "Padding", "ShadowEnabled", "BackgroundGradientEnabled", "Transparency"][..]),
        ("Label", &["FontSize", "Transparency", "TextAlignment", "ForegroundColor"][..]),
    ] {
        for p in props {
            assert!(cobolt_compiler::property_reference_for(control, p).is_some(), "{control} has no `{p}`");
        }
    }
}

#[test]
fn the_contrast_figures_the_steps_state_are_true() {
    let kit = kit();
    let steps = design_skill_text(&kit);
    let accent = solid("#0A84FFFF");
    let navy = solid("#050A26FF");
    let on_accent = ratio(navy, accent);
    let white_on_accent = ratio((255.0, 255.0, 255.0), accent);
    // The figures are stated to one decimal, so they must round to what the steps say.
    assert_eq!(format!("{on_accent:.1}"), "5.3", "navy on the accent is {on_accent:.3}");
    assert_eq!(format!("{white_on_accent:.1}"), "3.6", "white on the accent is {white_on_accent:.3}");
    assert!(steps.contains("5.3:1") && steps.contains("3.6:1"));
    for state in ["#30D158FF", "#FF9F0AFF", "#FF453AFF"] {
        let r = ratio(navy, solid(state));
        assert!(r >= 4.5, "the dark ink on {state} is {r:.2}:1, under the 4.5 the contrast rule asks");
    }
    // Secondary text, white at alpha AA, on the lightest part of a Spatial card.
    let card_light = composite(parse_color("#4E4E4EB3").unwrap(), BACKDROP);
    let muted = composite(parse_color("#FFFFFFAA").unwrap(), card_light);
    assert!(ratio(muted, card_light) >= 4.5, "secondary text, {:.2}:1", ratio(muted, card_light));
    // …and an alpha lower than AA would not: the steps' "never below AA" is the edge.
    let fainter = composite(parse_color("#FFFFFF88").unwrap(), card_light);
    assert!(ratio(fainter, card_light) < 4.5, "alpha 88 fails the contrast rule, as the steps imply");
}

#[test]
fn the_other_skills_and_the_reviewer_send_the_agent_to_the_design_steps() {
    let kit = kit();
    let named = "powerrustcobol-layout-and-themes";
    for skill in ["powerrustcobol-create-form", "powerrustcobol-add-control", "powerrustcobol-build-an-application"] {
        let s = kit.skills.iter().find(|s| s.name == skill).unwrap_or_else(|| panic!("no skill {skill}"));
        assert!(s.steps.join(" ").contains(named), "{skill} does not point at {named}");
    }
    assert!(kit.reviewer.checks.iter().any(|c| c.contains(named) && c.contains("12 points")), "the reviewer checks the design");
    // The skill that writes forms uses the tool that shows them.
    let create = kit.skills.iter().find(|s| s.name == "powerrustcobol-create-form").unwrap();
    assert!(create.uses_tools.iter().any(|t| t == "render_form"));
    // The standing rule carries the few numbers an agent must have before it reads any skill.
    let (_, slick) = content::APPLICATION_RULES.iter().find(|(id, _)| *id == "slick").expect("a slick rule");
    assert!(slick.contains("12 points") && slick.contains("overlap"), "{slick}");
}
