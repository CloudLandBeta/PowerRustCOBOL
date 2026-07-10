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
//! - AgentObject, RestClient control rendering
//! - Undo / Redo command stack

use cobolt_forms::model::{
    derive_paragraph_name, AnimKind, AnimRepeat, AnimTrigger, AnimationDef, BgImageMode,
    EasingKind, PropValue,
};
use cobolt_forms::{BindingTargetDescriptor, Control, ControlType, Form};
use egui::{Color32, CursorIcon, Pos2, Rect, Sense, Shape, Stroke, Ui, Vec2};
use std::collections::{HashMap, HashSet};

use super::properties::PropertiesPanel;
use super::toolbox::ToolboxPanel;
use crate::app::{
    refresh_data_binding_target_properties, seed_control_array_binding_preview_values,
    DesignerClipboard,
};
use crate::project_model::{UserControlDef, UserControlEntry};
use cobolt_forms::render::{card_appear_transform, PlacementEffect};

// The shared control renderer now lives in `cobolt_forms::paint` (007 T1) so the
// designer, preview, run form and compiled binaries all draw identically.
// Re-exported here so existing `designer::draw_control` call sites keep working.
pub(crate) use cobolt_forms::paint::{draw_control, parse_color};
// Used only by the behavioral render tests below (they drive these primitives
// directly); the runtime designer now goes through the engine's `render_faces`.
#[cfg(test)]
use cobolt_forms::paint::{draw_animator, live_control, scale_rect_about_center, text_halign};

/// `FormState` for the design-time canvas (spec 017 T6): the designed form IS the
/// source of truth (no live overrides), every control is shown (design-time),
/// disabled controls aren't dimmed, and the only transform is an in-progress
/// animation preview (supplied per control via `anim`).
struct DesignerState<'a> {
    anim: &'a std::collections::HashMap<String, cobolt_forms::render::RenderTransform>,
}
impl cobolt_forms::render::FormState for DesignerState<'_> {
    fn visible(&self, _base: &cobolt_forms::Control) -> bool {
        true
    }
    fn enabled(&self, _base: &cobolt_forms::Control) -> bool {
        true
    }
    fn transform(&self, base: &cobolt_forms::Control) -> cobolt_forms::render::RenderTransform {
        self.anim
            .get(&base.id)
            .copied()
            .unwrap_or(cobolt_forms::render::RenderTransform::IDENTITY)
    }
}

#[derive(Default)]
pub(crate) struct DesignerShowResult {
    pub(crate) selection_changed: bool,
    pub(crate) user_control_created: Option<UserControlDef>,
    pub(crate) user_control_delete_requested: Option<String>,
}

#[derive(Clone, Debug)]
struct UserControlCreateDialog {
    group_id: String,
    name: String,
    error: Option<UserControlNameError>,
}

#[derive(Clone, Copy, Debug)]
enum UserControlNameError {
    Empty,
    Invalid,
    Duplicate,
    Circular,
}

// ── Grid ──────────────────────────────────────────────────────────────────────
/// Snap `v` to the nearest multiple of `grid_px` (only when snap is enabled).
fn snap(v: i32, grid_px: i32, enabled: bool) -> i32 {
    if enabled && grid_px > 0 {
        (v / grid_px) * grid_px
    } else {
        v
    }
}

// ── Animation preview state ───────────────────────────────────────────────────

/// Live animation state used for designer preview only.
pub(crate) struct AnimState {
    /// Animation name being played.
    pub(crate) name: String,
    /// Progress 0.0 → 1.0.
    pub(crate) t: f32,
    /// Is the preview playing?
    pub(crate) playing: bool,
    /// True = forward, false = reverse (for PingPong).
    pub(crate) forward: bool,
    /// How many full loops completed.
    pub(crate) loops: u32,
    /// Seconds of delay still to wait before `t` starts advancing.
    pub(crate) delay_remaining: f32,
}

impl AnimState {
    pub(crate) fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            t: 0.0,
            playing: false,
            forward: true,
            loops: 0,
            delay_remaining: 0.0,
        }
    }
    pub(crate) fn play(&mut self, delay_secs: f32) {
        self.t = 0.0;
        self.playing = true;
        self.forward = true;
        self.loops = 0;
        self.delay_remaining = delay_secs.max(0.0);
    }
    pub(crate) fn stop(&mut self) {
        self.playing = false;
        self.t = 1.0;
    }
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
pub(crate) fn anim_transform(
    anim: &AnimationDef,
    form_w: f32,
    form_h: f32,
    t: f32,
) -> (f32, f32, f32, f32) {
    let te = anim.easing.apply(t); // eased progress
    let inv = 1.0 - te;
    match &anim.kind {
        AnimKind::FlyFromLeft => (-form_w * inv, 0.0, 1.0, 1.0),
        AnimKind::FlyFromRight => (form_w * inv, 0.0, 1.0, 1.0),
        AnimKind::FlyFromTop => (0.0, -form_h * inv, 1.0, 1.0),
        AnimKind::FlyFromBottom => (0.0, form_h * inv, 1.0, 1.0),
        AnimKind::FlyFromTopLeft => (-form_w * inv, -form_h * inv, 1.0, 1.0),
        AnimKind::FlyFromTopRight => (form_w * inv, -form_h * inv, 1.0, 1.0),
        AnimKind::FlyFromBottomLeft => (-form_w * inv, form_h * inv, 1.0, 1.0),
        AnimKind::FlyFromBottomRight => (form_w * inv, form_h * inv, 1.0, 1.0),
        AnimKind::FadeIn => (0.0, 0.0, 1.0, te),
        AnimKind::FadeOut => (0.0, 0.0, 1.0, 1.0 - te),
        // ZoomIn grows 0 → 100% (eased; Elastic overshoots past 100% and settles).
        AnimKind::ZoomIn => (0.0, 0.0, te.max(0.001), te),
        // ZoomOut dips and returns: 100% → 25% → 100%. With Elastic easing this
        // becomes a damped multi-bounce (overshoots 3–4 times before settling).
        AnimKind::ZoomOut => {
            let scale = if matches!(anim.easing, EasingKind::Elastic) {
                zoomout_scale(t)
            } else {
                // Smooth single dip-and-return (no overshoot), timed by the easing.
                (1.0 - 0.75 * (std::f32::consts::PI * te).sin()).max(0.02)
            };
            (0.0, 0.0, scale, 1.0)
        }
        AnimKind::Bounce => {
            let dy = -50.0 * (std::f32::consts::PI * t * 3.0).sin().abs() * inv;
            (0.0, dy, 1.0, 1.0)
        }
        AnimKind::Shake => {
            let dx = 6.0 * (t * std::f32::consts::TAU * 5.0).sin() * inv;
            (dx, 0.0, 1.0, 1.0)
        }
        AnimKind::Pulse => {
            let s = 1.0 + 0.15 * (t * std::f32::consts::TAU * 2.0).sin() * inv;
            (0.0, 0.0, s, 1.0)
        }
        AnimKind::Slide { dx, dy } => ((*dx as f32) * inv, (*dy as f32) * inv, 1.0, 1.0),
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
        AnimKind::None | AnimKind::Custom(_) => (0.0, 0.0, 1.0, 1.0),
    }
}

// ── Undo / Redo command ───────────────────────────────────────────────────────

#[derive(Clone)]
enum Cmd {
    AddControl {
        index: usize,
        ctrl: Control,
    },
    DeleteControl {
        index: usize,
        ctrl: Control,
        deleted_at: String,
    },
    MoveControl {
        id: String,
        old_x: i32,
        old_y: i32,
        new_x: i32,
        new_y: i32,
    },
    MoveMany {
        moves: Vec<(String, i32, i32, i32, i32)>,
    }, // id, ox, oy, nx, ny
    ResizeControl {
        id: String,
        old_rect: cobolt_forms::model::Rect,
        new_rect: cobolt_forms::model::Rect,
    },
    SetProperty {
        id: String,
        key: String,
        old: Option<PropValue>,
        new: PropValue,
    },
    ReorderControl {
        from: usize,
        to: usize,
    },
    SetZOrder {
        id: String,
        old_z: i32,
        new_z: i32,
    },
    /// Move a control into a different container (or the form) — spec 012.
    Reparent {
        id: String,
        old_parent: Option<String>,
        old_tab: Option<u32>,
        new_parent: Option<String>,
        new_tab: Option<u32>,
    },
    /// Rename a control's id (updates all references form-wide).
    Rename {
        old: String,
        new: String,
    },
    /// Set (or create) the COBOL code of a control's event handler (spec 025).
    /// `old` is `None` when the binding did not exist before.
    SetEventCode {
        control_id: String,
        event: String,
        old: Option<String>,
        new: String,
    },
    /// Add or replace a common procedure's body (spec 025). `old` is `None` when no
    /// procedure of that name existed before.
    SetProcedure {
        name: String,
        old: Option<String>,
        new: String,
    },
    /// A batch of commands applied and reverted as **one** undoable step — the unit
    /// an approved agent change-set becomes (spec 025 R6).
    AgentBatch {
        cmds: Vec<Cmd>,
    },
}

// ── Resize handle ─────────────────────────────────────────────────────────────

#[derive(Clone, Copy, PartialEq, Debug)]
enum Handle {
    TopLeft,
    Top,
    TopRight,
    Left,
    Right,
    BotLeft,
    Bot,
    BotRight,
}

const ALL_HANDLES: [Handle; 8] = [
    Handle::TopLeft,
    Handle::Top,
    Handle::TopRight,
    Handle::Left,
    Handle::Right,
    Handle::BotLeft,
    Handle::Bot,
    Handle::BotRight,
];

fn handle_pos(r: &cobolt_forms::model::Rect, h: Handle) -> Pos2 {
    let (x, y, w, hh) = (r.x as f32, r.y as f32, r.w as f32, r.h as f32);
    match h {
        Handle::TopLeft => Pos2::new(x, y),
        Handle::Top => Pos2::new(x + w / 2.0, y),
        Handle::TopRight => Pos2::new(x + w, y),
        Handle::Left => Pos2::new(x, y + hh / 2.0),
        Handle::Right => Pos2::new(x + w, y + hh / 2.0),
        Handle::BotLeft => Pos2::new(x, y + hh),
        Handle::Bot => Pos2::new(x + w / 2.0, y + hh),
        Handle::BotRight => Pos2::new(x + w, y + hh),
    }
}

fn handle_cursor(h: Handle) -> CursorIcon {
    match h {
        Handle::TopLeft | Handle::BotRight => CursorIcon::ResizeNwSe,
        Handle::TopRight | Handle::BotLeft => CursorIcon::ResizeNeSw,
        Handle::Top | Handle::Bot => CursorIcon::ResizeVertical,
        Handle::Left | Handle::Right => CursorIcon::ResizeHorizontal,
    }
}

fn apply_resize(
    r: cobolt_forms::model::Rect,
    h: Handle,
    dx: i32,
    dy: i32,
    grid_px: i32,
    snapping: bool,
) -> cobolt_forms::model::Rect {
    let s = |v| snap(v, grid_px, snapping);
    let mut nr = r;
    match h {
        Handle::TopLeft => {
            nr.x = s(r.x + dx);
            nr.y = s(r.y + dy);
            nr.w = (r.w - dx).max(8);
            nr.h = (r.h - dy).max(8);
        }
        Handle::Top => {
            nr.y = s(r.y + dy);
            nr.h = (r.h - dy).max(8);
        }
        Handle::TopRight => {
            nr.y = s(r.y + dy);
            nr.w = s(r.w + dx).max(8);
            nr.h = (r.h - dy).max(8);
        }
        Handle::Left => {
            nr.x = s(r.x + dx);
            nr.w = (r.w - dx).max(8);
        }
        Handle::Right => {
            nr.w = s(r.w + dx).max(8);
        }
        Handle::BotLeft => {
            nr.x = s(r.x + dx);
            nr.w = (r.w - dx).max(8);
            nr.h = s(r.h + dy).max(8);
        }
        Handle::Bot => {
            nr.h = s(r.h + dy).max(8);
        }
        Handle::BotRight => {
            nr.w = s(r.w + dx).max(8);
            nr.h = s(r.h + dy).max(8);
        }
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
        id: String,
        handle: Handle,
        orig_rect: cobolt_forms::model::Rect,
        start_x: i32,
        start_y: i32,
    },
    PlacingNew {
        ctrl_type: ControlType,
        start_x: i32,
        start_y: i32,
        cur_x: i32,
        cur_y: i32,
    },
    /// Rubber-band lasso selection.
    RubberBand {
        start_x: i32,
        start_y: i32,
        cur_x: i32,
        cur_y: i32,
    },
    /// Resizing the form canvas itself by dragging its right/bottom/corner edge.
    ResizingForm {
        edge: FormEdge,
        orig_w: i32,
        orig_h: i32,
        start_x: i32,
        start_y: i32,
    },
}

/// Which edge of the form canvas is being dragged to resize it.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum FormEdge {
    Right,
    Bottom,
    Corner,
}

#[derive(Clone, Debug)]
struct DeleteConfirmation {
    control_ids: Vec<String>,
    control_count: usize,
    event_count: usize,
}

/// Half-width (px) of the grab band along the form's right/bottom border.
const FORM_EDGE_GRAB: f32 = 7.0;

/// Smallest form dimension allowed when resizing by drag (matches `set_form_prop`).
const FORM_MIN_SIZE: i32 = 64;

/// Detect whether the canvas-space pointer `(px, py)` is over the form's resize
/// border, given the form size `(w, h)`. Returns the edge, or `None`.
fn detect_form_edge(px: i32, py: i32, w: f32, h: f32) -> Option<FormEdge> {
    let (px, py) = (px as f32, py as f32);
    let near_right =
        (px - w).abs() <= FORM_EDGE_GRAB && py >= -FORM_EDGE_GRAB && py <= h + FORM_EDGE_GRAB;
    let near_bottom =
        (py - h).abs() <= FORM_EDGE_GRAB && px >= -FORM_EDGE_GRAB && px <= w + FORM_EDGE_GRAB;
    match (near_right, near_bottom) {
        (true, true) => Some(FormEdge::Corner),
        (true, false) => Some(FormEdge::Right),
        (false, true) => Some(FormEdge::Bottom),
        _ => None,
    }
}

fn form_edge_cursor(e: FormEdge) -> CursorIcon {
    match e {
        FormEdge::Right => CursorIcon::ResizeHorizontal,
        FormEdge::Bottom => CursorIcon::ResizeVertical,
        FormEdge::Corner => CursorIcon::ResizeNwSe,
    }
}

// ── Format Painter ────────────────────────────────────────────────────────────

/// Visual style properties that can be copied between controls.
const STYLE_PROP_KEYS: &[&str] = &[
    "BackgroundColor",
    "ForegroundColor",
    "BorderColor",
    "FontSize",
    "Bold",
    "Italic",
    "Underline",
    "Strikethrough",
    "FontName",
    "Opacity",
    "CornerRadius",
    "BorderWidth",
    "BorderStyle",
    "HeaderBackgroundColor",
    "HeaderForegroundColor",
    "AlternatingRowColor",
    "AlternatingRowOpacity",
    "AlternatingMode",
    "GridLineColor",
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
        props: std::collections::HashMap<String, cobolt_forms::model::PropValue>,
        animations: Vec<AnimationDef>,
        src_rect: cobolt_forms::model::Rect,
    },
}

// ── Menu Editor Modal (spec 018) ──────────────────────────────────────────────

pub struct MenuEditorModal {
    pub ctrl_id: String,
    pub def: cobolt_forms::menu::MenuDefinition,
    /// Path of `[index_in_parent, …]` to the currently selected node.
    pub selected: Vec<usize>,
    /// Scratch text buffers for the detail form fields.
    label_buf: String,
    accel_buf: String,
    target_buf: String,
    /// Icon picker modal state
    icon_picker_open: bool,
    icon_search: String,
    /// Generation counter for icon picker window ID (forces fresh egui state on reopen)
    icon_picker_gen: u32,
    /// Horizontal split ratio between Menu Items and Item Properties panes (0.0–1.0)
    split_ratio: f32,
}

impl MenuEditorModal {
    pub fn new(ctrl_id: String, def: cobolt_forms::menu::MenuDefinition) -> Self {
        Self {
            ctrl_id,
            def,
            selected: Vec::new(),
            label_buf: String::new(),
            accel_buf: String::new(),
            target_buf: String::new(),
            icon_picker_open: false,
            icon_search: String::new(),
            icon_picker_gen: 0,
            split_ratio: 0.40,
        }
    }

    fn selected_item(&self) -> Option<&cobolt_forms::menu::MenuItem> {
        Self::item_at(&self.def.menu, &self.selected)
    }

    fn selected_item_mut(&mut self) -> Option<&mut cobolt_forms::menu::MenuItem> {
        Self::item_at_mut(&mut self.def.menu, &self.selected)
    }

    fn item_at<'a>(
        items: &'a [cobolt_forms::menu::MenuItem],
        path: &[usize],
    ) -> Option<&'a cobolt_forms::menu::MenuItem> {
        if path.is_empty() {
            return None;
        }
        let item = items.get(path[0])?;
        if path.len() == 1 {
            Some(item)
        } else {
            Self::item_at(&item.items, &path[1..])
        }
    }

    fn item_at_mut<'a>(
        items: &'a mut [cobolt_forms::menu::MenuItem],
        path: &[usize],
    ) -> Option<&'a mut cobolt_forms::menu::MenuItem> {
        if path.is_empty() {
            return None;
        }
        let item = items.get_mut(path[0])?;
        if path.len() == 1 {
            Some(item)
        } else {
            Self::item_at_mut(&mut item.items, &path[1..])
        }
    }

    fn parent_list_mut<'a>(
        def: &'a mut cobolt_forms::menu::MenuDefinition,
        path: &[usize],
    ) -> &'a mut Vec<cobolt_forms::menu::MenuItem> {
        if path.len() <= 1 {
            return &mut def.menu;
        }
        let mut items = &mut def.menu;
        for &idx in &path[..path.len() - 1] {
            items = &mut items[idx].items;
        }
        items
    }

    fn depth_of(path: &[usize]) -> usize {
        path.len()
    }

    fn next_id(&self) -> String {
        fn count_all(items: &[cobolt_forms::menu::MenuItem]) -> usize {
            items.iter().map(|i| 1 + count_all(&i.items)).sum()
        }
        let n = count_all(&self.def.menu);
        format!("item-{}", n + 1)
    }

    fn sync_bufs_from_selection(&mut self) {
        if let Some(item) = Self::item_at(&self.def.menu, &self.selected) {
            self.label_buf = item.label.clone();
            self.accel_buf = item.accelerator.clone().unwrap_or_default();
            self.target_buf = match &item.action {
                Some(a) => {
                    if let Some(rest) = a.strip_prefix("open-form:") {
                        rest.to_string()
                    } else if let Some(rest) = a.strip_prefix("property:") {
                        rest.to_string()
                    } else {
                        String::new()
                    }
                }
                None => String::new(),
            };
        }
    }

    fn action_type_of(item: &cobolt_forms::menu::MenuItem) -> &'static str {
        match item.action.as_deref() {
            Some("close-application") => "close",
            Some("event") | None => "event",
            Some(a) if a.starts_with("open-form:") => "open-form",
            _ => "event",
        }
    }
}

// ── Event Editor Modal ────────────────────────────────────────────────────────

/// State for the modal COBOL code editor that pops up when
/// the user clicks an event row in the Properties panel.
pub struct EventEditorModal {
    /// Control ID whose event is being edited (empty string = form-level event).
    pub ctrl_id: String,
    /// Human-readable display name for the title bar (e.g. "BTN-OK · Click").
    pub ctrl_display: String,
    /// Event name, e.g. "Click".
    pub event_name: String,
    /// The nested PROGRAM-ID that will be emitted for this handler.
    pub program_id: String,
    /// The handler source when the modal opened — used to detect real changes so
    /// an untouched first-time template is not persisted as handler code. (The
    /// live, editable text lives in the hosted `event_editor`.)
    orig_source: String,
    /// Draft text in the modal's AI prompt box (empty ⇒ nothing typed).
    ai_prompt: String,
    /// In-flight AI request for this handler; `Some` while the model is thinking.
    ai_pending: Option<std::sync::mpsc::Receiver<crate::llm::LlmResponse>>,
    /// Last AI error to surface below the prompt row (`None` ⇒ no error).
    ai_status: Option<String>,
}

impl EventEditorModal {
    pub fn new(
        ctrl_id: impl Into<String>,
        ctrl_display: impl Into<String>,
        event_name: impl Into<String>,
        program_id: impl Into<String>,
        source: impl Into<String>,
    ) -> Self {
        Self {
            ctrl_id: ctrl_id.into(),
            ctrl_display: ctrl_display.into(),
            event_name: event_name.into(),
            program_id: program_id.into(),
            orig_source: source.into(),
            ai_prompt: String::new(),
            ai_pending: None,
            ai_status: None,
        }
    }
}

// ── DesignerPanel ─────────────────────────────────────────────────────────────

pub struct DesignerPanel {
    pub form: Form,
    /// Directory containing the .cfrm file (for loading menu YAML files etc.).
    pub cfrm_dir: Option<std::path::PathBuf>,

    /// All currently selected control IDs (first = primary selection).
    pub selected_ids: Vec<String>,

    /// Design-time active tab page per `TabControl` id (spec 012). Set when the
    /// user clicks a tab to edit its page; absent ⇒ fall back to `SelectedTab`.
    active_tabs: std::collections::HashMap<String, u32>,

    /// Per-container scroll offset (spec 012) when `AutoScroll` is on.
    scroll_offsets: std::collections::HashMap<String, egui::Vec2>,

    drag: DragState,

    undo_stack: Vec<Cmd>,
    redo_stack: Vec<Cmd>,

    pub dirty: bool,
    pub close_requested: bool,
    /// Set when the user tries to close a dirty designer — shows the Save/Discard/Cancel dialog.
    pub close_confirm: bool,
    pending_delete: Option<DeleteConfirmation>,
    create_user_control: Option<UserControlCreateDialog>,

    /// Format-painter (copy-style) state.
    pub(crate) format_painter: FormatPainter,

    pub toolbox: ToolboxPanel,
    pub properties: PropertiesPanel,

    // ── UI options ────────────────────────────────────────────────────────────
    pub show_grid: bool,
    pub glass_mode: bool,

    // ── Animation preview ─────────────────────────────────────────────────────
    /// ctrl_id → AnimState (for designer-time preview of animations)
    anim_states: HashMap<String, AnimState>,
    /// Elapsed time from last frame for animation stepping.
    last_frame_time: Option<std::time::Instant>,

    // ── Image preview cache ───────────────────────────────────────────────────
    /// Maps absolute image path → loaded egui texture handle.
    /// `None` means the path was tried but failed to load.
    pub(crate) image_cache: HashMap<String, Option<egui::TextureHandle>>,

    /// Resolved asset-pack theme for the current form (spec 007). `None` =
    /// procedural Liquid Glass. Set by the app each frame from the project
    /// default + the per-form override; consumed by the canvas (and preview)
    /// draw loops via `cobolt_forms::paint::set_active_theme`.
    pub active_theme_pack: Option<std::sync::Arc<cobolt_forms::theme_pack::ThemePack>>,

    /// The font the user most recently set on a control in this form. New controls
    /// inherit it so a form keeps a consistent typeface.
    last_font_name: Option<String>,
    last_font_size: Option<i64>,

    // ── Resize handle press capture ───────────────────────────────────────────
    /// Stores which resize handle the pointer was on when the mouse button was
    /// first pressed.  Consumed on `drag_started()` so the drag-start check
    /// doesn't have to re-test the (now moved) pointer against the small handle.
    press_handle: Option<Handle>,
    /// Stores which form edge (if any) the pointer was on when the mouse button
    /// was first pressed, so the form-resize drag can begin on `drag_started()`.
    press_form_edge: Option<FormEdge>,

    // ── Menu editor modal (spec 018) ────────────────────────────────────────
    pub menu_modal: Option<MenuEditorModal>,

    // ── Event editor modal ────────────────────────────────────────────────────
    /// When `Some`, a modal COBOL code editor is displayed over the canvas.
    pub event_modal: Option<EventEditorModal>,
    /// The full-featured COBOL editor hosted inside the event modal (IntelliSense,
    /// find/replace, status bar) — the same engine as the main code editor.
    event_editor: super::editor::EditorPanel,
    /// The same hosted COBOL editor for the COBOL Structure popup (spec 005), so
    /// section / procedure code gets IntelliSense too. `cs_loaded` is the block
    /// currently in its buffer (reloaded only when the selection changes).
    cs_editor: super::editor::EditorPanel,
    cs_loaded: Option<super::cobol_structure::CsTarget>,

    // ── Form preview ──────────────────────────────────────────────────────────
    /// Whether the live preview viewport is open.
    pub show_preview: bool,
    /// Which COBOL Structure block the popup editor is editing (None = closed; spec 005).
    pub cobol_structure_edit: Option<super::cobol_structure::CsTarget>,
    /// Runtime state for preview: maps ctrl_id → current value (for interactive controls).
    pub preview_state: HashMap<String, String>,
    /// Animation states for the live preview (separate from designer preview).
    pub preview_anim_states: HashMap<String, AnimState>,
    /// Last frame time for the live preview animation ticker.
    pub preview_last_frame: Option<std::time::Instant>,
    /// Tracks which ComboBox (by control ID) is currently open in the preview.
    pub(crate) preview_combo_open: HashMap<String, bool>,
    /// Designer-only clock for repeating GroupBox placement effects. It is reset
    /// on mouse release so the elastic card placement starts from the committed
    /// final layout, not while the user is still dragging.
    placement_release_starts: HashMap<String, f64>,
}

impl DesignerPanel {
    pub fn new(form: Form) -> Self {
        Self {
            form,
            cfrm_dir: None,
            selected_ids: Vec::new(),
            active_tabs: std::collections::HashMap::new(),
            scroll_offsets: std::collections::HashMap::new(),
            drag: DragState::None,
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            dirty: false,
            close_requested: false,
            close_confirm: false,
            pending_delete: None,
            create_user_control: None,
            toolbox: ToolboxPanel::new(),
            properties: PropertiesPanel::new(),
            show_grid: true,
            glass_mode: true,
            anim_states: HashMap::new(),
            last_frame_time: None,
            format_painter: FormatPainter::Idle,
            image_cache: HashMap::new(),
            last_font_name: None,
            last_font_size: None,
            press_handle: None,
            press_form_edge: None,
            menu_modal: None,
            event_modal: None,
            event_editor: super::editor::EditorPanel::new(),
            cs_editor: super::editor::EditorPanel::new(),
            cs_loaded: None,
            show_preview: false,
            cobol_structure_edit: None,
            preview_state: HashMap::new(),
            preview_anim_states: HashMap::new(),
            preview_last_frame: None,
            preview_combo_open: HashMap::new(),
            active_theme_pack: None,
            placement_release_starts: HashMap::new(),
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

    fn repeating_group_placement_effect(ctrl: &Control) -> PlacementEffect {
        if !matches!(ctrl.control_type, ControlType::GroupBox) {
            return PlacementEffect::None;
        }
        if !ctrl
            .get_prop("IsRepeatingGroup")
            .map(|v| v.as_bool())
            .unwrap_or(false)
        {
            return PlacementEffect::None;
        }
        ctrl.get_prop("PlacementEffect")
            .map(|v| PlacementEffect::parse(v.as_str()))
            .unwrap_or(PlacementEffect::None)
    }

    fn trigger_repeating_group_placement_release(
        &mut self,
        ctx: &egui::Context,
        changed_ids: &[String],
    ) {
        let now = ctx.input(|i| i.time);
        let mut group_ids = Vec::new();
        for id in changed_ids {
            let mut cur = self.form.controls.iter().position(|c| c.id == *id);
            while let Some(idx) = cur {
                let ctrl = &self.form.controls[idx];
                if Self::repeating_group_placement_effect(ctrl) != PlacementEffect::None {
                    if !group_ids.iter().any(|gid: &String| gid == &ctrl.id) {
                        group_ids.push(ctrl.id.clone());
                    }
                    break;
                }
                cur = ctrl
                    .parent
                    .as_ref()
                    .and_then(|pid| self.form.controls.iter().position(|c| c.id == *pid));
            }
        }
        if group_ids.is_empty() {
            return;
        }
        for group_id in group_ids {
            self.placement_release_starts.insert(group_id, now);
        }
        ctx.request_repaint();
    }

    /// Load an image from disk and register it as an egui texture.
    /// Returns `Some(handle)` on success, `None` on any error.
    /// Results are cached by path so each file is read at most once per session.
    pub(crate) fn load_image(
        &mut self,
        path: &str,
        ctx: &egui::Context,
    ) -> Option<&egui::TextureHandle> {
        if !self.image_cache.contains_key(path) {
            let result: Option<egui::TextureHandle> =
                (|| cobolt_forms::paint::load_image_texture(ctx, path))();
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
        if let Some(id) = id {
            self.selected_ids.push(id);
        }
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

    /// Apply an approved agent change-set as **one** undoable action (spec 025
    /// R6/R7). Invalid ops (per `agent::validate`) are skipped. Returns the number
    /// of operations applied. Nothing is pushed when the change-set is all-invalid
    /// or empty.
    pub fn apply_agent_change_set(&mut self, cs: &crate::agent::AgentChangeSet) -> usize {
        use crate::agent::AgentOp;
        let status = crate::agent::validate(cs, &self.form);
        let mut cmds: Vec<Cmd> = Vec::new();
        let mut reserved: HashSet<String> =
            self.form.controls.iter().map(|c| c.id.clone()).collect();
        let mut added = 0usize;

        for (op, err) in cs.operations.iter().zip(status.iter()) {
            if err.is_some() {
                continue; // R9 — invalid ops are shown in the preview, never applied
            }
            match op {
                AgentOp::DeployControl {
                    control_type,
                    id,
                    properties,
                } => {
                    let ct = ControlType::from_str(control_type);
                    let cid = id
                        .clone()
                        .filter(|s| !s.trim().is_empty() && !reserved.contains(s))
                        .unwrap_or_else(|| self.next_unique_id_reserved(&ct, &reserved));
                    reserved.insert(cid.clone());
                    // Geometry: honour X/Y/Width/Height when given, else stagger a
                    // sensible default so the developer can rearrange it (R13).
                    let gx = json_prop_i32(properties, "X").unwrap_or(20);
                    let gy = json_prop_i32(properties, "Y")
                        .unwrap_or(20 + 28 * (self.form.controls.len() + added) as i32);
                    let mut c = Control::new(cid.clone(), ct.clone(), gx, gy);
                    if let Some(w) = json_prop_i32(properties, "Width") {
                        c.rect.w = w;
                    }
                    if let Some(h) = json_prop_i32(properties, "Height") {
                        c.rect.h = h;
                    }
                    for (k, v) in properties {
                        if matches!(k.as_str(), "X" | "Y" | "Width" | "Height") {
                            continue;
                        }
                        if let Some(pv) = json_to_prop(v) {
                            apply_structural_prop(&mut c, k, &pv);
                        }
                    }
                    cmds.push(Cmd::AddControl {
                        index: self.form.controls.len() + added,
                        ctrl: c,
                    });
                    added += 1;
                }
                AgentOp::SetProperty {
                    control_id,
                    key,
                    value,
                } => {
                    let Some(pv) = json_to_prop(value) else {
                        continue;
                    };
                    let old = self
                        .form
                        .find_control(control_id)
                        .and_then(|c| c.properties.get(key).cloned());
                    cmds.push(Cmd::SetProperty {
                        id: control_id.clone(),
                        key: key.clone(),
                        old,
                        new: pv,
                    });
                }
                AgentOp::GenerateEventHandler {
                    control_id,
                    event,
                    code,
                } => {
                    let old = self.form.find_control(control_id).and_then(|c| {
                        c.events
                            .iter()
                            .find(|b| b.event.eq_ignore_ascii_case(event))
                            .map(|b| b.code.clone())
                    });
                    cmds.push(Cmd::SetEventCode {
                        control_id: control_id.clone(),
                        event: event.clone(),
                        old,
                        new: code.clone(),
                    });
                }
                AgentOp::CreateProcedure { name, code } => {
                    let old = self
                        .form
                        .user_procedures
                        .iter()
                        .find(|p| p.name.eq_ignore_ascii_case(name))
                        .map(|p| p.code.clone());
                    cmds.push(Cmd::SetProcedure {
                        name: name.clone(),
                        old,
                        new: code.clone(),
                    });
                }
            }
        }

        let n = cmds.len();
        if n > 0 {
            self.apply(Cmd::AgentBatch { cmds });
        }
        n
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
            Cmd::DeleteControl {
                index, deleted_at, ..
            } => {
                if *index < self.form.controls.len() {
                    let id = self.form.controls[*index].id.clone();
                    self.form.recycle_control(&id, deleted_at.clone());
                }
            }
            Cmd::MoveControl {
                id, new_x, new_y, ..
            } => {
                if let Some(c) = self.form.find_control_mut(id) {
                    c.rect.x = *new_x;
                    c.rect.y = *new_y;
                }
            }
            Cmd::MoveMany { moves } => {
                for (id, _, _, nx, ny) in moves {
                    if let Some(c) = self.form.find_control_mut(id) {
                        c.rect.x = *nx;
                        c.rect.y = *ny;
                    }
                }
            }
            Cmd::ResizeControl { id, new_rect, .. } => {
                if let Some(c) = self.form.find_control_mut(id) {
                    c.rect = *new_rect;
                }
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
                if let Some(c) = self.form.find_control_mut(id) {
                    c.z_order = *new_z;
                }
            }
            Cmd::Reparent {
                id,
                new_parent,
                new_tab,
                ..
            } => {
                if let Some(c) = self.form.find_control_mut(id) {
                    c.parent = new_parent.clone();
                    c.tab = *new_tab;
                }
            }
            Cmd::Rename { old, new } => {
                self.form.rename_control(old, new);
                self.retarget_selection(old, new);
            }
            Cmd::SetEventCode {
                control_id,
                event,
                new,
                ..
            } => {
                set_control_event_code(&mut self.form, control_id, event, Some(new.clone()));
            }
            Cmd::SetProcedure { name, new, .. } => {
                set_form_procedure(&mut self.form, name, Some(new.clone()));
            }
            Cmd::AgentBatch { cmds } => {
                for c in cmds {
                    self.execute(c);
                }
            }
        }
    }

    /// Point any selected id at its renamed replacement.
    fn retarget_selection(&mut self, from: &str, to: &str) {
        for s in &mut self.selected_ids {
            if s.eq_ignore_ascii_case(from) {
                *s = to.to_owned();
            }
        }
    }

    fn reverse(&mut self, cmd: &Cmd) {
        match cmd {
            Cmd::AddControl { index, .. } => {
                if *index < self.form.controls.len() {
                    self.form.controls.remove(*index);
                }
            }
            Cmd::DeleteControl {
                index,
                ctrl,
                deleted_at,
            } => {
                self.form.deleted_code.retain(|deleted| {
                    !(deleted.control_id.eq_ignore_ascii_case(&ctrl.id)
                        && deleted.deleted_at == *deleted_at)
                });
                let idx = (*index).min(self.form.controls.len());
                self.form.controls.insert(idx, ctrl.clone());
            }
            Cmd::MoveControl {
                id, old_x, old_y, ..
            } => {
                if let Some(c) = self.form.find_control_mut(id) {
                    c.rect.x = *old_x;
                    c.rect.y = *old_y;
                }
            }
            Cmd::MoveMany { moves } => {
                for (id, ox, oy, _, _) in moves {
                    if let Some(c) = self.form.find_control_mut(id) {
                        c.rect.x = *ox;
                        c.rect.y = *oy;
                    }
                }
            }
            Cmd::ResizeControl { id, old_rect, .. } => {
                if let Some(c) = self.form.find_control_mut(id) {
                    c.rect = *old_rect;
                }
            }
            Cmd::SetProperty { id, key, old, .. } => {
                if let Some(c) = self.form.find_control_mut(id) {
                    if let Some(v) = old {
                        apply_structural_prop(c, key, v);
                    } else {
                        c.properties.swap_remove(key);
                    }
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
                if let Some(c) = self.form.find_control_mut(id) {
                    c.z_order = *old_z;
                }
            }
            Cmd::Reparent {
                id,
                old_parent,
                old_tab,
                ..
            } => {
                if let Some(c) = self.form.find_control_mut(id) {
                    c.parent = old_parent.clone();
                    c.tab = *old_tab;
                }
            }
            Cmd::Rename { old, new } => {
                self.form.rename_control(new, old);
                self.retarget_selection(new, old);
            }
            Cmd::SetEventCode {
                control_id,
                event,
                old,
                ..
            } => {
                set_control_event_code(&mut self.form, control_id, event, old.clone());
            }
            Cmd::SetProcedure { name, old, .. } => {
                set_form_procedure(&mut self.form, name, old.clone());
            }
            Cmd::AgentBatch { cmds } => {
                for c in cmds.iter().rev() {
                    self.reverse(c);
                }
            }
        }
    }

    /// Rename a control's id form-wide (undoable). Returns `false` if the new id
    /// is invalid or already taken.
    pub fn rename_control(&mut self, old: &str, new: &str) -> bool {
        if !self.form.rename_control(old, new) {
            return false;
        }
        self.retarget_selection(old, new);
        self.undo_stack.push(Cmd::Rename {
            old: old.to_owned(),
            new: new.to_owned(),
        });
        self.redo_stack.clear();
        self.dirty = true;
        true
    }

    // ── Control manipulation ──────────────────────────────────────────────────

    // (see `control_type_name` free function below for the prefix.)

    /// A fresh, readable control ID: `<TypeName>-<n>` with a **per-type** counter
    /// (e.g. `Button-1`, `Button-2`, `TextBox-1`). `n` is one past the highest
    /// existing number for that type, so IDs stay unique and gap-free.
    fn next_unique_id(&self, ct: &ControlType) -> String {
        let prefix = control_type_name(ct);
        fn scan(ctrls: &[Control], prefix: &str, max_n: &mut u32) {
            for c in ctrls {
                if let Some(num) = c.id.strip_prefix(prefix).and_then(|r| r.strip_prefix('-')) {
                    if let Ok(n) = num.parse::<u32>() {
                        *max_n = (*max_n).max(n);
                    }
                }
                scan(&c.children, prefix, max_n);
            }
        }
        let mut max_n = 0u32;
        scan(&self.form.controls, prefix, &mut max_n);
        format!("{prefix}-{}", max_n + 1)
    }

    fn next_unique_id_reserved(&self, ct: &ControlType, reserved: &HashSet<String>) -> String {
        let prefix = control_type_name(ct);
        let mut max_n = 0u32;
        for c in &self.form.controls {
            if let Some(num) = c.id.strip_prefix(prefix).and_then(|r| r.strip_prefix('-')) {
                if let Ok(n) = num.parse::<u32>() {
                    max_n = max_n.max(n);
                }
            }
        }
        for id in reserved {
            if let Some(num) = id.strip_prefix(prefix).and_then(|r| r.strip_prefix('-')) {
                if let Ok(n) = num.parse::<u32>() {
                    max_n = max_n.max(n);
                }
            }
        }
        format!("{prefix}-{}", max_n + 1)
    }

    /// Re-parent `id` after a move so it belongs to whatever container its body
    /// now sits over (spec 012 R7–R10). The drop point is the moved control's
    /// centre; `resolve_drop_target` excludes the control and its descendants
    /// (cycle guard). No-op when the parent/tab is unchanged.
    fn reparent_to_drop(&mut self, id: &str) {
        let Some(idx) = self.form.controls.iter().position(|c| c.id == id) else {
            return;
        };
        let r = self.form.controls[idx].rect;
        let (px, py) = (r.x + r.w / 2, r.y + r.h / 2);
        let target = super::containers::resolve_drop_target(
            &self.form.controls,
            px,
            py,
            idx,
            &self.active_tabs,
        );
        let (new_parent, new_tab) = match target {
            super::containers::DropTarget::Form => (None, None),
            super::containers::DropTarget::Into { container, tab } => (Some(container), tab),
        };
        let c = &self.form.controls[idx];
        if c.parent == new_parent && c.tab == new_tab {
            return;
        }
        let cmd = Cmd::Reparent {
            id: id.to_string(),
            old_parent: c.parent.clone(),
            old_tab: c.tab,
            new_parent,
            new_tab,
        };
        self.apply(cmd);
    }

    /// If `(cx, cy)` lands on a `TabControl`'s tab strip, return that control's id
    /// and the clicked tab index (spec 012). The geometry mirrors the strip drawn
    /// in `cobolt_forms::paint::draw_control`.
    fn tab_strip_hit(&self, cx: i32, cy: i32) -> Option<(String, u32)> {
        for &idx in super::containers::render_order(&self.form.controls)
            .iter()
            .rev()
        {
            let c = &self.form.controls[idx];
            if c.control_type != ControlType::TabControl {
                continue;
            }
            if !super::containers::is_visible(&self.form.controls, idx, &self.active_tabs) {
                continue;
            }
            let r = c.rect;
            if cy < r.y || cy > r.y + c.tab_strip_height() || cx < r.x || cx > r.x + r.w {
                continue;
            }
            let tabs: Vec<String> = c
                .get_prop("Tabs")
                .map(|v| v.as_str().lines().map(|s| s.to_string()).collect())
                .unwrap_or_default();
            let mut tx = r.x as f32;
            let gap = c.tab_padding().max(0) as f32;
            for (i, t) in tabs.iter().enumerate() {
                let tw = (t.chars().count() as f32 * 7.0 + 18.0).clamp(40.0, 160.0);
                if (cx as f32) >= tx && (cx as f32) < tx + tw {
                    return Some((c.id.clone(), i as u32));
                }
                tx += tw + gap;
            }
        }
        None
    }

    /// Topmost **visible** control under a form-space point, respecting container
    /// clipping and tab visibility (spec 012). Children win over their container.
    fn hit_top_id(&self, cx: i32, cy: i32) -> Option<String> {
        for &idx in super::containers::render_order(&self.form.controls)
            .iter()
            .rev()
        {
            if !super::containers::is_visible(&self.form.controls, idx, &self.active_tabs) {
                continue;
            }
            if let Some(clip) = super::containers::clip_rect(&self.form.controls, idx) {
                if !clip.contains(cx, cy) {
                    continue;
                }
            }
            if self.form.controls[idx].rect.contains(cx, cy) {
                return Some(self.form.controls[idx].id.clone());
            }
        }
        None
    }

    pub fn add_control(&mut self, ct: ControlType, x: i32, y: i32) {
        let id = self.next_unique_id(&ct);
        let gp = self.form.grid_size as i32;
        let sn = self.form.snap_to_grid;
        let mut ctrl = Control::new(id.clone(), ct.clone(), snap(x, gp, sn), snap(y, gp, sn));
        // Assign z_order = highest existing + 1
        let max_z = self
            .form
            .controls
            .iter()
            .map(|c| c.z_order)
            .max()
            .unwrap_or(-1);
        ctrl.z_order = max_z + 1;
        // Controls whose control intrinsically shows a text label get a Caption.
        let has_caption = matches!(
            ct,
            ControlType::Label
                | ControlType::Button
                | ControlType::CheckBox
                | ControlType::RadioButton
        );
        if has_caption {
            ctrl.properties
                .insert("Caption".into(), PropValue::String(id.clone()));
        }

        // Inherit the font the user last set this session, or — if none yet —
        // the font of the most recently added control, so new controls match the
        // rest of the form instead of resetting to the default typeface.
        let inherit_name = self.last_font_name.clone().or_else(|| {
            self.form
                .controls
                .last()
                .and_then(|c| c.get_prop("FontName"))
                .map(|v| v.as_str().to_owned())
        });
        let inherit_size = self.last_font_size.or_else(|| {
            self.form
                .controls
                .last()
                .and_then(|c| c.get_prop("FontSize"))
                .map(|v| v.as_i64())
        });
        if let Some(name) = inherit_name {
            if ctrl.properties.contains_key("FontName") {
                ctrl.properties
                    .insert("FontName".into(), PropValue::String(name));
            }
        }
        if let Some(size) = inherit_size {
            if ctrl.properties.contains_key("FontSize") {
                ctrl.properties
                    .insert("FontSize".into(), PropValue::Int(size));
            }
        }

        let index = self.form.controls.len();
        self.apply(Cmd::AddControl { index, ctrl });
        self.set_selected_one(Some(id.clone()));
        // If dropped over a container's content area, nest the new control in it
        // (spec 012). No-op when placed on the bare form.
        self.reparent_to_drop(&id);
    }

    fn cascade_ids_for(&self, ids: &[String]) -> Vec<String> {
        let mut id_set: Vec<String> = ids.to_vec();
        for sid in ids {
            if let Some(i) = self.form.controls.iter().position(|c| &c.id == sid) {
                for d in super::containers::collect_descendants(&self.form.controls, i) {
                    let did = self.form.controls[d].id.clone();
                    if !id_set.contains(&did) {
                        id_set.push(did);
                    }
                }
            }
        }
        id_set
    }

    fn delete_event_count(&self, ids: &[String]) -> usize {
        ids.iter()
            .filter_map(|id| self.form.find_control(id))
            .flat_map(|ctrl| ctrl.events.iter())
            .filter(|event| event.has_code())
            .count()
    }

    fn delete_ids_now(&mut self, ids: &[String]) {
        let mut indices: Vec<usize> = ids
            .iter()
            .filter_map(|id| self.form.controls.iter().position(|c| &c.id == id))
            .collect();
        indices.sort_unstable();
        indices.dedup();
        // Apply deletes from highest index down
        let secs = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        for idx in indices.into_iter().rev() {
            let ctrl = self.form.controls[idx].clone();
            self.apply(Cmd::DeleteControl {
                index: idx,
                ctrl,
                deleted_at: format!("designer-delete-{secs}-{idx}"),
            });
        }
        self.selected_ids.clear();
    }

    pub fn delete_selected(&mut self) {
        // Delete all selected controls AND the descendants of any selected
        // container (cascade — spec 012 R13), highest index first so indices
        // don't shift mid-loop.
        let ids = self.cascade_ids_for(&self.selected_ids);
        let event_count = self.delete_event_count(&ids);
        if event_count > 0 {
            self.pending_delete = Some(DeleteConfirmation {
                control_ids: ids.clone(),
                control_count: ids.len(),
                event_count,
            });
            return;
        }
        self.delete_ids_now(&ids);
    }

    pub fn copy_selected(&self, clipboard: &mut Option<DesignerClipboard>) {
        if self.selected_ids.is_empty() {
            return;
        }
        let ids = self.cascade_ids_for(&self.selected_ids);
        let mut indices: Vec<usize> = ids
            .iter()
            .filter_map(|id| self.form.controls.iter().position(|c| &c.id == id))
            .collect();
        indices.sort_unstable();
        indices.dedup();
        if indices.is_empty() {
            return;
        }

        let min_x = indices
            .iter()
            .map(|&idx| self.form.controls[idx].rect.x)
            .min()
            .unwrap_or(0);
        let min_y = indices
            .iter()
            .map(|&idx| self.form.controls[idx].rect.y)
            .min()
            .unwrap_or(0);

        let controls = indices
            .into_iter()
            .map(|idx| {
                let mut ctrl = self.form.controls[idx].clone();
                ctrl.rect.x -= min_x;
                ctrl.rect.y -= min_y;
                ctrl
            })
            .collect();

        *clipboard = Some(DesignerClipboard {
            controls,
            source_form: self.form.name.clone(),
            origin_x: min_x,
            origin_y: min_y,
        });
    }

    fn selected_groupbox_id(&self) -> Option<String> {
        if self.selected_ids.len() != 1 {
            return None;
        }
        let id = self.selected_ids[0].clone();
        self.form
            .find_control(&id)
            .filter(|ctrl| matches!(ctrl.control_type, ControlType::GroupBox))
            .map(|_| id)
    }

    fn validate_user_control_name(
        name: &str,
        existing_names: &[String],
    ) -> Result<(), UserControlNameError> {
        let trimmed = name.trim();
        if trimmed.is_empty() {
            return Err(UserControlNameError::Empty);
        }
        let mut chars = trimmed.chars();
        let Some(first) = chars.next() else {
            return Err(UserControlNameError::Empty);
        };
        if !first.is_ascii_alphabetic() {
            return Err(UserControlNameError::Invalid);
        }
        let mut previous_hyphen = false;
        for ch in trimmed.chars() {
            let ok = ch.is_ascii_alphanumeric() || ch == '-';
            if !ok || (ch == '-' && previous_hyphen) {
                return Err(UserControlNameError::Invalid);
            }
            previous_hyphen = ch == '-';
        }
        if trimmed.ends_with('-') {
            return Err(UserControlNameError::Invalid);
        }
        if existing_names
            .iter()
            .any(|existing| existing.eq_ignore_ascii_case(trimmed))
        {
            return Err(UserControlNameError::Duplicate);
        }
        Ok(())
    }

    fn has_circular_user_control_reference(
        new_name: &str,
        controls: &[UserControlEntry],
        definitions: &[UserControlDef],
    ) -> bool {
        fn definition_refs_name(
            target: &str,
            def_name: &str,
            definitions: &[UserControlDef],
            visiting: &mut HashSet<String>,
        ) -> bool {
            if def_name.eq_ignore_ascii_case(target) {
                return true;
            }
            let key = def_name.to_ascii_lowercase();
            if !visiting.insert(key) {
                return false;
            }
            let Some(def) = definitions
                .iter()
                .find(|def| def.name.eq_ignore_ascii_case(def_name))
            else {
                return false;
            };
            def.controls
                .iter()
                .filter_map(|entry| entry.properties.get("UserControl"))
                .any(|nested| definition_refs_name(target, nested, definitions, visiting))
        }

        controls
            .iter()
            .filter_map(|entry| entry.properties.get("UserControl"))
            .any(|nested| definition_refs_name(new_name, nested, definitions, &mut HashSet::new()))
    }

    fn capture_user_control_def(&self, group_id: &str, name: String) -> Option<UserControlDef> {
        let ids = self.cascade_ids_for(&[group_id.to_owned()]);
        let group = self.form.find_control(group_id)?;
        let origin_x = group.rect.x;
        let origin_y = group.rect.y;

        let mut controls = Vec::new();
        for ctrl_id in ids {
            let ctrl = self.form.find_control(&ctrl_id)?;
            let properties = ctrl
                .properties
                .iter()
                .map(|(key, value)| (key.clone(), value.to_xml_string()))
                .collect();
            controls.push(UserControlEntry {
                id: ctrl.id.clone(),
                control_type: ctrl.control_type.as_str().to_string(),
                parent: ctrl.parent.clone(),
                x: ctrl.rect.x - origin_x,
                y: ctrl.rect.y - origin_y,
                w: ctrl.rect.w,
                h: ctrl.rect.h,
                z_order: ctrl.z_order,
                properties,
            });
        }

        Some(UserControlDef {
            name,
            width: group.rect.w,
            height: group.rect.h,
            controls,
        })
    }

    fn next_user_control_instance_id(&self, name: &str) -> String {
        let prefix = format!("{name}-");
        let max_n = self
            .form
            .controls
            .iter()
            .filter_map(|ctrl| ctrl.id.strip_prefix(&prefix))
            .filter_map(|suffix| suffix.parse::<usize>().ok())
            .max()
            .unwrap_or(0);
        format!("{name}-{}", max_n + 1)
    }

    pub fn deploy_user_control(
        &mut self,
        def: &UserControlDef,
        x: i32,
        y: i32,
        definitions: &[UserControlDef],
    ) {
        let Some(root_entry) = def.controls.first() else {
            return;
        };

        let gp = self.form.grid_size as i32;
        let sn = self.form.snap_to_grid;
        let origin_x = snap(x.max(0), gp, sn);
        let origin_y = snap(y.max(0), gp, sn);
        let instance_id = self.next_user_control_instance_id(&def.name);
        let base_z = self
            .form
            .controls
            .iter()
            .map(|ctrl| ctrl.z_order)
            .max()
            .unwrap_or(-1)
            + 1;
        let min_z = def
            .controls
            .iter()
            .map(|entry| entry.z_order)
            .min()
            .unwrap_or(0);

        let mut id_map = HashMap::new();
        id_map.insert(root_entry.id.clone(), instance_id.clone());

        let mut root = Control::new(
            instance_id.clone(),
            ControlType::GroupBox,
            origin_x,
            origin_y,
        );
        root.rect.w = def.width;
        root.rect.h = def.height;
        root.z_order = base_z + (root_entry.z_order - min_z);
        for (key, value) in &root_entry.properties {
            root.set_prop(key.clone(), PropValue::String(value.clone()));
        }
        root.set_prop("UserControl", PropValue::String(def.name.clone()));

        let mut deployed = vec![root];
        for entry in def.controls.iter().skip(1) {
            let new_id = format!("{instance_id}-{}", entry.id);
            id_map.insert(entry.id.clone(), new_id.clone());
            let mut ctrl = Control::new(
                new_id,
                ControlType::from_str(&entry.control_type),
                origin_x + entry.x,
                origin_y + entry.y,
            );
            ctrl.rect.w = entry.w;
            ctrl.rect.h = entry.h;
            ctrl.z_order = base_z + (entry.z_order - min_z);
            for (key, value) in &entry.properties {
                ctrl.set_prop(key.clone(), PropValue::String(value.clone()));
            }
            deployed.push(ctrl);
        }

        let mut extra = Vec::new();
        for entry in def.controls.iter().skip(1) {
            let Some(nested_name) = entry.properties.get("UserControl") else {
                continue;
            };
            if def
                .controls
                .iter()
                .any(|candidate| candidate.parent.as_deref() == Some(entry.id.as_str()))
            {
                continue;
            }
            let Some(nested_def) = definitions
                .iter()
                .find(|nested| nested.name.eq_ignore_ascii_case(nested_name))
            else {
                continue;
            };
            let Some(nested_root_id) = id_map.get(&entry.id).cloned() else {
                continue;
            };
            for nested_entry in nested_def.controls.iter().skip(1) {
                let child_id = format!("{nested_root_id}-{}", nested_entry.id);
                id_map.insert(
                    format!("{}:{}", entry.id, nested_entry.id),
                    child_id.clone(),
                );
                let mut ctrl = Control::new(
                    child_id,
                    ControlType::from_str(&nested_entry.control_type),
                    origin_x + entry.x + nested_entry.x,
                    origin_y + entry.y + nested_entry.y,
                );
                ctrl.rect.w = nested_entry.w;
                ctrl.rect.h = nested_entry.h;
                ctrl.z_order = base_z + (entry.z_order - min_z) + nested_entry.z_order + 1;
                for (key, value) in &nested_entry.properties {
                    ctrl.set_prop(key.clone(), PropValue::String(value.clone()));
                }
                ctrl.parent = nested_entry
                    .parent
                    .as_ref()
                    .and_then(|parent| {
                        if parent == &nested_def.controls[0].id {
                            Some(nested_root_id.clone())
                        } else {
                            id_map.get(&format!("{}:{parent}", entry.id)).cloned()
                        }
                    })
                    .or_else(|| Some(nested_root_id.clone()));
                extra.push(ctrl);
            }
        }
        deployed.extend(extra);

        for (entry, ctrl) in def.controls.iter().zip(deployed.iter_mut()) {
            ctrl.parent = entry
                .parent
                .as_ref()
                .and_then(|parent| id_map.get(parent).cloned());
        }
        if let Some(root) = deployed.first_mut() {
            root.parent = None;
        }

        let selected_ids: Vec<String> = deployed.iter().map(|ctrl| ctrl.id.clone()).collect();
        for ctrl in deployed {
            let index = self.form.controls.len();
            self.apply(Cmd::AddControl { index, ctrl });
        }
        self.selected_ids = selected_ids;
    }

    pub fn paste_from_clipboard(&mut self, clipboard: &Option<DesignerClipboard>) {
        let Some(clipboard) = clipboard else {
            return;
        };
        if clipboard.controls.is_empty() {
            return;
        }

        let mut reserved = HashSet::new();
        let mut id_map = HashMap::new();
        let min_z = clipboard
            .controls
            .iter()
            .map(|ctrl| ctrl.z_order)
            .min()
            .unwrap_or(0);
        let base_z = self
            .form
            .controls
            .iter()
            .map(|ctrl| ctrl.z_order)
            .max()
            .unwrap_or(-1)
            + 1;

        let mut pasted = Vec::with_capacity(clipboard.controls.len());
        for source in &clipboard.controls {
            let new_id = self.next_unique_id_reserved(&source.control_type, &reserved);
            reserved.insert(new_id.clone());
            id_map.insert(source.id.clone(), new_id.clone());

            let mut ctrl = source.clone();
            ctrl.id = new_id.clone();
            ctrl.rect.x = clipboard.origin_x + 20 + source.rect.x;
            ctrl.rect.y = clipboard.origin_y + 20 + source.rect.y;
            ctrl.z_order = base_z + (source.z_order - min_z);
            pasted.push(ctrl);
        }

        let same_form = clipboard.source_form == self.form.name;
        let mut paragraph_map = HashMap::new();
        let mut reserved_paragraphs = self.existing_procedure_names();
        for ctrl in &mut pasted {
            ctrl.parent = ctrl
                .parent
                .as_ref()
                .and_then(|old_parent| id_map.get(old_parent).cloned());
            remap_control_reference_props(ctrl, &id_map);
            if same_form {
                ctrl.events.clear();
            } else {
                for event in &mut ctrl.events {
                    for (old_id, new_id) in &id_map {
                        rename_control_refs_in_cobol(&mut event.code, old_id, new_id);
                    }
                    let key = event.paragraph.clone();
                    let paragraph = paragraph_map
                        .entry(key)
                        .or_insert_with(|| {
                            let base = derive_paragraph_name(&ctrl.id, &event.event);
                            unique_procedure_name(&base, &mut reserved_paragraphs)
                        })
                        .clone();
                    event.paragraph = paragraph;
                }
            }
        }

        let selected_ids: Vec<String> = pasted.iter().map(|ctrl| ctrl.id.clone()).collect();
        for ctrl in pasted {
            let index = self.form.controls.len();
            self.apply(Cmd::AddControl { index, ctrl });
        }
        self.selected_ids = selected_ids;
    }

    fn existing_procedure_names(&self) -> HashSet<String> {
        let mut names = HashSet::new();
        for event in &self.form.form_events {
            names.insert(event.paragraph.to_ascii_uppercase());
        }
        for ctrl in &self.form.controls {
            for event in &ctrl.events {
                names.insert(event.paragraph.to_ascii_uppercase());
            }
        }
        for procedure in &self.form.user_procedures {
            names.insert(procedure.name.to_ascii_uppercase());
        }
        names
    }

    pub fn cut_selected(&mut self, clipboard: &mut Option<DesignerClipboard>) {
        if self.selected_ids.is_empty() {
            return;
        }
        self.copy_selected(clipboard);
        self.delete_selected();
    }

    pub fn duplicate_selected(&mut self, clipboard: &mut Option<DesignerClipboard>) {
        if self.selected_ids.is_empty() {
            return;
        }
        self.copy_selected(clipboard);
        self.paste_from_clipboard(clipboard);
    }

    pub fn bring_to_front(&mut self) {
        for sid in &self.selected_ids.clone() {
            let max_z = self
                .form
                .controls
                .iter()
                .filter(|c| &c.id != sid)
                .map(|c| c.z_order)
                .max()
                .unwrap_or(0);
            if let Some(c) = self.form.find_control(sid) {
                let old_z = c.z_order;
                let new_z = max_z + 1;
                if old_z != new_z {
                    self.apply(Cmd::SetZOrder {
                        id: sid.clone(),
                        old_z,
                        new_z,
                    });
                }
            }
        }
    }

    pub fn send_to_back(&mut self) {
        for sid in &self.selected_ids.clone() {
            let min_z = self
                .form
                .controls
                .iter()
                .filter(|c| &c.id != sid)
                .map(|c| c.z_order)
                .min()
                .unwrap_or(0);
            if let Some(c) = self.form.find_control(sid) {
                let old_z = c.z_order;
                let new_z = min_z - 1;
                if old_z != new_z {
                    self.apply(Cmd::SetZOrder {
                        id: sid.clone(),
                        old_z,
                        new_z,
                    });
                }
            }
        }
    }

    pub fn bring_forward(&mut self) {
        for sid in &self.selected_ids.clone() {
            if let Some(c) = self.form.find_control(sid) {
                let old_z = c.z_order;
                let new_z = old_z + 1;
                self.apply(Cmd::SetZOrder {
                    id: sid.clone(),
                    old_z,
                    new_z,
                });
            }
        }
    }

    pub fn send_backward(&mut self) {
        for sid in &self.selected_ids.clone() {
            if let Some(c) = self.form.find_control(sid) {
                let old_z = c.z_order;
                let new_z = old_z - 1;
                self.apply(Cmd::SetZOrder {
                    id: sid.clone(),
                    old_z,
                    new_z,
                });
            }
        }
    }

    // ── Alignment ─────────────────────────────────────────────────────────────

    fn selected_rects(&self) -> Vec<(String, cobolt_forms::model::Rect)> {
        self.selected_ids
            .iter()
            .filter_map(|id| self.form.find_control(id).map(|c| (id.clone(), c.rect)))
            .collect()
    }

    pub fn align_left(&mut self) {
        let rects = self.selected_rects();
        if rects.len() < 2 {
            return;
        }
        let min_x = rects.iter().map(|(_, r)| r.x).min().unwrap();
        let moves: Vec<Cmd> = rects
            .iter()
            .filter(|(_, r)| r.x != min_x)
            .map(|(id, r)| Cmd::MoveControl {
                id: id.clone(),
                old_x: r.x,
                old_y: r.y,
                new_x: min_x,
                new_y: r.y,
            })
            .collect();
        for cmd in moves {
            self.apply(cmd);
        }
    }

    pub fn align_right(&mut self) {
        let rects = self.selected_rects();
        if rects.len() < 2 {
            return;
        }
        let max_right = rects.iter().map(|(_, r)| r.x + r.w).max().unwrap();
        let moves: Vec<Cmd> = rects
            .iter()
            .map(|(id, r)| {
                let nx = max_right - r.w;
                Cmd::MoveControl {
                    id: id.clone(),
                    old_x: r.x,
                    old_y: r.y,
                    new_x: nx,
                    new_y: r.y,
                }
            })
            .filter(|c| matches!(c, Cmd::MoveControl { new_x, old_x, .. } if new_x != old_x))
            .collect();
        for cmd in moves {
            self.apply(cmd);
        }
    }

    pub fn align_top(&mut self) {
        let rects = self.selected_rects();
        if rects.len() < 2 {
            return;
        }
        let min_y = rects.iter().map(|(_, r)| r.y).min().unwrap();
        let moves: Vec<Cmd> = rects
            .iter()
            .filter(|(_, r)| r.y != min_y)
            .map(|(id, r)| Cmd::MoveControl {
                id: id.clone(),
                old_x: r.x,
                old_y: r.y,
                new_x: r.x,
                new_y: min_y,
            })
            .collect();
        for cmd in moves {
            self.apply(cmd);
        }
    }

    pub fn align_bottom(&mut self) {
        let rects = self.selected_rects();
        if rects.len() < 2 {
            return;
        }
        let max_bottom = rects.iter().map(|(_, r)| r.y + r.h).max().unwrap();
        let moves: Vec<Cmd> = rects
            .iter()
            .map(|(id, r)| {
                let ny = max_bottom - r.h;
                Cmd::MoveControl {
                    id: id.clone(),
                    old_x: r.x,
                    old_y: r.y,
                    new_x: r.x,
                    new_y: ny,
                }
            })
            .filter(|c| matches!(c, Cmd::MoveControl { new_y, old_y, .. } if new_y != old_y))
            .collect();
        for cmd in moves {
            self.apply(cmd);
        }
    }

    pub fn center_horizontal(&mut self) {
        let rects = self.selected_rects();
        if rects.len() < 2 {
            return;
        }
        let avg_cx = rects.iter().map(|(_, r)| r.x + r.w / 2).sum::<i32>() / rects.len() as i32;
        let moves: Vec<Cmd> = rects
            .iter()
            .map(|(id, r)| {
                let nx = avg_cx - r.w / 2;
                Cmd::MoveControl {
                    id: id.clone(),
                    old_x: r.x,
                    old_y: r.y,
                    new_x: nx,
                    new_y: r.y,
                }
            })
            .collect();
        for cmd in moves {
            self.apply(cmd);
        }
    }

    pub fn center_vertical(&mut self) {
        let rects = self.selected_rects();
        if rects.len() < 2 {
            return;
        }
        let avg_cy = rects.iter().map(|(_, r)| r.y + r.h / 2).sum::<i32>() / rects.len() as i32;
        let moves: Vec<Cmd> = rects
            .iter()
            .map(|(id, r)| {
                let ny = avg_cy - r.h / 2;
                Cmd::MoveControl {
                    id: id.clone(),
                    old_x: r.x,
                    old_y: r.y,
                    new_x: r.x,
                    new_y: ny,
                }
            })
            .collect();
        for cmd in moves {
            self.apply(cmd);
        }
    }

    pub fn space_evenly_horizontal(&mut self) {
        let mut rects = self.selected_rects();
        if rects.len() < 3 {
            return;
        }
        rects.sort_by_key(|(_, r)| r.x);
        let total_w: i32 = rects.iter().map(|(_, r)| r.w).sum();
        let span = (rects.last().unwrap().1.x + rects.last().unwrap().1.w) - rects[0].1.x;
        let gap = (span - total_w).max(0) / (rects.len() as i32 - 1);
        let mut x = rects[0].1.x;
        for (id, r) in &rects {
            let nx = x;
            if nx != r.x {
                let _ = self.apply(Cmd::MoveControl {
                    id: id.clone(),
                    old_x: r.x,
                    old_y: r.y,
                    new_x: nx,
                    new_y: r.y,
                });
            }
            x += r.w + gap;
        }
    }

    pub fn space_evenly_vertical(&mut self) {
        let mut rects = self.selected_rects();
        if rects.len() < 3 {
            return;
        }
        rects.sort_by_key(|(_, r)| r.y);
        let total_h: i32 = rects.iter().map(|(_, r)| r.h).sum();
        let span = (rects.last().unwrap().1.y + rects.last().unwrap().1.h) - rects[0].1.y;
        let gap = (span - total_h).max(0) / (rects.len() as i32 - 1);
        let mut y = rects[0].1.y;
        for (id, r) in &rects {
            let ny = y;
            if ny != r.y {
                let _ = self.apply(Cmd::MoveControl {
                    id: id.clone(),
                    old_x: r.x,
                    old_y: r.y,
                    new_x: r.x,
                    new_y: ny,
                });
            }
            y += r.h + gap;
        }
    }

    /// Auto-arrange: find (Label, input) pairs by LabelFor and stack them in rows.
    /// Labels go on the left column, inputs on the right, aligned vertically.
    pub fn auto_arrange_labels(&mut self) {
        // Collect (label_id, input_id) pairs from LabelFor properties
        let pairs: Vec<(String, String)> = self
            .form
            .controls
            .iter()
            .filter(|c| c.control_type == ControlType::Label)
            .filter_map(|lbl| {
                let for_id = lbl.get_prop("LabelFor").and_then(|v| {
                    if v.as_str().is_empty() {
                        None
                    } else {
                        Some(v.as_str().to_owned())
                    }
                })?;
                // Verify the target exists
                if self.form.find_control(&for_id).is_some() {
                    Some((lbl.id.clone(), for_id))
                } else {
                    None
                }
            })
            .collect();

        if pairs.is_empty() {
            return;
        }

        let margin_x = 16;
        let margin_y = 24;
        let label_w = 120;
        let gap_x = 8;
        let row_h = 28;

        let mut y = margin_y;
        for (lbl_id, inp_id) in pairs {
            // Move label
            let lbl_rect = self.form.find_control(&lbl_id).map(|c| c.rect);
            let inp_rect = self.form.find_control(&inp_id).map(|c| c.rect);

            if let (Some(lr), Some(ir)) = (lbl_rect, inp_rect) {
                // Center label vertically with input
                let lbl_y = y + (ir.h - lr.h) / 2;
                self.apply(Cmd::MoveControl {
                    id: lbl_id,
                    old_x: lr.x,
                    old_y: lr.y,
                    new_x: margin_x,
                    new_y: lbl_y,
                });
                self.apply(Cmd::MoveControl {
                    id: inp_id,
                    old_x: ir.x,
                    old_y: ir.y,
                    new_x: margin_x + label_w + gap_x,
                    new_y: y,
                });
                y += ir.h.max(lr.h) + row_h / 2;
            }
        }
    }

    /// Open the modal COBOL code editor for `event_name` on control `ctrl_id`.
    /// Pass an empty `ctrl_id` for form-level events (OnLoad, OnClose).
    pub fn open_event_modal(&mut self, ctrl_id: &str, event_name: &str) {
        // Find the event binding — either in a control or in form_events — and
        // resolve its PROGRAM-ID and existing source.
        let (program_id, existing, display) = if ctrl_id.is_empty() {
            let ev = self.form.form_events.iter().find(|e| e.event == event_name);
            let (pid, code) = ev
                .map(|e| (e.paragraph.clone(), e.code.clone()))
                .unwrap_or_else(|| {
                    let pid = format!(
                        "{}--{}",
                        self.form.name,
                        event_name.to_ascii_uppercase().replace(' ', "-")
                    );
                    (pid, String::new())
                });
            (pid, code, format!("Form · {}", event_name))
        } else {
            let ev = self
                .form
                .find_control(ctrl_id)
                .and_then(|c| c.events.iter().find(|e| e.event == event_name));
            let (pid, code) = ev
                .map(|e| (e.paragraph.clone(), e.code.clone()))
                .unwrap_or_else(|| {
                    let pid = format!(
                        "{}--{}",
                        ctrl_id.to_ascii_uppercase(),
                        event_name.to_ascii_uppercase().replace(' ', "-")
                    );
                    (pid, String::new())
                });
            (pid, code, format!("{} · {}", ctrl_id, event_name))
        };

        // First time this handler is edited → open it with the standard
        // skeleton (incl. the event's LINKAGE data and PROCEDURE DIVISION USING,
        // if any). Otherwise show the saved source.
        let source = if existing.trim().is_empty() {
            // A control that belongs to a repeating group (array) gets the indexed
            // skeleton — the handler receives the fired item's array index.
            if !ctrl_id.is_empty()
                && self
                    .form
                    .array_binding_context_for_member(ctrl_id)
                    .is_some()
            {
                cobolt_forms::model::event_handler_template_indexed(event_name, ctrl_id)
            } else {
                cobolt_forms::model::event_handler_template(event_name)
            }
        } else {
            existing
        };

        // Host the full COBOL editor (IntelliSense + find/replace + status bar)
        // on this handler's source. Feed it the form's controls for completion.
        self.event_editor.open_buffer(
            std::path::PathBuf::from(format!("{program_id}.handler")),
            source.clone(),
        );
        self.event_editor.known_controls = super::editor::build_known_controls(&self.form);

        self.event_modal = Some(EventEditorModal::new(
            ctrl_id, display, event_name, program_id, source,
        ));
    }

    /// Commit the modal editor's content back into the form's event binding.
    pub fn save_event_handler(&mut self, ctrl_id: &str, event_name: &str, source: String) {
        if ctrl_id.is_empty() {
            // Form-level event — create the binding if it doesn't exist yet
            // (only onLoad/onClose are pre-stubbed; the rest are created lazily).
            if !self.form.form_events.iter().any(|e| e.event == event_name) {
                let paragraph =
                    cobolt_forms::model::derive_paragraph_name(&self.form.name, event_name);
                self.form.form_events.push(cobolt_forms::EventBinding {
                    event: event_name.to_string(),
                    paragraph,
                    code: String::new(),
                });
            }
            if let Some(ev) = self
                .form
                .form_events
                .iter_mut()
                .find(|e| e.event == event_name)
            {
                ev.code = source;
                self.dirty = true;
            }
        } else if let Some(ctrl) = self.form.find_control_mut(ctrl_id) {
            ctrl.ensure_event(event_name);
            if let Some(ev) = ctrl.events.iter_mut().find(|e| e.event == event_name) {
                ev.code = source;
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
                let field = &rest[us + 1..];
                if let Ok(idx) = idx_str.parse::<usize>() {
                    if let Some(ctrl) = self.form.find_control_mut(ctrl_id) {
                        if let Some(anim) = ctrl.animations.get_mut(idx) {
                            match field {
                                "Name" => anim.name = value.as_str().to_owned(),
                                "Trigger" => anim.trigger = AnimTrigger::from_str(value.as_str()),
                                "Kind" => anim.kind = AnimKind::from_str(value.as_str()),
                                "Duration" => anim.duration_ms = value.as_i64().max(1) as u64,
                                "Delay" => anim.delay_ms = value.as_i64().max(0) as u64,
                                "Easing" => anim.easing = EasingKind::from_str(value.as_str()),
                                "Repeat" => {
                                    anim.repeat = match value.as_str() {
                                        "Loop" => AnimRepeat::Loop,
                                        "PingPong" => AnimRepeat::PingPong,
                                        "Count" => AnimRepeat::Count(3),
                                        _ => AnimRepeat::Once,
                                    }
                                }
                                "SlideDX" => anim.slide_dx = value.as_i64() as i32,
                                "SlideDY" => anim.slide_dy = value.as_i64() as i32,
                                _ => {}
                            }
                            self.dirty = true;
                        }
                    }
                }
            }
            return;
        }

        // Remember the last font the user chose, so newly-added controls inherit
        // it (see `add_control`).
        match key {
            "FontName" => self.last_font_name = Some(value.as_str().to_owned()),
            "FontSize" => self.last_font_size = Some(value.as_i64()),
            _ => {}
        }

        match key {
            "X" | "Y" | "Width" | "Height" => {
                let old_opt = self.form.find_control(ctrl_id).map(|c| c.rect);
                if let Some(old_rect) = old_opt {
                    let mut new_rect = old_rect;
                    match key {
                        "X" => new_rect.x = value.as_i64() as i32,
                        "Y" => new_rect.y = value.as_i64() as i32,
                        "Width" => new_rect.w = (value.as_i64() as i32).max(1),
                        "Height" => new_rect.h = (value.as_i64() as i32).max(1),
                        _ => {}
                    }
                    if new_rect != old_rect {
                        self.apply(Cmd::ResizeControl {
                            id: ctrl_id.to_owned(),
                            old_rect,
                            new_rect,
                        });
                    }
                }
            }
            "ZOrder" => {
                if let Some(c) = self.form.find_control(ctrl_id) {
                    let old_z = c.z_order;
                    let new_z = value.as_i64() as i32;
                    if old_z != new_z {
                        self.apply(Cmd::SetZOrder {
                            id: ctrl_id.to_owned(),
                            old_z,
                            new_z,
                        });
                    }
                }
            }
            "Visible" => {
                if let Some(c) = self.form.find_control_mut(ctrl_id) {
                    c.visible = value.as_bool();
                    self.dirty = true;
                }
            }
            "Enabled" => {
                if let Some(c) = self.form.find_control_mut(ctrl_id) {
                    c.enabled = value.as_bool();
                    self.dirty = true;
                }
            }
            "TabOrder" => {
                if let Some(c) = self.form.find_control_mut(ctrl_id) {
                    c.tab_order = value.as_i64() as u32;
                    self.dirty = true;
                }
            }
            _ => {
                // When the ImagePath changes, evict the old texture from cache
                if key == "ImagePath" {
                    if let Some(old_path) = self
                        .form
                        .find_control(ctrl_id)
                        .and_then(|c| c.get_prop("ImagePath"))
                        .map(|v| v.as_str().to_owned())
                    {
                        self.image_cache.remove(&old_path);
                    }
                    // Also evict the new path in case the file changed on disk
                    self.image_cache.remove(value.as_str());
                }
                let old = self
                    .form
                    .find_control(ctrl_id)
                    .and_then(|c| c.properties.get(key).cloned());
                self.apply(Cmd::SetProperty {
                    id: ctrl_id.to_owned(),
                    key: key.to_owned(),
                    old,
                    new: value,
                });
            }
        }
    }

    pub fn set_form_prop(&mut self, key: &str, value: String) {
        match key {
            "Title" => {
                self.form.title = value;
                self.dirty = true;
            }
            "BackgroundColor" => {
                self.form.background_color = value.trim_start_matches('#').to_owned();
                self.dirty = true;
            }
            "Width" => {
                if let Ok(w) = value.parse::<u32>() {
                    self.form.width = w.max(64);
                    self.dirty = true;
                }
            }
            "Height" => {
                if let Ok(h) = value.parse::<u32>() {
                    self.form.height = h.max(64);
                    self.dirty = true;
                }
            }
            "Transparency" => {
                if let Ok(v) = value.parse::<u8>() {
                    self.form.transparency = v.min(100);
                    self.dirty = true;
                }
            }
            "GridSize" => {
                if let Ok(v) = value.parse::<u8>() {
                    self.form.grid_size = v.clamp(4, 64);
                    self.dirty = true;
                }
            }
            "SnapToGrid" => {
                self.form.snap_to_grid = value == "true" || value == "1";
                self.dirty = true;
            }
            "GlassStyle" => {
                self.form.glass_style = cobolt_forms::model::GlassStyle::from_str(&value);
                self.dirty = true;
            }
            "Target" => {
                if let Some((w, h)) = target_preset_size(&value) {
                    self.form.width = w;
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
            "BgImageMode" => {
                self.form.bg_image_mode = BgImageMode::from_str(&value);
                self.dirty = true;
            }
            // 007 Form themes — per-form override + themed-background opt-in.
            "Theme" => {
                let v = value.trim();
                self.form.theme = if v.is_empty() {
                    None
                } else {
                    Some(v.to_owned())
                };
                self.dirty = true;
            }
            "UseThemeBackground" => {
                self.form.use_theme_background = value == "true" || value == "1";
                self.dirty = true;
            }

            _ => {}
        }
    }

    /// Trigger animation preview for a control by animation name.
    pub fn play_animation_preview(&mut self, ctrl_id: &str, anim_name: &str) {
        // Look up the delay so we honour it during preview.
        let delay_secs = self
            .form
            .find_control(ctrl_id)
            .and_then(|c| c.animations.iter().find(|a| a.name == anim_name))
            .map(|a| a.delay_ms as f32 / 1000.0)
            .unwrap_or(0.0);
        let state = self
            .anim_states
            .entry(format!("{ctrl_id}:{anim_name}"))
            .or_insert_with(|| AnimState::new(anim_name));
        state.play(delay_secs);
    }

    /// Whether there is an undoable command on the stack (drives the toolbar Undo icon).
    pub(crate) fn can_undo(&self) -> bool {
        !self.undo_stack.is_empty()
    }

    /// Whether there is a redoable command on the stack (drives the toolbar Redo icon).
    pub(crate) fn can_redo(&self) -> bool {
        !self.redo_stack.is_empty()
    }

    /// Toggle the format-painter state machine (same logic as the old toolbar click).
    pub(crate) fn toggle_format_painter(&mut self) {
        match &self.format_painter {
            FormatPainter::WaitingForTarget { .. } | FormatPainter::WaitingForSource => {
                self.format_painter = FormatPainter::Idle;
            }
            FormatPainter::Idle => {
                if let Some(sid) = self.selected_ids.first().cloned() {
                    if let Some(src) = self.form.find_control(&sid) {
                        let props = src
                            .properties
                            .iter()
                            .filter(|(k, _)| STYLE_PROP_KEYS.contains(&k.as_str()))
                            .map(|(k, v)| (k.clone(), v.clone()))
                            .collect();
                        let animations = src.animations.clone();
                        let src_rect = src.rect.clone();
                        self.format_painter = FormatPainter::WaitingForTarget {
                            props,
                            animations,
                            src_rect,
                        };
                    }
                }
            }
        }
    }

    /// Play all OnFormLoad animations (Preview Anims button).
    pub(crate) fn play_all_form_load_anims(&mut self) {
        let ctrl_anims: Vec<(String, String)> = self
            .form
            .controls
            .iter()
            .flat_map(|c| {
                c.animations
                    .iter()
                    .filter(|a| a.trigger == AnimTrigger::OnFormLoad)
                    .map(move |a| (c.id.clone(), a.name.clone()))
            })
            .collect();
        for (cid, aname) in ctrl_anims {
            self.play_animation_preview(&cid, &aname);
        }
    }

    // ── Rendering ─────────────────────────────────────────────────────────────

    pub fn show(
        &mut self,
        ui: &mut Ui,
        clipboard: &mut Option<DesignerClipboard>,
        user_controls: &[UserControlDef],
        llm_cfg: &crate::llm::LlmConfig,
    ) -> DesignerShowResult {
        let mut result = DesignerShowResult::default();
        let mut selection_changed = false;

        // 007 Form themes — publish the resolved asset-pack theme for this frame
        // so the shared `draw_control` skins controls (canvas + preview). `None`
        // ⇒ procedural Liquid Glass.
        cobolt_forms::paint::set_active_theme(ui.ctx(), self.active_theme_pack.clone());
        cobolt_forms::paint::set_glass_style(ui.ctx(), self.form.glass_style);

        // Load menu YAML files for any MenuBar controls and cache them
        if let Some(dir) = &self.cfrm_dir {
            for ctrl in &self.form.controls {
                if ctrl.control_type == ControlType::MenuBar {
                    let yaml_path = cobolt_forms::menu::menu_yaml_path(dir, &ctrl.id);
                    if yaml_path.exists() {
                        if cobolt_forms::paint::get_menu_cache(ui.ctx(), &ctrl.id).is_none() {
                            if let Ok(def) = cobolt_forms::menu::load_menu(&yaml_path) {
                                cobolt_forms::paint::set_menu_cache(
                                    ui.ctx(),
                                    &ctrl.id,
                                    std::sync::Arc::new(def),
                                );
                            }
                        }
                    }
                }
            }
        }

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
            let anim_meta: HashMap<String, (u64, u64)> = self
                .form
                .controls
                .iter()
                .flat_map(|c| {
                    c.animations
                        .iter()
                        .map(move |a| (format!("{}:{}", c.id, a.name), (a.duration_ms, a.delay_ms)))
                })
                .collect();

            for (key, state) in self.anim_states.iter_mut() {
                if !state.playing {
                    continue;
                }

                // ── Delay phase: count down before t starts moving ────────────
                if state.delay_remaining > 0.0 {
                    state.delay_remaining -= dt;
                    if state.delay_remaining < 0.0 {
                        state.delay_remaining = 0.0;
                    }
                    need_repaint = true;
                    continue; // don't advance t yet
                }

                let dur = anim_meta.get(key).map(|(d, _)| *d).unwrap_or(400) as f32 / 1000.0;
                if dur <= 0.0 {
                    state.stop();
                    continue;
                }
                state.t += dt / dur;
                if state.t >= 1.0 {
                    state.t = 1.0;
                    state.playing = false;
                }
                need_repaint = true;
            }
            if need_repaint {
                ui.ctx().request_repaint();
            }
        }

        let canvas_w = self.form.width as f32;
        let canvas_h = self.form.height as f32;

        egui::ScrollArea::both()
            .id_salt("designer_canvas")
            // Fill the available panel rather than growing to the form size, so
            // the canvas actually scrolls when the form is larger than the view
            // (spec 012 follow-up: restore lost form-content scrolling).
            .auto_shrink([false, false])
            .show(ui, |ui| {
                let (resp, painter) =
                    ui.allocate_painter(Vec2::new(canvas_w, canvas_h), Sense::click_and_drag());
                let origin = resp.rect.min;

                // ── Form canvas background ─────────────────────────────────────
                // BackColor (RRGGBBAA hex) controls fill + alpha.
                // Transparent (alpha=0) means the wallpaper shows through.
                // form.transparency (0=opaque..100=fully transparent) also dims the canvas.
                let form_alpha_mul = 1.0 - (self.form.transparency as f32 / 100.0);
                let bg_raw = parse_color(&self.form.background_color);
                // Apply form transparency to background alpha
                let bg = Color32::from_rgba_premultiplied(
                    bg_raw.r(),
                    bg_raw.g(),
                    bg_raw.b(),
                    ((bg_raw.a() as f32) * form_alpha_mul) as u8,
                );
                // WYSIWYG: a fully transparent form renders over the runtime's
                // dark glass base, NOT over the IDE theme — paint that same
                // dark base here so light IDE themes don't hide light-coloured
                // captions that will be perfectly visible at run time.
                let runtime_glass = Color32::from_rgba_unmultiplied(20, 24, 44, 200);
                let canvas_bg = if bg.a() > 0 { bg } else { runtime_glass };
                // Corner-notch masks repaint the visible backdrop after child
                // controls. Use the composited canvas colour so translucent
                // forms/glass do not either darken (double alpha) or fail to
                // cover child bleed.
                let notch_fill = cobolt_forms::paint::composite_premultiplied_over(
                    canvas_bg,
                    ui.visuals().panel_fill,
                );
                if self.glass_mode {
                    let corner = egui::Rounding::same(6.0);
                    painter.rect_filled(resp.rect, corner, canvas_bg);
                    // Thin border so the form boundary is always visible
                    painter.rect_stroke(
                        resp.rect,
                        corner,
                        egui::Stroke::new(1.0, Color32::from_rgba_unmultiplied(255, 255, 255, 60)),
                    );
                } else {
                    painter.rect_filled(resp.rect, 0.0, canvas_bg);
                }

                // ── Themed background (007 R8) ─────────────────────────────────
                // When the form opts in and the active pack provides one, the
                // theme background replaces the form's own back-colour image.
                let themed_bg = cobolt_forms::paint::draw_theme_background(
                    &painter,
                    resp.rect,
                    self.form.use_theme_background,
                    form_alpha_mul,
                );

                // ── Background image ───────────────────────────────────────────
                // Captured for the corner-notch mask: the backdrop texture + the
                // screen rect it maps to, so a rounded container's notches can be
                // repainted with the same image behind its children (spec 017).
                let mut notch_img: Option<(egui::TextureId, egui::Rect)> = None;
                let bg_img_path = self.form.background_image.clone();
                let bg_img_mode = self.form.bg_image_mode;
                if !themed_bg && !bg_img_path.is_empty() {
                    let ctx_ref2 = ui.ctx().clone();
                    self.load_image(&bg_img_path, &ctx_ref2);
                    let img_alpha = (255.0 * form_alpha_mul) as u8;
                    if img_alpha > 0 {
                        if let Some(tex) =
                            self.image_cache.get(&bg_img_path).and_then(|o| o.as_ref())
                        {
                            let tex_size = tex.size_vec2();
                            // White tint at varying alpha — no color modulation, just transparency
                            let tint = Color32::from_rgba_premultiplied(255, 255, 255, img_alpha);
                            let tex_id = tex.id();
                            let form_rect = resp.rect;
                            match bg_img_mode {
                                BgImageMode::Stretch => {
                                    painter.image(
                                        tex_id,
                                        form_rect,
                                        egui::Rect::from_min_max(
                                            egui::pos2(0.0, 0.0),
                                            egui::pos2(1.0, 1.0),
                                        ),
                                        tint,
                                    );
                                    notch_img = Some((tex_id, form_rect));
                                }
                                BgImageMode::Fill => {
                                    // Scale so image fills the whole form keeping aspect ratio (crops if needed)
                                    let sx = form_rect.width() / tex_size.x;
                                    let sy = form_rect.height() / tex_size.y;
                                    let s = sx.max(sy);
                                    let dw = tex_size.x * s;
                                    let dh = tex_size.y * s;
                                    let ox = (form_rect.width() - dw) / 2.0;
                                    let oy = (form_rect.height() - dh) / 2.0;
                                    let dest = egui::Rect::from_min_size(
                                        form_rect.min + egui::vec2(ox, oy),
                                        egui::vec2(dw, dh),
                                    );
                                    painter.image(
                                        tex_id,
                                        dest,
                                        egui::Rect::from_min_max(
                                            egui::pos2(0.0, 0.0),
                                            egui::pos2(1.0, 1.0),
                                        ),
                                        tint,
                                    );
                                    notch_img = Some((tex_id, dest));
                                }
                                BgImageMode::Fit => {
                                    // Scale so whole image fits inside form, keeping aspect ratio (letterbox)
                                    let sx = form_rect.width() / tex_size.x;
                                    let sy = form_rect.height() / tex_size.y;
                                    let s = sx.min(sy);
                                    let dw = tex_size.x * s;
                                    let dh = tex_size.y * s;
                                    let ox = (form_rect.width() - dw) / 2.0;
                                    let oy = (form_rect.height() - dh) / 2.0;
                                    let dest = egui::Rect::from_min_size(
                                        form_rect.min + egui::vec2(ox, oy),
                                        egui::vec2(dw, dh),
                                    );
                                    painter.image(
                                        tex_id,
                                        dest,
                                        egui::Rect::from_min_max(
                                            egui::pos2(0.0, 0.0),
                                            egui::pos2(1.0, 1.0),
                                        ),
                                        tint,
                                    );
                                    notch_img = Some((tex_id, dest));
                                }
                                BgImageMode::Center => {
                                    let ox = (form_rect.width() - tex_size.x) / 2.0;
                                    let oy = (form_rect.height() - tex_size.y) / 2.0;
                                    let dest = egui::Rect::from_min_size(
                                        form_rect.min + egui::vec2(ox, oy),
                                        tex_size,
                                    );
                                    painter.image(
                                        tex_id,
                                        dest,
                                        egui::Rect::from_min_max(
                                            egui::pos2(0.0, 0.0),
                                            egui::pos2(1.0, 1.0),
                                        ),
                                        tint,
                                    );
                                    notch_img = Some((tex_id, dest));
                                }
                                BgImageMode::Tile => {
                                    // Tile the image across the form canvas
                                    let tw = tex_size.x.max(1.0);
                                    let th = tex_size.y.max(1.0);
                                    // Notch mask samples one tile from the form origin and
                                    // relies on Repeat wrap to tile (matches this phase).
                                    notch_img = Some((
                                        tex_id,
                                        egui::Rect::from_min_size(
                                            form_rect.min,
                                            egui::vec2(tw, th),
                                        ),
                                    ));
                                    let cols = (form_rect.width() / tw).ceil() as i32 + 1;
                                    let rows = (form_rect.height() / th).ceil() as i32 + 1;
                                    for row in 0..rows {
                                        for col in 0..cols {
                                            let tile_min = form_rect.min
                                                + egui::vec2(col as f32 * tw, row as f32 * th);
                                            let tile_max = egui::pos2(
                                                (tile_min.x + tw).min(form_rect.max.x),
                                                (tile_min.y + th).min(form_rect.max.y),
                                            );
                                            if tile_min.x >= form_rect.max.x
                                                || tile_min.y >= form_rect.max.y
                                            {
                                                continue;
                                            }
                                            let u1 = (tile_max.x - tile_min.x) / tw;
                                            let v1 = (tile_max.y - tile_min.y) / th;
                                            let dest_tile =
                                                egui::Rect::from_min_max(tile_min, tile_max);
                                            painter.image(
                                                tex_id,
                                                dest_tile,
                                                egui::Rect::from_min_max(
                                                    egui::pos2(0.0, 0.0),
                                                    egui::pos2(u1, v1),
                                                ),
                                                tint,
                                            );
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
                let ptr_canvas: Option<(i32, i32)> = ui.ctx().pointer_interact_pos().map(|p| {
                    let rel = p - origin;
                    (rel.x as i32, rel.y as i32)
                });

                // Show pointer cursor when hovering over any control on the canvas.
                if let Some((cx, cy)) = ptr_canvas {
                    let over_ctrl = self.form.controls.iter().any(|c| c.rect.contains(cx, cy));
                    if over_ctrl {
                        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                    }
                }

                // Handle drag input
                self.handle_drag(&resp, &painter, origin, ptr_canvas, &mut selection_changed);

                // A control dragged out of the toolbox: show a ghost at the cursor
                // and drop it where released (see `handle_toolbox_dnd`).
                self.handle_toolbox_dnd(ui, resp.rect, origin, ptr_canvas, user_controls);

                // Draw controls sorted by z_order
                let selected_ids = self.selected_ids.clone();
                let form_w = self.form.width as f32;
                let form_h = self.form.height as f32;

                // Build render list in container tree order — parents before
                // children, siblings by z_order — so nested controls paint on top
                // of their container (spec 012).
                let render_order: Vec<usize> = super::containers::render_order(&self.form.controls);
                // Active tab per TabControl for design-time visibility. The
                // interactive selection lives in `self.active_tabs`; an entry is
                // absent until the user clicks a tab, in which case `is_visible`
                // falls back to the control's `SelectedTab` property.
                let active_tabs = self.active_tabs.clone();

                // ── Control faces via the unified engine (spec 017 T6) ──────────
                // Every face is drawn through the same `draw_control` path as the
                // preview / running form / compiled binary, so the canvas matches
                // them exactly. The designer overlays its editor chrome (selection
                // border + handles, badges, clones, grid, drop hints) on top using
                // the on-screen rects the engine returns.
                let anim_tf: std::collections::HashMap<
                    String,
                    cobolt_forms::render::RenderTransform,
                > = self
                    .form
                    .controls
                    .iter()
                    .filter_map(|c| {
                        c.animations.iter().find_map(|a| {
                            let key = format!("{}:{}", c.id, a.name);
                            self.anim_states
                                .get(&key)
                                .filter(|s| s.playing || (s.t > 0.0 && s.t < 1.0))
                                .map(|s| {
                                    let (dx, dy, scale, alpha) =
                                        anim_transform(a, form_w, form_h, s.t);
                                    (
                                        c.id.clone(),
                                        cobolt_forms::render::RenderTransform {
                                            dx,
                                            dy,
                                            scale,
                                            alpha,
                                        },
                                    )
                                })
                        })
                    })
                    .collect();
                // ── Rounded-container child clip (spec 017) ─────────────────────
                // When enabled, the render walk hands each rounded container to this
                // GL hook right after its face+shadow are painted; the hook captures
                // the backdrop+shadow behind the corners and re-blits the notches
                // once children are drawn — clipping bleed without erasing the
                // shadow. Opt-in via COBOLT_ROUNDED_CLIP; otherwise the flat notch
                // mask below is used. `finish` is called inside `render_faces`.
                let rounded_clip_on = crate::panels::rounded_clip::enabled();
                let clip_hook = crate::panels::rounded_clip::RoundedClipHook::new();
                let hook_ref: Option<&dyn cobolt_forms::render::RoundedClipHook> =
                    if rounded_clip_on {
                        Some(&clip_hook)
                    } else {
                        None
                    };

                let control_rects = {
                    let st = DesignerState { anim: &anim_tf };
                    let input = cobolt_forms::render::RenderInput {
                        controls: &self.form.controls,
                        state: &st,
                        form_size: Vec2::new(form_w, form_h),
                        glass: self.glass_mode,
                        mode: cobolt_forms::render::RenderMode::Static,
                        active_tabs: &active_tabs,
                        backdrop: cobolt_forms::render::Backdrop::default(),
                    };
                    cobolt_forms::render::render_faces(&painter, origin, &input, hook_ref)
                        .control_rects
                };

                // ── Corner-notch masks (spec 017) ───────────────────────────────
                // Legacy fallback: egui can't clip children to a rounded rect, so
                // after the faces are drawn we repaint each rounded GroupBox/Panel's
                // corner notches with the canvas backdrop (colour + image) to cover
                // child bleed. Skipped when the GL rounded clip is active, which
                // handles it correctly (backdrop + shadow) via `render_faces`.
                if !rounded_clip_on {
                    let img_alpha = (255.0 * form_alpha_mul) as u8;
                    for (idx, ctrl) in self.form.controls.iter().enumerate() {
                        if !matches!(
                            ctrl.control_type,
                            ControlType::GroupBox | ControlType::Panel
                        ) {
                            continue;
                        }
                        if !cobolt_forms::containers::has_descendants(&self.form.controls, idx) {
                            continue;
                        }
                        if ctrl.parent.is_some() {
                            // Nested rounded containers must reveal the already
                            // painted parent surface in their notches. Masking
                            // them with the form/canvas backdrop cuts through
                            // that parent and creates the dark patterned corner
                            // rectangles we are debugging.
                            continue;
                        }
                        let rad = cobolt_forms::paint::corner_radius(ctrl);
                        if rad < 0.5 {
                            continue;
                        }
                        if let Some(crect) = control_rects.get(&ctrl.id) {
                            // Only mask the corners a descendant actually reaches;
                            // leave clean corners untouched so the panel keeps its own
                            // rounded corner (matching an empty GroupBox) instead of
                            // having the backdrop painted over it for no reason.
                            let rounding = cobolt_forms::render::corner_notch_rounding(
                                *crect,
                                rad,
                                &self.form.controls,
                                idx,
                                &control_rects,
                            );
                            cobolt_forms::paint::draw_container_notch_mask(
                                &painter,
                                *crect,
                                rounding,
                                notch_fill,
                                notch_img,
                                img_alpha,
                            );
                            if self.show_grid {
                                draw_grid_in_rounded_notches(
                                    &painter,
                                    resp.rect,
                                    *crect,
                                    egui::Rounding::same(rad),
                                    self.form.grid_size.max(4) as f32,
                                    self.glass_mode,
                                );
                            }
                        }
                    }
                }

                // (Rounded-clip re-blit is flushed inside `render_faces` via the
                // hook's `finish`, so no separate designer pass is needed here.)

                // ── Editor badges on top of the faces ───────────────────────────
                for &idx in &render_order {
                    let ctrl = &self.form.controls[idx];
                    let Some(crect) = control_rects.get(&ctrl.id) else {
                        continue;
                    };
                    // Repeating-group ARRAY marker at the GroupBox top-right (spec 015).
                    if matches!(ctrl.control_type, ControlType::GroupBox)
                        && ctrl
                            .get_prop("IsRepeatingGroup")
                            .map(|v| v.as_bool())
                            .unwrap_or(false)
                    {
                        let (bw, bh) = (46.0_f32, 15.0_f32);
                        let brect = egui::Rect::from_min_size(
                            egui::pos2(crect.max.x - bw - 3.0, crect.min.y + 3.0),
                            Vec2::new(bw, bh),
                        );
                        painter.rect_filled(
                            brect,
                            3.0,
                            Color32::from_rgba_premultiplied(60, 120, 230, 230),
                        );
                        painter.text(
                            brect.center(),
                            egui::Align2::CENTER_CENTER,
                            "▦ ARRAY",
                            egui::FontId::proportional(9.0),
                            Color32::WHITE,
                        );
                    }
                    // Animation badge tooltip — hover to see the animation list.
                    if !ctrl.animations.is_empty() {
                        let badge_rect = egui::Rect::from_center_size(
                            egui::pos2(crect.max.x - 2.0, crect.min.y + 2.0),
                            Vec2::splat(12.0),
                        );
                        let anim_summary: String = ctrl
                            .animations
                            .iter()
                            .map(|a| format!("▶ {} ({:?})", a.name, a.trigger))
                            .collect::<Vec<_>>()
                            .join("\n");
                        ui.interact(
                            badge_rect,
                            egui::Id::new(("anim_badge", ctrl.id.as_str())),
                            egui::Sense::hover(),
                        )
                        .on_hover_text(format!("Animations set:\n{anim_summary}"));
                    }
                }

                // Selection border for every selected control (the engine draws
                // faces unselected; the editor owns the selection chrome). Handles
                // and secondary highlights are drawn further below.
                for sid in &selected_ids {
                    if let (Some(crect), Some(ctrl)) =
                        (control_rects.get(sid), self.form.find_control(sid))
                    {
                        let corner = cobolt_forms::paint::corner_radius(ctrl);
                        painter.rect_stroke(
                            *crect,
                            corner,
                            Stroke::new(2.0, Color32::from_rgba_premultiplied(60, 120, 230, 255)),
                        );
                    }
                }

                // Refresh all databindings (so DataGrids get updated Rows etc. live in canvas too)
                // + special array seeding for counts + per-row preview_state for ghosts.
                refresh_data_binding_target_properties(&mut self.form);
                {
                    let array_bindings: Vec<_> = self
                        .form
                        .data_bindings
                        .iter()
                        .filter(|b| {
                            matches!(&b.target, BindingTargetDescriptor::ControlArray { .. })
                        })
                        .cloned()
                        .collect();
                    for b in &array_bindings {
                        seed_control_array_binding_preview_values(self, b);
                    }
                }

                // ── Design-time preview clones for repeating groups (spec 015) ──
                // Render-only ghosts of each top-level repeating GroupBox + its
                // subtree, laid out per LayoutDirection. They never enter the form
                // model (no selection/undo impact); v1 previews top-level groups.
                {
                    let controls = &self.form.controls;
                    let ro = super::containers::render_order(controls);
                    for gi in 0..controls.len() {
                        let g = &controls[gi];
                        if !matches!(g.control_type, ControlType::GroupBox) {
                            continue;
                        }
                        if g.parent.is_some() {
                            continue;
                        }
                        if !g
                            .get_prop("IsRepeatingGroup")
                            .map(|v| v.as_bool())
                            .unwrap_or(false)
                        {
                            continue;
                        }
                        let n = g
                            .get_prop("PreviewItemCount")
                            .map(|v| v.as_i64())
                            .unwrap_or(1)
                            .clamp(1, 50);
                        if n <= 1 {
                            continue;
                        }
                        let effect = Self::repeating_group_placement_effect(g);
                        let spacing = g
                            .get_prop("ItemSpacing")
                            .map(|v| v.as_i64())
                            .unwrap_or(8)
                            .max(0) as f32;
                        let layout = g
                            .get_prop("LayoutDirection")
                            .map(|v| v.as_str().to_owned())
                            .unwrap_or_else(|| "Vertical".into());
                        let ipr = g
                            .get_prop("ItemsPerRow")
                            .map(|v| v.as_i64())
                            .unwrap_or(1)
                            .max(1);
                        let gw = g.rect.w as f32;
                        let gh = g.rect.h as f32;
                        let subtree: Vec<usize> = std::iter::once(gi)
                            .chain(super::containers::collect_descendants(controls, gi))
                            .collect();
                        let g_content = g.content_rect();
                        let effect_start = self.placement_release_starts.get(&g.id).copied();
                        let now = ui.ctx().input(|i| i.time);
                        for k in 1..n {
                            let (dx, dy) = match layout.as_str() {
                                "Horizontal" => ((k as f32) * (gw + spacing), 0.0),
                                "Grid" => {
                                    let col = (k % ipr) as f32;
                                    let row = (k / ipr) as f32;
                                    (col * (gw + spacing), row * (gh + spacing))
                                }
                                _ /* Vertical */ => (0.0, (k as f32) * (gh + spacing)),
                            };
                            let mut card_scale = 1.0;
                            let mut card_alpha = 1.0;
                            let mut card_shift = Vec2::ZERO;
                            let mut root_center = None;
                            if effect != PlacementEffect::None {
                                if let Some(start) = effect_start {
                                    let root_screen = egui::Rect::from_min_size(
                                        origin
                                            + Vec2::new(g.rect.x as f32 + dx, g.rect.y as f32 + dy),
                                        Vec2::new(gw.max(0.0), gh.max(0.0)),
                                    );
                                    let clipped = !painter.clip_rect().intersects(root_screen);
                                    let dur = g
                                        .get_prop("CardAppearDuration")
                                        .map(|v| v.as_i64() as f32 / 1000.0)
                                        .unwrap_or(0.2);
                                    let elapsed = (now - start).max(0.0) as f32;
                                    let (tf, animating) = card_appear_transform(
                                        effect,
                                        1,
                                        elapsed,
                                        (-(dx), -(dy)),
                                        clipped,
                                        dur,
                                    );
                                    card_scale = tf.scale;
                                    card_alpha = tf.alpha;
                                    root_center = Some(root_screen.center());
                                    card_shift = Vec2::new(tf.dx, tf.dy);
                                    if animating {
                                        ui.ctx().request_repaint();
                                    }
                                }
                            }
                            // Clip descendants to the shifted group's content area.
                            let gclip = egui::Rect::from_min_size(
                                origin
                                    + card_shift
                                    + Vec2::new(g_content.x as f32 + dx, g_content.y as f32 + dy),
                                Vec2::new(g_content.w as f32, g_content.h as f32),
                            );
                            // Group frame first (behind), then its subtree in z-order.
                            for &si in std::iter::once(&gi)
                                .chain(ro.iter().filter(|i| **i != gi && subtree.contains(i)))
                            {
                                let mut clone = controls[si].clone();
                                clone.rect.x += dx as i32;
                                clone.rect.y += dy as i32;
                                // For ControlArray + databinding: inject preview row values into
                                // the ghost clones using the same instanced id scheme as expand + seed.
                                if si != gi {
                                    let logical_inst = (k + 1) as usize;
                                    let base_mid = &controls[si].id;
                                    let inst_id = if logical_inst <= 1 {
                                        base_mid.clone()
                                    } else {
                                        format!("{}#{}", base_mid, logical_inst)
                                    };
                                    if let Some(val) = self.preview_state.get(&inst_id) {
                                        let pkey = match clone.control_type {
                                            ControlType::TextBox => "Text",
                                            ControlType::PictureBox => "ImagePath",
                                            ControlType::CheckBox | ControlType::RadioButton => {
                                                "Checked"
                                            }
                                            ControlType::ComboBox
                                            | ControlType::ListBox
                                            | ControlType::Slider
                                            | ControlType::ProgressBar
                                            | ControlType::NumericUpDown
                                            | ControlType::DateTimePicker => "Value",
                                            _ => "Caption",
                                        };
                                        clone.set_prop(
                                            pkey.to_string(),
                                            PropValue::String(val.clone()),
                                        );
                                    }
                                }
                                let dp = if si == gi {
                                    painter.clone()
                                } else {
                                    painter.with_clip_rect(painter.clip_rect().intersect(gclip))
                                };
                                let control_screen = egui::Rect::from_min_size(
                                    origin + Vec2::new(clone.rect.x as f32, clone.rect.y as f32),
                                    Vec2::new(clone.rect.w as f32, clone.rect.h as f32),
                                );
                                let grouped_shift = if let Some(root_center) = root_center {
                                    card_shift
                                        + (control_screen.center() - root_center)
                                            * (card_scale - 1.0)
                                } else {
                                    card_shift
                                };
                                draw_control(
                                    &dp,
                                    origin + grouped_shift,
                                    &clone,
                                    false,
                                    self.glass_mode,
                                    0.45 * card_alpha,
                                    card_scale,
                                    None,
                                );
                            }
                        }
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
                        painter.rect_stroke(
                            rect,
                            2.0,
                            Stroke::new(1.5, Color32::from_rgba_premultiplied(100, 200, 255, 200)),
                        );
                    }
                }

                // Draw rubber-band rectangle
                if let DragState::RubberBand {
                    start_x,
                    start_y,
                    cur_x,
                    cur_y,
                } = self.drag
                {
                    let x0 = start_x.min(cur_x) as f32 + origin.x;
                    let y0 = start_y.min(cur_y) as f32 + origin.y;
                    let x1 = start_x.max(cur_x) as f32 + origin.x;
                    let y1 = start_y.max(cur_y) as f32 + origin.y;
                    let band_rect = egui::Rect::from_min_max(Pos2::new(x0, y0), Pos2::new(x1, y1));
                    painter.rect_filled(
                        band_rect,
                        0.0,
                        Color32::from_rgba_premultiplied(80, 140, 255, 30),
                    );
                    painter.rect_stroke(
                        band_rect,
                        0.0,
                        Stroke::new(1.0, Color32::from_rgba_premultiplied(100, 170, 255, 220)),
                    );
                }

                // Right-click context menu
                resp.context_menu(|ui| {
                    let tr = crate::i18n::current_tr(ui.ctx());
                    if let Some(group_id) = self.selected_groupbox_id() {
                        if ui.button(tr.uc_create).clicked() {
                            self.create_user_control = Some(UserControlCreateDialog {
                                group_id,
                                name: String::new(),
                                error: None,
                            });
                            ui.close_menu();
                        }
                        ui.separator();
                    }
                    if !user_controls.is_empty() {
                        ui.menu_button(tr.uc_delete, |ui| {
                            for def in user_controls {
                                if ui.button(&def.name).clicked() {
                                    result.user_control_delete_requested = Some(def.name.clone());
                                    ui.close_menu();
                                }
                            }
                        });
                        ui.separator();
                    }
                    let has_selection = !self.selected_ids.is_empty();
                    let has_clipboard = clipboard
                        .as_ref()
                        .map(|clip| !clip.controls.is_empty())
                        .unwrap_or(false);
                    if ui
                        .add_enabled(
                            has_selection,
                            egui::Button::new(format!("{}  ⌘X", tr.clipboard_cut)),
                        )
                        .clicked()
                    {
                        self.cut_selected(clipboard);
                        selection_changed = true;
                        ui.close_menu();
                    }
                    if ui
                        .add_enabled(
                            has_selection,
                            egui::Button::new(format!("{}  ⌘C", tr.clipboard_copy)),
                        )
                        .clicked()
                    {
                        self.copy_selected(clipboard);
                        ui.close_menu();
                    }
                    if ui
                        .add_enabled(
                            has_clipboard,
                            egui::Button::new(format!("{}  ⌘V", tr.clipboard_paste)),
                        )
                        .clicked()
                    {
                        self.paste_from_clipboard(clipboard);
                        selection_changed = true;
                        ui.close_menu();
                    }
                    if ui
                        .add_enabled(
                            has_selection,
                            egui::Button::new(format!("{}  ⌘D", tr.clipboard_duplicate)),
                        )
                        .clicked()
                    {
                        self.duplicate_selected(clipboard);
                        selection_changed = true;
                        ui.close_menu();
                    }
                    ui.separator();
                    if ui.button("🗑 Delete").clicked() {
                        self.delete_selected();
                        ui.close_menu();
                    }
                    ui.separator();
                    if ui.button("⬆ Bring to Front").clicked() {
                        self.bring_to_front();
                        ui.close_menu();
                    }
                    if ui.button("⬇ Send to Back").clicked() {
                        self.send_to_back();
                        ui.close_menu();
                    }
                    if ui.button("+1 Forward").clicked() {
                        self.bring_forward();
                        ui.close_menu();
                    }
                    if ui.button("-1 Backward").clicked() {
                        self.send_backward();
                        ui.close_menu();
                    }
                    ui.separator();
                    // Play animations
                    let anim_preview: Option<(String, Vec<String>)> =
                        self.selected_ids.first().cloned().and_then(|sid| {
                            self.form.find_control(&sid).and_then(|ctrl| {
                                if ctrl.animations.is_empty() {
                                    None
                                } else {
                                    Some((
                                        sid,
                                        ctrl.animations.iter().map(|a| a.name.clone()).collect(),
                                    ))
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
                    // Repeating-group toggle for a single selected GroupBox (spec 015).
                    let gb_rep: Option<(String, bool)> = if self.selected_ids.len() == 1 {
                        let sid = self.selected_ids[0].clone();
                        self.form.find_control(&sid).and_then(|c| {
                            if matches!(c.control_type, ControlType::GroupBox) {
                                Some((
                                    sid,
                                    c.get_prop("IsRepeatingGroup")
                                        .map(|v| v.as_bool())
                                        .unwrap_or(false),
                                ))
                            } else {
                                None
                            }
                        })
                    } else {
                        None
                    };
                    if let Some((gid, is_rep)) = gb_rep {
                        ui.separator();
                        if is_rep {
                            if ui.button("▦ Unset Repeating Group").clicked() {
                                let old = self
                                    .form
                                    .find_control(&gid)
                                    .and_then(|c| c.get_prop("IsRepeatingGroup").cloned());
                                self.apply(Cmd::SetProperty {
                                    id: gid.clone(),
                                    key: "IsRepeatingGroup".into(),
                                    old,
                                    new: PropValue::Bool(false),
                                });
                                ui.close_menu();
                            }
                        } else if ui.button("▦ Set as Repeating Group").clicked() {
                            // Seed ArrayName with the control id when still empty.
                            let cur_an = self
                                .form
                                .find_control(&gid)
                                .and_then(|c| {
                                    c.get_prop("ArrayName").map(|v| v.as_str().to_owned())
                                })
                                .unwrap_or_default();
                            if cur_an.is_empty() {
                                let old_an = self
                                    .form
                                    .find_control(&gid)
                                    .and_then(|c| c.get_prop("ArrayName").cloned());
                                self.apply(Cmd::SetProperty {
                                    id: gid.clone(),
                                    key: "ArrayName".into(),
                                    old: old_an,
                                    new: PropValue::String(gid.clone()),
                                });
                            }
                            let old = self
                                .form
                                .find_control(&gid)
                                .and_then(|c| c.get_prop("IsRepeatingGroup").cloned());
                            self.apply(Cmd::SetProperty {
                                id: gid.clone(),
                                key: "IsRepeatingGroup".into(),
                                old,
                                new: PropValue::Bool(true),
                            });
                            ui.close_menu();
                        }
                    }
                    ui.separator();
                    if ui.button("🏷 Auto-arrange Labels").clicked() {
                        self.auto_arrange_labels();
                        ui.close_menu();
                    }
                });

                // Click on canvas — select / deselect
                if resp.clicked() {
                    let ctrl_held = ui.ctx().input(|i| i.modifiers.command);
                    if let Some((cx, cy)) = ptr_canvas {
                        // A click on a TabControl's tab strip switches its active
                        // page (spec 012) instead of selecting a child.
                        if let Some((tab_id, ti)) = self.tab_strip_hit(cx, cy) {
                            let old = self
                                .form
                                .find_control(&tab_id)
                                .and_then(|c| c.get_prop("SelectedTab").cloned());
                            self.apply(Cmd::SetProperty {
                                id: tab_id.clone(),
                                key: "SelectedTab".into(),
                                old,
                                new: PropValue::Int(ti as i64),
                            });
                            self.active_tabs.insert(tab_id.clone(), ti);
                            self.set_selected_one(Some(tab_id));
                            selection_changed = true;
                        } else {
                            // Hit-test topmost visible control (container-aware, spec 012).
                            let hit: Option<String> = self.hit_top_id(cx, cy);
                            if ctrl_held {
                                // Ctrl+click = toggle in multi-select
                                if let Some(id) = hit {
                                    self.toggle_selected(&id);
                                    selection_changed = true;
                                }
                            } else {
                                let hit_same = self.selected_ids.len() == 1
                                    && hit.as_deref()
                                        == self.selected_ids.first().map(|s| s.as_str());
                                if !hit_same {
                                    self.set_selected_one(hit);
                                    selection_changed = true;
                                }
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
        // Guard: only fire when no text-input control has keyboard focus (i.e. the user is
        // not editing a property field, animation name, etc. in the properties panel).
        let no_text_focus = ctx.memory(|m| m.focused().is_none());
        let want_delete = no_text_focus
            && !self.selected_ids.is_empty()
            && ctx.input(|i| {
                (i.key_pressed(egui::Key::Delete) || i.key_pressed(egui::Key::Backspace))
                    && !i.modifiers.command // don't eat Cmd+Backspace (system shortcuts)
            });
        if want_delete {
            self.delete_selected();
        }
        if ctx.input(|i| i.key_pressed(egui::Key::Z) && i.modifiers.command && !i.modifiers.shift) {
            self.undo();
        }
        if ctx.input(|i| i.key_pressed(egui::Key::Y) && i.modifiers.command) {
            self.redo();
        }
        if ctx.input(|i| i.key_pressed(egui::Key::Z) && i.modifiers.command && i.modifiers.shift) {
            self.redo();
        }
        if no_text_focus && ctx.input(|i| i.key_pressed(egui::Key::A) && i.modifiers.command) {
            // Select all — but only when not editing a property textbox etc.
            self.selected_ids = self.form.controls.iter().map(|c| c.id.clone()).collect();
            selection_changed = true;
        }
        if no_text_focus
            && !self.selected_ids.is_empty()
            && ctx.input(|i| i.key_pressed(egui::Key::C) && i.modifiers.command)
        {
            self.copy_selected(clipboard);
        }
        if no_text_focus
            && !self.selected_ids.is_empty()
            && ctx.input(|i| i.key_pressed(egui::Key::X) && i.modifiers.command)
        {
            self.cut_selected(clipboard);
            selection_changed = true;
        }
        if no_text_focus
            && !self.selected_ids.is_empty()
            && ctx.input(|i| i.key_pressed(egui::Key::D) && i.modifiers.command)
        {
            self.duplicate_selected(clipboard);
            selection_changed = true;
        }
        if no_text_focus && ctx.input(|i| i.key_pressed(egui::Key::V) && i.modifiers.command) {
            self.paste_from_clipboard(clipboard);
            selection_changed = true;
        }

        // ── Deletion confirmation (spec 020) ─────────────────────────────────
        self.show_delete_confirmation(ui);

        // ── User Control creation (spec 020) ─────────────────────────────────
        result.user_control_created = self.show_user_control_create_dialog(ui, user_controls);

        // ── Menu Editor Modal (spec 018) ──────────────────────────────────────
        self.show_menu_editor(ui);

        // ── Event Editor Modal ──────────────────────────────────────────────────
        self.show_event_modal(ui, llm_cfg);

        result.selection_changed |= selection_changed;
        result
    }

    fn show_delete_confirmation(&mut self, ui: &mut Ui) {
        let Some(pending) = self.pending_delete.clone() else {
            return;
        };
        let tr = crate::i18n::current_tr(ui.ctx());
        let message = tr
            .delete_confirm_message
            .replace("{controls}", &pending.control_count.to_string())
            .replace("{handlers}", &pending.event_count.to_string());
        let mut cancel = false;
        let mut confirm = false;

        egui::Window::new(tr.delete_confirm_title)
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO)
            .show(ui.ctx(), |ui| {
                ui.label(message);
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    if ui.button(tr.delete_confirm_cancel).clicked() {
                        cancel = true;
                    }
                    if ui.button(tr.delete_confirm_ok).clicked() {
                        confirm = true;
                    }
                });
            });

        if cancel {
            self.pending_delete = None;
        }
        if confirm {
            self.pending_delete = None;
            self.delete_ids_now(&pending.control_ids);
        }
    }

    fn show_user_control_create_dialog(
        &mut self,
        ui: &mut Ui,
        user_controls: &[UserControlDef],
    ) -> Option<UserControlDef> {
        let mut dialog = self.create_user_control.take()?;
        let tr = crate::i18n::current_tr(ui.ctx());
        let mut cancel = false;
        let mut confirm = false;

        egui::Window::new(tr.uc_create)
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO)
            .show(ui.ctx(), |ui| {
                ui.label(tr.uc_name_prompt);
                ui.text_edit_singleline(&mut dialog.name);
                if let Some(error) = dialog.error {
                    let text = match error {
                        UserControlNameError::Empty | UserControlNameError::Invalid => {
                            tr.uc_name_invalid
                        }
                        UserControlNameError::Duplicate => tr.uc_name_duplicate,
                        UserControlNameError::Circular => tr.uc_circular_ref,
                    };
                    ui.colored_label(ui.visuals().error_fg_color, text);
                }
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    if ui.button(tr.btn_cancel).clicked() {
                        cancel = true;
                    }
                    if ui.button(tr.btn_save).clicked() {
                        confirm = true;
                    }
                });
            });

        if cancel {
            return None;
        }

        if !confirm {
            self.create_user_control = Some(dialog);
            return None;
        }

        let group_id = dialog.group_id.clone();
        let name = dialog.name.trim().to_string();
        let existing_names: Vec<String> =
            user_controls.iter().map(|def| def.name.clone()).collect();
        match Self::validate_user_control_name(&name, &existing_names) {
            Ok(()) => {
                let Some(def) = self.capture_user_control_def(&group_id, name) else {
                    return None;
                };
                if Self::has_circular_user_control_reference(
                    &def.name,
                    &def.controls,
                    user_controls,
                ) {
                    dialog.error = Some(UserControlNameError::Circular);
                    self.create_user_control = Some(dialog);
                    None
                } else {
                    Some(def)
                }
            }
            Err(error) => {
                dialog.error = Some(error);
                self.create_user_control = Some(dialog);
                None
            }
        }
    }

    /// Render the menu tree editor modal (spec 018).
    fn show_menu_editor(&mut self, ui: &mut Ui) {
        if self.menu_modal.is_none() {
            return;
        }

        let overlay = ui.ctx().screen_rect();
        ui.painter()
            .rect_filled(overlay, 0.0, Color32::from_rgba_premultiplied(0, 0, 0, 140));

        let mut save_clicked = false;
        let mut cancel_clicked = false;

        let screen = ui.ctx().screen_rect();
        let tr = crate::i18n::current_tr(ui.ctx());

        let modal_id = egui::Id::new("menu_editor_modal");

        egui::Window::new(tr.menu_editor_title)
            .id(modal_id)
            .collapsible(false)
            .resizable(true)
            .default_size([800.0, 500.0])
            .default_pos(egui::Pos2::new(
                screen.center().x - 400.0,
                screen.center().y - 250.0,
            ))
            .frame(egui::Frame::window(&ui.ctx().style()).inner_margin(egui::Margin::same(12.0)))
            .show(ui.ctx(), |ui| {
                let modal = self.menu_modal.as_mut().unwrap();

                // ── Toolbar (full width, at top) ─────────────────────────
                ui.horizontal_wrapped(|ui| {
                    if ui.small_button(tr.menu_add_item).clicked() {
                        let id = modal.next_id();
                        let item = cobolt_forms::menu::MenuItem::new_action(&id, "New Item");
                        let list =
                            MenuEditorModal::parent_list_mut(&mut modal.def, &modal.selected);
                        let idx = modal.selected.last().map(|&i| i + 1).unwrap_or(list.len());
                        list.insert(idx.min(list.len()), item);
                        if modal.selected.is_empty() {
                            modal.selected = vec![list.len() - 1];
                        } else {
                            *modal.selected.last_mut().unwrap() = idx.min(list.len() - 1);
                        }
                        modal.sync_bufs_from_selection();
                    }
                    if ui.small_button(tr.menu_add_submenu).clicked() {
                        if MenuEditorModal::depth_of(&modal.selected) < 2 {
                            let id = modal.next_id();
                            if let Some(parent) =
                                MenuEditorModal::item_at_mut(&mut modal.def.menu, &modal.selected)
                            {
                                parent.items.push(cobolt_forms::menu::MenuItem::new_action(
                                    &id, "Sub Item",
                                ));
                            }
                        }
                    }
                    if ui.small_button(tr.menu_add_separator).clicked() {
                        let id = modal.next_id();
                        let item = cobolt_forms::menu::MenuItem::new_separator(&id);
                        let list =
                            MenuEditorModal::parent_list_mut(&mut modal.def, &modal.selected);
                        let idx = modal.selected.last().map(|&i| i + 1).unwrap_or(list.len());
                        list.insert(idx.min(list.len()), item);
                    }
                    if ui.small_button(tr.menu_move_up).clicked() && !modal.selected.is_empty() {
                        let idx = *modal.selected.last().unwrap();
                        if idx > 0 {
                            let list =
                                MenuEditorModal::parent_list_mut(&mut modal.def, &modal.selected);
                            list.swap(idx, idx - 1);
                            *modal.selected.last_mut().unwrap() = idx - 1;
                        }
                    }
                    if ui.small_button(tr.menu_move_down).clicked() && !modal.selected.is_empty() {
                        let idx = *modal.selected.last().unwrap();
                        let list =
                            MenuEditorModal::parent_list_mut(&mut modal.def, &modal.selected);
                        if idx + 1 < list.len() {
                            list.swap(idx, idx + 1);
                            *modal.selected.last_mut().unwrap() = idx + 1;
                        }
                    }
                    if ui.small_button(tr.menu_delete).clicked() && !modal.selected.is_empty() {
                        let idx = *modal.selected.last().unwrap();
                        let list =
                            MenuEditorModal::parent_list_mut(&mut modal.def, &modal.selected);
                        if idx < list.len() {
                            list.remove(idx);
                            if list.is_empty() {
                                modal.selected.pop();
                            } else if idx >= list.len() {
                                *modal.selected.last_mut().unwrap() = list.len() - 1;
                            }
                            modal.sync_bufs_from_selection();
                        }
                    }
                });

                ui.separator();

                // ── Two-pane area ────────────────────────────────────────
                let pane_h = 350.0;
                let content_w = 776.0; // 800 - 24 (margins)
                let left_w = (content_w * modal.split_ratio).clamp(150.0, content_w - 150.0);
                let right_w = (content_w - left_w - 8.0).max(100.0);

                ui.horizontal(|ui| {
                    // ── Left pane: Menu treeview ──────────────────────────
                    ui.allocate_ui_with_layout(
                        egui::Vec2::new(left_w, pane_h),
                        egui::Layout::top_down(egui::Align::LEFT),
                        |ui| {
                            ui.label(egui::RichText::new("Menu treeview").strong());
                            ui.separator();

                            egui::ScrollArea::vertical()
                                .max_height(300.0)
                                .show(ui, |ui| {
                                    fn draw_tree(
                                        ui: &mut egui::Ui,
                                        items: &[cobolt_forms::menu::MenuItem],
                                        path: &mut Vec<usize>,
                                        selected: &[usize],
                                        depth: usize,
                                    ) -> Option<Vec<usize>> {
                                        let mut clicked = None;
                                        for (i, item) in items.iter().enumerate() {
                                            path.push(i);
                                            let is_sel = *path == selected;
                                            let indent = depth as f32 * 16.0;
                                            ui.horizontal(|ui| {
                                                ui.add_space(indent);
                                                let label = if item.item_type
                                                    == cobolt_forms::menu::MenuItemType::Separator
                                                {
                                                    "── separator ──".to_string()
                                                } else {
                                                    let icon_str =
                                                        item.icon.as_deref().unwrap_or("");
                                                    let _accel =
                                                        item.accelerator.as_deref().unwrap_or("");
                                                    let en =
                                                        if item.enabled { "" } else { " [off]" };
                                                    if icon_str.is_empty() {
                                                        format!("{}{}", item.label, en)
                                                    } else {
                                                        format!(
                                                            "[{}] {}{}",
                                                            icon_str, item.label, en
                                                        )
                                                    }
                                                };
                                                let resp = ui.selectable_label(
                                                    is_sel,
                                                    egui::RichText::new(&label).monospace(),
                                                );
                                                if resp.clicked() {
                                                    clicked = Some(path.clone());
                                                }
                                            });
                                            if !item.items.is_empty() {
                                                if let Some(c) = draw_tree(
                                                    ui,
                                                    &item.items,
                                                    path,
                                                    selected,
                                                    depth + 1,
                                                ) {
                                                    clicked = Some(c);
                                                }
                                            }
                                            path.pop();
                                        }
                                        clicked
                                    }

                                    let mut path = Vec::new();
                                    let selected = modal.selected.clone();
                                    if let Some(clicked) =
                                        draw_tree(ui, &modal.def.menu, &mut path, &selected, 0)
                                    {
                                        modal.selected = clicked;
                                        modal.sync_bufs_from_selection();
                                    }
                                });
                        },
                    );

                    // ── Draggable splitter ────────────────────────────────
                    {
                        let splitter_id = egui::Id::new("menu_editor_splitter");
                        let (splitter_rect, _) = ui.allocate_exact_size(
                            egui::Vec2::new(8.0, pane_h),
                            egui::Sense::hover(),
                        );
                        let resp = ui.interact(splitter_rect, splitter_id, egui::Sense::drag());
                        let active = resp.dragged() || resp.hovered();
                        let color = if active {
                            Color32::from_rgb(100, 160, 255)
                        } else {
                            Color32::from_rgb(80, 80, 100)
                        };
                        ui.painter().rect_filled(
                            egui::Rect::from_center_size(
                                splitter_rect.center(),
                                egui::Vec2::new(2.0, splitter_rect.height()),
                            ),
                            1.0,
                            color,
                        );
                        if resp.dragged() {
                            let delta = resp.drag_delta().x;
                            modal.split_ratio =
                                (modal.split_ratio + delta / content_w).clamp(0.2, 0.8);
                        }
                        if resp.hovered() || resp.dragged() {
                            ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeHorizontal);
                        }
                    }

                    // ── Right pane: Item properties ──────────────────────
                    ui.allocate_ui_with_layout(
                        egui::Vec2::new(right_w, pane_h),
                        egui::Layout::top_down(egui::Align::LEFT),
                        |ui| {
                            ui.label(egui::RichText::new("Item properties").strong());
                            ui.separator();
                            if let Some(item) =
                                MenuEditorModal::item_at(&modal.def.menu, &modal.selected)
                            {
                                let is_sep =
                                    item.item_type == cobolt_forms::menu::MenuItemType::Separator;
                                let cur_action_type =
                                    MenuEditorModal::action_type_of(item).to_string();
                                let cur_icon = item.icon.clone().unwrap_or_default();
                                let cur_enabled = item.enabled;

                                if !is_sep {
                                    // Label
                                    ui.horizontal(|ui| {
                                        ui.label(tr.menu_lbl_label);
                                        if ui
                                            .text_edit_singleline(&mut modal.label_buf)
                                            .lost_focus()
                                        {
                                            if let Some(it) = MenuEditorModal::item_at_mut(
                                                &mut modal.def.menu,
                                                &modal.selected,
                                            ) {
                                                it.label = modal.label_buf.clone();
                                            }
                                        }
                                    });

                                    // Icon — click to open picker, Delete to clear
                                    ui.horizontal(|ui| {
                                        ui.label(tr.menu_lbl_icon);
                                        if !cur_icon.is_empty() {
                                            let icon_rect =
                                                ui.allocate_space(egui::Vec2::splat(24.0)).1;
                                            cobolt_forms::icons::draw_menu_icon(
                                                ui.painter(),
                                                icon_rect,
                                                &cur_icon,
                                                Color32::WHITE,
                                            );
                                        }
                                        let display = if cur_icon.is_empty() {
                                            tr.menu_no_icon.to_string()
                                        } else {
                                            cur_icon.clone()
                                        };
                                        let resp = ui.button(&display);
                                        if resp.clicked()
                                            || (resp.has_focus()
                                                && ui.input(|i| i.key_pressed(egui::Key::Tab)))
                                        {
                                            modal.icon_picker_open = true;
                                            modal.icon_picker_gen += 1;
                                            modal.icon_search.clear();
                                        }
                                        if resp.has_focus()
                                            && ui.input(|i| {
                                                i.key_pressed(egui::Key::Delete)
                                                    || i.key_pressed(egui::Key::Backspace)
                                            })
                                        {
                                            if let Some(it) = MenuEditorModal::item_at_mut(
                                                &mut modal.def.menu,
                                                &modal.selected,
                                            ) {
                                                it.icon = None;
                                            }
                                        }
                                    });

                                    // Accelerator — key capture widget
                                    ui.horizontal(|ui| {
                                        ui.label(tr.menu_lbl_accel);
                                        let accel_id = egui::Id::new("menu_accel_capture");
                                        let is_capturing = ui
                                            .data(|d| d.get_temp::<bool>(accel_id))
                                            .unwrap_or(false);

                                        if is_capturing {
                                            let mut parts: Vec<String> = Vec::new();
                                            let mut final_key: Option<String> = None;
                                            ui.input(|i| {
                                                if i.modifiers.shift {
                                                    parts.push("Shift".into());
                                                }
                                                if i.modifiers.ctrl {
                                                    parts.push("Ctrl".into());
                                                }
                                                if i.modifiers.alt {
                                                    parts.push("Alt".into());
                                                }
                                                if i.modifiers.command {
                                                    parts.push("Cmd".into());
                                                }
                                                for ev in &i.events {
                                                    if let egui::Event::Key {
                                                        key,
                                                        pressed: true,
                                                        ..
                                                    } = ev
                                                    {
                                                        if *key == egui::Key::Escape {
                                                            final_key = Some("__ESC__".into());
                                                        } else {
                                                            final_key = Some(format!("{key:?}"));
                                                        }
                                                    }
                                                }
                                            });

                                            let display = if parts.is_empty() {
                                                "Press keys...".to_string()
                                            } else {
                                                parts
                                                    .iter()
                                                    .map(|p| format!("[{p}]"))
                                                    .collect::<Vec<_>>()
                                                    .join(" + ")
                                            };

                                            ui.label(
                                                egui::RichText::new(&display)
                                                    .monospace()
                                                    .color(Color32::YELLOW),
                                            );

                                            if let Some(key) = final_key {
                                                if key == "__ESC__" {
                                                    ui.data_mut(|d| d.insert_temp(accel_id, false));
                                                } else if !parts.is_empty() {
                                                    parts.push(key);
                                                    modal.accel_buf = parts.join("+");
                                                    if let Some(it) = MenuEditorModal::item_at_mut(
                                                        &mut modal.def.menu,
                                                        &modal.selected,
                                                    ) {
                                                        it.accelerator =
                                                            Some(modal.accel_buf.clone());
                                                    }
                                                    ui.data_mut(|d| d.insert_temp(accel_id, false));
                                                }
                                            }
                                        } else {
                                            let display = if modal.accel_buf.is_empty() {
                                                "(none)".to_string()
                                            } else {
                                                modal.accel_buf.clone()
                                            };
                                            if ui.button(&display).clicked() {
                                                ui.data_mut(|d| d.insert_temp(accel_id, true));
                                            }
                                            if !modal.accel_buf.is_empty() {
                                                if ui.small_button("✕").clicked() {
                                                    modal.accel_buf.clear();
                                                    if let Some(it) = MenuEditorModal::item_at_mut(
                                                        &mut modal.def.menu,
                                                        &modal.selected,
                                                    ) {
                                                        it.accelerator = None;
                                                    }
                                                }
                                            }
                                        }
                                    });

                                    // Action type
                                    ui.horizontal(|ui| {
                                        ui.label(tr.menu_lbl_action);
                                        let mut action_sel = cur_action_type.clone();
                                        egui::ComboBox::from_id_salt("menu_action_type")
                                            .selected_text(match action_sel.as_str() {
                                                "event" => tr.menu_action_event,
                                                "open-form" => tr.menu_action_open_form,
                                                "close" => tr.menu_action_close,
                                                _ => tr.menu_action_event,
                                            })
                                            .width(140.0)
                                            .show_ui(ui, |ui| {
                                                for (key, label) in [
                                                    ("event", tr.menu_action_event),
                                                    ("open-form", tr.menu_action_open_form),
                                                    ("close", tr.menu_action_close),
                                                ] {
                                                    if ui
                                                        .selectable_label(action_sel == key, label)
                                                        .clicked()
                                                    {
                                                        action_sel = key.to_string();
                                                        if let Some(it) =
                                                            MenuEditorModal::item_at_mut(
                                                                &mut modal.def.menu,
                                                                &modal.selected,
                                                            )
                                                        {
                                                            it.action = Some(match key {
                                                                "close" => {
                                                                    "close-application".to_string()
                                                                }
                                                                "event" => "event".to_string(),
                                                                "open-form" => format!(
                                                                    "open-form:{}",
                                                                    modal.target_buf
                                                                ),
                                                                _ => "event".to_string(),
                                                            });
                                                        }
                                                    }
                                                }
                                            });
                                    });

                                    // Form selector (for open-form action)
                                    if cur_action_type == "open-form" {
                                        ui.horizontal(|ui| {
                                            ui.label(tr.menu_lbl_target);
                                            let forms: Vec<String> = self
                                                .cfrm_dir
                                                .as_ref()
                                                .and_then(|dir| std::fs::read_dir(dir).ok())
                                                .map(|entries| {
                                                    entries
                                                        .filter_map(|e| {
                                                            let e = e.ok()?;
                                                            let name = e
                                                                .file_name()
                                                                .to_string_lossy()
                                                                .to_string();
                                                            if name.ends_with(".cfrm") {
                                                                Some(
                                                                    name.trim_end_matches(".cfrm")
                                                                        .to_string(),
                                                                )
                                                            } else {
                                                                None
                                                            }
                                                        })
                                                        .collect()
                                                })
                                                .unwrap_or_default();
                                            let cur_form = modal.target_buf.clone();
                                            egui::ComboBox::from_id_salt("menu_form_select")
                                                .selected_text(if cur_form.is_empty() {
                                                    "(select form)"
                                                } else {
                                                    &cur_form
                                                })
                                                .width(180.0)
                                                .show_ui(ui, |ui| {
                                                    for form_name in &forms {
                                                        if ui
                                                            .selectable_label(
                                                                cur_form == *form_name,
                                                                form_name,
                                                            )
                                                            .clicked()
                                                        {
                                                            modal.target_buf = form_name.clone();
                                                            if let Some(it) =
                                                                MenuEditorModal::item_at_mut(
                                                                    &mut modal.def.menu,
                                                                    &modal.selected,
                                                                )
                                                            {
                                                                it.action = Some(format!(
                                                                    "open-form:{}",
                                                                    form_name
                                                                ));
                                                            }
                                                        }
                                                    }
                                                });
                                        });
                                    }

                                    // Enabled
                                    ui.horizontal(|ui| {
                                        let mut en = cur_enabled;
                                        if ui.checkbox(&mut en, tr.menu_lbl_enabled).changed() {
                                            if let Some(it) = MenuEditorModal::item_at_mut(
                                                &mut modal.def.menu,
                                                &modal.selected,
                                            ) {
                                                it.enabled = en;
                                            }
                                        }
                                    });
                                } else {
                                    ui.label("── separator ──");
                                }
                            } else {
                                ui.colored_label(Color32::GRAY, "Select an item to edit");
                            }
                        },
                    );
                });

                ui.separator();
                // Save / Cancel
                ui.horizontal(|ui| {
                    if ui.button(tr.me_cancel).clicked() {
                        cancel_clicked = true;
                    }
                    if ui.button(tr.me_save).clicked() {
                        save_clicked = true;
                    }
                });
            });

        // ── Icon picker modal ─────────────────────────────────────────────
        if let Some(modal) = self.menu_modal.as_mut() {
            if modal.icon_picker_open {
                let mut icon_picked: Option<Option<String>> = None;

                let screen = ui.ctx().screen_rect();
                let picker_id = egui::Id::new(("icon_picker", modal.icon_picker_gen));
                egui::Window::new("Select Icon")
                    .id(picker_id)
                    .collapsible(false)
                    .resizable(true)
                    .default_size([600.0, 500.0])
                    .default_pos([screen.center().x - 300.0, screen.center().y - 250.0])
                    .frame(
                        egui::Frame::window(&ui.ctx().style())
                            .inner_margin(egui::Margin::same(12.0)),
                    )
                    .show(ui.ctx(), |ui| {
                        // Search field
                        ui.horizontal(|ui| {
                            ui.label("🔍");
                            ui.text_edit_singleline(&mut modal.icon_search);
                        });
                        ui.separator();

                        let search = modal.icon_search.to_ascii_lowercase();
                        let categories: &[(&str, &[&str])] = &[
                            (
                                "Document",
                                &[
                                    "doc-new",
                                    "doc-open",
                                    "doc-save",
                                    "doc-save-as",
                                    "doc-copy",
                                    "doc-blank",
                                    "doc-text",
                                    "doc-pdf",
                                    "doc-spreadsheet",
                                    "doc-stack",
                                ],
                            ),
                            (
                                "Edit",
                                &[
                                    "scissors",
                                    "clipboard-copy",
                                    "clipboard-paste",
                                    "pencil",
                                    "eraser",
                                    "pen",
                                    "brush",
                                    "type-text",
                                    "bold",
                                    "italic",
                                    "underline",
                                    "strikethrough",
                                ],
                            ),
                            (
                                "Navigation",
                                &[
                                    "arrow-left",
                                    "arrow-right",
                                    "arrow-up",
                                    "arrow-down",
                                    "chevron-left",
                                    "chevron-right",
                                    "chevron-up",
                                    "chevron-down",
                                    "home",
                                    "external-link",
                                ],
                            ),
                            (
                                "Action",
                                &[
                                    "plus", "minus", "check", "x-mark", "refresh", "sync",
                                    "download", "upload", "share", "export", "import", "link",
                                ],
                            ),
                            (
                                "UI/View",
                                &[
                                    "eye",
                                    "eye-off",
                                    "magnifier",
                                    "zoom-in",
                                    "zoom-out",
                                    "fullscreen",
                                    "collapse",
                                    "expand",
                                    "grid-view",
                                    "list-view",
                                ],
                            ),
                            (
                                "Communication",
                                &[
                                    "mail",
                                    "mail-open",
                                    "send",
                                    "inbox",
                                    "chat",
                                    "phone",
                                    "video",
                                    "bell",
                                    "bell-off",
                                    "at-sign",
                                ],
                            ),
                            (
                                "Social",
                                &[
                                    "heart",
                                    "star",
                                    "thumbs-up",
                                    "thumbs-down",
                                    "bookmark",
                                    "flag",
                                ],
                            ),
                            (
                                "People",
                                &[
                                    "user",
                                    "users",
                                    "user-plus",
                                    "user-minus",
                                    "user-check",
                                    "user-circle",
                                ],
                            ),
                            (
                                "Media",
                                &[
                                    "play",
                                    "pause",
                                    "stop",
                                    "skip-forward",
                                    "skip-back",
                                    "volume",
                                    "volume-off",
                                    "music",
                                ],
                            ),
                            (
                                "Data",
                                &[
                                    "database",
                                    "chart-bar",
                                    "chart-line",
                                    "chart-pie",
                                    "table",
                                    "filter",
                                    "sort-asc",
                                    "sort-desc",
                                ],
                            ),
                            (
                                "System",
                                &[
                                    "gear", "wrench", "shield", "lock", "unlock", "key",
                                    "terminal", "code", "bug", "cpu",
                                ],
                            ),
                            (
                                "Status",
                                &[
                                    "info-circle",
                                    "warning-triangle",
                                    "error-circle",
                                    "help-circle",
                                    "check-circle",
                                    "x-circle",
                                    "clock",
                                    "calendar",
                                ],
                            ),
                            (
                                "Commerce",
                                &["cart", "credit-card", "wallet", "receipt", "tag", "percent"],
                            ),
                            (
                                "File/Folder",
                                &[
                                    "folder",
                                    "folder-open",
                                    "folder-plus",
                                    "archive",
                                    "trash",
                                    "printer",
                                ],
                            ),
                            (
                                "Payroll",
                                &[
                                    "payroll-check",
                                    "payroll-schedule",
                                    "payroll-deduction",
                                    "payroll-bonus",
                                    "payroll-overtime",
                                    "payroll-tax",
                                    "payroll-slip",
                                    "payroll-direct-deposit",
                                    "payroll-timesheet",
                                    "payroll-hours",
                                    "payroll-employee",
                                    "payroll-benefits",
                                    "payroll-pension",
                                    "payroll-vacation",
                                    "payroll-sick-leave",
                                    "payroll-commission",
                                    "payroll-garnishment",
                                    "payroll-reimbursement",
                                    "payroll-w2",
                                    "payroll-1099",
                                    "payroll-ytd",
                                    "payroll-net-pay",
                                    "payroll-gross-pay",
                                    "payroll-withholding",
                                    "payroll-frequency",
                                ],
                            ),
                            (
                                "Receivables",
                                &[
                                    "invoice",
                                    "invoice-paid",
                                    "invoice-overdue",
                                    "invoice-draft",
                                    "invoice-send",
                                    "credit-memo",
                                    "debit-memo",
                                    "aging-report",
                                    "collection",
                                    "dunning-letter",
                                    "payment-received",
                                    "partial-payment",
                                    "advance-payment",
                                    "refund",
                                    "write-off",
                                    "bad-debt",
                                    "interest-charge",
                                    "statement",
                                    "customer-balance",
                                    "account-receivable",
                                    "open-items",
                                    "clearing",
                                    "remittance",
                                    "factoring",
                                    "credit-limit",
                                ],
                            ),
                            (
                                "Payments",
                                &[
                                    "payment-check",
                                    "payment-wire",
                                    "payment-ach",
                                    "payment-cash",
                                    "payment-pending",
                                    "payment-approved",
                                    "payment-rejected",
                                    "payment-recurring",
                                    "payment-split",
                                    "payment-batch",
                                    "payment-void",
                                    "payment-reversal",
                                    "vendor-payment",
                                    "bill-pay",
                                    "purchase-order",
                                    "expense-report",
                                    "petty-cash",
                                    "bank-transfer",
                                    "payment-gateway",
                                    "payment-terms",
                                    "early-discount",
                                    "payment-plan",
                                    "installment",
                                    "escrow",
                                    "disbursement",
                                ],
                            ),
                            (
                                "Stock Control",
                                &[
                                    "inventory",
                                    "warehouse",
                                    "stock-in",
                                    "stock-out",
                                    "stock-count",
                                    "stock-transfer",
                                    "stock-adjust",
                                    "stock-reserve",
                                    "stock-alert",
                                    "stock-reorder",
                                    "barcode",
                                    "qr-code",
                                    "pallet",
                                    "shelf",
                                    "bin-location",
                                    "lot-number",
                                    "serial-number",
                                    "expiry-date",
                                    "fifo",
                                    "lifo",
                                    "cycle-count",
                                    "physical-count",
                                    "stock-valuation",
                                    "safety-stock",
                                    "dead-stock",
                                ],
                            ),
                            (
                                "Transportation",
                                &[
                                    "truck",
                                    "truck-loading",
                                    "truck-delivery",
                                    "van",
                                    "ship",
                                    "ship-cargo",
                                    "airplane",
                                    "airplane-landing",
                                    "helicopter",
                                    "train",
                                    "railway",
                                    "container",
                                    "forklift",
                                    "crane",
                                    "anchor",
                                    "compass",
                                    "route",
                                    "highway",
                                    "bridge",
                                    "toll",
                                    "fuel-pump",
                                    "tire",
                                    "engine",
                                    "speedometer",
                                    "odometer",
                                ],
                            ),
                            (
                                "Logistics",
                                &[
                                    "package",
                                    "package-open",
                                    "package-check",
                                    "package-x",
                                    "package-search",
                                    "conveyor",
                                    "loading-dock",
                                    "dispatch",
                                    "tracking",
                                    "tracking-number",
                                    "delivery-time",
                                    "express",
                                    "fragile",
                                    "hazmat",
                                    "temperature",
                                    "weight-scale",
                                    "dimensions",
                                    "customs",
                                    "manifest",
                                    "bill-of-lading",
                                    "cross-dock",
                                    "last-mile",
                                    "return-shipment",
                                    "consolidation",
                                    "deconsolidation",
                                ],
                            ),
                            (
                                "Financial",
                                &[
                                    "dollar",
                                    "euro",
                                    "yen",
                                    "pound",
                                    "bitcoin",
                                    "coins",
                                    "money-bag",
                                    "piggy-bank",
                                    "vault",
                                    "safe",
                                    "bank",
                                    "atm",
                                    "exchange-rate",
                                    "stock-market",
                                    "bull-market",
                                    "bear-market",
                                    "dividend",
                                    "interest-rate",
                                    "mortgage",
                                    "loan",
                                    "audit",
                                    "ledger",
                                    "balance-sheet",
                                    "profit-loss",
                                    "cash-flow",
                                ],
                            ),
                            (
                                "Social Media",
                                &[
                                    "like",
                                    "dislike",
                                    "comment",
                                    "repost",
                                    "mention",
                                    "hashtag",
                                    "trending",
                                    "viral",
                                    "follower",
                                    "following",
                                    "profile",
                                    "bio",
                                    "story",
                                    "reel",
                                    "live-stream",
                                    "notification-dot",
                                    "verified",
                                    "influencer",
                                    "engagement",
                                    "reach",
                                    "post",
                                    "feed",
                                    "timeline",
                                    "dm",
                                    "group-chat",
                                ],
                            ),
                        ];

                        let cur_icon = MenuEditorModal::item_at(&modal.def.menu, &modal.selected)
                            .and_then(|i| i.icon.clone())
                            .unwrap_or_default();

                        egui::ScrollArea::vertical()
                            .max_height(400.0)
                            .show(ui, |ui| {
                                for (cat_name, icons) in categories {
                                    let filtered: Vec<&&str> = icons
                                        .iter()
                                        .filter(|n| {
                                            search.is_empty()
                                                || n.contains(&search)
                                                || cat_name.to_ascii_lowercase().contains(&search)
                                        })
                                        .collect();
                                    if filtered.is_empty() {
                                        continue;
                                    }

                                    ui.label(egui::RichText::new(*cat_name).strong().size(12.0));
                                    ui.horizontal_wrapped(|ui| {
                                        for &&name in &filtered {
                                            let is_sel = cur_icon == name;
                                            let (rect, resp) = ui.allocate_exact_size(
                                                egui::Vec2::splat(48.0),
                                                egui::Sense::click(),
                                            );
                                            if is_sel {
                                                ui.painter().rect_filled(
                                                    rect,
                                                    4.0,
                                                    Color32::from_rgb(60, 100, 200),
                                                );
                                            } else if resp.hovered() {
                                                ui.painter().rect_filled(
                                                    rect,
                                                    4.0,
                                                    Color32::from_rgb(60, 60, 80),
                                                );
                                            }
                                            let icon_rect = rect.shrink(4.0);
                                            let icon_color = if is_sel {
                                                Color32::WHITE
                                            } else {
                                                Color32::from_rgb(200, 210, 230)
                                            };
                                            cobolt_forms::icons::draw_menu_icon(
                                                ui.painter(),
                                                icon_rect,
                                                name,
                                                icon_color,
                                            );
                                            if resp.clicked() {
                                                icon_picked = Some(Some(name.to_string()));
                                            }
                                            resp.on_hover_text(name);
                                        }
                                    });
                                    ui.add_space(4.0);
                                }
                            });

                        ui.separator();
                        ui.horizontal(|ui| {
                            if ui.button("Close").clicked() {
                                modal.icon_picker_open = false;
                            }
                            if ui.button("Clear icon").clicked() {
                                icon_picked = Some(None);
                            }
                        });
                    });

                // ESC closes
                if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                    modal.icon_picker_open = false;
                }
                if let Some(picked) = icon_picked {
                    if let Some(it) =
                        MenuEditorModal::item_at_mut(&mut modal.def.menu, &modal.selected)
                    {
                        it.icon = picked;
                    }
                    modal.icon_picker_open = false;
                }
            }
        }

        if save_clicked {
            if let Some(modal) = self.menu_modal.take() {
                if let Some(dir) = &self.cfrm_dir {
                    let path = cobolt_forms::menu::menu_yaml_path(dir, &modal.ctrl_id);
                    if let Err(e) = cobolt_forms::menu::save_menu(&path, &modal.def) {
                        eprintln!("Failed to save menu: {e}");
                    } else {
                        cobolt_forms::paint::set_menu_cache(
                            ui.ctx(),
                            &modal.ctrl_id,
                            std::sync::Arc::new(
                                cobolt_forms::menu::load_menu(&path).unwrap_or_default(),
                            ),
                        );
                        self.dirty = true;
                    }
                }
            }
        }
        if cancel_clicked {
            self.menu_modal = None;
        }
    }

    /// Render the event code editor modal (if open).
    ///
    /// A single editable COBOL area holds the whole handler body (`ENVIRONMENT
    /// DIVISION` … `PROCEDURE DIVISION` + statements). The generator-owned
    /// `IDENTIFICATION DIVISION` / `PROGRAM-ID` header and the closing `GOBACK`
    /// / `END PROGRAM` are shown read-only around it.
    fn show_event_modal(&mut self, ui: &mut Ui, llm_cfg: &crate::llm::LlmConfig) {
        // Snapshot the scalar modal fields and drop the borrow so we can render
        // the hosted editor (`self.event_editor`) freely inside the window.
        let (title, program_id, ctrl_id, event_name, orig_source) = {
            let Some(m) = self.event_modal.as_ref() else {
                return;
            };
            (
                format!("COBOL Event Editor  —  {}", m.ctrl_display),
                m.program_id.clone(),
                m.ctrl_id.clone(),
                m.event_name.clone(),
                m.orig_source.clone(),
            )
        };
        let tr = crate::i18n::current_tr(ui.ctx());

        // ── Poll an in-flight AI request for this handler ────────────────────
        // On completion, splice the model's ```cobol block into the hosted
        // editor's buffer (or surface the error) so the developer can review /
        // tweak / save it like any hand-written handler.
        let mut ai_reply: Option<crate::llm::LlmResponse> = None;
        if let Some(m) = self.event_modal.as_ref() {
            if let Some(rx) = &m.ai_pending {
                match rx.try_recv() {
                    Ok(resp) => ai_reply = Some(resp),
                    Err(std::sync::mpsc::TryRecvError::Empty) => ui.ctx().request_repaint(),
                    Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                        ai_reply = Some(crate::llm::LlmResponse::Err(
                            "The assistant worker stopped unexpectedly.".into(),
                        ));
                    }
                }
            }
        }
        if let Some(resp) = ai_reply {
            if let Some(m) = self.event_modal.as_mut() {
                m.ai_pending = None;
            }
            match resp {
                crate::llm::LlmResponse::Ok(reply) => {
                    let code = crate::llm::extract_code(&reply).unwrap_or(reply);
                    self.event_editor.open_buffer(
                        std::path::PathBuf::from(format!("{program_id}.handler")),
                        code,
                    );
                    if let Some(m) = self.event_modal.as_mut() {
                        m.ai_status = None;
                    }
                }
                crate::llm::LlmResponse::Err(e) => {
                    if let Some(m) = self.event_modal.as_mut() {
                        m.ai_status = Some(e);
                    }
                }
            }
        }

        // UI-owned AI state, snapshotted so the window closure borrows locals.
        let busy = self
            .event_modal
            .as_ref()
            .map(|m| m.ai_pending.is_some())
            .unwrap_or(false);
        let ai_status = self.event_modal.as_ref().and_then(|m| m.ai_status.clone());
        let mut ai_prompt = self
            .event_modal
            .as_mut()
            .map(|m| std::mem::take(&mut m.ai_prompt))
            .unwrap_or_default();
        let mut do_send = false;

        // Dim overlay covering the canvas (behind the window).
        let overlay = ui.ctx().screen_rect();
        ui.painter()
            .rect_filled(overlay, 0.0, Color32::from_rgba_premultiplied(0, 0, 0, 140));

        let mut save_clicked = false;
        let mut cancel_clicked = false;

        // Open at 70 % of the window size; `default_*` only seed the initial
        // size, so the modal does not track the window — the user can resize.
        let screen = ui.ctx().screen_rect();
        let default_w = (screen.width() * 0.70).max(360.0);
        let default_h = (screen.height() * 0.70).max(420.0);

        egui::Window::new(&title)
            .id(egui::Id::new("event_editor_modal"))
            .collapsible(false)
            .resizable(true)
            .default_width(default_w)
            .default_height(default_h)
            .min_width(360.0)
            .min_height(420.0)
            .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
            .frame(egui::Frame::window(&ui.ctx().style()).inner_margin(egui::Margin::same(16.0)))
            .show(ui.ctx(), |ui| {
                let scaffold_color = Color32::from_rgb(140, 200, 140); // muted green
                let readonly_color = Color32::from_rgb(160, 170, 190); // subdued blue-gray

                // ── Status row at the TOP (line/col · INS/OVR · trim · beautify)
                self.event_editor.status_row(ui);
                ui.add_space(4.0);

                // ── Read-only scaffold header (generator-owned) ──────────────
                ui.monospace(
                    egui::RichText::new("       IDENTIFICATION DIVISION.")
                        .color(readonly_color)
                        .size(12.0),
                );
                ui.monospace(
                    egui::RichText::new(format!("       PROGRAM-ID. {}.", program_id))
                        .color(scaffold_color)
                        .size(12.0),
                );
                ui.add_space(4.0);

                // ── Hosted COBOL editor — a BOUNDED, user-resizable container. ──
                //    The editor fills and scrolls INSIDE this box; the box height
                //    is a fixed default and is changed ONLY by the user dragging
                //    the grip at its bottom edge. It is never derived from the
                //    window's available/max height, so neither the box nor the
                //    window can grow on their own: the modal opens at the default
                //    height and stays there until the user resizes it.
                //    (See `.claude/agents/egui-resize-guardian.md` — deriving a
                //    child's size from the parent's available space is exactly the
                //    feedback loop that makes "resizable" widgets self-inflate.)
                let editor_default_h = (default_h - 250.0).max(220.0);
                let editor_w = ui.available_width();
                let ectx = ui.ctx().clone();
                let theme = crate::theme::active();
                // A snug container (no outer gap) that fills the allocated box;
                // the editor scrolls *inside* it.
                let frame = egui::Frame::none()
                    .fill(theme.bg_extreme)
                    .stroke(egui::Stroke::new(1.0, theme.panel_border()))
                    .rounding(egui::Rounding::same(6.0))
                    .inner_margin(egui::Margin::same(2.0));
                egui::Resize::default()
                    .id_salt("event_editor_code_box")
                    .resizable([false, true])
                    .min_size(egui::vec2(editor_w, 160.0))
                    .max_size(egui::vec2(editor_w, 4000.0))
                    .default_size(egui::vec2(editor_w, editor_default_h))
                    .show(ui, |ui| {
                        let sz = ui.available_size();
                        ui.allocate_ui(sz, |ui| {
                            frame.show(ui, |ui| {
                                self.event_editor.render_code_area(&ectx, ui);
                            });
                        });
                    });

                ui.add_space(4.0);

                // ── Read-only GOBACK / END PROGRAM footer (generator-owned) ──
                ui.monospace(
                    egui::RichText::new("           GOBACK.")
                        .color(readonly_color)
                        .size(12.0),
                );
                ui.monospace(
                    egui::RichText::new(format!("       END PROGRAM {}.", program_id))
                        .color(scaffold_color)
                        .size(12.0),
                );

                // ── AI assistant prompt row (only when an LLM is configured) ─
                //    Ask the model to write / edit this handler's COBOL; the
                //    reply's ```cobol block replaces the editor buffer above.
                if llm_cfg.is_configured() {
                    ui.add_space(6.0);
                    ui.separator();
                    // Prompt box on the LEFT (multiline, vertically resizable via
                    // the grip at its bottom edge), Send button on the RIGHT.
                    let btn_col_w = 96.0;
                    let gap = 8.0;
                    let text_w = (ui.available_width() - btn_col_w - gap).max(140.0);
                    ui.horizontal_top(|ui| {
                        egui::Resize::default()
                            .id_salt("event_ai_prompt_box")
                            .resizable([false, true])
                            .min_size(egui::vec2(text_w, 40.0))
                            .max_size(egui::vec2(text_w, 320.0))
                            .default_size(egui::vec2(text_w, 64.0))
                            .show(ui, |ui| {
                                let resp = ui.add_sized(
                                    ui.available_size(),
                                    egui::TextEdit::multiline(&mut ai_prompt)
                                        .hint_text(tr.ai_prompt_placeholder)
                                        .interactive(!busy),
                                );
                                // Enter inserts a newline; ⌘/Ctrl+Enter submits.
                                let submit = resp.has_focus()
                                    && ui.input(|i| {
                                        i.key_pressed(egui::Key::Enter)
                                            && (i.modifiers.command || i.modifiers.ctrl)
                                    })
                                    && !ai_prompt.trim().is_empty();
                                if submit && !busy {
                                    do_send = true;
                                }
                            });
                        ui.add_space(gap);
                        ui.vertical(|ui| {
                            ui.label(egui::RichText::new("✨").size(15.0));
                            let can_send = !busy && !ai_prompt.trim().is_empty();
                            if ui
                                .add_enabled(can_send, egui::Button::new(tr.ai_send))
                                .clicked()
                            {
                                do_send = true;
                            }
                            if busy {
                                ui.add(egui::Spinner::new());
                                ui.label(
                                    egui::RichText::new(tr.ai_thinking)
                                        .small()
                                        .color(Color32::from_gray(170)),
                                );
                            }
                        });
                    });
                    if let Some(err) = &ai_status {
                        ui.label(
                            egui::RichText::new(err)
                                .small()
                                .color(Color32::from_rgb(220, 120, 120)),
                        );
                    }
                }

                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    if ui.button("💾  Save").clicked() {
                        save_clicked = true;
                    }
                    if ui.button("✖  Cancel").clicked() {
                        cancel_clicked = true;
                    }
                });
            });

        // Persist the (possibly edited) prompt draft back onto the modal.
        if let Some(m) = self.event_modal.as_mut() {
            m.ai_prompt = ai_prompt;
        }

        // Launch a handler-generation request on explicit submit only.
        if do_send && !busy {
            let code = self.event_editor.buffer_for_save().unwrap_or_default();
            let user_prompt = self
                .event_modal
                .as_ref()
                .map(|m| m.ai_prompt.clone())
                .unwrap_or_default();
            // Anchor the model to a nested-program handler body (the IDE owns the
            // IDENTIFICATION / PROGRAM-ID / GOBACK / END PROGRAM scaffold shown
            // read-only above and below the editor).
            let guided = format!(
                "{user_prompt}\n\nWrite the COBOL statements for this event handler only \
                 (a RustCOBOL nested-program body). Do NOT emit IDENTIFICATION DIVISION, \
                 PROGRAM-ID, GOBACK, or END PROGRAM — the IDE supplies those. Return the \
                 code in a ```cobol fenced block."
            );
            let rx = crate::llm::spawn_request(
                llm_cfg,
                &[],
                &guided,
                &code,
                &format!("{program_id}.cob"),
            );
            if let Some(m) = self.event_modal.as_mut() {
                m.ai_pending = Some(rx);
                m.ai_prompt.clear();
                m.ai_status = None;
            }
            ui.ctx().request_repaint();
        }

        if save_clicked {
            // Don't persist an untouched first-time template as real handler code.
            let content = self.event_editor.buffer_for_save().unwrap_or_default();
            if content != orig_source {
                self.save_event_handler(&ctrl_id, &event_name, content);
            }
            self.event_modal = None;
        } else if cancel_clicked {
            self.event_modal = None;
        }
    }

    /// COBOL Structure popup (spec 005): hosts the **same** `EditorPanel` used by
    /// the event modal and the main code editor, so the section / procedure code
    /// gets IntelliSense, syntax colouring and find/replace too. Edits live-sync
    /// back to the form block.
    pub fn show_cobol_structure_window(&mut self, ctx: &egui::Context, tr: &crate::i18n::Tr) {
        use super::cobol_structure as cs;
        let Some(target) = self.cobol_structure_edit else {
            return;
        };

        // (Re)load the selected block into the hosted editor when it changes.
        if self.cs_loaded != Some(target) {
            let text = cs::block_text(&self.form, target);
            self.cs_editor.open_buffer(
                std::path::PathBuf::from(format!("cobol-structure/{}", target.buffer_key())),
                text,
            );
            self.cs_editor.known_controls = super::editor::build_known_controls(&self.form);
            self.cs_loaded = Some(target);
        }

        let title = match target {
            cs::CsTarget::Procedure(i) => self
                .form
                .user_procedures
                .get(i)
                .map(|p| p.name.trim().to_owned())
                .filter(|s| !s.is_empty())
                .unwrap_or_else(|| tr.cs_user_procedures.to_owned()),
            other => other.section_keyword().unwrap_or("").to_owned(),
        };

        let screen = ctx.screen_rect();
        let default_w = (screen.width() * 0.6).max(420.0);
        let default_h = (screen.height() * 0.7).max(360.0);
        let mut close = false;

        egui::Window::new(format!("{} — {title}", tr.cs_open))
            .id(egui::Id::new("cobol_structure_window"))
            .collapsible(false)
            .resizable(true)
            .default_width(default_w)
            .default_height(default_h)
            .max_height(screen.height() * 0.7)
            .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
            .frame(egui::Frame::window(&ctx.style()).inner_margin(egui::Margin::same(14.0)))
            .show(ctx, |ui| {
                // Editable procedure name, or the fixed section keyword.
                if let cs::CsTarget::Procedure(i) = target {
                    if let Some(up) = self.form.user_procedures.get_mut(i) {
                        ui.horizontal(|ui| {
                            ui.label(tr.cs_proc_name);
                            if ui
                                .add(egui::TextEdit::singleline(&mut up.name).desired_width(260.0))
                                .changed()
                            {
                                self.dirty = true;
                            }
                        });
                    }
                } else {
                    ui.label(
                        egui::RichText::new(target.section_keyword().unwrap_or(""))
                            .monospace()
                            .strong(),
                    );
                }
                ui.label(egui::RichText::new(tr.cs_hint).weak().italics());
                ui.add_space(4.0);

                self.cs_editor.status_row(ui);
                ui.add_space(4.0);

                // Fixed-size container the editor fills and scrolls inside (a
                // height from `available_height` would creep on every repaint).
                let editor_h = (default_h - 170.0).max(200.0);
                let editor_w = ui.available_width();
                let ectx = ui.ctx().clone();
                let theme = crate::theme::active();
                let frame = egui::Frame::none()
                    .fill(theme.bg_extreme)
                    .stroke(egui::Stroke::new(1.0, theme.panel_border()))
                    .rounding(egui::Rounding::same(6.0))
                    .inner_margin(egui::Margin::same(2.0));
                ui.allocate_ui(egui::vec2(editor_w, editor_h), |ui| {
                    frame.show(ui, |ui| {
                        self.cs_editor.render_code_area(&ectx, ui);
                    });
                });

                ui.add_space(6.0);
                if ui.button(tr.cs_close).clicked() {
                    close = true;
                }
            });

        // Live-sync the edited buffer back to the form block.
        if let Some(content) = self.cs_editor.buffer_for_save() {
            if cs::set_block_text(&mut self.form, target, content) {
                self.dirty = true;
            }
        }
        if close {
            self.cobol_structure_edit = None;
            self.cs_loaded = None;
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
        let (px, py) = match ptr_canvas {
            Some(p) => p,
            None => return,
        };

        // ── Format Painter: intercept clicks while in WaitingForTarget mode ───
        if matches!(self.format_painter, FormatPainter::WaitingForTarget { .. }) {
            resp.ctx.set_cursor_icon(egui::CursorIcon::Crosshair);

            if resp.clicked() {
                // Find which control was clicked (topmost visible, container-aware).
                let hit_id: Option<String> = self.hit_top_id(px, py);
                if let Some(target_id) = hit_id {
                    // Extract captured style before mutably borrowing controls
                    let (props, animations, src_rect) =
                        match std::mem::replace(&mut self.format_painter, FormatPainter::Idle) {
                            FormatPainter::WaitingForTarget {
                                props,
                                animations,
                                src_rect,
                            } => (props, animations, src_rect),
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
            if resp.drag_started()
                || resp.dragged()
                || resp.drag_stopped()
                || resp.ctx.input(|i| i.pointer.primary_pressed())
            {
                return;
            }
        }

        // Check if pointer is currently over a resize handle (for cursor feedback).
        let handle_hover = self.selected_ids.first().and_then(|sid| {
            self.form.find_control(sid).and_then(|ctrl| {
                for &h in &ALL_HANDLES {
                    let hp = handle_pos(&ctrl.rect, h);
                    let dist = ((px as f32 - hp.x).powi(2) + (py as f32 - hp.y).powi(2)).sqrt();
                    if dist < 8.0 {
                        return Some(h);
                    }
                }
                None
            })
        });

        if let Some(h) = handle_hover {
            resp.ctx.set_cursor_icon(handle_cursor(h));
        }

        // Detect hovering the form's own resize border (only when not over a
        // control's resize handle — control handles take priority).
        let form_edge_hover = if handle_hover.is_none() {
            detect_form_edge(px, py, self.form.width as f32, self.form.height as f32)
        } else {
            None
        };
        if let Some(e) = form_edge_hover {
            resp.ctx.set_cursor_icon(form_edge_cursor(e));
        }

        // Capture which handle (if any) was under the pointer at the exact moment
        // the mouse button went down.  We must store this NOW because by the time
        // `drag_started()` fires the pointer has already moved away from the handle.
        // Guard with `resp.contains_pointer()` so clicks outside the canvas control
        // don't overwrite the stored value.
        if resp.contains_pointer() {
            let primary_just_pressed = resp.ctx.input(|i| i.pointer.primary_pressed());
            if primary_just_pressed {
                self.press_handle = handle_hover;
                self.press_form_edge = form_edge_hover;
            }
        }
        // Clear if the button is no longer held (cancelled press with no drag).
        let primary_held = resp.ctx.input(|i| i.pointer.primary_down());
        if !primary_held {
            self.press_handle = None;
            self.press_form_edge = None;
        }

        let primary_just_pressed =
            resp.contains_pointer() && resp.ctx.input(|i| i.pointer.primary_pressed());
        let begin_drag = resp.drag_started() || primary_just_pressed;

        // Begin drag immediately on mouse-down. Waiting for `drag_started()` makes
        // fast pointer motion outrun the selected control/tool before egui's drag
        // threshold is crossed.
        if begin_drag && matches!(&self.drag, DragState::None) {
            match self.drag.clone() {
                DragState::PlacingNew { .. } => {}
                _ => {
                    // Form-edge resize takes priority (captured at press-time).
                    if let Some(edge) = self.press_form_edge.take() {
                        self.drag = DragState::ResizingForm {
                            edge,
                            orig_w: self.form.width as i32,
                            orig_h: self.form.height as i32,
                            start_x: px,
                            start_y: py,
                        };
                    } else
                    // Use the handle captured at press-time, not the current hover
                    // (the pointer has already moved by the time drag_started fires).
                    if let Some(h) = self.press_handle.take() {
                        if let Some(sid) = self.selected_ids.first().cloned() {
                            if let Some(ctrl) = self.form.find_control(&sid) {
                                self.drag = DragState::ResizingControl {
                                    id: sid,
                                    handle: h,
                                    orig_rect: ctrl.rect,
                                    start_x: px,
                                    start_y: py,
                                };
                            }
                        }
                    } else {
                        // Hit-test for move (topmost visible, container-aware).
                        let hit_id: Option<String> = self.hit_top_id(px, py);
                        if let Some(id) = hit_id {
                            // If not already selected, select it (unless Ctrl held)
                            let ctrl_held = resp.ctx.input(|i| i.modifiers.command);
                            if !self.is_selected(&id) {
                                if ctrl_held {
                                    self.selected_ids.push(id.clone());
                                } else {
                                    self.set_selected_one(Some(id.clone()));
                                }
                                *selection_changed = true;
                            }
                            // Gather origins for the selected controls AND the
                            // descendants of any selected container, so a
                            // container drags its whole subtree (spec 012 R2).
                            let mut move_ids: Vec<String> = self.selected_ids.clone();
                            for sid in &self.selected_ids {
                                if let Some(i) =
                                    self.form.controls.iter().position(|c| &c.id == sid)
                                {
                                    for d in super::containers::collect_descendants(
                                        &self.form.controls,
                                        i,
                                    ) {
                                        let did = self.form.controls[d].id.clone();
                                        if !move_ids.contains(&did) {
                                            move_ids.push(did);
                                        }
                                    }
                                }
                            }
                            let origins: Vec<(String, i32, i32)> = move_ids
                                .iter()
                                .filter_map(|sid| {
                                    self.form
                                        .find_control(sid)
                                        .map(|c| (sid.clone(), c.rect.x, c.rect.y))
                                })
                                .collect();
                            self.drag = DragState::MovingControls {
                                primary_id: id,
                                origins,
                                start_x: px,
                                start_y: py,
                            };
                        } else {
                            // Started drag on empty canvas — begin rubber-band
                            self.drag = DragState::RubberBand {
                                start_x: px,
                                start_y: py,
                                cur_x: px,
                                cur_y: py,
                            };
                        }
                    }
                }
            }
        }

        // Update drag in-progress
        if resp.dragged() || (primary_held && !matches!(&self.drag, DragState::None)) {
            match self.drag.clone() {
                DragState::MovingControls {
                    origins,
                    start_x,
                    start_y,
                    ..
                } => {
                    let dx = px - start_x;
                    let dy = py - start_y;
                    let gp = self.form.grid_size as i32;
                    let sn = self.form.snap_to_grid;
                    for (id, ox, oy) in &origins {
                        if let Some(ctrl) = self.form.find_control_mut(id) {
                            // Anchored controls are locked against mouse dragging;
                            // X/Y can still be set via the property pane (keyboard).
                            if ctrl.is_anchored() {
                                continue;
                            }
                            ctrl.rect.x = snap(ox + dx, gp, sn);
                            ctrl.rect.y = snap(oy + dy, gp, sn);
                        }
                    }
                }
                DragState::ResizingControl {
                    ref id,
                    handle,
                    orig_rect,
                    start_x,
                    start_y,
                } => {
                    let dx = px - start_x;
                    let dy = py - start_y;
                    // Read snap settings before the mutable borrow of find_control_mut.
                    let gp = self.form.grid_size as i32;
                    let sn = self.form.snap_to_grid;
                    if let Some(ctrl) = self.form.find_control_mut(id) {
                        ctrl.rect = apply_resize(orig_rect, handle, dx, dy, gp, sn);
                    }
                }
                DragState::PlacingNew {
                    ref ctrl_type,
                    start_x,
                    start_y,
                    ..
                } => {
                    self.drag = DragState::PlacingNew {
                        ctrl_type: ctrl_type.clone(),
                        start_x,
                        start_y,
                        cur_x: px,
                        cur_y: py,
                    };
                    // Draw ghost preview
                    let x0 = start_x.min(px) as f32 + origin.x;
                    let y0 = start_y.min(py) as f32 + origin.y;
                    let x1 = start_x.max(px) as f32 + origin.x;
                    let y1 = start_y.max(py) as f32 + origin.y;
                    if (x1 - x0) > 4.0 && (y1 - y0) > 4.0 {
                        let ghost = egui::Rect::from_min_max(Pos2::new(x0, y0), Pos2::new(x1, y1));
                        painter.rect_filled(
                            ghost,
                            2.0,
                            Color32::from_rgba_premultiplied(80, 140, 255, 60),
                        );
                        painter.rect_stroke(
                            ghost,
                            2.0,
                            Stroke::new(1.5, Color32::from_rgb(80, 140, 255)),
                        );
                    }
                }
                DragState::RubberBand {
                    start_x, start_y, ..
                } => {
                    self.drag = DragState::RubberBand {
                        start_x,
                        start_y,
                        cur_x: px,
                        cur_y: py,
                    };
                }
                DragState::ResizingForm {
                    edge,
                    orig_w,
                    orig_h,
                    start_x,
                    start_y,
                } => {
                    let dx = px - start_x;
                    let dy = py - start_y;
                    let gp = self.form.grid_size as i32;
                    let sn = self.form.snap_to_grid;
                    if matches!(edge, FormEdge::Right | FormEdge::Corner) {
                        self.form.width = snap((orig_w + dx).max(FORM_MIN_SIZE), gp, sn) as u32;
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
        let primary_released = resp.ctx.input(|i| i.pointer.primary_released());
        if resp.drag_stopped() || (primary_released && !matches!(&self.drag, DragState::None)) {
            match self.drag.clone() {
                DragState::MovingControls {
                    origins,
                    start_x,
                    start_y,
                    ..
                } => {
                    let dx = px - start_x;
                    let dy = py - start_y;
                    if dx != 0 || dy != 0 {
                        let gp = self.form.grid_size as i32;
                        let sn = self.form.snap_to_grid;
                        // Anchored controls are locked against mouse dragging, so
                        // don't commit a moved position for them on release — this
                        // mirrors the in-drag skip above. X/Y stay editable via the
                        // property pane (keyboard).
                        let moves: Vec<(String, i32, i32, i32, i32)> = origins
                            .iter()
                            .filter(|(id, _, _)| {
                                !self
                                    .form
                                    .find_control(id)
                                    .map_or(false, |c| c.is_anchored())
                            })
                            .map(|(id, ox, oy)| {
                                (
                                    id.clone(),
                                    *ox,
                                    *oy,
                                    snap(ox + dx, gp, sn),
                                    snap(oy + dy, gp, sn),
                                )
                            })
                            .collect();
                        if !moves.is_empty() {
                            let changed_ids: Vec<String> =
                                moves.iter().map(|(id, ..)| id.clone()).collect();
                            self.apply(Cmd::MoveMany { moves });
                            // Re-parent the *selected* controls to whatever container
                            // their body now sits over — or back to the form (spec
                            // 012). Carried descendants keep their container.
                            for id in self.selected_ids.clone() {
                                self.reparent_to_drop(&id);
                            }
                            self.trigger_repeating_group_placement_release(&resp.ctx, &changed_ids);
                        }
                    }
                }
                DragState::ResizingControl {
                    id,
                    handle,
                    orig_rect,
                    start_x,
                    start_y,
                } => {
                    let dx = px - start_x;
                    let dy = py - start_y;
                    let new_rect = apply_resize(
                        orig_rect,
                        handle,
                        dx,
                        dy,
                        self.form.grid_size as i32,
                        self.form.snap_to_grid,
                    );
                    if new_rect != orig_rect {
                        self.apply(Cmd::ResizeControl {
                            id: id.clone(),
                            old_rect: orig_rect,
                            new_rect,
                        });
                        self.trigger_repeating_group_placement_release(&resp.ctx, &[id]);
                    }
                }
                DragState::PlacingNew {
                    ctrl_type,
                    start_x,
                    start_y,
                    cur_x,
                    cur_y,
                } => {
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
                            ctrl.rect.w = fw;
                            ctrl.rect.h = fh;
                        }
                        self.trigger_repeating_group_placement_release(&resp.ctx, &[sid]);
                    }
                }
                DragState::RubberBand {
                    start_x,
                    start_y,
                    cur_x,
                    cur_y,
                } => {
                    let min_x = start_x.min(cur_x);
                    let min_y = start_y.min(cur_y);
                    let max_x = start_x.max(cur_x);
                    let max_y = start_y.max(cur_y);
                    if (max_x - min_x) > 4 && (max_y - min_y) > 4 {
                        let ctrl_held = resp.ctx.input(|i| i.modifiers.command);
                        if !ctrl_held {
                            self.selected_ids.clear();
                        }
                        let new_sel: Vec<String> = self
                            .form
                            .controls
                            .iter()
                            .filter(|c| {
                                c.rect.x < max_x
                                    && c.rect.x + c.rect.w > min_x
                                    && c.rect.y < max_y
                                    && c.rect.y + c.rect.h > min_y
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
        self.drag = DragState::PlacingNew {
            ctrl_type: ct,
            start_x: x,
            start_y: y,
            cur_x: x,
            cur_y: y,
        };
    }

    /// Handle a control dragged out of the toolbox onto the canvas.
    ///
    /// The toolbox lives in a side panel, so a normal per-widget drag never reaches
    /// the canvas. Instead the toolbox stashes the control type as an egui
    /// `DragAndDrop` payload on drag-start; here we read it, draw a ghost preview at
    /// the (snapped) drop point while the pointer is over the canvas, and on mouse
    /// release add the control there. This is the drag counterpart to the
    /// click-to-centre path in `app.rs`.
    fn handle_toolbox_dnd(
        &mut self,
        ui: &Ui,
        canvas_rect: Rect,
        origin: Pos2,
        ptr_canvas: Option<(i32, i32)>,
        user_controls: &[UserControlDef],
    ) {
        enum ToolboxPayload {
            BuiltIn(ControlType),
            UserControl(UserControlDef),
        }

        let ctx = ui.ctx();
        let payload = if let Some(ct) =
            egui::DragAndDrop::payload::<ControlType>(ctx).map(|p| (*p).clone())
        {
            ToolboxPayload::BuiltIn(ct)
        } else if let Some(name) = egui::DragAndDrop::payload::<String>(ctx).map(|p| (*p).clone()) {
            let Some(def) = user_controls
                .iter()
                .find(|def| def.name.eq_ignore_ascii_case(&name))
                .cloned()
            else {
                return;
            };
            ToolboxPayload::UserControl(def)
        } else {
            return;
        };

        let (dw, dh, label) = match &payload {
            ToolboxPayload::BuiltIn(ct) => {
                let (w, h) = ct.default_size();
                (w, h, ct.as_str().to_string())
            }
            ToolboxPayload::UserControl(def) => (def.width, def.height, def.name.clone()),
        };

        if dw <= 0 || dh <= 0 {
            return;
        };
        // Repaint each frame so the ghost tracks the cursor smoothly.
        ctx.request_repaint();

        let over_canvas = ctx
            .pointer_interact_pos()
            .map(|p| canvas_rect.contains(p))
            .unwrap_or(false);
        let released = ctx.input(|i| i.pointer.any_released());

        // If the pointer is off the canvas, do nothing: when the button is released
        // off-canvas egui discards the payload on its own (no stray placement).
        let Some((px, py)) = ptr_canvas else {
            return;
        };
        if !over_canvas {
            return;
        }

        ctx.set_cursor_icon(CursorIcon::Grabbing);

        let gp = self.form.grid_size as i32;
        let sn = self.form.snap_to_grid;
        // Centre the control under the cursor, clamp into the form, then snap.
        let x = snap((px - dw / 2).max(0), gp, sn);
        let y = snap((py - dh / 2).max(0), gp, sn);

        if released {
            match payload {
                ToolboxPayload::BuiltIn(ct) => {
                    let _ = egui::DragAndDrop::take_payload::<ControlType>(ctx);
                    self.add_control(ct, x, y);
                }
                ToolboxPayload::UserControl(def) => {
                    let _ = egui::DragAndDrop::take_payload::<String>(ctx);
                    self.deploy_user_control(&def, x, y, user_controls);
                }
            }
            self.dirty = true;
            return;
        }

        // Ghost preview on a foreground layer so it sits above canvas + controls.
        let ghost = Rect::from_min_size(
            origin + Vec2::new(x as f32, y as f32),
            Vec2::new(dw as f32, dh as f32),
        );
        let painter = ctx.layer_painter(egui::LayerId::new(
            egui::Order::Foreground,
            egui::Id::new("toolbox_drop_ghost"),
        ));
        painter.rect_filled(
            ghost,
            2.0,
            Color32::from_rgba_premultiplied(80, 120, 220, 70),
        );
        painter.rect_stroke(
            ghost,
            2.0,
            Stroke::new(1.0, Color32::from_rgba_premultiplied(120, 160, 255, 220)),
        );
        painter.text(
            ghost.center(),
            egui::Align2::CENTER_CENTER,
            label,
            egui::FontId::proportional(11.0),
            Color32::WHITE,
        );
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

fn draw_grid_in_rounded_notches(
    painter: &egui::Painter,
    canvas: egui::Rect,
    rect: egui::Rect,
    rounding: egui::Rounding,
    step: f32,
    glass: bool,
) {
    if step <= 0.5 {
        return;
    }
    let cap = 0.5 * rect.width().min(rect.height());
    let clamp_r = |r: f32| r.max(0.0).min(cap);
    let radii = [
        clamp_r(rounding.nw),
        clamp_r(rounding.ne),
        clamp_r(rounding.se),
        clamp_r(rounding.sw),
    ];
    if radii.iter().all(|r| *r < 0.5) {
        return;
    }

    let alpha = if glass { 35 } else { 60 };
    let dot_color = Color32::from_rgba_premultiplied(140, 160, 220, alpha);
    let first_grid = |lo: f32, base: f32| base + ((lo - base) / step).ceil().max(0.0) * step;
    let painter = painter.with_clip_rect(canvas.intersect(rect));

    let in_notch = |p: Pos2| -> bool {
        let corners = [
            (
                radii[0],
                rect.min,
                Pos2::new(rect.min.x + radii[0], rect.min.y + radii[0]),
            ),
            (
                radii[1],
                Pos2::new(rect.max.x - radii[1], rect.min.y),
                Pos2::new(rect.max.x - radii[1], rect.min.y + radii[1]),
            ),
            (
                radii[2],
                Pos2::new(rect.max.x - radii[2], rect.max.y - radii[2]),
                Pos2::new(rect.max.x - radii[2], rect.max.y - radii[2]),
            ),
            (
                radii[3],
                Pos2::new(rect.min.x, rect.max.y - radii[3]),
                Pos2::new(rect.min.x + radii[3], rect.max.y - radii[3]),
            ),
        ];
        for (r, min, center) in corners {
            if r < 0.5 {
                continue;
            }
            let max = Pos2::new(min.x + r, min.y + r);
            if p.x >= min.x && p.x <= max.x && p.y >= min.y && p.y <= max.y {
                let d = p - center;
                if d.x * d.x + d.y * d.y >= r * r {
                    return true;
                }
            }
        }
        false
    };

    let mut x = first_grid(rect.min.x, canvas.min.x);
    while x <= rect.max.x {
        let mut y = first_grid(rect.min.y, canvas.min.y);
        while y <= rect.max.y {
            let p = Pos2::new(x, y);
            if in_notch(p) {
                painter.circle_filled(p, 0.7, dot_color);
            }
            y += step;
        }
        x += step;
    }
}

/// The readable type name used as the prefix of an auto-generated control ID
/// (`Button-1`, `TextBox-2`, …) and, uppercased, of its generated COBOL names.
fn control_type_name(ct: &ControlType) -> &'static str {
    use ControlType as CT;
    match ct {
        CT::Button => "Button",
        CT::Label => "Label",
        CT::TextBox => "TextBox",
        CT::CheckBox => "CheckBox",
        CT::RadioButton => "RadioButton",
        CT::ComboBox => "ComboBox",
        CT::ListBox => "ListBox",
        CT::PictureBox => "PictureBox",
        CT::Animator => "Animator",
        CT::GroupBox => "GroupBox",
        CT::Panel => "Panel",
        CT::TabControl => "TabControl",
        CT::ProgressBar => "ProgressBar",
        CT::DataGrid => "DataGrid",
        CT::MenuBar => "MenuBar",
        CT::ToolBar => "ToolBar",
        CT::StatusBar => "StatusBar",
        CT::Line => "Line",
        CT::DateTimePicker => "DateTimePicker",
        CT::NumericUpDown => "NumericUpDown",
        CT::TreeView => "TreeView",
        CT::Splitter => "Splitter",
        CT::Timer => "Timer",
        CT::Shape => "Shape",
        CT::AgentObject => "AgentObject",
        CT::RestClient => "RestClient",
        CT::Slider => "Slider",
        CT::SqlDatabase => "SqlDatabase",
        CT::BarChart => "BarChart",
        CT::LineChart => "LineChart",
        CT::PieChart => "PieChart",
        CT::AreaChart => "AreaChart",
        CT::ScatterChart => "ScatterChart",
        CT::DonutChart => "DonutChart",
        CT::Custom { .. } => "Control",
    }
}

fn unique_procedure_name(base: &str, reserved: &mut HashSet<String>) -> String {
    let base = base.trim();
    let base = if base.is_empty() { "HANDLER" } else { base };
    let mut candidate = base.to_owned();
    let mut suffix = 1usize;
    while reserved.contains(&candidate.to_ascii_uppercase()) {
        candidate = format!("{base}-{suffix}");
        suffix += 1;
    }
    reserved.insert(candidate.to_ascii_uppercase());
    candidate
}

fn remap_control_reference_props(ctrl: &mut Control, id_map: &HashMap<String, String>) {
    if let Some(value) = ctrl.get_prop("LabelFor") {
        let old = value.as_str();
        if let Some(new) = id_map
            .iter()
            .find(|(source, _)| source.eq_ignore_ascii_case(old))
            .map(|(_, target)| target.clone())
        {
            ctrl.set_prop("LabelFor", PropValue::String(new));
        }
    }
}

fn is_cobol_id_byte(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'-' || b == b'_'
}

fn rename_control_refs_in_cobol(code: &mut String, old: &str, new: &str) {
    let old_up = old.to_ascii_uppercase();
    let hay_up = code.to_ascii_uppercase();
    if old_up.is_empty() || !hay_up.contains(&old_up) {
        return;
    }

    let bytes = code.as_bytes();
    let hay = hay_up.as_bytes();
    let needle = old_up.as_bytes();
    let mut out = String::with_capacity(code.len());
    let mut i = 0usize;
    let mut last = 0usize;
    while i + needle.len() <= bytes.len() {
        if &hay[i..i + needle.len()] == needle {
            let before_ok = i == 0 || !is_cobol_id_byte(bytes[i - 1]);
            let j = i + needle.len();
            let after_ok = (j + 1 < bytes.len() && &hay[j..j + 2] == b"::")
                || (j < bytes.len() && bytes[j] == b'(');
            if before_ok && after_ok {
                out.push_str(&code[last..i]);
                out.push_str(new);
                i = j;
                last = j;
                continue;
            }
        }
        i += 1;
    }
    if last == 0 {
        return;
    }
    out.push_str(&code[last..]);
    *code = out;
}

fn draw_handles(painter: &egui::Painter, origin: Pos2, r: &cobolt_forms::model::Rect, glass: bool) {
    for &h in &ALL_HANDLES {
        let hp = handle_pos(r, h);
        let screen = origin + Vec2::new(hp.x, hp.y);
        if glass {
            painter.circle_filled(
                screen,
                5.0,
                Color32::from_rgba_premultiplied(30, 60, 160, 200),
            );
            painter.circle_filled(
                screen,
                4.0,
                Color32::from_rgba_premultiplied(255, 255, 255, 220),
            );
            painter.circle_stroke(
                screen,
                5.0,
                Stroke::new(1.0, Color32::from_rgba_premultiplied(100, 160, 255, 200)),
            );
        } else {
            painter.circle_filled(screen, 4.5, Color32::WHITE);
            painter.circle_stroke(
                screen,
                4.5,
                Stroke::new(1.5, Color32::from_rgb(60, 120, 230)),
            );
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
        Stroke::new(
            if active == Some(FormEdge::Right) {
                4.0
            } else {
                3.0
            },
            col(FormEdge::Right),
        ),
    );

    // Bottom edge — a short horizontal bar centred on the bottom border.
    let by = canvas.bottom();
    let bcx = canvas.center().x;
    painter.line_segment(
        [Pos2::new(bcx - 14.0, by), Pos2::new(bcx + 14.0, by)],
        Stroke::new(
            if active == Some(FormEdge::Bottom) {
                4.0
            } else {
                3.0
            },
            col(FormEdge::Bottom),
        ),
    );

    // Corner — a small filled square at the bottom-right.
    let corner = canvas.max;
    let sz = 7.0;
    let crect = egui::Rect::from_min_max(Pos2::new(corner.x - sz, corner.y - sz), corner);
    painter.rect_filled(crect, 1.5, col(FormEdge::Corner));
    painter.rect_stroke(
        crect,
        1.5,
        Stroke::new(1.0, Color32::from_rgba_premultiplied(255, 255, 255, 180)),
    );
}

/// Set (`Some`) or remove (`None`) the COBOL code of a control's event handler
/// (spec 025). Creates the binding on first write; `None` restores the "no binding"
/// state (used to undo a create).
fn set_control_event_code(form: &mut Form, control_id: &str, event: &str, code: Option<String>) {
    let Some(c) = form.find_control_mut(control_id) else {
        return;
    };
    match code {
        Some(text) => {
            if let Some(b) = c.events.iter_mut().find(|b| b.event.eq_ignore_ascii_case(event)) {
                b.code = text;
            } else {
                let mut b = cobolt_forms::EventBinding::for_control(control_id, event);
                b.code = text;
                c.events.push(b);
            }
        }
        None => c.events.retain(|b| !b.event.eq_ignore_ascii_case(event)),
    }
}

/// Set (`Some`) or remove (`None`) a common procedure's body by name (spec 025).
fn set_form_procedure(form: &mut Form, name: &str, code: Option<String>) {
    match code {
        Some(text) => {
            if let Some(p) = form
                .user_procedures
                .iter_mut()
                .find(|p| p.name.eq_ignore_ascii_case(name))
            {
                p.code = text;
            } else {
                form.user_procedures.push(cobolt_forms::model::UserProcedure {
                    name: name.to_string(),
                    code: text,
                });
            }
        }
        None => form
            .user_procedures
            .retain(|p| !p.name.eq_ignore_ascii_case(name)),
    }
}

/// Convert an agent-supplied JSON value to a `PropValue` (spec 025).
fn json_to_prop(v: &serde_json::Value) -> Option<PropValue> {
    match v {
        serde_json::Value::String(s) => Some(PropValue::String(s.clone())),
        serde_json::Value::Bool(b) => Some(PropValue::Bool(*b)),
        serde_json::Value::Number(n) => n.as_i64().map(PropValue::Int),
        _ => None,
    }
}

/// Read an integer property from an agent-supplied JSON object (accepts a JSON
/// number or a numeric string).
fn json_prop_i32(map: &serde_json::Map<String, serde_json::Value>, key: &str) -> Option<i32> {
    let v = map.get(key)?;
    v.as_i64()
        .or_else(|| v.as_str().and_then(|s| s.trim().parse::<i64>().ok()))
        .map(|n| n as i32)
}

fn apply_structural_prop(ctrl: &mut Control, key: &str, value: &PropValue) {
    match key {
        "Visible" => ctrl.visible = value.as_bool(),
        "Enabled" => ctrl.enabled = value.as_bool(),
        "TabOrder" => ctrl.tab_order = value.as_i64() as u32,
        "ZOrder" => ctrl.z_order = value.as_i64() as i32,
        _ => {
            ctrl.properties.insert(key.to_owned(), value.clone());
        }
    }
}

// ── Target device presets ─────────────────────────────────────────────────────

/// All available target device presets: (label, width, height).
///
/// Dimensions are logical/point pixels at 1× scale (portrait by default).
pub(crate) const TARGET_PRESETS: &[(&str, u32, u32)] = &[
    // ── Custom ───────────────────────────────────────────────────────────────
    ("Custom", 640, 480),
    // ── Apple iPhone ─────────────────────────────────────────────────────────
    ("iPhone 16 Pro Max", 440, 956),
    ("iPhone 16 / 15 Pro", 393, 852),
    ("iPhone 15 / 14", 390, 844),
    ("iPhone SE (3rd gen)", 375, 667),
    // ── Apple iPad ───────────────────────────────────────────────────────────
    ("iPad Pro 13\" (M4)", 1032, 1376),
    ("iPad Pro 11\" (M4)", 834, 1210),
    ("iPad Air 13\" (M2)", 1024, 1366),
    ("iPad (10th gen)", 820, 1180),
    ("iPad mini (7th gen)", 744, 1133),
    // ── Apple Watch ──────────────────────────────────────────────────────────
    ("Apple Watch Ultra 2 (49mm)", 205, 251),
    ("Apple Watch Series 10 (46mm)", 198, 242),
    ("Apple Watch Series 10 (42mm)", 176, 215),
    // ── Android Phone ────────────────────────────────────────────────────────
    ("Samsung Galaxy S24 Ultra", 384, 824),
    ("Samsung Galaxy S24", 360, 780),
    ("Google Pixel 9 Pro", 412, 892),
    ("Android Phone (generic 1080p)", 393, 851),
    // ── Android Tablet ───────────────────────────────────────────────────────
    ("Samsung Galaxy Tab S9 Ultra", 1280, 800),
    ("Samsung Galaxy Tab S9", 800, 1280),
    ("Lenovo Tab P12", 1280, 800),
    ("Android Tablet (generic)", 800, 1280),
    // ── Android SmartWatch ───────────────────────────────────────────────────
    ("Samsung Galaxy Watch 7 (44mm)", 456, 456),
    ("Samsung Galaxy Watch 7 (40mm)", 432, 432),
    ("Wear OS (generic round)", 384, 384),
    ("Wear OS (generic square)", 320, 320),
];

// ── Unified Form-Designer Icon Toolbar ───────────────────────────────────────

/// All actions the unified 50-px icon toolbar can emit to the caller in app.rs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum DesignerToolbarAction {
    None,
    // History
    Undo,
    Redo,
    // File
    SaveAndGenerate,
    GenerateOnly,
    // View
    TogglePreview,
    ToggleAnimPreview,
    ToggleGrid,
    ToggleGlass,
    // Run
    RunForm,
    StopForm,
    /// Toggle the Run-Form process/memory inspector window.
    ToggleInspector,
    // Edit
    Cut,
    Copy,
    Paste,
    Duplicate,
    Delete,
    BringToFront,
    SendToBack,
    BringForward,
    SendBackward,
    // Align
    AlignLeft,
    AlignRight,
    AlignTop,
    AlignBottom,
    CenterH,
    CenterV,
    SpaceH,
    SpaceV,
    // Style
    FormatPainter,
    AutoArrange,
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
    ui: &mut egui::Ui,
    can_undo: bool,
    can_redo: bool,
    has_sel: bool,
    has_multi: bool,
    has_clipboard: bool,
    clipboard_cut: &str,
    clipboard_copy: &str,
    clipboard_paste: &str,
    clipboard_duplicate: &str,
    preview_on: bool,
    grid_on: bool,
    glass_on: bool,
    form_running: bool,
    fp_active: bool,
    inspector_on: bool,
) -> DesignerToolbarAction {
    use egui::{Color32, Rect, Vec2};

    let mut action = DesignerToolbarAction::None;

    // Fill the ENTIRE reserved panel height with the toolbox (panel) colour.
    // egui's Frame only paints the *content* area (the icon row), and the content
    // ui's `max_rect` is content-sized too — so we use `clip_rect`, which the panel
    // sets to its full reserved rect. Without this the unused bottom of the
    // `exact_height` panel showed the white viewport clear (the "white band").
    let strip_rect = ui.clip_rect();
    ui.painter()
        .rect_filled(strip_rect, 0.0, ui.visuals().panel_fill);

    // Suppress egui control backgrounds so icons paint cleanly over glass
    {
        let v = &mut ui.style_mut().visuals;
        v.widgets.inactive.weak_bg_fill = Color32::TRANSPARENT;
        v.widgets.inactive.bg_fill = Color32::TRANSPARENT;
        v.widgets.hovered.weak_bg_fill = Color32::from_rgba_premultiplied(60, 90, 180, 55);
        v.widgets.hovered.bg_fill = Color32::from_rgba_premultiplied(60, 90, 180, 55);
        v.widgets.active.weak_bg_fill = Color32::from_rgba_premultiplied(80, 120, 220, 80);
        v.widgets.active.bg_fill = Color32::from_rgba_premultiplied(80, 120, 220, 80);
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
    let icon_size = 30.0_f32;
    // Cell = icon + 5px padding on each side, so icons aren't crowded together.
    let btn_size = icon_ref_ext + 10.0;
    // Inter-group gap — half of one icon (button) width, with a separator line.
    let group_gap = btn_size * 0.5;

    // Colour palette (frozen white glass)
    let col_normal = Color32::from_rgba_premultiplied(215, 225, 255, 210);
    let col_dim = Color32::from_rgba_premultiplied(215, 225, 255, 70);
    let _col_active = Color32::from_rgba_premultiplied(130, 180, 255, 255);
    let col_accent = Color32::from_rgba_premultiplied(255, 220, 100, 240); // gold for toggles

    // Closure: allocate a button rect, draw the icon (collected as shapes and
    // uniformly resized to the reference extent), return whether it was clicked.
    let icon_btn = |ui: &mut egui::Ui,
                    enabled: bool,
                    toggled: bool,
                    tooltip: &str,
                    draw_fn: &dyn Fn(&mut Vec<Shape>, Rect, Color32)|
     -> bool {
        let (resp, painter) = ui.allocate_painter(Vec2::splat(btn_size), egui::Sense::click());
        let icon_rect = Rect::from_center_size(resp.rect.center(), Vec2::splat(icon_size));
        let col = if !enabled {
            col_dim
        } else if toggled {
            col_accent
        } else {
            col_normal
        };
        // Hover/active bg ring
        if resp.hovered() && enabled {
            painter.rect_filled(
                resp.rect,
                6.0,
                Color32::from_rgba_premultiplied(80, 110, 200, 40),
            );
        }
        if toggled {
            painter.rect_filled(
                resp.rect,
                6.0,
                Color32::from_rgba_premultiplied(60, 100, 200, 55),
            );
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
        if icon_btn(
            ui,
            true,
            preview_on,
            "Toggle Live Preview window",
            &icon_preview,
        ) {
            action = DesignerToolbarAction::TogglePreview;
        }
        if icon_btn(
            ui,
            true,
            false,
            "Play all OnFormLoad animations",
            &icon_anim_play,
        ) {
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
        // Run-Form inspector toggle (enabled only while a form runs).
        if icon_btn(
            ui,
            form_running,
            inspector_on,
            "Run-Form Inspector — CPU / memory / process charts",
            &icon_inspector,
        ) {
            action = DesignerToolbarAction::ToggleInspector;
        }

        group_separator(ui, group_gap);

        // ── Group 5: Edit Controls ───────────────────────────────────────────
        let cut_tip = format!("{clipboard_cut}  (⌘X)");
        let copy_tip = format!("{clipboard_copy}  (⌘C)");
        let paste_tip = format!("{clipboard_paste}  (⌘V)");
        let duplicate_tip = format!("{clipboard_duplicate}  (⌘D)");
        if icon_btn(ui, has_sel, false, &cut_tip, &icon_cut) {
            action = DesignerToolbarAction::Cut;
        }
        if icon_btn(ui, has_sel, false, &copy_tip, &icon_copy) {
            action = DesignerToolbarAction::Copy;
        }
        if icon_btn(ui, has_clipboard, false, &paste_tip, &icon_paste) {
            action = DesignerToolbarAction::Paste;
        }
        if icon_btn(ui, has_sel, false, &duplicate_tip, &icon_duplicate) {
            action = DesignerToolbarAction::Duplicate;
        }
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
        if icon_btn(
            ui,
            has_multi,
            false,
            "Align Bottom Edges",
            &icon_align_bottom,
        ) {
            action = DesignerToolbarAction::AlignBottom;
        }
        if icon_btn(ui, has_multi, false, "Center Horizontally", &icon_center_h) {
            action = DesignerToolbarAction::CenterH;
        }
        if icon_btn(ui, has_multi, false, "Center Vertically", &icon_center_v) {
            action = DesignerToolbarAction::CenterV;
        }
        if icon_btn(
            ui,
            has_multi,
            false,
            "Space Evenly (horizontal)",
            &icon_space_h,
        ) {
            action = DesignerToolbarAction::SpaceH;
        }
        if icon_btn(
            ui,
            has_multi,
            false,
            "Space Evenly (vertical)",
            &icon_space_v,
        ) {
            action = DesignerToolbarAction::SpaceV;
        }

        group_separator(ui, group_gap);

        // ── Group 7: Style ───────────────────────────────────────────────────
        if icon_btn(
            ui,
            has_sel,
            fp_active,
            "Format Painter — copy/paste control style",
            &icon_format_painter,
        ) {
            action = DesignerToolbarAction::FormatPainter;
        }
        if icon_btn(
            ui,
            true,
            false,
            "Auto-arrange: labels left, inputs right",
            &icon_auto_arrange,
        ) {
            action = DesignerToolbarAction::AutoArrange;
        }

        group_separator(ui, group_gap);

        // ── Group 8: Misc ────────────────────────────────────────────────────
        if icon_btn(
            ui,
            true,
            false,
            "Report a Problem with the Form Designer",
            &icon_bug,
        ) {
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
    for s in shapes.iter() {
        bbox = bbox.union(s.visual_bounding_rect());
    }
    if !bbox.is_finite() {
        return;
    }
    let cur = bbox.size().max_elem();
    if cur <= 0.01 || target_ext <= 0.01 {
        return;
    }
    let k = target_ext / cur;
    let translation = center.to_vec2() - k * bbox.center().to_vec2();
    let t = TSTransform::new(translation, k);
    for s in shapes.iter_mut() {
        s.transform(t);
    }
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
    let cx = r.center().x;
    let cy = r.center().y;
    let rad = r.width() * 0.38;
    let pts: Vec<Pos2> = (0..=14)
        .map(|i| {
            let a = std::f32::consts::PI * (0.3 + i as f32 / 14.0 * 1.4);
            Pos2::new(cx + rad * a.cos(), cy - rad * a.sin())
        })
        .collect();
    for w in pts.windows(2) {
        out.push(Shape::line_segment([w[0], w[1]], s));
    }
    let tip = pts[0];
    out.push(Shape::line_segment([tip, tip + egui::vec2(-4.0, 1.0)], s));
    out.push(Shape::line_segment([tip, tip + egui::vec2(0.0, -4.0)], s));
}

fn icon_redo(out: &mut Vec<Shape>, r: Rect, c: Color32) {
    let s = Stroke::new(1.8, c);
    let cx = r.center().x;
    let cy = r.center().y;
    let rad = r.width() * 0.38;
    let pts: Vec<Pos2> = (0..=14)
        .map(|i| {
            let a = std::f32::consts::PI * (0.3 + i as f32 / 14.0 * 1.4);
            Pos2::new(cx - rad * a.cos(), cy - rad * a.sin())
        })
        .collect();
    for w in pts.windows(2) {
        out.push(Shape::line_segment([w[0], w[1]], s));
    }
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
    out.push(Shape::line_segment(
        [
            Pos2::new(mid_x, r.min.y + 4.0),
            Pos2::new(mid_x, bot.min.y - 2.0),
        ],
        s,
    ));
    out.push(Shape::line_segment(
        [
            Pos2::new(mid_x - 3.0, bot.min.y - 5.0),
            Pos2::new(mid_x, bot.min.y - 2.0),
        ],
        s,
    ));
    out.push(Shape::line_segment(
        [
            Pos2::new(mid_x + 3.0, bot.min.y - 5.0),
            Pos2::new(mid_x, bot.min.y - 2.0),
        ],
        s,
    ));
}

fn icon_generate(out: &mut Vec<Shape>, r: Rect, c: Color32) {
    let s = Stroke::new(1.8, c);
    let cx = r.center().x;
    let cy = r.center().y;
    out.push(Shape::line_segment(
        [Pos2::new(cx - 5.0, cy - 5.0), Pos2::new(cx - 9.0, cy)],
        s,
    ));
    out.push(Shape::line_segment(
        [Pos2::new(cx - 9.0, cy), Pos2::new(cx - 5.0, cy + 5.0)],
        s,
    ));
    out.push(Shape::line_segment(
        [Pos2::new(cx + 5.0, cy - 5.0), Pos2::new(cx + 9.0, cy)],
        s,
    ));
    out.push(Shape::line_segment(
        [Pos2::new(cx + 9.0, cy), Pos2::new(cx + 5.0, cy + 5.0)],
        s,
    ));
    out.push(Shape::line_segment(
        [Pos2::new(cx + 3.0, cy - 6.0), Pos2::new(cx - 3.0, cy + 6.0)],
        s,
    ));
}

fn icon_preview(out: &mut Vec<Shape>, r: Rect, c: Color32) {
    let s = Stroke::new(1.6, c);
    let cx = r.center().x;
    let cy = r.center().y;
    let brow_pts: Vec<Pos2> = (0..=12)
        .map(|i| {
            let t = i as f32 / 12.0;
            let a = std::f32::consts::PI * t;
            Pos2::new(
                cx + r.width() * 0.42 * (a - std::f32::consts::PI * 0.5).cos() * 1.2,
                cy + r.height() * 0.28 * a.sin(),
            )
        })
        .collect();
    for w in brow_pts.windows(2) {
        out.push(Shape::line_segment([w[0], w[1]], s));
    }
    let bot_pts: Vec<Pos2> = brow_pts
        .iter()
        .map(|pt| Pos2::new(pt.x, cy - (pt.y - cy)))
        .collect();
    for w in bot_pts.windows(2) {
        out.push(Shape::line_segment([w[0], w[1]], s));
    }
    out.push(Shape::circle_stroke(r.center(), r.width() * 0.14, s));
    out.push(Shape::circle_filled(r.center(), r.width() * 0.07, c));
}

fn icon_anim_play(out: &mut Vec<Shape>, r: Rect, c: Color32) {
    let s = Stroke::new(1.5, c);
    let cx = r.center().x;
    let cy = r.center().y;
    let rad = r.width() * 0.4;
    out.push(Shape::circle_stroke(Pos2::new(cx, cy), rad, s));
    let pts = vec![
        Pos2::new(cx - rad * 0.3, cy - rad * 0.45),
        Pos2::new(cx + rad * 0.5, cy),
        Pos2::new(cx - rad * 0.3, cy + rad * 0.45),
    ];
    out.push(Shape::convex_polygon(pts, c, Stroke::NONE));
    for (dx, dy) in [
        (-rad * 0.75, -rad * 0.6),
        (rad * 0.75, -rad * 0.6),
        (0.0_f32, -rad * 0.9),
    ] {
        out.push(Shape::circle_filled(Pos2::new(cx + dx, cy + dy), 1.5, c));
    }
}

fn icon_grid(out: &mut Vec<Shape>, r: Rect, c: Color32) {
    let s = Stroke::new(1.3, c);
    let sr = r.shrink(3.0);
    for row in 0..3 {
        for col in 0..3 {
            let pt = Pos2::new(
                sr.min.x + col as f32 * sr.width() * 0.5,
                sr.min.y + row as f32 * sr.height() * 0.5,
            );
            out.push(Shape::circle_filled(pt, 1.5, c));
        }
    }
    out.push(Shape::rect_stroke(sr, 1.0, s));
}

fn icon_glass(out: &mut Vec<Shape>, r: Rect, c: Color32) {
    let s = Stroke::new(1.6, c);
    let cx = r.center().x;
    let cy = r.center().y;
    let hw = r.width() * 0.42;
    let hh = r.height() * 0.42;
    let pts = vec![
        Pos2::new(cx, cy - hh),
        Pos2::new(cx + hw, cy - hh * 0.2),
        Pos2::new(cx + hw * 0.6, cy + hh),
        Pos2::new(cx - hw * 0.6, cy + hh),
        Pos2::new(cx - hw, cy - hh * 0.2),
    ];
    for i in 0..pts.len() {
        out.push(Shape::line_segment([pts[i], pts[(i + 1) % pts.len()]], s));
    }
    out.push(Shape::line_segment(
        [pts[0], pts[2]],
        Stroke::new(
            1.0,
            Color32::from_rgba_premultiplied(c.r(), c.g(), c.b(), 90),
        ),
    ));
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

fn icon_inspector(out: &mut Vec<Shape>, r: Rect, c: Color32) {
    // A mini bar-chart glyph on a baseline — the metrics/inspector icon.
    let (w, h) = (r.width(), r.height());
    let base = r.max.y - h * 0.20;
    let bar_w = w * 0.15;
    for (x, bh) in [(0.24_f32, 0.32_f32), (0.45, 0.56), (0.66, 0.42)] {
        let left = r.min.x + w * x;
        let top = base - h * bh;
        out.push(Shape::rect_filled(
            Rect::from_min_max(Pos2::new(left, top), Pos2::new(left + bar_w, base)),
            1.0,
            c,
        ));
    }
    out.push(Shape::line_segment(
        [
            Pos2::new(r.min.x + w * 0.18, base),
            Pos2::new(r.max.x - w * 0.14, base),
        ],
        Stroke::new(1.4, c),
    ));
}

fn icon_cut(out: &mut Vec<Shape>, r: Rect, c: Color32) {
    let s = Stroke::new(1.7, c);
    let left = Pos2::new(r.min.x + r.width() * 0.25, r.max.y - r.height() * 0.25);
    let right = Pos2::new(r.max.x - r.width() * 0.25, r.max.y - r.height() * 0.25);
    let pivot = r.center();
    let top_left = Pos2::new(r.min.x + r.width() * 0.25, r.min.y + r.height() * 0.24);
    let top_right = Pos2::new(r.max.x - r.width() * 0.25, r.min.y + r.height() * 0.24);
    out.push(Shape::line_segment([top_left, pivot], s));
    out.push(Shape::line_segment([top_right, pivot], s));
    out.push(Shape::line_segment([pivot, left], s));
    out.push(Shape::line_segment([pivot, right], s));
    out.push(Shape::circle_stroke(left, r.width() * 0.10, s));
    out.push(Shape::circle_stroke(right, r.width() * 0.10, s));
}

fn icon_copy(out: &mut Vec<Shape>, r: Rect, c: Color32) {
    let s = Stroke::new(1.6, c);
    let back = Rect::from_min_max(r.min + egui::vec2(4.0, 3.0), r.max - egui::vec2(7.0, 8.0));
    let front = Rect::from_min_max(r.min + egui::vec2(8.0, 8.0), r.max - egui::vec2(3.0, 3.0));
    out.push(Shape::rect_stroke(
        back,
        1.5,
        Stroke::new(1.2, c.linear_multiply(0.75)),
    ));
    out.push(Shape::rect_stroke(front, 1.5, s));
    out.push(Shape::line_segment(
        [
            Pos2::new(front.min.x + 3.0, front.min.y + 5.0),
            Pos2::new(front.max.x - 3.0, front.min.y + 5.0),
        ],
        Stroke::new(1.1, c),
    ));
    out.push(Shape::line_segment(
        [
            Pos2::new(front.min.x + 3.0, front.min.y + 10.0),
            Pos2::new(front.max.x - 5.0, front.min.y + 10.0),
        ],
        Stroke::new(1.1, c),
    ));
}

fn icon_paste(out: &mut Vec<Shape>, r: Rect, c: Color32) {
    let s = Stroke::new(1.6, c);
    let board = Rect::from_min_max(r.min + egui::vec2(5.0, 6.0), r.max - egui::vec2(5.0, 3.0));
    let clip = Rect::from_center_size(
        Pos2::new(r.center().x, board.min.y),
        Vec2::new(r.width() * 0.34, r.height() * 0.18),
    );
    out.push(Shape::rect_stroke(board, 2.0, s));
    out.push(Shape::rect_filled(
        clip,
        2.0,
        Color32::from_rgba_premultiplied(c.r(), c.g(), c.b(), 35),
    ));
    out.push(Shape::rect_stroke(clip, 2.0, Stroke::new(1.4, c)));
    let page = Rect::from_min_max(
        board.min + egui::vec2(5.0, 7.0),
        board.max - egui::vec2(5.0, 4.0),
    );
    out.push(Shape::rect_stroke(page, 1.0, Stroke::new(1.2, c)));
}

fn icon_duplicate(out: &mut Vec<Shape>, r: Rect, c: Color32) {
    icon_copy(out, r, c);
    let s = Stroke::new(1.7, c);
    let cx = r.max.x - r.width() * 0.22;
    let cy = r.min.y + r.height() * 0.24;
    out.push(Shape::line_segment(
        [Pos2::new(cx - 4.0, cy), Pos2::new(cx + 4.0, cy)],
        s,
    ));
    out.push(Shape::line_segment(
        [Pos2::new(cx, cy - 4.0), Pos2::new(cx, cy + 4.0)],
        s,
    ));
}

fn icon_delete(out: &mut Vec<Shape>, r: Rect, c: Color32) {
    let s = Stroke::new(1.8, c);
    let sr = r.shrink(3.5);
    let body = Rect::from_min_max(
        Pos2::new(sr.min.x + 2.0, sr.min.y + sr.height() * 0.28),
        sr.max,
    );
    out.push(Shape::rect_stroke(body, 1.0, s));
    out.push(Shape::line_segment(
        [
            Pos2::new(sr.min.x, sr.min.y + sr.height() * 0.22),
            Pos2::new(sr.max.x, sr.min.y + sr.height() * 0.22),
        ],
        s,
    ));
    out.push(Shape::line_segment(
        [
            Pos2::new(sr.center().x - 3.0, sr.min.y),
            Pos2::new(sr.center().x + 3.0, sr.min.y),
        ],
        s,
    ));
    for i in 1..=3 {
        let x = body.min.x + body.width() * i as f32 / 4.0;
        out.push(Shape::line_segment(
            [
                Pos2::new(x, body.min.y + 3.0),
                Pos2::new(x, body.max.y - 3.0),
            ],
            Stroke::new(1.2, c),
        ));
    }
}

fn icon_bring_front(out: &mut Vec<Shape>, r: Rect, c: Color32) {
    let s = Stroke::new(1.5, c);
    let cx = r.center().x;
    let top = r.min.y + 4.0;
    let bot = r.max.y - 4.0;
    let r1 = Rect::from_min_max(
        Pos2::new(r.min.x + 5.0, top + 4.0),
        Pos2::new(r.max.x - 2.0, bot),
    );
    let r2 = Rect::from_min_max(
        Pos2::new(r.min.x + 2.0, top + 8.0),
        Pos2::new(r.max.x - 5.0, bot + 3.0),
    );
    out.push(Shape::rect_stroke(
        r2,
        1.0,
        Stroke::new(
            1.2,
            Color32::from_rgba_premultiplied(c.r(), c.g(), c.b(), 120),
        ),
    ));
    out.push(Shape::rect_filled(
        r1,
        1.0,
        Color32::from_rgba_premultiplied(c.r(), c.g(), c.b(), 40),
    ));
    out.push(Shape::rect_stroke(r1, 1.0, s));
    out.push(Shape::line_segment(
        [Pos2::new(cx, top - 1.0), Pos2::new(cx, top + 6.0)],
        s,
    ));
    out.push(Shape::line_segment(
        [Pos2::new(cx - 3.0, top + 3.0), Pos2::new(cx, top - 1.0)],
        s,
    ));
    out.push(Shape::line_segment(
        [Pos2::new(cx + 3.0, top + 3.0), Pos2::new(cx, top - 1.0)],
        s,
    ));
}

fn icon_send_back(out: &mut Vec<Shape>, r: Rect, c: Color32) {
    let s = Stroke::new(1.5, c);
    let cx = r.center().x;
    let top = r.min.y + 4.0;
    let bot = r.max.y - 4.0;
    let r1 = Rect::from_min_max(
        Pos2::new(r.min.x + 5.0, top + 4.0),
        Pos2::new(r.max.x - 2.0, bot),
    );
    let r2 = Rect::from_min_max(
        Pos2::new(r.min.x + 2.0, top + 8.0),
        Pos2::new(r.max.x - 5.0, bot + 3.0),
    );
    out.push(Shape::rect_filled(
        r1,
        1.0,
        Color32::from_rgba_premultiplied(c.r(), c.g(), c.b(), 40),
    ));
    out.push(Shape::rect_stroke(
        r1,
        1.0,
        Stroke::new(
            1.2,
            Color32::from_rgba_premultiplied(c.r(), c.g(), c.b(), 120),
        ),
    ));
    out.push(Shape::rect_stroke(r2, 1.0, s));
    out.push(Shape::line_segment(
        [Pos2::new(cx, bot + 4.0), Pos2::new(cx, bot - 3.0)],
        s,
    ));
    out.push(Shape::line_segment(
        [Pos2::new(cx - 3.0, bot + 1.0), Pos2::new(cx, bot + 4.0)],
        s,
    ));
    out.push(Shape::line_segment(
        [Pos2::new(cx + 3.0, bot + 1.0), Pos2::new(cx, bot + 4.0)],
        s,
    ));
}

fn icon_fwd(out: &mut Vec<Shape>, r: Rect, c: Color32) {
    let s = Stroke::new(1.5, c);
    let cx = r.center().x;
    let cy = r.center().y;
    out.push(Shape::rect_stroke(
        Rect::from_center_size(Pos2::new(cx - 2.0, cy + 1.0), Vec2::new(10.0, 8.0)),
        1.0,
        Stroke::new(
            1.2,
            Color32::from_rgba_premultiplied(c.r(), c.g(), c.b(), 120),
        ),
    ));
    out.push(Shape::rect_stroke(
        Rect::from_center_size(Pos2::new(cx + 1.0, cy - 1.0), Vec2::new(10.0, 8.0)),
        1.0,
        s,
    ));
    // "+" marker (Bring Forward = +1 z-order)
    let mx = cx + 1.0;
    let my = cy - 1.0;
    out.push(Shape::line_segment(
        [Pos2::new(mx - 2.5, my), Pos2::new(mx + 2.5, my)],
        s,
    ));
    out.push(Shape::line_segment(
        [Pos2::new(mx, my - 2.5), Pos2::new(mx, my + 2.5)],
        s,
    ));
}

fn icon_bwd(out: &mut Vec<Shape>, r: Rect, c: Color32) {
    let s = Stroke::new(1.5, c);
    let cx = r.center().x;
    let cy = r.center().y;
    out.push(Shape::rect_stroke(
        Rect::from_center_size(Pos2::new(cx + 2.0, cy - 1.0), Vec2::new(10.0, 8.0)),
        1.0,
        Stroke::new(
            1.2,
            Color32::from_rgba_premultiplied(c.r(), c.g(), c.b(), 120),
        ),
    ));
    out.push(Shape::rect_stroke(
        Rect::from_center_size(Pos2::new(cx - 1.0, cy + 1.0), Vec2::new(10.0, 8.0)),
        1.0,
        s,
    ));
    // "−" marker (Send Backward = -1 z-order)
    let mx = cx - 1.0;
    let my = cy + 1.0;
    out.push(Shape::line_segment(
        [Pos2::new(mx - 2.5, my), Pos2::new(mx + 2.5, my)],
        s,
    ));
}

fn _icon_align(out: &mut Vec<Shape>, r: Rect, c: Color32, horiz: bool, lo_side: bool) {
    let s = Stroke::new(1.5, c);
    let sr = r.shrink(3.0);
    if horiz {
        let x = if lo_side { sr.min.x } else { sr.max.x };
        out.push(Shape::line_segment(
            [Pos2::new(x, sr.min.y), Pos2::new(x, sr.max.y)],
            Stroke::new(1.8, c),
        ));
        for (i, w, h) in [(0, 8.0, 4.0), (1, 6.0, 4.0)] {
            let y = sr.min.y + sr.height() * (0.2 + i as f32 * 0.45);
            let x_rect = if lo_side { x + 1.0 } else { x - w - 1.0 };
            out.push(Shape::rect_stroke(
                Rect::from_min_size(Pos2::new(x_rect, y), Vec2::new(w, h)),
                1.0,
                s,
            ));
        }
    } else {
        let y = if lo_side { sr.min.y } else { sr.max.y };
        out.push(Shape::line_segment(
            [Pos2::new(sr.min.x, y), Pos2::new(sr.max.x, y)],
            Stroke::new(1.8, c),
        ));
        for (i, w, h) in [(0, 4.0, 7.0), (1, 4.0, 5.0)] {
            let x = sr.min.x + sr.width() * (0.2 + i as f32 * 0.45);
            let y_rect = if lo_side { y + 1.0 } else { y - h - 1.0 };
            out.push(Shape::rect_stroke(
                Rect::from_min_size(Pos2::new(x, y_rect), Vec2::new(w, h)),
                1.0,
                s,
            ));
        }
    }
}
fn icon_align_left(out: &mut Vec<Shape>, r: Rect, c: Color32) {
    _icon_align(out, r, c, true, true);
}
fn icon_align_right(out: &mut Vec<Shape>, r: Rect, c: Color32) {
    _icon_align(out, r, c, true, false);
}
fn icon_align_top(out: &mut Vec<Shape>, r: Rect, c: Color32) {
    _icon_align(out, r, c, false, true);
}
fn icon_align_bottom(out: &mut Vec<Shape>, r: Rect, c: Color32) {
    _icon_align(out, r, c, false, false);
}

fn icon_center_h(out: &mut Vec<Shape>, r: Rect, c: Color32) {
    let s = Stroke::new(1.5, c);
    let sr = r.shrink(3.0);
    let cx = sr.center().x;
    out.push(Shape::line_segment(
        [Pos2::new(cx, sr.min.y), Pos2::new(cx, sr.max.y)],
        Stroke::new(1.8, c),
    ));
    for (dy, w) in [(0.15_f32, 9.0_f32), (0.55, 7.0)] {
        let y = sr.min.y + sr.height() * dy;
        out.push(Shape::rect_stroke(
            Rect::from_center_size(Pos2::new(cx, y + 2.0), Vec2::new(w, 4.0)),
            1.0,
            s,
        ));
    }
}

fn icon_center_v(out: &mut Vec<Shape>, r: Rect, c: Color32) {
    let s = Stroke::new(1.5, c);
    let sr = r.shrink(3.0);
    let cy = sr.center().y;
    out.push(Shape::line_segment(
        [Pos2::new(sr.min.x, cy), Pos2::new(sr.max.x, cy)],
        Stroke::new(1.8, c),
    ));
    for (dx, h) in [(0.15_f32, 9.0_f32), (0.55, 7.0)] {
        let x = sr.min.x + sr.width() * dx;
        out.push(Shape::rect_stroke(
            Rect::from_center_size(Pos2::new(x + 2.0, cy), Vec2::new(4.0, h)),
            1.0,
            s,
        ));
    }
}

fn icon_space_h(out: &mut Vec<Shape>, r: Rect, c: Color32) {
    let s = Stroke::new(1.4, c);
    let sr = r.shrink(3.0);
    for i in 0..3 {
        let x = sr.min.x + sr.width() * (0.15 + i as f32 * 0.35);
        out.push(Shape::rect_stroke(
            Rect::from_min_size(
                Pos2::new(x, sr.min.y + 3.0),
                Vec2::new(3.5, sr.height() - 6.0),
            ),
            1.0,
            s,
        ));
    }
    let y = sr.max.y - 2.0;
    out.push(Shape::line_segment(
        [Pos2::new(sr.min.x, y), Pos2::new(sr.max.x, y)],
        s,
    ));
    out.push(Shape::line_segment(
        [Pos2::new(sr.min.x + 2.0, y - 2.0), Pos2::new(sr.min.x, y)],
        s,
    ));
    out.push(Shape::line_segment(
        [Pos2::new(sr.max.x - 2.0, y - 2.0), Pos2::new(sr.max.x, y)],
        s,
    ));
}

fn icon_space_v(out: &mut Vec<Shape>, r: Rect, c: Color32) {
    let s = Stroke::new(1.4, c);
    let sr = r.shrink(3.0);
    for i in 0..3 {
        let y = sr.min.y + sr.height() * (0.12 + i as f32 * 0.35);
        out.push(Shape::rect_stroke(
            Rect::from_min_size(
                Pos2::new(sr.min.x + 3.0, y),
                Vec2::new(sr.width() - 6.0, 3.5),
            ),
            1.0,
            s,
        ));
    }
    let x = sr.max.x - 2.0;
    out.push(Shape::line_segment(
        [Pos2::new(x, sr.min.y), Pos2::new(x, sr.max.y)],
        s,
    ));
    out.push(Shape::line_segment(
        [Pos2::new(x - 2.0, sr.min.y + 2.0), Pos2::new(x, sr.min.y)],
        s,
    ));
    out.push(Shape::line_segment(
        [Pos2::new(x - 2.0, sr.max.y - 2.0), Pos2::new(x, sr.max.y)],
        s,
    ));
}

fn icon_format_painter(out: &mut Vec<Shape>, r: Rect, c: Color32) {
    let s = Stroke::new(1.5, c);
    let cx = r.center().x;
    let cy = r.center().y;
    out.push(Shape::line_segment(
        [Pos2::new(cx + 2.0, cy - 7.0), Pos2::new(cx + 2.0, cy + 4.0)],
        s,
    ));
    out.push(Shape::line_segment(
        [Pos2::new(cx + 2.0, cy - 7.0), Pos2::new(cx - 5.0, cy - 7.0)],
        s,
    ));
    out.push(Shape::rect_stroke(
        Rect::from_min_size(Pos2::new(cx - 6.0, cy - 5.0), Vec2::new(10.0, 5.0)),
        1.0,
        s,
    ));
    out.push(Shape::line_segment(
        [Pos2::new(cx + 2.0, cy + 4.0), Pos2::new(cx + 2.0, cy + 7.0)],
        Stroke::new(1.2, c),
    ));
    out.push(Shape::circle_filled(Pos2::new(cx + 2.0, cy + 8.0), 1.5, c));
}

fn icon_auto_arrange(out: &mut Vec<Shape>, r: Rect, c: Color32) {
    let s = Stroke::new(1.4, c);
    let sr = r.shrink(3.0);
    out.push(Shape::rect_stroke(
        Rect::from_min_size(sr.min, Vec2::new(sr.width() * 0.38, 4.5)),
        1.0,
        s,
    ));
    out.push(Shape::rect_stroke(
        Rect::from_min_size(
            Pos2::new(sr.min.x + sr.width() * 0.45, sr.min.y),
            Vec2::new(sr.width() * 0.55, 4.5),
        ),
        1.0,
        Stroke::new(
            1.4,
            Color32::from_rgba_premultiplied(c.r(), c.g(), c.b(), 180),
        ),
    ));
    let y2 = sr.min.y + 7.0;
    out.push(Shape::rect_stroke(
        Rect::from_min_size(Pos2::new(sr.min.x, y2), Vec2::new(sr.width() * 0.30, 4.5)),
        1.0,
        s,
    ));
    out.push(Shape::rect_stroke(
        Rect::from_min_size(
            Pos2::new(sr.min.x + sr.width() * 0.45, y2),
            Vec2::new(sr.width() * 0.55, 4.5),
        ),
        1.0,
        Stroke::new(
            1.4,
            Color32::from_rgba_premultiplied(c.r(), c.g(), c.b(), 180),
        ),
    ));
    out.push(Shape::line_segment(
        [
            Pos2::new(sr.center().x - 1.0, sr.max.y - 5.0),
            Pos2::new(sr.center().x + 4.0, sr.max.y - 1.0),
        ],
        Stroke::new(1.6, c),
    ));
    out.push(Shape::circle_filled(
        Pos2::new(sr.center().x - 1.0, sr.max.y - 5.0),
        2.0,
        c,
    ));
}

fn icon_bug(out: &mut Vec<Shape>, r: Rect, c: Color32) {
    let s = Stroke::new(1.4, c);
    let cx = r.center().x;
    let cy = r.center().y;
    let pts: Vec<Pos2> = (0..=20)
        .map(|i| {
            let a = std::f32::consts::TAU * i as f32 / 20.0;
            Pos2::new(cx + 4.5 * a.cos(), cy + 1.5 + 5.5 * a.sin())
        })
        .collect();
    for w in pts.windows(2) {
        out.push(Shape::line_segment([w[0], w[1]], s));
    }
    out.push(Shape::circle_stroke(Pos2::new(cx, cy - 4.0), 3.0, s));
    out.push(Shape::line_segment(
        [Pos2::new(cx - 1.5, cy - 6.5), Pos2::new(cx - 4.0, cy - 9.0)],
        s,
    ));
    out.push(Shape::line_segment(
        [Pos2::new(cx + 1.5, cy - 6.5), Pos2::new(cx + 4.0, cy - 9.0)],
        s,
    ));
    for (i, sign) in [
        (-3.0_f32, -1.0_f32),
        (0.0, -1.0),
        (3.0, -1.0),
        (-3.0, 1.0),
        (0.0, 1.0),
        (3.0, 1.0),
    ] {
        let by = cy + 1.5 + i;
        out.push(Shape::line_segment(
            [
                Pos2::new(cx + sign * 4.5, by),
                Pos2::new(cx + sign * 8.0, by - 1.5),
            ],
            s,
        ));
    }
}

/// Return `(width, height)` for a named preset, or `None` for "Custom" / unknown.
pub(crate) fn target_preset_size(name: &str) -> Option<(u32, u32)> {
    if name == "Custom" {
        return None;
    }
    TARGET_PRESETS
        .iter()
        .find(|(label, ..)| *label == name)
        .map(|(_, w, h)| (*w, *h))
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
        assert_eq!(resize(FormEdge::Right, 400, 300, 50, 99), (450, 300));
        assert_eq!(resize(FormEdge::Bottom, 400, 300, 99, 40), (400, 340));
        assert_eq!(resize(FormEdge::Corner, 400, 300, 60, 30), (460, 330));
        // Shrinking past the minimum clamps to FORM_MIN_SIZE.
        assert_eq!(
            resize(FormEdge::Corner, 100, 100, -90, -90),
            (FORM_MIN_SIZE, FORM_MIN_SIZE)
        );
    }
}

#[cfg(test)]
mod animator_tests {
    use super::draw_animator;
    use cobolt_forms::{Control, ControlType};
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
            enc.encode_frame(Frame::from_parts(
                red,
                0,
                0,
                Delay::from_numer_denom_ms(100, 1),
            ))
            .unwrap();
            enc.encode_frame(Frame::from_parts(
                blue,
                0,
                0,
                Delay::from_numer_denom_ms(100, 1),
            ))
            .unwrap();
        }
        path
    }

    /// Render the Animator at virtual time `t` (seconds) and return the texture
    /// id of the painted image, if any.
    fn frame_tex(ctx: &egui::Context, src: &str, t: f64) -> Option<egui::TextureId> {
        let raw = egui::RawInput {
            time: Some(t),
            ..Default::default()
        };
        let out = ctx.run(raw, |ctx| {
            let painter = ctx.layer_painter(egui::LayerId::background());
            let ctrl = Control::new("anim", ControlType::Animator, 0, 0);
            draw_animator(
                &painter,
                Rect::from_min_size(pos2(0.0, 0.0), vec2(64.0, 64.0)),
                &ctrl,
                "anim-key",
                src,
                true,
                true,
                "Fit",
                1.0,
                false,
            );
        });
        out.shapes.into_iter().find_map(|cs| match cs.shape {
            egui::Shape::Mesh(m) => Some(m.texture_id),
            egui::Shape::Rect(r) if r.fill_texture_id != egui::TextureId::default() => {
                Some(r.fill_texture_id)
            }
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
        assert!(
            f0.is_some(),
            "Animator should paint an image once a source is set"
        );

        // 150 ms later we are on the second frame → a different texture.
        let f1 = frame_tex(&ctx, &src, 0.15);
        assert!(f1.is_some());
        assert_ne!(
            f0, f1,
            "Animator should advance to a different frame over time"
        );

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

    /// All visual controls the design-time canvas paints.
    fn visual_widgets() -> Vec<(ControlType, &'static str)> {
        use ControlType::*;
        vec![
            (Label, "Label"),
            (Button, "Button"),
            (TextBox, "TextBox"),
            (CheckBox, "CheckBox"),
            (RadioButton, "RadioButton"),
            (ComboBox, "ComboBox"),
            (ListBox, "ListBox"),
            (GroupBox, "GroupBox"),
            (Panel, "Panel"),
            (ProgressBar, "ProgressBar"),
            (Slider, "Slider"),
            (NumericUpDown, "NumericUpDown"),
            (DateTimePicker, "DateTimePicker"),
            (PictureBox, "PictureBox"),
            (DataGrid, "DataGrid"),
            (TabControl, "TabControl"),
            (TreeView, "TreeView"),
            (Line, "Line"),
            (Shape, "Shape"),
            (Splitter, "Splitter"),
            (MenuBar, "MenuBar"),
            (ToolBar, "ToolBar"),
            (StatusBar, "StatusBar"),
            (BarChart, "BarChart"),
            (LineChart, "LineChart"),
            (PieChart, "PieChart"),
            (AreaChart, "AreaChart"),
            (ScatterChart, "ScatterChart"),
            (DonutChart, "DonutChart"),
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
        for ct in [
            ControlType::Label,
            ControlType::Button,
            ControlType::GroupBox,
        ] {
            let mut c = Control::new("W", ct.clone(), 5, 7);
            c.set_prop("Caption", PropValue::String("CAP-RC".into()));
            let ts = texts(&render(&c));
            assert!(
                ts.iter().any(|t| t.galley.text().contains("CAP-RC")),
                "{ct:?}: Caption not painted; texts={:?}",
                ts.iter()
                    .map(|t| t.galley.text().to_owned())
                    .collect::<Vec<_>>()
            );
        }
    }

    #[test]
    fn groupbox_without_caption_does_not_paint_control_name() {
        let c = Control::new("GroupBox-1", ControlType::GroupBox, 5, 7);
        let ts = texts(&render(&c));
        assert!(
            ts.iter().all(|t| !t.galley.text().contains("GroupBox-1")),
            "GroupBox painted its control id as a caption; texts={:?}",
            ts.iter()
                .map(|t| t.galley.text().to_owned())
                .collect::<Vec<_>>()
        );
    }

    #[test]
    fn groupbox_legacy_generated_caption_is_not_painted_after_rename() {
        let mut c = Control::new("Menu", ControlType::GroupBox, 5, 7);
        c.set_prop("Caption", PropValue::String("GroupBox-1".into()));

        let ts = texts(&render(&c));

        assert!(
            ts.iter().all(|t| !t.galley.text().contains("GroupBox-1")),
            "GroupBox painted stale generated caption; texts={:?}",
            ts.iter()
                .map(|t| t.galley.text().to_owned())
                .collect::<Vec<_>>()
        );
    }

    #[test]
    fn groupbox_explicit_caption_still_paints() {
        let mut c = Control::new("GroupBox-1", ControlType::GroupBox, 5, 7);
        c.set_prop("Caption", PropValue::String("Menu".into()));

        let ts = texts(&render(&c));

        assert!(
            ts.iter().any(|t| t.galley.text().contains("Menu")),
            "explicit GroupBox caption was not painted; texts={:?}",
            ts.iter()
                .map(|t| t.galley.text().to_owned())
                .collect::<Vec<_>>()
        );
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
            label_with("Italic")
                .galley
                .job
                .sections
                .iter()
                .any(|s| s.format.italics),
            "Italic not applied"
        );
        assert!(
            label_with("Underline")
                .galley
                .job
                .sections
                .iter()
                .any(|s| s.format.underline.width > 0.0),
            "Underline not applied"
        );
        assert!(
            label_with("Strikethrough")
                .galley
                .job
                .sections
                .iter()
                .any(|s| s.format.strikethrough.width > 0.0),
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
        let n_plain = texts(&render(&plain))
            .iter()
            .filter(|t| t.galley.text().contains("BOLD-RC"))
            .count();
        let n_bold = texts(&render(&bold))
            .iter()
            .filter(|t| t.galley.text().contains("BOLD-RC"))
            .count();
        assert!(
            n_bold > n_plain,
            "Bold did not add an extra paint pass (plain={n_plain}, bold={n_bold})"
        );
    }

    #[test]
    fn label_forecolor_is_applied() {
        let mut c = Control::new("LBL", ControlType::Label, 5, 7);
        c.set_prop("Caption", PropValue::String("RED-RC".into()));
        c.set_prop("ForegroundColor", PropValue::String("#FF0000".into()));
        let t = texts(&render(&c))
            .into_iter()
            .find(|t| t.galley.text().contains("RED-RC"))
            .expect("painted");
        let col = t
            .galley
            .job
            .sections
            .first()
            .map(|s| s.format.color)
            .unwrap_or(egui::Color32::TRANSPARENT);
        assert!(
            col.r() > 180 && col.g() < 90 && col.b() < 90,
            "ForeColor not applied; got {col:?}"
        );
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
        assert!(
            (dx0 + W).abs() < 0.5,
            "start should be off-screen left (dx≈-W), got {dx0}"
        );
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
        assert!(
            (a0 - 1.0).abs() < 0.01 && a1.abs() < 0.01,
            "fade-out 1→0, got {a0}→{a1}"
        );
    }

    #[test]
    fn zoom_out_elastic_is_a_damped_multi_bounce() {
        // With Elastic easing: starts 100%, dips toward ~25%, bounces 3–4 times,
        // settles 100%.
        let a = AnimationDef {
            easing: EasingKind::Elastic,
            ..anim(AnimKind::ZoomOut)
        };
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
        assert!(
            crossings >= 4,
            "should bounce several times, got {crossings} baseline crossings"
        );
    }

    #[test]
    fn zoom_out_non_elastic_is_a_single_dip_and_return() {
        // Linear (or any non-Elastic) easing: a single smooth dip — 100% → 25% →
        // 100% — with no overshoot above 100%.
        let a = anim(AnimKind::ZoomOut); // Linear
        let s = |t: f32| anim_transform(&a, W, H, t).2;
        assert!((s(0.0) - 1.0).abs() < 0.01, "start≈100%, got {}", s(0.0));
        assert!((s(1.0) - 1.0).abs() < 0.01, "end≈100%, got {}", s(1.0));
        assert!(
            (s(0.5) - 0.25).abs() < 0.02,
            "deepest dip ≈25% at midpoint, got {}",
            s(0.5)
        );
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
        assert!((half.width() - 100.0).abs() < 0.01);
        assert!((half.height() - 50.0).abs() < 0.01);
        assert!(
            (half.center() - centre).length() < 0.01,
            "centre must be preserved"
        );

        // Double size, same centre.
        let dbl = scale_rect_about_center(base, 2.0);
        assert!((dbl.width() - 400.0).abs() < 0.01);
        assert!((dbl.height() - 200.0).abs() < 0.01);
        assert!((dbl.center() - centre).length() < 0.01);

        // A zoom-in at t=0 (scale≈0) collapses the rect to (almost) nothing at the centre.
        let a = anim(AnimKind::ZoomIn);
        let (_, _, s0, _) = anim_transform(&a, W, H, 0.0);
        let collapsed = scale_rect_about_center(base, s0);
        assert!(collapsed.width() < 10.0 && collapsed.height() < 10.0);
    }
}

#[cfg(test)]
mod clipboard_tests {
    use super::*;

    fn button_with_click(id: &str, paragraph: &str, code: &str) -> Control {
        let mut ctrl = Control::new(id, ControlType::Button, 10, 20);
        let mut event = cobolt_forms::EventBinding::new("onClick", paragraph);
        event.code = code.to_owned();
        ctrl.events.push(event);
        ctrl
    }

    #[test]
    fn same_form_paste_duplicates_controls_without_event_handlers() {
        let mut designer = DesignerPanel::new(Form::new("FormA", "A", 640, 480));
        designer.form.controls.push(button_with_click(
            "Button-1",
            "BUTTON-1--ONCLICK",
            "       PROCEDURE DIVISION.\n           MOVE \"OK\" TO Button-1::Caption.",
        ));
        designer.selected_ids = vec!["Button-1".to_owned()];

        let mut clipboard = None;
        designer.copy_selected(&mut clipboard);
        assert!(
            clipboard.as_ref().unwrap().controls[0].events[0].has_code(),
            "copy buffer must retain handler source until paste decides target form"
        );

        designer.paste_from_clipboard(&clipboard);

        let original = designer.form.find_control("Button-1").unwrap();
        assert_eq!(original.events.len(), 1);
        let pasted = designer.form.find_control("Button-2").unwrap();
        assert!(
            pasted.events.is_empty(),
            "same-form paste must not duplicate or reconnect handlers"
        );
        assert_eq!((pasted.rect.x, pasted.rect.y), (30, 40));
    }

    #[test]
    fn cross_form_paste_copies_handlers_and_resolves_conflicts() {
        let mut source = DesignerPanel::new(Form::new("FormA", "A", 640, 480));
        source.form.controls.push(button_with_click(
            "Button-1",
            "BUTTON-1--ONCLICK",
            "       PROCEDURE DIVISION.\n           MOVE \"OK\" TO Button-1::Caption.",
        ));
        source.selected_ids = vec!["Button-1".to_owned()];
        let mut clipboard = None;
        source.copy_selected(&mut clipboard);

        let mut target = DesignerPanel::new(Form::new("FormB", "B", 640, 480));
        target
            .form
            .controls
            .push(Control::new("Button-1", ControlType::Button, 0, 0));
        let mut conflict = Control::new("Label-1", ControlType::Label, 0, 50);
        conflict.events.push(cobolt_forms::EventBinding::new(
            "onClick",
            "BUTTON-2--ONCLICK",
        ));
        target.form.controls.push(conflict);

        target.paste_from_clipboard(&clipboard);

        let pasted = target.form.find_control("Button-2").unwrap();
        assert_eq!(pasted.events.len(), 1);
        assert_eq!(pasted.events[0].paragraph, "BUTTON-2--ONCLICK-1");
        assert!(pasted.events[0].code.contains("Button-2::Caption"));
        assert!(!pasted.events[0].code.contains("Button-1::Caption"));
    }

    #[test]
    fn cross_form_paste_preserves_shared_handler_relationship() {
        let mut source = DesignerPanel::new(Form::new("FormA", "A", 640, 480));
        source.form.controls.push(button_with_click(
            "Button-1",
            "SHARED-HANDLER",
            "       PROCEDURE DIVISION.\n           MOVE \"A\" TO Button-1::Caption.",
        ));
        source.form.controls.push(button_with_click(
            "Button-2",
            "SHARED-HANDLER",
            "       PROCEDURE DIVISION.\n           MOVE \"B\" TO Button-2::Caption.",
        ));
        source.selected_ids = vec!["Button-1".to_owned(), "Button-2".to_owned()];
        let mut clipboard = None;
        source.copy_selected(&mut clipboard);

        let mut target = DesignerPanel::new(Form::new("FormB", "B", 640, 480));
        target.paste_from_clipboard(&clipboard);

        let first = target.form.find_control("Button-1").unwrap();
        let second = target.form.find_control("Button-2").unwrap();
        assert_eq!(first.events[0].paragraph, second.events[0].paragraph);
    }
}

// ── Sticky-font tests ───────────────────────────────────────────────────────────
#[cfg(test)]
mod sticky_font_tests {
    use super::*;

    fn font_of(d: &DesignerPanel, id: &str) -> (String, i64) {
        let c = d.form.controls.iter().find(|c| c.id == id).unwrap();
        (
            c.get_prop("FontName").unwrap().as_str().to_owned(),
            c.get_prop("FontSize").unwrap().as_i64(),
        )
    }

    #[test]
    fn new_widget_inherits_last_manual_font() {
        let mut d = DesignerPanel::new(Form::new("F", "T", 640, 480));
        d.add_control(ControlType::Label, 10, 10);
        let first = d.form.controls[0].id.clone();
        // User manually picks a font on the first control.
        d.set_property(&first, "FontName", PropValue::String("Courier New".into()));
        d.set_property(&first, "FontSize", PropValue::Int(14));
        // A newly-added control inherits that exact font.
        d.add_control(ControlType::Button, 50, 50);
        let second = d.form.controls[1].id.clone();
        assert_eq!(font_of(&d, &second), ("Courier New".to_string(), 14));
    }

    #[test]
    fn new_widget_falls_back_to_last_control_font() {
        let mut d = DesignerPanel::new(Form::new("F", "T", 640, 480));
        d.add_control(ControlType::Label, 10, 10);
        let first = d.form.controls[0].id.clone();
        d.set_property(&first, "FontName", PropValue::String("Verdana".into()));
        d.set_property(&first, "FontSize", PropValue::Int(18));
        // Simulate a fresh session (no remembered font): a new control should
        // still match the existing control's font, not reset to the default.
        d.last_font_name = None;
        d.last_font_size = None;
        d.add_control(ControlType::Button, 0, 0);
        let added = d.form.controls.last().unwrap().id.clone();
        assert_eq!(font_of(&d, &added), ("Verdana".to_string(), 18));
    }

    #[test]
    fn first_widget_keeps_default_font() {
        let mut d = DesignerPanel::new(Form::new("F", "T", 640, 480));
        d.add_control(ControlType::Label, 10, 10);
        let first = d.form.controls[0].id.clone();
        assert_eq!(font_of(&d, &first), ("Arial".to_string(), 10));
    }
}

#[cfg(test)]
mod user_control_tests {
    use super::*;

    fn entry(id: &str, user_control: Option<&str>) -> UserControlEntry {
        let mut properties = HashMap::new();
        if let Some(name) = user_control {
            properties.insert("UserControl".to_string(), name.to_string());
        }
        UserControlEntry {
            id: id.to_string(),
            control_type: "GroupBox".to_string(),
            parent: None,
            x: 0,
            y: 0,
            w: 100,
            h: 50,
            z_order: 0,
            properties,
        }
    }

    fn child_entry(id: &str, parent: &str) -> UserControlEntry {
        let mut properties = HashMap::new();
        properties.insert("Caption".to_string(), "Save".to_string());
        UserControlEntry {
            id: id.to_string(),
            control_type: "Button".to_string(),
            parent: Some(parent.to_string()),
            x: 20,
            y: 30,
            w: 80,
            h: 28,
            z_order: 1,
            properties,
        }
    }

    #[test]
    fn circular_reference_detected_through_nested_definition() {
        let existing = vec![
            UserControlDef {
                name: "AddressBlock".to_string(),
                width: 200,
                height: 100,
                controls: vec![entry("PhoneEntry-1", Some("PhoneEntry"))],
            },
            UserControlDef {
                name: "PhoneEntry".to_string(),
                width: 120,
                height: 40,
                controls: vec![entry("CustomerCard-1", Some("CustomerCard"))],
            },
        ];
        let new_controls = vec![entry("AddressBlock-1", Some("AddressBlock"))];

        assert!(DesignerPanel::has_circular_user_control_reference(
            "CustomerCard",
            &new_controls,
            &existing,
        ));
    }

    #[test]
    fn deploy_user_control_qualifies_ids_and_remaps_parent() {
        let def = UserControlDef {
            name: "CustomerCard".to_string(),
            width: 240,
            height: 120,
            controls: vec![
                entry("GroupBox-1", None),
                child_entry("Button-1", "GroupBox-1"),
            ],
        };
        let mut designer = DesignerPanel::new(Form::new("F", "T", 640, 480));
        designer.form.snap_to_grid = false;

        designer.deploy_user_control(&def, 50, 60, &[def.clone()]);

        let root = designer
            .form
            .find_control("CustomerCard-1")
            .expect("root deployed");
        assert_eq!(root.control_type, ControlType::GroupBox);
        assert_eq!(
            root.get_prop("UserControl").unwrap().as_str(),
            "CustomerCard"
        );
        assert_eq!(
            (root.rect.x, root.rect.y, root.rect.w, root.rect.h),
            (50, 60, 240, 120)
        );
        let child = designer
            .form
            .find_control("CustomerCard-1-Button-1")
            .expect("child deployed");
        assert_eq!(child.parent.as_deref(), Some("CustomerCard-1"));
        assert_eq!(
            (child.rect.x, child.rect.y, child.rect.w, child.rect.h),
            (70, 90, 80, 28)
        );
    }

    #[test]
    fn deploy_user_control_expands_uncaptured_nested_definition() {
        let phone_def = UserControlDef {
            name: "PhoneEntry".to_string(),
            width: 120,
            height: 40,
            controls: vec![
                entry("PhoneRoot", None),
                child_entry("PhoneButton", "PhoneRoot"),
            ],
        };
        let mut nested_root = entry("PhoneSlot", Some("PhoneEntry"));
        nested_root.parent = Some("AddressRoot".to_string());
        nested_root.x = 10;
        nested_root.y = 12;
        nested_root.w = 120;
        nested_root.h = 40;
        let address_def = UserControlDef {
            name: "AddressBlock".to_string(),
            width: 300,
            height: 160,
            controls: vec![entry("AddressRoot", None), nested_root],
        };
        let defs = vec![address_def.clone(), phone_def];
        let mut designer = DesignerPanel::new(Form::new("F", "T", 640, 480));
        designer.form.snap_to_grid = false;

        designer.deploy_user_control(&address_def, 100, 100, &defs);

        let nested_child = designer
            .form
            .find_control("AddressBlock-1-PhoneSlot-PhoneButton")
            .expect("nested child expanded");
        assert_eq!(
            nested_child.parent.as_deref(),
            Some("AddressBlock-1-PhoneSlot")
        );
        assert_eq!(
            (
                nested_child.rect.x,
                nested_child.rect.y,
                nested_child.rect.w,
                nested_child.rect.h
            ),
            (130, 142, 80, 28)
        );
    }
}

#[cfg(test)]
mod live_control_tests {
    use super::*;

    /// `live_control` must carry the designed/live properties into the
    /// snapshot `draw_control` renders, so preview/run faces match the
    /// designer exactly (WYSIWYG).
    #[test]
    fn live_control_carries_designed_props_and_size() {
        let props: Vec<(String, String)> = vec![
            ("BackgroundColor".into(), "#112233FF".into()),
            ("Caption".into(), "Hi".into()),
            ("CornerRadius".into(), "9".into()),
        ];
        let c = live_control(
            "Button-1",
            cobolt_forms::ControlType::Button,
            egui::vec2(120.0, 36.0),
            props.iter().map(|(k, v)| (k, v)),
        );
        assert_eq!(c.get_prop("BackgroundColor").unwrap().as_str(), "#112233FF");
        assert_eq!(c.get_prop("Caption").unwrap().as_str(), "Hi");
        assert_eq!(c.get_prop("CornerRadius").unwrap().as_i64(), 9);
        assert_eq!((c.rect.w, c.rect.h), (120, 36));
        assert_eq!(c.id, "Button-1");
    }
}

#[cfg(test)]
mod text_align_tests {
    use super::*;

    #[test]
    fn label_text_alignment_maps_to_egui_align() {
        assert_eq!(text_halign("Left"), egui::Align::LEFT);
        assert_eq!(text_halign("Center"), egui::Align::Center);
        assert_eq!(text_halign("Right"), egui::Align::RIGHT);
        // Default / unknown → left.
        assert_eq!(text_halign(""), egui::Align::LEFT);
        assert_eq!(text_halign("???"), egui::Align::LEFT);
        // Lenient about compound 9-position values.
        assert_eq!(text_halign("MiddleRight"), egui::Align::RIGHT);
        assert_eq!(text_halign("TopCenter"), egui::Align::Center);
    }

    #[test]
    fn agent_batch_applies_and_undoes_as_one_step() {
        use crate::agent::{AgentChangeSet, AgentOp};
        let mut d = DesignerPanel::new(Form::new("F", "T", 640, 480));
        d.form
            .controls
            .push(Control::new("L1", ControlType::Label, 0, 0));
        let before = format!("{:?}", d.form);

        let mut props = serde_json::Map::new();
        props.insert("Caption".into(), serde_json::json!("Save"));
        let cs = AgentChangeSet {
            operations: vec![
                AgentOp::DeployControl {
                    control_type: "Button".into(),
                    id: Some("SAVE".into()),
                    properties: props,
                },
                AgentOp::SetProperty {
                    control_id: "L1".into(),
                    key: "Bold".into(),
                    value: serde_json::json!(true),
                },
                AgentOp::GenerateEventHandler {
                    control_id: "SAVE".into(),
                    event: "onClick".into(),
                    code: "       PROCEDURE DIVISION.\n           DISPLAY \"hi\".\n".into(),
                },
                AgentOp::CreateProcedure {
                    name: "VALIDATE-INPUT".into(),
                    code: "       PROCEDURE DIVISION.\n".into(),
                },
            ],
            note: None,
        };

        let n = d.apply_agent_change_set(&cs);
        assert_eq!(n, 4, "all four ops applied");
        assert!(d.form.find_control("SAVE").is_some(), "control deployed");
        assert_eq!(
            d.form.find_control("L1").unwrap().properties.get("Bold"),
            Some(&PropValue::Bool(true)),
            "property set"
        );
        assert!(
            d.form
                .find_control("SAVE")
                .unwrap()
                .events
                .iter()
                .any(|b| b.event.eq_ignore_ascii_case("onClick") && b.code.contains("DISPLAY")),
            "handler set"
        );
        assert!(
            d.form
                .user_procedures
                .iter()
                .any(|p| p.name == "VALIDATE-INPUT"),
            "procedure created"
        );

        // One undo reverts the entire change-set (R6).
        d.undo();
        assert_eq!(
            format!("{:?}", d.form),
            before,
            "single undo restores the pre-change form byte-for-byte"
        );

        // Redo re-applies it.
        d.redo();
        assert!(d.form.find_control("SAVE").is_some(), "redo re-applies");
    }

    #[test]
    fn agent_preview_approve_is_one_undo_reject_is_none() {
        use crate::agent::{AgentChangeSet, AgentOp, AgentPreview};
        let mut d = DesignerPanel::new(Form::new("F", "T", 640, 480));
        let cs = AgentChangeSet {
            operations: vec![
                AgentOp::DeployControl {
                    control_type: "Button".into(),
                    id: Some("B".into()),
                    properties: serde_json::Map::new(),
                },
                // one invalid op — must be counted as an error and skipped on approve
                AgentOp::SetProperty {
                    control_id: "GHOST".into(),
                    key: "Caption".into(),
                    value: serde_json::json!("x"),
                },
            ],
            note: None,
        };
        let preview = AgentPreview::build(cs, &d.form);
        assert_eq!(preview.valid_count(), 1, "one valid op");
        assert!(preview.has_errors(), "the GHOST op is flagged");
        assert!(preview.is_applicable());

        // Reject = do nothing: the undo stack stays empty (R7).
        assert_eq!(d.undo_stack.len(), 0);

        // Approve = apply the valid ops as one batch (R6): exactly one undo entry.
        let n = d.apply_agent_change_set(&preview.change_set);
        assert_eq!(n, 1, "only the valid op applies");
        assert_eq!(d.undo_stack.len(), 1, "approve is one AgentBatch = one Undo");
        assert!(d.form.find_control("B").is_some());
    }
}
