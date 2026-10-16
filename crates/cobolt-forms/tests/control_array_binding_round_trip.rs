// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Spec 024 AC12 (R22): a form with a repeating group bound to a COBOL table saves and
//! loads back identical — the binding, the group's own properties and its members.

use cobolt_forms::{
    load_form, save_form, BindingDataType, BindingField, BindingMode, BindingSourceDescriptor, BindingTargetDescriptor,
    BindingTargetPath, Control, ControlType, DataBindingDef, EventBinding, FieldMapping, Form, PropValue,
};

fn bound_form() -> Form {
    let mut form = Form::new("CARDS-FORM", "Cards", 640, 480);
    let mut group = Control::new("ROWS", ControlType::GroupBox, 10, 10);
    group.set_prop("IsRepeatingGroup", PropValue::Bool(true));
    group.set_prop("ArrayName", PropValue::String("ACTORS".into()));
    group.set_prop("LayoutDirection", PropValue::String("Grid".into()));
    group.set_prop("ItemsPerRow", PropValue::Int(3));
    let mut name = Control::new("TXT-NAME", ControlType::TextBox, 12, 12);
    name.parent = Some("ROWS".into());
    name.events.push(EventBinding::new("onChange", "TXT-NAME--ONCHANGE"));
    let mut fee = Control::new("NUD-FEE", ControlType::NumericUpDown, 12, 40);
    fee.parent = Some("ROWS".into());
    form.add_control(group);
    form.add_control(name);
    form.add_control(fee);
    let mut binding = DataBindingDef::new(
        "BIND-ROWS",
        "Rows",
        BindingSourceDescriptor::CobolTable {
            table_name: "ACTORS".into(),
            occurs_item: "ACTORS".into(),
            fields: vec![
                BindingField::new("ACTOR-NAME", BindingDataType::Text),
                BindingField::new("ACTOR-FEE", BindingDataType::Integer),
            ],
            key_fields: Vec::new(),
            writable: true,
        },
        BindingTargetDescriptor::ControlArray { array_id: "ACTORS".into(), member_control_ids: vec!["TXT-NAME".into(), "NUD-FEE".into()] },
    )
    .with_mappings(vec![
        FieldMapping::new("ACTOR-NAME", BindingTargetPath::ControlProperty { array_id: "ACTORS".into(), control_id: "TXT-NAME".into(), property_name: "Text".into() }),
        FieldMapping::new("ACTOR-FEE", BindingTargetPath::ControlProperty { array_id: "ACTORS".into(), control_id: "NUD-FEE".into(), property_name: "Value".into() }),
    ]);
    binding.mode = BindingMode::Writable;
    form.data_bindings.push(binding);
    form
}

#[test]
fn an_array_bound_form_saves_and_loads_identical_and_writes_the_same_bytes_again() {
    let dir = std::env::temp_dir().join(format!("prc-024-roundtrip-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let (first, second) = (dir.join("a.cfrm"), dir.join("b.cfrm"));
    let form = bound_form();
    save_form(&form, &first).unwrap();
    let loaded = load_form(&first).unwrap();

    assert_eq!(loaded.data_bindings, form.data_bindings, "the binding changed");
    assert_eq!(loaded.data_bindings[0].mode, BindingMode::Writable);
    let group = loaded.find_control("ROWS").expect("the group");
    assert_eq!(group.get_prop("ArrayName").map(|v| v.as_str().to_owned()), Some("ACTORS".to_owned()));
    for member in ["TXT-NAME", "NUD-FEE"] {
        assert_eq!(loaded.find_control(member).and_then(|c| c.parent.as_deref()), Some("ROWS"), "{member}");
    }
    assert_eq!(loaded.find_control("TXT-NAME").unwrap().events.len(), 1, "the member's handler");

    save_form(&loaded, &second).unwrap();
    let bytes = std::fs::read(&first).unwrap();
    assert_eq!(bytes, std::fs::read(&second).unwrap(), "saving what was loaded must not change a byte");
    let _ = std::fs::remove_dir_all(&dir);
    println!("spec 024 AC12 — an array-bound form: 1 binding, 2 mappings, 3 controls, {} bytes, identical after load and save", bytes.len());
}
