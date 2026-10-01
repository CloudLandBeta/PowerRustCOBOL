// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! **The layout defaults table** (spec 056 R85).
//!
//! Every number or choice that shapes a responsive layout — default anchors,
//! docking, size limits, flex/grid/flow container and item values, padding,
//! the breakpoint table, the font-scale limits, the smallest form — is a
//! property with a seeded default, and this file is the one place those
//! defaults are written. `Control::new`, `Form::new`, the loader's seeding and
//! the project's `[forms]` defaults all read them from here, and the rest of
//! `layout/` reads a property (falling back here), never a constant of its
//! own: `tests/layout_has_no_static_values.rs` holds the module to that.

use crate::layout::breakpoints::Breakpoint;
use crate::model::{ControlType, PropValue};

// ── Item properties: every visual control ───────────────────────────────────

/// The canvas drag-lock (what the boolean `Anchor` used to be — R34).
pub const LOCKED: bool = false;
/// `Top,Left` — a fixed offset from the parent's top-left: today's behaviour.
pub const ANCHOR: &str = "Top,Left";
/// A Responsive MenuBar spans the top of a responsive form (R43).
pub const ANCHOR_MENU_BAR: &str = "Top,Left,Right";
/// A StatusBar spans the bottom of a responsive form (R43).
pub const ANCHOR_STATUS_BAR: &str = "Bottom,Left,Right";
pub const DOCK: &str = "None";
/// `MinWidth`/`MinHeight`/`MaxWidth`/`MaxHeight` — 0 means no limit (R16).
pub const SIZE_LIMIT: i64 = 0;
pub const FLEX_GROW: &str = "0";
pub const FLEX_SHRINK: &str = "1";
pub const FLEX_BASIS: &str = "Auto";
pub const ALIGN_SELF: &str = "Auto";
pub const ORDER: i64 = 0;
/// `GridColumn`/`GridRow` — 0 is auto-placed (R56).
pub const GRID_CELL: i64 = 0;
pub const GRID_SPAN: i64 = 1;
pub const JUSTIFY_SELF: &str = "Auto";
pub const FLOW_BREAK: bool = false;
pub const SCALE_FONT: bool = true;
/// `MinFontSize`/`MaxFontSize` — 0 means no limit (R69).
pub const FONT_SIZE_LIMIT: i64 = 0;
/// `Padding` — uniform, in form pixels (R51). Already seeded on every control.
pub const PADDING: i64 = 0;
/// `PaddingLeft/Top/Right/Bottom` — empty means "use `Padding`".
pub const PADDING_SIDE: &str = "";

// ── Container properties: the form, Panel, GroupBox, TabControl ─────────────

pub const LAYOUT_MODE: &str = "Absolute";
pub const FLEX_DIRECTION: &str = "Row";
pub const FLEX_WRAP: &str = "NoWrap";
pub const JUSTIFY_CONTENT: &str = "Start";
pub const ALIGN_ITEMS: &str = "Stretch";
pub const ALIGN_CONTENT: &str = "Stretch";
pub const GAP: i64 = 0;
/// `RowGap`/`ColumnGap` — empty means "use `Gap`".
pub const AXIS_GAP: &str = "";
pub const GRID_TRACKS: &str = "";
pub const JUSTIFY_ITEMS: &str = "Stretch";
pub const FLOW_DIRECTION: &str = "LeftToRight";
pub const WRAP_CONTENTS: bool = true;

// ── Units and tolerances of the flex, flow and grid solvers ─────────────────

/// A percentage is of this whole: `50%` of a 400 px container is 200 px.
pub const PERCENT: f32 = 100.0;
/// Two lengths closer than this are the same length — where the flex freeze
/// loop stops and where a wrapping line is judged full.
pub const EPSILON: f32 = 0.01;

// ── Form properties ─────────────────────────────────────────────────────────

pub const FONT_SCALING: &str = "None";
pub const MIN_FONT_SCALE: &str = "0.85";
pub const MAX_FONT_SCALE: &str = "1.5";
/// Spec 081 — `ObsoleteScalingStyle`: 0 is off (spec-056 layout); otherwise
/// a set of these flags.
pub const OBSOLETE_SCALING_STYLE: i64 = 0;
pub const SCALING_RESIZE: i64 = 1;
pub const SCALING_REPOSITION: i64 = 2;
pub const SCALING_FONT: i64 = 4;
/// Every flag at once: the largest value the property takes.
pub const SCALING_STYLE_MAX: i64 = SCALING_RESIZE | SCALING_REPOSITION | SCALING_FONT;
/// The smallest surface a responsive form lays out for (R18).
pub const MIN_FORM_WIDTH: i64 = 64;
pub const MIN_FORM_HEIGHT: i64 = 64;
/// A run-form window's minimum inner size is never below this, whatever
/// `MinFormWidth`/`MinFormHeight` say (R18).
pub const WINDOW_MIN_INNER: f32 = 64.0;
/// A breakpoint's font factor when it states none (R67).
pub const BREAKPOINT_FONT_FACTOR: f32 = 1.0;

/// The default breakpoint table (R58): name, minimum available width, font
/// factor.
pub const BREAKPOINTS: [(&str, i64, f32); 3] = [
    ("Compact", 0, 1.0),
    ("Medium", 600, 1.0),
    ("Expanded", 1024, 1.0),
];

// ── The engine's own font bounds (R69: "and to the engine's existing bounds") ─

/// The size a control with no `FontSize` paints at.
pub const FONT_SIZE_MISSING: f32 = 11.0;
pub const FONT_SIZE_MIN: f32 = 4.0;
pub const FONT_SIZE_MAX: f32 = 200.0;

// ── Tables ──────────────────────────────────────────────────────────────────

/// The anchor a control of `ct` starts with (R7, R43).
pub fn anchor_default(ct: &ControlType) -> &'static str {
    match ct {
        ControlType::MenuBar => ANCHOR_MENU_BAR,
        ControlType::StatusBar => ANCHOR_STATUS_BAR,
        _ => ANCHOR,
    }
}

/// The item properties every visual control carries, with their defaults.
/// `Padding` is not here: `Control::new` seeds it already (it predates this
/// feature), and it keeps its own seeding.
pub fn item_defaults(ct: &ControlType) -> Vec<(&'static str, PropValue)> {
    let s = |v: &str| PropValue::String(v.to_owned());
    vec![
        ("Locked", PropValue::Bool(LOCKED)),
        ("Anchor", s(anchor_default(ct))),
        ("Dock", s(DOCK)),
        ("MinWidth", PropValue::Int(SIZE_LIMIT)),
        ("MinHeight", PropValue::Int(SIZE_LIMIT)),
        ("MaxWidth", PropValue::Int(SIZE_LIMIT)),
        ("MaxHeight", PropValue::Int(SIZE_LIMIT)),
        ("FlexGrow", s(FLEX_GROW)),
        ("FlexShrink", s(FLEX_SHRINK)),
        ("FlexBasis", s(FLEX_BASIS)),
        ("AlignSelf", s(ALIGN_SELF)),
        ("Order", PropValue::Int(ORDER)),
        ("GridColumn", PropValue::Int(GRID_CELL)),
        ("GridRow", PropValue::Int(GRID_CELL)),
        ("ColumnSpan", PropValue::Int(GRID_SPAN)),
        ("RowSpan", PropValue::Int(GRID_SPAN)),
        ("JustifySelf", s(JUSTIFY_SELF)),
        ("FlowBreak", PropValue::Bool(FLOW_BREAK)),
        ("ScaleFont", PropValue::Bool(SCALE_FONT)),
        ("MinFontSize", PropValue::Int(FONT_SIZE_LIMIT)),
        ("MaxFontSize", PropValue::Int(FONT_SIZE_LIMIT)),
        ("PaddingLeft", s(PADDING_SIDE)),
        ("PaddingTop", s(PADDING_SIDE)),
        ("PaddingRight", s(PADDING_SIDE)),
        ("PaddingBottom", s(PADDING_SIDE)),
    ]
}

/// The container properties the form and every container carry.
pub fn container_defaults() -> Vec<(&'static str, PropValue)> {
    let s = |v: &str| PropValue::String(v.to_owned());
    vec![
        ("LayoutMode", s(LAYOUT_MODE)),
        ("FlexDirection", s(FLEX_DIRECTION)),
        ("FlexWrap", s(FLEX_WRAP)),
        ("JustifyContent", s(JUSTIFY_CONTENT)),
        ("AlignItems", s(ALIGN_ITEMS)),
        ("AlignContent", s(ALIGN_CONTENT)),
        ("Gap", PropValue::Int(GAP)),
        ("RowGap", s(AXIS_GAP)),
        ("ColumnGap", s(AXIS_GAP)),
        ("GridColumns", s(GRID_TRACKS)),
        ("GridRows", s(GRID_TRACKS)),
        ("JustifyItems", s(JUSTIFY_ITEMS)),
        ("FlowDirection", s(FLOW_DIRECTION)),
        ("WrapContents", PropValue::Bool(WRAP_CONTENTS)),
    ]
}

/// The form's own layout bag: its container properties, its padding, and the
/// form-only font-scaling and minimum-size properties.
pub fn form_defaults() -> Vec<(&'static str, PropValue)> {
    let s = |v: &str| PropValue::String(v.to_owned());
    let mut out = container_defaults();
    out.extend([
        ("Padding", PropValue::Int(PADDING)),
        ("PaddingLeft", s(PADDING_SIDE)),
        ("PaddingTop", s(PADDING_SIDE)),
        ("PaddingRight", s(PADDING_SIDE)),
        ("PaddingBottom", s(PADDING_SIDE)),
        ("FontScaling", s(FONT_SCALING)),
        ("MinFontScale", s(MIN_FONT_SCALE)),
        ("MaxFontScale", s(MAX_FONT_SCALE)),
        ("ObsoleteScalingStyle", PropValue::Int(OBSOLETE_SCALING_STYLE)),
        ("MinFormWidth", PropValue::Int(MIN_FORM_WIDTH)),
        ("MinFormHeight", PropValue::Int(MIN_FORM_HEIGHT)),
    ]);
    out
}

/// The default of layout property `key` on a control of type `ct`, if it is
/// one this table owns — the fallback of every typed getter in `props.rs`.
pub fn control_default(ct: &ControlType, key: &str) -> Option<PropValue> {
    item_defaults(ct)
        .into_iter()
        .chain(container_defaults())
        .chain([("Padding", PropValue::Int(PADDING))])
        .find(|(k, _)| k.eq_ignore_ascii_case(key))
        .map(|(_, v)| v)
}

/// The default of form layout property `key`.
pub fn form_default(key: &str) -> Option<PropValue> {
    form_defaults()
        .into_iter()
        .find(|(k, _)| k.eq_ignore_ascii_case(key))
        .map(|(_, v)| v)
}

/// The default breakpoint table (R58), as the model stores it.
pub fn default_breakpoints() -> Vec<Breakpoint> {
    BREAKPOINTS
        .iter()
        .map(|(name, min_width, factor)| Breakpoint {
            name: (*name).to_owned(),
            min_width: *min_width,
            font_factor: *factor,
            overrides: Vec::new(),
        })
        .collect()
}
