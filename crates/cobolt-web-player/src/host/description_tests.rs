// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! The player reads the form description the Build writes (WASM-016 R3, R11; Q1: JSON, versioned), and
//! refuses one of another format with both numbers named.

use super::*;
use cobolt_forms::description::{describe, to_json, DESCRIPTION_FORMAT};
use cobolt_forms::xml::load_form_from_str;

fn host_and_json() -> (Host, String) {
    let (form, manifest) = crate::tree::testing::demo();
    (
        Host::new(manifest, HostOptions::default()),
        to_json(&describe(&form)),
    )
}

#[test]
fn add_description_registers_the_form_and_draws_its_controls() {
    let (mut h, json) = host_and_json();
    assert_eq!(h.missing_forms(), vec!["main-form".to_owned()]);
    h.add_description(&json).unwrap();
    assert!(h.missing_forms().is_empty());
    h.create_tree(0, 0, None, Vec::new()).unwrap();
    let t = &h.trees[&0];
    assert_eq!(t.get(t.by_name("TXT-IN").unwrap(), "Text"), "hello");
    println!("description of {} bytes registered and drawn", json.len());
}

#[test]
fn add_description_of_an_unknown_format_names_both_numbers() {
    let (mut h, json) = host_and_json();
    let mut v: serde_json::Value = serde_json::from_str(&json).unwrap();
    v["format"] = (DESCRIPTION_FORMAT + 1).into();
    let err = h.add_description(&v.to_string()).unwrap_err();
    assert!(
        err.contains(&format!("format {}", DESCRIPTION_FORMAT + 1))
            && err.contains(&format!("format {DESCRIPTION_FORMAT}")),
        "{err}"
    );
    assert_eq!(
        h.missing_forms(),
        vec!["main-form".to_owned()],
        "nothing registered"
    );
    println!("refused: {err}");
}

#[test]
fn a_cfrm_is_not_a_description() {
    let (mut h, _) = host_and_json();
    let xml = r#"<?xml version="1.0" encoding="UTF-8"?><Form name="main-form" title="x" width="10" height="10"></Form>"#;
    assert!(load_form_from_str(xml).is_ok(), "the text is a valid .cfrm");
    let err = h.add_description(xml).unwrap_err();
    assert!(err.contains("cannot be read"), "{err}");
}
