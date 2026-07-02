// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

use cobolt_codegen::generate;
use cobolt_forms::{
    BindingDataType, BindingField, BindingSourceDescriptor, BindingTargetDescriptor,
    BindingTargetPath, Control, ControlType, DataBindingDef, FieldMapping, Form,
};
use cobolt_lexer::{tokenize, SourceFormat};
use cobolt_parser::parse;
use cobolt_semantic::{analyze, Severity};

fn generated_binding_form() -> Form {
    let mut form = Form::new("BIND-FORM", "Bindings", 800, 600);
    form.add_control(Control::new("GRID-1", ControlType::DataGrid, 0, 0));
    let fields = vec![
        BindingField::new("ID", BindingDataType::Integer).key(),
        BindingField::new("NAME", BindingDataType::Text).required(),
    ];
    form.data_bindings.push(
        DataBindingDef::new(
            "BIND-IDX-GRID",
            "Indexed Grid",
            BindingSourceDescriptor::IndexedFile {
                definition_path: "data/customers.cidx".into(),
                record_name: "CUSTOMER-REC".into(),
                fields,
                key_field: Some("ID".into()),
                writable: true,
            },
            BindingTargetDescriptor::DataGrid {
                control_id: "GRID-1".into(),
            },
        )
        .with_mappings(vec![FieldMapping::new(
            "NAME",
            BindingTargetPath::GridColumn {
                control_id: "GRID-1".into(),
                column_id: "NAME".into(),
            },
        )]),
    );
    form
}

#[test]
fn data_binding_generated_cobol_is_semantically_valid() {
    let src = generate(&generated_binding_form());
    let parsed = parse(tokenize(&src, SourceFormat::Free));
    assert!(
        parsed.diagnostics.is_empty(),
        "{:?}\n{}",
        parsed.diagnostics,
        src
    );
    let program = parsed.program.expect("generated program");
    let diagnostics = analyze(&program).diagnostics;
    assert!(
        diagnostics
            .iter()
            .all(|diagnostic| diagnostic.severity != Severity::Error),
        "semantic errors: {:?}\n{}",
        diagnostics,
        src
    );
}
