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
