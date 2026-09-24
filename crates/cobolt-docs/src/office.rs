// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Word, Excel, OpenDocument and delimited tables, through `markdownify`'s
//! format readers — called directly, below its own detection, which would
//! otherwise expand archives without bounds and turn an unreadable file into a
//! link rather than an error.

use std::collections::HashMap;
use std::io::{Cursor, Read, Write};

use quick_xml::events::Event;
use quick_xml::Reader;

use crate::{code, Skip};

fn damaged(e: impl std::fmt::Display) -> Skip {
    Skip::new(code::DAMAGED, format!("the document could not be read: {e}"))
}

/// Excel (`.xlsx`, `.xls`) and OpenDocument spreadsheets: `## <sheet>` and a
/// table per sheet.
pub fn workbook(bytes: &[u8]) -> Result<String, Skip> {
    markdownify::sheets::parse_sheets(bytes).map_err(damaged)
}

/// OpenDocument text and presentations.
pub fn opendoc(bytes: &[u8]) -> Result<String, Skip> {
    markdownify::opendoc::parse_opendoc(bytes, false).map_err(damaged)
}

/// A comma- or tab-separated table.
pub fn table(bytes: &[u8]) -> Result<String, Skip> {
    markdownify::sheets::parse_csv(bytes).map_err(damaged)
}

/// Word. Headings are found by **outline level**, whatever the style is
/// called, so a Portuguese "Título 1" is a heading as much as "Heading 1"
/// (operator, 2026-09-24): the reader below recognises only English style ids,
/// so each outline-level style is renamed `HeadingN` first.
pub fn docx(bytes: &[u8]) -> Result<String, Skip> {
    let normalised = normalise_headings(bytes).map_err(damaged)?;
    markdownify::docx::parse_docx(normalised.as_deref().unwrap_or(bytes), false).map_err(damaged)
}

/// The package with its heading styles renamed, or `None` when nothing needs
/// renaming.
fn normalise_headings(bytes: &[u8]) -> Result<Option<Vec<u8>>, String> {
    let mut zip = zip::ZipArchive::new(Cursor::new(bytes)).map_err(|e| e.to_string())?;
    let Some(styles) = read_member(&mut zip, "word/styles.xml")? else {
        return Ok(None);
    };
    let renames = heading_styles(&styles);
    if renames.is_empty() {
        return Ok(None);
    }
    let Some(document) = read_member(&mut zip, "word/document.xml")? else {
        return Ok(None);
    };
    let document = rename_paragraph_styles(&document, &renames);

    let mut out = zip::ZipWriter::new(Cursor::new(Vec::with_capacity(bytes.len())));
    for i in 0..zip.len() {
        let file = zip.by_index_raw(i).map_err(|e| e.to_string())?;
        if file.name() == "word/document.xml" {
            drop(file);
            let options = zip::write::SimpleFileOptions::default()
                .compression_method(zip::CompressionMethod::Deflated);
            out.start_file("word/document.xml", options).map_err(|e| e.to_string())?;
            out.write_all(document.as_bytes()).map_err(|e| e.to_string())?;
        } else {
            out.raw_copy_file(file).map_err(|e| e.to_string())?;
        }
    }
    Ok(Some(out.finish().map_err(|e| e.to_string())?.into_inner()))
}

fn read_member(
    zip: &mut zip::ZipArchive<Cursor<&[u8]>>,
    name: &str,
) -> Result<Option<String>, String> {
    let Ok(mut file) = zip.by_name(name) else {
        return Ok(None);
    };
    let mut text = String::new();
    file.read_to_string(&mut text).map_err(|e| e.to_string())?;
    Ok(Some(text))
}

/// Paragraph style id → `HeadingN`, for every style with an outline level
/// (its own or inherited through `basedOn`).
fn heading_styles(styles_xml: &str) -> HashMap<String, String> {
    struct Style {
        based_on: Option<String>,
        level: Option<u8>,
    }
    let mut styles: HashMap<String, Style> = HashMap::new();
    let mut reader = Reader::from_str(styles_xml);
    let mut current: Option<(String, Style)> = None;
    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) | Ok(Event::Empty(e)) => match e.name().as_ref() {
                b"w:style" => {
                    let paragraph = attr(&e, b"w:type").as_deref() == Some("paragraph");
                    current = match (paragraph, attr(&e, b"w:styleId")) {
                        (true, Some(id)) => Some((id, Style { based_on: None, level: None })),
                        _ => None,
                    };
                }
                b"w:basedOn" => {
                    if let Some((_, s)) = current.as_mut() {
                        s.based_on = attr(&e, b"w:val");
                    }
                }
                b"w:outlineLvl" => {
                    if let Some((_, s)) = current.as_mut() {
                        // 0–8 are levels; 9 is "body text".
                        s.level = attr(&e, b"w:val").and_then(|v| v.parse().ok()).filter(|l| *l < 9);
                    }
                }
                _ => {}
            },
            Ok(Event::End(e)) if e.name().as_ref() == b"w:style" => {
                if let Some((id, s)) = current.take() {
                    styles.insert(id, s);
                }
            }
            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
    }
    let level_of = |id: &str| -> Option<u8> {
        let mut id = id.to_string();
        for _ in 0..10 {
            let s = styles.get(&id)?;
            if s.level.is_some() {
                return s.level;
            }
            id = s.based_on.clone()?;
        }
        None
    };
    styles
        .keys()
        .filter_map(|id| level_of(id).map(|l| (id.clone(), format!("Heading{}", (l + 1).min(6)))))
        .collect()
}

fn attr(e: &quick_xml::events::BytesStart, name: &[u8]) -> Option<String> {
    e.attributes()
        .flatten()
        .find(|a| a.key.as_ref() == name)
        .and_then(|a| a.unescape_value().ok().map(|v| v.into_owned()))
}

/// Rewrite the value of every `<w:pStyle w:val="…"/>` found in `renames`.
fn rename_paragraph_styles(document: &str, renames: &HashMap<String, String>) -> String {
    const TAG: &str = "<w:pStyle";
    const VAL: &str = "w:val=\"";
    let mut out = String::with_capacity(document.len());
    let mut rest = document;
    while let Some(start) = rest.find(TAG) {
        let Some(tag_len) = rest[start..].find('>') else { break };
        let tag = &rest[start..start + tag_len];
        out.push_str(&rest[..start]);
        match tag.find(VAL).and_then(|v| {
            let from = v + VAL.len();
            tag[from..].find('"').map(|to| (from, from + to))
        }) {
            Some((from, to)) if renames.contains_key(&tag[from..to]) => {
                out.push_str(&tag[..from]);
                out.push_str(&renames[&tag[from..to]]);
                out.push_str(&tag[to..]);
            }
            _ => out.push_str(tag),
        }
        rest = &rest[start + tag_len..];
    }
    out.push_str(rest);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn outline_levels_name_headings_in_any_language() {
        let styles = r#"<w:styles>
            <w:style w:type="paragraph" w:styleId="Ttulo1"><w:name w:val="heading 1"/><w:pPr><w:outlineLvl w:val="0"/></w:pPr></w:style>
            <w:style w:type="paragraph" w:styleId="Ttulo2"><w:basedOn w:val="Ttulo1"/><w:pPr><w:outlineLvl w:val="1"/></w:pPr></w:style>
            <w:style w:type="paragraph" w:styleId="Capitulo"><w:basedOn w:val="Ttulo1"/></w:style>
            <w:style w:type="paragraph" w:styleId="Corpo"><w:pPr><w:outlineLvl w:val="9"/></w:pPr></w:style>
            <w:style w:type="character" w:styleId="Ttulo1Char"><w:pPr><w:outlineLvl w:val="0"/></w:pPr></w:style>
        </w:styles>"#;
        let map = heading_styles(styles);
        assert_eq!(map.get("Ttulo1").map(String::as_str), Some("Heading1"));
        assert_eq!(map.get("Ttulo2").map(String::as_str), Some("Heading2"));
        assert_eq!(map.get("Capitulo").map(String::as_str), Some("Heading1"), "inherited");
        assert!(!map.contains_key("Corpo"), "level 9 is body text");
        assert!(!map.contains_key("Ttulo1Char"), "character styles are not paragraphs");

        let doc = r#"<w:p><w:pPr><w:pStyle w:val="Ttulo2"/></w:pPr></w:p><w:p><w:pPr><w:pStyle w:val="Corpo"/></w:pPr></w:p>"#;
        let out = rename_paragraph_styles(doc, &map);
        assert!(out.contains(r#"<w:pStyle w:val="Heading2"/>"#), "{out}");
        assert!(out.contains(r#"<w:pStyle w:val="Corpo"/>"#), "{out}");
    }
}
