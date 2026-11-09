// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! The cascade a web form's stylesheets make on one element, as the player builds it
//! (WASM-036 R1, R4, R20; plan §0.1, §1.1). No egui and no `render` feature: Check, `render_form`
//! and the agent tools ask the same questions the designer does.
//!
//! A [`SheetSet`] is a list of sheets parsed once into [`Rule`]s: each rule's selectors, its
//! declarations with their byte ranges, its 1-based line, its layer and the conditions
//! (`@media`, `@container`, `@supports`) around it. [`cascade`] takes the element path of a control
//! (or of the form), keeps the declarations that match it, and picks for each CSS property the one
//! with the highest key `(importance-and-layer rank, specificity, source order)`:
//!
//! | Normal declarations | Rank | `!important` declarations | Rank |
//! |---|---|---|---|
//! | base | 0 | unlayered | 5 |
//! | theme | 1 | project | 6 |
//! | props | 2 | props | 7 |
//! | project | 3 | theme | 8 |
//! | unlayered: form sheet, then `CssStyle` | 4 | base | 9 |
//!
//! `CssStyle` is not an inline style: it is a rule of specificity (1,1,0) adopted after every
//! unlayered rule, so a form-sheet rule with two ids beats it. What the IDE cannot know for sure is
//! "may": a rule under an `@media` the environment does not decide, any `@container` or `@supports`,
//! a rule whose subject has a pseudo-class or an element type (the tag is the player's choice). A
//! "may" declaration is recorded and never wins. The properties layer (rank 2) is added by
//! `props_layer` (T4); this module cascades the sheets.

use std::collections::HashMap;

use crate::css::{media_eval, parse_declarations_spanned, parse_selector, split_top, substitute_vars, DeclSpan, ElementRef, MediaEnv, Selector, Tri};
use crate::css_reverse::{read, row_of, Ctx, Reading, Value};
use crate::model::{Control, ControlType, Form, PropValue};

/// Where a sheet comes from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Origin {
    /// The theme's CSS, whole: the player adopts it in `@layer pr-theme`.
    Theme,
    /// The project stylesheet: `@layer pr-app`.
    Project,
    /// The form's stylesheet: unlayered, inside `@scope`.
    Form,
}

/// The player's layers, lowest first (`@layer pr-base, pr-theme, pr-props, pr-app;`), then the
/// unlayered rules above all of them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Layer {
    Base,
    Theme,
    Props,
    Project,
    Unlayered,
}

impl Layer {
    /// The rank of a normal declaration of this layer (0–4).
    pub fn normal_rank(self) -> u8 {
        self as u8
    }

    /// The rank of an `!important` declaration of this layer (5–9): the order reverses.
    pub fn important_rank(self) -> u8 {
        9 - self as u8
    }

    fn named(name: &str) -> Layer {
        match name {
            "pr-base" => Layer::Base,
            "pr-theme" => Layer::Theme,
            "pr-props" => Layer::Props,
            // `pr-app`, and a layer the developer names: declared after the player's, so above it.
            _ => Layer::Project,
        }
    }
}

/// One sheet handed to [`SheetSet::parse`].
#[derive(Debug, Clone, Copy)]
pub struct SheetSource<'a> {
    /// What messages call it: its project-relative file name, or the theme's name.
    pub label: &'a str,
    pub text: &'a str,
    pub origin: Origin,
}

/// A parsed sheet, kept with its text so a declaration's byte ranges can be read back.
#[derive(Debug, Clone)]
pub struct Source {
    pub label: String,
    pub text: String,
    pub origin: Origin,
}

/// The kind of block a condition comes from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CondKind {
    Media,
    Container,
    Supports,
}

/// An `@media`, `@container` or `@supports` prelude around a rule.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Condition {
    pub kind: CondKind,
    /// The query as written, after the at-keyword.
    pub query: String,
}

/// One selector of a rule's list.
#[derive(Debug, Clone)]
pub struct SelectorData {
    /// As written.
    pub text: String,
    /// The testable form (a pseudo-class and the subject's element type taken out); `None` when
    /// this subset cannot read it.
    pub(crate) parsed: Option<Selector>,
    /// `true`: it was simplified to be tested, so a match is only "may".
    pub simplified: bool,
}

/// A style rule.
#[derive(Debug, Clone)]
pub struct Rule {
    /// Index into [`SheetSet::sources`].
    pub source: usize,
    /// 1-based line of its selector.
    pub line: usize,
    pub layer: Layer,
    pub conditions: Vec<Condition>,
    pub selectors: Vec<SelectorData>,
    /// Its declarations, ranges in the source's text.
    pub decls: Vec<DeclSpan>,
    /// Position in source order over the whole set.
    pub order: usize,
}

/// Sheets parsed once, in cascade order within their origin.
#[derive(Debug, Clone, Default)]
pub struct SheetSet {
    pub sources: Vec<Source>,
    pub rules: Vec<Rule>,
    /// Per source, how many selectors the subset could not read (Check reports them).
    pub unparsed: Vec<usize>,
}

impl SheetSet {
    /// Parse `sources`; their order is the source order of the cascade.
    pub fn parse(sources: &[SheetSource<'_>]) -> SheetSet {
        let mut set = SheetSet::default();
        for (si, s) in sources.iter().enumerate() {
            let mut unparsed = 0;
            for raw in scan(s.text) {
                let layer = match &raw.layer {
                    Some(name) => Layer::named(name),
                    None => match s.origin {
                        Origin::Theme => Layer::Theme,
                        Origin::Project => Layer::Project,
                        Origin::Form => Layer::Unlayered,
                    },
                };
                let mut selectors = Vec::new();
                for sel in split_top(&s.text[raw.sel_at..raw.sel_end], ',') {
                    let sel = sel.trim();
                    let Some((testable_text, simplified)) = testable(sel) else { continue };
                    let parsed = parse_selector(&testable_text);
                    if parsed.is_none() {
                        unparsed += 1;
                    }
                    selectors.push(SelectorData { text: sel.to_owned(), parsed, simplified });
                }
                if selectors.is_empty() {
                    // Every selector styles a pseudo-element: no rule for an element.
                    continue;
                }
                let decls = parse_declarations_spanned(s.text, raw.open + 1..raw.close);
                let order = set.rules.len();
                set.rules.push(Rule { source: si, line: raw.line, layer, conditions: raw.conditions, selectors, decls, order });
            }
            set.sources.push(Source { label: s.label.to_owned(), text: s.text.to_owned(), origin: s.origin });
            set.unparsed.push(unparsed);
        }
        set
    }
}

// ── The layer-aware scanner ───────────────────────────────────────────────────────────────────

struct RawRule {
    sel_at: usize,
    sel_end: usize,
    open: usize,
    close: usize,
    line: usize,
    layer: Option<String>,
    conditions: Vec<Condition>,
}

struct Frame {
    group: bool,
    cond: Option<Condition>,
    layer: Option<String>,
    skipped: bool,
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

/// The style rules of a sheet with the blocks they sit in. `@font-face`, `@keyframes`, `@page` and
/// the like hold no rule for an element and are left out; `@media`, `@container`, `@supports`,
/// `@layer` and `@scope` are groups.
fn scan(css: &str) -> Vec<RawRule> {
    let m = masked(css);
    let mb = m.as_bytes();
    let line_of = |at: usize| m[..at].bytes().filter(|&c| c == b'\n').count() + 1;
    let mut stack: Vec<Frame> = Vec::new();
    let mut out = Vec::new();
    let mut start = 0;
    let mut i = 0;
    while i < mb.len() {
        match mb[i] {
            b'{' => {
                let prelude = m[start..i].trim();
                if let Some(at) = prelude.strip_prefix('@') {
                    let name: String = at.chars().take_while(|c| c.is_ascii_alphanumeric() || *c == '-').collect::<String>().to_ascii_lowercase();
                    let rest = at[name.len()..].trim();
                    let group = matches!(name.as_str(), "media" | "container" | "supports" | "layer" | "scope");
                    let kind = match name.as_str() {
                        "media" => Some(CondKind::Media),
                        "container" => Some(CondKind::Container),
                        "supports" => Some(CondKind::Supports),
                        _ => None,
                    };
                    let layer = (name == "layer").then(|| rest.split_whitespace().next().unwrap_or("").to_owned()).filter(|l| !l.is_empty());
                    stack.push(Frame { group, cond: kind.map(|kind| Condition { kind, query: rest.to_owned() }), layer, skipped: !group });
                } else if stack.last().is_none_or(|f| f.group) {
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
                    if !stack.iter().any(|f| f.skipped) {
                        let sel_at = start + (m[start..i].len() - m[start..i].trim_start().len());
                        out.push(RawRule {
                            sel_at,
                            sel_end: i,
                            open,
                            close: j.min(css.len()),
                            line: line_of(sel_at),
                            layer: stack.iter().find_map(|f| f.layer.clone()),
                            conditions: stack.iter().filter_map(|f| f.cond.clone()).collect(),
                        });
                    }
                    i = j + 1;
                    start = i;
                    continue;
                } else {
                    stack.push(Frame { group: false, cond: None, layer: None, skipped: true });
                }
                start = i + 1;
            }
            b'}' => {
                stack.pop();
                start = i + 1;
            }
            b';' if stack.last().is_none_or(|f| f.group) => start = i + 1,
            _ => {}
        }
        i += 1;
    }
    out
}

/// A selector the subset can test, and whether it was simplified to get there (a pseudo-class taken
/// out, the subject's element type taken out). `:host` is kept: it names the root of the path.
/// `None`: it styles a pseudo-element, another box.
fn testable(selector: &str) -> Option<(String, bool)> {
    if selector.contains("::") || selector.contains(":before") || selector.contains(":after") {
        return None;
    }
    let mut out = String::new();
    let mut simplified = false;
    let mut chars = selector.trim().chars().peekable();
    while let Some(c) = chars.next() {
        if c == ':' {
            let mut name = String::new();
            while chars.peek().is_some_and(|c| c.is_ascii_alphanumeric() || *c == '-') {
                name.push(chars.next().unwrap_or(' '));
            }
            let has_args = chars.peek() == Some(&'(');
            if name == "host" && !has_args {
                out.push_str(":host");
                continue;
            }
            simplified = true;
            if has_args {
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
    let mut cut = 0;
    let mut depth = 0i32;
    let mut quote: Option<char> = None;
    for (i, c) in out.char_indices() {
        match (quote, c) {
            (Some(q), c) if c == q => quote = None,
            (Some(_), _) => {}
            (None, '"' | '\'') => quote = Some(c),
            (None, '[' | '(') => depth += 1,
            (None, ']' | ')') => depth -= 1,
            (None, ' ' | '>' | '+' | '~') if depth == 0 => cut = i + 1,
            _ => {}
        }
    }
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

// ── The element path ──────────────────────────────────────────────────────────────────────────

/// One element of the path from the root to the element being styled, as the player mounts it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PathEl {
    pub tag: &'static str,
    pub id: Option<String>,
    pub classes: Vec<String>,
    pub attrs: Vec<(String, String)>,
}

impl PathEl {
    fn new(tag: &'static str, classes: &[&str]) -> Self {
        PathEl { tag, id: None, classes: classes.iter().map(|c| (*c).to_owned()).collect(), attrs: Vec::new() }
    }
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

/// The path to the form's root element (`id` `None`) or to control `id`; `None` when the form has
/// no such control. The root is `section.pr-form-root`; a layered control sits in
/// `div.pr-form-layer`, any other in `div.pr-form` > `div.pr-surface`; a TabControl's children
/// are in its `div.pr-tabpages` > `div.pr-tabpages` > `div.pr-tabpanel`, a Splitter's in its `div.pr-pane`.
pub fn element_path(form: &Form, id: Option<&str>) -> Option<Vec<PathEl>> {
    let mut root = PathEl::new("section", &["pr-form-root"]);
    root.attrs.push(("data-pr-form".to_owned(), form.name.clone()));
    root.attrs.push(("data-pr-inst".to_owned(), "1".to_owned()));
    let mut path = vec![root];
    let Some(id) = id else { return Some(path) };
    let chain = ancestry(form, id);
    let ctl = chain.last().copied()?;
    if let Some(layer) = ctl.layer.as_deref().filter(|l| !l.is_empty()) {
        let mut l = PathEl::new("div", &["pr-form-layer"]);
        l.attrs.push(("data-layer".to_owned(), layer.to_owned()));
        path.push(l);
    } else {
        path.push(PathEl::new("div", &["pr-form"]));
        path.push(PathEl::new("div", &["pr-surface"]));
    }
    for (i, c) in chain.iter().enumerate() {
        let mut el = PathEl::new("div", &[]);
        el.id = Some(c.id.clone());
        el.classes = classes(c);
        el.attrs.push(("data-name".to_owned(), c.id.clone()));
        if let Some(layer) = c.layer.as_deref().filter(|l| !l.is_empty()) {
            el.attrs.push(("data-layer".to_owned(), layer.to_owned()));
        }
        path.push(el);
        if i + 1 < chain.len() {
            match c.control_type {
                ControlType::TabControl => {
                    path.push(PathEl::new("div", &["pr-tabpages"]));
                    path.push(PathEl::new("div", &["pr-tabpanel"]));
                }
                ControlType::Splitter => path.push(PathEl::new("div", &["pr-pane"])),
                _ => {}
            }
        }
    }
    Some(path)
}

// ── The cascade ───────────────────────────────────────────────────────────────────────────────

/// Where a winning declaration is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeclSource {
    /// `rules[rule].decls[decl]`.
    Rule { rule: usize, decl: usize },
    /// The control's `CssStyle` text: its `decl`-th declaration.
    CssStyle { decl: usize },
}

/// One declaration that matched.
#[derive(Debug, Clone)]
pub struct Matched {
    /// The CSS property, lower-case (`--name` for a custom property).
    pub css: String,
    /// The value as written.
    pub value: String,
    /// The value with `var()` resolved through the path; `None` when a `var()` has no value (the
    /// declaration is invalid at computed-value time).
    pub resolved: Option<String>,
    pub important: bool,
    pub source: DeclSource,
    pub layer: Layer,
    pub specificity: (u32, u32, u32),
    /// The sheet's index in [`SheetSet::sources`]; `None` for `CssStyle`.
    pub sheet: Option<usize>,
    /// 1-based line of the rule; 0 for `CssStyle`.
    pub line: usize,
    /// `false`: it applies only under an undecided condition, or through a simplified selector.
    pub certain: bool,
    /// Index of the selector of the rule that matched (best one); `None` for `CssStyle`.
    pub selector: Option<usize>,
    order: usize,
}

impl Matched {
    /// The cascade key: higher wins.
    pub fn key(&self) -> (u8, (u32, u32, u32), usize) {
        let rank = if self.important { self.layer.important_rank() } else { self.layer.normal_rank() };
        (rank, self.specificity, self.order)
    }
}

/// What the sheets say about one element.
#[derive(Debug, Clone, Default)]
pub struct Cascaded {
    /// For each CSS property, the certain declaration that wins it.
    pub winners: Vec<Matched>,
    /// Every "may" declaration (it never wins).
    pub may: Vec<Matched>,
    /// The custom properties in force on the element.
    pub vars: HashMap<String, String>,
}

impl Cascaded {
    pub fn winner(&self, css: &str) -> Option<&Matched> {
        self.winners.iter().find(|m| m.css == css)
    }
}

fn tri_and(a: Tri, b: Tri) -> Tri {
    match (a, b) {
        (Tri::Fails, _) | (_, Tri::Fails) => Tri::Fails,
        (Tri::Unknown, _) | (_, Tri::Unknown) => Tri::Unknown,
        _ => Tri::Holds,
    }
}

/// Do the conditions around a rule hold in `env`? With no `env` every one is undecided.
fn conditions_hold(conds: &[Condition], env: Option<&MediaEnv>) -> Tri {
    let mut all = Tri::Holds;
    for c in conds {
        let t = match (c.kind, env) {
            (CondKind::Media, Some(env)) => media_eval(&c.query, env),
            _ => Tri::Unknown,
        };
        all = tri_and(all, t);
    }
    all
}

/// The rule's best matching selector at `refs`: `(simplified, specificity, index)`, preferring a sure match,
/// then the more specific one.
fn selector_hit(rule: &Rule, refs: &[ElementRef<'_>]) -> Option<(bool, (u32, u32, u32), usize)> {
    let mut best: Option<(bool, (u32, u32, u32), usize)> = None;
    for (si, s) in rule.selectors.iter().enumerate() {
        let Some(sel) = &s.parsed else { continue };
        if sel.matches(refs) {
            let hit = (s.simplified, sel.specificity(), si);
            best = match best {
                Some(b) if (b.0, std::cmp::Reverse(b.1)) <= (hit.0, std::cmp::Reverse(hit.1)) => Some(b),
                _ => Some(hit),
            };
        }
    }
    best
}

/// Every declaration of `sheets` that matches the element at the end of `refs`.
fn matching(sheets: &SheetSet, refs: &[ElementRef<'_>], env: Option<&MediaEnv>) -> Vec<Matched> {
    let mut out = Vec::new();
    for (ri, rule) in sheets.rules.iter().enumerate() {
        let cond = conditions_hold(&rule.conditions, env);
        if cond == Tri::Fails {
            continue;
        }
        let Some((simplified, specificity, selector)) = selector_hit(rule, refs) else { continue };
        let certain = !simplified && cond == Tri::Holds;
        for (di, d) in rule.decls.iter().enumerate() {
            out.push(Matched {
                css: d.decl.name.clone(),
                value: d.decl.value.clone(),
                resolved: None,
                important: d.decl.important,
                source: DeclSource::Rule { rule: ri, decl: di },
                layer: rule.layer,
                specificity,
                sheet: Some(rule.source),
                line: rule.line,
                certain,
                selector: Some(selector),
                order: rule.order,
            });
        }
    }
    out
}

/// The highest key of `candidates` for each property, certain ones only.
fn winners_of(candidates: &[Matched]) -> Vec<Matched> {
    let mut best: Vec<Matched> = Vec::new();
    for c in candidates.iter().filter(|c| c.certain) {
        match best.iter_mut().find(|b| b.css == c.css) {
            Some(b) if b.key() >= c.key() => {}
            Some(b) => *b = c.clone(),
            None => best.push(c.clone()),
        }
    }
    best
}

/// The cascade of `sheets` on control `id` of `form` (`None`: on the form's own root), with the
/// control's `CssStyle` as the last, (1,1,0) rule. `env`: the screen `@media` is decided against;
/// `None` leaves every condition undecided.
pub fn cascade(form: &Form, id: Option<&str>, sheets: &SheetSet, env: Option<&MediaEnv>) -> Cascaded {
    let Some(path) = element_path(form, id) else { return Cascaded::default() };
    let attr_store: Vec<Vec<(&str, &str)>> = path.iter().map(|p| p.attrs.iter().map(|(n, v)| (n.as_str(), v.as_str())).collect()).collect();
    let refs: Vec<ElementRef<'_>> = path
        .iter()
        .zip(&attr_store)
        .map(|(p, attrs)| ElementRef { name: p.tag, id: p.id.as_deref(), classes: &p.classes, index: 1, count: 1, siblings: &[], attrs })
        .collect();

    // Custom properties inherit: resolve them element by element down the path.
    let mut vars: HashMap<String, String> = HashMap::new();
    for k in 0..refs.len() {
        let here: Vec<Matched> = matching(sheets, &refs[..=k], env).into_iter().filter(|m| m.css.starts_with("--")).collect();
        for w in winners_of(&here) {
            if let Some(v) = substitute_vars(&w.value, &vars) {
                vars.insert(w.css.clone(), v);
            }
        }
    }

    let mut all = matching(sheets, &refs, env);
    let mut css_style_order = sheets.rules.len();
    if let Some(ctl) = id.and_then(|id| ancestry(form, id).last().copied()) {
        if let Some(text) = ctl.get_prop("CssStyle").map(|v| v.as_str().to_owned()).filter(|s| !s.trim().is_empty()) {
            for (di, d) in parse_declarations_spanned(&text, 0..text.len()).into_iter().enumerate() {
                css_style_order += 1;
                all.push(Matched {
                    css: d.decl.name,
                    value: d.decl.value,
                    resolved: None,
                    important: d.decl.important,
                    source: DeclSource::CssStyle { decl: di },
                    layer: Layer::Unlayered,
                    specificity: (1, 1, 0),
                    sheet: None,
                    line: 0,
                    certain: true,
                    selector: None,
                    order: css_style_order,
                });
            }
        }
    }
    for m in &mut all {
        m.resolved = substitute_vars(&m.value, &vars);
    }
    let mut out = Cascaded { winners: winners_of(&all), may: all.iter().filter(|m| !m.certain).cloned().collect(), vars };
    out.may.sort_by_key(|m| m.order);
    out
}

// ── What the sheets make of the model's properties ────────────────────────────────────────────

/// One model property a winning declaration sets.
#[derive(Debug, Clone, PartialEq)]
pub struct Entry {
    /// The model property (`BackgroundColor`).
    pub property: &'static str,
    pub value: Value,
    /// The CSS property that sets it.
    pub css: String,
    /// The layer of the declaration: only `Project` and above beat the properties layer (T4).
    pub layer: Layer,
    /// The sheet's label; `None` for the control's own `CssStyle`.
    pub sheet: Option<String>,
    /// 1-based line of the rule; 0 for `CssStyle`.
    pub line: usize,
    /// The declaration's value as written.
    pub declared: String,
    /// The winning rule can match an element other than this one (plan §1.4): its subject names a
    /// type class, a `CssClass`, another attribute or no id, or its value comes through a `var()`.
    pub shared: bool,
}

/// Why the canvas cannot paint a declaration.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Why {
    /// A CSS property the reverse table does not have.
    OutsideTable,
    /// A value of the R3 list, or an unresolved `var()`.
    Unknown,
    /// Geometry or layout: the designer places controls itself.
    Geometry,
    /// It applies only under an undecided condition, or through a simplified selector.
    May,
}

/// A declaration the canvas does not paint, for the badge's tooltip.
#[derive(Debug, Clone, PartialEq)]
pub struct Unpainted {
    pub css: String,
    pub value: String,
    pub sheet: Option<String>,
    pub line: usize,
    pub why: Why,
}

/// What the sheets set on one control (or on the form), as model values.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Effective {
    pub entries: Vec<Entry>,
    pub unpainted: Vec<Unpainted>,
}

impl Effective {
    pub fn get(&self, property: &str) -> Option<&Entry> {
        self.entries.iter().find(|e| e.property == property)
    }
}

const LAYOUT: &[&str] = &[
    "width", "height", "min-width", "max-width", "min-height", "max-height", "margin", "margin-left", "margin-top", "margin-right", "margin-bottom", "top", "left", "right", "bottom", "inset", "position", "float",
    "flex", "flex-direction", "flex-wrap", "flex-grow", "flex-shrink", "flex-basis", "grid", "grid-template-columns", "grid-template-rows", "gap", "row-gap", "column-gap", "order", "align-self", "overflow",
    "overflow-x", "overflow-y", "box-sizing", "display", "transform-origin", "aspect-ratio",
];

/// The last compound of a selector, outside brackets and quotes.
fn last_compound(sel: &str) -> &str {
    let (mut cut, mut depth, mut quote) = (0, 0i32, None::<char>);
    for (i, c) in sel.char_indices() {
        match (quote, c) {
            (Some(q), c) if c == q => quote = None,
            (Some(_), _) => {}
            (None, '"' | '\'') => quote = Some(c),
            (None, '[' | '(') => depth += 1,
            (None, ']' | ')') => depth -= 1,
            (None, ' ' | '>' | '+' | '~') if depth == 0 => cut = i + 1,
            _ => {}
        }
    }
    sel[cut..].trim()
}

fn names_this_control(sel: &str, id: &str) -> bool {
    let subject = last_compound(sel).to_ascii_lowercase();
    let id = id.to_ascii_lowercase();
    let by_id = subject.match_indices(&format!("#{id}")).any(|(at, m)| !subject[at + m.len()..].starts_with(|c: char| c.is_ascii_alphanumeric() || c == '-' || c == '_'));
    by_id || subject.contains(&format!("[data-name=\"{id}\"]")) || subject.contains(&format!("[data-name='{id}']"))
}

fn is_shared(m: &Matched, sheets: &SheetSet, id: Option<&str>) -> bool {
    let DeclSource::Rule { rule, .. } = m.source else { return false };
    if m.value.contains("var(") {
        return true;
    }
    let Some(id) = id else { return false };
    let text = m.selector.and_then(|s| sheets.rules[rule].selectors.get(s)).map_or("", |s| s.text.as_str());
    !names_this_control(text, id)
}

/// What `sheets` set on control `id` (`None`: the form's root) at screen `env`: the declarations that
/// win, as model values with where they come from, and the ones the canvas cannot paint.
pub fn effective(form: &Form, id: Option<&str>, sheets: &SheetSet, env: &MediaEnv) -> Effective {
    let c = cascade(form, id, sheets, Some(env));
    let ctl = id.and_then(|id| ancestry(form, id).last().copied());
    // `em` in `font-size` is the size the element inherits; in any other property, its own.
    let mut inherited = Ctx::new(ctl.map_or(ControlType::Panel, |c| c.control_type.clone()));
    if let Some(n) = ctl.and_then(crate::layout::fonts::designed_size_opt) {
        inherited.font_px = n;
    }
    let mut ctx = inherited.clone();
    if let Some(Reading::Known(k)) = c.winner("font-size").and_then(|w| w.resolved.as_deref()).map(|v| read("font-size", v, &inherited)) {
        if let Some((_, Value::Num(n))) = k.first() {
            ctx.font_px = *n;
        }
    }
    // The properties layer (rank 2) beats the theme and the base (0, 1) for a normal declaration: where the
    // control's own property is written, the stored value is the effective one.
    let props = ctl.map(|c| props_layer(c, form)).unwrap_or_default();
    let label = |m: &Matched| m.sheet.map(|s| sheets.sources[s].label.clone());
    let mut out = Effective::default();
    for m in c.winners.iter().filter(|m| !m.css.starts_with("--")) {
        if m.layer < Layer::Props && !m.important && props.iter().any(|(k, _)| *k == m.css) {
            continue;
        }
        let unp = |why| Unpainted { css: m.css.clone(), value: m.value.clone(), sheet: label(m), line: m.line, why };
        let Some(resolved) = m.resolved.as_deref() else {
            out.unpainted.push(unp(Why::Unknown));
            continue;
        };
        match read(&m.css, resolved, if m.css == "font-size" { &inherited } else { &ctx }) {
            Reading::Known(values) => {
                let shared = is_shared(m, sheets, id);
                for (property, value) in values {
                    out.entries.push(Entry { property, value, css: m.css.clone(), layer: m.layer, sheet: label(m), line: m.line, declared: m.value.clone(), shared });
                }
            }
            Reading::Unknown(_) if row_of(&m.css).is_some() => out.unpainted.push(unp(Why::Unknown)),
            Reading::Unknown(_) => out.unpainted.push(unp(if LAYOUT.contains(&m.css.as_str()) { Why::Geometry } else { Why::OutsideTable })),
        }
    }
    for m in c.may.iter().filter(|m| !m.css.starts_with("--")) {
        out.unpainted.push(Unpainted { css: m.css.clone(), value: m.value.clone(), sheet: label(m), line: m.line, why: Why::May });
    }
    out
}

/// The colours a control paints exactly as the sheets give them (not faded by `Transparency`, which
/// the page does not apply to a colour that comes from CSS): plain data for the render engine.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct FinalColors {
    pub face: Option<[u8; 4]>,
    pub gradient: Option<([u8; 4], [u8; 4])>,
    pub border: Option<[u8; 4]>,
    pub shadow: Option<[u8; 4]>,
}

impl FinalColors {
    /// From what the developer's rules (`Project` and above) set; the theme's values are the
    /// properties' business, not CSS-final.
    pub fn from_effective(e: &Effective) -> Self {
        let colour = |p: &str| match e.entries.iter().find(|x| x.property == p && x.layer >= Layer::Project) {
            Some(Entry { value: Value::Color(c), .. }) => Some(*c),
            _ => None,
        };
        let opacity = e.entries.iter().find(|x| x.property == "ShadowOpacity").and_then(|x| if let Value::Num(n) = x.value { Some(n) } else { None });
        FinalColors {
            face: colour("BackgroundColor"),
            gradient: colour("BackgroundGradientStartColor").zip(colour("BackgroundGradientEndColor")),
            border: colour("BorderColor"),
            shadow: colour("ShadowColor").map(|c| [c[0], c[1], c[2], opacity.map_or(c[3], |o| (o / 100.0 * 255.0).round().clamp(0.0, 255.0) as u8)]),
        }
    }

    pub fn is_empty(&self) -> bool {
        *self == FinalColors::default()
    }
}

/// [`FinalColors`] per control id, for the render engine.
#[derive(Debug, Clone, Default)]
pub struct CssFinal {
    pub controls: HashMap<String, FinalColors>,
}

impl CssFinal {
    /// Every control of `form` that has something in its colours.
    pub fn of_form(form: &Form, sheets: &SheetSet, env: &MediaEnv) -> CssFinal {
        fn walk(cs: &[Control], form: &Form, sheets: &SheetSet, env: &MediaEnv, out: &mut CssFinal) {
            for c in cs {
                let f = FinalColors::from_effective(&effective(form, Some(&c.id), sheets, env));
                if !f.is_empty() {
                    out.controls.insert(c.id.clone(), f);
                }
                walk(&c.children, form, sheets, env, out);
            }
        }
        let mut out = CssFinal::default();
        walk(&form.controls, form, sheets, env, &mut out);
        out
    }

    pub fn get(&self, id: &str) -> Option<&FinalColors> {
        self.controls.get(id)
    }
}

// ── The properties layer, as data ─────────────────────────────────────────────────────────────

/// What the player's `@layer pr-props` says for one CSS property of one control.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PropsDecl {
    /// Written from the control's own property: when this layer wins, the value is the stored one.
    Own,
    /// Written, but not as the stored value (a `Bold` becomes `font-weight: 400` and a text shadow, a
    /// Label's padding is the player's inset plus the property): the property still wins the property.
    Computed,
    /// Written by the form's theme (Spatial) into the same rule.
    Theme,
}

fn text_of(c: &Control, key: &str) -> Option<String> {
    c.get_prop(key).map(|v| match v {
        PropValue::String(s) => s.trim().to_owned(),
        PropValue::Int(n) => n.to_string(),
        PropValue::Bool(b) => (if *b { "1" } else { "0" }).to_owned(),
    })
}

fn num_of(c: &Control, key: &str) -> Option<f64> {
    text_of(c, key).and_then(|s| s.parse::<f64>().ok())
}

fn flag(c: &Control, key: &str) -> bool {
    text_of(c, key).is_some_and(|s| matches!(s.to_ascii_lowercase().as_str(), "1" | "true" | "yes" | "on"))
}

fn is_chart(t: &ControlType) -> bool {
    matches!(t, ControlType::BarChart | ControlType::LineChart | ControlType::PieChart | ControlType::AreaChart | ControlType::ScatterChart | ControlType::DonutChart | ControlType::RadarChart)
}

/// The CSS properties the player writes for `ctl`'s own properties, and whose they are: the rules
/// of plan §0.2 (`cobolt-web-player/src/css.rs::appearance` and `view/style.rs::kind_decls`),
/// held to the player by its `props_layer` test over every control type. Names are the ones of
/// `CSS_TO_PROPERTIES`; a CSS property not listed is not written from a property.
pub fn props_layer(ctl: &Control, form: &Form) -> Vec<(&'static str, PropsDecl)> {
    use PropsDecl::*;
    if ctl.control_type.is_non_visual() {
        return Vec::new();
    }
    let t = &ctl.control_type;
    let mut out: Vec<(&'static str, PropsDecl)> = Vec::new();
    let face = !matches!(t, ControlType::Line);
    let transparency = num_of(ctl, "Transparency").unwrap_or(0.0);
    let _ = transparency;
    // Background.
    if face {
        if flag(ctl, "BackgroundGradientEnabled") {
            out.push(("background-image", Own));
        } else if let Some(c) = text_of(ctl, "BackgroundColor").and_then(|s| crate::css::color(&s)) {
            let default = crate::css::color(crate::model::DEFAULT_BACKGROUND_COLOR).is_some_and(|d| d[..3] == c[..3]) && c[3] == 255;
            if !default {
                out.push(("background-color", Own));
            } else if is_chart(t) && !flag(ctl, "HideBackground") {
                out.push(("background-color", Computed));
            }
        }
    }
    // Text.
    if text_of(ctl, "ForegroundColor").and_then(|s| crate::css::color(&s)).is_some() {
        out.push(("color", Own));
    }
    if text_of(ctl, "FontName").is_some_and(|s| !s.is_empty()) {
        out.push(("font-family", Own));
    }
    if num_of(ctl, "FontSize").is_some_and(|n| n > 0.0) {
        out.push(("font-size", Own));
    }
    if flag(ctl, "Bold") {
        out.push(("font-weight", Computed));
    }
    if flag(ctl, "Italic") {
        out.push(("font-style", Own));
    }
    if flag(ctl, "Underline") || flag(ctl, "Strikethrough") {
        out.push(("text-decoration", Own));
    }
    if face {
        if num_of(ctl, "CornerRadius").is_some_and(|r| r as i64 > 0) {
            out.push(("border-radius", Own));
        }
        if let Some(style) = text_of(ctl, "BorderStyle") {
            out.push(("border", Own));
            if matches!(style.to_ascii_lowercase().as_str(), "raised" | "outset" | "sunken" | "inset") {
                out.push(("border-color", Computed));
            }
        }
        if flag(ctl, "ShadowEnabled") && !matches!(t, ControlType::Timer | ControlType::AgentObject | ControlType::KnowledgeBase | ControlType::RestClient | ControlType::SqlDatabase | ControlType::IndexedFile) {
            out.push(("box-shadow", Own));
        }
    }
    if !matches!(t, ControlType::Panel | ControlType::GroupBox) {
        if num_of(ctl, "Padding").is_some_and(|p| p as i64 > 0) {
            out.push(("padding", Own));
        }
        for (k, css) in [("PaddingLeft", "padding-left"), ("PaddingTop", "padding-top"), ("PaddingRight", "padding-right"), ("PaddingBottom", "padding-bottom")] {
            if num_of(ctl, k).is_some() {
                out.push((css, Own));
            }
        }
    }
    if text_of(ctl, "Cursor").is_some_and(|s| !s.is_empty()) {
        out.push(("cursor", Own));
    }
    // A container's client area (`layout_css::container`): the padding, plus the 2 px the desktop takes off a
    // Panel or GroupBox, less the border the element draws itself; written when any side is left.
    if matches!(t, ControlType::Panel | ControlType::GroupBox | ControlType::TabControl | ControlType::Splitter | ControlType::SideMenu | ControlType::StatusBar | ControlType::ToolBar | ControlType::MenuBar) {
        let card = matches!(t, ControlType::Panel | ControlType::GroupBox);
        let all = num_of(ctl, "Padding").unwrap_or(0.0);
        let border = if card && flag(ctl, "HideBackground") {
            0.0
        } else {
            let width = num_of(ctl, "BorderWidth").unwrap_or(1.0).clamp(0.0, 32.0);
            match text_of(ctl, "BorderStyle").map(|s| s.to_ascii_lowercase()) {
                None => if matches!(t, ControlType::GroupBox) { 1.0 } else { 0.0 },
                Some(s) if s.is_empty() || s == "none" => 0.0,
                Some(s) if s == "double" => width.max(3.0),
                Some(_) => width,
            }
        };
        let chrome = if card { 2.0 } else { 0.0 };
        let left = ["PaddingTop", "PaddingRight", "PaddingBottom", "PaddingLeft"].iter().map(|k| (num_of(ctl, k).unwrap_or(all) + chrome - border).max(0.0)).sum::<f64>();
        if left > 0.0 {
            out.retain(|(k, _)| *k != "padding");
            out.push(("padding", Computed));
        }
    }
    // What a type adds by itself: always written, from the properties or their defaults.
    match t {
        ControlType::Label => {
            out.push(("display", Computed));
            for css in ["justify-content", "align-items", "text-align", "white-space"] {
                out.push((css, Computed));
            }
            if !out.iter().any(|(k, _)| *k == "padding") {
                out.push(("padding", Computed));
            }
        }
        ControlType::Button => {
            for css in ["display", "align-items", "justify-content"] {
                out.push((css, Computed));
            }
            if !out.iter().any(|(k, _)| *k == "padding") {
                out.push(("padding", Computed));
            }
        }
        ControlType::TextBox => {
            out.push(("text-align", Computed));
            if !out.iter().any(|(k, _)| *k == "padding") {
                out.push(("padding", Computed));
            }
        }
        ControlType::PictureBox => out.push(("background", Computed)),
        _ => {}
    }
    // `ZOrder` and `Visible` are the layout's: the player writes no `z-index` or `display` from them.
    spatial_props(ctl, form, &mut out);
    out
}

/// What the Spatial theme writes into the same rule (`theme_css.rs::spatial_decls`): where the
/// properties are silent, its radius, its face and its rim.
fn spatial_props(ctl: &Control, form: &Form, out: &mut Vec<(&'static str, PropsDecl)>) {
    if !form.theme.as_deref().is_some_and(|t| t.trim().eq_ignore_ascii_case(crate::theme::SPATIAL)) {
        return;
    }
    let t = &ctl.control_type;
    let gradient = flag(ctl, "BackgroundGradientEnabled");
    let seeded = [crate::model::DEFAULT_BACKGROUND_COLOR, crate::model::NEUMORPHIC_SURFACE_COLOR, crate::model::NEUMORPHIC_DARK_SURFACE_COLOR];
    let named = text_of(ctl, "BackgroundColor").and_then(|s| crate::css::color(&s)).filter(|c| c[3] > 0 && !seeded.iter().any(|s| crate::css::color(s).is_some_and(|d| d == *c)));
    #[derive(PartialEq)]
    enum Face {
        Card,
        Input,
        Button,
        Label,
    }
    let face = match t {
        ControlType::Button => Some(Face::Button),
        ControlType::Panel | ControlType::GroupBox => (!flag(ctl, "HideBackground") && !gradient).then_some(Face::Card),
        ControlType::TextBox | ControlType::ComboBox | ControlType::NumericUpDown | ControlType::DateTimePicker => (!gradient).then_some(Face::Input),
        ControlType::Label => (named.is_some() && !gradient).then_some(Face::Label),
        _ => None,
    };
    let Some(face) = face else { return };
    fn put(out: &mut Vec<(&'static str, PropsDecl)>, css: &'static str) {
        if !out.iter().any(|(k, _)| *k == css) {
            out.push((css, PropsDecl::Theme));
        }
    }
    let seeded_radius = if matches!(t, ControlType::Button) { 3 } else { 0 };
    let own_radius = num_of(ctl, "CornerRadius").or_else(|| num_of(ctl, "BorderRadius")).map(|r| r as i64);
    if face != Face::Label && own_radius.is_none_or(|r| r == seeded_radius) {
        put(out, "border-radius");
    }
    if !out.iter().any(|(k, _)| *k == "background-image") && (named.is_some() || face != Face::Label) {
        put(out, "background-color");
    }
    let style = text_of(ctl, "BorderStyle").unwrap_or_default().to_ascii_lowercase();
    let width = num_of(ctl, "BorderWidth").unwrap_or(1.0).clamp(0.0, 32.0);
    let own_border = !matches!(style.as_str(), "" | "none") && (width > 0.0 || style == "double");
    if !own_border && num_of(ctl, "Transparency").unwrap_or(0.0) < 100.0 {
        put(out, "box-shadow");
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::css::Scheme;
    use crate::model::PropValue;

    fn form() -> Form {
        let mut f = Form::new("F", "F", 800, 600);
        let mut b = Control::new("BTN-1", ControlType::Button, 0, 0);
        b.properties.insert("CssClass".into(), PropValue::String("primary".into()));
        f.controls.push(b);
        f.controls.push(Control::new("BTN-2", ControlType::Button, 0, 40));
        f
    }

    fn set(parts: &[(&str, Origin, &str)]) -> SheetSet {
        let sources: Vec<SheetSource<'_>> = parts.iter().map(|(l, o, t)| SheetSource { label: l, text: t, origin: *o }).collect();
        SheetSet::parse(&sources)
    }

    fn value(c: &Cascaded, css: &str) -> Option<String> {
        c.winner(css).and_then(|m| m.resolved.clone())
    }

    /// Plan §6.1, the cascade rows: each layer against the next, `!important` reversing them,
    /// `CssStyle` beaten by a two-id rule, source order, specificity.
    #[test]
    fn the_cascade_of_the_player_one_case_per_row() {
        let f = form();
        let mut cases = 0;
        // Each layer against the next: the same selector, a different value in each source.
        let layers = [(Origin::Theme, "theme"), (Origin::Project, "project"), (Origin::Form, "form")];
        for hi in 1..layers.len() {
            let parts: Vec<(&str, Origin, String)> = layers.iter().map(|(o, n)| (*n, *o, format!("#BTN-1 {{ color: {n} }}"))).collect();
            let refs: Vec<(&str, Origin, &str)> = parts.iter().map(|(l, o, t)| (*l, *o, t.as_str())).collect();
            let c = cascade(&f, Some("BTN-1"), &set(&refs[..=hi]), None);
            assert_eq!(value(&c, "color").as_deref(), Some(layers[hi].1), "{} beats the layers below it", layers[hi].1);
            cases += 1;
        }
        // Layers named inside a sheet: base < theme < props < project (the player's order), unlayered above all.
        let sheet = "@layer pr-app { #BTN-1 { color: app } } @layer pr-props { #BTN-1 { color: props } } @layer pr-theme { #BTN-1 { color: theme } } @layer pr-base { #BTN-1 { color: base } }";
        let c = cascade(&f, Some("BTN-1"), &set(&[("s.css", Origin::Form, sheet)]), None);
        assert_eq!(value(&c, "color").as_deref(), Some("app"));
        let c = cascade(&f, Some("BTN-1"), &set(&[("s.css", Origin::Form, "@layer pr-theme { #BTN-1 { color: theme } } @layer pr-base { #BTN-1 { color: base } }")]), None);
        assert_eq!(value(&c, "color").as_deref(), Some("theme"));
        let c = cascade(&f, Some("BTN-1"), &set(&[("s.css", Origin::Form, "@layer pr-props { #BTN-1 { color: props } } @layer pr-theme { #BTN-1 { color: theme } }")]), None);
        assert_eq!(value(&c, "color").as_deref(), Some("props"));
        cases += 3;
        // !important reverses them: a theme !important beats the form's normal rule, and base beats theme.
        let c = cascade(
            &f,
            Some("BTN-1"),
            &set(&[("theme.css", Origin::Theme, "#BTN-1 { color: theme !important }"), ("f.css", Origin::Form, "#BTN-1 { color: form }")]),
            None,
        );
        assert_eq!(value(&c, "color").as_deref(), Some("theme"), "important in the theme layer beats an unlayered normal");
        let c = cascade(&f, Some("BTN-1"), &set(&[("f.css", Origin::Form, "@layer pr-base { #BTN-1 { color: base !important } } @layer pr-theme { #BTN-1 { color: theme !important } } #BTN-1 { color: form !important }")]), None);
        assert_eq!(value(&c, "color").as_deref(), Some("base"), "among important ones the order reverses: base wins");
        let c = cascade(&f, Some("BTN-1"), &set(&[("f.css", Origin::Form, "#BTN-1 { color: a !important } #BTN-1 { color: b !important }")]), None);
        assert_eq!(value(&c, "color").as_deref(), Some("b"), "equal importance: the later one");
        cases += 3;
        // CssStyle: (1,1,0), after every unlayered rule; beaten by a two-id rule and by !important.
        let mut g = form();
        g.controls[1].properties.insert("CssStyle".into(), PropValue::String("color: style; margin: 1px".into()));
        let beats = cascade(&g, Some("BTN-2"), &set(&[("f.css", Origin::Form, "#BTN-2 { color: blue }")]), None);
        assert_eq!(value(&beats, "color").as_deref(), Some("style"), "CssStyle comes after an id rule");
        assert!(matches!(beats.winner("color").map(|m| m.source), Some(DeclSource::CssStyle { decl: 0 })));
        let two_ids = cascade(&g, Some("BTN-2"), &set(&[("f.css", Origin::Form, "#BTN-2 { color: blue } #BTN-2.pr-button.pr-control { color: teal }")]), None);
        assert_eq!(value(&two_ids, "color").as_deref(), Some("teal"), "(1,2,0) beats CssStyle (1,1,0)");
        let imp = cascade(&g, Some("BTN-2"), &set(&[("f.css", Origin::Form, "#BTN-2 { color: blue !important }")]), None);
        assert_eq!(value(&imp, "color").as_deref(), Some("blue"), "an important rule beats CssStyle");
        cases += 3;
        // Source order and specificity.
        let c = cascade(&f, Some("BTN-1"), &set(&[("f.css", Origin::Form, ".primary { color: first } .primary { color: second }")]), None);
        assert_eq!(value(&c, "color").as_deref(), Some("second"));
        let c = cascade(&f, Some("BTN-1"), &set(&[("f.css", Origin::Form, "#BTN-1 { color: id } .pr-button.primary { color: classes } *{color: star}")]), None);
        assert_eq!(value(&c, "color").as_deref(), Some("id"), "the id outweighs two classes");
        cases += 2;
        eprintln!("css_effective cascade: {cases} cases (layer pairs, named layers, !important reversal, CssStyle, order, specificity)");
    }

    #[test]
    fn var_resolves_through_the_path_and_through_host() {
        let f = form();
        let sheet = ":host { --accent: red; --pad: 4px } .pr-surface { --accent: green } #BTN-1 { color: var(--accent); padding: var(--pad); margin: var(--none) } #BTN-2 { color: var(--accent, blue) }";
        let s = set(&[("f.css", Origin::Form, sheet)]);
        let c = cascade(&f, Some("BTN-1"), &s, None);
        assert_eq!(value(&c, "color").as_deref(), Some("green"), "the nearer ancestor's value inherits down");
        assert_eq!(value(&c, "padding").as_deref(), Some("4px"), "from :host, the root");
        assert!(c.winner("margin").is_some_and(|m| m.resolved.is_none()), "an unresolved var(): no value");
        assert_eq!(c.winner("margin").map(|m| m.value.as_str()), Some("var(--none)"));
        assert_eq!(c.vars.get("--accent").map(String::as_str), Some("green"));
        let root = cascade(&f, None, &s, None);
        assert_eq!(root.vars.get("--accent").map(String::as_str), Some("red"), "the form's own path ends at the root");
        eprintln!("css_effective var(): path inheritance, :host, unresolved, the form's root checked");
    }

    #[test]
    fn the_form_tabcontrol_and_splitter_paths_are_the_players() {
        let mut f = Form::new("F", "F", 800, 600);
        let mut tabs = Control::new("TABS", ControlType::TabControl, 0, 0);
        tabs.children.push(Control::new("IN-TAB", ControlType::Button, 0, 0));
        let mut split = Control::new("SPL", ControlType::Splitter, 0, 0);
        split.children.push(Control::new("IN-SPL", ControlType::Button, 0, 0));
        f.controls.push(tabs);
        f.controls.push(split);
        let css = ".pr-tabcontrol > .pr-tabpages > .pr-tabpanel > .pr-button { color: tab } .pr-splitter > .pr-pane > .pr-button { color: pane } [data-pr-form=\"F\"] { --root: yes } section.pr-form-root { margin: 0 }";
        let s = set(&[("f.css", Origin::Form, css)]);
        assert_eq!(value(&cascade(&f, Some("IN-TAB"), &s, None), "color").as_deref(), Some("tab"));
        assert_eq!(value(&cascade(&f, Some("IN-SPL"), &s, None), "color").as_deref(), Some("pane"));
        assert!(cascade(&f, Some("TABS"), &s, None).winner("color").is_none(), "the container itself is not in its page");
        let root = cascade(&f, None, &s, None);
        assert_eq!(root.vars.get("--root").map(String::as_str), Some("yes"), "[data-pr-form] is on the root");
        assert!(root.winner("margin").is_none(), "section.pr-form-root names an element type: may");
        assert!(root.may.iter().any(|m| m.css == "margin"));
        assert!(cascade(&f, Some("NOPE"), &s, None).winners.is_empty());
        eprintln!("css_effective paths: form root, TabControl page, Splitter pane, unknown id");
    }

    #[test]
    fn attribute_selectors_see_the_players_attributes() {
        let mut f = form();
        f.controls[0].layer = Some("LAYER-1".into());
        let css = "[data-name=\"BTN-1\"] { color: exact } [data-name^=\"btn-\" i] { margin: 0 } [data-name$=\"2\"] { padding: 0 } [data-layer=\"LAYER-1\"] { cursor: wait }";
        let s = set(&[("f.css", Origin::Form, css)]);
        let c = cascade(&f, Some("BTN-1"), &s, None);
        assert_eq!(value(&c, "color").as_deref(), Some("exact"));
        assert!(c.winner("margin").is_some() && c.winner("padding").is_none() && c.winner("cursor").is_some());
        let c2 = cascade(&f, Some("BTN-2"), &s, None);
        assert!(c2.winner("color").is_none() && c2.winner("padding").is_some() && c2.winner("cursor").is_none());
    }

    #[test]
    fn a_pseudo_class_or_an_undecided_condition_is_may_and_never_wins() {
        let f = form();
        let css = "#BTN-1 { color: sure }\n#BTN-1:hover { color: hover }\n@media (hover: hover) { #BTN-1 { color: media } }\n@supports (display: grid) { #BTN-1 { padding: 0 } }\n@media (max-width: 500px) { #BTN-1 { cursor: wait } }\n@media (min-width: 500px) { #BTN-1 { margin: 0 } }";
        let s = set(&[("f.css", Origin::Form, css)]);
        let env = MediaEnv { width: 800.0, height: 600.0, scheme: Scheme::Light };
        let c = cascade(&f, Some("BTN-1"), &s, Some(&env));
        assert_eq!(value(&c, "color").as_deref(), Some("sure"), "a later may never beats a sure one");
        let may: Vec<(&str, usize)> = c.may.iter().map(|m| (m.css.as_str(), m.line)).collect();
        assert!(may.contains(&("color", 2)) && may.contains(&("color", 3)), "{may:?}");
        assert!(may.contains(&("padding", 4)), "@supports is undecided: {may:?}");
        assert!(c.winner("cursor").is_none() && !c.may.iter().any(|m| m.css == "cursor"), "a Fails condition: not a candidate");
        assert!(c.winner("margin").is_some(), "a Holds condition: a sure declaration");
        // With no environment every @media is undecided.
        let c = cascade(&f, Some("BTN-1"), &s, None);
        assert!(c.winner("margin").is_none() && c.may.iter().any(|m| m.css == "margin"));
        // Dark scheme decides prefers-color-scheme.
        let dark = set(&[("f.css", Origin::Form, "@media (prefers-color-scheme: dark) { #BTN-1 { color: night } }")]);
        let d = MediaEnv { width: 412.0, height: 915.0, scheme: Scheme::Dark };
        assert_eq!(value(&cascade(&f, Some("BTN-1"), &dark, Some(&d)), "color").as_deref(), Some("night"));
        assert!(cascade(&f, Some("BTN-1"), &dark, Some(&MediaEnv { scheme: Scheme::Light, ..d })).winners.is_empty());
        eprintln!("css_effective may: :hover, undecided @media, @supports, decided Holds/Fails, Light/Dark");
    }


    fn env() -> MediaEnv {
        MediaEnv { width: 800.0, height: 600.0, scheme: Scheme::Light }
    }

    /// T3: `effective()` turns winning declarations into model values, keeps what it cannot paint
    /// with its line, and decides `shared`.
    #[test]
    fn effective_reads_model_values_and_lists_what_the_canvas_cannot_paint() {
        let f = form();
        let css = ".pr-button { color: #112233; width: 50% }\n#BTN-1 { background-color: rgb(1, 2, 3); font-size: 1.5em; margin: 4px; transform: scale(2); padding: var(--none) }\n#BTN-1:hover { cursor: wait }\n@media (min-width: 1000px) { #BTN-1 { border-width: 9px } }\n";
        let s = set(&[("webforms/F.css", Origin::Form, css)]);
        let e = effective(&f, Some("BTN-1"), &s, &env());
        assert_eq!(e.get("ForegroundColor").map(|x| x.value.clone()), Some(Value::Color([0x11, 0x22, 0x33, 255])));
        assert_eq!(e.get("BackgroundColor").map(|x| (x.sheet.clone(), x.line, x.declared.clone())), Some((Some("webforms/F.css".into()), 2, "rgb(1, 2, 3)".into())));
        assert_eq!(e.get("FontSize").map(|x| x.value.clone()), Some(Value::Num(21.0)), "1.5em of the default 14");
        assert!(e.get("ForegroundColor").is_some_and(|x| x.shared), "a type-class rule can match another control");
        assert!(e.get("BackgroundColor").is_some_and(|x| !x.shared), "an id rule is this control's own");
        let why = |css: &str| e.unpainted.iter().find(|u| u.css == css).map(|u| (u.why, u.line));
        assert_eq!(why("width"), Some((Why::Geometry, 1)), "width: 50% is layout, with its line");
        assert_eq!(why("margin"), Some((Why::Geometry, 2)));
        assert_eq!(why("transform"), Some((Why::Unknown, 2)), "the row exists, the model cannot hold it");
        assert_eq!(why("padding"), Some((Why::Unknown, 2)), "an unresolved var()");
        assert_eq!(why("cursor"), Some((Why::May, 3)), ":hover is a pseudo-class");
        assert_eq!(why("border-width"), None, "width 800 < 1000: the condition Fails, so it is not even a candidate");
    }

    #[test]
    fn a_var_value_is_shared_and_css_style_is_not() {
        let mut f = form();
        f.controls[1].properties.insert("CssStyle".into(), PropValue::String("color: red".into()));
        let s = set(&[("a.css", Origin::Form, ":host { --c: blue } #BTN-1 { color: var(--c) } [data-name=\"BTN-1\"] { cursor: wait } .primary { padding: 2px }")]);
        let e = effective(&f, Some("BTN-1"), &s, &env());
        assert!(e.get("ForegroundColor").is_some_and(|x| x.shared && x.value == Value::Color([0, 0, 255, 255])), "through var(): shared");
        assert!(e.get("Cursor").is_some_and(|x| !x.shared), "[data-name] of this control is its own");
        assert!(e.get("PaddingLeft").is_none() && e.get("Padding").is_some_and(|x| x.shared), "a CssClass rule is shared");
        let own = effective(&f, Some("BTN-2"), &s, &env());
        assert!(own.get("ForegroundColor").is_some_and(|x| !x.shared && x.sheet.is_none() && x.line == 0), "CssStyle is this control's alone");
        let colors = CssFinal::of_form(&f, &s, &env());
        assert!(colors.get("BTN-2").is_none(), "a foreground colour is not a CSS-final face");
        assert_eq!(colors.get("BTN-1").and_then(|c| c.face), None);
        let fill = set(&[("a.css", Origin::Form, "#BTN-1 { background: linear-gradient(to right, red, blue); border: 2px solid green; box-shadow: 2px 2px 4px rgba(0, 0, 0, 0.5) }")]);
        let c = CssFinal::of_form(&f, &fill, &env());
        let b = c.get("BTN-1").expect("colours");
        assert_eq!(b.gradient, Some(([255, 0, 0, 255], [0, 0, 255, 255])));
        assert_eq!(b.border, Some([0, 128, 0, 255]));
        assert_eq!(b.shadow, Some([0, 0, 0, 128]), "ShadowOpacity 50 is alpha 128");
        eprintln!("css_effective effective(): model values, unpainted (geometry, Unknown, may), shared, CssFinal colours");
    }

    #[test]
    fn the_properties_layer_beats_the_theme_but_not_important_or_the_developer() {
        let mut f = form();
        f.controls[0].properties.insert("ForegroundColor".into(), PropValue::String("#FFFFFFFF".into()));
        let theme = ".pr-button { color: red; background-color: blue } .pr-label { color: green }";
        let s = set(&[("elegance", Origin::Theme, theme)]);
        let e = effective(&f, Some("BTN-1"), &s, &env());
        assert!(e.get("ForegroundColor").is_none(), "the property is written into pr-props: the stored value is the effective one");
        assert!(e.get("BackgroundColor").is_some_and(|x| x.layer == Layer::Theme && x.sheet.as_deref() == Some("elegance")), "no property written: the theme's face shows");
        let important = set(&[("elegance", Origin::Theme, ".pr-button { color: red !important }")]);
        assert!(effective(&f, Some("BTN-1"), &important, &env()).get("ForegroundColor").is_some(), "an important theme declaration beats pr-props");
        let dev = set(&[("elegance", Origin::Theme, theme), ("f.css", Origin::Form, "#BTN-1 { color: teal }")]);
        assert!(effective(&f, Some("BTN-1"), &dev, &env()).get("ForegroundColor").is_some_and(|x| x.layer == Layer::Unlayered));
        let layer = props_layer(&f.controls[0], &f);
        assert!(layer.contains(&("color", PropsDecl::Own)) && layer.contains(&("align-items", PropsDecl::Computed)) && layer.contains(&("padding", PropsDecl::Computed)), "{layer:?}");
        assert!(props_layer(&Control::new("T-1", ControlType::Timer, 0, 0), &f).is_empty(), "a non-visual control has no box");
    }
    #[test]
    fn rules_keep_their_lines_layers_conditions_and_declaration_ranges() {
        let css = "/* c */\n#A { color: red; margin: 0 !important }\n@layer pr-theme {\n  @media (min-width: 1px) { .x, .y:hover { padding: 1px } }\n}\n@font-face { font-family: F; src: url(x) }\n#B::after { color: blue }\n";
        let s = set(&[("a.css", Origin::Form, css)]);
        assert_eq!(s.rules.len(), 2, "@font-face and the pseudo-element are not rules for an element");
        let a = &s.rules[0];
        assert_eq!((a.line, a.layer, a.conditions.len()), (2, Layer::Unlayered, 0));
        assert_eq!(&css[a.decls[0].value.clone()], "red");
        assert_eq!(&css[a.decls[1].whole.clone()], "margin: 0 !important");
        let b = &s.rules[1];
        assert_eq!((b.line, b.layer), (4, Layer::Theme));
        assert_eq!(b.conditions, vec![Condition { kind: CondKind::Media, query: "(min-width: 1px)".into() }]);
        assert_eq!(b.selectors.len(), 2);
        assert!(!b.selectors[0].simplified || b.selectors[0].text == ".x", "{:?}", b.selectors[0]);
        assert!(b.selectors[1].simplified, ".y:hover is simplified");
        assert_eq!(s.unparsed, vec![0]);
        let bad = set(&[("b.css", Origin::Form, "a[unclosed { color: red }")]);
        assert_eq!(bad.unparsed, vec![1], "a selector the subset cannot read is counted, not guessed");
    }
}
