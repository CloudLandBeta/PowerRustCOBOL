// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Spec 074 through the Knowledge Base: a folder of real formats is indexed,
//! hits name their page and their archive member, and what cannot be read is
//! reported by name while the rest is indexed. Run with `--nocapture` for the
//! result block (GOLDEN RULE #7).

use std::io::{Cursor, Write};
use std::sync::atomic::AtomicBool;
use std::time::Instant;

use cobolt_kb::convert::{code, Converters};
use cobolt_kb::embed::HashingEmbedder;
use cobolt_kb::refresh::{refresh, Scope};
use cobolt_kb::search::search;
use cobolt_kb::store::Collection;

fn zip_of(members: &[(&str, &[u8])]) -> Vec<u8> {
    let mut w = zip::ZipWriter::new(Cursor::new(Vec::new()));
    let opts = zip::write::SimpleFileOptions::default();
    for (name, data) in members {
        w.start_file(*name, opts).unwrap();
        w.write_all(data).unwrap();
    }
    w.finish().unwrap().into_inner()
}

fn pdf(pages: &[&str]) -> Vec<u8> {
    use lopdf::{dictionary, Document, Object, Stream};
    let mut doc = Document::with_version("1.5");
    let pages_id = doc.new_object_id();
    let font = doc.add_object(dictionary! { "Type" => "Font", "Subtype" => "Type1", "BaseFont" => "Helvetica" });
    let resources = doc.add_object(dictionary! { "Font" => dictionary! { "F1" => font } });
    let mut kids = Vec::new();
    for text in pages {
        let content = doc.add_object(Stream::new(
            dictionary! {},
            format!("BT /F1 18 Tf 72 700 Td ({text}) Tj ET\n").into_bytes(),
        ));
        let page = doc.add_object(dictionary! {
            "Type" => "Page", "Parent" => pages_id, "Contents" => content, "Resources" => resources,
            "MediaBox" => vec![0.into(), 0.into(), 612.into(), 792.into()],
        });
        kids.push(page.into());
    }
    let count = kids.len() as i64;
    doc.objects.insert(pages_id, Object::Dictionary(dictionary! { "Type" => "Pages", "Kids" => kids, "Count" => count }));
    let catalog = doc.add_object(dictionary! { "Type" => "Catalog", "Pages" => pages_id });
    doc.trailer.set("Root", catalog);
    let mut out = Vec::new();
    doc.save_to(&mut out).unwrap();
    out
}

#[test]
fn a_folder_of_formats_is_searchable_and_hits_name_page_and_member() {
    let root = tempfile::tempdir().unwrap();
    let c = Collection::open(root.path(), "handbook").unwrap();
    let docs = c.documents_dir();
    std::fs::write(
        docs.join("manual.pdf"),
        pdf(&["Installation steps", "The warranty covers the quetzalvalve for two years", "Index"]),
    )
    .unwrap();
    let old_doc = {
        let mut b = vec![0xD0, 0xCF, 0x11, 0xE0, 0xA1, 0xB1, 0x1A, 0xE1];
        b.extend(std::iter::repeat_n(0u8, 504));
        b.extend("WordDocument".encode_utf16().flat_map(u16::to_le_bytes));
        b
    };
    let inner = zip_of(&[("legal/nda.md", b"# NDA\nThe okapiclause binds both parties.")]);
    std::fs::write(
        docs.join("contracts-2025.zip"),
        zip_of(&[("2025/inner.zip", &inner), ("legal/old.doc", &old_doc)]),
    )
    .unwrap();
    std::fs::write(docs.join("setup.exe"), b"MZ\x90\x00").unwrap();
    std::fs::write(docs.join("notes.md"), "# Notes\nNothing special.").unwrap();

    let started = Instant::now();
    let out = refresh(&c, &HashingEmbedder, &Converters::default(), Scope::All, &mut |_| {}, &AtomicBool::new(false)).unwrap();
    let ms = started.elapsed().as_millis();
    assert_eq!(out.added, 3, "pdf, zip and md indexed; {:?}", out.skipped);
    let skipped: Vec<(&str, &str)> = out.skipped.iter().map(|(n, s)| (n.as_str(), s.code)).collect();
    assert_eq!(
        skipped,
        [
            ("contracts-2025.zip › legal/old.doc", code::LEGACY_OFFICE),
            ("setup.exe", code::UNSUPPORTED),
        ]
    );

    let pdf_hit = &search(&c, &HashingEmbedder, "quetzalvalve warranty", 3).unwrap().hits[0];
    assert_eq!(pdf_hit.document, "manual.pdf");
    assert!(pdf_hit.heading.ends_with("Page 2"), "{}", pdf_hit.heading);

    let zip_hit = &search(&c, &HashingEmbedder, "okapiclause", 3).unwrap().hits[0];
    assert_eq!(zip_hit.document, "contracts-2025.zip › 2025/inner.zip › legal/nda.md");
    assert!(zip_hit.passage.contains("okapiclause"));

    println!(
        "\n══ cobolt-kb — document import (spec 074) ══\n\
         indexed: manual.pdf (3 pages), contracts-2025.zip (zip in zip), notes.md — {} passages in {ms} ms\n\
         skipped: {:?}\n\
         hit 1: {} › {}\n\
         hit 2: {}",
        out.passages, skipped, pdf_hit.document, pdf_hit.heading, zip_hit.document
    );
}
