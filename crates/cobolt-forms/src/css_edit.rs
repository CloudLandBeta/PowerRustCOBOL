// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Writing a property's new value back to the stylesheet that wins it (WASM-036 R9–R12, R14, R16,
//! R18; plan §1.5). Pure: it takes the texts and returns the new texts; the IDE writes them and
//! records the undo step. No egui and no `render` feature.
//!
//! [`plan`] decides, in order: a value no sheet wins (or the theme, or the properties layer) is
//! stored as a property, as before; a property CSS cannot hold is refused; otherwise the winning
//! declaration is rewritten in place (its value's bytes only, in its own notation, keeping
//! `!important`), or a longhand is inserted after a shorthand it cannot rewrite, or, when the
//! developer chose "only this control", the declaration goes into the form sheet's `#NAME` rule.
//! The new texts are then checked in memory: the property must come out as typed (R12), and the
//! lint must not gain an error (R18). Nothing is returned for a plan that fails either check.

use crate::css::{parse_declarations_spanned, DeclSpan, MediaEnv};
use crate::css_effective::{effective, DeclSource, Layer, Origin, SheetSet, SheetSource};
use crate::css_reverse::{longhand_of, notation_of, read, row_of, write, write_decoration, write_length, write_shadow, Ctx, Fidelity, Notation, Reading, Value};
use crate::model::{ControlType, Form, PropValue};
use crate::stylesheet_lint::{lint, lint_declarations, message, Options, Severity};

/// What the developer chose when the winning rule is shared with other controls (R11).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Choice {
    /// Edit the rule that wins.
    ChangeRule,
    /// Leave it as it is for the others and add the declaration to this control's own `#NAME` rule.
    OnlyThisControl,
}

/// A sheet's text before and after.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SheetEdit {
    /// The source's label, as given to the planner.
    pub label: String,
    pub before: String,
    pub after: String,
}

/// What to do with an edit.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Plan {
    /// No sheet wins the property: store it as the control's property, as always.
    StoreProperty,
    /// Write these sheet texts and, when set, this `CssStyle`.
    Edit { sheets: Vec<SheetEdit>, css_style: Option<String> },
}

/// Why an edit is not made.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Refusal {
    /// The model property has no CSS to write it in (`Visible`, `ZOrder`, a transform).
    NotWritable,
    /// The value would not be the one typed: a rule at `file:line` still wins.
    NoEffect { file: Option<String>, line: usize },
    /// The new text would have a lint error the old text did not.
    Lint { file: Option<String>, line: usize, message: String },
    /// No control of that name on the form.
    NoControl,
    /// The edit has nowhere to go (no form stylesheet for "only this control", a value the row cannot write).
    CannotEdit(&'static str),
}

fn newline(text: &str) -> &'static str {
    if text.contains("\r\n") {
        "\r\n"
    } else {
        "\n"
    }
}

/// The id as a CSS identifier: a leading digit becomes `\3N ` (`1A` → `\31 A`).
pub fn escape_id(id: &str) -> String {
    match id.chars().next() {
        Some(d) if d.is_ascii_digit() => format!("\\3{d} {}", &id[1..]),
        _ => id.to_owned(),
    }
}

/// The whitespace at the start of the line `at` is on, when only whitespace comes before it.
fn indent_before(text: &str, at: usize) -> Option<&str> {
    let line_start = text[..at].rfind('\n').map_or(0, |i| i + 1);
    let lead = &text[line_start..at];
    lead.chars().all(|c| c == ' ' || c == '\t').then_some(lead)
}

/// Where the `;` that ends the declaration is, if there is one right after it.
fn semicolon_after(text: &str, whole_end: usize) -> Option<usize> {
    let rest = &text[whole_end..];
    let skip = rest.len() - rest.trim_start().len();
    rest[skip..].starts_with(';').then_some(whole_end + skip)
}

/// `text` with `decl` removed: the declaration, its `;`, and its line when it had the line to itself.
pub fn remove(text: &str, decl: &DeclSpan) -> String {
    let start = decl.whole.start;
    let end = semicolon_after(text, decl.whole.end).map_or(decl.whole.end, |s| s + 1);
    let line_start = text[..start].rfind('\n').map_or(0, |i| i + 1);
    let alone_before = text[line_start..start].chars().all(|c| c == ' ' || c == '\t');
    let line_end = text[end..].find('\n').map(|i| end + i);
    let alone_after = text[end..line_end.unwrap_or(text.len())].chars().all(|c| c == ' ' || c == '\t' || c == '\r');
    if alone_before && alone_after {
        let cut_end = line_end.map_or(text.len(), |i| i + 1);
        let cut_start = if line_end.is_none() && line_start > 0 { line_start - 1 } else { line_start };
        let mut out = text[..cut_start].to_owned();
        out.push_str(&text[cut_end..]);
        return out;
    }
    let mut stop = end;
    if text[stop..].starts_with(' ') {
        stop += 1;
    }
    // The last declaration of an inline text: the space that led to it goes with it.
    let head = if stop >= text.len() { text[..start].trim_end_matches([' ', '\t']) } else { &text[..start] };
    format!("{head}{}", &text[stop..])
}

/// Insert `css: value` as a new declaration right after `decl` (its `;` added when the last
/// declaration had none), in the same rule, with the same indentation.
fn insert_after(text: &str, decl: &DeclSpan, css: &str, value: &str) -> String {
    let important = if decl.decl.important { " !important" } else { "" };
    let nl = newline(text);
    let new = format!("{css}: {value}{important}");
    match semicolon_after(text, decl.whole.end) {
        Some(semi) => match indent_before(text, decl.whole.start) {
            Some(ind) => format!("{}{nl}{ind}{new};{}", &text[..=semi], &text[semi + 1..]),
            None => format!("{} {new};{}", &text[..=semi], &text[semi + 1..]),
        },
        None => format!("{}; {new}{}", &text[..decl.whole.end], &text[decl.whole.end..]),
    }
}

fn replace(text: &str, range: std::ops::Range<usize>, with: &str) -> String {
    format!("{}{with}{}", &text[..range.start], &text[range.end..])
}

/// Does the value read back from the sheets equal the one typed (within what CSS can say)?
fn same(a: &Value, b: &Value) -> bool {
    match (a, b) {
        (Value::Num(x), Value::Num(y)) => (x - y).abs() < 0.05,
        _ => a == b,
    }
}

/// The text of a value in the notation `table` prefers (a colour as hex, a length in px).
fn table_text(property: &str, value: &Value, ctx: &Ctx) -> Option<String> {
    let nt = match value {
        Value::Color(_) => Notation::Hex,
        _ => Notation::Px,
    };
    write(property, value, nt, ctx)
}

/// The `border` shorthand with one component replaced (width, style or colour), or added.
fn border_component(value: &str, property: &str, new: &Value, ctx: &Ctx) -> Option<String> {
    let mut toks: Vec<String> = crate::css::tokens(value).into_iter().map(str::to_owned).collect();
    let kind = |t: &str| -> &'static str {
        if matches!(t.to_ascii_lowercase().as_str(), "solid" | "dashed" | "dotted" | "double" | "none" | "hidden" | "outset" | "inset" | "groove" | "ridge") {
            "BorderStyle"
        } else if crate::css::color(t).is_some() {
            "BorderColor"
        } else {
            "BorderWidth"
        }
    };
    let text = match (property, new) {
        ("BorderWidth", Value::Num(n)) => {
            let nt = toks.iter().find(|t| kind(t) == "BorderWidth").map_or(Notation::Px, |t| notation_of(t));
            write_length(*n, nt, ctx)
        }
        ("BorderStyle", Value::Text(_)) => write(property, new, Notation::Px, ctx)?,
        ("BorderColor", Value::Color(c)) => {
            let nt = toks.iter().find(|t| kind(t) == "BorderColor").map_or(Notation::Hex, |t| notation_of(t));
            crate::css_reverse::write_color(*c, nt)
        }
        _ => return None,
    };
    match toks.iter().position(|t| kind(t) == property) {
        Some(i) => toks[i] = text,
        None => toks.push(text),
    }
    Some(toks.join(" "))
}

struct Located<'a> {
    text: &'a str,
    span: DeclSpan,
}

/// The new text of the sheet (or of `CssStyle`) with the winning declaration edited for `property`.
fn edit_winner(loc: &Located<'_>, css: &str, property: &str, new: &Value, current: &[(&'static str, Value)], ctx: &Ctx) -> Result<String, Refusal> {
    let value_text = &loc.text[loc.span.value.clone()];
    let row = row_of(css).ok_or(Refusal::CannotEdit("a CSS property the table does not know"))?;
    if row.fidelity == Fidelity::NotReversible {
        return Err(Refusal::NotWritable);
    }
    let nt = notation_of(value_text);
    let longhand = longhand_of(property);
    if row.writable && property_is_of(css, property) {
        let new_value = match css {
            "text-decoration" | "text-decoration-line" => {
                let get = |p: &str| current.iter().find(|(k, _)| *k == p).is_some_and(|(_, v)| *v == Value::Bool(true));
                let (u, s) = match (property, new) {
                    ("Underline", Value::Bool(b)) => (*b, get("Strikethrough")),
                    ("Strikethrough", Value::Bool(b)) => (get("Underline"), *b),
                    _ => return Err(Refusal::CannotEdit("not a decoration")),
                };
                write_decoration(u, s)
            }
            "box-shadow" => {
                let get = |p: &str| current.iter().find(|(k, _)| *k == p).map(|(_, v)| v.clone());
                let mut num = |p: &str, d: f32| match (p == property, &get(p)) {
                    (true, _) => if let Value::Num(n) = new { *n } else { d },
                    (false, Some(Value::Num(n))) => *n,
                    _ => d,
                };
                let enabled = if property == "ShadowEnabled" { matches!(new, Value::Bool(true)) } else { !matches!(get("ShadowEnabled"), Some(Value::Bool(false))) };
                let dir = match (property, new) {
                    ("ShadowDirection", Value::Text(t)) => t.clone(),
                    _ => if let Some(Value::Text(t)) = get("ShadowDirection") { t } else { "South".to_owned() },
                };
                let color = match (property, new) {
                    ("ShadowColor", Value::Color(c)) => [c[0], c[1], c[2]],
                    _ => if let Some(Value::Color(c)) = get("ShadowColor") { [c[0], c[1], c[2]] } else { [0, 0, 0] },
                };
                write_shadow(enabled, &dir, num("ShadowDistance", 0.0), num("ShadowBlurStrength", 0.0), color, num("ShadowOpacity", 100.0))
            }
            _ => write(property, new, nt, ctx).ok_or(Refusal::CannotEdit("no CSS text for this value"))?,
        };
        return Ok(replace(loc.text, loc.span.value.clone(), &new_value));
    }
    // A shorthand: replace the component it owns, or add the longhand after it.
    match (css, property) {
        ("border", "BorderWidth" | "BorderStyle" | "BorderColor") => {
            if let Some(v) = border_component(value_text, property, new, ctx) {
                return Ok(replace(loc.text, loc.span.value.clone(), &v));
            }
        }
        ("padding", "Padding") if crate::css::tokens(value_text).len() == 1 => {
            if let Value::Num(n) = new {
                return Ok(replace(loc.text, loc.span.value.clone(), &write_length(*n, nt, ctx)));
            }
        }
        ("background", "BackgroundColor") if crate::css::color(value_text).is_some() => {
            if let Value::Color(c) = new {
                return Ok(replace(loc.text, loc.span.value.clone(), &crate::css_reverse::write_color(*c, nt)));
            }
        }
        _ => {}
    }
    let lh = longhand.ok_or(Refusal::NotWritable)?;
    let text = table_text(property, new, ctx).ok_or(Refusal::CannotEdit("no CSS text for this value"))?;
    Ok(insert_after(loc.text, &loc.span, lh, &text))
}

/// Is `css` the property that sets `property` by itself (a longhand of it, or its own shorthand-less row)?
fn property_is_of(css: &str, property: &str) -> bool {
    longhand_of(property) == Some(css)
        || matches!((css, property), ("text-decoration-line", "Underline" | "Strikethrough") | ("border-radius", "CornerRadius"))
        || (css == "padding" && property == "Padding")
}

/// The declaration added to the form sheet for "only this control": into its last unconditional,
/// unlayered `#NAME` rule, or as a new rule at the end.
fn only_this_control(text: &str, set: &SheetSet, source: usize, id: &str, css: &str, value: &str, important: bool) -> String {
    let sel = format!("#{}", escape_id(id));
    let imp = if important { " !important" } else { "" };
    let nl = newline(text);
    let rule = set.rules.iter().filter(|r| r.source == source && r.layer == Layer::Unlayered && r.conditions.is_empty()).rfind(|r| r.selectors.len() == 1 && crate::css_effective::unescape(r.selectors[0].text.trim()) == crate::css_effective::unescape(&sel));
    match rule {
        Some(r) => match r.decls.last() {
            Some(last) => {
                let ind = indent_before(text, last.whole.start).unwrap_or("  ");
                let (cut, lead) = match semicolon_after(text, last.whole.end) {
                    Some(semi) => (semi + 1, String::new()),
                    None => (last.whole.end, ";".to_owned()),
                };
                format!("{}{lead}{nl}{ind}{css}: {value}{imp};{}", &text[..cut], &text[cut..])
            }
            None => format!("{}{nl}  {css}: {value}{imp};{nl}{}", &text[..r.body.start], &text[r.body.end..]),
        },
        None => {
            let mut out = text.to_owned();
            if !out.ends_with('\n') {
                out.push_str(nl);
            }
            out.push_str(&format!("{nl}{sel} {{{nl}  {css}: {value}{imp};{nl}}}{nl}"));
            out
        }
    }
}

fn error_list(css: &str, declarations: bool, opts: &Options<'_>) -> Vec<crate::stylesheet_lint::Diagnostic> {
    let all = if declarations { lint_declarations(css, opts) } else { lint(css, opts) };
    all.into_iter().filter(|d| d.severity() == Severity::Error).collect()
}

/// R18: `after` may not have a lint error that `before` did not have (by count). `declarations`:
/// the text is a `CssStyle` list, not a sheet.
pub fn lint_gate(file: Option<&str>, before: &str, after: &str, declarations: bool, opts: &Options<'_>) -> Result<(), Refusal> {
    let (old, now) = (error_list(before, declarations, opts), error_list(after, declarations, opts));
    if now.len() > old.len() {
        let d = now.into_iter().find(|d| !old.iter().any(|o| o.code == d.code && o.line == d.line)).or_else(|| error_list(after, declarations, opts).into_iter().next());
        return Err(Refusal::Lint { file: file.map(str::to_owned), line: d.as_ref().map_or(0, |d| d.line), message: d.map(|d| message(&d)).unwrap_or_default() });
    }
    Ok(())
}

/// Plan writing `new` into model property `property` of control `id`.
///
/// `sources` are the sheets in cascade order, project sheet first, then the form's: `Project` and
/// `Form` sources are writable, a `Theme` source is never written. `env` is the screen the
/// effective value is decided at. `options` is the lint's view of the project.
pub fn plan(form: &Form, id: &str, property: &str, new: &Value, sources: &[SheetSource<'_>], env: &MediaEnv, choice: Choice, options: &Options<'_>) -> Result<Plan, Refusal> {
    let ctl = form.find_control(id).ok_or(Refusal::NoControl)?;
    if matches!(property, "Visible" | "ZOrder") {
        return Err(Refusal::NotWritable);
    }
    let set = SheetSet::parse(sources);
    let eff = effective(form, Some(id), &set, env);
    let Some(entry) = eff.get(property) else { return Ok(Plan::StoreProperty) };
    // From the theme, the base or the properties layer: the developer's own value is a property.
    if entry.layer < Layer::Project {
        return Ok(Plan::StoreProperty);
    }
    if row_of(&entry.css).is_none_or(|r| r.fidelity == Fidelity::NotReversible) {
        return Err(Refusal::NotWritable);
    }
    let mut ctx = Ctx::new(ControlType::Panel);
    ctx.control_type = ctl.control_type.clone();
    // `em` in `font-size` is the size the element inherits; in any other length, its own.
    if let Some(n) = crate::layout::fonts::designed_size_opt(ctl) {
        ctx.font_px = n;
    }
    if property != "FontSize" {
        if let Some(Value::Num(n)) = eff.get("FontSize").map(|e| e.value.clone()) {
            ctx.font_px = n;
        }
    }
    let current: Vec<(&'static str, Value)> = eff.entries.iter().filter(|e| e.css == entry.css).map(|e| (e.property, e.value.clone())).collect();

    let mut new_sources: Vec<(String, String)> = sources.iter().map(|s| (s.label.to_owned(), s.text.to_owned())).collect();
    let mut changed: Vec<usize> = Vec::new();
    let mut new_style: Option<String> = None;
    let mut new_form = form.clone();

    match entry.decl {
        DeclSource::CssStyle { decl } => {
            let text = ctl.get_prop("CssStyle").map(|v| v.as_str().to_owned()).unwrap_or_default();
            let span = parse_declarations_spanned(&text, 0..text.len()).into_iter().nth(decl).ok_or(Refusal::CannotEdit("the CssStyle declaration is gone"))?;
            let edited = edit_winner(&Located { text: &text, span }, &entry.css, property, new, &current, &ctx)?;
            if let Some(c) = new_form.find_control_mut(id) {
                c.properties.insert("CssStyle".into(), PropValue::String(edited.clone()));
            }
            new_style = Some(edited);
        }
        DeclSource::Rule { rule, decl } => {
            let r = &set.rules[rule];
            let writable = |src: usize| matches!(sources[src].origin, Origin::Project | Origin::Form);
            if !writable(r.source) {
                return Ok(Plan::StoreProperty);
            }
            match choice {
                Choice::ChangeRule => {
                    let text = &sources[r.source].text;
                    let edited = edit_winner(&Located { text, span: r.decls[decl].clone() }, &entry.css, property, new, &current, &ctx)?;
                    new_sources[r.source].1 = edited;
                    changed.push(r.source);
                }
                Choice::OnlyThisControl => {
                    let target = sources.iter().rposition(|s| s.origin == Origin::Form).ok_or(Refusal::CannotEdit("the form has no stylesheet"))?;
                    let css = longhand_of(property).ok_or(Refusal::NotWritable)?;
                    let value = if css == "text-decoration" {
                        let get = |p: &str| current.iter().find(|(k, _)| *k == p).is_some_and(|(_, v)| *v == Value::Bool(true));
                        match (property, new) {
                            ("Underline", Value::Bool(b)) => write_decoration(*b, get("Strikethrough")),
                            ("Strikethrough", Value::Bool(b)) => write_decoration(get("Underline"), *b),
                            _ => return Err(Refusal::CannotEdit("not a decoration")),
                        }
                    } else {
                        table_text(property, new, &ctx).ok_or(Refusal::CannotEdit("no CSS text for this value"))?
                    };
                    new_sources[target].1 = only_this_control(&sources[target].text, &set, target, id, css, &value, entry.important);
                    changed.push(target);
                }
            }
        }
    }

    // R12: the property must come out as typed.
    let after: Vec<SheetSource<'_>> = sources.iter().zip(&new_sources).map(|(s, (_, t))| SheetSource { label: s.label, text: t, origin: s.origin }).collect();
    let after_set = SheetSet::parse(&after);
    let again = effective(&new_form, Some(id), &after_set, env);
    match again.get(property) {
        Some(e) if same(&e.value, new) => {}
        other => {
            return Err(Refusal::NoEffect { file: other.and_then(|e| e.sheet.clone()), line: other.map_or(0, |e| e.line) });
        }
    }

    // R18: no lint error the old text did not have.
    for &i in &changed {
        lint_gate(Some(sources[i].label), sources[i].text, &new_sources[i].1, false, options)?;
    }
    if let Some(style) = &new_style {
        let old = ctl.get_prop("CssStyle").map(|v| v.as_str().to_owned()).unwrap_or_default();
        lint_gate(None, &old, style, true, options)?;
    }

    let sheets = changed.into_iter().map(|i| SheetEdit { label: sources[i].label.to_owned(), before: sources[i].text.to_owned(), after: new_sources[i].1.clone() }).collect();
    Ok(Plan::Edit { sheets, css_style: new_style })
}

/// Read a declaration back for display: its model values, or unknown (a thin wrapper for the panel).
pub fn read_declaration(css: &str, value: &str, ctx: &Ctx) -> Reading {
    read(css, value, ctx)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::css::Scheme;
    use crate::model::Control;

    fn form() -> Form {
        let mut f = Form::new("F", "F", 800, 600);
        f.controls.push(Control::new("BTN-1", ControlType::Button, 0, 0));
        f.controls.push(Control::new("1ST", ControlType::Button, 0, 40));
        f
    }

    fn env() -> MediaEnv {
        MediaEnv { width: 800.0, height: 600.0, scheme: Scheme::Light }
    }

    fn src<'a>(label: &'a str, origin: Origin, text: &'a str) -> SheetSource<'a> {
        SheetSource { label, text, origin }
    }

    fn run(f: &Form, id: &str, prop: &str, v: Value, sheets: &[SheetSource<'_>], choice: Choice) -> Result<Plan, Refusal> {
        plan(f, id, prop, &v, sheets, &env(), choice, &Options::default())
    }

    fn after(p: Result<Plan, Refusal>) -> String {
        match p.expect("a plan") {
            Plan::Edit { sheets, .. } => sheets[0].after.clone(),
            other => panic!("{other:?}"),
        }
    }

    /// The bytes that differ between two texts: `(prefix kept, removed, inserted, suffix kept)`.
    fn diff<'a>(a: &'a str, b: &'a str) -> (usize, &'a str, &'a str) {
        let p = a.bytes().zip(b.bytes()).take_while(|(x, y)| x == y).count();
        let s = a[p..].bytes().rev().zip(b[p..].bytes().rev()).take_while(|(x, y)| x == y).count();
        (p, &a[p..a.len() - s], &b[p..b.len() - s])
    }

    /// AC6: byte diffs; only the named bytes change, and the value afterwards is the one typed.
    #[test]
    fn edits_change_only_the_named_bytes_and_the_value_comes_out_as_typed() {
        let f = form();
        let mut cases = 0;
        // A longhand, hex stays hex.
        let css = "#BTN-1 {\n  color: #112233;\n  margin: 0;\n}\n";
        let new = after(run(&f, "BTN-1", "ForegroundColor", Value::Color([255, 0, 0, 255]), &[src("f.css", Origin::Form, css)], Choice::ChangeRule));
        assert_eq!(diff(css, &new), (css.find("112233").unwrap(), "112233", "ff0000"));
        cases += 1;
        // Each notation stays what it was.
        for (written, expect) in [("rgb(1, 2, 3)", "rgb(255, 0, 0)"), ("hsl(10, 20%, 30%)", "hsl(0, 100%, 50%)"), ("blue", "red"), ("#abc", "#ff0000")] {
            let css = format!("#BTN-1 {{ color: {written}; }}");
            let new = after(run(&f, "BTN-1", "ForegroundColor", Value::Color([255, 0, 0, 255]), &[src("f.css", Origin::Form, &css)], Choice::ChangeRule));
            assert_eq!(new, css.replace(written, expect), "{written}");
            cases += 1;
        }
        // Lengths keep their unit.
        let css = "#BTN-1 { font-size: 2em; }";
        let new = after(run(&f, "BTN-1", "FontSize", Value::Num(28.0), &[src("f.css", Origin::Form, css)], Choice::ChangeRule));
        assert_eq!(new, css, "28px is 2em of the inherited 14: the text does not change");
        let new = after(run(&f, "BTN-1", "FontSize", Value::Num(21.0), &[src("f.css", Origin::Form, css)], Choice::ChangeRule));
        assert_eq!(new, "#BTN-1 { font-size: 1.5em; }");
        cases += 1;
        // !important is kept.
        let css = "#BTN-1 { color: red !important; }";
        let new = after(run(&f, "BTN-1", "ForegroundColor", Value::Color([0, 0, 255, 255]), &[src("f.css", Origin::Form, css)], Choice::ChangeRule));
        assert_eq!(new, "#BTN-1 { color: blue !important; }", "a name stays a name when one names the colour");
        cases += 1;
        // A shorthand: border's colour is a component; its width and style stay.
        let css = "#BTN-1 { border: 1px solid red; }";
        let new = after(run(&f, "BTN-1", "BorderColor", Value::Color([0, 0, 255, 255]), &[src("f.css", Origin::Form, css)], Choice::ChangeRule));
        assert_eq!(new, "#BTN-1 { border: 1px solid blue; }");
        let new = after(run(&f, "BTN-1", "BorderWidth", Value::Num(3.0), &[src("f.css", Origin::Form, css)], Choice::ChangeRule));
        assert_eq!(new, "#BTN-1 { border: 3px solid red; }");
        cases += 2;
        // A shorthand with no component for it: the longhand goes after it, with its indentation.
        let css = "#BTN-1 {\n    padding: 4px 8px;\n    color: red;\n}\n";
        let new = after(run(&f, "BTN-1", "PaddingLeft", Value::Num(20.0), &[src("f.css", Origin::Form, css)], Choice::ChangeRule));
        assert_eq!(new, "#BTN-1 {\n    padding: 4px 8px;\n    padding-left: 20px;\n    color: red;\n}\n");
        cases += 1;
        // background with one colour is rewritten in place.
        let css = "#BTN-1 { background: #123456; }";
        let new = after(run(&f, "BTN-1", "BackgroundColor", Value::Color([1, 2, 3, 255]), &[src("f.css", Origin::Form, css)], Choice::ChangeRule));
        assert_eq!(new, "#BTN-1 { background: #010203; }");
        cases += 1;
        // An id starting with a digit, escaped in the selector it writes.
        let css = "#\\31 ST { color: red; }";
        let new = after(run(&f, "1ST", "ForegroundColor", Value::Color([0, 128, 0, 255]), &[src("f.css", Origin::Form, css)], Choice::ChangeRule));
        assert_eq!(new, "#\\31 ST { color: green; }");
        let new = after(run(&f, "1ST", "Cursor", Value::Text("Hand".into()), &[src("f.css", Origin::Form, "#\\31 ST { cursor: wait; }")], Choice::OnlyThisControl));
        assert_eq!(new, "#\\31 ST { cursor: wait;\n  cursor: pointer; }", "the last #NAME rule takes it, after the old one");
        cases += 2;
        // Comments, CRLF and odd spacing around the value.
        let css = "/* top */\r\n#BTN-1{\r\n\tcolor :red;/* c */\r\n\tmargin:0\r\n}\r\n";
        let new = after(run(&f, "BTN-1", "ForegroundColor", Value::Color([0, 0, 255, 255]), &[src("f.css", Origin::Form, css)], Choice::ChangeRule));
        assert_eq!(new, css.replace("red", "blue"));
        cases += 1;
        // The control's own CssStyle.
        let mut g = form();
        g.controls[0].properties.insert("CssStyle".into(), PropValue::String("color: red; padding: 2px".into()));
        match run(&g, "BTN-1", "ForegroundColor", Value::Color([0, 0, 255, 255]), &[], Choice::ChangeRule).expect("plan") {
            Plan::Edit { sheets, css_style } => {
                assert!(sheets.is_empty());
                assert_eq!(css_style.as_deref(), Some("color: blue; padding: 2px"));
            }
            other => panic!("{other:?}"),
        }
        cases += 1;
        eprintln!("css_edit: {cases} byte-diff cases (longhand, 4 notations, units, !important, border components, shorthand insert, background, escaped id, CRLF, CssStyle)");
    }

    #[test]
    fn a_later_important_rule_gives_no_effect_and_names_the_file_and_line() {
        let f = form();
        let project = "#BTN-1 { color: green !important; }\n";
        let form_sheet = ".pr-button { margin: 0 }\n#BTN-1 {\n  color: red;\n}\n";
        let sheets = [src("webforms/application.css", Origin::Project, project), src("webforms/F.css", Origin::Form, form_sheet)];
        let r = run(&f, "BTN-1", "ForegroundColor", Value::Color([0, 0, 255, 255]), &sheets, Choice::ChangeRule);
        // The winner is the project sheet's important declaration; editing it works. Editing the form's red cannot.
        assert!(matches!(r, Ok(Plan::Edit { .. })), "the important declaration is the winner and is rewritten: {r:?}");
        let only = run(&f, "BTN-1", "ForegroundColor", Value::Color([0, 0, 255, 255]), &sheets, Choice::OnlyThisControl);
        assert_eq!(only, Err(Refusal::NoEffect { file: Some("webforms/application.css".into()), line: 1 }), "the new #BTN-1 rule is beaten by the important one");
    }

    #[test]
    fn a_value_from_the_theme_or_no_sheet_is_a_property_and_the_theme_is_never_written() {
        let f = form();
        let theme = src("elegance", Origin::Theme, ".pr-button { color: red; }");
        assert_eq!(run(&f, "BTN-1", "ForegroundColor", Value::Color([1, 2, 3, 255]), &[theme.clone()], Choice::ChangeRule), Ok(Plan::StoreProperty));
        assert_eq!(run(&f, "BTN-1", "ForegroundColor", Value::Color([1, 2, 3, 255]), &[], Choice::ChangeRule), Ok(Plan::StoreProperty));
        let sheet = "@layer pr-theme { .pr-button { color: red; } }\n";
        let r = run(&f, "BTN-1", "ForegroundColor", Value::Color([1, 2, 3, 255]), &[src("F.css", Origin::Form, sheet)], Choice::ChangeRule);
        assert_eq!(r, Ok(Plan::StoreProperty), "the form sheet's pr-theme block is the theme's");
        assert_eq!(run(&f, "BTN-1", "Visible", Value::Bool(false), &[], Choice::ChangeRule), Err(Refusal::NotWritable));
        assert_eq!(run(&f, "BTN-1", "ZOrder", Value::Num(2.0), &[], Choice::ChangeRule), Err(Refusal::NotWritable));
        assert_eq!(run(&f, "NOPE", "FontSize", Value::Num(2.0), &[], Choice::ChangeRule), Err(Refusal::NoControl));
    }

    #[test]
    fn only_this_control_appends_a_rule_or_joins_the_last_one() {
        let f = form();
        let css = ".pr-button { color: red }\n";
        let sheets = [src("F.css", Origin::Form, css)];
        let p = after(run(&f, "BTN-1", "ForegroundColor", Value::Color([0, 0, 255, 255]), &sheets, Choice::OnlyThisControl));
        assert_eq!(p, ".pr-button { color: red }\n\n#BTN-1 {\n  color: #0000ff;\n}\n", "appended at the end; the shared rule is untouched");
        let again = "#BTN-1 { margin: 0 }\n@media (min-width: 1px) { #BTN-1 { cursor: wait } }\n.pr-button { color: red }\n";
        let p = after(run(&f, "BTN-1", "ForegroundColor", Value::Color([0, 0, 255, 255]), &[src("F.css", Origin::Form, again)], Choice::OnlyThisControl));
        assert_eq!(p, "#BTN-1 { margin: 0;\n  color: #0000ff; }\n@media (min-width: 1px) { #BTN-1 { cursor: wait } }\n.pr-button { color: red }\n".replace("0;\n  color: #0000ff; }", "0;\n  color: #0000ff; }"), "{p}");
        let none = run(&f, "BTN-1", "ForegroundColor", Value::Color([0, 0, 255, 255]), &[src("a.css", Origin::Project, css)], Choice::OnlyThisControl);
        assert_eq!(none, Err(Refusal::CannotEdit("the form has no stylesheet")));
        // The generated sheet: the rule goes after `/* Your rules. */`.
        let generated = crate::web_themes::form_stylesheet_for(&f, None);
        let project = src("application.css", Origin::Project, ".pr-button { color: red }\n");
        let g = after(run(&f, "BTN-1", "ForegroundColor", Value::Color([0, 0, 255, 255]), &[project, src("F.css", Origin::Form, &generated)], Choice::OnlyThisControl));
        assert!(g.trim_end().ends_with("#BTN-1 {\n  color: #0000ff;\n}"), "{g}");
        assert!(g.contains("/* Your rules. */\n"), "{g}");
    }

    #[test]
    fn a_write_that_adds_a_lint_error_is_refused() {
        let opts = Options::default();
        let clean = "#BTN-1 { color: red; }\n";
        let broken = "#BTN-1 { color: red; }\n.x { color: blue \n";
        assert_eq!(lint_gate(Some("f.css"), clean, clean, false, &opts), Ok(()));
        assert_eq!(lint_gate(Some("f.css"), broken, broken, false, &opts), Ok(()), "an error already there is not the edit's");
        assert!(matches!(lint_gate(Some("f.css"), clean, broken, false, &opts), Err(Refusal::Lint { .. })), "an edit that adds one is refused");
        assert!(matches!(lint_gate(None, "color: red", "color: red; }", true, &opts), Err(Refusal::Lint { file: None, .. })), "a stray brace in a CssStyle");
        // A hostile value is quoted, so the edit stays inside its declaration and passes.
        let f = form();
        let r = plan(&f, "BTN-1", "FontName", &Value::Text("x} body{".into()), &[src("f.css", Origin::Form, "#BTN-1 { font-family: a; }")], &env(), Choice::ChangeRule, &opts);
        match r {
            Ok(Plan::Edit { sheets, .. }) => assert_eq!(sheets[0].after, "#BTN-1 { font-family: \"x} body{\"; }"),
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn remove_takes_the_declaration_and_its_line_and_leaves_an_empty_rule() {
        let css = "#A {\n  color: red;\n  margin: 0;\n}\n#B { color: blue; margin: 0 }\n#C {\n  color: red;\n}\n";
        let spans = |text: &str| parse_declarations_spanned(text, text.find('{').unwrap() + 1..text.find('}').unwrap());
        let a = spans(css);
        assert_eq!(remove(css, &a[0]), "#A {\n  margin: 0;\n}\n#B { color: blue; margin: 0 }\n#C {\n  color: red;\n}\n");
        let b_text = "#B { color: blue; margin: 0 }";
        let b = spans(b_text);
        assert_eq!(remove(b_text, &b[0]), "#B { margin: 0 }", "a shared line keeps the others");
        assert_eq!(remove(b_text, &b[1]), "#B { color: blue; }".replace("; }", "; }"));
        let c_text = "#C {\n  color: red;\n}\n";
        let c = spans(c_text);
        assert_eq!(remove(c_text, &c[0]), "#C {\n}\n", "the rule stays, empty");
        let crlf = "#D {\r\n  color: red;\r\n}\r\n";
        let d = spans(crlf);
        assert_eq!(remove(crlf, &d[0]), "#D {\r\n}\r\n");
    }
}
