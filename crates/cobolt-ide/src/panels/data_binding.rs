// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Data-binding panel helpers shared by the Properties inspector and editor.

use crate::i18n::Tr;
use cobolt_forms::{
    ApprovedBindingTargetKind, BindingDataType, BindingField, BindingTargetDescriptor,
    BindingTargetPath, Control, ControlType, FieldMapping, Form,
};
#[cfg(test)]
use cobolt_forms::{BindingSourceDescriptor, DataBindingDef};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DataBindingVisibility {
    Hidden,
    ApprovedTarget(ApprovedBindingTargetKind),
    ArrayMemberMapping { array_id: String, member_id: String },
}

pub fn visibility_for_control(form: &Form, control: &Control) -> DataBindingVisibility {
    if let Some((array_id, member_id)) = form.array_binding_context_for_member(&control.id) {
        return DataBindingVisibility::ArrayMemberMapping {
            array_id,
            member_id,
        };
    }
    match control.approved_binding_target_kind() {
        Some(kind) => DataBindingVisibility::ApprovedTarget(kind),
        None => DataBindingVisibility::Hidden,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BindingEditorSourceKind {
    IndexedFile,
    Sql,
    CobolTable,
    RestApi,
    AgentAi,
}

impl BindingEditorSourceKind {
    pub const ALL: [Self; 5] = [
        Self::IndexedFile,
        Self::Sql,
        Self::CobolTable,
        Self::RestApi,
        Self::AgentAi,
    ];

    pub fn label(self, tr: &Tr) -> &'static str {
        match self {
            Self::IndexedFile => tr.data_binding_source_indexed,
            Self::Sql => tr.data_binding_source_sql,
            Self::CobolTable => tr.data_binding_source_cobol_table,
            Self::RestApi => tr.data_binding_source_rest,
            Self::AgentAi => tr.data_binding_source_agent,
        }
    }

    pub(crate) fn id_fragment(self) -> &'static str {
        match self {
            Self::IndexedFile => "IDX",
            Self::Sql => "SQL",
            Self::CobolTable => "TABLE",
            Self::RestApi => "REST",
            Self::AgentAi => "AGENT",
        }
    }
}

#[cfg(test)]
pub fn create_binding_from_editor(
    form: &Form,
    control: &Control,
    source_kind: BindingEditorSourceKind,
) -> Option<cobolt_forms::DataBindingDef> {
    let target = form.binding_target_descriptor_for_control(&control.id)?;
    let fields = default_editor_fields();
    let source = source_descriptor_for_editor(source_kind, &control.id, fields.clone());
    let display_name = format!(
        "{} -> {}",
        source_kind.id_fragment(),
        target.primary_control_id()
    );
    let binding_id = next_binding_id(form, source_kind, target.primary_control_id());
    Some(
        DataBindingDef::new(binding_id, display_name, source, target.clone())
            .with_mappings(default_mappings_for_target(form, &target, &fields)),
    )
}

#[cfg(test)]
pub fn source_descriptor_for_editor(
    source_kind: BindingEditorSourceKind,
    control_id: &str,
    fields: Vec<BindingField>,
) -> BindingSourceDescriptor {
    let key_fields = key_field_names(&fields);
    match source_kind {
        BindingEditorSourceKind::IndexedFile => BindingSourceDescriptor::IndexedFile {
            definition_path: format!("data/{}.cidx", normalized_id(control_id).to_lowercase()),
            record_name: format!("{}-RECORD", normalized_id(control_id)),
            key_field: key_fields.first().cloned(),
            fields,
            writable: true,
        },
        BindingEditorSourceKind::Sql => BindingSourceDescriptor::Sql {
            source_control_id: format!("SQL-{}", normalized_id(control_id)),
            query_name: format!("{}-QUERY", normalized_id(control_id)),
            result_set_name: format!("{}-RESULT", normalized_id(control_id)),
            fields,
            key_fields,
            writable: true,
        },
        BindingEditorSourceKind::CobolTable => BindingSourceDescriptor::CobolTable {
            table_name: format!("{}-TABLE", normalized_id(control_id)),
            occurs_item: format!("{}-ROW", normalized_id(control_id)),
            fields,
            key_fields,
            writable: true,
        },
        BindingEditorSourceKind::RestApi => BindingSourceDescriptor::RestApi {
            source_control_id: format!("REST-{}", normalized_id(control_id)),
            endpoint_name: format!("{}-ENDPOINT", normalized_id(control_id)),
            response_data_item: format!("{}-RESPONSE", normalized_id(control_id)),
            fields,
            update: None,
        },
        BindingEditorSourceKind::AgentAi => BindingSourceDescriptor::AgentAi {
            source_control_id: format!("AGENT-{}", normalized_id(control_id)),
            output_name: format!("{}-OUTPUT", normalized_id(control_id)),
            fields,
            update: None,
        },
    }
}

pub fn default_mappings_for_target(
    form: &Form,
    target: &BindingTargetDescriptor,
    fields: &[BindingField],
) -> Vec<FieldMapping> {
    let Some(first) = fields.first() else {
        return Vec::new();
    };
    match target {
        BindingTargetDescriptor::DataGrid { control_id } => fields
            .iter()
            .map(|field| {
                FieldMapping::new(
                    field.name.clone(),
                    BindingTargetPath::GridColumn {
                        control_id: control_id.clone(),
                        column_id: field.name.clone(),
                    },
                )
            })
            .collect(),
        BindingTargetDescriptor::Chart { control_id, .. } => {
            let mut mappings = vec![FieldMapping::new(
                first.name.clone(),
                BindingTargetPath::ChartCategory {
                    control_id: control_id.clone(),
                },
            )];
            let value_fields: Vec<&BindingField> = fields
                .iter()
                .filter(|field| {
                    matches!(
                        field.data_type,
                        BindingDataType::Integer | BindingDataType::Decimal
                    )
                })
                .collect();
            for field in value_fields.into_iter().take(2) {
                mappings.push(FieldMapping::new(
                    field.name.clone(),
                    BindingTargetPath::ChartValueSeries {
                        control_id: control_id.clone(),
                        series_id: field.name.clone(),
                    },
                ));
            }
            mappings
        }
        BindingTargetDescriptor::ComboBox { control_id }
        | BindingTargetDescriptor::ListBox { control_id } => {
            let value = fields.iter().find(|field| field.key).unwrap_or(first);
            vec![
                FieldMapping::new(
                    first.name.clone(),
                    BindingTargetPath::ListDisplayItem {
                        control_id: control_id.clone(),
                    },
                ),
                FieldMapping::new(
                    value.name.clone(),
                    BindingTargetPath::ListValue {
                        control_id: control_id.clone(),
                    },
                ),
            ]
        }
        BindingTargetDescriptor::ControlArray {
            array_id,
            member_control_ids,
        } => fields
            .iter()
            .zip(member_control_ids.iter())
            .map(|(field, member_id)| {
                FieldMapping::new(
                    field.name.clone(),
                    BindingTargetPath::ControlProperty {
                        array_id: array_id.clone(),
                        control_id: member_id.clone(),
                        property_name: default_member_property(form, member_id),
                    },
                )
            })
            .collect(),
    }
}

#[cfg(test)]
pub fn default_editor_fields() -> Vec<BindingField> {
    vec![
        BindingField::new("ID", BindingDataType::Integer).key(),
        BindingField::new("NAME", BindingDataType::Text).required(),
        BindingField::new("AMOUNT", BindingDataType::Decimal).required(),
        BindingField::new("ACTIVE", BindingDataType::Boolean),
    ]
}

#[cfg(test)]
fn key_field_names(fields: &[BindingField]) -> Vec<String> {
    fields
        .iter()
        .filter(|field| field.key)
        .map(|field| field.name.clone())
        .collect()
}

/// The default bindable property for a member control of a repeating group — the
/// property a mapped source field fills in every repeated item.
pub fn default_member_property(form: &Form, member_id: &str) -> String {
    match form
        .find_control(member_id)
        .map(|control| &control.control_type)
    {
        Some(ControlType::CheckBox) | Some(ControlType::RadioButton) => "Checked",
        Some(ControlType::Label) | Some(ControlType::Button) | Some(ControlType::GroupBox) => {
            "Caption"
        }
        Some(ControlType::PictureBox) => "ImagePath",
        Some(ControlType::ComboBox) | Some(ControlType::ListBox) => "Value",
        Some(ControlType::NumericUpDown)
        | Some(ControlType::Slider)
        | Some(ControlType::ProgressBar)
        | Some(ControlType::DateTimePicker) => "Value",
        _ => "Text",
    }
    .to_string()
}

#[cfg(test)]
fn next_binding_id(
    form: &Form,
    source_kind: BindingEditorSourceKind,
    target_control_id: &str,
) -> String {
    let base = format!(
        "BIND-{}-{}",
        source_kind.id_fragment(),
        normalized_id(target_control_id)
    );
    if !form
        .data_bindings
        .iter()
        .any(|binding| binding.id.eq_ignore_ascii_case(&base))
    {
        return base;
    }
    for n in 2.. {
        let candidate = format!("{base}-{n}");
        if !form
            .data_bindings
            .iter()
            .any(|binding| binding.id.eq_ignore_ascii_case(&candidate))
        {
            return candidate;
        }
    }
    unreachable!("unbounded suffix search must return")
}

#[cfg(test)]
fn normalized_id(id: &str) -> String {
    let mut out = String::with_capacity(id.len());
    for ch in id.chars() {
        if ch.is_ascii_alphanumeric() {
            out.push(ch.to_ascii_uppercase());
        } else {
            out.push('-');
        }
    }
    out.trim_matches('-').to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use cobolt_forms::{BindingSourceKind, BindingTargetPath, Control, ControlType, PropValue};

    #[test]
    fn data_binding_properties_show_only_approved_targets() {
        let mut form = Form::new("MAIN", "Main", 800, 600);
        let grid = Control::new("GRID-1", ControlType::DataGrid, 0, 0);
        let chart = Control::new("CHART-1", ControlType::BarChart, 0, 220);
        let combo = Control::new("COMBO-1", ControlType::ComboBox, 0, 460);
        let list = Control::new("LIST-1", ControlType::ListBox, 0, 500);
        let scalar = Control::new("TEXT-1", ControlType::TextBox, 0, 620);
        form.controls = vec![
            grid.clone(),
            chart.clone(),
            combo.clone(),
            list.clone(),
            scalar.clone(),
        ];

        assert!(matches!(
            visibility_for_control(&form, &grid),
            DataBindingVisibility::ApprovedTarget(ApprovedBindingTargetKind::DataGrid)
        ));
        assert!(matches!(
            visibility_for_control(&form, &chart),
            DataBindingVisibility::ApprovedTarget(ApprovedBindingTargetKind::Chart(_))
        ));
        assert!(matches!(
            visibility_for_control(&form, &combo),
            DataBindingVisibility::ApprovedTarget(ApprovedBindingTargetKind::ComboBox)
        ));
        assert!(matches!(
            visibility_for_control(&form, &list),
            DataBindingVisibility::ApprovedTarget(ApprovedBindingTargetKind::ListBox)
        ));
        assert_eq!(
            visibility_for_control(&form, &scalar),
            DataBindingVisibility::Hidden
        );
    }

    #[test]
    fn data_binding_properties_show_array_member_mapping_context_only() {
        let mut form = Form::new("MAIN", "Main", 800, 600);
        let mut group = Control::new("ROWS", ControlType::GroupBox, 0, 0);
        group.set_prop("IsRepeatingGroup", PropValue::Bool(true));
        group.set_prop("ArrayName", PropValue::String("CUSTOMERS".into()));
        let mut scalar = Control::new("NAME", ControlType::TextBox, 10, 10);
        scalar.parent = Some("ROWS".into());
        form.controls = vec![group.clone(), scalar.clone()];

        assert_eq!(
            visibility_for_control(&form, &scalar),
            DataBindingVisibility::ArrayMemberMapping {
                array_id: "CUSTOMERS".into(),
                member_id: "NAME".into(),
            }
        );
        assert_eq!(
            visibility_for_control(&form, &group),
            DataBindingVisibility::ApprovedTarget(ApprovedBindingTargetKind::ControlArray)
        );
    }

    #[test]
    fn data_binding_editor_builds_each_source_descriptor() {
        let fields = default_editor_fields();
        let descriptors: Vec<BindingSourceDescriptor> = BindingEditorSourceKind::ALL
            .iter()
            .map(|kind| source_descriptor_for_editor(*kind, "Grid-1", fields.clone()))
            .collect();

        assert_eq!(descriptors[0].kind(), BindingSourceKind::IndexedFile);
        assert_eq!(descriptors[1].kind(), BindingSourceKind::Sql);
        assert_eq!(descriptors[2].kind(), BindingSourceKind::CobolTable);
        assert_eq!(descriptors[3].kind(), BindingSourceKind::RestApi);
        assert_eq!(descriptors[4].kind(), BindingSourceKind::AgentAi);
        assert!(descriptors
            .iter()
            .all(|descriptor| descriptor.fields().len() == fields.len()));
    }

    #[test]
    fn data_binding_editor_maps_grid_chart_list_and_array_targets() {
        let fields = default_editor_fields();
        let mut form = Form::new("MAIN", "Main", 800, 600);
        let grid = Control::new("GRID-1", ControlType::DataGrid, 0, 0);
        let chart = Control::new("CHART-1", ControlType::BarChart, 0, 100);
        let combo = Control::new("COMBO-1", ControlType::ComboBox, 0, 200);
        let list = Control::new("LIST-1", ControlType::ListBox, 0, 300);
        let mut group = Control::new("ROWS", ControlType::GroupBox, 0, 400);
        group.set_prop("IsRepeatingGroup", PropValue::Bool(true));
        group.set_prop("ArrayName", PropValue::String("CUSTOMERS".into()));
        let mut name = Control::new("NAME", ControlType::TextBox, 10, 430);
        name.parent = Some("ROWS".into());
        let mut active = Control::new("ACTIVE", ControlType::CheckBox, 10, 460);
        active.parent = Some("ROWS".into());
        form.controls = vec![
            grid.clone(),
            chart.clone(),
            combo.clone(),
            list.clone(),
            group.clone(),
            name,
            active,
        ];

        let grid_target = form
            .binding_target_descriptor_for_control("GRID-1")
            .unwrap();
        assert!(default_mappings_for_target(&form, &grid_target, &fields)
            .iter()
            .all(|mapping| matches!(mapping.target, BindingTargetPath::GridColumn { .. })));

        let chart_target = form
            .binding_target_descriptor_for_control("CHART-1")
            .unwrap();
        let chart_mappings = default_mappings_for_target(&form, &chart_target, &fields);
        assert!(chart_mappings
            .iter()
            .any(|mapping| matches!(mapping.target, BindingTargetPath::ChartCategory { .. })));
        assert!(chart_mappings
            .iter()
            .any(|mapping| matches!(mapping.target, BindingTargetPath::ChartValueSeries { .. })));

        for id in ["COMBO-1", "LIST-1"] {
            let target = form.binding_target_descriptor_for_control(id).unwrap();
            let mappings = default_mappings_for_target(&form, &target, &fields);
            assert!(mappings.iter().any(|mapping| matches!(
                mapping.target,
                BindingTargetPath::ListDisplayItem { .. }
            )));
            assert!(mappings
                .iter()
                .any(|mapping| matches!(mapping.target, BindingTargetPath::ListValue { .. })));
        }

        let array_target = form.binding_target_descriptor_for_control("ROWS").unwrap();
        let array_mappings = default_mappings_for_target(&form, &array_target, &fields);
        assert!(array_mappings.iter().any(|mapping| matches!(
            &mapping.target,
            BindingTargetPath::ControlProperty { property_name, .. } if property_name == "Text"
        )));
        assert!(array_mappings.iter().any(|mapping| matches!(
            &mapping.target,
            BindingTargetPath::ControlProperty { property_name, .. } if property_name == "Checked"
        )));
    }

    #[test]
    fn data_binding_editor_creates_binding_for_every_source_type() {
        let mut form = Form::new("MAIN", "Main", 800, 600);
        let grid = Control::new("GRID-1", ControlType::DataGrid, 0, 0);
        form.controls.push(grid.clone());

        for kind in BindingEditorSourceKind::ALL {
            let binding = create_binding_from_editor(&form, &grid, kind).unwrap();
            assert!(binding.id.starts_with("BIND-"));
            assert_eq!(binding.target.primary_control_id(), "GRID-1");
            assert!(!binding.mappings.is_empty());
            form.data_bindings.push(binding);
        }

        let ids: std::collections::BTreeSet<&str> = form
            .data_bindings
            .iter()
            .map(|binding| binding.id.as_str())
            .collect();
        assert_eq!(ids.len(), BindingEditorSourceKind::ALL.len());
    }
}
