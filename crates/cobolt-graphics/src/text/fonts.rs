// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Font faces and the registry that finds them.

use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};

use skrifa::instance::{LocationRef, Size};
use skrifa::outline::{DrawSettings, OutlinePen};
use skrifa::{FontRef, GlyphId, MetadataProvider};
use tiny_skia::{Path, PathBuilder};

use crate::error::{GraphicsError, Result};

/// One face of a font file.
pub(crate) struct Face {
    pub data: Arc<dyn AsRef<[u8]> + Send + Sync>,
    pub index: u32,
    pub family: String,
    pub upem: f32,
    shaper: OnceLock<harfrust::ShaperData>,
    outlines: Mutex<HashMap<u32, Option<Arc<Path>>>>,
}

struct PathPen(PathBuilder);

impl OutlinePen for PathPen {
    fn move_to(&mut self, x: f32, y: f32) {
        self.0.move_to(x, y);
    }
    fn line_to(&mut self, x: f32, y: f32) {
        self.0.line_to(x, y);
    }
    fn quad_to(&mut self, cx0: f32, cy0: f32, x: f32, y: f32) {
        self.0.quad_to(cx0, cy0, x, y);
    }
    fn curve_to(&mut self, cx0: f32, cy0: f32, cx1: f32, cy1: f32, x: f32, y: f32) {
        self.0.cubic_to(cx0, cy0, cx1, cy1, x, y);
    }
    fn close(&mut self) {
        self.0.close();
    }
}

impl Face {
    pub fn new(data: Arc<dyn AsRef<[u8]> + Send + Sync>, index: u32, family: String) -> Result<Face> {
        let bytes: &[u8] = (*data).as_ref();
        let font = FontRef::from_index(bytes, index).map_err(|e| GraphicsError::Font(format!("{family}: {e}")))?;
        let upem = {
            use skrifa::raw::TableProvider;
            font.head().map(|h| h.units_per_em()).unwrap_or(0)
        };
        if upem == 0 {
            return Err(GraphicsError::Font(format!("{family}: the font has no usable units-per-em (a bitmap-only font?)")));
        }
        // A face must have outlines to be drawn.
        if font.outline_glyphs().get(GlyphId::new(0)).is_none() && font.outline_glyphs().get(GlyphId::new(1)).is_none() {
            return Err(GraphicsError::Font(format!("{family}: the font has no glyph outlines (a bitmap or colour-only font?)")));
        }
        Ok(Face { data, index, family, upem: upem as f32, shaper: OnceLock::new(), outlines: Mutex::new(HashMap::new()) })
    }

    pub fn font_ref(&self) -> Option<FontRef<'_>> {
        FontRef::from_index((*self.data).as_ref(), self.index).ok()
    }

    pub fn shaper_data(&self, font: &FontRef<'_>) -> &harfrust::ShaperData {
        self.shaper.get_or_init(|| harfrust::ShaperData::new(font))
    }

    /// Ascent, descent (both positive) and leading, in pixels at `size`.
    pub fn vertical_metrics(&self, size: f32) -> (f32, f32, f32) {
        match self.font_ref() {
            Some(f) => {
                let m = f.metrics(Size::new(size), LocationRef::default());
                (m.ascent, -m.descent, m.leading.max(0.0))
            }
            None => (size * 0.8, size * 0.2, 0.0),
        }
    }

    /// The glyph's outline in font units, y up. `None` for a glyph without ink.
    pub fn outline(&self, gid: u32) -> Option<Arc<Path>> {
        if let Some(hit) = self.outlines.lock().ok().and_then(|m| m.get(&gid).cloned()) {
            return hit;
        }
        let path = (|| {
            let font = self.font_ref()?;
            let glyph = font.outline_glyphs().get(GlyphId::new(gid))?;
            let mut pen = PathPen(PathBuilder::new());
            glyph.draw(DrawSettings::unhinted(Size::unscaled(), LocationRef::default()), &mut pen).ok()?;
            pen.0.finish().map(Arc::new)
        })();
        if let Ok(mut m) = self.outlines.lock() {
            if m.len() > 20_000 {
                m.clear();
            }
            m.insert(gid, path.clone());
        }
        path
    }
}

/// What a registered face is, for hosts that list fonts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FontInfo {
    /// The family name.
    pub family: String,
    /// 100 to 900.
    pub weight: u16,
    /// Whether the face is italic or oblique.
    pub italic: bool,
    /// The index inside its file (collections hold several).
    pub index: u32,
}
