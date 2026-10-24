// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! **What a DOM event asks of the host** (WASM-004 R3, R9–R10; WASM-016 R6–R7).
//!
//! The browser layer reduces an event to a [`Payload`] — plain data: which node, what value, which
//! key, where — and [`on_event`] answers a list of [`Act`]s: store this property, raise this model
//! event, change this bit of page state. Nothing here knows a DOM type, so the behaviour of every
//! control (what a click on a list row selects, what Space does on a check box, which events a drag of
//! a knob raises and in which order) is checked by native tests.
//!
//! Two kinds of effect, kept apart because the desktop keeps them apart:
//!
//! * a **quiet store** ([`Act::Quiet`]) writes the property the user's action changed *without*
//!   raising its observer events — the user's act raises the model events itself, in the order the
//!   desktop raises them;
//! * a **change** ([`Act::Change`]) is a user's edit whose observer events are the right events to
//!   raise (typing in a text box raises `onChange` and `onTextChanged`, once, only if the text differs).
//!
//! Events are raised only for handlers that are bound: the host drops the rest.

use cobolt_forms::menu::{open_form_target, open_standalone_target, MenuItem};
use cobolt_forms::model::{ControlType, CHECKED_PROP, SELECTED_PROP};
use cobolt_forms::treenodes::{node_at, nodes};

use super::controls::{display_items, lines, lines_nonempty};
use super::data::{node_payload, visible_nodes};
use super::shell::{menu_items, toolbar_def};
use super::ViewCtx;
use crate::tree::{Ctl, FormTree, Kind};
use crate::value::{fmt_number, parse_f64, truthy};

/// A DOM event, reduced to what the behaviour needs.
#[derive(Debug, Clone, Default)]
pub struct Payload {
    /// The DOM event type: `click`, `input`, `keydown`, `pointerdown`, …
    pub kind: String,
    /// The `key` of the node the listener sits on (`root`, `input`, `tablist`, `handle`, …).
    pub key: String,
    /// The value of an input, select or textarea.
    pub value: String,
    pub checked: bool,
    /// `data-i` of the nearest element at or above the target, inside the control.
    pub index: Option<i64>,
    /// `data-r`, `data-c` (a DataGrid cell).
    pub row: Option<i64>,
    pub col: Option<i64>,
    /// `data-id` (a menu item, a tool bar button, a marker).
    pub id: Option<String>,
    /// `data-part` (`arrow`, `check`, `label`, `toggle`, `zoom-in`, …).
    pub part: Option<String>,
    /// `KeyboardEvent.key`.
    pub key_name: String,
    /// 0 primary, 1 middle, 2 secondary.
    pub button: i16,
    /// The buttons held (bit 0: primary), for a move event.
    pub buttons: u16,
    pub ctrl: bool,
    pub shift: bool,
    pub alt: bool,
    pub meta: bool,
    /// The pointer inside the control's box, in CSS pixels.
    pub x: f64,
    pub y: f64,
    /// The control's box.
    pub w: f64,
    pub h: f64,
    pub delta: f64,
    pub scroll_top: f64,
    pub scroll_left: f64,
    pub client_height: f64,
    pub client_width: f64,
    /// Files of a drop or a picker: `(name, bytes)`.
    pub files: Vec<(String, u64)>,
    /// The named fields of a form at the moment of its `submit`: `(name, value)`.
    pub fields: Fields,
}

/// The fields of a submitted form. Their values may be passwords: a `Debug` output shows the names only.
#[derive(Clone, Default)]
pub struct Fields(pub Vec<(String, String)>);

impl std::fmt::Debug for Fields {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_list().entries(self.0.iter().map(|(k, _)| k)).finish()
    }
}

impl Payload {
    pub fn new(kind: &str, key: &str) -> Payload {
        Payload { kind: kind.to_owned(), key: key.to_owned(), ..Payload::default() }
    }
}

/// Page state an act may change (never visible to the program).
#[derive(Debug, Clone, PartialEq)]
pub enum UiOp {
    /// Open this chain of menu items (a MenuBar dropdown) or set the expanded set (a SideMenu).
    SetOpenMenu(Vec<String>),
    /// A DataGrid or a ListBox was scrolled to this offset.
    GridScroll { left: f64, top: f64 },
    GridSort { col: usize, descending: bool },
    Sidebar(bool),
    DropOver(bool),
    /// A pointer drag on a Maps control began at this pixel, with this centre.
    MapDrag { x: f64, y: f64, lat: f64, lng: f64 },
    MapDragEnd,
    /// Move the keyboard focus to the control.
    Focus,
    /// Dismiss a notification.
    Dismiss(u32),
    /// Open the browser's file picker of a FileDropZone.
    PickFiles,
    /// The user is in (`true`) or has left (`false`) a TextBox that has a numeric-edited PICTURE: it shows
    /// the plain value to edit while focused and the edited form at rest.
    Editing(bool),
}

/// One thing to do.
#[derive(Debug, Clone, PartialEq)]
pub enum Act {
    /// Raise a model event on the control (the host raises it only if a handler is bound).
    Event { name: &'static str, args: Vec<String> },
    /// Store a property the user changed, raising no observer event.
    Quiet { prop: String, value: String },
    /// Store a property the user edited and raise its observer events if the value differs.
    Change { prop: String, value: String },
    Ui(UiOp),
    /// `preventDefault()` on the DOM event.
    Prevent,
    /// A menu item was chosen: resolve its action (load a form, open a window).
    Menu { item: String },
    /// A toolbar button asked for a modal form.
    OpenModal { form: String },
    /// Show the user this text (a refused address, a button the browser cannot honour).
    Notice(String),
    /// Open a window with features (`window.open`).
    OpenWindow { url: String, target: String, features: String, rel: String },
    /// The files a drop accepted, for the player to hold.
    Files { accepted: Vec<(String, u64)> },
    /// An Authentication control's user acted (WASM-009): the host's machine takes it.
    Auth(crate::auth::Action),
}

fn ev(name: &'static str) -> Act {
    Act::Event { name, args: Vec::new() }
}

fn ev1(name: &'static str, a: impl Into<String>) -> Act {
    Act::Event { name, args: vec![a.into()] }
}

fn quiet(prop: &str, value: impl Into<String>) -> Act {
    Act::Quiet { prop: prop.to_owned(), value: value.into() }
}

fn b01(on: bool) -> &'static str {
    if on {
        "1"
    } else {
        "0"
    }
}

// ── the generic events ──────────────────────────────────────────────────────

/// The model events a plain DOM event raises on any control (mouse, keyboard, focus).
pub fn generic(c: &Ctl, p: &Payload) -> Vec<Act> {
    let mut out = Vec::new();
    let any = |names: &[&str]| names.iter().any(|n| c.has_handler(n));
    match p.kind.as_str() {
        "click" => out.push(ev("onClick")),
        "dblclick" => {
            out.push(ev("onDblClick"));
            out.push(ev("onDoubleClick"));
        }
        "contextmenu" => {
            out.push(ev("onRightClick"));
            out.push(ev("onContextMenu"));
            if any(&["onRightClick", "onContextMenu"]) {
                out.push(Act::Prevent);
            }
        }
        "auxclick" if p.button == 1 => out.push(ev("onMiddleClick")),
        "mousedown" | "pointerdown" => out.push(ev("onMouseDown")),
        "mouseup" | "pointerup" => out.push(ev("onMouseUp")),
        "mousemove" | "pointermove" => out.push(ev("onMouseMove")),
        "mouseenter" | "pointerenter" => {
            out.push(ev("onMouseEnter"));
            out.push(ev("onHoverEnter"));
        }
        "mouseleave" | "pointerleave" => {
            out.push(ev("onMouseLeave"));
            out.push(ev("onHoverLeave"));
        }
        "wheel" => out.push(ev1("onMouseWheel", fmt_number(p.delta))),
        "focusin" => {
            out.push(ev("onGotFocus"));
            out.push(ev("onEnter"));
        }
        "focusout" => {
            out.push(ev("onLostFocus"));
            out.push(ev("onLeave"));
        }
        "keydown" => {
            out.push(ev1("onKeyDown", p.key_name.clone()));
            if p.key_name.chars().count() == 1 && !p.ctrl && !p.meta {
                out.push(ev1("onKeyPress", p.key_name.clone()));
            }
            match p.key_name.as_str() {
                "Enter" => out.push(ev("onEnterPressed")),
                "Escape" => out.push(ev("onEscapePressed")),
                _ => {}
            }
        }
        "keyup" => out.push(ev1("onKeyUp", p.key_name.clone())),
        _ => {}
    }
    out
}

/// Which node of a control the generic events come from.
fn generic_key(c: &Ctl) -> &'static str {
    match c.control_type() {
        Some(ControlType::CheckBox | ControlType::RadioButton) => "input",
        _ => "root",
    }
}

// ── the entry point ─────────────────────────────────────────────────────────

/// Everything a DOM event on control `ctl` asks for, in order.
pub fn on_event(cx: &ViewCtx, ctl: usize, p: &Payload) -> Vec<Act> {
    let c = cx.tree.ctl(ctl);
    let mut acts = Vec::new();
    acts.extend(behaviour(cx, ctl, p));
    // A click on a menu item, a tool bar button or a link has already raised the click that means
    // something for that control (`onMenuClick`, `onClick(button)`, `onClick(modifiers)`).
    let own_click = p.kind == "click"
        && match c.control_type() {
            Some(ControlType::MenuBar | ControlType::SideMenu | ControlType::ToolBar) => p.id.is_some(),
            _ => crate::kinds::player_kind(c) == Some(crate::kinds::PlayerKind::WebLink),
        };
    // The expand icon of a card takes its own click: it is not a click on the card (spec 090 R3).
    let on_expander = p.part.as_deref() == Some("expander");
    if p.key == generic_key(c) && !own_click && !on_expander {
        acts.extend(generic(c, p));
    }
    acts
}

fn behaviour(cx: &ViewCtx, i: usize, p: &Payload) -> Vec<Act> {
    let tree = cx.tree;
    let c = tree.ctl(i);
    if let Some(k) = crate::kinds::player_kind(c) {
        return match k {
            crate::kinds::PlayerKind::WebLink => weblink(cx, i, p),
            crate::kinds::PlayerKind::Authentication => authentication(cx, i, p),
            _ => Vec::new(),
        };
    }
    let Kind::Control(t) = &c.kind else { return Vec::new() };
    if !c.enabled() {
        return Vec::new();
    }
    match t {
        ControlType::TextBox => text_box(cx, i, p),
        ControlType::CheckBox | ControlType::RadioButton => check_or_radio(cx, i, p),
        ControlType::Switch => switch(cx, i, p),
        ControlType::ListBox => list_box(cx, i, p),
        ControlType::ComboBox => combo_box(cx, i, p),
        ControlType::TabControl => tab_control(cx, i, p),
        ControlType::Slider => slider(p),
        ControlType::NumericUpDown => numeric(cx, i, p),
        ControlType::Panel | ControlType::GroupBox => expander(cx, i, p),
        ControlType::DateTimePicker => {
            if matches!(p.kind.as_str(), "input" | "change") {
                vec![Act::Change { prop: "Value".into(), value: p.value.clone() }]
            } else {
                Vec::new()
            }
        }
        ControlType::TreeView => tree_view(cx, i, p),
        ControlType::DataGrid => data_grid(cx, i, p),
        ControlType::MenuBar => menu(cx, i, p, true),
        ControlType::SideMenu => menu(cx, i, p, false),
        ControlType::ToolBar => tool_bar(cx, i, p),
        ControlType::Splitter => splitter(cx, i, p),
        ControlType::Knob => knob(cx, i, p),
        ControlType::Maps => maps(cx, i, p),
        ControlType::FileDropZone => drop_zone(cx, i, p),
        ControlType::Viewer => viewer(cx, i, p),
        _ => Vec::new(),
    }
}

// ── the controls ────────────────────────────────────────────────────────────

/// A click on a card's expand icon turns its `Expanded` over (spec 090 R3).
fn expander(cx: &ViewCtx, i: usize, p: &Payload) -> Vec<Act> {
    if p.kind != "click" || p.part.as_deref() != Some("expander") {
        return Vec::new();
    }
    let expanded = crate::value::truthy(&cx.tree.get(i, "Expanded"));
    vec![Act::Change { prop: "Expanded".into(), value: if expanded { "0" } else { "1" }.into() }]
}

fn text_box(cx: &ViewCtx, i: usize, p: &Payload) -> Vec<Act> {
    let Some(pic) = cx.tree.picture_of(i) else {
        return match p.kind.as_str() {
            "input" => vec![Act::Change { prop: "Text".into(), value: p.value.clone() }],
            _ => Vec::new(),
        };
    };
    // A PICTURE decides what the box may hold, keystroke by keystroke (the desktop's rule: the text is valid
    // so far, or the keystroke is refused).
    let dc = cx.tree.decimal_comma;
    match p.kind.as_str() {
        "input" => {
            if pic.accepts_partial(&p.value, dc) {
                vec![Act::Change { prop: "Text".into(), value: p.value.clone() }]
            } else {
                // Refused: the box goes back to what it held. Storing the same value redraws the control,
                // and the redraw writes the stored text over what the user typed.
                vec![quiet("Text", cx.tree.get(i, "Text"))]
            }
        }
        "focusin" if pic.is_edited() => vec![Act::Ui(UiOp::Editing(true))],
        "focusout" if pic.is_numeric() => {
            let mut v = vec![Act::Ui(UiOp::Editing(false))];
            // A typed sign goes where the picture puts it (`ZZ9.99-`, `9(4)CR`).
            let stored = cx.tree.get(i, "Text");
            let moved = pic.normalize_sign(&stored, dc);
            if moved != stored {
                v.push(Act::Change { prop: "Text".into(), value: moved });
            }
            v
        }
        _ => Vec::new(),
    }
}

fn toggle_events(on: bool, radio: bool) -> Vec<Act> {
    let mut v = vec![ev("onChange"), ev1("onCheckedChanged", if on { "true" } else { "false" }), ev("onValueChanged")];
    v.push(ev(if on { "onCheck" } else { "onUncheck" }));
    let _ = radio;
    v
}

fn check_or_radio(cx: &ViewCtx, i: usize, p: &Payload) -> Vec<Act> {
    let radio = matches!(cx.tree.ctl(i).control_type(), Some(ControlType::RadioButton));
    match (p.kind.as_str(), p.key.as_str()) {
        ("change", "input") => {
            let before = truthy(&cx.tree.get(i, SELECTED_PROP)) || truthy(&cx.tree.get(i, CHECKED_PROP));
            if before == p.checked {
                return Vec::new();
            }
            let mut v = vec![quiet(if radio { SELECTED_PROP } else { CHECKED_PROP }, b01(p.checked))];
            v.extend(toggle_events(p.checked, radio));
            v
        }
        _ => Vec::new(),
    }
}

fn switch(cx: &ViewCtx, i: usize, p: &Payload) -> Vec<Act> {
    if p.kind != "click" && !(p.kind == "keydown" && p.key_name == " ") {
        return Vec::new();
    }
    if p.kind == "keydown" {
        return Vec::new(); // a <button> already turns Space into a click
    }
    let on = !(truthy(&cx.tree.get(i, "Checked")) || truthy(&cx.tree.get(i, "Selected")));
    let mut v = vec![quiet("Checked", b01(on))];
    v.extend(toggle_events(on, false));
    v
}

fn slider(p: &Payload) -> Vec<Act> {
    if matches!(p.kind.as_str(), "input") {
        vec![Act::Change { prop: "Value".into(), value: p.value.clone() }]
    } else {
        Vec::new()
    }
}

fn numeric(cx: &ViewCtx, i: usize, p: &Payload) -> Vec<Act> {
    if !matches!(p.kind.as_str(), "input" | "change") {
        return Vec::new();
    }
    let Some(v) = parse_f64(&p.value) else { return Vec::new() };
    let min = parse_f64(&cx.tree.get(i, "Minimum")).unwrap_or(0.0);
    let max = parse_f64(&cx.tree.get(i, "Maximum")).unwrap_or(100.0);
    let v = v.clamp(min.min(max), max.max(min));
    vec![Act::Change { prop: "Value".into(), value: fmt_number(v) }]
}

fn list_items(cx: &ViewCtx, i: usize) -> Vec<String> {
    let mut items = lines(&cx.tree.get(i, "Items"));
    display_items(cx, i, &mut items);
    items
}

fn list_box(cx: &ViewCtx, i: usize, p: &Payload) -> Vec<Act> {
    let items = list_items(cx, i);
    let multi = truthy(&cx.tree.get(i, "MultiSelect"));
    let checks = truthy(&cx.tree.get(i, "ShowCheckBoxes"));
    let current = parse_f64(&cx.tree.get(i, "SelectedIndex")).map(|v| v as i64).unwrap_or(0);
    let select = |k: usize, extra: Vec<Act>| -> Vec<Act> {
        let mut v = vec![quiet("Value", items[k].clone()), quiet("SelectedIndex", (k + 1).to_string())];
        v.extend(extra);
        v.push(ev("onChange"));
        v.push(ev("onSelectedIndexChanged"));
        v
    };
    match p.kind.as_str() {
        // The handle follows where the rows have been scrolled to.
        "scroll" => {
            let (left, top) = (p.scroll_left.max(0.0).round(), p.scroll_top.max(0.0).round());
            if cx.ui.grid_scroll.get(&i).copied().unwrap_or((0.0, 0.0)) == (left, top) {
                return Vec::new();
            }
            vec![Act::Ui(UiOp::GridScroll { left, top })]
        }
        "click" => {
            let Some(k) = p.index.filter(|k| *k >= 0 && (*k as usize) < items.len()).map(|k| k as usize) else { return Vec::new() };
            if checks && p.part.as_deref() == Some("check") {
                let mut set = lines_nonempty(&cx.tree.get(i, "CheckedItems"));
                match set.iter().position(|s| *s == items[k]) {
                    Some(at) => {
                        set.remove(at);
                    }
                    None => set.push(items[k].clone()),
                }
                let joined = set.join("\n");
                return vec![quiet("CheckedItems", joined.clone()), ev1("onItemChecked", joined)];
            }
            let mut extra = Vec::new();
            if multi {
                let mut set = lines_nonempty(&cx.tree.get(i, "SelectedItems"));
                if p.ctrl || p.meta {
                    match set.iter().position(|s| *s == items[k]) {
                        Some(at) => {
                            set.remove(at);
                        }
                        None => set.push(items[k].clone()),
                    }
                } else if p.shift && current > 0 {
                    let (a, b) = ((current as usize - 1).min(k), (current as usize - 1).max(k));
                    set = items[a..=b.min(items.len() - 1)].to_vec();
                } else {
                    set = vec![items[k].clone()];
                }
                extra.push(quiet("SelectedItems", set.join("\n")));
                extra.push(ev("onSelectionChanged"));
            }
            select(k, extra)
        }
        "dblclick" => match p.index.filter(|k| *k >= 0 && (*k as usize) < items.len()) {
            Some(k) => vec![ev1("onItemDoubleClick", items[k as usize].clone())],
            None => Vec::new(),
        },
        "keydown" if !items.is_empty() => {
            let n = items.len() as i64;
            let to = match p.key_name.as_str() {
                "ArrowDown" => Some((current + 1).clamp(1, n)),
                "ArrowUp" => Some(if current <= 1 { 1 } else { current - 1 }),
                "Home" => Some(1),
                "End" => Some(n),
                " " if checks && current > 0 => {
                    let k = current as usize - 1;
                    let mut set = lines_nonempty(&cx.tree.get(i, "CheckedItems"));
                    match set.iter().position(|s| *s == items[k]) {
                        Some(at) => {
                            set.remove(at);
                        }
                        None => set.push(items[k].clone()),
                    }
                    let joined = set.join("\n");
                    return vec![Act::Prevent, quiet("CheckedItems", joined.clone()), ev1("onItemChecked", joined)];
                }
                _ => None,
            };
            match to {
                Some(t) if t != current => {
                    let mut v = vec![Act::Prevent];
                    v.extend(select(t as usize - 1, Vec::new()));
                    v
                }
                Some(_) => vec![Act::Prevent],
                None => Vec::new(),
            }
        }
        _ => Vec::new(),
    }
}

fn combo_box(cx: &ViewCtx, i: usize, p: &Payload) -> Vec<Act> {
    let items = list_items(cx, i);
    let current = parse_f64(&cx.tree.get(i, "SelectedIndex")).map(|v| v as i64).unwrap_or(0);
    match (p.kind.as_str(), p.key.as_str()) {
        // A <select> (the list-only style).
        ("change", "root" | "select") => {
            let k = p.value.trim().parse::<usize>().ok().filter(|k| *k < items.len());
            let (index, text) = match k {
                Some(k) => ((k + 1) as i64, items[k].clone()),
                None => (0, String::new()),
            };
            let mut v = vec![quiet("Text", text.clone()), quiet("Value", text), quiet("SelectedIndex", index.to_string()), ev("onChange")];
            if index != current {
                v.push(ev("onSelectedIndexChanged"));
            }
            v
        }
        // The editable style: a text field offering the items.
        ("input", "input") | ("change", "input") => {
            let matched = items.iter().position(|s| *s == p.value).map(|k| (k + 1) as i64).unwrap_or(0);
            let mut v = vec![Act::Change { prop: "Text".into(), value: p.value.clone() }, quiet("Value", p.value.clone())];
            if matched != current {
                v.push(quiet("SelectedIndex", matched.to_string()));
                v.push(ev("onSelectedIndexChanged"));
            }
            v
        }
        _ => Vec::new(),
    }
}

fn tab_control(cx: &ViewCtx, i: usize, p: &Payload) -> Vec<Act> {
    let pages = super::controls::tab_titles(cx, i).len() as i64;
    let current = parse_f64(&cx.tree.get(i, "SelectedTab")).map(|v| v as i64).unwrap_or(1);
    let to = match p.kind.as_str() {
        "click" => p.index.filter(|k| *k >= 1 && *k <= pages),
        "keydown" => match p.key_name.as_str() {
            "ArrowRight" | "ArrowDown" => Some(if current >= pages { 1 } else { current + 1 }),
            "ArrowLeft" | "ArrowUp" => Some(if current <= 1 { pages } else { current - 1 }),
            "Home" => Some(1),
            "End" => Some(pages),
            _ => None,
        },
        _ => None,
    };
    let Some(page) = to else { return Vec::new() };
    let mut v = Vec::new();
    if p.kind == "keydown" {
        v.push(Act::Prevent);
    }
    if page != current {
        v.push(quiet("SelectedTab", page.to_string()));
    }
    v.push(ev1("onTabClick", page.to_string()));
    if page != current {
        v.push(ev1("onTabChanged", page.to_string()));
    }
    v
}

// ── TreeView ────────────────────────────────────────────────────────────────

fn tree_view(cx: &ViewCtx, i: usize, p: &Payload) -> Vec<Act> {
    let items = cx.tree.get(i, "Items");
    let selected = cx.tree.get(i, "SelectedNode");
    let checked: Vec<String> = lines_nonempty(&cx.tree.get(i, "CheckedNodes").lines().map(str::trim).collect::<Vec<_>>().join("\n"));
    let collapsed: Vec<String> = cx.tree.get(i, "CollapsedNodes").lines().map(str::trim).filter(|s| !s.is_empty()).map(str::to_owned).collect();
    let toggled = |list: &[String], text: &str| -> Vec<String> {
        let mut v = list.to_vec();
        match v.iter().position(|s| s == text) {
            Some(at) => {
                v.remove(at);
            }
            None => v.push(text.to_owned()),
        }
        v
    };
    match p.kind.as_str() {
        "click" | "dblclick" => {
            let Some(idx) = p.index.filter(|k| *k >= 0).map(|k| k as usize) else { return Vec::new() };
            let Some(n) = node_at(&items, idx) else { return Vec::new() };
            if p.kind == "dblclick" {
                let payload = node_payload(&n, &checked);
                return vec![ev1("onNodeDblClick", payload.clone()), ev1("onNodeDoubleClick", payload)];
            }
            match p.part.as_deref() {
                Some("arrow") => {
                    let was_collapsed = collapsed.iter().any(|c| *c == n.text);
                    let next = toggled(&collapsed, &n.text);
                    vec![quiet("CollapsedNodes", next.join("\n")), ev1(if was_collapsed { "onNodeExpand" } else { "onNodeCollapse" }, node_payload(&n, &checked))]
                }
                Some("check") => {
                    let next = toggled(&checked, &n.text);
                    let payload = node_payload(&n, &next);
                    vec![quiet("CheckedNodes", next.join("\n")), ev1("onNodeCheck", payload)]
                }
                _ => {
                    let mut v = vec![quiet("SelectedNode", n.text.clone()), ev1("onNodeClick", node_payload(&n, &checked))];
                    if selected != n.text {
                        v.push(ev1("onNodeSelect", node_payload(&n, &checked)));
                    }
                    v
                }
            }
        }
        "keydown" => {
            let all = nodes(&items);
            let vis = visible_nodes(&all, &collapsed);
            if vis.is_empty() {
                return Vec::new();
            }
            let at = vis.iter().position(|n| !selected.is_empty() && n.text == selected);
            let go = |k: usize| -> Vec<Act> {
                let n = vis[k];
                let mut v = vec![Act::Prevent, quiet("SelectedNode", n.text.clone()), ev1("onNodeClick", node_payload(n, &checked))];
                if selected != n.text {
                    v.push(ev1("onNodeSelect", node_payload(n, &checked)));
                }
                v
            };
            match p.key_name.as_str() {
                "ArrowDown" => go(match at {
                    None => 0,
                    Some(k) => (k + 1).min(vis.len() - 1),
                }),
                "ArrowUp" => go(match at {
                    None => vis.len() - 1,
                    Some(k) => k.saturating_sub(1),
                }),
                "Home" => go(0),
                "End" => go(vis.len() - 1),
                "ArrowRight" | "ArrowLeft" => {
                    let Some(k) = at else { return Vec::new() };
                    let n = vis[k];
                    if n.child_count == 0 {
                        return Vec::new();
                    }
                    let is_collapsed = collapsed.iter().any(|c| *c == n.text);
                    let want_open = p.key_name == "ArrowRight";
                    if want_open != is_collapsed {
                        return vec![Act::Prevent];
                    }
                    vec![Act::Prevent, quiet("CollapsedNodes", toggled(&collapsed, &n.text).join("\n")), ev1(if is_collapsed { "onNodeExpand" } else { "onNodeCollapse" }, node_payload(n, &checked))]
                }
                _ => Vec::new(),
            }
        }
        _ => Vec::new(),
    }
}

// ── DataGrid ────────────────────────────────────────────────────────────────

fn data_grid(cx: &ViewCtx, i: usize, p: &Payload) -> Vec<Act> {
    use super::datagrid;
    let sort = cx.ui.grid_sort.get(&i).copied();
    match p.kind.as_str() {
        "scroll" => {
            // Where the page is scrolled to is drawn (the bars follow it, the rows in view are chosen by it).
            let (left, top) = (p.scroll_left.max(0.0).round(), p.scroll_top.max(0.0).round());
            if cx.ui.grid_scroll.get(&i).copied().unwrap_or((0.0, 0.0)) == (left, top) {
                return Vec::new();
            }
            vec![Act::Ui(UiOp::GridScroll { left, top }), ev1("onScroll", format!("{top:.0},{left:.0}"))]
        }
        "click" | "dblclick" => {
            // The CSV button asks for the grid's rows as a file.
            if p.part.as_deref() == Some("csv") {
                return if p.kind == "click" { vec![ev("onExportCSV")] } else { Vec::new() };
            }
            // A column-move button: the column swaps places with its neighbour, as the desktop's does.
            if p.part.as_deref() == Some("colmove") {
                let (Some(col), "click") = (p.col.filter(|c| *c >= 0).map(|c| c as usize), p.kind.as_str()) else { return Vec::new() };
                let left = p.id.as_deref() == Some("left");
                return match datagrid::moved_columns(cx, i, col, left) {
                    Some(json) => vec![quiet("AdvancedGrid", json)],
                    None => Vec::new(),
                };
            }
            // A header click: sort, and report the column.
            if p.part.as_deref() == Some("header") || (p.row.is_none() && p.col.is_some()) {
                let Some(col) = p.col.filter(|c| *c >= 0).map(|c| c as usize) else { return Vec::new() };
                if p.kind == "dblclick" {
                    return Vec::new();
                }
                let mut v = vec![ev1("onColumnClick", col.to_string())];
                if truthy(&cx.tree.get(i, "AllowSorting")) && datagrid::sortable(cx, i, col) {
                    let descending = matches!(sort, Some((c, false)) if c == col);
                    v.push(Act::Ui(UiOp::GridSort { col, descending }));
                }
                return v;
            }
            let rows = datagrid::row_count(cx, i);
            let (Some(row), col) = (p.row.filter(|r| *r >= 0).map(|r| r as usize), p.col.unwrap_or(0).max(0) as usize) else { return Vec::new() };
            if row >= rows {
                return Vec::new();
            }
            let prev = parse_f64(&cx.tree.get(i, "ClickedRow")).map(|v| v as i64).unwrap_or(0);
            // The cell as the program counts it: the data row and the data column from 1 (0 for a column with no data behind it).
            let source = datagrid::source_of(cx, i, col).map(|s| s + 1).unwrap_or(0);
            let value = format!("{row},{col}");
            if p.kind == "dblclick" {
                return vec![ev1("onCellDoubleClick", value), ev1("onRowDoubleClick", row.to_string())];
            }
            let mut v = vec![quiet("ClickedRow", (row + 1).to_string()), quiet("ClickedColumn", source.to_string())];
            if prev != row as i64 + 1 {
                v.push(ev("onSelectionChanged"));
                v.push(ev("onRowSelect"));
            }
            v.push(ev1("onCellClick", value));
            v
        }
        "keydown" => {
            let order = datagrid::displayed_order(cx, i);
            if order.is_empty() {
                return Vec::new();
            }
            let prev = parse_f64(&cx.tree.get(i, "ClickedRow")).map(|v| v as i64).unwrap_or(0);
            let pos = order.iter().position(|r| *r as i64 + 1 == prev);
            let to = match p.key_name.as_str() {
                "ArrowDown" => Some(pos.map(|k| (k + 1).min(order.len() - 1)).unwrap_or(0)),
                "ArrowUp" => Some(pos.map(|k| k.saturating_sub(1)).unwrap_or(0)),
                "Home" => Some(0),
                "End" => Some(order.len() - 1),
                "PageDown" => Some(pos.map(|k| (k + 10).min(order.len() - 1)).unwrap_or(0)),
                "PageUp" => Some(pos.map(|k| k.saturating_sub(10)).unwrap_or(0)),
                _ => None,
            };
            match to {
                Some(k) => {
                    let row = order[k];
                    let mut v = vec![Act::Prevent];
                    if prev != row as i64 + 1 {
                        v.push(quiet("ClickedRow", (row + 1).to_string()));
                        v.push(quiet("ClickedColumn", "1"));
                        v.push(ev("onSelectionChanged"));
                        v.push(ev("onRowSelect"));
                        v.push(ev1("onCellClick", format!("{row},0")));
                    }
                    v
                }
                None => Vec::new(),
            }
        }
        _ => Vec::new(),
    }
}

// ── menus, tool bar ─────────────────────────────────────────────────────────

fn find_item<'a>(items: &'a [MenuItem], id: &str) -> Option<&'a MenuItem> {
    for it in items {
        if it.id == id {
            return Some(it);
        }
        if let Some(f) = find_item(&it.items, id) {
            return Some(f);
        }
    }
    None
}

/// The id of every ancestor of `id`, outermost first (the chain a MenuBar keeps open).
fn path_to(items: &[MenuItem], id: &str) -> Option<Vec<String>> {
    for it in items {
        if it.id == id {
            return Some(vec![it.id.clone()]);
        }
        if let Some(mut rest) = path_to(&it.items, id) {
            let mut v = vec![it.id.clone()];
            v.append(&mut rest);
            return Some(v);
        }
    }
    None
}

fn menu(cx: &ViewCtx, i: usize, p: &Payload, bar: bool) -> Vec<Act> {
    let items = {
        let designed = menu_items(cx, i);
        let raw = cx.tree.get(i, cobolt_forms::menu::runtime::RUNTIME_ROWS_PROP);
        if raw.trim().is_empty() {
            designed.to_vec()
        } else {
            cobolt_forms::menu::runtime::merge_rows(designed, &cobolt_forms::menu::runtime::parse_rows(&raw))
        }
    };
    match p.kind.as_str() {
        "click" => {
            if p.part.as_deref() == Some("toggle") {
                let now = cx.ui.sidebar_collapsed.unwrap_or_else(|| truthy(&cx.tree.get(i, "Collapsed")));
                return vec![Act::Ui(UiOp::Sidebar(!now))];
            }
            let Some(id) = p.id.clone() else {
                // A click that hits no item closes an open MenuBar.
                return if bar && !cx.ui.open_menu.is_empty() { vec![Act::Ui(UiOp::SetOpenMenu(Vec::new()))] } else { Vec::new() };
            };
            let Some(item) = find_item(&items, &id) else { return Vec::new() };
            if !item.enabled {
                return Vec::new();
            }
            if item.has_children() {
                let mut open = cx.ui.open_menu.clone();
                if bar {
                    // A MenuBar keeps one chain open: clicking the open item closes it.
                    let chain = path_to(&items, &id).unwrap_or_else(|| vec![id.clone()]);
                    open = if open.last() == chain.last() { chain[..chain.len() - 1].to_vec() } else { chain };
                } else if let Some(at) = open.iter().position(|o| *o == id) {
                    open.remove(at);
                } else {
                    open.push(id);
                }
                return vec![Act::Ui(UiOp::SetOpenMenu(open))];
            }
            let mut v = vec![quiet("SelectedItemId", item.id.clone()), ev1("onMenuClick", item.id.clone()), ev1("onMenuItemClick", item.id.clone())];
            if bar {
                v.push(Act::Ui(UiOp::SetOpenMenu(Vec::new())));
            }
            v.push(Act::Menu { item: item.id.clone() });
            v
        }
        "keydown" if p.key_name == "Escape" && bar && !cx.ui.open_menu.is_empty() => vec![Act::Prevent, Act::Ui(UiOp::SetOpenMenu(Vec::new()))],
        _ => Vec::new(),
    }
}

/// What choosing a menu item asks the application to do.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MenuTarget {
    /// Load the form into the ContentPane (`open-form:NAME`).
    Embed { form: String, preserve_previous: bool },
    /// Open the form in a window of its own (`open-standalone-*:NAME`).
    Standalone { form: String, sync: bool },
    /// Nothing to load: the program's `onMenuClick` handler decides.
    None,
}

pub fn menu_target(item: &MenuItem) -> MenuTarget {
    if let Some(form) = open_form_target(item) {
        return MenuTarget::Embed { form: form.to_owned(), preserve_previous: item.preserve_previous_form };
    }
    if let Some((form, sync)) = open_standalone_target(item) {
        return MenuTarget::Standalone { form: form.to_owned(), sync };
    }
    MenuTarget::None
}

/// The item of control `i`'s menu with this id.
pub fn item_of(cx: &ViewCtx, i: usize, id: &str) -> Option<MenuItem> {
    find_item(menu_items(cx, i), id).cloned()
}

fn tool_bar(cx: &ViewCtx, i: usize, p: &Payload) -> Vec<Act> {
    if p.kind != "click" {
        return Vec::new();
    }
    let Some(id) = p.id.clone() else { return Vec::new() };
    let def = toolbar_def(cx.tree, i);
    let Some(btn) = def.groups.iter().flat_map(|g| g.buttons.iter()).find(|b| b.id == id) else { return Vec::new() };
    if !btn.enabled {
        return Vec::new();
    }
    let action = cobolt_forms::toolbar::ToolbarAction::parse(&btn.action);
    if super::shell::unavailable_in_a_browser(&action) {
        return vec![Act::Notice(format!("\u{201C}{}\u{201D} needs the operating system and is not available in a browser", if btn.label.is_empty() { btn.id.clone() } else { btn.label.clone() }))];
    }
    let mut v = vec![quiet("LastButton", id.clone())];
    if let cobolt_forms::toolbar::ToolbarAction::OpenModal(form) = action {
        v.push(Act::OpenModal { form });
    }
    v.push(ev1("onClick", id));
    v
}

// ── Splitter, Knob ──────────────────────────────────────────────────────────

fn splitter(cx: &ViewCtx, i: usize, p: &Payload) -> Vec<Act> {
    let horizontal = !cx.tree.get(i, "Orientation").eq_ignore_ascii_case("Vertical");
    let pct = parse_f64(&cx.tree.get(i, "SplitPosition")).unwrap_or(50.0);
    match p.kind.as_str() {
        "pointermove" | "pointerdown" if p.buttons & 1 == 1 && p.key == "handle" => {
            let (pos, span) = if horizontal { (p.x, p.w) } else { (p.y, p.h) };
            if span <= 0.0 {
                return Vec::new();
            }
            let np = (pos / span * 100.0).round().clamp(0.0, 100.0);
            if (np - pct).abs() < 0.5 {
                return Vec::new();
            }
            vec![Act::Prevent, quiet("SplitPosition", fmt_number(np))]
        }
        "keydown" if p.key == "handle" => {
            let step = if p.shift { 10.0 } else { 2.0 };
            let np = match (p.key_name.as_str(), horizontal) {
                ("ArrowLeft", true) | ("ArrowUp", false) => pct - step,
                ("ArrowRight", true) | ("ArrowDown", false) => pct + step,
                ("Home", _) => 0.0,
                ("End", _) => 100.0,
                _ => return Vec::new(),
            };
            vec![Act::Prevent, quiet("SplitPosition", fmt_number(np.clamp(0.0, 100.0)))]
        }
        _ => Vec::new(),
    }
}

/// The value a pointer at `(x, y)` inside a knob's box selects: the angle from the centre, on a dial
/// of ±135 degrees, snapped to `step`.
pub fn knob_value_at(x: f64, y: f64, w: f64, h: f64, min: f64, max: f64, step: f64) -> f64 {
    let (dx, dy) = (x - w / 2.0, y - h / 2.0);
    // Degrees clockwise from the top.
    let mut deg = dx.atan2(-dy).to_degrees();
    if deg < -180.0 {
        deg += 360.0;
    }
    let deg = deg.clamp(-135.0, 135.0);
    let frac = (deg + 135.0) / 270.0;
    let raw = min + frac * (max - min);
    let s = step.max(f64::EPSILON);
    ((raw - min) / s).round() * s + min
}

fn knob(cx: &ViewCtx, i: usize, p: &Payload) -> Vec<Act> {
    let get = |k: &str, d: f64| parse_f64(&cx.tree.get(i, k)).unwrap_or(d);
    let (min, max, step) = (get("Minimum", 0.0), get("Maximum", 100.0), get("Step", 1.0).max(f64::EPSILON));
    let cur = get("Value", 0.0);
    let set = |v: f64| -> Vec<Act> {
        let v = v.clamp(min.min(max), max.max(min));
        if (v - cur).abs() < f64::EPSILON {
            return Vec::new();
        }
        vec![Act::Change { prop: "Value".into(), value: fmt_number(v) }]
    };
    match p.kind.as_str() {
        "pointerdown" => {
            let mut v = vec![Act::Prevent, Act::Ui(UiOp::Focus)];
            v.extend(set(knob_value_at(p.x, p.y, p.w, p.h, min, max, step)));
            v
        }
        "pointermove" if p.buttons & 1 == 1 => set(knob_value_at(p.x, p.y, p.w, p.h, min, max, step)),
        "wheel" => {
            let mut v = vec![Act::Prevent];
            v.extend(set(cur + if p.delta < 0.0 { step } else { -step }));
            v
        }
        "keydown" => {
            let to = match p.key_name.as_str() {
                "ArrowUp" | "ArrowRight" => Some(cur + step),
                "ArrowDown" | "ArrowLeft" => Some(cur - step),
                "Home" => Some(min),
                "End" => Some(max),
                "PageUp" => Some(cur + step * 10.0),
                "PageDown" => Some(cur - step * 10.0),
                _ => None,
            };
            match to {
                Some(t) => {
                    let mut v = vec![Act::Prevent];
                    v.extend(set(t));
                    v
                }
                None => Vec::new(),
            }
        }
        _ => Vec::new(),
    }
}

// ── Maps ────────────────────────────────────────────────────────────────────

fn maps(cx: &ViewCtx, i: usize, p: &Payload) -> Vec<Act> {
    use super::maps::Viewport;
    let v = Viewport::read(cx, i);
    let set_view = |lat: f64, lng: f64, zoom: u8| -> Vec<Act> {
        vec![quiet("CenterLat", fmt_number((lat * 1e6).round() / 1e6)), quiet("CenterLng", fmt_number((lng * 1e6).round() / 1e6)), quiet("Zoom", zoom.to_string()), ev("onBoundsChanged")]
    };
    match p.kind.as_str() {
        "click" => {
            match p.part.as_deref() {
                Some("zoom-in") => return set_view(v.lat, v.lng, (v.zoom + 1).min(super::maps::MAX_ZOOM)),
                Some("zoom-out") => return set_view(v.lat, v.lng, v.zoom.saturating_sub(1).max(super::maps::MIN_ZOOM)),
                _ => {}
            }
            if let Some(id) = p.id.clone() {
                // A marker.
                let markers = cobolt_forms::parse_map_markers(&cx.tree.get(i, "Markers"));
                let Some(m) = markers.iter().find(|m| m.id == id) else { return Vec::new() };
                let was_open = cx.tree.get(i, "SelectedMarkerId") == id;
                let mut out = vec![quiet("SelectedMarkerId", if was_open { String::new() } else { id.clone() })];
                if !was_open {
                    out.push(ev1("onMarkerClick", m.id.clone()));
                }
                return out;
            }
            let (lat, lng) = v.to_lat_lng(p.x, p.y);
            vec![quiet("SelectedMarkerId", ""), ev1("onMapClick", format!("{},{}", fmt_number((lat * 1e6).round() / 1e6), fmt_number((lng * 1e6).round() / 1e6)))]
        }
        "dblclick" => {
            let (lat, lng) = v.to_lat_lng(p.x, p.y);
            set_view(lat, lng, (v.zoom + 1).min(super::maps::MAX_ZOOM))
        }
        "wheel" => {
            let (lat, lng) = v.to_lat_lng(p.x, p.y);
            let z = if p.delta < 0.0 { (v.zoom + 1).min(super::maps::MAX_ZOOM) } else { v.zoom.saturating_sub(1) };
            if z == v.zoom {
                return vec![Act::Prevent];
            }
            // Zoom about the pointer: the coordinate under it stays under it.
            let nv = Viewport { zoom: z, ..v };
            let (under_lat, under_lng) = nv.to_lat_lng(p.x, p.y);
            let (nlat, nlng) = (v.lat + (lat - under_lat), v.lng + (lng - under_lng));
            let mut out = vec![Act::Prevent];
            out.extend(set_view(nlat.clamp(-85.0, 85.0), nlng, z));
            out
        }
        "pointerdown" if p.part.is_none() && p.id.is_none() && p.button == 0 => vec![Act::Ui(UiOp::MapDrag { x: p.x, y: p.y, lat: v.lat, lng: v.lng })],
        "pointermove" if p.buttons & 1 == 1 => {
            let Some((x0, y0, lat0, lng0)) = cx.ui.map_drag.get(&i).copied() else { return Vec::new() };
            let start = Viewport { lat: lat0, lng: lng0, ..v };
            let (lat, lng) = start.panned(p.x - x0, p.y - y0);
            vec![Act::Prevent, quiet("CenterLat", fmt_number((lat * 1e6).round() / 1e6)), quiet("CenterLng", fmt_number((lng * 1e6).round() / 1e6))]
        }
        "pointerup" | "pointercancel" => {
            let moved = cx.ui.map_drag.get(&i).is_some_and(|(x0, y0, _, _)| (p.x - x0).abs() > 3.0 || (p.y - y0).abs() > 3.0);
            let mut out = vec![Act::Ui(UiOp::MapDragEnd)];
            if moved {
                out.push(ev("onBoundsChanged"));
            }
            out
        }
        "keydown" => {
            let (dx, dy) = match p.key_name.as_str() {
                "ArrowLeft" => (80.0, 0.0),
                "ArrowRight" => (-80.0, 0.0),
                "ArrowUp" => (0.0, 80.0),
                "ArrowDown" => (0.0, -80.0),
                "+" | "=" => return set_view(v.lat, v.lng, (v.zoom + 1).min(super::maps::MAX_ZOOM)),
                "-" => return set_view(v.lat, v.lng, v.zoom.saturating_sub(1)),
                _ => return Vec::new(),
            };
            let (lat, lng) = v.panned(dx, dy);
            let mut out = vec![Act::Prevent];
            out.extend(set_view(lat, lng, v.zoom));
            out
        }
        _ => Vec::new(),
    }
}

// ── FileDropZone ────────────────────────────────────────────────────────────

/// Split dropped files into the ones the zone takes and the ones it turns away (by extension and size).
pub fn classify_files(tree: &FormTree, ctl: usize, files: &[(String, u64)]) -> (Vec<(String, u64)>, Vec<(String, u64)>) {
    let allowed: Vec<String> = tree.get(ctl, "AllowedExtensions").split([',', ';', ' ']).map(|s| s.trim().trim_start_matches('.').to_ascii_lowercase()).filter(|s| !s.is_empty()).collect();
    let max_kb = parse_f64(&tree.get(ctl, "MaximumFileSizeKB")).unwrap_or(0.0).max(0.0);
    let mut ok = Vec::new();
    let mut no = Vec::new();
    for (name, size) in files {
        let ext = name.rsplit_once('.').map(|(_, e)| e.to_ascii_lowercase()).unwrap_or_default();
        let ext_ok = allowed.is_empty() || allowed.iter().any(|a| *a == ext);
        let size_ok = max_kb <= 0.0 || (*size as f64) <= max_kb * 1024.0;
        if ext_ok && size_ok {
            ok.push((name.clone(), *size));
        } else {
            no.push((name.clone(), *size));
        }
    }
    (ok, no)
}

fn drop_zone(cx: &ViewCtx, i: usize, p: &Payload) -> Vec<Act> {
    match p.kind.as_str() {
        "dragover" => vec![Act::Prevent, Act::Ui(UiOp::DropOver(true))],
        "dragleave" => vec![Act::Ui(UiOp::DropOver(false))],
        "drop" | "change" => {
            let mut out = vec![Act::Ui(UiOp::DropOver(false))];
            if p.kind == "drop" {
                out.insert(0, Act::Prevent);
            }
            if p.files.is_empty() {
                return out;
            }
            let (ok, no) = classify_files(cx.tree, i, &p.files);
            let join = |v: &[(String, u64)]| v.iter().map(|(n, _)| n.clone()).collect::<Vec<_>>().join("\n");
            if !ok.is_empty() {
                let stage = truthy(&cx.tree.get(i, "StageOnly"));
                out.push(quiet("DroppedFiles", join(&ok)));
                if stage {
                    out.push(quiet("StagedFiles", join(&ok)));
                }
                out.push(Act::Files { accepted: ok.clone() });
                out.push(ev1("onFilesDropped", join(&ok)));
            }
            if !no.is_empty() {
                out.push(quiet("RejectedFiles", join(&no)));
                out.push(ev1("onFilesRejected", join(&no)));
            }
            out
        }
        "click" | "keydown" if (p.kind == "click" && p.part.as_deref() != Some("file")) || (p.kind == "keydown" && matches!(p.key_name.as_str(), "Enter" | " ")) => vec![Act::Ui(UiOp::PickFiles)],
        _ => Vec::new(),
    }
}

// ── WebLink ─────────────────────────────────────────────────────────────────

fn weblink(cx: &ViewCtx, i: usize, p: &Payload) -> Vec<Act> {
    use crate::weblink::{plan, Click, LinkProps, Target};
    if !matches!(p.kind.as_str(), "click" | "auxclick") {
        return Vec::new();
    }
    let click = Click { ctrl: p.ctrl, meta: p.meta, shift: p.shift, alt: p.alt, button: if p.kind == "auxclick" { 1 } else { 0 } };
    let props = LinkProps::read(cx.tree, i);
    let mut out = Vec::new();
    match plan(&props, click, &cx.tree.policy) {
        Ok(Some(np)) => {
            // The browser performs the navigation (so Ctrl-click, middle-click and the context menu keep
            // their native meaning); the player steps in only for what an anchor cannot say.
            if np.target == Target::NewWindow && !np.features.is_empty() && np.download.is_none() {
                out.push(Act::Prevent);
                out.push(Act::OpenWindow { url: np.url.clone(), target: np.target.html().to_owned(), features: np.features.clone(), rel: np.rel.clone() });
            }
        }
        Ok(None) => out.push(Act::Prevent),
        Err(r) => {
            out.push(Act::Prevent);
            out.push(Act::Notice(r.message));
        }
    }
    out.push(ev1("onClick", click.modifiers_text()));
    out
}

/// The Authentication control (WASM-009): what a submit of its form, a click on one of its links or
/// on Sign out asks of the machine. The fields arrive in the payload only at the moment of the submit
/// and are not kept anywhere here.
fn authentication(cx: &ViewCtx, i: usize, p: &Payload) -> Vec<Act> {
    use crate::auth::{Action, Screen};
    let Some(ui) = cx.tree.auth(i) else { return Vec::new() };
    let screen = ui.machine.screen;
    match (p.kind.as_str(), p.key.as_str()) {
        ("submit", k) if k.starts_with("form-") => {
            let field = |n: &str| p.fields.0.iter().find(|(k, _)| k == n).map(|(_, v)| v.trim().to_owned()).unwrap_or_default();
            // The password fields are sent as typed: leading and trailing spaces can be part of one.
            let raw = |n: &str| p.fields.0.iter().find(|(k, _)| k == n).map(|(_, v)| v.clone()).unwrap_or_default();
            let (action, needed): (Action, &[&str]) = match screen {
                Screen::SignIn => (Action::SignIn { user: field("user"), password: raw("password") }, &["user", "password"]),
                Screen::SignUp => (Action::SignUp { user: field("user"), email: field("email"), password: raw("password") }, &["user", "email", "password"]),
                Screen::Confirm => (Action::Confirm { user: field("user"), code: field("code") }, &["user", "code"]),
                Screen::Forgot => (Action::Forgot { user: field("user") }, &["user"]),
                Screen::Reset => (Action::Reset { user: field("user"), code: field("code"), new_password: raw("new_password") }, &["user", "code", "new_password"]),
                Screen::Mfa => (Action::Mfa { code: field("code") }, &["code"]),
                Screen::SignedIn => return vec![Act::Prevent],
            };
            // The browser's own `required` validation normally stops an empty field before this point;
            // an empty one that gets here (a script, an old browser) is not sent.
            if needed.iter().any(|n| raw(n).is_empty()) {
                return vec![Act::Prevent];
            }
            vec![Act::Prevent, Act::Auth(action)]
        }
        ("click", _) => match p.id.as_deref() {
            Some("signout") => vec![Act::Auth(Action::SignOut)],
            Some("hosted") => vec![Act::Auth(Action::Hosted)],
            Some(id) => match id.strip_prefix("show:") {
                Some("SignIn") => vec![Act::Auth(Action::Show(Screen::SignIn))],
                Some("SignUp") => vec![Act::Auth(Action::Show(Screen::SignUp))],
                Some("Confirm") => vec![Act::Auth(Action::Show(Screen::Confirm))],
                Some("Forgot") => vec![Act::Auth(Action::Show(Screen::Forgot))],
                _ => Vec::new(),
            },
            None => Vec::new(),
        },
        _ => Vec::new(),
    }
}

/// The Viewer's Find bar and its shortcut (spec 058 R26–R30). The needle, the case switch and the current
/// match are the control's properties; the page reports how many matches there are (`find_total`).
fn viewer(cx: &ViewCtx, i: usize, p: &Payload) -> Vec<Act> {
    let open = truthy(&cx.tree.get(i, "View1FindOpen"));
    let total = cx.ui.find_total.get(&i).copied().unwrap_or(0);
    let current = parse_f64(&cx.tree.get(i, "View1SearchCurrentMatch")).map(|v| v.max(0.0) as usize).unwrap_or(0);
    // Next and Previous wrap round; with no match they do nothing.
    let step = |forward: bool| -> Vec<Act> {
        if total == 0 {
            return Vec::new();
        }
        let to = if forward {
            if current == 0 || current >= total { 1 } else { current + 1 }
        } else if current <= 1 {
            total
        } else {
            current - 1
        };
        vec![quiet("View1SearchCurrentMatch", to.to_string())]
    };
    let close = || vec![quiet("View1FindOpen", "0"), ev("onFindClosed")];
    match (p.kind.as_str(), p.key.as_str()) {
        // Ctrl/Cmd+F in the document opens the bar (and the browser's own find is not opened over it).
        ("keydown", "root") if (p.ctrl || p.meta) && p.key_name.eq_ignore_ascii_case("f") => {
            let mut v = vec![Act::Prevent];
            if !open {
                v.push(quiet("View1FindOpen", "1"));
                v.push(ev("onFindOpened"));
            }
            v
        }
        ("keydown", "root") if open && p.key_name == "Escape" => close(),
        ("input", "find-input") => vec![quiet("View1SearchText", p.value.clone()), quiet("View1SearchCurrentMatch", if p.value.is_empty() { "0" } else { "1" })],
        ("keydown", "find-input") => match p.key_name.as_str() {
            "Enter" => {
                let mut v = vec![Act::Prevent];
                v.extend(step(!p.shift));
                v
            }
            "Escape" => {
                let mut v = vec![Act::Prevent];
                v.extend(close());
                v
            }
            _ => Vec::new(),
        },
        ("click", _) => match p.part.as_deref() {
            Some("find-next") => step(true),
            Some("find-prev") => step(false),
            Some("find-close") => close(),
            Some("find-case") => {
                let on = truthy(&cx.tree.get(i, "View1SearchCaseSensitive"));
                vec![quiet("View1SearchCaseSensitive", b01(!on)), quiet("View1SearchCurrentMatch", "1")]
            }
            _ => Vec::new(),
        },
        _ => Vec::new(),
    }
}

// ── the form ────────────────────────────────────────────────────────────────

/// A key pressed on the form (outside any control that takes it): Enter presses the default button.
pub fn form_key(tree: &FormTree, p: &Payload) -> Vec<(usize, Vec<Act>)> {
    let mut out = Vec::new();
    if p.kind != "keydown" {
        return out;
    }
    if p.key_name == "Enter" {
        let default = tree.ctls.iter().enumerate().find(|(_, c)| {
            matches!(c.control_type(), Some(ControlType::Button)) && c.flag("isdefault") && c.visible() && c.enabled()
        });
        if let Some((k, _)) = default {
            out.push((k, vec![ev("onClick")]));
        }
    }
    out
}

/// What a click on a notification's button does (spec 055): record which button, raise `onButtonClick`.
pub fn toast_button(button_id: &str, index: usize, dismiss: bool) -> Vec<Act> {
    let mut v = vec![quiet("LastButtonId", button_id.to_owned()), quiet("LastButtonIndex", (index + 1).to_string()), ev1("onButtonClick", button_id.to_owned())];
    if dismiss {
        v.push(ev("onClosing"));
        v.push(ev("onClosed"));
    }
    v
}

#[cfg(test)]
mod tests {
    use super::super::Empty;
    use super::*;
    use crate::tree::testing::{tree, tree_with};

    fn run(t: &FormTree, i: usize, e: &Empty, p: Payload) -> Vec<Act> {
        on_event(&e.cx(t), i, &p)
    }

    fn names(acts: &[Act]) -> Vec<String> {
        acts.iter()
            .map(|a| match a {
                Act::Event { name, args } if args.is_empty() => (*name).to_owned(),
                Act::Event { name, args } => format!("{name}({})", args.join("|")),
                Act::Quiet { prop, value } => format!("{prop}={value}"),
                Act::Change { prop, value } => format!("~{prop}={value}"),
                Act::Prevent => "prevent".into(),
                other => format!("{other:?}"),
            })
            .collect()
    }

    #[test]
    fn a_click_on_a_button_raises_onclick_and_a_double_click_both_spellings() {
        let t = tree();
        let e = Empty::default();
        let b = t.by_name("BTN-GO").unwrap();
        assert_eq!(names(&run(&t, b, &e, Payload::new("click", "root"))), ["onClick"]);
        assert_eq!(names(&run(&t, b, &e, Payload::new("dblclick", "root"))), ["onDblClick", "onDoubleClick"]);
        let mut key = Payload::new("keydown", "root");
        key.key_name = "a".into();
        assert_eq!(names(&run(&t, b, &e, key)), ["onKeyDown(a)", "onKeyPress(a)"]);
        let mut enter = Payload::new("keydown", "root");
        enter.key_name = "Enter".into();
        assert_eq!(names(&run(&t, b, &e, enter)), ["onKeyDown(Enter)", "onEnterPressed"]);
    }

    #[test]
    fn a_disabled_control_does_nothing_on_its_own() {
        let mut t = tree();
        let i = t.by_name("CHK-1").unwrap();
        t.set(i, "Enabled", "0");
        let e = Empty::default();
        let mut p = Payload::new("change", "input");
        p.checked = true;
        assert!(run(&t, i, &e, p).is_empty());
    }

    #[test]
    fn typing_in_a_text_box_is_one_change_and_nothing_else() {
        let t = tree();
        let e = Empty::default();
        let i = t.by_name("TXT-IN").unwrap();
        let mut p = Payload::new("input", "input");
        p.value = "héllo".into();
        assert_eq!(names(&run(&t, i, &e, p)), ["~Text=héllo"]);
    }

    #[test]
    fn a_check_box_stores_its_state_then_raises_the_events_in_the_desktops_order() {
        let t = tree();
        let e = Empty::default();
        let i = t.by_name("CHK-1").unwrap();
        let mut p = Payload::new("change", "input");
        p.checked = true;
        assert_eq!(names(&run(&t, i, &e, p.clone())), ["Checked=1", "onChange", "onCheckedChanged(true)", "onValueChanged", "onCheck"]);
        let mut off = Payload::new("change", "input");
        off.checked = false;
        assert!(run(&t, i, &e, off).is_empty(), "no change, no events");
        let mut click = Payload::new("click", "input");
        click.checked = true;
        assert_eq!(names(&run(&t, i, &e, click)), ["onClick"], "a click on the input is the control's click");
    }

    #[test]
    fn a_radio_stores_selected_and_a_switch_toggles() {
        let mut t = tree();
        let a = t.by_name("RAD-A").unwrap();
        let e = Empty::default();
        let mut p = Payload::new("change", "input");
        p.checked = true;
        assert!(names(&run(&t, a, &e, p)).contains(&"Selected=1".to_owned()));
        let (mut st, s) = tree_with("Switch", "SW", &[]);
        let acts = names(&run(&st, s, &e, Payload::new("click", "root")));
        assert_eq!(&acts[..2], ["Checked=1", "onChange"], "{acts:?}");
        st.set(s, "Checked", "1");
        let acts = names(&run(&st, s, &e, Payload::new("click", "root")));
        assert_eq!(acts[0], "Checked=0");
        assert!(acts.contains(&"onUncheck".to_owned()));
        let _ = &mut t;
    }

    #[test]
    fn a_click_on_a_list_row_selects_it_and_raises_the_selection_events() {
        let t = tree();
        let e = Empty::default();
        let l = t.by_name("LST-1").unwrap();
        let mut p = Payload::new("click", "root");
        p.index = Some(1);
        assert_eq!(names(&run(&t, l, &e, p)), ["Value=two", "SelectedIndex=2", "onChange", "onSelectedIndexChanged", "onClick"]);
        let mut d = Payload::new("dblclick", "root");
        d.index = Some(0);
        assert_eq!(names(&run(&t, l, &e, d)), ["onItemDoubleClick(one)", "onDblClick", "onDoubleClick"]);
    }

    #[test]
    fn a_multi_select_list_follows_ctrl_and_shift() {
        let (mut t, l) = tree_with("ListBox", "L", &[("Items", "a\nb\nc\nd"), ("MultiSelect", "1"), ("SelectedIndex", "1"), ("SelectedItems", "a")]);
        let e = Empty::default();
        let mut ctrl = Payload::new("click", "root");
        ctrl.index = Some(2);
        ctrl.ctrl = true;
        let acts = names(&run(&t, l, &e, ctrl));
        assert!(acts.contains(&"SelectedItems=a\nc".to_owned()), "{acts:?}");
        let mut shift = Payload::new("click", "root");
        shift.index = Some(3);
        shift.shift = true;
        let acts = names(&run(&t, l, &e, shift));
        assert!(acts.contains(&"SelectedItems=a\nb\nc\nd".to_owned()), "{acts:?}");
        t.set(l, "ShowCheckBoxes", "1");
        let mut tick = Payload::new("click", "root");
        tick.index = Some(1);
        tick.part = Some("check".into());
        assert_eq!(names(&run(&t, l, &e, tick))[..2], ["CheckedItems=b", "onItemChecked(b)"]);
    }

    #[test]
    fn the_arrow_keys_move_the_active_row_and_never_scroll_the_page() {
        let t = tree();
        let e = Empty::default();
        let l = t.by_name("LST-1").unwrap();
        let mut down = Payload::new("keydown", "root");
        down.key_name = "ArrowDown".into();
        let acts = names(&run(&t, l, &e, down));
        assert_eq!(acts[..3], ["prevent", "Value=one", "SelectedIndex=1"], "{acts:?}");
    }

    #[test]
    fn a_select_and_an_editable_combo_report_their_choice() {
        let (t, c) = tree_with("ComboBox", "C", &[("Items", "red\ngreen\nblue"), ("DropDownStyle", "DropDownList")]);
        let e = Empty::default();
        let mut p = Payload::new("change", "root");
        p.value = "2".into();
        assert_eq!(names(&run(&t, c, &e, p)), ["Text=blue", "Value=blue", "SelectedIndex=3", "onChange", "onSelectedIndexChanged"]);
        let (t, c) = tree_with("ComboBox", "C", &[("Items", "red\ngreen")]);
        let mut p = Payload::new("input", "input");
        p.value = "green".into();
        let acts = names(&run(&t, c, &e, p));
        assert_eq!(acts[0], "~Text=green");
        assert!(acts.contains(&"SelectedIndex=2".to_owned()));
        let mut free = Payload::new("input", "input");
        free.value = "mauve".into();
        assert!(names(&run(&t, c, &e, free)).contains(&"~Text=mauve".to_owned()));
    }

    #[test]
    fn a_tab_click_selects_the_page_and_reports_it() {
        let (t, c) = tree_with("TabControl", "TC", &[("Tabs", "One\nTwo\nThree")]);
        let e = Empty::default();
        let mut p = Payload::new("click", "tablist");
        p.index = Some(3);
        assert_eq!(names(&run(&t, c, &e, p)), ["SelectedTab=3", "onTabClick(3)", "onTabChanged(3)"]);
        let mut same = Payload::new("click", "tablist");
        same.index = Some(1);
        assert_eq!(names(&run(&t, c, &e, same)), ["onTabClick(1)"]);
        let mut key = Payload::new("keydown", "tablist");
        key.key_name = "ArrowRight".into();
        assert_eq!(names(&run(&t, c, &e, key))[..2], ["prevent", "SelectedTab=2"]);
    }

    #[test]
    fn a_tree_click_is_an_arrow_a_box_or_a_selection_by_where_it_lands() {
        let (t, c) = tree_with("TreeView", "TV", &[("Items", "A\n  B\nC"), ("CheckBoxes", "1")]);
        let e = Empty::default();
        let mut arrow = Payload::new("click", "root");
        arrow.index = Some(0);
        arrow.part = Some("arrow".into());
        assert_eq!(names(&run(&t, c, &e, arrow)), ["CollapsedNodes=A", "onNodeCollapse(A\t0\t0\t0)", "onClick"]);
        let mut chk = Payload::new("click", "root");
        chk.index = Some(1);
        chk.part = Some("check".into());
        assert_eq!(names(&run(&t, c, &e, chk))[..2], ["CheckedNodes=B", "onNodeCheck(B\t1\t1\t1)"]);
        let mut sel = Payload::new("click", "root");
        sel.index = Some(2);
        let acts = names(&run(&t, c, &e, sel));
        assert_eq!(acts[..3], ["SelectedNode=C", "onNodeClick(C\t2\t0\t0)", "onNodeSelect(C\t2\t0\t0)"]);
        let (t2, c2) = tree_with("TreeView", "TV", &[("Items", "A\n  B\nC"), ("SelectedNode", "C")]);
        let mut again = Payload::new("click", "root");
        again.index = Some(2);
        assert!(!names(&run(&t2, c2, &e, again)).iter().any(|n| n.starts_with("onNodeSelect")), "already selected: no select event");
    }

    #[test]
    fn a_grid_click_records_the_cell_and_a_header_click_sorts() {
        let (t, g) = tree_with("DataGrid", "G", &[("Columns", "A:string\nB:number"), ("Rows", "x\t1\ny\t2\nz\t3")]);
        let e = Empty::default();
        let mut cell = Payload::new("click", "root");
        cell.row = Some(1);
        cell.col = Some(1);
        assert_eq!(names(&run(&t, g, &e, cell)), ["ClickedRow=2", "ClickedColumn=2", "onSelectionChanged", "onRowSelect", "onCellClick(1,1)", "onClick"]);
        let mut head = Payload::new("click", "root");
        head.col = Some(1);
        let acts = run(&t, g, &e, head.clone());
        assert!(acts.contains(&Act::Ui(UiOp::GridSort { col: 1, descending: false })), "{acts:?}");
        let mut e2 = Empty::default();
        e2.ui.grid_sort.insert(g, (1, false));
        let acts = run(&t, g, &e2, head);
        assert!(acts.contains(&Act::Ui(UiOp::GridSort { col: 1, descending: true })), "a second click reverses");
    }

    #[test]
    fn scrolling_a_grid_reports_where_it_stands_and_does_nothing_when_it_has_not_moved() {
        let rows: String = (0..1000).map(|i| format!("r{i}")).collect::<Vec<_>>().join("\n");
        let (t, g) = tree_with("DataGrid", "G", &[("Columns", "A:string"), ("Rows", &rows)]);
        let e = Empty::default();
        let mut far = Payload::new("scroll", "scroller");
        far.scroll_top = 22.0 * 500.0;
        far.scroll_left = 40.0;
        far.client_height = 220.0;
        let acts = run(&t, g, &e, far.clone());
        assert!(acts.contains(&Act::Ui(UiOp::GridScroll { left: 40.0, top: 11000.0 })), "{acts:?}");
        assert_eq!(names(&acts).last().map(String::as_str), Some("onScroll(11000,40)"), "the desktop's value: the top, then the left");
        let mut again = Empty::default();
        again.ui.grid_scroll.insert(g, (40.0, 11000.0));
        assert!(run(&t, g, &again, far).is_empty(), "the page told us where it already is");
    }

    #[test]
    fn the_buttons_of_a_grid_ask_for_what_they_say() {
        let (t, g) = tree_with("DataGrid", "G", &[("Columns", "A:string\nB:string\nC:string"), ("Rows", "x\ty\tz"), ("ShowCSVExportButton", "1")]);
        let e = Empty::default();
        let mut csv = Payload::new("click", "root");
        csv.part = Some("csv".into());
        assert_eq!(names(&run(&t, g, &e, csv)), ["onExportCSV", "onClick"]);
        // The right-hand button of the first column moves it one place right: the advanced settings are written whole.
        let mut right = Payload::new("click", "root");
        right.part = Some("colmove".into());
        right.id = Some("right".into());
        right.col = Some(0);
        let acts = run(&t, g, &e, right);
        let written = acts.iter().find_map(|a| match a {
            Act::Quiet { prop, value } if prop == "AdvancedGrid" => Some(value.clone()),
            _ => None,
        });
        let json = written.expect("the settings with the columns in their new order");
        let titles: Vec<String> = serde_json::from_str::<serde_json::Value>(&json).unwrap()["columns"].as_array().unwrap().iter().map(|c| c["title"].as_str().unwrap().to_owned()).collect();
        assert_eq!(titles, ["B", "A", "C"]);
        // The left-hand button of the first column has nowhere to go.
        let mut left = Payload::new("click", "root");
        left.part = Some("colmove".into());
        left.id = Some("left".into());
        left.col = Some(0);
        assert!(!run(&t, g, &e, left).iter().any(|a| matches!(a, Act::Quiet { prop, .. } if prop == "AdvancedGrid")));
    }

    #[test]
    fn a_menu_leaf_raises_the_menu_events_and_a_group_opens() {
        use cobolt_forms::menu::{MenuDefinition, MenuItem};
        let (t, m) = tree_with("MenuBar", "MB", &[]);
        let mut e = Empty::default();
        let mut file = MenuItem::new_action("file", "File");
        let mut open = MenuItem::new_action("open", "Open");
        open.action = Some("open-form:Customers".into());
        file.items = vec![open];
        e.menus.insert("main-form", "MB", MenuDefinition { menu: vec![file], hash: String::new() });
        let mut group = Payload::new("click", "root");
        group.id = Some("file".into());
        assert_eq!(run(&t, m, &e, group.clone()), vec![Act::Ui(UiOp::SetOpenMenu(vec!["file".into()]))]);
        e.ui.open_menu = vec!["file".into()];
        assert_eq!(run(&t, m, &e, group), vec![Act::Ui(UiOp::SetOpenMenu(vec![]))], "clicking the open group closes it");
        let mut leaf = Payload::new("click", "root");
        leaf.id = Some("open".into());
        let acts = run(&t, m, &e, leaf);
        assert_eq!(names(&acts)[..3], ["SelectedItemId=open", "onMenuClick(open)", "onMenuItemClick(open)"]);
        assert!(acts.contains(&Act::Menu { item: "open".into() }));
        let item = item_of(&e.cx(&t), m, "open").unwrap();
        assert_eq!(menu_target(&item), MenuTarget::Embed { form: "Customers".into(), preserve_previous: false });
    }

    #[test]
    fn a_tool_bar_button_records_itself_and_a_platform_action_is_refused() {
        let def = r#"{"groups":[{"id":"g","buttons":[{"id":"save","label":"Save"},{"id":"print","label":"Print","action":"print:x.pdf"},{"id":"dlg","label":"Dialog","action":"open-modal:About"}]}]}"#;
        let (t, b) = tree_with("ToolBar", "TB", &[(cobolt_forms::toolbar::TOOLBAR_DEF_PROP, def)]);
        let e = Empty::default();
        let mut save = Payload::new("click", "root");
        save.id = Some("save".into());
        assert_eq!(names(&run(&t, b, &e, save)), ["LastButton=save", "onClick(save)"]);
        let mut print = Payload::new("click", "root");
        print.id = Some("print".into());
        assert!(matches!(run(&t, b, &e, print)[0], Act::Notice(_)));
        let mut dlg = Payload::new("click", "root");
        dlg.id = Some("dlg".into());
        assert!(run(&t, b, &e, dlg).contains(&Act::OpenModal { form: "About".into() }));
    }

    #[test]
    fn dragging_the_splitter_handle_sets_the_split_in_percent() {
        let (t, s) = tree_with("Splitter", "SP", &[]);
        let e = Empty::default();
        let mut p = Payload::new("pointermove", "handle");
        p.buttons = 1;
        p.x = 90.0;
        p.w = 300.0;
        assert_eq!(names(&run(&t, s, &e, p)), ["prevent", "SplitPosition=30"]);
        let mut idle = Payload::new("pointermove", "handle");
        idle.x = 90.0;
        idle.w = 300.0;
        assert!(run(&t, s, &e, idle).is_empty(), "a move without the button is not a drag");
    }

    #[test]
    fn a_knob_reads_its_value_from_the_angle_of_the_pointer() {
        assert_eq!(knob_value_at(100.0, 0.0, 200.0, 200.0, 0.0, 100.0, 1.0), 50.0, "straight up is the middle");
        assert_eq!(knob_value_at(200.0, 100.0, 200.0, 200.0, 0.0, 100.0, 1.0), 83.0, "three o'clock is 90 degrees: (90+135)/270");
        assert_eq!(knob_value_at(0.0, 200.0, 200.0, 200.0, 0.0, 100.0, 1.0), 0.0, "below the dial clamps to the minimum");
        let (t, k) = tree_with("Knob", "K", &[("Value", "10")]);
        let e = Empty::default();
        let mut p = Payload::new("pointerdown", "root");
        p.x = 100.0;
        p.y = 0.0;
        p.w = 200.0;
        p.h = 200.0;
        assert!(names(&run(&t, k, &e, p)).contains(&"~Value=50".to_owned()));
        let mut key = Payload::new("keydown", "root");
        key.key_name = "ArrowUp".into();
        assert_eq!(names(&run(&t, k, &e, key))[..2], ["prevent", "~Value=11"]);
    }

    #[test]
    fn a_map_drag_pans_and_the_zoom_buttons_zoom() {
        let (t, m) = tree_with("Maps", "M", &[("Width", "400"), ("Height", "300"), ("Zoom", "5")]);
        let mut e = Empty::default();
        let mut down = Payload::new("pointerdown", "root");
        down.x = 100.0;
        down.y = 100.0;
        assert!(matches!(run(&t, m, &e, down)[0], Act::Ui(UiOp::MapDrag { .. })));
        e.ui.map_drag.insert(m, (100.0, 100.0, 0.0, 0.0));
        let mut mv = Payload::new("pointermove", "root");
        mv.buttons = 1;
        mv.x = 160.0;
        mv.y = 100.0;
        let acts = names(&run(&t, m, &e, mv));
        assert_eq!(acts[0], "prevent");
        assert!(acts[2].starts_with("CenterLng=-"), "dragging right moves the centre west: {acts:?}");
        let mut zin = Payload::new("click", "root");
        zin.part = Some("zoom-in".into());
        assert!(names(&run(&t, m, &e, zin)).contains(&"Zoom=6".to_owned()));
    }

    #[test]
    fn a_clicked_marker_opens_its_card_and_a_second_click_closes_it() {
        let (t, m) = tree_with("Maps", "M", &[("Markers", "a\t0\t0\tAlpha\tinfo")]);
        let e = Empty::default();
        let mut click = Payload::new("click", "root");
        click.id = Some("a".into());
        assert_eq!(names(&run(&t, m, &e, click.clone()))[..2], ["SelectedMarkerId=a", "onMarkerClick(a)"]);
        let (t2, m2) = tree_with("Maps", "M", &[("Markers", "a\t0\t0\tAlpha\tinfo"), ("SelectedMarkerId", "a")]);
        assert_eq!(names(&run(&t2, m2, &e, click))[0], "SelectedMarkerId=");
    }

    #[test]
    fn dropped_files_are_taken_or_turned_away_by_extension_and_size() {
        let (t, z) = tree_with("FileDropZone", "DZ", &[("AllowedExtensions", "png, .jpg"), ("MaximumFileSizeKB", "10")]);
        let files = vec![("a.PNG".to_owned(), 5_000), ("b.gif".to_owned(), 100), ("c.jpg".to_owned(), 20_000), ("d".to_owned(), 1)];
        let (ok, no) = classify_files(&t, z, &files);
        assert_eq!(ok.iter().map(|f| f.0.as_str()).collect::<Vec<_>>(), ["a.PNG"]);
        assert_eq!(no.len(), 3);
        let e = Empty::default();
        let mut p = Payload::new("drop", "root");
        p.files = files;
        let acts = names(&run(&t, z, &e, p));
        assert!(acts.contains(&"DroppedFiles=a.PNG".to_owned()) && acts.contains(&"onFilesDropped(a.PNG)".to_owned()), "{acts:?}");
        assert!(acts.iter().any(|a| a.starts_with("onFilesRejected(")), "{acts:?}");
        assert_eq!(acts[0], "prevent");
    }

    #[test]
    fn a_web_link_click_hands_the_navigation_to_the_browser_and_refuses_a_script_address() {
        let (t, w) = {
            let mut t = crate::tree::testing::tree_of(r#"<Control id="LNK" type="WebLink" x="0" y="0" w="100" h="20"><Property name="Url">https://example.com/a</Property></Control>"#);
            let w = t.by_name("LNK").unwrap();
            t.set(w, "Target", "NewTab");
            (t, w)
        };
        let e = Empty::default();
        let acts = names(&run(&t, w, &e, Payload::new("click", "root")));
        assert_eq!(acts, ["onClick()"], "{acts:?}");
        let mut ctrl = Payload::new("click", "root");
        ctrl.ctrl = true;
        assert_eq!(names(&run(&t, w, &e, ctrl)), ["onClick(Ctrl)"]);
        let mut bad = t.clone();
        bad.set(w, "Url", "javascript:alert(1)");
        let acts = run(&bad, w, &e, Payload::new("click", "root"));
        assert_eq!(acts[0], Act::Prevent);
        assert!(matches!(acts[1], Act::Notice(_)), "{acts:?}");
    }

    #[test]
    fn enter_on_the_form_presses_the_default_button() {
        let mut t = tree();
        let b = t.by_name("BTN-GO").unwrap();
        t.set(b, "IsDefault", "1");
        let mut p = Payload::new("keydown", "root");
        p.key_name = "Enter".into();
        let r = form_key(&t, &p);
        assert_eq!(r.len(), 1);
        assert_eq!(r[0].0, b);
        t.set(b, "Enabled", "0");
        assert!(form_key(&t, &p).is_empty(), "a disabled default button is not pressed");
    }

    #[test]
    fn a_notification_button_records_which_button_before_it_raises_the_event() {
        assert_eq!(names(&toast_button("undo", 0, true)), ["LastButtonId=undo", "LastButtonIndex=1", "onButtonClick(undo)", "onClosing", "onClosed"]);
    }

    fn auth_tree() -> (FormTree, usize) {
        let (mut t, i) = tree_with("Authentication", "AUTH-1", &[]);
        t.init_auth(crate::auth::Flow::InApp, "", &crate::auth::Session::default());
        (t, i)
    }

    fn submit(fields: &[(&str, &str)]) -> Payload {
        let mut p = Payload::new("submit", "form-SignIn-0");
        p.fields = Fields(fields.iter().map(|(k, v)| ((*k).to_owned(), (*v).to_owned())).collect());
        p
    }

    #[test]
    fn submitting_each_screen_asks_the_machine_for_that_screens_action() {
        use crate::auth::{Action, Screen};
        let e = Empty::default();
        let cases: Vec<(Screen, Vec<(&str, &str)>, Action)> = vec![
            (Screen::SignIn, vec![("user", " ana "), ("password", " p ss ")], Action::SignIn { user: "ana".into(), password: " p ss ".into() }),
            (Screen::SignUp, vec![("user", "bia"), ("email", "b@x.y"), ("password", "pw")], Action::SignUp { user: "bia".into(), email: "b@x.y".into(), password: "pw".into() }),
            (Screen::Confirm, vec![("user", "bia"), ("code", " 123456 ")], Action::Confirm { user: "bia".into(), code: "123456".into() }),
            (Screen::Forgot, vec![("user", "ana")], Action::Forgot { user: "ana".into() }),
            (Screen::Reset, vec![("user", "ana"), ("code", "9"), ("new_password", "n w")], Action::Reset { user: "ana".into(), code: "9".into(), new_password: "n w".into() }),
            (Screen::Mfa, vec![("code", "000000")], Action::Mfa { code: "000000".into() }),
        ];
        for (screen, fields, want) in cases {
            let (mut t, i) = auth_tree();
            t.auth_mut(i).unwrap().machine.screen = screen;
            let acts = run(&t, i, &e, submit(&fields));
            assert_eq!(acts, vec![Act::Prevent, Act::Auth(want)], "{screen:?}: the user name is trimmed, a password is sent as typed");
        }
    }

    #[test]
    fn an_empty_required_field_is_never_sent_and_the_submit_is_still_cancelled() {
        let e = Empty::default();
        let (t, i) = auth_tree();
        assert_eq!(run(&t, i, &e, submit(&[("user", "ana"), ("password", "")])), vec![Act::Prevent]);
        assert_eq!(run(&t, i, &e, submit(&[])), vec![Act::Prevent]);
    }

    #[test]
    fn the_links_and_sign_out_ask_for_their_screen_or_action() {
        use crate::auth::{Action, Screen};
        let e = Empty::default();
        let (t, i) = auth_tree();
        let click = |id: &str| {
            let mut p = Payload::new("click", "nav-x");
            p.id = Some(id.to_owned());
            p
        };
        assert_eq!(run(&t, i, &e, click("show:SignUp")), vec![Act::Auth(Action::Show(Screen::SignUp))]);
        assert_eq!(run(&t, i, &e, click("show:Forgot")), vec![Act::Auth(Action::Show(Screen::Forgot))]);
        assert_eq!(run(&t, i, &e, click("show:SignIn")), vec![Act::Auth(Action::Show(Screen::SignIn))]);
        assert_eq!(run(&t, i, &e, click("signout")), vec![Act::Auth(Action::SignOut)]);
        assert_eq!(run(&t, i, &e, click("hosted")), vec![Act::Auth(Action::Hosted)]);
        assert!(run(&t, i, &e, click("show:Nowhere")).is_empty(), "an unknown id asks for nothing");
        assert!(run(&t, i, &e, click("javascript:alert(1)")).is_empty());
    }

    #[test]
    fn what_a_submit_carries_never_shows_in_a_debug_output() {
        use crate::auth::Action;
        let p = submit(&[("user", "ana"), ("password", "hunter2")]);
        let shown = format!("{p:?}");
        assert!(shown.contains("password") && !shown.contains("hunter2"), "R10: {shown}");
        let a = Action::SignIn { user: "ana".into(), password: "hunter2".into() };
        assert!(!format!("{a:?} {:?}", Act::Auth(a.clone())).contains("hunter2"));
    }

    #[test]
    fn a_picture_refuses_the_keystroke_that_would_break_it_and_the_box_goes_back_to_what_it_held() {
        let e = Empty::default();
        let (t, i) = tree_with("TextBox", "N-1", &[("Picture", "9(3)"), ("Text", "12")]);
        let typed = |v: &str| {
            let mut p = Payload::new("input", "input");
            p.value = v.to_owned();
            names(&run(&t, i, &e, p))
        };
        assert_eq!(typed("123"), ["~Text=123"], "a digit within the width is the user's edit");
        assert_eq!(typed("12a"), ["Text=12"], "a letter is refused: the stored text is written back over it");
        assert_eq!(typed("1234"), ["Text=12"], "one digit too many");
        assert_eq!(typed(""), ["~Text="], "emptying the box is allowed");
        // Letters only.
        let (t, i) = tree_with("TextBox", "A-1", &[("Picture", "A(3)"), ("Text", "ab")]);
        let mut p = Payload::new("input", "input");
        p.value = "ab1".into();
        assert_eq!(names(&run(&t, i, &e, p.clone())), ["Text=ab"]);
        p.value = "abc".into();
        assert_eq!(names(&run(&t, i, &e, p)), ["~Text=abc"]);
    }

    #[test]
    fn a_number_is_checked_as_a_shape_and_the_edited_display_follows_the_focus_and_the_sign_goes_to_its_place() {
        let e = Empty::default();
        let (t, i) = tree_with("TextBox", "N-2", &[("Picture", "ZZ9.99-"), ("Text", "-5.5")]);
        let key = |v: &str| {
            let mut p = Payload::new("input", "input");
            p.value = v.to_owned();
            names(&run(&t, i, &e, p))
        };
        assert_eq!(key("12."), ["~Text=12."], "an unfinished number can be typed");
        assert_eq!(key("12.345"), ["Text=-5.5"], "a third decimal does not fit");
        assert_eq!(key("1.2.3"), ["Text=-5.5"], "one decimal separator");
        // Focus shows the plain value; leaving shows the edited one and puts the sign where the picture does.
        let fin = Payload::new("focusin", "input");
        assert_eq!(run(&t, i, &e, fin), vec![Act::Ui(UiOp::Editing(true))]);
        let fout = Payload::new("focusout", "input");
        assert_eq!(run(&t, i, &e, fout), vec![Act::Ui(UiOp::Editing(false)), Act::Change { prop: "Text".into(), value: "5.5-".into() }]);
        // A character picture has no edited form: focus changes nothing for it.
        let (t, i) = tree_with("TextBox", "A-1", &[("Picture", "X(5)")]);
        assert!(run(&t, i, &e, Payload::new("focusin", "input")).is_empty());
    }

    #[test]
    fn the_find_bar_opens_with_ctrl_f_steps_round_and_closes_and_the_events_are_the_desktops() {
        let mut e = Empty::default();
        let (mut t, i) = tree_with("Viewer", "VW-1", &[]);
        // The generic key events of the document come along (a Viewer raises onKeyDown like any control): leave them out.
        let find_only = |v: Vec<String>| v.into_iter().filter(|n| !n.starts_with("onKey") && n != "onEscapePressed").collect::<Vec<_>>();
        // Ctrl+F (or Cmd+F) opens it; the browser's own find is not opened over it.
        let mut f = Payload::new("keydown", "root");
        f.key_name = "f".into();
        f.ctrl = true;
        assert_eq!(find_only(names(&run(&t, i, &e, f.clone()))), ["prevent", "View1FindOpen=1", "onFindOpened"]);
        t.set(i, "View1FindOpen", "1");
        assert_eq!(find_only(names(&run(&t, i, &e, f))), ["prevent"], "already open: nothing to raise");
        // Typing sets the needle and starts at the first match.
        let mut typed = Payload::new("input", "find-input");
        typed.value = "alpha".into();
        assert_eq!(names(&run(&t, i, &e, typed)), ["View1SearchText=alpha", "View1SearchCurrentMatch=1"]);
        // Enter steps forward and wraps; Shift+Enter steps back and wraps; with no match nothing happens.
        t.set(i, "View1SearchText", "alpha");
        e.ui.find_total.insert(i, 3);
        let step = |t: &crate::tree::FormTree, e: &Empty, cur: &str, shift: bool| {
            let mut t2 = t.clone();
            t2.set(i, "View1SearchCurrentMatch", cur);
            let mut p = Payload::new("keydown", "find-input");
            p.key_name = "Enter".into();
            p.shift = shift;
            names(&run(&t2, i, e, p))
        };
        assert_eq!(step(&t, &e, "1", false), ["prevent", "View1SearchCurrentMatch=2"]);
        assert_eq!(step(&t, &e, "3", false), ["prevent", "View1SearchCurrentMatch=1"], "wraps to the first");
        assert_eq!(step(&t, &e, "1", true), ["prevent", "View1SearchCurrentMatch=3"], "wraps to the last");
        assert_eq!(step(&t, &e, "3", true), ["prevent", "View1SearchCurrentMatch=2"]);
        e.ui.find_total.insert(i, 0);
        assert_eq!(step(&t, &e, "0", false), ["prevent"], "no match: Enter goes nowhere");
        // Escape in the box, Escape in the document and the Close button all close it and say so.
        let mut esc = Payload::new("keydown", "find-input");
        esc.key_name = "Escape".into();
        assert_eq!(names(&run(&t, i, &e, esc)), ["prevent", "View1FindOpen=0", "onFindClosed"]);
        let mut esc_doc = Payload::new("keydown", "root");
        esc_doc.key_name = "Escape".into();
        assert_eq!(find_only(names(&run(&t, i, &e, esc_doc))), ["View1FindOpen=0", "onFindClosed"]);
        let mut close = Payload::new("click", "close");
        close.part = Some("find-close".into());
        assert_eq!(names(&run(&t, i, &e, close)), ["View1FindOpen=0", "onFindClosed"]);
        // Match case toggles and starts over.
        let mut case = Payload::new("click", "case");
        case.part = Some("find-case".into());
        assert_eq!(names(&run(&t, i, &e, case)), ["View1SearchCaseSensitive=1", "View1SearchCurrentMatch=1"]);
        // Escape in a document whose bar is closed is the page's own key.
        t.set(i, "View1FindOpen", "0");
        let mut esc_doc = Payload::new("keydown", "root");
        esc_doc.key_name = "Escape".into();
        assert!(!find_only(names(&run(&t, i, &e, esc_doc))).contains(&"View1FindOpen=0".to_owned()));
    }
}
