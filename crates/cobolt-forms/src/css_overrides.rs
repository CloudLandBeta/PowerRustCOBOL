// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Which of a control's properties its stylesheets override (WASM-007 R12), for the Properties
//! panel's marks and the designer's badge. No egui: Check and the agent tools can ask too.
//!
//! The developer's rules (`pr-app`, or no layer) win over the Properties panel's values (`pr-props`)
//! whatever their specificity, so a rule that matches the control and declares a property's CSS
//! overrides that property. Rules in `@layer pr-theme` / `pr-base` lose to the properties and are not
//! reported. Matching reuses [`crate::css::Stylesheet`] over the element path the player builds
//! (`dom/instance.rs`: `section.pr-form-root` → `div.pr-form` → `div.pr-surface`, or the layer's
//! `div.pr-form-layer`, → the containers → the control, whose id is its name). What the IDE cannot
//! know for sure is marked "may": a rule under `@media`, `@container` or `@supports`, one with a
//! pseudo-class (`:hover`), and one whose subject names an element type (the tag a control's element
//! has is the player's).

use crate::css::{parse_declarations, ElementRef, Stylesheet};
use crate::model::{Control, Form};

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

/// One style rule of a sheet: its selector list, its declarations, its line, and whether it applies
/// only under a condition.
struct RuleAt<'a> {
    selectors: &'a str,
    body: &'a str,
    line: usize,
    conditional: bool,
}

/// Comments and strings replaced by spaces (newlines kept), so braces inside them are not seen.
fn masked(css: &str) -> String {
    let b = css.as_bytes();
    let mut out = b.to_vec();
    let mut i = 0;
    while i < b.len() {
        let end = if b[i] == b'/' && b.get(i + 1) == Some(&b'*') {
            css[i + 2..].find("*/").map_or(b.len(), |e| i + 2 + e + 2)
        } else if b[i] == b'"' || b[i] == b'\'' {
            let mut j = i + 1;
            while j < b.len() && b[j] != b[i] && b[j] != b'\n' {
                j += if b[j] == b'\\' { 2 } else { 1 };
            }
            (j + 1).min(b.len())
        } else {
            i += 1;
            continue;
        };
        let keep_quotes = b[i] != b'/';
        for (k, byte) in out.iter_mut().enumerate().take(end).skip(i) {
            if b[k] != b'\n' && !(keep_quotes && (k == i || k + 1 == end)) && b[k].is_ascii() {
                *byte = b' ';
            }
        }
        i = end;
    }
    // Only ASCII bytes were replaced, by ASCII: still UTF-8.
    String::from_utf8(out).unwrap_or_else(|_| css.to_owned())
}

/// The style rules of a sheet, with the block they sit in. `@layer pr-theme` / `pr-base` blocks are
/// left out (the properties beat them); `@font-face`, `@keyframes`, `@page` and the like hold no rule
/// for an element.
fn rules(css: &str) -> Vec<RuleAt<'_>> {
    let m = masked(css);
    let mb = m.as_bytes();
    let line_of = |at: usize| m[..at].bytes().filter(|&c| c == b'\n').count() + 1;
    // The blocks open around the cursor: (is a group of rules, is conditional, is skipped).
    let mut stack: Vec<(bool, bool, bool)> = Vec::new();
    let mut out = Vec::new();
    let mut start = 0;
    let mut i = 0;
    while i < mb.len() {
        match mb[i] {
            b'{' => {
                let prelude = m[start..i].trim();
                let in_skipped = stack.iter().any(|f| f.2);
                let conditional = stack.iter().any(|f| f.1);
                if let Some(at) = prelude.strip_prefix('@') {
                    let name: String = at.chars().take_while(|c| c.is_ascii_alphanumeric() || *c == '-').collect::<String>().to_ascii_lowercase();
                    let rest = at[name.len()..].trim();
                    let group = matches!(name.as_str(), "media" | "container" | "supports" | "layer" | "scope");
                    let cond = matches!(name.as_str(), "media" | "container" | "supports");
                    let skipped = !group || (name == "layer" && matches!(rest, "pr-theme" | "pr-base"));
                    stack.push((group, cond, skipped));
                } else if stack.last().is_none_or(|f| f.0) {
                    // A style rule: its body runs to the matching brace (nesting inside it is not read).
                    let open = i;
                    let mut depth = 0usize;
                    let mut j = i;
                    while j < mb.len() {
                        match mb[j] {
                            b'{' => depth += 1,
                            b'}' => {
                                depth -= 1;
                                if depth == 0 {
                                    break;
                                }
                            }
                            _ => {}
                        }
                        j += 1;
                    }
                    if !in_skipped {
                        let sel_at = start + (m[start..i].len() - m[start..i].trim_start().len());
                        out.push(RuleAt { selectors: &css[sel_at..i], body: &css[open + 1..j.min(css.len())], line: line_of(sel_at), conditional });
                    }
                    i = j + 1;
                    start = i;
                    continue;
                } else {
                    stack.push((false, false, true));
                }
                start = i + 1;
            }
            b'}' => {
                stack.pop();
                start = i + 1;
            }
            b';' if stack.last().is_none_or(|f| f.0) => start = i + 1,
            _ => {}
        }
        i += 1;
    }
    out
}

/// A selector the subset can test, and whether it was simplified to get there (a pseudo-class taken
/// out, the subject's element type taken out). `None`: it styles a pseudo-element, another box.
fn testable(selector: &str) -> Option<(String, bool)> {
    if selector.contains("::") || selector.contains(":before") || selector.contains(":after") {
        return None;
    }
    let mut out = String::new();
    let mut simplified = false;
    let mut chars = selector.trim().chars().peekable();
    while let Some(c) = chars.next() {
        if c == ':' {
            simplified = true;
            while chars.peek().is_some_and(|c| c.is_ascii_alphanumeric() || *c == '-') {
                chars.next();
            }
            if chars.peek() == Some(&'(') {
                let mut depth = 0;
                for c in chars.by_ref() {
                    match c {
                        '(' => depth += 1,
                        ')' => {
                            depth -= 1;
                            if depth == 0 {
                                break;
                            }
                        }
                        _ => {}
                    }
                }
            }
            continue;
        }
        out.push(c);
    }
    // The subject (the last compound): a leading element type is the player's choice of tag.
    let cut = out.rfind([' ', '>', '+', '~']).map_or(0, |i| i + 1);
    let subject = &out[cut..];
    let tag_len = subject.chars().take_while(|c| c.is_ascii_alphanumeric() || *c == '-' || *c == '*').count();
    if tag_len > 0 {
        simplified |= &subject[..tag_len] != "*";
        out.replace_range(cut..cut + tag_len, "");
    }
    if out[cut..].trim().is_empty() {
        out.push('*');
    }
    Some((out, simplified))
}

/// The control and its containers, outermost first.
fn ancestry<'a>(form: &'a Form, id: &str) -> Vec<&'a Control> {
    fn walk<'a>(cs: &'a [Control], id: &str, path: &mut Vec<&'a Control>) -> bool {
        for c in cs {
            path.push(c);
            if c.id.eq_ignore_ascii_case(id) || walk(&c.children, id, path) {
                return true;
            }
            path.pop();
        }
        false
    }
    let mut path = Vec::new();
    if !walk(&form.controls, id, &mut path) {
        return Vec::new();
    }
    // A flat model names the container in `parent` instead.
    while let Some(p) = path.first().and_then(|c| c.parent.clone()) {
        match form.find_control(&p) {
            Some(c) if !path.iter().any(|x| x.id == c.id) => path.insert(0, c),
            _ => break,
        }
    }
    path
}

/// The classes of a control's element, as `cobolt-web-player/src/css.rs::class_names` gives them.
fn classes(c: &Control) -> Vec<String> {
    let mut v = vec!["pr-control".to_owned(), crate::web_themes::type_class(&c.control_type)];
    if let Some(cls) = c.get_prop("CssClass") {
        v.extend(cls.as_str().split_whitespace().filter(|n| n.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')).map(str::to_owned));
    }
    v
}

/// Does `selector` match the control at the end of `path` (the player's element path)?
fn matches(selector: &str, path: &[ElementRef<'_>]) -> bool {
    !Stylesheet::parse(&format!("{selector} {{ x: y }}")).cascade(path, &[]).is_empty()
}

/// What the result is about.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Overrides {
    /// Per property, the rule that wins it (the last one in cascade order; a certain one before one
    /// that "may").
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
    let chain = ancestry(form, id);
    let Some(ctl) = chain.last().copied() else {
        return Overrides::default();
    };
    let root_classes = vec!["pr-form-root".to_owned()];
    let base_classes = vec!["pr-form".to_owned()];
    let surface_classes = vec!["pr-surface".to_owned()];
    let layer_classes = vec!["pr-form-layer".to_owned()];
    let all_classes: Vec<Vec<String>> = chain.iter().map(|c| classes(c)).collect();
    let mut path: Vec<ElementRef<'_>> = vec![ElementRef { name: "section", id: None, classes: &root_classes, index: 1, count: 1, siblings: &[] }];
    if ctl.layer.as_deref().is_some_and(|l| !l.is_empty()) {
        path.push(ElementRef { name: "div", id: None, classes: &layer_classes, index: 1, count: 1, siblings: &[] });
    } else {
        path.push(ElementRef { name: "div", id: None, classes: &base_classes, index: 1, count: 1, siblings: &[] });
        path.push(ElementRef { name: "div", id: None, classes: &surface_classes, index: 1, count: 1, siblings: &[] });
    }
    for (c, cls) in chain.iter().zip(&all_classes) {
        path.push(ElementRef { name: "div", id: Some(c.id.as_str()), classes: cls, index: 1, count: 1, siblings: &[] });
    }

    let mut out = Overrides::default();
    let mut put = |o: Override| match out.properties.iter_mut().find(|x| x.property == o.property) {
        // A later rule wins, unless it only "may" and the one before is sure.
        Some(x) if x.certain && !o.certain => {}
        Some(x) => *x = o,
        None => out.properties.push(o),
    };
    let mut styled = false;
    for (label, css) in sheets {
        for r in rules(css) {
            let hit = crate::css::split_top(r.selectors, ',').into_iter().filter_map(testable).filter(|(s, _)| matches(s, &path)).map(|(_, simplified)| simplified).min();
            let Some(simplified) = hit else { continue };
            let decls = parse_declarations(r.body);
            styled |= !decls.is_empty();
            for d in decls {
                for p in properties_of(&d.name) {
                    put(Override { property: p, file: Some((*label).to_owned()), line: r.line, certain: !(simplified || r.conditional) });
                }
            }
        }
    }
    if let Some(inline) = ctl.get_prop("CssStyle").map(|v| v.as_str().to_owned()).filter(|s| !s.trim().is_empty()) {
        styled = true;
        for d in parse_declarations(&inline) {
            for p in properties_of(&d.name) {
                put(Override { property: p, file: None, line: 0, certain: true });
            }
        }
    }
    out.styled = styled;
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{ControlType, PropValue};

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
