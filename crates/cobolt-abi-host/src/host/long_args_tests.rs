// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! ABI amendment A37: an argument longer than the module's path arrives as `PART`s ahead of its
//! command, perhaps over several flushes, and the host makes the argument whole before it carries
//! the command out. This is how a chart gets a table of a million characters in the browser and on
//! the desktop runner, both of which use this host.

use super::*;
use cobolt_forms::xml::load_form_from_str;
use cobolt_wasm_abi::batch::{BatchWriter, PathSpec};
use cobolt_wasm_abi::manifest::{ControlEntry, FormEntry, Limits, VariantInfo};

fn host() -> Host {
    let form = load_form_from_str(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<Form name="main-form" title="Big" width="640" height="480" index-base="1">
  <Control id="LST-1" type="ListBox" x="10" y="10" w="200" h="120"></Control>
</Form>"#,
    )
    .expect("the form loads");
    let manifest = Manifest {
        contract: None,
        remote: Vec::new(),
        capabilities: Vec::new(),
        abi: 1,
        generator: "test".into(),
        project: "big".into(),
        variant: VariantInfo { multi_memory: true, conformance: false },
        limits: Limits::default(),
        names: vec!["AddItem".into()],
        events: vec!["onClick".into()],
        forms: vec![FormEntry {
            id: 0,
            name: "main-form".into(),
            main: true,
            instances: 1,
            region_size: 64,
            controls: vec![ControlEntry { id: 3, name: "LST-1".into(), kind: "ListBox".into() }],
            handlers: vec![],
            layers: vec![],
        }],
        builtins: vec![],
        imports: vec![],
        sizes: Default::default(),
    };
    let mut h = Host::new(manifest, HostOptions::default());
    h.add_form(form).unwrap();
    h.create_tree(0, 0, None, Vec::new()).expect("the tree builds");
    h
}

fn items(h: &Host) -> String {
    let idx = h.trees[&0].by_name("LST-1").unwrap();
    h.trees[&0].get(idx, "Items")
}

fn errors(h: &Host) -> Vec<String> {
    h.effects.iter().filter_map(|e| if let Effect::RuntimeError(r) = e { Some(r.message.clone()) } else { None }).collect()
}

#[test]
fn parts_over_two_flushes_make_one_whole_argument() {
    let mut h = host();
    let first = "a".repeat(60_000);
    let mut w = BatchWriter::new();
    w.part(0, 0, &first);
    h.flush(w.as_bytes());
    assert_eq!(items(&h), "", "nothing is carried out before the command comes");
    let mut w = BatchWriter::new();
    w.part(0, 0, "é-end").call(&PathSpec::call(0, 3, 0, &["!"])).call(&PathSpec::call(0, 3, 0, &["next"]));
    h.flush(w.as_bytes());
    let whole = format!("{first}é-end!");
    assert_eq!(items(&h), format!("{whole}\nnext"), "the pieces, then the path's own text; the next call is plain");

    // Pieces that name an argument the path does not have: the module's error, and the call is not made.
    let mut w = BatchWriter::new();
    w.part(0, 4, "lost").call(&PathSpec::call(0, 3, 0, &["never"]));
    h.flush(w.as_bytes());
    assert_eq!(items(&h), format!("{whole}\nnext"));
    let errs = errors(&h);
    assert_eq!(errs.len(), 1, "{errs:?}");
    assert!(errs[0].contains("argument 5 of segment 1"), "{errs:?}");

    // An abort forgets pieces whose command never came.
    let mut w = BatchWriter::new();
    w.part(0, 0, "stale");
    h.flush(w.as_bytes());
    h.abort(&[]);
    let mut w = BatchWriter::new();
    w.call(&PathSpec::call(0, 3, 0, &["after"]));
    h.flush(w.as_bytes());
    assert!(items(&h).ends_with("\nafter") && !items(&h).contains("stale"));
    eprintln!("long arguments: a {}-byte argument from 2 pieces over 2 flushes; 1 misaddressed piece refused; pieces forgotten at an abort: 4 of 4", whole.len());
}
