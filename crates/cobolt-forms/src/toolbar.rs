// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! What a ToolBar is made of.
//!
//! A toolbar used to be a newline-separated list of words — `Items`, the same
//! property a StatusBar has. It drew them in a row and that was the whole of it:
//! no icons, no grouping, nothing to press.
//!
//! A real toolbar is **groups of buttons**. Each group is a frame with its own
//! border, corner radius and padding; an invisible separator sets one group
//! apart from the next. Every element inside a group is a button the developer
//! controls completely — its icon and that icon's size and colour, its own size,
//! a solid or gradient background, a drop shadow, its foreground colour, whether
//! it is enabled, and what pressing it does.
//!
//! ```text
//! ┌──────────────────┐   ┌──────────┐        ┌───────────┐
//! │  ⎘   ✂   📋      │   │  🖨  ⇪   │        │   ▶       │
//! └──────────────────┘   └──────────┘        └───────────┘
//!    group "clipboard"    group "output"  ↑     group "run"
//!                                    separator
//! ```
//!
//! # Colours follow the form's theme
//!
//! Every colour here defaults to empty, and empty means *the form's theme
//! decides* — the same rule the Gauge's `NeedleColor` follows. A colour the
//! developer actually picks always wins. So a toolbar dropped on a themed form
//! looks like it belongs there without being configured, and a toolbar that was
//! configured keeps exactly what it was given.
//!
//! # Where it lives
//!
//! The whole definition is serialised into one control property,
//! [`TOOLBAR_DEF_PROP`], the way a DataGrid keeps its advanced column metadata
//! in `AdvancedGrid`. The form stays self-contained: no side-car file, and the
//! `.cfrm` round-trip works with no new machinery.
//!
//! # Toolbars that already exist
//!
//! [`ToolbarDef::from_control`] never returns nothing. A toolbar with no
//! definition but a populated `Items` is read as ONE group of plain labelled
//! buttons, in order — so a form built before any of this keeps the toolbar it
//! had, and editing it in the designer is what promotes it.

use serde::{Deserialize, Serialize};

use crate::model::Control;

/// The control property the serialised definition lives in.
pub const TOOLBAR_DEF_PROP: &str = "ToolbarLayout";

/// Corner radius a group and a button are born with (operator decision,
/// 2026-08-16). The artwork is rounded by default rather than square, so a
/// toolbar looks finished before anyone opens the editor.
pub const DEFAULT_CORNER_RADIUS: i64 = 10;

/// Padding a group is born with, in pixels, between its frame and its buttons.
pub const DEFAULT_GROUP_PADDING: i64 = 6;

/// Width of the invisible gap that sets one group apart from the next.
pub const DEFAULT_SEPARATOR_WIDTH: i64 = 12;

/// Default button box, in pixels.
pub const DEFAULT_BUTTON_SIZE: (i64, i64) = (32, 28);

/// Default icon box inside a button, in pixels.
pub const DEFAULT_ICON_SIZE: i64 = 18;

fn default_true() -> bool {
    true
}
fn is_true(b: &bool) -> bool {
    *b
}
fn default_radius() -> i64 {
    DEFAULT_CORNER_RADIUS
}
fn default_padding() -> i64 {
    DEFAULT_GROUP_PADDING
}
fn default_separator_width() -> i64 {
    DEFAULT_SEPARATOR_WIDTH
}
fn default_button_width() -> i64 {
    DEFAULT_BUTTON_SIZE.0
}
fn default_button_height() -> i64 {
    DEFAULT_BUTTON_SIZE.1
}
fn default_icon_size() -> i64 {
    DEFAULT_ICON_SIZE
}
fn is_empty(s: &String) -> bool {
    s.is_empty()
}
fn is_false(b: &bool) -> bool {
    !*b
}
fn is_zero(n: &i64) -> bool {
    *n == 0
}

// ── Actions ───────────────────────────────────────────────────────────────────

/// What pressing a toolbar button does.
///
/// Serialised as a string, the way a menu item's action is, so the vocabulary is
/// readable in the `.cfrm` and one variant can be added without breaking a form
/// that was saved before it existed. Anything unrecognised parses as
/// [`ToolbarAction::Event`], which is the harmless default: the button fires its
/// own `onClick` and the developer's COBOL decides.
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub enum ToolbarAction {
    /// Fire the button's own `onClick` handler. The default.
    #[default]
    Event,
    /// `PERFORM` one of the form's user procedures.
    Procedure(String),
    /// Open a **standalone** form as a modal window. Only standalone forms can
    /// be opened this way (operator decision): an embedded form belongs in a
    /// ContentPane, not in a window of its own.
    OpenModal(String),
    /// Send a document to the OS print dialog. The target is a file path, or the
    /// name of a data item holding one — so a form can print the report it just
    /// wrote.
    Print(String),
    /// Offer this form's window, as an image, to the OS share sheet.
    Share,
    /// Put an image of this form's window on the clipboard.
    Screenshot,
    /// OS clipboard, acting on whichever control has focus.
    Copy,
    Cut,
    Paste,
    /// Launch an external application: a path, optionally followed by arguments.
    RunApp(String),
    /// Open a terminal, optionally in a given directory.
    OpenTerminal(String),
}

impl ToolbarAction {
    /// Parse the stored string. Unknown verbs become [`ToolbarAction::Event`]
    /// rather than an error: a form saved by a newer PowerRustCOBOL must still
    /// open in an older one, with the button merely doing less.
    pub fn parse(raw: &str) -> Self {
        let raw = raw.trim();
        let (verb, target) = match raw.split_once(':') {
            Some((v, t)) => (v.trim(), t.trim().to_owned()),
            None => (raw, String::new()),
        };
        match verb.to_ascii_lowercase().as_str() {
            "procedure" => Self::Procedure(target),
            "open-modal" => Self::OpenModal(target),
            "print" => Self::Print(target),
            "share" => Self::Share,
            "screenshot" => Self::Screenshot,
            "copy" => Self::Copy,
            "cut" => Self::Cut,
            "paste" => Self::Paste,
            "run-app" => Self::RunApp(target),
            "open-terminal" => Self::OpenTerminal(target),
            _ => Self::Event,
        }
    }

    /// The stored form, round-tripping [`ToolbarAction::parse`].
    pub fn to_action_string(&self) -> String {
        match self {
            Self::Event => "event".into(),
            Self::Procedure(t) => format!("procedure:{t}"),
            Self::OpenModal(t) => format!("open-modal:{t}"),
            Self::Print(t) => format!("print:{t}"),
            Self::Share => "share".into(),
            Self::Screenshot => "screenshot".into(),
            Self::Copy => "copy".into(),
            Self::Cut => "cut".into(),
            Self::Paste => "paste".into(),
            Self::RunApp(t) => format!("run-app:{t}"),
            Self::OpenTerminal(t) => format!("open-terminal:{t}"),
        }
    }

    /// Whether this action is carried out by the platform rather than by the
    /// form's own COBOL — the ones that need a window, a clipboard, a printer or
    /// another process. Used to decide what the running host has to do itself.
    pub fn is_platform_action(&self) -> bool {
        !matches!(self, Self::Event | Self::Procedure(_) | Self::OpenModal(_))
    }

    /// The designer-facing name of the verb, for the editor's picker.
    pub fn verb(&self) -> &'static str {
        match self {
            Self::Event => "event",
            Self::Procedure(_) => "procedure",
            Self::OpenModal(_) => "open-modal",
            Self::Print(_) => "print",
            Self::Share => "share",
            Self::Screenshot => "screenshot",
            Self::Copy => "copy",
            Self::Cut => "cut",
            Self::Paste => "paste",
            Self::RunApp(_) => "run-app",
            Self::OpenTerminal(_) => "open-terminal",
        }
    }

    /// Whether the verb takes a target, so the editor knows to offer the field.
    pub fn takes_target(verb: &str) -> bool {
        matches!(
            verb,
            "procedure" | "open-modal" | "print" | "run-app" | "open-terminal"
        )
    }

    /// Every verb the editor offers, in the order it offers them: what the
    /// form's own code does first, then what the platform does.
    pub const VERBS: &'static [&'static str] = &[
        "event",
        "procedure",
        "open-modal",
        "print",
        "share",
        "screenshot",
        "copy",
        "cut",
        "paste",
        "run-app",
        "open-terminal",
    ];
}

// ── How a button looks ────────────────────────────────────────────────────────

/// Everything about a button's APPEARANCE, with every field optional.
///
/// One struct, used at two levels, which is what makes a group's settings the
/// defaults for its buttons (operator, 2026-08-17):
///
/// * on a [`ToolbarGroup`] it is the default for every button in that group;
/// * on a [`ToolbarButton`] it is that button's own override.
///
/// [`ButtonStyle::resolve`] walks button → group → built-in, so an unset field
/// means "whatever my group says", and an unset field on the group means
/// "whatever the theme says". A field the developer sets anywhere wins over
/// everything above it.
///
/// Colours are `String` and empty means unset; the rest are `Option` because a
/// zero corner radius and a zero shadow distance are real choices that a
/// sentinel would swallow.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct ButtonStyle {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub icon_size: Option<i64>,
    #[serde(default, skip_serializing_if = "is_empty")]
    pub icon_color: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub width: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub height: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub corner_radius: Option<i64>,
    #[serde(default, skip_serializing_if = "is_empty")]
    pub background_color: String,
    #[serde(default, skip_serializing_if = "is_empty")]
    pub foreground_color: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gradient: Option<bool>,
    #[serde(default, skip_serializing_if = "is_empty")]
    pub gradient_start_color: String,
    #[serde(default, skip_serializing_if = "is_empty")]
    pub gradient_end_color: String,
    /// `Vertical` | `Horizontal` | `Diagonal`. Empty = `Vertical`.
    #[serde(default, skip_serializing_if = "is_empty")]
    pub gradient_direction: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shadow: Option<bool>,
    #[serde(default, skip_serializing_if = "is_empty")]
    pub shadow_color: String,
    /// 0-100 %.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shadow_opacity: Option<i64>,
    /// Pixels.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shadow_distance: Option<i64>,
    /// 0-20 blur layers.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shadow_blur_strength: Option<i64>,
}

/// A style with every field decided — what the painter actually draws from.
#[derive(Clone, Debug, PartialEq)]
pub struct ResolvedStyle {
    pub icon_size: i64,
    pub icon_color: String,
    pub width: i64,
    pub height: i64,
    pub corner_radius: i64,
    pub background_color: String,
    pub foreground_color: String,
    pub gradient: bool,
    pub gradient_start_color: String,
    pub gradient_end_color: String,
    pub gradient_direction: String,
    pub shadow: bool,
    pub shadow_color: String,
    pub shadow_opacity: i64,
    pub shadow_distance: i64,
    pub shadow_blur_strength: i64,
}

/// The first of `a` then `b` that the developer actually set.
fn pick_str<'a>(a: &'a str, b: &'a str) -> String {
    if !a.trim().is_empty() {
        a.to_owned()
    } else {
        b.to_owned()
    }
}

impl ButtonStyle {
    /// Resolve `self` (a button's overrides) against `group` (its group's
    /// defaults), falling back to the built-in values.
    pub fn resolve(&self, group: &ButtonStyle) -> ResolvedStyle {
        ResolvedStyle {
            icon_size: self
                .icon_size
                .or(group.icon_size)
                .unwrap_or(DEFAULT_ICON_SIZE),
            icon_color: pick_str(&self.icon_color, &group.icon_color),
            width: self
                .width
                .or(group.width)
                .unwrap_or(DEFAULT_BUTTON_SIZE.0),
            height: self
                .height
                .or(group.height)
                .unwrap_or(DEFAULT_BUTTON_SIZE.1),
            corner_radius: self
                .corner_radius
                .or(group.corner_radius)
                .unwrap_or(DEFAULT_CORNER_RADIUS),
            background_color: pick_str(&self.background_color, &group.background_color),
            foreground_color: pick_str(&self.foreground_color, &group.foreground_color),
            gradient: self.gradient.or(group.gradient).unwrap_or(false),
            gradient_start_color: pick_str(
                &self.gradient_start_color,
                &group.gradient_start_color,
            ),
            gradient_end_color: pick_str(&self.gradient_end_color, &group.gradient_end_color),
            gradient_direction: pick_str(&self.gradient_direction, &group.gradient_direction),
            shadow: self.shadow.or(group.shadow).unwrap_or(false),
            shadow_color: pick_str(&self.shadow_color, &group.shadow_color),
            shadow_opacity: self
                .shadow_opacity
                .or(group.shadow_opacity)
                .unwrap_or(0),
            shadow_distance: self
                .shadow_distance
                .or(group.shadow_distance)
                .unwrap_or(0),
            shadow_blur_strength: self
                .shadow_blur_strength
                .or(group.shadow_blur_strength)
                .unwrap_or(0),
        }
    }
}

// ── Buttons ───────────────────────────────────────────────────────────────────

/// One pressable element of a group.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ToolbarButton {
    pub id: String,
    /// Text on the button.
    ///
    /// A button carries a label OR an icon, never both (operator, 2026-08-17) —
    /// see [`ToolbarButton::set_label`] and [`ToolbarButton::set_icon`], which
    /// keep that true by clearing the other one.
    #[serde(default, skip_serializing_if = "is_empty")]
    pub label: String,
    /// Hover text. Empty draws none.
    #[serde(default, skip_serializing_if = "is_empty")]
    pub tooltip: String,
    /// A name from the hand-drawn icon catalogue (`icons::menu_icon_names`).
    #[serde(default, skip_serializing_if = "is_empty")]
    pub icon: String,
    #[serde(default = "default_true", skip_serializing_if = "is_true")]
    pub enabled: bool,
    /// The stored action string — see [`ToolbarAction`].
    #[serde(default, skip_serializing_if = "is_empty")]
    pub action: String,
    /// This button's own appearance, over its group's.
    #[serde(default, skip_serializing_if = "is_default_style")]
    pub style: ButtonStyle,
}

fn is_default_style(s: &ButtonStyle) -> bool {
    *s == ButtonStyle::default()
}

impl ToolbarButton {
    /// A new button: enabled, its own `onClick` to run, and no appearance of its
    /// own — so it looks like whatever its group says, which in turn looks like
    /// whatever the form's theme says.
    pub fn new(id: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            tooltip: String::new(),
            icon: String::new(),
            enabled: true,
            action: String::new(),
            style: ButtonStyle::default(),
        }
    }

    pub fn action(&self) -> ToolbarAction {
        ToolbarAction::parse(&self.action)
    }

    /// Give the button a label, which takes its icon away.
    ///
    /// A toolbar button shows one thing. Letting it hold both meant deciding at
    /// paint time which won, and the developer could not tell from the editor
    /// which they would get.
    pub fn set_label(&mut self, label: impl Into<String>) {
        self.label = label.into();
        if !self.label.trim().is_empty() {
            self.icon.clear();
        }
    }

    /// Give the button an icon, which takes its label away.
    pub fn set_icon(&mut self, icon: impl Into<String>) {
        self.icon = icon.into();
        if !self.icon.trim().is_empty() {
            self.label.clear();
        }
    }

    /// What this button draws as, given its group.
    pub fn resolved(&self, group: &ToolbarGroup) -> ResolvedStyle {
        self.style.resolve(&group.button_defaults)
    }
}

// ── Groups ────────────────────────────────────────────────────────────────────

/// A framed set of buttons.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ToolbarGroup {
    pub id: String,
    /// Designer-facing only — names the group in the editor, never drawn.
    #[serde(default, skip_serializing_if = "is_empty")]
    pub label: String,
    #[serde(default)]
    pub buttons: Vec<ToolbarButton>,
    /// `None` | `Single` | `Fixed3D`. Empty = `Single`; `None` draws no frame,
    /// which is how a group becomes invisible while still grouping.
    #[serde(default, skip_serializing_if = "is_empty")]
    pub border_style: String,
    /// Empty = the theme's border colour.
    #[serde(default, skip_serializing_if = "is_empty")]
    pub border_color: String,
    #[serde(default, skip_serializing_if = "is_zero")]
    pub border_width: i64,
    #[serde(default = "default_radius")]
    pub corner_radius: i64,
    /// Between the frame and the buttons, per group.
    #[serde(default = "default_padding")]
    pub padding: i64,
    /// Empty = the theme's panel face; a group is see-through by default.
    #[serde(default, skip_serializing_if = "is_empty")]
    pub background_color: String,
    /// An invisible gap after this group, setting it apart from the next.
    #[serde(default, skip_serializing_if = "is_false")]
    pub separator_after: bool,
    #[serde(default = "default_separator_width")]
    pub separator_width: i64,
    /// How every button in this group looks unless it says otherwise. Set the
    /// icon size once here rather than on each of six buttons.
    #[serde(default, skip_serializing_if = "is_default_style")]
    pub button_defaults: ButtonStyle,
}

impl ToolbarGroup {
    pub fn new(id: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            buttons: Vec::new(),
            border_style: String::new(),
            border_color: String::new(),
            border_width: 1,
            corner_radius: DEFAULT_CORNER_RADIUS,
            padding: DEFAULT_GROUP_PADDING,
            background_color: String::new(),
            separator_after: false,
            separator_width: DEFAULT_SEPARATOR_WIDTH,
            button_defaults: ButtonStyle::default(),
        }
    }

    /// Whether this group draws a frame at all. `None` (or a zero width) means
    /// the group still groups — its padding and separator apply — but nothing is
    /// drawn around it.
    pub fn draws_frame(&self) -> bool {
        !self.border_style.eq_ignore_ascii_case("None") && self.border_width > 0
    }

    /// The group's width, laid out left to right: padding, each button, padding,
    /// and the separator when one follows. Button widths are the RESOLVED ones,
    /// so a size set once on the group is what the layout measures.
    pub fn layout_width(&self, button_gap: i64) -> i64 {
        let buttons: i64 = self
            .buttons
            .iter()
            .map(|b| b.resolved(self).width)
            .sum();
        let gaps = button_gap * (self.buttons.len().saturating_sub(1) as i64);
        let sep = if self.separator_after {
            self.separator_width
        } else {
            0
        };
        self.padding * 2 + buttons + gaps + sep
    }
}

// ── The whole toolbar ─────────────────────────────────────────────────────────

/// Every group on one ToolBar.
#[derive(Clone, Debug, PartialEq, Default, Serialize, Deserialize)]
pub struct ToolbarDef {
    #[serde(default)]
    pub groups: Vec<ToolbarGroup>,
    /// Gap between two buttons inside a group, in pixels.
    #[serde(default = "default_button_gap")]
    pub button_gap: i64,
}

fn default_button_gap() -> i64 {
    4
}

impl ToolbarDef {
    /// Read a control's toolbar. Never empty-handed:
    ///
    /// * a stored [`TOOLBAR_DEF_PROP`] is used as it stands;
    /// * failing that, a populated `Items` becomes ONE group of plain labelled
    ///   buttons, so a toolbar built before groups existed still has its
    ///   buttons — and opening the editor is what turns it into a real one;
    /// * a control with neither gives an empty toolbar, not an error.
    pub fn from_control(control: &Control) -> Self {
        if let Some(raw) = control.get_prop(TOOLBAR_DEF_PROP).map(|v| v.as_str()) {
            let raw = raw.trim();
            if !raw.is_empty() {
                if let Ok(def) = serde_json::from_str::<Self>(raw) {
                    return def;
                }
            }
        }
        let items = control
            .get_prop("Items")
            .map(|v| v.as_str().to_owned())
            .unwrap_or_default();
        Self::from_items(&items)
    }

    /// The legacy `Items` list as one group. Kept separate from
    /// [`Self::from_control`] so the migration is testable on its own.
    pub fn from_items(items: &str) -> Self {
        let labels: Vec<&str> = items
            .lines()
            .map(str::trim)
            .filter(|l| !l.is_empty())
            .collect();
        if labels.is_empty() {
            return Self {
                groups: Vec::new(),
                button_gap: default_button_gap(),
            };
        }
        let mut group = ToolbarGroup::new("group-1", "Items");
        // The frame is off: these buttons were never grouped, and drawing a box
        // round them would change how an existing form looks.
        group.border_style = "None".into();
        for (n, label) in labels.iter().enumerate() {
            group
                .buttons
                .push(ToolbarButton::new(format!("button-{}", n + 1), *label));
        }
        Self {
            groups: vec![group],
            button_gap: default_button_gap(),
        }
    }

    /// What a brand-new ToolBar starts as: one group holding one button, with
    /// the folder-open icon (operator, 2026-08-17).
    ///
    /// An empty toolbar shows nothing but a hint, which tells a developer
    /// nothing about what a toolbar IS. One real button does — and it is a
    /// button, so pressing it fires the toolbar's `onClick` like any other.
    pub fn example() -> Self {
        let mut group = ToolbarGroup::new("group-1", "File");
        // The default frame is off, so the example reads as buttons on the form
        // rather than as a box the developer has to go and switch off.
        group.border_style = "None".into();
        let mut button = ToolbarButton::new("button-1", "");
        button.set_icon("folder-open");
        button.tooltip = "Open".into();
        group.buttons.push(button);
        Self {
            groups: vec![group],
            button_gap: default_button_gap(),
        }
    }

    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string(self)
    }

    /// Whether anything at all is defined.
    pub fn is_empty(&self) -> bool {
        self.groups.iter().all(|g| g.buttons.is_empty())
    }

    /// Every button, with the group it belongs to, in draw order.
    pub fn buttons(&self) -> impl Iterator<Item = (&ToolbarGroup, &ToolbarButton)> {
        self.groups
            .iter()
            .flat_map(|g| g.buttons.iter().map(move |b| (g, b)))
    }

    /// Find a button by id, wherever it is.
    pub fn button(&self, id: &str) -> Option<&ToolbarButton> {
        self.button_with_group(id).map(|(_, b)| b)
    }

    /// Find a button by id together with the group holding it — what
    /// [`button_control_id`] needs, since a button's outside-world id is built
    /// from all three names.
    pub fn button_with_group(&self, id: &str) -> Option<(&ToolbarGroup, &ToolbarButton)> {
        self.buttons().find(|(_, b)| b.id.eq_ignore_ascii_case(id))
    }

    /// An id no group is using yet.
    pub fn next_group_id(&self) -> String {
        let mut n = self.groups.len() + 1;
        while self.groups.iter().any(|g| g.id == format!("group-{n}")) {
            n += 1;
        }
        format!("group-{n}")
    }

    /// An id no button is using yet, anywhere on the toolbar — button ids are
    /// what a handler and an action refer to, so they are unique per toolbar,
    /// not per group.
    pub fn next_button_id(&self) -> String {
        let mut n = self.buttons().count() + 1;
        while self.button(&format!("button-{n}")).is_some() {
            n += 1;
        }
        format!("button-{n}")
    }

    /// Total width the toolbar wants, so the designer can warn when it does not
    /// fit the control it is on.
    pub fn layout_width(&self) -> i64 {
        self.groups
            .iter()
            .map(|g| g.layout_width(self.button_gap))
            .sum()
    }

    /// Where every group frame and every button goes, in toolbar-local pixels.
    ///
    /// Pure geometry — no egui, so it is testable without a window, and the
    /// designer canvas, the preview, the running form and the compiled binary
    /// all lay a toolbar out identically because they all call this.
    ///
    /// Groups run left to right. A group is as tall as its tallest button plus
    /// its padding, centred vertically in `bar_h`; a button is centred in its
    /// group. A group that would start past `bar_w` is dropped rather than drawn
    /// half off the end — the designer reports the overflow instead
    /// ([`Self::layout_width`] against the control's width).
    pub fn layout(&self, bar_w: i64, bar_h: i64) -> Vec<GroupLayout> {
        let mut out = Vec::with_capacity(self.groups.len());
        let mut x = 0i64;
        for (index, group) in self.groups.iter().enumerate() {
            if group.buttons.is_empty() {
                continue;
            }
            let tallest = group
                .buttons
                .iter()
                .map(|b| b.resolved(group).height)
                .max()
                .unwrap_or(0);
            let frame_h = (tallest + group.padding * 2).min(bar_h.max(1));
            let frame_w = group.layout_width(self.button_gap)
                - if group.separator_after {
                    group.separator_width
                } else {
                    0
                };
            if x >= bar_w {
                break;
            }
            let frame = Box2 {
                x,
                y: (bar_h - frame_h) / 2,
                w: frame_w,
                h: frame_h,
            };
            let mut buttons = Vec::with_capacity(group.buttons.len());
            let mut bx = x + group.padding;
            for button in &group.buttons {
                let style = button.resolved(group);
                buttons.push((
                    button.id.clone(),
                    Box2 {
                        x: bx,
                        y: frame.y + (frame_h - style.height) / 2,
                        w: style.width,
                        h: style.height,
                    },
                ));
                bx += style.width + self.button_gap;
            }
            out.push(GroupLayout {
                group_index: index,
                frame,
                buttons,
            });
            x += group.layout_width(self.button_gap);
        }
        out
    }
}

// ── Naming a button from outside its toolbar ───────────────────────────────────

/// The longest derived button id the generated event loop can dispatch.
///
/// `COBOL-CONTROL-ID` is `PIC X(64)`, so an id longer than that arrives
/// truncated and could never match its own `WHEN` — a button that looks bound
/// and is dead. Codegen reports one it cannot dispatch instead of emitting a
/// `WHEN` that can never fire.
pub const MAX_BUTTON_CONTROL_ID: usize = 64;

/// The id a toolbar button answers to OUTSIDE its toolbar:
/// `<toolbar>-<group>-<button>`, upper-cased (operator, 2026-08-17).
///
/// A button is deliberately not a [`Control`] — the toolbar owns its layout, so
/// there is nothing in `form.controls` to name it, and nothing for the designer's
/// drag/resize machinery to grab. But the two places that must name it, must
/// name it the SAME: the event the renderer fires when the button is pressed,
/// and the `WHEN` the generated event loop dispatches on. Both call this.
///
/// Ids inside a [`ToolbarDef`] are machine-made (`group-1`, `button-2`), so the
/// result is a valid COBOL word by construction. A hand-edited `.cfrm` could put
/// anything there, so anything outside `A-Z 0-9 -` is mapped to a hyphen rather
/// than trusted into generated source.
pub fn button_control_id(toolbar_id: &str, group_id: &str, button_id: &str) -> String {
    let mut out = String::with_capacity(
        toolbar_id.len() + group_id.len() + button_id.len() + 2,
    );
    for (n, part) in [toolbar_id, group_id, button_id].iter().enumerate() {
        if n > 0 {
            out.push('-');
        }
        for ch in part.trim().chars() {
            if ch.is_ascii_alphanumeric() {
                out.push(ch.to_ascii_uppercase());
            } else {
                out.push('-');
            }
        }
    }
    out
}

/// A rectangle in toolbar-local pixels. Deliberately not egui's `Rect`: the
/// layout is model-side so it can be tested and reasoned about without a window.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Box2 {
    pub x: i64,
    pub y: i64,
    pub w: i64,
    pub h: i64,
}

impl Box2 {
    pub fn right(&self) -> i64 {
        self.x + self.w
    }
    pub fn bottom(&self) -> i64 {
        self.y + self.h
    }
    pub fn contains(&self, px: i64, py: i64) -> bool {
        px >= self.x && px < self.right() && py >= self.y && py < self.bottom()
    }
}

/// One group's place on the bar, and its buttons' places within it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GroupLayout {
    /// Index into [`ToolbarDef::groups`] — empty groups are skipped, so this is
    /// not the position in this vector.
    pub group_index: usize,
    pub frame: Box2,
    pub buttons: Vec<(String, Box2)>,
}

/// Which button is at a toolbar-local point, if any. What a click resolves to.
pub fn button_at(layout: &[GroupLayout], px: i64, py: i64) -> Option<&str> {
    layout
        .iter()
        .flat_map(|g| g.buttons.iter())
        .find(|(_, box2)| box2.contains(px, py))
        .map(|(id, _)| id.as_str())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{ControlType, PropValue};

    #[test]
    fn an_action_round_trips_through_its_stored_string() {
        let cases = [
            ("event", ToolbarAction::Event),
            ("procedure:UPDATE-TOTAL", ToolbarAction::Procedure("UPDATE-TOTAL".into())),
            ("open-modal:CUST-LOOKUP", ToolbarAction::OpenModal("CUST-LOOKUP".into())),
            ("print:/tmp/report.pdf", ToolbarAction::Print("/tmp/report.pdf".into())),
            ("share", ToolbarAction::Share),
            ("screenshot", ToolbarAction::Screenshot),
            ("copy", ToolbarAction::Copy),
            ("cut", ToolbarAction::Cut),
            ("paste", ToolbarAction::Paste),
            ("run-app:/usr/bin/vim", ToolbarAction::RunApp("/usr/bin/vim".into())),
            ("open-terminal:/tmp", ToolbarAction::OpenTerminal("/tmp".into())),
        ];
        for (stored, want) in &cases {
            assert_eq!(&ToolbarAction::parse(stored), want, "parsing {stored:?}");
            assert_eq!(&want.to_action_string(), stored, "writing {want:?}");
        }

        // A verb from a newer PowerRustCOBOL degrades to the button's own
        // handler rather than refusing to open the form.
        assert_eq!(ToolbarAction::parse("teleport:MARS"), ToolbarAction::Event);
        assert_eq!(ToolbarAction::parse(""), ToolbarAction::Event);
        // Case and stray spacing are the developer's, not the format's.
        assert_eq!(
            ToolbarAction::parse("  PRINT :  /tmp/a.pdf  "),
            ToolbarAction::Print("/tmp/a.pdf".into())
        );

        // The platform carries out everything except the three that are the
        // form's own code.
        for cobol in ["event", "procedure:P", "open-modal:F"] {
            assert!(!ToolbarAction::parse(cobol).is_platform_action(), "{cobol}");
        }
        for platform in ["print:/a", "share", "screenshot", "copy", "cut", "paste", "run-app:/a", "open-terminal:"] {
            assert!(ToolbarAction::parse(platform).is_platform_action(), "{platform}");
        }
        // Every verb the editor offers must parse to itself.
        for verb in ToolbarAction::VERBS {
            let raw = if ToolbarAction::takes_target(verb) {
                format!("{verb}:X")
            } else {
                (*verb).to_owned()
            };
            assert_eq!(
                ToolbarAction::parse(&raw).verb(),
                *verb,
                "the editor offers {verb} but it does not round-trip"
            );
        }

        println!(
            "\n  Toolbar actions — {} verbs round-trip; an unknown verb degrades to \
             the button's own onClick; 8 of them are the platform's work\n",
            ToolbarAction::VERBS.len()
        );
    }

    #[test]
    fn groups_and_buttons_are_born_rounded_and_theme_coloured() {
        let g = ToolbarGroup::new("group-1", "Clipboard");
        let b = ToolbarButton::new("button-1", "Copy");
        let resolved = b.resolved(&g);
        // Operator decision: 10, for both.
        assert_eq!(g.corner_radius, DEFAULT_CORNER_RADIUS);
        assert_eq!(resolved.corner_radius, DEFAULT_CORNER_RADIUS);
        assert_eq!(DEFAULT_CORNER_RADIUS, 10);
        // Every colour empty = the form's theme decides.
        for colour in [
            &g.border_color,
            &g.background_color,
            &resolved.background_color,
            &resolved.foreground_color,
            &resolved.icon_color,
        ] {
            assert!(colour.is_empty(), "a new element must carry no colour of its own");
        }
        assert!(b.enabled, "a new button is enabled");
        assert_eq!(b.action().verb(), "event", "…and runs its own onClick");
        assert!(g.draws_frame(), "a new group has a frame");

        // `None` still groups — padding and separator apply — but draws nothing.
        let mut invisible = ToolbarGroup::new("g", "");
        invisible.border_style = "None".into();
        assert!(!invisible.draws_frame());
        assert_eq!(invisible.padding, DEFAULT_GROUP_PADDING);

        println!(
            "\n  Toolbar defaults — radius {} on groups and buttons, every colour \
             unset (theme decides), enabled, own onClick\n",
            DEFAULT_CORNER_RADIUS
        );
    }

    /// A toolbar built before groups existed must not lose its buttons.
    #[test]
    fn a_legacy_items_toolbar_becomes_one_unframed_group() {
        let mut ctrl = Control::new("TB-1", ControlType::ToolBar, 0, 0);
        // A control loaded from a `.cfrm` written before groups existed has no
        // definition at all — only `Items`. A NEW control is seeded with the
        // example toolbar, so that has to come off to be the legacy case.
        ctrl.properties.shift_remove(TOOLBAR_DEF_PROP);
        ctrl.set_prop("Items", PropValue::String("a\nb\n\nc\n".into()));

        let def = ToolbarDef::from_control(&ctrl);
        assert_eq!(def.groups.len(), 1, "one group");
        let g = &def.groups[0];
        assert_eq!(
            g.buttons.iter().map(|b| b.label.as_str()).collect::<Vec<_>>(),
            vec!["a", "b", "c"],
            "each item becomes a button, blank lines dropped, order kept"
        );
        assert!(
            !g.draws_frame(),
            "a frame round buttons that were never grouped would change how an \
             existing form looks"
        );
        assert_eq!(def.button(&g.buttons[1].id).map(|b| b.label.as_str()), Some("b"));

        // A stored definition wins over Items.
        let mut real = ToolbarGroup::new("group-1", "Real");
        real.buttons.push(ToolbarButton::new("button-1", "Save"));
        let stored = ToolbarDef {
            groups: vec![real],
            button_gap: 4,
        };
        ctrl.set_prop(
            TOOLBAR_DEF_PROP,
            PropValue::String(stored.to_json().unwrap()),
        );
        let def = ToolbarDef::from_control(&ctrl);
        assert_eq!(def, stored, "the stored definition is what is used");

        // Unreadable JSON falls back to Items rather than losing the toolbar.
        ctrl.set_prop(TOOLBAR_DEF_PROP, PropValue::String("{not json".into()));
        assert_eq!(ToolbarDef::from_control(&ctrl).groups[0].buttons.len(), 3);

        // A control with neither is empty, not an error.
        let mut bare = Control::new("TB-2", ControlType::ToolBar, 0, 0);
        bare.properties.shift_remove(TOOLBAR_DEF_PROP);
        assert!(ToolbarDef::from_control(&bare).is_empty());

        // …while a BRAND-NEW control comes with the example: one group, one
        // folder-open button, so a dropped toolbar shows what a toolbar is.
        let fresh = Control::new("TB-3", ControlType::ToolBar, 0, 0);
        let def = ToolbarDef::from_control(&fresh);
        assert_eq!(def.groups.len(), 1, "one group");
        assert_eq!(def.groups[0].buttons.len(), 1, "one button");
        assert_eq!(def.groups[0].buttons[0].icon, "folder-open");
        assert!(
            def.groups[0].buttons[0].label.is_empty(),
            "an icon button carries no label"
        );
        assert!(!def.groups[0].draws_frame(), "the example group has no frame");
        // And the control's own frame: rounded 10, no border, fully transparent.
        assert_eq!(fresh.get_prop("CornerRadius").map(|v| v.as_i64()), Some(10));
        assert_eq!(
            fresh.get_prop("BorderStyle").map(|v| v.as_str().to_owned()),
            Some("None".to_owned())
        );
        assert_eq!(fresh.get_prop("Transparency").map(|v| v.as_i64()), Some(100));

        println!(
            "\n  Toolbar migration — Items \"a\\nb\\n\\nc\" ⇒ 1 unframed group of 3 \
             buttons; a stored definition wins; bad JSON falls back to Items; a bare \
             control is empty\n"
        );
    }

    #[test]
    fn the_definition_survives_json_and_ids_never_collide() {
        let mut def = ToolbarDef::default();
        let g1 = def.next_group_id();
        let mut group = ToolbarGroup::new(&g1, "Clipboard");
        for label in ["Copy", "Cut", "Paste"] {
            let id = {
                let mut probe = def.clone();
                probe.groups.push(group.clone());
                probe.next_button_id()
            };
            let mut b = ToolbarButton::new(id, label);
            b.icon = label.to_ascii_lowercase();
            b.action = label.to_ascii_lowercase();
            group.buttons.push(b);
        }
        group.separator_after = true;
        def.groups.push(group);

        let json = def.to_json().expect("serialises");
        let back: ToolbarDef = serde_json::from_str(&json).expect("deserialises");
        assert_eq!(back, def, "the definition survives a JSON round-trip");

        // Ids are unique across the whole toolbar, not per group.
        let ids: Vec<&str> = def.buttons().map(|(_, b)| b.id.as_str()).collect();
        let mut sorted = ids.clone();
        sorted.sort_unstable();
        sorted.dedup();
        assert_eq!(sorted.len(), ids.len(), "button ids collide: {ids:?}");
        assert_ne!(def.next_group_id(), g1);

        // Width accounts for padding, buttons, gaps and the separator.
        let g = &def.groups[0];
        let want = g.padding * 2 + 3 * DEFAULT_BUTTON_SIZE.0 + 2 * def.button_gap + g.separator_width;
        assert_eq!(def.layout_width(), want);

        println!(
            "\n  Toolbar definition — 1 group of 3 buttons round-trips through JSON \
             ({} bytes), ids unique toolbar-wide, layout width {}px \
             (padding {}×2 + 3 buttons + 2 gaps + separator {})\n",
            json.len(),
            def.layout_width(),
            g.padding,
            g.separator_width
        );
    }

    /// A group's settings are its buttons' defaults, and a button's own values
    /// win over them (operator, 2026-08-17).
    #[test]
    fn a_group_dresses_its_buttons_and_a_button_can_still_disagree() {
        let mut g = ToolbarGroup::new("group-1", "File");
        // Set once on the group instead of on each of three buttons.
        g.button_defaults.icon_size = Some(30);
        g.button_defaults.width = Some(44);
        g.button_defaults.background_color = "#204080FF".into();
        g.button_defaults.shadow = Some(true);
        g.button_defaults.shadow_opacity = Some(40);

        let plain = ToolbarButton::new("b1", "A");
        let mut own = ToolbarButton::new("b2", "B");
        own.style.icon_size = Some(12);
        own.style.background_color = "#FF0000FF".into();
        own.style.shadow = Some(false);
        g.buttons.push(plain);
        g.buttons.push(own);

        // A button that says nothing takes the group's word for everything.
        let a = g.buttons[0].resolved(&g);
        assert_eq!(a.icon_size, 30);
        assert_eq!(a.width, 44);
        assert_eq!(a.background_color, "#204080FF");
        assert!(a.shadow);
        assert_eq!(a.shadow_opacity, 40);
        // …and for anything the group ALSO left alone, the built-in value.
        assert_eq!(a.height, DEFAULT_BUTTON_SIZE.1);
        assert_eq!(a.corner_radius, DEFAULT_CORNER_RADIUS);
        assert!(a.foreground_color.is_empty(), "still the theme's");

        // A button that disagrees wins, field by field — the ones it did not
        // mention still come from the group.
        let b = g.buttons[1].resolved(&g);
        assert_eq!(b.icon_size, 12, "its own size");
        assert_eq!(b.background_color, "#FF0000FF", "its own colour");
        assert!(!b.shadow, "its own switch, and `false` is a real choice");
        assert_eq!(b.width, 44, "the group's width, which it never overrode");
        assert_eq!(b.shadow_opacity, 40, "…and the group's opacity");

        // The layout measures the RESOLVED width, so a size set on the group is
        // the size the bar is laid out at.
        assert_eq!(g.layout_width(4), g.padding * 2 + 44 + 44 + 4);

        println!(
            "\n  Toolbar inheritance — group sets icon 30 / width 44 / #204080 / shadow \
             40%: an unset button takes all of it, a button overriding size+colour+shadow \
             keeps the group's width and opacity; layout measures {}px\n",
            g.layout_width(4)
        );
    }

    /// A button's id OUTSIDE its toolbar. The renderer fires a press under it and
    /// the generated event loop dispatches on it, so the two must agree exactly —
    /// which is why there is one function and not two conventions.
    #[test]
    fn a_button_is_named_by_its_toolbar_its_group_and_itself() {
        assert_eq!(
            button_control_id("TOOLBAR-1", "group-1", "button-1"),
            "TOOLBAR-1-GROUP-1-BUTTON-1"
        );
        // Case is the designer's, not the format's.
        assert_eq!(
            button_control_id("Tb", "Clipboard", "Copy"),
            "TB-CLIPBOARD-COPY"
        );
        // Ids inside a definition are machine-made, but a hand-edited `.cfrm`
        // can hold anything — and nothing but a COBOL word may reach generated
        // source, so everything else becomes a hyphen.
        assert_eq!(
            button_control_id("TB 1", "my group!", "btn.2"),
            "TB-1-MY-GROUP--BTN-2"
        );
        assert_eq!(
            button_control_id("  TB  ", " g ", " b "),
            "TB-G-B",
            "surrounding space is not part of a name"
        );

        // The result is a COBOL word: starts with a letter, then letters, digits
        // and hyphens.
        let id = button_control_id("TOOLBAR-1", "group-1", "button-1");
        assert!(crate::model::is_valid_control_id(&id), "{id} is not addressable");

        // And it must fit COBOL-CONTROL-ID, or the WHEN it generates could never
        // match. Codegen reports one that does not rather than emitting it.
        assert!(id.len() <= MAX_BUTTON_CONTROL_ID);
        assert_eq!(MAX_BUTTON_CONTROL_ID, 64, "COBOL-CONTROL-ID is PIC X(64)");

        // A press resolves button → group → derived id through one lookup.
        let mut def = ToolbarDef::default();
        let mut g = ToolbarGroup::new("group-7", "Output");
        g.buttons.push(ToolbarButton::new("button-3", "Print"));
        def.groups.push(g);
        let (group, button) = def.button_with_group("button-3").expect("found");
        assert_eq!(
            button_control_id("BAR", &group.id, &button.id),
            "BAR-GROUP-7-BUTTON-3"
        );
        assert!(def.button_with_group("nope").is_none());

        println!(
            "\n  Toolbar button ids — TOOLBAR-1/group-1/button-1 ⇒ \
             \"TOOLBAR-1-GROUP-1-BUTTON-1\"; case is normalised, anything outside a \
             COBOL word becomes a hyphen, and the result fits COBOL-CONTROL-ID's \
             {MAX_BUTTON_CONTROL_ID} characters\n"
        );
    }

    /// A button shows a label OR an icon, never both.
    #[test]
    fn a_label_and_an_icon_are_mutually_exclusive() {
        let mut b = ToolbarButton::new("b1", "Save");
        assert_eq!(b.label, "Save");
        assert!(b.icon.is_empty());

        // An icon takes the label away.
        b.set_icon("folder-open");
        assert_eq!(b.icon, "folder-open");
        assert!(b.label.is_empty(), "the label must go when an icon arrives");

        // …and a label takes the icon away.
        b.set_label("Open");
        assert_eq!(b.label, "Open");
        assert!(b.icon.is_empty(), "the icon must go when a label arrives");

        // Clearing one does NOT invent the other: a button with neither is
        // legal (an empty spacer), so setting blank is not a swap.
        b.set_label("");
        assert!(b.label.is_empty() && b.icon.is_empty());
        b.set_icon("");
        assert!(b.label.is_empty() && b.icon.is_empty());
        // Blank-but-whitespace counts as blank, not as a value that clears.
        b.set_icon("home");
        b.set_label("   ");
        assert_eq!(b.icon, "home", "whitespace is not a label");

        println!(
            "\n  Toolbar button — label and icon are exclusive: set_icon clears the \
             label, set_label clears the icon, and setting either to blank (or \
             whitespace) clears nothing\n"
        );
    }

    /// The geometry every surface shares: groups left to right, the separator
    /// showing up as a gap between frames rather than as anything drawn, and a
    /// click landing on exactly one button.
    #[test]
    fn groups_lay_out_left_to_right_with_the_separator_as_a_gap() {
        let mut def = ToolbarDef::default();

        let mut clip = ToolbarGroup::new("group-1", "Clipboard");
        for (n, label) in ["Copy", "Cut", "Paste"].iter().enumerate() {
            clip.buttons
                .push(ToolbarButton::new(format!("b{}", n + 1), *label));
        }
        clip.separator_after = true;
        def.groups.push(clip);

        let mut out = ToolbarGroup::new("group-2", "Output");
        out.buttons.push(ToolbarButton::new("b4", "Print"));
        def.groups.push(out);

        // An empty group is skipped entirely — it has nothing to frame.
        def.groups.push(ToolbarGroup::new("group-3", "Empty"));

        let bar_h = 40;
        let l = def.layout(1000, bar_h);
        assert_eq!(l.len(), 2, "the empty group is not laid out");
        assert_eq!(l[1].group_index, 1, "indices point back at the real groups");

        // Group 1: padding, 3 buttons, 2 gaps — and the separator is NOT part of
        // the frame, only of the distance to the next group.
        let g1 = &l[0];
        let frame_w = 6 * 2 + 3 * DEFAULT_BUTTON_SIZE.0 + 2 * def.button_gap;
        assert_eq!(g1.frame.x, 0);
        assert_eq!(g1.frame.w, frame_w, "the separator is outside the frame");
        assert_eq!(g1.frame.h, DEFAULT_BUTTON_SIZE.1 + 6 * 2);
        assert_eq!(
            g1.frame.y,
            (bar_h - g1.frame.h) / 2,
            "the group is centred in the bar"
        );

        // The gap between the two frames IS the separator.
        let g2 = &l[1];
        assert_eq!(
            g2.frame.x - g1.frame.right(),
            DEFAULT_SEPARATOR_WIDTH,
            "the invisible separator sets the groups apart"
        );

        // Buttons: inside the padding, in order, gap between them.
        let b: Vec<&Box2> = g1.buttons.iter().map(|(_, r)| r).collect();
        assert_eq!(b[0].x, g1.frame.x + 6);
        assert_eq!(b[1].x - b[0].right(), def.button_gap);
        for r in &b {
            assert!(
                r.y >= g1.frame.y && r.bottom() <= g1.frame.bottom(),
                "a button must sit inside its frame: {r:?} in {:?}",
                g1.frame
            );
        }

        // A click lands on exactly one button, and on none in the padding or the
        // separator.
        assert_eq!(button_at(&l, b[0].x + 1, b[0].y + 1), Some("b1"));
        assert_eq!(button_at(&l, b[2].x + 1, b[2].y + 1), Some("b3"));
        assert_eq!(button_at(&l, g2.buttons[0].1.x + 1, g2.buttons[0].1.y + 1), Some("b4"));
        assert_eq!(
            button_at(&l, g1.frame.right() + 2, bar_h / 2),
            None,
            "the separator is not pressable"
        );
        assert_eq!(button_at(&l, g1.frame.x, g1.frame.y), None, "padding is not pressable");

        // A bar too narrow drops whole groups rather than drawing half of one.
        let narrow = def.layout(frame_w, bar_h);
        assert_eq!(narrow.len(), 1, "only what starts inside the bar is laid out");

        println!(
            "\n  Toolbar layout — 2 groups ({} buttons) on a {}px bar: frame 1 at x0 \
             w{}, frame 2 at x{} ({}px separator between), buttons inside their \
             padding; clicks resolve to exactly one button, separator and padding \
             to none; a {}px bar drops the second group\n",
            def.buttons().count(),
            bar_h,
            g1.frame.w,
            g2.frame.x,
            DEFAULT_SEPARATOR_WIDTH,
            frame_w
        );
    }
}
