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
use crate::project_model::{CoboltProject, UserControlDef, UserControlEntry};
use cobolt_forms::render::{card_appear_transform, PlacementEffect};

/// Runtime state of the AI-pane layout debug trace (`[ai-pane]` lines on stderr).
/// Driven by the IDE's Project Settings "AI-pane layout debug" toggle each frame;
/// seeds from the `COBOLT_AI_PANE_DEBUG` env var until the setting overrides it.
static AI_PANE_DEBUG: std::sync::atomic::AtomicU8 = std::sync::atomic::AtomicU8::new(0);

/// Turn the AI-pane layout debug trace on/off at runtime. An explicit call always
/// wins over the env var.
pub fn set_ai_pane_debug(on: bool) {
    AI_PANE_DEBUG.store(if on { 2 } else { 1 }, std::sync::atomic::Ordering::Relaxed);
}

/// Emit an `[ai-pane]` trace line, but only when it differs from the last one.
///
/// The pane lays itself out every frame, so the trace fired ~60 times a second
/// with identical text — enough to make the terminal useless and to bury any
/// other output, including the change the developer was watching for. A layout
/// trace is interesting exactly when the layout *changes*, so that is when it
/// prints. Repeats are counted and reported once, when something finally moves,
/// so a stuck layout is still visible as a stuck layout.
fn trace_ai_pane_layout(line: String) {
    use std::sync::Mutex;
    static STATE: Mutex<TraceDedupe> = Mutex::new(TraceDedupe::new());
    let Ok(mut state) = STATE.lock() else {
        return;
    };
    for out in state.next(line) {
        eprintln!("{out}");
    }
}

/// Collapses a repeating trace line down to "print it when it changes".
#[derive(Default)]
struct TraceDedupe {
    last: Option<String>,
    repeats: u64,
}

impl TraceDedupe {
    const fn new() -> Self {
        Self {
            last: None,
            repeats: 0,
        }
    }

    /// What to print for `line`, if anything. A repeat prints nothing and is
    /// counted; the count is reported when the value finally changes, so a
    /// layout that is stuck still reads as stuck rather than as silence.
    fn next(&mut self, line: String) -> Vec<String> {
        match &self.last {
            Some(prev) if *prev == line => {
                self.repeats += 1;
                Vec::new()
            }
            Some(_) => {
                let mut out = Vec::new();
                if self.repeats > 0 {
                    out.push(format!(
                        "[ai-pane] (unchanged for {} more frame(s))",
                        self.repeats
                    ));
                }
                out.push(line.clone());
                self.last = Some(line);
                self.repeats = 0;
                out
            }
            None => {
                self.last = Some(line.clone());
                self.repeats = 0;
                vec![line]
            }
        }
    }
}

fn ai_pane_debug() -> bool {
    use std::sync::atomic::Ordering;
    match AI_PANE_DEBUG.load(Ordering::Relaxed) {
        1 => false,
        2 => true,
        _ => {
            let on = std::env::var_os("COBOLT_AI_PANE_DEBUG").is_some();
            AI_PANE_DEBUG.store(if on { 2 } else { 1 }, Ordering::Relaxed);
            on
        }
    }
}

// The prompt/input block is a slab pinned to the pane bottom: 170px of fixed
// chrome around a prompt box whose height the user may drag between 1 and 6
// text rows (default 3 — at the default the slab is exactly 170px). The
// history takes everything above it, so the pane's top-edge resizer resizes
// the history (default pane height gives it ~70px to start).
const GLOBAL_AI_HISTORY_MIN_HEIGHT: f32 = 48.0;
/// Vertical chrome INSIDE the prompt box (frame border + the TextEdit's own
/// margins). The old 4.0 under-counted it, so the last text row's descenders
/// were clipped at the box's bottom edge.
const GLOBAL_AI_PROMPT_CHROME: f32 = 12.0;
/// Padding between the prompt box's border and its text viewport, per side.
/// Two of these are the box's own chrome; the rest of
/// [`GLOBAL_AI_PROMPT_CHROME`] is the slack that keeps the last row's
/// descenders clear of the border.
const PROMPT_FRAME_MARGIN: f32 = 4.0;
/// Width of that frame's border. egui draws it OUTSIDE the margin box, so the
/// text viewport must give it back or the box overshoots its dragged height.
const PROMPT_FRAME_STROKE: f32 = 1.0;
/// Breathing room UNDER the prompt box, before the status/history-controls
/// rows, so the box never sits flush against the pane below it.
const GLOBAL_AI_PROMPT_BOTTOM_PAD: f32 = 6.0;
/// Fixed input-slab chrome at the DEFAULT prompt height. Includes the prompt
/// box chrome and its bottom pad above — grow those and this must grow with
/// them, or the slab clips its own controls.
const GLOBAL_AI_INPUT_HEIGHT: f32 = 184.0;
const GLOBAL_AI_PANE_MIN_HEIGHT: f32 = GLOBAL_AI_HISTORY_MIN_HEIGHT + GLOBAL_AI_INPUT_HEIGHT + 24.0;

/// Is this `ai_status` a work-in-progress message rather than a failure?
///
/// The field carries both, and the pane's footer treats everything that is not
/// progress as an error — so a new progress message that forgets to register
/// here is shown to the developer as "AI error" with a Details button, in the
/// model indicator's place. That is exactly what happened to Grace's review
/// status (operator, 2026-07-31). Register progress text here, or use one of
/// the strings already listed.
pub(crate) fn status_is_progress(status: &str, tr: &crate::i18n::Tr) -> bool {
    status == "Thinking..." || status == tr.review_working
}

// ── Collapsible designer chrome (spec 033) ──────────────────────────────────
// Fixed geometry for the toolbox rail and properties drawer. These are CONSTANTS,
// never derived from available/max space, so the collapsed/hidden states can't
// self-inflate. The expanded widths live in `DesignerPanel::{toolbox,props}_width`
// and are only ever written from the panel's own resized rect (a user drag).

/// Default (seed) width of the expanded left sidebar (forms list + toolbox).
pub const TOOLBOX_DEFAULT_W: f32 = 150.0;
/// Minimum width the expanded left sidebar may be dragged to.
pub const TOOLBOX_MIN_W: f32 = 130.0;
/// Fixed width of the collapsed toolbox icon rail. Wide enough for one 49px icon
/// button plus the side-panel frame margins (8+8) and a scrollbar.
pub const TOOLBOX_RAIL_W: f32 = 78.0;

/// Default (seed) width of the properties pane.
pub const PROPS_DEFAULT_W: f32 = 300.0;
/// Minimum width the properties pane may be dragged to.
pub const PROPS_MIN_W: f32 = 220.0;
/// Fixed width of the thin reopen/hide tab strip beside the properties pane —
/// wide enough to hold the enlarged collapse chevron without clipping it.
pub const PROPS_TAB_W: f32 = 30.0;
/// Glyph size for every collapse/expand chevron (toolbox ◀/▶ and properties
/// ◀/▶) so they are all the SAME size — ~2× the old `small_button` glyph.
pub const COLLAPSE_CHEVRON_SIZE: f32 = 20.0;

/// Clamp a captured expanded left-sidebar width into `[min, max]`.
///
/// The input is the panel's own resized outer width (user drag / persisted /
/// default) — never available space — so this is a pure clamp, called once per
/// frame to keep `toolbox_width` a faithful record of the user's chosen width.
pub fn clamp_toolbox_width(width: f32, min: f32, max: f32) -> f32 {
    clamp_pane_width(width, min, max, TOOLBOX_DEFAULT_W)
}

/// Clamp a captured properties-pane width into `[min, max]`. Same contract as
/// [`clamp_toolbox_width`]. (The properties drawer now lets egui persist its own
/// width per panel id, so this is retained only for its tests / future use.)
#[allow(dead_code)]
pub fn clamp_props_width(width: f32, min: f32, max: f32) -> f32 {
    clamp_pane_width(width, min, max, PROPS_DEFAULT_W)
}

/// Shared width-restore helper: clamp `width` into `[min, max]`, falling back to
/// `fallback` when the value is non-finite or non-positive (e.g. a transient
/// zero rect on the first frame). Guarantees the stored width is always a sane,
/// bounded seed for the next expand — never a runaway value.
fn clamp_pane_width(width: f32, min: f32, max: f32, fallback: f32) -> f32 {
    let hi = max.max(min);
    if !width.is_finite() || width <= 0.0 {
        return fallback.clamp(min, hi);
    }
    width.clamp(min, hi)
}

#[cfg(test)]
mod collapsible_chrome_tests {
    use super::*;

    /// A user-dragged width is recorded verbatim (clamped to the pane's range),
    /// so re-expanding restores exactly what the user set.
    #[test]
    fn dragged_width_is_recorded_within_range() {
        assert_eq!(clamp_toolbox_width(210.0, TOOLBOX_MIN_W, 600.0), 210.0);
        assert_eq!(clamp_props_width(420.0, PROPS_MIN_W, 800.0), 420.0);
    }

    /// Captured widths are clamped to the pane's min/max — never allowed to be a
    /// runaway value that would let the pane grow past its bounds on re-expand.
    #[test]
    fn width_is_clamped_to_bounds() {
        // Below min → snaps up to min.
        assert_eq!(clamp_toolbox_width(10.0, TOOLBOX_MIN_W, 600.0), TOOLBOX_MIN_W);
        // Above max → snaps down to max.
        assert_eq!(clamp_props_width(5000.0, PROPS_MIN_W, 700.0), 700.0);
    }

    /// A transient zero / non-finite rect (e.g. first frame) falls back to the
    /// sane default seed instead of poisoning the stored width with 0.
    #[test]
    fn non_positive_or_nan_falls_back_to_default() {
        assert_eq!(
            clamp_toolbox_width(0.0, TOOLBOX_MIN_W, 600.0),
            TOOLBOX_DEFAULT_W
        );
        assert_eq!(
            clamp_props_width(f32::NAN, PROPS_MIN_W, 700.0),
            PROPS_DEFAULT_W
        );
    }

    /// The restore contract: while collapsed we must NOT overwrite the stored
    /// expanded width, so on expand `default_size` still opens at the user's
    /// last width. This models the app-side capture guard.
    #[test]
    fn collapse_preserves_expanded_width() {
        let mut stored = 265.0_f32;
        let rail_rect_width = TOOLBOX_RAIL_W; // what the rail panel reports while collapsed

        // Collapsed: capture is skipped → stored is untouched.
        let collapsed = true;
        if !collapsed {
            stored = clamp_toolbox_width(rail_rect_width, TOOLBOX_MIN_W, 600.0);
        }
        assert_eq!(stored, 265.0, "collapse must not clobber the expanded width");

        // Expanded again, user has not dragged: rect == stored → idempotent.
        let collapsed = false;
        if !collapsed {
            stored = clamp_toolbox_width(stored, TOOLBOX_MIN_W, 600.0);
        }
        assert_eq!(stored, 265.0, "re-expand restores the user's width");
    }
}

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

impl UserControlNameError {
    /// The message the developer reads. One source for both the dialog label
    /// and the IDE console entry (operator, 2026-08-09), so the console can
    /// never quote different wording from the dialog it came from.
    fn message(self, tr: crate::i18n::Tr) -> &'static str {
        match self {
            UserControlNameError::Empty | UserControlNameError::Invalid => tr.uc_name_invalid,
            UserControlNameError::Duplicate => tr.uc_name_duplicate,
            UserControlNameError::Circular => tr.uc_circular_ref,
        }
    }
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

/// The animation transform math lives in `cobolt_forms::anim` so every surface —
/// designer canvas, preview, the standalone run-form process and compiled
/// binaries — plays an animation identically (it used to be IDE-only, which is
/// why Run Form showed no animation at all).
pub(crate) use cobolt_forms::anim::anim_transform;

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
    /// One form-level property change (Title, Width, Theme, gradients, …) —
    /// undoable like any control property (operator, 2026-07-28: "any change
    /// in the form should be possible to undo").
    SetFormProp {
        key: String,
        old: String,
        new: String,
    },
    /// One of the form's raw-COBOL structure blocks (SPECIAL-NAMES, REPOSITORY,
    /// FILE-CONTROL, FILE SECTION, WORKING-STORAGE) — the outermost program's
    /// own declarations, which only a `set_form_structure` operation or the
    /// COBOL Structure panel can write.
    SetFormStructure {
        block: String,
        old: String,
        new: String,
    },
    /// A GlassStyle switch. Applying a Neumorphic style bulldozes appearance
    /// defaults across EVERY control, so reverting the style enum alone would
    /// leave the bulldozed colours behind — undo restores the full pre-switch
    /// snapshot of the controls and the form appearance fields the style
    /// appliers touch.
    SetGlassStyle {
        before: Box<StyleSnapshot>,
        style: String,
    },
    /// A control's full animation list (add / remove / field edit — the
    /// `_AddAnimation` / `_RemoveAnimN` / `AnimN_*` meta-keys returned before
    /// the stack until the 2026-07-29 audit). Whole-list snapshots keep the
    /// three operation kinds on one variant.
    SetAnimations {
        id: String,
        old: Vec<AnimationDef>,
        new: Vec<AnimationDef>,
    },
    /// A user procedure added through the COBOL Structure panel.
    AddProcedure {
        index: usize,
        proc: cobolt_forms::model::UserProcedure,
    },
    /// A user procedure deleted through the COBOL Structure panel — the whole
    /// procedure (name + code) rides along so undo restores it verbatim.
    RemoveProcedure {
        index: usize,
        proc: cobolt_forms::model::UserProcedure,
    },
    /// A data binding applied from the binding editor. Binding application
    /// rewrites target-control properties (DataGrid columns, sources, preview
    /// values), so the pre-apply bindings AND controls are snapshotted.
    ApplyDataBinding {
        binding: cobolt_forms::DataBindingDef,
        before_bindings: Vec<cobolt_forms::DataBindingDef>,
        before_controls: Vec<Control>,
    },
    /// A MenuBar definition save. The menu lives in a YAML next to the .cfrm,
    /// so execute/reverse rewrite the file (`old: None` means the menu did not
    /// exist — undo removes the file) and queue a paint-cache refresh.
    SetMenuDefinition {
        control_id: String,
        old: Option<cobolt_forms::menu::MenuDefinition>,
        new: cobolt_forms::menu::MenuDefinition,
    },
}

/// Which direction of history navigation is waiting on the developer's
/// confirmation (procedure-touching steps only — operator, 2026-07-29).
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum HistoryDir {
    Undo,
    Redo,
}

/// Whether reverting/re-applying this command changes COBOL procedure code —
/// those history steps require explicit developer confirmation.
fn touches_procedures(cmd: &Cmd) -> bool {
    match cmd {
        Cmd::SetProcedure { .. } | Cmd::AddProcedure { .. } | Cmd::RemoveProcedure { .. } => true,
        Cmd::AgentBatch { cmds } => cmds.iter().any(touches_procedures),
        _ => false,
    }
}

/// Everything `Form::apply_glass_style_defaults` can touch — captured before a
/// GlassStyle switch so [`Cmd::SetGlassStyle`] restores the exact pre-switch
/// appearance on undo.
#[derive(Clone)]
struct StyleSnapshot {
    glass_style: cobolt_forms::GlassStyle,
    background_color: String,
    background_gradient_enabled: bool,
    background_gradient_start_color: String,
    background_gradient_end_color: String,
    background_gradient_direction: String,
    controls: Vec<Control>,
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
    // The control being resized, for its own floor: a ListBox may not be
    // dragged below one line of its own text (`min_control_size`). `None`
    // keeps the universal 8x8.
    ctrl: Option<&Control>,
) -> cobolt_forms::model::Rect {
    let (min_w, min_h) = ctrl
        .map(cobolt_forms::model::min_control_size)
        .unwrap_or((8, 8));
    let s = |v| snap(v, grid_px, snapping);
    let mut nr = r;
    match h {
        Handle::TopLeft => {
            nr.x = s(r.x + dx);
            nr.y = s(r.y + dy);
            nr.w = (r.w - dx).max(min_w);
            nr.h = (r.h - dy).max(min_h);
        }
        Handle::Top => {
            nr.y = s(r.y + dy);
            nr.h = (r.h - dy).max(min_h);
        }
        Handle::TopRight => {
            nr.y = s(r.y + dy);
            nr.w = s(r.w + dx).max(min_w);
            nr.h = (r.h - dy).max(min_h);
        }
        Handle::Left => {
            nr.x = s(r.x + dx);
            nr.w = (r.w - dx).max(min_w);
        }
        Handle::Right => {
            nr.w = s(r.w + dx).max(min_w);
        }
        Handle::BotLeft => {
            nr.x = s(r.x + dx);
            nr.w = (r.w - dx).max(min_w);
            nr.h = s(r.h + dy).max(min_h);
        }
        Handle::Bot => {
            nr.h = s(r.h + dy).max(min_h);
        }
        Handle::BotRight => {
            nr.w = s(r.w + dx).max(min_w);
            nr.h = s(r.h + dy).max(min_h);
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
    /// 049 — dragging the seam between a SideMenu's header (or footer) pane and
    /// its menu. DESIGN TIME ONLY: the pane heights are the developer's, and at
    /// run time nothing may resize them.
    ResizingSidebarPane {
        id: String,
        pane: SidebarPane,
        orig_h: i32,
        start_y: i32,
    },
}

/// Which of a SideMenu's fixed panes a seam drag is sizing.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SidebarPane {
    Header,
    Footer,
}

impl SidebarPane {
    fn prop(self) -> &'static str {
        match self {
            SidebarPane::Header => "HeaderHeight",
            SidebarPane::Footer => "FooterHeight",
        }
    }
}

/// How close to a seam the pointer must be to grab it, in canvas points.
const SIDEBAR_SEAM_TOL: f32 = 5.0;
/// The smallest either pane may be dragged to. Zero is left reachable for the
/// footer — a rail with no footer band is a legitimate design — but the header
/// keeps enough to show a logo.
const SIDEBAR_PANE_MIN: i32 = 0;

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

/// How much of a roaming modal must stay on screen, in points.
const MODAL_KEEP_ON_SCREEN: f32 = 120.0;

/// The rect a big designer modal may be dragged within.
///
/// `movable(true)` alone did not give the developer a window they could move.
/// egui constrains a window's whole RECT, not just its title bar, so a modal
/// that opens at 70 % of the screen can only travel the leftover 30 % before it
/// is clamped back — it reads as pinned. Constraining to a rect that extends
/// PAST the screen gives the whole desk to push it around on.
///
/// It stays recoverable. Each side is widened by (size − [`MODAL_KEEP_ON_SCREEN`]),
/// so however far the window is pushed, that many points of it remain on screen.
/// The top is deliberately NOT widened: a title bar dragged above the screen's
/// top edge could never be grabbed again, which is exactly how a movable window
/// gets lost for good.
///
/// Note this is about the CONSTRAINT. A window built with `.anchor(…)` is
/// pinned outright and cannot be dragged at all, whatever it is constrained to —
/// seed the position with `.default_pos(…)` instead.
/// The COBOL Structure editor box opens tall enough for this many code lines.
/// It is a SEED for the box's `egui::Resize` state: after the first frame only
/// the user's grip drag changes the box. Content, language and screen size can
/// never resize it — deriving the box from `available_*` space is the feedback
/// loop that made this window inflate or pin itself in past releases.
const CS_EDITOR_DEFAULT_ROWS: f32 = 12.0;
/// The grip cannot shrink the editor box below this many code lines.
const CS_EDITOR_MIN_ROWS: f32 = 4.0;
/// Vertical chrome inside the editor box (the 2 px frame margins plus an
/// allowance for the horizontal scrollbar), so the default really shows
/// [`CS_EDITOR_DEFAULT_ROWS`] full lines.
const CS_EDITOR_BOX_CHROME: f32 = 14.0;
/// Nominal window chrome above+below the editor box (title bar, hint, status
/// row, AI bar, buttons). Used ONLY to seed the centred `default_pos` and the
/// roam constraint — never to size anything.
const CS_WINDOW_CHROME_NOMINAL: f32 = 260.0;

fn modal_roam_rect(screen: egui::Rect, w: f32, h: f32) -> egui::Rect {
    let slack_x = (w - MODAL_KEEP_ON_SCREEN).max(0.0);
    let slack_y = (h - MODAL_KEEP_ON_SCREEN).max(0.0);
    egui::Rect::from_min_max(
        egui::pos2(screen.left() - slack_x, screen.top()),
        egui::pos2(screen.right() + slack_x, screen.bottom() + slack_y),
    )
}

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
    "Transparency",
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

/// May the format painter carry this property from one control to another?
///
/// Between controls of the SAME type the painter deep-copies: the developer
/// styled one and wants the other to match, and an allowlist of nineteen keys
/// silently dropped everything else — shadows, icon size and effect, padding,
/// alignment, gradients, the highlight/selected colours. So the rule is
/// inverted here: everything is appearance UNLESS it is one of the three
/// things a copy must never touch.
///
/// 1. **Data binding.** The Form owns the binding table (so bindings are not
///    copied at all), but a control can still name its own source, and pasting
///    that would silently rebind the target to the source's data.
/// 2. **Content.** What a control SAYS is not how it looks. Pasting a caption,
///    a value or a row set overwrites the developer's data with the source's.
/// 3. **Configuration that does not change the UI.** A Timer's interval, a
///    database's connection string, a REST endpoint, an agent's model: copying
///    a look must never repoint a control at a different service.
///
/// Events are not properties and are never copied on any path.
fn is_copyable_style_prop(key: &str) -> bool {
    // Data binding, by prefix: `DataSource`, `DataField`, `BindingMode`, …
    if key.starts_with("Data") || key.starts_with("Binding") {
        return false;
    }
    const NEVER: &[&str] = &[
        // Identity and stacking — per instance by definition. Pasting a tab
        // order or a z-order makes two controls claim one position.
        "Name",
        "TabOrder",
        "ZOrder",
        // Content.
        "Caption",
        "Text",
        "Value",
        "Items",
        "Rows",
        "Columns",
        "Checked",
        "SelectedIndex",
        "SelectedItem",
        "SelectedItemId",
        "SelectedTab",
        "Minimum",
        "Maximum",
        "Placeholder",
        "ToolTip",
        // Configuration with no bearing on the UI.
        "Interval",
        "AutoStart",
        "ConnectionString",
        "Query",
        "Url",
        "Endpoint",
        "Method",
        "Headers",
        "Body",
        "ApiKey",
        "Model",
        "Provider",
        "Prompt",
        "Timeout",
        "FilePath",
        "FileName",
    ];
    !NEVER.contains(&key)
}

/// State machine for the format-painter (copy style) tool.
///
/// New UX flow:
///   1. User selects the source control on the canvas normally.
///   2. User clicks "🖌 Copy Style" — style is captured immediately from the selection.
///   3. Painter enters `WaitingForTarget`; cursor becomes a crosshair.
///   4. User clicks one or more target controls → style is pasted and the
///      painter remains active.
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
        /// What the style was taken FROM. A target of the same type gets the
        /// deep copy; a different type gets only the properties that mean the
        /// same thing everywhere, since one control's `IconSize` is another's
        /// nonsense.
        src_type: ControlType,
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
    /// 051 R16 — the edited menu belongs to a SideMenu (the editor is shared
    /// with MenuBar): only a SideMenu's menu offers the standalone actions.
    pub is_side_menu: bool,
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
            is_side_menu: false,
        }
    }

    /// 051 R16 — builder: mark this menu as a SideMenu's.
    pub fn for_side_menu(mut self, is_side_menu: bool) -> Self {
        self.is_side_menu = is_side_menu;
        self
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
                    } else if let Some(rest) = a.strip_prefix("open-standalone-sync:") {
                        rest.to_string()
                    } else if let Some(rest) = a.strip_prefix("open-standalone-async:") {
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
            // Home takes no target: it shows the shell form's own pane.
            Some("home") => "home",
            Some(a) if a.starts_with("open-form:") => "open-form",
            // 051 R16/R17 — the SideMenu's standalone pair.
            Some(a) if a.starts_with("open-standalone-sync:") => "open-standalone-sync",
            Some(a) if a.starts_with("open-standalone-async:") => "open-standalone-async",
            _ => "event",
        }
    }

    /// 051 R16 — the Action combo's choices: a SideMenu's menu offers six
    /// (Home, then the standalone pair between the form loaders and Close), a
    /// MenuBar's the classic three.
    ///
    /// Home is SideMenu-only on purpose: it restores the shell's ContentPane,
    /// and only a SideMenu form has one.
    pub(crate) fn action_type_options(is_side_menu: bool) -> Vec<&'static str> {
        if is_side_menu {
            vec![
                "event",
                "open-form",
                "home",
                "open-standalone-sync",
                "open-standalone-async",
                "close",
            ]
        } else {
            vec!["event", "open-form", "close"]
        }
    }

    /// 051 R16 — every action type that picks a target form, with the
    /// persisted prefix it writes (`open-form:` needs `Embedded`/`Both`
    /// targets; the standalone pair needs `Standalone`/`Both`).
    fn action_prefix(kind: &str) -> Option<&'static str> {
        match kind {
            "open-form" => Some("open-form:"),
            "open-standalone-sync" => Some("open-standalone-sync:"),
            "open-standalone-async" => Some("open-standalone-async:"),
            _ => None,
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
    /// Streaming buffer for the current in-flight AI request.
    pub ai_streaming_reply: String,
    /// Last AI error to surface below the prompt row (`None` ⇒ no error).
    ai_status: Option<String>,
    /// Syntax errors blocking a save/close. `Some` ⇒ the syntax-error modal is up.
    syntax_errors: Option<Vec<crate::runner::DiagMsg>>,
    /// Per-handler conversation history — replayed to the model on each request and
    /// persisted independently for this event handler. Loaded lazily on first open.
    ai_history: Vec<crate::llm::ChatTurn>,
    /// Whether `ai_history` has been loaded from disk yet.
    ai_loaded: bool,
    /// In-flight compaction (summarization) request for this handler.
    ai_compact_pending: Option<std::sync::mpsc::Receiver<crate::llm::LlmResponse>>,
    /// Whether the clear-history confirmation dialog is showing.
    ai_confirm_clear: bool,
    /// Transcript auto-scroll bookkeeping: the number of turns rendered last frame.
    /// When the history grows past this, the transcript scrolls to the newest turn
    /// once (so a fresh reply is visible) without yanking the view while the user
    /// is scrolled up reading.
    ai_last_seen_turns: usize,
    /// How many times the assistant has been auto-asked to fix invalid COBOL for
    /// the current request. Reset when the developer sends a new prompt; capped by
    /// [`MAX_AI_FIX_ATTEMPTS`] so a model that can't recover doesn't loop forever.
    ai_fix_attempts: u8,
}

/// Maximum automatic "your COBOL doesn't parse — fix it" round-trips per request
/// before the assistant gives up and surfaces the errors (leaving the developer's
/// existing code untouched).
const MAX_AI_FIX_ATTEMPTS: u8 = 3;

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
            ai_streaming_reply: String::new(),
            ai_status: None,
            syntax_errors: None,
            ai_history: Vec::new(),
            ai_loaded: false,
            ai_compact_pending: None,
            ai_confirm_clear: false,
            ai_last_seen_turns: 0,
            ai_fix_attempts: 0,
        }
    }
}

/// Conversation-store key for an event handler. The nested `PROGRAM-ID` is unique
/// per control+event, and the `__events__/` prefix keeps it from colliding with a
/// source-file conversation key.
fn event_history_key(program_id: &str) -> String {
    format!("__events__/{program_id}")
}

/// Load a handler's saved conversation (empty when there is no project or none
/// stored yet).
fn load_event_history(
    project_root: Option<&std::path::Path>,
    program_id: &str,
) -> Vec<crate::llm::ChatTurn> {
    match project_root {
        Some(root) => crate::llm::load_history(&root.join("data"), &event_history_key(program_id)),
        None => Vec::new(),
    }
}

/// Persist a handler's conversation (no-op without an open project). An empty
/// `turns` deletes the stored record.
fn save_event_history(
    project_root: Option<&std::path::Path>,
    program_id: &str,
    turns: &[crate::llm::ChatTurn],
) {
    if let Some(root) = project_root {
        crate::llm::save_history(&root.join("data"), &event_history_key(program_id), turns);
    }
}

/// Validate the **syntax** of an event handler's COBOL body by wrapping it in the
/// same `IDENTIFICATION`/`PROGRAM-ID` … `END PROGRAM` scaffold the generator emits
/// and running the parser. Only *parse* (syntax) diagnostics are returned — the
/// handler is parsed in isolation, so semantic checks (which would false-flag the
/// form's shared data items and controls) are intentionally skipped. Diagnostic
/// line numbers are mapped back to the editor's coordinate space (the two-line
/// scaffold header is subtracted).
fn validate_handler_syntax(program_id: &str, body: &str) -> Vec<crate::runner::DiagMsg> {
    use crate::runner::{DiagMsg, DiagSeverity};
    if let Some(message) = crate::agent::handler_body_shape_error(body) {
        return vec![DiagMsg {
            severity: DiagSeverity::Error,
            message,
            line: 1,
            col: 1,
        }];
    }
    // Two header lines precede the editable body.
    const HEADER_LINES: u32 = 2;
    let src = format!(
        "       IDENTIFICATION DIVISION.\n       PROGRAM-ID. {program_id}.\n{body}\n       END PROGRAM {program_id}.\n"
    );
    let pr = cobolt_parser::parse(cobolt_lexer::tokenize(
        &src,
        cobolt_lexer::SourceFormat::Free,
    ));
    let mut diags = Vec::new();
    for d in &pr.diagnostics {
        if !matches!(d.severity, cobolt_parser::Severity::Error) {
            continue; // syntax errors only
        }
        let line = d.span.line.saturating_sub(HEADER_LINES).max(1);
        diags.push(DiagMsg {
            severity: DiagSeverity::Error,
            message: d.message.clone(),
            line,
            col: d.span.col,
        });
    }
    diags
}

/// Validate that every `Control::property` / `Control::method(...)` referenced in a
/// handler body actually exists on that control. Returns one `DiagMsg` per unknown
/// member (with its source line), so a hallucinated property (`TextBox-2::Depth`)
/// or method is caught at save time. Uses the same property/method registries the
/// IntelliSense and the dev-agent gate use, so it never flags a real member.
fn validate_handler_members(form: &Form, code: &str) -> Vec<crate::runner::DiagMsg> {
    use crate::runner::{DiagMsg, DiagSeverity};
    let known = super::editor::build_known_controls(form);
    let mut out = Vec::new();
    for r in crate::agent::scan_member_refs(code) {
        // Only check references to controls this form actually has.
        let Some(kc) = known.iter().find(|k| k.id.eq_ignore_ascii_case(&r.recv)) else {
            continue;
        };
        // User Controls / Custom types expose developer-defined members not in the
        // built-in registries — don't second-guess them.
        if kc.ctrl_type.starts_with("Custom") || kc.ctrl_type.starts_with("UserControl") {
            continue;
        }
        let (kind, ok) = if r.is_call {
            let ok = super::editor::method_names_for_type(&kc.ctrl_type)
                .iter()
                .any(|m| m.eq_ignore_ascii_case(&r.member))
                || kc
                    .extra_methods
                    .iter()
                    .any(|m| m.eq_ignore_ascii_case(&r.member));
            ("method", ok)
        } else {
            let ok = kc
                .properties
                .iter()
                .any(|p| p.eq_ignore_ascii_case(&r.member));
            ("property", ok)
        };
        if !ok {
            out.push(DiagMsg {
                severity: DiagSeverity::Error,
                message: format!("Control '{}' has no {} '{}'.", r.recv, kind, r.member),
                line: r.line,
                col: 1,
            });
        }
    }
    out
}

/// Semantically validate a candidate handler body **in the context of the whole
/// form**. Isolated parsing (see [`validate_handler_syntax`]) can't catch a data
/// name that the handler *uses* but no longer *declares* — e.g. when the assistant
/// drops the DATA DIVISION while "only" translating comments, leaving a
/// PROCEDURE-only program that parses fine but references an undeclared item. Here
/// the candidate is spliced into a clone of the form, the whole program is
/// regenerated, and the same semantic analyser the runner uses resolves every
/// identifier — so form-global data items resolve (no false positives) while a
/// genuinely undeclared name is reported. Only the errors falling inside this
/// handler's generated nested program are returned, mapped back to editor lines.
fn validate_handler_semantics(
    form: &Form,
    ctrl_id: &str,
    event_name: &str,
    program_id: &str,
    candidate: &str,
) -> Vec<crate::runner::DiagMsg> {
    use crate::runner::{DiagMsg, DiagSeverity};

    // Splice the candidate into a clone and regenerate the complete program.
    let mut probe = form.clone();
    if ctrl_id.is_empty() {
        if let Some(ev) = probe.form_events.iter_mut().find(|e| e.event == event_name) {
            ev.code = candidate.to_string();
        }
    } else if let Some(ctrl) = probe.find_control_mut(ctrl_id) {
        ctrl.ensure_event(event_name);
        if let Some(ev) = ctrl.events.iter_mut().find(|e| e.event == event_name) {
            ev.code = candidate.to_string();
        }
    }
    let src = cobolt_codegen::generate(&probe);
    let parsed = cobolt_parser::parse(cobolt_lexer::tokenize(
        &src,
        cobolt_lexer::SourceFormat::Free,
    ));
    // If the full program didn't even parse, leave it to the syntax check.
    let Some(program) = parsed.program else {
        return Vec::new();
    };
    // Spec 044 R20 — the service wrapper allows registered External Crates.
    let sem = crate::external_crates_service::analyze_project(&program);

    // Locate this handler's nested program (`PROGRAM-ID. <id>.` … `END PROGRAM
    // <id>.`) so only its diagnostics are attributed to the candidate.
    let pid_marker = format!("PROGRAM-ID. {program_id}.");
    let end_marker = format!("END PROGRAM {program_id}.");
    let Some(start) = src
        .lines()
        .position(|l| l.trim_start().starts_with(&pid_marker))
    else {
        return Vec::new();
    };
    let end = src
        .lines()
        .enumerate()
        .skip(start + 1)
        .find(|(_, l)| l.contains(&end_marker))
        .map(|(i, _)| i)
        .unwrap_or(usize::MAX);

    let mut out = Vec::new();
    for d in sem.errors() {
        // `start` is 0-based; the PROGRAM-ID line is 1-based `start + 1`, so the
        // editable body runs 1-based [start + 2 ..= end].
        let line0 = d.span.line as usize;
        if line0 > start + 1 && line0 <= end {
            let editor_line = line0.saturating_sub(start + 1).max(1) as u32;
            out.push(DiagMsg {
                severity: DiagSeverity::Error,
                message: d.message.clone(),
                line: editor_line,
                col: d.span.col,
            });
        }
    }
    out
}

/// A short, plain-English hint for a raw parser message, so the modal explains the
/// error as well as showing it verbatim. Falls back to generic guidance.
fn explain_syntax_error(message: &str) -> &'static str {
    let m = message.to_ascii_lowercase();
    if m.contains("has no property") {
        "That property does not exist on this control. Use a real one — e.g. a control's depth (Neumorphic) is ShadowBlurStrength, not Depth. Check the properties pane."
    } else if m.contains("has no method") {
        "That method does not exist on this control. Type `control-id::` in the editor to see the methods it actually supports."
    } else if m.contains("period") || m.contains("'.'") || m.contains("expected `.`") {
        "A COBOL sentence must end with a period. Add a `.` at the end of the statement."
    } else if m.contains("expected") && (m.contains("identifier") || m.contains("name")) {
        "The parser expected a name (a data item, paragraph, or control id) at this point."
    } else if m.contains("unexpected") || m.contains("unmatched") {
        "There is an out-of-place or unmatched token here — check for a missing keyword, quote, or scope terminator (e.g. END-IF, END-PERFORM)."
    } else if m.contains("string") || m.contains("quote") {
        "A quoted literal is not closed — add the missing closing quote."
    } else {
        "Correct the highlighted statement so it matches COBOL-85 syntax, then check again."
    }
}

// ── Animated agent control moves (spec 035) ────────────────────────────────────

/// One control gliding from its pre-change position to the agent's new one. The
/// model already holds `to`; only the *drawn* position interpolates (R5).
#[derive(Debug, Clone, PartialEq)]
pub struct MoveAnim {
    pub id: String,
    pub from: egui::Pos2,
    pub to: egui::Pos2,
}

/// Duration of an agent control-move animation, in seconds (spec 035, R3).
const MOVE_ANIM_SECS: f64 = 1.0;

/// Ease-in-out over `[0,1]` (cubic): smooth acceleration and settle (R3).
fn eased(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    if t < 0.5 {
        4.0 * t * t * t
    } else {
        let f = 2.0 * t - 2.0;
        1.0 + f * f * f / 2.0
    }
}

/// The draw offset for a moving control at progress `t`: `lerp(from,to,eased) - to`.
/// Zero at `t≥1`, so the control rests exactly at its final (model) position (R5).
fn move_offset(from: egui::Pos2, to: egui::Pos2, t: f32) -> egui::Vec2 {
    let e = eased(t);
    let cur = from + (to - from) * e;
    cur - to
}

/// Build move animations by diffing a `before` snapshot of control positions
/// against the form after an agent change-set. Only a control that existed
/// before, still exists, kept the **same parent**, and whose `(x,y)` changed is
/// animated — created / deleted / unmoved / reparented controls are not (R1, R7,
/// R8, Q3).
fn diff_moves(
    before: &std::collections::HashMap<String, (i32, i32, Option<String>)>,
    form: &Form,
) -> Vec<MoveAnim> {
    let mut anims = Vec::new();
    for c in &form.controls {
        let Some((ox, oy, oparent)) = before.get(&c.id) else {
            continue; // newly created → no move animation
        };
        if *oparent != c.parent {
            continue; // reparented → just apply (Q3)
        }
        if *ox == c.rect.x && *oy == c.rect.y {
            continue; // unmoved
        }
        anims.push(MoveAnim {
            id: c.id.clone(),
            from: egui::pos2(*ox as f32, *oy as f32),
            to: egui::pos2(c.rect.x as f32, c.rect.y as f32),
        });
    }
    anims
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

    /// BackgroundColor edits held back while the "this breaks the style unit —
    /// continue?" confirmation is up (styled forms only). Newest value per
    /// control; applied on confirm, dropped on cancel.
    pub style_break_pending: Vec<(String, String, PropValue)>,
    /// Set once the developer confirmed breaking the style unit on this form —
    /// the question is asked once per designer session, not per colour tick.
    pub style_break_ack: bool,
    /// An undo/redo held for confirmation because the step changes COBOL
    /// procedure code (operator, 2026-07-29). Resolved by
    /// [`Self::confirm_pending_history`].
    pub pending_history_confirm: Option<HistoryDir>,
    /// MenuBar ids whose YAML changed via undo/redo — the per-frame cache
    /// loader force-reloads these (execute/reverse have no egui context).
    menu_cache_dirty: Vec<String>,

    /// 037 R2 — MainForm flag transitions (true = claimed, false = un-claimed
    /// by undo) awaiting the app's cross-file settlement. Drained every frame
    /// by `App::drain_main_form_changes`.
    pub main_form_changes: Vec<bool>,

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

    // ── Collapsible designer chrome (spec 033) ────────────────────────────────
    /// When true the left sidebar shrinks to a narrow icon rail (toolbox only,
    /// no labels / no forms list). Toggled by the toolbox chevron; the rail uses
    /// a FIXED width so it never self-inflates.
    pub toolbox_collapsed: bool,
    /// The user's last expanded left-panel width. Seeds the panel's `default_size`
    /// and is refreshed only from the panel's own resized rect (user drag), never
    /// from available space — so re-expanding restores exactly what the user set.
    pub toolbox_width: f32,
    /// When true the right properties pane is slid away, leaving only a thin
    /// reopen tab (fixed width — no self-inflation).
    pub props_hidden: bool,

    // ── UI options ────────────────────────────────────────────────────────────
    pub show_grid: bool,
    pub glass_mode: bool,

    // ── 049 — the rail state the CANVAS is showing ────────────────────────────
    /// Which state the sidebar is drawn in on the canvas. Design time ONLY,
    /// and never persisted.
    ///
    /// `Collapsed` on the control was doing two unrelated jobs: the state the
    /// finished application OPENS in, and the state currently being SHOWN.
    /// They are separate facts, so the shown one lives here. Clicking the
    /// breadcrumb's toggle while designing flips this and nothing else — it is
    /// visual confirmation of a control the operator will use, never an edit to
    /// the developer's design.
    ///
    /// `None` means "show whatever `Collapsed` says", so the property still
    /// drives the canvas until the developer takes the view over by clicking.
    pub(crate) rail_view_collapsed: Option<bool>,
    /// The designed `Collapsed` as of last frame. When it changes — the
    /// developer edited the property in the inspector — the override above is
    /// dropped, so the property takes the view back.
    rail_designed_collapsed: Option<bool>,
    /// Where the breadcrumb's toggle landed last frame, so the hover wash and
    /// the click test use the very rect that was drawn.
    pub(crate) crumb_toggle_rect: Option<egui::Rect>,

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
    /// The theme the form's controls are painted by this frame (spec 047/050).
    /// Resolved by the app alongside `active_theme_pack` and published to the
    /// canvas + preview contexts each frame.
    pub active_surface_theme: std::sync::Arc<dyn cobolt_forms::surface_theme::SurfaceTheme>,

    /// The font the user most recently set on a control in this form. New controls
    /// inherit it so a form keeps a consistent typeface.
    last_font_name: Option<String>,
    last_font_size: Option<i64>,

    /// In-flight agent control-move animations and their shared start time
    /// (spec 035). Purely visual — the model already holds the final positions.
    move_anims: Vec<MoveAnim>,
    move_anim_start: Option<f64>,
    /// The `ctx.input().time` of the last canvas paint, so a retargeting move
    /// (R6) can compute each control's current on-screen position at apply time.
    move_anim_last_now: Option<f64>,

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
    /// The AI prompt box in the event modal also uses the COBOL editor engine so
    /// prompts that mention controls can complete `Control::Property` names.
    ai_prompt_editor: super::editor::EditorPanel,
    /// The same hosted COBOL editor for the COBOL Structure popup (spec 005), so
    /// section / procedure code gets IntelliSense too. `cs_loaded` is the block
    /// currently in its buffer (reloaded only when the selection changes).
    cs_editor: super::editor::EditorPanel,
    cs_loaded: Option<super::cobol_structure::CsTarget>,
    /// Last frame's COBOL Structure editor-box rect. Its WIDTH bounds the
    /// window's other rows (`set_max_width`), so width-filling widgets (the
    /// status row's right-aligner, separators, the AI bar) follow the BOX —
    /// the user-dragged width authority — and never `available_width`, which
    /// is window memory and ratchets. Written only from the `egui::Resize`
    /// box (= grip drag / default seed), never from measured content.
    cs_box_rect: Option<egui::Rect>,

    // ── Global AI Assistant (Designer bottom pane) ────────────────────────────
    pub ai_pane_open: bool,
    pub ai_history: Vec<crate::llm::ChatTurn>,
    pub ai_status: Option<String>,
    /// When set, a user-resizable modal window shows this AI error message
    /// (Copy / Save… / font size / OK). Replaces the old inline red label.
    pub ai_error_modal: Option<String>,
    pub ai_last_seen_turns: usize,
    pub ai_rx: Option<std::sync::mpsc::Receiver<crate::llm::LlmResponse>>,
    pub ai_pane_height: f32,
    pub ai_history_font_size: f32,
    pub ai_error_font_size: f32,
    pub global_ai_prompt: String,
    /// Cleanups the designer performed on its own, waiting to be reported in
    /// the Output panel. Automatic removal that leaves no trace is how a
    /// developer loses work without knowing it; the app drains this every
    /// frame and prints one line per item.
    pub orphan_notices: Vec<String>,
    /// Prompt-box height, user-authoritative: 0 = "never dragged" (renders at
    /// the 3-row default); only the box's corner-grip drag writes it, clamped
    /// between the 1-row and 6-row limits.
    pub ai_prompt_height: f32,
    pub global_ai_streaming: String,
    /// What the AI pane's transcript looked like last frame — turn count,
    /// streamed-buffer length, and whether the agents were working. Any change
    /// is new material at the bottom (a sent prompt, a returned turn, a Grace
    /// or specialist progress line, the "Thinking…" indicator appearing), and
    /// that is exactly when the view follows it down.
    ai_transcript_mark: (usize, usize, bool),
    /// Whether the transcript was parked at its bottom last frame. Reading back
    /// through earlier turns is the same intent as holding the mouse button —
    /// while the view is away from the bottom, new material does not drag it
    /// back; returning to the bottom resumes the follow.
    ai_transcript_at_bottom: bool,
    /// Name completion for the prompt box (controls, data items, properties,
    /// events) — never COBOL itself.
    prompt_ac: PromptAc,
    /// Grace's review of the request, in flight: the developer pressed send,
    /// and the workflow does not start until they have read the rewrite.
    review_rx: Option<std::sync::mpsc::Receiver<crate::llm::LlmResponse>>,
    /// The request as the developer wrote it, kept while the review runs.
    review_original: Option<String>,
    /// The open modal, if any.
    review_modal: Option<crate::panels::prompt_review::PromptReview>,
    /// Set by the modal's Submit: the text the workflow actually receives.
    review_accepted: Option<String>,

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
            style_break_pending: Vec::new(),
            style_break_ack: false,
            pending_history_confirm: None,
            menu_cache_dirty: Vec::new(),
            main_form_changes: Vec::new(),
            dirty: false,
            close_requested: false,
            close_confirm: false,
            pending_delete: None,
            create_user_control: None,
            toolbox: ToolboxPanel::new(),
            properties: PropertiesPanel::new(),
            toolbox_collapsed: false,
            toolbox_width: TOOLBOX_DEFAULT_W,
            props_hidden: false,
            show_grid: true,
            glass_mode: true,
            rail_view_collapsed: None,
            rail_designed_collapsed: None,
            crumb_toggle_rect: None,
            anim_states: HashMap::new(),
            last_frame_time: None,
            format_painter: FormatPainter::Idle,
            image_cache: HashMap::new(),
            last_font_name: None,
            last_font_size: None,
            move_anims: Vec::new(),
            move_anim_start: None,
            move_anim_last_now: None,
            press_handle: None,
            press_form_edge: None,
            menu_modal: None,
            event_modal: None,
            event_editor: super::editor::EditorPanel::new(),
            ai_prompt_editor: super::editor::EditorPanel::new(),
            cs_editor: super::editor::EditorPanel::new(),
            cs_loaded: None,
            cs_box_rect: None,
            ai_pane_open: false,
            ai_history: Vec::new(),
            ai_status: None,
            ai_error_modal: None,
            ai_last_seen_turns: 0,
            ai_rx: None,
            // Sized so the history above the fixed 170px input starts at ~70px.
            ai_pane_height: 254.0,
            ai_history_font_size: 14.0,
            ai_error_font_size: 13.0,
            global_ai_prompt: String::new(),
            orphan_notices: Vec::new(),
            ai_prompt_height: 0.0, // 0 = never dragged → 3-row default
            global_ai_streaming: String::new(),
            ai_transcript_mark: (0, 0, false),
            ai_transcript_at_bottom: true,
            prompt_ac: PromptAc::default(),
            review_rx: None,
            review_original: None,
            review_modal: None,
            review_accepted: None,
            show_preview: false,
            cobol_structure_edit: None,
            preview_state: HashMap::new(),
            preview_anim_states: HashMap::new(),
            preview_last_frame: None,
            preview_combo_open: HashMap::new(),
            active_theme_pack: None,
            active_surface_theme: cobolt_forms::surface_theme::liquid_glass(),
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
    /// Drive the prompt box's name completion: accept what the developer
    /// picked, otherwise recompute the list from the caret and draw it.
    ///
    /// The list is names only — control ids, data items, procedures, and the
    /// properties/events of the form's own control types. COBOL itself is never
    /// completed (see [`crate::prompt_complete`]): a prompt is prose, and a
    /// popup over "display the total" would be in the way, not in the help.
    fn prompt_completion(
        &mut self,
        ctx: &egui::Context,
        te_id: egui::Id,
        out: &egui::text_edit::TextEditOutput,
        box_rect: egui::Rect,
        input: PromptAcInput,
    ) {
        if input.accept {
            self.prompt_accept(ctx, te_id, out, input.sel);
            return;
        }
        if input.dismiss || !input.editable || !ctx.memory(|m| m.has_focus(te_id)) {
            self.prompt_ac.close();
            return;
        }
        self.prompt_ac.sel = input.sel;

        // Recompute from the caret. The catalogue is built only while the box
        // has focus — it walks the form and parses working-storage, which is
        // not work to do on every frame of a pane nobody is typing into.
        let Some(cursor) = out.state.cursor.char_range() else {
            self.prompt_ac.close();
            return;
        };
        let caret = char_to_byte(&self.global_ai_prompt, cursor.primary.index.0);
        let catalog = crate::prompt_complete::Catalog::from_form(
            &self.form,
            crate::panels::editor::build_prompt_data_items(&self.form, ""),
        );
        match crate::prompt_complete::complete(&self.global_ai_prompt, caret, &catalog) {
            Some(c) => {
                if c.items != self.prompt_ac.items {
                    self.prompt_ac.sel = 0; // a new list starts at the top
                }
                self.prompt_ac.replace = (c.replace.start, c.replace.end);
                self.prompt_ac.items = c.items;
                self.prompt_ac.visible = true;
            }
            None => self.prompt_ac.close(),
        }

        if let Some(clicked) = self.prompt_popup(ctx, box_rect) {
            self.prompt_accept(ctx, te_id, out, clicked);
        }
    }

    /// Put item `index` into the prompt, replacing the word the caret is on,
    /// and leave the caret after it so typing simply continues.
    fn prompt_accept(
        &mut self,
        ctx: &egui::Context,
        te_id: egui::Id,
        out: &egui::text_edit::TextEditOutput,
        index: usize,
    ) {
        let (start, end) = self.prompt_ac.replace;
        if let Some(item) = self.prompt_ac.items.get(index) {
            if start <= end
                && end <= self.global_ai_prompt.len()
                && self.global_ai_prompt.is_char_boundary(start)
                && self.global_ai_prompt.is_char_boundary(end)
            {
                let label = item.label.clone();
                self.global_ai_prompt.replace_range(start..end, &label);
                let caret = self.global_ai_prompt[..start + label.len()].chars().count();
                let mut state = out.state.clone();
                state.cursor.set_char_range(Some(egui::text::CCursorRange::one(
                    egui::text::CCursor::new(egui::text::CharIndex(caret)),
                )));
                state.store(ctx, te_id);
            }
        }
        self.prompt_ac.close();
    }

    /// Draw the completion list under the prompt box. Returns the index the
    /// developer clicked, if any.
    fn prompt_popup(&self, ctx: &egui::Context, box_rect: egui::Rect) -> Option<usize> {
        if !self.prompt_ac.visible || self.prompt_ac.items.is_empty() {
            return None;
        }
        let mut clicked = None;
        let screen = ctx.content_rect();
        let width = box_rect.width().min(420.0).max(220.0);
        let pos = egui::pos2(
            box_rect.left().min(screen.right() - width - 8.0),
            box_rect.bottom() + 4.0,
        );
        egui::Area::new(egui::Id::new("global_ai_prompt_ac"))
            .order(egui::Order::Foreground)
            .fixed_pos(pos)
            .show(ctx, |ui| {
                egui::Frame::popup(ui.style()).show(ui, |ui| {
                    ui.set_width(width);
                    ui.spacing_mut().item_spacing.y = 1.0;
                    for (i, item) in self.prompt_ac.items.iter().enumerate() {
                        let selected = i == self.prompt_ac.sel;
                        let label = egui::RichText::new(&item.label)
                            .monospace()
                            .color(prompt_ac_color(item.kind));
                        let resp = ui.selectable_label(selected, label);
                        if resp.clicked() {
                            clicked = Some(i);
                        }
                        // The kind (and the type a member came from) rides on
                        // the same row, right-aligned and quiet.
                        let hint = match &item.owner {
                            Some(t) if item.kind != crate::prompt_complete::Kind::Control => {
                                format!("{} · {t}", item.kind.detail())
                            }
                            _ => item.kind.detail().to_string(),
                        };
                        ui.painter().text(
                            egui::pos2(resp.rect.right() - 4.0, resp.rect.center().y),
                            egui::Align2::RIGHT_CENTER,
                            hint,
                            egui::FontId::proportional(10.0),
                            egui::Color32::from_gray(140),
                        );
                    }
                    ui.label(
                        egui::RichText::new("↑↓ escolher · Tab/Enter aceitar · Esc fechar")
                            .small()
                            .color(egui::Color32::from_gray(120)),
                    );
                });
            });
        clicked
    }

    /// Where a `deploy_control` whose id is already taken must land: on the
    /// control this change-set is still staging, or on the one already on the
    /// form. `None` when the id is free — or when it names a control of a
    /// DIFFERENT type, which is a genuine collision and keeps the old
    /// behaviour of minting a fresh auto id.
    fn redeploy_target(
        &self,
        id: &str,
        ct: &ControlType,
        deployed: &std::collections::HashMap<String, usize>,
        cmds: &[Cmd],
    ) -> Option<RedeployTarget> {
        if let Some(&i) = deployed.get(&id.to_ascii_uppercase()) {
            return match cmds.get(i) {
                Some(Cmd::AddControl { ctrl, .. }) if ctrl.control_type == *ct => {
                    Some(RedeployTarget::Pending(i))
                }
                _ => None,
            };
        }
        let c = self.form.find_control(id)?;
        (c.control_type == *ct).then(|| RedeployTarget::OnForm(c.id.clone()))
    }

    pub fn apply_agent_change_set(&mut self, cs: &crate::agent::AgentChangeSet) -> usize {
        use crate::agent::AgentOp;
        let status = crate::agent::validate(cs, &self.form);
        // Agent-placed geometry goes on the grid the same way a dragged control
        // does, and a coordinate the change-set repeats stays one coordinate —
        // so a column the agent aligned is still aligned once snapped. Geometry
        // does not affect validation, so `status` is still row-for-row.
        let normalized =
            crate::agent::normalize_geometry(cs, self.form.grid_size as i32, self.form.snap_to_grid);
        let cs = &normalized;
        let mut cmds: Vec<Cmd> = Vec::new();
        let mut reserved: HashSet<String> =
            self.form.controls.iter().map(|c| c.id.clone()).collect();
        let mut added = 0usize;
        // Controls this change-set deploys, by upper-cased id → index in `cmds`.
        let mut deployed: std::collections::HashMap<String, usize> =
            std::collections::HashMap::new();

        for (op, err) in cs.operations.iter().zip(status.iter()) {
            if err.is_some() {
                continue; // R9 — invalid ops are shown in the preview, never applied
            }
            match op {
                AgentOp::DeployControl {
                    control_type,
                    id,
                    parent_id,
                    parent,
                    properties,
                } => {
                    let ct = ControlType::from_str(control_type);
                    // A deploy that names a control ALREADY on the form is a
                    // redeploy, not a second control. Agents re-emit their whole
                    // change-set as a matter of course — a correction round, or a
                    // second task over the same form — and every id they repeat
                    // used to be minted as a clone under an auto id, silently
                    // doubling the form. Fold the properties into the control
                    // that already carries the id instead.
                    if let Some(cid) = id
                        .as_deref()
                        .map(str::trim)
                        .filter(|s| !s.is_empty())
                        .and_then(|s| self.redeploy_target(s, &ct, &deployed, &cmds))
                    {
                        match cid {
                            RedeployTarget::Pending(i) => {
                                if let Some(Cmd::AddControl { ctrl, .. }) = cmds.get_mut(i) {
                                    apply_deploy_properties(ctrl, properties);
                                }
                            }
                            RedeployTarget::OnForm(cid) => {
                                for (k, v) in properties {
                                    let Some(pv) = json_to_prop(v) else { continue };
                                    let old = self
                                        .form
                                        .find_control(&cid)
                                        .and_then(|c| structural_prop_value(c, k));
                                    cmds.push(Cmd::SetProperty {
                                        id: cid.clone(),
                                        key: k.clone(),
                                        old,
                                        new: pv,
                                    });
                                }
                            }
                        }
                        continue;
                    }
                    let cid = id
                        .clone()
                        .filter(|s| !s.trim().is_empty() && !reserved.contains(s))
                        .unwrap_or_else(|| self.next_unique_id_reserved(&ct, &reserved));
                    reserved.insert(cid.clone());
                    // Geometry: honour X/Y/Width/Height when given, else stagger a
                    // sensible default so the developer can rearrange it (R13).
                    // Present coordinates were already snapped (and lane-aligned)
                    // by `normalize_geometry`; the fallback stagger is a
                    // placement too, so it lands on the grid as well.
                    let gp = self.form.grid_size as i32;
                    let sn = self.form.snap_to_grid;
                    let on_grid = |v: i32| if sn { crate::agent::snap_nearest(v, gp) } else { v };
                    let gx = json_prop_i32(properties, "X").unwrap_or_else(|| on_grid(20));
                    let gy = json_prop_i32(properties, "Y").unwrap_or_else(|| {
                        on_grid(20 + 28 * (self.form.controls.len() + added) as i32)
                    });
                    let mut c = Control::new(cid.clone(), ct.clone(), gx, gy);
                    if let Some(style) = self.neumorphic_seed() {
                        c.apply_glass_style_defaults(style);
                    }
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
                    if c.parent.is_none() {
                        let explicit_parent = parent.as_ref().or(parent_id.as_ref());
                        if let Some(pid) = explicit_parent {
                            apply_agent_parent_target(&self.form, &mut c, pid);
                        }
                    } else if let Some(existing_parent) = c.parent.clone() {
                        apply_agent_parent_target(&self.form, &mut c, &existing_parent);
                    }
                    cmds.push(Cmd::AddControl {
                        index: self.form.controls.len() + added,
                        ctrl: c,
                    });
                    deployed.insert(cid.to_ascii_uppercase(), cmds.len() - 1);
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
                    let old = if self.is_form_id(control_id) {
                        self.get_form_prop(key).map(PropValue::String)
                    } else {
                        self.form
                            .find_control(control_id)
                            .and_then(|c| structural_prop_value(c, key))
                    };
                    cmds.push(Cmd::SetProperty {
                        id: control_id.clone(),
                        key: key.clone(),
                        old,
                        new: pv,
                    });
                }
                AgentOp::SetFormStructure { block, code } => {
                    let old = crate::agent::form_structure_field(&mut self.form, block)
                        .map(|s| s.clone())
                        .unwrap_or_default();
                    let mut new = crate::llm::normalize_comments(code);
                    // A form-level `01` without GLOBAL is private to the form,
                    // so no contained handler can name it — and nothing the
                    // handler agent writes afterwards can repair that, because
                    // declaring the item locally makes a second, unrelated
                    // copy. Apply the clause as the change-set lands, not as
                    // advice: it is added here rather than in `execute` so undo
                    // and redo replay the exact stored text.
                    if matches!(
                        crate::agent::form_structure_block(block),
                        Some("WORKING-STORAGE") | Some("FILE SECTION")
                    ) {
                        new = crate::panels::cobol_structure::ensure_global_on_01_levels(&new);
                    }
                    cmds.push(Cmd::SetFormStructure {
                        block: block.clone(),
                        old,
                        new,
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
                        // Normalise `*` comments to `*>` deterministically.
                        new: crate::llm::normalize_comments(code),
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
                        new: crate::llm::normalize_comments(code),
                    });
                }
                AgentOp::Message { .. } => {
                    // Handled during chat history building.
                }
            }
        }

        let n = cmds.len();
        if n > 0 {
            // Containers move as a WHOLE and moved controls avoid overlaps: fold
            // the carry + nudge moves into the SAME batch so undo and the move
            // animation treat each container-and-children motion as one.
            cmds.extend(self.plan_container_and_overlap_moves(cs, &status));

            // Snapshot positions BEFORE applying so we can animate the moves
            // (spec 035, R1). If an animation is already running, use each
            // control's CURRENT on-screen position as the "before" so a new
            // change-set retargets smoothly (R6).
            let mut before: std::collections::HashMap<String, (i32, i32, Option<String>)> =
                std::collections::HashMap::new();
            for c in &self.form.controls {
                let (bx, by) = self
                    .live_move_from(&c.id)
                    .unwrap_or((c.rect.x, c.rect.y));
                before.insert(c.id.clone(), (bx, by, c.parent.clone()));
            }

            self.apply(Cmd::AgentBatch { cmds });

            let anims = diff_moves(&before, &self.form);
            if !anims.is_empty() {
                self.move_anims = anims;
                self.move_anim_start = None; // armed — first paint stamps the start
            }
        }
        n
    }

    /// Plan the extra position moves that follow an agent change-set:
    /// 1. **Container carry** — every control keeps its place inside a container
    ///    the change-set repositions (a container and its children move as one).
    /// 2. **Overlap avoidance** — a control the change-set moves is nudged off
    ///    any same-level control it would land on; only the moved control (with
    ///    its subtree) shifts, obstacles stay put.
    ///
    /// Returned as `MoveControl`s that go from each control's *staged* position
    /// (after the change-set's own X/Y) to its *final* position, so folding them
    /// into the same batch yields one undo and one animation. Manual drag already
    /// carries children (`handle_drag`); this brings the agent path in line and
    /// adds the overlap nudge on top.
    fn plan_container_and_overlap_moves(
        &self,
        cs: &crate::agent::AgentChangeSet,
        status: &[Option<String>],
    ) -> Vec<Cmd> {
        use crate::agent::AgentOp;
        use std::collections::{HashMap, HashSet};
        let to_i32 = |v: &serde_json::Value| -> Option<i32> {
            v.as_i64()
                .map(|n| n as i32)
                .or_else(|| v.as_str().and_then(|s| s.trim().parse().ok()))
        };
        // Explicit geometry the change-set sets, per control.
        let mut explicit: HashSet<String> = HashSet::new();
        let mut tgt_x: HashMap<String, i32> = HashMap::new();
        let mut tgt_y: HashMap<String, i32> = HashMap::new();
        let mut tgt_w: HashMap<String, i32> = HashMap::new();
        let mut tgt_h: HashMap<String, i32> = HashMap::new();
        for (op, err) in cs.operations.iter().zip(status.iter()) {
            if err.is_some() {
                continue;
            }
            if let AgentOp::SetProperty {
                control_id,
                key,
                value,
            } = op
            {
                let Some(v) = to_i32(value) else { continue };
                match key.as_str() {
                    "X" => {
                        tgt_x.insert(control_id.clone(), v);
                        explicit.insert(control_id.clone());
                    }
                    "Y" => {
                        tgt_y.insert(control_id.clone(), v);
                        explicit.insert(control_id.clone());
                    }
                    "Width" => {
                        tgt_w.insert(control_id.clone(), v);
                    }
                    "Height" => {
                        tgt_h.insert(control_id.clone(), v);
                    }
                    _ => {}
                }
            }
        }
        // Staged rect = original rect with the change-set's own X/Y/W/H applied.
        let staged = |c: &Control| -> (i32, i32, i32, i32) {
            (
                tgt_x.get(&c.id).copied().unwrap_or(c.rect.x),
                tgt_y.get(&c.id).copied().unwrap_or(c.rect.y),
                tgt_w.get(&c.id).copied().unwrap_or(c.rect.w),
                tgt_h.get(&c.id).copied().unwrap_or(c.rect.h),
            )
        };
        // Delta of every container the change-set actually moves.
        let mut cont_delta: HashMap<String, (i32, i32)> = HashMap::new();
        for c in &self.form.controls {
            if !c.is_container() {
                continue;
            }
            let d = (
                tgt_x.get(&c.id).copied().unwrap_or(c.rect.x) - c.rect.x,
                tgt_y.get(&c.id).copied().unwrap_or(c.rect.y) - c.rect.y,
            );
            if d != (0, 0) {
                cont_delta.insert(c.id.clone(), d);
            }
        }
        // Carry delta = delta of the nearest moved-container ancestor, unless the
        // control is explicitly placed by the change-set itself.
        let carry_of = |c: &Control| -> (i32, i32) {
            if explicit.contains(&c.id) {
                return (0, 0);
            }
            let mut cur = c.parent.clone();
            while let Some(pid) = cur {
                if let Some(d) = cont_delta.get(&pid) {
                    return *d;
                }
                cur = self.form.find_control(&pid).and_then(|p| p.parent.clone());
            }
            (0, 0)
        };

        // Positions after staging + carry (before overlap nudging).
        let mut pos: HashMap<String, (i32, i32, i32, i32)> = HashMap::new();
        for c in &self.form.controls {
            let (sx, sy, sw, sh) = staged(c);
            let (cx, cy) = carry_of(c);
            pos.insert(c.id.clone(), (sx + cx, sy + cy, sw, sh));
        }

        // Overlap avoidance: nudge each *moved root* (a control moved by its own
        // explicit target or as a top-level move, not one merely carried) off any
        // same-parent control it overlaps. The root drags its subtree along.
        //
        // A deliberate agent layout (two or more explicitly placed controls) is
        // trusted as-is: the Form Designer computes non-overlapping coordinates,
        // and the nudge would otherwise shove the one control whose slot happens
        // to sit over a leftover, untouched control out of an otherwise clean grid
        // ("all but one aligned"). A lone placed control still gets the drag-like
        // nudge so it slides off a sibling it lands on.
        let form_w = self.form.width as i32;
        let form_h = self.form.height as i32;
        let nudge_overlaps = explicit.len() < 2;
        for (i, c) in self.form.controls.iter().enumerate() {
            if !nudge_overlaps {
                break;
            }
            let moved = {
                let p = pos[&c.id];
                (p.0, p.1) != (c.rect.x, c.rect.y)
            };
            if !moved {
                continue;
            }
            if !explicit.contains(&c.id) && carry_of(c) != (0, 0) {
                continue; // moves as part of its container's group
            }
            // Subtree ids (root + descendants) — they move rigidly together.
            let mut subtree: HashSet<String> = HashSet::new();
            subtree.insert(c.id.clone());
            for d in super::containers::collect_descendants(&self.form.controls, i) {
                subtree.insert(self.form.controls[d].id.clone());
            }
            // Obstacles = same-parent controls outside this subtree, at their
            // planned positions.
            let obstacles: Vec<(i32, i32, i32, i32)> = self
                .form
                .controls
                .iter()
                .filter(|o| o.parent == c.parent && !subtree.contains(&o.id))
                .map(|o| pos[&o.id])
                .collect();
            let rroot = pos[&c.id];
            // Keep top-level roots inside the form; nested roots are unbounded.
            let bounds = c.parent.is_none().then_some((form_w, form_h));
            let (nx, ny) =
                nearest_free_offset(rroot, &obstacles, bounds, form_w.max(form_h));
            if (nx, ny) == (0, 0) {
                continue;
            }
            for id in subtree {
                if let Some(p) = pos.get_mut(&id) {
                    p.0 += nx;
                    p.1 += ny;
                }
            }
        }

        // One MoveControl per control whose final pos differs from its staged pos
        // (carry and/or nudge); the change-set itself already produced staged.
        let mut moves = Vec::new();
        for c in &self.form.controls {
            let (sx, sy, _, _) = staged(c);
            let (fx, fy, _, _) = pos[&c.id];
            if (fx, fy) != (sx, sy) {
                moves.push(Cmd::MoveControl {
                    id: c.id.clone(),
                    old_x: sx,
                    old_y: sy,
                    new_x: fx,
                    new_y: fy,
                });
            }
        }
        moves
    }

    /// The current on-screen position of a control mid-animation, as integer
    /// design coordinates — the eased interpolation from `from` to `to` at the
    /// last painted time. `None` when it is not animating. Retargets a fresh
    /// move so it continues from where the control visually is (R6).
    fn live_move_from(&self, id: &str) -> Option<(i32, i32)> {
        let anim = self.move_anims.iter().find(|a| a.id == id)?;
        let (start, now) = (self.move_anim_start?, self.move_anim_last_now?);
        let t = ((now - start) / MOVE_ANIM_SECS).clamp(0.0, 1.0) as f32;
        let cur = anim.from + (anim.to - anim.from) * eased(t);
        Some((cur.x.round() as i32, cur.y.round() as i32))
    }

    /// Advance the agent control-move animation for this paint (spec 035): stamp
    /// the start on the first frame, drive repaints while running, finish at
    /// `MOVE_ANIM_SECS`, and return each animating control's current **draw
    /// offset**. Empty when idle. The offset feeds paint positions ONLY — never
    /// any layout/container size (egui self-inflation guard, plan §5).
    fn tick_move_anims(&mut self, now: f64, ctx: &egui::Context) -> HashMap<String, Vec2> {
        self.move_anim_last_now = Some(now);
        if self.move_anims.is_empty() {
            return HashMap::new();
        }
        let start = *self.move_anim_start.get_or_insert(now);
        let t = ((now - start) / MOVE_ANIM_SECS).clamp(0.0, 1.0) as f32;
        if t >= 1.0 {
            self.move_anims.clear();
            self.move_anim_start = None;
            return HashMap::new();
        }
        ctx.request_repaint(); // keep ticking until the motion completes (R4)
        self.move_anims
            .iter()
            .map(|a| (a.id.clone(), move_offset(a.from, a.to, t)))
            .collect()
    }

    pub fn undo(&mut self) {
        // A step that changes COBOL procedure code waits for the developer's
        // explicit confirmation (operator, 2026-07-29); further Ctrl+Z presses
        // while the question is up do nothing.
        if self.pending_history_confirm.is_some() {
            return;
        }
        if self
            .undo_stack
            .last()
            .map(touches_procedures)
            .unwrap_or(false)
        {
            self.pending_history_confirm = Some(HistoryDir::Undo);
            return;
        }
        self.undo_unchecked();
    }

    fn undo_unchecked(&mut self) {
        if let Some(cmd) = self.undo_stack.pop() {
            self.reverse(&cmd);
            self.redo_stack.push(cmd);
            self.dirty = true;
        }
    }

    /// Resolve the pending procedure-history confirmation: `accept` performs
    /// the held undo/redo, `false` drops the request (nothing moves).
    pub fn confirm_pending_history(&mut self, accept: bool) {
        match self.pending_history_confirm.take() {
            Some(HistoryDir::Undo) if accept => self.undo_unchecked(),
            Some(HistoryDir::Redo) if accept => self.redo_unchecked(),
            _ => {}
        }
    }

    pub fn redo(&mut self) {
        if self.pending_history_confirm.is_some() {
            return;
        }
        if self
            .redo_stack
            .last()
            .map(touches_procedures)
            .unwrap_or(false)
        {
            self.pending_history_confirm = Some(HistoryDir::Redo);
            return;
        }
        self.redo_unchecked();
    }

    fn redo_unchecked(&mut self) {
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
                if self.is_form_id(id) {
                    let val_str = match new {
                        PropValue::String(s) => s.clone(),
                        PropValue::Int(n) => n.to_string(),
                        PropValue::Bool(b) => b.to_string(),
                    };
                    // The direct setter: this IS a command being executed —
                    // going through the undoable wrapper would push a second
                    // command mid-execute and corrupt the stacks.
                    self.set_form_prop_direct(key, val_str);
                } else if let Some(c) = self.form.find_control_mut(id) {
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
            Cmd::SetFormProp { key, new, .. } => {
                self.set_form_prop_direct(key, new.clone());
            }
            Cmd::SetFormStructure { block, new, .. } => {
                if let Some(slot) = crate::agent::form_structure_field(&mut self.form, block) {
                    *slot = new.clone();
                }
            }
            Cmd::SetGlassStyle { style, .. } => {
                self.set_form_prop_direct("GlassStyle", style.clone());
            }
            Cmd::SetAnimations { id, new, .. } => {
                if let Some(c) = self.form.find_control_mut(id) {
                    c.animations = new.clone();
                }
            }
            Cmd::AddProcedure { index, proc } => {
                let idx = (*index).min(self.form.user_procedures.len());
                self.form.user_procedures.insert(idx, proc.clone());
            }
            Cmd::RemoveProcedure { index, .. } => {
                if *index < self.form.user_procedures.len() {
                    self.form.user_procedures.remove(*index);
                }
            }
            Cmd::ApplyDataBinding { binding, .. } => {
                crate::app::apply_data_binding_to_form(&mut self.form, binding.clone());
                crate::app::seed_control_array_binding_preview_values(self, binding);
            }
            Cmd::SetMenuDefinition {
                control_id, new, ..
            } => {
                self.write_menu_yaml(control_id, Some(new));
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
                if self.is_form_id(id) {
                    if let Some(v) = old {
                        let val_str = match v {
                            PropValue::String(s) => s.clone(),
                            PropValue::Int(n) => n.to_string(),
                            PropValue::Bool(b) => b.to_string(),
                        };
                        self.set_form_prop_direct(key, val_str);
                    }
                } else if let Some(c) = self.form.find_control_mut(id) {
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
            Cmd::SetAnimations { id, old, .. } => {
                if let Some(c) = self.form.find_control_mut(id) {
                    c.animations = old.clone();
                }
            }
            Cmd::AddProcedure { index, .. } => {
                if *index < self.form.user_procedures.len() {
                    self.form.user_procedures.remove(*index);
                }
            }
            Cmd::RemoveProcedure { index, proc } => {
                let idx = (*index).min(self.form.user_procedures.len());
                self.form.user_procedures.insert(idx, proc.clone());
            }
            Cmd::ApplyDataBinding {
                before_bindings,
                before_controls,
                ..
            } => {
                self.form.data_bindings = before_bindings.clone();
                self.form.controls = before_controls.clone();
            }
            Cmd::SetMenuDefinition {
                control_id, old, ..
            } => {
                self.write_menu_yaml(control_id, old.as_ref());
            }
            Cmd::SetFormProp { key, old, .. } => {
                self.set_form_prop_direct(key, old.clone());
            }
            Cmd::SetFormStructure { block, old, .. } => {
                if let Some(slot) = crate::agent::form_structure_field(&mut self.form, block) {
                    *slot = old.clone();
                }
            }
            Cmd::SetGlassStyle { before, .. } => {
                self.form.glass_style = before.glass_style;
                self.form.background_color = before.background_color.clone();
                self.form.background_gradient_enabled = before.background_gradient_enabled;
                self.form.background_gradient_start_color =
                    before.background_gradient_start_color.clone();
                self.form.background_gradient_end_color =
                    before.background_gradient_end_color.clone();
                self.form.background_gradient_direction =
                    before.background_gradient_direction.clone();
                self.form.controls = before.controls.clone();
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
            let origin = egui::pos2(c.rect.x as f32, c.rect.y as f32);
            let p = egui::pos2(cx as f32, cy as f32);
            for (i, tr) in cobolt_forms::paint::tabcontrol_tab_rects(origin, c)
                .iter()
                .enumerate()
            {
                if tr.contains(p) {
                    return Some((c.id.clone(), i as u32));
                }
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
        if let Some(style) = self.neumorphic_seed() {
            ctrl.apply_glass_style_defaults(style);
        }
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
        // A control deletion can leave a form-level procedure addressing nothing
        // that exists — and it can break the form outright.
        //
        // It is **reported, never removed** (operator, 2026-08-05: "treat user
        // code as sacred"). Deleting a control takes that control's own handler
        // code with it, which is the control's; a common procedure is separate
        // code that merely mentions it, and no condition — orphaned, uncalled,
        // unparseable — earns the right to delete what the developer wrote.
        // Telling them costs a line in the Output panel; guessing costs them the
        // work.
        for index in self.form.orphaned_user_procedures() {
            if let Some(proc) = self.form.user_procedures.get(index) {
                self.orphan_notices.push(format!(
                    "Procedure {} now references only controls that no longer exist, \
                     and nothing calls it. It was KEPT — the form will not run until \
                     you fix or delete it.",
                    proc.name
                ));
            }
        }
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
        // A control arriving from another form adopts this form's style, the
        // same way a control dropped from the toolbox does — otherwise pasting
        // from a Classic form leaves a Classic-looking control sitting on a
        // neumorphic surface. A same-form paste is a plain duplicate and keeps
        // whatever the developer customised on the original.
        if !same_form {
            if let Some(style) = self.neumorphic_seed() {
                for ctrl in &mut pasted {
                    ctrl.apply_glass_style_defaults(style);
                }
            }
        }
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
        self.event_editor.known_data_items = super::editor::build_known_data_items(&self.form);
        self.ai_prompt_editor.open_buffer(
            std::path::PathBuf::from(format!("{program_id}.ai-prompt")),
            String::new(),
        );
        self.ai_prompt_editor.set_context_only_completions(true);
        self.ai_prompt_editor.known_controls = super::editor::build_known_controls(&self.form);
        self.ai_prompt_editor.known_data_items =
            super::editor::build_prompt_data_items(&self.form, &source);

        self.event_modal = Some(EventEditorModal::new(
            ctrl_id, display, event_name, program_id, source,
        ));
    }

    /// The action the 🔧 Auto-fix button used to run in the syntax-error modal:
    /// reformat/normalise the handler (uppercases reserved words, fixes column
    /// alignment, collapses spacing), re-validate, and — if that cleared every
    /// error — save and close.
    ///
    /// The button was removed from the modal; the same reformat is still one
    /// click away as ✨ Beautify in the editor status row, and Save re-validates.
    /// Kept here rather than deleted so re-wiring it is a one-line change.
    #[allow(dead_code)]
    fn autofix_event_handler(
        &mut self,
        ctx: &egui::Context,
        program_id: &str,
        ctrl_id: &str,
        event_name: &str,
        orig_source: &str,
    ) {
        // Deterministic fix: reformat/normalise (uppercases reserved words,
        // fixes column alignment, collapses spacing), then re-validate.
        self.event_editor.beautify_active();
        let fixed = self.event_editor.buffer_for_save().unwrap_or_default();
        let mut errs2 = validate_handler_syntax(program_id, &fixed);
        errs2.extend(validate_handler_members(&self.form, &fixed));
        errs2.extend(validate_handler_semantics(
            &self.form,
            ctrl_id,
            event_name,
            program_id,
            &fixed,
        ));
        if let Some(m) = self.event_modal.as_mut() {
            if errs2.is_empty() {
                // Clean now — save and close.
                if fixed != orig_source {
                    m.syntax_errors = None;
                }
            }
            m.syntax_errors = if errs2.is_empty() { None } else { Some(errs2) };
        }
        if self
            .event_modal
            .as_ref()
            .map(|m| m.syntax_errors.is_none())
            .unwrap_or(false)
        {
            if fixed != orig_source {
                self.save_event_handler(ctrl_id, event_name, fixed);
            }
            self.event_modal = None;
        }
        ctx.request_repaint();
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
        // Add / remove / field edits all become one undoable SetAnimations
        // command carrying the full before/after list (audit, 2026-07-29 —
        // these returned before the stack and were invisible to undo).
        if key == "_AddAnimation" {
            if let Some(old) = self.form.find_control(ctrl_id).map(|c| c.animations.clone()) {
                let mut new = old.clone();
                new.retain(|a| a.name != value.as_str());
                new.push(AnimationDef::new(value.as_str()));
                self.apply(Cmd::SetAnimations {
                    id: ctrl_id.to_owned(),
                    old,
                    new,
                });
            }
            return;
        }
        if let Some(idx_str) = key.strip_prefix("_RemoveAnim") {
            if let Ok(idx) = idx_str.parse::<usize>() {
                if let Some(old) = self.form.find_control(ctrl_id).map(|c| c.animations.clone()) {
                    if idx < old.len() {
                        let mut new = old.clone();
                        new.remove(idx);
                        self.apply(Cmd::SetAnimations {
                            id: ctrl_id.to_owned(),
                            old,
                            new,
                        });
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
                    if let Some(old) =
                        self.form.find_control(ctrl_id).map(|c| c.animations.clone())
                    {
                        if idx < old.len() {
                            let mut new = old.clone();
                            let anim = &mut new[idx];
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
                            self.apply(Cmd::SetAnimations {
                                id: ctrl_id.to_owned(),
                                old,
                                new,
                            });
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
            // Struct-backed flags, routed through the undo stack like any other
            // property (they mutated directly before — audit, 2026-07-28).
            // `apply_structural_prop` maps them onto the struct fields on
            // execute; `structural_prop_value` captures the old value.
            "Visible" | "Enabled" | "TabOrder" => {
                if let Some(old) = self
                    .form
                    .find_control(ctrl_id)
                    .and_then(|c| structural_prop_value(c, key))
                {
                    if old != value {
                        self.apply(Cmd::SetProperty {
                            id: ctrl_id.to_owned(),
                            key: key.to_owned(),
                            old: Some(old),
                            new: value,
                        });
                    }
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

    /// Set one form-level property by name, as an **undoable** command.
    ///
    /// A plain property becomes a [`Cmd::SetFormProp`] carrying the previous
    /// value; a GlassStyle switch becomes a [`Cmd::SetGlassStyle`] carrying a
    /// full pre-switch snapshot (the Neumorphic appliers rewrite appearance
    /// defaults on every control, so the style enum alone cannot restore it).
    /// Unknown keys are ignored, exactly like the direct setter.
    pub fn set_form_prop(&mut self, key: &str, value: String) {
        let Some(canonical) = canonical_form_prop_key(key) else {
            return;
        };
        if canonical == "GlassStyle" {
            let style = cobolt_forms::model::GlassStyle::from_str(&value);
            if style == self.form.glass_style {
                return;
            }
            let before = Box::new(StyleSnapshot {
                glass_style: self.form.glass_style,
                background_color: self.form.background_color.clone(),
                background_gradient_enabled: self.form.background_gradient_enabled,
                background_gradient_start_color: self
                    .form
                    .background_gradient_start_color
                    .clone(),
                background_gradient_end_color: self.form.background_gradient_end_color.clone(),
                background_gradient_direction: self.form.background_gradient_direction.clone(),
                controls: self.form.controls.clone(),
            });
            self.apply(Cmd::SetGlassStyle {
                before,
                style: value,
            });
            return;
        }
        let Some(old) = self.get_form_prop(canonical) else {
            return;
        };
        if old == value {
            return;
        }
        self.apply(Cmd::SetFormProp {
            key: canonical.to_string(),
            old,
            new: value,
        });
    }

    /// Add an empty user procedure (COBOL Structure panel) as an undoable
    /// command. Returns the new procedure's index.
    pub fn add_user_procedure(&mut self) -> usize {
        let index = self.form.user_procedures.len();
        let proc = cobolt_forms::model::UserProcedure {
            name: format!("USER-PROC-{}", index + 1),
            code: String::new(),
        };
        self.apply(Cmd::AddProcedure { index, proc });
        index
    }

    /// Delete a user procedure (COBOL Structure panel) as an undoable command
    /// — the full procedure rides the stack so undo restores its code.
    pub fn remove_user_procedure(&mut self, index: usize) {
        if let Some(proc) = self.form.user_procedures.get(index).cloned() {
            self.apply(Cmd::RemoveProcedure { index, proc });
        }
    }

    // NOTE: there is deliberately no `prune_orphaned_procedures` (operator,
    // 2026-08-05). Nothing in this program removes a common procedure except
    // the developer, through the delete button, after confirming.
    // [`cobolt_forms::Form::orphaned_user_procedures`] is now purely a query:
    // it says which procedures address nothing that exists, so the designer can
    // *report* them.

    /// Apply a data binding from the binding editor as an undoable command.
    /// Binding application rewrites target-control properties (columns,
    /// sources, preview values), so the pre-apply bindings and controls are
    /// snapshotted for undo.
    pub fn apply_data_binding(&mut self, binding: cobolt_forms::DataBindingDef) {
        let before_bindings = self.form.data_bindings.clone();
        let before_controls = self.form.controls.clone();
        self.apply(Cmd::ApplyDataBinding {
            binding,
            before_bindings,
            before_controls,
        });
    }

    /// Save a MenuBar's menu definition as an undoable command. The previous
    /// definition (or its absence) is captured from the YAML so undo restores
    /// — or removes — the file.
    pub fn set_menu_definition(
        &mut self,
        control_id: String,
        def: cobolt_forms::menu::MenuDefinition,
    ) {
        let Some(dir) = &self.cfrm_dir else {
            eprintln!("Menu save skipped: the form has no directory yet");
            return;
        };
        let path = cobolt_forms::menu::menu_yaml_path(dir, &control_id);
        let old = if path.exists() {
            cobolt_forms::menu::load_menu(&path).ok()
        } else {
            None
        };
        self.apply(Cmd::SetMenuDefinition {
            control_id,
            old,
            new: def,
        });
    }

    /// Write (or, with `None`, remove) a MenuBar's YAML and queue the paint
    /// cache for a forced reload on the next frame — the [`Cmd`] execution
    /// primitive; undo/redo have no egui context of their own.
    fn write_menu_yaml(
        &mut self,
        control_id: &str,
        def: Option<&cobolt_forms::menu::MenuDefinition>,
    ) {
        let Some(dir) = &self.cfrm_dir else {
            return;
        };
        let path = cobolt_forms::menu::menu_yaml_path(dir, control_id);
        match def {
            Some(d) => {
                if let Err(e) = cobolt_forms::menu::save_menu(&path, d) {
                    eprintln!("Failed to save menu: {e}");
                    return;
                }
            }
            None => {
                let _ = std::fs::remove_file(&path);
            }
        }
        self.menu_cache_dirty.push(control_id.to_owned());
    }

    /// Apply one form-level property directly, with **no** undo record — the
    /// execution primitive [`Cmd::SetFormProp`] / [`Cmd::SetGlassStyle`] and
    /// the undo machinery drive. Case-insensitive like the public setter.
    fn set_form_prop_direct(&mut self, key: &str, value: String) {
        match canonical_form_prop_key(key).unwrap_or(key) {
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
                let style = cobolt_forms::model::GlassStyle::from_str(&value);
                // 050 R7/R8 — under a self-contained theme the glass style is
                // inert, so it must not rewrite the developer's form either.
                // `apply_glass_style_defaults` overwrites background colours,
                // gradient flags and per-control shadow properties; doing that
                // for a setting that changes nothing on screen means switching
                // back to Liquid Glass no longer reproduces the form they had.
                // Store the choice, touch nothing else.
                if self.active_surface_theme.is_self_contained() {
                    self.form.glass_style = style;
                } else {
                    self.form.apply_glass_style_defaults(style);
                }
                self.dirty = true;
            }
            "BackgroundGradientEnabled" => {
                self.form.background_gradient_enabled = value == "true" || value == "1";
                self.dirty = true;
            }
            "BackgroundGradientStartColor" => {
                self.form.background_gradient_start_color = value;
                self.dirty = true;
            }
            "BackgroundGradientEndColor" => {
                self.form.background_gradient_end_color = value;
                self.dirty = true;
            }
            "BackgroundGradientDirection" => {
                self.form.background_gradient_direction = value;
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

            // ── 037 Main form & window lifecycle ─────────────────────────────
            "MainForm" => {
                let want = value == "true" || value == "1";
                if self.form.main_form != want {
                    self.form.main_form = want;
                    self.dirty = true;
                    // The exactly-one invariant spans FILES; the app drains
                    // these transitions and settles the other form(s) — a
                    // claim clears the previous holder, an un-claim (undo)
                    // restores it. Emitted only on a real flag change so a
                    // no-op undo (this form already demoted by a later claim
                    // elsewhere) stays silent.
                    self.main_form_changes.push(want);
                }
            }
            "TaskbarIcon" => {
                self.form.taskbar_icon = value;
                self.dirty = true;
            }
            "CanMinimize" => {
                self.form.can_minimize = value != "false" && value != "0";
                self.dirty = true;
            }
            "CanMaximize" => {
                self.form.can_maximize = value != "false" && value != "0";
                self.dirty = true;
            }
            "WindowState" => {
                self.form.window_state = cobolt_forms::model::WindowState::from_str(&value);
                self.dirty = true;
            }
            "FullScreen" => {
                self.form.full_screen = value == "true" || value == "1";
                self.dirty = true;
            }
            "TitleVisible" => {
                self.form.title_visible = value != "false" && value != "0";
                self.dirty = true;
            }
            "WindowEffects" => {
                self.form.window_effects = value != "false" && value != "0";
                self.dirty = true;
            }

            // ── 049 Application shell ─────────────────────────────────────
            "FormFormat" => {
                // R5: the main form owns the window; its format is pinned.
                if !self.form.main_form {
                    self.form.form_format = cobolt_forms::model::FormFormat::from_str(&value);
                    self.dirty = true;
                }
            }
            "MenuPaneCustom" => {
                // The R39 group: checking materialises defaults, unchecking
                // returns the pane to the shell's default chrome fill.
                if value == "true" || value == "1" {
                    self.form
                        .menu_pane_background
                        .get_or_insert_with(Default::default);
                } else {
                    self.form.menu_pane_background = None;
                }
                self.dirty = true;
            }
            "MenuPaneColor" => {
                let mp = self.form.menu_pane_background.get_or_insert_with(Default::default);
                mp.color = value;
                self.dirty = true;
            }
            "MenuPaneGradientEnabled" => {
                let mp = self.form.menu_pane_background.get_or_insert_with(Default::default);
                mp.gradient_enabled = value == "true" || value == "1";
                self.dirty = true;
            }
            "MenuPaneGradientStartColor" => {
                let mp = self.form.menu_pane_background.get_or_insert_with(Default::default);
                mp.gradient_start_color = value;
                self.dirty = true;
            }
            "MenuPaneGradientEndColor" => {
                let mp = self.form.menu_pane_background.get_or_insert_with(Default::default);
                mp.gradient_end_color = value;
                self.dirty = true;
            }
            "MenuPaneGradientDirection" => {
                let mp = self.form.menu_pane_background.get_or_insert_with(Default::default);
                mp.gradient_direction = value;
                self.dirty = true;
            }
            "MenuPaneTransparency" => {
                if let Ok(v) = value.parse::<u8>() {
                    let mp =
                        self.form.menu_pane_background.get_or_insert_with(Default::default);
                    mp.transparency = v.min(100);
                    self.dirty = true;
                }
            }
            "MenuPaneImage" => {
                let mp = self.form.menu_pane_background.get_or_insert_with(Default::default);
                mp.image = value;
                self.dirty = true;
            }
            "MenuPaneImageMode" => {
                let mp = self.form.menu_pane_background.get_or_insert_with(Default::default);
                mp.image_mode = cobolt_forms::model::BgImageMode::from_str(&value);
                self.dirty = true;
            }

            // ── Window start position ────────────────────────────────────
            "X" => {
                if let Ok(v) = value.parse::<i32>() {
                    self.form.x = v;
                    self.dirty = true;
                }
            }
            "Y" => {
                if let Ok(v) = value.parse::<i32>() {
                    self.form.y = v;
                    self.dirty = true;
                }
            }
            "StartPosition" => {
                self.form.start_position =
                    cobolt_forms::model::FormStartPosition::from_str(&value);
                self.dirty = true;
            }

            _ => {}
        }
    }

    pub fn is_form_id(&self, id: &str) -> bool {
        id.is_empty() || id.eq_ignore_ascii_case("Form") || id.eq_ignore_ascii_case(&self.form.name)
    }

    /// Read one form-level property by name, case-insensitively (see
    /// [`Self::set_form_prop`]).
    pub fn get_form_prop(&self, key: &str) -> Option<String> {
        match canonical_form_prop_key(key)? {
            "Title" => Some(self.form.title.clone()),
            "BackgroundColor" => Some(self.form.background_color.clone()),
            "Width" => Some(self.form.width.to_string()),
            "Height" => Some(self.form.height.to_string()),
            "Transparency" => Some(self.form.transparency.to_string()),
            "GridSize" => Some(self.form.grid_size.to_string()),
            "SnapToGrid" => Some(if self.form.snap_to_grid { "true".to_string() } else { "false".to_string() }),
            "GlassStyle" => Some(self.form.glass_style.as_str().to_string()),
            "BackgroundGradientEnabled" => Some(if self.form.background_gradient_enabled { "true".to_string() } else { "false".to_string() }),
            "BackgroundGradientStartColor" => Some(self.form.background_gradient_start_color.clone()),
            "BackgroundGradientEndColor" => Some(self.form.background_gradient_end_color.clone()),
            "BackgroundGradientDirection" => Some(self.form.background_gradient_direction.clone()),
            "Target" => Some(self.form.target.clone()),
            "BackgroundImage" => Some(self.form.background_image.clone()),
            "BgImageMode" => Some(self.form.bg_image_mode.as_str().to_string()),
            "Theme" => Some(self.form.theme.clone().unwrap_or_default()),
            "UseThemeBackground" => Some(if self.form.use_theme_background { "true".to_string() } else { "false".to_string() }),
            "MainForm" => Some(bool_str(self.form.main_form)),
            "TaskbarIcon" => Some(self.form.taskbar_icon.clone()),
            "CanMinimize" => Some(bool_str(self.form.can_minimize)),
            "CanMaximize" => Some(bool_str(self.form.can_maximize)),
            "WindowState" => Some(self.form.window_state.as_str().to_string()),
            "FullScreen" => Some(bool_str(self.form.full_screen)),
            "TitleVisible" => Some(bool_str(self.form.title_visible)),
            "WindowEffects" => Some(bool_str(self.form.window_effects)),
            "FormFormat" => Some(self.form.form_format.as_str().to_string()),
            "MenuPaneCustom" => Some(bool_str(self.form.menu_pane_background.is_some())),
            "MenuPaneColor" => Some(
                self.form
                    .menu_pane_background
                    .as_ref()
                    .map(|m| m.color.clone())
                    .unwrap_or_default(),
            ),
            "MenuPaneGradientEnabled" => Some(bool_str(
                self.form
                    .menu_pane_background
                    .as_ref()
                    .map(|m| m.gradient_enabled)
                    .unwrap_or(false),
            )),
            "MenuPaneGradientStartColor" => Some(
                self.form
                    .menu_pane_background
                    .as_ref()
                    .map(|m| m.gradient_start_color.clone())
                    .unwrap_or_default(),
            ),
            "MenuPaneGradientEndColor" => Some(
                self.form
                    .menu_pane_background
                    .as_ref()
                    .map(|m| m.gradient_end_color.clone())
                    .unwrap_or_default(),
            ),
            "MenuPaneGradientDirection" => Some(
                self.form
                    .menu_pane_background
                    .as_ref()
                    .map(|m| m.gradient_direction.clone())
                    .unwrap_or_default(),
            ),
            "MenuPaneTransparency" => Some(
                self.form
                    .menu_pane_background
                    .as_ref()
                    .map(|m| m.transparency.to_string())
                    .unwrap_or_else(|| "0".into()),
            ),
            "MenuPaneImage" => Some(
                self.form
                    .menu_pane_background
                    .as_ref()
                    .map(|m| m.image.clone())
                    .unwrap_or_default(),
            ),
            "MenuPaneImageMode" => Some(
                self.form
                    .menu_pane_background
                    .as_ref()
                    .map(|m| m.image_mode.as_str().to_string())
                    .unwrap_or_else(|| "Stretch".into()),
            ),
            "X" => Some(self.form.x.to_string()),
            "Y" => Some(self.form.y.to_string()),
            "StartPosition" => Some(self.form.start_position.as_str().to_string()),
            _ => None,
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

    /// Toggle the format-painter state machine.
    pub(crate) fn toggle_format_painter(&mut self) {
        match &self.format_painter {
            FormatPainter::WaitingForTarget { .. } | FormatPainter::WaitingForSource => {
                self.format_painter = FormatPainter::Idle;
            }
            FormatPainter::Idle => {
                if let Some(sid) = self.selected_ids.first().cloned() {
                    if let Some(src) = self.form.find_control(&sid) {
                        // Capture EVERY property that may travel. Which of them
                        // actually land depends on the target: same type ⇒ all
                        // of them, a different type ⇒ the cross-type subset.
                        // Filtering to that subset here would throw away what a
                        // same-type paste needs.
                        let props = src
                            .properties
                            .iter()
                            .filter(|(k, _)| is_copyable_style_prop(k))
                            .map(|(k, v)| (k.clone(), v.clone()))
                            .collect();
                        let animations = src.animations.clone();
                        let src_rect = src.rect.clone();
                        self.format_painter = FormatPainter::WaitingForTarget {
                            props,
                            animations,
                            src_rect,
                            src_type: src.control_type.clone(),
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
        project: Option<&CoboltProject>,
        project_root: Option<&std::path::Path>,
    ) -> DesignerShowResult {
        let mut result = DesignerShowResult::default();
        let mut selection_changed = false;

        // 049 — re-pin every FullHeight SideMenu to the form's height before
        // anything reads a rect this frame. Idempotent, and one call here is
        // what makes the sidebar follow a form resize, a Height edit or a
        // FullHeight toggle without each of those knowing about the others.
        self.form.sync_side_menu_full_height();
        // …and its footer Panel with it: created if absent, re-pinned to the
        // footer band otherwise. Runs AFTER the height sync, because the band
        // is measured from the sidebar's bottom edge.
        self.form.sync_side_menu_footer_panels();
        // An edit to `Collapsed` itself takes the canvas back from a toggle.
        self.sync_rail_view();

        // 007 Form themes — publish the resolved asset-pack theme for this frame
        // so the shared `draw_control` skins controls (canvas + preview). `None`
        // ⇒ procedural Liquid Glass.
        cobolt_forms::paint::set_active_theme(ui.ctx(), self.active_theme_pack.clone());
        cobolt_forms::paint::set_glass_style(ui.ctx(), self.form.glass_style);
        cobolt_forms::paint::set_surface_theme(ui.ctx(), self.active_surface_theme.clone());

        // Load menu YAML files for any MenuBar controls and cache them
        if let Some(dir) = &self.cfrm_dir {
            // Ids whose YAML an undoable menu command just rewrote (or removed)
            // — force-reload those; a removed file becomes the empty menu.
            let dirty: Vec<String> = std::mem::take(&mut self.menu_cache_dirty);
            for id in &dirty {
                let yaml_path = cobolt_forms::menu::menu_yaml_path(dir, id);
                let def = if yaml_path.exists() {
                    cobolt_forms::menu::load_menu(&yaml_path).unwrap_or_default()
                } else {
                    cobolt_forms::menu::MenuDefinition::default()
                };
                cobolt_forms::paint::set_menu_cache(ui.ctx(), id, std::sync::Arc::new(def));
            }
            for ctrl in &self.form.controls {
                // 049 — a SideMenu keeps its structure in the same sidecar as
                // a MenuBar, so it warms the same cache; without this the
                // canvas would report every SideMenu as empty.
                if matches!(
                    ctrl.control_type,
                    ControlType::MenuBar | ControlType::SideMenu
                ) {
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

        let tr = crate::i18n::current_tr(ui.ctx());
        let mut panel = egui::Panel::bottom("global_ai_pane").resizable(self.ai_pane_open);

        if self.ai_pane_open {
            panel = panel
                .default_size(self.ai_pane_height.max(GLOBAL_AI_PANE_MIN_HEIGHT))
                .min_size(GLOBAL_AI_PANE_MIN_HEIGHT);
        }

        let original_style = ui.style().clone();
        let mut ai_pane_style = (*original_style).clone();
        ai_pane_style.visuals.widgets.noninteractive.bg_stroke =
            egui::Stroke::new(0.0, egui::Color32::TRANSPARENT);
        ai_pane_style.visuals.widgets.hovered.fg_stroke =
            egui::Stroke::new(0.0, egui::Color32::TRANSPARENT);
        ai_pane_style.visuals.widgets.active.fg_stroke =
            egui::Stroke::new(0.0, egui::Color32::TRANSPARENT);
        ai_pane_style.interaction.resize_grab_radius_side = 8.0;
        ui.set_style(ai_pane_style);

        let resp = panel.show(ui, |ui| {
                if !self.ai_pane_open {
                    ui.vertical_centered(|ui| {
                        if ui.button("AI Assistant").clicked() {
                            self.ai_pane_open = true;
                        }
                    });
                } else {
                    let mut do_send = false;
                    let mut do_close = false;
                    let mut do_save = false;
                    let mut do_compact = false;
                    let mut do_clear = false;
                    let mut show_error_modal = false;
                    let mut decrease_history_font = false;
                    let mut increase_history_font = false;
                    // `ai_status` carries BOTH progress and failure, and the
                    // footer below renders anything that is not progress as
                    // "AI error". Grace's review status was neither listed nor
                    // an error, so "Grace is reviewing the request…" was shown
                    // to the developer as a failure with a Details button, in
                    // the model indicator's place (operator, 2026-07-31).
                    let busy = self
                        .ai_status
                        .as_deref()
                        .is_some_and(|s| status_is_progress(s, &tr));
                    let history_len = self.ai_history.len();
                    let history_font_size = self.ai_history_font_size;

                    // Keep the history clear of the pane's resize handle, which
                    // egui hit-tests ±resize_grab_radius_side around the top edge
                    // — a bubble inside that band steals the drag as text select.
                    ui.add_space(10.0);
                    // Prompt-box height: the 3-row default, or the height the
                    // user dragged the box's corner grip to — clamped between
                    // 1 and 6 text rows. Derived only from the style's row
                    // height and the stored drag, never from content, so the
                    // box cannot grow by itself.
                    let prompt_row = ui.text_style_height(&egui::TextStyle::Body);
                    let prompt_height_for = |rows: f32| rows * prompt_row + GLOBAL_AI_PROMPT_CHROME;
                    let prompt_min_height = prompt_height_for(1.0);
                    let prompt_max_height = prompt_height_for(6.0);
                    let prompt_default_height = prompt_height_for(3.0);
                    let prompt_height = if self.ai_prompt_height > 0.0 {
                        self.ai_prompt_height
                    } else {
                        prompt_default_height
                    }
                    .clamp(prompt_min_height, prompt_max_height);
                    // The input slab keeps its fixed chrome (buttons, status
                    // rows); only the prompt's share of it varies with the drag.
                    let input_height =
                        GLOBAL_AI_INPUT_HEIGHT + (prompt_height - prompt_default_height);
                    // The history takes everything above the input slab, so the
                    // pane's top-edge resizer keeps resizing the history.
                    let history_h = (ui.available_height()
                        - input_height
                        - ui.spacing().item_spacing.y)
                        .max(0.0);
                    let (history_rect, _) = ui.allocate_exact_size(
                        egui::vec2(ui.available_width(), history_h),
                        egui::Sense::hover(),
                    );
                    if ai_pane_debug() {
                        // Only when something MOVED. This is a layout trace, and
                        // a layout that is not changing has nothing to report —
                        // but the pane lays out every frame, so an unconditional
                        // print emitted the same line sixty times a second and
                        // buried whatever the developer turned it on to see.
                        trace_ai_pane_layout(format!(
                            "[ai-pane] max_rect={:?} h={:.1} hist_rect={:?} turns={}",
                            ui.max_rect(),
                            history_h,
                            history_rect,
                            self.ai_history.len(),
                        ));
                    }
                    // Follow the transcript down whenever something is appended to
                    // it — a turn, a Grace/specialist progress line, the
                    // "Thinking…" indicator — but never while a mouse button is
                    // held: that is the developer scrolling back, dragging the
                    // scrollbar or selecting text, and yanking the view then is
                    // worse than not following at all (operator, 2026-07-31).
                    let mark = (
                        self.ai_history.len(),
                        self.global_ai_streaming.len(),
                        busy,
                    );
                    let grew = mark != self.ai_transcript_mark;
                    self.ai_transcript_mark = mark;
                    let follow_transcript = grew
                        && self.ai_transcript_at_bottom
                        && !ui.input(|i| i.pointer.any_down());
                    // Detached child: paints inside history_rect without reporting
                    // its size back to the panel — overflow here must never grow
                    // the pane (egui stores a resizable panel's height from its
                    // content rect, so any overflow compounds frame over frame).
                    let mut hist_ui = ui.new_child(egui::UiBuilder::new().max_rect(history_rect));
                    {
                        let ui = &mut hist_ui;
                        ui.set_clip_rect(history_rect);
                        let out = egui::ScrollArea::vertical()
                            .id_salt("global_ai_history_scroll")
                            .auto_shrink([false, false])
                            .max_height(history_rect.height())
                            .show(ui, |ui| {
                                ui.set_min_height(history_rect.height());
                                ui.vertical(|ui| {
                                    for (index, turn) in self.ai_history.iter().enumerate() {
                                        crate::panels::editor::chat_bubble_with_response_actions(
                                            ui,
                                            &turn.role,
                                            &turn.content,
                                            history_font_size,
                                            project_root,
                                            egui::Id::new((
                                                "designer_agent_response",
                                                self.cfrm_dir.as_deref(),
                                                index,
                                            )),
                                        );
                                    }

                                    if !self.global_ai_streaming.is_empty() {
                                        crate::panels::editor::chat_bubble_with_font_size(
                                            ui,
                                            "assistant",
                                            &self.global_ai_streaming,
                                            history_font_size,
                                        );
                                    }
                                    // While Grace and the specialists are still
                                    // working, the transcript itself shows the
                                    // reasoning indicator right after the last
                                    // balloon — ALWAYS while busy. (An earlier
                                    // "yield while text is streaming" condition
                                    // read the progress bubble as a streaming
                                    // reply: on the Grace path the buffer holds
                                    // static progress lines for minutes, and
                                    // the indicator vanished exactly when the
                                    // wait was longest.)
                                    if busy {
                                        crate::panels::editor::chat_thinking_indicator(
                                            ui,
                                            tr.ai_thinking,
                                            history_font_size,
                                            Some(crate::llm::token_meter()),
                                        );
                                    }
                                    if follow_transcript {
                                        ui.scroll_to_cursor(Some(egui::Align::BOTTOM));
                                    }
                                });
                            });
                        // Where the view ended up, for the next frame's decision.
                        // A transcript shorter than its viewport is trivially at
                        // the bottom.
                        let slack = 4.0;
                        self.ai_transcript_at_bottom = out.content_size.y
                            <= out.inner_rect.height() + slack
                            || out.state.offset.y + out.inner_rect.height() + slack
                                >= out.content_size.y;
                    }

                    let pane_style = ui.style().clone();
                    let mut input_style = (*pane_style).clone();
                    input_style.visuals.widgets.noninteractive.bg_stroke =
                        egui::Stroke::new(1.0, egui::Color32::WHITE);
                    input_style.visuals.widgets.hovered.fg_stroke =
                        egui::Stroke::new(1.0, egui::Color32::WHITE);
                    input_style.visuals.widgets.active.fg_stroke =
                        egui::Stroke::new(1.0, egui::Color32::WHITE);
                    ui.set_style(input_style);
                    // Pin the input block to the pane bottom and fill the gap above
                    // it: egui persists a resizable panel's height from its CONTENT
                    // rect, so if the content is shorter than the dragged height the
                    // pane snaps back on mouse release.
                    ui.add_space((ui.available_height() - input_height).max(0.0));
                    let (input_rect, _) = ui.allocate_exact_size(
                        egui::vec2(ui.available_width(), input_height),
                        egui::Sense::hover(),
                    );
                    // Detached child (see history above): the busy spinner/status
                    // rows can overflow input_rect; reporting that overflow to the
                    // panel inflates the pane by ~18px every frame while streaming.
                    let mut input_ui = ui.new_child(egui::UiBuilder::new().max_rect(input_rect));
                    {
                        let ui = &mut input_ui;
                        ui.set_clip_rect(input_rect);
                            ui.add_space(4.0);

                            // 1. Prompt Editor
                            let ectx = ui.ctx().clone();
                            let btn_col_w = 96.0;
                            let gap = 8.0;
                            let text_w = (ui.available_width() - btn_col_w - gap).max(140.0);

                            ui.horizontal_top(|ui| {
                                // The box the developer SEES is this frame, and it
                                // is the only thing that carries the border. The
                                // editor inside is frameless: a TextEdit's own
                                // frame is content-sized, so pasting more lines
                                // than the box holds grew that border past the
                                // viewport — the bottom edge disappeared and the
                                // last line read as clipped by the pane (operator
                                // report, 2026-07-31). Only the grip drag resizes.
                                let frame = egui::Frame::NONE
                                    .fill(crate::theme::active().bg_extreme)
                                    .stroke(egui::Stroke::new(
                                        PROMPT_FRAME_STROKE,
                                        crate::theme::active().panel_border(),
                                    ))
                                    .corner_radius(egui::CornerRadius::same(6))
                                    .inner_margin(egui::Margin::same(PROMPT_FRAME_MARGIN as i8));

                                ui.vertical(|ui| {
                                    // Fixed (text_w × prompt_height) box; text
                                    // beyond it scrolls INSIDE. The viewport is
                                    // the frame's content area, and desired_rows
                                    // is what FITS in it, so an idle box shows no
                                    // scrollbar at any dragged size.
                                    let box_size = egui::vec2(text_w, prompt_height);
                                    let box_chrome =
                                        2.0 * (PROMPT_FRAME_MARGIN + PROMPT_FRAME_STROKE);
                                    let view_size = egui::vec2(
                                        (box_size.x - box_chrome).max(1.0),
                                        (box_size.y - box_chrome).max(prompt_row),
                                    );
                                    let prompt_rows =
                                        ((view_size.y / prompt_row).floor().max(1.0)) as usize;
                                    // Enter sends; Shift+Enter inserts a newline.
                                    // Plain Enter is consumed BEFORE the TextEdit
                                    // sees it (only while the box is focused) so no
                                    // newline is inserted; Shift+Enter is left alone.
                                    let te_id = egui::Id::new("global_ai_prompt_edit");
                                    let mut enter_send = false;
                                    // The completion popup owns Enter, Tab, the
                                    // arrows and Escape WHILE IT IS OPEN — the
                                    // keys are consumed before the editor sees
                                    // them, exactly as plain Enter already is, so
                                    // accepting a name never also sends the
                                    // prompt.
                                    let ac_len = self.prompt_ac.items.len();
                                    let ac_open = self.prompt_ac.visible && ac_len > 0;
                                    let mut ac_sel = self.prompt_ac.sel.min(ac_len.saturating_sub(1));
                                    let mut ac_accept = false;
                                    let mut ac_dismiss = false;
                                    let inner = frame.show(ui, |ui| {
                                        ui.set_min_size(view_size);
                                        ui.set_max_size(view_size);
                                        egui::ScrollArea::vertical()
                                            .id_salt("global_ai_prompt_scroll")
                                            .auto_shrink([false, false])
                                            // Both bounds, or the 64px default
                                            // `min_scrolled_height` silently makes
                                            // the box taller than the drag asked
                                            // for at the small end.
                                            .min_scrolled_height(view_size.y)
                                            .max_height(view_size.y)
                                            .show(ui, |ui| {
                                                if ui.memory(|m| m.has_focus(te_id)) {
                                                    ui.input_mut(|i| {
                                                        if ac_open {
                                                            if i.consume_key(
                                                                egui::Modifiers::NONE,
                                                                egui::Key::ArrowDown,
                                                            ) {
                                                                ac_sel = (ac_sel + 1) % ac_len;
                                                            }
                                                            if i.consume_key(
                                                                egui::Modifiers::NONE,
                                                                egui::Key::ArrowUp,
                                                            ) {
                                                                ac_sel =
                                                                    (ac_sel + ac_len - 1) % ac_len;
                                                            }
                                                            if i.consume_key(
                                                                egui::Modifiers::NONE,
                                                                egui::Key::Tab,
                                                            ) || i.consume_key(
                                                                egui::Modifiers::NONE,
                                                                egui::Key::Enter,
                                                            ) {
                                                                ac_accept = true;
                                                            }
                                                            if i.consume_key(
                                                                egui::Modifiers::NONE,
                                                                egui::Key::Escape,
                                                            ) {
                                                                ac_dismiss = true;
                                                            }
                                                        } else {
                                                            // NOT consume_key: it matches
                                                            // "logically" and ignores extra
                                                            // Shift/Alt, so Shift+Enter was
                                                            // being swallowed as plain Enter
                                                            // and submitting instead of
                                                            // inserting a newline. Match the
                                                            // event's modifiers exactly.
                                                            i.events.retain(|event| {
                                                                let plain_enter = matches!(
                                                                    event,
                                                                    egui::Event::Key {
                                                                        key: egui::Key::Enter,
                                                                        pressed: true,
                                                                        modifiers,
                                                                        ..
                                                                    } if modifiers.is_none()
                                                                );
                                                                enter_send |= plain_enter;
                                                                !plain_enter
                                                            });
                                                        }
                                                    });
                                                }
                                                egui::TextEdit::multiline(
                                                    &mut self.global_ai_prompt,
                                                )
                                                .id(te_id)
                                                .frame(egui::Frame::NONE)
                                                .hint_text("How can I help you today?")
                                                .desired_width(f32::INFINITY)
                                                .desired_rows(prompt_rows)
                                                .interactive(!busy)
                                                .show(ui)
                                            })
                                            .inner
                                    });
                                    if enter_send && !busy && !self.global_ai_prompt.trim().is_empty()
                                    {
                                        do_send = true;
                                    }
                                    self.prompt_completion(
                                        &ectx,
                                        te_id,
                                        &inner.inner,
                                        inner.response.rect,
                                        PromptAcInput {
                                            sel: ac_sel,
                                            accept: ac_accept,
                                            dismiss: ac_dismiss,
                                            editable: !busy,
                                        },
                                    );
                                    // The grip belongs to the BORDERED BOX the user
                                    // sees — the frame — which is exactly
                                    // `box_size` and follows the drag continuously.
                                    // Anchoring it to anything content-sized (the
                                    // old TextEdit frame) walked it off the corner
                                    // as the text grew (operator report,
                                    // 2026-07-31).
                                    let box_rect = inner.response.rect;
                                    // Bottom-right resize grip, registered AFTER
                                    // the TextEdit so it wins the hit-test over
                                    // text selection. The clamp pins the grip
                                    // inside the 1-row/6-row limits.
                                    let grip_size = 14.0;
                                    let grip_rect = egui::Rect::from_min_size(
                                        box_rect.max - egui::vec2(grip_size, grip_size),
                                        egui::vec2(grip_size, grip_size),
                                    );
                                    let grip = ui.interact(
                                        grip_rect,
                                        egui::Id::new("global_ai_prompt_grip"),
                                        egui::Sense::drag(),
                                    );
                                    if grip.dragged() {
                                        self.ai_prompt_height = (prompt_height
                                            + grip.drag_delta().y)
                                            .clamp(prompt_min_height, prompt_max_height);
                                    }
                                    if grip.hovered() || grip.dragged() {
                                        ui.ctx()
                                            .set_cursor_icon(egui::CursorIcon::ResizeVertical);
                                    }
                                    let stroke = if grip.hovered() || grip.dragged() {
                                        ui.visuals().widgets.hovered.fg_stroke
                                    } else {
                                        ui.visuals().widgets.inactive.fg_stroke
                                    };
                                    // Inset past the frame's stroke and corner
                                    // radius so the diagonals read as sitting on
                                    // the box's INNER edge, not straddling it.
                                    let corner = box_rect.max - egui::vec2(5.0, 5.0);
                                    for step in 1..=3 {
                                        let offset = 3.0 * step as f32;
                                        ui.painter().line_segment(
                                            [
                                                egui::pos2(corner.x - offset, corner.y),
                                                egui::pos2(corner.x, corner.y - offset),
                                            ],
                                            stroke,
                                        );
                                    }
                                    let submit = ui.input(|i| {
                                        i.key_pressed(egui::Key::Enter)
                                            && (i.modifiers.command || i.modifiers.ctrl)
                                    }) && !self.global_ai_prompt.trim().is_empty();
                                    if submit && !busy {
                                        do_send = true;
                                    }
                                });

                                ui.add_space(gap);
                                ui.vertical(|ui| {
                                    ui.label(egui::RichText::new("✨").size(15.0));
                                    let can_send = !busy && !self.global_ai_prompt.trim().is_empty();
                                    if ui.add_enabled(can_send, egui::Button::new(tr.ai_send)).clicked() {
                                        do_send = true;
                                    }
                                    // The reasoning indicator lives in the
                                    // history (after the last balloon), not
                                    // here — see chat_thinking_indicator.
                                });
                            });
                            // Bottom padding for the prompt editor: without it
                            // the box sat flush against the rows below and its
                            // last text line read as clipped by the pane.
                            ui.add_space(GLOBAL_AI_PROMPT_BOTTOM_PAD);

                            if let Some(err) = &self.ai_status {
                                if !status_is_progress(err, &tr) {
                                    ui.horizontal(|ui| {
                                        ui.label(
                                            egui::RichText::new("AI error")
                                                .small()
                                                .color(egui::Color32::from_rgb(220, 120, 120)),
                                        );
                                        if ui
                                            .small_button("Details")
                                            .on_hover_text("Open the full AI error message")
                                            .clicked()
                                        {
                                            show_error_modal = true;
                                        }
                                    });
                                }
                            }

                            // 2. Controls
                            ui.add_space(4.0);
                            ui.horizontal(|ui| {
                                // Model-in-use + context gauge (operator,
                                // 2026-07-29), same indicator as the Grace chat.
                                let fallback_model = project_root.and_then(|root| {
                                    crate::grace_host::grace_model_display_cached(root, llm_cfg)
                                });
                                crate::panels::editor::chat_model_context_indicator(
                                    ui,
                                    &tr,
                                    fallback_model.as_deref(),
                                );
                                ui.separator();
                                ui.label(egui::RichText::new(format!("💬 {}", history_len)).small().color(egui::Color32::from_gray(150)));
                                if ui.add_enabled(history_len > 0, egui::Button::new(format!("💾 {}", tr.ai_save_history))).clicked() {
                                    do_save = true;
                                }
                                if ui.add_enabled(history_len > 0, egui::Button::new(format!("🗜 {}", tr.ai_compact_history))).clicked() {
                                    do_compact = true;
                                }
                                if ui.add_enabled(history_len > 0, egui::Button::new(format!("🗑 {}", tr.ai_clear_history))).clicked() {
                                    do_clear = true;
                                }
                                ui.separator();
                                if ui
                                    .add(egui::Button::new("−"))
                                    .on_hover_text("Decrease history font size")
                                    .clicked()
                                {
                                    decrease_history_font = true;
                                }
                                ui.label(
                                    egui::RichText::new(format!(
                                        "{} pt",
                                        self.ai_history_font_size.round() as i32
                                    ))
                                    .small()
                                    .color(egui::Color32::from_gray(170)),
                                );
                                if ui
                                    .add(egui::Button::new("+"))
                                    .on_hover_text("Increase history font size")
                                    .clicked()
                                {
                                    increase_history_font = true;
                                }
                                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                    if ui.button("Close AI Assistant").clicked() {
                                        do_close = true;
                                    }
                                });
                            });
                    }
                    ui.set_style(pane_style);

                    if do_close {
                        self.ai_pane_open = false;
                    }
                    if show_error_modal {
                        if let Some(err) = &self.ai_status {
                            self.ai_error_modal = Some(err.clone());
                        }
                    }
                    // Pressing send asks Grace to REVIEW the request first;
                    // the workflow starts only from the modal's Submit, with
                    // the text the developer approved (operator, 2026-07-31).
                    if do_send
                        && self.review_rx.is_none()
                        && self.review_modal.is_none()
                        && !self.global_ai_prompt.trim().is_empty()
                    {
                        let original = self.global_ai_prompt.clone();
                        let mut ctx_for_review = crate::agent::build_context_with_project(
                            &self.form,
                            project,
                            project_root,
                        );
                        if let Some(tree) = crate::agent_inspection::latest_summary() {
                            ctx_for_review.push_str("\n\n");
                            ctx_for_review.push_str(&tree);
                        }
                        // The review is Grace's own step, so it runs on GRACE's
                        // model — not on this panel's profile, which is a
                        // different connection with different credentials.
                        let review_cfg = crate::grace_host::grace_connection(
                            project_root.unwrap_or_else(|| std::path::Path::new("")),
                            llm_cfg,
                        );
                        self.ai_status = Some(tr.review_working.to_string());
                        self.review_original = Some(original.clone());
                        self.review_rx = Some(crate::llm::spawn_prompt_review(
                            review_cfg.as_ref().unwrap_or(llm_cfg),
                            &ctx_for_review,
                            &original,
                        ));
                    }
                    if let Some(prompt) = self.review_accepted.take() {
                        {
                            self.ai_history.push(crate::llm::ChatTurn::user(prompt.clone()));
                            self.global_ai_prompt.clear();
                            self.ai_status = Some("Thinking...".to_string());
                            self.ai_error_modal = None;
                            let (sys_prompt, skills) = match project_root {
                                Some(d) => (
                                    crate::agent::effective_prompt(d),
                                    crate::agent::load_skills(d),
                                ),
                                None => (
                                    crate::agent::effective_prompt(std::path::Path::new("")),
                                    crate::agent::load_skills(std::path::Path::new("")),
                                ),
                            };
                            let mut context = crate::agent::build_context_with_project(
                                &self.form,
                                project,
                                project_root,
                            );
                            // Live-UI eyes (spec 027): append the latest
                            // inspection tree snapshot so the model sees the
                            // rendered IDE, not just the form model — and
                            // queue a fresh snapshot for the next turn.
                            if let Some(tree) = crate::agent_inspection::latest_summary() {
                                context.push_str("\n\n");
                                context.push_str(&tree);
                            }
                            crate::agent_inspection::request_snapshot(ui.ctx());
                            self.ai_rx = Some(match project_root {
                                Some(root) => crate::grace_session::spawn_contextual_request(
                                    root,
                                    llm_cfg,
                                    &self.ai_history,
                                    &prompt,
                                    "RAD Form Designer chatbot",
                                    Some(crate::agents_db::FORM_DESIGNER),
                                    &context,
                                    crate::i18n::current_tr(ui.ctx()),
                                ),
                                None => crate::llm::spawn_agent_request(
                                    llm_cfg,
                                    &sys_prompt,
                                    &skills,
                                    &self.ai_history,
                                    &prompt,
                                    &context,
                                    None,
                                ),
                            });
                        }
                    }
                    if do_clear {
                        self.ai_history.clear();
                        self.ai_status = None;
                        self.ai_error_modal = None;
                        self.global_ai_prompt.clear();
                        self.global_ai_streaming.clear();
                    }
                    if decrease_history_font {
                        self.ai_history_font_size = (self.ai_history_font_size - 1.0).max(10.0);
                    }
                    if increase_history_font {
                        self.ai_history_font_size = (self.ai_history_font_size + 1.0).min(28.0);
                    }
                    if do_compact {
                        self.ai_status = Some("Thinking...".to_string());
                        self.ai_rx = Some(crate::llm::spawn_compaction(llm_cfg, &self.ai_history));
                    }
                    if do_save {
                        if let Some(path) = rfd::FileDialog::new()
                            .set_file_name("global_conversation.json")
                            .save_file()
                        {
                            if let Ok(json) = serde_json::to_string_pretty(&self.ai_history) {
                                let _ = std::fs::write(path, json);
                            }
                        }
                    }
                    // ── Grace's review came back ─────────────────────────
                    if let Some(rx) = self.review_rx.take() {
                        match rx.try_recv() {
                            Ok(crate::llm::LlmResponse::Ok(text)) => {
                                let original =
                                    self.review_original.take().unwrap_or_default();
                                match crate::prompt_polish::parse_review(&text) {
                                    Some(review) => {
                                        self.ai_status = None;
                                        self.review_modal =
                                            Some(crate::panels::prompt_review::PromptReview::new(
                                                original,
                                                review.revised,
                                                review.notes,
                                            ));
                                    }
                                    // A review that did not parse must never
                                    // cost the developer their request: run it
                                    // as written.
                                    None => self.review_accepted = Some(original),
                                }
                            }
                            Ok(crate::llm::LlmResponse::Err(e)) => {
                                crate::llm::push_ai_log(
                                    crate::llm::AiLogKind::Error,
                                    format!("prompt review failed, sending the request as written: {e}"),
                                );
                                self.review_accepted = self.review_original.take();
                            }
                            // Streaming chunks are not interesting here.
                            Ok(_) => self.review_rx = Some(rx),
                            Err(std::sync::mpsc::TryRecvError::Empty) => {
                                ui.ctx().request_repaint();
                                self.review_rx = Some(rx);
                            }
                            Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                                self.review_accepted = self.review_original.take();
                            }
                        }
                    }
                    if let Some(modal) = self.review_modal.as_mut() {
                        match crate::panels::prompt_review::show(ui.ctx(), &tr, modal) {
                            Some(crate::panels::prompt_review::ReviewAction::Submit(text)) => {
                                self.review_modal = None;
                                self.review_accepted = Some(text);
                            }
                            Some(crate::panels::prompt_review::ReviewAction::Cancel) => {
                                self.review_modal = None;
                                self.ai_status = None;
                            }
                            None => {}
                        }
                    }

                    let mut keep_rx = true;
                    let mut ai_reply = None;
                    if let Some(rx) = self.ai_rx.take() {
                        loop {
                            match rx.try_recv() {
                                Ok(crate::llm::LlmResponse::Chunk(text)) => {
                                    self.global_ai_streaming.push_str(&text);
                                    ui.ctx().request_repaint();
                                }
                                Ok(resp) => {
                                    ai_reply = Some(resp);
                                    keep_rx = false;
                                    break;
                                }
                                Err(std::sync::mpsc::TryRecvError::Empty) => {
                                    ui.ctx().request_repaint();
                                    break;
                                }
                                Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                                    ai_reply = Some(crate::llm::LlmResponse::Err(
                                        "The assistant worker stopped unexpectedly.".into(),
                                    ));
                                    keep_rx = false;
                                    break;
                                }
                            }
                        }
                        if keep_rx {
                            self.ai_rx = Some(rx);
                        }
                    }
                    if let Some(resp) = ai_reply {
                        match resp {
                            crate::llm::LlmResponse::Ok(text) => {
                                self.ai_status = None;
                                self.global_ai_streaming.clear();

                                // Try to parse it as operations
                                if let Ok(cs) = crate::agent::parse_change_set(&text) {
                                    let applied = self.apply_agent_change_set(&cs);
                                    // Snapshot the post-change UI so the next
                                    // agent turn can verify its own edits
                                    // rendered as intended (spec 027).
                                    crate::agent_inspection::request_snapshot(ui.ctx());

                                    let mut messages: Vec<String> = Vec::new();
                                    for op in &cs.operations {
                                        if let crate::agent::AgentOp::Message { message } = op {
                                            messages.push(message.clone());
                                        }
                                    }

                                    let mut combined_note = String::new();
                                    if !messages.is_empty() {
                                        combined_note.push_str(&messages.join("\n"));
                                    }
                                    if let Some(n) = cs.note {
                                        if !combined_note.is_empty() {
                                            combined_note.push_str("\n\n");
                                        }
                                        combined_note.push_str(&n);
                                    }

                                    let actionable_ops = cs.operations.iter().filter(|op| !matches!(op, crate::agent::AgentOp::Message { .. })).count();

                                    if actionable_ops == 0 {
                                        if !combined_note.is_empty() {
                                            self.ai_history.push(crate::llm::ChatTurn::assistant(combined_note));
                                        } else {
                                            self.ai_history.push(crate::llm::ChatTurn::assistant("I didn't find any actionable changes to apply based on your request. If I misunderstood, please clarify!".to_string()));
                                        }
                                    } else {
                                        if !combined_note.is_empty() {
                                            self.ai_history.push(crate::llm::ChatTurn::assistant(format!("Applied {} changes.\n\n{}", applied, combined_note)));
                                        } else {
                                            self.ai_history.push(crate::llm::ChatTurn::assistant(format!("Applied {} changes.", applied)));
                                        }
                                    }
                                } else {
                                    // Grace answered in prose. A developer-facing
                                    // clarification gets its OWN red balloon (role
                                    // "question"), exactly like the project Grace
                                    // chat surface — the RAD designer chat used a
                                    // plain assistant balloon, so the same Grace
                                    // looked different here. Surrounding context
                                    // stays a normal assistant balloon.
                                    let (context, questions) =
                                        crate::grace_host::split_developer_questions(&text);
                                    if questions.is_empty() {
                                        self.ai_history.push(crate::llm::ChatTurn::assistant(text));
                                    } else {
                                        if !context.trim().is_empty() {
                                            self.ai_history
                                                .push(crate::llm::ChatTurn::assistant(context));
                                        }
                                        for q in questions {
                                            self.ai_history
                                                .push(crate::llm::ChatTurn::question(q));
                                        }
                                    }
                                }
                            }
                            crate::llm::LlmResponse::Chunk(_) => {}
                            crate::llm::LlmResponse::Err(err) => {
                                let err = err.trim_end_matches('.');
                                let msg = if err.starts_with("Model returned") {
                                    err.to_string()
                                } else {
                                    format!("Model returned {err}.")
                                };
                                self.ai_status = Some(msg.clone());
                                self.ai_error_modal = Some(msg);
                            }
                        }
                    }
                }
            });
        if self.ai_pane_open {
            let panel_rect = resp.response.rect;
            let y = panel_rect.min.y - 1.5;
            let line_rect = egui::Rect::from_min_max(
                egui::pos2(panel_rect.min.x, y),
                egui::pos2(panel_rect.max.x, y + 3.0),
            );
            ui.painter()
                .rect_filled(line_rect, 0.0, egui::Color32::WHITE);
        }
        ui.set_style(original_style);
        self.show_global_ai_error_modal(ui.ctx());

        // The canvas is registered after the AI pane, so egui's hit test lets it
        // win the few pixels around the pane's resize handle — grabbing the pane
        // edge could scroll the canvas or drag a form control. Reserve a thin
        // dead strip above the pane to keep the handle uncontested.
        let canvas_max_h = if self.ai_pane_open {
            (ui.available_height() - 10.0).max(0.0)
        } else {
            ui.available_height()
        };
        egui::ScrollArea::both()
            .id_salt("designer_canvas")
            // Fill the available panel rather than growing to the form size, so
            // the canvas actually scrolls when the form is larger than the view
            // (spec 012 follow-up: restore lost form-content scrolling).
            .auto_shrink([false, false])
            .max_height(canvas_max_h)
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
                let canvas_gradient = if self.form.background_gradient_enabled {
                    let gradient_color = |hex: &str| {
                        let color = parse_color(hex);
                        let alpha = color.a() as f32 * form_alpha_mul;
                        let scale = alpha / 255.0;
                        Color32::from_rgba_premultiplied(
                            (color.r() as f32 * scale) as u8,
                            (color.g() as f32 * scale) as u8,
                            (color.b() as f32 * scale) as u8,
                            alpha as u8,
                        )
                    };
                    Some((
                        gradient_color(&self.form.background_gradient_start_color),
                        gradient_color(&self.form.background_gradient_end_color),
                    ))
                } else {
                    None
                };
                let canvas_rounding = if self.glass_mode {
                    egui::CornerRadius::same(6)
                } else {
                    egui::CornerRadius::ZERO
                };
                if self.glass_mode {
                    painter.rect_filled(resp.rect, canvas_rounding, canvas_bg);
                    // Thin border so the form boundary is always visible
                    painter.rect_stroke(
                        resp.rect,
                        canvas_rounding,
                        egui::Stroke::new(1.0, Color32::from_rgba_unmultiplied(255, 255, 255, 60)),
                        egui::StrokeKind::Middle,
                    );
                } else {
                    painter.rect_filled(resp.rect, 0.0, canvas_bg);
                }
                if let Some((start, end)) = canvas_gradient {
                    painter.add(egui::Shape::mesh(
                        cobolt_forms::paint::background_gradient_mesh(
                            resp.rect,
                            start,
                            end,
                            &self.form.background_gradient_direction,
                            canvas_rounding,
                        ),
                    ));
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

                // Grid — dots derive their color from the composited backdrop
                // so a form background near the classic dot color still shows
                // a visible grid.
                if self.show_grid {
                    let gstep = self.form.grid_size.max(4) as f32;
                    draw_grid(&painter, resp.rect, gstep, self.glass_mode, notch_fill);
                }

                // What a control with a TRANSLUCENT background is translucent
                // against. The SideMenu's rail needs it: its colour is
                // routinely 20 %-opaque, and it must resolve here to the very
                // shade the preview and the running shell resolve it to.
                cobolt_forms::paint::set_form_backdrop(ui.ctx(), canvas_bg);

                // ── 049 — the shell's breadcrumb strip ─────────────────────────
                // A form with a SideMenu opens as an application SHELL, and the
                // shell draws a breadcrumb the developer never sees otherwise.
                // Drawn STATIC (one segment, the form itself) and BEFORE the
                // control faces, so a control placed in that band paints over it
                // and the indicator can never hide the developer's own work.
                //
                // Its toggle is LIVE here: the developer can see the rail in
                // both states without running the application. It flips the
                // canvas's own view (`rail_view_collapsed`) and writes nothing —
                // `Collapsed` is the state the finished application opens in,
                // and is the developer's to set in the inspector.
                let crumb_shown_collapsed = self.rail_shown_collapsed();
                let crumb_pointer = ui.ctx().pointer_interact_pos();
                let crumb_layout = cobolt_forms::breadcrumb::draw_design_strip(
                    &painter,
                    ui.ctx(),
                    &self.form,
                    origin,
                    cobolt_forms::breadcrumb::DesignView {
                        collapsed: crumb_shown_collapsed,
                        toggle_hovered: crumb_pointer
                            .zip(self.crumb_toggle_rect)
                            .map(|(p, t)| t.contains(p))
                            .unwrap_or(false),
                    },
                );
                self.crumb_toggle_rect = crumb_layout.as_ref().map(|l| l.toggle);
                if let Some(toggle) = self.crumb_toggle_rect {
                    let r = ui.interact(
                        toggle,
                        ui.id().with(("designer-crumb-toggle", &self.form.name)),
                        egui::Sense::click(),
                    );
                    if r.hovered() {
                        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                    }
                    if r.clicked() {
                        self.rail_view_collapsed = Some(!crumb_shown_collapsed);
                    }
                }

                // Pointer position in canvas space
                let ptr_canvas: Option<(i32, i32)> = ui.ctx().pointer_interact_pos().map(|p| {
                    let rel = p - origin;
                    (rel.x as i32, rel.y as i32)
                });

                if let Some((cx, cy)) = ptr_canvas {
                    if self.sidebar_seam_at(cx, cy).is_some()
                        || matches!(self.drag, DragState::ResizingSidebarPane { .. })
                    {
                        ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeVertical);
                    }
                }

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

                // Agent control-move animation (spec 035): interpolate the DRAWN
                // positions only. The model keeps its final coordinates; we render
                // a lightweight clone shifted by each control's live offset.
                let anim_now = ui.ctx().input(|i| i.time);
                let move_offsets = self.tick_move_anims(anim_now, ui.ctx());
                let animated_controls: Option<Vec<cobolt_forms::model::Control>> =
                    (!move_offsets.is_empty()).then(|| {
                        self.form
                            .controls
                            .iter()
                            .map(|c| {
                                let mut c = c.clone();
                                if let Some(off) = move_offsets.get(&c.id) {
                                    c.rect.x = (c.rect.x as f32 + off.x).round() as i32;
                                    c.rect.y = (c.rect.y as f32 + off.y).round() as i32;
                                }
                                c
                            })
                            .collect()
                    });
                let controls_for_render: &[cobolt_forms::model::Control] =
                    animated_controls.as_deref().unwrap_or(&self.form.controls);
                // 049 — and the rail in the state the canvas is showing it in,
                // drawn at the width that state actually has.
                let rail_view = self.rail_view_controls(controls_for_render);
                let controls_for_render: &[cobolt_forms::model::Control] =
                    rail_view.as_deref().unwrap_or(controls_for_render);

                let control_rects = {
                    let st = DesignerState { anim: &anim_tf };
                    let input = cobolt_forms::render::RenderInput {
                        controls: controls_for_render,
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
                                canvas_gradient.map(|(start, end)| {
                                    (
                                        resp.rect,
                                        cobolt_forms::paint::composite_premultiplied_over(
                                            start,
                                            ui.visuals().panel_fill,
                                        ),
                                        cobolt_forms::paint::composite_premultiplied_over(
                                            end,
                                            ui.visuals().panel_fill,
                                        ),
                                        self.form.background_gradient_direction.as_str(),
                                    )
                                }),
                                notch_img,
                                img_alpha,
                            );
                            if self.show_grid {
                                draw_grid_in_rounded_notches(
                                    &painter,
                                    resp.rect,
                                    *crect,
                                    egui::CornerRadius::same(crate::cr8(rad)),
                                    self.form.grid_size.max(4) as f32,
                                    self.glass_mode,
                                    notch_fill,
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
                        painter.circle_filled(
                            badge_rect.center(),
                            5.0,
                            Color32::from_rgba_premultiplied(255, 180, 0, 180),
                        );
                        painter.text(
                            badge_rect.center(),
                            egui::Align2::CENTER_CENTER,
                            "▶",
                            egui::FontId::proportional(6.0),
                            Color32::WHITE,
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
                            egui::StrokeKind::Middle,
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

                // 049 — the two draggable seams on a selected sidebar. Drawn
                // HERE, with the selection border and the resize handles: a
                // handle painted before the control faces is painted over by
                // the very control it belongs to, which is why these were
                // invisible on an opaque rail. Design time only — the preview
                // and the running shell have no such affordance, because the
                // pane heights are the developer's alone.
                self.draw_sidebar_seam_grips(&painter, origin);

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
                            egui::StrokeKind::Middle,
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
                        egui::StrokeKind::Middle,
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
                            ui.close();
                        }
                        ui.separator();
                    }
                    if !user_controls.is_empty() {
                        ui.menu_button(tr.uc_delete, |ui| {
                            for def in user_controls {
                                if ui.button(&def.name).clicked() {
                                    result.user_control_delete_requested = Some(def.name.clone());
                                    ui.close();
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
                        ui.close();
                    }
                    if ui
                        .add_enabled(
                            has_selection,
                            egui::Button::new(format!("{}  ⌘C", tr.clipboard_copy)),
                        )
                        .clicked()
                    {
                        self.copy_selected(clipboard);
                        ui.close();
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
                        ui.close();
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
                        ui.close();
                    }
                    ui.separator();
                    if ui.button("🗑 Delete").clicked() {
                        self.delete_selected();
                        ui.close();
                    }
                    ui.separator();
                    if ui.button("⬆ Bring to Front").clicked() {
                        self.bring_to_front();
                        ui.close();
                    }
                    if ui.button("⬇ Send to Back").clicked() {
                        self.send_to_back();
                        ui.close();
                    }
                    if ui.button("+1 Forward").clicked() {
                        self.bring_forward();
                        ui.close();
                    }
                    if ui.button("-1 Backward").clicked() {
                        self.send_backward();
                        ui.close();
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
                                    ui.close();
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
                                ui.close();
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
                            ui.close();
                        }
                    }
                    ui.separator();
                    if ui.button("🏷 Auto-arrange Labels").clicked() {
                        self.auto_arrange_labels();
                        ui.close();
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
        self.show_event_modal(ui, llm_cfg, project_root);

        result.selection_changed |= selection_changed;
        result
    }

    /// True while this designer has a dialog waiting for the user. Read by the
    /// app to un-pin the always-on-top auxiliary OS windows (debugger,
    /// Run-Form Inspector) so they cannot cover it.
    pub fn has_blocking_modal(&self) -> bool {
        self.pending_delete.is_some()
            || self.ai_error_modal.is_some()
            || self.close_confirm
            || self
                .event_modal
                .as_ref()
                .is_some_and(|m| m.ai_confirm_clear || m.syntax_errors.is_some())
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

        let win_id = egui::Id::new("designer_delete_confirm");
        crate::app::raise_modal_layer(ui.ctx(), win_id);
        egui::Window::new(tr.delete_confirm_title)
            .id(win_id)
            .order(egui::Order::Foreground) // above the Event/Menu/Structure windows
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
                    ui.colored_label(ui.visuals().error_fg_color, error.message(tr));
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
                    crate::error_log::record(UserControlNameError::Circular.message(tr));
                    dialog.error = Some(UserControlNameError::Circular);
                    self.create_user_control = Some(dialog);
                    None
                } else {
                    Some(def)
                }
            }
            Err(error) => {
                crate::error_log::record(error.message(tr));
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

        let overlay = ui.ctx().content_rect();
        ui.painter()
            .rect_filled(overlay, 0.0, Color32::from_rgba_premultiplied(0, 0, 0, 140));

        let mut save_clicked = false;
        let mut cancel_clicked = false;

        let screen = ui.ctx().content_rect();
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
            .frame(
                egui::Frame::window(&ui.ctx().global_style()).inner_margin(egui::Margin::same(12)),
            )
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
                    // ── Indent / Outdent: restructure across sections ────
                    // Indent makes the item a child of its previous sibling;
                    // outdent promotes it next to its parent. Together with
                    // Up/Down they move an item anywhere in the tree.
                    if ui.small_button(tr.menu_indent).clicked() && !modal.selected.is_empty() {
                        let idx = *modal.selected.last().unwrap();
                        if idx > 0 {
                            let depth = modal.selected.len() - 1;
                            let list = MenuEditorModal::parent_list_mut(
                                &mut modal.def,
                                &modal.selected,
                            );
                            fn height(it: &cobolt_forms::menu::MenuItem) -> usize {
                                1 + it.items.iter().map(height).max().unwrap_or(0)
                            }
                            // 3 levels max (0-based depth ≤ 2), subtree included.
                            if depth + height(&list[idx]) <= 2 {
                                let item = list.remove(idx);
                                let prev = &mut list[idx - 1];
                                prev.items.push(item);
                                let child_ix = prev.items.len() - 1;
                                *modal.selected.last_mut().unwrap() = idx - 1;
                                modal.selected.push(child_ix);
                            }
                        }
                    }
                    if ui.small_button(tr.menu_outdent).clicked() && modal.selected.len() >= 2 {
                        let idx = *modal.selected.last().unwrap();
                        let parent_path = modal.selected[..modal.selected.len() - 1].to_vec();
                        let item = {
                            let list = MenuEditorModal::parent_list_mut(
                                &mut modal.def,
                                &modal.selected,
                            );
                            list.remove(idx)
                        };
                        let parent_idx = *parent_path.last().unwrap();
                        let glist =
                            MenuEditorModal::parent_list_mut(&mut modal.def, &parent_path);
                        glist.insert(parent_idx + 1, item);
                        modal.selected = parent_path;
                        *modal.selected.last_mut().unwrap() = parent_idx + 1;
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
                                let cur_preserve = item.preserve_previous_form;

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
                                        // An icon is OPTIONAL on every item:
                                        // the ✕ beside its name drops the
                                        // definition, so an item can be
                                        // text-only without hunting through
                                        // the picker for a Clear button.
                                        if !cur_icon.is_empty()
                                            && ui
                                                .small_button("✕")
                                                .on_hover_text(tr.menu_clear_icon)
                                                .clicked()
                                        {
                                            if let Some(it) = MenuEditorModal::item_at_mut(
                                                &mut modal.def.menu,
                                                &modal.selected,
                                            ) {
                                                it.icon = None;
                                            }
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

                                    // Action type — a SideMenu's menu offers
                                    // the two standalone actions too (051
                                    // R16); a MenuBar's keeps the classic
                                    // three.
                                    ui.horizontal(|ui| {
                                        ui.label(tr.menu_lbl_action);
                                        let mut action_sel = cur_action_type.clone();
                                        let label_of = |key: &str| match key {
                                            "open-form" => tr.menu_action_open_form,
                                            "close" => tr.menu_action_close,
                                            "home" => tr.menu_action_home,
                                            "open-standalone-sync" => {
                                                tr.menu_action_open_standalone_sync
                                            }
                                            "open-standalone-async" => {
                                                tr.menu_action_open_standalone_async
                                            }
                                            _ => tr.menu_action_event,
                                        };
                                        let options =
                                            MenuEditorModal::action_type_options(modal.is_side_menu);
                                        egui::ComboBox::from_id_salt("menu_action_type")
                                            .selected_text(label_of(action_sel.as_str()))
                                            .width(140.0)
                                            .show_ui(ui, |ui| {
                                                for key in options {
                                                    if ui
                                                        .selectable_label(
                                                            action_sel == key,
                                                            label_of(key),
                                                        )
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
                                                                "home" => "home".to_string(),
                                                                other => {
                                                                    match MenuEditorModal::action_prefix(other) {
                                                                        Some(prefix) => format!(
                                                                            "{prefix}{}",
                                                                            modal.target_buf
                                                                        ),
                                                                        None => "event".to_string(),
                                                                    }
                                                                }
                                                            });
                                                        }
                                                    }
                                                }
                                            });
                                    });

                                    // Form selector — for every form-loading
                                    // action, FILTERED to the targets its
                                    // action may legally load (051 R25): the
                                    // embedded door lists Embedded/Both, the
                                    // standalone pair Standalone/Both. A form
                                    // whose format cannot be read appears in
                                    // both lists — a guess never hides a form.
                                    if let Some(prefix) =
                                        MenuEditorModal::action_prefix(&cur_action_type)
                                    {
                                        let want_embedded = cur_action_type == "open-form";
                                        ui.horizontal(|ui| {
                                            ui.label(tr.menu_lbl_target);
                                            let forms = Self::forms_under(self.cfrm_dir.as_deref());
                                            let cur_form = modal.target_buf.clone();
                                            egui::ComboBox::from_id_salt("menu_form_select")
                                                .selected_text(if cur_form.is_empty() {
                                                    "(select form)"
                                                } else {
                                                    &cur_form
                                                })
                                                .width(180.0)
                                                .show_ui(ui, |ui| {
                                                    for (label, form_name, embeddable, standaloneable) in
                                                        &forms
                                                    {
                                                        let legal = if want_embedded {
                                                            *embeddable
                                                        } else {
                                                            *standaloneable
                                                        };
                                                        if !legal {
                                                            continue;
                                                        }
                                                        if ui
                                                            .selectable_label(
                                                                cur_form == *form_name,
                                                                label,
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
                                                                    "{prefix}{form_name}"
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

                                    // 049 R24 — meaningful only when this item
                                    // loads a form into the shell's ContentPane.
                                    if cur_action_type == "open-form" {
                                        ui.horizontal(|ui| {
                                            let mut pv = cur_preserve;
                                            if ui
                                                .checkbox(&mut pv, tr.menu_lbl_preserve)
                                                .on_hover_text(tr.tip_menu_preserve)
                                                .changed()
                                            {
                                                if let Some(it) = MenuEditorModal::item_at_mut(
                                                    &mut modal.def.menu,
                                                    &modal.selected,
                                                ) {
                                                    it.preserve_previous_form = pv;
                                                }
                                            }
                                        });
                                    }
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

                let screen = ui.ctx().content_rect();
                let picker_id = egui::Id::new(("icon_picker", modal.icon_picker_gen));
                egui::Window::new("Select Icon")
                    .id(picker_id)
                    .collapsible(false)
                    .resizable(true)
                    .default_size([600.0, 500.0])
                    .default_pos([screen.center().x - 300.0, screen.center().y - 250.0])
                    .frame(
                        egui::Frame::window(&ui.ctx().global_style())
                            .inner_margin(egui::Margin::same(12)),
                    )
                    .show(ui.ctx(), |ui| {
                        // Search field
                        ui.horizontal(|ui| {
                            ui.label("🔍");
                            ui.text_edit_singleline(&mut modal.icon_search);
                        });
                        ui.separator();

                        let search = modal.icon_search.to_ascii_lowercase();
                        // The catalogue lives ONCE, in cobolt-forms — the
                        // picker renders whatever the icon engine ships, so
                        // the two can never drift apart again.
                        let categories: &[(&str, &[&str])] =
                            cobolt_forms::icons::MENU_ICON_CATEGORIES;

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
                // Undoable save: the command writes the YAML and queues the
                // paint-cache refresh (drained at the top of the next frame).
                self.set_menu_definition(modal.ctrl_id, modal.def);
            }
        }
        if cancel_clicked {
            self.menu_modal = None;
        }
    }

    fn show_global_ai_error_modal(&mut self, ctx: &egui::Context) {
        let Some(message) = self.ai_error_modal.clone() else {
            return;
        };

        let mut open = true;
        let mut close = false;
        // Sizing (anti self-inflation) — same pattern as the debugger window:
        // the outer Window is NOT resizable; the inner `egui::Resize` is the
        // single size authority. Its size comes from the 800×450 seed plus the
        // user's grip drag only — never from measured content or the screen —
        // so the modal cannot grow on its own.
        let win_id = egui::Id::new("form_designer_ai_error_modal");
        crate::app::raise_modal_layer(ctx, win_id);
        egui::Window::new("AI Assistant Error")
            .id(win_id)
            .order(egui::Order::Foreground) // above the Event/Menu/Structure windows
            .open(&mut open)
            .resizable(false) // the inner `Resize` grip is the sole size control
            .collapsible(false)
            .show(ctx, |ui| {
                egui::Resize::default()
                    .id_salt("form_designer_ai_error_resize")
                    .resizable([true, true])
                    .min_size(egui::vec2(380.0, 220.0))
                    .max_size(egui::vec2(4000.0, 4000.0))
                    .default_size(egui::vec2(800.0, 450.0)) // seed only
                    .show(ui, |ui| {
                        // `sz` is the Resize box: user/default state, bounded —
                        // NOT "remaining space" of an auto-sizing container.
                        let sz = ui.available_size();
                        ui.allocate_ui(sz, |ui| {
                            // Fill the box exactly so the reported content
                            // min-size equals the box: the Resize can neither
                            // auto-grow nor auto-shrink to measured content.
                            ui.set_min_size(sz);
                            self.ai_error_modal_body(ui, &message, &mut close);
                        });
                    });
            });

        if close || !open {
            self.ai_error_modal = None;
        }
    }

    /// Body of the AI error modal: header, scrollable message, button row.
    /// Laid out inside the fixed Resize box, so all "available" space here is
    /// user-controlled state, not measured content.
    fn ai_error_modal_body(&mut self, ui: &mut egui::Ui, message: &str, close: &mut bool) {
        // Embedded panels partition the fixed Resize box EXACTLY (no estimated
        // heights): egui 0.35's Resize ratchets up to the measured content min
        // every frame, so an overflowing estimate becomes unbounded growth.
        egui::Panel::bottom(ui.id().with("ai_error_footer"))
            .resizable(false)
            .show_separator_line(false)
            .frame(egui::Frame::NONE)
            .show(ui, |ui| {
                ui.add_space(6.0);
                self.ai_error_modal_footer(ui, message, close);
            });
        egui::CentralPanel::default()
            .frame(egui::Frame::NONE)
            .show(ui, |ui| {
                ui.label(
                    egui::RichText::new("The model returned an error or unusable response.")
                        .strong()
                        .color(egui::Color32::from_rgb(240, 160, 130)),
                );
                ui.add_space(8.0);
                egui::ScrollArea::both()
                    .id_salt("form_designer_ai_error_scroll")
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        ui.label(
                            egui::RichText::new(message)
                                .monospace()
                                .size(self.ai_error_font_size)
                                .color(egui::Color32::from_gray(220)),
                        );
                    });
            });
    }

    /// Button row of the AI error modal (hosted in its bottom panel).
    fn ai_error_modal_footer(&mut self, ui: &mut egui::Ui, message: &str, close: &mut bool) {
        ui.horizontal(|ui| {
            if ui.button("OK").clicked() {
                *close = true;
            }
            ui.separator();
            if ui
                .button("Copy")
                .on_hover_text("Copy the full error message to the clipboard")
                .clicked()
            {
                ui.ctx().copy_text(message.to_owned());
            }
            if ui
                .button("Save...")
                .on_hover_text("Save the full error message to a text file")
                .clicked()
            {
                if let Some(path) = rfd::FileDialog::new()
                    .add_filter("Text file", &["txt", "log"])
                    .set_file_name("ai-assistant-error.txt")
                    .save_file()
                {
                    let _ = std::fs::write(path, message);
                }
            }
            ui.separator();
            if ui
                .small_button("A-")
                .on_hover_text("Decrease error font size")
                .clicked()
            {
                self.ai_error_font_size = (self.ai_error_font_size - 1.0).max(8.0);
            }
            ui.label(
                egui::RichText::new(format!("{} px", self.ai_error_font_size.round() as i32))
                    .small(),
            );
            if ui
                .small_button("A+")
                .on_hover_text("Increase error font size")
                .clicked()
            {
                self.ai_error_font_size = (self.ai_error_font_size + 1.0).min(28.0);
            }
        });
    }

    /// Render the event code editor modal (if open).
    ///
    /// A single editable COBOL area holds the whole handler body (`ENVIRONMENT
    /// DIVISION` … `PROCEDURE DIVISION` + statements). The generator-owned
    /// `IDENTIFICATION DIVISION` / `PROGRAM-ID` header and the closing `GOBACK`
    /// / `END PROGRAM` are shown read-only around it.
    fn show_event_modal(
        &mut self,
        ui: &mut Ui,
        llm_cfg: &crate::llm::LlmConfig,
        project_root: Option<&std::path::Path>,
    ) {
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

        // Lazily load this handler's saved conversation the first time it opens.
        if let Some(m) = self.event_modal.as_mut() {
            if !m.ai_loaded {
                m.ai_loaded = true;
                m.ai_history = load_event_history(project_root, &program_id);
            }
        }

        // ── Poll an in-flight AI request for this handler ────────────────────
        // On completion, splice the model's ```cobol block into the hosted
        // editor's buffer (or surface the error) so the developer can review /
        // tweak / save it like any hand-written handler.
        let mut ai_reply: Option<crate::llm::LlmResponse> = None;
        if let Some(m) = self.event_modal.as_mut() {
            let mut keep_pending = true;
            if let Some(rx) = m.ai_pending.take() {
                loop {
                    match rx.try_recv() {
                        Ok(crate::llm::LlmResponse::Chunk(text)) => {
                            m.ai_streaming_reply.push_str(&text);
                            ui.ctx().request_repaint();
                        }
                        Ok(resp) => {
                            ai_reply = Some(resp);
                            keep_pending = false;
                            break;
                        }
                        Err(std::sync::mpsc::TryRecvError::Empty) => {
                            ui.ctx().request_repaint();
                            break;
                        }
                        Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                            ai_reply = Some(crate::llm::LlmResponse::Err(
                                "The assistant worker stopped unexpectedly.".into(),
                            ));
                            keep_pending = false;
                            break;
                        }
                    }
                }
                if keep_pending {
                    m.ai_pending = Some(rx);
                }
            }
        }
        if let Some(resp) = ai_reply {
            match resp {
                crate::llm::LlmResponse::Ok(reply) => {
                    match crate::llm::extract_code(&reply) {
                        Some(code) => {
                            // Always record the assistant's turn (even when its
                            // code is rejected, so the fix round-trip has the
                            // full context).
                            if let Some(m) = self.event_modal.as_mut() {
                                m.ai_history.push(crate::llm::ChatTurn::assistant(&reply));
                                save_event_history(project_root, &program_id, &m.ai_history);
                            }
                            // Validate the returned handler BEFORE it replaces the
                            // developer's code: parse ENVIRONMENT / DATA / PROCEDURE
                            // (IDENTIFICATION is IDE-owned and supplied by the
                            // validator), then check control-member references.
                            let mut errs = validate_handler_syntax(&program_id, &code);
                            errs.extend(validate_handler_members(&self.form, &code));
                            errs.extend(validate_handler_semantics(
                                &self.form,
                                &ctrl_id,
                                &event_name,
                                &program_id,
                                &code,
                            ));
                            if errs.is_empty() {
                                self.event_editor.open_buffer(
                                    std::path::PathBuf::from(format!("{program_id}.handler")),
                                    code,
                                );
                                if let Some(m) = self.event_modal.as_mut() {
                                    m.ai_status = None;
                                    m.ai_fix_attempts = 0;
                                }
                            } else {
                                // Broken — do NOT apply. Loop the parser errors back
                                // to the assistant and ask for a fix (bounded).
                                let error_list = errs
                                    .iter()
                                    .map(|e| format!("- line {}: {}", e.line, e.message))
                                    .collect::<Vec<_>>()
                                    .join("\n");
                                let attempts = self
                                    .event_modal
                                    .as_ref()
                                    .map(|m| m.ai_fix_attempts)
                                    .unwrap_or(0);
                                if attempts < MAX_AI_FIX_ATTEMPTS {
                                    let skills = project_root
                                        .map(crate::agent::load_event_handler_skills)
                                        .unwrap_or_default();
                                    let mut event_llm_cfg = llm_cfg.clone();
                                    if let Some(root) = project_root {
                                        event_llm_cfg.system_prompt =
                                            crate::agent::effective_event_handler_prompt(root);
                                    }
                                    let fix_prompt = format!(
                                        "The COBOL you just returned is INVALID and was not \
                                         applied. The parser reported:\n{error_list}\n\nReturn a \
                                         corrected event-handler body that fixes every error. Return \
                                         the COMPLETE nested-program body, not a fragment: it MUST \
                                         include ENVIRONMENT DIVISION., DATA DIVISION., and PROCEDURE \
                                         DIVISION. Preserve every WORKING-STORAGE or LINKAGE \
                                         declaration that the procedure still references. Use only \
                                         real control properties from the supplied skills/context \
                                         (for example drop shadow => ShadowEnabled). Do NOT emit \
                                         IDENTIFICATION DIVISION, PROGRAM-ID, GOBACK, or END PROGRAM. \
                                         Return the code in a ```cobol fenced block. If you cannot \
                                         determine the right property or declarations, ask the \
                                         developer for directions with no code block."
                                    );
                                    let prior = self
                                        .event_modal
                                        .as_ref()
                                        .map(|m| m.ai_history.clone())
                                        .unwrap_or_default();
                                    let rx = match project_root {
                                        Some(root) => {
                                            crate::grace_session::spawn_contextual_request(
                                                root,
                                                &event_llm_cfg,
                                                &prior,
                                                &fix_prompt,
                                                "RAD event-handler chatbot validation",
                                                Some(crate::agents_db::EVENT_HANDLER),
                                                &format!(
                                                    "Handler `{program_id}` for `{ctrl_id}.{event_name}` failed validation.\n\nCURRENT INVALID HANDLER:\n```cobol\n{code}\n```\n\nVALIDATION ERRORS:\n{error_list}"
                                                ),
                                                crate::i18n::current_tr(ui.ctx()),
                                            )
                                        }
                                        None => crate::llm::spawn_request(
                                            &event_llm_cfg,
                                            &prior,
                                            &fix_prompt,
                                            &code,
                                            &format!("{program_id}.cob"),
                                            &skills,
                                            Some("CodeGenerator".to_string()),
                                        ),
                                    };
                                    if let Some(m) = self.event_modal.as_mut() {
                                        m.ai_history.push(crate::llm::ChatTurn::user(&fix_prompt));
                                        m.ai_fix_attempts = attempts + 1;
                                        m.ai_pending = Some(rx);
                                        m.ai_status = Some(tr.ai_fixing.to_string());
                                        save_event_history(
                                            project_root,
                                            &program_id,
                                            &m.ai_history,
                                        );
                                    }
                                    ui.ctx().request_repaint();
                                } else if let Some(m) = self.event_modal.as_mut() {
                                    // Out of retries — surface the errors, leave the
                                    // developer's existing code untouched.
                                    m.ai_status =
                                        Some(format!("{}\n{error_list}", tr.ai_fix_failed));
                                    m.ai_fix_attempts = 0;
                                }
                            }
                        }
                        None => {
                            // No code block ⇒ Grace answered in prose or asked a
                            // clarifying question. Split it the same way the RAD
                            // designer's own prompt box does, so a question gets
                            // its own highlighted balloon instead of blending
                            // into a plain reply.
                            let (context, questions) =
                                crate::grace_host::split_developer_questions(&reply);
                            if let Some(m) = self.event_modal.as_mut() {
                                if questions.is_empty() {
                                    m.ai_history.push(crate::llm::ChatTurn::assistant(&reply));
                                } else {
                                    if !context.trim().is_empty() {
                                        m.ai_history
                                            .push(crate::llm::ChatTurn::assistant(context));
                                    }
                                    for q in questions {
                                        m.ai_history.push(crate::llm::ChatTurn::question(q));
                                    }
                                }
                                save_event_history(project_root, &program_id, &m.ai_history);
                                m.ai_status = Some(tr.ai_no_code.to_string());
                            }
                        }
                    }
                }
                crate::llm::LlmResponse::Chunk(_) => {}
                crate::llm::LlmResponse::Err(e) => {
                    if let Some(m) = self.event_modal.as_mut() {
                        m.ai_status = Some(e);
                    }
                }
            }
        }

        // ── Poll an in-flight compaction request for this handler ────────────
        let mut compact_reply: Option<crate::llm::LlmResponse> = None;
        if let Some(m) = self.event_modal.as_mut() {
            let mut keep_pending = true;
            if let Some(rx) = m.ai_compact_pending.take() {
                loop {
                    match rx.try_recv() {
                        Ok(crate::llm::LlmResponse::Chunk(_)) => {
                            // Compaction streams don't show UI, ignore chunks
                            ui.ctx().request_repaint();
                        }
                        Ok(resp) => {
                            compact_reply = Some(resp);
                            keep_pending = false;
                            break;
                        }
                        Err(std::sync::mpsc::TryRecvError::Empty) => {
                            ui.ctx().request_repaint();
                            break;
                        }
                        Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                            compact_reply = Some(crate::llm::LlmResponse::Err(
                                "The assistant worker stopped unexpectedly.".into(),
                            ));
                            keep_pending = false;
                            break;
                        }
                    }
                }
                if keep_pending {
                    m.ai_compact_pending = Some(rx);
                }
            }
        }
        if let Some(resp) = compact_reply {
            match resp {
                crate::llm::LlmResponse::Ok(summary) => {
                    let summary = summary.trim().to_string();
                    if summary.is_empty() {
                        if let Some(m) = self.event_modal.as_mut() {
                            m.ai_status = Some(tr.ai_no_code.to_string());
                        }
                    } else {
                        let turns = vec![crate::llm::ChatTurn::user(format!(
                            "[Compacted conversation summary]\n\n{summary}"
                        ))];
                        save_event_history(project_root, &program_id, &turns);
                        if let Some(m) = self.event_modal.as_mut() {
                            m.ai_history = turns;
                            m.ai_status = Some(tr.ai_compacted.to_string());
                        }
                    }
                }
                crate::llm::LlmResponse::Chunk(_) => {}
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
        let compacting = self
            .event_modal
            .as_ref()
            .map(|m| m.ai_compact_pending.is_some())
            .unwrap_or(false);
        let history_len = self
            .event_modal
            .as_ref()
            .map(|m| m.ai_history.len())
            .unwrap_or(0);
        // Snapshot the turns so the window closure can render the transcript
        // without holding a borrow on `self.event_modal`.
        let history_snapshot: Vec<crate::llm::ChatTurn> = self
            .event_modal
            .as_ref()
            .map(|m| m.ai_history.clone())
            .unwrap_or_default();
        // Scroll the transcript to the newest turn only on the frame the history
        // grows (a sent prompt or a returned reply) — never every frame, so the
        // user can freely scroll up to read earlier turns.
        let scroll_transcript = if let Some(m) = self.event_modal.as_mut() {
            let grew = history_len > m.ai_last_seen_turns;
            m.ai_last_seen_turns = history_len;
            grew
        } else {
            false
        };
        let ai_status = self.event_modal.as_ref().and_then(|m| m.ai_status.clone());
        let modal_prompt = self
            .event_modal
            .as_ref()
            .map(|m| m.ai_prompt.clone())
            .unwrap_or_default();
        if self.ai_prompt_editor.buffer_content().is_none() {
            self.ai_prompt_editor.open_buffer(
                std::path::PathBuf::from(format!("{program_id}.ai-prompt")),
                modal_prompt,
            );
        }
        self.ai_prompt_editor.known_controls = super::editor::build_known_controls(&self.form);
        let handler_source = self.event_editor.buffer_content().unwrap_or_default();
        self.ai_prompt_editor.known_data_items =
            super::editor::build_prompt_data_items(&self.form, handler_source);
        let mut do_send = false;
        let mut do_save = false;
        let mut do_compact = false;
        let mut do_clear = false;

        // Dim overlay covering the canvas (behind the window).
        let overlay = ui.ctx().content_rect();
        ui.painter()
            .rect_filled(overlay, 0.0, Color32::from_rgba_premultiplied(0, 0, 0, 140));

        let mut save_clicked = false;
        let mut cancel_clicked = false;

        // Open at 70 % of the window size; `default_*` only seed the initial
        // size, so the modal does not track the window — the user can resize.
        let screen = ui.ctx().content_rect();
        let default_w = (screen.width() * 0.70).max(360.0);
        let default_h = (screen.height() * 0.70).max(420.0);
        // Seed the initial position centred. We use `default_pos` (a seed) rather
        // than `anchor` so the window is *movable*: an anchored egui window is
        // pinned and cannot be dragged by its title bar. egui remembers the
        // dragged position by id afterwards.
        let default_pos = screen.center() - egui::vec2(default_w * 0.5, default_h * 0.5);

        let roam = modal_roam_rect(screen, default_w, default_h);

        egui::Window::new(&title)
            .id(egui::Id::new("event_editor_modal"))
            .collapsible(false)
            .resizable(true)
            .movable(true)
            .default_width(default_w)
            .default_height(default_h)
            .min_width(360.0)
            .min_height(420.0)
            .default_pos(default_pos)
            .constrain_to(roam)
            .frame(
                egui::Frame::window(&ui.ctx().global_style()).inner_margin(egui::Margin::same(16)),
            )
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
                let frame = egui::Frame::NONE
                    .fill(theme.bg_extreme)
                    .stroke(egui::Stroke::new(1.0, theme.panel_border()))
                    .corner_radius(egui::CornerRadius::same(6))
                    .inner_margin(egui::Margin::same(2));
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

                    // ── Conversation transcript for THIS handler ─────────────
                    //    Renders the stored turns so the developer can see the
                    //    conversation (the reply's code also lands in the editor
                    //    above). Bounded + scrollable so it can't push the prompt
                    //    or buttons off the modal.
                    if history_len > 0 {
                        egui::ScrollArea::vertical()
                            .max_height(150.0)
                            .auto_shrink([false, true])
                            .id_salt("event_ai_transcript")
                            .show(ui, |ui| {
                                for (index, turn) in history_snapshot.iter().enumerate() {
                                    super::editor::chat_bubble_with_response_actions(
                                        ui,
                                        &turn.role,
                                        &turn.content,
                                        14.0,
                                        project_root,
                                        egui::Id::new(("event_agent_response", &program_id, index)),
                                    );
                                }
                                // Auto-scroll to the newest turn on growth only.
                                if scroll_transcript {
                                    ui.scroll_to_cursor(Some(egui::Align::BOTTOM));
                                }
                            });
                        ui.separator();
                    }

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
                                let sz = ui.available_size();
                                ui.allocate_ui(sz, |ui| {
                                    frame.show(ui, |ui| {
                                        self.ai_prompt_editor.render_code_area(&ectx, ui);
                                    });
                                });
                                let prompt =
                                    self.ai_prompt_editor.buffer_for_save().unwrap_or_default();
                                // Enter inserts a newline; ⌘/Ctrl+Enter submits.
                                let submit = ui.input(|i| {
                                    i.key_pressed(egui::Key::Enter)
                                        && (i.modifiers.command || i.modifiers.ctrl)
                                }) && !prompt.trim().is_empty();
                                if submit && !busy {
                                    do_send = true;
                                }
                            });
                        ui.add_space(gap);
                        ui.vertical(|ui| {
                            ui.label(egui::RichText::new("✨").size(15.0));
                            let prompt =
                                self.ai_prompt_editor.buffer_for_save().unwrap_or_default();
                            let can_send = !busy && !prompt.trim().is_empty();
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
                                crate::panels::editor::token_counter(
                                    ui,
                                    Some(crate::llm::token_meter()),
                                    11.0,
                                    Color32::from_gray(170),
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

                    // ── Conversation history controls (per handler) ──────────
                    //    Always visible when an LLM is configured so the controls
                    //    are discoverable; Save/Compact/Clear are disabled until
                    //    this handler actually has a conversation.
                    let has_history = history_len > 0;
                    ui.add_space(4.0);
                    ui.horizontal(|ui| {
                        ui.label(
                            egui::RichText::new(format!("💬 {history_len}"))
                                .small()
                                .color(Color32::from_gray(150)),
                        );
                        if ui
                            .add_enabled(
                                has_history,
                                egui::Button::new(format!("💾 {}", tr.ai_save_history)),
                            )
                            .clicked()
                        {
                            do_save = true;
                        }
                        if ui
                            .add_enabled(
                                has_history && !busy && !compacting,
                                egui::Button::new(format!("🗜 {}", tr.ai_compact_history)),
                            )
                            .clicked()
                        {
                            do_compact = true;
                        }
                        if ui
                            .add_enabled(
                                has_history,
                                egui::Button::new(format!("🗑 {}", tr.ai_clear_history)),
                            )
                            .clicked()
                        {
                            do_clear = true;
                        }
                        if compacting {
                            ui.add(egui::Spinner::new());
                            ui.label(
                                egui::RichText::new(tr.ai_compacting)
                                    .small()
                                    .color(Color32::from_gray(170)),
                            );
                        }
                    });
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
            m.ai_prompt = self.ai_prompt_editor.buffer_for_save().unwrap_or_default();
        }

        // Launch a handler-generation request on explicit submit only.
        if do_send && !busy {
            let code = self.event_editor.buffer_for_save().unwrap_or_default();
            let user_prompt = self.ai_prompt_editor.buffer_for_save().unwrap_or_default();
            // Anchor the model to a nested-program handler body (the IDE owns the
            // IDENTIFICATION / PROGRAM-ID / GOBACK / END PROGRAM scaffold shown
            // read-only above and below the editor).
            let guided = format!(
                "{user_prompt}\n\nIf this is a request to WRITE or CHANGE the handler, \
                 return the COMPLETE RustCOBOL nested-program body for this event handler \
                 in a ```cobol fenced block. The body MUST include ENVIRONMENT DIVISION., \
                 DATA DIVISION., and PROCEDURE DIVISION.; preserve any WORKING-STORAGE or \
                 LINKAGE declarations the procedure uses. Do NOT emit IDENTIFICATION \
                 DIVISION, PROGRAM-ID, GOBACK, or END PROGRAM — the IDE supplies those. \
                 Use only real control properties from the skills/context; for example \
                 drop shadow/dropshadow/shadow on means ShadowEnabled. If you cannot \
                 determine the correct property or declarations after the validation \
                 round-trips, ask the developer for directions with no code block. If \
                 instead this is a QUESTION or a discussion (not a \
                 request to change the code), answer in plain prose with NO code block \
                 — your answer is shown to the developer and never written into the \
                 handler."
            );
            // Include the project's RustCOBOL skills (agentic_ai/skills) so the
            // handler assistant follows the same conventions as the dev agent.
            let skills = project_root
                .map(crate::agent::load_event_handler_skills)
                .unwrap_or_default();
            let mut event_llm_cfg = llm_cfg.clone();
            if let Some(root) = project_root {
                event_llm_cfg.system_prompt = crate::agent::effective_event_handler_prompt(root);
            }
            // Replay this handler's prior conversation so the assistant has the
            // full context (the guided suffix carries the scaffold instructions).
            let prior = self
                .event_modal
                .as_ref()
                .map(|m| m.ai_history.clone())
                .unwrap_or_default();
            let rx = match project_root {
                Some(root) => crate::grace_session::spawn_contextual_request(
                    root,
                    &event_llm_cfg,
                    &prior,
                    &guided,
                    "RAD event-handler chatbot",
                    Some(crate::agents_db::EVENT_HANDLER),
                    &format!(
                        "Editing handler `{program_id}` for `{ctrl_id}.{event_name}`.\n\nCURRENT HANDLER:\n```cobol\n{code}\n```"
                    ),
                    crate::i18n::current_tr(ui.ctx()),
                ),
                None => crate::llm::spawn_request(
                    &event_llm_cfg,
                    &prior,
                    &guided,
                    &code,
                    &format!("{program_id}.cob"),
                    &skills,
                    Some("CodeGenerator".to_string()),
                ),
            };
            if let Some(m) = self.event_modal.as_mut() {
                // Record the developer's turn (the clean prompt, not the guided
                // wrapper) so the transcript stays readable.
                m.ai_history.push(crate::llm::ChatTurn::user(&user_prompt));
                m.ai_pending = Some(rx);
                m.ai_prompt.clear();
                self.ai_prompt_editor.open_buffer(
                    std::path::PathBuf::from(format!("{program_id}.ai-prompt")),
                    String::new(),
                );
                self.ai_prompt_editor.set_context_only_completions(true);
                self.ai_prompt_editor.known_controls =
                    super::editor::build_known_controls(&self.form);
                self.ai_prompt_editor.known_data_items =
                    super::editor::build_prompt_data_items(&self.form, &code);
                m.ai_status = None;
                m.ai_fix_attempts = 0;
                save_event_history(project_root, &program_id, &m.ai_history);
            }
            ui.ctx().request_repaint();
        }

        // ── Save / Compact / Clear conversation controls ─────────────────────
        if do_save {
            if let Some(m) = self.event_modal.as_mut() {
                save_event_history(project_root, &program_id, &m.ai_history);
                m.ai_status = Some(tr.ai_history_saved.to_string());
            }
        }
        if do_compact && !busy && !compacting {
            let prior = self
                .event_modal
                .as_ref()
                .map(|m| m.ai_history.clone())
                .unwrap_or_default();
            if !prior.is_empty() {
                let rx = crate::llm::spawn_compaction(llm_cfg, &prior);
                if let Some(m) = self.event_modal.as_mut() {
                    m.ai_compact_pending = Some(rx);
                    m.ai_status = None;
                }
                ui.ctx().request_repaint();
            }
        }
        if do_clear {
            if let Some(m) = self.event_modal.as_mut() {
                m.ai_confirm_clear = true;
            }
        }

        // Clear-history confirmation dialog.
        if self
            .event_modal
            .as_ref()
            .map(|m| m.ai_confirm_clear)
            .unwrap_or(false)
        {
            let mut cancel = false;
            let mut confirm = false;
            let win_id = egui::Id::new("designer_ai_clear_confirm");
            crate::app::raise_modal_layer(ui.ctx(), win_id);
            egui::Window::new(tr.ai_clear_confirm_title)
                .id(win_id)
                .order(egui::Order::Foreground) // above the Event Editor window
                .collapsible(false)
                .resizable(false)
                .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
                .show(ui.ctx(), |ui| {
                    ui.label(tr.ai_clear_confirm_body);
                    ui.add_space(8.0);
                    ui.horizontal(|ui| {
                        if ui.button(tr.btn_cancel).clicked() {
                            cancel = true;
                        }
                        if ui.button(tr.delete_confirm_ok).clicked() {
                            confirm = true;
                        }
                    });
                });
            if cancel {
                if let Some(m) = self.event_modal.as_mut() {
                    m.ai_confirm_clear = false;
                }
            }
            if confirm {
                save_event_history(project_root, &program_id, &[]);
                if let Some(m) = self.event_modal.as_mut() {
                    m.ai_history.clear();
                    m.ai_confirm_clear = false;
                    m.ai_status = None;
                }
            }
        }

        if save_clicked {
            // Validate the handler before persisting/closing: syntax first, then
            // that every `::property` / `::method(...)` reference actually exists.
            // On any error, raise the modal instead of saving (auto-fix / save
            // anyway / keep editing).
            let content = self.event_editor.buffer_for_save().unwrap_or_default();
            let mut errs = validate_handler_syntax(&program_id, &content);
            errs.extend(validate_handler_members(&self.form, &content));
            errs.extend(validate_handler_semantics(
                &self.form,
                &ctrl_id,
                &event_name,
                &program_id,
                &content,
            ));
            if errs.is_empty() {
                // Don't persist an untouched first-time template as real handler code.
                if content != orig_source {
                    self.save_event_handler(&ctrl_id, &event_name, content);
                }
                self.event_modal = None;
            } else if let Some(m) = self.event_modal.as_mut() {
                m.syntax_errors = Some(errs);
            }
        } else if cancel_clicked {
            self.event_modal = None;
        }

        // ── Syntax-error modal (blocks save/close until resolved) ────────────
        let pending_errs = self
            .event_modal
            .as_ref()
            .and_then(|m| m.syntax_errors.clone());
        if let Some(errs) = pending_errs {
            let mut save_anyway = false;
            let mut keep_editing = false;

            let overlay = ui.ctx().content_rect();
            ui.painter()
                .rect_filled(overlay, 0.0, Color32::from_rgba_premultiplied(0, 0, 0, 160));
            crate::app::raise_modal_layer(ui.ctx(), egui::Id::new("event_syntax_modal"));
            egui::Window::new(format!("⚠  {}", tr.syntax_modal_title))
                .id(egui::Id::new("event_syntax_modal"))
                .order(egui::Order::Foreground) // above the Event Editor window
                .collapsible(false)
                .resizable(true)
                .movable(true)
                .default_width(560.0)
                .default_pos(overlay.center() - egui::vec2(280.0, 180.0))
                .frame(
                    egui::Frame::window(&ui.ctx().global_style())
                        .inner_margin(egui::Margin::same(16)),
                )
                .show(ui.ctx(), |ui| {
                    ui.label(
                        egui::RichText::new(tr.syntax_modal_explain).color(Color32::from_gray(200)),
                    );
                    ui.add_space(8.0);
                    egui::ScrollArea::vertical()
                        .max_height(280.0)
                        .auto_shrink([false, true])
                        .show(ui, |ui| {
                            for d in &errs {
                                ui.horizontal_wrapped(|ui| {
                                    ui.label(
                                        egui::RichText::new(format!("line {}:{}", d.line, d.col))
                                            .monospace()
                                            .strong()
                                            .color(Color32::from_rgb(240, 130, 100)),
                                    );
                                    // The raw parser message, verbatim.
                                    ui.label(
                                        egui::RichText::new(&d.message)
                                            .monospace()
                                            .color(Color32::from_rgb(240, 200, 120)),
                                    );
                                });
                                // Plain-English explanation.
                                ui.label(
                                    egui::RichText::new(explain_syntax_error(&d.message))
                                        .small()
                                        .color(Color32::from_gray(170)),
                                );
                                ui.add_space(6.0);
                            }
                        });
                    ui.add_space(8.0);
                    ui.separator();
                    ui.horizontal(|ui| {
                        // The one-click 🔧 Auto-fix button was removed: the same
                        // reformat is reachable from ✨ Beautify in the editor
                        // status row above, and Save re-validates. The action it
                        // ran is kept in `autofix_event_handler` below.
                        if ui.button(tr.syntax_keep_editing).clicked() {
                            keep_editing = true;
                        }
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if ui
                                .button(
                                    egui::RichText::new(tr.syntax_save_anyway)
                                        .color(Color32::from_rgb(220, 120, 120)),
                                )
                                .clicked()
                            {
                                save_anyway = true;
                            }
                        });
                    });
                });

            if save_anyway {
                let content = self.event_editor.buffer_for_save().unwrap_or_default();
                if content != orig_source {
                    self.save_event_handler(&ctrl_id, &event_name, content);
                }
                self.event_modal = None;
            } else if keep_editing {
                if let Some(m) = self.event_modal.as_mut() {
                    m.syntax_errors = None;
                }
            }
        }
    }

    /// COBOL Structure popup (spec 005): hosts the **same** `EditorPanel` used by
    /// the event modal and the main code editor, so the section / procedure code
    /// gets IntelliSense, syntax colouring and find/replace too. Edits live-sync
    /// back to the form block.
    pub fn show_cobol_structure_window(
        &mut self,
        ctx: &egui::Context,
        tr: &crate::i18n::Tr,
        llm: &crate::llm::LlmConfig,
        project_root: Option<&std::path::Path>,
    ) {
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

        let screen = ctx.content_rect();
        let default_w = (screen.width() * 0.6).max(420.0);
        // Nominal opened height, used ONLY to centre `default_pos` and to build
        // the roam constraint. The window itself auto-sizes to its content,
        // whose one variable part is the grip-owned editor box below.
        let nominal_h = CS_EDITOR_DEFAULT_ROWS * self.cs_editor.code_row_height(ctx)
            + CS_EDITOR_BOX_CHROME
            + CS_WINDOW_CHROME_NOMINAL;
        let mut close = false;

        // `anchor` pins a window outright: egui re-places an anchored window
        // every frame, so its title bar cannot be dragged at all — no amount of
        // constraining changes that. Seed the centred position with
        // `default_pos` instead and let egui remember where the developer drags
        // it, the same way the Event Editor modal does.
        let default_pos = screen.center() - egui::vec2(default_w * 0.5, nominal_h * 0.5);
        let roam = modal_roam_rect(screen, default_w, nominal_h);

        egui::Window::new(format!("{} — {title}", tr.cs_open))
            .id(egui::Id::new("cobol_structure_window"))
            .collapsible(false)
            // The window frame itself is NOT resizable: the editor box's grip
            // below is the single size authority and the window hugs it. A
            // resizable window renegotiates its rect against content every
            // frame — the self-inflation this modal has relapsed into before
            // (see CONVENTIONS.md, "a window may NEVER resize itself").
            //
            // `auto_sized`, not plain `resizable(false)`: a non-auto-sized
            // window's TITLE BAR takes `available_width()` as its min width,
            // and that available comes from the internal resize's
            // `desired_size`, which egui only ever ratchets UP — the title
            // would echo the widest the window has ever been and the modal
            // could grow with the grip but never shrink back. An auto-sized
            // window's title follows last frame's window rect instead, which
            // converges when the box shrinks (egui 0.36 `window.rs`).
            .auto_sized()
            .movable(true)
            .default_width(default_w)
            .default_pos(default_pos)
            .constrain_to(roam)
            .frame(egui::Frame::window(&ctx.global_style()).inner_margin(egui::Margin::same(14)))
            .show(ctx, |ui| {
                // Width authority = the editor box (grip state, seeded at
                // `default_w`). Every width-FILLING row (the status row's
                // right-aligner, separators, the AI bar) must follow the box,
                // never `available_width`: an auto-sizing window echoes a row
                // that fills its available width, and that echo is a ratchet —
                // the window could grow but never shrink back.
                let body_w = self.cs_box_rect.map(|r| r.width()).unwrap_or(default_w);
                ui.set_max_width(body_w);
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

                // ── Hosted COBOL editor — a BOUNDED, user-resizable box. ─────
                //    Same cure as the Event Editor modal and the debugger
                //    window: the `egui::Resize` state (default seed + the
                //    corner grip's drag, clamped) is the ONLY size authority.
                //    The editor fills the box exactly and scrolls inside it, so
                //    the reported content min-size equals the box and egui's
                //    per-frame `max(desired, content)` ratchet has nothing to
                //    grow on — the box cannot change size on its own, whatever
                //    the content length or the IDE language.
                let row_h = self.cs_editor.code_row_height(ui.ctx());
                let box_default = egui::vec2(
                    default_w,
                    (CS_EDITOR_DEFAULT_ROWS * row_h + CS_EDITOR_BOX_CHROME).ceil(),
                );
                let box_min = egui::vec2(
                    420.0,
                    (CS_EDITOR_MIN_ROWS * row_h + CS_EDITOR_BOX_CHROME).ceil(),
                );
                let ectx = ui.ctx().clone();
                let theme = crate::theme::active();
                let frame = egui::Frame::NONE
                    .fill(theme.bg_extreme)
                    .stroke(egui::Stroke::new(1.0, theme.panel_border()))
                    .corner_radius(egui::CornerRadius::same(6))
                    .inner_margin(egui::Margin::same(2));
                let box_rect = egui::Resize::default()
                    // `Resize` keeps its state in `ctx.data()`, which all
                    // viewports share — salt with the viewport so two open
                    // designers don't fight over one box size.
                    .id_salt(format!(
                        "cobol_structure_code_box_{}",
                        ui.ctx().viewport_id().0.value()
                    ))
                    .resizable([true, true])
                    .min_size(box_min)
                    .max_size(egui::vec2(4000.0, 4000.0))
                    .default_size(box_default) // seed only
                    .show(ui, |ui| {
                        // `sz` is the Resize box: user/default state, bounded —
                        // NOT "remaining space" of an auto-sizing container.
                        let sz = ui.available_size();
                        let origin = ui.max_rect().min;
                        ui.allocate_ui(sz, |ui| {
                            // Fill the box exactly so the content min-size
                            // equals the box (no auto-grow, no auto-shrink).
                            ui.set_min_size(sz);
                            frame.show(ui, |ui| {
                                self.cs_editor.render_code_area(&ectx, ui);
                            });
                        });
                        egui::Rect::from_min_size(origin, sz)
                    });
                self.cs_box_rect = Some(box_rect);

                // ── AI assistant (inline) — same bar as the code editor, with
                //    per-block conversation history, transcript, and save/compact/
                //    clear. Its reply replaces this block's COBOL.
                if llm.is_configured() {
                    ui.add_space(4.0);
                    ui.separator();
                    let code = self.cs_editor.buffer_content().unwrap_or("").to_string();
                    let buf_path = std::path::PathBuf::from(format!(
                        "cobol-structure/{}",
                        target.buffer_key()
                    ));
                    if let Some(new_code) = self.cs_editor.ai_bar_inline(
                        ui,
                        llm,
                        tr,
                        "cs_ai",
                        &buf_path,
                        &code,
                        false,
                        project_root,
                    ) {
                        self.cs_editor.open_buffer(buf_path, new_code);
                        self.dirty = true;
                    }
                }

                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    if ui.button(tr.btn_save).clicked() {
                        if let Some(content) = self.cs_editor.buffer_for_save() {
                            if cs::set_block_text(&mut self.form, target, content) {
                                self.dirty = true;
                            }
                        }
                        close = true;
                    }
                    if ui.button(tr.btn_cancel).clicked() {
                        close = true;
                    }
                });
            });

        if close {
            self.cobol_structure_edit = None;
            self.cs_loaded = None;
        }
    }

    /// 050 R7 — the neumorphic seed a NEW control should take, if any.
    ///
    /// `None` under a self-contained theme. Seeding there would write
    /// background colours and shadow properties for a register the theme
    /// ignores, so the developer's form would accumulate settings that change
    /// nothing on screen — and switching back to Liquid Glass would show them.
    fn neumorphic_seed(&self) -> Option<cobolt_forms::model::GlassStyle> {
        (!self.active_surface_theme.is_self_contained()
            && self.form.glass_style.is_neumorphic())
        .then_some(self.form.glass_style)
    }

    /// 049 — the state the canvas is SHOWING the rail in.
    ///
    /// The developer's `Collapsed` until they click the breadcrumb's toggle,
    /// their choice afterwards. `false` for a form with no sidebar at all.
    pub(crate) fn rail_shown_collapsed(&self) -> bool {
        let designed = self
            .shell_side_menu()
            .map(|c| c.side_menu_collapsed())
            .unwrap_or(false);
        self.rail_view_collapsed.unwrap_or(designed)
    }

    /// The SideMenu that makes this form a shell, if it has one.
    fn shell_side_menu(&self) -> Option<&cobolt_forms::Control> {
        cobolt_forms::breadcrumb::shell_side_menu(&self.form)
    }

    /// Drop the shown-state override when the developer edits `Collapsed`
    /// itself, so the property they just typed is what they see. Called once a
    /// frame, before the canvas draws.
    fn sync_rail_view(&mut self) {
        let designed = self.shell_side_menu().map(|c| c.side_menu_collapsed());
        if designed != self.rail_designed_collapsed {
            self.rail_designed_collapsed = designed;
            self.rail_view_collapsed = None;
        }
    }

    /// The controls the CANVAS paints: the designed ones with the shown rail
    /// state applied. `None` when nothing needs changing (the common case).
    ///
    /// A rail shown collapsed is DRAWN at the collapsed width. It used to keep
    /// its full designed width and merely lay collapsed content out inside it,
    /// so `Collapsed` appeared to do half its job. The designed rect is left
    /// exactly as it is — this list is for painting only, and selection,
    /// dragging and the saved `.cfrm` all still see the design.
    fn rail_view_controls(
        &self,
        controls: &[cobolt_forms::Control],
    ) -> Option<Vec<cobolt_forms::Control>> {
        let side = self.shell_side_menu()?;
        let shown = self.rail_shown_collapsed();
        if !shown && !side.side_menu_collapsed() {
            return None; // open, and designed open: nothing to override
        }
        let side_id = side.id.clone();
        let footer_id = cobolt_forms::model::side_menu_footer_id(&side_id);
        let width = cobolt_forms::sidebar::shown_width(side, shown) as i32;
        Some(
            controls
                .iter()
                .map(|c| {
                    if c.id != side_id && c.id != footer_id {
                        return c.clone();
                    }
                    let mut c = c.clone();
                    // The footer Panel is pinned to the rail's column, so it
                    // narrows with it — otherwise it hangs out over the
                    // content area the moment the rail collapses.
                    c.rect.w = width;
                    if c.id == side_id {
                        c.set_prop("Collapsed", shown);
                    }
                    c
                })
                .collect(),
        )
    }

    /// 049 — the SideMenu seam under `(px, py)`, if any: the line between the
    /// header pane and the menu, or between the menu and the footer.
    ///
    /// DESIGN TIME ONLY. The pane heights are properties the developer sets and
    /// nothing at run time may touch, so this lives in the designer and has no
    /// counterpart in the preview, Run Form or the shell. Offered only while
    /// the sidebar is SELECTED, so the seams cannot steal a drag from a control
    /// sitting over the rail.
    fn sidebar_seam_at(&self, px: i32, py: i32) -> Option<(String, SidebarPane)> {
        let sid = self.selected_ids.first()?;
        let c = self.form.find_control(sid)?;
        if c.control_type != ControlType::SideMenu {
            return None;
        }
        if px < c.rect.x || px > c.rect.x + c.rect.w {
            return None;
        }
        let chrome = cobolt_forms::sidebar::SidebarChrome::from_control(c);
        let near = |edge: f32| (py as f32 - edge).abs() <= SIDEBAR_SEAM_TOL;
        let header_seam = c.rect.y as f32 + chrome.header_h;
        let footer_seam = (c.rect.y + c.rect.h) as f32 - chrome.footer_h;
        // Footer first: with a tiny rail the two seams can overlap, and the
        // bottom one is the one under the pointer's own half.
        if near(footer_seam) && py as f32 >= (header_seam + footer_seam) * 0.5 {
            return Some((c.id.clone(), SidebarPane::Footer));
        }
        if near(header_seam) {
            return Some((c.id.clone(), SidebarPane::Header));
        }
        if near(footer_seam) {
            return Some((c.id.clone(), SidebarPane::Footer));
        }
        None
    }

    /// One pointer move of a seam drag: write the pane's height live, so the
    /// rail re-lays out under the pointer.
    ///
    /// The header grows DOWN from the top and the footer grows UP from the
    /// bottom, so the seam tracks the pointer in both cases.
    fn apply_sidebar_seam_drag(&mut self, py: i32) {
        let DragState::ResizingSidebarPane {
            ref id,
            pane,
            orig_h,
            start_y,
        } = self.drag.clone()
        else {
            return;
        };
        let delta = match pane {
            SidebarPane::Header => py - start_y,
            SidebarPane::Footer => start_y - py,
        };
        let gp = self.form.grid_size as i32;
        let sn = self.form.snap_to_grid;
        let limit = self
            .form
            .find_control(id)
            .map(|c| c.rect.h)
            .unwrap_or(i32::MAX);
        let h = snap((orig_h + delta).max(SIDEBAR_PANE_MIN), gp, sn).min(limit);
        if let Some(c) = self.form.find_control_mut(id) {
            c.set_prop(pane.prop(), h as i64);
        }
        self.dirty = true;
    }

    /// End a seam drag: the height was written live, so this records ONE undo
    /// entry for the whole gesture rather than one per pointer move.
    fn finish_sidebar_seam_drag(&mut self) {
        let DragState::ResizingSidebarPane {
            ref id,
            pane,
            orig_h,
            ..
        } = self.drag.clone()
        else {
            return;
        };
        let now = self
            .form
            .find_control(id)
            .and_then(|c| c.get_prop(pane.prop()))
            .map(|v| v.as_i64() as i32)
            .unwrap_or(orig_h);
        if now != orig_h {
            self.apply(Cmd::SetProperty {
                id: id.clone(),
                key: pane.prop().to_owned(),
                old: Some(PropValue::Int(orig_h as i64)),
                new: PropValue::Int(now as i64),
            });
        }
        self.dirty = true;
    }

    /// Draw the two seam grips on a selected SideMenu, so the developer can see
    /// that the header and footer are draggable at all.
    fn draw_sidebar_seam_grips(&self, painter: &egui::Painter, origin: Pos2) {
        let Some(sid) = self.selected_ids.first() else {
            return;
        };
        let Some(c) = self.form.find_control(sid) else {
            return;
        };
        if c.control_type != ControlType::SideMenu {
            return;
        }
        let chrome = cobolt_forms::sidebar::SidebarChrome::from_control(c);

        // HIGH CONTRAST against the rail the grip sits on, not a fixed accent.
        // A rail is as likely to be black as white — the developer chooses —
        // and one hardcoded blue is invisible on one of them. The accent is
        // kept where it reads and replaced where it does not, then the grip is
        // outlined in the opposite tone so it stands off ANY background.
        let rail_bg = cobolt_forms::paint::composite_premultiplied_over(
            c.get_prop("BackgroundColor")
                .map(|v| parse_color(v.as_str()))
                .unwrap_or(Color32::TRANSPARENT),
            cobolt_forms::paint::form_backdrop_of(painter.ctx()),
        );
        // MAXIMUM contrast, not merely adequate: whichever of black or white
        // reads best on this rail, never a mid accent. An accent that merely
        // clears the 3:1 graphics minimum still disappears into a dark slate
        // rail at a glance, and a handle the developer cannot find is not a
        // handle. The rim is then the opposite tone, so the grip stands off
        // even where the rail happens to match the ink.
        let ink = if cobolt_forms::paint::contrast_ratio(Color32::WHITE, rail_bg)
            >= cobolt_forms::paint::contrast_ratio(Color32::BLACK, rail_bg)
        {
            Color32::WHITE
        } else {
            Color32::BLACK
        };
        let rim = if ink == Color32::WHITE {
            Color32::from_black_alpha(220)
        } else {
            Color32::from_white_alpha(230)
        };

        for y in [
            c.rect.y as f32 + chrome.header_h,
            (c.rect.y + c.rect.h) as f32 - chrome.footer_h,
        ] {
            let (x0, x1) = (
                origin.x + c.rect.x as f32,
                origin.x + (c.rect.x + c.rect.w) as f32,
            );
            let sy = origin.y + y;
            // The seam itself: a hairline of the opposite tone under the ink,
            // so the line survives even where the rail matches the ink exactly.
            painter.line_segment(
                [Pos2::new(x0, sy + 1.0), Pos2::new(x1, sy + 1.0)],
                egui::Stroke::new(1.0, rim),
            );
            painter.line_segment(
                [Pos2::new(x0, sy), Pos2::new(x1, sy)],
                egui::Stroke::new(1.5, ink),
            );
            // A stubby centre grip, the same language the form-edge grips use,
            // outlined so it reads on either tone.
            let cx = (x0 + x1) * 0.5;
            let grip = egui::Rect::from_center_size(Pos2::new(cx, sy), Vec2::new(28.0, 6.0));
            painter.rect_filled(grip, 3.0, ink);
            painter.rect_stroke(
                grip,
                3.0,
                egui::Stroke::new(1.0, rim),
                egui::StrokeKind::Outside,
            );
        }
    }

    /// Every form a menu item can open: the WHOLE `forms/` tree, subfolders
    /// included.
    ///
    /// This used to read one directory — the folder the form being edited
    /// happens to sit in — so a menu edited from `forms/Menus & Bars/` could
    /// not target anything outside that folder, and no nested form was ever
    /// offered anywhere. The root is the nearest ancestor called `forms`,
    /// falling back to the form's own folder for a project that does not use
    /// that layout.
    ///
    /// Which load paths this `.cfrm` allows — `(embeddable, standaloneable)`
    /// (049 R17 / 051 R25).
    ///
    /// Read straight out of the file's `form-format` attribute rather than by
    /// parsing the whole form: the picker asks this of every candidate, and a
    /// full parse per form per frame is not worth a single attribute. An
    /// UNREADABLE file counts as both — the picker now filters rather than
    /// warns (051 R25), and a guess must never hide a form from every list;
    /// the build's own checks stay the authority.
    pub(crate) fn cfrm_load_paths(path: &std::path::Path) -> (bool, bool) {
        let Ok(text) = std::fs::read_to_string(path) else {
            return (true, true);
        };
        let head = &text[..text.len().min(4096)];
        match head.find("form-format=\"") {
            Some(at) => {
                let rest = &head[at + 13..];
                let value = rest.split('"').next().unwrap_or("");
                let fmt = cobolt_forms::model::FormFormat::from_str(value);
                (fmt.allows_embedded(), fmt.allows_standalone())
            }
            // No attribute at all = a form written before 049: Standalone.
            None => (false, true),
        }
    }

    /// Returns `(label, name, embeddable, standaloneable)`: the label is the
    /// path relative to the root, so two forms sharing a name in different
    /// folders are tellable apart, while the action keeps storing the form's
    /// own name.
    fn forms_under(cfrm_dir: Option<&std::path::Path>) -> Vec<(String, String, bool, bool)> {
        let Some(dir) = cfrm_dir else {
            return Vec::new();
        };
        let root = dir
            .ancestors()
            .find(|a| {
                a.file_name()
                    .map(|n| n.to_string_lossy().eq_ignore_ascii_case("forms"))
                    .unwrap_or(false)
            })
            .unwrap_or(dir);

        let mut out: Vec<(String, String, bool, bool)> = Vec::new();
        let mut stack = vec![(root.to_path_buf(), 0usize)];
        while let Some((d, depth)) = stack.pop() {
            let Ok(entries) = std::fs::read_dir(&d) else {
                continue;
            };
            for e in entries.flatten() {
                let path = e.path();
                let Ok(kind) = e.file_type() else { continue };
                if kind.is_dir() {
                    // Bounded: a deep tree (or one a symlink made circular)
                    // must not hang the editor while the modal is open.
                    if depth < 8 {
                        stack.push((path, depth + 1));
                    }
                    continue;
                }
                if path.extension().and_then(|x| x.to_str()) != Some("cfrm") {
                    continue;
                }
                let Some(name) = path.file_stem().map(|s| s.to_string_lossy().into_owned()) else {
                    continue;
                };
                let label = path
                    .strip_prefix(root)
                    .ok()
                    .map(|rel| rel.with_extension(""))
                    .map(|rel| rel.to_string_lossy().replace('\\', "/"))
                    .unwrap_or_else(|| name.clone());
                let (embeddable, standaloneable) = Self::cfrm_load_paths(&path);
                out.push((label, name, embeddable, standaloneable));
            }
        }
        out.sort_by_key(|(label, _, _, _)| label.to_lowercase());
        out.dedup_by(|a, b| a.0 == b.0);
        out
    }

    /// Outline the container a drag would drop into, so the developer sees the
    /// target before letting go.
    ///
    /// The hint asks `resolve_drop_target` the same question `reparent_to_drop`
    /// will ask when the drag ends, from the same point — the dragged control's
    /// centre — so what is outlined is what actually adopts it. The SideMenu's
    /// footer Panel lights up like any other container: it IS one.
    fn draw_drop_hint(
        &self,
        painter: &egui::Painter,
        origin: Pos2,
        origins: &[(String, i32, i32)],
    ) {
        let Some((primary, _, _)) = origins.first() else {
            return;
        };
        let Some(idx) = self.form.controls.iter().position(|c| &c.id == primary) else {
            return;
        };
        let r = self.form.controls[idx].rect;
        let target = super::containers::resolve_drop_target(
            &self.form.controls,
            r.x + r.w / 2,
            r.y + r.h / 2,
            idx,
            &self.active_tabs,
        );
        let super::containers::DropTarget::Into { container, .. } = target else {
            return;
        };
        // Never outline the container it already belongs to: the hint is for a
        // change of parent, and a permanent glow around the current one is
        // noise.
        if self.form.controls[idx].parent.as_deref() == Some(container.as_str()) {
            return;
        }
        let Some(c) = self.form.find_control(&container) else {
            return;
        };
        let cr = c.content_rect();
        if cr.w <= 0 || cr.h <= 0 {
            return;
        }
        let rect = egui::Rect::from_min_size(
            origin + Vec2::new(cr.x as f32, cr.y as f32),
            Vec2::new(cr.w as f32, cr.h as f32),
        );
        let accent = Color32::from_rgb(90, 170, 255);
        painter.rect_filled(rect, 4.0, Color32::from_rgba_unmultiplied(90, 170, 255, 28));
        painter.rect_stroke(
            rect,
            4.0,
            egui::Stroke::new(2.0, accent),
            egui::StrokeKind::Middle,
        );
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
                    let (props, animations, src_rect, src_type) =
                        match std::mem::replace(&mut self.format_painter, FormatPainter::Idle) {
                            FormatPainter::WaitingForTarget {
                                props,
                                animations,
                                src_rect,
                                src_type,
                            } => (props, animations, src_rect, src_type),
                            _ => unreachable!(),
                        };
                    // Paste style + geometry onto the target control
                    if let Some(tgt) = self.form.find_control_mut(&target_id) {
                        // Same type ⇒ a deep copy of the look: everything the
                        // capture kept. Different types ⇒ only the properties
                        // that mean the same thing on both, because a
                        // Button has no use for a DataGrid's header colours.
                        let same_type = tgt.control_type == src_type;
                        for (k, v) in &props {
                            if same_type || STYLE_PROP_KEYS.contains(&k.as_str()) {
                                tgt.properties.insert(k.clone(), v.clone());
                            }
                        }
                        tgt.animations = animations.clone();
                        // Copy only size (w, h) from source — preserve target's x, y position
                        tgt.rect.w = src_rect.w;
                        tgt.rect.h = src_rect.h;
                    }
                    self.format_painter = FormatPainter::WaitingForTarget {
                        props,
                        animations,
                        src_rect,
                        src_type,
                    };
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
                    // 049 — a sidebar seam outranks everything: the pointer is
                    // on the rail, and without this the press would start
                    // moving the rail instead of sizing its pane.
                    if let Some((id, pane)) = self.sidebar_seam_at(px, py) {
                        // Read the height the way the RENDERER does, defaults
                        // and all. Reading the raw property and falling back to
                        // ZERO meant the first drag on a sidebar that had never
                        // set the property started from 0: the header snapped to
                        // the top and the footer ran a whole pane out of step
                        // with the cursor. Every drag after that worked, because
                        // the first one had written the property.
                        let orig_h = self
                            .form
                            .find_control(&id)
                            .map(|c| {
                                let chrome =
                                    cobolt_forms::sidebar::SidebarChrome::from_control(c);
                                match pane {
                                    SidebarPane::Header => chrome.header_h,
                                    SidebarPane::Footer => chrome.footer_h,
                                }
                            })
                            .unwrap_or(0.0) as i32;
                        self.drag = DragState::ResizingSidebarPane {
                            id,
                            pane,
                            orig_h,
                            start_y: py,
                        };
                    } else
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
                            //
                            // 049 — so is a SideMenu's footer Panel: the sidebar
                            // owns where it sits. It is a normal container in
                            // every other way, and it is the drop target the
                            // footer band offers.
                            if ctrl.is_anchored() || ctrl.is_side_menu_footer() {
                                continue;
                            }
                            ctrl.rect.x = snap(ox + dx, gp, sn);
                            ctrl.rect.y = snap(oy + dy, gp, sn);
                        }
                    }
                    // Show where the drop will land. `reparent_to_drop` decides
                    // the real target from the control's CENTRE when the drag
                    // ends, so the hint asks the same question of the same
                    // resolver — a hint that guessed differently would be worse
                    // than none.
                    self.draw_drop_hint(painter, origin, &origins);
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
                        let floor = ctrl.clone();
                        ctrl.rect = apply_resize(orig_rect, handle, dx, dy, gp, sn, Some(&floor));
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
                            egui::StrokeKind::Middle,
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
                DragState::ResizingSidebarPane { .. } => {
                    self.apply_sidebar_seam_drag(py);
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
                        self.form.find_control(&id),
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
                DragState::ResizingSidebarPane { .. } => {
                    self.finish_sidebar_seam_drag();
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
            egui::StrokeKind::Middle,
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

/// Grid dot color with guaranteed contrast against the canvas backdrop.
///
/// The classic bluish dot (140,160,220) disappears when the form background
/// sits near it — a periwinkle or light-blue form hides its own grid. When the
/// luminance gap to the backdrop is too small, switch to dark dots on light
/// backgrounds and light dots on dark ones, at a stronger alpha: the fallback
/// exists precisely because visibility was the problem.
fn grid_dot_color(backdrop: Color32, glass: bool) -> Color32 {
    let luminance = |color: Color32| {
        (0.299 * color.r() as f32 + 0.587 * color.g() as f32 + 0.114 * color.b() as f32) / 255.0
    };
    let default_dot = Color32::from_rgb(140, 160, 220);
    if (luminance(backdrop) - luminance(default_dot)).abs() >= 0.18 {
        let alpha = if glass { 35 } else { 60 };
        return Color32::from_rgba_premultiplied(140, 160, 220, alpha);
    }
    let alpha = if glass { 90 } else { 120 };
    if luminance(backdrop) > 0.5 {
        Color32::from_rgba_unmultiplied(30, 40, 70, alpha)
    } else {
        Color32::from_rgba_unmultiplied(225, 232, 255, alpha)
    }
}

fn draw_grid(
    painter: &egui::Painter,
    canvas: egui::Rect,
    step: f32,
    glass: bool,
    backdrop: Color32,
) {
    let dot_color = grid_dot_color(backdrop, glass);
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
    rounding: egui::CornerRadius,
    step: f32,
    glass: bool,
    backdrop: Color32,
) {
    if step <= 0.5 {
        return;
    }
    let cap = 0.5 * rect.width().min(rect.height());
    let clamp_r = |r: f32| r.max(0.0).min(cap);
    let radii = [
        clamp_r(f32::from(rounding.nw)),
        clamp_r(f32::from(rounding.ne)),
        clamp_r(f32::from(rounding.se)),
        clamp_r(f32::from(rounding.sw)),
    ];
    if radii.iter().all(|r| *r < 0.5) {
        return;
    }

    let dot_color = grid_dot_color(backdrop, glass);
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
        CT::SideMenu => "SideMenu",
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
        CT::IndexedFile => "IndexedFile",
        CT::BarChart => "BarChart",
        CT::LineChart => "LineChart",
        CT::PieChart => "PieChart",
        CT::AreaChart => "AreaChart",
        CT::ScatterChart => "ScatterChart",
        CT::DonutChart => "DonutChart",
        CT::Knob => "Knob",
        CT::Gauge => "Gauge",
        CT::Switch => "Switch",
        CT::FileDropZone => "FileDropZone",
        CT::Maps => "Maps",
        CT::WebSearch => "WebSearch",
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
        egui::StrokeKind::Middle,
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
            if let Some(b) = c
                .events
                .iter_mut()
                .find(|b| b.event.eq_ignore_ascii_case(event))
            {
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
                form.user_procedures
                    .push(cobolt_forms::model::UserProcedure {
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

/// `true` when two axis-aligned rects `(x, y, w, h)` overlap (share area).
fn rects_overlap(a: (i32, i32, i32, i32), b: (i32, i32, i32, i32)) -> bool {
    a.0 < b.0 + b.2 && b.0 < a.0 + a.2 && a.1 < b.1 + b.3 && b.1 < a.1 + a.3
}

/// Smallest offset that moves `moving` clear of every `obstacle`, searched in an
/// expanding ring (grid step `STEP`), preferring down/right for a natural layout
/// flow. `bounds` — when `Some((w, h))` — keeps the moved rect inside the form.
/// Returns `(0, 0)` if it is already clear, or if nothing within `max` pixels
/// clears it (the caller then leaves the control where the agent put it rather
/// than shoving it somewhere worse).
fn nearest_free_offset(
    moving: (i32, i32, i32, i32),
    obstacles: &[(i32, i32, i32, i32)],
    bounds: Option<(i32, i32)>,
    max: i32,
) -> (i32, i32) {
    let clear = |dx: i32, dy: i32| -> bool {
        let m = (moving.0 + dx, moving.1 + dy, moving.2, moving.3);
        if let Some((bw, bh)) = bounds {
            if m.0 < 0 || m.1 < 0 || m.0 + m.2 > bw || m.1 + m.3 > bh {
                return false;
            }
        }
        !obstacles.iter().any(|o| rects_overlap(m, *o))
    };
    if clear(0, 0) {
        return (0, 0);
    }
    const STEP: i32 = 4;
    let mut r = STEP;
    while r <= max {
        // Ring candidates ordered down, right, up, left, then the diagonals.
        for (dx, dy) in [
            (0, r),
            (r, 0),
            (0, -r),
            (-r, 0),
            (r, r),
            (-r, r),
            (r, -r),
            (-r, -r),
        ] {
            if clear(dx, dy) {
                return (dx, dy);
            }
        }
        r += STEP;
    }
    (0, 0)
}

/// Current value of a property for undo capture. Structural properties (X, Y,
/// Width, Height, Visible, …) live on `Control`'s own fields rather than in its
/// `properties` map, so reading them from `properties` always yields `None` and
/// leaves the change un-undoable. This mirrors `apply_structural_prop`'s key set
/// and returns the live field value; everything else falls back to `properties`.
fn structural_prop_value(ctrl: &Control, key: &str) -> Option<PropValue> {
    match key.to_ascii_lowercase().as_str() {
        "x" => Some(PropValue::Int(ctrl.rect.x as i64)),
        "y" => Some(PropValue::Int(ctrl.rect.y as i64)),
        "width" => Some(PropValue::Int(ctrl.rect.w as i64)),
        "height" => Some(PropValue::Int(ctrl.rect.h as i64)),
        "visible" => Some(PropValue::Bool(ctrl.visible)),
        "enabled" => Some(PropValue::Bool(ctrl.enabled)),
        "taborder" => Some(PropValue::Int(ctrl.tab_order as i64)),
        "zorder" => Some(PropValue::Int(ctrl.z_order as i64)),
        // `get_prop` (not `properties.get`) so an agent-supplied spelling that
        // differs only in case still captures the real previous value for undo.
        _ => ctrl.get_prop(key).cloned(),
    }
}

/// Canonical (CamelCase) spelling of a form-level property name, or `None` when
/// the form has no such property.
///
/// This is the single list of settable form properties, and it must stay in step
/// with `crate::agent::form_property_valid`, which the change-set validator uses.
/// Canonical "true"/"false" for form-property reads.
fn bool_str(v: bool) -> String {
    if v { "true".to_string() } else { "false".to_string() }
}

pub(crate) fn canonical_form_prop_key(key: &str) -> Option<&'static str> {
    FORM_PROP_KEYS
        .iter()
        .find(|k| k.eq_ignore_ascii_case(key))
        .copied()
}

/// Every settable form-level property, in canonical spelling.
pub(crate) const FORM_PROP_KEYS: &[&str] = &[
    "Title",
    "BackgroundColor",
    "Width",
    "Height",
    "Transparency",
    "GridSize",
    "SnapToGrid",
    "GlassStyle",
    "BackgroundGradientEnabled",
    "BackgroundGradientStartColor",
    "BackgroundGradientEndColor",
    "BackgroundGradientDirection",
    "Target",
    "BackgroundImage",
    "BgImageMode",
    "Theme",
    "UseThemeBackground",
    // 037 Main form & window lifecycle
    "MainForm",
    "TaskbarIcon",
    "CanMinimize",
    "CanMaximize",
    "WindowState",
    "FullScreen",
    "TitleVisible",
    "WindowEffects",
    // 049 Application shell
    "FormFormat",
    "MenuPaneCustom",
    "MenuPaneColor",
    "MenuPaneGradientEnabled",
    "MenuPaneGradientStartColor",
    "MenuPaneGradientEndColor",
    "MenuPaneGradientDirection",
    "MenuPaneTransparency",
    "MenuPaneImage",
    "MenuPaneImageMode",
    // Window start position
    "X",
    "Y",
    "StartPosition",
];

/// The spelling under which `key` is already stored on `ctrl`, or `key` itself
/// when the control has no such property yet.
///
/// RustCOBOL property names are case-insensitive and the change-set validator
/// accepts any casing, so an agent may legitimately send `caption` for
/// `Caption`. Inserting that verbatim would leave a second entry beside the real
/// one, and `get_prop`'s exact-match-first lookup would keep returning the stale
/// value — the write would be reported as applied and change nothing.
fn canonical_prop_key(ctrl: &Control, key: &str) -> String {
    ctrl.properties
        .keys()
        .find(|k| k.eq_ignore_ascii_case(key))
        .cloned()
        .unwrap_or_else(|| key.to_owned())
}

/// Target of a `deploy_control` whose id is already in use — see
/// [`FormDesigner::redeploy_target`].
enum RedeployTarget {
    /// Index in the pending command list of the `AddControl` that owns the id.
    Pending(usize),
    /// Id of the control already on the form (its own exact spelling).
    OnForm(String),
}

/// Apply a `deploy_control` property bag to a control, geometry included.
fn apply_deploy_properties(
    ctrl: &mut Control,
    properties: &serde_json::Map<String, serde_json::Value>,
) {
    if let Some(x) = json_prop_i32(properties, "X") {
        ctrl.rect.x = x;
    }
    if let Some(y) = json_prop_i32(properties, "Y") {
        ctrl.rect.y = y;
    }
    if let Some(w) = json_prop_i32(properties, "Width") {
        ctrl.rect.w = w;
    }
    if let Some(h) = json_prop_i32(properties, "Height") {
        ctrl.rect.h = h;
    }
    for (k, v) in properties {
        if matches!(k.as_str(), "X" | "Y" | "Width" | "Height") {
            continue;
        }
        if let Some(pv) = json_to_prop(v) {
            apply_structural_prop(ctrl, k, &pv);
        }
    }
}

/// The prompt box's completion state, carried between frames.
#[derive(Default)]
pub struct PromptAc {
    visible: bool,
    sel: usize,
    items: Vec<crate::prompt_complete::Suggestion>,
    /// Byte range in the prompt text that accepting replaces.
    replace: (usize, usize),
}

impl PromptAc {
    fn close(&mut self) {
        self.visible = false;
        self.items.clear();
        self.sel = 0;
    }
}

/// What the key handling in front of the editor decided this frame.
struct PromptAcInput {
    sel: usize,
    accept: bool,
    dismiss: bool,
    editable: bool,
}

fn char_to_byte(text: &str, char_index: usize) -> usize {
    text.char_indices()
        .nth(char_index)
        .map(|(b, _)| b)
        .unwrap_or(text.len())
}

/// One colour per kind, so a name's origin reads at a glance: the project's own
/// words warm, the platform's members cool.
fn prompt_ac_color(kind: crate::prompt_complete::Kind) -> egui::Color32 {
    use crate::prompt_complete::Kind;
    match kind {
        Kind::Control => egui::Color32::from_rgb(120, 190, 255),
        Kind::DataItem => egui::Color32::from_rgb(220, 200, 120),
        Kind::Procedure => egui::Color32::from_rgb(200, 160, 240),
        Kind::Property => egui::Color32::from_rgb(140, 220, 160),
        Kind::Event => egui::Color32::from_rgb(240, 170, 130),
    }
}

fn apply_structural_prop(ctrl: &mut Control, key: &str, value: &PropValue) {
    let lower_key = key.to_ascii_lowercase();
    match lower_key.as_str() {
        "visible" => ctrl.visible = value.as_bool(),
        "enabled" => ctrl.enabled = value.as_bool(),
        "taborder" => ctrl.tab_order = value.as_i64() as u32,
        "zorder" => ctrl.z_order = value.as_i64() as i32,
        "x" => ctrl.rect.x = value.as_i64() as i32,
        "y" => ctrl.rect.y = value.as_i64() as i32,
        "width" => ctrl.rect.w = value.as_i64() as i32,
        "height" => ctrl.rect.h = value.as_i64() as i32,
        "parent" => {
            ctrl.parent = if value.as_str().is_empty() {
                None
            } else {
                Some(value.as_str().to_string())
            }
        }
        "tab" => ctrl.tab = Some(value.as_i64() as u32),
        _ => {
            let canonical = canonical_prop_key(ctrl, key);
            ctrl.properties.insert(canonical, value.clone());
        }
    }
}

/// Resolve a tab-page name (e.g. `"Tab1"`) to `(tab_control_id, tab_index)` by
/// searching every `TabControl` on the form. Returns `None` if no tab page with
/// that name is found.
fn resolve_tab_page(form: &cobolt_forms::Form, name: &str) -> Option<(String, u32)> {
    for ctrl in &form.controls {
        if ctrl.control_type != cobolt_forms::ControlType::TabControl {
            continue;
        }
        if let Some(tabs_prop) = ctrl.properties.get("Tabs") {
            let tabs_str = tabs_prop.as_str();
            for (i, tab_name) in tabs_str.split('\n').enumerate() {
                if tab_name.trim().eq_ignore_ascii_case(name) {
                    return Some((ctrl.id.clone(), i as u32));
                }
            }
        }
    }
    None
}

fn apply_agent_parent_target(
    form: &cobolt_forms::Form,
    ctrl: &mut cobolt_forms::Control,
    pid: &str,
) {
    let pid = pid.trim();
    if pid.is_empty() {
        ctrl.parent = None;
        return;
    }
    if let Some((tc_id, tab_idx)) = resolve_tab_page(form, pid) {
        ctrl.parent = Some(tc_id);
        if ctrl.tab.is_none() {
            ctrl.tab = Some(tab_idx);
        }
    } else {
        ctrl.parent = Some(pid.to_string());
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
    // Debug
    DebugForm,
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
    // A built binary for this form is being compiled or is running — the Run
    // button reads as engaged (toggled accent) for that whole stretch, so the
    // operator can see the IDE is busy on their behalf.
    run_busy: bool,
    fp_active: bool,
    inspector_on: bool,
    debug_active: bool,
    // When true, the Save button paints a checkmark (a transient "saved" cue)
    // instead of its normal icon.
    saved_flash: bool,
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

    // Colour palette (frozen white glass). This reads correctly against the
    // dark/glass chrome themes, but near-white icons wash out on light
    // backgrounds — Neumorphic Light in particular — so light themes fall
    // back to the theme's own high-contrast bright/dim text colours instead.
    let theme = crate::theme::active();
    let is_neumorphic = theme.is_neumorphic();
    let (col_normal, col_dim) = if theme.dark {
        (
            Color32::from_rgba_premultiplied(215, 225, 255, 210),
            Color32::from_rgba_premultiplied(215, 225, 255, 70),
        )
    } else {
        (
            theme.text_bright,
            Color32::from_rgba_unmultiplied(
                theme.text_dim.r(),
                theme.text_dim.g(),
                theme.text_dim.b(),
                110,
            ),
        )
    };
    let _col_active = Color32::from_rgba_premultiplied(130, 180, 255, 255);
    let col_accent = if theme.dark {
        Color32::from_rgba_premultiplied(255, 220, 100, 240) // gold for toggles
    } else if is_neumorphic {
        // The toggled fill below is a graphite badge (see `face`), not the
        // theme's blue accent — drawing the icon in that same blue would
        // repeat the blue-on-blue invisibility bug, so use white instead.
        Color32::WHITE
    } else {
        theme.accent
    };

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
        // Toggled wins over disabled: a button that is disabled *because* it is
        // currently engaged (e.g. Debug Form while a debug session is already
        // running) must still read clearly as "this is the active state" —
        // fading it to the dim/disabled colour would hide the one thing the
        // badge exists to communicate. Applies to every button that reaches
        // this closure (Toggle Grid, Toggle Glass, Run/Stop, Debug, Inspector,
        // Live Preview), so the high-contrast fix is uniform, not per-button.
        let col = if toggled {
            col_accent
        } else if !enabled {
            col_dim
        } else {
            col_normal
        };
        if is_neumorphic {
            // Discrete neumorphic 3D relief (dark shadow SE + light highlight
            // NW) painted BEFORE the flat surface fill, so only the soft
            // edges peek out — same technique as the toolbox buttons.
            crate::theme::paint_neumorphic_relief(&painter, resp.rect, 6.0, &theme);
            let face = if toggled {
                // Graphite, not the theme's blue accent — paired with the
                // white icon colour set above so the toggled icon stays
                // legible instead of disappearing into a same-hue fill.
                crate::theme::NEUMORPHIC_ACTIVE_GRAPHITE
            } else if resp.hovered() && enabled {
                theme.bg_hover
            } else {
                theme.bg_control
            };
            painter.rect_filled(resp.rect, 6.0, face);
        } else {
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
        }
        // Draw the icon into a shape buffer, then scale it to the common size.
        let mut shapes: Vec<Shape> = Vec::new();
        draw_fn(&mut shapes, icon_rect, col);
        normalize_icon(&mut shapes, icon_rect.center(), icon_ref_ext);
        painter.extend(shapes);
        if !tooltip.is_empty() {
            resp.clone().on_hover_text(tooltip);
        }
        // Brief accent flash acknowledging the click, uniform across toolbars.
        if enabled {
            crate::theme::flash_on_click(ui, &resp);
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
        // Right after a save, the button flashes a checkmark for ~1s (see
        // `saved_flash`) in place of its normal icon, then reverts. Request a
        // repaint while flashing so it reverts even if nothing else is animating.
        if saved_flash {
            ui.ctx()
                .request_repaint_after(std::time::Duration::from_millis(80));
        }
        let save_icon: &dyn Fn(&mut Vec<Shape>, Rect, Color32) =
            if saved_flash { &icon_check } else { &icon_save };
        if icon_btn(ui, true, false, "Save & Generate COBOL  (⌘S)", save_icon) {
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
        } else if run_busy {
            // Building or running the compiled binary: the Run button reads as
            // engaged — same accent treatment as a toggled Grid/Glass button.
            // Disabled (a second Run mid-build would queue a second build), and
            // "toggled wins over disabled" in icon_btn keeps it high-contrast.
            let _ = icon_btn(ui, false, true, "Run Form — building/running…", &icon_run);
        } else {
            if icon_btn(ui, true, false, "Run Form (live interpreter)", &icon_run) {
                action = DesignerToolbarAction::RunForm;
            }
        }
        // Debug button — starts a debug session for the generated COBOL.
        if icon_btn(
            ui,
            !form_running && !debug_active,
            debug_active,
            "Debug Form — step through generated COBOL with breakpoints",
            &icon_bug,
        ) {
            action = DesignerToolbarAction::DebugForm;
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
            "Format Painter — copy/paste control style. Hit ESCape key to stop pasting style",
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

/// Checkmark — the transient "form saved" cue shown on the Save button for ~1s.
fn icon_check(out: &mut Vec<Shape>, r: Rect, c: Color32) {
    let s = Stroke::new(2.2, c);
    let p0 = Pos2::new(r.min.x + r.width() * 0.20, r.min.y + r.height() * 0.55);
    let p1 = Pos2::new(r.min.x + r.width() * 0.42, r.min.y + r.height() * 0.75);
    let p2 = Pos2::new(r.min.x + r.width() * 0.80, r.min.y + r.height() * 0.28);
    out.push(Shape::line_segment([p0, p1], s));
    out.push(Shape::line_segment([p1, p2], s));
}

/// Save — a floppy disk carrying a download badge.
///
/// The disk occupies the upper-left of the cell and the badge sits in the
/// bottom-right corner, tangent to the disk's rounded corner rather than
/// overlapping it: an icon function is handed only the foreground colour, so it
/// has no background to punch a gap with, and crossing outlines read as noise at
/// this size. The old drawing put the arrow *inside* the disk, where it collided
/// with the label and looked like a scribble.
fn icon_save(out: &mut Vec<Shape>, r: Rect, c: Color32) {
    let s = Stroke::new(1.8, c);
    // 0.62 / 0.20 keeps the badge clear of the disk: with the badge centred at
    // `max - rad`, the disk's far corner is `sqrt(2)·(0.80 - rad)` away, which
    // has to stay above `rad`. At 0.70 / 0.21 the corner sat *inside* the
    // circle.
    let side = r.width().min(r.height()) * 0.62;
    let body = Rect::from_min_size(r.min, Vec2::splat(side));

    // The disk itself.
    out.push(Shape::rect_stroke(body, 3.0, s, egui::StrokeKind::Middle));

    // Shutter, and the hub slot inside it.
    let shutter = Rect::from_min_max(
        Pos2::new(body.min.x + side * 0.20, body.min.y),
        Pos2::new(body.min.x + side * 0.74, body.min.y + side * 0.34),
    );
    out.push(Shape::rect_stroke(shutter, 2.0, s, egui::StrokeKind::Middle));
    let slot = Rect::from_min_max(
        Pos2::new(body.min.x + side * 0.50, body.min.y + side * 0.07),
        Pos2::new(body.min.x + side * 0.63, body.min.y + side * 0.26),
    );
    out.push(Shape::rect_filled(slot, 1.0, c));

    // Label, open at the disk's bottom edge.
    let label = Rect::from_min_max(
        Pos2::new(body.min.x + side * 0.16, body.min.y + side * 0.54),
        Pos2::new(body.max.x - side * 0.16, body.max.y),
    );
    out.push(Shape::rect_stroke(label, 1.0, s, egui::StrokeKind::Middle));

    // Download badge.
    let rad = r.width() * 0.20;
    let ctr = Pos2::new(r.max.x - rad - 0.5, r.max.y - rad - 0.5);
    out.push(Shape::circle_stroke(ctr, rad, s));
    let tip = Pos2::new(ctr.x, ctr.y + rad * 0.50);
    out.push(Shape::line_segment(
        [Pos2::new(ctr.x, ctr.y - rad * 0.52), tip],
        s,
    ));
    out.push(Shape::line_segment(
        [tip, Pos2::new(ctr.x - rad * 0.38, ctr.y + rad * 0.06)],
        s,
    ));
    out.push(Shape::line_segment(
        [tip, Pos2::new(ctr.x + rad * 0.38, ctr.y + rad * 0.06)],
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
    out.push(Shape::rect_stroke(sr, 1.0, s, egui::StrokeKind::Middle));
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
        egui::StrokeKind::Middle,
    ));
    out.push(Shape::rect_stroke(front, 1.5, s, egui::StrokeKind::Middle));
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
    out.push(Shape::rect_stroke(board, 2.0, s, egui::StrokeKind::Middle));
    out.push(Shape::rect_filled(
        clip,
        2.0,
        Color32::from_rgba_premultiplied(c.r(), c.g(), c.b(), 35),
    ));
    out.push(Shape::rect_stroke(
        clip,
        2.0,
        Stroke::new(1.4, c),
        egui::StrokeKind::Middle,
    ));
    let page = Rect::from_min_max(
        board.min + egui::vec2(5.0, 7.0),
        board.max - egui::vec2(5.0, 4.0),
    );
    out.push(Shape::rect_stroke(
        page,
        1.0,
        Stroke::new(1.2, c),
        egui::StrokeKind::Middle,
    ));
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
    out.push(Shape::rect_stroke(body, 1.0, s, egui::StrokeKind::Middle));
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
        egui::StrokeKind::Middle,
    ));
    out.push(Shape::rect_filled(
        r1,
        1.0,
        Color32::from_rgba_premultiplied(c.r(), c.g(), c.b(), 40),
    ));
    out.push(Shape::rect_stroke(r1, 1.0, s, egui::StrokeKind::Middle));
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
        egui::StrokeKind::Middle,
    ));
    out.push(Shape::rect_stroke(r2, 1.0, s, egui::StrokeKind::Middle));
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
        egui::StrokeKind::Middle,
    ));
    out.push(Shape::rect_stroke(
        Rect::from_center_size(Pos2::new(cx + 1.0, cy - 1.0), Vec2::new(10.0, 8.0)),
        1.0,
        s,
        egui::StrokeKind::Middle,
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
        egui::StrokeKind::Middle,
    ));
    out.push(Shape::rect_stroke(
        Rect::from_center_size(Pos2::new(cx - 1.0, cy + 1.0), Vec2::new(10.0, 8.0)),
        1.0,
        s,
        egui::StrokeKind::Middle,
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
                egui::StrokeKind::Middle,
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
                egui::StrokeKind::Middle,
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
            egui::StrokeKind::Middle,
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
            egui::StrokeKind::Middle,
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
            egui::StrokeKind::Middle,
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
            egui::StrokeKind::Middle,
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
        egui::StrokeKind::Middle,
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
        egui::StrokeKind::Middle,
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
        egui::StrokeKind::Middle,
    ));
    let y2 = sr.min.y + 7.0;
    out.push(Shape::rect_stroke(
        Rect::from_min_size(Pos2::new(sr.min.x, y2), Vec2::new(sr.width() * 0.30, 4.5)),
        1.0,
        s,
        egui::StrokeKind::Middle,
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
        egui::StrokeKind::Middle,
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
mod save_icon_tests {
    use super::*;

    /// The download badge must not eat into the disk.
    ///
    /// An icon function gets only the foreground colour, so it cannot paint a
    /// gap behind the badge; if the circle reaches the disk the two outlines
    /// cross and the icon turns to mush at 30 px. The first cut of this drawing
    /// had exactly that bug — the disk's corner sat well inside the circle — and
    /// it is invisible in a build, so it is pinned here.
    #[test]
    fn the_badge_clears_the_disk() {
        let r = Rect::from_min_size(Pos2::ZERO, Vec2::splat(30.0));

        let side = r.width().min(r.height()) * 0.62;
        let body = Rect::from_min_size(r.min, Vec2::splat(side));
        let rad = r.width() * 0.20;
        let ctr = Pos2::new(r.max.x - rad - 0.5, r.max.y - rad - 0.5);

        // Nearest point of the (square) disk to the badge centre is its far
        // corner, since the centre lies beyond the disk on both axes.
        assert!(ctr.x > body.max.x && ctr.y > body.max.y);
        let d = ((ctr.x - body.max.x).powi(2) + (ctr.y - body.max.y).powi(2)).sqrt();
        assert!(
            d > rad,
            "the badge (r={rad:.2}) overlaps the disk: corner is {d:.2} away"
        );

        // And both stay inside the cell they were given.
        assert!(body.max.x <= r.max.x && body.max.y <= r.max.y);
        assert!(ctr.x + rad <= r.max.x && ctr.y + rad <= r.max.y);
    }

    /// The drawing emits the parts the icon is made of, and nothing is empty.
    #[test]
    fn the_icon_draws_disk_shutter_label_and_badge() {
        let mut out = Vec::new();
        icon_save(
            &mut out,
            Rect::from_min_size(Pos2::ZERO, Vec2::splat(30.0)),
            Color32::BLACK,
        );
        let circles = out
            .iter()
            .filter(|s| matches!(s, Shape::Circle(_)))
            .count();
        let rects = out.iter().filter(|s| matches!(s, Shape::Rect(_))).count();
        let lines = out
            .iter()
            .filter(|s| matches!(s, Shape::LineSegment { .. }))
            .count();
        assert_eq!(circles, 1, "one badge circle");
        assert_eq!(rects, 4, "disk, shutter, hub slot, label");
        assert_eq!(lines, 3, "the arrow: stem plus two barbs");
    }
}

#[cfg(test)]
mod shell_prop_tests {
    use super::*;
    use cobolt_forms::model::FormFormat;

    #[test]
    fn form_format_prop_round_trips_and_main_form_is_pinned_049() {
        // 049 R1/R5 — FormFormat is settable through the prop plumbing on an
        // ordinary form, and a no-op on the main form (pinned Standalone).
        let mut d = DesignerPanel::new(Form::new("FormA", "A", 640, 480));
        assert_eq!(d.get_form_prop("FormFormat").as_deref(), Some("Standalone"));
        d.set_form_prop("FormFormat", "Embedded".into());
        assert_eq!(d.get_form_prop("FormFormat").as_deref(), Some("Embedded"));
        d.set_form_prop("formformat", "Both".into()); // case-insensitive key
        assert_eq!(d.get_form_prop("FormFormat").as_deref(), Some("Both"));

        let mut main = DesignerPanel::new(Form::new("MAIN", "Main", 640, 480));
        main.form.main_form = true;
        main.set_form_prop("FormFormat", "Embedded".into());
        assert_eq!(
            main.get_form_prop("FormFormat").as_deref(),
            Some("Standalone"),
            "R5: the main form's format is pinned"
        );
        assert_eq!(main.form.form_format, FormFormat::Standalone);

        println!(
            "049 FormFormat plumbing — 3 transitions on a normal form \
             (Standalone→Embedded→Both, case-insensitive), main form pinned Standalone"
        );
    }

    /// 049 — the designer's per-frame sync is what makes a FullHeight
    /// SideMenu track the form: it follows a resize, and switching the
    /// property off hands the geometry back to the developer.
    #[test]
    fn side_menu_tracks_the_form_height_until_full_height_is_off_049() {
        let mut d = DesignerPanel::new(Form::new("MAIN", "Main", 640, 480));
        let mut side = Control::new("SIDE-1", ControlType::SideMenu, 0, 0);
        side.rect.w = 200;
        side.rect.y = 40;
        side.rect.h = 400;
        d.form.controls.push(side);

        // What `show` runs at the top of every frame.
        d.form.sync_side_menu_full_height();
        let c = d.form.find_control("SIDE-1").unwrap();
        assert_eq!((c.rect.y, c.rect.h), (0, 480), "pinned to the form");
        assert_eq!(c.rect.w, 200, "the width is left alone");

        // A form resize carries the sidebar with it.
        d.form.height = 900;
        d.form.sync_side_menu_full_height();
        assert_eq!(d.form.find_control("SIDE-1").unwrap().rect.h, 900);

        // FullHeight off ⇒ the developer's numbers stand, resize or not.
        d.form
            .find_control_mut("SIDE-1")
            .unwrap()
            .set_prop("FullHeight", false);
        let c = d.form.find_control_mut("SIDE-1").unwrap();
        c.rect.y = 60;
        c.rect.h = 250;
        d.form.height = 1000;
        d.form.sync_side_menu_full_height();
        let c = d.form.find_control("SIDE-1").unwrap();
        assert_eq!(
            (c.rect.y, c.rect.h),
            (60, 250),
            "FullHeight off leaves the placed geometry untouched"
        );

        println!(
            "049 FullHeight (designer) — on: pinned y=0 h=480 then followed a \
             resize to h=900 (width 200 untouched); off: kept the placed \
             y=60 h=250 across a resize to 1000 (3/3)"
        );
    }

    #[test]
    fn menu_pane_background_props_materialise_and_clear_049() {
        // 049 R39 — the 9 MenuPane* keys: MenuPaneCustom materialises/clears
        // the group; each field key reads back what it wrote.
        let mut d = DesignerPanel::new(Form::new("MAIN", "Main", 640, 480));
        assert_eq!(d.get_form_prop("MenuPaneCustom").as_deref(), Some("false"));

        d.set_form_prop("MenuPaneCustom", "true".into());
        assert_eq!(d.get_form_prop("MenuPaneCustom").as_deref(), Some("true"));

        let cases: &[(&str, &str)] = &[
            ("MenuPaneColor", "#123456FF"),
            ("MenuPaneGradientEnabled", "true"),
            ("MenuPaneGradientStartColor", "#111111"),
            ("MenuPaneGradientEndColor", "#222222"),
            ("MenuPaneGradientDirection", "East"),
            ("MenuPaneTransparency", "40"),
            ("MenuPaneImage", "assets/rail.png"),
            ("MenuPaneImageMode", "Tile"),
        ];
        for (key, value) in cases {
            d.set_form_prop(key, (*value).into());
            assert_eq!(
                d.get_form_prop(key).as_deref(),
                Some(*value),
                "{key} did not round-trip"
            );
        }

        d.set_form_prop("MenuPaneCustom", "false".into());
        assert!(d.form.menu_pane_background.is_none(), "cleared to None");
        assert_eq!(d.get_form_prop("MenuPaneCustom").as_deref(), Some("false"));

        println!(
            "049 MenuPane background plumbing — materialise + {} field keys \
             round-trip + clear back to None",
            cases.len()
        );
    }
}

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
        let mut out = ctx.run_ui(raw, |root_ui| {
            let ctx = root_ui.ctx().clone();
            let ctx = &ctx;
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
        out.textures_delta.clear();
        out.shapes.into_iter().find_map(|cs| match cs.shape {
            egui::Shape::Mesh(m) => Some(m.texture_id),
            egui::Shape::Rect(r) if r.fill_texture_id() != egui::TextureId::default() => {
                Some(r.fill_texture_id())
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
        let mut out = ctx.run_ui(egui::RawInput::default(), |root_ui| {
            let ctx = root_ui.ctx().clone();
            let ctx = &ctx;
            // Frame::none → the panel paints no background, so captured shapes are
            // exactly what `draw_control` emitted (no full-panel fill skewing bbox).
            egui::CentralPanel::default()
                .frame(egui::Frame::NONE)
                .show(root_ui, |ui| {
                    let painter = ui.painter().clone();
                    draw_control(&painter, origin, ctrl, false, false, 1.0, 1.0, None);
                });
        });
        out.textures_delta.clear();
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

    // ── Text-bearing control font-style properties (LayoutJob format) ─────────
    fn styled_control_with(control_type: ControlType, prop: &str) -> egui::epaint::TextShape {
        let is_textbox = matches!(control_type, ControlType::TextBox);
        let mut c = Control::new("TXT", control_type, 5, 7);
        if is_textbox {
            c.set_prop("Text", PropValue::String("STYLE-RC".into()));
        } else {
            c.set_prop("Caption", PropValue::String("STYLE-RC".into()));
        }
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
            styled_control_with(ControlType::Label, "Italic")
                .galley
                .job
                .sections
                .iter()
                .any(|s| s.format.italics),
            "Italic not applied"
        );
        assert!(
            styled_control_with(ControlType::Label, "Underline")
                .galley
                .job
                .sections
                .iter()
                .any(|s| s.format.underline.width > 0.0),
            "Underline not applied"
        );
        assert!(
            styled_control_with(ControlType::Label, "Strikethrough")
                .galley
                .job
                .sections
                .iter()
                .any(|s| s.format.strikethrough.width > 0.0),
            "Strikethrough not applied"
        );
        // Sanity: a plain label has none of them.
        let plain = styled_control_with(ControlType::Label, "");
        assert!(
            plain.galley.job.sections.iter().all(|s| !s.format.italics),
            "plain label unexpectedly italic"
        );
    }

    #[test]
    fn button_and_textbox_font_styles_apply() {
        for control_type in [ControlType::Button, ControlType::TextBox] {
            assert!(
                styled_control_with(control_type.clone(), "Italic")
                    .galley
                    .job
                    .sections
                    .iter()
                    .any(|s| s.format.italics),
                "{control_type:?} italic not applied"
            );
            assert!(
                styled_control_with(control_type.clone(), "Underline")
                    .galley
                    .job
                    .sections
                    .iter()
                    .any(|s| s.format.underline.width > 0.0),
                "{control_type:?} underline not applied"
            );
            assert!(
                styled_control_with(control_type.clone(), "Strikethrough")
                    .galley
                    .job
                    .sections
                    .iter()
                    .any(|s| s.format.strikethrough.width > 0.0),
                "{control_type:?} strikethrough not applied"
            );
        }
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
    fn button_and_textbox_bold_paints_extra_glyph_pass() {
        for control_type in [ControlType::Button, ControlType::TextBox] {
            let is_textbox = matches!(control_type, ControlType::TextBox);
            let mut plain = Control::new("TXT", control_type.clone(), 5, 7);
            if is_textbox {
                plain.set_prop("Text", PropValue::String("BOLD-RC".into()));
            } else {
                plain.set_prop("Caption", PropValue::String("BOLD-RC".into()));
            }
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
                "{control_type:?} bold did not add an extra paint pass \
                 (plain={n_plain}, bold={n_bold})"
            );
        }
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
    fn format_painter_click_toggles_persistent_mode() {
        let mut designer = DesignerPanel::new(Form::new("FormA", "A", 640, 480));
        designer
            .form
            .controls
            .push(Control::new("Button-1", ControlType::Button, 10, 20));
        designer.selected_ids = vec!["Button-1".to_owned()];

        designer.toggle_format_painter();
        match &designer.format_painter {
            FormatPainter::WaitingForTarget { .. } => {}
            _ => panic!("click should capture a persistent format painter"),
        }

        designer.toggle_format_painter();
        assert!(matches!(designer.format_painter, FormatPainter::Idle));
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
        // Default control font is 14pt since the 1.30.x control-defaults
        // update (operator decision 2026-07-16: keep 14).
        assert_eq!(font_of(&d, &first), ("Arial".to_string(), 14));
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

    /// A second workflow task that re-emits the SAME deploy set (the trace that
    /// produced 32 textboxes for a 15-textbox request) must land on the controls
    /// that already carry those ids, not clone them under auto ids.
    #[test]
    fn redeploying_an_existing_id_updates_that_control() {
        use crate::agent::{AgentChangeSet, AgentOp};
        let deploy = |id: &str, x: i64, colour: &str| {
            let mut props = serde_json::Map::new();
            props.insert("X".into(), serde_json::json!(x));
            props.insert("Y".into(), serde_json::json!(50));
            props.insert("ForegroundColor".into(), serde_json::json!(colour));
            AgentOp::DeployControl {
                control_type: "TextBox".into(),
                id: Some(id.into()),
                parent_id: None,
                parent: None,
                properties: props,
            }
        };
        let mut d = DesignerPanel::new(Form::new("F", "T", 640, 480));
        let first = AgentChangeSet {
            operations: vec![deploy("TXT-1", 50, "#000000"), deploy("TXT-2", 400, "#0000FF")],
            note: None,
        };
        d.apply_agent_change_set(&first);
        assert_eq!(d.form.controls.len(), 2);

        // Task 3 "validates" by re-emitting the whole set with its own layout.
        let again = AgentChangeSet {
            operations: vec![deploy("TXT-1", 70, "#FF0000"), deploy("TXT-2", 370, "#0000FF")],
            note: None,
        };
        d.apply_agent_change_set(&again);
        assert_eq!(d.form.controls.len(), 2, "no clones: the ids were reused");
        let t1 = d.form.find_control("TXT-1").unwrap();
        assert_eq!(t1.rect.x, 72, "the redeploy moved the control it named");
        assert_eq!(
            t1.get_prop("ForegroundColor").unwrap().as_str(),
            "#FF0000",
            "and set its properties"
        );
    }

    /// Same id twice inside ONE change-set (a merged correction round): the
    /// second deploy folds into the control the first one is still staging.
    #[test]
    fn a_repeated_id_within_one_change_set_folds_into_one_control() {
        use crate::agent::{AgentChangeSet, AgentOp};
        let deploy = |id: &str, w: i64| {
            let mut props = serde_json::Map::new();
            props.insert("Width".into(), serde_json::json!(w));
            AgentOp::DeployControl {
                control_type: "TextBox".into(),
                id: Some(id.into()),
                parent_id: None,
                parent: None,
                properties: props,
            }
        };
        let mut d = DesignerPanel::new(Form::new("F", "T", 640, 480));
        d.apply_agent_change_set(&AgentChangeSet {
            operations: vec![deploy("TXT-1", 300), deploy("TXT-1", 250)],
            note: None,
        });
        assert_eq!(d.form.controls.len(), 1);
        assert_eq!(d.form.find_control("TXT-1").unwrap().rect.w, 250);
    }

    /// The id collision that is NOT a redeploy — a different control type —
    /// still gets a fresh auto id rather than silently retyping the control.
    #[test]
    fn a_colliding_id_of_another_type_still_gets_a_new_id() {
        use crate::agent::{AgentChangeSet, AgentOp};
        let mut d = DesignerPanel::new(Form::new("F", "T", 640, 480));
        d.form
            .controls
            .push(Control::new("TXT-1", ControlType::TextBox, 0, 0));
        d.apply_agent_change_set(&AgentChangeSet {
            operations: vec![AgentOp::DeployControl {
                control_type: "Button".into(),
                id: Some("TXT-1".into()),
                parent_id: None,
                parent: None,
                properties: serde_json::Map::new(),
            }],
            note: None,
        });
        assert_eq!(d.form.controls.len(), 2);
        assert_eq!(
            d.form.find_control("TXT-1").unwrap().control_type,
            ControlType::TextBox,
            "the original control kept its type"
        );
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
                    parent_id: None,
                    parent: None,
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
                    code: "       ENVIRONMENT DIVISION.\n       DATA DIVISION.\n       PROCEDURE DIVISION.\n           DISPLAY \"hi\".\n".into(),
                },
                AgentOp::CreateProcedure {
                    name: "VALIDATE-INPUT".into(),
                    code: "       ENVIRONMENT DIVISION.\n       DATA DIVISION.\n       PROCEDURE DIVISION.\n           CONTINUE.\n".into(),
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

        // One undo reverts the entire change-set (R6). This batch touches
        // procedure code, so the step is held for the developer's
        // confirmation first (operator, 2026-07-29).
        d.undo();
        assert!(
            d.pending_history_confirm.is_some(),
            "a procedure-touching batch asks before undoing"
        );
        d.confirm_pending_history(true);
        assert_eq!(
            format!("{:?}", d.form),
            before,
            "single undo restores the pre-change form byte-for-byte"
        );

        // Redo re-applies it (after the same confirmation).
        d.redo();
        d.confirm_pending_history(true);
        assert!(d.form.find_control("SAVE").is_some(), "redo re-applies");
    }

    /// End to end through the real applier: a column the agent placed off-grid
    /// lands on the grid AND stays a column. Snapping each coordinate on its
    /// own would put `X=19` on 16 and `X=21` on 24 — the column the developer
    /// asked for, 8px crooked (operator, 2026-08-01).
    #[test]
    fn agent_placed_columns_are_snapped_without_losing_their_alignment() {
        use crate::agent::{AgentChangeSet, AgentOp};
        let mut d = DesignerPanel::new(Form::new("F", "T", 640, 480));
        assert!(d.form.snap_to_grid, "the grid is on by default");
        let g = d.form.grid_size as i32;

        let deploy = |id: &str, x: i32, y: i32| AgentOp::DeployControl {
            control_type: "CheckBox".into(),
            id: Some(id.into()),
            parent_id: None,
            parent: None,
            properties: serde_json::json!({ "X": x, "Y": y, "Width": 150 })
                .as_object()
                .cloned()
                .unwrap(),
        };
        let cs = AgentChangeSet {
            operations: vec![
                deploy("CHK-1", 19, 20),
                deploy("CHK-2", 21, 50),
                deploy("CHK-3", 20, 80),
            ],
            note: None,
        };
        d.apply_agent_change_set(&cs);

        let xs: Vec<i32> = ["CHK-1", "CHK-2", "CHK-3"]
            .iter()
            .map(|id| d.form.find_control(id).expect("deployed").rect.x)
            .collect();
        assert_eq!(xs[0] % g, 0, "the first control opens the lane on the grid");
        assert!(
            xs.iter().all(|x| *x == xs[0]),
            "the column must survive snapping: {xs:?}"
        );
        // Rows are translated as a run, so the 30px pitch the agent asked for
        // survives to the pixel and only the first row sits on a grid point.
        let ys: Vec<i32> = ["CHK-1", "CHK-2", "CHK-3"]
            .iter()
            .map(|id| d.form.find_control(id).expect("deployed").rect.y)
            .collect();
        assert_eq!(ys[0] % g, 0, "the first row lands on the grid");
        assert_eq!(ys[1] - ys[0], 30, "row pitch preserved: {ys:?}");
        assert_eq!(ys[2] - ys[1], 30, "row pitch preserved: {ys:?}");
    }

    /// The gap this closes: `DECIMAL-POINT IS COMMA` is reserved to the
    /// outermost program, and no operation could write it — so a request for
    /// comma currency was unsatisfiable and the handlers' comma literals could
    /// never parse (operator, 2026-08-02). It must reach generated COBOL, and
    /// it must undo.
    #[test]
    fn set_form_structure_reaches_generated_cobol_and_undoes() {
        use crate::agent::{AgentChangeSet, AgentOp};
        let mut d = DesignerPanel::new(Form::new("F", "T", 640, 480));
        assert!(
            !cobolt_codegen::generate(&d.form).contains("DECIMAL-POINT IS COMMA"),
            "precondition: the clause is not there yet"
        );

        let cs = AgentChangeSet {
            operations: vec![
                AgentOp::SetFormStructure {
                    block: "SPECIAL-NAMES".into(),
                    code: "       DECIMAL-POINT IS COMMA.".into(),
                },
                AgentOp::SetFormStructure {
                    block: "WORKING-STORAGE".into(),
                    code: "       01  WS-TOTAL-PRICE  PIC 9(5)V99 GLOBAL VALUE 0.".into(),
                },
            ],
            note: None,
        };
        assert_eq!(d.apply_agent_change_set(&cs), 2);

        let src = cobolt_codegen::generate(&d.form);
        assert!(src.contains("SPECIAL-NAMES."), "section header is codegen's");
        assert!(src.contains("DECIMAL-POINT IS COMMA."));
        assert!(src.contains("WS-TOTAL-PRICE"));
        assert!(src.contains("GLOBAL"), "the GLOBAL clause survives verbatim");

        // One change-set is one undoable step (spec 025 R6).
        d.undo();
        assert!(d.form.cobol_structure.special_names.trim().is_empty());
        assert!(d.form.user_ws_source.trim().is_empty());
        assert!(!cobolt_codegen::generate(&d.form).contains("DECIMAL-POINT IS COMMA"));
    }

    /// With the grid switched off nothing quantises, so the agent's own
    /// placement stands exactly as written.
    #[test]
    fn a_disabled_grid_leaves_agent_placement_alone() {
        use crate::agent::{AgentChangeSet, AgentOp};
        let mut d = DesignerPanel::new(Form::new("F", "T", 640, 480));
        d.form.snap_to_grid = false;
        d.form
            .controls
            .push(Control::new("L1", ControlType::Label, 0, 0));
        let cs = AgentChangeSet {
            operations: vec![
                AgentOp::SetProperty {
                    control_id: "L1".into(),
                    key: "X".into(),
                    value: serde_json::json!(19),
                },
                AgentOp::SetProperty {
                    control_id: "L1".into(),
                    key: "Y".into(),
                    value: serde_json::json!(13),
                },
            ],
            note: None,
        };
        d.apply_agent_change_set(&cs);
        let c = d.form.find_control("L1").unwrap();
        assert_eq!((c.rect.x, c.rect.y), (19, 13));
    }

    #[test]
    fn moving_a_container_carries_its_children() {
        use crate::agent::{AgentChangeSet, AgentOp};
        let mut d = DesignerPanel::new(Form::new("F", "T", 640, 480));
        // A Panel at (10,10) holding a Button child at (20,20) and a Label
        // grandchild-less sibling outside the panel that must NOT move.
        let mut panel = Control::new("P1", ControlType::Panel, 10, 10);
        panel.rect.w = 300;
        panel.rect.h = 300;
        let mut child = Control::new("C1", ControlType::Button, 20, 20);
        child.parent = Some("P1".into());
        let outside = Control::new("O1", ControlType::Label, 400, 400);
        d.form.controls.push(panel);
        d.form.controls.push(child);
        d.form.controls.push(outside);

        // Agent repositions the panel to (60,40): dx=50, dy=30.
        let cs = AgentChangeSet {
            operations: vec![
                AgentOp::SetProperty {
                    control_id: "P1".into(),
                    key: "X".into(),
                    value: serde_json::json!(60),
                },
                AgentOp::SetProperty {
                    control_id: "P1".into(),
                    key: "Y".into(),
                    value: serde_json::json!(40),
                },
            ],
            note: None,
        };
        d.apply_agent_change_set(&cs);

        let pos = |d: &DesignerPanel, id: &str| {
            d.form.find_control(id).map(|c| (c.rect.x, c.rect.y))
        };
        // The agent asked for (60,40); 60 is not on the 8px grid, so the panel
        // lands on the nearest point (64) and the carry delta is (54,30).
        assert_eq!(pos(&d, "P1"), Some((64, 40)), "panel moved to target");
        assert_eq!(
            pos(&d, "C1"),
            Some((74, 50)),
            "child carried by the same (54,30) delta, keeping its place inside"
        );
        assert_eq!(
            pos(&d, "O1"),
            Some((400, 400)),
            "unrelated control outside the container stays put"
        );

        // The whole motion — container and child — reverts in ONE undo.
        d.undo();
        assert_eq!(pos(&d, "P1"), Some((10, 10)), "container restored");
        assert_eq!(
            pos(&d, "C1"),
            Some((20, 20)),
            "child restored with the container in a single undo"
        );
    }

    #[test]
    fn moving_a_container_leaves_an_explicitly_placed_child_alone() {
        use crate::agent::{AgentChangeSet, AgentOp};
        let mut d = DesignerPanel::new(Form::new("F", "T", 640, 480));
        let mut panel = Control::new("P1", ControlType::Panel, 0, 0);
        panel.rect.w = 300;
        panel.rect.h = 300;
        let mut child = Control::new("C1", ControlType::Button, 10, 10);
        child.parent = Some("P1".into());
        d.form.controls.push(panel);
        d.form.controls.push(child);

        // Panel moves by (100,0); the child is ALSO explicitly placed by the
        // same change-set, so it must land at its explicit spot, not be shifted.
        let cs = AgentChangeSet {
            operations: vec![
                AgentOp::SetProperty {
                    control_id: "P1".into(),
                    key: "X".into(),
                    value: serde_json::json!(100),
                },
                AgentOp::SetProperty {
                    control_id: "C1".into(),
                    key: "X".into(),
                    value: serde_json::json!(5),
                },
                AgentOp::SetProperty {
                    control_id: "C1".into(),
                    key: "Y".into(),
                    value: serde_json::json!(5),
                },
            ],
            note: None,
        };
        d.apply_agent_change_set(&cs);
        let c = d.form.find_control("C1").unwrap();
        assert_eq!(
            (c.rect.x, c.rect.y),
            (9, 8),
            "explicitly repositioned child is not double-shifted by its container"
        );
    }

    #[test]
    fn nearest_free_offset_nudges_off_an_overlap() {
        // Moving box overlaps an obstacle directly below-right; the search prefers
        // "down", finding the smallest clear offset.
        let moving = (0, 0, 20, 20);
        let obstacles = [(10, 10, 20, 20)];
        let (dx, dy) = nearest_free_offset(moving, &obstacles, None, 200);
        assert!((dx, dy) != (0, 0), "an offset is found");
        let cleared = (moving.0 + dx, moving.1 + dy, moving.2, moving.3);
        assert!(
            !rects_overlap(cleared, obstacles[0]),
            "the chosen offset clears the overlap"
        );
    }

    #[test]
    fn nearest_free_offset_is_zero_when_already_clear() {
        assert_eq!(
            nearest_free_offset((0, 0, 10, 10), &[(100, 100, 10, 10)], None, 200),
            (0, 0)
        );
    }

    #[test]
    fn nearest_free_offset_respects_form_bounds() {
        // An obstacle that blankets the whole form: no in-bounds spot clears it,
        // so the moved control is left where it is rather than shoved off-canvas.
        let moving = (0, 0, 40, 40);
        let obstacles = [(-100, -100, 300, 300)];
        assert_eq!(
            nearest_free_offset(moving, &obstacles, Some((100, 100)), 200),
            (0, 0),
            "no in-bounds spot clears it → no nudge"
        );
    }

    #[test]
    fn agent_move_onto_a_sibling_is_nudged_off() {
        use crate::agent::{AgentChangeSet, AgentOp};
        let mut d = DesignerPanel::new(Form::new("F", "T", 640, 480));
        // Two top-level buttons well apart.
        d.form
            .controls
            .push(Control::new("A", ControlType::Button, 10, 10));
        d.form
            .controls
            .push(Control::new("B", ControlType::Button, 300, 300));

        // Agent drops B right on top of A.
        let cs = AgentChangeSet {
            operations: vec![
                AgentOp::SetProperty {
                    control_id: "B".into(),
                    key: "X".into(),
                    value: serde_json::json!(10),
                },
                AgentOp::SetProperty {
                    control_id: "B".into(),
                    key: "Y".into(),
                    value: serde_json::json!(10),
                },
            ],
            note: None,
        };
        d.apply_agent_change_set(&cs);

        let a = d.form.find_control("A").unwrap();
        let b = d.form.find_control("B").unwrap();
        let ar = (a.rect.x, a.rect.y, a.rect.w, a.rect.h);
        let br = (b.rect.x, b.rect.y, b.rect.w, b.rect.h);
        assert!(
            !rects_overlap(ar, br),
            "B was nudged so it no longer overlaps A ({ar:?} vs {br:?})"
        );
        assert_eq!(
            (a.rect.x, a.rect.y),
            (10, 10),
            "the obstacle A stayed put — only the moved control shifted"
        );
    }

    #[test]
    fn agent_batch_layout_trusts_coordinates_over_the_nudge() {
        use crate::agent::{AgentChangeSet, AgentOp};
        // A deliberate grid of two charts; the second slot lands on a leftover,
        // untouched Label. The batch (≥2 explicitly placed controls) must be
        // trusted verbatim — no chart is nudged off the label. Regression for the
        // "all but one aligned" scatter.
        let mut d = DesignerPanel::new(Form::new("F", "T", 1700, 2000));
        d.form
            .controls
            .push(Control::new("BarChart-1", ControlType::BarChart, 0, 0));
        d.form
            .controls
            .push(Control::new("AreaChart-1", ControlType::AreaChart, 0, 0));
        // Stray label sitting inside AreaChart-1's target slot (656,868 320x220).
        d.form
            .controls
            .push(Control::new("Label-6", ControlType::Label, 672, 1000));

        let place = |id: &str, x: i32, y: i32| -> Vec<AgentOp> {
            [("X", x), ("Y", y), ("Width", 320), ("Height", 220)]
                .into_iter()
                .map(|(k, v)| AgentOp::SetProperty {
                    control_id: id.to_string(),
                    key: k.to_string(),
                    value: serde_json::json!(v),
                })
                .collect()
        };
        let mut operations = place("BarChart-1", 656, 624);
        operations.extend(place("AreaChart-1", 656, 868));
        let cs = AgentChangeSet {
            operations,
            note: None,
        };
        d.apply_agent_change_set(&cs);

        let bar = d.form.find_control("BarChart-1").unwrap();
        let area = d.form.find_control("AreaChart-1").unwrap();
        assert_eq!(
            (bar.rect.x, bar.rect.y),
            (656, 624),
            "row-1 chart landed on its computed slot"
        );
        assert_eq!(
            (area.rect.x, area.rect.y),
            (656, 868),
            "row-2 chart stayed on its computed slot despite the stray label under it"
        );
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
                    parent_id: None,
                    parent: None,
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
        assert_eq!(
            d.undo_stack.len(),
            1,
            "approve is one AgentBatch = one Undo"
        );
        assert!(d.form.find_control("B").is_some());
    }

    /// Operator, 2026-07-28: "any change in the form should be possible to
    /// undo. If I change the theme I cannot undo it." A plain form property
    /// undoes to its previous value, and a GlassStyle switch — which bulldozes
    /// appearance defaults across every control — undoes to the exact
    /// pre-switch appearance, user-chosen control colours included.
    #[test]
    fn form_prop_and_glass_style_changes_are_undoable() {
        use cobolt_forms::GlassStyle;
        let mut d = DesignerPanel::new(Form::new("F", "T", 640, 480));
        d.form
            .controls
            .push(Control::new("L1", ControlType::Label, 10, 10));

        // Plain form property.
        d.set_form_prop("Title", "New title".into());
        assert_eq!(d.form.title, "New title");
        d.undo();
        assert_eq!(d.form.title, "T", "form Title undoes");

        // Style switch: Neumorphic Dark forces every foreground to white…
        d.set_property("L1", "ForegroundColor", PropValue::String("#FF0000".into()));
        d.set_form_prop("GlassStyle", "Neumorphic Dark".into());
        assert_eq!(d.form.glass_style, GlassStyle::NeumorphicDark);
        assert_eq!(
            d.form
                .find_control("L1")
                .unwrap()
                .get_prop("ForegroundColor")
                .unwrap()
                .as_str(),
            "#FFFFFFFF",
            "dark style bulldozed the label foreground"
        );
        // …and ONE undo restores both the style and the user's red.
        d.undo();
        assert_eq!(d.form.glass_style, GlassStyle::Classic, "style undoes");
        assert_eq!(
            d.form
                .find_control("L1")
                .unwrap()
                .get_prop("ForegroundColor")
                .unwrap()
                .as_str(),
            "#FF0000",
            "the user-chosen foreground survives the style-switch undo"
        );
        // Redo re-applies the full switch.
        d.redo();
        assert_eq!(d.form.glass_style, GlassStyle::NeumorphicDark);

        // A no-op set (same value) pushes nothing.
        let depth = d.undo_stack.len();
        d.set_form_prop("GlassStyle", "Neumorphic Dark".into());
        assert_eq!(d.undo_stack.len(), depth, "same-style set is a no-op");
    }

    /// Visible / Enabled / TabOrder mutated struct fields directly and were
    /// invisible to undo (audit, 2026-07-28) — they ride the stack now.
    #[test]
    fn visible_enabled_taborder_are_undoable() {
        let mut d = DesignerPanel::new(Form::new("F", "T", 640, 480));
        d.form
            .controls
            .push(Control::new("B1", ControlType::Button, 0, 0));
        d.set_property("B1", "Visible", PropValue::Bool(false));
        d.set_property("B1", "Enabled", PropValue::Bool(false));
        d.set_property("B1", "TabOrder", PropValue::Int(7));
        let c = d.form.find_control("B1").unwrap();
        assert!(!c.visible && !c.enabled);
        assert_eq!(c.tab_order, 7);
        d.undo();
        d.undo();
        d.undo();
        let c = d.form.find_control("B1").unwrap();
        assert!(c.visible, "Visible undoes");
        assert!(c.enabled, "Enabled undoes");
        assert_eq!(c.tab_order, 0, "TabOrder undoes");
    }

    /// Audit 2026-07-29: animation add/remove/field edits and data-binding
    /// application returned before the undo stack — they ride it now, with
    /// redo.
    #[test]
    fn animations_and_data_bindings_are_undoable() {
        use cobolt_forms::{BindingSourceDescriptor, BindingTargetDescriptor, DataBindingDef};
        let mut d = DesignerPanel::new(Form::new("F", "T", 640, 480));
        d.form
            .controls
            .push(Control::new("B1", ControlType::Button, 0, 0));

        // Add an animation, edit a field, then walk the history back and forth.
        d.set_property("B1", "_AddAnimation", PropValue::String("fly".into()));
        assert_eq!(d.form.find_control("B1").unwrap().animations.len(), 1);
        let default_ms = d.form.find_control("B1").unwrap().animations[0].duration_ms;
        d.set_property("B1", "Anim0_Duration", PropValue::Int(900));
        assert_eq!(
            d.form.find_control("B1").unwrap().animations[0].duration_ms,
            900
        );
        d.undo();
        assert_eq!(
            d.form.find_control("B1").unwrap().animations[0].duration_ms,
            default_ms,
            "field edit undoes"
        );
        d.undo();
        assert!(
            d.form.find_control("B1").unwrap().animations.is_empty(),
            "add undoes"
        );
        d.redo();
        d.redo();
        assert_eq!(
            d.form.find_control("B1").unwrap().animations[0].duration_ms,
            900,
            "redo replays add + edit"
        );
        d.set_property("B1", "_RemoveAnim0", PropValue::Bool(true));
        assert!(d.form.find_control("B1").unwrap().animations.is_empty());
        d.undo();
        assert_eq!(
            d.form.find_control("B1").unwrap().animations.len(),
            1,
            "remove undoes"
        );

        // Data binding: one undoable step, snapshot-based.
        d.form
            .controls
            .push(Control::new("LB", ControlType::ListBox, 0, 100));
        let binding = DataBindingDef::new(
            "b1",
            "B",
            BindingSourceDescriptor::IndexedFile {
                definition_path: "x.cidx".into(),
                record_name: "R".into(),
                fields: Vec::new(),
                key_field: None,
                writable: false,
            },
            BindingTargetDescriptor::ListBox {
                control_id: "LB".into(),
            },
        );
        d.apply_data_binding(binding);
        assert_eq!(d.form.data_bindings.len(), 1);
        d.undo();
        assert!(d.form.data_bindings.is_empty(), "binding undoes");
        d.redo();
        assert_eq!(d.form.data_bindings.len(), 1, "binding redoes");
    }

    /// Operator, 2026-07-29: undo/redo of a step that changes COBOL procedure
    /// code must be confirmed by the developer first.
    #[test]
    fn procedure_history_requires_confirmation() {
        let mut d = DesignerPanel::new(Form::new("F", "T", 640, 480));
        d.add_user_procedure();
        assert_eq!(d.form.user_procedures.len(), 1);

        // Ctrl+Z holds the step for confirmation — nothing moves yet, and
        // further presses do nothing.
        d.undo();
        assert!(d.pending_history_confirm.is_some());
        assert_eq!(d.form.user_procedures.len(), 1);
        d.undo();
        assert_eq!(d.form.user_procedures.len(), 1);

        // Declining drops the request.
        d.confirm_pending_history(false);
        assert!(d.pending_history_confirm.is_none());
        assert_eq!(d.form.user_procedures.len(), 1);

        // Accepting performs the held undo.
        d.undo();
        d.confirm_pending_history(true);
        assert!(d.form.user_procedures.is_empty());

        // Redo asks the same question.
        d.redo();
        assert!(d.pending_history_confirm.is_some());
        d.confirm_pending_history(true);
        assert_eq!(d.form.user_procedures.len(), 1);

        // Deleting a procedure with code and undoing restores it verbatim.
        d.form.user_procedures[0].code = "       PROCEDURE DIVISION.".into();
        d.remove_user_procedure(0);
        assert!(d.form.user_procedures.is_empty());
        d.undo();
        d.confirm_pending_history(true);
        assert_eq!(
            d.form.user_procedures[0].code,
            "       PROCEDURE DIVISION."
        );
    }

    /// Audit 2026-07-29: a MenuBar definition save rewrites a YAML next to
    /// the .cfrm — undo restores the previous file (or removes one that did
    /// not exist), redo rewrites it.
    #[test]
    fn menu_definition_save_is_undoable() {
        let mut d = DesignerPanel::new(Form::new("F", "T", 640, 480));
        let dir = std::env::temp_dir().join(format!("prc-menu-undo-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        d.cfrm_dir = Some(dir.clone());
        let path = cobolt_forms::menu::menu_yaml_path(&dir, "MENU-1");

        d.set_menu_definition("MENU-1".into(), cobolt_forms::menu::MenuDefinition::default());
        assert!(path.exists(), "menu YAML written");
        d.undo();
        assert!(!path.exists(), "undo removes a menu that did not exist before");
        d.redo();
        assert!(path.exists(), "redo rewrites it");
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn agent_deploy_control_resolves_tab_page_parent() {
        use crate::agent::{AgentChangeSet, AgentOp};

        let mut form = Form::new("F", "T", 640, 480);
        let mut tabs = Control::new("TabControl-1", ControlType::TabControl, 100, 100);
        tabs.properties
            .insert("Tabs".into(), PropValue::String("Tab1\nTab2".into()));
        form.controls.push(tabs);
        let mut d = DesignerPanel::new(form);

        let mut props = serde_json::Map::new();
        props.insert("Caption".into(), serde_json::json!("Nuevo"));
        props.insert("X".into(), serde_json::json!(120));
        props.insert("Y".into(), serde_json::json!(150));
        props.insert("Parent".into(), serde_json::json!("Tab1"));

        let cs = AgentChangeSet {
            operations: vec![AgentOp::DeployControl {
                control_type: "Button".into(),
                id: Some("Button-1".into()),
                parent_id: None,
                parent: None,
                properties: props,
            }],
            note: None,
        };

        assert_eq!(d.apply_agent_change_set(&cs), 1);
        let button = d.form.find_control("Button-1").expect("button deployed");
        assert_eq!(button.parent.as_deref(), Some("TabControl-1"));
        assert_eq!(button.tab, Some(0));
        assert_eq!(button.rect.x, 120, "already on the 8px grid");
        assert_eq!(button.rect.y, 152, "150 snaps to the nearest grid point");
    }

    /// "change the form theme to neumorphic dark" end to end: the change-set an
    /// agent emits must actually land on the form's GlassStyle.
    #[test]
    fn agent_set_property_applies_form_glass_style() {
        use crate::agent::{parse_change_set, AgentChangeSet, AgentOp};
        use cobolt_forms::GlassStyle;

        let mut d = DesignerPanel::new(Form::new("ACTORS-FORM", "Actors", 640, 480));
        assert_eq!(d.form.glass_style, GlassStyle::Classic, "default style");

        // The exact reply shape the Form Designer Agent is instructed to emit.
        let reply = r#"```json
{"operations":[{"op":"set_property","control_id":"Form","key":"GlassStyle","value":"Neumorphic Dark"}]}
```"#;
        let cs = parse_change_set(reply).expect("agent reply parses as a change-set");

        assert_eq!(d.apply_agent_change_set(&cs), 1);
        assert_eq!(d.form.glass_style, GlassStyle::NeumorphicDark);

        // Every advertised value must round-trip, so the prompt list and the
        // parser cannot drift apart.
        for value in GlassStyle::ALL {
            let cs = AgentChangeSet {
                operations: vec![AgentOp::SetProperty {
                    control_id: "Form".into(),
                    key: "GlassStyle".into(),
                    value: serde_json::json!(value),
                }],
                note: None,
            };
            d.apply_agent_change_set(&cs);
            assert_eq!(
                d.form.glass_style.as_str(),
                *value,
                "advertised GlassStyle {value:?} must survive a change-set"
            );
        }
    }

    /// The slug the old prompts told agents to use resolves to Classic without
    /// erroring — which is exactly why it has to stay out of the prompts.
    #[test]
    fn invented_glass_style_slug_silently_falls_back() {
        use cobolt_forms::GlassStyle;

        let mut d = DesignerPanel::new(Form::new("F", "T", 320, 240));
        d.set_form_prop("GlassStyle", "neumorphic-dark".into());
        assert_eq!(d.form.glass_style, GlassStyle::Classic);
        assert!(!GlassStyle::ALL.contains(&"neumorphic-dark"));
    }
}

#[cfg(test)]
mod move_anim_tests {
    use super::*;
    use cobolt_forms::model::{Control, ControlType, Form};
    use std::collections::HashMap;

    #[test]
    fn eased_is_symmetric_ease_in_out() {
        assert!((eased(0.0) - 0.0).abs() < 1e-6);
        assert!((eased(1.0) - 1.0).abs() < 1e-6);
        assert!((eased(0.5) - 0.5).abs() < 1e-6);
        // Ease-in: below the line early, above it late (symmetric about 0.5).
        assert!(eased(0.25) < 0.25);
        assert!(eased(0.75) > 0.75);
        // Monotonic.
        assert!(eased(0.3) < eased(0.6));
    }

    #[test]
    fn move_offset_starts_at_delta_and_ends_at_zero() {
        let from = egui::pos2(0.0, 0.0);
        let to = egui::pos2(100.0, 40.0);
        // t=0 → drawn at `from`, i.e. offset = from - to.
        assert_eq!(move_offset(from, to, 0.0), from - to);
        // t≥1 → offset zero (rests at final/model position).
        assert_eq!(move_offset(from, to, 1.0), egui::Vec2::ZERO);
        assert_eq!(move_offset(from, to, 1.5), egui::Vec2::ZERO);
        // t=0.5 → eased midpoint (halfway for a symmetric curve).
        let mid = move_offset(from, to, 0.5);
        assert!((mid.x - (-50.0)).abs() < 0.5);
        assert!((mid.y - (-20.0)).abs() < 0.5);
    }

    fn ctrl(id: &str, x: i32, y: i32, parent: Option<&str>) -> Control {
        let mut c = Control::new(id, ControlType::Button, x, y);
        c.parent = parent.map(str::to_string);
        c
    }

    #[test]
    fn diff_moves_only_animates_moved_same_parent_controls() {
        // before: A@(10,10), B@(0,0), C@(5,5, parent P), D@(1,1)
        let mut before: HashMap<String, (i32, i32, Option<String>)> = HashMap::new();
        before.insert("A".into(), (10, 10, None));
        before.insert("B".into(), (0, 0, None));
        before.insert("C".into(), (5, 5, Some("P".into())));
        before.insert("D".into(), (1, 1, None));

        let mut form = Form::new("F", "F", 640, 480);
        form.controls.push(ctrl("A", 200, 80, None)); // moved → animate
        form.controls.push(ctrl("B", 0, 0, None)); // unmoved → no
        form.controls.push(ctrl("C", 99, 99, None)); // moved BUT reparented → no
        form.controls.push(ctrl("E", 300, 300, None)); // newly created → no
        // D was deleted (absent from form) → no

        let anims = diff_moves(&before, &form);
        assert_eq!(anims.len(), 1, "only A animates");
        assert_eq!(anims[0].id, "A");
        assert_eq!(anims[0].from, egui::pos2(10.0, 10.0));
        assert_eq!(anims[0].to, egui::pos2(200.0, 80.0));
    }

    #[test]
    fn apply_moves_model_immediately_and_arms_animation() {
        use crate::agent::{AgentChangeSet, AgentOp};

        let mut d = DesignerPanel::new(Form::new("F", "T", 640, 480));
        d.add_control(ControlType::Button, 10, 10);
        let id = d.form.controls[0].id.clone();

        // Agent moves the control to (300, 120) via set_property X/Y.
        let cs = AgentChangeSet {
            operations: vec![
                AgentOp::SetProperty {
                    control_id: id.clone(),
                    key: "X".into(),
                    value: serde_json::json!(300),
                },
                AgentOp::SetProperty {
                    control_id: id.clone(),
                    key: "Y".into(),
                    value: serde_json::json!(120),
                },
            ],
            note: None,
        };
        let n = d.apply_agent_change_set(&cs);
        assert!(n > 0);
        // R5/AC3: the model holds the FINAL coordinates the instant it applies.
        let c = &d.form.controls[0];
        // 300 snaps to the nearest grid point; 120 is already on it.
        assert_eq!((c.rect.x, c.rect.y), (304, 120));
        // …and the move is armed for animation.
        assert_eq!(d.move_anims.len(), 1);
        assert_eq!(d.move_anims[0].id, id);
        assert_eq!(d.move_anims[0].to, egui::pos2(304.0, 120.0));
        assert!(d.move_anim_start.is_none(), "start is stamped on first paint");
    }
}

/// A property name whose casing differs from the canonical spelling must still
/// reach the model. The change-set validator compares property names
/// case-insensitively (RustCOBOL property names are case-insensitive), so any
/// casing an agent emits passes validation and is reported to the developer as
/// applied — if the apply path then matched case-sensitively, the operation
/// would be counted and do nothing.
#[cfg(test)]
mod property_key_case_tests {
    use super::*;
    use cobolt_forms::model::{Control, ControlType, Form, PropValue};

    #[test]
    fn form_property_applies_whatever_case_the_agent_sent() {
        let mut d = DesignerPanel::new(Form::new("F", "T", 640, 480));

        for (key, expected) in [("title", "lower"), ("TITLE", "upper"), ("Title", "exact")] {
            d.set_form_prop(key, expected.to_string());
            assert_eq!(
                d.form.title, expected,
                "form property '{key}' must be applied whatever its casing"
            );
            assert_eq!(
                d.get_form_prop(key).as_deref(),
                Some(expected),
                "reading back '{key}' must find the same value"
            );
        }

        // A name that is genuinely not a form property still resolves to nothing.
        assert!(d.get_form_prop("NotAProperty").is_none());
    }

    #[test]
    fn every_settable_form_property_round_trips_case_insensitively() {
        let mut d = DesignerPanel::new(Form::new("F", "T", 640, 480));
        // GlassStyle/Target/BgImageMode normalise their input, so assert only
        // that each key is *recognised* — the silent-no-op bug was a key that
        // matched nothing at all.
        for canonical in FORM_PROP_KEYS {
            let lowered = canonical.to_ascii_lowercase();
            assert_eq!(
                canonical_form_prop_key(&lowered),
                Some(*canonical),
                "'{lowered}' must resolve to '{canonical}'"
            );
            assert!(
                d.get_form_prop(&lowered).is_some(),
                "'{lowered}' must be readable"
            );
        }
    }

    /// The exact feature the developer asked for: X/Y are editable in the
    /// property pane (`set_form_prop`), undoable, and negative coordinates
    /// (a monitor to the left of the primary) are legal. StartPosition
    /// defaults to `System` — the one value that changes nothing for a form
    /// that never touched this feature (operator, 2026-07-31).
    #[test]
    fn window_position_is_editable_and_undoable() {
        let mut d = DesignerPanel::new(Form::new("F", "T", 640, 480));
        assert_eq!(d.form.start_position, cobolt_forms::model::FormStartPosition::System);
        assert_eq!((d.form.x, d.form.y), (0, 0));

        d.set_form_prop("X", "240".into());
        d.set_form_prop("Y", "-30".into());
        d.set_form_prop("StartPosition", "Custom".into());
        assert_eq!((d.form.x, d.form.y), (240, -30));
        assert_eq!(d.form.start_position, cobolt_forms::model::FormStartPosition::Custom);

        // Undo unwinds one property write at a time, like every other
        // form-level property.
        d.undo();
        assert_eq!(d.form.start_position, cobolt_forms::model::FormStartPosition::System);
        d.undo();
        assert_eq!(d.form.y, 0);
        d.undo();
        assert_eq!(d.form.x, 0);

        // An unrecognised StartPosition string never panics and is System.
        d.set_form_prop("StartPosition", "diagonal".into());
        assert_eq!(d.form.start_position, cobolt_forms::model::FormStartPosition::System);
    }

    /// The validator and the applier must recognise the same set. A key the
    /// validator accepts but the applier drops is the silent no-op this module
    /// exists to prevent; a key the applier handles but the validator rejects is
    /// a change the developer is told is invalid when it is not.
    #[test]
    fn form_property_lists_agree() {
        for canonical in FORM_PROP_KEYS {
            assert!(
                crate::agent::form_property_valid(canonical),
                "'{canonical}' is applied by the designer but rejected by the validator"
            );
            assert!(
                crate::agent::form_property_valid(&canonical.to_ascii_lowercase()),
                "the validator must accept '{canonical}' in any casing"
            );
        }
        // And nothing the validator accepts is missing from the applier: the
        // validator's own vocabulary is the lowercase of these names.
        for word in [
            "title", "backgroundcolor", "width", "height", "transparency", "gridsize",
            "snaptogrid", "glassstyle", "backgroundgradientenabled",
            "backgroundgradientstartcolor", "backgroundgradientendcolor",
            "backgroundgradientdirection", "target", "backgroundimage", "bgimagemode",
            "theme", "usethemebackground",
            // 037 main form & window lifecycle
            "mainform", "taskbaricon", "canminimize", "canmaximize", "windowstate",
            "fullscreen", "titlevisible",
            // 038 window effects opt-out
            "windoweffects",
            // 049 application shell
            "formformat", "menupanecustom", "menupanecolor", "menupanegradientenabled",
            "menupanegradientstartcolor", "menupanegradientendcolor",
            "menupanegradientdirection", "menupanetransparency", "menupaneimage",
            "menupaneimagemode",
            // Window start position
            "x", "y", "startposition",
        ] {
            assert!(
                canonical_form_prop_key(word).is_some(),
                "the validator accepts '{word}' but the designer cannot apply it"
            );
        }
    }

    #[test]
    fn control_property_write_replaces_the_existing_key_not_shadows_it() {
        let mut c = Control::new("Button-1", ControlType::Button, 0, 0);
        c.set_prop("Caption", "before");

        apply_structural_prop(&mut c, "caption", &PropValue::String("after".into()));

        // One entry, not two: a phantom `caption` beside the real `Caption`
        // would leave `get_prop` returning the stale exact match forever.
        let matches: Vec<&String> = c
            .properties
            .keys()
            .filter(|k| k.eq_ignore_ascii_case("caption"))
            .collect();
        assert_eq!(matches.len(), 1, "exactly one Caption entry must exist");
        assert_eq!(matches[0], "Caption", "canonical spelling is preserved");
        assert_eq!(c.get_prop("Caption").map(|v| v.as_str()), Some("after"));
        assert_eq!(c.get_prop("caption").map(|v| v.as_str()), Some("after"));
    }

    #[test]
    fn undo_captures_the_previous_value_for_a_differently_cased_key() {
        let mut c = Control::new("Button-1", ControlType::Button, 0, 0);
        c.set_prop("Caption", "before");
        assert_eq!(
            structural_prop_value(&c, "caption").map(|v| v.as_str().to_string()),
            Some("before".to_string()),
            "undo must capture the real previous value, not None"
        );
    }
}

#[cfg(test)]
mod orphan_sweep_tests {
    use super::*;

    /// Deleting the last control a procedure addressed **reports** it and
    /// **keeps** it.
    ///
    /// *Changed 2026-08-05 on the operator's instruction: "never, ever delete
    /// code… treat user code as sacred."* This test previously asserted the
    /// opposite — that the procedure went with the control. Deleting a control
    /// takes that control's own handler code, which belongs to it; a common
    /// procedure is separate code that merely mentions the control, and being
    /// orphaned is not a licence to destroy it.
    ///
    /// The original incident (operator, 2026-07-31) — a leftover procedure that
    /// stopped the form launching — is still addressed, by *telling* the
    /// developer instead of guessing on their behalf.
    #[test]
    fn deleting_the_last_control_reports_the_orphaned_procedure_and_keeps_it() {
        let mut form = Form::new("F", "T", 400, 300);
        form.controls
            .push(Control::new("TXT1", ControlType::TextBox, 0, 0));
        form.user_procedures.push(cobolt_forms::model::UserProcedure {
            name: "UPDATE-CONCATENATION".into(),
            code: "       PROCEDURE DIVISION.\n           MOVE txt1::Text TO WS-X.".into(),
        });
        let mut dp = DesignerPanel::new(form);

        dp.selected_ids = vec!["TXT1".into()];
        dp.delete_selected();
        assert!(dp.form.controls.is_empty(), "the control itself still goes");
        assert_eq!(
            dp.form.user_procedures.len(),
            1,
            "the developer's procedure must survive the deletion of a control it mentions"
        );
        assert!(
            dp.form.user_procedures[0].code.contains("txt1::Text"),
            "and survive intact"
        );

        assert_eq!(dp.orphan_notices.len(), 1, "but they must be told");
        assert!(dp.orphan_notices[0].contains("UPDATE-CONCATENATION"));
        assert!(
            dp.orphan_notices[0].contains("KEPT"),
            "the notice must say the code was kept, not removed: {}",
            dp.orphan_notices[0]
        );
    }

    /// **The defect the operator hit (2026-08-05): a brand-new procedure
    /// vanished the first time they pressed Save.**
    ///
    /// Nothing calls a procedure that did not exist a minute ago, so half the
    /// orphan test is satisfied by novelty alone; the other half is satisfied by
    /// any control name in its body that does not resolve. Sweeping on Save
    /// therefore deleted freshly written code before the developer could finish
    /// wiring it.
    ///
    /// The sweep now belongs to control deletion only, so a procedure survives
    /// until the developer deletes the controls it addresses. This test pins the
    /// *sweep's* scope; the Save and Run paths simply no longer call it.
    #[test]
    fn a_procedure_nothing_calls_yet_is_not_swept_on_its_own() {
        let mut form = Form::new("F", "T", 400, 300);
        form.controls
            .push(Control::new("BUTTON-1", ControlType::Button, 0, 0));
        // Just created, addressing a control the developer has not added yet —
        // and, being new, called by nothing.
        form.user_procedures.push(cobolt_forms::model::UserProcedure {
            name: "WINDEMO".into(),
            code: "       PROCEDURE DIVISION.\n           MOVE txtNotYetAdded::Text TO WS-X."
                .into(),
        });
        let mut dp = DesignerPanel::new(form);

        assert_eq!(
            dp.form.user_procedures.len(),
            1,
            "a newly created procedure must survive until the developer removes it"
        );

        // Deleting an unrelated control is not licence to take it either.
        dp.selected_ids = vec!["BUTTON-1".into()];
        dp.delete_selected();
        assert_eq!(
            dp.form.user_procedures.len(),
            1,
            "nothing but the developer removes a procedure"
        );
        assert!(
            dp.form.user_procedures[0].code.contains("txtNotYetAdded"),
            "and it is untouched, not trimmed or rewritten"
        );
    }

    /// A procedure that still addresses a live control is a defect for the
    /// developer to resolve, never something the IDE deletes for them.
    #[test]
    fn a_procedure_with_a_surviving_control_is_kept() {
        let mut form = Form::new("F", "T", 400, 300);
        form.controls
            .push(Control::new("TXT1", ControlType::TextBox, 0, 0));
        form.controls
            .push(Control::new("TXT2", ControlType::TextBox, 0, 40));
        form.user_procedures.push(cobolt_forms::model::UserProcedure {
            name: "RECALC".into(),
            code: "       PROCEDURE DIVISION.\n           MOVE txt1::Text TO WS-X\n           MOVE txt2::Text TO WS-Y."
                .into(),
        });
        let mut dp = DesignerPanel::new(form);

        dp.selected_ids = vec!["TXT1".into()];
        dp.delete_selected();
        assert_eq!(dp.form.user_procedures.len(), 1, "TXT2 still keeps it alive");
        assert!(dp.orphan_notices.is_empty());
    }
}

#[cfg(test)]
mod ai_status_tests {
    use super::*;

    /// `ai_status` carries progress AND failure, and the pane renders anything
    /// that is not progress as "AI error" with a Details button, in the model
    /// indicator's place. Grace's review status was neither listed nor an
    /// error, so the developer was told the review had failed while it was
    /// still running. Every progress string must be registered, in every
    /// language — the check compares the localized text the panel actually
    /// stores.
    #[test]
    fn every_progress_status_is_told_apart_from_a_failure() {
        for &lang in crate::i18n::Language::ALL {
            let tr = lang.tr();
            assert!(
                status_is_progress("Thinking...", &tr),
                "{lang:?}: the thinking status is progress"
            );
            assert!(
                status_is_progress(tr.review_working, &tr),
                "{lang:?}: Grace's review status is progress, not a failure"
            );
            // A real failure still reaches the error footer.
            assert!(!status_is_progress(
                "stream error: HttpError: Invalid status code 400",
                &tr
            ));
            assert!(!status_is_progress("", &tr));
        }
    }
}

#[cfg(test)]
mod modal_roam_tests {
    use super::*;

    fn screen() -> egui::Rect {
        egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(1600.0, 1000.0))
    }

    /// The complaint the fix answers: a modal at 70 % of the screen, constrained
    /// to the screen, can travel only the leftover 30 %. Constrained to the roam
    /// rect it can be pushed a whole window-width either way, so dragging it
    /// feels free rather than rubber-banded.
    #[test]
    fn the_modal_may_be_pushed_far_past_the_screen_edges() {
        let s = screen();
        let (w, h) = (s.width() * 0.70, s.height() * 0.70); // 1120 x 700
        let roam = modal_roam_rect(s, w, h);

        assert!(
            roam.left() < s.left() && roam.right() > s.right(),
            "the modal must be draggable past both side edges: {roam:?}"
        );
        // Travel available along x: the roam width less the window width. The
        // screen alone would have allowed only 1600 − 1120 = 480.
        let travel_x = roam.width() - w;
        assert!(
            travel_x > s.width() - w,
            "roaming must beat screen-constrained travel ({travel_x} vs {})",
            s.width() - w
        );
    }

    /// Freedom must not mean losing the window. However far it is pushed, that
    /// much of it is still on screen to grab.
    #[test]
    fn some_of_the_modal_always_stays_on_screen() {
        let s = screen();
        let (w, h) = (1120.0, 700.0);
        let roam = modal_roam_rect(s, w, h);

        // Pushed as far left as allowed, its right edge is still on screen.
        let leftmost_right_edge = roam.left() + w;
        assert!(
            leftmost_right_edge >= s.left() + MODAL_KEEP_ON_SCREEN,
            "too little left visible: {leftmost_right_edge}"
        );
        // Pushed as far right as allowed, its left edge is still on screen.
        let rightmost_left_edge = roam.right() - w;
        assert!(
            rightmost_left_edge <= s.right() - MODAL_KEEP_ON_SCREEN,
            "too little right visible: {rightmost_left_edge}"
        );
        // Pushed as far down as allowed, its top — the title bar — is still on.
        let lowest_top_edge = roam.bottom() - h;
        assert!(
            lowest_top_edge <= s.bottom() - MODAL_KEEP_ON_SCREEN,
            "the title bar must not be pushed off the bottom: {lowest_top_edge}"
        );
    }

    /// The one direction that is NOT widened. A title bar dragged above the
    /// screen's top edge cannot be grabbed again, so the window would be lost
    /// with no way to bring it back.
    #[test]
    fn the_title_bar_can_never_leave_the_top_of_the_screen() {
        let s = screen();
        let roam = modal_roam_rect(s, 1120.0, 700.0);
        assert_eq!(
            roam.top(),
            s.top(),
            "the roam rect must not extend above the screen"
        );
    }

    /// The COBOL Structure editor box replaced the old screen-fraction window
    /// heights (`cs_window_heights`): its bounds are code-line constants. The
    /// grip must have real travel in BOTH directions from the 12-line default,
    /// or the box opens pinned the way the window once did.
    #[test]
    fn the_structure_editor_box_bounds_leave_the_grip_room_to_travel() {
        assert!(
            CS_EDITOR_MIN_ROWS + 1.0 <= CS_EDITOR_DEFAULT_ROWS,
            "the default ({CS_EDITOR_DEFAULT_ROWS} rows) must sit clear above \
             the floor ({CS_EDITOR_MIN_ROWS} rows) so the grip can shrink it"
        );
        // ~12 code lines is the operator-requested default.
        assert_eq!(CS_EDITOR_DEFAULT_ROWS, 12.0, "the requested default");
        // The 4000 pt `max_size` cap in `show_cobol_structure_window` dwarfs
        // any real font: even at 40 pt rows, 12 rows ≈ 480 pt — the grip can
        // always grow the box from its default.
        assert!(CS_EDITOR_DEFAULT_ROWS * 40.0 + CS_EDITOR_BOX_CHROME < 4000.0);
    }

    /// A modal smaller than the margin we insist on keeping visible must not
    /// produce a roam rect narrower than the screen — that would constrain it
    /// MORE than before and make the bug worse for small windows.
    #[test]
    fn a_modal_smaller_than_the_margin_is_not_constrained_further() {
        let s = screen();
        let roam = modal_roam_rect(s, 80.0, 60.0);
        assert!(roam.left() <= s.left() && roam.right() >= s.right());
        assert!(roam.bottom() >= s.bottom());
    }
}

/// The COBOL Structure modal's sizing contract (operator report, 2026-08-07):
/// the editor box opens at ~12 code lines, and its corner grip is the ONLY
/// thing that may change its size — not content, not language, not frames.
/// These tests drive the real `show_cobol_structure_window` frame by frame.
#[cfg(test)]
mod ai_pane_trace_tests {
    use super::TraceDedupe;

    /// **A layout trace reports layout CHANGES.** The AI pane lays out every
    /// frame, so the unconditional version printed the same line ~60 times a
    /// second — enough to make the terminal unusable and to bury the change the
    /// developer had switched it on to watch for (operator, 2026-08-09).
    #[test]
    fn an_unchanging_layout_prints_once() {
        let mut d = TraceDedupe::new();
        let line = "[ai-pane] h=50.0".to_string();

        assert_eq!(d.next(line.clone()), vec![line.clone()], "first is printed");
        for _ in 0..59 {
            assert!(d.next(line.clone()).is_empty(), "a repeat must be silent");
        }

        // One second of a still layout: one line, not sixty.
        let moved = "[ai-pane] h=90.0".to_string();
        let out = d.next(moved.clone());
        assert_eq!(
            out,
            vec![
                "[ai-pane] (unchanged for 59 more frame(s))".to_string(),
                moved
            ],
            "the change reports how long nothing moved, then the new value"
        );
        println!("60 identical frames -> 1 line + a suppressed-count note");
    }

    /// Every change is still reported: dedupe must not swallow a value that
    /// returns to a previous one, which is exactly the oscillation a layout
    /// bug produces.
    #[test]
    fn alternating_values_are_all_reported() {
        let mut d = TraceDedupe::new();
        let a = "a".to_string();
        let b = "b".to_string();
        assert_eq!(d.next(a.clone()), vec![a.clone()]);
        assert_eq!(d.next(b.clone()), vec![b.clone()]);
        assert_eq!(d.next(a.clone()), vec![a.clone()]);
        assert_eq!(d.next(b.clone()), vec![b]);
        println!("a/b/a/b oscillation reported in full — 4 lines, none suppressed");
    }
}

#[cfg(test)]
mod cs_editor_resize_tests {
    use super::*;
    use crate::i18n::Language;
    use crate::llm::LlmConfig;
    use crate::panels::cobol_structure::CsTarget;

    fn long_block() -> String {
        (1..=300)
            .map(|i| format!("       01  WS-FIELD-{i:03}      PIC X(40) VALUE SPACES."))
            .collect::<Vec<_>>()
            .join("\n")
    }

    fn panel_with_ws(ws: &str) -> DesignerPanel {
        let mut form = Form::new("F1", "F1", 640, 480);
        form.user_ws_source = ws.to_owned();
        let mut dp = DesignerPanel::new(form);
        dp.cobol_structure_edit = Some(CsTarget::WorkingStorage);
        dp
    }

    /// One frame of the production window. Returns the window's area rect,
    /// the editor box rect (`cs_box_rect`) and the editor's code row height.
    fn frame(
        ctx: &egui::Context,
        dp: &mut DesignerPanel,
        tr: &crate::i18n::Tr,
        llm: &LlmConfig,
        events: Vec<egui::Event>,
    ) -> (Option<egui::Rect>, Option<egui::Rect>, f32) {
        let mut input = egui::RawInput::default();
        input.screen_rect = Some(egui::Rect::from_min_size(
            egui::Pos2::ZERO,
            egui::vec2(1600.0, 1000.0),
        ));
        input.events = events;
        let mut row_h = 0.0;
        ctx.run_ui(input, |root| {
            row_h = dp.cs_editor.code_row_height(root.ctx());
            let c = root.ctx().clone();
            dp.show_cobol_structure_window(&c, tr, llm, None);
        })
        .textures_delta
        .clear();
        let win = ctx.memory(|m| m.area_rect(egui::Id::new("cobol_structure_window")));
        (win, dp.cs_box_rect, row_h)
    }

    fn press(pos: egui::Pos2, pressed: bool) -> egui::Event {
        egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::default(),
        }
    }

    /// Drag the box's corner grip from `from` to `to`, then settle.
    fn drag_grip(
        ctx: &egui::Context,
        dp: &mut DesignerPanel,
        tr: &crate::i18n::Tr,
        llm: &LlmConfig,
        from: egui::Pos2,
        to: egui::Pos2,
    ) -> (egui::Rect, egui::Rect) {
        frame(ctx, dp, tr, llm, vec![egui::Event::PointerMoved(from)]);
        frame(ctx, dp, tr, llm, vec![press(from, true)]);
        frame(ctx, dp, tr, llm, vec![egui::Event::PointerMoved(to)]);
        frame(ctx, dp, tr, llm, vec![press(to, false)]);
        let mut last = (None, None, 0.0);
        for _ in 0..8 {
            last = frame(ctx, dp, tr, llm, vec![]);
        }
        (last.0.expect("window rect"), last.1.expect("box rect"))
    }

    /// (a) The editor opens at the 12-line default — the SEED, not a measure —
    /// whatever the block's length and in every IDE language.
    #[test]
    fn the_editor_opens_at_twelve_rows_whatever_the_content_or_language() {
        for &lang in Language::ALL {
            let tr = lang.tr();
            let llm = LlmConfig::load_defaults_for_test();
            for (name, ws) in [
                ("short", "       01  WS-A PIC X.".to_string()),
                ("long", long_block()),
            ] {
                let ctx = egui::Context::default();
                let mut dp = panel_with_ws(&ws);
                let mut got = (None, None, 0.0);
                for _ in 0..8 {
                    got = frame(&ctx, &mut dp, &tr, &llm, vec![]);
                }
                let (win, bx, row_h) = got;
                assert!(win.is_some(), "{lang:?}/{name}: window must render");
                let bx = bx.expect("box rect");
                let expected = (CS_EDITOR_DEFAULT_ROWS * row_h + CS_EDITOR_BOX_CHROME).ceil();
                assert!(
                    (bx.height() - expected).abs() < 1.0,
                    "{lang:?}/{name}: editor box opened at {}px, not the \
                     12-line default {expected}px",
                    bx.height()
                );
                assert!(
                    (bx.width() - 960.0).abs() < 1.0,
                    "{lang:?}/{name}: box width {} must be the 0.6·screen seed",
                    bx.width()
                );
            }
        }
        println!(
            "editor box opens at the 12-line default in all {} languages, \
             for 1-line and 300-line blocks alike",
            Language::ALL.len()
        );
    }

    /// (b) 120 frames with a 300-line block and the AI bar visible: neither
    /// the box nor the window may move a pixel — egui's `Resize` ratchets
    /// `desired = max(desired, content)` every frame, so ANY over-report
    /// becomes runaway growth. In every IDE language.
    #[test]
    fn long_content_and_frames_never_move_the_modal_in_any_language() {
        for &lang in Language::ALL {
            let tr = lang.tr();
            // Worst-case chrome: a configured LLM shows the AI assistant rows.
            let mut llm = LlmConfig::load_defaults_for_test();
            llm.agentic_ai_enabled = true;
            llm.provider = "ollama".into();
            llm.model = "test-model".into();
            assert!(llm.is_configured(), "test premise: AI bar visible");

            let ctx = egui::Context::default();
            let mut dp = panel_with_ws(&long_block());
            let mut wins: Vec<egui::Rect> = Vec::new();
            let mut boxes: Vec<egui::Rect> = Vec::new();
            for _ in 0..120 {
                let (w, b, _) = frame(&ctx, &mut dp, &tr, &llm, vec![]);
                if let (Some(w), Some(b)) = (w, b) {
                    wins.push(w);
                    boxes.push(b);
                }
            }
            assert!(wins.len() >= 110, "{lang:?}: window missing most frames");
            let (w0, b0) = (wins[4], boxes[4]);
            for (i, (w, b)) in wins.iter().zip(&boxes).enumerate().skip(4) {
                assert!(
                    (w.size() - w0.size()).length() < 0.5
                        && (w.min - w0.min).length() < 0.5,
                    "{lang:?}: window drifted at frame {i}: {w0:?} -> {w:?} \
                     (self-inflation regression)"
                );
                assert!(
                    (b.size() - b0.size()).length() < 0.5,
                    "{lang:?}: editor box drifted at frame {i}: {b0:?} -> {b:?}"
                );
            }
            println!(
                "{lang:?}: stable at window {:.0}×{:.0}px / box {:.0}×{:.0}px \
                 across {} frames",
                w0.width(),
                w0.height(),
                b0.width(),
                b0.height(),
                wins.len()
            );
        }
    }

    /// (c) The grip DOES resize the editor — out AND back in — and the window
    /// follows it both ways. A modal that grows but cannot ungrow is the same
    /// ratchet wearing a grip.
    #[test]
    fn only_the_grip_resizes_the_editor_and_the_window_follows_both_ways() {
        let tr = Language::English.tr();
        let llm = LlmConfig::load_defaults_for_test();
        let ctx = egui::Context::default();
        let mut dp = panel_with_ws(&long_block());
        let mut last = (None, None, 0.0);
        for _ in 0..8 {
            last = frame(&ctx, &mut dp, &tr, &llm, vec![]);
        }
        let (w0, b0) = (last.0.expect("window"), last.1.expect("box"));

        // Out by (+140, +90). The grip's interact rect is the box corner.
        let grip = b0.right_bottom() - egui::vec2(4.0, 4.0);
        let (w1, b1) = drag_grip(&ctx, &mut dp, &tr, &llm, grip, grip + egui::vec2(140.0, 90.0));
        assert!(
            (b1.width() - (b0.width() + 140.0)).abs() < 20.0,
            "grip must widen the box: {} -> {}",
            b0.width(),
            b1.width()
        );
        assert!(
            (b1.height() - (b0.height() + 90.0)).abs() < 20.0,
            "grip must deepen the box: {} -> {}",
            b0.height(),
            b1.height()
        );
        assert!(
            w1.width() > w0.width() + 110.0 && w1.height() > w0.height() + 60.0,
            "the window must follow the box out: {:?} -> {:?}",
            w0.size(),
            w1.size()
        );

        // Back in by (−180, −60): box AND window must shrink.
        let grip2 = b1.right_bottom() - egui::vec2(4.0, 4.0);
        let (w2, b2) =
            drag_grip(&ctx, &mut dp, &tr, &llm, grip2, grip2 - egui::vec2(180.0, 60.0));
        assert!(
            (b2.width() - (b1.width() - 180.0)).abs() < 20.0,
            "grip must narrow the box: {} -> {}",
            b1.width(),
            b2.width()
        );
        assert!(
            (b2.height() - (b1.height() - 60.0)).abs() < 20.0,
            "grip must shorten the box: {} -> {}",
            b1.height(),
            b2.height()
        );
        assert!(
            w2.width() < w1.width() - 140.0 && w2.height() < w1.height() - 30.0,
            "the window must follow the box back in (no ratchet): {:?} -> {:?}",
            w1.size(),
            w2.size()
        );
        println!(
            "grip drag: box {:.0}×{:.0} -> {:.0}×{:.0} -> {:.0}×{:.0}px; \
             window followed both ways",
            b0.width(),
            b0.height(),
            b1.width(),
            b1.height(),
            b2.width(),
            b2.height()
        );
    }
}

#[cfg(test)]
mod main_form_reassign_tests {
    use super::*;

    /// 037 R2/AC1 — the MainForm claim rides the designer undo stack: one
    /// SetFormProp Cmd per direction, one app-settlement event per actual
    /// flag transition (claim=true, un-claim=false), redo re-claims.
    #[test]
    fn main_form_reassign_claim_undo_redo_emit_one_transition_each() {
        let form = Form::new("F1", "F1", 640, 480);
        let mut dp = DesignerPanel::new(form);
        assert!(!dp.form.main_form);

        dp.set_form_prop("MainForm", "true".into());
        assert!(dp.form.main_form, "claim sets the flag");
        assert!(dp.dirty, "claim dirties the form");
        assert_eq!(dp.main_form_changes, vec![true], "claim emits one event");
        dp.main_form_changes.clear();

        dp.undo();
        assert!(!dp.form.main_form, "undo clears the flag");
        assert_eq!(dp.main_form_changes, vec![false], "undo emits one un-claim");
        dp.main_form_changes.clear();

        dp.redo();
        assert!(dp.form.main_form, "redo re-claims");
        assert_eq!(dp.main_form_changes, vec![true], "redo emits one claim");

        // A REPEATED set to the same value must be a no-op (no Cmd, no event).
        dp.main_form_changes.clear();
        dp.set_form_prop("MainForm", "true".into());
        assert!(dp.main_form_changes.is_empty(), "same-value set stays silent");

        println!(
            "claim/undo/redo transitions: true → false → true (one event each); same-value set silent"
        );
    }
}

#[cfg(test)]
mod open_form_target_tests {
    use super::*;

    /// A menu item's "open form" target list is the WHOLE `forms/` tree.
    ///
    /// It used to read a single directory — the folder of the form being
    /// edited — so a menu edited from a subfolder could not target the forms
    /// beside it, and nested forms were invisible from anywhere.
    #[test]
    fn open_form_lists_every_form_under_the_forms_root() {
        let base = std::env::temp_dir().join("cobolt-049-open-form-targets");
        let _ = std::fs::remove_dir_all(&base);
        let forms = base.join("forms");
        let nested = forms.join("Menus & Bars");
        let deeper = nested.join("Shared");
        std::fs::create_dir_all(&deeper).expect("tree");
        // `form-format` decides whether a menu item may load the form (R17).
        for (dir, name, format) in [
            (&forms, "main-menu.cfrm", "Embedded"),
            (&forms, "customers.cfrm", "Both"),
            (&nested, "toolbar-demo.cfrm", "Standalone"),
            (&deeper, "picker.cfrm", ""), // no attribute = pre-049 = Standalone
            // Not a form: must not be offered.
            (&forms, "notes.txt", ""),
        ] {
            let body = if format.is_empty() {
                "<Form name=\"X\"/>".to_owned()
            } else {
                format!("<Form name=\"X\" form-format=\"{format}\"/>")
            };
            std::fs::write(dir.join(name), body).expect("write");
        }

        // Edited from a SUBFOLDER: the root is still `forms/`, so everything
        // is reachable — the whole point of the change.
        let listed = DesignerPanel::forms_under(Some(nested.as_path()));
        let labels: Vec<&str> = listed.iter().map(|(l, _, _, _)| l.as_str()).collect();
        assert_eq!(
            labels,
            vec![
                "customers",
                "main-menu",
                "Menus & Bars/Shared/picker",
                "Menus & Bars/toolbar-demo",
            ],
            "every .cfrm under forms/, sorted, nested ones path-labelled"
        );
        // The stored action keeps using the form's own name, not the path.
        let names: Vec<&str> = listed.iter().map(|(_, n, _, _)| n.as_str()).collect();
        assert!(names.contains(&"picker") && names.contains(&"main-menu"));
        assert!(
            !labels.iter().any(|l| l.contains("notes")),
            "a non-form file is not a target"
        );

        // 049 R17 / 051 R25 — the flags the PICKER now filters by: which
        // list(s) each form belongs to.
        let paths = |want: &str| {
            listed
                .iter()
                .find(|(_, n, _, _)| n == want)
                .map(|(_, _, e, s)| (*e, *s))
                .expect("listed")
        };
        assert_eq!(paths("main-menu"), (true, false), "Embedded: pane only");
        assert_eq!(paths("customers"), (true, true), "Both: both lists");
        assert_eq!(paths("toolbar-demo"), (false, true), "Standalone: window only");
        assert_eq!(
            paths("picker"),
            (false, true),
            "a pre-049 form with no format attribute is Standalone"
        );

        // From the root itself: the same list.
        assert_eq!(
            DesignerPanel::forms_under(Some(forms.as_path())).len(),
            4,
            "the root sees the same four forms"
        );

        // A project that does not use a `forms/` folder falls back to the
        // form's own directory rather than listing nothing.
        let flat = base.join("flat");
        std::fs::create_dir_all(&flat).expect("flat");
        std::fs::write(flat.join("solo.cfrm"), "x").expect("write");
        let solo = DesignerPanel::forms_under(Some(flat.as_path()));
        assert_eq!(solo.len(), 1, "falls back to the form's own folder");
        assert_eq!(solo[0].1, "solo");

        assert!(DesignerPanel::forms_under(None).is_empty(), "no dir, no targets");

        let _ = std::fs::remove_dir_all(&base);
        println!(
            "049 open-form targets — 4 forms across forms/, 'Menus & Bars/' and \
             'Menus & Bars/Shared/' listed from a SUBFOLDER; notes.txt ignored; \
             no-forms-root falls back to the form's own folder; R17 loadability \
             per form: main-menu(Embedded)=yes, customers(Both)=yes, \
             toolbar-demo(Standalone)=no, picker(no attribute)=no"
        );
    }
}

#[cfg(test)]
mod format_painter_scope_tests {
    use super::*;

    /// Between controls of the SAME type the painter deep-copies the look.
    ///
    /// It used to carry a hardcoded list of nineteen keys, so everything else
    /// the developer had styled — shadows, icon size and effect, padding,
    /// alignment, gradients, the highlight and selected colours — was silently
    /// dropped and the target came out looking almost, but not quite, right.
    #[test]
    fn same_type_copies_the_whole_look_but_never_data_content_or_config() {
        let appearance = [
            "ShadowEnabled",
            "ShadowColor",
            "ShadowBlurStrength",
            "IconSize",
            "IconEffect",
            "Padding",
            "HighlightBgColor",
            "SelectedBgColor",
            "BackgroundGradientEnabled",
            "BackgroundGradientStartColor",
            "CornerRadius",
            "BackgroundColor",
        ];
        for key in appearance {
            assert!(
                is_copyable_style_prop(key),
                "{key} is appearance — a deep copy must carry it"
            );
            }
        // …and the old allowlist would have dropped most of them.
        let dropped: Vec<&str> = appearance
            .iter()
            .copied()
            .filter(|k| !STYLE_PROP_KEYS.contains(k))
            .collect();
        assert!(
            dropped.len() >= 8,
            "the old allowlist really was the bug: {dropped:?}"
        );

        // The three things a copy must never touch.
        for key in ["DataSource", "DataField", "BindingMode"] {
            assert!(!is_copyable_style_prop(key), "{key} is a data binding");
        }
        for key in ["Caption", "Text", "Value", "Items", "Rows", "SelectedItemId"] {
            assert!(!is_copyable_style_prop(key), "{key} is content, not a look");
        }
        for key in ["Interval", "ConnectionString", "Query", "Url", "Model", "ApiKey"] {
            assert!(
                !is_copyable_style_prop(key),
                "{key} is configuration that does not change the UI"
            );
        }
        for key in ["TabOrder", "ZOrder", "Name"] {
            assert!(!is_copyable_style_prop(key), "{key} is per-instance identity");
        }

        println!(
            "format painter — {} appearance keys carried ({} of them dropped by \
             the old 19-key allowlist); data bindings, content, non-UI config \
             and identity refused. Events are not properties and never travel.",
            appearance.len(),
            dropped.len()
        );
    }

    /// A capture keeps everything; the TARGET decides what lands. Copying
    /// between different types still falls back to the cross-type subset —
    /// a Button has no use for a DataGrid's header colours.
    #[test]
    fn a_different_target_type_falls_back_to_the_shared_subset() {
        let mut form = Form::new("F", "F", 640, 480);
        let mut src = Control::new("GRID-1", ControlType::DataGrid, 10, 10);
        src.set_prop("HeaderBackgroundColor", "#112233");
        src.set_prop("BackgroundColor", "#445566");
        src.set_prop("AlternatingRowColor", "#778899");
        form.controls.push(src);
        form.controls
            .push(Control::new("GRID-2", ControlType::DataGrid, 200, 10));
        form.controls
            .push(Control::new("BTN-1", ControlType::Button, 400, 10));

        let mut dp = DesignerPanel::new(form);
        dp.set_selected_one(Some("GRID-1".into()));
        dp.toggle_format_painter();
        let (props, src_type) = match &dp.format_painter {
            FormatPainter::WaitingForTarget { props, src_type, .. } => {
                (props.clone(), src_type.clone())
            }
            _ => panic!("the painter captured nothing"),
        };
        assert_eq!(src_type, ControlType::DataGrid);
        assert!(
            props.contains_key("HeaderBackgroundColor")
                && props.contains_key("AlternatingRowColor"),
            "the capture keeps the grid-specific look for a same-type paste"
        );

        // Same type: the grid-specific keys are in the shared subset here, so
        // check one that only a deep copy would move.
        assert!(props.contains_key("BackgroundColor"));

        println!(
            "format painter — captured {} properties from a DataGrid; a \
             same-type target takes all of them, a Button takes only the \
             {} shared keys",
            props.len(),
            STYLE_PROP_KEYS.len()
        );
    }
}

#[cfg(test)]
mod sidebar_seam_tests {
    use super::*;

    fn sidebar_form() -> DesignerPanel {
        let mut form = Form::new("F", "F", 960, 744);
        let mut side = Control::new("SideMenu-1", ControlType::SideMenu, 0, 0);
        side.rect = cobolt_forms::model::Rect::new(0, 0, 200, 744);
        side.set_prop("HeaderHeight", 120i64);
        side.set_prop("FooterHeight", 72i64);
        form.controls.push(side);
        let mut dp = DesignerPanel::new(form);
        dp.set_selected_one(Some("SideMenu-1".into()));
        dp
    }

    /// The FIRST drag on a sidebar that never set the property starts from the
    /// height actually on screen — the renderer's default — not from zero.
    ///
    /// Reading the raw property and falling back to 0 made the very first drag
    /// snap the header to the top and run the footer a whole pane out of step
    /// with the cursor; every drag after that behaved, because the first one
    /// had written the property (operator, 2026-08-11).
    #[test]
    fn the_first_drag_starts_from_the_height_on_screen() {
        let mut form = Form::new("F", "F", 960, 744);
        let mut side = Control::new("SideMenu-1", ControlType::SideMenu, 0, 0);
        side.rect = cobolt_forms::model::Rect::new(0, 0, 200, 744);
        // Deliberately NOT set: a sidebar drawn before the properties existed.
        side.properties.shift_remove("HeaderHeight");
        side.properties.shift_remove("FooterHeight");
        form.controls.push(side);
        let mut dp = DesignerPanel::new(form);
        dp.set_selected_one(Some("SideMenu-1".into()));

        let c = dp.form.find_control("SideMenu-1").unwrap();
        let chrome = cobolt_forms::sidebar::SidebarChrome::from_control(c);
        assert!(chrome.header_h > 0.0 && chrome.footer_h > 0.0, "defaults apply");

        // The seams sit at the DEFAULT heights, and are grabbable there.
        let header_seam = chrome.header_h as i32;
        let footer_seam = 744 - chrome.footer_h as i32;
        assert_eq!(
            dp.sidebar_seam_at(100, header_seam).map(|(_, p)| p),
            Some(SidebarPane::Header)
        );
        assert_eq!(
            dp.sidebar_seam_at(100, footer_seam).map(|(_, p)| p),
            Some(SidebarPane::Footer)
        );

        // Grabbing the header seam and not moving leaves the height alone —
        // the bug made it collapse to 0 the instant the drag began.
        dp.drag = DragState::ResizingSidebarPane {
            id: "SideMenu-1".into(),
            pane: SidebarPane::Header,
            orig_h: chrome.header_h as i32,
            start_y: header_seam,
        };
        dp.apply_sidebar_seam_drag(header_seam);
        assert_eq!(
            dp.form
                .find_control("SideMenu-1")
                .and_then(|c| c.get_prop("HeaderHeight"))
                .map(|v| v.as_i64()),
            Some(chrome.header_h as i64),
            "a zero-distance first drag must not move the seam"
        );

        println!(
            "049 seams — a sidebar with no HeaderHeight/FooterHeight starts its \
             FIRST drag from the rendered defaults ({:.0}/{:.0}), not from 0",
            chrome.header_h, chrome.footer_h
        );
    }

    /// The header and footer seams are grabbable where they are DRAWN, and
    /// only on the rail — a design-time affordance, so it exists here and
    /// nowhere else.
    #[test]
    fn both_sidebar_seams_are_grabbable_where_they_are_drawn() {
        let dp = sidebar_form();

        // Header seam at y = 120, footer seam at y = 744 - 72 = 672.
        assert_eq!(
            dp.sidebar_seam_at(100, 120).map(|(_, p)| p),
            Some(SidebarPane::Header)
        );
        assert_eq!(
            dp.sidebar_seam_at(100, 672).map(|(_, p)| p),
            Some(SidebarPane::Footer)
        );
        // Within tolerance either side, so the seam is not a one-pixel target.
        assert!(dp.sidebar_seam_at(100, 116).is_some());
        assert!(dp.sidebar_seam_at(100, 676).is_some());
        // …but not in the middle of the menu, nor off the rail.
        assert!(dp.sidebar_seam_at(100, 400).is_none(), "the menu is not a seam");
        assert!(dp.sidebar_seam_at(500, 120).is_none(), "right of the rail");

        // Nothing is offered unless the sidebar is the selected control, so a
        // seam can never steal a drag from something sitting over the rail.
        let mut unselected = sidebar_form();
        unselected.set_selected_one(None);
        assert!(unselected.sidebar_seam_at(100, 120).is_none());

        println!(
            "049 seams — header at y=120 and footer at y=672 grabbable within \
             ±{SIDEBAR_SEAM_TOL:.0}px on the rail only, and only while it is \
             selected"
        );
    }

    /// The grips take the MAXIMUM contrast available on whatever rail the
    /// developer designed — black or white, never a mid accent that merely
    /// clears the 3:1 graphics minimum and still vanishes at a glance.
    #[test]
    fn the_seam_grips_take_the_highest_contrast_available() {
        use cobolt_forms::paint::contrast_ratio;

        // The rule the painter applies, in one place so the test checks the
        // real thing rather than a paraphrase of it.
        let ink = |bg: Color32| {
            if contrast_ratio(Color32::WHITE, bg) >= contrast_ratio(Color32::BLACK, bg) {
                Color32::WHITE
            } else {
                Color32::BLACK
            }
        };

        // Every rail the operator has actually used this session, plus the two
        // extremes. All must clear the 7:1 the WCAG asks of enhanced text —
        // far above the 3:1 minimum an accent was scraping past.
        for (name, bg) in [
            ("black", Color32::BLACK),
            ("slate", Color32::from_rgb(0x2E, 0x31, 0x38)),
            ("navy", Color32::from_rgb(20, 22, 45)),
            ("paper", Color32::from_rgb(0xF6, 0xF6, 0xF6)),
            ("white", Color32::WHITE),
        ] {
            let c = contrast_ratio(ink(bg), bg);
            assert!(c >= 7.0, "{name} rail: grip contrast {c:.1} is not high");
        }

        // A mid accent is exactly what fails here: on the slate rail it clears
        // the graphics minimum and nothing more, which is why the grips still
        // read as faint (operator, 2026-08-12).
        let slate = Color32::from_rgb(0x2E, 0x31, 0x38);
        let accent = Color32::from_rgb(90, 170, 255);
        assert!(
            contrast_ratio(accent, slate) < contrast_ratio(ink(slate), slate),
            "the accent must be beaten by the high-contrast choice"
        );

        println!(
            "designer seam grips — slate rail: accent {:.1}:1 vs white \
             {:.1}:1; every rail tested clears 7:1",
            contrast_ratio(accent, slate),
            contrast_ratio(ink(slate), slate)
        );
    }

    /// Dragging a seam writes the developer's property, one undo entry for the
    /// whole drag — and the header grows down while the footer grows up.
    #[test]
    fn dragging_a_seam_sets_the_property_and_undoes_in_one_step() {
        let mut dp = sidebar_form();
        let h = |dp: &DesignerPanel, key: &str| -> i64 {
            dp.form
                .find_control("SideMenu-1")
                .and_then(|c| c.get_prop(key))
                .map(|v| v.as_i64())
                .unwrap_or(0)
        };

        // Header: drag the seam DOWN 40 ⇒ a taller header.
        dp.drag = DragState::ResizingSidebarPane {
            id: "SideMenu-1".into(),
            pane: SidebarPane::Header,
            orig_h: 120,
            start_y: 120,
        };
        dp.apply_sidebar_seam_drag(160);
        assert_eq!(h(&dp, "HeaderHeight"), 160, "the header grows downwards");
        dp.finish_sidebar_seam_drag();
        assert_eq!(h(&dp, "HeaderHeight"), 160);
        dp.undo();
        assert_eq!(h(&dp, "HeaderHeight"), 120, "one undo restores the drag");

        // Footer: drag the seam UP 30 ⇒ a taller footer.
        dp.drag = DragState::ResizingSidebarPane {
            id: "SideMenu-1".into(),
            pane: SidebarPane::Footer,
            orig_h: 72,
            start_y: 672,
        };
        // 72 + 30 = 102, snapped to the form's 8pt grid like every other drag
        // in the designer.
        dp.apply_sidebar_seam_drag(642);
        let footer = h(&dp, "FooterHeight");
        assert!(footer > 72, "the footer grows upwards, got {footer}");
        assert_eq!(footer % 8, 0, "and lands on the grid: {footer}");
        assert!((footer - 102).abs() < 8, "…nearest the drag: {footer}");

        // Neither pane goes negative, however far the pointer travels.
        dp.apply_sidebar_seam_drag(5_000);
        assert!(h(&dp, "FooterHeight") >= 0);

        println!(
            "049 seams — header 120→160 dragging down, footer 72→{footer} \
             dragging up (grid-snapped), floored at 0, one undo entry per drag"
        );
    }

    /// 049 — the state the canvas SHOWS the rail in is not the property.
    ///
    /// `Collapsed` was doing two jobs at once: the state the finished
    /// application opens in, and the state on screen. So the designer's toggle
    /// had to rewrite the developer's design to show them anything, and a rail
    /// switched to collapsed kept its full designed width and merely laid
    /// collapsed content out inside it.
    #[test]
    fn the_canvas_shows_the_rail_in_a_state_of_its_own() {
        let mut dp = sidebar_form();
        let designed_w = dp.form.find_control("SideMenu-1").unwrap().rect.w;
        dp.form.sync_side_menu_footer_panels();

        // With nothing toggled the canvas follows the property.
        assert!(!dp.rail_shown_collapsed(), "designed open ⇒ shown open");
        assert!(
            dp.rail_view_controls(&dp.form.controls.clone()).is_none(),
            "and there is nothing to override"
        );

        // The toggle flips the VIEW and writes nothing.
        dp.rail_view_collapsed = Some(true);
        assert!(dp.rail_shown_collapsed());
        assert!(
            !dp.form.find_control("SideMenu-1").unwrap().side_menu_collapsed(),
            "the developer's `Collapsed` is untouched by the canvas toggle"
        );
        assert_eq!(
            dp.form.find_control("SideMenu-1").unwrap().rect.w,
            designed_w,
            "…and so is the designed width"
        );

        // What is DRAWN is the narrow rail — and its footer Panel with it.
        let drawn = dp
            .rail_view_controls(&dp.form.controls.clone())
            .expect("a collapsed view overrides the paint list");
        let side = drawn.iter().find(|c| c.id == "SideMenu-1").unwrap();
        assert_eq!(
            side.rect.w as f32,
            cobolt_forms::sidebar::COLLAPSED_WIDTH,
            "a rail shown collapsed is drawn at the collapsed width"
        );
        assert!(side.side_menu_collapsed(), "…in the collapsed state");
        let footer_id = cobolt_forms::model::side_menu_footer_id("SideMenu-1");
        let footer = drawn.iter().find(|c| c.id == footer_id).unwrap();
        assert_eq!(
            footer.rect.w, side.rect.w,
            "the footer Panel is pinned to the rail's column, so it narrows too"
        );

        // Editing `Collapsed` itself takes the view back from the toggle.
        dp.form
            .find_control_mut("SideMenu-1")
            .unwrap()
            .set_prop("Collapsed", true);
        dp.sync_rail_view();
        assert_eq!(
            dp.rail_view_collapsed, None,
            "an inspector edit drops the canvas override"
        );
        assert!(dp.rail_shown_collapsed(), "…and the property is what shows");

        println!(
            "049 rail view — designed Collapsed=false, toggle ⇒ shown collapsed \
             at {}pt (designed {designed_w}pt kept, property untouched); footer \
             Panel narrows with the rail; an inspector edit resets the override",
            cobolt_forms::sidebar::COLLAPSED_WIDTH
        );
    }

    /// 050 AC5/R7/R8 — a self-contained theme writes NOTHING to the model.
    ///
    /// `apply_glass_style_defaults` rewrites background colours, gradient flags
    /// and per-control shadow properties. Running it for a setting the active
    /// theme ignores means the developer's `.cfrm` quietly accumulates values
    /// they never chose — and switching back to Liquid Glass then shows a form
    /// that is not the one they had.
    #[test]
    fn a_self_contained_theme_writes_nothing_to_the_model() {
        use cobolt_forms::model::GlassStyle;

        fn snapshot(dp: &DesignerPanel) -> Vec<String> {
            let mut v = vec![
                format!("bg={}", dp.form.background_color),
                format!("grad={}", dp.form.background_gradient_enabled),
                format!("grad_start={}", dp.form.background_gradient_start_color),
                format!("grad_end={}", dp.form.background_gradient_end_color),
            ];
            for c in &dp.form.controls {
                for k in ["BackgroundColor", "ShadowEnabled", "ShadowColor", "ShadowOpacity"] {
                    v.push(format!(
                        "{}.{k}={:?}",
                        c.id,
                        c.get_prop(k).map(|p| p.as_str().to_owned())
                    ));
                }
            }
            v
        }

        const STYLES: [&str; 4] =
            ["Classic", "Enhanced", "Neumorphic Light", "Neumorphic Dark"];

        // ── Self-contained: the model must not move ──────────────────────────
        let mut dp = sidebar_form();
        dp.active_surface_theme = cobolt_forms::surface_theme::elegance();
        let before = snapshot(&dp);
        for s in STYLES {
            dp.set_form_prop_direct("GlassStyle", s.to_owned());
        }
        let after = snapshot(&dp);
        assert_eq!(
            before, after,
            "a self-contained theme rewrote the developer's form"
        );
        // The choice is still STORED — it is inert, not refused (R18).
        assert_eq!(
            dp.form.glass_style,
            GlassStyle::from_str("Neumorphic Dark"),
            "the last pick is remembered, so switching back restores it"
        );
        assert!(
            dp.neumorphic_seed().is_none(),
            "…and a new control is not seeded with glass defaults either"
        );

        // ── Liquid Glass: unchanged behaviour, so the fix is surgical ────────
        let mut lg = sidebar_form();
        lg.active_surface_theme = cobolt_forms::surface_theme::liquid_glass();
        let lg_before = snapshot(&lg);
        lg.set_form_prop_direct("GlassStyle", "Neumorphic Light".to_owned());
        assert_ne!(
            lg_before,
            snapshot(&lg),
            "Liquid Glass must still apply its defaults (R21)"
        );
        assert!(lg.neumorphic_seed().is_some(), "…and still seed new controls");

        println!(
            "\n  050 AC5 — {} properties watched across {} glass-style changes\n  \
             self-contained: 0 changed (choice stored: {:?})\n  \
             liquid glass:   model updated as before\n",
            before.len(),
            STYLES.len(),
            dp.form.glass_style
        );
    }
}

#[cfg(test)]
mod menu_editor_051_tests {
    use super::*;

    /// 051 R16/R17 — the action encodings round-trip through the modal's
    /// classifier, and the combo offers 5 choices for a SideMenu's menu but
    /// the classic 3 for a MenuBar's.
    #[test]
    fn action_encodings_round_trip_and_the_combo_gates_by_control() {
        use cobolt_forms::menu::MenuItem;
        let item = |a: Option<&str>| MenuItem {
            action: a.map(str::to_string),
            ..MenuItem::new_action("x", "X")
        };
        let cases = [
            (Some("open-form:CRM"), "open-form"),
            (Some("open-standalone-sync:REPORT"), "open-standalone-sync"),
            (Some("open-standalone-async:MONITOR"), "open-standalone-async"),
            (Some("close-application"), "close"),
            (Some("home"), "home"),
            (Some("event"), "event"),
            (None, "event"),
            (Some("mystery:thing"), "event"),
        ];
        for (action, want) in cases {
            assert_eq!(
                MenuEditorModal::action_type_of(&item(action)),
                want,
                "{action:?}"
            );
        }
        // The classifier's kind maps back to the exact persisted prefix.
        assert_eq!(MenuEditorModal::action_prefix("open-form"), Some("open-form:"));
        assert_eq!(
            MenuEditorModal::action_prefix("open-standalone-sync"),
            Some("open-standalone-sync:")
        );
        assert_eq!(
            MenuEditorModal::action_prefix("open-standalone-async"),
            Some("open-standalone-async:")
        );
        assert_eq!(MenuEditorModal::action_prefix("event"), None);
        assert_eq!(MenuEditorModal::action_prefix("close"), None);
        // Home names no form, so it must never grow a Target picker.
        assert_eq!(MenuEditorModal::action_prefix("home"), None);

        let side = MenuEditorModal::action_type_options(true);
        assert_eq!(side.len(), 6, "a SideMenu's menu offers six actions");
        assert!(side.contains(&"home"), "Home is offered on a SideMenu: {side:?}");
        assert!(
            !MenuEditorModal::action_type_options(false).contains(&"home"),
            "Home restores the shell's ContentPane, which a MenuBar form has not got"
        );
        assert_eq!(
            MenuEditorModal::action_type_options(false),
            vec!["event", "open-form", "close"],
            "a MenuBar's keeps the classic three"
        );
        println!(
            "051 menu editor — 7/7 encodings classified, 3 prefixes mapped, \
             combo 5 (SideMenu) vs 3 (MenuBar)"
        );
    }

    /// 051 R25 — the picker filter's source of truth: each `form-format`
    /// yields the right (embeddable, standaloneable) pair, a pre-049 file is
    /// Standalone, and an unreadable file appears in BOTH lists.
    #[test]
    fn cfrm_load_paths_cover_all_format_cases() {
        let dir = std::env::temp_dir().join(format!(
            "prc-051-loadpaths-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let write = |name: &str, fmt: Option<&str>| {
            let attr = fmt
                .map(|f| format!(" form-format=\"{f}\""))
                .unwrap_or_default();
            let p = dir.join(name);
            std::fs::write(
                &p,
                format!("<Form name=\"X\" title=\"X\" width=\"100\" height=\"100\"{attr}></Form>"),
            )
            .unwrap();
            p
        };
        let cases = [
            (write("emb.cfrm", Some("Embedded")), (true, false)),
            (write("std.cfrm", Some("Standalone")), (false, true)),
            (write("both.cfrm", Some("Both")), (true, true)),
            (write("old.cfrm", None), (false, true)),
            (dir.join("missing.cfrm"), (true, true)),
        ];
        for (path, want) in &cases {
            assert_eq!(
                DesignerPanel::cfrm_load_paths(path),
                *want,
                "{}",
                path.display()
            );
        }
        let _ = std::fs::remove_dir_all(&dir);
        println!(
            "051 picker filter — 5/5 format cases: Embedded (embed only), \
             Standalone + pre-049 (standalone only), Both (both), unreadable (both)"
        );
    }
}
