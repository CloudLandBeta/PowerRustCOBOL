// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! What a form theme IS (spec 050).
//!
//! A theme used to be an *identity* — a two-variant enum every painter tested
//! against (`if elegance_active(ctx) { … } else { … }`). That shape has two
//! consequences, and the project hit both:
//!
//! 1. **Adding a theme meant editing every painter.** Eleven sites asked "is it
//!    Elegance?", so a third theme meant eleven more branches.
//! 2. **Anything nobody remembered to branch on leaked.** The form's
//!    `GlassStyle` was read straight through, so a Liquid Glass setting reached
//!    controls painted by a theme that has no frost and no relief — suppressing
//!    drop shadows and painting neumorphic rims on a flat slate surface.
//!
//! Here a theme is an **implementation**: painters ask it questions and it
//! answers, so registering one touches no painter at all.
//!
//! ## The `Option` contract
//!
//! Every accessor returns `Option`, and **`None` means "I have nothing to say —
//! use the built-in Liquid Glass default"**.
//!
//! That is not a convenience. It is what makes [`LiquidGlassTheme`] answer
//! `None` to everything and therefore run *literally the same code with the
//! same constants* it always did — the historical look cannot drift, because
//! the theme layer cannot reach it. It also lets a partial theme cover what it
//! wants and inherit the rest, which is spec 007 R11's fallback rule expressed
//! in the type rather than in a comment.
//!
//! ## Self-contained themes
//!
//! [`SurfaceTheme::is_self_contained`] is the declaration that a theme owns the
//! WHOLE look. Liquid Glass configuration — the `GlassStyle` register, its
//! frost and its neumorphic relief — must not be applied on top of one. The
//! developer's own explicit control properties still win; it is the *ambient
//! glass configuration* that is excluded, never the design.

use crate::model::ControlType;
use egui::{Color32, Context};
use std::fmt::Debug;
use std::sync::Arc;

// ── The vocabulary painters speak ───────────────────────────────────────────

/// What a sub-element *is*, so a theme can paint it in the right register.
///
/// Liquid Glass ignores this (every sub-element is frost tinted by `base`); a
/// flat theme needs it, because a recessed input well and a raised card are
/// different colours in a flat palette rather than the same frost at different
/// tints.
///
/// Five variants, and deliberately still five (spec 050 plan §4 decision 2).
/// This answers "which visual register is this surface in", a genuinely closed
/// question. "What colour defaults this property" is open-ended and is asked
/// through [`ColorToken`] instead — otherwise this enum would grow a variant
/// per call site (`SliderTrack`, `ProgressTrough`, `GridHeader`, …), each
/// meaning "a colour I needed once".
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SurfaceRole {
    /// A raised card face — non-visual control cards, picture frames, popups.
    Card,
    /// A recessed well the user reads or types into — combo headers, tick boxes.
    Input,
    /// A pressable control face — the accent-filled button look.
    Button,
    /// An accent-filled indicator — the filled portion of a progress bar.
    Accent,
    /// A face whose colour the developer chose; the caller's `base` leads.
    Shape,
    /// A two-state toggle indicator — a check box, a radio dot, a switch track.
    ///
    /// The sixth role, and it earns its place: unlike the eight colour-defaults
    /// that go through [`ColorToken`], a toggle really is its own structural
    /// register, and it is the only one whose face depends on a *state*
    /// ([`SurfaceState::on`]) rather than on the control's kind.
    Toggle,
}

/// Which Elegance-style register a control's face belongs in.
///
/// Containers and display surfaces read as raised cards; anything the operator
/// reads values out of or types into reads as a recessed well; a button is the
/// one accent-filled face. Kinds not listed here never reach this function —
/// they are frameless (Label, Line) or paint themselves (charts do their own
/// face, then their data marks).
pub fn role_for(ct: &ControlType) -> SurfaceRole {
    use ControlType as CT;
    match ct {
        CT::Button => SurfaceRole::Button,
        CT::TextBox
        | CT::ComboBox
        | CT::ListBox
        | CT::DataGrid
        | CT::TreeView
        | CT::NumericUpDown
        | CT::DateTimePicker
        | CT::Slider
        | CT::ProgressBar => SurfaceRole::Input,
        _ => SurfaceRole::Card,
    }
}

/// How a surface is being drawn right now.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct SurfaceState {
    /// The control is selected on the designer canvas, or focused at run time.
    pub selected: bool,
    /// A [`SurfaceRole::Toggle`] that is ON — checked, or switched on.
    pub on: bool,
}

/// How to paint one surface: a flat fill and a one-pixel border.
///
/// Deliberately not a painter callback. A theme describes; `paint.rs` draws.
/// That keeps every theme free of `egui` drawing code and keeps the corner
/// rounding, alpha folding and stroke-kind rules in one place.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SurfaceSpec {
    /// The face colour — or `None` when the CALLER's `base` leads: a
    /// [`SurfaceRole::Shape`] whose colour the developer chose, and a
    /// [`SurfaceRole::Accent`] indicator whose fill carries its own meaning.
    pub fill: Option<Color32>,
    pub border: Color32,
    pub border_width: f32,
}

/// A named colour a painter uses to DEFAULT an unset property.
///
/// Eight of the eleven old `elegance_active` sites wanted one of these, not a
/// structural face: the slider track, the spec-039 widget accents, the progress
/// trough, the text colour (twice), the chart series, the grid header and the
/// tree foreground.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ColorToken {
    /// Body text — what the operator reads out of and types into a control.
    Text,
    /// A Label's own text. Separate from [`Self::Text`] because a Label is
    /// frameless — its glyphs ARE its face, and a theme may want it quieter
    /// than the text inside an input.
    LabelText,
    /// Secondary text — hints, disabled captions.
    DimText,
    /// The filled part of a slider's rail, from the start to the knob. `None`
    /// leaves the rail unfilled, which is what Liquid Glass has always drawn.
    SliderFill,
    /// A slider's knob face.
    SliderKnob,
    /// The FORM's own backdrop, when the developer set no background colour.
    /// `None` leaves the form's existing default alone.
    FormBackground,
    /// The background of a recessed well (input, trough, slider track).
    InputBg,
    /// A raised card face.
    Card,
    /// A card lifted one step — a grid's header band against its body.
    CardRaised,
    /// Ordinary borders and rules.
    Border,
    /// The focus / selection ring.
    Focus,
    /// One of the theme's named accents.
    Accent(AccentName),
}

/// The accent family, named in OUR vocabulary.
///
/// A theme maps these onto whatever it uses internally. Keeping the names here
/// rather than re-exporting a third-party enum is what stops the palette crate's
/// types — and its name — spreading through the codebase (spec 050 R22).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AccentName {
    Blue,
    Green,
    Red,
    Purple,
    Amber,
    Sky,
}

impl AccentName {
    /// Parse the `AccentColor` property's value; anything unrecognised is Blue,
    /// which is the historical default.
    pub fn parse(name: &str) -> Self {
        match name {
            "Green" => AccentName::Green,
            "Red" => AccentName::Red,
            "Purple" => AccentName::Purple,
            "Amber" => AccentName::Amber,
            "Sky" => AccentName::Sky,
            _ => AccentName::Blue,
        }
    }
}

/// Which corner radius a caller wants when the control has none of its own.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RadiusKind {
    /// Ordinary controls — buttons, inputs, tabs.
    Control,
    /// Card-like surfaces — panels, group boxes, popups.
    Card,
}

// ── The trait ───────────────────────────────────────────────────────────────

/// A form theme, as the painters see it.
///
/// See the module note for the `Option` contract: **`None` = use the built-in
/// Liquid Glass default**.
pub trait SurfaceTheme: Debug + Send + Sync {
    /// The stable catalogue id (`"liquid-glass"`, `"elegance"`).
    fn id(&self) -> &str;

    /// Does this theme own the WHOLE look?
    ///
    /// `true` excludes Liquid Glass's ambient configuration — the `GlassStyle`
    /// register, its frost, its neumorphic relief — from every control the theme
    /// paints. It does **not** exclude the developer's own explicit properties,
    /// which always win.
    fn is_self_contained(&self) -> bool {
        false
    }

    /// How to paint a structural surface, or `None` to leave it to Liquid Glass.
    fn surface(&self, _role: SurfaceRole, _state: SurfaceState) -> Option<SurfaceSpec> {
        None
    }

    /// A named colour, or `None` to leave the caller's built-in default alone.
    fn token(&self, _tok: ColorToken) -> Option<Color32> {
        None
    }

    /// A default corner radius, or `None` to leave the caller's built-in alone.
    fn radius(&self, _kind: RadiusKind) -> Option<f32> {
        None
    }

    /// The chart series palette, or `None` for the built-in accents.
    fn data_marks(&self) -> Option<Vec<Color32>> {
        None
    }

    /// The theme's own colours, for the colour picker's swatch grid.
    ///
    /// These are what a developer reaches for first when styling a control, so
    /// the picker offers them before anything else. Order matters — it is the
    /// order they appear in, filling left to right, top to bottom.
    ///
    /// A theme may offer as many or as few as it likes; whatever is left of the
    /// grid becomes the operator's own custom-colour memory. An empty list (the
    /// default) gives the whole grid over to that memory.
    fn swatches(&self) -> Vec<Color32> {
        Vec::new()
    }

    /// Install this theme's visuals for widgets that are drawn by a third-party
    /// crate and read their palette from the `egui::Context`.
    ///
    /// ⚠️ **Call this only from a host whose `Context` belongs solely to the
    /// form window.** It mutates the *global* style, so calling it from a
    /// surface that shares its Context with other UI — the IDE, which drives
    /// every form through `show_viewport_immediate` — would restyle that UI
    /// too. There is exactly one call site, in `cobolt-form-host`, and a test
    /// asserts `cobolt-ide` never gains one.
    ///
    /// Surfaces that skip it lose nothing structural: such widgets fall back to
    /// their crate's own default palette.
    fn install_widget_visuals(&self, _ctx: &Context) {}
}

// ── Liquid Glass ────────────────────────────────────────────────────────────

/// The original procedural look — and the behaviour of any surface that never
/// publishes a theme at all.
///
/// Every accessor is the trait default (`None`), which is the whole point: the
/// glass paths keep their own constants and their own code, so this theme
/// cannot move them. Adding a colour here would be a bug, not a feature.
#[derive(Debug, Clone, Copy, Default)]
pub struct LiquidGlassTheme;

impl SurfaceTheme for LiquidGlassTheme {
    fn id(&self) -> &str {
        crate::theme::LIQUID_GLASS
    }
}

/// The shared Liquid Glass instance — the default for an unpublished context.
pub fn liquid_glass() -> Arc<dyn SurfaceTheme> {
    Arc::new(LiquidGlassTheme)
}

// ── Elegance ────────────────────────────────────────────────────────────────

/// Elegance: flat slate surfaces and a cool accent family (spec 047).
///
/// **Self-contained** — it paints from a fixed palette rather than compositing
/// art, so there is nothing for Liquid Glass's frost or neumorphic relief to
/// sit on. `GlassStyle` does not apply to it, and this is where that is
/// declared rather than guessed at eleven painter sites.
///
/// The third-party palette is held **privately**. Nothing outside this type may
/// name that crate's types, which is what keeps its name out of the codebase —
/// and therefore out of any user-facing string — structurally rather than by
/// review (spec 050 R22).
/// PowerRustCOBOL's Elegance defaults.
///
/// These are **ours** — informed by the palette crate, not copied from it. The
/// theme's structural colours (card, input well, focus ring) still come from the
/// crate; the ones a developer actually notices on a form are fixed here, so a
/// crate upgrade cannot silently restyle every shipped application.
///
/// Every one of them is a DEFAULT. A control carrying an explicit
/// `BackgroundColor` / `ForegroundColor` keeps it, under this theme as under any
/// other (R9).
mod eleg {
    use egui::Color32;

    /// Text inside an input, and a slider's knob face.
    pub const INK: Color32 = Color32::from_rgb(0xFF, 0xFF, 0xFF);
    /// Labels, borders, the filled part of a slider rail, a toggle's off rim.
    pub const MUTED: Color32 = Color32::from_rgb(0x86, 0x91, 0xA3);
    /// Every button's face.
    pub const PRIMARY: Color32 = Color32::from_rgb(0x37, 0x61, 0xE2);
    /// A toggle that is ON — check box, radio, switch.
    pub const ON: Color32 = Color32::from_rgb(0x4C, 0xA0, 0x53);
    /// The FORM's own backdrop.
    pub const FORM_BG: Color32 = Color32::from_rgb(0x0F, 0x17, 0x2A);
    /// A container's face — Panel, GroupBox — one step lighter than the form,
    /// so a panel reads as sitting ON the form rather than cut out of it.
    pub const CONTAINER_BG: Color32 = Color32::from_rgb(0x20, 0x29, 0x3A);
}

#[derive(Clone)]
pub struct EleganceTheme {
    palette: elegance::Palette,
    control_radius: f32,
    card_radius: f32,
}

impl Debug for EleganceTheme {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EleganceTheme")
            .field("control_radius", &self.control_radius)
            .field("card_radius", &self.card_radius)
            .finish_non_exhaustive()
    }
}

impl Default for EleganceTheme {
    fn default() -> Self {
        Self::new()
    }
}

impl EleganceTheme {
    pub fn new() -> Self {
        let t = elegance::Theme::slate();
        Self {
            palette: t.palette,
            control_radius: t.control_radius,
            card_radius: t.card_radius,
        }
    }
}

impl SurfaceTheme for EleganceTheme {
    fn id(&self) -> &str {
        crate::theme::ELEGANCE
    }

    /// Yes — and this single `true` is what suppresses the whole Liquid Glass
    /// configuration for forms painted in this theme.
    fn is_self_contained(&self) -> bool {
        true
    }

    fn surface(&self, role: SurfaceRole, state: SurfaceState) -> Option<SurfaceSpec> {
        let p = &self.palette;
        // A toggle is the one role whose face follows a STATE: filled green when
        // on, and nothing but a rim when off, so an unchecked box reads as an
        // empty outline rather than as a filled well.
        if role == SurfaceRole::Toggle {
            return Some(SurfaceSpec {
                fill: Some(if state.on {
                    eleg::ON
                } else {
                    Color32::TRANSPARENT
                }),
                border: if state.on { eleg::ON } else { eleg::MUTED },
                border_width: 1.0,
            });
        }
        // `Shape` and `Accent` are colour-led by the caller: the developer
        // picked a shape's fill, and a progress fill carries its own meaning.
        // The structural roles take the palette.
        let fill = match role {
            SurfaceRole::Card => Some(eleg::CONTAINER_BG),
            SurfaceRole::Input => Some(p.input_bg),
            SurfaceRole::Button => Some(eleg::PRIMARY),
            SurfaceRole::Shape | SurfaceRole::Accent | SurfaceRole::Toggle => None,
        };
        Some(SurfaceSpec {
            fill,
            border: if state.selected { p.focus } else { eleg::MUTED },
            border_width: if state.selected { 1.5 } else { 1.0 },
        })
    }

    fn token(&self, tok: ColorToken) -> Option<Color32> {
        let p = &self.palette;
        Some(match tok {
            ColorToken::Text => eleg::INK,
            ColorToken::LabelText => eleg::MUTED,
            ColorToken::SliderFill => eleg::MUTED,
            ColorToken::SliderKnob => eleg::INK,
            ColorToken::DimText => eleg::MUTED,
            ColorToken::InputBg => p.input_bg,
            ColorToken::Card => eleg::CONTAINER_BG,
            ColorToken::FormBackground => eleg::FORM_BG,
            // A grid's header band against its body: one step of depth, not a
            // second colour to keep in sync.
            ColorToken::CardRaised => p.depth_tint(eleg::CONTAINER_BG, 0.04),
            ColorToken::Border => eleg::MUTED,
            ColorToken::Focus => p.focus,
            ColorToken::Accent(name) => {
                use elegance::Accent;
                p.accent_fill(match name {
                    AccentName::Green => Accent::Green,
                    AccentName::Red => Accent::Red,
                    AccentName::Purple => Accent::Purple,
                    AccentName::Amber => Accent::Amber,
                    AccentName::Sky => Accent::Sky,
                    AccentName::Blue => Accent::Blue,
                })
            }
        })
    }

    fn radius(&self, kind: RadiusKind) -> Option<f32> {
        Some(match kind {
            RadiusKind::Control => self.control_radius,
            RadiusKind::Card => self.card_radius,
        })
    }

    /// The theme's accent family is already a set of distinguishable hues,
    /// which is exactly what a series palette needs (spec 047 R4).
    fn data_marks(&self) -> Option<Vec<Color32>> {
        let p = &self.palette;
        Some(vec![p.blue, p.amber, p.green, p.purple, p.red, p.focus])
    }

    /// Elegance's own 24: six accents, then their disabled and hover variants,
    /// then the six neutrals — four rows of six, in that order.
    fn swatches(&self) -> Vec<Color32> {
        const HEX: [u32; 24] = [
            // Basic
            0x2563EB, 0x16A34A, 0xDC2626, 0x7C3AED, 0xD97706, 0x38BDF8,
            // Disabled variants
            0x21438A, 0x1A6042, 0x742832, 0x48318B, 0x724C23, 0x2A6C90,
            // Hover variants
            0x1D4ED8, 0x15803D, 0xB91C1C, 0x6D28D9, 0xB45309, 0x30A1D3,
            // Neutrals
            0xE2E8F0, 0x94A3B8, 0x64748B, 0x475569, 0x334155, 0x1E293B,
        ];
        HEX.iter()
            .map(|v| {
                Color32::from_rgb((v >> 16) as u8, ((v >> 8) & 0xFF) as u8, (v & 0xFF) as u8)
            })
            .collect()
    }

    /// Knob, Gauge, Switch and FileDropZone are real widgets from the palette
    /// crate; they read their theme from the context and otherwise fall back to
    /// an *un-installed* default. Installing it makes the palette explicit
    /// rather than accidental, and registers the bundled symbol font so their
    /// glyphs render instead of tofu. Cheap per frame — it early-returns when
    /// the theme is unchanged.
    ///
    /// ⚠️ See the trait method: host-only, because this mutates the global
    /// style.
    fn install_widget_visuals(&self, ctx: &Context) {
        elegance::Theme::slate().install(ctx);
    }
}

/// The shared Elegance instance.
pub fn elegance() -> Arc<dyn SurfaceTheme> {
    Arc::new(EleganceTheme::new())
}

// ── The registry ────────────────────────────────────────────────────────────

/// The theme a resolved catalogue id paints with — **the one place a procedural
/// theme is registered** (spec 050 R14).
///
/// Adding a look is: implement [`SurfaceTheme`], add a [`crate::theme::FormTheme`]
/// entry, and add one arm here. No painter changes, because no painter asks who
/// the theme is.
///
/// Anything unrecognised — an asset-pack id, an unknown id, an empty string —
/// maps to Liquid Glass, which is both the historical default and the correct
/// base for a pack theme (a pack skins the controls it covers and falls back to
/// glass for the rest).
pub fn for_theme_id(id: &str) -> Arc<dyn SurfaceTheme> {
    match id.trim() {
        crate::theme::ELEGANCE => elegance(),
        _ => liquid_glass(),
    }
}

/// The painting theme for an ASSET PACK.
///
/// A pack's look comes from its 9-slice art, not from tokens, so it supplies no
/// colours here — but it may still declare that it owns the WHOLE look, in which
/// case Liquid Glass's frost and relief must not be layered over its art
/// (R3). That declaration is the pack's, from its manifest; this carries it to
/// the gate.
pub fn for_pack(self_contained: bool) -> Arc<dyn SurfaceTheme> {
    if self_contained {
        Arc::new(PackBaseTheme)
    } else {
        liquid_glass()
    }
}

/// A pack that owns the whole look: no tokens, no surfaces — Liquid Glass's
/// *configuration* simply does not apply over its art.
#[derive(Debug, Clone, Copy)]
struct PackBaseTheme;

impl SurfaceTheme for PackBaseTheme {
    fn id(&self) -> &str {
        "asset-pack"
    }
    fn is_self_contained(&self) -> bool {
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The `Option` contract, stated as a test: Liquid Glass answers `None` to
    /// everything, so it cannot reach — and therefore cannot move — the
    /// built-in painting it stands for.
    #[test]
    fn liquid_glass_says_nothing_at_all() {
        let t = LiquidGlassTheme;
        assert_eq!(t.id(), "liquid-glass");
        assert!(!t.is_self_contained(), "glass config applies to itself");

        let roles = [
            SurfaceRole::Card,
            SurfaceRole::Input,
            SurfaceRole::Button,
            SurfaceRole::Accent,
            SurfaceRole::Shape,
            SurfaceRole::Toggle,
        ];
        for r in roles {
            for selected in [false, true] {
                for on in [false, true] {
                    assert!(
                        t.surface(r, SurfaceState { selected, on }).is_none(),
                        "{r:?}/{selected}/{on} must fall through to the glass painter"
                    );
                }
            }
        }

        let tokens = [
            ColorToken::Text,
            ColorToken::DimText,
            ColorToken::InputBg,
            ColorToken::Card,
            ColorToken::CardRaised,
            ColorToken::Border,
            ColorToken::Focus,
            ColorToken::Accent(AccentName::Blue),
            ColorToken::Accent(AccentName::Green),
            ColorToken::Accent(AccentName::Red),
            ColorToken::Accent(AccentName::Purple),
            ColorToken::Accent(AccentName::Amber),
            ColorToken::Accent(AccentName::Sky),
        ];
        for tok in tokens {
            assert!(t.token(tok).is_none(), "{tok:?} must keep its built-in");
        }
        for k in [RadiusKind::Control, RadiusKind::Card] {
            assert!(t.radius(k).is_none(), "{k:?} must keep its built-in");
        }
        assert!(t.data_marks().is_none(), "charts keep the built-in accents");

        eprintln!(
            "050 Liquid Glass — {} roles x2 states, {} tokens, {} radii, \
             data marks: all None (the built-ins are unreachable from a theme)",
            roles.len(),
            tokens.len(),
            2
        );
    }

    /// The accent names are ours, and an unknown one is Blue (the historical
    /// default) rather than a panic or a blank.
    #[test]
    fn accent_names_parse_ours_and_default_to_blue() {
        for (s, want) in [
            ("Green", AccentName::Green),
            ("Red", AccentName::Red),
            ("Purple", AccentName::Purple),
            ("Amber", AccentName::Amber),
            ("Sky", AccentName::Sky),
            ("Blue", AccentName::Blue),
            ("", AccentName::Blue),
            ("Chartreuse", AccentName::Blue),
        ] {
            assert_eq!(AccentName::parse(s), want, "{s:?}");
        }
        eprintln!("050 accents — 6 named + unknown/empty → Blue");
    }

    /// 050 AC1 — the catalogue REPORTS look ownership; nothing infers it.
    #[test]
    fn catalog_declares_look_ownership() {
        use crate::theme::{FormTheme, ThemeCatalog, ELEGANCE, LIQUID_GLASS};
        let cat = ThemeCatalog::builtin();
        let rows: Vec<(String, String, bool)> = cat
            .themes()
            .iter()
            .map(|t| (t.id.clone(), format!("{:?}", t.kind), t.self_contained))
            .collect();

        assert!(
            !cat.get(LIQUID_GLASS).expect("built in").self_contained,
            "Liquid Glass IS the glass configuration — it applies to itself"
        );
        assert!(
            cat.get(ELEGANCE).expect("built in").self_contained,
            "Elegance is flat: no frost and no relief for the register to configure"
        );
        assert!(!FormTheme::liquid_glass().self_contained);
        assert!(FormTheme::elegance().self_contained);

        // R3 — a pack's declaration reaches the PAINTING theme too, not just
        // the catalogue entry. A flag that is stored and never consulted is the
        // exact bug pattern this spec exists to close.
        assert!(
            !for_pack(false).is_self_contained(),
            "an ordinary pack keeps Liquid Glass underneath it"
        );
        assert!(
            for_pack(true).is_self_contained(),
            "a pack that declares it owns the look closes the gate"
        );
        assert!(
            for_pack(true).token(ColorToken::Text).is_none(),
            "…and still supplies no tokens — its look is its art"
        );

        // A pack manifest without the key reports `false`, so every pack
        // authored before it existed keeps its behaviour with no edit.
        let bare: crate::theme_pack::ThemeManifest =
            toml::from_str("id = \"p\"\ndisplay_name = \"P\"\n").expect("minimal manifest");
        assert!(!bare.self_contained, "absent ⇒ not self-contained");
        let declared: crate::theme_pack::ThemeManifest =
            toml::from_str("id = \"p\"\ndisplay_name = \"P\"\nself_contained = true\n")
                .expect("manifest with the key");
        assert!(declared.self_contained, "a pack may declare it");

        println!("\n  050 AC1 — {:<16} {:<12} self_contained", "id", "kind");
        for (id, kind, sc) in &rows {
            println!("             {id:<16} {kind:<12} {sc}");
        }
        println!("             (pack manifest: absent ⇒ false, declared ⇒ true)\n");
    }

    /// The Elegance defaults the operator specified, pinned by value.
    ///
    /// These are PowerRustCOBOL's, not the palette crate's — which is the point:
    /// a crate upgrade cannot restyle a shipped application's buttons, labels or
    /// toggles behind the developer's back.
    #[test]
    fn elegance_defaults_are_the_specified_colours() {
        let t = EleganceTheme::new();
        let hex = |c: Color32| format!("#{:02x}{:02x}{:02x}", c.r(), c.g(), c.b());
        let tok = |k: ColorToken| t.token(k).expect("Elegance answers every token");

        // Text inside an input is white; a Label's own text is the muted grey.
        assert_eq!(hex(tok(ColorToken::Text)), "#ffffff");
        assert_eq!(hex(tok(ColorToken::LabelText)), "#8691a3");

        // Every button, unless the developer sets its BackgroundColor.
        let button = t
            .surface(SurfaceRole::Button, SurfaceState::default())
            .expect("a button face");
        assert_eq!(hex(button.fill.expect("buttons are theme-filled")), "#3761e2");

        // A toggle: filled green ON, nothing but a rim OFF.
        let on = t
            .surface(SurfaceRole::Toggle, SurfaceState { selected: false, on: true })
            .expect("a toggle face");
        assert_eq!(hex(on.fill.expect("on is filled")), "#4ca053");
        let off = t
            .surface(SurfaceRole::Toggle, SurfaceState { selected: false, on: false })
            .expect("a toggle face");
        assert_eq!(
            off.fill,
            Some(Color32::TRANSPARENT),
            "an unchecked toggle is an outline, not a filled well"
        );
        assert_eq!(hex(off.border), "#8691a3");

        // The slider: filled range grey, knob white with the same grey rim.
        assert_eq!(hex(tok(ColorToken::SliderFill)), "#8691a3");
        assert_eq!(hex(tok(ColorToken::SliderKnob)), "#ffffff");
        assert_eq!(hex(tok(ColorToken::Border)), "#8691a3");

        println!(
            "\n  050 Elegance defaults — input text {}, label {}, button {}, \
             toggle on {} / off transparent+{}, slider fill {} knob {} rim {}\n",
            hex(tok(ColorToken::Text)),
            hex(tok(ColorToken::LabelText)),
            hex(button.fill.unwrap()),
            hex(on.fill.unwrap()),
            hex(off.border),
            hex(tok(ColorToken::SliderFill)),
            hex(tok(ColorToken::SliderKnob)),
            hex(tok(ColorToken::Border)),
        );
    }

    /// Liquid Glass paints no toggles, so a RadioButton keeps its `(●)`/`( )`
    /// caption glyph and a slider rail keeps its frost — the whole of R21 for
    /// this change in one assertion.
    #[test]
    fn liquid_glass_keeps_its_glyph_radio_and_frosted_rail() {
        let g = LiquidGlassTheme;
        assert!(
            g.surface(SurfaceRole::Toggle, SurfaceState { selected: false, on: true })
                .is_none(),
            "no Toggle surface ⇒ the radio glyph and the Input well are untouched"
        );
        assert!(
            g.token(ColorToken::SliderFill).is_none(),
            "no filled range ⇒ the rail is drawn exactly as it always was"
        );
        assert!(g.token(ColorToken::LabelText).is_none());
        assert!(g.token(ColorToken::SliderKnob).is_none());
    }

    /// 050 AC7 — registering a theme touches **no painter**.
    ///
    /// The whole point of the trait. A throwaway third theme renders a fixture
    /// with zero edits to any painting site — which was impossible when eleven
    /// sites asked "is it Elegance?".
    #[test]
    fn registering_a_theme_touches_no_painter() {
        #[derive(Debug)]
        struct TestTheme;
        impl SurfaceTheme for TestTheme {
            fn id(&self) -> &str {
                "test-theme"
            }
            fn is_self_contained(&self) -> bool {
                true
            }
            fn surface(&self, role: SurfaceRole, _st: SurfaceState) -> Option<SurfaceSpec> {
                Some(SurfaceSpec {
                    fill: match role {
                        SurfaceRole::Shape | SurfaceRole::Accent => None,
                        _ => Some(Color32::from_rgb(9, 99, 9)),
                    },
                    border: Color32::from_rgb(1, 2, 3),
                    border_width: 1.0,
                })
            }
            fn token(&self, _t: ColorToken) -> Option<Color32> {
                Some(Color32::from_rgb(7, 77, 7))
            }
        }

        let ctx = egui::Context::default();
        crate::paint::set_surface_theme(&ctx, Arc::new(TestTheme));

        let mut c = crate::model::Control::new("P", ControlType::Panel, 0, 0);
        c.rect = crate::model::Rect::new(10, 10, 120, 80);
        let mut input = egui::RawInput::default();
        input.screen_rect = Some(egui::Rect::from_min_size(
            egui::Pos2::ZERO,
            egui::Vec2::new(300.0, 200.0),
        ));
        let mut full = ctx.run_ui(input, |root_ui| {
            egui::CentralPanel::default()
                .frame(egui::Frame::NONE)
                .show_inside(root_ui, |ui| {
                    crate::paint::draw_control(
                        ui.painter(),
                        egui::Pos2::ZERO,
                        &c,
                        false,
                        true,
                        1.0,
                        1.0,
                        None,
                    );
                });
        });
        full.textures_delta.clear();

        fn find(s: &egui::Shape, want: Color32, hit: &mut bool) {
            match s {
                egui::Shape::Vec(v) => v.iter().for_each(|s| find(s, want, hit)),
                egui::Shape::Rect(r) => {
                    let f = r.fill;
                    if f.r() == want.r() && f.g() == want.g() && f.b() == want.b() {
                        *hit = true;
                    }
                }
                _ => {}
            }
        }
        let mut painted = false;
        for cs in &full.shapes {
            find(&cs.shape, Color32::from_rgb(9, 99, 9), &mut painted);
        }
        assert!(
            painted,
            "the third theme's card colour never reached the painter"
        );

        println!(
            "\n  050 AC7 — a third theme rendered through the shared painter.\n  \
             painter sites changed: 0/11 (the trait is the only contact surface)\n"
        );
    }

    /// The role mapping is spec 047's, moved wholesale: containers are cards,
    /// value-bearing controls are wells, a Button is the one accent face.
    #[test]
    fn role_mapping_is_unchanged() {
        use ControlType as CT;
        assert_eq!(role_for(&CT::Button), SurfaceRole::Button);
        for ct in [
            CT::TextBox,
            CT::ComboBox,
            CT::ListBox,
            CT::DataGrid,
            CT::TreeView,
            CT::NumericUpDown,
            CT::DateTimePicker,
            CT::Slider,
            CT::ProgressBar,
        ] {
            assert_eq!(role_for(&ct), SurfaceRole::Input, "{ct:?}");
        }
        for ct in [CT::Panel, CT::GroupBox, CT::PictureBox, CT::CheckBox] {
            assert_eq!(role_for(&ct), SurfaceRole::Card, "{ct:?}");
        }
        eprintln!("050 roles — 1 Button, 9 Input, everything else Card");
    }
}
