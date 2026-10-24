// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! **The form description** (WASM-016 R3, R4, R18; Q1 decided 2026-10-22: JSON, versioned): what the
//! browser player draws a form from. The Build and the Preview write it with [`describe`] from a form
//! the one `.cfrm` parser loaded (so migrations and seeding ran once, at Build), and the player turns it
//! back into the desktop's own [`Form`] with [`to_form`].
//!
//! The schema is its own set of types, not `serde` on the model, for two reasons:
//! * **No code by construction (R4).** There is no field for handler bodies, the working-storage text,
//!   the COBOL-structure blocks, the named procedures or the recycle bin; a code-bearing field added to
//!   [`Form`] later cannot leak, because the description has no place for it. A credential property is
//!   written empty, as the `.cfrm` writer does.
//! * **Versioned on purpose (R11).** Renaming a model field does not change the format; a field is
//!   added compatibly with `#[serde(default)]`, and a removal or a change of meaning bumps
//!   [`DESCRIPTION_FORMAT`].
//!
//! Enumerations are written with the `.cfrm` spellings (`as_str` / `from_str`). Designer-only state
//! (the grid size and snap) is not described: [`code_free`] is the form a description stands for.

use std::collections::BTreeMap;

use indexmap::IndexMap;
use serde::{Deserialize, Serialize};

use crate::layout::breakpoints::{Breakpoint, Override};
use crate::model::{
    AnimKind, AnimRepeat, AnimTrigger, AnimationDef, BgImageMode, Control, ControlType,
    DataBindingDef, DockEdge, EasingKind, EventBinding, Form, FormFormat, FormStartPosition,
    GlassStyle, Layer, MenuPaneBackground, ModalOverlayStyle, PropValue, Rect, WindowState,
};

/// The version of the description format. The player refuses a description with another number.
pub const DESCRIPTION_FORMAT: u32 = 1;

/// One form, as the player reads it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FormDescription {
    pub format: u32,
    pub name: String,
    /// The window-level properties.
    pub form: FormProperties,
    /// The layers above the base, in stack order (spec 091 R1, R9). Never a `Visible`: every layer starts
    /// hidden in each instance (R19). Not written for a form with no layers (WASM-004 R24).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub layers: Vec<LayerDescription>,
    pub controls: Vec<ControlDescription>,
    /// The form's lifecycle events that are bound (OnLoad, OnClose …); the code is in the program module.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub form_events: Vec<EventDescription>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub data_bindings: Vec<DataBindingDef>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub animations: Vec<AnimationDescription>,
    /// The form's layout properties (spec 056).
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub layout: BTreeMap<String, PropValue>,
    pub breakpoints: Vec<BreakpointDescription>,
    /// The bundle path of the form's stylesheet (WASM-007), when it has one. Written by the Build.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stylesheet: Option<String>,
}

/// The window-level properties of a form (the `.cfrm` `<Form>` attributes the player can use).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FormProperties {
    pub title: String,
    pub width: u32,
    pub height: u32,
    pub background: BackdropDescription,
    pub target: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub theme: Option<String>,
    pub use_theme_background: bool,
    pub glass_style: String,
    pub main_form: bool,
    pub taskbar_icon: String,
    pub can_minimize: bool,
    pub can_maximize: bool,
    pub window_state: String,
    pub full_screen: bool,
    pub title_visible: bool,
    pub corner_radius: u32,
    pub resizable: bool,
    pub screen_fill: u32,
    pub dock_to_opener: String,
    pub dock_gap: u32,
    pub dock_length: u32,
    pub modal_overlay_style: String,
    pub form_format: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub menu_pane_background: Option<BackdropDescription>,
    pub window_effects: bool,
    pub x: i32,
    pub y: i32,
    pub start_position: String,
    pub responsive: bool,
    pub control_style: String,
}

/// A backdrop: the form's own background, a layer's, the shell MenuPane's.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BackdropDescription {
    pub color: String,
    pub gradient_enabled: bool,
    pub gradient_start: String,
    pub gradient_end: String,
    pub gradient_direction: String,
    pub transparency: u8,
    pub image: String,
    pub image_mode: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LayerDescription {
    pub name: String,
    pub backdrop: BackdropDescription,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ControlDescription {
    pub name: String,
    #[serde(rename = "type")]
    pub control_type: String,
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
    pub tab_order: u32,
    pub z_order: i32,
    pub visible: bool,
    pub enabled: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tab: Option<u32>,
    /// The layer the control sits in; absent = the base (`Form`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub layer: Option<String>,
    pub properties: IndexMap<String, PropValue>,
    /// The events bound to handlers, by name; the handlers are in the program module.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub events: Vec<EventDescription>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub animations: Vec<AnimationDescription>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub children: Vec<ControlDescription>,
}

/// A bound event: its name and the handler's nested-program name. Never its code.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EventDescription {
    pub event: String,
    pub handler: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AnimationDescription {
    pub name: String,
    pub trigger: String,
    pub kind: String,
    pub duration_ms: u64,
    pub delay_ms: u64,
    pub easing: String,
    pub repeat: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub repeat_count: Option<u32>,
    pub repeat_delay_ms: u64,
    pub slide_dx: i32,
    pub slide_dy: i32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BreakpointDescription {
    pub name: String,
    pub min_width: i64,
    pub font_factor: f32,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub overrides: Vec<OverrideDescription>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OverrideDescription {
    pub control: String,
    pub property: String,
    pub value: PropValue,
}

/// Why a description cannot be turned back into a form.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DescriptionError {
    /// The description is of another format than the one this build reads.
    Format { found: u32, supported: u32 },
    /// The text is not a description.
    Json(String),
}

impl std::fmt::Display for DescriptionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DescriptionError::Format { found, supported } => write!(
                f,
                "the form description is format {found}, and this player reads format {supported}: rebuild the application with the matching player"
            ),
            DescriptionError::Json(e) => write!(f, "the form description cannot be read: {e}"),
        }
    }
}

impl std::error::Error for DescriptionError {}

fn backdrop(b: &MenuPaneBackground) -> BackdropDescription {
    BackdropDescription {
        color: b.color.clone(),
        gradient_enabled: b.gradient_enabled,
        gradient_start: b.gradient_start_color.clone(),
        gradient_end: b.gradient_end_color.clone(),
        gradient_direction: b.gradient_direction.clone(),
        transparency: b.transparency,
        image: b.image.clone(),
        image_mode: b.image_mode.as_str().to_owned(),
    }
}

fn menu_pane_background(b: &BackdropDescription) -> MenuPaneBackground {
    MenuPaneBackground {
        color: b.color.clone(),
        gradient_enabled: b.gradient_enabled,
        gradient_start_color: b.gradient_start.clone(),
        gradient_end_color: b.gradient_end.clone(),
        gradient_direction: b.gradient_direction.clone(),
        transparency: b.transparency,
        image: b.image.clone(),
        image_mode: BgImageMode::from_str(&b.image_mode),
    }
}

fn events(list: &[EventBinding]) -> Vec<EventDescription> {
    list.iter()
        .map(|e| EventDescription {
            event: e.event.clone(),
            handler: e.paragraph.clone(),
        })
        .collect()
}

fn bindings(list: &[EventDescription]) -> Vec<EventBinding> {
    list.iter()
        .map(|e| EventBinding {
            event: e.event.clone(),
            paragraph: e.handler.clone(),
            code: String::new(),
        })
        .collect()
}

fn animation(a: &AnimationDef) -> AnimationDescription {
    AnimationDescription {
        name: a.name.clone(),
        trigger: a.trigger.as_str().to_owned(),
        kind: a.kind.as_str().to_owned(),
        duration_ms: a.duration_ms,
        delay_ms: a.delay_ms,
        easing: a.easing.as_str().to_owned(),
        repeat: a.repeat.as_str().to_owned(),
        repeat_count: match a.repeat {
            AnimRepeat::Count(n) => Some(n),
            _ => None,
        },
        repeat_delay_ms: a.repeat_delay_ms,
        slide_dx: a.slide_dx,
        slide_dy: a.slide_dy,
    }
}

fn animation_def(a: &AnimationDescription) -> AnimationDef {
    let mut d = AnimationDef::new(&a.name);
    d.trigger = AnimTrigger::from_str(&a.trigger);
    d.kind = AnimKind::from_str(&a.kind);
    d.duration_ms = a.duration_ms;
    d.delay_ms = a.delay_ms;
    d.easing = EasingKind::from_str(&a.easing);
    d.repeat = match a.repeat.as_str() {
        "Loop" => AnimRepeat::Loop,
        "PingPong" => AnimRepeat::PingPong,
        "Count" => AnimRepeat::Count(a.repeat_count.unwrap_or(3).max(1)),
        _ => AnimRepeat::Once,
    };
    d.repeat_delay_ms = a.repeat_delay_ms;
    d.slide_dx = a.slide_dx;
    d.slide_dy = a.slide_dy;
    d
}

fn control(c: &Control) -> ControlDescription {
    ControlDescription {
        name: c.id.clone(),
        control_type: c.control_type.as_str().to_owned(),
        x: c.rect.x,
        y: c.rect.y,
        w: c.rect.w,
        h: c.rect.h,
        tab_order: c.tab_order,
        z_order: c.z_order,
        visible: c.visible,
        enabled: c.enabled,
        parent: c.parent.clone(),
        tab: c.tab,
        layer: c.layer.clone(),
        properties: c
            .properties
            .iter()
            .map(|(k, v)| {
                // R4 — as the `.cfrm` writer does: the property is kept, its secret value is not.
                let v = if crate::connections::is_credential_prop(k) {
                    PropValue::String(String::new())
                } else {
                    v.clone()
                };
                (k.clone(), v)
            })
            .collect(),
        events: events(&c.events),
        animations: c.animations.iter().map(animation).collect(),
        children: c.children.iter().map(control).collect(),
    }
}

fn model_control(d: &ControlDescription) -> Control {
    let mut c = Control::new(
        d.name.clone(),
        ControlType::from_str(&d.control_type),
        d.x,
        d.y,
    );
    c.rect = Rect::new(d.x, d.y, d.w, d.h);
    c.tab_order = d.tab_order;
    c.z_order = d.z_order;
    c.visible = d.visible;
    c.enabled = d.enabled;
    c.parent = d.parent.clone();
    c.tab = d.tab;
    c.layer = d.layer.clone();
    // The description is authoritative, as the file is to the `.cfrm` reader.
    c.properties = d.properties.clone();
    c.events = bindings(&d.events);
    c.animations = d.animations.iter().map(animation_def).collect();
    c.children = d.children.iter().map(model_control).collect();
    c
}

/// The description of `form`: everything the player draws, nothing of the program (R4).
pub fn describe(form: &Form) -> FormDescription {
    FormDescription {
        format: DESCRIPTION_FORMAT,
        name: form.name.clone(),
        form: FormProperties {
            title: form.title.clone(),
            width: form.width,
            height: form.height,
            background: BackdropDescription {
                color: form.background_color.clone(),
                gradient_enabled: form.background_gradient_enabled,
                gradient_start: form.background_gradient_start_color.clone(),
                gradient_end: form.background_gradient_end_color.clone(),
                gradient_direction: form.background_gradient_direction.clone(),
                transparency: form.transparency,
                image: form.background_image.clone(),
                image_mode: form.bg_image_mode.as_str().to_owned(),
            },
            target: form.target.clone(),
            theme: form.theme.clone(),
            use_theme_background: form.use_theme_background,
            glass_style: form.glass_style.as_str().to_owned(),
            main_form: form.main_form,
            taskbar_icon: form.taskbar_icon.clone(),
            can_minimize: form.can_minimize,
            can_maximize: form.can_maximize,
            window_state: form.window_state.as_str().to_owned(),
            full_screen: form.full_screen,
            title_visible: form.title_visible,
            corner_radius: form.corner_radius,
            resizable: form.resizable,
            screen_fill: form.screen_fill,
            dock_to_opener: form.dock_to_opener.as_str().to_owned(),
            dock_gap: form.dock_gap,
            dock_length: form.dock_length,
            modal_overlay_style: form.modal_overlay_style.as_str().to_owned(),
            form_format: form.form_format.as_str().to_owned(),
            menu_pane_background: form.menu_pane_background.as_ref().map(backdrop),
            window_effects: form.window_effects,
            x: form.x,
            y: form.y,
            start_position: form.start_position.as_str().to_owned(),
            responsive: form.responsive,
            control_style: form.control_style.clone(),
        },
        layers: form
            .layers
            .iter()
            .map(|l| LayerDescription {
                name: l.name.clone(),
                backdrop: backdrop(&l.backdrop),
            })
            .collect(),
        controls: form.controls.iter().map(control).collect(),
        form_events: events(&form.form_events),
        data_bindings: form.data_bindings.clone(),
        animations: form.animations.iter().map(animation).collect(),
        layout: form.layout.clone(),
        breakpoints: form
            .breakpoints
            .iter()
            .map(|b| BreakpointDescription {
                name: b.name.clone(),
                min_width: b.min_width,
                font_factor: b.font_factor,
                overrides: b
                    .overrides
                    .iter()
                    .map(|o| OverrideDescription {
                        control: o.control.clone(),
                        property: o.property.clone(),
                        value: o.value.clone(),
                    })
                    .collect(),
            })
            .collect(),
        stylesheet: None,
    }
}

/// The form a description stands for — the player's model of it. It equals [`code_free`] of the form
/// that was described.
pub fn to_form(d: &FormDescription) -> Result<Form, DescriptionError> {
    if d.format != DESCRIPTION_FORMAT {
        return Err(DescriptionError::Format {
            found: d.format,
            supported: DESCRIPTION_FORMAT,
        });
    }
    let p = &d.form;
    let mut f = Form::new(d.name.clone(), p.title.clone(), p.width, p.height);
    // `Form::new` seeds what a new form starts with — the repository's type bridge among it; a
    // description stands for the form without code, so none of that is kept.
    f.cobol_structure = Default::default();
    f.background_color = p.background.color.clone();
    f.background_gradient_enabled = p.background.gradient_enabled;
    f.background_gradient_start_color = p.background.gradient_start.clone();
    f.background_gradient_end_color = p.background.gradient_end.clone();
    f.background_gradient_direction = p.background.gradient_direction.clone();
    f.transparency = p.background.transparency;
    f.background_image = p.background.image.clone();
    f.bg_image_mode = BgImageMode::from_str(&p.background.image_mode);
    f.target = p.target.clone();
    f.theme = p.theme.clone();
    f.use_theme_background = p.use_theme_background;
    f.glass_style = GlassStyle::from_str(&p.glass_style);
    f.main_form = p.main_form;
    f.taskbar_icon = p.taskbar_icon.clone();
    f.can_minimize = p.can_minimize;
    f.can_maximize = p.can_maximize;
    f.window_state = WindowState::from_str(&p.window_state);
    f.full_screen = p.full_screen;
    f.title_visible = p.title_visible;
    f.corner_radius = p.corner_radius;
    f.resizable = p.resizable;
    f.screen_fill = p.screen_fill;
    f.dock_to_opener = DockEdge::from_str(&p.dock_to_opener);
    f.dock_gap = p.dock_gap;
    f.dock_length = p.dock_length;
    f.modal_overlay_style = ModalOverlayStyle::from_str(&p.modal_overlay_style);
    f.form_format = FormFormat::from_str(&p.form_format);
    f.menu_pane_background = p.menu_pane_background.as_ref().map(menu_pane_background);
    f.window_effects = p.window_effects;
    f.x = p.x;
    f.y = p.y;
    f.start_position = FormStartPosition::from_str(&p.start_position);
    f.responsive = p.responsive;
    f.control_style = p.control_style.clone();
    f.layers = d
        .layers
        .iter()
        .map(|l| Layer {
            name: l.name.clone(),
            backdrop: menu_pane_background(&l.backdrop),
        })
        .collect();
    f.controls = d.controls.iter().map(model_control).collect();
    f.form_events = bindings(&d.form_events);
    f.data_bindings = d.data_bindings.clone();
    f.animations = d.animations.iter().map(animation_def).collect();
    f.layout = d.layout.clone();
    f.breakpoints = d
        .breakpoints
        .iter()
        .map(|b| Breakpoint {
            name: b.name.clone(),
            min_width: b.min_width,
            font_factor: b.font_factor,
            overrides: b
                .overrides
                .iter()
                .map(|o| Override {
                    control: o.control.clone(),
                    property: o.property.clone(),
                    value: o.value.clone(),
                })
                .collect(),
        })
        .collect();
    Ok(f)
}

/// Read a description from its JSON, refusing another format with both numbers named.
pub fn from_json(text: &str) -> Result<FormDescription, DescriptionError> {
    // The format is checked before the rest is read: a newer description may not fit these types at all.
    #[derive(Deserialize)]
    struct Head {
        format: u32,
    }
    let head: Head =
        serde_json::from_str(text).map_err(|e| DescriptionError::Json(e.to_string()))?;
    if head.format != DESCRIPTION_FORMAT {
        return Err(DescriptionError::Format {
            found: head.format,
            supported: DESCRIPTION_FORMAT,
        });
    }
    serde_json::from_str(text).map_err(|e| DescriptionError::Json(e.to_string()))
}

/// The JSON of a description, as the Build writes it.
pub fn to_json(d: &FormDescription) -> String {
    serde_json::to_string_pretty(d).expect("a form description always serialises")
}

/// `form` without what a description leaves out: every piece of program code (handler bodies, the
/// working-storage text, the COBOL-structure blocks, the named procedures, the recycle bin), the
/// credential values, and the designer's grid.
pub fn code_free(form: &Form) -> Form {
    fn walk(controls: &mut [Control]) {
        for c in controls {
            for e in &mut c.events {
                e.code.clear();
            }
            for (k, v) in c.properties.iter_mut() {
                if crate::connections::is_credential_prop(k) {
                    *v = PropValue::String(String::new());
                }
            }
            walk(&mut c.children);
        }
    }
    let mut f = form.clone();
    walk(&mut f.controls);
    for e in &mut f.form_events {
        e.code.clear();
    }
    f.user_ws_source.clear();
    f.cobol_structure = Default::default();
    f.user_procedures.clear();
    f.deleted_code.clear();
    let blank = Form::new("", "", 0, 0);
    f.grid_size = blank.grid_size;
    f.snap_to_grid = blank.snap_to_grid;
    f
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::xml::{form_to_string, load_form, load_form_from_str};
    use pretty_assertions::assert_eq;

    fn example_forms() -> Vec<std::path::PathBuf> {
        fn walk(dir: &std::path::Path, out: &mut Vec<std::path::PathBuf>) {
            let Ok(rd) = std::fs::read_dir(dir) else {
                return;
            };
            for e in rd.flatten() {
                let p = e.path();
                if p.is_dir() {
                    walk(&p, out);
                } else if matches!(
                    p.extension().and_then(|x| x.to_str()),
                    Some("cfrm" | "wcfrm" | "mcfrm")
                ) {
                    out.push(p);
                }
            }
        }
        let mut out = Vec::new();
        walk(
            &std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples"),
            &mut out,
        );
        out.sort();
        out
    }

    /// AC2 (description half) — every example form survives the round trip: the form the player
    /// rebuilds is the code-free form the Build described, compared through the one `.cfrm` writer.
    #[test]
    fn description_round_trips_every_example_form() {
        let forms = example_forms();
        assert!(
            forms.len() > 10,
            "the example forms were not found ({} files)",
            forms.len()
        );
        let mut controls = 0;
        for path in &forms {
            let form = load_form(path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
            let d = from_json(&to_json(&describe(&form)))
                .unwrap_or_else(|e| panic!("{}: {e}", path.display()));
            let back = to_form(&d).unwrap();
            assert_eq!(
                form_to_string(&back).unwrap(),
                form_to_string(&code_free(&form)).unwrap(),
                "{} does not survive its description",
                path.display()
            );
            controls += form.controls.len();
        }
        println!(
            "description round trip: {} example forms, {controls} controls, all equal",
            forms.len()
        );
    }

    const WITH_CODE: &str = r##"<?xml version="1.0" encoding="UTF-8"?>
<Form name="MAIN" title="T" width="400" height="300" background="#FFFFFFFF" transparency="0" grid-size="8" snap-to-grid="true" target="Custom" index-base="1">
  <working-storage><![CDATA[       01 WS-SECRET-NOTE PIC X(10) VALUE "WSMARKER".]]></working-storage>
  <file-control><![CDATA[           SELECT FCMARKER ASSIGN TO "x.dat".]]></file-control>
  <user-procedures><Event name="PROC-ONE" paragraph="PROC-ONE"><![CDATA[           DISPLAY "PROCMARKER".]]></Event></user-procedures>
  <form-events><Event name="onLoad" paragraph="MAIN--ONLOAD"><![CDATA[           DISPLAY "LOADMARKER".]]></Event></form-events>
  <Control id="BTN-OK" type="Button" x="10" y="10" w="80" h="30" tab-order="0" z-order="0" visible="true" enabled="true">
    <Property name="Caption">OK</Property>
    <Event name="onClick" paragraph="BTN-OK--ONCLICK"><![CDATA[           DISPLAY "CLICKMARKER".]]></Event>
  </Control>
  <deleted-controls><DeletedControl id="OLD" deleted-at="2026-01-01"><Event name="onClick" paragraph="OLD--ONCLICK"><![CDATA[ DISPLAY "BINMARKER".]]></Event></DeletedControl></deleted-controls>
</Form>"##;

    /// R4 — none of the program reaches the description; the bound events are named.
    #[test]
    fn a_description_carries_no_program_code() {
        let form = load_form_from_str(WITH_CODE).unwrap();
        let json = to_json(&describe(&form));
        for marker in [
            "WSMARKER",
            "FCMARKER",
            "PROCMARKER",
            "LOADMARKER",
            "CLICKMARKER",
            "BINMARKER",
            "DISPLAY",
        ] {
            assert!(
                !json.contains(marker),
                "the description carries `{marker}`:\n{json}"
            );
        }
        assert!(
            json.contains("BTN-OK--ONCLICK") && json.contains("MAIN--ONLOAD"),
            "the bound handlers are named"
        );
        println!(
            "no code in the description: 7 markers absent, {} bytes",
            json.len()
        );
    }

    #[test]
    fn a_form_without_layers_writes_none_and_layers_round_trip_in_stack_order() {
        let plain = load_form_from_str(WITH_CODE).unwrap();
        let json = to_json(&describe(&plain));
        assert!(
            !json.contains("\"layers\""),
            "no layers key for a form without layers"
        );
        assert!(!json.contains("\"layer\""), "no control carries a layer");

        let mut layered = plain.clone();
        let mut top = Layer::new("TOP-LAYER");
        top.backdrop.color = "#11223344".into();
        layered.layers = vec![Layer::new("MID-LAYER"), top];
        layered.controls[0].layer = Some("TOP-LAYER".into());
        let d = from_json(&to_json(&describe(&layered))).unwrap();
        assert_eq!(
            d.layers.iter().map(|l| l.name.as_str()).collect::<Vec<_>>(),
            ["MID-LAYER", "TOP-LAYER"]
        );
        let back = to_form(&d).unwrap();
        assert_eq!(back.layers, layered.layers);
        assert_eq!(back.controls[0].layer.as_deref(), Some("TOP-LAYER"));
        let layer_json = serde_json::to_value(&d.layers[0]).unwrap();
        let mut keys: Vec<_> = layer_json.as_object().unwrap().keys().cloned().collect();
        keys.sort();
        assert_eq!(keys, ["backdrop", "name"], "a layer carries no Visible");
    }

    #[test]
    fn a_description_of_another_format_is_refused_with_both_numbers() {
        let mut d = describe(&load_form_from_str(WITH_CODE).unwrap());
        d.format = DESCRIPTION_FORMAT + 1;
        let err = from_json(&to_json(&d)).unwrap_err();
        let shown = err.to_string();
        assert!(
            shown.contains(&(DESCRIPTION_FORMAT + 1).to_string())
                && shown.contains(&DESCRIPTION_FORMAT.to_string()),
            "{shown}"
        );
        assert_eq!(
            to_form(&d).unwrap_err(),
            DescriptionError::Format {
                found: DESCRIPTION_FORMAT + 1,
                supported: DESCRIPTION_FORMAT
            }
        );
        println!("refused: {shown}");
    }

    #[test]
    fn a_credential_value_is_not_described() {
        let mut form = load_form_from_str(WITH_CODE).unwrap();
        let key = "APIKey";
        assert!(
            crate::connections::is_credential_prop(key),
            "the credential list names {key}"
        );
        form.controls[0]
            .properties
            .insert(key.into(), PropValue::String("sk-live-CREDMARKER".into()));
        let json = to_json(&describe(&form));
        assert!(!json.contains("CREDMARKER"), "{json}");
        assert!(
            json.contains(&format!("\"{key}\"")),
            "the property itself is kept"
        );
    }
}
