// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Render a documentation Markdown document to a PDF (for File → Print).
//!
//! The Markdown is walked with `pulldown-cmark` and mapped to `genpdf`
//! elements (headings, paragraphs with bold/italic runs, bullet lists, code
//! blocks). Each ```` ```mermaid ```` fenced block is rendered to a PNG (via the
//! shared Mermaid pipeline) and embedded as an image; if a diagram cannot be
//! rendered, its source is emitted as a code block instead, so export never
//! fails on an unsupported diagram.
//!
//! The font is a system sans-serif extracted at runtime (see
//! [`crate::fonts::pdf_font_bytes`]) — nothing is bundled.

use std::path::Path;

use genpdf::elements::{Break, Image, Paragraph};
use genpdf::fonts::{FontData, FontFamily};
use genpdf::style::Style;
use genpdf::{Alignment, Document, Scale, SimplePageDecorator};

use pulldown_cmark::{CodeBlockKind, Event, HeadingLevel, Options, Parser, Tag, TagEnd};

use crate::panels::doc_viewer::render_mermaid_pixmap;

/// Render `markdown` (titled `title`) to a PDF at `out`.
pub fn export(title: &str, markdown: &str, out: &Path) -> Result<(), String> {
    let bytes = crate::fonts::pdf_font_bytes()
        .ok_or_else(|| "no usable system font found for PDF export".to_string())?;
    let regular = FontData::new(bytes, None).map_err(|e| e.to_string())?;
    let family = FontFamily {
        regular: regular.clone(),
        bold: regular.clone(),
        italic: regular.clone(),
        bold_italic: regular,
    };

    let mut doc = Document::new(family);
    doc.set_title(title);
    doc.set_font_size(11);
    let mut deco = SimplePageDecorator::new();
    deco.set_margins(18);
    doc.set_page_decorator(deco);

    let mut w = Writer::new(out.to_path_buf());
    let parser = Parser::new_ext(markdown, Options::ENABLE_TABLES | Options::ENABLE_STRIKETHROUGH);
    for ev in parser {
        w.event(&mut doc, ev);
    }
    w.flush(&mut doc);

    doc.render_to_file(out).map_err(|e| e.to_string())
}

/// Incrementally builds genpdf elements from a Markdown event stream.
struct Writer {
    /// Output path (its parent dir holds temporary diagram PNGs).
    tmp_dir: std::path::PathBuf,
    /// Current block's styled runs.
    runs: Vec<(String, Style)>,
    /// Inline style flags.
    bold: u32,
    italic: u32,
    code: u32,
    /// Heading level of the current block, if any.
    heading: Option<HeadingLevel>,
    /// Bullet prefix when inside a list item.
    in_item: bool,
    /// Inside a fenced block: `Some(lang)`.
    fence: Option<String>,
    fence_buf: String,
    /// Counter for unique temp PNG names.
    diagram_n: u32,
}

impl Writer {
    fn new(out: std::path::PathBuf) -> Self {
        let tmp_dir = out.parent().map(|p| p.to_path_buf()).unwrap_or_else(std::env::temp_dir);
        Self {
            tmp_dir,
            runs: Vec::new(),
            bold: 0,
            italic: 0,
            code: 0,
            heading: None,
            in_item: false,
            fence: None,
            fence_buf: String::new(),
            diagram_n: 0,
        }
    }

    fn cur_style(&self) -> Style {
        let mut s = Style::new();
        if self.bold > 0 {
            s = s.bold();
        }
        if self.italic > 0 {
            s = s.italic();
        }
        s
    }

    fn push_text(&mut self, t: &str) {
        if self.fence.is_some() {
            self.fence_buf.push_str(t);
            return;
        }
        let style = self.cur_style();
        self.runs.push((t.to_string(), style));
    }

    /// Flush the accumulated runs as a paragraph and reset block state.
    fn flush(&mut self, doc: &mut Document) {
        if self.runs.is_empty() {
            return;
        }
        let mut p = Paragraph::default();
        if self.in_item {
            p.push_styled("• ", Style::new());
        }
        // Heading styling: bold + larger.
        let extra = self.heading.map(heading_pt);
        let runs = std::mem::take(&mut self.runs);
        for (text, mut style) in runs {
            if let Some(pt) = extra {
                style = style.bold().with_font_size(pt);
            }
            p.push_styled(text, style);
        }
        doc.push(p);
        doc.push(Break::new(if extra.is_some() { 0.6 } else { 0.35 }));
    }

    fn event(&mut self, doc: &mut Document, ev: Event) {
        match ev {
            Event::Start(tag) => self.start(doc, tag),
            Event::End(tag) => self.end(doc, tag),
            Event::Text(t) => self.push_text(&t),
            Event::Code(t) => {
                // Inline code — keep as plain text (single font family).
                self.push_text(&t);
            }
            Event::SoftBreak | Event::HardBreak => self.push_text(" "),
            Event::Rule => {
                self.flush(doc);
                doc.push(Break::new(0.3));
            }
            _ => {}
        }
    }

    fn start(&mut self, doc: &mut Document, tag: Tag) {
        match tag {
            Tag::Heading { level, .. } => {
                self.flush(doc);
                self.heading = Some(level);
            }
            Tag::Paragraph => {}
            Tag::Strong => self.bold += 1,
            Tag::Emphasis => self.italic += 1,
            Tag::List(_) => self.flush(doc),
            Tag::Item => {
                self.flush(doc);
                self.in_item = true;
            }
            Tag::CodeBlock(kind) => {
                self.flush(doc);
                let lang = match kind {
                    CodeBlockKind::Fenced(l) => l.to_string(),
                    CodeBlockKind::Indented => String::new(),
                };
                self.fence = Some(lang);
                self.fence_buf.clear();
            }
            Tag::Image { .. } => {
                // Skip local markdown images (not embedded in the binary).
                self.code += 1; // suppress alt-text emission until End
            }
            _ => {}
        }
    }

    fn end(&mut self, doc: &mut Document, tag: TagEnd) {
        match tag {
            TagEnd::Heading(_) => {
                self.flush(doc);
                self.heading = None;
            }
            TagEnd::Paragraph => self.flush(doc),
            TagEnd::Strong => self.bold = self.bold.saturating_sub(1),
            TagEnd::Emphasis => self.italic = self.italic.saturating_sub(1),
            TagEnd::Item => {
                self.flush(doc);
                self.in_item = false;
            }
            TagEnd::List(_) => doc.push(Break::new(0.2)),
            TagEnd::CodeBlock => {
                let lang = self.fence.take().unwrap_or_default();
                let body = std::mem::take(&mut self.fence_buf);
                if lang.eq_ignore_ascii_case("mermaid") {
                    if !self.embed_diagram(doc, &body) {
                        self.push_code_block(doc, &body);
                    }
                } else {
                    self.push_code_block(doc, &body);
                }
            }
            TagEnd::Image => self.code = self.code.saturating_sub(1),
            _ => {}
        }
    }

    /// Render a Mermaid diagram and embed it as a centered image. Returns false
    /// if it could not be rendered (caller then emits the source instead).
    fn embed_diagram(&mut self, doc: &mut Document, code: &str) -> bool {
        let pixmap = match render_mermaid_pixmap(code, 1.0) {
            Ok(p) => p,
            Err(_) => return false,
        };
        self.diagram_n += 1;
        let path = self.tmp_dir.join(format!(".prc-doc-diagram-{}.png", self.diagram_n));
        if pixmap.save_png(&path).is_err() {
            return false;
        }
        match Image::from_path(&path) {
            Ok(img) => {
                // Fit the page text width (~165 mm) without upscaling.
                let px_w = pixmap.width() as f64;
                let max_pt = 165.0 * 72.0 / 25.4; // mm → pt
                let scale = if px_w > max_pt { max_pt / px_w } else { 1.0 };
                doc.push(Break::new(0.3));
                doc.push(
                    img.with_alignment(Alignment::Center)
                        .with_scale(Scale::new(scale, scale)),
                );
                doc.push(Break::new(0.3));
                true
            }
            Err(_) => false,
        }
    }

    fn push_code_block(&mut self, doc: &mut Document, body: &str) {
        doc.push(Break::new(0.2));
        for line in body.lines() {
            let mut p = Paragraph::default();
            p.push_styled(line.to_string(), Style::new().with_color(genpdf::style::Color::Rgb(60, 60, 60)));
            doc.push(p);
        }
        doc.push(Break::new(0.3));
    }
}

/// Heading font size (points) by level.
fn heading_pt(level: HeadingLevel) -> u8 {
    match level {
        HeadingLevel::H1 => 20,
        HeadingLevel::H2 => 17,
        HeadingLevel::H3 => 15,
        HeadingLevel::H4 => 13,
        _ => 12,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exports_a_valid_pdf_including_a_mermaid_diagram() {
        let md = "# Title\n\nSome **bold** and *italic* text with a list:\n\n\
                  - one\n- two\n\n```mermaid\nflowchart LR\nA-->B-->C\n```\n\nClosing text.\n";
        let out = std::env::temp_dir().join("prc-doc-export-test.pdf");
        let _ = std::fs::remove_file(&out);
        export("Export Test", md, &out).expect("pdf export");
        let bytes = std::fs::read(&out).expect("read pdf");
        assert!(bytes.len() > 500, "pdf is non-trivial ({} bytes)", bytes.len());
        assert_eq!(&bytes[0..4], b"%PDF", "valid PDF header");
        let _ = std::fs::remove_file(&out);
    }
}
