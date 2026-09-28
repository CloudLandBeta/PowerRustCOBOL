// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! A Viewer's document or conversation written out as a PDF — `SaveAsPdf`.
//!
//! The PDF is what the Viewer PAINTS: the conversation is laid out by the
//! Viewer's own painter at the page's width — the user's and the agent's
//! bubbles in their colours, a page's CSS boxes, gradients, borders, flex
//! rows and grids, tables, code, a Mermaid diagram as its picture, the same
//! fonts and emoji — and those shapes are written as PDF operators by
//! [`crate::pdf_paint`]. Text stays text: it selects, copies and searches.
//!
//! It used to be a second layout of the same blocks through a PDF layout
//! library, which could draw neither a background nor a box, fell back to a
//! font with no emoji, and turned every styled answer into plain prose
//! (operator, 2026-09-28: "the PDF generation is not faithful to the
//! generated text in the chat").

use std::path::Path;

use crate::model::Control;
use crate::pdf_paint::{write_pdf, PageSetup};
use crate::viewer::MessageRole;

/// Write the conversation a `Streamed` Viewer holds (`_ConversationHtml`) to
/// `out` as a PDF titled `title`. `viewer` is the Viewer control, whose own
/// settings — bubble colours, `FontSize` — the painting follows.
pub fn conversation_to_pdf(viewer: &Control, conversation_html: &str, title: &str, out: &Path) -> Result<(), String> {
    let messages = crate::viewer::parse_conversation_html(conversation_html);
    if messages.iter().all(|m| m.blocks.is_empty()) {
        return Err("the conversation is empty".to_string());
    }
    let page = PageSetup::A4;
    let width = page.content_width_px();
    let bubbles = messages.iter().any(|m| m.role != MessageRole::None);
    let blocks: Vec<_> = messages.iter().flat_map(|m| m.blocks.clone()).collect();
    let paint = crate::paint::paint_viewer_export(viewer, bubbles.then_some(&messages[..]), &blocks, width);
    let bytes = write_pdf(&paint, title, page)?;
    std::fs::write(out, bytes).map_err(|e| format!("could not write the PDF: {e}"))
}

/// Write a Markdown (or plain text) document to `out` as a PDF titled
/// `title`, painted the way `viewer` shows it.
pub fn markdown_to_pdf(viewer: &Control, markdown: &str, title: &str, out: &Path) -> Result<(), String> {
    let parsed = crate::viewer::parse_markdown(markdown);
    if parsed.blocks.is_empty() {
        return Err("the document is empty".to_string());
    }
    let page = PageSetup::A4;
    let paint = crate::paint::paint_viewer_export(viewer, None, &parsed.blocks, page.content_width_px());
    let bytes = write_pdf(&paint, title, page)?;
    std::fs::write(out, bytes).map_err(|e| format!("could not write the PDF: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::viewer::{AppendMode, Conversation};
    use std::collections::HashMap;

    /// A Viewer set up the way PowerChat's is: its user bubble green.
    fn viewer() -> Control {
        let mut c = Control::new("VWR-CHAT", crate::model::ControlType::Viewer, 0, 0);
        c.set_prop("UserBubbleColor", crate::model::PropValue::String("#61D467FF".into()));
        c
    }

    /// The text a PDF reader recovers — each glyph code through its font's
    /// `ToUnicode` map, the path copy and search take — and the content
    /// streams, decompressed.
    fn read_back(bytes: &[u8]) -> (usize, String, String) {
        let doc = lopdf::Document::load_mem(bytes).expect("a PDF lopdf reads");
        let pages = doc.get_pages();
        let mut map: HashMap<(String, u16), String> = HashMap::new();
        let mut text = String::new();
        let mut ops = String::new();
        for (_, page_id) in &pages {
            let fonts = doc.get_page_fonts(*page_id);
            for (res, font) in &fonts {
                let res = String::from_utf8_lossy(res).to_string();
                let Ok(tu) = font.get(b"ToUnicode").and_then(|o| o.as_reference()) else { continue };
                let Ok(stream) = doc.get_object(tu).and_then(|o| o.as_stream()) else { continue };
                let cmap = String::from_utf8_lossy(&stream.decompressed_content().expect("inflates")).to_string();
                for line in cmap.lines() {
                    let parts: Vec<&str> = line.split_whitespace().collect();
                    if parts.len() == 2 && parts[0].starts_with('<') && parts[1].starts_with('<') {
                        let gid = u16::from_str_radix(parts[0].trim_matches(|c| c == '<' || c == '>'), 16).unwrap_or(0);
                        let hex = parts[1].trim_matches(|c| c == '<' || c == '>');
                        let units: Vec<u16> =
                            (0..hex.len() / 4).map(|i| u16::from_str_radix(&hex[i * 4..i * 4 + 4], 16).unwrap()).collect();
                        map.insert((res.clone(), gid), String::from_utf16_lossy(&units));
                    }
                }
            }
            let content = String::from_utf8_lossy(&doc.get_page_content(*page_id).expect("content")).to_string();
            let mut font = String::new();
            for tok in content.split_whitespace() {
                if let Some(f) = tok.strip_prefix('/').filter(|f| f.starts_with('F')) {
                    font = f.to_string();
                } else if tok.starts_with('<') && tok.ends_with('>') && tok.len() == 6 {
                    let gid = u16::from_str_radix(&tok[1..5], 16).unwrap_or(0);
                    text.push_str(map.get(&(font.clone(), gid)).map(String::as_str).unwrap_or("?"));
                }
            }
            ops.push_str(&content);
        }
        (pages.len(), text, ops)
    }

    /// Operator (2026-09-28): the PDF must be faithful to the chat. The
    /// user's green bubble, the agent's blue one, a styled HTML answer with
    /// its gradient banner, its cards, a round step badge and a grid — all
    /// painted — and every word still text a reader can search.
    #[test]
    fn a_conversation_pdf_is_what_the_viewer_painted() {
        let mut c = Conversation::new();
        c.append_as(AppendMode::Markdown, "Define un proceso para contratos", MessageRole::User);
        c.append_as(AppendMode::Markdown, "Aquí está, con **formato**:", MessageRole::Agent);
        c.append_as(
            AppendMode::Html,
            r#"<style>
              .banner { background: linear-gradient(90deg, #1e3c72, #2a5298); color: #fff; padding: 16px; border-radius: 12px }
              .step { display: flex; align-items: center; gap: 12px }
              .num { width: 36px; height: 36px; border-radius: 50%; background: #e74c3c; color: #fff; display: flex; align-items: center; justify-content: center }
              .grid { display: grid; grid-template-columns: repeat(2, 1fr); gap: 12px }
              .card { background: #fef9e7; padding: 10px; border-left: 5px solid #f39c12 }
            </style>
            <div class="banner"><h1>Proceso de Contratos</h1></div>
            <div class="step"><div class="num">1</div><p>Definición del documento 📄</p></div>
            <div class="grid"><div class="card"><p>Legal</p></div><div class="card"><p>Finanzas</p></div></div>"#,
            MessageRole::None,
        );
        let out = std::env::temp_dir().join(format!("prc-viewer-pdf-{}.pdf", std::process::id()));
        let _ = std::fs::remove_file(&out);
        let t = std::time::Instant::now();
        conversation_to_pdf(&viewer(), &c.to_html(), "Contratos — conversation", &out).expect("the PDF is written");
        let ms = t.elapsed().as_secs_f64() * 1000.0;
        let bytes = std::fs::read(&out).expect("the PDF exists");
        let _ = std::fs::remove_file(&out);
        // `PRC_KEEP_PDF=<path>` keeps a copy to look at.
        if let Ok(keep) = std::env::var("PRC_KEEP_PDF") {
            let _ = std::fs::write(keep, &bytes);
        }
        assert_eq!(&bytes[..5], b"%PDF-", "a PDF file");
        let (pages, text, ops) = read_back(&bytes);
        let colour = |hex: u32| {
            let f = |s: u32| {
                let v = format!("{:.3}", ((hex >> s) & 0xff) as f32 / 255.0);
                v.trim_end_matches('0').trim_end_matches('.').to_string()
            };
            format!("{} {} {} rg", f(16), f(8), f(0))
        };
        println!("\n  ── conversation → PDF ──");
        println!("  {} bytes, {pages} page(s), in {ms:.0} ms", bytes.len());
        println!("  text read back: {text:?}");
        let checks = [
            ("user bubble #61D467", ops.contains(&colour(0x61d467))),
            ("agent bubble #2C6FD2", ops.contains(&colour(0x2c6fd2))),
            ("banner gradient (shading)", ops.contains(" sh ")),
            ("step badge #E74C3C", ops.contains(&colour(0xe74c3c))),
            ("cards #FEF9E7", ops.contains(&colour(0xfef9e7))),
            ("card border #F39C12 (stroke)", ops.contains(&colour(0xf39c12).replace(" rg", " RG"))),
        ];
        for (what, ok) in &checks {
            println!("  {what:<28} {}", if *ok { "painted" } else { "MISSING" });
        }
        for (what, ok) in checks {
            assert!(ok, "{what}");
        }
        for word in ["Define un proceso para contratos", "Aquí está", "formato", "Proceso de Contratos", "Definición del documento", "📄", "Legal", "Finanzas"] {
            assert!(text.replace(' ', "").contains(&word.replace(' ', "")), "{word:?} reads back as text: {text:?}");
        }
    }

    /// A long document runs onto more pages, and no page is blank.
    #[test]
    fn a_long_document_runs_onto_more_pages() {
        let md: String = (1..=120).map(|i| format!("Paragraph number {i} of a long document.\n\n")).collect();
        let out = std::env::temp_dir().join(format!("prc-viewer-pdf-long-{}.pdf", std::process::id()));
        markdown_to_pdf(&viewer(), &md, "Long", &out).expect("written");
        let bytes = std::fs::read(&out).unwrap();
        let _ = std::fs::remove_file(&out);
        let (pages, text, _) = read_back(&bytes);
        println!("  120 paragraphs → {pages} pages, {} bytes", bytes.len());
        assert!(pages >= 3, "{pages} pages");
        let t = text.replace(' ', "");
        assert!(t.contains("Paragraphnumber1of") && t.contains("Paragraphnumber120of"), "first and last paragraphs read back");
    }

    #[test]
    fn an_empty_conversation_is_refused_with_a_reason() {
        let out = std::env::temp_dir().join("prc-viewer-pdf-empty.pdf");
        let err = conversation_to_pdf(&viewer(), "", "Empty", &out).unwrap_err();
        assert_eq!(err, "the conversation is empty");
        assert!(!out.exists(), "nothing is written for nothing");
    }
}
