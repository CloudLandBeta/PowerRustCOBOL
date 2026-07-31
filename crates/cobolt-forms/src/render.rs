// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Unified form rendering engine (spec 017).
//!
//! One renderer for **every** surface — the Form Designer canvas, the live
//! preview, the running (interpreted) form, and the compiled binary — so the same
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
//! widgets (editable text, combo popups, slider drag, …) layer on top in
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
    /// Form background colour as `#RRGGBB[AA]` (or empty/unset).
    pub color_hex: String,
    /// Form transparency 0–100 (0 = opaque).
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
    /// background image — exactly as on the designer canvas.
    pub use_theme_background: bool,
    /// The host window's client size, when the backdrop belongs to a real
    /// window the user can maximize or drag bigger. The backdrop then covers
    /// `max(form_size, window_size)` on each axis: the gradient or background
    /// image stretches across the WHOLE window when it is enlarged, while the
    /// controls stay at their designed size — and a window dragged SMALLER
    /// than the form keeps a form-sized backdrop rather than shrinking with
    /// the window. `None` (designer canvas, previews) pins the backdrop to
    /// the form, so the designed extent stays visible while editing.
    pub window_size: Option<Vec2>,
}

impl Default for Backdrop {
    fn default() -> Self {
        Backdrop {
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
    /// UI events from interactive controls (clicks, changes, focus, keys, …).
    pub events: Vec<UiEvent>,
    /// Live property updates to apply back to the caller's state: (id, key, value).
    pub prop_updates: Vec<(String, String, String)>,
    /// Each control's on-screen rect, so the designer can position its overlay
    /// (selection handles, badges, drop hints) without re-deriving geometry.
    pub control_rects: HashMap<String, Rect>,
}

/// The size the backdrop covers: the form's own size, stretched to the host
/// window on each axis where the window is BIGGER (maximized, or the border
/// dragged out — the gradient or background image then fills the whole
/// window while the controls keep their designed size), and never smaller
/// than the form (a window dragged in keeps a form-sized backdrop, which the
/// form scrolls inside). `None` — the designer canvas and previews — pins the
/// backdrop to the form so its designed extent stays visible while editing.
pub fn backdrop_size(form_size: Vec2, window_size: Option<Vec2>) -> Vec2 {
    window_size.map_or(form_size, |w| form_size.max(w))
}

/// What the backdrop pass painted, so the caller can reuse it — the
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
/// ONE implementation, so every surface shows the same backdrop — the
/// designer, the preview, the running form, a compiled binary AND the static
/// face a window effect animates. The effect face used to paint the solid
/// colour only, so a form with a gradient or a background image was revealed
/// bare and then jumped to its real background the moment the animation
/// handed over to the live UI (operator report, 2026-07-30).
pub fn paint_backdrop(painter: &egui::Painter, rect: Rect, backdrop: &Backdrop) -> BackdropPaint {
    let bg = backdrop_color(&backdrop.color_hex, backdrop.transparency);
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

/// Whether a control's drawn content (image, film, glass card, chart, …) should be
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
/// exceeds the parent's border is cut by the parent — not by the child's own bounds
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
/// ── CORNER GUARDIAN RULE (do not regress) ─────────────────────────────────────
/// The notch mask exists ONLY to cut child content that bled past a rounded
/// corner. It must therefore touch a corner **only when a descendant actually
/// overlaps that corner's notch square** — never "all four corners because the
/// container happens to have children". Painting the backdrop over a corner no
/// child reaches destroys the container's OWN rounded corner (fill / rim / shadow),
/// which shows up as a transparent or discoloured crescent — the exact bug this
/// function was added to prevent. See `corner_notch_guardian_*` regression tests.
///
/// So: keep a corner's radius only when some descendant of `container_idx` overlaps
/// its notch square; otherwise zero it, leaving that corner untouched. When every
/// corner is clean the returned rounding is `ZERO` and the mask early-returns.
///
/// Both notch-mask call sites (runtime `mask_container_notches` and the designer's
/// notch loop) MUST route through this — do not call `draw_container_notch_mask`
/// with a blanket `CornerRadius::same(rad)`.
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

fn mask_container_notches(
    painter: &egui::Painter,
    input: &RenderInput<'_>,
    controls: &[Control],
    out: &RenderOutput,
    image: Option<(egui::TextureId, Rect)>,
    img_alpha: u8,
    bg: Color32,
    gradient: Option<(Rect, Color32, Color32, &str)>,
) {
    // `controls` is the EFFECTIVE (post-`expand_repeating_groups`) list the render
    // loop drew from — NOT `input.controls`. The notch-mask guardian
    // (`corner_notch_rounding`) decides which corners to mask by looking each
    // descendant's rect up in `out.control_rects`, which is keyed by the drawn
    // (instance) ids. Walking the original template here would leave a databound
    // container's expanded card instances invisible to the guardian, so it would
    // mask nothing and the card content bleeds past the container arc (spec 015/024
    // repeating groups × the spec 017 notch mask).
    for (idx, base) in controls.iter().enumerate() {
        if !matches!(
            base.control_type,
            ControlType::GroupBox | ControlType::Panel
        ) {
            continue;
        }
        if !containers::has_descendants(controls, idx) {
            continue;
        }
        if base.parent.is_some() {
            // A nested rounded container sits on top of another container. Its
            // notches must reveal that parent surface, not the form backdrop.
            // Repainting with the form backdrop cuts a dark/background-pattern
            // hole through the parent panel (visible in the designer grid). A
            // true fix needs parent-surface/offscreen compositing; until then,
            // only form-level containers use the global backdrop mask.
            continue;
        }
        if !input.state.visible(base) || !containers::is_visible(controls, idx, input.active_tabs) {
            continue;
        }
        let live = input.state.live(base);
        let rad = crate::paint::corner_radius(&live);
        if rad < 0.5 {
            continue;
        }
        let Some(&screen) = out.control_rects.get(&live.id) else {
            continue;
        };
        // Only mask corners a child actually reaches; clean corners stay untouched.
        let rounding = corner_notch_rounding(screen, rad, controls, idx, &out.control_rects);
        crate::paint::draw_container_notch_mask(
            painter, screen, rounding, bg, gradient, image, img_alpha,
        );
        // The notch mask repaints the backdrop over the corner arcs it touched,
        // erasing the container's own border/rim there. Restore the rim on exactly
        // those corners (`rounding`) — restoring an unmasked corner would
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
/// authoritative — including **0**, which renders NO card at all (an empty data
/// source shows nothing; task 3). An **unbound** template group falls back to
/// `PreviewItemCount` (clamped ≥1) so the designer always has one card to edit.
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
/// `"<group>.<group>-<inst>"`. Every instance — including the first — is prefixed,
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
/// `[(inst-1)·DUR, inst·DUR]`.
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
            // Invisible until its turn, then fade 0→1 in place.
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
    // Indices belonging to ANY repeating group (the group + its descendants) —
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
        // (task 2) — full HEIGHT+padding down for Vertical, full WIDTH+padding
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
    let mut out = RenderOutput::default();
    let origin = ui.min_rect().min;
    let painter = ui.painter().clone();

    // ── Backdrop: solid colour, gradient, theme art or image. ─────────────────
    let form_rect = Rect::from_min_size(origin, input.form_size);
    // The backdrop covers the form, and stretches to the host window when the
    // user maximizes it or drags it bigger — the controls keep their designed
    // size, only the background follows the window. A window dragged SMALLER
    // than the form keeps the form-sized backdrop (the form scrolls inside
    // it) rather than cropping the background to the window.
    let backdrop_rect = Rect::from_min_size(
        origin,
        backdrop_size(input.form_size, input.backdrop.window_size),
    );
    let painted = paint_backdrop(&painter, backdrop_rect, &input.backdrop);
    let bg = painted.bg;
    let backdrop_gradient = painted.gradient;
    let backdrop_img_alpha = painted.image_alpha;
    let backdrop_img = painted.image;
    // The notch mask is drawn *after* children. If the form background is
    // translucent, repainting `bg` would darken the corner wedges; skipping it
    // would leave rectangular child bleed visible. Use the effective one-pass
    // colour over the panel fill instead.
    let notch_bg = crate::paint::composite_premultiplied_over(bg, ui.visuals().panel_fill);
    // ── Controls: designer order, clipped + faded by container ancestry. ──────
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
    // other control: (id, items, header rect, current value).
    let mut open_combos: Vec<(String, Vec<String>, Rect, String)> = Vec::new();
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
        // control each frame — a control hidden THIS frame must still fire
        // its onVisibleChanged — so this runs before the visibility skips.
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
        // The card's FINAL (un-animated) screen rect — used to decide whether a
        // Deal card is off-screen (no phantom fly-in) before adding the effect.
        let final_screen = Rect::from_min_size(
            origin + Vec2::new(r.x as f32 + tf.dx - scroll.x, r.y as f32 + tf.dy - scroll.y),
            Vec2::new(r.w as f32, r.h as f32),
        );
        // Clip to ancestor container content areas (rounded clipping is cosmetic;
        // egui clips to the axis-aligned rect — spec 012/016). Start from the whole
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
            // (text edit, slider drag, combo popup, …) ported from the run path.
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
            // image (not a placeholder) — same as the designer canvas.
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

    // ── Corner-notch masks: cut any child content that bled past a rounded
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
    );
    draw_deferred_groupbox_captions(&painter, input, &out);
    draw_deferred_tabcontrol_tabs(&painter, input, &out);

    // ── Second pass: open ComboBox dropdowns float above everything. ──────────
    for (cid, items, header, cur) in open_combos {
        match crate::paint::glass_combo_popup(ui, &cid, header, &items, &cur) {
            Some(crate::paint::GlassComboAction::Select(val)) => {
                out.prop_updates
                    .push((cid.clone(), "Value".to_owned(), val.clone()));
                out.events.push(UiEvent::change(&cid, &val));
                out.events.push(UiEvent::ev(&cid, "onSelectedIndexChanged"));
                let open_id = egui::Id::new(("rt_ctrl", cid.as_str())).with("combo_open");
                ui.data_mut(|d| d.insert_temp(open_id, false));
            }
            Some(crate::paint::GlassComboAction::Close) => {
                let open_id = egui::Id::new(("rt_ctrl", cid.as_str())).with("combo_open");
                ui.data_mut(|d| d.insert_temp(open_id, false));
            }
            None => {}
        }
    }
    out
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
/// canvas, not clipped to the form bounds — the designer's long-standing
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
        // BORDER path (spec 017) — see `render_form`.
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
        // corners here — captured after the shadow, so re-blitting the notch later
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
        _ => area, // Stretch / Tile → fill the area
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
        match k.as_str() {
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
            "Width" => {
                if let Ok(n) = v.trim().parse::<f32>() {
                    c.rect.w = n.round() as i32;
                }
            }
            "Height" => {
                if let Ok(n) = v.trim().parse::<f32>() {
                    c.rect.h = n.round() as i32;
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
        "VISIBLE" => {
            if ctrl.visible {
                "1".to_owned()
            } else {
                "0".to_owned()
            }
        }
        "ENABLED" => {
            if ctrl.enabled {
                "1".to_owned()
            } else {
                "0".to_owned()
            }
        }
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
    /// A valueless event (`onClick`, `onGotFocus`, `onTick`, …).
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
    /// Any event carrying a payload (node text, tab index, cell coordinates…).
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
    /// Dark on the left edge → for the vertical shadow cast rightward by frozen
    /// columns.
    Left,
    /// Dark on the top edge → for the horizontal shadow cast downward by the
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
/// so the leading and trailing margins are equal (half a cell) — an even
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

/// Vertical inset of a rounded-rect silhouette at horizontal position `x` — the
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
/// case). The DataGrid is a leaf drawn directly, and — when nested inside a
/// translucent panel — the backdrop notch-mask can't be used, so preventing the
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
/// — the data-driven loop ignores any without a bound handler. Mirrors the IDE's
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
        if clicked
            && want("onClick")
            && ui
                .ctx()
                .memory(|m| m.data.get_temp::<bool>(press_mem).unwrap_or(false))
        {
            out.events.push(UiEvent::click(id));
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
/// `onMove`/`onMoved` for position. Fires regardless of Enabled — geometry is
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

fn cursor_icon_for(value: &str) -> Option<egui::CursorIcon> {
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
/// is on, outline every structural sub-component of the grid — the whole viewport,
/// the header band, the body band, each column (frozen + scrollable), each visible
/// row, each visible cell, the frozen-column band, and the vertical scrollbar
/// track — each in a distinct colour with a small label, so a mis-sized or
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
    const C_GRID: Color32 = Color32::from_rgb(255, 64, 64); // red    – whole grid
    const C_HEADER: Color32 = Color32::from_rgb(64, 220, 96); // green  – header
    const C_BODY: Color32 = Color32::from_rgb(80, 160, 255); // blue   – body
    const C_COL: Color32 = Color32::from_rgb(255, 200, 32); // amber  – columns
    const C_ROW: Color32 = Color32::from_rgb(210, 96, 255); // magenta – rows
    const C_CELL: Color32 = Color32::from_rgb(0, 220, 220); // cyan   – cells
    const C_FROZEN: Color32 = Color32::from_rgb(255, 140, 0); // orange – frozen band

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
    // slice — otherwise the overlay (an unclipped foreground layer, unlike the
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
/// 1. **Gapless** — the returned rects tile `r`'s vertical span exactly. Any gap,
///    even a sub-pixel one, lets the grid's own background show through as a thin
///    seam. (That was a real bug: a `> min.y + eps` guard skipped the strip above
///    the arc zone when it was thinner than `eps`, revealing the yellow underlay
///    as a 1px line that flashed on and off with the fractional scroll offset.)
/// 2. **Inside the arc** — no rect crosses the corner arc, so nothing bleeds into
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
    // Part ABOVE the arc zone: one plain full-width rect. No `eps` threshold here —
    // any positive height must be painted or it becomes a visible seam (see 1.).
    let zone_top = (screen.max.y - r_arc).max(r.min.y);
    if zone_top > r.min.y {
        out.push(Rect::from_min_max(r.min, pos2(r.max.x, zone_top)));
    }
    // Corner zone: 1px bands, each inset by the arc at the band BOTTOM (its widest
    // point → never crosses the arc; over-insets by <1px, which is invisible).
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
/// pixel-for-pixel; only the interaction (text edit, drag, popup, …) is added.
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
    // The form's effective (opaque) backdrop colour — what a translucent glass
    // control shows through, so colours that must stay legible on the face can
    // be measured against what the eye actually sees.
    form_bg: Color32,
    out: &mut RenderOutput,
    open_combos: &mut Vec<(String, Vec<String>, Rect, String)>,
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
        // (default 200 ms) — not a hardcoded constant.
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
                out.events.push(UiEvent::ev(id, "onCheckedChanged"));
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
                out.events.push(UiEvent::ev(id, "onCheckedChanged"));
                out.events.push(UiEvent::ev(id, "onValueChanged"));
            }
        }
        CT::TextBox => {
            // Face via the shared renderer; static caption blanked so the editable
            // overlay shows the value.
            let mut drawn = ctrl.clone();
            drawn
                .properties
                .insert("Text".to_owned(), crate::PropValue::String(String::new()));
            paint::draw_control(&painter, screen.min, &drawn, false, glass, alpha, 1.0, None);
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
            // Placeholder shown while the box is empty — same font as the
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
            // Justified lays out left in the editor — egui's TextEdit cannot
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
            // alignment — the box's clip rect then reveals the correct window.
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
            let resp = if multiline {
                // egui's multiline editor auto-grows to its content, so it would
                // spill past the TextBox's fixed height (and its rounded bottom).
                // Host it in a scroll area clipped to the field so extra rows scroll
                // instead of overflowing — the box keeps its designed height.
                ui.scope_builder(egui::UiBuilder::new().max_rect(edit_rect), |ui| {
                    ui.set_clip_rect(edit_rect);
                    ui.visuals_mut().text_cursor.stroke.color = caret_col;
                    egui::ScrollArea::vertical()
                        .auto_shrink([false, false])
                        .max_height(edit_rect.height())
                        .show(ui, |ui| {
                            ui.add(
                                egui::TextEdit::multiline(&mut buf)
                                    .id(ctrl_id)
                                    .frame(egui::Frame::NONE)
                                    .interactive(enabled)
                                    .desired_rows(1)
                                    .desired_width(edit_rect.width())
                                    .horizontal_align(halign)
                                    .font(edit_font.clone())
                                    .hint_text(
                                        egui::RichText::new(hint_text.as_str())
                                            .color(hint_col),
                                    )
                                    .text_color(txt_col),
                            )
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
                    ui.put(
                        single_rect,
                        egui::TextEdit::singleline(&mut buf)
                            .id(ctrl_id)
                            .frame(egui::Frame::NONE)
                            .interactive(enabled)
                            .horizontal_align(halign)
                            .vertical_align(valign)
                            .font(edit_font.clone())
                            .hint_text(
                                egui::RichText::new(hint_text.as_str()).color(hint_col),
                            )
                            .text_color(txt_col),
                    )
                })
                .inner
            };
            if resp.changed() {
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
            // button is up — robust regardless of whether `drag_released` fired.
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
        CT::NumericUpDown => {
            paint::draw_glass_auto(
                &painter,
                screen,
                Color32::from_rgb(30, 40, 80),
                paint::corner_radius(ctrl),
                false,
                alpha,
            );
            let min = sv(ctrl, "Minimum").parse::<f64>().unwrap_or(0.0);
            let max = sv(ctrl, "Maximum").parse::<f64>().unwrap_or(100.0);
            let step = sv(ctrl, "Step").parse::<f64>().unwrap_or(1.0).max(0.0001);
            let mut val = sv(ctrl, "Value").parse::<f64>().unwrap_or(min);
            let resp = ui.put(
                screen,
                egui::DragValue::new(&mut val).range(min..=max).speed(step),
            );
            focus_keyboard_events(ui, &resp, id, out, &bound);
            if resp.changed() && enabled {
                let s = format!("{val}");
                out.prop_updates
                    .push((id.to_owned(), "Value".to_owned(), s.clone()));
                out.events.push(UiEvent::change(id, &s));
            }
        }
        CT::ComboBox => {
            // Glass header now; the popup is drawn in the engine's second pass so
            // it floats above every control. Open state lives in egui memory.
            let cur = sv(ctrl, "Value");
            let sel = if cur.is_empty() {
                sv(ctrl, "Items").lines().next().unwrap_or("").to_owned()
            } else {
                cur
            };
            let open_id = ctrl_id.with("combo_open");
            let is_open = ui.data(|d| d.get_temp::<bool>(open_id)).unwrap_or(false);
            // onDropDownClosed (spec 021 T12): the popup pass flips the open
            // flag when an item is picked or the click lands outside; compare
            // against last frame's state here.
            let was_open_id = ctrl_id.with("combo_was_open");
            let was_open = ui.data(|d| d.get_temp::<bool>(was_open_id)).unwrap_or(false);
            if was_open && !is_open {
                out.events.push(UiEvent::ev(id, "onDropDownClosed"));
            }
            if was_open != is_open {
                ui.data_mut(|d| d.insert_temp(was_open_id, is_open));
            }
            if paint::glass_combo_header(
                &painter, ui, screen, ctrl_id, &sel, is_open, enabled, alpha,
            ) {
                let now = !is_open;
                ui.data_mut(|d| d.insert_temp(open_id, now));
                if now {
                    out.events.push(UiEvent::ev(id, "onDropDown"));
                }
            }
            if ui.data(|d| d.get_temp::<bool>(open_id)).unwrap_or(false) {
                let items: Vec<String> = sv(ctrl, "Items").lines().map(|l| l.to_owned()).collect();
                open_combos.push((id.to_owned(), items, screen, sv(ctrl, "Value")));
            }
        }
        CT::ListBox => {
            paint::draw_glass_auto(
                &painter,
                screen,
                Color32::from_rgb(30, 40, 80),
                paint::corner_radius(ctrl),
                false,
                alpha,
            );
            let items: Vec<String> = sv(ctrl, "Items").lines().map(|l| l.to_owned()).collect();
            let cur = sv(ctrl, "Value");
            let mut picked: Option<String> = None;
            let mut double_picked: Option<String> = None;
            ui.scope_builder(egui::UiBuilder::new().max_rect(screen), |ui| {
                if !enabled {
                    ui.disable();
                }
                egui::ScrollArea::vertical()
                    .id_salt(ctrl_id)
                    .max_height(screen.height())
                    .show(ui, |ui| {
                        for item in &items {
                            let resp = ui.selectable_label(&cur == item, item);
                            if resp.clicked() {
                                picked = Some(item.clone());
                            }
                            if resp.double_clicked() {
                                double_picked = Some(item.clone());
                            }
                        }
                    });
            });
            if let Some(item) = picked {
                out.prop_updates
                    .push((id.to_owned(), "Value".to_owned(), item.clone()));
                out.events.push(UiEvent::change(id, &item));
                out.events.push(UiEvent::ev(id, "onSelectedIndexChanged"));
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
                            egui::Button::new("◀").frame(false),
                        );
                        let next = ui.put(
                            Rect::from_min_size(
                                area_pos + vec2(paint::CAL_W - paint::CAL_CELL, 0.0),
                                vec2(paint::CAL_CELL, paint::CAL_NAV_H),
                            ),
                            egui::Button::new("▶").frame(false),
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
            let header_bg = paint::parse_hex(&sv(ctrl, "HeaderBackgroundColor"))
                .unwrap_or(Color32::from_rgb(60, 66, 96));
            let header_fg = paint::parse_hex(&sv(ctrl, "HeaderForegroundColor"))
                .unwrap_or(Color32::from_rgb(235, 238, 250));
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
            let grid_bg = grid_bg_underlay.unwrap_or(Color32::from_rgb(26, 32, 58));
            let alt_bg_base = paint::parse_hex(&sv(ctrl, "AlternatingRowColor"))
                .unwrap_or(Color32::from_rgb(38, 44, 72));
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

            paint::draw_glass_auto_bg(
                &painter,
                screen,
                grid_bg,
                grid_bg_underlay,
                paint::corner_radius(ctrl),
                false,
                alpha,
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
            // deltas — the ancestor ScrollArea reads `smooth_scroll_delta` in its
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
                            false // consumed by the DataGrid — do not bubble up
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
                            "‹",
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
                            "›",
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
            // the rounded background — this is what makes a DataGrid render rounded
            // even when nested inside another container (where the backdrop
            // notch-mask can't be used).
            //
            // Clamping is essential: the last row's rect usually extends *past*
            // `screen.max.y` and is cut square by the body clip. CornerRadius that
            // off-clip rect is invisible — so we intersect with the grid rect first,
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
                    // Cell background fallback chain: value-rule colour → column
                    // colour → the grid's own appearance BackgroundColor (its flat
                    // underlay). The last step matters for cells whose visible
                    // content doesn't cover the whole cell — a framed "pill" column
                    // (the inner-shape is inset), or plain text. Without it those
                    // gaps fall through to the frosted glass sheen and read grey
                    // instead of the solid appearance colour the user configured.
                    // When the grid is on the default (translucent) background,
                    // `grid_bg_underlay` is `None` and the gap stays glass.
                    // A fully-transparent colour (the column default `#00000000`)
                    // is "unset", not "paint nothing" — filter it out at each step
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
                                // image shows at the control opacity alone — a
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
                                // Missing/undecodable image → show the path so the
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
                            "▼",
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
                    // — a centred stroke spills half a pixel past the edge, which
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
            paint::draw_glass_auto(
                &painter,
                screen,
                Color32::from_rgb(28, 36, 64),
                paint::corner_radius(ctrl),
                false,
                alpha,
            );
            let fg = Color32::from_rgb(220, 226, 250);
            let selected = sv(ctrl, "SelectedNode");
            let mut y = screen.min.y + 12.0;
            for (line_index, line) in sv(ctrl, "Items").lines().enumerate() {
                if y > screen.max.y {
                    break;
                }
                let depth = (line.len() - line.trim_start().len()) / 2;
                let text = line.trim();
                if text.is_empty() {
                    continue;
                }
                let row = Rect::from_min_max(
                    pos2(screen.min.x + 2.0, y - 9.0),
                    pos2(screen.max.x - 2.0, y + 9.0),
                );
                // spec 021 T12: node selection. Rows are click targets; the
                // picked node lands in SelectedNode and fires the node events.
                let resp = ui.interact(row, ctrl_id.with(("tv-node", line_index)), Sense::click());
                let is_selected = !selected.is_empty() && selected == text;
                if is_selected {
                    painter.rect_filled(
                        row,
                        3.0,
                        Color32::from_rgba_premultiplied(70, 110, 200, 70),
                    );
                }
                if resp.clicked() && enabled {
                    out.prop_updates
                        .push((id.to_owned(), "SelectedNode".to_owned(), text.to_owned()));
                    out.events.push(UiEvent::with_value(id, "onNodeClick", text));
                    if !is_selected {
                        out.events
                            .push(UiEvent::with_value(id, "onNodeSelect", text));
                    }
                }
                if resp.double_clicked() && enabled {
                    out.events
                        .push(UiEvent::with_value(id, "onNodeDblClick", text));
                    out.events
                        .push(UiEvent::with_value(id, "onNodeDoubleClick", text));
                }
                painter.text(
                    pos2(screen.min.x + 8.0 + depth as f32 * 16.0, y),
                    Align2::LEFT_CENTER,
                    format!("• {text}"),
                    FontId::proportional(12.0),
                    fg,
                );
                y += 18.0;
            }
        }
        CT::Splitter => {
            let horiz = !sv(ctrl, "Orientation").starts_with('V');
            paint::draw_glass_auto(
                &painter,
                screen,
                Color32::from_rgb(60, 66, 96),
                paint::corner_radius(ctrl),
                false,
                alpha,
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
            let menu_bg = ctrl
                .get_prop("BackgroundColor")
                .map(|v| paint::parse_color(v.as_str()))
                .unwrap_or(Color32::TRANSPARENT);
            if menu_bg.a() > 0 {
                paint::draw_glass_auto(
                    &painter,
                    screen,
                    menu_bg,
                    paint::corner_radius(ctrl),
                    false,
                    alpha,
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

                    let resp = ui.allocate_rect(label_rect, egui::Sense::click());
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
                                                        egui::RichText::new("▸").color(item_fg),
                                                    );
                                                }
                                            });
                                            let row_resp = ui.interact(
                                                item_resp.response.rect,
                                                egui::Id::new(("mi", &item.id)),
                                                egui::Sense::click(),
                                            );
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
                    "☰ MenuBar (empty)",
                    FontId::proportional(12.0),
                    fg,
                );
            }
        }
        CT::ToolBar | CT::StatusBar => {
            paint::draw_glass_auto(
                &painter,
                screen,
                Color32::from_rgb(40, 46, 76),
                paint::corner_radius(ctrl),
                false,
                alpha,
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
            // Render through `draw_control` with a pre-loaded texture — the SAME
            // path the designer canvas uses — so the image is tinted/framed
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
            // (codegen agrees — it seeds WS-<timer>-ENABLED from this property).
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
                let last = match ui.ctx().memory(|m| m.data.get_temp::<f64>(mem)) {
                    None => {
                        ui.ctx().memory_mut(|m| m.data.insert_temp(mem, now));
                        now
                    }
                    Some(last) if now - last >= interval_s => {
                        out.events.push(UiEvent::ev(id, "onTick"));
                        ui.ctx().memory_mut(|m| m.data.insert_temp(mem, now));
                        now
                    }
                    Some(last) => last,
                };
                // Wake exactly when the NEXT tick is due — not every interval/4.
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
            // Charts render through the SAME path as the designer (draw_control →
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
            // Non-visual — nothing to draw.
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
        // Faces whose designer rendering IS the real face (Label, Panel, Shape, …).
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

    // ── CORNER GUARDIAN regression tests ─────────────────────────────────────
    // These pin the rule that the notch mask must only touch corners a child
    // actually reaches; if they fail, a clean container corner is being masked
    // (painted over) again — the bug corner_notch_rounding was added to stop.

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
        // Child parked in the middle — reaches no corner.
        rects.insert(
            "CHILD".to_string(),
            Rect::from_min_size(pos2(80.0, 60.0), Vec2::new(40.0, 20.0)),
        );
        let r = corner_notch_rounding(cont, 20.0, &controls, 0, &rects);
        assert_eq!(
            r,
            egui::CornerRadius::ZERO,
            "no child at any corner ⇒ NOTHING masked (panel keeps its own corners)"
        );
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
        assert_eq!(r.sw, 20, "child in bottom-left ⇒ SW masked");
        assert_eq!(r.nw, 0, "NW is clean ⇒ untouched");
        assert_eq!(r.ne, 0, "NE is clean ⇒ untouched");
        assert_eq!(r.se, 0, "SE is clean ⇒ untouched");
    }

    /// Which of the four corner squares of a 200×150 / r=20 panel a restore stroke
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
        let full = ctx.run_ui(input, |root_ui| {
            let painter = root_ui.painter_at(Rect::from_min_size(pos2(0.0, 0.0), Vec2::new(600.0, 400.0)));
            crate::paint::restore_container_outline(&painter, &panel, rect, r, true, masked);
        });
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
        // (now per-corner) notch mask left clean — a light spur at the corner
        // (visible on databound DataGrids / dropshadowed cards after egui 0.35).
        let rect = Rect::from_min_size(pos2(0.0, 0.0), Vec2::new(200.0, 150.0));
        let r = 20.0;
        let cr = crate::paint::cr8(r);

        // Only the SW corner masked ⇒ only SW restored.
        let sw_only = egui::CornerRadius { nw: 0, ne: 0, se: 0, sw: cr };
        let hit = restored_corners(rect, r, sw_only);
        assert_eq!(
            hit.into_iter().collect::<Vec<_>>(),
            vec!["sw"],
            "restore must touch ONLY the masked (SW) corner, never the clean ones",
        );

        // Nothing masked ⇒ nothing restored (no spur on a container with a clean rim).
        let hit = restored_corners(rect, r, egui::CornerRadius::ZERO);
        assert!(
            hit.is_empty(),
            "no corner masked ⇒ restore must be a no-op, saw {hit:?}",
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
        // A separator in the middle clears the corners → untouched.
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
        // 3 instances × 2 controls.
        assert_eq!(expanded.len(), 6);
        // Every instance — including the first — uses the group-prefixed scheme
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
        // Deal off-screen: no phantom fly-in — placed at final immediately.
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
        // No DataSource → ItemCount is ignored; PreviewItemCount governs (default 1).
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
        // 100px wide, nominal 12 → round(8.33)=8 cells, spacing 12.5.
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
        // Unset / pure black ⇒ default dark navy (matches preview + run).
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
        let (border, rad) = mk(&controls).expect("rounded parent → border clip");
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
            "clip must match fixed_panel ∩ (card_content - scroll)"
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
        let _ = ctx.run_ui(Default::default(), |root_ui| {
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
        });
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
        let _ = ctx.run_ui(Default::default(), |root_ui| {
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
        });
    }

    /// Operator rule (2026-07-30): the form keeps the size its author gave
    /// it, but its gradient / background image follows the WINDOW — over the
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
        // cropped to the window — the form scrolls inside it.
        assert_eq!(backdrop_size(form, Some(Vec2::new(400.0, 300.0))), form);
        // Mixed axes are handled independently.
        assert_eq!(
            backdrop_size(form, Some(Vec2::new(1600.0, 300.0))),
            Vec2::new(1600.0, 600.0)
        );
        println!(
            "backdrop: form {form:?}, maximized ⇒ {:?}, shrunk ⇒ {:?}, mixed ⇒ {:?}",
            backdrop_size(form, Some(big)),
            backdrop_size(form, Some(Vec2::new(400.0, 300.0))),
            backdrop_size(form, Some(Vec2::new(1600.0, 300.0)))
        );
    }

    #[test]
    fn render_form_static_smoke() {
        // Headless: a form with a Panel ⊃ Button renders without panic and reports
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
        let _ = ctx.run_ui(Default::default(), |root_ui| {
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
                        color_hex: "#00000000".into(),
                        ..Default::default()
                    },
                };
                captured = Some(render_form(ui, &input));
            });
        });
        let out = captured.expect("rendered");
        assert!(out.control_rects.contains_key("Pnl"));
        assert!(out.control_rects.contains_key("Btn"));
    }

    #[test]
    fn engine_reference_form_parity_static_vs_faces() {
        // Parity invariant (spec 017 T8): the designer canvas entry `render_faces`
        // and the `render_form(Static)` entry used by every other surface must
        // agree on every control's on-screen geometry for a reference form
        // (Panel ⊃ {AreaChart, PictureBox, TextBox} + a top-level Label). This is
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
        let _ = ctx.run_ui(Default::default(), |root_ui| {
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
        });
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

    // ── Interaction simulation (Interactive mode) ─────────────────────────────
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

    /// A `FormState` over a per-control live-override map (id → key → value),
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
    /// Like [`drive`] but wraps the engine in `ScrollArea::both()` — the way the
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
            let _ = ctx.run_ui(input, |root_ui| {
                let ctx = root_ui.ctx().clone();
                let ctx = &ctx;
                egui::CentralPanel::default()
                    .frame(egui::Frame::NONE)
                    .show_inside(root_ui, |ui| {
                        egui::ScrollArea::both()
                            .auto_shrink([false, false])
                            .show(ui, |ui| {
                                // Content larger than the 1000×800 viewport → the scroll area
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
            });
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
            let _ = ctx.run_ui(input, |root_ui| {
                let ctx = root_ui.ctx().clone();
                let ctx = &ctx;
                egui::CentralPanel::default()
                    .frame(egui::Frame::NONE)
                    .show_inside(root_ui, |ui| {
                        let out =
                            egui::ScrollArea::both()
                                .auto_shrink([false, false])
                                .show(ui, |ui| {
                                    // Content larger than the viewport → the outer area
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
            });
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
        // the outer area — proving the harness allows outer scrolling, so the
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
            // Advance by small steps so a press→release across two frames still
            // counts as a click (egui's max click duration), while clearing the
            // Timer's 10 ms interval.
            input.time = Some(i as f64 * 0.05);
            input.events = evs;

            let updates = RefCell::new(Vec::<(String, String, String)>::new());
            let events = RefCell::new(Vec::<UiEvent>::new());
            let st = MapState(&overrides);
            let _ = ctx.run_ui(input, |root_ui| {
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
            });
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
                (1.0, vec![]), // 1s later → tick (interval 10ms)
            ],
        );
        assert!(
            names(&evs).contains(&"onTick"),
            "Timer: no onTick; got {:?}",
            names(&evs)
        );
    }

    /// A FormState with a fixed chrome-`enabled` answer — mirrors the real run
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
            input.time = Some(i as f64); // 1s/frame → clears any interval
            let events = RefCell::new(Vec::<UiEvent>::new());
            let _ = ctx.run_ui(input, |root_ui| {
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
            });
            all.extend(events.into_inner());
        }
        all
    }

    #[test]
    fn engine_timer_ticks_governed_by_enabled_property_not_chrome_flag() {
        // Real .cfrm shape: a non-visual Timer with chrome `enabled="false"` but
        // its own `Enabled` property = true. It MUST still tick — the property
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
// Shape-dump differ (spec 027 corner-bleed hunt) — egui 0.35 branch flavor.
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
            S::Text(_) => {} // font engines differ across versions — geometry only
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
        let full = ctx.run_ui(input, |root_ui| {
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
        let mut out = Vec::new();
        for cs in &full.shapes {
            dump_shape(&mut out, cs.clip_rect, &cs.shape);
        }
        std::fs::write(&path, out.join("\n")).unwrap();
        println!("dumped {} shapes", out.len());
    }

    /// Scene B — Classic glass + backdrop image + corner-reaching child:
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
        let full = ctx.run_ui(input, |root_ui| {
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
        let mut out = Vec::new();
        for cs in &full.shapes {
            dump_shape(&mut out, cs.clip_rect, &cs.shape);
        }
        std::fs::write(&path, out.join("\n")).unwrap();
        println!("scene B dumped {} shapes", out.len());
    }

    /// Scene C — captioned GroupBox + nested Panel + corner children, Classic
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
        let full = ctx.run_ui(input, |root_ui| {
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
        let mut out = Vec::new();
        for cs in &full.shapes {
            dump_shape(&mut out, cs.clip_rect, &cs.shape);
        }
        std::fs::write(&path, out.join("\n")).unwrap();
        println!("scene C dumped {} shapes", out.len());
    }

    /// Scene D — TRANSPARENT Panel + DataGrid child on image backdrop, Classic
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
        let full = ctx.run_ui(input, |root_ui| {
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
        let mut out = Vec::new();
        for cs in &full.shapes {
            dump_shape(&mut out, cs.clip_rect, &cs.shape);
        }
        std::fs::write(&path, out.join("\n")).unwrap();
        println!("scene D dumped {} shapes", out.len());
    }

    // ── DataGrid confined-fill geometry (pure, no egui context needed) ───────

    /// A fill's rects must tile its vertical span with NO gap. A sub-pixel gap is
    /// not harmless: the grid's own background (a solid BackgroundColor — yellow
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
        // appeared at the same y — it is pinned to the zone top, and a row boundary
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
                    "fill top={:.4}: coverage starts at {:.4} — {:.4}px SEAM at the top",
                    r.min.y,
                    spans[0].0,
                    spans[0].0 - r.min.y
                ));
            }
            let mut cursor = spans[0].1;
            for (a, b) in spans.iter().skip(1) {
                if *a - cursor > 1e-3 {
                    failures.push(format!(
                        "fill top={:.4}: GAP {:.4}..{:.4} ({:.4}px) — grid background bleeds through",
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

    /// No emitted rect may cross the bottom-corner arcs — that is the bleed.
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

    // ── DataGrid rounded-corner silhouette guards ────────────────────────────
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
    /// bleeding), CornerRadius 15, RowHeight 43, and column filters ON — the tall
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
        let full = ctx.run_ui(input, |root_ui| {
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
        full.shapes
    }

    /// Does this rect shape actually paint `p`? Accounts for the shape's OWN
    /// effective corner radius — egui clamps each corner to half the shorter side,
    /// so a short fill's stored radius is NOT what it renders (see the
    /// CORNER-BLEED-PLAYBOOK, §1.1).
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
                    // The form backdrop legitimately covers everything — identify it
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
            let dir = Vec2::new(cos, sin); // +y is down ⇒ this sweeps the SW arc
            // Just OUTSIDE the arc but still inside the bbox → must be unpainted.
            let po = c + dir * (DG_R + 1.2);
            if po.x >= bbox_x0 + 0.2 && po.y <= bbox_y1 - 0.2 {
                let hits = dg_painters_at(shapes, po);
                if !hits.is_empty() {
                    bleeds.push(format!(
                        "  θ={deg:.0}° ({:.1},{:.1}) ← {}",
                        po.x,
                        po.y,
                        hits.join(" | ")
                    ));
                }
            }
            // Just INSIDE the arc → must be painted.
            let pi = c + dir * (DG_R - 3.0);
            if dg_painters_at(shapes, pi).is_empty() {
                gaps.push(format!("  θ={deg:.0}° ({:.1},{:.1})", pi.x, pi.y));
            }
        }
        assert!(
            bleeds.is_empty(),
            "{label}: opaque fill(s) BLEED outside the grid's bottom-left arc \
             (radius {DG_R}) — they must be clipped to the arc:\n{}",
            bleeds.join("\n")
        );
        assert!(
            gaps.is_empty(),
            "{label}: the bottom-left arc INTERIOR is not filled — the corner fill \
             over-inset and left a square gap instead of tracking the arc:\n{}",
            gaps.join("\n")
        );
    }

    /// Bleed guard, case 1 (operator report 2026-07-25): MORE rows than fit, so the
    /// last visible row is a few-pixel sliver clipped at the grid bottom. Its
    /// requested corner radius gets clamped to `height/2` — a tiny arc that pokes
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
        // 7 rows × 43px under the tall filter header end ~3px above the grid
        // bottom — i.e. inside the 15px arc zone.
        let shapes = datagrid_corner_scene(7);
        dg_assert_corner_silhouette(&shapes, "rows ending inside the arc zone");
    }

    /// Corner-bleed guard (egui 0.35 regression): every stroked rect that is
    /// concentric with the panel face must keep its corner radius STRICTLY
    /// inside the face radius. u8 radii can't express `face - 0.5`, and
    /// rounding UP pushed the dark border arc outside the face — the visible
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
        let full = ctx.run_ui(input, |root_ui| {
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
                                "border arc (r={}, {:?}) may spill outside the face arc (r={fr}) — corner bleed regression",
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
