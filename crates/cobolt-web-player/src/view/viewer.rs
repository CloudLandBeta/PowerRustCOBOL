// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! **The Viewer** (WASM-004 class B for its pages, C for open and save, D for print and share;
//! WASM-013 R5).
//!
//! The desktop Viewer parses a document into a typed layout tree ([`Block`] and [`Inline`] of
//! `cobolt_forms::viewer`): Markdown, a subset of HTML, plain text, conversations. This module turns
//! *that tree* into nodes. The page never receives markup:
//!
//! * a **block kind** the tree can hold maps to one fixed element (a heading to `h1`…`h6`, a table to
//!   `table`), and nothing else becomes an element — raw HTML the source embedded is shown as the
//!   inert text it is;
//! * a **link** is an anchor only when [`crate::weblink::check`] allows its scheme; otherwise it is plain
//!   text. Every anchor is `rel="noopener noreferrer nofollow"` and opens in a new tab;
//! * an **image** is loaded only from the application's own files, a `data:` image or an allowed origin
//!   ([`ViewCtx::image_url`]); otherwise its alt text is shown;
//! * a **colour or length** a document's CSS gave an element reaches the CSSOM only if it is made of
//!   the characters of a colour or a length (no `url()`, no `expression()`).
//!
//! A test greps this crate's sources for the APIs that would insert markup, and this file is the one
//! it watches most closely.

use cobolt_forms::viewer::{mermaid_kind, Background, Block, BoxStyle, Conversation, Inline, MermaidKind, TableAlignment, TextAlign, TextStyle};

use super::controls::root;
use super::svg::svg_to_vnode;
use super::{MermaidState, VNode, ViewCtx};
use crate::value::{fmt_number, parse_f64, truthy};
use crate::weblink::{check, Policy};

/// A document the page has read for a Viewer: its text and how to read it.
#[derive(Debug, Clone, PartialEq)]
pub struct LoadedDoc {
    pub format: DocFormat,
    pub text: String,
    /// Why it could not be shown (a missing file, a format this player cannot draw).
    pub error: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DocFormat {
    Markdown,
    Html,
    Text,
    /// A PDF the PDF part read: the pages' text, separated by a form feed (`\u{c}`).
    Pdf,
}

impl LoadedDoc {
    /// Read a document's `text` by the format its file name suggests (`.md`, `.html`, anything else is text).
    pub fn from_text(path: &str, text: String) -> LoadedDoc {
        let lower = path.to_ascii_lowercase();
        let format = if lower.ends_with(".md") || lower.ends_with(".markdown") {
            DocFormat::Markdown
        } else if lower.ends_with(".html") || lower.ends_with(".htm") {
            DocFormat::Html
        } else {
            DocFormat::Text
        };
        LoadedDoc { format, text, error: None }
    }

    /// A PDF the PDF part answered, as its JSON (`{"pages":[{"width":..,"height":..,"text":".."}]}`): each page
    /// becomes a heading with its size and its text. The desktop Viewer also paints the page's lines and
    /// rectangles; this one shows the text, which is what a reader and a screen reader use.
    pub fn from_pdf_json(json: &str) -> LoadedDoc {
        let Ok(v) = serde_json::from_str::<serde_json::Value>(json) else { return LoadedDoc::failed("the PDF reader answered something unreadable") };
        let pages: Vec<String> = v["pages"]
            .as_array()
            .map(|a| a.iter().map(|p| format!("{} x {} pt\n{}", p["width"].as_f64().unwrap_or(0.0).round(), p["height"].as_f64().unwrap_or(0.0).round(), p["text"].as_str().unwrap_or(""))).collect())
            .unwrap_or_default();
        if pages.is_empty() {
            return LoadedDoc::failed("the PDF has no pages");
        }
        LoadedDoc { format: DocFormat::Pdf, text: pages.join("\u{c}"), error: None }
    }

    pub fn failed(message: impl Into<String>) -> LoadedDoc {
        LoadedDoc { format: DocFormat::Text, text: String::new(), error: Some(message.into()) }
    }

    pub fn blocks(&self) -> Vec<Block> {
        match self.format {
            DocFormat::Markdown => cobolt_forms::viewer::parse_markdown(&self.text).blocks,
            DocFormat::Html => cobolt_forms::viewer::parse_html(&self.text).blocks,
            DocFormat::Text => vec![Block::CodeBlock { language: None, text: self.text.clone() }],
            DocFormat::Pdf => {
                let n = self.text.split('\u{c}').count();
                let mut out = Vec::new();
                for (k, page) in self.text.split('\u{c}').enumerate() {
                    let (size, body) = page.split_once('\n').unwrap_or((page, ""));
                    let title = format!("Page {} of {} ({size})", k + 1, n);
                    out.push(Block::Heading { level: 3, content: vec![Inline::Text { text: title, style: Default::default() }] });
                    out.push(Block::CodeBlock { language: None, text: body.to_owned() });
                }
                out
            }
        }
    }
}

// ── values that may reach the CSSOM ─────────────────────────────────────────

/// A colour string a document may carry: `#rgb`, `#rrggbb`, a name, `rgb(...)`/`hsl(...)`.
pub fn safe_color(c: &str) -> Option<String> {
    let t = c.trim();
    if t.is_empty() || t.len() > 64 {
        return None;
    }
    let lower = t.to_ascii_lowercase();
    if lower.contains("url(") || lower.contains("expression") || lower.contains("javascript") || lower.contains('\\') {
        return None;
    }
    t.chars().all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '#' | '(' | ')' | ',' | '.' | '%' | ' ' | '/' | '-')).then(|| t.to_owned())
}

fn px(v: f32) -> String {
    format!("{}px", fmt_number(((v as f64) * 100.0).round() / 100.0))
}

fn sides(v: [f32; 4]) -> String {
    format!("{} {} {} {}", px(v[0]), px(v[1]), px(v[2]), px(v[3]))
}

fn inline_styles(mut n: VNode, s: &TextStyle) -> VNode {
    if let Some(c) = s.color.as_deref().and_then(safe_color) {
        n = n.style("color", c);
    }
    if let Some(c) = s.background.as_deref().and_then(safe_color) {
        n = n.style("background-color", c);
    }
    if let Some(u) = s.underline {
        n = n.style("text-decoration", if u { "underline" } else { "none" });
    }
    if let Some(sz) = s.size.filter(|v| v.is_finite() && *v > 0.0 && *v < 400.0) {
        n = n.style("font-size", px(sz));
    }
    if s.mono {
        n = n.style("font-family", "ui-monospace, Menlo, Consolas, monospace");
    }
    if s.letter_spacing.is_finite() && s.letter_spacing != 0.0 && s.letter_spacing.abs() < 40.0 {
        n = n.style("letter-spacing", px(s.letter_spacing));
    }
    if let Some(l) = s.line_height.filter(|v| v.is_finite() && *v > 0.0 && *v < 400.0) {
        n = n.style("line-height", px(l));
    }
    n
}

/// The scheme check every link goes through (the WebLink policy: `http`, `https`, `mailto`, `tel`,
/// a bundle-relative path or a fragment; never `javascript:` or `data:`).
pub fn link_allowed(url: &str, policy: &Policy) -> bool {
    let u = url.trim();
    if u.is_empty() {
        return false;
    }
    if u.starts_with('#') {
        return true;
    }
    check(u, false, policy).is_ok()
}

fn inline(cx: &ViewCtx, it: &Inline, key: String) -> VNode {
    match it {
        Inline::Text { text, style } => {
            let mut n = VNode::new("span").key("t").text(text.clone());
            for (tag, on) in [("strong", style.strong), ("em", style.emphasis), ("del", style.strikethrough), ("code", style.code)] {
                if on {
                    n = VNode::new(tag).key(tag).child(n);
                }
            }
            if let Some(href) = style.link.as_deref() {
                if link_allowed(href, &cx.tree.policy) {
                    let external = !href.trim().starts_with('#');
                    let mut a = VNode::new("a").key("a").attr("href", href.trim());
                    if external {
                        a = a.attr("target", "_blank").attr("rel", "noopener noreferrer nofollow");
                    }
                    n = a.child(n);
                }
            }
            // Colours and sizes from the document's CSS go on the outermost wrapper.
            n = inline_styles(n, style);
            n.key = key;
            n
        }
        Inline::Image { alt, src, title } => match cx.image_url(src) {
            Some(url) => {
                let mut img = VNode::new("img").key(key).attr("src", url).attr("alt", alt.clone()).attr("loading", "lazy").attr("decoding", "async");
                if let Some(t) = title {
                    img = img.attr("title", t.clone());
                }
                img
            }
            None => VNode::new("span").key(key).class("pr-missing-image").text(if alt.is_empty() { "[image]".to_owned() } else { alt.clone() }),
        },
        Inline::FootnoteRef { label } => VNode::new("sup").key(key).class("pr-footnote-ref").text(format!("[{label}]")),
        Inline::Break { hard } => {
            if *hard {
                VNode::new("br").key(key)
            } else {
                VNode::new("span").key(key).text(" ")
            }
        }
    }
}

fn inlines(cx: &ViewCtx, items: &[Inline], parent: VNode) -> VNode {
    let mut p = parent;
    for (k, it) in items.iter().enumerate() {
        p = p.child(inline(cx, it, format!("i{k}")));
    }
    p
}

fn align_name(a: TextAlign) -> &'static str {
    match a {
        TextAlign::Left => "left",
        TextAlign::Center => "center",
        TextAlign::Right => "right",
    }
}

fn box_styles(mut n: VNode, s: &BoxStyle) -> VNode {
    match &s.background {
        Some(Background::Solid(c)) => {
            if let Some(c) = safe_color(c) {
                n = n.style("background-color", c);
            }
        }
        Some(Background::Linear { angle, stops }) => {
            let parts: Vec<String> = stops.iter().filter_map(|(c, at)| safe_color(c).map(|c| format!("{c} {}%", fmt_number((*at as f64 * 100.0).round())))).collect();
            if parts.len() >= 2 && angle.is_finite() {
                n = n.style("background-image", format!("linear-gradient({}deg, {})", fmt_number(*angle as f64), parts.join(", ")));
            }
        }
        None => {}
    }
    if s.border_width.iter().any(|w| *w > 0.0) {
        n = n.style("border-style", "solid").style("border-width", sides(s.border_width));
        let colors: Vec<String> = s.border_color.iter().map(|c| c.as_deref().and_then(safe_color).unwrap_or_else(|| "currentColor".into())).collect();
        n = n.style("border-color", colors.join(" "));
    }
    if s.radius > 0.0 {
        n = n.style("border-radius", px(s.radius));
    }
    if s.padding.iter().any(|v| *v != 0.0) {
        n = n.style("padding", sides(s.padding));
    }
    if s.margin.iter().any(|v| *v != 0.0) {
        n = n.style("margin", sides(s.margin));
    }
    if s.center {
        n = n.style("margin-left", "auto").style("margin-right", "auto");
    }
    if let Some(w) = s.width {
        n = n.style("width", px(w));
    } else if let Some(sh) = s.width_share {
        n = n.style("width", format!("{}%", fmt_number((sh as f64 * 100.0).round())));
    }
    if let Some(w) = s.max_width {
        n = n.style("max-width", px(w));
    } else if let Some(sh) = s.max_width_share {
        n = n.style("max-width", format!("{}%", fmt_number((sh as f64 * 100.0).round())));
    }
    if let Some(h) = s.min_height {
        n = n.style("min-height", px(h));
    }
    if let Some(a) = s.text_align {
        n = n.style("text-align", align_name(a));
    }
    if let Some(sh) = &s.shadow {
        if let Some(c) = safe_color(&sh.color) {
            n = n.style("box-shadow", format!("{} {} {} {} {}", px(sh.x), px(sh.y), px(sh.blur), px(sh.spread), c));
        }
    }
    n
}

/// A code-fence language, as a class: letters, digits, `-` and `+` only.
fn language_class(l: &str) -> Option<String> {
    let l = l.trim();
    (!l.is_empty() && l.len() < 24 && l.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '+' | '_'))).then(|| format!("language-{}", l.to_ascii_lowercase()))
}

pub fn blocks_to_nodes(cx: &ViewCtx, blocks: &[Block], prefix: &str) -> Vec<VNode> {
    blocks.iter().enumerate().map(|(k, b)| block(cx, b, format!("{prefix}{k}"))).collect()
}

fn block(cx: &ViewCtx, b: &Block, key: String) -> VNode {
    match b {
        Block::Heading { level, content } => {
            let tag = match level {
                1 => "h1",
                2 => "h2",
                3 => "h3",
                4 => "h4",
                5 => "h5",
                _ => "h6",
            };
            inlines(cx, content, VNode::new(tag).key(key))
        }
        Block::Paragraph { content } => inlines(cx, content, VNode::new("p").key(key)),
        Block::CodeBlock { language, text } => {
            let mut code = VNode::new("code").key("code").text(text.clone());
            if let Some(c) = language.as_deref().and_then(language_class) {
                code = code.class(c);
            }
            VNode::new("pre").key(key).child(code)
        }
        Block::BlockQuote { blocks } => VNode::new("blockquote").key(key.clone()).children(blocks_to_nodes(cx, blocks, &format!("{key}.")) ),
        Block::List { ordered, start, items } => {
            let mut list = VNode::new(if *ordered { "ol" } else { "ul" }).key(key.clone());
            if *ordered {
                if let Some(s) = start.filter(|s| *s != 1) {
                    list = list.attr("start", s.to_string());
                }
            }
            for (k, it) in items.iter().enumerate() {
                let mut li = VNode::new("li").key(format!("li{k}"));
                if let Some(done) = it.checked {
                    li = li.class("pr-task").child(VNode::new("input").key("task").attr("type", "checkbox").attr("disabled", "").prop_bool("checked", done));
                }
                li = li.children(blocks_to_nodes(cx, &it.blocks, &format!("{key}.{k}.")));
                list = list.child(li);
            }
            list
        }
        Block::Table { alignments, header, rows, style: _ } => {
            let cell = |tag: &'static str, col: usize, content: &[Inline], key: String| {
                let mut c = inlines(cx, content, VNode::new(tag).key(key));
                match alignments.get(col) {
                    Some(TableAlignment::Left) => c = c.style("text-align", "left"),
                    Some(TableAlignment::Center) => c = c.style("text-align", "center"),
                    Some(TableAlignment::Right) => c = c.style("text-align", "right"),
                    _ => {}
                }
                c
            };
            let mut table = VNode::new("table").key(key);
            if !header.is_empty() {
                let mut tr = VNode::new("tr").key("hr");
                for (c, h) in header.iter().enumerate() {
                    tr = tr.child(cell("th", c, h, format!("h{c}")));
                }
                table = table.child(VNode::new("thead").key("thead").child(tr));
            }
            let mut body = VNode::new("tbody").key("tbody");
            for (r, row) in rows.iter().enumerate() {
                let mut tr = VNode::new("tr").key(format!("r{r}"));
                for (c, content) in row.iter().enumerate() {
                    tr = tr.child(cell("td", c, content, format!("c{c}")));
                }
                body = body.child(tr);
            }
            table.child(body)
        }
        Block::Styled { style, blocks } => box_styles(VNode::new("div").key(key.clone()).class("pr-styled"), style).children(blocks_to_nodes(cx, blocks, &format!("{key}."))),
        Block::ThematicBreak => VNode::new("hr").key(key),
        Block::Mermaid { source } => mermaid(cx, source, key),
        Block::FootnoteDefinition { label, blocks } => {
            VNode::new("div").key(key.clone()).class("pr-footnote").child(VNode::new("sup").key("label").text(format!("[{label}]"))).children(blocks_to_nodes(cx, blocks, &format!("{key}.")))
        }
        // Markup the source embedded stays the text it is.
        Block::RawHtml(text) => VNode::new("pre").key(key).class("pr-raw").child(VNode::new("code").key("code").text(text.clone())),
    }
}

/// The sources of the diagrams a document holds that the Viewer draws, at any depth: what the page
/// must ask the Mermaid part for (WASM-016 R13). A kind it does not draw is not asked for.
pub fn mermaid_wanted(blocks: &[Block], out: &mut Vec<String>) {
    for b in blocks {
        match b {
            Block::Mermaid { source } if matches!(mermaid_kind(source), MermaidKind::Flowchart | MermaidKind::Sequence) => {
                if !out.contains(source) {
                    out.push(source.clone());
                }
            }
            Block::BlockQuote { blocks } | Block::FootnoteDefinition { blocks, .. } | Block::Styled { blocks, .. } => mermaid_wanted(blocks, out),
            Block::List { items, .. } => {
                for it in items {
                    mermaid_wanted(&it.blocks, out);
                }
            }
            _ => {}
        }
    }
}

/// The diagrams a Viewer shows now (its document or its conversation), for the page to render.
pub fn mermaid_sources(cx: &ViewCtx, i: usize) -> Vec<String> {
    let mut out = Vec::new();
    if let Some(c) = cx.tree.conversation(i).filter(|c| !c.is_empty()) {
        for m in &c.messages {
            mermaid_wanted(&m.blocks, &mut out);
        }
    } else if let Some(doc) = cx.ui.viewer_docs.get(&i) {
        if doc.error.is_none() {
            mermaid_wanted(&doc.blocks(), &mut out);
        }
    }
    out
}

/// A Mermaid fence: the diagram as SVG nodes when it is a kind the Viewer draws and the Mermaid part
/// has answered, else the source and the reason (or that the diagram is on its way).
fn mermaid(cx: &ViewCtx, source: &str, key: String) -> VNode {
    let listing = |reason: Option<String>| {
        let mut n = VNode::new("figure").key(key.clone()).class("pr-mermaid-source");
        if let Some(r) = reason.filter(|r| !r.is_empty()) {
            n = n.child(VNode::new("figcaption").key("why").text(r));
        }
        n.child(VNode::new("pre").key("src").child(VNode::new("code").key("code").text(source.to_owned())))
    };
    match mermaid_kind(source) {
        MermaidKind::Flowchart | MermaidKind::Sequence => match cx.ui.mermaid.get(source) {
            Some(MermaidState::Ready(svg)) => match svg_to_vnode(svg, 20_000) {
                Some(mut s) => {
                    s.key = "diagram".into();
                    VNode::new("figure").key(key.clone()).class("pr-mermaid").attr("role", "img").child(s)
                }
                None => listing(Some("the diagram could not be drawn".into())),
            },
            Some(MermaidState::Failed(e)) => listing(Some(e.clone())),
            // Not asked for yet, or on its way: the source stays readable meanwhile.
            Some(MermaidState::Loading) | None => listing(Some("Loading the diagram\u{2026}".into())).attr("aria-busy", "true"),
        },
        MermaidKind::Unsupported(kind) => listing(Some(format!("Mermaid '{kind}' diagrams are not supported; flowchart and sequence diagrams are"))),
        MermaidKind::NotMermaid => listing(None),
    }
}

// ── the control ─────────────────────────────────────────────────────────────

/// A Viewer message: the role decides the bubble.
fn conversation(cx: &ViewCtx, c: &Conversation, i: usize) -> Vec<VNode> {
    c.messages
        .iter()
        .map(|m| {
            let role = m.role.as_str();
            let mut n = VNode::new("div").key(format!("msg:{}", m.id)).class("pr-message").attr("data-message", m.id.clone());
            if !role.is_empty() {
                n = n.class(format!("pr-{role}")).attr("data-role", role);
                let (bg, fg) = if role == "user" { ("UserBubbleColor", "UserBubbleTextColor") } else { ("AgentBubbleColor", "AgentBubbleTextColor") };
                if let Some(c) = safe_color(&bubble_color(&cx.tree.get(i, bg))) {
                    n = n.style("background-color", c);
                }
                if let Some(c) = safe_color(&bubble_color(&cx.tree.get(i, fg))) {
                    n = n.style("color", c);
                }
            }
            n.children(blocks_to_nodes(cx, &m.blocks, &format!("{}.", m.id)))
        })
        .collect()
}

/// `#RRGGBBAA` to a CSS colour (the property spelling is not CSS's).
fn bubble_color(c: &str) -> String {
    match crate::value::parse_color(c) {
        Some(a) => crate::value::css_color(a),
        None => String::new(),
    }
}

pub fn view(cx: &ViewCtx, i: usize) -> VNode {
    let mut n = root(cx, i, "div").attr("role", "document").attr("tabindex", "0").on("scroll").on("keydown");
    let zoom = parse_f64(&cx.tree.get(i, "Zoom")).or_else(|| parse_f64(&cx.tree.get(i, "View1Zoom"))).unwrap_or(100.0).clamp(25.0, 1600.0);
    let mut content = VNode::new("div").key("content").class("pr-content").style("font-size", format!("{}%", fmt_number(zoom)));
    let conv = cx.tree.conversation(i).filter(|c| !c.is_empty());
    if let Some(c) = conv {
        content = content.class("pr-conversation").children(conversation(cx, c, i));
    } else if let Some(doc) = cx.ui.viewer_docs.get(&i) {
        match &doc.error {
            Some(e) => content = content.child(VNode::new("p").key("error").class("pr-error").attr("role", "alert").text(e.clone())),
            None => content = content.children(blocks_to_nodes(cx, &doc.blocks(), "d")),
        }
    } else {
        let src = cx.tree.get(i, "Source");
        let src = if src.is_empty() { cx.tree.get(i, "View1Source") } else { src };
        if !src.trim().is_empty() {
            content = content.child(VNode::new("p").key("loading").class("pr-loading").text("\u{2026}"));
        }
    }
    let err = cx.tree.get(i, "LastError");
    if !err.is_empty() {
        content = content.child(VNode::new("p").key("last-error").class("pr-error").attr("role", "alert").text(err));
    }
    if truthy(&cx.tree.get(i, "View1FindOpen")) {
        n = n.child(find_bar(cx, i));
    }
    n = n.child(content);
    n
}

/// The Find bar (spec 058 R26–R30): a search box, "n / m", Previous, Next, Match case and Close. The needle,
/// the case switch and the current match are the control's properties (`View1SearchText`,
/// `View1SearchCaseSensitive`, `View1SearchCurrentMatch`, from 1; 0 = none), as on the desktop; the page counts
/// the matches in the text it shows and highlights them (`dom/find.rs`).
fn find_bar(cx: &ViewCtx, i: usize) -> VNode {
    let total = cx.ui.find_total.get(&i).copied().unwrap_or(0);
    let needle = cx.tree.get(i, "View1SearchText");
    let current = parse_f64(&cx.tree.get(i, "View1SearchCurrentMatch")).map(|v| v.max(0.0) as usize).unwrap_or(0);
    let case = truthy(&cx.tree.get(i, "View1SearchCaseSensitive"));
    let counter = if needle.is_empty() { String::new() } else { cobolt_forms::viewer::find_counter_text(current.saturating_sub(1).min(total.saturating_sub(1)), total) };
    let button = |key: &'static str, part: &'static str, label: &'static str, text: &'static str| {
        VNode::new("button").key(key).attr("type", "button").attr("data-part", part).attr("aria-label", label).attr("title", label).on("click").text(text)
    };
    VNode::new("div")
        .key("findbar")
        .class("pr-findbar")
        .attr("role", "search")
        .child(
            VNode::new("input")
                .key("find-input")
                .attr("type", "search")
                .attr("aria-label", "Find in the document")
                .attr("autocomplete", "off")
                .attr("spellcheck", "false")
                .attr("autofocus", "")
                .prop_str("value", needle)
                .on("input")
                .on("keydown"),
        )
        .child(VNode::new("span").key("count").class("pr-find-count").attr("role", "status").attr("aria-live", "polite").text(counter))
        .child(button("prev", "find-prev", "Previous match", "\u{2039}"))
        .child(button("next", "find-next", "Next match", "\u{203a}"))
        .child(button("case", "find-case", "Match case", "Aa").attr("aria-pressed", if case { "true" } else { "false" }))
        .child(button("close", "find-close", "Close find", "\u{00d7}"))
}

#[cfg(test)]
mod tests {
    use super::super::{Empty, Prop, UiState};
    use super::*;
    use crate::tree::testing::tree_with;
    use cobolt_forms::viewer::{AppendMode, MessageRole};

    fn render(md: &str) -> VNode {
        let (t, i) = tree_with("Viewer", "VW-1", &[]);
        let mut ui = UiState::default();
        ui.viewer_docs.insert(i, LoadedDoc { format: DocFormat::Markdown, text: md.to_owned(), error: None });
        let mut e = Empty::default();
        e.ui = ui;
        view(&e.cx(&t), i)
    }

    #[test]
    fn markdown_blocks_become_their_elements() {
        let n = render("# Title\n\nSome *emphasis* and **bold** and `code`.\n\n- one\n- two\n\n1. a\n2. b\n\n> quote\n\n| A | B |\n|---|--:|\n| 1 | 2 |\n\n---\n");
        for tag in ["h1", "p", "em", "strong", "code", "ul", "ol", "li", "blockquote", "table", "thead", "tbody", "hr"] {
            assert!(n.find_tag(tag).is_some(), "missing <{tag}>: {}", n.describe());
        }
        assert_eq!(n.find_tag("h1").unwrap().all_text(), "Title");
        let right = n.find(&|x| x.tag == "td" && x.styles.iter().any(|(k, v)| k == "text-align" && v == "right")).expect("a right-aligned cell");
        assert_eq!(right.all_text(), "2");
    }

    #[test]
    fn a_link_is_an_anchor_only_when_its_scheme_is_allowed_and_is_always_noopener() {
        let n = render("[ok](https://example.com/x) and [bad](javascript:alert(1)) and [anchor](#top) and [data](data:text/html,<b>x</b>)");
        let mut anchors = Vec::new();
        n.find_all(&|x| x.tag == "a", &mut anchors);
        let hrefs: Vec<&str> = anchors.iter().map(|a| a.attr_of("href").unwrap()).collect();
        assert_eq!(hrefs, ["https://example.com/x", "#top"], "{}", n.describe());
        assert_eq!(anchors[0].attr_of("rel"), Some("noopener noreferrer nofollow"));
        assert_eq!(anchors[0].attr_of("target"), Some("_blank"));
        assert!(n.all_text().contains("bad") && n.all_text().contains("data"), "a refused link keeps its text");
    }

    #[test]
    fn raw_html_in_a_document_stays_inert_text() {
        let n = render("Hello\n\n<script>alert(1)</script>\n\n<img src=x onerror=alert(1)>\n");
        assert!(n.find_tag("script").is_none() && n.find_tag("img").is_none());
        assert!(n.all_text().contains("<script>alert(1)</script>"), "shown as the text it is: {}", n.describe());
    }

    #[test]
    fn an_image_loads_only_from_the_application_or_an_allowed_origin() {
        let (t, i) = tree_with("Viewer", "VW-1", &[]);
        let mut e = Empty::default();
        e.assets.insert("assets/logo.png".into(), "logo.abcd1234.png".into());
        e.ui.viewer_docs.insert(i, LoadedDoc { format: DocFormat::Markdown, text: "![logo](Assets/logo.png) ![evil](https://evil.example/p.png)".into(), error: None });
        let n = view(&e.cx(&t), i);
        let mut imgs = Vec::new();
        n.find_all(&|x| x.tag == "img", &mut imgs);
        assert_eq!(imgs.len(), 1, "{}", n.describe());
        assert_eq!(imgs[0].attr_of("src"), Some("logo.abcd1234.png"));
        assert!(n.all_text().contains("evil"), "the refused image shows its alt text");
    }

    #[test]
    fn html_subset_colours_pass_only_when_they_are_colours() {
        assert_eq!(safe_color("#336699").as_deref(), Some("#336699"));
        assert_eq!(safe_color("rgb(1, 2, 3)").as_deref(), Some("rgb(1, 2, 3)"));
        assert_eq!(safe_color("url(https://evil.example/x)"), None);
        assert_eq!(safe_color("red; background: url(x)"), None);
        assert_eq!(safe_color("expression(alert(1))"), None);
    }

    #[test]
    fn a_conversation_draws_each_message_with_its_role_bubble() {
        let (mut t, i) = tree_with("Viewer", "VW-1", &[]);
        let e = Empty::default();
        let mut c = Conversation::new();
        c.append_as(AppendMode::Markdown, "Hello **there**", MessageRole::User);
        c.append_as(AppendMode::Html, "<p>Hi</p>", MessageRole::Agent);
        c.append(AppendMode::Raw, "<b>raw</b>");
        t.conversations.insert(i, c);
        let n = view(&e.cx(&t), i);
        let mut msgs = Vec::new();
        n.find_all(&|x| x.has_class("pr-message"), &mut msgs);
        assert_eq!(msgs.len(), 3);
        assert_eq!(msgs[0].attr_of("data-role"), Some("user"));
        assert!(msgs[0].styles.iter().any(|(k, _)| k == "background-color"));
        assert_eq!(msgs[1].attr_of("data-role"), Some("agent"));
        assert!(msgs[2].all_text().contains("<b>raw</b>"), "Raw is shown, not interpreted");
        assert!(n.find_tag("b").is_none());
    }

    const FLOW: &str = "flowchart LR\n  A[Start] --> B[Review] --> C[Done]\n";

    /// A document with one fence, the Mermaid part's answer for `source` being `state`.
    fn render_diagram(source: &str, state: Option<MermaidState>) -> VNode {
        let (t, i) = tree_with("Viewer", "VW-1", &[]);
        let mut e = Empty::default();
        e.ui.viewer_docs.insert(i, LoadedDoc { format: DocFormat::Markdown, text: format!("```mermaid\n{source}```\n"), error: None });
        if let Some(s) = state {
            e.ui.mermaid.insert(source.to_owned(), s);
        }
        view(&e.cx(&t), i)
    }

    #[test]
    fn a_flowchart_the_part_has_drawn_is_svg_nodes() {
        // What the part answers is what the desktop Viewer's renderer answers (the dev-dependency turns it on here).
        let svg_text = cobolt_forms::viewer::render_mermaid_svg(FLOW).expect("the renderer draws a flowchart");
        let n = render_diagram(FLOW, Some(MermaidState::Ready(svg_text)));
        let figure = n.find(&|x| x.has_class("pr-mermaid")).expect("a diagram");
        let svg = figure.find_tag("svg").unwrap();
        assert!(svg.count("path") + svg.count("line") + svg.count("polyline") > 0, "{}", svg.describe());
        assert!(svg.all_text().contains("Review"), "the node labels are text nodes");
        assert!(n.find_tag("pre").is_none(), "the source is not shown beside a diagram that is drawn");
    }

    #[test]
    fn until_the_part_answers_the_source_stays_readable_and_a_failure_says_why() {
        for state in [None, Some(MermaidState::Loading)] {
            let n = render_diagram(FLOW, state);
            let fig = n.find(&|x| x.has_class("pr-mermaid-source")).expect("the listing");
            assert_eq!(fig.attr_of("aria-busy"), Some("true"));
            assert!(fig.all_text().contains("Loading the diagram") && fig.all_text().contains("A[Start]"), "{}", fig.describe());
        }
        let n = render_diagram(FLOW, Some(MermaidState::Failed("the Mermaid part could not be loaded".into())));
        let fig = n.find(&|x| x.has_class("pr-mermaid-source")).unwrap();
        assert!(fig.attr_of("aria-busy").is_none() && fig.all_text().contains("could not be loaded"), "{}", fig.describe());
    }

    #[test]
    fn an_unsupported_diagram_says_why_without_asking_the_part() {
        let class = "classDiagram\n  A <|-- B\n";
        let bad = render_diagram(class, None);
        assert!(bad.all_text().contains("not supported"), "{}", bad.describe());
        let mut wanted = Vec::new();
        mermaid_wanted(&cobolt_forms::viewer::parse_markdown(&format!("```mermaid\n{class}```\n")).blocks, &mut wanted);
        assert!(wanted.is_empty(), "a kind the Viewer does not draw is not asked for: {wanted:?}");
        // Text that is not a diagram is shown as text, with no reason and no request.
        let text = render_diagram("[ Start ] -> [ End ]\n", None);
        assert!(text.find(&|x| x.has_class("pr-mermaid-source")).unwrap().find_tag("figcaption").is_none());
    }

    #[test]
    fn the_diagrams_to_ask_for_are_found_at_any_depth_once_each() {
        let md = format!(
            "```mermaid\n{FLOW}```\n\n> ```mermaid\n> sequenceDiagram\n>   A->>B: hi\n> ```\n\n- item\n\n  ```mermaid\n  {}  ```\n\n```mermaid\n{FLOW}```\n",
            "flowchart TD\n  X --> Y\n"
        );
        let mut wanted = Vec::new();
        mermaid_wanted(&cobolt_forms::viewer::parse_markdown(&md).blocks, &mut wanted);
        assert_eq!(wanted.len(), 3, "the repeated diagram once: {wanted:?}");
        assert!(wanted.iter().any(|s| s.starts_with("sequenceDiagram")));
        // From the view's own state: a Viewer's document.
        let (t, i) = tree_with("Viewer", "VW-1", &[]);
        let mut e = Empty::default();
        e.ui.viewer_docs.insert(i, LoadedDoc { format: DocFormat::Markdown, text: md, error: None });
        assert_eq!(mermaid_sources(&e.cx(&t), i).len(), 3);
        // And a conversation's messages.
        let (mut t2, i2) = tree_with("Viewer", "VW-2", &[]);
        let mut c = Conversation::new();
        c.append_as(AppendMode::Markdown, &format!("```mermaid\n{FLOW}```\n"), MessageRole::Agent);
        t2.conversations.insert(i2, c);
        assert_eq!(mermaid_sources(&Empty::default().cx(&t2), i2), vec![FLOW.to_owned()]);
    }

    #[test]
    fn the_find_bar_shows_the_needle_the_live_counter_and_its_buttons_only_while_it_is_open() {
        let (mut t, i) = tree_with("Viewer", "VW-1", &[]);
        let mut e = Empty::default();
        e.ui.viewer_docs.insert(i, LoadedDoc { format: DocFormat::Markdown, text: "alpha beta alpha".into(), error: None });
        assert!(view(&e.cx(&t), i).find(&|x| x.has_class("pr-findbar")).is_none(), "closed: no bar");
        t.set(i, "View1FindOpen", "1");
        t.set(i, "View1SearchText", "alpha");
        t.set(i, "View1SearchCurrentMatch", "2");
        e.ui.find_total.insert(i, 2);
        let n = view(&e.cx(&t), i);
        let bar = n.find(&|x| x.has_class("pr-findbar")).expect("the bar");
        assert_eq!(bar.attr_of("role"), Some("search"));
        let input = bar.find_tag("input").unwrap();
        assert_eq!((input.attr_of("type"), input.prop_of("value")), (Some("search"), Some(&Prop::Str("alpha".into()))));
        assert_eq!(bar.find(&|x| x.has_class("pr-find-count")).unwrap().all_text(), "2 / 2", "the desktop's own counter text");
        let parts: Vec<&str> = {
            let mut b = Vec::new();
            bar.find_all(&|x| x.tag == "button", &mut b);
            b.iter().map(|x| x.attr_of("data-part").unwrap()).collect()
        };
        assert_eq!(parts, ["find-prev", "find-next", "find-case", "find-close"]);
        assert!(bar.find(&|x| x.attr_of("aria-pressed") == Some("false")).is_some(), "Match case is a toggle that says its state");
        // Nothing found says so honestly; no needle shows no counter.
        e.ui.find_total.insert(i, 0);
        t.set(i, "View1SearchCurrentMatch", "0");
        assert_eq!(view(&e.cx(&t), i).find(&|x| x.has_class("pr-find-count")).unwrap().all_text(), "0 / 0");
        t.set(i, "View1SearchText", "");
        assert_eq!(view(&e.cx(&t), i).find(&|x| x.has_class("pr-find-count")).unwrap().all_text(), "");
        // The bar is not the document: the content is still there, after it.
        assert!(view(&e.cx(&t), i).find(&|x| x.has_class("pr-content")).is_some());
    }

    #[test]
    fn a_pdf_the_part_read_shows_one_heading_and_one_text_block_per_page_and_an_empty_one_says_so() {
        let doc = LoadedDoc::from_pdf_json(r#"{"pages":[{"width":612,"height":792,"text":"first\nline"},{"width":595.2,"height":841.8,"text":"second"}]}"#);
        assert_eq!(doc.error, None);
        let blocks = doc.blocks();
        assert_eq!(blocks.len(), 4, "a heading and a text block for each of the two pages");
        let Block::Heading { content, .. } = &blocks[2] else { panic!("a heading") };
        let Inline::Text { text, .. } = &content[0] else { panic!("text") };
        assert_eq!(text, "Page 2 of 2 (595 x 842 pt)");
        let Block::CodeBlock { text, .. } = &blocks[1] else { panic!("the text") };
        assert_eq!(text, "first\nline", "the page's line structure is kept");
        assert!(LoadedDoc::from_pdf_json(r#"{"pages":[]}"#).error.is_some(), "no pages is said, not drawn blank");
        assert!(LoadedDoc::from_pdf_json("not json").error.is_some());
    }

    #[test]
    fn a_failed_document_shows_its_error_as_an_alert() {
        let (t, i) = tree_with("Viewer", "VW-1", &[]);
        let mut e = Empty::default();
        e.ui.viewer_docs.insert(i, LoadedDoc::failed("the file could not be read"));
        let n = view(&e.cx(&t), i);
        let alert = n.find(&|x| x.attr_of("role") == Some("alert")).unwrap();
        assert_eq!(alert.all_text(), "the file could not be read");
    }

    #[test]
    fn the_zoom_scales_the_content() {
        let (t, i) = tree_with("Viewer", "VW-1", &[("Zoom", "150")]);
        let e = Empty::default();
        let n = view(&e.cx(&t), i);
        let content = n.find(&|x| x.has_class("pr-content")).unwrap();
        assert!(content.styles.contains(&("font-size".to_owned(), "150%".to_owned())));
        let _ = UiState::default();
    }
}
