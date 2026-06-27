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

use egui::{Color32, Rect, Vec2};

use crate::containers::{self, ActiveTabs};
use crate::model::{BgImageMode, PropValue};
use crate::{Control, ControlType};

/// Supplies live control state to the engine, source-agnostic.
///
/// The default implementations render the **designed** form unchanged (what the
/// designer wants). Callers with live state override [`FormState::live`] to merge
/// their values onto the base control before it is drawn.
pub trait FormState {
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
    /// Optional background image, already resolved to a texture by the caller
    /// (the engine has no texture cache), plus its pixel size.
    pub image: Option<(egui::TextureId, Vec2)>,
    pub image_mode: BgImageMode,
}

impl Default for Backdrop {
    fn default() -> Self {
        Backdrop {
            color_hex: String::new(),
            transparency: 0,
            image: None,
            image_mode: BgImageMode::Fit,
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
    input: &RenderInput<'_>,
    idx: usize,
    origin: egui::Pos2,
) -> Option<(Rect, f32)> {
    let controls = input.controls;
    let parent_id = controls[idx].parent.as_ref()?;
    let parent = controls.iter().find(|c| &c.id == parent_id)?;
    if !matches!(
        parent.control_type,
        ControlType::GroupBox | ControlType::Panel
    ) {
        return None;
    }
    let plive = input.state.live(parent);
    let rad = crate::paint::corner_radius(&plive);
    if rad < 0.5 {
        return None;
    }
    let v = plive.rect; // visual (border) rect in form coords
    let border = Rect::from_min_max(
        origin + Vec2::new(v.x as f32, v.y as f32),
        origin + Vec2::new((v.x + v.w) as f32, (v.y + v.h) as f32),
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
fn mask_container_notches(
    painter: &egui::Painter,
    input: &RenderInput<'_>,
    out: &RenderOutput,
    image: Option<(egui::TextureId, Rect)>,
    img_alpha: u8,
    bg: Color32,
) {
    let controls = input.controls;
    for (idx, base) in controls.iter().enumerate() {
        if !matches!(
            base.control_type,
            ControlType::GroupBox | ControlType::Panel
        ) {
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
        crate::paint::draw_container_notch_mask(
            painter,
            screen,
            egui::Rounding::same(rad),
            bg,
            image,
            img_alpha,
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

/// Render a whole form into `ui` at its content origin. The caller sets up the
/// `CentralPanel` / `ScrollArea` and `ui.set_min_size(form_size)` first.
pub fn render_form(ui: &mut egui::Ui, input: &RenderInput<'_>) -> RenderOutput {
    let mut out = RenderOutput::default();
    let origin = ui.min_rect().min;
    let painter = ui.painter().clone();

    // ── Backdrop: solid colour, then optional image. ──────────────────────────
    let form_rect = Rect::from_min_size(origin, input.form_size);
    let bg = backdrop_color(&input.backdrop.color_hex, input.backdrop.transparency);
    painter.rect_filled(form_rect, 0.0, bg);
    // The notch mask is drawn *after* children. If the form background is
    // translucent, repainting `bg` would darken the corner wedges; skipping it
    // would leave rectangular child bleed visible. Use the effective one-pass
    // colour over the panel fill instead.
    let notch_bg = crate::paint::composite_premultiplied_over(bg, ui.visuals().panel_fill);
    let backdrop_img_alpha =
        ((100 - input.backdrop.transparency.min(100)) as f32 / 100.0 * 255.0) as u8;
    // Backdrop image, also remembered (texture + screen dest) so the corner-notch
    // mask can repaint it behind a rounded container's children (spec 017).
    let backdrop_img: Option<(egui::TextureId, Rect)> = input.backdrop.image.map(|(tex, tsize)| {
        let dest = image_dest(form_rect, tsize, input.backdrop.image_mode);
        let uv = Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0));
        painter.with_clip_rect(form_rect).image(
            tex,
            dest,
            uv,
            Color32::from_white_alpha(backdrop_img_alpha),
        );
        (tex, dest)
    });

    // ── Controls: designer order, clipped + faded by container ancestry. ──────
    let controls = input.controls;
    let order = containers::render_order(controls);
    let interactive = input.mode == RenderMode::Interactive;
    // ComboBox dropdowns are drawn in a second pass so they float above every
    // other control: (id, items, header rect, current value).
    let mut open_combos: Vec<(String, Vec<String>, Rect, String)> = Vec::new();
    for &idx in &order {
        let base = &controls[idx];
        if !input.state.visible(base) {
            continue;
        }
        if !containers::is_visible(controls, idx, input.active_tabs) {
            continue;
        }

        // Live control (designer source-of-truth face via draw_control).
        let live = input.state.live(base);
        let r = live.rect;
        // Animation transform: shift then scale about the control centre. Both
        // surfaces (preview now, designer later) supply entrance effects this way;
        // the default is identity (run / compiled / static designer).
        let tf = input.state.transform(base);
        let base_screen = Rect::from_min_size(
            origin + Vec2::new(r.x as f32 + tf.dx, r.y as f32 + tf.dy),
            Vec2::new(r.w as f32, r.h as f32),
        );
        let screen = crate::paint::scale_rect_about_center(base_screen, tf.scale);
        out.control_rects.insert(live.id.clone(), screen);

        // A PictureBox inside a rounded GroupBox/Panel is clipped to the parent's
        // BORDER path, so any overflow is cut by the container shape rather than the
        // image's own bounds (spec 017). The image is allowed to reach the parent's
        // border (not just its inset content area), so the clip widens to the border.
        let pic_border = if clips_to_container_border(&base.control_type) {
            picturebox_container_border(input, idx, origin)
        } else {
            None
        };

        // Clip to ancestor container content areas (rounded clipping is cosmetic;
        // egui clips to the axis-aligned rect — spec 012/016). Start from the whole
        // form so a top-level control is never clipped to its own bounds.
        let clip = match containers::clip_rect(controls, idx) {
            Some(cm) => form_rect.intersect(Rect::from_min_size(
                origin + Vec2::new(cm.x as f32, cm.y as f32),
                Vec2::new(cm.w as f32, cm.h as f32),
            )),
            None => form_rect,
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

    // ── Corner-notch masks: cut any child content that bled past a rounded
    // container's arc by repainting the backdrop in its corner notches (spec 017).
    mask_container_notches(
        &painter,
        input,
        &out,
        backdrop_img,
        backdrop_img_alpha,
        notch_bg,
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
            picturebox_container_border(input, idx, origin)
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

/// Universal pointer/gesture events for one control, derived purely from pointer
/// geometry (no extra interactable, so it never steals the control's own
/// interaction). Emits only the events the control declares in `supported_events`
/// — the data-driven loop ignores any without a bound handler. Mirrors the IDE's
/// `control_pointer_events`, but emits neutral [`UiEvent`]s.
fn control_pointer_events(
    ui: &egui::Ui,
    screen: Rect,
    ctrl_id: egui::Id,
    id: &str,
    ct: &ControlType,
    enabled: bool,
    out: &mut RenderOutput,
) {
    if !enabled {
        return;
    }
    let supported = ct.supported_events();
    let want = |e: &str| supported.contains(&e);

    let over = ui.rect_contains_pointer(screen);
    let (pressed, released, dbl, clicked) = ui.input(|i| {
        (
            i.pointer.primary_pressed(),
            i.pointer.primary_released(),
            i.pointer
                .button_double_clicked(egui::PointerButton::Primary),
            i.pointer.primary_clicked(),
        )
    });
    let press_mem = ctrl_id.with("press-began-over");
    if pressed {
        ui.ctx().memory_mut(|m| m.data.insert_temp(press_mem, over));
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

    // Universal pointer/gesture events for every visual control.
    let non_visual = matches!(
        ct,
        CT::Timer | CT::AgentObject | CT::SqlDatabase | CT::RestClient
    );
    if !non_visual {
        control_pointer_events(ui, screen, ctrl_id, id, &ct, enabled, out);
    }

    match ct {
        CT::Button => {
            // WYSIWYG face; only the press/hover feedback is added here.
            let resp = ui.interact(screen, ctrl_id, Sense::click());
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
            let corner = sv(ctrl, "CornerRadius").parse::<f32>().unwrap_or(4.0);
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
            if resp.clicked() && enabled {
                let v = if checked { "0" } else { "1" };
                out.prop_updates
                    .push((id.to_owned(), "Value".to_owned(), v.to_owned()));
                out.events.push(UiEvent::change(id, v));
                out.events.push(UiEvent::ev(id, "onCheckedChanged"));
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
            if resp.clicked() && enabled {
                out.prop_updates
                    .push((id.to_owned(), "Value".to_owned(), "1".to_owned()));
                out.events.push(UiEvent::change(id, "1"));
                out.events.push(UiEvent::ev(id, "onCheckedChanged"));
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
            let resp = ui.put(
                screen,
                egui::TextEdit::singleline(&mut buf)
                    .id(ctrl_id)
                    .frame(false)
                    .interactive(enabled)
                    .text_color(txt_col),
            );
            if resp.changed() {
                out.prop_updates
                    .push((id.to_owned(), "Text".to_owned(), buf.clone()));
                out.events.push(UiEvent::change(id, &buf));
            }
            if resp.gained_focus() {
                out.events.push(UiEvent::ev(id, "onGotFocus"));
                out.events.push(UiEvent::ev(id, "onEnter"));
            }
            if resp.lost_focus() {
                out.events.push(UiEvent::ev(id, "onLostFocus"));
                out.events.push(UiEvent::ev(id, "onLeave"));
            }
            if resp.has_focus() {
                let (key_down, key_up, typed) = ui.input(|i| {
                    let mut down = false;
                    let mut up = false;
                    let mut typed = false;
                    for e in &i.events {
                        match e {
                            egui::Event::Key { pressed: true, .. } => down = true,
                            egui::Event::Key { pressed: false, .. } => up = true,
                            egui::Event::Text(_) => typed = true,
                            _ => {}
                        }
                    }
                    (down, up, typed)
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
            let mut display_val = cur;

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
                        let delta_val = (delta_axis / track_len) * (max_v - min_v);
                        let raw = start_val + delta_val;
                        display_val = ((raw / step).round() * step).clamp(min_v, max_v);
                    }
                }
                if resp.drag_released() {
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
            }
        }
        CT::NumericUpDown => {
            paint::draw_glass_auto(
                &painter,
                screen,
                Color32::from_rgb(30, 40, 80),
                6.0,
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
                6.0,
                false,
                alpha,
            );
            let items: Vec<String> = sv(ctrl, "Items").lines().map(|l| l.to_owned()).collect();
            let cur = sv(ctrl, "Value");
            let mut picked: Option<String> = None;
            ui.allocate_ui_at_rect(screen, |ui| {
                ui.set_enabled(enabled);
                egui::ScrollArea::vertical()
                    .id_salt(ctrl_id)
                    .max_height(screen.height())
                    .show(ui, |ui| {
                        for item in &items {
                            if ui.selectable_label(&cur == item, item).clicked() {
                                picked = Some(item.clone());
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
        }
        CT::DateTimePicker => {
            let white = Color32::from_rgb(230, 235, 255);
            let dim = Color32::from_rgb(150, 160, 200);
            paint::draw_control(&painter, screen.min, ctrl, false, glass, alpha, 1.0, None);
            let val = sv(ctrl, "Value");
            let resp = ui.interact(screen, ctrl_id, Sense::click());

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
            let cell_fg = Color32::from_rgb(225, 230, 250);
            let cols: Vec<(String, String)> = sv(ctrl, "Columns")
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
            let ncols = cols.len().max(1);
            let rows: Vec<Vec<String>> = sv(ctrl, "Rows")
                .lines()
                .filter(|l| !l.is_empty())
                .map(|l| l.split('\t').map(|c| c.to_owned()).collect())
                .collect();
            let row_h = sv(ctrl, "RowHeight")
                .parse::<f32>()
                .unwrap_or(22.0)
                .clamp(14.0, 60.0);
            let col_w = screen.width() / ncols as f32;
            let header_bg = paint::parse_hex(&sv(ctrl, "HeaderBackgroundColor"))
                .unwrap_or(Color32::from_rgb(60, 66, 96));
            let header_fg = paint::parse_hex(&sv(ctrl, "HeaderForegroundColor"))
                .unwrap_or(Color32::from_rgb(235, 238, 250));
            let alt_bg = paint::parse_hex(&sv(ctrl, "AlternatingRowColor"))
                .unwrap_or(Color32::from_rgb(38, 44, 72));
            let grid_c = paint::parse_hex(&sv(ctrl, "GridLineColor"))
                .unwrap_or(Color32::from_rgba_premultiplied(150, 160, 200, 90));

            paint::draw_glass_auto(
                &painter,
                screen,
                Color32::from_rgb(26, 32, 58),
                4.0,
                false,
                alpha * 0.7,
            );
            let header_rect = Rect::from_min_size(screen.min, vec2(screen.width(), row_h));
            painter.rect_filled(header_rect, 0.0, header_bg);
            for (i, (name, _)) in cols.iter().enumerate() {
                let x = screen.min.x + i as f32 * col_w;
                painter.text(
                    pos2(x + 6.0, header_rect.center().y),
                    Align2::LEFT_CENTER,
                    name,
                    FontId::proportional(12.0),
                    header_fg,
                );
            }
            for (r, row) in rows.iter().enumerate() {
                let y = screen.min.y + row_h * (r as f32 + 1.0);
                if y >= screen.max.y {
                    break;
                }
                let rrect = Rect::from_min_size(pos2(screen.min.x, y), vec2(screen.width(), row_h));
                if r % 2 == 1 {
                    painter.rect_filled(rrect, 0.0, alt_bg);
                }
                for (i, (_, ty)) in cols.iter().enumerate() {
                    let raw = row.get(i).map(|s| s.as_str()).unwrap_or("");
                    let x0 = screen.min.x + i as f32 * col_w;
                    if matches!(ty.as_str(), "image" | "img" | "picture") {
                        let path = raw.trim();
                        let cell = Rect::from_min_size(pos2(x0, rrect.min.y), vec2(col_w, row_h))
                            .shrink(2.0);
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
                                painter.image(
                                    t.id(),
                                    irect,
                                    Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)),
                                    Color32::WHITE,
                                );
                            } else {
                                painter.rect_stroke(
                                    cell,
                                    2.0,
                                    Stroke::new(1.0, Color32::from_rgb(110, 120, 160)),
                                );
                            }
                        }
                        continue;
                    }
                    let (text, right) = paint::format_cell(raw, ty);
                    if right {
                        painter.text(
                            pos2(x0 + col_w - 6.0, rrect.center().y),
                            Align2::RIGHT_CENTER,
                            &text,
                            FontId::proportional(12.0),
                            cell_fg,
                        );
                    } else {
                        painter.text(
                            pos2(x0 + 6.0, rrect.center().y),
                            Align2::LEFT_CENTER,
                            &text,
                            FontId::proportional(12.0),
                            cell_fg,
                        );
                    }
                }
            }
            for i in 1..ncols {
                let x = screen.min.x + i as f32 * col_w;
                painter.line_segment(
                    [pos2(x, screen.min.y), pos2(x, screen.max.y)],
                    Stroke::new(1.0, grid_c),
                );
            }
            painter.line_segment(
                [
                    pos2(screen.min.x, screen.min.y + row_h),
                    pos2(screen.max.x, screen.min.y + row_h),
                ],
                Stroke::new(1.0, grid_c),
            );
        }
        CT::TabControl => {
            let tabs: Vec<String> = sv(ctrl, "Tabs").lines().map(|s| s.to_owned()).collect();
            let selected = sv(ctrl, "SelectedTab").parse::<usize>().unwrap_or(0);
            let tab_h = 26.0_f32;
            let content = Rect::from_min_max(pos2(screen.min.x, screen.min.y + tab_h), screen.max);
            paint::draw_glass_auto(
                &painter,
                content,
                Color32::from_rgb(34, 40, 70),
                6.0,
                false,
                alpha * 0.6,
            );
            let mut x = screen.min.x;
            for (i, tab) in tabs.iter().enumerate() {
                let w = 84.0_f32;
                let tr = Rect::from_min_size(pos2(x, screen.min.y), vec2(w, tab_h));
                let active = i == selected;
                painter.rect_filled(
                    tr,
                    4.0,
                    if active {
                        Color32::from_rgb(60, 80, 140)
                    } else {
                        Color32::from_rgb(40, 46, 78)
                    },
                );
                painter.text(
                    tr.center(),
                    Align2::CENTER_CENTER,
                    tab,
                    FontId::proportional(12.0),
                    if active {
                        Color32::from_rgb(235, 240, 255)
                    } else {
                        Color32::from_rgb(180, 188, 220)
                    },
                );
                if ui
                    .interact(tr, ctrl_id.with(("tab", i)), Sense::click())
                    .clicked()
                    && enabled
                {
                    out.prop_updates
                        .push((id.to_owned(), "SelectedTab".to_owned(), i.to_string()));
                    out.events.push(UiEvent::ev(id, "onChange"));
                }
                x += w + 2.0;
            }
            painter.rect_stroke(
                content,
                6.0,
                Stroke::new(1.0, Color32::from_rgba_premultiplied(160, 170, 230, 110)),
            );
        }
        CT::TreeView => {
            paint::draw_glass_auto(
                &painter,
                screen,
                Color32::from_rgb(28, 36, 64),
                6.0,
                false,
                alpha * 0.7,
            );
            let fg = Color32::from_rgb(220, 226, 250);
            let mut y = screen.min.y + 12.0;
            for line in sv(ctrl, "Items").lines() {
                if y > screen.max.y {
                    break;
                }
                let depth = (line.len() - line.trim_start().len()) / 2;
                let text = line.trim();
                if text.is_empty() {
                    continue;
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
                3.0,
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
            let menu_bg = ctrl.get_prop("BackgroundColor")
                .map(|v| paint::parse_color(v.as_str()))
                .unwrap_or(Color32::TRANSPARENT);
            if menu_bg.a() > 0 {
                paint::draw_glass_auto(
                    &painter, screen, menu_bg, 4.0, false, alpha * 0.85,
                );
            }
            let fg = ctrl.get_prop("ForegroundColor")
                .map(|v| paint::parse_color(v.as_str()))
                .unwrap_or(Color32::from_rgb(225, 230, 250));
            let highlight_bg = ctrl.get_prop("HighlightBgColor")
                .map(|v| paint::parse_color(v.as_str()))
                .unwrap_or(Color32::from_rgb(68, 136, 255));
            let highlight_fg = ctrl.get_prop("HighlightFgColor")
                .map(|v| paint::parse_color(v.as_str()))
                .unwrap_or(Color32::WHITE);
            let selected_bg = ctrl.get_prop("SelectedBgColor")
                .map(|v| paint::parse_color(v.as_str()))
                .unwrap_or(Color32::from_rgb(51, 102, 204));

            let menu_id = egui::Id::new(("menu_open", id));
            let open_idx: Option<usize> = ui.data(|d| d.get_temp(menu_id)).unwrap_or(None);

            if let Some(def) = paint::get_menu_cache(ui.ctx(), id) {
                let font = FontId::proportional(12.0);
                let mut x = screen.min.x + 8.0;
                let pad = 8.0;

                for (ti, entry) in def.menu.iter().enumerate() {
                    if entry.item_type == crate::menu::MenuItemType::Separator { continue; }
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
                        painter.galley(pos2(x, screen.center().y - galley.size().y * 0.5), galley, highlight_fg);
                    } else {
                        painter.galley(pos2(x, screen.center().y - galley.size().y * 0.5), galley, fg);
                    }

                    if resp.clicked() {
                        let new_idx = if is_open { None } else { Some(ti) };
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
                                egui::Frame::popup(&ui.ctx().style())
                                    .inner_margin(egui::Margin::same(4.0))
                                    .show(ui, |ui| {
                                        for item in &entry.items {
                                            if item.item_type == crate::menu::MenuItemType::Separator {
                                                ui.separator();
                                                continue;
                                            }
                                            let item_resp = ui.horizontal(|ui| {
                                                let dimmed = !item.enabled;
                                                let item_fg = if dimmed {
                                                    Color32::from_rgb(120, 120, 130)
                                                } else { fg };
                                                // Icon
                                                if let Some(icon_name) = &item.icon {
                                                    let icon_rect = ui.allocate_space(Vec2::splat(24.0)).1;
                                                    crate::icons::draw_menu_icon(&painter, icon_rect, icon_name, item_fg);
                                                } else {
                                                    ui.allocate_space(Vec2::splat(24.0));
                                                }
                                                // Label
                                                ui.label(egui::RichText::new(&item.label).color(item_fg));
                                                // Spacer
                                                ui.add_space(40.0);
                                                // Accelerator
                                                if let Some(accel_str) = &item.accelerator {
                                                    if let Some(accel) = crate::menu::parse_accelerator(accel_str) {
                                                        let formatted = crate::menu::format_accelerator(&accel);
                                                        ui.label(egui::RichText::new(formatted)
                                                            .color(Color32::from_rgb(140, 140, 160)).small());
                                                    }
                                                }
                                                // Sub-menu indicator
                                                if !item.items.is_empty() {
                                                    ui.label(egui::RichText::new("▸").color(item_fg));
                                                }
                                            });
                                            let row_resp = ui.interact(item_resp.response.rect, egui::Id::new(("mi", &item.id)), egui::Sense::click());
                                            if item.enabled && row_resp.hovered() {
                                                ui.painter().rect_filled(item_resp.response.rect, 2.0, highlight_bg);
                                            }
                                            if item.enabled && row_resp.clicked() {
                                                ui.data_mut(|d| d.insert_temp(menu_id, None::<usize>));
                                                if let Some(action) = &item.action {
                                                    if action == "close-application" {
                                                        out.events.push(UiEvent { ctrl_id: id.to_owned(), event: "onCloseApplication".to_owned(), value: None });
                                                    }
                                                }
                                                out.events.push(UiEvent { ctrl_id: id.to_owned(), event: "onMenuClick".to_owned(), value: Some(item.id.clone()) });
                                                let path = def.item_path(&item.id).unwrap_or_else(|| format!("/{}", item.label));
                                                out.events.push(UiEvent { ctrl_id: id.to_owned(), event: "onMenuItemClick".to_owned(), value: Some(path) });
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
                fn collect_accels<'a>(items: &'a [crate::menu::MenuItem], out: &mut Vec<(&'a crate::menu::MenuItem, crate::menu::Accelerator)>) {
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
                        out.events.push(UiEvent { ctrl_id: id.to_owned(), event: "onMenuClick".to_owned(), value: Some(item.id.clone()) });
                        let path = def.item_path(&item.id).unwrap_or_else(|| format!("/{}", item.label));
                        out.events.push(UiEvent { ctrl_id: id.to_owned(), event: "onMenuItemClick".to_owned(), value: Some(path) });
                    }
                }
            } else {
                painter.text(screen.center(), egui::Align2::CENTER_CENTER,
                    "☰ MenuBar (empty)", FontId::proportional(12.0), fg);
            }
        }
        CT::ToolBar | CT::StatusBar => {
            paint::draw_glass_auto(
                &painter, screen, Color32::from_rgb(40, 46, 76), 4.0, false, alpha * 0.85,
            );
            let fg = Color32::from_rgb(225, 230, 250);
            let mut x = screen.min.x + 8.0;
            for item in sv(ctrl, "Items").lines().filter(|l| !l.trim().is_empty()) {
                let galley =
                    painter.layout_no_wrap(item.trim().to_owned(), FontId::proportional(12.0), fg);
                let w = galley.size().x;
                painter.galley(
                    pos2(x, screen.center().y - galley.size().y / 2.0),
                    galley, fg,
                );
                x += w + 18.0;
            }
        }
        CT::PictureBox => {
            // Render through `draw_control` with a pre-loaded texture — the SAME
            // path the designer canvas uses — so the image is tinted/framed
            // identically and is never dimmed or washed-out relative to the canvas
            // (spec 017 parity). `draw_picturebox` used a different tint + frame.
            let tex =
                paint::picturebox_texture(ui.ctx(), sv(ctrl, "ImagePath").trim()).map(|t| t.id());
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
        }
        CT::Timer => {
            // Non-visual, but it TICKS: fire `onTick` every Interval ms while enabled.
            if enabled {
                let interval_s = sv(ctrl, "Interval")
                    .trim()
                    .parse::<f64>()
                    .unwrap_or(1000.0)
                    .max(10.0)
                    / 1000.0;
                let mem = ctrl_id.with("last_tick");
                let now = ui.input(|i| i.time);
                match ui.ctx().memory(|m| m.data.get_temp::<f64>(mem)) {
                    None => {
                        ui.ctx().memory_mut(|m| m.data.insert_temp(mem, now));
                    }
                    Some(last) if now - last >= interval_s => {
                        out.events.push(UiEvent::ev(id, "onTick"));
                        ui.ctx().memory_mut(|m| m.data.insert_temp(mem, now));
                    }
                    _ => {}
                }
                ui.ctx()
                    .request_repaint_after(std::time::Duration::from_millis(
                        (interval_s * 250.0) as u64 + 10,
                    ));
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
        }
        CT::AgentObject | CT::SqlDatabase | CT::RestClient => {
            // Non-visual — nothing to draw.
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
            let input = RenderInput {
                controls,
                state: &DesignedState,
                form_size: Vec2::new(400.0, 400.0),
                glass: true,
                mode: RenderMode::Static,
                active_tabs: &active,
                backdrop: Backdrop::default(),
            };
            picturebox_container_border(&input, 1, egui::pos2(0.0, 0.0))
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
        let _ = ctx.run(Default::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
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
        let _ = ctx.run(Default::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
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
                rects_faces = Some(render_faces(&painter, origin, &input).control_rects);
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
    use egui::{pos2, Event, Modifiers, PointerButton, Pos2};
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
            let _ = ctx.run(input, |ctx| {
                egui::CentralPanel::default()
                    .frame(egui::Frame::none())
                    .show(ctx, |ui| {
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
            let _ = ctx.run(input, |ctx| {
                egui::CentralPanel::default()
                    .frame(egui::Frame::none())
                    .show(ctx, |ui| {
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
        let c = [ctrlp(
            "Btn",
            ControlType::Button,
            0,
            0,
            80,
            30,
            &[("Caption", "OK")],
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
            ],
        );
        let n = names(&evs);
        for want in ["onGotFocus", "onEnter", "onChange", "onKeyPress"] {
            assert!(n.contains(&want), "TextBox: missing {want}; got {n:?}");
        }
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
}

/// Map a character from `menu::Accelerator` to an `egui::Key`.
#[cfg(feature = "render")]
fn char_to_key(c: char) -> egui::Key {
    match c {
        'A' => egui::Key::A, 'B' => egui::Key::B, 'C' => egui::Key::C,
        'D' => egui::Key::D, 'E' => egui::Key::E, 'F' => egui::Key::F,
        'G' => egui::Key::G, 'H' => egui::Key::H, 'I' => egui::Key::I,
        'J' => egui::Key::J, 'K' => egui::Key::K, 'L' => egui::Key::L,
        'M' => egui::Key::M, 'N' => egui::Key::N, 'O' => egui::Key::O,
        'P' => egui::Key::P, 'Q' => egui::Key::Q, 'R' => egui::Key::R,
        'S' => egui::Key::S, 'T' => egui::Key::T, 'U' => egui::Key::U,
        'V' => egui::Key::V, 'W' => egui::Key::W, 'X' => egui::Key::X,
        'Y' => egui::Key::Y, 'Z' => egui::Key::Z,
        '0' => egui::Key::Num0, '1' => egui::Key::Num1, '2' => egui::Key::Num2,
        '3' => egui::Key::Num3, '4' => egui::Key::Num4, '5' => egui::Key::Num5,
        '6' => egui::Key::Num6, '7' => egui::Key::Num7, '8' => egui::Key::Num8,
        '9' => egui::Key::Num9,
        '\u{F001}' => egui::Key::F1, '\u{F002}' => egui::Key::F2,
        '\u{F003}' => egui::Key::F3, '\u{F004}' => egui::Key::F4,
        '\u{F005}' => egui::Key::F5, '\u{F006}' => egui::Key::F6,
        '\u{F007}' => egui::Key::F7, '\u{F008}' => egui::Key::F8,
        '\u{F009}' => egui::Key::F9, '\u{F00A}' => egui::Key::F10,
        '\u{F00B}' => egui::Key::F11, '\u{F00C}' => egui::Key::F12,
        '\u{007F}' => egui::Key::Delete,
        '\u{0008}' => egui::Key::Backspace,
        '\t' => egui::Key::Tab,
        '\r' => egui::Key::Enter,
        '\u{001B}' => egui::Key::Escape,
        ' ' => egui::Key::Space,
        _ => egui::Key::A,
    }
}
