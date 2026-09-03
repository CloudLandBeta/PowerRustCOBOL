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
        write_binding_refresh_seed(out, form, binding);
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

fn write_binding_refresh_seed(out: &mut String, form: &Form, binding: &DataBindingDef) {
    match &binding.target {
        BindingTargetDescriptor::ScalarControl { control_id } => {
            write_scalar_binding_seed(out, form, control_id, binding);
            return;
        }
        BindingTargetDescriptor::MarkerCollection { control_id } => {
            write_marker_binding_seed(out, control_id, binding);
            return;
        }
        BindingTargetDescriptor::DataGrid { control_id }
            if matches!(&binding.source, BindingSourceDescriptor::IndexedFile { .. }) =>
        {
            write_indexed_file_binding_seed(out, control_id, binding);
            return;
        }
        _ => {}
    }
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

/// `IndexedFile` source -> `DataGrid` target: seed `_BindingKind`/`_BindingFields`
/// (same shape as the `CobolTable` seed above) plus `_BindingIndexedPath` (the
/// `.cidx` definition path exactly as the Designer saved it — resolved at
/// runtime the same way a plain `SELECT ... ASSIGN TO` resolves its target:
/// against the process's own working directory, since the interpreter has no
/// project-root concept of its own).
///
/// Unlike the `CobolTable` `DataGrid` seed, this also invokes `RefreshBinding`
/// immediately. A `CobolTable` source is a WS table the running program's own
/// code has to fill first, so codegen cannot know when it is ready and leaves
/// the first refresh to the developer; an indexed file has no such fill step —
/// the `.cidx` plus the file already on disk are everything needed to populate
/// the grid the moment the binding loads.
fn write_indexed_file_binding_seed(out: &mut String, control_id: &str, binding: &DataBindingDef) {
    let BindingSourceDescriptor::IndexedFile {
        definition_path,
        fields,
        ..
    } = &binding.source
    else {
        return;
    };
    if fields.is_empty() || definition_path.trim().is_empty() {
        return;
    }
    let fields_joined = fields
        .iter()
        .map(|field| field.name.as_str())
        .collect::<Vec<_>>()
        .join(",");
    out.push_str(&format!(
        "           INVOKE {control_id} 'SetProperty' USING BY CONTENT \"_BindingKind\" BY CONTENT \"IndexedFile\"\n"
    ));
    out.push_str(&format!(
        "           INVOKE {control_id} 'SetProperty' USING BY CONTENT \"_BindingFields\" BY CONTENT \"{fields_joined}\"\n"
    ));
    out.push_str(&format!(
        "           INVOKE {control_id} 'SetProperty' USING BY CONTENT \"_BindingIndexedPath\" BY CONTENT \"{definition_path}\"\n"
    ));
    out.push_str(&format!("           INVOKE {control_id} 'RefreshBinding'\n"));
}

/// Spec 039 T13 (and, retroactively, T6's `ScalarControl`): seed
/// `_BindingScalarField`/`_BindingScalarProperty` via generated COBOL
/// `INVOKE 'SetProperty'` calls, same shape as the DataGrid/ControlArray
/// seeding above — this is what makes a genuinely standalone `rcrun build`
/// binary's Knob/Gauge/Switch binding refresh on load, not just an
/// interpreted `rcrun run-form` (which also seeds it Rust-side, in
/// `cobolt-cli/src/form_gui.rs` — redundant with this but harmless, since
/// both write the same value).
fn write_scalar_binding_seed(out: &mut String, form: &Form, control_id: &str, binding: &DataBindingDef) {
    if !matches!(&binding.source, BindingSourceDescriptor::CobolTable { .. }) {
        return;
    }
    let Some(scalar_field) = binding.mappings.iter().find_map(|m| {
        matches!(&m.target, BindingTargetPath::ScalarValue { .. }).then(|| m.source_field.as_str())
    }) else {
        return;
    };
    let property = form
        .find_control(control_id)
        .and_then(|c| c.scalar_binding_property())
        .unwrap_or("Value");
    out.push_str(&format!(
        "           INVOKE {control_id} 'SetProperty' USING BY CONTENT \"_BindingKind\" BY CONTENT \"CobolTable\"\n"
    ));
    out.push_str(&format!(
        "           INVOKE {control_id} 'SetProperty' USING BY CONTENT \"_BindingScalarField\" BY CONTENT \"{scalar_field}\"\n"
    ));
    out.push_str(&format!(
        "           INVOKE {control_id} 'SetProperty' USING BY CONTENT \"_BindingScalarProperty\" BY CONTENT \"{property}\"\n"
    ));
    out.push_str(&format!("           INVOKE {control_id} 'RefreshBinding'\n"));
}

/// Spec 039 T13/R22: seed `_BindingMarkerFields` (positional
/// `id\tlat\tlng\tlabel\tinfo`) via generated COBOL, mirroring
/// `write_scalar_binding_seed` above. A binding missing lat or lng (the
/// Guardian should already have blocked this before Build/Run/Debug ever
/// reach codegen) emits nothing rather than a field spec `refresh_marker_
/// binding` would treat as "not bound".
fn write_marker_binding_seed(out: &mut String, control_id: &str, binding: &DataBindingDef) {
    if !matches!(&binding.source, BindingSourceDescriptor::CobolTable { .. }) {
        return;
    }
    let field_for = |field: cobolt_forms::MapMarkerField| -> String {
        binding
            .mappings
            .iter()
            .find_map(|m| match &m.target {
                BindingTargetPath::MarkerField { field: f, .. } if *f == field => {
                    Some(m.source_field.clone())
                }
                _ => None,
            })
            .unwrap_or_default()
    };
    let lat = field_for(cobolt_forms::MapMarkerField::Lat);
    let lng = field_for(cobolt_forms::MapMarkerField::Lng);
    if lat.is_empty() || lng.is_empty() {
        return;
    }
    let id = field_for(cobolt_forms::MapMarkerField::Id);
    let label = field_for(cobolt_forms::MapMarkerField::Label);
    let info = field_for(cobolt_forms::MapMarkerField::Info);
    let spec = format!("{id}\t{lat}\t{lng}\t{label}\t{info}");
    out.push_str(&format!(
        "           INVOKE {control_id} 'SetProperty' USING BY CONTENT \"_BindingKind\" BY CONTENT \"CobolTable\"\n"
    ));
    out.push_str(&format!(
        "           INVOKE {control_id} 'SetProperty' USING BY CONTENT \"_BindingMarkerFields\" BY CONTENT \"{spec}\"\n"
    ));
    out.push_str(&format!("           INVOKE {control_id} 'RefreshBinding'\n"));
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
        BindingTargetDescriptor::ScalarControl { control_id } => {
            format!("ScalarControl:{control_id}")
        }
        BindingTargetDescriptor::MarkerCollection { control_id } => {
            format!("MarkerCollection:{control_id}")
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
        BindingTargetPath::ScalarValue { control_id } => format!("{control_id}.Value"),
        BindingTargetPath::MarkerField { control_id, field } => {
            format!("{control_id}.{}", field.as_str())
        }
    }
}
