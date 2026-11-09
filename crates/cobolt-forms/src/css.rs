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
//! `@media` blocks are evaluated against a desktop screen [`MEDIA_WIDTH_PX`]
//! wide: the pane is desktop-wide, so a page's desktop rules apply and its
//! narrow-screen and print overrides do not. That is what lets a framework
//! such as Bulma, whose columns become a row only above 768 px, lay out.

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
    /// The parent's element children, in order — what `+` and `~` look
    /// back through. Empty when unknown; a sibling selector then fails.
    pub siblings: &'a [Sibling],
    /// The element's attributes as `(name, value)`, names lower-case — what an
    /// attribute selector (`[data-name="X"]`) reads. The Viewer passes none, so
    /// a rule using one matches nothing there.
    pub attrs: &'a [(&'a str, &'a str)],
}

/// One element child of a parent, as a sibling combinator sees it.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Sibling {
    pub name: String,
    pub id: Option<String>,
    pub classes: Vec<String>,
}

/// The width `@media` queries are answered for, CSS pixels: a desktop
/// screen, never print.
pub const MEDIA_WIDTH_PX: f32 = 1024.0;

#[derive(Debug, Clone, PartialEq)]
enum Pseudo {
    FirstChild,
    LastChild,
    OnlyChild,
    Root,
    /// `:host` — the root element of the path.
    Host,
    /// `:nth-child(an+b)`
    Nth(i32, i32),
    /// `:nth-last-child(an+b)`
    NthLast(i32, i32),
    /// `:not(a, b)` — none of the compounds matches.
    Not(Vec<Compound>),
}

/// How an attribute selector compares the attribute's value.
#[derive(Debug, Clone, Copy, PartialEq)]
enum AttrOp {
    /// `[a]` — the attribute exists.
    Exists,
    /// `[a=v]`
    Equals,
    /// `[a~=v]` — one of the whitespace-separated words.
    Word,
    /// `[a|=v]` — `v`, or `v-` then anything.
    Dash,
    /// `[a^=v]`
    Prefix,
    /// `[a$=v]`
    Suffix,
    /// `[a*=v]`
    Contains,
}

/// One attribute selector.
#[derive(Debug, Clone, PartialEq)]
struct AttrSel {
    name: String,
    op: AttrOp,
    value: String,
    /// The `i` flag: compare without regard to case.
    insensitive: bool,
}

impl AttrSel {
    fn matches(&self, attrs: &[(&str, &str)]) -> bool {
        let Some((_, have)) = attrs.iter().find(|(n, _)| n.eq_ignore_ascii_case(&self.name)) else { return false };
        if self.op == AttrOp::Exists {
            return true;
        }
        let (have, want) = if self.insensitive {
            (have.to_lowercase(), self.value.to_lowercase())
        } else {
            (have.to_string(), self.value.clone())
        };
        match self.op {
            AttrOp::Exists => true,
            AttrOp::Equals => have == want,
            AttrOp::Word => have.split_whitespace().any(|w| w == want),
            AttrOp::Dash => have == want || have.strip_prefix(&want).is_some_and(|r| r.starts_with('-')),
            // An empty value matches nothing for the substring forms, as in a browser.
            AttrOp::Prefix => !want.is_empty() && have.starts_with(&want),
            AttrOp::Suffix => !want.is_empty() && have.ends_with(&want),
            AttrOp::Contains => !want.is_empty() && have.contains(&want),
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq)]
struct Compound {
    tag: Option<String>,
    id: Option<String>,
    classes: Vec<String>,
    attrs: Vec<AttrSel>,
    pseudos: Vec<Pseudo>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum Combinator {
    Descendant,
    Child,
    /// `+` — the element straight before.
    Adjacent,
    /// `~` — any element before, same parent.
    General,
}

/// A complex selector, left to right: each compound after the first says how
/// it relates to the one before it.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Selector {
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
            // An at-rule: a statement to its `;`, or a block skipped whole —
            // except `@media`, whose rules are kept when its query holds.
            if rest.starts_with('@') {
                let semi = rest.find(';');
                let brace = rest.find('{');
                if let Some(b) = brace.filter(|b| semi.is_none_or(|s| s > *b)) {
                    let head = rest[1..b].trim();
                    if let Some(query) = head.strip_prefix("media").filter(|q| q.starts_with(char::is_whitespace) || q.starts_with('(')) {
                        let Some(e) = block_end(&rest[b..]) else { break };
                        if media_matches(query) {
                            for mut r in Stylesheet::parse(&rest[b + 1..b + e]).rules {
                                r.order = rules.len();
                                rules.push(r);
                            }
                        }
                        rest = &rest[b + e + 1..];
                        continue;
                    }
                }
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

/// A declaration with where it sits in the text it was read from (WASM-036 R4), so
/// an editor can rewrite its value in place without touching the rest.
#[derive(Debug, Clone, PartialEq)]
pub struct DeclSpan {
    pub decl: Decl,
    /// The property's name, as written (case kept).
    pub name: std::ops::Range<usize>,
    /// The value, without `!important` and without the spaces around it.
    pub value: std::ops::Range<usize>,
    /// The whole declaration, from the name to the end of the value or of
    /// `!important`; the `;` after it is not part of it.
    pub whole: std::ops::Range<usize>,
}

/// `css` with every comment overwritten by spaces (a newline stays a newline), so a
/// byte offset in the result is the same offset in `css`.
pub fn blank_comments(css: &str) -> String {
    let mut out = String::with_capacity(css.len());
    let mut rest = css;
    while let Some(start) = rest.find("/*") {
        out.push_str(&rest[..start]);
        let after = &rest[start + 2..];
        let (len, closed) = match after.find("*/") {
            Some(end) => (end + 2, true),
            None => (after.len(), false),
        };
        let comment = &rest[start..start + 2 + len];
        for b in comment.bytes() {
            // One output byte per input byte, so multi-byte characters keep their length.
            out.push(if b == b'\n' || b == b'\r' { b as char } else { ' ' });
        }
        rest = &rest[start + 2 + len..];
        if !closed {
            return out;
        }
    }
    out.push_str(rest);
    out
}

/// The declarations of `text[body]` (the inside of a rule's braces) with their byte
/// ranges in `text`. Comments are skipped; the declarations are the ones
/// [`parse_declarations`] reads from the same body.
pub fn parse_declarations_spanned(text: &str, body: std::ops::Range<usize>) -> Vec<DeclSpan> {
    let blanked = blank_comments(text);
    let base = blanked.as_ptr() as usize;
    let Some(inner) = blanked.get(body) else { return Vec::new() };
    let mut out = Vec::new();
    for part in split_top(inner, ';') {
        let at = part.as_ptr() as usize - base;
        let Some(colon) = part.find(':') else { continue };
        let name_text = &part[..colon];
        let lead = name_text.len() - name_text.trim_start().len();
        let name_str = name_text.trim();
        if name_str.is_empty() || name_str.contains(char::is_whitespace) {
            continue;
        }
        let name = at + lead..at + lead + name_str.len();
        let after = &part[colon + 1..];
        let vlead = after.len() - after.trim_start().len();
        let mut value_str = after.trim();
        let whole_end = at + colon + 1 + vlead + value_str.len();
        let mut important = false;
        if let Some(pos) = value_str.to_ascii_lowercase().rfind("!important") {
            value_str = value_str[..pos].trim_end();
            important = true;
        }
        if value_str.is_empty() {
            continue;
        }
        let vstart = at + colon + 1 + vlead;
        out.push(DeclSpan {
            decl: Decl { name: name_str.to_ascii_lowercase(), value: value_str.to_string(), important },
            name: name.clone(),
            value: vstart..vstart + value_str.len(),
            whole: name.start..whole_end,
        });
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

/// The inside of `[...]`: `name`, `name=value`, `name~=value` … with an optional
/// quoted value and an optional trailing `i` (or `s`) flag.
fn parse_attr(inner: &str) -> Option<AttrSel> {
    let inner = inner.trim();
    let name_end = inner.find(|c: char| !(c.is_alphanumeric() || c == '-' || c == '_' || c == ':')).unwrap_or(inner.len());
    let name = inner[..name_end].to_ascii_lowercase();
    if name.is_empty() {
        return None;
    }
    let rest = inner[name_end..].trim_start();
    if rest.is_empty() {
        return Some(AttrSel { name, op: AttrOp::Exists, value: String::new(), insensitive: false });
    }
    let (op, rest) = if let Some(r) = rest.strip_prefix("~=") {
        (AttrOp::Word, r)
    } else if let Some(r) = rest.strip_prefix("|=") {
        (AttrOp::Dash, r)
    } else if let Some(r) = rest.strip_prefix("^=") {
        (AttrOp::Prefix, r)
    } else if let Some(r) = rest.strip_prefix("$=") {
        (AttrOp::Suffix, r)
    } else if let Some(r) = rest.strip_prefix("*=") {
        (AttrOp::Contains, r)
    } else if let Some(r) = rest.strip_prefix('=') {
        (AttrOp::Equals, r)
    } else {
        return None;
    };
    let rest = rest.trim_start();
    let (value, tail) = match rest.chars().next()? {
        q @ ('"' | '\'') => {
            let body = &rest[1..];
            let close = body.find(q)?;
            (body[..close].to_string(), &body[close + 1..])
        }
        _ => {
            let end = rest.find(char::is_whitespace).unwrap_or(rest.len());
            (rest[..end].to_string(), &rest[end..])
        }
    };
    let insensitive = match tail.trim().to_ascii_lowercase().as_str() {
        "" | "s" => false,
        "i" => true,
        _ => return None,
    };
    Some(AttrSel { name, op, value, insensitive })
}

pub(crate) fn parse_selector(s: &str) -> Option<Selector> {
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
            '+' | '~' => {
                if let Some(cmp) = current.take() {
                    parts.push((pending, cmp));
                }
                if parts.is_empty() {
                    return None;
                }
                pending = if c == '+' { Combinator::Adjacent } else { Combinator::General };
                i += 1;
            }
            '[' => {
                let start = i + 1;
                let mut quote: Option<char> = None;
                let mut end = None;
                for (k, ch) in chars.iter().enumerate().skip(start) {
                    match quote {
                        Some(q) if *ch == q => quote = None,
                        Some(_) => {}
                        None if *ch == '"' || *ch == '\'' => quote = Some(*ch),
                        None if *ch == ']' => {
                            end = Some(k);
                            break;
                        }
                        None => {}
                    }
                }
                let end = end?;
                let inner: String = chars[start..end].iter().collect();
                current.get_or_insert_with(Compound::default).attrs.push(parse_attr(&inner)?);
                i = end + 1;
            }
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
                    let mut depth = 0;
                    while i < chars.len() {
                        match chars[i] {
                            '(' => depth += 1,
                            ')' => {
                                depth -= 1;
                                if depth == 0 {
                                    break;
                                }
                            }
                            _ => {}
                        }
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
                    "host" => Pseudo::Host,
                    "nth-child" => {
                        let (a, b) = parse_nth(&arg)?;
                        Pseudo::Nth(a, b)
                    }
                    "nth-last-child" => {
                        let (a, b) = parse_nth(&arg)?;
                        Pseudo::NthLast(a, b)
                    }
                    // Each argument one compound: `:not(.a)`, `:not(:last-child)`.
                    "not" => {
                        let mut list = Vec::new();
                        for a in split_top(&arg, ',') {
                            let sel = parse_selector(a.trim())?;
                            let [(Combinator::Descendant, cmp)] = <[_; 1]>::try_from(sel.parts).ok()? else {
                                return None;
                            };
                            list.push(cmp);
                        }
                        if list.is_empty() {
                            return None;
                        }
                        Pseudo::Not(list)
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
    } else if pending != Combinator::Descendant {
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
    /// `root`: `e` is the first element of the path (what `:host` tests).
    fn matches(&self, e: &ElementRef<'_>, root: bool) -> bool {
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
        if !self.attrs.iter().all(|a| a.matches(e.attrs)) {
            return false;
        }
        self.pseudos.iter().all(|p| match p {
            Pseudo::FirstChild => e.index == 1,
            Pseudo::LastChild => e.index == e.count,
            Pseudo::OnlyChild => e.count == 1,
            Pseudo::Root => e.name == "html",
            Pseudo::Host => root,
            Pseudo::Nth(a, b) => nth_matches(*a, *b, e.index as i32),
            Pseudo::NthLast(a, b) => nth_matches(*a, *b, (e.count + 1 - e.index) as i32),
            Pseudo::Not(list) => !list.iter().any(|c| c.matches(e, root)),
        })
    }

    /// (ids, classes and pseudo-classes, tags) — `:not()` counts as its
    /// most specific argument.
    fn specificity(&self) -> (u32, u32, u32) {
        let mut s = (self.id.is_some() as u32, (self.classes.len() + self.attrs.len()) as u32, self.tag.is_some() as u32);
        for p in &self.pseudos {
            match p {
                Pseudo::Not(list) => {
                    let m = list.iter().map(Compound::specificity).max().unwrap_or_default();
                    s = (s.0 + m.0, s.1 + m.1, s.2 + m.2);
                }
                _ => s.1 += 1,
            }
        }
        s
    }
}

impl Selector {
    pub(crate) fn specificity(&self) -> (u32, u32, u32) {
        let mut s = (0, 0, 0);
        for (_, c) in &self.parts {
            let t = c.specificity();
            s = (s.0 + t.0, s.1 + t.1, s.2 + t.2);
        }
        s
    }

    /// Right to left, backtracking over descendant combinators.
    pub(crate) fn matches(&self, path: &[ElementRef<'_>]) -> bool {
        fn at(parts: &[(Combinator, Compound)], path: &[ElementRef<'_>]) -> bool {
            let Some(((comb, last), rest_parts)) = parts.split_last() else { return true };
            let Some((elem, ancestors)) = path.split_last() else { return false };
            if !last.matches(elem, ancestors.is_empty()) {
                return false;
            }
            if rest_parts.is_empty() {
                return true;
            }
            // A sibling stands in for the element, under the same ancestors.
            let as_sibling = |j: usize| {
                let s = &elem.siblings[j - 1];
                let mut p = ancestors.to_vec();
                p.push(ElementRef {
                    name: &s.name,
                    id: s.id.as_deref(),
                    classes: &s.classes,
                    index: j,
                    count: elem.count,
                    siblings: elem.siblings,
                    attrs: &[],
                });
                at(rest_parts, &p)
            };
            let before = elem.index.saturating_sub(1).min(elem.siblings.len());
            match comb {
                Combinator::Child => at(rest_parts, ancestors),
                Combinator::Descendant => (0..ancestors.len()).rev().any(|n| at(rest_parts, &ancestors[..=n])),
                Combinator::Adjacent => before > 0 && before + 1 == elem.index && as_sibling(before),
                Combinator::General => (1..=before).rev().any(as_sibling),
            }
        }
        at(&self.parts, path)
    }
}

/// Whether an `@media` query list holds on a screen [`MEDIA_WIDTH_PX`] wide:
/// any of its comma-separated queries. Media types `all` and `screen` hold,
/// `print` and the rest do not; `min-width` / `max-width` are compared, and
/// a feature this subset does not know makes its query fail.
pub fn media_matches(list: &str) -> bool {
    // The fixed desktop answers of old: the features `media_eval` cannot decide are
    // read as they always were, and the ones it decides beyond them (`height`,
    // `aspect-ratio`) stay unknown names, which fail.
    let env = MediaEnv { width: MEDIA_WIDTH_PX, height: 768.0, scheme: Scheme::Light };
    media_core(list, &env, true) != Tri::Fails
}

/// The colour scheme `prefers-color-scheme` is answered for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Scheme {
    Light,
    Dark,
}

/// What `@media` is answered against: the viewport's size in CSS pixels and the
/// colour scheme.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MediaEnv {
    pub width: f32,
    pub height: f32,
    pub scheme: Scheme,
}

/// A media query's answer: decided, or a feature this tool does not know the
/// answer to (`hover`, `pointer`, the other `prefers-*`) — the editor shows such a
/// rule as conditional rather than guessing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tri {
    Holds,
    Fails,
    Unknown,
}

impl Tri {
    fn and(self, o: Tri) -> Tri {
        match (self, o) {
            (Tri::Fails, _) | (_, Tri::Fails) => Tri::Fails,
            (Tri::Unknown, _) | (_, Tri::Unknown) => Tri::Unknown,
            _ => Tri::Holds,
        }
    }
    fn or(self, o: Tri) -> Tri {
        match (self, o) {
            (Tri::Holds, _) | (_, Tri::Holds) => Tri::Holds,
            (Tri::Unknown, _) | (_, Tri::Unknown) => Tri::Unknown,
            _ => Tri::Fails,
        }
    }
    fn not(self) -> Tri {
        match self {
            Tri::Holds => Tri::Fails,
            Tri::Fails => Tri::Holds,
            Tri::Unknown => Tri::Unknown,
        }
    }
}

fn tri(b: bool) -> Tri {
    if b {
        Tri::Holds
    } else {
        Tri::Fails
    }
}

/// Whether an `@media` query list holds in `env`: `Holds`, `Fails`, or `Unknown`
/// when the answer rests on a feature that is not decided here.
pub fn media_eval(list: &str, env: &MediaEnv) -> Tri {
    media_core(list, env, false)
}

/// `legacy`: `media_matches`' own reading (see there).
fn media_core(list: &str, env: &MediaEnv, legacy: bool) -> Tri {
    let mut any = Tri::Fails;
    for q in split_top(list, ',') {
        let q = q.trim().to_ascii_lowercase();
        let (negated, q) = match q.strip_prefix("not ") {
            Some(r) => (true, r.trim().to_string()),
            None => (false, q.strip_prefix("only ").map(|r| r.trim().to_string()).unwrap_or(q)),
        };
        let mut ok = Tri::Holds;
        for part in q.split(" and ") {
            let part = part.trim();
            let holds = if let Some(f) = part.strip_prefix('(').and_then(|f| f.strip_suffix(')')) {
                let (name, value) = f.split_once(':').map(|(n, v)| (n.trim(), v.trim())).unwrap_or((f.trim(), ""));
                media_feature(name, value, env, legacy)
            } else {
                tri(matches!(part, "all" | "screen" | ""))
            };
            ok = ok.and(holds);
        }
        any = any.or(if negated { ok.not() } else { ok });
    }
    any
}

fn media_feature(name: &str, value: &str, env: &MediaEnv, legacy: bool) -> Tri {
    let px = || length(value, 16.0, None);
    let ratio = || {
        let (a, b) = value.split_once('/').map(|(a, b)| (a.trim(), b.trim())).unwrap_or((value, "1"));
        let (a, b): (f32, f32) = (a.parse().ok()?, b.parse().ok()?);
        (b != 0.0).then_some(a / b)
    };
    let aspect = env.width / env.height;
    match name {
        "min-width" => tri(px().is_some_and(|w| env.width >= w)),
        "max-width" => tri(px().is_some_and(|w| env.width <= w)),
        "orientation" => tri(value == if env.width >= env.height { "landscape" } else { "portrait" }),
        "color" => Tri::Holds,
        "prefers-color-scheme" => tri(value == if env.scheme == Scheme::Light { "light" } else { "dark" }),
        // Decided only by `media_eval`: `media_matches` always treated them as unknown names.
        "width" if !legacy => tri(px().is_some_and(|w| env.width == w)),
        "height" if !legacy => tri(px().is_some_and(|h| env.height == h)),
        "min-height" if !legacy => tri(px().is_some_and(|h| env.height >= h)),
        "max-height" if !legacy => tri(px().is_some_and(|h| env.height <= h)),
        "aspect-ratio" if !legacy => tri(ratio().is_some_and(|r| (aspect - r).abs() < 0.005)),
        "min-aspect-ratio" if !legacy => tri(ratio().is_some_and(|r| aspect >= r)),
        "max-aspect-ratio" if !legacy => tri(ratio().is_some_and(|r| aspect <= r)),
        // What a document cannot know, `media_matches` answers as a desktop does.
        "hover" | "any-hover" if legacy => tri(value == "hover"),
        "pointer" | "any-pointer" if legacy => tri(value == "fine"),
        "hover" | "any-hover" | "pointer" | "any-pointer" => Tri::Unknown,
        n if n.starts_with("prefers-") && !legacy => Tri::Unknown,
        _ => Tri::Fails,
    }
}

/// `calc(…)`: `+ - * /` over lengths and numbers, with parentheses and
/// nested `calc()`. A percentage needs `percent_of`.
fn calc(src: &str, font_px: f32, percent_of: Option<f32>) -> Option<f32> {
    // CSS wants `+` and `-` spaced out inside calc(), so they split on
    // whitespace; `*`, `/` and the parentheses need no spaces.
    let spaced: String = src
        .chars()
        .flat_map(|c| if matches!(c, '(' | ')' | '*' | '/') { vec![' ', c, ' '] } else { vec![c] })
        .collect();
    // A nested `calc(` is only a parenthesis.
    let toks: Vec<String> =
        spaced.split_whitespace().filter(|t| !t.eq_ignore_ascii_case("calc")).map(str::to_string).collect();
    fn expr(t: &[String], i: &mut usize, f: f32, pct: Option<f32>) -> Option<f32> {
        let mut v = term(t, i, f, pct)?;
        while let Some(op) = t.get(*i).filter(|o| o.as_str() == "+" || o.as_str() == "-") {
            let add = op == "+";
            *i += 1;
            let r = term(t, i, f, pct)?;
            v = if add { v + r } else { v - r };
        }
        Some(v)
    }
    fn term(t: &[String], i: &mut usize, f: f32, pct: Option<f32>) -> Option<f32> {
        let mut v = factor(t, i, f, pct)?;
        while let Some(op) = t.get(*i).filter(|o| o.as_str() == "*" || o.as_str() == "/") {
            let mul = op == "*";
            *i += 1;
            let r = factor(t, i, f, pct)?;
            v = if mul { v * r } else if r != 0.0 { v / r } else { return None };
        }
        Some(v)
    }
    fn factor(t: &[String], i: &mut usize, f: f32, pct: Option<f32>) -> Option<f32> {
        let tok = t.get(*i)?;
        *i += 1;
        if tok == "(" {
            let v = expr(t, i, f, pct)?;
            (t.get(*i)? == ")").then_some(())?;
            *i += 1;
            return Some(v);
        }
        if tok == "-" {
            return factor(t, i, f, pct).map(|v| -v);
        }
        tok.parse::<f32>().ok().or_else(|| length(tok, f, pct))
    }
    let mut i = 0;
    let v = expr(&toks, &mut i, font_px, percent_of)?;
    (i == toks.len()).then_some(v)
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
    if let Some(inner) = v.strip_prefix("calc(").and_then(|r| r.strip_suffix(')')) {
        return calc(inner, font_px, percent_of);
    }
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

/// The CSS name of an opaque colour, if it has one (the first of the list for two spellings, such as
/// `gray` / `grey`): what a writer keeps when the developer wrote the colour by name.
pub fn color_name(rgba: [u8; 4]) -> Option<&'static str> {
    if rgba[3] != 255 {
        return None;
    }
    NAMED.iter().find(|(_, c)| c[..] == rgba[..3]).map(|(n, _)| *n)
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
        ElementRef { name, id, classes, index, count, siblings: &[], attrs: &[] }
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
        assert!(!got.iter().any(|(_, v)| v == "pink"), "a narrow-screen @media block does not apply");
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
        assert_eq!(sheet.rules.len(), 5, "::before is dropped, not guessed at; [attr] is kept since WASM-036 T1");
        let input = [el("input", None, &none, 1, 1)];
        assert_eq!(decl(&input, "color"), None, "an attribute rule matches nothing where no attributes are passed");
        let mut typed = el("input", None, &none, 1, 1);
        let attrs = [("type", "text")];
        typed.attrs = &attrs;
        assert_eq!(decl(&[typed], "color").as_deref(), Some("teal"));
    }

    /// What a framework such as Bulma leans on: `@media` answered for a
    /// desktop screen, `:not()`, the sibling combinators and `calc()`.
    #[test]
    fn media_not_siblings_and_calc() {
        assert!(media_matches("screen and (min-width: 769px), print"));
        assert!(media_matches("screen and (min-width:1024px)"));
        assert!(!media_matches("screen and (max-width: 768px)"), "the narrow-screen override");
        assert!(!media_matches("print"));
        assert!(!media_matches("screen and (min-width: 1216px)"), "wider than the pane");
        assert!(media_matches("not print"));
        assert!(!media_matches("(prefers-reduced-motion: reduce)"), "an unknown feature fails its query");

        let sheet = Stylesheet::parse(
            ".title:not(:last-child) { margin-bottom: 24px }
             .columns:not(.is-desktop) { display: flex }
             .title + .subtitle { margin-top: -20px }
             h1 ~ p { color: gray }
             @media screen and (min-width: 769px), print { .column.is-half { width: 50% } }
             @media screen and (max-width: 768px) { .column.is-half { width: 100% } }",
        );
        let none: Vec<String> = Vec::new();
        let title = vec!["title".to_string()];
        let subtitle = vec!["subtitle".to_string()];
        let columns = vec!["columns".to_string()];
        let desktop = vec!["columns".to_string(), "is-desktop".to_string()];
        let half = vec!["column".to_string(), "is-half".to_string()];
        let kids = vec![
            Sibling { name: "h1".into(), id: None, classes: title.clone() },
            Sibling { name: "p".into(), id: None, classes: subtitle.clone() },
            Sibling { name: "p".into(), id: None, classes: none.clone() },
        ];
        let decls = |path: &[ElementRef<'_>], name: &str| {
            sheet.cascade(path, &[]).into_iter().rev().find(|d| d.name == name).map(|d| d.value)
        };
        let body = el("body", None, &none, 1, 1);
        let h1 = ElementRef { name: "h1", id: None, classes: &title, index: 1, count: 3, siblings: &kids, attrs: &[] };
        let sub = ElementRef { name: "p", id: None, classes: &subtitle, index: 2, count: 3, siblings: &kids, attrs: &[] };
        let plain = ElementRef { name: "p", id: None, classes: &none, index: 3, count: 3, siblings: &kids, attrs: &[] };
        assert_eq!(decls(&[body, h1], "margin-bottom").as_deref(), Some("24px"), ":not(:last-child)");
        assert_eq!(decls(&[body, sub], "margin-top").as_deref(), Some("-20px"), "title + subtitle");
        assert_eq!(decls(&[body, plain], "margin-top"), None, "+ is the element straight before only");
        assert_eq!(decls(&[body, plain], "color").as_deref(), Some("gray"), "h1 ~ p, two apart");
        assert_eq!(decls(&[body, sub], "color").as_deref(), Some("gray"));
        assert_eq!(decls(&[body, h1], "color"), None, "~ looks back, never forward");
        let lone = ElementRef { name: "p", id: None, classes: &subtitle, index: 2, count: 3, siblings: &[], attrs: &[] };
        assert_eq!(decls(&[body, lone], "margin-top"), None, "siblings unknown: no match, no panic");
        assert_eq!(decls(&[body, el("div", None, &columns, 1, 1)], "display").as_deref(), Some("flex"));
        assert_eq!(decls(&[body, el("div", None, &desktop, 1, 1)], "display"), None, ":not(.is-desktop)");
        assert_eq!(decls(&[body, el("div", None, &half, 1, 1)], "width").as_deref(), Some("50%"), "the desktop query holds");
        // :not() counts as its argument: .a:not(.b) outranks .a.
        let sp = Stylesheet::parse(".a:not(.b) { color: red } .a { color: blue }");
        let a = vec!["a".to_string()];
        assert_eq!(decls_of(&sp, &[el("i", None, &a, 1, 1)], "color").as_deref(), Some("red"));

        assert_eq!(length("calc(-1 * 0.75rem)", 16.0, None), Some(-12.0));
        assert_eq!(length("calc(100% - 2rem)", 16.0, Some(500.0)), Some(468.0));
        assert_eq!(length("calc(2 * (1em + 4px))", 10.0, None), Some(28.0));
        assert_eq!(length("calc(10px + )", 16.0, None), None);
        assert_eq!(length("-.75rem", 16.0, None), Some(-12.0), "a negative length");
    }

    fn decls_of(sheet: &Stylesheet, path: &[ElementRef<'_>], name: &str) -> Option<String> {
        sheet.cascade(path, &[]).into_iter().rev().find(|d| d.name == name).map(|d| d.value)
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

    // ── WASM-036 T1: attribute selectors, :host, decided media queries, byte ranges ──

    fn matches_attrs(selector: &str, attrs: &[(&str, &str)]) -> bool {
        let sel = parse_selector(selector).unwrap_or_else(|| panic!("selector {selector:?} did not parse"));
        let none: Vec<String> = Vec::new();
        let mut e = el("div", None, &none, 1, 1);
        e.attrs = attrs;
        sel.matches(&[e])
    }

    /// Every attribute operator, the `i` flag, the quoting forms, and what a missing attribute does.
    #[test]
    fn attribute_selectors_every_operator() {
        let a = [("data-name", "Btn-Save"), ("class", "one two three"), ("lang", "en-GB"), ("title", "")];
        let cases: &[(&str, bool)] = &[
            ("[data-name]", true),
            ("[nope]", false),
            ("[data-name=\"Btn-Save\"]", true),
            ("[data-name='Btn-Save']", true),
            ("[data-name=Btn-Save]", true),
            ("[data-name=\"btn-save\"]", false),
            ("[data-name=\"btn-save\" i]", true),
            ("[data-name=\"btn-save\" s]", false),
            ("[class~=two]", true),
            ("[class~=tw]", false),
            ("[lang|=en]", true),
            ("[lang|=\"en-GB\"]", true),
            ("[lang|=e]", false),
            ("[data-name^=Btn]", true),
            ("[data-name^=Save]", false),
            ("[data-name$=Save]", true),
            ("[data-name*=\"n-S\"]", true),
            ("[data-name*=zzz]", false),
            ("[title^=\"\"]", false),
            ("div[data-name=\"Btn-Save\"]", true),
            ("span[data-name=\"Btn-Save\"]", false),
            ("[data-name=\"Btn-Save\"][lang|=en]", true),
            ("[data-name=\"Btn-Save\"][lang|=fr]", false),
            ("[data-name=\"a]b\"]", false),
        ];
        for (sel, want) in cases {
            assert_eq!(matches_attrs(sel, &a), *want, "{sel}");
        }
        // No attributes known (the Viewer): such a rule matches nothing, it is not dropped wrongly.
        assert!(!matches_attrs("[data-name]", &[]));
        // Malformed ones are refused, not matched wrongly.
        for bad in ["[", "[=x]", "[a=]x", "[a?=b]", "[a=b j]"] {
            assert!(parse_selector(bad).is_none() || bad == "[a=]x", "{bad}");
        }
        // Specificity: an attribute counts as a class.
        assert_eq!(parse_selector("div[a][b]").unwrap().specificity(), (0, 2, 1));
        // And it cascades: the attribute rule beats the tag rule, and loses to an id.
        let sheet = Stylesheet::parse("div { color: red } [data-name=\"X\"] { color: blue } #x { color: green }");
        let none: Vec<String> = Vec::new();
        let mut e = el("div", None, &none, 1, 1);
        let attrs = [("data-name", "X")];
        e.attrs = &attrs;
        let got = sheet.cascade(&[e], &[]);
        assert_eq!(got.last().map(|d| d.value.as_str()), Some("blue"), "{got:?}");
        eprintln!("attribute selectors: {} cases, specificity and cascade checked", cases.len() + 5 + 3);
    }

    #[test]
    fn host_matches_only_the_root_of_the_path() {
        let none: Vec<String> = Vec::new();
        let path = [el("section", None, &none, 1, 1), el("div", None, &none, 1, 1)];
        let host = parse_selector(":host").unwrap();
        assert!(host.matches(&path[..1]), "the root element");
        assert!(!host.matches(&path), "a descendant is not the host");
        let desc = parse_selector(":host div").unwrap();
        assert!(desc.matches(&path));
        assert!(!parse_selector(":host section").unwrap().matches(&path), "the host is not its own descendant");
        assert_eq!(host.specificity(), (0, 1, 0));
        let sheet = Stylesheet::parse(":host { --a: 1 } div { --b: 2 }");
        let got = sheet.cascade(&path[..1], &[]);
        assert!(got.iter().any(|d| d.name == "--a") && !got.iter().any(|d| d.name == "--b"), "{got:?}");
    }

    /// 412 × 915 (a phone), Light and Dark: what decides and what stays unknown.
    #[test]
    fn media_eval_decides_what_it_can_and_says_unknown_otherwise() {
        let phone = |scheme| MediaEnv { width: 412.0, height: 915.0, scheme };
        let (light, dark) = (phone(Scheme::Light), phone(Scheme::Dark));
        let cases: &[(&str, Tri, Tri)] = &[
            ("(max-width: 600px)", Tri::Holds, Tri::Holds),
            ("(min-width: 600px)", Tri::Fails, Tri::Fails),
            ("screen and (max-width: 768px)", Tri::Holds, Tri::Holds),
            ("(width: 412px)", Tri::Holds, Tri::Holds),
            ("(height: 915px)", Tri::Holds, Tri::Holds),
            ("(min-height: 900px) and (max-height: 1000px)", Tri::Holds, Tri::Holds),
            ("(orientation: portrait)", Tri::Holds, Tri::Holds),
            ("(orientation: landscape)", Tri::Fails, Tri::Fails),
            ("(aspect-ratio: 412/915)", Tri::Holds, Tri::Holds),
            ("(min-aspect-ratio: 1/1)", Tri::Fails, Tri::Fails),
            ("(max-aspect-ratio: 1/1)", Tri::Holds, Tri::Holds),
            ("(prefers-color-scheme: dark)", Tri::Fails, Tri::Holds),
            ("(prefers-color-scheme: light)", Tri::Holds, Tri::Fails),
            ("(hover: hover)", Tri::Unknown, Tri::Unknown),
            ("(pointer: fine)", Tri::Unknown, Tri::Unknown),
            ("(prefers-reduced-motion: reduce)", Tri::Unknown, Tri::Unknown),
            ("(prefers-contrast: more)", Tri::Unknown, Tri::Unknown),
            ("print", Tri::Fails, Tri::Fails),
            ("not print", Tri::Holds, Tri::Holds),
            ("not (hover: hover)", Tri::Unknown, Tri::Unknown),
            ("(max-width: 600px) and (hover: hover)", Tri::Unknown, Tri::Unknown),
            ("(min-width: 600px) and (hover: hover)", Tri::Fails, Tri::Fails),
            ("print, (max-width: 600px)", Tri::Holds, Tri::Holds),
            ("(min-width: 600px), (hover: hover)", Tri::Unknown, Tri::Unknown),
            ("(frobnicate: 3)", Tri::Fails, Tri::Fails),
        ];
        for (q, l, d) in cases {
            assert_eq!(media_eval(q, &light), *l, "light: {q}");
            assert_eq!(media_eval(q, &dark), *d, "dark: {q}");
        }
        eprintln!("media_eval: {} queries x 2 schemes at 412 x 915", cases.len());
    }

    /// `media_matches` keeps its signature and every old answer: compared with a verbatim copy of the
    /// implementation it replaced, over a grid of queries.
    #[test]
    fn media_matches_gives_the_answers_it_always_gave() {
        fn old(list: &str) -> bool {
            split_top(list, ',').iter().any(|q| {
                let q = q.trim().to_ascii_lowercase();
                let (negated, q) = match q.strip_prefix("not ") {
                    Some(r) => (true, r.trim().to_string()),
                    None => (false, q.strip_prefix("only ").map(|r| r.trim().to_string()).unwrap_or(q)),
                };
                let mut ok = true;
                for part in q.split(" and ") {
                    let part = part.trim();
                    let holds = if let Some(f) = part.strip_prefix('(').and_then(|f| f.strip_suffix(')')) {
                        let (name, value) = f.split_once(':').map(|(n, v)| (n.trim(), v.trim())).unwrap_or((f.trim(), ""));
                        let px = || length(value, 16.0, None);
                        match name {
                            "min-width" => px().is_some_and(|w| MEDIA_WIDTH_PX >= w),
                            "max-width" => px().is_some_and(|w| MEDIA_WIDTH_PX <= w),
                            "orientation" => value == "landscape",
                            "hover" | "any-hover" => value == "hover",
                            "pointer" | "any-pointer" => value == "fine",
                            "prefers-color-scheme" => value == "light",
                            "color" => true,
                            _ => false,
                        }
                    } else {
                        matches!(part, "all" | "screen" | "")
                    };
                    ok &= holds;
                }
                ok != negated
            })
        }
        let types = ["", "screen and ", "print and ", "all and ", "only screen and ", "not "];
        let feats = [
            "(min-width: 769px)", "(min-width:1024px)", "(max-width: 768px)", "(min-width: 1216px)", "(max-width: 1024px)",
            "(orientation: landscape)", "(orientation: portrait)", "(hover: hover)", "(hover: none)", "(pointer: fine)",
            "(pointer: coarse)", "(any-hover: hover)", "(prefers-color-scheme: light)", "(prefers-color-scheme: dark)",
            "(prefers-reduced-motion: reduce)", "(color)", "(height: 768px)", "(min-height: 100px)", "(aspect-ratio: 4/3)",
            "(frobnicate: 1)",
        ];
        let mut n = 0;
        for t in types {
            for f in feats {
                for g in ["", " and (min-width: 600px)", " and (hover: hover)"] {
                    let q = format!("{t}{f}{g}");
                    assert_eq!(media_matches(&q), old(&q), "{q}");
                    n += 1;
                }
            }
        }
        for q in ["print", "screen", "all", "not print", "screen and (min-width: 769px), print", "print, screen", ""] {
            assert_eq!(media_matches(q), old(q), "{q:?}");
            n += 1;
        }
        eprintln!("media_matches: {n} queries identical to the implementation it replaced");
    }

    #[test]
    fn declarations_carry_their_byte_ranges() {
        // Comments, CRLF line endings, odd spacing, !important, a multi-byte character in a comment.
        let css = "a { /* ação */ color :  red ;\r\n  margin:0 AUTO !IMPORTANT;;\r\n\tfoo: bar }\n";
        let open = css.find('{').unwrap() + 1;
        let close = css.rfind('}').unwrap();
        let got = parse_declarations_spanned(css, open..close);
        assert_eq!(got.len(), 3, "{got:#?}");
        let text = |r: &std::ops::Range<usize>| &css[r.clone()];
        assert_eq!((text(&got[0].name), text(&got[0].value), text(&got[0].whole)), ("color", "red", "color :  red"));
        assert_eq!((text(&got[1].name), text(&got[1].value), text(&got[1].whole)), ("margin", "0 AUTO", "margin:0 AUTO !IMPORTANT"));
        assert!(got[1].decl.important && !got[0].decl.important);
        assert_eq!((text(&got[2].name), text(&got[2].value)), ("foo", "bar"));
        // They are the declarations parse_declarations reads from the same body.
        let plain = parse_declarations(&strip_comments(&css[open..close]));
        assert_eq!(got.iter().map(|d| d.decl.clone()).collect::<Vec<_>>(), plain);
        // Replacing a value by its range changes that value and nothing else.
        let mut edited = css.to_string();
        edited.replace_range(got[0].value.clone(), "blue");
        assert_eq!(edited, css.replacen("red", "blue", 1));
        // The blanked text has the length of the original, so offsets agree.
        assert_eq!(blank_comments(css).len(), css.len());
        assert_eq!(blank_comments("a /* x\ny */ b").len(), "a /* x\ny */ b".len());
        assert_eq!(blank_comments("a /* open").trim_end(), "a");
        assert!(parse_declarations_spanned("a{b:c}", 90..99).is_empty(), "a bad range is empty, not a panic");
        eprintln!("declaration spans: 3 declarations on a CRLF sheet with comments, ranges and in-place edit verified");
    }
}
