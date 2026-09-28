// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! A Viewer's document or conversation written out as a PDF — `SaveAsPdf`.
//!
//! The PDF is laid out from the SAME block model the Viewer paints
//! ([`crate::viewer::Block`]), so what the reader saw is what they get:
//! headings at their sizes, bold and italic as the real faces of the font
//! (not the regular face standing in for them), strikethrough-free runs,
//! numbered and bulleted lists — task lists with their boxes — tables with
//! their header row, block quotes, and code in a monospaced face inside a
//! frame. A conversation keeps its turns apart the way the bubbles do: the
//! user's messages are set in, in the user's ink, and each message is
//! separated from the next. The speaker's name is not added — a message
//! carries its own when the application writes one.
//!
//! What a PDF cannot hold is said, never dropped: an image appears as its
//! alt text in brackets, a Mermaid diagram as its source, a link as its text
//! followed by the address.
//!
//! Fonts are the machine's own (`fontdb`), nothing is bundled; a family is
//! used only when its regular face parses, and a bold or italic face only
//! when it really is one.

use std::path::Path;
use std::sync::OnceLock;

use genpdf::elements::{
    Break, BulletPoint, FrameCellDecorator, LinearLayout, PaddedElement, Paragraph, TableLayout,
};
use genpdf::fonts::{Font, FontData, FontFamily};
use genpdf::style::{Color, Style};
use genpdf::{Alignment, Document, Element, Margins, SimplePageDecorator};

use crate::viewer::{Block, Inline, MessageRole, TextStyle};

/// Body text size, in points.
const BODY_PT: u8 = 11;
/// Ink for the user's own messages — PowerChat's navy, the bubble's colour.
const USER_INK: Color = Color::Rgb(0, 55, 88);
/// Ink for code and for what stands in for an image or a diagram.
const QUIET_INK: Color = Color::Rgb(70, 70, 70);
/// Ink for a link's text.
const LINK_INK: Color = Color::Rgb(20, 80, 180);

/// Write the conversation a `Streamed` Viewer holds (`_ConversationHtml`) to
/// `out` as a PDF titled `title`.
pub fn conversation_to_pdf(conversation_html: &str, title: &str, out: &Path) -> Result<(), String> {
    let messages = crate::viewer::parse_conversation_html(conversation_html);
    if messages.iter().all(|m| m.blocks.is_empty()) {
        return Err("the conversation is empty".to_string());
    }
    let code = messages.iter().any(|m| has_code(&m.blocks));
    let (mut doc, fonts) = new_document(title, code)?;
    doc.push(title_paragraph(title));
    for (i, m) in messages.iter().enumerate() {
        if m.blocks.is_empty() {
            continue;
        }
        if i > 0 {
            doc.push(Break::new(0.8));
        }
        let body = render_blocks(&m.blocks, &fonts);
        if m.role == MessageRole::User {
            // Set in, like the user's bubble, in the user's ink.
            doc.push(PaddedElement::new(body, Margins::trbl(0, 0, 0, 30)).styled(Style::new().with_color(USER_INK)));
        } else {
            doc.push(body);
        }
    }
    doc.render_to_file(out).map_err(|e| format!("could not write the PDF: {e}"))
}

/// Write a Markdown (or plain text) document to `out` as a PDF titled
/// `title`.
pub fn markdown_to_pdf(markdown: &str, title: &str, out: &Path) -> Result<(), String> {
    let parsed = crate::viewer::parse_markdown(markdown);
    if parsed.blocks.is_empty() {
        return Err("the document is empty".to_string());
    }
    let (mut doc, fonts) = new_document(title, has_code(&parsed.blocks))?;
    doc.push(render_blocks(&parsed.blocks, &fonts));
    doc.render_to_file(out).map_err(|e| format!("could not write the PDF: {e}"))
}

/// The families a document is set in besides the body's: a monospaced one
/// for code. Every face of a family is embedded WHOLE (genpdf does not
/// subset), about 2.5 MB for Courier New's four — so it is added only when
/// the document holds code, and code falls back to the body face when the
/// machine has no monospaced family.
struct Fonts {
    mono: Option<FontFamily<Font>>,
}

fn new_document(title: &str, code: bool) -> Result<(Document, Fonts), String> {
    let body = body_family().ok_or_else(|| "no usable system font found for the PDF".to_string())?;
    let mut doc = Document::new(body);
    let mono = code.then(mono_family).flatten().map(|m| doc.add_font_family(m));
    doc.set_title(title);
    doc.set_font_size(BODY_PT);
    let mut deco = SimplePageDecorator::new();
    deco.set_margins(18);
    doc.set_page_decorator(deco);
    Ok((doc, Fonts { mono }))
}

fn title_paragraph(title: &str) -> impl Element {
    let mut p = Paragraph::default();
    p.push_styled(title.to_string(), Style::new().bold().with_font_size(16));
    PaddedElement::new(p, Margins::trbl(0, 0, 4, 0))
}

fn heading_pt(level: u8) -> u8 {
    match level {
        1 => 20,
        2 => 17,
        3 => 15,
        4 => 13,
        _ => 12,
    }
}

/// Whether any block, at any depth, is code — a listing, a diagram's source
/// or an inline `code` run.
fn has_code(blocks: &[Block]) -> bool {
    let inline = |c: &[Inline]| c.iter().any(|i| matches!(i, Inline::Text { style, .. } if style.code));
    blocks.iter().any(|b| match b {
        Block::CodeBlock { .. } | Block::Mermaid { .. } => true,
        Block::Heading { content, .. } | Block::Paragraph { content } => inline(content),
        Block::BlockQuote { blocks }
        | Block::FootnoteDefinition { blocks, .. }
        | Block::Styled { blocks, .. } => has_code(blocks),
        Block::List { items, .. } => items.iter().any(|i| has_code(&i.blocks)),
        Block::Table { header, rows, .. } => {
            header.iter().any(|c| inline(c)) || rows.iter().flatten().any(|c| inline(c))
        }
        Block::ThematicBreak | Block::RawHtml(_) => false,
    })
}

/// Lay a list of blocks out, top to bottom.
fn render_blocks(blocks: &[Block], fonts: &Fonts) -> LinearLayout {
    let mut out = LinearLayout::vertical();
    for (i, b) in blocks.iter().enumerate() {
        if i > 0 {
            out.push(Break::new(0.4));
        }
        render_block(b, fonts, &mut out);
    }
    out
}

fn render_block(block: &Block, fonts: &Fonts, out: &mut LinearLayout) {
    match block {
        Block::Heading { level, content } => {
            let size = heading_pt(*level);
            for p in paragraphs(content, fonts, Style::new().bold().with_font_size(size)) {
                out.push(p);
            }
        }
        Block::Paragraph { content } => {
            for p in paragraphs(content, fonts, Style::new()) {
                out.push(p);
            }
        }
        Block::CodeBlock { text, .. } => out.push(code_block(text, fonts)),
        Block::Mermaid { source } => out.push(code_block(source, fonts)),
        Block::BlockQuote { blocks } => {
            let inner = render_blocks(blocks, fonts);
            out.push(
                PaddedElement::new(inner, Margins::trbl(0, 0, 0, 8))
                    .styled(Style::new().italic().with_color(QUIET_INK)),
            );
        }
        Block::List { ordered, start, items } => {
            let mut list = LinearLayout::vertical();
            let first = start.unwrap_or(1);
            for (n, item) in items.iter().enumerate() {
                let bullet = match (item.checked, *ordered) {
                    (Some(true), _) => "[x]".to_string(),
                    (Some(false), _) => "[ ]".to_string(),
                    (None, true) => format!("{}.", first + n as u64),
                    (None, false) => "•".to_string(),
                };
                list.push(BulletPoint::new(render_blocks(&item.blocks, fonts)).with_bullet(bullet));
            }
            out.push(list);
        }
        Block::Table { header, rows, .. } => {
            let columns = rows
                .iter()
                .map(Vec::len)
                .chain(std::iter::once(header.len()))
                .max()
                .unwrap_or(0);
            if columns == 0 {
                return;
            }
            let mut table = TableLayout::new(vec![1; columns]);
            table.set_cell_decorator(FrameCellDecorator::new(true, true, false));
            let mut push_row = |cells: &[Vec<Inline>], style: Style| {
                let mut row = table.row();
                for c in 0..columns {
                    let mut cell = LinearLayout::vertical();
                    if let Some(content) = cells.get(c) {
                        for p in paragraphs(content, fonts, style) {
                            cell.push(p);
                        }
                    }
                    row = row.element(PaddedElement::new(cell, Margins::trbl(1, 1, 1, 1)));
                }
                // A row of the wrong width is the only error `push` has; the
                // width is fixed above, so this cannot fail.
                let _ = row.push();
            };
            if !header.is_empty() {
                push_row(header, Style::new().bold());
            }
            for r in rows {
                push_row(r, Style::new());
            }
            out.push(table);
        }
        // A CSS box keeps its CONTENT in the PDF, in order; its background,
        // border and shadow are the screen's (the PDF writer has no boxes).
        Block::Styled { blocks, .. } => out.push(render_blocks(blocks, fonts)),
        Block::ThematicBreak => {
            out.push(
                Paragraph::new("* * *")
                    .aligned(Alignment::Center)
                    .styled(Style::new().with_color(QUIET_INK)),
            );
        }
        Block::FootnoteDefinition { label, blocks } => {
            let mut p = Paragraph::default();
            p.push_styled(format!("[{label}]"), Style::new().bold());
            out.push(p);
            out.push(PaddedElement::new(render_blocks(blocks, fonts), Margins::trbl(0, 0, 0, 6)));
        }
        Block::RawHtml(html) => {
            for line in html.lines().filter(|l| !l.trim().is_empty()) {
                out.push(Paragraph::new(line.to_string()));
            }
        }
    }
}

/// A code listing: one paragraph per line in the monospaced face, framed.
fn code_block(text: &str, fonts: &Fonts) -> impl Element {
    let mut style = Style::new().with_font_size(BODY_PT - 1).with_color(QUIET_INK);
    if let Some(mono) = fonts.mono {
        style = style.with_font_family(mono);
    }
    let mut listing = LinearLayout::vertical();
    for line in text.trim_end_matches('\n').lines() {
        // A blank line would collapse to nothing; a space keeps its height.
        let line = if line.is_empty() { " " } else { line };
        listing.push(Paragraph::new(genpdf::style::StyledString::new(line.to_string(), style)));
    }
    PaddedElement::new(listing, Margins::trbl(2, 2, 2, 2)).framed()
}

/// Running text as paragraphs: a HARD break starts the next one (a PDF
/// paragraph has no line break of its own); a soft break is a space.
fn paragraphs(content: &[Inline], fonts: &Fonts, base: Style) -> Vec<Paragraph> {
    let mut out = Vec::new();
    let mut p = Paragraph::default();
    let mut empty = true;
    for inline in content {
        match inline {
            Inline::Text { text, style } => {
                push_run(&mut p, text, style, fonts, base);
                if let Some(link) = style.link.as_deref().filter(|l| *l != text.as_str()) {
                    p.push_styled(format!(" ({link})"), base.with_color(QUIET_INK));
                }
                empty = false;
            }
            Inline::Image { alt, .. } => {
                let label = if alt.trim().is_empty() { "image" } else { alt.trim() };
                p.push_styled(format!("[{label}]"), base.italic().with_color(QUIET_INK));
                empty = false;
            }
            Inline::FootnoteRef { label } => {
                p.push_styled(format!("[{label}]"), base);
                empty = false;
            }
            Inline::Break { hard: false } => p.push_styled(" ", base),
            Inline::Break { hard: true } => {
                out.push(std::mem::take(&mut p));
                empty = true;
            }
        }
    }
    if !empty {
        out.push(p);
    }
    out
}

fn push_run(p: &mut Paragraph, text: &str, style: &TextStyle, fonts: &Fonts, base: Style) {
    let mut s = base;
    if style.strong {
        s = s.bold();
    }
    if style.emphasis {
        s = s.italic();
    }
    if style.code {
        s = s.with_color(QUIET_INK);
        if let Some(mono) = fonts.mono {
            s = s.with_font_family(mono);
        }
    }
    if style.link.is_some() {
        s = s.with_color(LINK_INK);
    }
    if let Some(c) = style.color.as_deref().and_then(parse_hex) {
        s = s.with_color(c);
    }
    p.push_styled(text.to_string(), s);
}

/// `#rrggbb` (the HTML subset's colours); anything else keeps the ink.
fn parse_hex(c: &str) -> Option<Color> {
    let h = c.trim().strip_prefix('#')?;
    if h.len() != 6 {
        return None;
    }
    let v = |i: usize| u8::from_str_radix(&h[i..i + 2], 16).ok();
    Some(Color::Rgb(v(0)?, v(2)?, v(4)?))
}

// ── Fonts ────────────────────────────────────────────────────────────────

fn db() -> &'static fontdb::Database {
    static DB: OnceLock<fontdb::Database> = OnceLock::new();
    DB.get_or_init(|| {
        let mut db = fontdb::Database::new();
        db.load_system_fonts();
        db
    })
}

/// The first of these the machine has. Families that ship one FILE per face
/// come first: a face inside a collection (macOS's Helvetica, Menlo) cannot
/// be embedded here, so its bold would fall back to the regular face.
const BODY_FAMILIES: [&str; 7] =
    ["Arial", "DejaVu Sans", "Liberation Sans", "Segoe UI", "Verdana", "Tahoma", "Helvetica"];
const MONO_FAMILIES: [&str; 6] =
    ["Courier New", "DejaVu Sans Mono", "Liberation Mono", "Consolas", "Menlo", "Monaco"];

fn body_family() -> Option<FontFamily<FontData>> {
    BODY_FAMILIES.iter().find_map(|f| family(f))
}

fn mono_family() -> Option<FontFamily<FontData>> {
    MONO_FAMILIES.iter().find_map(|f| family(f))
}

/// `name`'s four faces. The regular face must exist and parse; a bold or
/// italic face stands in for itself only when it really is one — otherwise the
/// nearest face that is.
fn family(name: &str) -> Option<FontFamily<FontData>> {
    let regular = face(name, fontdb::Weight::NORMAL, fontdb::Style::Normal)?;
    let bold = face(name, fontdb::Weight::BOLD, fontdb::Style::Normal);
    let italic = face(name, fontdb::Weight::NORMAL, fontdb::Style::Italic);
    let bold_italic = face(name, fontdb::Weight::BOLD, fontdb::Style::Italic);
    Some(FontFamily {
        bold: bold.clone().unwrap_or_else(|| regular.clone()),
        italic: italic.clone().unwrap_or_else(|| regular.clone()),
        bold_italic: bold_italic.or(bold).or(italic).unwrap_or_else(|| regular.clone()),
        regular,
    })
}

/// One face of `name`, when the machine has exactly that face as a single-font
/// file: a PDF embeds the whole file, and the reader takes its first font.
fn face(name: &str, weight: fontdb::Weight, style: fontdb::Style) -> Option<FontData> {
    let q = fontdb::Query {
        families: &[fontdb::Family::Name(name)],
        weight,
        stretch: fontdb::Stretch::Normal,
        style,
    };
    let id = db().query(&q)?;
    let info = db().face(id)?;
    if weight == fontdb::Weight::BOLD && info.weight.0 < 600 {
        return None;
    }
    if style != fontdb::Style::Normal && info.style == fontdb::Style::Normal {
        return None;
    }
    let bytes = db().with_face_data(id, |data, index| (index == 0).then(|| data.to_vec()))??;
    FontData::new(bytes, None).ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::viewer::{AppendMode, Conversation};

    /// Operator (2026-09-27): "save the file in PDF form, keeping the
    /// formatting done during the conversation". Every block kind a chat
    /// reply can carry goes in, and a real PDF comes out.
    #[test]
    fn a_conversation_saves_as_a_pdf_with_its_formatting() {
        let mut c = Conversation::new();
        c.append_as(AppendMode::Markdown, "**You:** what is the leave policy?", MessageRole::User);
        c.append_as(
            AppendMode::Markdown,
            "# Leave policy\n\nEmployees get **twenty** working days, *pro rata* for \
             part-time staff, see `LEAVE-DAYS`.\n\n1. Ask your manager\n2. Book it in HR\n\n\
             - [x] approved\n- [ ] taken\n\n| Grade | Days |\n|---|---|\n| A | 20 |\n| B | 25 |\n\n\
             > Carry-over ends in March.\n\n```cobol\n       MOVE 20 TO LEAVE-DAYS.\n```\n\n---\n\n\
             Ask [HR](https://example.org/hr) for more.",
            MessageRole::Agent,
        );
        let out = std::env::temp_dir().join(format!("prc-viewer-pdf-{}.pdf", std::process::id()));
        let _ = std::fs::remove_file(&out);
        let t = std::time::Instant::now();
        if body_family().is_none() {
            println!("no system font this test can use — skipped");
            return;
        }
        conversation_to_pdf(&c.to_html(), "Leave policy — conversation", &out).expect("the PDF is written");
        let bytes = std::fs::read(&out).expect("the PDF exists");
        assert_eq!(&bytes[..5], b"%PDF-", "a PDF file");
        assert!(bytes.len() > 2000, "a real document, {} bytes", bytes.len());
        let kinds = ["heading", "bold", "italic", "code", "ordered list", "task list", "table", "quote", "code block", "rule", "link"];
        println!(
            "conversation → PDF: 2 messages, {} block kinds ({}), {} bytes in {:.0} ms",
            kinds.len(),
            kinds.join(", "),
            bytes.len(),
            t.elapsed().as_secs_f64() * 1000.0
        );
        let _ = std::fs::remove_file(&out);
    }

    /// A conversation with no code embeds no monospaced family.
    #[test]
    fn prose_alone_embeds_only_the_body_font() {
        if body_family().is_none() {
            return;
        }
        let mut c = Conversation::new();
        c.append_as(AppendMode::Markdown, "**Você:** quantos dias de férias?", MessageRole::User);
        c.append_as(AppendMode::Markdown, "São **vinte** dias úteis — *proporcionais* em meio período.", MessageRole::Agent);
        let with_code = {
            let mut c2 = c.clone();
            c2.append_as(AppendMode::Markdown, "`LEAVE-DAYS`", MessageRole::Agent);
            c2
        };
        let size = |conv: &Conversation, tag: &str| {
            let out = std::env::temp_dir().join(format!("prc-viewer-pdf-{tag}-{}.pdf", std::process::id()));
            conversation_to_pdf(&conv.to_html(), "Férias", &out).expect("written");
            let n = std::fs::metadata(&out).unwrap().len();
            let _ = std::fs::remove_file(&out);
            n
        };
        let (prose, code) = (size(&c, "prose"), size(&with_code, "code"));
        println!("PDF size — prose only {prose} bytes; with one code run {code} bytes");
        assert!(prose < code || mono_family().is_none(), "no code → no monospaced faces embedded");
    }

    #[test]
    fn an_empty_conversation_is_refused_with_a_reason() {
        let out = std::env::temp_dir().join("prc-viewer-pdf-empty.pdf");
        let err = conversation_to_pdf("", "Empty", &out).unwrap_err();
        assert_eq!(err, "the conversation is empty");
        assert!(!out.exists(), "nothing is written for nothing");
    }
}
