// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! The form descriptions of a bundle (WASM-016 R3, R4, R11; Q1: JSON, versioned): every form is
//! written as the JSON of `cobolt_forms::description`, at the format the bundle announces, and the
//! secret scan reads it.

mod support;

use cobolt_forms::description::{from_json, DESCRIPTION_FORMAT};
use cobolt_forms::model::PropValue;
use cobolt_forms::xml::{form_to_string, load_form_from_str};
use cobolt_web_bundle::{
    assemble_bundle, BundleOptions, PlayerInput, PlayerVariantInput, DESCRIPTION_VERSION,
};

/// A stand-in plain player: the descriptions do not depend on it.
fn player() -> PlayerInput {
    let plain = PlayerVariantInput {
        files: vec![
            (
                "player.js".into(),
                b"export default async function init() {}\n".to_vec(),
            ),
            ("player_bg.wasm".into(), vec![0u8; 64]),
        ],
        entry_js: "player.js".into(),
        entry_wasm: "player_bg.wasm".into(),
    };
    PlayerInput {
        plain: Some(plain),
        ..Default::default()
    }
}

#[test]
fn every_description_is_json_at_the_announced_format() {
    let out = support::assemble(player(), BundleOptions::default());
    let config = out.config();
    assert_eq!(config["versions"]["description"], DESCRIPTION_FORMAT);
    assert_eq!(DESCRIPTION_VERSION, DESCRIPTION_FORMAT);
    let forms = config["forms"].as_array().unwrap();
    assert!(!forms.is_empty());
    for f in forms {
        let url = f["url"].as_str().unwrap();
        assert!(url.starts_with("forms/") && url.ends_with(".json"), "{url}");
        let text = out
            .text(url)
            .unwrap_or_else(|| panic!("{url} is not in the bundle"));
        let d = from_json(&text).unwrap_or_else(|e| panic!("{url}: {e}"));
        assert_eq!(d.format, DESCRIPTION_FORMAT);
        assert!(d.name.eq_ignore_ascii_case(f["name"].as_str().unwrap()));
        assert!(
            !text.contains("PROCEDURE DIVISION") && !text.contains("<Form"),
            "{url} carries code or XML"
        );
        println!(
            "{url}: format {}, {} controls, {} bytes",
            d.format,
            d.controls.len(),
            text.len()
        );
    }
    assert!(
        !out.files.iter().any(|f| f.path.ends_with(".cfrm")),
        "no .cfrm is shipped"
    );
}

#[test]
fn a_planted_secret_in_a_property_stops_the_bundle() {
    let mut input = support::input_with(player(), BundleOptions::default());
    let mut form = load_form_from_str(&input.forms[0].cfrm).unwrap();
    // Built at run time so the repository holds no secret-shaped literal.
    let planted = format!("postgres://app:{}@db.example.com/prod", "s3cr3tPassw0rd");
    form.controls[0]
        .properties
        .insert("Caption".into(), PropValue::String(planted));
    input.forms[0].cfrm = form_to_string(&form).unwrap();
    let err = assemble_bundle(&input).expect_err("a secret in a description must stop the bundle");
    let shown = err.to_string();
    println!("{shown}");
    assert!(
        shown.contains("forms/") && shown.contains(".json") && shown.contains("connection string"),
        "{shown}"
    );
    assert!(
        !shown.contains("s3cr3t"),
        "the value is never printed: {shown}"
    );
}
