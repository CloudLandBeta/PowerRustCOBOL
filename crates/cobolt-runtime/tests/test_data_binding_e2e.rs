// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

use cobolt_codegen::generate;
use cobolt_forms::{
    BindingDataType, BindingField, BindingMode, BindingSourceDescriptor, BindingTargetDescriptor,
    BindingTargetPath, Control, ControlType, DataBindingDef, FieldMapping, Form,
};
use cobolt_lexer::{tokenize, SourceFormat};
use cobolt_parser::{parse, Severity};
use cobolt_runtime::Interpreter;

fn generated_runtime_binding_form() -> Form {
    let mut form = Form::new("BIND-FORM", "Bindings", 800, 600);
    form.add_control(Control::new("GRID-1", ControlType::DataGrid, 0, 0));
    let fields = vec![
        BindingField::new("ID", BindingDataType::Integer).key(),
        BindingField::new("NAME", BindingDataType::Text).required(),
    ];
    let mut binding = DataBindingDef::new(
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
    )]);
    binding.mode = BindingMode::Writable;
    form.data_bindings.push(binding);
    form
}

#[test]
fn data_binding_e2e_generated_form_runs_through_runtime_helpers() {
    let src = generate(&generated_runtime_binding_form());
    let parsed = parse(tokenize(&src, SourceFormat::Free));
    assert!(
        parsed
            .diagnostics
            .iter()
            .all(|diagnostic| diagnostic.severity != Severity::Error),
        "parse errors: {:?}\n{}",
        parsed.diagnostics,
        src
    );
    let program = parsed.program.expect("generated program");
    let mut interpreter = Interpreter::new(program);
    interpreter
        .run()
        .expect("generated binding form should run");
}
