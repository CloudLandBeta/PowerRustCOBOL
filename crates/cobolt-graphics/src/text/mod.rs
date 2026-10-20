// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Text (WASM-018 R8): fonts, shaping, bidirectional text and wrapping.
//!
//! A [`FontSet`] is the registry of the fonts a canvas may use and the
//! [`TextEngine`] the canvas draws and measures with. The crate has **no
//! built-in fonts and reads no files**: the host adds the font bytes it has - the
//! project's `Assets/`, a bundled base set, the operating system's fonts on the
//! desktop - with [`FontSet::add_font_data`].
//!
//! * **Shaping** is `harfrust` (a Rust port of HarfBuzz): ligatures, kerning,
//!   Arabic joining, Indic reordering and so on come from the font's own tables.
//! * **Outlines** come from `skrifa`; glyphs are drawn as paths, unhinted and
//!   positioned at sub-pixel precision, so text scales and rotates like any
//!   other shape.
//! * **Direction** follows UAX #9 (`unicode-bidi`): Latin, Japanese and Chinese
//!   run left to right, Arabic and Hebrew right to left, mixed text is reordered
//!   per line.
//! * **Wrapping** breaks at the UAX #14 opportunities (`icu_segmenter`); Japanese
//!   and Chinese may break between any two ideographs, with the usual
//!   prohibition on starting a line with closing punctuation.
//! * **Fallback**: a character missing from the first font of the CSS font
//!   family list is taken from the next one, then from the other registered
//!   fonts in the order they were added; so a Latin font plus one CJK font draws
//!   mixed text.
//!
//! `MeasureText` and `DrawText` call [`TextEngine::layout`] with the same request
//! and read the same [`TextBlock`], so what is measured is exactly what is
//! drawn. Results are cached per (text, font, direction, width).
//!
//! Limits of this version: variable fonts use their default instance; colour
//! glyph tables (emoji in COLR/CBDT/sbix) are not drawn - only outlines are;
//! vertical writing modes are not supported.

mod fonts;
mod layout;
mod script;

use std::collections::HashMap;
use std::sync::{Arc, Mutex, RwLock};

pub use fonts::FontInfo;

use crate::display_list::consts::TextDirection;
use crate::error::{GraphicsError, Result};
use crate::limits::Limits;
use crate::textapi::{FontSpec, FontStyle, TextBlock, TextEngine, TextMetrics, TextRequest};
use fonts::Face;

/// The generic CSS families a font stack may end with.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Generic {
    /// `serif`
    Serif,
    /// `sans-serif`
    SansSerif,
    /// `monospace`
    Monospace,
    /// `cursive`
    Cursive,
    /// `fantasy`
    Fantasy,
}

struct Inner {
    db: fontdb::Database,
    faces: HashMap<fontdb::ID, Arc<Face>>,
    /// Registration order, for the fallback chain.
    order: Vec<fontdb::ID>,
    /// Lower-case family name -> the name as registered.
    names: HashMap<String, String>,
    generation: u64,
    fallback: Vec<String>,
}

#[derive(Hash, PartialEq, Eq)]
struct CacheKey {
    text: Box<str>,
    families: Vec<String>,
    size: u32,
    weight: u16,
    style: FontStyle,
    stretch: u16,
    direction: u8,
    wrap: Option<u32>,
    line_height: Option<u32>,
    generation: u64,
}

/// The fonts of a canvas and the text engine over them.
pub struct FontSet {
    inner: RwLock<Inner>,
    cache: Mutex<HashMap<CacheKey, Arc<TextBlock>>>,
    limits: Limits,
}

impl Default for FontSet {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Debug for FontSet {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "FontSet({} face(s))", self.face_count())
    }
}

fn stretch_for(percent: u16) -> fontdb::Stretch {
    match percent {
        0..=56 => fontdb::Stretch::UltraCondensed,
        57..=68 => fontdb::Stretch::ExtraCondensed,
        69..=81 => fontdb::Stretch::Condensed,
        82..=93 => fontdb::Stretch::SemiCondensed,
        94..=106 => fontdb::Stretch::Normal,
        107..=118 => fontdb::Stretch::SemiExpanded,
        119..=137 => fontdb::Stretch::Expanded,
        138..=175 => fontdb::Stretch::ExtraExpanded,
        _ => fontdb::Stretch::UltraExpanded,
    }
}

impl FontSet {
    /// An empty set with the default [`Limits`].
    pub fn new() -> Self {
        Self::with_limits(Limits::default())
    }

    /// An empty set with explicit limits (the font file size is checked).
    pub fn with_limits(limits: Limits) -> Self {
        Self {
            inner: RwLock::new(Inner {
                db: fontdb::Database::new(),
                faces: HashMap::new(),
                order: Vec::new(),
                names: HashMap::new(),
                generation: 0,
                fallback: Vec::new(),
            }),
            cache: Mutex::new(HashMap::new()),
            limits,
        }
    }

    /// Register every face of a font file (`.ttf`, `.otf`, `.ttc`).
    ///
    /// Fonts without outlines (bitmap-only) are refused. Returns what was
    /// registered.
    pub fn add_font_data(&self, data: Vec<u8>) -> Result<Vec<FontInfo>> {
        self.limits.check_font_bytes(data.len())?;
        let mut inner = self.inner.write().map_err(|_| GraphicsError::Font("the font registry is poisoned".into()))?;
        let ids = inner.db.load_font_source(fontdb::Source::Binary(Arc::new(data)));
        if ids.is_empty() {
            return Err(GraphicsError::Font("the data is not a font file".into()));
        }
        let mut added = Vec::new();
        let mut last_err: Option<GraphicsError> = None;
        for id in ids {
            let Some(info) = inner.db.face(id).cloned() else { continue };
            let family = info.families.first().map(|f| f.0.clone()).unwrap_or_else(|| info.post_script_name.clone());
            // With fontdb's `fs` feature on (another crate may enable it) a source can also be a
            // file path; this registry only ever creates Binary sources.
            #[allow(irrefutable_let_patterns)]
            let fontdb::Source::Binary(arc) = &info.source else { continue };
            match Face::new(arc.clone(), info.index, family.clone()) {
                Ok(face) => {
                    for (name, _) in &info.families {
                        inner.names.entry(name.to_ascii_lowercase()).or_insert_with(|| name.clone());
                    }
                    inner.faces.insert(id, Arc::new(face));
                    inner.order.push(id);
                    added.push(FontInfo {
                        family,
                        weight: info.weight.0,
                        italic: !matches!(info.style, fontdb::Style::Normal),
                        index: info.index,
                    });
                }
                Err(e) => {
                    inner.db.remove_face(id);
                    last_err = Some(e);
                }
            }
        }
        if added.is_empty() {
            return Err(last_err.unwrap_or_else(|| GraphicsError::Font("the file holds no usable face".into())));
        }
        inner.generation += 1;
        drop(inner);
        if let Ok(mut c) = self.cache.lock() {
            c.clear();
        }
        Ok(added)
    }

    /// How many faces are registered.
    pub fn face_count(&self) -> usize {
        self.inner.read().map(|i| i.faces.len()).unwrap_or(0)
    }

    /// The registered family names, sorted.
    pub fn families(&self) -> Vec<String> {
        let Ok(inner) = self.inner.read() else { return Vec::new() };
        let mut v: Vec<String> = inner.names.values().cloned().collect();
        v.sort();
        v.dedup();
        v
    }

    /// Say which registered family a generic CSS family (`serif`, `sans-serif`,
    /// ...) means. Until set, a generic family resolves to the first registered
    /// font.
    pub fn set_generic(&self, generic: Generic, family: &str) {
        if let Ok(mut inner) = self.inner.write() {
            let canonical = inner.names.get(&family.to_ascii_lowercase()).cloned().unwrap_or_else(|| family.to_string());
            match generic {
                Generic::Serif => inner.db.set_serif_family(canonical),
                Generic::SansSerif => inner.db.set_sans_serif_family(canonical),
                Generic::Monospace => inner.db.set_monospace_family(canonical),
                Generic::Cursive => inner.db.set_cursive_family(canonical),
                Generic::Fantasy => inner.db.set_fantasy_family(canonical),
            }
            inner.generation += 1;
        }
        if let Ok(mut c) = self.cache.lock() {
            c.clear();
        }
    }

    /// Families tried, in this order, for characters the requested families lack
    /// (before the remaining fonts in registration order).
    pub fn set_fallback_families(&self, families: &[&str]) {
        if let Ok(mut inner) = self.inner.write() {
            inner.fallback = families.iter().map(|s| s.to_string()).collect();
            inner.generation += 1;
        }
        if let Ok(mut c) = self.cache.lock() {
            c.clear();
        }
    }

    fn resolve_chain(inner: &Inner, spec: &FontSpec) -> Vec<Arc<Face>> {
        let mut out: Vec<Arc<Face>> = Vec::new();
        let push = |out: &mut Vec<Arc<Face>>, id: fontdb::ID| {
            if let Some(f) = inner.faces.get(&id) {
                if !out.iter().any(|x| Arc::ptr_eq(x, f)) {
                    out.push(f.clone());
                }
            }
        };
        let style = match spec.style {
            FontStyle::Normal => fontdb::Style::Normal,
            FontStyle::Italic => fontdb::Style::Italic,
            FontStyle::Oblique => fontdb::Style::Oblique,
        };
        let weight = fontdb::Weight(spec.weight);
        let stretch = stretch_for(spec.stretch);
        let query = |name: &str| -> Option<fontdb::ID> {
            let lower = name.to_ascii_lowercase();
            let family = match lower.as_str() {
                "serif" => fontdb::Family::Serif,
                "sans-serif" | "system-ui" | "ui-sans-serif" => fontdb::Family::SansSerif,
                "monospace" | "ui-monospace" => fontdb::Family::Monospace,
                "cursive" => fontdb::Family::Cursive,
                "fantasy" => fontdb::Family::Fantasy,
                _ => fontdb::Family::Name(inner.names.get(&lower).map(String::as_str).unwrap_or(name)),
            };
            inner.db.query(&fontdb::Query { families: &[family], weight, stretch, style })
        };
        for fam in &spec.families {
            if let Some(id) = query(fam) {
                push(&mut out, id);
            }
        }
        for fam in &inner.fallback {
            if let Some(id) = query(fam) {
                push(&mut out, id);
            }
        }
        for id in &inner.order {
            push(&mut out, *id);
        }
        out
    }

    /// The width, ascent and descent of a one-line text, left aligned on the
    /// alphabetic baseline - the numbers `MeasureText` reports.
    pub fn measure(&self, text: &str, font: &FontSpec) -> Result<TextMetrics> {
        let cleaned: String = text.chars().map(|c| if matches!(c, '\n' | '\r' | '\t') { ' ' } else { c }).collect();
        let block = self.layout(&TextRequest { text: &cleaned, font, direction: TextDirection::Auto, wrap_width: None, line_height: None })?;
        Ok(crate::textapi::metrics_for(&block, crate::display_list::consts::TextAlign::Left, 0.0))
    }
}

impl TextEngine for FontSet {
    fn layout(&self, req: &TextRequest<'_>) -> Result<TextBlock> {
        self.limits.check_text(req.text.len())?;
        let (chain, generation) = {
            let inner = self.inner.read().map_err(|_| GraphicsError::Font("the font registry is poisoned".into()))?;
            (Self::resolve_chain(&inner, req.font), inner.generation)
        };
        let cacheable = req.text.len() <= 4096;
        let key = cacheable.then(|| CacheKey {
            text: req.text.into(),
            families: req.font.families.clone(),
            size: req.font.size.to_bits(),
            weight: req.font.weight,
            style: req.font.style,
            stretch: req.font.stretch,
            direction: req.direction.to_u8(),
            wrap: req.wrap_width.map(f32::to_bits),
            line_height: req.line_height.map(f32::to_bits),
            generation,
        });
        if let Some(k) = &key {
            if let Some(hit) = self.cache.lock().ok().and_then(|c| c.get(k).cloned()) {
                return Ok((*hit).clone());
            }
        }
        let block = layout::layout(&layout::Chain { faces: &chain }, req)?;
        if let Some(k) = key {
            if let Ok(mut c) = self.cache.lock() {
                if c.len() >= 512 {
                    c.clear();
                }
                c.insert(k, Arc::new(block.clone()));
            }
        }
        Ok(block)
    }
}
