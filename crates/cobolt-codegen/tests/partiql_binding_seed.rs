// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Spec 108 (T17 / T19): a `PartiQl` binding source round-trips through serde
//! as the other sources do, and the generated COBOL seeds it as hidden
//! `_Binding*` properties plus one `RefreshBinding` — with no `READ` and no
//! loop of COBOL.

use cobolt_codegen::generate;
use cobolt_forms::{
    BindingDataType, BindingField, BindingSourceDescriptor, BindingTargetDescriptor, Control, ControlType,
    DataBindingDef, Form,
};

fn source() -> BindingSourceDescriptor {
    BindingSourceDescriptor::PartiQl {
        source_control_id: "IXF-ORD".into(),
        query: "SELECT ORDER-NO, ORDER-DATE, ORDER-TOTAL\n  FROM ORDERS WHERE CUST-ID = ?".into(),
        parameters: vec!["DG-CUSTOMERS::SelectedValue".into()],
        fields: vec![
            BindingField::new("ORDER-NO", BindingDataType::Integer).key(),
            BindingField::new("ORDER-DATE", BindingDataType::Text),
            BindingField::new("ORDER-TOTAL", BindingDataType::Decimal),
        ],
        key_fields: vec!["ORDER-NO".into()],
        writable: false,
    }
}

fn generated(control: (&str, ControlType), target: BindingTargetDescriptor) -> String {
    let mut form = Form::new("F", "F", 400, 300);
    form.add_control(Control::new("IXF-ORD", ControlType::IndexedFile, 0, 0));
    form.add_control(Control::new(control.0, control.1, 0, 0));
    form.data_bindings.push(DataBindingDef::new("BIND-1", "b", source(), target));
    generate(&form)
}

fn seeds(src: &str, control: &str, name: &str) -> Vec<String> {
    let needle = format!("INVOKE {control} 'SetProperty' USING BY CONTENT \"{name}\" BY CONTENT \"");
    src.lines()
        .filter(|l| l.contains(&needle))
        .map(|l| l.split(&needle).nth(1).unwrap().strip_suffix('"').unwrap().to_owned())
        .collect()
}

#[test]
fn a_grid_and_a_list_are_seeded_and_refreshed_with_no_read_and_no_loop() {
    let grid = generated(("DG-ORDERS", ControlType::DataGrid), BindingTargetDescriptor::DataGrid { control_id: "DG-ORDERS".into() });
    assert_eq!(seeds(&grid, "DG-ORDERS", "_BindingKind"), ["PartiQl"]);
    assert_eq!(seeds(&grid, "DG-ORDERS", "_BindingPartiqlControl"), ["IXF-ORD"]);
    assert_eq!(seeds(&grid, "DG-ORDERS", "_BindingPartiqlQuery"), ["SELECT ORDER-NO, ORDER-DATE, ORDER-TOTAL FROM ORDERS WHERE CUST-ID = ?"]);
    assert_eq!(seeds(&grid, "DG-ORDERS", "_BindingPartiqlParams"), ["DG-CUSTOMERS::SelectedValue"]);
    assert_eq!(seeds(&grid, "DG-ORDERS", "_BindingFields"), ["ORDER-NO,ORDER-DATE,ORDER-TOTAL"]);
    assert_eq!(seeds(&grid, "DG-ORDERS", "_BindingFieldTypes"), ["number,text,number"]);
    assert_eq!(seeds(&grid, "DG-ORDERS", "_BindingKeyFields"), ["ORDER-NO"]);
    assert!(grid.contains("INVOKE DG-ORDERS 'RefreshBinding'"));
    assert!(!grid.contains("READ "), "no READ in the generated program");
    assert!(!grid.contains("OPEN INPUT"), "the control holds the file");

    let list = generated(("CMB-1", ControlType::ComboBox), BindingTargetDescriptor::ComboBox { control_id: "CMB-1".into() });
    assert_eq!(seeds(&list, "CMB-1", "_BindingList"), ["1"]);
    assert!(list.contains("INVOKE CMB-1 'RefreshBinding'"));
}
