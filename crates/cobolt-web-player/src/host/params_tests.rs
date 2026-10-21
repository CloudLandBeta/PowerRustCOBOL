// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! The parameters a form is opened with (WASM-011 R4): what the program passes to `OpenFormSync` and
//! `OpenFormAsync` after the form's name, what a deep link carries, and what the opened form reads back
//! with `GetParam` / `GetParams` — at `onLoad` already, since the tree is made before the handler runs.

use super::*;
use cobolt_forms::xml::load_form_from_str;
use cobolt_wasm_abi::manifest::{FormEntry, Limits, VariantInfo};

fn form_entry(id: u32, name: &str, main: bool) -> FormEntry {
    FormEntry { id, name: name.into(), main, instances: 4, region_size: 64, controls: vec![], handlers: vec![] }
}

fn host(standalone: bool) -> Host {
    let manifest = Manifest {
        abi: 1,
        generator: "test".into(),
        project: "demo".into(),
        variant: VariantInfo { multi_memory: true, conformance: false },
        limits: Limits::default(),
        names: vec![],
        events: vec!["onLoad".into()],
        forms: vec![form_entry(0, "main-form", true), form_entry(1, "child-form", false)],
        builtins: vec![],
        imports: vec![],
        sizes: Default::default(),
    };
    let mut h = Host::new(manifest, HostOptions { standalone, ..HostOptions::default() });
    for name in ["main-form", "child-form"] {
        let xml = format!(r#"<?xml version="1.0" encoding="UTF-8"?><Form name="{name}" title="{name}" width="320" height="200" index-base="1"></Form>"#);
        h.add_form(load_form_from_str(&xml).unwrap()).unwrap();
    }
    h
}

/// The open the module is asked for next, the way the driver pops it, and the instance it would create.
fn open_next(h: &mut Host, inst: u32) {
    let call = h.next_call().expect("an open is queued");
    assert!(matches!(call, ModuleCall::OpenForm { .. }), "{call:?}");
    h.ensure_tree(inst).expect("the instance's tree is made while its start-up handler runs");
}

#[test]
fn the_arguments_after_the_form_name_are_name_value_pairs_and_a_bad_one_is_refused_with_the_reason() {
    assert_eq!(parse_params(&["id=7", " mode = edit", "q=a=b", ""]).unwrap(), vec![("id".to_owned(), "7".to_owned()), ("mode".into(), " edit".into()), ("q".into(), "a=b".into())]);
    assert!(parse_params(&["id"]).unwrap_err().contains("name=value"));
    assert!(parse_params(&["bad name=1"]).unwrap_err().contains("not a parameter name"));
    assert!(parse_params(&["=1"]).unwrap_err().contains("not a parameter name"));
    assert!(parse_params(&["id=1", "ID=2"]).unwrap_err().contains("twice"), "names are case-insensitive");
    assert!(parse_params(&[format!("k={}", "x".repeat(MAX_PARAM_VALUE + 1)).as_str()]).unwrap_err().contains("longer"));
    let many: Vec<String> = (0..=MAX_PARAMS).map(|i| format!("p{i}=1")).collect();
    let refs: Vec<&str> = many.iter().map(String::as_str).collect();
    assert!(parse_params(&refs).unwrap_err().contains("at most"));
}

#[test]
fn a_form_opened_with_parameters_reads_them_back_from_its_first_handler() {
    let mut h = host(false);
    h.create_tree(0, 0, None, Vec::new()).unwrap();
    let r = h.form_verb(0, "OpenFormAsync", &["child-form", "id=7", "mode=edit"]).unwrap();
    assert!(r.is_some(), "the verb answers a handle");
    open_next(&mut h, 1);
    assert_eq!(h.trees[&1].params, vec![("id".to_owned(), "7".to_owned()), ("mode".into(), "edit".into())]);
    assert_eq!(h.form_verb(1, "GetParam", &["id"]).unwrap(), Some(Some("7".to_owned())));
    assert_eq!(h.form_verb(1, "GetParam", &["MODE"]).unwrap(), Some(Some("edit".to_owned())), "names are case-insensitive");
    assert_eq!(h.form_verb(1, "GetParam", &["missing"]).unwrap(), Some(Some(String::new())), "an absent one is empty, not an error");
    assert_eq!(h.form_verb(1, "GetParams", &[]).unwrap(), Some(Some("id=7\nmode=edit".to_owned())));
    // The main form, opened with none, has none.
    assert_eq!(h.form_verb(0, "GetParams", &[]).unwrap(), Some(Some(String::new())));
}

#[test]
fn a_refused_parameter_refuses_the_open_and_queues_nothing() {
    let mut h = host(false);
    h.create_tree(0, 0, None, Vec::new()).unwrap();
    let e = h.form_verb(0, "OpenFormSync", &["child-form", "no equals sign"]).unwrap_err();
    assert!(matches!(e, CmdError::Bad(m) if m.contains("OpenFormSync") && m.contains("name=value")));
    assert!(h.work.is_empty(), "no form was opened");
}

#[test]
fn in_a_window_of_its_own_the_parameters_go_to_the_address() {
    let mut h = host(true);
    h.create_tree(0, 0, None, Vec::new()).unwrap();
    h.form_verb(0, "OpenFormAsync", &["child-form", "id=7"]).unwrap();
    let w: Vec<_> = h.effects.iter().filter_map(|e| if let Effect::OpenWindow { params, form, .. } = e { Some((*form, params.clone())) } else { None }).collect();
    assert_eq!(w, vec![(1usize, vec![("id".to_owned(), "7".to_owned())])]);
}

#[test]
fn what_comes_from_an_address_is_the_users_so_a_bad_parameter_is_left_out_not_trusted() {
    let mut h = host(false);
    h.start_with(
        1,
        vec![
            ("id".into(), "7".into()),
            ("<script>".into(), "x".into()),
            ("ID".into(), "dup".into()),
            ("long".into(), "x".repeat(MAX_PARAM_VALUE + 1)),
            ("ok.name-1_x".into(), "fine ação 日本".into()),
        ],
    );
    open_next(&mut h, 0);
    assert_eq!(h.trees[&0].params, vec![("id".to_owned(), "7".to_owned()), ("ok.name-1_x".into(), "fine ação 日本".into())]);
    // And never more than the limit.
    let mut h = host(false);
    h.start_with(0, (0..40).map(|i| (format!("p{i}"), "1".to_owned())).collect());
    open_next(&mut h, 0);
    assert_eq!(h.trees[&0].params.len(), MAX_PARAMS);
}
