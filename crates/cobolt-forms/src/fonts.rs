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
            // Start from the base set (with the Latin + CJK fallbacks) so the
            // on-demand `set_fonts` in `font_id` never drops them.
            defs: base_font_definitions(),
            state: HashMap::new(),
        })
    })
}

/// Whether egui/epaint (>=0.34: skrifa + vello_cpu) can safely rasterise this
/// face: it must parse with skrifa AND have a non-zero `units_per_em` —
/// epaint divides by it to compute `px_scale_factor` and panics with
/// "Bad px_scale_factor: inf" on degenerate (e.g. bitmap-only) faces.
fn egui_can_rasterize(bytes: &[u8], index: u32) -> bool {
    use skrifa::raw::TableProvider;
    let Ok(font) = skrifa::FontRef::from_index(bytes, index) else {
        return false;
    };
    font.head().map(|h| h.units_per_em() > 0).unwrap_or(false)
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
    if !egui_can_rasterize(&bytes, 0) {
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

/// Load a CJK-capable system font (bytes + face index within a collection),
/// validated against egui's rasteriser. Tries families that ship with macOS,
/// Windows and common Linux distros so Japanese (日本語) / Chinese (中文) glyphs
/// render instead of showing as tofu boxes.
fn cjk_font() -> Option<(Vec<u8>, u32)> {
    for fam in [
        "Arial Unicode MS",
        "Hiragino Sans",
        "Hiragino Kaku Gothic ProN",
        "Hiragino Kaku Gothic Pro",
        "PingFang SC",
        "Hiragino Sans GB",
        "Heiti SC",
        "YuGothic",
        "Yu Gothic",
        "Meiryo",
        "MS Gothic",
        "Microsoft YaHei",
        "SimSun",
        "Noto Sans CJK JP",
        "Noto Sans CJK SC",
        "Noto Sans JP",
        "Noto Sans SC",
    ] {
        let q = fontdb::Query {
            families: &[fontdb::Family::Name(fam)],
            weight: fontdb::Weight::NORMAL,
            stretch: fontdb::Stretch::Normal,
            style: fontdb::Style::Normal,
        };
        let Some(id) = db().query(&q) else { continue };
        let Some((bytes, idx)) =
            db().with_face_data(id, |data, face_index| (data.to_vec(), face_index))
        else {
            continue;
        };
        // Must parse with egui's rasteriser (collections need the right index).
        if egui_can_rasterize(&bytes, idx) {
            return Some((bytes, idx));
        }
    }
    None
}

/// The IDE's base font set: egui's defaults plus broad-Latin and CJK system
/// fonts appended as fallbacks (so the language selector's 日本語 / 中文 and
/// punctuation like the U+2011 non-breaking hyphen render everywhere).
pub fn base_font_definitions() -> egui::FontDefinitions {
    let mut defs = egui::FontDefinitions::default();
    let mut fallbacks: Vec<String> = Vec::new();

    if let Some(bytes) = pdf_font_bytes() {
        defs.font_data.insert(
            "latin_fallback".to_owned(),
            std::sync::Arc::new(egui::FontData::from_owned(bytes)),
        );
        fallbacks.push("latin_fallback".to_owned());
    }
    if let Some((bytes, idx)) = cjk_font() {
        let mut fd = egui::FontData::from_owned(bytes);
        fd.index = idx;
        defs.font_data
            .insert("cjk_fallback".to_owned(), std::sync::Arc::new(fd));
        fallbacks.push("cjk_fallback".to_owned());
    }

    for fam in [egui::FontFamily::Proportional, egui::FontFamily::Monospace] {
        let chain = defs.families.entry(fam).or_default();
        for fb in &fallbacks {
            chain.push(fb.clone());
        }
    }
    defs
}

/// Whether `family` should use egui's built-in proportional font (our Arial-ish
/// fallback) rather than a loaded system font.
fn is_builtin(fam: &str) -> bool {
    fam.is_empty()
        || fam.eq_ignore_ascii_case("Arial")
        || fam.eq_ignore_ascii_case("Helvetica")
        || fam.eq_ignore_ascii_case("sans-serif")
}

/// Is `fam` bound to fonts in THIS context right now?
///
/// Naming a family epaint does not know is not a fallback — it is a **panic**
/// (`FontFamily::Name("Arial Black") is not bound to any fonts`, epaint's
/// `FontsImpl::font`). So the question has to be asked of the context, not of
/// our own cache: `set_fonts` REPLACES a context's definitions wholesale, and
/// anyone may call it, which drops every family loaded here while our cache
/// still says `Ready`. That is exactly what happened — opening the
/// documentation window re-installed the base definitions on the IDE's shared
/// context and the next repaint of an open designer panicked on a control
/// whose Font was "Arial Black" (operator, 2026-08-20).
///
/// `false` when the context has no fonts yet (before the first pass), which
/// keeps this usable outside a running app.
fn is_bound(ctx: &egui::Context, fam: &str) -> bool {
    let target = egui::FontFamily::Name(fam.into());
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        ctx.fonts(|f| f.families().iter().any(|k| *k == target))
    }))
    .unwrap_or(false)
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
    // Asked BEFORE our own lock is taken: `is_bound` writes the egui context,
    // and so does `set_fonts` below — nesting the two would be a deadlock
    // waiting to happen, and there is nothing to gain by holding both.
    let bound = is_bound(ctx, fam);
    let mut g = inner().lock().unwrap();
    let named = || egui::FontId::new(size, egui::FontFamily::Name(fam.into()));

    // Our cache says the family is installed but the context disagrees: the
    // definitions were replaced under us. Forget what we knew and let the
    // `None` arm register it again — one proportional pass, then it is back.
    if !bound && matches!(g.state.get(fam), Some(FontState::Ready)) {
        g.state.remove(fam);
    }

    match g.state.get(fam).copied() {
        Some(FontState::Ready) => named(),
        Some(FontState::Failed) => egui::FontId::proportional(size),
        Some(FontState::Loading(when)) => {
            // `now > when` says the atlas has had a pass to rebuild — but only
            // the context can say the family SURVIVED it (a clobber inside that
            // window would otherwise be promoted straight to a panic).
            if now > when && bound {
                g.state.insert(fam.to_owned(), FontState::Ready);
                named()
            } else {
                // Same pass set_fonts was issued — atlas not rebuilt yet.
                egui::FontId::proportional(size)
            }
        }
        None => {
            // Already loaded once (this is a re-registration after the context's
            // definitions were replaced): the bytes are still in `defs`, so put
            // them back rather than reading the file again. One `set_fonts`
            // restores EVERY family loaded here, not just this one.
            if g.defs.families.contains_key(&egui::FontFamily::Name(fam.into())) {
                let defs = g.defs.clone();
                ctx.set_fonts(defs);
                g.state.insert(fam.to_owned(), FontState::Loading(now));
                return egui::FontId::proportional(size);
            }
            match load_font_bytes(fam) {
                Some(bytes) => {
                    g.defs.font_data.insert(
                        fam.to_owned(),
                        std::sync::Arc::new(egui::FontData::from_owned(bytes)),
                    );
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

    /// `font_id`'s cache is process-global while the binding it hands out is
    /// per-`Context` — a `set_fonts` reaches ONE context. That is fine in an
    /// application (one context per process) and a race between any two tests
    /// that drive it with contexts of their own: each marks the shared state
    /// `Ready` for a family the other's context has never been given. Every
    /// test below that calls `font_id` on a real font takes this first.
    static FONT_TEST_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

    /// Take it without caring that a previous failure poisoned it — a panicking
    /// test must not turn one red into several.
    fn font_test_guard() -> std::sync::MutexGuard<'static, ()> {
        FONT_TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner())
    }

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

    /// **The crash, reproduced: a font family the context lost must never be
    /// named again.** `set_fonts` REPLACES a context's definitions, and epaint
    /// PANICS on a family it cannot find rather than falling back — so any part
    /// of the app installing its own font set silently armed a crash in every
    /// open designer holding a non-builtin Font. Opening the documentation
    /// window did exactly that and the IDE went down on
    /// `FontFamily::Name("Arial Black") is not bound to any fonts`
    /// (operator, 2026-08-20).
    ///
    /// Drives the real sequence: load a family, clobber the context the way the
    /// doc window did, then ask again — and lay the answer out for real, which
    /// is where the panic actually happened.
    #[test]
    fn a_clobbered_font_family_is_re_registered_never_named_unbound() {
        let _serial = font_test_guard();
        let ctx = egui::Context::default();
        // Any installed non-builtin face will do; the mechanism is not
        // particular to one name.
        let Some(fam) = system_fonts()
            .into_iter()
            .find(|f| !is_builtin(f) && load_font_bytes(f).is_some())
        else {
            eprintln!("no loadable non-builtin system font here — nothing to check");
            return;
        };

        // One repaint: resolve the family and lay text out with it INSIDE the
        // same pass, which is what a control caption does (`styled_text_job`
        // asks for the `FontId`, `Painter::layout_job` uses it). Resolving
        // outside a pass would be testing something the app never does —
        // `set_fonts` only takes effect at the START of the next pass, so a
        // resolve made between passes reads a font set the layout will not use.
        let pass = |ctx: &egui::Context| -> egui::FontId {
            let mut got = egui::FontId::proportional(14.0);
            ctx.run_ui(Default::default(), |ui| {
                let id = font_id(ui.ctx(), &fam, 14.0);
                // An unbound family panics HERE, which is the whole point.
                ui.ctx().fonts_mut(|f| {
                    f.layout_no_wrap("Ag".to_owned(), id.clone(), egui::Color32::WHITE)
                });
                got = id;
            })
            .textures_delta
            .clear();
            got
        };

        // Warm up until the family is actually in use (loading spends a pass
        // rebuilding the atlas before it reports Ready).
        let mut named = egui::FontId::proportional(14.0);
        for _ in 0..6 {
            named = pass(&ctx);
        }
        assert_eq!(
            named.family,
            egui::FontFamily::Name(fam.as_str().into()),
            "{fam:?} never became usable, so this test proves nothing"
        );

        // The doc window's move: install a fresh set over the top.
        ctx.set_fonts(base_font_definitions());

        // The repaints that follow. Before the fix the first of these named the
        // lost family and epaint brought the process down.
        let mut recovered = egui::FontId::proportional(14.0);
        for _ in 0..6 {
            recovered = pass(&ctx);
        }

        // …and it recovers rather than falling back to Arial forever.
        assert_eq!(
            recovered.family,
            egui::FontFamily::Name(fam.as_str().into()),
            "{fam:?} must be re-registered after the clobber, not abandoned"
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
        // Whatever we hand to egui must parse with egui's own font stack
        // (skrifa since epaint 0.34), so no accepted face can panic inside
        // set_fonts.
        let mut checked = 0usize;
        for fam in system_fonts() {
            if let Some(bytes) = load_font_bytes(fam) {
                assert!(
                    egui_can_rasterize(&bytes, 0),
                    "load_font_bytes returned a face egui can't rasterise: {fam:?}"
                );
                checked += 1;
            }
        }
        assert!(checked > 0, "no loadable fonts to validate");
        println!("validated {checked} loadable system faces against skrifa");

        // The bitmap-only face from the original bug report used to be
        // rejected because ab_glyph (epaint <=0.33) could not parse it.
        // skrifa CAN parse it, so the guarantee is now exercised end-to-end:
        // if the loader accepts it, installing and laying it out must not
        // panic anywhere in egui.
        if system_fonts().iter().any(|f| f == "GB18030 Bitmap") {
            if let Some(bytes) = load_font_bytes("GB18030 Bitmap") {
                let ctx = egui::Context::default();
                let mut defs = base_font_definitions();
                defs.font_data.insert(
                    "gb18030_bitmap_test".to_owned(),
                    std::sync::Arc::new(egui::FontData::from_owned(bytes)),
                );
                defs.families
                    .entry(egui::FontFamily::Name("gb18030_bitmap_test".into()))
                    .or_default()
                    .push("gb18030_bitmap_test".to_owned());
                ctx.set_fonts(defs);
                ctx.run_ui(Default::default(), |ui| {
                    ui.label(
                        egui::RichText::new("汉字 GB18030 ABC")
                            .family(egui::FontFamily::Name("gb18030_bitmap_test".into())),
                    );
                })
                .textures_delta
                .clear();
                println!("GB18030 Bitmap accepted by skrifa and laid out without panic");
            } else {
                println!("GB18030 Bitmap present but rejected by the loader");
            }
        }
    }

    #[test]
    fn chosen_system_font_loads_and_resolves_to_named_family() {
        let _serial = font_test_guard();
        // Find a real, loadable, non-builtin system font.
        let fam = system_fonts()
            .iter()
            .find(|f| !is_builtin(f) && load_font_bytes(f).is_some())
            .expect("expected at least one loadable system font")
            .clone();

        let ctx = egui::Context::default();
        // Frame 1: first request triggers on-demand load (still falls back this pass).
        ctx.run_ui(Default::default(), |_| {}).textures_delta.clear();
        let first = font_id(&ctx, &fam, 16.0);
        assert_eq!(
            first.family,
            egui::FontFamily::Proportional,
            "first request should fall back while the atlas rebuilds"
        );
        // Frame 2: atlas has been rebuilt, the named family is now usable.
        ctx.run_ui(Default::default(), |_| {}).textures_delta.clear();
        let ready = font_id(&ctx, &fam, 16.0);
        assert_eq!(
            ready.family,
            egui::FontFamily::Name(fam.as_str().into()),
            "loaded font {fam:?} should resolve to its named family"
        );
        assert_eq!(ready.size, 16.0);
    }
}
