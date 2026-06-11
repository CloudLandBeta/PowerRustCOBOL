// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Core data model: `Form`, `Control`, `ControlType`, `EventBinding`, `AnimationDef`.

use indexmap::IndexMap;

// ── Geometry ──────────────────────────────────────────────────────────────────

/// Bounding rectangle of a control in form-space pixels.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rect {
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
}

impl Rect {
    pub fn new(x: i32, y: i32, w: i32, h: i32) -> Self { Self { x, y, w, h } }

    pub fn contains(&self, px: i32, py: i32) -> bool {
        px >= self.x && px < self.x + self.w
            && py >= self.y && py < self.y + self.h
    }
}

impl Default for Rect {
    fn default() -> Self { Self::new(0, 0, 100, 30) }
}

// ── PropValue ─────────────────────────────────────────────────────────────────

/// The value of a control property.
#[derive(Debug, Clone, PartialEq)]
pub enum PropValue {
    String(String),
    Int(i64),
    Bool(bool),
}

impl PropValue {
    pub fn as_str(&self) -> &str {
        if let PropValue::String(s) = self { s } else { "" }
    }
    pub fn as_i64(&self) -> i64 {
        match self {
            PropValue::Int(n)  => *n,
            PropValue::Bool(b) => *b as i64,
            PropValue::String(s) => s.parse().unwrap_or(0),
        }
    }
    pub fn as_bool(&self) -> bool {
        match self {
            PropValue::Bool(b)   => *b,
            PropValue::Int(n)    => *n != 0,
            PropValue::String(s) => !s.is_empty() && s != "0" && s != "false",
        }
    }
    pub fn to_xml_string(&self) -> String {
        match self {
            PropValue::String(s) => s.clone(),
            PropValue::Int(n)    => n.to_string(),
            PropValue::Bool(b)   => if *b { "1".to_owned() } else { "0".to_owned() },
        }
    }
}

impl From<&str>   for PropValue { fn from(s: &str)  -> Self { PropValue::String(s.to_owned()) } }
impl From<String> for PropValue { fn from(s: String) -> Self { PropValue::String(s) } }
impl From<i64>    for PropValue { fn from(n: i64)    -> Self { PropValue::Int(n) } }
impl From<i32>    for PropValue { fn from(n: i32)    -> Self { PropValue::Int(n as i64) } }
impl From<bool>   for PropValue { fn from(b: bool)   -> Self { PropValue::Bool(b) } }

impl std::fmt::Display for PropValue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.to_xml_string())
    }
}

// ── Animation ─────────────────────────────────────────────────────────────────

/// What event triggers an animation.
#[derive(Debug, Clone, PartialEq)]
pub enum AnimTrigger {
    OnFormLoad,
    OnShow,
    OnHide,
    OnClick,
    OnHover,
    OnFocus,
    Programmatic,    // invoked by name via COBOL PERFORM or code
    OnTimer(String), // a specific Timer control ID fires it
}

impl AnimTrigger {
    pub fn as_str(&self) -> &str {
        match self {
            AnimTrigger::OnFormLoad     => "OnFormLoad",
            AnimTrigger::OnShow         => "OnShow",
            AnimTrigger::OnHide         => "OnHide",
            AnimTrigger::OnClick        => "OnClick",
            AnimTrigger::OnHover        => "OnHover",
            AnimTrigger::OnFocus        => "OnFocus",
            AnimTrigger::Programmatic   => "Programmatic",
            AnimTrigger::OnTimer(_)     => "OnTimer",
        }
    }
    pub fn from_str(s: &str) -> Self {
        match s {
            "OnFormLoad"   => AnimTrigger::OnFormLoad,
            "OnShow"       => AnimTrigger::OnShow,
            "OnHide"       => AnimTrigger::OnHide,
            "OnClick"      => AnimTrigger::OnClick,
            "OnHover"      => AnimTrigger::OnHover,
            "OnFocus"      => AnimTrigger::OnFocus,
            "Programmatic" => AnimTrigger::Programmatic,
            _              => AnimTrigger::OnFormLoad,
        }
    }
    pub const ALL: &'static [&'static str] = &[
        "OnFormLoad", "OnShow", "OnHide", "OnClick", "OnHover", "OnFocus", "Programmatic", "OnTimer",
    ];
}

/// The animation motion/effect kind.
#[derive(Debug, Clone, PartialEq)]
pub enum AnimKind {
    None,
    FlyFromLeft,
    FlyFromRight,
    FlyFromTop,
    FlyFromBottom,
    FlyFromTopLeft,
    FlyFromTopRight,
    FlyFromBottomLeft,
    FlyFromBottomRight,
    FadeIn,
    FadeOut,
    ZoomIn,
    ZoomOut,
    Bounce,
    Shake,
    Pulse,
    Spin,
    Flip,
    Slide { dx: i32, dy: i32 },
    Custom(String), // name of a keyframe set defined in project
}

impl AnimKind {
    pub fn as_str(&self) -> &str {
        match self {
            AnimKind::None              => "None",
            AnimKind::FlyFromLeft       => "FlyFromLeft",
            AnimKind::FlyFromRight      => "FlyFromRight",
            AnimKind::FlyFromTop        => "FlyFromTop",
            AnimKind::FlyFromBottom     => "FlyFromBottom",
            AnimKind::FlyFromTopLeft    => "FlyFromTopLeft",
            AnimKind::FlyFromTopRight   => "FlyFromTopRight",
            AnimKind::FlyFromBottomLeft => "FlyFromBottomLeft",
            AnimKind::FlyFromBottomRight=> "FlyFromBottomRight",
            AnimKind::FadeIn            => "FadeIn",
            AnimKind::FadeOut           => "FadeOut",
            AnimKind::ZoomIn            => "ZoomIn",
            AnimKind::ZoomOut           => "ZoomOut",
            AnimKind::Bounce            => "Bounce",
            AnimKind::Shake             => "Shake",
            AnimKind::Pulse             => "Pulse",
            AnimKind::Spin              => "Spin",
            AnimKind::Flip              => "Flip",
            AnimKind::Slide { .. }      => "Slide",
            AnimKind::Custom(n)         => n.as_str(),
        }
    }
    pub fn from_str(s: &str) -> Self {
        match s {
            "None"               => AnimKind::None,
            "FlyFromLeft"        => AnimKind::FlyFromLeft,
            "FlyFromRight"       => AnimKind::FlyFromRight,
            "FlyFromTop"         => AnimKind::FlyFromTop,
            "FlyFromBottom"      => AnimKind::FlyFromBottom,
            "FlyFromTopLeft"     => AnimKind::FlyFromTopLeft,
            "FlyFromTopRight"    => AnimKind::FlyFromTopRight,
            "FlyFromBottomLeft"  => AnimKind::FlyFromBottomLeft,
            "FlyFromBottomRight" => AnimKind::FlyFromBottomRight,
            "FadeIn"             => AnimKind::FadeIn,
            "FadeOut"            => AnimKind::FadeOut,
            "ZoomIn"             => AnimKind::ZoomIn,
            "ZoomOut"            => AnimKind::ZoomOut,
            "Bounce"             => AnimKind::Bounce,
            "Shake"              => AnimKind::Shake,
            "Pulse"              => AnimKind::Pulse,
            "Spin"               => AnimKind::Spin,
            "Flip"               => AnimKind::Flip,
            _                    => AnimKind::None,
        }
    }
    pub const ALL: &'static [&'static str] = &[
        "None", "FlyFromLeft", "FlyFromRight", "FlyFromTop", "FlyFromBottom",
        "FlyFromTopLeft", "FlyFromTopRight", "FlyFromBottomLeft", "FlyFromBottomRight",
        "FadeIn", "FadeOut", "ZoomIn", "ZoomOut",
        "Bounce", "Shake", "Pulse", "Spin", "Flip",
    ];
}

/// Easing function for animations.
#[derive(Debug, Clone, PartialEq)]
pub enum EasingKind {
    Linear,
    EaseIn,
    EaseOut,
    EaseInOut,
    Bounce,
    Elastic,
    Back,
    Spring,
}

impl EasingKind {
    pub fn as_str(&self) -> &str {
        match self {
            EasingKind::Linear   => "Linear",
            EasingKind::EaseIn   => "EaseIn",
            EasingKind::EaseOut  => "EaseOut",
            EasingKind::EaseInOut=> "EaseInOut",
            EasingKind::Bounce   => "Bounce",
            EasingKind::Elastic  => "Elastic",
            EasingKind::Back     => "Back",
            EasingKind::Spring   => "Spring",
        }
    }
    pub fn from_str(s: &str) -> Self {
        match s {
            "Linear"   => EasingKind::Linear,
            "EaseIn"   => EasingKind::EaseIn,
            "EaseOut"  => EasingKind::EaseOut,
            "EaseInOut"=> EasingKind::EaseInOut,
            "Bounce"   => EasingKind::Bounce,
            "Elastic"  => EasingKind::Elastic,
            "Back"     => EasingKind::Back,
            "Spring"   => EasingKind::Spring,
            _          => EasingKind::EaseOut,
        }
    }
    pub const ALL: &'static [&'static str] = &[
        "Linear", "EaseIn", "EaseOut", "EaseInOut", "Bounce", "Elastic", "Back", "Spring",
    ];
    /// Evaluate easing at t ∈ [0,1].
    pub fn apply(&self, t: f32) -> f32 {
        match self {
            EasingKind::Linear    => t,
            EasingKind::EaseIn    => t * t,
            EasingKind::EaseOut   => t * (2.0 - t),
            EasingKind::EaseInOut => if t < 0.5 { 2.0*t*t } else { -1.0 + (4.0 - 2.0*t)*t },
            EasingKind::Bounce    => {
                let t = 1.0 - t;
                let r = if t < 1.0/2.75 { 7.5625*t*t }
                        else if t < 2.0/2.75 { let t=t-1.5/2.75; 7.5625*t*t+0.75 }
                        else if t < 2.5/2.75 { let t=t-2.25/2.75; 7.5625*t*t+0.9375 }
                        else { let t=t-2.625/2.75; 7.5625*t*t+0.984375 };
                1.0 - r
            }
            EasingKind::Elastic   => {
                if t == 0.0 || t == 1.0 { t }
                else { 2.0_f32.powf(-10.0*t) * ((t-0.075)*std::f32::consts::TAU/0.3).sin() + 1.0 }
            }
            EasingKind::Back      => { let c = 1.70158; t*t*((c+1.0)*t - c) }
            EasingKind::Spring    => {
                // damped spring approximation
                (1.0 - (-6.0*t).exp() * (8.0*t).cos()).clamp(0.0, 1.0)
            }
        }
    }
}

/// How many times an animation repeats.
#[derive(Debug, Clone, PartialEq)]
pub enum AnimRepeat {
    Once,
    Loop,
    PingPong,
    Count(u32),
}

impl AnimRepeat {
    pub fn as_str(&self) -> &str {
        match self {
            AnimRepeat::Once     => "Once",
            AnimRepeat::Loop     => "Loop",
            AnimRepeat::PingPong => "PingPong",
            AnimRepeat::Count(_) => "Count",
        }
    }
    pub const ALL: &'static [&'static str] = &["Once", "Loop", "PingPong", "Count"];
}

/// A single animation definition attached to a control or form.
#[derive(Debug, Clone)]
pub struct AnimationDef {
    /// Unique name for this animation (used by COBOL PERFORM to trigger it).
    pub name:        String,
    pub trigger:     AnimTrigger,
    pub kind:        AnimKind,
    pub duration_ms: u64,
    pub delay_ms:    u64,
    pub easing:      EasingKind,
    pub repeat:      AnimRepeat,
    /// When kind=Slide, the pixel offset from which the control enters.
    pub slide_dx:    i32,
    pub slide_dy:    i32,
}

impl AnimationDef {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name:        name.into(),
            trigger:     AnimTrigger::OnFormLoad,
            kind:        AnimKind::FlyFromLeft,
            duration_ms: 400,
            delay_ms:    0,
            easing:      EasingKind::EaseOut,
            repeat:      AnimRepeat::Once,
            slide_dx:    0,
            slide_dy:    0,
        }
    }
}

// ── EventBinding ──────────────────────────────────────────────────────────────

/// Maps a UI event to a COBOL nested-program handler.
///
/// `paragraph` is auto-derived via `derive_paragraph_name()` as `"CTRL-ID--EVENT-NAME"`.
/// `code` holds the raw COBOL statements (no paragraph header, no GOBACK) that
/// the user typed in the IDE's Code View for this event.
#[derive(Debug, Clone, PartialEq)]
pub struct EventBinding {
    pub event:     String,
    pub paragraph: String,  // auto-derived; kept for compat
    /// CDATA body — the user's COBOL statements for the PROCEDURE DIVISION body
    /// (no PROGRAM-ID, no GOBACK, no headers — just the statements).
    pub code:      String,
    /// Optional WORKING-STORAGE declarations local to this handler.
    /// Emitted verbatim into the nested program's DATA DIVISION WS section.
    pub local_ws:  String,
}

impl EventBinding {
    /// Create a new binding with an empty code body.
    pub fn new(event: impl Into<String>, paragraph: impl Into<String>) -> Self {
        Self { event: event.into(), paragraph: paragraph.into(), code: String::new(), local_ws: String::new() }
    }

    /// Create a binding and derive the paragraph name automatically from
    /// the control ID and event name: `"BTN-OK--CLICK"`.
    pub fn for_control(control_id: &str, event: impl Into<String>) -> Self {
        let ev = event.into();
        let para = derive_paragraph_name(control_id, &ev);
        Self { event: ev, paragraph: para, code: String::new(), local_ws: String::new() }
    }

    /// True if the user has written any code in this handler.
    pub fn has_code(&self) -> bool {
        !self.code.trim().is_empty()
    }

    /// Count non-blank lines in the code body (for UI display: "3 lines").
    pub fn code_line_count(&self) -> usize {
        self.code.lines().filter(|l| !l.trim().is_empty()).count()
    }
}

/// Derive the nested-program name for an event handler.
/// e.g. control_id="BTN-OK", event="Click"  →  "BTN-OK--CLICK"
pub fn derive_paragraph_name(control_id: &str, event: &str) -> String {
    format!(
        "{}--{}",
        control_id.to_ascii_uppercase(),
        event.to_ascii_uppercase().replace(' ', "-")
    )
}

// ── DeletedControlCode ────────────────────────────────────────────────────────

/// Preserves event code from a control that was deleted by the user.
/// Stored in the .cfrm XML under <deleted-controls> so it can be recovered.
/// Never emitted into the generated .cbl.
#[derive(Debug, Clone, PartialEq)]
pub struct DeletedControlCode {
    /// Original control ID (e.g. "BTN-OK").
    pub control_id:  String,
    /// ISO 8601 timestamp of when the control was deleted.
    pub deleted_at:  String,
    /// All event bindings that had code at the time of deletion.
    pub events:      Vec<EventBinding>,
}

// ── ControlType ───────────────────────────────────────────────────────────────

/// The type of a visual (or non-visual) control.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ControlType {
    // Core controls
    Button,
    TextBox,
    Label,
    CheckBox,
    RadioButton,
    ListBox,
    ComboBox,
    GroupBox,
    Panel,
    TabControl,
    DataGrid,
    PictureBox,
    ProgressBar,
    MenuBar,
    ToolBar,
    StatusBar,
    // Extended controls
    Line,
    DateTimePicker,
    NumericUpDown,
    TreeView,
    Splitter,
    Timer,
    Shape,
    // New controls
    Animator,      // Plays animated images (GIF / WebP / APNG)
    AgentObject,   // AI Agent (non-visual) — connects to local LLM
    ModalWindow,   // Modal dialog window — runs its own COBOL program
    RestClient,    // REST API client (non-visual) — INVOKE-based HTTP calls
    SqlDatabase,   // SQL database client (non-visual) — SQLx-backed open/query/fetch
    Slider,        // Horizontal or vertical slider with min/max/step/tick marks
    // Charts — each binds to a COBOL data structure (table/array) and supports INVOKE
    BarChart,      // Vertical / horizontal bar chart
    LineChart,     // Line / area line chart
    PieChart,      // Pie chart (360° sectors)
    AreaChart,     // Stacked or overlapping area chart
    ScatterChart,  // Scatter / bubble plot
    DonutChart,    // Donut (ring) chart
    // Plugin-provided
    Custom { plugin_id: String, control_id: String },
}

impl ControlType {
    pub fn as_str(&self) -> &str {
        match self {
            ControlType::Button           => "Button",
            ControlType::TextBox          => "TextBox",
            ControlType::Label            => "Label",
            ControlType::CheckBox         => "CheckBox",
            ControlType::RadioButton      => "RadioButton",
            ControlType::ListBox          => "ListBox",
            ControlType::ComboBox         => "ComboBox",
            ControlType::GroupBox         => "GroupBox",
            ControlType::Panel            => "Panel",
            ControlType::TabControl       => "TabControl",
            ControlType::DataGrid         => "DataGrid",
            ControlType::PictureBox       => "PictureBox",
            ControlType::Animator         => "Animator",
            ControlType::ProgressBar      => "ProgressBar",
            ControlType::MenuBar          => "MenuBar",
            ControlType::ToolBar          => "ToolBar",
            ControlType::StatusBar        => "StatusBar",
            ControlType::Line             => "Line",
            ControlType::DateTimePicker   => "DateTimePicker",
            ControlType::NumericUpDown    => "NumericUpDown",
            ControlType::TreeView         => "TreeView",
            ControlType::Splitter         => "Splitter",
            ControlType::Timer            => "Timer",
            ControlType::Shape            => "Shape",
            ControlType::AgentObject      => "AgentObject",
            ControlType::ModalWindow      => "ModalWindow",
            ControlType::RestClient       => "RestClient",
            ControlType::SqlDatabase      => "SqlDatabase",
            ControlType::Slider           => "Slider",
            ControlType::BarChart         => "BarChart",
            ControlType::LineChart        => "LineChart",
            ControlType::PieChart         => "PieChart",
            ControlType::AreaChart        => "AreaChart",
            ControlType::ScatterChart     => "ScatterChart",
            ControlType::DonutChart       => "DonutChart",
            ControlType::Custom { plugin_id, control_id } =>
                Box::leak(format!("{plugin_id}:{control_id}").into_boxed_str()),
        }
    }

    pub fn from_str(s: &str) -> Self {
        match s {
            "Button"        => ControlType::Button,
            "TextBox"       => ControlType::TextBox,
            "Label"         => ControlType::Label,
            "CheckBox"      => ControlType::CheckBox,
            "RadioButton"   => ControlType::RadioButton,
            "ListBox"       => ControlType::ListBox,
            "ComboBox"      => ControlType::ComboBox,
            "GroupBox"      => ControlType::GroupBox,
            "Panel"         => ControlType::Panel,
            "TabControl"    => ControlType::TabControl,
            "DataGrid"      => ControlType::DataGrid,
            "PictureBox"    => ControlType::PictureBox,
            "Animator"      => ControlType::Animator,
            "ProgressBar"   => ControlType::ProgressBar,
            "MenuBar"       => ControlType::MenuBar,
            "ToolBar"       => ControlType::ToolBar,
            "StatusBar"     => ControlType::StatusBar,
            "Line"          => ControlType::Line,
            "DateTimePicker"=> ControlType::DateTimePicker,
            "NumericUpDown" => ControlType::NumericUpDown,
            "TreeView"      => ControlType::TreeView,
            "Splitter"      => ControlType::Splitter,
            "Timer"         => ControlType::Timer,
            "Shape"         => ControlType::Shape,
            "AgentObject"   => ControlType::AgentObject,
            "ModalWindow"   => ControlType::ModalWindow,
            "RestClient"    => ControlType::RestClient,
            "SqlDatabase"   => ControlType::SqlDatabase,
            "Slider"        => ControlType::Slider,
            "BarChart"      => ControlType::BarChart,
            "LineChart"     => ControlType::LineChart,
            "PieChart"      => ControlType::PieChart,
            "AreaChart"     => ControlType::AreaChart,
            "ScatterChart"  => ControlType::ScatterChart,
            "DonutChart"    => ControlType::DonutChart,
            other => {
                if let Some((p, c)) = other.split_once(':') {
                    ControlType::Custom { plugin_id: p.to_owned(), control_id: c.to_owned() }
                } else {
                    ControlType::Custom { plugin_id: "unknown".to_owned(), control_id: other.to_owned() }
                }
            }
        }
    }

    pub fn default_size(&self) -> (i32, i32) {
        match self {
            ControlType::Button       => (80, 28),
            ControlType::TextBox      => (160, 24),
            ControlType::Label        => (120, 20),
            ControlType::CheckBox     => (120, 22),
            ControlType::RadioButton  => (120, 22),
            ControlType::ListBox      => (160, 100),
            ControlType::ComboBox     => (160, 24),
            ControlType::GroupBox     => (200, 120),
            ControlType::Panel        => (200, 150),
            ControlType::TabControl   => (300, 200),
            ControlType::DataGrid     => (300, 200),
            ControlType::PictureBox   => (120, 120),
            ControlType::Animator     => (160, 120),
            ControlType::ProgressBar  => (200, 22),
            ControlType::MenuBar      => (400, 24),
            ControlType::ToolBar      => (400, 32),
            ControlType::StatusBar    => (400, 22),
            ControlType::Line         => (200, 4),
            ControlType::DateTimePicker => (200, 24),
            ControlType::NumericUpDown  => (120, 24),
            ControlType::TreeView       => (200, 200),
            ControlType::Splitter       => (200, 8),
            ControlType::Timer          => (48,  48),
            ControlType::Shape          => (120, 80),
            ControlType::AgentObject    => (56,  56),
            ControlType::ModalWindow    => (300, 220),
            ControlType::RestClient     => (56,  56),
            ControlType::SqlDatabase    => (64,  64),
            ControlType::Slider         => (200, 36),
            ControlType::BarChart       => (320, 220),
            ControlType::LineChart      => (320, 220),
            ControlType::PieChart       => (240, 240),
            ControlType::AreaChart      => (320, 220),
            ControlType::ScatterChart   => (320, 220),
            ControlType::DonutChart     => (240, 240),
            ControlType::Custom {..}    => (100, 30),
        }
    }

    pub fn primary_event(&self) -> &str {
        match self {
            ControlType::Button         => "onClick",
            ControlType::TextBox        => "onChange",
            ControlType::CheckBox       => "onClick",
            ControlType::RadioButton    => "onClick",
            ControlType::ListBox        => "onClick",
            ControlType::ComboBox       => "onChange",
            ControlType::DateTimePicker => "onChange",
            ControlType::NumericUpDown  => "onChange",
            ControlType::TreeView       => "onNodeClick",
            ControlType::Timer          => "onTick",
            ControlType::AgentObject    => "onResponse",
            ControlType::ModalWindow    => "onClosed",
            ControlType::RestClient     => "onResponseReceived",
            ControlType::SqlDatabase    => "onQueryComplete",
            ControlType::Slider         => "onChange",
            ControlType::BarChart
            | ControlType::LineChart
            | ControlType::PieChart
            | ControlType::AreaChart
            | ControlType::ScatterChart
            | ControlType::DonutChart   => "onDataChanged",
            _                           => "onClick",
        }
    }

    pub fn supported_events(&self) -> &'static [&'static str] {
        match self {
            ControlType::Button      => &["onClick", "onDblClick", "onMouseEnter", "onMouseLeave", "onMouseDown", "onMouseUp"],
            ControlType::TextBox     => &["onChange", "onKeyPress", "onKeyDown", "onKeyUp", "onGotFocus", "onLostFocus", "onEnter", "onLeave"],
            ControlType::Label       => &["onClick", "onDblClick", "onMouseEnter", "onMouseLeave"],
            ControlType::CheckBox    => &["onClick", "onCheckedChanged"],
            ControlType::RadioButton => &["onClick", "onCheckedChanged"],
            ControlType::ListBox     => &["onClick", "onDblClick", "onChange", "onSelectedIndexChanged"],
            ControlType::ComboBox    => &["onChange", "onClick", "onSelectedIndexChanged", "onDropDown"],
            ControlType::DateTimePicker => &["onChange", "onGotFocus", "onLostFocus"],
            ControlType::NumericUpDown  => &["onChange", "onGotFocus", "onLostFocus"],
            ControlType::TreeView    => &["onNodeClick", "onNodeDblClick", "onNodeExpand", "onNodeCollapse", "onNodeChecked"],
            ControlType::Timer       => &["onTick"],
            ControlType::PictureBox  => &["onClick", "onDblClick", "onMouseEnter", "onMouseLeave"],
            ControlType::Animator    => &["onClick", "onDblClick", "onStarted", "onEnded"],
            ControlType::DataGrid    => &["onCellClick", "onCellChange", "onRowSelect", "onColumnClick", "onExportCSV"],
            ControlType::AgentObject => &["onResponse", "onError", "onStreamChunk", "onThinking"],
            ControlType::ModalWindow => &["onClosed", "onLoaded", "onConfirmed", "onCancelled"],
            ControlType::RestClient   => &["onResponseReceived", "onError", "onTimeout", "onProgress"],
            ControlType::SqlDatabase  => &["onQueryComplete", "onConnectOk", "onConnectError", "onQueryError", "onRowFetched"],
            ControlType::Slider       => &["onChange", "onMouseUp", "onGotFocus", "onLostFocus"],
            ControlType::BarChart
            | ControlType::LineChart
            | ControlType::PieChart
            | ControlType::AreaChart
            | ControlType::ScatterChart
            | ControlType::DonutChart => &["onDataChanged", "onClick", "onSeriesClick", "onTooltipShow"],
            _                        => &["onClick"],
        }
    }

    /// Returns true for controls that are invisible at runtime (shown as icon boxes in designer).
    pub fn is_non_visual(&self) -> bool {
        matches!(self,
            ControlType::Timer | ControlType::AgentObject |
            ControlType::RestClient | ControlType::SqlDatabase)
    }
}

// ── Form events ─────────────────────────────────────────────────────────────────

/// The events the **form** itself supports, grouped by category (display order).
/// A handler binding is created lazily when the user first attaches code to one;
/// `onLoad` / `onClose` are pre-stubbed by `Form::new`. (Not all are wired into
/// the runtime/codegen yet — they are designable now, fired as support lands.)
pub const FORM_EVENT_GROUPS: &[(&str, &[&str])] = &[
    ("Lifecycle", &[
        "onCreate", "onInitialize", "onLoad", "onOpened", "onShow",
        "onHide", "onClose", "onClosing", "onClosed", "onDestroy",
    ]),
    ("Activation & Focus", &[
        "onActivate", "onActivated", "onDeactivate", "onDeactivated",
        "onGotFocus", "onLostFocus",
    ]),
    ("Window State", &[
        "onResize", "onResizing", "onMove", "onMoving", "onMinimize",
        "onMaximize", "onRestore", "onFullscreen", "onExitFullscreen",
    ]),
    ("Layout & Painting", &[
        "onLayout", "onPaint", "onRepaint", "onThemeChanged",
        "onDpiChanged", "onFontChanged",
    ]),
    ("Mouse", &[
        "onClick", "onDoubleClick", "onMouseDown", "onMouseUp", "onMouseMove",
        "onMouseEnter", "onMouseLeave", "onMouseWheel", "onContextMenu",
    ]),
    ("Touch & Pointer", &[
        "onPointerDown", "onPointerUp", "onPointerMove", "onPointerEnter",
        "onPointerLeave", "onPointerCancel", "onGesture",
    ]),
    ("Scrolling", &[
        "onScroll", "onScrollStart", "onScrollEnd",
        "onHorizontalScroll", "onVerticalScroll",
    ]),
    ("Drag & Drop", &[
        "onDragEnter", "onDragLeave", "onDragOver", "onDrop",
    ]),
    ("Clipboard", &[
        "onCut", "onCopy", "onPaste",
    ]),
    ("System / OS", &[
        "onSystemColorChanged", "onDisplayChanged", "onPowerSuspend",
        "onPowerResume", "onSessionLock", "onSessionUnlock",
    ]),
    ("Error Handling", &[
        "onUnhandledException",
    ]),
];

/// Flat iterator over every supported form event name (across all groups).
pub fn form_supported_events() -> impl Iterator<Item = &'static str> {
    FORM_EVENT_GROUPS.iter().flat_map(|(_, evs)| evs.iter().copied())
}

// ── Control ───────────────────────────────────────────────────────────────────

/// A single visual (or non-visual) control on a form.
#[derive(Debug, Clone)]
pub struct Control {
    pub id:           String,
    pub control_type: ControlType,
    pub rect:         Rect,
    pub tab_order:    u32,
    /// Z-order: higher = drawn on top. 0 = bottommost. Negative values allowed.
    pub z_order:      i32,
    pub visible:      bool,
    pub enabled:      bool,
    pub properties:   IndexMap<String, PropValue>,
    pub events:       Vec<EventBinding>,
    pub children:     Vec<Control>,
    /// Animation definitions for this control.
    pub animations:   Vec<AnimationDef>,
}

impl Control {
    pub fn new(id: impl Into<String>, control_type: ControlType, x: i32, y: i32) -> Self {
        let (w, h) = control_type.default_size();
        let mut props = IndexMap::new();

        let id_str = id.into();
        // Controls whose widget intrinsically shows a text label.
        let has_caption = matches!(
            control_type,
            ControlType::Label
            | ControlType::Button
            | ControlType::CheckBox
            | ControlType::RadioButton
            | ControlType::GroupBox
        );
        if has_caption {
            props.insert("Caption".to_owned(), PropValue::from(id_str.clone()));
        }

        // ── Universal appearance props ─────────────────────────────────────────
        props.insert("BackgroundColor".into(),       PropValue::String("#F0F0F0".into()));
        props.insert("ForegroundColor".into(),       PropValue::String("#000000".into()));
        props.insert("FontName".into(),        PropValue::String("Arial".into()));
        props.insert("FontSize".into(),        PropValue::Int(10));
        props.insert("Bold".into(),            PropValue::Bool(false));
        props.insert("Italic".into(),          PropValue::Bool(false));
        props.insert("Underline".into(),       PropValue::Bool(false));
        props.insert("Strikethrough".into(),   PropValue::Bool(false));

        // ── Layout & behaviour ────────────────────────────────────────────────
        props.insert("Tooltip".into(),     PropValue::String("".into()));
        props.insert("Cursor".into(),      PropValue::String("Default".into()));
        props.insert("Dock".into(),        PropValue::String("None".into()));
        props.insert("Anchor".into(),      PropValue::String("Top,Left".into()));
        props.insert("Padding".into(),     PropValue::Int(0));
        props.insert("Opacity".into(),     PropValue::Int(100));

        // ── Drop shadow ───────────────────────────────────────────────────────
        props.insert("ShadowEnabled".into(),      PropValue::Bool(false));
        props.insert("ShadowOpacity".into(),      PropValue::Int(20));   // 0-100 %
        props.insert("ShadowColor".into(),        PropValue::String("#000000".into()));
        props.insert("ShadowDirection".into(),    PropValue::String("South".into())); // N/NE/E/SE/S/SW/W/NW
        props.insert("ShadowDistance".into(),     PropValue::Int(7));    // px
        props.insert("ShadowBlur".into(),         PropValue::Bool(true)); // enable soft-blur falloff
        props.insert("ShadowBlurStrength".into(), PropValue::Int(8));    // 0-20, blur radius in layers

        // ── Identification (z-order, label association) ────────────────────────
        props.insert("ZOrder".into(),      PropValue::Int(0));
        props.insert("LabelFor".into(),    PropValue::String("".into())); // ID of associated Label

        // ── Data binding (all controls) ────────────────────────────────────────
        props.insert("DataItem".into(),    PropValue::String("".into()));
        props.insert("DataFormat".into(),  PropValue::String("".into()));

        // ── Type-specific props ────────────────────────────────────────────────
        match &control_type {
            ControlType::TextBox => {
                props.insert("Text".into(),         PropValue::String("".into()));
                props.insert("HintText".into(),     PropValue::String("".into()));
                props.insert("MaximumLength".into(),    PropValue::Int(0));
                props.insert("Multiline".into(),    PropValue::Bool(false));
                props.insert("PasswordCharacter".into(), PropValue::String("".into()));
                props.insert("ReadOnly".into(),     PropValue::Bool(false));
                props.insert("ScrollBars".into(),   PropValue::String("None".into()));
                props.insert("WordWrap".into(),     PropValue::Bool(true));
                props.insert("BorderStyle".into(),  PropValue::String("Fixed3D".into()));
                props.insert("BorderColor".into(),  PropValue::String("#AAAAAA".into()));
            }
            ControlType::Label => {
                props.insert("TextAlignment".into(),  PropValue::String("Left".into()));
                props.insert("WordWrap".into(),   PropValue::Bool(false));
                props.insert("AutoSize".into(),   PropValue::Bool(false));
                props.insert("BorderStyle".into(),PropValue::String("None".into()));
            }
            ControlType::CheckBox | ControlType::RadioButton => {
                props.insert("Checked".into(),    PropValue::Bool(false));
                props.insert("GroupName".into(),  PropValue::String("".into()));
                props.insert("CheckAlignment".into(), PropValue::String("Left".into()));
                props.insert("CheckColor".into(), PropValue::String("#0078D7".into()));
            }
            ControlType::PictureBox => {
                props.insert("ImagePath".into(),  PropValue::String("".into()));
                props.insert("SizeMode".into(),   PropValue::String("Normal".into()));
                props.insert("ImageAlignment".into(), PropValue::String("MiddleCenter".into()));
                props.insert("BorderStyle".into(),PropValue::String("None".into()));
                props.insert("BorderColor".into(),PropValue::String("#888888".into()));
                // When false, the surrounding frame/background is not drawn — only
                // the image shows (transparent PNG areas reveal what's behind).
                props.insert("ShowFrame".into(),  PropValue::Bool(true));
            }
            ControlType::Animator => {
                // Plays an animated image (GIF / WebP / APNG) or a still image.
                props.insert("Source".into(),     PropValue::String("".into()));
                props.insert("AutoPlay".into(),   PropValue::Bool(true));
                props.insert("Loop".into(),       PropValue::Bool(true));
                props.insert("SizeMode".into(),   PropValue::String("Fit".into()));
                props.insert("BackgroundColor".into(),  PropValue::String("#00000000".into()));
                props.insert("BorderStyle".into(),PropValue::String("None".into()));
                props.insert("BorderColor".into(),PropValue::String("#888888".into()));
            }
            ControlType::ProgressBar => {
                props.insert("Minimum".into(),     PropValue::Int(0));
                props.insert("Maximum".into(),     PropValue::Int(100));
                props.insert("Value".into(),       PropValue::Int(0));
                props.insert("BarColor".into(),    PropValue::String("#00AA00".into()));
                props.insert("Orientation".into(), PropValue::String("Horizontal".into()));
                props.insert("Style".into(),       PropValue::String("Continuous".into()));
                props.insert("ShowValue".into(),   PropValue::Bool(false));
            }
            ControlType::ListBox => {
                props.insert("Items".into(),          PropValue::String("".into()));
                props.insert("SelectedIndex".into(),  PropValue::Int(-1));
                props.insert("MultiSelect".into(),    PropValue::Bool(false));
                props.insert("Sorted".into(),         PropValue::Bool(false));
                props.insert("BorderStyle".into(),    PropValue::String("Single".into()));
                props.insert("BorderColor".into(),    PropValue::String("#888888".into()));
            }
            ControlType::ComboBox => {
                props.insert("Items".into(),          PropValue::String("".into()));
                props.insert("SelectedIndex".into(),  PropValue::Int(-1));
                props.insert("Sorted".into(),         PropValue::Bool(false));
                props.insert("DropDownStyle".into(),  PropValue::String("DropDown".into()));
                props.insert("DropDownHeight".into(), PropValue::Int(200));
                props.insert("Editable".into(),       PropValue::Bool(true));
            }
            ControlType::Button => {
                props.insert("IsDefault".into(),    PropValue::Bool(false));
                props.insert("IsCancel".into(),     PropValue::Bool(false));
                props.insert("ModalResult".into(),  PropValue::String("None".into()));
                props.insert("BorderColor".into(),  PropValue::String("#888888".into()));
                props.insert("BorderStyle".into(),  PropValue::String("Single".into()));
                props.insert("CornerRadius".into(), PropValue::Int(3));
                props.insert("FlatStyle".into(),    PropValue::Bool(false));
                props.insert("ImagePath".into(),    PropValue::String("".into()));
                props.insert("ImageAlignment".into(),   PropValue::String("MiddleLeft".into()));
                props.insert("TextAlignment".into(),    PropValue::String("MiddleCenter".into()));
            }
            ControlType::Panel | ControlType::GroupBox => {
                props.insert("BorderStyle".into(),  PropValue::String("Single".into()));
                props.insert("BorderColor".into(),  PropValue::String("#888888".into()));
                props.insert("Scrollable".into(),   PropValue::Bool(false));
            }
            ControlType::DataGrid => {
                // "Name:Type" per line (Type ∈ string|number|datetime; default string).
                props.insert("Columns".into(),             PropValue::String("".into()));
                // Cell data: rows separated by '\n', cells within a row by TAB.
                // Populated at runtime (e.g. from a bound COBOL table via SET-PROPERTY).
                props.insert("Rows".into(),                PropValue::String("".into()));
                props.insert("ReadOnly".into(),            PropValue::Bool(false));
                props.insert("AlternatingRowColor".into(), PropValue::String("#F0F8FF".into()));
                props.insert("HeaderBackgroundColor".into(),     PropValue::String("#E0E0E0".into()));
                props.insert("HeaderForegroundColor".into(),     PropValue::String("#000000".into()));
                props.insert("GridLineColor".into(),       PropValue::String("#CCCCCC".into()));
                props.insert("SelectionMode".into(),       PropValue::String("Row".into()));
                props.insert("RowHeight".into(),           PropValue::Int(22));
                props.insert("AllowSorting".into(),        PropValue::Bool(true));
                props.insert("AllowColumnResize".into(),   PropValue::Bool(true));
                props.insert("ShowRowNumbers".into(),      PropValue::Bool(false));
                props.insert("ExportCSV".into(),           PropValue::Bool(true));
                props.insert("CSVDelimiter".into(),        PropValue::String(",".into()));
                props.insert("CSVParagraph".into(),        PropValue::String("".into())); // COBOL para called after export
            }
            ControlType::TabControl => {
                props.insert("Tabs".into(),        PropValue::String("Tab1\nTab2".into()));
                props.insert("TabPosition".into(), PropValue::String("Top".into()));
                props.insert("SelectedTab".into(), PropValue::Int(0));
            }
            ControlType::MenuBar | ControlType::ToolBar | ControlType::StatusBar => {
                props.insert("Items".into(), PropValue::String("".into()));
            }
            ControlType::Line => {
                props.insert("LineColor".into(),     PropValue::String("#000000".into()));
                props.insert("LineThickness".into(), PropValue::Int(1));
                props.insert("LineDirection".into(), PropValue::String("Horizontal".into()));
                props.insert("DashStyle".into(),     PropValue::String("Solid".into()));
            }
            ControlType::DateTimePicker => {
                props.insert("Value".into(),        PropValue::String("".into()));
                props.insert("Format".into(),       PropValue::String("Short".into()));
                props.insert("CustomFormat".into(), PropValue::String("".into()));
                props.insert("ShowUpDown".into(),   PropValue::Bool(false));
                props.insert("MinimumDate".into(),      PropValue::String("".into()));
                props.insert("MaximumDate".into(),      PropValue::String("".into()));
                props.insert("BorderColor".into(),  PropValue::String("#888888".into()));
            }
            ControlType::NumericUpDown => {
                props.insert("Value".into(),        PropValue::Int(0));
                props.insert("Minimum".into(),      PropValue::Int(0));
                props.insert("Maximum".into(),      PropValue::Int(100));
                props.insert("Step".into(),         PropValue::Int(1));
                props.insert("DecimalPlaces".into(),PropValue::Int(0));
                props.insert("ThousandsSeparator".into(), PropValue::Bool(false));
                props.insert("ReadOnly".into(),     PropValue::Bool(false));
                props.insert("BorderColor".into(),  PropValue::String("#888888".into()));
            }
            ControlType::TreeView => {
                props.insert("Items".into(),         PropValue::String("Node 1\n  Child 1\n  Child 2\nNode 2".into()));
                props.insert("AllowEdit".into(),     PropValue::Bool(false));
                props.insert("CheckBoxes".into(),    PropValue::Bool(false));
                props.insert("ShowLines".into(),     PropValue::Bool(true));
                props.insert("ShowRootLines".into(), PropValue::Bool(true));
                props.insert("Sorted".into(),        PropValue::Bool(false));
                props.insert("HotTracking".into(),   PropValue::Bool(false));
                props.insert("LineColor".into(),     PropValue::String("#AAAAAA".into()));
                props.insert("BorderColor".into(),   PropValue::String("#888888".into()));
            }
            ControlType::Splitter => {
                props.insert("Orientation".into(),   PropValue::String("Horizontal".into()));
                props.insert("MinimumSize".into(),        PropValue::Int(25));
                props.insert("SplitPosition".into(), PropValue::Int(100));
                props.insert("BorderColor".into(),   PropValue::String("#CCCCCC".into()));
            }
            ControlType::Timer => {
                props.insert("Interval".into(),  PropValue::Int(1000)); // milliseconds
                props.insert("Enabled".into(),   PropValue::Bool(true));
                props.insert("Paragraph".into(), PropValue::String("".into())); // COBOL para to PERFORM on Tick
            }
            ControlType::Shape => {
                props.insert("ShapeType".into(),     PropValue::String("Rectangle".into()));
                props.insert("FillColor".into(),     PropValue::String("#C0C0C0".into()));
                props.insert("FillStyle".into(),     PropValue::String("Solid".into()));
                props.insert("LineColor".into(),     PropValue::String("#000000".into()));
                props.insert("LineThickness".into(), PropValue::Int(1));
                props.insert("LineStyle".into(),     PropValue::String("Solid".into()));
            }
            ControlType::AgentObject => {
                // Network / LLM connection
                props.insert("AgentURL".into(),          PropValue::String("http://localhost:11434".into()));
                props.insert("AgentModel".into(),        PropValue::String("llama3.2".into()));
                props.insert("AgentAPI".into(),          PropValue::String("Ollama".into())); // Ollama | LMStudio | OpenAI | Custom
                props.insert("AgentAPIKey".into(),       PropValue::String("".into()));
                props.insert("AgentEndpoint".into(),     PropValue::String("".into())); // override default endpoint
                // Behaviour
                props.insert("SystemPrompt".into(),      PropValue::String("You are a helpful assistant.".into()));
                props.insert("Temperature".into(),       PropValue::Int(70)); // stored as int 0-100 (0.0-1.0)
                props.insert("MaximumTokens".into(),         PropValue::Int(1024));
                props.insert("Stream".into(),            PropValue::Bool(true));
                props.insert("TimeoutSeconds".into(),        PropValue::Int(30));
                // Target controls — comma-sep list of IDs this agent is allowed to modify
                props.insert("TargetControls".into(),    PropValue::String("".into()));
                // COBOL paragraphs for events
                props.insert("ResponseParagraph".into(),      PropValue::String("".into()));
                props.insert("ErrorParagraph".into(),         PropValue::String("".into()));
                props.insert("StreamChunkParagraph".into(),   PropValue::String("".into()));
                // Data item to put the response into
                props.insert("ResponseDataItem".into(),  PropValue::String("".into()));
            }
            ControlType::ModalWindow => {
                props.insert("FormFile".into(),          PropValue::String("".into())); // .cfrm path
                props.insert("ProgramName".into(),       PropValue::String("".into())); // COBOL PROGRAM-ID
                props.insert("Title".into(),             PropValue::String("Dialog".into()));
                props.insert("Width".into(),             PropValue::Int(400));
                props.insert("Height".into(),            PropValue::Int(300));
                props.insert("Resizable".into(),         PropValue::Bool(false));
                props.insert("StartPosition".into(),     PropValue::String("CenterParent".into())); // CenterParent | CenterScreen | Manual
                // Shared COBOL data items (comma-separated) passed to/from the modal
                props.insert("SharedDataItems".into(),   PropValue::String("".into()));
                // COBOL paragraphs
                props.insert("OpenParagraph".into(),          PropValue::String("".into()));
                props.insert("ClosedParagraph".into(),        PropValue::String("".into()));
                props.insert("ConfirmedParagraph".into(),     PropValue::String("".into()));
                props.insert("CancelledParagraph".into(),     PropValue::String("".into()));
                props.insert("ModalResult".into(),       PropValue::String("None".into())); // None | OK | Cancel | Yes | No
            }
            ControlType::Slider => {
                props.insert("Minimum".into(),       PropValue::Int(0));
                props.insert("Maximum".into(),       PropValue::Int(100));
                props.insert("Value".into(),         PropValue::Int(0));
                props.insert("Step".into(),          PropValue::Int(10));
                props.insert("LargeChange".into(),   PropValue::Int(20));  // Page Up/Down increment
                props.insert("Orientation".into(),   PropValue::String("Horizontal".into())); // Horizontal | Vertical
                props.insert("TickFrequency".into(), PropValue::Int(10));  // Draw a tick every N units
                props.insert("TickStyle".into(),     PropValue::String("Bottom".into())); // None | Top | Bottom | Both
                props.insert("TrackColor".into(),    PropValue::String("#AAAAAA".into()));
                props.insert("ThumbColor".into(),    PropValue::String("#0078D7".into()));
                props.insert("FillColor".into(),     PropValue::String("#0078D7".into())); // filled portion of track
                props.insert("ShowValue".into(),     PropValue::Bool(false)); // label current value
                props.insert("DataItem".into(),      PropValue::String("".into()));
                props.insert("ChangeParagraph".into(),    PropValue::String("".into())); // COBOL para called on change
            }
            ControlType::RestClient => {
                props.insert("BaseURL".into(),       PropValue::String("https://api.example.com".into()));
                props.insert("DefaultMethod".into(), PropValue::String("GET".into())); // GET | POST | PUT | PATCH | DELETE
                props.insert("AuthType".into(),      PropValue::String("None".into())); // None | Bearer | Basic | APIKey
                props.insert("AuthToken".into(),     PropValue::String("".into()));
                props.insert("DefaultHeaders".into(),PropValue::String("".into())); // key:value pairs, newline-separated
                props.insert("TimeoutSeconds".into(),    PropValue::Int(30));
                props.insert("FollowRedirects".into(),PropValue::Bool(true));
                props.insert("VerifyTLS".into(),     PropValue::Bool(true));
                // COBOL data items
                props.insert("RequestDataItem".into(),   PropValue::String("".into())); // JSON body source
                props.insert("ResponseDataItem".into(),  PropValue::String("".into())); // where response goes
                props.insert("StatusDataItem".into(),    PropValue::String("".into())); // HTTP status code
                // COBOL paragraphs
                props.insert("ResponseParagraph".into(),  PropValue::String("".into()));
                props.insert("ErrorParagraph".into(),     PropValue::String("".into()));
            }
            ControlType::SqlDatabase => {
                // Connection
                props.insert("Driver".into(),            PropValue::String("sqlite".into())); // sqlite | postgres | mysql | mssql
                props.insert("ConnectionString".into(),  PropValue::String("sqlite::memory:".into()));
                props.insert("AutoConnect".into(),       PropValue::Bool(false));
                props.insert("MaximumConnections".into(),    PropValue::Int(5));
                // COBOL object data items generated in WORKING-STORAGE
                props.insert("ConnectionDataItem".into(),      PropValue::String("".into())); // e.g. conn1
                props.insert("ResultSetDataItem".into(), PropValue::String("".into())); // e.g. resultset1
                // COBOL paragraphs
                props.insert("ConnectParagraph".into(),       PropValue::String("".into())); // called after connect
                props.insert("ErrorParagraph".into(),         PropValue::String("".into())); // called on any SQL error
                props.insert("QueryCompleteParagraph".into(), PropValue::String("".into())); // called after exec
            }

            // ── Charts ────────────────────────────────────────────────────────
            ControlType::BarChart
            | ControlType::LineChart
            | ControlType::PieChart
            | ControlType::AreaChart
            | ControlType::ScatterChart
            | ControlType::DonutChart => {
                // Visual
                props.insert("Title".into(),           PropValue::String("".into()));
                props.insert("ShowLegend".into(),      PropValue::Bool(true));
                props.insert("ShowGridLines".into(),   PropValue::Bool(true));
                props.insert("ShowTooltips".into(),    PropValue::Bool(true));
                props.insert("AnimateOnLoad".into(),   PropValue::Bool(true));
                props.insert("XAxisLabel".into(),      PropValue::String("".into()));
                props.insert("YAxisLabel".into(),      PropValue::String("".into()));
                props.insert("SeriesColors".into(),    PropValue::String(
                    "#4C9BE8,#E87A4C,#4CE87A,#E84C9B,#9B4CE8,#E8C84C".into())); // comma-sep hex
                // Data binding — COBOL table
                props.insert("DataSource".into(),      PropValue::String("".into())); // WS data-item name
                props.insert("DataCount".into(),       PropValue::String("".into())); // count / tally item
                props.insert("LabelField".into(),      PropValue::String("".into())); // sub-field for X labels
                props.insert("ValueFields".into(),     PropValue::String("".into())); // comma-sep sub-fields for Y series
                props.insert("SeriesLabels".into(),    PropValue::String("".into())); // display names for series
                // COBOL paragraphs
                props.insert("DataChangedParagraph".into(), PropValue::String("".into()));
                props.insert("ClickParagraph".into(),       PropValue::String("".into()));
                // Bar/Line/Area specifics
                if matches!(control_type, ControlType::BarChart) {
                    props.insert("Horizontal".into(),  PropValue::Bool(false));
                    props.insert("Stacked".into(),     PropValue::Bool(false));
                    props.insert("BarCornerRadius".into(), PropValue::Int(3));
                }
                if matches!(control_type, ControlType::LineChart | ControlType::AreaChart) {
                    props.insert("Smooth".into(),      PropValue::Bool(true));
                    props.insert("ShowPoints".into(),  PropValue::Bool(true));
                    props.insert("PointRadius".into(), PropValue::Int(4));
                    if matches!(control_type, ControlType::AreaChart) {
                        props.insert("FillAlpha".into(), PropValue::Int(40)); // 0-100%
                        props.insert("Stacked".into(),   PropValue::Bool(false));
                    }
                }
                if matches!(control_type, ControlType::PieChart | ControlType::DonutChart) {
                    props.insert("ShowLabels".into(),  PropValue::Bool(true));
                    props.insert("LabelFormat".into(), PropValue::String("percent".into())); // percent | value | label
                    if matches!(control_type, ControlType::DonutChart) {
                        props.insert("InnerRadius".into(), PropValue::Int(40)); // % of outer radius
                    }
                }
                if matches!(control_type, ControlType::ScatterChart) {
                    props.insert("BubbleField".into(), PropValue::String("".into())); // field for bubble size
                    props.insert("BubbleScale".into(), PropValue::Int(20)); // max bubble radius px
                }
            }

            _ => {}
        }

        Self {
            id:           id_str,
            control_type,
            rect:         Rect::new(x, y, w, h),
            tab_order:    0,
            z_order:      0,
            visible:      true,
            enabled:      true,
            properties:   props,
            events:       Vec::new(),
            children:     Vec::new(),
            animations:   Vec::new(),
        }
    }

    pub fn get_prop(&self, name: &str) -> Option<&PropValue> {
        self.properties.get(name).or_else(|| {
            let lower = name.to_ascii_lowercase();
            self.properties.iter()
                .find(|(k, _)| k.to_ascii_lowercase() == lower)
                .map(|(_, v)| v)
        })
    }

    pub fn set_prop(&mut self, name: impl Into<String>, value: impl Into<PropValue>) {
        self.properties.insert(name.into(), value.into());
    }

    pub fn display_text(&self) -> String {
        self.get_prop("Caption")
            .or_else(|| self.get_prop("Text"))
            .map(|v| v.to_string())
            .unwrap_or_else(|| self.id.clone())
    }

    /// Bind an event to a paragraph name (legacy API — paragraph is auto-derived in v1.0).
    pub fn bind_event(&mut self, event: impl Into<String>, paragraph: impl Into<String>) {
        let event_s   = event.into();
        let para      = paragraph.into();
        self.events.retain(|e| e.event != event_s);
        self.events.push(EventBinding::new(event_s, para));
    }

    /// Ensure the control has an `EventBinding` for the given event, creating
    /// one with an auto-derived paragraph name if absent. Returns a mutable ref.
    pub fn ensure_event(&mut self, event: &str) -> &mut EventBinding {
        if !self.events.iter().any(|e| e.event == event) {
            self.events.push(EventBinding::for_control(&self.id, event));
        }
        self.events.iter_mut().find(|e| e.event == event).unwrap()
    }

    /// Add an animation definition. Replaces any existing animation with the same name.
    pub fn add_animation(&mut self, anim: AnimationDef) {
        self.animations.retain(|a| a.name != anim.name);
        self.animations.push(anim);
    }

    /// Remove an animation by name.
    pub fn remove_animation(&mut self, name: &str) {
        self.animations.retain(|a| a.name != name);
    }
}

// ── BgImageMode ───────────────────────────────────────────────────────────────

/// How the form background image is scaled / tiled.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum BgImageMode {
    /// Stretch to fill the form rectangle (may distort).
    #[default]
    Stretch,
    /// Tile the image across the form (repeat like wallpaper).
    Tile,
    /// Center the image without scaling.
    Center,
    /// Scale uniformly to cover the entire form (may clip edges).
    Fill,
    /// Scale uniformly to fit fully inside the form (may leave empty margins).
    Fit,
}

impl BgImageMode {
    pub fn as_str(self) -> &'static str {
        match self {
            BgImageMode::Stretch => "Stretch",
            BgImageMode::Tile    => "Tile",
            BgImageMode::Center  => "Center",
            BgImageMode::Fill    => "Fill",
            BgImageMode::Fit     => "Fit",
        }
    }
    pub fn from_str(s: &str) -> Self {
        match s {
            "Tile"   => BgImageMode::Tile,
            "Center" => BgImageMode::Center,
            "Fill"   => BgImageMode::Fill,
            "Fit"    => BgImageMode::Fit,
            _        => BgImageMode::Stretch,
        }
    }
    pub fn all() -> &'static [&'static str] {
        &["Stretch","Tile","Center","Fill","Fit"]
    }
}

// ── Form ─────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct Form {
    pub name:             String,
    pub title:            String,
    pub width:            u32,
    pub height:           u32,
    pub background_color: String,
    /// Window-level transparency: 0 = fully opaque, 100 = fully transparent.
    pub transparency:     u8,
    /// Optional background image path (empty = none).
    pub background_image: String,
    /// How the background image is scaled / tiled.
    pub bg_image_mode:    BgImageMode,
    pub controls:         Vec<Control>,
    /// Form-level animations (e.g. form entrance effect).
    pub animations:       Vec<AnimationDef>,
    /// Designer grid dot spacing in pixels (4–64). Default 8.
    pub grid_size:        u8,
    /// Whether controls snap to the grid when moved or resized. Default true.
    pub snap_to_grid:     bool,
    /// Target device preset name (e.g. "iPhone 15", "Custom"). Controls default width/height.
    pub target:           String,

    // ── v1.0 nested-program fields ────────────────────────────────────────────

    /// Raw COBOL text for the WORKING-STORAGE section — emitted verbatim into the
    /// outer program's WS after the generated control-bound items.
    /// The user writes normal COBOL declarations here, including GLOBAL / EXTERNAL.
    pub user_ws_source:   String,

    /// Form-level lifecycle event handlers (OnLoad, OnClose).
    /// Uses the same `EventBinding` struct as control events; `control_id` is "".
    pub form_events:      Vec<EventBinding>,

    /// Recycle bin: code preserved from deleted controls.
    /// Never emitted into generated .cbl — only stored in .cfrm.
    pub deleted_code:     Vec<DeletedControlCode>,
}

impl Form {
    pub fn new(name: impl Into<String>, title: impl Into<String>, width: u32, height: u32) -> Self {
        let form_name = name.into();
        // Pre-populate onLoad and onClose with empty stubs so the Code View
        // always shows them even before the user writes anything.
        let form_events = vec![
            EventBinding {
                event:     "onLoad".into(),
                paragraph: derive_paragraph_name(&form_name, "onLoad"),
                code:      String::new(),
                local_ws:  String::new(),
            },
            EventBinding {
                event:     "onClose".into(),
                paragraph: derive_paragraph_name(&form_name, "onClose"),
                code:      String::new(),
                local_ws:  String::new(),
            },
        ];
        Self {
            name:             form_name,
            title:            title.into(),
            width,
            height,
            background_color: "00000000".to_owned(),
            transparency:     0,
            background_image: String::new(),
            bg_image_mode:    BgImageMode::Stretch,
            controls:         Vec::new(),
            animations:       Vec::new(),
            grid_size:        8,
            snap_to_grid:     true,
            target:           "Custom".to_owned(),
            user_ws_source:   String::new(),
            form_events,
            deleted_code:     Vec::new(),
        }
    }

    pub fn find_control(&self, id: &str) -> Option<&Control> {
        let upper = id.to_ascii_uppercase();
        self.controls.iter().find_map(|c| find_in(c, &upper))
    }

    pub fn find_control_mut(&mut self, id: &str) -> Option<&mut Control> {
        let upper = id.to_ascii_uppercase();
        self.controls.iter_mut().find_map(|c| find_in_mut(c, &upper))
    }

    pub fn add_control(&mut self, mut ctrl: Control) {
        ctrl.tab_order = self.controls.len() as u32;
        ctrl.z_order   = self.controls.len() as i32;
        self.controls.push(ctrl);
    }

    /// Remove a control unconditionally (no code preservation).
    /// Call `remove_control_with_code_check` from the IDE for interactive deletion.
    pub fn remove_control(&mut self, id: &str) {
        let upper = id.to_ascii_uppercase();
        self.controls.retain(|c| c.id.to_ascii_uppercase() != upper);
    }

    /// Check whether a control has any non-empty event code.
    /// Returns the list of (event_name, line_count) pairs that have code.
    pub fn control_has_code(&self, id: &str) -> Vec<(String, usize)> {
        let Some(ctrl) = self.find_control(id) else { return Vec::new() };
        ctrl.events.iter()
            .filter(|ev| ev.has_code())
            .map(|ev| (ev.event.clone(), ev.code_line_count()))
            .collect()
    }

    /// Move a control's event code to the recycle bin, then remove the control.
    /// Called when the user chooses "Preserve in Recycle" in the deletion dialog.
    pub fn recycle_control(&mut self, id: &str, deleted_at: impl Into<String>) {
        let upper = id.to_ascii_uppercase();
        if let Some(ctrl) = self.find_control(&upper) {
            let events_with_code: Vec<EventBinding> = ctrl.events.iter()
                .filter(|ev| ev.has_code())
                .cloned()
                .collect();
            if !events_with_code.is_empty() {
                self.deleted_code.push(DeletedControlCode {
                    control_id: ctrl.id.clone(),
                    deleted_at: deleted_at.into(),
                    events:     events_with_code,
                });
            }
        }
        self.controls.retain(|c| c.id.to_ascii_uppercase() != upper);
    }

    /// Restore a recycled control's code entries back into an existing control
    /// (e.g. if the user re-added the control and wants its old code back).
    pub fn restore_from_recycle(&mut self, deleted_at: &str, target_control_id: &str) {
        let Some(pos) = self.deleted_code.iter().position(|d| d.deleted_at == deleted_at)
        else { return };
        let recycled = self.deleted_code.remove(pos);
        let upper = target_control_id.to_ascii_uppercase();
        if let Some(ctrl) = self.find_control_mut(&upper) {
            for recycled_ev in recycled.events {
                if let Some(existing) = ctrl.events.iter_mut().find(|e| e.event == recycled_ev.event) {
                    existing.code = recycled_ev.code;
                } else {
                    ctrl.events.push(recycled_ev);
                }
            }
        }
    }

    pub fn all_event_paragraphs(&self) -> Vec<String> {
        let mut paras = Vec::new();
        // form-level events (OnLoad, OnClose nested programs)
        for ev in &self.form_events { paras.push(ev.paragraph.clone()); }
        // per-control events
        for ctrl in &self.controls {
            collect_paragraphs(ctrl, &mut paras);
        }
        paras.dedup();
        paras
    }

    /// Return controls sorted by z_order ascending (for rendering back-to-front).
    pub fn controls_by_z(&self) -> Vec<&Control> {
        let mut v: Vec<&Control> = self.controls.iter().collect();
        v.sort_by_key(|c| c.z_order);
        v
    }
}

// ── Internal helpers ──────────────────────────────────────────────────────────

fn find_in<'a>(ctrl: &'a Control, id: &str) -> Option<&'a Control> {
    if ctrl.id.to_ascii_uppercase() == id { return Some(ctrl); }
    ctrl.children.iter().find_map(|c| find_in(c, id))
}

fn find_in_mut<'a>(ctrl: &'a mut Control, id: &str) -> Option<&'a mut Control> {
    if ctrl.id.to_ascii_uppercase() == id { return Some(ctrl); }
    ctrl.children.iter_mut().find_map(|c| find_in_mut(c, id))
}

fn collect_paragraphs(ctrl: &Control, out: &mut Vec<String>) {
    for ev in &ctrl.events { out.push(ev.paragraph.clone()); }
    for child in &ctrl.children { collect_paragraphs(child, out); }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn form_events_unique_and_include_lifecycle() {
        let all: Vec<&str> = form_supported_events().collect();
        // No duplicates across groups.
        let mut seen = std::collections::HashSet::new();
        for ev in &all {
            assert!(seen.insert(*ev), "duplicate form event: {ev}");
            assert!(ev.starts_with("on"), "form event not 'on'-prefixed: {ev}");
        }
        // Pre-stubbed lifecycle events are present.
        assert!(all.contains(&"onLoad"));
        assert!(all.contains(&"onClose"));
        // A representative sample from later groups.
        for ev in ["onResize", "onDoubleClick", "onPaste", "onUnhandledException"] {
            assert!(all.contains(&ev), "missing form event: {ev}");
        }
        assert_eq!(all.len(), 66, "expected 66 form events");
    }

    #[test]
    fn form_add_and_find() {
        let mut form = Form::new("MAIN-FORM", "My App", 800, 600);
        let ctrl = Control::new("BTN-OK", ControlType::Button, 10, 10);
        form.add_control(ctrl);
        assert!(form.find_control("BTN-OK").is_some());
        assert!(form.find_control("btn-ok").is_some());
        assert!(form.find_control("NONEXISTENT").is_none());
    }

    #[test]
    fn control_default_size_button() {
        let (w, h) = ControlType::Button.default_size();
        assert_eq!(w, 80); assert_eq!(h, 28);
    }

    #[test]
    fn prop_value_roundtrip() {
        let v = PropValue::Int(42);
        assert_eq!(v.as_i64(), 42);
        assert_eq!(v.to_xml_string(), "42");
    }

    #[test]
    fn animation_def_basic() {
        let mut ctrl = Control::new("BTN-1", ControlType::Button, 10, 10);
        let anim = AnimationDef::new("fly-in");
        ctrl.add_animation(anim);
        assert_eq!(ctrl.animations.len(), 1);
        assert_eq!(ctrl.animations[0].name, "fly-in");
    }

    #[test]
    fn easing_linear() {
        let e = EasingKind::Linear;
        assert!((e.apply(0.5) - 0.5).abs() < 0.001);
    }

    #[test]
    fn z_order_sort() {
        let mut form = Form::new("F", "T", 800, 600);
        let mut a = Control::new("A", ControlType::Label, 0, 0);
        a.z_order = 5;
        let mut b = Control::new("B", ControlType::Label, 0, 0);
        b.z_order = 1;
        form.controls.push(a);
        form.controls.push(b);
        let sorted = form.controls_by_z();
        assert_eq!(sorted[0].id, "B");
        assert_eq!(sorted[1].id, "A");
    }

    #[test]
    fn agent_object_defaults() {
        let ctrl = Control::new("AGT-1", ControlType::AgentObject, 0, 0);
        assert!(ctrl.get_prop("AgentURL").is_some());
        assert!(ctrl.get_prop("AgentModel").is_some());
        assert!(ctrl.get_prop("SystemPrompt").is_some());
    }
}
