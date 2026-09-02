// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! The Snackbar's **pure** parts (spec 055).
//!
//! A Snackbar is a transient, non-modal notification. The control the developer
//! drops on the form is the **template** — it carries the defaults and never
//! paints itself; every `Show()` mints a new notification from those values into
//! the surface's stack (055 D1/D2).
//!
//! Everything here is a function of its arguments: size classes, category
//! defaults, `Buttons` parsing, content layout and stack geometry. There is **no
//! clock and no state** — a notification's lifetime belongs to
//! `cobolt-form-host`, which owns cross-frame state and can be driven by a
//! fabricated instant in a test. Splitting it that way is what lets the same
//! arithmetic serve `rcrun run-form`, a compiled binary and the designer without
//! any of them owning a live timer (plan §1).
//!
//! Deliberately **not** behind the `render` feature, for the reason
//! [`crate::splitter`] is not: the geometry is read by the host and the runtime,
//! and a second copy behind a feature gate is free to disagree with this one.

use crate::model::{Control, Rect};

// ── The vocabulary ───────────────────────────────────────────────────────────

/// What a notification is *about*. Supplies the colours, the icon and the
/// timeout a developer did not set (055 §7, R23).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SnackCategory {
    #[default]
    Info,
    Question,
    Warning,
    Error,
    Critical,
}

/// How much room a notification takes, and how many lines of text it will show
/// before ellipsizing (R22).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SnackSize {
    Small,
    #[default]
    Medium,
    Large,
}

/// Where the stack sits on the surface (R16). Nine positions, resolved against
/// the FORM's own surface — never the desktop (R17).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SnackAnchor {
    TopLeft,
    TopCenter,
    TopRight,
    CenterLeft,
    Center,
    CenterRight,
    BottomLeft,
    BottomCenter,
    #[default]
    BottomRight,
}

/// Which end of the stack the newest notification takes (R12/R13).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum StackOrder {
    /// Follow the anchor: Top grows down, Bottom grows up, Centre newest-first.
    #[default]
    Auto,
    NewestFirst,
    NewestLast,
}

/// What a `Show()` does once `MaximumVisible` notifications are already up (R15).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum OverflowBehavior {
    /// Hold the new one back and raise it when a slot frees.
    #[default]
    Queue,
    /// Dismiss the oldest live notification to make room.
    DiscardOldest,
    /// Drop the notification being raised.
    DiscardNewest,
}

/// Why a notification left. Reported verbatim to `onClosing` / `onClosed`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DismissReason {
    Timeout,
    User,
    Action,
    Programmatic,
    Overflow,
}

impl DismissReason {
    /// The name COBOL sees. **English in every UI language** — this is a value a
    /// handler compares against, not a label (the CRITICAL constraint).
    pub fn as_str(self) -> &'static str {
        match self {
            DismissReason::Timeout => "Timeout",
            DismissReason::User => "User",
            DismissReason::Action => "Action",
            DismissReason::Programmatic => "Programmatic",
            DismissReason::Overflow => "Overflow",
        }
    }
}

/// Where a button's icon sits relative to its caption.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ButtonIconPosition {
    None,
    #[default]
    Left,
    Right,
}

// Each enum parses from the property string, and an unrecognised value falls
// back to the default rather than failing. A form copied from another project
// may carry a partial or misspelled property set — that must mean "the default",
// never a panic and never a silently wrong value (plan §5, the DataGrid hunt).

impl SnackCategory {
    pub fn from_prop(s: &str) -> Self {
        match s.trim().to_ascii_lowercase().as_str() {
            "question" => SnackCategory::Question,
            "warning" => SnackCategory::Warning,
            "error" => SnackCategory::Error,
            "critical" => SnackCategory::Critical,
            _ => SnackCategory::Info,
        }
    }
    pub fn as_str(self) -> &'static str {
        match self {
            SnackCategory::Info => "Info",
            SnackCategory::Question => "Question",
            SnackCategory::Warning => "Warning",
            SnackCategory::Error => "Error",
            SnackCategory::Critical => "Critical",
        }
    }
    pub const ALL: [SnackCategory; 5] = [
        SnackCategory::Info,
        SnackCategory::Question,
        SnackCategory::Warning,
        SnackCategory::Error,
        SnackCategory::Critical,
    ];
}

impl SnackSize {
    pub fn from_prop(s: &str) -> Self {
        match s.trim().to_ascii_lowercase().as_str() {
            "small" => SnackSize::Small,
            "large" => SnackSize::Large,
            _ => SnackSize::Medium,
        }
    }
    pub fn as_str(self) -> &'static str {
        match self {
            SnackSize::Small => "Small",
            SnackSize::Medium => "Medium",
            SnackSize::Large => "Large",
        }
    }
    pub const ALL: [SnackSize; 3] = [SnackSize::Small, SnackSize::Medium, SnackSize::Large];
}

impl SnackAnchor {
    pub fn from_prop(s: &str) -> Self {
        match s.trim().to_ascii_lowercase().replace(['-', '_', ' '], "").as_str() {
            "topleft" => SnackAnchor::TopLeft,
            "topcenter" | "topcentre" => SnackAnchor::TopCenter,
            "topright" => SnackAnchor::TopRight,
            "centerleft" | "centreleft" => SnackAnchor::CenterLeft,
            "center" | "centre" => SnackAnchor::Center,
            "centerright" | "centreright" => SnackAnchor::CenterRight,
            "bottomleft" => SnackAnchor::BottomLeft,
            "bottomcenter" | "bottomcentre" => SnackAnchor::BottomCenter,
            _ => SnackAnchor::BottomRight,
        }
    }
    pub fn as_str(self) -> &'static str {
        match self {
            SnackAnchor::TopLeft => "TopLeft",
            SnackAnchor::TopCenter => "TopCenter",
            SnackAnchor::TopRight => "TopRight",
            SnackAnchor::CenterLeft => "CenterLeft",
            SnackAnchor::Center => "Center",
            SnackAnchor::CenterRight => "CenterRight",
            SnackAnchor::BottomLeft => "BottomLeft",
            SnackAnchor::BottomCenter => "BottomCenter",
            SnackAnchor::BottomRight => "BottomRight",
        }
    }
    pub const ALL: [SnackAnchor; 9] = [
        SnackAnchor::TopLeft,
        SnackAnchor::TopCenter,
        SnackAnchor::TopRight,
        SnackAnchor::CenterLeft,
        SnackAnchor::Center,
        SnackAnchor::CenterRight,
        SnackAnchor::BottomLeft,
        SnackAnchor::BottomCenter,
        SnackAnchor::BottomRight,
    ];

    /// True for the three Top anchors — the stack grows **downward** (R12).
    pub fn is_top(self) -> bool {
        matches!(self, SnackAnchor::TopLeft | SnackAnchor::TopCenter | SnackAnchor::TopRight)
    }
    /// True for the three Bottom anchors — the stack grows **upward** (R12).
    pub fn is_bottom(self) -> bool {
        matches!(
            self,
            SnackAnchor::BottomLeft | SnackAnchor::BottomCenter | SnackAnchor::BottomRight
        )
    }
    /// True for the three Centre-row anchors — newest first, growing down (R13).
    pub fn is_middle(self) -> bool {
        !self.is_top() && !self.is_bottom()
    }
}

impl StackOrder {
    pub fn from_prop(s: &str) -> Self {
        match s.trim().to_ascii_lowercase().as_str() {
            "newestfirst" => StackOrder::NewestFirst,
            "newestlast" => StackOrder::NewestLast,
            _ => StackOrder::Auto,
        }
    }
    pub fn as_str(self) -> &'static str {
        match self {
            StackOrder::Auto => "Auto",
            StackOrder::NewestFirst => "NewestFirst",
            StackOrder::NewestLast => "NewestLast",
        }
    }
}

impl OverflowBehavior {
    pub fn from_prop(s: &str) -> Self {
        match s.trim().to_ascii_lowercase().replace(['-', '_', ' '], "").as_str() {
            "discardoldest" => OverflowBehavior::DiscardOldest,
            "discardnewest" => OverflowBehavior::DiscardNewest,
            _ => OverflowBehavior::Queue,
        }
    }
    pub fn as_str(self) -> &'static str {
        match self {
            OverflowBehavior::Queue => "Queue",
            OverflowBehavior::DiscardOldest => "DiscardOldest",
            OverflowBehavior::DiscardNewest => "DiscardNewest",
        }
    }
}

impl ButtonIconPosition {
    pub fn from_prop(s: &str) -> Self {
        match s.trim().to_ascii_lowercase().as_str() {
            "none" => ButtonIconPosition::None,
            "right" => ButtonIconPosition::Right,
            _ => ButtonIconPosition::Left,
        }
    }
    pub fn as_str(self) -> &'static str {
        match self {
            ButtonIconPosition::None => "None",
            ButtonIconPosition::Left => "Left",
            ButtonIconPosition::Right => "Right",
        }
    }
}

// ── Size classes ─────────────────────────────────────────────────────────────

/// Everything a size class fixes, in form points.
///
/// `height` is derived, not stored: it is `pad_y * 2 + line_h * lines_used`,
/// floored at `min_height`, so a one-line Medium is compact and a three-line
/// Large is tall — while content stays vertically centred either way (R18).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SizeMetrics {
    /// Narrowest a notification may be.
    pub min_width: f32,
    /// Widest it may grow before the text wraps.
    pub max_width: f32,
    /// Floor for the derived height.
    pub min_height: f32,
    /// Inner padding, left/right — the `Margin →` and `→ Margin` of R19.
    pub pad_x: f32,
    /// Inner padding, top/bottom.
    pub pad_y: f32,
    /// The category icon's square side, when `CategoryIconSize` is 0.
    pub icon: f32,
    /// The gap between icon and text, and between text and the buttons.
    pub gap: f32,
    /// One line of text.
    pub line_h: f32,
    /// How many lines before the text is ellipsized (R22).
    pub line_budget: usize,
    /// A button's height; its width is its content plus `pad_x`.
    pub button_h: f32,
}

impl SnackSize {
    /// The class's fixed measurements.
    pub fn metrics(self) -> SizeMetrics {
        match self {
            SnackSize::Small => SizeMetrics {
                min_width: 220.0,
                max_width: 420.0,
                min_height: 40.0,
                pad_x: 12.0,
                pad_y: 8.0,
                icon: 18.0,
                gap: 10.0,
                line_h: 18.0,
                line_budget: 1,
                button_h: 24.0,
            },
            SnackSize::Medium => SizeMetrics {
                min_width: 280.0,
                max_width: 520.0,
                min_height: 56.0,
                pad_x: 14.0,
                pad_y: 10.0,
                icon: 22.0,
                gap: 12.0,
                line_h: 20.0,
                line_budget: 2,
                button_h: 28.0,
            },
            SnackSize::Large => SizeMetrics {
                min_width: 320.0,
                max_width: 620.0,
                min_height: 76.0,
                pad_x: 16.0,
                pad_y: 12.0,
                icon: 26.0,
                gap: 14.0,
                line_h: 22.0,
                line_budget: 3,
                button_h: 32.0,
            },
        }
    }
}

// ── Category defaults ────────────────────────────────────────────────────────

/// What a category supplies when the developer set nothing (055 §7, R23).
///
/// These are **defaults, not appearance**: any explicitly set property wins, and
/// each one wins alone — setting `BackgroundColor` leaves the category's
/// foreground and icon in place.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CategoryDefaults {
    pub background: &'static str,
    pub foreground: &'static str,
    /// A **catalogue** icon name (`crate::icons`), so a notification's icon is
    /// the same artwork the toolbox and the menus draw.
    pub icon: &'static str,
    /// Milliseconds; `0` means it stays until dismissed.
    pub timeout_ms: i64,
}

impl SnackCategory {
    pub fn defaults(self) -> CategoryDefaults {
        match self {
            // The four familiar categories reuse the catalogue icons that
            // already mean exactly this — `info-circle`, `help-circle`,
            // `warning-triangle`, `error-circle`. Drawing five near-duplicates
            // beside them would have put a second vocabulary in the catalogue
            // for something it already names, which is the cost spec §8 exists
            // to avoid. Only Critical needed new artwork.
            SnackCategory::Info => CategoryDefaults {
                background: "#1E4E8C",
                foreground: "#F2F7FF",
                icon: "info-circle",
                timeout_ms: 4000,
            },
            SnackCategory::Question => CategoryDefaults {
                background: "#4B3A8C",
                foreground: "#F5F2FF",
                icon: "help-circle",
                timeout_ms: 6000,
            },
            SnackCategory::Warning => CategoryDefaults {
                background: "#8A5A0B",
                foreground: "#FFF7E8",
                icon: "warning-triangle",
                timeout_ms: 6000,
            },
            SnackCategory::Error => CategoryDefaults {
                background: "#8C2323",
                foreground: "#FFF0F0",
                icon: "error-circle",
                timeout_ms: 8000,
            },
            SnackCategory::Critical => CategoryDefaults {
                background: "#5A0F0F",
                foreground: "#FFEAEA",
                icon: "critical-octagon",
                // 0 = stays until dismissed (§7). Severe enough that letting it
                // time out would be the wrong default.
                timeout_ms: 0,
            },
        }
    }
}

// ── Reading the template ─────────────────────────────────────────────────────
//
// Every read defaults. A form copied from another project may carry a partial
// property set, and a missing property must mean "the documented default" —
// never a panic, never a silently wrong value (plan §5).

fn prop_str(ctrl: &Control, name: &str) -> String {
    ctrl.get_prop(name)
        .map(|v| v.as_str().to_owned())
        .unwrap_or_default()
}

fn prop_i64(ctrl: &Control, name: &str, default: i64) -> i64 {
    ctrl.get_prop(name).map(|v| v.as_i64()).unwrap_or(default)
}

fn prop_bool(ctrl: &Control, name: &str, default: bool) -> bool {
    ctrl.get_prop(name).map(|v| v.as_bool()).unwrap_or(default)
}

/// The timeout a notification actually gets, in milliseconds.
///
/// `Timeout` seeds `-1` — "ask the Category" — because R6 gives `0` the distinct
/// meaning "never expires", and Critical's own category default *is* 0. Seeding
/// 0 would have collapsed the two: every default Snackbar would have stayed up
/// forever and §7's 4000/6000/8000 ms would have been unreachable. `-1` is the
/// catalogue's established "ask the other one" sentinel (`CornerRadiusTopLeft`).
pub fn effective_timeout_ms(ctrl: &Control) -> i64 {
    let raw = prop_i64(ctrl, "Timeout", -1);
    if raw < 0 {
        category_of(ctrl).defaults().timeout_ms
    } else {
        raw
    }
}

/// The template's category.
pub fn category_of(ctrl: &Control) -> SnackCategory {
    SnackCategory::from_prop(&prop_str(ctrl, "Category"))
}

/// The template's size class.
pub fn size_of(ctrl: &Control) -> SnackSize {
    SnackSize::from_prop(&prop_str(ctrl, "Size"))
}

/// An explicitly set colour, or the category's (R23). Empty means "the category
/// decides" — that is why the property seeds empty rather than a concrete value.
pub fn effective_background(ctrl: &Control) -> String {
    let set = prop_str(ctrl, "BackgroundColor");
    if set.trim().is_empty() {
        category_of(ctrl).defaults().background.to_owned()
    } else {
        set
    }
}

/// An explicitly set ink colour, or the category's (R23).
pub fn effective_foreground(ctrl: &Control) -> String {
    let set = prop_str(ctrl, "ForegroundColor");
    if set.trim().is_empty() {
        category_of(ctrl).defaults().foreground.to_owned()
    } else {
        set
    }
}

/// The category icon's catalogue name, or `None` when `ShowCategoryIcon` is off.
pub fn effective_icon(ctrl: &Control) -> Option<String> {
    if !prop_bool(ctrl, "ShowCategoryIcon", true) {
        return None;
    }
    Some(category_of(ctrl).defaults().icon.to_owned())
}

/// The category icon's side in points: `CategoryIconSize` when set, else the
/// size class's.
pub fn effective_icon_size(ctrl: &Control) -> f32 {
    let set = prop_i64(ctrl, "CategoryIconSize", 0);
    if set > 0 {
        set as f32
    } else {
        size_of(ctrl).metrics().icon
    }
}

// ── Buttons ──────────────────────────────────────────────────────────────────

/// The most buttons one notification will show. A fourth is reported, never
/// silently dropped (spec Q5).
pub const MAX_BUTTONS: usize = 3;

/// One parsed button.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SnackButton {
    /// The id reported by `onButtonClick`. **English, developer-chosen** — a
    /// value COBOL compares against, never a translated label.
    pub id: String,
    /// The caption; empty means an icon-only button.
    pub text: String,
    /// A catalogue icon name, or empty for none.
    pub icon: String,
    pub position: ButtonIconPosition,
    /// Whether clicking it dismisses the notification (R8).
    pub dismiss: bool,
}

/// What `parse_buttons` found that the developer should know about.
///
/// A **designer-time warning**, not a build failure: the first three render and
/// the developer is told while the choice is still theirs, exactly as the
/// ContentPane-overflow warning does at 1.62.133 (plan §7, spec Q5).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ButtonsDiagnostic {
    /// More than [`MAX_BUTTONS`] lines. Carries how many were declared and the
    /// ids of the ones that will not be shown — naming them is the difference
    /// between a warning and a silent truncation.
    TooMany { declared: usize, dropped: Vec<String> },
}

/// Parse the `Buttons` property — one button per line, `|`-separated, trailing
/// fields omittable (spec §6):
///
/// ```text
/// retry|Retry|refresh|Left|true
/// close||x-mark|Left|true
/// ```
///
/// Blank lines are skipped. A line with no id at all is skipped too — there
/// would be nothing for `onButtonClick` to report.
///
/// Returns the buttons that will be shown (at most [`MAX_BUTTONS`]) and, when
/// there were more, a diagnostic naming what was left out.
pub fn parse_buttons(spec: &str) -> (Vec<SnackButton>, Option<ButtonsDiagnostic>) {
    let mut all: Vec<SnackButton> = Vec::new();
    for line in spec.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let mut f = line.split('|');
        let id = f.next().unwrap_or("").trim().to_owned();
        if id.is_empty() {
            continue;
        }
        let text = f.next().unwrap_or("").trim().to_owned();
        let icon = f.next().unwrap_or("").trim().to_owned();
        let position_field = f.next().unwrap_or("").trim().to_owned();
        let dismiss_field = f.next().unwrap_or("").trim().to_owned();

        // "default `Left` when an icon is given" (§6) — an omitted position on a
        // button with no icon is `None`, so a text-only button reserves no icon
        // slot it will never fill.
        let position = if position_field.is_empty() {
            if icon.is_empty() {
                ButtonIconPosition::None
            } else {
                ButtonIconPosition::Left
            }
        } else {
            ButtonIconPosition::from_prop(&position_field)
        };
        // Default true (§6); anything that is not a recognised "false" is true,
        // so a typo leaves the button dismissing rather than sticking.
        let dismiss = !matches!(
            dismiss_field.to_ascii_lowercase().as_str(),
            "false" | "no" | "0"
        );

        all.push(SnackButton { id, text, icon, position, dismiss });
    }

    if all.len() > MAX_BUTTONS {
        let declared = all.len();
        let dropped = all[MAX_BUTTONS..].iter().map(|b| b.id.clone()).collect();
        all.truncate(MAX_BUTTONS);
        (all, Some(ButtonsDiagnostic::TooMany { declared, dropped }))
    } else {
        (all, None)
    }
}

// ── AddButton() — declaring a button one call at a time ──────────────────────
//
// The `Buttons` property is one line per button, and a COBOL literal cannot
// contain a newline: `MOVE "a|A" TO SNACK-1::Buttons` can therefore only ever
// declare ONE button, however many `|` it carries. A handler that wanted two had
// to STRING them together around `FUNCTION CHAR(11)`, which is a trick, not an
// interface (operator, 2026-09-02).
//
// `Clear()` and `AddButton("key=value, …")` are that interface. They are still
// only ways of writing `Buttons` — the property stays the single source of
// truth, so the designer, the .cfrm, `mint`, the painter and the hit test all
// carry on reading exactly what they read before.

/// What one `AddButton()` argument declares: the button, and where in the row it
/// asked to sit.
#[derive(Debug, Clone, PartialEq)]
pub struct ButtonSpec {
    pub button: SnackButton,
    /// 1-based, left to right (operator, 2026-09-02). `None` = at the end, in
    /// call order.
    pub position: Option<usize>,
}

/// Split `key=value, key=value` into pairs, keys normalised.
///
/// A fragment with **no `=` is a comma inside a value**, not a new pair, so
/// `caption=Saved, undo?` survives as one caption rather than becoming a pair
/// named `undo?`. Keys lose everything but their letters and digits, which is
/// what lets `iconposition`, `icon-position` and `Icon_Position` all be the
/// same key.
fn button_spec_pairs(spec: &str) -> Vec<(String, String)> {
    let mut out: Vec<(String, String)> = Vec::new();
    for frag in spec.split(',') {
        match frag.split_once('=') {
            Some((k, v)) => {
                let key: String = k
                    .trim()
                    .to_ascii_lowercase()
                    .chars()
                    .filter(|c| c.is_ascii_alphanumeric())
                    .collect();
                out.push((key, v.to_owned()));
            }
            None => {
                if let Some((_, v)) = out.last_mut() {
                    v.push(',');
                    v.push_str(frag);
                }
            }
        }
    }
    for (_, v) in &mut out {
        *v = v.trim().to_owned();
    }
    out
}

/// Parse an `AddButton()` argument.
///
/// Recognised keys — `id`, `caption` (or `text`), `icon`, `position`,
/// `dismiss`, `iconposition`. Every one is optional except **`id`**: it is what
/// `onButtonClick` reports, and without it there would be nothing to report, so
/// a spec that names none declares no button at all and answers `None`.
///
/// Defaults match the `Buttons` line exactly, because they ARE the same
/// defaults: `dismiss` is true unless it says otherwise, and an omitted
/// `iconposition` is `Left` when an icon was given and `None` when it was not.
/// `|` and line breaks are stripped from every value — they are the property's
/// own separators, and a caption carrying one would corrupt the row rather than
/// print it.
pub fn parse_button_spec(spec: &str) -> Option<ButtonSpec> {
    let pairs = button_spec_pairs(spec);
    let get = |k: &str| pairs.iter().find(|(a, _)| a == k).map(|(_, v)| v.as_str());
    let clean = |s: &str| -> String {
        s.chars().filter(|c| !matches!(c, '|' | '\n' | '\r')).collect::<String>().trim().to_owned()
    };

    let id = clean(get("id").unwrap_or(""));
    if id.is_empty() {
        return None;
    }
    let text = clean(get("caption").or_else(|| get("text")).unwrap_or(""));
    let icon = clean(get("icon").unwrap_or(""));
    let icon_position = get("iconposition").unwrap_or("").trim().to_owned();
    let position = if icon_position.is_empty() {
        if icon.is_empty() {
            ButtonIconPosition::None
        } else {
            ButtonIconPosition::Left
        }
    } else {
        ButtonIconPosition::from_prop(&icon_position)
    };
    let dismiss = !matches!(
        get("dismiss").unwrap_or("").trim().to_ascii_lowercase().as_str(),
        "false" | "no" | "0"
    );
    // A zero or a non-number is "no opinion", not slot zero.
    let ordinal = get("position")
        .and_then(|v| v.trim().parse::<usize>().ok())
        .filter(|n| *n >= 1);

    Some(ButtonSpec {
        button: SnackButton { id, text, icon, position, dismiss },
        position: ordinal,
    })
}

/// Render one button back to its `Buttons` line.
pub fn button_line(b: &SnackButton) -> String {
    format!(
        "{}|{}|{}|{}|{}",
        b.id,
        b.text,
        b.icon,
        b.position.as_str(),
        if b.dismiss { "true" } else { "false" }
    )
}

/// What `Buttons` becomes once `spec` is added to it — `None` when the spec
/// declared no `id` and so declared no button.
///
/// Lines already present are carried across **verbatim**: a value the designer
/// wrote is not round-tripped through the parser and re-rendered, so adding a
/// button at run time cannot quietly rewrite the ones beside it.
///
/// `position` is an insertion point, not a fixed slot — asking for 1 twice puts
/// the newer one first and pushes the other along, which is the only reading
/// under which two calls cannot both claim the same place.
pub fn add_button(existing: &str, spec: &str) -> Option<String> {
    let parsed = parse_button_spec(spec)?;
    let mut lines: Vec<String> = existing
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .map(str::to_owned)
        .collect();
    let line = button_line(&parsed.button);
    match parsed.position {
        Some(n) => {
            let at = (n - 1).min(lines.len());
            lines.insert(at, line);
        }
        None => lines.push(line),
    }
    Some(lines.join("\n"))
}

// ── Content layout ───────────────────────────────────────────────────────────

/// Where each part of a notification's content sits, in **surface** coordinates.
///
/// R19's order — `Margin → [icon] → gap → text → flexible space → buttons →
/// Margin` — with the text taking whatever is left between icon and buttons.
/// Every band is vertically centred in the notification (R18), which is contract
/// rather than a property: `ContentVerticalAlignment` was dropped for exactly
/// that reason (§8).
#[derive(Debug, Clone, PartialEq)]
pub struct ContentLayout {
    /// The category icon's square, when there is one.
    pub icon: Option<Rect>,
    /// The text band. Never overlaps the icon or the buttons.
    pub text: Rect,
    /// One rect per button, left to right, in `Buttons` order.
    pub buttons: Vec<Rect>,
    /// How many lines the text will actually use — capped at the class's budget.
    pub lines_used: usize,
    /// True when the text needed more lines than the class allows and will be
    /// ellipsized (R22).
    pub ellipsized: bool,
}

/// Lay a notification's content out inside `rect`.
///
/// `text_width_of` measures a string in the notification's own font — the caller
/// supplies it so this stays free of egui and testable without a render context.
/// `button_widths` are already-measured button widths, in `Buttons` order.
pub fn layout_content(
    rect: Rect,
    size: SnackSize,
    icon_side: Option<f32>,
    text: &str,
    button_widths: &[f32],
    text_width_of: &dyn Fn(&str) -> f32,
    wrap: bool,
) -> ContentLayout {
    let m = size.metrics();
    let left = rect.x as f32 + m.pad_x;
    let right = rect.x as f32 + rect.w as f32 - m.pad_x;
    let cy = rect.y as f32 + rect.h as f32 / 2.0;

    // Icon band, hard against the left margin.
    let (icon, text_left) = match icon_side {
        Some(side) if side > 0.0 => {
            let r = Rect::new(
                left.round() as i32,
                (cy - side / 2.0).round() as i32,
                side.round() as i32,
                side.round() as i32,
            );
            (Some(r), left + side + m.gap)
        }
        _ => (None, left),
    };

    // Buttons, right to left from the right margin — the "flexible space" of R19
    // is whatever the text does not need, so the buttons never move as the text
    // changes length.
    let total_buttons: f32 = button_widths.iter().sum::<f32>()
        + m.gap * button_widths.len().saturating_sub(1) as f32;
    let buttons_left = if button_widths.is_empty() {
        right
    } else {
        right - total_buttons
    };
    let mut buttons = Vec::with_capacity(button_widths.len());
    let mut bx = buttons_left;
    for w in button_widths {
        buttons.push(Rect::new(
            bx.round() as i32,
            (cy - m.button_h / 2.0).round() as i32,
            w.round() as i32,
            m.button_h.round() as i32,
        ));
        bx += w + m.gap;
    }

    // The text takes the space between. `gap` before the buttons keeps the two
    // apart; with no buttons it runs to the right margin.
    let text_right = if button_widths.is_empty() {
        right
    } else {
        buttons_left - m.gap
    };
    let text_w = (text_right - text_left).max(0.0);

    // How many lines it needs, and whether the class's budget cuts it (R22).
    let needed = if !wrap {
        1
    } else if text_w <= 0.5 {
        // No room to wrap into — one line, and the ellipsis does the rest.
        1
    } else {
        let measured = text_width_of(text);
        ((measured / text_w).ceil() as usize).max(1)
    };
    let lines_used = needed.min(m.line_budget);
    let ellipsized = needed > m.line_budget;

    let text_h = m.line_h * lines_used as f32;
    let text_rect = Rect::new(
        text_left.round() as i32,
        (cy - text_h / 2.0).round() as i32,
        text_w.round() as i32,
        text_h.round() as i32,
    );

    ContentLayout { icon, text: text_rect, buttons, lines_used, ellipsized }
}

/// The size one notification wants, before the stack places it.
///
/// Width is the content's natural width clamped to the class's `min_width` …
/// `max_width`; height is derived from the lines it will use, floored at the
/// class's `min_height`. A notification never asks its surface to grow — R26 is
/// a property of the arithmetic, not a check bolted on afterwards.
pub fn notification_size(
    size: SnackSize,
    icon_side: Option<f32>,
    text: &str,
    button_widths: &[f32],
    text_width_of: &dyn Fn(&str) -> f32,
    wrap: bool,
    surface: Rect,
) -> (f32, f32) {
    let m = size.metrics();
    let icon_w = icon_side.map(|s| s + m.gap).unwrap_or(0.0);
    let buttons_w = if button_widths.is_empty() {
        0.0
    } else {
        button_widths.iter().sum::<f32>()
            + m.gap * button_widths.len().saturating_sub(1) as f32
            + m.gap
    };
    let natural = m.pad_x * 2.0 + icon_w + text_width_of(text) + buttons_w;

    // The surface is a ceiling as well as the class: a notification wider than
    // the pane it is anchored in would be clipped, and the developer would see
    // a message with its end cut off rather than an ellipsis.
    let ceiling = m.max_width.min((surface.w as f32 - m.pad_x * 2.0).max(m.min_width));
    let width = natural.clamp(m.min_width, ceiling.max(m.min_width));

    // Measure the lines against the width we just settled on.
    let text_w = (width - m.pad_x * 2.0 - icon_w - buttons_w).max(0.0);
    let needed = if !wrap || text_w <= 0.5 {
        1
    } else {
        ((text_width_of(text) / text_w).ceil() as usize).max(1)
    };
    let lines_used = needed.min(m.line_budget);
    let height = (m.pad_y * 2.0 + m.line_h * lines_used as f32).max(m.min_height);
    (width, height)
}

// ── What `Show()` mints ──────────────────────────────────────────────────────

/// A notification's polar drop shadow, already resolved.
#[derive(Debug, Clone, PartialEq)]
pub struct SnackShadow {
    pub color: String,
    /// Percent, 0–100.
    pub opacity: i64,
    pub blur: f32,
    /// Degrees, 0–359. The catalogue's shadows are polar, not cartesian (§8).
    pub direction: f32,
    pub distance: f32,
}

/// One notification's **resolved** appearance and content.
///
/// This is what a `Show()` mints from the template (D2): every category default
/// already applied, every override already won, `Buttons` already parsed. A
/// notification raised from a template does not keep looking at that template —
/// changing `Text` after `Show()` must not rewrite a message already on screen,
/// and taking a snapshot here is what makes that true.
///
/// It is also what the painter is handed, so the painter needs to know nothing
/// about categories, defaults or property names.
#[derive(Debug, Clone, PartialEq)]
pub struct SnackVisual {
    pub text: String,
    pub category: SnackCategory,
    pub size: SnackSize,
    /// Resolved `#RRGGBB[AA]` — never empty.
    pub background: String,
    /// Resolved `#RRGGBB[AA]` — never empty.
    pub foreground: String,
    /// Resolved catalogue icon name; `None` when `ShowCategoryIcon` is off.
    pub icon: Option<String>,
    pub icon_size: f32,
    /// Empty = paint the icon in `foreground`.
    pub icon_color: String,
    pub background_image: String,
    pub background_image_mode: crate::model::BgImageMode,
    /// Percent, 0–100.
    pub background_image_opacity: i64,
    pub font_name: String,
    pub font_size: f32,
    pub bold: bool,
    pub text_wrap: bool,
    /// Top-left, top-right, bottom-right, bottom-left — each already resolved
    /// against `CornerRadius` when its own value was the `-1` sentinel.
    pub corner_radius: [f32; 4],
    pub border_style: String,
    pub border_width: f32,
    pub border_color: String,
    /// `None` when `ShadowEnabled` is false.
    pub shadow: Option<SnackShadow>,
    pub buttons: Vec<SnackButton>,
    /// Already resolved through [`effective_timeout_ms`]; `0` = never (R6).
    pub timeout_ms: i64,
    pub pause_on_hover: bool,
    pub anchor: SnackAnchor,
    pub margin: f32,
    pub stack_spacing: f32,
    pub stack_order: StackOrder,
    pub maximum_visible: usize,
    pub overflow: OverflowBehavior,
}

/// Mint a notification from the template's **current** property values (D2).
///
/// Returns the resolved notification and, when the template declared more than
/// [`MAX_BUTTONS`], the diagnostic naming what will not be shown (spec Q5).
///
/// Every read defaults (plan §5): a template carrying a partial property set —
/// an old form copied from another project — yields a fully-formed notification
/// rather than a panic or a silently wrong value.
pub fn mint(ctrl: &Control) -> (SnackVisual, Option<ButtonsDiagnostic>) {
    let category = category_of(ctrl);
    let size = size_of(ctrl);
    let (buttons, diag) = parse_buttons(&prop_str(ctrl, "Buttons"));

    // Per-corner radius, each falling back to `CornerRadius` on the `-1`
    // sentinel — the same rule `CornerRadiusTopLeft` already documents.
    let base_radius = prop_i64(ctrl, "CornerRadius", 12).max(0) as f32;
    let corner = |name: &str| {
        let v = prop_i64(ctrl, name, -1);
        if v < 0 {
            base_radius
        } else {
            v as f32
        }
    };

    let shadow = if prop_bool(ctrl, "ShadowEnabled", true) {
        Some(SnackShadow {
            color: {
                let c = prop_str(ctrl, "ShadowColor");
                if c.trim().is_empty() { "#000000".to_owned() } else { c }
            },
            opacity: prop_i64(ctrl, "ShadowOpacity", 25).clamp(0, 100),
            blur: prop_i64(ctrl, "ShadowBlur", 12).max(0) as f32,
            direction: prop_i64(ctrl, "ShadowDirection", 270).rem_euclid(360) as f32,
            distance: prop_i64(ctrl, "ShadowDistance", 4).max(0) as f32,
        })
    } else {
        None
    };

    let visual = SnackVisual {
        text: prop_str(ctrl, "Text"),
        category,
        size,
        background: effective_background(ctrl),
        foreground: effective_foreground(ctrl),
        icon: effective_icon(ctrl),
        icon_size: effective_icon_size(ctrl),
        icon_color: prop_str(ctrl, "CategoryIconColor"),
        background_image: prop_str(ctrl, "BackgroundImage"),
        background_image_mode: crate::model::BgImageMode::from_str(&{
            let m = prop_str(ctrl, "BackgroundImageMode");
            if m.trim().is_empty() { "Fill".to_owned() } else { m }
        }),
        background_image_opacity: prop_i64(ctrl, "BackgroundImageOpacity", 15).clamp(0, 100),
        font_name: prop_str(ctrl, "FontName"),
        font_size: prop_i64(ctrl, "FontSize", 14).max(1) as f32,
        bold: prop_bool(ctrl, "Bold", false),
        text_wrap: prop_bool(ctrl, "TextWrap", true),
        corner_radius: [
            corner("CornerRadiusTopLeft"),
            corner("CornerRadiusTopRight"),
            corner("CornerRadiusBottomRight"),
            corner("CornerRadiusBottomLeft"),
        ],
        border_style: {
            let s = prop_str(ctrl, "BorderStyle");
            if s.trim().is_empty() { "None".to_owned() } else { s }
        },
        border_width: prop_i64(ctrl, "BorderWidth", 1).max(0) as f32,
        border_color: {
            let c = prop_str(ctrl, "BorderColor");
            if c.trim().is_empty() { "#00000000".to_owned() } else { c }
        },
        shadow,
        buttons,
        timeout_ms: effective_timeout_ms(ctrl).max(0),
        pause_on_hover: prop_bool(ctrl, "PauseTimeoutOnHover", true),
        anchor: SnackAnchor::from_prop(&prop_str(ctrl, "StackAnchor")),
        // A negative margin or spacing would place a notification outside its
        // own surface, so the floor is 0 rather than the raw value.
        margin: prop_i64(ctrl, "Margin", 16).max(0) as f32,
        stack_spacing: prop_i64(ctrl, "StackSpacing", 8).max(0) as f32,
        stack_order: StackOrder::from_prop(&prop_str(ctrl, "StackOrder")),
        // At least one: `MaximumVisible = 0` would make `Show()` a no-op that
        // looks exactly like a broken handler.
        maximum_visible: prop_i64(ctrl, "MaximumVisible", 5).max(1) as usize,
        overflow: OverflowBehavior::from_prop(&prop_str(ctrl, "OverflowBehavior")),
    };
    (visual, diag)
}

// ── The stack ────────────────────────────────────────────────────────────────

/// Where the stack puts each live notification.
///
/// `sizes` are in **stack order, oldest first** — index 0 is the notification
/// that has been up longest. The returned rects are in the same order, so a
/// caller can zip them against its own list without re-sorting.
///
/// The rules (R11–R13):
///
/// * **Vertical only.** There is no horizontal stacking, by contract (§2).
/// * **Top anchors grow downward**, newest nearest the anchor — so the newest
///   is at the TOP and older ones are pushed down.
/// * **Bottom anchors grow upward**, newest nearest the anchor — the newest is
///   at the BOTTOM.
/// * **Centre-row anchors** place the newest first and grow downward (R13),
///   unless `StackOrder` overrides it.
///
/// `StackOrder::NewestFirst` / `NewestLast` override the anchor's own rule:
/// "first" means nearest the anchor, "last" means furthest from it.
pub fn stack_layout(
    anchor: SnackAnchor,
    margin: f32,
    spacing: f32,
    order: StackOrder,
    surface: Rect,
    sizes: &[(f32, f32)],
) -> Vec<Rect> {
    if sizes.is_empty() {
        return Vec::new();
    }

    // Which end of the run the NEWEST notification takes. `Auto` follows the
    // anchor: nearest it, which is what "newest nearest the anchor" means for
    // Top and Bottom alike, and newest-first for the Centre row (R13).
    let newest_nearest = match order {
        StackOrder::Auto => true,
        StackOrder::NewestFirst => true,
        StackOrder::NewestLast => false,
    };

    // `sizes` is oldest-first. Build the visual run — index 0 is the slot
    // nearest the anchor — by reversing when the newest belongs there.
    let mut run: Vec<usize> = (0..sizes.len()).collect();
    if newest_nearest {
        run.reverse();
    }

    let total_h: f32 =
        sizes.iter().map(|(_, h)| *h).sum::<f32>() + spacing * (sizes.len() - 1) as f32;

    // The run's own top edge, before any slot is placed.
    let sx = surface.x as f32;
    let sy = surface.y as f32;
    let sw = surface.w as f32;
    let sh = surface.h as f32;
    let run_top = if anchor.is_top() {
        sy + margin
    } else if anchor.is_bottom() {
        sy + sh - margin - total_h
    } else {
        // Centre row: the whole run is centred on the surface, and grows down
        // from there as notifications arrive.
        sy + (sh - total_h) / 2.0
    };

    // A Bottom anchor's run is laid out top-to-bottom like any other, but its
    // slot 0 (nearest the anchor) is the BOTTOM one — so the run is walked from
    // the far end.
    let mut placed: Vec<Option<Rect>> = vec![None; sizes.len()];
    let mut y = run_top;
    // Visual top-to-bottom order of the run's slots.
    let top_down: Vec<usize> = if anchor.is_bottom() {
        run.iter().rev().copied().collect()
    } else {
        run.clone()
    };
    for &idx in &top_down {
        let (w, h) = sizes[idx];
        let x = match anchor {
            SnackAnchor::TopLeft | SnackAnchor::CenterLeft | SnackAnchor::BottomLeft => sx + margin,
            SnackAnchor::TopRight | SnackAnchor::CenterRight | SnackAnchor::BottomRight => {
                sx + sw - margin - w
            }
            _ => sx + (sw - w) / 2.0,
        };
        placed[idx] = Some(Rect::new(
            x.round() as i32,
            y.round() as i32,
            w.round() as i32,
            h.round() as i32,
        ));
        y += h + spacing;
    }

    placed.into_iter().map(|r| r.unwrap_or(Rect::new(0, 0, 0, 0))).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::ControlType;

    /// A crude but *deterministic* text measurer: 7 points per character. The
    /// point of `text_width_of` being a parameter is that these tests never need
    /// a font, a render context or a window (plan §5 — nothing here may flake).
    fn measure(s: &str) -> f32 {
        s.chars().count() as f32 * 7.0
    }

    fn surface() -> Rect {
        Rect::new(0, 0, 1000, 700)
    }

    #[test]
    fn snackbar_size_classes_have_the_documented_dimensions() {
        eprintln!(
            "\n  size     min_w  max_w  min_h  pad_x  pad_y  icon  gap  line_h  lines  btn_h"
        );
        eprintln!("  ------   -----  -----  -----  -----  -----  ----  ---  ------  -----  -----");
        for s in SnackSize::ALL {
            let m = s.metrics();
            eprintln!(
                "  {:<7}  {:>5}  {:>5}  {:>5}  {:>5}  {:>5}  {:>4}  {:>3}  {:>6}  {:>5}  {:>5}",
                s.as_str(),
                m.min_width,
                m.max_width,
                m.min_height,
                m.pad_x,
                m.pad_y,
                m.icon,
                m.gap,
                m.line_h,
                m.line_budget,
                m.button_h
            );
        }
        // The line budget is the contract R22 names, one line per class.
        assert_eq!(SnackSize::Small.metrics().line_budget, 1);
        assert_eq!(SnackSize::Medium.metrics().line_budget, 2);
        assert_eq!(SnackSize::Large.metrics().line_budget, 3);
        // Each class is strictly roomier than the one below it — a "Large" that
        // is not larger would make the property meaningless.
        for w in SnackSize::ALL.windows(2) {
            let (a, b) = (w[0].metrics(), w[1].metrics());
            assert!(b.min_width > a.min_width, "min_width must grow with the class");
            assert!(b.min_height > a.min_height, "min_height must grow with the class");
            assert!(b.line_budget > a.line_budget, "line budget must grow with the class");
        }
        eprintln!("  → 3 size classes verified: budgets 1/2/3, each class strictly roomier\n");
    }

    #[test]
    fn content_is_vertically_centred_in_every_size_class() {
        let mut checked = 0usize;
        eprintln!("\n  class    rect h   icon cy   text cy   button cy   (centre)");
        eprintln!("  ------   ------   -------   -------   ---------   --------");
        for s in SnackSize::ALL {
            let m = s.metrics();
            let h = m.min_height.round() as i32;
            let rect = Rect::new(40, 100, 400, h);
            let l = layout_content(
                rect,
                s,
                Some(m.icon),
                "Saved",
                &[70.0, 60.0],
                &measure,
                true,
            );
            let centre = rect.y as f32 + rect.h as f32 / 2.0;
            let icon = l.icon.expect("icon requested");
            let icon_cy = icon.y as f32 + icon.h as f32 / 2.0;
            let text_cy = l.text.y as f32 + l.text.h as f32 / 2.0;
            let btn_cy = l.buttons[0].y as f32 + l.buttons[0].h as f32 / 2.0;
            eprintln!(
                "  {:<7}  {:>6}   {:>7.1}   {:>7.1}   {:>9.1}   {:>8.1}",
                s.as_str(), h, icon_cy, text_cy, btn_cy, centre
            );
            // Rounding to whole points can move a centre by half a point; that
            // is the only tolerance allowed.
            for (what, cy) in [("icon", icon_cy), ("text", text_cy), ("button", btn_cy)] {
                assert!(
                    (cy - centre).abs() <= 0.5,
                    "{what} is not vertically centred in {}: {cy} vs {centre}",
                    s.as_str()
                );
            }
            checked += 3;
        }
        eprintln!("  → AC8: {checked} content bands centred across 3 size classes\n");
    }

    #[test]
    fn the_content_runs_icon_then_text_then_buttons_and_never_overlaps() {
        let m = SnackSize::Medium.metrics();
        let rect = Rect::new(0, 0, 520, m.min_height.round() as i32);
        let l = layout_content(rect, SnackSize::Medium, Some(m.icon), "Record saved", &[80.0, 64.0], &measure, true);
        let icon = l.icon.unwrap();
        assert!(icon.x >= rect.x, "icon starts inside the left margin");
        assert!(
            l.text.x >= icon.x + icon.w,
            "R19: the text starts after the icon ({} vs {})",
            l.text.x,
            icon.x + icon.w
        );
        assert!(
            l.text.x + l.text.w <= l.buttons[0].x,
            "R19: the text ends before the first button ({} vs {})",
            l.text.x + l.text.w,
            l.buttons[0].x
        );
        assert!(
            l.buttons[0].x + l.buttons[0].w <= l.buttons[1].x,
            "buttons run left to right without overlapping"
        );
        let right_margin = rect.x + rect.w - (l.buttons[1].x + l.buttons[1].w);
        assert!(
            (right_margin as f32 - m.pad_x).abs() <= 1.0,
            "the last button sits on the right margin (got {right_margin}, want {})",
            m.pad_x
        );
        eprintln!(
            "\n  R19 order — icon [{}..{}] · text [{}..{}] · buttons [{}..{}] [{}..{}] · right margin {}\n",
            icon.x, icon.x + icon.w,
            l.text.x, l.text.x + l.text.w,
            l.buttons[0].x, l.buttons[0].x + l.buttons[0].w,
            l.buttons[1].x, l.buttons[1].x + l.buttons[1].w,
            right_margin
        );
    }

    #[test]
    fn text_beyond_the_line_budget_is_ellipsized() {
        // One string, measured against all three budgets. 60 chars * 7 = 420 pt
        // of text in a band ~120 pt wide is 4 lines' worth — over every budget.
        let long = "x".repeat(60);
        eprintln!("\n  class    band w   lines needed   budget   used   ellipsized");
        eprintln!("  ------   ------   ------------   ------   ----   ----------");
        let mut any = false;
        for s in SnackSize::ALL {
            let m = s.metrics();
            let rect = Rect::new(0, 0, 260, m.min_height.round() as i32);
            let l = layout_content(rect, s, Some(m.icon), &long, &[70.0], &measure, true);
            let needed = (measure(&long) / l.text.w as f32).ceil() as usize;
            eprintln!(
                "  {:<7}  {:>6}   {:>12}   {:>6}   {:>4}   {}",
                s.as_str(), l.text.w, needed, m.line_budget, l.lines_used, l.ellipsized
            );
            assert!(l.ellipsized, "R22: {} must ellipsize 60 chars", s.as_str());
            assert_eq!(l.lines_used, m.line_budget, "capped at the class budget");
            any = true;
        }
        assert!(any);
        // And the opposite: something that fits is NOT ellipsized.
        let l = layout_content(
            Rect::new(0, 0, 520, 56), SnackSize::Medium, Some(22.0), "OK", &[], &measure, true,
        );
        assert!(!l.ellipsized, "a string that fits is never ellipsized");
        assert_eq!(l.lines_used, 1);
        eprintln!("  → AC14: over-long text ellipsized in all 3 classes; a fitting string is not\n");
    }

    #[test]
    fn buttons_parse_one_per_line_with_trailing_fields_omitted() {
        let (bs2, diag2) =
            parse_buttons("retry|Retry|refresh|Left|true\nclose||x-mark\nmore|More\n");
        assert!(diag2.is_none(), "three buttons produce no diagnostic");
        assert_eq!(bs2.len(), 3);
        assert_eq!(bs2[0], SnackButton {
            id: "retry".into(), text: "Retry".into(), icon: "refresh".into(),
            position: ButtonIconPosition::Left, dismiss: true,
        });
        // Trailing fields omitted: an icon with no position defaults to Left,
        // and dismiss defaults to true.
        assert_eq!(bs2[1], SnackButton {
            id: "close".into(), text: String::new(), icon: "x-mark".into(),
            position: ButtonIconPosition::Left, dismiss: true,
        });
        // No icon at all: the position is None, not a reserved-and-empty Left.
        assert_eq!(bs2[2], SnackButton {
            id: "more".into(), text: "More".into(), icon: String::new(),
            position: ButtonIconPosition::None, dismiss: true,
        });
        eprintln!(
            "\n  Buttons parse — 3 lines → 3 buttons; omitted position→{:?}/{:?}, omitted dismiss→true\n",
            bs2[1].position, bs2[2].position
        );
    }

    #[test]
    fn dismiss_false_is_honoured_and_blank_lines_are_skipped() {
        let (bs, diag) = parse_buttons("\n\n  \nkeep|Keep|check|Right|false\n\n");
        assert!(diag.is_none());
        assert_eq!(bs.len(), 1, "three blank lines contribute nothing");
        assert!(!bs[0].dismiss, "dismiss=false leaves the notification up (R8)");
        assert_eq!(bs[0].position, ButtonIconPosition::Right);
        // A line with no id has nothing for onButtonClick to report — skipped.
        let (none, _) = parse_buttons("|No id|check\n");
        assert!(none.is_empty(), "a button with no id is not a button");
        eprintln!("\n  Buttons edge cases — blanks skipped, dismiss=false honoured, id-less line skipped\n");
    }

    #[test]
    fn a_fourth_button_is_reported_not_silently_dropped() {
        let (bs, diag) = parse_buttons("a|A\nb|B\nc|C\nd|D\ne|E\n");
        assert_eq!(bs.len(), MAX_BUTTONS, "the first three render");
        let diag = diag.expect("spec Q5: a fourth button must produce a diagnostic");
        match diag {
            ButtonsDiagnostic::TooMany { declared, dropped } => {
                assert_eq!(declared, 5);
                // Naming what was left out is the whole difference between a
                // warning and a silent truncation (GOLDEN RULE #7's principle).
                assert_eq!(dropped, vec!["d".to_string(), "e".to_string()]);
                eprintln!(
                    "\n  Q5 diagnostic — {declared} declared, {} shown, dropped {dropped:?}\n",
                    bs.len()
                );
            }
        }
    }

    #[test]
    fn every_category_supplies_its_documented_timeout_and_artwork() {
        eprintln!("\n  category    timeout   background   foreground   icon");
        eprintln!("  ---------   -------   ----------   ----------   ----------------");
        for c in SnackCategory::ALL {
            let d = c.defaults();
            eprintln!(
                "  {:<9}   {:>7}   {:<10}   {:<10}   {}",
                c.as_str(), d.timeout_ms, d.background, d.foreground, d.icon
            );
            assert!(!d.background.is_empty() && !d.foreground.is_empty());
            assert!(!d.icon.is_empty());
        }
        // §7's table, verbatim.
        assert_eq!(SnackCategory::Info.defaults().timeout_ms, 4000);
        assert_eq!(SnackCategory::Question.defaults().timeout_ms, 6000);
        assert_eq!(SnackCategory::Warning.defaults().timeout_ms, 6000);
        assert_eq!(SnackCategory::Error.defaults().timeout_ms, 8000);
        assert_eq!(SnackCategory::Critical.defaults().timeout_ms, 0, "Critical stays");
        eprintln!("  → 5 categories verified against spec §7\n");
    }

    #[test]
    fn the_seeded_template_resolves_every_default_from_its_category() {
        let c = crate::model::Control::new("SNACK-1", ControlType::Snackbar, 0, 0);
        // A freshly dropped Snackbar is Info, and reads Info's whole kit.
        assert_eq!(category_of(&c), SnackCategory::Info);
        assert_eq!(effective_timeout_ms(&c), 4000, "-1 seed means 'ask the category'");
        assert_eq!(effective_background(&c), "#1E4E8C");
        assert_eq!(effective_foreground(&c), "#F2F7FF");
        assert_eq!(effective_icon(&c).as_deref(), Some("info-circle"));
        assert_eq!(effective_icon_size(&c), SnackSize::Medium.metrics().icon);
        eprintln!("\n  seeded template → category Info, timeout 4000ms, bg #1E4E8C, icon info-circle");

        // R23 — an explicitly set property overrides its category default, and
        // ONLY that one: the rest still come from the category.
        let mut c2 = c.clone();
        c2.set_prop("BackgroundColor", crate::model::PropValue::String("#123456".into()));
        c2.set_prop("Timeout", crate::model::PropValue::Int(1500));
        assert_eq!(effective_background(&c2), "#123456", "explicit background wins");
        assert_eq!(effective_timeout_ms(&c2), 1500, "explicit timeout wins");
        assert_eq!(effective_foreground(&c2), "#F2F7FF", "foreground still the category's");
        assert_eq!(effective_icon(&c2).as_deref(), Some("info-circle"), "icon still the category's");

        // Changing the category moves everything that was never set.
        let mut c3 = c.clone();
        c3.set_prop("Category", crate::model::PropValue::String("Critical".into()));
        assert_eq!(effective_timeout_ms(&c3), 0, "Critical stays up (R6)");
        assert_eq!(effective_background(&c3), "#5A0F0F");
        assert_eq!(effective_icon(&c3).as_deref(), Some("critical-octagon"));

        // Timeout 0 set EXPLICITLY still means "never" (R6) — the sentinel is
        // -1, so 0 keeps its documented meaning.
        let mut c4 = c.clone();
        c4.set_prop("Timeout", crate::model::PropValue::Int(0));
        assert_eq!(effective_timeout_ms(&c4), 0, "R6: an explicit 0 means never");

        // ShowCategoryIcon off means no icon at all.
        let mut c5 = c.clone();
        c5.set_prop("ShowCategoryIcon", crate::model::PropValue::Bool(false));
        assert!(effective_icon(&c5).is_none());
        eprintln!("  → AC10: 4 overrides verified, each winning alone; R6's explicit 0 preserved\n");
    }

    #[test]
    fn a_missing_property_means_the_default_never_a_panic() {
        // The 1.62.139 DataGrid hunt in miniature: a form copied from another
        // project carries a Snackbar with almost nothing on it.
        let mut bare = crate::model::Control::new("SNACK-BARE", ControlType::Snackbar, 0, 0);
        bare.properties.clear();
        assert_eq!(category_of(&bare), SnackCategory::Info);
        assert_eq!(size_of(&bare), SnackSize::Medium);
        assert_eq!(effective_timeout_ms(&bare), 4000);
        assert_eq!(effective_background(&bare), "#1E4E8C");
        assert!(effective_icon(&bare).is_some());
        assert_eq!(effective_icon_size(&bare), 22.0);
        // And a misspelled enum falls back rather than failing.
        bare.set_prop("Category", crate::model::PropValue::String("Kritical".into()));
        assert_eq!(category_of(&bare), SnackCategory::Info, "an unknown category is Info");
        bare.set_prop("StackAnchor", crate::model::PropValue::String("nowhere".into()));
        assert_eq!(
            SnackAnchor::from_prop(&bare.get_prop("StackAnchor").unwrap().as_str().to_owned()),
            SnackAnchor::BottomRight
        );
        eprintln!("\n  bare/misspelled template — 8 reads defaulted, 0 panics\n");
    }

    #[test]
    fn add_button_declares_one_button_per_call() {
        // The operator's own shape (2026-09-02): Clear() then one AddButton per
        // button, each a `key=value` list.
        let mut spec = String::new();
        for one in [
            "id=button1,icon=undo,caption=Undo,position=1,dismiss=true",
            "id=button2,icon=clock,caption=Later,position=2,dismiss=false",
        ] {
            spec = add_button(&spec, one).expect("declared");
        }
        let (buttons, diag) = parse_buttons(&spec);
        assert!(diag.is_none());

        eprintln!("\n  #   id        caption   icon    icon pos   dismiss");
        eprintln!("  -   -------   -------   -----   --------   -------");
        for (i, b) in buttons.iter().enumerate() {
            eprintln!(
                "  {i}   {:<7}   {:<7}   {:<5}   {:<8}   {}",
                b.id, b.text, b.icon, b.position.as_str(), b.dismiss
            );
        }
        assert_eq!(buttons.len(), 2, "two calls, two buttons: {spec:?}");
        assert_eq!(buttons[0].id, "button1");
        assert_eq!(buttons[0].text, "Undo");
        assert_eq!(buttons[0].icon, "undo");
        // An icon with no `iconposition` lands Left, the same rule the property
        // line already follows.
        assert_eq!(buttons[0].position, ButtonIconPosition::Left);
        assert!(buttons[0].dismiss);
        assert_eq!(buttons[1].id, "button2");
        assert!(!buttons[1].dismiss, "dismiss=false leaves it up");
        eprintln!("  → 2 calls → 2 buttons, in the declared order\n");
    }

    #[test]
    fn a_button_spec_defaults_everything_but_the_id() {
        // `id` is the one thing that cannot be defaulted — it is what
        // onButtonClick reports.
        assert!(parse_button_spec("caption=Undo,icon=undo").is_none(), "no id, no button");
        assert!(parse_button_spec("").is_none());
        assert!(parse_button_spec("id=   ").is_none(), "a blank id is no id");

        eprintln!("\n  spec                          id      caption  icon   icon pos  dismiss");
        eprintln!("  ---------------------------   -----   -------  ----   --------  -------");
        for (spec, want_text, want_icon, want_pos, want_dismiss) in [
            ("id=ok", "", "", ButtonIconPosition::None, true),
            ("id=ok,caption=OK", "OK", "", ButtonIconPosition::None, true),
            ("id=ok,text=OK", "OK", "", ButtonIconPosition::None, true),
            ("id=ok,icon=check", "", "check", ButtonIconPosition::Left, true),
            ("id=ok,icon=check,iconposition=Right", "", "check", ButtonIconPosition::Right, true),
            ("id=ok,icon-position=None,icon=check", "", "check", ButtonIconPosition::None, true),
            ("id=ok,dismiss=false", "", "", ButtonIconPosition::None, false),
            ("ID=ok, Dismiss = NO", "", "", ButtonIconPosition::None, false),
            ("id=ok,dismiss=maybe", "", "", ButtonIconPosition::None, true),
        ] {
            let b = parse_button_spec(spec).expect("has an id").button;
            eprintln!(
                "  {spec:<27}   {:<5}   {:<7}  {:<5}  {:<8}  {}",
                b.id,
                if b.text.is_empty() { "-" } else { &b.text },
                if b.icon.is_empty() { "-" } else { &b.icon },
                b.position.as_str(),
                b.dismiss
            );
            assert_eq!(b.id, "ok", "{spec}");
            assert_eq!(b.text, want_text, "{spec}");
            assert_eq!(b.icon, want_icon, "{spec}");
            assert_eq!(b.position, want_pos, "{spec}");
            assert_eq!(b.dismiss, want_dismiss, "{spec}: an unrecognised value must NOT stick");
        }
        eprintln!("  → 9 specs, every field defaulted from the id alone\n");
    }

    #[test]
    fn a_caption_may_contain_a_comma_and_never_a_pipe() {
        // A comma inside a value is not a new pair — otherwise half the
        // captions a developer would write would silently become junk keys.
        let b = parse_button_spec("id=undo,caption=Saved, undo?,dismiss=false")
            .expect("declared")
            .button;
        assert_eq!(b.text, "Saved, undo?", "the comma stayed in the caption");
        assert!(!b.dismiss, "and the pair AFTER it was still read");

        // `|` is the line's own separator: stripped, never allowed to corrupt
        // the row it is written into.
        let b = parse_button_spec("id=a|b,caption=x|y").expect("declared").button;
        assert_eq!((b.id.as_str(), b.text.as_str()), ("ab", "xy"));
        let (parsed, _) = parse_buttons(&add_button("", "id=a|b,caption=x|y").unwrap());
        assert_eq!(parsed.len(), 1, "one button, not two fields' worth of wreckage");
        eprintln!("\n  → a comma survives inside a caption; a pipe is stripped, not honoured\n");
    }

    #[test]
    fn position_is_an_ordinal_left_to_right() {
        // Operator: "Position is ordinal (left to right)".
        let mut spec = String::new();
        for one in ["id=c,position=1", "id=a,position=1", "id=b,position=2"] {
            spec = add_button(&spec, one).unwrap();
        }
        let ids: Vec<String> = parse_buttons(&spec).0.into_iter().map(|b| b.id).collect();
        eprintln!("\n  added c@1, a@1, b@2 → row reads {ids:?}");
        assert_eq!(ids, vec!["a", "b", "c"], "each position is an insertion point");

        // No position at all = the end, in call order; and out-of-range clamps
        // rather than being dropped.
        let mut spec = String::new();
        for one in ["id=first", "id=second", "id=last,position=99"] {
            spec = add_button(&spec, one).unwrap();
        }
        let ids: Vec<String> = parse_buttons(&spec).0.into_iter().map(|b| b.id).collect();
        assert_eq!(ids, vec!["first", "second", "last"]);
        eprintln!("  → no position = call order; position 99 clamps to the end, never dropped\n");
    }

    #[test]
    fn add_button_leaves_the_lines_beside_it_alone() {
        // A designer-written row is carried across verbatim — adding one button
        // at run time must not re-render the others.
        let designed = "retry|Retry|refresh|Left|true\nlater|Later||None|false";
        let next = add_button(designed, "id=undo,caption=Undo").expect("declared");
        assert!(next.starts_with(designed), "the designed lines are untouched: {next:?}");
        assert_eq!(parse_buttons(&next).0.len(), 3);
        eprintln!("\n  → 2 designed lines carried verbatim, 1 appended\n");
    }
}
