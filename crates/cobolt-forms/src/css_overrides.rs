// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.


//! Which of a control's properties its stylesheets override (WASM-007 R12), for the Properties
//! panel's marks and the designer's badge. No egui: Check and the agent tools can ask too.
//!
//! A view over [`crate::css_effective`] (WASM-036 R20): the developer's rules (`pr-app`, or no layer)
//! win over the Properties panel's values (`pr-props`) whatever their specificity, so a declaration
//! that matches the control and wins a CSS property overrides the model properties that CSS names.
//! Rules in `@layer pr-theme` / `pr-base` lose to the properties and are not reported. What the IDE
//! cannot know for sure is marked "may" (see `css_effective`).

use crate::css_effective::{cascade, Layer, Matched, Origin, SheetSet, SheetSource};
use crate::model::Form;

/// One overridden property.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Override {
    /// The model property (`BackgroundColor`).
    pub property: &'static str,
    /// Where: the sheet's label (its project-relative file name), or `None` for the control's own
    /// `CssStyle`.
    pub file: Option<String>,
    /// The rule's line in that sheet (1-based); 0 for `CssStyle`.
    pub line: usize,
    /// `false`: the rule applies only in some states or screens ("may be overridden").
    pub certain: bool,
}

/// Each CSS property and the model properties it overrides. The first one named is the property
/// whose CSS this is in the player's table (`cobolt-web-player/src/css.rs::TABLE`, which a player test
/// holds this list to).
pub const CSS_TO_PROPERTIES: &[(&str, &[&str])] = &[
    ("background-color", &["BackgroundColor"]),
    (
        "background",
        &["BackgroundColor", "BackgroundGradientEnabled", "BackgroundGradientStartColor", "BackgroundGradientEndColor", "BackgroundGradientDirection"],
    ),
    ("background-image", &["BackgroundGradientEnabled", "BackgroundGradientStartColor", "BackgroundGradientEndColor", "BackgroundGradientDirection"]),
    ("color", &["ForegroundColor"]),
    ("font-family", &["FontName"]),
    ("font-size", &["FontSize"]),
    ("font-weight", &["Bold"]),
    ("font-style", &["Italic"]),
    ("font", &["FontName", "FontSize", "Bold", "Italic"]),
    ("text-decoration", &["Underline", "Strikethrough"]),
    ("text-decoration-line", &["Underline", "Strikethrough"]),
    ("cursor", &["Cursor"]),
    ("padding", &["Padding", "PaddingLeft", "PaddingTop", "PaddingRight", "PaddingBottom"]),
    ("padding-left", &["PaddingLeft"]),
    ("padding-top", &["PaddingTop"]),
    ("padding-right", &["PaddingRight"]),
    ("padding-bottom", &["PaddingBottom"]),
    ("border-radius", &["CornerRadius", "CornerRadiusTopLeft", "CornerRadiusTopRight", "CornerRadiusBottomLeft", "CornerRadiusBottomRight"]),
    ("border-top-left-radius", &["CornerRadiusTopLeft"]),
    ("border-top-right-radius", &["CornerRadiusTopRight"]),
    ("border-bottom-left-radius", &["CornerRadiusBottomLeft"]),
    ("border-bottom-right-radius", &["CornerRadiusBottomRight"]),
    ("border", &["BorderStyle", "BorderColor", "BorderWidth"]),
    ("border-style", &["BorderStyle"]),
    ("border-color", &["BorderColor"]),
    ("border-width", &["BorderWidth"]),
    (
        "box-shadow",
        &["ShadowEnabled", "ShadowColor", "ShadowLightColor", "ShadowOpacity", "ShadowDirection", "ShadowDistance", "ShadowBlur", "ShadowBlurStrength", "BorderBlur"],
    ),
    ("text-align", &["TextAlignment"]),
    ("justify-content", &["TextAlignment"]),
    ("align-items", &["VerticalAlignment"]),
    ("white-space", &["WordWrap"]),
    ("transform", &["RotateX", "RotateY"]),
    ("z-index", &["ZOrder"]),
    ("display", &["Visible"]),
    ("backdrop-filter", &["OsBlur"]),
];

fn properties_of(css_name: &str) -> &'static [&'static str] {
    CSS_TO_PROPERTIES.iter().find(|(n, _)| n.eq_ignore_ascii_case(css_name)).map_or(&[], |(_, p)| *p)
}

/// What the result is about.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Overrides {
    /// Per property, the rule that wins it (a certain one before one that "may", then the higher
    /// cascade key).
    pub properties: Vec<Override>,
    /// A rule of the developer's matches the control, or it has a `CssStyle`: the designer draws its
    /// badge, because it does not paint the CSS.
    pub styled: bool,
}

impl Overrides {
    pub fn of(&self, property: &str) -> Option<&Override> {
        self.properties.iter().find(|o| o.property.eq_ignore_ascii_case(property))
    }
}

/// The overrides of control `id`: `sheets` in cascade order (the project sheet, then the form's), as
/// `(label, text)`; then its `CssStyle`, which comes last (plan §1.1 level 6).
pub fn overrides(form: &Form, id: &str, sheets: &[(&str, &str)]) -> Overrides {
    let sources: Vec<SheetSource<'_>> = sheets.iter().map(|(label, text)| SheetSource { label, text, origin: Origin::Form }).collect();
    let set = SheetSet::parse(&sources);
    let c = cascade(form, Some(id), &set, None);
    let mut candidates: Vec<&Matched> = c.winners.iter().chain(&c.may).filter(|m| m.layer >= Layer::Project).collect();
    candidates.sort_by_key(|m| (m.certain, m.key()));
    let mut out = Overrides { styled: !candidates.is_empty(), ..Overrides::default() };
    // Ascending: a later candidate replaces an earlier one, so the best ends up last.
    for m in candidates {
        for p in properties_of(&m.css) {
            let o = Override { property: p, file: m.sheet.map(|s| set.sources[s].label.clone()), line: m.line, certain: m.certain };
            match out.properties.iter_mut().find(|x| x.property == *p) {
                Some(x) => *x = o,
                None => out.properties.push(o),
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Control, ControlType, PropValue};

    fn form() -> Form {
        let mut f = Form::new("F", "F", 800, 600);
        let mut b = Control::new("BTN-1", ControlType::Button, 0, 0);
        b.properties.insert("CssClass".into(), PropValue::String("primary".into()));
        f.controls.push(b);
        f.controls.push(Control::new("BTN-2", ControlType::Button, 0, 40));
        f
    }

    #[test]
    fn a_matching_rule_marks_its_property_with_file_and_line_and_a_condition_makes_it_may() {
        let f = form();
        let sheet = "/* mine */\n#BTN-1 {\n  background-color: red;\n}\n@media (width < 600px) {\n  .primary { color: blue }\n}\n.pr-button:hover { border-color: green }\n#BTN-2 { cursor: wait }\n";
        let o = overrides(&f, "BTN-1", &[("webforms/F.css", sheet)]);
        assert_eq!(o.of("BackgroundColor"), Some(&Override { property: "BackgroundColor", file: Some("webforms/F.css".into()), line: 2, certain: true }));
        let fg = o.of("ForegroundColor").expect("the @media rule");
        assert!(!fg.certain && fg.line == 6, "{fg:?}");
        let bc = o.of("BorderColor").expect("the :hover rule");
        assert!(!bc.certain && bc.line == 8, "{bc:?}");
        assert!(o.of("Cursor").is_none(), "another control's rule");
        assert!(o.styled);
    }

    #[test]
    fn the_theme_block_and_a_pseudo_element_do_not_count_and_css_style_comes_last() {
        let f = form();
        let generated = crate::web_themes::form_stylesheet("elegance", &[ControlType::Button]);
        assert!(generated.contains(".pr-button"), "the block has a type rule");
        let o = overrides(&f, "BTN-2", &[("webforms/F.css", &generated)]);
        assert_eq!(o, Overrides::default(), "the properties beat the theme's layer: no mark, no badge");

        let o = overrides(&f, "BTN-2", &[("a.css", "#BTN-2::after { color: red }\n")]);
        assert!(o.of("ForegroundColor").is_none());

        let mut f = form();
        f.controls[1].properties.insert("CssStyle".into(), PropValue::String("color: red; padding: 4px".into()));
        let o = overrides(&f, "BTN-2", &[("webforms/application.css", "#BTN-2 { color: blue }")]);
        assert_eq!(o.of("ForegroundColor").map(|o| (o.file.clone(), o.line)), Some((None, 0)), "the inline style wins");
        assert!(o.of("PaddingLeft").is_some() && o.styled);
    }

    #[test]
    fn the_element_path_is_the_players() {
        let mut f = Form::new("F", "F", 800, 600);
        let mut panel = Control::new("PNL-1", ControlType::Panel, 0, 0);
        panel.children.push(Control::new("TXT-1", ControlType::TextBox, 0, 0));
        f.controls.push(panel);
        let mut on_layer = Control::new("BTN-9", ControlType::Button, 0, 0);
        on_layer.layer = Some("LAYER-1".into());
        f.controls.push(on_layer);
        let css = ".pr-form-root .pr-panel > .pr-textbox { color: red }\n.pr-surface .pr-button { color: blue }\n.pr-form-layer .pr-button { background-color: red }\nbutton { cursor: wait }\n";
        let sheets = [("s.css", css)];
        assert!(overrides(&f, "TXT-1", &sheets).of("ForegroundColor").is_some_and(|o| o.certain));
        let b = overrides(&f, "BTN-9", &sheets);
        assert!(b.of("ForegroundColor").is_none() && b.of("BackgroundColor").is_some(), "{b:?}");
        assert!(b.of("Cursor").is_some_and(|o| !o.certain), "an element type is the player's: may");
    }
}
