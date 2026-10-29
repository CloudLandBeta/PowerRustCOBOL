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

use super::{Slot, VNode, ViewCtx};
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
    // A control of a layer (spec 091; WASM-004 R13–R24): named, and while the layer is hidden neither
    // drawn nor reachable by focus or the pointer (`inert`, D20). A container's contents go with it.
    if let Some(l) = c.layer {
        n = n.attr("data-layer", cx.tree.layers[l].name.clone());
        if !cx.tree.layers[l].visible {
            n = n.flag("hidden", true).flag("inert", true);
        }
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
    with_tab_order(cx, i, element_of(cx, i))
}

/// The first element of `n` (itself included) the browser can focus: a native one, or one that carries a `tabindex`.
fn first_focusable_mut(n: &mut VNode) -> Option<&mut VNode> {
    let native = matches!(n.tag, "button" | "select" | "textarea" | "a") || (n.tag == "input" && n.attr_of("type") != Some("hidden"));
    let out_of_order = n.attr_of("tabindex") == Some("-1") || n.attrs.iter().any(|(k, _)| k == "hidden");
    if !out_of_order && (native || n.attr_of("tabindex").is_some()) {
        return Some(n);
    }
    n.children.iter_mut().find_map(first_focusable_mut)
}

/// The form's **tab order** is the browser's sequential focus order (WASM-004 R10): a control the
/// desktop numbers `TabOrder = n` gets `tabindex="n"` on the element that takes the focus, so Tab
/// walks the controls in the order the designer set — which the document order does not say once the
/// developer has moved a control in the order. A control with no number (0) keeps the document order, a
/// disabled or hidden one keeps `-1`, and only the types that are tab stops take part.
fn with_tab_order(cx: &ViewCtx, i: usize, mut n: VNode) -> VNode {
    let c = cx.tree.ctl(i);
    let Some(t) = c.control_type() else { return n };
    if !t.is_tab_stop() {
        return n;
    }
    let order = parse_f64(&cx.tree.get(i, "TabOrder")).map(|v| v as i64).unwrap_or(0);
    if order > 0 {
        if let Some(focus) = first_focusable_mut(&mut n) {
            focus.attrs.retain(|(k, _)| k != "tabindex");
            focus.attrs.push(("tabindex".into(), order.to_string()));
        }
    }
    n
}

fn element_of(cx: &ViewCtx, i: usize) -> VNode {
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
        ControlType::DataGrid => super::datagrid::data_grid(cx, i),
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
    // An explicit PICTURE (WASM-004): its width bounds the box, a number gets the keypad a number needs, and
    // a numeric-edited one shows the edited form at rest and the plain value to edit while it has the focus.
    let pic = cx.tree.picture_of(i);
    let raw = cx.tree.get(i, "Text");
    let shown = match &pic {
        Some(p) => p.display(&raw, cx.ui.focused == Some(i), cx.tree.decimal_comma, cx.tree.currency),
        None => raw,
    };
    let mut input = VNode::new(if multiline { "textarea" } else { "input" })
        .key("input")
        .prop_str("value", shown)
        .prop_bool("readOnly", flag(cx, i, "ReadOnly"))
        .prop_bool("disabled", !c.enabled())
        .attr("spellcheck", "false")
        .attr("autocomplete", "off")
        .on("input")
        .on("keydown")
        .on("focusin")
        .on("focusout");
    // The hint an empty box shows (the painter draws it in the text's own colour at 55 %).
    let hint = cx.tree.get(i, "HintText");
    if !hint.trim().is_empty() {
        input = input.attr("placeholder", hint);
    }
    if !multiline {
        input = input.attr("type", if password { "password" } else { "text" });
    } else if !flag(cx, i, "WordWrap") {
        input = input.attr("wrap", "off");
    }
    match &pic {
        Some(p) => {
            // The raw text is at most `max_raw_len` long; the edited display at rest may be longer (grouping
            // and currency), which the browser's maxlength does not stop (it only limits typing).
            input = input.prop_num("maxLength", p.max_raw_len().max(1) as f64);
            if let cobolt_forms::picture::PicShape::Number { frac_digits, signed, .. } = p.shape() {
                input = input.attr("inputmode", if *frac_digits > 0 || *signed { "decimal" } else { "numeric" });
            }
        }
        None if max > 0 => input = input.prop_num("maxLength", max as f64),
        None => {}
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

/// What a ListBox draws, from the painter's own arithmetic (`render.rs`, the ListBox arm): rows `FontSize x 1.35 + 4` tall in a scroll area
/// 3 px inside the frame, the text 5 px in from the highlight (and a tick box before it, when the list has them), a pill-shaped handle
/// on the right while the rows do not fit.
pub fn list_box(cx: &ViewCtx, i: usize) -> VNode {
    use crate::value::parse_color;
    let c = cx.tree.ctl(i);
    let mut items = lines(&cx.tree.get(i, "Items"));
    display_items(cx, i, &mut items);
    let multi = flag(cx, i, "MultiSelect");
    let show_checks = flag(cx, i, "ShowCheckBoxes");
    let selected_index = num(cx, i, "SelectedIndex", 0.0) as i64;
    let selected = lines_nonempty(&cx.tree.get(i, "SelectedItems"));
    let checked = lines_nonempty(&cx.tree.get(i, "CheckedItems"));
    let (w, h) = cx.box_size(i);
    let font = cx.tree.font_size_of(i);
    // The row is laid out from the size the layout asks for (14 when none is named), the text is drawn at the size the painter's item font takes (11).
    let row_h = font.unwrap_or(14.0).clamp(4.0, 200.0) * 1.35 + 4.0;
    let size = font.unwrap_or(11.0).clamp(4.0, 200.0);
    let bw = c.raw("borderwidth").and_then(crate::value::parse_f64).unwrap_or(1.0).max(0.0);
    let ink = c.raw("foregroundcolor").and_then(parse_color).filter(|_| true).unwrap_or([255, 255, 255, 255]);
    // The highlights: the developer's, else the form host's selection colour and that colour a little under half lit.
    let active_fill = c.raw("activeitemcolor").and_then(parse_color).unwrap_or([144, 209, 255, 255]);
    let selected_fill = c.raw("selecteditemscolor").and_then(parse_color).unwrap_or([active_fill[0], active_fill[1], active_fill[2], (active_fill[3] as f64 * 0.45).round() as u8]);
    let active_ink = c.raw("activeitemtextcolor").and_then(parse_color).unwrap_or(ink);
    let content_w = (w - 6.0).max(0.0);
    let content_h = (h - 6.0).max(0.0);
    let text_x = bw + 4.0;
    let nudge = crate::metrics::baseline_nudge(size);
    let mut ul = VNode::new("ul")
        .key("scroller")
        .class("pr-lb-scroll")
        .attr("role", "listbox")
        .attr("tabindex", if c.enabled() { "0" } else { "-1" })
        .attr("aria-multiselectable", if multi { "true" } else { "false" })
        .style("left", format!("{}px", fmt_number(3.0 - bw)))
        .style("top", format!("{}px", fmt_number(3.0 - bw)))
        .style("width", format!("{}px", fmt_number(content_w)))
        .style("height", format!("{}px", fmt_number(content_h)))
        .on("click")
        .on("dblclick")
        .on("keydown")
        .on("scroll");
    for (k, item) in items.iter().enumerate() {
        let idx = k as i64 + 1;
        let is_active = idx == selected_index;
        let is_selected = !is_active && selected.iter().any(|s| s == item);
        let mut li = VNode::new("li")
            .key(format!("item:{k}"))
            .attr("role", "option")
            .attr("data-i", k.to_string())
            .attr("aria-selected", if is_active || is_selected { "true" } else { "false" })
            .style("height", format!("{}px", fmt_number(row_h)))
            .style("padding-left", format!("{}px", fmt_number(text_x)));
        if is_active {
            li = li.class("pr-active").style("background-color", crate::value::css_color(active_fill));
        } else if is_selected {
            li = li.style("background-color", crate::value::css_color(selected_fill));
        }
        if show_checks {
            let d = (row_h - 4.0).clamp(9.0, 18.0);
            let on = checked.iter().any(|s| s == item);
            let mut b = VNode::new("span")
                .key(format!("chk:{k}"))
                .class("pr-lb-box")
                .attr("data-part", "check")
                .attr("data-i", k.to_string())
                .attr("role", "checkbox")
                .attr("aria-checked", if on { "true" } else { "false" })
                .style("width", format!("{}px", fmt_number(d)))
                .style("height", format!("{}px", fmt_number(d)))
                .style("border-color", crate::value::css_color(ink))
                .style("border-radius", format!("{}px", (d * 0.22).floor()))
                .style("margin-right", "6px");
            if on {
                b = b.style("background-color", crate::value::css_color(active_fill)).child(
                    VNode::svg("svg")
                        .key("tick")
                        .attr("viewBox", "0 0 1 1")
                        .attr("width", format!("{}", fmt_number(d)))
                        .attr("height", format!("{}", fmt_number(d)))
                        .child(VNode::svg("path").attr("d", "M0.22 0.52L0.42 0.74L0.80 0.26").attr("fill", "none").attr("stroke", crate::value::css_color(ink)).attr("stroke-width", fmt_number((d * 0.14).max(1.2) / d)).attr("stroke-linecap", "round")),
                );
            }
            li = li.child(b);
        }
        let text_ink = if is_active { active_ink } else { ink };
        li = li.child(
            VNode::new("span")
                .key(format!("txt:{k}"))
                .class("pr-lb-text")
                .text(item.clone())
                .attr("data-i", k.to_string())
                .style("color", crate::value::css_color(text_ink))
                .style("font-size", format!("{}px", fmt_number(size)))
                .style("line-height", format!("{}px", fmt_number(crate::metrics::row_height(size))))
                .style("top", format!("{}px", fmt_number(nudge)))
                .style("-webkit-text-stroke", format!("{}px", fmt_number(crate::metrics::stroke(size, text_ink)))),
        );
        ul = ul.child(li);
    }
    let mut n = root(cx, i, "div").class("pr-lb").child(ul);
    // The handle: as long as the part of the rows in view, put where the scroll offset puts it.
    let total = items.len() as f64 * row_h;
    if total > content_h + 0.5 && content_h > 0.0 {
        let max_offset = total - content_h;
        let offset = cx.ui.grid_scroll.get(&i).map(|s| s.1).unwrap_or(0.0).clamp(0.0, max_offset);
        let size_h = (content_h / total * content_h).max(12.0);
        let top = 3.0 + offset / max_offset * (content_h - size_h);
        n = n.child(
            VNode::new("div")
                .key("handle")
                .class("pr-lb-handle")
                .style("left", format!("{}px", fmt_number(w - 3.0 - 5.0 - bw)))
                .style("top", format!("{}px", fmt_number(top - bw)))
                .style("width", "5px")
                .style("height", format!("{}px", fmt_number(size_h))),
        );
    }
    n
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
    // The list-only style: the desktop's closed header (`glass_combo_header`) — the value at the left and a small triangle at
    // the right — is drawn, and the native <select> stays over it, clear, to take the click that opens its list.
    let size = cx.tree.font_size_of(i).unwrap_or_else(|| num(cx, i, "FontSize", 11.0)).max(1.0);
    let (w, h) = cx.box_size(i);
    // What the header shows: the value, else the item at `SelectedIndex`, else (a combo nothing was chosen in) the first item.
    let value = cx.tree.get(i, "Value");
    let current = if !value.is_empty() {
        value
    } else if selected_index >= 1 {
        items.get(selected_index as usize - 1).cloned().unwrap_or_default()
    } else {
        String::new()
    };
    let shown = if current.is_empty() { items.first().cloned().unwrap_or_default() } else { current };
    let at = items.iter().position(|it| *it == shown);
    let ink = c.raw("foregroundcolor").and_then(crate::value::parse_color).unwrap_or([220, 228, 255, 255]);
    let mut sel = VNode::new("select").key("select").class("pr-combo-select").prop_bool("disabled", !c.enabled()).on("change");
    // An empty first option stands for "nothing selected" (SelectedIndex 0).
    sel = sel.child(VNode::new("option").key("opt:none").prop_str("value", "").text("").attr("hidden", "").prop_bool("selected", at.is_none()));
    for (k, it) in items.iter().enumerate() {
        sel = sel.child(VNode::new("option").key(format!("opt:{k}")).prop_str("value", k.to_string()).text(it.clone()).prop_bool("selected", at == Some(k)));
    }
    // Places are the painter's, measured from the frame's outer edge; an element is placed from inside the border.
    let bw = crate::css::border_width(c);
    let top = crate::metrics::text_top(h, size) - bw;
    let text = VNode::new("span")
        .key("t")
        .class("pr-combo-text")
        .text(shown)
        .style("left", format!("{}px", fmt_number(8.0 - bw)))
        .style("top", format!("{}px", fmt_number(top)))
        .style("font-size", format!("{}px", fmt_number(size)))
        .style("line-height", format!("{}px", fmt_number(crate::metrics::row_height(size))))
        .style("-webkit-text-stroke", format!("{}px", fmt_number(crate::metrics::stroke(size, ink))));
    // The triangle the painter sets at 13 px in from the right edge, centred on the middle of the frame (a 9-pixel glyph of the
    // fallback face, measured: 5.6 wide at the top, 5.7 high), at three quarters of the value's ink.
    let (cx0, cy0) = (w - 13.0, h / 2.0);
    let ax = |dx: f64, dy: f64| format!("{},{}", fmt_number(cx0 + dx - (w - 16.0)), fmt_number(cy0 + dy - (h / 2.0 - 4.0)));
    let arrow = VNode::svg("svg")
        .key("arrow")
        .class("pr-combo-arrow")
        .attr("width", "16")
        .attr("height", "9")
        .attr("viewBox", "0 0 16 9")
        .attr("aria-hidden", "true")
        .style("left", format!("{}px", fmt_number(w - 16.0 - bw)))
        .style("top", format!("{}px", fmt_number(h / 2.0 - 4.0 - bw)))
        .child(VNode::svg("polygon").attr("points", format!("{} {} {}", ax(-2.36, -3.07), ax(3.28, -3.07), ax(0.46, 2.7))).attr("fill", crate::value::css_color([ink[0], ink[1], ink[2], (ink[3] as f64 * 0.75).round() as u8])));
    root(cx, i, "div").class("pr-combobox").child(sel).child(text).child(arrow)
}

// ── Containers ──────────────────────────────────────────────────────────────

pub fn group_box(cx: &ViewCtx, i: usize) -> VNode {
    let mut n = root(cx, i, "fieldset").slot(Slot::Children);
    if !flag(cx, i, "HideCaption") && !caption(cx, i).is_empty() {
        n = n.child(VNode::new("legend").key("legend").text(caption(cx, i)));
    }
    with_expander(cx, i, n)
}

pub fn panel(cx: &ViewCtx, i: usize) -> VNode {
    let mut n = root(cx, i, "div").slot(Slot::Children);
    if flag(cx, i, "HScroll") {
        n = n.class("pr-hscroll");
    }
    if flag(cx, i, "VScroll") {
        n = n.class("pr-vscroll");
    }
    with_expander(cx, i, n).on("click")
}

/// The painter strokes the icon 1.6 points wide and its anti-aliasing feathers that out; the page's 1.9 carries the same ink.
/// The side of the expand/collapse icon's square, and how far the card's corner pushes it in (`paint::EXPAND_ICON_SIZE`, `expand_icon_rect`).
const EXPAND_ICON: f64 = 24.0;

/// Spec 090: a card that is `Expandable` carries the standard expand icon in its top-right corner — two arrows pointing apart, or together while
/// the card is expanded — which a click turns into the card's `Expanded`. Drawn only while the form is laid out, as the painter draws it
/// (a form that is not laid out would answer the click with nothing).
fn with_expander(cx: &ViewCtx, i: usize, n: VNode) -> VNode {
    let c = cx.tree.ctl(i);
    // The icon is there when the form is laid out; a control of a layer (spec 091 R21), and so everything inside it, is not.
    if !c.flag("expandable") || !cx.tree.ctl(0).flag("responsive") || cx.tree.layer_of(i).is_some() {
        return n;
    }
    let (w, h) = (num(cx, i, "Width", 0.0), num(cx, i, "Height", 0.0));
    if w < EXPAND_ICON * 2.0 || h < EXPAND_ICON * 2.0 {
        return n;
    }
    let expanded = flag(cx, i, "Expanded");
    let (cx_, cy_) = (EXPAND_ICON / 2.0, EXPAND_ICON / 2.0);
    // The diagonal runs from the bottom-left to the top-right of the icon; each arrow has two legs along the axes.
    let (reach, arm) = (7.0, 5.5);
    let mut d = String::new();
    let mut seg = |x1: f64, y1: f64, x2: f64, y2: f64| d.push_str(&format!("M{} {}L{} {}", fmt_number(x1), fmt_number(y1), fmt_number(x2), fmt_number(y2)));
    let head = |seg: &mut dyn FnMut(f64, f64, f64, f64), tx: f64, ty: f64, toward: f64| {
        let back = -toward;
        seg(tx, ty, tx + back * arm, ty);
        seg(tx, ty, tx, ty - back * arm);
    };
    if expanded {
        let (ax, ay, bx, by) = (cx_ + 1.5, cy_ - 1.5, cx_ - 1.5, cy_ + 1.5);
        head(&mut seg, ax, ay, -1.0);
        head(&mut seg, bx, by, 1.0);
        seg(cx_ + reach, cy_ - reach, ax, ay);
        seg(cx_ - reach, cy_ + reach, bx, by);
    } else {
        let (ax, ay, bx, by) = (cx_ + reach, cy_ - reach, cx_ - reach, cy_ + reach);
        head(&mut seg, ax, ay, 1.0);
        head(&mut seg, bx, by, -1.0);
        seg(ax, ay, bx, by);
    }
    let icon = VNode::svg("svg")
        .key("glyph")
        .attr("viewBox", format!("0 0 {0} {0}", fmt_number(EXPAND_ICON)))
        .attr("aria-hidden", "true")
        .child(VNode::svg("path").attr("d", d).attr("fill", "none").attr("stroke", "currentColor").attr("stroke-width", "1.9"));
    let button = VNode::new("button")
        .key("expander")
        .class("pr-expander")
        .attr("type", "button")
        .attr("data-part", "expander")
        .attr("aria-label", if expanded { "Collapse" } else { "Expand" })
        .attr("aria-expanded", if expanded { "true" } else { "false" })
        .child(icon);
    n.child(button).on("click")
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
    // `data-pr-slide`: the DOM layer wires the strip's drag and wheel once, when the element is made (`dom/tabslide.rs`).
    let mut list = VNode::new("div")
        .key("tablist")
        .class("pr-tablist")
        .attr("role", "tablist")
        .attr("aria-orientation", if vertical { "vertical" } else { "horizontal" })
        .attr("data-pr-slide", "1")
        .on("click")
        .on("keydown");
    for (k, title) in titles.iter().enumerate() {
        let n = k as u32 + 1;
        let mut tab = VNode::new("button")
            .key(format!("tab:{n}"))
            .class("pr-tab")
            .attr("type", "button")
            .attr("role", "tab")
            .attr("data-i", n.to_string())
            .attr("aria-selected", if n == selected { "true" } else { "false" })
            .attr("tabindex", if n == selected { "0" } else { "-1" })
            .text(title.clone());
        // The strip scrolls when the tabs do not all fit. The selected tab says so, and the DOM layer scrolls the
        // strip (never the page) to it when that attribute LANDS on a tab: the moment a tab becomes the selected one,
        // and not on a redraw that leaves it selected, so a strip the operator has scrolled stays where it was left.
        if n == selected {
            tab = tab.attr("data-pr-reveal", "1");
        }
        list = list.child(tab);
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

/// A model control carrying the properties of tree control `i` that a shared formatter reads (`cobolt_forms::fieldtext`).
fn model_for(cx: &ViewCtx, i: usize, keys: &[&str]) -> cobolt_forms::model::Control {
    let c = cx.tree.ctl(i);
    let t = c.control_type().cloned().unwrap_or(ControlType::TextBox);
    let mut m = cobolt_forms::model::Control::new(c.name.clone(), t, 0, 0);
    for k in keys {
        if let Some(v) = c.raw(k) {
            m.set_prop(*k, v.to_owned());
        }
    }
    m
}

/// What a field shows when it is not being edited, drawn over the native input that does the editing: the desktop's
/// text for the value (its formats, not the browser's locale's), centred in the frame, as the painter centres it.
fn face(text_nodes: Vec<VNode>) -> VNode {
    VNode::new("span").key("face").class("pr-field-face").attr("aria-hidden", "true").children(text_nodes)
}

/// The outline of 📅 and of 🕐 in `NotoEmoji-Regular` (the glyphs the painter's caption uses), in 1/1000 of the font's em, y down, on the baseline.
const CALENDAR_PATH: &str = "M1199.7 -752.4Q1211.9 -752.4 1220.2 -743.9Q1228.5 -735.4 1228.5 -723.6L1228.5 38.6Q1228.5 50.3 1220.2 58.6Q1211.9 66.9 1199.7 66.9L118.7 66.9Q106.9 66.9 98.4 58.6Q89.8 50.3 89.8 38.6L89.8 -723.6Q89.8 -735.4 98.4 -743.9Q106.9 -752.4 118.7 -752.4L412.1 -752.4L642.6 -914.6Q649.4 -919.4 659.2 -919.4Q668.9 -919.4 675.8 -914.6L906.2 -752.4L1199.7 -752.4ZM1170.9 -694.3L147.5 -694.3L147.5 9.3L945.8 9.3L945.8 -111.3Q945.8 -123.0 954.1 -131.3Q962.4 -139.6 974.1 -139.6L987.8 -140.1Q1064.0 -143.1 1099.9 -154.3Q1135.7 -165.5 1170.9 -197.8L1170.9 -694.3ZM512.2 -752.4L806.2 -752.4L659.2 -856.0L512.2 -752.4ZM698.7 -633.3L698.7 -556.2L669.4 -556.2L669.4 -566.4L648.4 -566.4L648.4 -556.2L619.6 -556.2L619.6 -633.3L648.4 -633.3L648.4 -622.1L669.4 -622.1L669.4 -633.3L698.7 -633.3ZM1003.9 -7.3Q1043.9 -30.8 1068.4 -52.2Q1092.8 -73.7 1111.8 -97.7Q1092.8 -92.8 1058.8 -88.6Q1024.9 -84.5 1003.9 -83.5L1003.9 -7.3ZM1170.9 -79.6Q1152.8 -56.2 1132.6 -33.7Q1112.3 -11.2 1083.5 9.3L1170.9 9.3L1170.9 -79.6ZM572.8 -478.5L493.7 -478.5L493.7 -508.3L572.8 -508.3L572.8 -478.5ZM698.7 -478.5L619.6 -478.5L619.6 -508.3L698.7 -508.3L698.7 -478.5ZM824.7 -478.5L745.6 -478.5L745.6 -508.3L824.7 -508.3L824.7 -478.5ZM320.8 -478.5L241.7 -478.5L241.7 -508.3L320.8 -508.3L320.8 -478.5ZM950.7 -478.5L871.6 -478.5L871.6 -508.3L950.7 -508.3L950.7 -478.5ZM1076.7 -478.5L997.6 -478.5L997.6 -508.3L1076.7 -508.3L1076.7 -478.5ZM446.8 -478.5L367.7 -478.5L367.7 -508.3L446.8 -508.3L446.8 -478.5ZM1057.1 -373.0L1017.1 -373.0L1017.1 -412.1L1057.1 -412.1L1057.1 -373.0ZM805.2 -373.0L765.1 -373.0L765.1 -412.1L805.2 -412.1L805.2 -373.0ZM931.2 -373.0L891.1 -373.0L891.1 -412.1L931.2 -412.1L931.2 -373.0ZM553.2 -277.8L513.2 -277.8L513.2 -316.4L553.2 -316.4L553.2 -277.8ZM679.2 -277.8L639.2 -277.8L639.2 -316.4L679.2 -316.4L679.2 -277.8ZM805.2 -277.8L765.1 -277.8L765.1 -316.4L805.2 -316.4L805.2 -277.8ZM931.2 -277.8L891.1 -277.8L891.1 -316.4L931.2 -316.4L931.2 -277.8ZM1057.1 -277.8L1017.1 -277.8L1017.1 -316.4L1057.1 -316.4L1057.1 -277.8ZM300.8 -277.8L261.2 -277.8L261.2 -316.4L300.8 -316.4L300.8 -277.8ZM426.8 -277.8L387.2 -277.8L387.2 -316.4L426.8 -316.4L426.8 -277.8ZM553.2 -87.4L513.2 -87.4L513.2 -125.5L553.2 -125.5L553.2 -87.4ZM679.2 -182.6L639.2 -182.6L639.2 -220.7L679.2 -220.7L679.2 -182.6ZM931.2 -182.6L891.1 -182.6L891.1 -220.7L931.2 -220.7L931.2 -182.6ZM1057.1 -182.6L1017.1 -182.6L1017.1 -220.7L1057.1 -220.7L1057.1 -182.6ZM805.2 -182.6L765.1 -182.6L765.1 -220.7L805.2 -220.7L805.2 -182.6ZM679.2 -87.4L639.2 -87.4L639.2 -125.5L679.2 -125.5L679.2 -87.4ZM553.2 -182.6L513.2 -182.6L513.2 -220.7L553.2 -220.7L553.2 -182.6ZM300.8 -87.4L261.2 -87.4L261.2 -125.5L300.8 -125.5L300.8 -87.4ZM426.8 -182.6L387.2 -182.6L387.2 -220.7L426.8 -220.7L426.8 -182.6ZM300.8 -182.6L261.2 -182.6L261.2 -220.7L300.8 -220.7L300.8 -182.6ZM426.8 -87.4L387.2 -87.4L387.2 -125.5L426.8 -125.5L426.8 -87.4Z";
const CLOCK_PATH: &str = "M1101.3 -88.4Q1028.8 36.6 903.6 109.1Q778.3 181.6 634.3 181.6Q490.2 181.6 365.2 109.1Q240.2 36.6 167.7 -88.4Q95.2 -213.4 95.2 -357.4Q95.2 -501.5 167.7 -626.7Q240.2 -752.0 365.2 -824.5Q490.2 -897.0 634.3 -897.0Q778.3 -897.0 903.6 -824.5Q1028.8 -752.0 1101.3 -626.7Q1173.8 -501.5 1173.8 -357.4Q1173.8 -213.4 1101.3 -88.4ZM1056.4 -601.3Q990.7 -713.9 878.2 -779.5Q765.6 -845.2 634.3 -845.2Q503.4 -845.2 390.9 -779.8Q278.3 -714.4 212.6 -601.6Q147.0 -488.8 147.0 -357.4Q147.0 -227.1 212.4 -114.5Q277.8 -2.0 390.4 64.0Q502.9 129.9 634.3 129.9Q765.6 129.9 878.2 64.5Q990.7 -1.0 1056.4 -113.5Q1122.1 -226.1 1122.1 -357.4Q1122.1 -488.8 1056.4 -601.3ZM808.6 -581.5L704.6 -400.9Q710.4 -391.1 713.9 -380.4Q717.3 -369.6 717.3 -357.4Q717.3 -323.7 692.6 -299.3Q668.0 -274.9 634.3 -274.9Q600.6 -274.9 576.2 -299.3Q551.8 -323.7 551.8 -357.4Q551.8 -380.9 563.7 -399.9Q575.7 -418.9 595.2 -430.2L595.2 -802.2L672.9 -802.2L672.9 -502.4L741.2 -620.1L808.6 -581.5ZM655.3 -334.2Q665.5 -341.8 665.5 -357.4Q665.5 -370.1 656.2 -379.2Q647.0 -388.2 634.3 -388.2Q621.6 -388.2 612.5 -379.2Q603.5 -370.1 603.5 -357.4Q603.5 -347.2 611.8 -336.9Q620.1 -326.7 634.3 -326.7Q645.0 -326.7 655.3 -334.2ZM1077.9 -335.7Q1068.8 -326.7 1055.7 -326.7Q1043.0 -326.7 1033.9 -335.7Q1024.9 -344.7 1024.9 -357.4Q1024.9 -370.1 1033.9 -379.2Q1043.0 -388.2 1055.7 -388.2Q1068.8 -388.2 1077.9 -379.2Q1086.9 -370.1 1086.9 -357.4Q1086.9 -344.7 1077.9 -335.7ZM978.0 -545.7Q968.8 -554.2 968.8 -567.9Q968.8 -579.6 976.8 -589.6Q984.9 -599.6 998.5 -599.6Q1011.2 -599.6 1020.8 -591.1Q1030.3 -582.5 1030.3 -568.8Q1030.3 -555.7 1021.2 -546.4Q1012.2 -537.1 999.5 -537.1Q987.3 -537.1 978.0 -545.7ZM866.7 -699.7Q856.9 -690.9 845.7 -690.9Q831.5 -690.9 822.8 -700.9Q814.0 -710.9 814.0 -722.7Q814.0 -736.3 823.7 -744.9Q833.5 -753.4 844.7 -753.4Q857.4 -753.4 866.9 -744.9Q876.5 -736.3 876.5 -722.7Q876.5 -708.5 866.7 -699.7ZM445.3 -745.1Q455.1 -736.8 455.1 -722.7Q455.1 -710.4 446.3 -700.7Q437.5 -690.9 423.3 -690.9Q412.1 -690.9 402.3 -699.7Q392.6 -708.5 392.6 -722.7Q392.6 -735.4 401.6 -744.4Q410.6 -753.4 423.3 -753.4Q435.5 -753.4 445.3 -745.1ZM290.3 -545.4Q280.3 -537.1 269.0 -537.1Q255.4 -537.1 246.8 -546.9Q238.3 -556.6 238.3 -567.9Q238.3 -581.1 247.6 -590.3Q256.8 -599.6 269.5 -599.6Q283.2 -599.6 291.7 -590.1Q300.3 -580.6 300.3 -567.9Q300.3 -553.7 290.3 -545.4ZM234.9 -335.7Q225.6 -326.7 212.9 -326.7Q199.7 -326.7 190.9 -335.7Q182.1 -344.7 182.1 -357.4Q182.1 -370.1 190.9 -379.2Q199.7 -388.2 212.9 -388.2Q225.6 -388.2 234.9 -379.2Q244.1 -370.1 244.1 -357.4Q244.1 -344.7 234.9 -335.7ZM290.5 -168.9Q300.3 -160.6 300.3 -146.5Q300.3 -133.8 291.7 -124.8Q283.2 -115.7 269.0 -115.7Q256.8 -115.7 247.6 -124.8Q238.3 -133.8 238.3 -147.5Q238.3 -159.2 246.3 -168.2Q254.4 -177.2 268.1 -177.2Q280.8 -177.2 290.5 -168.9ZM445.3 30.3Q435.5 38.6 423.3 38.6Q409.7 38.6 401.1 29.1Q392.6 19.5 392.6 7.8Q392.6 -6.3 402.3 -14.9Q412.1 -23.4 423.3 -23.4Q437.0 -23.4 446.0 -14.6Q455.1 -5.9 455.1 7.8Q455.1 22.0 445.3 30.3ZM656.2 86.4Q647.0 95.7 634.3 95.7Q621.6 95.7 612.5 86.4Q603.5 77.1 603.5 64.0Q603.5 51.3 612.3 42.2Q621.1 33.2 634.3 33.2Q647.0 33.2 656.2 42.2Q665.5 51.3 665.5 64.0Q665.5 77.1 656.2 86.4ZM866.7 -14.9Q876.5 -6.3 876.5 7.8Q876.5 19.5 867.7 29.1Q858.9 38.6 844.7 38.6Q833.5 38.6 823.7 30.0Q814.0 21.5 814.0 7.8Q814.0 -5.4 822.8 -14.4Q831.5 -23.4 844.7 -23.4Q856.9 -23.4 866.7 -14.9ZM1020.5 -123.8Q1010.7 -115.7 999.5 -115.7Q985.8 -115.7 977.3 -125.5Q968.8 -135.3 968.8 -146.5Q968.8 -160.2 978.0 -168.7Q987.3 -177.2 1000.0 -177.2Q1015.6 -177.2 1022.9 -167.0Q1030.3 -156.7 1030.3 -146.0Q1030.3 -131.8 1020.5 -123.8Z";

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
    let text = cobolt_forms::fieldtext::numeric_updown_text(&model_for(cx, i, &["Value", "DecimalPlaces", "Step", "ThousandsSeparator"]));
    root(cx, i, "div").child(face(vec![VNode::new("span").key("t").text(text)])).child(input)
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
    // What the field shows: the value laid out by the field's own format, behind a calendar (or a clock, for a time) — "📅 01/01/1970",
    // the desktop's caption, with the glyph drawn from the font the painter takes it from. The native input stays over it, clear, to take the click
    // that opens the picker and what is typed.
    let m = model_for(cx, i, &["Value", "Format", "CustomFormat", "ShowUpDown"]);
    let parts = cobolt_forms::fieldtext::dt_parts(&m);
    let shown = cobolt_forms::fieldtext::display_dt_for(&m, &cx.tree.get(i, "Value"));
    let custom = format.trim().eq_ignore_ascii_case("custom");
    let pattern = cx.tree.get(i, "CustomFormat").trim().to_owned();
    let hint = if custom && !pattern.is_empty() { pattern } else if parts.date && parts.time { "DD/MM/YYYY HH:MM".to_owned() } else if parts.time { "HH:MM".to_owned() } else { "DD/MM/YYYY".to_owned() };
    let text = if shown.is_empty() { hint } else { shown };
    let mut nodes = Vec::new();
    if !flag(cx, i, "ShowUpDown") {
        nodes.push(field_glyph(parts.date));
        nodes.push(VNode::new("span").key("t").text(format!(" {text}")));
    } else {
        nodes.push(VNode::new("span").key("t").text(text));
    }
    root(cx, i, "div").child(face(nodes)).child(input)
}

/// The calendar (`date`) or the clock the painter's text face puts before a date-time picker's text: the glyph of the font it
/// falls back to (`NotoEmoji-Regular`, scaled to 0.81 of the text and set 0.24 of that lower), drawn as an outline, so it looks as it does there.
fn field_glyph(date: bool) -> VNode {
    // In 1/1000 em of the glyph's own font, y down, origin on the baseline; the viewBox is the glyph's extent after the scale and the drop.
    let (path, view) = if date { (CALENDAR_PATH, "0 -679 1270 986") } else { (CLOCK_PATH, "0 -657 1270 1079") };
    VNode::svg("svg")
        .key("glyph")
        .class(if date { "pr-glyph pr-glyph-date" } else { "pr-glyph pr-glyph-time" })
        .attr("viewBox", view)
        .attr("aria-hidden", "true")
        .child(VNode::svg("path").attr("d", path).attr("fill", "currentColor"))
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
    let plain = crate::weblink::plan(&props, crate::weblink::Click::default(), &cx.tree.policy);
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

/// The Authentication control (WASM-009): its screens are in [`super::auth`].
pub fn authentication(cx: &ViewCtx, i: usize) -> VNode {
    super::auth::view(cx, i)
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

#[cfg(test)]
mod tests {
    use super::super::{Empty, Prop, VNode};
    use super::*;
    use crate::tree::testing::{tree, tree_with};

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
        assert_eq!(n.tag, "div");
        let list = n.find_tag("ul").expect("the scroll area of the rows");
        assert_eq!(list.attr_of("role"), Some("listbox"));
        let rows: Vec<&VNode> = list.children.iter().collect();
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].attr_of("aria-selected"), Some("false"));
        assert_eq!(rows[1].attr_of("aria-selected"), Some("true"));
        assert!(rows[1].has_class("pr-active"));
        assert_eq!(rows[1].all_text(), "two");
        // The painter's rows: FontSize (14 when none) x 1.35 + 4 tall.
        assert_eq!(rows[0].styles.iter().find(|(k, _)| k == "height").map(|(_, v)| v.as_str()), Some("22.9px"));
        let checks = view_of("LST-1", &[("ShowCheckBoxes", "1"), ("CheckedItems", "one")]);
        let boxes: Vec<&VNode> = {
            let mut v = Vec::new();
            checks.find_all(&|x| x.attr_of("data-part") == Some("check"), &mut v);
            v
        };
        assert_eq!(boxes.len(), 2);
        assert_eq!(boxes[0].attr_of("aria-checked"), Some("true"));
        assert_eq!(boxes[1].attr_of("aria-checked"), Some("false"));
    }

    #[test]
    fn sorted_changes_the_display_order_only() {
        let n = view_of("LST-1", &[("Items", "pear\napple\nfig"), ("Sorted", "1")]);
        let texts: Vec<String> = n.find_tag("ul").unwrap().children.iter().map(|c| c.all_text()).collect();
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

    /// The selected tab, and only it, asks the DOM layer to bring it into view; it moves when the selection does.
    #[test]
    fn only_the_selected_tab_asks_to_be_brought_into_view() {
        let names: Vec<String> = (1..=40).map(|k| format!("Sheet {k:02}")).collect();
        let tabs = names.join("\n");
        let mut moved = 0;
        for selected in [1, 2, 17, 40] {
            let (t, c) = tree_with("TabControl", "TC", &[("Tabs", &tabs), ("SelectedTab", &selected.to_string())]);
            let e = Empty::default();
            let cx = e.cx(&t);
            let strip = tab_control(&cx, c);
            let mut buttons: Vec<&VNode> = Vec::new();
            strip.find_all(&|n| n.has_class("pr-tab"), &mut buttons);
            assert_eq!(buttons.len(), 40, "one button per tab, drawn or not: the browser scrolls the strip");
            let revealing: Vec<usize> = buttons
                .iter()
                .enumerate()
                .filter(|(_, b)| b.attr_of("data-pr-reveal").is_some())
                .map(|(i, _)| i + 1)
                .collect();
            assert_eq!(revealing, [selected], "exactly the selected tab asks");
            // The strip itself asks, once, to have its drag and wheel wired.
            let mut strips: Vec<&VNode> = Vec::new();
            strip.find_all(&|n| n.has_class("pr-tablist"), &mut strips);
            assert_eq!(strips.len(), 1);
            assert_eq!(strips[0].attr_of("data-pr-slide"), Some("1"), "the strip asks for its slide gestures");
            moved += 1;
        }
        eprintln!("web TabControl reveal — 40 tabs, the selection moved {moved} times: each time exactly one tab, the selected one, carried data-pr-reveal; the strip carried data-pr-slide");
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
        let t = tree();
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
    fn the_tab_order_is_the_browsers_focus_order_on_the_element_that_takes_the_focus() {
        // A button, a check box (the focus is on its input), a text box and a label: only tab stops are numbered.
        let (t, _) = tree_with("Button", "B-1", &[("TabOrder", "3")]);
        let e = Empty::default();
        let i = t.by_name("B-1").unwrap();
        let n = view(&e.cx(&t), i);
        assert_eq!(n.attr_of("tabindex"), Some("3"), "{}", n.describe());

        let (t, i) = tree_with("CheckBox", "C-1", &[("TabOrder", "7")]);
        let n = view(&e.cx(&t), i);
        let input = n.find(&|x| x.tag == "input").unwrap();
        assert_eq!(input.attr_of("tabindex"), Some("7"), "the check box's own input takes the number: {}", n.describe());
        assert!(n.attr_of("tabindex").is_none(), "not its label");

        let (t, i) = tree_with("TextBox", "T-1", &[("TabOrder", "2")]);
        assert_eq!(view(&e.cx(&t), i).find(&|x| x.tag == "input" || x.tag == "textarea").unwrap().attr_of("tabindex"), Some("2"));

        // No number, a label, a disabled control: nothing is forced.
        let (t, i) = tree_with("Button", "B-2", &[]);
        assert!(view(&e.cx(&t), i).attr_of("tabindex").is_none(), "no TabOrder: the document order");
        let (t, i) = tree_with("Label", "L-1", &[("TabOrder", "1")]);
        assert!(view(&e.cx(&t), i).attr_of("tabindex").is_none(), "a Label holds a place in the order but never the focus");
        let (t, i) = tree_with("ListBox", "S-1", &[("TabOrder", "5"), ("Enabled", "0")]);
        let n = view(&e.cx(&t), i);
        assert_eq!(n.find_tag("ul").unwrap().attr_of("tabindex"), Some("-1"), "a disabled control stays out of the order: {}", n.describe());
    }

    #[test]
    fn a_picture_bounds_the_box_chooses_the_keypad_and_shows_the_edited_form_only_at_rest() {
        let e = Empty::default();
        let input = |t: &crate::tree::FormTree, i: usize, focused: bool| {
            let mut e2 = Empty::default();
            if focused {
                e2.ui.focused = Some(i);
            }
            view(&e2.cx(t), i).find(&|x| x.tag == "input").cloned().unwrap()
        };
        // A digits-only picture: its width is the limit and the numeric keypad is offered.
        let (t, i) = tree_with("TextBox", "N-1", &[("Picture", "9(4)"), ("Text", "42")]);
        let n = input(&t, i, false);
        assert_eq!(n.prop_of("maxLength"), Some(&Prop::Num(4.0)));
        assert_eq!(n.attr_of("inputmode"), Some("numeric"));
        // A numeric-edited one: decimal keypad, the edited form at rest, the plain value while focused.
        let (t, i) = tree_with("TextBox", "N-2", &[("Picture", "ZZ,ZZ9.99"), ("Text", "1234.5")]);
        let rest = input(&t, i, false);
        let editing = input(&t, i, true);
        assert_eq!(rest.attr_of("inputmode"), Some("decimal"));
        assert_eq!(rest.prop_of("value"), Some(&Prop::Str(" 1,234.50".into())), "the edited form at rest: {}", rest.describe());
        assert_eq!(editing.prop_of("value"), Some(&Prop::Str("1234.5".into())), "the plain value to edit");
        assert_eq!(rest.prop_of("maxLength"), Some(&Prop::Num(8.0)), "digits, the separator: what can be typed");
        // `DECIMAL-POINT IS COMMA` swaps the roles: the picture is then written with `.` for grouping and `,` for the point.
        let (mut t, i) = tree_with("TextBox", "N-3", &[("Picture", "ZZ.ZZ9,99"), ("Text", "1234,5")]);
        t.decimal_comma = true;
        assert_eq!(input(&t, i, false).prop_of("value"), Some(&Prop::Str(" 1.234,50".into())));
        // No explicit picture: the box is what it was (MaximumLength, no keypad hint).
        let (t, i) = tree_with("TextBox", "N-4", &[("MaximumLength", "10"), ("Text", "hello")]);
        let n = input(&t, i, false);
        assert_eq!((n.prop_of("maxLength"), n.attr_of("inputmode"), n.prop_of("value")), (Some(&Prop::Num(10.0)), None, Some(&Prop::Str("hello".into()))));
        let _ = e;
    }

    #[test]
    fn the_catalogue_icon_is_an_svg_node_and_a_path_is_not_an_icon_name() {
        assert!(catalogue_icon("doc-open").is_some());
        assert!(catalogue_icon("../../etc/passwd").is_none());
        assert!(catalogue_icon("").is_none());
        assert!(catalogue_icon("no-such-icon").is_none());
    }
}
