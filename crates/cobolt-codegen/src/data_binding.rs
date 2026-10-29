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

use crate::{next_line_number, GenOwner, SourceMap};

/// What a binding's generated lines are written for (spec 053 source map).
fn owner(binding: &DataBindingDef) -> GenOwner {
    GenOwner::DataBinding {
        binding_id: binding.id.clone(),
        target: binding.target.primary_control_id().to_owned(),
    }
}

pub fn write_data_binding_storage(out: &mut String, form: &Form, map: &mut SourceMap) {
    if form.data_bindings.is_empty() {
        return;
    }
    out.push_str("      *>-- Data binding runtime state --------------------------------\n");
    for binding in sorted_bindings(form) {
        let start = next_line_number(out);
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
        map.own(owner(binding), start, out);
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

pub fn write_data_binding_paragraphs(out: &mut String, form: &Form, map: &mut SourceMap) {
    if form.data_bindings.is_empty() {
        return;
    }
    out.push_str("       COBOL-DATA-BINDINGS-LOAD.\n");
    for binding in sorted_bindings(form) {
        let start = next_line_number(out);
        let pfx = binding_prefix(&binding.id);
        let read_only = if binding.mode == BindingMode::ReadOnly {
            "1"
        } else {
            "0"
        };
        out.push_str(&format!(
            "           COBOL::\"BINDING-SET-READ-ONLY\" ( \"{}\" \"{}\" )\n",
            binding.id, read_only
        ));
        out.push_str(&format!(
            "      *>    Load {} from {}.\n",
            binding.id,
            source_label(&binding.source)
        ));
        out.push_str(&format!(
            "           COBOL::\"BINDING-LOAD\" ( \"{}\" {}-STATUS )\n",
            binding.id, pfx
        ));
        map.own(owner(binding), start, out);
    }
    out.push_str("           CONTINUE.\n\n");

    out.push_str("       COBOL-DATA-BINDINGS-POPULATE.\n");
    for binding in sorted_bindings(form) {
        let start = next_line_number(out);
        let pfx = binding_prefix(&binding.id);
        out.push_str(&format!(
            "      *>    Populate {} target {}.\n",
            binding.id,
            target_label(&binding.target)
        ));
        write_binding_refresh_seed(out, form, binding);
        out.push_str(&format!(
            "           COBOL::\"BINDING-POPULATE\" ( \"{}\" {}-STATUS )\n",
            binding.id, pfx
        ));
        for mapping in sorted_mappings(binding) {
            out.push_str(&format!(
                "      *>       {} -> {}\n",
                mapping.source_field,
                target_path_label(&mapping.target)
            ));
        }
        map.own(owner(binding), start, out);
    }
    out.push_str("           CONTINUE.\n\n");

    out.push_str("       COBOL-DATA-BINDINGS-MARK-CLEAN.\n");
    for binding in sorted_bindings(form) {
        let start = next_line_number(out);
        let pfx = binding_prefix(&binding.id);
        out.push_str(&format!(
            "           COBOL::\"BINDING-MARK-CLEAN\" ( \"{}\" {}-DIRTY )\n",
            binding.id, pfx
        ));
        map.own(owner(binding), start, out);
    }
    out.push_str("           CONTINUE.\n\n");

    out.push_str("       COBOL-DATA-BINDINGS-UPDATE.\n");
    for binding in sorted_bindings(form)
        .into_iter()
        .filter(|binding| binding.mode == BindingMode::Writable)
    {
        let start = next_line_number(out);
        let pfx = binding_prefix(&binding.id);
        out.push_str(&format!(
            "           COBOL::\"BINDING-UPDATE\" ( \"{}\" {}-ROW-KEY {}-STATUS )\n",
            binding.id, pfx, pfx
        ));
        map.own(owner(binding), start, out);
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
    if let BindingTargetDescriptor::DataGrid { control_id } = &binding.target {
        write_grid_write_seed(out, control_id, binding);
    }
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
        BindingTargetDescriptor::DataGrid { control_id }
            if matches!(&binding.source, BindingSourceDescriptor::Sql { .. }) =>
        {
            write_sql_binding_seed(out, control_id, binding);
            return;
        }
        // Spec 108: a PartiQL source seeds hidden properties and one `RefreshBinding`;
        // no `READ`, no COBOL loop — the interpreter runs the query on the control.
        BindingTargetDescriptor::DataGrid { control_id }
        | BindingTargetDescriptor::ComboBox { control_id }
        | BindingTargetDescriptor::ListBox { control_id }
            if matches!(&binding.source, BindingSourceDescriptor::PartiQl { .. }) =>
        {
            write_partiql_binding_seed(out, control_id, binding);
            return;
        }
        BindingTargetDescriptor::ComboBox { control_id } | BindingTargetDescriptor::ListBox { control_id }
            if matches!(&binding.source, BindingSourceDescriptor::CobolTable { .. }) =>
        {
            write_list_binding_seed(out, control_id, binding);
            return;
        }
        BindingTargetDescriptor::Chart { control_id, .. }
            if matches!(&binding.source, BindingSourceDescriptor::CobolTable { .. }) =>
        {
            write_chart_binding_seed(out, control_id, binding);
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
        // Spec 024 R17/R18: whether what the operator types into a card is written back.
        out.push_str(&format!(
            "           INVOKE {control_id} 'SetProperty' USING BY CONTENT \"_BindingWritable\" BY CONTENT \"{}\"\n",
            if binding.mode == cobolt_forms::BindingMode::Writable && binding.source.is_writable() { "1" } else { "0" }
        ));
        // Seed target mappings so REFRESHBINDING can hydrate per-instance member values
        // from the source table (format: sourceField\tmemberControlId\tpropName, the rows
        // separated by `;` — a line break would end the COBOL literal).
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
            let joined = maps.join(";");
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
    } else {
        // A DataGrid loads as the form opens too: POPULATE runs after
        // `onLoad`, so a table filled by VALUE clauses or by `onLoad` is on
        // screen without a call. It used to wait for the program's own
        // `RefreshBinding()`, and a grid bound to a VALUE'd table stayed
        // empty (operator, 2026-09-25).
        out.push_str(&format!("           INVOKE {control_id} 'RefreshBinding'\n"));
    }
}

/// `CobolTable` source -> `ComboBox` / `ListBox` target: the list's items are
/// the table's occurrences of the field mapped to the list's display text
/// (the first field when no mapping names one). Refreshed here, at POPULATE,
/// which runs after `onLoad` — so a table filled by VALUE clauses or by the
/// form's own `onLoad` is listed when the form opens. `RefreshBinding` lists
/// it again whenever the program has changed the table.
fn write_list_binding_seed(out: &mut String, control_id: &str, binding: &DataBindingDef) {
    let BindingSourceDescriptor::CobolTable { fields, .. } = &binding.source else {
        return;
    };
    let display = binding
        .mappings
        .iter()
        .find(|m| matches!(m.target, BindingTargetPath::ListDisplayItem { .. }))
        .map(|m| m.source_field.as_str())
        .or_else(|| fields.first().map(|f| f.name.as_str()));
    let Some(display) = display else {
        return;
    };
    out.push_str(&format!(
        "           INVOKE {control_id} 'SetProperty' USING BY CONTENT \"_BindingKind\" BY CONTENT \"CobolTable\"\n"
    ));
    out.push_str(&format!(
        "           INVOKE {control_id} 'SetProperty' USING BY CONTENT \"_BindingFields\" BY CONTENT \"{display}\"\n"
    ));
    out.push_str(&format!(
        "           INVOKE {control_id} 'SetProperty' USING BY CONTENT \"_BindingList\" BY CONTENT \"1\"\n"
    ));
    out.push_str(&format!("           INVOKE {control_id} 'RefreshBinding'\n"));
}

/// `CobolTable` source -> chart target: one point per occurrence — the field
/// mapped to the chart's category is the label, and EVERY field mapped to a
/// value series is one series, in mapping order (spec 052 R1/R6: no cap).
/// More than one series also names them (`_BindingSeriesNames`), which the
/// legend and the tooltip show. Refreshed at POPULATE, after `onLoad`, like
/// a list.
fn write_chart_binding_seed(out: &mut String, control_id: &str, binding: &DataBindingDef) {
    let category = binding
        .mappings
        .iter()
        .find(|m| matches!(m.target, BindingTargetPath::ChartCategory { .. }))
        .map(|m| m.source_field.as_str());
    let series: Vec<(&str, &str)> = binding
        .sorted_mapping_refs()
        .into_iter()
        .filter_map(|m| match &m.target {
            BindingTargetPath::ChartValueSeries { series_id, .. } => Some((m.source_field.as_str(), series_id.as_str())),
            _ => None,
        })
        .collect();
    let (Some(category), false) = (category, series.is_empty()) else {
        return;
    };
    let value = series.iter().map(|(f, _)| *f).collect::<Vec<_>>().join(",");
    out.push_str(&format!(
        "           INVOKE {control_id} 'SetProperty' USING BY CONTENT \"_BindingKind\" BY CONTENT \"CobolTable\"\n"
    ));
    out.push_str(&format!(
        "           INVOKE {control_id} 'SetProperty' USING BY CONTENT \"_BindingFields\" BY CONTENT \"{category},{value}\"\n"
    ));
    out.push_str(&format!(
        "           INVOKE {control_id} 'SetProperty' USING BY CONTENT \"_BindingChart\" BY CONTENT \"1\"\n"
    ));
    if series.len() > 1 {
        let names = series.iter().map(|(_, id)| *id).collect::<Vec<_>>().join(",");
        out.push_str(&format!(
            "           INVOKE {control_id} 'SetProperty' USING BY CONTENT \"_BindingSeriesNames\" BY CONTENT \"{names}\"\n"
        ));
    }
    out.push_str(&format!("           INVOKE {control_id} 'RefreshBinding'\n"));
}

/// Spec 094: what a bound grid needs to write an edit back — which binding it
/// is, whether the binding may be written, and the fields that identify a row —
/// seeded the way the other `_Binding*` properties are, so a compiled binary has
/// them too. (`cobolt-form-host`'s `grid_write_seed` is the same list for the
/// interpreted Run Form.)
fn write_grid_write_seed(out: &mut String, control_id: &str, binding: &DataBindingDef) {
    let writable = binding.mode == BindingMode::Writable && binding.source.is_writable();
    let keys: Vec<String> = match &binding.source {
        BindingSourceDescriptor::IndexedFile { key_field, .. } => key_field.iter().cloned().collect(),
        BindingSourceDescriptor::Sql { key_fields, .. }
        | BindingSourceDescriptor::CobolTable { key_fields, .. }
        | BindingSourceDescriptor::PartiQl { key_fields, .. } => key_fields.clone(),
        _ => Vec::new(),
    };
    for (name, value) in [
        ("_BindingId", binding.id.clone()),
        ("_BindingWritable", if writable { "1" } else { "0" }.to_owned()),
        ("_BindingKeyFields", keys.join(",")),
    ] {
        out.push_str(&format!(
            "           INVOKE {control_id} 'SetProperty' USING BY CONTENT \"{name}\" BY CONTENT \"{value}\"\n"
        ));
    }
}

/// Spec 094: a `Sql` source -> `DataGrid` target. The connection is the
/// `SqlDatabase` control `source_control_id`; its query fills the grid and an
/// edit updates `table_name`. The program opens the connection, so the first
/// `RefreshBinding` here finds none and leaves the grid alone — the program
/// calls `RefreshBinding()` itself once the connection is open.
fn write_sql_binding_seed(out: &mut String, control_id: &str, binding: &DataBindingDef) {
    use cobolt_forms::BindingDataType as T;
    let BindingSourceDescriptor::Sql { source_control_id, fields, query, table_name, .. } = &binding.source else {
        return;
    };
    if fields.is_empty() {
        return;
    }
    // A COBOL literal holds no line break, and a quote inside one is doubled.
    let literal = |text: &str| text.split_whitespace().collect::<Vec<_>>().join(" ").replace('"', "\"\"");
    let names = fields.iter().map(|f| f.name.as_str()).collect::<Vec<_>>().join(",");
    let types = fields
        .iter()
        .map(|f| match f.data_type {
            T::Integer | T::Decimal => "number",
            T::Boolean => "boolean",
            _ => "text",
        })
        .collect::<Vec<_>>()
        .join(",");
    for (name, value) in [
        ("_BindingKind", "Sql".to_owned()),
        ("_BindingSqlControl", source_control_id.clone()),
        ("_BindingSql", query.clone()),
        ("_BindingTable", table_name.clone()),
        ("_BindingFields", names),
        ("_BindingFieldTypes", types),
    ] {
        out.push_str(&format!(
            "           INVOKE {control_id} 'SetProperty' USING BY CONTENT \"{name}\" BY CONTENT \"{}\"\n",
            literal(&value)
        ));
    }
    out.push_str(&format!("           INVOKE {control_id} 'RefreshBinding'\n"));
}

/// Spec 108: a `PartiQl` source -> `DataGrid` / `ComboBox` / `ListBox` target.
/// The source control is the `IndexedFile` control `source_control_id`; its
/// `query` (empty: follow the control's own result set) runs there with the
/// `parameters` filling the `?` in order — data item names, or `CONTROL::Property`.
/// Seeded as hidden properties and one `RefreshBinding`, as the other kinds are.
fn write_partiql_binding_seed(out: &mut String, control_id: &str, binding: &DataBindingDef) {
    use cobolt_forms::BindingDataType as T;
    let BindingSourceDescriptor::PartiQl { source_control_id, query, parameters, fields, .. } = &binding.source else {
        return;
    };
    if source_control_id.trim().is_empty() {
        return;
    }
    // A COBOL literal holds no line break, and a quote inside one is doubled.
    let literal = |text: &str| text.split_whitespace().collect::<Vec<_>>().join(" ").replace('"', "\"\"");
    let names = fields.iter().map(|f| f.name.as_str()).collect::<Vec<_>>().join(",");
    let types = fields
        .iter()
        .map(|f| match f.data_type {
            T::Integer | T::Decimal => "number",
            T::Boolean => "boolean",
            _ => "text",
        })
        .collect::<Vec<_>>()
        .join(",");
    let is_list = matches!(
        &binding.target,
        BindingTargetDescriptor::ComboBox { .. } | BindingTargetDescriptor::ListBox { .. }
    );
    let mut props: Vec<(&str, String)> = vec![
        ("_BindingKind", "PartiQl".to_owned()),
        ("_BindingPartiqlControl", source_control_id.clone()),
        ("_BindingPartiqlQuery", query.clone()),
        ("_BindingPartiqlParams", parameters.join(",")),
        ("_BindingFields", names),
        ("_BindingFieldTypes", types),
    ];
    if is_list {
        props.push(("_BindingList", "1".to_owned()));
    }
    for (name, value) in props {
        out.push_str(&format!(
            "           INVOKE {control_id} 'SetProperty' USING BY CONTENT \"{name}\" BY CONTENT \"{}\"\n",
            literal(&value)
        ));
    }
    out.push_str(&format!("           INVOKE {control_id} 'RefreshBinding'\n"));
}

/// `IndexedFile` source -> `DataGrid` target: seed `_BindingKind`/`_BindingFields`
/// (same shape as the `CobolTable` seed above) plus `_BindingIndexedPath` (the
/// `.cidx` definition path exactly as the Designer saved it — resolved at
/// runtime the same way a plain `SELECT ... ASSIGN TO` resolves its target:
/// against the process's own working directory, since the interpreter has no
/// project-root concept of its own).
///
/// Like the `CobolTable` seed, this invokes `RefreshBinding` at once: the
/// `.cidx` plus the file already on disk are everything needed to populate
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
        BindingSourceDescriptor::PartiQl { source_control_id, .. } => {
            format!("PartiQL:{source_control_id}")
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
