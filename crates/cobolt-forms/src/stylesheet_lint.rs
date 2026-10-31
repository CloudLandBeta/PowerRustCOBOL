// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! **The stylesheet check** (WASM-007 R5, R10, R13, R16): what the stylesheet modal lists under its
//! editor, what Check reports and what stops a Build — each finding a [`Code`], its arguments and the
//! line it is on. No egui here: the IDE formats a code through its translations, the Build in English
//! ([`message`]).
//!
//! The scanner keeps lines, which the Viewer's cascade (`crate::css::Stylesheet`) does not; that parser
//! is not changed. The run-time safety check of the player ([`css_is_safe`]) is the first error of
//! [`safety`], so the Build and the browser never disagree on what is refused.

use crate::css::{color, length, split_top};

/// How bad a finding is: an error stops the Build; a warning is shown and the sheet still applies.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Severity {
    Error,
    Warning,
}

/// What a finding is. Its arguments are listed with each code.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Code {
    /// A `}` with no `{`, or a `{` never closed. No argument.
    UnbalancedBrace,
    /// A string with no closing quote on its line. No argument.
    UnterminatedString,
    /// A `/*` with no `*/`. No argument.
    UnterminatedComment,
    /// Text that is not `property: value`. Args: the text.
    NotADeclaration,
    /// A property CSS does not have. Args: the property.
    UnknownProperty,
    /// A colour or a length that does not parse. Args: the property, the value.
    BadValue,
    /// A feature outside the project's browsers. Args: the feature, what is missing.
    Unsupported,
    /// `@import`, `expression(`, `javascript:`, `vbscript:`, `behavior:`, `-moz-binding`. Args: the construct.
    Refused,
    /// A `url()` outside the project's assets, data URIs and allowed origins. Args: the address.
    RemoteUrl,
    /// A rule that moves or hides a form layer (R16). Args: the selector, the property.
    LayerRule,
}

impl Code {
    pub fn severity(self) -> Severity {
        match self {
            Code::UnbalancedBrace | Code::UnterminatedString | Code::UnterminatedComment | Code::Refused | Code::RemoteUrl | Code::LayerRule => Severity::Error,
            Code::NotADeclaration | Code::UnknownProperty | Code::BadValue | Code::Unsupported => Severity::Warning,
        }
    }

    /// Every code, for a test that each has a message in every language.
    pub const ALL: [Code; 10] = [
        Code::UnbalancedBrace,
        Code::UnterminatedString,
        Code::UnterminatedComment,
        Code::NotADeclaration,
        Code::UnknownProperty,
        Code::BadValue,
        Code::Unsupported,
        Code::Refused,
        Code::RemoteUrl,
        Code::LayerRule,
    ];
}

/// One finding: a code, its arguments, and the 1-based line it is on.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diagnostic {
    pub code: Code,
    pub line: usize,
    pub args: Vec<String>,
}

impl Diagnostic {
    pub fn severity(&self) -> Severity {
        self.code.severity()
    }
}

/// The browsers a project builds for (the fields of `cobolt_compiler::project_kind::BrowserMatrix`,
/// which this crate cannot name: the compiler depends on it).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Browsers {
    pub chromium: bool,
    pub firefox: bool,
    pub safari: bool,
    pub ios_webview: bool,
    pub android_webview: bool,
}

impl Default for Browsers {
    fn default() -> Self {
        Browsers { chromium: true, firefox: true, safari: true, ios_webview: true, android_webview: true }
    }
}

/// What the check needs to know about the project.
#[derive(Debug, Clone, Default)]
pub struct Options<'a> {
    /// The origins a `url()` may load from besides the project's own files (`https://fonts.example`).
    pub allowed_origins: &'a [String],
    pub browsers: Browsers,
    /// The form's layers, whose elements a rule must not move or hide (R16).
    pub layers: &'a [String],
}

/// The English text of a finding, for the Build's report and the player's banner.
pub fn message(d: &Diagnostic) -> String {
    let a = |i: usize| d.args.get(i).map(String::as_str).unwrap_or("");
    match d.code {
        Code::UnbalancedBrace => "unbalanced braces".to_owned(),
        Code::UnterminatedString => "a string is not closed on its line".to_owned(),
        Code::UnterminatedComment => "a comment is not closed".to_owned(),
        Code::NotADeclaration => format!("`{}` is not a declaration (`property: value`)", a(0)),
        Code::UnknownProperty => format!("`{}` is not a CSS property", a(0)),
        Code::BadValue => format!("`{}` is not a valid value for `{}`", a(1), a(0)),
        Code::Unsupported => format!("`{}` does not work in {}", a(0), a(1)),
        Code::Refused => format!("the stylesheet uses `{}`, which is refused", a(0)),
        Code::RemoteUrl => format!("the stylesheet loads `{}`, which is outside the project's assets and allowed origins", a(0)),
        Code::LayerRule => format!("`{}` sets `{}` on a form layer, which would break its stacking or visibility", a(0), a(1)),
    }
}

// ── the safety check (R13) ──────────────────────────────────────────────────────────────────────

const REFUSED: [&str; 6] = ["expression(", "javascript:", "vbscript:", "-moz-binding", "behavior:", "@import"];

fn line_of(text: &str, at: usize) -> usize {
    text[..at.min(text.len())].bytes().filter(|b| *b == b'\n').count() + 1
}

/// Is a `url()` argument one of the project's own files (relative), a data or blob URI, or a fragment?
fn internal(arg: &str) -> bool {
    arg.is_empty() || arg.starts_with("data:") || arg.starts_with("blob:") || arg.starts_with('#') || (!arg.contains(':') && !arg.starts_with("//"))
}

/// The `url()` arguments of a text, lower-cased, with their byte offsets.
fn urls(lower: &str) -> Vec<(usize, &str)> {
    let mut out = Vec::new();
    let mut from = 0;
    while let Some(i) = lower[from..].find("url(") {
        let start = from + i + 4;
        let after = &lower[start..];
        let end = after.find(')').unwrap_or(after.len());
        out.push((from + i, after[..end].trim().trim_matches(|c| c == '"' || c == '\'').trim()));
        from = start + end;
    }
    out
}

/// Every construct that would load or run something the application did not ship, in the order of
/// the text: the refused words first wherever they are (comments included, as the player always
/// refused them), then each `url()` outside the assets and the allowed origins.
pub fn safety(css: &str, allowed_origins: &[String]) -> Vec<Diagnostic> {
    let lower = css.to_ascii_lowercase();
    let mut out = Vec::new();
    for bad in REFUSED {
        for (at, _) in lower.match_indices(bad) {
            out.push(Diagnostic { code: Code::Refused, line: line_of(css, at), args: vec![bad.to_owned()] });
        }
    }
    for (at, arg) in urls(&lower) {
        if !internal(arg) && !allowed_origins.iter().any(|o| arg.starts_with(&o.to_ascii_lowercase())) {
            out.push(Diagnostic { code: Code::RemoteUrl, line: line_of(css, at), args: vec![arg[..arg.len().min(60)].to_owned()] });
        }
    }
    out
}

/// Does a stylesheet attempt to load something from outside the bundle? (WASM-007 R13: `url()` is
/// limited to the project's assets, data URIs and the allowed origins; legacy script-in-CSS is
/// refused.) The player calls this at run time and the Build through [`lint`], so the two agree.
pub fn css_is_safe(css: &str, allowed_origins: &[String]) -> Result<(), String> {
    match safety(css, allowed_origins).first() {
        Some(d) => Err(message(d)),
        None => Ok(()),
    }
}

/// The origin (`scheme://host[:port]`) of an absolute address; `None` for a relative one.
fn origin(arg: &str) -> Option<String> {
    let (scheme, rest) = if let Some(r) = arg.strip_prefix("//") { ("https", r) } else { arg.split_once("://")? };
    let host = rest.split(['/', '?', '#']).next().unwrap_or("");
    (!host.is_empty()).then(|| format!("{scheme}://{host}"))
}

/// The origins a sheet loads from, for the Content-Security-Policy the Build prints (R13, AC7):
/// `(style-src, font-src)` — a `url()` in an `@font-face` rule is a font, any other a style resource.
/// Sorted, without repeats.
pub fn sources(css: &str) -> (Vec<String>, Vec<String>) {
    let (mut style, mut font) = (std::collections::BTreeSet::new(), std::collections::BTreeSet::new());
    let scan = scan(css, &Options::default());
    let lower = css.to_ascii_lowercase();
    for (at, arg) in urls(&lower) {
        if let Some(o) = origin(arg) {
            if scan.font_face.iter().any(|r| r.contains(&at)) {
                font.insert(o);
            } else {
                style.insert(o);
            }
        }
    }
    (style.into_iter().collect(), font.into_iter().collect())
}

// ── the scanner ─────────────────────────────────────────────────────────────────────────────────

/// The text with comments blanked and strings' contents masked, the same length and the same lines,
/// so that braces and semicolons inside them never count.
fn mask(css: &str, out: &mut Vec<Diagnostic>) -> Vec<u8> {
    let b = css.as_bytes();
    let mut m = b.to_vec();
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'/' && b.get(i + 1) == Some(&b'*') {
            let end = css[i + 2..].find("*/").map(|e| i + 2 + e + 2);
            if end.is_none() {
                out.push(Diagnostic { code: Code::UnterminatedComment, line: line_of(css, i), args: vec![] });
            }
            let end = end.unwrap_or(b.len());
            for c in &mut m[i..end] {
                if *c != b'\n' {
                    *c = b' ';
                }
            }
            i = end;
            continue;
        }
        if b[i] == b'"' || b[i] == b'\'' {
            let q = b[i];
            let mut j = i + 1;
            while j < b.len() && b[j] != q && b[j] != b'\n' {
                j += if b[j] == b'\\' { 2 } else { 1 };
            }
            if j >= b.len() || b[j] == b'\n' {
                out.push(Diagnostic { code: Code::UnterminatedString, line: line_of(css, i), args: vec![] });
            }
            let end = j.min(b.len());
            for c in &mut m[i + 1..end] {
                if *c != b'\n' {
                    *c = b'x';
                }
            }
            i = end + 1;
            continue;
        }
        i += 1;
    }
    m
}

enum Frame {
    /// A rule: its selector and whether that selector can match a form layer (R16).
    Rule { selector: String, guarded: bool },
    /// `@font-face`: descriptors, not properties.
    FontFace { start: usize },
    /// `@media`, `@layer`, `@supports`, `@container`, `@scope`, `@keyframes`, …: rules inside.
    Group,
}

struct Scan {
    diags: Vec<Diagnostic>,
    /// The byte ranges of `@font-face` blocks.
    font_face: Vec<std::ops::Range<usize>>,
}

/// Can a selector list match the element of a form layer, the form root, or a layer's controls?
/// Only the subject of each selector (its last compound) is read.
fn guarded_selector(selector: &str, layers: &[String]) -> bool {
    split_top(selector, ',').into_iter().any(|sel| {
        let subject = sel.trim().rsplit([' ', '>', '+', '~']).next().unwrap_or("").to_ascii_lowercase();
        let has_class = |c: &str| subject.split('.').skip(1).any(|p| p.split([':', '[', '#']).next() == Some(c));
        has_class("pr-form-layer")
            || has_class("pr-form")
            || subject.contains("[data-layer")
            || layers.iter().any(|l| subject.split('#').skip(1).any(|p| p.split(['.', ':', '[']).next() == Some(l.to_ascii_lowercase().as_str())))
    })
}

const GUARDED_PROPERTIES: [&str; 6] = ["position", "z-index", "display", "visibility", "pointer-events", "interactivity"];

fn scan(css: &str, opts: &Options) -> Scan {
    let mut diags = Vec::new();
    let m = mask(css, &mut diags);
    let masked = String::from_utf8_lossy(&m).into_owned();
    let mut stack: Vec<(Frame, usize)> = Vec::new();
    let mut font_face = Vec::new();
    let mut pos = 0;
    let first_char = |from: usize, to: usize| from + masked[from..to].len() - masked[from..to].trim_start().len();
    loop {
        let next = masked[pos..].find(['{', '}', ';']).map(|k| pos + k);
        let end = next.unwrap_or(masked.len());
        let chunk = masked[pos..end].trim();
        let at = first_char(pos, end);
        match next.map(|k| masked.as_bytes()[k]) {
            Some(b'{') => {
                let frame = if let Some(name) = chunk.strip_prefix('@') {
                    let name = name.split(|c: char| c.is_whitespace() || c == '(').next().unwrap_or("").to_ascii_lowercase();
                    if name == "font-face" {
                        Frame::FontFace { start: pos }
                    } else {
                        Frame::Group
                    }
                } else {
                    let selector = css[at..end].trim();
                    Frame::Rule { selector: selector.to_owned(), guarded: guarded_selector(selector, opts.layers) }
                };
                stack.push((frame, line_of(css, at)));
            }
            Some(b';') | Some(b'}') | None => {
                if !chunk.is_empty() {
                    match stack.last() {
                        Some((Frame::Rule { selector, guarded }, _)) => declaration(css, at, end, Some((selector, *guarded)), false, opts, &mut diags),
                        Some((Frame::FontFace { .. }, _)) => declaration(css, at, end, None, true, opts, &mut diags),
                        // An at-statement (`@layer a, b;`, `@charset`): the refused ones are the safety check's.
                        _ if chunk.starts_with('@') => {}
                        _ => diags.push(Diagnostic { code: Code::NotADeclaration, line: line_of(css, at), args: vec![css[at..end].trim().to_owned()] }),
                    }
                }
                if next.is_some_and(|k| masked.as_bytes()[k] == b'}') {
                    match stack.pop() {
                        Some((Frame::FontFace { start }, _)) => font_face.push(start..end),
                        Some(_) => {}
                        None => diags.push(Diagnostic { code: Code::UnbalancedBrace, line: line_of(css, end), args: vec![] }),
                    }
                }
            }
            Some(_) => unreachable!(),
        }
        match next {
            Some(k) => pos = k + 1,
            None => break,
        }
    }
    for (_, line) in stack {
        diags.push(Diagnostic { code: Code::UnbalancedBrace, line, args: vec![] });
    }
    Scan { diags, font_face }
}

/// Check one `property: value` at `css[at..end]`.
fn declaration(css: &str, at: usize, end: usize, rule: Option<(&String, bool)>, descriptor: bool, opts: &Options, out: &mut Vec<Diagnostic>) {
    let text = css[at..end].trim();
    let line = line_of(css, at);
    let Some((name, value)) = text.split_once(':') else {
        out.push(Diagnostic { code: Code::NotADeclaration, line, args: vec![text.to_owned()] });
        return;
    };
    let name = name.trim().to_ascii_lowercase();
    let mut value = value.trim();
    if let Some(i) = value.to_ascii_lowercase().rfind("!important") {
        value = value[..i].trim();
    }
    if name.is_empty() || name.contains(char::is_whitespace) {
        out.push(Diagnostic { code: Code::NotADeclaration, line, args: vec![text.to_owned()] });
        return;
    }
    if name.starts_with("--") {
        return;
    }
    if let Some((engine, missing)) = prefix_gap(&name, opts.browsers) {
        out.push(Diagnostic { code: Code::Unsupported, line, args: vec![name.clone(), format!("{missing} ({engine} only)")] });
        return;
    }
    // A vendor-prefixed name is the prefix rule's alone: the table holds standard names.
    if name.starts_with('-') {
        return;
    }
    let known = if descriptor { FONT_DESCRIPTORS.contains(&name.as_str()) } else { is_property(&name) };
    if !known {
        out.push(Diagnostic { code: Code::UnknownProperty, line, args: vec![name] });
        return;
    }
    if !descriptor && !value_parses(&name, value) {
        out.push(Diagnostic { code: Code::BadValue, line, args: vec![name.clone(), value.to_owned()] });
    }
    if let Some((selector, true)) = rule {
        if GUARDED_PROPERTIES.contains(&name.as_str()) {
            out.push(Diagnostic { code: Code::LayerRule, line, args: vec![selector.clone(), name] });
        }
    }
}

/// A vendor-prefixed property some browser of the matrix does not have: `-moz-` is Gecko's alone,
/// `-ms-` and `-o-` no browser of the matrix has. (`-webkit-` is left alone: Gecko answers many of
/// those names.) Returns the engine that has it and the browsers that do not.
fn prefix_gap(name: &str, b: Browsers) -> Option<(&'static str, String)> {
    let mut missing = Vec::new();
    let engine = if name.starts_with("-moz-") {
        for (on, n) in [(b.chromium, "Chrome/Edge"), (b.safari, "Safari"), (b.ios_webview, "the iOS web view"), (b.android_webview, "Android's WebView")] {
            if on {
                missing.push(n);
            }
        }
        "Firefox"
    } else if name.starts_with("-ms-") || name.starts_with("-o-") {
        missing.push("every browser of the project");
        "legacy browsers"
    } else {
        return None;
    };
    (!missing.is_empty()).then(|| (engine, missing.join(", ")))
}

const WIDE_KEYWORDS: [&str; 6] = ["inherit", "initial", "unset", "revert", "revert-layer", "currentcolor"];

/// Colours and lengths are checked (`crate::css::{color, length}`); any other value, and any value
/// computed by `var()`, `calc()`, `env()` or another function the subset does not evaluate, passes.
fn value_parses(name: &str, value: &str) -> bool {
    let lower = value.to_ascii_lowercase();
    if lower.is_empty() {
        return false;
    }
    if WIDE_KEYWORDS.contains(&lower.as_str()) || lower.contains("var(") || lower.contains("env(") || lower.contains("attr(") {
        return true;
    }
    if COLOR_PROPERTIES.contains(&name) {
        return color(&lower).is_some() || lower.contains("color-mix(") || lower.contains("light-dark(") || lower.contains("from ");
    }
    if LENGTH_PROPERTIES.contains(&name) {
        return split_top(&lower, ' ').into_iter().filter(|p| !p.is_empty()).all(|p| {
            let starts_numeric = p.starts_with(|c: char| c.is_ascii_digit() || c == '.') || (p.starts_with('-') && p[1..].starts_with(|c: char| c.is_ascii_digit() || c == '.'));
            !starts_numeric || length(p, 16.0, Some(100.0)).is_some() || ["vw", "vh", "vmin", "vmax", "dvh", "svh", "lvh", "dvw", "ch", "ex", "cqw", "cqh", "cqi", "cqb", "fr", "cm", "mm", "in", "q", "pc"].iter().any(|u| p.strip_suffix(u).is_some_and(|n| n.parse::<f32>().is_ok()))
        });
    }
    true
}

const COLOR_PROPERTIES: &[&str] = &[
    "color", "background-color", "border-color", "border-top-color", "border-right-color", "border-bottom-color", "border-left-color",
    "border-block-color", "border-inline-color", "outline-color", "caret-color", "accent-color", "text-decoration-color", "column-rule-color",
    "fill", "stroke", "stop-color", "flood-color", "lighting-color", "text-emphasis-color", "scrollbar-color",
];

const LENGTH_PROPERTIES: &[&str] = &[
    "width", "height", "min-width", "min-height", "max-width", "max-height", "top", "right", "bottom", "left", "inset", "margin",
    "margin-top", "margin-right", "margin-bottom", "margin-left", "padding", "padding-top", "padding-right", "padding-bottom",
    "padding-left", "border-width", "border-top-width", "border-right-width", "border-bottom-width", "border-left-width",
    "border-radius", "border-top-left-radius", "border-top-right-radius", "border-bottom-left-radius", "border-bottom-right-radius",
    "gap", "row-gap", "column-gap", "font-size", "letter-spacing", "word-spacing", "outline-width", "outline-offset", "text-indent",
];

const FONT_DESCRIPTORS: &[&str] = &[
    "src", "font-family", "font-style", "font-weight", "font-stretch", "font-display", "unicode-range", "font-feature-settings",
    "font-variation-settings", "ascent-override", "descent-override", "line-gap-override", "size-adjust",
];

/// The CSS properties of current browsers (standard names; vendor prefixes are [`prefix_gap`]'s).
const PROPERTIES: &str = "accent-color align-content align-items align-self alignment-baseline all anchor-name anchor-scope animation \
animation-composition animation-delay animation-direction animation-duration animation-fill-mode animation-iteration-count animation-name \
animation-play-state animation-range animation-range-end animation-range-start animation-timeline animation-timing-function appearance \
aspect-ratio backdrop-filter backface-visibility background background-attachment background-blend-mode background-clip background-color \
background-image background-origin background-position background-position-x background-position-y background-repeat background-size \
baseline-shift block-size border border-block border-block-color border-block-end border-block-end-color border-block-end-style \
border-block-end-width border-block-start border-block-start-color border-block-start-style border-block-start-width border-block-style \
border-block-width border-bottom border-bottom-color border-bottom-left-radius border-bottom-right-radius border-bottom-style \
border-bottom-width border-collapse border-color border-end-end-radius border-end-start-radius border-image border-image-outset \
border-image-repeat border-image-slice border-image-source border-image-width border-inline border-inline-color border-inline-end \
border-inline-end-color border-inline-end-style border-inline-end-width border-inline-start border-inline-start-color \
border-inline-start-style border-inline-start-width border-inline-style border-inline-width border-left border-left-color \
border-left-style border-left-width border-radius border-right border-right-color border-right-style border-right-width border-spacing \
border-start-end-radius border-start-start-radius border-style border-top border-top-color border-top-left-radius border-top-right-radius \
border-top-style border-top-width border-width bottom box-decoration-break box-shadow box-sizing break-after break-before break-inside \
caption-side caret-color clear clip clip-path clip-rule color color-interpolation color-interpolation-filters color-scheme column-count \
column-fill column-gap column-rule column-rule-color column-rule-style column-rule-width column-span column-width columns contain \
contain-intrinsic-block-size contain-intrinsic-height contain-intrinsic-inline-size contain-intrinsic-size contain-intrinsic-width \
container container-name container-type content content-visibility counter-increment counter-reset counter-set cursor cx cy d \
direction display dominant-baseline empty-cells field-sizing fill fill-opacity fill-rule filter flex flex-basis flex-direction flex-flow \
flex-grow flex-shrink flex-wrap float flood-color flood-opacity font font-family font-feature-settings font-kerning font-language-override \
font-optical-sizing font-palette font-size font-size-adjust font-stretch font-style font-synthesis font-synthesis-position \
font-synthesis-small-caps font-synthesis-style font-synthesis-weight font-variant font-variant-alternates font-variant-caps \
font-variant-east-asian font-variant-emoji font-variant-ligatures font-variant-numeric font-variant-position font-variation-settings \
font-weight forced-color-adjust gap grid grid-area grid-auto-columns grid-auto-flow grid-auto-rows grid-column grid-column-end \
grid-column-start grid-row grid-row-end grid-row-start grid-template grid-template-areas grid-template-columns grid-template-rows \
hanging-punctuation height hyphenate-character hyphenate-limit-chars hyphens image-orientation image-rendering initial-letter \
inline-size inset inset-block inset-block-end inset-block-start inset-inline inset-inline-end inset-inline-start interactivity \
interpolate-size isolation justify-content justify-items justify-self left letter-spacing lighting-color line-break line-clamp \
line-height list-style list-style-image list-style-position list-style-type margin margin-block margin-block-end margin-block-start \
margin-bottom margin-inline margin-inline-end margin-inline-start margin-left margin-right margin-top marker marker-end marker-mid \
marker-start mask mask-border mask-clip mask-composite mask-image mask-mode mask-origin mask-position mask-repeat mask-size mask-type \
math-depth math-shift math-style max-block-size max-height max-inline-size max-width min-block-size min-height min-inline-size \
min-width mix-blend-mode object-fit object-position offset offset-anchor offset-distance offset-path offset-position offset-rotate \
opacity order orphans outline outline-color outline-offset outline-style outline-width overflow overflow-anchor overflow-block \
overflow-clip-margin overflow-inline overflow-wrap overflow-x overflow-y overlay overscroll-behavior overscroll-behavior-block \
overscroll-behavior-inline overscroll-behavior-x overscroll-behavior-y padding padding-block padding-block-end padding-block-start \
padding-bottom padding-inline padding-inline-end padding-inline-start padding-left padding-right padding-top page page-break-after \
page-break-before page-break-inside paint-order perspective perspective-origin place-content place-items place-self pointer-events \
position position-anchor position-area position-try position-try-fallbacks position-try-order position-visibility print-color-adjust \
quotes r resize right rotate row-gap ruby-align ruby-position rx ry scale scroll-behavior scroll-margin scroll-margin-block \
scroll-margin-block-end scroll-margin-block-start scroll-margin-bottom scroll-margin-inline scroll-margin-inline-end \
scroll-margin-inline-start scroll-margin-left scroll-margin-right scroll-margin-top scroll-padding scroll-padding-block \
scroll-padding-block-end scroll-padding-block-start scroll-padding-bottom scroll-padding-inline scroll-padding-inline-end \
scroll-padding-inline-start scroll-padding-left scroll-padding-right scroll-padding-top scroll-snap-align scroll-snap-stop \
scroll-snap-type scroll-timeline scroll-timeline-axis scroll-timeline-name scrollbar-color scrollbar-gutter scrollbar-width \
shape-image-threshold shape-margin shape-outside shape-rendering speak stop-color stop-opacity stroke stroke-dasharray \
stroke-dashoffset stroke-linecap stroke-linejoin stroke-miterlimit stroke-opacity stroke-width tab-size table-layout text-align \
text-align-last text-anchor text-box text-box-edge text-box-trim text-combine-upright text-decoration text-decoration-color \
text-decoration-line text-decoration-skip-ink text-decoration-style text-decoration-thickness text-emphasis text-emphasis-color \
text-emphasis-position text-emphasis-style text-indent text-justify text-orientation text-overflow text-rendering text-shadow \
text-size-adjust text-transform text-underline-offset text-underline-position text-wrap text-wrap-mode text-wrap-style timeline-scope \
top touch-action transform transform-box transform-origin transform-style transition transition-behavior transition-delay \
transition-duration transition-property transition-timing-function translate unicode-bidi user-select vector-effect vertical-align \
view-timeline view-timeline-axis view-timeline-inset view-timeline-name view-transition-class view-transition-name visibility \
white-space white-space-collapse widows width will-change word-break word-spacing writing-mode x y z-index zoom";

/// Is `name` (lower case) a CSS property?
pub fn is_property(name: &str) -> bool {
    PROPERTIES.split_ascii_whitespace().any(|p| p == name)
}

// ── the entry points ────────────────────────────────────────────────────────────────────────────

fn sort(mut v: Vec<Diagnostic>) -> Vec<Diagnostic> {
    v.sort_by_key(|d| d.line);
    v
}

/// Check a whole stylesheet: its structure, its declarations, its safety. Sorted by line.
pub fn lint(css: &str, opts: &Options) -> Vec<Diagnostic> {
    let mut v = scan(css, opts).diags;
    v.extend(safety(css, opts.allowed_origins));
    sort(v)
}

/// Check a control's inline CSS (`CssStyle`): a declaration list, on line 1 and on.
pub fn lint_declarations(text: &str, opts: &Options) -> Vec<Diagnostic> {
    // The list as the body of one rule whose selector matches nothing: the scanner's checks, and a
    // stray brace is the list's own error.
    let wrapped = format!("x{{{text}\n}}");
    let mut v: Vec<Diagnostic> = scan(&wrapped, opts).diags;
    let masked = mask(text, &mut Vec::new());
    if let Some(at) = masked.iter().position(|b| *b == b'{' || *b == b'}') {
        v.push(Diagnostic { code: Code::UnbalancedBrace, line: line_of(text, at), args: vec![] });
    }
    v.extend(safety(text, opts.allowed_origins));
    sort(v)
}

/// Is any finding an error (what stops the Build)?
pub fn has_errors(diags: &[Diagnostic]) -> bool {
    diags.iter().any(|d| d.severity() == Severity::Error)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn codes(css: &str, opts: &Options) -> Vec<(Code, usize)> {
        lint(css, opts).into_iter().map(|d| (d.code, d.line)).collect()
    }

    fn one(css: &str) -> Vec<(Code, usize)> {
        codes(css, &Options::default())
    }

    #[test]
    fn a_clean_sheet_has_no_finding() {
        let css = "/* a comment { */\n@layer pr-theme {\n  .pr-button { color: #fff; padding: 2px 10px; }\n}\n@media (min-width: 600px) {\n  #BTN-1:hover { background-color: rgb(1 2 3 / .5); width: calc(100% - 4px); }\n}\n@container (width > 20em) { .a { margin: 0 auto; } }\n.b { padding: env(safe-area-inset-top); --x: 1px; color: var(--pr-accent); }\n@font-face { font-family: \"PR\"; src: url(assets/f.woff2) format(\"woff2\"); }\n.c { content: \"}\"; }";
        let d = lint(css, &Options::default());
        assert!(d.is_empty(), "{d:?}");
    }

    #[test]
    fn unbalanced_braces_and_unterminated_strings_and_comments_are_errors_with_their_lines() {
        assert_eq!(one(".a { color: red;\n.b { color: blue; }"), [(Code::UnbalancedBrace, 1)]);
        assert_eq!(one(".a { color: red; }\n}"), [(Code::UnbalancedBrace, 2)]);
        assert_eq!(one(".a { content: \"open;\n}"), [(Code::UnterminatedString, 1)]);
        assert_eq!(one(".a { color: red; }\n\n/* never closed"), [(Code::UnterminatedComment, 3)]);
        assert_eq!(Code::UnbalancedBrace.severity(), Severity::Error);
    }

    #[test]
    fn an_unknown_property_and_a_bad_value_are_warnings_with_their_lines() {
        assert_eq!(one(".a {\n  colr: red;\n}"), [(Code::UnknownProperty, 2)]);
        assert_eq!(one(".a {\n  color: reddish;\n  width: 10pz;\n  margin: 0 12;\n  height: 50%;\n}"), [(Code::BadValue, 2), (Code::BadValue, 3), (Code::BadValue, 4)]);
        assert_eq!(one(".a { nonsense }"), [(Code::NotADeclaration, 1)]);
        assert_eq!(Code::UnknownProperty.severity(), Severity::Warning);
        assert!(one(".a { width: 10vw; height: 2dvh; flex: 1 1 0; }").is_empty());
    }

    #[test]
    fn a_prefix_outside_the_browser_matrix_is_a_warning_and_the_matrix_decides() {
        assert_eq!(one(".a {\n  -moz-appearance: none;\n}"), [(Code::Unsupported, 2)]);
        let firefox_only = Options { browsers: Browsers { chromium: false, firefox: true, safari: false, ios_webview: false, android_webview: false }, ..Options::default() };
        assert!(codes(".a { -moz-appearance: none; }", &firefox_only).is_empty());
        assert_eq!(one(".a { -ms-filter: none; }"), [(Code::Unsupported, 1)]);
        // R10: what the matrix has is not reported.
        assert!(one("@media (max-width: 1px) { .a { padding-top: env(safe-area-inset-top); } }\n@container (width > 1px) { .b { color: red; } }\n@font-face { font-family: X; src: url(assets/x.woff2); }").is_empty());
    }

    #[test]
    fn the_safety_check_refuses_what_loads_or_runs_with_the_line() {
        assert_eq!(one(".a { color: red; }\n@import url(x.css);"), [(Code::Refused, 2)]);
        assert_eq!(one(".a {\n  width: expression(alert(1));\n}").into_iter().filter(|(c, _)| *c == Code::Refused).collect::<Vec<_>>(), [(Code::Refused, 2)]);
        assert_eq!(one(".a {\n  behavior: url(x.htc);\n}").iter().filter(|(c, _)| *c == Code::Refused).count(), 1);
        assert_eq!(Code::RemoteUrl.severity(), Severity::Error);
    }

    /// AC7, natively: a remote `url()` is refused until its origin is allowed, and `sources` names it.
    #[test]
    fn a_remote_font_is_refused_until_its_origin_is_allowed_and_sources_names_it() {
        let css = "@font-face {\n  font-family: Brand;\n  src: url(https://cdn.example/x.woff2) format(\"woff2\");\n}\n.a { background: url(assets/bg.png); }";
        assert_eq!(one(css), [(Code::RemoteUrl, 3)]);
        let allowed = vec!["https://cdn.example".to_owned()];
        assert!(lint(css, &Options { allowed_origins: &allowed, ..Options::default() }).is_empty());
        assert_eq!(sources(css), (vec![], vec!["https://cdn.example".to_owned()]));
        let (style, font) = sources(".a { background: url('https://img.example/a.png') } .b { background: url(//img.example/b.png) }");
        assert_eq!((style, font), (vec!["https://img.example".to_owned()], vec![]));
        println!("AC7 (native): refused at line 3, accepted with the origin allowed, font-src = [https://cdn.example]");
    }

    /// AC10: a rule that moves or hides a form layer is an error with its line (R16).
    #[test]
    fn a_rule_that_moves_or_hides_a_layer_is_an_error_with_its_line() {
        let layers = vec!["LAYER-1".to_owned()];
        let opts = Options { layers: &layers, ..Options::default() };
        let css = ".pr-form-layer { z-index: 5 }\n#LAYER-1 { pointer-events: none }\n#LAYER-1 .x { z-index: 9 }\n.pr-form { color: red }\n.pr-form-layerx { z-index: 1 }\n.a, [data-layer=\"L\"]:hover { display: none }";
        let d = lint(css, &opts);
        let got: Vec<(Code, usize, String)> = d.iter().map(|d| (d.code, d.line, d.args.join(" "))).collect();
        assert_eq!(
            got,
            [
                (Code::LayerRule, 1, ".pr-form-layer z-index".to_owned()),
                (Code::LayerRule, 2, "#LAYER-1 pointer-events".to_owned()),
                (Code::LayerRule, 6, ".a, [data-layer=\"L\"]:hover display".to_owned()),
            ]
        );
        println!("AC10 (native): {}", d.iter().map(|d| format!("line {}: {}", d.line, message(d))).collect::<Vec<_>>().join("; "));
    }

    #[test]
    fn the_players_safety_check_answers_as_before() {
        let ok = vec!["https://fonts.example.com".to_owned()];
        assert!(css_is_safe(".a { background: url(assets/bg.png) }", &ok).is_ok());
        assert!(css_is_safe(".a { background: url(\"data:image/png;base64,AAAA\") }", &ok).is_ok());
        assert!(css_is_safe(".a { background: url(https://fonts.example.com/x.woff2) }", &ok).is_ok());
        assert_eq!(css_is_safe(".a { background: url(https://evil.example.org/x.png) }", &ok), Err("the stylesheet loads `https://evil.example.org/x.png`, which is outside the project's assets and allowed origins".to_owned()));
        assert_eq!(css_is_safe("@import url(x.css);", &ok), Err("the stylesheet uses `@import`, which is refused".to_owned()));
        assert!(css_is_safe(".a { background: url(javascript:alert(1)) }", &ok).is_err());
    }

    #[test]
    fn an_inline_declaration_list_is_checked_on_its_own_lines() {
        let d: Vec<(Code, usize)> = lint_declarations("color: red;\ncolr: blue;\nwidth: 3pz", &Options::default()).into_iter().map(|d| (d.code, d.line)).collect();
        assert_eq!(d, [(Code::UnknownProperty, 2), (Code::BadValue, 3)]);
        assert_eq!(lint_declarations("color: red } body { color: blue", &Options::default()).iter().filter(|d| d.code == Code::UnbalancedBrace).count(), 1);
        assert!(lint_declarations("", &Options::default()).is_empty());
    }

    #[test]
    fn every_code_has_an_english_message() {
        for c in Code::ALL {
            let m = message(&Diagnostic { code: c, line: 1, args: vec!["a".into(), "b".into()] });
            assert!(!m.is_empty(), "{c:?}");
        }
    }
}
