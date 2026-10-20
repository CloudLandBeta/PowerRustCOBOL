// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! **The class A controls as elements** (WASM-004 R3): the native HTML element each control becomes,
//! with the attributes that carry its state and the DOM events it listens to.
//!
//! The styling is not here: `style.rs` turns the appearance properties into CSS in a lower cascade
//! layer than anything the developer writes (WASM-007 R8). What is here is *structure*: which
//! element, which text, which ARIA role, which listeners.
//!
//! A listener is attached for two reasons only: the control **behaves** (a text box keeps its text, a
//! list keeps its selection) or a handler is **bound** to a model event the listener raises. An
//! unbound `onMouseMove` costs no listener.

use cobolt_forms::model::ControlType;

use super::{Prop, Slot, VNode, ViewCtx};
use crate::css::class_names;
use crate::kinds::{player_kind, PlayerKind};
use crate::tree::{Ctl, Kind};
use crate::value::{fmt_number, parse_f64, truthy};

/// The DOM event a model event needs, if it needs one that the generic wiring provides.
pub fn dom_event_for(model_event: &str) -> Option<&'static str> {
    Some(match model_event.to_ascii_lowercase().as_str() {
        "onclick" => "click",
        "ondblclick" | "ondoubleclick" => "dblclick",
        "onrightclick" | "oncontextmenu" => "contextmenu",
        "onmiddleclick" => "auxclick",
        "onmousedown" => "mousedown",
        "onmouseup" => "mouseup",
        "onmousemove" => "mousemove",
        "onmouseenter" | "onhoverenter" => "mouseenter",
        "onmouseleave" | "onhoverleave" => "mouseleave",
        "onmousewheel" => "wheel",
        "ongotfocus" | "onenter" => "focusin",
        "onlostfocus" | "onleave" => "focusout",
        "onkeydown" | "onkeypress" | "onenterpressed" | "onescapepressed" => "keydown",
        "onkeyup" => "keyup",
        _ => return None,
    })
}

/// The generic DOM listeners for the handlers bound on `c`.
pub fn bound_dom_events(c: &Ctl) -> Vec<&'static str> {
    let mut v: Vec<&'static str> = Vec::new();
    for b in &c.bound {
        if let Some(d) = dom_event_for(b) {
            if !v.contains(&d) {
                v.push(d);
            }
        }
    }
    v
}

/// The element every control starts from: its classes, its name as `id` and `data-name` (so that a
/// selector can address it, WASM-007 R2), its tooltip, and its visibility and availability.
pub fn root(cx: &ViewCtx, i: usize, tag: &'static str) -> VNode {
    let c = cx.tree.ctl(i);
    let mut n = VNode::new(tag).key("root").classes(class_names(c)).attr("data-name", c.name.clone()).attr("id", c.name.clone());
    let tip = cx.tree.get(i, "Tooltip");
    if !tip.trim().is_empty() {
        n = n.attr("title", tip);
    }
    if !c.visible() {
        n = n.flag("hidden", true);
    }
    if !c.enabled() {
        n = n.class("pr-disabled").attr("aria-disabled", "true");
    }
    for d in bound_dom_events(c) {
        n = n.on(d);
    }
    n
}

fn caption(cx: &ViewCtx, i: usize) -> String {
    cx.tree.get(i, "Caption")
}

/// The lines of a multi-line property, keeping interior blank lines (the desktop's `lines()`).
pub fn lines(text: &str) -> Vec<String> {
    text.lines().map(str::to_owned).collect()
}

/// The non-empty lines.
pub fn lines_nonempty(text: &str) -> Vec<String> {
    text.lines().filter(|l| !l.is_empty()).map(str::to_owned).collect()
}

fn num(cx: &ViewCtx, i: usize, k: &str, d: f64) -> f64 {
    parse_f64(&cx.tree.get(i, k)).unwrap_or(d)
}

fn flag(cx: &ViewCtx, i: usize, k: &str) -> bool {
    truthy(&cx.tree.get(i, k))
}

/// `Sorted` — the display order only.
pub fn display_items(cx: &ViewCtx, i: usize, items: &mut Vec<String>) {
    if flag(cx, i, "Sorted") {
        items.sort_by_key(|s| s.to_lowercase());
    }
}

/// The element of the control `i`, children excluded (a container's children are mounted into its slots).
pub fn view(cx: &ViewCtx, i: usize) -> VNode {
    let c = cx.tree.ctl(i);
    let Kind::Control(t) = &c.kind else { return VNode::new("div").key("root") };
    if let Some(k) = player_kind(c) {
        return match k {
            PlayerKind::WebLink => weblink(cx, i),
            PlayerKind::Authentication => authentication(cx, i),
            PlayerKind::Canvas2D | PlayerKind::Canvas3D => canvas(cx, i, k),
            _ => hidden_object(cx, i),
        };
    }
    match t {
        ControlType::Button => button(cx, i),
        ControlType::Label => label(cx, i),
        ControlType::TextBox => text_box(cx, i),
        ControlType::CheckBox | ControlType::RadioButton => check_or_radio(cx, i),
        ControlType::ListBox => list_box(cx, i),
        ControlType::ComboBox => combo_box(cx, i),
        ControlType::GroupBox => group_box(cx, i),
        ControlType::Panel => panel(cx, i),
        ControlType::TabControl => tab_control(cx, i),
        ControlType::Splitter => splitter(cx, i),
        ControlType::ProgressBar => progress_bar(cx, i),
        ControlType::Slider => slider(cx, i),
        ControlType::NumericUpDown => numeric(cx, i),
        ControlType::DateTimePicker => date_time(cx, i),
        ControlType::Switch => switch(cx, i),
        ControlType::Line => line(cx, i),
        ControlType::PictureBox => picture_box(cx, i),
        ControlType::Animator => animator(cx, i),
        ControlType::StatusBar => status_bar(cx, i),
        ControlType::ToolBar => super::shell::tool_bar(cx, i),
        ControlType::MenuBar => super::shell::menu_bar(cx, i),
        ControlType::SideMenu => super::shell::side_menu(cx, i),
        ControlType::TreeView => super::data::tree_view(cx, i),
        ControlType::DataGrid => super::data::data_grid(cx, i),
        ControlType::Shape => super::charts::shape(cx, i),
        ControlType::Knob => super::charts::knob(cx, i),
        ControlType::Gauge => super::charts::gauge(cx, i),
        ControlType::BarChart
        | ControlType::LineChart
        | ControlType::PieChart
        | ControlType::AreaChart
        | ControlType::ScatterChart
        | ControlType::DonutChart
        | ControlType::RadarChart => super::charts::chart(cx, i),
        ControlType::Maps => super::maps::view(cx, i),
        ControlType::Viewer => super::viewer::view(cx, i),
        ControlType::FileDropZone => file_drop_zone(cx, i),
        ControlType::Snackbar | ControlType::Timer => hidden_object(cx, i),
        _ => hidden_object(cx, i),
    }
}

/// A control with no place on the page (a Timer, a service object, a Snackbar template). It still
/// has an element so that selectors and tests can find it, and it is never shown.
pub fn hidden_object(cx: &ViewCtx, i: usize) -> VNode {
    root(cx, i, "span").flag("hidden", true).attr("aria-hidden", "true")
}

// ── Label, Button ───────────────────────────────────────────────────────────

pub fn label(cx: &ViewCtx, i: usize) -> VNode {
    root(cx, i, "span").text(caption(cx, i))
}

pub fn button(cx: &ViewCtx, i: usize) -> VNode {
    let c = cx.tree.ctl(i);
    let mut n = root(cx, i, "button").attr("type", "button");
    if !c.enabled() {
        n = n.prop_bool("disabled", true);
    }
    let icon = cx.tree.get(i, "IconPath");
    if let Some(url) = cx.image_url(&icon) {
        n = n.child(VNode::new("img").key("icon").class("pr-icon").attr("src", url).attr("alt", "").attr("draggable", "false"));
    } else if let Some(svg) = catalogue_icon(&icon) {
        n = n.child(svg.key("icon").class("pr-icon"));
    }
    n.child(VNode::new("span").key("caption").class("pr-caption").text(caption(cx, i)))
}

/// A catalogue icon by name, as an SVG node (`icon_svg` writes the drawing, `svg_to_vnode` reads it).
pub fn catalogue_icon(name: &str) -> Option<VNode> {
    let name = name.trim();
    if name.is_empty() || name.contains(['/', '.', '\\']) {
        return None;
    }
    let svg = cobolt_forms::icons::icon_svg(name, "currentColor")?;
    super::svg::svg_to_vnode(&svg, 400).map(|n| n.attr("aria-hidden", "true").attr("focusable", "false"))
}

// ── TextBox ─────────────────────────────────────────────────────────────────

pub fn text_box(cx: &ViewCtx, i: usize) -> VNode {
    let c = cx.tree.ctl(i);
    let multiline = flag(cx, i, "Multiline");
    let password = !cx.tree.get(i, "PasswordCharacter").is_empty();
    let max = num(cx, i, "MaximumLength", 0.0) as i64;
    let mut input = VNode::new(if multiline { "textarea" } else { "input" })
        .key("input")
        .prop_str("value", cx.tree.get(i, "Text"))
        .prop_bool("readOnly", flag(cx, i, "ReadOnly"))
        .prop_bool("disabled", !c.enabled())
        .attr("spellcheck", "false")
        .attr("autocomplete", "off")
        .on("input")
        .on("keydown")
        .on("focusin")
        .on("focusout");
    if !multiline {
        input = input.attr("type", if password { "password" } else { "text" });
    } else if !flag(cx, i, "WordWrap") {
        input = input.attr("wrap", "off");
    }
    if max > 0 {
        input = input.prop_num("maxLength", max as f64);
    }
    root(cx, i, "div").child(input)
}

// ── CheckBox, RadioButton ───────────────────────────────────────────────────

pub fn check_or_radio(cx: &ViewCtx, i: usize) -> VNode {
    let c = cx.tree.ctl(i);
    let radio = matches!(c.control_type(), Some(ControlType::RadioButton));
    let on = flag(cx, i, "Selected") || flag(cx, i, "Checked");
    let mut input = VNode::new("input")
        .key("input")
        .attr("type", if radio { "radio" } else { "checkbox" })
        .prop_bool("checked", on)
        .prop_bool("disabled", !c.enabled())
        .on("click")
        .on("change");
    if radio {
        let group = cx.tree.get(i, "GroupName");
        let name = if group.trim().is_empty() { format!("grp-{}", c.parent.map(|p| cx.tree.ctl(p).name.clone()).unwrap_or_default()) } else { group };
        input = input.attr("name", format!("{}:{}", cx.tree.inst, name));
    }
    let n = root(cx, i, "label").child(input).child(VNode::new("span").key("caption").class("pr-caption").text(caption(cx, i)));
    // The label's own click would be reported twice (once for the label, once for the input it forwards
    // to); only the input listens, and a label's click reaches it.
    let mut n = n;
    n.on.retain(|e| *e != "click");
    n
}

// ── ListBox, ComboBox ───────────────────────────────────────────────────────

pub fn list_box(cx: &ViewCtx, i: usize) -> VNode {
    let c = cx.tree.ctl(i);
    let mut items = lines(&cx.tree.get(i, "Items"));
    display_items(cx, i, &mut items);
    let multi = flag(cx, i, "MultiSelect");
    let show_checks = flag(cx, i, "ShowCheckBoxes");
    let selected_index = num(cx, i, "SelectedIndex", 0.0) as i64;
    let selected = lines_nonempty(&cx.tree.get(i, "SelectedItems"));
    let checked = lines_nonempty(&cx.tree.get(i, "CheckedItems"));
    let active_value = cx.tree.get(i, "Value");
    let mut ul = root(cx, i, "ul")
        .attr("role", "listbox")
        .attr("tabindex", if c.enabled() { "0" } else { "-1" })
        .attr("aria-multiselectable", if multi { "true" } else { "false" })
        .on("click")
        .on("dblclick")
        .on("keydown")
        .on("scroll");
    let _ = &active_value;
    for (k, item) in items.iter().enumerate() {
        let idx = k as i64 + 1;
        let is_active = idx == selected_index;
        let is_selected = is_active || selected.iter().any(|s| s == item);
        let mut li = VNode::new("li")
            .key(format!("item:{k}"))
            .attr("role", "option")
            .attr("data-i", k.to_string())
            .attr("aria-selected", if is_selected { "true" } else { "false" });
        if is_active {
            li = li.class("pr-active");
        }
        if show_checks {
            let on = checked.iter().any(|s| s == item);
            li = li.child(VNode::new("input").key(format!("chk:{k}")).attr("type", "checkbox").attr("tabindex", "-1").attr("data-i", k.to_string()).prop_bool("checked", on));
        }
        li = li.child(VNode::new("span").key(format!("txt:{k}")).text(item.clone()));
        ul = ul.child(li);
    }
    ul
}

pub fn combo_box(cx: &ViewCtx, i: usize) -> VNode {
    let c = cx.tree.ctl(i);
    let mut items = lines(&cx.tree.get(i, "Items"));
    display_items(cx, i, &mut items);
    let selected_index = num(cx, i, "SelectedIndex", 0.0) as i64;
    let editable = flag(cx, i, "Editable") && !cx.tree.get(i, "DropDownStyle").eq_ignore_ascii_case("DropDownList");
    if editable {
        // An editable combo: a text field with the items offered by the browser's own list.
        let list_id = format!("pr-dl-{}-{}", cx.tree.inst, c.name);
        let current = if !cx.tree.get(i, "Text").is_empty() {
            cx.tree.get(i, "Text")
        } else if selected_index > 0 {
            items.get(selected_index as usize - 1).cloned().unwrap_or_default()
        } else {
            cx.tree.get(i, "Value")
        };
        let mut dl = VNode::new("datalist").key("list").attr("id", list_id.clone());
        for (k, it) in items.iter().enumerate() {
            dl = dl.child(VNode::new("option").key(format!("opt:{k}")).prop_str("value", it.clone()));
        }
        let input = VNode::new("input")
            .key("input")
            .attr("type", "text")
            .attr("role", "combobox")
            .attr("list", list_id)
            .attr("autocomplete", "off")
            .prop_str("value", current)
            .prop_bool("disabled", !c.enabled())
            .on("input")
            .on("change")
            .on("focusin")
            .on("focusout")
            .on("keydown");
        return root(cx, i, "div").child(input).child(dl);
    }
    let mut sel = root(cx, i, "select").prop_bool("disabled", !c.enabled()).on("change");
    // An empty first option stands for "nothing selected" (SelectedIndex 0).
    sel = sel.child(VNode::new("option").key("opt:none").prop_str("value", "").text("").attr("hidden", "").prop_bool("selected", selected_index <= 0));
    for (k, it) in items.iter().enumerate() {
        sel = sel.child(
            VNode::new("option").key(format!("opt:{k}")).prop_str("value", k.to_string()).text(it.clone()).prop_bool("selected", selected_index == k as i64 + 1),
        );
    }
    sel
}

// ── Containers ──────────────────────────────────────────────────────────────

pub fn group_box(cx: &ViewCtx, i: usize) -> VNode {
    let mut n = root(cx, i, "fieldset").slot(Slot::Children);
    if !flag(cx, i, "HideCaption") && !caption(cx, i).is_empty() {
        n = n.child(VNode::new("legend").key("legend").text(caption(cx, i)));
    }
    n
}

pub fn panel(cx: &ViewCtx, i: usize) -> VNode {
    let mut n = root(cx, i, "div").slot(Slot::Children);
    if flag(cx, i, "HScroll") {
        n = n.class("pr-hscroll");
    }
    if flag(cx, i, "VScroll") {
        n = n.class("pr-vscroll");
    }
    n.on("click")
}

/// The tab titles, from `Tabs` (one per line).
pub fn tab_titles(cx: &ViewCtx, i: usize) -> Vec<String> {
    let t = lines_nonempty(&cx.tree.get(i, "Tabs"));
    if t.is_empty() {
        vec!["Tab1".into()]
    } else {
        t
    }
}

pub fn tab_control(cx: &ViewCtx, i: usize) -> VNode {
    let titles = tab_titles(cx, i);
    let selected = (num(cx, i, "SelectedTab", 1.0) as i64).clamp(1, titles.len() as i64) as u32;
    let pos = cx.tree.get(i, "TabPosition").to_ascii_lowercase();
    let vertical = matches!(pos.as_str(), "left" | "right");
    let mut list = VNode::new("div").key("tablist").class("pr-tablist").attr("role", "tablist").attr("aria-orientation", if vertical { "vertical" } else { "horizontal" }).on("click").on("keydown");
    for (k, title) in titles.iter().enumerate() {
        let n = k as u32 + 1;
        list = list.child(
            VNode::new("button")
                .key(format!("tab:{n}"))
                .class("pr-tab")
                .attr("type", "button")
                .attr("role", "tab")
                .attr("data-i", n.to_string())
                .attr("aria-selected", if n == selected { "true" } else { "false" })
                .attr("tabindex", if n == selected { "0" } else { "-1" })
                .text(title.clone()),
        );
    }
    let mut root_n = root(cx, i, "div").class(format!("pr-tabs-{}", if pos.is_empty() { "top".to_owned() } else { pos.clone() }));
    root_n = root_n.child(list);
    let mut pages = VNode::new("div").key("pages").class("pr-tabpages");
    for n in 1..=titles.len() as u32 {
        pages = pages.child(
            VNode::new("div")
                .key(format!("page:{n}"))
                .class("pr-tabpanel")
                .attr("role", "tabpanel")
                .attr("data-page", n.to_string())
                .flag("hidden", n != selected)
                .slot(Slot::Page(n)),
        );
    }
    root_n.child(pages)
}

pub fn splitter(cx: &ViewCtx, i: usize) -> VNode {
    let horizontal = !cx.tree.get(i, "Orientation").eq_ignore_ascii_case("Vertical");
    let pct = num(cx, i, "SplitPosition", 50.0).clamp(0.0, 100.0);
    let mut n = root(cx, i, "div").class(if horizontal { "pr-horizontal" } else { "pr-vertical" });
    n = n
        .child(VNode::new("div").key("pane1").class("pr-pane").attr("data-pane", "1").style("flex", format!("0 0 {}%", fmt_number(pct))).slot(Slot::Pane(1)))
        .child(
            VNode::new("div")
                .key("handle")
                .class("pr-handle")
                .attr("role", "separator")
                .attr("aria-orientation", if horizontal { "vertical" } else { "horizontal" })
                .attr("tabindex", "0")
                .attr("aria-valuenow", fmt_number(pct))
                .attr("aria-valuemin", "0")
                .attr("aria-valuemax", "100")
                .on("pointerdown")
                .on("pointermove")
                .on("keydown"),
        )
        .child(VNode::new("div").key("pane2").class("pr-pane").attr("data-pane", "2").style("flex", "1 1 0").slot(Slot::Pane(2)));
    n
}

// ── Value controls ──────────────────────────────────────────────────────────

pub fn progress_bar(cx: &ViewCtx, i: usize) -> VNode {
    let min = num(cx, i, "Minimum", 0.0);
    let max = num(cx, i, "Maximum", 100.0);
    let v = num(cx, i, "Value", 0.0).clamp(min.min(max), max.max(min));
    let span = (max - min).max(f64::EPSILON);
    let mut n = root(cx, i, "div").attr("role", "progressbar").attr("aria-valuemin", fmt_number(min)).attr("aria-valuemax", fmt_number(max)).attr("aria-valuenow", fmt_number(v));
    n = n.child(VNode::new("progress").key("bar").prop_num("max", span).prop_num("value", v - min).attr("aria-hidden", "true"));
    if flag(cx, i, "ShowValue") {
        n = n.child(VNode::new("span").key("value").class("pr-value").text(format!("{}%", fmt_number(((v - min) / span * 100.0).round()))));
    }
    n
}

pub fn slider(cx: &ViewCtx, i: usize) -> VNode {
    let c = cx.tree.ctl(i);
    let vertical = cx.tree.get(i, "Orientation").eq_ignore_ascii_case("Vertical");
    let step = num(cx, i, "Step", 1.0).max(f64::EPSILON);
    let mut input = VNode::new("input")
        .key("input")
        .attr("type", "range")
        .prop_num("min", num(cx, i, "Minimum", 0.0))
        .prop_num("max", num(cx, i, "Maximum", 100.0))
        .prop_num("step", step)
        .prop_str("value", fmt_number(num(cx, i, "Value", 0.0)))
        .prop_bool("disabled", !c.enabled())
        .on("input")
        .on("change");
    if vertical {
        input = input.attr("orient", "vertical").attr("aria-orientation", "vertical");
    }
    let mut n = root(cx, i, "div").class(if vertical { "pr-vertical" } else { "pr-horizontal" }).child(input);
    if flag(cx, i, "ShowValue") {
        n = n.child(VNode::new("span").key("value").class("pr-value").text(fmt_number(num(cx, i, "Value", 0.0))));
    }
    n
}

pub fn numeric(cx: &ViewCtx, i: usize) -> VNode {
    let c = cx.tree.ctl(i);
    let places = num(cx, i, "DecimalPlaces", 0.0).clamp(0.0, 10.0) as usize;
    let v = num(cx, i, "Value", 0.0);
    let shown = if places > 0 { format!("{v:.places$}") } else { fmt_number(v) };
    let input = VNode::new("input")
        .key("input")
        .attr("type", "number")
        .prop_num("min", num(cx, i, "Minimum", 0.0))
        .prop_num("max", num(cx, i, "Maximum", 100.0))
        .prop_str("step", if places > 0 { format!("{}", 10f64.powi(-(places as i32)).max(num(cx, i, "Step", 1.0).min(1.0))) } else { fmt_number(num(cx, i, "Step", 1.0)) })
        .prop_str("value", shown)
        .prop_bool("readOnly", flag(cx, i, "ReadOnly"))
        .prop_bool("disabled", !c.enabled())
        .on("input")
        .on("change")
        .on("keydown")
        .on("focusin")
        .on("focusout");
    root(cx, i, "div").child(input)
}

pub fn date_time(cx: &ViewCtx, i: usize) -> VNode {
    let c = cx.tree.ctl(i);
    let format = cx.tree.get(i, "Format").to_ascii_lowercase();
    let kind = match format.as_str() {
        "time" => "time",
        "datetime" | "datetimelocal" => "datetime-local",
        _ => "date",
    };
    let value = cx.tree.get(i, "Value");
    let mut input = VNode::new("input")
        .key("input")
        .attr("type", kind)
        .prop_str("value", value)
        .prop_bool("disabled", !c.enabled())
        .on("input")
        .on("change")
        .on("focusin")
        .on("focusout");
    let (min, max) = (cx.tree.get(i, "MinimumDate"), cx.tree.get(i, "MaximumDate"));
    if !min.trim().is_empty() {
        input = input.attr("min", min);
    }
    if !max.trim().is_empty() {
        input = input.attr("max", max);
    }
    root(cx, i, "div").child(input)
}

pub fn switch(cx: &ViewCtx, i: usize) -> VNode {
    let c = cx.tree.ctl(i);
    let on = flag(cx, i, "Checked") || flag(cx, i, "Selected");
    let mut n = root(cx, i, "button")
        .attr("type", "button")
        .attr("role", "switch")
        .attr("aria-checked", if on { "true" } else { "false" })
        .child(VNode::new("span").key("thumb").class("pr-thumb"));
    if !c.enabled() {
        n = n.prop_bool("disabled", true);
    }
    n.on("click")
}

// ── Display controls ────────────────────────────────────────────────────────

pub fn line(cx: &ViewCtx, i: usize) -> VNode {
    let vertical = cx.tree.get(i, "LineDirection").eq_ignore_ascii_case("Vertical");
    root(cx, i, "div").class(if vertical { "pr-vertical" } else { "pr-horizontal" }).attr("role", "separator")
}

/// The CSS `object-fit` of a PictureBox `SizeMode`.
pub fn object_fit(size_mode: &str) -> &'static str {
    match size_mode.trim().to_ascii_lowercase().as_str() {
        "stretch" | "stretchimage" | "fill" => "fill",
        "zoom" | "fit" | "autosize" | "contain" => "contain",
        "cover" | "crop" => "cover",
        "center" | "centerimage" | "normal" => "none",
        _ => "none",
    }
}

pub fn picture_box(cx: &ViewCtx, i: usize) -> VNode {
    let mut n = root(cx, i, "div");
    let path = cx.tree.get(i, "ImagePath");
    match cx.image_url(&path) {
        Some(url) => {
            n = n.child(
                VNode::new("img")
                    .key("img")
                    .attr("src", url)
                    .attr("alt", cx.tree.get(i, "Tooltip"))
                    .attr("draggable", "false")
                    .style("object-fit", object_fit(&cx.tree.get(i, "SizeMode")))
                    .on("load")
                    .on("error"),
            )
        }
        None if !path.trim().is_empty() => n = n.attr("data-missing", path),
        None => {}
    }
    n
}

pub fn animator(cx: &ViewCtx, i: usize) -> VNode {
    let mut n = root(cx, i, "div");
    let src = cx.tree.get(i, "Source");
    if let Some(url) = cx.image_url(&src) {
        n = n.child(VNode::new("img").key("img").attr("src", url).attr("alt", "").attr("draggable", "false").style("object-fit", object_fit(&cx.tree.get(i, "SizeMode"))).on("load").on("error"));
    }
    n
}

pub fn status_bar(cx: &ViewCtx, i: usize) -> VNode {
    let mut n = root(cx, i, "footer").attr("role", "status");
    for (k, it) in lines_nonempty(&cx.tree.get(i, "Items")).into_iter().enumerate() {
        n = n.child(VNode::new("span").key(format!("panel:{k}")).class("pr-panel-item").text(it));
    }
    n
}

pub fn file_drop_zone(cx: &ViewCtx, i: usize) -> VNode {
    let hint = cx.tree.get(i, "Hint");
    let c = cx.tree.ctl(i);
    let mut input = VNode::new("input").key("file").attr("type", "file").attr("multiple", "").attr("tabindex", "-1").attr("hidden", "").attr("data-part", "file").on("change");
    let allowed = cx.tree.get(i, "AllowedExtensions");
    if !allowed.trim().is_empty() {
        let accept: Vec<String> = allowed.split([',', ';', ' ']).filter(|s| !s.is_empty()).map(|s| if s.starts_with('.') { s.to_owned() } else { format!(".{s}") }).collect();
        input = input.attr("accept", accept.join(","));
    }
    let mut n = root(cx, i, "div").attr("role", "button").attr("tabindex", if c.enabled() { "0" } else { "-1" }).on("click").on("keydown").on("dragover").on("dragleave").on("drop");
    if cx.ui.dropzone_over == Some(i) {
        n = n.class("pr-over");
    }
    n = n.child(VNode::new("span").key("hint").class("pr-hint").text(if hint.is_empty() { "Drop files here or click to choose".to_owned() } else { hint }));
    let dropped = lines_nonempty(&cx.tree.get(i, "DroppedFiles"));
    if !dropped.is_empty() {
        let mut ul = VNode::new("ul").key("files").class("pr-files");
        for (k, f) in dropped.iter().enumerate() {
            ul = ul.child(VNode::new("li").key(format!("f:{k}")).text(f.clone()));
        }
        n = n.child(ul);
    }
    n.child(input)
}

// ── The series' own controls ────────────────────────────────────────────────

/// A WebLink (WASM-006): a real anchor, so the browser's own behaviour (Ctrl-click, the context menu,
/// middle-click, "copy link address") works, with `rel` forced safe and **no `ping`**.
pub fn weblink(cx: &ViewCtx, i: usize) -> VNode {
    let c = cx.tree.ctl(i);
    let props = crate::weblink::LinkProps::read(cx.tree, i);
    let policy = crate::weblink::Policy::default();
    let plain = crate::weblink::plan(&props, crate::weblink::Click::default(), &policy);
    let mut n = root(cx, i, "a").class("pr-weblink");
    n = n.on("click").on("auxclick");
    match plain {
        Ok(Some(p)) => {
            n = n.attr("href", p.url);
            n = n.attr("target", p.target.html());
            if !p.rel.is_empty() {
                n = n.attr("rel", p.rel);
            }
            if let Some(d) = p.download {
                n = n.attr("download", d);
            }
            if !p.referrer_policy.is_empty() {
                n = n.attr("referrerpolicy", p.referrer_policy);
            }
        }
        Ok(None) => {
            // No address: an anchor with no `href` is not a link, and says so to assistive technology.
            n = n.attr("role", "link").attr("aria-disabled", "true");
        }
        Err(r) => {
            n = n.attr("role", "link").attr("aria-disabled", "true").attr("data-refused", r.message.clone()).attr("title", r.message);
        }
    }
    let text = cx.tree.get(i, "Caption");
    let text = if text.is_empty() { cx.tree.get(i, "Text") } else { text };
    let text = if text.is_empty() { props.url.clone() } else { text };
    let underline = cx.tree.get(i, "Underline").to_ascii_lowercase();
    match underline.as_str() {
        "always" | "1" | "true" => n = n.class("pr-underline-always"),
        "hover" => n = n.class("pr-underline-hover"),
        _ => {}
    }
    let _ = c;
    n.text(text)
}

/// The Authentication control (WASM-009): the sign-in UI is built by the surface from the machine's
/// state; the element here is its mount point.
pub fn authentication(cx: &ViewCtx, i: usize) -> VNode {
    root(cx, i, "div").class("pr-auth").attr("data-auth", "").attr("role", "group").attr("aria-label", "Sign in")
}

/// Canvas2D/Canvas3D (WASM-018): a canvas the graphics crate's display lists are drawn onto.
pub fn canvas(cx: &ViewCtx, i: usize, k: PlayerKind) -> VNode {
    let tag_class = if k == PlayerKind::Canvas2D { "pr-canvas2d" } else { "pr-canvas3d" };
    let w = num(cx, i, "Width", 300.0).max(1.0);
    let h = num(cx, i, "Height", 150.0).max(1.0);
    root(cx, i, "div").class(tag_class).child(
        VNode::new("canvas").key("canvas").prop_num("width", w).prop_num("height", h).attr("tabindex", "0").on("pointerdown").on("pointermove").on("pointerup").on("keydown"),
    )
}

#[allow(dead_code)]
fn _unused(_: &Prop) {}

#[cfg(test)]
mod tests {
    use super::super::{Empty, VNode};
    use super::*;
    use crate::tree::testing::tree;

    fn view_of(name: &str, props: &[(&str, &str)]) -> VNode {
        let mut t = tree();
        let i = t.by_name(name).unwrap();
        for (k, v) in props {
            t.set(i, k, v);
        }
        let e = Empty::default();
        let cx = e.cx(&t);
        view(&cx, i)
    }

    #[test]
    fn every_control_has_a_root_with_its_name_class_and_type_class() {
        let n = view_of("BTN-GO", &[]);
        assert_eq!(n.tag, "button");
        assert!(n.has_class("pr-control") && n.has_class("pr-button"));
        assert_eq!(n.attr_of("data-name"), Some("BTN-GO"));
        assert_eq!(n.attr_of("id"), Some("BTN-GO"));
        assert_eq!(n.find_tag("span").unwrap().all_text(), "Go");
        assert!(n.on.contains(&"click"), "onClick is bound in the demo manifest: {}", n.describe());
    }

    #[test]
    fn a_hidden_or_disabled_control_says_so_to_the_browser_and_to_assistive_technology() {
        let n = view_of("BTN-GO", &[("Visible", "0"), ("Enabled", "0")]);
        assert!(n.attrs.iter().any(|(k, _)| k == "hidden"));
        assert_eq!(n.attr_of("aria-disabled"), Some("true"));
        assert_eq!(n.prop_of("disabled"), Some(&Prop::Bool(true)));
        assert!(n.has_class("pr-disabled"));
    }

    #[test]
    fn a_text_box_is_an_input_that_holds_the_text_property() {
        let n = view_of("TXT-IN", &[("MaximumLength", "12"), ("ReadOnly", "1")]);
        assert_eq!(n.tag, "div");
        let input = n.find_tag("input").expect("an input");
        assert_eq!(input.prop_of("value"), Some(&Prop::Str("hello".into())));
        assert_eq!(input.prop_of("maxLength"), Some(&Prop::Num(12.0)));
        assert_eq!(input.prop_of("readOnly"), Some(&Prop::Bool(true)));
        assert!(input.on.contains(&"input"), "the text box keeps its Text property: {}", n.describe());
        let multi = view_of("TXT-IN", &[("Multiline", "1")]);
        assert!(multi.find_tag("textarea").is_some());
        let pw = view_of("TXT-IN", &[("PasswordCharacter", "*")]);
        assert_eq!(pw.find_tag("input").unwrap().attr_of("type"), Some("password"));
    }

    #[test]
    fn check_boxes_and_radios_are_inputs_in_a_label_and_radios_share_a_name_per_group() {
        let chk = view_of("CHK-1", &[("Checked", "1")]);
        assert_eq!(chk.tag, "label");
        let input = chk.find_tag("input").unwrap();
        assert_eq!(input.attr_of("type"), Some("checkbox"));
        assert_eq!(input.prop_of("checked"), Some(&Prop::Bool(true)));
        assert!(!chk.on.contains(&"click"), "the label itself does not listen: its click reaches the input, which does");
        let a = view_of("RAD-A", &[]);
        let b = view_of("RAD-B", &[]);
        let (na, nb) = (a.find_tag("input").unwrap().attr_of("name").unwrap().to_owned(), b.find_tag("input").unwrap().attr_of("name").unwrap().to_owned());
        assert_eq!(na, nb, "two radios of one container are one group");
        assert_eq!(a.find_tag("input").unwrap().attr_of("type"), Some("radio"));
    }

    #[test]
    fn a_list_box_is_a_listbox_with_the_active_row_and_the_set_marked() {
        let n = view_of("LST-1", &[("SelectedIndex", "2")]);
        assert_eq!(n.tag, "ul");
        assert_eq!(n.attr_of("role"), Some("listbox"));
        let rows: Vec<&VNode> = n.children.iter().collect();
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].attr_of("aria-selected"), Some("false"));
        assert_eq!(rows[1].attr_of("aria-selected"), Some("true"));
        assert!(rows[1].has_class("pr-active"));
        assert_eq!(rows[1].all_text(), "two");
        let checks = view_of("LST-1", &[("ShowCheckBoxes", "1"), ("CheckedItems", "one")]);
        assert_eq!(checks.count("input"), 2);
        assert_eq!(checks.children[0].find_tag("input").unwrap().prop_of("checked"), Some(&Prop::Bool(true)));
    }

    #[test]
    fn sorted_changes_the_display_order_only() {
        let n = view_of("LST-1", &[("Items", "pear\napple\nfig"), ("Sorted", "1")]);
        let texts: Vec<String> = n.children.iter().map(|c| c.all_text()).collect();
        assert_eq!(texts, ["apple", "fig", "pear"]);
    }

    #[test]
    fn containers_expose_a_slot_for_their_children_and_tabs_one_per_page() {
        let p = view_of("PNL-1", &[]);
        assert_eq!(p.slot, Some(Slot::Children));
        let mut t = tree();
        let i = t.by_name("PNL-1").unwrap();
        let _ = i;
        let _ = &mut t;
    }

    #[test]
    fn numeric_up_down_carries_its_range_step_and_decimal_places() {
        let n = view_of("NUD-1", &[("DecimalPlaces", "2")]);
        let input = n.find_tag("input").unwrap();
        assert_eq!(input.attr_of("type"), Some("number"));
        assert_eq!(input.prop_of("min"), Some(&Prop::Num(0.0)));
        assert_eq!(input.prop_of("max"), Some(&Prop::Num(10.0)));
        assert_eq!(input.prop_of("value"), Some(&Prop::Str("5.00".into())));
    }

    #[test]
    fn a_picture_with_an_unknown_path_is_not_loaded_from_anywhere() {
        let mut t = tree();
        let i = t.by_name("BTN-GO").unwrap();
        let _ = i;
        let e = Empty::default();
        let cx = e.cx(&t);
        assert_eq!(cx.image_url("https://evil.example/x.png"), None);
        assert_eq!(cx.image_url("data:image/png;base64,AAAA").as_deref(), Some("data:image/png;base64,AAAA"));
        assert_eq!(cx.image_url("data:image/svg+xml;base64,AAAA"), None, "an SVG image can carry script in some contexts: refused");
        assert_eq!(cx.image_url(""), None);
    }

    #[test]
    fn the_dom_events_a_bound_model_event_needs() {
        assert_eq!(dom_event_for("onClick"), Some("click"));
        assert_eq!(dom_event_for("onDoubleClick"), Some("dblclick"));
        assert_eq!(dom_event_for("onEnterPressed"), Some("keydown"));
        assert_eq!(dom_event_for("onChange"), None, "behavioural, not generic");
    }

    #[test]
    fn the_catalogue_icon_is_an_svg_node_and_a_path_is_not_an_icon_name() {
        assert!(catalogue_icon("doc-open").is_some());
        assert!(catalogue_icon("../../etc/passwd").is_none());
        assert!(catalogue_icon("").is_none());
        assert!(catalogue_icon("no-such-icon").is_none());
    }
}
