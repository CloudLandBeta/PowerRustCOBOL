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

use cobolt_forms::viewer::{mermaid_kind, render_mermaid_svg, Background, Block, BoxStyle, Conversation, Inline, MermaidKind, TableAlignment, TextAlign, TextStyle};

use super::controls::root;
use super::svg::svg_to_vnode;
use super::{VNode, ViewCtx};
use crate::value::{fmt_number, parse_f64};
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

    pub fn failed(message: impl Into<String>) -> LoadedDoc {
        LoadedDoc { format: DocFormat::Text, text: String::new(), error: Some(message.into()) }
    }

    pub fn blocks(&self) -> Vec<Block> {
        match self.format {
            DocFormat::Markdown => cobolt_forms::viewer::parse_markdown(&self.text).blocks,
            DocFormat::Html => cobolt_forms::viewer::parse_html(&self.text).blocks,
            DocFormat::Text => vec![Block::CodeBlock { language: None, text: self.text.clone() }],
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
pub fn link_allowed(url: &str) -> bool {
    let u = url.trim();
    if u.is_empty() {
        return false;
    }
    if u.starts_with('#') {
        return true;
    }
    check(u, false, &Policy::default()).is_ok()
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
                if link_allowed(href) {
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
        Block::Mermaid { source } => mermaid(source, key),
        Block::FootnoteDefinition { label, blocks } => {
            VNode::new("div").key(key.clone()).class("pr-footnote").child(VNode::new("sup").key("label").text(format!("[{label}]"))).children(blocks_to_nodes(cx, blocks, &format!("{key}.")))
        }
        // Markup the source embedded stays the text it is.
        Block::RawHtml(text) => VNode::new("pre").key(key).class("pr-raw").child(VNode::new("code").key("code").text(text.clone())),
    }
}

/// A Mermaid fence: the diagram as SVG nodes when it is a kind the Viewer draws, else the source and the reason.
fn mermaid(source: &str, key: String) -> VNode {
    let listing = |reason: Option<String>| {
        let mut n = VNode::new("figure").key(key.clone()).class("pr-mermaid-source");
        if let Some(r) = reason.filter(|r| !r.is_empty()) {
            n = n.child(VNode::new("figcaption").key("why").text(r));
        }
        n.child(VNode::new("pre").key("src").child(VNode::new("code").key("code").text(source.to_owned())))
    };
    match mermaid_kind(source) {
        MermaidKind::Flowchart | MermaidKind::Sequence => match render_mermaid_svg(source) {
            Ok(svg) => match svg_to_vnode(&svg, 20_000) {
                Some(mut s) => {
                    s.key = "diagram".into();
                    VNode::new("figure").key(key.clone()).class("pr-mermaid").attr("role", "img").child(s)
                }
                None => listing(Some("the diagram could not be drawn".into())),
            },
            Err(e) => listing(Some(e)),
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
    let mut n = root(cx, i, "div").attr("role", "document").attr("tabindex", "0").on("scroll");
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
    n = n.child(content);
    n
}

#[cfg(test)]
mod tests {
    use super::super::{Empty, UiState};
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

    #[test]
    fn a_flowchart_is_drawn_as_svg_nodes_and_an_unsupported_diagram_says_why() {
        let ok = render("```mermaid\nflowchart LR\n  A[Start] --> B[Review] --> C[Done]\n```\n");
        let figure = ok.find(&|x| x.has_class("pr-mermaid")).expect("a diagram");
        let svg = figure.find_tag("svg").unwrap();
        assert!(svg.count("path") + svg.count("line") + svg.count("polyline") > 0, "{}", svg.describe());
        assert!(svg.all_text().contains("Review"), "the node labels are text nodes");
        let bad = render("```mermaid\nclassDiagram\n  A <|-- B\n```\n");
        assert!(bad.all_text().contains("not supported"), "{}", bad.describe());
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
