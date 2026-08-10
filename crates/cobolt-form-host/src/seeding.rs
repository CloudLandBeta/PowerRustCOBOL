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
    let b = |v: bool| if v { "1" } else { "0" }.to_string();
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
        ],
    );
    std::iter::once(form_entry)
        .chain(flat.iter().map(|c| {
            let mut props: Vec<(String, String)> = c
                .properties
                .iter()
                .map(|(k, v)| (k.clone(), v.to_xml_string()))
                .collect();
            let b = |v: bool| if v { "1" } else { "0" }.to_string();
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
        assert_eq!(get("Visible"), Some("1"));
        assert_eq!(get("Enabled"), Some("1"));
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
}
