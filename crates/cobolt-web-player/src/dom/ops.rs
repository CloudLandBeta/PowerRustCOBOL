// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! **What the program and the user ask of the page itself**: effects (a dialog, a download, a window
//! to open), page-state changes (an open menu, a sorted grid) and the acts the host cannot perform
//! alone (resolve a menu item, show a notice).

use std::rc::Rc;

use js_sys::Reflect;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;

use super::h;
use super::{js_num, Shared};
use crate::api::{Inbox, Inner, Timeout};
use crate::host::{Effect, OpenMode};
use crate::view::interact::{self, Act, MenuTarget, UiOp};
use crate::view::shell::{DialogSpec, Route as AppRoute, Toast};
use crate::view::{Empty, ViewCtx};
use cobolt_forms::snackbar::SnackCategory;

impl Shared {
    pub(super) fn on_effect(self: &Rc<Self>, inner: &Rc<Inner>, e: &Effect) {
        match e {
            Effect::RuntimeError(r) => {
                self.set_banner(Some((r.describe(), r.code == cobolt_wasm_abi::codes::err::TRAPPED)));
            }
            Effect::Dialog { id, kind, text, title, buttons, default } => {
                self.show_dialog(DialogSpec { id: *id, kind: *kind, title: title.clone(), text: text.clone(), buttons: buttons.clone(), default: default.clone() });
            }
            Effect::OpenWindow { handle, form, sync, params, opener } => self.open_window(inner, handle, *form, *sync, params, *opener),
            Effect::Stopped => self.show_ended(),
            Effect::Title(t) => {
                if inner.cfg.standalone || self.state.try_borrow().map(|s| s.chain.len() <= 1).unwrap_or(true) {
                    self.doc.set_title(t);
                }
            }
            Effect::Download { name, text } => self.download(name, text),
            Effect::Clipboard(text) => {
                if let Some(w) = web_sys::window() {
                    let nav: JsValue = w.navigator().into();
                    if let Ok(cb) = Reflect::get(&nav, &"clipboard".into()) {
                        if cb.is_object() {
                            let _ = Reflect::get(&cb, &"writeText".into()).ok().and_then(|f| f.dyn_into::<js_sys::Function>().ok()).map(|f| f.call1(&cb, &text.as_str().into()));
                        }
                    }
                }
            }
            Effect::Navigate { url, target } => self.navigate(url, target),
            Effect::Fullscreen(on) => {
                if *on {
                    let _ = Reflect::get(&self.root, &"requestFullscreen".into()).ok().and_then(|f| f.dyn_into::<js_sys::Function>().ok()).map(|f| f.call0(&self.root));
                } else {
                    let d: JsValue = self.doc.clone().into();
                    let _ = Reflect::get(&d, &"exitFullscreen".into()).ok().and_then(|f| f.dyn_into::<js_sys::Function>().ok()).map(|f| f.call0(&d));
                }
            }
            Effect::Notice(n) => self.notice(n),
            // The driver carries these out itself (it owns the connection and the reCAPTCHA script).
            Effect::Display(_)
            | Effect::Log { .. }
            | Effect::Timer { .. }
            | Effect::Opened { .. }
            | Effect::Closed { .. }
            | Effect::SuperSet { .. }
            | Effect::SuperCall { .. }
            | Effect::AuthCall { .. }
            | Effect::AuthCaptcha { .. } => {}
        }
    }

    /// A short message for the user, as a notification that belongs to no control.
    pub(crate) fn notice(self: &Rc<Self>, text: &str) {
        let toast = {
            let Ok(mut st) = self.state.try_borrow_mut() else { return };
            let id = st.next_toast;
            st.next_toast += 1;
            Toast {
                id,
                inst: u32::MAX,
                ctl: usize::MAX,
                text: text.to_owned(),
                category: SnackCategory::Info,
                buttons: Vec::new(),
                timeout_ms: 6000,
                pause_on_hover: true,
                shown_at_ms: crate::api::now_ms(),
            }
        };
        let id = toast.id;
        if let Ok(mut st) = self.state.try_borrow_mut() {
            st.toasts.push(toast);
            while st.toasts.len() > 5 {
                st.toasts.remove(0);
            }
        }
        self.render_toasts();
        let weak = Rc::downgrade(self);
        let t = Timeout::new(6000, move || {
            if let Some(sh) = weak.upgrade() {
                sh.dismiss_toast(id);
            }
        });
        if let Ok(mut st) = self.state.try_borrow_mut() {
            st.timers.insert((u32::MAX - id, 0), (id as u64, t));
        }
    }

    fn download(&self, name: &str, text: &str) {
        // A Blob and an object URL: the bytes stay in the page; nothing is sent anywhere.
        let parts = js_sys::Array::new();
        parts.push(&text.into());
        let Ok(blob) = web_sys::Blob::new_with_str_sequence(&parts) else { return };
        let Ok(url) = web_sys::Url::create_object_url_with_blob(&blob) else { return };
        let a = h::el(&self.doc, "a", &[]);
        h::attr(&a, "href", &url);
        let safe: String = name.chars().filter(|c| !c.is_control() && !matches!(c, '/' | '\\')).collect();
        h::attr(&a, "download", if safe.is_empty() { "download" } else { &safe });
        h::append(&self.root, &a);
        if let Ok(h) = a.clone().dyn_into::<web_sys::HtmlElement>() {
            h.click();
        }
        a.remove();
        let _ = web_sys::Url::revoke_object_url(&url);
    }

    fn navigate(self: &Rc<Self>, url: &str, target: &str) {
        let policy = self.inner.upgrade().and_then(|i| i.host.try_borrow().ok().map(|h| h.policy.clone())).unwrap_or_default();
        match crate::weblink::check(url, false, &policy) {
            Ok(_) => {
                let norm = crate::weblink::normalize(url);
                if let Some(w) = web_sys::window() {
                    let t = crate::weblink::Target::parse(target);
                    match t {
                        crate::weblink::Target::SelfFrame => {
                            let _ = w.location().assign(&norm);
                        }
                        other => {
                            let _ = w.open_with_url_and_target_and_features(&norm, other.html(), "noopener,noreferrer");
                        }
                    }
                }
            }
            Err(r) => self.notice(&r.message),
        }
    }

    // ── a form in a window of its own (WASM-011 R9–R12) ─────────────────────

    fn open_window(self: &Rc<Self>, inner: &Rc<Inner>, handle: &str, form: usize, _sync: bool, params: &[(String, String)], opener: u32) {
        let name = inner.host.try_borrow().ok().map(|h| h.manifest.forms[form].name.clone()).unwrap_or_default();
        let base = if inner.cfg.app_url.trim().is_empty() {
            web_sys::window().and_then(|w| w.location().href().ok()).unwrap_or_default().split('#').next().unwrap_or("").split('?').next().unwrap_or("").to_owned()
        } else {
            inner.cfg.app_url.clone()
        };
        // The window gets a private channel to this one (WASM-011 R11): `super::` there is this form.
        let token = inner.super_open(opener);
        let mut url = format!("{base}?pr-form={}&pr-opener={}&pr-super={}", urlencode(&name), urlencode(handle), urlencode(&token));
        for (k, v) in params {
            url.push_str(&format!("&pr-param-{}={}", urlencode(k), urlencode(v)));
        }
        let win = web_sys::window().and_then(|w| w.open_with_url_and_target_and_features(&url, "_blank", "noopener=0").ok().flatten());
        match win {
            Some(win) => self.watch_window(handle.to_owned(), win, 0),
            None => {
                // A popup blocker refused it (a call from a timer, or after a wait): tell the user how
                // to allow it, and release the suspended caller with a failure, never leave it hanging.
                self.notice("The browser blocked the new window. Allow pop-ups for this site and try again.");
                inner.push(Inbox::ChildClosed(handle.to_owned()));
            }
        }
    }

    /// Poll a window the application opened; when it closes, release the form that waited on it.
    fn watch_window(self: &Rc<Self>, handle: String, win: web_sys::Window, n: u32) {
        let weak = Rc::downgrade(self);
        let key = (u32::MAX - 0x4000 - (n % 0x3000), 1usize);
        let h2 = handle.clone();
        let w2 = win.clone();
        let t = Timeout::new(500, move || {
            let Some(sh) = weak.upgrade() else { return };
            if w2.closed().unwrap_or(true) {
                if let Some(inner) = sh.inner.upgrade() {
                    inner.push(Inbox::ChildClosed(h2.clone()));
                }
                if let Ok(mut st) = sh.state.try_borrow_mut() {
                    st.timers.remove(&key);
                }
            } else {
                sh.watch_window(h2.clone(), w2.clone(), n + 1);
            }
        });
        if let Ok(mut st) = self.state.try_borrow_mut() {
            st.timers.insert(key, (n as u64, t));
        }
    }

    // ── page state ──────────────────────────────────────────────────────────

    pub(super) fn ui_op(self: &Rc<Self>, inner: &Rc<Inner>, inst: u32, ctl: usize, op: UiOp) {
        match op {
            UiOp::SetOpenMenu(v) => self.with_ui(inst, |ui| ui.open_menu = v),
            UiOp::GridScroll { left, top } => self.with_ui(inst, |ui| {
                ui.grid_scroll.insert(ctl, (left, top));
            }),
            UiOp::GridScrollTo { top } => {
                self.with_ui(inst, |ui| {
                    let left = ui.grid_scroll.get(&ctl).map(|s| s.0).unwrap_or(0.0);
                    ui.grid_scroll.insert(ctl, (left, top));
                });
                if let Some(el) = self.control_element(inst, ctl) {
                    if let Ok(Some(scroller)) = el.query_selector(".pr-dg-scroll") {
                        scroller.set_scroll_top(top.round() as i32);
                    }
                }
            }
            UiOp::GridSort { col, descending } => self.with_ui(inst, |ui| {
                ui.grid_sort.insert(ctl, (col, descending));
            }),
            UiOp::Sidebar(collapsed) => {
                self.with_ui(inst, |ui| ui.sidebar_collapsed = Some(collapsed));
                self.persist_sidebar(collapsed);
                self.update_rail(inner, inst, ctl, collapsed);
            }
            UiOp::DropOver(on) => self.with_ui(inst, |ui| ui.dropzone_over = if on { Some(ctl) } else { None }),
            UiOp::Editing(on) => self.with_ui(inst, |ui| {
                if on {
                    ui.focused = Some(ctl);
                } else if ui.focused == Some(ctl) {
                    ui.focused = None;
                }
            }),
            UiOp::MapDrag { x, y, lat, lng } => self.with_ui(inst, |ui| {
                ui.map_drag.insert(ctl, (x, y, lat, lng));
            }),
            UiOp::MapDragEnd => self.with_ui(inst, |ui| {
                ui.map_drag.remove(&ctl);
            }),
            UiOp::Focus => {
                if let Some(el) = self.control_element(inst, ctl) {
                    h::focus(&el);
                }
                return;
            }
            UiOp::Dismiss(id) => {
                self.dismiss_toast(id);
                return;
            }
            UiOp::ShowValue(text) => {
                if let Some(el) = self.control_element(inst, ctl) {
                    if let Ok(Some(input)) = el.query_selector("input") {
                        let _ = js_sys::Reflect::set(&input, &"value".into(), &text.as_str().into());
                    }
                }
                return;
            }
            UiOp::PickFiles => {
                if let Some(el) = self.control_element(inst, ctl) {
                    if let Ok(Some(input)) = el.query_selector("input[type=file]") {
                        if let Ok(i) = input.dyn_into::<web_sys::HtmlElement>() {
                            i.click();
                        }
                    }
                }
                return;
            }
        }
        self.mark_dirty(inst, ctl);
    }

    pub(crate) fn with_ui(&self, inst: u32, f: impl FnOnce(&mut crate::view::UiState)) {
        if let Ok(mut st) = self.state.try_borrow_mut() {
            if let Some(iv) = st.instances.get_mut(&inst) {
                f(&mut iv.ui);
            }
        }
    }

    fn storage_key(&self) -> String {
        let loc = web_sys::window().map(|w| w.location());
        let path = loc.as_ref().and_then(|l| l.pathname().ok()).unwrap_or_default();
        let origin = loc.and_then(|l| l.origin().ok()).unwrap_or_default();
        format!("pr:sidebar:{origin}{path}")
    }

    /// The rail's state is a preference of the user, remembered per application (spec 049 R9).
    fn persist_sidebar(&self, collapsed: bool) {
        if let Some(Ok(Some(s))) = web_sys::window().map(|w| w.local_storage()) {
            let _ = s.set_item(&self.storage_key(), if collapsed { "1" } else { "0" });
        }
    }

    pub(crate) fn saved_sidebar(&self) -> Option<bool> {
        let s = web_sys::window()?.local_storage().ok().flatten()?;
        match s.get_item(&self.storage_key()).ok().flatten().as_deref() {
            Some("1") => Some(true),
            Some("0") => Some(false),
            _ => None,
        }
    }

    /// The ContentPane starts where the rail ends (spec 049 R10), and the rail ends where the designer draws it:
    /// the control's designed x plus its width (a rail at x = -8, 264 wide, ends at 256). The rail takes its
    /// width from `--pr-rail`, set on the main surface it sits in; the pane is a sibling of that surface and
    /// takes its origin from `--pr-rail-end`, set on the pane itself.
    pub(crate) fn update_rail(self: &Rc<Self>, inner: &Rc<Inner>, inst: u32, ctl: usize, collapsed: bool) {
        let (x, width) = {
            let Ok(host) = inner.host.try_borrow() else { return };
            let Some(tree) = host.trees.get(&inst) else { return };
            let x = crate::value::parse_f64(&tree.get(ctl, "X")).unwrap_or(0.0);
            let w = crate::value::parse_f64(&tree.get(ctl, "Width")).unwrap_or(220.0);
            let cw = crate::value::parse_f64(&tree.get(ctl, "CollapsedWidth")).unwrap_or(48.0);
            (x, if collapsed { cw } else { w })
        };
        h::css(&self.chrome.main, "--pr-rail", &format!("{}px", width.round()));
        h::css(&self.chrome.pane, "--pr-rail-end", &format!("{}px", (x + width).max(0.0).round()));
    }

    // ── acts the host cannot perform alone ──────────────────────────────────

    pub(super) fn act(self: &Rc<Self>, inner: &Rc<Inner>, inst: u32, ctl: usize, act: Act) {
        match act {
            Act::Menu { item } => self.resolve_menu_item(inner, inst, ctl, &item),
            Act::OpenModal { form } => {
                let mode = if inner.cfg.standalone { OpenMode::Window } else { OpenMode::Modal };
                self.open_named(inner, &form, Some(inst), mode, Vec::new());
            }
            Act::Notice(t) => self.notice(&t),
            Act::Download { name, text } => self.download(&name, &text),
            Act::OpenWindow { url, target, features, rel } => {
                let feat = if rel.contains("noopener") || features.is_empty() { format!("{features},noopener") } else { features };
                if let Some(w) = web_sys::window() {
                    let _ = w.open_with_url_and_target_and_features(&url, &target, feat.trim_start_matches(','));
                }
            }
            Act::Files { .. } => {}
            _ => {}
        }
    }

    fn open_named(self: &Rc<Self>, inner: &Rc<Inner>, form: &str, opener: Option<u32>, mode: OpenMode, params: Vec<(String, String)>) {
        let idx = inner.host.try_borrow().ok().and_then(|h| h.manifest.form_by_name(form).map(|f| f.id as usize));
        match idx {
            Some(i) => {
                inner.host.borrow_mut().open_form_with(i, opener, mode, params);
            }
            None => self.notice(&format!("The application has no form named {form}.")),
        }
    }

    fn resolve_menu_item(self: &Rc<Self>, inner: &Rc<Inner>, inst: u32, ctl: usize, item: &str) {
        let target = {
            let Ok(host) = inner.host.try_borrow() else { return };
            let Some(tree) = host.trees.get(&inst) else { return };
            let Ok(styles) = inner.styles.try_borrow() else { return };
            let empty = Empty::default();
            let ui = crate::view::UiState::default();
            let tile = String::new();
            let cx = ViewCtx { tree, assets: &empty.assets, locale: "", ui: &ui, menus: &styles.menus, image_origins: &[], tile_url: &tile };
            interact::item_of(&cx, ctl, item).map(|i| interact::menu_target(&i)).unwrap_or(MenuTarget::None)
        };
        let shell_on = self.state.try_borrow().map(|s| s.shell).unwrap_or(false);
        match target {
            MenuTarget::Embed { form, preserve_previous } => {
                if let Ok(mut st) = self.state.try_borrow_mut() {
                    st.preserve_next = preserve_previous;
                }
                let mode = if shell_on { OpenMode::Embedded } else { OpenMode::Panel };
                self.open_named(inner, &form, Some(inst), mode, Vec::new());
            }
            MenuTarget::Standalone { form, sync } => {
                let mode = if inner.cfg.standalone {
                    OpenMode::Window
                } else if sync {
                    OpenMode::Modal
                } else {
                    OpenMode::Panel
                };
                self.open_named(inner, &form, Some(inst), mode, Vec::new());
            }
            MenuTarget::None => {}
        }
    }

    // ── the Viewer's document ───────────────────────────────────────────────

    /// Read the document a Viewer's `Source` names, once per value. Only a file of the application
    /// (the asset table) is ever fetched: a path outside it is an error shown in the control.
    pub(crate) fn ensure_viewer_docs(self: &Rc<Self>, inner: &Rc<Inner>, inst: u32) {
        let todo: Vec<(usize, String, Option<String>)> = {
            let Ok(host) = inner.host.try_borrow() else { return };
            let Some(tree) = host.trees.get(&inst) else { return };
            let Ok(styles) = inner.styles.try_borrow() else { return };
            let Ok(st) = self.state.try_borrow() else { return };
            let Some(iv) = st.instances.get(&inst) else { return };
            tree.ctls
                .iter()
                .enumerate()
                .filter(|(_, c)| matches!(c.control_type(), Some(cobolt_forms::model::ControlType::Viewer)))
                .filter_map(|(i, _)| {
                    let src = {
                        let s = tree.get(i, "Source");
                        if s.trim().is_empty() {
                            tree.get(i, "View1Source")
                        } else {
                            s
                        }
                    };
                    let src = src.trim().to_owned();
                    if src.is_empty() || iv.ui.viewer_src.get(&i) == Some(&src) {
                        return None;
                    }
                    let key = src.to_ascii_lowercase().replace('\\', "/");
                    let url = styles.assets.get(&key).cloned();
                    Some((i, src, url))
                })
                .collect()
        };
        for (ctl, src, url) in todo {
            self.with_ui(inst, |ui| {
                ui.viewer_src.insert(ctl, src.clone());
                ui.viewer_docs.remove(&ctl);
            });
            let weak = Rc::downgrade(self);
            let inner = inner.clone();
            wasm_bindgen_futures::spawn_local(async move {
                let is_pdf = src.to_ascii_lowercase().ends_with(".pdf");
                let doc = match url {
                    None => crate::view::viewer::LoadedDoc::failed(format!("`{src}` is not a file of this application")),
                    // A PDF is read by its part (a module of its own, loaded the first time one is shown).
                    Some(u) if is_pdf => match fetch_bytes(&u).await {
                        Ok(bytes) => match inner.call_part_bytes("pdf", "read", &bytes).await {
                            Ok(json) => crate::view::viewer::LoadedDoc::from_pdf_json(&json),
                            Err(e) => crate::view::viewer::LoadedDoc::failed(format!("`{src}` could not be read as a PDF ({e})")),
                        },
                        Err(e) => crate::view::viewer::LoadedDoc::failed(format!("`{src}` could not be read ({e})")),
                    },
                    Some(u) => match fetch_text(&u).await {
                        Ok(text) => crate::view::viewer::LoadedDoc::from_text(&src, text),
                        Err(e) => crate::view::viewer::LoadedDoc::failed(format!("`{src}` could not be read ({e})")),
                    },
                };
                if let Some(sh) = weak.upgrade() {
                    sh.with_ui(inst, |ui| {
                        ui.viewer_docs.insert(ctl, doc);
                    });
                    sh.mark_dirty(inst, ctl);
                }
            });
        }
    }

    /// The Mermaid diagrams the instance's Viewers show that the Mermaid part has not been asked about:
    /// ask once each (WASM-016 R13). The part is a module of its own that the loader fetches the first
    /// time this runs, so a form with no diagram never pays for it; the diagram reads "loading" until
    /// the part answers, and the Viewers are redrawn when it does.
    pub(crate) fn ensure_mermaid(self: &Rc<Self>, inner: &Rc<Inner>, inst: u32) {
        let (wanted, viewers): (Vec<String>, Vec<usize>) = {
            let Ok(host) = inner.host.try_borrow() else { return };
            let Some(tree) = host.trees.get(&inst) else { return };
            let Ok(styles) = inner.styles.try_borrow() else { return };
            let Ok(st) = self.state.try_borrow() else { return };
            let Some(iv) = st.instances.get(&inst) else { return };
            let tile = self.tile_url(inner);
            let cx = ViewCtx { tree, assets: &styles.assets, locale: &styles.locale, ui: &iv.ui, menus: &styles.menus, image_origins: &inner.cfg.image_origins, tile_url: &tile };
            let mut wanted: Vec<String> = Vec::new();
            let mut viewers = Vec::new();
            for (i, c) in tree.ctls.iter().enumerate() {
                if matches!(c.control_type(), Some(cobolt_forms::model::ControlType::Viewer)) {
                    viewers.push(i);
                    for s in crate::view::viewer::mermaid_sources(&cx, i) {
                        if !iv.ui.mermaid.contains_key(&s) && !wanted.contains(&s) {
                            wanted.push(s);
                        }
                    }
                }
            }
            (wanted, viewers)
        };
        for source in wanted {
            self.with_ui(inst, |ui| {
                ui.mermaid.insert(source.clone(), crate::view::MermaidState::Loading);
            });
            let weak = Rc::downgrade(self);
            let inner = inner.clone();
            let viewers = viewers.clone();
            wasm_bindgen_futures::spawn_local(async move {
                let state = match inner.call_part("mermaid", "render", &source).await {
                    Ok(svg) => crate::view::MermaidState::Ready(svg),
                    Err(e) => crate::view::MermaidState::Failed(format!("the diagram could not be drawn ({e})")),
                };
                if let Some(sh) = weak.upgrade() {
                    sh.with_ui(inst, |ui| {
                        ui.mermaid.insert(source, state);
                    });
                    for v in viewers {
                        sh.mark_dirty(inst, v);
                    }
                }
            });
        }
    }

    // ── the address (WASM-011 R4) ───────────────────────────────────────────

    /// The address names a form: show it (a deep link at start-up, the Back button later).
    pub(crate) fn follow_address(self: &Rc<Self>, inner: &Rc<Inner>) {
        let hash = web_sys::window().and_then(|w| w.location().hash().ok()).unwrap_or_default();
        let route = AppRoute::parse(&hash);
        // An empty address is where the application started: the main form (Back past the first route).
        // Any other address that is not a route (`#section`) is the page's own and none of ours.
        if route.is_none() && !(hash.trim().is_empty() || hash.trim() == "#") {
            return;
        }
        let (shell_on, in_chain, above): (bool, bool, Vec<u32>) = {
            let Ok(st) = self.state.try_borrow() else { return };
            match &route {
                None => (st.shell, true, st.chain.iter().skip(1).rev().map(|c| c.inst).collect()),
                Some(route) => {
                    let at = st.chain.iter().position(|c| st.instances.get(&c.inst).is_some_and(|iv| iv.form.eq_ignore_ascii_case(&route.form)));
                    // The form is already shown, but opened with other parameters than the address names: it is
                    // opened again (the shown one is replaced), not left as it was.
                    let same_params = |inst: u32| inner.host.try_borrow().ok().and_then(|h| h.trees.get(&inst).map(|t| t.params == route.params)).unwrap_or(true);
                    match at {
                        Some(a) if same_params(st.chain[a].inst) => (st.shell, true, st.chain[a + 1..].iter().rev().map(|c| c.inst).collect()),
                        _ => (st.shell, false, Vec::new()),
                    }
                }
            }
        };
        if !shell_on {
            return;
        }
        if in_chain {
            for i in above {
                inner.push(Inbox::CloseInstance(i));
            }
        } else if let Some(route) = &route {
            let main = self.state.try_borrow().ok().and_then(|st| st.chain.first().map(|c| c.inst));
            if let Some(main) = main {
                self.open_named(inner, &route.form, Some(main), OpenMode::Embedded, route.params.clone());
            }
        }
    }

    /// A popstate handler that follows the address, without loops: the player's own `pushState` does not fire `popstate`.
    pub(crate) fn watch_address(self: &Rc<Self>) -> Option<h::Listener> {
        let w = web_sys::window()?;
        let weak = Rc::downgrade(self);
        let target: web_sys::EventTarget = w.into();
        Some(h::Listener::new(&target, "popstate", move |_| {
            if let Some(sh) = weak.upgrade() {
                if let Some(inner) = sh.inner.upgrade() {
                    inner.push(Inbox::Raw(Box::new(move |inner| sh.follow_address(inner))));
                }
            }
        }))
    }
}

/// `fetch(url)` of a file of the bundle, as text.
async fn fetch_bytes(url: &str) -> Result<Vec<u8>, String> {
    let w = web_sys::window().ok_or("no window")?;
    let resp = wasm_bindgen_futures::JsFuture::from(w.fetch_with_str(url)).await.map_err(|e| format!("{e:?}"))?;
    if !Reflect::get(&resp, &"ok".into()).ok().and_then(|v| v.as_bool()).unwrap_or(false) {
        return Err(format!("HTTP {}", js_num(&resp, "status")));
    }
    let f = Reflect::get(&resp, &"arrayBuffer".into()).ok().and_then(|f| f.dyn_into::<js_sys::Function>().ok()).ok_or("no arrayBuffer()")?;
    let p = f.call0(&resp).map_err(|e| format!("{e:?}"))?;
    let v = wasm_bindgen_futures::JsFuture::from(js_sys::Promise::from(p)).await.map_err(|e| format!("{e:?}"))?;
    Ok(js_sys::Uint8Array::new(&v).to_vec())
}

async fn fetch_text(url: &str) -> Result<String, String> {
    let w = web_sys::window().ok_or("no window")?;
    let resp = wasm_bindgen_futures::JsFuture::from(w.fetch_with_str(url)).await.map_err(|e| format!("{e:?}"))?;
    if !Reflect::get(&resp, &"ok".into()).ok().and_then(|v| v.as_bool()).unwrap_or(false) {
        return Err(format!("HTTP {}", js_num(&resp, "status")));
    }
    let text = Reflect::get(&resp, &"text".into()).ok().and_then(|f| f.dyn_into::<js_sys::Function>().ok()).ok_or("no text()")?;
    let p = text.call0(&resp).map_err(|e| format!("{e:?}"))?;
    let v = wasm_bindgen_futures::JsFuture::from(js_sys::Promise::from(p)).await.map_err(|e| format!("{e:?}"))?;
    v.as_string().ok_or_else(|| "not text".to_owned())
}

fn urlencode(s: &str) -> String {
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
