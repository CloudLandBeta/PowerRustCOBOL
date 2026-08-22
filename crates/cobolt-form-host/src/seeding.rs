// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Object-registry seeding (spec 042 R20), shared by every form host.
//!
//! The interpreter's visual-object registry is seeded with every control's
//! designed properties before the program runs, so property references and
//! method getters return the configured values before any setter runs.
//! A host that skips this (the compiled application did, until spec 042)
//! answers property reads with nothing instead of the design.

/// Env var the IDE sets on a form-host child when the project has a Google
/// Maps API key configured (spec 039 T12/R23) — resolved IDE-side from
/// `LlmConfig.api_keys`, never written to the `.cfrm`/`.cbl`/project file, so
/// a Maps control's key never lands on disk or in generated COBOL. Kept in
/// sync with `cobolt-ide/src/form_runtime.rs`'s constant of the same name.
pub const GOOGLE_MAPS_API_KEY_ENV: &str = "COBOLT_GOOGLE_MAPS_API_KEY";

/// Env var for a resolved Google Custom Search API key (spec 039 T15/R30) —
/// same discipline as [`GOOGLE_MAPS_API_KEY_ENV`]. Kept in sync with
/// `cobolt-ide/src/form_runtime.rs`'s constant of the same name.
pub const GOOGLE_SEARCH_API_KEY_ENV: &str = "COBOLT_GOOGLE_SEARCH_API_KEY";

/// The (maps, search) API keys from this process's environment — `None` when
/// unset or blank.
pub fn resolve_api_keys() -> (Option<String>, Option<String>) {
    let read = |name: &str| {
        std::env::var(name)
            .ok()
            .filter(|v| !v.trim().is_empty())
    };
    (
        read(GOOGLE_MAPS_API_KEY_ENV),
        read(GOOGLE_SEARCH_API_KEY_ENV),
    )
}

/// Build the interpreter object seed: one `(id, control type, props)` entry per
/// designed control, carrying its designed properties, the standard
/// geometry/identity props (`Name`/`Visible`/`Enabled`/`X`/`Y`/`Width`/
/// `Height`/`TabOrder`), the `_Binding*` seeds data binding reads, and the
/// resolved API keys for Maps / WebSearch controls. Feed the result to
/// `Interpreter::seed_objects`.
pub fn build_object_seed(
    form: &cobolt_forms::Form,
    flat: &[cobolt_forms::Control],
    maps_api_key: Option<&str>,
    search_api_key: Option<&str>,
) -> Vec<(String, String, Vec<(String, String)>)> {
    // 049 R30/R33 — the FORM ITSELF is seeded as an object, carrying the
    // universal form surface, so `me::Width` (and `<FORM-NAME>::Width`)
    // read the designed values from the first frame. Before this the form
    // had no registry entry at all and every form-property read was empty.
    // Same one spelling the controls get — the form's own booleans are read by
    // the same COBOL and must compare the same way.
    let b = |v: bool| cobolt_forms::model::bool_text(v).to_string();
    let form_entry = (
        form.name.clone(),
        "Form".to_string(),
        vec![
            ("Name".into(), form.name.clone()),
            ("Title".into(), form.title.clone()),
            ("Width".into(), form.width.to_string()),
            ("Height".into(), form.height.to_string()),
            ("X".into(), form.x.to_string()),
            ("Y".into(), form.y.to_string()),
            ("WindowState".into(), form.window_state.as_str().to_string()),
            ("FullScreen".into(), b(form.full_screen)),
            ("TitleVisible".into(), b(form.title_visible)),
            ("CanMinimize".into(), b(form.can_minimize)),
            ("CanMaximize".into(), b(form.can_maximize)),
            // Design-time FormState is always Ready (spec 037 R16).
            ("FormState".into(), "Ready".to_string()),
            ("FormFormat".into(), form.form_format.as_str().to_string()),
            ("BackgroundColor".into(), form.background_color.clone()),
            ("Transparency".into(), form.transparency.to_string()),
            // The breadcrumb RESET guard: while it is on, a click on this
            // form's own breadcrumb segment refuses to start the form over and
            // fires `onResetRejected` instead. Off by default — a form that
            // holds nothing worth losing needs no guard.
            ("PreventReset".into(), "0".to_string()),
        ],
    );
    std::iter::once(form_entry)
        .chain(flat.iter().map(|c| {
            let mut props: Vec<(String, String)> = c
                .properties
                .iter()
                .map(|(k, v)| {
                    // Booleans get ONE spelling on the way in. The stored value
                    // may be a `Bool`, or the string "true", or "1", depending
                    // on whether it came from the designer, the .cfrm reader or
                    // a previous runtime write — and COBOL then had to compare
                    // against whichever it happened to be.
                    let text = cobolt_forms::model::property_runtime_text(
                        c.control_type.as_str(),
                        k,
                        &v.to_xml_string(),
                    );
                    (k.clone(), text)
                })
                .collect();
            let b = |v: bool| cobolt_forms::model::bool_text(v).to_string();
            props.push(("Name".into(), c.id.clone()));
            props.push(("Visible".into(), b(c.visible)));
            props.push(("Enabled".into(), b(c.enabled)));
            props.push(("X".into(), c.rect.x.to_string()));
            props.push(("Y".into(), c.rect.y.to_string()));
            props.push(("Width".into(), c.rect.w.to_string()));
            props.push(("Height".into(), c.rect.h.to_string()));
            props.push(("TabOrder".into(), c.tab_order.to_string()));
            append_data_binding_seed_props(form, &c.id, &mut props);
            if c.control_type == cobolt_forms::ControlType::Maps {
                if let Some(key) = maps_api_key {
                    props.push(("_ResolvedMapsApiKey".into(), key.to_owned()));
                }
            }
            if c.control_type == cobolt_forms::ControlType::WebSearch {
                if let Some(key) = search_api_key {
                    props.push(("_ResolvedSearchApiKey".into(), key.to_owned()));
                }
            }
            (c.id.clone(), c.control_type.as_str().to_string(), props)
        }))
        .chain(flat.iter().flat_map(toolbar_button_seed))
        .collect()
}

/// One entry per toolbar BUTTON, under the derived id it answers to
/// (`<toolbar>-<group>-<button>`) and the class `ToolbarButton`.
///
/// A button is not a control, so nothing above seeds it — and without a registry
/// entry COBOL could neither read a button's tooltip nor be told that writing its
/// width is refused, because the runtime would have no idea the id named a
/// button. The seeded props are the button's own designed values, resolved
/// through its group so a colour set once on the group reads back on every button
/// in it.
fn toolbar_button_seed(
    ctrl: &cobolt_forms::Control,
) -> Vec<(String, String, Vec<(String, String)>)> {
    if ctrl.control_type != cobolt_forms::ControlType::ToolBar {
        return Vec::new();
    }
    let def = cobolt_forms::toolbar::ToolbarDef::from_control(ctrl);
    def.buttons()
        .map(|(group, button)| {
            let style = button.resolved(group);
            let id = cobolt_forms::toolbar::button_control_id(&ctrl.id, &group.id, &button.id);
            let props = vec![
                ("Name".to_string(), id.clone()),
                ("ToolBar".to_string(), ctrl.id.clone()),
                ("Group".to_string(), group.id.clone()),
                ("Button".to_string(), button.id.clone()),
                ("Label".to_string(), button.label.clone()),
                ("Icon".to_string(), button.icon.clone()),
                ("Tooltip".to_string(), button.tooltip.clone()),
                (
                    "Enabled".to_string(),
                    if button.enabled { "1" } else { "0" }.to_string(),
                ),
                ("Action".to_string(), button.action.clone()),
                ("BackgroundColor".to_string(), style.background_color),
                ("ForegroundColor".to_string(), style.foreground_color),
                ("IconColor".to_string(), style.icon_color),
                ("GradientStartColor".to_string(), style.gradient_start_color),
                ("GradientEndColor".to_string(), style.gradient_end_color),
                ("ShadowColor".to_string(), style.shadow_color),
                ("Width".to_string(), style.width.to_string()),
                ("Height".to_string(), style.height.to_string()),
            ];
            (id, "ToolbarButton".to_string(), props)
        })
        .collect()
}

/// Seed `_Binding*` props for DataGrid, databound repeating GroupBoxes
/// (ControlArray) and standalone scalar controls (spec 039 R21), so
/// `RefreshBinding()` has what it needs at runtime.
pub fn append_data_binding_seed_props(
    form: &cobolt_forms::Form,
    control_id: &str,
    props: &mut Vec<(String, String)>,
) {
    let binding = form.data_bindings.iter().find(|binding| {
        match &binding.target {
            cobolt_forms::BindingTargetDescriptor::DataGrid {
                control_id: target_id,
            } => target_id.eq_ignore_ascii_case(control_id),
            cobolt_forms::BindingTargetDescriptor::ControlArray { array_id, .. } => {
                array_id.eq_ignore_ascii_case(control_id)
                    || form.controls.iter().any(|c| {
                        c.id.eq_ignore_ascii_case(control_id)
                            && c.explicit_control_array_id().as_deref() == Some(array_id.as_str())
                    })
            }
            cobolt_forms::BindingTargetDescriptor::ScalarControl {
                control_id: target_id,
            } => target_id.eq_ignore_ascii_case(control_id),
            cobolt_forms::BindingTargetDescriptor::MarkerCollection {
                control_id: target_id,
            } => target_id.eq_ignore_ascii_case(control_id),
            cobolt_forms::BindingTargetDescriptor::Chart { .. }
            | cobolt_forms::BindingTargetDescriptor::ComboBox { .. }
            | cobolt_forms::BindingTargetDescriptor::ListBox { .. } => false,
        }
    });
    let Some(binding) = binding else {
        return;
    };
    let cobolt_forms::BindingSourceDescriptor::CobolTable { fields, .. } = &binding.source else {
        return;
    };
    props.push(("_BindingKind".into(), "CobolTable".into()));
    props.push((
        "_BindingFields".into(),
        fields
            .iter()
            .map(|field| field.name.as_str())
            .collect::<Vec<_>>()
            .join("\n"),
    ));
    if matches!(
        &binding.target,
        cobolt_forms::BindingTargetDescriptor::ControlArray { .. }
    ) {
        props.push(("_BindingArray".into(), "1".into()));
        let maps: Vec<String> = binding
            .mappings
            .iter()
            .filter_map(|m| {
                if let cobolt_forms::BindingTargetPath::ControlProperty {
                    control_id: member,
                    property_name: prop,
                    ..
                } = &m.target
                {
                    Some(format!("{}\t{}\t{}", m.source_field, member, prop))
                } else {
                    None
                }
            })
            .collect();
        if !maps.is_empty() {
            props.push(("_BindingMappings".into(), maps.join("\n")));
        }
    }
    if let cobolt_forms::BindingTargetDescriptor::ScalarControl { .. } = &binding.target {
        if let Some(scalar_field) = binding
            .mappings
            .iter()
            .find(|m| matches!(&m.target, cobolt_forms::BindingTargetPath::ScalarValue { .. }))
            .map(|m| m.source_field.clone())
        {
            let property = form
                .find_control(control_id)
                .and_then(|c| c.scalar_binding_property())
                .unwrap_or("Value");
            props.push(("_BindingScalarField".into(), scalar_field));
            props.push(("_BindingScalarProperty".into(), property.to_owned()));
        }
    }
    if let cobolt_forms::BindingTargetDescriptor::MarkerCollection { .. } = &binding.target {
        // Spec 039 T13/R22: seed one source field per marker attribute (in a
        // fixed order — refresh_marker_binding in interpreter.rs reads them
        // positionally).
        if let Some(spec) = marker_binding_seed(binding) {
            props.push(("_BindingMarkerFields".into(), spec));
        }
    }
}

/// Build the `_BindingMarkerFields` seed value (`id\tlat\tlng\tlabel\tinfo`,
/// any entry empty except lat/lng — enforced by the Guardian before a binding
/// can be saved) from a `MarkerCollection` binding's field mappings.
pub fn marker_binding_seed(binding: &cobolt_forms::DataBindingDef) -> Option<String> {
    let field_for = |target: cobolt_forms::MapMarkerField| -> String {
        binding
            .mappings
            .iter()
            .find_map(|m| match &m.target {
                cobolt_forms::BindingTargetPath::MarkerField { field, .. } if *field == target => {
                    Some(m.source_field.clone())
                }
                _ => None,
            })
            .unwrap_or_default()
    };
    let lat = field_for(cobolt_forms::MapMarkerField::Lat);
    let lng = field_for(cobolt_forms::MapMarkerField::Lng);
    if lat.is_empty() || lng.is_empty() {
        return None;
    }
    let id = field_for(cobolt_forms::MapMarkerField::Id);
    let label = field_for(cobolt_forms::MapMarkerField::Label);
    let info = field_for(cobolt_forms::MapMarkerField::Info);
    Some(format!("{id}\t{lat}\t{lng}\t{label}\t{info}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use cobolt_forms::{Control, ControlType, Form, PropValue};

    fn form_with(label: Control) -> (Form, Vec<Control>) {
        let mut form = Form::new("TEST-FORM", "Test", 400, 300);
        form.controls.push(label);
        let flat = form.controls.clone();
        (form, flat)
    }

    /// A read-before-write returns the DESIGNED value: caption and geometry
    /// are present in the seed entry for the control (R20 / AC9's registry
    /// half).
    #[test]
    fn designed_caption_and_geometry_are_seeded() {
        let mut label = Control::new("Label-1", ControlType::Label, 10, 20);
        label.set_prop("Caption", PropValue::String("Designed!".into()));
        label.rect.w = 120;
        label.rect.h = 30;
        label.tab_order = 3;
        let (form, flat) = form_with(label);

        let seed = build_object_seed(&form, &flat, None, None);
        // 049 R30 — entry 0 is the FORM itself (universal surface), controls
        // follow.
        assert_eq!(seed.len(), 2);
        let (fid, fkind, fprops) = &seed[0];
        assert_eq!(fid, "TEST-FORM");
        assert_eq!(fkind, "Form");
        let fget = |k: &str| {
            fprops
                .iter()
                .find(|(name, _)| name == k)
                .map(|(_, v)| v.as_str())
        };
        assert_eq!(fget("Title"), Some("Test"));
        assert_eq!(fget("Width"), Some("400"));
        assert_eq!(fget("Height"), Some("300"));
        assert_eq!(fget("FormState"), Some("Ready"));
        assert_eq!(fget("FormFormat"), Some("Standalone"));
        let (id, kind, props) = &seed[1];
        assert_eq!(id, "Label-1");
        assert_eq!(kind, "Label");
        let get = |k: &str| {
            props
                .iter()
                .find(|(name, _)| name == k)
                .map(|(_, v)| v.as_str())
        };
        assert_eq!(get("Caption"), Some("Designed!"));
        assert_eq!(get("X"), Some("10"));
        assert_eq!(get("Y"), Some("20"));
        assert_eq!(get("Width"), Some("120"));
        assert_eq!(get("Height"), Some("30"));
        assert_eq!(get("TabOrder"), Some("3"));
        assert_eq!(get("Name"), Some("Label-1"));
        // Booleans are seeded in the one spelling COBOL reads them in — `true`,
        // not `1`. `Visible` used to arrive as a digit while `Checked` arrived
        // as a word, so no single comparison could be right for both
        // (operator, 2026-08-21).
        assert_eq!(get("Visible"), Some("true"));
        assert_eq!(get("Enabled"), Some("true"));
    }

    /// API-key seeds appear only on the matching control type and only when a
    /// key was provided.
    #[test]
    fn api_keys_seed_only_where_provided() {
        let maps = Control::new("Map-1", ControlType::Maps, 0, 0);
        let (form, flat) = form_with(maps);

        // Entry 0 is the form (049); the Maps control is entry 1.
        let none = build_object_seed(&form, &flat, None, None);
        assert!(none[1].2.iter().all(|(k, _)| k != "_ResolvedMapsApiKey"));

        let some = build_object_seed(&form, &flat, Some("KEY-123"), None);
        assert!(some[1]
            .2
            .iter()
            .any(|(k, v)| k == "_ResolvedMapsApiKey" && v == "KEY-123"));
        // The search key never lands on a Maps control.
        let cross = build_object_seed(&form, &flat, None, Some("SRCH"));
        assert!(cross[1].2.iter().all(|(k, _)| k != "_ResolvedSearchApiKey"));
    }

    /// A toolbar's BUTTONS are seeded as objects of this form, so the form's own
    /// COBOL can address them (operator, 2026-08-17).
    ///
    /// This one builder serves every form instance there is — the root form (via
    /// `rcrun run-form` and a compiled binary), a child window, and a ContentPane
    /// occupant (`FormHost::build_form_instance`) — so a toolbar's buttons exist in
    /// the interpreter of whichever form holds the toolbar, whether that form is
    /// Standalone or Embedded, and in no other. That is the point of pinning it
    /// here: the two bugs before this one were both a fix that reached one form
    /// path and not the other.
    #[test]
    fn a_toolbars_buttons_are_seeded_as_objects_of_their_own_form() {
        use cobolt_forms::toolbar::{
            button_control_id, ToolbarButton, ToolbarDef, ToolbarGroup, TOOLBAR_DEF_PROP,
        };

        let mut group = ToolbarGroup::new("group-1", "File");
        let mut save = ToolbarButton::new("button-1", "");
        save.set_icon("folder-open");
        save.tooltip = "Open a record".into();
        // A colour set on the GROUP is what an unset button inherits, so the seeded
        // value has to be the RESOLVED one or a read-back would come up empty.
        group.button_defaults.background_color = "#204080FF".into();
        group.buttons.push(save);
        let mut disabled = ToolbarButton::new("button-2", "Find");
        disabled.enabled = false;
        group.buttons.push(disabled);
        let def = ToolbarDef {
            groups: vec![group],
            button_gap: 4,
        };

        // Nested inside a Panel — a toolbar does not have to sit at the top level,
        // and the seed walks the FLAT list, which is what makes that work.
        let mut bar = Control::new("TB", ControlType::ToolBar, 0, 0);
        bar.set_prop(TOOLBAR_DEF_PROP, PropValue::String(def.to_json().unwrap()));
        let panel = Control::new("PANEL-1", ControlType::Panel, 0, 0);

        // An EMBEDDED form: the case the operator is building — a toolbar in a form
        // loaded into a ContentPane by a sidebar.
        let mut form = cobolt_forms::Form::new("EMB-FORM", "Embedded", 400, 300);
        form.form_format = cobolt_forms::model::FormFormat::Embedded;
        let flat = vec![panel, bar];
        form.controls = flat.clone();

        let seed = build_object_seed(&form, &flat, None, None);
        let entry = |id: &str| {
            seed.iter()
                .find(|(seed_id, _, _)| seed_id == id)
                .unwrap_or_else(|| panic!("{id} is not in the seed: {:?}", ids(&seed)))
        };

        let b1 = button_control_id("TB", "group-1", "button-1");
        let b2 = button_control_id("TB", "group-1", "button-2");
        assert_eq!(b1, "TB-GROUP-1-BUTTON-1");

        for id in [&b1, &b2] {
            let (_, class, _) = entry(id);
            assert_eq!(
                class, "ToolbarButton",
                "{id} must be seeded as a button, not as a control"
            );
        }

        let (_, _, props) = entry(&b1);
        let get = |k: &str| {
            props
                .iter()
                .find(|(name, _)| name == k)
                .map(|(_, v)| v.as_str())
        };
        assert_eq!(get("Tooltip"), Some("Open a record"));
        assert_eq!(get("Icon"), Some("folder-open"));
        assert_eq!(get("Enabled"), Some("1"));
        assert_eq!(
            get("BackgroundColor"),
            Some("#204080FF"),
            "a colour inherited from the group must read back on the button"
        );
        // Where the button is, so a handler can find its way back to the toolbar.
        assert_eq!(get("ToolBar"), Some("TB"));
        assert_eq!(get("Group"), Some("group-1"));
        assert_eq!(get("Button"), Some("button-1"));

        let (_, _, props2) = entry(&b2);
        assert_eq!(
            props2
                .iter()
                .find(|(k, _)| k == "Enabled")
                .map(|(_, v)| v.as_str()),
            Some("0"),
            "a button designed disabled reads back disabled"
        );

        // A form with NO toolbar seeds no buttons — nothing is invented.
        let (plain, plain_flat) = form_with(Control::new("Label-1", ControlType::Label, 0, 0));
        let plain_seed = build_object_seed(&plain, &plain_flat, None, None);
        assert!(
            plain_seed.iter().all(|(_, class, _)| class != "ToolbarButton"),
            "a form without a toolbar must seed no buttons"
        );

        println!(
            "\n  Toolbar button seeding — an EMBEDDED form with a ToolBar nested in a Panel \
             seeds {} objects: the form, 2 controls and 2 ToolbarButtons ({b1}, {b2}) \
             carrying their tooltip, icon, enabled flag, their group-inherited colour and \
             where they live. ONE builder serves the root form, a child window and a \
             ContentPane occupant, so a toolbar's buttons exist in its own form's \
             interpreter and nowhere else\n",
            seed.len()
        );
    }

    fn ids(seed: &[(String, String, Vec<(String, String)>)]) -> Vec<&str> {
        seed.iter().map(|(id, _, _)| id.as_str()).collect()
    }
}
