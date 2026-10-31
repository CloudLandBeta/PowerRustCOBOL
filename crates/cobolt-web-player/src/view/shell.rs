// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! **Menus, the application shell and notifications** (WASM-011, spec 049, spec 055).
//!
//! * the MenuBar, the SideMenu rail and the ToolBar as elements;
//! * the **routes** of a single-page application: a hash (`#/customer-form?id=7`) that names a form and
//!   the parameters it was opened with, so Back, Forward, a reload and a pasted address all work on any
//!   static host without server configuration (WASM-011 R4–R5, Q2);
//! * the **breadcrumb** of the navigation chain;
//! * the **snackbar** notifications the Snackbar control mints;
//! * the **"connecting" modal** that blocks the application while the connection to the backend is retried (WASM-016 R20–R21).

use std::collections::BTreeMap;

use cobolt_forms::menu::{MenuItem, MenuItemType};
use cobolt_forms::snackbar::{parse_buttons, SnackButton, SnackCategory};
use cobolt_forms::toolbar::{ToolbarAction, ToolbarDef};

use super::controls::{catalogue_icon, root};
use super::{VNode, ViewCtx};
use crate::tree::FormTree;
use crate::value::{fmt_number, parse_f64, truthy};

// ── menus ───────────────────────────────────────────────────────────────────

/// The menu the bundle carries for control `i`, or nothing.
pub fn menu_items<'a>(cx: &'a ViewCtx, i: usize) -> &'a [MenuItem] {
    cx.menus.get(&cx.tree.form_name, &cx.tree.ctl(i).name).map(|d| d.menu.as_slice()).unwrap_or(&[])
}

/// The designed items with the rows a program added to a SideMenu at run time (spec 066).
fn merged_items(cx: &ViewCtx, i: usize) -> Vec<MenuItem> {
    let raw = cx.tree.get(i, cobolt_forms::menu::runtime::RUNTIME_ROWS_PROP);
    let designed = menu_items(cx, i);
    if raw.trim().is_empty() {
        return designed.to_vec();
    }
    cobolt_forms::menu::runtime::merge_rows(designed, &cobolt_forms::menu::runtime::parse_rows(&raw))
}

fn item_node(item: &MenuItem, key: String, selected: &str, open: &[String], depth: usize, bar: bool) -> VNode {
    if item.item_type == MenuItemType::Separator {
        return match item.section_title() {
            Some(t) => VNode::new("div").key(key).class("pr-menu-section").attr("role", "presentation").text(t.to_owned()),
            None => VNode::new("hr").key(key).attr("role", "separator"),
        };
    }
    let has_kids = item.has_children();
    let expanded = has_kids && open.iter().any(|o| *o == item.id);
    let mut b = VNode::new("button")
        .key(format!("{key}.b"))
        .attr("type", "button")
        .attr("role", "menuitem")
        .attr("data-id", item.id.clone())
        .attr("tabindex", "-1")
        .prop_bool("disabled", !item.enabled);
    if has_kids {
        b = b.attr("aria-haspopup", "menu").attr("aria-expanded", if expanded { "true" } else { "false" });
    }
    if !selected.is_empty() && selected == item.id {
        b = b.attr("aria-current", "page");
    }
    if let Some(icon) = item.icon.as_deref().and_then(catalogue_icon) {
        b = b.child(icon.key("icon").class("pr-menu-icon"));
    }
    b = b.child(VNode::new("span").key("label").class("pr-menu-label").text(item.label.clone()));
    if let Some(acc) = item.accelerator.as_deref().filter(|a| !a.trim().is_empty()) {
        b = b.child(VNode::new("span").key("accel").class("pr-menu-accel").text(acc.to_owned()));
    }
    if let Some(badge) = item.badge_text() {
        b = b.child(VNode::new("span").key("badge").class(format!("pr-badge pr-badge-{}", item.badge_style.as_str().to_ascii_lowercase())).text(badge.to_owned()));
    }
    if has_kids && !bar {
        b = b.child(VNode::new("span").key("chev").class("pr-chevron").attr("aria-hidden", "true").text(if expanded { "\u{25BE}" } else { "\u{25B8}" }));
    }
    let mut li = VNode::new(if bar && depth == 0 { "div" } else { "li" }).key(key.clone()).class("pr-menu").attr("role", "none");
    if expanded {
        li = li.class("pr-open");
    }
    li = li.child(b);
    if expanded {
        let mut sub = VNode::new("ul").key(format!("{key}.sub")).class("pr-menu-items").attr("role", "menu");
        for (k, child) in item.items.iter().enumerate() {
            sub = sub.child(item_node(child, format!("{key}.{k}"), selected, open, depth + 1, bar));
        }
        li = li.child(sub);
    }
    li
}

pub fn menu_bar(cx: &ViewCtx, i: usize) -> VNode {
    let items = menu_items(cx, i);
    let selected = cx.tree.get(i, "SelectedItemId");
    let mut n = root(cx, i, "nav").attr("role", "menubar").attr("aria-label", "Menu").on("click").on("keydown").on("pointerenter");
    if cx.tree.get(i, "MenuBarStyle").eq_ignore_ascii_case("Responsive") {
        n = n.class("pr-responsive");
    }
    for (k, item) in items.iter().enumerate() {
        n = n.child(item_node(item, format!("m{k}"), &selected, &cx.ui.open_menu, 0, true));
    }
    n
}

pub fn side_menu(cx: &ViewCtx, i: usize) -> VNode {
    let selected = cx.tree.get(i, "SelectedItemId");
    let collapsed = cx.ui.sidebar_collapsed.unwrap_or_else(|| truthy(&cx.tree.get(i, "Collapsed")));
    let mut n = root(cx, i, "nav").attr("role", "navigation").attr("aria-label", "Main menu").on("click").on("keydown");
    if collapsed {
        n = n.class("pr-collapsed");
    }
    // The header: the logo (the collapsed rail shows the icon instead) and the application title.
    let title = cx.tree.get(i, "AppTitle");
    let logo_path = if collapsed && !cx.tree.get(i, "HeaderIcon").is_empty() { cx.tree.get(i, "HeaderIcon") } else { cx.tree.get(i, "HeaderImage") };
    let mut header = VNode::new("header").key("header").class("pr-sm-header");
    if let Some(url) = cx.image_url(&logo_path) {
        header = header.child(VNode::new("img").key("logo").attr("src", url).attr("alt", "").attr("draggable", "false"));
    }
    if !title.is_empty() && !collapsed {
        header = header.child(VNode::new("span").key("title").class("pr-sm-title").text(title));
    }
    n = n.child(header);
    n = n.child(
        VNode::new("button")
            .key("toggle")
            .class("pr-sm-toggle")
            .attr("type", "button")
            .attr("data-part", "toggle")
            .attr("aria-label", if collapsed { "Expand the menu" } else { "Collapse the menu" })
            .attr("aria-expanded", if collapsed { "false" } else { "true" })
            .text(if collapsed { "\u{00BB}" } else { "\u{00AB}" }),
    );
    let mut list = VNode::new("ul").key("items").class("pr-sm-items").attr("role", "menu");
    for (k, item) in merged_items(cx, i).iter().enumerate() {
        list = list.child(item_node(item, format!("s{k}"), &selected, &cx.ui.open_menu, 0, false));
    }
    n = n.child(list);
    n.child(VNode::new("footer").key("footer").class("pr-sm-footer").slot(super::Slot::Children))
}

// ── ToolBar ─────────────────────────────────────────────────────────────────

/// The toolbar of control `i`: the stored definition, else the legacy `Items` as one group.
pub fn toolbar_def(tree: &FormTree, i: usize) -> ToolbarDef {
    let raw = tree.get(i, cobolt_forms::toolbar::TOOLBAR_DEF_PROP);
    if !raw.trim().is_empty() {
        if let Ok(def) = serde_json::from_str::<ToolbarDef>(raw.trim()) {
            return def;
        }
    }
    ToolbarDef::from_items(&tree.get(i, "Items"))
}

/// Does this action need the machine the application runs on (WASM-004 class D)?
pub fn unavailable_in_a_browser(action: &ToolbarAction) -> bool {
    action.is_platform_action()
}

pub fn tool_bar(cx: &ViewCtx, i: usize) -> VNode {
    let def = toolbar_def(cx.tree, i);
    let mut n = root(cx, i, "div").attr("role", "toolbar").on("click");
    for (g, group) in def.groups.iter().enumerate() {
        let mut gn = VNode::new("div").key(format!("g{g}")).class("pr-tb-group").attr("role", "group");
        for (b, btn) in group.buttons.iter().enumerate() {
            let action = ToolbarAction::parse(&btn.action);
            let mut bn = VNode::new("button")
                .key(format!("g{g}b{b}"))
                .attr("type", "button")
                .attr("data-id", btn.id.clone())
                .prop_bool("disabled", !btn.enabled || unavailable_in_a_browser(&action));
            if unavailable_in_a_browser(&action) {
                bn = bn.attr("title", "Not available in a browser").attr("data-unavailable", "");
            } else if !btn.tooltip.is_empty() {
                bn = bn.attr("title", btn.tooltip.clone());
            }
            if let Some(icon) = catalogue_icon(&btn.icon) {
                bn = bn.child(icon.key("icon").class("pr-tb-icon"));
                if btn.label.is_empty() {
                    bn = bn.attr("aria-label", if btn.tooltip.is_empty() { btn.id.clone() } else { btn.tooltip.clone() });
                }
            }
            if !btn.label.is_empty() {
                bn = bn.child(VNode::new("span").key("label").text(btn.label.clone()));
            }
            gn = gn.child(bn);
        }
        n = n.child(gn);
    }
    n
}

// ── notifications ───────────────────────────────────────────────────────────

/// A notification on screen (spec 055): minted from the Snackbar control's template when `Show()` runs.
#[derive(Debug, Clone, PartialEq)]
pub struct Toast {
    pub id: u32,
    /// The instance and the Snackbar control that minted it (its events go back to that control).
    pub inst: u32,
    pub ctl: usize,
    pub text: String,
    pub category: SnackCategory,
    pub buttons: Vec<SnackButton>,
    /// `0` = stays until dismissed.
    pub timeout_ms: i64,
    pub pause_on_hover: bool,
    pub shown_at_ms: u64,
}

pub fn category_name(c: SnackCategory) -> &'static str {
    match c {
        SnackCategory::Info => "Info",
        SnackCategory::Question => "Question",
        SnackCategory::Warning => "Warning",
        SnackCategory::Error => "Error",
        SnackCategory::Critical => "Critical",
    }
}

/// Mint a notification from the template's current properties. `text` overrides the template's `Text`
/// (`Show("message")`).
pub fn mint_toast(tree: &FormTree, inst: u32, ctl: usize, id: u32, text: Option<&str>, now_ms: u64) -> Toast {
    let category = SnackCategory::from_prop(&tree.get(ctl, "Category"));
    let timeout = parse_f64(&tree.get(ctl, "Timeout")).map(|v| v as i64).unwrap_or(-1);
    let (buttons, _diag) = parse_buttons(&tree.get(ctl, "Buttons"));
    Toast {
        id,
        inst,
        ctl,
        text: text.map(str::to_owned).unwrap_or_else(|| tree.get(ctl, "Text")),
        category,
        buttons,
        timeout_ms: if timeout < 0 { category.defaults().timeout_ms } else { timeout },
        pause_on_hover: truthy(&tree.get(ctl, "PauseTimeoutOnHover")),
        shown_at_ms: now_ms,
    }
}

pub fn toast_view(t: &Toast) -> VNode {
    let mut n = VNode::new("div")
        .key(format!("toast:{}", t.id))
        .class("pr-snackbar")
        .attr("role", if matches!(t.category, SnackCategory::Error | SnackCategory::Critical) { "alert" } else { "status" })
        .attr("data-category", category_name(t.category))
        .attr("data-id", t.id.to_string());
    if let Some(icon) = catalogue_icon(t.category.defaults().icon) {
        n = n.child(icon.key("icon").class("pr-snack-icon"));
    }
    n = n.child(VNode::new("span").key("text").class("pr-snack-text").text(t.text.clone()));
    for (k, b) in t.buttons.iter().enumerate() {
        let mut bn = VNode::new("button").key(format!("b{k}")).attr("type", "button").attr("data-toast", t.id.to_string()).attr("data-part", "toast-button").attr("data-id", b.id.clone()).attr("data-dismiss", if b.dismiss { "1" } else { "0" });
        if let Some(icon) = catalogue_icon(&b.icon) {
            bn = bn.child(icon.key("icon"));
        }
        if !b.text.is_empty() {
            bn = bn.child(VNode::new("span").key("t").text(b.text.clone()));
        } else {
            bn = bn.attr("aria-label", b.id.clone());
        }
        n = n.child(bn);
    }
    n.child(VNode::new("button").key("close").class("pr-snack-close").attr("type", "button").attr("data-toast", t.id.to_string()).attr("data-part", "toast-close").attr("aria-label", "Dismiss").text("\u{00D7}"))
}

/// The container that holds the notifications of an application.
pub fn toast_stack(toasts: &[Toast]) -> VNode {
    VNode::new("div").key("snackbars").class("pr-snackbars").attr("aria-live", "polite").on("click").children(toasts.iter().map(toast_view))
}

// ── routes ──────────────────────────────────────────────────────────────────

/// A place in the application: a form and the parameters it was opened with.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Route {
    pub form: String,
    pub params: Vec<(String, String)>,
}

fn pct_encode(s: &str) -> String {
    let mut out = String::new();
    for b in s.bytes() {
        if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.' | b'~') {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

fn pct_decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut k = 0;
    while k < bytes.len() {
        if bytes[k] == b'%' && k + 2 < bytes.len() {
            let hex = std::str::from_utf8(&bytes[k + 1..k + 3]).ok().and_then(|h| u8::from_str_radix(h, 16).ok());
            if let Some(v) = hex {
                out.push(v);
                k += 3;
                continue;
            }
        }
        out.push(bytes[k]);
        k += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

impl Route {
    pub fn new(form: &str) -> Route {
        Route { form: form.to_owned(), params: Vec::new() }
    }

    /// `#/form-name?key=value&key2=value2` — a hash works on any static host and never touches the
    /// host page's own path (WASM-011 Q2).
    pub fn to_hash(&self) -> String {
        let mut s = format!("#/{}", pct_encode(&self.form));
        for (k, (name, value)) in self.params.iter().enumerate() {
            s.push(if k == 0 { '?' } else { '&' });
            s.push_str(&pct_encode(name));
            s.push('=');
            s.push_str(&pct_encode(value));
        }
        s
    }

    /// Read a hash. `None` for an empty hash or one that is not a route (an in-page anchor).
    pub fn parse(hash: &str) -> Option<Route> {
        let h = hash.trim().strip_prefix('#')?;
        let h = h.strip_prefix('/')?;
        let (path, query) = h.split_once('?').unwrap_or((h, ""));
        let form = pct_decode(path.trim_end_matches('/'));
        if form.is_empty() || form.contains('/') {
            return None;
        }
        let params = query
            .split('&')
            .filter(|p| !p.is_empty())
            .map(|p| {
                let (k, v) = p.split_once('=').unwrap_or((p, ""));
                (pct_decode(k), pct_decode(v))
            })
            .collect();
        Some(Route { form, params })
    }
}

/// One entry of the navigation chain (spec 049 R19–R22).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Crumb {
    pub inst: u32,
    pub label: String,
}

pub fn breadcrumb(chain: &[Crumb], detail: &str) -> VNode {
    let mut n = VNode::new("nav").key("breadcrumb").class("pr-breadcrumb").attr("aria-label", "Breadcrumb").on("click");
    let mut list = VNode::new("ol").key("list");
    for (k, c) in chain.iter().enumerate() {
        let last = k + 1 == chain.len();
        let mut li = VNode::new("li").key(format!("c{}", c.inst));
        if last {
            li = li.child(VNode::new("span").key("here").attr("aria-current", "page").text(c.label.clone()));
        } else {
            li = li.child(VNode::new("button").key("go").attr("type", "button").attr("data-inst", c.inst.to_string()).attr("data-part", "crumb").text(c.label.clone()));
        }
        list = list.child(li);
    }
    n = n.child(list);
    if !detail.is_empty() {
        n = n.child(VNode::new("span").key("detail").class("pr-crumb-detail").text(detail.to_owned()));
    }
    n
}

// ── dialogs, frames and the banners ─────────────────────────────────────────

/// A dialog the program asked for (`COBOL::"MESSAGE"`, `CONFIRM`, `INPUTBOX`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DialogSpec {
    pub id: u32,
    pub kind: crate::host::DialogKind,
    pub title: String,
    pub text: String,
    /// `buttons` as the program gave it: captions separated by `,` or `|`.
    pub buttons: String,
    /// The input box's starting text.
    pub default: String,
}

/// The captions of a dialog's buttons.
pub fn dialog_buttons(spec: &DialogSpec) -> Vec<String> {
    use crate::host::DialogKind::*;
    let given: Vec<String> = spec.buttons.split([',', '|']).map(|s| s.trim().to_owned()).filter(|s| !s.is_empty()).collect();
    if !given.is_empty() && spec.kind != Input {
        return given;
    }
    match spec.kind {
        Message => vec!["OK".into()],
        Confirm => vec!["Yes".into(), "No".into()],
        Input => vec!["OK".into(), "Cancel".into()],
    }
}

/// The result of a dialog that is dismissed with Escape: the safe answer.
pub fn dialog_cancel_result(spec: &DialogSpec) -> String {
    use crate::host::DialogKind::*;
    match spec.kind {
        Message => dialog_buttons(spec).first().cloned().unwrap_or_else(|| "OK".into()),
        Confirm => dialog_buttons(spec).last().cloned().unwrap_or_else(|| "No".into()),
        Input => String::new(),
    }
}

pub fn dialog_view(spec: &DialogSpec) -> VNode {
    use crate::host::DialogKind::*;
    let title_id = format!("pr-dlg-title-{}", spec.id);
    let mut d = VNode::new("dialog")
        .key("dialog")
        .class("pr-dialog")
        .attr("role", if spec.kind == Message { "alertdialog" } else { "dialog" })
        .attr("aria-labelledby", title_id.clone())
        .attr("data-dialog", spec.id.to_string())
        .on("cancel")
        .on("click")
        .on("keydown");
    let title = if spec.title.trim().is_empty() { "PowerRustCOBOL".to_owned() } else { spec.title.clone() };
    d = d.child(VNode::new("div").key("titlebar").class("pr-titlebar").child(VNode::new("span").key("title").attr("id", title_id).class("pr-title").text(title)));
    d = d.child(VNode::new("div").key("text").class("pr-dialog-text").text(spec.text.clone()));
    if spec.kind == Input {
        d = d.child(VNode::new("div").key("field").class("pr-dialog-field").child(VNode::new("input").key("input").attr("type", "text").attr("aria-label", spec.text.clone()).prop_str("value", spec.default.clone())));
    }
    let mut row = VNode::new("div").key("buttons").class("pr-dialog-buttons");
    for (k, label) in dialog_buttons(spec).into_iter().enumerate() {
        let mut b = VNode::new("button").key(format!("b{k}")).attr("type", "button").attr("data-part", "dialog-button").attr("data-id", label.clone()).text(label);
        if k == 0 {
            b = b.class("pr-default");
        }
        row = row.child(b);
    }
    d.child(row)
}

/// The frame of a form shown as a modal dialog.
pub fn modal_frame(title: &str, closable: bool, size: Option<(f64, f64)>) -> VNode {
    let mut bar = VNode::new("div").key("titlebar").class("pr-titlebar").child(VNode::new("span").key("title").class("pr-title").text(title.to_owned()));
    if closable {
        bar = bar.child(VNode::new("button").key("close").attr("type", "button").attr("data-part", "close").attr("aria-label", "Close").text("\u{00D7}"));
    }
    let mut body = VNode::new("div").key("body").class("pr-body");
    if let Some((w, h)) = size {
        body = body.style("width", format!("{}px", fmt_number(w))).style("height", format!("{}px", fmt_number(h)));
    }
    VNode::new("dialog").key("frame").class("pr-dialog pr-form-dialog").attr("role", "dialog").attr("aria-label", title.to_owned()).on("cancel").on("click").child(bar).child(body)
}

/// The frame of a form shown as a modeless panel.
pub fn panel_frame(title: &str, size: Option<(f64, f64)>, offset: (f64, f64)) -> VNode {
    let bar = VNode::new("div")
        .key("titlebar")
        .class("pr-titlebar")
        .on("pointerdown")
        .child(VNode::new("span").key("title").class("pr-title").text(title.to_owned()))
        .child(VNode::new("button").key("close").attr("type", "button").attr("data-part", "close").attr("aria-label", "Close").text("\u{00D7}"));
    let mut body = VNode::new("div").key("body").class("pr-body");
    if let Some((w, h)) = size {
        body = body.style("width", format!("{}px", fmt_number(w))).style("height", format!("{}px", fmt_number(h)));
    }
    VNode::new("section")
        .key("frame")
        .class("pr-panelwin")
        .attr("role", "dialog")
        .attr("aria-label", title.to_owned())
        .style("left", format!("{}px", fmt_number(offset.0)))
        .style("top", format!("{}px", fmt_number(offset.1)))
        .on("click")
        .child(bar)
        .child(body)
}

/// The banner a runtime error shows (WASM-017 R15: readable, with a way on).
pub fn banner_view(message: &str, can_reload: bool) -> VNode {
    let mut n = VNode::new("div").key("banner").class("pr-error-banner").attr("role", "alert").on("click").child(VNode::new("span").key("text").text(message.to_owned()));
    let mut actions = VNode::new("span").key("actions");
    if can_reload {
        actions = actions.child(VNode::new("button").key("reload").attr("type", "button").attr("data-part", "reload").text("Reload"));
    }
    actions = actions.child(VNode::new("button").key("dismiss").attr("type", "button").attr("data-part", "dismiss").text("Dismiss"));
    n = n.child(actions);
    n
}

/// What the page says when the application has ended, in one language.
pub struct EndedText {
    /// A BCP 47 tag; its first subtag is the language.
    pub lang: &'static str,
    pub title: &'static str,
    pub hint: &'static str,
    pub reload: &'static str,
}

/// The six languages of the IDE (the order of its language menu), as the page that opens from disk
/// speaks them (`cobolt-web-bundle`'s `notice`).
pub const ENDED_TEXTS: [EndedText; 6] = [
    EndedText { lang: "en", title: "The application has ended.", hint: "Reload the page to start it again.", reload: "Reload" },
    EndedText { lang: "pt-BR", title: "O aplicativo foi encerrado.", hint: "Recarregue a página para iniciá-lo de novo.", reload: "Recarregar" },
    EndedText { lang: "es", title: "La aplicación ha terminado.", hint: "Recarga la página para iniciarla de nuevo.", reload: "Recargar" },
    EndedText { lang: "fr", title: "L'application est terminée.", hint: "Rechargez la page pour la relancer.", reload: "Recharger" },
    EndedText { lang: "ja", title: "アプリケーションは終了しました。", hint: "もう一度開始するには、ページを再読み込みしてください。", reload: "再読み込み" },
    EndedText { lang: "zh", title: "应用程序已结束。", hint: "重新加载页面即可再次启动。", reload: "重新加载" },
];

/// The text for the browser's language (`navigator.language`): its first subtag, English otherwise.
pub fn ended_text(language: &str) -> &'static EndedText {
    let first = language.split(['-', '_']).next().unwrap_or("").to_ascii_lowercase();
    ENDED_TEXTS.iter().find(|t| t.lang.split('-').next() == Some(first.as_str())).unwrap_or(&ENDED_TEXTS[0])
}

/// What stands in the page's place once the application has ended — its main form closed (`me::Close()`, the
/// window's red x) or `STOP RUN`. The desktop closes the window and the program ends; a page can close its tab only
/// when a script opened it, so it says so, in the browser's language, and offers to start again.
pub fn ended_view(language: &str) -> VNode {
    let t = ended_text(language);
    VNode::new("div")
        .key("ended")
        .class("pr-ended")
        .attr("role", "status")
        .attr("lang", t.lang)
        .on("click")
        .child(VNode::new("p").key("title").class("pr-ended-title").text(t.title.to_owned()))
        .child(VNode::new("p").key("hint").class("pr-ended-hint").text(t.hint.to_owned()))
        .child(VNode::new("button").key("reload").attr("type", "button").attr("data-part", "reload").text(t.reload.to_owned()))
}

/// The badge that shows the backend connection is not up.
pub fn net_badge(text: &str) -> VNode {
    VNode::new("div").key("net").class("pr-net").attr("role", "status").text(text.to_owned())
}

/// The "connecting" modal's texts in one language (WASM-016 R20–R21).
pub struct NetModalText {
    /// A BCP 47 tag; its first subtag is the language.
    pub lang: &'static str,
    pub connecting: &'static str,
    pub unreachable: &'static str,
    pub reload: &'static str,
}

/// The six languages of the IDE, as [`ENDED_TEXTS`]. The host page's own texts (`texts.connecting`,
/// `texts.unreachable`, `texts.reload`; WASM-002 R15) replace them when the developer gives them.
pub const NET_MODAL_TEXTS: [NetModalText; 6] = [
    NetModalText { lang: "en", connecting: "Connecting to the server\u{2026}", unreachable: "The server could not be reached.", reload: "Reload" },
    NetModalText { lang: "pt-BR", connecting: "Conectando ao servidor\u{2026}", unreachable: "Não foi possível alcançar o servidor.", reload: "Recarregar" },
    NetModalText { lang: "es", connecting: "Conectando con el servidor\u{2026}", unreachable: "No se pudo conectar con el servidor.", reload: "Recargar" },
    NetModalText { lang: "fr", connecting: "Connexion au serveur\u{2026}", unreachable: "Le serveur est injoignable.", reload: "Recharger" },
    NetModalText { lang: "ja", connecting: "サーバーに接続しています\u{2026}", unreachable: "サーバーに接続できませんでした。", reload: "再読み込み" },
    NetModalText { lang: "zh", connecting: "正在连接服务器\u{2026}", unreachable: "无法连接到服务器。", reload: "重新加载" },
];

/// The modal's caption and its Reload button's label: the host page's text when it has one, otherwise the page's
/// language (its first subtag) among the six, English otherwise.
pub fn net_modal_texts(locale: &str, page: &BTreeMap<String, String>, unreachable: bool) -> (String, String) {
    let first = locale.split(['-', '_']).next().unwrap_or("").to_ascii_lowercase();
    let t = NET_MODAL_TEXTS.iter().find(|t| t.lang.split('-').next() == Some(first.as_str())).unwrap_or(&NET_MODAL_TEXTS[0]);
    let pick = |key: &str, fallback: &str| page.get(key).map(|s| s.trim()).filter(|s| !s.is_empty()).unwrap_or(fallback).to_owned();
    let caption = if unreachable { pick("unreachable", t.unreachable) } else { pick("connecting", t.connecting) };
    (caption, pick("reload", t.reload))
}

/// The modal's background (WASM-016 R20): left to right, red → dark red → crimson → black.
pub const NET_MODAL_GRADIENT: &str = "linear-gradient(to right, #ff0000 0%, #8b0000 40%, #dc143c 70%, #000000 100%)";

/// The "connecting" modal (WASM-016 R20–R21): a screen on the left trying to reach a server on the right through a cloud,
/// all white on the gradient, and the caption. While the player retries the attempt is animated (the links flow, the cloud
/// pulses: `dom::animate_net_modal`); once the retries are exhausted the drawing stands still, the caption says the server could not be
/// reached, and the host page's own recovery — Reload — is offered. The colours are the element's own declarations, so no
/// stylesheet of the application can change them.
pub fn net_modal_view(unreachable: bool, caption: &str, reload: &str) -> VNode {
    const WHITE: &str = "#ffffff";
    let stroke = |n: VNode| n.attr("fill", "none").attr("stroke", WHITE).attr("stroke-width", "3").attr("stroke-linecap", "round").attr("stroke-linejoin", "round");
    let link = |key: &str, x1: &str, x2: &str| {
        stroke(VNode::svg("line").key(key).class("pr-netmodal-link").attr("x1", x1).attr("y1", "46").attr("x2", x2).attr("y2", "46")).attr("stroke-dasharray", "6 6")
    };
    let led = |key: &str, cy: &str| VNode::svg("circle").key(key).attr("cx", "254").attr("cy", cy).attr("r", "3").attr("fill", WHITE);
    let drawing = VNode::svg("svg")
        .key("drawing")
        .class("pr-netmodal-drawing")
        .attr("viewBox", "0 0 320 110")
        .attr("aria-hidden", "true")
        .child(
            VNode::svg("g")
                .key("screen")
                .class("pr-netmodal-screen")
                .child(stroke(VNode::svg("rect").key("frame").attr("x", "10").attr("y", "18").attr("width", "72").attr("height", "50").attr("rx", "4")))
                .child(stroke(VNode::svg("path").key("stand").attr("d", "M40 68 v12 M52 68 v12 M30 82 h32"))),
        )
        .child(link("link-left", "88", "124"))
        .child(stroke(VNode::svg("path").key("cloud").class("pr-netmodal-cloud").attr("d", "M138 64 a14 14 0 0 1 2 -28 a20 20 0 0 1 38 -6 a16 16 0 0 1 14 34 z")))
        .child(link("link-right", "198", "232"))
        .child(
            VNode::svg("g")
                .key("server")
                .class("pr-netmodal-server")
                .child(stroke(VNode::svg("rect").key("u1").attr("x", "240").attr("y", "14").attr("width", "70").attr("height", "22").attr("rx", "3")))
                .child(stroke(VNode::svg("rect").key("u2").attr("x", "240").attr("y", "40").attr("width", "70").attr("height", "22").attr("rx", "3")))
                .child(stroke(VNode::svg("rect").key("u3").attr("x", "240").attr("y", "66").attr("width", "70").attr("height", "22").attr("rx", "3")))
                .child(led("l1", "25"))
                .child(led("l2", "51"))
                .child(led("l3", "77")),
        );
    let mut n = VNode::new("dialog")
        .key("netmodal")
        .class("pr-netmodal")
        .attr("role", "alertdialog")
        .attr("aria-modal", "true")
        .attr("tabindex", "-1")
        .attr("data-state", if unreachable { "unreachable" } else { "connecting" })
        .style("background", NET_MODAL_GRADIENT)
        .style("color", WHITE)
        .on_all(["cancel", "close", "click", "keydown", "keyup", "keypress", "pointerdown", "pointerup"])
        .child(drawing)
        .child(VNode::new("p").key("caption").class("pr-netmodal-caption").attr("role", "status").attr("aria-live", "assertive").text(caption.to_owned()));
    if unreachable {
        n = n.class("pr-netmodal-failed").child(VNode::new("button").key("reload").attr("type", "button").attr("data-part", "reload").flag("autofocus", true).text(reload.to_owned()));
    }
    n
}

/// Format a number for the shell (used for the badge counters).
pub fn count_text(n: usize) -> String {
    if n > 99 {
        "99+".to_owned()
    } else {
        fmt_number(n as f64)
    }
}

#[cfg(test)]
mod tests {
    use super::super::{Empty, MenuStore};
    use super::*;

    /// FX-4a: the page that stands in for a closed application speaks the browser's language — the six of the
    /// IDE, by the first subtag — and English otherwise, and always offers to start again.
    #[test]
    fn the_ended_page_speaks_the_browsers_language() {
        let cases = [("en-US", "en"), ("pt-BR", "pt-BR"), ("pt-PT", "pt-BR"), ("es-419", "es"), ("fr-CA", "fr"), ("ja", "ja"), ("zh-CN", "zh"), ("zh_TW", "zh"), ("de-DE", "en"), ("", "en")];
        for (browser, lang) in cases {
            assert_eq!(ended_text(browser).lang, lang, "{browser}");
            let v = ended_view(browser);
            assert_eq!(v.attr_of("lang"), Some(lang));
            assert!(v.find(&|n| n.attr_of("data-part") == Some("reload")).is_some(), "{browser}: a Reload button");
        }
        let titles: std::collections::BTreeSet<&str> = ENDED_TEXTS.iter().map(|t| t.title).collect();
        assert_eq!(titles.len(), 6, "six different texts");
    }

    /// WASM-016 R20–R21: the modal's two captions and its Reload are in the page's language — one of the six, English
    /// otherwise — and the host page's own text wins over them when the developer gave one.
    #[test]
    fn the_connecting_modal_speaks_the_pages_language_and_the_host_pages_texts_win() {
        let none = BTreeMap::new();
        let mut seen = Vec::new();
        for (locale, lang) in [("en-US", "en"), ("pt-BR", "pt-BR"), ("es-419", "es"), ("fr-CA", "fr"), ("ja-JP", "ja"), ("zh_CN", "zh"), ("de", "en")] {
            let t = NET_MODAL_TEXTS.iter().find(|t| t.lang == lang).unwrap();
            assert_eq!(net_modal_texts(locale, &none, false), (t.connecting.to_owned(), t.reload.to_owned()), "{locale}");
            assert_eq!(net_modal_texts(locale, &none, true), (t.unreachable.to_owned(), t.reload.to_owned()), "{locale}");
            seen.push(format!("{locale}: {:?} / {:?}", t.connecting, t.unreachable));
        }
        for field in [|t: &NetModalText| t.connecting, |t: &NetModalText| t.unreachable, |t: &NetModalText| t.reload] {
            let distinct: std::collections::BTreeSet<&str> = NET_MODAL_TEXTS.iter().map(field).collect();
            assert!(distinct.len() >= 5, "a text is not translated: {distinct:?}");
            assert!(distinct.iter().all(|s| !s.trim().is_empty()));
        }
        let page: BTreeMap<String, String> = [("connecting", "Ligando…"), ("unreachable", "Sem servidor."), ("reload", "  ")].into_iter().map(|(k, v)| (k.to_owned(), v.to_owned())).collect();
        assert_eq!(net_modal_texts("en", &page, false), ("Ligando…".to_owned(), "Reload".to_owned()), "a blank text of the page keeps the default");
        assert_eq!(net_modal_texts("en", &page, true).0, "Sem servidor.");
        println!("{} languages: {}", NET_MODAL_TEXTS.len(), seen.join("; "));
    }

    /// WASM-016 R20–R21: the modal is a dialog drawn white on the red → dark red → crimson → black gradient (its own
    /// declarations): a screen, a cloud and a server, the two links animated while the player retries; once the retries are
    /// exhausted it says so and offers Reload, and nothing in it is a markup string.
    #[test]
    fn the_connecting_modal_draws_the_screen_the_cloud_and_the_server_white_on_the_gradient() {
        let v = net_modal_view(false, "Connecting to the server\u{2026}", "Reload");
        assert_eq!(v.tag, "dialog");
        assert_eq!(v.attr_of("aria-modal"), Some("true"));
        assert_eq!(v.attr_of("data-state"), Some("connecting"));
        let style = |k: &str| v.styles.iter().find(|(a, _)| a == k).map(|(_, b)| b.as_str());
        assert_eq!(style("background"), Some(NET_MODAL_GRADIENT));
        assert_eq!(style("color"), Some("#ffffff"));
        let stops: Vec<&str> = ["#ff0000", "#8b0000", "#dc143c", "#000000"].into_iter().filter(|c| NET_MODAL_GRADIENT.contains(c)).collect();
        assert_eq!(stops.len(), 4, "red, dark red, crimson, black");
        assert!(NET_MODAL_GRADIENT.find("#ff0000") < NET_MODAL_GRADIENT.find("#8b0000") && NET_MODAL_GRADIENT.find("#dc143c") < NET_MODAL_GRADIENT.find("#000000"));
        for part in ["pr-netmodal-screen", "pr-netmodal-cloud", "pr-netmodal-server"] {
            assert!(v.find(&|n| n.svg && n.classes.iter().any(|c| c == part)).is_some(), "{part} is drawn");
        }
        fn count(n: &VNode, class: &str) -> usize {
            usize::from(n.classes.iter().any(|c| c == class)) + n.children.iter().map(|c| count(c, class)).sum::<usize>()
        }
        assert_eq!(count(&v, "pr-netmodal-link"), 2, "a link from the screen to the cloud and one from the cloud to the server");
        // Every stroke and fill of the drawing is white (or none).
        fn paints(n: &VNode, out: &mut Vec<String>) {
            for (k, val) in &n.attrs {
                if k == "fill" || k == "stroke" {
                    out.push(val.clone());
                }
            }
            n.children.iter().for_each(|c| paints(c, out));
        }
        let mut colours = Vec::new();
        paints(&v, &mut colours);
        assert!(!colours.is_empty() && colours.iter().all(|c| c == "#ffffff" || c == "none"), "{colours:?}");
        assert!(v.find(&|n| n.attr_of("data-part") == Some("reload")).is_none(), "no Reload while it retries");
        assert!(v.on.contains(&"keydown") && v.on.contains(&"pointerdown") && v.on.contains(&"cancel"), "keys, pointers and Escape are taken by the modal");

        let f = net_modal_view(true, "The server could not be reached.", "Reload");
        assert_eq!(f.attr_of("data-state"), Some("unreachable"));
        assert!(f.classes.iter().any(|c| c == "pr-netmodal-failed"), "the drawing stands still");
        let reload = f.find(&|n| n.attr_of("data-part") == Some("reload")).expect("Reload once the retries are exhausted");
        assert_eq!(reload.text.as_deref(), Some("Reload"));
        assert_eq!(f.find(&|n| n.key == "caption").and_then(|n| n.text.as_deref()), Some("The server could not be reached."));
        println!("{} colour attributes, all white; links 2; Reload only when unreachable", colours.len());
    }
    use crate::tree::testing::tree_with;
    use cobolt_forms::menu::{MenuDefinition, MenuItem};

    fn menu() -> MenuDefinition {
        let mut file = MenuItem::new_action("file", "File");
        let mut open = MenuItem::new_action("open", "Open");
        open.action = Some("open-form:Customers".into());
        open.icon = Some("doc-open".into());
        let mut quit = MenuItem::new_action("quit", "Quit");
        quit.enabled = false;
        file.items = vec![open, MenuItem::new_separator("sep"), quit];
        let mut help = MenuItem::new_action("help", "Help");
        help.badge = Some("New".into());
        MenuDefinition { menu: vec![file, help], hash: String::new() }
    }

    fn with_menu(kind: &str, open: &[&str]) -> VNode {
        let (t, i) = tree_with(kind, "MNU-1", &[("SelectedItemId", "open")]);
        let mut e = Empty::default();
        let mut store = MenuStore::default();
        store.insert("main-form", "MNU-1", menu());
        e.menus = store;
        e.ui.open_menu = open.iter().map(|s| s.to_string()).collect();
        let cx = e.cx(&t);
        if kind == "MenuBar" {
            menu_bar(&cx, i)
        } else {
            side_menu(&cx, i)
        }
    }

    #[test]
    fn a_menu_bar_shows_its_top_level_items_and_opens_one_dropdown() {
        let closed = with_menu("MenuBar", &[]);
        assert_eq!(closed.attr_of("role"), Some("menubar"));
        let tops: Vec<&str> = closed.children.iter().map(|c| c.find_tag("button").unwrap().attr_of("data-id").unwrap()).collect();
        assert_eq!(tops, ["file", "help"]);
        assert_eq!(closed.count("ul"), 0, "nothing is open");
        let open = with_menu("MenuBar", &["file"]);
        let sub = open.find_tag("ul").expect("the open menu");
        assert_eq!(sub.attr_of("role"), Some("menu"));
        assert_eq!(sub.children.len(), 3);
        assert_eq!(sub.children[1].tag, "hr", "an unlabelled separator is a rule");
        let quit = sub.children[2].find_tag("button").unwrap();
        assert_eq!(quit.prop_of("disabled"), Some(&super::super::Prop::Bool(true)));
        let file = open.children[0].find_tag("button").unwrap();
        assert_eq!(file.attr_of("aria-expanded"), Some("true"));
        assert_eq!(file.attr_of("aria-haspopup"), Some("menu"));
        assert!(sub.children[0].find_tag("svg").is_some(), "the catalogue icon of an item");
    }

    #[test]
    fn a_side_menu_has_a_toggle_a_badge_and_marks_the_current_item() {
        let n = with_menu("SideMenu", &["file"]);
        assert_eq!(n.attr_of("role"), Some("navigation"));
        let toggle = n.find(&|x| x.attr_of("data-part") == Some("toggle")).unwrap();
        assert_eq!(toggle.attr_of("aria-expanded"), Some("true"));
        let current = n.find(&|x| x.attr_of("aria-current") == Some("page")).unwrap();
        assert_eq!(current.attr_of("data-id"), Some("open"));
        assert!(n.find(&|x| x.has_class("pr-badge")).unwrap().all_text() == "New");
        assert!(n.find_tag("footer").is_some_and(|f| f.slot.is_some()), "the footer takes the developer's panel");
    }

    #[test]
    fn a_collapsed_side_menu_says_so() {
        let (t, i) = tree_with("SideMenu", "MNU-1", &[("Collapsed", "1")]);
        let e = Empty::default();
        let n = side_menu(&e.cx(&t), i);
        assert!(n.has_class("pr-collapsed"));
        assert_eq!(n.find(&|x| x.attr_of("data-part") == Some("toggle")).unwrap().attr_of("aria-expanded"), Some("false"));
    }

    #[test]
    fn a_tool_bar_draws_its_groups_and_refuses_the_buttons_that_need_the_machine() {
        let def = r#"{"groups":[{"id":"g","buttons":[{"id":"open","label":"Open","action":"open-modal:Dialog"},{"id":"print","label":"Print","action":"print:doc.pdf"},{"id":"ico","icon":"doc-open","tooltip":"Open file"}]}]}"#;
        let (t, i) = tree_with("ToolBar", "TB-1", &[(cobolt_forms::toolbar::TOOLBAR_DEF_PROP, def)]);
        let e = Empty::default();
        let n = tool_bar(&e.cx(&t), i);
        let buttons: Vec<&VNode> = n.children[0].children.iter().collect();
        assert_eq!(buttons.len(), 3);
        assert_eq!(buttons[0].prop_of("disabled"), Some(&super::super::Prop::Bool(false)));
        assert_eq!(buttons[1].prop_of("disabled"), Some(&super::super::Prop::Bool(true)), "print goes through the operating system");
        assert!(buttons[1].attr_of("data-unavailable").is_some());
        assert!(buttons[2].find_tag("svg").is_some());
        assert_eq!(buttons[2].attr_of("aria-label"), Some("Open file"));
    }

    #[test]
    fn a_route_round_trips_through_a_hash_with_awkward_characters() {
        let r = Route { form: "customer-form".into(), params: vec![("id".into(), "7".into()), ("q".into(), "a b&c=d/é".into())] };
        let h = r.to_hash();
        assert!(h.starts_with("#/customer-form?id=7&q="), "{h}");
        assert!(!h.contains(' ') && !h[2..].contains('/'), "{h}");
        assert_eq!(Route::parse(&h), Some(r));
        assert_eq!(Route::parse("#/orders"), Some(Route::new("orders")));
        assert_eq!(Route::parse("#top"), None, "an in-page anchor is not a route");
        assert_eq!(Route::parse(""), None);
        assert_eq!(Route::parse("#/"), None);
        assert_eq!(Route::parse("#/a/b"), None, "one form per route");
        assert_eq!(Route::parse("#/%E2%82%AC").unwrap().form, "\u{20AC}");
    }

    #[test]
    fn a_notification_takes_the_timeout_of_its_category_unless_the_template_sets_one() {
        let (t, i) = tree_with("Snackbar", "SNK-1", &[("Text", "Saved"), ("Category", "Error"), ("Buttons", "undo|Undo")]);
        let toast = mint_toast(&t, 0, i, 1, None, 1000);
        assert_eq!(toast.timeout_ms, 8000);
        assert_eq!(toast.text, "Saved");
        assert_eq!(toast.buttons.len(), 1);
        let (t, i) = tree_with("Snackbar", "SNK-1", &[("Category", "Critical")]);
        assert_eq!(mint_toast(&t, 0, i, 2, Some("Disk full"), 0).timeout_ms, 0, "a critical notice stays until dismissed");
        let (t, i) = tree_with("Snackbar", "SNK-1", &[("Timeout", "1500")]);
        assert_eq!(mint_toast(&t, 0, i, 3, None, 0).timeout_ms, 1500);
    }

    #[test]
    fn a_notification_is_a_status_and_an_error_is_an_alert() {
        let (t, i) = tree_with("Snackbar", "SNK-1", &[("Text", "x"), ("Category", "Error")]);
        let n = toast_view(&mint_toast(&t, 0, i, 1, None, 0));
        assert_eq!(n.attr_of("role"), Some("alert"));
        assert!(n.find(&|x| x.attr_of("data-part") == Some("toast-close")).is_some());
        let (t, i) = tree_with("Snackbar", "SNK-1", &[("Text", "x")]);
        assert_eq!(toast_view(&mint_toast(&t, 0, i, 1, None, 0)).attr_of("role"), Some("status"));
    }

    #[test]
    fn the_breadcrumb_links_every_entry_but_the_current_one() {
        let n = breadcrumb(&[Crumb { inst: 0, label: "Home".into() }, Crumb { inst: 4, label: "Customers".into() }], "");
        assert_eq!(n.count("button"), 1);
        assert_eq!(n.find(&|x| x.attr_of("aria-current") == Some("page")).unwrap().all_text(), "Customers");
    }
}
