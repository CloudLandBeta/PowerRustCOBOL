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
//! inline search-term highlighting (blue-on-yellow), inline Mermaid diagrams and
//! embedded images (both drawn via a caller-supplied closure). Every block is a
//! single egui widget, so there are no widget-id clashes.
//!
//! # Drawing only what is on screen
//!
//! The Developer's Guide is half a megabyte of Markdown — around four thousand
//! blocks. Laying every one of them out on every frame is what made the
//! documentation window crawl. [`BlockCache`] fixes that: each block's height is
//! remembered from the pass that drew it, and a block far enough outside the
//! viewport is replaced by that much empty space instead of being laid out
//! again. The scrollbar and every scroll offset stay exactly where they were,
//! because the reserved space is the height the block really had.
//!
//! A caller that passes no cache draws everything, exactly as before.

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
    /// How tables lay their columns out.
    pub table_layout: TableLayout,
}

/// Column strategy for a rendered markdown table.
#[derive(Clone, Copy, PartialEq, Eq, Default, Debug)]
pub enum TableLayout {
    /// Every column the same share of the width, no resizing. The historical
    /// behaviour, and what the documentation viewer keeps.
    #[default]
    Equal,
    /// Columns sized to their own content and **resizable by the user**, with
    /// a visible line at each boundary; the **last** column takes whatever
    /// width is left and is the only one that wraps. For a results table
    /// whose first columns are short values and whose last is prose
    /// (spec 044: crate · version · downloads · description).
    TightResizable,
}

/// One image the document embeds — from Markdown `![alt](src)` or from an HTML
/// `<img src=… alt=… width=…>` tag, which is how the guide places its
/// screenshots. The renderer never loads anything itself: it hands this to the
/// caller's closure, which owns path resolution and the texture cache.
pub struct ImageRef<'a> {
    /// The `src` exactly as the document wrote it, e.g.
    /// `../assets/images/screenshots/welcome.png`.
    pub src: &'a str,
    /// The `alt` text, shown when the image cannot be loaded.
    pub alt: &'a str,
    /// The `width=` attribute in CSS pixels, when the tag carried one.
    pub width: Option<f32>,
}

/// Remembered block heights, so a long document only lays out what is near the
/// viewport. See the module documentation.
#[derive(Default)]
pub struct BlockCache {
    /// Identity of the layout these heights were measured under (document,
    /// content width, body font size). Anything else and they do not apply.
    key: u64,
    /// Height of block *i*, in points, or `0.0` for a block never drawn yet.
    heights: Vec<f32>,
    /// Blocks actually drawn in the last pass — the measurement behind the
    /// speed claim, and what the tests assert on.
    drawn: usize,
    /// How far past the viewport to keep drawing for real, in points. Blocks
    /// inside this margin are ready before they are scrolled into view.
    read_ahead: f32,
}

impl BlockCache {
    /// A cache that keeps `read_ahead` points of drawn-for-real content above
    /// and below the viewport.
    pub fn new(read_ahead: f32) -> Self {
        Self {
            read_ahead,
            ..Default::default()
        }
    }

    /// Point the cache at a layout. Remembered heights are dropped when the
    /// document, the content width or the font size changes, because they were
    /// measured under the old one.
    pub fn retarget(&mut self, key: u64) {
        if self.key != key {
            self.key = key;
            self.heights.clear();
            self.drawn = 0;
        }
    }

    /// Blocks drawn for real in the last pass (the rest were reserved space).
    pub fn drawn(&self) -> usize {
        self.drawn
    }

    /// Blocks the last pass walked, drawn or reserved.
    pub fn blocks(&self) -> usize {
        self.heights.len()
    }
}

/// The callbacks and caches a render pass may be given. Everything is optional
/// except the Mermaid drawer, so existing callers are unaffected.
pub struct Extras<'a> {
    /// Draws one Mermaid diagram; its code is passed.
    pub mermaid: &'a mut dyn FnMut(&mut Ui, &str),
    /// Draws one embedded image. Without it, images render as their alt text.
    pub image: Option<&'a mut dyn FnMut(&mut Ui, &ImageRef<'_>)>,
    /// Block-height memory. Without it, every block is drawn every pass.
    pub cache: Option<&'a mut BlockCache>,
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
    /// Destination of any link clicked this frame, in body text or in a table
    /// cell — reported verbatim so a caller can act on its own link scheme
    /// (spec 044: the External Crates results table uses `crate:<name>` links
    /// to pick a crate). In-document anchors still resolve through
    /// [`Self::clicked_heading`]; a caller that only reads that field is
    /// unaffected.
    pub clicked_link: Option<String>,
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
        Self {
            bold: false,
            italic: false,
            code: false,
            link: false,
        }
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
    render_with(
        ui,
        markdown,
        opts,
        Extras {
            mermaid,
            image: None,
            cache: None,
        },
    )
}

/// Render `markdown` into `ui` with images and/or viewport-limited drawing.
pub fn render_with(
    ui: &mut Ui,
    markdown: &str,
    opts: &RenderOpts,
    extras: Extras<'_>,
) -> RenderOutput {
    let Extras {
        mermaid,
        mut image,
        cache,
    } = extras;
    let mut r = Renderer::new(ui, opts, cache);
    let parser = Parser::new_ext(
        markdown,
        Options::ENABLE_TABLES | Options::ENABLE_STRIKETHROUGH | Options::ENABLE_TASKLISTS,
    );
    let events: Vec<Event> = parser.collect();
    let mut i = 0;
    while i < events.len() {
        i = r.event(ui, &events, i, mermaid, &mut image);
    }
    r.flush_block(ui);
    let drawn = r.drawn;
    let blocks = r.block_idx;
    if let Some(c) = r.cache.as_mut() {
        c.drawn = drawn;
        // A pass that ended early (a shorter document) must not leave the
        // heights of the longer one behind it.
        c.heights.truncate(blocks);
    }
    RenderOutput {
        heading_count: r.heading_idx,
        match_count: r.match_count,
        clicked_heading: r.clicked_heading,
        clicked_link: r.clicked_link,
        scroll_target_y: r.scroll_target_y,
    }
}

struct Runs {
    job: LayoutJob,
}

/// One table cell: its flattened text plus the link it carries, if any.
/// Cells used to be plain `String`, which silently dropped a link — a
/// `[name](target)` inside a table rendered as dead text. Keeping the
/// destination lets a caller act on a clicked row (spec 044).
struct Cell {
    text: String,
    link: Option<String>,
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
    clicked_link: Option<String>,
    table_layout: TableLayout,
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
    /// Remembered block heights, when the caller supplied a cache.
    cache: Option<&'a mut BlockCache>,
    /// Screen-space band that is drawn for real: the viewport, grown by the
    /// cache's read-ahead margin.
    live: egui::Rect,
    /// Index of the next block, in document order.
    block_idx: usize,
    /// Blocks drawn for real in this pass.
    drawn: usize,
    /// The block being accumulated will be reserved rather than drawn, so its
    /// layout job is never built.
    reserving: bool,
    /// Which block [`Renderer::reserving`] was decided for.
    ///
    /// Two decisions are taken about the same block — one when it starts, so
    /// its layout job need not be built, and one at the flush, which places it
    /// — and they must agree. If the first said reserve and the second said
    /// draw, an empty block would be drawn and its height recorded as nothing,
    /// shortening the document silently.
    ///
    /// Today they cannot disagree: the decision is re-taken at every event
    /// until a block actually begins accumulating, which is the same cursor
    /// position the flush sees. `the_reserve_boundary_never_lands_inside_a_block`
    /// passes with this field ignored, so this is a guard rather than a fix —
    /// it costs one comparison and makes the agreement structural instead of a
    /// consequence of where the spacing calls happen to sit.
    reserve_for: Option<usize>,
}

/// A block being placed: where it starts, and the height it may stand in for.
struct Block {
    /// Index in document order — the slot its height is remembered in.
    idx: usize,
    /// Screen-space Y the block starts at.
    y0: f32,
    /// The remembered height, when this block is far enough off screen to be
    /// reserved rather than drawn.
    skip: Option<f32>,
}

impl<'a> Renderer<'a> {
    fn new(ui: &Ui, opts: &'a RenderOpts, cache: Option<&'a mut BlockCache>) -> Self {
        let v = ui.visuals();
        let read_ahead = cache.as_ref().map(|c| c.read_ahead).unwrap_or(0.0);
        let live = ui.clip_rect().expand2(egui::vec2(0.0, read_ahead));
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
            clicked_link: None,
            table_layout: opts.table_layout,
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
            cache,
            live,
            block_idx: 0,
            drawn: 0,
            reserving: false,
            reserve_for: None,
        }
    }

    /// Would the block about to start be reserved rather than drawn?
    ///
    /// Asked before a single character of it is turned into a layout job — the
    /// building is most of the cost, and a reserved block never shows any of it.
    fn will_reserve(&self, ui: &Ui) -> bool {
        let y0 = ui.cursor().top();
        self.cache
            .as_ref()
            .and_then(|c| c.heights.get(self.block_idx).copied())
            .filter(|h| *h > 0.0)
            .is_some_and(|h| y0 + h < self.live.top() || y0 > self.live.bottom())
    }

    /// Tally the search matches in `text` without building anything, for a
    /// block that will be reserved. The count has to be right whether or not
    /// the block is drawn — it is what the match navigator counts through.
    fn count_matches(&mut self, text: &str) {
        if self.search.is_empty() {
            return;
        }
        let hay = text.to_lowercase();
        let mut start = 0usize;
        while let Some(rel) = hay[start..].find(self.search) {
            if self.active_match == Some(self.match_count) && self.scroll_to_active {
                self.scroll_block = true;
            }
            self.match_count += 1;
            start += rel + self.search.len();
        }
    }

    /// Claim the next block slot. `Block::skip` carries the remembered height
    /// when this block is far enough outside the live band to be reserved
    /// rather than laid out again.
    fn begin_block(&mut self, ui: &Ui) -> Block {
        let idx = self.block_idx;
        self.block_idx += 1;
        let y0 = ui.cursor().top();
        let height = self
            .cache
            .as_ref()
            .and_then(|c| c.heights.get(idx).copied())
            // A block never drawn yet has no height to stand in for it, and a
            // zero-height one costs nothing to draw.
            .filter(|h| *h > 0.0);
        // Consume any decision already taken for this block.
        let decided = (self.reserve_for.take() == Some(idx)).then_some(self.reserving);
        self.reserving = false;
        let skip = match decided {
            Some(true) => height,
            Some(false) => None,
            None => height.filter(|h| y0 + h < self.live.top() || y0 > self.live.bottom()),
        };
        if skip.is_none() {
            self.drawn += 1;
        }
        Block { idx, y0, skip }
    }

    /// Record what the block just drawn actually measured.
    fn end_block(&mut self, ui: &Ui, b: Block) {
        let h = (ui.cursor().top() - b.y0).max(0.0);
        if let Some(c) = self.cache.as_mut() {
            if c.heights.len() <= b.idx {
                c.heights.resize(b.idx + 1, 0.0);
            }
            c.heights[b.idx] = h;
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
            self.runs = Some(Runs {
                job: LayoutJob::default(),
            });
        }
        if self.reserving {
            // Nothing of this block will be seen. The empty `Runs` above still
            // stands for it, so it claims its slot and reserves its height.
            self.count_matches(text);
            return;
        }
        let (font, color) = self.font_for(self.inline, self.heading);
        let mut fmt = TextFormat {
            font_id: font,
            color,
            ..Default::default()
        };
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
        self.anchors
            .iter()
            .find(|(s, _)| *s == slug)
            .map(|(_, i)| *i)
    }

    /// Draw the accumulated text block as one wrapped label.
    fn flush_block(&mut self, ui: &mut Ui) {
        // A block containing links is drawn as a sequence of inline widgets.
        if !self.segs.is_empty() || self.link_target.is_some() {
            self.flush_segs(ui);
            return;
        }
        let Some(mut runs) = self.runs.take() else {
            return;
        };
        runs.job.wrap.max_width = ui.available_width();
        let heading = self.heading.take();

        let indent = self.quote_depth as f32 * 14.0;
        let scroll_match = std::mem::replace(&mut self.scroll_block, false);
        let b = self.begin_block(ui);
        if let Some(h) = b.skip {
            // Reserved, not drawn — but a jump to a heading or a match that
            // lands here still needs its position, and this is exactly it.
            let wanted = scroll_match
                || (heading.is_some() && self.scroll_to_heading == Some(self.heading_idx));
            if wanted {
                self.scroll_target_y = Some(b.y0);
            }
            if heading.is_some() {
                self.heading_idx += 1;
            }
            ui.add_space(h);
            return;
        }
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
        self.end_block(ui, b);
    }

    /// Draw a block that contains links: text runs as labels, links as clickable
    /// widgets that jump to their target heading (the table of contents).
    fn flush_segs(&mut self, ui: &mut Ui) {
        self.push_run_seg();
        let segs = std::mem::take(&mut self.segs);
        let heading = self.heading.take();
        let indent = self.quote_depth as f32 * 14.0;
        let scroll_match = std::mem::replace(&mut self.scroll_block, false);

        let b = self.begin_block(ui);
        if let Some(h) = b.skip {
            let wanted = scroll_match
                || (heading.is_some() && self.scroll_to_heading == Some(self.heading_idx));
            if wanted {
                self.scroll_target_y = Some(b.y0);
            }
            if heading.is_some() {
                self.heading_idx += 1;
            }
            ui.add_space(h);
            return;
        }
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
                            self.clicked_link = Some(target);
                        }
                    }
                }
            }
        });
        if scroll_match {
            self.scroll_target_y = Some(inner.response.rect.top());
        }
        if heading.is_some() {
            // A heading that carries a link is still an outline entry, and the
            // outline must be able to jump to it.
            if self.scroll_to_heading == Some(self.heading_idx) {
                self.scroll_target_y = Some(inner.response.rect.top());
            }
            self.heading_idx += 1;
            ui.add_space(self.base * 0.25);
        }
        self.end_block(ui, b);
    }

    fn event(
        &mut self,
        ui: &mut Ui,
        events: &[Event],
        i: usize,
        mermaid: &mut dyn FnMut(&mut Ui, &str),
        image: &mut Option<&mut dyn FnMut(&mut Ui, &ImageRef<'_>)>,
    ) -> usize {
        // Nothing is being accumulated, so whatever this event opens is the
        // start of the next block: settle now whether it is worth building.
        if self.runs.is_none() && self.segs.is_empty() && self.link_target.is_none() {
            self.reserving = self.will_reserve(ui);
            self.reserve_for = Some(self.block_idx);
        }
        match &events[i] {
            Event::Start(tag) => self.start(ui, tag.clone(), events, i, mermaid, image),
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
                let b = self.begin_block(ui);
                match b.skip {
                    Some(h) => ui.add_space(h),
                    None => {
                        ui.separator();
                        self.end_block(ui, b);
                    }
                }
                i + 1
            }
            Event::TaskListMarker(done) => {
                self.push_text(if *done { "☑ " } else { "☐ " });
                i + 1
            }
            // The guide places its screenshots as raw HTML —
            // `<p align="center"><img src=… width=…></p>` — which arrives here
            // as one HTML event per block. Anything else in the block (the
            // `<p>` wrapper, comments) is not rendered.
            Event::Html(html) | Event::InlineHtml(html) => {
                for img in html_images(html) {
                    self.draw_image(ui, &img, image);
                }
                i + 1
            }
            _ => i + 1,
        }
    }

    /// Draw one embedded image through the caller's closure, or its alt text
    /// when the caller supplied none.
    fn draw_image(
        &mut self,
        ui: &mut Ui,
        img: &ImageRef<'_>,
        image: &mut Option<&mut dyn FnMut(&mut Ui, &ImageRef<'_>)>,
    ) {
        self.flush_block(ui);
        let Some(draw) = image else {
            if !img.alt.is_empty() {
                self.push_text(img.alt);
                self.flush_block(ui);
            }
            return;
        };
        let b = self.begin_block(ui);
        match b.skip {
            Some(h) => ui.add_space(h),
            None => {
                draw(ui, img);
                self.end_block(ui, b);
            }
        }
    }

    fn start(
        &mut self,
        ui: &mut Ui,
        tag: Tag,
        events: &[Event],
        i: usize,
        mermaid: &mut dyn FnMut(&mut Ui, &str),
        image: &mut Option<&mut dyn FnMut(&mut Ui, &ImageRef<'_>)>,
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
                self.runs = Some(Runs {
                    job: LayoutJob::default(),
                });
                // A list item is a block of its own, and `flush_block` has
                // already run, so this is where its fate is decided.
                self.reserving = self.will_reserve(ui);
                self.reserve_for = Some(self.block_idx);
                if self.reserving {
                    return i + 1;
                }
                // Prepend the marker as dim text.
                let (font, _c) = self.font_for(Inline::none(), None);
                self.runs.as_mut().unwrap().job.append(
                    &format!("{:width$}{marker}", "", width = (indent / 6.0) as usize),
                    0.0,
                    TextFormat {
                        font_id: font,
                        color: self.dim_color,
                        ..Default::default()
                    },
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
                let b = self.begin_block(ui);
                match b.skip {
                    Some(h) => ui.add_space(h),
                    None => {
                        if lang.eq_ignore_ascii_case("mermaid") {
                            mermaid(ui, &code);
                        } else {
                            self.draw_code_box(ui, &code, &lang);
                        }
                        self.end_block(ui, b);
                    }
                }
                j + 1 // skip past End(CodeBlock)
            }
            Tag::Table(_) => {
                self.flush_block(ui);
                self.draw_table(ui, events, i)
            }
            Tag::Image { dest_url, .. } => {
                // `![alt](src)`. The alt text lives in the events up to
                // `End(Image)`; it is the image's fallback, not body text, so
                // it is collected here rather than pushed into the block.
                let mut alt = String::new();
                let mut j = i + 1;
                while j < events.len() {
                    match &events[j] {
                        Event::Text(t) | Event::Code(t) => alt.push_str(t),
                        Event::End(TagEnd::Image) => break,
                        _ => {}
                    }
                    j += 1;
                }
                self.draw_image(
                    ui,
                    &ImageRef {
                        src: &dest_url,
                        alt: &alt,
                        width: None,
                    },
                    image,
                );
                j + 1 // skip past End(Image)
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
            editor::mono_layout_job(code, FontId::monospace(self.base * 0.95), self.body_color)
        };
        ui.add_space(self.base * 0.3);
        egui::Frame::NONE
            .fill(self.code_bg)
            .inner_margin(egui::Margin::same(8))
            .corner_radius(egui::CornerRadius::same(5))
            .stroke(Stroke::new(
                1.0,
                ui.visuals().widgets.noninteractive.bg_stroke.color,
            ))
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
        // Claimed before the cells are built, not after: a table off screen is
        // reserved without its rows ever being assembled.
        let b = self.begin_block(ui);
        if let Some(h) = b.skip {
            ui.add_space(h);
            return table_end(events, start);
        }
        // Collect rows of cells (each cell is the concatenated text).
        let mut rows: Vec<Vec<Cell>> = Vec::new();
        let mut cur_row: Vec<Cell> = Vec::new();
        let mut cur_cell = String::new();
        let mut cur_link: Option<String> = None;
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
                    cur_row.push(Cell {
                        text: std::mem::take(&mut cur_cell),
                        link: cur_link.take(),
                    });
                }
                // The first link in a cell owns the whole cell — a results
                // table has one target per row, not a paragraph of prose.
                Event::Start(Tag::Link { dest_url, .. }) => {
                    if cur_link.is_none() {
                        cur_link = Some(dest_url.to_string());
                    }
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
            self.end_block(ui, b);
            return j;
        }
        if self.table_layout == TableLayout::TightResizable {
            self.draw_table_tight(ui, &rows, cols, start);
            self.end_block(ui, b);
            return j;
        }
        let col_w = (ui.available_width() / cols as f32 - 8.0).max(60.0);
        egui::Frame::NONE
            .stroke(Stroke::new(
                1.0,
                ui.visuals().widgets.noninteractive.bg_stroke.color,
            ))
            .inner_margin(egui::Margin::same(2))
            .show(ui, |ui| {
                egui::Grid::new(("md_table", start))
                    .striped(true)
                    .min_col_width(col_w)
                    .max_col_width(col_w)
                    .show(ui, |ui| {
                        for (ri, row) in rows.iter().enumerate() {
                            for c in 0..cols {
                                let cell = row.get(c);
                                let text = cell.map(|c| c.text.as_str()).unwrap_or("");
                                let mut rt = RichText::new(text).size(self.base);
                                if ri == 0 {
                                    rt = rt.strong();
                                }
                                // A header row is never a link, whatever the
                                // source says — the header labels the column.
                                match cell.and_then(|c| c.link.as_ref()).filter(|_| ri > 0) {
                                    Some(target) => {
                                        if ui.link(rt).clicked() {
                                            self.clicked_link = Some(target.clone());
                                        }
                                    }
                                    None => {
                                        ui.add(egui::Label::new(rt).wrap());
                                    }
                                }
                            }
                            ui.end_row();
                        }
                    });
            });
        ui.add_space(self.base * 0.4);
        self.end_block(ui, b);
        j
    }

    /// [`TableLayout::TightResizable`]: every column but the last is sized to
    /// its own widest cell and never wraps; the last takes the width that is
    /// left and is the only one that wraps. `egui_extras` supplies the
    /// user-draggable boundaries and paints a line at each one.
    ///
    /// Row heights have to be known before the body is built, and the last
    /// column's real width is only known while drawing it — so each frame
    /// records that width and the next frame measures against it. The lag is
    /// one frame after a drag, and it corrects itself.
    fn draw_table_tight(&mut self, ui: &mut Ui, rows: &[Vec<Cell>], cols: usize, start: usize) {
        use egui_extras::{Column, TableBuilder};

        let font = egui::FontId::proportional(self.base);
        let measure = |ui: &Ui, text: &str| -> f32 {
            ui.fonts_mut(|f| {
                f.layout_no_wrap(text.to_owned(), font.clone(), Color32::WHITE)
                    .rect
                    .width()
            })
        };
        // Padding so a tight column does not touch its neighbour's line.
        let pad = self.base * 0.9;

        // Natural width of every column except the last.
        let fixed = cols.saturating_sub(1);
        let mut widths = vec![0.0_f32; fixed];
        for row in rows {
            for (c, w) in widths.iter_mut().enumerate() {
                let text = row.get(c).map(|x| x.text.as_str()).unwrap_or("");
                *w = w.max(measure(ui, text) + pad);
            }
        }

        // The last column's width, as measured while drawing the previous
        // frame; the first frame estimates it from what is left over.
        let last_w_id = ui.make_persistent_id(("md_tight_last_w", start));
        let spacing = ui.spacing().item_spacing.x;
        let estimate =
            (ui.available_width() - widths.iter().sum::<f32>() - spacing * cols as f32).max(120.0);
        let last_w: f32 = ui
            .data(|d| d.get_temp(last_w_id))
            .unwrap_or(estimate)
            .max(60.0);

        let line_h = ui.fonts_mut(|f| f.row_height(&font));
        let row_h = |ui: &Ui, row: &Vec<Cell>| -> f32 {
            let text = row.get(cols - 1).map(|x| x.text.as_str()).unwrap_or("");
            let wrapped = ui.fonts_mut(|f| {
                f.layout(text.to_owned(), font.clone(), Color32::WHITE, last_w)
                    .rect
                    .height()
            });
            wrapped.max(line_h) + self.base * 0.5
        };

        let header = rows.first();
        let body_rows = &rows[header.map(|_| 1).unwrap_or(0)..];
        let heights: Vec<f32> = body_rows.iter().map(|r| row_h(ui, r)).collect();
        let header_h = line_h + self.base * 0.5;

        // Drawn inside the closures, read back after the table.
        let mut clicked: Option<String> = None;
        let mut measured_last_w: Option<f32> = None;

        let mut builder = TableBuilder::new(ui)
            .id_salt(("md_tight", start))
            .striped(true)
            .resizable(true)
            .vscroll(false)
            .auto_shrink([false, false])
            .cell_layout(egui::Layout::left_to_right(egui::Align::TOP));
        for w in &widths {
            builder = builder.column(Column::initial(*w).at_least(28.0).clip(true).resizable(true));
        }
        builder = builder.column(Column::remainder().at_least(80.0));

        let draw_cell = |ui: &mut Ui,
                         cell: Option<&Cell>,
                         is_last: bool,
                         strong: bool,
                         clicked: &mut Option<String>,
                         measured: &mut Option<f32>| {
            if is_last {
                *measured = Some(ui.available_width());
            }
            let text = cell.map(|c| c.text.as_str()).unwrap_or("");
            let mut rt = RichText::new(text).size(self.base);
            if strong {
                rt = rt.strong();
            }
            // Only the last column wraps; the others stay on one line so the
            // table reads as a grid of values.
            match cell.and_then(|c| c.link.as_ref()).filter(|_| !strong) {
                Some(target) => {
                    if ui.link(rt).clicked() {
                        *clicked = Some(target.clone());
                    }
                }
                None => {
                    let label = if is_last {
                        egui::Label::new(rt).wrap()
                    } else {
                        egui::Label::new(rt).extend()
                    };
                    ui.add(label);
                }
            }
        };

        builder
            .header(header_h, |mut hrow| {
                for c in 0..cols {
                    hrow.col(|ui| {
                        draw_cell(
                            ui,
                            header.and_then(|r| r.get(c)),
                            c + 1 == cols,
                            true,
                            &mut clicked,
                            &mut measured_last_w,
                        );
                    });
                }
            })
            .body(|body| {
                body.heterogeneous_rows(heights.into_iter(), |mut brow| {
                    let idx = brow.index();
                    for c in 0..cols {
                        brow.col(|ui| {
                            draw_cell(
                                ui,
                                body_rows.get(idx).and_then(|r| r.get(c)),
                                c + 1 == cols,
                                false,
                                &mut clicked,
                                &mut measured_last_w,
                            );
                        });
                    }
                });
            });

        if let Some(w) = measured_last_w {
            ui.data_mut(|d| d.insert_temp(last_w_id, w));
        }
        if clicked.is_some() {
            self.clicked_link = clicked;
        }
        ui.add_space(self.base * 0.4);
    }
}

/// Index just past a table's `End` event, without touching its cells — what a
/// reserved table needs and all it needs.
fn table_end(events: &[Event], start: usize) -> usize {
    let mut depth = 1; // already inside Table
    let mut j = start + 1;
    while j < events.len() && depth > 0 {
        match &events[j] {
            Event::Start(Tag::Table(_)) => depth += 1,
            Event::End(TagEnd::Table) => depth -= 1,
            _ => {}
        }
        j += 1;
    }
    j
}

/// Pull every `<img>` out of a raw-HTML block.
///
/// Deliberately a scan for one tag rather than an HTML parser: the guide's
/// screenshots are written as `<p align="center"><img src=… alt=… width=…></p>`
/// and nothing else in the documentation needs HTML. Attribute values may be
/// quoted with `"` or `'`.
fn html_images(html: &str) -> Vec<ImageRef<'_>> {
    let mut out = Vec::new();
    let mut rest = html;
    while let Some(at) = rest.find("<img") {
        let after = &rest[at + 4..];
        // The tag ends at the first `>`; a malformed tag ends the scan.
        let Some(end) = after.find('>') else { break };
        let tag = &after[..end];
        if let Some(src) = html_attr(tag, "src") {
            out.push(ImageRef {
                src,
                alt: html_attr(tag, "alt").unwrap_or(""),
                width: html_attr(tag, "width").and_then(|w| w.trim().parse::<f32>().ok()),
            });
        }
        rest = &after[end + 1..];
    }
    out
}

/// Value of `name="…"` (or `name='…'`) inside one tag's attribute text.
fn html_attr<'a>(tag: &'a str, name: &str) -> Option<&'a str> {
    let mut rest = tag;
    loop {
        let at = rest.find(name)?;
        let before_ok = at == 0
            || rest[..at]
                .chars()
                .next_back()
                .is_some_and(|c| c.is_whitespace());
        let after = &rest[at + name.len()..];
        let trimmed = after.trim_start();
        // `width=` must not match inside `data-width=`, and `src=` must be
        // followed by its `=` rather than being the prefix of another name.
        if before_ok && trimmed.starts_with('=') {
            let value = trimmed[1..].trim_start();
            let quote = value.chars().next()?;
            if quote == '"' || quote == '\'' {
                let inner = &value[1..];
                let end = inner.find(quote)?;
                return Some(&inner[..end]);
            }
            // Unquoted: up to the next whitespace.
            let end = value.find(char::is_whitespace).unwrap_or(value.len());
            return Some(&value[..end]);
        }
        rest = after;
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

#[cfg(test)]
mod tests {
    use super::*;

    /// Render `markdown` for `frames` frames inside a scroll area of
    /// `viewport` points, with a block cache, and report what the last pass
    /// walked and what it actually drew.
    fn cached_pass(markdown: &str, viewport: f32, frames: usize) -> (usize, usize) {
        let mut cache = BlockCache::new(0.0); // no read-ahead: measure the band itself
        run_frames(markdown, viewport, frames, Some(&mut cache));
        (cache.blocks(), cache.drawn())
    }

    /// The same, with no cache at all — what every frame used to cost.
    fn uncached_pass(markdown: &str, viewport: f32, frames: usize) {
        run_frames(markdown, viewport, frames, None);
    }

    fn run_frames(markdown: &str, viewport: f32, frames: usize, mut cache: Option<&mut BlockCache>) {
        let ctx = egui::Context::default();
        for _ in 0..frames {
            let input = egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(900.0, viewport),
                )),
                ..Default::default()
            };
            ctx.run_ui(input, |root_ui| {
                egui::CentralPanel::default().show(root_ui, |ui| {
                    egui::ScrollArea::vertical().show(ui, |ui| {
                        render_with(
                            ui,
                            markdown,
                            &RenderOpts {
                                base: 14.0,
                                search: "",
                                scroll_to_heading: None,
                                active_match: None,
                                scroll_to_active: false,
                                anchors: &[],
                                table_layout: TableLayout::Equal,
                            },
                            Extras {
                                mermaid: &mut |_, _| {},
                                image: None,
                                cache: cache.as_deref_mut(),
                            },
                        );
                    });
                });
            })
            .textures_delta
            .clear();
        }
    }

    /// The point of the cache: a long document stops laying itself out in
    /// full on every frame. The first pass has no heights to stand in for
    /// anything, so it draws everything; the second draws only the band that
    /// is actually on screen.
    #[test]
    fn a_long_document_only_draws_what_is_on_screen() {
        let md: String = (0..400)
            .map(|i| format!("## Section {i}\n\nSome prose in section {i}.\n\n"))
            .collect();

        let (blocks_first, drawn_first) = cached_pass(&md, 400.0, 1);
        assert_eq!(
            blocks_first, drawn_first,
            "the first pass has no remembered heights, so it must draw every block"
        );
        assert!(blocks_first >= 800, "expected ~800 blocks, got {blocks_first}");

        let (blocks, drawn) = cached_pass(&md, 400.0, 2);
        assert_eq!(
            blocks, blocks_first,
            "the same document must walk the same number of blocks either way"
        );
        assert!(
            drawn * 10 < blocks,
            "a 400-point viewport over {blocks} blocks should draw a small \
             fraction of them, not {drawn}"
        );
    }

    /// Reserved space is the height the block really had, so the document is
    /// exactly as tall either way — otherwise the scrollbar would jump as the
    /// reader scrolled.
    #[test]
    fn reserving_a_block_keeps_the_document_the_same_height() {
        fn total_height(markdown: &str, cache: Option<&mut BlockCache>) -> f32 {
            let ctx = egui::Context::default();
            let mut height = 0.0;
            let mut cache = cache;
            let input = egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(900.0, 300.0),
                )),
                ..Default::default()
            };
            ctx.run_ui(input, |root_ui| {
                egui::CentralPanel::default().show(root_ui, |ui| {
                    let out = egui::ScrollArea::vertical().show(ui, |ui| {
                        render_with(
                            ui,
                            markdown,
                            &RenderOpts {
                                base: 14.0,
                                search: "",
                                scroll_to_heading: None,
                                active_match: None,
                                scroll_to_active: false,
                                anchors: &[],
                                table_layout: TableLayout::Equal,
                            },
                            Extras {
                                mermaid: &mut |_, _| {},
                                image: None,
                                cache: cache.as_deref_mut(),
                            },
                        );
                    });
                    height = out.content_size.y;
                });
            })
            .textures_delta
            .clear();
            height
        }

        let md: String = (0..120)
            .map(|i| format!("## Section {i}\n\nProse for section {i}.\n\n"))
            .collect();

        let uncached = total_height(&md, None);
        let mut cache = BlockCache::new(0.0);
        total_height(&md, Some(&mut cache)); // first pass measures
        let cached = total_height(&md, Some(&mut cache)); // second reserves
        assert!(
            cache.drawn() < cache.blocks(),
            "the second pass must have reserved something to be a real test"
        );
        assert!(
            (uncached - cached).abs() < 1.0,
            "reserving changed the document height: {uncached} vs {cached}"
        );
    }

    /// The decision to reserve a block is taken when the block *starts*, so its
    /// layout job is never built — but a heading, a list and a quote all add
    /// spacing between that moment and the flush. If the flush took its own
    /// decision from the moved position it could disagree and draw a block
    /// whose text was never assembled: an empty block, recorded as no height,
    /// silently shortening the document.
    ///
    /// Sweeping the viewport height walks that boundary across every kind of
    /// block in turn; the document must measure the same every time.
    #[test]
    fn the_reserve_boundary_never_lands_inside_a_block() {
        let md: String = (0..60)
            .map(|i| {
                format!(
                    "## Heading {i}\n\nProse {i} with a [link](#heading-{i}) in it.\n\n\
                     - item {i} one\n- item {i} two\n\n> quoted {i}\n\n\
                     | a | b |\n|---|---|\n| {i} | x |\n\n```\ncode {i}\n```\n\n"
                )
            })
            .collect();

        let expected = document_height(&md, 400.0, None);
        for step in 0..24 {
            let viewport = 180.0 + step as f32 * 31.0;
            let mut cache = BlockCache::new(0.0);
            // Three passes: measure, reserve, and reserve again from the
            // heights the reserving pass recorded.
            document_height(&md, viewport, Some(&mut cache));
            document_height(&md, viewport, Some(&mut cache));
            let settled = document_height(&md, viewport, Some(&mut cache));
            assert!(
                (settled - expected).abs() < 1.0,
                "viewport {viewport}: document measured {settled}, not {expected} \
                 — a block was reserved and drawn out of step"
            );
            assert!(
                cache.drawn() < cache.blocks(),
                "viewport {viewport}: nothing was reserved, so this proves nothing"
            );
        }
    }

    /// Height of the whole document, as the scroll area measures it.
    fn document_height(markdown: &str, viewport: f32, cache: Option<&mut BlockCache>) -> f32 {
        let ctx = egui::Context::default();
        let mut height = 0.0;
        let mut cache = cache;
        let input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(900.0, viewport),
            )),
            ..Default::default()
        };
        ctx.run_ui(input, |root_ui| {
            egui::CentralPanel::default().show(root_ui, |ui| {
                let out = egui::ScrollArea::vertical().show(ui, |ui| {
                    render_with(
                        ui,
                        markdown,
                        &RenderOpts {
                            base: 14.0,
                            search: "",
                            scroll_to_heading: None,
                            active_match: None,
                            scroll_to_active: false,
                            anchors: &[],
                            table_layout: TableLayout::Equal,
                        },
                        Extras {
                            mermaid: &mut |_, _| {},
                            image: None,
                            cache: cache.as_deref_mut(),
                        },
                    );
                });
                height = out.content_size.y;
            });
        })
        .textures_delta
        .clear();
        height
    }

    /// The document this was all for. The Developer's Guide is the largest
    /// thing the viewer opens, and the guard is on the *fraction* drawn, not on
    /// a wall-clock number that would differ on every machine — but the pass
    /// times are printed (`--nocapture`) so the cost can be read off directly.
    #[test]
    fn the_developers_guide_settles_to_a_fraction_of_itself() {
        let docs = crate::docs_embed::doc_list(crate::i18n::Language::English);
        let guide = docs
            .iter()
            .find(|d| d.id.starts_with("developers-guide"))
            .expect("the guide ships");

        // Warm the galley cache the same way for both, so the comparison is
        // of the work done and not of a cold font atlas.
        uncached_pass(&guide.source, 700.0, 1);
        let t0 = std::time::Instant::now();
        uncached_pass(&guide.source, 700.0, 4);
        let before = t0.elapsed() / 4;

        let (blocks, drawn) = cached_pass(&guide.source, 700.0, 1);
        let t1 = std::time::Instant::now();
        let (_, drawn_steady) = cached_pass(&guide.source, 700.0, 5);
        let after = t1.elapsed() / 5;

        eprintln!(
            "developers guide: {} KB, {blocks} blocks\n  \
             every block, every frame : {before:?}\n  \
             only what is on screen   : {after:?}  ({drawn} of {blocks} blocks drawn)",
            guide.source.len() / 1024,
        );

        assert!(
            drawn * 20 < blocks,
            "a screenful of a {blocks}-block guide should be a few dozen blocks, not {drawn}"
        );
        assert_eq!(
            drawn, drawn_steady,
            "the drawn band must be stable from frame to frame"
        );
    }

    /// A screenshot written as raw HTML reaches the image callback with its
    /// source, its alt text and the width the document asked for.
    #[test]
    fn an_html_screenshot_reaches_the_image_callback() {
        let md = concat!(
            "Before.\n\n",
            r#"<p align="center"><img src="../assets/images/screenshots/welcome.png" alt="The welcome screen" width="900"></p>"#,
            "\n\nAfter.\n",
        );
        let ctx = egui::Context::default();
        let mut seen: Vec<(String, String, Option<f32>)> = Vec::new();
        let input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(900.0, 700.0),
            )),
            ..Default::default()
        };
        ctx.run_ui(input, |root_ui| {
            egui::CentralPanel::default().show(root_ui, |ui| {
                render_with(
                    ui,
                    md,
                    &RenderOpts {
                        base: 14.0,
                        search: "",
                        scroll_to_heading: None,
                        active_match: None,
                        scroll_to_active: false,
                        anchors: &[],
                        table_layout: TableLayout::Equal,
                    },
                    Extras {
                        mermaid: &mut |_, _| {},
                        image: Some(&mut |_ui: &mut Ui, img: &ImageRef<'_>| {
                            seen.push((img.src.to_owned(), img.alt.to_owned(), img.width));
                        }),
                        cache: None,
                    },
                );
            });
        })
        .textures_delta
        .clear();

        assert_eq!(
            seen,
            vec![(
                "../assets/images/screenshots/welcome.png".to_string(),
                "The welcome screen".to_string(),
                Some(900.0)
            )],
            "the guide's screenshots must reach the caller that can load them"
        );
    }

    /// Markdown's own image syntax goes to the same place, and its alt text is
    /// the image's fallback rather than a stray paragraph of body text.
    #[test]
    fn a_markdown_image_reaches_the_callback_with_its_alt() {
        let ctx = egui::Context::default();
        let mut seen: Vec<(String, String)> = Vec::new();
        let input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(900.0, 700.0),
            )),
            ..Default::default()
        };
        ctx.run_ui(input, |root_ui| {
            egui::CentralPanel::default().show(root_ui, |ui| {
                render_with(
                    ui,
                    "![a diagram](pic.png)\n",
                    &RenderOpts {
                        base: 14.0,
                        search: "",
                        scroll_to_heading: None,
                        active_match: None,
                        scroll_to_active: false,
                        anchors: &[],
                        table_layout: TableLayout::Equal,
                    },
                    Extras {
                        mermaid: &mut |_, _| {},
                        image: Some(&mut |_ui: &mut Ui, img: &ImageRef<'_>| {
                            seen.push((img.src.to_owned(), img.alt.to_owned()));
                        }),
                        cache: None,
                    },
                );
            });
        })
        .textures_delta
        .clear();

        assert_eq!(seen, vec![("pic.png".to_string(), "a diagram".to_string())]);
    }

    /// Every string this markdown actually paints, with how many wrapped
    /// rows each one occupies — the ground truth for "did that cell render
    /// at all" and "how tightly is it wrapped".
    fn painted_text(markdown: &str, layout: TableLayout) -> Vec<(String, usize)> {
        fn collect(shape: &egui::Shape, out: &mut Vec<(String, usize)>) {
            match shape {
                egui::Shape::Text(t) => {
                    out.push((t.galley.text().to_owned(), t.galley.rows.len()))
                }
                egui::Shape::Vec(v) => v.iter().for_each(|s| collect(s, out)),
                _ => {}
            }
        }
        let ctx = egui::Context::default();
        let mut painted = Vec::new();
        // Three frames: the tight layout measures the last column while
        // drawing it and uses that width on the next frame.
        for _ in 0..3 {
            let input = egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(900.0, 700.0),
                )),
                ..Default::default()
            };
            let mut full = ctx.run_ui(input, |root_ui| {
                egui::CentralPanel::default().show(root_ui, |ui| {
                    render(
                        ui,
                        markdown,
                        &RenderOpts {
                            base: 14.0,
                            search: "",
                            scroll_to_heading: None,
                            active_match: None,
                            scroll_to_active: false,
                            anchors: &[],
                            table_layout: layout,
                        },
                        &mut |_, _| {},
                    );
                });
            });
            painted.clear();
            for clipped in &full.shapes {
                collect(&clipped.shape, &mut painted);
            }
            // epaint panics if a texture delta is dropped unhandled.
            full.textures_delta.clear();
        }
        painted
    }

    /// Spec 044 — the operator's screenshot showed a results row whose
    /// crate, version and downloads cells were blank while its (long,
    /// wrapping) description rendered. Every cell of every row must paint,
    /// however tall the last column's text makes the row.
    #[test]
    fn no_row_loses_its_cells_to_a_tall_description() {
        let long = "Elusion is a modern DataFrame / Data Engineering / Data Analysis \
                    library that combines the familiarity of DataFrame operations with \
                    the power of SQL query building, over many more words than fit on \
                    one line at any sane column width, which is the case that broke it.";
        let md = format!(
            "| Crate | Version | Downloads | Description |\n|---|---|---|---|\n\
             | [csv](crate:csv) | 1.4.0 | 221 | Fast CSV parsing. |\n\
             | [qsv](crate:qsv) | 16.1.0 | 243 | A data-wrangling toolkit. |\n\
             | [elusion](crate:elusion) | 8.3.0 | 24 | {long} |\n\
             | [xan](crate:xan) | 0.60.0 | 12 | The CSV magician |\n"
        );
        let painted = painted_text(&md, TableLayout::TightResizable);
        for needed in [
            "csv", "1.4.0", "221", "qsv", "16.1.0", "243", "elusion", "8.3.0", "24", "xan",
            "0.60.0", "12",
        ] {
            assert!(
                painted.iter().any(|(t, _)| t == needed),
                "`{needed}` was never painted — a row lost a cell. Painted: {painted:?}"
            );
        }
    }

    /// The tight layout gives the value columns only what they need, so the
    /// description gets the rest: the same table must wrap the description
    /// into fewer lines than the equal-width layout does.
    #[test]
    fn tight_columns_give_the_description_more_room() {
        let md = "| Crate | Version | Downloads | Description |\n|---|---|---|---|\n\
                  | csv | 1.4.0 | 221530986 | Fast CSV parsing with support for serde \
                  and a description long enough to wrap in a narrow column. |\n";
        let lines = |layout| -> usize {
            painted_text(md, layout)
                .into_iter()
                .filter(|(t, _)| t.contains("Fast CSV parsing"))
                .map(|(_, rows)| rows)
                .max()
                .unwrap_or(0)
        };
        let equal = lines(TableLayout::Equal);
        let tight = lines(TableLayout::TightResizable);
        assert!(equal > 0 && tight > 0, "the description must paint in both");
        assert!(
            tight < equal,
            "tight columns must wrap the description less than equal ones \
             (tight {tight} lines vs equal {equal})"
        );
    }

    /// Render `markdown` for a few frames, clicking at `click` on the last
    /// one, and return the output of that frame.
    fn render_frames(markdown: &str, click: Option<egui::Pos2>) -> RenderOutput {
        let ctx = egui::Context::default();
        let mut last = RenderOutput::default();
        // Frame 1-2 lay out; frame 3 presses, frame 4 releases (a click needs
        // both, delivered where the widget already is).
        let scripts: Vec<Vec<egui::Event>> = vec![
            vec![],
            vec![],
            click
                .map(|p| {
                    vec![
                        egui::Event::PointerMoved(p),
                        egui::Event::PointerButton {
                            pos: p,
                            button: egui::PointerButton::Primary,
                            pressed: true,
                            modifiers: egui::Modifiers::default(),
                        },
                    ]
                })
                .unwrap_or_default(),
            click
                .map(|p| {
                    vec![egui::Event::PointerButton {
                        pos: p,
                        button: egui::PointerButton::Primary,
                        pressed: false,
                        modifiers: egui::Modifiers::default(),
                    }]
                })
                .unwrap_or_default(),
        ];
        for events in scripts {
            let input = egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(900.0, 700.0),
                )),
                events,
                ..Default::default()
            };
            ctx.run_ui(input, |root_ui| {
                egui::CentralPanel::default().show(root_ui, |ui| {
                    last = render(
                        ui,
                        markdown,
                        &RenderOpts {
                            base: 14.0,
                            search: "",
                            scroll_to_heading: None,
                            active_match: None,
                            scroll_to_active: false,
                            anchors: &[],
                            table_layout: TableLayout::Equal,
                        },
                        &mut |_, _| {},
                    );
                });
            })
            .textures_delta
            .clear();
        }
        last
    }

    /// Spec 044 — a link inside a table cell survives to the rendered cell
    /// and reports its destination when clicked. Before this, `draw_table`
    /// flattened cells to text and the link was silently dead.
    #[test]
    fn a_link_in_a_table_cell_is_clickable() {
        let md = "| Crate | Version |\n|---|---|\n| [csv](crate:csv) | 1.4.0 |\n";
        // Nothing clicked: no link reported.
        assert!(render_frames(md, None).clicked_link.is_none());

        // The link sits in the first body cell — just under the header row,
        // near the left edge. Probe a small grid of points inside that cell
        // rather than guessing one pixel.
        let mut found = None;
        'probe: for y in [40, 50, 60, 70, 80, 90] {
            for x in [16, 24, 32, 40] {
                let out = render_frames(md, Some(egui::pos2(x as f32, y as f32)));
                if let Some(target) = out.clicked_link {
                    found = Some(target);
                    break 'probe;
                }
            }
        }
        assert_eq!(
            found.as_deref(),
            Some("crate:csv"),
            "clicking the crate cell must report its link destination"
        );
    }

    /// The header row is a label, never a link, even if the source marks it
    /// as one — clicking a column title must not pick anything.
    #[test]
    fn a_header_cell_is_never_a_link() {
        let md = "| [Crate](crate:HEADER) | Version |\n|---|---|\n| csv | 1.4.0 |\n";
        for y in [20, 30, 40] {
            for x in [16, 24, 32, 40] {
                let out = render_frames(md, Some(egui::pos2(x as f32, y as f32)));
                assert_ne!(
                    out.clicked_link.as_deref(),
                    Some("crate:HEADER"),
                    "a header cell must not be clickable"
                );
            }
        }
    }
}
