// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Form designer canvas panel.
//!
//! Features:
//! - WYSIWYG canvas with configurable grid snap (uses form.grid_size, toggleable via form.snap_to_grid)
//! - Multi-select with rubber-band lasso and Ctrl+click
//! - Z-order rendering (controls_by_z) and z-order controls
//! - Alignment toolbar: left / right / top / bottom / center-H / center-V / space-H / space-V
//! - Auto-arrange: labels left, textboxes right, perfectly aligned in rows
//! - Liquid-glass rendering mode (frosted glass aesthetic with reflections + shadows)
//! - Animation preview: play animations in the designer so you see them before runtime
//! - AgentObject, ModalWindow, RestClient control rendering
//! - Undo / Redo command stack

use std::collections::HashMap;
use std::f32::consts::TAU;
use egui::{Color32, CursorIcon, Pos2, Rect, Sense, Shape, Stroke, Ui, Vec2};
use cobolt_forms::{Control, ControlType, Form};
use cobolt_forms::model::{PropValue, AnimationDef, AnimKind, AnimTrigger, EasingKind, AnimRepeat, BgImageMode};

use super::properties::PropertiesPanel;
use super::toolbox::ToolboxPanel;

// ── Grid ──────────────────────────────────────────────────────────────────────
/// Snap `v` to the nearest multiple of `grid_px` (only when snap is enabled).
fn snap(v: i32, grid_px: i32, enabled: bool) -> i32 {
    if enabled && grid_px > 0 { (v / grid_px) * grid_px } else { v }
}

// ── Animation preview state ───────────────────────────────────────────────────

/// Live animation state used for designer preview only.
pub(crate) struct AnimState {
    /// Animation name being played.
    pub(crate) name:            String,
    /// Progress 0.0 → 1.0.
    pub(crate) t:               f32,
    /// Is the preview playing?
    pub(crate) playing:         bool,
    /// True = forward, false = reverse (for PingPong).
    pub(crate) forward:         bool,
    /// How many full loops completed.
    pub(crate) loops:           u32,
    /// Seconds of delay still to wait before `t` starts advancing.
    pub(crate) delay_remaining: f32,
}

impl AnimState {
    pub(crate) fn new(name: impl Into<String>) -> Self {
        Self { name: name.into(), t: 0.0, playing: false, forward: true, loops: 0, delay_remaining: 0.0 }
    }
    pub(crate) fn play(&mut self, delay_secs: f32) {
        self.t = 0.0;
        self.playing = true;
        self.forward = true;
        self.loops = 0;
        self.delay_remaining = delay_secs.max(0.0);
    }
    pub(crate) fn stop(&mut self) { self.playing = false; self.t = 1.0; }
}

/// ZoomOut "bounce" scale over progress `t`: a damped oscillation that starts at
/// 100%, dips toward 25%, then bounces 3–4 times with decreasing amplitude,
/// settling exactly at 100%.
fn zoomout_scale(t: f32) -> f32 {
    // N half-cycles (→ ~3–4 visible bounces); A sets the first dip (≈25%);
    // D damps each successive bounce. sin(Nπ·t) = 0 at t=0 and t=1, so the curve
    // begins and ends exactly at 100%.
    const N: f32 = 5.0;
    const A: f32 = 1.06;
    const D: f32 = 3.5;
    let osc = (N * std::f32::consts::PI * t).sin();
    (1.0 - A * (-D * t).exp() * osc).max(0.02)
}

/// Compute offset in canvas-space for an animation at progress t.
/// Returns (dx, dy, scale, alpha_mul) where alpha_mul is 0..1.
pub(crate) fn anim_transform(anim: &AnimationDef, form_w: f32, form_h: f32, t: f32) -> (f32, f32, f32, f32) {
    let te = anim.easing.apply(t);  // eased progress
    let inv = 1.0 - te;
    match &anim.kind {
        AnimKind::FlyFromLeft       => (-form_w * inv, 0.0, 1.0, 1.0),
        AnimKind::FlyFromRight      => ( form_w * inv, 0.0, 1.0, 1.0),
        AnimKind::FlyFromTop        => (0.0, -form_h * inv, 1.0, 1.0),
        AnimKind::FlyFromBottom     => (0.0,  form_h * inv, 1.0, 1.0),
        AnimKind::FlyFromTopLeft    => (-form_w * inv, -form_h * inv, 1.0, 1.0),
        AnimKind::FlyFromTopRight   => ( form_w * inv, -form_h * inv, 1.0, 1.0),
        AnimKind::FlyFromBottomLeft => (-form_w * inv,  form_h * inv, 1.0, 1.0),
        AnimKind::FlyFromBottomRight=> ( form_w * inv,  form_h * inv, 1.0, 1.0),
        AnimKind::FadeIn            => (0.0, 0.0, 1.0, te),
        AnimKind::FadeOut           => (0.0, 0.0, 1.0, 1.0 - te),
        // ZoomIn grows 0 → 100% (eased; Elastic overshoots past 100% and settles).
        AnimKind::ZoomIn            => (0.0, 0.0, te.max(0.001), te),
        // ZoomOut dips and returns: 100% → 25% → 100%. With Elastic easing this
        // becomes a damped multi-bounce (overshoots 3–4 times before settling).
        AnimKind::ZoomOut           => {
            let scale = if matches!(anim.easing, EasingKind::Elastic) {
                zoomout_scale(t)
            } else {
                // Smooth single dip-and-return (no overshoot), timed by the easing.
                (1.0 - 0.75 * (std::f32::consts::PI * te).sin()).max(0.02)
            };
            (0.0, 0.0, scale, 1.0)
        }
        AnimKind::Bounce            => {
            let dy = -50.0 * (std::f32::consts::PI * t * 3.0).sin().abs() * inv;
            (0.0, dy, 1.0, 1.0)
        }
        AnimKind::Shake             => {
            let dx = 6.0 * (t * std::f32::consts::TAU * 5.0).sin() * inv;
            (dx, 0.0, 1.0, 1.0)
        }
        AnimKind::Pulse             => {
            let s = 1.0 + 0.15 * (t * std::f32::consts::TAU * 2.0).sin() * inv;
            (0.0, 0.0, s, 1.0)
        }
        AnimKind::Slide { dx, dy }  => {
            ((*dx as f32) * inv, (*dy as f32) * inv, 1.0, 1.0)
        }
        AnimKind::Spin => {
            // Simulate spin as a scale pulse that goes through 0 twice (simulates
            // a 360° rotation in 2D by shrinking to nothing and back twice).
            let angle = te * std::f32::consts::TAU;
            let s = angle.cos().abs().max(0.05); // 1 → 0 → 1 twice = perceived spin
            (0.0, 0.0, s, te)
        }
        AnimKind::Flip => {
            // Horizontal flip: scale goes 1 → 0 → 1 (one half-rotation).
            let s = (te * std::f32::consts::PI).cos().abs().max(0.05);
            (0.0, 0.0, s, 1.0)
        }
        AnimKind::None | AnimKind::Custom(_) => {
            (0.0, 0.0, 1.0, 1.0)
        }
    }
}

// ── Undo / Redo command ───────────────────────────────────────────────────────

#[derive(Clone)]
enum Cmd {
    AddControl    { index: usize, ctrl: Control },
    DeleteControl { index: usize, ctrl: Control },
    MoveControl   { id: String, old_x: i32, old_y: i32, new_x: i32, new_y: i32 },
    MoveMany      { moves: Vec<(String, i32, i32, i32, i32)> },  // id, ox, oy, nx, ny
    ResizeControl { id: String, old_rect: cobolt_forms::model::Rect, new_rect: cobolt_forms::model::Rect },
    SetProperty   { id: String, key: String, old: Option<PropValue>, new: PropValue },
    ReorderControl{ from: usize, to: usize },
    SetZOrder     { id: String, old_z: i32, new_z: i32 },
}

// ── Resize handle ─────────────────────────────────────────────────────────────

#[derive(Clone, Copy, PartialEq, Debug)]
enum Handle { TopLeft, Top, TopRight, Left, Right, BotLeft, Bot, BotRight }

const ALL_HANDLES: [Handle; 8] = [
    Handle::TopLeft, Handle::Top, Handle::TopRight,
    Handle::Left,                 Handle::Right,
    Handle::BotLeft, Handle::Bot, Handle::BotRight,
];

fn handle_pos(r: &cobolt_forms::model::Rect, h: Handle) -> Pos2 {
    let (x, y, w, hh) = (r.x as f32, r.y as f32, r.w as f32, r.h as f32);
    match h {
        Handle::TopLeft  => Pos2::new(x,           y),
        Handle::Top      => Pos2::new(x + w / 2.0, y),
        Handle::TopRight => Pos2::new(x + w,        y),
        Handle::Left     => Pos2::new(x,            y + hh / 2.0),
        Handle::Right    => Pos2::new(x + w,        y + hh / 2.0),
        Handle::BotLeft  => Pos2::new(x,            y + hh),
        Handle::Bot      => Pos2::new(x + w / 2.0,  y + hh),
        Handle::BotRight => Pos2::new(x + w,         y + hh),
    }
}

fn handle_cursor(h: Handle) -> CursorIcon {
    match h {
        Handle::TopLeft  | Handle::BotRight => CursorIcon::ResizeNwSe,
        Handle::TopRight | Handle::BotLeft  => CursorIcon::ResizeNeSw,
        Handle::Top      | Handle::Bot      => CursorIcon::ResizeVertical,
        Handle::Left     | Handle::Right    => CursorIcon::ResizeHorizontal,
    }
}

fn apply_resize(r: cobolt_forms::model::Rect, h: Handle, dx: i32, dy: i32, grid_px: i32, snapping: bool) -> cobolt_forms::model::Rect {
    let s = |v| snap(v, grid_px, snapping);
    let mut nr = r;
    match h {
        Handle::TopLeft  => { nr.x = s(r.x+dx); nr.y = s(r.y+dy); nr.w=(r.w-dx).max(8); nr.h=(r.h-dy).max(8); }
        Handle::Top      => { nr.y = s(r.y+dy); nr.h=(r.h-dy).max(8); }
        Handle::TopRight => { nr.y = s(r.y+dy); nr.w=s(r.w+dx).max(8); nr.h=(r.h-dy).max(8); }
        Handle::Left     => { nr.x = s(r.x+dx); nr.w=(r.w-dx).max(8); }
        Handle::Right    => { nr.w = s(r.w+dx).max(8); }
        Handle::BotLeft  => { nr.x = s(r.x+dx); nr.w=(r.w-dx).max(8); nr.h=s(r.h+dy).max(8); }
        Handle::Bot      => { nr.h = s(r.h+dy).max(8); }
        Handle::BotRight => { nr.w = s(r.w+dx).max(8); nr.h=s(r.h+dy).max(8); }
    }
    nr
}

// ── Drag state ────────────────────────────────────────────────────────────────

#[derive(Clone, Debug)]
enum DragState {
    None,
    /// Moving one or more selected controls.
    MovingControls {
        /// primary dragged id + its origin
        primary_id: String,
        /// all selected ids with their original positions
        origins: Vec<(String, i32, i32)>,
        start_x: i32,
        start_y: i32,
    },
    ResizingControl {
        id: String, handle: Handle,
        orig_rect: cobolt_forms::model::Rect,
        start_x: i32, start_y: i32,
    },
    PlacingNew {
        ctrl_type: ControlType,
        start_x: i32, start_y: i32,
        cur_x: i32, cur_y: i32,
    },
    /// Rubber-band lasso selection.
    RubberBand { start_x: i32, start_y: i32, cur_x: i32, cur_y: i32 },
    /// Resizing the form canvas itself by dragging its right/bottom/corner edge.
    ResizingForm { edge: FormEdge, orig_w: i32, orig_h: i32, start_x: i32, start_y: i32 },
}

/// Which edge of the form canvas is being dragged to resize it.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum FormEdge { Right, Bottom, Corner }

/// Half-width (px) of the grab band along the form's right/bottom border.
const FORM_EDGE_GRAB: f32 = 7.0;

/// Smallest form dimension allowed when resizing by drag (matches `set_form_prop`).
const FORM_MIN_SIZE: i32 = 64;

/// Detect whether the canvas-space pointer `(px, py)` is over the form's resize
/// border, given the form size `(w, h)`. Returns the edge, or `None`.
fn detect_form_edge(px: i32, py: i32, w: f32, h: f32) -> Option<FormEdge> {
    let (px, py) = (px as f32, py as f32);
    let near_right  = (px - w).abs() <= FORM_EDGE_GRAB && py >= -FORM_EDGE_GRAB && py <= h + FORM_EDGE_GRAB;
    let near_bottom = (py - h).abs() <= FORM_EDGE_GRAB && px >= -FORM_EDGE_GRAB && px <= w + FORM_EDGE_GRAB;
    match (near_right, near_bottom) {
        (true, true)  => Some(FormEdge::Corner),
        (true, false) => Some(FormEdge::Right),
        (false, true) => Some(FormEdge::Bottom),
        _ => None,
    }
}

fn form_edge_cursor(e: FormEdge) -> CursorIcon {
    match e {
        FormEdge::Right  => CursorIcon::ResizeHorizontal,
        FormEdge::Bottom => CursorIcon::ResizeVertical,
        FormEdge::Corner => CursorIcon::ResizeNwSe,
    }
}

// ── Format Painter ────────────────────────────────────────────────────────────

/// Visual style properties that can be copied between controls.
const STYLE_PROP_KEYS: &[&str] = &[
    "BackgroundColor", "ForegroundColor", "BorderColor",
    "FontSize", "Bold", "Italic", "Underline", "Strikethrough",
    "FontName", "Opacity",
    "CornerRadius", "BorderWidth", "BorderStyle",
    "HeaderBackgroundColor", "HeaderForegroundColor",
    "AlternatingRowColor", "GridLineColor",
];

/// State machine for the format-painter (copy style) tool.
///
/// New UX flow:
///   1. User selects the source control on the canvas normally.
///   2. User clicks "🖌 Copy Style" — style is captured immediately from the selection.
///   3. Painter enters `WaitingForTarget`; cursor becomes a crosshair.
///   4. User clicks any target control → style is pasted; returns to `Idle`.
///   Clicking the button again while in `WaitingForTarget` cancels.
#[allow(dead_code)]
pub(crate) enum FormatPainter {
    /// Inactive.
    Idle,
    /// Reserved / legacy — not entered in the current flow.
    WaitingForSource,
    /// Style has been captured from the source; waiting for the user to click a target.
    WaitingForTarget {
        props:      std::collections::HashMap<String, cobolt_forms::model::PropValue>,
        animations: Vec<AnimationDef>,
        src_rect:   cobolt_forms::model::Rect,
    },
}

// ── Event Editor Modal ────────────────────────────────────────────────────────

/// State for the modal COBOL code editor that pops up when
/// the user clicks an event row in the Properties panel.
pub struct EventEditorModal {
    /// Control ID whose event is being edited (empty string = form-level event).
    pub ctrl_id:      String,
    /// Human-readable display name for the title bar (e.g. "BTN-OK · Click").
    pub ctrl_display: String,
    /// Event name, e.g. "Click".
    pub event_name:   String,
    /// The nested PROGRAM-ID that will be emitted for this handler.
    pub program_id:   String,
    /// Editable WORKING-STORAGE content (items specific to this handler).
    pub ws_buf:       String,
    /// Editable PROCEDURE DIVISION body (user COBOL statements).
    pub proc_buf:     String,
    /// Original values — used to detect whether anything actually changed.
    orig_ws:   String,
    orig_proc: String,
    /// True once Save was clicked.
    pub saved: bool,
}

impl EventEditorModal {
    pub fn new(
        ctrl_id:      impl Into<String>,
        ctrl_display: impl Into<String>,
        event_name:   impl Into<String>,
        program_id:   impl Into<String>,
        ws_buf:       impl Into<String>,
        proc_buf:     impl Into<String>,
    ) -> Self {
        let ws   = ws_buf.into();
        let proc = proc_buf.into();
        Self {
            ctrl_id:      ctrl_id.into(),
            ctrl_display: ctrl_display.into(),
            event_name:   event_name.into(),
            program_id:   program_id.into(),
            orig_ws:   ws.clone(),
            orig_proc: proc.clone(),
            ws_buf:   ws,
            proc_buf: proc,
            saved: false,
        }
    }

    pub fn is_dirty(&self) -> bool {
        self.ws_buf != self.orig_ws || self.proc_buf != self.orig_proc
    }
}

// ── DesignerPanel ─────────────────────────────────────────────────────────────

pub struct DesignerPanel {
    pub form: Form,

    /// All currently selected control IDs (first = primary selection).
    pub selected_ids: Vec<String>,

    drag: DragState,

    undo_stack: Vec<Cmd>,
    redo_stack: Vec<Cmd>,

    pub dirty: bool,
    pub close_requested: bool,
    /// Set when the user tries to close a dirty designer — shows the Save/Discard/Cancel dialog.
    pub close_confirm: bool,

    /// Format-painter (copy-style) state.
    pub(crate) format_painter: FormatPainter,

    next_id: u32,

    pub toolbox: ToolboxPanel,
    pub properties: PropertiesPanel,

    // ── UI options ────────────────────────────────────────────────────────────
    pub show_grid:    bool,
    pub glass_mode:   bool,

    // ── Animation preview ─────────────────────────────────────────────────────
    /// ctrl_id → AnimState (for designer-time preview of animations)
    anim_states: HashMap<String, AnimState>,
    /// Elapsed time from last frame for animation stepping.
    last_frame_time: Option<std::time::Instant>,

    // ── Image preview cache ───────────────────────────────────────────────────
    /// Maps absolute image path → loaded egui texture handle.
    /// `None` means the path was tried but failed to load.
    pub(crate) image_cache: HashMap<String, Option<egui::TextureHandle>>,

    // ── Resize handle press capture ───────────────────────────────────────────
    /// Stores which resize handle the pointer was on when the mouse button was
    /// first pressed.  Consumed on `drag_started()` so the drag-start check
    /// doesn't have to re-test the (now moved) pointer against the small handle.
    press_handle: Option<Handle>,
    /// Stores which form edge (if any) the pointer was on when the mouse button
    /// was first pressed, so the form-resize drag can begin on `drag_started()`.
    press_form_edge: Option<FormEdge>,

    // ── Event editor modal ────────────────────────────────────────────────────
    /// When `Some`, a modal COBOL code editor is displayed over the canvas.
    pub event_modal: Option<EventEditorModal>,

    // ── Form preview ──────────────────────────────────────────────────────────
    /// Whether the live preview viewport is open.
    pub show_preview: bool,
    /// Runtime state for preview: maps ctrl_id → current value (for interactive controls).
    pub preview_state: HashMap<String, String>,
    /// Animation states for the live preview (separate from designer preview).
    pub preview_anim_states: HashMap<String, AnimState>,
    /// Last frame time for the live preview animation ticker.
    pub preview_last_frame: Option<std::time::Instant>,
    /// Tracks which ComboBox (by control ID) is currently open in the preview.
    pub(crate) preview_combo_open: HashMap<String, bool>,
}

impl DesignerPanel {
    pub fn new(form: Form) -> Self {
        Self {
            next_id:          form.controls.len() as u32 + 1,
            form,
            selected_ids:     Vec::new(),
            drag:             DragState::None,
            undo_stack:       Vec::new(),
            redo_stack:       Vec::new(),
            dirty:            false,
            close_requested:  false,
            close_confirm:    false,
            toolbox:          ToolboxPanel::new(),
            properties:       PropertiesPanel::new(),
            show_grid:        true,
            glass_mode:       true,
            anim_states:      HashMap::new(),
            last_frame_time:  None,
            format_painter:   FormatPainter::Idle,
            image_cache:      HashMap::new(),
            press_handle:          None,
            press_form_edge:       None,
            event_modal:           None,
            show_preview:          false,
            preview_state:         HashMap::new(),
            preview_anim_states:   HashMap::new(),
            preview_last_frame:    None,
            preview_combo_open:    HashMap::new(),
        }
    }

    pub fn new_blank(name: impl Into<String>) -> Self {
        let form = Form::new(name.into(), "New Form", 640, 480);
        Self::new(form)
    }

    /// Primary selected ID (first in the selection list).
    pub fn primary_selected(&self) -> Option<&str> {
        self.selected_ids.first().map(|s| s.as_str())
    }

    /// Load an image from disk and register it as an egui texture.
    /// Returns `Some(handle)` on success, `None` on any error.
    /// Results are cached by path so each file is read at most once per session.
    pub(crate) fn load_image(&mut self, path: &str, ctx: &egui::Context) -> Option<&egui::TextureHandle> {
        if !self.image_cache.contains_key(path) {
            let result: Option<egui::TextureHandle> = (|| {
                let bytes = std::fs::read(path).ok()?;
                let img   = image::load_from_memory(&bytes).ok()?.into_rgba8();
                let (w, h) = (img.width() as usize, img.height() as usize);
                let pixels: Vec<egui::Color32> = img.pixels()
                    .map(|p| egui::Color32::from_rgba_unmultiplied(p[0], p[1], p[2], p[3]))
                    .collect();
                let ci = egui::ColorImage { size: [w, h], pixels };
                Some(ctx.load_texture(path, ci, egui::TextureOptions::LINEAR))
            })();
            self.image_cache.insert(path.to_owned(), result);
        }
        self.image_cache.get(path).and_then(|o| o.as_ref())
    }

    /// Invalidate a cached image texture so it will be reloaded next frame.
    pub fn invalidate_image(&mut self, path: &str) {
        self.image_cache.remove(path);
    }

    fn is_selected(&self, id: &str) -> bool {
        self.selected_ids.iter().any(|s| s == id)
    }

    fn set_selected_one(&mut self, id: Option<String>) {
        self.selected_ids.clear();
        if let Some(id) = id { self.selected_ids.push(id); }
    }

    fn toggle_selected(&mut self, id: &str) {
        if let Some(pos) = self.selected_ids.iter().position(|s| s == id) {
            self.selected_ids.remove(pos);
        } else {
            self.selected_ids.push(id.to_owned());
        }
    }

    // ── Undo / Redo ───────────────────────────────────────────────────────────

    fn apply(&mut self, cmd: Cmd) {
        self.execute(&cmd);
        self.undo_stack.push(cmd);
        self.redo_stack.clear();
        self.dirty = true;
    }

    pub fn undo(&mut self) {
        if let Some(cmd) = self.undo_stack.pop() {
            self.reverse(&cmd);
            self.redo_stack.push(cmd);
            self.dirty = true;
        }
    }

    pub fn redo(&mut self) {
        if let Some(cmd) = self.redo_stack.pop() {
            self.execute(&cmd);
            self.undo_stack.push(cmd);
            self.dirty = true;
        }
    }

    fn execute(&mut self, cmd: &Cmd) {
        match cmd {
            Cmd::AddControl { index, ctrl } => {
                let idx = (*index).min(self.form.controls.len());
                self.form.controls.insert(idx, ctrl.clone());
            }
            Cmd::DeleteControl { index, .. } => {
                if *index < self.form.controls.len() { self.form.controls.remove(*index); }
            }
            Cmd::MoveControl { id, new_x, new_y, .. } => {
                if let Some(c) = self.form.find_control_mut(id) {
                    c.rect.x = *new_x; c.rect.y = *new_y;
                }
            }
            Cmd::MoveMany { moves } => {
                for (id, _, _, nx, ny) in moves {
                    if let Some(c) = self.form.find_control_mut(id) {
                        c.rect.x = *nx; c.rect.y = *ny;
                    }
                }
            }
            Cmd::ResizeControl { id, new_rect, .. } => {
                if let Some(c) = self.form.find_control_mut(id) { c.rect = *new_rect; }
            }
            Cmd::SetProperty { id, key, new, .. } => {
                if let Some(c) = self.form.find_control_mut(id) {
                    apply_structural_prop(c, key, new);
                }
            }
            Cmd::ReorderControl { from, to } => {
                let len = self.form.controls.len();
                if *from < len && *to < len {
                    let ctrl = self.form.controls.remove(*from);
                    self.form.controls.insert(*to, ctrl);
                }
            }
            Cmd::SetZOrder { id, new_z, .. } => {
                if let Some(c) = self.form.find_control_mut(id) { c.z_order = *new_z; }
            }
        }
    }

    fn reverse(&mut self, cmd: &Cmd) {
        match cmd {
            Cmd::AddControl { index, .. } => {
                if *index < self.form.controls.len() { self.form.controls.remove(*index); }
            }
            Cmd::DeleteControl { index, ctrl } => {
                let idx = (*index).min(self.form.controls.len());
                self.form.controls.insert(idx, ctrl.clone());
            }
            Cmd::MoveControl { id, old_x, old_y, .. } => {
                if let Some(c) = self.form.find_control_mut(id) {
                    c.rect.x = *old_x; c.rect.y = *old_y;
                }
            }
            Cmd::MoveMany { moves } => {
                for (id, ox, oy, _, _) in moves {
                    if let Some(c) = self.form.find_control_mut(id) {
                        c.rect.x = *ox; c.rect.y = *oy;
                    }
                }
            }
            Cmd::ResizeControl { id, old_rect, .. } => {
                if let Some(c) = self.form.find_control_mut(id) { c.rect = *old_rect; }
            }
            Cmd::SetProperty { id, key, old, .. } => {
                if let Some(c) = self.form.find_control_mut(id) {
                    if let Some(v) = old { apply_structural_prop(c, key, v); }
                    else { c.properties.swap_remove(key); }
                }
            }
            Cmd::ReorderControl { from, to } => {
                let len = self.form.controls.len();
                if *from < len && *to < len {
                    let ctrl = self.form.controls.remove(*to);
                    self.form.controls.insert(*from, ctrl);
                }
            }
            Cmd::SetZOrder { id, old_z, .. } => {
                if let Some(c) = self.form.find_control_mut(id) { c.z_order = *old_z; }
            }
        }
    }

    // ── Control manipulation ──────────────────────────────────────────────────

    fn next_unique_id(&mut self, ct: &ControlType) -> String {
        let prefix = match ct {
            ControlType::Button       => "BTN",
            ControlType::Label        => "LBL",
            ControlType::TextBox      => "TXT",
            ControlType::CheckBox     => "CHK",
            ControlType::RadioButton  => "RDO",
            ControlType::ComboBox     => "CMB",
            ControlType::ListBox      => "LST",
            ControlType::PictureBox   => "PIC",
            ControlType::Animator     => "ANM",
            ControlType::GroupBox     => "GRP",
            ControlType::Panel        => "PNL",
            ControlType::TabControl   => "TAB",
            ControlType::ProgressBar  => "PGR",
            ControlType::DataGrid     => "GRD",
            ControlType::MenuBar      => "MNU",
            ControlType::ToolBar      => "TBR",
            ControlType::StatusBar    => "SBR",
            ControlType::Line         => "LIN",
            ControlType::DateTimePicker => "DTP",
            ControlType::NumericUpDown  => "NUD",
            ControlType::TreeView       => "TRV",
            ControlType::Splitter       => "SPL",
            ControlType::Timer          => "TMR",
            ControlType::Shape          => "SHP",
            ControlType::AgentObject    => "AGT",
            ControlType::ModalWindow    => "MDL",
            ControlType::RestClient     => "RST",
            ControlType::Slider         => "SLD",
            ControlType::Custom { .. }  => "CTL",
            ControlType::SqlDatabase    => "SQL",
            ControlType::BarChart       => "BAR",
            ControlType::LineChart      => "LIN",
            ControlType::PieChart       => "PIE",
            ControlType::AreaChart      => "ARE",
            ControlType::ScatterChart   => "SCT",
            ControlType::DonutChart     => "DNT",
        };
        let id = format!("{}-{}", prefix, self.next_id);
        self.next_id += 1;
        id
    }

    pub fn add_control(&mut self, ct: ControlType, x: i32, y: i32) {
        let id = self.next_unique_id(&ct);
        let gp = self.form.grid_size as i32;
        let sn = self.form.snap_to_grid;
        let mut ctrl = Control::new(id.clone(), ct.clone(), snap(x, gp, sn), snap(y, gp, sn));
        // Assign z_order = highest existing + 1
        let max_z = self.form.controls.iter().map(|c| c.z_order).max().unwrap_or(-1);
        ctrl.z_order = max_z + 1;
        // Controls whose widget intrinsically shows a text label get a Caption.
        let has_caption = matches!(
            ct,
            ControlType::Label
            | ControlType::Button
            | ControlType::CheckBox
            | ControlType::RadioButton
            | ControlType::GroupBox
        );
        if has_caption {
            ctrl.properties.insert("Caption".into(), PropValue::String(id.clone()));
        }
        let index = self.form.controls.len();
        self.apply(Cmd::AddControl { index, ctrl });
        self.set_selected_one(Some(id));
    }

    pub fn delete_selected(&mut self) {
        // Delete all selected controls, highest index first to not shift indices
        let mut indices: Vec<usize> = self.selected_ids.iter()
            .filter_map(|sid| self.form.controls.iter().position(|c| &c.id == sid))
            .collect();
        indices.sort_unstable();
        indices.dedup();
        // Apply deletes from highest index down
        for idx in indices.into_iter().rev() {
            let ctrl = self.form.controls[idx].clone();
            self.apply(Cmd::DeleteControl { index: idx, ctrl });
        }
        self.selected_ids.clear();
    }

    pub fn bring_to_front(&mut self) {
        for sid in &self.selected_ids.clone() {
            let max_z = self.form.controls.iter()
                .filter(|c| &c.id != sid)
                .map(|c| c.z_order)
                .max()
                .unwrap_or(0);
            if let Some(c) = self.form.find_control(sid) {
                let old_z = c.z_order;
                let new_z = max_z + 1;
                if old_z != new_z {
                    self.apply(Cmd::SetZOrder { id: sid.clone(), old_z, new_z });
                }
            }
        }
    }

    pub fn send_to_back(&mut self) {
        for sid in &self.selected_ids.clone() {
            let min_z = self.form.controls.iter()
                .filter(|c| &c.id != sid)
                .map(|c| c.z_order)
                .min()
                .unwrap_or(0);
            if let Some(c) = self.form.find_control(sid) {
                let old_z = c.z_order;
                let new_z = min_z - 1;
                if old_z != new_z {
                    self.apply(Cmd::SetZOrder { id: sid.clone(), old_z, new_z });
                }
            }
        }
    }

    pub fn bring_forward(&mut self) {
        for sid in &self.selected_ids.clone() {
            if let Some(c) = self.form.find_control(sid) {
                let old_z = c.z_order;
                let new_z = old_z + 1;
                self.apply(Cmd::SetZOrder { id: sid.clone(), old_z, new_z });
            }
        }
    }

    pub fn send_backward(&mut self) {
        for sid in &self.selected_ids.clone() {
            if let Some(c) = self.form.find_control(sid) {
                let old_z = c.z_order;
                let new_z = old_z - 1;
                self.apply(Cmd::SetZOrder { id: sid.clone(), old_z, new_z });
            }
        }
    }

    // ── Alignment ─────────────────────────────────────────────────────────────

    fn selected_rects(&self) -> Vec<(String, cobolt_forms::model::Rect)> {
        self.selected_ids.iter()
            .filter_map(|id| self.form.find_control(id).map(|c| (id.clone(), c.rect)))
            .collect()
    }

    pub fn align_left(&mut self) {
        let rects = self.selected_rects();
        if rects.len() < 2 { return; }
        let min_x = rects.iter().map(|(_, r)| r.x).min().unwrap();
        let moves: Vec<Cmd> = rects.iter()
            .filter(|(_, r)| r.x != min_x)
            .map(|(id, r)| Cmd::MoveControl { id: id.clone(), old_x: r.x, old_y: r.y, new_x: min_x, new_y: r.y })
            .collect();
        for cmd in moves { self.apply(cmd); }
    }

    pub fn align_right(&mut self) {
        let rects = self.selected_rects();
        if rects.len() < 2 { return; }
        let max_right = rects.iter().map(|(_, r)| r.x + r.w).max().unwrap();
        let moves: Vec<Cmd> = rects.iter()
            .map(|(id, r)| { let nx = max_right - r.w; Cmd::MoveControl { id: id.clone(), old_x: r.x, old_y: r.y, new_x: nx, new_y: r.y } })
            .filter(|c| matches!(c, Cmd::MoveControl { new_x, old_x, .. } if new_x != old_x))
            .collect();
        for cmd in moves { self.apply(cmd); }
    }

    pub fn align_top(&mut self) {
        let rects = self.selected_rects();
        if rects.len() < 2 { return; }
        let min_y = rects.iter().map(|(_, r)| r.y).min().unwrap();
        let moves: Vec<Cmd> = rects.iter()
            .filter(|(_, r)| r.y != min_y)
            .map(|(id, r)| Cmd::MoveControl { id: id.clone(), old_x: r.x, old_y: r.y, new_x: r.x, new_y: min_y })
            .collect();
        for cmd in moves { self.apply(cmd); }
    }

    pub fn align_bottom(&mut self) {
        let rects = self.selected_rects();
        if rects.len() < 2 { return; }
        let max_bottom = rects.iter().map(|(_, r)| r.y + r.h).max().unwrap();
        let moves: Vec<Cmd> = rects.iter()
            .map(|(id, r)| { let ny = max_bottom - r.h; Cmd::MoveControl { id: id.clone(), old_x: r.x, old_y: r.y, new_x: r.x, new_y: ny } })
            .filter(|c| matches!(c, Cmd::MoveControl { new_y, old_y, .. } if new_y != old_y))
            .collect();
        for cmd in moves { self.apply(cmd); }
    }

    pub fn center_horizontal(&mut self) {
        let rects = self.selected_rects();
        if rects.len() < 2 { return; }
        let avg_cx = rects.iter().map(|(_, r)| r.x + r.w/2).sum::<i32>() / rects.len() as i32;
        let moves: Vec<Cmd> = rects.iter()
            .map(|(id, r)| { let nx = avg_cx - r.w/2; Cmd::MoveControl { id: id.clone(), old_x: r.x, old_y: r.y, new_x: nx, new_y: r.y } })
            .collect();
        for cmd in moves { self.apply(cmd); }
    }

    pub fn center_vertical(&mut self) {
        let rects = self.selected_rects();
        if rects.len() < 2 { return; }
        let avg_cy = rects.iter().map(|(_, r)| r.y + r.h/2).sum::<i32>() / rects.len() as i32;
        let moves: Vec<Cmd> = rects.iter()
            .map(|(id, r)| { let ny = avg_cy - r.h/2; Cmd::MoveControl { id: id.clone(), old_x: r.x, old_y: r.y, new_x: r.x, new_y: ny } })
            .collect();
        for cmd in moves { self.apply(cmd); }
    }

    pub fn space_evenly_horizontal(&mut self) {
        let mut rects = self.selected_rects();
        if rects.len() < 3 { return; }
        rects.sort_by_key(|(_, r)| r.x);
        let total_w: i32 = rects.iter().map(|(_, r)| r.w).sum();
        let span = (rects.last().unwrap().1.x + rects.last().unwrap().1.w) - rects[0].1.x;
        let gap = (span - total_w).max(0) / (rects.len() as i32 - 1);
        let mut x = rects[0].1.x;
        for (id, r) in &rects {
            let nx = x;
            if nx != r.x {
                let _ = self.apply(Cmd::MoveControl { id: id.clone(), old_x: r.x, old_y: r.y, new_x: nx, new_y: r.y });
            }
            x += r.w + gap;
        }
    }

    pub fn space_evenly_vertical(&mut self) {
        let mut rects = self.selected_rects();
        if rects.len() < 3 { return; }
        rects.sort_by_key(|(_, r)| r.y);
        let total_h: i32 = rects.iter().map(|(_, r)| r.h).sum();
        let span = (rects.last().unwrap().1.y + rects.last().unwrap().1.h) - rects[0].1.y;
        let gap = (span - total_h).max(0) / (rects.len() as i32 - 1);
        let mut y = rects[0].1.y;
        for (id, r) in &rects {
            let ny = y;
            if ny != r.y {
                let _ = self.apply(Cmd::MoveControl { id: id.clone(), old_x: r.x, old_y: r.y, new_x: r.x, new_y: ny });
            }
            y += r.h + gap;
        }
    }

    /// Auto-arrange: find (Label, input) pairs by LabelFor and stack them in rows.
    /// Labels go on the left column, inputs on the right, aligned vertically.
    pub fn auto_arrange_labels(&mut self) {
        // Collect (label_id, input_id) pairs from LabelFor properties
        let pairs: Vec<(String, String)> = self.form.controls.iter()
            .filter(|c| c.control_type == ControlType::Label)
            .filter_map(|lbl| {
                let for_id = lbl.get_prop("LabelFor")
                    .and_then(|v| if v.as_str().is_empty() { None } else { Some(v.as_str().to_owned()) })?;
                // Verify the target exists
                if self.form.find_control(&for_id).is_some() {
                    Some((lbl.id.clone(), for_id))
                } else {
                    None
                }
            })
            .collect();

        if pairs.is_empty() { return; }

        let margin_x = 16;
        let margin_y = 24;
        let label_w  = 120;
        let gap_x    = 8;
        let row_h    = 28;

        let mut y = margin_y;
        for (lbl_id, inp_id) in pairs {
            // Move label
            let lbl_rect = self.form.find_control(&lbl_id).map(|c| c.rect);
            let inp_rect = self.form.find_control(&inp_id).map(|c| c.rect);

            if let (Some(lr), Some(ir)) = (lbl_rect, inp_rect) {
                // Center label vertically with input
                let lbl_y = y + (ir.h - lr.h) / 2;
                self.apply(Cmd::MoveControl { id: lbl_id, old_x: lr.x, old_y: lr.y, new_x: margin_x, new_y: lbl_y });
                self.apply(Cmd::MoveControl { id: inp_id, old_x: ir.x, old_y: ir.y, new_x: margin_x + label_w + gap_x, new_y: y });
                y += ir.h.max(lr.h) + row_h / 2;
            }
        }
    }

    /// Open the modal COBOL code editor for `event_name` on control `ctrl_id`.
    /// Pass an empty `ctrl_id` for form-level events (OnLoad, OnClose).
    pub fn open_event_modal(&mut self, ctrl_id: &str, event_name: &str) {
        // Find the event binding — either in a control or in form_events.
        let (program_id, ws_buf, proc_buf, display) = if ctrl_id.is_empty() {
            // Form-level event
            let ev = self.form.form_events.iter().find(|e| e.event == event_name);
            let (pid, ws, code) = ev.map(|e| (e.paragraph.clone(), e.local_ws.clone(), e.code.clone()))
                .unwrap_or_else(|| {
                    // Auto-generate paragraph name
                    let pid = format!("{}--{}",
                        self.form.name,
                        event_name.to_ascii_uppercase().replace(' ', "-"));
                    (pid, String::new(), String::new())
                });
            (pid, ws, code, format!("Form · {}", event_name))
        } else {
            let ev = self.form.find_control(ctrl_id)
                .and_then(|c| c.events.iter().find(|e| e.event == event_name));
            let (pid, ws, code) = ev.map(|e| (e.paragraph.clone(), e.local_ws.clone(), e.code.clone()))
                .unwrap_or_else(|| {
                    let pid = format!("{}--{}",
                        ctrl_id.to_ascii_uppercase(),
                        event_name.to_ascii_uppercase().replace(' ', "-"));
                    (pid, String::new(), String::new())
                });
            (pid, ws, code, format!("{} · {}", ctrl_id, event_name))
        };

        self.event_modal = Some(EventEditorModal::new(
            ctrl_id, display, event_name, program_id, ws_buf, proc_buf,
        ));
    }

    /// Commit the modal editor's content back into the form's event binding.
    pub fn save_event_handler(&mut self, ctrl_id: &str, event_name: &str, ws: String, code: String) {
        if ctrl_id.is_empty() {
            // Form-level event — create the binding if it doesn't exist yet
            // (only onLoad/onClose are pre-stubbed; the rest are created lazily).
            if !self.form.form_events.iter().any(|e| e.event == event_name) {
                let paragraph = cobolt_forms::model::derive_paragraph_name(&self.form.name, event_name);
                self.form.form_events.push(cobolt_forms::EventBinding {
                    event:     event_name.to_string(),
                    paragraph,
                    code:      String::new(),
                    local_ws:  String::new(),
                });
            }
            if let Some(ev) = self.form.form_events.iter_mut().find(|e| e.event == event_name) {
                ev.local_ws = ws;
                ev.code     = code;
                self.dirty  = true;
            }
        } else if let Some(ctrl) = self.form.find_control_mut(ctrl_id) {
            ctrl.ensure_event(event_name);
            if let Some(ev) = ctrl.events.iter_mut().find(|e| e.event == event_name) {
                ev.local_ws = ws;
                ev.code     = code;
            }
            self.dirty = true;
        }
    }

    pub fn set_property(&mut self, ctrl_id: &str, key: &str, value: PropValue) {
        // ── Animation management meta-keys ────────────────────────────────────
        if key == "_AddAnimation" {
            if let Some(ctrl) = self.form.find_control_mut(ctrl_id) {
                ctrl.add_animation(AnimationDef::new(value.as_str()));
                self.dirty = true;
            }
            return;
        }
        if let Some(idx_str) = key.strip_prefix("_RemoveAnim") {
            if let Ok(idx) = idx_str.parse::<usize>() {
                if let Some(ctrl) = self.form.find_control_mut(ctrl_id) {
                    if idx < ctrl.animations.len() {
                        ctrl.animations.remove(idx);
                        self.dirty = true;
                    }
                }
            }
            return;
        }
        if let Some(idx_str) = key.strip_prefix("_PreviewAnim") {
            if let Ok(idx) = idx_str.parse::<usize>() {
                if let Some(ctrl) = self.form.find_control(ctrl_id) {
                    if let Some(anim) = ctrl.animations.get(idx) {
                        let anim_name = anim.name.clone();
                        self.play_animation_preview(ctrl_id, &anim_name);
                    }
                }
            }
            return;
        }
        // ── Animation field updates (Anim{N}_Kind, etc.) ──────────────────────
        if let Some(rest) = key.strip_prefix("Anim") {
            if let Some(us) = rest.find('_') {
                let idx_str = &rest[..us];
                let field   = &rest[us+1..];
                if let Ok(idx) = idx_str.parse::<usize>() {
                    if let Some(ctrl) = self.form.find_control_mut(ctrl_id) {
                        if let Some(anim) = ctrl.animations.get_mut(idx) {
                            match field {
                                "Name"     => anim.name        = value.as_str().to_owned(),
                                "Trigger"  => anim.trigger     = AnimTrigger::from_str(value.as_str()),
                                "Kind"     => anim.kind        = AnimKind::from_str(value.as_str()),
                                "Duration" => anim.duration_ms = value.as_i64().max(1) as u64,
                                "Delay"    => anim.delay_ms    = value.as_i64().max(0) as u64,
                                "Easing"   => anim.easing      = EasingKind::from_str(value.as_str()),
                                "Repeat"   => anim.repeat      = match value.as_str() {
                                    "Loop"     => AnimRepeat::Loop,
                                    "PingPong" => AnimRepeat::PingPong,
                                    "Count"    => AnimRepeat::Count(3),
                                    _          => AnimRepeat::Once,
                                },
                                "SlideDX"  => anim.slide_dx    = value.as_i64() as i32,
                                "SlideDY"  => anim.slide_dy    = value.as_i64() as i32,
                                _          => {}
                            }
                            self.dirty = true;
                        }
                    }
                }
            }
            return;
        }

        match key {
            "X" | "Y" | "Width" | "Height" => {
                let old_opt = self.form.find_control(ctrl_id).map(|c| c.rect);
                if let Some(old_rect) = old_opt {
                    let mut new_rect = old_rect;
                    match key {
                        "X"      => new_rect.x = value.as_i64() as i32,
                        "Y"      => new_rect.y = value.as_i64() as i32,
                        "Width"  => new_rect.w = (value.as_i64() as i32).max(1),
                        "Height" => new_rect.h = (value.as_i64() as i32).max(1),
                        _        => {}
                    }
                    if new_rect != old_rect {
                        self.apply(Cmd::ResizeControl { id: ctrl_id.to_owned(), old_rect, new_rect });
                    }
                }
            }
            "ZOrder" => {
                if let Some(c) = self.form.find_control(ctrl_id) {
                    let old_z = c.z_order;
                    let new_z = value.as_i64() as i32;
                    if old_z != new_z {
                        self.apply(Cmd::SetZOrder { id: ctrl_id.to_owned(), old_z, new_z });
                    }
                }
            }
            "Visible"  => { if let Some(c) = self.form.find_control_mut(ctrl_id) { c.visible   = value.as_bool(); self.dirty = true; } }
            "Enabled"  => { if let Some(c) = self.form.find_control_mut(ctrl_id) { c.enabled   = value.as_bool(); self.dirty = true; } }
            "TabOrder" => { if let Some(c) = self.form.find_control_mut(ctrl_id) { c.tab_order = value.as_i64() as u32; self.dirty = true; } }
            _ => {
                // When the ImagePath changes, evict the old texture from cache
                if key == "ImagePath" {
                    if let Some(old_path) = self.form.find_control(ctrl_id)
                        .and_then(|c| c.get_prop("ImagePath"))
                        .map(|v| v.as_str().to_owned())
                    {
                        self.image_cache.remove(&old_path);
                    }
                    // Also evict the new path in case the file changed on disk
                    self.image_cache.remove(value.as_str());
                }
                let old = self.form.find_control(ctrl_id).and_then(|c| c.properties.get(key).cloned());
                self.apply(Cmd::SetProperty { id: ctrl_id.to_owned(), key: key.to_owned(), old, new: value });
            }
        }
    }

    pub fn set_form_prop(&mut self, key: &str, value: String) {
        match key {
            "Title"     => { self.form.title            = value; self.dirty = true; }
            "BackgroundColor" => { self.form.background_color = value.trim_start_matches('#').to_owned(); self.dirty = true; }
            "Width"     => { if let Ok(w) = value.parse::<u32>() { self.form.width  = w.max(64); self.dirty = true; } }
            "Height"    => { if let Ok(h) = value.parse::<u32>() { self.form.height = h.max(64); self.dirty = true; } }
            "Transparency"    => { if let Ok(v) = value.parse::<u8>() { self.form.transparency = v.min(100); self.dirty = true; } }
            "GridSize"        => { if let Ok(v) = value.parse::<u8>() { self.form.grid_size = v.clamp(4, 64); self.dirty = true; } }
            "SnapToGrid"      => { self.form.snap_to_grid = value == "true" || value == "1"; self.dirty = true; }
            "Target"          => {
                if let Some((w, h)) = target_preset_size(&value) {
                    self.form.width  = w;
                    self.form.height = h;
                }
                self.form.target = value;
                self.dirty = true;
            }
            "BackgroundImage" => {
                // Evict old cache entry if path changed
                if self.form.background_image != value {
                    self.image_cache.remove(&self.form.background_image);
                }
                self.form.background_image = value;
                self.dirty = true;
            }
            "BgImageMode"     => { self.form.bg_image_mode = BgImageMode::from_str(&value); self.dirty = true; }
            _ => {}
        }
    }

    /// Trigger animation preview for a control by animation name.
    pub fn play_animation_preview(&mut self, ctrl_id: &str, anim_name: &str) {
        // Look up the delay so we honour it during preview.
        let delay_secs = self.form.find_control(ctrl_id)
            .and_then(|c| c.animations.iter().find(|a| a.name == anim_name))
            .map(|a| a.delay_ms as f32 / 1000.0)
            .unwrap_or(0.0);
        let state = self.anim_states
            .entry(format!("{ctrl_id}:{anim_name}"))
            .or_insert_with(|| AnimState::new(anim_name));
        state.play(delay_secs);
    }

    /// Whether there is an undoable command on the stack (drives the toolbar Undo icon).
    pub(crate) fn can_undo(&self) -> bool { !self.undo_stack.is_empty() }

    /// Whether there is a redoable command on the stack (drives the toolbar Redo icon).
    pub(crate) fn can_redo(&self) -> bool { !self.redo_stack.is_empty() }

    /// Toggle the format-painter state machine (same logic as the old toolbar click).
    pub(crate) fn toggle_format_painter(&mut self) {
        match &self.format_painter {
            FormatPainter::WaitingForTarget { .. } | FormatPainter::WaitingForSource => {
                self.format_painter = FormatPainter::Idle;
            }
            FormatPainter::Idle => {
                if let Some(sid) = self.selected_ids.first().cloned() {
                    if let Some(src) = self.form.find_control(&sid) {
                        let props = src.properties.iter()
                            .filter(|(k, _)| STYLE_PROP_KEYS.contains(&k.as_str()))
                            .map(|(k, v)| (k.clone(), v.clone()))
                            .collect();
                        let animations = src.animations.clone();
                        let src_rect = src.rect.clone();
                        self.format_painter = FormatPainter::WaitingForTarget { props, animations, src_rect };
                    }
                }
            }
        }
    }

    /// Play all OnFormLoad animations (Preview Anims button).
    pub(crate) fn play_all_form_load_anims(&mut self) {
        let ctrl_anims: Vec<(String, String)> = self.form.controls.iter()
            .flat_map(|c| c.animations.iter()
                .filter(|a| a.trigger == AnimTrigger::OnFormLoad)
                .map(move |a| (c.id.clone(), a.name.clone())))
            .collect();
        for (cid, aname) in ctrl_anims {
            self.play_animation_preview(&cid, &aname);
        }
    }

    // ── Rendering ─────────────────────────────────────────────────────────────

    pub fn show(&mut self, ui: &mut Ui) -> bool {
        let mut selection_changed = false;

        // Step animation previews
        let now = std::time::Instant::now();
        let dt = if let Some(last) = self.last_frame_time {
            now.duration_since(last).as_secs_f32()
        } else {
            0.0
        };
        self.last_frame_time = Some(now);

        // Advance animation previews
        if dt > 0.0 {
            let mut need_repaint = false;
            // Collect animation definitions: key -> (duration_ms, delay_ms)
            let anim_meta: HashMap<String, (u64, u64)> = self.form.controls.iter()
                .flat_map(|c| c.animations.iter()
                    .map(move |a| (format!("{}:{}", c.id, a.name), (a.duration_ms, a.delay_ms))))
                .collect();

            for (key, state) in self.anim_states.iter_mut() {
                if !state.playing { continue; }

                // ── Delay phase: count down before t starts moving ────────────
                if state.delay_remaining > 0.0 {
                    state.delay_remaining -= dt;
                    if state.delay_remaining < 0.0 { state.delay_remaining = 0.0; }
                    need_repaint = true;
                    continue; // don't advance t yet
                }

                let dur = anim_meta.get(key).map(|(d, _)| *d).unwrap_or(400) as f32 / 1000.0;
                if dur <= 0.0 { state.stop(); continue; }
                state.t += dt / dur;
                if state.t >= 1.0 {
                    state.t = 1.0;
                    state.playing = false;
                }
                need_repaint = true;
            }
            if need_repaint { ui.ctx().request_repaint(); }
        }

        let canvas_w = self.form.width  as f32;
        let canvas_h = self.form.height as f32;

        egui::ScrollArea::both()
            .id_salt("designer_canvas")
            .show(ui, |ui| {
                let (resp, painter) = ui.allocate_painter(
                    Vec2::new(canvas_w, canvas_h),
                    Sense::click_and_drag(),
                );
                let origin = resp.rect.min;

                // ── Form canvas background ─────────────────────────────────────
                // BackColor (RRGGBBAA hex) controls fill + alpha.
                // Transparent (alpha=0) means the wallpaper shows through.
                // form.transparency (0=opaque..100=fully transparent) also dims the canvas.
                let form_alpha_mul = 1.0 - (self.form.transparency as f32 / 100.0);
                let bg_raw = parse_color(&self.form.background_color);
                // Apply form transparency to background alpha
                let bg = Color32::from_rgba_premultiplied(
                    bg_raw.r(), bg_raw.g(), bg_raw.b(),
                    ((bg_raw.a() as f32) * form_alpha_mul) as u8,
                );
                if self.glass_mode {
                    let corner = egui::Rounding::same(6.0);
                    if bg.a() > 0 {
                        painter.rect_filled(resp.rect, corner, bg);
                    }
                    // Thin border so the form boundary is always visible
                    painter.rect_stroke(resp.rect, corner,
                        egui::Stroke::new(1.0,
                            Color32::from_rgba_unmultiplied(255, 255, 255, 60)));
                } else {
                    painter.rect_filled(resp.rect, 0.0, bg);
                }

                // ── Background image ───────────────────────────────────────────
                let bg_img_path = self.form.background_image.clone();
                let bg_img_mode = self.form.bg_image_mode;
                if !bg_img_path.is_empty() {
                    let ctx_ref2 = ui.ctx().clone();
                    self.load_image(&bg_img_path, &ctx_ref2);
                    let img_alpha = (255.0 * form_alpha_mul) as u8;
                    if img_alpha > 0 {
                    if let Some(tex) = self.image_cache.get(&bg_img_path).and_then(|o| o.as_ref()) {
                        let tex_size = tex.size_vec2();
                        // White tint at varying alpha — no color modulation, just transparency
                        let tint = Color32::from_rgba_premultiplied(255, 255, 255, img_alpha);
                        let tex_id = tex.id();
                        let form_rect = resp.rect;
                        match bg_img_mode {
                            BgImageMode::Stretch => {
                                painter.image(tex_id, form_rect, egui::Rect::from_min_max(egui::pos2(0.0,0.0), egui::pos2(1.0,1.0)), tint);
                            }
                            BgImageMode::Fill => {
                                // Scale so image fills the whole form keeping aspect ratio (crops if needed)
                                let sx = form_rect.width()  / tex_size.x;
                                let sy = form_rect.height() / tex_size.y;
                                let s  = sx.max(sy);
                                let dw = tex_size.x * s;
                                let dh = tex_size.y * s;
                                let ox = (form_rect.width()  - dw) / 2.0;
                                let oy = (form_rect.height() - dh) / 2.0;
                                let dest = egui::Rect::from_min_size(
                                    form_rect.min + egui::vec2(ox, oy),
                                    egui::vec2(dw, dh),
                                );
                                painter.image(tex_id, dest, egui::Rect::from_min_max(egui::pos2(0.0,0.0), egui::pos2(1.0,1.0)), tint);
                            }
                            BgImageMode::Fit => {
                                // Scale so whole image fits inside form, keeping aspect ratio (letterbox)
                                let sx = form_rect.width()  / tex_size.x;
                                let sy = form_rect.height() / tex_size.y;
                                let s  = sx.min(sy);
                                let dw = tex_size.x * s;
                                let dh = tex_size.y * s;
                                let ox = (form_rect.width()  - dw) / 2.0;
                                let oy = (form_rect.height() - dh) / 2.0;
                                let dest = egui::Rect::from_min_size(
                                    form_rect.min + egui::vec2(ox, oy),
                                    egui::vec2(dw, dh),
                                );
                                painter.image(tex_id, dest, egui::Rect::from_min_max(egui::pos2(0.0,0.0), egui::pos2(1.0,1.0)), tint);
                            }
                            BgImageMode::Center => {
                                let ox = (form_rect.width()  - tex_size.x) / 2.0;
                                let oy = (form_rect.height() - tex_size.y) / 2.0;
                                let dest = egui::Rect::from_min_size(
                                    form_rect.min + egui::vec2(ox, oy),
                                    tex_size,
                                );
                                painter.image(tex_id, dest, egui::Rect::from_min_max(egui::pos2(0.0,0.0), egui::pos2(1.0,1.0)), tint);
                            }
                            BgImageMode::Tile => {
                                // Tile the image across the form canvas
                                let tw = tex_size.x.max(1.0);
                                let th = tex_size.y.max(1.0);
                                let cols = (form_rect.width()  / tw).ceil() as i32 + 1;
                                let rows = (form_rect.height() / th).ceil() as i32 + 1;
                                for row in 0..rows {
                                    for col in 0..cols {
                                        let tile_min = form_rect.min + egui::vec2(col as f32 * tw, row as f32 * th);
                                        let tile_max = egui::pos2(
                                            (tile_min.x + tw).min(form_rect.max.x),
                                            (tile_min.y + th).min(form_rect.max.y),
                                        );
                                        if tile_min.x >= form_rect.max.x || tile_min.y >= form_rect.max.y { continue; }
                                        let u1 = (tile_max.x - tile_min.x) / tw;
                                        let v1 = (tile_max.y - tile_min.y) / th;
                                        let dest_tile = egui::Rect::from_min_max(tile_min, tile_max);
                                        painter.image(tex_id, dest_tile, egui::Rect::from_min_max(egui::pos2(0.0,0.0), egui::pos2(u1,v1)), tint);
                                    }
                                }
                            }
                        }
                    } // if let Some(tex)
                    } // if img_alpha > 0
                }

                // Grid
                if self.show_grid {
                    let gstep = self.form.grid_size.max(4) as f32;
                    draw_grid(&painter, resp.rect, gstep, self.glass_mode);
                }

                // Pointer position in canvas space
                let ptr_canvas: Option<(i32, i32)> = ui.ctx()
                    .pointer_interact_pos()
                    .map(|p| { let rel = p - origin; (rel.x as i32, rel.y as i32) });

                // Show pointer cursor when hovering over any control on the canvas.
                if let Some((cx, cy)) = ptr_canvas {
                    let over_ctrl = self.form.controls.iter().any(|c| c.rect.contains(cx, cy));
                    if over_ctrl {
                        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                    }
                }

                // Handle drag input
                self.handle_drag(&resp, &painter, origin, ptr_canvas, &mut selection_changed);

                // Draw controls sorted by z_order
                let selected_ids = self.selected_ids.clone();
                let form_w = self.form.width  as f32;
                let form_h = self.form.height as f32;

                // Build render list sorted by z_order
                let mut render_order: Vec<usize> = (0..self.form.controls.len()).collect();
                render_order.sort_by_key(|&i| self.form.controls[i].z_order);

                // Pre-collect PictureBox image paths before the borrow-split below
                let pic_paths: Vec<(usize, String)> = render_order.iter().map(|&idx| {
                    let ctrl = &self.form.controls[idx];
                    let path = if matches!(ctrl.control_type, cobolt_forms::ControlType::PictureBox) {
                        ctrl.get_prop("ImagePath").map(|v| v.as_str().to_owned()).unwrap_or_default()
                    } else { String::new() };
                    (idx, path)
                }).collect();

                // Eagerly load/cache textures for all PictureBox controls this frame
                let ctx_ref = ui.ctx().clone();
                for (_, path) in &pic_paths {
                    if !path.is_empty() {
                        self.load_image(path, &ctx_ref);
                    }
                }

                for (idx, img_path) in &pic_paths {
                    let ctrl = &self.form.controls[*idx];
                    let is_sel = selected_ids.contains(&ctrl.id);

                    // Check if an animation preview is playing for this control
                    let anim_offset = ctrl.animations.iter()
                        .find_map(|a| {
                            let key = format!("{}:{}", ctrl.id, a.name);
                            self.anim_states.get(&key).filter(|s| s.playing || s.t > 0.0 && s.t < 1.0)
                                .map(|s| anim_transform(a, form_w, form_h, s.t))
                        })
                        .unwrap_or((0.0, 0.0, 1.0, 1.0));

                    let (adx, ady, scale, alpha_mul) = anim_offset;
                    let anim_origin = origin + Vec2::new(adx, ady);

                    // Combine animation alpha with the control's Opacity property (0–100)
                    let ctrl_opacity = ctrl.get_prop("Opacity")
                        .map(|v| (v.as_i64() as f32 / 100.0).clamp(0.0, 1.0))
                        .unwrap_or(1.0);
                    let effective_alpha = (alpha_mul * ctrl_opacity).clamp(0.0, 1.0);

                    // Retrieve cached texture for PictureBox (if any)
                    let pic_tex: Option<egui::TextureId> = if !img_path.is_empty() {
                        self.image_cache.get(img_path).and_then(|o| o.as_ref()).map(|h| h.id())
                    } else { None };

                    draw_control(&painter, anim_origin, ctrl, is_sel, self.glass_mode, effective_alpha, scale, pic_tex);

                    // Animation badge tooltip — show animation list on hover
                    if !ctrl.animations.is_empty() {
                        let r = &ctrl.rect;
                        let badge_pos = origin
                            + Vec2::new(r.x as f32 + r.w as f32 * scale - 2.0, r.y as f32 + 2.0);
                        let badge_rect = egui::Rect::from_center_size(badge_pos, Vec2::splat(12.0));
                        let anim_summary: String = ctrl.animations.iter()
                            .map(|a| format!("▶ {} ({:?})", a.name, a.trigger))
                            .collect::<Vec<_>>()
                            .join("\n");
                        let tooltip = format!("Animations set:\n{anim_summary}");
                        ui.interact(badge_rect, egui::Id::new(("anim_badge", ctrl.id.as_str())), egui::Sense::hover())
                            .on_hover_text(tooltip);
                    }
                }

                // Draw the form's own resize grips (right / bottom / corner).
                let active_form_edge = match self.drag {
                    DragState::ResizingForm { edge, .. } => Some(edge),
                    _ => self.press_form_edge,
                };
                draw_form_resize_grips(&painter, resp.rect, active_form_edge, self.glass_mode);

                // Draw selection handles over the primary selected control
                if let Some(sid) = self.selected_ids.first() {
                    if let Some(ctrl) = self.form.find_control(sid) {
                        draw_handles(&painter, origin, &ctrl.rect, self.glass_mode);
                    }
                }
                // Draw secondary selection highlight boxes
                for sid in self.selected_ids.iter().skip(1) {
                    if let Some(ctrl) = self.form.find_control(sid) {
                        let r = ctrl.rect;
                        let rect = egui::Rect::from_min_size(
                            origin + Vec2::new(r.x as f32, r.y as f32),
                            Vec2::new(r.w as f32, r.h as f32),
                        );
                        painter.rect_stroke(rect, 2.0, Stroke::new(1.5, Color32::from_rgba_premultiplied(100,200,255,200)));
                    }
                }

                // Draw rubber-band rectangle
                if let DragState::RubberBand { start_x, start_y, cur_x, cur_y } = self.drag {
                    let x0 = start_x.min(cur_x) as f32 + origin.x;
                    let y0 = start_y.min(cur_y) as f32 + origin.y;
                    let x1 = start_x.max(cur_x) as f32 + origin.x;
                    let y1 = start_y.max(cur_y) as f32 + origin.y;
                    let band_rect = egui::Rect::from_min_max(Pos2::new(x0,y0), Pos2::new(x1,y1));
                    painter.rect_filled(band_rect, 0.0, Color32::from_rgba_premultiplied(80,140,255,30));
                    painter.rect_stroke(band_rect, 0.0, Stroke::new(1.0, Color32::from_rgba_premultiplied(100,170,255,220)));
                }

                // Right-click context menu
                resp.context_menu(|ui| {
                    if ui.button("🗑 Delete").clicked() { self.delete_selected(); ui.close_menu(); }
                    ui.separator();
                    if ui.button("⬆ Bring to Front").clicked() { self.bring_to_front(); ui.close_menu(); }
                    if ui.button("⬇ Send to Back").clicked()   { self.send_to_back();   ui.close_menu(); }
                    if ui.button("+1 Forward").clicked()        { self.bring_forward();  ui.close_menu(); }
                    if ui.button("-1 Backward").clicked()       { self.send_backward();  ui.close_menu(); }
                    ui.separator();
                    // Play animations
                    let anim_preview: Option<(String, Vec<String>)> =
                        self.selected_ids.first().cloned().and_then(|sid| {
                            self.form.find_control(&sid).and_then(|ctrl| {
                                if ctrl.animations.is_empty() { None }
                                else {
                                    Some((sid, ctrl.animations.iter().map(|a| a.name.clone()).collect()))
                                }
                            })
                        });
                    if let Some((sid, anim_names)) = anim_preview {
                        ui.menu_button("▶ Preview Animation", |ui| {
                            for aname in &anim_names {
                                if ui.button(aname).clicked() {
                                    self.play_animation_preview(&sid, aname);
                                    ui.close_menu();
                                }
                            }
                        });
                    }
                    ui.separator();
                    if ui.button("🏷 Auto-arrange Labels").clicked() { self.auto_arrange_labels(); ui.close_menu(); }
                });

                // Click on canvas — select / deselect
                if resp.clicked() {
                    let ctrl_held = ui.ctx().input(|i| i.modifiers.command);
                    if let Some((cx, cy)) = ptr_canvas {
                        let mut hit: Option<String> = None;
                        // Hit-test in reverse z_order (topmost first)
                        let mut hit_order: Vec<usize> = (0..self.form.controls.len()).collect();
                        hit_order.sort_by_key(|&i| std::cmp::Reverse(self.form.controls[i].z_order));
                        for idx in hit_order {
                            let ctrl = &self.form.controls[idx];
                            if ctrl.rect.contains(cx, cy) { hit = Some(ctrl.id.clone()); break; }
                        }
                        if ctrl_held {
                            // Ctrl+click = toggle in multi-select
                            if let Some(id) = hit {
                                self.toggle_selected(&id);
                                selection_changed = true;
                            }
                        } else {
                            let hit_same = self.selected_ids.len() == 1
                                && hit.as_deref() == self.selected_ids.first().map(|s| s.as_str());
                            if !hit_same {
                                self.set_selected_one(hit);
                                selection_changed = true;
                            }
                        }
                    }
                }
            });

        // Keyboard shortcuts
        let ctx = ui.ctx();
        if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            // ESC cancels format painter if active
            if !matches!(self.format_painter, FormatPainter::Idle) {
                self.format_painter = FormatPainter::Idle;
            }
        }
        // Delete key: on macOS the physical Delete key sends Backspace; forward-delete sends Delete.
        // Accept both so that the delete action works on all platforms.
        // Guard: only fire when no text-input widget has keyboard focus (i.e. the user is
        // not editing a property field, animation name, etc. in the properties panel).
        let no_text_focus = ctx.memory(|m| m.focused().is_none());
        let want_delete = no_text_focus && !self.selected_ids.is_empty() && ctx.input(|i| {
            (i.key_pressed(egui::Key::Delete) || i.key_pressed(egui::Key::Backspace))
                && !i.modifiers.command  // don't eat Cmd+Backspace (system shortcuts)
        });
        if want_delete { self.delete_selected(); }
        if ctx.input(|i| i.key_pressed(egui::Key::Z) && i.modifiers.command && !i.modifiers.shift) { self.undo(); }
        if ctx.input(|i| i.key_pressed(egui::Key::Y) && i.modifiers.command) { self.redo(); }
        if ctx.input(|i| i.key_pressed(egui::Key::Z) && i.modifiers.command && i.modifiers.shift) { self.redo(); }
        if ctx.input(|i| i.key_pressed(egui::Key::A) && i.modifiers.command) {
            // Select all
            self.selected_ids = self.form.controls.iter().map(|c| c.id.clone()).collect();
            selection_changed = true;
        }

        // ── Event Editor Modal ──────────────────────────────────────────────────
        self.show_event_modal(ui);

        selection_changed
    }

    /// Render the event code editor modal (if open).
    ///
    /// The modal shows a read-only COBOL scaffold around two editable areas:
    ///   • WORKING-STORAGE SECTION  (local data items for this handler)
    ///   • PROCEDURE DIVISION body  (the handler's COBOL statements)
    fn show_event_modal(&mut self, ui: &mut Ui) {
        // Work entirely with a local copy so we can close the modal based on its own state.
        let Some(modal) = self.event_modal.as_mut() else { return };

        // Dim overlay covering the canvas (drawn before the window so it sits behind it)
        let overlay = ui.ctx().screen_rect();
        ui.painter().rect_filled(overlay, 0.0,
            Color32::from_rgba_premultiplied(0, 0, 0, 140));

        // Build window title
        let title = format!("COBOL Event Editor  —  {}", modal.ctrl_display);

        // ── Clone the mutable buffers out so we can pass them to the TextEdit widgets
        //    and write results back without borrow conflicts.
        let mut ws_buf   = modal.ws_buf.clone();
        let mut proc_buf = modal.proc_buf.clone();
        let program_id   = modal.program_id.clone();

        let mut save_clicked   = false;
        let mut cancel_clicked = false;

        egui::Window::new(&title)
            .id(egui::Id::new("event_editor_modal"))
            .collapsible(false)
            .resizable(true)
            .min_width(560.0)
            .min_height(480.0)
            .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
            .frame(egui::Frame::window(&ui.ctx().style()).inner_margin(egui::Margin::same(16.0)))
            .show(ui.ctx(), |ui| {
                // ── Read-only scaffold header ────────────────────────────────
                let scaffold_color = Color32::from_rgb(140, 200, 140);  // muted green
                let readonly_color = Color32::from_rgb(160, 170, 190);  // subdued blue-gray

                ui.horizontal(|ui| {
                    ui.monospace(egui::RichText::new("       IDENTIFICATION DIVISION.")
                        .color(readonly_color).size(12.0));
                });
                ui.horizontal(|ui| {
                    ui.monospace(egui::RichText::new(
                        format!("       PROGRAM-ID. {}.", program_id))
                        .color(scaffold_color).size(12.0));
                });
                ui.add_space(4.0);

                // ── DATA DIVISION + editable WS ──────────────────────────────
                ui.horizontal(|ui| {
                    ui.monospace(egui::RichText::new("       DATA DIVISION.")
                        .color(readonly_color).size(12.0));
                });
                ui.horizontal(|ui| {
                    ui.monospace(egui::RichText::new("       WORKING-STORAGE SECTION.")
                        .color(readonly_color).size(12.0));
                });

                // Editable WS area
                let ws_font = egui::FontId::monospace(12.0);
                let ws_resp = ui.add(
                    egui::TextEdit::multiline(&mut ws_buf)
                        .font(ws_font)
                        .desired_width(f32::INFINITY)
                        .desired_rows(4)
                        .hint_text("       *> Add local data items here, e.g.:\n       01 WS-MY-VAR   PIC X(64) VALUE SPACES.")
                        .code_editor(),
                );
                if ws_resp.changed() {}  // handled below via clone swap

                ui.add_space(6.0);

                // ── PROCEDURE DIVISION (read-only label) ─────────────────────
                ui.horizontal(|ui| {
                    ui.monospace(egui::RichText::new("       PROCEDURE DIVISION.")
                        .color(readonly_color).size(12.0));
                });

                // Editable procedure body
                let proc_font = egui::FontId::monospace(12.0);
                let proc_resp = ui.add(
                    egui::TextEdit::multiline(&mut proc_buf)
                        .font(proc_font)
                        .desired_width(f32::INFINITY)
                        .desired_rows(14)
                        .hint_text("           *> Write your COBOL statements here.\n           CONTINUE.")
                        .code_editor(),
                );
                if proc_resp.changed() {}

                ui.add_space(4.0);

                // ── Read-only GOBACK / END PROGRAM footer ────────────────────
                ui.horizontal(|ui| {
                    ui.monospace(egui::RichText::new("           GOBACK.")
                        .color(readonly_color).size(12.0));
                });
                ui.horizontal(|ui| {
                    ui.monospace(egui::RichText::new(
                        format!("       END PROGRAM {}.", program_id))
                        .color(scaffold_color).size(12.0));
                });

                ui.add_space(10.0);
                ui.separator();
                ui.add_space(6.0);

                ui.horizontal(|ui| {
                    if ui.button("💾  Save").clicked()   { save_clicked   = true; }
                    if ui.button("✖  Cancel").clicked() { cancel_clicked = true; }
                });
            });

        // Write updated buffers back into the modal state
        if let Some(m) = self.event_modal.as_mut() {
            m.ws_buf   = ws_buf;
            m.proc_buf = proc_buf;
        }

        if save_clicked {
            let m = self.event_modal.take().unwrap();
            self.save_event_handler(&m.ctrl_id, &m.event_name, m.ws_buf, m.proc_buf);
        } else if cancel_clicked {
            self.event_modal = None;
        }
    }

    fn handle_drag(
        &mut self,
        resp: &egui::Response,
        painter: &egui::Painter,
        origin: Pos2,
        ptr_canvas: Option<(i32, i32)>,
        selection_changed: &mut bool,
    ) {
        let (px, py) = match ptr_canvas { Some(p) => p, None => return };

        // ── Format Painter: intercept clicks while in WaitingForTarget mode ───
        if matches!(self.format_painter, FormatPainter::WaitingForTarget { .. }) {
            resp.ctx.set_cursor_icon(egui::CursorIcon::Crosshair);

            if resp.clicked() {
                // Find which control was clicked (topmost by z-order)
                let mut hit_id: Option<String> = None;
                let mut hit_order: Vec<usize> = (0..self.form.controls.len()).collect();
                hit_order.sort_by_key(|&i| std::cmp::Reverse(self.form.controls[i].z_order));
                for idx in hit_order {
                    if self.form.controls[idx].rect.contains(px, py) {
                        hit_id = Some(self.form.controls[idx].id.clone());
                        break;
                    }
                }
                if let Some(target_id) = hit_id {
                    // Extract captured style before mutably borrowing controls
                    let (props, animations, src_rect) = match std::mem::replace(&mut self.format_painter, FormatPainter::Idle) {
                        FormatPainter::WaitingForTarget { props, animations, src_rect } => (props, animations, src_rect),
                        _ => unreachable!(),
                    };
                    // Paste style + geometry onto the target control
                    if let Some(tgt) = self.form.find_control_mut(&target_id) {
                        for (k, v) in &props {
                            tgt.properties.insert(k.clone(), v.clone());
                        }
                        tgt.animations = animations;
                        // Copy only size (w, h) from source — preserve target's x, y position
                        tgt.rect.w = src_rect.w;
                        tgt.rect.h = src_rect.h;
                    }
                    self.dirty = true;
                }
                return; // Consume the click — don't fall through to selection logic
            }

            // While waiting, also block drag-start so we don't move things
            if resp.drag_started() || resp.dragged() || resp.drag_stopped() {
                return;
            }
        }

        // Check if pointer is currently over a resize handle (for cursor feedback).
        let handle_hover = self.selected_ids.first().and_then(|sid| {
            self.form.find_control(sid).and_then(|ctrl| {
                for &h in &ALL_HANDLES {
                    let hp = handle_pos(&ctrl.rect, h);
                    let dist = ((px as f32 - hp.x).powi(2) + (py as f32 - hp.y).powi(2)).sqrt();
                    if dist < 8.0 { return Some(h); }
                }
                None
            })
        });

        if let Some(h) = handle_hover { resp.ctx.set_cursor_icon(handle_cursor(h)); }

        // Detect hovering the form's own resize border (only when not over a
        // control's resize handle — control handles take priority).
        let form_edge_hover = if handle_hover.is_none() {
            detect_form_edge(px, py, self.form.width as f32, self.form.height as f32)
        } else { None };
        if let Some(e) = form_edge_hover { resp.ctx.set_cursor_icon(form_edge_cursor(e)); }

        // Capture which handle (if any) was under the pointer at the exact moment
        // the mouse button went down.  We must store this NOW because by the time
        // `drag_started()` fires the pointer has already moved away from the handle.
        // Guard with `resp.contains_pointer()` so clicks outside the canvas widget
        // don't overwrite the stored value.
        if resp.contains_pointer() {
            let primary_just_pressed = resp.ctx.input(|i| i.pointer.primary_pressed());
            if primary_just_pressed {
                self.press_handle    = handle_hover;
                self.press_form_edge = form_edge_hover;
            }
        }
        // Clear if the button is no longer held (cancelled press with no drag).
        let primary_held = resp.ctx.input(|i| i.pointer.primary_down());
        if !primary_held {
            self.press_handle    = None;
            self.press_form_edge = None;
        }

        // Begin drag
        if resp.drag_started() {
            match self.drag.clone() {
                DragState::PlacingNew { .. } => {}
                _ => {
                    // Form-edge resize takes priority (captured at press-time).
                    if let Some(edge) = self.press_form_edge.take() {
                        self.drag = DragState::ResizingForm {
                            edge,
                            orig_w: self.form.width  as i32,
                            orig_h: self.form.height as i32,
                            start_x: px, start_y: py,
                        };
                    } else
                    // Use the handle captured at press-time, not the current hover
                    // (the pointer has already moved by the time drag_started fires).
                    if let Some(h) = self.press_handle.take() {
                        if let Some(sid) = self.selected_ids.first().cloned() {
                            if let Some(ctrl) = self.form.find_control(&sid) {
                                self.drag = DragState::ResizingControl {
                                    id: sid, handle: h, orig_rect: ctrl.rect, start_x: px, start_y: py,
                                };
                            }
                        }
                    } else {
                        // Hit-test for move
                        let mut hit_id: Option<String> = None;
                        let mut hit_order: Vec<usize> = (0..self.form.controls.len()).collect();
                        hit_order.sort_by_key(|&i| std::cmp::Reverse(self.form.controls[i].z_order));
                        for idx in hit_order {
                            if self.form.controls[idx].rect.contains(px, py) {
                                hit_id = Some(self.form.controls[idx].id.clone());
                                break;
                            }
                        }
                        if let Some(id) = hit_id {
                            // If not already selected, select it (unless Ctrl held)
                            let ctrl_held = resp.ctx.input(|i| i.modifiers.command);
                            if !self.is_selected(&id) {
                                if ctrl_held { self.selected_ids.push(id.clone()); }
                                else         { self.set_selected_one(Some(id.clone())); }
                                *selection_changed = true;
                            }
                            // Gather origins for all selected controls
                            let origins: Vec<(String, i32, i32)> = self.selected_ids.iter()
                                .filter_map(|sid| self.form.find_control(sid).map(|c| (sid.clone(), c.rect.x, c.rect.y)))
                                .collect();
                            self.drag = DragState::MovingControls { primary_id: id, origins, start_x: px, start_y: py };
                        } else {
                            // Started drag on empty canvas — begin rubber-band
                            self.drag = DragState::RubberBand { start_x: px, start_y: py, cur_x: px, cur_y: py };
                        }
                    }
                }
            }
        }

        // Update drag in-progress
        if resp.dragged() {
            match self.drag.clone() {
                DragState::MovingControls { origins, start_x, start_y, .. } => {
                    let dx = px - start_x;
                    let dy = py - start_y;
                    let gp = self.form.grid_size as i32;
                    let sn = self.form.snap_to_grid;
                    for (id, ox, oy) in &origins {
                        if let Some(ctrl) = self.form.find_control_mut(id) {
                            ctrl.rect.x = snap(ox + dx, gp, sn);
                            ctrl.rect.y = snap(oy + dy, gp, sn);
                        }
                    }
                }
                DragState::ResizingControl { ref id, handle, orig_rect, start_x, start_y } => {
                    let dx = px - start_x;
                    let dy = py - start_y;
                    // Read snap settings before the mutable borrow of find_control_mut.
                    let gp = self.form.grid_size as i32;
                    let sn = self.form.snap_to_grid;
                    if let Some(ctrl) = self.form.find_control_mut(id) {
                        ctrl.rect = apply_resize(orig_rect, handle, dx, dy, gp, sn);
                    }
                }
                DragState::PlacingNew { ref ctrl_type, start_x, start_y, .. } => {
                    self.drag = DragState::PlacingNew { ctrl_type: ctrl_type.clone(), start_x, start_y, cur_x: px, cur_y: py };
                    // Draw ghost preview
                    let x0 = start_x.min(px) as f32 + origin.x;
                    let y0 = start_y.min(py) as f32 + origin.y;
                    let x1 = start_x.max(px) as f32 + origin.x;
                    let y1 = start_y.max(py) as f32 + origin.y;
                    if (x1 - x0) > 4.0 && (y1 - y0) > 4.0 {
                        let ghost = egui::Rect::from_min_max(Pos2::new(x0, y0), Pos2::new(x1, y1));
                        painter.rect_filled(ghost, 2.0, Color32::from_rgba_premultiplied(80,140,255,60));
                        painter.rect_stroke(ghost, 2.0, Stroke::new(1.5, Color32::from_rgb(80,140,255)));
                    }
                }
                DragState::RubberBand { start_x, start_y, .. } => {
                    self.drag = DragState::RubberBand { start_x, start_y, cur_x: px, cur_y: py };
                }
                DragState::ResizingForm { edge, orig_w, orig_h, start_x, start_y } => {
                    let dx = px - start_x;
                    let dy = py - start_y;
                    let gp = self.form.grid_size as i32;
                    let sn = self.form.snap_to_grid;
                    if matches!(edge, FormEdge::Right | FormEdge::Corner) {
                        self.form.width  = snap((orig_w + dx).max(FORM_MIN_SIZE), gp, sn) as u32;
                    }
                    if matches!(edge, FormEdge::Bottom | FormEdge::Corner) {
                        self.form.height = snap((orig_h + dy).max(FORM_MIN_SIZE), gp, sn) as u32;
                    }
                    self.dirty = true;
                }
                DragState::None => {}
            }
        }

        // End drag
        if resp.drag_stopped() {
            match self.drag.clone() {
                DragState::MovingControls { origins, start_x, start_y, .. } => {
                    let dx = px - start_x;
                    let dy = py - start_y;
                    if dx != 0 || dy != 0 {
                        let gp = self.form.grid_size as i32;
                        let sn = self.form.snap_to_grid;
                        let moves: Vec<(String, i32, i32, i32, i32)> = origins.iter()
                            .map(|(id, ox, oy)| (id.clone(), *ox, *oy, snap(ox + dx, gp, sn), snap(oy + dy, gp, sn)))
                            .collect();
                        self.apply(Cmd::MoveMany { moves });
                    }
                }
                DragState::ResizingControl { id, handle, orig_rect, start_x, start_y } => {
                    let dx = px - start_x;
                    let dy = py - start_y;
                    let new_rect = apply_resize(orig_rect, handle, dx, dy,
                        self.form.grid_size as i32, self.form.snap_to_grid);
                    if new_rect != orig_rect {
                        self.apply(Cmd::ResizeControl { id, old_rect: orig_rect, new_rect });
                    }
                }
                DragState::PlacingNew { ctrl_type, start_x, start_y, cur_x, cur_y } => {
                    let x = start_x.min(cur_x);
                    let y = start_y.min(cur_y);
                    let w = (start_x - cur_x).unsigned_abs() as i32;
                    let h = (start_y - cur_y).unsigned_abs() as i32;
                    let (dw, dh) = ctrl_type.default_size();
                    let gp = self.form.grid_size as i32;
                    let sn = self.form.snap_to_grid;
                    let fw = if w > 8 { snap(w, gp, sn) } else { dw };
                    let fh = if h > 8 { snap(h, gp, sn) } else { dh };
                    self.add_control(ctrl_type.clone(), x, y);
                    // resize to drawn size
                    if let Some(sid) = self.selected_ids.first().cloned() {
                        if let Some(ctrl) = self.form.find_control_mut(&sid) {
                            ctrl.rect.w = fw; ctrl.rect.h = fh;
                        }
                    }
                }
                DragState::RubberBand { start_x, start_y, cur_x, cur_y } => {
                    let min_x = start_x.min(cur_x);
                    let min_y = start_y.min(cur_y);
                    let max_x = start_x.max(cur_x);
                    let max_y = start_y.max(cur_y);
                    if (max_x - min_x) > 4 && (max_y - min_y) > 4 {
                        let ctrl_held = resp.ctx.input(|i| i.modifiers.command);
                        if !ctrl_held { self.selected_ids.clear(); }
                        let new_sel: Vec<String> = self.form.controls.iter()
                            .filter(|c| {
                                c.rect.x < max_x && c.rect.x + c.rect.w > min_x &&
                                c.rect.y < max_y && c.rect.y + c.rect.h > min_y
                            })
                            .map(|c| c.id.clone())
                            .collect();
                        for id in new_sel {
                            if !self.selected_ids.contains(&id) {
                                self.selected_ids.push(id);
                            }
                        }
                        *selection_changed = true;
                    }
                }
                DragState::ResizingForm { .. } => {
                    // Final size was applied live during `dragged()`; nothing more to do.
                    self.dirty = true;
                }
                DragState::None => {}
            }
            self.drag = DragState::None;
        }
    }

    /// Called by app.rs toolbox result to start a new control placement drag.
    pub fn start_place(&mut self, ct: ControlType, x: i32, y: i32) {
        self.drag = DragState::PlacingNew { ctrl_type: ct, start_x: x, start_y: y, cur_x: x, cur_y: y };
    }
}

// ── Drawing helpers ───────────────────────────────────────────────────────────

fn draw_grid(painter: &egui::Painter, canvas: egui::Rect, step: f32, glass: bool) {
    let alpha = if glass { 35 } else { 60 };
    let dot_color = Color32::from_rgba_premultiplied(140, 160, 220, alpha);
    let mut x = canvas.min.x;
    while x <= canvas.max.x {
        let mut y = canvas.min.y;
        while y <= canvas.max.y {
            painter.circle_filled(Pos2::new(x, y), 0.7, dot_color);
            y += step;
        }
        x += step;
    }
}

/// Draw a liquid-glass / glassmorphism rectangle that matches the reference aesthetic:
/// predominantly bright-white frosted glass with the background showing through clearly.
///
/// Key principles (from reference image):
///   • Base fill is near-white at very low opacity (~22 %) — not the control's dark colour.
///   • The `base` colour contributes only a faint tint so controls remain distinguishable.
///   • A strong top-to-transparent gradient (vertex mesh) simulates the specular reflection.
///   • A bright crisp inner rim at the very top edge reinforces the glass look.
///   • The bottom third darkens subtly (depth cue).
///   • A soft drop shadow underneath.
///   • A bright white/silver border.
///
/// All colour values are in egui **premultiplied** alpha space:
///   premult_rgb = straight_rgb × (alpha / 255).
/// Frosted-glass disc effect for **circular** controls (Circle shape).
///
/// Uses **radial-gradient polygon fans** (48-sided mesh, centre → edge colour)
/// for perfectly smooth gradients with zero banding.  `circle_filled` has a hard
/// perimeter edge that creates visible concentric rings when layered; fans avoid
/// that entirely because colour is interpolated per-vertex by the GPU.
///
/// Layer order (back → front):
///   1. Drop shadow
///   2. Nearly-transparent frosted body (cool blue-gray tint, ~20 % opacity)
///   3. Top-arc highlight fan   — gentle upper brightening
///   4. Bottom crescent fan     — characteristic glass-disc reflection at base
///   5. Rim stroke
pub(crate) fn draw_glass_circle(
    painter:   &egui::Painter,
    center:    Pos2,
    radius:    f32,
    base:      Color32,
    selected:  bool,
    alpha_mul: f32,
) {
    if alpha_mul <= 0.0 { return; }
    let am = alpha_mul.clamp(0.0, 1.0);

    let white = |alpha: u8| -> Color32 {
        let a = (alpha as f32 * am) as u8;
        Color32::from_rgba_premultiplied(a, a, a, a)
    };
    let pm = |rv: u8, gv: u8, bv: u8, alpha: u8| -> Color32 {
        let a = (alpha as f32 * am) as u8;
        Color32::from_rgba_premultiplied(
            (rv as f32 * am * alpha as f32 / 255.0) as u8,
            (gv as f32 * am * alpha as f32 / 255.0) as u8,
            (bv as f32 * am * alpha as f32 / 255.0) as u8,
            a,
        )
    };

    // Radial-gradient fan: 48-gon, colour goes from `cc` at `origin`
    // to `ce` at the perimeter.  GPU interpolation = perfectly smooth gradient.
    let radial_fan = |origin: Pos2, rad: f32, cc: Color32, ce: Color32|
        -> egui::epaint::Mesh
    {
        let uv = egui::pos2(0.0, 0.0);
        let n  = 48_u32;
        let mut m = egui::epaint::Mesh::default();
        m.vertices.push(egui::epaint::Vertex { pos: origin, uv, color: cc });
        for i in 0..n {
            let a = i as f32 / n as f32 * std::f32::consts::TAU;
            m.vertices.push(egui::epaint::Vertex {
                pos: origin + Vec2::new(a.cos(), a.sin()) * rad,
                uv, color: ce,
            });
        }
        for i in 1..=n {
            let j = if i == n { 1 } else { i + 1 };
            m.indices.extend([0, i, j]);
        }
        m
    };

    // ── 1. Drop shadow ────────────────────────────────────────────────────────
    painter.circle_filled(
        center + Vec2::new(0.0, radius * 0.10),
        radius * 0.97,
        pm(0, 0, 0, 58),
    );

    // ── 2. Frosted body ───────────────────────────────────────────────────────
    // Barely-there tint so the canvas background shows through (real-glass feel).
    // 85 % cool-blue-white (200, 210, 220) + 15 % control base colour, at 20 % opacity.
    let t  = 0.20_f32 * am;
    let fr = ((200.0 * 0.85 + base.r() as f32 * 0.15) * t) as u8;
    let fg = ((210.0 * 0.85 + base.g() as f32 * 0.15) * t) as u8;
    let fb = ((220.0 * 0.85 + base.b() as f32 * 0.15) * t) as u8;
    let fa = (255.0 * t) as u8;
    painter.circle_filled(center, radius,
        Color32::from_rgba_premultiplied(fr, fg, fb, fa));

    // ── 3. Top-arc highlight ──────────────────────────────────────────────────
    // Subtle brightening in the upper third — centre at -30 % of radius.
    let top_c = center + Vec2::new(0.0, -radius * 0.30);
    painter.add(egui::Shape::mesh(radial_fan(
        top_c, radius * 0.65,
        white(52),   // centre: soft white
        white(0),    // edge:   fully transparent
    )));

    // ── 4. Bottom crescent reflection ─────────────────────────────────────────
    // The defining glass-disc feature: a smooth bright oval near the bottom,
    // like light reflecting off the curved lower surface.
    let bot_c = center + Vec2::new(0.0, radius * 0.62);
    painter.add(egui::Shape::mesh(radial_fan(
        bot_c, radius * 0.50,
        white(100),  // centre: bright reflection
        white(0),    // edge:   fades to transparent
    )));

    // ── 5. Rim ────────────────────────────────────────────────────────────────
    let (border_w, border_c) = if selected {
        (2.0, Color32::from_rgba_premultiplied(
            (140.0 * am) as u8,
            (190.0 * am) as u8,
            (255.0 * am) as u8,
            (255.0 * am) as u8,
        ))
    } else {
        (1.5, white(150))
    };
    painter.circle_stroke(center, radius, Stroke::new(border_w, border_c));
}

pub(crate) fn draw_glass(
    painter:   &egui::Painter,
    rect:      egui::Rect,
    base:      Color32,   // control's own colour — used only as a faint frost tint
    corner:    f32,
    selected:  bool,
    alpha_mul: f32,
) {
    if alpha_mul <= 0.0 { return; }
    let am = alpha_mul.clamp(0.0, 1.0);

    // Helper: premultiplied white at `alpha` (0-255), scaled by `am`.
    let white = |alpha: u8| -> Color32 {
        let a = (alpha as f32 * am) as u8;
        Color32::from_rgba_premultiplied(a, a, a, a)
    };

    // Helper: premultiplied arbitrary straight-alpha colour.
    let pm = |r: u8, g: u8, b: u8, alpha: u8| -> Color32 {
        let a = (alpha as f32 * am) as u8;
        Color32::from_rgba_premultiplied(
            (r as f32 * am * alpha as f32 / 255.0) as u8,
            (g as f32 * am * alpha as f32 / 255.0) as u8,
            (b as f32 * am * alpha as f32 / 255.0) as u8,
            a,
        )
    };

    let (x0, x1) = (rect.min.x, rect.max.x);
    let (y0, y1) = (rect.min.y, rect.max.y);
    let w = (x1 - x0).max(1.0);
    let h = (y1 - y0).max(1.0);
    let radius = corner
        .max(0.0)
        .min(w * 0.5)
        .min(h * 0.5);

    // Build a rounded-rectangle mesh from horizontal strips.  This preserves a
    // true top-to-bottom gradient while following the exact rounded contour on
    // the left and right sides.  Unlike a centre-fan mesh, it does not create
    // side bands or corner warping inside the chart frame.
    let rounded_vertical_mesh = |area: egui::Rect,
                                 r: f32,
                                 rows: usize,
                                 color_at_t: &dyn Fn(f32) -> Color32|
        -> egui::epaint::Mesh
    {
        let uv = egui::pos2(0.0, 0.0);
        let mut m = egui::epaint::Mesh::default();
        let rr = r
            .max(0.0)
            .min(area.width() * 0.5)
            .min(area.height() * 0.5);

        let inset_at_y = |y: f32| -> f32 {
            if rr <= 0.0 { return 0.0; }
            let mut inset: f32 = 0.0;

            let top = (y - area.min.y).clamp(0.0, area.height());
            if top < rr {
                let dy = rr - top;
                inset = inset.max(rr - (rr * rr - dy * dy).max(0.0).sqrt());
            }

            let bottom = (area.max.y - y).clamp(0.0, area.height());
            if bottom < rr {
                let dy = rr - bottom;
                inset = inset.max(rr - (rr * rr - dy * dy).max(0.0).sqrt());
            }

            inset
        };

        let n = rows.max(32);
        for i in 0..=n {
            let t = i as f32 / n as f32;
            let y = area.min.y + area.height() * t;
            let inset = inset_at_y(y);
            let c = color_at_t(t);
            m.vertices.push(egui::epaint::Vertex {
                pos: Pos2::new(area.min.x + inset, y),
                uv,
                color: c,
            });
            m.vertices.push(egui::epaint::Vertex {
                pos: Pos2::new(area.max.x - inset, y),
                uv,
                color: c,
            });
        }

        for i in 0..n {
            let k = (i * 2) as u32;
            m.indices.extend([k, k + 1, k + 3, k, k + 3, k + 2]);
        }

        m
    };

    // ── 1. Layered shadow ────────────────────────────────────────────────────
    painter.rect_filled(
        rect.translate(Vec2::new(0.0, 8.0)).expand(1.0),
        radius + 4.0,
        pm(0, 0, 0, 18),
    );
    painter.rect_filled(
        rect.translate(Vec2::new(0.0, 16.0)).expand(4.0),
        radius + 10.0,
        pm(0, 0, 0, 8),
    );

    // ── 2. Continuous frosted field ───────────────────────────────────────────
    let glass_color = |t: f32| -> Color32 {
        let u = t.clamp(0.0, 1.0);
        let smooth = u * u * (3.0 - 2.0 * u);
        let alpha  = 30.0 + 82.0 * (1.0 - smooth).powf(1.18);
        let lip    = 10.0 * (1.0 - u).powf(5.2);

        let mix_base = 0.035;
        let r = 255.0 * (1.0 - mix_base) + base.r() as f32 * mix_base;
        let g = 255.0 * (1.0 - mix_base) + base.g() as f32 * mix_base;
        let b = 255.0 * (1.0 - mix_base) + base.b() as f32 * mix_base;

        let a = ((alpha + lip) * am).clamp(0.0, 255.0);
        Color32::from_rgba_premultiplied(
            (r * a / 255.0).clamp(0.0, 255.0) as u8,
            (g * a / 255.0).clamp(0.0, 255.0) as u8,
            (b * a / 255.0).clamp(0.0, 255.0) as u8,
            a as u8,
        )
    };
    painter.add(egui::Shape::mesh(rounded_vertical_mesh(rect, radius, 220, &glass_color)));

    // ── 3. Very gentle depth tint ─────────────────────────────────────────────
    let depth_color = |t: f32| -> Color32 {
        let u = t.clamp(0.0, 1.0);
        let smooth = u * u * (3.0 - 2.0 * u);
        let a = (1.0 + 13.0 * smooth.powf(1.5)).clamp(0.0, 18.0) as u8;
        pm(28, 44, 56, a)
    };
    painter.add(egui::Shape::mesh(rounded_vertical_mesh(rect, radius, 220, &depth_color)));

    // ── 4. Single rounded frame ───────────────────────────────────────────────
    let (border_w, border_c) = if selected {
        (2.0, Color32::from_rgba_premultiplied(
            (140.0 * am) as u8,
            (190.0 * am) as u8,
            (255.0 * am) as u8,
            (255.0 * am) as u8,
        ))
    } else {
        (1.15, white(164))
    };
    painter.rect_stroke(rect, radius, Stroke::new(border_w, border_c));
}

/// Scale `base` uniformly about its centre by `scale` (1.0 = unchanged).
/// Shared by the designer canvas, the preview window and the run form so that
/// zoom/spin/flip animations resize widgets identically everywhere.
pub(crate) fn scale_rect_about_center(base: egui::Rect, scale: f32) -> egui::Rect {
    if (scale - 1.0).abs() < 0.001 {
        base
    } else {
        egui::Rect::from_center_size(base.center(), base.size() * scale)
    }
}

// ── Non-visual widget rendering (standardised "liquid glass" icons) ─────────────
//
// All non-visual controls (Timer / AgentObject / RestClient / SqlDatabase) share
// one dark glass card + a consistent light, stroke-drawn ("hand-drawn") icon and
// a larger label, so they look uniform on the canvas.

/// Shared glass-card colour for every non-visual widget.
const NV_CARD: Color32 = Color32::from_rgb(40, 54, 84);

/// Light "glass" colour for the stroke icons + labels.
fn nv_icon_color(a: u8) -> Color32 { Color32::from_rgba_premultiplied(212, 226, 255, a) }

/// Draw the shared non-visual card background.
fn nv_card(painter: &egui::Painter, rect: egui::Rect, selected: bool, glass: bool, alpha_mul: f32, a: u8) {
    if glass {
        draw_glass(painter, rect, NV_CARD, 12.0, selected, alpha_mul);
    } else {
        let fill   = Color32::from_rgba_premultiplied(NV_CARD.r(), NV_CARD.g(), NV_CARD.b(), a);
        let border = if selected {
            Color32::from_rgba_premultiplied(90, 160, 255, a)
        } else {
            Color32::from_rgba_premultiplied(110, 130, 180, a)
        };
        painter.rect_filled(rect, 12.0, fill);
        painter.rect_stroke(rect, 12.0, Stroke::new(if selected { 2.0 } else { 1.0 }, border));
    }
}

/// Centre / size / stroke for a non-visual icon within `rect`.
fn nv_icon_geom(rect: egui::Rect, a: u8) -> (Pos2, f32, Stroke) {
    let cen = Pos2::new(rect.center().x, rect.min.y + rect.height() * 0.40);
    let s   = rect.height().min(rect.width()) * 0.22;
    let sw  = (s * 0.18).clamp(1.6, 3.0);
    (cen, s, Stroke::new(sw, nv_icon_color(a)))
}

/// A larger label centred at the bottom of the card (≈2× the previous size).
fn nv_label(painter: &egui::Painter, rect: egui::Rect, text: &str, a: u8) {
    let t: String = text.chars().take(14).collect();
    painter.text(
        rect.center_bottom() - Vec2::new(0.0, 7.0),
        egui::Align2::CENTER_BOTTOM,
        t,
        // 20% smaller than 16px, and 25% darker label colour.
        egui::FontId::proportional(12.8),
        Color32::from_rgba_premultiplied(154, 165, 186, a),
    );
}

fn nv_ellipse(painter: &egui::Painter, cx: f32, cy: f32, rw: f32, rh: f32, st: Stroke) {
    let steps = 28u32;
    let pts: Vec<Pos2> = (0..=steps).map(|i| {
        let t = i as f32 / steps as f32 * std::f32::consts::TAU;
        Pos2::new(cx + rw * t.cos(), cy + rh * t.sin())
    }).collect();
    painter.add(egui::Shape::closed_line(pts, st));
}

fn nv_icon_clock(painter: &egui::Painter, c: Pos2, s: f32, st: Stroke) {
    painter.circle_stroke(c, s, st);
    // top stem (stopwatch button)
    painter.line_segment([c + Vec2::new(0.0, -s), c + Vec2::new(0.0, -s - s * 0.30)], st);
    // hands
    painter.line_segment([c, c + Vec2::new(0.0, -s * 0.6)], st);
    painter.line_segment([c, c + Vec2::new(s * 0.45, s * 0.12)], st);
}

fn nv_icon_robot(painter: &egui::Painter, c: Pos2, s: f32, st: Stroke) {
    let head = egui::Rect::from_center_size(c + Vec2::new(0.0, s * 0.1), Vec2::new(s * 1.7, s * 1.5));
    painter.rect_stroke(head, s * 0.28, st);
    // antenna
    painter.line_segment([Pos2::new(c.x, head.min.y), Pos2::new(c.x, head.min.y - s * 0.4)], st);
    painter.circle_filled(Pos2::new(c.x, head.min.y - s * 0.45), st.width * 1.1, st.color);
    // eyes
    painter.circle_filled(c + Vec2::new(-s * 0.42, 0.0), st.width * 1.2, st.color);
    painter.circle_filled(c + Vec2::new(s * 0.42, 0.0), st.width * 1.2, st.color);
    // mouth
    painter.line_segment([c + Vec2::new(-s * 0.4, s * 0.5), c + Vec2::new(s * 0.4, s * 0.5)], st);
}

fn nv_icon_globe(painter: &egui::Painter, c: Pos2, s: f32, st: Stroke) {
    painter.circle_stroke(c, s, st);
    // equator + two latitude lines
    painter.line_segment([c + Vec2::new(-s, 0.0), c + Vec2::new(s, 0.0)], st);
    painter.line_segment([c + Vec2::new(-s * 0.86, -s * 0.5), c + Vec2::new(s * 0.86, -s * 0.5)], st);
    painter.line_segment([c + Vec2::new(-s * 0.86, s * 0.5), c + Vec2::new(s * 0.86, s * 0.5)], st);
    // central meridian
    nv_ellipse(painter, c.x, c.y, s * 0.45, s, st);
}

fn nv_icon_database(painter: &egui::Painter, c: Pos2, s: f32, st: Stroke) {
    let rw = s;
    let rh = s * 0.40;
    let top = c.y - s * 0.72;
    let bot = c.y + s * 0.72;
    // top + middle rings
    nv_ellipse(painter, c.x, top, rw, rh, st);
    nv_ellipse(painter, c.x, c.y, rw, rh, st);
    // sides
    painter.line_segment([Pos2::new(c.x - rw, top), Pos2::new(c.x - rw, bot)], st);
    painter.line_segment([Pos2::new(c.x + rw, top), Pos2::new(c.x + rw, bot)], st);
    // front-bottom curve
    let steps = 18u32;
    let front: Vec<Pos2> = (0..=steps).map(|i| {
        let t = i as f32 / steps as f32 * std::f32::consts::PI;
        Pos2::new(c.x + rw * t.cos(), bot + rh * t.sin())
    }).collect();
    painter.add(egui::Shape::line(front, st));
}

fn draw_control(
    painter:   &egui::Painter,
    origin:    Pos2,
    ctrl:      &Control,
    selected:  bool,
    glass:     bool,
    alpha_mul: f32,
    scale:     f32,                        // animation scale factor (1.0 = normal)
    pic_tex:   Option<egui::TextureId>,   // pre-loaded texture for PictureBox
) {
    use cobolt_forms::ControlType as CT;

    let r = ctrl.rect;
    // Compute the base rect, then apply scale around the control center.
    let base_rect = egui::Rect::from_min_size(
        origin + Vec2::new(r.x as f32, r.y as f32),
        Vec2::new(r.w as f32, r.h as f32),
    );
    let rect = scale_rect_about_center(base_rect, scale);

    let a = (alpha_mul.clamp(0.0, 1.0) * 255.0) as u8;
    let c_scale = |c: u8| -> u8 { ((c as f32) * alpha_mul) as u8 };
    let alpha_color = |c: Color32| Color32::from_rgba_premultiplied(c.r(), c.g(), c.b(), c_scale(c.a()));

    // ── Drop shadow ───────────────────────────────────────────────────────────
    let shadow_on = ctrl.get_prop("ShadowEnabled").map(|v| v.as_bool()).unwrap_or(false);
    if shadow_on && !matches!(ctrl.control_type, CT::Line | CT::Timer | CT::AgentObject | CT::RestClient | CT::SqlDatabase) {
        let shadow_color   = ctrl.get_prop("ShadowColor").map(|v| parse_color(v.as_str())).unwrap_or(Color32::BLACK);
        let shadow_opac    = ctrl.get_prop("ShadowOpacity").map(|v| v.as_i64()).unwrap_or(20).clamp(0, 100) as f32 / 100.0;
        let shadow_dir     = ctrl.get_prop("ShadowDirection").map(|v| v.as_str().to_owned()).unwrap_or_else(|| "South".into());
        let distance       = ctrl.get_prop("ShadowDistance").map(|v| v.as_i64()).unwrap_or(7).clamp(0, 60) as f32;
        let blur_enabled   = ctrl.get_prop("ShadowBlur").map(|v| v.as_bool()).unwrap_or(true);
        let blur_strength  = if blur_enabled {
            ctrl.get_prop("ShadowBlurStrength").map(|v| v.as_i64()).unwrap_or(8).clamp(0, 20) as usize
        } else { 0 };

        // Direction → unit vector (ux, uy)
        let (ux, uy): (f32, f32) = match shadow_dir.as_str() {
            "North"     => ( 0.0,   -1.0  ),
            "NorthEast" => ( 0.707, -0.707),
            "East"      => ( 1.0,    0.0  ),
            "SouthEast" => ( 0.707,  0.707),
            "South"     => ( 0.0,    1.0  ),
            "SouthWest" => (-0.707,  0.707),
            "West"      => (-1.0,    0.0  ),
            "NorthWest" => (-0.707, -0.707),
            _           => ( 0.0,    1.0  ),
        };
        let shadow_rect = rect.translate(Vec2::new(ux * distance, uy * distance));
        let corner_r    = ctrl.get_prop("CornerRadius").map(|v| v.as_i64() as f32).unwrap_or(3.0);
        let sc          = shadow_color;

        if blur_strength == 0 {
            // ── Hard shadow — single solid rect ───────────────────────────────
            let alpha = (shadow_opac * alpha_mul * 255.0) as u8;
            painter.rect_filled(
                shadow_rect,
                corner_r,
                Color32::from_rgba_premultiplied(
                    (sc.r() as f32 * shadow_opac * alpha_mul) as u8,
                    (sc.g() as f32 * shadow_opac * alpha_mul) as u8,
                    (sc.b() as f32 * shadow_opac * alpha_mul) as u8,
                    alpha,
                ),
            );
        } else {
            // ── Soft blur — concentric expanding rects with gaussian falloff ──
            // We draw `blur_strength + 1` layers from outermost (faintest) to
            // innermost (darkest), so the painter's back-to-front order gives the
            // right look: the core of the shadow is the most opaque.
            let layers = blur_strength;
            for i in 0..=layers {
                // i=0 → outer rim (t=1, faintest); i=layers → core (t=0, darkest)
                let t       = 1.0 - (i as f32 / layers as f32); // 1 → 0
                let expand  = t * blur_strength as f32;
                // Gaussian falloff: e^(-k·t²) where k controls how sharply the
                // shadow fades.  k=3 gives a natural soft shadow feel.
                let falloff = (-3.0 * t * t).exp();
                let alpha   = (shadow_opac * alpha_mul * falloff * 255.0) as u8;
                let layer_rect = shadow_rect.expand(expand);
                painter.rect_filled(
                    layer_rect,
                    corner_r + expand,
                    Color32::from_rgba_premultiplied(
                        (sc.r() as f32 * (alpha as f32 / 255.0)) as u8,
                        (sc.g() as f32 * (alpha as f32 / 255.0)) as u8,
                        (sc.b() as f32 * (alpha as f32 / 255.0)) as u8,
                        alpha,
                    ),
                );
            }
        }
    }

    // ── Line control ──────────────────────────────────────────────────────────
    if matches!(ctrl.control_type, CT::Line) {
        let line_color = ctrl.get_prop("LineColor").map(|v| parse_color(v.as_str())).unwrap_or(Color32::BLACK);
        let thickness  = ctrl.get_prop("LineThickness").map(|v| v.as_i64() as f32).unwrap_or(1.0);
        let dir        = ctrl.get_prop("LineDirection").map(|v| v.as_str().to_owned()).unwrap_or_else(|| "Horizontal".into());
        let (p1, p2)   = match dir.as_str() {
            "Vertical" => (rect.left_top(),  rect.left_bottom()),
            "Diagonal" => (rect.left_top(),  rect.right_bottom()),
            _          => (rect.left_center(), rect.right_center()),
        };
        painter.line_segment([p1, p2], Stroke::new(thickness, alpha_color(line_color)));
        if selected {
            painter.circle_stroke(p1, 4.0, Stroke::new(1.0, Color32::from_rgba_premultiplied(60,120,230, a)));
            painter.circle_stroke(p2, 4.0, Stroke::new(1.0, Color32::from_rgba_premultiplied(60,120,230, a)));
        }
        return;
    }

    // ── Shape control ─────────────────────────────────────────────────────────
    if matches!(ctrl.control_type, CT::Shape) {
        let fill_color = ctrl.get_prop("FillColor").map(|v| parse_color(v.as_str())).unwrap_or(Color32::from_rgb(192,192,192));
        let line_color = ctrl.get_prop("LineColor").map(|v| parse_color(v.as_str())).unwrap_or(Color32::BLACK);
        let thickness  = ctrl.get_prop("LineThickness").map(|v| v.as_i64() as f32).unwrap_or(1.0);
        let fill_style = ctrl.get_prop("FillStyle").map(|v| v.as_str().to_owned()).unwrap_or_else(|| "Solid".into());
        let shape_type = ctrl.get_prop("ShapeType").map(|v| v.as_str().to_owned()).unwrap_or_else(|| "Rectangle".into());

        let rr = match shape_type.as_str() {
            "Circle"    => rect.width().min(rect.height()) / 2.0,
            "Ellipse"   => rect.width().min(rect.height()) / 2.0, // backward compat
            "RoundRect" => 8.0,
            _           => 0.0,
        };

        let border_c = if selected {
            Color32::from_rgba_premultiplied(60, 120, 230, a)
        } else {
            alpha_color(line_color)
        };

        if shape_type == "Circle" || shape_type == "Ellipse" {
            // Circle / Ellipse — use circle primitives so the shape doesn't bleed.
            let circ_r = rect.width().min(rect.height()) / 2.0;
            let cc     = rect.center();
            if glass && fill_style != "None" {
                draw_glass_circle(painter, cc, circ_r, fill_color, selected, alpha_mul);
                if thickness > 0.0 {
                    painter.circle_stroke(cc, circ_r, Stroke::new(thickness, border_c));
                }
            } else {
                let fill = if fill_style == "None" { Color32::TRANSPARENT } else { alpha_color(fill_color) };
                painter.circle_filled(cc, circ_r, fill);
                painter.circle_stroke(cc, circ_r, Stroke::new(thickness, border_c));
            }
        } else if shape_type == "Triangle" {
            // Triangle — equilateral pointing up, filling the bounding rect.
            let top    = Pos2::new(rect.center().x, rect.min.y);
            let bot_l  = Pos2::new(rect.min.x, rect.max.y);
            let bot_r  = Pos2::new(rect.max.x, rect.max.y);
            let pts    = vec![top, bot_r, bot_l];
            let fill   = if fill_style == "None" { Color32::TRANSPARENT } else { alpha_color(fill_color) };
            painter.add(egui::Shape::convex_polygon(pts, fill, Stroke::new(thickness, border_c)));
        } else if glass && fill_style != "None" {
            // Rectangle / RoundRect — draw frosted glass using the user's FillColor as tint.
            draw_glass(painter, rect, fill_color, rr, selected, alpha_mul);
            if thickness > 0.0 {
                painter.rect_stroke(rect, rr, Stroke::new(thickness, border_c));
            }
        } else {
            let fill = if fill_style == "None" { Color32::TRANSPARENT } else { alpha_color(fill_color) };
            painter.rect_filled(rect, rr, fill);
            painter.rect_stroke(rect, rr, Stroke::new(thickness, border_c));
        }
        return;
    }

    // ── Non-visual widgets — standardised glass card + stroke icon + label ─────
    if matches!(ctrl.control_type, CT::Timer | CT::AgentObject | CT::RestClient | CT::SqlDatabase) {
        nv_card(painter, rect, selected, glass, alpha_mul, a);
        let (cen, s, st) = nv_icon_geom(rect, a);
        let label: String = match ctrl.control_type {
            CT::Timer => {
                nv_icon_clock(painter, cen, s, st);
                let iv = ctrl.get_prop("Interval").map(|v| v.as_i64()).unwrap_or(1000);
                format!("{iv}ms")
            }
            CT::AgentObject => {
                nv_icon_robot(painter, cen, s, st);
                ctrl.get_prop("AgentModel").map(|v| v.as_str().to_owned()).unwrap_or_else(|| "LLM".into())
            }
            CT::RestClient => {
                nv_icon_globe(painter, cen, s, st);
                ctrl.get_prop("DefaultMethod").map(|v| v.as_str().to_owned()).unwrap_or_else(|| "GET".into())
            }
            _ /* SqlDatabase */ => {
                nv_icon_database(painter, cen, s, st);
                ctrl.get_prop("Driver").map(|v| v.as_str().to_owned()).unwrap_or_else(|| "sqlite".into())
            }
        };
        nv_label(painter, rect, &label, a);
        return;
    }

    // ── Modal Window ──────────────────────────────────────────────────────────
    if matches!(ctrl.control_type, CT::ModalWindow) {
        let title = ctrl.get_prop("Title").map(|v| v.as_str().to_owned()).unwrap_or_else(|| "Dialog".into());
        if glass {
            draw_glass(painter, rect, Color32::from_rgb(40,40,80), 8.0, selected, alpha_mul);
        } else {
            let fill = Color32::from_rgba_premultiplied(40,40,80,a);
            let border = if selected { Color32::from_rgba_premultiplied(60,120,230,a) } else { Color32::from_rgba_premultiplied(100,100,160,a) };
            painter.rect_filled(rect, 4.0, fill);
            painter.rect_stroke(rect, 4.0, Stroke::new(if selected { 2.0 } else { 1.0 }, border));
        }
        // Draw mini title bar
        let tb_h = (rect.height() * 0.2).max(16.0);
        let tb_rect = egui::Rect::from_min_size(rect.min, Vec2::new(rect.width(), tb_h));
        painter.rect_filled(tb_rect, 4.0, Color32::from_rgba_premultiplied(80,80,160,a));
        painter.text(tb_rect.center(), egui::Align2::CENTER_CENTER,
            &title, egui::FontId::proportional(ctrl_font_size(ctrl).min(10.0)), Color32::from_rgba_premultiplied(220,220,255,a));
        // Mini window body outline
        let body_rect = egui::Rect::from_min_max(tb_rect.left_bottom(), rect.right_bottom());
        painter.rect_filled(body_rect, 0.0, Color32::from_rgba_premultiplied(60,60,100,a));
        painter.text(body_rect.center(), egui::Align2::CENTER_CENTER,
            "⊞ Modal", egui::FontId::proportional(9.0), Color32::from_rgba_premultiplied(160,160,200,a));
        return;
    }

    // ── Slider ────────────────────────────────────────────────────────────────
    if matches!(ctrl.control_type, CT::Slider) {
        let min_v   = ctrl.get_prop("Minimum").map(|v| v.as_i64()).unwrap_or(0) as f32;
        let max_v   = ctrl.get_prop("Maximum").map(|v| v.as_i64()).unwrap_or(100).max(1) as f32;
        let val     = ctrl.get_prop("Value").map(|v| v.as_i64()).unwrap_or(0) as f32;
        let step_v  = ctrl.get_prop("Step").map(|v| v.as_i64()).unwrap_or(10).max(1) as f32;
        let tick_fr = ctrl.get_prop("TickFrequency").map(|v| v.as_i64()).unwrap_or(10).max(1) as f32;
        let tick_st = ctrl.get_prop("TickStyle").map(|v| v.as_str().to_owned()).unwrap_or_else(|| "Bottom".into());
        let orient  = ctrl.get_prop("Orientation").map(|v| v.as_str().to_owned()).unwrap_or_else(|| "Horizontal".into());
        let vertical = orient.starts_with('V');

        let track_c  = alpha_color(ctrl.get_prop("TrackColor").map(|v| parse_color(v.as_str())).unwrap_or(Color32::from_rgb(170,170,170)));
        let thumb_c  = alpha_color(ctrl.get_prop("ThumbColor").map(|v| parse_color(v.as_str())).unwrap_or(Color32::from_rgb(0,120,215)));
        let fill_c   = alpha_color(ctrl.get_prop("FillColor").map(|v| parse_color(v.as_str())).unwrap_or(Color32::from_rgb(0,120,215)));
        let show_val = ctrl.get_prop("ShowValue").map(|v| v.as_bool()).unwrap_or(false);

        let _ = (track_c, thumb_c, fill_c); // glass design uses its own colors

        let pct = ((val - min_v) / (max_v - min_v)).clamp(0.0, 1.0);
        let range_units = max_v - min_v;

        // ── Helper: draw a frosted-glass pill ─────────────────────────────────
        // pill_rect: the full bounding rect of the pill
        // body_rgba: base frosted color (r,g,b,a) – already alpha-premultiplied
        // sheen: if true, add a top-half white gradient sheen
        let draw_glass_pill = |painter: &egui::Painter,
                               pill: egui::Rect,
                               body: Color32,
                               sheen: bool,
                               rim: Color32| {
            let r = pill.height() / 2.0;
            painter.rect_filled(pill, r, body);
            if sheen {
                // Top-half gradient mesh: opaque white → transparent
                let mut mesh = egui::epaint::Mesh::default();
                let top    = pill.min.y;
                let mid    = pill.min.y + pill.height() * 0.5;
                let left   = pill.min.x + r;
                let right  = pill.max.x - r;
                let w_hi   = Color32::from_rgba_premultiplied(120,130,150, (80.0 * alpha_mul) as u8);
                let w_lo   = Color32::from_rgba_premultiplied(0,0,0,0);
                // quad: 4 vertices
                let i = mesh.vertices.len() as u32;
                mesh.vertices.push(egui::epaint::Vertex { pos: Pos2::new(left,  top), uv: egui::epaint::WHITE_UV, color: w_hi });
                mesh.vertices.push(egui::epaint::Vertex { pos: Pos2::new(right, top), uv: egui::epaint::WHITE_UV, color: w_hi });
                mesh.vertices.push(egui::epaint::Vertex { pos: Pos2::new(right, mid), uv: egui::epaint::WHITE_UV, color: w_lo });
                mesh.vertices.push(egui::epaint::Vertex { pos: Pos2::new(left,  mid), uv: egui::epaint::WHITE_UV, color: w_lo });
                mesh.indices.extend_from_slice(&[i,i+1,i+2, i,i+2,i+3]);
                painter.add(egui::Shape::mesh(mesh));
            }
            painter.rect_stroke(pill, r, Stroke::new(1.0, rim));
        };

        // ── Helper: draw radial lens highlight at bottom-center of thumb ──────
        let draw_lens = |painter: &egui::Painter, center: Pos2, rx: f32, ry: f32| {
            let mut mesh = egui::epaint::Mesh::default();
            let center_c = Color32::from_rgba_premultiplied(
                (200.0 * alpha_mul) as u8,
                (215.0 * alpha_mul) as u8,
                (255.0 * alpha_mul) as u8,
                (160.0 * alpha_mul) as u8,
            );
            let edge_c = Color32::from_rgba_premultiplied(0,0,0,0);
            let ci = mesh.vertices.len() as u32;
            mesh.vertices.push(egui::epaint::Vertex { pos: center, uv: egui::epaint::WHITE_UV, color: center_c });
            let n = 32u32;
            for i in 0..n {
                let angle = (i as f32 / n as f32) * TAU;
                mesh.vertices.push(egui::epaint::Vertex {
                    pos: Pos2::new(center.x + rx * angle.cos(), center.y + ry * angle.sin()),
                    uv: egui::epaint::WHITE_UV,
                    color: edge_c,
                });
            }
            for i in 0..n {
                mesh.indices.extend_from_slice(&[ci, ci+1+i, ci+1+(i+1)%n]);
            }
            painter.add(egui::Shape::mesh(mesh));
        };

        // Glass track colors
        let track_body = Color32::from_rgba_premultiplied(
            (100.0 * alpha_mul) as u8, (110.0 * alpha_mul) as u8,
            (135.0 * alpha_mul) as u8, (90.0  * alpha_mul) as u8);
        let track_rim  = Color32::from_rgba_premultiplied(
            (180.0 * alpha_mul) as u8, (185.0 * alpha_mul) as u8,
            (210.0 * alpha_mul) as u8, (120.0 * alpha_mul) as u8);
        let thumb_body = Color32::from_rgba_premultiplied(
            (150.0 * alpha_mul) as u8, (160.0 * alpha_mul) as u8,
            (195.0 * alpha_mul) as u8, (140.0 * alpha_mul) as u8);
        let thumb_rim  = Color32::from_rgba_premultiplied(
            (220.0 * alpha_mul) as u8, (225.0 * alpha_mul) as u8,
            (245.0 * alpha_mul) as u8, (180.0 * alpha_mul) as u8);

        if vertical {
            // ── Vertical glass slider ────────────────────────────────────────
            let track_half_w = (rect.width() * 0.18).clamp(4.0, 12.0);
            let cx      = rect.center().x;
            let track_t = rect.min.y + 10.0;
            let track_b = rect.max.y - 10.0;
            let track_h = (track_b - track_t).max(1.0);
            let thumb_y = track_b - pct * track_h;
            let thumb_h = (track_half_w * 2.0 * 1.6).clamp(16.0, 32.0);
            let thumb_w = track_half_w * 2.0 + 6.0;

            // Track pill
            let track_rect = egui::Rect::from_min_max(
                Pos2::new(cx - track_half_w, track_t),
                Pos2::new(cx + track_half_w, track_b),
            );
            draw_glass_pill(painter, track_rect, track_body, true, track_rim);

            // Tick marks
            if tick_st != "None" && range_units > 0.0 {
                let mut tick_v = min_v;
                while tick_v <= max_v + 0.001 {
                    let ty = track_b - ((tick_v - min_v) / range_units).clamp(0.0, 1.0) * track_h;
                    let tick_color = Color32::from_rgba_premultiplied(140,145,165,(80.0*alpha_mul) as u8);
                    let tick_len = 5.0;
                    if tick_st == "Left" || tick_st == "Both" {
                        painter.line_segment([Pos2::new(cx - track_half_w - tick_len, ty), Pos2::new(cx - track_half_w - 1.0, ty)], Stroke::new(1.0, tick_color));
                    }
                    if tick_st != "Left" || tick_st == "Both" {
                        painter.line_segment([Pos2::new(cx + track_half_w + 1.0, ty), Pos2::new(cx + track_half_w + tick_len, ty)], Stroke::new(1.0, tick_color));
                    }
                    tick_v += tick_fr;
                }
            }

            // Thumb pill
            let thumb_rect = egui::Rect::from_center_size(
                Pos2::new(cx, thumb_y),
                Vec2::new(thumb_w, thumb_h),
            );
            draw_glass_pill(painter, thumb_rect, thumb_body, true, thumb_rim);
            // Lens at bottom-center of thumb
            draw_lens(painter,
                Pos2::new(cx, thumb_rect.max.y - thumb_h * 0.28),
                thumb_w * 0.32, thumb_h * 0.18);
        } else {
            // ── Horizontal glass slider ──────────────────────────────────────
            let track_half_h = (rect.height() * 0.18).clamp(4.0, 12.0);
            let cy      = rect.center().y;
            let track_l = rect.min.x + 10.0;
            let track_r = rect.max.x - 10.0;
            let track_w = (track_r - track_l).max(1.0);
            let thumb_x = track_l + pct * track_w;
            let thumb_w_half = (track_half_h * 1.6).clamp(8.0, 20.0);
            let thumb_h = track_half_h * 2.0 + 6.0;

            // Track pill
            let track_rect = egui::Rect::from_min_max(
                Pos2::new(track_l, cy - track_half_h),
                Pos2::new(track_r, cy + track_half_h),
            );
            draw_glass_pill(painter, track_rect, track_body, true, track_rim);

            // Tick marks
            if tick_st != "None" && range_units > 0.0 {
                let mut tick_v = min_v;
                while tick_v <= max_v + 0.001 {
                    let tx = track_l + ((tick_v - min_v) / range_units).clamp(0.0, 1.0) * track_w;
                    let tick_color = Color32::from_rgba_premultiplied(140,145,165,(80.0*alpha_mul) as u8);
                    let tick_len = 5.0;
                    if tick_st == "Top" || tick_st == "Both" {
                        painter.line_segment([Pos2::new(tx, cy - track_half_h - tick_len), Pos2::new(tx, cy - track_half_h - 1.0)], Stroke::new(1.0, tick_color));
                    }
                    if tick_st != "Top" || tick_st == "Both" {
                        painter.line_segment([Pos2::new(tx, cy + track_half_h + 1.0), Pos2::new(tx, cy + track_half_h + tick_len)], Stroke::new(1.0, tick_color));
                    }
                    tick_v += tick_fr;
                }
            }

            // Thumb pill
            let thumb_rect = egui::Rect::from_center_size(
                Pos2::new(thumb_x, cy),
                Vec2::new(thumb_w_half * 2.0, thumb_h),
            );
            draw_glass_pill(painter, thumb_rect, thumb_body, true, thumb_rim);
            // Lens at bottom-center of thumb
            draw_lens(painter,
                Pos2::new(thumb_x, thumb_rect.max.y - thumb_h * 0.28),
                thumb_w_half * 0.6, thumb_h * 0.18);
        }

        // Step label (min / max corners)
        let font_s = egui::FontId::proportional(9.0);
        let lbl_c  = Color32::from_rgba_premultiplied(80,80,80,a);
        if vertical {
            painter.text(Pos2::new(rect.center().x, rect.max.y - 2.0), egui::Align2::CENTER_BOTTOM,
                format!("{}", min_v as i64), font_s.clone(), lbl_c);
            painter.text(Pos2::new(rect.center().x, rect.min.y + 2.0), egui::Align2::CENTER_TOP,
                format!("{}", max_v as i64), font_s.clone(), lbl_c);
        } else {
            painter.text(Pos2::new(rect.min.x + 2.0, rect.max.y - 1.0), egui::Align2::LEFT_BOTTOM,
                format!("{}", min_v as i64), font_s.clone(), lbl_c);
            painter.text(Pos2::new(rect.max.x - 2.0, rect.max.y - 1.0), egui::Align2::RIGHT_BOTTOM,
                format!("{}", max_v as i64), font_s.clone(), lbl_c);
        }

        // Optional current value label
        if show_val {
            painter.text(rect.center(), egui::Align2::CENTER_CENTER,
                format!("{}", val as i64), egui::FontId::proportional(ctrl_font_size(ctrl)),
                Color32::from_rgba_premultiplied(0,0,0,a));
        }

        // Selection border
        if selected {
            painter.rect_stroke(rect, 3.0, Stroke::new(2.0, Color32::from_rgba_premultiplied(60,120,230,a)));
        }
        return;
    }

    // ── ProgressBar ───────────────────────────────────────────────────────────
    if matches!(ctrl.control_type, CT::ProgressBar) {
        let bg_c  = Color32::from_rgba_premultiplied(220,220,220,a);
        let bar_c = alpha_color(ctrl.get_prop("BarColor").map(|v| parse_color(v.as_str())).unwrap_or(Color32::from_rgb(0,170,0)));
        let val   = ctrl.get_prop("Value").map(|v| v.as_i64()).unwrap_or(0) as f32;
        let min   = ctrl.get_prop("Minimum").map(|v| v.as_i64()).unwrap_or(0) as f32;
        let max   = ctrl.get_prop("Maximum").map(|v| v.as_i64()).unwrap_or(100).max(1) as f32;
        let pct   = ((val - min) / (max - min)).clamp(0.0, 1.0);
        painter.rect_filled(rect, 2.0, bg_c);
        let bar = egui::Rect::from_min_size(rect.min, Vec2::new(rect.width() * pct, rect.height()));
        if glass { draw_glass(painter, bar, Color32::from_rgb(0,170,0), 2.0, false, alpha_mul * pct); }
        else     { painter.rect_filled(bar, 2.0, bar_c); }
        let border_c = if selected { Color32::from_rgba_premultiplied(60,120,230,a) } else { Color32::from_rgba_premultiplied(140,140,160,a) };
        painter.rect_stroke(rect, 2.0, Stroke::new(if selected { 2.0 } else { 1.0 }, border_c));
        if ctrl.get_prop("ShowValue").map(|v| v.as_bool()).unwrap_or(false) {
            painter.text(rect.center(), egui::Align2::CENTER_CENTER, format!("{:.0}%", pct*100.0),
                egui::FontId::proportional(ctrl_font_size(ctrl)), Color32::from_rgba_premultiplied(0,0,0,a));
        }
        return;
    }

    // ── Generic rect-based controls ───────────────────────────────────────────

    let (default_fill, default_border, default_text) = control_colors(&ctrl.control_type, selected);

    let fill = ctrl.get_prop("BackgroundColor").map(|v| parse_color(v.as_str())).unwrap_or(default_fill);
    let label_color = ctrl.get_prop("ForegroundColor").map(|v| parse_color(v.as_str())).unwrap_or(default_text);
    let stroke_color = ctrl.get_prop("BorderColor").map(|v| parse_color(v.as_str())).unwrap_or(default_border);

    let corner = match ctrl.control_type {
        CT::Button   => ctrl.get_prop("CornerRadius").map(|v| v.as_i64() as f32).unwrap_or(4.0),
        CT::GroupBox => 4.0,
        _            => 2.0,
    };

    let is_label = matches!(ctrl.control_type, CT::Label);

    // A PictureBox with ShowFrame = false draws no card/background/border —
    // only the image (so transparent PNG areas reveal what's behind).
    let pic_frameless = matches!(ctrl.control_type, CT::PictureBox)
        && !ctrl.get_prop("ShowFrame").map(|v| v.as_bool()).unwrap_or(true);

    if is_label || pic_frameless {
        // No visible frame. When selected, show a lightweight selection outline.
        if selected {
            let sel_c = Color32::from_rgba_premultiplied(60, 120, 230, a);
            painter.rect_stroke(rect, 0.0, Stroke::new(1.0, sel_c));
        }
    } else if glass {
        draw_glass(painter, rect, fill, corner, selected, alpha_mul);
    } else {
        painter.rect_filled(rect, corner, alpha_color(fill));
        let bc = if selected { Color32::from_rgba_premultiplied(60,120,230,a) } else { alpha_color(stroke_color) };
        painter.rect_stroke(rect, corner, Stroke::new(if selected { 2.0 } else { 1.0 }, bc));
    }

    // Label text — Caption is on Label, Button, CheckBox, RadioButton, GroupBox.
    let label: String = match ctrl.control_type {
        CT::CheckBox => {
            let checked = ctrl.get_prop("Checked").map(|v| v.as_bool()).unwrap_or(false);
            let cap = ctrl.get_prop("Caption").map(|v| v.as_str().to_owned()).unwrap_or_else(|| ctrl.id.clone());
            format!("{} {cap}", if checked { "[✓]" } else { "[ ]" })
        }
        CT::RadioButton => {
            let checked = ctrl.get_prop("Checked").map(|v| v.as_bool()).unwrap_or(false);
            let cap = ctrl.get_prop("Caption").map(|v| v.as_str().to_owned()).unwrap_or_else(|| ctrl.id.clone());
            format!("{} {cap}", if checked { "(●)" } else { "( )" })
        }
        CT::ComboBox => {
            let items = ctrl.get_prop("Items").map(|v| v.as_str().to_owned()).unwrap_or_default();
            format!("{} ▾", items.lines().next().unwrap_or(""))
        }
        CT::DateTimePicker => {
            let val = ctrl.get_prop("Value").map(|v| v.as_str().to_owned()).filter(|s| !s.is_empty()).unwrap_or_else(|| "DD/MM/YYYY".into());
            format!("📅 {val}")
        }
        CT::NumericUpDown => {
            let v = ctrl.get_prop("Value").map(|v| v.as_i64()).unwrap_or(0);
            format!("{v} ▲▼")
        }
        CT::PictureBox => {
            // If we have a loaded texture, draw it directly and skip the text label.
            if let Some(tex_id) = pic_tex {
                let size_mode = ctrl.get_prop("SizeMode").map(|v| v.as_str().to_owned()).unwrap_or_else(|| "Normal".into());
                let uv = egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0));
                let tint = Color32::from_rgba_premultiplied(255, 255, 255, a);
                let img_rect = match size_mode.as_str() {
                    "StretchImage" | "Zoom" | "AutoSize" => rect, // stretch to fill
                    _ => rect, // Normal/Tile: for designer preview just stretch too
                };
                painter.image(tex_id, img_rect, uv, tint);
                // Selection border on top
                if selected {
                    painter.rect_stroke(rect, 0.0, Stroke::new(2.0, Color32::from_rgba_premultiplied(60,120,230,a)));
                }
                return; // skip generic text rendering below
            }
            // No image loaded — show placeholder text
            if ctrl.get_prop("ImagePath").map(|v| !v.as_str().is_empty()).unwrap_or(false) {
                "🖼 [loading…]".into()
            } else {
                "🖼 (empty)".into()
            }
        }
        CT::Animator => {
            let source = ctrl.get_prop("Source").map(|v| v.as_str().to_owned()).unwrap_or_default();
            let auto    = ctrl.get_prop("AutoPlay").map(|v| v.as_bool()).unwrap_or(true);
            let looping = ctrl.get_prop("Loop").map(|v| v.as_bool()).unwrap_or(true);
            let size_mode = ctrl.get_prop("SizeMode").map(|v| v.as_str().to_owned())
                .unwrap_or_else(|| "Fit".into());
            let key = format!("{}|{}", ctrl.id, source.trim());
            draw_animator(painter, rect, &key, source.trim(), auto, looping, &size_mode, alpha_mul, selected);
            return;
        }
        CT::TreeView   => "🌲 [TreeView]".into(),
        CT::DataGrid   => {
            let cols = ctrl.get_prop("Columns").map(|v| v.as_str().to_owned()).unwrap_or_default();
            let col_count = cols.lines().count().max(1);
            format!("⊞ DataGrid ({col_count} cols)")
        }
        CT::Splitter   => {
            let dir = ctrl.get_prop("Orientation").map(|v| v.as_str().to_owned()).unwrap_or_else(|| "H".into());
            if dir.starts_with('V') { "║ Splitter".into() } else { "═ Splitter".into() }
        }
        CT::TabControl => {
            let tabs = ctrl.get_prop("Tabs").map(|v| v.as_str().to_owned()).unwrap_or_default();
            let first = tabs.lines().next().unwrap_or("Tab1");
            format!("[{first}] [...]")
        }
        CT::MenuBar    => "☰ MenuBar".into(),
        CT::ToolBar    => "⬛ ToolBar".into(),
        CT::StatusBar  => "▬ StatusBar".into(),
        // Controls with an intrinsic text label use their Caption property.
        CT::Label | CT::Button | CT::GroupBox =>
            ctrl.get_prop("Caption").map(|v| v.to_string()).unwrap_or_else(|| ctrl.id.clone()),
        // TextBox shows its current text value.
        CT::TextBox => ctrl.get_prop("Text").map(|v| v.to_string()).unwrap_or_default(),
        // Everything else: show the control ID.
        _ => ctrl.id.clone(),
    };

    if !label.is_empty() {
        let txt_color = Color32::from_rgba_premultiplied(
            label_color.r(), label_color.g(), label_color.b(), a,
        );
        let fsize = ctrl_font_size(ctrl);
        let font_name = ctrl.get_prop("FontName").map(|v| v.as_str()).unwrap_or_default();

        // For Label controls, apply font-style properties via LayoutJob.
        if matches!(ctrl.control_type, CT::Label) {
            use egui::text::{LayoutJob, TextFormat};

            let bold        = ctrl.get_prop("Bold").map(|v| v.as_bool()).unwrap_or(false);
            let italic      = ctrl.get_prop("Italic").map(|v| v.as_bool()).unwrap_or(false);
            let underline   = ctrl.get_prop("Underline").map(|v| v.as_bool()).unwrap_or(false);
            let strikeout   = ctrl.get_prop("Strikethrough").map(|v| v.as_bool()).unwrap_or(false);

            // Egui doesn't have a separate bold typeface registered by default.
            // Simulate bold by painting the galley twice with a tiny x-offset.
            let font_id = crate::fonts::font_id(painter.ctx(), &font_name, fsize);
            let mut job = LayoutJob::default();
            job.halign = egui::Align::Center;
            job.wrap.max_width = rect.width();
            job.wrap.break_anywhere = false;
            job.append(&label, 0.0, TextFormat {
                font_id: font_id.clone(),
                color: txt_color,
                italics: italic,
                underline: if underline {
                    Stroke::new(1.0, txt_color)
                } else {
                    Stroke::NONE
                },
                strikethrough: if strikeout {
                    Stroke::new(1.0, txt_color)
                } else {
                    Stroke::NONE
                },
                ..Default::default()
            });

            let galley = painter.layout_job(job);
            // halign=Center means the draw origin is the top-centre of the galley block.
            // So anchor x at rect.centre; y centres the wrapped block vertically.
            let text_pos = egui::pos2(
                rect.center().x,
                rect.center().y - galley.size().y / 2.0,
            );
            painter.galley(text_pos, galley.clone(), txt_color);

            // Simulate bold: repaint shifted by 0.5 px
            if bold {
                painter.galley(text_pos + Vec2::new(0.5, 0.0), galley, txt_color);
            }
        } else {
            painter.text(
                rect.center(), egui::Align2::CENTER_CENTER, &label,
                crate::fonts::font_id(painter.ctx(), &font_name, fsize), txt_color,
            );
        }
    }

    // ── Charts ───────────────────────────────────────────────────────────────
    if matches!(ctrl.control_type,
        CT::BarChart | CT::LineChart | CT::PieChart |
        CT::AreaChart | CT::ScatterChart | CT::DonutChart)
    {
        draw_chart_preview(painter, ctrl, rect, a, alpha_mul, glass, selected);
        if selected {
            painter.rect_stroke(rect, 8.0, Stroke::new(2.0, Color32::from_rgba_premultiplied(60,120,230,a)));
        }
        // Animation indicator falls through to the shared badge below.
    }

    // Animation indicator badge
    if !ctrl.animations.is_empty() {
        let badge_pos = rect.right_top() + Vec2::new(-2.0, 2.0);
        painter.circle_filled(badge_pos, 5.0, Color32::from_rgba_premultiplied(255,180,0,180));
        painter.text(badge_pos, egui::Align2::CENTER_CENTER,
            "▶", egui::FontId::proportional(6.0), Color32::WHITE);
    }
}

fn draw_handles(painter: &egui::Painter, origin: Pos2, r: &cobolt_forms::model::Rect, glass: bool) {
    for &h in &ALL_HANDLES {
        let hp = handle_pos(r, h);
        let screen = origin + Vec2::new(hp.x, hp.y);
        if glass {
            painter.circle_filled(screen, 5.0, Color32::from_rgba_premultiplied(30,60,160,200));
            painter.circle_filled(screen, 4.0, Color32::from_rgba_premultiplied(255,255,255,220));
            painter.circle_stroke(screen, 5.0, Stroke::new(1.0, Color32::from_rgba_premultiplied(100,160,255,200)));
        } else {
            painter.circle_filled(screen, 4.5, Color32::WHITE);
            painter.circle_stroke(screen, 4.5, Stroke::new(1.5, Color32::from_rgb(60, 120, 230)));
        }
    }
}

/// Draw the form-canvas resize grips along the right edge, bottom edge and the
/// bottom-right corner. The grip matching `active` (being hovered/dragged) is
/// highlighted so the user sees what they're about to resize.
fn draw_form_resize_grips(
    painter: &egui::Painter,
    canvas: egui::Rect,
    active: Option<FormEdge>,
    glass: bool,
) {
    let base = if glass {
        Color32::from_rgba_premultiplied(120, 160, 255, 130)
    } else {
        Color32::from_rgb(120, 150, 210)
    };
    let hot = Color32::from_rgb(80, 150, 255);

    let col = |e: FormEdge| if active == Some(e) { hot } else { base };

    // Right edge — a short vertical bar centred on the right border.
    let rx = canvas.right();
    let rcy = canvas.center().y;
    painter.line_segment(
        [Pos2::new(rx, rcy - 14.0), Pos2::new(rx, rcy + 14.0)],
        Stroke::new(if active == Some(FormEdge::Right) { 4.0 } else { 3.0 }, col(FormEdge::Right)),
    );

    // Bottom edge — a short horizontal bar centred on the bottom border.
    let by = canvas.bottom();
    let bcx = canvas.center().x;
    painter.line_segment(
        [Pos2::new(bcx - 14.0, by), Pos2::new(bcx + 14.0, by)],
        Stroke::new(if active == Some(FormEdge::Bottom) { 4.0 } else { 3.0 }, col(FormEdge::Bottom)),
    );

    // Corner — a small filled square at the bottom-right.
    let corner = canvas.max;
    let sz = 7.0;
    let crect = egui::Rect::from_min_max(Pos2::new(corner.x - sz, corner.y - sz), corner);
    painter.rect_filled(crect, 1.5, col(FormEdge::Corner));
    painter.rect_stroke(crect, 1.5, Stroke::new(1.0, Color32::from_rgba_premultiplied(255,255,255,180)));
}

/// Compute the destination rect for an image of `native` size inside `rect`,
/// according to a PictureBox/Animator-style `size_mode`.
pub(crate) fn media_dest_rect(rect: egui::Rect, native: Vec2, size_mode: &str) -> egui::Rect {
    if native.x <= 0.0 || native.y <= 0.0 {
        return rect;
    }
    match size_mode {
        "Stretch" => rect,
        "Fill" => {
            // Cover: scale up so the rect is fully covered (may overflow → clipped).
            let s = (rect.width() / native.x).max(rect.height() / native.y);
            egui::Rect::from_center_size(rect.center(), native * s)
        }
        "Center" | "Normal" => {
            // Native size centred, but never larger than the rect.
            let s = (rect.width() / native.x).min(rect.height() / native.y).min(1.0);
            egui::Rect::from_center_size(rect.center(), native * s)
        }
        // "Fit" (default): contain, preserving aspect ratio.
        _ => {
            let s = (rect.width() / native.x).min(rect.height() / native.y);
            egui::Rect::from_center_size(rect.center(), native * s)
        }
    }
}

/// Render an Animator control: plays its animated/still image (GIF/WebP/APNG/…)
/// at the current moment, or a placeholder when no source is set / decode fails.
#[allow(clippy::too_many_arguments)]
pub(crate) fn draw_animator(
    painter:   &egui::Painter,
    rect:      egui::Rect,
    key:       &str,
    source:    &str,
    auto_play: bool,
    looping:   bool,
    size_mode: &str,
    alpha_mul: f32,
    selected:  bool,
) {
    let a = (alpha_mul.clamp(0.0, 1.0) * 255.0) as u8;

    let played = if source.is_empty() {
        None
    } else {
        let path = source.to_owned();
        cobolt_media::play(
            painter.ctx(),
            key,
            move || std::fs::read(&path).ok(),
            auto_play,
            looping,
        )
    };

    match played {
        Some((tex, native)) => {
            let dest = media_dest_rect(rect, native, size_mode);
            let clip = painter.with_clip_rect(rect);
            clip.image(
                tex,
                dest,
                egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                Color32::from_white_alpha(a),
            );
        }
        None => {
            // Placeholder: a dark "film" panel with a play glyph.
            painter.rect_filled(rect, 6.0, Color32::from_rgba_premultiplied(18, 24, 48, a));
            painter.rect_stroke(rect, 6.0,
                Stroke::new(1.0, Color32::from_rgba_premultiplied(120, 150, 230, a)));
            let label = if source.is_empty() { "▶ Animator" } else { "▶ (cannot load)" };
            painter.text(rect.center(), egui::Align2::CENTER_CENTER, label,
                egui::FontId::proportional(13.0),
                Color32::from_rgba_premultiplied(190, 205, 255, a));
        }
    }

    if selected {
        painter.rect_stroke(rect, 6.0,
            Stroke::new(2.0, Color32::from_rgba_premultiplied(60, 120, 230, a)));
    }
}

/// Draw a rich glass chart preview on the canvas for all chart control types.
pub(crate) fn draw_chart_preview(
    painter:   &egui::Painter,
    ctrl:      &Control,
    rect:      egui::Rect,
    a:         u8,
    alpha_mul: f32,
    glass:     bool,
    selected:  bool,
) {
    use cobolt_forms::model::ControlType as CT;

    let _ = selected; // selection border drawn by caller

    // ── Background ────────────────────────────────────────────────────────────
    let bg = Color32::from_rgba_premultiplied(15,20,45,a);
    if glass {
        draw_glass(painter, rect, Color32::from_rgb(15,20,45), 8.0, false, alpha_mul);
    } else {
        painter.rect_filled(rect, 8.0, bg);
        let border = Color32::from_rgba_premultiplied(60,80,160,a);
        painter.rect_stroke(rect, 8.0, Stroke::new(1.0, border));
    }

    // All chart content is drawn through a clipped painter so nothing bleeds
    // outside the rounded-corner frame.  We inset by 1 px so the border stroke
    // itself is never covered.
    let painter = &painter.with_clip_rect(rect.shrink(1.0));

    // Palette — 4 accent colours
    let pal_raw: &[(u8,u8,u8)] = &[(76,155,232),(232,122,76),(76,232,122),(232,76,155)];
    let pal: Vec<Color32> = pal_raw.iter()
        .map(|&(r,g,b)| Color32::from_rgb(r, g, b))
        .collect();

    // Inner plot area (leave margin for axes / labels)
    let margin_l = rect.width()  * 0.10;
    let margin_b = rect.height() * 0.12;
    let margin_t = rect.height() * 0.12;
    let margin_r = rect.width()  * 0.04;
    let plot = egui::Rect::from_min_max(
        Pos2::new(rect.min.x + margin_l, rect.min.y + margin_t),
        Pos2::new(rect.max.x - margin_r, rect.max.y - margin_b),
    );

    // title
    let title = ctrl.get_prop("Title").map(|v| v.as_str().to_owned()).unwrap_or_default();
    if !title.is_empty() {
        painter.text(
            Pos2::new(rect.center().x, rect.min.y + margin_t * 0.5),
            egui::Align2::CENTER_CENTER, &title,
            egui::FontId::proportional(10.0),
            Color32::from_rgb(235, 240, 255));
    }

    // ── Grid lines ────────────────────────────────────────────────────────────
    let show_grid = ctrl.get_prop("ShowGridLines").map(|v| v.as_bool()).unwrap_or(true);
    if show_grid {
        let grid_c = Color32::from_rgb(118, 142, 225);
        let n_h = 4u32;
        for i in 1..n_h {
            let y = plot.min.y + plot.height() * i as f32 / n_h as f32;
            painter.line_segment([Pos2::new(plot.min.x, y), Pos2::new(plot.max.x, y)],
                Stroke::new(1.15, grid_c));
        }
        if !matches!(ctrl.control_type, CT::PieChart | CT::DonutChart) {
            let n_v = 5u32;
            for i in 1..n_v {
                let x = plot.min.x + plot.width() * i as f32 / n_v as f32;
                painter.line_segment([Pos2::new(x, plot.min.y), Pos2::new(x, plot.max.y)],
                    Stroke::new(1.15, grid_c));
            }
        }
    }

    // Axes
    let ax_c = Color32::from_rgb(84, 104, 190);
    if !matches!(ctrl.control_type, CT::PieChart | CT::DonutChart) {
        painter.line_segment([plot.left_bottom(), plot.right_bottom()], Stroke::new(1.45, ax_c));
        painter.line_segment([plot.left_bottom(), plot.left_top()],     Stroke::new(1.45, ax_c));
    }

    // ── Sample data (representative preview) ──────────────────────────────────
    // Normalised Y values for 5 data points, 2 series
    let series1: &[f32] = &[0.40, 0.70, 0.55, 0.85, 0.60];
    let series2: &[f32] = &[0.25, 0.45, 0.70, 0.50, 0.80];
    let n = series1.len();

    let px_x = |i: usize| plot.min.x + (i as f32 + 0.5) / n as f32 * plot.width();
    let px_y = |v: f32|   plot.max.y - v * plot.height();

    match ctrl.control_type {
        CT::BarChart => {
            let horizontal = ctrl.get_prop("Horizontal").map(|v| v.as_bool()).unwrap_or(false);
            let bar_total  = plot.width() / n as f32;
            let bar_w      = bar_total * 0.38;
            let gap        = bar_total * 0.05;
            for (si, series) in [series1, series2].iter().enumerate() {
                for (i, &v) in series.iter().enumerate() {
                    let c = &pal[si % pal.len()];
                    if horizontal {
                        let y  = plot.min.y + (i as f32 + 0.5 + si as f32 * (0.5 + gap)) / n as f32 * plot.height() - bar_w * 0.5;
                        let w  = v * plot.width();
                        let br = egui::Rect::from_min_size(Pos2::new(plot.min.x, y), Vec2::new(w, bar_w));
                        painter.rect_filled(br, 2.0, *c);
                    } else {
                        let x  = plot.min.x + (i as f32 * bar_total) + si as f32 * (bar_w + gap) + gap;
                        let h  = v * plot.height();
                        let br = egui::Rect::from_min_size(Pos2::new(x, plot.max.y - h), Vec2::new(bar_w, h));
                        painter.rect_filled(br, 2.0, *c);
                    }
                }
            }
        }
        CT::LineChart => {
            for (si, series) in [series1, series2].iter().enumerate() {
                let pts: Vec<Pos2> = series.iter().enumerate()
                    .map(|(i, &v)| Pos2::new(px_x(i), px_y(v)))
                    .collect();
                let c = pal[si % pal.len()];
                for w in pts.windows(2) {
                    painter.line_segment([w[0], w[1]], Stroke::new(1.8, c));
                }
                for &p in &pts {
                    painter.circle_filled(p, 3.0, c);
                }
            }
        }
        CT::AreaChart => {
            for (si, series) in [series1, series2].iter().enumerate() {
                let pts: Vec<Pos2> = series.iter().enumerate()
                    .map(|(i, &v)| Pos2::new(px_x(i), px_y(v)))
                    .collect();
                let c = pal[si % pal.len()];
                let fill = Color32::from_rgba_unmultiplied(c.r(), c.g(), c.b(), 80);
                // Filled polygon
                let mut poly: Vec<Pos2> = pts.clone();
                poly.push(Pos2::new(pts.last().unwrap().x, plot.max.y));
                poly.push(Pos2::new(pts[0].x, plot.max.y));
                painter.add(egui::Shape::convex_polygon(poly, fill, Stroke::NONE));
                // Line
                for w in pts.windows(2) {
                    painter.line_segment([w[0], w[1]], Stroke::new(1.8, c));
                }
            }
        }
        CT::ScatterChart => {
            let pts1: &[(f32,f32)] = &[(0.15,0.65),(0.35,0.40),(0.50,0.78),(0.70,0.30),(0.88,0.55)];
            let pts2: &[(f32,f32)] = &[(0.20,0.30),(0.42,0.72),(0.60,0.45),(0.78,0.85)];
            for (pts, ci) in [(pts1, 0usize), (pts2, 1)] {
                let c = pal[ci];
                for &(fx, fy) in pts {
                    let p = Pos2::new(plot.min.x + fx*plot.width(), plot.max.y - fy*plot.height());
                    painter.circle_stroke(p, 4.5, Stroke::new(1.5, c));
                }
            }
        }
        CT::PieChart | CT::DonutChart => {
            let center  = plot.center();
            let outer_r = plot.size().min_elem() * 0.44;
            let inner_r = if ctrl.control_type == CT::DonutChart {
                let pct = ctrl.get_prop("InnerRadius").map(|v| v.as_i64()).unwrap_or(40) as f32 / 100.0;
                outer_r * pct
            } else { 0.0 };

            let slices: &[f32] = &[0.30, 0.20, 0.25, 0.25]; // proportions
            let mut start = -std::f32::consts::FRAC_PI_2; // top
            for (i, &frac) in slices.iter().enumerate() {
                let sweep = frac * TAU;
                let end   = start + sweep;
                let c     = pal[i % pal.len()];
                let fill  = Color32::from_rgba_premultiplied(c.r(), c.g(), c.b(), (a as f32 * 0.85) as u8);
                // Arc approximated with a fan polygon
                let steps = ((sweep * outer_r).max(4.0) as u32).min(40).max(4);
                let mut pts: Vec<Pos2> = Vec::with_capacity(steps as usize + 2);
                if inner_r > 0.0 {
                    // Donut: outer arc then inner arc reversed
                    for s in 0..=steps {
                        let t = start + sweep * s as f32 / steps as f32;
                        pts.push(Pos2::new(center.x + t.cos()*outer_r, center.y + t.sin()*outer_r));
                    }
                    for s in (0..=steps).rev() {
                        let t = start + sweep * s as f32 / steps as f32;
                        pts.push(Pos2::new(center.x + t.cos()*inner_r, center.y + t.sin()*inner_r));
                    }
                } else {
                    // Solid pie
                    pts.push(center);
                    for s in 0..=steps {
                        let t = start + sweep * s as f32 / steps as f32;
                        pts.push(Pos2::new(center.x + t.cos()*outer_r, center.y + t.sin()*outer_r));
                    }
                }
                painter.add(egui::Shape::convex_polygon(pts, fill, Stroke::new(0.8, bg)));
                start = end;
            }
        }
        _ => {}
    }

    // data source hint
    let ds = ctrl.get_prop("DataSource").map(|v| v.as_str().to_owned()).unwrap_or_default();
    if !ds.is_empty() {
        painter.text(
            Pos2::new(rect.center().x, rect.max.y - margin_b * 0.4),
            egui::Align2::CENTER_CENTER,
            format!("⬡ {ds}"),
            egui::FontId::proportional(8.5),
            Color32::from_rgba_premultiplied(130,160,220,a));
    }

    // type badge
    let badge = match ctrl.control_type {
        CT::BarChart     => "BAR",
        CT::LineChart    => "LINE",
        CT::PieChart     => "PIE",
        CT::AreaChart    => "AREA",
        CT::ScatterChart => "SCATTER",
        CT::DonutChart   => "DONUT",
        _                => "",
    };
    if !badge.is_empty() {
        painter.text(
            Pos2::new(rect.max.x - margin_r - 2.0, rect.min.y + margin_t * 0.45),
            egui::Align2::RIGHT_CENTER,
            badge,
            egui::FontId::proportional(8.0),
            Color32::from_rgba_premultiplied(80,100,180,a));
    }
}

fn control_colors(ct: &ControlType, selected: bool) -> (Color32, Color32, Color32) {
    let border = if selected { Color32::from_rgb(60,120,230) } else { Color32::from_rgb(140,140,160) };
    match ct {
        ControlType::Button         => (Color32::from_rgb(220,220,235), border, Color32::BLACK),
        ControlType::Label          => (Color32::TRANSPARENT, border, Color32::from_rgb(40,40,40)),
        ControlType::TextBox        => (Color32::WHITE, border, Color32::DARK_GRAY),
        ControlType::CheckBox | ControlType::RadioButton => (Color32::TRANSPARENT, border, Color32::BLACK),
        ControlType::GroupBox | ControlType::Panel => (Color32::from_rgba_premultiplied(200,200,210,40), border, Color32::DARK_GRAY),
        ControlType::PictureBox     => (Color32::from_rgb(180,200,220), border, Color32::DARK_GRAY),
        ControlType::DataGrid | ControlType::ListBox => (Color32::WHITE, border, Color32::DARK_GRAY),
        ControlType::MenuBar | ControlType::ToolBar | ControlType::StatusBar => (Color32::from_rgb(200,200,215), border, Color32::BLACK),
        ControlType::DateTimePicker | ControlType::NumericUpDown => (Color32::WHITE, border, Color32::DARK_GRAY),
        ControlType::TreeView       => (Color32::WHITE, border, Color32::DARK_GRAY),
        ControlType::Splitter       => (Color32::from_rgb(180,180,190), border, Color32::DARK_GRAY),
        ControlType::ComboBox       => (Color32::WHITE, border, Color32::DARK_GRAY),
        ControlType::TabControl     => (Color32::from_rgba_premultiplied(210,215,230,120), border, Color32::BLACK),
        _                           => (Color32::from_rgb(210,210,225), border, Color32::BLACK),
    }
}

fn ctrl_font_size(ctrl: &Control) -> f32 {
    ctrl.get_prop("FontSize").map(|v| v.as_i64() as f32).unwrap_or(11.0).clamp(4.0, 200.0)
}

fn parse_color(s: &str) -> Color32 {
    let s = s.trim_start_matches('#');
    // 8-char RRGGBBAA — straight alpha
    if s.len() == 8 {
        if let (Ok(r), Ok(g), Ok(b), Ok(a)) = (
            u8::from_str_radix(&s[0..2], 16),
            u8::from_str_radix(&s[2..4], 16),
            u8::from_str_radix(&s[4..6], 16),
            u8::from_str_radix(&s[6..8], 16),
        ) { return Color32::from_rgba_unmultiplied(r, g, b, a); }
    }
    // 6-char RRGGBB — fully opaque
    if s.len() == 6 {
        if let (Ok(r), Ok(g), Ok(b)) = (
            u8::from_str_radix(&s[0..2], 16),
            u8::from_str_radix(&s[2..4], 16),
            u8::from_str_radix(&s[4..6], 16),
        ) { return Color32::from_rgb(r, g, b); }
    }
    Color32::TRANSPARENT
}

fn apply_structural_prop(ctrl: &mut Control, key: &str, value: &PropValue) {
    match key {
        "Visible"  => ctrl.visible   = value.as_bool(),
        "Enabled"  => ctrl.enabled   = value.as_bool(),
        "TabOrder" => ctrl.tab_order = value.as_i64() as u32,
        "ZOrder"   => ctrl.z_order   = value.as_i64() as i32,
        _          => { ctrl.properties.insert(key.to_owned(), value.clone()); }
    }
}

// ── Target device presets ─────────────────────────────────────────────────────

/// All available target device presets: (label, width, height).
///
/// Dimensions are logical/point pixels at 1× scale (portrait by default).
pub(crate) const TARGET_PRESETS: &[(&str, u32, u32)] = &[
    // ── Custom ───────────────────────────────────────────────────────────────
    ("Custom",                         640,  480),

    // ── Apple iPhone ─────────────────────────────────────────────────────────
    ("iPhone 16 Pro Max",              440,  956),
    ("iPhone 16 / 15 Pro",             393,  852),
    ("iPhone 15 / 14",                 390,  844),
    ("iPhone SE (3rd gen)",            375,  667),

    // ── Apple iPad ───────────────────────────────────────────────────────────
    ("iPad Pro 13\" (M4)",            1032, 1376),
    ("iPad Pro 11\" (M4)",             834, 1210),
    ("iPad Air 13\" (M2)",            1024, 1366),
    ("iPad (10th gen)",                820, 1180),
    ("iPad mini (7th gen)",            744, 1133),

    // ── Apple Watch ──────────────────────────────────────────────────────────
    ("Apple Watch Ultra 2 (49mm)",     205,  251),
    ("Apple Watch Series 10 (46mm)",   198,  242),
    ("Apple Watch Series 10 (42mm)",   176,  215),

    // ── Android Phone ────────────────────────────────────────────────────────
    ("Samsung Galaxy S24 Ultra",       384,  824),
    ("Samsung Galaxy S24",             360,  780),
    ("Google Pixel 9 Pro",             412,  892),
    ("Android Phone (generic 1080p)",  393,  851),

    // ── Android Tablet ───────────────────────────────────────────────────────
    ("Samsung Galaxy Tab S9 Ultra",   1280,  800),
    ("Samsung Galaxy Tab S9",          800, 1280),
    ("Lenovo Tab P12",                1280,  800),
    ("Android Tablet (generic)",       800, 1280),

    // ── Android SmartWatch ───────────────────────────────────────────────────
    ("Samsung Galaxy Watch 7 (44mm)",  456,  456),
    ("Samsung Galaxy Watch 7 (40mm)",  432,  432),
    ("Wear OS (generic round)",        384,  384),
    ("Wear OS (generic square)",       320,  320),
];

// ── Unified Form-Designer Icon Toolbar ───────────────────────────────────────

/// All actions the unified 50-px icon toolbar can emit to the caller in app.rs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum DesignerToolbarAction {
    None,
    // History
    Undo, Redo,
    // File
    SaveAndGenerate, GenerateOnly,
    // View
    TogglePreview, ToggleAnimPreview, ToggleGrid, ToggleGlass,
    // Run
    RunForm, StopForm,
    // Edit
    Delete,
    BringToFront, SendToBack, BringForward, SendBackward,
    // Align
    AlignLeft, AlignRight, AlignTop, AlignBottom,
    CenterH, CenterV, SpaceH, SpaceV,
    // Style
    FormatPainter, AutoArrange,
    // Misc
    ReportBug,
}

/// Draw the merged 50-px icon toolbar.
///
/// `can_undo/redo/sel/multi` drive enabled/disabled state.
/// `preview_on` / `anim_preview_on` / `grid_on` / `glass_on` / `form_running`
/// drive toggle-button active state.
/// `fp_active` — true when format painter is in paste mode.
///
/// Returns the action clicked this frame (or `None`).
pub(crate) fn draw_icon_toolbar(
    ui:             &mut egui::Ui,
    can_undo:       bool,
    can_redo:       bool,
    has_sel:        bool,
    has_multi:      bool,
    preview_on:     bool,
    grid_on:        bool,
    glass_on:       bool,
    form_running:   bool,
    fp_active:      bool,
) -> DesignerToolbarAction {
    use egui::{Color32, Rect, Vec2};

    let mut action = DesignerToolbarAction::None;

    // Fill the ENTIRE reserved panel height with the toolbox (panel) colour.
    // egui's Frame only paints the *content* area (the icon row), and the content
    // ui's `max_rect` is content-sized too — so we use `clip_rect`, which the panel
    // sets to its full reserved rect. Without this the unused bottom of the
    // `exact_height` panel showed the white viewport clear (the "white band").
    let strip_rect = ui.clip_rect();
    ui.painter().rect_filled(strip_rect, 0.0, ui.visuals().panel_fill);

    // Suppress egui widget backgrounds so icons paint cleanly over glass
    {
        let v = &mut ui.style_mut().visuals;
        v.widgets.inactive.weak_bg_fill = Color32::TRANSPARENT;
        v.widgets.inactive.bg_fill      = Color32::TRANSPARENT;
        v.widgets.hovered.weak_bg_fill  = Color32::from_rgba_premultiplied(60, 90, 180, 55);
        v.widgets.hovered.bg_fill       = Color32::from_rgba_premultiplied(60, 90, 180, 55);
        v.widgets.active.weak_bg_fill   = Color32::from_rgba_premultiplied(80, 120, 220, 80);
        v.widgets.active.bg_fill        = Color32::from_rgba_premultiplied(80, 120, 220, 80);
    }

    // ── Icon button helper ────────────────────────────────────────────────────
    // Returns true if clicked. Draws a 32×32 interact area containing a 22×22
    // painter-drawn icon centred inside it.
    // Icon/button sizes (toolbar is exactly 50px tall):
    //  - `icon_ref_ext` is the final, uniform icon size — every icon is scaled
    //    so its bounding box is exactly this many px.
    //  - `icon_size` is the coordinate space the painters draw in (kept a touch
    //    larger so the normalisation factor stays near 1, keeping strokes crisp).
    //  - `btn_size` is the click/hover cell (icon size + a little padding).
    let icon_ref_ext = 26.25_f32;
    let icon_size    = 30.0_f32;
    // Cell = icon + 5px padding on each side, so icons aren't crowded together.
    let btn_size     = icon_ref_ext + 10.0;
    // Inter-group gap — half of one icon (button) width, with a separator line.
    let group_gap    = btn_size * 0.5;

    // Colour palette (frozen white glass)
    let col_normal   = Color32::from_rgba_premultiplied(215, 225, 255, 210);
    let col_dim      = Color32::from_rgba_premultiplied(215, 225, 255, 70);
    let _col_active  = Color32::from_rgba_premultiplied(130, 180, 255, 255);
    let col_accent   = Color32::from_rgba_premultiplied(255, 220, 100, 240); // gold for toggles

    // Closure: allocate a button rect, draw the icon (collected as shapes and
    // uniformly resized to the reference extent), return whether it was clicked.
    let mut icon_btn = |ui: &mut egui::Ui,
                        enabled: bool,
                        toggled: bool,
                        tooltip: &str,
                        draw_fn: &dyn Fn(&mut Vec<Shape>, Rect, Color32)| -> bool
    {
        let (resp, painter) = ui.allocate_painter(Vec2::splat(btn_size), egui::Sense::click());
        let icon_rect = Rect::from_center_size(resp.rect.center(), Vec2::splat(icon_size));
        let col = if !enabled     { col_dim    }
                  else if toggled { col_accent  }
                  else            { col_normal  };
        // Hover/active bg ring
        if resp.hovered() && enabled {
            painter.rect_filled(resp.rect, 6.0,
                Color32::from_rgba_premultiplied(80, 110, 200, 40));
        }
        if toggled {
            painter.rect_filled(resp.rect, 6.0,
                Color32::from_rgba_premultiplied(60, 100, 200, 55));
        }
        // Draw the icon into a shape buffer, then scale it to the common size.
        let mut shapes: Vec<Shape> = Vec::new();
        draw_fn(&mut shapes, icon_rect, col);
        normalize_icon(&mut shapes, icon_rect.center(), icon_ref_ext);
        painter.extend(shapes);
        if !tooltip.is_empty() {
            resp.clone().on_hover_text(tooltip);
        }
        enabled && resp.clicked()
    };

    // `horizontal_centered` centres the icon row vertically within the toolbar height.
    ui.horizontal_centered(|ui| {
        // Tight spacing within a group; groups are separated by one icon width below.
        ui.spacing_mut().item_spacing.x = 0.0;
        ui.add_space(4.0);

        // ── Group 1: History ─────────────────────────────────────────────────
        if icon_btn(ui, can_undo, false, "Undo  (⌘Z)", &icon_undo) {
            action = DesignerToolbarAction::Undo;
        }
        if icon_btn(ui, can_redo, false, "Redo  (⌘⇧Z)", &icon_redo) {
            action = DesignerToolbarAction::Redo;
        }

        group_separator(ui, group_gap);

        // ── Group 2: File ────────────────────────────────────────────────────
        if icon_btn(ui, true, false, "Save & Generate COBOL  (⌘S)", &icon_save) {
            action = DesignerToolbarAction::SaveAndGenerate;
        }
        if icon_btn(ui, true, false, "Generate COBOL only", &icon_generate) {
            action = DesignerToolbarAction::GenerateOnly;
        }

        group_separator(ui, group_gap);

        // ── Group 3: View ────────────────────────────────────────────────────
        if icon_btn(ui, true, preview_on, "Toggle Live Preview window", &icon_preview) {
            action = DesignerToolbarAction::TogglePreview;
        }
        if icon_btn(ui, true, false, "Play all OnFormLoad animations", &icon_anim_play) {
            action = DesignerToolbarAction::ToggleAnimPreview;
        }
        if icon_btn(ui, true, grid_on, "Toggle Grid", &icon_grid) {
            action = DesignerToolbarAction::ToggleGrid;
        }
        if icon_btn(ui, true, glass_on, "Toggle Glass Theme", &icon_glass) {
            action = DesignerToolbarAction::ToggleGlass;
        }

        group_separator(ui, group_gap);

        // ── Group 4: Run ─────────────────────────────────────────────────────
        if form_running {
            if icon_btn(ui, true, true, "Stop Running Form", &icon_stop) {
                action = DesignerToolbarAction::StopForm;
            }
        } else {
            if icon_btn(ui, true, false, "Run Form (live interpreter)", &icon_run) {
                action = DesignerToolbarAction::RunForm;
            }
        }

        group_separator(ui, group_gap);

        // ── Group 5: Edit Controls ───────────────────────────────────────────
        if icon_btn(ui, has_sel, false, "Delete selected  (Del)", &icon_delete) {
            action = DesignerToolbarAction::Delete;
        }
        if icon_btn(ui, has_sel, false, "Bring to Front", &icon_bring_front) {
            action = DesignerToolbarAction::BringToFront;
        }
        if icon_btn(ui, has_sel, false, "Send to Back", &icon_send_back) {
            action = DesignerToolbarAction::SendToBack;
        }
        if icon_btn(ui, has_sel, false, "Bring Forward (+1 z-order)", &icon_fwd) {
            action = DesignerToolbarAction::BringForward;
        }
        if icon_btn(ui, has_sel, false, "Send Backward (-1 z-order)", &icon_bwd) {
            action = DesignerToolbarAction::SendBackward;
        }

        group_separator(ui, group_gap);

        // ── Group 6: Align ───────────────────────────────────────────────────
        if icon_btn(ui, has_multi, false, "Align Left Edges", &icon_align_left) {
            action = DesignerToolbarAction::AlignLeft;
        }
        if icon_btn(ui, has_multi, false, "Align Right Edges", &icon_align_right) {
            action = DesignerToolbarAction::AlignRight;
        }
        if icon_btn(ui, has_multi, false, "Align Top Edges", &icon_align_top) {
            action = DesignerToolbarAction::AlignTop;
        }
        if icon_btn(ui, has_multi, false, "Align Bottom Edges", &icon_align_bottom) {
            action = DesignerToolbarAction::AlignBottom;
        }
        if icon_btn(ui, has_multi, false, "Center Horizontally", &icon_center_h) {
            action = DesignerToolbarAction::CenterH;
        }
        if icon_btn(ui, has_multi, false, "Center Vertically", &icon_center_v) {
            action = DesignerToolbarAction::CenterV;
        }
        if icon_btn(ui, has_multi, false, "Space Evenly (horizontal)", &icon_space_h) {
            action = DesignerToolbarAction::SpaceH;
        }
        if icon_btn(ui, has_multi, false, "Space Evenly (vertical)", &icon_space_v) {
            action = DesignerToolbarAction::SpaceV;
        }

        group_separator(ui, group_gap);

        // ── Group 7: Style ───────────────────────────────────────────────────
        if icon_btn(ui, has_sel, fp_active, "Format Painter — copy/paste control style", &icon_format_painter) {
            action = DesignerToolbarAction::FormatPainter;
        }
        if icon_btn(ui, true, false, "Auto-arrange: labels left, inputs right", &icon_auto_arrange) {
            action = DesignerToolbarAction::AutoArrange;
        }

        group_separator(ui, group_gap);

        // ── Group 8: Misc ────────────────────────────────────────────────────
        if icon_btn(ui, true, false, "Report a Problem with the Form Designer", &icon_bug) {
            action = DesignerToolbarAction::ReportBug;
        }
    });

    action
}


// ── Icon painters ─────────────────────────────────────────────────────────────
// Each receives (shape buffer, rect, colour) and PUSHES shapes into the buffer.
// The buffer is then uniformly scaled by `normalize_icon` so every icon ends up
// the same bounding size (matched to `icon_send_back`). Style: frozen white glass.

/// Uniformly scale a set of icon shapes so their combined bounding box has a
/// maximum extent of `target_ext`, re-centred on `center`. This is what makes
/// every toolbar icon render at an identical visual size.
fn normalize_icon(shapes: &mut [Shape], center: Pos2, target_ext: f32) {
    use egui::emath::TSTransform;
    let mut bbox = Rect::NOTHING;
    for s in shapes.iter() { bbox = bbox.union(s.visual_bounding_rect()); }
    if !bbox.is_finite() { return; }
    let cur = bbox.size().max_elem();
    if cur <= 0.01 || target_ext <= 0.01 { return; }
    let k = target_ext / cur;
    let translation = center.to_vec2() - k * bbox.center().to_vec2();
    let t = TSTransform::new(translation, k);
    for s in shapes.iter_mut() { s.transform(t); }
}

/// Draw a vertical separator line in the middle of a `gap`-wide space between
/// two icon groups.
fn group_separator(ui: &mut Ui, gap: f32) {
    ui.add_space(gap * 0.5);
    let rect = ui.max_rect();
    let cy = rect.center().y;
    let half = 14.0;
    let x = ui.cursor().min.x;
    ui.painter().vline(
        x,
        (cy - half)..=(cy + half),
        Stroke::new(1.0, Color32::from_rgba_premultiplied(120, 150, 220, 110)),
    );
    ui.add_space(gap * 0.5);
}

fn icon_undo(out: &mut Vec<Shape>, r: Rect, c: Color32) {
    let s = Stroke::new(1.8, c);
    let cx = r.center().x; let cy = r.center().y;
    let rad = r.width() * 0.38;
    let pts: Vec<Pos2> = (0..=14).map(|i| {
        let a = std::f32::consts::PI * (0.3 + i as f32 / 14.0 * 1.4);
        Pos2::new(cx + rad * a.cos(), cy - rad * a.sin())
    }).collect();
    for w in pts.windows(2) { out.push(Shape::line_segment([w[0], w[1]], s)); }
    let tip = pts[0];
    out.push(Shape::line_segment([tip, tip + egui::vec2(-4.0, 1.0)], s));
    out.push(Shape::line_segment([tip, tip + egui::vec2(0.0, -4.0)], s));
}

fn icon_redo(out: &mut Vec<Shape>, r: Rect, c: Color32) {
    let s = Stroke::new(1.8, c);
    let cx = r.center().x; let cy = r.center().y;
    let rad = r.width() * 0.38;
    let pts: Vec<Pos2> = (0..=14).map(|i| {
        let a = std::f32::consts::PI * (0.3 + i as f32 / 14.0 * 1.4);
        Pos2::new(cx - rad * a.cos(), cy - rad * a.sin())
    }).collect();
    for w in pts.windows(2) { out.push(Shape::line_segment([w[0], w[1]], s)); }
    let tip = pts[0];
    out.push(Shape::line_segment([tip, tip + egui::vec2(4.0, 1.0)], s));
    out.push(Shape::line_segment([tip, tip + egui::vec2(0.0, -4.0)], s));
}

fn icon_save(out: &mut Vec<Shape>, r: Rect, c: Color32) {
    let s = Stroke::new(1.6, c);
    out.push(Shape::rect_stroke(r.shrink(2.0), 1.5, s));
    let bot = Rect::from_min_max(
        Pos2::new(r.min.x + 4.0, r.max.y - r.height() * 0.32),
        r.max - egui::vec2(4.0, 2.0),
    );
    out.push(Shape::rect_stroke(bot, 0.0, s));
    let notch = Rect::from_min_size(
        Pos2::new(r.max.x - r.width() * 0.38, r.min.y + 2.0),
        Vec2::new(r.width() * 0.25, r.height() * 0.30),
    );
    out.push(Shape::rect_stroke(notch, 0.0, Stroke::new(1.4, c)));
    let mid_x = r.center().x - 1.0;
    out.push(Shape::line_segment([Pos2::new(mid_x, r.min.y + 4.0), Pos2::new(mid_x, bot.min.y - 2.0)], s));
    out.push(Shape::line_segment([Pos2::new(mid_x - 3.0, bot.min.y - 5.0), Pos2::new(mid_x, bot.min.y - 2.0)], s));
    out.push(Shape::line_segment([Pos2::new(mid_x + 3.0, bot.min.y - 5.0), Pos2::new(mid_x, bot.min.y - 2.0)], s));
}

fn icon_generate(out: &mut Vec<Shape>, r: Rect, c: Color32) {
    let s = Stroke::new(1.8, c);
    let cx = r.center().x; let cy = r.center().y;
    out.push(Shape::line_segment([Pos2::new(cx - 5.0, cy - 5.0), Pos2::new(cx - 9.0, cy)], s));
    out.push(Shape::line_segment([Pos2::new(cx - 9.0, cy), Pos2::new(cx - 5.0, cy + 5.0)], s));
    out.push(Shape::line_segment([Pos2::new(cx + 5.0, cy - 5.0), Pos2::new(cx + 9.0, cy)], s));
    out.push(Shape::line_segment([Pos2::new(cx + 9.0, cy), Pos2::new(cx + 5.0, cy + 5.0)], s));
    out.push(Shape::line_segment([Pos2::new(cx + 3.0, cy - 6.0), Pos2::new(cx - 3.0, cy + 6.0)], s));
}

fn icon_preview(out: &mut Vec<Shape>, r: Rect, c: Color32) {
    let s = Stroke::new(1.6, c);
    let cx = r.center().x; let cy = r.center().y;
    let brow_pts: Vec<Pos2> = (0..=12).map(|i| {
        let t = i as f32 / 12.0;
        let a = std::f32::consts::PI * t;
        Pos2::new(cx + r.width() * 0.42 * (a - std::f32::consts::PI * 0.5).cos() * 1.2,
                  cy + r.height() * 0.28 * a.sin())
    }).collect();
    for w in brow_pts.windows(2) { out.push(Shape::line_segment([w[0], w[1]], s)); }
    let bot_pts: Vec<Pos2> = brow_pts.iter().map(|pt| Pos2::new(pt.x, cy - (pt.y - cy))).collect();
    for w in bot_pts.windows(2) { out.push(Shape::line_segment([w[0], w[1]], s)); }
    out.push(Shape::circle_stroke(r.center(), r.width() * 0.14, s));
    out.push(Shape::circle_filled(r.center(), r.width() * 0.07, c));
}

fn icon_anim_play(out: &mut Vec<Shape>, r: Rect, c: Color32) {
    let s = Stroke::new(1.5, c);
    let cx = r.center().x; let cy = r.center().y;
    let rad = r.width() * 0.4;
    out.push(Shape::circle_stroke(Pos2::new(cx, cy), rad, s));
    let pts = vec![
        Pos2::new(cx - rad * 0.3, cy - rad * 0.45),
        Pos2::new(cx + rad * 0.5, cy),
        Pos2::new(cx - rad * 0.3, cy + rad * 0.45),
    ];
    out.push(Shape::convex_polygon(pts, c, Stroke::NONE));
    for (dx, dy) in [(-rad*0.75, -rad*0.6), (rad*0.75, -rad*0.6), (0.0_f32, -rad*0.9)] {
        out.push(Shape::circle_filled(Pos2::new(cx + dx, cy + dy), 1.5, c));
    }
}

fn icon_grid(out: &mut Vec<Shape>, r: Rect, c: Color32) {
    let s = Stroke::new(1.3, c);
    let sr = r.shrink(3.0);
    for row in 0..3 { for col in 0..3 {
        let pt = Pos2::new(sr.min.x + col as f32 * sr.width() * 0.5,
                           sr.min.y + row as f32 * sr.height() * 0.5);
        out.push(Shape::circle_filled(pt, 1.5, c));
    }}
    out.push(Shape::rect_stroke(sr, 1.0, s));
}

fn icon_glass(out: &mut Vec<Shape>, r: Rect, c: Color32) {
    let s = Stroke::new(1.6, c);
    let cx = r.center().x; let cy = r.center().y;
    let hw = r.width() * 0.42; let hh = r.height() * 0.42;
    let pts = vec![
        Pos2::new(cx, cy - hh),
        Pos2::new(cx + hw, cy - hh * 0.2),
        Pos2::new(cx + hw * 0.6, cy + hh),
        Pos2::new(cx - hw * 0.6, cy + hh),
        Pos2::new(cx - hw, cy - hh * 0.2),
    ];
    for i in 0..pts.len() { out.push(Shape::line_segment([pts[i], pts[(i+1) % pts.len()]], s)); }
    out.push(Shape::line_segment([pts[0], pts[2]], Stroke::new(1.0, Color32::from_rgba_premultiplied(c.r(), c.g(), c.b(), 90))));
}

fn icon_run(out: &mut Vec<Shape>, r: Rect, c: Color32) {
    let pts = vec![
        Pos2::new(r.min.x + r.width() * 0.28, r.min.y + r.height() * 0.18),
        Pos2::new(r.max.x - r.width() * 0.15, r.center().y),
        Pos2::new(r.min.x + r.width() * 0.28, r.max.y - r.height() * 0.18),
    ];
    out.push(Shape::convex_polygon(pts, c, Stroke::NONE));
}

fn icon_stop(out: &mut Vec<Shape>, r: Rect, c: Color32) {
    out.push(Shape::rect_filled(r.shrink(r.width() * 0.22), 2.0, c));
}

fn icon_delete(out: &mut Vec<Shape>, r: Rect, c: Color32) {
    let s = Stroke::new(1.8, c);
    let sr = r.shrink(3.5);
    let body = Rect::from_min_max(
        Pos2::new(sr.min.x + 2.0, sr.min.y + sr.height() * 0.28),
        sr.max,
    );
    out.push(Shape::rect_stroke(body, 1.0, s));
    out.push(Shape::line_segment([Pos2::new(sr.min.x, sr.min.y + sr.height() * 0.22),
                    Pos2::new(sr.max.x, sr.min.y + sr.height() * 0.22)], s));
    out.push(Shape::line_segment([Pos2::new(sr.center().x - 3.0, sr.min.y),
                    Pos2::new(sr.center().x + 3.0, sr.min.y)], s));
    for i in 1..=3 {
        let x = body.min.x + body.width() * i as f32 / 4.0;
        out.push(Shape::line_segment([Pos2::new(x, body.min.y + 3.0), Pos2::new(x, body.max.y - 3.0)],
            Stroke::new(1.2, c)));
    }
}

fn icon_bring_front(out: &mut Vec<Shape>, r: Rect, c: Color32) {
    let s = Stroke::new(1.5, c);
    let cx = r.center().x; let top = r.min.y + 4.0; let bot = r.max.y - 4.0;
    let r1 = Rect::from_min_max(Pos2::new(r.min.x + 5.0, top + 4.0), Pos2::new(r.max.x - 2.0, bot));
    let r2 = Rect::from_min_max(Pos2::new(r.min.x + 2.0, top + 8.0), Pos2::new(r.max.x - 5.0, bot + 3.0));
    out.push(Shape::rect_stroke(r2, 1.0, Stroke::new(1.2, Color32::from_rgba_premultiplied(c.r(), c.g(), c.b(), 120))));
    out.push(Shape::rect_filled(r1, 1.0, Color32::from_rgba_premultiplied(c.r(), c.g(), c.b(), 40)));
    out.push(Shape::rect_stroke(r1, 1.0, s));
    out.push(Shape::line_segment([Pos2::new(cx, top - 1.0), Pos2::new(cx, top + 6.0)], s));
    out.push(Shape::line_segment([Pos2::new(cx - 3.0, top + 3.0), Pos2::new(cx, top - 1.0)], s));
    out.push(Shape::line_segment([Pos2::new(cx + 3.0, top + 3.0), Pos2::new(cx, top - 1.0)], s));
}

fn icon_send_back(out: &mut Vec<Shape>, r: Rect, c: Color32) {
    let s = Stroke::new(1.5, c);
    let cx = r.center().x; let top = r.min.y + 4.0; let bot = r.max.y - 4.0;
    let r1 = Rect::from_min_max(Pos2::new(r.min.x + 5.0, top + 4.0), Pos2::new(r.max.x - 2.0, bot));
    let r2 = Rect::from_min_max(Pos2::new(r.min.x + 2.0, top + 8.0), Pos2::new(r.max.x - 5.0, bot + 3.0));
    out.push(Shape::rect_filled(r1, 1.0, Color32::from_rgba_premultiplied(c.r(), c.g(), c.b(), 40)));
    out.push(Shape::rect_stroke(r1, 1.0, Stroke::new(1.2, Color32::from_rgba_premultiplied(c.r(), c.g(), c.b(), 120))));
    out.push(Shape::rect_stroke(r2, 1.0, s));
    out.push(Shape::line_segment([Pos2::new(cx, bot + 4.0), Pos2::new(cx, bot - 3.0)], s));
    out.push(Shape::line_segment([Pos2::new(cx - 3.0, bot + 1.0), Pos2::new(cx, bot + 4.0)], s));
    out.push(Shape::line_segment([Pos2::new(cx + 3.0, bot + 1.0), Pos2::new(cx, bot + 4.0)], s));
}

fn icon_fwd(out: &mut Vec<Shape>, r: Rect, c: Color32) {
    let s = Stroke::new(1.5, c);
    let cx = r.center().x; let cy = r.center().y;
    out.push(Shape::rect_stroke(Rect::from_center_size(Pos2::new(cx - 2.0, cy + 1.0), Vec2::new(10.0, 8.0)), 1.0, Stroke::new(1.2, Color32::from_rgba_premultiplied(c.r(), c.g(), c.b(), 120))));
    out.push(Shape::rect_stroke(Rect::from_center_size(Pos2::new(cx + 1.0, cy - 1.0), Vec2::new(10.0, 8.0)), 1.0, s));
    // "+" marker (Bring Forward = +1 z-order)
    let mx = cx + 1.0; let my = cy - 1.0;
    out.push(Shape::line_segment([Pos2::new(mx - 2.5, my), Pos2::new(mx + 2.5, my)], s));
    out.push(Shape::line_segment([Pos2::new(mx, my - 2.5), Pos2::new(mx, my + 2.5)], s));
}

fn icon_bwd(out: &mut Vec<Shape>, r: Rect, c: Color32) {
    let s = Stroke::new(1.5, c);
    let cx = r.center().x; let cy = r.center().y;
    out.push(Shape::rect_stroke(Rect::from_center_size(Pos2::new(cx + 2.0, cy - 1.0), Vec2::new(10.0, 8.0)), 1.0, Stroke::new(1.2, Color32::from_rgba_premultiplied(c.r(), c.g(), c.b(), 120))));
    out.push(Shape::rect_stroke(Rect::from_center_size(Pos2::new(cx - 1.0, cy + 1.0), Vec2::new(10.0, 8.0)), 1.0, s));
    // "−" marker (Send Backward = -1 z-order)
    let mx = cx - 1.0; let my = cy + 1.0;
    out.push(Shape::line_segment([Pos2::new(mx - 2.5, my), Pos2::new(mx + 2.5, my)], s));
}

fn _icon_align(out: &mut Vec<Shape>, r: Rect, c: Color32, horiz: bool, lo_side: bool) {
    let s = Stroke::new(1.5, c);
    let sr = r.shrink(3.0);
    if horiz {
        let x = if lo_side { sr.min.x } else { sr.max.x };
        out.push(Shape::line_segment([Pos2::new(x, sr.min.y), Pos2::new(x, sr.max.y)], Stroke::new(1.8, c)));
        for (i, w, h) in [(0, 8.0, 4.0), (1, 6.0, 4.0)] {
            let y = sr.min.y + sr.height() * (0.2 + i as f32 * 0.45);
            let x_rect = if lo_side { x + 1.0 } else { x - w - 1.0 };
            out.push(Shape::rect_stroke(Rect::from_min_size(Pos2::new(x_rect, y), Vec2::new(w, h)), 1.0, s));
        }
    } else {
        let y = if lo_side { sr.min.y } else { sr.max.y };
        out.push(Shape::line_segment([Pos2::new(sr.min.x, y), Pos2::new(sr.max.x, y)], Stroke::new(1.8, c)));
        for (i, w, h) in [(0, 4.0, 7.0), (1, 4.0, 5.0)] {
            let x = sr.min.x + sr.width() * (0.2 + i as f32 * 0.45);
            let y_rect = if lo_side { y + 1.0 } else { y - h - 1.0 };
            out.push(Shape::rect_stroke(Rect::from_min_size(Pos2::new(x, y_rect), Vec2::new(w, h)), 1.0, s));
        }
    }
}
fn icon_align_left(out: &mut Vec<Shape>, r: Rect, c: Color32)   { _icon_align(out, r, c, true,  true);  }
fn icon_align_right(out: &mut Vec<Shape>, r: Rect, c: Color32)  { _icon_align(out, r, c, true,  false); }
fn icon_align_top(out: &mut Vec<Shape>, r: Rect, c: Color32)    { _icon_align(out, r, c, false, true);  }
fn icon_align_bottom(out: &mut Vec<Shape>, r: Rect, c: Color32) { _icon_align(out, r, c, false, false); }

fn icon_center_h(out: &mut Vec<Shape>, r: Rect, c: Color32) {
    let s = Stroke::new(1.5, c); let sr = r.shrink(3.0);
    let cx = sr.center().x;
    out.push(Shape::line_segment([Pos2::new(cx, sr.min.y), Pos2::new(cx, sr.max.y)], Stroke::new(1.8, c)));
    for (dy, w) in [(0.15_f32, 9.0_f32), (0.55, 7.0)] {
        let y = sr.min.y + sr.height() * dy;
        out.push(Shape::rect_stroke(Rect::from_center_size(Pos2::new(cx, y + 2.0), Vec2::new(w, 4.0)), 1.0, s));
    }
}

fn icon_center_v(out: &mut Vec<Shape>, r: Rect, c: Color32) {
    let s = Stroke::new(1.5, c); let sr = r.shrink(3.0);
    let cy = sr.center().y;
    out.push(Shape::line_segment([Pos2::new(sr.min.x, cy), Pos2::new(sr.max.x, cy)], Stroke::new(1.8, c)));
    for (dx, h) in [(0.15_f32, 9.0_f32), (0.55, 7.0)] {
        let x = sr.min.x + sr.width() * dx;
        out.push(Shape::rect_stroke(Rect::from_center_size(Pos2::new(x + 2.0, cy), Vec2::new(4.0, h)), 1.0, s));
    }
}

fn icon_space_h(out: &mut Vec<Shape>, r: Rect, c: Color32) {
    let s = Stroke::new(1.4, c); let sr = r.shrink(3.0);
    for i in 0..3 {
        let x = sr.min.x + sr.width() * (0.15 + i as f32 * 0.35);
        out.push(Shape::rect_stroke(Rect::from_min_size(Pos2::new(x, sr.min.y + 3.0),
            Vec2::new(3.5, sr.height() - 6.0)), 1.0, s));
    }
    let y = sr.max.y - 2.0;
    out.push(Shape::line_segment([Pos2::new(sr.min.x, y), Pos2::new(sr.max.x, y)], s));
    out.push(Shape::line_segment([Pos2::new(sr.min.x + 2.0, y - 2.0), Pos2::new(sr.min.x, y)], s));
    out.push(Shape::line_segment([Pos2::new(sr.max.x - 2.0, y - 2.0), Pos2::new(sr.max.x, y)], s));
}

fn icon_space_v(out: &mut Vec<Shape>, r: Rect, c: Color32) {
    let s = Stroke::new(1.4, c); let sr = r.shrink(3.0);
    for i in 0..3 {
        let y = sr.min.y + sr.height() * (0.12 + i as f32 * 0.35);
        out.push(Shape::rect_stroke(Rect::from_min_size(Pos2::new(sr.min.x + 3.0, y),
            Vec2::new(sr.width() - 6.0, 3.5)), 1.0, s));
    }
    let x = sr.max.x - 2.0;
    out.push(Shape::line_segment([Pos2::new(x, sr.min.y), Pos2::new(x, sr.max.y)], s));
    out.push(Shape::line_segment([Pos2::new(x - 2.0, sr.min.y + 2.0), Pos2::new(x, sr.min.y)], s));
    out.push(Shape::line_segment([Pos2::new(x - 2.0, sr.max.y - 2.0), Pos2::new(x, sr.max.y)], s));
}

fn icon_format_painter(out: &mut Vec<Shape>, r: Rect, c: Color32) {
    let s = Stroke::new(1.5, c);
    let cx = r.center().x; let cy = r.center().y;
    out.push(Shape::line_segment([Pos2::new(cx + 2.0, cy - 7.0), Pos2::new(cx + 2.0, cy + 4.0)], s));
    out.push(Shape::line_segment([Pos2::new(cx + 2.0, cy - 7.0), Pos2::new(cx - 5.0, cy - 7.0)], s));
    out.push(Shape::rect_stroke(Rect::from_min_size(Pos2::new(cx - 6.0, cy - 5.0), Vec2::new(10.0, 5.0)), 1.0, s));
    out.push(Shape::line_segment([Pos2::new(cx + 2.0, cy + 4.0), Pos2::new(cx + 2.0, cy + 7.0)], Stroke::new(1.2, c)));
    out.push(Shape::circle_filled(Pos2::new(cx + 2.0, cy + 8.0), 1.5, c));
}

fn icon_auto_arrange(out: &mut Vec<Shape>, r: Rect, c: Color32) {
    let s = Stroke::new(1.4, c); let sr = r.shrink(3.0);
    out.push(Shape::rect_stroke(Rect::from_min_size(sr.min, Vec2::new(sr.width() * 0.38, 4.5)), 1.0, s));
    out.push(Shape::rect_stroke(Rect::from_min_size(Pos2::new(sr.min.x + sr.width() * 0.45, sr.min.y),
        Vec2::new(sr.width() * 0.55, 4.5)), 1.0,
        Stroke::new(1.4, Color32::from_rgba_premultiplied(c.r(), c.g(), c.b(), 180))));
    let y2 = sr.min.y + 7.0;
    out.push(Shape::rect_stroke(Rect::from_min_size(Pos2::new(sr.min.x, y2), Vec2::new(sr.width() * 0.30, 4.5)), 1.0, s));
    out.push(Shape::rect_stroke(Rect::from_min_size(Pos2::new(sr.min.x + sr.width() * 0.45, y2),
        Vec2::new(sr.width() * 0.55, 4.5)), 1.0,
        Stroke::new(1.4, Color32::from_rgba_premultiplied(c.r(), c.g(), c.b(), 180))));
    out.push(Shape::line_segment([Pos2::new(sr.center().x - 1.0, sr.max.y - 5.0),
                    Pos2::new(sr.center().x + 4.0, sr.max.y - 1.0)], Stroke::new(1.6, c)));
    out.push(Shape::circle_filled(Pos2::new(sr.center().x - 1.0, sr.max.y - 5.0), 2.0, c));
}

fn icon_bug(out: &mut Vec<Shape>, r: Rect, c: Color32) {
    let s = Stroke::new(1.4, c);
    let cx = r.center().x; let cy = r.center().y;
    let pts: Vec<Pos2> = (0..=20).map(|i| {
        let a = std::f32::consts::TAU * i as f32 / 20.0;
        Pos2::new(cx + 4.5 * a.cos(), cy + 1.5 + 5.5 * a.sin())
    }).collect();
    for w in pts.windows(2) { out.push(Shape::line_segment([w[0], w[1]], s)); }
    out.push(Shape::circle_stroke(Pos2::new(cx, cy - 4.0), 3.0, s));
    out.push(Shape::line_segment([Pos2::new(cx - 1.5, cy - 6.5), Pos2::new(cx - 4.0, cy - 9.0)], s));
    out.push(Shape::line_segment([Pos2::new(cx + 1.5, cy - 6.5), Pos2::new(cx + 4.0, cy - 9.0)], s));
    for (i, sign) in [(-3.0_f32, -1.0_f32), (0.0, -1.0), (3.0, -1.0),
                      (-3.0, 1.0), (0.0, 1.0), (3.0, 1.0)] {
        let by = cy + 1.5 + i;
        out.push(Shape::line_segment([Pos2::new(cx + sign * 4.5, by),
                        Pos2::new(cx + sign * 8.0, by - 1.5)], s));
    }
}

/// Return `(width, height)` for a named preset, or `None` for "Custom" / unknown.
pub(crate) fn target_preset_size(name: &str) -> Option<(u32, u32)> {
    if name == "Custom" { return None; }
    TARGET_PRESETS.iter()
        .find(|(label, ..)| *label == name)
        .map(|(_, w, h)| (*w, *h))
}

// ── Shared Glass ComboBox renderer ───────────────────────────────────────────
//
// Used identically in both the Form Preview and the Run Form window so the
// look-and-feel is pixel-identical regardless of execution mode.
//
// The combo is rendered in two passes:
//   1. `glass_combo_header` — draws the closed bar into the main painter pass.
//   2. `glass_combo_popup`  — draws the open dropdown AFTER all controls so it
//      always floats on top.
//
// The caller is responsible for storing open/closed state in a
// `HashMap<String, bool>` keyed by control ID.

/// Draw the ComboBox header bar (always visible regardless of open state).
///
/// Returns `true` if the user clicked the bar this frame (caller should toggle
/// the open flag).
pub(crate) fn glass_combo_header(
    painter:     &egui::Painter,
    ui:          &mut egui::Ui,
    rect:        egui::Rect,
    widget_id:   egui::Id,
    selected:    &str,
    is_open:     bool,
    enabled:     bool,
    alpha:       f32,
) -> bool {
    use egui::{Color32, FontId, Pos2, Align2};

    // The closed combo field stays translucent glass (like other form fields).
    // Only the open dropdown list (`glass_combo_popup`) is opaque, so it doesn't
    // mix with the controls it overlaps.
    draw_glass(painter, rect, Color32::from_rgb(25, 38, 80), 6.0, false, alpha);
    painter.rect_stroke(
        rect, 6.0,
        egui::Stroke::new(1.0, Color32::from_rgba_premultiplied(100, 140, 230, 150)),
    );

    // Selected text
    painter.text(
        Pos2::new(rect.min.x + 8.0, rect.center().y),
        Align2::LEFT_CENTER, selected,
        FontId::proportional(12.0),
        Color32::from_rgb(220, 228, 255),
    );

    // ▼ / ▲ arrow
    painter.text(
        Pos2::new(rect.max.x - 13.0, rect.center().y),
        Align2::CENTER_CENTER,
        if is_open { "▲" } else { "▼" },
        FontId::proportional(9.0),
        Color32::from_rgba_premultiplied(160, 190, 255, 200),
    );

    // Click detection
    enabled && ui.interact(rect, widget_id, egui::Sense::click()).clicked()
}

/// Draw the ComboBox dropdown popup (call after all normal controls).
///
/// `header_rect`  — position of the closed header bar (popup opens below it).
/// `items`        — list of items to show.
/// `selected_val` — currently selected string (highlighted in the list).
///
/// Returns `Some(new_value)` if the user picked an item, `None` otherwise.
/// Returns `Some(selected_val.to_owned())` with a special sentinel `__close__`
/// if the user clicked outside (caller should close without changing value).
pub(crate) fn glass_combo_popup(
    ui:          &mut egui::Ui,
    ctrl_id_str: &str,
    header_rect: egui::Rect,
    items:       &[String],
    selected_val: &str,
) -> Option<GlassComboAction> {
    use egui::{Color32, FontId, Pos2, Vec2, Align2};

    let item_h   = 22.0_f32;
    let popup_h  = (items.len() as f32 * item_h).min(180.0);
    let popup_rect = egui::Rect::from_min_size(
        Pos2::new(header_rect.min.x, header_rect.max.y + 1.0),
        Vec2::new(header_rect.width(), popup_h),
    );

    let pointer_pos = ui.input(|i| i.pointer.hover_pos());
    let any_click   = ui.input(|i| i.pointer.any_click());

    // Click outside → close
    if any_click {
        let inside = header_rect.contains(pointer_pos.unwrap_or(Pos2::ZERO))
            || popup_rect.contains(pointer_pos.unwrap_or(Pos2::ZERO));
        if !inside {
            return Some(GlassComboAction::Close);
        }
    }

    // Popup background — strong "frozen" frost. An OPAQUE base fully occludes the
    // form content behind the popup (so it never mixes with the items), with a
    // subtle frosted sheen on top for the glass look.
    let pp = ui.painter_at(popup_rect);
    pp.rect_filled(popup_rect, 6.0, Color32::from_rgb(22, 30, 58));
    draw_glass(&pp, popup_rect, Color32::from_rgb(30, 42, 80), 6.0, false, 0.35);
    pp.rect_stroke(
        popup_rect, 6.0,
        egui::Stroke::new(1.0, Color32::from_rgba_premultiplied(90, 130, 220, 180)),
    );

    // Items
    let mut action = None;
    for (i, item) in items.iter().enumerate() {
        let item_y = popup_rect.min.y + i as f32 * item_h;
        if item_y + item_h > popup_rect.max.y { break; }

        let item_rect = egui::Rect::from_min_size(
            Pos2::new(popup_rect.min.x, item_y),
            Vec2::new(popup_rect.width(), item_h),
        );
        let iid = egui::Id::new(("glass_combo_item", ctrl_id_str, i));

        let is_sel  = item == selected_val;
        let hovered = pointer_pos.map(|p| item_rect.contains(p)).unwrap_or(false);

        if is_sel {
            pp.rect_filled(item_rect, 4.0, Color32::from_rgba_premultiplied(60, 100, 200, 120));
        } else if hovered {
            pp.rect_filled(item_rect, 4.0, Color32::from_rgba_premultiplied(50, 70, 150, 80));
        }

        pp.text(
            Pos2::new(item_rect.min.x + 10.0, item_rect.center().y),
            Align2::LEFT_CENTER, item,
            FontId::proportional(12.0),
            if is_sel { Color32::from_rgb(200, 220, 255) } else { Color32::from_rgb(210, 218, 245) },
        );

        if ui.interact(item_rect, iid, egui::Sense::click()).clicked() {
            action = Some(GlassComboAction::Select(item.clone()));
        }
    }

    action
}

/// Result of a `glass_combo_popup` interaction.
#[derive(Debug)]
pub(crate) enum GlassComboAction {
    /// User selected this item.
    Select(String),
    /// User clicked outside the popup — close without changing value.
    Close,
}

// ── Behavioral render tests — Phase 1: design-time canvas (`draw_control`) ──────
//
// These drive the REAL `draw_control` painter headlessly via an egui Context,
// capture the emitted `Shape`s, and assert that properties actually affect what
// is painted. Phase 2 (runtime/interactive: typed grid cells, calendar popup,
// animations) is covered separately via egui_kittest.
#[cfg(test)]
mod form_resize_tests {
    use super::*;

    #[test]
    fn detect_edge_classifies_right_bottom_corner() {
        let (w, h) = (400.0, 300.0);
        // Right edge, away from the bottom.
        assert_eq!(detect_form_edge(400, 150, w, h), Some(FormEdge::Right));
        assert_eq!(detect_form_edge(396, 150, w, h), Some(FormEdge::Right)); // inner band
        // Bottom edge, away from the right.
        assert_eq!(detect_form_edge(200, 300, w, h), Some(FormEdge::Bottom));
        // Bottom-right corner — both edges → corner.
        assert_eq!(detect_form_edge(400, 300, w, h), Some(FormEdge::Corner));
        // Interior → nothing.
        assert_eq!(detect_form_edge(200, 150, w, h), None);
        // Top-left corner is not a resize edge.
        assert_eq!(detect_form_edge(0, 0, w, h), None);
    }

    #[test]
    fn resize_drag_grows_form_and_clamps_to_minimum() {
        // Mirrors the math applied in `handle_drag` for DragState::ResizingForm.
        let resize = |edge: FormEdge, w: i32, h: i32, dx: i32, dy: i32| {
            let mut nw = w;
            let mut nh = h;
            if matches!(edge, FormEdge::Right | FormEdge::Corner) {
                nw = (w + dx).max(FORM_MIN_SIZE);
            }
            if matches!(edge, FormEdge::Bottom | FormEdge::Corner) {
                nh = (h + dy).max(FORM_MIN_SIZE);
            }
            (nw, nh)
        };
        assert_eq!(resize(FormEdge::Right,  400, 300,  50, 99), (450, 300));
        assert_eq!(resize(FormEdge::Bottom, 400, 300,  99, 40), (400, 340));
        assert_eq!(resize(FormEdge::Corner, 400, 300,  60, 30), (460, 330));
        // Shrinking past the minimum clamps to FORM_MIN_SIZE.
        assert_eq!(resize(FormEdge::Corner, 100, 100, -90, -90), (FORM_MIN_SIZE, FORM_MIN_SIZE));
    }
}

#[cfg(test)]
mod animator_tests {
    use super::draw_animator;
    use egui::{pos2, vec2, Rect};

    /// Write a 2-frame (red→blue) animated GIF, 100 ms each, to a temp file.
    fn write_gif() -> std::path::PathBuf {
        use image::{codecs::gif::GifEncoder, Delay, Frame, Rgba, RgbaImage};
        let dir = std::env::temp_dir().join(format!("rcrun-anim-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("two.gif");
        let mut f = std::fs::File::create(&path).unwrap();
        {
            let mut enc = GifEncoder::new(&mut f);
            let red = RgbaImage::from_pixel(8, 8, Rgba([255, 0, 0, 255]));
            let blue = RgbaImage::from_pixel(8, 8, Rgba([0, 0, 255, 255]));
            enc.encode_frame(Frame::from_parts(red, 0, 0, Delay::from_numer_denom_ms(100, 1))).unwrap();
            enc.encode_frame(Frame::from_parts(blue, 0, 0, Delay::from_numer_denom_ms(100, 1))).unwrap();
        }
        path
    }

    /// Render the Animator at virtual time `t` (seconds) and return the texture
    /// id of the painted image, if any.
    fn frame_tex(ctx: &egui::Context, src: &str, t: f64) -> Option<egui::TextureId> {
        let raw = egui::RawInput { time: Some(t), ..Default::default() };
        let out = ctx.run(raw, |ctx| {
            let painter = ctx.layer_painter(egui::LayerId::background());
            draw_animator(&painter, Rect::from_min_size(pos2(0.0, 0.0), vec2(64.0, 64.0)),
                "anim-key", src, true, true, "Fit", 1.0, false);
        });
        out.shapes.into_iter().find_map(|cs| match cs.shape {
            egui::Shape::Mesh(m) => Some(m.texture_id),
            _ => None,
        })
    }

    #[test]
    fn animator_paints_and_advances_frames_over_time() {
        let path = write_gif();
        let src = path.to_string_lossy().to_string();
        let ctx = egui::Context::default();

        // First render (t=0) decodes + shows frame 0; the playback clock starts here.
        let f0 = frame_tex(&ctx, &src, 0.0);
        assert!(f0.is_some(), "Animator should paint an image once a source is set");

        // 150 ms later we are on the second frame → a different texture.
        let f1 = frame_tex(&ctx, &src, 0.15);
        assert!(f1.is_some());
        assert_ne!(f0, f1, "Animator should advance to a different frame over time");

        let _ = std::fs::remove_dir_all(path.parent().unwrap());
    }

    #[test]
    fn animator_without_source_shows_placeholder_not_image() {
        let ctx = egui::Context::default();
        // No source → no image mesh is painted (placeholder text/box instead).
        assert!(frame_tex(&ctx, "", 0.0).is_none());
    }
}

#[cfg(test)]
mod render_behavior_tests {
    use super::*;
    use cobolt_forms::model::PropValue;
    use cobolt_forms::{Control, ControlType};

    /// Render a control through `draw_control` at the given origin; return shapes.
    fn render_at(ctrl: &Control, origin: Pos2) -> Vec<egui::Shape> {
        let ctx = egui::Context::default();
        ctx.set_fonts(egui::FontDefinitions::default());
        let out = ctx.run(egui::RawInput::default(), |ctx| {
            // Frame::none → the panel paints no background, so captured shapes are
            // exactly what `draw_control` emitted (no full-panel fill skewing bbox).
            egui::CentralPanel::default()
                .frame(egui::Frame::none())
                .show(ctx, |ui| {
                    let painter = ui.painter().clone();
                    draw_control(&painter, origin, ctrl, false, false, 1.0, 1.0, None);
                });
        });
        out.shapes.into_iter().map(|cs| cs.shape).collect()
    }

    fn render(ctrl: &Control) -> Vec<egui::Shape> {
        render_at(ctrl, Pos2::ZERO)
    }

    fn texts(shapes: &[egui::Shape]) -> Vec<egui::epaint::TextShape> {
        shapes
            .iter()
            .filter_map(|s| match s {
                egui::Shape::Text(t) => Some(t.clone()),
                _ => None,
            })
            .collect()
    }

    /// Union bounding box of all painted shapes.
    fn bbox(shapes: &[egui::Shape]) -> egui::Rect {
        let mut r = egui::Rect::NOTHING;
        for s in shapes {
            r = r.union(s.visual_bounding_rect());
        }
        r
    }

    /// All visual widgets the design-time canvas paints.
    fn visual_widgets() -> Vec<(ControlType, &'static str)> {
        use ControlType::*;
        vec![
            (Label, "Label"), (Button, "Button"), (TextBox, "TextBox"),
            (CheckBox, "CheckBox"), (RadioButton, "RadioButton"),
            (ComboBox, "ComboBox"), (ListBox, "ListBox"),
            (GroupBox, "GroupBox"), (Panel, "Panel"),
            (ProgressBar, "ProgressBar"), (Slider, "Slider"),
            (NumericUpDown, "NumericUpDown"), (DateTimePicker, "DateTimePicker"),
            (PictureBox, "PictureBox"), (DataGrid, "DataGrid"),
            (TabControl, "TabControl"), (TreeView, "TreeView"),
            (Line, "Line"), (Shape, "Shape"), (Splitter, "Splitter"),
            (MenuBar, "MenuBar"), (ToolBar, "ToolBar"), (StatusBar, "StatusBar"),
            (BarChart, "BarChart"), (LineChart, "LineChart"),
            (PieChart, "PieChart"), (AreaChart, "AreaChart"),
            (ScatterChart, "ScatterChart"), (DonutChart, "DonutChart"),
        ]
    }

    // ── Geometry: painting must follow the control's x/y ──────────────────────
    #[test]
    fn geometry_follows_position_for_every_widget() {
        for (ct, name) in visual_widgets() {
            let a = Control::new("W", ct.clone(), 10, 10);
            let mut b = Control::new("W", ct.clone(), 10, 10);
            b.rect.x = 110; // +100
            b.rect.y = 60; //  +50
            let ba = bbox(&render(&a));
            let bb = bbox(&render(&b));
            assert!(ba.is_finite() && bb.is_finite(), "{name}: nothing painted");
            let dx = bb.min.x - ba.min.x;
            let dy = bb.min.y - ba.min.y;
            assert!(
                (dx - 100.0).abs() < 3.0 && (dy - 50.0).abs() < 3.0,
                "{name}: painting did not follow position (Δ=({dx},{dy}), expected ~(100,50))"
            );
        }
    }

    // ── Caption / Text content ────────────────────────────────────────────────
    #[test]
    fn caption_is_painted_for_caption_widgets() {
        for ct in [ControlType::Label, ControlType::Button, ControlType::GroupBox] {
            let mut c = Control::new("W", ct.clone(), 5, 7);
            c.set_prop("Caption", PropValue::String("CAP-RC".into()));
            let ts = texts(&render(&c));
            assert!(
                ts.iter().any(|t| t.galley.text().contains("CAP-RC")),
                "{ct:?}: Caption not painted; texts={:?}",
                ts.iter().map(|t| t.galley.text().to_owned()).collect::<Vec<_>>()
            );
        }
    }

    #[test]
    fn textbox_text_is_painted() {
        let mut c = Control::new("TB", ControlType::TextBox, 5, 7);
        c.set_prop("Text", PropValue::String("TBVAL-RC".into()));
        let ts = texts(&render(&c));
        assert!(
            ts.iter().any(|t| t.galley.text().contains("TBVAL-RC")),
            "TextBox Text not painted"
        );
    }

    // ── Label font-style properties (LayoutJob format) ────────────────────────
    fn label_with(prop: &str) -> egui::epaint::TextShape {
        let mut c = Control::new("LBL", ControlType::Label, 5, 7);
        c.set_prop("Caption", PropValue::String("STYLE-RC".into()));
        if !prop.is_empty() {
            c.set_prop(prop, PropValue::Bool(true));
        }
        texts(&render(&c))
            .into_iter()
            .find(|t| t.galley.text().contains("STYLE-RC"))
            .expect("caption painted")
    }

    #[test]
    fn label_italic_underline_strike_apply() {
        assert!(
            label_with("Italic").galley.job.sections.iter().any(|s| s.format.italics),
            "Italic not applied"
        );
        assert!(
            label_with("Underline").galley.job.sections.iter().any(|s| s.format.underline.width > 0.0),
            "Underline not applied"
        );
        assert!(
            label_with("Strikethrough").galley.job.sections.iter().any(|s| s.format.strikethrough.width > 0.0),
            "Strikethrough not applied"
        );
        // Sanity: a plain label has none of them.
        let plain = label_with("");
        assert!(
            plain.galley.job.sections.iter().all(|s| !s.format.italics),
            "plain label unexpectedly italic"
        );
    }

    #[test]
    fn label_bold_paints_extra_glyph_pass() {
        // Bold is simulated by painting the galley twice; expect more text shapes.
        let mut plain = Control::new("LBL", ControlType::Label, 5, 7);
        plain.set_prop("Caption", PropValue::String("BOLD-RC".into()));
        let mut bold = plain.clone();
        bold.set_prop("Bold", PropValue::Bool(true));
        let n_plain = texts(&render(&plain)).iter().filter(|t| t.galley.text().contains("BOLD-RC")).count();
        let n_bold = texts(&render(&bold)).iter().filter(|t| t.galley.text().contains("BOLD-RC")).count();
        assert!(n_bold > n_plain, "Bold did not add an extra paint pass (plain={n_plain}, bold={n_bold})");
    }

    #[test]
    fn label_forecolor_is_applied() {
        let mut c = Control::new("LBL", ControlType::Label, 5, 7);
        c.set_prop("Caption", PropValue::String("RED-RC".into()));
        c.set_prop("ForegroundColor", PropValue::String("#FF0000".into()));
        let t = texts(&render(&c)).into_iter().find(|t| t.galley.text().contains("RED-RC")).expect("painted");
        let col = t.galley.job.sections.first().map(|s| s.format.color).unwrap_or(egui::Color32::TRANSPARENT);
        assert!(col.r() > 180 && col.g() < 90 && col.b() < 90, "ForeColor not applied; got {col:?}");
    }
}

// ── Behavioral render tests — Phase 2a: animations (`anim_transform`) ──────────
#[cfg(test)]
mod anim_behavior_tests {
    use super::anim_transform;
    use cobolt_forms::model::{AnimKind, AnimRepeat, AnimTrigger, AnimationDef, EasingKind};

    fn anim(kind: AnimKind) -> AnimationDef {
        AnimationDef {
            name: "a".into(),
            trigger: AnimTrigger::OnFormLoad,
            kind,
            duration_ms: 400,
            delay_ms: 0,
            easing: EasingKind::Linear, // linear → eased(t) == t, so checks are exact
            repeat: AnimRepeat::Once,
            slide_dx: 0,
            slide_dy: 0,
        }
    }

    const W: f32 = 800.0;
    const H: f32 = 600.0;

    #[test]
    fn fly_from_left_moves_into_place() {
        let a = anim(AnimKind::FlyFromLeft);
        let (dx0, dy0, sc0, al0) = anim_transform(&a, W, H, 0.0);
        let (dx1, _, _, _) = anim_transform(&a, W, H, 1.0);
        assert!((dx0 + W).abs() < 0.5, "start should be off-screen left (dx≈-W), got {dx0}");
        assert!(dy0.abs() < 0.5 && (sc0 - 1.0).abs() < 0.01 && (al0 - 1.0).abs() < 0.01);
        assert!(dx1.abs() < 0.5, "end should be in place (dx≈0), got {dx1}");
    }

    #[test]
    fn fade_in_ramps_alpha_0_to_1() {
        let a = anim(AnimKind::FadeIn);
        let (_, _, _, a0) = anim_transform(&a, W, H, 0.0);
        let (_, _, _, ah) = anim_transform(&a, W, H, 0.5);
        let (_, _, _, a1) = anim_transform(&a, W, H, 1.0);
        assert!(a0.abs() < 0.01, "fade-in start alpha≈0, got {a0}");
        assert!((ah - 0.5).abs() < 0.05, "fade-in mid alpha≈0.5, got {ah}");
        assert!((a1 - 1.0).abs() < 0.01, "fade-in end alpha≈1, got {a1}");
    }

    #[test]
    fn fade_out_ramps_alpha_1_to_0() {
        let a = anim(AnimKind::FadeOut);
        let (_, _, _, a0) = anim_transform(&a, W, H, 0.0);
        let (_, _, _, a1) = anim_transform(&a, W, H, 1.0);
        assert!((a0 - 1.0).abs() < 0.01 && a1.abs() < 0.01, "fade-out 1→0, got {a0}→{a1}");
    }

    #[test]
    fn zoom_out_elastic_is_a_damped_multi_bounce() {
        // With Elastic easing: starts 100%, dips toward ~25%, bounces 3–4 times,
        // settles 100%.
        let a = AnimationDef { easing: EasingKind::Elastic, ..anim(AnimKind::ZoomOut) };
        let s = |t: f32| anim_transform(&a, W, H, t).2;
        assert!((s(0.0) - 1.0).abs() < 0.01, "start≈100%, got {}", s(0.0));
        assert!((s(1.0) - 1.0).abs() < 0.01, "end≈100%, got {}", s(1.0));

        // First dip drops well below 100% (toward ~25%).
        let mut mn = f32::INFINITY;
        for i in 0..=200 {
            mn = mn.min(s(i as f32 / 200.0));
        }
        assert!(mn < 0.35, "should shrink toward ~25%, got {mn}");

        // Counts how often the scale crosses the 100% baseline — each crossing is
        // an over/undershoot, so several crossings ⇒ multiple bounces.
        let mut crossings = 0;
        let mut prev = (s(0.001) - 1.0).signum();
        for i in 1..=400 {
            let cur = (s(i as f32 / 400.0) - 1.0).signum();
            if cur != 0.0 && cur != prev {
                crossings += 1;
                prev = cur;
            }
        }
        assert!(crossings >= 4, "should bounce several times, got {crossings} baseline crossings");
    }

    #[test]
    fn zoom_out_non_elastic_is_a_single_dip_and_return() {
        // Linear (or any non-Elastic) easing: a single smooth dip — 100% → 25% →
        // 100% — with no overshoot above 100%.
        let a = anim(AnimKind::ZoomOut); // Linear
        let s = |t: f32| anim_transform(&a, W, H, t).2;
        assert!((s(0.0) - 1.0).abs() < 0.01, "start≈100%, got {}", s(0.0));
        assert!((s(1.0) - 1.0).abs() < 0.01, "end≈100%, got {}", s(1.0));
        assert!((s(0.5) - 0.25).abs() < 0.02, "deepest dip ≈25% at midpoint, got {}", s(0.5));
        // Never overshoots above 100%.
        for i in 0..=100 {
            assert!(s(i as f32 / 100.0) <= 1.0001, "no overshoot expected");
        }
    }

    #[test]
    fn zoom_in_ramps_scale_0_to_1() {
        // Original ZoomIn: grows from nothing to full size (with a fade-in).
        let a = anim(AnimKind::ZoomIn);
        let (_, _, s0, _) = anim_transform(&a, W, H, 0.0);
        let (_, _, s1, _) = anim_transform(&a, W, H, 1.0);
        assert!(s0 < 0.05, "zoom-in start scale≈0, got {s0}");
        assert!((s1 - 1.0).abs() < 0.01, "zoom-in end scale≈1, got {s1}");
    }

    #[test]
    fn scale_rect_shrinks_and_grows_about_centre() {
        use super::scale_rect_about_center;
        let base = egui::Rect::from_min_size(egui::pos2(100.0, 50.0), egui::vec2(200.0, 100.0));
        let centre = base.center();

        // scale == 1.0 is an exact no-op.
        assert_eq!(scale_rect_about_center(base, 1.0), base);

        // Half size, same centre.
        let half = scale_rect_about_center(base, 0.5);
        assert!((half.width()  - 100.0).abs() < 0.01);
        assert!((half.height() -  50.0).abs() < 0.01);
        assert!((half.center() - centre).length() < 0.01, "centre must be preserved");

        // Double size, same centre.
        let dbl = scale_rect_about_center(base, 2.0);
        assert!((dbl.width()  - 400.0).abs() < 0.01);
        assert!((dbl.height() - 200.0).abs() < 0.01);
        assert!((dbl.center() - centre).length() < 0.01);

        // A zoom-in at t=0 (scale≈0) collapses the rect to (almost) nothing at the centre.
        let a = anim(AnimKind::ZoomIn);
        let (_, _, s0, _) = anim_transform(&a, W, H, 0.0);
        let collapsed = scale_rect_about_center(base, s0);
        assert!(collapsed.width() < 10.0 && collapsed.height() < 10.0);
    }
}
