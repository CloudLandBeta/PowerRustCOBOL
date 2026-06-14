// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! System font enumeration + on-demand loading into egui.
//!
//! - `system_fonts()` lists installed font families (for the Font dropdown).
//! - `font_id()` resolves a family+size to an `egui::FontId`, loading the system
//!   font into egui the first time it's used, and falling back to the built-in
//!   proportional font (the "Arial" stand-in) when the family is Arial/default or
//!   can't be loaded.

use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

/// Shared font database (system fonts scanned once).
fn db() -> &'static fontdb::Database {
    static DB: OnceLock<fontdb::Database> = OnceLock::new();
    DB.get_or_init(|| {
        let mut d = fontdb::Database::new();
        d.load_system_fonts();
        d
    })
}

/// Sorted, de-duplicated installed font families, with "Arial" guaranteed
/// present (the fallback font). Enumerated once, then cached.
pub fn system_fonts() -> &'static [String] {
    static FONTS: OnceLock<Vec<String>> = OnceLock::new();
    FONTS.get_or_init(|| {
        let mut names: Vec<String> = db()
            .faces()
            .filter_map(|f| f.families.first().map(|(name, _lang)| name.clone()))
            // Skip OS-internal/hidden families (their names start with '.').
            .filter(|name| !name.starts_with('.') && !name.trim().is_empty())
            .collect();
        names.sort_by_key(|n| n.to_lowercase());
        names.dedup();
        if !names.iter().any(|n| n.eq_ignore_ascii_case("Arial")) {
            names.insert(0, "Arial".to_owned());
        }
        names
    })
}

#[derive(Clone, Copy)]
enum FontState {
    /// `set_fonts` was issued on this pass number; usable once a later pass runs.
    Loading(u64),
    Ready,
    Failed,
}

struct Inner {
    defs: egui::FontDefinitions,
    state: HashMap<String, FontState>,
}

fn inner() -> &'static Mutex<Inner> {
    static I: OnceLock<Mutex<Inner>> = OnceLock::new();
    I.get_or_init(|| {
        Mutex::new(Inner {
            defs: egui::FontDefinitions::default(),
            state: HashMap::new(),
        })
    })
}

fn load_font_bytes(family: &str) -> Option<Vec<u8>> {
    let q = fontdb::Query {
        families: &[fontdb::Family::Name(family)],
        weight: fontdb::Weight::NORMAL,
        stretch: fontdb::Stretch::Normal,
        style: fontdb::Style::Normal,
    };
    let id = db().query(&q)?;
    let bytes = db().with_face_data(id, |data, _idx| data.to_vec())?;
    // Reject faces egui's rasteriser can't parse (e.g. bitmap-only fonts such as
    // "GB18030 Bitmap"), which would otherwise panic inside `set_fonts`.
    if ab_glyph::FontRef::try_from_slice(&bytes).is_err() {
        return None;
    }
    Some(bytes)
}

/// TTF bytes for a common sans-serif system font, for embedding into a PDF
/// (used by the Documentation viewer's Print → PDF). Tries a few widely-present
/// families and returns the first that parses.
pub fn pdf_font_bytes() -> Option<Vec<u8>> {
    for fam in [
        "DejaVu Sans",
        "Liberation Sans",
        "Arial",
        "Helvetica Neue",
        "Verdana",
        "Tahoma",
    ] {
        if let Some(b) = load_font_bytes(fam) {
            return Some(b);
        }
    }
    None
}

/// Whether `family` should use egui's built-in proportional font (our Arial-ish
/// fallback) rather than a loaded system font.
fn is_builtin(fam: &str) -> bool {
    fam.is_empty()
        || fam.eq_ignore_ascii_case("Arial")
        || fam.eq_ignore_ascii_case("Helvetica")
        || fam.eq_ignore_ascii_case("sans-serif")
}

/// Resolve a `FontId` for `family` at `size`, loading the system font on demand.
/// Falls back to the built-in proportional font for Arial/default or if the font
/// can't be loaded (i.e. "fall back to Arial when the font isn't available").
pub fn font_id(ctx: &egui::Context, family: &str, size: f32) -> egui::FontId {
    let size = size.max(1.0);
    let fam = family.trim();
    if is_builtin(fam) {
        return egui::FontId::proportional(size);
    }

    let now = ctx.cumulative_pass_nr();
    let mut g = inner().lock().unwrap();
    let named = || egui::FontId::new(size, egui::FontFamily::Name(fam.into()));

    match g.state.get(fam).copied() {
        Some(FontState::Ready) => named(),
        Some(FontState::Failed) => egui::FontId::proportional(size),
        Some(FontState::Loading(when)) => {
            if now > when {
                g.state.insert(fam.to_owned(), FontState::Ready);
                named()
            } else {
                // Same pass set_fonts was issued — atlas not rebuilt yet.
                egui::FontId::proportional(size)
            }
        }
        None => {
            match load_font_bytes(fam) {
                Some(bytes) => {
                    g.defs.font_data.insert(fam.to_owned(), egui::FontData::from_owned(bytes));
                    // Chain egui's default proportional fonts after this face so any
                    // glyphs it lacks still render (instead of showing tofu).
                    let mut chain = vec![fam.to_owned()];
                    if let Some(defaults) = g.defs.families.get(&egui::FontFamily::Proportional) {
                        chain.extend(defaults.iter().cloned());
                    }
                    g.defs
                        .families
                        .insert(egui::FontFamily::Name(fam.into()), chain);
                    let defs = g.defs.clone();
                    ctx.set_fonts(defs);
                    g.state.insert(fam.to_owned(), FontState::Loading(now));
                }
                None => {
                    g.state.insert(fam.to_owned(), FontState::Failed);
                }
            }
            egui::FontId::proportional(size)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn enumerates_system_fonts_with_arial() {
        let fonts = system_fonts();
        assert!(!fonts.is_empty(), "no system fonts enumerated");
        assert!(
            fonts.iter().any(|f| f.eq_ignore_ascii_case("Arial")),
            "Arial fallback missing from list"
        );
        eprintln!(
            "enumerated {} font families (e.g. {:?})",
            fonts.len(),
            &fonts[..fonts.len().min(8)]
        );
    }

    #[test]
    fn builtin_families_use_proportional_fallback() {
        let ctx = egui::Context::default();
        for fam in ["", "Arial", "arial", "Helvetica", "sans-serif"] {
            let id = font_id(&ctx, fam, 18.0);
            assert_eq!(id.family, egui::FontFamily::Proportional, "{fam:?}");
            assert_eq!(id.size, 18.0);
        }
    }

    #[test]
    fn load_font_bytes_only_returns_egui_parseable_faces() {
        // Whatever we hand to egui must parse with egui's own rasteriser, so a
        // bitmap-only face (e.g. "GB18030 Bitmap") never panics inside set_fonts.
        let mut checked = 0usize;
        for fam in system_fonts() {
            if let Some(bytes) = load_font_bytes(fam) {
                assert!(
                    ab_glyph::FontRef::try_from_slice(&bytes).is_ok(),
                    "load_font_bytes returned a face egui can't parse: {fam:?}"
                );
                checked += 1;
            }
        }
        assert!(checked > 0, "no loadable fonts to validate");

        // The specific bitmap font from the bug report, if present, must be rejected.
        if system_fonts().iter().any(|f| f == "GB18030 Bitmap") {
            assert!(load_font_bytes("GB18030 Bitmap").is_none());
        }
    }

    #[test]
    fn chosen_system_font_loads_and_resolves_to_named_family() {
        // Find a real, loadable, non-builtin system font.
        let fam = system_fonts()
            .iter()
            .find(|f| !is_builtin(f) && load_font_bytes(f).is_some())
            .expect("expected at least one loadable system font")
            .clone();

        let ctx = egui::Context::default();
        // Frame 1: first request triggers on-demand load (still falls back this pass).
        let _ = ctx.run(Default::default(), |_| {});
        let first = font_id(&ctx, &fam, 16.0);
        assert_eq!(
            first.family,
            egui::FontFamily::Proportional,
            "first request should fall back while the atlas rebuilds"
        );
        // Frame 2: atlas has been rebuilt, the named family is now usable.
        let _ = ctx.run(Default::default(), |_| {});
        let ready = font_id(&ctx, &fam, 16.0);
        assert_eq!(
            ready.family,
            egui::FontFamily::Name(fam.as_str().into()),
            "loaded font {fam:?} should resolve to its named family"
        );
        assert_eq!(ready.size, 16.0);
    }
}
