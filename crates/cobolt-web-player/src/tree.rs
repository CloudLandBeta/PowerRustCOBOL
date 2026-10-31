// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! The state of one form **instance**: its controls and their properties as text.
//!
//! This is the player's counterpart of the desktop host's `state` map and of the interpreter's
//! `objects` table, and it follows the same rules because the values must mean the same on
//! every surface: properties are text; a boolean reads back as `true`/`false`; a state
//! property of a toggle is written under both of its spellings; one radio at a time is on
//! within a group; an *observer* event (`onChange`, `onVisibleChanged`, …) is raised when a
//! program's write actually changes a value.
//!
//! Nothing here touches the DOM. The renderer reads [`FormTree::take_changes`] and projects
//! them; a test reads the state directly. That separation is what lets the whole ABI host be
//! driven natively against a real module (`tests/abi_host.rs`).

use std::collections::{BTreeMap, HashMap};

use cobolt_forms::layout::breakpoints::Breakpoint;
use cobolt_forms::layout::fonts;
use cobolt_forms::layout::props::FormBag;
use cobolt_forms::model::{
    is_toggle_state_property, property_runtime_text, radio_group_key, Control, ControlType, Form, PropValue, CHECKED_PROP,
    SELECTED_PROP,
};
use cobolt_wasm_abi::manifest::{FormEntry, Manifest};
use indexmap::IndexMap;

use crate::value::{fmt_number, parse_f64, turned_on};

/// What a [`Ctl`] is: the form itself (ABI control id 0) or one of its controls.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Kind {
    Form,
    Control(ControlType),
}

impl Kind {
    pub fn type_name(&self) -> String {
        match self {
            Kind::Form => "Form".to_owned(),
            Kind::Control(t) => t.as_str().to_owned(),
        }
    }
}

/// One control (or the form) of an instance.
#[derive(Debug, Clone)]
pub struct Ctl {
    /// The control id as the developer named it (`BTN-OK`); the form's name for the form.
    pub name: String,
    /// The ABI control id (§5): `0` for the form, `3…` for a control listed in the manifest,
    /// `None` for a control the program never addresses.
    pub abi_id: Option<u32>,
    pub kind: Kind,
    /// Index of the container in [`FormTree::ctls`] (the form for a direct child).
    pub parent: Option<usize>,
    /// For a control whose parent is a TabControl: the page it sits on, from 1.
    pub tab: Option<u32>,
    pub children: Vec<usize>,
    /// Lower-case property name → stored text.
    pub props: IndexMap<String, String>,
    /// Event names (as the manifest spells them, lower-cased) with a handler bound.
    pub bound: Vec<String>,
    /// Bumped on every change; the renderer compares it to know what to redraw.
    pub rev: u64,
    /// The layer this control sits in, as an index into [`FormTree::layers`] (its place in the stack);
    /// `None` is the base. Only a direct child of the form carries one: a control inside a container
    /// belongs to its container's layer (spec 091 R8), and a name the form does not define is the base (R40).
    pub layer: Option<usize>,
    /// A Timer ticks while this is on (the desktop renderer, `render.rs` CT::Timer): its own `Enabled`
    /// *property*, on when the design has none, and whatever the program last wrote to `Enabled` since.
    /// The element's `enabled` flag never starts or stops it, though it is what `TMR::Enabled` reads at
    /// load (the desktop seeds the flag over the property). Always on for every other control.
    pub timer_on: bool,
    /// Toggle properties (`Checked`/`Selected`/`Value`) whose last write was a click's — or the `0` a radio
    /// gets when another of its group turns on: the program reads them as written, not canonicalised, because
    /// the desktop stores those writes as they come (`render.rs` `push_toggle_state`, form-host
    /// `clear_radio_siblings`), where a program's own `SET` is canonicalised (`property_runtime_text`).
    ui_raw: Vec<String>,
    /// The control's animations as designed (spec 038), in its own order: what `PlayAnimation` and the triggers start
    /// (`crate::motion`). Empty for the form and for a control with none.
    pub anims: Vec<cobolt_forms::model::AnimationDef>,
}

impl Ctl {
    pub fn control_type(&self) -> Option<&ControlType> {
        match &self.kind {
            Kind::Control(t) => Some(t),
            Kind::Form => None,
        }
    }

    /// The stored text of a property (case-insensitive), as written — not canonicalised.
    pub fn raw(&self, name: &str) -> Option<&str> {
        self.props.get(&name.to_ascii_lowercase()).map(String::as_str)
    }

    /// `raw`, or `default` when the property is absent or empty.
    pub fn text_or<'a>(&'a self, name: &str, default: &'a str) -> &'a str {
        match self.raw(name) {
            Some(v) if !v.is_empty() => v,
            _ => default,
        }
    }

    pub fn flag(&self, name: &str) -> bool {
        self.raw(name).map(|v| turned_on(v) && v.trim() != "0").unwrap_or(false)
    }

    pub fn num(&self, name: &str) -> Option<f64> {
        self.raw(name).and_then(crate::value::parse_f64)
    }

    pub fn visible(&self) -> bool {
        self.raw("visible").map(turned_on).unwrap_or(true)
    }

    pub fn enabled(&self) -> bool {
        self.raw("enabled").map(turned_on).unwrap_or(true)
    }

    pub fn is_container(&self) -> bool {
        matches!(
            &self.kind,
            Kind::Form
                | Kind::Control(
                    ControlType::Panel
                        | ControlType::GroupBox
                        | ControlType::TabControl
                        | ControlType::Splitter
                        | ControlType::SideMenu
                        | ControlType::StatusBar
                        | ControlType::ToolBar
                        | ControlType::MenuBar
                )
        )
    }

    /// Is a handler bound to this event? Events fire only for bound handlers.
    pub fn has_handler(&self, event: &str) -> bool {
        let e = event.to_ascii_lowercase();
        self.bound.iter().any(|b| *b == e)
    }
}

/// One layer of a form instance (spec 091; ABI A9): its backdrop properties, and whether it is shown —
/// the player's state for this instance only, `false` when the instance is created (WASM-016 R19), and
/// dropped with the instance.
#[derive(Debug, Clone)]
pub struct LayerState {
    pub name: String,
    /// The ABI id the program addresses it by (`3 + n …`, A9); `None` when the program never does.
    pub abi_id: Option<u32>,
    /// Lower-case property name → stored text: `name` and the backdrop properties.
    pub props: IndexMap<String, String>,
    pub visible: bool,
}

/// The backdrop properties of a layer a program may write (A9). `Name` is read-only.
const LAYER_WRITABLE: &[&str] = &[
    "visible",
    "transparency",
    "backgroundcolor",
    "backgroundgradientenabled",
    "backgroundgradientstartcolor",
    "backgroundgradientendcolor",
    "backgroundgradientdirection",
    "backgroundimage",
    "backgroundimagemode",
];

/// Why a layer write was not done.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LayerWriteError {
    /// `Name` cannot be written (A9: the program sees `-2`).
    ReadOnly(String),
    /// A layer has no such property.
    Unknown(String),
}

fn layer_props(l: &cobolt_forms::model::Layer) -> IndexMap<String, String> {
    let b = &l.backdrop;
    let mut p: IndexMap<String, String> = IndexMap::new();
    p.insert("name".into(), l.name.clone());
    p.insert("transparency".into(), b.transparency.to_string());
    p.insert("backgroundcolor".into(), b.color.clone());
    p.insert("backgroundgradientenabled".into(), if b.gradient_enabled { "1" } else { "0" }.into());
    p.insert("backgroundgradientstartcolor".into(), b.gradient_start_color.clone());
    p.insert("backgroundgradientendcolor".into(), b.gradient_end_color.clone());
    p.insert("backgroundgradientdirection".into(), b.gradient_direction.clone());
    p.insert("backgroundimage".into(), b.image.clone());
    p.insert("backgroundimagemode".into(), b.image_mode.as_str().to_owned());
    p
}

/// Something the renderer must project.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Change {
    /// A property of `ctls[ctl]` is now different.
    Prop { ctl: usize, key: String },
    /// A command with no state behind it (`SetFocus`, `SelectAll`, `Start`, …): do it once.
    Command { ctl: usize, name: String, args: Vec<String> },
}

/// An observer event a write raised, for the host to queue if a handler is bound.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EventReq {
    pub ctl: usize,
    pub event: &'static str,
}

/// The live state of one form instance.
#[derive(Debug, Clone)]
pub struct FormTree {
    pub inst: u32,
    pub form_idx: usize,
    pub form_name: String,
    pub opener: Option<u32>,
    pub main: bool,
    pub design_size: (u32, u32),
    pub ctls: Vec<Ctl>,
    /// The form's layers, in stack order, with this instance's state of each (empty for a form with none).
    pub layers: Vec<LayerState>,
    abi_index: HashMap<u32, usize>,
    layer_abi: HashMap<u32, usize>,
    name_index: HashMap<String, usize>,
    pub layout_bag: BTreeMap<String, PropValue>,
    pub breakpoints: Vec<Breakpoint>,
    /// What the form's `FontScaling` makes of the surface this instance is shown on (spec 056 R66–R67): every lettered control's
    /// `FontSize` is taken through it ([`FormTree::font_size_of`]). The page sets it from the form's own width, as it selects the
    /// breakpoint; `1` is the designed size.
    pub font_factor: f32,
    /// The box the browser gave each expanded card, by control (its border box, in CSS pixels): what the one-factor zoom of the
    /// contents of an expanded card is taken from (spec 090, [`crate::zoom`]). Set by the page; a card that is not expanded has none.
    pub card_boxes: BTreeMap<usize, (f64, f64)>,
    changes: Vec<Change>,
    /// Properties a running program has written, by control: a breakpoint never overrides one (spec 056 R64).
    written: std::collections::HashSet<(usize, String)>,
    /// The Viewers' conversations, by control.
    pub conversations: BTreeMap<usize, cobolt_forms::viewer::Conversation>,
    /// The Authentication controls' live state, by control (WASM-009).
    pub auths: BTreeMap<usize, crate::auth::AuthUi>,
    /// What a link may point at, beyond the defaults: the project's extra schemes (WASM-006 R7). Every
    /// place that checks an address — the Hyperlink control, `SetUrl`, a Viewer's links, `Navigate` — reads it here.
    pub policy: crate::hyperlink::Policy,
    /// Whose modifier-key conventions a link's click follows (WASM-006 Q2): Safari's on Apple platforms.
    pub platform: crate::hyperlink::Platform,
    /// What this form was opened with, in order (WASM-011 R4): the query of its route.
    pub params: Vec<(String, String)>,
    /// The form's `SPECIAL-NAMES` decide how a PICTURE reads and prints: `DECIMAL-POINT IS COMMA` swaps the
    /// roles of `.` and `,`, and `CURRENCY SIGN` names the character a currency position prints.
    pub decimal_comma: bool,
    pub currency: char,
    /// The form theme this form is drawn in (`liquid-glass`, `spatial`, …): the form's own, or the project's default
    /// baked in by the build; empty when neither named one (Liquid Glass).
    pub theme: String,
    /// The size and breakpoint the program reads from `me::` (`me::Width`, `me::Height`, `me::Breakpoint`), kept as the
    /// desktop host keeps them; set by the host from what the page measures ([`LiveSize`]).
    pub live: LiveSize,
}

/// What the program of one instance is told about the room its form has (the PowerAnalyticsWeb parity job, P4).
///
/// The desktop host mirrors two things onto the form object (form-host `host.rs`):
/// * **the window's size**, `Width` and `Height` in design pixels, by the host that owns the application's window (a
///   window, or the shell's main form): `onResizing` while the size changes, `onResize` once when it settles, the size
///   written just before each (`mirror_size`). A form in the ContentPane, a dialog or a panel is a form *body*, which is
///   told nothing of the kind: its `me::Width` stays its designed width.
/// * **the active breakpoint** of a responsive form, by every surface after it lays the form out (`mirror_layout`):
///   `Breakpoint` written when it changes, then `onBreakpointChanged` — never for the breakpoint the form opens in.
///
/// On a page the form's surface is measured by the browser; the host turns those measurements into the same writes
/// and events (`Host::surface_resized`, `Host::surface_settled`).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct LiveSize {
    /// This instance is the application's window: its size is mirrored and `onResize` raised.
    pub sized: bool,
    /// The size the program reads (`None`: the designed size, as nothing was measured yet).
    pub size: Option<(f64, f64)>,
    /// The size the last `onResize` reported (or the one the form opened at).
    pub reported: Option<(f64, f64)>,
    /// A change of size was seen and its `onResize` is still owed.
    pub pending: bool,
    /// The breakpoint the program reads (`None`: not responsive, or not measured yet).
    pub bp: Option<String>,
}

/// The size a control is lettered at, given the form's font factor (`fonts::effective_size`, spec 056 R69–R70): the `FontSize` it
/// states times the factor, held to its own `MinFontSize`/`MaxFontSize` and to the engine's bounds, and its own size when it sets
/// `ScaleFont` off. `None` for a control that states no size — each painter keeps its own default for it, unscaled — and for one in a
/// layer, which is laid out nowhere and so keeps its designed size (R21). `c` is the control as it stands, so a size a breakpoint
/// overrode or a program wrote is the one scaled.
pub fn scaled_font(c: &Ctl, layered: bool, factor: f32) -> Option<f64> {
    let stated = c.raw("fontsize").and_then(parse_f64).filter(|s| *s > 0.0)?;
    if layered {
        return Some(stated);
    }
    let scales = c.raw("scalefont").map(|v| turned_on(v) && v.trim() != "0").unwrap_or(true);
    let limit = |k: &str| c.raw(k).and_then(parse_f64).unwrap_or(0.0) as f32;
    let size = fonts::scaled_size(stated as f32, scales, limit("minfontsize"), limit("maxfontsize"), factor);
    // The painters read the designed size where the layout wrote none, and the layout writes none where the size did not change.
    Some(if (size - stated as f32).abs() > f32::EPSILON { size as f64 } else { stated })
}

/// Why a form description cannot be paired with its manifest entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PairError(pub String);

impl std::fmt::Display for PairError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

fn flatten<'a>(controls: &'a [Control], out: &mut Vec<(&'a Control, Option<&'a str>)>, parent: Option<&'a str>) {
    for c in controls {
        let p = c.parent.as_deref().or(parent);
        out.push((c, p));
        flatten(&c.children, out, Some(c.id.as_str()));
    }
}

fn control_props(c: &Control) -> IndexMap<String, String> {
    let mut props: IndexMap<String, String> = IndexMap::new();
    for (k, v) in &c.properties {
        props.insert(k.to_ascii_lowercase(), v.to_xml_string());
    }
    // Field-backed properties the interpreter exposes by name (property_names_for).
    props.insert("name".into(), c.id.clone());
    props.insert("x".into(), c.rect.x.to_string());
    props.insert("y".into(), c.rect.y.to_string());
    props.insert("width".into(), c.rect.w.to_string());
    props.insert("height".into(), c.rect.h.to_string());
    props.insert("visible".into(), if c.visible { "1" } else { "0" }.into());
    // What `Enabled` reads at load is the element's flag, for a Timer too: the desktop seeds the flag after the
    // designed properties (form-host `seeding.rs`). Whether a Timer RUNS is `Ctl::timer_on`.
    props.insert("enabled".into(), if c.enabled { "1" } else { "0" }.into());
    props.insert("taborder".into(), c.tab_order.to_string());
    props.insert("zorder".into(), c.z_order.to_string());
    if let Some(t) = c.tab {
        props.insert("tab".into(), t.to_string());
    }
    props
}

/// Whether a Timer is running when its form opens: its own `Enabled` property, on when the design has none;
/// the element's `enabled` flag plays no part (`render.rs` CT::Timer, `prop_bool(ctrl, "Enabled", true)`).
fn timer_designed_on(c: &Control) -> bool {
    if c.control_type != ControlType::Timer {
        return true;
    }
    c.properties.iter().find(|(k, _)| k.eq_ignore_ascii_case("Enabled")).map(|(_, v)| v.as_bool()).unwrap_or(true)
}

fn form_props(f: &Form) -> IndexMap<String, String> {
    let mut p: IndexMap<String, String> = IndexMap::new();
    let mut put = |k: &str, v: String| {
        p.insert(k.to_ascii_lowercase(), v);
    };
    put("Name", f.name.clone());
    put("Title", f.title.clone());
    put("Caption", f.title.clone());
    put("Width", f.width.to_string());
    put("Height", f.height.to_string());
    put("BackgroundColor", f.background_color.clone());
    put("BackgroundGradientEnabled", if f.background_gradient_enabled { "1" } else { "0" }.into());
    put("BackgroundGradientStartColor", f.background_gradient_start_color.clone());
    put("BackgroundGradientEndColor", f.background_gradient_end_color.clone());
    put("BackgroundGradientDirection", f.background_gradient_direction.clone());
    put("Transparency", f.transparency.to_string());
    put("BackgroundImage", f.background_image.clone());
    put("Responsive", if f.responsive { "1" } else { "0" }.into());
    put("FullScreen", if f.full_screen { "1" } else { "0" }.into());
    put("TitleVisible", if f.title_visible { "1" } else { "0" }.into());
    put("CanMinimize", if f.can_minimize { "1" } else { "0" }.into());
    put("CanMaximize", if f.can_maximize { "1" } else { "0" }.into());
    put("Resizable", if f.resizable { "1" } else { "0" }.into());
    put("CornerRadius", f.corner_radius.to_string());
    put("ControlStyle", f.control_style.clone());
    // The form's theme, as the build resolved it (the form's own, else the project's default).
    if let Some(t) = f.theme.as_deref().map(str::trim).filter(|t| !t.is_empty()) {
        put("Theme", t.to_owned());
    }
    put("Visible", "1".into());
    put("Enabled", "1".into());
    for (k, v) in &f.layout {
        p.insert(k.to_ascii_lowercase(), v.to_xml_string());
    }
    p
}

impl FormTree {
    /// Pair a form description with the manifest entry the module was built with.
    ///
    /// Every control the manifest lists must exist in the description with the same type,
    /// or the application and its forms were built apart: that is reported as one readable
    /// error, never found later as a property that silently does nothing.
    pub fn build(
        form: &Form,
        entry: &FormEntry,
        manifest: &Manifest,
        inst: u32,
        opener: Option<u32>,
    ) -> Result<FormTree, PairError> {
        let mut flat: Vec<(&Control, Option<&str>)> = Vec::new();
        flatten(&form.controls, &mut flat, None);

        let mut ctls: Vec<Ctl> = Vec::with_capacity(flat.len() + 1);
        let bound_for = |ctrl: u32| -> Vec<String> {
            entry
                .handlers
                .iter()
                .filter(|h| h.ctrl == ctrl)
                .filter_map(|h| manifest.events.get(h.event as usize))
                .map(|e| e.to_ascii_lowercase())
                .collect()
        };
        ctls.push(Ctl {
            name: form.name.clone(),
            abi_id: Some(cobolt_wasm_abi::codes::ctrl::FORM),
            kind: Kind::Form,
            parent: None,
            tab: None,
            children: Vec::new(),
            props: form_props(form),
            bound: bound_for(cobolt_wasm_abi::codes::ctrl::FORM),
            rev: 0,
            layer: None,
            timer_on: true,
            ui_raw: Vec::new(),
            anims: Vec::new(),
        });
        // The layers: every one starts hidden in a new instance (spec 091 R35, WASM-016 R19).
        let layers: Vec<LayerState> = form
            .layers
            .iter()
            .map(|l| LayerState {
                name: l.name.clone(),
                abi_id: entry.layers.iter().find(|e| e.name.eq_ignore_ascii_case(&l.name)).map(|e| e.id),
                props: layer_props(l),
                visible: false,
            })
            .collect();
        for listed in &entry.layers {
            if !layers.iter().any(|l| l.name.eq_ignore_ascii_case(&listed.name)) {
                return Err(PairError(format!(
                    "the program addresses layer `{}` of form `{}`, which the form description does not contain: rebuild the application",
                    listed.name, entry.name
                )));
            }
        }

        let mut name_index: HashMap<String, usize> = HashMap::new();
        name_index.insert(form.name.to_ascii_uppercase(), 0);
        for (c, _) in &flat {
            let abi = entry.control_by_name(&c.id);
            if let Some(a) = abi {
                // Compared through the model's own parser, so a type the model carries as `Custom`
                // (Authentication, …) matches under the name the manifest uses for it.
                if ControlType::from_str(&a.kind) != c.control_type {
                    return Err(PairError(format!(
                        "control `{}` of form `{}` is a {} in the form description but a {} in the program: rebuild the application",
                        c.id,
                        entry.name,
                        c.control_type.as_str(),
                        a.kind
                    )));
                }
            }
            let idx = ctls.len();
            name_index.insert(c.id.to_ascii_uppercase(), idx);
            let layer = match (&c.parent, &c.layer) {
                (None, Some(name)) => layers.iter().position(|l| l.name.eq_ignore_ascii_case(name)),
                _ => None,
            };
            ctls.push(Ctl {
                name: c.id.clone(),
                abi_id: abi.map(|a| a.id),
                kind: Kind::Control(c.control_type.clone()),
                parent: None,
                tab: c.tab,
                children: Vec::new(),
                props: control_props(c),
                bound: abi.map(|a| bound_for(a.id)).unwrap_or_default(),
                rev: 0,
                layer,
                timer_on: timer_designed_on(c),
                ui_raw: Vec::new(),
                anims: c.animations.clone(),
            });
        }
        // Parent links, from the flat list.
        for (i, (_, parent)) in flat.iter().enumerate() {
            let idx = i + 1;
            let p = parent.and_then(|p| name_index.get(&p.to_ascii_uppercase()).copied()).unwrap_or(0);
            ctls[idx].parent = Some(p);
            ctls[p].children.push(idx);
        }
        // Every control the program addresses must exist in the description.
        for listed in &entry.controls {
            if !name_index.contains_key(&listed.name.to_ascii_uppercase()) {
                return Err(PairError(format!(
                    "the program addresses control `{}` of form `{}`, which the form description does not contain: rebuild the application",
                    listed.name, entry.name
                )));
            }
        }
        let mut abi_index = HashMap::new();
        for (i, c) in ctls.iter().enumerate() {
            if let Some(a) = c.abi_id {
                abi_index.insert(a, i);
            }
        }
        let layer_abi = layers.iter().enumerate().filter_map(|(k, l)| l.abi_id.map(|a| (a, k))).collect();
        Ok(FormTree {
            inst,
            form_idx: entry.id as usize,
            form_name: form.name.clone(),
            opener,
            main: entry.main,
            design_size: (form.width, form.height),
            ctls,
            layers,
            abi_index,
            layer_abi,
            name_index,
            layout_bag: form.layout.clone(),
            breakpoints: form.breakpoints.clone(),
            font_factor: 1.0,
            card_boxes: BTreeMap::new(),
            changes: Vec::new(),
            written: std::collections::HashSet::new(),
            conversations: BTreeMap::new(),
            auths: BTreeMap::new(),
            policy: crate::hyperlink::Policy::default(),
            platform: crate::hyperlink::Platform::Other,
            params: Vec::new(),
            theme: form.theme.clone().unwrap_or_default().trim().to_owned(),
            decimal_comma: cobolt_forms::picture::decimal_comma_from_special_names(&form.cobol_structure.special_names),
            currency: cobolt_forms::picture::currency_from_special_names(&form.cobol_structure.special_names),
            live: LiveSize::default(),
        })
    }

    /// The index of the control with this ABI id (`0` is the form).
    pub fn by_abi(&self, id: u32) -> Option<usize> {
        self.abi_index.get(&id).copied()
    }

    /// The layer with this ABI id (A9).
    pub fn layer_by_abi(&self, id: u32) -> Option<usize> {
        self.layer_abi.get(&id).copied()
    }

    pub fn layer_by_name(&self, name: &str) -> Option<usize> {
        self.layers.iter().position(|l| l.name.eq_ignore_ascii_case(name.trim()))
    }

    /// The layer control `i` belongs to: its own, or the one of the direct child of the form it sits in.
    pub fn layer_of(&self, mut i: usize) -> Option<usize> {
        while let Some(p) = self.ctls[i].parent.filter(|&p| p != 0) {
            i = p;
        }
        self.ctls[i].layer
    }

    /// Is control `i` in a layer that is hidden? (It is then not drawn and not reachable: `inert`, D20.)
    pub fn in_hidden_layer(&self, i: usize) -> bool {
        self.layer_of(i).is_some_and(|l| !self.layers[l].visible)
    }

    /// A layer's property as the program reads it.
    pub fn layer_get(&self, l: usize, name: &str) -> Result<String, LayerWriteError> {
        let key = name.trim().to_ascii_lowercase();
        let layer = &self.layers[l];
        if key == "visible" {
            return Ok(property_runtime_text("Layer", "Visible", if layer.visible { "1" } else { "0" }));
        }
        layer.props.get(&key).cloned().ok_or_else(|| LayerWriteError::Unknown(format!("a layer has no property `{}`", name.trim())))
    }

    /// A program's write to a layer (A9): `Visible` is this instance's state, shown on the next frame;
    /// the backdrop properties are stored; `Name` is refused. Every control of the layer is marked for
    /// redrawing, so the change reaches the page in the next render pass.
    pub fn layer_set(&mut self, l: usize, name: &str, value: &str) -> Result<(), LayerWriteError> {
        let key = name.trim().to_ascii_lowercase();
        if key == "name" {
            return Err(LayerWriteError::ReadOnly(format!("the name of layer `{}` cannot be changed", self.layers[l].name)));
        }
        if !LAYER_WRITABLE.contains(&key.as_str()) {
            return Err(LayerWriteError::Unknown(format!("a layer has no property `{}`", name.trim())));
        }
        if key == "visible" {
            self.layers[l].visible = turned_on(value);
        } else {
            self.layers[l].props.insert(key.clone(), value.to_owned());
        }
        let members: Vec<usize> = (1..self.ctls.len()).filter(|&i| self.ctls[i].layer == Some(l)).collect();
        for i in members {
            self.touch(i, &key);
        }
        Ok(())
    }

    pub fn by_name(&self, name: &str) -> Option<usize> {
        self.name_index.get(&name.trim().to_ascii_uppercase()).copied()
    }

    pub fn ctl(&self, i: usize) -> &Ctl {
        &self.ctls[i]
    }

    /// The changes since the last call.
    pub fn take_changes(&mut self) -> Vec<Change> {
        std::mem::take(&mut self.changes)
    }

    pub fn has_changes(&self) -> bool {
        !self.changes.is_empty()
    }

    fn touch(&mut self, ctl: usize, key: &str) {
        self.ctls[ctl].rev += 1;
        let ch = Change::Prop { ctl, key: key.to_owned() };
        if !self.changes.contains(&ch) {
            self.changes.push(ch);
        }
    }

    /// The Viewer's conversation of control `ctl` (made on first use).
    pub fn conversation_mut(&mut self, ctl: usize) -> &mut cobolt_forms::viewer::Conversation {
        self.conversations.entry(ctl).or_insert_with(cobolt_forms::viewer::Conversation::new)
    }

    pub fn conversation(&self, ctl: usize) -> Option<&cobolt_forms::viewer::Conversation> {
        self.conversations.get(&ctl)
    }

    /// Something the renderer must redraw that no property holds (a Viewer's conversation).
    pub fn changed(&mut self, ctl: usize, key: &str) {
        self.touch(ctl, key);
    }

    /// Make the state of every Authentication control from its properties. `flow` and `site_key`
    /// are the page's settings, for what a control leaves open; `session` is the application's
    /// (a form opened while the user is signed in shows it).
    pub fn init_auth(&mut self, flow: crate::auth::Flow, site_key: &str, session: &crate::auth::Session) {
        let ids: Vec<usize> = (0..self.ctls.len()).filter(|&i| crate::kinds::player_kind(&self.ctls[i]) == Some(crate::kinds::PlayerKind::Authentication)).collect();
        for i in ids {
            let cfg = {
                let this = &*self;
                crate::auth::config_from(&|p| this.get(i, p), flow, site_key)
            };
            let mut ui = crate::auth::AuthUi::new(cfg);
            if session.is_signed_in() {
                ui.machine.adopt(session);
            }
            self.auths.insert(i, ui);
            self.sync_auth_props(i);
        }
    }

    /// The COBOL PICTURE a TextBox's contents obey, when the developer set one (WASM-004; the desktop's
    /// `Control::effective_picture` falls back to `X(n)` from `MaximumLength`, which the browser's own
    /// `maxlength` already says, so only an explicit picture changes what is shown and accepted).
    pub fn picture_of(&self, ctl: usize) -> Option<cobolt_forms::picture::Picture> {
        if !matches!(self.ctls[ctl].control_type(), Some(ControlType::TextBox)) {
            return None;
        }
        let template = self.get(ctl, "Picture");
        let template = template.trim();
        (!template.is_empty()).then(|| cobolt_forms::picture::Picture::parse(template, self.decimal_comma))
    }

    pub fn auth(&self, ctl: usize) -> Option<&crate::auth::AuthUi> {
        self.auths.get(&ctl)
    }

    /// The state of Authentication control `ctl`, to be changed; the control is redrawn.
    pub fn auth_mut(&mut self, ctl: usize) -> Option<&mut crate::auth::AuthUi> {
        if self.auths.contains_key(&ctl) {
            self.touch(ctl, "auth");
        }
        self.auths.get_mut(&ctl)
    }

    /// Publish `State`, `UserName` and `Email` where the program reads properties (WASM-009 R3).
    pub fn sync_auth_props(&mut self, ctl: usize) {
        let Some(ui) = self.auths.get(&ctl) else { return };
        let (state, user, email) = (ui.machine.state_name().to_owned(), ui.machine.user_name.clone(), ui.machine.email.clone());
        let mode = if ui.machine.cfg.flow == crate::auth::Flow::Hosted { "Hosted" } else { "InApp" };
        for (k, v) in [("State", state), ("UserName", user), ("Email", email), ("Mode", mode.to_owned())] {
            if self.get(ctl, k) != v {
                self.set_from_ui(ctl, k, &v);
            }
        }
    }

    /// Record a command for the renderer (focus, select-all, a timer start, …).
    pub fn command(&mut self, ctl: usize, name: &str, args: Vec<String>) {
        self.changes.push(Change::Command { ctl, name: name.to_owned(), args });
    }

    // ── reading ─────────────────────────────────────────────────────────────

    /// A property as the program reads it: booleans canonicalised, everything else as stored;
    /// a property never written falls back to the control type's seeded default.
    pub fn get(&self, ctl: usize, name: &str) -> String {
        let c = &self.ctls[ctl];
        let key = name.trim().to_ascii_lowercase();
        let tn = c.kind.type_name();
        if let Some(stored) = c.props.get(&key) {
            if c.ui_raw.contains(&key) {
                return stored.clone();
            }
            return property_runtime_text(&tn, name.trim(), stored);
        }
        if let Kind::Control(t) = &c.kind {
            // A toggle's other spelling answers too (Selected ⇄ Checked).
            if is_toggle_state_property(t, name) {
                let other = if key == SELECTED_PROP.to_ascii_lowercase() { CHECKED_PROP } else { SELECTED_PROP };
                if let Some(stored) = c.props.get(&other.to_ascii_lowercase()) {
                    return property_runtime_text(&tn, name.trim(), stored);
                }
            }
            if let Some(d) = Control::default_prop(t.clone(), name.trim()) {
                return property_runtime_text(&tn, name.trim(), &d.to_xml_string());
            }
        }
        String::new()
    }

    // ── writing ─────────────────────────────────────────────────────────────

    /// A program's write. Returns the observer events it raised.
    pub fn set(&mut self, ctl: usize, name: &str, value: &str) -> Vec<EventReq> {
        let key = name.trim().to_ascii_lowercase();
        let tn = self.ctls[ctl].kind.type_name();
        // Every COBOL write is canonicalised here, in every host (model::property_runtime_text).
        let value = property_runtime_text(&tn, name.trim(), value);
        let before = self.raw_or_design(ctl, name);
        let toggle = matches!(&self.ctls[ctl].kind, Kind::Control(t) if is_toggle_state_property(t, name));
        let is_radio = matches!(&self.ctls[ctl].kind, Kind::Control(ControlType::RadioButton));
        // A program's write outranks the active breakpoint's override of the same property.
        self.written.insert((ctl, key.clone()));

        if toggle {
            // Written under BOTH spellings so they can never disagree.
            for k in [SELECTED_PROP, CHECKED_PROP] {
                self.ctls[ctl].props.insert(k.to_ascii_lowercase(), value.clone());
                self.touch(ctl, &k.to_ascii_lowercase());
            }
            // The program's own write reads canonically; a `Value` a click left stays as the click wrote it.
            self.ctls[ctl].ui_raw.retain(|k| k == "value");
        } else {
            self.ctls[ctl].props.insert(key.clone(), value.clone());
            self.touch(ctl, &key);
            self.ctls[ctl].ui_raw.retain(|k| *k != key);
        }
        if is_radio && toggle && turned_on(&value) {
            self.clear_radio_group(ctl);
        }
        // A program's `SET TMR::Enabled` starts and stops a Timer, whatever its design.
        if key == "enabled" && matches!(&self.ctls[ctl].kind, Kind::Control(ControlType::Timer)) {
            self.ctls[ctl].timer_on = turned_on(&value);
        }
        // Observer events: only when the value actually changed.
        let changed = before.as_deref() != Some(value.as_str());
        let mut out = Vec::new();
        if changed {
            if let Kind::Control(t) = &self.ctls[ctl].kind {
                for ev in t.observer_events_for(name) {
                    if self.ctls[ctl].has_handler(ev) {
                        out.push(EventReq { ctl, event: ev });
                    }
                }
            }
        }
        out
    }

    /// A user's edit from the DOM: store the new value *without* raising an observer event (the
    /// DOM event the user caused is delivered by the renderer as itself) and without clearing
    /// siblings beyond what a click does.
    pub fn set_from_ui(&mut self, ctl: usize, name: &str, value: &str) {
        let key = name.trim().to_ascii_lowercase();
        let toggle = matches!(&self.ctls[ctl].kind, Kind::Control(t) if is_toggle_state_property(t, name));
        if toggle {
            for k in [SELECTED_PROP, CHECKED_PROP] {
                self.ctls[ctl].props.insert(k.to_ascii_lowercase(), value.to_owned());
                self.touch(ctl, &k.to_ascii_lowercase());
            }
            // The program reads a click as the click spelled it (`1`/`0` for a CheckBox or a RadioButton,
            // `true`/`false` for a Switch), as the desktop's render engine writes it into the live state; a
            // CheckBox or a RadioButton click writes its `Value` too (`render.rs` `push_toggle_state`).
            self.ctls[ctl].ui_raw = vec![SELECTED_PROP.to_ascii_lowercase(), CHECKED_PROP.to_ascii_lowercase()];
            if matches!(&self.ctls[ctl].kind, Kind::Control(ControlType::CheckBox | ControlType::RadioButton)) {
                self.ctls[ctl].props.insert("value".into(), value.to_owned());
                self.touch(ctl, "value");
                self.ctls[ctl].ui_raw.push("value".into());
            }
            if matches!(&self.ctls[ctl].kind, Kind::Control(ControlType::RadioButton)) && turned_on(value) {
                self.clear_radio_group(ctl);
            }
        } else {
            self.ctls[ctl].props.insert(key.clone(), value.to_owned());
            // The page shows what the user did, and what depends on it (a selected tab shows its page).
            self.touch(ctl, &key);
        }
    }

    fn raw_or_design(&self, ctl: usize, name: &str) -> Option<String> {
        let c = &self.ctls[ctl];
        let key = name.trim().to_ascii_lowercase();
        if let Some(v) = c.props.get(&key) {
            return Some(v.clone());
        }
        if let Kind::Control(t) = &c.kind {
            return Control::default_prop(t.clone(), name.trim()).map(|v| v.to_xml_string());
        }
        None
    }

    /// One radio at a time within its group, whoever turned it on.
    /// A radio turned on turns every other radio of its group off, and the desktop writes `0` to each of them,
    /// lit or not, under `Selected`, `Checked` and `Value` — after a click (`render.rs`
    /// `clear_radio_group_siblings`) and after a program's write (form-host `clear_radio_siblings`): that `0` is
    /// what the program reads back.
    fn clear_radio_group(&mut self, ctl: usize) {
        let group = self.radio_group(ctl);
        let siblings: Vec<usize> = (0..self.ctls.len())
            .filter(|&i| i != ctl && matches!(self.ctls[i].kind, Kind::Control(ControlType::RadioButton)))
            .filter(|&i| self.radio_group(i) == group)
            .collect();
        for i in siblings {
            let keys = [SELECTED_PROP.to_ascii_lowercase(), CHECKED_PROP.to_ascii_lowercase(), "value".to_owned()];
            for k in &keys {
                self.ctls[i].props.insert(k.clone(), "0".into());
                self.touch(i, k);
            }
            self.ctls[i].ui_raw = keys.to_vec();
        }
    }

    fn radio_group(&self, ctl: usize) -> String {
        // `radio_group_key` reads `GroupName` and the container; rebuild the two facts.
        let c = &self.ctls[ctl];
        let mut probe = Control::new(c.name.clone(), ControlType::RadioButton, 0, 0);
        if let Some(g) = c.raw("groupname") {
            probe.set_prop("GroupName", g.to_owned());
        }
        probe.parent = c.parent.map(|p| self.ctls[p].name.clone());
        radio_group_key(&probe)
    }

    /// The controls as the active breakpoint presents them (spec 056 R58–R64): the design, with the
    /// breakpoint's sparse `(control, property, value)` overrides laid over it — except for a
    /// property a program has written, which the program keeps. `bp = None` is the design itself.
    pub fn effective_ctls(&self, bp: Option<&Breakpoint>) -> Vec<Ctl> {
        let mut out = self.ctls.clone();
        if let Some(bp) = bp {
            for o in &bp.overrides {
                let Some(i) = self.name_index.get(&o.control.trim().to_ascii_uppercase()).copied() else { continue };
                let key = o.property.trim().to_ascii_lowercase();
                if self.written.contains(&(i, key.clone())) {
                    continue;
                }
                out[i].props.insert(key, o.value.to_xml_string());
            }
        }
        // Then the form's font factor, over what the breakpoint left: the size a control is lettered at is the size it states
        // (the breakpoint's, if it overrode it) taken through the factor, as the desktop's layout writes it for the painters.
        for i in 1..out.len() {
            if let Some(size) = scaled_font(&out[i], self.layer_of(i).is_some(), self.font_factor) {
                if out[i].raw("fontsize").and_then(parse_f64) != Some(size) {
                    out[i].props.insert("fontsize".into(), fmt_number((size * 1000.0).round() / 1000.0));
                }
            }
        }
        // And what is inside an expanded card grows with it, by one factor (spec 090): last, so the factor multiplies the size above.
        crate::zoom::apply(&mut out, &self.card_boxes);
        out
    }

    /// The page measured expanded card `i` at `size` (its border box), or it is not expanded (`None`). Did that change what the tree
    /// holds? A change of a quarter of a pixel or less is none: a layout that settles must not set it going again.
    pub fn set_card_box(&mut self, i: usize, size: Option<(f64, f64)>) -> bool {
        match size {
            Some((w, h)) => {
                if let Some((lw, lh)) = self.card_boxes.get(&i) {
                    if (lw - w).abs() <= 0.25 && (lh - h).abs() <= 0.25 {
                        return false;
                    }
                }
                self.card_boxes.insert(i, (w, h));
                true
            }
            None => self.card_boxes.remove(&i).is_some(),
        }
    }

    /// The font factor the desktop's solver gives this form on a surface `width` wide (spec 056 R66–R67, `layout::lay_out`): what
    /// `FontScaling` makes of the width (`Fluid`, held between `MinFontScale` and `MaxFontScale`) or of the active breakpoint
    /// (`Stepped`), and 1 for a form that does not scale. A legacy `ObsoleteScalingStyle` is not read: it also resizes and moves
    /// controls, which the page does not do.
    pub fn font_factor_at(&self, width: f64, bp: Option<&Breakpoint>) -> f32 {
        fonts::form_factor(&FormBag(&self.layout_bag), self.design_size.0 as f32, width as f32, bp, 1.0, None)
    }

    /// The breakpoint a surface `width` wide selects, as the program reads it: its name, empty when the form has none
    /// (`layout::Layout::breakpoint`); `None` for a form that is not responsive, which the desktop does not lay out and so
    /// never tells a breakpoint (`mirror_layout` needs a layout).
    pub fn breakpoint_at(&self, width: f64) -> Option<String> {
        if !crate::value::truthy(&self.get(0, "Responsive")) {
            return None;
        }
        Some(cobolt_forms::layout::breakpoints::select(&self.breakpoints, width as f32, None).map(|b| b.name.clone()).unwrap_or_default())
    }

    /// Set the font factor; did it change?
    pub fn set_font_factor(&mut self, factor: f32) -> bool {
        if (self.font_factor - factor).abs() < 1e-6 {
            return false;
        }
        self.font_factor = factor;
        true
    }

    /// The size control `i` is lettered at: what it states, taken through the form's font factor ([`scaled_font`]).
    pub fn font_size_of(&self, i: usize) -> Option<f64> {
        let size = scaled_font(&self.ctls[i], self.layer_of(i).is_some(), self.font_factor)?;
        // Inside an expanded card the text grows with the card (spec 090).
        Some(match crate::zoom::origin_of(&self.ctls, &self.card_boxes, i) {
            Some((_, z)) => crate::zoom::zoomed_size(&self.ctls[i], size, z),
            None => size,
        })
    }

    /// Has a program written this property of this control?
    pub fn was_written(&self, ctl: usize, prop: &str) -> bool {
        self.written.contains(&(ctl, prop.trim().to_ascii_lowercase()))
    }

    /// The controls of one tab page of a TabControl.
    pub fn page_children(&self, tab_ctl: usize, page: u32) -> Vec<usize> {
        self.ctls[tab_ctl].children.iter().copied().filter(|&i| self.ctls[i].tab.unwrap_or(1) == page).collect()
    }

    /// A snapshot for tests and diagnostics: `{"name": {"prop": "value", …}, …}`.
    pub fn snapshot(&self) -> serde_json::Value {
        let mut m = serde_json::Map::new();
        for c in &self.ctls {
            let mut props = serde_json::Map::new();
            for (k, v) in &c.props {
                props.insert(k.clone(), serde_json::Value::String(v.clone()));
            }
            m.insert(c.name.clone(), serde_json::Value::Object(props));
        }
        serde_json::Value::Object(m)
    }
}

#[cfg(test)]
pub(crate) mod testing {
    use super::*;
    use cobolt_forms::xml::load_form_from_str;
    use cobolt_wasm_abi::manifest::{ControlEntry, HandlerEntry, Limits, VariantInfo};

    /// A small form as the `.cfrm` loader reads it, and the manifest entry that goes with it.
    pub fn demo() -> (Form, Manifest) {
        let xml = r##"<?xml version="1.0" encoding="UTF-8"?>
<Form name="main-form" title="Demo" width="640" height="480" index-base="1">
  <Control id="BTN-GO" type="Button" x="10" y="10" w="80" h="30"><Property name="Caption">Go</Property>
    <Event name="onClick" paragraph="BTN-GO--ONCLICK"><![CDATA[x]]></Event></Control>
  <Control id="TXT-IN" type="TextBox" x="10" y="50" w="200" h="26"><Property name="Text">hello</Property></Control>
  <Control id="CHK-1" type="CheckBox" x="10" y="90" w="100" h="20"><Property name="Caption">Check</Property></Control>
  <Control id="RAD-A" type="RadioButton" x="10" y="120" w="100" h="20"><Property name="Caption">A</Property></Control>
  <Control id="RAD-B" type="RadioButton" x="10" y="140" w="100" h="20"><Property name="Caption">B</Property></Control>
  <Control id="LST-1" type="ListBox" x="10" y="170" w="150" h="80"><Property name="Items">one
two</Property></Control>
  <Control id="NUD-1" type="NumericUpDown" x="200" y="170" w="80" h="26"><Property name="Value">5</Property><Property name="Minimum">0</Property><Property name="Maximum">10</Property><Property name="Step">2</Property>
    <Event name="onChange" paragraph="NUD-1--ONCHANGE"><![CDATA[x]]></Event></Control>
  <Control id="PNL-1" type="Panel" x="300" y="10" w="200" h="200"></Control>
  <Control id="LBL-IN" type="Label" x="10" y="10" w="80" h="20" parent="PNL-1"><Property name="Caption">in panel</Property></Control>
</Form>"##;
        let form = load_form_from_str(xml).expect("the demo form loads");
        let mut controls = Vec::new();
        for (i, c) in form.controls.iter().enumerate() {
            controls.push(ControlEntry { id: 3 + i as u32, name: c.id.clone(), kind: c.control_type.as_str().to_owned() });
        }
        let manifest = Manifest {
            abi: 1,
            generator: "test".into(),
            project: "demo".into(),
            variant: VariantInfo { multi_memory: true, conformance: false },
            limits: Limits::default(),
            names: vec!["Caption".into(), "Text".into()],
            events: vec!["onLoad".into(), "onClose".into(), "onClick".into(), "onChange".into()],
            forms: vec![FormEntry {
                id: 0,
                name: "main-form".into(),
                main: true,
                instances: 1,
                region_size: 64,
                controls,
                handlers: vec![
                    HandlerEntry { ctrl: 3, event: 2, entry: 0 },  // BTN-GO onClick
                    HandlerEntry { ctrl: 9, event: 3, entry: 1 },  // NUD-1 onChange
                ],
                layers: vec![],
            }],
            builtins: vec![],
            imports: vec![],
            sizes: Default::default(),
        };
        (form, manifest)
    }

    pub fn tree() -> FormTree {
        let (form, manifest) = demo();
        FormTree::build(&form, &manifest.forms[0], &manifest, 0, None).expect("pairs")
    }

    /// A tree of the controls written in `controls` (the inside of a `<Form>`), paired with a
    /// manifest that lists none of them: the view tests need the controls, not the program.
    pub fn tree_of(controls: &str) -> FormTree {
        let xml = format!(
            r##"<?xml version="1.0" encoding="UTF-8"?>
<Form name="main-form" title="Demo" width="640" height="480" index-base="1">{controls}</Form>"##
        );
        let form = load_form_from_str(&xml).expect("the test form loads");
        let (_, mut manifest) = demo();
        manifest.forms[0].controls.clear();
        manifest.forms[0].handlers.clear();
        FormTree::build(&form, &manifest.forms[0], &manifest, 0, None).expect("pairs")
    }

    /// `tree_of` with one control of `kind` named `name`, its properties set the way a program sets them.
    pub fn tree_with(kind: &str, name: &str, props: &[(&str, &str)]) -> (FormTree, usize) {
        let mut t = tree_of(&format!(r#"<Control id="{name}" type="{kind}" x="10" y="10" w="300" h="200"></Control>"#));
        let i = t.by_name(name).expect("the control exists");
        for (k, v) in props {
            t.set(i, k, v);
        }
        (t, i)
    }
}

#[cfg(test)]
mod tests {
    use super::testing::*;
    use super::*;

    /// A Timer designed `enabled="true"` with its `Enabled` property false does not run until the program
    /// starts it (the desktop: render.rs CT::Timer reads the property); the element flag still governs every
    /// other control.
    #[test]
    fn a_timer_is_on_or_off_by_its_enabled_property() {
        let t = tree_of(
            r#"<Control id="TMR-1" type="Timer" x="0" y="0" w="24" h="24" enabled="true"><Property name="Interval">50</Property><Property name="Enabled">false</Property></Control>
               <Control id="TMR-2" type="Timer" x="0" y="0" w="24" h="24" enabled="true"><Property name="Interval">50</Property></Control>
               <Control id="BTN-1" type="Button" x="0" y="0" w="24" h="24" enabled="true"><Property name="Enabled">false</Property></Control>"#,
        );
        let (t1, t2, b) = (t.by_name("TMR-1").unwrap(), t.by_name("TMR-2").unwrap(), t.by_name("BTN-1").unwrap());
        // Whether a Timer runs is `timer_on` (the element flag stays what `Enabled` reads at load, as on the
        // desktop: see `a_timer_runs_by_its_own_property_whatever_the_element_flag_says`).
        assert!(!t.ctls[t1].timer_on, "the property says off");
        assert!(t.ctls[t2].timer_on, "no property: on (the desktop's default; this element's flag is on too)");
        assert!(t.ctls[b].enabled(), "a Button keeps the element's flag");
        let mut t = t;
        t.set(t1, "Enabled", "TRUE");
        assert!(t.ctls[t1].timer_on, "the program starts it");
    }

    /// P-8b. Every combination of the element's flag and the `Enabled` property, measured on the desktop
    /// (`rcrun run-form --headless`, 1.5 s, Interval 100): it ticks by the property, on when there is none, and
    /// the flag never matters (enabled="false" with no property: 18 ticks; the page had 0). What `Enabled`
    /// reads at load is the flag (desktop `LOAD TA=false` while TA ticks, `TD=true` while TD does not).
    #[test]
    fn a_timer_runs_by_its_own_property_whatever_the_element_flag_says() {
        let t = tree_of(
            r#"<Control id="TA" type="Timer" x="0" y="0" w="24" h="24" enabled="false"><Property name="Interval">100</Property></Control>
               <Control id="TB" type="Timer" x="0" y="0" w="24" h="24" enabled="true"><Property name="Interval">100</Property></Control>
               <Control id="TC" type="Timer" x="0" y="0" w="24" h="24" enabled="false"><Property name="Interval">100</Property><Property name="Enabled">true</Property></Control>
               <Control id="TD" type="Timer" x="0" y="0" w="24" h="24" enabled="true"><Property name="Interval">100</Property><Property name="Enabled">false</Property></Control>
               <Control id="TE" type="Timer" x="0" y="0" w="24" h="24" enabled="false"><Property name="Interval">100</Property><Property name="Enabled">false</Property></Control>
               <Control id="TF" type="Timer" x="0" y="0" w="24" h="24" enabled="true"><Property name="Interval">100</Property><Property name="Enabled">true</Property></Control>
               <Control id="TG" type="Timer" x="0" y="0" w="24" h="24"><Property name="Interval">100</Property></Control>"#,
        );
        // (timer, runs at load, what `Enabled` reads at load) — the desktop's values.
        for (name, runs, reads) in [
            ("TA", true, "false"),
            ("TB", true, "true"),
            ("TC", true, "false"),
            ("TD", false, "true"),
            ("TE", false, "false"),
            ("TF", true, "true"),
            ("TG", true, "true"),
        ] {
            let i = t.by_name(name).unwrap();
            assert_eq!(t.ctls[i].timer_on, runs, "{name} runs at load");
            assert_eq!(t.get(i, "Enabled"), reads, "{name}::Enabled at load");
        }
        // The program's SET starts and stops every one of them, and reads back what it wrote.
        let mut t = t;
        for name in ["TA", "TB", "TC", "TD", "TE", "TF", "TG"] {
            let i = t.by_name(name).unwrap();
            t.set(i, "Enabled", "FALSE");
            assert!(!t.ctls[i].timer_on, "{name} stops");
            assert_eq!(t.get(i, "Enabled"), "false");
            t.set(i, "Enabled", "TRUE");
            assert!(t.ctls[i].timer_on, "{name} starts");
            assert_eq!(t.get(i, "Enabled"), "true");
        }
    }

    /// R-1. What the program reads from a toggle after the USER clicked it is what the desktop's render engine
    /// wrote into the live state: `1`/`0` for a CheckBox and a RadioButton (`push_toggle_state`, and `0` for
    /// every other radio of the group), the word for a Switch; a program's own write reads canonically.
    #[test]
    fn a_toggle_reads_back_as_the_users_click_wrote_it() {
        let mut t = tree_of(
            r#"<Control id="CHK" type="CheckBox" x="0" y="0" w="24" h="24"><Property name="Caption">c</Property></Control>
               <Control id="RB-1" type="RadioButton" x="0" y="0" w="24" h="24"><Property name="Selected">true</Property></Control>
               <Control id="RB-2" type="RadioButton" x="0" y="0" w="24" h="24"></Control>
               <Control id="RB-3" type="RadioButton" x="0" y="0" w="24" h="24"><Property name="Selected">false</Property></Control>
               <Control id="SW" type="Switch" x="0" y="0" w="24" h="24"><Property name="Checked">true</Property></Control>"#,
        );
        let (chk, rb1, rb2, rb3, sw) =
            (t.by_name("CHK").unwrap(), t.by_name("RB-1").unwrap(), t.by_name("RB-2").unwrap(), t.by_name("RB-3").unwrap(), t.by_name("SW").unwrap());
        // At load: canonical (the desktop seeds the designed property through `property_runtime_text`).
        assert_eq!(t.get(rb1, "Selected"), "true");
        assert_eq!(t.get(sw, "Checked"), "true");
        // The clicks, as the page's views write them.
        t.set_from_ui(chk, "Checked", "1");
        t.set_from_ui(rb2, "Selected", "1");
        t.set_from_ui(sw, "Checked", "false");
        assert_eq!(t.get(chk, "Checked"), "1");
        assert_eq!(t.get(chk, "Value"), "1");
        assert_eq!(t.get(rb2, "Selected"), "1");
        assert_eq!(t.get(rb2, "Checked"), "1");
        assert_eq!(t.get(rb2, "Value"), "1");
        for rb in [rb1, rb3] {
            assert_eq!(t.get(rb, "Selected"), "0", "the click wrote 0 to every other radio of the group");
            assert_eq!(t.get(rb, "Checked"), "0");
            assert_eq!(t.get(rb, "Value"), "0");
        }
        assert_eq!(t.get(sw, "Checked"), "false");
        t.set_from_ui(chk, "Checked", "0");
        assert_eq!(t.get(chk, "Checked"), "0");
        // A program's write reads canonically again (desktop: `SET CHK-1::Checked TO TRUE` reads `true`).
        t.set(chk, "Checked", "TRUE");
        assert_eq!(t.get(chk, "Checked"), "true");
        assert_eq!(t.get(chk, "Value"), "0", "the program wrote Checked, not the Value the click left");
        // A program turning a radio on writes 0 to the others, as the desktop's host does.
        t.set(rb1, "Selected", "TRUE");
        assert_eq!(t.get(rb1, "Selected"), "true");
        assert_eq!(t.get(rb2, "Selected"), "0");
        t.set(sw, "Checked", "1");
        assert_eq!(t.get(sw, "Checked"), "true");
    }

    #[test]
    fn a_form_description_pairs_with_its_manifest_entry_and_nests_by_parent() {
        let t = tree();
        assert_eq!(t.ctls[0].kind, Kind::Form);
        let btn = t.by_name("btn-go").expect("case-insensitive lookup");
        assert_eq!(t.ctls[btn].abi_id, Some(3));
        assert!(t.ctls[btn].has_handler("onClick"));
        assert!(!t.ctls[btn].has_handler("onChange"), "events fire only for bound handlers");
        let panel = t.by_name("PNL-1").unwrap();
        let inner = t.by_name("LBL-IN").unwrap();
        assert_eq!(t.ctls[inner].parent, Some(panel));
        assert_eq!(t.ctls[panel].children, vec![inner]);
        assert_eq!(t.get(btn, "Caption"), "Go");
        assert_eq!(t.get(0, "Title"), "Demo");
    }

    #[test]
    fn a_manifest_that_disagrees_with_the_description_is_one_readable_error() {
        let (form, mut manifest) = demo();
        manifest.forms[0].controls[0].kind = "Label".into();
        let e = FormTree::build(&form, &manifest.forms[0], &manifest, 0, None).unwrap_err();
        assert!(e.0.contains("rebuild the application") && e.0.contains("BTN-GO"), "{e}");
        let (form, mut manifest) = demo();
        manifest.forms[0].controls.push(cobolt_wasm_abi::manifest::ControlEntry { id: 99, name: "NOPE".into(), kind: "Button".into() });
        let e = FormTree::build(&form, &manifest.forms[0], &manifest, 0, None).unwrap_err();
        assert!(e.0.contains("NOPE"), "{e}");
    }

    #[test]
    fn the_page_a_control_sits_on_is_counted_from_one() {
        let t = tree_of(
            r#"<Control id="TC" type="TabControl" x="0" y="0" w="200" h="100"></Control>
               <Control id="A" type="Label" x="5" y="30" w="40" h="20" parent="TC" tab="1"></Control>
               <Control id="B" type="Label" x="5" y="30" w="40" h="20" parent="TC" tab="2"></Control>"#,
        );
        let tc = t.by_name("TC").unwrap();
        assert_eq!(t.ctl(t.by_name("A").unwrap()).tab, Some(1));
        assert_eq!(t.ctl(t.by_name("B").unwrap()).tab, Some(2));
        assert_eq!(t.page_children(tc, 1), vec![t.by_name("A").unwrap()]);
        assert_eq!(t.page_children(tc, 2), vec![t.by_name("B").unwrap()]);
    }

    #[test]
    fn booleans_read_back_canonically_and_unset_properties_use_the_seeded_default() {
        let mut t = tree();
        let btn = t.by_name("BTN-GO").unwrap();
        t.set(btn, "Visible", "0");
        assert_eq!(t.get(btn, "Visible"), "false");
        t.set(btn, "Enabled", "TRUE");
        assert_eq!(t.get(btn, "Enabled"), "true");
        // Never written in the .cfrm: the type's seeded default answers.
        assert!(!t.get(btn, "BackgroundColor").is_empty());
    }

    #[test]
    fn a_toggle_state_is_written_under_both_spellings_and_one_radio_is_on_at_a_time() {
        let mut t = tree();
        let a = t.by_name("RAD-A").unwrap();
        let b = t.by_name("RAD-B").unwrap();
        t.set(a, "Checked", "1");
        assert_eq!(t.get(a, "Selected"), "true");
        assert_eq!(t.get(a, "Checked"), "true");
        t.set(b, "Selected", "1");
        // Read as the desktop writes it (form-host `clear_radio_siblings` sends `0`; measured with
        // `SET RB-1::Selected TO TRUE`: the other radio then reads `0`).
        assert_eq!(t.get(a, "Selected"), "0", "a write from code clears the group's other radio");
        assert_eq!(t.get(b, "Checked"), "true");
        let chk = t.by_name("CHK-1").unwrap();
        t.set(chk, "Checked", "1");
        assert_eq!(t.get(chk, "Checked"), "true");
    }

    #[test]
    fn an_observer_event_is_raised_only_when_the_value_changes_and_a_handler_is_bound() {
        let mut t = tree();
        let nud = t.by_name("NUD-1").unwrap();
        let ev = t.set(nud, "Value", "7");
        assert_eq!(ev, vec![EventReq { ctl: nud, event: "onChange" }]);
        assert!(t.set(nud, "Value", "7").is_empty(), "same value: no event (this is what stops the obvious feedback loop)");
        let txt = t.by_name("TXT-IN").unwrap();
        assert!(t.set(txt, "Text", "changed").is_empty(), "no onChange handler bound on TXT-IN");
    }

    #[test]
    fn writes_are_logged_once_per_property_for_the_renderer() {
        let mut t = tree();
        let lbl = t.by_name("LBL-IN").unwrap();
        for i in 0..1000 {
            t.set(lbl, "Caption", &format!("v{i}"));
        }
        let ch = t.take_changes();
        assert_eq!(ch.len(), 1, "1000 writes of one property are one change");
        assert_eq!(t.get(lbl, "Caption"), "v999");
        assert!(t.take_changes().is_empty());
    }

    /// A form of `Fluid` text, designed 640 wide: three labels (one scales, one does not, one states no size) and a fourth in a layer.
    fn fluid() -> FormTree {
        tree_of(
            r##"<Layer name="POP" color="#00000000" />
               <FormLayout FontScaling="Fluid" MaxFontScale="1.3" />
               <Control id="LBL-A" type="Label" x="0" y="0" w="50" h="20"><Property name="FontSize">28</Property></Control>
               <Control id="LBL-B" type="Label" x="0" y="30" w="50" h="20"><Property name="FontSize">28</Property><Property name="ScaleFont">false</Property></Control>
               <Control id="LBL-C" type="Label" x="0" y="60" w="50" h="20"></Control>
               <Control id="LBL-D" type="Label" x="0" y="90" w="50" h="20" layer="POP"><Property name="FontSize">28</Property></Control>
               <Control id="LBL-E" type="Label" x="0" y="120" w="50" h="20"><Property name="FontSize">28</Property><Property name="MaxFontSize">30</Property></Control>"##,
        )
    }

    #[test]
    fn a_fluid_form_follows_its_width_between_its_limits() {
        let t = fluid();
        assert_eq!(t.design_size.0, 640);
        // The desktop's default floor is 0.85 and this form raised the ceiling from 1.5 to 1.3.
        assert_eq!(t.font_factor_at(320.0, None), 0.85);
        assert_eq!(t.font_factor_at(640.0, None), 1.0);
        assert_eq!(t.font_factor_at(704.0, None), 1.1);
        assert_eq!(t.font_factor_at(1024.0, None), 1.3);
    }

    #[test]
    fn a_stepped_form_follows_the_active_breakpoint_and_a_form_that_does_not_scale_is_one() {
        let mut t = tree_of(
            r#"<FormLayout FontScaling="Stepped" />
               <Breakpoints><Breakpoint name="Narrow" min-width="0" font-factor="0.9" /><Breakpoint name="Wide" min-width="800" font-factor="1.2" /></Breakpoints>"#,
        );
        let (narrow, wide) = (t.breakpoints[0].clone(), t.breakpoints[1].clone());
        assert_eq!(t.font_factor_at(500.0, Some(&narrow)), 0.9);
        assert_eq!(t.font_factor_at(900.0, Some(&wide)), 1.2);
        t.layout_bag.insert("FontScaling".into(), PropValue::String("None".into()));
        assert_eq!(t.font_factor_at(500.0, Some(&narrow)), 1.0);
    }

    #[test]
    fn a_control_is_lettered_at_the_size_it_states_taken_through_the_factor() {
        let mut t = fluid();
        let (a, b, c, d, e) = ["LBL-A", "LBL-B", "LBL-C", "LBL-D", "LBL-E"].map(|n| t.by_name(n).unwrap()).into();
        assert_eq!(t.font_size_of(a), Some(28.0), "at the designed size nothing changes");
        assert!(t.set_font_factor(0.85));
        assert!(!t.set_font_factor(0.85), "the same factor is no change");
        assert!((t.font_size_of(a).unwrap() - 23.8).abs() < 1e-4, "28 at 0.85, a fraction: {:?}", t.font_size_of(a));
        assert_eq!(t.font_size_of(b), Some(28.0), "ScaleFont off keeps the designed size");
        assert_eq!(t.font_size_of(c), None, "a control that states no size is not scaled: each painter keeps its own default");
        assert_eq!(t.font_size_of(d), Some(28.0), "a control in a layer is laid out nowhere, so its text is not scaled either (R21)");
        t.set_font_factor(1.3);
        assert_eq!(t.font_size_of(e), Some(30.0), "held to the control's own MaxFontSize");
        assert!((t.font_size_of(a).unwrap() - 36.4).abs() < 1e-4);
    }

    #[test]
    fn the_effective_controls_carry_the_scaled_size_over_a_breakpoints_override_and_never_over_the_design() {
        use cobolt_forms::layout::breakpoints::{Breakpoint, Override};
        let mut t = fluid();
        let a = t.by_name("LBL-A").unwrap();
        t.set_font_factor(0.85);
        assert_eq!(t.effective_ctls(None)[a].raw("fontsize"), Some("23.8"), "scaled, with no breakpoint at all");
        assert_eq!(t.ctls[a].raw("fontsize"), Some("28"), "what a program reads back is the designed size");
        let narrow = Breakpoint {
            name: "narrow".into(),
            min_width: 0,
            font_factor: 1.0,
            overrides: vec![Override { control: "LBL-A".into(), property: "FontSize".into(), value: PropValue::Int(20) }],
        };
        assert_eq!(t.effective_ctls(Some(&narrow))[a].raw("fontsize"), Some("17"), "the breakpoint's 20, scaled: 20 x 0.85");
        t.set_font_factor(1.0);
        assert_eq!(t.effective_ctls(None)[a].raw("fontsize"), Some("28"));
    }
}
