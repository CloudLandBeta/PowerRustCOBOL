// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Form theme catalog + resolution (spec 007 — Form themes).
//!
//! A *theme* gives the developer's forms a selectable look. There are two kinds
//! ([`ThemeKind`]): the built-in **procedural** Liquid Glass (the default,
//! unchanged) and **asset-pack** skins discovered from `assets/themes/<id>/`.
//!
//! The model stores only an id: a per-form override ([`crate::Form::theme`]) and
//! a project default. The effective theme is resolved by [`resolve_theme_id`]:
//! `form ?? project ?? liquid-glass`. The renderer reads the resolved theme; a
//! `None`/empty selection always means Liquid Glass, so existing forms are
//! pixel-identical (R9).
//!
//! Asset-pack discovery and the 9-slice/chart-style data live in
//! [`ThemePack`] (populated in later phases). This module's catalog always
//! contains at least the built-in Liquid Glass entry (R1).

/// The stable id of the built-in default theme.
pub const LIQUID_GLASS: &str = "liquid-glass";

/// The stable id of the built-in **Elegance** theme (spec 047).
///
/// A second procedural look alongside Liquid Glass: flat slate surfaces with a
/// cool accent family, drawn entirely in code (no asset pack). Selected exactly
/// like any other catalog entry.
///
/// **Naming:** "Elegance" is the only user-facing name. The third-party crate
/// whose palette it follows is an implementation detail and must never appear
/// in UI text, docs, or generated COBOL (spec 047 R9).
pub const ELEGANCE: &str = "elegance";

/// Whether a theme is drawn procedurally or composited from an asset pack.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThemeKind {
    /// Procedural Liquid Glass — drawn entirely in code (the default look).
    Procedural,
    /// Asset-pack skin — 9-slice imagery from `assets/themes/<id>/`.
    AssetPack,
}

/// One entry in the theme catalog (one selectable look).
#[derive(Debug, Clone)]
pub struct FormTheme {
    /// Stable id stored in the model (e.g. `"liquid-glass"`, `"stainless-steel"`).
    pub id: String,
    /// Human-readable label shown in the picker (a product term, not localised).
    pub display_name: String,
    /// Procedural vs asset-pack.
    pub kind: ThemeKind,
    /// The loaded asset pack for [`ThemeKind::AssetPack`] themes; `None` for the
    /// procedural Liquid Glass. Populated by pack discovery (Phase 3).
    pub pack: Option<crate::theme_pack::ThemePack>,
}

impl FormTheme {
    /// The built-in procedural Liquid Glass theme (R1, R9).
    pub fn liquid_glass() -> Self {
        FormTheme {
            id: LIQUID_GLASS.to_owned(),
            display_name: "Liquid Glass".to_owned(),
            kind: ThemeKind::Procedural,
            pack: None,
        }
    }

    /// The built-in procedural **Elegance** theme (spec 047 R1).
    pub fn elegance() -> Self {
        FormTheme {
            id: ELEGANCE.to_owned(),
            display_name: "Elegance".to_owned(),
            kind: ThemeKind::Procedural,
            pack: None,
        }
    }
}

/// The extensible catalog of selectable themes (R1, R2).
#[derive(Debug, Clone)]
pub struct ThemeCatalog {
    themes: Vec<FormTheme>,
}

impl ThemeCatalog {
    /// A catalog containing the built-in **procedural** themes — Liquid Glass
    /// (the default) then Elegance. Asset packs are added by discovery via
    /// [`ThemeCatalog::with_packs`].
    ///
    /// Order is load-bearing: Liquid Glass must stay first, because
    /// [`ThemeCatalog::resolve`] falls back to `themes[0]` and the pickers show
    /// this order.
    pub fn builtin() -> Self {
        ThemeCatalog {
            themes: vec![FormTheme::liquid_glass(), FormTheme::elegance()],
        }
    }

    /// The ids of the built-in procedural themes — i.e. every catalog entry that
    /// is drawn in code and has **no** asset pack on disk.
    ///
    /// Callers that map a theme id to a pack (the compiler's build-time staging,
    /// the IDE's pack lookup) use this to skip the procedural ids instead of
    /// hunting for an `assets/themes/<id>/` folder that will never exist and
    /// warning when it is missing (spec 047 R-3).
    pub fn procedural_ids() -> &'static [&'static str] {
        &[LIQUID_GLASS, ELEGANCE]
    }

    /// Append discovered asset-pack themes to the built-in catalog, skipping any
    /// whose id collides with an existing entry (built-in wins).
    pub fn with_packs(mut self, packs: impl IntoIterator<Item = FormTheme>) -> Self {
        for t in packs {
            if !self.themes.iter().any(|e| e.id == t.id) {
                self.themes.push(t);
            }
        }
        self
    }

    /// Build the full catalog: built-in Liquid Glass first, then every asset
    /// pack discovered under `themes_dir` (R1, R2, AC7). Drop-in packs appear
    /// automatically with no code change.
    #[cfg(feature = "render")]
    pub fn discover(themes_dir: &std::path::Path) -> Self {
        let packs = crate::theme_pack::discover_packs(themes_dir)
            .into_iter()
            .map(|pack| FormTheme {
                id: pack.id.clone(),
                display_name: pack.display_name.clone(),
                kind: ThemeKind::AssetPack,
                pack: Some(pack),
            });
        ThemeCatalog::builtin().with_packs(packs)
    }

    /// All themes in catalog order (Liquid Glass first).
    pub fn themes(&self) -> &[FormTheme] {
        &self.themes
    }

    /// Catalog ids in order (Liquid Glass first).
    pub fn ids(&self) -> Vec<&str> {
        self.themes.iter().map(|t| t.id.as_str()).collect()
    }

    /// Look up a theme by id.
    pub fn get(&self, id: &str) -> Option<&FormTheme> {
        self.themes.iter().find(|t| t.id == id)
    }

    /// Resolve a (possibly absent) selection to a concrete theme, falling back to
    /// Liquid Glass when the id is empty/unknown (R9, R11).
    pub fn resolve<'a>(
        &'a self,
        form: Option<&str>,
        project_default: Option<&str>,
    ) -> &'a FormTheme {
        let id = resolve_theme_id(form, project_default);
        self.get(&id)
            .or_else(|| self.get(LIQUID_GLASS))
            .expect("catalog always contains liquid-glass")
    }
}

impl Default for ThemeCatalog {
    fn default() -> Self {
        ThemeCatalog::builtin()
    }
}

/// Resolve the effective theme id: per-form override, else project default, else
/// `liquid-glass`. Empty/whitespace strings are treated as unset (R3, R9).
pub fn resolve_theme_id(form: Option<&str>, project_default: Option<&str>) -> String {
    fn nonempty(s: Option<&str>) -> Option<&str> {
        s.map(str::trim).filter(|s| !s.is_empty())
    }
    nonempty(form)
        .or_else(|| nonempty(project_default))
        .unwrap_or(LIQUID_GLASS)
        .to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn catalog_contains_liquid_glass_first() {
        let cat = ThemeCatalog::builtin();
        assert_eq!(cat.ids().first().copied(), Some(LIQUID_GLASS));
        assert!(cat.get(LIQUID_GLASS).is_some());
        assert_eq!(cat.get(LIQUID_GLASS).unwrap().kind, ThemeKind::Procedural);
    }

    /// Spec 047 T1 — the catalog ships **two** procedural themes, in order.
    #[test]
    fn elegance_catalog_lists_both_builtin_themes() {
        let cat = ThemeCatalog::builtin();

        // Report what the catalog actually contains, by name.
        let listing: Vec<String> = cat
            .themes()
            .iter()
            .map(|t| format!("{} ({:?}, pack={})", t.id, t.kind, t.pack.is_some()))
            .collect();
        println!(
            "built-in catalog: {} entr{}\n  {}",
            listing.len(),
            if listing.len() == 1 { "y" } else { "ies" },
            listing.join("\n  ")
        );

        assert_eq!(
            cat.ids(),
            vec![LIQUID_GLASS, ELEGANCE],
            "Liquid Glass must stay first (resolve falls back to themes[0])"
        );

        let eleg = cat.get(ELEGANCE).expect("elegance is in the catalog");
        assert_eq!(eleg.display_name, "Elegance");
        assert_eq!(eleg.kind, ThemeKind::Procedural);
        assert!(eleg.pack.is_none(), "Elegance is drawn in code, not a pack");

        // R9 — the crate behind the palette never surfaces as a name.
        for t in cat.themes() {
            let lower = t.display_name.to_lowercase();
            assert!(
                !lower.contains("egui"),
                "display name must not leak the crate name: {}",
                t.display_name
            );
        }
    }

    /// Both built-ins are reported as procedural, so pack lookups skip them.
    #[test]
    fn elegance_catalog_procedural_ids_cover_both_builtins() {
        let ids = ThemeCatalog::procedural_ids();
        println!("procedural ids (never looked up as packs): {ids:?}");
        assert_eq!(ids, [LIQUID_GLASS, ELEGANCE]);
        for t in ThemeCatalog::builtin().themes() {
            assert!(
                ids.contains(&t.id.as_str()),
                "built-in {} must be listed as procedural",
                t.id
            );
        }
    }

    /// Selecting Elegance resolves to Elegance — the existing precedence rules
    /// are untouched, it is just another id they can carry.
    #[test]
    fn elegance_is_selectable_through_normal_resolution() {
        let cat = ThemeCatalog::builtin();
        assert_eq!(cat.resolve(Some(ELEGANCE), None).id, ELEGANCE);
        assert_eq!(cat.resolve(None, Some(ELEGANCE)).id, ELEGANCE);
        // per-form override still beats the project default
        assert_eq!(cat.resolve(Some(LIQUID_GLASS), Some(ELEGANCE)).id, LIQUID_GLASS);
        assert_eq!(resolve_theme_id(Some(ELEGANCE), None), ELEGANCE);
    }

    #[test]
    fn resolution_precedence_form_then_project_then_glass() {
        // form override wins
        assert_eq!(
            resolve_theme_id(Some("dark-wood"), Some("stainless-steel")),
            "dark-wood"
        );
        // project default when no form override
        assert_eq!(
            resolve_theme_id(None, Some("stainless-steel")),
            "stainless-steel"
        );
        // empty strings are unset → fall through to glass
        assert_eq!(resolve_theme_id(Some("  "), Some("")), LIQUID_GLASS);
        // nothing set → glass
        assert_eq!(resolve_theme_id(None, None), LIQUID_GLASS);
    }

    #[test]
    fn resolve_unknown_id_falls_back_to_glass() {
        let cat = ThemeCatalog::builtin();
        // an id not in the catalog resolves to the glass entry, never panics
        assert_eq!(cat.resolve(Some("no-such-theme"), None).id, LIQUID_GLASS);
        assert_eq!(cat.resolve(None, None).id, LIQUID_GLASS);
    }
}
