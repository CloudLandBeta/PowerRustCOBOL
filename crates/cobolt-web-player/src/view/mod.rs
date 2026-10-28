// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! **What each control becomes in the browser, as plain data** (WASM-004 R3–R4, WASM-016 R5).
//!
//! Every function in this module tree takes a control's state and answers a [`VNode`]: an element,
//! its attributes, its text, its children and the DOM events it listens to. Nothing here touches the
//! browser, so the whole of "which element does a ListBox become, what does a click on its third row
//! do" is checked by ordinary native tests; the thin layer in `dom` only turns a `VNode` into a real
//! node and patches it when the next one differs.
//!
//! A `VNode` carries **text, never markup**: there is no field that holds an HTML string and no
//! function that would turn one into nodes (WASM-013 R5). Text reaches the page through
//! `textContent`; an attribute value reaches it through `setAttribute`.
//!
//! | Module | What it knows |
//! |---|---|
//! | [`controls`] | the class A controls: the native element each becomes |
//! | [`charts`] | the class B controls drawn with SVG: charts, Knob, Gauge, Shape |
//! | [`maps`] | the Maps tile grid and its overlays |
//! | [`viewer`] | the Viewer: a parsed document, allow-listed, as nodes |
//! | [`interact`] | what a DOM event on any of them asks of the host |
//! | [`shell`] | the application shell: routes, the SideMenu, the breadcrumb |

pub mod auth;
pub mod chart_paint;
pub mod charts;
pub mod controls;
#[cfg(test)]
mod contract;
pub mod data;
pub mod datagrid;
pub mod interact;
pub mod maps;
pub mod shell;
pub mod style;
pub mod svg;
pub mod viewer;

use std::collections::BTreeMap;

use cobolt_forms::menu::MenuDefinition;

use crate::tree::FormTree;

/// A property of the element itself (an IDL attribute): `value`, `checked`, `disabled`, …
#[derive(Debug, Clone, PartialEq)]
pub enum Prop {
    Str(String),
    Bool(bool),
    Num(f64),
}

/// Where the child controls of a control are mounted in its element tree.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Slot {
    /// Every child control (a Panel, a GroupBox).
    Children,
    /// The controls on one page of a TabControl (from 1).
    Page(u32),
    /// The controls in one pane of a Splitter (1 or 2).
    Pane(u8),
}

/// An element, described.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct VNode {
    pub tag: &'static str,
    pub svg: bool,
    /// Identity within its parent for the patch, and the routing key of its events.
    pub key: String,
    pub classes: Vec<String>,
    pub attrs: Vec<(String, String)>,
    pub props: Vec<(&'static str, Prop)>,
    /// Declarations applied through the CSSOM (never a `style` attribute: the page's CSP forbids it).
    pub styles: Vec<(String, String)>,
    pub text: Option<String>,
    pub children: Vec<VNode>,
    /// DOM event names this node listens to.
    pub on: Vec<&'static str>,
    pub slot: Option<Slot>,
}

impl VNode {
    pub fn new(tag: &'static str) -> VNode {
        VNode { tag, ..VNode::default() }
    }

    pub fn svg(tag: &'static str) -> VNode {
        VNode { tag, svg: true, ..VNode::default() }
    }

    pub fn key(mut self, k: impl Into<String>) -> VNode {
        self.key = k.into();
        self
    }

    /// Add class names (several may be given, separated by spaces).
    pub fn class(mut self, c: impl Into<String>) -> VNode {
        for part in c.into().split_whitespace() {
            if !self.classes.iter().any(|x| x == part) {
                self.classes.push(part.to_owned());
            }
        }
        self
    }

    pub fn classes(mut self, cs: impl IntoIterator<Item = String>) -> VNode {
        for c in cs {
            self = self.class(c);
        }
        self
    }

    pub fn attr(mut self, k: impl Into<String>, v: impl Into<String>) -> VNode {
        let k = k.into();
        let v = v.into();
        if let Some(slot) = self.attrs.iter_mut().find(|(a, _)| *a == k) {
            slot.1 = v;
        } else {
            self.attrs.push((k, v));
        }
        self
    }

    /// A boolean attribute: present (empty) when `on`, absent otherwise.
    pub fn flag(self, k: impl Into<String>, on: bool) -> VNode {
        if on {
            self.attr(k, "")
        } else {
            self
        }
    }

    pub fn prop(mut self, k: &'static str, v: Prop) -> VNode {
        if let Some(slot) = self.props.iter_mut().find(|(a, _)| *a == k) {
            slot.1 = v;
        } else {
            self.props.push((k, v));
        }
        self
    }

    pub fn prop_str(self, k: &'static str, v: impl Into<String>) -> VNode {
        self.prop(k, Prop::Str(v.into()))
    }

    pub fn prop_bool(self, k: &'static str, v: bool) -> VNode {
        self.prop(k, Prop::Bool(v))
    }

    pub fn prop_num(self, k: &'static str, v: f64) -> VNode {
        self.prop(k, Prop::Num(v))
    }

    pub fn style(mut self, k: impl Into<String>, v: impl Into<String>) -> VNode {
        let k = k.into();
        let v = v.into();
        if let Some(slot) = self.styles.iter_mut().find(|(a, _)| *a == k) {
            slot.1 = v;
        } else {
            self.styles.push((k, v));
        }
        self
    }

    pub fn text(mut self, t: impl Into<String>) -> VNode {
        self.text = Some(t.into());
        self
    }

    pub fn child(mut self, c: VNode) -> VNode {
        self.children.push(c);
        self
    }

    pub fn children(mut self, cs: impl IntoIterator<Item = VNode>) -> VNode {
        self.children.extend(cs);
        self
    }

    pub fn on(mut self, ev: &'static str) -> VNode {
        if !self.on.contains(&ev) {
            self.on.push(ev);
        }
        self
    }

    pub fn on_all(mut self, evs: impl IntoIterator<Item = &'static str>) -> VNode {
        for e in evs {
            self = self.on(e);
        }
        self
    }

    pub fn slot(mut self, s: Slot) -> VNode {
        self.slot = Some(s);
        self
    }

    // ── reading it back (tests, the accessibility check) ───────────────────

    pub fn attr_of(&self, k: &str) -> Option<&str> {
        self.attrs.iter().find(|(a, _)| a == k).map(|(_, v)| v.as_str())
    }

    pub fn prop_of(&self, k: &str) -> Option<&Prop> {
        self.props.iter().find(|(a, _)| *a == k).map(|(_, v)| v)
    }

    pub fn has_class(&self, c: &str) -> bool {
        self.classes.iter().any(|x| x == c)
    }

    /// The first node (this one included, depth first) for which `f` holds.
    pub fn find(&self, f: &dyn Fn(&VNode) -> bool) -> Option<&VNode> {
        if f(self) {
            return Some(self);
        }
        self.children.iter().find_map(|c| c.find(f))
    }

    pub fn find_tag(&self, tag: &str) -> Option<&VNode> {
        self.find(&|n| n.tag == tag)
    }

    pub fn find_all<'a>(&'a self, f: &dyn Fn(&VNode) -> bool, out: &mut Vec<&'a VNode>) {
        if f(self) {
            out.push(self);
        }
        for c in &self.children {
            c.find_all(f, out);
        }
    }

    pub fn count(&self, tag: &str) -> usize {
        let mut v = Vec::new();
        self.find_all(&|n| n.tag == tag, &mut v);
        v.len()
    }

    /// All the text under this node, in order.
    pub fn all_text(&self) -> String {
        let mut s = String::new();
        if let Some(t) = &self.text {
            s.push_str(t);
        }
        for c in &self.children {
            s.push_str(&c.all_text());
        }
        s
    }

    /// A readable outline of the node (for test failures and the conformance report).
    pub fn describe(&self) -> String {
        let mut out = String::new();
        self.describe_into(&mut out, 0);
        out
    }

    fn describe_into(&self, out: &mut String, depth: usize) {
        use std::fmt::Write;
        let pad = "  ".repeat(depth);
        let _ = write!(out, "{pad}<{}", self.tag);
        if !self.classes.is_empty() {
            let _ = write!(out, " .{}", self.classes.join("."));
        }
        for (k, v) in &self.attrs {
            let _ = write!(out, " {k}={v:?}");
        }
        for (k, v) in &self.props {
            let _ = write!(out, " ${k}={v:?}");
        }
        if !self.on.is_empty() {
            let _ = write!(out, " on[{}]", self.on.join(","));
        }
        if let Some(t) = &self.text {
            let _ = write!(out, " {t:?}");
        }
        let _ = writeln!(out, ">");
        for c in &self.children {
            c.describe_into(out, depth + 1);
        }
    }
}

/// Per-instance state the player keeps for the page only: nothing here is visible to the program,
/// which sees only properties (WASM-ABI-v1). It exists because a browser widget has transient state
/// a desktop painter keeps in its frame state.
#[derive(Debug, Clone, Default)]
pub struct UiState {
    /// The menu items open, outermost first (a MenuBar dropdown).
    pub open_menu: Vec<String>,
    /// How far a DataGrid has been scrolled, by control: `(left, top)` in pixels, as the page last reported it.
    pub grid_scroll: BTreeMap<usize, (f64, f64)>,
    /// The DataGrid column sorted by the user and its direction.
    pub grid_sort: BTreeMap<usize, (usize, bool)>,
    /// A ComboBox whose list is open (a custom popup is never used: the native `<select>` opens itself).
    pub focused: Option<usize>,
    /// How many matches the page found for each Viewer's search text, by control (the page counts them in the text it shows).
    pub find_total: BTreeMap<usize, usize>,
    /// The document a Viewer's `Source` named, once the page has read it.
    pub viewer_docs: BTreeMap<usize, viewer::LoadedDoc>,
    /// The `Source` value already asked for, by control (a document is read once per value).
    pub viewer_src: BTreeMap<usize, String>,
    /// The Mermaid diagrams the part has drawn (or failed to), by their source text (WASM-016 R13).
    pub mermaid: BTreeMap<String, MermaidState>,
    /// The SideMenu rail's state, an application-level preference that survives navigation (spec 049
    /// R9): `None` until the user chooses, then the choice (the control's `Collapsed` is the start).
    pub sidebar_collapsed: Option<bool>,
    /// The FileDropZone a drag is over.
    pub dropzone_over: Option<usize>,
    /// A pointer drag on a Maps control: where it began `(x, y, centre latitude, centre longitude)`.
    pub map_drag: BTreeMap<usize, (f64, f64, f64, f64)>,
    /// The size the browser gave a control's box, by control, in CSS pixels. A control that is drawn
    /// to its own size (a chart, a gauge, a caption that shrinks to fit) is drawn for THIS, not for the
    /// size it was designed at: layout may have grown or shrunk it, as the desktop's solver does before
    /// its painter draws (`ViewCtx::box_size`). Filled by the page from a `ResizeObserver`; empty until
    /// the first layout has been measured.
    pub box_sizes: BTreeMap<usize, (f64, f64)>,
    /// A chart's motion at this frame, by control (`AnimateOnLoad`'s growth, `AnimateValues`' travel; `crate::motion`):
    /// what its painter draws instead of its own `__ChartGrow` and `__ChartData` while it moves. Empty at rest.
    pub chart_frames: BTreeMap<usize, crate::motion::ChartFrame>,
}

/// Where a Mermaid diagram stands: the Mermaid part is a module of its own, loaded the first time a
/// Viewer shows one, so the answer comes later than the document.
#[derive(Debug, Clone, PartialEq)]
pub enum MermaidState {
    /// Asked for; no answer yet.
    Loading,
    /// The SVG text the part answered. It is read by the allow-listed reader, never inserted.
    Ready(String),
    /// Why it could not be drawn (a kind the Viewer does not draw, a source the renderer refused, a
    /// part that could not be loaded).
    Failed(String),
}

/// Menu definitions delivered with the bundle, by `(form name lower-case, control name upper-case)`.
#[derive(Debug, Clone, Default)]
pub struct MenuStore {
    pub menus: BTreeMap<(String, String), MenuDefinition>,
}

impl MenuStore {
    pub fn get(&self, form: &str, control: &str) -> Option<&MenuDefinition> {
        self.menus.get(&(form.to_ascii_lowercase(), control.to_ascii_uppercase()))
    }

    pub fn insert(&mut self, form: &str, control: &str, def: MenuDefinition) {
        self.menus.insert((form.to_ascii_lowercase(), control.to_ascii_uppercase()), def);
    }
}

/// Everything a view function may read.
pub struct ViewCtx<'a> {
    pub tree: &'a FormTree,
    /// The asset table: the path a form uses (lower-cased) → the URL the bundle serves it at.
    pub assets: &'a BTreeMap<String, String>,
    pub locale: &'a str,
    pub ui: &'a UiState,
    pub menus: &'a MenuStore,
    /// Origins the application may load images from besides its own files (the Maps tile server, the
    /// backend); a `src` outside this list and the asset table is refused.
    pub image_origins: &'a [String],
    /// The tile URL template of the Maps control: `{z}`, `{x}`, `{y}` (WASM-004 Maps).
    pub tile_url: &'a str,
}

/// A view context with nothing in it, for tests.
pub struct Empty {
    pub assets: BTreeMap<String, String>,
    pub ui: UiState,
    pub menus: MenuStore,
}

impl Default for Empty {
    fn default() -> Self {
        Empty { assets: BTreeMap::new(), ui: UiState::default(), menus: MenuStore::default() }
    }
}

impl Empty {
    pub fn cx<'a>(&'a self, tree: &'a FormTree) -> ViewCtx<'a> {
        ViewCtx {
            tree,
            assets: &self.assets,
            locale: "en",
            ui: &self.ui,
            menus: &self.menus,
            image_origins: &[],
            tile_url: "https://tile.openstreetmap.org/{z}/{x}/{y}.png",
        }
    }
}

impl ViewCtx<'_> {
    /// The size control `i` is drawn for: the box the browser gave it when the page has measured one,
    /// else the size it was designed at (`Width` × `Height`).
    pub fn box_size(&self, i: usize) -> (f64, f64) {
        if let Some(s) = self.ui.box_sizes.get(&i) {
            return *s;
        }
        let dim = |k: &str| crate::value::parse_f64(&self.tree.get(i, k)).unwrap_or(0.0);
        (dim("Width"), dim("Height"))
    }

    /// Resolve a path a form wrote (`Assets/logo.png`) to a URL the page may load, or `None`.
    /// A `data:` image, a path in the asset table and an absolute URL of an allowed origin are
    /// accepted; nothing else is (WASM-013: the page loads only what the application shipped).
    pub fn image_url(&self, path: &str) -> Option<String> {
        let p = path.trim();
        if p.is_empty() {
            return None;
        }
        let lower = p.to_ascii_lowercase();
        if lower.starts_with("data:image/") && !lower.starts_with("data:image/svg") {
            return Some(p.to_owned());
        }
        let normal = lower.replace('\\', "/");
        if let Some(u) = self.assets.get(&normal).or_else(|| self.assets.get(normal.trim_start_matches("./"))) {
            return Some(u.clone());
        }
        if lower.starts_with("https://") || lower.starts_with("http://") {
            if self.image_origins.iter().any(|o| lower.starts_with(&o.to_ascii_lowercase())) {
                return Some(p.to_owned());
            }
        }
        None
    }
}
