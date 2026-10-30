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

// ── Spec 108 T24: the findings that need the project's `.cidx` ──

use cobolt_project_tools::binding_guardian::validate_form_bindings_in;

fn cidx_in(dir: &std::path::Path) -> String {
    let xml = r#"<?xml version="1.0" encoding="UTF-8"?><IndexedFile name="CUSTOMERS" finalized="true" version="1.0"><assign-path>customers.idx</assign-path><access-mode>dynamic</access-mode><record-format fixed-length="56"/><storage mode="disk" compression="false" persistence="false"/><keys><primary duplicates="false" ordering="ascending"><part field="CUST-ID" offset="0" length="8" encoding="bytes"/></primary></keys><fields><Field level="1" name="CUST-REC" usage="display"><Field level="5" name="CUST-ID" pic="9(8)" usage="display" offset="0" length="8"></Field><Field level="5" name="CUST-NAME" pic="X(40)" usage="display" offset="8" length="40"></Field><Field level="5" name="CUST-BALANCE" pic="S9(6)V99" usage="display" offset="48" length="8"></Field></Field></fields></IndexedFile>"#;
    let path = dir.join("customers.cidx");
    std::fs::write(&path, xml).unwrap();
    "customers.cidx".to_owned()
}

fn form_over(cidx: &str, query: &str, writable: bool, field_names: &[&str]) -> Form {
    let mut form = form_with("IXF-CUST", query, &[]);
    for c in form.controls.iter_mut().filter(|c| c.id == "IXF-CUST") {
        c.set_prop("IndexedFile", cidx);
    }
    if let BindingSourceDescriptor::PartiQl { fields, writable: w, .. } = &mut form.data_bindings[0].source {
        *fields = field_names.iter().map(|n| BindingField::new(*n, BindingDataType::Text)).collect();
        *w = writable;
    }
    form
}

fn project_codes(form: &Form, dir: &std::path::Path) -> Vec<(String, bool)> {
    validate_form_bindings_in(form, dir)
        .into_iter()
        .filter(|f| f.binding_id == "BIND-1")
        .map(|f| (f.code, f.severity == cobolt_forms::GuardianSeverity::Warning))
        .collect()
}

#[test]
fn the_guardian_reads_a_query_against_the_cidx() {
    let dir = std::env::temp_dir().join(format!("prc-guardian-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let cidx = cidx_in(&dir);
    let has = |list: &[(String, bool)], code: &str| list.iter().any(|(c, _)| c == code);

    // A name the file does not have.
    let bad = project_codes(&form_over(&cidx, "SELECT CUST-ID, NO-SUCH-FIELD FROM CUSTOMERS", false, &["CUST-ID", "NO-SUCH-FIELD"]), &dir);
    assert!(has(&bad, "invalid-partiql"), "{bad:?}");
    // A WHERE on a field that is not a key reads the whole file - a warning, not a blocker.
    let scan = project_codes(&form_over(&cidx, "SELECT CUST-ID, CUST-NAME FROM CUSTOMERS WHERE CUST-BALANCE > 1000", false, &["CUST-ID", "CUST-NAME"]), &dir);
    assert!(scan.iter().any(|(c, warn)| c == "full-scan" && *warn), "{scan:?}");
    // A WHERE on the key does not.
    let keyed = project_codes(&form_over(&cidx, "SELECT CUST-ID, CUST-NAME FROM CUSTOMERS WHERE CUST-ID = 1", false, &["CUST-ID", "CUST-NAME"]), &dir);
    assert!(!has(&keyed, "full-scan") && !has(&keyed, "invalid-partiql"), "{keyed:?}");
    // A writable binding that projects CUST-NAME only cannot tell its rows apart.
    let anonymous = project_codes(&form_over(&cidx, "SELECT CUST-NAME FROM CUSTOMERS WHERE CUST-ID = 1", true, &["CUST-NAME"]), &dir);
    assert!(has(&anonymous, "missing-row-identity"), "{anonymous:?}");
    // With the key in the projection it can.
    let identified = project_codes(&form_over(&cidx, "SELECT CUST-ID, CUST-NAME FROM CUSTOMERS WHERE CUST-ID = 1", true, &["CUST-ID", "CUST-NAME"]), &dir);
    assert!(!has(&identified, "missing-row-identity"), "{identified:?}");
    let _ = std::fs::remove_dir_all(&dir);
}
