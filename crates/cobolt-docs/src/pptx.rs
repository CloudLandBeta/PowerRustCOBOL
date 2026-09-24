// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! PowerPoint: one `## Slide N: <title>` section per slide, numbered as the
//! presentation shows them — empty slides keep their number — so a hit can
//! name its slide (spec 074 R9). Only text is read: no images, notes or
//! embedded objects.

use std::collections::HashMap;
use std::io::{Cursor, Read};

use quick_xml::events::Event;
use quick_xml::Reader;

use crate::{code, Skip};

pub fn convert(bytes: &[u8]) -> Result<String, Skip> {
    let damaged = |e: String| Skip::new(code::DAMAGED, format!("the presentation could not be read: {e}"));
    let mut zip = zip::ZipArchive::new(Cursor::new(bytes)).map_err(|e| damaged(e.to_string()))?;
    let slides = slide_order(&mut zip);
    if slides.is_empty() {
        return Err(damaged("it has no slides".into()));
    }
    let mut out = String::new();
    for (i, path) in slides.iter().enumerate() {
        let xml = read(&mut zip, path).map_err(damaged)?;
        let (title, body) = slide_text(&xml);
        out.push_str(&format!("## Slide {}", i + 1));
        if !title.is_empty() {
            out.push_str(": ");
            out.push_str(&title);
        }
        out.push_str("\n\n");
        for line in body {
            out.push_str(&line);
            out.push('\n');
        }
        out.push('\n');
    }
    Ok(out)
}

fn read(zip: &mut zip::ZipArchive<Cursor<&[u8]>>, name: &str) -> Result<String, String> {
    let mut file = zip.by_name(name).map_err(|e| format!("{name}: {e}"))?;
    let mut text = String::new();
    file.read_to_string(&mut text).map_err(|e| format!("{name}: {e}"))?;
    Ok(text)
}

/// Slide parts in presentation order: `presentation.xml`'s slide list through
/// its relationships, or — when those cannot be followed — by slide number.
fn slide_order(zip: &mut zip::ZipArchive<Cursor<&[u8]>>) -> Vec<String> {
    let listed = (|| {
        let pres = read(zip, "ppt/presentation.xml").ok()?;
        let rels = read(zip, "ppt/_rels/presentation.xml.rels").ok()?;
        let mut targets: HashMap<String, String> = HashMap::new();
        let mut reader = Reader::from_str(&rels);
        loop {
            match reader.read_event() {
                Ok(Event::Start(e)) | Ok(Event::Empty(e)) if e.local_name().as_ref() == b"Relationship" => {
                    if let (Some(id), Some(target)) = (attr(&e, b"Id"), attr(&e, b"Target")) {
                        let target = target.trim_start_matches('/');
                        let path = target
                            .strip_prefix("ppt/")
                            .map(|t| format!("ppt/{t}"))
                            .unwrap_or_else(|| format!("ppt/{target}"));
                        targets.insert(id, path);
                    }
                }
                Ok(Event::Eof) | Err(_) => break,
                _ => {}
            }
        }
        let mut order = Vec::new();
        let mut reader = Reader::from_str(&pres);
        loop {
            match reader.read_event() {
                Ok(Event::Start(e)) | Ok(Event::Empty(e)) if e.name().as_ref() == b"p:sldId" => {
                    if let Some(path) = attr(&e, b"r:id").and_then(|id| targets.get(&id)) {
                        order.push(path.clone());
                    }
                }
                Ok(Event::Eof) | Err(_) => break,
                _ => {}
            }
        }
        let all_present = order.iter().all(|p| zip.index_for_name(p).is_some());
        (!order.is_empty() && all_present).then_some(order)
    })();
    if let Some(order) = listed {
        return order;
    }
    let mut numbered: Vec<(u32, String)> = zip
        .file_names()
        .filter_map(|n| {
            let num = n.strip_prefix("ppt/slides/slide")?.strip_suffix(".xml")?.parse().ok()?;
            Some((num, n.to_string()))
        })
        .collect();
    numbered.sort();
    numbered.into_iter().map(|(_, n)| n).collect()
}

/// A slide's title (its title placeholder) and its other paragraphs.
fn slide_text(xml: &str) -> (String, Vec<String>) {
    let mut reader = Reader::from_str(xml);
    let mut title = Vec::new();
    let mut body = Vec::new();
    // Inside a shape: whether it is the title placeholder, and its paragraphs.
    let mut shape: Option<(bool, Vec<String>)> = None;
    let mut paragraph: Option<String> = None;
    let mut in_text = false;
    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) => match e.name().as_ref() {
                b"p:sp" => shape = Some((false, Vec::new())),
                b"a:p" => paragraph = Some(String::new()),
                b"a:t" => in_text = true,
                b"p:ph" => mark_title(&e, &mut shape),
                _ => {}
            },
            Ok(Event::Empty(e)) => match e.name().as_ref() {
                b"p:ph" => mark_title(&e, &mut shape),
                b"a:br" => {
                    if let Some(p) = paragraph.as_mut() {
                        p.push(' ');
                    }
                }
                _ => {}
            },
            Ok(Event::Text(t)) if in_text => {
                if let (Some(p), Ok(s)) = (paragraph.as_mut(), t.unescape()) {
                    p.push_str(&s);
                }
            }
            Ok(Event::End(e)) => match e.name().as_ref() {
                b"a:t" => in_text = false,
                b"a:p" => {
                    let text = paragraph.take().unwrap_or_default().trim().to_string();
                    if !text.is_empty() {
                        match shape.as_mut() {
                            Some((_, paras)) => paras.push(text),
                            None => body.push(text),
                        }
                    }
                }
                b"p:sp" => {
                    if let Some((is_title, paras)) = shape.take() {
                        if is_title && title.is_empty() {
                            title = paras;
                        } else {
                            body.extend(paras);
                        }
                    }
                }
                _ => {}
            },
            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
    }
    (title.join(" "), body)
}

fn mark_title(e: &quick_xml::events::BytesStart, shape: &mut Option<(bool, Vec<String>)>) {
    if let Some((is_title, _)) = shape.as_mut() {
        *is_title |= matches!(attr(e, b"type").as_deref(), Some("title" | "ctrTitle"));
    }
}

fn attr(e: &quick_xml::events::BytesStart, name: &[u8]) -> Option<String> {
    e.attributes()
        .flatten()
        .find(|a| a.key.as_ref() == name)
        .and_then(|a| a.unescape_value().ok().map(|v| v.into_owned()))
}
