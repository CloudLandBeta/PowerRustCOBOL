// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! The CSS the Viewer's HTML subset honours (operator, 2026-09-27: "the page's
//! CSS must be applied too").
//!
//! **Styling, not layout.** A page's `<style>` blocks and `style="…"`
//! attributes are read and cascaded — selectors, specificity, source order,
//! `!important`, inheritance, custom properties — and what they say about
//! colour, type, boxes (background, border, padding, margin, width, shadow)
//! and alignment reaches the paint. What a *layout engine* would do — flex,
//! grid, floats, positioning, animation — is not done: such a page keeps its
//! content, stacked, which is the subset's standing promise.
//!
//! **Nothing is fetched.** `@import`, `<link rel=stylesheet>` and every
//! `url(…)` are ignored: a viewer that loaded what a page asked for would be a
//! browser, with a browser's attack surface.
//!
//! `@media` blocks are skipped whole. The pane is desktop-wide, and the rules
//! a page puts behind a media query are, overwhelmingly, its narrow-screen
//! overrides.

use std::collections::HashMap;

/// One `name: value` pair, as written (the name lower-cased).
#[derive(Debug, Clone, PartialEq)]
pub struct Decl {
    pub name: String,
    pub value: String,
    pub important: bool,
}

/// What a selector is tested against: one element of the path from the root
/// to the element being styled.
#[derive(Debug, Clone, Copy)]
pub struct ElementRef<'a> {
    /// The tag name, lower-case.
    pub name: &'a str,
    pub id: Option<&'a str>,
    pub classes: &'a [String],
    /// 1-based position among the parent's element children.
    pub index: usize,
    /// How many element children the parent has.
    pub count: usize,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum Pseudo {
    FirstChild,
    LastChild,
    OnlyChild,
    Root,
    /// `:nth-child(an+b)`
    Nth(i32, i32),
    /// `:nth-last-child(an+b)`
    NthLast(i32, i32),
}

#[derive(Debug, Clone, Default, PartialEq)]
struct Compound {
    tag: Option<String>,
    id: Option<String>,
    classes: Vec<String>,
    pseudos: Vec<Pseudo>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum Combinator {
    Descendant,
    Child,
}

/// A complex selector, left to right: each compound after the first says how
/// it relates to the one before it.
#[derive(Debug, Clone, PartialEq)]
struct Selector {
    parts: Vec<(Combinator, Compound)>,
}

#[derive(Debug, Clone)]
struct Rule {
    selector: Selector,
    specificity: (u32, u32, u32),
    order: usize,
    decls: std::rc::Rc<Vec<Decl>>,
}

/// A parsed style sheet.
#[derive(Debug, Clone, Default)]
pub struct Stylesheet {
    rules: Vec<Rule>,
}

impl Stylesheet {
    /// Parse one or more style sheets' text. Never fails: a rule this subset
    /// cannot read is dropped and the rest are kept, which is what a browser
    /// does with a rule it does not understand.
    pub fn parse(css: &str) -> Self {
        let text = strip_comments(css);
        let mut rules = Vec::new();
        let mut rest = text.as_str();
        loop {
            rest = rest.trim_start();
            if rest.is_empty() {
                break;
            }
            // An at-rule: a statement to its `;`, or a block skipped whole.
            if rest.starts_with('@') {
                let semi = rest.find(';');
                let brace = rest.find('{');
                match (semi, brace) {
                    (Some(s), Some(b)) if s < b => rest = &rest[s + 1..],
                    (Some(s), None) => rest = &rest[s + 1..],
                    (_, Some(b)) => match block_end(&rest[b..]) {
                        Some(e) => rest = &rest[b + e + 1..],
                        None => break,
                    },
                    (None, None) => break,
                }
                continue;
            }
            let Some(b) = rest.find('{') else { break };
            let prelude = &rest[..b];
            let Some(e) = block_end(&rest[b..]) else { break };
            let body = &rest[b + 1..b + e];
            rest = &rest[b + e + 1..];
            let decls = std::rc::Rc::new(parse_declarations(body));
            if decls.is_empty() {
                continue;
            }
            for sel in split_top(prelude, ',') {
                if let Some(selector) = parse_selector(sel.trim()) {
                    let specificity = selector.specificity();
                    rules.push(Rule { selector, specificity, order: rules.len(), decls: decls.clone() });
                }
            }
        }
        Stylesheet { rules }
    }

    /// Add another sheet's rules after this one's (a later `<style>` block).
    pub fn extend(&mut self, other: Stylesheet) {
        let base = self.rules.len();
        for mut r in other.rules {
            r.order += base;
            self.rules.push(r);
        }
    }

    pub fn is_empty(&self) -> bool {
        self.rules.is_empty()
    }

    /// Every declaration that applies to the LAST element of `path`, then the
    /// element's own `inline` declarations — in cascade order, so applying
    /// them one after another leaves the winning value in place: normal
    /// before `!important`, and within each, lower specificity and earlier
    /// source order first, with the inline style above every sheet rule.
    pub fn cascade(&self, path: &[ElementRef<'_>], inline: &[Decl]) -> Vec<Decl> {
        // (important, inline, specificity, order)
        let mut found: Vec<((bool, bool, (u32, u32, u32), usize), &Decl)> = Vec::new();
        if !path.is_empty() {
            for r in &self.rules {
                if r.selector.matches(path) {
                    for d in r.decls.iter() {
                        found.push(((d.important, false, r.specificity, r.order), d));
                    }
                }
            }
        }
        for (i, d) in inline.iter().enumerate() {
            found.push(((d.important, true, (0, 0, 0), i), d));
        }
        found.sort_by(|a, b| a.0.cmp(&b.0));
        found.into_iter().map(|(_, d)| d.clone()).collect()
    }
}

/// Parse a declaration block (`color: red; margin: 0 auto !important`).
pub fn parse_declarations(body: &str) -> Vec<Decl> {
    let mut out = Vec::new();
    for part in split_top(body, ';') {
        let Some((name, value)) = part.split_once(':') else { continue };
        let name = name.trim().to_ascii_lowercase();
        if name.is_empty() || name.contains(char::is_whitespace) {
            continue;
        }
        let mut value = value.trim().to_string();
        let mut important = false;
        if let Some(at) = value.to_ascii_lowercase().rfind("!important") {
            value.truncate(at);
            value = value.trim().to_string();
            important = true;
        }
        if value.is_empty() {
            continue;
        }
        // A custom property keeps its case; a standard one's name is
        // case-insensitive.
        out.push(Decl { name, value, important });
    }
    out
}

fn strip_comments(css: &str) -> String {
    let mut out = String::with_capacity(css.len());
    let mut rest = css;
    while let Some(start) = rest.find("/*") {
        out.push_str(&rest[..start]);
        match rest[start + 2..].find("*/") {
            Some(end) => rest = &rest[start + 2 + end + 2..],
            None => return out,
        }
    }
    out.push_str(rest);
    out
}

/// The index of the `}` closing the block that starts at `s[0] == '{'`.
fn block_end(s: &str) -> Option<usize> {
    let mut depth = 0usize;
    let mut quote: Option<char> = None;
    for (i, c) in s.char_indices() {
        if let Some(q) = quote {
            if c == q {
                quote = None;
            }
            continue;
        }
        match c {
            '"' | '\'' => quote = Some(c),
            '{' => depth += 1,
            '}' => {
                depth = depth.saturating_sub(1);
                if depth == 0 {
                    return Some(i);
                }
            }
            _ => {}
        }
    }
    None
}

/// Split on `sep` outside parentheses and quotes.
pub fn split_top(s: &str, sep: char) -> Vec<&str> {
    let mut out = Vec::new();
    let mut depth = 0i32;
    let mut quote: Option<char> = None;
    let mut start = 0;
    for (i, c) in s.char_indices() {
        if let Some(q) = quote {
            if c == q {
                quote = None;
            }
            continue;
        }
        match c {
            '"' | '\'' => quote = Some(c),
            '(' => depth += 1,
            ')' => depth -= 1,
            _ if c == sep && depth <= 0 => {
                out.push(&s[start..i]);
                start = i + c.len_utf8();
            }
            _ => {}
        }
    }
    out.push(&s[start..]);
    out.into_iter().filter(|p| !p.trim().is_empty()).collect()
}

/// Split a value into its space-separated tokens, keeping `rgb(…)`-style
/// functions whole.
pub fn tokens(s: &str) -> Vec<&str> {
    split_top(s, ' ').into_iter().flat_map(|t| split_top(t, '\t')).map(str::trim).filter(|t| !t.is_empty()).collect()
}

fn parse_selector(s: &str) -> Option<Selector> {
    if s.is_empty() {
        return None;
    }
    let mut parts: Vec<(Combinator, Compound)> = Vec::new();
    let mut pending = Combinator::Descendant;
    let chars: Vec<char> = s.chars().collect();
    let mut i = 0;
    let mut current: Option<Compound> = None;
    let ident = |i: &mut usize| -> String {
        let start = *i;
        while *i < chars.len() && (chars[*i].is_alphanumeric() || chars[*i] == '-' || chars[*i] == '_') {
            *i += 1;
        }
        chars[start..*i].iter().collect()
    };
    while i < chars.len() {
        let c = chars[i];
        match c {
            ' ' | '\t' | '\n' | '\r' => {
                if let Some(cmp) = current.take() {
                    parts.push((pending, cmp));
                    pending = Combinator::Descendant;
                }
                i += 1;
            }
            '>' => {
                if let Some(cmp) = current.take() {
                    parts.push((pending, cmp));
                }
                if parts.is_empty() {
                    return None;
                }
                pending = Combinator::Child;
                i += 1;
            }
            // Sibling combinators and attribute selectors are not in the
            // subset: a selector using one is dropped rather than matched
            // wrongly.
            '+' | '~' | '[' => return None,
            '*' => {
                current.get_or_insert_with(Compound::default);
                i += 1;
            }
            '#' => {
                i += 1;
                let name = ident(&mut i);
                if name.is_empty() {
                    return None;
                }
                current.get_or_insert_with(Compound::default).id = Some(name);
            }
            '.' => {
                i += 1;
                let name = ident(&mut i);
                if name.is_empty() {
                    return None;
                }
                current.get_or_insert_with(Compound::default).classes.push(name);
            }
            ':' => {
                i += 1;
                // A pseudo-ELEMENT (`::before`) generates content this
                // subset does not; `:hover` and friends describe a state a
                // document never has. Either way the selector is dropped.
                if i < chars.len() && chars[i] == ':' {
                    return None;
                }
                let name = ident(&mut i).to_ascii_lowercase();
                let mut arg = String::new();
                if i < chars.len() && chars[i] == '(' {
                    let start = i + 1;
                    while i < chars.len() && chars[i] != ')' {
                        i += 1;
                    }
                    arg = chars[start..i.min(chars.len())].iter().collect();
                    i += 1;
                }
                let p = match name.as_str() {
                    "first-child" => Pseudo::FirstChild,
                    "last-child" => Pseudo::LastChild,
                    "only-child" => Pseudo::OnlyChild,
                    "root" => Pseudo::Root,
                    "nth-child" => {
                        let (a, b) = parse_nth(&arg)?;
                        Pseudo::Nth(a, b)
                    }
                    "nth-last-child" => {
                        let (a, b) = parse_nth(&arg)?;
                        Pseudo::NthLast(a, b)
                    }
                    _ => return None,
                };
                current.get_or_insert_with(Compound::default).pseudos.push(p);
            }
            _ if c.is_alphanumeric() || c == '-' || c == '_' => {
                let name = ident(&mut i).to_ascii_lowercase();
                let cmp = current.get_or_insert_with(Compound::default);
                if cmp.tag.is_some() || !cmp.classes.is_empty() || cmp.id.is_some() {
                    return None;
                }
                cmp.tag = Some(name);
            }
            _ => return None,
        }
    }
    if let Some(cmp) = current.take() {
        parts.push((pending, cmp));
    } else if pending == Combinator::Child {
        return None;
    }
    (!parts.is_empty()).then_some(Selector { parts })
}

/// `odd`, `even`, `3`, `2n+1`, `-n+3`, `n`.
fn parse_nth(arg: &str) -> Option<(i32, i32)> {
    let a = arg.trim().to_ascii_lowercase().replace(' ', "");
    match a.as_str() {
        "odd" => return Some((2, 1)),
        "even" => return Some((2, 0)),
        _ => {}
    }
    if let Some(npos) = a.find('n') {
        let coef = &a[..npos];
        let a_val = match coef {
            "" | "+" => 1,
            "-" => -1,
            c => c.parse().ok()?,
        };
        let rest = &a[npos + 1..];
        let b_val = if rest.is_empty() { 0 } else { rest.trim_start_matches('+').parse().ok()? };
        Some((a_val, b_val))
    } else {
        Some((0, a.parse().ok()?))
    }
}

fn nth_matches(a: i32, b: i32, pos: i32) -> bool {
    if a == 0 {
        return pos == b;
    }
    let diff = pos - b;
    diff % a == 0 && diff / a >= 0
}

impl Compound {
    fn matches(&self, e: &ElementRef<'_>) -> bool {
        if let Some(t) = &self.tag {
            if t != e.name {
                return false;
            }
        }
        if let Some(id) = &self.id {
            if e.id != Some(id.as_str()) {
                return false;
            }
        }
        if !self.classes.iter().all(|c| e.classes.iter().any(|ec| ec == c)) {
            return false;
        }
        self.pseudos.iter().all(|p| match *p {
            Pseudo::FirstChild => e.index == 1,
            Pseudo::LastChild => e.index == e.count,
            Pseudo::OnlyChild => e.count == 1,
            Pseudo::Root => e.name == "html",
            Pseudo::Nth(a, b) => nth_matches(a, b, e.index as i32),
            Pseudo::NthLast(a, b) => nth_matches(a, b, (e.count + 1 - e.index) as i32),
        })
    }
}

impl Selector {
    fn specificity(&self) -> (u32, u32, u32) {
        let mut s = (0, 0, 0);
        for (_, c) in &self.parts {
            s.0 += c.id.is_some() as u32;
            s.1 += (c.classes.len() + c.pseudos.len()) as u32;
            s.2 += c.tag.is_some() as u32;
        }
        s
    }

    /// Right to left, backtracking over descendant combinators.
    fn matches(&self, path: &[ElementRef<'_>]) -> bool {
        fn at(parts: &[(Combinator, Compound)], path: &[ElementRef<'_>]) -> bool {
            let Some(((comb, last), rest_parts)) = parts.split_last() else { return true };
            let Some((elem, ancestors)) = path.split_last() else { return false };
            if !last.matches(elem) {
                return false;
            }
            if rest_parts.is_empty() {
                return true;
            }
            match comb {
                Combinator::Child => at(rest_parts, ancestors),
                Combinator::Descendant => (0..ancestors.len()).rev().any(|n| at(rest_parts, &ancestors[..=n])),
            }
        }
        at(&self.parts, path)
    }
}

// ── Values ────────────────────────────────────────────────────────────────

/// Replace every `var(--name[, fallback])` in `value` from `vars`. An unknown
/// variable with no fallback yields `None` — the declaration is then invalid,
/// exactly as CSS says ("invalid at computed-value time").
pub fn substitute_vars(value: &str, vars: &HashMap<String, String>) -> Option<String> {
    let mut out = value.to_string();
    for _ in 0..16 {
        let Some(start) = out.find("var(") else { return Some(out) };
        let open = start + 3;
        let mut depth = 0;
        let mut end = None;
        for (i, c) in out[open..].char_indices() {
            match c {
                '(' => depth += 1,
                ')' => {
                    depth -= 1;
                    if depth == 0 {
                        end = Some(open + i);
                        break;
                    }
                }
                _ => {}
            }
        }
        let end = end?;
        let inner = &out[open + 1..end];
        let (name, fallback) = match inner.split_once(',') {
            Some((n, f)) => (n.trim(), Some(f.trim())),
            None => (inner.trim(), None),
        };
        let replacement = vars.get(name).map(String::as_str).or(fallback)?.to_string();
        out.replace_range(start..=end, &replacement);
    }
    Some(out)
}

/// A length in CSS pixels. `em` is relative to `font_px`, `rem` to 16 px (the
/// root size the Viewer maps its own base font onto), `%` to `percent_of`
/// when there is one. A unitless `0` is a length; any other bare number is not.
pub fn length(value: &str, font_px: f32, percent_of: Option<f32>) -> Option<f32> {
    let v = value.trim().to_ascii_lowercase();
    let num = |s: &str| s.trim().parse::<f32>().ok();
    if let Some(n) = v.strip_suffix("px") {
        return num(n);
    }
    if let Some(n) = v.strip_suffix("rem") {
        return num(n).map(|n| n * 16.0);
    }
    if let Some(n) = v.strip_suffix("em") {
        return num(n).map(|n| n * font_px);
    }
    if let Some(n) = v.strip_suffix("pt") {
        return num(n).map(|n| n * 4.0 / 3.0);
    }
    if let Some(n) = v.strip_suffix('%') {
        return num(n).and_then(|n| percent_of.map(|p| p * n / 100.0));
    }
    if v == "0" {
        return Some(0.0);
    }
    None
}

/// A CSS colour as RGBA: `#rgb`, `#rgba`, `#rrggbb`, `#rrggbbaa`, `rgb()` /
/// `rgba()` (commas or spaces, numbers or percentages, `/ alpha`), `hsl()` /
/// `hsla()`, `transparent`, and the full list of CSS named colours.
pub fn color(value: &str) -> Option<[u8; 4]> {
    let v = value.trim().to_ascii_lowercase();
    if let Some(hex) = v.strip_prefix('#') {
        let d = |s: &str| u8::from_str_radix(s, 16).ok();
        let dd = |c: char| u8::from_str_radix(&format!("{c}{c}"), 16).ok();
        let cs: Vec<char> = hex.chars().collect();
        return match cs.len() {
            3 => Some([dd(cs[0])?, dd(cs[1])?, dd(cs[2])?, 255]),
            4 => Some([dd(cs[0])?, dd(cs[1])?, dd(cs[2])?, dd(cs[3])?]),
            6 => Some([d(&hex[0..2])?, d(&hex[2..4])?, d(&hex[4..6])?, 255]),
            8 => Some([d(&hex[0..2])?, d(&hex[2..4])?, d(&hex[4..6])?, d(&hex[6..8])?]),
            _ => None,
        };
    }
    if v == "transparent" {
        return Some([0, 0, 0, 0]);
    }
    let func = |prefix: &str| -> Option<Vec<String>> {
        let inner = v.strip_prefix(prefix)?.trim_start().strip_prefix('(')?.strip_suffix(')')?;
        Some(inner.replace(['/', ','], " ").split_whitespace().map(str::to_string).collect())
    };
    let channel = |s: &str| -> Option<f32> {
        match s.strip_suffix('%') {
            Some(p) => p.parse::<f32>().ok().map(|p| p * 2.55),
            None => s.parse::<f32>().ok(),
        }
    };
    let alpha = |s: Option<&String>| -> Option<u8> {
        match s {
            None => Some(255),
            Some(a) => {
                let f = match a.strip_suffix('%') {
                    Some(p) => p.parse::<f32>().ok()? / 100.0,
                    None => a.parse::<f32>().ok()?,
                };
                Some((f.clamp(0.0, 1.0) * 255.0).round() as u8)
            }
        }
    };
    let clamp = |f: f32| f.round().clamp(0.0, 255.0) as u8;
    if let Some(p) = func("rgba").or_else(|| func("rgb")) {
        if p.len() < 3 {
            return None;
        }
        return Some([clamp(channel(&p[0])?), clamp(channel(&p[1])?), clamp(channel(&p[2])?), alpha(p.get(3))?]);
    }
    if let Some(p) = func("hsla").or_else(|| func("hsl")) {
        if p.len() < 3 {
            return None;
        }
        let h = p[0].trim_end_matches("deg").parse::<f32>().ok()?.rem_euclid(360.0) / 360.0;
        let s = p[1].trim_end_matches('%').parse::<f32>().ok()? / 100.0;
        let l = p[2].trim_end_matches('%').parse::<f32>().ok()? / 100.0;
        let q = if l < 0.5 { l * (1.0 + s) } else { l + s - l * s };
        let pp = 2.0 * l - q;
        let hue = |mut t: f32| {
            if t < 0.0 {
                t += 1.0;
            }
            if t > 1.0 {
                t -= 1.0;
            }
            if t < 1.0 / 6.0 {
                pp + (q - pp) * 6.0 * t
            } else if t < 0.5 {
                q
            } else if t < 2.0 / 3.0 {
                pp + (q - pp) * (2.0 / 3.0 - t) * 6.0
            } else {
                pp
            }
        };
        let (r, g, b) = if s == 0.0 { (l, l, l) } else { (hue(h + 1.0 / 3.0), hue(h), hue(h - 1.0 / 3.0)) };
        return Some([clamp(r * 255.0), clamp(g * 255.0), clamp(b * 255.0), alpha(p.get(3))?]);
    }
    NAMED.iter().find(|(n, _)| *n == v).map(|(_, c)| [c[0], c[1], c[2], 255])
}

/// A colour as the `#rrggbbaa` string the layout model carries.
pub fn color_string(value: &str) -> Option<String> {
    color(value).map(|[r, g, b, a]| format!("#{r:02x}{g:02x}{b:02x}{a:02x}"))
}

/// The CSS named colours (CSS Color Module Level 4).
const NAMED: &[(&str, [u8; 3])] = &[
    ("aliceblue", [240, 248, 255]), ("antiquewhite", [250, 235, 215]), ("aqua", [0, 255, 255]),
    ("aquamarine", [127, 255, 212]), ("azure", [240, 255, 255]), ("beige", [245, 245, 220]),
    ("bisque", [255, 228, 196]), ("black", [0, 0, 0]), ("blanchedalmond", [255, 235, 205]),
    ("blue", [0, 0, 255]), ("blueviolet", [138, 43, 226]), ("brown", [165, 42, 42]),
    ("burlywood", [222, 184, 135]), ("cadetblue", [95, 158, 160]), ("chartreuse", [127, 255, 0]),
    ("chocolate", [210, 105, 30]), ("coral", [255, 127, 80]), ("cornflowerblue", [100, 149, 237]),
    ("cornsilk", [255, 248, 220]), ("crimson", [220, 20, 60]), ("cyan", [0, 255, 255]),
    ("darkblue", [0, 0, 139]), ("darkcyan", [0, 139, 139]), ("darkgoldenrod", [184, 134, 11]),
    ("darkgray", [169, 169, 169]), ("darkgreen", [0, 100, 0]), ("darkgrey", [169, 169, 169]),
    ("darkkhaki", [189, 183, 107]), ("darkmagenta", [139, 0, 139]), ("darkolivegreen", [85, 107, 47]),
    ("darkorange", [255, 140, 0]), ("darkorchid", [153, 50, 204]), ("darkred", [139, 0, 0]),
    ("darksalmon", [233, 150, 122]), ("darkseagreen", [143, 188, 143]), ("darkslateblue", [72, 61, 139]),
    ("darkslategray", [47, 79, 79]), ("darkslategrey", [47, 79, 79]), ("darkturquoise", [0, 206, 209]),
    ("darkviolet", [148, 0, 211]), ("deeppink", [255, 20, 147]), ("deepskyblue", [0, 191, 255]),
    ("dimgray", [105, 105, 105]), ("dimgrey", [105, 105, 105]), ("dodgerblue", [30, 144, 255]),
    ("firebrick", [178, 34, 34]), ("floralwhite", [255, 250, 240]), ("forestgreen", [34, 139, 34]),
    ("fuchsia", [255, 0, 255]), ("gainsboro", [220, 220, 220]), ("ghostwhite", [248, 248, 255]),
    ("gold", [255, 215, 0]), ("goldenrod", [218, 165, 32]), ("gray", [128, 128, 128]),
    ("green", [0, 128, 0]), ("greenyellow", [173, 255, 47]), ("grey", [128, 128, 128]),
    ("honeydew", [240, 255, 240]), ("hotpink", [255, 105, 180]), ("indianred", [205, 92, 92]),
    ("indigo", [75, 0, 130]), ("ivory", [255, 255, 240]), ("khaki", [240, 230, 140]),
    ("lavender", [230, 230, 250]), ("lavenderblush", [255, 240, 245]), ("lawngreen", [124, 252, 0]),
    ("lemonchiffon", [255, 250, 205]), ("lightblue", [173, 216, 230]), ("lightcoral", [240, 128, 128]),
    ("lightcyan", [224, 255, 255]), ("lightgoldenrodyellow", [250, 250, 210]), ("lightgray", [211, 211, 211]),
    ("lightgreen", [144, 238, 144]), ("lightgrey", [211, 211, 211]), ("lightpink", [255, 182, 193]),
    ("lightsalmon", [255, 160, 122]), ("lightseagreen", [32, 178, 170]), ("lightskyblue", [135, 206, 250]),
    ("lightslategray", [119, 136, 153]), ("lightslategrey", [119, 136, 153]), ("lightsteelblue", [176, 196, 222]),
    ("lightyellow", [255, 255, 224]), ("lime", [0, 255, 0]), ("limegreen", [50, 205, 50]),
    ("linen", [250, 240, 230]), ("magenta", [255, 0, 255]), ("maroon", [128, 0, 0]),
    ("mediumaquamarine", [102, 205, 170]), ("mediumblue", [0, 0, 205]), ("mediumorchid", [186, 85, 211]),
    ("mediumpurple", [147, 112, 219]), ("mediumseagreen", [60, 179, 113]), ("mediumslateblue", [123, 104, 238]),
    ("mediumspringgreen", [0, 250, 154]), ("mediumturquoise", [72, 209, 204]), ("mediumvioletred", [199, 21, 133]),
    ("midnightblue", [25, 25, 112]), ("mintcream", [245, 255, 250]), ("mistyrose", [255, 228, 225]),
    ("moccasin", [255, 228, 181]), ("navajowhite", [255, 222, 173]), ("navy", [0, 0, 128]),
    ("oldlace", [253, 245, 230]), ("olive", [128, 128, 0]), ("olivedrab", [107, 142, 35]),
    ("orange", [255, 165, 0]), ("orangered", [255, 69, 0]), ("orchid", [218, 112, 214]),
    ("palegoldenrod", [238, 232, 170]), ("palegreen", [152, 251, 152]), ("paleturquoise", [175, 238, 238]),
    ("palevioletred", [219, 112, 147]), ("papayawhip", [255, 239, 213]), ("peachpuff", [255, 218, 185]),
    ("peru", [205, 133, 63]), ("pink", [255, 192, 203]), ("plum", [221, 160, 221]),
    ("powderblue", [176, 224, 230]), ("purple", [128, 0, 128]), ("rebeccapurple", [102, 51, 153]),
    ("red", [255, 0, 0]), ("rosybrown", [188, 143, 143]), ("royalblue", [65, 105, 225]),
    ("saddlebrown", [139, 69, 19]), ("salmon", [250, 128, 114]), ("sandybrown", [244, 164, 96]),
    ("seagreen", [46, 139, 87]), ("seashell", [255, 245, 238]), ("sienna", [160, 82, 45]),
    ("silver", [192, 192, 192]), ("skyblue", [135, 206, 235]), ("slateblue", [106, 90, 205]),
    ("slategray", [112, 128, 144]), ("slategrey", [112, 128, 144]), ("snow", [255, 250, 250]),
    ("springgreen", [0, 255, 127]), ("steelblue", [70, 130, 180]), ("tan", [210, 180, 140]),
    ("teal", [0, 128, 128]), ("thistle", [216, 191, 216]), ("tomato", [255, 99, 71]),
    ("turquoise", [64, 224, 208]), ("violet", [238, 130, 238]), ("wheat", [245, 222, 179]),
    ("white", [255, 255, 255]), ("whitesmoke", [245, 245, 245]), ("yellow", [255, 255, 0]),
    ("yellowgreen", [154, 205, 50]),
];

#[cfg(test)]
mod tests {
    use super::*;

    fn el<'a>(name: &'a str, id: Option<&'a str>, classes: &'a [String], index: usize, count: usize) -> ElementRef<'a> {
        ElementRef { name, id, classes, index, count }
    }

    #[test]
    fn the_cascade_orders_by_importance_specificity_and_source() {
        let sheet = Stylesheet::parse(
            "/* a comment */ p { color: red } .note { color: blue } #x { color: green }
             p { color: black } @media (max-width: 600px) { p { color: pink } }
             div > p.note { margin: 0 !important } p { margin: 4px }",
        );
        let none: Vec<String> = Vec::new();
        let note = vec!["note".to_string()];
        let path = [el("body", None, &none, 1, 1), el("div", None, &none, 1, 1), el("p", Some("x"), &note, 1, 1)];
        let got: Vec<(String, String)> =
            sheet.cascade(&path, &[]).into_iter().map(|d| (d.name, d.value)).collect();
        // Applied in order, the last colour wins: the id rule, then margin 0 (important).
        let last = |name: &str| got.iter().rev().find(|(n, _)| n == name).map(|(_, v)| v.clone());
        assert_eq!(last("color").as_deref(), Some("green"), "{got:?}");
        assert_eq!(last("margin").as_deref(), Some("0"), "!important beats a later rule: {got:?}");
        assert!(!got.iter().any(|(_, v)| v == "pink"), "an @media block is skipped");
        // The inline style outranks every sheet rule but an important one.
        let inline = parse_declarations("color: orange; margin: 9px");
        let got = sheet.cascade(&path, &inline);
        let colour = got.iter().rev().find(|d| d.name == "color").unwrap();
        let margin = got.iter().rev().find(|d| d.name == "margin").unwrap();
        assert_eq!(colour.value, "orange");
        assert_eq!(margin.value, "0");
    }

    #[test]
    fn selectors_match_structure_and_position() {
        let sheet = Stylesheet::parse(
            "tr:nth-child(even) td { background: #eee } table > tr > td:first-child { color: red }
             td:last-child { color: blue } a:hover { color: pink } p::before { content: 'x' }
             ul li:nth-child(2n+1) { color: gray } input[type=text] { color: teal }",
        );
        let none: Vec<String> = Vec::new();
        let decl = |path: &[ElementRef<'_>], name: &str| {
            sheet.cascade(path, &[]).into_iter().rev().find(|d| d.name == name).map(|d| d.value)
        };
        let row2 = [el("table", None, &none, 1, 1), el("tr", None, &none, 2, 3), el("td", None, &none, 1, 2)];
        assert_eq!(decl(&row2, "background").as_deref(), Some("#eee"), "an even row");
        assert_eq!(decl(&row2, "color").as_deref(), Some("red"), "first cell, child combinators");
        let row3 = [el("table", None, &none, 1, 1), el("tr", None, &none, 3, 3), el("td", None, &none, 2, 2)];
        assert_eq!(decl(&row3, "background"), None, "an odd row");
        assert_eq!(decl(&row3, "color").as_deref(), Some("blue"), "the last cell");
        let li = [el("ul", None, &none, 1, 1), el("li", None, &none, 3, 4)];
        assert_eq!(decl(&li, "color").as_deref(), Some("gray"));
        let a = [el("a", None, &none, 1, 1)];
        assert_eq!(decl(&a, "color"), None, ":hover never matches a document");
        assert_eq!(sheet.rules.len(), 4, "::before and [attr] are dropped, not guessed at");
    }

    #[test]
    fn values_colours_lengths_and_variables() {
        assert_eq!(color("#667eea"), Some([0x66, 0x7e, 0xea, 255]));
        assert_eq!(color("#fff8"), Some([255, 255, 255, 0x88]));
        assert_eq!(color("rgba(0, 0, 0, 0.5)"), Some([0, 0, 0, 128]));
        assert_eq!(color("rgb(255 0 0 / 50%)"), Some([255, 0, 0, 128]));
        assert_eq!(color("hsl(120, 100%, 25%)"), Some([0, 128, 0, 255]));
        assert_eq!(color("RebeccaPurple"), Some([102, 51, 153, 255]));
        assert_eq!(color("transparent"), Some([0, 0, 0, 0]));
        assert_eq!(color("nonsense"), None);
        assert_eq!(length("12px", 16.0, None), Some(12.0));
        assert_eq!(length("1.5em", 20.0, None), Some(30.0));
        assert_eq!(length("2rem", 20.0, None), Some(32.0));
        assert_eq!(length("50%", 16.0, Some(800.0)), Some(400.0));
        assert_eq!(length("0", 16.0, None), Some(0.0));
        assert_eq!(length("auto", 16.0, None), None);
        let mut vars = HashMap::new();
        vars.insert("--primary".to_string(), "#667eea".to_string());
        assert_eq!(substitute_vars("1px solid var(--primary)", &vars).as_deref(), Some("1px solid #667eea"));
        assert_eq!(substitute_vars("var(--missing, red)", &vars).as_deref(), Some("red"));
        assert_eq!(substitute_vars("var(--missing)", &vars), None);
        assert_eq!(parse_nth("-n+3"), Some((-1, 3)));
        assert!(nth_matches(-1, 3, 2) && !nth_matches(-1, 3, 4));
    }
}
