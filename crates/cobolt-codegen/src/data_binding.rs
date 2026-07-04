// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Deterministic COBOL generation for form-level data bindings.

use cobolt_forms::{
    BindingMode, BindingSourceDescriptor, BindingTargetDescriptor, BindingTargetPath,
    DataBindingDef, Form,
};

pub fn write_data_binding_storage(out: &mut String, form: &Form) {
    if form.data_bindings.is_empty() {
        return;
    }
    out.push_str("      *>-- Data binding runtime state --------------------------------\n");
    for binding in sorted_bindings(form) {
        let pfx = binding_prefix(&binding.id);
        out.push_str(&format!(
            "      *>   {}: {} -> {}\n",
            binding.id,
            source_label(&binding.source),
            target_label(&binding.target)
        ));
        out.push_str(&format!("       01 {pfx}-STATE.\n"));
        out.push_str(&format!("          05 {pfx}-DIRTY      PIC 9 VALUE 0.\n"));
        out.push_str(&format!(
            "          05 {pfx}-READ-ONLY  PIC 9 VALUE {}.\n",
            if binding.mode == BindingMode::ReadOnly {
                1
            } else {
                0
            }
        ));
        out.push_str(&format!(
            "          05 {pfx}-ROW-KEY    PIC X(256) VALUE SPACES.\n"
        ));
        out.push_str(&format!(
            "          05 {pfx}-STATUS     PIC X(256) VALUE SPACES.\n"
        ));
        out.push('\n');
    }
}

pub fn write_data_binding_bootstrap(out: &mut String, form: &Form) {
    if form.data_bindings.is_empty() {
        return;
    }
    out.push_str("           PERFORM COBOL-DATA-BINDINGS-LOAD\n");
    out.push_str("           PERFORM COBOL-DATA-BINDINGS-POPULATE\n");
    out.push_str("           PERFORM COBOL-DATA-BINDINGS-MARK-CLEAN\n");
}

pub fn write_data_binding_paragraphs(out: &mut String, form: &Form) {
    if form.data_bindings.is_empty() {
        return;
    }
    out.push_str("       COBOL-DATA-BINDINGS-LOAD.\n");
    for binding in sorted_bindings(form) {
        let pfx = binding_prefix(&binding.id);
        let read_only = if binding.mode == BindingMode::ReadOnly {
            "1"
        } else {
            "0"
        };
        out.push_str(&format!(
            "           CALL \"COBOL-BINDING-SET-READ-ONLY\" USING \"{}\" \"{}\"\n",
            binding.id, read_only
        ));
        out.push_str(&format!(
            "      *>    Load {} from {}.\n",
            binding.id,
            source_label(&binding.source)
        ));
        out.push_str(&format!(
            "           CALL \"COBOL-BINDING-LOAD\" USING \"{}\" {}-STATUS\n",
            binding.id, pfx
        ));
    }
    out.push_str("           CONTINUE.\n\n");

    out.push_str("       COBOL-DATA-BINDINGS-POPULATE.\n");
    for binding in sorted_bindings(form) {
        let pfx = binding_prefix(&binding.id);
        out.push_str(&format!(
            "      *>    Populate {} target {}.\n",
            binding.id,
            target_label(&binding.target)
        ));
        write_binding_refresh_seed(out, binding);
        out.push_str(&format!(
            "           CALL \"COBOL-BINDING-POPULATE\" USING \"{}\" {}-STATUS\n",
            binding.id, pfx
        ));
        for mapping in sorted_mappings(binding) {
            out.push_str(&format!(
                "      *>       {} -> {}\n",
                mapping.source_field,
                target_path_label(&mapping.target)
            ));
        }
    }
    out.push_str("           CONTINUE.\n\n");

    out.push_str("       COBOL-DATA-BINDINGS-MARK-CLEAN.\n");
    for binding in sorted_bindings(form) {
        let pfx = binding_prefix(&binding.id);
        out.push_str(&format!(
            "           CALL \"COBOL-BINDING-MARK-CLEAN\" USING \"{}\" {}-DIRTY\n",
            binding.id, pfx
        ));
    }
    out.push_str("           CONTINUE.\n\n");

    out.push_str("       COBOL-DATA-BINDINGS-UPDATE.\n");
    for binding in sorted_bindings(form)
        .into_iter()
        .filter(|binding| binding.mode == BindingMode::Writable)
    {
        let pfx = binding_prefix(&binding.id);
        out.push_str(&format!(
            "           CALL \"COBOL-BINDING-UPDATE\" USING \"{}\" {}-ROW-KEY {}-STATUS\n",
            binding.id, pfx, pfx
        ));
    }
    out.push_str("           CONTINUE.\n\n");
}

fn sorted_bindings(form: &Form) -> Vec<&DataBindingDef> {
    let mut bindings: Vec<&DataBindingDef> = form.data_bindings.iter().collect();
    bindings.sort_by_key(|binding| binding.id.to_ascii_uppercase());
    bindings
}

fn sorted_mappings(binding: &DataBindingDef) -> Vec<&cobolt_forms::FieldMapping> {
    binding.sorted_mapping_refs()
}

fn write_binding_refresh_seed(out: &mut String, binding: &DataBindingDef) {
    let (control_id, is_array) = match &binding.target {
        BindingTargetDescriptor::DataGrid { control_id } => (control_id.as_str(), false),
        BindingTargetDescriptor::ControlArray { array_id, .. } => (array_id.as_str(), true),
        _ => return,
    };
    let BindingSourceDescriptor::CobolTable { fields, .. } = &binding.source else {
        return;
    };
    if fields.is_empty() {
        return;
    }
    let fields_joined = fields
        .iter()
        .map(|field| field.name.as_str())
        .collect::<Vec<_>>()
        .join(",");
    out.push_str(&format!(
        "           INVOKE {control_id} 'SetProperty' USING BY CONTENT \"_BindingKind\" BY CONTENT \"CobolTable\"\n"
    ));
    out.push_str(&format!(
        "           INVOKE {control_id} 'SetProperty' USING BY CONTENT \"_BindingFields\" BY CONTENT \"{fields_joined}\"\n"
    ));
    if is_array {
        // For arrayed GroupBox, also seed the array name if different, for runtime lookup
        out.push_str(&format!(
            "           INVOKE {control_id} 'SetProperty' USING BY CONTENT \"_BindingArray\" BY CONTENT \"1\"\n"
        ));
        // Seed target mappings so REFRESHBINDING can hydrate per-instance member values
        // from the source table (format: sourceField\tmemberControlId\tpropName per line).
        let maps: Vec<String> = binding
            .mappings
            .iter()
            .filter_map(|m| {
                if let BindingTargetPath::ControlProperty {
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
            let joined = maps.join("\n");
            out.push_str(&format!(
                "           INVOKE {control_id} 'SetProperty' USING BY CONTENT \"_BindingMappings\" BY CONTENT \"{}\"\n",
                joined
            ));
        }
        // Trigger full bind (count + member hydration + effects) during initial POPULATE
        // so databound repeating GroupBox cards appear with row data on load, just like
        // DataGrids. User can also CALL/INVOKE RefreshBinding later to re-sync.
        out.push_str(&format!(
            "           INVOKE {control_id} 'RefreshBinding'\n"
        ));
    }
}

fn binding_prefix(id: &str) -> String {
    let normalized: String = id
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() {
                ch.to_ascii_uppercase()
            } else {
                '-'
            }
        })
        .collect();
    format!("WS-BIND-{}", normalized.trim_matches('-'))
}

fn source_label(source: &BindingSourceDescriptor) -> String {
    match source {
        BindingSourceDescriptor::IndexedFile { record_name, .. } => {
            format!("IndexedFile:{record_name}")
        }
        BindingSourceDescriptor::Sql {
            result_set_name, ..
        } => format!("SQL:{result_set_name}"),
        BindingSourceDescriptor::CobolTable { table_name, .. } => {
            format!("COBOLTable:{table_name}")
        }
        BindingSourceDescriptor::RestApi { endpoint_name, .. } => {
            format!("REST:{endpoint_name}")
        }
        BindingSourceDescriptor::AgentAi { output_name, .. } => {
            format!("AgentAI:{output_name}")
        }
    }
}

fn target_label(target: &BindingTargetDescriptor) -> String {
    match target {
        BindingTargetDescriptor::DataGrid { control_id } => format!("DataGrid:{control_id}"),
        BindingTargetDescriptor::Chart { control_id, .. } => format!("Chart:{control_id}"),
        BindingTargetDescriptor::ComboBox { control_id } => format!("ComboBox:{control_id}"),
        BindingTargetDescriptor::ListBox { control_id } => format!("ListBox:{control_id}"),
        BindingTargetDescriptor::ControlArray { array_id, .. } => {
            format!("ControlArray:{array_id}")
        }
    }
}

fn target_path_label(target: &BindingTargetPath) -> String {
    match target {
        BindingTargetPath::GridColumn {
            control_id,
            column_id,
        } => format!("{control_id}.{column_id}"),
        BindingTargetPath::ChartCategory { control_id } => format!("{control_id}.Category"),
        BindingTargetPath::ChartValueSeries {
            control_id,
            series_id,
        } => format!("{control_id}.{series_id}"),
        BindingTargetPath::ChartSeriesLabel {
            control_id,
            series_id,
        } => format!("{control_id}.{series_id}.Label"),
        BindingTargetPath::ListDisplayItem { control_id } => format!("{control_id}.Display"),
        BindingTargetPath::ListValue { control_id } => format!("{control_id}.Value"),
        BindingTargetPath::ControlProperty {
            control_id,
            property_name,
            ..
        } => format!("{control_id}.{property_name}"),
    }
}
