// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! The Viewer's pure, `egui`-free model code (spec 058). This task's own
//! slice: format resolution (R3/R4), and plain-text loading, page-break
//! indexing and on-demand page decoding (R1, R2, R6, R9). `cobolt-forms` owns
//! no thread and no cache — see `cobolt-form-host::viewer_session` for both —
//! so everything here is a plain function or value type a test can call
//! directly, with no `Instant::now()` and no I/O beyond reading the document
//! itself.
//!
//! # Storage is native-format, always (plan.md §3)
//!
//! A document's canonical, retained representation is its own source
//! format — text stays text, Markdown stays Markdown text — never converted
//! into a shared intermediate. What later stages *display* (a Markdown
//! layout tree, wrapped lines) is computed from this at paint time and is
//! never a replacement for it. For plain text there is no derived tree at
//! all: the retained unit **is** the text.
//!
//! # Why indexing precedes decoding
//!
//! R2 forbids holding a whole document in memory. [`index_text`] makes one
//! sequential pass over the source to find page boundaries — explicit ones at
//! each form-feed ([`FORM_FEED`], the traditional plain-text page break, and
//! a natural fit for a COBOL-facing product whose own report `DISPLAY`
//! output already uses it), and a byte-size ceiling ([`MAX_TEXT_PAGE_BYTES`])
//! wherever a run between breaks (or the whole file) would otherwise grow
//! unbounded — so a single page never costs more to decode than that
//! ceiling, however large the source is. The pass itself never buffers more
//! than one page's worth of bytes at a time. Once indexed,
//! [`decode_text_page`] reads exactly one page's bytes — jumping to the LAST
//! page of a multi-gigabyte file costs one page's I/O, not a scan of
//! everything before it.
//!
//! Computed (font-metric-dependent) page breaks for `Print`/`Page` layout are
//! a *visual* re-flow of a page's text, not this module's job — that is
//! `paint::draw_viewer`'s (T11), which has the `egui` fonts context this
//! module deliberately does not depend on.

use std::fs::File;
use std::io::{self, Read, Seek, SeekFrom};
use std::sync::Arc;

use pulldown_cmark::{Alignment, CodeBlockKind, Event, Options, Parser, Tag, TagEnd};

// ── Format ───────────────────────────────────────────────────────────────

/// The five document formats the Viewer can resolve a source into (R0's
/// honest naming — `HtmlSubset`, never `Html`, since neither renders like a
/// browser; §3). Detecting a format and being able to *display* it are
/// different milestones: this enum exists in full from this task on, even
/// though only `Text` decodes until T9/T10/T18/T21 land the rest.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum ViewerFormat {
    Text,
    Markdown,
    Image,
    Pdf,
    HtmlSubset,
}

impl ViewerFormat {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Text => "Text",
            Self::Markdown => "Markdown",
            Self::Image => "Image",
            Self::Pdf => "Pdf",
            Self::HtmlSubset => "HtmlSubset",
        }
    }

    /// The `DataGridGridLineStyle` idiom (`model.rs:177-194`): lenient,
    /// case/whitespace-insensitive. This round-trips an already-resolved
    /// name (a test literal, a saved value) rather than sniffing content, so
    /// unlike [`detect_format`] it never needs to say "I don't know" — an
    /// unrecognized string falls back to `Text`, the same "just show the
    /// characters" fallback `detect_format` itself uses last.
    pub fn from_str(s: &str) -> Self {
        match s.trim().to_ascii_lowercase().as_str() {
            "markdown" | "md" => Self::Markdown,
            "image" => Self::Image,
            "pdf" => Self::Pdf,
            "htmlsubset" | "html" | "htm" => Self::HtmlSubset,
            _ => Self::Text,
        }
    }
}

impl std::fmt::Display for ViewerFormat {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Where a document's bytes actually live — a path to read on demand
/// (`Source`), or a buffer COBOL already holds in memory (`LoadBytes`, R1).
/// Cheap to clone either way: a path is a small string, and `Bytes` is
/// reference-counted rather than duplicated per page.
#[derive(Debug, Clone)]
pub enum DocumentSource {
    Path(String),
    Bytes(Arc<[u8]>),
}

/// R3: resolve a source's format from its content first, its path's
/// extension second. Returns `None` when neither signal matches anything
/// this control supports — the R4 trigger for `onError`.
///
/// `path_hint` is `None` for a `LoadBytes` document with no filename at all
/// — content is then the only signal available.
pub fn detect_format(path_hint: Option<&str>, bytes: &[u8]) -> Option<ViewerFormat> {
    if let Some(fmt) = sniff_content(bytes) {
        return Some(fmt);
    }
    if let Some(fmt) = path_hint.and_then(extension_of).and_then(|e| format_from_extension(&e)) {
        return Some(fmt);
    }
    // Neither content nor extension gave a match — the honest, permissive
    // last resort: printable text still opens as Text (a `.log`, a `.cfg`,
    // an extensionless README all deserve to display); anything else is
    // genuinely unsupported.
    if looks_like_text(bytes) {
        Some(ViewerFormat::Text)
    } else {
        None
    }
}

fn extension_of(path: &str) -> Option<String> {
    let name = path.rsplit(['/', '\\']).next().unwrap_or(path);
    let (base, ext) = name.rsplit_once('.')?;
    if base.is_empty() || ext.is_empty() {
        None
    } else {
        Some(ext.to_ascii_lowercase())
    }
}

fn format_from_extension(ext: &str) -> Option<ViewerFormat> {
    match ext {
        "txt" | "text" | "log" => Some(ViewerFormat::Text),
        "md" | "markdown" => Some(ViewerFormat::Markdown),
        "png" | "jpg" | "jpeg" | "gif" | "webp" | "bmp" | "tiff" | "tif" | "svg" => {
            Some(ViewerFormat::Image)
        }
        "pdf" => Some(ViewerFormat::Pdf),
        "html" | "htm" => Some(ViewerFormat::HtmlSubset),
        _ => None,
    }
}

/// Strong, unambiguous binary signatures first (a `.txt`-named PNG must
/// still resolve as an Image — R3's "content first"), then the text-only
/// formats a magic-byte table can't cover (HTML/SVG are just text with a
/// recognizable opening tag), then a conservative Markdown heuristic.
fn sniff_content(bytes: &[u8]) -> Option<ViewerFormat> {
    if bytes.starts_with(b"%PDF-") {
        return Some(ViewerFormat::Pdf);
    }
    let is_image_signature = bytes.starts_with(b"\x89PNG\r\n\x1a\n")
        || bytes.starts_with(b"\xFF\xD8\xFF")
        || bytes.starts_with(b"GIF87a")
        || bytes.starts_with(b"GIF89a")
        || bytes.starts_with(b"BM")
        || bytes.starts_with(b"II*\0")
        || bytes.starts_with(b"MM\0*")
        || (bytes.len() >= 12 && &bytes[0..4] == b"RIFF" && &bytes[8..12] == b"WEBP");
    if is_image_signature {
        return Some(ViewerFormat::Image);
    }

    let head = leading_text(bytes, 512)?;
    let probe = head.trim_start().to_ascii_lowercase();
    if probe.starts_with("<!doctype html") || probe.starts_with("<html") {
        return Some(ViewerFormat::HtmlSubset);
    }
    if probe.starts_with("<svg") || (probe.starts_with("<?xml") && probe.contains("<svg")) {
        return Some(ViewerFormat::Image);
    }
    if looks_like_markdown(&head) {
        return Some(ViewerFormat::Markdown);
    }
    None
}

/// The first `max_bytes` of `bytes`, decoded as UTF-8 text — `None` if even
/// that prefix isn't valid text (a binary file's opening bytes almost never
/// are), so the caller can tell "no textual signal" from "this isn't text."
fn leading_text(bytes: &[u8], max_bytes: usize) -> Option<String> {
    let head = &bytes[..bytes.len().min(max_bytes)];
    match std::str::from_utf8(head) {
        Ok(s) => Some(s.to_owned()),
        Err(e) if e.valid_up_to() > 0 => {
            std::str::from_utf8(&head[..e.valid_up_to()]).ok().map(str::to_owned)
        }
        Err(_) => None,
    }
}

/// A deliberately conservative heuristic — only consulted once no stronger
/// signal (binary magic bytes, an HTML/SVG tag, a recognized extension)
/// already resolved the format. Markdown vs. plain text is genuinely
/// ambiguous from content alone in many real documents; this only fires on
/// markers a plain text file is unlikely to open with by coincidence.
fn looks_like_markdown(text: &str) -> bool {
    text.lines().take(20).any(|line| {
        let l = line.trim_start();
        l.starts_with("# ")
            || l.starts_with("## ")
            || l.starts_with("```")
            || l.starts_with("- [ ] ")
            || l.starts_with("- [x] ")
            || (l.starts_with('|') && l.trim_end().ends_with('|') && l.len() > 1)
    })
}

/// The permissive last resort: valid UTF-8, or bytes with vanishingly few
/// non-whitespace control characters — the same bar a "just open it as
/// text" tool should use, not a strict encoding check. [`FORM_FEED`] is
/// explicitly allowed: it is this control's own explicit page-break marker,
/// not a sign of binary content.
fn looks_like_text(bytes: &[u8]) -> bool {
    if bytes.is_empty() {
        return true;
    }
    let sample = &bytes[..bytes.len().min(8192)];
    if std::str::from_utf8(sample).is_ok() {
        return true;
    }
    let bad = sample
        .iter()
        .filter(|&&b| b < 0x20 && !matches!(b, 0x09 | 0x0A | FORM_FEED | 0x0D))
        .count();
    (bad as f64) < (sample.len() as f64) * 0.01
}

// ── Indexing and decoding ───────────────────────────────────────────────

/// The largest single page [`index_text`] will produce without an explicit
/// form-feed break — the ceiling that keeps a breakless file's decode cost
/// bounded (R2), not a claim about what a *visual* page looks like on
/// screen (that re-flow, font-metric-dependent, is T11's job).
pub const MAX_TEXT_PAGE_BYTES: u64 = 64 * 1024;

/// The plain-text explicit page-break character (R9) — see the module intro.
pub const FORM_FEED: u8 = 0x0C;

/// One page's byte range within the source, `end` exclusive. An index holds
/// only offsets, never content — that is what lets [`decode_text_page`] cost
/// one page, regardless of which page or how large the source is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PageSpan {
    pub start: u64,
    pub end: u64,
}

impl PageSpan {
    pub fn len(&self) -> u64 {
        self.end - self.start
    }

    pub fn is_empty(&self) -> bool {
        self.end == self.start
    }
}

/// A document's page-break index, built once by [`index_text`].
#[derive(Debug, Clone)]
pub struct DocumentIndex {
    pub format: ViewerFormat,
    pub pages: Vec<PageSpan>,
    pub total_len: u64,
}

impl DocumentIndex {
    pub fn page_count(&self) -> usize {
        self.pages.len()
    }
}

/// Why a document failed to open (R4) — [`ViewerLoadError::to_message`] is
/// the exact text a caller puts in `LastError` when raising `onError`.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ViewerLoadError {
    #[error("unsupported document format")]
    UnsupportedFormat,
    #[error("{0} rendering is not yet available")]
    NotYetImplemented(ViewerFormat),
    #[error("could not read document: {0}")]
    Io(String),
}

impl From<io::Error> for ViewerLoadError {
    fn from(e: io::Error) -> Self {
        Self::Io(e.to_string())
    }
}

impl ViewerLoadError {
    pub fn to_message(&self) -> String {
        self.to_string()
    }
}

fn source_len(source: &DocumentSource) -> io::Result<u64> {
    match source {
        DocumentSource::Path(path) => Ok(std::fs::metadata(path)?.len()),
        DocumentSource::Bytes(bytes) => Ok(bytes.len() as u64),
    }
}

fn open_reader(source: &DocumentSource) -> io::Result<Box<dyn Read>> {
    match source {
        DocumentSource::Path(path) => Ok(Box::new(File::open(path)?)),
        DocumentSource::Bytes(bytes) => Ok(Box::new(io::Cursor::new(Arc::clone(bytes)))),
    }
}

fn read_head(source: &DocumentSource, max_bytes: usize) -> io::Result<Vec<u8>> {
    match source {
        DocumentSource::Path(path) => {
            let mut f = File::open(path)?;
            let mut buf = vec![0u8; max_bytes];
            let n = f.read(&mut buf)?;
            buf.truncate(n);
            Ok(buf)
        }
        DocumentSource::Bytes(bytes) => Ok(bytes[..bytes.len().min(max_bytes)].to_vec()),
    }
}

/// The longest prefix length of `pending` (≤ `pending.len()`) that keeps a
/// UTF-8-valid string — used to cut a page at the size ceiling without ever
/// splitting a multi-byte character. Falls back to `1` only if `pending`
/// isn't valid UTF-8 at all from its very first byte, which would mean the
/// source stopped being valid text partway through; that byte then renders
/// as a replacement character on decode rather than looping forever.
fn utf8_safe_cut(pending: &[u8]) -> usize {
    match std::str::from_utf8(pending) {
        Ok(_) => pending.len(),
        Err(e) => e.valid_up_to().max(1).min(pending.len()),
    }
}

/// Builds a plain-text document's page index — one sequential pass, never
/// holding more than one page's bytes at a time (R2). `on_progress` is
/// called with a 0-100 value as the scan advances (R6); [`index_text`] is
/// the plain convenience form for a caller with nothing to report to.
///
/// See the module intro for why an explicit form-feed and the size ceiling
/// end a page the same way.
pub fn index_text_with_progress(
    source: &DocumentSource,
    mut on_progress: impl FnMut(i64),
) -> io::Result<DocumentIndex> {
    let total_len = source_len(source)?;
    let mut pages = Vec::new();
    let mut page_start: u64 = 0;
    // Bytes read since `page_start`, not yet assigned to a finished page —
    // bounded to roughly one page's worth at any moment, regardless of
    // `total_len` (R2).
    let mut pending: Vec<u8> = Vec::new();
    let mut offset: u64 = 0;
    let mut last_reported: i64 = -1;

    let mut reader = open_reader(source)?;
    let mut block = [0u8; 8192];
    loop {
        let n = reader.read(&mut block)?;
        if n == 0 {
            break;
        }
        for &byte in &block[..n] {
            pending.push(byte);
            offset += 1;
            if byte == FORM_FEED {
                pages.push(PageSpan { start: page_start, end: offset });
                page_start = offset;
                pending.clear();
            } else if pending.len() as u64 >= MAX_TEXT_PAGE_BYTES {
                let cut = utf8_safe_cut(&pending);
                let end = page_start + cut as u64;
                pages.push(PageSpan { start: page_start, end });
                page_start = end;
                pending.drain(..cut);
            }
        }
        let pct = if total_len == 0 {
            100
        } else {
            ((offset.saturating_mul(100)) / total_len).min(100) as i64
        };
        if pct != last_reported {
            on_progress(pct);
            last_reported = pct;
        }
    }
    if page_start < total_len || pages.is_empty() {
        pages.push(PageSpan { start: page_start, end: total_len });
    }
    if last_reported != 100 {
        on_progress(100);
    }
    Ok(DocumentIndex { format: ViewerFormat::Text, pages, total_len })
}

/// [`index_text_with_progress`] with no progress callback.
pub fn index_text(source: &DocumentSource) -> io::Result<DocumentIndex> {
    index_text_with_progress(source, |_| {})
}

/// Reads exactly one page's bytes and decodes them as text. Costs one
/// page's I/O regardless of the source's total size or which page is asked
/// for — the whole reason [`index_text`] runs first (R2).
pub fn decode_text_page(source: &DocumentSource, span: PageSpan) -> io::Result<String> {
    let mut buf = vec![0u8; span.len() as usize];
    match source {
        DocumentSource::Path(path) => {
            let mut f = File::open(path)?;
            f.seek(SeekFrom::Start(span.start))?;
            f.read_exact(&mut buf)?;
        }
        DocumentSource::Bytes(bytes) => {
            let start = span.start as usize;
            let end = span.end as usize;
            buf.copy_from_slice(&bytes[start..end]);
        }
    }
    // index_text() only ever cuts at a validated UTF-8 boundary (or a
    // form-feed, itself a single ASCII byte), so re-reading the exact same
    // span is always complete, valid text — a lossy fallback here would
    // mask an indexing bug rather than handle a real one.
    String::from_utf8(buf).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e.to_string()))
}

/// Ties format resolution and indexing together — R1's `Source`/`LoadBytes`
/// both funnel through this once the caller has turned either into a
/// [`DocumentSource`]. Returns R3's resolved index on success, or R4's error
/// (unsupported, or unreadable) — leaving what to do with a prior document
/// entirely to the caller, since that live state does not exist in this
/// crate (R4's "leaves any previously loaded document displayed" is true by
/// construction as long as the caller only replaces its current document on
/// `Ok`).
pub fn open_document(
    source: &DocumentSource,
    on_progress: impl FnMut(i64),
) -> Result<DocumentIndex, ViewerLoadError> {
    let path_hint = match source {
        DocumentSource::Path(p) => Some(p.as_str()),
        DocumentSource::Bytes(_) => None,
    };
    let head = read_head(source, 4096)?;
    let format = detect_format(path_hint, &head).ok_or(ViewerLoadError::UnsupportedFormat)?;
    match format {
        ViewerFormat::Text => Ok(index_text_with_progress(source, on_progress)?),
        other => Err(ViewerLoadError::NotYetImplemented(other)),
    }
}

// ── Markdown → layout model (T9: R7, R9) ────────────────────────────────
//
// `parse_markdown` walks `pulldown-cmark`'s event stream into
// [`MarkdownDocument`] — a plain tree, no `egui` in sight. Per plan.md §3
// ("storage is native-format, always") the document's retained
// representation stays its own Markdown *text*; this tree is a derived view
// computed from it, cheap enough to recompute rather than needing to be kept
// in sync with anything. `paint::draw_viewer` (T11) is the only consumer.

/// A run of inline text sharing one style, or a non-text inline element —
/// the atoms a [`Block`] of prose is built from.
#[derive(Debug, Clone, PartialEq)]
pub enum Inline {
    Text { text: String, style: TextStyle },
    Image { alt: String, src: String, title: Option<String> },
    /// A footnote marker (`[^label]`) — its definition, if any, is a sibling
    /// or ancestor [`Block::FootnoteDefinition`] with the same label;
    /// references and definitions may occur in either order in the source.
    FootnoteRef { label: String },
    /// A line break inside running text — soft (an ordinary newline in the
    /// source) or hard (an explicit break, e.g. a trailing backslash).
    Break { hard: bool },
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct TextStyle {
    pub strong: bool,
    pub emphasis: bool,
    pub strikethrough: bool,
    pub code: bool,
    pub link: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TableAlignment {
    None,
    Left,
    Center,
    Right,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ListItem {
    pub blocks: Vec<Block>,
    /// `Some(true/false)` for a task-list item (`- [ ]`/`- [x]`); `None` for
    /// an ordinary list item.
    pub checked: Option<bool>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Block {
    Heading { level: u8, content: Vec<Inline> },
    Paragraph { content: Vec<Inline> },
    CodeBlock { language: Option<String>, text: String },
    BlockQuote { blocks: Vec<Block> },
    List { ordered: bool, start: Option<u64>, items: Vec<ListItem> },
    Table { alignments: Vec<TableAlignment>, header: Vec<Vec<Inline>>, rows: Vec<Vec<Vec<Inline>>> },
    ThematicBreak,
    FootnoteDefinition { label: String, blocks: Vec<Block> },
    /// Raw HTML the source embedded, carried through verbatim — nothing is
    /// silently dropped. Actually *rendering* HTML is a different format's
    /// job entirely (§3 `HtmlSubset`); this is Markdown's own escape hatch,
    /// kept as inert text.
    RawHtml(String),
}

/// A Markdown document's layout model — the result of walking its text once
/// with [`parse_markdown`].
#[derive(Debug, Clone, Default, PartialEq)]
pub struct MarkdownDocument {
    pub blocks: Vec<Block>,
}

impl MarkdownDocument {
    /// A flat count of every block, by kind, at any nesting depth — the
    /// shape this task's own test reports coverage in (this project's
    /// "quantify what a test exercised" rule).
    pub fn count_by_kind(&self) -> std::collections::BTreeMap<&'static str, usize> {
        fn walk(blocks: &[Block], counts: &mut std::collections::BTreeMap<&'static str, usize>) {
            for b in blocks {
                let key = match b {
                    Block::Heading { .. } => "Heading",
                    Block::Paragraph { .. } => "Paragraph",
                    Block::CodeBlock { .. } => "CodeBlock",
                    Block::BlockQuote { .. } => "BlockQuote",
                    Block::List { .. } => "List",
                    Block::Table { .. } => "Table",
                    Block::ThematicBreak => "ThematicBreak",
                    Block::FootnoteDefinition { .. } => "FootnoteDefinition",
                    Block::RawHtml(_) => "RawHtml",
                };
                *counts.entry(key).or_insert(0) += 1;
                match b {
                    Block::BlockQuote { blocks } | Block::FootnoteDefinition { blocks, .. } => {
                        walk(blocks, counts)
                    }
                    Block::List { items, .. } => {
                        for item in items {
                            walk(&item.blocks, counts);
                        }
                    }
                    _ => {}
                }
            }
        }
        let mut counts = std::collections::BTreeMap::new();
        walk(&self.blocks, &mut counts);
        counts
    }

    /// Every distinct image `src` referenced anywhere in the document, at
    /// any nesting depth — T10's own decode step consumes these.
    pub fn image_sources(&self) -> Vec<String> {
        fn walk_inlines(inlines: &[Inline], out: &mut Vec<String>) {
            for i in inlines {
                if let Inline::Image { src, .. } = i {
                    out.push(src.clone());
                }
            }
        }
        fn walk_cells(cells: &[Vec<Inline>], out: &mut Vec<String>) {
            for c in cells {
                walk_inlines(c, out);
            }
        }
        fn walk(blocks: &[Block], out: &mut Vec<String>) {
            for b in blocks {
                match b {
                    Block::Heading { content, .. } | Block::Paragraph { content, .. } => {
                        walk_inlines(content, out)
                    }
                    Block::BlockQuote { blocks } | Block::FootnoteDefinition { blocks, .. } => {
                        walk(blocks, out)
                    }
                    Block::List { items, .. } => {
                        for item in items {
                            walk(&item.blocks, out);
                        }
                    }
                    Block::Table { header, rows, .. } => {
                        walk_cells(header, out);
                        for row in rows {
                            walk_cells(row, out);
                        }
                    }
                    Block::CodeBlock { .. } | Block::ThematicBreak | Block::RawHtml(_) => {}
                }
            }
        }
        let mut out = Vec::new();
        walk(&self.blocks, &mut out);
        out
    }
}

/// An in-progress container on the walk's stack — becomes a [`Block`] (or
/// folds into its parent frame) once its matching `End` event arrives.
enum Frame {
    Paragraph(Vec<Inline>),
    Heading { level: u8, content: Vec<Inline> },
    CodeBlock { language: Option<String>, text: String },
    HtmlBlock(String),
    BlockQuote(Vec<Block>),
    List { ordered: bool, start: Option<u64>, items: Vec<ListItem> },
    Item { blocks: Vec<Block>, checked: Option<bool>, inline: Vec<Inline> },
    FootnoteDefinition { label: String, blocks: Vec<Block> },
    Table { alignments: Vec<TableAlignment>, header: Vec<Vec<Inline>>, rows: Vec<Vec<Vec<Inline>>> },
    TableRow(Vec<Vec<Inline>>),
    TableCell(Vec<Inline>),
}

/// A just-closed block goes onto whichever container is now on top of the
/// stack (a `BlockQuote`/`Item`/`FootnoteDefinition` — the only frames that
/// hold a plain `Vec<Block>`), or onto the document root if none is open.
fn push_block(stack: &mut [Frame], doc_blocks: &mut Vec<Block>, block: Block) {
    match stack.last_mut() {
        Some(Frame::BlockQuote(blocks)) => blocks.push(block),
        Some(Frame::Item { blocks, .. }) => blocks.push(block),
        Some(Frame::FootnoteDefinition { blocks, .. }) => blocks.push(block),
        _ => doc_blocks.push(block),
    }
}

/// The inline-content buffer that `Text`/`Code`/etc. events append to right
/// now — whichever of `Paragraph`/`Heading`/`TableCell`/a tight `Item` is on
/// top of the stack. `None` while inside a block that carries no inline
/// content of its own (a `List`, a `Table`, a `BlockQuote` before its first
/// child opens).
fn current_inline_buf(stack: &mut [Frame]) -> Option<&mut Vec<Inline>> {
    match stack.last_mut()? {
        Frame::Paragraph(v) => Some(v),
        Frame::Heading { content, .. } => Some(content),
        Frame::TableCell(v) => Some(v),
        Frame::Item { inline, .. } => Some(inline),
        _ => None,
    }
}

/// Walks `pulldown-cmark` events into a [`MarkdownDocument`] (R7, R9). The
/// "common extensions" §3 promises — tables, task lists, footnotes,
/// strikethrough — are enabled explicitly; CommonMark's core constructs need
/// no flag.
pub fn parse_markdown(text: &str) -> MarkdownDocument {
    let mut options = Options::empty();
    options.insert(Options::ENABLE_TABLES);
    options.insert(Options::ENABLE_TASKLISTS);
    options.insert(Options::ENABLE_FOOTNOTES);
    options.insert(Options::ENABLE_STRIKETHROUGH);

    let mut doc_blocks: Vec<Block> = Vec::new();
    let mut stack: Vec<Frame> = Vec::new();
    // Inline style state lives outside the block stack: Emphasis / Strong /
    // Strikethrough / Link nest independently of whichever block currently
    // holds the text they wrap.
    let mut strong_depth = 0u32;
    let mut emphasis_depth = 0u32;
    let mut strike_depth = 0u32;
    let mut link_stack: Vec<String> = Vec::new();
    // An Image's events between Start/End are its alt text, not a normal
    // inline run — accumulated here instead of through `current_inline_buf`.
    let mut image_stack: Vec<(String, String, Option<String>)> = Vec::new(); // (alt, src, title)

    for event in Parser::new_ext(text, options) {
        match event {
            Event::Start(tag) => match tag {
                Tag::Paragraph => stack.push(Frame::Paragraph(Vec::new())),
                Tag::Heading { level, .. } => {
                    stack.push(Frame::Heading { level: level as u8, content: Vec::new() })
                }
                Tag::CodeBlock(kind) => {
                    let language = match kind {
                        CodeBlockKind::Fenced(lang) if !lang.is_empty() => Some(lang.to_string()),
                        CodeBlockKind::Fenced(_) | CodeBlockKind::Indented => None,
                    };
                    stack.push(Frame::CodeBlock { language, text: String::new() });
                }
                Tag::HtmlBlock => stack.push(Frame::HtmlBlock(String::new())),
                Tag::BlockQuote(_) => stack.push(Frame::BlockQuote(Vec::new())),
                Tag::List(start) => {
                    stack.push(Frame::List { ordered: start.is_some(), start, items: Vec::new() })
                }
                Tag::Item => {
                    stack.push(Frame::Item { blocks: Vec::new(), checked: None, inline: Vec::new() })
                }
                Tag::FootnoteDefinition(label) => stack.push(Frame::FootnoteDefinition {
                    label: label.to_string(),
                    blocks: Vec::new(),
                }),
                Tag::Table(aligns) => {
                    let alignments = aligns
                        .iter()
                        .map(|a| match a {
                            Alignment::None => TableAlignment::None,
                            Alignment::Left => TableAlignment::Left,
                            Alignment::Center => TableAlignment::Center,
                            Alignment::Right => TableAlignment::Right,
                        })
                        .collect();
                    stack.push(Frame::Table { alignments, header: Vec::new(), rows: Vec::new() });
                }
                // `TableHead` needs no frame of its own: per pulldown-cmark's
                // own doc comment it "contains only TableCells" — no
                // `TableRow` wraps the header, unlike every body row — so a
                // header cell is told apart from a body cell structurally,
                // by what sits on the stack when its `TagEnd::TableCell`
                // fires (see there), not by a flag set here.
                Tag::TableHead => {}
                Tag::TableRow => stack.push(Frame::TableRow(Vec::new())),
                Tag::TableCell => stack.push(Frame::TableCell(Vec::new())),
                Tag::Emphasis => emphasis_depth += 1,
                Tag::Strong => strong_depth += 1,
                Tag::Strikethrough => strike_depth += 1,
                Tag::Link { dest_url, .. } => link_stack.push(dest_url.to_string()),
                Tag::Image { dest_url, title, .. } => {
                    let title = if title.is_empty() { None } else { Some(title.to_string()) };
                    image_stack.push((String::new(), dest_url.to_string(), title));
                }
                // DefinitionList*/MetadataBlock: gated behind options this
                // task does not enable, so these never fire in practice.
                Tag::DefinitionList | Tag::DefinitionListTitle | Tag::DefinitionListDefinition
                | Tag::MetadataBlock(_) => {}
            },
            Event::End(tag_end) => match tag_end {
                TagEnd::Paragraph => {
                    if let Some(Frame::Paragraph(content)) = stack.pop() {
                        push_block(&mut stack, &mut doc_blocks, Block::Paragraph { content });
                    }
                }
                TagEnd::Heading(_) => {
                    if let Some(Frame::Heading { level, content }) = stack.pop() {
                        push_block(&mut stack, &mut doc_blocks, Block::Heading { level, content });
                    }
                }
                TagEnd::CodeBlock => {
                    if let Some(Frame::CodeBlock { language, text }) = stack.pop() {
                        push_block(&mut stack, &mut doc_blocks, Block::CodeBlock { language, text });
                    }
                }
                TagEnd::HtmlBlock => {
                    if let Some(Frame::HtmlBlock(text)) = stack.pop() {
                        push_block(&mut stack, &mut doc_blocks, Block::RawHtml(text));
                    }
                }
                TagEnd::BlockQuote(_) => {
                    if let Some(Frame::BlockQuote(blocks)) = stack.pop() {
                        push_block(&mut stack, &mut doc_blocks, Block::BlockQuote { blocks });
                    }
                }
                TagEnd::List(ordered) => {
                    if let Some(Frame::List { start, items, .. }) = stack.pop() {
                        push_block(&mut stack, &mut doc_blocks, Block::List { ordered, start, items });
                    }
                }
                TagEnd::Item => {
                    if let Some(Frame::Item { mut blocks, checked, inline }) = stack.pop() {
                        // A tight item's direct text never went through a
                        // `Paragraph` frame — synthesize one so it is not lost.
                        if !inline.is_empty() {
                            blocks.push(Block::Paragraph { content: inline });
                        }
                        if let Some(Frame::List { items, .. }) = stack.last_mut() {
                            items.push(ListItem { blocks, checked });
                        }
                    }
                }
                TagEnd::FootnoteDefinition => {
                    if let Some(Frame::FootnoteDefinition { label, blocks }) = stack.pop() {
                        push_block(&mut stack, &mut doc_blocks, Block::FootnoteDefinition { label, blocks });
                    }
                }
                TagEnd::Table => {
                    if let Some(Frame::Table { alignments, header, rows }) = stack.pop() {
                        push_block(&mut stack, &mut doc_blocks, Block::Table { alignments, header, rows });
                    }
                }
                TagEnd::TableHead => {}
                TagEnd::TableRow => {
                    if let Some(Frame::TableRow(cells)) = stack.pop() {
                        if let Some(Frame::Table { rows, .. }) = stack.last_mut() {
                            rows.push(cells);
                        }
                    }
                }
                TagEnd::TableCell => {
                    if let Some(Frame::TableCell(content)) = stack.pop() {
                        // A body cell sits inside a `TableRow` frame; a
                        // header cell sits directly inside `Table` — see the
                        // comment on `Tag::TableHead` above.
                        match stack.last_mut() {
                            Some(Frame::TableRow(cells)) => cells.push(content),
                            Some(Frame::Table { header, .. }) => header.push(content),
                            _ => {}
                        }
                    }
                }
                TagEnd::Emphasis => emphasis_depth = emphasis_depth.saturating_sub(1),
                TagEnd::Strong => strong_depth = strong_depth.saturating_sub(1),
                TagEnd::Strikethrough => strike_depth = strike_depth.saturating_sub(1),
                TagEnd::Link => {
                    link_stack.pop();
                }
                TagEnd::Image => {
                    if let Some((alt, src, title)) = image_stack.pop() {
                        let image = Inline::Image { alt, src, title };
                        if let Some(buf) = current_inline_buf(&mut stack) {
                            buf.push(image);
                        } else if let Inline::Image { alt, src, title } = image {
                            // No enclosing paragraph (malformed/unusual
                            // nesting) — still surface it, as its own block,
                            // rather than lose it.
                            push_block(&mut stack, &mut doc_blocks, Block::Paragraph {
                                content: vec![Inline::Image { alt, src, title }],
                            });
                        }
                    }
                }
                TagEnd::DefinitionList | TagEnd::DefinitionListTitle | TagEnd::DefinitionListDefinition
                | TagEnd::MetadataBlock(_) => {}
            },
            Event::Text(t) => {
                if let Some(Frame::CodeBlock { text, .. }) = stack.last_mut() {
                    text.push_str(&t);
                } else if let Some((alt, _, _)) = image_stack.last_mut() {
                    alt.push_str(&t);
                } else if let Some(buf) = current_inline_buf(&mut stack) {
                    let style = TextStyle {
                        strong: strong_depth > 0,
                        emphasis: emphasis_depth > 0,
                        strikethrough: strike_depth > 0,
                        code: false,
                        link: link_stack.last().cloned(),
                    };
                    buf.push(Inline::Text { text: t.to_string(), style });
                }
            }
            Event::Code(t) => {
                if let Some((alt, _, _)) = image_stack.last_mut() {
                    alt.push_str(&t);
                } else if let Some(buf) = current_inline_buf(&mut stack) {
                    let style = TextStyle {
                        strong: strong_depth > 0,
                        emphasis: emphasis_depth > 0,
                        strikethrough: strike_depth > 0,
                        code: true,
                        link: link_stack.last().cloned(),
                    };
                    buf.push(Inline::Text { text: t.to_string(), style });
                }
            }
            Event::InlineMath(t) | Event::DisplayMath(t) => {
                // Not in this task's scope — kept as inert code-styled text
                // rather than silently dropped.
                if let Some(buf) = current_inline_buf(&mut stack) {
                    buf.push(Inline::Text {
                        text: t.to_string(),
                        style: TextStyle { code: true, ..TextStyle::default() },
                    });
                }
            }
            Event::Html(s) => {
                if let Some(Frame::HtmlBlock(buf)) = stack.last_mut() {
                    buf.push_str(&s);
                } else {
                    push_block(&mut stack, &mut doc_blocks, Block::RawHtml(s.to_string()));
                }
            }
            Event::InlineHtml(s) => {
                if let Some(buf) = current_inline_buf(&mut stack) {
                    buf.push(Inline::Text { text: s.to_string(), style: TextStyle::default() });
                }
            }
            Event::FootnoteReference(label) => {
                if let Some(buf) = current_inline_buf(&mut stack) {
                    buf.push(Inline::FootnoteRef { label: label.to_string() });
                }
            }
            Event::SoftBreak => {
                if let Some(buf) = current_inline_buf(&mut stack) {
                    buf.push(Inline::Break { hard: false });
                }
            }
            Event::HardBreak => {
                if let Some(buf) = current_inline_buf(&mut stack) {
                    buf.push(Inline::Break { hard: true });
                }
            }
            Event::Rule => push_block(&mut stack, &mut doc_blocks, Block::ThematicBreak),
            Event::TaskListMarker(checked) => {
                if let Some(Frame::Item { checked: c, .. }) = stack.last_mut() {
                    *c = Some(checked);
                }
            }
        }
    }

    MarkdownDocument { blocks: doc_blocks }
}

// ── Image decoding (T10: R7, AC2) ───────────────────────────────────────
//
// Raster and vector decode both need real codecs (`image`, `resvg`,
// `cobolt-media`) — exactly why they already sit behind `render` in this
// crate's `Cargo.toml`. A headless consumer of `detect_format`/`index_text`
// never needed a GPU-adjacent pixel buffer; gated here for the same reason,
// not a new one.

/// One decoded frame: RGBA8 pixels (`width * height * 4` bytes, row-major,
/// unpremultiplied) and how long it is shown before the next one — `0` for
/// a still image with no animation of its own.
#[cfg(feature = "render")]
#[derive(Debug, Clone, PartialEq)]
pub struct ImageFrame {
    pub rgba: Vec<u8>,
    pub delay_ms: u32,
}

/// A decoded image — one frame for a still image (PNG/JPEG/BMP/TIFF/SVG),
/// several for an animated one (GIF/WebP/APNG).
#[cfg(feature = "render")]
#[derive(Debug, Clone, PartialEq)]
pub struct DecodedImage {
    pub width: u32,
    pub height: u32,
    pub frames: Vec<ImageFrame>,
}

#[cfg(feature = "render")]
impl DecodedImage {
    pub fn frame_count(&self) -> usize {
        self.frames.len()
    }
}

/// Decodes any of §3's image formats. Raster formats (PNG/JPEG/GIF/WebP/
/// APNG/BMP/TIFF) go through `cobolt-media`'s existing Animator decoder —
/// one implementation for both controls, never two that can drift apart —
/// which also downscales any frame past 2048px on a side (its own
/// established safety cap; not new here, and not Viewer-specific). SVG is a
/// vector format neither `image` nor `cobolt-media` can read at all, so it
/// goes through `paint.rs`'s existing `resvg` path instead, rasterized once
/// at its own native size.
#[cfg(feature = "render")]
pub fn decode_image(bytes: &[u8]) -> Result<DecodedImage, String> {
    if crate::paint::is_svg_bytes(bytes) {
        let img =
            crate::paint::decode_svg_bytes(bytes).ok_or_else(|| "could not parse SVG".to_string())?;
        let rgba: Vec<u8> = img.pixels.iter().flat_map(|c| [c.r(), c.g(), c.b(), c.a()]).collect();
        return Ok(DecodedImage {
            width: img.size[0] as u32,
            height: img.size[1] as u32,
            frames: vec![ImageFrame { rgba, delay_ms: 0 }],
        });
    }
    let anim = cobolt_media::decode_animation(bytes)?;
    // cobolt-media's own still-image fallback reports a uniform minimum
    // delay for every format, including genuinely static ones — an
    // implementation detail of the Animator's frame-cycling code, not a
    // fact about the image. A single-frame result is never animated, so it
    // is reported as such here rather than passed through unchanged.
    let multi_frame = anim.frames.len() > 1;
    Ok(DecodedImage {
        width: anim.width,
        height: anim.height,
        frames: anim
            .frames
            .into_iter()
            .map(|f| ImageFrame { rgba: f.rgba, delay_ms: if multi_frame { f.delay_ms } else { 0 } })
            .collect(),
    })
}

// ── Navigation: zoom, cards, filmstrip, scrolling (T12) ─────────────────
//
// R11–R15, R33–R33.3 and R14–R14.4 are all *arithmetic* — how far a key
// moves the page, where a card grid reflows to, what a thrown page does
// next. Every bit of it lives here as plain functions and value types over
// numbers the caller supplies, with **time passed in as an explicit `dt`**
// rather than read from a clock. That is what lets a test hold an arrow key
// for two simulated seconds in a tight loop with no sleeping (this
// project's "measured completion signal, never a sleep-and-hope" rule), and
// what keeps this module `egui`-free while `paint.rs` draws and `render.rs`
// reads the real input.
//
// The scroll mechanics reproduce `cobolt-ide`'s Documentation viewer
// (`panels/doc_viewer.rs`) exactly, as spec.md §9 requires — the *algorithm*
// and its measured constants, not shared code: `cobolt-ide` is a binary
// crate with no `lib` target, so nothing here can call into it even in
// principle.

/// A view's two mutually exclusive modes (R14). `Cards` **replaces** the
/// document with a reflowing grid of one card per page — it never sits
/// beside it, which is the filmstrip's job (R14.3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ViewMode {
    #[default]
    Full,
    Cards,
}

impl ViewMode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Full => "Full",
            Self::Cards => "Cards",
        }
    }

    /// The lenient `DataGridGridLineStyle` idiom again — anything
    /// unrecognized reads as `Full`, the mode a freshly-dropped Viewer is
    /// seeded with, so a typo in a COBOL `SET-PROPERTY` shows the document
    /// rather than an empty grid.
    pub fn from_str(s: &str) -> Self {
        match s.trim().to_ascii_lowercase().as_str() {
            "cards" | "card" => Self::Cards,
            _ => Self::Full,
        }
    }
}

impl std::fmt::Display for ViewMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

// ── Zoom (R11–R13, R14.1/R14.2) ─────────────────────────────────────────

/// R12's cap — 16×, in the percent unit every `Zoom` property already uses.
pub const ZOOM_MAX_PCT: i64 = 1600;
/// R14.2's "legible minimum" end of the slider's range.
pub const ZOOM_MIN_PCT: i64 = 25;
/// What R13's Esc returns to.
pub const ZOOM_DEFAULT_PCT: i64 = 100;
/// One zoom step — a double-click (R12) or one wheel notch (R11). A 25 %
/// ratio, so reaching the 16× cap from 100 % takes thirteen steps rather
/// than arriving there on the first double-click.
pub const ZOOM_STEP_RATIO: f64 = 1.25;

pub fn clamp_zoom(pct: i64) -> i64 {
    pct.clamp(ZOOM_MIN_PCT, ZOOM_MAX_PCT)
}

/// One step in. Always advances by at least 1 % so a low zoom cannot get
/// stuck on integer rounding, and never past R12's cap.
pub fn zoom_in_step(pct: i64) -> i64 {
    let stepped = ((pct as f64) * ZOOM_STEP_RATIO).round() as i64;
    clamp_zoom(stepped.max(pct + 1))
}

/// One step out, the exact inverse ratio, never below the legible minimum.
pub fn zoom_out_step(pct: i64) -> i64 {
    let stepped = ((pct as f64) / ZOOM_STEP_RATIO).round() as i64;
    clamp_zoom(stepped.min(pct - 1))
}

/// `notches` wheel notches at once — positive zooms in. Fractional notches
/// (a trackpad's continuous scroll) are honoured rather than rounded away,
/// which is what makes a pinch feel continuous instead of stepped.
pub fn zoom_by_notches(pct: i64, notches: f32) -> i64 {
    if notches == 0.0 {
        return clamp_zoom(pct);
    }
    let scaled = (pct as f64) * ZOOM_STEP_RATIO.powf(notches as f64);
    let rounded = scaled.round() as i64;
    // Same anti-stall guard as the single-step helpers: at 25 % a tenth of a
    // notch would otherwise round back to where it started forever.
    let moved = if rounded == pct {
        if notches > 0.0 {
            pct + 1
        } else {
            pct - 1
        }
    } else {
        rounded
    };
    clamp_zoom(moved)
}

/// R11: the scroll offset that keeps the document point currently under the
/// pointer exactly under it after the zoom changes.
///
/// `offset` and `cursor_from_top` are both in painted points at the *old*
/// zoom, measured down from the content area's top edge. The document
/// coordinate under the pointer is `(offset + cursor_from_top) / old`; hold
/// that constant across the change and the new offset falls out directly.
pub fn zoom_anchored_offset(offset: f32, cursor_from_top: f32, old_pct: i64, new_pct: i64) -> f32 {
    if old_pct <= 0 {
        return offset;
    }
    let ratio = new_pct as f32 / old_pct as f32;
    ((offset + cursor_from_top) * ratio - cursor_from_top).max(0.0)
}

// ── Card grid (R14/R14.1, AC19) ─────────────────────────────────────────

/// The narrowest a page card is drawn, at `CardSize` 0 %.
pub const CARD_MIN_WIDTH: f32 = 72.0;
/// The widest, at `CardSize` 100 % — "fewer, bigger cards per row" (R14.1).
pub const CARD_MAX_WIDTH: f32 = 320.0;
/// The gutter between cards, and around the grid's own edge.
pub const CARD_GAP: f32 = 12.0;
/// A card's height as a multiple of its width — ISO A4 portrait (1:√2),
/// the shape a paginated document's page already has under `Print`/`Page`.
pub const CARD_ASPECT: f32 = 1.414;

/// Where a `Cards`-mode grid reflowed to.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CardGrid {
    pub cols: usize,
    pub rows: usize,
    pub card_w: f32,
    pub card_h: f32,
}

impl CardGrid {
    /// The grid's full painted height, however far it overflows the view —
    /// what a `Cards`-mode view scrolls against.
    pub fn content_height(&self) -> f32 {
        if self.rows == 0 {
            return 0.0;
        }
        CARD_GAP + self.rows as f32 * (self.card_h + CARD_GAP)
    }

    /// The top-left corner of card `i`, relative to the grid's own origin.
    pub fn card_origin(&self, i: usize) -> (f32, f32) {
        let col = if self.cols == 0 { 0 } else { i % self.cols };
        let row = if self.cols == 0 { 0 } else { i / self.cols };
        (
            CARD_GAP + col as f32 * (self.card_w + CARD_GAP),
            CARD_GAP + row as f32 * (self.card_h + CARD_GAP),
        )
    }
}

/// AC19: the row **and** column counts are a function of the card size and
/// **the view's own width** — never the window, the screen, or anything
/// else the control cannot see. `page_count` is not a third independent
/// input: with one card per page and the columns already fixed by width,
/// the row count is just `ceil(pages / cols)`.
pub fn card_grid(view_width: f32, card_size_pct: i64, page_count: usize) -> CardGrid {
    let t = (card_size_pct.clamp(0, 100) as f32) / 100.0;
    let card_w = CARD_MIN_WIDTH + t * (CARD_MAX_WIDTH - CARD_MIN_WIDTH);
    let card_h = (card_w * CARD_ASPECT).round();
    let usable = (view_width - CARD_GAP).max(0.0);
    let cols = ((usable / (card_w + CARD_GAP)).floor() as usize).max(1);
    let rows = page_count.div_ceil(cols);
    CardGrid { cols, rows, card_w: card_w.round(), card_h }
}

// ── The one slider per view (R14.1/R14.2) ───────────────────────────────

/// Which of a view's two remembered values R14.1's single bottom-right
/// slider is driving right now, and the range it spans.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SliderTarget {
    /// The **view-prefixed suffix** of the property — `"Zoom"` or
    /// `"CardSize"`, which the caller prefixes with `View1`/`View2`.
    pub property: &'static str,
    pub min: i64,
    pub max: i64,
}

pub fn slider_target(mode: ViewMode) -> SliderTarget {
    match mode {
        ViewMode::Full => SliderTarget { property: "Zoom", min: ZOOM_MIN_PCT, max: ZOOM_MAX_PCT },
        ViewMode::Cards => SliderTarget { property: "CardSize", min: 0, max: 100 },
    }
}

/// The slider's handle position (0…1) for whichever value `mode` drives.
///
/// `Zoom` is placed on a **log** scale: its range spans six octaves
/// (25 %…1600 %), and a linear handle would bunch everything below 400 % into
/// the first quarter of the track. `CardSize` is already a percentage of the
/// thing it sizes, so it maps straight through.
pub fn slider_position(mode: ViewMode, zoom_pct: i64, card_size_pct: i64) -> f32 {
    match mode {
        ViewMode::Full => {
            let z = clamp_zoom(zoom_pct) as f32;
            let lo = ZOOM_MIN_PCT as f32;
            let hi = ZOOM_MAX_PCT as f32;
            ((z / lo).ln() / (hi / lo).ln()).clamp(0.0, 1.0)
        }
        ViewMode::Cards => (card_size_pct.clamp(0, 100) as f32) / 100.0,
    }
}

/// R14.2 made structural rather than remembered: dragging the slider to `t`
/// returns the `(zoom, card_size)` pair with **only the value the active mode
/// drives** changed. Switching modes cannot disturb the other value because
/// nothing here ever writes it.
pub fn apply_slider(mode: ViewMode, t: f32, zoom_pct: i64, card_size_pct: i64) -> (i64, i64) {
    let t = t.clamp(0.0, 1.0);
    match mode {
        ViewMode::Full => {
            let lo = ZOOM_MIN_PCT as f32;
            let hi = ZOOM_MAX_PCT as f32;
            (clamp_zoom((lo * (hi / lo).powf(t)).round() as i64), card_size_pct)
        }
        ViewMode::Cards => (zoom_pct, (t * 100.0).round() as i64),
    }
}

// ── Filmstrip (R14.3/R14.4) ─────────────────────────────────────────────

/// The rail's width the first time a view opens it.
pub const FILMSTRIP_DEFAULT_WIDTH: f32 = 128.0;
/// The narrowest a rail is kept at while still open.
pub const FILMSTRIP_MIN_WIDTH: f32 = 64.0;
/// The widest the splitter will let it grow.
pub const FILMSTRIP_MAX_WIDTH: f32 = 320.0;
/// R14.4's second close gesture: drag the splitter this close to the view's
/// left edge and the filmstrip closes rather than shrinking further — the
/// same gesture that resizes it, taken to its limit.
pub const FILMSTRIP_CLOSE_WIDTH: f32 = 40.0;

/// What a filmstrip splitter drag to `width` should do: `Some(kept width)`
/// while it is still a resize, `None` once it has been taken to the view's
/// left edge and the rail should close (R14.4).
pub fn filmstrip_width_after_drag(width: f32) -> Option<f32> {
    if width < FILMSTRIP_CLOSE_WIDTH {
        None
    } else {
        Some(width.clamp(FILMSTRIP_MIN_WIDTH, FILMSTRIP_MAX_WIDTH))
    }
}

// ── Chrome geometry (R14.1, R14.3, R15, R16) ────────────────────────────

/// A rectangle in the same absolute screen points `egui::Rect` uses, spelled
/// without depending on `egui` so this geometry stays testable from the pure
/// half of the crate. `paint.rs` converts at the boundary.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ViewRect {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

impl ViewRect {
    pub fn new(x: f32, y: f32, w: f32, h: f32) -> Self {
        Self { x, y, w: w.max(0.0), h: h.max(0.0) }
    }
    pub fn right(&self) -> f32 {
        self.x + self.w
    }
    pub fn bottom(&self) -> f32 {
        self.y + self.h
    }
    pub fn contains(&self, px: f32, py: f32) -> bool {
        px >= self.x && px <= self.right() && py >= self.y && py <= self.bottom()
    }
}

/// The toolbar band's height (R16) — reserved here so R15's hide/restore is
/// a geometry fact both surfaces agree on before T13 paints anything into it.
pub const TOOLBAR_HEIGHT: f32 = 34.0;
/// The height of the strip R14.1's slider sits in, directly below the
/// content.
pub const SLIDER_STRIP_HEIGHT: f32 = 22.0;
/// The slider's own painted width inside that strip, right-aligned.
pub const SLIDER_WIDTH: f32 = 140.0;
/// The inset from the view's right edge and from the strip's edges.
pub const SLIDER_MARGIN: f32 = 8.0;

/// What a single view is currently showing, as far as its chrome cares.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ChromeOpts {
    /// R15 — fullscreen hides the toolbar entirely.
    pub fullscreen: bool,
    /// `Layout = Streamed` (§8.8) — one content pane, no chrome at all.
    /// Honoured here from the start so T35 is a paint change, not a second
    /// geometry model that could disagree with this one.
    pub streamed: bool,
    /// `Some(width)` while this view's filmstrip is open (R14.3).
    pub filmstrip: Option<f32>,
}

impl Default for ChromeOpts {
    fn default() -> Self {
        Self { fullscreen: false, streamed: false, filmstrip: None }
    }
}

/// Where each piece of a view's chrome lands inside `bounds`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ChromeLayout {
    /// The toolbar band — `None` under fullscreen (R15) or `Streamed`.
    pub toolbar: Option<ViewRect>,
    /// The page-thumbnail rail, docked to the **content's** left edge
    /// (R14.3), not the control's.
    pub filmstrip: Option<ViewRect>,
    /// What is left for the document (or the card grid).
    pub content: ViewRect,
    /// R14.1's one slider per view, bottom-right, directly below that
    /// view's own content. Zero-sized under `Streamed`, which has no
    /// slider to place.
    pub slider: ViewRect,
}

/// Splits a view's rect into toolbar / filmstrip / content / slider, in that
/// docking order. One function, used by every surface — so "fullscreen hides
/// the toolbar" is one fact, checked once (AC5), not a condition repeated at
/// each call site.
pub fn chrome_layout(bounds: ViewRect, opts: &ChromeOpts) -> ChromeLayout {
    if opts.streamed {
        return ChromeLayout {
            toolbar: None,
            filmstrip: None,
            content: bounds,
            slider: ViewRect::new(bounds.x, bounds.bottom(), 0.0, 0.0),
        };
    }

    let mut y = bounds.y;
    let mut remaining_h = bounds.h;

    let toolbar = if opts.fullscreen || remaining_h < TOOLBAR_HEIGHT * 2.0 {
        None
    } else {
        let r = ViewRect::new(bounds.x, y, bounds.w, TOOLBAR_HEIGHT);
        y += TOOLBAR_HEIGHT;
        remaining_h -= TOOLBAR_HEIGHT;
        Some(r)
    };

    let slider_h = if remaining_h > SLIDER_STRIP_HEIGHT * 2.0 { SLIDER_STRIP_HEIGHT } else { 0.0 };
    let body_h = (remaining_h - slider_h).max(0.0);

    let (filmstrip, content) = match opts.filmstrip {
        Some(w) if w > 0.0 && bounds.w > w + FILMSTRIP_MIN_WIDTH => {
            let w = w.clamp(FILMSTRIP_MIN_WIDTH, FILMSTRIP_MAX_WIDTH).min(bounds.w * 0.5);
            (
                Some(ViewRect::new(bounds.x, y, w, body_h)),
                ViewRect::new(bounds.x + w, y, bounds.w - w, body_h),
            )
        }
        _ => (None, ViewRect::new(bounds.x, y, bounds.w, body_h)),
    };

    // Directly below *that view's own content* (R14.1) — so in split view it
    // follows the content's left edge past the filmstrip, rather than
    // spanning the whole control.
    let slider = ViewRect::new(
        (content.right() - SLIDER_MARGIN - SLIDER_WIDTH).max(content.x),
        content.bottom() + (slider_h - 10.0).max(0.0) * 0.5,
        SLIDER_WIDTH.min(content.w),
        if slider_h > 0.0 { 10.0 } else { 0.0 },
    );

    ChromeLayout { toolbar, filmstrip, content, slider }
}

// ── The toolbar (T13: R16, R17, AC10) ───────────────────────────────────

/// Everything R16 puts on the toolbar, in the order it is drawn.
///
/// **Zoom and card size are deliberately absent** — R14.1's one
/// bottom-right slider per view is their only control, "unified rather than
/// duplicated in the toolbar too" (R16's own wording).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ToolbarAction {
    /// Cycles `Layout` through `Raw` → `Web` → `Print` → `Page`.
    CycleLayout,
    ViewFull,
    ViewCards,
    FontSmaller,
    FontLarger,
    Filmstrip,
    Fullscreen,
    Split,
    Find,
    Print,
    Share,
    SaveAs,
}

impl ToolbarAction {
    /// A stable name — this is what a host sees in
    /// `RenderOutput::toolbar_actions`, so it never changes.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::CycleLayout => "layout",
            Self::ViewFull => "view-full",
            Self::ViewCards => "view-cards",
            Self::FontSmaller => "font-smaller",
            Self::FontLarger => "font-larger",
            Self::Filmstrip => "filmstrip",
            Self::Fullscreen => "fullscreen",
            Self::Split => "split",
            Self::Find => "find",
            Self::Print => "print",
            Self::Share => "share",
            Self::SaveAs => "save-as",
        }
    }

    /// The catalogue icon that draws it (R17: painter-drawn vectors, never a
    /// glyph or a bitmap — `icons.rs` has no raster asset anywhere).
    ///
    /// Every one of these already existed. plan.md §2 expected to have to
    /// author a Find glyph and to "verify layout-switcher/font-size/
    /// filmstrip icons exist or author them"; measured against the
    /// catalogue, `magnifier`, `layout-dashboard`, `type-text` and
    /// `thumbnails` all do, and `grid-view` is already exactly a card grid.
    /// Drawing near-duplicates of those would have grown a 600-icon set for
    /// nothing.
    pub fn icon(self) -> &'static str {
        match self {
            Self::CycleLayout => "layout-dashboard",
            Self::ViewFull => "doc-text",
            Self::ViewCards => "grid-view",
            Self::FontSmaller => "font-smaller",
            Self::FontLarger => "font-larger",
            Self::Filmstrip => "thumbnails",
            Self::Fullscreen => "fullscreen",
            Self::Split => "split-view",
            Self::Find => "magnifier",
            Self::Print => "printer",
            Self::Share => "share",
            Self::SaveAs => "doc-save-as",
        }
    }

    /// R17: "each shall carry its function name as a tooltip". English is
    /// what this crate ships — it has no `Tr` table of its own and cannot
    /// reach `cobolt-ide`'s (a binary crate) — so a host with translations
    /// installs them through [`set_toolbar_tooltips`]. The DataGrid's own
    /// "Export CSV" tooltip set that precedent; this at least has a seam.
    pub fn default_tooltip(self) -> &'static str {
        match self {
            Self::CycleLayout => "Layout",
            Self::ViewFull => "Show the document",
            Self::ViewCards => "Show page cards",
            Self::FontSmaller => "Smaller text",
            Self::FontLarger => "Larger text",
            Self::Filmstrip => "Page thumbnails",
            Self::Fullscreen => "Fullscreen",
            Self::Split => "Split view",
            Self::Find => "Find",
            Self::Print => "Print",
            Self::Share => "Share",
            Self::SaveAs => "Save As",
        }
    }

    /// Lenient parse of [`Self::as_str`], for a host reading an action back.
    pub fn from_str(s: &str) -> Option<Self> {
        TOOLBAR_ITEMS.iter().copied().find(|a| a.as_str() == s.trim().to_ascii_lowercase())
    }
}

/// R16's toolbar, in painted order.
pub const TOOLBAR_ITEMS: &[ToolbarAction] = &[
    ToolbarAction::CycleLayout,
    ToolbarAction::ViewFull,
    ToolbarAction::ViewCards,
    ToolbarAction::FontSmaller,
    ToolbarAction::FontLarger,
    ToolbarAction::Filmstrip,
    ToolbarAction::Split,
    ToolbarAction::Find,
    ToolbarAction::Fullscreen,
    ToolbarAction::Print,
    ToolbarAction::Share,
    ToolbarAction::SaveAs,
];

/// One toolbar button's painted size, and the gap between two of them.
pub const TOOLBAR_BUTTON: f32 = 26.0;
pub const TOOLBAR_GAP: f32 = 4.0;
/// The inset from the band's own left and right edges.
pub const TOOLBAR_PAD: f32 = 6.0;

thread_local! {
    /// Translated tooltips, when a host has any. A thread-local because egui
    /// renders on one thread — the same shape `theme::set_active()` already
    /// uses to publish the active editor palette into the painter.
    static TOOLBAR_TOOLTIPS: std::cell::RefCell<Vec<(ToolbarAction, String)>> =
        const { std::cell::RefCell::new(Vec::new()) };
}

/// Install translated toolbar tooltips for this thread. An action left out
/// keeps its English [`ToolbarAction::default_tooltip`]; an empty table
/// restores English for all of them.
pub fn set_toolbar_tooltips(table: &[(ToolbarAction, String)]) {
    TOOLBAR_TOOLTIPS.with(|t| *t.borrow_mut() = table.to_vec());
}

/// The tooltip to show for `action` — the host's translation if one was
/// installed, otherwise this crate's English.
pub fn toolbar_tooltip(action: ToolbarAction) -> String {
    TOOLBAR_TOOLTIPS.with(|t| {
        t.borrow()
            .iter()
            .find(|(a, _)| *a == action)
            .map(|(_, s)| s.clone())
            .unwrap_or_else(|| action.default_tooltip().to_owned())
    })
}

/// Where each toolbar button lands inside `band`, left to right. Buttons
/// past the band's width are **dropped rather than squeezed** — a control
/// too narrow for its whole toolbar shows the leading actions at full size,
/// which reads, instead of twelve unrecognisable slivers.
pub fn toolbar_slots(band: ViewRect) -> Vec<(ToolbarAction, ViewRect)> {
    let y = band.y + (band.h - TOOLBAR_BUTTON).max(0.0) * 0.5;
    let mut x = band.x + TOOLBAR_PAD;
    let mut out = Vec::new();
    for action in TOOLBAR_ITEMS {
        if x + TOOLBAR_BUTTON > band.right() - TOOLBAR_PAD {
            break;
        }
        out.push((*action, ViewRect::new(x, y, TOOLBAR_BUTTON, TOOLBAR_BUTTON.min(band.h))));
        x += TOOLBAR_BUTTON + TOOLBAR_GAP;
    }
    out
}

/// The next `Layout` R16's layout button moves to. `Streamed` is **not** in
/// the cycle: it is a mode the developer chooses for a whole conversation
/// surface (§8.8), not something a user toggles past on the way to `Page`.
pub fn next_layout(current: &str) -> &'static str {
    match current.trim() {
        "Raw" => "Web",
        "Web" => "Print",
        "Print" => "Page",
        "Page" => "Raw",
        // Anything else (including `Streamed`) leaves the cycle where a
        // reader would expect to start.
        _ => "Web",
    }
}

/// R10's `FontSize` steps, in points. A short explicit ladder rather than a
/// multiplier: the sizes a reader actually wants are not a geometric series,
/// and every step must land on a whole point.
pub const FONT_SIZES: &[i64] = &[8, 9, 10, 11, 12, 13, 14, 16, 18, 20, 24, 28, 32, 40, 48];

pub fn font_size_step(current: i64, larger: bool) -> i64 {
    if larger {
        FONT_SIZES.iter().copied().find(|&s| s > current).unwrap_or(*FONT_SIZES.last().unwrap())
    } else {
        FONT_SIZES.iter().copied().rev().find(|&s| s < current).unwrap_or(FONT_SIZES[0])
    }
}

// ── Save As naming (T13: R18, R18.1, AC20) ──────────────────────────────

/// The filename extension a resolved format saves under (R18.1).
pub fn extension_for(format: ViewerFormat) -> &'static str {
    match format {
        ViewerFormat::Text => "txt",
        ViewerFormat::Markdown => "md",
        ViewerFormat::Image => "png",
        ViewerFormat::Pdf => "pdf",
        ViewerFormat::HtmlSubset => "html",
    }
}

/// The base name R18.1 falls back to for a document with no extractable
/// text — an image, say.
pub const DEFAULT_SAVE_BASE: &str = "document";

/// R18.1: the filename Save As proposes for a `LoadBytes` document that has
/// no source path to name it after — "the document's first three words of
/// extracted text, joined, plus the extension matching the resolved
/// `Format`", falling back to a generic base name when there is no text.
///
/// Words are taken from the text the document itself yields, with anything
/// a filesystem would object to (or a leading dot) removed. A document whose
/// "text" turns out to be only punctuation therefore takes the fallback too,
/// rather than proposing a name made of hyphens.
pub fn default_save_name(format: ViewerFormat, extracted_text: Option<&str>) -> String {
    let words: Vec<String> = extracted_text
        .unwrap_or("")
        .split_whitespace()
        .map(|w| {
            w.chars()
                .filter(|c| c.is_alphanumeric() || matches!(c, '-' | '_'))
                .collect::<String>()
        })
        // A run of punctuation is not a word. `-` and `_` survive the filter
        // above because they belong INSIDE a word ("2026-0417"), but a token
        // made of nothing else would propose a filename of hyphens — worse
        // than the generic fallback this then takes instead.
        .filter(|w| w.chars().any(char::is_alphanumeric))
        .take(3)
        .collect();
    let base = if words.is_empty() { DEFAULT_SAVE_BASE.to_owned() } else { words.join("-") };
    format!("{base}.{}", extension_for(format))
}

/// R18.1's last clause: "if the edited name is missing its extension, the
/// control shall append the correct one when writing the file regardless of
/// what the user typed."
///
/// A name that already ends in the right extension is left exactly as it is;
/// one that ends in a *different* one keeps it and gains the right one —
/// renaming `report.pdf` to `report.txt` behind the user's back would be a
/// lie about what the bytes are (R18 writes the original bytes unmodified).
pub fn ensure_extension(name: &str, format: ViewerFormat) -> String {
    let want = extension_for(format);
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return format!("{DEFAULT_SAVE_BASE}.{want}");
    }
    let has_it = trimmed
        .rsplit_once('.')
        .is_some_and(|(base, ext)| !base.is_empty() && ext.eq_ignore_ascii_case(want));
    if has_it {
        trimmed.to_owned()
    } else {
        format!("{trimmed}.{want}")
    }
}

// ── Scrolling (R33–R33.3, AC31, AC32) ───────────────────────────────────
//
// The constants below are `doc_viewer.rs`'s own, name for name and value for
// value. Changing one here without changing it there makes the two viewers
// behave differently under the same gesture, which is precisely what
// spec.md §9 set out to avoid.

/// One line, as a multiple of `FontSize` (R33).
pub const SCROLL_LINE_FACTOR: f32 = 1.6;
/// Points per second a held scroll key moves the document before the ramp.
pub const KEY_BASE_SPEED: f32 = 320.0;
/// The ramp's ceiling: four times the starting pace (R33).
pub const KEY_MAX_FACTOR: f32 = 4.0;
/// Seconds of holding it takes to reach [`KEY_MAX_FACTOR`].
pub const KEY_ACCEL_TIME: f32 = 2.0;
/// A held key only starts moving continuously after this long, so a tap
/// always reads as a tap rather than a flicker of fast scroll (R33).
pub const KEY_REPEAT_DELAY: f32 = 0.25;
/// Below this speed a glide stops rather than creeping, in points/second.
pub const THROW_STOP_SPEED: f32 = 20.0;
/// Constant friction, in points per second squared (R33.2 — *constant*, so a
/// fast throw travels further and takes longer, both ending at exactly zero).
pub const THROW_FRICTION: f32 = 1000.0;
/// How much of the drag's tail a throw's speed is measured over (R33.2's
/// "last ~0.12 s"), in seconds.
pub const THROW_SAMPLE_SECS: f64 = 0.12;

/// One line's travel for `font_size` (R33).
pub fn line_height(font_size: f32) -> f32 {
    (font_size * SCROLL_LINE_FACTOR).max(1.0)
}

/// R33.1: Page Up/Down move a viewport minus two lines, so context carries
/// across the jump.
pub fn page_step(viewport_h: f32, line: f32) -> f32 {
    (viewport_h - line * 2.0).max(line)
}

/// Speed multiplier for a scroll key held `held` seconds: `None` while it is
/// still a tap, then `1.0` rising to [`KEY_MAX_FACTOR`] over
/// [`KEY_ACCEL_TIME`] and never past it.
pub fn key_accel_factor(held: f32) -> Option<f32> {
    if held <= KEY_REPEAT_DELAY {
        return None;
    }
    let ramp = (held - KEY_REPEAT_DELAY) / KEY_ACCEL_TIME;
    Some((1.0 + ramp * (KEY_MAX_FACTOR - 1.0)).min(KEY_MAX_FACTOR))
}

/// How fast the page was moving when it was let go, in points per second,
/// measured across the drag's own tail samples — oldest to newest.
///
/// A hand that slowed to a stop before releasing throws nothing (R33.2),
/// because the oldest and newest samples in the window are then at the same
/// place. Positive means the pointer was moving **down**, which scrolls the
/// content back toward the start.
pub fn throw_speed(samples: &[(f64, f32)]) -> f32 {
    let (Some((t0, y0)), Some((t1, y1))) = (samples.first(), samples.last()) else {
        return 0.0;
    };
    let dt = (t1 - t0) as f32;
    if dt <= 0.0 {
        return 0.0;
    }
    (y1 - y0) / dt
}

/// Which scroll keys are down this frame (R33/R33.1). Held and tapped are
/// separate signals on purpose: the tap fires once on the press, the hold
/// ramps, and a single press produces both — exactly one line immediately,
/// then nothing until [`KEY_REPEAT_DELAY`] has passed.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct KeyScrollInput {
    pub down_held: bool,
    pub up_held: bool,
    pub down_tap: bool,
    pub up_tap: bool,
    pub page_down: bool,
    pub page_up: bool,
    pub home: bool,
    pub end: bool,
}

impl KeyScrollInput {
    pub fn any(&self) -> bool {
        self.down_held
            || self.up_held
            || self.down_tap
            || self.up_tap
            || self.page_down
            || self.page_up
            || self.home
            || self.end
    }
}

/// A grab in flight: where the pointer has been during this drag, and when.
///
/// Tracked here rather than read back from the pointer at release time —
/// `doc_viewer.rs` learned that the hard way: the pointer's own velocity
/// belongs to the pointer, so releasing anywhere but over the content read
/// back as zero and the page stopped dead instead of gliding.
#[derive(Debug, Clone, Default)]
struct Grab {
    samples: Vec<(f64, f32)>,
    last_y: f32,
}

/// One view's live scroll state — position, limit, and whatever gesture is
/// currently moving it. Driven entirely by explicit `dt`/`time` arguments,
/// so a test runs two seconds of held key in a loop that takes microseconds.
#[derive(Debug, Clone, Default)]
pub struct ScrollKinetics {
    offset: f32,
    max: f32,
    key_hold: f32,
    keys_down: bool,
    glide: f32,
    grab: Option<Grab>,
    /// Set by anything that moved `offset`; cleared by [`Self::take_settled`]
    /// once nothing is moving it any more. This is what makes R32's
    /// `onScrolled` fire on the rest, never on a glide frame.
    pending_settle: bool,
}

impl ScrollKinetics {
    pub fn offset(&self) -> f32 {
        self.offset
    }

    pub fn max(&self) -> f32 {
        self.max
    }

    pub fn glide_speed(&self) -> f32 {
        self.glide
    }

    pub fn is_gliding(&self) -> bool {
        self.glide != 0.0
    }

    pub fn is_grabbed(&self) -> bool {
        self.grab.is_some()
    }

    /// True while any gesture is still moving the content — a held key, a
    /// drag, or a glide.
    pub fn is_moving(&self) -> bool {
        self.keys_down || self.glide != 0.0 || self.grab.is_some()
    }

    /// The scrollable range, recomputed each frame from the laid-out content
    /// height against the viewport. Shrinking it pulls the offset back in
    /// rather than leaving the view parked past the end.
    pub fn set_max(&mut self, max: f32) {
        self.max = max.max(0.0);
        let clamped = self.offset.clamp(0.0, self.max);
        if clamped != self.offset {
            self.offset = clamped;
            self.pending_settle = true;
        }
    }

    /// Move to an absolute offset — a programmatic change (COBOL writing
    /// `ScrollPosition`), a filmstrip click, a Find match scrolled into view.
    pub fn set_offset(&mut self, offset: f32) {
        let next = offset.clamp(0.0, self.max);
        if next != self.offset {
            self.offset = next;
            self.pending_settle = true;
        }
    }

    /// Adopt an offset without treating it as a movement to report — used
    /// when re-seeding from stored state at the start of a frame.
    pub fn adopt_offset(&mut self, offset: f32) {
        self.offset = offset.clamp(0.0, self.max);
    }

    fn shift(&mut self, delta: f32) {
        if delta == 0.0 {
            return;
        }
        let next = (self.offset + delta).clamp(0.0, self.max);
        if next != self.offset {
            self.offset = next;
            self.pending_settle = true;
        }
    }

    /// R33/R33.1. `line` is [`line_height`] for the view's font size and
    /// `viewport_h` its content height; `dt` is the frame's elapsed time,
    /// clamped by the caller the same way `doc_viewer.rs` clamps it.
    pub fn apply_keys(&mut self, input: &KeyScrollInput, dt: f32, line: f32, viewport_h: f32) {
        self.keys_down = input.down_held || input.up_held;

        if input.home {
            self.set_offset(0.0);
            self.key_hold = 0.0;
            return;
        }
        if input.end {
            self.set_offset(self.max);
            self.key_hold = 0.0;
            return;
        }

        let mut delta = 0.0;
        if input.down_tap {
            delta += line;
        }
        if input.up_tap {
            delta -= line;
        }
        if self.keys_down {
            self.key_hold += dt;
            if let Some(factor) = key_accel_factor(self.key_hold) {
                let step = KEY_BASE_SPEED * factor * dt;
                if input.down_held {
                    delta += step;
                }
                if input.up_held {
                    delta -= step;
                }
            }
        } else {
            self.key_hold = 0.0;
        }
        let page = page_step(viewport_h, line);
        if input.page_down {
            delta += page;
        }
        if input.page_up {
            delta -= page;
        }
        self.shift(delta);
    }

    /// How long the current key hold has lasted — what AC31's three-point
    /// speed report is measured against.
    pub fn key_hold(&self) -> f32 {
        self.key_hold
    }

    /// A press over the content takes hold of it, **and stops whatever glide
    /// was still running** (R33.2's "catching a moving page").
    pub fn press(&mut self, time: f64, y: f32) {
        self.glide = 0.0;
        self.grab = Some(Grab { samples: vec![(time, y)], last_y: y });
    }

    /// The drag itself: the content follows the pointer 1:1 (R33.2), and the
    /// tail of the gesture is remembered for the throw.
    pub fn drag(&mut self, time: f64, y: f32) {
        let Some(grab) = self.grab.as_mut() else { return };
        let delta = y - grab.last_y;
        grab.last_y = y;
        grab.samples.push((time, y));
        grab.samples.retain(|(s, _)| time - *s <= THROW_SAMPLE_SECS);
        // Dragging the pointer DOWN pulls the content back toward the start.
        self.shift(-delta);
    }

    /// Let go — anywhere. What happens next is decided by how the hand was
    /// moving, never by where it stopped (R33.2).
    pub fn release(&mut self) {
        let Some(grab) = self.grab.take() else { return };
        self.glide = throw_speed(&grab.samples);
    }

    /// One frame of the glide, under constant friction, ending at exactly
    /// zero or exactly at a scroll limit — never on a fixed-duration ease
    /// (R33.2/AC32).
    pub fn glide_tick(&mut self, dt: f32) {
        if self.grab.is_some() || self.glide == 0.0 {
            return;
        }
        let friction = THROW_FRICTION * dt;
        if friction >= self.glide.abs() || self.glide.abs() < THROW_STOP_SPEED {
            self.glide = 0.0;
            return;
        }
        self.glide -= friction * self.glide.signum();
        let before = self.offset;
        self.shift(-self.glide * dt);
        if self.offset == before {
            // Ran into an end: the throw is spent.
            self.glide = 0.0;
        }
    }

    /// True exactly once, on the frame the content comes to rest after
    /// moving — R32's `onScrolled` moment, and never a glide frame.
    pub fn take_settled(&mut self) -> bool {
        if self.pending_settle && !self.is_moving() {
            self.pending_settle = false;
            return true;
        }
        false
    }
}

/// Fires once a value has stopped changing.
///
/// R32's table says `onZoomChanged` and `onCardSizeChanged` fire when the
/// value **settles** — so one wheel gesture or one slider drag raises one
/// event, not one per notch or one per frame. The caller says whether the
/// input driving the value is still happening; the settle is everything
/// else.
#[derive(Debug, Clone, PartialEq)]
pub struct SettleWatch<T> {
    current: T,
    reported: T,
    changing: bool,
}

impl<T: Copy + PartialEq> SettleWatch<T> {
    pub fn new(initial: T) -> Self {
        Self { current: initial, reported: initial, changing: false }
    }

    /// This frame's value, and whether the gesture driving it is still live.
    pub fn observe(&mut self, value: T, still_changing: bool) {
        self.current = value;
        self.changing = still_changing;
    }

    /// True exactly once per settled change.
    pub fn take_settled(&mut self) -> bool {
        if !self.changing && self.current != self.reported {
            self.reported = self.current;
            return true;
        }
        false
    }

    pub fn current(&self) -> T {
        self.current
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn bytes_source(data: &[u8]) -> DocumentSource {
        DocumentSource::Bytes(Arc::from(data.to_vec().into_boxed_slice()))
    }

    fn path_source_with(dir: &tempfile::TempDir, name: &str, data: &[u8]) -> DocumentSource {
        let path = dir.path().join(name);
        std::fs::File::create(&path).unwrap().write_all(data).unwrap();
        DocumentSource::Path(path.to_string_lossy().into_owned())
    }

    // ── R3/R4: format detection ─────────────────────────────────────────

    #[test]
    fn content_wins_over_a_misleading_extension() {
        let mut png = b"\x89PNG\r\n\x1a\n".to_vec();
        png.extend_from_slice(&[0u8; 16]);
        let dir = tempfile::tempdir().unwrap();
        let src = path_source_with(&dir, "report.txt", &png);
        let head = read_head(&src, 4096).unwrap();
        let detected = detect_format(Some("report.txt"), &head);
        println!("a .txt-named PNG detects as {detected:?}");
        assert_eq!(detected, Some(ViewerFormat::Image), "content must win over the .txt extension (R3)");
    }

    #[test]
    fn every_documented_extension_resolves_without_content_help() {
        let cases: &[(&str, ViewerFormat)] = &[
            ("notes.txt", ViewerFormat::Text),
            ("README.md", ViewerFormat::Markdown),
            ("photo.png", ViewerFormat::Image),
            ("photo.JPG", ViewerFormat::Image),
            ("scan.pdf", ViewerFormat::Pdf),
            ("page.html", ViewerFormat::HtmlSubset),
        ];
        for (name, want) in cases {
            let got = detect_format(Some(name), b"plain ambiguous bytes");
            println!("{name} -> {got:?} (want {want:?})");
            assert_eq!(got, Some(*want), "extension {name}");
        }
    }

    #[test]
    fn load_bytes_with_no_filename_detects_html_from_content_alone() {
        let html = b"<!DOCTYPE html>\n<html><body>Hi</body></html>";
        let got = detect_format(None, html);
        println!("no path_hint, HTML content -> {got:?}");
        assert_eq!(got, Some(ViewerFormat::HtmlSubset));
    }

    #[test]
    fn an_unrecognized_binary_blob_with_no_extension_is_unsupported() {
        // Non-UTF8, no magic-byte match, no extension: genuinely unsupported.
        let junk: Vec<u8> = (0..256u16).map(|b| (b % 256) as u8).collect();
        let got = detect_format(None, &junk);
        println!("unrecognized binary blob -> {got:?}");
        assert_eq!(got, None, "R4's trigger case: nothing recognized it");
    }

    #[test]
    fn extensionless_plain_text_still_opens_as_text() {
        let got = detect_format(Some("CHANGELOG"), b"Just some ordinary prose.\nSecond line.\n");
        println!("extensionless README-like file -> {got:?}");
        assert_eq!(got, Some(ViewerFormat::Text));
    }

    // ── R2/R9/AC1: indexing ─────────────────────────────────────────────

    #[test]
    fn explicit_form_feeds_become_page_boundaries() {
        let mut data = Vec::new();
        data.extend_from_slice(b"page one");
        data.push(FORM_FEED);
        data.extend_from_slice(b"page two");
        data.push(FORM_FEED);
        data.extend_from_slice(b"page three");
        let src = bytes_source(&data);
        let idx = index_text(&src).unwrap();
        println!("{} form-feed-delimited pages: {:?}", idx.page_count(), idx.pages);
        assert_eq!(idx.page_count(), 3);
        assert_eq!(decode_text_page(&src, idx.pages[0]).unwrap(), "page one\u{c}");
        assert_eq!(decode_text_page(&src, idx.pages[1]).unwrap(), "page two\u{c}");
        assert_eq!(decode_text_page(&src, idx.pages[2]).unwrap(), "page three");
    }

    #[test]
    fn a_breakless_file_is_chunked_at_the_size_ceiling() {
        let data = vec![b'x'; (MAX_TEXT_PAGE_BYTES as usize) * 3 + 100];
        let src = bytes_source(&data);
        let idx = index_text(&src).unwrap();
        println!(
            "breakless {}-byte file -> {} pages, ceiling {MAX_TEXT_PAGE_BYTES}",
            data.len(),
            idx.page_count()
        );
        assert_eq!(idx.page_count(), 4, "3 full pages plus a 100-byte remainder");
        for page in &idx.pages[..3] {
            assert_eq!(page.len(), MAX_TEXT_PAGE_BYTES);
        }
        assert_eq!(idx.pages[3].len(), 100);
    }

    #[test]
    fn a_page_boundary_never_splits_a_multibyte_character() {
        // Japanese text (multi-byte UTF-8 throughout) padded so the size
        // ceiling lands mid-run — a wrong cut would make from_utf8 fail on
        // one of the pages, or decode_text_page would error.
        let one_char = "あ"; // 3 bytes in UTF-8
        let repeat = (MAX_TEXT_PAGE_BYTES as usize) / one_char.len() + 500;
        let data: String = one_char.repeat(repeat);
        let src = bytes_source(data.as_bytes());
        let idx = index_text(&src).unwrap();
        println!(
            "{} bytes of 3-byte chars -> {} pages, sizes {:?}",
            data.len(),
            idx.page_count(),
            idx.pages.iter().map(PageSpan::len).collect::<Vec<_>>()
        );
        assert!(idx.page_count() >= 2, "must have actually split into multiple pages");
        for (i, page) in idx.pages.iter().enumerate() {
            let text = decode_text_page(&src, *page)
                .unwrap_or_else(|e| panic!("page {i} at {page:?} failed to decode: {e}"));
            assert!(text.chars().all(|c| c == 'あ'), "page {i} must be clean whole characters");
        }
        let rejoined: String = idx.pages.iter().map(|p| decode_text_page(&src, *p).unwrap()).collect();
        assert_eq!(rejoined, data, "pages must reassemble to the original text losslessly");
    }

    /// Spec 058 AC1 — jumping straight to a huge file's LAST page costs one
    /// page's I/O, not a scan of everything before it: the decoded content
    /// and its length are exactly that one page's, never the whole file's.
    #[test]
    fn decoding_only_the_last_page_of_a_large_file_reads_just_that_page() {
        let dir = tempfile::tempdir().unwrap();
        let page_count = 40usize;
        let mut data = Vec::with_capacity(page_count * MAX_TEXT_PAGE_BYTES as usize);
        for i in 0..page_count {
            let marker = b'A' + (i % 26) as u8;
            data.extend(std::iter::repeat(marker).take(MAX_TEXT_PAGE_BYTES as usize));
        }
        let src = path_source_with(&dir, "huge.txt", &data);
        let idx = index_text(&src).unwrap();
        println!(
            "huge.txt: {} bytes, {} pages (each <= {MAX_TEXT_PAGE_BYTES})",
            data.len(),
            idx.page_count()
        );
        assert_eq!(idx.page_count(), page_count);

        let last = *idx.pages.last().unwrap();
        let decoded = decode_text_page(&src, last).unwrap();
        let expected_marker = b'A' + ((page_count - 1) % 26) as u8;
        println!(
            "last page span {last:?}, decoded {} bytes, marker '{}'",
            decoded.len(),
            expected_marker as char
        );
        assert_eq!(decoded.len() as u64, MAX_TEXT_PAGE_BYTES, "must read exactly one page, not the file");
        assert!(
            decoded.bytes().all(|b| b == expected_marker),
            "must be the LAST page's own content, not page 0's"
        );
    }

    #[test]
    fn empty_document_indexes_as_one_empty_page() {
        let src = bytes_source(b"");
        let idx = index_text(&src).unwrap();
        println!("empty document -> {} page(s)", idx.page_count());
        assert_eq!(idx.page_count(), 1);
        assert!(idx.pages[0].is_empty());
        assert_eq!(decode_text_page(&src, idx.pages[0]).unwrap(), "");
    }

    // ── R6: load progress ───────────────────────────────────────────────

    #[test]
    fn progress_is_monotonic_and_reaches_100() {
        let data = vec![b'y'; (MAX_TEXT_PAGE_BYTES as usize) * 5];
        let src = bytes_source(&data);
        let mut reported = Vec::new();
        index_text_with_progress(&src, |pct| reported.push(pct)).unwrap();
        println!("progress ticks for a {}-byte document: {reported:?}", data.len());
        assert_eq!(reported.last().copied(), Some(100), "must reach 100 on completion");
        assert!(reported.windows(2).all(|w| w[0] <= w[1]), "must never go backwards");
        assert!(reported.iter().all(|&p| (0..=100).contains(&p)), "must stay within 0-100");
    }

    // ── R1/R3/R4: open_document end-to-end ──────────────────────────────

    #[test]
    fn open_document_loads_a_text_file_end_to_end() {
        let dir = tempfile::tempdir().unwrap();
        let src = path_source_with(&dir, "notes.txt", b"hello\nworld\n");
        let idx = open_document(&src, |_| {}).expect("a plain text file must open");
        assert_eq!(idx.format, ViewerFormat::Text);
        assert_eq!(idx.page_count(), 1);
    }

    #[test]
    fn open_document_reports_unsupported_format_as_an_error() {
        let junk: Vec<u8> = (0..256u16).map(|b| (b % 256) as u8).collect();
        let src = bytes_source(&junk);
        let err = open_document(&src, |_| {}).expect_err("unrecognized binary must error, not open");
        println!("LastError text: {}", err.to_message());
        assert_eq!(err, ViewerLoadError::UnsupportedFormat);
    }

    #[test]
    fn open_document_detects_markdown_honestly_even_though_it_cannot_render_it_yet() {
        let dir = tempfile::tempdir().unwrap();
        let src = path_source_with(&dir, "guide.md", b"# Title\n\nSome body text.\n");
        let err = open_document(&src, |_| {}).expect_err("Markdown decoding lands in T9, not here");
        println!("guide.md -> {err:?} ({})", err.to_message());
        assert_eq!(err, ViewerLoadError::NotYetImplemented(ViewerFormat::Markdown));
    }

    // ── T9: Markdown → layout model (R7, R9) ────────────────────────────

    /// Concatenates every plain-text run in an inline sequence, dropping
    /// style/images/breaks — robust to exactly how `pulldown-cmark` chose to
    /// split a phrase across Text events, which this crate does not control.
    fn plain_text(inlines: &[Inline]) -> String {
        inlines
            .iter()
            .filter_map(|i| match i {
                Inline::Text { text, .. } => Some(text.as_str()),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn headings_and_paragraphs_produce_the_right_block_kinds_and_levels() {
        let doc = parse_markdown("# Title\n\nSome prose here.\n\n## Subtitle\n");
        let counts = doc.count_by_kind();
        println!("headings+paragraphs: {counts:?}");
        assert_eq!(counts.get("Heading").copied(), Some(2));
        assert_eq!(counts.get("Paragraph").copied(), Some(1));
        match &doc.blocks[0] {
            Block::Heading { level, content } => {
                assert_eq!(*level, 1);
                assert_eq!(plain_text(content), "Title");
            }
            other => panic!("expected a Heading, got {other:?}"),
        }
        match &doc.blocks[2] {
            Block::Heading { level, .. } => assert_eq!(*level, 2),
            other => panic!("expected a Heading, got {other:?}"),
        }
    }

    #[test]
    fn emphasis_strong_and_inline_code_carry_their_own_style() {
        let doc = parse_markdown("Some **bold**, some *italic*, and `code()`.\n");
        let Block::Paragraph { content } = &doc.blocks[0] else { panic!("expected a Paragraph") };
        let find = |pred: fn(&TextStyle) -> bool| {
            content.iter().find_map(|i| match i {
                Inline::Text { text, style } if pred(style) => Some(text.clone()),
                _ => None,
            })
        };
        let bold = find(|s| s.strong);
        let italic = find(|s| s.emphasis);
        let code = find(|s| s.code);
        println!("bold={bold:?} italic={italic:?} code={code:?}");
        assert_eq!(bold.as_deref(), Some("bold"));
        assert_eq!(italic.as_deref(), Some("italic"));
        assert_eq!(code.as_deref(), Some("code()"));
    }

    #[test]
    fn links_carry_their_destination_url() {
        let doc = parse_markdown("See [the guide](https://example.test/guide) for details.\n");
        let Block::Paragraph { content } = &doc.blocks[0] else { panic!("expected a Paragraph") };
        let link = content.iter().find_map(|i| match i {
            Inline::Text { text, style } if style.link.is_some() => {
                Some((text.clone(), style.link.clone().unwrap()))
            }
            _ => None,
        });
        println!("link = {link:?}");
        let (text, href) = link.expect("a link must be present");
        assert_eq!(text, "the guide");
        assert_eq!(href, "https://example.test/guide");
    }

    #[test]
    fn strikethrough_text_carries_its_style() {
        let doc = parse_markdown("This is ~~wrong~~ right.\n");
        let Block::Paragraph { content } = &doc.blocks[0] else { panic!("expected a Paragraph") };
        let struck = content.iter().find_map(|i| match i {
            Inline::Text { text, style } if style.strikethrough => Some(text.clone()),
            _ => None,
        });
        println!("strikethrough text = {struck:?}");
        assert_eq!(struck.as_deref(), Some("wrong"));
    }

    #[test]
    fn tight_and_loose_lists_produce_the_same_paragraph_wrapped_shape() {
        let tight = parse_markdown("- one\n- two\n");
        let loose = parse_markdown("- one\n\n- two\n");
        for (name, doc) in [("tight", &tight), ("loose", &loose)] {
            let Block::List { items, ordered, .. } = &doc.blocks[0] else {
                panic!("{name}: expected a List")
            };
            assert!(!ordered, "{name}: must be an unordered list");
            assert_eq!(items.len(), 2, "{name}: must have two items");
            let texts: Vec<String> = items
                .iter()
                .map(|it| {
                    let Block::Paragraph { content } = &it.blocks[0] else {
                        panic!("{name}: item content must come through as a Paragraph, got {:?}", it.blocks)
                    };
                    plain_text(content)
                })
                .collect();
            println!("{name} list item texts: {texts:?}");
            assert_eq!(texts, vec!["one".to_string(), "two".to_string()]);
        }
    }

    #[test]
    fn an_ordered_list_reports_its_start_number() {
        let doc = parse_markdown("3. first\n4. second\n5. third\n");
        let Block::List { ordered, start, items } = &doc.blocks[0] else { panic!("expected a List") };
        println!("ordered={ordered} start={start:?} items={}", items.len());
        assert!(ordered);
        assert_eq!(*start, Some(3));
        assert_eq!(items.len(), 3);
    }

    #[test]
    fn nested_lists_produce_a_nested_list_block() {
        let doc = parse_markdown("- outer one\n  - inner a\n  - inner b\n- outer two\n");
        let Block::List { items, .. } = &doc.blocks[0] else { panic!("expected a List") };
        assert_eq!(items.len(), 2);
        let nested = items[0].blocks.iter().find_map(|b| match b {
            Block::List { items, .. } => Some(items.len()),
            _ => None,
        });
        println!("outer item 1's nested list has {nested:?} items");
        assert_eq!(nested, Some(2));
    }

    #[test]
    fn task_list_items_carry_their_checked_state() {
        let doc = parse_markdown("- [ ] todo\n- [x] done\n");
        let Block::List { items, .. } = &doc.blocks[0] else { panic!("expected a List") };
        let states: Vec<Option<bool>> = items.iter().map(|it| it.checked).collect();
        println!("task states = {states:?}");
        assert_eq!(states, vec![Some(false), Some(true)]);
    }

    #[test]
    fn tables_capture_header_alignment_and_body_rows() {
        let md = "| Name | Qty |\n|:---|---:|\n| Widget | 12 |\n| Gadget | 7 |\n";
        let doc = parse_markdown(md);
        let Block::Table { alignments, header, rows } = &doc.blocks[0] else { panic!("expected a Table") };
        let header_text: Vec<String> = header.iter().map(|c| plain_text(c)).collect();
        let row_texts: Vec<Vec<String>> =
            rows.iter().map(|r| r.iter().map(|c| plain_text(c)).collect()).collect();
        println!("alignments={alignments:?} header={header_text:?} rows={row_texts:?}");
        assert_eq!(*alignments, vec![TableAlignment::Left, TableAlignment::Right]);
        assert_eq!(header_text, vec!["Name".to_string(), "Qty".to_string()]);
        assert_eq!(
            row_texts,
            vec![
                vec!["Widget".to_string(), "12".to_string()],
                vec!["Gadget".to_string(), "7".to_string()],
            ]
        );
    }

    #[test]
    fn footnote_references_and_definitions_share_a_label() {
        let doc = parse_markdown("Here is a claim.[^1]\n\n[^1]: The supporting detail.\n");
        let Block::Paragraph { content } = &doc.blocks[0] else { panic!("expected a Paragraph") };
        let ref_label = content.iter().find_map(|i| match i {
            Inline::FootnoteRef { label } => Some(label.clone()),
            _ => None,
        });
        let def = doc.blocks.iter().find_map(|b| match b {
            Block::FootnoteDefinition { label, blocks } => Some((label, blocks)),
            _ => None,
        });
        println!("ref_label={ref_label:?} has_definition={}", def.is_some());
        let (def_label, def_blocks) = def.expect("a footnote definition must be present");
        assert_eq!(ref_label.as_deref(), Some("1"));
        assert_eq!(def_label, "1");
        let Block::Paragraph { content } = &def_blocks[0] else { panic!("expected a Paragraph") };
        assert!(plain_text(content).contains("The supporting detail."));
    }

    #[test]
    fn images_are_captured_inline_with_alt_src_and_are_listed_by_image_sources() {
        let doc = parse_markdown("Here: ![a logo](logo.png \"Our logo\") end.\n");
        let Block::Paragraph { content } = &doc.blocks[0] else { panic!("expected a Paragraph") };
        let img = content.iter().find_map(|i| match i {
            Inline::Image { alt, src, title } => Some((alt.clone(), src.clone(), title.clone())),
            _ => None,
        });
        println!("image = {img:?}, image_sources() = {:?}", doc.image_sources());
        let (alt, src, title) = img.expect("an inline image must be present");
        assert_eq!(alt, "a logo");
        assert_eq!(src, "logo.png");
        assert_eq!(title.as_deref(), Some("Our logo"));
        assert_eq!(doc.image_sources(), vec!["logo.png".to_string()]);
    }

    #[test]
    fn a_blockquote_nests_its_own_blocks() {
        let doc = parse_markdown("> Quoted line one.\n> Quoted line two.\n");
        let Block::BlockQuote { blocks } = &doc.blocks[0] else { panic!("expected a BlockQuote") };
        println!("blockquote nested blocks: {}", blocks.len());
        assert_eq!(blocks.len(), 1);
        let Block::Paragraph { content } = &blocks[0] else { panic!("expected a Paragraph") };
        let text = plain_text(content);
        println!("blockquote paragraph text = {text:?}");
        assert!(text.contains("Quoted line one."));
        assert!(text.contains("Quoted line two."));
    }

    #[test]
    fn a_fenced_code_block_captures_its_language_and_text() {
        let doc = parse_markdown("```cobol\nDISPLAY \"HELLO\".\n```\n");
        let Block::CodeBlock { language, text } = &doc.blocks[0] else { panic!("expected a CodeBlock") };
        println!("language={language:?} text={text:?}");
        assert_eq!(language.as_deref(), Some("cobol"));
        assert!(text.contains("DISPLAY \"HELLO\"."));
    }

    #[test]
    fn a_thematic_break_becomes_its_own_block() {
        let doc = parse_markdown("Above.\n\n---\n\nBelow.\n");
        let counts = doc.count_by_kind();
        println!("thematic break doc counts: {counts:?}");
        assert_eq!(counts.get("ThematicBreak").copied(), Some(1));
        assert!(matches!(doc.blocks[1], Block::ThematicBreak));
    }

    #[test]
    fn raw_html_is_carried_through_verbatim_not_dropped() {
        let doc = parse_markdown("<div class=\"note\">\nRaw HTML block.\n</div>\n");
        let counts = doc.count_by_kind();
        println!("raw-html doc counts: {counts:?}");
        assert!(
            counts.get("RawHtml").copied().unwrap_or(0) >= 1,
            "a raw HTML block must be preserved, not dropped"
        );
    }

    // ── T10: image decoding (R7, AC2) ───────────────────────────────────
    // Every format §3 promises is exercised by encoding a tiny synthetic
    // image with the SAME `image` crate that decodes it, rather than
    // vendoring binary fixtures — a real round trip, not a hand-guessed one.
    #[cfg(feature = "render")]
    mod image_tests {
        use super::*;

        fn encode(w: u32, h: u32, format: image::ImageFormat) -> Vec<u8> {
            let img = image::RgbaImage::from_fn(w, h, |x, y| {
                image::Rgba([(x * 10) as u8, (y * 10) as u8, 128, 255])
            });
            let mut buf = Vec::new();
            image::DynamicImage::ImageRgba8(img)
                .write_to(&mut std::io::Cursor::new(&mut buf), format)
                .expect("the image crate must be able to encode its own test fixture");
            buf
        }

        fn encode_gif(w: u32, h: u32, frame_count: u32) -> Vec<u8> {
            use image::codecs::gif::GifEncoder;
            use image::{Delay, Frame};
            let mut buf = Vec::new();
            {
                let mut encoder = GifEncoder::new(&mut buf);
                for i in 0..frame_count {
                    let img = image::RgbaImage::from_fn(w, h, |x, y| {
                        image::Rgba([((x + i) * 10) as u8, (y * 10) as u8, 0, 255])
                    });
                    let frame = Frame::from_parts(img, 0, 0, Delay::from_numer_denom_ms(50, 1));
                    encoder.encode_frame(frame).expect("encoding a synthetic GIF frame must succeed");
                }
            }
            buf
        }

        fn sample_svg(w: u32, h: u32) -> Vec<u8> {
            // `rgb(...)`, not a `#`-hex color: a literal `"#` inside this
            // raw string would close it early (the delimiter is `r#"…"#`).
            format!(
                r#"<svg xmlns="http://www.w3.org/2000/svg" width="{w}" height="{h}"><rect width="{w}" height="{h}" fill="rgb(51,102,153)"/></svg>"#
            )
            .into_bytes()
        }

        #[test]
        fn a_png_decodes_to_its_own_dimensions_as_a_still_frame() {
            let img = decode_image(&encode(6, 4, image::ImageFormat::Png)).expect("a valid PNG must decode");
            println!("PNG: {}x{}, {} frame(s), delay {}", img.width, img.height, img.frame_count(), img.frames[0].delay_ms);
            assert_eq!((img.width, img.height), (6, 4));
            assert_eq!(img.frame_count(), 1);
            assert_eq!(img.frames[0].rgba.len(), 6 * 4 * 4);
            assert_eq!(img.frames[0].delay_ms, 0, "a still image reports no animation delay");
        }

        #[test]
        fn a_jpeg_decodes_to_its_own_dimensions() {
            let img = decode_image(&encode(8, 5, image::ImageFormat::Jpeg)).expect("a valid JPEG must decode");
            println!("JPEG: {}x{}, {} frame(s)", img.width, img.height, img.frame_count());
            assert_eq!((img.width, img.height), (8, 5));
            assert_eq!(img.frame_count(), 1);
        }

        #[test]
        fn a_bmp_decodes_to_its_own_dimensions() {
            let img = decode_image(&encode(5, 5, image::ImageFormat::Bmp)).expect("a valid BMP must decode");
            println!("BMP: {}x{}, {} frame(s)", img.width, img.height, img.frame_count());
            assert_eq!((img.width, img.height), (5, 5));
            assert_eq!(img.frame_count(), 1);
        }

        #[test]
        fn a_tiff_decodes_to_its_own_dimensions() {
            let img = decode_image(&encode(7, 3, image::ImageFormat::Tiff)).expect("a valid TIFF must decode");
            println!("TIFF: {}x{}, {} frame(s)", img.width, img.height, img.frame_count());
            assert_eq!((img.width, img.height), (7, 3));
            assert_eq!(img.frame_count(), 1);
        }

        #[test]
        fn an_animated_gif_decodes_every_frame_with_a_real_delay() {
            let img = decode_image(&encode_gif(4, 4, 3)).expect("a valid animated GIF must decode");
            let delays: Vec<u32> = img.frames.iter().map(|f| f.delay_ms).collect();
            println!("GIF: {}x{}, {} frame(s), delays {delays:?}", img.width, img.height, img.frame_count());
            assert_eq!((img.width, img.height), (4, 4));
            assert_eq!(img.frame_count(), 3, "all 3 encoded frames must be decoded, not just the first");
            assert!(delays.iter().all(|&d| d > 0), "an animated image's frames must report a real delay");
        }

        #[test]
        fn an_svg_rasterizes_at_its_own_native_size() {
            let img = decode_image(&sample_svg(40, 20)).expect("a valid SVG must decode");
            println!("SVG: {}x{}, {} frame(s)", img.width, img.height, img.frame_count());
            assert_eq!((img.width, img.height), (40, 20));
            assert_eq!(img.frame_count(), 1);
            assert_eq!(img.frames[0].rgba.len(), 40 * 20 * 4);
        }

        #[test]
        fn malformed_image_bytes_report_an_error_not_a_panic() {
            let junk = vec![0u8; 32];
            let result = decode_image(&junk);
            println!("malformed bytes -> {result:?}");
            assert!(result.is_err());
        }
    }
}

/// Spec 058 T12 — navigation: zoom (R11–R13), the card grid and its slider
/// (R14–R14.2), the filmstrip (R14.3/R14.4), chrome geometry under
/// fullscreen (R15), and scrolling (R33–R33.3). Every test here reports the
/// numbers it measured, per this project's standing "quantify what you
/// exercised" rule — a bare pass would not tell a reader whether a held key
/// actually accelerated or merely moved.
#[cfg(test)]
mod nav_tests {
    use super::*;

    const DT: f32 = 1.0 / 60.0;

    // ── Zoom (R11–R13, AC4) ─────────────────────────────────────────────

    #[test]
    fn double_click_zoom_climbs_in_steps_and_stops_at_sixteen_times() {
        let mut z = ZOOM_DEFAULT_PCT;
        let mut ladder = vec![z];
        for _ in 0..40 {
            z = zoom_in_step(z);
            ladder.push(z);
        }
        println!("double-click ladder from 100 %: {ladder:?}");
        println!("cap reached after {} steps", ladder.iter().position(|&v| v == ZOOM_MAX_PCT).unwrap());
        assert_eq!(z, ZOOM_MAX_PCT, "R12: zoom must stop at exactly 16x (1600 %)");
        assert_eq!(zoom_in_step(ZOOM_MAX_PCT), ZOOM_MAX_PCT, "already at the cap: no further movement");
        assert!(ladder.windows(2).take(10).all(|w| w[1] > w[0]), "each step must actually zoom in");
    }

    #[test]
    fn zooming_out_stops_at_the_legible_minimum_and_never_stalls() {
        let mut z = ZOOM_DEFAULT_PCT;
        let mut steps = 0;
        while z > ZOOM_MIN_PCT && steps < 200 {
            let next = zoom_out_step(z);
            assert!(next < z, "every step out must move: {z} -> {next}");
            z = next;
            steps += 1;
        }
        println!("zoom-out from 100 % reached {z} % in {steps} steps (floor {ZOOM_MIN_PCT} %)");
        assert_eq!(z, ZOOM_MIN_PCT);
        assert_eq!(zoom_out_step(ZOOM_MIN_PCT), ZOOM_MIN_PCT);
    }

    #[test]
    fn wheel_zoom_keeps_the_document_point_under_the_pointer_fixed() {
        // The pointer sits 200 pt below the content's top edge, with the view
        // already scrolled 400 pt down.
        let (offset, cursor) = (400.0f32, 200.0f32);
        let old = 100;
        let new = zoom_by_notches(old, 3.0);
        let moved = zoom_anchored_offset(offset, cursor, old, new);
        let doc_before = (offset + cursor) / old as f32;
        let doc_after = (moved + cursor) / new as f32;
        println!(
            "zoom {old} % -> {new} %: offset {offset} -> {moved:.2}; \
             document point under the pointer {doc_before:.4} -> {doc_after:.4}"
        );
        assert!(new > old, "three notches in must zoom in");
        assert!(
            (doc_before - doc_after).abs() < 1e-3,
            "R11: the point under the pointer must not move (was {doc_before}, now {doc_after})"
        );
    }

    #[test]
    fn a_fractional_wheel_notch_still_moves_the_zoom() {
        let stepped = zoom_by_notches(ZOOM_MIN_PCT, 0.02);
        println!("a 0.02-notch trackpad nudge at the {ZOOM_MIN_PCT} % floor -> {stepped} %");
        assert!(stepped > ZOOM_MIN_PCT, "a tiny notch must not round away to a stall");
    }

    // ── Card grid + the one slider (R14–R14.2, AC19) ────────────────────

    #[test]
    fn the_card_grid_reflows_to_the_views_own_width() {
        let pages = 37;
        let size = 40;
        let mut report = Vec::new();
        for width in [260.0f32, 420.0, 700.0, 1100.0] {
            let g = card_grid(width, size, pages);
            report.push((width, g.cols, g.rows));
            println!(
                "view {width:>6.0} pt wide, CardSize {size} % -> {} cols x {} rows \
                 (card {:.0}x{:.0}, content {:.0} pt tall)",
                g.cols, g.rows, g.card_w, g.card_h, g.content_height()
            );
        }
        let cols: Vec<usize> = report.iter().map(|r| r.1).collect();
        assert!(cols.windows(2).all(|w| w[1] >= w[0]), "a wider view never holds fewer columns");
        assert!(cols[3] > cols[0], "AC19: resizing the VIEW must change the column count");
        for (_, cols, rows) in &report {
            assert_eq!(*rows, pages.div_ceil(*cols), "rows follow from columns, one card per page");
        }
    }

    #[test]
    fn a_bigger_card_size_means_fewer_bigger_cards_per_row() {
        let width = 900.0;
        let pages = 24;
        let mut prev: Option<(usize, f32)> = None;
        for size in [0, 25, 50, 75, 100] {
            let g = card_grid(width, size, pages);
            println!(
                "CardSize {size:>3} % -> card {:.0} pt wide, {} cols x {} rows",
                g.card_w, g.cols, g.rows
            );
            if let Some((pc, pw)) = prev {
                assert!(g.card_w > pw, "a larger CardSize must draw a larger card");
                assert!(g.cols <= pc, "R14.1: larger cards mean fewer per row, never more");
            }
            prev = Some((g.cols, g.card_w));
        }
    }

    #[test]
    fn switching_view_mode_never_touches_the_other_modes_slider_value() {
        // A view reading at 250 % that has also browsed cards at 80 %.
        let (mut zoom, mut cards) = (250i64, 80i64);

        // Drag the slider in Cards mode: only CardSize moves.
        let (z2, c2) = apply_slider(ViewMode::Cards, 0.25, zoom, cards);
        println!("Cards-mode drag to 25 %: Zoom {zoom} -> {z2}, CardSize {cards} -> {c2}");
        assert_eq!(z2, zoom, "R14.2: the Cards slider must not touch Zoom");
        assert_eq!(c2, 25);
        zoom = z2;
        cards = c2;

        // Switch to Full and drag: only Zoom moves, and CardSize is still 25.
        let (z3, c3) = apply_slider(ViewMode::Full, 1.0, zoom, cards);
        println!("Full-mode drag to the top: Zoom {zoom} -> {z3}, CardSize {cards} -> {c3}");
        assert_eq!(c3, cards, "R14.2: the Full slider must not touch CardSize");
        assert_eq!(z3, ZOOM_MAX_PCT);

        // And coming back to Cards shows the size it was left at.
        let handle = slider_position(ViewMode::Cards, z3, c3);
        println!("returning to Cards: handle at {handle:.3} (CardSize {c3} %)");
        assert!((handle - 0.25).abs() < 1e-6, "the remembered card size must come back");
    }

    #[test]
    fn the_slider_reports_the_property_its_mode_drives() {
        for mode in [ViewMode::Full, ViewMode::Cards] {
            let t = slider_target(mode);
            println!("{mode} mode -> slider drives {} over {}..={}", t.property, t.min, t.max);
        }
        assert_eq!(slider_target(ViewMode::Full).property, "Zoom");
        assert_eq!(slider_target(ViewMode::Cards).property, "CardSize");
        assert_eq!(ViewMode::from_str("cards"), ViewMode::Cards);
        assert_eq!(ViewMode::from_str("nonsense"), ViewMode::Full, "an unknown value shows the document");
    }

    #[test]
    fn the_zoom_sliders_round_trip_is_stable_across_its_whole_range() {
        let mut worst = 0.0f32;
        for pct in [25, 50, 100, 200, 400, 800, 1600] {
            let t = slider_position(ViewMode::Full, pct, 50);
            let (back, _) = apply_slider(ViewMode::Full, t, pct, 50);
            let err = (back - pct).abs() as f32 / pct as f32;
            worst = worst.max(err);
            println!("Zoom {pct:>4} % -> handle {t:.4} -> {back:>4} % (error {:.3} %)", err * 100.0);
        }
        assert!(worst < 0.01, "the log-scale handle must round-trip within 1 %, worst was {worst}");
    }

    // ── Filmstrip (R14.3/R14.4, AC19) ───────────────────────────────────

    #[test]
    fn dragging_the_filmstrip_splitter_to_the_left_edge_closes_it() {
        let mut report = Vec::new();
        for w in [300.0f32, 128.0, 64.0, 41.0, 39.0, 4.0, 0.0] {
            let r = filmstrip_width_after_drag(w);
            report.push((w, r));
            println!("splitter dragged to {w:>5.0} pt -> {}", match r {
                Some(kept) => format!("still open at {kept:.0} pt"),
                None => "CLOSED (R14.4)".to_string(),
            });
        }
        assert!(report[..4].iter().all(|(_, r)| r.is_some()), "above the close threshold it resizes");
        assert!(report[4..].iter().all(|(_, r)| r.is_none()), "at the view's left edge it closes");
        assert_eq!(filmstrip_width_after_drag(1000.0), Some(FILMSTRIP_MAX_WIDTH), "clamped, never unbounded");
    }

    // ── Chrome geometry, fullscreen (R15, R16, AC5) ─────────────────────

    #[test]
    fn entering_fullscreen_hides_the_toolbar_and_gives_its_height_to_the_content() {
        let bounds = ViewRect::new(0.0, 0.0, 800.0, 600.0);
        let windowed = chrome_layout(bounds, &ChromeOpts::default());
        let full = chrome_layout(bounds, &ChromeOpts { fullscreen: true, ..Default::default() });
        println!(
            "windowed: toolbar {:?}, content {:.0} pt tall; fullscreen: toolbar {:?}, content {:.0} pt tall",
            windowed.toolbar.map(|t| t.h),
            windowed.content.h,
            full.toolbar.map(|t| t.h),
            full.content.h
        );
        assert!(windowed.toolbar.is_some(), "AC5: the toolbar shows when not fullscreen");
        assert!(full.toolbar.is_none(), "AC5: entering fullscreen hides the toolbar");
        assert_eq!(
            full.content.h - windowed.content.h,
            TOOLBAR_HEIGHT,
            "the hidden toolbar's height must go to the content, not be lost"
        );
    }

    #[test]
    fn the_slider_sits_bottom_right_directly_below_that_views_own_content() {
        let bounds = ViewRect::new(100.0, 50.0, 800.0, 600.0);
        let open = chrome_layout(
            bounds,
            &ChromeOpts { filmstrip: Some(FILMSTRIP_DEFAULT_WIDTH), ..Default::default() },
        );
        let strip = open.filmstrip.expect("the filmstrip must be placed when open");
        println!(
            "filmstrip x={:.0} w={:.0}; content x={:.0} w={:.0}; slider x={:.0} (content right {:.0})",
            strip.x, strip.w, open.content.x, open.content.w, open.slider.x, open.content.right()
        );
        assert_eq!(strip.x, bounds.x, "R14.3: the rail docks to the view's left edge");
        assert_eq!(open.content.x, strip.right(), "the content starts where the rail ends");
        assert!(open.slider.x > open.content.x, "R14.1: bottom-RIGHT of the content");
        assert!(open.slider.right() <= open.content.right(), "the slider stays inside its own content column");
        assert!(open.slider.y >= open.content.bottom(), "R14.1: directly BELOW the content");
    }

    #[test]
    fn streamed_layout_reserves_no_chrome_at_all() {
        let bounds = ViewRect::new(0.0, 0.0, 800.0, 600.0);
        let s = chrome_layout(bounds, &ChromeOpts { streamed: true, filmstrip: Some(200.0), ..Default::default() });
        println!(
            "Streamed: toolbar {:?}, filmstrip {:?}, content {:?}",
            s.toolbar.is_some(),
            s.filmstrip.is_some(),
            (s.content.w, s.content.h)
        );
        assert!(s.toolbar.is_none() && s.filmstrip.is_none(), "§8.8: no chrome, whatever was showing before");
        assert_eq!(s.content, bounds, "the whole rect is the one content pane");
    }

    // ── Scrolling: keys (R33/R33.1, AC31) ───────────────────────────────

    fn fresh(max: f32) -> ScrollKinetics {
        let mut k = ScrollKinetics::default();
        k.set_max(max);
        k.take_settled();
        k
    }

    /// Runs `secs` of a held ArrowDown and reports the average speed over
    /// the run, in points per second.
    fn hold_and_measure(kin: &mut ScrollKinetics, secs: f32, line: f32, viewport: f32) -> f32 {
        let held = KeyScrollInput { down_held: true, ..Default::default() };
        let before = kin.offset();
        let mut t = 0.0;
        while t < secs {
            kin.apply_keys(&held, DT, line, viewport);
            t += DT;
        }
        (kin.offset() - before) / t
    }

    #[test]
    fn a_held_arrow_key_starts_at_a_tap_and_winds_up_to_four_times_base() {
        let line = line_height(14.0);
        let viewport = 600.0;
        let mut kin = fresh(1_000_000.0);

        // The tap — one line, on the press frame, before any ramp exists.
        let tap = KeyScrollInput { down_tap: true, down_held: true, ..Default::default() };
        kin.apply_keys(&tap, DT, line, viewport);
        let tap_move = kin.offset();

        // 0 -> 0.25 s: still inside the repeat delay, so nothing but the tap.
        let during_delay = hold_and_measure(&mut kin, KEY_REPEAT_DELAY - DT, line, viewport);
        // Wind up to ~1 s held, then measure over the next 0.2 s.
        let to_one_second = 1.0 - kin.key_hold();
        hold_and_measure(&mut kin, to_one_second, line, viewport);
        let at_1s = hold_and_measure(&mut kin, 0.2, line, viewport);
        // Wind past the 2 s ramp, then measure at the ceiling.
        let to_ceiling = 2.6 - kin.key_hold();
        hold_and_measure(&mut kin, to_ceiling, line, viewport);
        let at_ceiling = hold_and_measure(&mut kin, 0.2, line, viewport);

        println!("AC31, measured at three points (FontSize 14 -> one line = {line:.1} pt):");
        println!("  tap                : {tap_move:.1} pt moved on the press frame");
        println!("  held  < 0.25 s     : {during_delay:.1} pt/s (the repeat delay)");
        println!("  held ~= 1.0 s      : {at_1s:.1} pt/s");
        println!("  held >= 2.0 s      : {at_ceiling:.1} pt/s (ceiling = {} pt/s)", KEY_BASE_SPEED * KEY_MAX_FACTOR);

        assert!((tap_move - line).abs() < 1e-3, "a tap moves exactly one line");
        assert!(during_delay < 1.0, "nothing moves during the repeat delay, measured {during_delay}");
        assert!(at_1s > KEY_BASE_SPEED, "by 1 s the ramp is above the base pace");
        assert!(at_ceiling > at_1s, "AC31: measurably faster by 2 s in");
        let ceiling = KEY_BASE_SPEED * KEY_MAX_FACTOR;
        assert!(
            (at_ceiling - ceiling).abs() / ceiling < 0.02,
            "the ramp caps at 4x base ({ceiling} pt/s), measured {at_ceiling}"
        );
    }

    #[test]
    fn page_and_home_and_end_move_by_the_documented_amounts() {
        let line = line_height(16.0);
        let viewport = 500.0;
        let expected_page = page_step(viewport, line);
        let mut kin = fresh(10_000.0);

        kin.apply_keys(&KeyScrollInput { page_down: true, ..Default::default() }, DT, line, viewport);
        let after_pgdn = kin.offset();
        kin.apply_keys(&KeyScrollInput { page_up: true, ..Default::default() }, DT, line, viewport);
        let after_pgup = kin.offset();
        kin.apply_keys(&KeyScrollInput { end: true, ..Default::default() }, DT, line, viewport);
        let after_end = kin.offset();
        kin.apply_keys(&KeyScrollInput { home: true, ..Default::default() }, DT, line, viewport);
        let after_home = kin.offset();

        println!(
            "R33.1 (viewport {viewport} pt, line {line:.1} pt, page step {expected_page:.1} pt): \
             PageDown -> {after_pgdn:.1}, PageUp -> {after_pgup:.1}, End -> {after_end:.1}, Home -> {after_home:.1}"
        );
        assert!((after_pgdn - expected_page).abs() < 1e-3, "a viewport minus two lines");
        assert_eq!(after_pgup, 0.0, "PageUp returns the same distance");
        assert_eq!(after_end, kin.max(), "End jumps to the bottom");
        assert_eq!(after_home, 0.0, "Home jumps to the top");
    }

    #[test]
    fn a_shrinking_viewport_pulls_the_offset_back_inside_the_new_limit() {
        let mut kin = fresh(5000.0);
        kin.set_offset(4800.0);
        kin.set_max(1000.0);
        println!("max 5000 -> 1000 with the view parked at 4800: offset is now {:.0}", kin.offset());
        assert_eq!(kin.offset(), 1000.0, "never left parked past the end");
    }

    // ── Scrolling: drag and throw (R33.2, AC32) ─────────────────────────

    /// A drag that moves the pointer `dy` points per frame for `frames`
    /// frames, then releases. Returns the glide speed it was let go at.
    fn drag_and_release(kin: &mut ScrollKinetics, dy: f32, frames: usize) -> f32 {
        let mut t = 0.0f64;
        let mut y = 500.0f32;
        kin.press(t, y);
        for _ in 0..frames {
            t += DT as f64;
            y += dy;
            kin.drag(t, y);
        }
        kin.release();
        kin.glide_speed()
    }

    /// Runs a glide to a stop, reporting `(frames, distance travelled)`.
    fn glide_to_rest(kin: &mut ScrollKinetics) -> (usize, f32) {
        let start = kin.offset();
        let mut frames = 0;
        while kin.is_gliding() && frames < 10_000 {
            kin.glide_tick(DT);
            frames += 1;
        }
        (frames, (kin.offset() - start).abs())
    }

    #[test]
    fn a_drag_pans_the_content_one_to_one_with_the_pointer() {
        let mut kin = fresh(10_000.0);
        kin.press(0.0, 500.0);
        kin.drag(DT as f64, 460.0); // pointer moved 40 pt UP
        kin.drag(2.0 * DT as f64, 430.0); // another 30 pt UP
        println!("pointer moved 70 pt up in two frames -> offset {:.1}", kin.offset());
        assert!((kin.offset() - 70.0).abs() < 1e-3, "R33.2: 1:1 with the pointer");
        kin.release();
    }

    #[test]
    fn a_throw_decelerates_under_constant_friction_to_exactly_zero() {
        let mut gentle = fresh(1_000_000.0);
        let gentle_v = drag_and_release(&mut gentle, -6.0, 12);
        let (gentle_frames, gentle_dist) = glide_to_rest(&mut gentle);

        let mut fast = fresh(1_000_000.0);
        let fast_v = drag_and_release(&mut fast, -24.0, 12);
        let (fast_frames, fast_dist) = glide_to_rest(&mut fast);

        println!("AC32, constant friction {THROW_FRICTION} pt/s^2:");
        println!(
            "  gentle throw: released at {:.0} pt/s -> {gentle_frames} frames ({:.2} s), travelled {gentle_dist:.0} pt",
            gentle_v.abs(),
            gentle_frames as f32 * DT
        );
        println!(
            "  fast   throw: released at {:.0} pt/s -> {fast_frames} frames ({:.2} s), travelled {fast_dist:.0} pt",
            fast_v.abs(),
            fast_frames as f32 * DT
        );
        assert!(fast_v.abs() > gentle_v.abs(), "a faster hand throws harder");
        assert!(fast_dist > gentle_dist, "AC32: a fast throw travels further");
        assert!(fast_frames > gentle_frames, "AC32: and takes longer — not a fixed-duration ease");
        assert_eq!(fast.glide_speed(), 0.0, "AC32: it ends at exactly zero velocity");
        assert_eq!(gentle.glide_speed(), 0.0);
    }

    #[test]
    fn a_throw_that_runs_into_the_end_stops_exactly_at_the_limit() {
        let mut kin = fresh(300.0);
        let v = drag_and_release(&mut kin, -40.0, 12);
        let (frames, dist) = glide_to_rest(&mut kin);
        println!(
            "thrown at {:.0} pt/s against a {:.0} pt range: stopped after {frames} frames at offset {:.1} (travelled {dist:.0})",
            v.abs(),
            kin.max(),
            kin.offset()
        );
        assert_eq!(kin.offset(), kin.max(), "AC32: exactly at the scroll limit");
        assert_eq!(kin.glide_speed(), 0.0, "and the throw is spent, not still pushing");
    }

    #[test]
    fn a_drag_that_stopped_before_release_throws_nothing() {
        let mut kin = fresh(10_000.0);
        let mut t = 0.0f64;
        kin.press(t, 500.0);
        // Move, then hold still for longer than the sample window.
        for i in 1..=10 {
            t += DT as f64;
            kin.drag(t, 500.0 - i as f32 * 10.0);
        }
        let moved_to = kin.offset();
        for _ in 0..12 {
            t += DT as f64;
            kin.drag(t, 400.0);
        }
        kin.release();
        println!(
            "dragged to offset {moved_to:.0}, then held still for {:.2} s before releasing -> glide {:.1} pt/s",
            12.0 * DT,
            kin.glide_speed()
        );
        assert_eq!(kin.glide_speed(), 0.0, "AC32: letting go of a stopped page throws nothing");
    }

    #[test]
    fn a_press_during_a_glide_stops_it_at_once() {
        let mut kin = fresh(1_000_000.0);
        let v = drag_and_release(&mut kin, -24.0, 12);
        for _ in 0..5 {
            kin.glide_tick(DT);
        }
        let mid = kin.glide_speed();
        kin.press(1.0, 200.0);
        println!(
            "released at {:.0} pt/s, still gliding at {:.0} pt/s after 5 frames; after a new press: {:.0} pt/s",
            v.abs(),
            mid.abs(),
            kin.glide_speed().abs()
        );
        assert!(mid.abs() > 0.0, "the glide must actually have been running");
        assert_eq!(kin.glide_speed(), 0.0, "AC32: catching a moving page stops it immediately");
    }

    // ── Settle events (R32) ─────────────────────────────────────────────

    #[test]
    fn onscrolled_settles_once_per_rest_never_on_a_glide_frame() {
        let mut kin = fresh(1_000_000.0);
        drag_and_release(&mut kin, -24.0, 12);
        // The rest arrives on the frame the glide itself ends, so "was it
        // still gliding *after* this tick" is what tells a mid-glide settle
        // from the at-rest one — not whether the loop is still running.
        let (mut mid_glide, mut at_rest, mut frames) = (0, 0, 0);
        loop {
            kin.glide_tick(DT);
            frames += 1;
            let still_gliding = kin.is_gliding();
            if kin.take_settled() {
                if still_gliding {
                    mid_glide += 1;
                } else {
                    at_rest += 1;
                }
            }
            if !still_gliding || frames >= 10_000 {
                break;
            }
        }
        println!("glide ran {frames} frames: {mid_glide} settle(s) mid-glide, {at_rest} at rest");
        assert_eq!(mid_glide, 0, "R32: onScrolled never fires mid-glide");
        assert_eq!(at_rest, 1, "R32: it fires once, when the content comes to rest");
        assert!(!kin.take_settled(), "and not again on any later frame");
    }

    #[test]
    fn settle_watch_reports_one_event_per_gesture_not_one_per_notch() {
        let mut watch = SettleWatch::new(100i64);
        let mut zoom = 100i64;
        let mut fired = 0;
        // A wheel gesture: five notches over five frames, still active.
        for _ in 0..5 {
            zoom = zoom_by_notches(zoom, 1.0);
            watch.observe(zoom, true);
            if watch.take_settled() {
                fired += 1;
            }
        }
        println!("five wheel notches to {zoom} %: {fired} event(s) so far");
        assert_eq!(fired, 0, "R32: nothing fires while the gesture is still running");
        // The gesture ends.
        watch.observe(zoom, false);
        assert!(watch.take_settled(), "R32: one event when it settles");
        assert!(!watch.take_settled(), "and only one");
        // A later, programmatic change settles immediately (no gesture).
        watch.observe(ZOOM_DEFAULT_PCT, false);
        let programmatic = watch.take_settled();
        println!("programmatic reset to {ZOOM_DEFAULT_PCT} %: fired = {programmatic}");
        assert!(programmatic, "R32: a programmatic change settles at once");
    }
}

/// Spec 058 T13 — the toolbar (R16/R17, AC10) and Save As naming (R18.1,
/// AC20). Every test reports the table it checked, so a reader finishes
/// knowing WHICH buttons and WHICH proposed names were exercised, not only
/// that a count matched.
#[cfg(test)]
mod toolbar_tests {
    use super::*;

    /// Every icon name published in the catalogue, flattened.
    fn catalogue() -> std::collections::BTreeSet<&'static str> {
        crate::icons::MENU_ICON_CATEGORIES
            .iter()
            .flat_map(|(_, names)| names.iter().copied())
            .collect()
    }

    /// **AC10** — "every toolbar icon is painter-drawn and carries a
    /// tooltip." Both halves, for every R16 item: the icon name must be one
    /// the catalogue actually publishes (so it draws rather than silently
    /// painting nothing), and the tooltip must be real text.
    #[test]
    fn every_toolbar_item_has_a_catalogue_icon_and_a_tooltip() {
        let known = catalogue();
        println!("R16's toolbar, in painted order:");
        for action in TOOLBAR_ITEMS {
            let icon = action.icon();
            let tip = action.default_tooltip();
            println!("  {:<13} icon={icon:<18} tooltip={tip:?}", action.as_str());
            assert!(known.contains(icon), "{} names an icon the catalogue does not publish: {icon}", action.as_str());
            assert!(!tip.trim().is_empty(), "{} has no tooltip (AC10)", action.as_str());
        }
        println!("{} buttons, all painter-drawn, all with tooltips", TOOLBAR_ITEMS.len());
        assert_eq!(TOOLBAR_ITEMS.len(), 12, "R16's list: layout, 2 view modes, 2 font sizes, filmstrip, split, find, fullscreen, print, share, save-as");
    }

    /// R16's own wording: "Zoom and card size are **not** toolbar items" —
    /// R14.1's single bottom-right slider is their only control.
    #[test]
    fn zoom_and_card_size_are_not_on_the_toolbar() {
        let icons: Vec<&str> = TOOLBAR_ITEMS.iter().map(|a| a.icon()).collect();
        println!("toolbar icons: {icons:?}");
        assert!(!icons.contains(&"zoom-in") && !icons.contains(&"zoom-out"), "R16: zoom is the slider's, not the toolbar's");
    }

    #[test]
    fn every_action_round_trips_through_its_stable_name() {
        for action in TOOLBAR_ITEMS {
            let back = ToolbarAction::from_str(action.as_str());
            println!("{:?} -> {:?} -> {back:?}", action, action.as_str());
            assert_eq!(back, Some(*action));
        }
        assert_eq!(ToolbarAction::from_str("no-such-button"), None);
    }

    #[test]
    fn a_narrow_toolbar_drops_buttons_rather_than_squeezing_them() {
        let mut report = Vec::new();
        for w in [1000.0f32, 400.0, 200.0, 90.0, 20.0] {
            let slots = toolbar_slots(ViewRect::new(0.0, 0.0, w, TOOLBAR_HEIGHT));
            let widths: Vec<f32> = slots.iter().map(|(_, r)| r.w).collect();
            println!("band {w:>6.0} pt -> {} button(s), widths {widths:?}", slots.len());
            assert!(widths.iter().all(|&x| (x - TOOLBAR_BUTTON).abs() < 0.01), "a drawn button is always full size");
            report.push(slots.len());
        }
        assert_eq!(report[0], TOOLBAR_ITEMS.len(), "a wide band shows every button");
        assert!(report.windows(2).all(|w| w[1] <= w[0]), "a narrower band never shows more");
        assert_eq!(*report.last().unwrap(), 0, "a band too narrow for even one button shows none");
    }

    #[test]
    fn the_layout_button_cycles_the_four_document_layouts() {
        let mut seen = vec!["Raw".to_string()];
        let mut cur = "Raw".to_string();
        for _ in 0..4 {
            cur = next_layout(&cur).to_string();
            seen.push(cur.clone());
        }
        println!("layout cycle: {}", seen.join(" -> "));
        assert_eq!(seen, ["Raw", "Web", "Print", "Page", "Raw"], "R7's four document layouts, in order");
        println!("from Streamed -> {}", next_layout("Streamed"));
        assert_eq!(next_layout("Streamed"), "Web", "§8.8's Streamed is a developer's choice, not a stop on the cycle");
    }

    #[test]
    fn font_size_steps_through_the_ladder_and_stops_at_both_ends() {
        let mut up = vec![14i64];
        let mut v = 14;
        for _ in 0..20 {
            v = font_size_step(v, true);
            up.push(v);
        }
        let mut down = vec![14i64];
        v = 14;
        for _ in 0..20 {
            v = font_size_step(v, false);
            down.push(v);
        }
        println!("larger  from 14: {:?}", up);
        println!("smaller from 14: {:?}", down);
        assert_eq!(*up.last().unwrap(), *FONT_SIZES.last().unwrap(), "stops at the top of the ladder");
        assert_eq!(*down.last().unwrap(), FONT_SIZES[0], "and at the bottom");
        assert!(up.windows(2).all(|w| w[1] >= w[0]) && down.windows(2).all(|w| w[1] <= w[0]));
    }

    #[test]
    fn an_installed_tooltip_table_replaces_the_english_one() {
        assert_eq!(toolbar_tooltip(ToolbarAction::Print), "Print", "English is what this crate ships");
        set_toolbar_tooltips(&[(ToolbarAction::Print, "Imprimir".to_string())]);
        let translated = toolbar_tooltip(ToolbarAction::Print);
        let untouched = toolbar_tooltip(ToolbarAction::Share);
        println!("after installing a table: Print={translated:?}, Share={untouched:?}");
        assert_eq!(translated, "Imprimir");
        assert_eq!(untouched, "Share", "an action left out of the table keeps its English");
        set_toolbar_tooltips(&[]);
        assert_eq!(toolbar_tooltip(ToolbarAction::Print), "Print", "an empty table restores English");
    }

    // ── R18.1 / AC20: the proposed Save As filename ─────────────────────

    /// **AC20**, first clause — "the document's first three words plus the
    /// extension matching its `Format`".
    #[test]
    fn a_loadbytes_document_is_named_after_its_first_three_words() {
        let cases: &[(ViewerFormat, &str, &str)] = &[
            (ViewerFormat::Text, "Quarterly sales report for the north region", "Quarterly-sales-report.txt"),
            (ViewerFormat::Markdown, "# Release Notes\n\nEverything that changed.", "Release-Notes-Everything.md"),
            (ViewerFormat::Pdf, "INVOICE 2026-0417 Acme Industrial", "INVOICE-2026-0417-Acme.pdf"),
            (ViewerFormat::HtmlSubset, "   leading   whitespace   only   ", "leading-whitespace-only.html"),
        ];
        for (format, text, want) in cases {
            let got = default_save_name(*format, Some(text));
            println!("{format} + {:?} -> proposed {got:?}", &text[..text.len().min(38)]);
            assert_eq!(&got, want);
        }
    }

    /// **AC20**, second clause — "an image or other textless document falls
    /// back to a generic name plus the correct extension."
    #[test]
    fn a_textless_document_falls_back_to_a_generic_name() {
        let cases: &[(ViewerFormat, Option<&str>, &str)] = &[
            (ViewerFormat::Image, None, "document.png"),
            (ViewerFormat::Image, Some("   "), "document.png"),
            // Punctuation is not a word: a name made of hyphens would be
            // worse than the generic one.
            (ViewerFormat::Pdf, Some("... --- ???"), "document.pdf"),
            (ViewerFormat::Text, Some(""), "document.txt"),
        ];
        for (format, text, want) in cases {
            let got = default_save_name(*format, *text);
            println!("{format} + {text:?} -> proposed {got:?}");
            assert_eq!(&got, want);
        }
    }

    /// **AC20**, third clause — "the user can edit the name, and the correct
    /// extension is restored at save time even if the user deletes it."
    #[test]
    fn the_correct_extension_is_restored_whatever_the_user_typed() {
        let cases: &[(&str, ViewerFormat, &str)] = &[
            ("my report", ViewerFormat::Pdf, "my report.pdf"),
            ("my report.pdf", ViewerFormat::Pdf, "my report.pdf"),
            ("MY REPORT.PDF", ViewerFormat::Pdf, "MY REPORT.PDF"),
            // A different extension is KEPT and the right one added: silently
            // renaming .pdf to .txt would misdescribe bytes R18 writes
            // unmodified.
            ("archive.txt", ViewerFormat::Pdf, "archive.txt.pdf"),
            ("", ViewerFormat::Markdown, "document.md"),
            ("   ", ViewerFormat::Image, "document.png"),
            (".hidden", ViewerFormat::Text, ".hidden.txt"),
        ];
        for (typed, format, want) in cases {
            let got = ensure_extension(typed, *format);
            println!("user typed {typed:?} ({format}) -> saved as {got:?}");
            assert_eq!(&got, want);
        }
    }

    #[test]
    fn every_format_has_its_own_extension() {
        let all = [
            ViewerFormat::Text,
            ViewerFormat::Markdown,
            ViewerFormat::Image,
            ViewerFormat::Pdf,
            ViewerFormat::HtmlSubset,
        ];
        let exts: Vec<&str> = all.iter().map(|f| extension_for(*f)).collect();
        for (f, e) in all.iter().zip(&exts) {
            println!("{f} saves as .{e}");
        }
        let unique: std::collections::BTreeSet<&&str> = exts.iter().collect();
        assert_eq!(unique.len(), exts.len(), "two formats sharing an extension would misname one of them");
    }
}
