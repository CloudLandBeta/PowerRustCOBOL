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

/// How a [`PageSpan`] in a [`DocumentIndex`] is to be read.
///
/// Plain text's pages **are** byte ranges of the file, which is what lets
/// [`decode_text_page`] jump to the last page of a multi-gigabyte log for
/// the cost of one page. A PDF's are not: its pages live inside compressed
/// object streams, and handing those offsets to a byte reader would hand it
/// the middle of a Flate stream. Saying which it is, per index, is cheaper
/// than every caller inferring it from `format`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PageAddressing {
    /// `start`/`end` are byte offsets into the source.
    Bytes,
    /// `start` is a 1-based logical page NUMBER and `end` is `start + 1`.
    LogicalPages,
}

/// A document's page-break index, built once by [`index_text`] (or
/// [`index_pdf`]).
#[derive(Debug, Clone)]
pub struct DocumentIndex {
    pub format: ViewerFormat,
    pub pages: Vec<PageSpan>,
    pub total_len: u64,
    pub addressing: PageAddressing,
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
    Ok(DocumentIndex {
        format: ViewerFormat::Text,
        pages,
        total_len,
        addressing: PageAddressing::Bytes,
    })
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
        // The HTML subset is one flow: its "pages" are a paginated
        // layout's, computed at paint time, not byte ranges of the source
        // (T21). One index entry keeps `page_count()` honest until a
        // paginating layout exists to say otherwise.
        ViewerFormat::HtmlSubset => {
            let total_len = source_len(source)?;
            let mut on_progress = on_progress;
            on_progress(100);
            Ok(DocumentIndex {
                format: ViewerFormat::HtmlSubset,
                pages: vec![PageSpan { start: 0, end: total_len }],
                total_len,
                addressing: PageAddressing::Bytes,
            })
        }
        // R9's page breaks for a PDF are its own page boundaries — the
        // document says where they are, so nothing is computed.
        ViewerFormat::Pdf => {
            let bytes = read_all(source)?;
            let doc = parse_pdf(&bytes).map_err(ViewerLoadError::Io)?;
            let mut on_progress = on_progress;
            on_progress(100);
            Ok(index_pdf(&doc, bytes.len() as u64))
        }
        other => Err(ViewerLoadError::NotYetImplemented(other)),
    }
}

/// Every byte of a source. Used only where a format's reader needs the whole
/// file — a PDF's cross-reference table lives at its END, so there is no
/// streaming read of one, unlike plain text's.
fn read_all(source: &DocumentSource) -> io::Result<Vec<u8>> {
    match source {
        DocumentSource::Path(path) => std::fs::read(path),
        DocumentSource::Bytes(bytes) => Ok(bytes.to_vec()),
    }
}

/// R9 for a PDF: one index entry per page, addressed by page NUMBER.
pub fn index_pdf(doc: &PdfDocument, total_len: u64) -> DocumentIndex {
    let pages = doc
        .pages
        .iter()
        .map(|p| PageSpan { start: p.number as u64, end: p.number as u64 + 1 })
        .collect();
    DocumentIndex {
        format: ViewerFormat::Pdf,
        pages,
        total_len,
        addressing: PageAddressing::LogicalPages,
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
    /// §3's "colours", as an HTML colour string (`#rrggbb`, `red`, …) —
    /// set only by the HTML subset (T21), which is the only format that
    /// carries one. `None` means "the theme's own ink", which is what every
    /// Markdown run means.
    pub color: Option<String>,
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
    /// A ```` ```mermaid ```` fence (T20). Its own block rather than a
    /// `CodeBlock` with a language, so the painter never has to sniff a
    /// fence's language to know whether to draw a diagram or a listing.
    Mermaid { source: String },
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
                    Block::Mermaid { .. } => "Mermaid",
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
                    Block::CodeBlock { .. }
                    | Block::ThematicBreak
                    | Block::Mermaid { .. }
                    | Block::RawHtml(_) => {}
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
                        let is_mermaid = language
                            .as_deref()
                            .is_some_and(|l| l.trim().eq_ignore_ascii_case("mermaid"));
                        let block = if is_mermaid {
                            Block::Mermaid { source: text }
                        } else {
                            Block::CodeBlock { language, text }
                        };
                        push_block(&mut stack, &mut doc_blocks, block);
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
                        color: None,
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
                        color: None,
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

/// The zoom levels the **slider** lands on.
///
/// Not a nicety — a measurement. One Viewer paint of a 360-block document costs
/// **1.53 ms** when the zoom has not moved, because egui caches a laid-out
/// galley and the cache key is the font size. Give it a new font size every
/// frame and the whole document is laid out again from scratch: **145 ms**, a
/// 95× difference and about seven frames a second. That is the whole of "Zoom
/// slider is too slow" (operator, 2026-09-20).
///
/// A thread cannot fix it — text layout belongs to the UI thread and the
/// galleys are wanted for this frame — but a LADDER can. Dragging the whole
/// range now visits at most these levels instead of one per frame, each is laid
/// out once, and dragging back over ground already covered costs nothing at
/// all.
///
/// The stops are the ones a document viewer offers anyway, so the slider also
/// stops landing on 137 %.
///
/// The wheel and the double-click keep their own continuous `ZOOM_STEP_RATIO`
/// ladder: a notch is already a discrete jump, so it never produced the stream
/// of distinct sizes a drag does.
pub const ZOOM_STOPS: &[i64] = &[
    25, 33, 50, 67, 75, 80, 90, 100, 110, 125, 150, 175, 200, 250, 300, 400, 500, 600, 800, 1000,
    1200, 1600,
];

/// The stop nearest `pct`, on a **log** scale — the scale the slider itself
/// uses, and the one the eye judges a zoom on: 90 is as far from 100 as 111 is,
/// not as far as 110.
pub fn nearest_zoom_stop(pct: i64) -> i64 {
    let target = (clamp_zoom(pct) as f64).ln();
    *ZOOM_STOPS
        .iter()
        .min_by(|a, b| {
            let da = ((**a as f64).ln() - target).abs();
            let db = ((**b as f64).ln() - target).abs();
            da.partial_cmp(&db).unwrap_or(std::cmp::Ordering::Equal)
        })
        .unwrap_or(&ZOOM_DEFAULT_PCT)
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
            // Snapped to a stop — see `ZOOM_STOPS` for the 95× measurement that
            // put it there. The exponential placement is unchanged; only where
            // it is allowed to come to rest.
            (
                nearest_zoom_stop((lo * (hi / lo).powf(t)).round() as i64),
                card_size_pct,
            )
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
    /// R26 — the Find bar is open, so it takes a band of its own between
    /// the toolbar and the content.
    pub find_open: bool,
}

impl Default for ChromeOpts {
    fn default() -> Self {
        Self { fullscreen: false, streamed: false, filmstrip: None, find_open: false }
    }
}

/// Where each piece of a view's chrome lands inside `bounds`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ChromeLayout {
    /// The toolbar band — `None` under fullscreen (R15) or `Streamed`.
    pub toolbar: Option<ViewRect>,
    /// The Find bar (R26) — `None` when closed, and always `None` under
    /// `Streamed`, which shows no Find chrome at all (§8.8/AC25).
    pub find_bar: Option<ViewRect>,
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
/// docking order. One function, used by every surface, so each of those facts
/// is checked once rather than repeated at each call site.
///
/// **`fullscreen` no longer takes the toolbar away**, and the reason is that
/// what fullscreen MEANS changed (operator, 2026-09-20: "Fullscreen is not
/// working as it is supposed to. It is maximizing the view inside the viewer
/// instead of the entire screen"). Hiding the toolbar bought the document a
/// toolbar's height out of a control; it buys nothing out of a whole screen,
/// and it costs the only visible way back — a fullscreen view whose exit is an
/// unadvertised `Esc` is a trap. The button stays, drawn pressed, and pressing
/// it returns.
///
/// The field is kept because a caller still says whether this view is the
/// fullscreen one; it simply no longer decides the chrome.
pub fn chrome_layout(bounds: ViewRect, opts: &ChromeOpts) -> ChromeLayout {
    if opts.streamed {
        return ChromeLayout {
            toolbar: None,
            find_bar: None,
            filmstrip: None,
            content: bounds,
            slider: ViewRect::new(bounds.x, bounds.bottom(), 0.0, 0.0),
        };
    }

    let mut y = bounds.y;
    let mut remaining_h = bounds.h;

    let toolbar = if remaining_h < TOOLBAR_HEIGHT * 2.0 {
        None
    } else {
        let r = ViewRect::new(bounds.x, y, bounds.w, TOOLBAR_HEIGHT);
        y += TOOLBAR_HEIGHT;
        remaining_h -= TOOLBAR_HEIGHT;
        Some(r)
    };

    // R26: the bar sits under the toolbar, above the content. It never
    // depended on the toolbar being there — Find is a reader's tool, not part
    // of the chrome — which is why nothing here changed when fullscreen
    // stopped taking the toolbar away.
    let find_bar = if opts.find_open && remaining_h > FIND_BAR_HEIGHT * 2.0 {
        let r = ViewRect::new(bounds.x, y, bounds.w, FIND_BAR_HEIGHT);
        y += FIND_BAR_HEIGHT;
        remaining_h -= FIND_BAR_HEIGHT;
        Some(r)
    } else {
        None
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

    ChromeLayout { toolbar, find_bar, filmstrip, content, slider }
}

// ── Conversation mode (T23: §8.1, §8.2, AC12, AC13, AC16, AC17) ─────────
//
// §8 drives the same control as an **append-only, self-following
// conversation surface**. The rule that shapes everything here is §8.2
// item 5: "Avoid rebuilding or reparsing the entire conversation." So an
// append lays out **only the chunk that arrived**, and extending a message
// re-lays-out **only that message** — never the stream. [`Conversation::
// relayouts`] counts it, so T27's performance claim is a measurement.
//
// **Storage stays native, per chunk.** tasks.md's preamble describes a
// conversation as "an assembled stream of heterogeneous chunks" whose
// substrate for stitching is HTML. Applied one level down — which is
// plan.md §3's own rule — each chunk keeps **its own** form (HTML text,
// Markdown text, or raw literal text) together with the mode it arrived
// in, and the stream is assembled from them on demand
// ([`Conversation::to_html`]). Converting Markdown to HTML on arrival
// would throw away the source for nothing: the layout is derived from the
// chunk either way, and a Markdown→HTML serializer is pure loss.

/// How an appended chunk is to be treated (§8.1). **Specified per append,
/// never inferred from the content** (§8.5): content received as `Raw`
/// stays raw even if it contains valid markup.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AppendMode {
    Html,
    Markdown,
    #[default]
    Raw,
}

impl AppendMode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Html => "Html",
            Self::Markdown => "Markdown",
            Self::Raw => "Raw",
        }
    }

    /// Lenient, and **`Raw` is the fallback** — the safe reading of an
    /// unrecognised mode is "show it, do not interpret it".
    pub fn from_str(s: &str) -> Self {
        match s.trim().to_ascii_lowercase().as_str() {
            "html" => Self::Html,
            "markdown" | "md" => Self::Markdown,
            _ => Self::Raw,
        }
    }
}

/// One chunk exactly as it arrived (§8.5: "the rendering mode must be
/// specified for each append operation rather than inferred").
#[derive(Debug, Clone, PartialEq)]
pub struct MessageChunk {
    pub mode: AppendMode,
    pub content: String,
}

/// One conversation message, identified by a **stable id** (§8.6) so
/// streamed chunks can extend it.
#[derive(Debug, Clone, PartialEq)]
pub struct ConversationMessage {
    pub id: String,
    /// Every chunk this message is made of, in arrival order.
    pub chunks: Vec<MessageChunk>,
    /// The derived layout for this message alone.
    pub blocks: Vec<Block>,
    /// How many of `blocks` the LAST chunk produced. A streamed chunk that
    /// merges into that chunk re-lays-out only those, never the message's
    /// whole history — the difference between "append to a message" and
    /// "rebuild a message" (§8.2 item 5).
    last_chunk_block_count: usize,
}

/// An append-only conversation (§8).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Conversation {
    pub messages: Vec<ConversationMessage>,
    /// §8.1/§8.4's blanket override (plan.md §7's flagged reading, and T25's
    /// to confirm): **when false, every append behaves as `Raw`** whichever
    /// mode was actually called, as a safety guarantee for a program
    /// showing untrusted content. Default true.
    render_as_html: bool,
    /// Messages laid out since this conversation was created — the number
    /// §8.2 item 5 and T27 are actually about. An append that rebuilt the
    /// stream would show up here as a jump, not as a +1.
    relayouts: usize,
    auto_id: u64,
}

impl Conversation {
    pub fn new() -> Self {
        Self { messages: Vec::new(), render_as_html: true, relayouts: 0, auto_id: 0 }
    }

    pub fn render_as_html(&self) -> bool {
        self.render_as_html
    }

    /// §8.1/§8.4: turning this off makes every subsequent append `Raw`.
    /// Content already appended is **not** reinterpreted — §8.1's own words,
    /// "the viewer must not automatically reinterpret the accumulated raw
    /// content as HTML unless explicitly requested".
    pub fn set_render_as_html(&mut self, on: bool) {
        self.render_as_html = on;
    }

    pub fn is_empty(&self) -> bool {
        self.messages.is_empty()
    }

    pub fn relayouts(&self) -> usize {
        self.relayouts
    }

    pub fn clear(&mut self) {
        self.messages.clear();
    }

    /// §8.6's "support pruning or virtualisation if conversation size
    /// becomes excessive."
    ///
    /// Pruning, not virtualisation: a conversation surface's memory has to
    /// be bounded by something the developer chooses, and the oldest
    /// messages are the ones a reader has already scrolled past. Returns
    /// how many were dropped. A ceiling of `0` means "never prune".
    pub fn prune_to(&mut self, ceiling: usize) -> usize {
        if ceiling == 0 || self.messages.len() <= ceiling {
            return 0;
        }
        let excess = self.messages.len() - ceiling;
        self.messages.drain(..excess);
        excess
    }

    /// The id of the message a streamed reply is currently extending — the
    /// last one — so a host that lost track can ask instead of guessing.
    pub fn current_message_id(&self) -> Option<&str> {
        self.messages.last().map(|m| m.id.as_str())
    }

    /// The mode an append actually uses, after §8.1's override.
    fn effective_mode(&self, requested: AppendMode) -> AppendMode {
        if self.render_as_html {
            requested
        } else {
            AppendMode::Raw
        }
    }

    /// Append a new message and return its id (§8.2). The id is generated
    /// here when the caller does not supply one, and is **stable** for the
    /// life of the conversation.
    pub fn append(&mut self, mode: AppendMode, content: &str) -> String {
        self.auto_id += 1;
        let id = format!("m{}", self.auto_id);
        self.append_with_id(&id, mode, content);
        id
    }

    /// Append a new message under a caller-chosen id.
    pub fn append_with_id(&mut self, id: &str, mode: AppendMode, content: &str) {
        let mode = self.effective_mode(mode);
        let blocks = layout_chunk(mode, content);
        self.relayouts += 1;
        self.messages.push(ConversationMessage {
            id: id.to_string(),
            chunks: vec![MessageChunk { mode, content: content.to_string() }],
            last_chunk_block_count: blocks.len(),
            blocks,
        });
    }

    /// §8.2's `append_to_message` — extend an existing message by id.
    ///
    /// Returns `false` when no message carries that id: a streamed chunk for
    /// a message that never started is a caller error worth reporting, not
    /// something to silently mint a new message for.
    ///
    /// Only **this message** is laid out again, and only its newly appended
    /// chunk is parsed — the rest of the conversation is not touched, which
    /// is §8.2 item 5 in one line.
    pub fn append_to_message(&mut self, id: &str, mode: AppendMode, content: &str) -> bool {
        let mode = self.effective_mode(mode);
        let Some(msg) = self.messages.iter_mut().find(|m| m.id == id) else {
            return false;
        };
        // Consecutive chunks of the SAME mode are merged before layout, so
        // a token-at-a-time stream does not end up as a thousand one-word
        // paragraphs (§8.6's batching, made structural).
        let new_blocks = match msg.chunks.last_mut() {
            Some(last) if last.mode == mode => {
                last.content.push_str(content);
                let merged = last.content.clone();
                Some((true, layout_chunk(mode, &merged)))
            }
            _ => {
                msg.chunks.push(MessageChunk { mode, content: content.to_string() });
                Some((false, layout_chunk(mode, content)))
            }
        };
        if let Some((merged, blocks)) = new_blocks {
            if merged {
                // Re-lay-out just the tail chunk this message ends with.
                let keep = msg
                    .blocks
                    .len()
                    .saturating_sub(msg.last_chunk_block_count);
                msg.blocks.truncate(keep);
            }
            msg.last_chunk_block_count = blocks.len();
            msg.blocks.extend(blocks);
        }
        self.relayouts += 1;
        true
    }

    /// Every message's text, in arrival order — what Find searches and what
    /// a "is this conversation empty" check reads.
    pub fn text(&self) -> String {
        self.messages
            .iter()
            .map(|m| LayoutDocument { blocks: m.blocks.clone() }.searchable_text().unwrap_or_default())
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// The assembled HTML stream (tasks.md's preamble) — built on demand
    /// from chunks that each kept their own form, never held as a second
    /// copy that could drift from them.
    pub fn to_html(&self) -> String {
        let mut out = String::new();
        for msg in &self.messages {
            out.push_str(&format!("<div data-message=\"{}\">", escape_html(&msg.id)));
            for chunk in &msg.chunks {
                match chunk.mode {
                    AppendMode::Html => out.push_str(&chunk.content),
                    // Markdown's own text is kept verbatim inside a marked
                    // element: converting it here would make this method the
                    // second place that decides what Markdown means.
                    AppendMode::Markdown => out.push_str(&format!(
                        "<div data-markdown=\"1\">{}</div>",
                        escape_html(&chunk.content)
                    )),
                    AppendMode::Raw => {
                        out.push_str(&format!("<pre>{}</pre>", escape_html(&chunk.content)))
                    }
                }
            }
            out.push_str("</div>");
        }
        out
    }

    /// All of this conversation's blocks, in order — what the painter draws.
    pub fn blocks(&self) -> Vec<Block> {
        self.messages.iter().flat_map(|m| m.blocks.clone()).collect()
    }
}

/// One chunk's derived layout, in the mode it arrived as.
fn layout_chunk(mode: AppendMode, content: &str) -> Vec<Block> {
    match mode {
        AppendMode::Html => parse_html(content).blocks,
        AppendMode::Markdown => parse_markdown(content).blocks,
        // §8.1/§8.5: raw is "inserted as an escaped text node or inside an
        // appropriate element such as `<pre>`". A `CodeBlock` IS this
        // model's `<pre>`: monospace, whitespace preserved, and — because
        // it is never parsed — markup inside it is displayed, not
        // interpreted, however valid it is.
        AppendMode::Raw => vec![Block::CodeBlock { language: None, text: content.to_string() }],
    }
}

// ── Conversation history (T36: §8.8, AC26–AC29) ─────────────────────────

/// History holds at most this many entries (§8.8); past it, the oldest is
/// evicted.
pub const HISTORY_CAP: usize = 10;

/// One history entry — **an id and a title, never a conversation's
/// content** (§8.8, plan.md §3/§4).
///
/// That is the whole design: selecting a past conversation is always a
/// fresh request back to the host (`onConversationSelected`), never a cache
/// restore, which is what keeps a long Streamed-layout session's memory
/// bounded the same way the page cache bounds one large document (R2).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConversationEntry {
    pub id: String,
    pub title: String,
}

/// §8.8's history: up to [`HISTORY_CAP`] entries, oldest evicted first.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ConversationHistory {
    entries: std::collections::VecDeque<ConversationEntry>,
}

impl ConversationHistory {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn ids(&self) -> Vec<String> {
        self.entries.iter().map(|e| e.id.clone()).collect()
    }

    pub fn contains(&self, id: &str) -> bool {
        self.entries.iter().any(|e| e.id == id)
    }

    /// Add an entry, evicting the oldest past the cap. Returns the evicted
    /// id, if one went.
    ///
    /// An id already in history is **moved**, not duplicated: archiving a
    /// conversation twice must not put it in the list twice.
    pub fn push(&mut self, entry: ConversationEntry) -> Option<String> {
        self.entries.retain(|e| e.id != entry.id);
        self.entries.push_back(entry);
        if self.entries.len() > HISTORY_CAP {
            return self.entries.pop_front().map(|e| e.id);
        }
        None
    }

    /// Take an entry out — §8.8's `SelectConversation` removes the selected
    /// id from history as it becomes current.
    pub fn take(&mut self, id: &str) -> Option<ConversationEntry> {
        let i = self.entries.iter().position(|e| e.id == id)?;
        self.entries.remove(i)
    }

    /// §8.8's `HistoryList` — "one entry per line as `id|title`", the same
    /// multi-line-list convention `Buttons` already uses on Snackbar.
    pub fn to_list(&self) -> String {
        self.entries
            .iter()
            .map(|e| format!("{}|{}", e.id, e.title))
            .collect::<Vec<_>>()
            .join("\n")
    }
}

// ── Auto-follow and the new-content indicator (T24/T25: §8.3, §8.4) ─────

/// §8.3's threshold — "a small threshold (**24–32 px**) should be used so
/// that minor rounding or layout differences do not incorrectly disable
/// automatic scrolling." The middle of that range.
pub const AUTO_FOLLOW_THRESHOLD: f32 = 28.0;

/// §8.3's auto-follow and §8.4's new-content indicator, as one state
/// machine — they are two faces of the same question ("is the reader at the
/// end?"), and splitting them is how they drift apart.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AutoFollow {
    active: bool,
    pending: bool,
}

impl Default for AutoFollow {
    fn default() -> Self {
        // A conversation starts at its end, because it starts empty.
        Self { active: true, pending: false }
    }
}

impl AutoFollow {
    pub fn is_active(&self) -> bool {
        self.active
    }

    /// §8.4 — content arrived while the reader was not at the end, so the
    /// "Jump to latest" affordance should be showing.
    pub fn has_pending(&self) -> bool {
        self.pending
    }

    /// Observe the viewport **before** the append (§8.3's own emphasis: "the
    /// decision must be based on the scroll position before the new content
    /// changes the document height").
    ///
    /// This is also how §8.3's "automatic following **resumes** as soon as
    /// the user manually returns to the end" happens: returning to the end
    /// is simply an observation that finds them there.
    pub fn observe(&mut self, offset: f32, max: f32) {
        let at_end = (max - offset) <= AUTO_FOLLOW_THRESHOLD;
        self.active = at_end;
        if at_end {
            self.pending = false;
        }
    }

    /// The viewport offset after the document's height changed — an append,
    /// or a late layout change such as an image finishing its decode
    /// (§8.3's last paragraph, AC18).
    ///
    /// Pinned to the new end while following; otherwise the reader's
    /// position is preserved exactly, clamped only by the new limit.
    pub fn after_height_change(&mut self, offset: f32, new_max: f32) -> f32 {
        if self.active {
            new_max.max(0.0)
        } else {
            // Content arrived somewhere the reader cannot see it.
            self.pending = true;
            offset.clamp(0.0, new_max.max(0.0))
        }
    }

    /// §8.4's "Jump to latest": scroll to the end, clear the indicator, and
    /// re-enable automatic following — all three, in that order.
    pub fn jump_to_latest(&mut self, max: f32) -> f32 {
        self.active = true;
        self.pending = false;
        max.max(0.0)
    }
}

/// §8.5's escaping, for [`Conversation::to_html`] and for any caller that
/// needs a literal string to survive an HTML context.
pub fn escape_html(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            _ => out.push(c),
        }
    }
    out
}

// ── HTML subset (T21: R7, AC2) ──────────────────────────────────────────
//
// §3's contract: "**Subset renderer**: block/inline layout, common
// typography, colours, borders, tables, images"; **not delivered** — CSS3
// grid/flex/animation/transform, JavaScript, floats beyond the simple case.
// **Not a browser.**
//
// plan.md §4's decision made concrete: HTML maps onto the **same** layout
// primitives the Markdown walker produces — [`Block`] and [`Inline`] — so
// this milestone's cost is "parse + map", not a second layout engine. Every
// rendering fix made for Markdown is therefore a fix for HTML too, and
// neither can drift from the other.
//
// An element this subset does not know is **descended into** rather than
// dropped: a `<div>`, a `<section>`, a `<main>` contribute their children.
// That is what "degrade to the supported subset" means in practice — a
// grid-laid-out page loses its grid and keeps its content.

/// The shared layout model. Named for the format that first produced it
/// (T9's Markdown walker); HTML produces exactly the same shape, which is
/// plan.md §4's decision expressed in the type system rather than in prose.
pub type LayoutDocument = MarkdownDocument;

/// Elements whose CONTENT must never be rendered — script and style are not
/// prose, and showing their source would be worse than dropping it. (§8.5's
/// sanitisation rests on the same list, from the other direction.)
const HTML_DROPPED: &[&str] = &[
    "script", "style", "head", "title", "meta", "link", "noscript", "iframe", "object", "embed",
    "applet", "frame", "frameset", "form", "input", "button", "textarea", "select",
];

/// §8.5 — inline event handlers.
///
/// They are neutralised **structurally** rather than by stripping: this
/// walker reads only `href`, `src`, `alt`, `title`, `color`, `style` and
/// `start`, so an `onclick` (or any other `on*`) is never looked at and can
/// never reach anything that could run it. [`html_has_event_handler`] exists
/// so a test can prove that, and so a future attribute reader cannot quietly
/// widen the surface without this rule noticing.
pub fn html_has_event_handler(tag_attributes: &[(&str, &str)]) -> bool {
    tag_attributes
        .iter()
        .any(|(k, _)| k.trim().to_ascii_lowercase().starts_with("on"))
}

/// Parse an HTML document into the shared layout model (R7, §3's subset).
///
/// Never fails: HTML a COBOL program received from a `RestClient` is not
/// guaranteed well-formed, and refusing to show a page because a tag was
/// unclosed would be the wrong answer for a *viewer*. Whatever parses,
/// renders.
pub fn parse_html(html: &str) -> LayoutDocument {
    let Ok(dom) = tl::parse(html, tl::ParserOptions::default()) else {
        return LayoutDocument::default();
    };
    let parser = dom.parser();
    let mut walker = HtmlWalker { parser, blocks: Vec::new(), inline: Vec::new() };
    for handle in dom.children() {
        walker.node(*handle, &TextStyle::default());
    }
    walker.flush_paragraph();
    LayoutDocument { blocks: walker.blocks }
}

struct HtmlWalker<'a, 'p> {
    parser: &'p tl::Parser<'a>,
    blocks: Vec<Block>,
    /// Inline runs seen outside any block element — flushed into a
    /// paragraph when a block boundary arrives, so loose text in a `<div>`
    /// is not lost.
    inline: Vec<Inline>,
}

impl HtmlWalker<'_, '_> {
    fn flush_paragraph(&mut self) {
        if self.inline.iter().any(|i| match i {
            Inline::Text { text, .. } => !text.trim().is_empty(),
            _ => true,
        }) {
            let content = std::mem::take(&mut self.inline);
            self.blocks.push(Block::Paragraph { content });
        } else {
            self.inline.clear();
        }
    }

    /// Collect an element's children as inline content, under `style`.
    fn inline_of(&mut self, tag: &tl::HTMLTag<'_>, style: &TextStyle) -> Vec<Inline> {
        let saved = std::mem::take(&mut self.inline);
        for child in tag.children().top().iter() {
            self.node(*child, style);
        }
        std::mem::replace(&mut self.inline, saved)
    }

    /// Collect an element's children as blocks (a list item, a quote, a
    /// table cell's block content).
    fn blocks_of(&mut self, tag: &tl::HTMLTag<'_>, style: &TextStyle) -> Vec<Block> {
        let saved_blocks = std::mem::take(&mut self.blocks);
        let saved_inline = std::mem::take(&mut self.inline);
        for child in tag.children().top().iter() {
            self.node(*child, style);
        }
        self.flush_paragraph();
        let out = std::mem::replace(&mut self.blocks, saved_blocks);
        self.inline = saved_inline;
        out
    }

    fn attr(tag: &tl::HTMLTag<'_>, name: &str) -> Option<String> {
        tag.attributes().get(name).flatten().map(|v| v.as_utf8_str().into_owned())
    }

    fn node(&mut self, handle: tl::NodeHandle, style: &TextStyle) {
        let Some(node) = handle.get(self.parser) else { return };
        if let Some(raw) = node.as_raw() {
            let text = decode_html_entities(&raw.as_utf8_str());
            if !text.is_empty() {
                self.inline.push(Inline::Text { text, style: style.clone() });
            }
            return;
        }
        let Some(tag) = node.as_tag() else { return };
        let name = tag.name().as_utf8_str().to_ascii_lowercase();

        if HTML_DROPPED.contains(&name.as_str()) {
            return;
        }

        // Inline elements: style the run and carry on in the same paragraph.
        let mut styled = style.clone();
        match name.as_str() {
            "strong" | "b" => styled.strong = true,
            "em" | "i" => styled.emphasis = true,
            "s" | "del" | "strike" => styled.strikethrough = true,
            "code" | "kbd" | "samp" | "tt" => styled.code = true,
            // §8.5: an unsafe scheme is dropped, so the text stays and
            // only its link goes — a `javascript:` anchor becomes plain
            // words rather than a live one or a hole in the prose.
            "a" => {
                styled.link = Self::attr(tag, "href")
                    .filter(|h| is_safe_url(h))
                    .or_else(|| style.link.clone())
            }
            _ => {}
        }
        if let Some(colour) = html_colour(tag) {
            styled.color = Some(colour);
        }

        match name.as_str() {
            "br" => self.inline.push(Inline::Break { hard: true }),
            "img" => {
                let src = Self::attr(tag, "src").filter(|s| is_safe_url(s)).unwrap_or_default();
                let alt = Self::attr(tag, "alt").unwrap_or_default();
                let title = Self::attr(tag, "title");
                // §8.5: an image whose source was refused keeps its ALT
                // text — a reader learns what was meant to be there.
                self.inline.push(Inline::Image { alt, src, title });
            }
            "hr" => {
                self.flush_paragraph();
                self.blocks.push(Block::ThematicBreak);
            }
            "h1" | "h2" | "h3" | "h4" | "h5" | "h6" => {
                self.flush_paragraph();
                let level = name[1..].parse::<u8>().unwrap_or(1);
                let content = self.inline_of(tag, &styled);
                self.blocks.push(Block::Heading { level, content });
            }
            "p" => {
                self.flush_paragraph();
                let content = self.inline_of(tag, &styled);
                if !content.is_empty() {
                    self.blocks.push(Block::Paragraph { content });
                }
            }
            "pre" => {
                self.flush_paragraph();
                let text = tag.inner_text(self.parser).into_owned();
                self.blocks.push(Block::CodeBlock { language: None, text: decode_html_entities(&text) });
            }
            "blockquote" => {
                self.flush_paragraph();
                let blocks = self.blocks_of(tag, &styled);
                self.blocks.push(Block::BlockQuote { blocks });
            }
            "ul" | "ol" => {
                self.flush_paragraph();
                let ordered = name == "ol";
                let start = Self::attr(tag, "start").and_then(|s| s.trim().parse::<u64>().ok());
                let mut items = Vec::new();
                for child in tag.children().top().iter() {
                    let Some(li) = child.get(self.parser).and_then(|n| n.as_tag()) else { continue };
                    if !li.name().as_utf8_str().eq_ignore_ascii_case("li") {
                        continue;
                    }
                    items.push(ListItem { blocks: self.blocks_of(li, &styled), checked: None });
                }
                self.blocks.push(Block::List { ordered, start: if ordered { start.or(Some(1)) } else { None }, items });
            }
            "table" => {
                self.flush_paragraph();
                let table = self.table(tag, &styled);
                self.blocks.push(table);
            }
            // Everything else — `div`, `section`, `span`, `main`, an
            // unknown custom element — contributes its CHILDREN. That is
            // "degrade to the supported subset": a grid-laid-out page loses
            // its grid and keeps its content.
            _ => {
                for child in tag.children().top().iter() {
                    self.node(*child, &styled);
                }
            }
        }
    }

    fn table(&mut self, tag: &tl::HTMLTag<'_>, style: &TextStyle) -> Block {
        let mut header: Vec<Vec<Inline>> = Vec::new();
        let mut rows: Vec<Vec<Vec<Inline>>> = Vec::new();
        // `<thead>`/`<tbody>`/`<tfoot>` are transparent here: what matters
        // is the rows, wherever they are grouped.
        let mut stack: Vec<tl::NodeHandle> = tag.children().top().iter().copied().collect();
        let mut row_handles: Vec<tl::NodeHandle> = Vec::new();
        while let Some(h) = stack.pop() {
            let Some(t) = h.get(self.parser).and_then(|n| n.as_tag()) else { continue };
            let n = t.name().as_utf8_str().to_ascii_lowercase();
            if n == "tr" {
                row_handles.push(h);
            } else if matches!(n.as_str(), "thead" | "tbody" | "tfoot") {
                stack.extend(t.children().top().iter().copied());
            }
        }
        // The walk above pops in reverse; restore document order.
        row_handles.reverse();
        for h in row_handles {
            let Some(tr) = h.get(self.parser).and_then(|n| n.as_tag()) else { continue };
            let mut cells: Vec<Vec<Inline>> = Vec::new();
            let mut all_header = true;
            for c in tr.children().top().iter() {
                let Some(cell) = c.get(self.parser).and_then(|n| n.as_tag()) else { continue };
                let cn = cell.name().as_utf8_str().to_ascii_lowercase();
                if cn != "td" && cn != "th" {
                    continue;
                }
                if cn != "th" {
                    all_header = false;
                }
                cells.push(self.inline_of(cell, style));
            }
            if cells.is_empty() {
                continue;
            }
            if all_header && header.is_empty() {
                header = cells;
            } else {
                rows.push(cells);
            }
        }
        let width = header.len().max(rows.first().map(Vec::len).unwrap_or(0));
        Block::Table { alignments: vec![TableAlignment::None; width], header, rows }
    }
}

/// An HTML colour string as RGB — `#rgb`, `#rrggbb`, `rgb(r,g,b)` and the
/// sixteen original HTML colour names.
///
/// The full CSS colour list is 148 names and a subset renderer gains little
/// from carrying it; anything unrecognised answers `None`, and the caller
/// then uses the theme's own ink rather than a guess.
pub fn parse_html_color(value: &str) -> Option<[u8; 3]> {
    let v = value.trim().to_ascii_lowercase();
    if let Some(hex) = v.strip_prefix('#') {
        let expand = |c: char| u8::from_str_radix(&format!("{c}{c}"), 16).ok();
        return match hex.len() {
            3 => {
                let mut it = hex.chars();
                Some([expand(it.next()?)?, expand(it.next()?)?, expand(it.next()?)?])
            }
            6 => Some([
                u8::from_str_radix(&hex[0..2], 16).ok()?,
                u8::from_str_radix(&hex[2..4], 16).ok()?,
                u8::from_str_radix(&hex[4..6], 16).ok()?,
            ]),
            _ => None,
        };
    }
    if let Some(inner) = v.strip_prefix("rgb(").and_then(|r| r.strip_suffix(')')) {
        let parts: Vec<u8> = inner
            .split(',')
            .filter_map(|p| p.trim().parse::<i32>().ok())
            .map(|n| n.clamp(0, 255) as u8)
            .collect();
        if parts.len() == 3 {
            return Some([parts[0], parts[1], parts[2]]);
        }
        return None;
    }
    let named: &[(&str, [u8; 3])] = &[
        ("black", [0, 0, 0]),
        ("silver", [192, 192, 192]),
        ("gray", [128, 128, 128]),
        ("grey", [128, 128, 128]),
        ("white", [255, 255, 255]),
        ("maroon", [128, 0, 0]),
        ("red", [255, 0, 0]),
        ("purple", [128, 0, 128]),
        ("fuchsia", [255, 0, 255]),
        ("green", [0, 128, 0]),
        ("lime", [0, 255, 0]),
        ("olive", [128, 128, 0]),
        ("yellow", [255, 255, 0]),
        ("navy", [0, 0, 128]),
        ("blue", [0, 0, 255]),
        ("teal", [0, 128, 128]),
        ("aqua", [0, 255, 255]),
    ];
    named.iter().find(|(n, _)| *n == v).map(|(_, c)| *c)
}

/// §3's "colours" for the common cases a subset renderer can honestly read:
/// `<font color>`, and a `color:` in an inline `style` attribute. A
/// stylesheet is **not** consulted — that is a cascade, and a cascade is a
/// browser.
fn html_colour(tag: &tl::HTMLTag<'_>) -> Option<String> {
    if let Some(c) = HtmlWalker::attr(tag, "color") {
        return Some(c);
    }
    let style = HtmlWalker::attr(tag, "style")?;
    for decl in style.split(';') {
        let (key, value) = decl.split_once(':')?;
        if key.trim().eq_ignore_ascii_case("color") {
            return Some(value.trim().to_string());
        }
    }
    None
}

/// §8.5 — URL schemes that must never survive into rendered content.
///
/// `javascript:` and `vbscript:` execute. `data:` can carry a whole HTML
/// document, so it is refused **except** for an image, which is the one
/// form of it a document legitimately embeds and which this renderer
/// decodes as pixels rather than as markup.
pub fn is_safe_url(url: &str) -> bool {
    let trimmed = url.trim();
    // A scheme cannot contain whitespace, and a colon after a `/`, `?` or
    // `#` is part of a path, not a scheme — so only the leading run counts.
    let scheme_end = trimmed.find(|c| c == ':' || c == '/' || c == '?' || c == '#');
    let Some(i) = scheme_end else { return true };
    if trimmed.as_bytes()[i] != b':' {
        return true; // a relative URL: no scheme at all
    }
    // Control characters and whitespace inside a scheme are an obfuscation
    // trick (`java\nscript:`), so they are stripped before the comparison
    // rather than making the scheme "unrecognised" and therefore allowed.
    let scheme: String = trimmed[..i]
        .chars()
        .filter(|c| !c.is_whitespace() && !c.is_control())
        .collect::<String>()
        .to_ascii_lowercase();
    match scheme.as_str() {
        "javascript" | "vbscript" | "file" => false,
        "data" => trimmed[i + 1..].to_ascii_lowercase().starts_with("image/"),
        _ => true,
    }
}

/// The handful of entities a subset renderer must not show raw. Numeric
/// references are decoded too, so `&#8212;` is an em dash rather than five
/// literal characters.
pub fn decode_html_entities(text: &str) -> String {
    if !text.contains('&') {
        return text.to_string();
    }
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(i) = rest.find('&') {
        out.push_str(&rest[..i]);
        let tail = &rest[i..];
        let Some(end) = tail[..tail.len().min(12)].find(';') else {
            out.push('&');
            rest = &tail[1..];
            continue;
        };
        let name = &tail[1..end];
        let decoded = match name {
            "amp" => Some("&".to_string()),
            "lt" => Some("<".to_string()),
            "gt" => Some(">".to_string()),
            "quot" => Some("\"".to_string()),
            "apos" | "#39" => Some("'".to_string()),
            "nbsp" => Some("\u{a0}".to_string()),
            n if n.starts_with("#x") || n.starts_with("#X") => u32::from_str_radix(&n[2..], 16)
                .ok()
                .and_then(char::from_u32)
                .map(|c| c.to_string()),
            n if n.starts_with('#') => {
                n[1..].parse::<u32>().ok().and_then(char::from_u32).map(|c| c.to_string())
            }
            _ => None,
        };
        match decoded {
            Some(s) => {
                out.push_str(&s);
                rest = &tail[end + 1..];
            }
            None => {
                out.push('&');
                rest = &tail[1..];
            }
        }
    }
    out.push_str(rest);
    out
}

// ── Mermaid subset (T20: R7, AC2) ───────────────────────────────────────
//
// §3's contract: "**Subset we implement**: flowchart and sequence diagrams";
// **not delivered** — class/state/gantt/ER/journey, and exact upstream
// layout.
//
// ⚠️ **The library can draw more than the contract promises.**
// `mermaid-rs-renderer 0.2` also renders class, state and gantt diagrams.
// This control renders only what §3 published, and *refuses the rest
// visibly* rather than silently ignoring them (T20's own Verify) — AC2's
// rule is that nothing is over- or under-delivered against that table. If
// the operator wants the wider set, §3 is the thing to widen; the code is
// two lines behind it.

/// Which kind of diagram a ```` ```mermaid ```` block holds.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MermaidKind {
    Flowchart,
    Sequence,
    /// A real Mermaid diagram type this control does not publish (§3) —
    /// carried by name so the reason can be shown rather than swallowed.
    Unsupported(String),
}

impl MermaidKind {
    pub fn is_supported(&self) -> bool {
        !matches!(self, Self::Unsupported(_))
    }
}

/// Read a diagram's kind from its first meaningful line — the same place
/// Mermaid itself declares it.
pub fn mermaid_kind(source: &str) -> MermaidKind {
    let first = source
        .lines()
        .map(str::trim)
        .find(|l| !l.is_empty() && !l.starts_with("%%"))
        .unwrap_or("");
    let word = first.split([' ', '\t', ';']).next().unwrap_or("").trim();
    match word.to_ascii_lowercase().as_str() {
        // `graph` is Mermaid's older spelling of `flowchart`.
        "flowchart" | "graph" => MermaidKind::Flowchart,
        "sequencediagram" => MermaidKind::Sequence,
        "" => MermaidKind::Unsupported("empty".to_string()),
        other => MermaidKind::Unsupported(other.to_string()),
    }
}

/// Render a supported diagram to SVG.
///
/// An unsupported kind is an `Err` naming it, **never** a silent blank: the
/// caller shows the reason beside the source, so a developer whose class
/// diagram did not draw learns why from the control rather than from this
/// spec.
pub fn render_mermaid_svg(source: &str) -> Result<String, String> {
    match mermaid_kind(source) {
        MermaidKind::Flowchart | MermaidKind::Sequence => {
            mermaid_rs_renderer::render(source).map_err(|e| e.to_string())
        }
        MermaidKind::Unsupported(kind) => Err(format!(
            "Mermaid '{kind}' diagrams are not supported — this Viewer draws flowchart and sequence diagrams"
        )),
    }
}

/// Render a supported diagram to pixels, through the same `resvg` path an
/// SVG document already takes (§3's own note: no new raster dependency).
#[cfg(feature = "render")]
pub fn render_mermaid(source: &str) -> Result<DecodedImage, String> {
    let svg = render_mermaid_svg(source)?;
    decode_image(svg.as_bytes())
}

// ── PDF (T18: R7, R9, AC2) ──────────────────────────────────────────────
//
// §3's contract for this format, verbatim: **delivered** — text and basic
// vector, page geometry, page breaks, search; **not delivered** — a
// faithful raster of complex pages, embedded fonts with unusual encodings,
// forms, annotations, and scanned-image-only pages beyond the embedded
// image. Everything here is on the delivered side of that line, and the
// tests confirm the other side is *refused*, not silently half-attempted.
//
// Per plan.md §3 the PDF's own bytes stay the canonical stored form —
// binary, unconverted, in memory and on disk alike. What follows is a
// **derived read for painting**, computed from those bytes and thrown away,
// which is why R18's byte-identical Save As needs no special case (T19).

/// One PDF page, as much of it as §3 promises.
#[derive(Debug, Clone, PartialEq)]
pub struct PdfPage {
    /// 1-based, as a PDF numbers its own pages.
    pub number: u32,
    pub text: String,
    /// The page's MediaBox, in PostScript points (1/72 inch).
    pub width_pt: f32,
    pub height_pt: f32,
    /// The "basic vector" of §3's promise — straight lines and rectangles
    /// lifted from the content stream. Curves, shading, patterns and
    /// clipping are **not** attempted.
    pub vectors: Vec<PdfVector>,
}

/// A straight-line or rectangular path from a page's content stream, in the
/// PDF's own coordinate space (origin bottom-left, y up).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PdfVector {
    Line { x1: f32, y1: f32, x2: f32, y2: f32 },
    Rect { x: f32, y: f32, w: f32, h: f32 },
}

/// A PDF, read as far as §3 says this control reads one.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct PdfDocument {
    pub pages: Vec<PdfPage>,
}

impl PdfDocument {
    pub fn page_count(&self) -> usize {
        self.pages.len()
    }

    /// How many vectors were lifted, across every page — the number this
    /// task's own test reports, so "basic vector" is a measurement rather
    /// than a claim.
    pub fn vector_count(&self) -> usize {
        self.pages.iter().map(|p| p.vectors.len()).sum()
    }
}

impl SearchableText for PdfDocument {
    /// Find needs no PDF-specific branch: once a format can say what its
    /// text is, T14's engine searches it. A PDF with no text layer at all
    /// (a scan) answers `None`, and the Find bar reports `0 / 0` — R26.1's
    /// rule, reached without a special case.
    fn searchable_text(&self) -> Option<String> {
        let joined: String =
            self.pages.iter().map(|p| p.text.as_str()).collect::<Vec<_>>().join("\n");
        if joined.trim().is_empty() {
            None
        } else {
            Some(joined)
        }
    }
}

/// US Letter, the MediaBox a PDF is assumed to use when it declares none.
const PDF_DEFAULT_MEDIABOX: (f32, f32) = (612.0, 792.0);

/// Read a PDF's pages, their text, their geometry and their basic vectors.
///
/// Errors carry the reader's own message rather than a generic one: a PDF
/// that will not open is exactly the case R4 exists for, and "could not read
/// document: Invalid file trailer" tells a developer more than "unsupported".
pub fn parse_pdf(bytes: &[u8]) -> Result<PdfDocument, String> {
    let doc = lopdf::Document::load_mem(bytes).map_err(|e| e.to_string())?;
    let page_ids = doc.get_pages();
    let mut pages = Vec::with_capacity(page_ids.len());
    for (&number, &page_id) in &page_ids {
        // Text is extracted one page at a time, deliberately: a page whose
        // font encoding this reader cannot follow then costs that page's
        // text, not the whole document's (§3's "embedded fonts with unusual
        // encodings" are on the NOT-delivered side of the line, and this is
        // how that degrades).
        let text = doc.extract_text(&[number]).unwrap_or_default();
        let (width_pt, height_pt) = pdf_media_box(&doc, page_id);
        let vectors = doc
            .get_page_content(page_id)
            .ok()
            .and_then(|c| lopdf::content::Content::decode(&c).ok())
            .map(|c| pdf_vectors(&c))
            .unwrap_or_default();
        pages.push(PdfPage { number, text, width_pt, height_pt, vectors });
    }
    Ok(PdfDocument { pages })
}

/// A page's MediaBox, inherited from its ancestors when the page itself
/// declares none — which is how most real PDFs are written.
fn pdf_media_box(doc: &lopdf::Document, page_id: lopdf::ObjectId) -> (f32, f32) {
    let Ok(boxed) = doc.get_dictionary(page_id).and_then(|_| {
        doc.get_page_resources(page_id);
        doc.get_dictionary(page_id)
    }) else {
        return PDF_DEFAULT_MEDIABOX;
    };
    let media = boxed
        .get(b"MediaBox")
        .ok()
        .and_then(|o| o.as_array().ok())
        .cloned()
        .or_else(|| pdf_inherited_media_box(doc, page_id));
    let Some(values) = media else { return PDF_DEFAULT_MEDIABOX };
    let nums: Vec<f32> = values
        .iter()
        .filter_map(|o| match o {
            lopdf::Object::Integer(i) => Some(*i as f32),
            lopdf::Object::Real(r) => Some(*r as f32),
            _ => None,
        })
        .collect();
    if nums.len() == 4 {
        ((nums[2] - nums[0]).abs(), (nums[3] - nums[1]).abs())
    } else {
        PDF_DEFAULT_MEDIABOX
    }
}

/// Walk up the page tree for a MediaBox the page inherits.
fn pdf_inherited_media_box(
    doc: &lopdf::Document,
    page_id: lopdf::ObjectId,
) -> Option<Vec<lopdf::Object>> {
    let mut current = page_id;
    // Bounded: a malformed PDF whose Parent chain loops must not hang a
    // form. Ten levels is far deeper than any real page tree.
    for _ in 0..10 {
        let dict = doc.get_dictionary(current).ok()?;
        if let Ok(arr) = dict.get(b"MediaBox").and_then(|o| o.as_array()) {
            return Some(arr.clone());
        }
        current = dict.get(b"Parent").ok()?.as_reference().ok()?;
    }
    None
}

/// §3's "basic vector": `re` rectangles and `m`/`l` straight segments.
///
/// Curves (`c`/`v`/`y`), shading, patterns and clipping are deliberately not
/// attempted — they are on §3's not-delivered side, and a half-drawn Bézier
/// would be worse than an honestly absent one.
fn pdf_vectors(content: &lopdf::content::Content) -> Vec<PdfVector> {
    let num = |o: &lopdf::Object| -> Option<f32> {
        match o {
            lopdf::Object::Integer(i) => Some(*i as f32),
            lopdf::Object::Real(r) => Some(*r as f32),
            _ => None,
        }
    };
    let mut out = Vec::new();
    let mut cursor: Option<(f32, f32)> = None;
    for op in &content.operations {
        let n: Vec<f32> = op.operands.iter().filter_map(num).collect();
        match op.operator.as_str() {
            "re" if n.len() >= 4 => {
                out.push(PdfVector::Rect { x: n[0], y: n[1], w: n[2], h: n[3] });
                cursor = None;
            }
            "m" if n.len() >= 2 => cursor = Some((n[0], n[1])),
            "l" if n.len() >= 2 => {
                if let Some((x1, y1)) = cursor {
                    out.push(PdfVector::Line { x1, y1, x2: n[0], y2: n[1] });
                }
                cursor = Some((n[0], n[1]));
            }
            // A subpath closed with `h` returns to its start; without
            // tracking the whole path that is nothing more to draw here.
            "h" => cursor = None,
            _ => {}
        }
    }
    out
}

// ── Split view (T16: R21, R21.1, AC8) ───────────────────────────────────

/// R21 — one view, or two side by side / one above the other.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SplitMode {
    #[default]
    None,
    LeftRight,
    TopBottom,
}

impl SplitMode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::None => "None",
            Self::LeftRight => "LeftRight",
            Self::TopBottom => "TopBottom",
        }
    }

    pub fn from_str(s: &str) -> Self {
        match s.trim().to_ascii_lowercase().as_str() {
            "leftright" | "left-right" | "horizontal" => Self::LeftRight,
            "topbottom" | "top-bottom" | "vertical" => Self::TopBottom,
            _ => Self::None,
        }
    }

    pub fn is_split(self) -> bool {
        self != Self::None
    }

    /// How many views this mode shows — the loop bound every surface uses,
    /// so "is there a second view" is asked in one place.
    pub fn view_count(self) -> usize {
        if self.is_split() {
            2
        } else {
            1
        }
    }
}

impl std::fmt::Display for SplitMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// The divider's painted thickness between two views.
pub const SPLIT_DIVIDER: f32 = 6.0;
/// Where the divider sits by default, as a percentage of the span.
pub const SPLIT_DEFAULT_PCT: i64 = 50;
/// Neither view is allowed below this, so a divider dragged to an edge
/// still leaves something to grab it back by.
pub const SPLIT_MIN_VIEW: f32 = 40.0;

/// Where the two views and the divider land.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SplitGeometry {
    pub view1: ViewRect,
    /// `None` when `SplitMode = None`.
    pub view2: Option<ViewRect>,
    pub divider: Option<ViewRect>,
}

/// R21's two views, from `splitter::geometry()`'s own percent-split
/// arithmetic — the line's centre at `percent` along the span, one pane
/// either side of its faces.
///
/// The *math* is what is reused, not the Splitter control's model: that one
/// owns two developer-droppable child Panels, which is the wrong shape
/// entirely for one control showing two viewports of its own state
/// (plan.md §1).
pub fn split_geometry(bounds: ViewRect, mode: SplitMode, percent: i64) -> SplitGeometry {
    if !mode.is_split() {
        return SplitGeometry { view1: bounds, view2: None, divider: None };
    }
    let horizontal = mode == SplitMode::LeftRight;
    let span = if horizontal { bounds.w } else { bounds.h };
    let start = if horizontal { bounds.x } else { bounds.y };
    let half = SPLIT_DIVIDER / 2.0;

    // Too small to split into two usable views: show one rather than two
    // slivers neither of which can be read.
    if span < SPLIT_MIN_VIEW * 2.0 + SPLIT_DIVIDER {
        return SplitGeometry { view1: bounds, view2: None, divider: None };
    }
    let raw = start + span * (percent.clamp(0, 100) as f32) / 100.0;
    let centre = raw.clamp(
        start + SPLIT_MIN_VIEW + half,
        start + span - SPLIT_MIN_VIEW - half,
    );
    let a_len = (centre - half - start).max(0.0);
    let b_start = centre + half;
    let b_len = (start + span - b_start).max(0.0);

    let (view1, view2, divider) = if horizontal {
        (
            ViewRect::new(bounds.x, bounds.y, a_len, bounds.h),
            ViewRect::new(b_start, bounds.y, b_len, bounds.h),
            ViewRect::new(centre - half, bounds.y, SPLIT_DIVIDER, bounds.h),
        )
    } else {
        (
            ViewRect::new(bounds.x, bounds.y, bounds.w, a_len),
            ViewRect::new(bounds.x, b_start, bounds.w, b_len),
            ViewRect::new(bounds.x, centre - half, bounds.w, SPLIT_DIVIDER),
        )
    };
    SplitGeometry { view1, view2: Some(view2), divider: Some(divider) }
}

/// The percentage a divider dragged to `pos` along `bounds` means.
pub fn split_percent_at(bounds: ViewRect, mode: SplitMode, x: f32, y: f32) -> i64 {
    let (span, start, pos) = if mode == SplitMode::LeftRight {
        (bounds.w, bounds.x, x)
    } else {
        (bounds.h, bounds.y, y)
    };
    if span <= 0.0 {
        return SPLIT_DEFAULT_PCT;
    }
    (((pos - start) / span * 100.0).round() as i64).clamp(0, 100)
}

/// The property name for one view's `name`, e.g. `("Zoom", 1)` →
/// `"View2Zoom"`. **One place** builds these, so a typo cannot make a
/// property that only one of the reader and the writer agrees on.
pub fn view_prop(name: &str, view_index: usize) -> String {
    format!("View{}{name}", view_index + 1)
}

// ── Find: match computation (T14: R26.1, R27) ───────────────────────────
//
// The whole of Find's *searching* is this one pure function over a string.
// Nothing here knows which format produced the text — that is the point:
// once a format can say what its text is ([`SearchableText`]), Find works on
// it with no per-format branch, which is why T18 (PDF) and T21 (HTML) are
// expected to need no change here at all.
//
// Modeled on `cobolt-ide`'s `code_search.rs::find_matches()` and
// reimplemented rather than shared: that file is inside a binary crate.

/// One match, as byte offsets into the text that was searched.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TextSpan {
    pub start: usize,
    pub end: usize,
}

impl TextSpan {
    pub fn len(&self) -> usize {
        self.end.saturating_sub(self.start)
    }
    pub fn is_empty(&self) -> bool {
        self.end <= self.start
    }
}

/// R26/R27: every occurrence of `needle` in `haystack`, in order.
///
/// **Case-insensitive search never lowercases the haystack.** Lowercasing
/// can change a string's byte length (`İ` becomes two chars), which would
/// put every span it returns at the wrong offset — and a span at the wrong
/// offset highlights the wrong words. Instead the haystack is walked as a
/// stream of lowercase characters, each remembering the byte range of the
/// *source* character it came from, so a span always names real bytes of the
/// original text.
///
/// Overlapping occurrences are not reported: `"aa"` in `"aaa"` is one match
/// followed by a leftover, the way every editor's Find counts, because Next
/// walks occurrences a reader can see.
///
/// An empty needle, or empty text, is **zero matches and never an error** —
/// which is exactly R26.1's rule for a format with no extractable text (an
/// image), with no special case needed to implement it.
pub fn find_matches(haystack: &str, needle: &str, case_sensitive: bool) -> Vec<TextSpan> {
    let mut out = Vec::new();
    if needle.is_empty() || haystack.is_empty() {
        return out;
    }

    if case_sensitive {
        let mut from = 0usize;
        while let Some(i) = haystack[from..].find(needle) {
            let start = from + i;
            let end = start + needle.len();
            out.push(TextSpan { start, end });
            from = end;
        }
        return out;
    }

    // (lowercase char, byte start of its SOURCE char, byte end of it)
    let hay: Vec<(char, usize, usize)> = haystack
        .char_indices()
        .flat_map(|(i, c)| {
            let end = i + c.len_utf8();
            c.to_lowercase().map(move |lc| (lc, i, end))
        })
        .collect();
    let want: Vec<char> = needle.chars().flat_map(char::to_lowercase).collect();
    if want.is_empty() || hay.len() < want.len() {
        return out;
    }

    let mut k = 0usize;
    while k + want.len() <= hay.len() {
        if (0..want.len()).all(|j| hay[k + j].0 == want[j]) {
            out.push(TextSpan { start: hay[k].1, end: hay[k + want.len() - 1].2 });
            k += want.len();
        } else {
            k += 1;
        }
    }
    out
}

/// Which match Next/Previous moves to (R28), wrapping past the last/first.
///
/// `current` is the 0-based index of the match in view; `total` is how many
/// there are. With no matches at all there is nowhere to go, so the answer
/// is `None` rather than a fabricated index.
pub fn step_match(current: usize, total: usize, forward: bool) -> Option<usize> {
    if total == 0 {
        return None;
    }
    Some(if forward {
        (current + 1) % total
    } else {
        (current + total - 1) % total
    })
}

// ── The Find bar (T15: R26, R28–R31, AC22–AC24) ─────────────────────────

/// The bar's own height, under the toolbar and above the content.
pub const FIND_BAR_HEIGHT: f32 = 30.0;
/// The query field's width inside it.
pub const FIND_FIELD_WIDTH: f32 = 180.0;
/// A Find-bar button's painted size (the same square the toolbar uses).
pub const FIND_BUTTON: f32 = 22.0;
pub const FIND_GAP: f32 = 4.0;

/// One control on the Find bar.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FindControl {
    /// The query field — the only one that is not a button.
    Field,
    Previous,
    Next,
    CaseSensitive,
    Highlight,
    /// The live "current of total" counter (R30).
    Counter,
    Close,
}

impl FindControl {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Field => "find-field",
            Self::Previous => "find-previous",
            Self::Next => "find-next",
            Self::CaseSensitive => "find-case",
            Self::Highlight => "find-highlight",
            Self::Counter => "find-counter",
            Self::Close => "find-close",
        }
    }

    /// `None` for the two that are not icon buttons.
    pub fn icon(self) -> Option<&'static str> {
        Some(match self {
            Self::Field | Self::Counter => return None,
            Self::Previous => "chevron-up",
            Self::Next => "chevron-down",
            Self::CaseSensitive => "case-sensitive",
            Self::Highlight => "highlighter",
            Self::Close => "x-mark",
        })
    }

    /// R17's rule reaches the Find bar too: every button carries its
    /// function as a tooltip.
    pub fn default_tooltip(self) -> &'static str {
        match self {
            Self::Field => "Find in document",
            Self::Previous => "Previous match",
            Self::Next => "Next match",
            Self::CaseSensitive => "Match case",
            Self::Highlight => "Highlight all matches",
            Self::Counter => "Matches",
            Self::Close => "Close Find",
        }
    }
}

/// The Find bar's controls, in painted order.
pub const FIND_CONTROLS: &[FindControl] = &[
    FindControl::Field,
    FindControl::Previous,
    FindControl::Next,
    FindControl::Counter,
    FindControl::CaseSensitive,
    FindControl::Highlight,
    FindControl::Close,
];

/// Where each Find-bar control lands inside `bar`, left to right, with the
/// Close button pinned to the right edge — the one place a reader expects
/// to find it however wide the control is.
pub fn find_slots(bar: ViewRect) -> Vec<(FindControl, ViewRect)> {
    let mid = |h: f32| bar.y + (bar.h - h).max(0.0) * 0.5;
    let mut out = Vec::new();
    let close = ViewRect::new(
        (bar.right() - FIND_GAP - FIND_BUTTON).max(bar.x),
        mid(FIND_BUTTON),
        FIND_BUTTON,
        FIND_BUTTON,
    );
    let mut x = bar.x + FIND_GAP;
    let limit = close.x - FIND_GAP;
    for control in FIND_CONTROLS {
        if *control == FindControl::Close {
            continue;
        }
        let w = match control {
            FindControl::Field => FIND_FIELD_WIDTH.min((limit - x).max(0.0)),
            FindControl::Counter => 64.0,
            _ => FIND_BUTTON,
        };
        if w <= 1.0 || x + w > limit {
            break;
        }
        let h = if *control == FindControl::Field { FIND_BUTTON } else { FIND_BUTTON };
        out.push((*control, ViewRect::new(x, mid(h), w, h)));
        x += w + FIND_GAP;
    }
    out.push((FindControl::Close, close));
    out
}

/// R30's live counter text — "current of total", and an honest `0 / 0` when
/// there is nothing to step through rather than a hidden or blank field.
pub fn find_counter_text(current: usize, total: usize) -> String {
    if total == 0 {
        "0 / 0".to_string()
    } else {
        format!("{} / {}", current + 1, total)
    }
}

/// What a document offers Find to search (R26.1).
///
/// A format that has no text at all answers `None`, which
/// [`find_matches`] turns into zero matches — the Find bar then reports
/// "0 of 0" rather than raising, exactly as R26.1 requires.
pub trait SearchableText {
    fn searchable_text(&self) -> Option<String>;
}

impl SearchableText for MarkdownDocument {
    /// The prose a reader actually sees, not the Markdown source: searching
    /// a rendered document for "Release Notes" must find the heading whether
    /// or not its source line began with `#`.
    fn searchable_text(&self) -> Option<String> {
        let mut out = String::new();
        fn inlines(v: &[Inline], out: &mut String) {
            for i in v {
                match i {
                    Inline::Text { text, .. } => out.push_str(text),
                    Inline::Image { alt, .. } => out.push_str(alt),
                    Inline::Break { .. } => out.push(' '),
                    Inline::FootnoteRef { .. } => {}
                }
            }
        }
        fn cells(v: &[Vec<Inline>], out: &mut String) {
            for c in v {
                inlines(c, out);
                out.push('\t');
            }
        }
        fn walk(blocks: &[Block], out: &mut String) {
            for b in blocks {
                match b {
                    Block::Heading { content, .. } | Block::Paragraph { content } => {
                        inlines(content, out);
                        out.push('\n');
                    }
                    Block::CodeBlock { text, .. } => {
                        out.push_str(text);
                        out.push('\n');
                    }
                    // A diagram's LABELS are what a reader sees, and its
                    // source is where they are written — so searching a
                    // guide for a node's name finds it. The layout
                    // keywords come along; that is the honest trade against
                    // parsing the diagram a second time just for Find.
                    Block::Mermaid { source } => {
                        out.push_str(source);
                        out.push('\n');
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
                        cells(header, out);
                        out.push('\n');
                        for row in rows {
                            cells(row, out);
                            out.push('\n');
                        }
                    }
                    // Inert by design (see `Block::RawHtml`) — and a reader
                    // does not see its tags, so Find must not match them.
                    Block::RawHtml(_) => {}
                    Block::ThematicBreak => out.push('\n'),
                }
            }
        }
        walk(&self.blocks, &mut out);
        if out.trim().is_empty() {
            None
        } else {
            Some(out)
        }
    }
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

    /// **Fullscreen keeps the toolbar**, because fullscreen changed meaning.
    ///
    /// This test used to assert the opposite, and was right to while
    /// "fullscreen" meant the control's own rect with the chrome taken away:
    /// a toolbar's height out of a control is worth having. It is worth
    /// nothing out of a whole screen, and it costs the only visible way back —
    /// a fullscreen view whose exit is an unadvertised `Esc` is a trap
    /// (operator, 2026-09-20).
    ///
    /// Inverted rather than deleted: the fact it guards is still a fact, it is
    /// just the other one now, and a layout that silently stopped placing a
    /// toolbar would otherwise have nothing to catch it.
    #[test]
    fn a_fullscreen_view_keeps_the_toolbar_that_lets_the_reader_leave() {
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
        assert!(windowed.toolbar.is_some(), "the toolbar shows when not fullscreen");
        assert!(
            full.toolbar.is_some(),
            "fullscreen must keep the toolbar — it carries the way out"
        );
        assert_eq!(
            full.content.h, windowed.content.h,
            "and the content keeps exactly the height it had: the chrome no \
             longer moves when fullscreen is entered, only the WINDOW does"
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

/// Spec 058 T14 — Find's match computation (R26.1, R27, R28). Pure string
/// work: no format knows about it, and no format needs a branch in it.
#[cfg(test)]
mod find_tests {
    use super::*;

    const DOC: &str = "COBOL is not cobol, and Cobol is neither. \
                       A COBOL program written in cobol stays COBOL.";

    /// R27: "the case-sensitivity toggle changes which matches are found
    /// (e.g. 'COBOL' vs. 'cobol') without retyping the search text."
    #[test]
    fn case_sensitivity_changes_which_matches_are_found() {
        let sensitive = find_matches(DOC, "COBOL", true);
        let insensitive = find_matches(DOC, "COBOL", false);
        let text_of = |spans: &[TextSpan]| -> Vec<&str> {
            spans.iter().map(|s| &DOC[s.start..s.end]).collect()
        };
        println!("query \"COBOL\" over {} bytes:", DOC.len());
        println!("  case-sensitive   : {} match(es) {:?}", sensitive.len(), text_of(&sensitive));
        println!("  case-insensitive : {} match(es) {:?}", insensitive.len(), text_of(&insensitive));
        assert_eq!(sensitive.len(), 3, "only the upper-case spellings");
        assert_eq!(insensitive.len(), 6, "every spelling");
        assert!(text_of(&sensitive).iter().all(|t| *t == "COBOL"));
        assert!(insensitive.len() > sensitive.len(), "R27: the toggle must change the answer");
        // Every span must name real bytes of the ORIGINAL text.
        for s in &insensitive {
            assert!(DOC[s.start..s.end].eq_ignore_ascii_case("COBOL"), "span {s:?} names {:?}", &DOC[s.start..s.end]);
        }
    }

    /// The reason case-insensitive search does not lowercase the haystack:
    /// lowercasing can change byte lengths, and every span after the change
    /// would then point one byte off — highlighting `STANBUL ` instead of
    /// `ISTANBUL`.
    ///
    /// `İ` (U+0130) is the classic case: it is **two** bytes, and Rust
    /// lowercases it to `i` + a combining dot, which is **three**. This
    /// asserts the exact byte offsets, because "two matches" alone would
    /// pass even with every one of them misplaced.
    ///
    /// It also records the honest limit: `İstanbul` does **not** match
    /// `istanbul`, here or under `str::to_lowercase`, because full case
    /// folding is not simple lowercasing. Find behaves exactly as Rust's own
    /// case conversion does, rather than inventing a third answer.
    #[test]
    fn spans_stay_correct_when_lowercasing_changes_a_strings_length() {
        let text = "İstanbul and ISTANBUL and istanbul";
        let spans = find_matches(text, "istanbul", false);
        let found: Vec<(usize, usize, &str)> =
            spans.iter().map(|s| (s.start, s.end, &text[s.start..s.end])).collect();
        println!("{text:?} ({} bytes) searched for \"istanbul\":", text.len());
        for (a, b, t) in &found {
            println!("  bytes {a}..{b} = {t:?}");
        }
        println!("  for contrast, to_lowercase() gives {:?} ({} bytes)", text.to_lowercase(), text.to_lowercase().len());

        assert_eq!(spans.len(), 2, "İ does not case-fold to i, in Rust or here");
        // The load-bearing assertion: exact offsets into the ORIGINAL text.
        // Lowercasing the haystack first would report 15 and 28.
        assert_eq!(found[0], (14, 22, "ISTANBUL"));
        assert_eq!(found[1], (27, 35, "istanbul"));
        for s in &spans {
            assert!(text.is_char_boundary(s.start) && text.is_char_boundary(s.end));
        }
    }

    /// **R26.1** — "a format with no extractable text (for example, a
    /// standalone image) has no matches; the Find bar reports zero results
    /// rather than raising an error."
    #[test]
    fn a_textless_document_reports_zero_matches_and_never_an_error() {
        let image_has_no_text: Option<String> = None;
        let haystack = image_has_no_text.clone().unwrap_or_default();
        let matches = find_matches(&haystack, "anything", false);
        println!("an image's searchable text is {image_has_no_text:?} -> {} match(es)", matches.len());
        assert!(matches.is_empty(), "R26.1: zero results, cleanly");
        // And the degenerate queries, which must behave the same way.
        assert!(find_matches("some real text", "", false).is_empty(), "an empty query matches nothing");
        assert!(find_matches("", "", true).is_empty());
    }

    #[test]
    fn overlapping_occurrences_are_counted_the_way_an_editor_counts_them() {
        let spans = find_matches("aaaa", "aa", true);
        println!("\"aa\" in \"aaaa\" -> {} match(es) at {:?}", spans.len(), spans);
        assert_eq!(spans.len(), 2, "two non-overlapping matches, not three overlapping ones");
        assert_eq!(spans[0], TextSpan { start: 0, end: 2 });
        assert_eq!(spans[1], TextSpan { start: 2, end: 4 });
    }

    /// **R28** — Next/Previous wrap past the last/first match.
    #[test]
    fn next_and_previous_wrap_at_both_ends() {
        let total = 4;
        let forward: Vec<usize> = (0..6)
            .scan(0usize, |cur, _| {
                *cur = step_match(*cur, total, true).unwrap();
                Some(*cur)
            })
            .collect();
        let backward: Vec<usize> = (0..6)
            .scan(0usize, |cur, _| {
                *cur = step_match(*cur, total, false).unwrap();
                Some(*cur)
            })
            .collect();
        println!("{total} matches — Next from 0: {forward:?}");
        println!("{total} matches — Previous from 0: {backward:?}");
        assert_eq!(forward, [1, 2, 3, 0, 1, 2], "R28: wraps past the last");
        assert_eq!(backward, [3, 2, 1, 0, 3, 2], "R28: and past the first");
        assert_eq!(step_match(0, 0, true), None, "nowhere to go with no matches");
    }

    /// Find searches the prose a reader sees, not the Markdown source: a
    /// heading is findable whether or not its line began with `#`.
    #[test]
    fn markdown_is_searched_as_the_reader_sees_it() {
        let doc = parse_markdown(
            "# Release Notes\n\nA **bold** claim about `code`.\n\n\
             | Item | Qty |\n|---|---|\n| Widget | 12 |\n\n<div>hidden markup</div>\n",
        );
        let text = doc.searchable_text().expect("this document has prose");
        println!("searchable text ({} bytes): {:?}", text.len(), text.replace('\n', "\\n"));
        for needle in ["Release Notes", "bold", "code", "Widget", "12"] {
            let n = find_matches(&text, needle, false).len();
            println!("  {needle:?} -> {n} match(es)");
            assert_eq!(n, 1, "{needle:?} must be findable in the rendered prose");
        }
        assert!(find_matches(&text, "#", true).is_empty(), "the reader never sees the heading marker");
        assert!(find_matches(&text, "<div>", false).is_empty(), "nor inert raw markup");
        assert!(find_matches(&text, "hidden markup", false).is_empty());
    }

    #[test]
    fn a_document_with_no_prose_at_all_offers_no_searchable_text() {
        let empty = parse_markdown("");
        println!("an empty Markdown document -> searchable_text = {:?}", empty.searchable_text());
        assert!(empty.searchable_text().is_none(), "R26.1's own case, from the other direction");
    }
}

/// Spec 058 T16/T17 — split view (R21, R21.1, R21.2, AC8). The geometry is
/// pure arithmetic; the independence it enables is asserted at the engine
/// and session levels, where the state actually lives.
#[cfg(test)]
mod split_tests {
    use super::*;

    #[test]
    fn split_mode_round_trips_through_its_own_name() {
        for mode in [SplitMode::None, SplitMode::LeftRight, SplitMode::TopBottom] {
            let back = SplitMode::from_str(mode.as_str());
            println!("{mode} -> {:?} -> {back}, {} view(s)", mode.as_str(), mode.view_count());
            assert_eq!(back, mode);
        }
        assert_eq!(SplitMode::from_str("nonsense"), SplitMode::None, "an unknown value shows one view");
        assert_eq!(SplitMode::None.view_count(), 1);
        assert_eq!(SplitMode::LeftRight.view_count(), 2);
    }

    #[test]
    fn a_left_right_split_divides_the_width_at_the_divider() {
        let bounds = ViewRect::new(10.0, 20.0, 800.0, 400.0);
        for pct in [25i64, 50, 75] {
            let g = split_geometry(bounds, SplitMode::LeftRight, pct);
            let v2 = g.view2.expect("two views");
            let d = g.divider.expect("a divider");
            println!(
                "{pct}% -> view1 x={:.0} w={:.0} | divider x={:.0} w={:.0} | view2 x={:.0} w={:.0}",
                g.view1.x, g.view1.w, d.x, d.w, v2.x, v2.w
            );
            assert_eq!(g.view1.x, bounds.x);
            assert_eq!(v2.right(), bounds.right(), "the pair must fill the control exactly");
            assert_eq!(d.x, g.view1.right(), "no gap between view 1 and the divider");
            assert_eq!(v2.x, d.right(), "nor between the divider and view 2");
            assert_eq!(d.w, SPLIT_DIVIDER);
            assert_eq!(g.view1.h, bounds.h, "a left/right split keeps full height");
            assert_eq!(v2.h, bounds.h);
        }
    }

    #[test]
    fn a_top_bottom_split_divides_the_height_instead() {
        let bounds = ViewRect::new(0.0, 0.0, 500.0, 600.0);
        let g = split_geometry(bounds, SplitMode::TopBottom, 40);
        let v2 = g.view2.expect("two views");
        println!(
            "40% top/bottom -> view1 y={:.0} h={:.0}, view2 y={:.0} h={:.0}",
            g.view1.y, g.view1.h, v2.y, v2.h
        );
        assert_eq!(g.view1.w, bounds.w);
        assert_eq!(v2.w, bounds.w);
        assert!(g.view1.h < v2.h, "40 % leaves the smaller share on top");
        assert_eq!(v2.bottom(), bounds.bottom());
    }

    #[test]
    fn no_split_is_one_view_filling_the_control() {
        let bounds = ViewRect::new(5.0, 5.0, 300.0, 200.0);
        let g = split_geometry(bounds, SplitMode::None, 50);
        println!("SplitMode=None -> view1 {:?}, view2 {:?}", (g.view1.w, g.view1.h), g.view2);
        assert_eq!(g.view1, bounds);
        assert!(g.view2.is_none() && g.divider.is_none());
    }

    #[test]
    fn a_divider_dragged_to_an_edge_still_leaves_both_views_usable() {
        let bounds = ViewRect::new(0.0, 0.0, 600.0, 300.0);
        for pct in [0i64, 2, 98, 100] {
            let g = split_geometry(bounds, SplitMode::LeftRight, pct);
            let v2 = g.view2.expect("two views");
            println!("{pct:>3}% -> view1 {:.0} pt, view2 {:.0} pt (minimum {SPLIT_MIN_VIEW})", g.view1.w, v2.w);
            assert!(g.view1.w >= SPLIT_MIN_VIEW, "view 1 must stay grabbable");
            assert!(v2.w >= SPLIT_MIN_VIEW, "and so must view 2");
        }
    }

    #[test]
    fn a_control_too_small_to_split_shows_one_view_rather_than_two_slivers() {
        let bounds = ViewRect::new(0.0, 0.0, 60.0, 200.0);
        let g = split_geometry(bounds, SplitMode::LeftRight, 50);
        println!("a {:.0} pt wide control asked to split -> view2 {:?}", bounds.w, g.view2);
        assert!(g.view2.is_none(), "two unreadable slivers help nobody");
        assert_eq!(g.view1, bounds);
    }

    #[test]
    fn dragging_the_divider_reports_the_percentage_it_landed_on() {
        let bounds = ViewRect::new(100.0, 0.0, 400.0, 300.0);
        for (x, want) in [(100.0f32, 0i64), (200.0, 25), (300.0, 50), (500.0, 100), (900.0, 100)] {
            let pct = split_percent_at(bounds, SplitMode::LeftRight, x, 0.0);
            println!("divider dragged to x={x:.0} over [{:.0}..{:.0}] -> {pct}%", bounds.x, bounds.right());
            assert_eq!(pct, want);
        }
    }

    #[test]
    fn a_view_property_is_built_in_exactly_one_place() {
        for (name, a, b) in [
            ("Zoom", "View1Zoom", "View2Zoom"),
            ("SearchText", "View1SearchText", "View2SearchText"),
            ("ShowFilmstrip", "View1ShowFilmstrip", "View2ShowFilmstrip"),
        ] {
            println!("{name} -> {:?} / {:?}", view_prop(name, 0), view_prop(name, 1));
            assert_eq!(view_prop(name, 0), a);
            assert_eq!(view_prop(name, 1), b);
        }
    }
}

/// Spec 058 T18 — PDF (R7, R9, AC2). plan.md §5 called this the largest
/// technical unknown and asked for the first attempt to be treated as a
/// **go/no-go spike**; these tests are that verdict, in numbers.
///
/// The fixtures are built with `lopdf`'s own writer rather than hand-written
/// byte strings: a PDF's cross-reference table is a list of byte offsets, and
/// a fixture with hand-computed offsets tests the arithmetic in the test far
/// more than it tests the reader.
#[cfg(test)]
mod pdf_tests {
    use super::*;
    use lopdf::{dictionary, Document, Object, Stream};

    /// A PDF of `pages` pages, each carrying `text` plus its number, at the
    /// given MediaBox, with `vectors` drawn as a rectangle and a line.
    fn sample_pdf(pages: usize, text: &str, media: (i64, i64), vectors: bool) -> Vec<u8> {
        let mut doc = Document::with_version("1.5");
        let pages_id = doc.new_object_id();
        let font_id = doc.add_object(dictionary! {
            "Type" => "Font",
            "Subtype" => "Type1",
            "BaseFont" => "Helvetica",
        });
        let resources = doc.add_object(dictionary! {
            "Font" => dictionary! { "F1" => font_id },
        });
        let mut kids = Vec::new();
        for i in 0..pages {
            // An EMPTY `text` emits no text block at all — a page that
            // draws and says nothing, which is what the scanned-page case
            // needs to be honest.
            let mut stream = if text.is_empty() {
                String::new()
            } else {
                format!("BT /F1 18 Tf 72 700 Td ({text} page {}) Tj ET\n", i + 1)
            };
            if vectors {
                stream.push_str("100 100 200 150 re S\n");
                stream.push_str("50 50 m 300 400 l S\n");
            }
            let content_id = doc.add_object(Stream::new(dictionary! {}, stream.into_bytes()));
            let page_id = doc.add_object(dictionary! {
                "Type" => "Page",
                "Parent" => pages_id,
                "Contents" => content_id,
                "Resources" => resources,
                "MediaBox" => vec![0.into(), 0.into(), media.0.into(), media.1.into()],
            });
            kids.push(page_id.into());
        }
        let count = kids.len() as i64;
        doc.objects.insert(
            pages_id,
            Object::Dictionary(dictionary! {
                "Type" => "Pages",
                "Kids" => kids,
                "Count" => count,
            }),
        );
        let catalog_id = doc.add_object(dictionary! {
            "Type" => "Catalog",
            "Pages" => pages_id,
        });
        doc.trailer.set("Root", catalog_id);
        let mut out = Vec::new();
        doc.save_to(&mut out).expect("lopdf must be able to write its own format");
        out
    }

    /// **AC2** — a real PDF opens, and its page count and page breaks come
    /// from the document rather than being computed.
    #[test]
    fn a_pdf_opens_with_the_page_count_the_document_declares() {
        for pages in [1usize, 3, 7] {
            let bytes = sample_pdf(pages, "Quarterly report", (612, 792), false);
            let doc = parse_pdf(&bytes).expect("a PDF this reader wrote must open");
            let index = index_pdf(&doc, bytes.len() as u64);
            println!(
                "{pages}-page PDF ({} bytes) -> {} page(s), addressing {:?}, spans {:?}",
                bytes.len(),
                index.page_count(),
                index.addressing,
                index.pages.iter().map(|p| p.start).collect::<Vec<_>>()
            );
            assert_eq!(doc.page_count(), pages);
            assert_eq!(index.page_count(), pages, "R9: one break per PDF page");
            assert_eq!(index.addressing, PageAddressing::LogicalPages);
            let numbers: Vec<u64> = index.pages.iter().map(|p| p.start).collect();
            assert_eq!(numbers, (1..=pages as u64).collect::<Vec<_>>(), "1-based, in order");
        }
    }

    /// §3's "text" and "page geometry", both measured.
    #[test]
    fn a_pdf_yields_its_text_and_its_page_geometry() {
        let bytes = sample_pdf(2, "Invoice total", (595, 842), false); // A4
        let doc = parse_pdf(&bytes).unwrap();
        for page in &doc.pages {
            println!(
                "page {}: {:.0}x{:.0} pt, text {:?}",
                page.number,
                page.width_pt,
                page.height_pt,
                page.text.trim()
            );
            assert_eq!((page.width_pt, page.height_pt), (595.0, 842.0), "§3: page geometry");
            assert!(page.text.contains("Invoice total"), "§3: the text layer");
            assert!(
                page.text.contains(&format!("page {}", page.number)),
                "each page's OWN text, not the document's first"
            );
        }
    }

    /// §3's "basic vector" — rectangles and straight segments, measured, so
    /// the promise is a number rather than a claim.
    #[test]
    fn a_pdf_yields_its_basic_vectors_and_attempts_nothing_more() {
        let with = parse_pdf(&sample_pdf(2, "Chart", (612, 792), true)).unwrap();
        let without = parse_pdf(&sample_pdf(2, "Chart", (612, 792), false)).unwrap();
        println!(
            "2 pages with a rect + a line each -> {} vector(s); the same document without -> {}",
            with.vector_count(),
            without.vector_count()
        );
        for page in &with.pages {
            println!("  page {}: {:?}", page.number, page.vectors);
        }
        assert_eq!(with.vector_count(), 4, "one rect and one line per page");
        assert_eq!(without.vector_count(), 0, "nothing invented where nothing was drawn");
        assert!(with.pages[0].vectors.contains(&PdfVector::Rect { x: 100.0, y: 100.0, w: 200.0, h: 150.0 }));
        assert!(with.pages[0].vectors.contains(&PdfVector::Line { x1: 50.0, y1: 50.0, x2: 300.0, y2: 400.0 }));
    }

    /// **AC2's search clause** — "extracted text is searchable via T14/T15
    /// **unchanged**". Find gets no PDF-specific branch; this proves it did
    /// not need one.
    #[test]
    fn a_pdfs_text_is_searchable_through_the_unchanged_find_engine() {
        let bytes = sample_pdf(3, "Balance forward COBOL", (612, 792), false);
        let doc = parse_pdf(&bytes).unwrap();
        let text = doc.searchable_text().expect("this PDF has a text layer");
        let hits = find_matches(&text, "balance", false);
        let case_on = find_matches(&text, "balance", true);
        println!(
            "3-page PDF, searchable text {} bytes: \"balance\" case-insensitive -> {}, case-sensitive -> {}",
            text.len(),
            hits.len(),
            case_on.len()
        );
        assert_eq!(hits.len(), 3, "one per page");
        assert_eq!(case_on.len(), 0, "R27's toggle works on a PDF exactly as on text");
        assert_eq!(find_matches(&text, "page 2", false).len(), 1);
    }

    /// **§3's NOT-delivered column, confirmed refused rather than
    /// half-attempted.** A PDF with no text layer at all — a scan — is not
    /// OCR'd, guessed at, or reported as having text it does not have.
    #[test]
    fn a_pdf_with_no_text_layer_reports_no_text_rather_than_inventing_some() {
        // Vectors only: a page that draws, and says nothing.
        let bytes = sample_pdf(1, "", (612, 792), true);
        let doc = parse_pdf(&bytes).unwrap();
        let text = doc.searchable_text();
        println!(
            "a vector-only page -> {} vector(s), searchable text {:?}",
            doc.vector_count(),
            text.as_deref().map(str::trim)
        );
        assert_eq!(doc.page_count(), 1, "the page still opens and is counted");
        assert!(doc.vector_count() > 0, "and what it does draw is read");
        assert!(text.is_none(), "§3: no text layer means NO text, not an invented one");
        // R26.1: no text is zero matches, cleanly — never an error.
        let haystack = text.unwrap_or_default();
        assert!(find_matches(&haystack, "anything", false).is_empty());
    }

    /// R4's own case: a file that claims to be a PDF and is not opens
    /// nothing, and says why.
    #[test]
    fn a_corrupt_pdf_reports_the_readers_own_reason() {
        let junk = b"%PDF-1.4\nthis is not a PDF at all\n";
        let err = parse_pdf(junk).expect_err("a corrupt PDF must not open");
        println!("corrupt PDF -> {err:?}");
        assert!(!err.trim().is_empty(), "R4: LastError must say something useful");
    }

    /// End-to-end through `open_document`, the door R1 puts every source
    /// through.
    #[test]
    fn open_document_opens_a_pdf_and_reports_its_format() {
        use std::io::Write;
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("report.pdf");
        let bytes = sample_pdf(4, "Statement", (612, 792), false);
        std::fs::File::create(&path).unwrap().write_all(&bytes).unwrap();
        let src = DocumentSource::Path(path.to_string_lossy().into_owned());
        let mut progress = Vec::new();
        let index = open_document(&src, |p| progress.push(p)).expect("a PDF must open");
        println!(
            "report.pdf: format {}, {} page(s), progress {progress:?}",
            index.format,
            index.page_count()
        );
        assert_eq!(index.format, ViewerFormat::Pdf, "R3: resolved from %PDF- content");
        assert_eq!(index.page_count(), 4);
        assert_eq!(progress.last().copied(), Some(100), "R6 reaches 100");
    }
}

/// Spec 058 T20 — the Mermaid subset (R7, AC2). §3 promises **flowchart and
/// sequence** diagrams and nothing else; these tests report the pixel
/// dimensions actually produced, and confirm the excluded kinds are
/// *refused by name* rather than silently ignored.
#[cfg(test)]
mod mermaid_tests {
    use super::*;

    const FLOWCHART: &str = "flowchart LR\n  A[Start] --> B{Check}\n  B -->|yes| C[Done]\n  B -->|no| A\n";
    const SEQUENCE: &str = "sequenceDiagram\n  participant Form\n  participant Runtime\n  Form->>Runtime: onClick\n  Runtime-->>Form: StateUpdate\n";

    #[test]
    fn a_fenced_mermaid_block_becomes_a_diagram_not_a_code_listing() {
        let doc = parse_markdown(&format!(
            "# Pipeline\n\n```mermaid\n{FLOWCHART}```\n\n```rust\nlet x = 1;\n```\n"
        ));
        let counts = doc.count_by_kind();
        println!("blocks: {counts:?}");
        assert_eq!(counts.get("Mermaid"), Some(&1), "the mermaid fence is a diagram");
        assert_eq!(counts.get("CodeBlock"), Some(&1), "and an ordinary fence is still a listing");
        let found = doc.blocks.iter().any(|b| matches!(b, Block::Mermaid { source } if source.contains("flowchart")));
        assert!(found, "the diagram carries its own source");
    }

    #[test]
    fn the_language_tag_is_matched_case_insensitively() {
        for tag in ["mermaid", "Mermaid", "MERMAID", " mermaid "] {
            let doc = parse_markdown(&format!("```{tag}\n{FLOWCHART}```\n"));
            let n = *doc.count_by_kind().get("Mermaid").unwrap_or(&0);
            println!("fence language {tag:?} -> {n} diagram(s)");
            assert_eq!(n, 1, "a fence tagged {tag:?} is a diagram");
        }
    }

    #[test]
    fn the_two_supported_kinds_are_recognised_and_the_rest_are_named() {
        let cases: &[(&str, MermaidKind)] = &[
            ("flowchart LR\n A-->B", MermaidKind::Flowchart),
            ("graph TD\n A-->B", MermaidKind::Flowchart),
            ("  flowchart TB; A-->B", MermaidKind::Flowchart),
            ("%% a comment\nsequenceDiagram\n A->>B: hi", MermaidKind::Sequence),
            ("classDiagram\n A <|-- B", MermaidKind::Unsupported("classdiagram".into())),
            ("stateDiagram-v2\n [*] --> S", MermaidKind::Unsupported("statediagram-v2".into())),
            ("gantt\n title X", MermaidKind::Unsupported("gantt".into())),
            ("", MermaidKind::Unsupported("empty".into())),
        ];
        for (src, want) in cases {
            let got = mermaid_kind(src);
            println!("{:?}... -> {got:?} (supported: {})", &src[..src.len().min(22)], got.is_supported());
            assert_eq!(&got, want);
        }
    }

    #[cfg(feature = "render")]
    #[test]
    fn a_flowchart_and_a_sequence_diagram_both_render_to_real_pixels() {
        for (name, src) in [("flowchart", FLOWCHART), ("sequenceDiagram", SEQUENCE)] {
            let svg = render_mermaid_svg(src)
                .unwrap_or_else(|e| panic!("§3 promises {name} diagrams, and this one failed: {e}"));
            let img = render_mermaid(src).expect("and it must rasterise");
            println!(
                "{name}: {} bytes of SVG -> {}x{} px, {} frame(s)",
                svg.len(),
                img.width,
                img.height,
                img.frame_count()
            );
            assert!(svg.contains("<svg"), "a real SVG document");
            assert!(img.width > 20 && img.height > 20, "{name} must produce a real drawing, got {}x{}", img.width, img.height);
            assert_eq!(img.frame_count(), 1, "a diagram is a still image");
            assert_eq!(img.frames[0].rgba.len(), (img.width * img.height * 4) as usize);
        }
    }

    /// **T20's own Verify** — "a `class`/`state`/`gantt` block is confirmed
    /// **not** attempted, not silently ignored."
    ///
    /// ⚠️ Recorded finding: `mermaid-rs-renderer 0.2` *can* draw all three.
    /// This control draws only what §3 published, and says so by name — the
    /// alternative would over-deliver against the table AC2 checks against.
    #[test]
    fn the_diagram_kinds_section_three_excludes_are_refused_by_name() {
        for (kind, src) in [
            ("class", "classDiagram\n  Account <|-- Savings\n"),
            ("state", "stateDiagram-v2\n  [*] --> Open\n"),
            ("gantt", "gantt\n  title Release\n  section A\n  Task :a1, 2026-09-01, 3d\n"),
        ] {
            let err = render_mermaid_svg(src)
                .expect_err("§3 does not publish this diagram type");
            println!("{kind} diagram -> refused: {err}");
            assert!(err.contains(kind), "the refusal must NAME the kind, got {err:?}");
            assert!(
                err.contains("flowchart") && err.contains("sequence"),
                "and say what IS supported, got {err:?}"
            );
        }
    }

    #[test]
    fn a_diagrams_labels_are_findable() {
        let doc = parse_markdown(&format!("```mermaid\n{SEQUENCE}```\n"));
        let text = doc.searchable_text().expect("a diagram's source is text");
        let hits = find_matches(&text, "StateUpdate", false);
        println!("searching a sequence diagram for \"StateUpdate\" -> {} hit(s)", hits.len());
        assert_eq!(hits.len(), 1, "a diagram's labels are part of what a reader can find");
    }
}

/// Spec 058 T21/T22 — the HTML subset (R7, AC2), and the confirmation that
/// Find, Save As, Print and Share needed nothing HTML-specific (T22).
///
/// §3's contract: **delivered** — block/inline layout, common typography,
/// colours, borders, tables, images; **not delivered** — CSS3 grid/flex/
/// animation/transform, JavaScript, floats beyond the simple case. Not a
/// browser.
#[cfg(test)]
mod html_tests {
    use super::*;

    const PAGE: &str = r#"<!DOCTYPE html>
<html><head><title>Ignored</title><style>p { color: lime }</style></head>
<body>
  <h1>Quarterly <em>Report</em></h1>
  <p>Totals for <strong>September</strong>, with a <a href="/detail">detail link</a>.</p>
  <div class="grid-wrapper" style="display:grid">
    <section><p>Nested inside two unknown elements.</p></section>
  </div>
  <ul><li>First</li><li>Second</li></ul>
  <ol start="3"><li>Third</li></ol>
  <table>
    <thead><tr><th>Item</th><th>Qty</th></tr></thead>
    <tbody><tr><td>Widget</td><td>12</td></tr><tr><td>Gadget</td><td>4</td></tr></tbody>
  </table>
  <blockquote><p>A quoted remark.</p></blockquote>
  <pre>let x = 1;</pre>
  <hr>
  <p><img src="chart.png" alt="Revenue chart"></p>
  <script>alert('never rendered');</script>
</body></html>"#;

    #[test]
    fn a_representative_html_page_maps_onto_the_shared_layout_primitives() {
        let doc = parse_html(PAGE);
        let counts = doc.count_by_kind();
        println!("HTML -> shared layout blocks: {counts:?}");
        for (kind, want) in [("Heading", 1usize), ("List", 2), ("Table", 1), ("BlockQuote", 1), ("CodeBlock", 1), ("ThematicBreak", 1)] {
            let got = *counts.get(kind).unwrap_or(&0);
            println!("  {kind}: {got} (want {want})");
            assert_eq!(got, want, "{kind}");
        }
        assert!(counts.get("Paragraph").copied().unwrap_or(0) >= 4, "prose paragraphs, including the nested one");
    }

    #[test]
    fn a_table_keeps_its_header_row_apart_from_its_body() {
        let doc = parse_html(PAGE);
        let table = doc
            .blocks
            .iter()
            .find_map(|b| match b {
                Block::Table { header, rows, .. } => Some((header, rows)),
                _ => None,
            })
            .expect("§3 promises tables");
        let flat = |cells: &Vec<Vec<Inline>>| -> Vec<String> {
            cells
                .iter()
                .map(|c| c.iter().filter_map(|i| match i {
                    Inline::Text { text, .. } => Some(text.trim().to_string()),
                    _ => None,
                }).collect::<String>())
                .collect()
        };
        println!("header {:?}", flat(table.0));
        for row in table.1 {
            println!("row    {:?}", flat(row));
        }
        assert_eq!(flat(table.0), ["Item", "Qty"], "a <th> row is the header");
        assert_eq!(table.1.len(), 2, "two body rows");
        assert_eq!(flat(&table.1[0]), ["Widget", "12"], "in document order");
        assert_eq!(flat(&table.1[1]), ["Gadget", "4"]);
    }

    #[test]
    fn nested_inline_formatting_survives_the_mapping() {
        let doc = parse_html("<p>plain <strong>bold <em>and italic</em></strong> <s>gone</s> <code>x</code></p>");
        let Some(Block::Paragraph { content }) = doc.blocks.first() else {
            panic!("one paragraph, got {:?}", doc.blocks)
        };
        for run in content {
            if let Inline::Text { text, style } = run {
                println!(
                    "{:>18} strong={} em={} strike={} code={}",
                    format!("{:?}", text),
                    style.strong,
                    style.emphasis,
                    style.strikethrough,
                    style.code
                );
            }
        }
        let find = |needle: &str| content.iter().find_map(|i| match i {
            Inline::Text { text, style } if text.contains(needle) => Some(style.clone()),
            _ => None,
        }).unwrap_or_else(|| panic!("no run holding {needle:?}"));
        assert!(find("bold").strong && !find("bold").emphasis);
        assert!(find("and italic").strong && find("and italic").emphasis, "nesting is cumulative");
        assert!(find("gone").strikethrough);
        assert!(find("x").code);
        assert!(!find("plain").strong);
    }

    #[test]
    fn a_link_carries_its_destination_and_an_image_its_source() {
        let doc = parse_html(PAGE);
        let link = doc.blocks.iter().find_map(|b| match b {
            Block::Paragraph { content } => content.iter().find_map(|i| match i {
                Inline::Text { style, .. } => style.link.clone(),
                _ => None,
            }),
            _ => None,
        });
        let images = doc.image_sources();
        println!("first link destination {link:?}, image sources {images:?}");
        assert_eq!(link.as_deref(), Some("/detail"));
        assert_eq!(images, vec!["chart.png".to_string()], "§3 promises images");
    }

    /// **T21's own Verify** — "a JS-bearing or grid-laid-out fixture is
    /// confirmed to degrade to the supported subset rather than silently
    /// break."
    #[test]
    fn javascript_is_dropped_and_an_unsupported_layout_degrades_to_its_content() {
        let doc = parse_html(PAGE);
        let text = doc.searchable_text().unwrap_or_default();
        println!("rendered text ({} bytes):\n{}", text.len(), text.trim());
        assert!(!text.contains("alert"), "§3: JavaScript is not delivered — and not shown either");
        assert!(!text.contains("never rendered"), "nor its source");
        assert!(!text.contains("color: lime"), "a stylesheet is not prose");
        assert!(!text.contains("Ignored"), "nor a <title>");
        assert!(
            text.contains("Nested inside two unknown elements"),
            "a grid-laid-out <div> loses its grid and KEEPS its content"
        );
    }

    #[test]
    fn entities_are_decoded_rather_than_shown_raw() {
        let cases: &[(&str, &str)] = &[
            ("<p>a &amp; b</p>", "a & b"),
            ("<p>&lt;tag&gt;</p>", "<tag>"),
            ("<p>&quot;quoted&quot;</p>", "\"quoted\""),
            ("<p>1&#8212;2</p>", "1\u{2014}2"),
            ("<p>&#x41;&#x42;</p>", "AB"),
            ("<p>a &notanentity; b</p>", "a &notanentity; b"),
        ];
        for (html, want) in cases {
            let got = parse_html(html).searchable_text().unwrap_or_default();
            println!("{html} -> {:?}", got.trim());
            assert_eq!(got.trim(), *want);
        }
    }

    #[test]
    fn a_runs_own_colour_is_read_where_a_subset_renderer_can_honestly_read_it() {
        let doc = parse_html(
            r#"<p><span style="color: #c00">red</span> <font color="blue">blue</font> <span style="font-weight:bold">plain</span></p>"#,
        );
        let colours: Vec<(String, Option<String>)> = doc
            .blocks
            .iter()
            .flat_map(|b| match b {
                Block::Paragraph { content } => content.clone(),
                _ => vec![],
            })
            .filter_map(|i| match i {
                Inline::Text { text, style } if !text.trim().is_empty() => {
                    Some((text.trim().to_string(), style.color.clone()))
                }
                _ => None,
            })
            .collect();
        for (text, colour) in &colours {
            println!("{text:>6} -> colour {colour:?} -> rgb {:?}", colour.as_deref().and_then(parse_html_color));
        }
        assert_eq!(colours[0].1.as_deref(), Some("#c00"));
        assert_eq!(parse_html_color("#c00"), Some([0xcc, 0x00, 0x00]));
        assert_eq!(colours[1].1.as_deref(), Some("blue"));
        assert_eq!(colours[2].1, None, "a declaration that is not a colour sets none");
        assert_eq!(parse_html_color("rgb(10, 20, 30)"), Some([10, 20, 30]));
        assert_eq!(parse_html_color("papayawhip"), None, "unrecognised falls back to the theme's ink");
    }

    #[test]
    fn malformed_html_still_renders_whatever_parsed() {
        // Unclosed tags, a stray close, and text outside any element —
        // exactly what a `RestClient` can hand a COBOL program.
        let doc = parse_html("loose text<p>open<b>bold</p></i><ul><li>item");
        let text = doc.searchable_text().unwrap_or_default();
        println!("malformed input -> {:?}", text.trim());
        assert!(text.contains("loose text"), "text outside any element is not lost");
        assert!(text.contains("bold"));
        assert!(text.contains("item"));
    }

    // ── T22: Find, Save As, Print and Share need nothing HTML-specific ──

    /// **T22** — "run Stage D/Stage C's tests against an HTML-subset
    /// document... if any of these needs an HTML-specific branch, that is
    /// new scope."
    ///
    /// None did. Find searches an HTML document through the very same
    /// `find_matches` and `SearchableText` that serve text, Markdown and
    /// PDF, with the same case toggle and the same wraparound.
    #[test]
    fn find_searches_an_html_document_through_the_unchanged_engine() {
        let doc = parse_html(PAGE);
        let text = doc.searchable_text().expect("this page has prose");
        let insensitive = find_matches(&text, "report", false);
        let sensitive = find_matches(&text, "Report", true);
        println!(
            "HTML page, searchable text {} bytes: \"report\" case-off -> {}, \"Report\" case-on -> {}",
            text.len(),
            insensitive.len(),
            sensitive.len()
        );
        assert_eq!(insensitive.len(), 1, "the heading's own word");
        assert_eq!(sensitive.len(), 1);
        assert!(find_matches(&text, "Widget", false).len() == 1, "a table cell is findable");
        assert!(find_matches(&text, "Revenue chart", false).len() == 1, "and an image's alt text");
        // R28's navigation is the same pure function, with no format in it.
        assert_eq!(step_match(0, insensitive.len(), true), Some(0), "one match wraps to itself");
    }

    /// R18/R18.1 need nothing HTML-specific either: the extension follows
    /// the resolved format, and the proposed name its first three words.
    #[test]
    fn save_as_naming_needs_no_html_specific_branch() {
        let doc = parse_html(PAGE);
        let text = doc.searchable_text().unwrap_or_default();
        let proposed = default_save_name(ViewerFormat::HtmlSubset, Some(&text));
        println!("an HTML document with no source path would be saved as {proposed:?}");
        assert!(proposed.ends_with(".html"), "R18.1: the extension follows the format");
        assert!(proposed.starts_with("Quarterly-Report"), "and the name its first words, got {proposed}");
        assert_eq!(ensure_extension("my page", ViewerFormat::HtmlSubset), "my page.html");
    }
}

/// Spec 058 T24–T27 — auto-follow (§8.3/AC15/AC18), the new-content
/// indicator (§8.4/AC14), sanitisation (§8.5) and §8.6's performance rules.
#[cfg(test)]
mod conversation_tests {
    use super::*;

    // ── T24: auto-follow (§8.3, AC15, AC18) ─────────────────────────────

    /// **AC15** — "the viewport follows new content only when it was
    /// already showing the end; it stays stable when the user is reading
    /// older content"; the decision uses the **pre-append** position and the
    /// 24–32 px threshold.
    #[test]
    fn at_end_and_scrolled_up_answer_the_same_append_differently() {
        // The same document, the same append, two readers.
        let (max_before, max_after) = (1000.0f32, 1400.0f32);
        let mut following = AutoFollow::default();
        following.observe(max_before - 5.0, max_before); // 5 px from the end
        let followed = following.after_height_change(max_before - 5.0, max_after);

        let mut reading = AutoFollow::default();
        reading.observe(300.0, max_before); // well up the conversation
        let stayed = reading.after_height_change(300.0, max_after);

        println!("AC15, one append of 400 pt (threshold {AUTO_FOLLOW_THRESHOLD} pt):");
        println!("  reader 5 pt from the end  -> offset {} -> {followed} (pinned)", max_before - 5.0);
        println!("  reader 300 pt up          -> offset 300 -> {stayed} (left alone)");
        assert!(following.is_active() && followed == max_after, "pinned to the NEW end");
        assert!(!reading.is_active() && stayed == 300.0, "AC15: not moved a pixel");
        assert_ne!(followed, stayed, "AC15: the same append, two different answers");
    }

    #[test]
    fn the_threshold_is_the_one_section_eight_three_asks_for() {
        let max = 1000.0f32;
        for gap in [0.0f32, 10.0, 24.0, 28.0, 29.0, 40.0, 200.0] {
            let mut f = AutoFollow::default();
            f.observe(max - gap, max);
            println!("{gap:>5.0} pt from the end -> following = {}", f.is_active());
            assert_eq!(f.is_active(), gap <= AUTO_FOLLOW_THRESHOLD);
        }
        assert!((24.0..=32.0).contains(&AUTO_FOLLOW_THRESHOLD), "§8.3 asks for 24-32 px");
    }

    #[test]
    fn following_resumes_the_moment_the_reader_returns_to_the_end() {
        let max = 1000.0f32;
        let mut f = AutoFollow::default();
        f.observe(200.0, max);
        assert!(!f.is_active(), "scrolled up: not following");
        f.after_height_change(200.0, 1200.0);
        assert!(f.has_pending(), "§8.4: content arrived out of sight");
        // The reader scrolls back down.
        f.observe(1200.0, 1200.0);
        println!("after scrolling back to the end: following = {}, pending = {}", f.is_active(), f.has_pending());
        assert!(f.is_active(), "§8.3: following resumes");
        assert!(!f.has_pending(), "and the indicator clears itself");
    }

    /// **AC18** — "late layout changes (images or fonts loading) keep the
    /// viewport pinned to the end when auto-follow is active and otherwise
    /// preserve the user's reading position."
    #[test]
    fn a_late_image_decode_pins_only_when_following() {
        // An image finishes decoding and the document grows by 300 pt.
        let mut following = AutoFollow::default();
        following.observe(1000.0, 1000.0);
        let pinned = following.after_height_change(1000.0, 1300.0);

        let mut reading = AutoFollow::default();
        reading.observe(420.0, 1000.0);
        let held = reading.after_height_change(420.0, 1300.0);

        println!("AC18, a late decode adding 300 pt:");
        println!("  following -> {pinned} (the new end)");
        println!("  reading   -> {held} (unchanged)");
        assert_eq!(pinned, 1300.0);
        assert_eq!(held, 420.0, "AC18: the reader's place is not disturbed by a late layout");
    }

    // ── T25: the new-content indicator (§8.4, AC14) ─────────────────────

    /// **§8.4** — activating the indicator must do all three things:
    /// scroll to the end, clear itself, and re-enable automatic following.
    #[test]
    fn jump_to_latest_scrolls_clears_and_re_enables_following() {
        let mut f = AutoFollow::default();
        f.observe(100.0, 900.0);
        f.after_height_change(100.0, 1500.0);
        println!("before: following={}, pending={}", f.is_active(), f.has_pending());
        assert!(f.has_pending() && !f.is_active());

        let offset = f.jump_to_latest(1500.0);
        println!("after \"Jump to latest\": offset={offset}, following={}, pending={}", f.is_active(), f.has_pending());
        assert_eq!(offset, 1500.0, "1. scrolled to the end");
        assert!(!f.has_pending(), "2. the indicator cleared itself");
        assert!(f.is_active(), "3. automatic following is back on");
    }

    #[test]
    fn nothing_is_pending_for_a_reader_who_is_already_at_the_end() {
        let mut f = AutoFollow::default();
        f.observe(800.0, 800.0);
        f.after_height_change(800.0, 1100.0);
        println!("a reader at the end, after an append: pending = {}", f.has_pending());
        assert!(!f.has_pending(), "§8.4's indicator is for content you CANNOT see");
    }

    /// **AC14** at the model level — `RenderAsHtml = false` makes every
    /// append Raw, and content already appended is **not** reinterpreted
    /// when it is turned back on (§8.1's own wording).
    #[test]
    fn render_as_html_governs_arriving_content_only() {
        let mut conv = Conversation::new();
        conv.set_render_as_html(false);
        conv.append(AppendMode::Html, "<b>one</b>");
        conv.append(AppendMode::Markdown, "**two**");
        conv.set_render_as_html(true);
        conv.append(AppendMode::Html, "<b>three</b>");

        let modes: Vec<&str> = conv.messages.iter().map(|m| m.chunks[0].mode.as_str()).collect();
        let text = conv.text();
        println!("modes as stored: {modes:?}");
        println!("text: {text:?}");
        assert_eq!(modes, ["Raw", "Raw", "Html"], "AC14: the override forces Raw, per call");
        assert!(text.contains("<b>one</b>"), "content appended while off stays literal");
        assert!(text.contains("**two**"), "and so does the Markdown");
        assert!(!text.contains("<b>three</b>"), "while what arrived after renders");
    }

    // ── T26: sanitisation (§8.5) ────────────────────────────────────────

    /// **§8.5** — "scripts, inline event handlers, unsafe URLs and other
    /// executable content must not run." Each rule is reported by name, so
    /// a reader of the output knows which one caught which case.
    #[test]
    fn every_unsafe_html_fragment_is_neutralised_and_the_rule_is_named() {
        let cases: &[(&str, &str, &str)] = &[
            ("script element", "<p>ok</p><script>alert(1)</script>", "alert"),
            ("noscript element", "<noscript>alert(2)</noscript>", "alert"),
            ("iframe element", "<iframe src=\"http://evil\"></iframe>", "evil"),
            ("object element", "<object data=\"x.swf\"></object>", "x.swf"),
            ("stylesheet", "<style>body{background:url(javascript:1)}</style>", "javascript"),
            ("form controls", "<form><input value=\"steal\"><button>go</button></form>", "steal"),
        ];
        for (rule, html, must_not_appear) in cases {
            let text = parse_html(html).searchable_text().unwrap_or_default();
            println!("{rule:>17}: {html}\n{:>19} -> {:?}", "", text.trim());
            assert!(
                !text.contains(must_not_appear),
                "§8.5: the {rule} rule must have caught {must_not_appear:?}, got {text:?}"
            );
        }
    }

    #[test]
    fn an_unsafe_url_is_refused_and_a_safe_one_is_kept() {
        let cases: &[(&str, bool)] = &[
            ("https://example.com/report", true),
            ("http://example.com", true),
            ("/relative/path", true),
            ("report.pdf", true),
            ("mailto:someone@example.com", true),
            ("#anchor", true),
            ("data:image/png;base64,AAAA", true),
            ("javascript:alert(1)", false),
            ("JavaScript:alert(1)", false),
            ("java\nscript:alert(1)", false),
            ("  javascript:alert(1)", false),
            ("vbscript:msgbox", false),
            ("data:text/html,<script>alert(1)</script>", false),
            ("file:///etc/passwd", false),
        ];
        for (url, safe) in cases {
            let got = is_safe_url(url);
            println!("{:>44} -> {}", format!("{url:?}"), if got { "kept" } else { "REFUSED" });
            assert_eq!(got, *safe, "{url:?}");
        }
    }

    #[test]
    fn a_javascript_link_loses_its_link_and_keeps_its_words() {
        let doc = parse_html(r#"<p><a href="javascript:steal()">Click me</a> and <a href="/ok">this</a></p>"#);
        let runs: Vec<(String, Option<String>)> = doc
            .blocks
            .iter()
            .flat_map(|b| match b {
                Block::Paragraph { content } => content.clone(),
                _ => vec![],
            })
            .filter_map(|i| match i {
                Inline::Text { text, style } if !text.trim().is_empty() => Some((text, style.link)),
                _ => None,
            })
            .collect();
        for (text, link) in &runs {
            println!("{:>12} -> link {link:?}", format!("{text:?}"));
        }
        assert_eq!(runs[0].0.trim(), "Click me");
        assert_eq!(runs[0].1, None, "§8.5: the unsafe link is dropped");
        assert_eq!(runs.last().unwrap().1.as_deref(), Some("/ok"), "a safe one is kept");
    }

    #[test]
    fn an_inline_event_handler_can_never_be_read_by_this_walker() {
        let doc = parse_html(r#"<p onclick="steal()" onmouseover="also()">visible words</p>"#);
        let text = doc.searchable_text().unwrap_or_default();
        println!("a paragraph with two handlers -> {:?}", text.trim());
        assert_eq!(text.trim(), "visible words", "the content survives, the handlers do not exist");
        assert!(!text.contains("steal") && !text.contains("also"));
        // The rule the walker relies on, asserted so a future attribute
        // reader cannot quietly widen the surface.
        assert!(html_has_event_handler(&[("onclick", "x")]));
        assert!(html_has_event_handler(&[("href", "/a"), ("ONMOUSEOVER", "y")]));
        assert!(!html_has_event_handler(&[("href", "/a"), ("alt", "b")]));
    }

    #[test]
    fn a_raw_chunk_containing_a_script_is_displayed_not_run() {
        let mut conv = Conversation::new();
        conv.append(AppendMode::Raw, "<script>alert('hi')</script>");
        let text = conv.text();
        println!("raw chunk -> {text:?}");
        assert!(text.contains("<script>alert('hi')</script>"), "AC13: the literal text");
        assert!(
            matches!(conv.messages[0].blocks.first(), Some(Block::CodeBlock { .. })),
            "raw lands in this model's <pre>, which is never parsed"
        );
        // And the assembled HTML escapes it, so it cannot become markup
        // wherever that stream is used.
        let html = conv.to_html();
        println!("assembled -> {html}");
        assert!(html.contains("&lt;script&gt;"), "§8.5: escaped in the stream too");
        assert!(!html.contains("<script>"));
    }

    // ── T27: performance (§8.6) ─────────────────────────────────────────

    /// **§8.6 / AC17** — "appending to a long, pre-built conversation
    /// reports the time spent laying out old vs. new content; old content's
    /// layout-recompute cost should be ~zero, not proportional to
    /// conversation length."
    #[test]
    fn appending_to_a_long_conversation_costs_one_layout_not_a_rebuild() {
        let mut conv = Conversation::new();
        for i in 0..2000 {
            conv.append(AppendMode::Raw, &format!("message {i}"));
        }
        let built = conv.relayouts();
        let blocks_before = conv.blocks().len();

        let before = std::time::Instant::now();
        conv.append(AppendMode::Raw, "one more");
        let elapsed = before.elapsed();
        let after = conv.relayouts();

        println!("§8.6, measured:");
        println!("  building 2000 messages: {built} layout pass(es), {blocks_before} blocks");
        println!("  the 2001st append     : {} layout pass(es), {:?}", after - built, elapsed);
        println!(
            "  a rebuild would have cost {} passes instead of {}",
            conv.messages.len(),
            after - built
        );
        assert_eq!(built, 2000, "one pass per message, never the stream");
        assert_eq!(after - built, 1, "AC17: the 2001st append lays out ONE message");
        assert_eq!(conv.blocks().len(), blocks_before + 1);
    }

    /// §8.6's batching, made structural: consecutive chunks of the same
    /// mode are merged, so a token-at-a-time stream does not end up as a
    /// thousand one-word paragraphs.
    #[test]
    fn a_token_at_a_time_stream_merges_into_one_chunk() {
        let mut conv = Conversation::new();
        let id = conv.append(AppendMode::Raw, "The");
        for word in [" quick", " brown", " fox", " jumps"] {
            assert!(conv.append_to_message(&id, AppendMode::Raw, word));
        }
        let msg = &conv.messages[0];
        println!(
            "5 streamed tokens -> {} chunk(s), {} block(s), text {:?}",
            msg.chunks.len(),
            msg.blocks.len(),
            conv.text().trim()
        );
        assert_eq!(msg.chunks.len(), 1, "§8.6: batched into one");
        assert_eq!(msg.blocks.len(), 1, "and one block, not five");
        assert!(conv.text().contains("The quick brown fox jumps"), "in arrival order");
    }

    #[test]
    fn a_mode_change_mid_message_starts_a_new_chunk_rather_than_merging() {
        let mut conv = Conversation::new();
        let id = conv.append(AppendMode::Raw, "literal ");
        conv.append_to_message(&id, AppendMode::Markdown, "**then markdown**");
        let msg = &conv.messages[0];
        println!(
            "modes in one message: {:?}",
            msg.chunks.iter().map(|c| c.mode.as_str()).collect::<Vec<_>>()
        );
        assert_eq!(msg.chunks.len(), 2, "§8.5: each chunk keeps the mode it arrived in");
        assert_eq!(msg.chunks[0].mode, AppendMode::Raw);
        assert_eq!(msg.chunks[1].mode, AppendMode::Markdown);
    }

    #[test]
    fn message_ids_stay_stable_across_appends_and_pruning() {
        let mut conv = Conversation::new();
        let ids: Vec<String> = (0..5).map(|i| conv.append(AppendMode::Raw, &format!("m{i}"))).collect();
        for id in &ids {
            conv.append_to_message(id, AppendMode::Raw, "!");
        }
        let after_appends: Vec<String> = conv.messages.iter().map(|m| m.id.clone()).collect();
        let dropped = conv.prune_to(3);
        let after_prune: Vec<String> = conv.messages.iter().map(|m| m.id.clone()).collect();
        println!("ids minted   {ids:?}");
        println!("after appends {after_appends:?}");
        println!("pruned to 3: dropped {dropped}, kept {after_prune:?}");
        assert_eq!(after_appends, ids, "§8.6: ids are stable across appends");
        assert_eq!(dropped, 2, "the two OLDEST go");
        assert_eq!(after_prune, ids[2..], "and the newest keep their own ids");
        assert_eq!(conv.prune_to(0), 0, "a ceiling of 0 never prunes");
        assert_eq!(conv.prune_to(100), 0, "nor does one above the size");
    }
}
