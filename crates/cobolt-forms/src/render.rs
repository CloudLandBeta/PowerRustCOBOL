// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Unified form rendering engine (spec 017).
//!
//! One renderer for **every** surface â the Form Designer canvas, the live
//! preview, the running (interpreted) form, and the compiled binary â so the same
//! form + state always produces the same pixels. The Form Designer's per-control
//! rendering (`paint::draw_control`) is the source of truth; this engine wraps it
//! with the shared form-level concerns (background, render order, container
//! clipping, ancestor opacity, tab visibility) that previously diverged across
//! four separate draw loops.
//!
//! Live values are supplied through the [`FormState`] trait so each caller plugs
//! in its own source (designer = the designed form, preview = a live-value map,
//! run = `CtrlState`, compiled = compiled state) without changing the engine.
//!
//! This module is the **Static** foundation (faces + form chrome). Interactive
//! widgets (editable text, combo popups, slider drag, â¦) layer on top in
//! `RenderMode::Interactive` and are added incrementally; in `Static` mode every
//! control is drawn as its designer face.

use std::collections::HashMap;

use egui::{pos2, Color32, Rect, Stroke, Vec2};

use crate::containers::{self, ActiveTabs};
use crate::datagrid::{
    datagrid_copy_text, DataGridCellSelection, DataGridColumnMeasure, DataGridLayout,
    DataGridLayoutInput,
};
use crate::model::{
    BgImageMode, DataGridAdvanced, DataGridGridLineStyle, PropValue, DATAGRID_ADVANCED_PROP,
};
use crate::{Control, ControlType};

/// Supplies live control state to the engine, source-agnostic.
///
/// The default implementations render the **designed** form unchanged (what the
/// designer wants). Callers with live state override [`FormState::live`] to merge
/// their values onto the base control before it is drawn.
pub trait FormState {
    /// Unique run ID for this form execution instance (used to clear anim clocks).
    fn run_id(&self) -> u64 {
        0
    }
    /// The control to actually draw: the designed `base` with any live overrides
    /// (text/value/checked, moved/resized geometry, SET-PROPERTY changes) applied.
    fn live(&self, base: &Control) -> Control {
        base.clone()
    }
    /// Whether the control is visible (COBOL may hide it). Default: visible.
    fn visible(&self, _base: &Control) -> bool {
        true
    }
    /// Whether the control is enabled. Default: enabled.
    fn enabled(&self, _base: &Control) -> bool {
        true
    }
    /// Per-control animation transform (preview/designer entrance effects): a
    /// pixel shift, a scale about the control centre, and an extra alpha on top
    /// of container opacity. Default = identity (no animation).
    fn transform(&self, _base: &Control) -> RenderTransform {
        RenderTransform::IDENTITY
    }
}

/// A live render transform for one control (animation). `dx`/`dy` shift it in
/// pixels, `scale` resizes it about its centre, and `alpha` multiplies its
/// opacity. The engine folds these into the on-screen rect + alpha so every
/// surface animates a control the same way.
#[derive(Clone, Copy, Debug)]
pub struct RenderTransform {
    pub dx: f32,
    pub dy: f32,
    pub scale: f32,
    pub alpha: f32,
}
impl RenderTransform {
    pub const IDENTITY: Self = Self {
        dx: 0.0,
        dy: 0.0,
        scale: 1.0,
        alpha: 1.0,
    };
}
impl Default for RenderTransform {
    fn default() -> Self {
        Self::IDENTITY
    }
}

/// A `FormState` that renders the designed form verbatim (the designer canvas).
/// The pan/zoom a `Maps` control is being driven to, between frames.
///
/// `CenterLat`/`CenterLng`/`Zoom` remain the published truth — COBOL reads and
/// writes them, and `onBoundsChanged` reports them. They are just not a good
/// place to accumulate a *gesture*: a property write goes out to the host and
/// comes back, so each frame's drag delta was being applied to a value one or
/// more frames old, and the map lagged and stuttered against the pointer.
///
/// So the gesture lives here and the properties are published from it. The one
/// subtlety is telling our own echo apart from a real write: `published` is the
/// exact text last sent, and [`Self::sync`] only surrenders the live view when
/// the property says something else — which is precisely when COBOL, a data
/// binding or the designer moved the map.
#[derive(Clone, Debug)]
pub struct MapView {
    pub lat: f64,
    pub lng: f64,
    pub zoom: u8,
    /// Zoom asked for but not yet applied, in levels — released a slice per
    /// frame by [`crate::map_tiles::zoom_glide`]. What makes one flick of the
    /// wheel glide to a stop instead of landing in a jump.
    pub zoom_accum: f32,
    /// How far the map is drawn from the whole level in `zoom`, in levels
    /// (`-0.5..=0.5`). The tiles come from `zoom`; this scales them, so the map
    /// can sit between levels rather than doubling in one frame.
    ///
    /// View state, not a property: `Zoom` stays the whole number a form stores
    /// and a handler reads.
    pub zoom_frac: f32,
    /// `(CenterLat, CenterLng, Zoom)` exactly as last published.
    pub published: (String, String, u8),
}

impl MapView {
    pub fn seeded(lat: f64, lng: f64, zoom: u8) -> Self {
        Self {
            lat,
            lng,
            zoom,
            zoom_accum: 0.0,
            zoom_frac: 0.0,
            published: (lat.to_string(), lng.to_string(), zoom),
        }
    }

    /// The zoom the map is DRAWN at — the whole level plus how far past it the
    /// glide has carried.
    pub fn zoom_at(&self) -> f32 {
        self.zoom as f32 + self.zoom_frac
    }

    /// Adopt the properties when they differ from what we last published —
    /// somebody else moved the map — and otherwise keep the live gesture.
    pub fn sync(&mut self, lat: f64, lng: f64, zoom: u8) {
        let outside_write = lat.to_string() != self.published.0
            || lng.to_string() != self.published.1
            || zoom != self.published.2;
        if outside_write {
            *self = Self::seeded(lat, lng, zoom);
        }
    }
}

pub struct DesignedState;
impl FormState for DesignedState {}

/// How the engine treats input: `Static` draws faces only (designer/snapshot);
/// `Interactive` also hosts editable widgets and returns events/updates.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum RenderMode {
    Static,
    Interactive,
}

/// Form background, owned by the engine so every surface shares the same rule.
pub struct Backdrop {
    /// Whether this surface paints a background AT ALL.
    ///
    /// `true` for a form, which owns its whole rectangle. `false` for a pass
    /// that draws controls into a rectangle **somebody else already painted** —
    /// the SideMenu's footer band, drawn on the rail. Such a pass painted the
    /// default navy over the rail, so a footer Panel at 100 % transparency
    /// showed a black block instead of the rail behind it (operator,
    /// 2026-08-22). `color_hex`/`transparency` are then not what to paint but
    /// what IS behind, so a translucent control still has something to resolve
    /// against. Use [`Backdrop::behind`].
    pub paint: bool,
    /// Form background colour as `#RRGGBB[AA]` (or empty/unset).
    pub color_hex: String,
    /// Form transparency 0â100 (0 = opaque).
    pub transparency: u8,
    pub gradient_enabled: bool,
    pub gradient_start_hex: String,
    pub gradient_end_hex: String,
    pub gradient_direction: String,
    /// Optional background image, already resolved to a texture by the caller
    /// (the engine has no texture cache), plus its pixel size.
    pub image: Option<(egui::TextureId, Vec2)>,
    pub image_mode: BgImageMode,
    /// The form's `UseThemeBackground` opt-in (007 R8). When set and the active
    /// theme pack provides a background, the pack's art replaces the form's own
    /// background image â exactly as on the designer canvas.
    pub use_theme_background: bool,
    /// The host window's client size, when the backdrop belongs to a real
    /// window the user can maximize or drag bigger. The backdrop then covers
    /// `max(form_size, window_size)` on each axis: the gradient or background
    /// image stretches across the WHOLE window when it is enlarged, while the
    /// controls stay at their designed size â and a window dragged SMALLER
    /// than the form keeps a form-sized backdrop rather than shrinking with
    /// the window. `None` (designer canvas, previews) pins the backdrop to
    /// the form, so the designed extent stays visible while editing.
    pub window_size: Option<Vec2>,
}

impl Backdrop {
    /// A surface that paints NOTHING because something else already painted it
    /// — `behind` being what that something put there, for controls whose own
    /// colour is translucent and cannot be resolved without it.
    pub fn behind(behind: Color32) -> Self {
        Backdrop {
            paint: false,
            color_hex: format!(
                "#{:02X}{:02X}{:02X}",
                behind.r(),
                behind.g(),
                behind.b()
            ),
            ..Default::default()
        }
    }
}

impl Default for Backdrop {
    fn default() -> Self {
        Backdrop {
            paint: true,
            color_hex: String::new(),
            transparency: 0,
            gradient_enabled: false,
            gradient_start_hex: String::new(),
            gradient_end_hex: String::new(),
            gradient_direction: "South".into(),
            image: None,
            image_mode: BgImageMode::Fit,
            use_theme_background: false,
            window_size: None,
        }
    }
}

/// All inputs to one form render.
pub struct RenderInput<'a> {
    /// The designed controls (flat list with parent/tab links).
    pub controls: &'a [Control],
    /// Live state source.
    pub state: &'a dyn FormState,
    /// Form size in form-space pixels (the backdrop fills this from the origin).
    pub form_size: Vec2,
    /// Liquid-Glass look on/off (mirrors the designer's glass toggle).
    pub glass: bool,
    /// Static vs interactive.
    pub mode: RenderMode,
    /// Active tab page per `TabControl` (for tab-scoped visibility).
    pub active_tabs: &'a ActiveTabs,
    /// Form background.
    pub backdrop: Backdrop,
}

/// Chrome the HOST paints between the form's backdrop and its controls, given
/// the painter and the form's rect.
///
/// The shell's breadcrumb frame is the one user: it is chrome, so it must sit
/// ON the form's background rather than under it, and it is NOT a container,
/// so a control the developer placed over that band has to paint on top of it.
/// That is exactly one slot in the paint order, and only the host knows what
/// goes in it.
pub type ChromeUnderControls<'a> = &'a dyn Fn(&egui::Painter, Rect);

/// Backend-agnostic hook the face-render walk calls so the host can clip a rounded
/// container's children to its rounded arc. egui only axis-aligns clip rects, so a
/// child's corner otherwise bleeds past the container's rounded corner (spec 017).
/// The default (`None`) keeps the legacy flat notch-mask behaviour; the IDE supplies
/// a GL implementation that captures the real backdrop + shadow behind each rounded
/// container and re-blits it through a rounded mask.
pub trait RoundedClipHook {
    /// Called right after a rounded container's own face + shadow are painted and
    /// before any of its children, with the container id, screen rect and radius.
    fn on_container(&self, painter: &egui::Painter, id: &str, rect: egui::Rect, radius: f32);
    /// Called once after the whole subtree is painted, to apply/flush the clip.
    fn finish(&self, painter: &egui::Painter);
}

/// A UI event emitted by an interactive control. Neutral (no `cobolt-runtime`
/// dependency); callers map it to their event type.
#[derive(Clone, Debug)]
pub struct UiEvent {
    pub ctrl_id: String,
    pub event: String,
    pub value: Option<String>,
}

/// What the engine produces for the caller to act on.
#[derive(Default)]
pub struct RenderOutput {
    /// UI events from interactive controls (clicks, changes, focus, keys, â¦).
    pub events: Vec<UiEvent>,
    /// Live property updates to apply back to the caller's state: (id, key, value).
    pub prop_updates: Vec<(String, String, String)>,
    /// Each control's on-screen rect, so the designer can position its overlay
    /// (selection handles, badges, drop hints) without re-deriving geometry.
    pub control_rects: HashMap<String, Rect>,
    /// Control ids whose `FileDropZone` was clicked (not dragged) this frame
    /// (spec 039 T4). `cobolt-forms` has no native-dialog dependency by
    /// design â the host (`cobolt-ide`) is expected to open a picker for
    /// each id here and feed the chosen paths back as an ordinary
    /// `DroppedFiles` prop write, the same channel the OS drag-drop path
    /// already uses.
    pub file_picker_requests: Vec<String>,
    /// Toolbar buttons pressed this frame whose action the PLATFORM must carry
    /// out â printing, sharing, a screenshot, the clipboard, another process.
    /// `(toolbar control id, button id, action string)`.
    ///
    /// Same division of labour as `file_picker_requests`: `cobolt-forms` knows
    /// which button was pressed and what it asked for, and takes no dependency
    /// on a print panel, a share sheet or a process launcher to find out. The
    /// host does the deed. A button whose action is the form's own business
    /// (`event`, `procedure:`, `open-modal:`) never appears here â it goes out as
    /// an ordinary `UiEvent` instead.
    pub toolbar_actions: Vec<(String, String, String)>,
}

/// The size the backdrop covers: the form's own size, stretched to the host
/// window on each axis where the window is BIGGER (maximized, or the border
/// dragged out â the gradient or background image then fills the whole
/// window while the controls keep their designed size), and never smaller
/// than the form (a window dragged in keeps a form-sized backdrop, which the
/// form scrolls inside). `None` â the designer canvas and previews â pins the
/// backdrop to the form so its designed extent stays visible while editing.
pub fn backdrop_size(form_size: Vec2, window_size: Option<Vec2>) -> Vec2 {
    window_size.map_or(form_size, |w| form_size.max(w))
}

/// What the backdrop pass painted, so the caller can reuse it â the
/// corner-notch mask repaints the very same background behind a rounded
/// container's children (spec 017).
pub struct BackdropPaint {
    /// The resolved solid background colour.
    pub bg: Color32,
    /// Gradient endpoint colours, when the form has one.
    pub gradient: Option<(Color32, Color32)>,
    /// True when the theme pack's art replaced the form's own image.
    pub themed: bool,
    /// The form's background image and the rect it was drawn into.
    pub image: Option<(egui::TextureId, Rect)>,
    /// Image alpha derived from the form's transparency.
    pub image_alpha: u8,
}

/// Paint a form's background into `rect`: solid colour, then the gradient,
/// then the theme pack's art or the form's own image.
///
/// ONE implementation, so every surface shows the same backdrop â the
/// designer, the preview, the running form, a compiled binary AND the static
/// face a window effect animates. The effect face used to paint the solid
/// colour only, so a form with a gradient or a background image was revealed
/// bare and then jumped to its real background the moment the animation
/// handed over to the live UI (operator report, 2026-07-30).
pub fn paint_backdrop(painter: &egui::Painter, rect: Rect, backdrop: &Backdrop) -> BackdropPaint {
    let bg = themed_backdrop_color(
        painter.ctx(),
        &backdrop.color_hex,
        backdrop.transparency,
    );
    // Somebody else owns this rectangle (see `Backdrop::behind`). Report what
    // is behind so a translucent control can resolve against it, and paint
    // nothing over it — not even "transparent", which an unset colour is NOT:
    // `backdrop_color` floors an unset one at alpha 200 by design, which is how
    // a footer band came out black.
    if !backdrop.paint {
        return BackdropPaint {
            bg,
            gradient: None,
            themed: false,
            image: None,
            image_alpha: 0,
        };
    }
    painter.rect_filled(rect, 0.0, bg);

    let gradient = if backdrop.gradient_enabled {
        let start = backdrop_gradient_color(&backdrop.gradient_start_hex, backdrop.transparency);
        let end = backdrop_gradient_color(&backdrop.gradient_end_hex, backdrop.transparency);
        painter.add(egui::Shape::mesh(crate::paint::background_gradient_mesh(
            rect,
            start,
            end,
            &backdrop.gradient_direction,
            egui::CornerRadius::ZERO,
        )));
        Some((start, end))
    } else {
        None
    };

    let alpha_mul = (100 - backdrop.transparency.min(100)) as f32 / 100.0;
    let image_alpha = (alpha_mul * 255.0) as u8;
    // Themed background (007 R8): when the form opts in and the active pack
    // provides one, the pack's art replaces the form's own image. Same call,
    // same order and same "themed wins" rule as the designer canvas.
    let themed = crate::paint::draw_theme_background(
        painter,
        rect,
        backdrop.use_theme_background,
        alpha_mul,
    );
    let image = backdrop.image.filter(|_| !themed).map(|(tex, tsize)| {
        let dest = image_dest(rect, tsize, backdrop.image_mode);
        let uv = Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0));
        painter
            .with_clip_rect(rect)
            .image(tex, dest, uv, Color32::from_white_alpha(image_alpha));
        (tex, dest)
    });

    BackdropPaint {
        bg,
        gradient,
        themed,
        image,
        image_alpha,
    }
}

/// Resolve the form background colour, applying the shared rule used on every
/// surface: strip `#`, take the first 6 hex digits, and treat unset / pure black
/// as the default dark navy so a transparent form is still a visible window.
pub fn backdrop_color(color_hex: &str, transparency: u8) -> Color32 {
    let s = color_hex.trim();
    let s = s.strip_prefix('#').unwrap_or(s);
    let hex = if s.len() >= 6 { &s[..6] } else { s };
    let bg_alpha = (255.0 * (1.0 - transparency.min(100) as f32 / 100.0)) as u8;
    if hex.len() == 6 {
        let r = u8::from_str_radix(&hex[0..2], 16).unwrap_or(20);
        let g = u8::from_str_radix(&hex[2..4], 16).unwrap_or(22);
        let b = u8::from_str_radix(&hex[4..6], 16).unwrap_or(45);
        let (r, g, b) = if r == 0 && g == 0 && b == 0 {
            (20, 22, 45)
        } else {
            (r, g, b)
        };
        Color32::from_rgba_premultiplied(
            (r as f32 * bg_alpha as f32 / 255.0) as u8,
            (g as f32 * bg_alpha as f32 / 255.0) as u8,
            (b as f32 * bg_alpha as f32 / 255.0) as u8,
            bg_alpha,
        )
    } else {
        Color32::from_rgba_premultiplied(20, 22, 45, bg_alpha.max(200))
    }
}

/// Has the developer actually chosen a form background?
///
/// Empty, or six hex digits that are all zero â the same "unset" that
/// [`backdrop_color`] maps to its default navy.
fn form_background_unset(color_hex: &str) -> bool {
    let s = color_hex.trim();
    let s = s.strip_prefix('#').unwrap_or(s);
    let hex = if s.len() >= 6 { &s[..6] } else { s };
    if hex.len() != 6 {
        return true;
    }
    ["r", "g", "b"]
        .iter()
        .enumerate()
        .all(|(i, _)| u8::from_str_radix(&hex[i * 2..i * 2 + 2], 16).unwrap_or(1) == 0)
}

/// The form's backdrop, letting the active THEME supply the default when the
/// developer chose no colour (050).
///
/// [`backdrop_color`] is ctx-free â every surface and several tests call it
/// without one â so it cannot ask a theme anything. This is the paint-time
/// wrapper that can. A developer's own colour always wins (R9), and a theme that
/// offers nothing leaves the historical navy exactly where it was (R21).
fn themed_backdrop_color(ctx: &egui::Context, color_hex: &str, transparency: u8) -> Color32 {
    if form_background_unset(color_hex) {
        if let Some(c) =
            crate::paint::theme_token(ctx, crate::surface_theme::ColorToken::FormBackground)
        {
            let a = (255.0 * (1.0 - transparency.min(100) as f32 / 100.0)) as u8;
            return Color32::from_rgba_premultiplied(
                (c.r() as f32 * a as f32 / 255.0) as u8,
                (c.g() as f32 * a as f32 / 255.0) as u8,
                (c.b() as f32 * a as f32 / 255.0) as u8,
                a,
            );
        }
    }
    backdrop_color(color_hex, transparency)
}

fn backdrop_gradient_color(color_hex: &str, transparency: u8) -> Color32 {
    let color = crate::paint::parse_hex(color_hex).unwrap_or(Color32::TRANSPARENT);
    let alpha = color.a() as f32 * (1.0 - transparency.min(100) as f32 / 100.0);
    let scale = alpha / 255.0;
    Color32::from_rgba_premultiplied(
        (color.r() as f32 * scale) as u8,
        (color.g() as f32 * scale) as u8,
        (color.b() as f32 * scale) as u8,
        alpha as u8,
    )
}

/// Whether a control's drawn content (image, film, glass card, chart, â¦) should be
/// clipped to a rounded GroupBox/Panel parent's border so it never bleeds past the
/// container's rounded corner (spec 017). True for every visual control; the
/// non-visual config objects (Timer/Agent/Sql/Rest) draw nothing that can bleed.
fn clips_to_container_border(ct: &ControlType) -> bool {
    !matches!(
        ct,
        ControlType::Timer
            | ControlType::AgentObject
            | ControlType::SqlDatabase
            | ControlType::RestClient
    )
}

/// Border path of a control's immediate rounded GroupBox/Panel parent, in screen
/// pixels: the parent's **visual** rect (its actual border, not the inset content
/// area) and its corner radius. A child is clipped to this shape, so any part that
/// exceeds the parent's border is cut by the parent â not by the child's own bounds
/// (spec 017, the container-clip rule). `None` when the parent isn't rounded.
fn picturebox_container_border(
    controls: &[Control],
    state: &dyn FormState,
    idx: usize,
    origin: egui::Pos2,
    scroll: egui::Vec2,
) -> Option<(Rect, f32)> {
    // Use the caller's effective controls list (may be the expanded list from
    // live+expand_repeating_groups in render_form). This prevents OOB when
    // instanced members (after databound ControlArray expansion) are indexed
    // and their (instanced) parents must be looked up in the same list.
    let parent_id = controls.get(idx).and_then(|c| c.parent.as_ref())?;
    let parent = controls.iter().find(|c| &c.id == parent_id)?;
    if !matches!(
        parent.control_type,
        ControlType::GroupBox | ControlType::Panel
    ) {
        return None;
    }
    let plive = state.live(parent);
    let rad = crate::paint::corner_radius(&plive);
    if rad < 0.5 {
        return None;
    }
    let v = plive.rect; // visual (border) rect in form coords
                        // If the immediate parent itself is the scroller (HScroll/VScroll), its
                        // border rect stays fixed on screen; only subtract scroll for non-scroller
                        // ancestors (e.g. a rounded GroupBox card that lives inside a scrolling
                        // Panel). This keeps _ContainerClip correct for PictureBox children both
                        // directly under a scroll panel and deep inside databound repeating cards.
    let parent_has_scroll = matches!(parent.control_type, ControlType::Panel)
        && (plive.get_prop("HScroll").map_or(false, |vv| vv.as_bool())
            || plive.get_prop("VScroll").map_or(false, |vv| vv.as_bool()));
    let off = if parent_has_scroll {
        egui::Vec2::ZERO
    } else {
        scroll
    };
    let border = Rect::from_min_max(
        origin + Vec2::new(v.x as f32, v.y as f32) - off,
        origin + Vec2::new((v.x + v.w) as f32, (v.y + v.h) as f32) - off,
    );
    Some((border, rad))
}

/// `_ContainerClip` descriptor string for `draw_control`: the parent border rect,
/// radius, and an all-corners-roundable flag set (every corner of a rounded
/// container border is rounded; `draw_control` still only rounds the corners the
/// image actually reaches).
fn container_clip_prop(border: Rect, rad: f32) -> String {
    format!(
        "{},{},{},{},{},1,1,1,1",
        border.min.x, border.min.y, border.max.x, border.max.y, rad
    )
}

/// After all controls are painted, repaint each rounded GroupBox/Panel's four
/// corner notches with the backdrop, covering any child content that bled past the
/// rounded arc (egui can only clip to axis-aligned rects). `bg` is the solid
/// backdrop colour and `image` the optional backdrop texture + its screen rect.
/// Per-corner rounding for [`crate::paint::draw_container_notch_mask`].
///
/// ââ CORNER GUARDIAN RULE (do not regress) âââââââââââââââââââââââââââââââââââââ
/// The notch mask exists ONLY to cut child content that bled past a rounded
/// corner. It must therefore touch a corner **only when a descendant actually
/// overlaps that corner's notch square** â never "all four corners because the
/// container happens to have children". Painting the backdrop over a corner no
/// child reaches destroys the container's OWN rounded corner (fill / rim / shadow),
/// which shows up as a transparent or discoloured crescent â the exact bug this
/// function was added to prevent. See `corner_notch_guardian_*` regression tests.
///
/// So: keep a corner's radius only when some descendant of `container_idx` overlaps
/// its notch square; otherwise zero it, leaving that corner untouched. When every
/// corner is clean the returned rounding is `ZERO` and the mask early-returns.
///
/// Both notch-mask call sites (runtime `mask_container_notches` and the designer's
/// notch loop) MUST route through this â do not call `draw_container_notch_mask`
/// with a blanket `CornerRadius::same(rad)`.
/// Does control `idx` need its corner notches repainted, and on which corners?
///
/// `None` means "no mask" — the answer for most controls. This is the ONE place
/// that decides, because the rule lives at two call sites (the run/preview
/// renderer and the designer canvas) and a rule implemented twice is a rule
/// that will disagree with itself.
///
/// Two kinds of control paint past their own arc, for the same reason — egui
/// clips to axis-aligned rects only:
///
/// * a **rounded container**, through its children. Only the corners a
///   descendant actually reaches are masked; a clean corner masked anyway loses
///   the container's own arc to a backdrop-coloured crescent.
/// * a **Maps** control, through its own tiles. It has no children and would
///   never qualify above, which is why a map with `CornerRadius` set drew
///   square corners inside a rounded selection outline (operator, 2026-08-21).
///   Its tiles cover the whole face, so **every** corner is genuinely reached
///   and the blanket radius is the correct answer here rather than the mistake
///   the per-corner guardian exists to prevent.
///
/// A **nested** control is skipped in both cases: its notches must reveal the
/// parent surface, and this mask can only repaint the form backdrop — which
/// would cut a hole through the parent panel.
pub fn notch_mask_rounding(
    controls: &[Control],
    idx: usize,
    rect: Rect,
    radius: f32,
    control_rects: &HashMap<String, Rect>,
) -> Option<egui::CornerRadius> {
    let ctrl = controls.get(idx)?;
    if radius < 0.5 || ctrl.parent.is_some() {
        return None;
    }
    match ctrl.control_type {
        ControlType::Maps => Some(egui::CornerRadius::same(crate::paint::cr8(radius))),
        ControlType::GroupBox | ControlType::Panel => containers::has_descendants(controls, idx)
            .then(|| corner_notch_rounding(rect, radius, controls, idx, control_rects)),
        _ => None,
    }
}

pub fn corner_notch_rounding(
    container: Rect,
    radius: f32,
    controls: &[Control],
    container_idx: usize,
    control_rects: &HashMap<String, Rect>,
) -> egui::CornerRadius {
    let r = radius.max(0.0);
    if r < 0.5 {
        return egui::CornerRadius::ZERO;
    }
    let child_rects: Vec<Rect> = containers::collect_descendants(controls, container_idx)
        .into_iter()
        .filter_map(|d| {
            controls
                .get(d)
                .and_then(|c| control_rects.get(&c.id))
                .copied()
        })
        .collect();
    let corner = |x: f32, y: f32| Rect::from_min_size(pos2(x, y), Vec2::new(r, r));
    let hit = |sq: Rect| child_rects.iter().any(|cr| cr.intersects(sq));
    egui::CornerRadius {
        nw: if hit(corner(container.min.x, container.min.y)) {
            crate::paint::cr8(r)
        } else {
            0
        },
        ne: if hit(corner(container.max.x - r, container.min.y)) {
            crate::paint::cr8(r)
        } else {
            0
        },
        se: if hit(corner(container.max.x - r, container.max.y - r)) {
            crate::paint::cr8(r)
        } else {
            0
        },
        sw: if hit(corner(container.min.x, container.max.y - r)) {
            crate::paint::cr8(r)
        } else {
            0
        },
    }
}

#[allow(clippy::too_many_arguments)]
fn mask_container_notches(
    painter: &egui::Painter,
    input: &RenderInput<'_>,
    controls: &[Control],
    out: &RenderOutput,
    image: Option<(egui::TextureId, Rect)>,
    img_alpha: u8,
    bg: Color32,
    gradient: Option<(Rect, Color32, Color32, &str)>,
    // The alpha each control was DRAWN with (ancestor opacity × animation ×
    // enabled), collected by the render loop. It cannot be recomputed here: the
    // animation transform only exists inside that loop.
    alphas: &HashMap<String, f32>,
) {
    // `controls` is the EFFECTIVE (post-`expand_repeating_groups`) list the render
    // loop drew from â NOT `input.controls`. The notch-mask guardian
    // (`corner_notch_rounding`) decides which corners to mask by looking each
    // descendant's rect up in `out.control_rects`, which is keyed by the drawn
    // (instance) ids. Walking the original template here would leave a databound
    // container's expanded card instances invisible to the guardian, so it would
    // mask nothing and the card content bleeds past the container arc (spec 015/024
    // repeating groups Ã the spec 017 notch mask).
    for (idx, base) in controls.iter().enumerate() {
        // Which control types can need a mask is `notch_mask_rounding`'s to
        // decide — this loop only skips what is not on screen.
        if !input.state.visible(base) || !containers::is_visible(controls, idx, input.active_tabs) {
            continue;
        }
        let live = input.state.live(base);
        let rad = crate::paint::corner_radius(&live);
        let Some(&screen) = out.control_rects.get(&live.id) else {
            continue;
        };
        // One rule, both call sites: which corners need repainting, if any.
        let Some(rounding) =
            notch_mask_rounding(controls, idx, screen, rad, &out.control_rects)
        else {
            continue;
        };
        // The backdrop is not everything behind this control: its own drop shadow
        // (or Neumorphic halo) is painted there too and shows through the notch.
        // Repainting the flat backdrop alone erased it inside the bbox while it
        // survived outside — the grey wedge at every rounded corner of a Maps
        // control (operator, 2026-08-21). The alpha it was DRAWN with, not 1.0:
        // restoring a full-strength shadow behind a faded or animating control
        // would trade one wedge for another.
        let stack = crate::paint::control_shadow_stack(
            painter.ctx(),
            &live,
            screen,
            alphas.get(&live.id).copied().unwrap_or(1.0),
        );
        crate::paint::draw_container_notch_mask(
            painter,
            screen,
            rounding,
            bg,
            gradient,
            image,
            img_alpha,
            (!stack.is_empty()).then_some(&stack),
        );
        // The notch mask repaints the backdrop over the corner arcs it touched,
        // erasing the container's own border/rim there. Restore the rim on exactly
        // those corners (`rounding`) â restoring an unmasked corner would
        // double-stroke the face's own rim and leave a light spur.
        crate::paint::restore_container_outline(
            painter,
            &live,
            screen,
            rad,
            input.glass,
            rounding,
        );
    }
}

fn draw_deferred_groupbox_captions(
    painter: &egui::Painter,
    input: &RenderInput<'_>,
    out: &RenderOutput,
) {
    for (idx, base) in input.controls.iter().enumerate() {
        if !matches!(base.control_type, ControlType::GroupBox) {
            continue;
        }
        if !input.state.visible(base)
            || !containers::is_visible(input.controls, idx, input.active_tabs)
        {
            continue;
        }
        let live = input.state.live(base);
        let Some(&screen) = out.control_rects.get(&live.id) else {
            continue;
        };
        let tf = input.state.transform(base);
        let enabled = input.state.enabled(base);
        let alpha = containers::ancestor_opacity(input.controls, idx)
            * tf.alpha
            * if enabled { 1.0 } else { 0.45 };
        let mut face = live.clone();
        face.rect = crate::model::Rect::new(
            0,
            0,
            screen.width().round() as i32,
            screen.height().round() as i32,
        );
        crate::paint::draw_groupbox_caption(painter, screen.min, &face, alpha);
    }
}

fn draw_deferred_tabcontrol_tabs(
    painter: &egui::Painter,
    input: &RenderInput<'_>,
    out: &RenderOutput,
) {
    for (idx, base) in input.controls.iter().enumerate() {
        if !matches!(base.control_type, ControlType::TabControl) {
            continue;
        }
        if !input.state.visible(base)
            || !containers::is_visible(input.controls, idx, input.active_tabs)
        {
            continue;
        }
        let live = input.state.live(base);
        let Some(&screen) = out.control_rects.get(&live.id) else {
            continue;
        };
        let tf = input.state.transform(base);
        let enabled = input.state.enabled(base);
        let alpha = containers::ancestor_opacity(input.controls, idx)
            * tf.alpha
            * if enabled { 1.0 } else { 0.45 };
        let mut face = live.clone();
        face.rect = crate::model::Rect::new(
            0,
            0,
            screen.width().round() as i32,
            screen.height().round() as i32,
        );
        crate::paint::draw_tabcontrol_tabs(painter, screen.min, &face, alpha);
    }
}

/// A GroupBox marked as a repeating group (spec 015 control array).
/// It may be placed inside other containers (parent may be set).
fn is_repeating_instance_group(c: &Control) -> bool {
    matches!(c.control_type, ControlType::GroupBox)
        && c.get_prop("IsRepeatingGroup")
            .map(|v| v.as_bool())
            .unwrap_or(false)
}

/// How many runtime instances a repeating group renders.
///
/// A **databound** group (its `DataSource` is set) treats `ItemCount` as
/// authoritative â including **0**, which renders NO card at all (an empty data
/// source shows nothing; task 3). An **unbound** template group falls back to
/// `PreviewItemCount` (clamped â¥1) so the designer always has one card to edit.
fn repeating_instance_count(c: &Control) -> usize {
    let bound = c
        .get_prop("DataSource")
        .map(|v| !v.as_str().trim().is_empty())
        .unwrap_or(false);
    if bound {
        c.get_prop("ItemCount")
            .map(|v| v.as_i64())
            .unwrap_or(0)
            .clamp(0, 500) as usize
    } else {
        c.get_prop("PreviewItemCount")
            .map(|v| v.as_i64())
            .unwrap_or(1)
            .clamp(1, 500) as usize
    }
}

/// Id of the `inst`-th (**1-based**) clone of repeating group `group_id`:
/// `"<group>.<group>-<inst>"`. Every instance â including the first â is prefixed,
/// so a member's runtime id can never collide with the designed base id or with a
/// same-named member of a different group (task 1).
pub fn group_instance_id(group_id: &str, inst: usize) -> String {
    format!("{group_id}.{group_id}-{inst}")
}

/// Id of member `member_id` inside the `inst`-th clone of `group_id`:
/// `"<group>.<group>-<inst>.<member>"`.
pub fn member_instance_id(group_id: &str, member_id: &str, inst: usize) -> String {
    format!("{group_id}.{group_id}-{inst}.{member_id}")
}

/// How each card of a repeating group appears as its row binds (`PlacementEffect`).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum PlacementEffect {
    /// Cards appear instantly at their final spot as the data binds.
    None,
    /// All cards start stacked on the first card, then deal out to their final
    /// spots one after another. A card whose final spot is off-screen is placed
    /// there instantly (no phantom fly-in).
    Deal,
    /// Each card fades in (200 ms) at its final spot, one after the previous.
    FadeIn,
    /// Each card starts smaller at its final spot, then elastically zooms to
    /// normal size around the card group's centre.
    ZoomIn,
    /// Each card starts larger at its final spot, then elastically settles to
    /// normal size around the card group's centre.
    ZoomOut,
}

impl PlacementEffect {
    pub fn parse(s: &str) -> Self {
        match s.trim().to_ascii_lowercase().as_str() {
            "deal" => Self::Deal,
            "fadein" | "fade-in" | "fade in" => Self::FadeIn,
            "zoomin" | "zoom-in" | "zoom in" => Self::ZoomIn,
            "zoomout" | "zoom-out" | "zoom out" => Self::ZoomOut,
            _ => Self::None,
        }
    }
}

fn elastic_out(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    if t <= f32::EPSILON {
        0.0
    } else if (1.0 - t).abs() <= f32::EPSILON {
        1.0
    } else {
        let c4 = (2.0 * std::f32::consts::PI) / 3.0;
        2.0_f32.powf(-10.0 * t) * ((t * 10.0 - 0.75) * c4).sin() + 1.0
    }
}

/// Duration (seconds) of one card's appearance; cards animate sequentially so the
/// stagger between card `i` and `i+1` is also this long.
pub const CARD_APPEAR_DUR: f32 = 0.2; // default when CardAppearDuration not set

/// Fold a repeating-group card's appearance effect into its base transform `tf`.
/// Reads the `_Card*` metadata stamped by [`expand_repeating_groups`], derives the
/// per-group appear clock from egui memory (the ctx time the cards first showed),
/// and requests a repaint while the animation runs. `viewport` is the visible area
/// used for the off-screen (no-phantom/no-work) test. Non-card controls pass through.
fn apply_card_appear(
    base: &Control,
    tf: RenderTransform,
    final_screen: Rect,
    viewport: Rect,
    ctx: &egui::Context,
    state: &dyn FormState,
) -> RenderTransform {
    let effect = match base.get_prop("_CardEffect") {
        Some(v) => PlacementEffect::parse(v.as_str()),
        None => return tf,
    };
    if effect == PlacementEffect::None {
        return tf;
    }
    let inst = base
        .get_prop("_CardInstance")
        .map(|v| v.as_i64())
        .unwrap_or(1)
        .max(1) as usize;
    let from = (
        base.get_prop("_CardFromDx")
            .map(|v| v.as_i64())
            .unwrap_or(0) as f32,
        base.get_prop("_CardFromDy")
            .map(|v| v.as_i64())
            .unwrap_or(0) as f32,
    );
    let group = base
        .get_prop("_CardGroup")
        .map(|v| v.as_str().to_owned())
        .unwrap_or_default();
    // Per-group clock: seconds since these cards first appeared. Stored in egui
    // memory keyed by the group id + the batch N (from expansion at bind time).
    // Including N ensures that RefreshBinding which changes ItemCount gets a
    // fresh clock so deployment effects replay on the recreated cards.
    let now = ctx.input(|i| i.time);
    let batch_n = base.get_prop("_CardN").map(|v| v.as_i64()).unwrap_or(0);
    let bind_seq = base
        .get_prop("_CardBindSeq")
        .map(|v| v.as_i64())
        .unwrap_or(0);
    let key = egui::Id::new((
        "card-appear-start",
        group,
        batch_n,
        bind_seq,
        state.run_id(),
    ));
    let start = ctx.memory_mut(|m| *m.data.get_temp_mut_or_insert_with(key, || now));
    let elapsed = (now - start).max(0.0) as f32;
    let card_screen = card_final_screen_rect(base, final_screen);
    // Placement effects skip animation for a card whose final spot is outside
    // the visible parent viewport. Partially visible cards still animate and are
    // clipped by the existing ancestor clip path.
    let clipped = !viewport.intersects(card_screen);
    let dur = base
        .get_prop("_CardDuration")
        .map(|v| v.as_i64() as f32 / 1000.0)
        .unwrap_or(CARD_APPEAR_DUR);
    let (mut card_tf, animating) = card_appear_transform(effect, inst, elapsed, from, clipped, dur);
    if card_tf.scale != 1.0 {
        let control_center = final_screen.center();
        let card_center = card_screen.center();
        let group_shift = (control_center - card_center) * (card_tf.scale - 1.0);
        card_tf.dx += group_shift.x;
        card_tf.dy += group_shift.y;
    }
    if animating {
        ctx.request_repaint();
    }
    RenderTransform {
        dx: tf.dx + card_tf.dx,
        dy: tf.dy + card_tf.dy,
        scale: tf.scale * card_tf.scale,
        alpha: tf.alpha * card_tf.alpha,
    }
}

fn card_final_screen_rect(base: &Control, final_screen: Rect) -> Rect {
    let root_x = match base.get_prop("_CardRootX") {
        Some(v) => v.as_i64() as f32,
        None => return final_screen,
    };
    let root_y = base
        .get_prop("_CardRootY")
        .map(|v| v.as_i64() as f32)
        .unwrap_or(base.rect.y as f32);
    let root_w = base
        .get_prop("_CardRootW")
        .map(|v| v.as_i64() as f32)
        .unwrap_or(final_screen.width());
    let root_h = base
        .get_prop("_CardRootH")
        .map(|v| v.as_i64() as f32)
        .unwrap_or(final_screen.height());
    let dx = root_x - base.rect.x as f32;
    let dy = root_y - base.rect.y as f32;
    Rect::from_min_size(
        final_screen.min + Vec2::new(dx, dy),
        Vec2::new(root_w.max(0.0), root_h.max(0.0)),
    )
}

/// The transform for one repeating-group card as it appears, plus whether it is
/// still animating (so the caller can keep requesting frames).
///
/// `inst` is 1-based; `elapsed` is seconds since the group's cards first appeared;
/// `from` is the vector from the card's FINAL screen position back to the first
/// card (used by Deal). `clipped` = the card's final spot is outside the visible
/// viewport, so effects skip their animation. Card `inst` animates during
/// `[(inst-1)Â·DUR, instÂ·DUR]`.
pub fn card_appear_transform(
    effect: PlacementEffect,
    inst: usize,
    elapsed: f32,
    from: (f32, f32),
    clipped: bool,
    dur: f32,
) -> (RenderTransform, bool) {
    let start = inst.saturating_sub(1) as f32 * dur;
    let local = if dur <= f32::EPSILON {
        1.0
    } else {
        ((elapsed - start) / dur).clamp(0.0, 1.0)
    };
    let done = elapsed >= start + dur;
    match effect {
        PlacementEffect::None => (RenderTransform::IDENTITY, false),
        PlacementEffect::FadeIn => {
            // Invisible until its turn, then fade 0â1 in place.
            let alpha = if elapsed < start { 0.0 } else { local };
            (
                RenderTransform {
                    alpha,
                    ..RenderTransform::IDENTITY
                },
                !done,
            )
        }
        PlacementEffect::Deal => {
            if clipped {
                // Off-screen: place at the final spot immediately (no phantom).
                (RenderTransform::IDENTITY, false)
            } else {
                // Smoothstep from the first-card position (factor 1) to final (0).
                let e = local * local * (3.0 - 2.0 * local);
                let f = 1.0 - e;
                (
                    RenderTransform {
                        dx: from.0 * f,
                        dy: from.1 * f,
                        ..RenderTransform::IDENTITY
                    },
                    !done,
                )
            }
        }
        PlacementEffect::ZoomIn => {
            if clipped {
                (RenderTransform::IDENTITY, false)
            } else {
                let eased = elastic_out(local);
                let scale = 0.65 + (1.0 - 0.65) * eased;
                (
                    RenderTransform {
                        scale,
                        ..RenderTransform::IDENTITY
                    },
                    !done,
                )
            }
        }
        PlacementEffect::ZoomOut => {
            if clipped {
                (RenderTransform::IDENTITY, false)
            } else {
                let eased = elastic_out(local);
                let scale = 1.25 + (1.0 - 1.25) * eased;
                (
                    RenderTransform {
                        scale,
                        ..RenderTransform::IDENTITY
                    },
                    !done,
                )
            }
        }
    }
}

/// Expand each top-level repeating GroupBox into its N runtime instances so the
/// shared render loop draws one card per row. The original template subtree is
/// removed and replaced by instances `1..=N`, each a clone shifted by the group's
/// layout (Vertical / Horizontal / Grid) and re-id'd under the group-prefixed
/// scheme. A databound group with 0 rows produces no instances at all. Returns
/// `None` when there is no repeating group to expand.
fn expand_repeating_groups(controls: &[Control]) -> Option<Vec<Control>> {
    let groups: Vec<usize> = (0..controls.len())
        .filter(|&i| is_repeating_instance_group(&controls[i]))
        .collect();
    if groups.is_empty() {
        return None;
    }
    // Indices belonging to ANY repeating group (the group + its descendants) â
    // these originals are dropped and re-emitted as numbered instances.
    let mut in_group: std::collections::HashSet<usize> = std::collections::HashSet::new();
    for &gi in &groups {
        in_group.insert(gi);
        for d in crate::containers::collect_descendants(controls, gi) {
            in_group.insert(d);
        }
    }
    // Every control that is NOT part of a repeating group stays as designed.
    let mut out: Vec<Control> = controls
        .iter()
        .enumerate()
        .filter(|(i, _)| !in_group.contains(i))
        .map(|(_, c)| c.clone())
        .collect();

    for &gi in &groups {
        let g = &controls[gi];
        let group_id = g.id.clone();
        let n = repeating_instance_count(g);
        let subtree: Vec<usize> = std::iter::once(gi)
            .chain(crate::containers::collect_descendants(controls, gi))
            .collect();
        if group_id.eq_ignore_ascii_case("GroupBox-2")
            || group_id.to_ascii_lowercase().contains("groupbox-2")
        {
            // debug was here during troubleshooting
        }
        let spacing = g
            .get_prop("ItemSpacing")
            .map(|v| v.as_i64())
            .unwrap_or(8)
            .max(0) as f32;
        let dir = g
            .get_prop("LayoutDirection")
            .map(|v| v.as_str().to_owned())
            .unwrap_or_else(|| "Vertical".into());
        let ipr = g
            .get_prop("ItemsPerRow")
            .map(|v| v.as_i64())
            .unwrap_or(1)
            .max(1) as usize;
        // Placement step per instance: the group's own size plus the item spacing
        // (task 2) â full HEIGHT+padding down for Vertical, full WIDTH+padding
        // across for Horizontal, and a Grid wraps every `ItemsPerRow`.
        let gw = g.rect.w as f32;
        let gh = g.rect.h as f32;
        // Card-appear effect (PlacementEffect). Stamped on every clone so the
        // render loop can animate the whole card; `None` skips stamping entirely.
        let effect = g
            .get_prop("PlacementEffect")
            .map(|v| v.as_str().to_owned())
            .unwrap_or_default();
        let animated = PlacementEffect::parse(&effect) != PlacementEffect::None;
        // Instances start at 1 (task 3). `n == 0` (empty databound source) skips
        // the loop entirely, so the group and its children never render.
        for inst in 1..=n {
            let step = (inst - 1) as f32;
            let (dx, dy) = match dir.as_str() {
                "Horizontal" => (step * (gw + spacing), 0.0),
                "Grid" => {
                    let col = ((inst - 1) % ipr) as f32;
                    let row = ((inst - 1) / ipr) as f32;
                    (col * (gw + spacing), row * (gh + spacing))
                }
                _ => (0.0, step * (gh + spacing)),
            };
            let reid = |orig: &str| -> String {
                if orig == group_id {
                    group_instance_id(&group_id, inst)
                } else {
                    member_instance_id(&group_id, orig, inst)
                }
            };

            // Collect the ids that belong to this subtree so we only remap parents
            // that pointed inside the original repeating group. External parents
            // (e.g. a Panel that contains the template GroupBox) must be kept as-is
            // so that the instanced cards remain children of the original parent
            // for clipping, visibility, render order, etc.
            let subtree_ids: std::collections::HashSet<&str> =
                subtree.iter().map(|&si| controls[si].id.as_str()).collect();

            for &si in &subtree {
                let mut clone = controls[si].clone();
                clone.rect.x += dx as i32;
                clone.rect.y += dy as i32;
                clone.id = reid(&controls[si].id);
                clone.parent = controls[si].parent.as_deref().map(|p| {
                    if subtree_ids.contains(p) {
                        reid(p)
                    } else {
                        p.to_string()
                    }
                });
                if animated {
                    let root_x = g.rect.x + dx as i32;
                    let root_y = g.rect.y + dy as i32;
                    clone.set_prop(
                        "_CardEffect",
                        crate::model::PropValue::String(effect.clone()),
                    );
                    clone.set_prop(
                        "_CardGroup",
                        crate::model::PropValue::String(group_id.clone()),
                    );
                    clone.set_prop("_CardInstance", crate::model::PropValue::Int(inst as i64));
                    clone.set_prop("_CardRootX", crate::model::PropValue::Int(root_x as i64));
                    clone.set_prop("_CardRootY", crate::model::PropValue::Int(root_y as i64));
                    clone.set_prop("_CardRootW", crate::model::PropValue::Int(g.rect.w as i64));
                    clone.set_prop("_CardRootH", crate::model::PropValue::Int(g.rect.h as i64));
                    // The card starts stacked on the first card (Deal), so the
                    // "from" vector is the delta from its final spot back there.
                    clone.set_prop("_CardFromDx", crate::model::PropValue::Int(-(dx as i64)));
                    clone.set_prop("_CardFromDy", crate::model::PropValue::Int(-(dy as i64)));
                    clone.set_prop("_CardN", crate::model::PropValue::Int(n as i64));
                    // Bind seq (bumped on each RefreshBinding) makes the clock key unique
                    // so effects replay even for same ItemCount.
                    if let Some(seqv) = g.get_prop("_BindSeq") {
                        clone.set_prop("_CardBindSeq", seqv.clone());
                    }
                    // Duration from property (ms), for Deal/FadeIn.
                    let dur_ms = g
                        .get_prop("CardAppearDuration")
                        .map(|v| v.as_i64())
                        .unwrap_or(200);
                    clone.set_prop("_CardDuration", crate::model::PropValue::Int(dur_ms));
                }
                out.push(clone);
            }
        }
        if group_id.eq_ignore_ascii_case("GroupBox-2")
            || group_id.to_ascii_lowercase().contains("groupbox-2")
        {
            // debug was here during troubleshooting
        }
    }
    Some(out)
}

fn ancestor_auto_scroll_offset(
    controls: &[Control],
    idx: usize,
    ctx: &egui::Context,
) -> egui::Vec2 {
    let mut off = egui::Vec2::ZERO;
    let mut cur = idx;
    while let Some(pid) = controls[cur].parent.clone() {
        if let Some(p) = controls.iter().position(|c| c.id == pid) {
            let is_panel = matches!(controls[p].control_type, ControlType::Panel);
            let has_h = is_panel
                && controls[p]
                    .get_prop("HScroll")
                    .map_or(false, |v| v.as_bool());
            let has_v = is_panel
                && controls[p]
                    .get_prop("VScroll")
                    .map_or(false, |v| v.as_bool());
            if has_h || has_v {
                let sid = egui::Id::new(("autoscr", pid));
                if let Some(o) = ctx.data(|d| d.get_temp::<egui::Vec2>(sid)) {
                    off += o;
                }
            }
            cur = p;
        } else {
            break;
        }
    }
    off
}

/// Compute the ancestor content clip rect in *screen* coordinates for `idx`,
/// honouring ancestor HScroll/VScroll. Container rects contributed by scrolling
/// ancestors (e.g. the Panel) are placed at their fixed on-screen position.
/// Container rects for non-scroller ancestors that live in the scrolled content
/// space (e.g. a databound repeating GroupBox card inside a scrolling Panel) are
/// shifted by the cumulative scroll so that children draw inside the moved card
/// bounds. This fixes "frame transparency" / missing inners when scrolling
/// repeating GroupBoxes (ControlArrays) that act as databound card lists.
fn ancestor_clip_rect(
    controls: &[Control],
    idx: usize,
    origin: egui::Pos2,
    scroll: egui::Vec2,
    state: &dyn FormState,
) -> Option<Rect> {
    let mut clip: Option<Rect> = None;
    let mut cur = idx;
    // Start assuming we are in "content space" under scroll; once we cross a
    // scrolling container we stop shifting (higher clips are viewport-fixed).
    let mut apply_scroll = true;
    while let Some(pid) = controls.get(cur).and_then(|c| c.parent.clone()) {
        if let Some(p) = controls.iter().position(|c| c.id == pid) {
            let pctrl = &controls[p];
            let plive = state.live(pctrl);
            let cr = plive.content_rect();
            let has_scroll = matches!(pctrl.control_type, ControlType::Panel)
                && (plive.get_prop("HScroll").map_or(false, |v| v.as_bool())
                    || plive.get_prop("VScroll").map_or(false, |v| v.as_bool()));
            let off = if apply_scroll && !has_scroll {
                scroll
            } else {
                egui::Vec2::ZERO
            };
            let r = Rect::from_min_size(
                origin + Vec2::new(cr.x as f32, cr.y as f32) - off,
                Vec2::new(cr.w as f32, cr.h as f32),
            );
            clip = Some(match clip {
                Some(c) => c.intersect(r),
                None => r,
            });
            if has_scroll {
                apply_scroll = false;
            }
            cur = p;
        } else {
            break;
        }
    }
    clip
}

fn panel_content_size(
    controls: &[Control],
    panel_idx: usize,
    panel_size: egui::Vec2,
) -> egui::Vec2 {
    let panel = &controls[panel_idx];
    let mut max_w = panel_size.x;
    let mut max_h = panel_size.y;
    for c in controls {
        let mut cur = c.parent.as_ref();
        let mut is_descendant = false;
        while let Some(pid) = cur {
            if pid.eq_ignore_ascii_case(&panel.id) {
                is_descendant = true;
                break;
            }
            if let Some(p) = controls.iter().find(|x| x.id.eq_ignore_ascii_case(pid)) {
                cur = p.parent.as_ref();
            } else {
                break;
            }
        }
        if is_descendant {
            max_w = max_w.max((c.rect.x + c.rect.w - panel.rect.x) as f32);
            max_h = max_h.max((c.rect.y + c.rect.h - panel.rect.y) as f32);
        }
    }
    egui::vec2(max_w, max_h)
}

/// Render a whole form into `ui` at its content origin. The caller sets up the
/// `CentralPanel` / `ScrollArea` and `ui.set_min_size(form_size)` first.
pub fn render_form(ui: &mut egui::Ui, input: &RenderInput<'_>) -> RenderOutput {
    render_form_with_chrome(ui, input, None)
}

/// [`render_form`], with one slot of host chrome painted between the backdrop
/// and the controls — see [`ChromeUnderControls`]. The shell renders through
/// this so its breadcrumb frame sits on the form's background while the
/// controls placed over that band still paint on top of it.
pub fn render_form_with_chrome(
    ui: &mut egui::Ui,
    input: &RenderInput<'_>,
    chrome: Option<ChromeUnderControls<'_>>,
) -> RenderOutput {
    let mut out = RenderOutput::default();
    // Each control's drawn alpha, for the corner-notch pass at the end of the
    // frame: it re-composites the control's own shadow and must use the alpha
    // that shadow was painted with, which only the control loop knows.
    let mut control_alphas: HashMap<String, f32> = HashMap::new();
    let origin = ui.min_rect().min;
    let painter = ui.painter().clone();

    // ââ Backdrop: solid colour, gradient, theme art or image. âââââââââââââââââ
    let form_rect = Rect::from_min_size(origin, input.form_size);
    // The backdrop covers the form, and stretches to the host window when the
    // user maximizes it or drags it bigger â the controls keep their designed
    // size, only the background follows the window. A window dragged SMALLER
    // than the form keeps the form-sized backdrop (the form scrolls inside
    // it) rather than cropping the background to the window.
    let backdrop_rect = Rect::from_min_size(
        origin,
        backdrop_size(input.form_size, input.backdrop.window_size),
    );
    let painted = paint_backdrop(&painter, backdrop_rect, &input.backdrop);
    let bg = painted.bg;
    // Publish it for the controls whose own background is translucent and so
    // cannot be resolved without knowing what is behind them (the SideMenu's
    // rail, spec 049).
    crate::paint::set_form_backdrop(ui.ctx(), bg);
    // The host's chrome band (the shell's breadcrumb frame): on the background,
    // under every control — a control drawn over it wins, because the frame is
    // chrome and not a container.
    if let Some(chrome) = chrome {
        chrome(&painter, form_rect);
    }
    let backdrop_gradient = painted.gradient;
    let backdrop_img_alpha = painted.image_alpha;
    let backdrop_img = painted.image;
    // The notch mask is drawn *after* children. If the form background is
    // translucent, repainting `bg` would darken the corner wedges; skipping it
    // would leave rectangular child bleed visible. Use the effective one-pass
    // colour over the panel fill instead.
    let notch_bg = crate::paint::composite_premultiplied_over(bg, ui.visuals().panel_fill);
    // ââ Controls: designer order, clipped + faded by container ancestry. ââââââ
    // Expand repeating groups (spec 015 / 024) into their N runtime instances so
    // the render loop below draws one card per item.
    // Use live state so that runtime-updated ItemCount / IsRepeatingGroup (e.g. from
    // RefreshBinding on a databound ControlArray) are seen for expansion.
    let live_controls: Vec<Control> = input.controls.iter().map(|c| input.state.live(c)).collect();
    let expanded = expand_repeating_groups(&live_controls);
    // (debug prints for repeating groups are now limited to databound ControlArrays
    // in the IDE layer)
    let controls: &[Control] = expanded.as_deref().unwrap_or(input.controls);
    let order = containers::render_order(controls);
    let interactive = input.mode == RenderMode::Interactive;
    // ComboBox dropdowns are drawn in a second pass so they float above every
    // other control. The Control itself is out of reach by then, so everything
    // the popup needs travels with it.
    let mut open_combos: Vec<OpenCombo> = Vec::new();
    let tab_focus_request = if interactive {
        apply_pending_tab_focus(ui);
        let mut tab_targets = collect_tab_targets(input, controls, &order);
        resolve_tab_traversal(ui, &mut tab_targets)
    } else {
        None
    };
    let default_button_click = if interactive {
        resolve_default_button_enter(input, controls, &order, ui)
    } else {
        None
    };
    for &idx in &order {
        let base = &controls[idx];
        if base
            .id
            .to_ascii_lowercase()
            .starts_with("groupbox-2.groupbox-2-")
            || base.id.eq_ignore_ascii_case("GroupBox-2")
        {
            // debug removed
        }
        // Visible/Enabled change events (spec 021 T9): tracked for EVERY
        // control each frame â a control hidden THIS frame must still fire
        // its onVisibleChanged â so this runs before the visibility skips.
        if interactive {
            visible_enabled_events(ui, input, controls, idx, &mut out);
        }
        if !input.state.visible(base) {
            continue;
        }
        if !containers::is_visible(controls, idx, input.active_tabs) {
            continue;
        }

        // Live control (designer source-of-truth face via draw_control).
        let live = input.state.live(base);
        let r = live.rect;

        // Apply ancestor AutoScroll offsets (if any) so children of a Panel with
        // AutoScroll=true are shifted, making the property actually scroll the
        // content.
        let scroll = ancestor_auto_scroll_offset(controls, idx, ui.ctx());

        // Animation transform: shift then scale about the control centre. Both
        // surfaces (preview now, designer later) supply entrance effects this way;
        // the default is identity (run / compiled / static designer).
        let tf = input.state.transform(base);
        // The card's FINAL (un-animated) screen rect â used to decide whether a
        // Deal card is off-screen (no phantom fly-in) before adding the effect.
        let final_screen = Rect::from_min_size(
            origin + Vec2::new(r.x as f32 + tf.dx - scroll.x, r.y as f32 + tf.dy - scroll.y),
            Vec2::new(r.w as f32, r.h as f32),
        );
        // Clip to ancestor container content areas (rounded clipping is cosmetic;
        // egui clips to the axis-aligned rect â spec 012/016). Start from the whole
        // form so a top-level control is never clipped to its own bounds.
        // Use scroll-aware placement so that clips contributed by non-scroller
        // containers (the instanced cards) move with -scroll while scroller
        // Panel clips stay fixed. Prevents the "growing transparent frame" over
        // databound card content on scroll.
        let clip = match ancestor_clip_rect(controls, idx, origin, scroll, input.state) {
            Some(c) => form_rect.intersect(c),
            None => form_rect,
        };
        // Fold the repeating-group card-appear effect into `tf`. The viewport is
        // the parent/container clip, so offscreen cards do not animate while
        // partially visible cards animate and remain clipped.
        let tf = apply_card_appear(base, tf, final_screen, clip, ui.ctx(), input.state);
        let base_screen = Rect::from_min_size(
            origin + Vec2::new(r.x as f32 + tf.dx - scroll.x, r.y as f32 + tf.dy - scroll.y),
            Vec2::new(r.w as f32, r.h as f32),
        );
        let screen = crate::paint::scale_rect_about_center(base_screen, tf.scale);
        out.control_rects.insert(live.id.clone(), screen);

        // Drive AutoScroll for Panels (interactive only). We show a ScrollArea at
        // the panel rect (with oversized content to enable bars/input). The
        // offset is stored in egui data and subtracted from descendant screens
        // above, so children appear scrolled inside the panel.
        if interactive && matches!(base.control_type, ControlType::Panel) {
            let hscroll = base.get_prop("HScroll").map_or(false, |v| v.as_bool());
            let vscroll = base.get_prop("VScroll").map_or(false, |v| v.as_bool());
            if hscroll || vscroll {
                let sid = egui::Id::new(("autoscr", &base.id));
                let overscroll_id = egui::Id::new(("overscroll", &base.id));

                // Read and decay overscroll
                let mut overscroll = ui
                    .data(|d| d.get_temp::<egui::Vec2>(overscroll_id))
                    .unwrap_or(egui::Vec2::ZERO);
                let dt = ui.input(|i| i.stable_dt).min(0.1);
                let decay = (-15.0 * dt).exp();
                overscroll *= decay;
                if overscroll.length() < 0.1 {
                    overscroll = egui::Vec2::ZERO;
                }

                let _ = ui.scope_builder(egui::UiBuilder::new().max_rect(screen), |ui| {
                    let sa = egui::ScrollArea::new([hscroll, vscroll])
                        .id_salt(sid)
                        .auto_shrink([false, false]);

                    let content_size = panel_content_size(controls, idx, screen.size());

                    let res = sa.show(ui, |ui| {
                        ui.set_min_size(content_size);
                    });

                    let offset = res.state.offset;
                    let max_scroll = (content_size - screen.size()).max(egui::Vec2::ZERO);

                    // Accumulate overscroll from input wheel delta when at boundaries
                    if ui.rect_contains_pointer(screen) {
                        let scroll_delta = ui.input(|i| i.smooth_scroll_delta);
                        if scroll_delta != egui::Vec2::ZERO {
                            // Check vertical boundaries
                            if vscroll {
                                if offset.y <= 0.0 && scroll_delta.y > 0.0 {
                                    overscroll.y =
                                        (overscroll.y + scroll_delta.y * 0.4).clamp(-40.0, 40.0);
                                } else if offset.y >= max_scroll.y && scroll_delta.y < 0.0 {
                                    overscroll.y =
                                        (overscroll.y + scroll_delta.y * 0.4).clamp(-40.0, 40.0);
                                }
                            }
                            // Check horizontal boundaries
                            if hscroll {
                                if offset.x <= 0.0 && scroll_delta.x > 0.0 {
                                    overscroll.x =
                                        (overscroll.x + scroll_delta.x * 0.4).clamp(-40.0, 40.0);
                                } else if offset.x >= max_scroll.x && scroll_delta.x < 0.0 {
                                    overscroll.x =
                                        (overscroll.x + scroll_delta.x * 0.4).clamp(-40.0, 40.0);
                                }
                            }
                        }
                    }

                    // Save new overscroll and request repaint if animating
                    if overscroll != egui::Vec2::ZERO {
                        ui.ctx().request_repaint();
                    }
                    ui.data_mut(|d| d.insert_temp(overscroll_id, overscroll));

                    let effective_scroll = offset - overscroll;
                    ui.ctx().data_mut(|d| d.insert_temp(sid, effective_scroll));

                    // spec 021 T12: Panel onScroll on offset change.
                    if base.events.iter().any(|e| e.event == "onScroll") {
                        let last_id = sid.with("last-offset");
                        let last = ui.data(|d| d.get_temp::<egui::Vec2>(last_id));
                        if let Some(last) = last {
                            if (last - offset).length() > 0.5 {
                                out.events.push(UiEvent::with_value(
                                    &base.id,
                                    "onScroll",
                                    &format!("{:.0},{:.0}", offset.y, offset.x),
                                ));
                            }
                        }
                        if last != Some(offset) {
                            ui.data_mut(|d| d.insert_temp(last_id, offset));
                        }
                    }

                    if ui.rect_contains_pointer(screen) {
                        ui.input_mut(|i| {
                            i.events
                                .retain(|event| !matches!(event, egui::Event::MouseWheel { .. }));
                            i.smooth_scroll_delta = egui::Vec2::ZERO;
                        });
                    }
                });
            }
        }

        // A PictureBox inside a rounded GroupBox/Panel is clipped to the parent's
        // BORDER path, so any overflow is cut by the container shape rather than the
        // image's own bounds (spec 017). The image is allowed to reach the parent's
        // border (not just its inset content area), so the clip widens to the border.
        let pic_border = if clips_to_container_border(&base.control_type) {
            picturebox_container_border(controls, input.state, idx, origin, scroll)
        } else {
            None
        };

        let anc = containers::ancestor_opacity(controls, idx);
        let enabled = input.state.enabled(base);
        let alpha = anc * tf.alpha * if enabled { 1.0 } else { 0.45 };
        // Kept for the corner-notch pass, which runs after this loop and cannot
        // recompute it — `tf` only exists here.
        control_alphas.insert(live.id.clone(), alpha);

        // Screen-normalised face: the live control re-based to the on-screen rect,
        // so a shifted/scaled animation draws at the transformed position+size.
        let mut face = live.clone();
        face.rect = crate::model::Rect::new(
            0,
            0,
            screen.width().round() as i32,
            screen.height().round() as i32,
        );

        if matches!(face.control_type, ControlType::GroupBox) {
            face.set_prop("_DeferCaption", PropValue::Bool(true));
        }

        if matches!(face.control_type, ControlType::TabControl) {
            face.set_prop("_DeferTabs", PropValue::Bool(true));
        }

        if let Some((border, rad)) = pic_border {
            face.set_prop("_ContainerClip", container_clip_prop(border, rad));
        }

        if interactive {
            // Live, editable widget: faces via `draw_control`, plus the interaction
            // (text edit, slider drag, combo popup, â¦) ported from the run path.
            render_interactive(
                ui,
                &face,
                screen,
                clip,
                input.glass,
                alpha,
                enabled,
                notch_bg,
                &mut out,
                &mut open_combos,
            );
        } else {
            // Static: the one true face renderer (charts, images, glass, rounding).
            // PictureBox needs its texture pre-loaded so `draw_control` paints the
            // image (not a placeholder) â same as the designer canvas.
            let pic_tex = if matches!(face.control_type, ControlType::PictureBox) {
                crate::paint::picturebox_texture(ui.ctx(), sv(&face, "ImagePath").trim())
                    .map(|t| t.id())
            } else {
                None
            };
            let dp = painter.with_clip_rect(painter.clip_rect().intersect(clip));
            crate::paint::draw_control(
                &dp,
                screen.min,
                &face,
                false,
                input.glass,
                alpha,
                1.0,
                pic_tex,
            );
        }
    }
    if let Some(focus_id) = tab_focus_request {
        ui.data_mut(|d| d.insert_temp(tab_pending_id(), Some(focus_id)));
    }
    if let Some(button_id) = default_button_click {
        out.events.push(UiEvent::click(&button_id));
    }

    // ââ Corner-notch masks: cut any child content that bled past a rounded
    // container's arc by repainting the backdrop in its corner notches (spec 017).
    mask_container_notches(
        &painter,
        input,
        controls,
        &out,
        backdrop_img,
        backdrop_img_alpha,
        notch_bg,
        backdrop_gradient.map(|(start, end)| {
            let panel = ui.visuals().panel_fill;
            (
                backdrop_rect,
                crate::paint::composite_premultiplied_over(start, panel),
                crate::paint::composite_premultiplied_over(end, panel),
                input.backdrop.gradient_direction.as_str(),
            )
        }),
        &control_alphas,
    );
    draw_deferred_groupbox_captions(&painter, input, &out);
    draw_deferred_tabcontrol_tabs(&painter, input, &out);

    clear_radio_group_siblings(input, controls, &mut out);

    // ââ Second pass: open ComboBox dropdowns float above everything. ââââââââââ
    for combo in open_combos {
        let OpenCombo {
            id: cid,
            items,
            header,
            current: cur,
            fills,
            face,
            item_h,
            font,
            text,
            max_h,
            reveal,
        } = combo;
        let gesture = ui
            .data(|d| d.get_temp(combo_gesture_id(&cid)))
            .unwrap_or_default();
        let highlight = ui
            .data(|d| d.get_temp(combo_highlight_id(&cid)))
            .unwrap_or(0);
        let outcome = crate::paint::glass_combo_popup(
            ui,
            crate::paint::ComboPopup {
                ctrl_id: &cid,
                header,
                items: &items,
                selected: &cur,
                highlight,
                gesture,
                fills,
                face,
                item_h,
                font,
                text,
                max_h,
                enabled: true,
                reveal,
            },
        );
        let open_id = rt_id(&cid).with("combo_open");
        ui.data_mut(|d| {
            d.insert_temp(combo_highlight_id(&cid), outcome.highlight);
            d.insert_temp(combo_gesture_id(&cid), outcome.gesture);
            // Picking from the list keeps the combo on the keyboard, so the
            // arrows carry on working straight after â the header pass dropped
            // it a moment ago, because the press was not on the header.
            if outcome.pressed_in_list {
                d.insert_temp(combo_keyboard_id(&cid), true);
            }
        });
        match outcome.action {
            Some(crate::paint::GlassComboAction::Select(idx, val)) => {
                out.prop_updates
                    .push((cid.clone(), "Value".to_owned(), val.clone()));
                out.prop_updates
                    .push((cid.clone(), "SelectedIndex".to_owned(), idx.to_string()));
                out.events.push(UiEvent::change(&cid, &val));
                out.events.push(UiEvent::ev(&cid, "onSelectedIndexChanged"));
                ui.data_mut(|d| d.insert_temp(open_id, false));
            }
            Some(crate::paint::GlassComboAction::Close) => {
                ui.data_mut(|d| d.insert_temp(open_id, false));
            }
            None => {}
        }
    }
    out
}

/// One ComboBox whose dropdown is open, held over for the second pass.
///
/// It carries everything the popup draws with, because by the time the pass
/// runs the `Control` it came from is out of scope â the two highlight colours,
/// the panel's own face, the item metrics and the typography, all resolved from
/// the control while it was still in hand.
struct OpenCombo {
    id: String,
    items: Vec<String>,
    /// The header bar the popup hangs below.
    header: Rect,
    current: String,
    /// `(selected item, hovered item)`, from `paint::combo_popup_fills`.
    fills: (Color32, Color32),
    /// The panel's surface and rim, from `paint::combo_popup_face`.
    face: crate::paint::ComboFace,
    /// One item's height â a line of the control's own text, plus air.
    item_h: f32,
    font: egui::FontId,
    text: Color32,
    /// `DropDownHeight`: the tallest the panel may be before it scrolls.
    max_h: f32,
    /// Scroll this item into view â set on the frame the dropdown opens.
    reveal: Option<usize>,
}

/// Where a ComboBox keeps the item it is highlighting between frames.
fn combo_highlight_id(id: &str) -> egui::Id {
    rt_id(id).with("combo-highlight")
}

/// Where a ComboBox keeps the pointer gesture in progress.
fn combo_gesture_id(id: &str) -> egui::Id {
    rt_id(id).with("combo-gesture")
}

/// Where a ComboBox keeps whether the arrow keys are talking to it.
fn combo_keyboard_id(id: &str) -> egui::Id {
    rt_id(id).with("combo-keyboard")
}

#[derive(Clone)]
struct TabTarget {
    tab_order: u32,
    sequence: usize,
    focus_id: egui::Id,
}

struct DefaultButtonTarget {
    sequence: usize,
    ctrl_id: String,
}

fn collect_tab_targets(
    input: &RenderInput<'_>,
    controls: &[Control],
    order: &[usize],
) -> Vec<TabTarget> {
    let mut targets = Vec::new();
    let mut sequence = 0usize;
    for &idx in order {
        let base = &controls[idx];
        if !input.state.visible(base) || !containers::is_visible(controls, idx, input.active_tabs) {
            continue;
        }
        if input.state.enabled(base) && is_tab_focusable(&base.control_type) {
            let live = input.state.live(base);
            targets.push(TabTarget {
                tab_order: base.tab_order,
                sequence,
                focus_id: tab_focus_id(&live),
            });
        }
        sequence += 1;
    }
    targets
}

fn resolve_default_button_enter(
    input: &RenderInput<'_>,
    controls: &[Control],
    order: &[usize],
    ui: &egui::Ui,
) -> Option<String> {
    let enter = ui.input(|i| i.key_pressed(egui::Key::Enter));
    if !enter || focused_control_is_input(ui, input, controls) {
        return None;
    }
    let target = collect_default_button_target(input, controls, order)?;
    ui.input_mut(|i| {
        i.consume_key(egui::Modifiers::default(), egui::Key::Enter);
        i.events.retain(|event| {
            !matches!(
                event,
                egui::Event::Key {
                    key: egui::Key::Enter,
                    ..
                }
            )
        });
    });
    Some(target.ctrl_id)
}

fn collect_default_button_target(
    input: &RenderInput<'_>,
    controls: &[Control],
    order: &[usize],
) -> Option<DefaultButtonTarget> {
    let mut explicit: Option<DefaultButtonTarget> = None;
    let mut sequence = 0usize;
    for &idx in order {
        let base = &controls[idx];
        if !input.state.visible(base) || !containers::is_visible(controls, idx, input.active_tabs) {
            continue;
        }
        if input.state.enabled(base) && matches!(base.control_type, ControlType::Button) {
            let live = input.state.live(base);
            let target = DefaultButtonTarget {
                sequence,
                ctrl_id: live.id.clone(),
            };
            if live.get_prop("IsDefault").map_or(false, |v| v.as_bool()) {
                if explicit
                    .as_ref()
                    .map_or(true, |current| target.sequence < current.sequence)
                {
                    explicit = Some(target);
                }
            }
        }
        sequence += 1;
    }
    explicit
}

fn focused_control_is_input(ui: &egui::Ui, input: &RenderInput<'_>, controls: &[Control]) -> bool {
    let focused = match ui.ctx().memory(|m| m.focused()) {
        Some(id) => id,
        None => return false,
    };
    controls.iter().enumerate().any(|(idx, base)| {
        input.state.visible(base)
            && containers::is_visible(controls, idx, input.active_tabs)
            && input.state.enabled(base)
            && is_enter_input_control(&base.control_type)
            && tab_focus_id(&input.state.live(base)) == focused
    })
}

fn is_enter_input_control(ct: &ControlType) -> bool {
    use ControlType as CT;
    matches!(
        ct,
        CT::TextBox
            | CT::ComboBox
            | CT::DateTimePicker
            | CT::NumericUpDown
            | CT::DataGrid
            | CT::ListBox
            | CT::TreeView
            | CT::Slider
            | CT::Custom { .. }
    )
}

fn is_tab_focusable(ct: &ControlType) -> bool {
    use ControlType as CT;
    matches!(
        ct,
        CT::Button
            | CT::TextBox
            | CT::CheckBox
            | CT::RadioButton
            | CT::ListBox
            | CT::ComboBox
            | CT::DataGrid
            | CT::DateTimePicker
            | CT::NumericUpDown
            | CT::TreeView
            | CT::Slider
            | CT::Custom { .. }
    )
}

fn tab_focus_id(ctrl: &Control) -> egui::Id {
    let base = rt_id(&ctrl.id);
    if matches!(ctrl.control_type, ControlType::DataGrid) {
        base.with("datagrid-focus")
    } else {
        base
    }
}

fn tab_memory_id() -> egui::Id {
    egui::Id::new("powerrustcobol-tab-order-current")
}

fn tab_pending_id() -> egui::Id {
    egui::Id::new("powerrustcobol-tab-order-pending")
}

fn apply_pending_tab_focus(ui: &egui::Ui) {
    let pending_id = tab_pending_id();
    let pending = ui.data(|d| d.get_temp::<Option<egui::Id>>(pending_id));
    if let Some(Some(focus_id)) = pending {
        ui.ctx().memory_mut(|m| m.request_focus(focus_id));
        ui.data_mut(|d| d.insert_temp(pending_id, None::<egui::Id>));
    }
}

fn resolve_tab_traversal(ui: &egui::Ui, targets: &mut Vec<TabTarget>) -> Option<egui::Id> {
    if targets.is_empty() {
        return None;
    }
    let (tab, shift) = ui.input(|i| (i.key_pressed(egui::Key::Tab), i.modifiers.shift));
    if !tab {
        return None;
    }
    ui.input_mut(|i| {
        let modifiers = egui::Modifiers {
            shift,
            ..egui::Modifiers::default()
        };
        i.consume_key(modifiers, egui::Key::Tab);
        i.events.retain(|event| {
            !matches!(
                event,
                egui::Event::Key {
                    key: egui::Key::Tab,
                    ..
                }
            )
        });
    });

    targets.sort_by_key(|t| (t.tab_order, t.sequence));
    let current = ui
        .data(|d| d.get_temp::<egui::Id>(tab_memory_id()))
        .or_else(|| ui.ctx().memory(|m| m.focused()));
    let current_idx =
        current.and_then(|focused| targets.iter().position(|t| t.focus_id == focused));
    let target_idx = if shift {
        current_idx
            .map(|idx| if idx == 0 { targets.len() - 1 } else { idx - 1 })
            .unwrap_or_else(|| targets.len() - 1)
    } else {
        current_idx
            .map(|idx| (idx + 1) % targets.len())
            .unwrap_or(0)
    };
    let focus_id = targets[target_idx].focus_id;
    ui.data_mut(|d| d.insert_temp(tab_memory_id(), focus_id));
    Some(focus_id)
}

/// Draw just the control **faces** (no backdrop, no interaction) onto an existing
/// `Painter` at `origin`, returning each control's on-screen rect. This is the
/// engine entry for the **Form Designer canvas** (spec 017 T6): the designer owns
/// its own canvas background + editor overlay (selection handles, badges, clones,
/// grid, drop hints) and draws those on top using the returned `control_rects`.
///
/// Faces are produced by the same `draw_control` path as every other surface, so
/// the canvas matches the preview / running form / compiled binary. Clipping uses
/// the painter's current clip as the baseline (top-level controls draw to the
/// canvas, not clipped to the form bounds â the designer's long-standing
/// behaviour, so e.g. a rotated Line past its box still shows on the canvas).
pub fn render_faces(
    painter: &egui::Painter,
    origin: egui::Pos2,
    input: &RenderInput<'_>,
    clip_hook: Option<&dyn RoundedClipHook>,
) -> RenderOutput {
    let mut out = RenderOutput::default();
    let controls = input.controls;
    let order = containers::render_order(controls);
    for &idx in &order {
        let base = &controls[idx];
        if !input.state.visible(base) {
            continue;
        }
        if !containers::is_visible(controls, idx, input.active_tabs) {
            continue;
        }

        let live = input.state.live(base);
        let r = live.rect;
        let tf = input.state.transform(base);
        let base_screen = Rect::from_min_size(
            origin + Vec2::new(r.x as f32 + tf.dx, r.y as f32 + tf.dy),
            Vec2::new(r.w as f32, r.h as f32),
        );
        let screen = crate::paint::scale_rect_about_center(base_screen, tf.scale);
        out.control_rects.insert(live.id.clone(), screen);

        // A PictureBox inside a rounded GroupBox/Panel is clipped to the parent's
        // BORDER path (spec 017) â see `render_form`.
        let pic_border = if clips_to_container_border(&base.control_type) {
            picturebox_container_border(input.controls, input.state, idx, origin, egui::Vec2::ZERO)
        } else {
            None
        };

        // Clip children to ancestor container content areas; top-level controls
        // draw to the painter's existing clip (the canvas), matching the designer.
        let clip = match containers::clip_rect(controls, idx) {
            Some(cm) => painter.clip_rect().intersect(Rect::from_min_size(
                origin + Vec2::new(cm.x as f32, cm.y as f32),
                Vec2::new(cm.w as f32, cm.h as f32),
            )),
            None => painter.clip_rect(),
        };

        let anc = containers::ancestor_opacity(controls, idx);
        let enabled = input.state.enabled(base);
        let alpha = anc * tf.alpha * if enabled { 1.0 } else { 0.45 };

        let mut face = live.clone();
        face.rect = crate::model::Rect::new(
            0,
            0,
            screen.width().round() as i32,
            screen.height().round() as i32,
        );
        if matches!(face.control_type, ControlType::GroupBox) {
            face.set_prop("_DeferCaption", PropValue::Bool(true));
        }
        if matches!(face.control_type, ControlType::TabControl) {
            face.set_prop("_DeferTabs", PropValue::Bool(true));
        }
        if let Some((border, rad)) = pic_border {
            face.set_prop("_ContainerClip", container_clip_prop(border, rad));
        }
        let pic_tex = if matches!(face.control_type, ControlType::PictureBox) {
            crate::paint::picturebox_texture(painter.ctx(), sv(&face, "ImagePath").trim())
                .map(|t| t.id())
        } else {
            None
        };
        let dp = painter.with_clip_rect(clip);
        crate::paint::draw_control(
            &dp,
            screen.min,
            &face,
            false,
            input.glass,
            alpha,
            1.0,
            pic_tex,
        );

        // Rounded-container child clip (spec 017): the face + shadow are now on the
        // framebuffer and the depth-first walk is about to draw this container's
        // children next, so let the host snapshot the backdrop behind its rounded
        // corners here â captured after the shadow, so re-blitting the notch later
        // restores backdrop + shadow instead of erasing it (the flat notch mask's bug).
        if let Some(hook) = clip_hook {
            if matches!(
                base.control_type,
                ControlType::GroupBox | ControlType::Panel
            ) && containers::has_descendants(controls, idx)
            {
                let rad = crate::paint::corner_radius(&live);
                if rad >= 0.5 {
                    hook.on_container(painter, &live.id, screen, rad);
                }
            }
        }
    }
    // All children are painted; flush the rounded clip (re-blit each captured notch).
    if let Some(hook) = clip_hook {
        hook.finish(painter);
    }
    draw_deferred_groupbox_captions(painter, input, &out);
    draw_deferred_tabcontrol_tabs(painter, input, &out);
    out
}

/// Scale `tsize` into `area` per `mode` (Fill/Fit centred, Center, Stretch, Tile).
fn image_dest(area: Rect, tsize: Vec2, mode: BgImageMode) -> Rect {
    match mode {
        BgImageMode::Fill | BgImageMode::Fit => {
            let sx = area.width() / tsize.x.max(1.0);
            let sy = area.height() / tsize.y.max(1.0);
            let s = if matches!(mode, BgImageMode::Fill) {
                sx.max(sy)
            } else {
                sx.min(sy)
            };
            let (dw, dh) = (tsize.x * s, tsize.y * s);
            Rect::from_min_size(
                area.min + Vec2::new((area.width() - dw) * 0.5, (area.height() - dh) * 0.5),
                Vec2::new(dw, dh),
            )
        }
        BgImageMode::Center => Rect::from_min_size(
            area.min
                + Vec2::new(
                    (area.width() - tsize.x) * 0.5,
                    (area.height() - tsize.y) * 0.5,
                ),
            tsize,
        ),
        _ => area, // Stretch / Tile â fill the area
    }
}

/// Build a live `Control` from a designed `base` by overriding the given string
/// props (and the geometry keys `X`/`Y`/`Width`/`Height`). Shared by the run and
/// compiled `FormState` impls, whose state is a full per-control prop map.
pub fn merge_props<'a>(
    base: &Control,
    props: impl IntoIterator<Item = (&'a String, &'a String)>,
) -> Control {
    let mut c = base.clone();
    for (k, v) in props {
        // Geometry is matched case-insensitively for the same reason as
        // `set_prop` below it: these names arrive from COBOL literals and from
        // the object registry, which upper-cases its keys.
        match k.to_ascii_uppercase().as_str() {
            "X" => {
                if let Ok(n) = v.trim().parse::<f32>() {
                    c.rect.x = n.round() as i32;
                }
            }
            "Y" => {
                if let Ok(n) = v.trim().parse::<f32>() {
                    c.rect.y = n.round() as i32;
                }
            }
            "WIDTH" => {
                if let Ok(n) = v.trim().parse::<f32>() {
                    c.rect.w = n.round() as i32;
                }
            }
            "HEIGHT" => {
                if let Ok(n) = v.trim().parse::<f32>() {
                    c.rect.h = n.round() as i32;
                }
            }
            // Struct-backed, like the geometry above it. Writing these into the
            // property map instead left the field — which is what every renderer
            // actually reads — untouched, so a `SET CTL::Visible TO FALSE` was
            // recorded and had no effect on screen. Every accepted spelling maps
            // here; an unrecognisable value leaves the field alone rather than
            // guessing a control into hiding.
            "VISIBLE" => {
                if let Some(b) = crate::model::parse_bool_text(v) {
                    c.visible = b;
                }
            }
            "ENABLED" => {
                if let Some(b) = crate::model::parse_bool_text(v) {
                    c.enabled = b;
                }
            }
            _ => c.set_prop(k.clone(), crate::PropValue::String(v.clone())),
        }
    }
    c
}

/// Resolve a property path for a deployed User Control instance.
///
/// `Caption` targets the receiver itself. `Child.Caption` targets the deployed
/// child id formed by qualifying the child name with the receiver id:
/// `CustomerCard-1` + `Button1.Caption` -> `CustomerCard-1-Button1`, `Caption`.
pub fn resolve_user_control_property_path(
    receiver_id: &str,
    property_path: &str,
) -> Option<(String, String)> {
    let receiver_id = receiver_id.trim();
    let property_path = property_path.trim();
    if receiver_id.is_empty() || property_path.is_empty() {
        return None;
    }

    if let Some((child, prop)) = property_path.split_once('.') {
        let child = child.trim();
        let prop = prop.trim();
        if child.is_empty() || prop.is_empty() {
            return None;
        }
        Some((format!("{receiver_id}-{child}"), prop.to_owned()))
    } else {
        Some((receiver_id.to_owned(), property_path.to_owned()))
    }
}

/// Read a live/designed control property using the same structural keys that
/// [`merge_props`] accepts.
pub fn control_property_string(ctrl: &Control, key: &str) -> String {
    match key.to_ascii_uppercase().as_str() {
        "NAME" => ctrl.id.clone(),
        "X" => ctrl.rect.x.to_string(),
        "Y" => ctrl.rect.y.to_string(),
        "WIDTH" => ctrl.rect.w.to_string(),
        "HEIGHT" => ctrl.rect.h.to_string(),
        // The same spelling the runtime registry uses. These read back as the
        // digits 1/0 while every other boolean read back as a word, so no single
        // comparison a developer wrote could be right for both.
        "VISIBLE" => crate::model::bool_text(ctrl.visible).to_owned(),
        "ENABLED" => crate::model::bool_text(ctrl.enabled).to_owned(),
        "TABORDER" => ctrl.tab_order.to_string(),
        "ZORDER" => ctrl.z_order.to_string(),
        _ => sv(ctrl, key),
    }
}

/// Read a property from the live form state, resolving User Control
/// `child.property` paths first. Returns `None` when the resolved control id does
/// not exist in the designed control list.
pub fn read_user_control_property(
    controls: &[Control],
    state: &dyn FormState,
    receiver_id: &str,
    property_path: &str,
) -> Option<String> {
    let (target_id, key) = resolve_user_control_property_path(receiver_id, property_path)?;
    let base = controls
        .iter()
        .find(|ctrl| ctrl.id.eq_ignore_ascii_case(&target_id))?;
    let live = state.live(base);
    Some(control_property_string(&live, &key))
}

impl UiEvent {
    /// A valueless event (`onClick`, `onGotFocus`, `onTick`, â¦).
    fn ev(id: &str, event: &str) -> Self {
        UiEvent {
            ctrl_id: id.to_owned(),
            event: event.to_owned(),
            value: None,
        }
    }
    /// An `onClick` event.
    fn click(id: &str) -> Self {
        UiEvent::ev(id, "onClick")
    }
    /// An `onChange` event carrying the control's new value.
    fn change(id: &str, value: &str) -> Self {
        UiEvent {
            ctrl_id: id.to_owned(),
            event: "onChange".to_owned(),
            value: Some(value.to_owned()),
        }
    }
    /// Any event carrying a payload (node text, tab index, cell coordinatesâ¦).
    fn with_value(id: &str, event: &str, value: &str) -> Self {
        UiEvent {
            ctrl_id: id.to_owned(),
            event: event.to_owned(),
            value: Some(value.to_owned()),
        }
    }
}

/// The egui interaction id for a running control, derived from its COBOL id.
fn rt_id(id: &str) -> egui::Id {
    egui::Id::new(("rt_ctrl", id))
}

/// Read a control property as a string (empty when unset). Booleans/ints are
/// rendered to their canonical string form so callers backed by a typed model or
/// a stringly-typed prop map both work.
fn sv(c: &Control, key: &str) -> String {
    c.get_prop(key)
        .map(|p| p.to_xml_string())
        .unwrap_or_default()
}

fn prop_bool(c: &Control, key: &str, default: bool) -> bool {
    c.get_prop(key).map(PropValue::as_bool).unwrap_or(default)
}

fn datagrid_filter_property(advanced: &DataGridAdvanced) -> String {
    advanced
        .filters
        .iter()
        .filter(|filter| filter.active && !filter.value.trim().is_empty())
        .map(|filter| format!("{}={}", filter.column_id.trim(), filter.value.trim()))
        .collect::<Vec<_>>()
        .join("\n")
}

/// Which edge of a frozen-pane shadow rectangle is dark (fading to transparent
/// across the rest).
enum FrozenShadowEdge {
    /// Dark on the left edge â for the vertical shadow cast rightward by frozen
    /// columns.
    Left,
    /// Dark on the top edge â for the horizontal shadow cast downward by the
    /// frozen header/rows.
    Top,
}

/// A soft one-directional shadow gradient quad (dark on `dark_edge`, fading to
/// transparent) used as the frozen-pane freeze cue.
fn frozen_shadow_shape(rect: Rect, dark_edge: FrozenShadowEdge, max_alpha: u8) -> egui::Shape {
    use egui::epaint::{Mesh, Vertex, WHITE_UV};
    let dark = Color32::from_black_alpha(max_alpha);
    let clear = Color32::TRANSPARENT;
    let (ctl, ctr, cbr, cbl) = match dark_edge {
        FrozenShadowEdge::Left => (dark, clear, clear, dark),
        FrozenShadowEdge::Top => (dark, dark, clear, clear),
    };
    let mut mesh = Mesh::default();
    for (pos, color) in [
        (rect.left_top(), ctl),
        (rect.right_top(), ctr),
        (rect.right_bottom(), cbr),
        (rect.left_bottom(), cbl),
    ] {
        mesh.vertices.push(Vertex {
            pos,
            uv: WHITE_UV,
            color,
        });
    }
    mesh.indices.extend_from_slice(&[0, 1, 2, 0, 2, 3]);
    egui::Shape::mesh(mesh)
}

/// Evenly distributed tile-center coordinates across `[start, end]` for a
/// nominal spacing. Instead of a fixed start offset with a ragged trailing gap,
/// this picks the tile count that best matches `nominal`, then spreads the tiles
/// so the leading and trailing margins are equal (half a cell) â an even
/// automatic-tile layout that adapts to the available extent.
fn even_tile_centers(start: f32, end: f32, nominal: f32) -> Vec<f32> {
    let len = end - start;
    if len <= 0.0 || nominal <= 0.0 {
        return Vec::new();
    }
    let count = (len / nominal).round().max(1.0) as usize;
    let spacing = len / count as f32;
    (0..count)
        .map(|i| start + spacing * (i as f32 + 0.5))
        .collect()
}

/// Horizontal inset of a rounded-rect silhouette at vertical position `y`. Used
/// to keep DataGrid background patterns inside the rounded corners instead of
/// bleeding into the square corner notches (spec 027 corner bleed).
fn rounded_edge_inset(rect: Rect, radius: f32, y: f32) -> f32 {
    let r = radius.min(rect.width() * 0.5).min(rect.height() * 0.5);
    if r <= 0.0 {
        return 0.0;
    }
    let d = (y - rect.min.y).min(rect.max.y - y);
    if d < 0.0 || d >= r {
        return 0.0;
    }
    let k = r - d;
    r - (r * r - k * k).max(0.0).sqrt()
}

/// Vertical inset of a rounded-rect silhouette at horizontal position `x` â the
/// transpose of [`rounded_edge_inset`]. How far in from the top/bottom edge the
/// arc has cut at that `x`. Used to shorten a DataGrid's vertical grid-line
/// separators so they follow the rounded corner instead of poking into the notch.
fn rounded_edge_inset_v(rect: Rect, radius: f32, x: f32) -> f32 {
    let r = radius.min(rect.width() * 0.5).min(rect.height() * 0.5);
    if r <= 0.0 {
        return 0.0;
    }
    let d = (x - rect.min.x).min(rect.max.x - x);
    if d < 0.0 || d >= r {
        return 0.0;
    }
    let k = r - d;
    r - (r * r - k * k).max(0.0).sqrt()
}

/// Shorten an axis-aligned DataGrid grid line so its ends stay inside the grid's
/// rounded silhouette. A vertical separator near a side edge, or a horizontal
/// separator near the top/bottom, otherwise runs its full extent and pokes past
/// the arc into the corner notch (the "datagrid lines bleed past the corner"
/// case). The DataGrid is a leaf drawn directly, and â when nested inside a
/// translucent panel â the backdrop notch-mask can't be used, so preventing the
/// bleed at the line is the only artifact-free fix. Returns the endpoints unchanged
/// for a non-axis-aligned line or when the radius is negligible.
fn clip_datagrid_line_to_corners(rect: Rect, radius: f32, pts: [egui::Pos2; 2]) -> [egui::Pos2; 2] {
    let r = radius.min(rect.width() * 0.5).min(rect.height() * 0.5);
    if r < 0.5 {
        return pts;
    }
    let [p0, p1] = pts;
    if (p0.x - p1.x).abs() < 0.5 {
        // Vertical line at x: clamp its y-span to the arc at that x.
        let x = p0.x;
        let v = rounded_edge_inset_v(rect, r, x);
        let lo = p0.y.min(p1.y).max(rect.min.y + v);
        let hi = p0.y.max(p1.y).min(rect.max.y - v);
        [pos2(x, lo), pos2(x, hi.max(lo))]
    } else if (p0.y - p1.y).abs() < 0.5 {
        // Horizontal line at y: clamp its x-span to the arc at that y.
        let y = p0.y;
        let h = rounded_edge_inset(rect, r, y);
        let lo = p0.x.min(p1.x).max(rect.min.x + h);
        let hi = p0.x.max(p1.x).min(rect.max.x - h);
        [pos2(lo, y), pos2(hi.max(lo), y)]
    } else {
        pts
    }
}

fn draw_datagrid_pattern(
    painter: &egui::Painter,
    rect: Rect,
    radius: f32,
    pattern: &str,
    color: Color32,
) {
    let pattern = pattern.trim().to_ascii_lowercase();
    if pattern.is_empty() || pattern == "none" {
        return;
    }

    // A point-pattern mark whose centre falls inside a rounded corner notch would
    // poke past the grid's arc; skip it. `rect` is the square grid body, so marks
    // in the corner triangles are exactly the corner bleed (spec 027).
    let inside_silhouette = |cx: f32, cy: f32| -> bool {
        let inset = rounded_edge_inset(rect, radius, cy);
        cx >= rect.min.x + inset && cx <= rect.max.x - inset
    };

    match pattern.as_str() {
        "stripes" | "stripe" => {
            // Horizontal bands, evenly distributed with balanced top/bottom margins.
            for cy in even_tile_centers(rect.min.y, rect.max.y, 12.0) {
                // Recede the band's ends along the corner arcs near top/bottom.
                let inset = rounded_edge_inset(rect, radius, cy)
                    .max(rounded_edge_inset(rect, radius, cy - 3.0))
                    .max(rounded_edge_inset(rect, radius, cy + 3.0));
                let x0 = rect.min.x + inset;
                let x1 = rect.max.x - inset;
                if x1 <= x0 {
                    continue;
                }
                painter.rect_filled(
                    Rect::from_min_max(
                        pos2(x0, (cy - 3.0).max(rect.min.y)),
                        pos2(x1, (cy + 3.0).min(rect.max.y)),
                    ),
                    0.0,
                    color,
                );
            }
        }
        "dots" | "dot" => {
            for cy in even_tile_centers(rect.min.y, rect.max.y, 12.0) {
                for cx in even_tile_centers(rect.min.x, rect.max.x, 12.0) {
                    if inside_silhouette(cx, cy) {
                        painter.circle_filled(pos2(cx, cy), 1.0, color);
                    }
                }
            }
        }
        "cross" | "plus" => {
            let stroke = Stroke::new(1.0, color);
            for cy in even_tile_centers(rect.min.y, rect.max.y, 14.0) {
                for cx in even_tile_centers(rect.min.x, rect.max.x, 14.0) {
                    if !inside_silhouette(cx, cy) {
                        continue;
                    }
                    painter.line_segment([pos2(cx - 3.0, cy), pos2(cx + 3.0, cy)], stroke);
                    painter.line_segment([pos2(cx, cy - 3.0), pos2(cx, cy + 3.0)], stroke);
                }
            }
        }
        "x" | "diagonal-cross" => {
            let stroke = Stroke::new(1.0, color);
            for cy in even_tile_centers(rect.min.y, rect.max.y, 14.0) {
                for cx in even_tile_centers(rect.min.x, rect.max.x, 14.0) {
                    if !inside_silhouette(cx, cy) {
                        continue;
                    }
                    painter
                        .line_segment([pos2(cx - 3.0, cy - 3.0), pos2(cx + 3.0, cy + 3.0)], stroke);
                    painter
                        .line_segment([pos2(cx - 3.0, cy + 3.0), pos2(cx + 3.0, cy - 3.0)], stroke);
                }
            }
        }
        "x dots" | "xdots" | "x-dots" | "x with dots" => {
            let stroke = Stroke::new(1.0, color);
            for cy in even_tile_centers(rect.min.y, rect.max.y, 14.0) {
                for cx in even_tile_centers(rect.min.x, rect.max.x, 14.0) {
                    if !inside_silhouette(cx, cy) {
                        continue;
                    }
                    let points = [
                        pos2(cx - 3.0, cy - 3.0),
                        pos2(cx + 3.0, cy - 3.0),
                        pos2(cx - 3.0, cy + 3.0),
                        pos2(cx + 3.0, cy + 3.0),
                    ];
                    painter.line_segment([points[0], points[3]], stroke);
                    painter.line_segment([points[2], points[1]], stroke);
                    for point in points {
                        painter.circle_filled(point, 1.0, color);
                    }
                }
            }
        }
        "o" | "circle" | "circles" => {
            let stroke = Stroke::new(1.0, color);
            for cy in even_tile_centers(rect.min.y, rect.max.y, 14.0) {
                for cx in even_tile_centers(rect.min.x, rect.max.x, 14.0) {
                    if inside_silhouette(cx, cy) {
                        painter.circle_stroke(pos2(cx, cy), 3.0, stroke);
                    }
                }
            }
        }
        _ => {}
    }
}

/// Universal pointer/gesture events for one control, derived purely from pointer
/// geometry (no extra interactable, so it never steals the control's own
/// interaction). Emits only the events the control declares in `supported_events`
/// â the data-driven loop ignores any without a bound handler. Mirrors the IDE's
/// `control_pointer_events`, but emits neutral [`UiEvent`]s.
#[allow(clippy::too_many_arguments)]
fn control_pointer_events(
    ui: &egui::Ui,
    screen: Rect,
    ctrl_id: egui::Id,
    id: &str,
    ct: &ControlType,
    enabled: bool,
    out: &mut RenderOutput,
    bound_events: &[&str],
    hover_delay_s: f64,
) {
    if !enabled {
        return;
    }
    // Only fire events that the control has a bound handler for, not all
    // supported events. Firing all supported events floods the COBOL event
    // loop (which is single-threaded and blocking) with unhandled events
    // like onMouseMove (60/sec) and onLoad, starving real handlers.
    let want = |e: &str| bound_events.contains(&e);

    let over = ui.rect_contains_pointer(screen);
    let (
        pressed,
        released,
        dbl,
        clicked,
        secondary_clicked,
        middle_clicked,
        pointer_moved,
        wheel_scrolled,
        now,
    ) = ui.input(|i| {
        let pointer_moved = i
            .events
            .iter()
            .any(|e| matches!(e, egui::Event::PointerMoved(_) | egui::Event::MouseMoved(_)));
        let wheel_scrolled = i.events.iter().any(|e| {
            matches!(
                e,
                egui::Event::MouseWheel { delta, .. } if delta.x != 0.0 || delta.y != 0.0
            )
        });
        let middle_clicked = i.events.iter().any(|e| {
            matches!(
                e,
                egui::Event::PointerButton {
                    button: egui::PointerButton::Middle,
                    pressed: false,
                    ..
                }
            )
        });
        (
            i.pointer.primary_pressed(),
            i.pointer.primary_released(),
            i.pointer
                .button_double_clicked(egui::PointerButton::Primary),
            i.pointer.primary_clicked(),
            i.pointer.secondary_clicked(),
            middle_clicked,
            pointer_moved,
            wheel_scrolled,
            i.time,
        )
    });
    let press_mem = ctrl_id.with("press-began-over");
    if pressed {
        ui.ctx().memory_mut(|m| m.data.insert_temp(press_mem, over));
    }
    let loaded_mem = ctrl_id.with("loaded");
    let loaded = ui
        .ctx()
        .memory(|m| m.data.get_temp::<bool>(loaded_mem).unwrap_or(false));
    if !loaded {
        if want("onLoad") {
            out.events.push(UiEvent::ev(id, "onLoad"));
        }
        ui.ctx()
            .memory_mut(|m| m.data.insert_temp(loaded_mem, true));
    }
    if over {
        if pressed && want("onMouseDown") {
            out.events.push(UiEvent::ev(id, "onMouseDown"));
        }
        if released && want("onMouseUp") {
            out.events.push(UiEvent::ev(id, "onMouseUp"));
        }
        if dbl && want("onDblClick") {
            out.events.push(UiEvent::ev(id, "onDblClick"));
        }
        if dbl && want("onDoubleClick") {
            out.events.push(UiEvent::ev(id, "onDoubleClick"));
        }
        if secondary_clicked {
            if want("onRightClick") {
                out.events.push(UiEvent::ev(id, "onRightClick"));
            }
            if want("onContextMenu") {
                out.events.push(UiEvent::ev(id, "onContextMenu"));
            }
        }
        if middle_clicked && want("onMiddleClick") {
            out.events.push(UiEvent::ev(id, "onMiddleClick"));
        }
        if pointer_moved && want("onMouseMove") {
            out.events.push(UiEvent::ev(id, "onMouseMove"));
        }
        if wheel_scrolled && want("onMouseWheel") {
            out.events.push(UiEvent::ev(id, "onMouseWheel"));
        }
        // One press, one click — the flag is CONSUMED as it is spent.
        //
        // This block reads raw pointer state (`primary_clicked`) rather than a
        // widget `Response`, deliberately: adding an interactable here would
        // steal the control's own interaction. But egui runs several PASSES per
        // frame (sizing passes, since 0.31), and raw input reads the same in
        // every one of them, while a widget's `Response` reports its click in
        // only one. So this pushed `onClick` once per pass: a Switch with a
        // bound handler ran it twice for one press, and the operator's handler
        // printed its DISPLAY twice (2026-08-21).
        //
        // Taking the flag rather than peeking at it makes the emission
        // once-per-press by construction — no dependence on how many passes a
        // frame happens to need, or on spotting a discard that may be requested
        // after this runs. The next press sets it again.
        if clicked && want("onClick") {
            let began_over = ui
                .ctx()
                .memory_mut(|m| m.data.remove_temp::<bool>(press_mem).unwrap_or(false));
            if began_over {
                out.events.push(UiEvent::click(id));
            }
        }
    }
    let mem_id = ctrl_id.with("ptr-over");
    let was = ui
        .ctx()
        .memory(|m| m.data.get_temp::<bool>(mem_id).unwrap_or(false));
    if over != was {
        let e = if over { "onMouseEnter" } else { "onMouseLeave" };
        if want(e) {
            out.events.push(UiEvent::ev(id, e));
        }
        ui.ctx().memory_mut(|m| m.data.insert_temp(mem_id, over));
    }
    let hover_start_id = ctrl_id.with("hover-start");
    let hover_fired_id = ctrl_id.with("hover-fired");
    if over {
        let hover_start = ui.ctx().memory(|m| m.data.get_temp::<f64>(hover_start_id));
        let start = hover_start.unwrap_or(now);
        if hover_start.is_none() {
            ui.ctx()
                .memory_mut(|m| m.data.insert_temp(hover_start_id, start));
        }
        let fired = ui
            .ctx()
            .memory(|m| m.data.get_temp::<bool>(hover_fired_id).unwrap_or(false));
        if !fired && now - start >= hover_delay_s {
            if want("onHoverEnter") {
                out.events.push(UiEvent::ev(id, "onHoverEnter"));
            }
            ui.ctx()
                .memory_mut(|m| m.data.insert_temp(hover_fired_id, true));
        }
    } else {
        let fired = ui
            .ctx()
            .memory(|m| m.data.get_temp::<bool>(hover_fired_id).unwrap_or(false));
        if fired && want("onHoverLeave") {
            out.events.push(UiEvent::ev(id, "onHoverLeave"));
        }
        ui.ctx().memory_mut(|m| {
            m.data.remove::<f64>(hover_start_id);
            m.data.insert_temp(hover_fired_id, false);
        });
    }
}

/// Visible/Enabled state-change events (spec 021 T9). Compares the control's
/// EFFECTIVE visibility (own flag + container ancestry) and enabled state
/// against the previous frame; fires only the bound events.
fn visible_enabled_events(
    ui: &egui::Ui,
    input: &RenderInput<'_>,
    controls: &[Control],
    idx: usize,
    out: &mut RenderOutput,
) {
    let base = &controls[idx];
    let want_visible = base.events.iter().any(|e| e.event == "onVisibleChanged");
    let want_enabled = base.events.iter().any(|e| e.event == "onEnabledChanged");
    if !want_visible && !want_enabled {
        return;
    }
    let visible =
        input.state.visible(base) && containers::is_visible(controls, idx, input.active_tabs);
    let enabled = input.state.enabled(base);
    let mem = rt_id(&base.id).with("vis-en");
    let prev = ui.ctx().memory(|m| m.data.get_temp::<(bool, bool)>(mem));
    if let Some((prev_visible, prev_enabled)) = prev {
        if want_visible && prev_visible != visible {
            out.events.push(UiEvent::ev(&base.id, "onVisibleChanged"));
        }
        if want_enabled && prev_enabled != enabled {
            out.events.push(UiEvent::ev(&base.id, "onEnabledChanged"));
        }
    }
    if prev != Some((visible, enabled)) {
        ui.ctx()
            .memory_mut(|m| m.data.insert_temp(mem, (visible, enabled)));
    }
}

/// Geometry change events (spec 021 T10): `onResize` on each frame the size
/// differs from the last, `onResized` once when it settles; likewise
/// `onMove`/`onMoved` for position. Fires regardless of Enabled â geometry is
/// state, not interaction.
fn control_geometry_events(
    ui: &egui::Ui,
    screen: Rect,
    ctrl_id: egui::Id,
    id: &str,
    out: &mut RenderOutput,
    bound: &[&str],
) {
    let want = |e: &str| bound.contains(&e);
    if !want("onResize") && !want("onResized") && !want("onMove") && !want("onMoved") {
        return;
    }
    let mem = ctrl_id.with("geom");
    let prev = ui
        .ctx()
        .memory(|m| m.data.get_temp::<(Rect, bool, bool)>(mem));
    let (mut size_pending, mut pos_pending) = (false, false);
    if let Some((p, sp, pp)) = prev {
        size_pending = sp;
        pos_pending = pp;
        let size_diff = (p.width() - screen.width()).abs() > 0.5
            || (p.height() - screen.height()).abs() > 0.5;
        let pos_diff =
            (p.min.x - screen.min.x).abs() > 0.5 || (p.min.y - screen.min.y).abs() > 0.5;
        if size_diff {
            if want("onResize") {
                out.events.push(UiEvent::ev(id, "onResize"));
            }
            size_pending = true;
        } else if size_pending {
            if want("onResized") {
                out.events.push(UiEvent::ev(id, "onResized"));
            }
            size_pending = false;
        }
        if pos_diff {
            if want("onMove") {
                out.events.push(UiEvent::ev(id, "onMove"));
            }
            pos_pending = true;
        } else if pos_pending {
            if want("onMoved") {
                out.events.push(UiEvent::ev(id, "onMoved"));
            }
            pos_pending = false;
        }
    }
    ui.ctx()
        .memory_mut(|m| m.data.insert_temp(mem, (screen, size_pending, pos_pending)));
}

/// Focus + keyboard events on a focusable control's primary response
/// (spec 021 T6/T8). egui grants click-sense widgets focus on click and via
/// Tab traversal, so `gained_focus`/`has_focus` work for plain `interact`
/// responses. TextBox keeps its own richer handling in its arm.
/// The three events a toggle's state change raises: the directional one for the
/// state it moved INTO, and `onCheckedChanged` for the move itself.
///
/// A handler that only cares about switching on binds `onCheck` and is not woken
/// for the other half; one that mirrors the state either way binds
/// `onCheckedChanged` and reads the value. Shared by the CheckBox, the
/// RadioButton and the Switch, so all three raise the same set â an event a
/// control advertises but never fires is worse than one it does not offer.
/// Which group a RadioButton belongs to.
///
/// Its `GroupName` when it has one â radios sharing a name are mutually
/// exclusive, whatever they sit in. With no name they group by what CONTAINS
/// them, so three radios dropped straight onto a form behave as one group
/// without the developer having to name it, and three inside a GroupBox make
/// their own.
fn radio_group_key(ctrl: &Control) -> String {
    let name = ctrl
        .get_prop("GroupName")
        .map(|v| v.as_str().trim().to_owned())
        .unwrap_or_default();
    if name.is_empty() {
        format!("\u{0}parent:{}", ctrl.parent.clone().unwrap_or_default())
    } else {
        format!("name:{name}")
    }
}

/// Is this radio currently on? `Value` answers when it has been set; otherwise
/// the designed `Checked` does.
fn radio_is_on(ctrl: &Control) -> bool {
    let value = sv(ctrl, "Value");
    if value.is_empty() {
        matches!(sv(ctrl, "Checked").as_str(), "1" | "true")
    } else {
        matches!(value.as_str(), "1" | "true")
    }
}

/// One radio at a time. A radio turns itself ON when clicked, but nothing ever
/// turned the others OFF â so a group could show two, three, every button
/// selected at once, and the form had no way to say which the operator meant.
///
/// Runs after the control loop, where the whole form is in scope: the arm that
/// handles the click can only see its own control.
fn clear_radio_group_siblings(
    input: &RenderInput<'_>,
    controls: &[Control],
    out: &mut RenderOutput,
) {
    let is_radio = |id: &str| {
        controls
            .iter()
            .any(|c| c.id == id && matches!(c.control_type, ControlType::RadioButton))
    };
    let turned_on: Vec<String> = out
        .prop_updates
        .iter()
        .filter(|(_, key, val)| key == "Value" && (val == "1" || val == "true"))
        .filter(|(id, _, _)| is_radio(id))
        .map(|(id, _, _)| id.clone())
        .collect();
    if turned_on.is_empty() {
        return;
    }

    for on_id in turned_on {
        let Some(on) = controls.iter().find(|c| c.id == on_id) else {
            continue;
        };
        let group = radio_group_key(on);
        for other in controls
            .iter()
            .filter(|c| matches!(c.control_type, ControlType::RadioButton))
            .filter(|c| c.id != on_id)
            .filter(|c| radio_group_key(c) == group)
        {
            out.prop_updates
                .push((other.id.clone(), "Value".to_owned(), "0".to_owned()));
            // Only the one that was actually lit reports going out â a form
            // that watches onUncheck should hear about a change, not about
            // every other button in the group on every click.
            if radio_is_on(&input.state.live(other)) {
                out.events.push(UiEvent::change(&other.id, "0"));
                push_toggle_events(out, &other.id, false);
                out.events.push(UiEvent::ev(&other.id, "onValueChanged"));
            }
        }
    }
}

fn push_toggle_events(out: &mut RenderOutput, id: &str, checked: bool) {
    out.events.push(UiEvent::ev(
        id,
        if checked { "onCheck" } else { "onUncheck" },
    ));
    out.events
        .push(UiEvent::with_value(id, "onCheckedChanged", &checked.to_string()));
}

fn focus_keyboard_events(
    ui: &egui::Ui,
    resp: &egui::Response,
    id: &str,
    out: &mut RenderOutput,
    bound: &[&str],
) {
    let want = |e: &str| bound.contains(&e);
    if resp.gained_focus() && want("onGotFocus") {
        out.events.push(UiEvent::ev(id, "onGotFocus"));
    }
    if resp.lost_focus() && want("onLostFocus") {
        out.events.push(UiEvent::ev(id, "onLostFocus"));
    }
    if !resp.has_focus() {
        return;
    }
    if !(want("onKeyDown")
        || want("onKeyUp")
        || want("onKeyPress")
        || want("onEnterPressed")
        || want("onEscapePressed"))
    {
        return;
    }
    let (down, up, typed, enter, escape) = ui.input(|i| {
        let mut down = false;
        let mut up = false;
        let mut typed = false;
        let mut enter = false;
        let mut escape = false;
        for e in &i.events {
            match e {
                egui::Event::Key {
                    key, pressed: true, ..
                } => {
                    down = true;
                    if *key == egui::Key::Enter {
                        enter = true;
                    }
                    if *key == egui::Key::Escape {
                        escape = true;
                    }
                }
                egui::Event::Key { pressed: false, .. } => up = true,
                egui::Event::Text(_) => typed = true,
                _ => {}
            }
        }
        (down, up, typed, enter, escape)
    });
    if down && want("onKeyDown") {
        out.events.push(UiEvent::ev(id, "onKeyDown"));
    }
    if up && want("onKeyUp") {
        out.events.push(UiEvent::ev(id, "onKeyUp"));
    }
    if (typed || down) && want("onKeyPress") {
        out.events.push(UiEvent::ev(id, "onKeyPress"));
    }
    if enter && want("onEnterPressed") {
        out.events.push(UiEvent::ev(id, "onEnterPressed"));
    }
    if escape && want("onEscapePressed") {
        out.events.push(UiEvent::ev(id, "onEscapePressed"));
    }
}

pub fn cursor_icon_for(value: &str) -> Option<egui::CursorIcon> {
    let v = value.trim().to_ascii_lowercase();
    match v.as_str() {
        "" | "default" | "arrow" => None,
        "hand" | "pointinghand" | "pointing_hand" | "pointer" => {
            Some(egui::CursorIcon::PointingHand)
        }
        "text" | "ibeam" | "i-beam" => Some(egui::CursorIcon::Text),
        "wait" | "busy" => Some(egui::CursorIcon::Wait),
        "crosshair" | "cross" => Some(egui::CursorIcon::Crosshair),
        "no" | "notallowed" | "not_allowed" => Some(egui::CursorIcon::NotAllowed),
        "sizeall" | "move" => Some(egui::CursorIcon::Move),
        "sizens" | "resizevertical" | "resize_vertical" => Some(egui::CursorIcon::ResizeVertical),
        "sizewe" | "resizehorizontal" | "resize_horizontal" => {
            Some(egui::CursorIcon::ResizeHorizontal)
        }
        "help" => Some(egui::CursorIcon::Help),
        _ => None,
    }
}

fn decorate_hover_response(resp: egui::Response, ctrl: &Control) -> egui::Response {
    let tooltip = ctrl
        .get_prop("Tooltip")
        .map(|v| v.as_str().trim().to_owned())
        .unwrap_or_default();
    let resp = if tooltip.is_empty() {
        resp
    } else {
        resp.on_hover_text(tooltip)
    };
    if let Some(icon) = ctrl
        .get_prop("Cursor")
        .and_then(|v| cursor_icon_for(v.as_str()))
    {
        resp.on_hover_cursor(icon)
    } else {
        resp
    }
}

/// DataGrid component-frame diagnostic (private to the DataGrid, distinct from the
/// global frame-diagnostics overlay). When [`paint::datagrid_diagnostics_enabled`]
/// is on, outline every structural sub-component of the grid â the whole viewport,
/// the header band, the body band, each column (frozen + scrollable), each visible
/// row, each visible cell, the frozen-column band, and the vertical scrollbar
/// track â each in a distinct colour with a small label, so a mis-sized or
/// mis-placed part is obvious. Purely additive: it paints on a foreground layer
/// after the grid and changes no grid geometry.
///
/// `origin` is the grid's screen-space top-left (`screen.min`); every rect in
/// `layout` is grid-local and offset by it.
#[allow(clippy::too_many_arguments)]
fn draw_datagrid_component_frames(
    painter: &egui::Painter,
    origin: egui::Pos2,
    layout: &DataGridLayout,
    row_h: f32,
) {
    // Foreground, unclipped: above the grid's own fills and outside its content
    // clip so every frame stays visible right to the grid's edges.
    let overlay = egui::Painter::new(
        painter.ctx().clone(),
        egui::LayerId::new(
            egui::Order::Foreground,
            egui::Id::new("cobolt_datagrid_diagnostics"),
        ),
        Rect::EVERYTHING,
    );
    let off = origin.to_vec2();
    let g2s = |x: f32, y: f32, w: f32, h: f32| {
        Rect::from_min_size(pos2(x, y) + off, egui::vec2(w.max(0.0), h.max(0.0)))
    };
    let frame = |rect: Rect, color: Color32, label: &str| {
        overlay.rect_stroke(
            rect,
            egui::CornerRadius::ZERO,
            Stroke::new(1.0, color),
            egui::StrokeKind::Inside,
        );
        if !label.is_empty() && rect.width() > 8.0 && rect.height() > 6.0 {
            overlay.text(
                rect.left_top() + egui::vec2(2.0, 1.0),
                egui::Align2::LEFT_TOP,
                label,
                egui::FontId::monospace(8.0),
                color,
            );
        }
    };
    const C_GRID: Color32 = Color32::from_rgb(255, 64, 64); // red    â whole grid
    const C_HEADER: Color32 = Color32::from_rgb(64, 220, 96); // green  â header
    const C_BODY: Color32 = Color32::from_rgb(80, 160, 255); // blue   â body
    const C_COL: Color32 = Color32::from_rgb(255, 200, 32); // amber  â columns
    const C_ROW: Color32 = Color32::from_rgb(210, 96, 255); // magenta â rows
    const C_CELL: Color32 = Color32::from_rgb(0, 220, 220); // cyan   â cells
    const C_FROZEN: Color32 = Color32::from_rgb(255, 140, 0); // orange â frozen band

    let v = layout.viewport;
    frame(g2s(v.x, v.y, v.w, v.h), C_GRID, "GRID");
    let hr = layout.header_rect;
    frame(g2s(hr.x, hr.y, hr.w, hr.h), C_HEADER, "HEADER");
    let br = layout.body_rect;
    frame(g2s(br.x, br.y, br.w, br.h), C_BODY, "BODY");

    // Frozen-column band (spreadsheet freeze panes), if any.
    if layout.frozen_columns_width > 0.0 {
        frame(
            g2s(v.x, v.y, layout.frozen_columns_width, v.h),
            C_FROZEN,
            "FROZEN",
        );
    }

    // Columns (header cell + full-height band) for frozen + scrollable alike.
    for col in layout
        .frozen_columns
        .iter()
        .chain(layout.scrollable_columns.iter())
    {
        frame(g2s(col.x, hr.y, col.width, hr.h), C_COL, &col.index.to_string());
        frame(g2s(col.x, br.y, col.width, br.h), C_COL, "");
    }

    // Visible rows and their cells (virtualised range only). Clamp every rect to
    // the body band so a partially-scrolled first/last row draws only its visible
    // slice â otherwise the overlay (an unclipped foreground layer, unlike the
    // grid's own clipped content) bleeds past the grid's rounded bottom corner.
    for row in layout.first_row..layout.last_row_exclusive {
        let row_y = br.y + row_h * row as f32 - layout.scroll_y;
        let top = row_y.max(br.y);
        let bot = (row_y + row_h).min(br.max_y());
        if bot - top <= 0.5 {
            continue; // scrolled out of, or clipped to nothing within, the body
        }
        let vis_h = bot - top;
        frame(g2s(br.x, top, br.w, vis_h), C_ROW, &format!("R{row}"));
        for col in layout
            .frozen_columns
            .iter()
            .chain(layout.scrollable_columns.iter())
        {
            frame(g2s(col.x, top, col.width, vis_h), C_CELL, "");
        }
    }
}

/// Decompose an opaque DataGrid fill into the rects that paint it while staying
/// behind the grid's rounded BOTTOM corners. Pure geometry, so the invariants can
/// be unit-tested (see `datagrid_fill_rects_*` tests):
///
/// 1. **Gapless** â the returned rects tile `r`'s vertical span exactly. Any gap,
///    even a sub-pixel one, lets the grid's own background show through as a thin
///    seam. (That was a real bug: a `> min.y + eps` guard skipped the strip above
///    the arc zone when it was thinner than `eps`, revealing the yellow underlay
///    as a 1px line that flashed on and off with the fractional scroll offset.)
/// 2. **Inside the arc** â no rect crosses the corner arc, so nothing bleeds into
///    the notch. Rows in the arc zone are emitted as 1px bands inset by the arc,
///    because a rounded rect cannot hold a radius larger than half its height.
///
/// `r` must already be clamped to `screen`.
fn datagrid_confined_fill_rects(screen: Rect, radius: f32, r: Rect) -> Vec<Rect> {
    let mut out = Vec::new();
    if r.width() <= 0.0 || r.height() <= 0.0 {
        return out;
    }
    let eps = 0.5;
    let r_arc = radius;
    // Gate on OVERLAPPING THE ARC ZONE (the bottom `r_arc` band), NOT on touching
    // the bottom edge: a fill ending *inside* the zone is still crossed by the arc
    // (that gap made the bleed intermittent and scroll-dependent).
    let at_left = (r.min.x - screen.min.x).abs() < eps;
    let at_right = (r.max.x - screen.max.x).abs() < eps;
    let in_arc_zone = r.max.y > screen.max.y - r_arc + eps;
    if !in_arc_zone || (!at_left && !at_right) {
        out.push(r); // away from the arcs a plain square fill is correct
        return out;
    }
    // Horizontal inset of the arc at vertical position `y` (mirrors `arc_inset` in
    // `paint::draw_glass`, including the +0.5 under-stroke nudge, so opaque fills
    // line up with the frost bands beneath them).
    let arc_inset = |y: f32| -> f32 {
        let dy = (screen.max.y - y).abs();
        if dy >= r_arc || r_arc < 0.5 {
            0.0
        } else {
            (r_arc - (r_arc * r_arc - (r_arc - dy) * (r_arc - dy)).max(0.0).sqrt() + 0.5).max(0.0)
        }
    };
    // Part ABOVE the arc zone: one plain full-width rect. No `eps` threshold here â
    // any positive height must be painted or it becomes a visible seam (see 1.).
    let zone_top = (screen.max.y - r_arc).max(r.min.y);
    if zone_top > r.min.y {
        out.push(Rect::from_min_max(r.min, pos2(r.max.x, zone_top)));
    }
    // Corner zone: 1px bands, each inset by the arc at the band BOTTOM (its widest
    // point â never crosses the arc; over-insets by <1px, which is invisible).
    let mut y = zone_top.max(r.min.y);
    while y < r.max.y {
        let yb = (y + 1.0).min(r.max.y);
        let inset = arc_inset(yb);
        let bx0 = if at_left { r.min.x + inset } else { r.min.x };
        let bx1 = if at_right { r.max.x - inset } else { r.max.x };
        if bx1 > bx0 {
            out.push(Rect::from_min_max(pos2(bx0, y), pos2(bx1, yb)));
        }
        y = yb;
    }
    out
}

fn draw_datagrid_line(
    painter: &egui::Painter,
    points: [egui::Pos2; 2],
    stroke: egui::Stroke,
    style: DataGridGridLineStyle,
) {
    match style {
        DataGridGridLineStyle::None => {}
        DataGridGridLineStyle::Solid => {
            painter.line_segment(points, stroke);
        }
        DataGridGridLineStyle::Dash | DataGridGridLineStyle::Dots => {
            let start = points[0];
            let end = points[1];
            let delta = end - start;
            let length = delta.length();
            if length <= 0.5 {
                return;
            }
            let dir = delta / length;
            let (segment, gap) = match style {
                DataGridGridLineStyle::Dash => (6.0, 4.0),
                DataGridGridLineStyle::Dots => (1.0, 4.0),
                _ => unreachable!(),
            };
            let mut offset = 0.0;
            while offset < length {
                let next = (offset + segment).min(length);
                painter.line_segment([start + dir * offset, start + dir * next], stroke);
                offset += segment + gap;
            }
        }
    }
}

/// Render one control as a live, interactive egui widget (Interactive mode),
/// accumulating events + property updates. Faces reuse `draw_control` /
/// `draw_animator` / `draw_picturebox` so the running widget matches the designer
/// pixel-for-pixel; only the interaction (text edit, drag, popup, â¦) is added.
///
/// Ported from the IDE's `render_run_control` + the inline run arms (spec 017
/// unification). `ctrl` is the **screen-normalised** face: its rect is rebased to
/// `(0,0,screen.w,screen.h)`, so faces draw at `screen.min` and any animation
/// shift/scale baked into `screen` is honoured.
#[allow(clippy::too_many_arguments)]
fn render_interactive(
    ui: &mut egui::Ui,
    ctrl: &Control,
    screen: Rect,
    clip: Rect,
    glass: bool,
    alpha: f32,
    enabled: bool,
    // The form's effective (opaque) backdrop colour â what a translucent glass
    // control shows through, so colours that must stay legible on the face can
    // be measured against what the eye actually sees.
    form_bg: Color32,
    out: &mut RenderOutput,
    open_combos: &mut Vec<OpenCombo>,
) {
    use crate::paint;
    use crate::ControlType as CT;
    use egui::{pos2, vec2, Align2, Color32, FontId, Sense, Stroke};

    let id = ctrl.id.as_str();
    let ctrl_id = rt_id(id);
    let ct = ctrl.control_type.clone();
    let painter = ui.painter_at(clip);

    // Universal pointer/gesture/geometry events for every visual control.
    let non_visual = matches!(
        ct,
        CT::Timer | CT::AgentObject | CT::SqlDatabase | CT::RestClient
    );
    let bound: Vec<&str> = ctrl.events.iter().map(|e| e.event.as_str()).collect();
    if !non_visual {
        // The onHoverEnter threshold is the control's HoverDelayMs property
        // (default 200 ms) â not a hardcoded constant.
        let hover_delay_s = (sv(ctrl, "HoverDelayMs").parse::<f64>().unwrap_or(200.0)
            / 1000.0)
            .clamp(0.0, 10.0);
        control_pointer_events(
            ui,
            screen,
            ctrl_id,
            id,
            &ct,
            enabled,
            out,
            &bound,
            hover_delay_s,
        );
        control_geometry_events(ui, screen, ctrl_id, id, out, &bound);
    }

    match ct {
        CT::Button => {
            // WYSIWYG face; only the press/hover feedback is added here.
            let resp = decorate_hover_response(ui.interact(screen, ctrl_id, Sense::click()), ctrl);
            focus_keyboard_events(ui, &resp, id, out, &bound);
            let pressed = resp.is_pointer_button_down_on() && enabled;
            let hovered = resp.hovered() && enabled;
            let draw_rect = if pressed { screen.shrink(1.5) } else { screen };
            // Press shrinks the control: rebase the face to the (shrunk) rect.
            let mut drawn = ctrl.clone();
            drawn.rect = crate::model::Rect::new(
                0,
                0,
                draw_rect.width().round() as i32,
                draw_rect.height().round() as i32,
            );
            paint::draw_control(
                &painter,
                draw_rect.min,
                &drawn,
                false,
                glass,
                alpha,
                1.0,
                None,
            );
            let corner = paint::corner_radius(ctrl);
            if pressed {
                painter.rect_filled(draw_rect, corner, Color32::from_black_alpha(70));
            } else if hovered {
                painter.rect_filled(draw_rect, corner, Color32::from_white_alpha(10));
            }
        }
        CT::CheckBox => {
            let cur = sv(ctrl, "Value");
            let checked = if cur.is_empty() {
                matches!(sv(ctrl, "Checked").as_str(), "1" | "true")
            } else {
                cur == "true" || cur == "1"
            };
            let mut drawn = ctrl.clone();
            drawn
                .properties
                .insert("Checked".to_owned(), crate::PropValue::Bool(checked));
            paint::draw_control(&painter, screen.min, &drawn, false, glass, alpha, 1.0, None);
            let resp = ui.interact(screen, ctrl_id, Sense::click());
            focus_keyboard_events(ui, &resp, id, out, &bound);
            if resp.clicked() && enabled {
                let v = if checked { "0" } else { "1" };
                out.prop_updates
                    .push((id.to_owned(), "Value".to_owned(), v.to_owned()));
                out.events.push(UiEvent::change(id, v));
                push_toggle_events(out, id, v == "1");
                out.events.push(UiEvent::ev(id, "onValueChanged"));
            }
        }
        CT::RadioButton => {
            let selected = matches!(sv(ctrl, "Value").as_str(), "1" | "true")
                || (sv(ctrl, "Value").is_empty()
                    && matches!(sv(ctrl, "Checked").as_str(), "1" | "true"));
            let mut drawn = ctrl.clone();
            drawn
                .properties
                .insert("Checked".to_owned(), crate::PropValue::Bool(selected));
            paint::draw_control(&painter, screen.min, &drawn, false, glass, alpha, 1.0, None);
            let resp = ui.interact(screen, ctrl_id, Sense::click());
            focus_keyboard_events(ui, &resp, id, out, &bound);
            if resp.clicked() && enabled {
                out.prop_updates
                    .push((id.to_owned(), "Value".to_owned(), "1".to_owned()));
                out.events.push(UiEvent::change(id, "1"));
                // A radio only ever moves INTO the selected state by being
                // clicked; the one it deselects is a sibling, not this control.
                push_toggle_events(out, id, true);
                out.events.push(UiEvent::ev(id, "onValueChanged"));
            }
        }
        CT::TextBox => {
            // Face only — no caption, no hint. `draw_control_face` rather than
            // `draw_control` with a blanked `Text`, which is what this used to
            // do: the canvas face previews `HintText` **when `Text` is empty**,
            // so blanking it to silence the caption switched the placeholder ON
            // instead — painted under the live editor, and staying there
            // however much the operator typed (operator, 2026-08-21).
            //
            // The editor supplies the placeholder itself through
            // `TextEdit::hint_text`, which egui shows only while the buffer is
            // empty. That is the one that should be visible, and now the only
            // one. Same reasoning as the ComboBox arm below.
            paint::draw_control_face(&painter, screen.min, ctrl, false, glass, alpha, 1.0, None);
            let txt_col = {
                let fg = sv(ctrl, "ForegroundColor");
                if fg.is_empty() {
                    Color32::DARK_GRAY
                } else {
                    paint::parse_color(&fg)
                }
            };
            let mut buf = sv(ctrl, "Text");
            // The designer paints the face with the control's own font
            // (family + size); the editable overlay must match or the run
            // form silently falls back to egui's default ~14 px font.
            let edit_font = crate::fonts::font_id(
                ui.ctx(),
                &sv(ctrl, "FontName"),
                paint::ctrl_font_size(ctrl),
            );
            // Placeholder shown while the box is empty â same font as the
            // text, foreground colour faded so it reads as a hint on both
            // light and dark faces (egui's default hint gray vanishes on
            // glass themes).
            let hint_text = sv(ctrl, "HintText");
            let hint_col = txt_col.gamma_multiply(0.55);
            // The caret must stay visible whatever the field sits on: egui
            // draws it from the ambient visuals, which left it dark-on-dark on
            // a dark BackgroundColor or a dark form seen through glass.
            let caret_col = paint::caret_color(
                paint::control_surface_tone(ui.ctx(), ctrl, form_bg),
                txt_col,
            );
            // TextAlignment / VerticalAlignment, matching the designer face.
            // Justified lays out left in the editor â egui's TextEdit cannot
            // justify editable text; the static designer face previews it.
            let halign = paint::text_halign(&sv(ctrl, "TextAlignment"));
            let valign = paint::text_valign(&sv(ctrl, "VerticalAlignment"));
            // Keep the editable content clear of the box's own rounded corners: inset
            // by at least the corner radius so text never renders in the corner zone
            // (outside the rounded arc), which would read as bleed past the corner.
            // Horizontal inset keeps the text clear of the rounded corners, so it
            // is floored by the corner radius. The VERTICAL inset is not: a centred
            // single line never reaches the corners, and flooring the top/bottom by
            // the corner radius (or any fixed pad) pushes the text off-centre and
            // wastes the height a tall font needs. So vertical padding is just
            // `InnerPadding`, capped so it can never consume the whole box.
            let pad = paint::textbox_inner_padding(ctrl)
                .max(paint::corner_radius(ctrl))
                .min((screen.width() * 0.45).min(screen.height() * 0.45));
            let vpad = paint::textbox_inner_padding(ctrl).min((screen.height() * 0.5 - 1.0).max(0.0));
            // A Multiline TextBox uses egui's multiline editor, which wraps text to
            // the field width (honouring WordWrap); single-line otherwise.
            let multiline = ctrl
                .get_prop("Multiline")
                .map(|v| v.as_bool())
                .unwrap_or(false);
            // ── The four input properties ─────────────────────────────────
            //
            // All four were seeded, shown in the inspector and documented, and
            // read by nothing at all: a field marked read-only took edits, a
            // password field showed the password, a length limit let anything
            // through and the scrollbars setting did nothing (operator,
            // 2026-08-18, from the dead-property audit).
            let read_only = ctrl
                .get_prop("ReadOnly")
                .map(|v| v.as_bool())
                .unwrap_or(false);
            // 0 = no limit, which is the seeded default.
            let char_limit = sv(ctrl, "MaximumLength").parse::<usize>().unwrap_or(0);
            // The FIRST character of the property masks the text. egui's own
            // `password` mode cannot be used: it masks with a fixed bullet,
            // and this property names the character to mask WITH.
            let mask_char = sv(ctrl, "PasswordCharacter").chars().next();
            // Multiline text starts at the top and can reach the corners, so it
            // keeps the corner-safe inset on every side; a single line is centred
            // and only needs the small vertical padding.
            let edit_rect = if multiline {
                screen.shrink(pad)
            } else {
                egui::Rect::from_min_max(
                    egui::pos2(screen.left() + pad, screen.top() + vpad),
                    egui::pos2(screen.right() - pad, screen.bottom() - vpad),
                )
            };
            // Where in an OVERFLOWING text the at-rest view window sits follows
            // the alignment: Left shows the head, Center the middle, Right the
            // tail. egui's editor always reveals the head when the galley is
            // wider than the field (its scroll offset only follows the caret
            // while focused), so an unfocused overflowing single-line box is
            // hosted in a rect widened to the full text and anchored per the
            // alignment â the box's clip rect then reveals the correct window.
            // While focused the normal rect is used so egui keeps the caret in
            // view as the user types. Interaction cannot leak outside the box:
            // egui clips a widget's interact rect to the active clip rect.
            let single_rect = if !multiline
                && !ui.ctx().memory(|m| m.has_focus(ctrl_id))
                && !matches!(halign, egui::Align::LEFT)
            {
                let text_w = ui.fonts_mut(|f| {
                    f.layout_no_wrap(buf.clone(), edit_font.clone(), txt_col)
                        .size()
                        .x
                });
                // TextEdit keeps its default inner margin (4 px each side)
                // even with Frame::NONE, so the editable width is slightly
                // narrower than the rect it is put into.
                let margin_w = 8.0;
                if text_w > (edit_rect.width() - margin_w).max(0.0) {
                    let w = text_w + margin_w;
                    match halign {
                        egui::Align::RIGHT => egui::Rect::from_min_max(
                            egui::pos2(edit_rect.right() - w, edit_rect.top()),
                            edit_rect.right_bottom(),
                        ),
                        _ => egui::Rect::from_center_size(
                            edit_rect.center(),
                            egui::vec2(w, edit_rect.height()),
                        ),
                    }
                } else {
                    edit_rect
                }
            } else {
                edit_rect
            };
            // ReadOnly is egui's IMMUTABLE buffer: a `&str` renders and can be
            // selected and copied, but takes no edits. `interactive(false)`
            // would have made the field unselectable too, which is a DISABLED
            // control, not a read-only one.
            let read_copy = buf.clone();
            let mut read_view: &str = read_copy.as_str();
            // Masking lays the text out AS the mask character rather than
            // altering the buffer: the value stays real, and the mask has the
            // same character count, so the caret and the selection still land
            // where the operator put them.
            let mut mask_layouter = |ui: &egui::Ui, text: &dyn egui::TextBuffer, wrap: f32| {
                let masked: String =
                    std::iter::repeat_n(mask_char.unwrap_or('*'), text.as_str().chars().count())
                        .collect();
                let mut job = egui::text::LayoutJob::simple(
                    masked,
                    edit_font.clone(),
                    txt_col,
                    if multiline { wrap } else { f32::INFINITY },
                );
                job.halign = halign;
                ui.fonts_mut(|f| f.layout_job(job))
            };
            // Which bars an overflowing multiline box shows. `None` still
            // SCROLLS -- it just draws no bars. Content the box cannot show
            // must never become unreachable, which is the whole reason the
            // editor sits in a scrolling pane at all.
            let (scroll_dirs, bar_vis) = match sv(ctrl, "ScrollBars").as_str() {
                "Horizontal" => (
                    [true, false],
                    egui::scroll_area::ScrollBarVisibility::VisibleWhenNeeded,
                ),
                "Both" => (
                    [true, true],
                    egui::scroll_area::ScrollBarVisibility::VisibleWhenNeeded,
                ),
                "Vertical" => (
                    [false, true],
                    egui::scroll_area::ScrollBarVisibility::VisibleWhenNeeded,
                ),
                _ => (
                    [false, true],
                    egui::scroll_area::ScrollBarVisibility::AlwaysHidden,
                ),
            };
            let resp = if multiline {
                // egui's multiline editor auto-grows to its content, so it would
                // spill past the TextBox's fixed height (and its rounded bottom).
                // Host it in a scroll area clipped to the field so extra rows scroll
                // instead of overflowing: the box keeps its designed height.
                ui.scope_builder(egui::UiBuilder::new().max_rect(edit_rect), |ui| {
                    ui.set_clip_rect(edit_rect);
                    ui.visuals_mut().text_cursor.stroke.color = caret_col;
                    egui::ScrollArea::new(scroll_dirs)
                        .scroll_bar_visibility(bar_vis)
                        .auto_shrink([false, false])
                        .max_height(edit_rect.height())
                        .show(ui, |ui| {
                            let text: &mut dyn egui::TextBuffer =
                                if read_only { &mut read_view } else { &mut buf };
                            let mut edit = egui::TextEdit::multiline(text)
                                .id(ctrl_id)
                                .frame(egui::Frame::NONE)
                                .interactive(enabled)
                                .desired_rows(1)
                                // Text that scrolls sideways must not also wrap,
                                // or there is nothing to scroll to.
                                .desired_width(if scroll_dirs[0] {
                                    f32::INFINITY
                                } else {
                                    edit_rect.width()
                                })
                                .horizontal_align(halign)
                                .font(edit_font.clone())
                                .hint_text(
                                    egui::RichText::new(hint_text.as_str()).color(hint_col),
                                )
                                .text_color(txt_col);
                            if char_limit > 0 {
                                edit = edit.char_limit(char_limit);
                            }
                            if mask_char.is_some() {
                                edit = edit.layouter(&mut mask_layouter);
                            }
                            ui.add(edit)
                        })
                        .inner
                })
                .inner
            } else {
                // Clip to the box so an oversized font is cut at the border instead
                // of spilling out, and vertically centre the single line of text.
                ui.scope_builder(egui::UiBuilder::new().max_rect(edit_rect), |ui| {
                    ui.set_clip_rect(screen.intersect(ui.clip_rect()));
                    ui.visuals_mut().text_cursor.stroke.color = caret_col;
                    let text: &mut dyn egui::TextBuffer =
                        if read_only { &mut read_view } else { &mut buf };
                    let mut edit = egui::TextEdit::singleline(text)
                        .id(ctrl_id)
                        .frame(egui::Frame::NONE)
                        .interactive(enabled)
                        .horizontal_align(halign)
                        .vertical_align(valign)
                        .font(edit_font.clone())
                        .hint_text(egui::RichText::new(hint_text.as_str()).color(hint_col))
                        .text_color(txt_col);
                    if char_limit > 0 {
                        edit = edit.char_limit(char_limit);
                    }
                    if mask_char.is_some() {
                        edit = edit.layouter(&mut mask_layouter);
                    }
                    ui.put(single_rect, edit)
                })
                .inner
            };
            // A read-only field reports no change: egui already refused the
            // edit, and announcing one would fire onChange for a value that
            // never moved.
            if resp.changed() && !read_only {
                out.prop_updates
                    .push((id.to_owned(), "Text".to_owned(), buf.clone()));
                out.events.push(UiEvent::change(id, &buf));
                out.events.push(UiEvent::ev(id, "onTextChanged"));
            }
            if resp.gained_focus() {
                out.events.push(UiEvent::ev(id, "onGotFocus"));
                out.events.push(UiEvent::ev(id, "onEnter"));
            }
            if resp.lost_focus() {
                out.events.push(UiEvent::ev(id, "onLostFocus"));
                out.events.push(UiEvent::ev(id, "onLeave"));
            }
            if resp.has_focus() || resp.lost_focus() {
                let (key_down, key_up, typed, enter, escape) = ui.input(|i| {
                    let mut down = false;
                    let mut up = false;
                    let mut typed = false;
                    let mut enter = false;
                    let mut escape = false;
                    for e in &i.events {
                        match e {
                            egui::Event::Key {
                                key, pressed: true, ..
                            } => {
                                down = true;
                                if *key == egui::Key::Enter {
                                    enter = true;
                                }
                                if *key == egui::Key::Escape {
                                    escape = true;
                                }
                            }
                            egui::Event::Key { pressed: false, .. } => up = true,
                            egui::Event::Text(_) => typed = true,
                            _ => {}
                        }
                    }
                    (down, up, typed, enter, escape)
                });
                if key_down {
                    out.events.push(UiEvent::ev(id, "onKeyDown"));
                }
                if key_up {
                    out.events.push(UiEvent::ev(id, "onKeyUp"));
                }
                if typed || key_down {
                    out.events.push(UiEvent::ev(id, "onKeyPress"));
                }
                if enter {
                    out.events.push(UiEvent::ev(id, "onEnterPressed"));
                }
                if escape {
                    out.events.push(UiEvent::ev(id, "onEscapePressed"));
                }
            }
        }
        CT::Slider => {
            let min_v: f32 = sv(ctrl, "Minimum").parse::<f32>().unwrap_or(0.0);
            let max_v: f32 = sv(ctrl, "Maximum")
                .parse::<f32>()
                .unwrap_or(100.0)
                .max(min_v + 1.0);
            let step: f32 = sv(ctrl, "Step").parse::<f32>().unwrap_or(1.0).max(0.0001);
            let cur: f32 = sv(ctrl, "Value").parse::<f32>().unwrap_or(min_v);
            let orient = sv(ctrl, "Orientation");
            let is_vertical = orient == "Vertical" || orient == "V" || orient == "vertical";

            let thumb_rect = paint::slider_thumb_rect(screen, min_v, max_v, cur, is_vertical);
            let resp = ui.interact(screen, ctrl_id, Sense::drag());
            focus_keyboard_events(ui, &resp, id, out, &bound);
            let mut display_val = cur;
            let slider_dirty_id = ctrl_id.with("value-dirty");

            // The grab (value + axis at press time) must only live while the
            // primary button is actually held. A phantom press at window-open or a
            // release missed because the control wasn't interacted that frame would
            // otherwise leave a STALE grab that corrupts the next real drag (the
            // knob jumps to an extreme and looks stuck). Clear it whenever the
            // button is up â robust regardless of whether `drag_released` fired.
            let primary_down = ui.input(|i| i.pointer.primary_down());
            if !primary_down {
                ui.data_mut(|d| d.remove::<(f32, f32)>(ctrl_id));
            }

            if resp.drag_started() && enabled {
                if let Some(press_pos) = ui.input(|i| i.pointer.press_origin()) {
                    if thumb_rect.contains(press_pos) {
                        let start_axis = if is_vertical {
                            press_pos.y
                        } else {
                            press_pos.x
                        };
                        ui.data_mut(|d| d.insert_temp(ctrl_id, (cur, start_axis)));
                    }
                }
            }
            if let Some((start_val, start_axis)) = ui.data(|d| d.get_temp::<(f32, f32)>(ctrl_id)) {
                if resp.dragged() && primary_down {
                    if let Some(ptr) = ui.ctx().pointer_latest_pos() {
                        let current_axis = if is_vertical { ptr.y } else { ptr.x };
                        let pad = 10.0_f32;
                        let (_track_start, track_len) = if is_vertical {
                            let t = screen.top() + pad;
                            let b = screen.bottom() - pad;
                            (t, (b - t).max(1.0))
                        } else {
                            let l = screen.left() + pad;
                            let r = screen.right() - pad;
                            (l, (r - l).max(1.0))
                        };
                        let delta_axis = current_axis - start_axis;
                        let axis_sign = if is_vertical { -1.0 } else { 1.0 };
                        let delta_val = (delta_axis * axis_sign / track_len) * (max_v - min_v);
                        let raw = start_val + delta_val;
                        display_val = ((raw / step).round() * step).clamp(min_v, max_v);
                    }
                }
                if resp.drag_stopped() {
                    ui.data_mut(|d| {
                        d.remove::<(f32, f32)>(ctrl_id);
                    });
                }
            }

            let mut drawn = ctrl.clone();
            drawn.properties.insert(
                "Value".to_owned(),
                crate::PropValue::String(display_val.to_string()),
            );
            paint::draw_control(&painter, screen.min, &drawn, false, glass, alpha, 1.0, None);

            if (display_val - cur).abs() > 1e-5 {
                out.prop_updates
                    .push((id.to_owned(), "Value".to_owned(), display_val.to_string()));
                out.events
                    .push(UiEvent::change(id, &display_val.to_string()));
                ui.data_mut(|d| d.insert_temp(slider_dirty_id, true));
            }
            if resp.drag_stopped() {
                let dirty = ui.data(|d| d.get_temp::<bool>(slider_dirty_id).unwrap_or(false));
                if dirty {
                    out.events.push(UiEvent::ev(id, "onValueChanged"));
                }
                ui.data_mut(|d| d.insert_temp(slider_dirty_id, false));
            }
        }
        // ââ Knob / Gauge / Switch / FileDropZone (spec 039) âââââââââââââââââ
        //
        // Unlike Slider/NumericUpDown, these are REAL egui-elegance widgets:
        // `Widget::ui` draws AND handles interaction in one call, so there is
        // no need to hand-roll drag math here â `ui.put(screen, widget)`
        // (the same idiom already used for `egui::DragValue` under
        // `CT::NumericUpDown` below) places it at the control's exact rect
        // and returns a standard `Response` whose `.changed()` reports
        // whether the bound value moved this frame, exactly like any other
        // egui widget.
        CT::Knob => {
            // Painted by the SHARED painter, like the check box and the switch
            // beside it, so the canvas, the preview, the running form and the
            // compiled binary draw one knob â at the size it was drawn, in the
            // control's own font. The widget this replaced picked one of three
            // fixed pixel sizes and laid its value out with egui, which is why
            // the canvas and the preview disagreed on both.
            paint::draw_control(&painter, screen.min, ctrl, false, glass, alpha, 1.0, None);

            let min_v: f32 = sv(ctrl, "Minimum").parse().unwrap_or(0.0);
            let max_v: f32 = sv(ctrl, "Maximum").parse::<f32>().unwrap_or(100.0).max(min_v + 1.0);
            let step: f32 = sv(ctrl, "Step").parse::<f32>().unwrap_or(1.0).max(0.0001);
            let val = paint::knob_value(ctrl).clamp(min_v, max_v);

            let resp = ui.interact(screen, ctrl_id, Sense::click_and_drag());
            focus_keyboard_events(ui, &resp, id, out, &bound);
            // Turn it freely: drag up (or right) to raise, down (or left) to
            // lower, a full sweep over about the control's own height.
            let mut moved = val;
            if enabled && resp.dragged() {
                let d = resp.drag_delta();
                // A full sweep takes about twice the knob's own height of
                // travel â far enough to place a value precisely, close enough
                // to reach either end in one gesture.
                let travel = screen.height().max(60.0) * 2.0;
                let span = max_v - min_v;
                moved = (val + (d.x - d.y) / travel * span).clamp(min_v, max_v);
            }
            if enabled && resp.hovered() {
                let wheel = ui.input(|i| i.smooth_scroll_delta.y);
                if wheel.abs() > 0.1 {
                    moved = (moved + wheel.signum() * step).clamp(min_v, max_v);
                }
            }
            // Snap to the step so a drag lands on values a handler can compare.
            let snapped = min_v + ((moved - min_v) / step).round() * step;
            let snapped = snapped.clamp(min_v, max_v);
            if enabled && (snapped - val).abs() > f32::EPSILON {
                let s = paint::format_knob_value(ctrl, snapped);
                out.prop_updates
                    .push((id.to_owned(), "Value".to_owned(), s.clone()));
                out.events.push(UiEvent::change(id, &s));
                out.events.push(UiEvent::ev(id, "onValueChanged"));
            }
        }
        CT::Gauge => {
            // Painted by the SHARED painter, like the Knob and the Switch. The
            // palette crate's gauges size themselves from a `size()` hint rather
            // than the rect they are put in, take their colours from the crate's
            // own palette (so `ForegroundColor`/`BackgroundColor` reached
            // nothing), and lay their reading out with egui â which is how the
            // canvas and the preview came to disagree about one control, and how
            // the reading ended up sitting on the band it reports.
            //
            // A Gauge is read-only: nothing in this arm can write Value.
            paint::draw_control(&painter, screen.min, ctrl, false, glass, alpha, 1.0, None);
        }
        CT::Switch => {
            // Drawn through the SHARED painter, like the CheckBox and the
            // RadioButton beside it â not through the palette crate's widget.
            //
            // That widget hard-codes a 32x18 track and allocates it with
            // `allocate_exact_size`, so it ignored the rect it was given: a
            // switch sized in the designer ran at 32x18 whatever the developer
            // drew, and the designer and the running form disagreed about the
            // same control. There is no builder to size it. Painting it here
            // costs the crate's knob-slide animation and buys size fidelity and
            // one drawing across all four surfaces.
            let checked = matches!(sv(ctrl, "Checked").as_str(), "1" | "true");
            let mut drawn = ctrl.clone();
            drawn
                .properties
                .insert("Checked".to_owned(), crate::PropValue::Bool(checked));
            paint::draw_control(&painter, screen.min, &drawn, false, glass, alpha, 1.0, None);
            let resp = ui.interact(screen, ctrl_id, Sense::click());
            focus_keyboard_events(ui, &resp, id, out, &bound);
            if resp.clicked() && enabled {
                let now = !checked;
                out.prop_updates.push((
                    id.to_owned(),
                    "Checked".to_owned(),
                    now.to_string(),
                ));
                // NO `onClick` here. `control_pointer_events` already emits it
                // for every control that binds a handler — which is how Button
                // and the rest get theirs, and this arm pushing a second one
                // meant a Switch with a bound onClick ran its handler TWICE per
                // click (operator, 2026-08-21). The fault was invisible to a
                // Switch with no handler bound, because the universal emitter
                // stays quiet then and only this push remained.
                //
                // The toggle events below are this arm's own: they carry the
                // new checked state and nothing else emits them.
                push_toggle_events(out, id, now);
            }
        }
        CT::FileDropZone => {
            use elegance::FileDropZone as EFileDropZone;

            let hint = sv(ctrl, "Hint");
            let mut zone = EFileDropZone::new()
                .min_height(screen.height())
                .enabled(enabled);
            if !hint.is_empty() {
                zone = zone.hint(hint);
            }
            let drop_resp = ui.scope_builder(egui::UiBuilder::new().max_rect(screen), |ui| {
                zone.show(ui)
            });
            let fdz = drop_resp.inner;
            // The intake, in a line along the bottom of the zone: what is
            // staged and waiting, and after the form goes ahead, what was
            // copied. Drawn OVER the widget rather than replacing the hint, so
            // the zone still says it takes files.
            let summary = sv(ctrl, "CommitSummary");
            if !summary.trim().is_empty() {
                ui.painter().text(
                    egui::pos2(screen.center().x, screen.bottom() - 4.0),
                    egui::Align2::CENTER_BOTTOM,
                    summary,
                    egui::FontId::proportional(11.0),
                    ui.visuals().strong_text_color(),
                );
            }
            focus_keyboard_events(ui, &fdz.response, id, out, &bound);
            if !fdz.dropped_files.is_empty() && enabled {
                // The OS drag-drop path needs no native dialog (egui's own
                // input already carries the dropped paths) â populate
                // DroppedFiles and fire onFilesDropped right here. The
                // click-to-browse path (rfd, a native dialog) is cross-crate
                // plumbing `cobolt-forms` cannot own on its own (no `rfd`
                // dependency here by design â see spec 039 T4) and is wired
                // at the `cobolt-ide` host level instead.
                // egui 0.36 made DroppedFile a trait: `path()` is a method and
                // always present (on the web it is just the file name).
                let paths: Vec<String> = fdz
                    .dropped_files
                    .iter()
                    .map(|f| f.path().display().to_string())
                    .collect();
                // What the zone accepts, and where it puts it: the same intake
                // the click-to-browse path runs, so a file is judged by the same
                // rules however it arrived. `apply_drop` also decides whether
                // this drop COPIES now or only stages for the form to confirm â
                // one answer, shared with that path.
                let writes = crate::dropzone::apply_drop(
                    id,
                    &paths,
                    crate::dropzone::ZoneRules {
                        filter: &sv(ctrl, "AllowedExtensions"),
                        max_kb: sv(ctrl, "MaximumFileSizeKB").parse::<i64>().unwrap_or(0),
                        destination: &sv(ctrl, "DestinationFolder"),
                        stage_only: prop_bool(ctrl, "StageOnly", false),
                        list_id: &sv(ctrl, "FileListControl"),
                        already_staged: &sv(ctrl, "StagedFiles"),
                    },
                );
                out.prop_updates.extend(writes.updates);
                if writes.accepted > 0 {
                    out.events.push(UiEvent::ev(id, "onFilesDropped"));
                }
                if writes.rejected > 0 {
                    out.events.push(UiEvent::ev(id, "onFilesRejected"));
                }
            } else if fdz.response.clicked() && enabled {
                // Click, not a drop â the host owns opening a native picker
                // (T4). Not gated on `dropped_files` being non-empty above,
                // since a click and a drop are mutually exclusive per frame.
                out.file_picker_requests.push(id.to_owned());
            }
        }
        // ââ Maps (spec 039 T9) âââââââââââââââââââââââââââââââââââââââââââââââ
        //
        // No off-the-shelf widget (T1's finding) â pan/zoom are computed
        // here from raw pointer/scroll input, exactly the way Slider
        // computes its own drag math above, and the shared
        // `map_tiles::paint_map` (also used by the designer canvas's static
        // face, `paint.rs`) draws the result.
        CT::Maps => {
            use crate::map_tiles;

            // The face first — drop shadow, neumorphic relief, the designed
            // background gradient, the corner radius. This arm used to go
            // straight to interaction and tiles, so everything a developer set
            // in the Appearance and Drop Shadow sections applied on the canvas
            // and vanished the moment the form ran. `draw_control_face` (not
            // `draw_control`) because the canvas's stand-in basemap must not be
            // painted under the live one — the same call, and the same reason,
            // as the TextBox and ComboBox arms.
            paint::draw_control_face(&painter, screen.min, ctrl, false, glass, alpha, 1.0, None);

            let center_lat: f64 = sv(ctrl, "CenterLat").parse().unwrap_or(0.0);
            let center_lng: f64 = sv(ctrl, "CenterLng").parse().unwrap_or(0.0);
            let zoom_i = sv(ctrl, "Zoom").parse::<i64>().unwrap_or(2);
            let zoom = zoom_i.clamp(
                map_tiles::MIN_ZOOM as i64,
                map_tiles::MAX_ZOOM as i64,
            ) as u8;

            let resp = ui.interact(screen, ctrl_id, Sense::click_and_drag());
            focus_keyboard_events(ui, &resp, id, out, &bound);

            // The view the developer is DRAGGING, held here rather than read
            // back from the properties each frame.
            //
            // The properties are the published truth, but they make a round
            // trip through the host before they come back — so applying each
            // frame's drag delta to whatever the property currently said meant
            // applying it to a value one or more frames stale, and the map
            // stuttered against the pointer. The live view is authoritative
            // while the pointer is on it; a write from COBOL still wins,
            // because it changes the property away from what we last published
            // and that is what `MapView::sync` watches for.
            let view_id = ctrl_id.with("map-view");
            let mut view = ui
                .ctx()
                .data(|d| d.get_temp::<MapView>(view_id))
                .unwrap_or(MapView::seeded(center_lat, center_lng, zoom));
            view.sync(center_lat, center_lng, zoom);

            let mut bounds_changed = false;

            if resp.dragged() && enabled {
                let delta = resp.drag_delta();
                if delta != egui::Vec2::ZERO {
                    // At the zoom the map is DRAWN at: mid-glide a pixel of drag
                    // covers less ground than the whole level would say, and
                    // resolving against the level alone made the map slip under
                    // the pointer while it was still scaling.
                    let (lat, lng) = map_tiles::offset_to_lat_lng_at(
                        -delta.x,
                        -delta.y,
                        view.lat,
                        view.lng,
                        view.zoom_at() as f64,
                    );
                    view.lat = lat;
                    view.lng = lng;
                    bounds_changed = true;
                }
            }
            if resp.hovered() && enabled {
                // The RAW wheel events, not `smooth_scroll_delta`.
                //
                // egui's smoothing spreads one gesture over several frames, and
                // `zoom_steps` already does that job with an accumulator this
                // code owns. Running both smooths the smoothed: the map answers
                // late and keeps drifting after the fingers stop. Units are
                // normalised to points exactly as egui does it, so a line-based
                // mouse and a pixel-based trackpad agree on what a level costs.
                let page = ui.ctx().content_rect().height();
                let line = ui.ctx().options(|o| o.input_options.line_scroll_speed);
                let scroll = ui.input(|i| {
                    i.raw
                        .events
                        .iter()
                        .filter_map(|e| match e {
                            egui::Event::MouseWheel { unit, delta, .. } => Some(match unit {
                                egui::MouseWheelUnit::Point => delta.y,
                                egui::MouseWheelUnit::Line => delta.y * line,
                                egui::MouseWheelUnit::Page => delta.y * page,
                            }),
                            _ => None,
                        })
                        .sum::<f32>()
                });
                // A SLICE of the pending zoom per frame, not whole levels.
                //
                // A whole level is a factor of two, so `zoom_steps` could only
                // ever replace the picture: the map went from 12 to 13 between
                // two frames with nothing in between. The glide hands back
                // fractions, the painter can hold the map at 12.4, and the eye
                // follows the scale (operator, 2026-08-22).
                let (step, pending) = map_tiles::zoom_glide(view.zoom_accum, scroll);
                view.zoom_accum = pending;
                if step != 0.0 {
                    let from = view.zoom_at();
                    let to = (from + step)
                        .clamp(map_tiles::MIN_ZOOM as f32, map_tiles::MAX_ZOOM as f32);
                    if to != from {
                        // Keep whatever is under the cursor under the cursor —
                        // at the CONTINUOUS zoom, or the anchor would drift by
                        // whatever fraction of a level the glide is holding.
                        let anchor = ui
                            .ctx()
                            .pointer_latest_pos()
                            .map(|p| p - screen.center())
                            .unwrap_or(egui::Vec2::ZERO);
                        let (lat, lng) = map_tiles::zoom_about_at(
                            view.lat, view.lng, from as f64, to as f64, anchor.x, anchor.y,
                        );
                        let (level, frac) = map_tiles::split_zoom(to);
                        view.lat = lat;
                        view.lng = lng;
                        view.zoom = level;
                        view.zoom_frac = frac;
                        bounds_changed = true;
                    } else {
                        // Held at an end stop: spend the rest rather than
                        // repainting forever against a clamp.
                        view.zoom_accum = 0.0;
                    }
                }
                if view.zoom_accum != 0.0 {
                    // The glide only continues if something asks for the next
                    // frame — the wheel has already stopped sending events.
                    ui.ctx().request_repaint();
                }
            }
            let mut new_center = (view.lat, view.lng);
            let mut new_zoom = view.zoom;
            if resp.double_clicked() && enabled {
                if let Some(pos) = ui.ctx().pointer_latest_pos() {
                    let off = pos - screen.center();
                    new_center = map_tiles::offset_to_lat_lng(
                        off.x, off.y, new_center.0, new_center.1, zoom,
                    );
                    new_zoom = (zoom + 1).min(map_tiles::MAX_ZOOM);
                    bounds_changed = true;
                }
            }

            if bounds_changed {
                let (lat_s, lng_s, zoom_s) = (
                    new_center.0.to_string(),
                    new_center.1.to_string(),
                    new_zoom.to_string(),
                );
                // Remember exactly what was published, so next frame's `sync`
                // can tell "the property came back as we left it" from "COBOL
                // moved the map" — the first must not reset the live view.
                view.published = (lat_s.clone(), lng_s.clone(), new_zoom);
                out.prop_updates
                    .push((id.to_owned(), "CenterLat".to_owned(), lat_s));
                out.prop_updates
                    .push((id.to_owned(), "CenterLng".to_owned(), lng_s));
                out.prop_updates
                    .push((id.to_owned(), "Zoom".to_owned(), zoom_s));
                out.events.push(UiEvent::ev(id, "onBoundsChanged"));
            }
            view.lat = new_center.0;
            view.lng = new_center.1;
            view.zoom = new_zoom;
            ui.ctx().data_mut(|d| d.insert_temp(view_id, view.clone()));

            let markers_raw = sv(ctrl, "Markers");
            let records = crate::parse_map_markers(&markers_raw);
            let markers: Vec<map_tiles::MapMarker> = records
                .iter()
                .map(|m| map_tiles::MapMarker {
                    lat: m.lat,
                    lng: m.lng,
                    label: &m.label,
                    id: &m.id,
                    info: &m.info,
                })
                .collect();
            let click_pos = if resp.clicked() {
                ui.ctx().pointer_interact_pos()
            } else {
                None
            };
            let routes = crate::model::parse_map_routes(&sv(ctrl, "Routes"));
            let regions = crate::model::parse_map_regions(&sv(ctrl, "Regions"));
            // The info window follows the FORM's colours by default and takes
            // an override per part, so a map matches the form it sits on
            // without being told to, and can still be restyled when it must be.
            let base = map_tiles::InfoStyle::default();
            // An EMPTY property means "follow the theme" — which is why each is
            // tested for emptiness rather than parsed straight through:
            // `parse_color` has no way to say "nothing was set".
            let styled = |key: &str| {
                let raw = sv(ctrl, key);
                (!raw.trim().is_empty()).then(|| paint::parse_color(&raw))
            };
            let mut info_style = map_tiles::InfoStyle {
                bg: styled("BackgroundColor").unwrap_or(base.bg),
                ..base
            };
            if let Some(c) = styled("InfoBackgroundColor") {
                info_style.bg = c;
            }
            // The ink is DERIVED from whichever background won, not inherited
            // beside it. Taking fg from ForegroundColor and bg from
            // BackgroundColor independently is what produced white text on a
            // light card — two colours from two places with nothing making them
            // contrast. An explicit InfoForegroundColor still overrides.
            info_style.fg = styled("InfoForegroundColor")
                .unwrap_or_else(|| map_tiles::readable_ink(info_style.bg));
            if let Some(c) = styled("InfoBorderColor") {
                info_style.border = c;
            }
            if let Ok(r) = sv(ctrl, "InfoCornerRadius").trim().parse::<f32>() {
                info_style.corner = r.clamp(0.0, 32.0);
            }
            let shadow_raw = sv(ctrl, "InfoShadow");
            if !shadow_raw.trim().is_empty() {
                info_style.shadow =
                    shadow_raw != "0" && !shadow_raw.eq_ignore_ascii_case("false");
            }
            let fs = sv(ctrl, "FontSize").trim().parse::<f32>().unwrap_or(0.0);
            if fs > 0.0 {
                info_style.font_size = fs.clamp(8.0, 28.0);
            }

            let pointer = map_tiles::MapPointer {
                // Only while the pointer is genuinely over THIS map: a card
                // that lingers after the pointer has left is worse than none.
                hover: if resp.hovered() && enabled {
                    ui.ctx().pointer_latest_pos().filter(|p| screen.contains(*p))
                } else {
                    None
                },
                click: click_pos,
            };
            let open_marker = sv(ctrl, "SelectedMarkerId");
            let open_region = sv(ctrl, "SelectedRegionId");
            // The fraction the glide is holding rides with the level. A
            // double-click zoom (`new_zoom` above) lands on a whole level, so
            // the fraction only applies while `new_zoom` is still the view's.
            let draw_frac = if new_zoom == view.zoom {
                view.zoom_frac
            } else {
                0.0
            };
            let hit = map_tiles::paint_map_at(
                &painter,
                screen,
                new_center.0,
                new_center.1,
                new_zoom,
                draw_frac,
                &markers,
                &routes,
                &regions,
                pointer,
                &open_marker,
                &open_region,
                &info_style,
                &map_tiles::MapColors::from_control(ctrl),
            );

            // Hover events fire BESIDE the native window, so a form can build
            // its own panel without giving up the default one. Only on the
            // frame the hovered item CHANGES: an event every frame would run
            // the handler hundreds of times a second over one marker.
            let hover_id = hit
                .hovered_marker
                .and_then(|i| records.get(i))
                .map(|m| m.id.clone())
                .or_else(|| {
                    hit.hovered_region
                        .and_then(|i| regions.get(i))
                        .map(|r| r.id.clone())
                })
                .unwrap_or_default();
            let hover_mem = ctrl_id.with("map-hover");
            let last_hover = ui
                .ctx()
                .data(|d| d.get_temp::<String>(hover_mem))
                .unwrap_or_default();
            if hover_id != last_hover {
                ui.ctx()
                    .data_mut(|d| d.insert_temp(hover_mem, hover_id.clone()));
                if !hover_id.is_empty() && enabled {
                    let (prop, event) = if hit.hovered_marker.is_some() {
                        ("HoveredMarkerId", "onMarkerHover")
                    } else {
                        ("HoveredRegionId", "onRegionHover")
                    };
                    out.prop_updates
                        .push((id.to_owned(), prop.to_owned(), hover_id.clone()));
                    out.events.push(UiEvent::ev(id, event));
                }
            }

            if let Some(idx) = hit.clicked_region {
                if let Some(r) = regions.get(idx) {
                    out.prop_updates.push((
                        id.to_owned(),
                        "SelectedRegionId".to_owned(),
                        r.id.clone(),
                    ));
                }
                out.prop_updates
                    .push((id.to_owned(), "SelectedMarkerId".to_owned(), String::new()));
                out.events.push(UiEvent::ev(id, "onRegionClick"));
            }
            if let Some(idx) = hit.clicked_marker {
                // Marker identity is exposed via a property, the same way
                // repeating-group instance data reaches COBOL through
                // CONTROL-ARRAY-INDEX rather than the event's own target id
                // â markers are not full Controls with their own event
                // routing.
                if let Some(m) = records.get(idx) {
                    out.prop_updates.push((
                        id.to_owned(),
                        "SelectedMarkerId".to_owned(),
                        m.id.clone(),
                    ));
                }
                out.prop_updates
                    .push((id.to_owned(), "SelectedRegionId".to_owned(), String::new()));
                out.events.push(UiEvent::ev(id, "onMarkerClick"));
            } else if hit.clicked_region.is_none() && resp.clicked() && enabled {
                // Clicking bare map dismisses whatever card was open — how
                // every map behaves, and the only way to shut one.
                out.prop_updates
                    .push((id.to_owned(), "SelectedMarkerId".to_owned(), String::new()));
                out.prop_updates
                    .push((id.to_owned(), "SelectedRegionId".to_owned(), String::new()));
                out.events.push(UiEvent::ev(id, "onMapClick"));
            }
        }
        CT::NumericUpDown => {
            // Face from the SHARED painter, dragging added here â so the canvas,
            // the preview, the running form and the compiled binary show one
            // field. It used to be an egui `DragValue` dropped onto a hand-drawn
            // surface: the widget brought its own background, its own hover and
            // the ambient font, none of which the canvas could draw â and the
            // canvas answered by lettering "â²â¼" into the caption, a control that
            // existed on no other surface.
            paint::draw_control(&painter, screen.min, ctrl, false, glass, alpha, 1.0, None);

            let min = sv(ctrl, "Minimum").parse::<f64>().unwrap_or(0.0);
            let max = sv(ctrl, "Maximum").parse::<f64>().unwrap_or(100.0).max(min);
            let step = sv(ctrl, "Step").parse::<f64>().unwrap_or(1.0).max(0.0001);
            let val = sv(ctrl, "Value").parse::<f64>().unwrap_or(min).clamp(min, max);

            let resp = ui.interact(screen, ctrl_id, Sense::click_and_drag());
            focus_keyboard_events(ui, &resp, id, out, &bound);
            let mut moved = val;
            if enabled && resp.dragged() {
                // One step per four pixels, the way a spinner's drag behaves.
                let d = resp.drag_delta();
                moved = (val + (d.x - d.y) as f64 / 4.0 * step).clamp(min, max);
            }
            if enabled && resp.hovered() {
                let wheel = ui.input(|i| i.smooth_scroll_delta.y);
                if wheel.abs() > 0.1 {
                    moved = (moved + wheel.signum() as f64 * step).clamp(min, max);
                }
            }
            let snapped = (min + ((moved - min) / step).round() * step).clamp(min, max);
            if enabled && (snapped - val).abs() > f64::EPSILON {
                let s = if step.fract().abs() > f64::EPSILON {
                    format!("{snapped:.2}")
                } else {
                    format!("{snapped:.0}")
                };
                out.prop_updates
                    .push((id.to_owned(), "Value".to_owned(), s.clone()));
                out.events.push(UiEvent::change(id, &s));
                out.events.push(UiEvent::ev(id, "onValueChanged"));
            }
        }
        CT::ComboBox => {
            // The face is the DEVELOPER'S â `BackgroundColor`, the background
            // gradient, the border and the corner radius â drawn by the same
            // call the designer canvas uses, so what is designed is what runs.
            //
            // The header used to lay a hardcoded navy surface and a blue rim
            // over the design, exactly as the ListBox did before 1.61.87
            // (operator, 2026-08-18). `draw_control_face` rather than
            // `draw_control` because the canvas's stand-in caption â the first
            // item and a `â¾` â would otherwise be painted underneath the real
            // value the header draws.
            paint::draw_control_face(&painter, screen.min, ctrl, false, glass, alpha, 1.0, None);
            let mut items: Vec<String> = sv(ctrl, "Items").lines().map(|l| l.to_owned()).collect();
            // `Sorted` — the display order only; what the developer typed stays
            // exactly as they typed it.
            paint::list_display_items(ctrl, &mut items);
            let items = items;
            let cur = sv(ctrl, "Value");
            let sel = if cur.is_empty() {
                items.first().cloned().unwrap_or_default()
            } else {
                cur.clone()
            };
            let open_id = ctrl_id.with("combo_open");
            let was_open_id = ctrl_id.with("combo_was_open");
            let highlight_id = combo_highlight_id(id);
            let gesture_id = combo_gesture_id(id);
            let mut is_open = ui.data(|d| d.get_temp::<bool>(open_id)).unwrap_or(false);
            let at = |value: &str| items.iter().position(|it| it == value);
            let mut highlight: usize = ui
                .data(|d| d.get_temp::<usize>(highlight_id))
                .unwrap_or_else(|| at(&sel).unwrap_or(0));
            // Scroll the current value into view on the frame the list opens,
            // so a dropdown never opens showing the top of a long list while
            // the value it holds is forty items further down.
            let mut reveal: Option<usize> = None;

            let (pointer_pressed, pointer_released, pointer_pos) = ui.input(|i| {
                (
                    i.pointer.primary_pressed(),
                    i.pointer.primary_released(),
                    i.pointer.interact_pos(),
                )
            });
            let on_control = pointer_pos.is_some_and(|p| screen.contains(p));
            let pressed_here = enabled && pointer_pressed && on_control;

            // A dropdown opens on the PRESS, not on the release: the classic
            // combo gesture is press on the header, drag into the list, release
            // on an item, and none of that can happen while the list only
            // appears once the button is already back up.
            if pressed_here {
                is_open = !is_open;
                ui.data_mut(|d| {
                    d.insert_temp(open_id, is_open);
                    d.insert_temp(
                        gesture_id,
                        if is_open {
                            paint::ComboGesture::Header
                        } else {
                            paint::ComboGesture::None
                        },
                    );
                });
                if is_open {
                    highlight = at(&sel).unwrap_or(0);
                    reveal = Some(highlight);
                    out.events.push(UiEvent::ev(id, "onDropDown"));
                }
            }

            // onDropDownClosed (spec 021 T12): the popup pass flips the open
            // flag when an item is picked or the gesture is dismissed; compare
            // against last frame's state here.
            let was_open = ui.data(|d| d.get_temp::<bool>(was_open_id)).unwrap_or(false);
            if was_open && !is_open {
                out.events.push(UiEvent::ev(id, "onDropDownClosed"));
            }
            if was_open != is_open {
                ui.data_mut(|d| d.insert_temp(was_open_id, is_open));
            }

            let item_color = {
                let fg = sv(ctrl, "ForegroundColor");
                paint::caret_color(
                    paint::control_surface_tone(ui.ctx(), ctrl, form_bg),
                    if fg.is_empty() {
                        Color32::from_rgb(220, 228, 255)
                    } else {
                        paint::parse_color(&fg)
                    },
                )
            };
            let item_font = crate::fonts::font_id(
                ui.ctx(),
                &sv(ctrl, "FontName"),
                paint::ctrl_font_size(ctrl),
            );
            // The header's own click (a release without a drag) is already
            // answered by the press above; the response is taken so the header
            // still swallows the pointer and reports hover.
            let _ = paint::glass_combo_header(
                &painter,
                ui,
                screen,
                ctrl_id,
                &sel,
                is_open,
                enabled,
                Some((item_font.clone(), item_color)),
            );

            // A press inside hands the combo the keyboard â on press AND on
            // release, because egui settles a click on the way UP and the
            // release would otherwise take the focus straight back off it.
            if enabled && (pointer_pressed || pointer_released) && on_control {
                ui.memory_mut(|m| m.request_focus(ctrl_id));
            }

            // ââ Arrow keys with the list CLOSED âââââââââââââââââââââââââââââ
            //
            // They change the value outright, as a Windows combo does. The
            // combo has to own them in its own state: egui answers a plain
            // arrow itself by walking focus to the widget lying in that
            // direction, so reading `has_focus` alone buys exactly one press
            // before the control goes deaf.
            //
            // `Editable` makes no difference here, and deliberately. No
            // ComboBox on any surface accepts typed text today â the property
            // is declared but the header is a click target, not a field â so
            // there is no caret for an arrow to move; and even where a combo
            // does type, the arrows belong to the list and the caret to
            // â / â. If typing ever lands, the arrows stay with the list.
            let kb_id = combo_keyboard_id(id);
            let mut has_keyboard: bool = ui.data(|d| d.get_temp(kb_id)).unwrap_or(false);
            if pointer_pressed {
                // Dropped on any press elsewhere â including one inside the
                // open list, which the popup pass then hands straight back.
                has_keyboard = on_control;
            }
            if ui.input(|i| i.key_pressed(egui::Key::Tab)) {
                has_keyboard = false;
            }
            let listening = has_keyboard || ui.memory(|m| m.has_focus(ctrl_id));
            if enabled && !is_open && !items.is_empty() && listening {
                let (up, down) = ui.input_mut(|i| {
                    (
                        i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowUp),
                        i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowDown),
                    )
                });
                if up || down {
                    has_keyboard = true;
                    let from = at(&cur);
                    // With nothing chosen yet the first arrow lands on the
                    // first item rather than jumping to the end of the list.
                    let to = match from {
                        None => 0,
                        Some(i) if up => i.saturating_sub(1),
                        Some(i) => (i + 1).min(items.len() - 1),
                    };
                    if Some(to) != from {
                        highlight = to;
                        out.prop_updates.push((
                            id.to_owned(),
                            "Value".to_owned(),
                            items[to].clone(),
                        ));
                        out.prop_updates.push((
                            id.to_owned(),
                            "SelectedIndex".to_owned(),
                            to.to_string(),
                        ));
                        out.events.push(UiEvent::change(id, &items[to]));
                        out.events.push(UiEvent::ev(id, "onSelectedIndexChanged"));
                    }
                }
            }
            ui.data_mut(|d| {
                d.insert_temp(kb_id, has_keyboard);
                d.insert_temp(highlight_id, highlight);
            });

            if is_open && enabled {
                open_combos.push(OpenCombo {
                    id: id.to_owned(),
                    items,
                    header: screen,
                    current: cur,
                    // Everything below is resolved HERE, while the control is
                    // still in hand: the popup pass has only the id.
                    fills: paint::combo_popup_fills(ctrl),
                    face: paint::combo_popup_face(ctrl),
                    item_h: crate::model::text_line_height(ctrl)
                        + crate::model::LIST_ROW_PAD * 2.0,
                    font: item_font,
                    text: item_color,
                    max_h: sv(ctrl, "DropDownHeight")
                        .parse::<f32>()
                        .unwrap_or(200.0)
                        .clamp(1.0, 4000.0),
                    reveal,
                });
            }
        }
        CT::ListBox => {
            // The face is the DEVELOPER'S â `BackgroundColor`, the background
            // gradient, the border and the corner radius â drawn by the same
            // call the designer canvas uses, so what is designed is what runs.
            //
            // It used to paint a hardcoded navy surface over the design: a list
            // given a grey-to-black gradient in the RAD came out blue the
            // moment the form ran (operator, 2026-08-18).
            paint::draw_control(&painter, screen.min, ctrl, false, glass, alpha, 1.0, None);
            let mut items: Vec<String> = sv(ctrl, "Items").lines().map(|l| l.to_owned()).collect();
            // `Sorted` — the display order only; what the developer typed stays
            // exactly as they typed it.
            paint::list_display_items(ctrl, &mut items);
            let items = items;
            let cur = sv(ctrl, "Value");
            let mut picked: Option<(usize, String)> = None;
            let mut double_picked: Option<String> = None;
            // The items are egui widgets, so their text came from the AMBIENT
            // visuals â the one colour in this control the developer's
            // ForegroundColor never reached, and the reason a list read as dark
            // grey on a dark theme's well. Painted like the TextBox's text: the
            // control's own colour while it clears WCAG AA on the surface the
            // list actually sits on, otherwise the pole that reads.
            let fg = sv(ctrl, "ForegroundColor");
            let item_color = paint::caret_color(
                paint::control_surface_tone(ui.ctx(), ctrl, form_bg),
                if fg.is_empty() {
                    Color32::WHITE
                } else {
                    paint::parse_color(&fg)
                },
            );
            // Rows painted here rather than assembled from egui widgets: a list
            // row is a full-width band with its own highlight, its own tick box
            // and its own clipping, none of which a `selectable_label` can be
            // talked into. It also took the HOST's chrome metrics â the IDE's
            // 30 px touch height and 8 px gaps â and spaced a list of one-word
            // items like a menu.
            let multi = matches!(sv(ctrl, "MultiSelect").as_str(), "1" | "true");
            let show_checks = matches!(sv(ctrl, "ShowCheckBoxes").as_str(), "1" | "true");
            let lines = |key: &str| -> Vec<String> {
                sv(ctrl, key)
                    .lines()
                    .map(|l| l.to_owned())
                    .filter(|l| !l.is_empty())
                    .collect()
            };
            let mut selected = lines("SelectedItems");
            let mut checked = lines("CheckedItems");
            let mut selection_changed = false;
            let mut checks_changed = false;

            let row_h = crate::model::text_line_height(ctrl) + crate::model::LIST_ROW_PAD * 2.0;
            let content = screen.shrink(crate::model::LIST_FRAME_PAD);
            // The highlight for the ACTIVE row, and the dimmed one every other
            // selected row wears â the same colour, half lit, so a list says
            // which row the cursor is on and which are merely in the selection.
            //
            // Both are the developer's to name (`ActiveItemColor`,
            // `SelectedItemsColor`); left unnamed they are the theme's
            // selection colour and that colour half lit, which is what a list
            // drew before the properties existed. The theme colour is only the
            // FALLBACK, so a form that names them looks the same in the
            // designer's preview, under Run Form and in the compiled binary â
            // three surfaces whose ambient palettes need not agree.
            let (active_fill, selected_fill) =
                paint::list_selection_fills(ctrl, ui.visuals().selection.bg_fill);
            let corner = paint::corner_radius(ctrl);
            // How far the highlight keeps off the border: the border's own width
            // plus a hairline, so the rim reads as a continuous line rather than
            // something the selection has eaten into.
            let border_w = sv(ctrl, "BorderWidth").parse::<f32>().unwrap_or(1.0).max(0.0);
            let inner = screen.shrink(border_w + paint::HIGHLIGHT_INSET);
            let highlight_x = inner.x_range();

            // A drag through the list is ONE gesture with an ANCHOR â the row
            // the press landed on â and what it selects is the range from that
            // anchor to the row under the pointer NOW, worked out afresh every
            // frame. Reversing direction therefore SHRINKS the range.
            //
            // It used to accumulate "every row this press has touched", and
            // never let go of any of them: dragging back up crossed only rows
            // already in the set, so the list stopped answering the drag in
            // either direction (operator, 2026-08-17).
            //
            // Tick boxes keep the crossing model â a sweep ticks each row it
            // crosses once, and crossing it again on the way back must not
            // untick it â so they keep the touched set.
            let sweep_id = ctrl_id.with("listbox-sweep");
            let drag_id = ctrl_id.with("listbox-drag");
            // Where the first row starts on screen, remembered from the frame
            // that drew it. The rows live inside a ScrollArea, so this is what
            // lets a pointer â including one BEYOND either end of the list â be
            // mapped onto a row before the rows are laid out again.
            let geom_id = ctrl_id.with("listbox-first-row");
            let (pointer_down, pointer_pressed, pointer_released, pointer_pos) = ui.input(|i| {
                (
                    i.pointer.primary_down(),
                    i.pointer.primary_pressed(),
                    i.pointer.primary_released(),
                    i.pointer.interact_pos(),
                )
            });
            let first_top: Option<f32> = ui.data(|d| d.get_temp(geom_id));
            let mut touched: Vec<usize> = if pointer_down {
                ui.data(|d| d.get_temp(sweep_id)).unwrap_or_default()
            } else {
                Vec::new()
            };
            // The row under a pointer, CLAMPED to the list: dragging above the
            // first row holds at the first, and below the last at the last, so
            // a drag that leaves the control stops at an end instead of
            // selecting nothing.
            let row_under_pointer = |p: egui::Pos2| -> Option<usize> {
                let top = first_top?;
                if items.is_empty() || row_h <= 0.0 {
                    return None;
                }
                let n = ((p.y - top) / row_h).floor().max(0.0) as usize;
                Some(n.min(items.len() - 1))
            };
            // The row the highlight follows THIS frame. It starts as the
            // committed `Value` and moves with the gesture, so a drag or an
            // arrow key shows immediately rather than a frame late.
            let mut active_item = cur.clone();
            // Where a row that has just been chosen must be scrolled into view.
            let mut reveal: Option<usize> = None;

            // Keyboard navigation. Registered on the control's own id â the one
            // Tab traversal aims at â so egui keeps the focus alive, and
            // registered BEFORE the rows so the rows still own the pointer and
            // a row's double-click still reaches the row.
            let _list_focus = ui.interact(screen, ctrl_id, Sense::click());
            // Whether the press that starts a gesture should also hand the list
            // the keyboard. Acted on AFTER the rows are drawn: a row is a
            // click-sensing widget of its own, so clicking one focuses THAT
            // row, and a request made here would be overwritten by it in the
            // same frame.
            // Press AND release, because egui decides a click on RELEASE: the
            // row focused on the way up would otherwise take the keyboard back
            // from the list one frame after the press handed it over.
            let take_focus = enabled
                && (pointer_pressed || pointer_released)
                && pointer_pos.is_some_and(|p| screen.contains(p));
            // Whether the list is the thing the keyboard is talking to, kept as
            // the list's OWN state rather than read from egui's focus.
            //
            // egui answers a plain arrow key itself, by moving focus to the
            // widget lying in that direction â every row of this list is one, so
            // the first ArrowDown handed the keyboard to a row and the list
            // walked exactly one line and then went deaf. A list owns its
            // arrows; it takes them on a press inside itself (or a Tab onto it)
            // and gives them up on a press elsewhere, or on the Tab that moves
            // to the next control.
            let kb_id = ctrl_id.with("listbox-keyboard");
            let mut has_keyboard: bool = ui.data(|d| d.get_temp(kb_id)).unwrap_or(false);
            if pointer_pressed {
                has_keyboard = pointer_pos.is_some_and(|p| screen.contains(p));
            }
            if ui.input(|i| i.key_pressed(egui::Key::Tab)) {
                has_keyboard = false;
            }
            let listening = has_keyboard || ui.memory(|m| m.has_focus(ctrl_id));
            if enabled && !items.is_empty() && listening {
                let (up, down) = ui.input(|i| {
                    (
                        i.key_pressed(egui::Key::ArrowUp),
                        i.key_pressed(egui::Key::ArrowDown),
                    )
                });
                if up || down {
                    // Tabbed onto: the first arrow is what takes the keyboard,
                    // and it stays taken however egui reassigns focus after it.
                    has_keyboard = true;
                    let at = items.iter().position(|it| it == &active_item);
                    // With nothing chosen yet, the first arrow lands on the
                    // first row rather than jumping to the end of the list.
                    let to = match at {
                        None => 0,
                        Some(i) if up => i.saturating_sub(1),
                        Some(i) => (i + 1).min(items.len() - 1),
                    };
                    if Some(to) != at {
                        active_item = items[to].clone();
                        selected = vec![items[to].clone()];
                        selection_changed = true;
                        picked = Some((to, items[to].clone()));
                        reveal = Some(to);
                    }
                }
            }

            // The pointer gesture, for a list without tick boxes: press sets
            // the anchor, dragging moves the far end.
            let mut drag: Option<(usize, usize)> = if pointer_down {
                ui.data(|d| d.get_temp(drag_id)).unwrap_or(None)
            } else {
                None
            };
            if enabled && !show_checks && !items.is_empty() {
                if pointer_pressed {
                    drag = pointer_pos
                        .filter(|p| screen.contains(*p))
                        .and_then(row_under_pointer)
                        // `usize::MAX` = nothing reported yet, so the press
                        // itself still counts as a move onto the anchor row.
                        .map(|anchor| (anchor, usize::MAX));
                }
                if let (Some((anchor, last)), Some(p)) = (drag, pointer_pos) {
                    // Once the press has landed the gesture belongs to the
                    // list: it follows the pointer even when it wanders off the
                    // side, exactly as it follows one dragged past an end.
                    if let Some(idx) = row_under_pointer(p).filter(|idx| *idx != last) {
                        let additive =
                            multi && ui.input(|i| i.modifiers.command || i.modifiers.ctrl);
                        if additive && idx == anchor && last == usize::MAX {
                            // Ctrl (Cmd on a Mac) click: this row joins or
                            // leaves the selection, and the rest stands.
                            match selected.iter().position(|s| s == &items[idx]) {
                                Some(at) => {
                                    selected.remove(at);
                                }
                                None => selected.push(items[idx].clone()),
                            }
                        } else if multi {
                            let (lo, hi) = (anchor.min(idx), anchor.max(idx));
                            let range = items[lo..=hi].to_vec();
                            if additive {
                                for it in range {
                                    if !selected.contains(&it) {
                                        selected.push(it);
                                    }
                                }
                            } else {
                                selected = range;
                            }
                        } else {
                            selected = vec![items[idx].clone()];
                        }
                        active_item = items[idx].clone();
                        selection_changed = true;
                        picked = Some((idx, items[idx].clone()));
                        reveal = Some(idx);
                        drag = Some((anchor, idx));
                    }
                }
            }
            // Where the first row landed this frame, for the next one's
            // pointer arithmetic.
            let mut first_row_top: Option<f32> = None;

            ui.scope_builder(egui::UiBuilder::new().max_rect(content), |ui| {
                if !enabled {
                    ui.disable();
                }
                egui::ScrollArea::vertical()
                    .id_salt(ctrl_id)
                    .max_height(content.height())
                    // Fill the control instead of shrinking to the content, so
                    // the scrollbar rests against the right border rather than
                    // just past the widest item.
                    .auto_shrink([false, false])
                    // A drag belongs to the SELECTION, not to the viewport: the
                    // list follows the pointer row by row and scrolls only to
                    // keep the row it has reached in view. Were egui's own
                    // drag-to-scroll on as well, the content would slide under
                    // the pointer at the same time and the row under the hand
                    // would run away from it. Off for a mouse by default; this
                    // says so for a touch screen too. The wheel and the
                    // scrollbar still scroll.
                    .scroll_source(egui::containers::scroll_area::ScrollSource {
                        drag: egui::containers::scroll_area::DragScroll::Never,
                        ..Default::default()
                    })
                    .show(ui, |ui| {
                        ui.spacing_mut().item_spacing.y = 0.0;
                        let row_painter = ui.painter().with_clip_rect(screen);
                        for (idx, item) in items.iter().enumerate() {
                            let (row, resp) = ui.allocate_exact_size(
                                vec2(ui.available_width(), row_h),
                                Sense::click(),
                            );
                            if idx == 0 {
                                first_row_top = Some(row.top());
                            }
                            // A row chosen by a drag or an arrow key is brought
                            // into view, landing on the first or last visible
                            // line: the align is `None`, which moves by the
                            // least it can and does nothing at all for a row
                            // already on screen, so this neither fights the
                            // operator's own scrolling nor runs past either end
                            // of the list.
                            //
                            // WITHOUT the animation. A list being walked has to
                            // keep up with the hand: with egui's default eased
                            // scroll the view is still catching up several
                            // frames later, so a fast drag ends with the chosen
                            // row well below the frame â which is exactly the
                            // "I cannot see what is selected" this fixes.
                            if reveal == Some(idx) {
                                ui.scroll_to_rect_animation(
                                    row,
                                    None,
                                    egui::style::ScrollAnimation::none(),
                                );
                            }
                            // The band spans the whole control â a highlight
                            // stops at no inner margin â and is square-cornered
                            // except where it meets the frame's own radius,
                            // which cuts it exactly as the border does.
                            // The band stops just SHORT of the border on every
                            // side â the border stays visible and unbroken, with
                            // a hairline of background between it and the
                            // highlight. Reaching the frame instead painted the
                            // rim away at the first and last row.
                            let band =
                                Rect::from_x_y_ranges(highlight_x, row.y_range()).intersect(inner);
                            let is_active = &active_item == item;
                            let is_selected = selected.iter().any(|s| s == item);
                            if is_active || is_selected && band.is_positive() {
                                // Where the band runs alongside a rounded corner
                                // it is rounded too, by what is left of the
                                // radius once the inset is taken off: egui clips
                                // to an axis-aligned rect, so a square band would
                                // still cut across the arc. The ComboBox's
                                // dropdown draws its bands through the same
                                // helper, so the two cannot drift.
                                row_painter.rect_filled(
                                    band,
                                    paint::highlight_band_rounding(band, inner, corner),
                                    if is_active { active_fill } else { selected_fill },
                                );
                            }

                            // A tick box per row, when the list wants them: what
                            // it holds is a set the user builds by clicking, in
                            // any order and with any gaps.
                            let mut text_x = band.left() + crate::model::LIST_FRAME_PAD + 2.0;
                            let mut check_hit = Rect::NOTHING;
                            if show_checks {
                                let d = (row_h - 4.0).clamp(9.0, 18.0);
                                let box_rect = Rect::from_min_size(
                                    pos2(text_x, band.center().y - d * 0.5),
                                    vec2(d, d),
                                );
                                check_hit = box_rect.expand(3.0);
                                let on = checked.iter().any(|c| c == item);
                                row_painter.rect(
                                    box_rect,
                                    (d * 0.22) as u8,
                                    if on { active_fill } else { Color32::TRANSPARENT },
                                    Stroke::new(1.0, item_color),
                                    egui::StrokeKind::Inside,
                                );
                                if on {
                                    let tick = paint::caret_color(active_fill, item_color);
                                    let p = |ux: f32, uy: f32| {
                                        pos2(
                                            box_rect.left() + ux * d,
                                            box_rect.top() + uy * d,
                                        )
                                    };
                                    let s = Stroke::new((d * 0.14).max(1.2), tick);
                                    row_painter.line_segment([p(0.22, 0.52), p(0.42, 0.74)], s);
                                    row_painter.line_segment([p(0.42, 0.74), p(0.80, 0.26)], s);
                                }
                                text_x = box_rect.right() + 6.0;
                            }

                            let text_colour = if is_active {
                                paint::caret_color(active_fill, item_color)
                            } else {
                                item_color
                            };
                            row_painter.text(
                                pos2(text_x, band.center().y),
                                Align2::LEFT_CENTER,
                                item,
                                crate::fonts::font_id(
                                    ui.ctx(),
                                    &sv(ctrl, "FontName"),
                                    paint::ctrl_font_size(ctrl),
                                ),
                                text_colour,
                            );

                            // Tick boxes: a row is ticked while the button is
                            // DOWN over it, not on release â that is what makes
                            // a press-and-sweep tick a run of rows â and each
                            // row only once per gesture, so resting on one, or
                            // crossing it again on the way back, does not
                            // un-tick what the sweep just ticked.
                            //
                            // Plain selection is not decided here: it follows an
                            // anchor worked out for the whole list above, which
                            // is what lets a drag reverse.
                            let over = show_checks
                                && pointer_down
                                && pointer_pos.is_some_and(|p| {
                                    Rect::from_x_y_ranges(screen.x_range(), row.y_range())
                                        .contains(p)
                                });
                            let _ = &check_hit;
                            if enabled && over && !touched.contains(&idx) {
                                touched.push(idx);
                                // The boxes ARE the multiple selection, so a
                                // plain click anywhere on the row ticks it, and
                                // ticking it again clears it.
                                match checked.iter().position(|c| c == item) {
                                    Some(at) => {
                                        checked.remove(at);
                                    }
                                    None => checked.push(item.clone()),
                                }
                                checks_changed = true;
                                picked = Some((idx, item.clone()));
                            }
                            if resp.double_clicked() && enabled {
                                double_picked = Some(item.clone());
                            }
                        }
                    });
            });

            if take_focus {
                ui.memory_mut(|m| m.request_focus(ctrl_id));
            }
            ui.data_mut(|d| {
                d.insert_temp(sweep_id, touched);
                d.insert_temp(drag_id, drag);
                d.insert_temp(kb_id, has_keyboard);
                if let Some(top) = first_row_top {
                    d.insert_temp(geom_id, top);
                }
            });
            if let Some((idx, item)) = picked {
                out.prop_updates
                    .push((id.to_owned(), "Value".to_owned(), item.clone()));
                out.prop_updates
                    .push((id.to_owned(), "SelectedIndex".to_owned(), idx.to_string()));
                out.events.push(UiEvent::change(id, &item));
                out.events.push(UiEvent::ev(id, "onSelectedIndexChanged"));
            }
            if selection_changed {
                out.prop_updates.push((
                    id.to_owned(),
                    "SelectedItems".to_owned(),
                    selected.join("\n"),
                ));
            }
            if checks_changed {
                out.prop_updates.push((
                    id.to_owned(),
                    "CheckedItems".to_owned(),
                    checked.join("\n"),
                ));
                out.events
                    .push(UiEvent::with_value(id, "onItemChecked", &checked.join("\n")));
            }
            if let Some(item) = double_picked {
                // spec 021 T12: item-level double click with the item text.
                out.events
                    .push(UiEvent::with_value(id, "onItemDoubleClick", &item));
            }
        }
        CT::DateTimePicker => {
            let white = Color32::from_rgb(230, 235, 255);
            let dim = Color32::from_rgb(150, 160, 200);
            paint::draw_control(&painter, screen.min, ctrl, false, glass, alpha, 1.0, None);
            let val = sv(ctrl, "Value");
            let resp = ui.interact(screen, ctrl_id, Sense::click());
            focus_keyboard_events(ui, &resp, id, out, &bound);

            let mut cal: paint::CalState = ui
                .data(|d| d.get_temp::<paint::CalState>(ctrl_id))
                .unwrap_or_else(|| match paint::parse_ymd(&val) {
                    Some((y, m, _)) => paint::CalState {
                        open: false,
                        year: y,
                        month: m,
                    },
                    None => paint::CalState::default(),
                });
            if resp.clicked() && enabled {
                cal.open = !cal.open;
            }
            if cal.open {
                let area_pos = screen.left_bottom() + vec2(0.0, 2.0);
                let inner = egui::Area::new(ctrl_id.with("cal"))
                    .order(egui::Order::Foreground)
                    .fixed_pos(area_pos)
                    .show(ui.ctx(), |ui| {
                        let area_rect = Rect::from_min_size(
                            area_pos,
                            vec2(paint::CAL_W, paint::CAL_GRID_Y + paint::CAL_CELL * 6.0),
                        );
                        let p = ui.painter();
                        p.rect_filled(area_rect, 6.0, Color32::from_rgb(28, 34, 60));
                        p.rect_stroke(
                            area_rect,
                            6.0,
                            Stroke::new(1.0, Color32::from_rgba_premultiplied(160, 170, 230, 150)),
                            egui::StrokeKind::Middle,
                        );
                        let prev = ui.put(
                            Rect::from_min_size(area_pos, vec2(paint::CAL_CELL, paint::CAL_NAV_H)),
                            egui::Button::new("â").frame(false),
                        );
                        let next = ui.put(
                            Rect::from_min_size(
                                area_pos + vec2(paint::CAL_W - paint::CAL_CELL, 0.0),
                                vec2(paint::CAL_CELL, paint::CAL_NAV_H),
                            ),
                            egui::Button::new("â¶").frame(false),
                        );
                        ui.painter().text(
                            area_pos + vec2(paint::CAL_W / 2.0, paint::CAL_NAV_H / 2.0),
                            Align2::CENTER_CENTER,
                            format!(
                                "{} {}",
                                paint::MONTHS[(cal.month.clamp(1, 12) - 1) as usize],
                                cal.year
                            ),
                            FontId::proportional(13.0),
                            white,
                        );
                        if prev.clicked() {
                            if cal.month == 1 {
                                cal.month = 12;
                                cal.year -= 1;
                            } else {
                                cal.month -= 1;
                            }
                        }
                        if next.clicked() {
                            if cal.month == 12 {
                                cal.month = 1;
                                cal.year += 1;
                            } else {
                                cal.month += 1;
                            }
                        }
                        for (i, wd) in ["S", "M", "T", "W", "T", "F", "S"].iter().enumerate() {
                            ui.painter().text(
                                area_pos
                                    + vec2(
                                        i as f32 * paint::CAL_CELL + paint::CAL_CELL / 2.0,
                                        paint::CAL_NAV_H + paint::CAL_WK_H / 2.0,
                                    ),
                                Align2::CENTER_CENTER,
                                *wd,
                                FontId::proportional(10.0),
                                dim,
                            );
                        }
                        let first_wd = paint::day_of_week(cal.year, cal.month, 1);
                        let ndays = paint::days_in_month(cal.year, cal.month);
                        let mut picked: Option<u32> = None;
                        for day in 1..=ndays {
                            let idx = first_wd + (day - 1);
                            let (col, row) = (idx % 7, idx / 7);
                            let cell = Rect::from_min_size(
                                area_pos
                                    + vec2(
                                        col as f32 * paint::CAL_CELL,
                                        paint::CAL_GRID_Y + row as f32 * paint::CAL_CELL,
                                    ),
                                vec2(paint::CAL_CELL, paint::CAL_CELL),
                            );
                            if ui
                                .put(cell, egui::Button::new(format!("{day}")).frame(false))
                                .clicked()
                            {
                                picked = Some(day);
                            }
                        }
                        picked
                    });
                if let Some(day) = inner.inner {
                    let date = format!("{:04}-{:02}-{:02}", cal.year, cal.month, day);
                    out.prop_updates
                        .push((id.to_owned(), "Value".to_owned(), date.clone()));
                    out.events.push(UiEvent::change(id, &date));
                    cal.open = false;
                } else if !resp.clicked() && inner.response.clicked_elsewhere() {
                    cal.open = false;
                }
            }
            ui.data_mut(|d| d.insert_temp(ctrl_id, cal));
        }
        CT::DataGrid => {
            let painter = painter.with_clip_rect(painter.clip_rect().intersect(screen));
            // Cell text honours the grid's ForegroundColor when explicitly set,
            // falling back to the readable default (previously hardcoded).
            let cell_fg = {
                let raw = sv(ctrl, "ForegroundColor");
                let default_fg = crate::model::DEFAULT_FOREGROUND_COLOR.trim_start_matches('#');
                paint::parse_hex(&raw)
                    .filter(|c| {
                        c.a() > 0
                            && !raw
                                .trim()
                                .trim_start_matches('#')
                                .eq_ignore_ascii_case(default_fg)
                    })
                    .unwrap_or(Color32::from_rgb(225, 230, 250))
            };
            let columns_raw = sv(ctrl, "Columns");
            let rows_raw = sv(ctrl, "Rows");
            let cols: Vec<(String, String)> = columns_raw
                .lines()
                .filter_map(|l| {
                    let mut it = l.splitn(2, ':');
                    let name = it.next().unwrap_or("").trim().to_owned();
                    if name.is_empty() {
                        return None;
                    }
                    let ty = it.next().unwrap_or("string").trim().to_lowercase();
                    Some((name, ty))
                })
                .collect();
            let rows: Vec<Vec<String>> = rows_raw
                .lines()
                .filter(|l| !l.is_empty())
                .map(|l| l.split('\t').map(|c| c.to_owned()).collect())
                .collect();
            let row_h = sv(ctrl, "RowHeight")
                .parse::<f32>()
                .unwrap_or(22.0)
                .clamp(14.0, 60.0);
            let advanced_grid = DataGridAdvanced::from_control(ctrl);
            let display_cols: Vec<(usize, String, String)> = if advanced_grid.columns.is_empty() {
                cols.iter()
                    .enumerate()
                    .map(|(i, (name, ty))| (i, name.clone(), ty.clone()))
                    .collect()
            } else {
                // Include ALL advanced columns so that "non-data-bound" columns
                // (those without a matching source in the bound Columns data) are
                // still rendered and receive their appearance settings (background etc).
                advanced_grid
                    .columns
                    .iter()
                    .map(|column| {
                        let source_index = cols
                            .iter()
                            .position(|(name, _)| {
                                name.eq_ignore_ascii_case(&column.source_name)
                                    || name.eq_ignore_ascii_case(&column.title)
                                    || name.eq_ignore_ascii_case(&column.id)
                            })
                            .unwrap_or(usize::MAX);
                        let ty = if source_index < cols.len() {
                            cols[source_index].1.clone()
                        } else {
                            "string".to_string()
                        };
                        (source_index, column.title.clone(), ty)
                    })
                    .collect()
            };
            let display_cols: Vec<(usize, String, String)> = if display_cols.is_empty() {
                vec![(0, String::new(), "string".into())]
            } else {
                display_cols
            };
            let source_names: Vec<String> = cols.iter().map(|(name, _)| name.clone()).collect();
            let displayed_row_indices =
                advanced_grid.filtered_row_indices_for_sources(&rows, &source_names);
            let ncols = display_cols.len().max(1);
            let col_w = screen.width() / ncols as f32;
            let frozen_columns = advanced_grid.frozen_columns.min(ncols);
            let frozen_rows = advanced_grid.frozen_rows.min(displayed_row_indices.len());
            let column_measures: Vec<DataGridColumnMeasure> = (0..ncols)
                .map(|i| DataGridColumnMeasure {
                    width: advanced_grid.column_width(i).unwrap_or(col_w).max(32.0),
                    frozen: advanced_grid
                        .columns
                        .get(i)
                        .map(|column| column.frozen)
                        .unwrap_or(false),
                })
                .collect();
            let column_widths: Vec<f32> =
                column_measures.iter().map(|column| column.width).collect();
            // 047 â under Elegance the *defaults* come from the palette, so a
            // themed grid reads as one surface instead of a themed frame around
            // built-in blues. An explicitly set property still wins (R8).
            use crate::surface_theme::ColorToken as Tok;
            let header_bg = paint::parse_hex(&sv(ctrl, "HeaderBackgroundColor")).unwrap_or_else(
                || {
                    paint::theme_token(painter.ctx(), Tok::CardRaised)
                        .unwrap_or(Color32::from_rgb(60, 66, 96))
                },
            );
            let header_fg = paint::parse_hex(&sv(ctrl, "HeaderForegroundColor")).unwrap_or_else(
                || {
                    paint::theme_token(painter.ctx(), Tok::Text)
                        .unwrap_or(Color32::from_rgb(235, 238, 250))
                },
            );
            // The DataGrid is the one control that supports a solid grid background
            // (grid/column/row/cell fine control). A user-chosen colour paints solid
            // beneath the glass; a grid still on the default sentinel stays fully
            // translucent Liquid Glass like every other control.
            let raw_grid_bg = sv(ctrl, "BackgroundColor");
            let default_bg = crate::model::DEFAULT_BACKGROUND_COLOR.trim_start_matches('#');
            let grid_bg_underlay = paint::parse_hex(&raw_grid_bg).filter(|c| {
                c.a() > 0
                    && !raw_grid_bg
                        .trim()
                        .trim_start_matches('#')
                        .eq_ignore_ascii_case(default_bg)
            });
            let grid_bg = grid_bg_underlay.unwrap_or_else(|| {
                paint::theme_token(painter.ctx(), Tok::InputBg)
                    .unwrap_or(Color32::from_rgb(26, 32, 58))
            });
            let alt_bg_base = paint::parse_hex(&sv(ctrl, "AlternatingRowColor")).unwrap_or_else(
                || {
                    paint::theme_token(painter.ctx(), Tok::CardRaised)
                        .unwrap_or(Color32::from_rgb(38, 44, 72))
                },
            );
            let alt_bg_opacity = sv(ctrl, "AlternatingRowOpacity")
                .parse::<u8>()
                .unwrap_or(20)
                .min(100);
            let alt_bg = Color32::from_rgba_unmultiplied(
                alt_bg_base.r(),
                alt_bg_base.g(),
                alt_bg_base.b(),
                ((alt_bg_opacity as u16 * 255) / 100) as u8,
            );
            // Which axis the alternating highlight is applied to: every other row
            // (default / legacy), every other column, or off. Unknown values fall
            // back to rows so existing forms are unchanged.
            let alt_axis = sv(ctrl, "AlternatingMode").trim().to_ascii_lowercase();
            let alt_none = matches!(alt_axis.as_str(), "none" | "off");
            let alt_cols = matches!(alt_axis.as_str(), "columns" | "column");
            let alt_rows = !alt_none && !alt_cols;
            // Grid-line colour is the DataGrid's "foreground": the Appearance
            // section's Fore color drives it. A grid still on the default
            // foreground sentinel uses the subtle built-in colour; the legacy
            // per-grid `GridLineColor` is honoured as a fallback for older forms.
            // (The grid background from appearance is used for the under-fill
            // and column areas; separators use this line colour.)
            let raw_fg = sv(ctrl, "ForegroundColor");
            let default_fg = crate::model::DEFAULT_FOREGROUND_COLOR.trim_start_matches('#');
            let fg_line_color = paint::parse_hex(&raw_fg).filter(|c| {
                c.a() > 0
                    && !raw_fg
                        .trim()
                        .trim_start_matches('#')
                        .eq_ignore_ascii_case(default_fg)
            });
            let grid_c = fg_line_color
                .or_else(|| paint::parse_hex(&sv(ctrl, "GridLineColor")))
                .unwrap_or(Color32::from_rgba_premultiplied(150, 160, 200, 90));
            let grid_line_style = advanced_grid.grid_line_style;
            let font_size = sv(ctrl, "FontSize")
                .parse::<f32>()
                .unwrap_or(12.0)
                .clamp(6.0, 72.0);
            let show_filters = prop_bool(ctrl, "ShowColumnFilters", false);
            let header_h = if show_filters {
                (row_h * 1.85).max(row_h + 18.0)
            } else {
                row_h
            };
            let frozen_rows_height = row_h * frozen_rows as f32;
            let scrollable_row_count = displayed_row_indices.len().saturating_sub(frozen_rows);

            paint::draw_surface_auto_bg(
                &painter,
                screen,
                grid_bg,
                grid_bg_underlay,
                paint::corner_radius(ctrl),
                false,
                alpha,
                paint::SurfaceRole::Input,
            );
            let bg_image = sv(ctrl, "GridBackgroundImage");
            if !bg_image.trim().is_empty() {
                let image_id = egui::Id::new(("dg_bg_img", &ctrl.id, bg_image.as_str()));
                let tex = match ui.data(|d| d.get_temp::<Option<egui::TextureHandle>>(image_id)) {
                    Some(t) => t,
                    None => {
                        let loaded = paint::load_image_texture(ui.ctx(), bg_image.trim());
                        ui.data_mut(|d| d.insert_temp(image_id, loaded.clone()));
                        loaded
                    }
                };
                if let Some(tex) = tex {
                    let mode = BgImageMode::from_str(&sv(ctrl, "GridBackgroundImageMode"));
                    let dest = image_dest(screen, tex.size_vec2(), mode);
                    // Clip the background image to the grid's rounded silhouette so it
                    // doesn't square off past the corner arcs (spec 027 corner bleed).
                    let visible = dest.intersect(screen);
                    if visible.width() > 0.5 && visible.height() > 0.5 {
                        let dw = dest.width().max(1.0);
                        let dh = dest.height().max(1.0);
                        let uv = Rect::from_min_max(
                            pos2(
                                (visible.min.x - dest.min.x) / dw,
                                (visible.min.y - dest.min.y) / dh,
                            ),
                            pos2(
                                (visible.max.x - dest.min.x) / dw,
                                (visible.max.y - dest.min.y) / dh,
                            ),
                        );
                        // Round only the corners where the image actually reaches a
                        // grid corner, so a smaller centred image is left square.
                        let r = paint::cr8(paint::corner_radius(ctrl));
                        let eps = 0.5;
                        let corner = |vx: f32, sx: f32, vy: f32, sy: f32| {
                            if (vx - sx).abs() < eps && (vy - sy).abs() < eps {
                                r
                            } else {
                                0
                            }
                        };
                        let rounding = egui::CornerRadius {
                            nw: corner(visible.min.x, screen.min.x, visible.min.y, screen.min.y),
                            ne: corner(visible.max.x, screen.max.x, visible.min.y, screen.min.y),
                            sw: corner(visible.min.x, screen.min.x, visible.max.y, screen.max.y),
                            se: corner(visible.max.x, screen.max.x, visible.max.y, screen.max.y),
                        };
                        painter.add(egui::Shape::Rect(
                            egui::epaint::RectShape::new(
                                visible,
                                rounding,
                                Color32::from_rgba_unmultiplied(
                                    255,
                                    255,
                                    255,
                                    (alpha * 255.0) as u8,
                                ),
                                Stroke::NONE,
                                egui::StrokeKind::Middle,
                            )
                            .with_texture(tex.id(), uv),
                        ));
                    }
                }
            }
            draw_datagrid_pattern(
                &painter,
                screen,
                paint::corner_radius(ctrl),
                &sv(ctrl, "GridBackgroundPattern"),
                Color32::from_rgba_unmultiplied(255, 255, 255, 24),
            );
            let scroll_id = ctrl_id.with("datagrid-scroll-y");
            let scroll_x_id = ctrl_id.with("datagrid-scroll-x");
            let selection_id = ctrl_id.with("datagrid-selection");
            let mut scroll_y = ui
                .ctx()
                .memory(|m| m.data.get_temp::<f32>(scroll_id).unwrap_or(0.0));
            let mut scroll_x = ui
                .ctx()
                .memory(|m| m.data.get_temp::<f32>(scroll_x_id).unwrap_or(0.0));
            let mut layout = DataGridLayout::compute(&DataGridLayoutInput {
                width: screen.width(),
                height: (screen.height() - frozen_rows_height).max(header_h),
                row_count: scrollable_row_count,
                columns: column_measures.clone(),
                row_height: row_h,
                header_height: header_h,
                frozen_columns,
                frozen_rows: 0,
                scroll_x,
                scroll_y,
                row_buffer: 2,
            });
            scroll_y = layout.scroll_y;
            scroll_x = layout.scroll_x;
            let header_rect = Rect::from_min_size(screen.min, vec2(screen.width(), header_h));
            let body_rect = Rect::from_min_max(pos2(screen.min.x, header_rect.max.y), screen.max);
            // While the pointer is anywhere over the DataGrid, the grid owns the
            // wheel: read AND *consume* the wheel so it never bleeds into the
            // containing ScrollArea (GroupBox / form). We remove the MouseWheel
            // events (for any event-based consumer) and zero this frame's scroll
            // deltas â the ancestor ScrollArea reads `smooth_scroll_delta` in its
            // `end()`, which runs after this content, so zeroing it here stops it.
            // Consumption is unconditional over the grid (even when the grid has
            // no overflow) so scrolling never leaks to the container; the clamps
            // below make the applied scroll a no-op when there's nothing to move.
            if ui.rect_contains_pointer(screen) {
                let (wheel_delta_x, wheel_delta_y) = ui.input_mut(|i| {
                    let mut dx = 0.0_f32;
                    let mut dy = 0.0_f32;
                    i.events.retain(|event| match event {
                        egui::Event::MouseWheel { delta, .. } => {
                            dx += delta.x;
                            dy += delta.y;
                            false // consumed by the DataGrid â do not bubble up
                        }
                        _ => true,
                    });
                    i.smooth_scroll_delta = egui::Vec2::ZERO;
                    (dx, dy)
                });
                if wheel_delta_y != 0.0 {
                    scroll_y = (scroll_y - wheel_delta_y).clamp(0.0, layout.max_scroll_y);
                }
                if wheel_delta_x != 0.0 {
                    scroll_x = (scroll_x - wheel_delta_x).clamp(0.0, layout.max_scroll_x);
                }
            }
            layout = DataGridLayout::compute(&DataGridLayoutInput {
                width: screen.width(),
                height: (screen.height() - frozen_rows_height).max(header_h),
                row_count: scrollable_row_count,
                columns: column_measures.clone(),
                row_height: row_h,
                header_height: header_h,
                frozen_columns,
                frozen_rows: 0,
                scroll_x,
                scroll_y,
                row_buffer: 2,
            });
            scroll_y = layout.scroll_y;
            scroll_x = layout.scroll_x;
            ui.ctx()
                .memory_mut(|m| m.data.insert_temp(scroll_id, scroll_y));
            ui.ctx()
                .memory_mut(|m| m.data.insert_temp(scroll_x_id, scroll_x));
            // spec 021 T12: onScroll whenever this frame's settled offset moved
            // (wheel, scrollbar drag, or keyboard row navigation alike).
            {
                let last_id = ctrl_id.with("dg-last-scroll");
                let last = ui
                    .ctx()
                    .memory(|m| m.data.get_temp::<(f32, f32)>(last_id));
                if let Some((ly, lx)) = last {
                    if (ly - scroll_y).abs() > 0.5 || (lx - scroll_x).abs() > 0.5 {
                        out.events.push(UiEvent::with_value(
                            id,
                            "onScroll",
                            &format!("{scroll_y:.0},{scroll_x:.0}"),
                        ));
                    }
                }
                if last != Some((scroll_y, scroll_x)) {
                    ui.ctx()
                        .memory_mut(|m| m.data.insert_temp(last_id, (scroll_y, scroll_x)));
                }
            }
            // spec 021 T12: column-header clicks with the display column index.
            {
                let mut x = header_rect.min.x - scroll_x;
                for (display_index, measure) in column_measures.iter().enumerate() {
                    let col_header = Rect::from_min_max(
                        pos2(x.max(header_rect.min.x), header_rect.min.y),
                        pos2((x + measure.width).min(header_rect.max.x), header_rect.max.y),
                    );
                    x += measure.width;
                    if col_header.width() <= 0.0 {
                        continue;
                    }
                    if ui
                        .interact(
                            col_header,
                            ctrl_id.with(("dg-colhdr", display_index)),
                            Sense::click(),
                        )
                        .clicked()
                        && enabled
                    {
                        out.events.push(UiEvent::with_value(
                            id,
                            "onColumnClick",
                            &display_index.to_string(),
                        ));
                    }
                }
            }
            let selected_cell = ui
                .ctx()
                .memory(|m| m.data.get_temp::<DataGridCellSelection>(selection_id));
            if let Some(selected_cell) = selected_cell {
                if ui.input(|i| i.key_pressed(egui::Key::C) && i.modifiers.command) {
                    let visible_source_columns: Vec<usize> = display_cols
                        .iter()
                        .map(|(source_index, _, _)| *source_index)
                        .collect();
                    if let Some(text) = datagrid_copy_text(
                        &rows,
                        &visible_source_columns,
                        selected_cell,
                        &sv(ctrl, "SelectionMode"),
                        &sv(ctrl, "CSVDelimiter"),
                    ) {
                        ui.ctx().copy_text(text);
                    }
                }
            }

            let grid_focus = ui.interact(screen, ctrl_id.with("datagrid-focus"), Sense::click());
            if grid_focus.clicked() {
                grid_focus.request_focus();
            }
            if enabled && grid_focus.has_focus() && !displayed_row_indices.is_empty() && ncols > 0 {
                let key_state = ui.input(|i| {
                    (
                        i.key_pressed(egui::Key::ArrowUp),
                        i.key_pressed(egui::Key::ArrowDown),
                        i.key_pressed(egui::Key::ArrowLeft),
                        i.key_pressed(egui::Key::ArrowRight),
                        i.key_pressed(egui::Key::PageUp),
                        i.key_pressed(egui::Key::PageDown),
                        i.key_pressed(egui::Key::Home),
                        i.key_pressed(egui::Key::End),
                        i.modifiers.command || i.modifiers.ctrl,
                        i.modifiers.shift,
                    )
                });
                let (
                    up,
                    down,
                    left,
                    right_key,
                    page_up,
                    page_down,
                    home,
                    end,
                    command_or_ctrl,
                    shift,
                ) = key_state;
                if up || down || left || right_key || page_up || page_down || home || end {
                    let selected = ui
                        .ctx()
                        .memory(|m| m.data.get_temp::<DataGridCellSelection>(selection_id))
                        .unwrap_or(DataGridCellSelection {
                            row_index: displayed_row_indices[0],
                            display_column_index: 0,
                        });
                    let mut display_row = displayed_row_indices
                        .iter()
                        .position(|row_index| *row_index == selected.row_index)
                        .unwrap_or(0);
                    let mut display_col = selected.display_column_index.min(ncols - 1);
                    let page_rows = ((body_rect.height() / row_h).floor() as usize).max(1);

                    if command_or_ctrl && up {
                        display_row = 0;
                    } else if up {
                        display_row = display_row.saturating_sub(1);
                    }
                    if command_or_ctrl && down {
                        display_row = displayed_row_indices.len() - 1;
                    } else if down {
                        display_row = (display_row + 1).min(displayed_row_indices.len() - 1);
                    }
                    if command_or_ctrl && left {
                        display_col = 0;
                    } else if left {
                        display_col = display_col.saturating_sub(1);
                    }
                    if command_or_ctrl && right_key {
                        display_col = ncols - 1;
                    } else if right_key {
                        display_col = (display_col + 1).min(ncols - 1);
                    }
                    if page_up {
                        display_row = display_row.saturating_sub(page_rows);
                    }
                    if page_down {
                        display_row =
                            (display_row + page_rows).min(displayed_row_indices.len() - 1);
                    }
                    if command_or_ctrl && home {
                        display_row = 0;
                        display_col = 0;
                    } else if home {
                        display_col = 0;
                    }
                    if command_or_ctrl && end {
                        display_row = displayed_row_indices.len() - 1;
                        display_col = ncols - 1;
                    } else if end {
                        display_col = ncols - 1;
                    }

                    let new_selection = DataGridCellSelection {
                        row_index: displayed_row_indices[display_row],
                        display_column_index: display_col,
                    };
                    if shift && selected == new_selection {
                        ui.ctx()
                            .memory_mut(|m| m.data.remove::<DataGridCellSelection>(selection_id));
                    } else {
                        ui.ctx()
                            .memory_mut(|m| m.data.insert_temp(selection_id, new_selection));
                    }
                    // Fire selection events when the selection actually moved.
                    if new_selection != selected {
                        out.events.push(UiEvent::ev(id, "onSelectionChanged"));
                        if new_selection.row_index != selected.row_index {
                            out.events.push(UiEvent::ev(id, "onRowSelect"));
                        }
                    }

                    if display_row >= frozen_rows {
                        let scroll_row = display_row - frozen_rows;
                        let visible_rows = (((body_rect.height() - frozen_rows_height).max(row_h)
                            / row_h)
                            .floor() as usize)
                            .max(1);
                        if scroll_row < layout.first_row {
                            scroll_y = scroll_row as f32 * row_h;
                        } else if scroll_row >= layout.first_row + visible_rows {
                            scroll_y = ((scroll_row + 1) as f32 * row_h
                                - (body_rect.height() - frozen_rows_height))
                                .max(0.0);
                        }
                    }

                    if display_col >= frozen_columns {
                        let target_left: f32 = column_widths.iter().take(display_col).sum();
                        let target_right = target_left + column_widths[display_col];
                        let visible_left = layout.frozen_columns_width + scroll_x;
                        let visible_right = screen.width() + scroll_x;
                        if target_left < visible_left {
                            scroll_x = (target_left - layout.frozen_columns_width).max(0.0);
                        } else if target_right > visible_right {
                            scroll_x = (target_right - screen.width()).max(0.0);
                        }
                    }

                    layout = DataGridLayout::compute(&DataGridLayoutInput {
                        width: screen.width(),
                        height: (screen.height() - frozen_rows_height).max(header_h),
                        row_count: scrollable_row_count,
                        columns: column_measures.clone(),
                        row_height: row_h,
                        header_height: header_h,
                        frozen_columns,
                        frozen_rows: 0,
                        scroll_x,
                        scroll_y,
                        row_buffer: 2,
                    });
                    scroll_y = layout.scroll_y;
                    scroll_x = layout.scroll_x;
                    ui.ctx()
                        .memory_mut(|m| m.data.insert_temp(scroll_id, scroll_y));
                    ui.ctx()
                        .memory_mut(|m| m.data.insert_temp(scroll_x_id, scroll_x));
                }
            }

            if enabled && prop_bool(ctrl, "AllowColumnResize", true) {
                for col in layout
                    .frozen_columns
                    .iter()
                    .chain(layout.scrollable_columns.iter())
                {
                    let edge_x = screen.min.x + col.x + col.width;
                    if edge_x <= screen.min.x || edge_x >= screen.max.x {
                        continue;
                    }
                    let handle = Rect::from_min_max(
                        pos2(edge_x - 3.0, header_rect.min.y),
                        pos2(edge_x + 3.0, body_rect.max.y),
                    );
                    let resp = ui.interact(
                        handle,
                        ctrl_id.with(("dg-col-resize", col.index)),
                        Sense::drag(),
                    );
                    if resp.hovered() || resp.dragged() {
                        ui.ctx()
                            .output_mut(|o| o.cursor_icon = egui::CursorIcon::ResizeHorizontal);
                    }
                    if resp.dragged() {
                        let mut resized = DataGridAdvanced::from_control(ctrl);
                        let new_width = (col.width + resp.drag_delta().x).clamp(32.0, 1600.0);
                        resized.set_column_width(col.index, new_width);
                        if let Ok(json) = resized.to_json() {
                            out.prop_updates.push((
                                id.to_owned(),
                                DATAGRID_ADVANCED_PROP.to_owned(),
                                json,
                            ));
                        }
                    }
                }
            }

            if enabled && prop_bool(ctrl, "AllowRowResize", true) {
                let mut row_handles = Vec::new();
                row_handles.push((None, screen.min.y + layout.header_rect.max_y()));
                for frozen_row in 0..frozen_rows {
                    let edge_y = body_rect.min.y + row_h * (frozen_row + 1) as f32;
                    if edge_y > body_rect.min.y && edge_y < body_rect.max.y {
                        row_handles.push((Some(frozen_row), edge_y));
                    }
                }
                let scroll_rows_min_y = (body_rect.min.y + frozen_rows_height).min(body_rect.max.y);
                for r in layout.first_row..layout.last_row_exclusive {
                    let display_row = frozen_rows + r;
                    let edge_y = scroll_rows_min_y + row_h * (r + 1) as f32 - scroll_y;
                    if edge_y > body_rect.min.y && edge_y < body_rect.max.y {
                        row_handles.push((Some(display_row), edge_y));
                    }
                }
                for (row_index, edge_y) in row_handles {
                    let handle = Rect::from_min_max(
                        pos2(screen.min.x, edge_y - 3.0),
                        pos2(screen.max.x, edge_y + 3.0),
                    );
                    let resp = ui.interact(
                        handle,
                        ctrl_id.with(("dg-row-resize", row_index)),
                        Sense::drag(),
                    );
                    if resp.hovered() || resp.dragged() {
                        ui.ctx()
                            .output_mut(|o| o.cursor_icon = egui::CursorIcon::ResizeVertical);
                    }
                    if resp.dragged() {
                        let new_height = (row_h + resp.drag_delta().y).clamp(14.0, 120.0);
                        out.prop_updates.push((
                            id.to_owned(),
                            "RowHeight".to_owned(),
                            format!("{:.0}", new_height),
                        ));
                    }
                }
            }

            let header_radius = paint::corner_radius(ctrl) as f32;
            painter.rect_filled(
                header_rect,
                egui::CornerRadius {
                    nw: crate::paint::cr8(header_radius),
                    ne: crate::paint::cr8(header_radius),
                    sw: 0,
                    se: 0,
                },
                header_bg,
            );
            for col in layout
                .frozen_columns
                .iter()
                .chain(layout.scrollable_columns.iter())
            {
                let (_, name, _) = &display_cols[col.index];
                let column_meta = advanced_grid.columns.get(col.index);
                let x = screen.min.x + col.x;
                // Clip a scrollable header cell to the region right of the frozen
                // band so it scrolls behind the frozen columns (matches the body).
                let painter = if col.frozen {
                    painter.clone()
                } else {
                    painter.with_clip_rect(Rect::from_min_max(
                        pos2(
                            screen.min.x + layout.frozen_columns_width,
                            header_rect.min.y,
                        ),
                        pos2(screen.max.x, header_rect.max.y),
                    ))
                };
                let cell_rect =
                    Rect::from_min_size(pos2(x, header_rect.min.y), vec2(col.width, header_h))
                        .shrink2(vec2(2.0, 0.0));
                let title_y = if show_filters {
                    header_rect.min.y + (header_h * 0.32)
                } else {
                    header_rect.center().y
                };
                let header_font_size = column_meta
                    .map(|column| column.header_font_size.max(6) as f32)
                    .unwrap_or(12.0);
                painter.with_clip_rect(cell_rect).text(
                    pos2(x + col.width * 0.5, title_y),
                    Align2::CENTER_CENTER,
                    name,
                    FontId::proportional(header_font_size),
                    header_fg,
                );
                if show_filters {
                    let filter_key = column_meta
                        .map(|column| {
                            if !column.id.trim().is_empty() {
                                column.id.as_str()
                            } else if !column.source_name.trim().is_empty() {
                                column.source_name.as_str()
                            } else {
                                name.as_str()
                            }
                        })
                        .unwrap_or(name.as_str());
                    let filter_value = advanced_grid
                        .filters
                        .iter()
                        .find(|filter| {
                            filter.column_id.eq_ignore_ascii_case(filter_key)
                                || filter.column_id.eq_ignore_ascii_case(name)
                                || column_meta
                                    .map(|column| {
                                        filter.column_id.eq_ignore_ascii_case(&column.id)
                                            || filter
                                                .column_id
                                                .eq_ignore_ascii_case(&column.source_name)
                                    })
                                    .unwrap_or(false)
                        })
                        .map(|filter| filter.value.as_str())
                        .unwrap_or("");
                    let filter_rect = Rect::from_center_size(
                        pos2(x + col.width * 0.5, header_rect.min.y + header_h * 0.72),
                        vec2((col.width - 12.0).max(16.0), (row_h * 0.68).max(14.0)),
                    );
                    painter.rect_filled(
                        filter_rect,
                        3.0,
                        Color32::from_rgba_unmultiplied(0, 0, 0, 120),
                    );
                    let mut filter_text = filter_value.to_owned();
                    // The filter input is an egui widget (not painter-drawn), so it
                    // isn't covered by the header painter clip. For a scrollable
                    // column, restrict the ui clip to the region right of the frozen
                    // band so the input scrolls behind the frozen columns instead of
                    // drawing over them.
                    let prev_clip = ui.clip_rect();
                    if !col.frozen {
                        let scrollable = Rect::from_min_max(
                            pos2(
                                screen.min.x + layout.frozen_columns_width,
                                header_rect.min.y,
                            ),
                            pos2(screen.max.x, header_rect.max.y),
                        );
                        ui.set_clip_rect(prev_clip.intersect(scrollable));
                    }
                    let filter_response = ui.put(
                        filter_rect.shrink2(vec2(4.0, 1.0)),
                        egui::TextEdit::singleline(&mut filter_text)
                            .hint_text("Filter...")
                            .font(FontId::proportional((font_size - 1.0).max(8.0)))
                            .desired_width((col.width - 18.0).max(16.0))
                            .frame(egui::Frame::NONE),
                    );
                    if !col.frozen {
                        ui.set_clip_rect(prev_clip);
                    }
                    if filter_response.changed() {
                        let mut updated = advanced_grid.clone();
                        updated.set_filter(filter_key.to_owned(), filter_text);
                        if let Ok(json) = updated.to_json() {
                            out.prop_updates.push((
                                id.to_owned(),
                                DATAGRID_ADVANCED_PROP.to_owned(),
                                json,
                            ));
                        }
                        out.prop_updates.push((
                            id.to_owned(),
                            "ColumnFilters".to_owned(),
                            datagrid_filter_property(&updated),
                        ));
                    }
                }
                if enabled
                    && ncols > 1
                    && prop_bool(ctrl, "AllowColumnReorder", true)
                    && col.width >= 58.0
                {
                    let left_rect = Rect::from_min_size(
                        pos2(x + col.width - 34.0, header_rect.min.y + 3.0),
                        vec2(14.0, (row_h - 6.0).max(10.0)),
                    );
                    let right_rect = Rect::from_min_size(
                        pos2(x + col.width - 18.0, header_rect.min.y + 3.0),
                        vec2(14.0, (row_h - 6.0).max(10.0)),
                    );
                    if col.index > 0 {
                        let resp = ui.interact(
                            left_rect,
                            ctrl_id.with(("dg-col-left", col.index)),
                            Sense::click(),
                        );
                        painter.with_clip_rect(left_rect).text(
                            left_rect.center(),
                            Align2::CENTER_CENTER,
                            "â¹",
                            FontId::proportional(12.0),
                            header_fg,
                        );
                        if resp.clicked() {
                            let mut reordered = DataGridAdvanced::from_control(ctrl);
                            if reordered.move_column_left(col.index) {
                                if let Ok(json) = reordered.to_json() {
                                    out.prop_updates.push((
                                        id.to_owned(),
                                        DATAGRID_ADVANCED_PROP.to_owned(),
                                        json,
                                    ));
                                }
                            }
                        }
                    }
                    if col.index + 1 < ncols {
                        let resp = ui.interact(
                            right_rect,
                            ctrl_id.with(("dg-col-right", col.index)),
                            Sense::click(),
                        );
                        painter.with_clip_rect(right_rect).text(
                            right_rect.center(),
                            Align2::CENTER_CENTER,
                            "âº",
                            FontId::proportional(12.0),
                            header_fg,
                        );
                        if resp.clicked() {
                            let mut reordered = DataGridAdvanced::from_control(ctrl);
                            if reordered.move_column_right(col.index) {
                                if let Ok(json) = reordered.to_json() {
                                    out.prop_updates.push((
                                        id.to_owned(),
                                        DATAGRID_ADVANCED_PROP.to_owned(),
                                        json,
                                    ));
                                }
                            }
                        }
                    }
                }
            }
            if enabled
                && prop_bool(ctrl, "ShowCSVExportButton", false)
                && header_rect.width() >= 64.0
            {
                let button_rect = Rect::from_min_size(
                    pos2(header_rect.max.x - 54.0, header_rect.min.y + 3.0),
                    vec2(48.0, (row_h - 6.0).max(14.0)),
                );
                if ui
                    .put(button_rect, egui::Button::new("CSV").small())
                    .on_hover_text("Export CSV")
                    .clicked()
                {
                    out.prop_updates.push((
                        id.to_owned(),
                        "_ExportCSVRequested".to_owned(),
                        "1".to_owned(),
                    ));
                    out.events.push(UiEvent::ev(id, "onExportCSV"));
                }
            }
            let scroll_body_rect = Rect::from_min_max(
                pos2(
                    body_rect.min.x,
                    (body_rect.min.y + frozen_rows_height).min(body_rect.max.y),
                ),
                body_rect.max,
            );
            let body_painter_base = painter.with_clip_rect(body_rect);
            let scroll_body_painter = painter.with_clip_rect(scroll_body_rect);
            let mut rows_to_draw = Vec::new();
            for display_row in 0..frozen_rows {
                let y = body_rect.min.y + row_h * display_row as f32;
                rows_to_draw.push((display_row, y, false));
            }
            for layout_row in layout.first_row..layout.last_row_exclusive {
                let display_row = frozen_rows + layout_row;
                if display_row >= displayed_row_indices.len() {
                    continue;
                }
                let y = scroll_body_rect.min.y + row_h * layout_row as f32 - scroll_y;
                if y + row_h < scroll_body_rect.min.y || y > scroll_body_rect.max.y {
                    continue;
                }
                rows_to_draw.push((display_row, y, true));
            }
            // Confine the grid's own opaque fills to its rounded shape at the two
            // BOTTOM corners (the header owns the rounded top corners). A fill that
            // reaches the grid's bottom-left / bottom-right corner is CLAMPED to the
            // grid rect and rounded to the grid radius, so nothing square pokes past
            // the rounded background â this is what makes a DataGrid render rounded
            // even when nested inside another container (where the backdrop
            // notch-mask can't be used).
            //
            // Clamping is essential: the last row's rect usually extends *past*
            // `screen.max.y` and is cut square by the body clip. CornerRadius that
            // off-clip rect is invisible â so we intersect with the grid rect first,
            // then round the now-on-edge bottom corners.
            let grid_cr = paint::corner_radius(ctrl);
            // Fill `r` with `color`, staying behind the grid's rounded bottom
            // corners. All the geometry (and the reasoning behind it) lives in
            // `datagrid_confined_fill_rects`, which is unit-tested for the two
            // invariants that matter: gapless coverage and no bleed past the arc.
            let fill_confined = move |painter: &egui::Painter, r: Rect, color: Color32| {
                for sub in datagrid_confined_fill_rects(screen, grid_cr, r.intersect(screen)) {
                    painter.rect_filled(sub, 0.0, color);
                }
            };
            for (display_row, y, scroll_clipped) in rows_to_draw {
                let Some(&row_index) = displayed_row_indices.get(display_row) else {
                    continue;
                };
                let Some(row) = rows.get(row_index) else {
                    continue;
                };
                let body_painter = if scroll_clipped {
                    scroll_body_painter.clone()
                } else {
                    body_painter_base.clone()
                };
                let rrect = Rect::from_min_size(pos2(screen.min.x, y), vec2(screen.width(), row_h));
                if alt_rows && display_row % 2 == 1 {
                    fill_confined(&body_painter, rrect, alt_bg);
                }
                draw_datagrid_pattern(
                    &body_painter,
                    rrect,
                    0.0,
                    &sv(ctrl, "RowBackgroundPattern"),
                    Color32::from_rgba_unmultiplied(255, 255, 255, 18),
                );
                for col in layout
                    .frozen_columns
                    .iter()
                    .chain(layout.scrollable_columns.iter())
                {
                    // A scrollable column is clipped to the region right of the
                    // frozen band, so horizontal scrolling slides it *behind* the
                    // frozen columns instead of painting over them.
                    let body_painter = if col.frozen {
                        body_painter.clone()
                    } else {
                        body_painter.with_clip_rect(Rect::from_min_max(
                            pos2(screen.min.x + layout.frozen_columns_width, body_rect.min.y),
                            pos2(screen.max.x, body_rect.max.y),
                        ))
                    };
                    let (source_index, _, ty) = &display_cols[col.index];
                    let raw = if *source_index < row.len() {
                        row.get(*source_index).map(|s| s.as_str()).unwrap_or("")
                    } else {
                        ""
                    };
                    let x0 = screen.min.x + col.x;
                    // Full column-width band for this row: the background layer
                    // (appearance/column colour) fills this so the inter-column
                    // gutter under each vertical separator obeys the appearance
                    // background instead of revealing the grid backdrop image.
                    let col_rect =
                        Rect::from_min_size(pos2(x0, rrect.min.y), vec2(col.width, row_h));
                    // Content/image/frame stay inset so cells keep a small gutter.
                    let cell_rect = col_rect.shrink2(vec2(2.0, 0.0));
                    // Alternating-column highlight: fill every other column's full
                    // width for this row segment, beneath any per-cell/column colour.
                    if alt_cols && col.index % 2 == 1 {
                        fill_confined(&body_painter, col_rect, alt_bg);
                    }
                    let mut cell_selected = false;
                    if prop_bool(ctrl, "SelectableText", true) {
                        let cell_resp = ui.interact(
                            cell_rect,
                            ctrl_id.with(("dg-cell", row_index, col.index)),
                            Sense::click(),
                        );
                        if cell_resp.clicked() {
                            ui.ctx().memory_mut(|m| {
                                m.data.insert_temp(
                                    selection_id,
                                    DataGridCellSelection {
                                        row_index,
                                        display_column_index: col.index,
                                    },
                                );
                            });
                            // spec 021 T12: cell-level click with coordinates.
                            out.events.push(UiEvent::with_value(
                                id,
                                "onCellClick",
                                &format!("{row_index},{}", col.index),
                            ));
                        }
                        if cell_resp.double_clicked() {
                            out.events.push(UiEvent::with_value(
                                id,
                                "onCellDoubleClick",
                                &format!("{row_index},{}", col.index),
                            ));
                            out.events.push(UiEvent::with_value(
                                id,
                                "onRowDoubleClick",
                                &row_index.to_string(),
                            ));
                        }
                        cell_selected = ui.ctx().memory(|m| {
                            m.data
                                .get_temp::<DataGridCellSelection>(selection_id)
                                .map(|selection| {
                                    selection.row_index == row_index
                                        && selection.display_column_index == col.index
                                })
                                .unwrap_or(false)
                        });
                    }
                    let column_meta = advanced_grid.columns.get(col.index);
                    let value_rule =
                        column_meta.and_then(|column| column.value_style_rule_for(raw));
                    // Cell background fallback chain: value-rule colour â column
                    // colour â the grid's own appearance BackgroundColor (its flat
                    // underlay). The last step matters for cells whose visible
                    // content doesn't cover the whole cell â a framed "pill" column
                    // (the inner-shape is inset), or plain text. Without it those
                    // gaps fall through to the frosted glass sheen and read grey
                    // instead of the solid appearance colour the user configured.
                    // When the grid is on the default (translucent) background,
                    // `grid_bg_underlay` is `None` and the gap stays glass.
                    // A fully-transparent colour (the column default `#00000000`)
                    // is "unset", not "paint nothing" â filter it out at each step
                    // so the chain falls through to the grid's appearance
                    // background instead of short-circuiting on a 0-alpha colour.
                    let cell_bg = value_rule
                        .and_then(|rule| paint::parse_hex(&rule.background_color))
                        .filter(|c| c.a() > 0)
                        .or_else(|| {
                            column_meta
                                .and_then(|column| paint::parse_hex(&column.background_color))
                                .filter(|c| c.a() > 0)
                        })
                        .or(grid_bg_underlay);
                    if let Some(bg) = cell_bg {
                        if bg.a() > 0 {
                            // Full column width (not the inset cell) so the gutter
                            // beneath the vertical separators is the appearance
                            // background, not the grid backdrop showing through.
                            // `fill_confined` follows the grid's bottom arc with
                            // bands, so the last row's fill tracks the corner radius
                            // instead of squaring (or under-rounding) past it.
                            fill_confined(&body_painter, col_rect, bg);
                        }
                    }
                    if let Some(column) = column_meta {
                        draw_datagrid_pattern(
                            &body_painter,
                            cell_rect,
                            0.0,
                            &column.background_pattern,
                            Color32::from_rgba_unmultiplied(255, 255, 255, 26),
                        );
                        if !column.background_image.trim().is_empty() {
                            let image_id = egui::Id::new((
                                "dg_col_bg_img",
                                &ctrl.id,
                                &column.id,
                                column.background_image.as_str(),
                            ));
                            let tex = match ui
                                .data(|d| d.get_temp::<Option<egui::TextureHandle>>(image_id))
                            {
                                Some(t) => t,
                                None => {
                                    let loaded = paint::load_image_texture(
                                        ui.ctx(),
                                        column.background_image.trim(),
                                    );
                                    ui.data_mut(|d| d.insert_temp(image_id, loaded.clone()));
                                    loaded
                                }
                            };
                            if let Some(tex) = tex {
                                let dest =
                                    image_dest(cell_rect, tex.size_vec2(), BgImageMode::Fill);
                                // Honour the opacity the user defined for the
                                // column background: the alpha of the "Cell
                                // background" colour drives how opaque the column
                                // background image is, scaled by the control's own
                                // Opacity. A fully-transparent cell colour (the
                                // default) means "no explicit opacity", so the
                                // image shows at the control opacity alone â a
                                // column that only sets an image still renders.
                                let col_alpha = paint::parse_hex(&column.background_color)
                                    .map(|c| c.a())
                                    .filter(|a| *a > 0)
                                    .map(|a| a as f32 / 255.0)
                                    .unwrap_or(1.0);
                                let img_a = (alpha * col_alpha * 255.0).clamp(0.0, 255.0) as u8;
                                body_painter.image(
                                    tex.id(),
                                    dest,
                                    Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)),
                                    Color32::from_rgba_unmultiplied(255, 255, 255, img_a),
                                );
                            }
                        }
                    }
                    if let Some((gauge, fraction)) = column_meta
                        .and_then(|column| column.gauge.as_ref())
                        .and_then(|gauge| {
                            gauge
                                .fraction_for_value(raw)
                                .map(|fraction| (gauge, fraction))
                        })
                    {
                        let gauge_rect = cell_rect.shrink2(vec2(3.0, 5.0));
                        body_painter.rect_filled(
                            gauge_rect,
                            3.0,
                            paint::parse_hex(&gauge.background_color)
                                .unwrap_or(Color32::from_rgba_premultiplied(0, 0, 0, 70)),
                        );
                        let fill_width = gauge_rect.width() * fraction;
                        if fill_width > 0.5 {
                            let fill = Rect::from_min_size(
                                gauge_rect.min,
                                vec2(fill_width, gauge_rect.height()),
                            );
                            body_painter.rect_filled(
                                fill,
                                3.0,
                                paint::parse_hex(&gauge.fill_color)
                                    .unwrap_or(Color32::from_rgb(63, 134, 245)),
                            );
                        }
                    }
                    let edit_control = column_meta
                        .map(|column| {
                            if column.control_kind.trim().is_empty() {
                                column.edit_control.as_str()
                            } else {
                                column.control_kind.as_str()
                            }
                        })
                        .unwrap_or("");
                    if edit_control.eq_ignore_ascii_case("image") {
                        // Render the cell value as an image path (alphanumeric
                        // fields whose value is a file path, e.g. a thumbnail).
                        let path = raw.trim();
                        if !path.is_empty() {
                            let image_id = egui::Id::new(("dg_cell_img", &ctrl.id, path));
                            let tex = match ui
                                .data(|d| d.get_temp::<Option<egui::TextureHandle>>(image_id))
                            {
                                Some(t) => t,
                                None => {
                                    let loaded = paint::load_image_texture(ui.ctx(), path);
                                    ui.data_mut(|d| d.insert_temp(image_id, loaded.clone()));
                                    loaded
                                }
                            };
                            let img_rect = cell_rect.shrink2(vec2(3.0, 3.0));
                            if let Some(tex) = tex {
                                let dest = image_dest(img_rect, tex.size_vec2(), BgImageMode::Fit);
                                let corner = column_meta
                                    .map(|column| column.image_corner_radius)
                                    .unwrap_or(0.0)
                                    .clamp(0.0, dest.width().min(dest.height()) * 0.5);
                                let shadow = column_meta
                                    .map(|column| column.image_shadow)
                                    .unwrap_or(false);
                                let img_painter = body_painter.with_clip_rect(cell_rect);
                                let uv = Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0));
                                if shadow {
                                    // Soft two-layer drop shadow beneath the image.
                                    img_painter.rect_filled(
                                        dest.translate(vec2(0.0, 2.0)).expand(1.0),
                                        egui::CornerRadius::same(crate::paint::cr8(corner + 1.0)),
                                        Color32::from_black_alpha(55),
                                    );
                                    img_painter.rect_filled(
                                        dest.translate(vec2(0.0, 4.0)).expand(2.5),
                                        egui::CornerRadius::same(crate::paint::cr8(corner + 2.0)),
                                        Color32::from_black_alpha(28),
                                    );
                                }
                                if corner > 0.0 {
                                    img_painter.add(egui::Shape::Rect(
                                        egui::epaint::RectShape::new(
                                            dest,
                                            egui::CornerRadius::same(crate::paint::cr8(corner)),
                                            Color32::WHITE,
                                            Stroke::NONE,
                                            egui::StrokeKind::Middle,
                                        )
                                        .with_texture(tex.id(), uv),
                                    ));
                                } else {
                                    img_painter.image(tex.id(), dest, uv, Color32::WHITE);
                                }
                            } else {
                                // Missing/undecodable image â show the path so the
                                // cell isn't silently blank.
                                body_painter.with_clip_rect(cell_rect).text(
                                    pos2(cell_rect.min.x + 4.0, rrect.center().y),
                                    Align2::LEFT_CENTER,
                                    path,
                                    FontId::proportional(font_size * 0.8),
                                    Color32::from_rgb(200, 160, 160),
                                );
                            }
                        }
                        if cell_selected {
                            body_painter.rect_filled(
                                cell_rect,
                                0.0,
                                Color32::from_rgba_unmultiplied(80, 145, 255, 55),
                            );
                        }
                        continue;
                    }
                    if edit_control.eq_ignore_ascii_case("button") {
                        let button_rect = cell_rect.shrink2(vec2(5.0, 4.0));
                        body_painter.rect_filled(
                            button_rect,
                            4.0,
                            Color32::from_rgba_unmultiplied(40, 92, 170, 210),
                        );
                        body_painter.rect_stroke(
                            button_rect,
                            4.0,
                            Stroke::new(1.0, Color32::from_rgba_unmultiplied(130, 175, 255, 210)),
                            egui::StrokeKind::Middle,
                        );
                        body_painter.with_clip_rect(button_rect.shrink(3.0)).text(
                            button_rect.center(),
                            Align2::CENTER_CENTER,
                            raw,
                            FontId::proportional(
                                column_meta
                                    .map(|column| column.font_size)
                                    .filter(|size| *size > 0)
                                    .map(|size| size as f32)
                                    .unwrap_or(font_size),
                            ),
                            Color32::WHITE,
                        );
                        if cell_selected {
                            body_painter.rect_filled(
                                cell_rect,
                                0.0,
                                Color32::from_rgba_unmultiplied(80, 145, 255, 55),
                            );
                        }
                        continue;
                    }
                    if edit_control.eq_ignore_ascii_case("checkbox") {
                        let box_size = (row_h - 8.0).clamp(12.0, 22.0);
                        let check_rect =
                            Rect::from_center_size(cell_rect.center(), vec2(box_size, box_size));
                        body_painter.rect_filled(
                            check_rect,
                            3.0,
                            Color32::from_rgba_unmultiplied(0, 0, 0, 100),
                        );
                        body_painter.rect_stroke(
                            check_rect,
                            3.0,
                            Stroke::new(1.0, Color32::from_rgba_unmultiplied(220, 230, 255, 180)),
                            egui::StrokeKind::Middle,
                        );
                        let truthy = matches!(
                            raw.trim().to_ascii_lowercase().as_str(),
                            "y" | "yes" | "true" | "1" | "x" | "checked"
                        );
                        if truthy {
                            body_painter.line_segment(
                                [
                                    pos2(check_rect.min.x + 3.0, check_rect.center().y),
                                    pos2(check_rect.center().x - 1.0, check_rect.max.y - 4.0),
                                ],
                                Stroke::new(2.0, Color32::from_rgb(120, 210, 150)),
                            );
                            body_painter.line_segment(
                                [
                                    pos2(check_rect.center().x - 1.0, check_rect.max.y - 4.0),
                                    pos2(check_rect.max.x - 3.0, check_rect.min.y + 4.0),
                                ],
                                Stroke::new(2.0, Color32::from_rgb(120, 210, 150)),
                            );
                        }
                        if cell_selected {
                            body_painter.rect_filled(
                                cell_rect,
                                0.0,
                                Color32::from_rgba_unmultiplied(80, 145, 255, 55),
                            );
                        }
                        continue;
                    }
                    if edit_control.eq_ignore_ascii_case("dropdown") {
                        let dropdown_rect = cell_rect.shrink2(vec2(4.0, 4.0));
                        body_painter.rect_filled(
                            dropdown_rect,
                            4.0,
                            Color32::from_rgba_unmultiplied(0, 0, 0, 95),
                        );
                        body_painter.rect_stroke(
                            dropdown_rect,
                            4.0,
                            Stroke::new(1.0, Color32::from_rgba_unmultiplied(220, 230, 255, 120)),
                            egui::StrokeKind::Middle,
                        );
                        let text_clip = Rect::from_min_max(
                            dropdown_rect.min + vec2(6.0, 0.0),
                            pos2(dropdown_rect.max.x - 18.0, dropdown_rect.max.y),
                        );
                        body_painter.with_clip_rect(text_clip).text(
                            pos2(text_clip.min.x, dropdown_rect.center().y),
                            Align2::LEFT_CENTER,
                            raw,
                            FontId::proportional(
                                column_meta
                                    .map(|column| column.font_size)
                                    .filter(|size| *size > 0)
                                    .map(|size| size as f32)
                                    .unwrap_or(font_size),
                            ),
                            cell_fg,
                        );
                        body_painter.text(
                            pos2(dropdown_rect.max.x - 9.0, dropdown_rect.center().y),
                            Align2::CENTER_CENTER,
                            "â¼",
                            FontId::proportional(10.0),
                            cell_fg,
                        );
                        if cell_selected {
                            body_painter.rect_filled(
                                cell_rect,
                                0.0,
                                Color32::from_rgba_unmultiplied(80, 145, 255, 55),
                            );
                        }
                        continue;
                    }
                    if matches!(ty.as_str(), "image" | "img" | "picture") {
                        let path = raw.trim();
                        let cell = cell_rect.shrink(2.0);
                        if !path.is_empty() {
                            let cid = egui::Id::new(("dg_img", path));
                            let tex =
                                match ui.data(|d| d.get_temp::<Option<egui::TextureHandle>>(cid)) {
                                    Some(t) => t,
                                    None => {
                                        let loaded = paint::load_image_texture(ui.ctx(), path);
                                        ui.data_mut(|d| d.insert_temp(cid, loaded.clone()));
                                        loaded
                                    }
                                };
                            if let Some(t) = tex {
                                let sz = t.size_vec2();
                                let scale = (cell.width() / sz.x)
                                    .min(cell.height() / sz.y)
                                    .min(1.0)
                                    .max(0.01);
                                let irect = Rect::from_center_size(cell.center(), sz * scale);
                                body_painter.image(
                                    t.id(),
                                    irect,
                                    Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)),
                                    Color32::WHITE,
                                );
                            } else {
                                body_painter.rect_stroke(
                                    cell,
                                    2.0,
                                    Stroke::new(1.0, Color32::from_rgb(110, 120, 160)),
                                    egui::StrokeKind::Middle,
                                );
                            }
                        }
                        if cell_selected {
                            body_painter.rect_filled(
                                cell_rect,
                                0.0,
                                Color32::from_rgba_unmultiplied(80, 145, 255, 55),
                            );
                        }
                        continue;
                    }
                    let cobol_mask = column_meta
                        .map(|column| column.cobol_mask.as_str())
                        .unwrap_or("");
                    let (text, _numeric_right_hint) =
                        paint::format_cell_with_cobol_mask(raw, ty, cobol_mask);
                    let cell_painter = body_painter.with_clip_rect(cell_rect);
                    let column_font_size = column_meta
                        .map(|column| column.font_size)
                        .filter(|size| *size > 0)
                        .map(|size| size as f32)
                        .unwrap_or(font_size);
                    let mut text_color = value_rule
                        .and_then(|rule| paint::parse_hex(&rule.foreground_color))
                        .or_else(|| {
                            column_meta
                                .and_then(|column| paint::parse_hex(&column.foreground_color))
                        })
                        .unwrap_or(cell_fg);
                    let mut text_rect = cell_rect;
                    if let Some(frame) = column_meta.and_then(|column| column.frame.as_ref()) {
                        if frame.enabled {
                            let frame_bg = value_rule
                                .and_then(|rule| paint::parse_hex(&rule.frame_background_color))
                                .or_else(|| paint::parse_hex(&frame.background_color))
                                .unwrap_or(Color32::from_rgb(27, 196, 125));
                            text_color = value_rule
                                .and_then(|rule| paint::parse_hex(&rule.frame_foreground_color))
                                .or_else(|| paint::parse_hex(&frame.foreground_color))
                                .unwrap_or(text_color);
                            text_rect = cell_rect.shrink2(vec2(frame.padding as f32, 4.0));
                            cell_painter.rect_filled(
                                text_rect,
                                frame.corner_radius as f32,
                                frame_bg,
                            );
                        }
                    }
                    let alignment = column_meta
                        .map(|column| column.text_alignment)
                        .unwrap_or_default();
                    match alignment {
                        crate::model::DataGridTextAlignment::Right => {
                            cell_painter.text(
                                pos2(text_rect.max.x - 6.0, rrect.center().y),
                                Align2::RIGHT_CENTER,
                                &text,
                                FontId::proportional(column_font_size),
                                text_color,
                            );
                        }
                        crate::model::DataGridTextAlignment::Center => {
                            cell_painter.text(
                                text_rect.center(),
                                Align2::CENTER_CENTER,
                                &text,
                                FontId::proportional(column_font_size),
                                text_color,
                            );
                        }
                        crate::model::DataGridTextAlignment::Left => {
                            cell_painter.text(
                                pos2(text_rect.min.x + 6.0, rrect.center().y),
                                Align2::LEFT_CENTER,
                                &text,
                                FontId::proportional(column_font_size),
                                text_color,
                            );
                        }
                    }
                    if cell_selected {
                        body_painter.rect_filled(
                            cell_rect,
                            0.0,
                            Color32::from_rgba_unmultiplied(80, 145, 255, 55),
                        );
                    }
                }
            }
            // Filler area to the right of the last column (when the columns are
            // narrower than the grid). It carries no cell, so it otherwise shows
            // the frosted glass sheen and reads grey. When an appearance
            // BackgroundColor is set (flat underlay = `grid_bg_underlay`), paint
            // that filler solid so the non-bound region obeys the datagrid's
            // appearance background instead of the glass. Rounded only on the
            // bottom-right, matching the grid's own corner. Drawn after the rows
            // (covers glass + alternating tint) and before the separators.
            if let Some(fill) = grid_bg_underlay {
                let filler_x0 = screen.min.x + layout.total_columns_width;
                if filler_x0 < screen.max.x - 0.5 {
                    let filler_rect = Rect::from_min_max(
                        pos2(filler_x0, body_rect.min.y),
                        pos2(screen.max.x, screen.max.y),
                    );
                    let r = paint::corner_radius(ctrl);
                    painter.rect_filled(
                        filler_rect,
                        egui::CornerRadius {
                            nw: 0,
                            ne: 0,
                            sw: 0,
                            se: crate::paint::cr8(r),
                        },
                        fill,
                    );
                }
            }
            for col in layout
                .frozen_columns
                .iter()
                .chain(layout.scrollable_columns.iter())
            {
                let x = screen.min.x + col.x + col.width;
                // A scrollable column's separator must not intrude into the frozen
                // band as it scrolls left behind the frozen columns.
                let min_x = if col.frozen {
                    screen.min.x
                } else {
                    screen.min.x + layout.frozen_columns_width
                };
                if x > min_x && x < screen.max.x {
                    draw_datagrid_line(
                        &painter,
                        clip_datagrid_line_to_corners(
                            screen,
                            grid_cr,
                            [pos2(x, screen.min.y), pos2(x, screen.max.y)],
                        ),
                        Stroke::new(1.0, grid_c),
                        grid_line_style,
                    );
                }
            }
            draw_datagrid_line(
                &painter,
                clip_datagrid_line_to_corners(
                    screen,
                    grid_cr,
                    [
                        pos2(screen.min.x, screen.min.y + header_h),
                        pos2(screen.max.x, screen.min.y + header_h),
                    ],
                ),
                Stroke::new(1.0, grid_c),
                grid_line_style,
            );

            // Outer border of the whole DataGrid (left and bottom especially, since
            // right-of-last and header-bottom are drawn above). Use the DataGrid's
            // own GridLineStyle and line colour (from appearance Foreground or
            // GridLineColor settings) so the outer obeys the datagrid line settings.
            {
                let o_stroke = Stroke::new(1.0, grid_c);
                let o_style = grid_line_style;
                if grid_cr >= 0.5 {
                    // Rounded grid: trace the whole outline as a rounded-rect stroke
                    // so the bottom corners follow the radius (the header already
                    // rounds the top corners to the same radius). egui can't dash a
                    // rounded corner, so a rounded grid uses a solid outline. Inset
                    // by half the stroke width so the line sits INSIDE the grid rect
                    // â a centred stroke spills half a pixel past the edge, which
                    // shows as a light rim bleeding outside the rounded corner.
                    painter.rect_stroke(
                        screen,
                        egui::CornerRadius::same(crate::paint::cr8(grid_cr)),
                        o_stroke,
                        egui::StrokeKind::Inside,
                    );
                } else {
                    // Square grid: left + bottom outer lines (obey GridLineStyle).
                    draw_datagrid_line(
                        &painter,
                        [
                            pos2(screen.min.x, screen.min.y),
                            pos2(screen.min.x, screen.max.y),
                        ],
                        o_stroke,
                        o_style,
                    );
                    draw_datagrid_line(
                        &painter,
                        [
                            pos2(screen.min.x, screen.max.y),
                            pos2(screen.max.x, screen.max.y),
                        ],
                        o_stroke,
                        o_style,
                    );
                }
            }
            // Frozen-pane drop shadow: the frozen columns / header+rows cast a soft
            // shadow onto the content that scrolls behind them (a spreadsheet cue).
            if prop_bool(ctrl, "FrozenShadow", true) {
                if layout.frozen_columns_width > 0.0 && layout.max_scroll_x > 0.0 {
                    let x0 = screen.min.x + layout.frozen_columns_width;
                    let shadow = Rect::from_min_max(
                        pos2(x0, screen.min.y),
                        pos2((x0 + 11.0).min(screen.max.x), screen.max.y),
                    );
                    painter.add(frozen_shadow_shape(shadow, FrozenShadowEdge::Left, 55));
                }
                if layout.max_scroll_y > 0.0 {
                    let y0 = body_rect.min.y + frozen_rows as f32 * row_h;
                    let shadow = Rect::from_min_max(
                        pos2(screen.min.x, y0),
                        pos2(screen.max.x, (y0 + 9.0).min(screen.max.y)),
                    );
                    painter.add(frozen_shadow_shape(shadow, FrozenShadowEdge::Top, 55));
                }
            }
            if layout.max_scroll_y > 0.0 && body_rect.height() > 8.0 {
                // The track hugs the right edge, so its bottom sits inside the grid's
                // rounded corner band. Pull the bottom up by the arc's vertical inset
                // at the track's x so the scrollbar never pokes past the rounded
                // bottom-right corner (a DataGrid-line-style bleed).
                let track_v_inset = rounded_edge_inset_v(screen, grid_cr, screen.max.x - 3.5);
                let track_bottom = (body_rect.max.y - 2.0).min(screen.max.y - track_v_inset);
                let track = Rect::from_min_max(
                    pos2(screen.max.x - 5.0, body_rect.min.y + 2.0),
                    pos2(screen.max.x - 2.0, track_bottom),
                );
                let thumb_h = (body_rect.height() / layout.total_rows_height * track.height())
                    .clamp(12.0, track.height());
                let thumb_y =
                    track.min.y + (track.height() - thumb_h) * (scroll_y / layout.max_scroll_y);
                let thumb =
                    Rect::from_min_size(pos2(track.min.x, thumb_y), vec2(track.width(), thumb_h));
                painter.rect_filled(track, 2.0, Color32::from_rgba_premultiplied(0, 0, 0, 55));
                painter.rect_filled(
                    thumb,
                    2.0,
                    Color32::from_rgba_premultiplied(230, 235, 255, 150),
                );
            }
            // DataGrid component-frame diagnostic (private to the grid): outline
            // every internal sub-component last, on a foreground layer, so the
            // real fills stay untouched underneath.
            if paint::datagrid_diagnostics_enabled() {
                draw_datagrid_component_frames(&painter, screen.min, &layout, row_h);
            }
        }
        CT::TabControl => {
            paint::draw_control(&painter, screen.min, ctrl, false, glass, alpha, 1.0, None);
            let selected = sv(ctrl, "SelectedTab").parse::<usize>().unwrap_or(0);
            for (i, tr) in paint::tabcontrol_tab_rects(screen.min, ctrl)
                .into_iter()
                .enumerate()
            {
                if ui
                    .interact(tr, ctrl_id.with(("tab", i)), Sense::click())
                    .clicked()
                    && enabled
                {
                    out.prop_updates
                        .push((id.to_owned(), "SelectedTab".to_owned(), i.to_string()));
                    out.events.push(UiEvent::ev(id, "onChange"));
                    // spec 021 T12: every tab click, plus the change event only
                    // when the selection actually moved.
                    out.events
                        .push(UiEvent::with_value(id, "onTabClick", &i.to_string()));
                    if i != selected {
                        out.events
                            .push(UiEvent::with_value(id, "onTabChanged", &i.to_string()));
                    }
                }
            }
        }
        CT::TreeView => {
            // Same rule as the ListBox above: the designed face, not a
            // hardcoded one — and then the tree itself, through the SHARED
            // renderer the designer canvas calls. The two used to disagree
            // completely: a flat bulleted list at a fixed 12pt here, and the
            // caption "[TreeView]" and nothing else on the canvas (operator,
            // 2026-08-22: "treeview not working / content not rendered").
            paint::draw_control(&painter, screen.min, ctrl, false, glass, alpha, 1.0, None);
            let rows = crate::treeview::layout(ctrl, screen);
            let selected = sv(ctrl, "SelectedNode");
            let checked: Vec<String> = sv(ctrl, "CheckedNodes")
                .lines()
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(str::to_owned)
                .collect();

            // Interaction first, so the paint below shows what THIS frame's
            // pointer is doing rather than what the last one did.
            let hot = ctrl
                .get_prop("HotTracking")
                .map(|v| v.as_bool())
                .unwrap_or(false);
            let mut hovered = None;
            let mut checked_after = checked.clone();
            let mut collapsed_after: Vec<String> = sv(ctrl, "CollapsedNodes")
                .lines()
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(str::to_owned)
                .collect();
            for row in &rows {
                let resp = ui.interact(
                    row.rect,
                    ctrl_id.with(("tv-node", row.index)),
                    Sense::click(),
                );
                if hot && resp.hovered() {
                    hovered = Some(row.index);
                }
                let is_selected = !selected.is_empty() && selected == row.text;
                if resp.clicked() && enabled {
                    // Three things a row can be clicked ON, and they do not
                    // overlap: the disclosure ARROW folds the node, the tick
                    // BOX checks it, anywhere else selects it.
                    let at = ui.ctx().pointer_interact_pos();
                    let hit = |r: Option<Rect>| {
                        r.zip(at).map(|(b, p)| b.expand(2.0).contains(p)).unwrap_or(false)
                    };
                    let on_arrow = hit(row.expander);
                    let on_box = !on_arrow && hit(row.check);
                    if on_arrow {
                        match collapsed_after.iter().position(|c| *c == row.text) {
                            Some(i) => {
                                collapsed_after.remove(i);
                            }
                            None => collapsed_after.push(row.text.clone()),
                        }
                        out.prop_updates.push((
                            id.to_owned(),
                            "CollapsedNodes".to_owned(),
                            collapsed_after.join("\n"),
                        ));
                        // Which way it went, so a handler can load children on
                        // first open without tracking the state itself.
                        out.events.push(UiEvent::with_value(
                            id,
                            if row.collapsed {
                                "onNodeExpand"
                            } else {
                                "onNodeCollapse"
                            },
                            &row.text,
                        ));
                    } else if on_box {
                        match checked_after.iter().position(|c| *c == row.text) {
                            Some(i) => {
                                checked_after.remove(i);
                            }
                            None => checked_after.push(row.text.clone()),
                        }
                        out.prop_updates.push((
                            id.to_owned(),
                            "CheckedNodes".to_owned(),
                            checked_after.join("\n"),
                        ));
                        out.events
                            .push(UiEvent::with_value(id, "onNodeCheck", &row.text));
                    } else {
                        out.prop_updates.push((
                            id.to_owned(),
                            "SelectedNode".to_owned(),
                            row.text.clone(),
                        ));
                        out.events
                            .push(UiEvent::with_value(id, "onNodeClick", &row.text));
                        if !is_selected {
                            out.events
                                .push(UiEvent::with_value(id, "onNodeSelect", &row.text));
                        }
                    }
                }
                if resp.double_clicked() && enabled {
                    out.events
                        .push(UiEvent::with_value(id, "onNodeDblClick", &row.text));
                    out.events
                        .push(UiEvent::with_value(id, "onNodeDoubleClick", &row.text));
                }
            }
            crate::treeview::paint(
                &painter,
                ctrl,
                screen,
                &rows,
                crate::treeview::TreeState {
                    selected: &selected,
                    checked: &checked_after,
                    hovered,
                    alpha,
                },
            );
        }
        CT::Splitter => {
            let horiz = !sv(ctrl, "Orientation").starts_with('V');
            paint::draw_surface_auto(
                &painter,
                screen,
                Color32::from_rgb(60, 66, 96),
                paint::corner_radius(ctrl),
                false,
                alpha,
                paint::SurfaceRole::Card,
            );
            let c = screen.center();
            let dot = Color32::from_rgba_premultiplied(200, 210, 240, 160);
            for k in -1..=1 {
                let p = if horiz {
                    pos2(c.x + k as f32 * 5.0, c.y)
                } else {
                    pos2(c.x, c.y + k as f32 * 5.0)
                };
                painter.circle_filled(p, 1.5, dot);
            }
        }
        CT::MenuBar => {
            // A menu's `Cursor` belongs to the things you point at â the titles
            // on the bar and the items under them, not the bar's own backdrop.
            let menu_cursor = ctrl
                .get_prop("Cursor")
                .and_then(|v| cursor_icon_for(v.as_str()));
            let menu_bg = ctrl
                .get_prop("BackgroundColor")
                .map(|v| paint::parse_color(v.as_str()))
                .unwrap_or(Color32::TRANSPARENT);
            // Historically the bar surface was drawn only when the developer
            // set a colour â with none, the bar is bare and the form shows
            // through. A flat theme has no frost to fall back on, so a theme
            // that supplies its own faces always lays its bar down; Liquid
            // Glass supplies none and keeps the original opt-in behaviour
            // exactly (R10).
            if menu_bg.a() > 0 {
                paint::draw_surface_auto(
                    &painter,
                    screen,
                    menu_bg,
                    paint::corner_radius(ctrl),
                    false,
                    alpha,
                    paint::SurfaceRole::Shape,
                );
            } else if paint::theme_has_surface(painter.ctx(), paint::SurfaceRole::Card) {
                paint::draw_surface_auto(
                    &painter,
                    screen,
                    menu_bg,
                    paint::corner_radius(ctrl),
                    false,
                    alpha,
                    paint::SurfaceRole::Card,
                );
            }
            let fg = ctrl
                .get_prop("ForegroundColor")
                .map(|v| paint::parse_color(v.as_str()))
                .unwrap_or(Color32::from_rgb(225, 230, 250));
            let highlight_bg = ctrl
                .get_prop("HighlightBgColor")
                .map(|v| paint::parse_color(v.as_str()))
                .unwrap_or(Color32::from_rgb(68, 136, 255));
            let highlight_fg = ctrl
                .get_prop("HighlightFgColor")
                .map(|v| paint::parse_color(v.as_str()))
                .unwrap_or(Color32::WHITE);
            let selected_bg = ctrl
                .get_prop("SelectedBgColor")
                .map(|v| paint::parse_color(v.as_str()))
                .unwrap_or(Color32::from_rgb(51, 102, 204));

            let menu_id = egui::Id::new(("menu_open", id));
            let open_idx: Option<usize> = ui.data(|d| d.get_temp(menu_id)).unwrap_or(None);

            if let Some(def) = paint::get_menu_cache(ui.ctx(), id) {
                let font = FontId::proportional(12.0);
                let mut x = screen.min.x + 8.0;
                let pad = 8.0;

                for (ti, entry) in def.menu.iter().enumerate() {
                    if entry.item_type == crate::menu::MenuItemType::Separator {
                        continue;
                    }
                    let galley = painter.layout_no_wrap(entry.label.clone(), font.clone(), fg);
                    let w = galley.size().x;
                    let label_rect = egui::Rect::from_min_size(
                        pos2(x - pad * 0.5, screen.min.y),
                        Vec2::new(w + pad, screen.height()),
                    );

                    let is_open = open_idx == Some(ti);
                    if is_open {
                        painter.rect_filled(label_rect, 2.0, selected_bg);
                    }

                    let mut resp = ui.allocate_rect(label_rect, egui::Sense::click());
                    if let Some(icon) = menu_cursor {
                        resp = resp.on_hover_cursor(icon);
                    }
                    if resp.hovered() && !is_open {
                        painter.rect_filled(label_rect, 2.0, highlight_bg);
                        painter.galley(
                            pos2(x, screen.center().y - galley.size().y * 0.5),
                            galley,
                            highlight_fg,
                        );
                    } else {
                        painter.galley(
                            pos2(x, screen.center().y - galley.size().y * 0.5),
                            galley,
                            fg,
                        );
                    }

                    if resp.clicked() {
                        let new_idx = if is_open { None } else { Some(ti) };
                        // spec 021 T12: menu open/close lifecycle.
                        out.events.push(UiEvent::with_value(
                            id,
                            if new_idx.is_some() {
                                "onMenuOpen"
                            } else {
                                "onMenuClose"
                            },
                            &entry.label,
                        ));
                        ui.data_mut(|d| d.insert_temp(menu_id, new_idx));
                    }

                    // Pulldown dropdown
                    if is_open && !entry.items.is_empty() {
                        let dropdown_id = egui::Id::new(("menu_dropdown", id, ti));
                        let dropdown_pos = pos2(label_rect.min.x, label_rect.max.y + 2.0);
                        egui::Area::new(dropdown_id)
                            .order(egui::Order::Foreground)
                            .fixed_pos(dropdown_pos)
                            .show(ui.ctx(), |ui| {
                                egui::Frame::popup(&ui.ctx().global_style())
                                    .inner_margin(egui::Margin::same(4))
                                    .show(ui, |ui| {
                                        for item in &entry.items {
                                            if item.item_type
                                                == crate::menu::MenuItemType::Separator
                                            {
                                                ui.separator();
                                                continue;
                                            }
                                            let item_resp = ui.horizontal(|ui| {
                                                let dimmed = !item.enabled;
                                                let item_fg = if dimmed {
                                                    Color32::from_rgb(120, 120, 130)
                                                } else {
                                                    fg
                                                };
                                                // Icon
                                                if let Some(icon_name) = &item.icon {
                                                    let icon_rect =
                                                        ui.allocate_space(Vec2::splat(24.0)).1;
                                                    crate::icons::draw_menu_icon(
                                                        &painter, icon_rect, icon_name, item_fg,
                                                    );
                                                } else {
                                                    ui.allocate_space(Vec2::splat(24.0));
                                                }
                                                // Label
                                                ui.label(
                                                    egui::RichText::new(&item.label).color(item_fg),
                                                );
                                                // Spacer
                                                ui.add_space(40.0);
                                                // Accelerator
                                                if let Some(accel_str) = &item.accelerator {
                                                    if let Some(accel) =
                                                        crate::menu::parse_accelerator(accel_str)
                                                    {
                                                        let formatted =
                                                            crate::menu::format_accelerator(&accel);
                                                        ui.label(
                                                            egui::RichText::new(formatted)
                                                                .color(Color32::from_rgb(
                                                                    140, 140, 160,
                                                                ))
                                                                .small(),
                                                        );
                                                    }
                                                }
                                                // Sub-menu indicator
                                                if !item.items.is_empty() {
                                                    ui.label(
                                                        egui::RichText::new("â¸").color(item_fg),
                                                    );
                                                }
                                            });
                                            let mut row_resp = ui.interact(
                                                item_resp.response.rect,
                                                egui::Id::new(("mi", &item.id)),
                                                egui::Sense::click(),
                                            );
                                            if let Some(icon) = menu_cursor {
                                                row_resp = row_resp.on_hover_cursor(icon);
                                            }
                                            if item.enabled && row_resp.hovered() {
                                                ui.painter().rect_filled(
                                                    item_resp.response.rect,
                                                    2.0,
                                                    highlight_bg,
                                                );
                                            }
                                            if item.enabled && row_resp.clicked() {
                                                ui.data_mut(|d| {
                                                    d.insert_temp(menu_id, None::<usize>)
                                                });
                                                if let Some(action) = &item.action {
                                                    if action == "close-application" {
                                                        out.events.push(UiEvent {
                                                            ctrl_id: id.to_owned(),
                                                            event: "onCloseApplication".to_owned(),
                                                            value: None,
                                                        });
                                                    }
                                                }
                                                out.events.push(UiEvent {
                                                    ctrl_id: id.to_owned(),
                                                    event: "onMenuClick".to_owned(),
                                                    value: Some(item.id.clone()),
                                                });
                                                let path = def
                                                    .item_path(&item.id)
                                                    .unwrap_or_else(|| format!("/{}", item.label));
                                                out.events.push(UiEvent {
                                                    ctrl_id: id.to_owned(),
                                                    event: "onMenuItemClick".to_owned(),
                                                    value: Some(path),
                                                });
                                            }
                                        }
                                    });
                            });
                    }

                    x += w + pad + 6.0;
                }

                // Click outside closes menus
                if open_idx.is_some() && ui.input(|i| i.pointer.any_pressed()) {
                    let ptr = ui.input(|i| i.pointer.interact_pos()).unwrap_or_default();
                    if !screen.contains(ptr) {
                        ui.data_mut(|d| d.insert_temp(menu_id, None::<usize>));
                    }
                }

                // Accelerator key dispatch (T13)
                fn collect_accels<'a>(
                    items: &'a [crate::menu::MenuItem],
                    out: &mut Vec<(&'a crate::menu::MenuItem, crate::menu::Accelerator)>,
                ) {
                    for item in items {
                        if item.enabled {
                            if let Some(accel_str) = &item.accelerator {
                                if let Some(accel) = crate::menu::parse_accelerator(accel_str) {
                                    out.push((item, accel));
                                }
                            }
                            collect_accels(&item.items, out);
                        }
                    }
                }
                let mut accels = Vec::new();
                collect_accels(&def.menu, &mut accels);
                for (item, accel) in &accels {
                    let mut mods = egui::Modifiers::NONE;
                    mods.ctrl = accel.ctrl;
                    mods.shift = accel.shift;
                    mods.alt = accel.alt;
                    mods.command = accel.cmd;
                    if ui.input(|i| i.modifiers == mods && i.key_pressed(char_to_key(accel.key))) {
                        out.events.push(UiEvent {
                            ctrl_id: id.to_owned(),
                            event: "onMenuClick".to_owned(),
                            value: Some(item.id.clone()),
                        });
                        let path = def
                            .item_path(&item.id)
                            .unwrap_or_else(|| format!("/{}", item.label));
                        out.events.push(UiEvent {
                            ctrl_id: id.to_owned(),
                            event: "onMenuItemClick".to_owned(),
                            value: Some(path),
                        });
                    }
                }
            } else {
                painter.text(
                    screen.center(),
                    egui::Align2::CENTER_CENTER,
                    "â° MenuBar (empty)",
                    FontId::proportional(12.0),
                    fg,
                );
            }
        }
        CT::ToolBar => {
            // The bar's own frame, from its own properties. It used to be a
            // hard-wired card the developer could not touch; the defaults now
            // reproduce nothing at all â radius 10, no border, fully transparent
            // â so a toolbar reads as buttons on the form until asked otherwise.
            let radius = paint::corner_radius(ctrl);
            // The colour the developer actually chose, if any — the seeded
            // `#F0F0F0` and the Neumorphic stamps all mean "not chosen", the
            // renderer-wide convention.
            let user_bg = paint::user_background_color(ctrl);
            // `face_opacity_of`, not the raw Transparency: a toolbar ships
            // fully transparent, and reading that seeded 100 literally is what
            // made a chosen BackgroundColor do nothing at all (operator,
            // 2026-08-22). Choosing a colour turns the frame on; a Transparency
            // the developer moved still fades it.
            let opacity = paint::face_opacity_of(ctrl);
            if opacity > 0.0 {
                let face = user_bg.unwrap_or(Color32::from_rgb(40, 46, 76));
                // `_bg` with the chosen colour, so THAT colour is the surface
                // rather than a hint the active theme is free to ignore — the
                // Card role answers with the theme's own fill and never reaches
                // the caller's, which is why lowering Transparency still did not
                // show the colour that had been picked. Same fix, and the same
                // reason, as the CheckBox box colour.
                paint::draw_surface_auto_bg(
                    &painter,
                    screen,
                    face,
                    user_bg,
                    radius,
                    false,
                    alpha * opacity,
                    paint::SurfaceRole::Card,
                );
            }
            let border_style = sv(ctrl, "BorderStyle");
            let border_w = sv(ctrl, "BorderWidth").parse::<f32>().unwrap_or(1.0);
            if !border_style.eq_ignore_ascii_case("None") && border_w > 0.0 {
                let bc = paint::parse_color(&sv(ctrl, "BorderColor"));
                let bc = if bc.a() > 0 { bc } else { Color32::from_gray(136) };
                painter.rect_stroke(
                    screen,
                    radius,
                    egui::Stroke::new(
                        border_w,
                        Color32::from_rgba_premultiplied(
                            bc.r(),
                            bc.g(),
                            bc.b(),
                            (bc.a() as f32 * alpha) as u8,
                        ),
                    ),
                    egui::StrokeKind::Inside,
                );
            }
            // Groups of buttons, drawn by the ONE toolbar renderer, then made
            // pressable. A button's `enabled` is its own â separate from the
            // toolbar control's, which gates the lot.
            let def = crate::toolbar::ToolbarDef::from_control(ctrl);
            let pointer = ui.ctx().pointer_latest_pos();
            let held = ui.ctx().input(|i| i.pointer.primary_down());
            // Which button is under the pointer decides hover AND what a release
            // lands on, so both are resolved from the same geometry.
            let probe = crate::toolbar_paint::draw(
                &painter,
                screen,
                &def,
                alpha,
                crate::toolbar_paint::Interaction::inert(),
            );
            let under = pointer.and_then(|p| {
                probe
                    .iter()
                    .find(|(_, r)| r.contains(p))
                    .map(|(id, _)| id.clone())
            });
            let live = under.as_deref().filter(|id| {
                enabled && def.button(id).is_some_and(|b| b.enabled)
            });
            // Redraw with the pointer state now that it is known â cheap, and it
            // keeps hover/press feedback in the shared renderer rather than
            // duplicating the face logic here.
            if live.is_some() {
                crate::toolbar_paint::draw(
                    &painter,
                    screen,
                    &def,
                    alpha,
                    crate::toolbar_paint::Interaction {
                        hovered: live,
                        pressed: if held { live } else { None },
                    },
                );
            }
            let resp = ui.interact(screen, ctrl_id, Sense::click());
            focus_keyboard_events(ui, &resp, id, out, &bound);
            if resp.clicked() && enabled {
                if let Some(button_id) = live.map(str::to_owned) {
                    let action = def
                        .button(&button_id)
                        .map(|b| b.action())
                        .unwrap_or_default();
                    if action.is_platform_action() {
                        // The host prints, shares, captures, launches.
                        out.toolbar_actions.push((
                            id.to_owned(),
                            button_id.clone(),
                            action.to_action_string(),
                        ));
                    }
                    // The form always hears about the press, whatever else
                    // happens â a handler may want to log it, or refuse it.
                    //
                    // WHICH button it was arrives as `LastButton` on the toolbar,
                    // written before the event so a handler reading
                    // `TOOLBAR-1::LastButton` already sees this press. That is
                    // what lets ONE handler serve a whole toolbar; the event also
                    // carries the id as its value for hosts that use it.
                    out.prop_updates.push((
                        id.to_owned(),
                        "LastButton".to_owned(),
                        button_id.clone(),
                    ));
                    out.events
                        .push(UiEvent::with_value(id, "onClick", &button_id));
                    // â¦and the BUTTON's own `onClick`, under the id it answers
                    // to outside this toolbar. A button is not a `Control`, so
                    // nothing in `form.controls` names it â this derived id is
                    // what the generated event loop dispatches on, and the only
                    // reason `procedure:` and `open-modal:` can reach anything.
                    // It is fired for EVERY press: the loop simply has no `WHEN`
                    // for a button nothing is attached to, exactly as for a
                    // control with no handler.
                    if let Some((group, _)) = def.button_with_group(&button_id) {
                        out.events.push(UiEvent::click(&crate::toolbar::button_control_id(
                            id, &group.id, &button_id,
                        )));
                    }
                }
            }
        }
        CT::StatusBar => {
            paint::draw_surface_auto(
                &painter,
                screen,
                Color32::from_rgb(40, 46, 76),
                paint::corner_radius(ctrl),
                false,
                alpha,
                paint::SurfaceRole::Card,
            );
            let fg = Color32::from_rgb(225, 230, 250);
            let mut x = screen.min.x + 8.0;
            for item in sv(ctrl, "Items").lines().filter(|l| !l.trim().is_empty()) {
                let galley =
                    painter.layout_no_wrap(item.trim().to_owned(), FontId::proportional(12.0), fg);
                let w = galley.size().x;
                painter.galley(
                    pos2(x, screen.center().y - galley.size().y / 2.0),
                    galley,
                    fg,
                );
                x += w + 18.0;
            }
        }
        CT::PictureBox => {
            // Render through `draw_control` with a pre-loaded texture â the SAME
            // path the designer canvas uses â so the image is tinted/framed
            // identically and is never dimmed or washed-out relative to the canvas
            // (spec 017 parity). `draw_picturebox` used a different tint + frame.
            let source = sv(ctrl, "ImagePath").trim().to_owned();
            let tex = paint::picturebox_texture(ui.ctx(), &source).map(|t| t.id());
            // spec 021 T12: image lifecycle, once per distinct source value.
            let mem = ctrl_id.with("pic-src-state");
            let state = (source.clone(), tex.is_some());
            let prev = ui
                .ctx()
                .memory(|m| m.data.get_temp::<(String, bool)>(mem));
            if prev.as_ref() != Some(&state) {
                if !source.is_empty() {
                    let event = if tex.is_some() {
                        "onImageLoaded"
                    } else {
                        "onImageError"
                    };
                    out.events.push(UiEvent::with_value(id, event, &source));
                }
                ui.ctx().memory_mut(|m| m.data.insert_temp(mem, state));
            }
            paint::draw_control(&painter, screen.min, ctrl, false, glass, alpha, 1.0, tex);
        }
        CT::Animator => {
            let source = sv(ctrl, "Source").trim().to_owned();
            let auto = !matches!(sv(ctrl, "AutoPlay").as_str(), "0" | "false" | "False");
            let looping = !matches!(sv(ctrl, "Loop").as_str(), "0" | "false" | "False");
            let size_mode = {
                let s = sv(ctrl, "SizeMode");
                if s.is_empty() {
                    "Fit".to_owned()
                } else {
                    s
                }
            };
            let key = format!("{}|{}", id, source);
            paint::draw_animator(
                &painter, screen, ctrl, &key, &source, auto, looping, &size_mode, alpha, false,
            );
            // spec 021 T12: playback lifecycle read back from the media clock.
            if let Some((frame, loops, ended)) =
                cobolt_media::playback_position(ui.ctx(), &key, auto, looping)
            {
                let mem = ctrl_id.with("anim-pos");
                let prev = ui
                    .ctx()
                    .memory(|m| m.data.get_temp::<(usize, u32, bool)>(mem));
                match prev {
                    None => out.events.push(UiEvent::ev(id, "onStarted")),
                    Some((prev_frame, prev_loops, prev_ended)) => {
                        if prev_frame != frame {
                            out.events.push(UiEvent::with_value(
                                id,
                                "onFrameChanged",
                                &frame.to_string(),
                            ));
                        }
                        if prev_loops != loops && loops > 0 {
                            out.events
                                .push(UiEvent::with_value(id, "onLooped", &loops.to_string()));
                        }
                        if ended && !prev_ended {
                            out.events.push(UiEvent::ev(id, "onEnded"));
                        }
                    }
                }
                if prev != Some((frame, loops, ended)) {
                    ui.ctx()
                        .memory_mut(|m| m.data.insert_temp(mem, (frame, loops, ended)));
                }
            }
        }
        CT::Timer => {
            // Non-visual, but it TICKS: fire `onTick` every Interval ms while on.
            // A Timer's on/off is its own `Enabled` *property* (default true), NOT
            // the generic control-enabled chrome flag: a non-visual control can
            // carry `enabled="false"` in the .cfrm yet still be an active timer
            // (codegen agrees â it seeds WS-<timer>-ENABLED from this property).
            let timer_on = prop_bool(ctrl, "Enabled", true);
            if timer_on {
                let interval_s = sv(ctrl, "Interval")
                    .trim()
                    .parse::<f64>()
                    .unwrap_or(1000.0)
                    .max(10.0)
                    / 1000.0;
                let mem = ctrl_id.with("last_tick");
                let now = ui.input(|i| i.time);
                // A frame that lands a WHISKER before the deadline has still met
                // it. Repaint scheduling is a hint with millisecond granularity,
                // and the clock arithmetic is binary floating point, so "early by
                // a microsecond" is routine â and without this it cost a whole
                // interval. The allowance never affects the RATE, because the
                // schedule below advances by exactly one interval either way; it
                // only decides which frame carries the tick.
                const DEADLINE_TOLERANCE_S: f64 = 0.001;
                let due = interval_s - DEADLINE_TOLERANCE_S;
                let last = match ui.ctx().memory(|m| m.data.get_temp::<f64>(mem)) {
                    None => {
                        ui.ctx().memory_mut(|m| m.data.insert_temp(mem, now));
                        now
                    }
                    Some(last) if now - last >= due => {
                        out.events.push(UiEvent::ev(id, "onTick"));
                        // Advance the SCHEDULE, not the observation.
                        //
                        // Storing `now` re-bases the whole cadence on whenever the
                        // frame happened to land, so a frame arriving a hair early
                        // â which is what floating-point time and a repaint hint
                        // guarantee will happen â cost a WHOLE interval. Measured:
                        // a 100 ms Timer fired 182 times in 300 intervals, and the
                        // losses read exactly like a timer quietly giving up.
                        //
                        // A form that was genuinely stalled (parked off-pane, its
                        // window dragged, a long handler) resyncs to `now` rather
                        // than firing a burst of catch-up ticks: a Timer never
                        // repays missed time, which is the semantics a PowerCOBOL
                        // or isCOBOL developer already expects.
                        let next = if now - last >= interval_s * 2.0 {
                            now
                        } else {
                            last + interval_s
                        };
                        ui.ctx().memory_mut(|m| m.data.insert_temp(mem, next));
                        next
                    }
                    Some(last) => last,
                };
                // Wake exactly when the NEXT tick is due â not every interval/4.
                // Between ticks the form sleeps, so a heavy form no longer
                // re-renders at ~14 fps merely to poll a 250 ms timer (that pegged
                // the CPU). A small floor avoids a zero-delay spin.
                let remaining = (interval_s - (now - last)).clamp(0.005, interval_s);
                ui.ctx()
                    .request_repaint_after(std::time::Duration::from_secs_f64(remaining));
            } else {
                let mem = ctrl_id.with("last_tick");
                ui.ctx().memory_mut(|m| m.data.remove::<f64>(mem));
            }
        }
        CT::BarChart
        | CT::LineChart
        | CT::PieChart
        | CT::AreaChart
        | CT::ScatterChart
        | CT::DonutChart => {
            // Charts render through the SAME path as the designer (draw_control â
            // chart painter) so the running chart matches the canvas (spec 017).
            paint::draw_control(&painter, screen.min, ctrl, false, glass, alpha, 1.0, None);
            // spec 021: onDataChanged when the chart's data-bearing properties
            // change (COBOL AddPoint/Clear/DataSource writes land here).
            if bound.contains(&"onDataChanged") {
                let mut hasher = std::collections::hash_map::DefaultHasher::new();
                use std::hash::{Hash, Hasher};
                sv(ctrl, "Data").hash(&mut hasher);
                sv(ctrl, "DataSource").hash(&mut hasher);
                sv(ctrl, "Values").hash(&mut hasher);
                sv(ctrl, "Labels").hash(&mut hasher);
                let digest = hasher.finish();
                let mem = ctrl_id.with("chart-data");
                let prev = ui.ctx().memory(|m| m.data.get_temp::<u64>(mem));
                if let Some(prev_digest) = prev {
                    if prev_digest != digest {
                        out.events.push(UiEvent::ev(id, "onDataChanged"));
                    }
                }
                if prev != Some(digest) {
                    ui.ctx().memory_mut(|m| m.data.insert_temp(mem, digest));
                }
            }
        }
        CT::AgentObject | CT::SqlDatabase | CT::RestClient => {
            // Non-visual â nothing to draw.
        }
        CT::ProgressBar => {
            paint::draw_control(&painter, screen.min, ctrl, false, glass, alpha, 1.0, None);
            // spec 021 T12: value lifecycle driven by COBOL property writes.
            let value = sv(ctrl, "Value").parse::<f32>().unwrap_or(0.0);
            let maximum = sv(ctrl, "Maximum").parse::<f32>().unwrap_or(100.0);
            let mem = ctrl_id.with("pb-value");
            let prev = ui.ctx().memory(|m| m.data.get_temp::<f32>(mem));
            if let Some(prev_value) = prev {
                if (prev_value - value).abs() > f32::EPSILON {
                    out.events
                        .push(UiEvent::with_value(id, "onValueChanged", &value.to_string()));
                    if value >= maximum && prev_value < maximum {
                        out.events
                            .push(UiEvent::with_value(id, "onCompleted", &value.to_string()));
                    }
                }
            }
            if prev != Some(value) {
                ui.ctx().memory_mut(|m| m.data.insert_temp(mem, value));
            }
        }
        // Faces whose designer rendering IS the real face (Label, Panel, Shape, â¦).
        // 049 â the sidebar is LIVE in interactive surfaces (preview, run):
        // the â° toggles the rail and items click. The `Collapsed` live state
        // drives what the shared painter draws, so preview and Run Form show
        // the same rail the shell shows.
        CT::SideMenu => {
            // Everything here goes through `crate::sidebar`: the rail is laid
            // out ONCE, painted from that layout, and hit-tested against the
            // very same rectangles. There is no second geometry to drift.
            let def = paint::get_menu_cache(ui.ctx(), &ctrl.id);
            let items: &[crate::menu::MenuItem] =
                def.as_ref().map(|d| d.menu.as_slice()).unwrap_or(&[]);

            // Live state, so the preview behaves like the running app without
            // touching the designed control: `Collapsed` and the open parents
            // ride in the control state, the expansion set in egui memory.
            let collapsed = matches!(sv(ctrl, "Collapsed").as_str(), "1" | "true");
            let exp_id = ctrl_id.with("side-expanded");
            let mut expanded: Vec<String> =
                ui.data(|d| d.get_temp::<Vec<String>>(exp_id)).unwrap_or_default();

            let mut live = ctrl.clone();
            live.properties
                .insert("Collapsed".to_owned(), crate::PropValue::Bool(collapsed));
            let selected = sv(ctrl, "SelectedItemId");
            if !selected.is_empty() {
                live.properties.insert(
                    "SelectedItemId".to_owned(),
                    crate::PropValue::String(selected),
                );
            }

            let a8 = (alpha.clamp(0.0, 1.0) * 255.0) as u8;
            let mut state =
                crate::sidebar::state_for_control(ui.ctx(), &live, items, a8, &expanded);

            // The menu pane scrolls when it has more rows than height. The
            // header and footer panes stay put, so the toggle and the footer
            // Panel are always reachable however long the menu is.
            let scroll_id = ctrl_id.with("side-scroll");
            let stored: f32 = ui.data(|d| d.get_temp(scroll_id)).unwrap_or(0.0);
            state.scroll = stored;
            let max_scroll = crate::sidebar::max_scroll(screen, &state);
            let hover_pos = ui.ctx().pointer_interact_pos().filter(|p| screen.contains(*p));
            if max_scroll > 0.0 && hover_pos.is_some() {
                let dy = ui.input(|i| i.smooth_scroll_delta.y);
                if dy != 0.0 {
                    state.scroll -= dy;
                }
            }
            state.scroll = state.scroll.clamp(0.0, max_scroll);
            if state.scroll != stored {
                ui.data_mut(|d| d.insert_temp(scroll_id, state.scroll));
            }

            state.backdrop = form_bg;
            let rows = crate::sidebar::layout(screen, &state);
            state.hovered = hover_pos.and_then(|p| crate::sidebar::row_at(&rows, p));
            crate::sidebar::paint(&painter, screen, &rows, &state);

            // One interaction per laid-out row, using that row's own rect.
            let mut toggle = false;
            let mut flip: Option<String> = None;
            // A menu's `Cursor` is about its ROWS â the control itself is a
            // pane you never point at. It reached nothing before, because only
            // the controls that own a single response ever applied it.
            let row_cursor = ctrl
                .get_prop("Cursor")
                .and_then(|v| cursor_icon_for(v.as_str()));
            for (ix, row) in rows.iter().enumerate() {
                // The row's VISIBLE part, never its full geometry: a row
                // scrolled half under the header must not take a click there.
                let mut r = ui.interact(row.visible, ctrl_id.with(("side-row", ix)), Sense::click());
                if let Some(icon) = row_cursor {
                    r = r.on_hover_cursor(icon);
                }
                if !(r.clicked() && enabled) {
                    continue;
                }
                match &row.kind {
                    // The header IS the toggle, so an empty menu still
                    // collapses â the operator's standing requirement.
                    crate::sidebar::RowKind::Header => toggle = true,
                    crate::sidebar::RowKind::Item { id: item_id, path, .. } => {
                        let Some(item) = crate::sidebar::item_at(items, path) else {
                            continue;
                        };
                        if !item.enabled {
                            continue;
                        }
                        if item.has_children() && !collapsed {
                            flip = Some(item_id.clone());
                        } else {
                            out.prop_updates.push((
                                id.to_owned(),
                                "SelectedItemId".to_owned(),
                                item_id.clone(),
                            ));
                            out.events.push(UiEvent::ev(id, "onMenuItemClick"));
                        }
                    }
                    _ => {}
                }
            }
            if toggle {
                let v = if collapsed { "0" } else { "1" };
                out.prop_updates
                    .push((id.to_owned(), "Collapsed".to_owned(), v.to_owned()));
                out.events.push(UiEvent::ev(
                    id,
                    if collapsed { "onMenuOpen" } else { "onMenuClose" },
                ));
            }
            if let Some(pid) = flip {
                if let Some(p) = expanded.iter().position(|e| e == &pid) {
                    expanded.remove(p);
                } else {
                    expanded.push(pid);
                }
                ui.data_mut(|d| d.insert_temp(exp_id, expanded));
            }
        }

        CT::Label => {
            // A Label IS text, and text a reader cannot select is text they
            // cannot copy — into a ticket, a mail, the next field. Every other
            // painted control stands for something (a face, a glyph, a chart);
            // a caption stands for nothing but itself, so it is hosted through
            // egui's label-selection machinery: drag to select, Cmd/Ctrl+C to
            // copy, and a selection that carries across neighbouring labels.
            //
            // The galley is the PAINTER'S own, captured rather than laid out a
            // second time here. A second layout drifts from the canvas the
            // moment either side changes — and a caption that moves a pixel
            // between the designer and the running form is a bug report.
            let caption = paint::draw_control_capturing_label(
                &painter, screen.min, ctrl, false, glass, alpha, 1.0, None,
            );
            if let Some(cap) = caption {
                // Bold is a second stamp at a half-pixel offset (egui has no
                // guaranteed bold face for an arbitrary system font). It goes
                // UNDER the selectable copy, which paints last and carries the
                // selection highlight over the glyphs.
                if ctrl.get_prop("Bold").map(|v| v.as_bool()).unwrap_or(false) {
                    painter.galley(cap.pos + vec2(0.5, 0.0), cap.galley.clone(), cap.color);
                }
                ui.scope_builder(egui::UiBuilder::new().max_rect(screen), |ui| {
                    ui.set_clip_rect(clip);
                    // Drag selects. FOCUSABLE is removed deliberately: a
                    // caption is not a tab stop, it is text that happens to be
                    // selectable, and TAB must keep walking the form's own
                    // controls in their designed order.
                    let resp = ui.interact(
                        screen,
                        ctrl_id.with("caption"),
                        Sense::click_and_drag() - Sense::FOCUSABLE,
                    );
                    egui::text_selection::LabelSelectionState::label_text_selection(
                        ui,
                        &resp,
                        cap.pos,
                        cap.galley,
                        cap.color,
                        Stroke::NONE,
                    );
                });
            }
        }

        _ => {
            paint::draw_control(&painter, screen.min, ctrl, false, glass, alpha, 1.0, None);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Rect as MRect;

    fn ctrl(id: &str, t: ControlType, x: i32, y: i32, w: i32, h: i32) -> Control {
        let mut c = Control::new(id, t, x, y);
        c.rect = MRect::new(x, y, w, h);
        c
    }

    /// Reads visibility straight off the designed control — the same thing the
    /// preview and every running host ultimately answer with.
    struct DesignedVisibility;
    impl FormState for DesignedVisibility {
        fn visible(&self, base: &Control) -> bool {
            base.visible
        }
    }

    /// **`Visible` hides a Label** — reported as doing nothing at all
    /// (operator, 2026-08-20). A control the engine skips leaves no rect
    /// behind, so `control_rects` is the honest witness: it is what the
    /// designer positions selection handles from, and an absent id means
    /// nothing was drawn. Checked against a visible sibling of the same type
    /// so a broken harness cannot pass this by drawing nothing at all.
    #[test]
    fn an_invisible_label_is_not_drawn() {
        let shown = ctrl("Lbl-Shown", ControlType::Label, 10, 10, 120, 20);
        let mut hidden = ctrl("Lbl-Hidden", ControlType::Label, 10, 50, 120, 20);
        hidden.visible = false;
        let controls = vec![shown, hidden];

        let ctx = egui::Context::default();
        let active = ActiveTabs::new();
        let mut rects: HashMap<String, Rect> = HashMap::new();
        ctx.run_ui(Default::default(), |root_ui| {
            egui::CentralPanel::default().show_inside(root_ui, |ui| {
                ui.set_min_size(Vec2::new(400.0, 300.0));
                let input = RenderInput {
                    controls: &controls,
                    state: &DesignedVisibility,
                    form_size: Vec2::new(400.0, 300.0),
                    glass: true,
                    mode: RenderMode::Interactive,
                    active_tabs: &active,
                    backdrop: Default::default(),
                };
                rects = render_form(ui, &input).control_rects;
            });
        })
        .textures_delta
        .clear();

        assert!(
            rects.contains_key("Lbl-Shown"),
            "the visible label must be drawn — otherwise this test proves nothing: {:?}",
            rects.keys().collect::<Vec<_>>()
        );
        assert!(
            !rects.contains_key("Lbl-Hidden"),
            "a Label with Visible=false must not be drawn, but it left a rect: {:?}",
            rects.get("Lbl-Hidden")
        );
    }

    /// Every rect fill painted in one frame, flattened out of the shape tree.
    fn painted_fills(out: &egui::FullOutput) -> Vec<egui::Color32> {
        fn walk(s: &egui::Shape, into: &mut Vec<egui::Color32>) {
            match s {
                egui::Shape::Rect(r) if r.fill.a() > 0 => into.push(r.fill),
                egui::Shape::Vec(v) => v.iter().for_each(|s| walk(s, into)),
                _ => {}
            }
        }
        let mut fills = Vec::new();
        for cs in &out.shapes {
            walk(&cs.shape, &mut fills);
        }
        fills
    }

    /// Render one control in Interactive mode and hand back the frame.
    fn render_one(ctrl: Control) -> egui::FullOutput {
        let controls = vec![ctrl];
        let ctx = egui::Context::default();
        let active = ActiveTabs::new();
        let mut out = ctx.run_ui(Default::default(), |root_ui| {
            egui::CentralPanel::default().show_inside(root_ui, |ui| {
                ui.set_min_size(Vec2::new(400.0, 200.0));
                let input = RenderInput {
                    controls: &controls,
                    state: &DesignedVisibility,
                    form_size: Vec2::new(400.0, 200.0),
                    glass: false,
                    mode: RenderMode::Interactive,
                    active_tabs: &active,
                    backdrop: Default::default(),
                };
                render_form(ui, &input);
            });
        });
        out.textures_delta.clear();
        out
    }

    /// **A surface somebody else painted must not be painted over.**
    ///
    /// Operator, 2026-08-22: the SideMenu's footer Panel, set to 100 %
    /// transparent, showed a BLACK block instead of the rail behind it. The
    /// footer pass handed the engine a default `Backdrop`, and an unset
    /// backdrop colour is not "nothing" — `backdrop_color` floors it at alpha
    /// 200 on purpose, so that a form with no background set is still a visible
    /// window. Over the rail, that is a black band.
    #[test]
    fn a_backdrop_that_paints_nothing_paints_nothing() {
        let ctx = egui::Context::default();
        let rail = Color32::from_rgb(0x3A, 0x74, 0x94);

        let count = |backdrop: &Backdrop| -> usize {
            let mut fills = Vec::new();
            let mut out = ctx.run_ui(Default::default(), |ui| {
                let rect = Rect::from_min_size(pos2(0.0, 0.0), Vec2::new(100.0, 40.0));
                let painter = ui.painter().clone();
                let paint = paint_backdrop(&painter, rect, backdrop);
                assert_eq!(
                    paint.bg,
                    crate::render::backdrop_color(&backdrop.color_hex, backdrop.transparency),
                    "what is BEHIND must still be reported, painted or not"
                );
            });
            out.textures_delta.clear();
            for cs in &out.shapes {
                if let egui::Shape::Rect(r) = &cs.shape {
                    if r.fill.a() > 0 {
                        fills.push(r.fill);
                    }
                }
            }
            fills.len()
        };

        assert_eq!(
            count(&Backdrop::behind(rail)),
            0,
            "a band the rail already painted must take no second background"
        );
        assert!(
            count(&Backdrop::default()) > 0,
            "…while a FORM still paints its own, or this test proves nothing"
        );
    }

    /// **A ToolBar's BackgroundColor is painted** — operator, 2026-08-22:
    /// "Toolbar, background color … does not work".
    ///
    /// It was voided twice over. A toolbar ships at `Transparency = 100`, and
    /// the face was gated on `transparency < 100`, so the colour never got as
    /// far as the painter; and when it did, `SurfaceRole::Card` answered with
    /// the theme's own fill and never reached the caller's colour at all. The
    /// operator picked a colour, nothing happened, and nothing said why.
    #[test]
    fn a_toolbar_paints_the_background_colour_it_was_given() {
        let mut bar = ctrl("TB", ControlType::ToolBar, 10, 10, 300, 44);
        bar.set_prop("BackgroundColor", crate::PropValue::String("#FF0000".into()));
        let fills = painted_fills(&render_one(bar));
        assert!(
            fills.iter().any(|c| (c.r(), c.g(), c.b()) == (255, 0, 0)),
            "the chosen colour must be painted at the shipped defaults; got {fills:?}"
        );
    }

    /// …and a toolbar nobody coloured still paints no frame. The fix must not
    /// hand every existing bare toolbar a card it never had.
    #[test]
    fn an_uncoloured_toolbar_still_has_no_frame() {
        let bar = ctrl("TB", ControlType::ToolBar, 10, 10, 300, 44);
        let seeded = bar
            .get_prop("BackgroundColor")
            .map(|v| v.as_str().to_owned())
            .unwrap_or_default();
        assert_eq!(
            seeded,
            crate::model::DEFAULT_BACKGROUND_COLOR,
            "a fresh ToolBar must still carry the seeded sentinel"
        );
        let target = crate::paint::parse_color(&seeded);
        let fills = painted_fills(&render_one(bar));
        assert!(
            !fills
                .iter()
                .any(|c| (c.r(), c.g(), c.b()) == (target.r(), target.g(), target.b())),
            "an untouched toolbar must stay frameless; got {fills:?}"
        );
    }

    /// **A Label's text selects and copies** — operator, 2026-08-22: a caption
    /// could be read but never quoted, because a painted galley is pixels and
    /// pixels have no selection.
    ///
    /// Driven the whole way through rather than asserted on a flag: press on
    /// the caption, drag across it, press Copy, and read what the platform was
    /// actually asked to put on the clipboard. A `CopyText` carrying part of
    /// the caption is the only evidence that all three — the response, the
    /// selection and the clipboard — are wired to each other.
    #[test]
    fn label_text_selects_and_copies_to_the_clipboard() {
        const CAPTION: &str = "Totals as at 22 August";
        let mut label = ctrl("LBL", ControlType::Label, 10, 10, 260, 24);
        label.set_prop("Caption", crate::PropValue::String(CAPTION.into()));
        let controls = vec![label];

        let ctx = egui::Context::default();
        let active = ActiveTabs::new();
        let mut rects: HashMap<String, Rect> = HashMap::new();
        let screen = Rect::from_min_size(pos2(0.0, 0.0), Vec2::new(400.0, 300.0));

        let mut frame = |events: Vec<egui::Event>, rects: &mut HashMap<String, Rect>| {
            let raw = egui::RawInput {
                screen_rect: Some(screen),
                events,
                ..Default::default()
            };
            let mut out = ctx.run_ui(raw, |root_ui| {
                egui::CentralPanel::default().show_inside(root_ui, |ui| {
                    ui.set_min_size(Vec2::new(400.0, 300.0));
                    let input = RenderInput {
                        controls: &controls,
                        state: &DesignedVisibility,
                        form_size: Vec2::new(400.0, 300.0),
                        glass: true,
                        mode: RenderMode::Interactive,
                        active_tabs: &active,
                        backdrop: Default::default(),
                    };
                    *rects = render_form(ui, &input).control_rects;
                });
            });
            // A dropped TexturesDelta panics if it still carries work; nothing
            // here uploads textures, so hand it back cleared.
            out.textures_delta.clear();
            out
        };

        // One frame to learn where the caption actually landed — egui resolves
        // interaction against the PREVIOUS frame's widgets, so the press below
        // needs this one to have registered the label first.
        frame(Vec::new(), &mut rects);
        let rect = *rects.get("LBL").expect("the label must be drawn");
        let press = rect.left_center() + Vec2::new(4.0, 0.0);
        let release = rect.left_center() + Vec2::new(rect.width() - 8.0, 0.0);
        let button = |pos: egui::Pos2, pressed: bool| egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: Default::default(),
        };

        frame(
            vec![egui::Event::PointerMoved(press), button(press, true)],
            &mut rects,
        );
        // Two moves: the first begins the drag, the second is where egui
        // extends the selection to.
        frame(vec![egui::Event::PointerMoved(release)], &mut rects);
        frame(vec![egui::Event::PointerMoved(release)], &mut rects);

        let selecting = ctx
            .plugin::<egui::text_selection::LabelSelectionState>()
            .lock()
            .has_selection();
        assert!(
            selecting,
            "dragging across a Label's caption must select it — nothing was selected"
        );

        let out = frame(vec![egui::Event::Copy], &mut rects);
        let copied: Vec<String> = out
            .platform_output
            .commands
            .iter()
            .filter_map(|c| match c {
                egui::OutputCommand::CopyText(t) => Some(t.clone()),
                _ => None,
            })
            .collect();
        assert!(
            !copied.is_empty(),
            "Copy over a selected caption must reach the clipboard; commands were {:?}",
            out.platform_output.commands
        );
        let text = copied.join("");
        assert!(
            !text.trim().is_empty() && CAPTION.contains(text.trim()),
            "the clipboard must carry the caption's own text, got {text:?}"
        );
    }

    // ââ CORNER GUARDIAN regression tests âââââââââââââââââââââââââââââââââââââ
    // These pin the rule that the notch mask must only touch corners a child
    // actually reaches; if they fail, a clean container corner is being masked
    // (painted over) again â the bug corner_notch_rounding was added to stop.

    #[test]
    fn corner_notch_guardian_leaves_clean_corners_untouched() {
        use std::collections::HashMap;
        let container = ctrl("PANEL", ControlType::Panel, 0, 0, 200, 150);
        let mut child = ctrl("CHILD", ControlType::Label, 80, 60, 40, 20);
        child.parent = Some("PANEL".into());
        let controls = vec![container, child];
        let cont = Rect::from_min_size(pos2(0.0, 0.0), Vec2::new(200.0, 150.0));
        let mut rects = HashMap::new();
        rects.insert("PANEL".to_string(), cont);
        // Child parked in the middle â reaches no corner.
        rects.insert(
            "CHILD".to_string(),
            Rect::from_min_size(pos2(80.0, 60.0), Vec2::new(40.0, 20.0)),
        );
        let r = corner_notch_rounding(cont, 20.0, &controls, 0, &rects);
        assert_eq!(
            r,
            egui::CornerRadius::ZERO,
            "no child at any corner â NOTHING masked (panel keeps its own corners)"
        );
    }

    /// A Maps control paints its own tiles past its arc, so it needs the same
    /// notch mask a container needs for its children — on **every** corner,
    /// because the basemap covers the whole face.
    ///
    /// It has no descendants, so the container rule would never have selected
    /// it: a map with `CornerRadius = 34` drew square tile corners inside a
    /// rounded selection outline (operator, 2026-08-21). This also pins the two
    /// exclusions that keep the blanket radius honest — a nested map cannot use
    /// this mask (its notches must reveal the parent, not the form backdrop),
    /// and no radius means no mask.
    #[test]
    fn a_maps_control_masks_all_four_corners_and_a_nested_one_masks_none() {
        use std::collections::HashMap;
        let map = ctrl("MAP-1", ControlType::Maps, 0, 0, 880, 700);
        let controls = vec![map];
        let rect = Rect::from_min_size(pos2(0.0, 0.0), Vec2::new(880.0, 700.0));
        let mut rects = HashMap::new();
        rects.insert("MAP-1".to_string(), rect);

        let r = notch_mask_rounding(&controls, 0, rect, 34.0, &rects)
            .expect("a rounded map needs its notches repainted");
        assert_eq!(
            r,
            egui::CornerRadius::same(crate::paint::cr8(34.0)),
            "tiles cover the whole face, so every corner is reached"
        );

        // No radius, no mask — nothing is being rounded away.
        assert!(notch_mask_rounding(&controls, 0, rect, 0.0, &rects).is_none());

        // Nested: the form backdrop is the wrong paint for those notches.
        let mut nested = ctrl("MAP-2", ControlType::Maps, 0, 0, 200, 200);
        nested.parent = Some("PANEL".into());
        let nested_controls = vec![nested];
        assert!(
            notch_mask_rounding(&nested_controls, 0, rect, 34.0, &rects).is_none(),
            "a nested map must not have the form backdrop painted into its corners"
        );

        // And the container rule is unchanged: an empty Panel still masks
        // nothing, so this did not become a blanket mask for everything.
        let empty = vec![ctrl("PANEL", ControlType::Panel, 0, 0, 200, 150)];
        let mut prects = HashMap::new();
        prects.insert("PANEL".to_string(), rect);
        // `None` rather than `Some(ZERO)`: both mask nothing, but skipping the
        // call is the honest answer for a container with nothing to bleed.
        assert_eq!(
            notch_mask_rounding(&empty, 0, rect, 20.0, &prects),
            None,
            "a childless Panel keeps its own corners"
        );
        // A control that paints nothing past its arc is never masked.
        let label = vec![ctrl("L", ControlType::Label, 0, 0, 100, 20)];
        assert!(notch_mask_rounding(&label, 0, rect, 20.0, &prects).is_none());
    }

    #[test]
    fn corner_notch_guardian_masks_only_the_reached_corner() {
        use std::collections::HashMap;
        let container = ctrl("PANEL", ControlType::Panel, 0, 0, 200, 150);
        let mut child = ctrl("CHILD", ControlType::PictureBox, 0, 140, 30, 20);
        child.parent = Some("PANEL".into());
        let controls = vec![container, child];
        let cont = Rect::from_min_size(pos2(0.0, 0.0), Vec2::new(200.0, 150.0));
        let mut rects = HashMap::new();
        rects.insert("PANEL".to_string(), cont);
        // Child overlapping the bottom-left (SW) corner square only.
        rects.insert(
            "CHILD".to_string(),
            Rect::from_min_size(pos2(0.0, 140.0), Vec2::new(30.0, 20.0)),
        );
        let r = corner_notch_rounding(cont, 20.0, &controls, 0, &rects);
        assert_eq!(r.sw, 20, "child in bottom-left â SW masked");
        assert_eq!(r.nw, 0, "NW is clean â untouched");
        assert_eq!(r.ne, 0, "NE is clean â untouched");
        assert_eq!(r.se, 0, "SE is clean â untouched");
    }

    /// Which of the four corner squares of a 200Ã150 / r=20 panel a restore stroke
    /// landed in, derived from each stroke shape's clip rect. Restore clips each
    /// corner's rim to that corner's square, so the clip rect names the corner.
    fn restored_corners(rect: Rect, r: f32, masked: egui::CornerRadius) -> std::collections::BTreeSet<&'static str> {
        let ctx = egui::Context::default();
        crate::paint::set_glass_style(&ctx, crate::model::GlassStyle::Enhanced);
        let mut panel = Control::new("PNL", ControlType::Panel, rect.min.x as i32, rect.min.y as i32);
        panel.rect = crate::model::Rect::new(
            rect.min.x as i32,
            rect.min.y as i32,
            rect.width() as i32,
            rect.height() as i32,
        );
        panel.set_prop("BorderStyle", crate::model::PropValue::String("Single".into()));
        panel.set_prop("BorderWidth", crate::model::PropValue::Int(1));
        let mut input = egui::RawInput::default();
        input.screen_rect = Some(Rect::from_min_size(pos2(0.0, 0.0), Vec2::new(600.0, 400.0)));
        let mut full = ctx.run_ui(input, |root_ui| {
            let painter = root_ui.painter_at(Rect::from_min_size(pos2(0.0, 0.0), Vec2::new(600.0, 400.0)));
            crate::paint::restore_container_outline(&painter, &panel, rect, r, true, masked);
        });
        full.textures_delta.clear();
        let mut hit = std::collections::BTreeSet::new();
        let classify = |c: egui::Pos2| -> Option<&'static str> {
            let left = c.x < rect.min.x + r;
            let right = c.x > rect.max.x - r;
            let top = c.y < rect.min.y + r;
            let bot = c.y > rect.max.y - r;
            match (left, right, top, bot) {
                (true, _, true, _) => Some("nw"),
                (_, true, true, _) => Some("ne"),
                (_, true, _, true) => Some("se"),
                (true, _, _, true) => Some("sw"),
                _ => None,
            }
        };
        for cs in &full.shapes {
            if let egui::Shape::Rect(rs) = &cs.shape {
                if rs.stroke.width > 0.0 {
                    if let Some(corner) = classify(cs.clip_rect.center()) {
                        hit.insert(corner);
                    }
                }
            }
        }
        hit
    }

    #[test]
    fn restore_outline_only_touches_masked_corners() {
        // Regression: `restore_container_outline` used to redraw the rim on ALL four
        // corners unconditionally, double-stroking the face's own rim on corners the
        // (now per-corner) notch mask left clean â a light spur at the corner
        // (visible on databound DataGrids / dropshadowed cards after egui 0.35).
        let rect = Rect::from_min_size(pos2(0.0, 0.0), Vec2::new(200.0, 150.0));
        let r = 20.0;
        let cr = crate::paint::cr8(r);

        // Only the SW corner masked â only SW restored.
        let sw_only = egui::CornerRadius { nw: 0, ne: 0, se: 0, sw: cr };
        let hit = restored_corners(rect, r, sw_only);
        assert_eq!(
            hit.into_iter().collect::<Vec<_>>(),
            vec!["sw"],
            "restore must touch ONLY the masked (SW) corner, never the clean ones",
        );

        // Nothing masked â nothing restored (no spur on a container with a clean rim).
        let hit = restored_corners(rect, r, egui::CornerRadius::ZERO);
        assert!(
            hit.is_empty(),
            "no corner masked â restore must be a no-op, saw {hit:?}",
        );
    }

    /// Restore means RESTORE: a control whose face draws no outline has none to
    /// put back.
    ///
    /// A Maps control joined the notch mask in 1.61.134 and inherited the restore
    /// with it — but `draw_control`'s Maps branch paints its halo, its gradient
    /// and its tiles and returns before any rim or border, so a map has no edge
    /// line anywhere. Restoring one gave it a hard 1px border on the four corner
    /// arcs and nowhere else: a dark hair at each corner of a RUNNING map, absent
    /// from the designer canvas, which never calls this function at all
    /// (operator, 2026-08-21).
    #[test]
    fn restore_outline_skips_a_control_whose_face_draws_none() {
        let rect = Rect::from_min_size(pos2(0.0, 0.0), Vec2::new(880.0, 700.0));
        let r = 51.0;
        let all = egui::CornerRadius::same(crate::paint::cr8(r));

        let strokes = |ct: ControlType| {
            let ctx = egui::Context::default();
            crate::paint::set_glass_style(&ctx, crate::model::GlassStyle::Neumorphic);
            let mut c = Control::new("C", ct, 0, 0);
            c.rect = crate::model::Rect::new(0, 0, rect.width() as i32, rect.height() as i32);
            c.set_prop("CornerRadius", PropValue::Int(r as i64));
            let mut input = egui::RawInput::default();
            input.screen_rect =
                Some(Rect::from_min_size(pos2(0.0, 0.0), Vec2::new(1000.0, 800.0)));
            let mut full = ctx.run_ui(input, |root_ui| {
                let painter = root_ui.painter_at(Rect::from_min_size(
                    pos2(0.0, 0.0),
                    Vec2::new(1000.0, 800.0),
                ));
                crate::paint::restore_container_outline(&painter, &c, rect, r, true, all);
            });
            full.textures_delta.clear();
            fn count(s: &egui::Shape, n: &mut usize) {
                match s {
                    egui::Shape::Vec(v) => v.iter().for_each(|s| count(s, n)),
                    egui::Shape::Rect(rs) if rs.stroke.width > 0.0 => *n += 1,
                    _ => {}
                }
            }
            let mut n = 0;
            for cs in &full.shapes {
                count(&cs.shape, &mut n);
            }
            n
        };

        assert_eq!(
            strokes(ControlType::Maps),
            0,
            "a map's face draws no outline, so its corners must get none either - \
             that invented border IS the dark hair at the corners"
        );
        assert!(
            strokes(ControlType::Panel) > 0,
            "a Panel DOES draw a rim on its face, so the mask erases it at the \
             corners and the restore must still put it back"
        );
    }

    #[test]
    fn datagrid_line_clip_keeps_lines_inside_the_arc() {
        let rect = Rect::from_min_size(pos2(0.0, 0.0), Vec2::new(200.0, 100.0));
        let r = 20.0;
        // A vertical separator hugging the left edge would poke into both left
        // corner notches; clipping pulls its ends inside the arc.
        let v = clip_datagrid_line_to_corners(rect, r, [pos2(2.0, 0.0), pos2(2.0, 100.0)]);
        assert_eq!(v[0].x, 2.0);
        assert!(
            v[0].y > 0.5 && v[1].y < 99.5,
            "near-edge vertical line must clip away from the corners, got {v:?}"
        );
        // A separator in the middle clears the corners â untouched.
        let mid = clip_datagrid_line_to_corners(rect, r, [pos2(100.0, 0.0), pos2(100.0, 100.0)]);
        assert_eq!(mid, [pos2(100.0, 0.0), pos2(100.0, 100.0)]);
        // A horizontal line hugging the bottom is pulled in at both ends.
        let h = clip_datagrid_line_to_corners(rect, r, [pos2(0.0, 98.0), pos2(200.0, 98.0)]);
        assert!(
            h[0].x > 0.5 && h[1].x < 199.5,
            "near-bottom horizontal line must clip away from the corners, got {h:?}"
        );
    }

    fn bound_repeating_group(count: i64) -> Vec<Control> {
        use crate::model::PropValue;
        let mut group = ctrl("CARD", ControlType::GroupBox, 0, 0, 200, 60);
        group.set_prop("IsRepeatingGroup", PropValue::Bool(true));
        group.set_prop("DataSource", PropValue::String("Customers".into()));
        group.set_prop("ItemCount", PropValue::Int(count));
        group.set_prop("LayoutDirection", PropValue::String("Vertical".into()));
        group.set_prop("ItemSpacing", PropValue::Int(10));
        let mut member = ctrl("NAME", ControlType::Label, 10, 10, 80, 20);
        member.parent = Some("CARD".into());
        vec![group, member]
    }

    #[test]
    fn repeating_group_expands_into_runtime_instances() {
        let expanded = expand_repeating_groups(&bound_repeating_group(3)).expect("should expand");
        // 3 instances Ã 2 controls.
        assert_eq!(expanded.len(), 6);
        // Every instance â including the first â uses the group-prefixed scheme
        // (task 1): no clone keeps the bare designed id.
        assert!(expanded.iter().all(|c| c.id != "CARD" && c.id != "NAME"));
        // Instance 1 at the origin; group id "CARD.CARD-1", member "CARD.CARD-1.NAME".
        let g1 = expanded.iter().find(|c| c.id == "CARD.CARD-1").expect("g1");
        assert_eq!(g1.rect.y, 0);
        let m1 = expanded
            .iter()
            .find(|c| c.id == "CARD.CARD-1.NAME")
            .expect("m1");
        assert_eq!(m1.parent.as_deref(), Some("CARD.CARD-1"));
        // Instance 2 shifted down by group height + spacing (60 + 10 = 70) (task 2).
        let g2 = expanded.iter().find(|c| c.id == "CARD.CARD-2").expect("g2");
        assert_eq!(g2.rect.y, 70);
        let m2 = expanded
            .iter()
            .find(|c| c.id == "CARD.CARD-2.NAME")
            .expect("m2");
        assert_eq!(m2.parent.as_deref(), Some("CARD.CARD-2"));
        assert_eq!(m2.rect.y, 10 + 70);
        // Instance 3 shifted twice.
        assert_eq!(
            expanded
                .iter()
                .find(|c| c.id == "CARD.CARD-3")
                .unwrap()
                .rect
                .y,
            140
        );

        // A form without a repeating group is left untouched.
        let plain = vec![ctrl("BTN", ControlType::Button, 0, 0, 40, 20)];
        assert!(expand_repeating_groups(&plain).is_none());
    }

    #[test]
    fn card_appear_effects_stagger_and_place() {
        let d = CARD_APPEAR_DUR;
        // None: always identity, never animating.
        let (t, a) = card_appear_transform(
            PlacementEffect::None,
            3,
            0.05,
            (10.0, 20.0),
            false,
            CARD_APPEAR_DUR,
        );
        assert!(!a && t.dx == 0.0 && t.alpha == 1.0);

        // FadeIn: card 2 is invisible before its window, mid-fade during, done after.
        let (t0, _) = card_appear_transform(
            PlacementEffect::FadeIn,
            2,
            0.0,
            (0.0, 0.0),
            false,
            CARD_APPEAR_DUR,
        );
        assert_eq!(t0.alpha, 0.0, "card 2 hidden before its turn");
        let (tm, anim) = card_appear_transform(
            PlacementEffect::FadeIn,
            2,
            d + d * 0.5,
            (0.0, 0.0),
            false,
            d,
        );
        assert!(tm.alpha > 0.3 && tm.alpha < 0.7 && anim, "card 2 mid-fade");
        let (tf, done) = card_appear_transform(
            PlacementEffect::FadeIn,
            2,
            2.0 * d + 0.01,
            (0.0, 0.0),
            false,
            d,
        );
        assert!(tf.alpha == 1.0 && !done, "card 2 fully faded in");

        // Deal: card 2 starts fully offset (on the first card) and ends at final.
        let (ds, _) = card_appear_transform(PlacementEffect::Deal, 2, d, (0.0, -70.0), false, d);
        assert_eq!(ds.dy, -70.0, "card 2 begins stacked on the first card");
        let (de, done) = card_appear_transform(
            PlacementEffect::Deal,
            2,
            2.0 * d + 0.01,
            (0.0, -70.0),
            false,
            d,
        );
        assert!(de.dy == 0.0 && !done, "card 2 dealt to its final spot");
        // Deal off-screen: no phantom fly-in â placed at final immediately.
        let (dc, anim) = card_appear_transform(PlacementEffect::Deal, 5, d, (0.0, -280.0), true, d);
        assert!(
            dc.dy == 0.0 && !anim,
            "clipped card is placed, not animated"
        );
    }

    #[test]
    fn card_zoom_effects_scale_without_position_interpolation() {
        let d = CARD_APPEAR_DUR;
        assert_eq!(PlacementEffect::parse("Zoom In"), PlacementEffect::ZoomIn);
        assert_eq!(PlacementEffect::parse("zoom-out"), PlacementEffect::ZoomOut);

        let (zin0, anim0) =
            card_appear_transform(PlacementEffect::ZoomIn, 1, 0.0, (80.0, 90.0), false, d);
        assert_eq!(zin0.dx, 0.0, "zoom-in must not use previous-position dx");
        assert_eq!(zin0.dy, 0.0, "zoom-in must not use previous-position dy");
        assert!(zin0.scale < 1.0 && anim0, "zoom-in starts smaller");

        let (zin_done, done) =
            card_appear_transform(PlacementEffect::ZoomIn, 1, d + 0.01, (80.0, 90.0), false, d);
        assert_eq!(zin_done.scale, 1.0, "zoom-in ends at normal scale");
        assert!(!done);

        let (zout0, anim0) =
            card_appear_transform(PlacementEffect::ZoomOut, 1, 0.0, (80.0, 90.0), false, d);
        assert_eq!(zout0.dx, 0.0, "zoom-out must not use previous-position dx");
        assert_eq!(zout0.dy, 0.0, "zoom-out must not use previous-position dy");
        assert!(zout0.scale > 1.0 && anim0, "zoom-out starts larger");

        let (zout_done, done) = card_appear_transform(
            PlacementEffect::ZoomOut,
            1,
            d + 0.01,
            (80.0, 90.0),
            false,
            d,
        );
        assert_eq!(zout_done.scale, 1.0, "zoom-out ends at normal scale");
        assert!(!done);

        let (offscreen, anim) =
            card_appear_transform(PlacementEffect::ZoomIn, 3, d, (80.0, 90.0), true, d);
        assert_eq!(offscreen.dx, 0.0);
        assert_eq!(offscreen.dy, 0.0);
        assert_eq!(offscreen.scale, 1.0);
        assert_eq!(offscreen.alpha, 1.0);
        assert!(!anim, "offscreen cards skip zoom animation");
    }

    #[test]
    fn card_final_screen_rect_uses_group_root_metadata() {
        let mut child = ctrl("CARD.CARD-2.NAME", ControlType::Label, 120, 70, 80, 20);
        child.set_prop("_CardRootX", crate::model::PropValue::Int(100));
        child.set_prop("_CardRootY", crate::model::PropValue::Int(50));
        child.set_prop("_CardRootW", crate::model::PropValue::Int(240));
        child.set_prop("_CardRootH", crate::model::PropValue::Int(90));

        let child_screen = Rect::from_min_size(Pos2::new(320.0, 170.0), Vec2::new(80.0, 20.0));
        let card_screen = card_final_screen_rect(&child, child_screen);
        assert_eq!(card_screen.min, Pos2::new(300.0, 150.0));
        assert_eq!(card_screen.size(), Vec2::new(240.0, 90.0));
    }

    #[test]
    fn databound_group_with_zero_rows_renders_no_instances() {
        // 0 rows (task 3): the group and its children disappear entirely.
        let expanded = expand_repeating_groups(&bound_repeating_group(0)).expect("still processed");
        assert!(expanded.is_empty(), "0 rows must produce no cards");
    }

    #[test]
    fn unbound_repeating_group_shows_one_template() {
        use crate::model::PropValue;
        // No DataSource â ItemCount is ignored; PreviewItemCount governs (default 1).
        let mut group = ctrl("CARD", ControlType::GroupBox, 0, 0, 200, 60);
        group.set_prop("IsRepeatingGroup", PropValue::Bool(true));
        group.set_prop("ItemCount", PropValue::Int(0));
        group.set_prop("PreviewItemCount", PropValue::Int(1));
        let mut member = ctrl("NAME", ControlType::Label, 10, 10, 80, 20);
        member.parent = Some("CARD".into());
        let expanded = expand_repeating_groups(&vec![group, member]).expect("expand");
        assert_eq!(expanded.len(), 2, "unbound group still shows its template");
        assert!(expanded.iter().any(|c| c.id == "CARD.CARD-1.NAME"));
    }

    #[test]
    fn even_tile_centers_balances_margins() {
        // 100px wide, nominal 12 â round(8.33)=8 cells, spacing 12.5.
        let centers = even_tile_centers(0.0, 100.0, 12.0);
        assert_eq!(centers.len(), 8);
        // Leading and trailing margins are equal (half a cell), not a fixed offset.
        let leading = centers[0];
        let trailing = 100.0 - centers[centers.len() - 1];
        assert!((leading - trailing).abs() < 0.001, "margins must match");
        assert!((leading - 6.25).abs() < 0.001);
        // Spacing between adjacent centers is uniform.
        assert!((centers[1] - centers[0] - 12.5).abs() < 0.001);
    }

    #[test]
    fn even_tile_centers_degenerate_inputs() {
        assert!(even_tile_centers(10.0, 10.0, 12.0).is_empty());
        assert!(even_tile_centers(0.0, 100.0, 0.0).is_empty());
        // A rect smaller than the nominal spacing still yields one centered tile.
        let centers = even_tile_centers(0.0, 5.0, 12.0);
        assert_eq!(centers.len(), 1);
        assert!((centers[0] - 2.5).abs() < 0.001);
    }

    #[test]
    fn backdrop_color_black_becomes_navy() {
        // Unset / pure black â default dark navy (matches preview + run).
        assert_eq!(
            backdrop_color("#00000000", 0),
            Color32::from_rgba_premultiplied(20, 22, 45, 255)
        );
        assert_eq!(
            backdrop_color("000000", 0),
            Color32::from_rgba_premultiplied(20, 22, 45, 255)
        );
        assert_eq!(
            backdrop_color("", 0),
            Color32::from_rgba_premultiplied(20, 22, 45, 255)
        );
        // A real colour is honoured.
        let c = backdrop_color("#204060", 0);
        assert_eq!((c.r(), c.g(), c.b()), (0x20, 0x40, 0x60));
    }

    #[test]
    fn merge_props_overrides_geometry_and_values() {
        let base = ctrl("T", ControlType::TextBox, 5, 6, 100, 24);
        let mut p = std::collections::HashMap::new();
        p.insert("X".to_string(), "40".to_string());
        p.insert("Text".to_string(), "hello".to_string());
        let live = merge_props(&base, p.iter());
        assert_eq!(live.rect.x, 40);
        assert_eq!(live.get_prop("Text").unwrap().as_str(), "hello");
        assert_eq!(live.rect.y, 6, "untouched geometry preserved");
    }

    #[test]
    fn picturebox_container_border_is_parent_visual_rect_and_radius() {
        // A PictureBox child of a rounded GroupBox is clipped to the parent's
        // BORDER path: the parent's full visual rect (NOT the inset content rect)
        // and its corner radius, so overflow is cut by the container shape.
        let mut gb = ctrl("GB", ControlType::GroupBox, 0, 0, 200, 200);
        gb.set_prop("CornerRadius", 12);
        let mut pic = ctrl("Pic", ControlType::PictureBox, 10, 10, 180, 180);
        pic.parent = Some("GB".into());
        let controls = vec![gb, pic];
        let active = ActiveTabs::new();
        // Build a fresh static RenderInput per probe and clip child #1 (avoids
        // moving the non-Copy Backdrop across reuses).
        let mk = |controls: &[Control]| -> Option<(Rect, f32)> {
            picturebox_container_border(
                controls,
                &DesignedState,
                1,
                egui::pos2(0.0, 0.0),
                egui::Vec2::ZERO,
            )
        };
        let (border, rad) = mk(&controls).expect("rounded parent â border clip");
        assert_eq!(
            border,
            Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(200.0, 200.0))
        );
        assert_eq!(rad, 12.0);
        assert_eq!(container_clip_prop(border, rad), "0,0,200,200,12,1,1,1,1");

        // A square (non-rounded) container needs no clip.
        let mut panel = ctrl("P", ControlType::Panel, 0, 0, 200, 200);
        panel.set_prop("CornerRadius", 0);
        let mut pic2 = ctrl("Pic", ControlType::PictureBox, 10, 10, 50, 50);
        pic2.parent = Some("P".into());
        let controls2 = vec![panel, pic2];
        assert_eq!(mk(&controls2), None);

        // A top-level PictureBox (no parent) needs no clip.
        let spacer = ctrl("S", ControlType::Label, 0, 0, 10, 10);
        let mut lone = ctrl("Pic", ControlType::PictureBox, 10, 10, 50, 50);
        lone.parent = None;
        assert_eq!(mk(&[spacer, lone]), None);
    }

    #[test]
    fn ancestor_clip_rect_shifts_only_inner_nonscroller_containers() {
        // Simulate: outer scrolling Panel (fixed clip) containing a rounded
        // databound-style GroupBox card (its clip must shift with -scroll so
        // children inside the card are not clipped away). Verifies the fix for
        // transparency/frame over card inners on scroll.
        let mut panel = ctrl("Pnl", ControlType::Panel, 0, 0, 300, 200);
        panel.set_prop("VScroll", crate::model::PropValue::Bool(true));
        let mut card = ctrl("GB", ControlType::GroupBox, 10, 20, 250, 60);
        card.set_prop("CornerRadius", crate::model::PropValue::Int(8));
        card.parent = Some("Pnl".into());
        let mut inner = ctrl("Lbl", ControlType::Label, 20, 30, 100, 20);
        inner.parent = Some("GB".into());
        let controls = vec![panel, card, inner];
        let origin = egui::pos2(5.0, 5.0);
        let scroll = egui::vec2(0.0, 42.0);

        // For the inner label (inside card), clip should be intersection of:
        // - panel's content rect (fixed, because it is the scroller)
        // - card's content rect shifted by -scroll (card is non-scroller container in content space)
        let clip = ancestor_clip_rect(&controls, 2, origin, scroll, &DesignedState).expect("clip");

        // Compute expected using same content_rects + rules.
        let p_cr = controls[0].content_rect();
        let fixed_panel = Rect::from_min_size(
            origin + egui::vec2(p_cr.x as f32, p_cr.y as f32),
            egui::vec2(p_cr.w as f32, p_cr.h as f32),
        );
        let c_cr = controls[1].content_rect();
        let shifted_card = Rect::from_min_size(
            origin + egui::vec2(c_cr.x as f32, c_cr.y as f32) - scroll,
            egui::vec2(c_cr.w as f32, c_cr.h as f32),
        );
        let expected = fixed_panel.intersect(shifted_card);

        assert!(
            (clip.min.x - expected.min.x).abs() < 0.01
                && (clip.min.y - expected.min.y).abs() < 0.01
                && (clip.max.x - expected.max.x).abs() < 0.01
                && (clip.max.y - expected.max.y).abs() < 0.01,
            "clip must match fixed_panel â© (card_content - scroll)"
        );

        // Prove the shift was applied to the card part: an unshifted card clip
        // would produce a different rect (higher y).
        let unshifted_card = Rect::from_min_size(
            origin + egui::vec2(c_cr.x as f32, c_cr.y as f32),
            egui::vec2(c_cr.w as f32, c_cr.h as f32),
        );
        let would_be_unshifted = fixed_panel.intersect(unshifted_card);
        assert!(
            (would_be_unshifted.min.y - clip.min.y).abs() > 0.1,
            "without shift the clip y would be different (higher)"
        );
    }

    #[test]
    fn ancestor_clip_rect_repeating_group_instance_shifting() {
        use crate::model::PropValue;
        let mut panel = ctrl("Pnl", ControlType::Panel, 0, 0, 300, 400);
        panel.set_prop("VScroll", PropValue::Bool(true));
        let mut card = ctrl("GB", ControlType::GroupBox, 10, 20, 250, 100);
        card.set_prop("IsRepeatingGroup", PropValue::Bool(true));
        card.set_prop("DataSource", PropValue::String("MyTable".into()));
        card.set_prop("ItemCount", PropValue::Int(3));
        card.set_prop("LayoutDirection", PropValue::String("Vertical".into()));
        card.set_prop("ItemSpacing", PropValue::Int(10));
        card.parent = Some("Pnl".into());
        let mut inner = ctrl("Lbl", ControlType::Label, 20, 30, 100, 20);
        inner.parent = Some("GB".into());

        let designed = vec![panel, card, inner];
        let controls = expand_repeating_groups(&designed).expect("expand");

        // Find "GB.GB-2" (instance 2 of card) and "GB.GB-2.Lbl" (instance 2 of label)
        let lbl_idx = controls
            .iter()
            .position(|c| c.id == "GB.GB-2.Lbl")
            .unwrap_or_else(|| {
                let ids: Vec<String> = controls.iter().map(|c| c.id.clone()).collect();
                panic!("Could not find GB.GB-2.Lbl. Available IDs: {:?}", ids);
            });

        let origin = egui::pos2(0.0, 0.0);
        let scroll = egui::vec2(0.0, 30.0);

        let clip =
            ancestor_clip_rect(&controls, lbl_idx, origin, scroll, &DesignedState).expect("clip");

        // Card height is 100, spacing is 10. Instance 2 is shifted down by dy = 110.
        // Scroll is 30.
        // Expected card content rect for instance 2 is (10 + 2, 20 + 110 + 2) = (12, 132).
        // Since scroll is 30, the card's clip rect should shift to 132 - 30 = 102.
        assert_eq!(clip.min.y, 102.0);
    }

    #[test]
    fn test_panel_content_size_calculation() {
        use crate::model::PropValue;
        let mut panel = ctrl("Pnl", ControlType::Panel, 0, 0, 300, 400);
        panel.set_prop("VScroll", PropValue::Bool(true));
        let mut card = ctrl("GB", ControlType::GroupBox, 10, 20, 250, 100);
        card.set_prop("IsRepeatingGroup", PropValue::Bool(true));
        card.set_prop("DataSource", PropValue::String("MyTable".into()));
        card.set_prop("ItemCount", PropValue::Int(3));
        card.set_prop("LayoutDirection", PropValue::String("Vertical".into()));
        card.set_prop("ItemSpacing", PropValue::Int(10));
        card.parent = Some("Pnl".into());
        let mut inner = ctrl("Lbl", ControlType::Label, 20, 30, 100, 20);
        inner.parent = Some("GB".into());

        let designed = vec![panel, card, inner];
        let controls = expand_repeating_groups(&designed).expect("expand");

        let pnl_idx = controls.iter().position(|c| c.id == "Pnl").unwrap();
        let size = panel_content_size(&controls, pnl_idx, egui::vec2(300.0, 400.0));

        assert_eq!(size.y, 400.0);

        let mut card_5 = designed[1].clone();
        card_5.set_prop("ItemCount", PropValue::Int(5));
        let controls_5 =
            expand_repeating_groups(&vec![designed[0].clone(), card_5, designed[2].clone()])
                .expect("expand");
        let p_idx = controls_5.iter().position(|c| c.id == "Pnl").unwrap();
        let size_5 = panel_content_size(&controls_5, p_idx, egui::vec2(300.0, 400.0));
        assert_eq!(size_5.y, 560.0);
    }

    #[test]
    fn render_form_scroll_smoke() {
        use crate::model::PropValue;
        let mut panel = ctrl("Pnl", ControlType::Panel, 0, 0, 300, 400);
        panel.set_prop("VScroll", PropValue::Bool(true));
        let mut card = ctrl("GB", ControlType::GroupBox, 10, 20, 250, 100);
        card.set_prop("IsRepeatingGroup", PropValue::Bool(true));
        card.set_prop("DataSource", PropValue::String("MyTable".into()));
        card.set_prop("ItemCount", PropValue::Int(3));
        card.set_prop("LayoutDirection", PropValue::String("Vertical".into()));
        card.set_prop("ItemSpacing", PropValue::Int(10));
        card.parent = Some("Pnl".into());
        let mut inner = ctrl("Lbl", ControlType::Label, 20, 30, 100, 20);
        inner.parent = Some("GB".into());

        let controls = vec![panel, card, inner];
        let ctx = egui::Context::default();
        let active = ActiveTabs::new();
        ctx.run_ui(Default::default(), |root_ui| {
            let ctx = root_ui.ctx().clone();
            let ctx = &ctx;
            // Seed scroll offset in temp data
            let sid = egui::Id::new(("autoscr", "Pnl"));
            ctx.data_mut(|d| d.insert_temp(sid, egui::vec2(0.0, 30.0)));

            egui::CentralPanel::default().show_inside(root_ui, |ui| {
                ui.set_min_size(Vec2::new(400.0, 500.0));
                let input = RenderInput {
                    controls: &controls,
                    state: &DesignedState,
                    form_size: Vec2::new(400.0, 500.0),
                    glass: true,
                    mode: RenderMode::Static,
                    active_tabs: &active,
                    backdrop: Default::default(),
                };
                let _ = render_form(ui, &input);
            });
        }).textures_delta.clear();
    }

    #[test]
    fn render_form_scroll_runstate_overwrite_smoke() {
        use crate::model::PropValue;
        let mut panel = ctrl("Pnl", ControlType::Panel, 0, 0, 300, 400);
        panel.set_prop("VScroll", PropValue::Bool(true));
        let mut card = ctrl("GB", ControlType::GroupBox, 10, 20, 250, 100);
        card.set_prop("IsRepeatingGroup", PropValue::Bool(true));
        card.set_prop("DataSource", PropValue::String("MyTable".into()));
        card.set_prop("ItemCount", PropValue::Int(3));
        card.set_prop("LayoutDirection", PropValue::String("Vertical".into()));
        card.set_prop("ItemSpacing", PropValue::Int(10));
        card.parent = Some("Pnl".into());
        let mut inner = ctrl("Lbl", ControlType::Label, 20, 30, 100, 20);
        inner.parent = Some("GB".into());

        let controls = vec![panel, card, inner];

        // Simulate RunState state snap.
        let mut states_snap = Map::new();
        let mut card_state = Map::new();
        card_state.insert("Y".to_string(), "20".to_string());
        states_snap.insert("GB.GB-2".to_string(), card_state);

        let state_cell = std::cell::RefCell::new(states_snap);
        let state = MapState(&state_cell);

        let ctx = egui::Context::default();
        let active = ActiveTabs::new();
        ctx.run_ui(Default::default(), |root_ui| {
            let ctx = root_ui.ctx().clone();
            let ctx = &ctx;
            // Seed scroll offset in temp data
            let sid = egui::Id::new(("autoscr", "Pnl"));
            ctx.data_mut(|d| d.insert_temp(sid, egui::vec2(0.0, 30.0)));

            egui::CentralPanel::default().show_inside(root_ui, |ui| {
                ui.set_min_size(Vec2::new(400.0, 500.0));
                let input = RenderInput {
                    controls: &controls,
                    state: &state,
                    form_size: Vec2::new(400.0, 500.0),
                    glass: true,
                    mode: RenderMode::Static,
                    active_tabs: &active,
                    backdrop: Default::default(),
                };
                let _ = render_form(ui, &input);
            });
        }).textures_delta.clear();
    }

    /// Operator rule (2026-07-30): the form keeps the size its author gave
    /// it, but its gradient / background image follows the WINDOW â over the
    /// whole thing when the user maximizes or drags it bigger, and never
    /// cropped below the form when the window is dragged smaller. Editing
    /// surfaces (no window) keep the backdrop pinned to the form.
    #[test]
    fn backdrop_follows_the_window_but_never_shrinks_below_the_form() {
        let form = Vec2::new(800.0, 600.0);
        // No host window (designer canvas, preview): pinned to the form.
        assert_eq!(backdrop_size(form, None), form);
        // Maximized / dragged bigger: the backdrop fills the window.
        let big = Vec2::new(1920.0, 1080.0);
        assert_eq!(backdrop_size(form, Some(big)), big);
        // Dragged smaller: clamped at the form, so the background is not
        // cropped to the window â the form scrolls inside it.
        assert_eq!(backdrop_size(form, Some(Vec2::new(400.0, 300.0))), form);
        // Mixed axes are handled independently.
        assert_eq!(
            backdrop_size(form, Some(Vec2::new(1600.0, 300.0))),
            Vec2::new(1600.0, 600.0)
        );
        println!(
            "backdrop: form {form:?}, maximized â {:?}, shrunk â {:?}, mixed â {:?}",
            backdrop_size(form, Some(big)),
            backdrop_size(form, Some(Vec2::new(400.0, 300.0))),
            backdrop_size(form, Some(Vec2::new(1600.0, 300.0)))
        );
    }

    fn collect_text(shape: &egui::Shape, out: &mut Vec<(String, Color32)>) {
        match shape {
            egui::Shape::Text(t) => {
                let colour = t
                    .galley
                    .job
                    .sections
                    .first()
                    .map(|sec| sec.format.color)
                    .unwrap_or(t.fallback_color);
                out.push((t.galley.text().to_owned(), colour));
            }
            egui::Shape::Vec(v) => v.iter().for_each(|s| collect_text(s, out)),
            _ => {}
        }
    }

    /// Every text run painted for `controls`, with its colour â used to answer
    /// "did the new caption reach the screen, and can it be read there?".
    fn painted_text(controls: &[Control], backdrop_hex: &str) -> Vec<(String, Color32)> {
        use collect_text as collect;
        let ctx = egui::Context::default();
        let active = ActiveTabs::new();
        let mut full = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(700.0, 560.0),
                )),
                ..Default::default()
            },
            |root_ui| {
                egui::CentralPanel::default().show_inside(root_ui, |ui| {
                    ui.set_min_size(Vec2::new(640.0, 480.0));
                    let input = RenderInput {
                        controls,
                        state: &DesignedState,
                        form_size: Vec2::new(640.0, 480.0),
                        glass: true,
                        mode: RenderMode::Static,
                        active_tabs: &active,
                        backdrop: Backdrop {
                            paint: true,
                            color_hex: backdrop_hex.into(),
                            ..Default::default()
                        },
                    };
                    let _ = render_form(ui, &input);
                });
            },
        );
        let mut out = Vec::new();
        for cs in &full.shapes {
            collect(&cs.shape, &mut out);
        }
        full.textures_delta.clear();
        out
    }

    /// Like [`painted_text`], but Interactive â the mode where the widget-backed
    /// controls (a ListBox's items, â¦) paint anything at all.
    fn painted_text_interactive(controls: &[Control]) -> Vec<(String, Color32)> {
        let ctx = egui::Context::default();
        let active = ActiveTabs::new();
        let mut full = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(700.0, 560.0),
                )),
                ..Default::default()
            },
            |root_ui| {
                egui::CentralPanel::default().show_inside(root_ui, |ui| {
                    ui.set_min_size(Vec2::new(640.0, 480.0));
                    let input = RenderInput {
                        controls,
                        state: &DesignedState,
                        form_size: Vec2::new(640.0, 480.0),
                        glass: true,
                        mode: RenderMode::Interactive,
                        active_tabs: &active,
                        backdrop: Backdrop::default(),
                    };
                    let _ = render_form(ui, &input);
                });
            },
        );
        let mut out = Vec::new();
        for cs in &full.shapes {
            collect_text(&cs.shape, &mut out);
        }
        full.textures_delta.clear();
        out
    }

    /// One painted text run: what it says, the font size it was LAID OUT at
    /// (the caption branches shrink the font to fit, so this reports whether the
    /// frame was big enough), where it starts and how much room it takes.
    #[derive(Clone, Debug)]
    struct PaintedText {
        text: String,
        font: f32,
        pos: egui::Pos2,
        size: Vec2,
        ink: Rect,
    }

    impl PaintedText {
        fn rect(&self) -> Rect {
            self.ink
        }
    }

    /// Every text run painted for `controls`, plus each control's screen rect.
    ///
    /// `chrome` tweaks the HOST's style before rendering, so a surface with the
    /// IDE's roomy touch metrics can be reproduced exactly.
    fn painted_text_layout_interactive(
        controls: &[Control],
        chrome: impl FnMut(&mut egui::Style),
    ) -> (Vec<PaintedText>, Map<String, Rect>) {
        // `clip` is the shape's own clip rect: text painted outside it never
        // reaches the screen, so a harness that ignores it reports "escaped"
        // glyphs that no one can see. Only the VISIBLE part is recorded, and a
        // run clipped away entirely is not recorded at all.
        fn collect(shape: &egui::Shape, clip: Rect, out: &mut Vec<PaintedText>) {
            match shape {
                egui::Shape::Text(t) => {
                    let ink = t.visual_bounding_rect().intersect(clip);
                    if ink.is_positive() {
                        out.push(PaintedText {
                            text: t.galley.text().to_owned(),
                            font: t
                                .galley
                                .job
                                .sections
                                .first()
                                .map(|s| s.format.font_id.size)
                                .unwrap_or(0.0),
                            pos: t.pos,
                            size: t.galley.size(),
                            // Where the glyphs REALLY land: a job with
                            // `halign = Center` treats `pos` as the text's
                            // centre, not its left edge.
                            ink,
                        });
                    }
                }
                egui::Shape::Vec(v) => v.iter().for_each(|s| collect(s, clip, out)),
                _ => {}
            }
        }
        let placed: RefCell<Map<String, Rect>> = RefCell::new(Map::new());
        let ctx = egui::Context::default();
        ctx.all_styles_mut(chrome);
        let active = ActiveTabs::new();
        let mut full = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(700.0, 560.0),
                )),
                ..Default::default()
            },
            |root_ui| {
                egui::CentralPanel::default().show_inside(root_ui, |ui| {
                    ui.set_min_size(Vec2::new(640.0, 480.0));
                    let input = RenderInput {
                        controls,
                        state: &DesignedState,
                        form_size: Vec2::new(640.0, 480.0),
                        glass: true,
                        mode: RenderMode::Interactive,
                        active_tabs: &active,
                        backdrop: Backdrop::default(),
                    };
                    let out = render_form(ui, &input);
                    *placed.borrow_mut() = out.control_rects;
                });
            },
        );
        let mut out = Vec::new();
        for cs in &full.shapes {
            collect(&cs.shape, cs.clip_rect, &mut out);
        }
        full.textures_delta.clear();
        (out, placed.into_inner())
    }

    /// [`painted_text_layout_interactive`] with the host's own default chrome.
    fn painted_captions(controls: &[Control]) -> (Vec<PaintedText>, Map<String, Rect>) {
        painted_text_layout_interactive(controls, |_| {})
    }

    /// A control you drop on the canvas must be big enough for the caption it
    /// arrives with, at the font it arrives with. The caption branches shrink
    /// the font to fit rather than spill over the border, so a frame a couple of
    /// pixels too short quietly renders its default caption at 12 or 10 pt â the
    /// developer sees a control whose text is smaller than the `FontSize` the
    /// properties pane reports.
    #[test]
    fn a_default_control_frame_fits_its_default_caption() {
        let seeded_font = 14.0_f32;
        let mut report: Vec<(String, f32)> = Vec::new();
        let mut too_small: Vec<String> = Vec::new();

        // The controls that arrive carrying text. The ones whose content is
        // empty by default (a TextBox, a ComboBox, a GroupBox) are given some,
        // since the question is whether the FRAME fits a line of the default
        // font â not whether the control happens to start out blank.
        for (t, seed) in [
            (ControlType::Button, None),
            (ControlType::Label, None),
            (ControlType::CheckBox, None),
            (ControlType::RadioButton, None),
            (ControlType::DateTimePicker, None),
            (ControlType::NumericUpDown, None),
            (ControlType::TextBox, Some(("Text", "Sample"))),
            (ControlType::ComboBox, Some(("Items", "Sample"))),
            (ControlType::GroupBox, Some(("Caption", "Sample"))),
        ] {
            let (w, h) = t.default_size();
            let mut c = Control::new(format!("{}-1", t.as_str()), t.clone(), 20, 20);
            c.rect = MRect::new(20, 20, w, h);
            if let Some((key, value)) = seed {
                c.set_prop(key, crate::PropValue::String(value.to_owned()));
            }
            assert_eq!(
                c.get_prop("FontSize").map(|v| v.as_i64()),
                Some(seeded_font as i64),
                "{} seeds a different font size",
                t.as_str()
            );
            let (captions, placed) = painted_captions(&[c]);
            let frame = *placed.get(&format!("{}-1", t.as_str())).expect("placed");
            let Some(caption) = captions
                .iter()
                .find(|p| !p.text.trim().is_empty())
                .cloned()
            else {
                continue; // nothing written on it by default
            };
            report.push((
                format!("{} {w}x{h} {:?}", t.as_str(), caption.text),
                caption.font,
            ));
            if caption.font < seeded_font {
                too_small.push(format!(
                    "{} at {w}x{h}: {:?} had to shrink to {}pt",
                    t.as_str(),
                    caption.text,
                    caption.font
                ));
            }
            // â¦and having kept its size, it must also fit inside the frame
            // rather than run under the border and get clipped. A GroupBox is
            // the one exception by design: its caption sits ON the top border,
            // in the notch, which is what makes it read as a group.
            let sits_on_the_border = matches!(t, ControlType::GroupBox);
            if !sits_on_the_border && !frame.expand(0.5).contains_rect(caption.rect()) {
                too_small.push(format!(
                    "{} at {w}x{h}: {:?} spills out of the frame ({:?} vs {:?})",
                    t.as_str(),
                    caption.text,
                    caption.rect(),
                    frame
                ));
            }
        }

        println!("\n  default frames vs the seeded {seeded_font}pt font");
        for (what, size) in &report {
            println!("    {what:<48} painted at {size}pt");
        }
        println!();
        assert!(
            too_small.is_empty(),
            "these defaults cannot hold their own caption:\n  {}",
            too_small.join("\n  ")
        );
    }

    /// Narrow a DateTimePicker past what its `DD/MM/YYYY` mask needs and the
    /// mask is cut from the RIGHT: you keep the start of the value. Centred â as
    /// every over-long caption used to be â it was cut at BOTH ends and showed
    /// the middle, which reads as a different date rather than a truncated one.
    #[test]
    fn a_narrowed_datetimepicker_loses_its_mask_from_the_right() {
        // Narrow, and holding a value long enough that it cannot fit even at the
        // 6 pt floor the caption branch shrinks to â the case where the clip is
        // what the developer actually sees.
        let mut dtp = ctrl("DateTimePicker-1", ControlType::DateTimePicker, 20, 20, 48, 22);
        dtp.set_prop(
            "Value",
            crate::PropValue::String("DD/MM/YYYYHH:MM:SS".to_owned()),
        );
        let (texts, placed) = painted_captions(&[dtp]);
        let mask = texts
            .iter()
            .find(|p| p.text.contains("YYYYHH"))
            .expect("the value must be painted");
        let field = *placed.get("DateTimePicker-1").expect("placed");

        assert!(
            mask.ink.left() >= field.left() - 1.0,
            "the value must never hang off the LEFT of its frame â what a narrow \
             field loses is the TAIL: ink {:?}, field {}..{}",
            mask.ink,
            field.left(),
            field.right()
        );

        println!(
            "\n  DateTimePicker â {:?} needs {:.0}px in a {:.0}px field: it starts at \
             x={:.0} (field starts at {:.0}), so the clip takes the tail\n",
            mask.text,
            mask.ink.width(),
            field.width(),
            mask.ink.left(),
            field.left()
        );
    }

    /// Every filled rectangle painted for `controls`, with its corner radius â
    /// for questions about a highlight's shape.
    fn painted_bands(controls: &[Control]) -> Vec<(Rect, egui::CornerRadius, Color32)> {
        fn collect(shape: &egui::Shape, out: &mut Vec<(Rect, egui::CornerRadius, Color32)>) {
            match shape {
                egui::Shape::Rect(r) => out.push((r.rect, r.corner_radius, r.fill)),
                egui::Shape::Vec(v) => v.iter().for_each(|s| collect(s, out)),
                _ => {}
            }
        }
        let ctx = egui::Context::default();
        let active = ActiveTabs::new();
        let mut full = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(700.0, 560.0),
                )),
                ..Default::default()
            },
            |root_ui| {
                egui::CentralPanel::default().show_inside(root_ui, |ui| {
                    ui.set_min_size(Vec2::new(640.0, 480.0));
                    let input = RenderInput {
                        controls,
                        state: &DesignedState,
                        form_size: Vec2::new(640.0, 480.0),
                        glass: true,
                        mode: RenderMode::Interactive,
                        active_tabs: &active,
                        backdrop: Backdrop::default(),
                    };
                    let _ = render_form(ui, &input);
                });
            },
        );
        let mut out = Vec::new();
        for cs in &full.shapes {
            collect(&cs.shape, &mut out);
        }
        full.textures_delta.clear();
        out
    }

    /// The highlight spans the list's whole width and is square â except where
    /// it meets the frame's own rounded corner, which must cut it exactly as the
    /// border is cut. egui clips to an axis-aligned rect, so a highlight left to
    /// itself paints straight through the arc and out past the border, which is
    /// what a one- or two-line list shows most: every row IS a corner row.
    #[test]
    fn a_listbox_highlight_fills_the_width_and_is_cut_by_the_corner() {
        let mut lb = ctrl("ListBox-1", ControlType::ListBox, 20, 20, 220, 120);
        lb.set_prop(
            "Items",
            crate::PropValue::String("Alpha\nBeta\nGamma\nDelta".to_owned()),
        );
        // Alpha is the ACTIVE row â the first one, against the top corners.
        lb.set_prop("Value", crate::PropValue::String("Alpha".to_owned()));
        lb.set_prop("CornerRadius", crate::PropValue::Int(8));

        let bands = painted_bands(&[lb.clone()]);
        let (_, placed) = painted_captions(&[lb]);
        let frame = *placed.get("ListBox-1").expect("placed");

        // The highlight: a band as wide as the control, in the top half.
        // Wide â but stopping short of the border, which stays visible.
        let inset = 1.0 + 2.0; // BorderWidth 1 + the hairline
        let band = bands
            .iter()
            .filter(|(r, _, _)| (r.width() - (frame.width() - inset * 2.0)).abs() <= 0.5)
            .filter(|(r, _, _)| r.height() < frame.height() * 0.5 && r.top() < frame.center().y)
            .max_by(|a, b| a.0.height().total_cmp(&b.0.height()))
            .expect("the active row must be highlighted");
        let (rect, corner, _) = band;

        assert!(
            rect.left() > frame.left() + 0.5 && rect.right() < frame.right() - 0.5,
            "the highlight must stop short of the border, not paint over it: \
             {rect:?} in {frame:?}"
        );
        assert!(
            rect.top() > frame.top() + 0.5,
            "â¦including at the top row: {rect:?} in {frame:?}"
        );
        assert!(
            corner.nw > 0 && corner.ne > 0,
            "the corners alongside the frame's arc are rounded, got {corner:?}"
        );
        assert_eq!(
            (corner.sw, corner.se),
            (0, 0),
            "â¦and the inner edge stays square, got {corner:?}"
        );

        println!(
            "\n  ListBox highlight â {}px wide inside a {}px control, starting {}px below the \
             top border; corners nw/ne={}/{} follow the arc, sw/se square\n",
            rect.width(),
            frame.width(),
            rect.top() - frame.top(),
            corner.nw,
            corner.ne
        );
    }

    /// A list with `MultiSelect` builds a set with Ctrl (Cmd on a Mac): each
    /// held click adds a row or takes it out again, and a plain click starts
    /// over with one. The active row â the one the cursor is on â stays a
    /// separate thing from the set, which is why they are separate properties.
    #[test]
    fn ctrl_click_builds_a_listbox_selection() {
        let mut lb = ctrl("ListBox-1", ControlType::ListBox, 20, 20, 220, 140);
        lb.set_prop(
            "Items",
            crate::PropValue::String("Alpha\nBeta\nGamma\nDelta".to_owned()),
        );
        lb.set_prop("MultiSelect", crate::PropValue::Bool(true));

        // Row pitch is the model's own: 14pt line + 2px of air each side.
        let pitch = crate::model::text_line_height(&lb) + crate::model::LIST_ROW_PAD * 2.0;
        let top = 28.0 + crate::model::LIST_FRAME_PAD;
        let row_at = |n: usize| pos2(120.0, top + pitch * (n as f32 + 0.5));

        // A held Ctrl is a modifier state that spans frames â released in the
        // SAME batch, egui reports the end-of-batch state and the click reads
        // as unmodified, exactly as it would if the user let go too early.
        let held = Modifiers {
            command: true,
            ctrl: true,
            ..Modifiers::default()
        };
        let press_at = |p: Pos2| Event::PointerButton {
            pos: p,
            button: PointerButton::Primary,
            pressed: true,
            modifiers: held,
        };
        let release_at = |p: Pos2| Event::PointerButton {
            pos: p,
            button: PointerButton::Primary,
            pressed: false,
            modifiers: held,
        };

        let (_, overrides) = drive(
            &[lb],
            vec![
                // Plain click on Alphaâ¦
                (0.0, vec![Event::PointerMoved(row_at(0))]),
                (0.05, vec![press(row_at(0))]),
                (0.10, vec![release(row_at(0))]),
                // â¦then Ctrl goes down and stays down for three clicks.
                (0.15, vec![Event::ModifiersChanged(held)]),
                (0.20, vec![Event::PointerMoved(row_at(2)), press_at(row_at(2))]),
                (0.25, vec![release_at(row_at(2))]),
                (0.30, vec![Event::PointerMoved(row_at(3)), press_at(row_at(3))]),
                (0.35, vec![release_at(row_at(3))]),
                // Ctrl-clicking Gamma again takes it back out.
                (0.40, vec![Event::PointerMoved(row_at(2)), press_at(row_at(2))]),
                (0.45, vec![release_at(row_at(2))]),
                (0.50, vec![Event::ModifiersChanged(Modifiers::default())]),
            ],
        );
        let props = overrides.get("ListBox-1").expect("the list wrote something");
        assert_eq!(
            props.get("SelectedItems").map(String::as_str),
            Some("Alpha\nDelta"),
            "Ctrl-click adds and removes; the plain click started the set"
        );
        assert_eq!(
            props.get("Value").map(String::as_str),
            Some("Gamma"),
            "the ACTIVE row is the last one clicked â the cursor lands on a row \
             whether the Ctrl-click added it or took it out"
        );

        println!(
            "\n  ListBox multi-select â click Alpha, Ctrl-click Gamma, Delta, then Gamma \
             again â SelectedItems \"Alpha, Delta\"; the cursor is on Gamma, which is the \
             row it last touched\n"
        );
    }

    /// With `ShowCheckBoxes` every row carries a tick box, and the boxes ARE the
    /// multiple selection: a plain click anywhere on a row ticks it, with no
    /// modifier to hold, and clicking again clears it. The set keeps whatever
    /// order and gaps the user made.
    #[test]
    fn a_listbox_tick_box_collects_its_own_set() {
        let mut lb = ctrl("ListBox-1", ControlType::ListBox, 20, 20, 220, 140);
        lb.set_prop(
            "Items",
            crate::PropValue::String("Alpha\nBeta\nGamma\nDelta".to_owned()),
        );
        lb.set_prop("ShowCheckBoxes", crate::PropValue::Bool(true));
        lb.set_prop("Value", crate::PropValue::String("Beta".to_owned()));

        let pitch = crate::model::text_line_height(&lb) + crate::model::LIST_ROW_PAD * 2.0;
        let top = 28.0 + crate::model::LIST_FRAME_PAD;
        // The tick box sits at the left of the row, inside the frame padding.
        let box_at = |n: usize| pos2(28.0 + 8.0, top + pitch * (n as f32 + 0.5));

        let (events, overrides) = drive(
            &[lb],
            vec![
                (0.0, vec![Event::PointerMoved(box_at(3))]),
                (0.05, vec![press(box_at(3))]),
                (0.10, vec![release(box_at(3))]),
                (0.15, vec![Event::PointerMoved(box_at(1))]),
                (0.20, vec![press(box_at(1))]),
                (0.25, vec![release(box_at(1))]),
                (0.30, vec![]),
            ],
        );
        let props = overrides.get("ListBox-1").expect("the list wrote something");
        assert_eq!(
            props.get("CheckedItems").map(String::as_str),
            Some("Delta\nBeta"),
            "the checked set keeps the order the user built it in"
        );
        assert_eq!(
            props.get("Value").map(String::as_str),
            Some("Beta"),
            "the cursor follows the click, as it does anywhere else in the list"
        );
        assert_eq!(
            props.get("SelectedItems").map(String::as_str),
            None,
            "â¦but the Ctrl-click selection is untouched: with tick boxes on, the \
             ticks are the set"
        );
        assert!(
            events.iter().any(|e| e.event == "onItemChecked"),
            "and it reports itself: {events:?}"
        );

        println!(
            "\n  ListBox tick boxes â a plain click on Delta then Beta â CheckedItems \
             \"Delta, Beta\", with no modifier held\n"
        );
    }

    /// One radio at a time. A radio turned itself on when clicked and nothing
    /// ever turned the others off, so a group could show two, three, every
    /// button selected at once â and the form had no way to say which the
    /// operator meant.
    #[test]
    fn only_the_last_clicked_radio_in_a_group_stays_on() {
        let radio = |id: &str, y: i32, group: &str, on: bool| {
            let mut c = ctrl(id, ControlType::RadioButton, 20, y, 160, 24);
            c.set_prop("GroupName", crate::PropValue::String(group.to_owned()));
            if on {
                c.set_prop("Checked", crate::PropValue::Bool(true));
            }
            c
        };
        // Two groups: PAGO starts with CASH lit, ENVIO is a separate set that
        // must not move when PAGO does.
        let controls = [
            radio("CASH", 20, "PAGO", true),
            radio("CARD", 50, "PAGO", false),
            radio("WIRE", 80, "PAGO", false),
            radio("POST", 110, "ENVIO", true),
        ];
        let at = |y: i32| pos2(100.0, 8.0 + y as f32 + 12.0);

        let (events, overrides) = drive(
            &controls,
            vec![
                (0.0, vec![Event::PointerMoved(at(50))]),
                (0.05, vec![press(at(50))]),
                (0.10, vec![release(at(50))]),
                (0.15, vec![]),
                // â¦then a third, so the one just lit goes out too.
                (0.20, vec![Event::PointerMoved(at(80))]),
                (0.25, vec![press(at(80))]),
                (0.30, vec![release(at(80))]),
                (0.35, vec![]),
            ],
        );
        let value = |id: &str| {
            overrides
                .get(id)
                .and_then(|p| p.get("Value"))
                .map(String::as_str)
        };

        assert_eq!(value("WIRE"), Some("1"), "the last one clicked is on");
        assert_eq!(value("CARD"), Some("0"), "the one before it went out");
        assert_eq!(value("CASH"), Some("0"), "and so did the designed default");
        assert_eq!(
            value("POST"),
            None,
            "a radio in ANOTHER group is not touched"
        );
        // The button that was really lit reports going out; the ones already
        // out say nothing.
        let unchecks: Vec<&str> = events
            .iter()
            .filter(|e| e.event == "onUncheck")
            .map(|e| e.ctrl_id.as_str())
            .collect();
        assert_eq!(unchecks, vec!["CASH", "CARD"], "got {unchecks:?}");

        println!(
            "\n  radio groups â clicking CARD then WIRE leaves WIRE on, CASH and CARD off, \
             and the ENVIO group untouched; onUncheck fired once per button that was lit\n"
        );
    }

    /// Press and sweep: every row the pointer crosses with the button down is
    /// taken, each once. With `MultiSelect` they join one selection; with tick
    /// boxes they are ticked. Releasing ends the gesture, so the next press
    /// starts a new one rather than continuing the old.
    #[test]
    fn a_sweep_takes_every_row_it_crosses() {
        let rows_at = |lb: &Control, n: usize| {
            let pitch = crate::model::text_line_height(lb) + crate::model::LIST_ROW_PAD * 2.0;
            pos2(120.0, 28.0 + crate::model::LIST_FRAME_PAD + pitch * (n as f32 + 0.5))
        };

        // Multi-select: a sweep down four rows selects all four.
        let mut lb = ctrl("ListBox-1", ControlType::ListBox, 20, 20, 220, 160);
        lb.set_prop(
            "Items",
            crate::PropValue::String("Alpha\nBeta\nGamma\nDelta".to_owned()),
        );
        lb.set_prop("MultiSelect", crate::PropValue::Bool(true));
        let (_, overrides) = drive(
            &[lb.clone()],
            vec![
                (0.0, vec![Event::PointerMoved(rows_at(&lb, 0))]),
                (0.05, vec![press(rows_at(&lb, 0))]),
                (0.10, vec![Event::PointerMoved(rows_at(&lb, 1))]),
                (0.15, vec![Event::PointerMoved(rows_at(&lb, 2))]),
                (0.20, vec![Event::PointerMoved(rows_at(&lb, 3))]),
                (0.25, vec![release(rows_at(&lb, 3))]),
                (0.30, vec![]),
            ],
        );
        assert_eq!(
            overrides
                .get("ListBox-1")
                .and_then(|p| p.get("SelectedItems"))
                .map(String::as_str),
            Some("Alpha\nBeta\nGamma\nDelta"),
            "a sweep takes every row it crossed"
        );

        // Tick boxes: the same sweep ticks them.
        let mut ticked = lb.clone();
        ticked.set_prop("MultiSelect", crate::PropValue::Bool(false));
        ticked.set_prop("ShowCheckBoxes", crate::PropValue::Bool(true));
        let (_, overrides) = drive(
            &[ticked.clone()],
            vec![
                (0.0, vec![Event::PointerMoved(rows_at(&ticked, 1))]),
                (0.05, vec![press(rows_at(&ticked, 1))]),
                (0.10, vec![Event::PointerMoved(rows_at(&ticked, 2))]),
                (0.15, vec![release(rows_at(&ticked, 2))]),
                (0.20, vec![]),
            ],
        );
        assert_eq!(
            overrides
                .get("ListBox-1")
                .and_then(|p| p.get("CheckedItems"))
                .map(String::as_str),
            Some("Beta\nGamma"),
            "a sweep ticks every row it crossed"
        );

        println!(
            "\n  ListBox sweep â pressing on row 1 and dragging to row 4 selects all four; \
             with tick boxes, the same sweep ticks them\n"
        );
    }

    /// Reversing a drag SHRINKS the selection â it does not freeze the list
    /// (operator, 2026-08-17).
    ///
    /// The sweep used to accumulate every row a press had touched and never let
    /// go of any: dragging back up crossed only rows already in the set, so the
    /// list answered nothing in either direction until the button came up.
    #[test]
    fn reversing_a_drag_shrinks_the_selection_instead_of_freezing_it() {
        let pitch = |lb: &Control| crate::model::text_line_height(lb) + crate::model::LIST_ROW_PAD * 2.0;
        let row_at = |lb: &Control, n: usize| {
            pos2(120.0, 28.0 + crate::model::LIST_FRAME_PAD + pitch(lb) * (n as f32 + 0.5))
        };

        let mut lb = ctrl("ListBox-1", ControlType::ListBox, 20, 20, 220, 240);
        lb.set_prop(
            "Items",
            crate::PropValue::String("Alpha\nBeta\nGamma\nDelta\nEpsilon".to_owned()),
        );
        lb.set_prop("MultiSelect", crate::PropValue::Bool(true));

        // Down to row 3, then back up to row 1, all in one press.
        let (_events, overrides) = drive(
            &[lb.clone()],
            vec![
                (0.00, vec![Event::PointerMoved(row_at(&lb, 0))]),
                (0.05, vec![press(row_at(&lb, 0))]),
                (0.10, vec![Event::PointerMoved(row_at(&lb, 2))]),
                (0.15, vec![Event::PointerMoved(row_at(&lb, 3))]),
                (0.20, vec![Event::PointerMoved(row_at(&lb, 2))]),
                (0.25, vec![Event::PointerMoved(row_at(&lb, 1))]),
                (0.30, vec![release(row_at(&lb, 1))]),
                (0.35, vec![]),
            ],
        );
        let prop = |k: &str| {
            overrides
                .get("ListBox-1")
                .and_then(|p| p.get(k))
                .map(String::as_str)
        };
        assert_eq!(
            prop("SelectedItems"),
            Some("Alpha\nBeta"),
            "the range follows the pointer back up: anchor row 0 to row 1"
        );
        assert_eq!(prop("Value"), Some("Beta"), "the active row is the one under the pointer");
        assert_eq!(prop("SelectedIndex"), Some("1"));

        // Dragging past the ENDS holds at the ends rather than selecting
        // nothing: far above the control, then far below it.
        let (_events, overrides) = drive(
            &[lb.clone()],
            vec![
                (0.00, vec![Event::PointerMoved(row_at(&lb, 2))]),
                (0.05, vec![press(row_at(&lb, 2))]),
                (0.10, vec![Event::PointerMoved(pos2(120.0, -400.0))]),
                (0.15, vec![release(pos2(120.0, -400.0))]),
                (0.20, vec![]),
            ],
        );
        assert_eq!(
            overrides
                .get("ListBox-1")
                .and_then(|p| p.get("Value"))
                .map(String::as_str),
            Some("Alpha"),
            "dragging above the list stops at the FIRST element"
        );

        println!(
            "\n  ListBox drag â press on row 0, down to row 3 and back to row 1 leaves \
             Alpha+Beta selected with Beta active (the reversal is answered, not frozen); \
             dragging far above the control holds at the first element\n"
        );
    }

    /// [`drive`], but it also hands back what the LAST frame painted and where
    /// the controls landed â for the questions that are about what the operator
    /// can see, not about what the form was told.
    /// Everything the last frame put on the screen, for the questions that are
    /// about what the operator can SEE.
    struct Painted {
        overrides: Map<String, Map<String, String>>,
        texts: Vec<PaintedText>,
        /// Every filled rectangle, with the colour it was filled with.
        fills: Vec<(Rect, Color32)>,
        /// Every filled rectangle with its corner radii too â what the corner
        /// guards need, since a band's silhouette is its rect AND its arc.
        bands: Vec<(Rect, egui::CornerRadius, Color32)>,
        /// The bounds of every mesh â a gradient is one.
        meshes: Vec<Rect>,
        /// Every mesh with the colours its vertices carry. A glass lens is a
        /// mesh too, so "is the DESIGNED gradient painted" is about the colours
        /// in one, not about a mesh being there at all.
        mesh_colors: Vec<(Rect, Vec<Color32>)>,
        placed: Map<String, Rect>,
    }

    fn drive_painted(controls: &[Control], frames: Vec<(f64, Vec<Event>)>) -> Painted {
        fn collect_faces(
            shape: &egui::Shape,
            fills: &mut Vec<(Rect, Color32)>,
            bands: &mut Vec<(Rect, egui::CornerRadius, Color32)>,
            meshes: &mut Vec<Rect>,
            mesh_colors: &mut Vec<(Rect, Vec<Color32>)>,
        ) {
            match shape {
                // `fills` is what was actually INKED; `bands` is every rect the
                // frame emitted, transparent ones included — egui fades an idle
                // scrollbar to nothing, and "where does the bar sit" is a
                // question about geometry, not about ink.
                egui::Shape::Rect(r) => {
                    bands.push((r.rect, r.corner_radius, r.fill));
                    if r.fill.a() > 0 {
                        fills.push((r.rect, r.fill));
                    }
                }
                egui::Shape::Mesh(m) => {
                    meshes.push(m.calc_bounds());
                    mesh_colors.push((
                        m.calc_bounds(),
                        m.vertices.iter().map(|v| v.color).collect(),
                    ));
                }
                egui::Shape::Vec(v) => {
                    v.iter().for_each(|s| collect_faces(s, fills, bands, meshes, mesh_colors))
                }
                _ => {}
            }
        }
        fn collect(shape: &egui::Shape, clip: Rect, out: &mut Vec<PaintedText>) {
            match shape {
                egui::Shape::Text(t) => {
                    let ink = t.visual_bounding_rect().intersect(clip);
                    if ink.is_positive() {
                        out.push(PaintedText {
                            text: t.galley.text().to_owned(),
                            font: t
                                .galley
                                .job
                                .sections
                                .first()
                                .map(|s| s.format.font_id.size)
                                .unwrap_or(0.0),
                            pos: t.pos,
                            size: t.galley.size(),
                            ink,
                        });
                    }
                }
                egui::Shape::Vec(v) => v.iter().for_each(|s| collect(s, clip, out)),
                _ => {}
            }
        }

        let ctx = egui::Context::default();
        ctx.set_fonts(egui::FontDefinitions::default());
        let active = ActiveTabs::new();
        let overrides: RefCell<Map<String, Map<String, String>>> = RefCell::new(Map::new());
        let placed: RefCell<Map<String, Rect>> = RefCell::new(Map::new());
        let mut painted: Vec<PaintedText> = Vec::new();
        let mut fills: Vec<(Rect, Color32)> = Vec::new();
        let mut bands: Vec<(Rect, egui::CornerRadius, Color32)> = Vec::new();
        let mut meshes: Vec<Rect> = Vec::new();
        let mut mesh_colors: Vec<(Rect, Vec<Color32>)> = Vec::new();

        for (i, (_time, evs)) in frames.into_iter().enumerate() {
            let mut input = egui::RawInput::default();
            input.screen_rect = Some(Rect::from_min_size(
                pos2(0.0, 0.0),
                Vec2::new(1000.0, 800.0),
            ));
            input.focused = true;
            input.time = Some(i as f64 * 0.05);
            input.events = evs;
            let updates = RefCell::new(Vec::<(String, String, String)>::new());
            let events = RefCell::new(Vec::<UiEvent>::new());
            let st = MapState(&overrides);
            let mut full = ctx.run_ui(input, |root_ui| {
                let ctx = root_ui.ctx().clone();
                let ctx = &ctx;
                egui::CentralPanel::default()
                    .frame(egui::Frame::NONE)
                    .show_inside(root_ui, |ui| {
                        ui.set_min_size(Vec2::new(400.0, 300.0));
                        let inp = RenderInput {
                            controls,
                            state: &st,
                            form_size: Vec2::new(400.0, 300.0),
                            glass: true,
                            mode: RenderMode::Interactive,
                            active_tabs: &active,
                            backdrop: Backdrop::default(),
                        };
                        let out = render_form(ui, &inp);
                        *placed.borrow_mut() = out.control_rects.clone();
                        updates.borrow_mut().extend(out.prop_updates);
                        events.borrow_mut().extend(out.events);
                        let _ = ctx;
                    });
            });
            painted.clear();
            fills.clear();
            bands.clear();
            meshes.clear();
            mesh_colors.clear();
            for cs in &full.shapes {
                collect(&cs.shape, cs.clip_rect, &mut painted);
                collect_faces(&cs.shape, &mut fills, &mut bands, &mut meshes, &mut mesh_colors);
            }
            full.textures_delta.clear();
            for (id, key, value) in updates.into_inner() {
                overrides
                    .borrow_mut()
                    .entry(id)
                    .or_default()
                    .insert(key, value);
            }
        }
        Painted {
            overrides: overrides.into_inner(),
            texts: painted,
            fills,
            bands,
            meshes,
            mesh_colors,
            placed: placed.into_inner(),
        }
    }

    /// A ListBox wears the background the RAD gave it â a colour or a gradient
    /// (operator, 2026-08-18).
    ///
    /// The running list painted a hardcoded navy surface over its own face, so
    /// a list designed with a grey-to-black gradient came out blue the moment
    /// the form ran, and nothing in the properties pane could change it. Its
    /// face is now drawn by the same call the designer canvas uses.
    #[test]
    fn a_listbox_wears_the_background_designed_in_the_rad() {
        const HARDCODED_NAVY: Color32 = Color32::from_rgb(30, 40, 80);
        let mut lb = ctrl("ListBox-1", ControlType::ListBox, 20, 20, 220, 140);
        lb.set_prop(
            "Items",
            crate::PropValue::String("Alpha\nBeta\nGamma".to_owned()),
        );

        // A designed COLOUR: a deep red, which no default in the engine is.
        let mut solid = lb.clone();
        solid.set_prop("BackgroundColor", crate::PropValue::String("#B00000FF".into()));
        let painted = drive_painted(&[solid], vec![(0.0, vec![]), (0.05, vec![])]);
        let frame = *painted.placed.get("ListBox-1").expect("placed");
        let covers = |r: &Rect| r.intersect(frame).area() >= frame.area() * 0.7;
        let face: Vec<Color32> = painted
            .fills
            .iter()
            .filter(|(r, _)| covers(r))
            .map(|(_, c)| *c)
            .collect();
        assert!(
            !face.is_empty(),
            "the list must paint a face over its own rect: {:?}",
            painted.fills
        );
        assert!(
            face.iter().any(|c| c.r() > c.g() + 30 && c.r() > c.b() + 30),
            "the designed red must reach the face (glass may tint it, not repaint it): {face:?}"
        );
        assert!(
            !face.contains(&HARDCODED_NAVY),
            "the hardcoded navy must be gone: {face:?}"
        );

        // A designed GRADIENT â the operator's own case, grey to black going
        // south. A gradient is a mesh; the hardcoded surface never made one.
        let mut grad = lb.clone();
        grad.set_prop("BackgroundGradientEnabled", crate::PropValue::Bool(true));
        grad.set_prop(
            "BackgroundGradientStartColor",
            crate::PropValue::String("#4E4E4EFF".into()),
        );
        grad.set_prop(
            "BackgroundGradientEndColor",
            crate::PropValue::String("#000000FF".into()),
        );
        grad.set_prop(
            "BackgroundGradientDirection",
            crate::PropValue::String("South".into()),
        );
        let painted = drive_painted(&[grad], vec![(0.0, vec![]), (0.05, vec![])]);
        let frame = *painted.placed.get("ListBox-1").expect("placed");
        assert!(
            painted
                .meshes
                .iter()
                .any(|m| m.intersect(frame).area() >= frame.area() * 0.7),
            "the designed gradient must be painted across the list: meshes {:?} vs {frame:?}",
            painted.meshes
        );
        assert!(
            !painted
                .fills
                .iter()
                .any(|(r, c)| *c == HARDCODED_NAVY && r.intersect(frame).is_positive()),
            "â¦and nothing paints over it"
        );

        println!(
            "\n  ListBox background â a designed #B00000 reaches the face, a designed \
             grey-to-black South gradient is painted as a mesh across the control, and the \
             hardcoded navy that used to cover both is gone\n"
        );
    }

    /// The two highlights a list draws are the developer's to name: the ACTIVE
    /// row's (`ActiveItemColor`) and the one the rest of a multi-select set
    /// wears (`SelectedItemsColor`) â operator, 2026-08-18.
    ///
    /// Left unnamed they are what they always were, so an old form is
    /// untouched: the palette's own selection colour, and that colour half lit.
    /// The palette is only the FALLBACK because it is not the same everywhere â
    /// the IDE's preview carries the IDE theme's selection colour and a
    /// compiled binary carries egui's â so a list that names its highlight is
    /// the one thing that looks identical on all three surfaces.
    #[test]
    fn a_listbox_draws_the_selection_colours_it_was_given() {
        let mut lb = ctrl("ListBox-1", ControlType::ListBox, 20, 20, 220, 140);
        lb.set_prop(
            "Items",
            crate::PropValue::String("Alpha\nBeta\nGamma".to_owned()),
        );
        lb.set_prop("MultiSelect", crate::PropValue::Bool(true));
        // Beta is the ACTIVE row; Alpha is in the selection with it, so both
        // kinds of band are on screen at once and can be told apart by which
        // row they sit on.
        lb.set_prop("Value", crate::PropValue::String("Beta".to_owned()));
        lb.set_prop(
            "SelectedItems",
            crate::PropValue::String("Alpha\nBeta".to_owned()),
        );

        // The two full-width bands, top-down: row 0 (Alpha, dimmed) then row 1
        // (Beta, active). Width is what separates a highlight from the face,
        // the border and the tick boxes, exactly as the geometry test does it.
        let two_bands = |c: &Control| -> (Color32, Color32) {
            let bands = painted_bands(std::slice::from_ref(c));
            let (_, placed) = painted_captions(std::slice::from_ref(c));
            let frame = *placed.get("ListBox-1").expect("placed");
            let inset = 1.0 + 2.0; // BorderWidth 1 + the hairline
            let mut rows: Vec<(Rect, Color32)> = bands
                .iter()
                .filter(|(r, _, _)| (r.width() - (frame.width() - inset * 2.0)).abs() <= 0.5)
                .filter(|(r, _, _)| r.height() < frame.height() * 0.5)
                // A highlight is something the operator can SEE. The scroll
                // area lays a fully transparent rect of the same width over
                // the whole list, which is not one.
                .filter(|(_, _, c)| c.a() > 0)
                .map(|(r, _, c)| (*r, *c))
                .collect();
            rows.sort_by(|a, b| a.0.top().total_cmp(&b.0.top()));
            assert_eq!(
                rows.len(),
                2,
                "a selected row and an active row must both be highlighted, got {rows:?}"
            );
            (rows[1].1, rows[0].1) // (active = Beta, selected = Alpha)
        };

        // ââ Named neither: the palette's colour, and that colour half lit ââ
        let (active, selected) = two_bands(&lb);
        assert_eq!(
            selected,
            active.gamma_multiply(crate::paint::LIST_SELECTED_DIM),
            "unnamed, the selection keeps the historical relationship to the active row"
        );

        // ââ Named both ââââââââââââââââââââââââââââââââââââââââââââââââââââ
        let mut named = lb.clone();
        named.set_prop(
            "ActiveItemColor",
            crate::PropValue::String("#FF8800".into()),
        );
        named.set_prop(
            "SelectedItemsColor",
            crate::PropValue::String("#116622".into()),
        );
        let (active_n, selected_n) = two_bands(&named);
        assert_eq!(
            active_n,
            Color32::from_rgb(0xFF, 0x88, 0x00),
            "the named active colour must reach the active row"
        );
        assert_eq!(
            selected_n,
            Color32::from_rgb(0x11, 0x66, 0x22),
            "â¦and the named selection colour the rest of the set"
        );

        // ââ Named the active one only: the dim follows it âââââââââââââââââ
        let mut active_only = lb.clone();
        active_only.set_prop(
            "ActiveItemColor",
            crate::PropValue::String("#FF8800".into()),
        );
        let (active_o, selected_o) = two_bands(&active_only);
        assert_eq!(active_o, Color32::from_rgb(0xFF, 0x88, 0x00));
        assert_eq!(
            selected_o,
            active_o.gamma_multiply(crate::paint::LIST_SELECTED_DIM),
            "naming only the active colour must restyle the whole list, not half of it"
        );
        assert_ne!(
            selected_o, selected,
            "the dimmed band must follow the NAMED active colour, not the palette's"
        );

        println!(
            "\n  ListBox selection colours â unnamed: active {active:?} with the set at \
             {selected:?} ({}% lit); named: active #FF8800 and set #116622 both reach the \
             band; active-only: the set follows it to {selected_o:?}\n",
            (crate::paint::LIST_SELECTED_DIM * 100.0) as i32
        );
    }

    /// Whatever moves the active row â a drag or an arrow â the row it lands on
    /// must be ON SCREEN when it gets there (operator, 2026-08-17).
    ///
    /// A list taller than its frame used to leave the operator selecting rows
    /// they could not see: the selection walked past the bottom of the control
    /// and the view stayed where it was.
    #[test]
    fn the_row_a_drag_or_an_arrow_reaches_is_scrolled_into_view() {
        let mut lb = ctrl("ListBox-1", ControlType::ListBox, 20, 20, 220, 120);
        let items: Vec<String> = (1..=30).map(|n| format!("Item-{n:02}")).collect();
        lb.set_prop("Items", crate::PropValue::String(items.join("\n")));
        let pitch = crate::model::text_line_height(&lb) + crate::model::LIST_ROW_PAD * 2.0;
        let row_at = |n: usize| {
            pos2(120.0, 28.0 + crate::model::LIST_FRAME_PAD + pitch * (n as f32 + 0.5))
        };
        let visible_rows = (120.0 / pitch).floor() as usize;
        assert!(
            visible_rows < items.len(),
            "the fixture must overflow its frame: {visible_rows} of {} rows fit",
            items.len()
        );

        // Drag from the first row to far below the control, then let the view
        // settle. Two settle frames: `scroll_to_rect` asks THIS frame and the
        // scroll area answers on the next.
        let far_below = pos2(120.0, 900.0);
        let Painted {
            overrides,
            texts: painted,
            placed,
            ..
        } = drive_painted(
            &[lb.clone()],
            vec![
                (0.00, vec![Event::PointerMoved(row_at(0))]),
                (0.05, vec![press(row_at(0))]),
                (0.10, vec![Event::PointerMoved(far_below)]),
                (0.15, vec![Event::PointerMoved(far_below)]),
                (0.20, vec![Event::PointerMoved(far_below)]),
                (0.25, vec![release(far_below)]),
                (0.30, vec![]),
                (0.35, vec![]),
            ],
        );
        let value = overrides
            .get("ListBox-1")
            .and_then(|p| p.get("Value"))
            .cloned()
            .unwrap_or_default();
        assert_eq!(value, "Item-30", "a drag past the end stops at the LAST element");
        let frame = *placed.get("ListBox-1").expect("placed");
        let on_screen = |text: &str| {
            painted
                .iter()
                .any(|p| p.text == text && frame.expand(1.0).contains_rect(p.ink))
        };
        assert!(
            on_screen(&value),
            "the row the drag reached must be visible inside {frame:?}: painted {:?}",
            painted.iter().map(|p| p.text.as_str()).collect::<Vec<_>>()
        );

        // The same for the keyboard: ten rows down from the first is well past
        // the bottom of a five-row frame.
        let key = |k: egui::Key| Event::Key {
            key: k,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: Modifiers::default(),
        };
        let mut frames = vec![
            (0.00, vec![Event::PointerMoved(row_at(0))]),
            (0.05, vec![press(row_at(0))]),
            (0.10, vec![release(row_at(0))]),
        ];
        for i in 0..10 {
            frames.push((0.15 + i as f64 * 0.05, vec![key(egui::Key::ArrowDown)]));
        }
        frames.push((0.70, vec![]));
        frames.push((0.75, vec![]));
        let Painted {
            overrides,
            texts: painted,
            placed,
            ..
        } = drive_painted(&[lb.clone()], frames);
        let value = overrides
            .get("ListBox-1")
            .and_then(|p| p.get("Value"))
            .cloned()
            .unwrap_or_default();
        assert_eq!(value, "Item-11", "ten rows down from the first");
        let frame = *placed.get("ListBox-1").expect("placed");
        assert!(
            painted
                .iter()
                .any(|p| p.text == value && frame.expand(1.0).contains_rect(p.ink)),
            "the row the arrows reached must be visible inside {frame:?}: painted {:?}",
            painted.iter().map(|p| p.text.as_str()).collect::<Vec<_>>()
        );

        println!(
            "\n  ListBox visibility â a {}-row list in a frame that holds {visible_rows}: a \
             drag past the bottom stops on Item-30 with Item-30 on screen, and ten ArrowDowns \
             land on Item-11 with Item-11 on screen\n",
            items.len()
        );
    }

    /// Up and down arrows move the active row, and stop at the ends (operator,
    /// 2026-08-17). A list you can click but not walk is half a control.
    #[test]
    fn the_arrow_keys_walk_the_list_and_stop_at_both_ends() {
        let pitch = |lb: &Control| crate::model::text_line_height(lb) + crate::model::LIST_ROW_PAD * 2.0;
        let row_at = |lb: &Control, n: usize| {
            pos2(120.0, 28.0 + crate::model::LIST_FRAME_PAD + pitch(lb) * (n as f32 + 0.5))
        };
        let key = |k: egui::Key| Event::Key {
            key: k,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: Modifiers::default(),
        };

        let mut lb = ctrl("ListBox-1", ControlType::ListBox, 20, 20, 220, 240);
        lb.set_prop(
            "Items",
            crate::PropValue::String("Alpha\nBeta\nGamma\nDelta".to_owned()),
        );

        // Click row 0 to focus the list, then walk down twice, up once, and
        // press up twice more at the top.
        let (events, overrides) = drive(
            &[lb.clone()],
            vec![
                (0.00, vec![Event::PointerMoved(row_at(&lb, 0))]),
                (0.05, vec![press(row_at(&lb, 0))]),
                (0.10, vec![release(row_at(&lb, 0))]),
                (0.15, vec![key(egui::Key::ArrowDown)]),
                (0.20, vec![key(egui::Key::ArrowDown)]),
                (0.25, vec![]),
            ],
        );
        let prop = |o: &Map<String, Map<String, String>>, k: &str| {
            o.get("ListBox-1").and_then(|p| p.get(k)).cloned()
        };
        assert_eq!(prop(&overrides, "Value").as_deref(), Some("Gamma"), "two rows down");
        assert_eq!(prop(&overrides, "SelectedIndex").as_deref(), Some("2"));
        assert_eq!(
            prop(&overrides, "SelectedItems").as_deref(),
            Some("Gamma"),
            "walking the list carries the selection with it"
        );
        assert!(
            events.iter().any(|e| e.event == "onSelectedIndexChanged"),
            "a keyboard move reports itself like a click does: {:?}",
            names(&events)
        );

        // Up from the top holds at the first row rather than wrapping.
        let (_events, overrides) = drive(
            &[lb.clone()],
            vec![
                (0.00, vec![Event::PointerMoved(row_at(&lb, 1))]),
                (0.05, vec![press(row_at(&lb, 1))]),
                (0.10, vec![release(row_at(&lb, 1))]),
                (0.15, vec![key(egui::Key::ArrowUp)]),
                (0.20, vec![key(egui::Key::ArrowUp)]),
                (0.25, vec![key(egui::Key::ArrowUp)]),
                (0.30, vec![]),
            ],
        );
        assert_eq!(
            prop(&overrides, "Value").as_deref(),
            Some("Alpha"),
            "three ups from row 1 stop at the first element"
        );

        // Down from the last row holds there too.
        let (_events, overrides) = drive(
            &[lb.clone()],
            vec![
                (0.00, vec![Event::PointerMoved(row_at(&lb, 3))]),
                (0.05, vec![press(row_at(&lb, 3))]),
                (0.10, vec![release(row_at(&lb, 3))]),
                (0.15, vec![key(egui::Key::ArrowDown)]),
                (0.20, vec![key(egui::Key::ArrowDown)]),
                (0.25, vec![]),
            ],
        );
        assert_eq!(
            prop(&overrides, "Value").as_deref(),
            Some("Delta"),
            "and down from the last element stays on it"
        );

        println!(
            "\n  ListBox keys â click Alpha then ââ â Gamma (SelectedIndex 2, \
             onSelectedIndexChanged fired); âââ from Beta stops at Alpha; ââ from Delta \
             stays on Delta\n"
        );
    }

    /// A short ListBox still clips: with room for one or two lines, the items
    /// that do not fit must be cut at the control's edge, not painted over
    /// whatever sits below it.
    #[test]
    fn a_short_listbox_clips_its_items_to_its_own_frame() {
        for height in [26, 40, 60] {
            let mut lb = ctrl("ListBox-1", ControlType::ListBox, 20, 20, 220, height);
            lb.set_prop(
                "Items",
                crate::PropValue::String(
                    (1..=8).map(|n| format!("Item-{n}")).collect::<Vec<_>>().join("\n"),
                ),
            );
            let (texts, placed) = painted_captions(&[lb]);
            let frame = *placed.get("ListBox-1").expect("placed");
            let escaped: Vec<&PaintedText> = texts
                .iter()
                .filter(|p| p.text.starts_with("Item-"))
                .filter(|p| !frame.expand(1.0).contains_rect(p.ink))
                .collect();
            assert!(
                escaped.is_empty(),
                "at {height}px the list painted outside its frame {frame:?}: {escaped:?}"
            );
        }
        println!(
            "\n  ListBox clipping â at 26px, 40px and 60px tall, every painted item stays \
             inside the control\n"
        );
    }

    /// A ListBox's lines are list lines: text plus a little air. Each item is an
    /// egui widget, so left alone it took the HOST's chrome metrics â in the IDE
    /// a 30 px minimum touch height and an 8 px gap â and a list of one-word
    /// items was spaced out like a menu.
    #[test]
    fn listbox_lines_keep_a_natural_pitch_under_roomy_host_chrome() {
        let mut lb = ctrl("ListBox-1", ControlType::ListBox, 20, 20, 220, 200);
        lb.set_prop(
            "Items",
            crate::PropValue::String("Alpha\nBeta\nGamma\nDelta".to_owned()),
        );

        // The IDE's own spacing, which is what made the list airy.
        let (texts, _) = painted_text_layout_interactive(&[lb], |style| {
            style.spacing.interact_size.y = 30.0;
            style.spacing.item_spacing.y = 8.0;
            style.spacing.button_padding.y = 7.0;
        });
        let mut ys: Vec<f32> = ["Alpha", "Beta", "Gamma", "Delta"]
            .iter()
            .map(|want| {
                texts
                    .iter()
                    .find(|p| p.text.trim() == *want)
                    .unwrap_or_else(|| panic!("{want} must be painted"))
                    .pos
                    .y
            })
            .collect();
        ys.sort_by(f32::total_cmp);
        let pitch = ys[1] - ys[0];
        for pair in ys.windows(2) {
            assert!(
                (pair[1] - pair[0] - pitch).abs() <= 0.5,
                "the lines must be evenly spaced, got {ys:?}"
            );
        }

        // A line is the text's own height plus a couple of pixels â nothing like
        // the host's 30 px + 8 px, which would put the pitch at 38.
        let line = texts
            .iter()
            .find(|p| p.text.trim() == "Alpha")
            .map(|p| p.size.y)
            .expect("Alpha must be painted");
        assert!(
            pitch <= crate::model::text_line_height(&ctrl("x", ControlType::ListBox, 0, 0, 10, 10))
                + crate::model::LIST_ROW_PAD * 2.0
                + 0.5,
            "a list line must be about its text ({line}px), got a pitch of {pitch}px"
        );

        println!(
            "\n  ListBox lines â under 30px/8px host chrome the pitch is {pitch}px \
             (a ~14px line plus air), not the host's 38px\n"
        );
    }

    /// Every rectangle painted for `controls` in Interactive mode, plus each
    /// control's own screen rect â enough to answer "where did that scrollbar
    /// end up, relative to the control it belongs to?".
    fn painted_rects_interactive(controls: &[Control]) -> (Vec<Rect>, Map<String, Rect>) {
        let placed: RefCell<Map<String, Rect>> = RefCell::new(Map::new());
        fn collect(shape: &egui::Shape, out: &mut Vec<Rect>) {
            match shape {
                egui::Shape::Rect(r) => out.push(r.rect),
                egui::Shape::Vec(v) => v.iter().for_each(|s| collect(s, out)),
                _ => {}
            }
        }
        let ctx = egui::Context::default();
        let active = ActiveTabs::new();
        let mut full = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(700.0, 560.0),
                )),
                ..Default::default()
            },
            |root_ui| {
                egui::CentralPanel::default().show_inside(root_ui, |ui| {
                    ui.set_min_size(Vec2::new(640.0, 480.0));
                    let input = RenderInput {
                        controls,
                        state: &DesignedState,
                        form_size: Vec2::new(640.0, 480.0),
                        glass: true,
                        mode: RenderMode::Interactive,
                        active_tabs: &active,
                        backdrop: Backdrop::default(),
                    };
                    let out = render_form(ui, &input);
                    *placed.borrow_mut() = out.control_rects;
                });
            },
        );
        let mut out = Vec::new();
        for cs in &full.shapes {
            collect(&cs.shape, &mut out);
        }
        full.textures_delta.clear();
        (out, placed.into_inner())
    }

    /// Clicking a toggle's CAPTION is a click on the toggle. The caption is the
    /// bigger target of the two and the one a developer's user aims at, so it
    /// must set the state exactly as hitting the box or the circle does â in
    /// every surface the engine drives.
    #[test]
    fn a_click_on_a_toggles_caption_is_a_click_on_the_toggle() {
        for (t, id) in [
            (ControlType::CheckBox, "CheckBox-1"),
            (ControlType::RadioButton, "RadioButton-1"),
        ] {
            // 240x34 at (20,20): the indicator occupies the first ~26px, so a
            // point at x = 150 is squarely on the caption, far from it.
            let c = ctrl(id, t.clone(), 20, 20, 240, 34);
            let on_caption = pos2(170.0, 37.0);
            let (events, overrides) = drive(
                &[c],
                vec![
                    (0.0, vec![Event::PointerMoved(on_caption)]),
                    (0.05, vec![press(on_caption)]),
                    (0.10, vec![release(on_caption)]),
                    (0.15, vec![]),
                ],
            );
            let state = overrides
                .get(id)
                .and_then(|p| p.get("Value"))
                .map(String::as_str);
            assert_eq!(
                state,
                Some("1"),
                "{t:?}: a click on the caption must set the toggle; events: {events:?}"
            );
        }

        println!(
            "\n  toggle captions â a click at x=170 (well past the indicator) sets both \
             the CheckBox and the RadioButton\n"
        );
    }

    /// Every circle painted for `controls`, as `(centre, radius)`.
    fn painted_circles(controls: &[Control]) -> Vec<(egui::Pos2, f32)> {
        fn collect(shape: &egui::Shape, out: &mut Vec<(egui::Pos2, f32)>) {
            match shape {
                egui::Shape::Circle(c) => out.push((c.center, c.radius)),
                egui::Shape::Vec(v) => v.iter().for_each(|s| collect(s, out)),
                _ => {}
            }
        }
        let ctx = egui::Context::default();
        let active = ActiveTabs::new();
        let mut full = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(700.0, 560.0),
                )),
                ..Default::default()
            },
            |root_ui| {
                egui::CentralPanel::default().show_inside(root_ui, |ui| {
                    ui.set_min_size(Vec2::new(640.0, 480.0));
                    let input = RenderInput {
                        controls,
                        state: &DesignedState,
                        form_size: Vec2::new(640.0, 480.0),
                        glass: true,
                        mode: RenderMode::Interactive,
                        active_tabs: &active,
                        backdrop: Backdrop::default(),
                    };
                    let _ = render_form(ui, &input);
                });
            },
        );
        let mut out = Vec::new();
        for cs in &full.shapes {
            collect(&cs.shape, &mut out);
        }
        full.textures_delta.clear();
        out
    }

    fn knob(id: &str, w: i32, h: i32, value: &str) -> Control {
        let mut k = ctrl(id, ControlType::Knob, 20, 20, w, h);
        k.set_prop("Value", crate::PropValue::String(value.to_owned()));
        k
    }

    /// A Knob is the size it was DRAWN, and its value is centred under the dial.
    /// The widget this replaced picked one of three fixed pixel sizes whatever
    /// the designed rect said, and laid its value out with egui â so the canvas
    /// and the preview disagreed about both, and the reading sat off-centre.
    #[test]
    fn a_knob_fills_its_rect_and_centres_its_value() {
        let mut radii = Vec::new();
        for (w, h) in [(80, 96), (200, 220)] {
            let k = knob("Knob-1", w, h, "42");
            let circles = painted_circles(&[k.clone()]);
            let (texts, placed) = painted_captions(&[k]);
            let rect = *placed.get("Knob-1").expect("placed");

            // The dial: the biggest circle painted inside the control.
            let dial = circles
                .iter()
                .filter(|(c, _)| rect.contains(*c))
                .map(|(_, r)| *r)
                .fold(0.0_f32, f32::max);
            assert!(dial > 0.0, "the dial must be painted for {w}x{h}");
            radii.push((w, h, dial));

            let val = texts
                .iter()
                .find(|p| p.text.trim() == "42")
                .expect("the value must be painted");
            assert!(
                (val.ink.center().x - rect.center().x).abs() <= 2.0,
                "the value must be centred on the control: ink {:?}, control {:?}",
                val.ink,
                rect
            );
            assert!(
                rect.expand(1.0).contains_rect(val.ink),
                "â¦and stay inside it: ink {:?}, control {:?}",
                val.ink,
                rect
            );
        }

        assert!(
            radii[1].2 > radii[0].2 * 1.5,
            "a knob drawn bigger must BE bigger: {radii:?}"
        );
        println!(
            "\n  Knob â 80x96 draws a dial of r={:.0}, 200x220 one of r={:.0}; \
             the value is centred on the control in both\n",
            radii[0].2, radii[1].2
        );
    }

    /// â¦and it turns. The preview drives the same painter, so the knob has to
    /// carry its own dragging: press on it and pull, and the value follows.
    #[test]
    fn a_knob_turns_when_dragged() {
        let k = knob("Knob-1", 120, 120, "50");
        // Down the middle of the control, dragging UPWARD to raise the value.
        let start = pos2(88.0, 88.0);
        let (events, overrides) = drive(
            &[k],
            vec![
                (0.0, vec![Event::PointerMoved(start)]),
                (0.05, vec![press(start)]),
                (0.10, vec![Event::PointerMoved(pos2(88.0, 48.0))]),
                (0.15, vec![release(pos2(88.0, 48.0))]),
            ],
        );
        let value = overrides
            .get("Knob-1")
            .and_then(|p| p.get("Value"))
            .and_then(|v| v.parse::<f32>().ok());
        assert!(
            value.is_some_and(|v| v > 50.0),
            "dragging up must raise the value, got {value:?} (events {})",
            events.len()
        );
        println!(
            "\n  Knob â a 40px upward drag on a 120px knob moved 50 â {}\n",
            value.unwrap()
        );
    }

    /// A Gauge paints with the colours the developer set â `ForegroundColor`
    /// for the meter, `BackgroundColor` for its track â and keeps its reading
    /// clear of the band it reports. The palette widget it replaced took both
    /// colours from its own theme and dropped the reading at the control's
    /// centre, which on a Radial is the middle of the sweep.
    #[test]
    fn a_gauge_paints_in_its_own_colours_and_keeps_its_reading_clear() {
        let mut g = ctrl("Gauge-1", ControlType::Gauge, 20, 20, 160, 100);
        g.set_prop("Value", crate::PropValue::Int(60));
        g.set_prop(
            "ForegroundColor",
            crate::PropValue::String("#FFD400".to_owned()),
        );
        g.set_prop(
            "BackgroundColor",
            crate::PropValue::String("#402060".to_owned()),
        );

        let (texts, placed) = painted_captions(&[g.clone()]);
        let rect = *placed.get("Gauge-1").expect("placed");
        let reading = texts
            .iter()
            .find(|p| p.text.trim() == "60")
            .expect("the reading must be painted");

        // Centred on the control, and in its upper half â inside the dial,
        // not down on the sweep's own centre line.
        assert!(
            (reading.ink.center().x - rect.center().x).abs() <= 2.0,
            "the reading must be centred: ink {:?}, gauge {:?}",
            reading.ink,
            rect
        );
        assert!(
            reading.ink.bottom() < rect.bottom() - rect.height() * 0.2,
            "the reading must sit clear of the band: ink {:?}, gauge {:?}",
            reading.ink,
            rect
        );

        // The meter is drawn in the chosen colours. Arcs are line shapes, so
        // look at every stroke the control painted.
        let strokes = painted_stroke_colours(&[g]);
        let has = |c: Color32| strokes.iter().any(|s| *s == c);
        assert!(
            has(Color32::from_rgb(0xFF, 0xD4, 0x00)),
            "ForegroundColor must paint the meter, got {strokes:?}"
        );
        assert!(
            has(Color32::from_rgb(0x40, 0x20, 0x60)),
            "BackgroundColor must paint the track, got {strokes:?}"
        );

        println!(
            "\n  Gauge â meter #FFD400 and track #402060 both reach the painter; \
             the reading is centred at x={:.0} and clear of the band\n",
            reading.ink.center().x
        );
    }

    /// The colours of every stroked path painted for `controls`.
    fn painted_stroke_colours(controls: &[Control]) -> Vec<Color32> {
        fn collect(shape: &egui::Shape, out: &mut Vec<Color32>) {
            match shape {
                egui::Shape::Path(p) => {
                    if let egui::epaint::ColorMode::Solid(c) = p.stroke.color {
                        out.push(c);
                    }
                }
                egui::Shape::LineSegment { stroke, .. } => out.push(stroke.color),
                egui::Shape::Rect(r) => {
                    out.push(r.fill);
                    out.push(r.stroke.color);
                }
                egui::Shape::Vec(v) => v.iter().for_each(|s| collect(s, out)),
                _ => {}
            }
        }
        let ctx = egui::Context::default();
        let active = ActiveTabs::new();
        let mut full = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(700.0, 560.0),
                )),
                ..Default::default()
            },
            |root_ui| {
                egui::CentralPanel::default().show_inside(root_ui, |ui| {
                    ui.set_min_size(Vec2::new(640.0, 480.0));
                    let input = RenderInput {
                        controls,
                        state: &DesignedState,
                        form_size: Vec2::new(640.0, 480.0),
                        glass: true,
                        mode: RenderMode::Interactive,
                        active_tabs: &active,
                        backdrop: Backdrop::default(),
                    };
                    let _ = render_form(ui, &input);
                });
            },
        );
        let mut out = Vec::new();
        for cs in &full.shapes {
            collect(&cs.shape, &mut out);
        }
        full.textures_delta.clear();
        out
    }

    /// A RadioButton's frame follows its own `BorderStyle`, seeded `None`. That
    /// property only ever governed the explicit border stroke, so the themed
    /// card underneath still drew a rim around every radio â under every theme,
    /// with nothing in the properties pane able to turn it off.
    #[test]
    fn a_radiobutton_paints_no_frame_of_its_own() {
        let bare = ctrl("RadioButton-1", ControlType::RadioButton, 20, 20, 240, 34);
        let (rects, placed) = painted_rects_interactive(&[bare.clone()]);
        let r = *placed.get("RadioButton-1").expect("placed");
        // Anything the size of the control itself is a card or a border.
        let framing = |rects: &[Rect], r: Rect| -> Vec<Rect> {
            rects
                .iter()
                .copied()
                .filter(|x| {
                    (x.width() - r.width()).abs() <= 4.0 && (x.height() - r.height()).abs() <= 4.0
                })
                .collect()
        };
        let framed = framing(&rects, r);
        assert!(
            framed.is_empty(),
            "a bare radio must paint no card or border of its own, got {framed:?}"
        );

        // â¦and a developer who asks for a border with the property gets one.
        let mut bordered = bare;
        bordered.set_prop(
            "BorderStyle",
            crate::PropValue::String("Single".to_owned()),
        );
        let (rects, placed) = painted_rects_interactive(&[bordered]);
        let r = *placed.get("RadioButton-1").expect("placed");
        assert!(
            !framing(&rects, r).is_empty(),
            "BorderStyle = Single must bring the frame back"
        );

        println!(
            "\n  RadioButton â a {}x{} control with the seeded BorderStyle None paints no \
             frame; BorderStyle Single brings it back\n",
            r.width(),
            r.height()
        );
    }

    /// A ListBox's scrollbar belongs against its RIGHT border. The scroll area
    /// was left to shrink to its content, so the bar came to rest just past the
    /// widest item â a white column through the middle of the list.
    #[test]
    fn a_listbox_scrollbar_sits_against_its_right_border() {
        let mut lb = ctrl("ListBox-1", ControlType::ListBox, 20, 20, 320, 120);
        let items = (1..=12).map(|n| n.to_string()).collect::<Vec<_>>();
        lb.set_prop("Items", crate::PropValue::String(items.join("\n")));

        let (rects, placed) = painted_rects_interactive(&[lb]);
        let list = *placed.get("ListBox-1").expect("the list is placed");

        // A scrollbar is a tall, narrow rectangle inside the list's band. Its
        // width depends on the ambient scroll style (a floating hairline here, a
        // solid bar in the IDE), so only its POSITION is asserted.
        let bars: Vec<Rect> = rects
            .iter()
            .copied()
            .filter(|r| {
                r.height() > r.width() * 3.0
                    && r.height() > 20.0
                    // Inside the control's band â which rules out the scrolled
                    // CONTENT, taller than the list by definition (that is why
                    // there is a bar at all).
                    && r.height() <= list.height() + 1.0
                    && r.top() >= list.top() - 1.0
                    && r.left() >= list.left() - 1.0
                    && r.right() <= list.right() + 1.0
            })
            .collect();
        assert!(
            !bars.is_empty(),
            "the list must have a scrollbar to place; rects: {rects:?}"
        );

        for bar in &bars {
            assert!(
                bar.center().x >= list.right() - 12.0,
                "the scrollbar must hug the right border: bar at {}, list spans {}..{}",
                bar.center().x,
                list.left(),
                list.right()
            );
        }

        println!(
            "\n  ListBox scrollbar â control spans x {}..{}: bar centred at {}, \
             i.e. {:.0}px from the right border\n",
            list.left(),
            list.right(),
            bars[0].center().x,
            list.right() - bars[0].center().x
        );
    }

    /// A ListBox's items are egui widgets, so their text came from the AMBIENT
    /// visuals: the developer's `ForegroundColor` never reached them, which is
    /// how a list ended up drawn in a dim grey on a dark theme's well. They are
    /// now painted like every other text this engine draws â the control's own
    /// colour, rescued to the pole that reads when it would not clear AA.
    #[test]
    fn listbox_items_are_painted_in_the_controls_own_colour() {
        let mut lb = ctrl("ListBox-1", ControlType::ListBox, 20, 20, 220, 120);
        lb.set_prop(
            "Items",
            crate::PropValue::String("Alpha\nBeta\nGamma".to_owned()),
        );
        let texts = painted_text_interactive(&[lb.clone()]);
        let (_, default_colour) = texts
            .iter()
            .find(|(s, _)| s.trim() == "Alpha")
            .expect("the items must be painted");

        // A colour that already reads on the list's well is used as chosen.
        lb.set_prop(
            "ForegroundColor",
            crate::PropValue::String("#FFD400".to_owned()),
        );
        let chosen = painted_text_interactive(&[lb]);
        let (_, chosen_colour) = chosen
            .iter()
            .find(|(s, _)| s.trim() == "Alpha")
            .expect("the items must be painted");

        assert_eq!(
            (chosen_colour.r(), chosen_colour.g(), chosen_colour.b()),
            (0xFF, 0xD4, 0x00),
            "the developer's ForegroundColor must reach the item text"
        );
        let tone = crate::paint::parse_color(crate::model::DEFAULT_BACKGROUND_COLOR);
        assert!(
            crate::paint::contrast_ratio(*default_colour, tone) >= 4.5
                || crate::paint::contrast_ratio(*default_colour, Color32::from_gray(20)) >= 4.5,
            "the default item colour must read on the well, got {default_colour:?}"
        );

        println!(
            "\n  ListBox items â default paints {:?}; a chosen #FFD400 reaches the \
             item text instead of egui's ambient colour\n",
            default_colour
        );
    }

    /// The operator's label: same place, same colours as `RustDemo`'s form.
    fn operator_label(caption: &str, foreground: &str) -> Control {
        let mut c = ctrl("Label-1", ControlType::Label, 174, 376, 280, 20);
        c.set_prop("Caption", crate::PropValue::String(caption.to_owned()));
        c.set_prop(
            "ForegroundColor",
            crate::PropValue::String(foreground.to_owned()),
        );
        c.set_prop(
            "BackgroundColor",
            crate::PropValue::String("#F0F0F0".to_owned()),
        );
        c
    }

    /// A caption a handler just wrote IS painted â the write reaches the screen.
    ///
    /// Pairs with cobolt-runtime's
    /// `setting_a_control_property_from_an_object_reference_emits_a_state_update`,
    /// which proves the other end: the update leaves the interpreter carrying
    /// the dereferenced value. Together they close the loop the operator kept
    /// reporting as "the label never changes".
    #[test]
    fn a_handler_written_caption_is_painted() {
        let texts = painted_text(&[operator_label("1", "#FFFFFF")], "#00000000");
        assert!(
            texts.iter().any(|(s, _)| s.trim() == "1"),
            "the new caption must be painted, got {texts:?}"
        );
    }

    /// â¦and on that form it is painted WHITE onto a fully transparent
    /// backdrop, which is why it cannot be read.
    ///
    /// A Label's own face is transparent (it never paints its
    /// `BackgroundColor`), so its text sits on whatever the FORM shows. A form
    /// created with the default transparent background and a label created
    /// with the default white foreground are invisible together â the COBOL is
    /// correct, the update arrives, the glyphs are drawn in white over nothing.
    #[test]
    fn white_text_on_a_transparent_backdrop_is_the_invisible_pairing() {
        let texts = painted_text(&[operator_label("1", "#FFFFFF")], "#00000000");
        let (_, colour) = texts
            .iter()
            .find(|(s, _)| s.trim() == "1")
            .expect("the caption is painted");
        assert_eq!(
            (colour.r(), colour.g(), colour.b()),
            (255, 255, 255),
            "painted white â unreadable over a transparent form"
        );

        // A dark ForegroundColor â the fix â reaches the painter.
        let dark = painted_text(&[operator_label("1", "#202020")], "#00000000");
        let (_, dark_colour) = dark
            .iter()
            .find(|(s, _)| s.trim() == "1")
            .expect("the caption is painted");
        assert!(
            dark_colour.r() < 64 && dark_colour.g() < 64 && dark_colour.b() < 64,
            "a dark ForegroundColor must reach the painter, got {dark_colour:?}"
        );
    }

    #[test]
    fn render_form_static_smoke() {
        // Headless: a form with a Panel â Button renders without panic and reports
        // both control rects through the engine (parity foundation).
        let controls = vec![
            {
                let mut c = ctrl("Pnl", ControlType::Panel, 0, 0, 200, 120);
                c.parent = None;
                c
            },
            {
                let mut c = ctrl("Btn", ControlType::Button, 20, 30, 80, 24);
                c.parent = Some("Pnl".into());
                c
            },
        ];
        let ctx = egui::Context::default();
        let active = ActiveTabs::new();
        let mut captured = None;
        ctx.run_ui(Default::default(), |root_ui| {
            let ctx = root_ui.ctx().clone();
            let ctx = &ctx;
            egui::CentralPanel::default().show_inside(root_ui, |ui| {
                ui.set_min_size(Vec2::new(400.0, 300.0));
                let input = RenderInput {
                    controls: &controls,
                    state: &DesignedState,
                    form_size: Vec2::new(400.0, 300.0),
                    glass: true,
                    mode: RenderMode::Static,
                    active_tabs: &active,
                    backdrop: Backdrop {
                        paint: true,
                        color_hex: "#00000000".into(),
                        ..Default::default()
                    },
                };
                captured = Some(render_form(ui, &input));
            });
        }).textures_delta.clear();
        let out = captured.expect("rendered");
        assert!(out.control_rects.contains_key("Pnl"));
        assert!(out.control_rects.contains_key("Btn"));
    }

    #[test]
    fn engine_reference_form_parity_static_vs_faces() {
        // Parity invariant (spec 017 T8): the designer canvas entry `render_faces`
        // and the `render_form(Static)` entry used by every other surface must
        // agree on every control's on-screen geometry for a reference form
        // (Panel â {AreaChart, PictureBox, TextBox} + a top-level Label). This is
        // the guarantee that designer == preview == run == binary.
        let controls = vec![
            {
                let mut c = ctrl("Pnl", ControlType::Panel, 10, 10, 300, 200);
                c.parent = None;
                c
            },
            {
                let mut c = ctrl("Chart", ControlType::AreaChart, 20, 30, 120, 80);
                c.parent = Some("Pnl".into());
                c
            },
            {
                let mut c = ctrl("Pic", ControlType::PictureBox, 20, 120, 60, 60);
                c.parent = Some("Pnl".into());
                c
            },
            {
                let mut c = ctrl("Txt", ControlType::TextBox, 150, 40, 120, 24);
                c.parent = Some("Pnl".into());
                c
            },
            {
                let mut c = ctrl("Lbl", ControlType::Label, 10, 230, 100, 20);
                c.parent = None;
                c
            },
        ];
        let ctx = egui::Context::default();
        ctx.set_fonts(egui::FontDefinitions::default());
        let active = ActiveTabs::new();
        let (mut rects_form, mut rects_faces) = (None, None);
        ctx.run_ui(Default::default(), |root_ui| {
            let ctx = root_ui.ctx().clone();
            let ctx = &ctx;
            egui::CentralPanel::default().show_inside(root_ui, |ui| {
                ui.set_min_size(Vec2::new(400.0, 300.0));
                let input = RenderInput {
                    controls: &controls,
                    state: &DesignedState,
                    form_size: Vec2::new(400.0, 300.0),
                    glass: true,
                    mode: RenderMode::Static,
                    active_tabs: &active,
                    backdrop: Backdrop::default(),
                };
                rects_form = Some(render_form(ui, &input).control_rects);
                let painter = ui.painter().clone();
                let origin = ui.min_rect().min;
                rects_faces = Some(render_faces(&painter, origin, &input, None).control_rects);
            });
        }).textures_delta.clear();
        let rf = rects_form.expect("render_form rects");
        let fc = rects_faces.expect("render_faces rects");
        for id in ["Pnl", "Chart", "Pic", "Txt", "Lbl"] {
            let a = rf
                .get(id)
                .unwrap_or_else(|| panic!("render_form missing {id}"));
            let b = fc
                .get(id)
                .unwrap_or_else(|| panic!("render_faces missing {id}"));
            assert!(
                (a.min.x - b.min.x).abs() < 0.5
                    && (a.min.y - b.min.y).abs() < 0.5
                    && (a.width() - b.width()).abs() < 0.5
                    && (a.height() - b.height()).abs() < 0.5,
                "geometry mismatch for {id}: render_form={a:?} render_faces={b:?}",
            );
        }
    }

    // ââ Interaction simulation (Interactive mode) âââââââââââââââââââââââââââââ
    // Drive the engine headlessly with simulated pointer/text/time input and
    // assert the neutral events + property updates it produces (T3 verification).
    use egui::{pos2, Event, Key, Modifiers, PointerButton, Pos2};
    use std::cell::RefCell;
    use std::collections::HashMap as Map;

    fn ctrlp(
        id: &str,
        t: ControlType,
        x: i32,
        y: i32,
        w: i32,
        h: i32,
        props: &[(&str, &str)],
    ) -> Control {
        let mut c = ctrl(id, t, x, y, w, h);
        for (k, v) in props {
            c.set_prop((*k).to_owned(), crate::PropValue::String((*v).to_owned()));
        }
        c
    }

    fn ctrlp_events(
        id: &str,
        t: ControlType,
        x: i32,
        y: i32,
        w: i32,
        h: i32,
        props: &[(&str, &str)],
        events: &[&str],
    ) -> Control {
        let mut c = ctrlp(id, t, x, y, w, h, props);
        for event in events {
            c.ensure_event(event);
        }
        c
    }

    fn press(p: Pos2) -> Event {
        Event::PointerButton {
            pos: p,
            button: PointerButton::Primary,
            pressed: true,
            modifiers: Modifiers::default(),
        }
    }
    fn release(p: Pos2) -> Event {
        Event::PointerButton {
            pos: p,
            button: PointerButton::Primary,
            pressed: false,
            modifiers: Modifiers::default(),
        }
    }
    fn right_click(p: Pos2) -> Vec<Event> {
        vec![
            Event::PointerMoved(p),
            Event::PointerButton {
                pos: p,
                button: PointerButton::Secondary,
                pressed: true,
                modifiers: Modifiers::default(),
            },
            Event::PointerButton {
                pos: p,
                button: PointerButton::Secondary,
                pressed: false,
                modifiers: Modifiers::default(),
            },
        ]
    }

    /// A `FormState` over a per-control live-override map (id â key â value),
    /// exactly how the run/compiled callers will drive the engine.
    struct MapState<'a>(&'a RefCell<Map<String, Map<String, String>>>);
    impl FormState for MapState<'_> {
        fn live(&self, base: &Control) -> Control {
            let m = self.0.borrow();
            match m.get(&base.id) {
                Some(p) => merge_props(base, p.iter()),
                None => base.clone(),
            }
        }
    }

    /// Run `frames` (each: simulated time + input events) through the engine in
    /// Interactive mode, applying prop updates between frames. Returns the events
    /// produced and the final override map.
    /// Like [`drive`] but wraps the engine in `ScrollArea::both()` â the way the
    /// running form and compiled binary host it. Diagnoses whether the scroll
    /// area swallows a widget drag (slider).
    fn drive_scroll(
        controls: &[Control],
        frames: Vec<(f64, Vec<Event>)>,
    ) -> (Vec<UiEvent>, Map<String, Map<String, String>>) {
        let ctx = egui::Context::default();
        ctx.set_fonts(egui::FontDefinitions::default());
        let active = ActiveTabs::new();
        let overrides: RefCell<Map<String, Map<String, String>>> = RefCell::new(Map::new());
        let mut all: Vec<UiEvent> = Vec::new();
        for (i, (_t, evs)) in frames.into_iter().enumerate() {
            let mut input = egui::RawInput::default();
            input.screen_rect = Some(Rect::from_min_size(
                pos2(0.0, 0.0),
                Vec2::new(1000.0, 800.0),
            ));
            input.focused = true;
            input.time = Some(i as f64 * 0.05);
            input.events = evs;
            let updates = RefCell::new(Vec::<(String, String, String)>::new());
            let events = RefCell::new(Vec::<UiEvent>::new());
            let st = MapState(&overrides);
            ctx.run_ui(input, |root_ui| {
                let ctx = root_ui.ctx().clone();
                let ctx = &ctx;
                egui::CentralPanel::default()
                    .frame(egui::Frame::NONE)
                    .show_inside(root_ui, |ui| {
                        egui::ScrollArea::both()
                            .auto_shrink([false, false])
                            .show(ui, |ui| {
                                // Content larger than the 1000Ã800 viewport â the scroll area
                                // has scroll room, reproducing the binary where the form is
                                // bigger than the window (drag-to-scroll can steal a drag).
                                ui.set_min_size(Vec2::new(2000.0, 2000.0));
                                let inp = RenderInput {
                                    controls,
                                    state: &st,
                                    form_size: Vec2::new(2000.0, 2000.0),
                                    glass: true,
                                    mode: RenderMode::Interactive,
                                    active_tabs: &active,
                                    backdrop: Backdrop::default(),
                                };
                                let out = render_form(ui, &inp);
                                updates.borrow_mut().extend(out.prop_updates);
                                events.borrow_mut().extend(out.events);
                            });
                    });
            }).textures_delta.clear();
            for (id, k, v) in updates.into_inner() {
                overrides.borrow_mut().entry(id).or_default().insert(k, v);
            }
            all.extend(events.into_inner());
        }
        (all, overrides.into_inner())
    }

    #[test]
    fn engine_slider_drag_inside_scrollarea() {
        let c = [ctrlp(
            "Sld",
            ControlType::Slider,
            0,
            0,
            200,
            30,
            &[
                ("Minimum", "0"),
                ("Maximum", "100"),
                ("Value", "50"),
                ("Step", "1"),
            ],
        )];
        let screen = Rect::from_min_size(pos2(0.0, 0.0), Vec2::new(200.0, 30.0));
        let tc = crate::paint::slider_thumb_rect(screen, 0.0, 100.0, 50.0, false).center();
        let to = tc + egui::vec2(30.0, 0.0);
        let (_evs, map) = drive_scroll(
            &c,
            vec![
                (0.0, vec![]),
                (1.0, vec![Event::PointerMoved(tc), press(tc)]),
                (2.0, vec![Event::PointerMoved(to)]),
                (3.0, vec![Event::PointerMoved(to)]),
                (4.0, vec![release(to)]),
            ],
        );
        let v = map
            .get("Sld")
            .and_then(|m| m.get("Value"))
            .cloned()
            .unwrap_or_default();
        assert_ne!(
            v, "50",
            "Slider inside ScrollArea: Value did not change after drag (still {v})"
        );
    }

    /// Drive a DataGrid inside `ScrollArea::both()`, send a wheel event at
    /// `wheel_pos`, and return `(outer_scrollarea_offset_y, datagrid_scroll_y)`
    /// after the wheel frame. Mirrors how the run/compiled surfaces host a form.
    fn drive_datagrid_wheel(controls: &[Control], wheel_pos: Pos2) -> (f32, f32) {
        let ctx = egui::Context::default();
        ctx.set_fonts(egui::FontDefinitions::default());
        let active = ActiveTabs::new();
        let overrides: RefCell<Map<String, Map<String, String>>> = RefCell::new(Map::new());
        let mut outer_offset_y = 0.0_f32;
        // Frame 0 settles layout; frame 1 delivers the wheel over the grid.
        let frames: Vec<Vec<Event>> = vec![
            vec![],
            vec![
                Event::PointerMoved(wheel_pos),
                Event::MouseWheel {
                    unit: egui::MouseWheelUnit::Point,
                    delta: Vec2::new(0.0, -40.0), // negative y = scroll down
                    modifiers: Modifiers::default(),
                    phase: egui::TouchPhase::Move,
                },
            ],
        ];
        for (i, evs) in frames.into_iter().enumerate() {
            let mut input = egui::RawInput::default();
            input.screen_rect = Some(Rect::from_min_size(
                pos2(0.0, 0.0),
                Vec2::new(1000.0, 800.0),
            ));
            input.focused = true;
            input.time = Some(i as f64 * 0.05);
            input.events = evs;
            let st = MapState(&overrides);
            ctx.run_ui(input, |root_ui| {
                let ctx = root_ui.ctx().clone();
                let ctx = &ctx;
                egui::CentralPanel::default()
                    .frame(egui::Frame::NONE)
                    .show_inside(root_ui, |ui| {
                        let out =
                            egui::ScrollArea::both()
                                .auto_shrink([false, false])
                                .show(ui, |ui| {
                                    // Content larger than the viewport â the outer area
                                    // has room to (wrongly) scroll if the grid bleeds.
                                    ui.set_min_size(Vec2::new(2000.0, 2000.0));
                                    let inp = RenderInput {
                                        controls,
                                        state: &st,
                                        form_size: Vec2::new(2000.0, 2000.0),
                                        glass: true,
                                        mode: RenderMode::Interactive,
                                        active_tabs: &active,
                                        backdrop: Backdrop::default(),
                                    };
                                    render_form(ui, &inp);
                                });
                        outer_offset_y = out.state.offset.y;
                    });
            }).textures_delta.clear();
        }
        let grid_scroll_y = ctx.memory(|m| {
            m.data
                .get_temp::<f32>(egui::Id::new(("rt_ctrl", "Grd")).with("datagrid-scroll-y"))
                .unwrap_or(0.0)
        });
        (outer_offset_y, grid_scroll_y)
    }

    #[test]
    fn engine_datagrid_wheel_does_not_bleed_into_container() {
        // A scrollable DataGrid (many rows) at the top-left of the form.
        let rows: String = (0..50).map(|i| format!("row{i}\n")).collect();
        let grid = ctrlp(
            "Grd",
            ControlType::DataGrid,
            0,
            0,
            300,
            150,
            &[("Columns", "A:string"), ("Rows", &rows)],
        );

        // Wheel with the pointer OVER the grid: the grid scrolls, the outer
        // ScrollArea must stay put (no bleed).
        let (outer_y, grid_y) = drive_datagrid_wheel(&[grid.clone()], pos2(150.0, 90.0));
        assert!(
            grid_y > 0.0,
            "DataGrid should scroll on wheel (grid_scroll_y={grid_y})"
        );
        assert!(
            outer_y.abs() < 0.5,
            "wheel over the DataGrid must not scroll the container (outer offset_y={outer_y})"
        );

        // Sanity: the same wheel with the pointer OUTSIDE the grid DOES scroll
        // the outer area â proving the harness allows outer scrolling, so the
        // assertion above is not vacuous.
        let (outer_y_off, _grid_y_off) = drive_datagrid_wheel(&[grid], pos2(600.0, 400.0));
        assert!(
            outer_y_off > 0.5,
            "control test: wheel outside the grid should scroll the container (offset_y={outer_y_off})"
        );
    }

    fn drive(
        controls: &[Control],
        frames: Vec<(f64, Vec<Event>)>,
    ) -> (Vec<UiEvent>, Map<String, Map<String, String>>) {
        let ctx = egui::Context::default();
        ctx.set_fonts(egui::FontDefinitions::default());
        let active = ActiveTabs::new();
        let overrides: RefCell<Map<String, Map<String, String>>> = RefCell::new(Map::new());
        let mut all: Vec<UiEvent> = Vec::new();

        for (i, (_time, evs)) in frames.into_iter().enumerate() {
            let mut input = egui::RawInput::default();
            input.screen_rect = Some(Rect::from_min_size(
                pos2(0.0, 0.0),
                Vec2::new(1000.0, 800.0),
            ));
            input.focused = true;
            // Advance by small steps so a pressârelease across two frames still
            // counts as a click (egui's max click duration), while clearing the
            // Timer's 10 ms interval.
            input.time = Some(i as f64 * 0.05);
            input.events = evs;

            let updates = RefCell::new(Vec::<(String, String, String)>::new());
            let events = RefCell::new(Vec::<UiEvent>::new());
            let st = MapState(&overrides);
            ctx.run_ui(input, |root_ui| {
                let ctx = root_ui.ctx().clone();
                let ctx = &ctx;
                egui::CentralPanel::default()
                    .frame(egui::Frame::NONE)
                    .show_inside(root_ui, |ui| {
                        ui.set_min_size(Vec2::new(400.0, 300.0));
                        let inp = RenderInput {
                            controls,
                            state: &st,
                            form_size: Vec2::new(400.0, 300.0),
                            glass: true,
                            mode: RenderMode::Interactive,
                            active_tabs: &active,
                            backdrop: Backdrop::default(),
                        };
                        let out = render_form(ui, &inp);
                        updates.borrow_mut().extend(out.prop_updates);
                        events.borrow_mut().extend(out.events);
                    });
            }).textures_delta.clear();
            for (id, k, v) in updates.into_inner() {
                overrides.borrow_mut().entry(id).or_default().insert(k, v);
            }
            all.extend(events.into_inner());
        }
        (all, overrides.into_inner())
    }

    fn names(evs: &[UiEvent]) -> Vec<&str> {
        evs.iter().map(|e| e.event.as_str()).collect()
    }

    /// Pressing a toolbar button raises TWO `onClick`s: the toolbar's own, which
    /// is what lets one handler serve a whole bar, and the button's own under the
    /// derived `<toolbar>-<group>-<button>` id.
    ///
    /// The second one is why `procedure:` and `open-modal:` can reach anything: a
    /// toolbar button is not a `Control`, so nothing in `form.controls` names it,
    /// and the generated event loop can only dispatch what it has a `WHEN` for.
    /// Both sides derive that id from the same function, so the press and the
    /// `WHEN` cannot drift apart.
    #[test]
    fn a_toolbar_press_raises_the_bars_click_and_the_buttons_own() {
        use crate::toolbar::{button_control_id, ToolbarButton, ToolbarDef, ToolbarGroup};

        let (bar_w, bar_h) = (300, 44);
        let mut group = ToolbarGroup::new("group-1", "File");
        group.buttons.push(ToolbarButton::new("button-1", "Save"));
        group.buttons.push(ToolbarButton::new("button-2", "Find"));
        let def = ToolbarDef {
            groups: vec![group],
            button_gap: 4,
        };

        let mut bar = ctrl("TOOLBAR-1", ControlType::ToolBar, 20, 20, bar_w, bar_h);
        bar.set_prop(
            crate::toolbar::TOOLBAR_DEF_PROP,
            crate::PropValue::String(def.to_json().unwrap()),
        );

        // Click the SECOND button, from the model's own geometry rather than a
        // hand-computed offset â the layout is what the painter used.
        let layout = def.layout(bar_w as i64, bar_h as i64);
        let (id, box2) = &layout[0].buttons[1];
        assert_eq!(id, "button-2");
        let at = pos2(
            20.0 + box2.x as f32 + box2.w as f32 / 2.0,
            20.0 + box2.y as f32 + box2.h as f32 / 2.0,
        );

        let (events, overrides) = drive(
            &[bar],
            vec![
                (0.0, vec![Event::PointerMoved(at)]),
                (0.05, vec![press(at)]),
                (0.10, vec![release(at)]),
            ],
        );

        // WHICH button it was, for the one-handler-per-bar route.
        assert_eq!(
            overrides
                .get("TOOLBAR-1")
                .and_then(|p| p.get("LastButton"))
                .map(String::as_str),
            Some("button-2"),
            "LastButton must name the button that was pressed"
        );

        let derived = button_control_id("TOOLBAR-1", "group-1", "button-2");
        assert_eq!(derived, "TOOLBAR-1-GROUP-1-BUTTON-2");
        let clicks: Vec<(&str, Option<&str>)> = events
            .iter()
            .filter(|e| e.event == "onClick")
            .map(|e| (e.ctrl_id.as_str(), e.value.as_deref()))
            .collect();
        assert_eq!(
            clicks,
            vec![
                ("TOOLBAR-1", Some("button-2")),
                (derived.as_str(), None),
            ],
            "the bar hears the press, then the button's own id does"
        );

        // A press that lands on nothing raises nothing â the padding between the
        // frame and the first button is not a button.
        let gap = pos2(20.0 + 1.0, 20.0 + bar_h as f32 / 2.0);
        let (none, _) = drive(
            &[ctrl("TOOLBAR-2", ControlType::ToolBar, 20, 20, bar_w, bar_h)],
            vec![
                (0.0, vec![Event::PointerMoved(gap)]),
                (0.05, vec![press(gap)]),
                (0.10, vec![release(gap)]),
            ],
        );
        assert!(
            !names(&none).contains(&"onClick"),
            "a press on the group's padding is not a button press: {:?}",
            names(&none)
        );

        println!(
            "\n  Toolbar press â clicking button-2 of a 2-button group raises onClick on \
             TOOLBAR-1 (value \"button-2\") AND on the derived id {derived}, and writes \
             LastButton; a press on the group's padding raises nothing\n"
        );
    }

    /// The Switch is clickable at its DESIGNED size, and raises the directional
    /// event for the state it moved into.
    ///
    /// It used to be drawn by the palette crate's widget, which hard-codes a
    /// 32x18 track and allocates exactly that â so a switch designed 200pt wide
    /// ran at 32pt, and only a click inside those 32pt registered. The designer
    /// and the running form disagreed about the same control.
    #[test]
    fn engine_switch_follows_its_designed_size_and_raises_toggle_events() {
        let c = [ctrlp_events(
            "Sw",
            ControlType::Switch,
            0,
            0,
            200,
            60,
            &[("Checked", "0"), ("Accent", "Blue")],
            &["onCheck", "onUncheck", "onCheckedChanged"],
        )];
        // Well outside the crate widget's 32x18 box, and inside the designed one.
        let p = pos2(160.0, 30.0);
        let (evs, overrides) = drive(
            &c,
            vec![
                (0.0, vec![]),
                (1.0, vec![Event::PointerMoved(p), press(p)]),
                (2.0, vec![Event::PointerMoved(p), release(p)]),
            ],
        );
        let n = names(&evs);
        assert!(
            n.contains(&"onCheck"),
            "switching ON must raise onCheck; got {n:?}"
        );
        assert!(
            n.contains(&"onCheckedChanged"),
            "â¦and onCheckedChanged for the move itself; got {n:?}"
        );
        assert!(
            !n.contains(&"onUncheck"),
            "â¦but not the other direction; got {n:?}"
        );
        assert_eq!(
            overrides.get("Sw").and_then(|m| m.get("Checked")).map(String::as_str),
            Some("true"),
            "the click must actually flip Checked"
        );
        println!(
            "  Switch â clicked at x=160 of a 200pt control (outside the old \
             32pt box): Checked â true, events {n:?}"
        );
    }

    #[test]
    fn engine_button_click_fires_onclick() {
        let c = [ctrlp_events(
            "Btn",
            ControlType::Button,
            0,
            0,
            80,
            30,
            &[("Caption", "OK")],
            &["onClick"],
        )];
        let p = pos2(40.0, 15.0);
        let (evs, _) = drive(
            &c,
            vec![
                (0.0, vec![]),
                (1.0, vec![Event::PointerMoved(p), press(p)]),
                (2.0, vec![Event::PointerMoved(p), release(p)]),
            ],
        );
        assert!(
            names(&evs).contains(&"onClick"),
            "Button: no onClick; got {:?}",
            names(&evs)
        );
    }

    #[test]
    fn engine_button_right_click_fires_context_events() {
        let c = [ctrlp_events(
            "Btn",
            ControlType::Button,
            0,
            0,
            80,
            30,
            &[("Caption", "OK")],
            &["onRightClick", "onContextMenu"],
        )];
        let p = pos2(40.0, 15.0);
        let (evs, _) = drive(&c, vec![(0.0, vec![]), (1.0, right_click(p))]);
        let n = names(&evs);
        assert!(
            n.contains(&"onRightClick"),
            "Button: no onRightClick; got {n:?}"
        );
        assert!(
            n.contains(&"onContextMenu"),
            "Button: no onContextMenu; got {n:?}"
        );
    }

    #[test]
    fn engine_label_hover_and_load_fire_events() {
        let c = [ctrlp_events(
            "Lbl",
            ControlType::Label,
            0,
            0,
            80,
            24,
            &[("Caption", "Name")],
            &["onLoad", "onHoverEnter", "onHoverLeave"],
        )];
        let p = pos2(40.0, 12.0);
        let (evs, _) = drive(
            &c,
            vec![
                (0.0, vec![]),
                (1.0, vec![Event::PointerMoved(p)]),
                (2.0, vec![Event::PointerMoved(p)]),
                (3.0, vec![Event::PointerMoved(p)]),
                (4.0, vec![Event::PointerMoved(p)]),
                (5.0, vec![Event::PointerMoved(p)]),
                (6.0, vec![Event::PointerMoved(p)]),
                (7.0, vec![Event::PointerMoved(pos2(200.0, 200.0))]),
            ],
        );
        let n = names(&evs);
        assert!(n.contains(&"onLoad"), "Label: no onLoad; got {n:?}");
        assert!(
            n.contains(&"onHoverEnter"),
            "Label: no onHoverEnter; got {n:?}"
        );
        assert!(
            n.contains(&"onHoverLeave"),
            "Label: no onHoverLeave; got {n:?}"
        );
    }

    #[test]
    fn engine_checkbox_toggle_changes_value() {
        let c = [ctrlp(
            "Chk",
            ControlType::CheckBox,
            0,
            0,
            140,
            24,
            &[("Value", "0")],
        )];
        let p = pos2(70.0, 12.0);
        let (evs, map) = drive(
            &c,
            vec![
                (0.0, vec![]),
                (1.0, vec![Event::PointerMoved(p), press(p)]),
                (2.0, vec![Event::PointerMoved(p), release(p)]),
            ],
        );
        let n = names(&evs);
        assert!(n.contains(&"onChange"), "CheckBox: no onChange; got {n:?}");
        assert!(
            n.contains(&"onCheckedChanged"),
            "CheckBox: no onCheckedChanged; got {n:?}"
        );
        assert!(
            n.contains(&"onValueChanged"),
            "CheckBox: no onValueChanged; got {n:?}"
        );
        assert_eq!(
            map.get("Chk")
                .and_then(|m| m.get("Value"))
                .map(String::as_str),
            Some("1")
        );
    }

    // ââ Spec 039 T3: Knob/Gauge/Switch/FileDropZone interactive render âââââ

    /// One click on a Switch emits exactly ONE `onClick`.
    ///
    /// `control_pointer_events` emits `onClick` for every control that binds a
    /// handler — that is how Button and the rest get theirs. The Switch arm
    /// pushed a second one of its own, so a Switch with a bound handler ran it
    /// TWICE per click: the operator's handler printed its DISPLAY twice, and an
    /// event trace showed two `send` lines for one press (2026-08-21).
    ///
    /// The binding is the whole point of this test. Without it the universal
    /// emitter stays quiet, only the duplicate push remained, and the count came
    /// out as one — which is exactly why an unbound reproduction looked healthy
    /// while the real form was broken.
    #[test]
    fn a_switch_click_emits_exactly_one_onclick() {
        let c = [ctrlp_events(
            "Swt",
            ControlType::Switch,
            0,
            0,
            52,
            28,
            &[("Checked", "false")],
            &["onClick"],
        )];
        let p = pos2(26.0, 14.0);
        let (evs, _map) = drive(
            &c,
            vec![
                (0.0, vec![]),
                (1.0, vec![Event::PointerMoved(p), press(p)]),
                (2.0, vec![Event::PointerMoved(p), release(p)]),
            ],
        );
        let clicks = evs
            .iter()
            .filter(|e| e.ctrl_id == "Swt" && e.event == "onClick")
            .count();
        assert_eq!(
            clicks, 1,
            "one press must emit one onClick; got {:?}",
            names(&evs)
        );
        // The toggle events are this arm's own and must survive: they are the
        // only place the new checked state is reported.
        let n = names(&evs);
        assert!(n.contains(&"onCheck"), "the toggle events stay: {n:?}");
        assert_eq!(
            n.iter().filter(|e| **e == "onCheck").count(),
            1,
            "...and they are not doubled either: {n:?}"
        );
    }

    #[test]
    fn engine_switch_click_toggles_checked() {
        // The handler must be BOUND for `onClick` to be emitted at all — that is
        // the universal rule for every control, and the Switch used to be the
        // one exception because it pushed its own. See
        // `a_switch_click_emits_exactly_one_onclick` for why that mattered.
        let c = [ctrlp_events(
            "Swt",
            ControlType::Switch,
            0,
            0,
            52,
            28,
            &[("Checked", "false")],
            &["onClick"],
        )];
        let p = pos2(26.0, 14.0);
        let (evs, map) = drive(
            &c,
            vec![
                (0.0, vec![]),
                (1.0, vec![Event::PointerMoved(p), press(p)]),
                (2.0, vec![Event::PointerMoved(p), release(p)]),
            ],
        );
        let n = names(&evs);
        assert!(n.contains(&"onClick"), "Switch: no onClick; got {n:?}");
        assert_eq!(
            map.get("Swt")
                .and_then(|m| m.get("Checked"))
                .map(String::as_str),
            Some("true"),
            "Switch: Checked did not flip to true"
        );
    }

    #[test]
    fn engine_knob_drag_changes_value() {
        let c = [ctrlp(
            "Knb",
            ControlType::Knob,
            0,
            0,
            120,
            120,
            &[
                ("Minimum", "0"),
                ("Maximum", "100"),
                ("Value", "50"),
                ("Step", "1"),
                ("ShowValue", "false"),
            ],
        )];
        // The dial is egui-elegance's own fixed-size allocation inside the
        // rect `ui.put` gave it â press near the control's top-left, where
        // the dial (no label row, since Label is unset) starts, then drag
        // up-and-right, which the widget's own documented interaction
        // (knob.rs) increases the value for, regardless of exact press
        // offset within the dial's circle.
        let start = pos2(20.0, 20.0);
        let dragged = pos2(50.0, -10.0); // up-and-right relative motion
        let (evs, map) = drive(
            &c,
            vec![
                (0.0, vec![]),
                (1.0, vec![Event::PointerMoved(start), press(start)]),
                (2.0, vec![Event::PointerMoved(dragged)]),
                (3.0, vec![Event::PointerMoved(dragged), release(dragged)]),
            ],
        );
        let n = names(&evs);
        assert!(
            n.contains(&"onChange") || n.contains(&"onValueChanged"),
            "Knob: no onChange/onValueChanged; got {n:?}"
        );
        let val: f64 = map
            .get("Knb")
            .and_then(|m| m.get("Value"))
            .and_then(|v| v.parse().ok())
            .unwrap_or(f64::NAN);
        assert!(
            val > 50.0,
            "Knob: dragging up-and-right must increase Value past 50 (got {val})"
        );
    }

    #[test]
    fn engine_gauge_ignores_click_and_drag_in_every_style() {
        for style in ["Radial", "Linear", "Donut"] {
            let c = [ctrlp(
                "Gau",
                ControlType::Gauge,
                0,
                0,
                140,
                90,
                &[
                    ("GaugeStyle", style),
                    ("Minimum", "0"),
                    ("Maximum", "100"),
                    ("Value", "50"),
                ],
            )];
            let start = pos2(70.0, 45.0);
            let dragged = pos2(120.0, 10.0);
            let (evs, map) = drive(
                &c,
                vec![
                    (0.0, vec![]),
                    (1.0, vec![Event::PointerMoved(start), press(start)]),
                    (2.0, vec![Event::PointerMoved(dragged)]),
                    (3.0, vec![Event::PointerMoved(dragged), release(dragged)]),
                ],
            );
            assert!(
                map.get("Gau").and_then(|m| m.get("Value")).is_none(),
                "Gauge ({style}): a click+drag must never write Value (R10); \
                 events: {:?}",
                names(&evs)
            );
        }
    }

    #[test]
    fn engine_maps_drag_pans_center_and_fires_bounds_changed() {
        let c = [ctrlp(
            "Map1",
            ControlType::Maps,
            0,
            0,
            320,
            240,
            &[
                ("CenterLat", "40.0"),
                ("CenterLng", "-74.0"),
                ("Zoom", "10"),
            ],
        )];
        let start = pos2(160.0, 120.0);
        let dragged = pos2(220.0, 150.0); // drag right+down
        let (evs, map) = drive(
            &c,
            vec![
                (0.0, vec![]),
                (1.0, vec![Event::PointerMoved(start), press(start)]),
                (2.0, vec![Event::PointerMoved(dragged)]),
                (3.0, vec![Event::PointerMoved(dragged), release(dragged)]),
            ],
        );
        let n = names(&evs);
        assert!(
            n.contains(&"onBoundsChanged"),
            "Maps: no onBoundsChanged after a drag; got {n:?}"
        );
        let new_lat: f64 = map
            .get("Map1")
            .and_then(|m| m.get("CenterLat"))
            .and_then(|v| v.parse().ok())
            .expect("CenterLat should have been updated");
        let new_lng: f64 = map
            .get("Map1")
            .and_then(|m| m.get("CenterLng"))
            .and_then(|v| v.parse().ok())
            .expect("CenterLng should have been updated");
        // Dragging the map to the right+down pans the VIEW right+down,
        // which means the centre coordinate itself moves the opposite way
        // (west and north) â same convention every drag-to-pan map uses.
        assert!(new_lng < -74.0, "dragging right must decrease longitude (west), got {new_lng}");
        assert!(new_lat > 40.0, "dragging down must increase latitude (north), got {new_lat}");
    }

    /// Scrolling zooms — but by **scroll distance**, not by scroll event, and
    /// it **glides** there rather than arriving in one frame.
    ///
    /// This test used to send 40 px and expect a whole level, which is the
    /// behaviour the operator reported as unusable: one level per EVENT meant a
    /// trackpad flick (dozens of events) crossed five or six levels and the map
    /// could not be aimed (2026-08-20). It now sends more than
    /// `SCROLL_PER_ZOOM` for the level it expects, and the companion test below
    /// pins the other half — that a small scroll moves nothing at all.
    ///
    /// The idle frames after the wheel event are the smooth-zoom change
    /// (2026-08-22): a whole level is a factor of two, so it is now released a
    /// slice per frame and the map is drawn between levels on the way. The
    /// destination is unchanged — one notch is still one level — so this asserts
    /// where the glide LANDS, and `map_tiles`'s own tests pin the slicing.
    #[test]
    fn engine_maps_scroll_changes_zoom_only_while_hovered() {
        let c = [ctrlp(
            "Map1",
            ControlType::Maps,
            0,
            0,
            320,
            240,
            &[
                ("CenterLat", "40.0"),
                ("CenterLng", "-74.0"),
                ("Zoom", "10"),
            ],
        )];
        let p = pos2(160.0, 120.0);
        let mut frames = vec![
            (0.0, vec![]),
            (
                1.0,
                vec![
                    Event::PointerMoved(p),
                    Event::MouseWheel {
                        unit: egui::MouseWheelUnit::Point,
                        delta: egui::vec2(0.0, crate::map_tiles::SCROLL_PER_ZOOM + 10.0),
                        modifiers: Modifiers::default(),
                        phase: egui::TouchPhase::Move,
                    },
                ],
            ),
        ];
        // Let the glide run out. The pointer has to stay over the map, since
        // that is the condition the zoom is gated on.
        for _ in 0..30 {
            frames.push((1.0, vec![Event::PointerMoved(p)]));
        }
        let (_, map) = drive(&c, frames);
        let zoom: i64 = map
            .get("Map1")
            .and_then(|m| m.get("Zoom"))
            .and_then(|v| v.parse().ok())
            .expect("Zoom should have been updated while hovered");
        assert_eq!(
            zoom, 11,
            "scrolling up while hovered must glide to exactly one level in"
        );
    }

    /// The other half of the fix: a scroll that has not travelled a whole
    /// level's worth moves nothing. Without this, one level per event is free
    /// to come back and the test above would still pass.
    #[test]
    fn engine_maps_a_small_scroll_does_not_zoom_at_all() {
        let c = [ctrlp(
            "Map1",
            ControlType::Maps,
            0,
            0,
            320,
            240,
            &[
                ("CenterLat", "40.0"),
                ("CenterLng", "-74.0"),
                ("Zoom", "10"),
            ],
        )];
        let p = pos2(160.0, 120.0);
        let (_, map) = drive(
            &c,
            vec![
                (0.0, vec![]),
                (
                    1.0,
                    vec![
                        Event::PointerMoved(p),
                        Event::MouseWheel {
                            unit: egui::MouseWheelUnit::Point,
                            delta: egui::vec2(0.0, crate::map_tiles::SCROLL_PER_ZOOM / 4.0),
                            modifiers: Modifiers::default(),
                            phase: egui::TouchPhase::Move,
                        },
                    ],
                ),
            ],
        );
        let zoom = map.get("Map1").and_then(|m| m.get("Zoom"));
        assert!(
            zoom.is_none() || zoom.map(|z| z.as_str()) == Some("10"),
            "a quarter-level scroll must leave the zoom alone, got {zoom:?}"
        );
    }

    #[test]
    fn engine_maps_marker_click_sets_selected_marker_id_and_fires_on_marker_click() {
        // A marker placed exactly at the map's centre, so a click at the
        // control's centre pixel is guaranteed to land on it regardless of
        // the projection's exact pixel math.
        let markers = "PIN-1\t40.0\t-74.0\tHQ\tHeadquarters";
        let c = [ctrlp(
            "Map1",
            ControlType::Maps,
            0,
            0,
            320,
            240,
            &[
                ("CenterLat", "40.0"),
                ("CenterLng", "-74.0"),
                ("Zoom", "10"),
                ("Markers", markers),
            ],
        )];
        let p = pos2(160.0, 120.0); // control centre == marker position
        let (evs, map) = drive(
            &c,
            vec![
                (0.0, vec![]),
                (1.0, vec![Event::PointerMoved(p), press(p)]),
                (2.0, vec![Event::PointerMoved(p), release(p)]),
            ],
        );
        let n = names(&evs);
        assert!(
            n.contains(&"onMarkerClick"),
            "Maps: clicking a marker must fire onMarkerClick; got {n:?}"
        );
        assert!(
            !n.contains(&"onMapClick"),
            "a marker hit must not ALSO fire onMapClick; got {n:?}"
        );
        assert_eq!(
            map.get("Map1")
                .and_then(|m| m.get("SelectedMarkerId"))
                .map(String::as_str),
            Some("PIN-1")
        );
    }

    #[test]
    fn engine_file_drop_zone_click_requests_a_native_picker() {
        // `drive()` discards `RenderOutput::file_picker_requests` (only
        // `events`/prop overrides matter to every other test), so this test
        // runs the render loop directly rather than extending `drive()`'s
        // signature for one caller.
        let c = [ctrlp(
            "Fdz",
            ControlType::FileDropZone,
            0,
            0,
            220,
            100,
            &[("Hint", "")],
        )];
        let ctx = egui::Context::default();
        ctx.set_fonts(egui::FontDefinitions::default());
        let active = ActiveTabs::new();
        let overrides: RefCell<Map<String, Map<String, String>>> = RefCell::new(Map::new());
        let st = MapState(&overrides);
        let p = pos2(110.0, 50.0); // centre of the 220Ã100 control
        let requests: RefCell<Vec<String>> = RefCell::new(Vec::new());

        for (i, evs) in [vec![], vec![Event::PointerMoved(p), press(p)], vec![
            Event::PointerMoved(p),
            release(p),
        ]]
        .into_iter()
        .enumerate()
        {
            let mut input = egui::RawInput::default();
            input.screen_rect = Some(Rect::from_min_size(pos2(0.0, 0.0), Vec2::new(1000.0, 800.0)));
            input.focused = true;
            input.time = Some(i as f64 * 0.05);
            input.events = evs;
            ctx.run_ui(input, |root_ui| {
                egui::CentralPanel::default()
                    .frame(egui::Frame::NONE)
                    .show_inside(root_ui, |ui| {
                        ui.set_min_size(Vec2::new(400.0, 300.0));
                        let inp = RenderInput {
                            controls: &c,
                            state: &st,
                            form_size: Vec2::new(400.0, 300.0),
                            glass: true,
                            mode: RenderMode::Interactive,
                            active_tabs: &active,
                            backdrop: Backdrop::default(),
                        };
                        let out = render_form(ui, &inp);
                        requests.borrow_mut().extend(out.file_picker_requests);
                    });
            }).textures_delta.clear();
        }
        assert!(
            requests.borrow().iter().any(|id| id == "Fdz"),
            "FileDropZone: a plain click must request a native picker; got {:?}",
            requests.borrow()
        );
    }

    #[test]
    fn engine_textbox_typing_fires_focus_change_key() {
        let c = [ctrlp(
            "Txt",
            ControlType::TextBox,
            0,
            0,
            200,
            24,
            &[("Text", "")],
        )];
        let p = pos2(100.0, 12.0);
        let (evs, _) = drive(
            &c,
            vec![
                (0.0, vec![]),
                (1.0, vec![Event::PointerMoved(p), press(p)]),
                (2.0, vec![Event::PointerMoved(p), release(p)]),
                (3.0, vec![Event::Text("Z".to_owned())]),
                (
                    4.0,
                    vec![Event::Key {
                        key: Key::Enter,
                        physical_key: None,
                        pressed: true,
                        repeat: false,
                        modifiers: Modifiers::default(),
                    }],
                ),
            ],
        );
        let n = names(&evs);
        for want in [
            "onGotFocus",
            "onEnter",
            "onChange",
            "onTextChanged",
            "onKeyPress",
            "onEnterPressed",
        ] {
            assert!(n.contains(&want), "TextBox: missing {want}; got {n:?}");
        }
    }

    fn tab_key(shift: bool, pressed: bool) -> Event {
        Event::Key {
            key: Key::Tab,
            physical_key: None,
            pressed,
            repeat: false,
            modifiers: Modifiers {
                shift,
                ..Modifiers::default()
            },
        }
    }

    fn enter_key(pressed: bool) -> Event {
        Event::Key {
            key: Key::Enter,
            physical_key: None,
            pressed,
            repeat: false,
            modifiers: Modifiers::default(),
        }
    }

    #[test]
    fn engine_cursor_property_maps_to_egui_icons() {
        assert_eq!(cursor_icon_for("Default"), None);
        assert_eq!(
            cursor_icon_for("Hand"),
            Some(egui::CursorIcon::PointingHand)
        );
        assert_eq!(cursor_icon_for("Text"), Some(egui::CursorIcon::Text));
        assert_eq!(cursor_icon_for("Wait"), Some(egui::CursorIcon::Wait));
        assert_eq!(
            cursor_icon_for("Crosshair"),
            Some(egui::CursorIcon::Crosshair)
        );
        assert_eq!(cursor_icon_for("No"), Some(egui::CursorIcon::NotAllowed));
        assert_eq!(cursor_icon_for("SizeAll"), Some(egui::CursorIcon::Move));
        assert_eq!(
            cursor_icon_for("SizeNS"),
            Some(egui::CursorIcon::ResizeVertical)
        );
        assert_eq!(
            cursor_icon_for("SizeWE"),
            Some(egui::CursorIcon::ResizeHorizontal)
        );
        assert_eq!(cursor_icon_for("Help"), Some(egui::CursorIcon::Help));
    }

    #[test]
    fn engine_tab_moves_focus_by_tab_order() {
        let mut first_visual = ctrlp(
            "VisualFirst",
            ControlType::TextBox,
            0,
            0,
            160,
            24,
            &[("Text", "")],
        );
        first_visual.tab_order = 2;
        let mut first_tab = ctrlp(
            "TabFirst",
            ControlType::TextBox,
            0,
            40,
            160,
            24,
            &[("Text", "")],
        );
        first_tab.tab_order = 1;
        let controls = [first_visual, first_tab];

        let (_evs, map) = drive(
            &controls,
            vec![
                (0.0, vec![]),
                (1.0, vec![tab_key(false, true)]),
                (2.0, vec![tab_key(false, false)]),
                (3.0, vec![Event::Text("A".to_owned())]),
                (4.0, vec![tab_key(false, true)]),
                (5.0, vec![tab_key(false, false)]),
                (6.0, vec![Event::Text("B".to_owned())]),
            ],
        );

        assert_eq!(
            map.get("TabFirst")
                .and_then(|m| m.get("Text"))
                .map(String::as_str),
            Some("A"),
            "first Tab should focus the lower TabOrder TextBox"
        );
        assert_eq!(
            map.get("VisualFirst")
                .and_then(|m| m.get("Text"))
                .map(String::as_str),
            Some("B"),
            "second Tab should advance to the next TextBox by TabOrder"
        );
    }

    #[test]
    fn engine_enter_clicks_default_button_without_input_focus() {
        let mut default_button = ctrlp_events(
            "Save",
            ControlType::Button,
            0,
            0,
            100,
            30,
            &[],
            &["onClick"],
        );
        default_button.set_prop("IsDefault".to_owned(), PropValue::Bool(true));
        let ordinary_button = ctrlp_events(
            "Cancel",
            ControlType::Button,
            120,
            0,
            100,
            30,
            &[],
            &["onClick"],
        );
        let controls = [ordinary_button, default_button];

        let (evs, _) = drive(&controls, vec![(0.0, vec![enter_key(true)])]);

        assert!(
            evs.iter()
                .any(|event| event.ctrl_id == "Save" && event.event == "onClick"),
            "Enter should click the explicit default button; got {evs:?}"
        );
        assert!(
            evs.iter()
                .all(|event| !(event.ctrl_id == "Cancel" && event.event == "onClick")),
            "Enter must not click a non-default button; got {evs:?}"
        );
    }

    #[test]
    fn engine_enter_ignores_when_no_default_button_exists() {
        let ordinary_button = ctrlp_events(
            "Cancel",
            ControlType::Button,
            0,
            0,
            100,
            30,
            &[],
            &["onClick"],
        );

        let (evs, _) = drive(&[ordinary_button], vec![(0.0, vec![enter_key(true)])]);

        assert!(
            evs.iter().all(|event| event.event != "onClick"),
            "Enter should be ignored when no default button exists; got {evs:?}"
        );
    }

    #[test]
    fn engine_slider_drag_changes_value() {
        let c = [ctrlp(
            "Sld",
            ControlType::Slider,
            0,
            0,
            200,
            30,
            &[
                ("Minimum", "0"),
                ("Maximum", "100"),
                ("Value", "50"),
                ("Step", "1"),
            ],
        )];
        let screen = Rect::from_min_size(pos2(0.0, 0.0), Vec2::new(200.0, 30.0));
        let tc = crate::paint::slider_thumb_rect(screen, 0.0, 100.0, 50.0, false).center();
        let to = tc + egui::vec2(30.0, 0.0);
        let (evs, map) = drive(
            &c,
            vec![
                (0.0, vec![]),
                (1.0, vec![Event::PointerMoved(tc), press(tc)]),
                (2.0, vec![Event::PointerMoved(to)]),
                (3.0, vec![Event::PointerMoved(to)]),
                (4.0, vec![release(to)]),
            ],
        );
        assert!(
            names(&evs).contains(&"onChange"),
            "Slider: no onChange; got {:?}",
            names(&evs)
        );
        assert!(
            names(&evs).contains(&"onValueChanged"),
            "Slider: no onValueChanged; got {:?}",
            names(&evs)
        );
        let v = map
            .get("Sld")
            .and_then(|m| m.get("Value"))
            .cloned()
            .unwrap_or_default();
        assert_ne!(
            v, "50",
            "Slider: Value did not change after drag (still {v})"
        );
    }

    #[test]
    fn engine_vertical_slider_drag_up_increases_value() {
        let c = [ctrlp(
            "Sld",
            ControlType::Slider,
            0,
            0,
            36,
            200,
            &[
                ("Minimum", "0"),
                ("Maximum", "100"),
                ("Value", "50"),
                ("Step", "1"),
                ("Orientation", "Vertical"),
            ],
        )];
        let screen = Rect::from_min_size(pos2(0.0, 0.0), Vec2::new(36.0, 200.0));
        let tc = crate::paint::slider_thumb_rect(screen, 0.0, 100.0, 50.0, true).center();
        let to = tc + egui::vec2(0.0, -40.0);
        let (_evs, map) = drive(
            &c,
            vec![
                (0.0, vec![]),
                (1.0, vec![Event::PointerMoved(tc), press(tc)]),
                (2.0, vec![Event::PointerMoved(to)]),
                (3.0, vec![Event::PointerMoved(to)]),
                (4.0, vec![release(to)]),
            ],
        );
        let v: f32 = map
            .get("Sld")
            .and_then(|m| m.get("Value"))
            .and_then(|v| v.parse().ok())
            .unwrap_or(50.0);
        assert!(
            v > 50.0,
            "Vertical slider: dragging knob upward should increase value; got {v}"
        );
    }

    #[test]
    fn engine_combobox_select_sets_value() {
        let c = [ctrlp(
            "Cmb",
            ControlType::ComboBox,
            0,
            0,
            160,
            26,
            &[("Items", "Apple\nBanana\nCherry"), ("Value", "")],
        )];
        let hc = pos2(80.0, 13.0);
        // Popup item rows start at header.max_y+1 = 27, 22px tall; Banana = index 1.
        let banana = pos2(80.0, 27.0 + 22.0 + 11.0);
        let (evs, map) = drive(
            &c,
            vec![
                (0.0, vec![]),
                (1.0, vec![Event::PointerMoved(hc), press(hc)]),
                (2.0, vec![Event::PointerMoved(hc), release(hc)]), // open
                (3.0, vec![Event::PointerMoved(banana), press(banana)]),
                (4.0, vec![Event::PointerMoved(banana), release(banana)]), // select
            ],
        );
        let n = names(&evs);
        assert!(
            n.contains(&"onDropDown"),
            "ComboBox: no onDropDown; got {n:?}"
        );
        assert!(
            n.contains(&"onSelectedIndexChanged"),
            "ComboBox: no select event; got {n:?}"
        );
        assert_eq!(
            map.get("Cmb")
                .and_then(|m| m.get("Value"))
                .map(String::as_str),
            Some("Banana")
        );
    }

    /// An open ComboBox draws two highlights â behind the SELECTED item and
    /// behind the one the pointer is over â and both are the developer's to
    /// name, exactly as a ListBox's are (operator, 2026-08-18).
    ///
    /// `ActiveItemColor` is deliberately the property a ListBox already
    /// carries: on both controls it colours the item `Value`/`SelectedIndex`
    /// reports. The second one is NOT the list's `SelectedItemsColor` â a
    /// ComboBox selects one item or none, so that has nothing to colour here â
    /// but the hover was hardcoded in the same way, and leaving it so would
    /// mean an orange selection still flashing blue under the pointer.
    ///
    /// Unnamed, both fall back to the constants the popup always painted, NOT
    /// to the palette: these two were never theme-derived, so that is what
    /// "unchanged" means for a ComboBox already designed.

    /// Whether a painted rect covers `p`, honouring its **effective** corner
    /// radius â egui clamps each corner to half the shorter side, so the stored
    /// radius lies about what was actually drawn (corner-bleed playbook Â§1.1).
    fn band_covers(rect: Rect, cr: egui::CornerRadius, p: egui::Pos2) -> bool {
        if !rect.contains(p) {
            return false;
        }
        let cap = (rect.width() * 0.5).min(rect.height() * 0.5);
        let eff = |v: u8| (v as f32).min(cap);
        let (r, cx, cy) = if p.x < rect.center().x && p.y < rect.center().y {
            (eff(cr.nw), rect.left(), rect.top())
        } else if p.x >= rect.center().x && p.y < rect.center().y {
            (eff(cr.ne), rect.right(), rect.top())
        } else if p.x < rect.center().x {
            (eff(cr.sw), rect.left(), rect.bottom())
        } else {
            (eff(cr.se), rect.right(), rect.bottom())
        };
        if r <= 0.0 {
            return true;
        }
        // The arc's centre, pulled `r` in from that corner on both axes.
        let ax = if cx == rect.left() { cx + r } else { cx - r };
        let ay = if cy == rect.top() { cy + r } else { cy - r };
        // Outside the corner square â the straight part of the edge covers it.
        if (p.x - ax) * (cx - ax) <= 0.0 || (p.y - ay) * (cy - ay) <= 0.0 {
            return true;
        }
        (p.x - ax).powi(2) + (p.y - ay).powi(2) <= r * r
    }

    /// `Sorted` sorts. It is seeded on every list-shaped control and shown in
    /// the inspector, and until now nothing anywhere read it (operator,
    /// 2026-08-18: "sort is not working").
    ///
    /// Alphabetical, by text, case-insensitively — so the operator's numeric
    /// items sort as the strings they are. And the DISPLAY order only: the
    /// stored `Items` is what the developer typed, untouched, so turning the
    /// property off gives their list straight back.
    #[test]
    fn sorted_orders_what_a_list_shows_without_rewriting_what_was_typed() {
        // The operator's own items, from PowerDemo3/inner-form2.cfrm.
        const TYPED: &str = "6\n1\n2\n3\n4\n5\n11\n7\n8\n9\n10";
        let sorted_texts = |ctrl: &Control, open: bool| -> Vec<String> {
            let frames = if open {
                let hc = pos2(
                    ctrl.rect.x as f32 + ctrl.rect.w as f32 * 0.5,
                    ctrl.rect.y as f32 + ctrl.rect.h as f32 * 0.5,
                );
                vec![
                    (0.0, vec![]),
                    (1.0, vec![Event::PointerMoved(hc), press(hc)]),
                    (2.0, vec![Event::PointerMoved(hc), release(hc)]),
                    (3.0, vec![]),
                ]
            } else {
                vec![(0.0, vec![]), (0.05, vec![])]
            };
            let painted = drive_painted(std::slice::from_ref(ctrl), frames);
            let mut seen: Vec<(f32, String)> = painted
                .texts
                .iter()
                .filter(|t| TYPED.lines().any(|l| l == t.text))
                .map(|t| (t.pos.y, t.text.clone()))
                .collect();
            seen.sort_by(|a, b| a.0.total_cmp(&b.0));
            seen.dedup_by(|a, b| a.1 == b.1);
            seen.into_iter().map(|(_, t)| t).collect()
        };

        // ── ListBox: tall enough to show all eleven at once ───────────────
        let mut lb = ctrl("ListBox-1", ControlType::ListBox, 20, 20, 220, 300);
        lb.set_prop("Items", crate::PropValue::String(TYPED.to_owned()));
        lb.set_prop("Sorted", crate::PropValue::Bool(false));
        assert_eq!(
            sorted_texts(&lb, false),
            TYPED.lines().collect::<Vec<_>>(),
            "Sorted off must leave the developer's own order alone"
        );

        lb.set_prop("Sorted", crate::PropValue::Bool(true));
        assert_eq!(
            sorted_texts(&lb, false),
            vec!["1", "10", "11", "2", "3", "4", "5", "6", "7", "8", "9"],
            "Sorted on orders the list alphabetically — items are TEXT, so 10 \
             sorts before 9"
        );
        assert_eq!(
            lb.get_prop("Items").map(|v| v.as_str().to_owned()),
            Some(TYPED.to_owned()),
            "…and the stored Items is never rewritten"
        );

        // ── ComboBox: the same, in the open dropdown ──────────────────────
        let mut cmb = ctrl("ComboBox-1", ControlType::ComboBox, 20, 20, 220, 24);
        cmb.set_prop("Items", crate::PropValue::String(TYPED.to_owned()));
        cmb.set_prop("DropDownHeight", crate::PropValue::Int(600));
        cmb.set_prop("Sorted", crate::PropValue::Bool(true));
        assert_eq!(
            sorted_texts(&cmb, true),
            vec!["1", "10", "11", "2", "3", "4", "5", "6", "7", "8", "9"],
            "a dropdown sorts on the same rule as a list"
        );

        // Case is ignored, and a tie keeps the typed order.
        let mut mixed = ctrl("ListBox-2", ControlType::ListBox, 20, 20, 220, 300);
        mixed.set_prop(
            "Items",
            crate::PropValue::String("delta\nAlpha\nbravo\nCharlie".to_owned()),
        );
        mixed.set_prop("Sorted", crate::PropValue::Bool(true));
        let painted = drive_painted(&[mixed], vec![(0.0, vec![]), (0.05, vec![])]);
        let mut seen: Vec<(f32, String)> = painted
            .texts
            .iter()
            .filter(|t| ["delta", "Alpha", "bravo", "Charlie"].contains(&t.text.as_str()))
            .map(|t| (t.pos.y, t.text.clone()))
            .collect();
        seen.sort_by(|a, b| a.0.total_cmp(&b.0));
        seen.dedup_by(|a, b| a.1 == b.1);
        assert_eq!(
            seen.into_iter().map(|(_, t)| t).collect::<Vec<_>>(),
            vec!["Alpha", "bravo", "Charlie", "delta"],
            "case is ignored, so a capital does not sort ahead of every lower-case word"
        );

        println!(
            "\n  Sorted — the operator's 6/1/2/3/4/5/11/7/8/9/10 shows as \
             1,10,11,2,…,9 in both a ListBox and an open dropdown (text order, so 10 \
             before 9), Items itself untouched; delta/Alpha/bravo/Charlie ⇒ \
             Alpha,bravo,Charlie,delta\n"
        );
    }

    /// A dropdown's scrollbar belongs INSIDE its border, as a ListBox's does.
    /// It used to ride on the rim and out past the rounded corner, because the
    /// scrolling pane was handed the whole panel instead of the inside of it
    /// (operator, 2026-08-18).
    ///
    /// And a list short enough to fit must not scroll for want of that margin:
    /// the panel stands tall enough for its items AND the margin.
    #[test]
    fn a_dropdowns_scrollbar_sits_inside_the_border() {
        let items: Vec<String> = (1..=30).map(|n| format!("Item-{n:02}")).collect();
        let mut cmb = ctrl("Cmb", ControlType::ComboBox, 20, 20, 320, 26);
        cmb.set_prop("Items", crate::PropValue::String(items.join("\n")));
        cmb.set_prop("Value", crate::PropValue::String("Item-01".to_owned()));
        let hc = pos2(180.0, 33.0);
        let painted = drive_painted(
            &[cmb.clone()],
            vec![
                (0.0, vec![]),
                (1.0, vec![Event::PointerMoved(hc), press(hc)]),
                (2.0, vec![Event::PointerMoved(hc), release(hc)]),
                (3.0, vec![]),
            ],
        );
        let header = *painted.placed.get("Cmb").expect("placed");
        let pad = crate::model::LIST_FRAME_PAD;
        let panel = Rect::from_min_size(
            pos2(header.left(), header.bottom() + 1.0),
            Vec2::new(header.width(), 200.0),
        );

        // The scrollbar is a tall, narrow rect inside the panel. Its width and
        // its ink depend on the ambient scroll style, and egui fades an idle bar
        // to nothing, so only its POSITION is asserted -- exactly as the
        // ListBox's own scrollbar guard does.
        let bars: Vec<Rect> = painted
            .bands
            .iter()
            .map(|(r, _, _)| *r)
            .filter(|r| r.height() > r.width() * 3.0 && r.height() > 20.0)
            .filter(|r| r.height() <= panel.height() + 1.0)
            .filter(|r| r.top() >= panel.top() - 1.0 && r.bottom() <= panel.bottom() + 1.0)
            .filter(|r| r.left() >= panel.left() - 1.0 && r.right() <= panel.right() + 1.0)
            .collect();
        assert!(
            !bars.is_empty(),
            "a 30-item list in a {}px panel must have a scrollbar to place",
            panel.height()
        );
        for bar in &bars {
            assert!(
                bar.right() <= panel.right() - pad + 0.5,
                "the scrollbar must sit inside the border, not on it: bar {bar:?} in {panel:?}"
            );
            assert!(
                bar.center().x >= panel.right() - 14.0,
                "...while still hugging the right border: bar {bar:?} in {panel:?}"
            );
        }

        // A list that FITS must not scroll: three items, in a panel tall enough
        // for them plus the margin, all three painted where they were laid out.
        let mut short = ctrl("Short", ControlType::ComboBox, 20, 200, 320, 26);
        short.set_prop(
            "Items",
            crate::PropValue::String("Alpha\nBeta\nGamma".to_owned()),
        );
        let sc = pos2(180.0, 213.0);
        let painted = drive_painted(
            &[short.clone()],
            vec![
                (0.0, vec![]),
                (1.0, vec![Event::PointerMoved(sc), press(sc)]),
                (2.0, vec![Event::PointerMoved(sc), release(sc)]),
                (3.0, vec![]),
            ],
        );
        let head = *painted.placed.get("Short").expect("placed");
        let ih = combo_item_h(&short);
        let box_ = Rect::from_min_size(
            pos2(head.left(), head.bottom() + 1.0),
            Vec2::new(head.width(), 3.0 * ih + pad * 2.0),
        );
        for want in ["Alpha", "Beta", "Gamma"] {
            assert!(
                painted
                    .texts
                    .iter()
                    .any(|t| t.text == want && box_.expand(1.0).contains_rect(t.ink)),
                "a three-item list must show all three without scrolling; {want} is not \
                 inside {box_:?}: painted {:?}",
                painted
                    .texts
                    .iter()
                    .map(|t| (t.text.as_str(), t.ink))
                    .collect::<Vec<_>>()
            );
        }

        println!(
            "\n  ComboBox scrollbar -- a 30-item list: {} bar rect(s), rightmost edge {:.1} \
             inside a panel ending at {:.1} ({pad}px margin, as a ListBox keeps); a \
             three-item list shows all three without scrolling\n",
            bars.len(),
            bars.iter().map(|b| b.right()).fold(f32::MIN, f32::max),
            panel.right()
        );
    }

    /// A Shape wears the background GRADIENT it was designed with, on every one

    /// The designer canvas shows a ComboBox the way the running one shows it:
    /// the chosen Value, or the first item in the order the list DISPLAYS them.
    ///
    /// The canvas lettered the first item as TYPED, so a combo with Sorted on
    /// read one thing while designing and another the moment the form ran --
    /// which is how a working sort looked broken (operator, 2026-08-18).
    #[test]
    fn the_canvas_shows_a_combobox_the_way_the_running_one_does() {
        let combo = |sorted: bool, value: &str| -> Control {
            let mut c = ctrl("Cmb", ControlType::ComboBox, 20, 20, 200, 26);
            c.set_prop("Items", crate::PropValue::String("6\n1\n2\n11\n10".into()));
            c.set_prop("Sorted", crate::PropValue::Bool(sorted));
            if !value.is_empty() {
                c.set_prop("Value", crate::PropValue::String(value.to_owned()));
            }
            c
        };
        let canvas_says = |c: &Control| -> String {
            painted_text(std::slice::from_ref(c), "#101010")
                .into_iter()
                .map(|(t, _)| t)
                .find(|t| t.contains('\u{25BE}'))
                .unwrap_or_default()
        };
        let running_says = |c: &Control| -> String {
            let painted = drive_painted(std::slice::from_ref(c), vec![(0.0, vec![]), (0.05, vec![])]);
            let frame = *painted.placed.get("Cmb").expect("placed");
            painted
                .texts
                .iter()
                .filter(|t| frame.expand(1.0).contains_rect(t.ink))
                .map(|t| t.text.clone())
                .find(|t| !t.is_empty() && t != "\u{25BC}" && t != "\u{25B2}")
                .unwrap_or_default()
        };

        // Unsorted, nothing chosen: both show the first item as typed.
        let c = combo(false, "");
        assert_eq!(canvas_says(&c), "6 \u{25BE}");
        assert_eq!(running_says(&c), "6");

        // Sorted, nothing chosen: both show the first item as DISPLAYED.
        let c = combo(true, "");
        assert_eq!(
            canvas_says(&c),
            "1 \u{25BE}",
            "the canvas must letter the first SORTED item, as the running header does"
        );
        assert_eq!(running_says(&c), "1");

        // A chosen value wins on both.
        let c = combo(true, "11");
        assert_eq!(
            canvas_says(&c),
            "11 \u{25BE}",
            "the canvas must show the chosen Value, which it ignored entirely"
        );
        assert_eq!(running_says(&c), "11");

        println!(
            "\n  ComboBox canvas -- unsorted shows 6 on both surfaces, Sorted shows 1 on \
             both, and a Value of 11 shows 11 on both; the canvas used to letter the first \
             TYPED item and ignore Value\n"
        );
    }

    /// The four TextBox input properties are honoured. All were seeded, shown in
    /// the inspector and documented, and read by NOTHING (operator, 2026-08-18,
    /// from the dead-property audit): a field marked read-only took edits, a
    /// password field showed the password, a length limit let anything through,
    /// and the scrollbars setting did nothing.
    #[test]
    fn a_textbox_honours_readonly_password_length_and_scrollbars() {
        let field = |props: &[(&str, &str)]| -> Vec<Control> {
            let mut base: Vec<(&str, &str)> = vec![("Text", "Secret")];
            base.extend_from_slice(props);
            vec![ctrlp("Tb", ControlType::TextBox, 20, 20, 240, 30, &base)]
        };
        let click = pos2(100.0, 35.0);
        let typed = |s: &str| Event::Text(s.to_owned());
        let run = |controls: &[Control]| -> (Map<String, Map<String, String>>, Vec<PaintedText>) {
            let painted = drive_painted(
                controls,
                vec![
                    (0.00, vec![Event::PointerMoved(click)]),
                    (0.05, vec![press(click)]),
                    (0.10, vec![release(click)]),
                    (0.15, vec![typed("XYZ")]),
                    (0.20, vec![]),
                ],
            );
            (painted.overrides, painted.texts)
        };
        let text_of = |o: &Map<String, Map<String, String>>| {
            o.get("Tb").and_then(|p| p.get("Text")).cloned()
        };

        // ── Baseline: an ordinary field takes the typing ──────────────────
        let (o, _) = run(&field(&[]));
        assert_eq!(
            text_of(&o).as_deref(),
            Some("SecretXYZ"),
            "an ordinary field must still accept typing"
        );

        // ── ReadOnly: no edit reaches the value ───────────────────────────
        let (o, painted) = run(&field(&[("ReadOnly", "true")]));
        assert_eq!(
            text_of(&o),
            None,
            "a read-only field must take no edits at all"
        );
        assert!(
            painted.iter().any(|t| t.text == "Secret"),
            "...while still SHOWING its value (read-only, not disabled): {:?}",
            painted.iter().map(|t| t.text.as_str()).collect::<Vec<_>>()
        );

        // ── MaximumLength: the limit is enforced ──────────────────────────
        let (o, _) = run(&field(&[("MaximumLength", "8")]));
        assert_eq!(
            text_of(&o).as_deref(),
            Some("SecretXY"),
            "typing must stop at MaximumLength characters"
        );

        // ── PasswordCharacter: the value is masked WITH THAT CHARACTER ────
        let (o, painted) = run(&field(&[("PasswordCharacter", "*")]));
        assert_eq!(
            text_of(&o).as_deref(),
            Some("SecretXYZ"),
            "masking must not alter the value: the field still holds what was typed"
        );
        assert!(
            !painted.iter().any(|t| t.text.contains("Secret")),
            "the password must not be painted in clear: {:?}",
            painted.iter().map(|t| t.text.as_str()).collect::<Vec<_>>()
        );
        assert!(
            painted.iter().any(|t| t.text.chars().all(|c| c == '*') && t.text.len() == 9),
            "...it must be painted as nine asterisks, the character chosen: {:?}",
            painted.iter().map(|t| t.text.as_str()).collect::<Vec<_>>()
        );

        // A different character is honoured too -- egui's own password mode
        // could only ever draw its fixed bullet.
        let (_, painted) = run(&field(&[("PasswordCharacter", "#")]));
        assert!(
            painted.iter().any(|t| t.text.chars().all(|c| c == '#') && t.text.len() == 9),
            "the chosen '#' must be the mask: {:?}",
            painted.iter().map(|t| t.text.as_str()).collect::<Vec<_>>()
        );
        // -- ScrollBars: which bars an overflowing multiline box shows -------
        //
        // `None` still SCROLLS, it just draws no bars: content the box cannot
        // show must never become unreachable.
        let tall = |bars: &str| -> Vec<Control> {
            vec![ctrlp(
                "Tb",
                ControlType::TextBox,
                20,
                20,
                240,
                60,
                &[
                    ("Text", "one\ntwo\nthree\nfour\nfive\nsix\nseven\neight"),
                    ("Multiline", "true"),
                    ("ScrollBars", bars),
                ],
            )]
        };
        let bar_count = |bars: &str| -> usize {
            let painted = drive_painted(&tall(bars), vec![(0.0, vec![]), (0.05, vec![])]);
            let frame = *painted.placed.get("Tb").expect("placed");
            painted
                .bands
                .iter()
                .filter(|(r, _, _)| r.height() > r.width() * 2.0 && r.height() > 8.0)
                .filter(|(r, _, _)| frame.expand(2.0).contains_rect(*r))
                .count()
        };
        assert_eq!(
            bar_count("None"),
            0,
            "ScrollBars None must draw no bar at all"
        );
        assert!(
            bar_count("Vertical") > 0,
            "ScrollBars Vertical must draw one"
        );


        println!(
            "\n  TextBox inputs -- ordinary field takes SecretXYZ; ReadOnly keeps Secret and \
             still shows it; MaximumLength 8 stops at SecretXY; PasswordCharacter paints \
             ********* and ######### while the value stays SecretXYZ\n"
        );
    }

    /// A chart honours its own visual properties. All were seeded, shown in the
    /// inspector and documented, and read by NOTHING (operator, 2026-08-18,
    /// from the dead-property audit).
    #[test]
    fn a_chart_honours_its_axis_captions_labels_and_legend() {
        let chart = |kind: ControlType, props: &[(&str, &str)]| -> Vec<Control> {
            let mut base: Vec<(&str, &str)> =
                vec![("__ChartData", "Alpha\t30\nBeta\t20\nGamma\t50")];
            base.extend_from_slice(props);
            vec![ctrlp("Ch", kind, 20, 20, 320, 240, &base)]
        };
        let texts = |c: &[Control]| -> Vec<String> {
            painted_text_interactive(c)
                .into_iter()
                .map(|(t, _)| t)
                .collect()
        };

        // -- XAxisLabel / YAxisLabel: free-text captions --------------------
        let painted = texts(&chart(
            ControlType::BarChart,
            &[("XAxisLabel", "Quarter"), ("YAxisLabel", "Revenue")],
        ));
        assert!(
            painted.iter().any(|t| t == "Quarter"),
            "the X axis caption must be drawn: {painted:?}"
        );
        assert!(
            painted.iter().any(|t| t == "Revenue"),
            "the Y axis caption must be drawn: {painted:?}"
        );
        // Empty means no caption, and no space taken for one.
        let bare = texts(&chart(ControlType::BarChart, &[("ShowLegend", "false")]));
        assert!(
            !bare.iter().any(|t| t == "Quarter" || t == "Revenue"),
            "an unset caption must draw nothing: {bare:?}"
        );

        // -- ShowLegend ------------------------------------------------------
        let with = texts(&chart(ControlType::BarChart, &[("ShowLegend", "true")]));
        assert!(
            with.iter().any(|t| t.starts_with("Series")),
            "a category chart's legend names its series: {with:?}"
        );
        assert!(
            !bare.iter().any(|t| t.starts_with("Series")),
            "...and unticking it draws none: {bare:?}"
        );
        // A pie's legend names its SLICES.
        let pie = texts(&chart(
            ControlType::PieChart,
            &[("ShowLegend", "true"), ("ShowLabels", "false")],
        ));
        for want in ["Alpha", "Beta", "Gamma"] {
            assert!(
                pie.iter().any(|t| t == want),
                "a pie's legend must name slice {want}: {pie:?}"
            );
        }

        // -- ShowLabels + LabelFormat: percent | value | label --------------
        let pct = texts(&chart(
            ControlType::PieChart,
            &[("ShowLabels", "true"), ("LabelFormat", "percent"), ("ShowLegend", "false")],
        ));
        assert!(
            pct.iter().any(|t| t == "30%") && pct.iter().any(|t| t == "50%"),
            "percent labels must show each slice's share: {pct:?}"
        );
        let val = texts(&chart(
            ControlType::PieChart,
            &[("ShowLabels", "true"), ("LabelFormat", "value"), ("ShowLegend", "false")],
        ));
        assert!(
            val.iter().any(|t| t == "30") && val.iter().any(|t| t == "50"),
            "value labels must show the value itself: {val:?}"
        );
        let lbl = texts(&chart(
            ControlType::DonutChart,
            &[("ShowLabels", "true"), ("LabelFormat", "label"), ("ShowLegend", "false")],
        ));
        assert!(
            lbl.iter().any(|t| t == "Alpha") && lbl.iter().any(|t| t == "Gamma"),
            "label labels must show the slice name, on a donut too: {lbl:?}"
        );
        let off = texts(&chart(
            ControlType::PieChart,
            &[("ShowLabels", "false"), ("ShowLegend", "false")],
        ));
        assert!(
            !off.iter().any(|t| t == "30%" || t == "30"),
            "unticked, a slice carries no label: {off:?}"
        );

        println!(
            "\n  Chart properties -- XAxisLabel/YAxisLabel draw their captions; ShowLegend \
             names Series on a bar chart and Alpha/Beta/Gamma on a pie; LabelFormat draws \
             30%/50%, then 30/50, then Alpha/Gamma; each off draws nothing\n"
        );
    }
    /// of its silhouettes (operator, 2026-08-18: "Shape background's color
    /// works, but background gradient does not").
    ///
    /// A Shape paints its own face and returns long before the generic frame
    /// code, which is the only place the gradient was ever read -- so Background
    /// colour worked while Background gradient did nothing whatever.
    #[test]
    fn a_shape_wears_the_background_gradient_it_was_designed_with() {
        let shape = |kind: &str, gradient: bool| -> Vec<Control> {
            let mut c = ctrl("Shape-1", ControlType::Shape, 20, 20, 160, 160);
            c.set_prop("ShapeType", crate::PropValue::String(kind.to_owned()));
            c.set_prop("FillColor", crate::PropValue::String("#C0C0C0FF".into()));
            // The operator's own pair: a blue start and a red end, going south.
            c.set_prop(
                "BackgroundGradientEnabled",
                crate::PropValue::Bool(gradient),
            );
            c.set_prop(
                "BackgroundGradientStartColor",
                crate::PropValue::String("#1367C4FF".into()),
            );
            c.set_prop(
                "BackgroundGradientEndColor",
                crate::PropValue::String("#EF0000FF".into()),
            );
            c.set_prop(
                "BackgroundGradientDirection",
                crate::PropValue::String("South".into()),
            );
            vec![c]
        };

        // The designed pair. A mesh "carries" a colour when some vertex is
        // within a small distance of it: the fan interpolates, so the exact
        // start and end land only at the extreme vertices.
        let start = Color32::from_rgb(0x13, 0x67, 0xC4);
        let end = Color32::from_rgb(0xEF, 0x00, 0x00);
        let near = |a: Color32, b: Color32| {
            (a.r() as i32 - b.r() as i32).abs()
                + (a.g() as i32 - b.g() as i32).abs()
                + (a.b() as i32 - b.b() as i32).abs()
                <= 24
        };

        for kind in ["Rectangle", "Circle", "Triangle"] {
            // Off: no mesh anywhere carries the designed pair. (A circle's glass
            // lens is a mesh too, so the question is the COLOURS, not the mesh.)
            let painted = drive_painted(&shape(kind, false), vec![(0.0, vec![]), (0.05, vec![])]);
            assert!(
                !painted.mesh_colors.iter().any(|(_, cs)| {
                    cs.iter().any(|c| near(*c, start)) && cs.iter().any(|c| near(*c, end))
                }),
                "{kind}: with the gradient off, nothing should paint the designed pair"
            );

            // On: one mesh carries both ends, across the shape.
            let painted = drive_painted(&shape(kind, true), vec![(0.0, vec![]), (0.05, vec![])]);
            let frame = *painted.placed.get("Shape-1").expect("placed");
            let found = painted.mesh_colors.iter().find(|(_, cs)| {
                cs.iter().any(|c| near(*c, start)) && cs.iter().any(|c| near(*c, end))
            });
            let (bounds, _) = found.unwrap_or_else(|| {
                panic!(
                    "{kind}: the designed #1367C4 to #EF0000 gradient must be painted; \
                     meshes carried {:?}",
                    painted
                        .mesh_colors
                        .iter()
                        .map(|(r, cs)| (*r, cs.len()))
                        .collect::<Vec<_>>()
                )
            });
            // A circle and a triangle cover less of their bounding box than a
            // rectangle, so the bar is what each silhouette can reach.
            let want = match kind {
                "Rectangle" => 0.9,
                "Circle" => 0.7,
                _ => 0.4,
            };
            assert!(
                bounds.intersect(frame).area() >= frame.area() * want,
                "{kind}: the gradient must cover the shape, not a corner of it: \
                 {bounds:?} vs {frame:?}"
            );
            assert!(
                bounds.top() >= frame.top() - 1.0 && bounds.bottom() <= frame.bottom() + 1.0,
                "{kind}: and must stay within it: {bounds:?} vs {frame:?}"
            );
        }

        println!(
            "\n  Shape gradient -- a designed #1367C4 to #EF0000 South gradient is painted \
             across a Rectangle, a Circle and a Triangle; with the property off no mesh \
             carries that pair at all\n"
        );
    }

    /// An open dropdown's selection band is cut by the panel's own arc, exactly
    /// as a ListBox row is (operator, 2026-08-18, with screenshots).
    ///
    /// The band was a flat 4 px round clipped to the panel rather than to the
    /// inside of its rim, so on a panel rounded any further than that it leaked
    /// out of BOTH ends: square shoulders poking past the arc at the top item
    /// and at the bottom one, and the highlight painted over the border instead
    /// of leaving the hairline a list leaves.
    ///
    /// Built from the operator's own ComboBox-1 (PowerDemo3/inner-form2.cfrm):
    /// 160Ã24 at CornerRadius 15 â clamped to 12 by the control's own height â
    /// eleven items, DropDownHeight 200, so the panel overflows and the selected
    /// item sits against the BOTTOM arc while another sits against the top.
    #[test]
    fn a_dropdowns_selection_band_is_cut_by_the_panels_corner() {
        let mut cmb = ctrl("ComboBox-1", ControlType::ComboBox, 336, 104, 160, 24);
        for (k, v) in [
            ("Items", "6\n1\n2\n3\n4\n5\n11\n7\n8\n9\n10"),
            ("Value", "10"),
            ("FontSize", "14"),
            ("DropDownHeight", "200"),
            ("CornerRadius", "15"),
            ("BackgroundColor", "#36383EFF"),
        ] {
            cmb.set_prop(k, crate::PropValue::String(v.to_owned()));
        }
        let radius = crate::paint::corner_radius(&cmb);
        assert!(
            (radius - 12.0).abs() < 0.01,
            "the fixture must reproduce the operator's clamped radius, got {radius}"
        );

        let hc = pos2(336.0 + 80.0, 104.0 + 12.0);
        let painted = drive_painted(
            &[cmb.clone()],
            vec![
                (0.0, vec![]),
                (1.0, vec![Event::PointerMoved(hc), press(hc)]),
                (2.0, vec![Event::PointerMoved(hc), release(hc)]),
                (3.0, vec![]),
                (4.0, vec![]),
            ],
        );
        let header = *painted.placed.get("ComboBox-1").expect("placed");
        let item_h = combo_item_h(&cmb);
        let panel = Rect::from_min_size(
            pos2(header.left(), header.bottom() + 1.0),
            Vec2::new(header.width(), (11.0 * item_h).min(200.0)),
        );

        // The highlights: the two fills the dropdown draws behind an item.
        let highlights: Vec<(Rect, egui::CornerRadius, Color32)> = painted
            .bands
            .iter()
            .filter(|(_, _, c)| {
                *c == crate::paint::COMBO_SELECTED_FILL || *c == crate::paint::COMBO_HOVER_FILL
            })
            .filter(|(r, _, _)| r.intersect(panel).is_positive())
            .copied()
            .collect();
        assert!(
            !highlights.is_empty(),
            "the selected item must be highlighted inside the open panel: {:?}",
            painted.bands.iter().map(|(r, _, c)| (*r, *c)).collect::<Vec<_>>()
        );

        // ââ No band may paint OUTSIDE the panel's arc âââââââââââââââââââââ
        //
        // Walk each of the four corner arcs by angle and probe just beyond it,
        // still inside the panel's bounding box: that is the notch the border
        // curves across, and nothing may reach into it.
        let mut leaks: Vec<(egui::Pos2, Rect, egui::CornerRadius)> = Vec::new();
        for (cx, cy, sx, sy) in [
            (panel.left(), panel.top(), -1.0_f32, -1.0_f32),
            (panel.right(), panel.top(), 1.0, -1.0),
            (panel.left(), panel.bottom(), -1.0, 1.0),
            (panel.right(), panel.bottom(), 1.0, 1.0),
        ] {
            let ax = cx + sx * -radius;
            let ay = cy + sy * -radius;
            for step in 0..=24 {
                let t = std::f32::consts::FRAC_PI_2 * (step as f32 / 24.0);
                for beyond in [0.6_f32, 1.5, 3.0] {
                    let d = radius + beyond;
                    let p = pos2(ax + sx * d * t.cos(), ay + sy * d * t.sin());
                    if !panel.contains(p) {
                        continue;
                    }
                    for (r, cr, _) in &highlights {
                        if band_covers(*r, *cr, p) {
                            leaks.push((p, *r, *cr));
                        }
                    }
                }
            }
        }
        assert!(
            leaks.is_empty(),
            "a selection band leaked past the panel's arc at {} point(s); first: {:?}",
            leaks.len(),
            leaks.first()
        );

        // ââ â¦and it is not squared off or shrunk away either ââââââââââââââ
        //
        // The band against an arc carries a real radius, and stops short of the
        // rim on every side so the border stays an unbroken line with a hairline
        // of panel showing â which is what a ListBox does and what this did not.
        let inset = 1.0 + crate::paint::HIGHLIGHT_INSET; // BorderWidth 1 + hairline
        let at_arc: Vec<&(Rect, egui::CornerRadius, Color32)> = highlights
            .iter()
            .filter(|(r, _, _)| {
                r.top() <= panel.top() + inset + 0.5 || r.bottom() >= panel.bottom() - inset - 0.5
            })
            .collect();
        assert!(
            !at_arc.is_empty(),
            "the fixture must put a highlight against an arc: {highlights:?}"
        );
        for (r, cr, _) in &at_arc {
            assert!(
                cr.nw > 0 || cr.ne > 0 || cr.sw > 0 || cr.se > 0,
                "a band against the arc must be rounded, not squared off: {r:?} {cr:?}"
            );
            assert!(
                r.left() > panel.left() + 0.5 && r.right() < panel.right() - 0.5,
                "the band must leave the rim its hairline: {r:?} in {panel:?}"
            );
            assert!(
                r.top() > panel.top() + 0.5 && r.bottom() < panel.bottom() - 0.5,
                "â¦at the top and bottom too: {r:?} in {panel:?}"
            );
        }

        println!(
            "\n  ComboBox corners â the operator's 11-item combo at radius {radius}: {} \
             highlight band(s), {} against an arc, none reaching past it, each inside the \
             rim by {inset}px\n",
            highlights.len(),
            at_arc.len()
        );
    }
    /// One item of an open dropdown: a line of the control's own text plus the
    /// same air a ListBox row gets. The popup used to hardcode 22 px.
    fn combo_item_h(ctrl: &Control) -> f32 {
        crate::model::text_line_height(ctrl) + crate::model::LIST_ROW_PAD * 2.0
    }

    /// The centre of item `n` of an open dropdown whose header is `header`.
    fn combo_item_at(header: Rect, item_h: f32, n: usize) -> egui::Pos2 {
        pos2(
            header.center().x,
            header.bottom() + 1.0 + item_h * (n as f32 + 0.5),
        )
    }

    #[test]
    fn an_open_combobox_draws_the_item_colours_it_was_given() {
        // Apple is the SELECTED item (row 0); the pointer rests on Cherry
        // (row 2), so one frame carries both highlights at once.
        let combo = |extra: &[(&str, &str)]| -> Vec<Control> {
            let mut props: Vec<(&str, &str)> =
                vec![("Items", "Apple\nBanana\nCherry"), ("Value", "Apple")];
            props.extend_from_slice(extra);
            vec![ctrlp("Cmb", ControlType::ComboBox, 0, 0, 160, 26, &props)]
        };
        // An item is one line of the control's OWN text plus air â the same
        // measure a ListBox row uses â and the items start one pixel below the
        // header, which is what tells an item band apart from the panel's face.
        let item_h = combo_item_h(&combo(&[])[0]);
        let hc = pos2(80.0, 13.0);
        let cherry = pos2(80.0, 26.0 + 1.0 + item_h * 2.0 + item_h * 0.5);
        let two_bands = |controls: &[Control]| -> (Color32, Color32) {
            let painted = drive_painted(
                controls,
                vec![
                    (0.0, vec![]),
                    (1.0, vec![Event::PointerMoved(hc), press(hc)]),
                    (2.0, vec![Event::PointerMoved(hc), release(hc)]), // open
                    (3.0, vec![Event::PointerMoved(cherry)]),          // hover Cherry
                ],
            );
            let header = *painted.placed.get("Cmb").expect("placed");
            // The band stops short of the panel's rim on every side, so the
            // border stays an unbroken line: its width is the panel's less the
            // inset twice, and the bands against the top and bottom arcs are
            // clipped by that inset too â so a band is at most one item tall.
            let inset = 1.0 + crate::paint::HIGHLIGHT_INSET;
            let mut rows: Vec<(Rect, Color32)> = painted
                .fills
                .iter()
                .filter(|(r, _)| (r.width() - (header.width() - inset * 2.0)).abs() <= 0.5)
                .filter(|(r, _)| r.height() <= item_h + 0.5 && r.height() >= item_h - inset - 0.5)
                .filter(|(r, _)| r.top() >= header.bottom())
                .map(|(r, c)| (*r, *c))
                .collect();
            rows.sort_by(|a, b| a.0.top().total_cmp(&b.0.top()));
            assert_eq!(
                rows.len(),
                2,
                "the selected item and the hovered one must both be highlighted, got {rows:?}"
            );
            (rows[0].1, rows[1].1) // (selected = Apple, hovered = Cherry)
        };

        // ââ Named neither: the constants the popup always painted âââââââââ
        let (selected, hovered) = two_bands(&combo(&[]));
        assert_eq!(
            selected,
            crate::paint::COMBO_SELECTED_FILL,
            "unnamed, the selected item keeps the popup's own highlight"
        );
        assert_eq!(
            hovered,
            crate::paint::COMBO_HOVER_FILL,
            "â¦and the hovered item keeps its own, fainter one"
        );

        // ââ Named both ââââââââââââââââââââââââââââââââââââââââââââââââââââ
        let (selected_n, hovered_n) = two_bands(&combo(&[
            ("ActiveItemColor", "#FF8800"),
            ("HoverItemColor", "#116622"),
        ]));
        assert_eq!(
            selected_n,
            Color32::from_rgb(0xFF, 0x88, 0x00),
            "the named colour must reach the selected item"
        );
        assert_eq!(
            hovered_n,
            Color32::from_rgb(0x11, 0x66, 0x22),
            "â¦and the named hover colour the item under the pointer"
        );

        // ââ Named one only: the other is untouched ââââââââââââââââââââââââ
        let (selected_o, hovered_o) = two_bands(&combo(&[("ActiveItemColor", "#FF8800")]));
        assert_eq!(selected_o, Color32::from_rgb(0xFF, 0x88, 0x00));
        assert_eq!(
            hovered_o,
            crate::paint::COMBO_HOVER_FILL,
            "a ComboBox's two highlights are independent â unlike a list's, where \
             the dimmed one follows the active colour"
        );

        println!(
            "\n  ComboBox item colours â unnamed: selected {selected:?} and hovered {hovered:?}, \
             the popup's own constants; named: #FF8800 and #116622 both reach their band; \
             naming only the selected colour leaves the hover at {hovered_o:?}\n"
        );
    }

    /// A running ComboBox wears the background the RAD gave it â a colour or a
    /// gradient â on its header AND on its open dropdown (operator, 2026-08-18).
    ///
    /// The header painted a hardcoded navy surface and a blue rim over its own
    /// face, the same defect the ListBox carried until 1.61.87: the designer
    /// canvas showed the design, the preview, Run Form and the compiled binary
    /// showed the navy. The panel hardcoded two more fills and a third rim.
    #[test]
    fn a_combobox_wears_the_background_designed_in_the_rad() {
        const HARDCODED_NAVY: Color32 = Color32::from_rgb(25, 38, 80);
        let base = |extra: &[(&str, &str)]| -> Vec<Control> {
            let mut props: Vec<(&str, &str)> =
                vec![("Items", "Apple\nBanana\nCherry"), ("Value", "Apple")];
            props.extend_from_slice(extra);
            vec![ctrlp("Cmb", ControlType::ComboBox, 20, 20, 220, 26, &props)]
        };
        let hc = pos2(130.0, 33.0);
        // Open it, so the header and the panel are both on screen at once.
        let open = |controls: &[Control]| -> Painted {
            drive_painted(
                controls,
                vec![
                    (0.0, vec![]),
                    (1.0, vec![Event::PointerMoved(hc), press(hc)]),
                    (2.0, vec![Event::PointerMoved(hc), release(hc)]),
                    (3.0, vec![]),
                ],
            )
        };

        // ââ A designed COLOUR: a deep red no default in the engine is ââââââ
        let painted = open(&base(&[("BackgroundColor", "#B00000FF")]));
        let header = *painted.placed.get("Cmb").expect("placed");
        let panel = Rect::from_min_max(
            pos2(header.left(), header.bottom() + 1.0),
            pos2(header.right(), header.bottom() + 200.0),
        );
        let red = |c: &Color32| c.r() > c.g() + 30 && c.r() > c.b() + 30;
        let covers = |r: &Rect, of: &Rect| r.intersect(*of).area() >= of.area() * 0.7;
        assert!(
            painted
                .fills
                .iter()
                .any(|(r, c)| covers(r, &header) && red(c)),
            "the designed red must reach the HEADER: {:?}",
            painted.fills
        );
        assert!(
            painted
                .fills
                .iter()
                .any(|(r, c)| r.intersect(panel).area() >= r.area() * 0.9
                    && r.width() >= header.width() - 1.0
                    && red(c)),
            "â¦and the open panel: {:?}",
            painted.fills
        );
        assert!(
            !painted.fills.iter().any(|(_, c)| *c == HARDCODED_NAVY),
            "the header's hardcoded navy must be gone: {:?}",
            painted.fills
        );

        // ââ A designed GRADIENT â the operator's own case, grey to black âââ
        let painted = open(&base(&[
            ("BackgroundGradientEnabled", "true"),
            ("BackgroundGradientStartColor", "#4E4E4EFF"),
            ("BackgroundGradientEndColor", "#000000FF"),
            ("BackgroundGradientDirection", "South"),
        ]));
        let header = *painted.placed.get("Cmb").expect("placed");
        let panel_top = header.bottom() + 1.0;
        assert!(
            painted
                .meshes
                .iter()
                .any(|m| m.intersect(header).area() >= header.area() * 0.7),
            "the designed gradient must be painted across the header: {:?}",
            painted.meshes
        );
        assert!(
            painted
                .meshes
                .iter()
                .any(|m| m.top() >= panel_top - 1.0 && m.width() >= header.width() - 1.0),
            "â¦and across the open panel: {:?}",
            painted.meshes
        );

        println!(
            "\n  ComboBox background â a designed #B00000 reaches the header AND the open \
             panel, a designed grey-to-black South gradient is painted as a mesh across \
             both, and the hardcoded navy that used to cover the header is gone\n"
        );
    }

    /// Every item of a long dropdown is reachable (operator, 2026-08-18).
    ///
    /// The panel stopped at 180 px and the item loop `break`ed as soon as an
    /// item would fall past the bottom, so anything past about the eighth was
    /// not clipped and not scrollable â it was never drawn at all. The panel
    /// now stands as tall as `DropDownHeight` allows and scrolls past that.
    #[test]
    fn a_long_dropdown_scrolls_instead_of_dropping_its_tail() {
        let items: Vec<String> = (1..=30).map(|n| format!("Item-{n:02}")).collect();
        let mut cmb = ctrl("Cmb", ControlType::ComboBox, 20, 20, 220, 26);
        cmb.set_prop("Items", crate::PropValue::String(items.join("\n")));
        cmb.set_prop("Value", crate::PropValue::String("Item-01".to_owned()));
        let item_h = combo_item_h(&cmb);
        let hc = pos2(130.0, 33.0);

        // Open, then walk to the very last item with the keyboard.
        let key = |k: egui::Key| Event::Key {
            key: k,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: Modifiers::default(),
        };
        let mut frames = vec![
            (0.00, vec![Event::PointerMoved(hc), press(hc)]),
            (0.05, vec![Event::PointerMoved(hc), release(hc)]),
        ];
        for i in 0..29 {
            frames.push((0.10 + i as f64 * 0.05, vec![key(egui::Key::ArrowDown)]));
        }
        frames.push((1.60, vec![key(egui::Key::Enter)]));
        frames.push((1.65, vec![]));
        let painted = drive_painted(&[cmb.clone()], frames);
        assert_eq!(
            painted
                .overrides
                .get("Cmb")
                .and_then(|p| p.get("Value"))
                .map(String::as_str),
            Some("Item-30"),
            "twenty-nine ArrowDowns and Enter must reach the LAST item â it used \
             to be undrawn and unreachable"
        );

        // â¦and while walking there, the item the keyboard is on is ON SCREEN.
        let mut frames = vec![
            (0.00, vec![Event::PointerMoved(hc), press(hc)]),
            (0.05, vec![Event::PointerMoved(hc), release(hc)]),
        ];
        for i in 0..20 {
            frames.push((0.10 + i as f64 * 0.05, vec![key(egui::Key::ArrowDown)]));
        }
        frames.push((1.20, vec![]));
        frames.push((1.25, vec![]));
        let painted = drive_painted(&[cmb.clone()], frames);
        let header = *painted.placed.get("Cmb").expect("placed");
        let panel = Rect::from_min_max(
            pos2(header.left(), header.bottom()),
            pos2(header.right(), header.bottom() + 1.0 + 200.0),
        );
        assert!(
            painted
                .texts
                .iter()
                .any(|t| t.text == "Item-21" && panel.expand(1.0).contains_rect(t.ink)),
            "the item twenty arrows down must be visible inside the panel: painted {:?}",
            painted.texts.iter().map(|t| t.text.as_str()).collect::<Vec<_>>()
        );

        // Opening the list shows the CURRENT value, wherever it sits: a
        // dropdown that opens at the top of a long list, with the value it
        // holds forty items further down, is one the operator has to hunt in.
        let mut deep = cmb.clone();
        deep.set_prop("Value", crate::PropValue::String("Item-25".to_owned()));
        let painted = drive_painted(
            &[deep],
            vec![
                (0.00, vec![Event::PointerMoved(hc), press(hc)]),
                (0.05, vec![Event::PointerMoved(hc), release(hc)]),
                (0.10, vec![]),
                (0.15, vec![]),
            ],
        );
        assert!(
            painted
                .texts
                .iter()
                .any(|t| t.text == "Item-25" && panel.expand(1.0).contains_rect(t.ink)),
            "opening the list must scroll to the value it holds: painted {:?}",
            painted.texts.iter().map(|t| t.text.as_str()).collect::<Vec<_>>()
        );

        println!(
            "\n  ComboBox scrolling â a 30-item list in a {}px panel of {item_h:.1}px items: \
             ArrowDown reaches Item-30 and Enter commits it, Item-21 is on screen twenty \
             arrows down, and a list holding Item-25 opens showing it\n",
            200
        );
    }

    /// The classic combo gesture: press on the header, drag into the list,
    /// release on an item to pick it. A drag is ONE gesture with an anchor â
    /// the header â and what it highlights is the item under the pointer NOW,
    /// so reversing direction walks the highlight back instead of freezing it.
    /// Dragging past either end holds at that end rather than choosing nothing.
    #[test]
    fn a_drag_from_the_header_picks_the_item_it_is_released_on() {
        let mut cmb = ctrl("Cmb", ControlType::ComboBox, 20, 20, 220, 26);
        cmb.set_prop(
            "Items",
            crate::PropValue::String("Alpha\nBeta\nGamma\nDelta\nEpsilon".to_owned()),
        );
        cmb.set_prop("Value", crate::PropValue::String("Alpha".to_owned()));
        let item_h = combo_item_h(&cmb);
        let header = Rect::from_min_size(pos2(20.0, 20.0), Vec2::new(220.0, 26.0));
        let item = |n: usize| combo_item_at(header, item_h, n);
        let hc = header.center();
        let value = |o: &Map<String, Map<String, String>>| {
            o.get("Cmb").and_then(|p| p.get("Value")).cloned()
        };

        // Down to item 3, back up to item 1, release there.
        let (events, overrides) = drive(
            &[cmb.clone()],
            vec![
                (0.00, vec![Event::PointerMoved(hc)]),
                (0.05, vec![press(hc)]),
                (0.10, vec![Event::PointerMoved(item(2))]),
                (0.15, vec![Event::PointerMoved(item(3))]),
                (0.20, vec![Event::PointerMoved(item(2))]),
                (0.25, vec![Event::PointerMoved(item(1))]),
                (0.30, vec![release(item(1))]),
                (0.35, vec![]),
            ],
        );
        assert_eq!(
            value(&overrides).as_deref(),
            Some("Beta"),
            "the release lands on the item under the pointer, not on the deepest \
             one the drag ever reached"
        );
        assert_eq!(
            overrides
                .get("Cmb")
                .and_then(|p| p.get("SelectedIndex"))
                .map(String::as_str),
            Some("1")
        );
        assert!(
            names(&events).contains(&"onSelectedIndexChanged"),
            "a drag reports itself like a click does: {:?}",
            names(&events)
        );

        // Dragging far BELOW the list holds at the last item.
        let (_events, overrides) = drive(
            &[cmb.clone()],
            vec![
                (0.00, vec![Event::PointerMoved(hc)]),
                (0.05, vec![press(hc)]),
                (0.10, vec![Event::PointerMoved(item(1))]),
                (0.15, vec![Event::PointerMoved(pos2(130.0, 900.0))]),
                (0.20, vec![release(pos2(130.0, 900.0))]),
                (0.25, vec![]),
            ],
        );
        assert_eq!(
            value(&overrides).as_deref(),
            Some("Epsilon"),
            "a drag past the bottom stops on the LAST item rather than choosing nothing"
        );

        // â¦and far ABOVE it holds at the first.
        let (_events, overrides) = drive(
            &[cmb.clone()],
            vec![
                (0.00, vec![Event::PointerMoved(hc)]),
                (0.05, vec![press(hc)]),
                (0.10, vec![Event::PointerMoved(item(3))]),
                (0.15, vec![Event::PointerMoved(pos2(130.0, -400.0))]),
                (0.20, vec![release(pos2(130.0, -400.0))]),
                (0.25, vec![]),
            ],
        );
        assert_eq!(
            value(&overrides).as_deref(),
            Some("Alpha"),
            "and a drag above the list stops on the FIRST item"
        );

        // A plain click on the header opens the list WITHOUT choosing anything.
        let (events, overrides) = drive(
            &[cmb.clone()],
            vec![
                (0.00, vec![Event::PointerMoved(hc)]),
                (0.05, vec![press(hc)]),
                (0.10, vec![release(hc)]),
                (0.15, vec![]),
            ],
        );
        assert_eq!(
            value(&overrides),
            None,
            "the click that opens the list must not also pick the item under it"
        );
        assert!(
            names(&events).contains(&"onDropDown"),
            "â¦but it does open it: {:?}",
            names(&events)
        );

        println!(
            "\n  ComboBox drag â press the header, down to Delta and back to Beta, release \
             â Beta (the reversal is answered, not frozen); past the bottom â Epsilon, \
             past the top â Alpha; a plain click opens without choosing\n"
        );
    }

    /// Up and down walk the list and stop at both ends, and what they mean
    /// depends on whether the dropdown is open â as a Windows combo does.
    ///
    ///   * closed: they change the value outright, reporting `onChange` and
    ///     `onSelectedIndexChanged` exactly as a click does;
    ///   * open: they move the highlight, Enter picks it, Escape closes without
    ///     changing anything.
    #[test]
    fn the_arrow_keys_walk_a_combobox_open_or_closed() {
        let mut cmb = ctrl("Cmb", ControlType::ComboBox, 20, 20, 220, 26);
        cmb.set_prop(
            "Items",
            crate::PropValue::String("Alpha\nBeta\nGamma\nDelta".to_owned()),
        );
        cmb.set_prop("Value", crate::PropValue::String("Beta".to_owned()));
        let hc = pos2(130.0, 33.0);
        let key = |k: egui::Key| Event::Key {
            key: k,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: Modifiers::default(),
        };
        let value = |o: &Map<String, Map<String, String>>| {
            o.get("Cmb").and_then(|p| p.get("Value")).cloned()
        };
        // Click the header twice â open, then closed â so the combo holds the
        // keyboard with its list shut.
        let focus_closed = || {
            vec![
                (0.00, vec![Event::PointerMoved(hc)]),
                (0.05, vec![press(hc)]),
                (0.10, vec![release(hc)]),
                (0.15, vec![press(hc)]),
                (0.20, vec![release(hc)]),
            ]
        };

        // ââ Closed: the arrows change the value âââââââââââââââââââââââââââ
        let mut frames = focus_closed();
        frames.push((0.25, vec![key(egui::Key::ArrowDown)]));
        frames.push((0.30, vec![]));
        let (events, overrides) = drive(&[cmb.clone()], frames);
        assert_eq!(
            value(&overrides).as_deref(),
            Some("Gamma"),
            "one ArrowDown from Beta with the list shut moves the value on"
        );
        assert!(
            names(&events).contains(&"onSelectedIndexChanged"),
            "a keyboard move reports itself like a click does: {:?}",
            names(&events)
        );

        // â¦and stops at the ends rather than wrapping.
        let mut frames = focus_closed();
        for i in 0..4 {
            frames.push((0.25 + i as f64 * 0.05, vec![key(egui::Key::ArrowUp)]));
        }
        frames.push((0.50, vec![]));
        let (_events, overrides) = drive(&[cmb.clone()], frames);
        assert_eq!(value(&overrides).as_deref(), Some("Alpha"), "up stops at the first");

        let mut frames = focus_closed();
        for i in 0..5 {
            frames.push((0.25 + i as f64 * 0.05, vec![key(egui::Key::ArrowDown)]));
        }
        frames.push((0.55, vec![]));
        let (_events, overrides) = drive(&[cmb.clone()], frames);
        assert_eq!(value(&overrides).as_deref(), Some("Delta"), "down stops at the last");

        // ââ Open: the arrows move the HIGHLIGHT and Enter picks it ââââââââ
        let (events, overrides) = drive(
            &[cmb.clone()],
            vec![
                (0.00, vec![Event::PointerMoved(hc)]),
                (0.05, vec![press(hc)]),
                (0.10, vec![release(hc)]),
                (0.15, vec![key(egui::Key::ArrowDown)]),
                (0.20, vec![key(egui::Key::ArrowDown)]),
                (0.25, vec![]),
            ],
        );
        assert_eq!(
            value(&overrides),
            None,
            "walking an OPEN list moves the highlight without committing anything"
        );
        assert!(
            !names(&events).contains(&"onSelectedIndexChanged"),
            "â¦and reports nothing until it is committed: {:?}",
            names(&events)
        );

        let (_events, overrides) = drive(
            &[cmb.clone()],
            vec![
                (0.00, vec![Event::PointerMoved(hc)]),
                (0.05, vec![press(hc)]),
                (0.10, vec![release(hc)]),
                (0.15, vec![key(egui::Key::ArrowDown)]),
                (0.20, vec![key(egui::Key::ArrowDown)]),
                (0.25, vec![key(egui::Key::Enter)]),
                (0.30, vec![]),
            ],
        );
        assert_eq!(
            value(&overrides).as_deref(),
            Some("Delta"),
            "Enter commits the item the arrows reached"
        );

        // ââ Escape closes without changing the value ââââââââââââââââââââââ
        let (_events, overrides) = drive(
            &[cmb.clone()],
            vec![
                (0.00, vec![Event::PointerMoved(hc)]),
                (0.05, vec![press(hc)]),
                (0.10, vec![release(hc)]),
                (0.15, vec![key(egui::Key::ArrowDown)]),
                (0.20, vec![key(egui::Key::Escape)]),
                (0.25, vec![]),
            ],
        );
        assert_eq!(
            value(&overrides),
            None,
            "Escape leaves the value exactly where it was"
        );

        // ââ Picking from the list leaves the arrows working âââââââââââââââ
        //
        // The header drops the keyboard on any press outside itself, and a
        // press on an item IS outside it â so without the popup handing it
        // back, choosing from the list left the combo unable to answer an
        // arrow until its header had been clicked again.
        let item_h = combo_item_h(&cmb);
        let header = Rect::from_min_size(pos2(20.0, 20.0), Vec2::new(220.0, 26.0));
        let gamma = combo_item_at(header, item_h, 2);
        let (_events, overrides) = drive(
            &[cmb.clone()],
            vec![
                (0.00, vec![Event::PointerMoved(hc)]),
                (0.05, vec![press(hc)]),
                (0.10, vec![release(hc)]),
                (0.15, vec![Event::PointerMoved(gamma), press(gamma)]),
                (0.20, vec![Event::PointerMoved(gamma), release(gamma)]),
                (0.25, vec![key(egui::Key::ArrowDown)]),
                (0.30, vec![]),
            ],
        );
        assert_eq!(
            value(&overrides).as_deref(),
            Some("Delta"),
            "clicking Gamma in the list and then pressing â must reach Delta â the \
             combo keeps the keyboard through the pick"
        );

        println!(
            "\n  ComboBox keys â shut: â from Beta â Gamma (onSelectedIndexChanged fired), \
             ââââ stops at Alpha, âââââ stops at Delta; open: ââ moves the highlight and \
             commits nothing, Enter â Delta, Escape â unchanged; picking Gamma from the \
             list then â â Delta\n"
        );
    }

    /// An open dropdown letters its items in the control's OWN font and colour,
    /// and gives each one a line of that font's height. All three were
    /// hardcoded â 22 px, 12 pt and a fixed near-white â so a combo set to
    /// 20 pt drew a 20 pt value over a list of 12 pt items.
    #[test]
    fn a_dropdowns_items_are_lettered_in_the_controls_own_type() {
        let mut cmb = ctrl("Cmb", ControlType::ComboBox, 20, 20, 260, 40);
        cmb.set_prop("Items", crate::PropValue::String("Alpha\nBeta".to_owned()));
        cmb.set_prop("Value", crate::PropValue::String("Alpha".to_owned()));
        cmb.set_prop("FontSize", crate::PropValue::Int(20));
        cmb.set_prop("ForegroundColor", crate::PropValue::String("#FFD400".into()));
        let hc = pos2(150.0, 40.0);
        let painted = drive_painted(
            &[cmb.clone()],
            vec![
                (0.0, vec![]),
                (1.0, vec![Event::PointerMoved(hc), press(hc)]),
                (2.0, vec![Event::PointerMoved(hc), release(hc)]),
                (3.0, vec![]),
            ],
        );
        let header = *painted.placed.get("Cmb").expect("placed");
        let beta = painted
            .texts
            .iter()
            .find(|t| t.text == "Beta")
            .unwrap_or_else(|| {
                panic!(
                    "the second item must be painted: {:?}",
                    painted.texts.iter().map(|t| t.text.as_str()).collect::<Vec<_>>()
                )
            });
        assert!(
            (beta.font - 20.0).abs() <= 0.5,
            "the item takes the control's FontSize, not a hardcoded 12: {}",
            beta.font
        );
        // The items are a line of that type apart, so a 20 pt list is not
        // crammed into 22 px rows.
        let alpha = painted
            .texts
            .iter()
            .find(|t| t.text == "Alpha" && t.pos.y > header.bottom())
            .expect("the first item is painted below the header");
        let pitch = beta.pos.y - alpha.pos.y;
        assert!(
            (pitch - combo_item_h(&cmb)).abs() <= 0.5,
            "one item is one line of the control's own text plus air: {pitch}"
        );

        println!(
            "\n  ComboBox typography â a 20 pt combo letters its items at {}pt on a \
             {pitch:.1}px pitch, where both were hardcoded at 12 pt on 22 px\n",
            beta.font
        );
    }

    #[test]
    fn engine_timer_ticks_on_interval() {
        let c = [ctrlp(
            "Tmr",
            ControlType::Timer,
            0,
            0,
            1,
            1,
            &[("Interval", "10")],
        )];
        let (evs, _) = drive(
            &c,
            vec![
                (0.0, vec![]), // arm
                (1.0, vec![]), // 1s later â tick (interval 10ms)
            ],
        );
        assert!(
            names(&evs).contains(&"onTick"),
            "Timer: no onTick; got {:?}",
            names(&evs)
        );
    }

    /// A FormState with a fixed chrome-`enabled` answer â mirrors the real run
    /// surface reporting `enabled="false"` on a (non-visual) control.
    struct ChromeState {
        chrome_enabled: bool,
    }
    impl FormState for ChromeState {
        fn enabled(&self, _base: &Control) -> bool {
            self.chrome_enabled
        }
    }

    /// Run `frames` frames (1 simulated second apart) and collect engine events.
    fn drive_state(state: &dyn FormState, controls: &[Control], frames: usize) -> Vec<UiEvent> {
        let ctx = egui::Context::default();
        ctx.set_fonts(egui::FontDefinitions::default());
        let active = ActiveTabs::new();
        let mut all = Vec::new();
        for i in 0..frames {
            let mut input = egui::RawInput::default();
            input.screen_rect = Some(Rect::from_min_size(pos2(0.0, 0.0), Vec2::new(400.0, 300.0)));
            input.focused = true;
            input.time = Some(i as f64); // 1s/frame â clears any interval
            let events = RefCell::new(Vec::<UiEvent>::new());
            ctx.run_ui(input, |root_ui| {
                let ctx = root_ui.ctx().clone();
                let ctx = &ctx;
                egui::CentralPanel::default()
                    .frame(egui::Frame::NONE)
                    .show_inside(root_ui, |ui| {
                        ui.set_min_size(Vec2::new(400.0, 300.0));
                        let inp = RenderInput {
                            controls,
                            state,
                            form_size: Vec2::new(400.0, 300.0),
                            glass: true,
                            mode: RenderMode::Interactive,
                            active_tabs: &active,
                            backdrop: Backdrop::default(),
                        };
                        let out = render_form(ui, &inp);
                        events.borrow_mut().extend(out.events);
                    });
            }).textures_delta.clear();
            all.extend(events.into_inner());
        }
        all
    }

    /// A Timer keeps ticking, for as long as the form runs.
    ///
    /// Reported 2026-08-17: "timer events are dying after some dozens of times.
    /// It stops silently, no warnings, just stops." This drives the renderer over
    /// hundreds of intervals and counts every tick, so a Timer that quietly gives
    /// up cannot pass.
    #[test]
    fn a_timer_keeps_ticking_for_hundreds_of_intervals() {
        let interval_ms = 100.0_f64;
        let timer = [ctrlp(
            "Tmr",
            ControlType::Timer,
            0,
            0,
            1,
            1,
            &[("Interval", "100"), ("Enabled", "true")],
        )];

        // One frame per interval, so every frame is a tick that is due. 300 of
        // them is far past "some dozens".
        let ctx = egui::Context::default();
        ctx.set_fonts(egui::FontDefinitions::default());
        let active = ActiveTabs::new();
        let frames = 300usize;
        let mut ticks = 0usize;
        let mut first_gap: Option<usize> = None;
        for i in 0..frames {
            let mut input = egui::RawInput::default();
            input.screen_rect = Some(Rect::from_min_size(pos2(0.0, 0.0), Vec2::new(400.0, 300.0)));
            input.focused = true;
            input.time = Some(i as f64 * interval_ms / 1000.0);
            let events = RefCell::new(Vec::<UiEvent>::new());
            ctx.run_ui(input, |root_ui| {
                egui::CentralPanel::default()
                    .frame(egui::Frame::NONE)
                    .show_inside(root_ui, |ui| {
                        ui.set_min_size(Vec2::new(400.0, 300.0));
                        let inp = RenderInput {
                            controls: &timer,
                            state: &DesignedState,
                            form_size: Vec2::new(400.0, 300.0),
                            glass: true,
                            mode: RenderMode::Interactive,
                            active_tabs: &active,
                            backdrop: Backdrop::default(),
                        };
                        let out = render_form(ui, &inp);
                        events.borrow_mut().extend(out.events);
                    });
            })
            .textures_delta.clear();
            let got = events
                .into_inner()
                .iter()
                .filter(|e| e.event == "onTick")
                .count();
            if got > 0 {
                ticks += got;
            } else if i > 0 && first_gap.is_none() {
                // Frame 0 only establishes the clock, so it never ticks.
                first_gap = Some(i);
            }
        }

        // Frame 0 sets the baseline; every frame after it is one interval on.
        assert_eq!(
            ticks,
            frames - 1,
            "a Timer must not stop: {ticks} ticks over {frames} intervals, first \
             missing at frame {first_gap:?}"
        );

        println!(
            "\n  Timer endurance â a 100 ms Timer driven over {frames} intervals fired \
             {ticks} times, one per interval with no gap; the clock is egui time and the \
             wake-up is rescheduled from the tick that just fired, so it cannot drift into \
             silence\n"
        );
    }

    #[test]
    fn engine_timer_ticks_governed_by_enabled_property_not_chrome_flag() {
        // Real .cfrm shape: a non-visual Timer with chrome `enabled="false"` but
        // its own `Enabled` property = true. It MUST still tick â the property
        // is the timer's on/off, not the chrome flag.
        let on = [ctrlp(
            "Tmr",
            ControlType::Timer,
            0,
            0,
            1,
            1,
            &[("Interval", "10"), ("Enabled", "true")],
        )];
        let evs = drive_state(
            &ChromeState {
                chrome_enabled: false,
            },
            &on,
            2,
        );
        assert!(
            names(&evs).contains(&"onTick"),
            "Timer with Enabled=true must tick even when chrome enabled=false; got {:?}",
            names(&evs)
        );

        // Conversely, the `Enabled` property = false silences it regardless of
        // the chrome flag being true.
        let off = [ctrlp(
            "Tmr",
            ControlType::Timer,
            0,
            0,
            1,
            1,
            &[("Interval", "10"), ("Enabled", "false")],
        )];
        let evs_off = drive_state(
            &ChromeState {
                chrome_enabled: true,
            },
            &off,
            2,
        );
        assert!(
            !names(&evs_off).contains(&"onTick"),
            "Timer with Enabled=false must not tick; got {:?}",
            names(&evs_off)
        );
    }
}

/// Map a character from `menu::Accelerator` to an `egui::Key`.
#[cfg(feature = "render")]
fn char_to_key(c: char) -> egui::Key {
    match c {
        'A' => egui::Key::A,
        'B' => egui::Key::B,
        'C' => egui::Key::C,
        'D' => egui::Key::D,
        'E' => egui::Key::E,
        'F' => egui::Key::F,
        'G' => egui::Key::G,
        'H' => egui::Key::H,
        'I' => egui::Key::I,
        'J' => egui::Key::J,
        'K' => egui::Key::K,
        'L' => egui::Key::L,
        'M' => egui::Key::M,
        'N' => egui::Key::N,
        'O' => egui::Key::O,
        'P' => egui::Key::P,
        'Q' => egui::Key::Q,
        'R' => egui::Key::R,
        'S' => egui::Key::S,
        'T' => egui::Key::T,
        'U' => egui::Key::U,
        'V' => egui::Key::V,
        'W' => egui::Key::W,
        'X' => egui::Key::X,
        'Y' => egui::Key::Y,
        'Z' => egui::Key::Z,
        '0' => egui::Key::Num0,
        '1' => egui::Key::Num1,
        '2' => egui::Key::Num2,
        '3' => egui::Key::Num3,
        '4' => egui::Key::Num4,
        '5' => egui::Key::Num5,
        '6' => egui::Key::Num6,
        '7' => egui::Key::Num7,
        '8' => egui::Key::Num8,
        '9' => egui::Key::Num9,
        '\u{F001}' => egui::Key::F1,
        '\u{F002}' => egui::Key::F2,
        '\u{F003}' => egui::Key::F3,
        '\u{F004}' => egui::Key::F4,
        '\u{F005}' => egui::Key::F5,
        '\u{F006}' => egui::Key::F6,
        '\u{F007}' => egui::Key::F7,
        '\u{F008}' => egui::Key::F8,
        '\u{F009}' => egui::Key::F9,
        '\u{F00A}' => egui::Key::F10,
        '\u{F00B}' => egui::Key::F11,
        '\u{F00C}' => egui::Key::F12,
        '\u{007F}' => egui::Key::Delete,
        '\u{0008}' => egui::Key::Backspace,
        '\t' => egui::Key::Tab,
        '\r' => egui::Key::Enter,
        '\u{001B}' => egui::Key::Escape,
        ' ' => egui::Key::Space,
        _ => egui::Key::A,
    }
}
// Shape-dump differ (spec 027 corner-bleed hunt) â egui 0.35 branch flavor.
// Appended to cobolt-forms/src/render.rs tests; renders one neumorphic-panel
// frame and dumps every non-text paint shape, normalized, to a file given in
// COBOLT_SHAPE_DUMP.
#[cfg(test)]
mod shape_dump {
    use super::*;
    use std::cell::RefCell;
    use std::collections::HashMap as Map;

    fn dump_shape(out: &mut Vec<String>, clip: egui::Rect, shape: &egui::Shape) {
        use egui::Shape as S;
        let r2 = |v: f32| (v * 4.0).round() / 4.0;
        let fr = |r: egui::Rect| {
            format!(
                "[{} {} {} {}]",
                r2(r.min.x),
                r2(r.min.y),
                r2(r.max.x),
                r2(r.max.y)
            )
        };
        match shape {
            S::Vec(v) => {
                for s in v {
                    dump_shape(out, clip, s);
                }
            }
            S::Text(_) => {} // font engines differ across versions â geometry only
            S::Rect(rs) => out.push(format!(
                "RECT bbox={} fill=#{:02x}{:02x}{:02x}{:02x} stroke={}@#{:02x}{:02x}{:02x}{:02x} r=[{} {} {} {}] clip={}",
                fr(rs.rect),
                rs.fill.r(), rs.fill.g(), rs.fill.b(), rs.fill.a(),
                r2(rs.stroke.width),
                rs.stroke.color.r(), rs.stroke.color.g(), rs.stroke.color.b(), rs.stroke.color.a(),
                rs.corner_radius.nw, rs.corner_radius.ne, rs.corner_radius.sw, rs.corner_radius.se,
                fr(clip),
            )),
            S::Path(ps) => out.push(format!(
                "PATH n={} bbox={} fill=#{:02x}{:02x}{:02x}{:02x} stroke={} clip={}",
                ps.points.len(),
                fr(shape.visual_bounding_rect()),
                ps.fill.r(), ps.fill.g(), ps.fill.b(), ps.fill.a(),
                r2(ps.stroke.width),
                fr(clip),
            )),
            S::Mesh(m) => out.push(format!(
                "MESH v={} i={} bbox={} c0=#{:02x}{:02x}{:02x}{:02x} clip={}",
                m.vertices.len(),
                m.indices.len(),
                fr(shape.visual_bounding_rect()),
                m.vertices.first().map(|v| v.color.r()).unwrap_or(0),
                m.vertices.first().map(|v| v.color.g()).unwrap_or(0),
                m.vertices.first().map(|v| v.color.b()).unwrap_or(0),
                m.vertices.first().map(|v| v.color.a()).unwrap_or(0),
                fr(clip),
            )),
            S::LineSegment { points, stroke } => out.push(format!(
                "LINE [{} {}]-[{} {}] w={} c=#{:02x}{:02x}{:02x}{:02x} clip={}",
                r2(points[0].x), r2(points[0].y), r2(points[1].x), r2(points[1].y),
                r2(stroke.width),
                stroke.color.r(), stroke.color.g(), stroke.color.b(), stroke.color.a(),
                fr(clip),
            )),
            S::Circle(cs) => out.push(format!(
                "CIRCLE c=[{} {}] r={} fill=#{:02x}{:02x}{:02x}{:02x} clip={}",
                r2(cs.center.x), r2(cs.center.y), r2(cs.radius),
                cs.fill.r(), cs.fill.g(), cs.fill.b(), cs.fill.a(),
                fr(clip),
            )),
            other => out.push(format!(
                "OTHER {:?} bbox={}",
                std::mem::discriminant(other),
                fr(other.visual_bounding_rect()),
            )),
        }
    }

    #[test]
    fn dump_neumorphic_panel_shapes() {
        let Some(path) = std::env::var_os("COBOLT_SHAPE_DUMP") else {
            return; // only runs when explicitly requested
        };
        let ctx = egui::Context::default();
        crate::paint::set_glass_style(&ctx, crate::model::GlassStyle::Neumorphic);

        let container = {
            let mut c = Control::new("PNL", ControlType::Panel, 40, 40);
            c.rect = crate::model::Rect::new(40, 40, 400, 200);
            c.set_prop("CornerRadius", crate::model::PropValue::Int(24));
            c
        };
        let controls = vec![container];
        let overrides: RefCell<Map<String, Map<String, String>>> = RefCell::new(Map::new());
        let active_tabs: crate::containers::ActiveTabs = Default::default();

        let mut input = egui::RawInput::default();
        input.screen_rect = Some(Rect::from_min_size(pos2(0.0, 0.0), Vec2::new(600.0, 300.0)));
        input.focused = true;
        let mut full = ctx.run_ui(input, |root_ui| {
            egui::CentralPanel::default()
                .frame(egui::Frame::NONE)
                .show_inside(root_ui, |ui| {
                    let st = MapState_dump(&overrides);
                    let rin = RenderInput {
                        controls: &controls,
                        state: &st,
                        form_size: Vec2::new(600.0, 300.0),
                        glass: true,
                        mode: RenderMode::Interactive,
                        active_tabs: &active_tabs,
                        backdrop: Backdrop {
                            paint: true,
                            color_hex: String::new(),
                            transparency: 0,
                            gradient_enabled: false,
                            gradient_start_hex: String::new(),
                            gradient_end_hex: String::new(),
                            gradient_direction: "South".into(),
                            image: None,
                            image_mode: Default::default(),
                            use_theme_background: false,
                            window_size: None,
                        },
                    };
                    let _ = render_form(ui, &rin);
                });
        });
        full.textures_delta.clear();
        let mut out = Vec::new();
        for cs in &full.shapes {
            dump_shape(&mut out, cs.clip_rect, &cs.shape);
        }
        std::fs::write(&path, out.join("\n")).unwrap();
        println!("dumped {} shapes", out.len());
    }

    /// Scene B â Classic glass + backdrop image + corner-reaching child:
    /// exercises the notch mask / restore-outline path. Dump-only (set
    /// COBOLT_SHAPE_DUMP_B=<file>).
    #[test]
    fn dump_classic_glass_notch_shapes() {
        let Some(path) = std::env::var_os("COBOLT_SHAPE_DUMP_B") else {
            return;
        };
        let ctx = egui::Context::default();
        crate::paint::set_glass_style(&ctx, crate::model::GlassStyle::Classic);

        let mut container = Control::new("PNL", ControlType::Panel, 40, 40);
        container.rect = crate::model::Rect::new(40, 40, 400, 200);
        container.set_prop("CornerRadius", crate::model::PropValue::Int(24));
        let mut child = Control::new("LBL", ControlType::Label, 42, 42);
        child.rect = crate::model::Rect::new(42, 42, 120, 30);
        child.parent = Some("PNL".into());
        let controls = vec![container, child];

        let tex = ctx.load_texture(
            "dump_bg",
            egui::ColorImage {
                size: [4, 4],
                source_size: egui::vec2(4.0, 4.0),
                pixels: vec![egui::Color32::from_rgb(160, 120, 60); 16],
            },
            egui::TextureOptions::LINEAR,
        );
        let overrides: RefCell<Map<String, Map<String, String>>> = RefCell::new(Map::new());
        let active_tabs: crate::containers::ActiveTabs = Default::default();
        let mut input = egui::RawInput::default();
        input.screen_rect = Some(Rect::from_min_size(pos2(0.0, 0.0), Vec2::new(600.0, 300.0)));
        let mut full = ctx.run_ui(input, |root_ui| {
            egui::CentralPanel::default()
                .frame(egui::Frame::NONE)
                .show_inside(root_ui, |ui| {
                    let st = MapState_dump(&overrides);
                    let rin = RenderInput {
                        controls: &controls,
                        state: &st,
                        form_size: Vec2::new(600.0, 300.0),
                        glass: true,
                        mode: RenderMode::Interactive,
                        active_tabs: &active_tabs,
                        backdrop: Backdrop {
                            paint: true,
                            color_hex: "8a6a3c".into(),
                            transparency: 0,
                            gradient_enabled: false,
                            gradient_start_hex: String::new(),
                            gradient_end_hex: String::new(),
                            gradient_direction: "South".into(),
                            image: Some((tex.id(), egui::vec2(4.0, 4.0))),
                            image_mode: Default::default(),
                            use_theme_background: false,
                            window_size: None,
                        },
                    };
                    let _ = render_form(ui, &rin);
                });
        });
        full.textures_delta.clear();
        let mut out = Vec::new();
        for cs in &full.shapes {
            dump_shape(&mut out, cs.clip_rect, &cs.shape);
        }
        std::fs::write(&path, out.join("\n")).unwrap();
        println!("scene B dumped {} shapes", out.len());
    }

    /// Scene C â captioned GroupBox + nested Panel + corner children, Classic
    /// glass, image backdrop (COBOLT_SHAPE_DUMP_C=<file>).
    #[test]
    fn dump_groupbox_nested_shapes() {
        let Some(path) = std::env::var_os("COBOLT_SHAPE_DUMP_C") else {
            return;
        };
        let ctx = egui::Context::default();
        crate::paint::set_glass_style(&ctx, crate::model::GlassStyle::Classic);

        let mut gb = Control::new("GB", ControlType::GroupBox, 40, 40);
        gb.rect = crate::model::Rect::new(40, 40, 400, 200);
        gb.set_prop("CornerRadius", crate::model::PropValue::Int(24));
        gb.set_prop("Caption", crate::model::PropValue::String("Group".into()));
        let mut inner = Control::new("PNL2", ControlType::Panel, 60, 80);
        inner.rect = crate::model::Rect::new(60, 80, 150, 100);
        inner.set_prop("CornerRadius", crate::model::PropValue::Int(16));
        inner.parent = Some("GB".into());
        let mut child = Control::new("LBL", ControlType::Label, 42, 42);
        child.rect = crate::model::Rect::new(42, 42, 120, 30);
        child.parent = Some("GB".into());
        let controls = vec![gb, inner, child];

        let tex = ctx.load_texture(
            "dump_bg_c",
            egui::ColorImage {
                size: [4, 4],
                source_size: egui::vec2(4.0, 4.0),
                pixels: vec![egui::Color32::from_rgb(160, 120, 60); 16],
            },
            egui::TextureOptions::LINEAR,
        );
        let overrides: RefCell<Map<String, Map<String, String>>> = RefCell::new(Map::new());
        let active_tabs: crate::containers::ActiveTabs = Default::default();
        let mut input = egui::RawInput::default();
        input.screen_rect = Some(Rect::from_min_size(pos2(0.0, 0.0), Vec2::new(600.0, 300.0)));
        let mut full = ctx.run_ui(input, |root_ui| {
            egui::CentralPanel::default()
                .frame(egui::Frame::NONE)
                .show_inside(root_ui, |ui| {
                    let st = MapState_dump(&overrides);
                    let rin = RenderInput {
                        controls: &controls,
                        state: &st,
                        form_size: Vec2::new(600.0, 300.0),
                        glass: true,
                        mode: RenderMode::Interactive,
                        active_tabs: &active_tabs,
                        backdrop: Backdrop {
                            paint: true,
                            color_hex: "8a6a3c".into(),
                            transparency: 0,
                            gradient_enabled: false,
                            gradient_start_hex: String::new(),
                            gradient_end_hex: String::new(),
                            gradient_direction: "South".into(),
                            image: Some((tex.id(), egui::vec2(4.0, 4.0))),
                            image_mode: Default::default(),
                            use_theme_background: false,
                            window_size: None,
                        },
                    };
                    let _ = render_form(ui, &rin);
                });
        });
        full.textures_delta.clear();
        let mut out = Vec::new();
        for cs in &full.shapes {
            dump_shape(&mut out, cs.clip_rect, &cs.shape);
        }
        std::fs::write(&path, out.join("\n")).unwrap();
        println!("scene C dumped {} shapes", out.len());
    }

    /// Scene D â TRANSPARENT Panel + DataGrid child on image backdrop, Classic
    /// glass (COBOLT_SHAPE_DUMP_D=<file>). Mirrors the operator's failing form.
    #[test]
    fn dump_transparent_panel_datagrid_shapes() {
        let Some(path) = std::env::var_os("COBOLT_SHAPE_DUMP_D") else {
            return;
        };
        let ctx = egui::Context::default();
        crate::paint::set_glass_style(&ctx, crate::model::GlassStyle::Classic);

        let mut pnl = Control::new("PNL", ControlType::Panel, 40, 40);
        pnl.rect = crate::model::Rect::new(40, 40, 400, 200);
        pnl.set_prop("CornerRadius", crate::model::PropValue::Int(24));
        pnl.set_prop(
            "BackgroundColor",
            crate::model::PropValue::String("00000000".into()),
        );
        let mut grid = Control::new("GRID", ControlType::DataGrid, 60, 60);
        grid.rect = crate::model::Rect::new(60, 60, 200, 120);
        grid.set_prop("CornerRadius", crate::model::PropValue::Int(16));
        grid.parent = Some("PNL".into());
        let controls = vec![pnl, grid];

        let tex = ctx.load_texture(
            "dump_bg_d",
            egui::ColorImage {
                size: [4, 4],
                source_size: egui::vec2(4.0, 4.0),
                pixels: vec![egui::Color32::from_rgb(160, 120, 60); 16],
            },
            egui::TextureOptions::LINEAR,
        );
        let overrides: RefCell<Map<String, Map<String, String>>> = RefCell::new(Map::new());
        let active_tabs: crate::containers::ActiveTabs = Default::default();
        let mut input = egui::RawInput::default();
        input.screen_rect = Some(Rect::from_min_size(pos2(0.0, 0.0), Vec2::new(600.0, 300.0)));
        let mut full = ctx.run_ui(input, |root_ui| {
            egui::CentralPanel::default()
                .frame(egui::Frame::NONE)
                .show_inside(root_ui, |ui| {
                    let st = MapState_dump(&overrides);
                    let rin = RenderInput {
                        controls: &controls,
                        state: &st,
                        form_size: Vec2::new(600.0, 300.0),
                        glass: true,
                        mode: RenderMode::Interactive,
                        active_tabs: &active_tabs,
                        backdrop: Backdrop {
                            paint: true,
                            color_hex: "8a6a3c".into(),
                            transparency: 0,
                            gradient_enabled: false,
                            gradient_start_hex: String::new(),
                            gradient_end_hex: String::new(),
                            gradient_direction: "South".into(),
                            image: Some((tex.id(), egui::vec2(4.0, 4.0))),
                            image_mode: Default::default(),
                            use_theme_background: false,
                            window_size: None,
                        },
                    };
                    let _ = render_form(ui, &rin);
                });
        });
        full.textures_delta.clear();
        let mut out = Vec::new();
        for cs in &full.shapes {
            dump_shape(&mut out, cs.clip_rect, &cs.shape);
        }
        std::fs::write(&path, out.join("\n")).unwrap();
        println!("scene D dumped {} shapes", out.len());
    }

    // ââ DataGrid confined-fill geometry (pure, no egui context needed) âââââââ

    /// A fill's rects must tile its vertical span with NO gap. A sub-pixel gap is
    /// not harmless: the grid's own background (a solid BackgroundColor â yellow
    /// in the operator's form) shows through it as a thin line, and because the
    /// gap depends on the fractional scroll offset it FLASHES on and off while
    /// scrolling. The original bug: the strip above the arc zone was skipped when
    /// thinner than `eps`. Swept across fractional offsets so any eps-style
    /// threshold reintroduced later is caught.
    #[test]
    fn datagrid_fill_rects_tile_without_gaps_at_any_subpixel_offset() {
        let screen = Rect::from_min_max(pos2(40.0, 40.0), pos2(1264.0, 424.0));
        let radius = 15.0_f32;
        let zone_top = screen.max.y - radius;
        let mut failures = Vec::new();
        // Sweep BOTH edges through the arc-zone boundary in 1/64px steps. The seam
        // bug needs a fill whose TOP sits a hair above `zone_top` (that strip was
        // skipped when thinner than `eps`), which is why the flashing line always
        // appeared at the same y â it is pinned to the zone top, and a row boundary
        // crosses that sub-pixel window as you scroll.
        let mut cases: Vec<Rect> = Vec::new();
        for step in -192i32..192 {
            let d = step as f32 / 64.0;
            // fill top walks across zone_top, bottom clipped at the grid edge
            cases.push(Rect::from_min_max(
                pos2(screen.min.x, zone_top + d),
                pos2(screen.max.x, screen.max.y),
            ));
            // fill bottom walks up from the grid edge (full 43px row)
            let bottom = screen.max.y - d.abs();
            cases.push(Rect::from_min_max(
                pos2(screen.min.x, bottom - 43.0),
                pos2(screen.max.x, bottom),
            ));
        }
        for r in cases {
            let r = r.intersect(screen);
            if r.height() <= 0.0 || r.width() <= 0.0 {
                continue;
            }
            let rects = datagrid_confined_fill_rects(screen, radius, r);
            if rects.is_empty() {
                failures.push(format!("fill top={:.4} h={:.4}: no rects", r.min.y, r.height()));
                continue;
            }
            let mut spans: Vec<(f32, f32)> = rects.iter().map(|x| (x.min.y, x.max.y)).collect();
            spans.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());
            if (spans[0].0 - r.min.y).abs() > 1e-3 {
                failures.push(format!(
                    "fill top={:.4}: coverage starts at {:.4} â {:.4}px SEAM at the top",
                    r.min.y,
                    spans[0].0,
                    spans[0].0 - r.min.y
                ));
            }
            let mut cursor = spans[0].1;
            for (a, b) in spans.iter().skip(1) {
                if *a - cursor > 1e-3 {
                    failures.push(format!(
                        "fill top={:.4}: GAP {:.4}..{:.4} ({:.4}px) â grid background bleeds through",
                        r.min.y,
                        cursor,
                        a,
                        a - cursor
                    ));
                    break;
                }
                cursor = cursor.max(*b);
            }
            if (cursor - r.max.y).abs() > 1e-3 {
                failures.push(format!(
                    "fill top={:.4}: coverage ends at {:.4}, expected {:.4}",
                    r.min.y, cursor, r.max.y
                ));
            }
        }
        assert!(
            failures.is_empty(),
            "confined fill rects must tile the fill with no gaps ({} failures):\n{}",
            failures.len(),
            failures
                .iter()
                .take(8)
                .cloned()
                .collect::<Vec<_>>()
                .join("\n")
        );
    }

    /// No emitted rect may cross the bottom-corner arcs â that is the bleed.
    #[test]
    fn datagrid_fill_rects_stay_inside_the_corner_arcs() {
        let screen = Rect::from_min_max(pos2(40.0, 40.0), pos2(1264.0, 424.0));
        let radius = 15.0_f32;
        let mut failures = Vec::new();
        for step in 0..160 {
            let frac = step as f32 / 8.0;
            let bottom = screen.max.y - frac;
            let top = bottom - 43.0;
            let r = Rect::from_min_max(pos2(screen.min.x, top), pos2(screen.max.x, bottom))
                .intersect(screen);
            if r.height() <= 0.0 {
                continue;
            }
            for sub in datagrid_confined_fill_rects(screen, radius, r) {
                // Bottom-left arc centre; a rect's bottom-left corner is the worst case.
                let c = pos2(screen.min.x + radius, screen.max.y - radius);
                let p = pos2(sub.min.x, sub.max.y);
                if p.x < c.x && p.y > c.y && (p - c).length() > radius + 0.01 {
                    failures.push(format!(
                        "frac={frac:.3}: rect [{:.2} {:.2} {:.2} {:.2}] corner ({:.2},{:.2}) is {:.2}px from the arc centre (radius {radius})",
                        sub.min.x, sub.min.y, sub.max.x, sub.max.y, p.x, p.y, (p - c).length()
                    ));
                }
            }
        }
        assert!(
            failures.is_empty(),
            "confined fill rects must stay inside the corner arc ({} failures):\n{}",
            failures.len(),
            failures.iter().take(8).cloned().collect::<Vec<_>>().join("\n")
        );
    }

    // ââ DataGrid rounded-corner silhouette guards ââââââââââââââââââââââââââââ
    // Geometry of the operator's failing grid (PowerDemo2 main-form, DataGrid-1).
    const DG_X: f32 = 40.0;
    const DG_Y: f32 = 40.0;
    const DG_W: f32 = 1224.0;
    const DG_H: f32 = 384.0;
    const DG_R: f32 = 15.0;
    const DG_FORM_W: f32 = 1320.0;
    const DG_FORM_H: f32 = 480.0;

    /// Render the operator's DataGrid scene with `row_count` rows over an image
    /// backdrop and return the paint shapes. Params verbatim from the failing
    /// form: yellow BackgroundColor, navy AlternatingRowColor (the colour seen
    /// bleeding), CornerRadius 15, RowHeight 43, and column filters ON â the tall
    /// filter header is what squeezes the last row into a few-pixel sliver.
    fn datagrid_corner_scene(row_count: usize) -> Vec<egui::epaint::ClippedShape> {
        let ctx = egui::Context::default();
        crate::paint::set_glass_style(&ctx, crate::model::GlassStyle::Classic);

        let mut grid = Control::new("GRID", ControlType::DataGrid, DG_X as i32, DG_Y as i32);
        grid.rect =
            crate::model::Rect::new(DG_X as i32, DG_Y as i32, DG_W as i32, DG_H as i32);
        grid.set_prop("CornerRadius", crate::model::PropValue::Int(DG_R as i64));
        grid.set_prop(
            "BackgroundColor",
            crate::model::PropValue::String("F5FF00FF".into()),
        );
        grid.set_prop(
            "AlternatingRowColor",
            crate::model::PropValue::String("12212FFF".into()),
        );
        grid.set_prop("ShowColumnFilters", crate::model::PropValue::Bool(true));
        grid.set_prop(
            "Columns",
            crate::model::PropValue::String("A:string\nB:string".into()),
        );
        let rows: String = (0..row_count)
            .map(|i| format!("row{i}-a\trow{i}-b"))
            .collect::<Vec<_>>()
            .join("\n");
        grid.set_prop("Rows", crate::model::PropValue::String(rows));
        grid.set_prop("RowHeight", crate::model::PropValue::String("43".into()));
        let controls = vec![grid];

        let tex = ctx.load_texture(
            "dg_corner_bg",
            egui::ColorImage {
                size: [4, 4],
                source_size: egui::vec2(4.0, 4.0),
                pixels: vec![egui::Color32::from_rgb(160, 120, 60); 16],
            },
            egui::TextureOptions::LINEAR,
        );
        let overrides: RefCell<Map<String, Map<String, String>>> = RefCell::new(Map::new());
        let active_tabs: crate::containers::ActiveTabs = Default::default();
        let mut input = egui::RawInput::default();
        input.screen_rect = Some(Rect::from_min_size(
            pos2(0.0, 0.0),
            Vec2::new(DG_FORM_W, DG_FORM_H),
        ));
        let mut full = ctx.run_ui(input, |root_ui| {
            egui::CentralPanel::default()
                .frame(egui::Frame::NONE)
                .show_inside(root_ui, |ui| {
                    let st = MapState_dump(&overrides);
                    let rin = RenderInput {
                        controls: &controls,
                        state: &st,
                        form_size: Vec2::new(DG_FORM_W, DG_FORM_H),
                        glass: true,
                        mode: RenderMode::Interactive,
                        active_tabs: &active_tabs,
                        backdrop: Backdrop {
                            paint: true,
                            color_hex: "8a6a3c".into(),
                            transparency: 0,
                            gradient_enabled: false,
                            gradient_start_hex: String::new(),
                            gradient_end_hex: String::new(),
                            gradient_direction: "South".into(),
                            image: Some((tex.id(), egui::vec2(4.0, 4.0))),
                            image_mode: Default::default(),
                            use_theme_background: false,
                            window_size: None,
                        },
                    };
                    let _ = render_form(ui, &rin);
                });
        });
        full.textures_delta.clear();
        full.shapes
    }

    /// Does this rect shape actually paint `p`? Accounts for the shape's OWN
    /// effective corner radius â egui clamps each corner to half the shorter side,
    /// so a short fill's stored radius is NOT what it renders (see the
    /// CORNER-BLEED-PLAYBOOK, Â§1.1).
    fn dg_rect_paints(rs: &egui::epaint::RectShape, p: egui::Pos2) -> bool {
        let r = rs.rect;
        if !r.contains(p) {
            return false;
        }
        let cap = (r.width() * 0.5).min(r.height() * 0.5);
        let sw = (rs.corner_radius.sw as f32).min(cap);
        if sw > 0.0 {
            let c = pos2(r.min.x + sw, r.max.y - sw);
            if p.x < c.x && p.y > c.y && (p - c).length() > sw {
                return false; // carved away by the shape's own bottom-left arc
            }
        }
        true
    }

    /// Opaque, non-backdrop fills painting `p`, described for failure output.
    fn dg_painters_at(shapes: &[egui::epaint::ClippedShape], p: egui::Pos2) -> Vec<String> {
        fn walk(s: &egui::Shape, p: egui::Pos2, out: &mut Vec<String>) {
            match s {
                egui::Shape::Vec(v) => v.iter().for_each(|s| walk(s, p, out)),
                egui::Shape::Rect(rs) => {
                    // The form backdrop legitimately covers everything â identify it
                    // by spanning the whole form, not by area heuristics (the grid
                    // itself is large).
                    let is_backdrop = rs.rect.min.x <= 0.5
                        && rs.rect.min.y <= 0.5
                        && rs.rect.max.x >= DG_FORM_W - 0.5
                        && rs.rect.max.y >= DG_FORM_H - 0.5;
                    if rs.fill.a() > 40 && !is_backdrop && dg_rect_paints(rs, p) {
                        out.push(format!(
                            "[{:.1} {:.1} {:.1} {:.1}] h={:.1} #{:02x}{:02x}{:02x}{:02x} sw={}",
                            rs.rect.min.x,
                            rs.rect.min.y,
                            rs.rect.max.x,
                            rs.rect.max.y,
                            rs.rect.height(),
                            rs.fill.r(),
                            rs.fill.g(),
                            rs.fill.b(),
                            rs.fill.a(),
                            rs.corner_radius.sw,
                        ));
                    }
                }
                _ => {}
            }
        }
        let mut out = Vec::new();
        for cs in shapes {
            walk(&cs.shape, p, &mut out);
        }
        out
    }

    /// Assert the grid's bottom-left silhouette, independent of HOW the fill is
    /// decomposed (rounded rect, arc-inset bands, anything): walking the arc,
    /// every point just OUTSIDE it must be unpainted (the wallpaper shows through
    /// the notch) and every point just INSIDE it must be painted (no square gap
    /// from over-insetting). Together these pin both regressions we hit.
    fn dg_assert_corner_silhouette(shapes: &[egui::epaint::ClippedShape], label: &str) {
        let c = pos2(DG_X + DG_R, DG_Y + DG_H - DG_R);
        let (bbox_x0, bbox_y1) = (DG_X, DG_Y + DG_H);
        let mut bleeds = Vec::new();
        let mut gaps = Vec::new();
        for deg in [120.0_f32, 125.0, 130.0, 135.0, 140.0, 145.0, 150.0] {
            let (sin, cos) = deg.to_radians().sin_cos();
            let dir = Vec2::new(cos, sin); // +y is down â this sweeps the SW arc
            // Just OUTSIDE the arc but still inside the bbox â must be unpainted.
            let po = c + dir * (DG_R + 1.2);
            if po.x >= bbox_x0 + 0.2 && po.y <= bbox_y1 - 0.2 {
                let hits = dg_painters_at(shapes, po);
                if !hits.is_empty() {
                    bleeds.push(format!(
                        "  Î¸={deg:.0}Â° ({:.1},{:.1}) â {}",
                        po.x,
                        po.y,
                        hits.join(" | ")
                    ));
                }
            }
            // Just INSIDE the arc â must be painted.
            let pi = c + dir * (DG_R - 3.0);
            if dg_painters_at(shapes, pi).is_empty() {
                gaps.push(format!("  Î¸={deg:.0}Â° ({:.1},{:.1})", pi.x, pi.y));
            }
        }
        assert!(
            bleeds.is_empty(),
            "{label}: opaque fill(s) BLEED outside the grid's bottom-left arc \
             (radius {DG_R}) â they must be clipped to the arc:\n{}",
            bleeds.join("\n")
        );
        assert!(
            gaps.is_empty(),
            "{label}: the bottom-left arc INTERIOR is not filled â the corner fill \
             over-inset and left a square gap instead of tracking the arc:\n{}",
            gaps.join("\n")
        );
    }

    /// Bleed guard, case 1 (operator report 2026-07-25): MORE rows than fit, so the
    /// last visible row is a few-pixel sliver clipped at the grid bottom. Its
    /// requested corner radius gets clamped to `height/2` â a tiny arc that pokes
    /// past the grid silhouette unless the fill follows the arc with bands.
    #[test]
    fn datagrid_bottom_left_corner_has_no_opaque_bleed() {
        let shapes = datagrid_corner_scene(20);
        dg_assert_corner_silhouette(&shapes, "overflowing rows (sliver last row)");
    }

    /// Bleed guard, case 2 (found from the operator's screen recording, where the
    /// corner was clean at some scroll offsets and bled at others): the data ends
    /// PART-WAY through the corner arc zone, so the last row's bottom lands inside
    /// the arc without touching the grid's bottom edge. Gating the arc-inset on
    /// "touches the bottom" misses exactly this case.
    #[test]
    fn datagrid_bottom_left_corner_clean_when_rows_end_inside_arc() {
        // 7 rows Ã 43px under the tall filter header end ~3px above the grid
        // bottom â i.e. inside the 15px arc zone.
        let shapes = datagrid_corner_scene(7);
        dg_assert_corner_silhouette(&shapes, "rows ending inside the arc zone");
    }

    /// Corner-bleed guard (egui 0.35 regression): every stroked rect that is
    /// concentric with the panel face must keep its corner radius STRICTLY
    /// inside the face radius. u8 radii can't express `face - 0.5`, and
    /// rounding UP pushed the dark border arc outside the face â the visible
    /// black corner arcs. Flooring keeps it inside; this test pins that.
    #[test]
    fn concentric_border_arcs_stay_inside_the_face() {
        let ctx = egui::Context::default();
        crate::paint::set_glass_style(&ctx, crate::model::GlassStyle::Neumorphic);
        let container = {
            let mut c = Control::new("PNL", ControlType::Panel, 40, 40);
            c.rect = crate::model::Rect::new(40, 40, 400, 200);
            c.set_prop("CornerRadius", crate::model::PropValue::Int(24));
            c
        };
        let controls = vec![container];
        let overrides: RefCell<Map<String, Map<String, String>>> = RefCell::new(Map::new());
        let active_tabs: crate::containers::ActiveTabs = Default::default();
        let mut input = egui::RawInput::default();
        input.screen_rect = Some(Rect::from_min_size(pos2(0.0, 0.0), Vec2::new(600.0, 300.0)));
        let mut full = ctx.run_ui(input, |root_ui| {
            egui::CentralPanel::default()
                .frame(egui::Frame::NONE)
                .show_inside(root_ui, |ui| {
                    let st = MapState_dump(&overrides);
                    let rin = RenderInput {
                        controls: &controls,
                        state: &st,
                        form_size: Vec2::new(600.0, 300.0),
                        glass: true,
                        mode: RenderMode::Interactive,
                        active_tabs: &active_tabs,
                        backdrop: Backdrop {
                            paint: true,
                            color_hex: String::new(),
                            transparency: 0,
                            gradient_enabled: false,
                            gradient_start_hex: String::new(),
                            gradient_end_hex: String::new(),
                            gradient_direction: "South".into(),
                            image: None,
                            image_mode: Default::default(),
                            use_theme_background: false,
                            window_size: None,
                        },
                    };
                    let _ = render_form(ui, &rin);
                });
        });
        full.textures_delta.clear();
        fn walk(shape: &egui::Shape, face_r: &mut Option<u8>, checked: &mut usize) {
            match shape {
                egui::Shape::Vec(v) => {
                    for s in v {
                        walk(s, face_r, checked);
                    }
                }
                egui::Shape::Rect(rs) => {
                    let panel_area = rs.rect.min.x >= 39.0
                        && rs.rect.max.x <= 441.0
                        && rs.rect.min.y >= 39.0
                        && rs.rect.max.y <= 241.0;
                    if !panel_area {
                        return;
                    }
                    if rs.fill.a() > 0 && rs.stroke.width == 0.0 {
                        *face_r = Some(rs.corner_radius.nw);
                    } else if rs.stroke.width > 0.0 {
                        if let Some(fr) = *face_r {
                            // Inside strokes may sit AT the face radius (their
                            // whole width is inside the rect); anything else
                            // must be strictly tighter than the face arc.
                            let inside_ok = rs.stroke_kind == egui::StrokeKind::Inside
                                && rs.corner_radius.nw <= fr;
                            let tighter_ok = rs.corner_radius.nw < fr;
                            assert!(
                                inside_ok || tighter_ok,
                                "border arc (r={}, {:?}) may spill outside the face arc (r={fr}) â corner bleed regression",
                                rs.corner_radius.nw,
                                rs.stroke_kind,
                            );
                            *checked += 1;
                        }
                    }
                }
                _ => {}
            }
        }
        let mut face_r = None;
        let mut checked = 0usize;
        for cs in &full.shapes {
            walk(&cs.shape, &mut face_r, &mut checked);
        }
        assert!(
            checked >= 2,
            "expected border strokes to check, saw {checked}"
        );
        println!("verified {checked} concentric border arcs inside face r={face_r:?}");
    }

    struct MapState_dump<'a>(&'a RefCell<Map<String, Map<String, String>>>);
    impl FormState for MapState_dump<'_> {
        fn live(&self, base: &Control) -> Control {
            let m = self.0.borrow();
            match m.get(&base.id) {
                Some(p) => merge_props(base, p.iter()),
                None => base.clone(),
            }
        }
    }
}

// ââ Spec 047 â Elegance on the live (interactive) surface ââââââââââââââââââââ

#[cfg(test)]
mod elegance_live_tests {
    use super::*;
    use crate::model::{Control, ControlType as CT};
    use crate::surface_theme::{ColorToken as Tok, SurfaceTheme};
    use std::sync::Arc;

    fn eleg() -> Arc<dyn SurfaceTheme> {
        crate::surface_theme::elegance()
    }
    fn glass() -> Arc<dyn SurfaceTheme> {
        crate::surface_theme::liquid_glass()
    }
    fn tok(t: Tok) -> Color32 {
        eleg().token(t).expect("Elegance answers every token")
    }

    /// The controls that hand-paint themselves on the live surface instead of
    /// going through `paint::draw_control` â each has a second, independent
    /// implementation there, which is exactly why they need their own check
    /// (spec 047 plan R-1).
    fn doubled_painters() -> Vec<(&'static str, CT)> {
        vec![
            ("NUMERICUPDOWN", CT::NumericUpDown),
            ("LISTBOX", CT::ListBox),
            ("DATAGRID", CT::DataGrid),
            ("TREEVIEW", CT::TreeView),
            ("SPLITTER", CT::Splitter),
            ("MENUBAR", CT::MenuBar),
            ("TOOLBAR", CT::ToolBar),
            ("STATUSBAR", CT::StatusBar),
        ]
    }

    /// Every rect fill painted by rendering `ct` interactively under `style`.
    fn live_fills(ct: CT, style: Arc<dyn SurfaceTheme>) -> Vec<Color32> {
        let mut c = Control::new("C", ct, 0, 0);
        c.rect = crate::model::Rect::new(10, 10, 240, 120);
        let controls = vec![c];
        let ctx = egui::Context::default();
        crate::paint::set_surface_theme(&ctx, style);
        let active = ActiveTabs::new();
        let mut input = egui::RawInput::default();
        input.screen_rect = Some(Rect::from_min_size(pos2(0.0, 0.0), Vec2::new(400.0, 300.0)));
        let mut full = ctx.run_ui(input, |root_ui| {
            egui::CentralPanel::default()
                .frame(egui::Frame::NONE)
                .show_inside(root_ui, |ui| {
                    let inp = RenderInput {
                        controls: &controls,
                        state: &DesignedState,
                        form_size: Vec2::new(400.0, 300.0),
                        glass: true,
                        mode: RenderMode::Interactive,
                        active_tabs: &active,
                        backdrop: Default::default(),
                    };
                    let _ = render_form(ui, &inp);
                });
        });
        full.textures_delta.clear();
        fn collect(s: &egui::Shape, out: &mut Vec<Color32>) {
            match s {
                egui::Shape::Vec(v) => v.iter().for_each(|s| collect(s, out)),
                egui::Shape::Rect(r) => out.push(r.fill),
                _ => {}
            }
        }
        let mut out = Vec::new();
        for cs in &full.shapes {
            collect(&cs.shape, &mut out);
        }
        out
    }

    /// T10âT12 â the live-only painters actually take the theme.
    ///
    /// Each of these eight controls paints its own background on the running
    /// form, separately from the designer face. If one is ever added back with
    /// a hard-coded colour, it will not carry a palette fill and this fails.
    #[test]
    fn elegance_reaches_every_hand_rolled_live_painter() {
        let palette = [
            tok(Tok::Card),
            tok(Tok::InputBg),
            tok(Tok::CardRaised),
            tok(Tok::Accent(crate::surface_theme::AccentName::Blue)),
        ];
        let same = |a: Color32, b: Color32| a.r() == b.r() && a.g() == b.g() && a.b() == b.b();

        let mut covered = Vec::new();
        let mut missing = Vec::new();
        for (name, ct) in doubled_painters() {
            let fills = live_fills(ct, eleg());
            if fills.iter().any(|f| palette.iter().any(|p| same(*f, *p))) {
                covered.push(name);
            } else {
                missing.push(name);
            }
        }

        println!(
            "\n  live hand-rolled painters themed by Elegance: {}/{}",
            covered.len(),
            covered.len() + missing.len()
        );
        for n in &covered {
            println!("    â {n}");
        }
        for n in &missing {
            println!("    â {n}");
        }
        println!();

        assert!(
            missing.is_empty(),
            "these live painters ignore the theme and still paint their own \
             hard-coded colours: {missing:?}"
        );
    }

    /// The same painters must be untouched under Liquid Glass (R10/AC8).
    #[test]
    fn liquid_glass_live_painters_are_unchanged_by_the_seam() {
        let card = tok(Tok::Card);
        let input_bg = tok(Tok::InputBg);
        let same = |a: Color32, b: Color32| a.r() == b.r() && a.g() == b.g() && a.b() == b.b();
        for (name, ct) in doubled_painters() {
            let fills = live_fills(ct, glass());
            assert!(
                !fills.iter().any(|f| same(*f, card) || same(*f, input_bg)),
                "{name} painted an Elegance colour under Liquid Glass"
            );
        }
    }
}

// ── Maps corner-notch measurement (CORNER-BLEED-PLAYBOOK §4) ─────────────────
//
// The operator's report: a Maps control with a corner radius shows a grey wedge
// at each rounded corner in the RUN form, easiest to see with an exaggerated
// drop shadow. Scene E reproduces `PowerDemo3/forms/Inner-Forms/maps-demo.cfrm`
// literally — the params matter, and a scaled-down guess passes while the real
// form bleeds (§4.2).
#[cfg(test)]
mod maps_corner_tests {
    use super::*;
    use crate::model::{Control, ControlType, PropValue};

    // Straight from the operator's .cfrm.
    const FORM_W: f32 = 1280.0;
    const FORM_H: f32 = 860.0;
    const MAP_X: f32 = 32.0;
    const MAP_Y: f32 = 96.0;
    const MAP_W: f32 = 880.0;
    const MAP_H: f32 = 700.0;
    const MAP_R: f32 = 51.0;
    const BACKDROP_HEX: &str = "EAEBEFFF";

    fn maps_scene() -> Vec<egui::epaint::ClippedShape> {
        let ctx = egui::Context::default();
        crate::paint::set_glass_style(&ctx, crate::model::GlassStyle::Neumorphic);

        let mut map = Control::new("MAP-1", ControlType::Maps, MAP_X as i32, MAP_Y as i32);
        map.rect = crate::model::Rect::new(
            MAP_X as i32,
            MAP_Y as i32,
            MAP_W as i32,
            MAP_H as i32,
        );
        for (k, v) in [
            ("CenterLat", "40.0000"),
            ("CenterLng", "-3.7000"),
            ("Zoom", "6"),
            ("BackgroundColor", "#FFFFFFFF"),
            ("ShadowLightColor", "#FFFFFFFF"),
            ("ShadowColor", "#000000"),
            ("ShadowDirection", "SouthEast"),
        ] {
            map.set_prop(k, PropValue::String(v.into()));
        }
        map.set_prop("ShadowEnabled", PropValue::Bool(true));
        map.set_prop("ShadowBlur", PropValue::Bool(true));
        map.set_prop("ShadowOpacity", PropValue::Int(33));
        map.set_prop("ShadowDistance", PropValue::Int(7));
        map.set_prop("ShadowBlurStrength", PropValue::Int(14));
        map.set_prop("CornerRadius", PropValue::Int(MAP_R as i64));
        map.set_prop("BackgroundGradientEnabled", PropValue::Bool(false));
        let controls = vec![map];

        let active_tabs: crate::containers::ActiveTabs = Default::default();
        let mut input = egui::RawInput::default();
        input.screen_rect = Some(Rect::from_min_size(
            pos2(0.0, 0.0),
            Vec2::new(FORM_W, FORM_H),
        ));
        let mut full = ctx.run_ui(input, |root_ui| {
            egui::CentralPanel::default()
                .frame(egui::Frame::NONE)
                .show_inside(root_ui, |ui| {
                    let rin = RenderInput {
                        controls: &controls,
                        state: &DesignedState,
                        form_size: Vec2::new(FORM_W, FORM_H),
                        glass: true,
                        mode: RenderMode::Interactive,
                        active_tabs: &active_tabs,
                        backdrop: Backdrop {
                            paint: true,
                            color_hex: BACKDROP_HEX.into(),
                            transparency: 0,
                            gradient_enabled: false,
                            gradient_start_hex: String::new(),
                            gradient_end_hex: String::new(),
                            gradient_direction: "South".into(),
                            image: None,
                            image_mode: Default::default(),
                            use_theme_background: false,
                            window_size: None,
                        },
                    };
                    let _ = render_form(ui, &rin);
                });
        });
        full.textures_delta.clear();
        full.shapes
    }

    /// Does this rect shape paint `p`, honouring its OWN effective corner radius
    /// (egui clamps each corner to half the shorter side — the stored value lies,
    /// playbook §1.1)?
    fn rect_paints(rs: &egui::epaint::RectShape, p: egui::Pos2) -> bool {
        let r = rs.rect;
        if !r.contains(p) {
            return false;
        }
        let cap = (r.width() * 0.5).min(r.height() * 0.5);
        let corners = [
            (rs.corner_radius.nw as f32, pos2(r.min.x, r.min.y), -1.0, -1.0),
            (rs.corner_radius.ne as f32, pos2(r.max.x, r.min.y), 1.0, -1.0),
            (rs.corner_radius.se as f32, pos2(r.max.x, r.max.y), 1.0, 1.0),
            (rs.corner_radius.sw as f32, pos2(r.min.x, r.max.y), -1.0, 1.0),
        ];
        for (stored, apex, sx, sy) in corners {
            let cr = stored.min(cap);
            if cr <= 0.0 {
                continue;
            }
            let c = pos2(apex.x - sx * cr, apex.y - sy * cr);
            let beyond_x = (p.x - c.x) * sx > 0.0;
            let beyond_y = (p.y - c.y) * sy > 0.0;
            if beyond_x && beyond_y && (p - c).length() > cr {
                return false; // carved away by this corner's arc
            }
        }
        true
    }

    fn tri_contains(a: egui::Pos2, b: egui::Pos2, c: egui::Pos2, p: egui::Pos2) -> bool {
        let cross = |u: egui::Vec2, v: egui::Vec2| u.x * v.y - u.y * v.x;
        let d1 = cross(b - a, p - a);
        let d2 = cross(c - b, p - b);
        let d3 = cross(a - c, p - c);
        let neg = d1 < 0.0 || d2 < 0.0 || d3 < 0.0;
        let pos = d1 > 0.0 || d2 > 0.0 || d3 > 0.0;
        !(neg && pos)
    }

    /// Every visible painter covering `p`, in PAINT ORDER, described.
    fn painters_at(shapes: &[egui::epaint::ClippedShape], p: egui::Pos2) -> Vec<String> {
        fn walk(s: &egui::Shape, clip: egui::Rect, p: egui::Pos2, out: &mut Vec<String>) {
            if !clip.contains(p) {
                return;
            }
            match s {
                egui::Shape::Vec(v) => v.iter().for_each(|s| walk(s, clip, p, out)),
                egui::Shape::Rect(rs) => {
                    if rs.fill.a() > 0 && rect_paints(rs, p) {
                        out.push(format!(
                            "RECT [{:.1} {:.1} {:.1} {:.1}] r=[{} {} {} {}] #{:02x}{:02x}{:02x}{:02x}",
                            rs.rect.min.x, rs.rect.min.y, rs.rect.max.x, rs.rect.max.y,
                            rs.corner_radius.nw, rs.corner_radius.ne,
                            rs.corner_radius.se, rs.corner_radius.sw,
                            rs.fill.r(), rs.fill.g(), rs.fill.b(), rs.fill.a(),
                        ));
                    }
                }
                egui::Shape::Mesh(m) => {
                    for tri in m.indices.chunks_exact(3) {
                        let (a, b, c) = (
                            m.vertices[tri[0] as usize],
                            m.vertices[tri[1] as usize],
                            m.vertices[tri[2] as usize],
                        );
                        if tri_contains(a.pos, b.pos, c.pos, p) && a.color.a() > 0 {
                            out.push(format!(
                                "MESH v={} #{:02x}{:02x}{:02x}{:02x} bbox=[{:.1} {:.1} {:.1} {:.1}]",
                                m.vertices.len(),
                                a.color.r(), a.color.g(), a.color.b(), a.color.a(),
                                m.calc_bounds().min.x, m.calc_bounds().min.y,
                                m.calc_bounds().max.x, m.calc_bounds().max.y,
                            ));
                            break;
                        }
                    }
                }
                _ => {}
            }
        }
        let mut out = Vec::new();
        for cs in shapes {
            walk(&cs.shape, cs.clip_rect, p, &mut out);
        }
        out
    }

    /// The colour the whole frame leaves at `p`: every painter that covers it,
    /// composited in paint order. Mesh colours are interpolated barycentrically,
    /// so a gradient across a mesh is read where it is sampled rather than at its
    /// first vertex.
    fn composite_at(shapes: &[egui::epaint::ClippedShape], p: egui::Pos2) -> Color32 {
        let mut acc = Color32::TRANSPARENT;
        for cs in shapes {
            walk_composite(&cs.shape, cs.clip_rect, p, &mut acc);
        }
        acc
    }

    fn walk_composite(shape: &egui::Shape, clip: egui::Rect, p: egui::Pos2, out: &mut Color32) {
        fn walk(s: &egui::Shape, clip: egui::Rect, p: egui::Pos2, acc: &mut Color32) {
            if !clip.contains(p) {
                return;
            }
            match s {
                egui::Shape::Vec(v) => v.iter().for_each(|s| walk(s, clip, p, acc)),
                egui::Shape::Rect(rs) => {
                    if rs.fill.a() > 0 && rect_paints(rs, p) {
                        *acc = crate::paint::composite_premultiplied_over(rs.fill, *acc);
                    }
                }
                egui::Shape::Mesh(m) => {
                    for tri in m.indices.chunks_exact(3) {
                        let (a, b, c) = (
                            m.vertices[tri[0] as usize],
                            m.vertices[tri[1] as usize],
                            m.vertices[tri[2] as usize],
                        );
                        if !tri_contains(a.pos, b.pos, c.pos, p) {
                            continue;
                        }
                        let cross = |u: egui::Vec2, v: egui::Vec2| u.x * v.y - u.y * v.x;
                        let area = cross(b.pos - a.pos, c.pos - a.pos);
                        let (wa, wb, wc) = if area.abs() < 1e-6 {
                            (1.0, 0.0, 0.0)
                        } else {
                            (
                                cross(b.pos - p, c.pos - p) / area,
                                cross(c.pos - p, a.pos - p) / area,
                                cross(a.pos - p, b.pos - p) / area,
                            )
                        };
                        let chan = |f: &dyn Fn(Color32) -> u8| {
                            (f(a.color) as f32 * wa + f(b.color) as f32 * wb
                                + f(c.color) as f32 * wc)
                                .clamp(0.0, 255.0) as u8
                        };
                        let col = Color32::from_rgba_premultiplied(
                            chan(&|c: Color32| c.r()),
                            chan(&|c: Color32| c.g()),
                            chan(&|c: Color32| c.b()),
                            chan(&|c: Color32| c.a()),
                        );
                        if col.a() > 0 {
                            *acc = crate::paint::composite_premultiplied_over(col, *acc);
                        }
                        break;
                    }
                }
                _ => {}
            }
        }
        walk(shape, clip, p, out);
    }

    /// The colour at `p` from everything painted BEFORE the control's own face —
    /// i.e. what is genuinely behind it there: the form backdrop plus its drop
    /// shadow.
    ///
    /// The cut is the first shape whose bbox fits INSIDE the control's rect. A
    /// shadow layer is always bigger than the control (it is offset and expanded)
    /// and the backdrop spans the form, so the first shape small enough to fit is
    /// the control's own content — the square basemap fill, in a Maps control's
    /// case. No painter is named, so the guard survives the face being drawn
    /// differently.
    fn behind_control_at(
        shapes: &[egui::epaint::ClippedShape],
        p: egui::Pos2,
        ctrl: egui::Rect,
    ) -> Color32 {
        fn walk(
            s: &egui::Shape,
            clip: egui::Rect,
            p: egui::Pos2,
            ctrl: egui::Rect,
            acc: &mut Color32,
            stop: &mut bool,
        ) {
            if *stop {
                return;
            }
            if let egui::Shape::Vec(v) = s {
                for s in v {
                    walk(s, clip, p, ctrl, acc, stop);
                }
                return;
            }
            let bbox = s.visual_bounding_rect();
            if bbox.is_finite()
                && bbox.width() > 0.0
                && ctrl.expand(0.5).contains_rect(bbox)
            {
                *stop = true;
                return;
            }
            let mut one = Color32::TRANSPARENT;
            walk_composite(s, clip, p, &mut one);
            if one.a() > 0 {
                *acc = crate::paint::composite_premultiplied_over(one, *acc);
            }
        }
        let mut acc = Color32::TRANSPARENT;
        let mut stop = false;
        for cs in shapes {
            walk(&cs.shape, cs.clip_rect, p, ctrl, &mut acc, &mut stop);
        }
        acc
    }

    /// Sample points inside each corner NOTCH: within the bbox, outside the arc,
    /// and clear of the restored rim that is redrawn on the arc itself.
    fn corner_notch_samples() -> Vec<(String, egui::Pos2)> {
        let (x0, y0) = (MAP_X, MAP_Y);
        let (x1, y1) = (MAP_X + MAP_W, MAP_Y + MAP_H);
        let mut out = Vec::new();
        for d in [3.0_f32, 6.0, 10.0] {
            for k in [3.0_f32, 8.0, 16.0, 26.0] {
                for (name, ax, ay, sx, sy) in [
                    ("NW", x0, y0, 1.0_f32, 1.0_f32),
                    ("NE", x1, y0, -1.0, 1.0),
                    ("SE", x1, y1, -1.0, -1.0),
                    ("SW", x0, y1, 1.0, -1.0),
                ] {
                    // Along the horizontal edge, then along the vertical one.
                    out.push((
                        format!("{name} horiz k={k} d={d}"),
                        pos2(ax + sx * k, ay + sy * d),
                    ));
                    out.push((
                        format!("{name} vert k={k} d={d}"),
                        pos2(ax + sx * d, ay + sy * k),
                    ));
                }
            }
        }
        // Keep only what is really in the notch: inside the bbox, outside the arc.
        let bbox = Rect::from_min_max(pos2(x0, y0), pos2(x1, y1));
        out.retain(|(_, p)| {
            bbox.contains(*p)
                && !crate::paint::rounded_rect_contains(
                    bbox,
                    egui::CornerRadius::same(crate::paint::cr8(MAP_R)),
                    *p,
                )
        });
        out
    }

    fn max_channel_diff(a: Color32, b: Color32) -> i32 {
        [
            (a.r() as i32 - b.r() as i32).abs(),
            (a.g() as i32 - b.g() as i32).abs(),
            (a.b() as i32 - b.b() as i32).abs(),
        ]
        .into_iter()
        .max()
        .unwrap_or(0)
    }

    /// The corner leak (operator, 2026-08-21): a Maps control with a corner
    /// radius showed a grey wedge at each rounded corner in the RUN form.
    ///
    /// The measurement that found it: the corner-notch mask repaints the FORM's
    /// flat backdrop over the notch — which also erases the control's own drop
    /// shadow, the one thing that legitimately shows there. The mask is clipped
    /// to the control's bbox, so the same shadow survived one pixel outside it,
    /// and the discontinuity along that edge is the wedge.
    ///
    /// So the guard is the discontinuity itself, not any particular painter:
    /// step across the bbox edge inside each corner's arc zone and the composited
    /// colour must barely move. That holds however the notch is repainted, and it
    /// goes red on the broken code (the jump there is ~60 levels of grey).
    #[test]
    fn a_rounded_maps_corner_keeps_the_shadow_the_mask_paints_over() {
        let shapes = maps_scene();
        let bbox = Rect::from_min_size(pos2(MAP_X, MAP_Y), Vec2::new(MAP_W, MAP_H));
        let samples = corner_notch_samples();
        assert!(
            samples.len() > 20,
            "the sample set collapsed - {} points landed in the notch",
            samples.len()
        );
        // The repaint samples the shadow on a radial grid and egui interpolates
        // between vertices, so it renders a RAMP where the layer stack is a
        // staircase: an inherent few levels of error at the dense core, which
        // more rings do not remove (measured max 20 over these 66 points, and
        // finer grids only move which point is worst). The flat backdrop repaint
        // this replaced was 60+ levels out AND hard-edged.
        const TOL: i32 = 24;
        // Where the shadow is strong, "the notch is still just the backdrop" is
        // the bug itself, and no tolerance should let it back in.
        const STRONG: i32 = 30;
        let backdrop = crate::paint::parse_color(BACKDROP_HEX);
        let mut wedges = Vec::new();
        for (label, p) in samples {
            let want = behind_control_at(&shapes, p, bbox);
            let got = composite_at(&shapes, p);
            let d = max_channel_diff(want, got);
            let shadow_depth = max_channel_diff(want, backdrop);
            let describe = |d: i32, why: &str| {
                format!(
                    "  {label} ({:.0},{:.0}): behind #{:02x}{:02x}{:02x} but painted \
                     #{:02x}{:02x}{:02x} - {d} levels apart ({why})",
                    p.x, p.y,
                    want.r(), want.g(), want.b(),
                    got.r(), got.g(), got.b(),
                )
            };
            if d > TOL {
                wedges.push(describe(d, "beyond the sampling tolerance"));
            } else if shadow_depth > STRONG && max_channel_diff(got, backdrop) * 2 < shadow_depth {
                wedges.push(describe(d, "closer to the bare backdrop than to the shadow"));
            }
        }
        assert!(
            wedges.is_empty(),
            "a rounded corner's notch must show what is BEHIND the control - the \
             backdrop AND the shadow the control casts on it. These notch points \
             do not, which is the grey wedge:\n{}",
            wedges.join("\n")
        );
    }

    /// Diagnostic: every STROKE in the scene, with its rect, radius, colour and
    /// clip. A thin line hugging a corner is a stroke, not a fill, so the notch
    /// dump above cannot see it.
    #[test]
    fn measure_maps_corner_strokes() {
        if std::env::var_os("COBOLT_MAPS_CORNER_MEASURE").is_none() {
            return;
        }
        fn walk(s: &egui::Shape, clip: egui::Rect, out: &mut Vec<String>) {
            match s {
                egui::Shape::Vec(v) => v.iter().for_each(|s| walk(s, clip, out)),
                egui::Shape::Rect(rs) if rs.stroke.width > 0.0 => out.push(format!(
                    "STROKE rect=[{:.1} {:.1} {:.1} {:.1}] r=[{} {} {} {}] w={:.2} \
                     #{:02x}{:02x}{:02x}{:02x} kind={:?} clip=[{:.1} {:.1} {:.1} {:.1}]",
                    rs.rect.min.x, rs.rect.min.y, rs.rect.max.x, rs.rect.max.y,
                    rs.corner_radius.nw, rs.corner_radius.ne,
                    rs.corner_radius.se, rs.corner_radius.sw,
                    rs.stroke.width,
                    rs.stroke.color.r(), rs.stroke.color.g(),
                    rs.stroke.color.b(), rs.stroke.color.a(),
                    rs.stroke_kind,
                    clip.min.x, clip.min.y, clip.max.x, clip.max.y,
                )),
                _ => {}
            }
        }
        let shapes = maps_scene();
        let mut out = Vec::new();
        for cs in &shapes {
            walk(&cs.shape, cs.clip_rect, &mut out);
        }
        println!("--- {} stroked shapes", out.len());
        for (i, s) in out.iter().enumerate() {
            println!("  {i:2}. {s}");
        }
    }

    /// Diagnostic: print, in paint order, everything that covers a point inside
    /// each corner NOTCH (inside the control's bbox, outside its arc). Run with
    /// `--nocapture` while chasing a corner artefact.
    #[test]
    fn measure_maps_corner_notch_painters() {
        if std::env::var_os("COBOLT_MAPS_CORNER_MEASURE").is_none() {
            return;
        }
        let shapes = maps_scene();
        let (x0, y0) = (MAP_X, MAP_Y);
        let (x1, y1) = (MAP_X + MAP_W, MAP_Y + MAP_H);
        for (name, ax, ay, sx, sy) in [
            ("NW", x0, y0, 1.0_f32, 1.0_f32),
            ("NE", x1, y0, -1.0, 1.0),
            ("SE", x1, y1, -1.0, -1.0),
            ("SW", x0, y1, 1.0, -1.0),
        ] {
            for d in [2.0_f32, 6.0, 12.0] {
                let p = pos2(ax + sx * d, ay + sy * d);
                println!("--- {name} notch d={d} at ({:.1},{:.1})", p.x, p.y);
                for (i, s) in painters_at(&shapes, p).iter().enumerate() {
                    println!("    {i:2}. {s}");
                }
            }
        }
        // And one point on the face, well inside the arc, for contrast.
        let p = pos2(x0 + MAP_W * 0.5, y0 + MAP_H * 0.5);
        println!("--- CENTRE at ({:.1},{:.1})", p.x, p.y);
        for (i, s) in painters_at(&shapes, p).iter().enumerate() {
            println!("    {i:2}. {s}");
        }
    }
}
