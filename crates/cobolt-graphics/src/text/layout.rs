// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Laying a text out: bidirectional runs, font fallback, shaping, UAX #14
//! line breaking, wrapping, and the glyph outlines of each line.
//!
//! The pipeline for one paragraph (text between line feeds):
//!
//! 1. **Bidi** (`unicode-bidi`, UAX #9): an embedding level for every byte.
//! 2. **Itemize**: cut the paragraph into segments of one level, one font and
//!    one script. The font of a character is the first face of the fallback
//!    chain that has a glyph for it; spaces and punctuation stay with the font of
//!    the text before them.
//! 3. **Shape** (`harfrust`, a port of HarfBuzz): each segment in its direction.
//! 4. **Break** (`icu_segmenter`, UAX #14) and **wrap**: greedy, at the break
//!    opportunities, trailing spaces not counted.
//! 5. **Reorder** each line visually (UAX #9 rule L2) and place the glyphs.
//!
//! Everything is arithmetic on font units scaled once by `size / units_per_em`
//! in `f32`, so the result is the same on every platform.

use std::sync::Arc;

use harfrust::{Direction, ShapeOptions, UnicodeBuffer};
use icu_segmenter::options::LineBreakOptions;
use icu_segmenter::LineSegmenter;
use skrifa::MetadataProvider;
use tiny_skia::{PathBuilder, Transform};
use unicode_bidi::{Level, ParagraphBidiInfo};

use super::fonts::Face;
use super::script::resolve_scripts;
use crate::display_list::consts::TextDirection;
use crate::error::{GraphicsError, Result};
use crate::textapi::{TextBlock, TextLine, TextRequest};

/// One positioned glyph, in pixels.
#[derive(Clone, Copy, Debug)]
struct Glyph {
    face: usize,
    gid: u32,
    /// Byte offset of its cluster in the paragraph.
    cluster: usize,
    adv: f32,
    dx: f32,
    dy: f32,
}

/// A run of one level, font and script, shaped.
struct Seg {
    start: usize,
    end: usize,
    level: u8,
    /// In the order the shaper returned them: left to right on screen.
    glyphs: Vec<Glyph>,
}

/// The faces to try, primary first.
pub(crate) struct Chain<'a> {
    pub faces: &'a [Arc<Face>],
}

/// Lay out a whole request.
pub(crate) fn layout(chain: &Chain<'_>, req: &TextRequest<'_>) -> Result<TextBlock> {
    let primary = chain.faces.first().ok_or_else(|| GraphicsError::Font("no font is registered".into()))?;
    let size = req.font.size;
    let (ascent, descent, leading) = primary.vertical_metrics(size);
    let natural = ascent + descent + leading;
    let line_height = req.line_height.filter(|h| h.is_finite() && *h > 0.0).unwrap_or(natural);

    let mut lines: Vec<TextLine> = Vec::new();
    let mut first_rtl: Option<bool> = None;
    for para in req.text.split('\n') {
        let para = para.strip_suffix('\r').unwrap_or(para);
        let (plines, rtl) = layout_paragraph(chain, req, para)?;
        first_rtl.get_or_insert(rtl);
        for glyphs in plines {
            let baseline = lines.len() as f32 * line_height;
            lines.push(build_line(chain, size, &glyphs, baseline, rtl));
        }
    }
    let width = lines.iter().map(|l| l.advance).fold(0.0f32, f32::max);
    Ok(TextBlock { lines, width, ascent, descent, line_height, rtl: first_rtl.unwrap_or(false) })
}

/// Turn positioned glyphs (in visual order) into a line with its outline.
fn build_line(chain: &Chain<'_>, size: f32, glyphs: &[Glyph], baseline: f32, rtl: bool) -> TextLine {
    let mut pb = PathBuilder::new();
    let mut x = 0.0f32;
    let mut ink: Option<(f32, f32, f32, f32)> = None;
    for g in glyphs {
        let face = &chain.faces[g.face];
        let s = size / face.upem;
        if let Some(outline) = face.outline(g.gid) {
            // Font units, y up  ->  pixels, baseline at y = 0, y down.
            let ts = Transform::from_row(s, 0.0, 0.0, -s, x + g.dx, -g.dy);
            if let Some(p) = (*outline).clone().transform(ts) {
                let b = p.bounds();
                ink = Some(match ink {
                    None => (b.left(), b.top(), b.right(), b.bottom()),
                    Some((l, t, r, bt)) => (l.min(b.left()), t.min(b.top()), r.max(b.right()), bt.max(b.bottom())),
                });
                pb.push_path(&p);
            }
        }
        x += g.adv;
    }
    TextLine { path: pb.finish().map(Arc::new), advance: x, baseline, ink, rtl }
}

fn is_ignorable(ch: char) -> bool {
    ch.is_control()
        || matches!(ch, '\u{200B}'..='\u{200F}' | '\u{2028}'..='\u{202E}' | '\u{2060}'..='\u{2064}' | '\u{FE00}'..='\u{FE0F}' | '\u{E0100}'..='\u{E01EF}' | '\u{034F}' | '\u{061C}')
}

/// Lay out one paragraph; returns the lines (glyphs in visual order) and whether the
/// paragraph's base direction is right-to-left.
fn layout_paragraph(chain: &Chain<'_>, req: &TextRequest<'_>, para: &str) -> Result<(Vec<Vec<Glyph>>, bool)> {
    let size = req.font.size;
    if para.is_empty() {
        return Ok((vec![Vec::new()], req.direction == TextDirection::Rtl));
    }
    let level = match req.direction {
        TextDirection::Auto => None,
        TextDirection::Ltr => Some(Level::ltr()),
        TextDirection::Rtl => Some(Level::rtl()),
    };
    let bidi = ParagraphBidiInfo::new(para, level);
    let para_rtl = bidi.paragraph_level.is_rtl();
    let scripts = resolve_scripts(para);

    // ---- itemize ---------------------------------------------------------
    let fonts: Vec<Option<skrifa::FontRef<'_>>> = chain.faces.iter().map(|f| f.font_ref()).collect();
    let maps: Vec<Option<skrifa::charmap::Charmap<'_>>> = fonts.iter().map(|f| f.as_ref().map(|f| f.charmap())).collect();
    let has = |i: usize, ch: char| maps[i].as_ref().map(|m| m.map(ch as u32).is_some()).unwrap_or(false);

    struct Pending {
        start: usize,
        level: u8,
        face: usize,
        script: Option<harfrust::Script>,
    }
    let mut ranges: Vec<(Pending, usize)> = Vec::new(); // (segment, end)
    let mut cur: Option<Pending> = None;
    for (ci, (bi, ch)) in para.char_indices().enumerate() {
        let lvl = bidi.levels[bi].number();
        let info = scripts[ci];
        let neutral = info.neutral || is_ignorable(ch);
        // Which face draws this character.
        let face = {
            let prev = cur.as_ref().map(|c| c.face);
            match prev {
                Some(p) if neutral && (is_ignorable(ch) || has(p, ch)) => p,
                _ => (0..chain.faces.len()).find(|&i| has(i, ch)).unwrap_or_else(|| prev.unwrap_or(0)),
            }
        };
        let split = match &cur {
            None => true,
            Some(c) => {
                c.level != lvl
                    || c.face != face
                    || matches!((c.script, info.script), (Some(a), Some(b)) if a != b && !neutral)
            }
        };
        if split {
            if let Some(c) = cur.take() {
                ranges.push((c, bi));
            }
            cur = Some(Pending { start: bi, level: lvl, face, script: info.script });
        } else if let Some(c) = cur.as_mut() {
            if c.script.is_none() {
                c.script = info.script;
            }
        }
    }
    if let Some(c) = cur.take() {
        ranges.push((c, para.len()));
    }

    // ---- shape -----------------------------------------------------------
    let mut segs: Vec<Seg> = Vec::with_capacity(ranges.len());
    for (p, end) in ranges {
        let face = &chain.faces[p.face];
        let font = fonts[p.face].as_ref().ok_or_else(|| GraphicsError::Font(format!("{}: the font could not be read", face.family)))?;
        let data = face.shaper_data(font);
        let shaper = data.shaper(font).build();
        let mut buf = UnicodeBuffer::new();
        buf.push_str(&para[p.start..end]);
        buf.set_direction(if p.level % 2 == 1 { Direction::RightToLeft } else { Direction::LeftToRight });
        if let Some(s) = p.script {
            buf.set_script(s);
        }
        buf.guess_segment_properties();
        let out = shaper.shape(buf, ShapeOptions::new());
        let scale = size / face.upem;
        let glyphs: Vec<Glyph> = out
            .glyph_infos()
            .iter()
            .zip(out.glyph_positions())
            .map(|(i, pos)| Glyph {
                face: p.face,
                gid: i.glyph_id,
                cluster: p.start + i.cluster as usize,
                adv: pos.x_advance as f32 * scale,
                dx: pos.x_offset as f32 * scale,
                dy: pos.y_offset as f32 * scale,
            })
            .collect();
        segs.push(Seg { start: p.start, end, level: p.level, glyphs });
    }

    // ---- break and wrap --------------------------------------------------
    let ranges: Vec<(usize, usize)> = match req.wrap_width {
        Some(w) if w.is_finite() && w > 0.0 => {
            let opps: Vec<usize> = LineSegmenter::new_for_non_complex_scripts(LineBreakOptions::default()).segment_str(para).collect();
            wrap(para, &segs, &opps, w)
        }
        _ => vec![(0, para.len())],
    };

    // ---- reorder and collect ---------------------------------------------
    let mut out_lines = Vec::with_capacity(ranges.len());
    let last_end = para.len();
    for (a, b) in ranges {
        // At a soft break the spaces that end the line are not part of it.
        let b = if b < last_end { trim_end(para, a, b) } else { b };
        if b <= a {
            out_lines.push(Vec::new());
            continue;
        }
        let (levels, runs) = bidi.visual_runs(a..b);
        let mut glyphs: Vec<Glyph> = Vec::new();
        for run in runs {
            let lvl = levels[run.start].number();
            let mut in_run: Vec<&Seg> = segs.iter().filter(|s| s.end > run.start && s.start < run.end).collect();
            if lvl % 2 == 1 {
                in_run.reverse();
            }
            for s in in_run {
                let _ = s.level;
                glyphs.extend(s.glyphs.iter().copied().filter(|g| g.cluster >= run.start && g.cluster < run.end));
            }
        }
        out_lines.push(glyphs);
    }
    Ok((out_lines, para_rtl))
}

/// The end of `para[a..b]` without trailing white space.
fn trim_end(para: &str, a: usize, b: usize) -> usize {
    let s = &para[a..b];
    a + s.trim_end_matches(char::is_whitespace).len()
}

/// Greedy line breaking at the UAX #14 opportunities.
fn wrap(para: &str, segs: &[Seg], opps: &[usize], max_width: f32) -> Vec<(usize, usize)> {
    // Advance per cluster start, sorted, with prefix sums, so the width of any
    // byte range is two lookups.
    let mut cl: Vec<(usize, f32)> = segs.iter().flat_map(|s| s.glyphs.iter().map(|g| (g.cluster, g.adv))).collect();
    cl.sort_by(|a, b| a.0.cmp(&b.0));
    let mut starts: Vec<usize> = Vec::with_capacity(cl.len());
    let mut prefix: Vec<f32> = Vec::with_capacity(cl.len() + 1);
    prefix.push(0.0);
    let mut acc = 0.0f32;
    for (c, a) in &cl {
        starts.push(*c);
        acc += *a;
        prefix.push(acc);
    }
    let width = |a: usize, b: usize| -> f32 {
        let i = starts.partition_point(|&c| c < a);
        let j = starts.partition_point(|&c| c < b);
        prefix[j] - prefix[i]
    };
    let eps = 1e-3f32;
    let mut lines = Vec::new();
    let mut start = 0usize;
    let mut last_ok: Option<usize> = None;
    let mut i = 0usize;
    while i < opps.len() {
        let b = opps[i];
        if b <= start {
            i += 1;
            continue;
        }
        let e = trim_end(para, start, b);
        if width(start, e) <= max_width + eps {
            last_ok = Some(b);
            i += 1;
        } else if let Some(l) = last_ok.take() {
            lines.push((start, l));
            start = l;
            // Look at the same opportunity again from the new start.
        } else {
            // One unbreakable unit wider than the line: it overflows.
            lines.push((start, b));
            start = b;
            i += 1;
        }
    }
    if start < para.len() {
        lines.push((start, para.len()));
    }
    if lines.is_empty() {
        lines.push((0, para.len()));
    }
    lines
}
