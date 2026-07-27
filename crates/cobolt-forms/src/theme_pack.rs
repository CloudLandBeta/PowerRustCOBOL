// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Self-describing asset-pack format for "special" form themes (spec 007).
//!
//! A pack is a folder `assets/themes/<id>/` with a `theme.toml` manifest plus
//! per-control / per-state 9-slice images, an optional themed background, and a
//! palette + chart-style spec. The catalog discovers packs without code changes
//! (R10), so a new theme is a drop-in (R2, AC7).
//!
//! The manifest is the pack's public contract. `theme.toml` looks like:
//!
//! ```toml
//! id = "stainless-steel"
//! display_name = "Stainless Steel"
//!
//! [background]
//! image = "background.png"   # optional themed background (R8)
//! tile  = false
//!
//! [palette]
//! foreground = "#dfe7ff"             # default text colour
//! chart = ["#4C9BE8", "#E87A4C", "#4CE87A", "#E84C9B"]  # chart data palette (R7)
//!
//! [chart_style]
//! stroke_width = 2.0
//! fill_texture = "chart_fill.png"    # optional material fill for bars/slices
//!
//! [controls.button]
//! image   = "button.png"
//! slice   = [12, 12, 12, 12]         # 9-slice insets: left, top, right, bottom
//! hover   = "button_hover.png"       # optional per-state images
//! pressed = "button_pressed.png"
//! disabled = "button_disabled.png"
//! focused  = "button_focused.png"
//! ```

use serde::Deserialize;
use std::collections::BTreeMap;

/// 9-slice insets in pixels: `[left, top, right, bottom]`. Corners are fixed;
/// edges and centre stretch (so art scales to any control size — R6).
pub type Slice = [u32; 4];

/// Interactive states a control skin can provide art for (R6). `Normal` is the
/// base image; the rest fall back to `Normal` when absent.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ControlState {
    Normal,
    Hover,
    Pressed,
    Disabled,
    Focused,
}

/// One control's skin: a base 9-slice image plus optional per-state images.
/// Image refs are relative to the pack folder.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct ControlSkin {
    /// Base (normal-state) 9-slice image, relative to the pack dir.
    pub image: String,
    /// 9-slice insets `[l, t, r, b]`; default `[0,0,0,0]` (no stretch protection).
    #[serde(default)]
    pub slice: Slice,
    #[serde(default)]
    pub hover: Option<String>,
    #[serde(default)]
    pub pressed: Option<String>,
    #[serde(default)]
    pub disabled: Option<String>,
    #[serde(default)]
    pub focused: Option<String>,
}

impl ControlSkin {
    /// The image ref for a given state, falling back to the normal image.
    pub fn image_for(&self, state: ControlState) -> &str {
        let alt = match state {
            ControlState::Normal => None,
            ControlState::Hover => self.hover.as_deref(),
            ControlState::Pressed => self.pressed.as_deref(),
            ControlState::Disabled => self.disabled.as_deref(),
            ControlState::Focused => self.focused.as_deref(),
        };
        alt.unwrap_or(self.image.as_str())
    }
}

/// Optional themed background (R8).
#[derive(Debug, Clone, Default, Deserialize)]
pub struct Background {
    /// Background image, relative to the pack dir.
    pub image: String,
    /// Tile (`true`) vs stretch-to-fill (`false`, default).
    #[serde(default)]
    pub tile: bool,
}

/// Palette + typography hints (R10).
#[derive(Debug, Clone, Default, Deserialize)]
pub struct Palette {
    /// Default foreground (text) colour as `#RRGGBB`; `None` ⇒ renderer default.
    #[serde(default)]
    pub foreground: Option<String>,
    /// Chart data-series palette as `#RRGGBB` strings (R7). Empty ⇒ glass palette.
    #[serde(default)]
    pub chart: Vec<String>,
}

/// Chart-style hook: themed palette + stroke + optional material fill (R7).
#[derive(Debug, Clone, Deserialize)]
pub struct ChartStyle {
    /// Stroke width for line/area charts and outlines.
    #[serde(default = "default_stroke_width")]
    pub stroke_width: f32,
    /// Optional material fill texture for bars / pie slices, relative to the pack.
    #[serde(default)]
    pub fill_texture: Option<String>,
}

fn default_stroke_width() -> f32 {
    2.0
}

impl Default for ChartStyle {
    fn default() -> Self {
        ChartStyle {
            stroke_width: default_stroke_width(),
            fill_texture: None,
        }
    }
}

/// The on-disk manifest (`theme.toml`), deserialised verbatim.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct ThemeManifest {
    pub id: String,
    pub display_name: String,
    #[serde(default)]
    pub background: Option<Background>,
    #[serde(default)]
    pub palette: Palette,
    #[serde(default)]
    pub chart_style: ChartStyle,
    /// Per-control skins, keyed by a lowercase control kind (e.g. `"button"`,
    /// `"panel"`, `"slider"`, `"label"`, `"textbox"`, …). Uncovered controls fall
    /// back to Liquid Glass (R11).
    #[serde(default)]
    pub controls: BTreeMap<String, ControlSkin>,
}

impl ThemeManifest {
    /// Every pack-relative image ref the manifest points at: the themed
    /// background, the chart fill texture, and each control skin's per-state
    /// images. Deduplicated, in a stable order.
    ///
    /// This is the pack's *drawable surface area*. The binary compiler embeds
    /// exactly this set into a built executable, so a themed form ships with
    /// the art it actually paints — and only that art (the packs also carry
    /// authoring/reference imagery the renderer never reads).
    pub fn referenced_assets(&self) -> Vec<String> {
        let mut out: Vec<String> = Vec::new();
        let mut push = |rel: &str| {
            let rel = rel.trim();
            if !rel.is_empty() && !out.iter().any(|e| e == rel) {
                out.push(rel.to_owned());
            }
        };
        if let Some(bg) = &self.background {
            push(&bg.image);
        }
        if let Some(fill) = &self.chart_style.fill_texture {
            push(fill);
        }
        for skin in self.controls.values() {
            push(&skin.image);
            for alt in [
                &skin.hover,
                &skin.pressed,
                &skin.disabled,
                &skin.focused,
            ] {
                if let Some(rel) = alt {
                    push(rel);
                }
            }
        }
        out
    }
}

/// Pack-relative image ref → file bytes, for a pack embedded in an executable.
pub type EmbeddedAssets = BTreeMap<String, std::sync::Arc<Vec<u8>>>;

/// A loaded asset-pack theme: the manifest plus its art.
///
/// The art comes from one of two places. Under the IDE (designer, preview, run
/// form) the pack is **disk-backed**: `dir` is the pack folder and image refs
/// resolve against it. A **compiled binary** has no pack folder to point at, so
/// the compiler embeds the referenced bytes into the executable and the pack
/// carries them in [`ThemePack::embedded`]. Both paths decode the same PNG
/// bytes into the same texture, so the two surfaces paint identically.
#[derive(Debug, Clone, Default)]
pub struct ThemePack {
    /// Stable id (matches the folder name and the catalog entry).
    pub id: String,
    /// Human-readable display name from the manifest.
    pub display_name: String,
    /// Absolute path of the pack folder (root for image refs). Empty for an
    /// embedded pack.
    pub dir: String,
    /// The parsed manifest.
    pub manifest: ThemeManifest,
    /// Art embedded in the executable; `None` for a disk-backed pack.
    pub embedded: Option<std::sync::Arc<EmbeddedAssets>>,
}

impl ThemePack {
    /// Build a pack whose art travels inside the executable (spec 007 R5 for
    /// compiled binaries). `manifest_toml` is the verbatim `theme.toml`;
    /// `assets` maps each pack-relative image ref to its file bytes.
    pub fn from_embedded(
        manifest_toml: &str,
        assets: &[(&str, &[u8])],
    ) -> Result<ThemePack, PackError> {
        let manifest = parse_manifest(manifest_toml)?;
        if manifest.id.is_empty() {
            return Err(PackError::BadId {
                dir: String::new(),
                id: manifest.id,
            });
        }
        let embedded: EmbeddedAssets = assets
            .iter()
            .map(|(rel, bytes)| ((*rel).to_owned(), std::sync::Arc::new(bytes.to_vec())))
            .collect();
        Ok(ThemePack {
            id: manifest.id.clone(),
            display_name: manifest.display_name.clone(),
            dir: String::new(),
            manifest,
            embedded: Some(std::sync::Arc::new(embedded)),
        })
    }

    /// Skin for a control kind (lowercased), if the pack covers it (R11).
    pub fn control(&self, kind: &str) -> Option<&ControlSkin> {
        self.manifest.controls.get(&kind.to_ascii_lowercase())
    }

    /// Absolute path for a pack-relative image ref. Meaningless for an embedded
    /// pack — use [`ThemePack::asset_bytes`], which handles both kinds.
    pub fn asset_path(&self, rel: &str) -> std::path::PathBuf {
        std::path::Path::new(&self.dir).join(rel)
    }

    /// Cache key for a pack-relative image ref, unique across packs and stable
    /// for the process lifetime.
    pub fn asset_key(&self, rel: &str) -> String {
        if self.embedded.is_some() {
            format!("theme-embed:{}/{rel}", self.id)
        } else {
            self.asset_path(rel).to_string_lossy().into_owned()
        }
    }

    /// Bytes for a pack-relative image ref: the embedded store when the pack
    /// carries its art, otherwise the file on disk. `None` when the ref is
    /// absent or unreadable, so callers fall back to Liquid Glass (R11).
    #[cfg(feature = "render")]
    pub fn asset_bytes(&self, rel: &str) -> Option<std::borrow::Cow<'_, [u8]>> {
        if let Some(store) = &self.embedded {
            return store.get(rel).map(|b| std::borrow::Cow::Borrowed(b.as_slice()));
        }
        std::fs::read(self.asset_path(rel))
            .ok()
            .map(std::borrow::Cow::Owned)
    }
}

/// Errors raised while loading a pack manifest.
#[derive(Debug)]
pub enum PackError {
    /// `theme.toml` is missing or unreadable.
    Io(std::io::Error),
    /// `theme.toml` failed to parse.
    Toml(String),
    /// The manifest id is empty or disagrees with the folder name.
    BadId { dir: String, id: String },
}

impl std::fmt::Display for PackError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PackError::Io(e) => write!(f, "theme.toml I/O error: {e}"),
            PackError::Toml(e) => write!(f, "theme.toml parse error: {e}"),
            PackError::BadId { dir, id } => {
                write!(f, "theme pack id {id:?} does not match folder {dir:?}")
            }
        }
    }
}

/// Parse a `theme.toml` manifest string (no I/O — used by tests and the wasm
/// embed path).
pub fn parse_manifest(toml_src: &str) -> Result<ThemeManifest, PackError> {
    toml::from_str(toml_src).map_err(|e| PackError::Toml(e.to_string()))
}

/// Load a pack from `<dir>/theme.toml`. The manifest `id` must match the folder
/// name (so the catalog id is stable and a drop-in needs no registration).
#[cfg(feature = "render")]
pub fn load_pack(dir: &std::path::Path) -> Result<ThemePack, PackError> {
    let manifest_path = dir.join("theme.toml");
    let src = std::fs::read_to_string(&manifest_path).map_err(PackError::Io)?;
    let manifest = parse_manifest(&src)?;
    let folder = dir
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_owned();
    if manifest.id.is_empty() || (!folder.is_empty() && manifest.id != folder) {
        return Err(PackError::BadId {
            dir: folder,
            id: manifest.id,
        });
    }
    Ok(ThemePack {
        id: manifest.id.clone(),
        display_name: manifest.display_name.clone(),
        dir: dir.to_string_lossy().into_owned(),
        manifest,
        embedded: None,
    })
}

/// Discover every pack under `themes_dir` (each `<themes_dir>/<id>/theme.toml`),
/// in sorted id order. Folders without a parseable `theme.toml` are skipped
/// (logged to stderr) rather than failing discovery (R10).
#[cfg(feature = "render")]
pub fn discover_packs(themes_dir: &std::path::Path) -> Vec<ThemePack> {
    let mut packs = Vec::new();
    let Ok(entries) = std::fs::read_dir(themes_dir) else {
        return packs;
    };
    let mut dirs: Vec<_> = entries
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.is_dir())
        .collect();
    dirs.sort();
    for d in dirs {
        match load_pack(&d) {
            Ok(p) => packs.push(p),
            Err(e) => eprintln!("skipping theme pack {}: {e}", d.display()),
        }
    }
    packs
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r##"
id = "stainless-steel"
display_name = "Stainless Steel"

[background]
image = "background.png"
tile = false

[palette]
foreground = "#dfe7ff"
chart = ["#4C9BE8", "#E87A4C", "#4CE87A", "#E84C9B"]

[chart_style]
stroke_width = 2.5
fill_texture = "chart_fill.png"

[controls.button]
image = "button.png"
slice = [12, 12, 12, 12]
hover = "button_hover.png"
pressed = "button_pressed.png"

[controls.panel]
image = "panel.png"
slice = [16, 16, 16, 16]
"##;

    #[test]
    fn parses_manifest_with_controls_palette_and_chart_style() {
        let m = parse_manifest(SAMPLE).expect("parse");
        assert_eq!(m.id, "stainless-steel");
        assert_eq!(m.display_name, "Stainless Steel");
        assert_eq!(m.palette.chart.len(), 4);
        assert_eq!(m.palette.foreground.as_deref(), Some("#dfe7ff"));
        assert_eq!(m.chart_style.stroke_width, 2.5);
        assert_eq!(
            m.chart_style.fill_texture.as_deref(),
            Some("chart_fill.png")
        );
        assert!(m.background.is_some());

        let btn = m.controls.get("button").expect("button skin");
        assert_eq!(btn.slice, [12, 12, 12, 12]);
        assert_eq!(btn.image_for(ControlState::Hover), "button_hover.png");
        // Disabled has no art → falls back to the normal image (R11/state fallback).
        assert_eq!(btn.image_for(ControlState::Disabled), "button.png");
        assert!(m.controls.contains_key("panel"));
    }

    #[test]
    fn chart_style_defaults_when_absent() {
        let m = parse_manifest("id = \"x\"\ndisplay_name = \"X\"\n").expect("parse");
        assert_eq!(m.chart_style.stroke_width, 2.0);
        assert!(m.chart_style.fill_texture.is_none());
        assert!(m.palette.chart.is_empty());
        assert!(m.controls.is_empty());
    }

    #[test]
    fn referenced_assets_covers_background_fill_and_every_skin_state() {
        let m = parse_manifest(SAMPLE).expect("parse");
        let refs = m.referenced_assets();
        for expected in [
            "background.png",
            "chart_fill.png",
            "button.png",
            "button_hover.png",
            "button_pressed.png",
            "panel.png",
        ] {
            assert!(refs.iter().any(|r| r == expected), "missing {expected}");
        }
        // States without their own art fall back to the normal image, which is
        // already listed — the set must not repeat it.
        let mut deduped = refs.clone();
        deduped.sort();
        deduped.dedup();
        assert_eq!(deduped.len(), refs.len(), "refs must be deduplicated");
    }

    #[test]
    fn embedded_pack_carries_its_art_and_needs_no_folder() {
        const ART: &[u8] = b"\x89PNG-not-really";
        let pack = ThemePack::from_embedded(SAMPLE, &[("button.png", ART)]).expect("embed");
        assert_eq!(pack.id, "stainless-steel");
        assert_eq!(pack.display_name, "Stainless Steel");
        assert!(pack.dir.is_empty(), "an embedded pack has no folder");
        assert_eq!(pack.control("button").unwrap().slice, [12, 12, 12, 12]);
        // The cache key is pack-scoped, so two packs' same-named art cannot
        // collide in the shared texture cache.
        assert_eq!(pack.asset_key("button.png"), "theme-embed:stainless-steel/button.png");
    }

    #[cfg(feature = "render")]
    #[test]
    fn embedded_bytes_are_served_without_touching_the_filesystem() {
        const ART: &[u8] = b"\x89PNG-not-really";
        let pack = ThemePack::from_embedded(SAMPLE, &[("button.png", ART)]).expect("embed");
        assert_eq!(pack.asset_bytes("button.png").as_deref(), Some(ART));
        // Art the pack does not carry resolves to nothing rather than falling
        // through to a stray relative path on the end user's machine.
        assert!(pack.asset_bytes("panel.png").is_none());
    }

    #[cfg(feature = "render")]
    #[test]
    fn disk_backed_pack_reads_its_art_from_the_pack_folder() {
        let tmp = std::env::temp_dir().join(format!("cobolt_pack_bytes_{}", std::process::id()));
        let dir = tmp.join("stainless-steel");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("theme.toml"), SAMPLE).unwrap();
        std::fs::write(dir.join("button.png"), b"on-disk").unwrap();

        let pack = load_pack(&dir).expect("load");
        assert!(pack.embedded.is_none());
        assert_eq!(pack.asset_bytes("button.png").as_deref(), Some(&b"on-disk"[..]));
        assert_eq!(pack.asset_key("button.png"), dir.join("button.png").to_string_lossy());

        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[cfg(feature = "render")]
    #[test]
    fn discovers_packs_from_a_directory() {
        let tmp = std::env::temp_dir().join(format!("cobolt_themes_{}", std::process::id()));
        let pack = tmp.join("stainless-steel");
        std::fs::create_dir_all(&pack).unwrap();
        std::fs::write(pack.join("theme.toml"), SAMPLE).unwrap();
        // A bogus folder without a manifest must be skipped, not fail discovery.
        std::fs::create_dir_all(tmp.join("not-a-pack")).unwrap();

        let found = discover_packs(&tmp);
        assert_eq!(found.len(), 1, "exactly one valid pack discovered");
        assert_eq!(found[0].id, "stainless-steel");
        assert_eq!(found[0].control("button").unwrap().slice, [12, 12, 12, 12]);

        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[cfg(feature = "render")]
    #[test]
    fn loads_bundled_cobalt_steel_reference_pack() {
        // The reference pack (T8) lives at <repo>/assets/themes/cobalt-steel,
        // generated by `examples/gen_reference_theme.rs`. Skip gracefully if it
        // has not been generated in this checkout.
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../assets/themes/cobalt-steel");
        if !dir.join("theme.toml").exists() {
            eprintln!("reference pack not generated; skipping");
            return;
        }
        let pack = load_pack(&dir).expect("load cobalt-steel");
        assert_eq!(pack.id, "cobalt-steel");
        assert!(
            pack.manifest.palette.chart.len() >= 4,
            "chart palette present"
        );
        let btn = pack.control("button").expect("button skin");
        assert!(btn.image_for(ControlState::Hover).contains("hover"));
        // The referenced image files exist on disk.
        assert!(
            pack.asset_path(&btn.image).exists(),
            "button image must exist"
        );
        assert!(pack.manifest.background.is_some());
    }
}
