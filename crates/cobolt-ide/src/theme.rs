// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! IDE colour themes (VSCode-inspired).
//!
//! A [`Theme`] is a flat palette that drives both the egui chrome
//! (`apply_glass_visuals` in `app.rs`) and the COBOL code-editor syntax colours
//! (`cobol_layout_job` in `panels/editor.rs`). The default theme reproduces the
//! original translucent dark-glass look exactly, so existing projects are
//! unchanged.
//!
//! Themes are selected per project (stored in `cobolt.toml` as
//! `ide.theme = "<id>"`) — see [`crate::project_model::IdeSettings`].

use egui::Color32;

/// One IDE colour theme. Panel/widget fills carry their own alpha so the
/// translucent "glass over the desktop / background image" look is preserved.
#[derive(Debug, Clone, Copy)]
pub struct Theme {
    pub id:   &'static str,
    pub name: &'static str,
    /// Base egui dark/light mode (affects defaults we do not override).
    pub dark: bool,

    // ── egui chrome ────────────────────────────────────────────────────────
    pub bg_panel:    Color32, // window/panel fill
    pub bg_widget:   Color32, // inactive widget fill
    pub bg_hover:    Color32,
    pub bg_active:   Color32,
    pub bg_extreme:  Color32, // text-edit background
    pub faint_bg:    Color32, // alternating rows
    pub code_bg:     Color32,
    pub accent:      Color32,
    pub border_dim:  Color32,
    pub border_hi:   Color32,
    pub text_dim:    Color32,
    pub text_bright: Color32,
    pub selection:   Color32,
    pub hyperlink:   Color32,
    pub warn:        Color32,
    pub error:       Color32,

    // ── COBOL editor syntax ────────────────────────────────────────────────
    pub ed_plain:     Color32,
    pub ed_keyword:   Color32,
    pub ed_data:      Color32,
    pub ed_paragraph: Color32,
    pub ed_string:    Color32,
    pub ed_comment:   Color32,
    /// Read-only RAD-generated code (blue).
    pub ed_generated: Color32,
}

const fn rgb(r: u8, g: u8, b: u8) -> Color32 {
    Color32::from_rgb(r, g, b)
}
const fn rgba(r: u8, g: u8, b: u8, a: u8) -> Color32 {
    Color32::from_rgba_premultiplied(
        // premultiply manually so `const` works
        ((r as u16 * a as u16) / 255) as u8,
        ((g as u16 * a as u16) / 255) as u8,
        ((b as u16 * a as u16) / 255) as u8,
        a,
    )
}

/// The original translucent dark-glass theme (the default).
pub const DARK_GLASS: Theme = Theme {
    id: "dark-glass",
    name: "Dark Glass",
    dark: true,
    bg_panel:    rgba(8, 8, 8, 205),
    bg_widget:   rgba(18, 18, 18, 195),
    bg_hover:    rgba(35, 35, 40, 215),
    bg_active:   rgba(45, 75, 160, 230),
    bg_extreme:  rgba(4, 4, 4, 210),
    faint_bg:    rgba(5, 5, 5, 140),
    code_bg:     rgba(12, 12, 14, 185),
    accent:      rgb(100, 160, 255),
    border_dim:  rgba(255, 255, 255, 40),
    border_hi:   rgba(130, 170, 255, 170),
    text_dim:    rgb(185, 190, 200),
    text_bright: rgb(230, 235, 245),
    selection:   rgba(65, 115, 225, 145),
    hyperlink:   rgb(130, 185, 255),
    warn:        rgb(255, 205, 80),
    error:       rgb(255, 100, 100),
    ed_plain:     rgb(212, 212, 212),
    ed_keyword:   rgb(100, 180, 255),
    ed_data:      rgb(78, 201, 130),
    ed_paragraph: rgb(220, 80, 80),
    ed_string:    rgb(210, 165, 80),
    ed_comment:   rgb(140, 140, 140),
    ed_generated: rgb(96, 160, 240),
};

/// VSCode "Dark+" — the editor's default dark theme.
pub const DARK_PLUS: Theme = Theme {
    id: "dark-plus",
    name: "Dark+",
    dark: true,
    bg_panel:    rgba(37, 37, 38, 240),
    bg_widget:   rgba(45, 45, 46, 235),
    bg_hover:    rgba(60, 60, 62, 240),
    bg_active:   rgba(14, 99, 156, 245),
    bg_extreme:  rgba(30, 30, 30, 245),
    faint_bg:    rgba(33, 33, 34, 200),
    code_bg:     rgba(30, 30, 30, 235),
    accent:      rgb(0, 122, 204),
    border_dim:  rgba(255, 255, 255, 30),
    border_hi:   rgba(0, 122, 204, 200),
    text_dim:    rgb(204, 204, 204),
    text_bright: rgb(241, 241, 241),
    selection:   rgba(38, 79, 120, 180),
    hyperlink:   rgb(86, 156, 214),
    warn:        rgb(229, 192, 123),
    error:       rgb(244, 135, 113),
    ed_plain:     rgb(212, 212, 212),
    ed_keyword:   rgb(86, 156, 214),
    ed_data:      rgb(78, 201, 176),
    ed_paragraph: rgb(220, 220, 170),
    ed_string:    rgb(206, 145, 120),
    ed_comment:   rgb(106, 153, 85),
    ed_generated: rgb(156, 220, 254),
};

/// VSCode "Light+" — the editor's default light theme.
pub const LIGHT_PLUS: Theme = Theme {
    id: "light-plus",
    name: "Light+",
    dark: false,
    bg_panel:    rgba(243, 243, 243, 245),
    bg_widget:   rgba(255, 255, 255, 245),
    bg_hover:    rgba(229, 229, 229, 248),
    bg_active:   rgba(0, 120, 215, 235),
    bg_extreme:  rgba(255, 255, 255, 250),
    faint_bg:    rgba(236, 236, 236, 220),
    code_bg:     rgba(255, 255, 255, 245),
    accent:      rgb(0, 120, 215),
    border_dim:  rgba(0, 0, 0, 40),
    border_hi:   rgba(0, 120, 215, 200),
    text_dim:    rgb(60, 60, 60),
    text_bright: rgb(20, 20, 20),
    selection:   rgba(173, 214, 255, 200),
    hyperlink:   rgb(0, 102, 204),
    warn:        rgb(191, 140, 0),
    error:       rgb(205, 49, 49),
    ed_plain:     rgb(40, 40, 40),
    ed_keyword:   rgb(0, 0, 255),
    ed_data:      rgb(38, 127, 153),
    ed_paragraph: rgb(121, 94, 38),
    ed_string:    rgb(163, 21, 21),
    ed_comment:   rgb(0, 128, 0),
    ed_generated: rgb(0, 90, 200),
};

/// Monokai-inspired dark theme.
pub const MONOKAI: Theme = Theme {
    id: "monokai",
    name: "Monokai",
    dark: true,
    bg_panel:    rgba(39, 40, 34, 240),
    bg_widget:   rgba(49, 50, 44, 235),
    bg_hover:    rgba(62, 61, 50, 240),
    bg_active:   rgba(73, 72, 62, 245),
    bg_extreme:  rgba(30, 31, 28, 245),
    faint_bg:    rgba(44, 45, 39, 200),
    code_bg:     rgba(39, 40, 34, 235),
    accent:      rgb(166, 226, 46),
    border_dim:  rgba(255, 255, 255, 28),
    border_hi:   rgba(166, 226, 46, 160),
    text_dim:    rgb(204, 204, 198),
    text_bright: rgb(248, 248, 242),
    selection:   rgba(73, 72, 62, 200),
    hyperlink:   rgb(102, 217, 239),
    warn:        rgb(230, 219, 116),
    error:       rgb(249, 38, 114),
    ed_plain:     rgb(248, 248, 242),
    ed_keyword:   rgb(249, 38, 114),
    ed_data:      rgb(102, 217, 239),
    ed_paragraph: rgb(166, 226, 46),
    ed_string:    rgb(230, 219, 116),
    ed_comment:   rgb(117, 113, 94),
    ed_generated: rgb(174, 129, 255),
};

/// Solarized Dark-inspired theme.
pub const SOLARIZED_DARK: Theme = Theme {
    id: "solarized-dark",
    name: "Solarized Dark",
    dark: true,
    bg_panel:    rgba(0, 43, 54, 240),
    bg_widget:   rgba(7, 54, 66, 235),
    bg_hover:    rgba(20, 70, 82, 240),
    bg_active:   rgba(38, 139, 210, 235),
    bg_extreme:  rgba(0, 36, 46, 245),
    faint_bg:    rgba(5, 48, 60, 200),
    code_bg:     rgba(0, 43, 54, 235),
    accent:      rgb(38, 139, 210),
    border_dim:  rgba(131, 148, 150, 50),
    border_hi:   rgba(38, 139, 210, 190),
    text_dim:    rgb(131, 148, 150),
    text_bright: rgb(238, 232, 213),
    selection:   rgba(7, 54, 66, 220),
    hyperlink:   rgb(42, 161, 152),
    warn:        rgb(181, 137, 0),
    error:       rgb(220, 50, 47),
    ed_plain:     rgb(147, 161, 161),
    ed_keyword:   rgb(133, 153, 0),
    ed_data:      rgb(42, 161, 152),
    ed_paragraph: rgb(38, 139, 210),
    ed_string:    rgb(203, 75, 22),
    ed_comment:   rgb(88, 110, 117),
    ed_generated: rgb(108, 113, 196),
};

/// High-contrast dark theme (accessibility).
pub const HIGH_CONTRAST: Theme = Theme {
    id: "high-contrast",
    name: "High Contrast",
    dark: true,
    bg_panel:    rgba(0, 0, 0, 245),
    bg_widget:   rgba(0, 0, 0, 245),
    bg_hover:    rgba(20, 20, 20, 248),
    bg_active:   rgba(0, 80, 160, 250),
    bg_extreme:  rgba(0, 0, 0, 250),
    faint_bg:    rgba(10, 10, 10, 220),
    code_bg:     rgba(0, 0, 0, 245),
    accent:      rgb(0, 200, 255),
    border_dim:  rgba(255, 255, 255, 120),
    border_hi:   rgb(255, 255, 255),
    text_dim:    rgb(230, 230, 230),
    text_bright: rgb(255, 255, 255),
    selection:   rgba(0, 120, 215, 220),
    hyperlink:   rgb(86, 200, 255),
    warn:        rgb(255, 215, 0),
    error:       rgb(255, 80, 80),
    ed_plain:     rgb(255, 255, 255),
    ed_keyword:   rgb(86, 200, 255),
    ed_data:      rgb(0, 255, 180),
    ed_paragraph: rgb(255, 140, 140),
    ed_string:    rgb(255, 200, 120),
    ed_comment:   rgb(180, 180, 180),
    ed_generated: rgb(120, 200, 255),
};

/// All selectable themes, in display order. The first is the default.
pub const THEMES: &[Theme] = &[
    DARK_GLASS,
    DARK_PLUS,
    LIGHT_PLUS,
    MONOKAI,
    SOLARIZED_DARK,
    HIGH_CONTRAST,
];

/// The default theme (preserves the original look).
pub fn default_theme() -> &'static Theme {
    &THEMES[0]
}

/// Look up a theme by id; falls back to the default for empty / unknown ids.
pub fn theme_by_id(id: &str) -> &'static Theme {
    THEMES.iter().find(|t| t.id == id).unwrap_or(&THEMES[0])
}

// ── Active-theme hand-off to the editor ─────────────────────────────────────
//
// The COBOL syntax layouter (`panels/editor.rs`) has no handle to the app, so
// the app publishes the active editor palette here once per frame and the
// layouter reads it. A thread-local keeps it lock-free (egui is single-threaded
// per context, and the IDE renders on one thread).

thread_local! {
    static ACTIVE: std::cell::Cell<Theme> = const { std::cell::Cell::new(DARK_GLASS) };
}

/// Publish the active theme (called each frame by the app).
pub fn set_active(theme: &Theme) {
    ACTIVE.with(|a| a.set(*theme));
}

/// The active theme (used by the editor's syntax layouter).
pub fn active() -> Theme {
    ACTIVE.with(|a| a.get())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_are_unique_and_lookup_works() {
        for t in THEMES {
            assert_eq!(theme_by_id(t.id).id, t.id);
        }
        // Unknown / empty fall back to default.
        assert_eq!(theme_by_id("").id, default_theme().id);
        assert_eq!(theme_by_id("nope").id, default_theme().id);
    }

    #[test]
    fn default_is_dark_glass_unchanged() {
        let d = default_theme();
        assert_eq!(d.id, "dark-glass");
        // Spot-check a couple of palette values against the historical look.
        assert_eq!(d.ed_plain, rgb(212, 212, 212));
        assert_eq!(d.ed_generated, rgb(96, 160, 240));
    }

    #[test]
    fn active_round_trips() {
        set_active(&MONOKAI);
        assert_eq!(active().id, "monokai");
        set_active(&DARK_GLASS);
        assert_eq!(active().id, "dark-glass");
    }
}
