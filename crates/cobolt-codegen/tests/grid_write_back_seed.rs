// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Spec 094: a compiled binary has no host to seed a bound grid, so the
//! generated COBOL seeds what the grid needs to write an edit back — the same
//! names and values `cobolt-form-host`'s
//! `a_bound_grid_is_seeded_with_what_it_needs_to_write_back` expects.

use cobolt_codegen::generate;
use cobolt_forms::{
    BindingDataType, BindingField, BindingMode, BindingSourceDescriptor, BindingTargetDescriptor, Control,
    ControlType, DataBindingDef, Form,
};

fn generated(source: BindingSourceDescriptor, mode: BindingMode) -> String {
    let mut form = Form::new("F", "F", 400, 300);
    form.add_control(Control::new("GRID-1", ControlType::DataGrid, 0, 0));
    let mut binding = DataBindingDef::new("BIND-1", "b", source, BindingTargetDescriptor::DataGrid { control_id: "GRID-1".into() });
    binding.mode = mode;
    form.data_bindings.push(binding);
    generate(&form)
}

fn seeds(src: &str, name: &str) -> Vec<String> {
    let needle = format!("USING BY CONTENT \"{name}\" BY CONTENT \"");
    src.lines()
        .filter(|l| l.contains(&needle))
        .map(|l| l.split(&needle).nth(1).unwrap().strip_suffix('"').unwrap().to_owned())
        .collect()
}

fn fields() -> Vec<BindingField> {
    vec![BindingField::new("ACTOR-ID", BindingDataType::Integer).key(), BindingField::new("ACTOR-NAME", BindingDataType::Text)]
}

#[test]
fn a_bound_grid_is_seeded_to_write_back() {
    let indexed = |writable| BindingSourceDescriptor::IndexedFile {
        definition_path: "indexed/actors.cidx".into(),
        record_name: "ACTOR-RECORD".into(),
        fields: fields(),
        key_field: Some("ACTOR-ID".into()),
        writable,
    };
    let on = generated(indexed(true), BindingMode::Writable);
    assert_eq!(seeds(&on, "_BindingId"), ["BIND-1"]);
    assert_eq!(seeds(&on, "_BindingWritable"), ["1"]);
    assert_eq!(seeds(&on, "_BindingKeyFields"), ["ACTOR-ID"]);
    assert_eq!(seeds(&generated(indexed(true), BindingMode::ReadOnly), "_BindingWritable"), ["0"]);
    assert_eq!(seeds(&generated(indexed(false), BindingMode::Writable), "_BindingWritable"), ["0"]);
}

#[test]
fn a_sql_bound_grid_is_seeded_with_its_query_table_and_connection() {
    let sql = generated(
        BindingSourceDescriptor::Sql {
            source_control_id: "SQL-1".into(),
            query_name: "q".into(),
            result_set_name: "r".into(),
            fields: fields(),
            key_fields: vec!["ACTOR-ID".into()],
            writable: true,
            query: "SELECT ACTOR-ID,\n   ACTOR-NAME FROM actors WHERE name <> \"x\"".into(),
            table_name: "actors".into(),
        },
        BindingMode::Writable,
    );
    assert_eq!(seeds(&sql, "_BindingKind"), ["Sql"]);
    assert_eq!(seeds(&sql, "_BindingSqlControl"), ["SQL-1"]);
    assert_eq!(seeds(&sql, "_BindingTable"), ["actors"]);
    assert_eq!(seeds(&sql, "_BindingFields"), ["ACTOR-ID,ACTOR-NAME"]);
    assert_eq!(seeds(&sql, "_BindingFieldTypes"), ["number,text"]);
    // A COBOL literal holds no line break and doubles a quote.
    assert_eq!(seeds(&sql, "_BindingSql"), ["SELECT ACTOR-ID, ACTOR-NAME FROM actors WHERE name <> \"\"x\"\""]);
    assert_eq!(seeds(&sql, "_BindingWritable"), ["1"]);
}
