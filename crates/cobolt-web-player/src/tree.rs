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
use cobolt_forms::model::{
    is_toggle_state_property, property_runtime_text, radio_group_key, Control, ControlType, Form, PropValue, CHECKED_PROP,
    SELECTED_PROP,
};
use cobolt_wasm_abi::manifest::{FormEntry, Manifest};
use indexmap::IndexMap;

use crate::value::turned_on;

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
    abi_index: HashMap<u32, usize>,
    name_index: HashMap<String, usize>,
    pub layout_bag: BTreeMap<String, PropValue>,
    pub breakpoints: Vec<Breakpoint>,
    changes: Vec<Change>,
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
    props.insert("enabled".into(), if c.enabled { "1" } else { "0" }.into());
    props.insert("taborder".into(), c.tab_order.to_string());
    props.insert("zorder".into(), c.z_order.to_string());
    if let Some(t) = c.tab {
        props.insert("tab".into(), t.to_string());
    }
    props
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
        });

        let mut name_index: HashMap<String, usize> = HashMap::new();
        name_index.insert(form.name.to_ascii_uppercase(), 0);
        for (c, _) in &flat {
            let abi = entry.control_by_name(&c.id);
            if let Some(a) = abi {
                // Compared through the model's own parser, so a type the model carries as `Custom`
                // (WebLink, Authentication, …) matches under the name the manifest uses for it.
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
        Ok(FormTree {
            inst,
            form_idx: entry.id as usize,
            form_name: form.name.clone(),
            opener,
            main: entry.main,
            design_size: (form.width, form.height),
            ctls,
            abi_index,
            name_index,
            layout_bag: form.layout.clone(),
            breakpoints: form.breakpoints.clone(),
            changes: Vec::new(),
        })
    }

    /// The index of the control with this ABI id (`0` is the form).
    pub fn by_abi(&self, id: u32) -> Option<usize> {
        self.abi_index.get(&id).copied()
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

        if toggle {
            // Written under BOTH spellings so they can never disagree.
            for k in [SELECTED_PROP, CHECKED_PROP] {
                self.ctls[ctl].props.insert(k.to_ascii_lowercase(), value.clone());
                self.touch(ctl, &k.to_ascii_lowercase());
            }
        } else {
            self.ctls[ctl].props.insert(key.clone(), value.clone());
            self.touch(ctl, &key);
        }
        if is_radio && toggle && turned_on(&value) {
            self.clear_radio_siblings(ctl);
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
            }
            if matches!(&self.ctls[ctl].kind, Kind::Control(ControlType::RadioButton)) && turned_on(value) {
                self.clear_radio_siblings(ctl);
            }
        } else {
            self.ctls[ctl].props.insert(key, value.to_owned());
        }
        self.ctls[ctl].rev += 1;
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
    fn clear_radio_siblings(&mut self, ctl: usize) {
        let group = self.radio_group(ctl);
        let siblings: Vec<usize> = (0..self.ctls.len())
            .filter(|&i| i != ctl && matches!(self.ctls[i].kind, Kind::Control(ControlType::RadioButton)))
            .filter(|&i| self.radio_group(i) == group)
            .collect();
        for i in siblings {
            let on = self.ctls[i].raw(SELECTED_PROP).map(turned_on).unwrap_or(false)
                || self.ctls[i].raw(CHECKED_PROP).map(turned_on).unwrap_or(false);
            if on {
                for k in [SELECTED_PROP, CHECKED_PROP] {
                    self.ctls[i].props.insert(k.to_ascii_lowercase(), "0".into());
                    self.touch(i, &k.to_ascii_lowercase());
                }
            }
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
<Form name="main-form" title="Demo" width="640" height="480">
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
}

#[cfg(test)]
mod tests {
    use super::testing::*;
    use super::*;

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
        assert_eq!(t.get(a, "Selected"), "false", "a write from code clears the group's other radio");
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
}
