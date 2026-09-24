// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Spec 074 — every format, every skip reason and every archive bound,
//! through `cobolt-docs` alone: this test links no Knowledge Base (AC10a).
//!
//! Fixtures are built here with the same writers the formats use (zip, tar,
//! lopdf), so each one is a real, minimal document of its kind. Run with
//! `--nocapture` for the result table (GOLDEN RULE #7).

use std::io::{Cursor, Write};
use std::time::Instant;

use cobolt_docs::{code, convert, Converted, Limits, Skip};

// ── fixture writers ────────────────────────────────────────────────────────

fn zip_of(members: &[(&str, &[u8])]) -> Vec<u8> {
    let mut w = zip::ZipWriter::new(Cursor::new(Vec::new()));
    let opts = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated);
    for (name, data) in members {
        w.start_file(*name, opts).unwrap();
        w.write_all(data).unwrap();
    }
    w.finish().unwrap().into_inner()
}

fn tar_of(members: &[(&str, &[u8])]) -> Vec<u8> {
    let mut b = tar::Builder::new(Vec::new());
    for (name, data) in members {
        let mut h = tar::Header::new_gnu();
        h.set_size(data.len() as u64);
        h.set_mode(0o644);
        h.set_cksum();
        b.append_data(&mut h, name, *data).unwrap();
    }
    b.into_inner().unwrap()
}

fn gzip(data: &[u8]) -> Vec<u8> {
    let mut e = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
    e.write_all(data).unwrap();
    e.finish().unwrap()
}

const W: &str = r#"xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main""#;

/// A Word document whose headings use Portuguese style ids — found by their
/// outline level, not their name.
fn docx() -> Vec<u8> {
    let para = |style: Option<&str>, text: &str| {
        let ppr = style.map(|s| format!(r#"<w:pPr><w:pStyle w:val="{s}"/></w:pPr>"#)).unwrap_or_default();
        format!(r#"<w:p>{ppr}<w:r><w:t>{text}</w:t></w:r></w:p>"#)
    };
    let body = [
        para(Some("Ttulo1"), "Leave policy"),
        para(None, "Every employee earns zebracorn days of annual leave."),
        para(Some("Ttulo2"), "Carry over"),
        para(None, "Five days carry over to the next year."),
        para(Some("Ttulo1"), "Expenses"),
        para(None, "Receipts are filed within thirty days."),
    ]
    .concat();
    let document = format!(r#"<?xml version="1.0"?><w:document {W}><w:body>{body}</w:body></w:document>"#);
    let styles = format!(
        r#"<?xml version="1.0"?><w:styles {W}>
        <w:style w:type="paragraph" w:styleId="Ttulo1"><w:name w:val="Título 1"/><w:pPr><w:outlineLvl w:val="0"/></w:pPr></w:style>
        <w:style w:type="paragraph" w:styleId="Ttulo2"><w:name w:val="Título 2"/><w:pPr><w:outlineLvl w:val="1"/></w:pPr></w:style>
        </w:styles>"#
    );
    zip_of(&[
        ("[Content_Types].xml", br#"<?xml version="1.0"?><Types/>"#),
        ("word/document.xml", document.as_bytes()),
        ("word/styles.xml", styles.as_bytes()),
    ])
}

const P: &str = r#"xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main" xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships""#;

/// Three slides, listed out of file order, the second one empty.
fn pptx() -> Vec<u8> {
    let slide = |title: &str, body: &str| {
        let shape = |ph: &str, text: &str| {
            if text.is_empty() {
                return String::new();
            }
            format!(r#"<p:sp><p:nvSpPr><p:nvPr>{ph}</p:nvPr></p:nvSpPr><p:txBody><a:p><a:r><a:t>{text}</a:t></a:r></a:p></p:txBody></p:sp>"#)
        };
        format!(
            r#"<?xml version="1.0"?><p:sld {P}><p:cSld><p:spTree>{}{}</p:spTree></p:cSld></p:sld>"#,
            shape(r#"<p:ph type="title"/>"#, title),
            shape("", body)
        )
    };
    let pres = format!(
        r#"<?xml version="1.0"?><p:presentation {P}><p:sldIdLst><p:sldId id="256" r:id="rId7"/><p:sldId id="257" r:id="rId8"/><p:sldId id="258" r:id="rId9"/></p:sldIdLst></p:presentation>"#
    );
    let rels = r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId7" Target="slides/slide3.xml"/><Relationship Id="rId8" Target="slides/slide1.xml"/><Relationship Id="rId9" Target="slides/slide2.xml"/></Relationships>"#;
    let s3 = slide("Pricing", "The quokkaberry plan costs ten dollars.");
    let s1 = slide("", "");
    let s2 = slide("Support", "Tickets are answered within a day.");
    zip_of(&[
        ("[Content_Types].xml", br#"<?xml version="1.0"?><Types/>"#),
        ("ppt/presentation.xml", pres.as_bytes()),
        ("ppt/_rels/presentation.xml.rels", rels.as_bytes()),
        ("ppt/slides/slide1.xml", s1.as_bytes()),
        ("ppt/slides/slide2.xml", s2.as_bytes()),
        ("ppt/slides/slide3.xml", s3.as_bytes()),
    ])
}

/// Two sheets, inline strings.
fn xlsx() -> Vec<u8> {
    let sheet = |rows: &[[&str; 2]]| {
        let rows: String = rows
            .iter()
            .enumerate()
            .map(|(i, r)| {
                let n = i + 1;
                format!(
                    r#"<row r="{n}"><c r="A{n}" t="inlineStr"><is><t>{}</t></is></c><c r="B{n}" t="inlineStr"><is><t>{}</t></is></c></row>"#,
                    r[0], r[1]
                )
            })
            .collect();
        format!(r#"<?xml version="1.0"?><worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main"><sheetData>{rows}</sheetData></worksheet>"#)
    };
    let workbook = r#"<?xml version="1.0"?><workbook xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships"><sheets><sheet name="Prices" sheetId="1" r:id="rId1"/><sheet name="Stock" sheetId="2" r:id="rId2"/></sheets></workbook>"#;
    let rels = r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/worksheet" Target="worksheets/sheet1.xml"/><Relationship Id="rId2" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/worksheet" Target="worksheets/sheet2.xml"/></Relationships>"#;
    let types = r#"<?xml version="1.0"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="xml" ContentType="application/xml"/><Override PartName="/xl/workbook.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.sheet.main+xml"/><Override PartName="/xl/worksheets/sheet1.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.worksheet+xml"/><Override PartName="/xl/worksheets/sheet2.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.worksheet+xml"/></Types>"#;
    let s1 = sheet(&[["Item", "Price"], ["narwhalnut", "12"]]);
    let s2 = sheet(&[["Item", "Units"], ["widget", "40"]]);
    zip_of(&[
        ("[Content_Types].xml", types.as_bytes()),
        ("_rels/.rels", br#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="xl/workbook.xml"/></Relationships>"#),
        ("xl/workbook.xml", workbook.as_bytes()),
        ("xl/_rels/workbook.xml.rels", rels.as_bytes()),
        ("xl/worksheets/sheet1.xml", s1.as_bytes()),
        ("xl/worksheets/sheet2.xml", s2.as_bytes()),
    ])
}

const ODF_NS: &str = r#"xmlns:office="urn:oasis:names:tc:opendocument:xmlns:office:1.0" xmlns:text="urn:oasis:names:tc:opendocument:xmlns:text:1.0" xmlns:table="urn:oasis:names:tc:opendocument:xmlns:table:1.0""#;

fn odt() -> Vec<u8> {
    let content = format!(
        r#"<?xml version="1.0"?><office:document-content {ODF_NS}><office:body><office:text><text:h text:outline-level="1">Travel</text:h><text:p>Trains are booked through the axolotlpass portal.</text:p></office:text></office:body></office:document-content>"#
    );
    zip_of(&[
        ("mimetype", b"application/vnd.oasis.opendocument.text"),
        ("content.xml", content.as_bytes()),
    ])
}

fn ods() -> Vec<u8> {
    let content = format!(
        r#"<?xml version="1.0"?><office:document-content {ODF_NS}><office:body><office:spreadsheet><table:table table:name="Rooms"><table:table-row><table:table-cell office:value-type="string"><text:p>Room</text:p></table:table-cell></table:table-row><table:table-row><table:table-cell office:value-type="string"><text:p>pangolinhall</text:p></table:table-cell></table:table-row></table:table></office:spreadsheet></office:body></office:document-content>"#
    );
    let manifest = r#"<?xml version="1.0"?><manifest:manifest xmlns:manifest="urn:oasis:names:tc:opendocument:xmlns:manifest:1.0"><manifest:file-entry manifest:full-path="/" manifest:media-type="application/vnd.oasis.opendocument.spreadsheet"/><manifest:file-entry manifest:full-path="content.xml" manifest:media-type="text/xml"/></manifest:manifest>"#;
    zip_of(&[
        ("mimetype", b"application/vnd.oasis.opendocument.spreadsheet"),
        ("META-INF/manifest.xml", manifest.as_bytes()),
        ("content.xml", content.as_bytes()),
    ])
}

/// A PDF of `pages` pages, page N saying `"<text> page N"` — or nothing.
fn pdf(pages: usize, text: &str) -> Vec<u8> {
    use lopdf::{dictionary, Document, Object, Stream};
    let mut doc = Document::with_version("1.5");
    let pages_id = doc.new_object_id();
    let font = doc.add_object(dictionary! { "Type" => "Font", "Subtype" => "Type1", "BaseFont" => "Helvetica" });
    let resources = doc.add_object(dictionary! { "Font" => dictionary! { "F1" => font } });
    let mut kids = Vec::new();
    for i in 0..pages {
        let stream = if text.is_empty() {
            "100 100 200 150 re S\n".to_string()
        } else {
            format!("BT /F1 18 Tf 72 700 Td ({text} page {}) Tj ET\n", i + 1)
        };
        let content = doc.add_object(Stream::new(dictionary! {}, stream.into_bytes()));
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

fn compound(stream: &str) -> Vec<u8> {
    let mut b = vec![0xD0, 0xCF, 0x11, 0xE0, 0xA1, 0xB1, 0x1A, 0xE1];
    b.extend(std::iter::repeat_n(0u8, 504));
    b.extend(stream.encode_utf16().flat_map(u16::to_le_bytes));
    b
}

// ── helpers ────────────────────────────────────────────────────────────────

fn one(name: &str, bytes: &[u8]) -> String {
    let c = convert(name, bytes, &Limits::default()).unwrap_or_else(|s| panic!("{name}: {s:?}"));
    assert_eq!(c.parts.len(), 1, "{name}");
    c.parts.into_iter().next().unwrap().markdown
}

fn skip(name: &str, bytes: &[u8]) -> Skip {
    convert(name, bytes, &Limits::default()).expect_err(name)
}

fn headings(md: &str) -> Vec<String> {
    md.lines().filter(|l| l.starts_with('#')).map(|l| l.trim().to_string()).collect()
}

fn part_names(c: &Converted) -> Vec<String> {
    c.parts.iter().map(|p| p.name.clone().unwrap_or_default()).collect()
}

// ── AC1: every format, found by a phrase only it holds ─────────────────────

#[test]
fn every_format_converts_and_keeps_its_unique_phrase() {
    let html = br#"<!doctype html><html><head><title>t</title><style>.x{}</style></head><body><nav>Home | About</nav><h1>Parking</h1><p>Visitors park at the dugongdeck.</p><script>var secret=1;</script></body></html>"#;
    let fixtures: Vec<(&str, Vec<u8>, &str)> = vec![
        ("policy.docx", docx(), "zebracorn"),
        ("sales.pptx", pptx(), "quokkaberry"),
        ("prices.xlsx", xlsx(), "narwhalnut"),
        ("travel.odt", odt(), "axolotlpass"),
        ("rooms.ods", ods(), "pangolinhall"),
        ("stock.csv", b"item,qty\nokapiorb,4\n".to_vec(), "okapiorb"),
        ("stock.tsv", b"item\tqty\nplatypod\t4\n".to_vec(), "platypod"),
        ("manual.pdf", pdf(3, "tapirtome"), "tapirtome"),
        ("visitors.html", html.to_vec(), "dugongdeck"),
        ("notes.md", b"# Notes\nThe lemurlog.".to_vec(), "lemurlog"),
        ("readme.txt", b"The ibexinfo.".to_vec(), "ibexinfo"),
        ("bundle.zip", zip_of(&[("inner/policy.docx", &docx())]), "zebracorn"),
        ("bundle.tar", tar_of(&[("inner/stock.csv", b"item\nkudukey\n")]), "kudukey"),
        ("bundle.tar.gz", gzip(&tar_of(&[("a.md", b"# A\nthe gnugist")])), "gnugist"),
    ];

    println!("\n══ cobolt-docs — formats (spec 074 AC1, AC11) ══");
    println!("{:<16} {:>8} {:>8} {:>10} {:>10}", "document", "bytes", "chars", "ms/doc", "ms/MB");
    for (name, bytes, phrase) in &fixtures {
        const RUNS: u32 = 20;
        let started = Instant::now();
        let mut c = Converted::default();
        for _ in 0..RUNS {
            c = convert(name, bytes, &Limits::default()).unwrap_or_else(|s| panic!("{name}: {s:?}"));
        }
        let ms = started.elapsed().as_secs_f64() * 1000.0 / RUNS as f64;
        let text: String = c.parts.iter().map(|p| p.markdown.as_str()).collect();
        assert!(text.contains(phrase), "{name}: {phrase:?} not in\n{text}");
        assert!(c.skipped.is_empty(), "{name}: {:?}", c.skipped);
        let mb = bytes.len() as f64 / (1024.0 * 1024.0);
        println!(
            "{:<16} {:>8} {:>8} {:>10.3} {:>10.1}",
            name,
            bytes.len(),
            text.chars().count(),
            ms,
            ms / mb
        );
    }
    println!("{} formats converted, each found by its own phrase", fixtures.len());

    // HTML keeps headings and drops scripts, styles and navigation (R5).
    let md = one("visitors.html", html);
    assert!(md.contains("# Parking"), "{md}");
    for gone in ["secret", "Home | About", ".x{}"] {
        assert!(!md.contains(gone), "{gone:?} kept in {md}");
    }
}

// ── AC2: content before name ───────────────────────────────────────────────

#[test]
fn a_misnamed_document_is_read_by_its_content() {
    assert!(one("policy.txt", &docx()).contains("zebracorn"));
    assert!(one("manual", &pdf(1, "tapirtome")).contains("tapirtome"));
    assert!(one("deck.bin", &pptx()).contains("quokkaberry"));
}

// ── AC3: structure — headings, slides, sheets, pages ───────────────────────

#[test]
fn structure_survives_as_headings() {
    let word = headings(&one("policy.docx", &docx()));
    assert_eq!(word, ["# Leave policy", "## Carry over", "# Expenses"], "outline levels, Portuguese style ids");

    let deck = one("sales.pptx", &pptx());
    assert_eq!(
        headings(&deck),
        ["## Slide 1: Pricing", "## Slide 2", "## Slide 3: Support"],
        "presentation order, the empty slide keeps its number"
    );

    let sheets = headings(&one("prices.xlsx", &xlsx()));
    assert_eq!(sheets, ["## Prices", "## Stock"]);

    let pages = headings(&one("manual.pdf", &pdf(3, "tapirtome")));
    assert_eq!(pages, ["## Page 1", "## Page 2", "## Page 3"]);
    println!("structure: docx 3 headings (2 levels), pptx 3 slides, xlsx 2 sheets, pdf 3 pages");
}

// ── AC4 / R10: nested archives name their path ─────────────────────────────

#[test]
fn a_document_in_a_zip_in_a_zip_is_named_through_both() {
    let inner = zip_of(&[("legal/nda.docx", &docx()), ("legal/old.doc", &compound("WordDocument"))]);
    let outer = zip_of(&[
        ("2025/inner.zip", &inner),
        ("readme.md", b"# Read me\nhello"),
        ("__MACOSX/._readme.md", b"junk"),
    ]);
    let c = convert("contracts-2025.zip", &outer, &Limits::default()).unwrap();
    assert_eq!(part_names(&c), ["2025/inner.zip › legal/nda.docx", "readme.md"]);
    assert!(c.parts[0].markdown.contains("zebracorn"));
    let skipped: Vec<(&str, &str)> = c.skipped.iter().map(|(n, s)| (n.as_str(), s.code)).collect();
    assert_eq!(skipped, [("2025/inner.zip › legal/old.doc", code::LEGACY_OFFICE)], "a bad member skips alone");
}

// ── AC5: each unreadable document with its reason ──────────────────────────

#[test]
fn unreadable_documents_are_skipped_with_the_right_reason() {
    let mut truncated = pptx();
    truncated.truncate(truncated.len() / 2);
    let cases: Vec<(&str, Vec<u8>, &str)> = vec![
        ("letter.doc", compound("WordDocument"), code::LEGACY_OFFICE),
        ("deck.ppt", compound("PowerPoint Document"), code::LEGACY_OFFICE),
        ("secret.docx", compound("EncryptedPackage"), code::PASSWORD_PROTECTED),
        ("scan.pdf", pdf(2, ""), code::NO_TEXT),
        ("broken.pptx", truncated, code::DAMAGED),
        ("setup.exe", b"MZ\x90\x00\x03\x00\x00\x00".to_vec(), code::UNSUPPORTED),
        ("photo.png", b"\x89PNG\r\n\x1a\n\0\0".to_vec(), code::UNSUPPORTED),
    ];
    println!("\n══ cobolt-docs — skip reasons (spec 074 AC5) ══");
    for (name, bytes, want) in &cases {
        let s = skip(name, bytes);
        assert_eq!(s.code, *want, "{name}: {}", s.message);
        println!("{name:<14} → {:<18} {}", s.code, s.message);
    }
}

// ── AC6: bounds — size, count, depth ───────────────────────────────────────

#[test]
fn an_archive_past_any_bound_is_too_large() {
    let small = Limits { max_unpacked_bytes: 1024 * 1024, max_files: 10, max_depth: 2 };

    // A bomb: 8 MB of zeros that compresses to a few kilobytes.
    let zeros = vec![b'a'; 8 * 1024 * 1024];
    let bomb = zip_of(&[("big.txt", &zeros)]);
    let started = Instant::now();
    let s = convert("bomb.zip", &bomb, &small).unwrap_err();
    assert_eq!(s.code, code::TOO_LARGE, "{}", s.message);
    let bomb_ms = started.elapsed().as_millis();

    let many: Vec<(String, Vec<u8>)> = (0..11).map(|i| (format!("f{i}.txt"), b"x".to_vec())).collect();
    let many_refs: Vec<(&str, &[u8])> = many.iter().map(|(n, b)| (n.as_str(), b.as_slice())).collect();
    assert_eq!(convert("many.zip", &zip_of(&many_refs), &small).unwrap_err().code, code::TOO_LARGE);
    assert!(convert("ten.zip", &zip_of(&many_refs[..10]), &small).is_ok(), "ten is within the bound");

    let level1 = zip_of(&[("deep.md", b"# Deep\ntext")]);
    let level2 = zip_of(&[("l1.zip", &level1)]);
    let level3 = zip_of(&[("l2.zip", &level2)]);
    assert!(convert("two.zip", &level2, &small).is_ok(), "two levels allowed");
    assert_eq!(convert("three.zip", &level3, &small).unwrap_err().code, code::TOO_LARGE);

    // gzip is transparent, but its bytes count.
    let gz_bomb = gzip(&tar_of(&[("big.txt", &zeros)]));
    assert_eq!(convert("bomb.tar.gz", &gz_bomb, &small).unwrap_err().code, code::TOO_LARGE);

    println!(
        "\n══ cobolt-docs — bounds (spec 074 AC6) ══\n\
         8 MB bomb ({} bytes packed) refused in {bomb_ms} ms at a 1 MB bound; \
         11 files refused at 10; 3 levels refused at 2; gzip bomb refused",
        bomb.len()
    );
}

// ── AC7: a member path is only ever a name ─────────────────────────────────

#[test]
fn a_member_path_cannot_escape() {
    let archive = zip_of(&[("../../escape.txt", b"the tamarintext"), ("/abs/x.md", b"# X\ny")]);
    let c = convert("evil.zip", &archive, &Limits::default()).unwrap();
    assert_eq!(part_names(&c), ["escape.txt", "abs/x.md"]);
    // TAR: the writer refuses `..`, so the header is set by hand, as a
    // hostile archive would be.
    let mut b = tar::Builder::new(Vec::new());
    let mut h = tar::Header::new_old();
    let name = b"docs/../../../up.md";
    h.as_old_mut().name[..name.len()].copy_from_slice(name);
    h.set_size(9);
    h.set_mode(0o644);
    h.set_cksum();
    b.append(&h, &b"# Up\nokay"[..9]).unwrap();
    let c = convert("evil.tar", &b.into_inner().unwrap(), &Limits::default()).unwrap();
    assert_eq!(part_names(&c), ["docs/up.md"]);
    println!("escaping members: ../../escape.txt → escape.txt, /abs/x.md → abs/x.md, docs/../../../up.md → docs/up.md; nothing written to disk");
}
