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

impl Theme {
    /// Colour for separator / border "lines". Dark themes use a **light-grey**
    /// so dividers are visible against the dark chrome; light themes use a
    /// mid-grey.
    pub fn line(&self) -> Color32 {
        if self.dark {
            Color32::from_rgb(150, 153, 160)
        } else {
            Color32::from_rgb(168, 168, 174)
        }
    }

    /// Subtle border for glass "card" panel surfaces (a faint cool tint on dark
    /// themes, a soft shadow-grey on light themes).
    pub fn panel_border(&self) -> Color32 {
        if self.dark {
            Color32::from_rgba_unmultiplied(120, 180, 220, 46)
        } else {
            Color32::from_rgba_unmultiplied(40, 70, 100, 40)
        }
    }
}

/// A glass "card" frame for a panel surface: rounded corners, a subtle border,
/// inner padding and an outer gap so panels read as separated floating cards
/// over the background. `fill` should be the live `visuals.panel_fill` so it
/// respects the active theme / transparent-background mode.
pub fn glass_panel_frame(fill: Color32, theme: &Theme) -> egui::Frame {
    use egui::{Margin, Rounding, Shadow, Stroke, Vec2};
    egui::Frame::none()
        .fill(fill)
        .stroke(Stroke::new(1.0, theme.panel_border()))
        .rounding(Rounding::same(10.0))
        .inner_margin(Margin::same(10.0))
        .outer_margin(Margin::same(6.0))
        .shadow(Shadow {
            offset: Vec2::new(0.0, 4.0),
            blur:   16.0,
            spread: 0.0,
            color:  Color32::from_black_alpha(60),
        })
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

/// Deep Blue studio glass — a translucent blue-tinted glass look (a calm,
/// professional "deep blue studio backdrop"). Pairs well with a blue background
/// image + the transparent-background option.
pub const DEEP_BLUE: Theme = Theme {
    id: "deep-blue",
    name: "Deep Blue",
    dark: true,
    bg_panel:    rgba(12, 20, 30, 210),
    bg_widget:   rgba(20, 31, 45, 205),
    bg_hover:    rgba(30, 45, 64, 218),
    bg_active:   rgba(40, 92, 165, 232),
    bg_extreme:  rgba(8, 14, 22, 215),
    faint_bg:    rgba(14, 24, 36, 150),
    code_bg:     rgba(10, 17, 27, 195),
    accent:      rgb(90, 170, 255),
    border_dim:  rgba(120, 180, 220, 46),
    border_hi:   rgba(120, 185, 235, 180),
    text_dim:    rgb(188, 203, 224),
    text_bright: rgb(235, 242, 252),
    selection:   rgba(40, 100, 190, 150),
    hyperlink:   rgb(120, 190, 255),
    warn:        rgb(255, 205, 90),
    error:       rgb(255, 110, 110),
    ed_plain:     rgb(206, 214, 228),
    ed_keyword:   rgb(90, 170, 255),
    ed_data:      rgb(80, 200, 180),
    ed_paragraph: rgb(240, 140, 150),
    ed_string:    rgb(220, 180, 120),
    ed_comment:   rgb(110, 128, 150),
    ed_generated: rgb(130, 195, 255),
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

/// Dracula.
pub const DRACULA: Theme = Theme {
    id: "dracula",
    name: "Dracula",
    dark: true,
    bg_panel:    rgba(40, 42, 54, 240),
    bg_widget:   rgba(49, 51, 66, 235),
    bg_hover:    rgba(68, 71, 90, 240),
    bg_active:   rgba(98, 114, 164, 240),
    bg_extreme:  rgba(33, 34, 44, 245),
    faint_bg:    rgba(45, 47, 61, 200),
    code_bg:     rgba(40, 42, 54, 235),
    accent:      rgb(189, 147, 249),
    border_dim:  rgba(248, 248, 242, 36),
    border_hi:   rgba(189, 147, 249, 170),
    text_dim:    rgb(189, 195, 220),
    text_bright: rgb(248, 248, 242),
    selection:   rgba(68, 71, 90, 210),
    hyperlink:   rgb(139, 233, 253),
    warn:        rgb(241, 250, 140),
    error:       rgb(255, 85, 85),
    ed_plain:     rgb(248, 248, 242),
    ed_keyword:   rgb(255, 121, 198),
    ed_data:      rgb(139, 233, 253),
    ed_paragraph: rgb(80, 250, 123),
    ed_string:    rgb(241, 250, 140),
    ed_comment:   rgb(98, 114, 164),
    ed_generated: rgb(189, 147, 249),
};

/// Nord.
pub const NORD: Theme = Theme {
    id: "nord",
    name: "Nord",
    dark: true,
    bg_panel:    rgba(46, 52, 64, 240),
    bg_widget:   rgba(59, 66, 82, 235),
    bg_hover:    rgba(67, 76, 94, 240),
    bg_active:   rgba(94, 129, 172, 240),
    bg_extreme:  rgba(38, 43, 54, 245),
    faint_bg:    rgba(52, 59, 73, 200),
    code_bg:     rgba(46, 52, 64, 235),
    accent:      rgb(136, 192, 208),
    border_dim:  rgba(216, 222, 233, 34),
    border_hi:   rgba(136, 192, 208, 170),
    text_dim:    rgb(216, 222, 233),
    text_bright: rgb(236, 239, 244),
    selection:   rgba(67, 76, 94, 210),
    hyperlink:   rgb(136, 192, 208),
    warn:        rgb(235, 203, 139),
    error:       rgb(191, 97, 106),
    ed_plain:     rgb(216, 222, 233),
    ed_keyword:   rgb(129, 161, 193),
    ed_data:      rgb(143, 188, 187),
    ed_paragraph: rgb(163, 190, 140),
    ed_string:    rgb(163, 190, 140),
    ed_comment:   rgb(97, 110, 136),
    ed_generated: rgb(136, 192, 208),
};

/// One Dark (Atom).
pub const ONE_DARK: Theme = Theme {
    id: "one-dark",
    name: "One Dark",
    dark: true,
    bg_panel:    rgba(40, 44, 52, 240),
    bg_widget:   rgba(49, 54, 63, 235),
    bg_hover:    rgba(62, 68, 81, 240),
    bg_active:   rgba(61, 90, 128, 240),
    bg_extreme:  rgba(33, 37, 43, 245),
    faint_bg:    rgba(44, 49, 58, 200),
    code_bg:     rgba(40, 44, 52, 235),
    accent:      rgb(97, 175, 239),
    border_dim:  rgba(171, 178, 191, 34),
    border_hi:   rgba(97, 175, 239, 170),
    text_dim:    rgb(171, 178, 191),
    text_bright: rgb(220, 223, 228),
    selection:   rgba(62, 68, 81, 210),
    hyperlink:   rgb(97, 175, 239),
    warn:        rgb(229, 192, 123),
    error:       rgb(224, 108, 117),
    ed_plain:     rgb(171, 178, 191),
    ed_keyword:   rgb(198, 120, 221),
    ed_data:      rgb(86, 182, 194),
    ed_paragraph: rgb(97, 175, 239),
    ed_string:    rgb(152, 195, 121),
    ed_comment:   rgb(92, 99, 112),
    ed_generated: rgb(97, 175, 239),
};

/// Gruvbox Dark.
pub const GRUVBOX_DARK: Theme = Theme {
    id: "gruvbox-dark",
    name: "Gruvbox Dark",
    dark: true,
    bg_panel:    rgba(40, 40, 40, 240),
    bg_widget:   rgba(50, 48, 47, 235),
    bg_hover:    rgba(60, 56, 54, 240),
    bg_active:   rgba(80, 73, 69, 240),
    bg_extreme:  rgba(29, 32, 33, 245),
    faint_bg:    rgba(45, 43, 42, 200),
    code_bg:     rgba(40, 40, 40, 235),
    accent:      rgb(254, 128, 25),
    border_dim:  rgba(235, 219, 178, 34),
    border_hi:   rgba(254, 128, 25, 160),
    text_dim:    rgb(213, 196, 161),
    text_bright: rgb(251, 241, 199),
    selection:   rgba(80, 73, 69, 210),
    hyperlink:   rgb(131, 165, 152),
    warn:        rgb(250, 189, 47),
    error:       rgb(251, 73, 52),
    ed_plain:     rgb(235, 219, 178),
    ed_keyword:   rgb(251, 73, 52),
    ed_data:      rgb(131, 165, 152),
    ed_paragraph: rgb(184, 187, 38),
    ed_string:    rgb(184, 187, 38),
    ed_comment:   rgb(146, 131, 116),
    ed_generated: rgb(131, 165, 152),
};

/// Tokyo Night.
pub const TOKYO_NIGHT: Theme = Theme {
    id: "tokyo-night",
    name: "Tokyo Night",
    dark: true,
    bg_panel:    rgba(26, 27, 38, 242),
    bg_widget:   rgba(36, 40, 59, 236),
    bg_hover:    rgba(41, 46, 66, 240),
    bg_active:   rgba(61, 89, 161, 240),
    bg_extreme:  rgba(22, 22, 30, 246),
    faint_bg:    rgba(31, 33, 47, 200),
    code_bg:     rgba(26, 27, 38, 236),
    accent:      rgb(122, 162, 247),
    border_dim:  rgba(192, 202, 245, 32),
    border_hi:   rgba(122, 162, 247, 170),
    text_dim:    rgb(169, 177, 214),
    text_bright: rgb(192, 202, 245),
    selection:   rgba(41, 46, 66, 215),
    hyperlink:   rgb(125, 207, 255),
    warn:        rgb(224, 175, 104),
    error:       rgb(247, 118, 142),
    ed_plain:     rgb(169, 177, 214),
    ed_keyword:   rgb(187, 154, 247),
    ed_data:      rgb(125, 207, 255),
    ed_paragraph: rgb(122, 162, 247),
    ed_string:    rgb(158, 206, 106),
    ed_comment:   rgb(86, 95, 137),
    ed_generated: rgb(125, 207, 255),
};

/// Night Owl.
pub const NIGHT_OWL: Theme = Theme {
    id: "night-owl",
    name: "Night Owl",
    dark: true,
    bg_panel:    rgba(1, 22, 39, 242),
    bg_widget:   rgba(10, 35, 56, 236),
    bg_hover:    rgba(17, 44, 66, 240),
    bg_active:   rgba(28, 70, 100, 240),
    bg_extreme:  rgba(0, 16, 30, 246),
    faint_bg:    rgba(7, 29, 47, 200),
    code_bg:     rgba(1, 22, 39, 236),
    accent:      rgb(130, 170, 255),
    border_dim:  rgba(214, 222, 235, 32),
    border_hi:   rgba(130, 170, 255, 170),
    text_dim:    rgb(180, 196, 222),
    text_bright: rgb(214, 222, 235),
    selection:   rgba(17, 44, 66, 215),
    hyperlink:   rgb(127, 219, 202),
    warn:        rgb(236, 196, 141),
    error:       rgb(239, 83, 80),
    ed_plain:     rgb(214, 222, 235),
    ed_keyword:   rgb(199, 146, 234),
    ed_data:      rgb(127, 219, 202),
    ed_paragraph: rgb(130, 170, 255),
    ed_string:    rgb(236, 196, 141),
    ed_comment:   rgb(99, 119, 119),
    ed_generated: rgb(130, 170, 255),
};

/// Cobalt2.
pub const COBALT2: Theme = Theme {
    id: "cobalt2",
    name: "Cobalt2",
    dark: true,
    bg_panel:    rgba(25, 53, 73, 242),
    bg_widget:   rgba(33, 65, 88, 236),
    bg_hover:    rgba(40, 77, 102, 240),
    bg_active:   rgba(0, 122, 204, 240),
    bg_extreme:  rgba(21, 43, 60, 246),
    faint_bg:    rgba(29, 59, 80, 200),
    code_bg:     rgba(25, 53, 73, 236),
    accent:      rgb(255, 198, 0),
    border_dim:  rgba(255, 255, 255, 34),
    border_hi:   rgba(255, 198, 0, 150),
    text_dim:    rgb(204, 217, 230),
    text_bright: rgb(255, 255, 255),
    selection:   rgba(0, 122, 204, 150),
    hyperlink:   rgb(0, 187, 255),
    warn:        rgb(255, 198, 0),
    error:       rgb(255, 98, 140),
    ed_plain:     rgb(230, 237, 244),
    ed_keyword:   rgb(255, 157, 0),
    ed_data:      rgb(0, 187, 255),
    ed_paragraph: rgb(255, 198, 0),
    ed_string:    rgb(63, 248, 175),
    ed_comment:   rgb(0, 136, 153),
    ed_generated: rgb(0, 187, 255),
};

/// Solarized Light.
pub const SOLARIZED_LIGHT: Theme = Theme {
    id: "solarized-light",
    name: "Solarized Light",
    dark: false,
    bg_panel:    rgba(253, 246, 227, 246),
    bg_widget:   rgba(238, 232, 213, 246),
    bg_hover:    rgba(228, 222, 203, 248),
    bg_active:   rgba(38, 139, 210, 235),
    bg_extreme:  rgba(255, 250, 235, 250),
    faint_bg:    rgba(245, 238, 220, 220),
    code_bg:     rgba(253, 246, 227, 246),
    accent:      rgb(38, 139, 210),
    border_dim:  rgba(101, 123, 131, 50),
    border_hi:   rgba(38, 139, 210, 190),
    text_dim:    rgb(101, 123, 131),
    text_bright: rgb(7, 54, 66),
    selection:   rgba(147, 161, 161, 120),
    hyperlink:   rgb(42, 161, 152),
    warn:        rgb(181, 137, 0),
    error:       rgb(220, 50, 47),
    ed_plain:     rgb(88, 110, 117),
    ed_keyword:   rgb(133, 153, 0),
    ed_data:      rgb(42, 161, 152),
    ed_paragraph: rgb(38, 139, 210),
    ed_string:    rgb(203, 75, 22),
    ed_comment:   rgb(147, 161, 161),
    ed_generated: rgb(108, 113, 196),
};

/// GitHub Dark.
pub const GITHUB_DARK: Theme = Theme {
    id: "github-dark",
    name: "GitHub Dark",
    dark: true,
    bg_panel:    rgba(13, 17, 23, 242),
    bg_widget:   rgba(22, 27, 34, 236),
    bg_hover:    rgba(33, 38, 45, 240),
    bg_active:   rgba(31, 111, 235, 240),
    bg_extreme:  rgba(1, 4, 9, 246),
    faint_bg:    rgba(18, 23, 30, 200),
    code_bg:     rgba(13, 17, 23, 236),
    accent:      rgb(88, 166, 255),
    border_dim:  rgba(240, 246, 252, 30),
    border_hi:   rgba(88, 166, 255, 170),
    text_dim:    rgb(201, 209, 217),
    text_bright: rgb(240, 246, 252),
    selection:   rgba(31, 111, 235, 150),
    hyperlink:   rgb(88, 166, 255),
    warn:        rgb(210, 153, 34),
    error:       rgb(248, 81, 73),
    ed_plain:     rgb(201, 209, 217),
    ed_keyword:   rgb(255, 123, 114),
    ed_data:      rgb(121, 192, 255),
    ed_paragraph: rgb(210, 168, 255),
    ed_string:    rgb(165, 214, 255),
    ed_comment:   rgb(139, 148, 158),
    ed_generated: rgb(121, 192, 255),
};

/// Material Palenight.
pub const PALENIGHT: Theme = Theme {
    id: "palenight",
    name: "Material Palenight",
    dark: true,
    bg_panel:    rgba(41, 45, 62, 242),
    bg_widget:   rgba(50, 55, 75, 236),
    bg_hover:    rgba(60, 66, 90, 240),
    bg_active:   rgba(124, 119, 191, 240),
    bg_extreme:  rgba(35, 39, 54, 246),
    faint_bg:    rgba(46, 51, 69, 200),
    code_bg:     rgba(41, 45, 62, 236),
    accent:      rgb(130, 170, 255),
    border_dim:  rgba(166, 172, 205, 34),
    border_hi:   rgba(130, 170, 255, 170),
    text_dim:    rgb(166, 172, 205),
    text_bright: rgb(213, 217, 240),
    selection:   rgba(60, 66, 90, 215),
    hyperlink:   rgb(137, 221, 255),
    warn:        rgb(255, 203, 107),
    error:       rgb(247, 140, 108),
    ed_plain:     rgb(166, 172, 205),
    ed_keyword:   rgb(199, 146, 234),
    ed_data:      rgb(137, 221, 255),
    ed_paragraph: rgb(130, 170, 255),
    ed_string:    rgb(195, 232, 141),
    ed_comment:   rgb(103, 110, 149),
    ed_generated: rgb(137, 221, 255),
};

/// All selectable themes, in display order. The first is the default.
pub const THEMES: &[Theme] = &[
    DARK_GLASS,
    DEEP_BLUE,
    DARK_PLUS,
    LIGHT_PLUS,
    MONOKAI,
    SOLARIZED_DARK,
    HIGH_CONTRAST,
    DRACULA,
    NORD,
    ONE_DARK,
    GRUVBOX_DARK,
    TOKYO_NIGHT,
    NIGHT_OWL,
    COBALT2,
    SOLARIZED_LIGHT,
    GITHUB_DARK,
    PALENIGHT,
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
        let mut seen = std::collections::HashSet::new();
        for t in THEMES {
            assert!(seen.insert(t.id), "duplicate theme id: {}", t.id);
            assert_eq!(theme_by_id(t.id).id, t.id);
        }
        // Unknown / empty fall back to default.
        assert_eq!(theme_by_id("").id, default_theme().id);
        assert_eq!(theme_by_id("nope").id, default_theme().id);
    }

    #[test]
    fn ships_seventeen_themes() {
        assert_eq!(THEMES.len(), 17, "6 original + 10 + Deep Blue");
    }

    #[test]
    fn line_is_light_grey_on_dark_themes() {
        // Dark themes get a light-grey divider; light themes a mid-grey.
        let dark_line  = DRACULA.line();
        let light_line = LIGHT_PLUS.line();
        assert!(dark_line.r() > 130 && dark_line.g() > 130 && dark_line.b() > 130,
            "dark-theme line should be light grey, got {dark_line:?}");
        assert!(light_line.r() < 200, "light-theme line should be a grey");
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
