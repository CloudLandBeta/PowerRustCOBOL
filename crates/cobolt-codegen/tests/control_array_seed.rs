// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Spec 024 T10 / T12 (R17, R18, R21): a compiled binary has no host to seed a bound
//! repeating group, so the generated COBOL seeds what the group needs — its fields, its
//! field → member-property mappings, and whether what the operator types into a card is
//! written back — and the program it generates parses and passes the analyser.

use cobolt_codegen::generate;
use cobolt_forms::{
    BindingDataType, BindingField, BindingMode, BindingSourceDescriptor, BindingTargetDescriptor, BindingTargetPath,
    Control, ControlType, DataBindingDef, FieldMapping, Form, PropValue,
};

fn form(mode: BindingMode) -> Form {
    let mut form = Form::new("F", "F", 400, 300);
    let mut group = Control::new("ROWS", ControlType::GroupBox, 0, 0);
    group.set_prop("IsRepeatingGroup", PropValue::Bool(true));
    group.set_prop("ArrayName", PropValue::String("ACTORS".into()));
    let mut name = Control::new("TXT-NAME", ControlType::TextBox, 10, 10);
    name.parent = Some("ROWS".into());
    form.add_control(group);
    form.add_control(name);
    let mut binding = DataBindingDef::new(
        "BIND-ROWS",
        "rows",
        BindingSourceDescriptor::CobolTable {
            table_name: "ACTORS".into(),
            occurs_item: "ACTORS".into(),
            fields: vec![BindingField::new("ACTOR-ID", BindingDataType::Integer), BindingField::new("ACTOR-NAME", BindingDataType::Text)],
            key_fields: Vec::new(),
            writable: true,
        },
        BindingTargetDescriptor::ControlArray { array_id: "ACTORS".into(), member_control_ids: vec!["TXT-NAME".into()] },
    )
    .with_mappings(vec![FieldMapping::new(
        "ACTOR-NAME",
        BindingTargetPath::ControlProperty { array_id: "ACTORS".into(), control_id: "TXT-NAME".into(), property_name: "Text".into() },
    )]);
    binding.mode = mode;
    form.data_bindings.push(binding);
    form
}

fn seeds(src: &str, name: &str) -> Vec<String> {
    let needle = format!("USING BY CONTENT \"{name}\" BY CONTENT \"");
    src.lines()
        .filter(|l| l.contains(&needle))
        .map(|l| l.split(&needle).nth(1).unwrap().strip_suffix('"').unwrap().to_owned())
        .collect()
}

#[test]
fn a_writable_group_is_seeded_to_write_back_and_a_read_only_one_is_not() {
    let writable = generate(&form(BindingMode::Writable));
    assert_eq!(seeds(&writable, "_BindingArray"), ["1"]);
    assert_eq!(seeds(&writable, "_BindingWritable"), ["1"]);
    assert_eq!(seeds(&writable, "_BindingFields"), ["ACTOR-ID,ACTOR-NAME"]);
    let mappings = seeds(&writable, "_BindingMappings");
    assert_eq!(mappings.len(), 1);
    assert!(mappings[0].contains("ACTOR-NAME") && mappings[0].contains("TXT-NAME") && mappings[0].contains("Text"), "{mappings:?}");

    let read_only = generate(&form(BindingMode::ReadOnly));
    assert_eq!(seeds(&read_only, "_BindingWritable"), ["0"]);
}

/// The form with a second member mapped, the way PowerDEMO maps `ITEM1 -> TextBox-1.Text`
/// and `ITEM2 -> Button-1.Caption`.
fn two_mapping_form(mode: BindingMode) -> Form {
    let mut form = form(mode);
    let mut fee = Control::new("BTN-ID", ControlType::Button, 10, 40);
    fee.parent = Some("ROWS".into());
    form.add_control(fee);
    let binding = &mut form.data_bindings[0];
    if let BindingTargetDescriptor::ControlArray { member_control_ids, .. } = &mut binding.target {
        member_control_ids.push("BTN-ID".into());
    }
    binding.mappings.push(FieldMapping::new(
        "ACTOR-ID",
        BindingTargetPath::ControlProperty { array_id: "ACTORS".into(), control_id: "BTN-ID".into(), property_name: "Caption".into() },
    ));
    form
}

/// A group mapping two members used to emit its mappings one per line inside a single
/// literal: the line break ended the literal and the whole generated program failed to
/// parse ("unterminated alphanumeric literal"), so the form could not run at all.
#[test]
fn two_mappings_stay_on_one_line_of_the_literal() {
    let src = generate(&two_mapping_form(BindingMode::Writable));
    assert_eq!(seeds(&src, "_BindingMappings"), ["ACTOR-NAME\tTXT-NAME\tText;ACTOR-ID\tBTN-ID\tCaption"]);
}

#[test]
fn the_generation_is_byte_stable_and_the_program_parses_and_checks() {
    let builds: [fn(BindingMode) -> Form; 2] = [form, two_mapping_form];
    for (build, mode) in builds.into_iter().flat_map(|b| [(b, BindingMode::Writable), (b, BindingMode::ReadOnly)]) {
        let first = generate(&build(mode.clone()));
        assert_eq!(first, generate(&build(mode)), "not deterministic");
        assert!(first.contains("RefreshBinding"), "the group is loaded as the form opens");
        let parsed = cobolt_parser::parse(cobolt_lexer::tokenize(&first, cobolt_lexer::SourceFormat::Free));
        let errors: Vec<String> = parsed.diagnostics.iter().filter(|d| d.is_error()).map(|d| format!("line {}: {}", d.span.line, d.message)).collect();
        assert!(errors.is_empty(), "{errors:?}\n{first}");
        let program = parsed.program.expect("a program");
        let sem: Vec<String> = cobolt_semantic::analyze(&program).errors().map(|d| d.message.clone()).collect();
        assert!(sem.is_empty(), "{sem:?}");
    }
}
