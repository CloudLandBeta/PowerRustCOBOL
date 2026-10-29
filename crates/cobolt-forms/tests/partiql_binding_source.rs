// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Spec 108 (T17): a `PartiQl` binding source round-trips through serde as the
//! other sources do, and a short or older descriptor still reads (every field
//! of the new variant is optional).

use cobolt_forms::{
    BindingDataType, BindingField, BindingSourceDescriptor, BindingTargetDescriptor, DataBindingDef,
};

fn source() -> BindingSourceDescriptor {
    BindingSourceDescriptor::PartiQl {
        source_control_id: "IXF-ORD".into(),
        query: "SELECT ORDER-NO FROM ORDERS WHERE CUST-ID = ?".into(),
        parameters: vec!["DG-CUSTOMERS::SelectedValue".into()],
        fields: vec![BindingField::new("ORDER-NO", BindingDataType::Integer).key()],
        key_fields: vec!["ORDER-NO".into()],
        writable: false,
    }
}

#[test]
fn the_partiql_source_survives_serde() {
    let binding = DataBindingDef::new("B", "b", source(), BindingTargetDescriptor::DataGrid { control_id: "G".into() });
    let json = serde_json::to_string(&binding).unwrap();
    let back: DataBindingDef = serde_json::from_str(&json).unwrap();
    assert_eq!(back.source, binding.source);
    // Every new field is optional: an older, shorter descriptor still reads.
    let short = r#"{"PartiQl":{"source_control_id":"IXF-ORD"}}"#;
    let parsed: BindingSourceDescriptor = serde_json::from_str(short).unwrap();
    assert!(matches!(parsed, BindingSourceDescriptor::PartiQl { ref query, .. } if query.is_empty()));
}

