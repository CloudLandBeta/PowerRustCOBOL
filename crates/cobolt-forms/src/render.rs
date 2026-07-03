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
        // The notch mask repaints the backdrop over the corner arcs, erasing the
        // container's own border/rim there. Restore it so all four rounded corners
        // keep their outline (otherwise a Panel shows a border on its straight
        // edges but a gap at every corner).
        crate::paint::restore_container_outline(painter, &live, screen, rad, input.glass);
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

/// A top-level GroupBox marked as a repeating group (spec 015 control array).
fn is_repeating_instance_group(c: &Control) -> bool {
    matches!(c.control_type, ControlType::GroupBox)
        && c.parent.is_none()
        && c.get_prop("IsRepeatingGroup")
            .map(|v| v.as_bool())
            .unwrap_or(false)
}

/// How many runtime instances a repeating group renders: `ItemCount` when it has
/// been populated (e.g. by a data binding), otherwise `PreviewItemCount`.
fn repeating_instance_count(c: &Control) -> usize {
    let item = c.get_prop("ItemCount").map(|v| v.as_i64()).unwrap_or(0);
    let n = if item > 0 {
        item
    } else {
        c.get_prop("PreviewItemCount")
            .map(|v| v.as_i64())
            .unwrap_or(1)
    };
    n.clamp(1, 500) as usize
}

/// The id of a member control in the `inst`-th (1-based) instance of an array.
/// Instance 1 keeps the original id; later instances are suffixed so they render
/// and interact independently.
fn instance_member_id(base: &str, inst: usize) -> String {
    if inst <= 1 {
        base.to_owned()
    } else {
        format!("{base}#{inst}")
    }
}

/// Expand each top-level repeating GroupBox into its N runtime instances so the
/// shared render loop draws N cards. Instance 1 is the original template in place;
/// instances 2..N are clones of the group's subtree, shifted by the group's
/// layout (Vertical / Horizontal / Grid) with instance-unique ids. Returns `None`
/// when there is nothing to expand.
fn expand_repeating_groups(controls: &[Control]) -> Option<Vec<Control>> {
    let groups: Vec<usize> = (0..controls.len())
        .filter(|&i| {
            is_repeating_instance_group(&controls[i]) && repeating_instance_count(&controls[i]) > 1
        })
        .collect();
    if groups.is_empty() {
        return None;
    }
    let mut out: Vec<Control> = controls.to_vec();
    for gi in groups {
        let g = &controls[gi];
        let n = repeating_instance_count(g);
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
        let gw = g.rect.w as f32;
        let gh = g.rect.h as f32;
        let subtree: Vec<usize> = std::iter::once(gi)
            .chain(crate::containers::collect_descendants(controls, gi))
            .collect();
        for inst in 2..=n {
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
            for &si in &subtree {
                let mut clone = controls[si].clone();
                clone.rect.x += dx as i32;
                clone.rect.y += dy as i32;
                clone.id = instance_member_id(&clone.id, inst);
                clone.parent = clone.parent.as_deref().map(|p| instance_member_id(p, inst));
                out.push(clone);
            }
        }
    }
    Some(out)
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
    // When Neumorphic glass style is active, default the page to the recipe's
    // very light neutral background (#ECEFF4) so cards pop with the dual-shadow
    // relief. Respect an explicit non-default colour and any transparency.
    let bg = {
        let mut b = bg;
        if crate::paint::is_neumorphic_style(ui.ctx()) {
            let hex = input.backdrop.color_hex.trim().trim_start_matches('#');
            let looks_default_dark = hex.is_empty()
                || hex.eq_ignore_ascii_case("000000")
                || hex.eq_ignore_ascii_case("000")
                || (b.r() < 55 && b.g() < 58 && b.b() < 82);
            if looks_default_dark {
                let ba = b.a();
                let rr = (236.0 * (ba as f32) / 255.0) as u8;
                let gg = (239.0 * (ba as f32) / 255.0) as u8;
                let bb = (244.0 * (ba as f32) / 255.0) as u8;
                b = Color32::from_rgba_premultiplied(rr, gg, bb, ba);
            }
        }
        b
    };
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
    // Expand repeating groups (spec 015 / 024) into their N runtime instances so
    // the render loop below draws one card per item.
    let expanded = expand_repeating_groups(input.controls);
    let controls: &[Control] = expanded.as_deref().unwrap_or(input.controls);
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

fn draw_datagrid_pattern(painter: &egui::Painter, rect: Rect, pattern: &str, color: Color32) {
    let pattern = pattern.trim().to_ascii_lowercase();
    if pattern.is_empty() || pattern == "none" {
        return;
    }

    match pattern.as_str() {
        "stripes" | "stripe" => {
            // Horizontal bands, evenly distributed with balanced top/bottom margins.
            for cy in even_tile_centers(rect.min.y, rect.max.y, 12.0) {
                painter.rect_filled(
                    Rect::from_min_max(
                        pos2(rect.min.x, (cy - 3.0).max(rect.min.y)),
                        pos2(rect.max.x, (cy + 3.0).min(rect.max.y)),
                    ),
                    0.0,
                    color,
                );
            }
        }
        "dots" | "dot" => {
            for cy in even_tile_centers(rect.min.y, rect.max.y, 12.0) {
                for cx in even_tile_centers(rect.min.x, rect.max.x, 12.0) {
                    painter.circle_filled(pos2(cx, cy), 1.0, color);
                }
            }
        }
        "cross" | "plus" => {
            let stroke = Stroke::new(1.0, color);
            for cy in even_tile_centers(rect.min.y, rect.max.y, 14.0) {
                for cx in even_tile_centers(rect.min.x, rect.max.x, 14.0) {
                    painter.line_segment([pos2(cx - 3.0, cy), pos2(cx + 3.0, cy)], stroke);
                    painter.line_segment([pos2(cx, cy - 3.0), pos2(cx, cy + 3.0)], stroke);
                }
            }
        }
        "x" | "diagonal-cross" => {
            let stroke = Stroke::new(1.0, color);
            for cy in even_tile_centers(rect.min.y, rect.max.y, 14.0) {
                for cx in even_tile_centers(rect.min.x, rect.max.x, 14.0) {
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
                    painter.circle_stroke(pos2(cx, cy), 3.0, stroke);
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
fn control_pointer_events(
    ui: &egui::Ui,
    screen: Rect,
    ctrl_id: egui::Id,
    id: &str,
    ct: &ControlType,
    enabled: bool,
    out: &mut RenderOutput,
    bound_events: &[&str],
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
        if !fired && now - start >= 0.2 {
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
        let bound: Vec<&str> = ctrl.events.iter().map(|e| e.event.as_str()).collect();
        control_pointer_events(ui, screen, ctrl_id, id, &ct, enabled, out, &bound);
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
                ui.data_mut(|d| d.insert_temp(slider_dirty_id, true));
            }
            if resp.drag_released() {
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
                paint::corner_radius(ctrl),
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
            let painter = painter.with_clip_rect(painter.clip_rect().intersect(screen));
            let cell_fg = Color32::from_rgb(225, 230, 250);
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
                    painter.image(
                        tex.id(),
                        dest,
                        Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)),
                        Color32::from_rgba_unmultiplied(255, 255, 255, (alpha * 255.0) as u8),
                    );
                }
            }
            draw_datagrid_pattern(
                &painter,
                screen,
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
                    i.raw_scroll_delta = egui::Vec2::ZERO;
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
                        ui.output_mut(|o| o.copied_text = text);
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
                egui::Rounding {
                    nw: header_radius,
                    ne: header_radius,
                    sw: 0.0,
                    se: 0.0,
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
                            .frame(false),
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
            // `screen.max.y` and is cut square by the body clip. Rounding that
            // off-clip rect is invisible — so we intersect with the grid rect first,
            // then round the now-on-edge bottom corners.
            let grid_cr = paint::corner_radius(ctrl);
            let confine_bottom = move |r: Rect| -> (Rect, egui::Rounding) {
                let c = r.intersect(screen);
                let eps = 0.5;
                let at_bottom = (c.max.y - screen.max.y).abs() < eps;
                let rnd = egui::Rounding {
                    nw: 0.0,
                    ne: 0.0,
                    sw: if at_bottom && (c.min.x - screen.min.x).abs() < eps {
                        grid_cr
                    } else {
                        0.0
                    },
                    se: if at_bottom && (c.max.x - screen.max.x).abs() < eps {
                        grid_cr
                    } else {
                        0.0
                    },
                };
                (c, rnd)
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
                    let (ar, arnd) = confine_bottom(rrect);
                    body_painter.rect_filled(ar, arnd, alt_bg);
                }
                draw_datagrid_pattern(
                    &body_painter,
                    rrect,
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
                        let (acr, acrnd) = confine_bottom(col_rect);
                        body_painter.rect_filled(acr, acrnd, alt_bg);
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
                            // Clamped + rounded at the grid's bottom corners so the
                            // last row's fill follows the grid radius instead of
                            // squaring past it.
                            let (cr_rect, cr_rnd) = confine_bottom(col_rect);
                            body_painter.rect_filled(cr_rect, cr_rnd, bg);
                        }
                    }
                    if let Some(column) = column_meta {
                        draw_datagrid_pattern(
                            &body_painter,
                            cell_rect,
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
                                        egui::Rounding::same(corner + 1.0),
                                        Color32::from_black_alpha(55),
                                    );
                                    img_painter.rect_filled(
                                        dest.translate(vec2(0.0, 4.0)).expand(2.5),
                                        egui::Rounding::same(corner + 2.0),
                                        Color32::from_black_alpha(28),
                                    );
                                }
                                if corner > 0.0 {
                                    img_painter.add(egui::Shape::Rect(egui::epaint::RectShape {
                                        rect: dest,
                                        rounding: egui::Rounding::same(corner),
                                        fill: Color32::WHITE,
                                        stroke: Stroke::NONE,
                                        blur_width: 0.0,
                                        fill_texture_id: tex.id(),
                                        uv,
                                    }));
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
                        egui::Rounding {
                            nw: 0.0,
                            ne: 0.0,
                            sw: 0.0,
                            se: r,
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
                        [pos2(x, screen.min.y), pos2(x, screen.max.y)],
                        Stroke::new(1.0, grid_c),
                        grid_line_style,
                    );
                }
            }
            draw_datagrid_line(
                &painter,
                [
                    pos2(screen.min.x, screen.min.y + header_h),
                    pos2(screen.max.x, screen.min.y + header_h),
                ],
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
                    let half = o_stroke.width * 0.5;
                    painter.rect_stroke(
                        screen.shrink(half),
                        egui::Rounding::same((grid_cr - half).max(0.0)),
                        o_stroke,
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
                let track = Rect::from_min_max(
                    pos2(screen.max.x - 5.0, body_rect.min.y + 2.0),
                    pos2(screen.max.x - 2.0, body_rect.max.y - 2.0),
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
                paint::corner_radius(ctrl),
                false,
                alpha,
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
                paint::corner_radius(ctrl),
                Stroke::new(1.0, Color32::from_rgba_premultiplied(160, 170, 230, 110)),
            );
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
    fn repeating_group_expands_into_runtime_instances() {
        use crate::model::PropValue;
        let mut group = ctrl("CARD", ControlType::GroupBox, 0, 0, 200, 60);
        group.set_prop("IsRepeatingGroup", PropValue::Bool(true));
        group.set_prop("ItemCount", PropValue::Int(3));
        group.set_prop("LayoutDirection", PropValue::String("Vertical".into()));
        group.set_prop("ItemSpacing", PropValue::Int(10));
        let mut member = ctrl("NAME", ControlType::Label, 10, 10, 80, 20);
        member.parent = Some("CARD".into());
        let controls = vec![group, member];

        let expanded = expand_repeating_groups(&controls).expect("should expand");
        // 3 instances × 2 controls.
        assert_eq!(expanded.len(), 6);
        // Instance 1 keeps the original ids in place.
        assert!(expanded.iter().any(|c| c.id == "CARD" && c.rect.y == 0));
        // Instance 2 is shifted down by group height + spacing (60 + 10) and its
        // member's parent points at the cloned group.
        let g2 = expanded.iter().find(|c| c.id == "CARD#2").expect("CARD#2");
        assert_eq!(g2.rect.y, 70);
        let m2 = expanded.iter().find(|c| c.id == "NAME#2").expect("NAME#2");
        assert_eq!(m2.parent.as_deref(), Some("CARD#2"));
        assert_eq!(m2.rect.y, 10 + 70);
        // Instance 3 shifted twice.
        assert_eq!(
            expanded.iter().find(|c| c.id == "CARD#3").unwrap().rect.y,
            140
        );

        // A form without a repeating group is left untouched.
        let plain = vec![ctrl("BTN", ControlType::Button, 0, 0, 40, 20)];
        assert!(expand_repeating_groups(&plain).is_none());
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
            let _ = ctx.run(input, |ctx| {
                egui::CentralPanel::default()
                    .frame(egui::Frame::none())
                    .show(ctx, |ui| {
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
            let _ = ctx.run(input, |ctx| {
                egui::CentralPanel::default()
                    .frame(egui::Frame::none())
                    .show(ctx, |ui| {
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
