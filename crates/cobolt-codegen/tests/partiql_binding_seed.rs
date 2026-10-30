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

use cobolt_forms::{BindingChartKind, BindingTargetPath, FieldMapping, MapMarkerField};

fn generated_mapped(control: (&str, ControlType), target: BindingTargetDescriptor, mappings: Vec<FieldMapping>) -> String {
    let mut form = Form::new("F", "F", 400, 300);
    form.add_control(Control::new("IXF-ORD", ControlType::IndexedFile, 0, 0));
    form.add_control(Control::new(control.0, control.1, 0, 0));
    form.data_bindings.push(DataBindingDef::new("BIND-1", "b", source(), target).with_mappings(mappings));
    generate(&form)
}

#[test]
fn a_chart_a_scalar_control_a_map_and_a_repeating_group_are_seeded_the_same_way() {
    let chart = generated_mapped(
        ("CHT-1", ControlType::BarChart),
        BindingTargetDescriptor::Chart { control_id: "CHT-1".into(), chart_kind: BindingChartKind::Bar },
        vec![
            FieldMapping::new("ORDER-DATE", BindingTargetPath::ChartCategory { control_id: "CHT-1".into() }),
            FieldMapping::new("ORDER-TOTAL", BindingTargetPath::ChartValueSeries { control_id: "CHT-1".into(), series_id: "Total".into() }),
        ],
    );
    assert_eq!(seeds(&chart, "CHT-1", "_BindingKind"), ["PartiQl"]);
    assert_eq!(seeds(&chart, "CHT-1", "_BindingChart"), ["1"]);
    assert_eq!(seeds(&chart, "CHT-1", "_BindingFields"), ["ORDER-DATE,ORDER-TOTAL"]);
    assert!(chart.contains("INVOKE CHT-1 'RefreshBinding'") && !chart.contains("READ "));

    let gauge = generated_mapped(
        ("GAU-1", ControlType::Gauge),
        BindingTargetDescriptor::ScalarControl { control_id: "GAU-1".into() },
        vec![FieldMapping::new("ORDER-TOTAL", BindingTargetPath::ScalarValue { control_id: "GAU-1".into() })],
    );
    assert_eq!(seeds(&gauge, "GAU-1", "_BindingScalarProperty"), ["Value"]);
    assert_eq!(seeds(&gauge, "GAU-1", "_BindingFields"), ["ORDER-TOTAL"]);
    assert!(gauge.contains("INVOKE GAU-1 'RefreshBinding'"));

    let marker = |field| BindingTargetPath::MarkerField { control_id: "MAP-1".into(), field };
    let map = generated_mapped(
        ("MAP-1", ControlType::Maps),
        BindingTargetDescriptor::MarkerCollection { control_id: "MAP-1".into() },
        vec![
            FieldMapping::new("ORDER-NO", marker(MapMarkerField::Id)),
            FieldMapping::new("ORDER-TOTAL", marker(MapMarkerField::Lat)),
            FieldMapping::new("ORDER-NO", marker(MapMarkerField::Lng)),
            FieldMapping::new("ORDER-DATE", marker(MapMarkerField::Label)),
        ],
    );
    assert_eq!(seeds(&map, "MAP-1", "_BindingMarkerFields"), ["ORDER-NO\tORDER-TOTAL\tORDER-NO\tORDER-DATE\t"]);
    assert!(map.contains("INVOKE MAP-1 'RefreshBinding'"));

    let cards = generated_mapped(
        ("CARDS", ControlType::GroupBox),
        BindingTargetDescriptor::ControlArray { array_id: "CARDS".into(), member_control_ids: vec!["TXT-DATE".into()] },
        vec![FieldMapping::new(
            "ORDER-DATE",
            BindingTargetPath::ControlProperty { array_id: "CARDS".into(), control_id: "TXT-DATE".into(), property_name: "Text".into() },
        )],
    );
    assert_eq!(seeds(&cards, "CARDS", "_BindingArray"), ["1"]);
    assert_eq!(seeds(&cards, "CARDS", "_BindingMappings"), ["ORDER-DATE\tTXT-DATE\tText"]);
    assert!(cards.contains("INVOKE CARDS 'RefreshBinding'") && !cards.contains("READ "));
}
