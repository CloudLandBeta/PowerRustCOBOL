// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Deterministic validation for form data-binding metadata.

use std::collections::{BTreeMap, BTreeSet};

use cobolt_forms::{
    BindingField, BindingMode, BindingSourceDescriptor, BindingSourceMetadata,
    BindingTargetDescriptor, BindingTargetPath, DataBindingDef, DataGridAdvanced, FieldMapping,
    Form, GuardianFinding, GuardianSeverity, MappingCompatibility,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BindingRepairAction {
    RemapField {
        old_field: String,
        new_field: String,
    },
    RemoveMapping {
        source_field: String,
    },
    MarkReadOnly,
    RefreshFromSavedMetadata,
    RefreshFromAvailableSource(BindingSourceMetadata),
    ReselectTarget(BindingTargetDescriptor),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BindingRepairError {
    BindingNotFound(String),
    FieldNotMapped(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BindingActionGate {
    SaveForm,
    RunForm,
    RunProject,
    DebugProject,
    CheckProject,
    BuildProject,
    PackageProject,
}

impl BindingActionGate {
    pub fn label(self) -> &'static str {
        match self {
            Self::SaveForm => "save",
            Self::RunForm => "run form",
            Self::RunProject => "run",
            Self::DebugProject => "debug",
            Self::CheckProject => "check",
            Self::BuildProject => "build",
            Self::PackageProject => "package",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BindingActionGateReport {
    pub action: BindingActionGate,
    pub findings: Vec<GuardianFinding>,
}

impl BindingActionGateReport {
    pub fn blocked(&self) -> bool {
        has_blockers(&self.findings)
    }

    pub fn blocker_count(&self) -> usize {
        self.findings
            .iter()
            .filter(|finding| finding.severity == GuardianSeverity::Blocker)
            .count()
    }

    pub fn warning_count(&self) -> usize {
        self.findings
            .iter()
            .filter(|finding| finding.severity == GuardianSeverity::Warning)
            .count()
    }
}

pub fn validate_form_bindings(form: &Form) -> Vec<GuardianFinding> {
    let mut findings = Vec::new();
    findings.extend(validate_control_collisions(form));
    for binding in &form.data_bindings {
        validate_binding(form, binding, &mut findings);
    }
    sort_findings(&mut findings);
    findings
}

pub fn has_blockers(findings: &[GuardianFinding]) -> bool {
    findings
        .iter()
        .any(|finding| finding.severity == GuardianSeverity::Blocker)
}

pub fn validate_binding_action(form: &Form, action: BindingActionGate) -> BindingActionGateReport {
    BindingActionGateReport {
        action,
        findings: validate_form_bindings(form),
    }
}

pub fn apply_repair(
    form: &mut Form,
    binding_id: &str,
    action: BindingRepairAction,
) -> Result<(), BindingRepairError> {
    let Some(binding) = form
        .data_bindings
        .iter_mut()
        .find(|binding| binding.id.eq_ignore_ascii_case(binding_id))
    else {
        return Err(BindingRepairError::BindingNotFound(binding_id.to_owned()));
    };

    match action {
        BindingRepairAction::RemapField {
            old_field,
            new_field,
        } => {
            let mut changed = false;
            for mapping in &mut binding.mappings {
                if mapping.source_field.eq_ignore_ascii_case(&old_field) {
                    mapping.source_field = new_field.clone();
                    changed = true;
                }
            }
            if changed {
                Ok(())
            } else {
                Err(BindingRepairError::FieldNotMapped(old_field))
            }
        }
        BindingRepairAction::RemoveMapping { source_field } => {
            let before = binding.mappings.len();
            binding
                .mappings
                .retain(|mapping| !mapping.source_field.eq_ignore_ascii_case(&source_field));
            if binding.mappings.len() == before {
                Err(BindingRepairError::FieldNotMapped(source_field))
            } else {
                Ok(())
            }
        }
        BindingRepairAction::MarkReadOnly => {
            binding.mode = BindingMode::ReadOnly;
            Ok(())
        }
        BindingRepairAction::RefreshFromSavedMetadata => {
            replace_source_fields(binding, binding.saved_source_metadata.fields.clone());
            Ok(())
        }
        BindingRepairAction::RefreshFromAvailableSource(metadata) => {
            replace_source_fields(binding, metadata.fields.clone());
            binding.saved_source_metadata = metadata;
            Ok(())
        }
        BindingRepairAction::ReselectTarget(target) => {
            binding.target = target;
            Ok(())
        }
    }
}

fn validate_binding(form: &Form, binding: &DataBindingDef, findings: &mut Vec<GuardianFinding>) {
    validate_source(binding, findings);
    validate_target(form, binding, findings);
    validate_source_field_collisions(binding, findings);
    validate_mappings(form, binding, findings);
    validate_writable_identity(binding, findings);
    validate_rest_agent_safety(binding, findings);
}

fn replace_source_fields(binding: &mut DataBindingDef, fields: Vec<BindingField>) {
    match &mut binding.source {
        BindingSourceDescriptor::IndexedFile {
            fields: source_fields,
            ..
        }
        | BindingSourceDescriptor::Sql {
            fields: source_fields,
            ..
        }
        | BindingSourceDescriptor::CobolTable {
            fields: source_fields,
            ..
        }
        | BindingSourceDescriptor::RestApi {
            fields: source_fields,
            ..
        }
        | BindingSourceDescriptor::AgentAi {
            fields: source_fields,
            ..
        } => {
            *source_fields = fields;
        }
    }
}

fn validate_source(binding: &DataBindingDef, findings: &mut Vec<GuardianFinding>) {
    match &binding.source {
        BindingSourceDescriptor::IndexedFile {
            definition_path,
            record_name,
            fields,
            ..
        } => {
            if definition_path.trim().is_empty() || record_name.trim().is_empty() {
                blocker(
                    findings,
                    binding,
                    "missing-source",
                    "Indexed file source is incomplete",
                );
            }
            validate_non_empty_fields(binding, fields, findings);
        }
        BindingSourceDescriptor::Sql {
            source_control_id,
            result_set_name,
            fields,
            ..
        } => {
            if source_control_id.trim().is_empty() || result_set_name.trim().is_empty() {
                blocker(
                    findings,
                    binding,
                    "missing-source",
                    "SQL source is incomplete",
                );
            }
            validate_non_empty_fields(binding, fields, findings);
        }
        BindingSourceDescriptor::CobolTable {
            table_name,
            occurs_item,
            fields,
            ..
        } => {
            if table_name.trim().is_empty() || occurs_item.trim().is_empty() {
                blocker(
                    findings,
                    binding,
                    "missing-source",
                    "COBOL table source is incomplete",
                );
            }
            validate_non_empty_fields(binding, fields, findings);
        }
        BindingSourceDescriptor::RestApi {
            source_control_id,
            response_data_item,
            fields,
            ..
        } => {
            if source_control_id.trim().is_empty() || response_data_item.trim().is_empty() {
                blocker(
                    findings,
                    binding,
                    "missing-source",
                    "REST source is incomplete",
                );
            }
            validate_non_empty_fields(binding, fields, findings);
        }
        BindingSourceDescriptor::AgentAi {
            source_control_id,
            output_name,
            fields,
            ..
        } => {
            if source_control_id.trim().is_empty() || output_name.trim().is_empty() {
                blocker(
                    findings,
                    binding,
                    "missing-source",
                    "Agent source is incomplete",
                );
            }
            validate_non_empty_fields(binding, fields, findings);
        }
    }
}

fn validate_non_empty_fields(
    binding: &DataBindingDef,
    fields: &[BindingField],
    findings: &mut Vec<GuardianFinding>,
) {
    if fields.is_empty() {
        blocker(
            findings,
            binding,
            "missing-fields",
            "Binding source has no fields",
        );
    }
}

fn validate_target(form: &Form, binding: &DataBindingDef, findings: &mut Vec<GuardianFinding>) {
    match &binding.target {
        BindingTargetDescriptor::DataGrid { control_id } => {
            let Some(control) = form.find_control(control_id) else {
                missing_target(findings, binding, control_id);
                return;
            };
            if !matches!(
                control.approved_binding_target_kind(),
                Some(cobolt_forms::ApprovedBindingTargetKind::DataGrid)
            ) {
                unsupported_target(findings, binding, control_id);
            }
        }
        BindingTargetDescriptor::Chart { control_id, .. } => {
            let Some(control) = form.find_control(control_id) else {
                missing_target(findings, binding, control_id);
                return;
            };
            if control.control_type.chart_binding_kind().is_none() {
                unsupported_target(findings, binding, control_id);
            }
        }
        BindingTargetDescriptor::ComboBox { control_id } => {
            let Some(control) = form.find_control(control_id) else {
                missing_target(findings, binding, control_id);
                return;
            };
            if !matches!(
                control.approved_binding_target_kind(),
                Some(cobolt_forms::ApprovedBindingTargetKind::ComboBox)
            ) {
                unsupported_target(findings, binding, control_id);
            }
        }
        BindingTargetDescriptor::ListBox { control_id } => {
            let Some(control) = form.find_control(control_id) else {
                missing_target(findings, binding, control_id);
                return;
            };
            if !matches!(
                control.approved_binding_target_kind(),
                Some(cobolt_forms::ApprovedBindingTargetKind::ListBox)
            ) {
                unsupported_target(findings, binding, control_id);
            }
        }
        BindingTargetDescriptor::ControlArray { array_id, .. } => {
            if resolve_control_array(form, array_id).is_none() {
                missing_target(findings, binding, array_id);
            }
        }
        BindingTargetDescriptor::ScalarControl { control_id } => {
            let Some(control) = form.find_control(control_id) else {
                missing_target(findings, binding, control_id);
                return;
            };
            if !matches!(
                control.approved_binding_target_kind(),
                Some(cobolt_forms::ApprovedBindingTargetKind::ScalarControl)
            ) {
                unsupported_target(findings, binding, control_id);
            }
        }
        BindingTargetDescriptor::MarkerCollection { control_id } => {
            let Some(control) = form.find_control(control_id) else {
                missing_target(findings, binding, control_id);
                return;
            };
            if !matches!(
                control.approved_binding_target_kind(),
                Some(cobolt_forms::ApprovedBindingTargetKind::MarkerCollection)
            ) {
                unsupported_target(findings, binding, control_id);
                return;
            }
            // R22: the mapped fields must cover lat/lng/label.
            let mapped: std::collections::HashSet<cobolt_forms::MapMarkerField> = binding
                .mappings
                .iter()
                .filter_map(|m| match &m.target {
                    BindingTargetPath::MarkerField { field, .. } => Some(*field),
                    _ => None,
                })
                .collect();
            let required = [
                cobolt_forms::MapMarkerField::Lat,
                cobolt_forms::MapMarkerField::Lng,
                cobolt_forms::MapMarkerField::Label,
            ];
            let missing: Vec<&str> = required
                .iter()
                .filter(|f| !mapped.contains(f))
                .map(|f| f.as_str())
                .collect();
            if !missing.is_empty() {
                blocker(
                    findings,
                    binding,
                    "missing-marker-fields",
                    format!(
                        "Maps binding is missing required marker field(s): {}",
                        missing.join(", ")
                    ),
                );
            }
        }
    }
}

fn validate_source_field_collisions(binding: &DataBindingDef, findings: &mut Vec<GuardianFinding>) {
    let mut seen = BTreeMap::<String, String>::new();
    for field in binding.source.fields() {
        let key = field.name.to_ascii_uppercase();
        if let Some(existing) = seen.insert(key, field.name.clone()) {
            let mut finding = GuardianFinding::new(
                GuardianSeverity::Blocker,
                "ambiguous-source-field",
                format!(
                    "Source fields '{existing}' and '{}' differ only by case",
                    field.name
                ),
                binding.id.clone(),
            );
            finding.source_field = Some(field.name.clone());
            findings.push(finding);
        }
    }
}

fn validate_control_collisions(form: &Form) -> Vec<GuardianFinding> {
    let mut findings = Vec::new();
    let mut seen = BTreeMap::<String, String>::new();
    for control in &form.controls {
        let key = control.id.to_ascii_uppercase();
        if let Some(existing) = seen.insert(key, control.id.clone()) {
            let mut finding = GuardianFinding::new(
                GuardianSeverity::Blocker,
                "ambiguous-target-control",
                format!(
                    "Controls '{existing}' and '{}' differ only by case",
                    control.id
                ),
                "",
            );
            finding.target_control_id = Some(control.id.clone());
            findings.push(finding);
        }
    }
    findings
}

fn validate_mappings(form: &Form, binding: &DataBindingDef, findings: &mut Vec<GuardianFinding>) {
    let source_fields: BTreeSet<String> = binding
        .source
        .fields()
        .iter()
        .map(|field| field.name.to_ascii_uppercase())
        .collect();

    for mapping in &binding.mappings {
        if !source_fields.contains(&mapping.source_field.to_ascii_uppercase()) {
            let mut finding = GuardianFinding::new(
                GuardianSeverity::Blocker,
                "missing-source-field",
                format!(
                    "Mapped source field '{}' does not exist",
                    mapping.source_field
                ),
                binding.id.clone(),
            );
            finding.source_field = Some(mapping.source_field.clone());
            findings.push(finding);
        }

        if let Some(target_control_id) = target_control_id(&mapping.target) {
            if form.find_control(target_control_id).is_none() {
                let mut finding = GuardianFinding::new(
                    GuardianSeverity::Blocker,
                    "missing-target-control",
                    format!("Mapped target control '{target_control_id}' does not exist"),
                    binding.id.clone(),
                );
                finding.target_control_id = Some(target_control_id.to_owned());
                findings.push(finding);
            }
        }

        validate_grid_column_identity(form, binding, mapping, findings);

        match mapping.compatibility {
            MappingCompatibility::Exact => {}
            MappingCompatibility::CoercibleWarning => {
                let mut finding = GuardianFinding::new(
                    GuardianSeverity::Warning,
                    "coercible-mapping",
                    format!(
                        "Mapped source field '{}' requires conversion",
                        mapping.source_field
                    ),
                    binding.id.clone(),
                );
                finding.source_field = Some(mapping.source_field.clone());
                findings.push(finding);
            }
            MappingCompatibility::Blocked => {
                let mut finding = GuardianFinding::new(
                    GuardianSeverity::Blocker,
                    "blocked-mapping",
                    format!(
                        "Mapped source field '{}' is incompatible",
                        mapping.source_field
                    ),
                    binding.id.clone(),
                );
                finding.source_field = Some(mapping.source_field.clone());
                findings.push(finding);
            }
        }

        if mapping_targets_scalar_from_multiple_field(binding, mapping) {
            let mut finding = GuardianFinding::new(
                GuardianSeverity::Blocker,
                "incompatible-cardinality",
                format!(
                    "Mapped source field '{}' is multi-value",
                    mapping.source_field
                ),
                binding.id.clone(),
            );
            finding.source_field = Some(mapping.source_field.clone());
            findings.push(finding);
        }
    }
}

fn validate_grid_column_identity(
    form: &Form,
    binding: &DataBindingDef,
    mapping: &FieldMapping,
    findings: &mut Vec<GuardianFinding>,
) {
    let BindingTargetPath::GridColumn {
        control_id,
        column_id,
    } = &mapping.target
    else {
        return;
    };
    let Some(control) = form.find_control(control_id) else {
        return;
    };
    let advanced = DataGridAdvanced::from_control(control);
    if advanced.columns.is_empty() {
        return;
    }
    let Some(column) = advanced.columns.iter().find(|column| {
        column.id.eq_ignore_ascii_case(column_id)
            || column.source_name.eq_ignore_ascii_case(column_id)
            || column.title.eq_ignore_ascii_case(column_id)
    }) else {
        return;
    };
    if column.source_name.trim().is_empty()
        || column
            .source_name
            .eq_ignore_ascii_case(&mapping.source_field)
    {
        return;
    }
    let mut finding = GuardianFinding::new(
        GuardianSeverity::Blocker,
        "datagrid-column-binding-drift",
        format!(
            "DataGrid column '{}' is bound to '{}' but its advanced metadata points to '{}'",
            column_id, mapping.source_field, column.source_name
        ),
        binding.id.clone(),
    );
    finding.source_field = Some(mapping.source_field.clone());
    finding.target_control_id = Some(control_id.clone());
    findings.push(finding);
}

fn validate_writable_identity(binding: &DataBindingDef, findings: &mut Vec<GuardianFinding>) {
    if binding.mode != BindingMode::Writable {
        return;
    }
    if source_identity_fields(&binding.source).is_empty() {
        blocker(
            findings,
            binding,
            "missing-row-identity",
            "Writable binding has no source key or row identity",
        );
    }
}

fn validate_rest_agent_safety(binding: &DataBindingDef, findings: &mut Vec<GuardianFinding>) {
    match &binding.source {
        BindingSourceDescriptor::RestApi { update, .. } => {
            if binding.mode == BindingMode::Writable
                && !update_metadata_is_complete(update.as_ref(), binding)
            {
                blocker(
                    findings,
                    binding,
                    "missing-rest-update-metadata",
                    "Writable REST binding needs update schema, row identity, and approved targets",
                );
            }
        }
        BindingSourceDescriptor::AgentAi { update, .. } => {
            if binding.mode == BindingMode::Writable {
                if !update_metadata_is_complete(update.as_ref(), binding) {
                    blocker(
                        findings,
                        binding,
                        "missing-agent-update-metadata",
                        "Writable Agent binding needs update schema, row identity, and approved targets",
                    );
                    return;
                }
                let approved: BTreeSet<String> = update
                    .as_ref()
                    .into_iter()
                    .flat_map(|metadata| metadata.approved_target_ids.iter())
                    .map(|id| id.to_ascii_uppercase())
                    .collect();
                for target in binding_target_ids(binding) {
                    if !approved.contains(&target.to_ascii_uppercase()) {
                        let mut finding = GuardianFinding::new(
                            GuardianSeverity::Blocker,
                            "agent-target-scope",
                            format!("Agent binding target '{target}' is outside the approved target list"),
                            binding.id.clone(),
                        );
                        finding.target_control_id = Some(target);
                        findings.push(finding);
                    }
                }
            }
        }
        _ => {}
    }
}

fn update_metadata_is_complete(
    update: Option<&cobolt_forms::BindingUpdateMetadata>,
    binding: &DataBindingDef,
) -> bool {
    let Some(update) = update else {
        return false;
    };
    !update.request_schema_name.trim().is_empty()
        && !update.key_fields.is_empty()
        && !update.approved_target_ids.is_empty()
        && binding.source.fields().iter().any(|field| {
            update
                .key_fields
                .iter()
                .any(|key| key.eq_ignore_ascii_case(&field.name))
        })
}

fn source_identity_fields(source: &BindingSourceDescriptor) -> Vec<String> {
    match source {
        BindingSourceDescriptor::IndexedFile {
            key_field, fields, ..
        } => key_field
            .clone()
            .into_iter()
            .chain(
                fields
                    .iter()
                    .filter(|field| field.key)
                    .map(|field| field.name.clone()),
            )
            .collect(),
        BindingSourceDescriptor::Sql {
            key_fields, fields, ..
        }
        | BindingSourceDescriptor::CobolTable {
            key_fields, fields, ..
        } => key_fields
            .iter()
            .cloned()
            .chain(
                fields
                    .iter()
                    .filter(|field| field.key)
                    .map(|field| field.name.clone()),
            )
            .collect(),
        BindingSourceDescriptor::RestApi { update, .. }
        | BindingSourceDescriptor::AgentAi { update, .. } => update
            .as_ref()
            .map(|metadata| metadata.key_fields.clone())
            .unwrap_or_default(),
    }
}

fn resolve_control_array<'a>(form: &'a Form, array_id: &str) -> Option<&'a cobolt_forms::Control> {
    form.controls.iter().find(|control| {
        control
            .explicit_control_array_id()
            .map(|id| id.eq_ignore_ascii_case(array_id))
            .unwrap_or(false)
            || control.id.eq_ignore_ascii_case(array_id)
                && control.explicit_control_array_id().is_some()
    })
}

fn target_control_id(target: &BindingTargetPath) -> Option<&str> {
    match target {
        BindingTargetPath::GridColumn { control_id, .. }
        | BindingTargetPath::ChartCategory { control_id }
        | BindingTargetPath::ChartValueSeries { control_id, .. }
        | BindingTargetPath::ChartSeriesLabel { control_id, .. }
        | BindingTargetPath::ListDisplayItem { control_id }
        | BindingTargetPath::ListValue { control_id }
        | BindingTargetPath::ScalarValue { control_id }
        | BindingTargetPath::MarkerField { control_id, .. }
        | BindingTargetPath::ControlProperty { control_id, .. } => Some(control_id),
    }
}

fn binding_target_ids(binding: &DataBindingDef) -> Vec<String> {
    let mut ids = Vec::new();
    ids.push(binding.target.primary_control_id().to_owned());
    for mapping in &binding.mappings {
        if let Some(id) = target_control_id(&mapping.target) {
            ids.push(id.to_owned());
        }
    }
    ids.sort_by_key(|id| id.to_ascii_uppercase());
    ids.dedup_by(|a, b| a.eq_ignore_ascii_case(b));
    ids
}

fn mapping_targets_scalar_from_multiple_field(
    binding: &DataBindingDef,
    mapping: &FieldMapping,
) -> bool {
    let maps_to_scalar = matches!(
        mapping.target,
        BindingTargetPath::ControlProperty { .. }
            | BindingTargetPath::ListValue { .. }
            | BindingTargetPath::ChartCategory { .. }
            | BindingTargetPath::ChartSeriesLabel { .. }
    );
    if !maps_to_scalar {
        return false;
    }
    binding
        .source
        .fields()
        .iter()
        .any(|field| field.multiple && field.name.eq_ignore_ascii_case(&mapping.source_field))
}

fn blocker(
    findings: &mut Vec<GuardianFinding>,
    binding: &DataBindingDef,
    code: impl Into<String>,
    message: impl Into<String>,
) {
    findings.push(GuardianFinding::new(
        GuardianSeverity::Blocker,
        code,
        message,
        binding.id.clone(),
    ));
}

fn missing_target(findings: &mut Vec<GuardianFinding>, binding: &DataBindingDef, control_id: &str) {
    let mut finding = GuardianFinding::new(
        GuardianSeverity::Blocker,
        "missing-target-control",
        format!("Binding target control '{control_id}' does not exist"),
        binding.id.clone(),
    );
    finding.target_control_id = Some(control_id.to_owned());
    findings.push(finding);
}

fn unsupported_target(
    findings: &mut Vec<GuardianFinding>,
    binding: &DataBindingDef,
    control_id: &str,
) {
    let mut finding = GuardianFinding::new(
        GuardianSeverity::Blocker,
        "unsupported-target-control",
        format!("Binding target control '{control_id}' is not approved for data binding"),
        binding.id.clone(),
    );
    finding.target_control_id = Some(control_id.to_owned());
    findings.push(finding);
}

fn sort_findings(findings: &mut Vec<GuardianFinding>) {
    findings.sort_by_key(|finding| {
        (
            severity_rank(&finding.severity),
            finding.binding_id.clone(),
            finding.code.clone(),
            finding.source_field.clone().unwrap_or_default(),
            finding.target_control_id.clone().unwrap_or_default(),
            finding.message.clone(),
        )
    });
}

fn severity_rank(severity: &GuardianSeverity) -> u8 {
    match severity {
        GuardianSeverity::Blocker => 0,
        GuardianSeverity::Warning => 1,
        GuardianSeverity::Info => 2,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cobolt_forms::{
        BindingDataType, BindingField, BindingSourceDescriptor, BindingSourceMetadata,
        BindingTargetDescriptor, BindingTargetPath, BindingUpdateMetadata, Control, ControlType,
        DataGridColumn, EventBinding, FieldMapping, MapMarkerField, PropValue, Rect,
        DATAGRID_ADVANCED_PROP,
    };

    type ControlSnapshot = Vec<(
        String,
        ControlType,
        Rect,
        Vec<(String, String)>,
        Vec<EventBinding>,
        Option<String>,
    )>;

    fn fields() -> Vec<BindingField> {
        vec![
            BindingField::new("CUSTOMER-ID", BindingDataType::Integer).key(),
            BindingField::new("CUSTOMER-NAME", BindingDataType::Text).required(),
        ]
    }

    fn valid_grid_binding() -> DataBindingDef {
        DataBindingDef::new(
            "BIND-CUSTOMERS",
            "Customers",
            BindingSourceDescriptor::IndexedFile {
                definition_path: "data/customers.cidx".into(),
                record_name: "CUSTOMER-REC".into(),
                fields: fields(),
                key_field: Some("CUSTOMER-ID".into()),
                writable: true,
            },
            BindingTargetDescriptor::DataGrid {
                control_id: "GRID-1".into(),
            },
        )
        .with_mappings(vec![
            FieldMapping::new(
                "CUSTOMER-ID",
                BindingTargetPath::GridColumn {
                    control_id: "GRID-1".into(),
                    column_id: "ID".into(),
                },
            ),
            FieldMapping::new(
                "CUSTOMER-NAME",
                BindingTargetPath::GridColumn {
                    control_id: "GRID-1".into(),
                    column_id: "NAME".into(),
                },
            ),
        ])
    }

    fn form_with_grid(binding: DataBindingDef) -> Form {
        let mut form = Form::new("MAIN", "Main", 800, 600);
        form.add_control(Control::new("GRID-1", ControlType::DataGrid, 0, 0));
        form.data_bindings.push(binding);
        form
    }

    fn control_snapshot(form: &Form) -> ControlSnapshot {
        form.controls
            .iter()
            .map(|control| {
                (
                    control.id.clone(),
                    control.control_type.clone(),
                    control.rect,
                    control
                        .properties
                        .iter()
                        .map(|(key, value)| (key.clone(), value.to_xml_string()))
                        .collect(),
                    control.events.clone(),
                    control.parent.clone(),
                )
            })
            .collect()
    }

    #[test]
    fn data_binding_guardian_core_accepts_valid_grid_binding() {
        let findings = validate_form_bindings(&form_with_grid(valid_grid_binding()));
        assert!(findings.is_empty(), "{findings:?}");
    }

    #[test]
    fn data_binding_guardian_core_blocks_missing_identity_for_writable() {
        let mut binding = valid_grid_binding();
        binding.mode = BindingMode::Writable;
        if let BindingSourceDescriptor::IndexedFile {
            key_field, fields, ..
        } = &mut binding.source
        {
            *key_field = None;
            for field in fields {
                field.key = false;
            }
        }
        let findings = validate_form_bindings(&form_with_grid(binding));
        assert!(has_blockers(&findings));
        assert!(findings.iter().any(|f| f.code == "missing-row-identity"));
    }

    #[test]
    fn data_binding_guardian_core_blocks_deleted_target_and_field() {
        let mut form = Form::new("MAIN", "Main", 800, 600);
        let mut binding = valid_grid_binding();
        binding.mappings.push(FieldMapping::new(
            "DELETED-FIELD",
            BindingTargetPath::GridColumn {
                control_id: "DELETED-GRID".into(),
                column_id: "OLD".into(),
            },
        ));
        form.data_bindings.push(binding);

        let findings = validate_form_bindings(&form);
        assert!(findings.iter().any(|f| f.code == "missing-target-control"));
        assert!(findings.iter().any(|f| f.code == "missing-source-field"));
    }

    #[test]
    fn data_binding_guardian_core_warns_on_coercible_mapping() {
        let mut binding = valid_grid_binding();
        binding.mappings[0].compatibility = MappingCompatibility::CoercibleWarning;
        let findings = validate_form_bindings(&form_with_grid(binding));
        assert!(!has_blockers(&findings));
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].severity, GuardianSeverity::Warning);
        assert_eq!(findings[0].code, "coercible-mapping");
    }

    #[test]
    fn data_binding_guardian_core_blocks_case_collisions_deterministically() {
        let mut form = form_with_grid(valid_grid_binding());
        form.add_control(Control::new("grid-1", ControlType::DataGrid, 10, 10));

        let findings = validate_form_bindings(&form);
        assert!(findings
            .iter()
            .any(|finding| finding.code == "ambiguous-target-control"));
        let codes: Vec<&str> = findings
            .iter()
            .map(|finding| finding.code.as_str())
            .collect();
        let mut sorted = codes.clone();
        sorted.sort_unstable();
        assert_eq!(codes, sorted);
    }

    #[test]
    fn data_binding_guardian_core_blocks_unsupported_scalar_target() {
        let mut form = Form::new("MAIN", "Main", 800, 600);
        form.add_control(Control::new("TEXT-1", ControlType::TextBox, 0, 0));
        let mut binding = valid_grid_binding();
        binding.target = BindingTargetDescriptor::DataGrid {
            control_id: "TEXT-1".into(),
        };
        form.data_bindings.push(binding);

        let findings = validate_form_bindings(&form);
        assert!(findings
            .iter()
            .any(|finding| finding.code == "unsupported-target-control"));
    }

    #[test]
    fn data_binding_guardian_core_blocks_multi_value_to_scalar_mapping() {
        let mut form = Form::new("MAIN", "Main", 800, 600);
        form.add_control(Control::new("LIST-1", ControlType::ListBox, 0, 0));
        let mut fields = fields();
        fields[1].multiple = true;
        let binding = DataBindingDef::new(
            "BIND-CUSTOMERS",
            "Customers",
            BindingSourceDescriptor::RestApi {
                source_control_id: "REST-1".into(),
                endpoint_name: "CUSTOMERS".into(),
                response_data_item: "REST-RESPONSE".into(),
                fields,
                update: None,
            },
            BindingTargetDescriptor::ListBox {
                control_id: "LIST-1".into(),
            },
        )
        .with_mappings(vec![FieldMapping::new(
            "CUSTOMER-NAME",
            BindingTargetPath::ListValue {
                control_id: "LIST-1".into(),
            },
        )]);
        form.data_bindings.push(binding);

        let findings = validate_form_bindings(&form);
        assert!(findings
            .iter()
            .any(|finding| finding.code == "incompatible-cardinality"));
    }

    #[test]
    fn data_binding_guardian_core_accepts_explicit_control_array_target() {
        let mut form = Form::new("MAIN", "Main", 800, 600);
        let mut group = Control::new("ROWS", ControlType::GroupBox, 0, 0);
        group.set_prop("IsRepeatingGroup", PropValue::Bool(true));
        group.set_prop("ArrayName", PropValue::String("CUSTOMERS".into()));
        let mut name = Control::new("NAME", ControlType::TextBox, 10, 10);
        name.parent = Some("ROWS".into());
        form.add_control(group);
        form.add_control(name);

        let mut binding = valid_grid_binding();
        binding.target = BindingTargetDescriptor::ControlArray {
            array_id: "CUSTOMERS".into(),
            member_control_ids: vec!["NAME".into()],
        };
        binding.mappings = vec![FieldMapping::new(
            "CUSTOMER-NAME",
            BindingTargetPath::ControlProperty {
                array_id: "CUSTOMERS".into(),
                control_id: "NAME".into(),
                property_name: "Text".into(),
            },
        )];
        form.data_bindings.push(binding);

        let findings = validate_form_bindings(&form);
        assert!(findings.is_empty(), "{findings:?}");
    }

    // ── Spec 039 T6: ScalarControl (Knob/Gauge/Switch) binding target ──────

    fn scalar_binding(control_id: &str, source_field_type: BindingDataType) -> DataBindingDef {
        DataBindingDef::new(
            "BIND-SCALAR",
            "Scalar",
            BindingSourceDescriptor::IndexedFile {
                definition_path: "data/readings.cidx".into(),
                record_name: "READING-REC".into(),
                fields: vec![BindingField::new("READING-VALUE", source_field_type).required()],
                key_field: None,
                writable: false,
            },
            BindingTargetDescriptor::ScalarControl {
                control_id: control_id.into(),
            },
        )
        .with_mappings(vec![FieldMapping::new(
            "READING-VALUE",
            BindingTargetPath::ScalarValue {
                control_id: control_id.into(),
            },
        )])
    }

    #[test]
    fn data_binding_guardian_accepts_a_standalone_knob_gauge_switch_target() {
        for ct in [ControlType::Knob, ControlType::Gauge, ControlType::Switch] {
            let mut form = Form::new("MAIN", "Main", 800, 600);
            form.add_control(Control::new("SCALAR-1", ct.clone(), 0, 0));
            form.data_bindings
                .push(scalar_binding("SCALAR-1", BindingDataType::Integer));
            let findings = validate_form_bindings(&form);
            assert!(findings.is_empty(), "{ct:?}: {findings:?}");
        }
    }

    #[test]
    fn data_binding_guardian_rejects_a_file_drop_zone_as_a_binding_target() {
        // R25: FileDropZone is deliberately NOT an approved target — its
        // DroppedFiles is event-shaped output, not a displayed value a
        // source drives. `approved_binding_target_kind()` returns `None`
        // for it, so a `ScalarControl` descriptor pointed at one must be
        // rejected as unsupported, the same as any other non-approved type.
        let mut form = Form::new("MAIN", "Main", 800, 600);
        form.add_control(Control::new(
            "FDZ-1",
            ControlType::FileDropZone,
            0,
            0,
        ));
        form.data_bindings
            .push(scalar_binding("FDZ-1", BindingDataType::Text));
        let findings = validate_form_bindings(&form);
        assert!(
            !findings.is_empty(),
            "FileDropZone must never be accepted as a binding target"
        );
    }

    #[test]
    fn data_binding_guardian_rejects_a_missing_scalar_control_target() {
        let form = Form::new("MAIN", "Main", 800, 600);
        // No SCALAR-1 control added — the binding points at nothing.
        let mut form = form;
        form.data_bindings
            .push(scalar_binding("SCALAR-1", BindingDataType::Integer));
        let findings = validate_form_bindings(&form);
        assert!(
            !findings.is_empty(),
            "a ScalarControl binding to a nonexistent control must be rejected"
        );
    }

    // ── Spec 039 T13: MarkerCollection (Maps) binding target ───────────────

    fn marker_binding(control_id: &str, fields: &[(&str, MapMarkerField)]) -> DataBindingDef {
        let source_fields: Vec<BindingField> = fields
            .iter()
            .map(|(name, _)| BindingField::new(*name, BindingDataType::Text).required())
            .collect();
        let mappings: Vec<FieldMapping> = fields
            .iter()
            .map(|(name, marker_field)| {
                FieldMapping::new(
                    *name,
                    BindingTargetPath::MarkerField {
                        control_id: control_id.into(),
                        field: *marker_field,
                    },
                )
            })
            .collect();
        DataBindingDef::new(
            "BIND-MARKERS",
            "Markers",
            BindingSourceDescriptor::IndexedFile {
                definition_path: "data/places.cidx".into(),
                record_name: "PLACE-REC".into(),
                fields: source_fields,
                key_field: None,
                writable: false,
            },
            BindingTargetDescriptor::MarkerCollection {
                control_id: control_id.into(),
            },
        )
        .with_mappings(mappings)
    }

    #[test]
    fn data_binding_guardian_accepts_a_maps_control_bound_to_lat_lng_label() {
        let mut form = Form::new("MAIN", "Main", 800, 600);
        form.add_control(Control::new("MAP-1", ControlType::Maps, 0, 0));
        form.data_bindings.push(marker_binding(
            "MAP-1",
            &[
                ("PLACE-LAT", MapMarkerField::Lat),
                ("PLACE-LNG", MapMarkerField::Lng),
                ("PLACE-NAME", MapMarkerField::Label),
            ],
        ));
        let findings = validate_form_bindings(&form);
        assert!(findings.is_empty(), "{findings:?}");
    }

    #[test]
    fn data_binding_guardian_rejects_a_maps_binding_missing_a_required_marker_field() {
        // R22: lat/lng/label are all required — mapping only lat/lng (no
        // label) must be rejected, not silently accepted with a blank label.
        let mut form = Form::new("MAIN", "Main", 800, 600);
        form.add_control(Control::new("MAP-1", ControlType::Maps, 0, 0));
        form.data_bindings.push(marker_binding(
            "MAP-1",
            &[
                ("PLACE-LAT", MapMarkerField::Lat),
                ("PLACE-LNG", MapMarkerField::Lng),
            ],
        ));
        let findings = validate_form_bindings(&form);
        assert!(
            findings.iter().any(|f| f.code == "missing-marker-fields"),
            "{findings:?}"
        );
    }

    #[test]
    fn data_binding_guardian_rejects_a_text_box_as_a_marker_collection_target() {
        let mut form = Form::new("MAIN", "Main", 800, 600);
        form.add_control(Control::new("TXT-1", ControlType::TextBox, 0, 0));
        form.data_bindings.push(marker_binding(
            "TXT-1",
            &[
                ("PLACE-LAT", MapMarkerField::Lat),
                ("PLACE-LNG", MapMarkerField::Lng),
                ("PLACE-NAME", MapMarkerField::Label),
            ],
        ));
        let findings = validate_form_bindings(&form);
        assert!(
            !findings.is_empty(),
            "a non-Maps control must never be accepted as a MarkerCollection target"
        );
    }

    #[test]
    fn data_binding_guardian_rest_agent_accepts_rest_read_only_offline() {
        let mut form = Form::new("MAIN", "Main", 800, 600);
        form.add_control(Control::new("GRID-1", ControlType::DataGrid, 0, 0));
        let binding = DataBindingDef::new(
            "REST-CUSTOMERS",
            "Customers",
            BindingSourceDescriptor::RestApi {
                source_control_id: "REST-1".into(),
                endpoint_name: "GET-CUSTOMERS".into(),
                response_data_item: "REST-RESPONSE".into(),
                fields: fields(),
                update: None,
            },
            BindingTargetDescriptor::DataGrid {
                control_id: "GRID-1".into(),
            },
        )
        .with_mappings(vec![FieldMapping::new(
            "CUSTOMER-NAME",
            BindingTargetPath::GridColumn {
                control_id: "GRID-1".into(),
                column_id: "NAME".into(),
            },
        )]);
        assert_eq!(binding.mode, BindingMode::ReadOnly);
        form.data_bindings.push(binding);

        let findings = validate_form_bindings(&form);
        assert!(findings.is_empty(), "{findings:?}");
    }

    // ── Spec 039 T16: WebSearch classified under the existing RestApi
    // BindingSourceKind — no new enum variant (plan.md Decision 2:
    // `BindingSourceDescriptor::RestApi` "already models 'a REST API
    // response,' with no assumption that it came specifically from a
    // RestClient control"). This is deliberately the SAME test as
    // `data_binding_guardian_rest_agent_accepts_rest_read_only_offline`
    // above, just pointed at a real `WebSearch` control instead of a bare
    // placeholder id — proving the existing generic `RestApi` path already
    // accepts it with zero code changes, not a parallel new code path.

    #[test]
    fn data_binding_guardian_accepts_a_web_search_control_as_a_rest_api_source() {
        let mut form = Form::new("MAIN", "Main", 800, 600);
        form.add_control(Control::new("GRID-1", ControlType::DataGrid, 0, 0));
        form.add_control(Control::new("SEARCH-1", ControlType::WebSearch, 0, 200));
        let binding = DataBindingDef::new(
            "REST-SEARCH-RESULTS",
            "Search Results",
            BindingSourceDescriptor::RestApi {
                source_control_id: "SEARCH-1".into(),
                endpoint_name: "SEARCH".into(),
                response_data_item: "SEARCH-RESPONSE".into(),
                fields: fields(),
                update: None,
            },
            BindingTargetDescriptor::DataGrid {
                control_id: "GRID-1".into(),
            },
        )
        .with_mappings(vec![FieldMapping::new(
            "CUSTOMER-NAME",
            BindingTargetPath::GridColumn {
                control_id: "GRID-1".into(),
                column_id: "NAME".into(),
            },
        )]);
        assert_eq!(
            binding.source.kind(),
            cobolt_forms::BindingSourceKind::RestApi,
            "a WebSearch-sourced binding must classify as RestApi, not a new kind"
        );
        form.data_bindings.push(binding);

        let findings = validate_form_bindings(&form);
        assert!(findings.is_empty(), "{findings:?}");
    }

    #[test]
    fn datagrid_binding_metadata_blocks_column_source_drift() {
        let mut form = Form::new("MAIN", "Main", 800, 600);
        let mut grid = Control::new("GRID-1", ControlType::DataGrid, 0, 0);
        let mut advanced = DataGridAdvanced::default();
        advanced.columns.push(DataGridColumn {
            id: "NAME".into(),
            title: "Name".into(),
            source_name: "CUSTOMER-ID".into(),
            ..DataGridColumn::default()
        });
        grid.set_prop(
            DATAGRID_ADVANCED_PROP,
            PropValue::String(
                advanced
                    .to_json()
                    .expect("advanced metadata should serialize"),
            ),
        );
        form.add_control(grid);
        form.data_bindings.push(
            DataBindingDef::new(
                "GRID-BIND",
                "Customers",
                BindingSourceDescriptor::CobolTable {
                    table_name: "WS-CUSTOMER-TABLE".into(),
                    occurs_item: "WS-CUSTOMER-ROW".into(),
                    fields: fields(),
                    key_fields: vec!["CUSTOMER-ID".into()],
                    writable: true,
                },
                BindingTargetDescriptor::DataGrid {
                    control_id: "GRID-1".into(),
                },
            )
            .with_mappings(vec![FieldMapping::new(
                "CUSTOMER-NAME",
                BindingTargetPath::GridColumn {
                    control_id: "GRID-1".into(),
                    column_id: "NAME".into(),
                },
            )]),
        );

        let findings = validate_form_bindings(&form);

        assert!(findings
            .iter()
            .any(|finding| finding.code == "datagrid-column-binding-drift"));
    }

    #[test]
    fn data_binding_guardian_rest_agent_blocks_writable_rest_without_update_metadata() {
        let mut form = Form::new("MAIN", "Main", 800, 600);
        form.add_control(Control::new("GRID-1", ControlType::DataGrid, 0, 0));
        let mut binding = DataBindingDef::new(
            "REST-CUSTOMERS",
            "Customers",
            BindingSourceDescriptor::RestApi {
                source_control_id: "REST-1".into(),
                endpoint_name: "GET-CUSTOMERS".into(),
                response_data_item: "REST-RESPONSE".into(),
                fields: fields(),
                update: None,
            },
            BindingTargetDescriptor::DataGrid {
                control_id: "GRID-1".into(),
            },
        );
        binding.mode = BindingMode::Writable;
        form.data_bindings.push(binding);

        let findings = validate_form_bindings(&form);
        assert!(findings
            .iter()
            .any(|finding| finding.code == "missing-rest-update-metadata"));
    }

    #[test]
    fn data_binding_guardian_rest_agent_blocks_agent_targets_outside_scope() {
        let mut form = Form::new("MAIN", "Main", 800, 600);
        form.add_control(Control::new("GRID-1", ControlType::DataGrid, 0, 0));
        form.add_control(Control::new("TEXT-1", ControlType::TextBox, 0, 220));
        let mut update = BindingUpdateMetadata::new("AGENT-UPDATE", vec!["CUSTOMER-ID".into()]);
        update.approved_target_ids = vec!["GRID-1".into()];
        let mut binding = DataBindingDef::new(
            "AGENT-CUSTOMERS",
            "Customers",
            BindingSourceDescriptor::AgentAi {
                source_control_id: "AGENT-1".into(),
                output_name: "CUSTOMERS".into(),
                fields: fields(),
                update: Some(update),
            },
            BindingTargetDescriptor::DataGrid {
                control_id: "GRID-1".into(),
            },
        )
        .with_mappings(vec![FieldMapping::new(
            "CUSTOMER-NAME",
            BindingTargetPath::ControlProperty {
                array_id: "CUSTOMERS".into(),
                control_id: "TEXT-1".into(),
                property_name: "Text".into(),
            },
        )]);
        binding.mode = BindingMode::Writable;
        form.data_bindings.push(binding);

        let findings = validate_form_bindings(&form);
        assert!(findings
            .iter()
            .any(|finding| finding.code == "agent-target-scope"));
    }

    #[test]
    fn data_binding_repair_applies_actions_without_touching_controls() {
        let mut binding = valid_grid_binding();
        binding.mode = BindingMode::Writable;
        let mut form = form_with_grid(binding);
        form.add_control(Control::new("LIST-1", ControlType::ListBox, 0, 220));
        form.find_control_mut("GRID-1")
            .unwrap()
            .events
            .push(EventBinding {
                event: "onRowSelect".into(),
                paragraph: "GRID-1--ONROWSELECT".into(),
                code: "       PROCEDURE DIVISION.\n           CONTINUE.\n".into(),
            });
        let before = control_snapshot(&form);

        apply_repair(
            &mut form,
            "BIND-CUSTOMERS",
            BindingRepairAction::RemapField {
                old_field: "CUSTOMER-NAME".into(),
                new_field: "CUSTOMER-FULL-NAME".into(),
            },
        )
        .expect("remap");
        assert!(form.data_bindings[0]
            .mappings
            .iter()
            .any(|mapping| mapping.source_field == "CUSTOMER-FULL-NAME"));

        apply_repair(
            &mut form,
            "BIND-CUSTOMERS",
            BindingRepairAction::RemoveMapping {
                source_field: "CUSTOMER-ID".into(),
            },
        )
        .expect("remove mapping");
        assert_eq!(form.data_bindings[0].mappings.len(), 1);

        apply_repair(
            &mut form,
            "BIND-CUSTOMERS",
            BindingRepairAction::MarkReadOnly,
        )
        .expect("mark read-only");
        assert_eq!(form.data_bindings[0].mode, BindingMode::ReadOnly);

        replace_source_fields(&mut form.data_bindings[0], Vec::new());
        apply_repair(
            &mut form,
            "BIND-CUSTOMERS",
            BindingRepairAction::RefreshFromSavedMetadata,
        )
        .expect("refresh saved");
        assert_eq!(form.data_bindings[0].source.fields().len(), 2);

        let metadata = BindingSourceMetadata {
            fields: vec![BindingField::new("ACTIVE", BindingDataType::Boolean)],
            schema_text: "schema-v2".into(),
            sample_payload: "{\"ACTIVE\":true}".into(),
        };
        apply_repair(
            &mut form,
            "BIND-CUSTOMERS",
            BindingRepairAction::RefreshFromAvailableSource(metadata.clone()),
        )
        .expect("refresh available");
        assert_eq!(form.data_bindings[0].source.fields()[0].name, "ACTIVE");
        assert_eq!(form.data_bindings[0].saved_source_metadata, metadata);

        apply_repair(
            &mut form,
            "BIND-CUSTOMERS",
            BindingRepairAction::ReselectTarget(BindingTargetDescriptor::ListBox {
                control_id: "LIST-1".into(),
            }),
        )
        .expect("reselect target");
        assert!(matches!(
            form.data_bindings[0].target,
            BindingTargetDescriptor::ListBox { .. }
        ));

        assert_eq!(control_snapshot(&form), before);
    }

    #[test]
    fn data_binding_repair_reports_missing_binding_or_mapping() {
        let mut form = form_with_grid(valid_grid_binding());
        assert_eq!(
            apply_repair(&mut form, "NOPE", BindingRepairAction::MarkReadOnly),
            Err(BindingRepairError::BindingNotFound("NOPE".into()))
        );
        assert_eq!(
            apply_repair(
                &mut form,
                "BIND-CUSTOMERS",
                BindingRepairAction::RemoveMapping {
                    source_field: "NO-FIELD".into(),
                },
            ),
            Err(BindingRepairError::FieldNotMapped("NO-FIELD".into()))
        );
    }

    #[test]
    fn data_binding_action_gates_block_all_actions_on_blockers() {
        let mut form = form_with_grid(valid_grid_binding());
        form.data_bindings[0].mappings.push(FieldMapping::new(
            "DELETED-FIELD",
            BindingTargetPath::GridColumn {
                control_id: "GRID-1".into(),
                column_id: "OLD".into(),
            },
        ));

        for action in [
            BindingActionGate::SaveForm,
            BindingActionGate::RunForm,
            BindingActionGate::RunProject,
            BindingActionGate::DebugProject,
            BindingActionGate::CheckProject,
            BindingActionGate::BuildProject,
            BindingActionGate::PackageProject,
        ] {
            let report = validate_binding_action(&form, action);
            assert!(report.blocked(), "{action:?} should block");
            assert!(report.blocker_count() > 0);
        }
    }

    #[test]
    fn data_binding_action_gates_allow_warnings_without_blocking() {
        let mut binding = valid_grid_binding();
        binding.mappings[0].compatibility = MappingCompatibility::CoercibleWarning;
        let form = form_with_grid(binding);

        let report = validate_binding_action(&form, BindingActionGate::BuildProject);
        assert!(!report.blocked());
        assert_eq!(report.blocker_count(), 0);
        assert_eq!(report.warning_count(), 1);
    }

    #[test]
    fn data_binding_action_gates_allow_forms_without_bindings() {
        let mut form = Form::new("MAIN", "Main", 800, 600);
        form.add_control(Control::new("TEXT-1", ControlType::TextBox, 0, 0));

        let report = validate_binding_action(&form, BindingActionGate::SaveForm);
        assert!(!report.blocked());
        assert!(report.findings.is_empty());
    }
}
