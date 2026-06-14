// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! A small, theme-aware Markdown renderer for the Documentation viewer.
//!
//! Rather than a third-party widget, this walks `pulldown-cmark` events and
//! draws egui directly, which gives full control over the things the docs need:
//! word-wrapped text, headings (with rects captured for the table-of-contents),
//! **COBOL-coloured code in its own boxed block**, tables, blockquotes,
//! inline search-term highlighting (blue-on-yellow), and inline Mermaid diagrams
//! (drawn via a caller-supplied closure). Every block is a single egui widget,
//! so there are no widget-id clashes.

use egui::text::{LayoutJob, TextFormat};
use egui::{Color32, FontId, RichText, Stroke, Ui, Vec2};
use pulldown_cmark::{CodeBlockKind, Event, HeadingLevel, Options, Parser, Tag, TagEnd};

use crate::panels::editor;

/// Options controlling a render pass.
pub struct RenderOpts<'a> {
    /// Lower-cased search query; matches are highlighted. Empty = no highlight.
    pub search: &'a str,
    /// Base body font size (points).
    pub base: f32,
    /// Heading index to scroll into view this frame, if any.
    pub scroll_to_heading: Option<usize>,
    /// Index of the currently-focused match (0-based). It is highlighted with a
    /// distinct colour so the user can see which match the nav controls landed on.
    pub active_match: Option<usize>,
    /// Scroll the block containing [`Self::active_match`] into view this frame.
    pub scroll_to_active: bool,
    /// Heading anchors as `(slug, heading_index)`, so in-document links of the
    /// form `[text](#slug)` (the table of contents) can jump to their section.
    pub anchors: &'a [(String, usize)],
}

/// Result of a render pass.
#[derive(Default)]
pub struct RenderOutput {
    /// Number of headings drawn (their order matches the outline).
    pub heading_count: usize,
    /// Number of search matches highlighted.
    pub match_count: usize,
    /// Heading index a clicked in-document anchor link resolved to, if any.
    pub clicked_heading: Option<usize>,
    /// Screen-space top Y of the block to scroll to this frame (the active match
    /// or the requested heading), so the caller can drive the scroll offset.
    pub scroll_target_y: Option<f32>,
}

/// Inline style state while accumulating a text block.
#[derive(Clone, Copy)]
struct Inline {
    bold: bool,
    italic: bool,
    code: bool,
    link: bool,
}

impl Inline {
    fn none() -> Self {
        Self { bold: false, italic: false, code: false, link: false }
    }
}

/// Render `markdown` into `ui`. `mermaid` is called to draw a Mermaid diagram
/// (its code is passed); it should add an image (or a fallback) to the `ui`.
pub fn render(
    ui: &mut Ui,
    markdown: &str,
    opts: &RenderOpts,
    mermaid: &mut dyn FnMut(&mut Ui, &str),
) -> RenderOutput {
    let mut r = Renderer::new(ui, opts);
    let parser = Parser::new_ext(
        markdown,
        Options::ENABLE_TABLES | Options::ENABLE_STRIKETHROUGH | Options::ENABLE_TASKLISTS,
    );
    let events: Vec<Event> = parser.collect();
    let mut i = 0;
    while i < events.len() {
        i = r.event(ui, &events, i, mermaid);
    }
    r.flush_block(ui);
    RenderOutput {
        heading_count: r.heading_idx,
        match_count: r.match_count,
        clicked_heading: r.clicked_heading,
        scroll_target_y: r.scroll_target_y,
    }
}

struct Runs {
    job: LayoutJob,
}

/// One inline piece of a block: either plain styled text or a clickable link.
enum Seg {
    Text(LayoutJob),
    Link(LayoutJob, String),
}

struct Renderer<'a> {
    base: f32,
    search: &'a str,
    body_color: Color32,
    dim_color: Color32,
    link_color: Color32,
    code_bg: Color32,
    // current block accumulation
    runs: Option<Runs>,
    /// Inline segments for the current block when it contains links; empty for
    /// the common link-free fast path (a single wrapped label).
    segs: Vec<Seg>,
    /// Destination of the link currently being accumulated, if any.
    link_target: Option<String>,
    anchors: &'a [(String, usize)],
    clicked_heading: Option<usize>,
    heading: Option<HeadingLevel>,
    inline: Inline,
    // list state: (ordered next number or None for bullet)
    list_stack: Vec<Option<u64>>,
    quote_depth: u32,
    heading_idx: usize,
    match_count: usize,
    scroll_to_heading: Option<usize>,
    active_match: Option<usize>,
    scroll_to_active: bool,
    /// Set while accumulating a block when it contains the active match, so
    /// `flush_block` can scroll that block into view.
    scroll_block: bool,
    /// Screen-space top Y of the block to scroll to (active match or heading).
    scroll_target_y: Option<f32>,
}

impl<'a> Renderer<'a> {
    fn new(ui: &Ui, opts: &'a RenderOpts) -> Self {
        let v = ui.visuals();
        Self {
            base: opts.base,
            search: opts.search,
            body_color: v.text_color(),
            dim_color: v.weak_text_color(),
            link_color: v.hyperlink_color,
            code_bg: v.extreme_bg_color,
            runs: None,
            segs: Vec::new(),
            link_target: None,
            anchors: opts.anchors,
            clicked_heading: None,
            heading: None,
            inline: Inline::none(),
            list_stack: Vec::new(),
            quote_depth: 0,
            heading_idx: 0,
            match_count: 0,
            scroll_to_heading: opts.scroll_to_heading,
            active_match: opts.active_match,
            scroll_to_active: opts.scroll_to_active,
            scroll_block: false,
            scroll_target_y: None,
        }
    }

    fn font_for(&self, inline: Inline, heading: Option<HeadingLevel>) -> (FontId, Color32) {
        let size = match heading {
            Some(HeadingLevel::H1) => self.base * 1.9,
            Some(HeadingLevel::H2) => self.base * 1.55,
            Some(HeadingLevel::H3) => self.base * 1.3,
            Some(HeadingLevel::H4) => self.base * 1.12,
            Some(_) => self.base * 1.02,
            None => self.base,
        };
        let family = if inline.code {
            egui::FontFamily::Monospace
        } else {
            egui::FontFamily::Proportional
        };
        let color = if inline.link {
            self.link_color
        } else if inline.code {
            self.dim_color
        } else {
            self.body_color
        };
        (FontId::new(size, family), color)
    }

    /// Append `text` to the current block, splitting out search matches so they
    /// can be highlighted (blue text on a yellow background).
    fn push_text(&mut self, text: &str) {
        if self.runs.is_none() {
            self.runs = Some(Runs { job: LayoutJob::default() });
        }
        let (font, color) = self.font_for(self.inline, self.heading);
        let mut fmt = TextFormat { font_id: font, color, ..Default::default() };
        if self.inline.italic {
            fmt.italics = true;
        }
        if self.inline.bold {
            // Approximate bold with a brighter colour (single font family).
            fmt.color = Color32::WHITE.gamma_multiply(0.92).max_color(fmt.color);
        }

        let job = &mut self.runs.as_mut().unwrap().job;
        if self.search.is_empty() {
            job.append(text, 0.0, fmt);
            return;
        }
        // Highlight occurrences of the (lower-cased) query.
        let hay = text.to_lowercase();
        let mut start = 0usize;
        while let Some(rel) = hay[start..].find(self.search) {
            let m0 = start + rel;
            let m1 = m0 + self.search.len();
            if m0 > start {
                job.append(&text[start..m0], 0.0, fmt.clone());
            }
            let mut hl = fmt.clone();
            if self.active_match == Some(self.match_count) {
                // The currently-focused match: dark text on orange.
                hl.color = Color32::from_rgb(20, 12, 0);
                hl.background = Color32::from_rgb(255, 150, 40);
                if self.scroll_to_active {
                    self.scroll_block = true;
                }
            } else {
                hl.color = Color32::from_rgb(20, 40, 200); // blue text
                hl.background = Color32::from_rgb(255, 235, 90); // yellow background
            }
            job.append(&text[m0..m1], 0.0, hl);
            self.match_count += 1;
            start = m1;
            if self.search.is_empty() {
                break;
            }
        }
        if start < text.len() {
            job.append(&text[start..], 0.0, fmt);
        }
    }

    /// Move the current text run into the segment list (used at link boundaries).
    fn push_run_seg(&mut self) {
        if let Some(r) = self.runs.take() {
            if !r.job.text.is_empty() {
                self.segs.push(Seg::Text(r.job));
            }
        }
    }

    /// Resolve an in-document `#slug` link to the heading index it points at.
    fn resolve_anchor(&self, target: &str) -> Option<usize> {
        let slug = target.strip_prefix('#')?.to_lowercase();
        self.anchors.iter().find(|(s, _)| *s == slug).map(|(_, i)| *i)
    }

    /// Draw the accumulated text block as one wrapped label.
    fn flush_block(&mut self, ui: &mut Ui) {
        // A block containing links is drawn as a sequence of inline widgets.
        if !self.segs.is_empty() || self.link_target.is_some() {
            self.flush_segs(ui);
            return;
        }
        let Some(mut runs) = self.runs.take() else { return };
        runs.job.wrap.max_width = ui.available_width();
        let heading = self.heading.take();

        let indent = self.quote_depth as f32 * 14.0;
        let scroll_match = std::mem::replace(&mut self.scroll_block, false);
        ui.horizontal_wrapped(|ui| {
            if indent > 0.0 {
                ui.add_space(indent);
            }
            let resp = ui.label(runs.job);
            // Record the scroll target (the caller drives the scroll offset).
            if scroll_match {
                self.scroll_target_y = Some(resp.rect.top());
            }
            if let Some(_h) = heading {
                if self.scroll_to_heading == Some(self.heading_idx) {
                    self.scroll_target_y = Some(resp.rect.top());
                }
                self.heading_idx += 1;
            }
        });
        if heading.is_some() {
            ui.add_space(self.base * 0.25);
        }
    }

    /// Draw a block that contains links: text runs as labels, links as clickable
    /// widgets that jump to their target heading (the table of contents).
    fn flush_segs(&mut self, ui: &mut Ui) {
        self.push_run_seg();
        let segs = std::mem::take(&mut self.segs);
        let heading = self.heading.take();
        let indent = self.quote_depth as f32 * 14.0;
        let scroll_match = std::mem::replace(&mut self.scroll_block, false);

        let inner = ui.horizontal_wrapped(|ui| {
            if indent > 0.0 {
                ui.add_space(indent);
            }
            for seg in segs {
                match seg {
                    Seg::Text(job) => {
                        ui.label(job);
                    }
                    Seg::Link(job, target) => {
                        if ui.link(job).clicked() {
                            if let Some(idx) = self.resolve_anchor(&target) {
                                self.clicked_heading = Some(idx);
                            }
                        }
                    }
                }
            }
        });
        if scroll_match {
            self.scroll_target_y = Some(inner.response.rect.top());
        }
        if heading.is_some() {
            self.heading_idx += 1;
            ui.add_space(self.base * 0.25);
        }
    }

    fn event(
        &mut self,
        ui: &mut Ui,
        events: &[Event],
        i: usize,
        mermaid: &mut dyn FnMut(&mut Ui, &str),
    ) -> usize {
        match &events[i] {
            Event::Start(tag) => self.start(ui, tag.clone(), events, i, mermaid),
            Event::End(tag) => {
                self.end(ui, *tag);
                i + 1
            }
            Event::Text(t) => {
                self.push_text(t);
                i + 1
            }
            Event::Code(t) => {
                let saved = self.inline.code;
                self.inline.code = true;
                self.push_text(t);
                self.inline.code = saved;
                i + 1
            }
            Event::SoftBreak => {
                self.push_text(" ");
                i + 1
            }
            Event::HardBreak => {
                self.flush_block(ui);
                i + 1
            }
            Event::Rule => {
                self.flush_block(ui);
                ui.separator();
                i + 1
            }
            Event::TaskListMarker(done) => {
                self.push_text(if *done { "☑ " } else { "☐ " });
                i + 1
            }
            _ => i + 1,
        }
    }

    fn start(
        &mut self,
        ui: &mut Ui,
        tag: Tag,
        events: &[Event],
        i: usize,
        mermaid: &mut dyn FnMut(&mut Ui, &str),
    ) -> usize {
        match tag {
            Tag::Heading { level, .. } => {
                self.flush_block(ui);
                ui.add_space(self.base * 0.5);
                self.heading = Some(level);
                i + 1
            }
            Tag::Paragraph => i + 1,
            Tag::Strong => {
                self.inline.bold = true;
                i + 1
            }
            Tag::Emphasis => {
                self.inline.italic = true;
                i + 1
            }
            Tag::Link { dest_url, .. } => {
                // Close the current text run, then accumulate the link's text
                // separately so it can be drawn as a clickable widget.
                self.push_run_seg();
                self.inline.link = true;
                self.link_target = Some(dest_url.to_string());
                i + 1
            }
            Tag::List(start) => {
                self.flush_block(ui);
                self.list_stack.push(start);
                i + 1
            }
            Tag::Item => {
                self.flush_block(ui);
                let marker = match self.list_stack.last_mut() {
                    Some(Some(n)) => {
                        let s = format!("{}. ", n);
                        *n += 1;
                        s
                    }
                    _ => "•  ".to_string(),
                };
                let indent = self.list_stack.len().saturating_sub(1) as f32 * 16.0 + 8.0;
                self.runs = Some(Runs { job: LayoutJob::default() });
                // Prepend the marker as dim text.
                let (font, _c) = self.font_for(Inline::none(), None);
                self.runs.as_mut().unwrap().job.append(
                    &format!("{:width$}{marker}", "", width = (indent / 6.0) as usize),
                    0.0,
                    TextFormat { font_id: font, color: self.dim_color, ..Default::default() },
                );
                i + 1
            }
            Tag::BlockQuote(_) => {
                self.flush_block(ui);
                self.quote_depth += 1;
                i + 1
            }
            Tag::CodeBlock(kind) => {
                self.flush_block(ui);
                let lang = match kind {
                    CodeBlockKind::Fenced(l) => l.to_string(),
                    CodeBlockKind::Indented => String::new(),
                };
                // Collect the code text until the matching End(CodeBlock).
                let mut code = String::new();
                let mut j = i + 1;
                while j < events.len() {
                    match &events[j] {
                        Event::Text(t) => code.push_str(t),
                        Event::End(TagEnd::CodeBlock) => break,
                        _ => {}
                    }
                    j += 1;
                }
                if lang.eq_ignore_ascii_case("mermaid") {
                    mermaid(ui, &code);
                } else {
                    self.draw_code_box(ui, &code, &lang);
                }
                j + 1 // skip past End(CodeBlock)
            }
            Tag::Table(_) => {
                self.flush_block(ui);
                self.draw_table(ui, events, i)
            }
            Tag::Image { .. } => {
                // Skip image data; emit the alt text (collected as following Text).
                i + 1
            }
            _ => i + 1,
        }
    }

    fn end(&mut self, ui: &mut Ui, tag: TagEnd) {
        match tag {
            TagEnd::Heading(_) => self.flush_block(ui),
            TagEnd::Paragraph => {
                self.flush_block(ui);
                ui.add_space(self.base * 0.4);
            }
            TagEnd::Strong => self.inline.bold = false,
            TagEnd::Emphasis => self.inline.italic = false,
            TagEnd::Link => {
                self.inline.link = false;
                let target = self.link_target.take().unwrap_or_default();
                let job = self.runs.take().map(|r| r.job).unwrap_or_default();
                self.segs.push(Seg::Link(job, target));
            }
            TagEnd::Item => self.flush_block(ui),
            TagEnd::List(_) => {
                self.list_stack.pop();
                ui.add_space(self.base * 0.3);
            }
            TagEnd::BlockQuote(_) => {
                self.flush_block(ui);
                self.quote_depth = self.quote_depth.saturating_sub(1);
            }
            _ => {}
        }
    }

    /// Draw a fenced code block as a full-width, boxed block on its own line.
    /// COBOL is syntax-coloured via the editor's highlighter.
    fn draw_code_box(&mut self, ui: &mut Ui, code: &str, lang: &str) {
        let code = code.strip_suffix('\n').unwrap_or(code);
        let is_cobol = matches!(
            lang.to_lowercase().as_str(),
            "cobol" | "cob" | "cbl" | "cobolt" | "pcr"
        );
        let job = if is_cobol {
            editor::highlight_cobol(code)
        } else {
            editor::mono_layout_job(
                code,
                FontId::monospace(self.base * 0.95),
                self.body_color,
            )
        };
        ui.add_space(self.base * 0.3);
        egui::Frame::none()
            .fill(self.code_bg)
            .inner_margin(egui::Margin::same(8.0))
            .rounding(egui::Rounding::same(5.0))
            .stroke(Stroke::new(1.0, ui.visuals().widgets.noninteractive.bg_stroke.color))
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                egui::ScrollArea::horizontal()
                    .id_salt(("codebox", code.len()))
                    .show(ui, |ui| {
                        ui.add(egui::Label::new(job).wrap_mode(egui::TextWrapMode::Extend));
                    });
            });
        ui.add_space(self.base * 0.4);
    }

    /// Render a Markdown table with wrapped cells. Returns the index just past
    /// the table's `End` event.
    fn draw_table(&mut self, ui: &mut Ui, events: &[Event], start: usize) -> usize {
        // Collect rows of cells (each cell is the concatenated text).
        let mut rows: Vec<Vec<String>> = Vec::new();
        let mut cur_row: Vec<String> = Vec::new();
        let mut cur_cell = String::new();
        let mut in_head = false;
        let mut j = start + 1;
        let mut depth = 1; // already inside Table
        while j < events.len() && depth > 0 {
            match &events[j] {
                Event::Start(Tag::Table(_)) => depth += 1,
                Event::End(TagEnd::Table) => depth -= 1,
                Event::Start(Tag::TableHead) => in_head = true,
                Event::End(TagEnd::TableHead) => {
                    in_head = false;
                    if !cur_row.is_empty() {
                        rows.push(std::mem::take(&mut cur_row));
                    }
                }
                Event::End(TagEnd::TableRow) => {
                    if !cur_row.is_empty() {
                        rows.push(std::mem::take(&mut cur_row));
                    }
                }
                Event::End(TagEnd::TableCell) => {
                    cur_row.push(std::mem::take(&mut cur_cell));
                }
                Event::Text(t) | Event::Code(t) => cur_cell.push_str(t),
                Event::SoftBreak | Event::HardBreak => cur_cell.push(' '),
                _ => {}
            }
            j += 1;
        }
        let _ = in_head;

        let cols = rows.iter().map(|r| r.len()).max().unwrap_or(0);
        if cols == 0 {
            return j;
        }
        let col_w = (ui.available_width() / cols as f32 - 8.0).max(60.0);
        egui::Frame::none()
            .stroke(Stroke::new(1.0, ui.visuals().widgets.noninteractive.bg_stroke.color))
            .inner_margin(egui::Margin::same(2.0))
            .show(ui, |ui| {
                egui::Grid::new(("md_table", start))
                    .striped(true)
                    .min_col_width(col_w)
                    .max_col_width(col_w)
                    .show(ui, |ui| {
                        for (ri, row) in rows.iter().enumerate() {
                            for c in 0..cols {
                                let text = row.get(c).map(String::as_str).unwrap_or("");
                                let mut rt = RichText::new(text).size(self.base);
                                if ri == 0 {
                                    rt = rt.strong();
                                }
                                ui.add(egui::Label::new(rt).wrap());
                            }
                            ui.end_row();
                        }
                    });
            });
        ui.add_space(self.base * 0.4);
        j
    }
}

// Small helper: pick the more visible of two colours (used to fake bold).
trait MaxColor {
    fn max_color(self, other: Color32) -> Color32;
}
impl MaxColor for Color32 {
    fn max_color(self, other: Color32) -> Color32 {
        // Prefer the brighter colour so "bold" reads as emphasis.
        let lum = |c: Color32| c.r() as u32 + c.g() as u32 + c.b() as u32;
        if lum(self) >= lum(other) {
            self
        } else {
            other
        }
    }
}

// Keep `Vec2` import used (silences unused warning if layout changes).
#[allow(dead_code)]
fn _vec2(_: Vec2) {}
