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
use crate::model::BgImageMode;
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
        Backdrop { color_hex: String::new(), transparency: 0, image: None, image_mode: BgImageMode::Fit }
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
        let (r, g, b) = if r == 0 && g == 0 && b == 0 { (20, 22, 45) } else { (r, g, b) };
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
    if let Some((tex, tsize)) = input.backdrop.image {
        let a = ((100 - input.backdrop.transparency.min(100)) as f32 / 100.0 * 255.0) as u8;
        let dest = image_dest(form_rect, tsize, input.backdrop.image_mode);
        let uv = Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0));
        painter.with_clip_rect(form_rect).image(tex, dest, uv, Color32::from_white_alpha(a));
    }

    // ── Controls: designer order, clipped + faded by container ancestry. ──────
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

        // Live control (designer source-of-truth face via draw_control).
        let live = input.state.live(base);
        let r = live.rect;
        let screen = Rect::from_min_size(
            origin + Vec2::new(r.x as f32, r.y as f32),
            Vec2::new(r.w as f32, r.h as f32),
        );
        out.control_rects.insert(live.id.clone(), screen);

        // Clip to ancestor container content areas (rounded clipping is cosmetic;
        // egui clips to the axis-aligned rect — spec 012/016).
        let dp = match containers::clip_rect(controls, idx) {
            Some(cm) => {
                let cs = Rect::from_min_size(
                    origin + Vec2::new(cm.x as f32, cm.y as f32),
                    Vec2::new(cm.w as f32, cm.h as f32),
                );
                painter.with_clip_rect(painter.clip_rect().intersect(cs))
            }
            None => painter.clone(),
        };

        let anc = containers::ancestor_opacity(controls, idx);
        let alpha = anc * if input.state.enabled(base) { 1.0 } else { 0.45 };

        // The one true face renderer (charts, images, glass, rounding included).
        crate::paint::draw_control(&dp, origin, &live, false, input.glass, alpha, 1.0, None);
    }

    // Interactive widgets layer on top in Interactive mode (added incrementally).
    let _ = input.mode;
    out
}

/// Scale `tsize` into `area` per `mode` (Fill/Fit centred, Center, Stretch, Tile).
fn image_dest(area: Rect, tsize: Vec2, mode: BgImageMode) -> Rect {
    match mode {
        BgImageMode::Fill | BgImageMode::Fit => {
            let sx = area.width() / tsize.x.max(1.0);
            let sy = area.height() / tsize.y.max(1.0);
            let s = if matches!(mode, BgImageMode::Fill) { sx.max(sy) } else { sx.min(sy) };
            let (dw, dh) = (tsize.x * s, tsize.y * s);
            Rect::from_min_size(
                area.min + Vec2::new((area.width() - dw) * 0.5, (area.height() - dh) * 0.5),
                Vec2::new(dw, dh),
            )
        }
        BgImageMode::Center => {
            Rect::from_min_size(
                area.min + Vec2::new((area.width() - tsize.x) * 0.5, (area.height() - tsize.y) * 0.5),
                tsize,
            )
        }
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
            "X" => { if let Ok(n) = v.trim().parse::<f32>() { c.rect.x = n.round() as i32; } }
            "Y" => { if let Ok(n) = v.trim().parse::<f32>() { c.rect.y = n.round() as i32; } }
            "Width" => { if let Ok(n) = v.trim().parse::<f32>() { c.rect.w = n.round() as i32; } }
            "Height" => { if let Ok(n) = v.trim().parse::<f32>() { c.rect.h = n.round() as i32; } }
            _ => c.set_prop(k.clone(), crate::PropValue::String(v.clone())),
        }
    }
    c
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
        assert_eq!(backdrop_color("#00000000", 0), Color32::from_rgba_premultiplied(20, 22, 45, 255));
        assert_eq!(backdrop_color("000000", 0), Color32::from_rgba_premultiplied(20, 22, 45, 255));
        assert_eq!(backdrop_color("", 0), Color32::from_rgba_premultiplied(20, 22, 45, 255));
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
    fn render_form_static_smoke() {
        // Headless: a form with a Panel ⊃ Button renders without panic and reports
        // both control rects through the engine (parity foundation).
        let controls = vec![
            { let mut c = ctrl("Pnl", ControlType::Panel, 0, 0, 200, 120); c.parent = None; c },
            { let mut c = ctrl("Btn", ControlType::Button, 20, 30, 80, 24); c.parent = Some("Pnl".into()); c },
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
                    backdrop: Backdrop { color_hex: "#00000000".into(), ..Default::default() },
                };
                captured = Some(render_form(ui, &input));
            });
        });
        let out = captured.expect("rendered");
        assert!(out.control_rects.contains_key("Pnl"));
        assert!(out.control_rects.contains_key("Btn"));
    }
}
