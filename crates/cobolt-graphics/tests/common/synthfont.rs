// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! A tiny TrueType font builder for tests.
//!
//! The tests need fonts that cover Latin, Japanese, Chinese, Arabic and Hebrew
//! text, that are legal to keep in the repository and that give exact, known
//! metrics. So they are generated here: every glyph is a rectangle of known
//! advance and height (the height identifies the glyph, which is how a test reads
//! the visual order of a line back from its outline). There is no shaping
//! table - joining, ligatures and kerning are the shaper's job and are tested with
//! the real fonts where the machine has them.

/// One character of a synthetic font.
#[derive(Clone, Copy)]
pub struct G {
    /// The character.
    pub ch: char,
    /// Advance width in font units (the em is 1000).
    pub advance: u16,
    /// Height of the box; 0 for a glyph with no ink (a space).
    pub height: i16,
}

fn u16be(v: &mut Vec<u8>, x: u16) {
    v.extend_from_slice(&x.to_be_bytes());
}
fn i16be(v: &mut Vec<u8>, x: i16) {
    v.extend_from_slice(&x.to_be_bytes());
}
fn u32be(v: &mut Vec<u8>, x: u32) {
    v.extend_from_slice(&x.to_be_bytes());
}

fn checksum(data: &[u8]) -> u32 {
    let mut sum = 0u32;
    for chunk in data.chunks(4) {
        let mut b = [0u8; 4];
        b[..chunk.len()].copy_from_slice(chunk);
        sum = sum.wrapping_add(u32::from_be_bytes(b));
    }
    sum
}

/// Build a TrueType font named `family` with a `.notdef` box and the given glyphs.
/// Ascent 800, descent 200, em 1000.
pub fn build(family: &str, glyphs: &[G]) -> Vec<u8> {
    let upem: u16 = 1000;
    // Glyph 0 is .notdef: a box 500 wide, 700 high.
    let mut all: Vec<(Option<char>, u16, i16)> = vec![(None, 500, 700)];
    for g in glyphs {
        all.push((Some(g.ch), g.advance, g.height));
    }
    let n = all.len() as u16;

    // glyf + loca (short offsets).
    let mut glyf = Vec::new();
    let mut loca: Vec<u16> = Vec::new();
    for (_, adv, h) in &all {
        loca.push((glyf.len() / 2) as u16);
        if *h > 0 {
            let (x0, x1) = (50i16, (*adv as i16 - 50).max(51));
            let (y0, y1) = (0i16, *h);
            i16be(&mut glyf, 1); // one contour
            i16be(&mut glyf, x0);
            i16be(&mut glyf, y0);
            i16be(&mut glyf, x1);
            i16be(&mut glyf, y1);
            u16be(&mut glyf, 3); // end point of contour 0
            u16be(&mut glyf, 0); // no instructions
            glyf.extend_from_slice(&[1, 1, 1, 1]); // on-curve, 16-bit deltas
            for dx in [x0, x1 - x0, 0, x0 - x1] {
                i16be(&mut glyf, dx);
            }
            for dy in [y0, 0, y1 - y0, 0] {
                i16be(&mut glyf, dy);
            }
            if glyf.len() % 2 == 1 {
                glyf.push(0);
            }
        }
    }
    loca.push((glyf.len() / 2) as u16);
    let mut loca_b = Vec::new();
    for o in &loca {
        u16be(&mut loca_b, *o);
    }

    // head
    let mut head = Vec::new();
    u32be(&mut head, 0x0001_0000);
    u32be(&mut head, 0x0001_0000);
    u32be(&mut head, 0);
    u32be(&mut head, 0x5F0F_3CF5);
    u16be(&mut head, 0x000B);
    u16be(&mut head, upem);
    head.extend_from_slice(&[0; 16]); // created, modified
    for v in [0i16, 0, 1000, 1000] {
        i16be(&mut head, v);
    }
    u16be(&mut head, 0); // macStyle
    u16be(&mut head, 8);
    i16be(&mut head, 2);
    i16be(&mut head, 0); // short loca
    i16be(&mut head, 0);

    // hhea
    let mut hhea = Vec::new();
    u32be(&mut hhea, 0x0001_0000);
    i16be(&mut hhea, 800);
    i16be(&mut hhea, -200);
    i16be(&mut hhea, 0);
    u16be(&mut hhea, all.iter().map(|g| g.1).max().unwrap_or(0));
    for v in [0i16, 0, 1000, 1, 0, 0, 0, 0, 0, 0, 0] {
        i16be(&mut hhea, v);
    }
    u16be(&mut hhea, n);

    // maxp 1.0
    let mut maxp = Vec::new();
    u32be(&mut maxp, 0x0001_0000);
    u16be(&mut maxp, n);
    for v in [4u16, 1, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0] {
        u16be(&mut maxp, v);
    }

    // hmtx
    let mut hmtx = Vec::new();
    for (_, adv, _) in &all {
        u16be(&mut hmtx, *adv);
        i16be(&mut hmtx, 50);
    }

    // cmap, format 12
    let mut chars: Vec<(u32, u32)> = all.iter().enumerate().filter_map(|(i, g)| g.0.map(|c| (c as u32, i as u32))).collect();
    chars.sort();
    let mut sub = Vec::new();
    u16be(&mut sub, 12);
    u16be(&mut sub, 0);
    u32be(&mut sub, 16 + 12 * chars.len() as u32);
    u32be(&mut sub, 0);
    u32be(&mut sub, chars.len() as u32);
    for (c, gid) in &chars {
        u32be(&mut sub, *c);
        u32be(&mut sub, *c);
        u32be(&mut sub, *gid);
    }
    let mut cmap = Vec::new();
    u16be(&mut cmap, 0);
    u16be(&mut cmap, 2);
    // (0, 4) Unicode full repertoire and (3, 10) Windows UCS-4, same subtable.
    u16be(&mut cmap, 0);
    u16be(&mut cmap, 4);
    u32be(&mut cmap, 20);
    u16be(&mut cmap, 3);
    u16be(&mut cmap, 10);
    u32be(&mut cmap, 20);
    cmap.extend_from_slice(&sub);

    // post, format 3
    let mut post = Vec::new();
    u32be(&mut post, 0x0003_0000);
    u32be(&mut post, 0);
    i16be(&mut post, -100);
    i16be(&mut post, 50);
    for _ in 0..5 {
        u32be(&mut post, 0);
    }

    // name: family (1), subfamily (2), full (4), PostScript (6), platform 3 / enc 1 / English US
    let ps = family.replace(' ', "");
    let strings: [(u16, String); 4] = [(1, family.to_string()), (2, "Regular".into()), (4, family.to_string()), (6, ps)];
    let mut name = Vec::new();
    u16be(&mut name, 0);
    u16be(&mut name, strings.len() as u16);
    u16be(&mut name, 6 + 12 * strings.len() as u16);
    let mut pool = Vec::new();
    for (id, s) in &strings {
        let utf16: Vec<u8> = s.encode_utf16().flat_map(|u| u.to_be_bytes()).collect();
        u16be(&mut name, 3);
        u16be(&mut name, 1);
        u16be(&mut name, 0x0409);
        u16be(&mut name, *id);
        u16be(&mut name, utf16.len() as u16);
        u16be(&mut name, pool.len() as u16);
        pool.extend_from_slice(&utf16);
    }
    name.extend_from_slice(&pool);

    let mut tables: Vec<([u8; 4], Vec<u8>)> = vec![
        (*b"cmap", cmap),
        (*b"glyf", glyf),
        (*b"head", head),
        (*b"hhea", hhea),
        (*b"hmtx", hmtx),
        (*b"loca", loca_b),
        (*b"maxp", maxp),
        (*b"name", name),
        (*b"post", post),
    ];
    tables.sort_by_key(|t| t.0);

    let count = tables.len() as u16;
    let mut out = Vec::new();
    u32be(&mut out, 0x0001_0000);
    u16be(&mut out, count);
    let mut sr = 1u16;
    let mut es = 0u16;
    while sr * 2 <= count {
        sr *= 2;
        es += 1;
    }
    u16be(&mut out, sr * 16);
    u16be(&mut out, es);
    u16be(&mut out, count * 16 - sr * 16);
    let mut offset = 12 + 16 * tables.len();
    let mut padded: Vec<Vec<u8>> = Vec::new();
    for (tag, data) in &tables {
        out.extend_from_slice(tag);
        u32be(&mut out, checksum(data));
        u32be(&mut out, offset as u32);
        u32be(&mut out, data.len() as u32);
        let mut d = data.clone();
        while d.len() % 4 != 0 {
            d.push(0);
        }
        offset += d.len();
        padded.push(d);
    }
    for d in padded {
        out.extend_from_slice(&d);
    }
    out
}

/// Printable ASCII, advance 600 for letters and digits, 300 for the space; the
/// height of a letter depends on the letter so a box can be told from another.
pub fn latin(family: &str) -> Vec<u8> {
    let mut g = vec![G { ch: ' ', advance: 300, height: 0 }];
    for (i, ch) in ('!'..='~').enumerate() {
        let advance = if ch.is_ascii_alphanumeric() { 600 } else { 400 };
        g.push(G { ch, advance, height: 300 + (i as i16 % 40) * 10 });
    }
    // A few accented letters and punctuation the tests use.
    for (i, ch) in "áéíóúñçüÁÉ—…".chars().enumerate() {
        g.push(G { ch, advance: 600, height: 700 + i as i16 * 5 });
    }
    build(family, &g)
}

/// Hiragana, katakana and some ideographs, all full-width (1000), plus CJK punctuation.
pub fn cjk(family: &str) -> Vec<u8> {
    let mut g = Vec::new();
    let text = "あいうえおかきくけこさしすせそたちつてとなにはひふへほまみむめもやゆよらりるれろわをんアイウエオカキクケコ日本語中文字漢字語言你好世界。、「」，．";
    for (i, ch) in text.chars().enumerate() {
        g.push(G { ch, advance: 1000, height: 500 + (i as i16 % 30) * 10 });
    }
    g.push(G { ch: '\u{3000}', advance: 1000, height: 0 }); // ideographic space
    build(family, &g)
}

/// The Hebrew letters alef to tav, advance 500 + 20 * index, height 400 + 15 * index.
pub fn hebrew(family: &str) -> Vec<u8> {
    let mut g = Vec::new();
    for (i, ch) in ('\u{05D0}'..='\u{05EA}').enumerate() {
        g.push(G { ch, advance: 500 + i as u16 * 20, height: 400 + i as i16 * 15 });
    }
    g.push(G { ch: ' ', advance: 250, height: 0 });
    build(family, &g)
}

/// The basic Arabic letters, no joining table.
pub fn arabic(family: &str) -> Vec<u8> {
    let mut g = Vec::new();
    for (i, ch) in ('\u{0621}'..='\u{064A}').enumerate() {
        g.push(G { ch, advance: 450 + i as u16 * 10, height: 350 + i as i16 * 8 });
    }
    g.push(G { ch: ' ', advance: 250, height: 0 });
    build(family, &g)
}
