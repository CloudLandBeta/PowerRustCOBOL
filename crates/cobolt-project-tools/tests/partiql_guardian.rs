// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Spec 108 (T24, the findings that need only the form): the Guardian judges a
//! PartiQL binding — its source control, its query, its parameters.

use cobolt_forms::{
    BindingDataType, BindingField, BindingSourceDescriptor, BindingTargetDescriptor, Control, ControlType,
    DataBindingDef, Form,
};
use cobolt_project_tools::binding_guardian::validate_form_bindings;

fn fields() -> Vec<BindingField> {
    vec![BindingField::new("CUST-ID", BindingDataType::Integer).key(), BindingField::new("CUST-NAME", BindingDataType::Text)]
}

fn form_with(source_id: &str, query: &str, params: &[&str]) -> Form {
    let mut form = Form::new("F", "F", 400, 300);
    form.add_control(Control::new("IXF-CUST", ControlType::IndexedFile, 0, 0));
    form.add_control(Control::new("LBL-1", ControlType::Label, 0, 0));
    form.add_control(Control::new("DG-1", ControlType::DataGrid, 0, 0));
    form.data_bindings.push(DataBindingDef::new(
        "BIND-1",
        "b",
        BindingSourceDescriptor::PartiQl {
            source_control_id: source_id.into(),
            query: query.into(),
            parameters: params.iter().map(|p| p.to_string()).collect(),
            fields: fields(),
            key_fields: vec!["CUST-ID".into()],
            writable: false,
        },
        BindingTargetDescriptor::DataGrid { control_id: "DG-1".into() },
    ));
    form
}

fn codes(form: &Form) -> Vec<String> {
    validate_form_bindings(form)
        .into_iter()
        .filter(|f| f.binding_id == "BIND-1" && f.code != "missing-mapping")
        .map(|f| f.code)
        .collect()
}

#[test]
fn the_guardian_judges_a_partiql_binding() {
    // A valid one: no PartiQL finding.
    let ok = codes(&form_with("IXF-CUST", "SELECT CUST-ID, CUST-NAME FROM CUSTOMERS WHERE CUST-ID = ?", &["WS-ID"]));
    assert!(!ok.iter().any(|c| matches!(c.as_str(), "missing-source" | "invalid-partiql" | "missing-parameter")), "{ok:?}");
    // A Label as the source.
    assert!(codes(&form_with("LBL-1", "SELECT CUST-ID FROM CUSTOMERS", &[])).contains(&"missing-source".to_string()));
    // An UPDATE as the query, and a statement that does not parse.
    assert!(codes(&form_with("IXF-CUST", "UPDATE CUSTOMERS SET CUST-NAME = 'x' WHERE CUST-ID = 1", &[])).contains(&"invalid-partiql".to_string()));
    assert!(codes(&form_with("IXF-CUST", "SELECT COUNT(*) FROM CUSTOMERS", &[])).contains(&"invalid-partiql".to_string()));
    // Two `?` and one parameter.
    assert!(codes(&form_with("IXF-CUST", "SELECT CUST-ID FROM CUSTOMERS WHERE CUST-ID BETWEEN ? AND ?", &["WS-ID"])).contains(&"missing-parameter".to_string()));
    // An empty query follows the control and needs no parameter.
    let follow = codes(&form_with("IXF-CUST", "", &[]));
    assert!(!follow.iter().any(|c| matches!(c.as_str(), "invalid-partiql" | "missing-parameter")), "{follow:?}");
}
