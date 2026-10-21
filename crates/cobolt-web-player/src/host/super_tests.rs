// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! `super::` across windows (WASM-011 R11): in a window another window opened, `super::Prop` is the form
//! that opened it. Reads come from the replica the opener keeps current; writes and calls are applied to
//! the replica and forwarded; when the opener is gone `super::` fails the way it does for a form nothing
//! opened. (The channel between the windows is the browser half, `superlink.rs`, proved in Chrome.)

use super::*;
use cobolt_forms::xml::load_form_from_str;
use cobolt_wasm_abi::batch::{Command, Path, Seg};
use cobolt_wasm_abi::manifest::{FormEntry, Limits, VariantInfo};

fn host() -> Host {
    let manifest = Manifest {
        abi: 1,
        generator: "test".into(),
        project: "demo".into(),
        variant: VariantInfo { multi_memory: true, conformance: false },
        limits: Limits::default(),
        names: vec!["Status".into(), "Refresh".into()],
        events: vec!["onLoad".into()],
        forms: vec![FormEntry { id: 0, name: "main-form".into(), main: true, instances: 4, region_size: 64, controls: vec![], handlers: vec![] }],
        builtins: vec![],
        imports: vec![],
        sizes: Default::default(),
    };
    let mut h = Host::new(manifest, HostOptions { standalone: true, ..HostOptions::default() });
    let xml = r#"<?xml version="1.0" encoding="UTF-8"?><Form name="main-form" title="main-form" width="320" height="200" index-base="1"></Form>"#;
    h.add_form(load_form_from_str(xml).unwrap()).unwrap();
    h.create_tree(0, 0, None, Vec::new()).unwrap();
    h
}

fn path<'a>(name: u32, args: Vec<&'a str>) -> Path<'a> {
    Path { inst: 0, ctrl: ctrl::SUPER, segs: vec![Seg { name, args }] }
}

fn get(h: &mut Host, name: u32) -> Result<String, i32> {
    h.eval_query(&Command::Get(path(name, vec![])))
}

#[test]
fn a_first_form_with_no_opener_anywhere_still_refuses_super() {
    let mut h = host();
    assert!(get(&mut h, 0).is_err(), "nothing opened this form, in this window or another");
}

#[test]
fn a_read_of_super_is_the_openers_property_as_it_last_sent_it_and_the_names_ignore_case() {
    let mut h = host();
    h.attach_remote_opener();
    assert_eq!(get(&mut h, 0).unwrap(), "", "before the opener has sent anything a property is empty, not an error");
    h.remote_opener_state([("Status".to_owned(), "ready".to_owned()), ("Lines".to_owned(), "a\nb\nc".to_owned())].into_iter().collect());
    assert_eq!(get(&mut h, 0).unwrap(), "ready");
    h.remote_opener_state([("STATUS".to_owned(), "busy".to_owned())].into_iter().collect());
    assert_eq!(get(&mut h, 0).unwrap(), "busy", "a newer state replaces the older one");
}

#[test]
fn a_write_is_seen_by_the_next_read_at_once_and_is_forwarded_to_the_opener() {
    let mut h = host();
    h.attach_remote_opener();
    h.remote_opener_state([("Status".to_owned(), "ready".to_owned())].into_iter().collect());
    h.cmd_set(&path(0, vec![]), "saved").unwrap();
    assert_eq!(get(&mut h, 0).unwrap(), "saved", "the replica took it before the opener's next message");
    let sent: Vec<_> = h.effects.iter().filter_map(|e| if let Effect::SuperSet { prop, value } = e { Some((prop.clone(), value.clone())) } else { None }).collect();
    assert_eq!(sent, vec![("Status".to_owned(), "saved".to_owned())]);
    assert!(matches!(h.cmd_set(&path(0, vec!["3"]), "x"), Err(CmdError::Bad(_))), "a subscripted write is refused, as the reason says");
}

#[test]
fn a_call_is_forwarded_with_its_arguments_and_answers_nothing() {
    let mut h = host();
    h.attach_remote_opener();
    assert_eq!(h.cmd_call(&path(1, vec!["one", "two"])).unwrap(), None);
    let sent: Vec<_> = h.effects.iter().filter_map(|e| if let Effect::SuperCall { name, args } = e { Some((name.clone(), args.clone())) } else { None }).collect();
    assert_eq!(sent, vec![("Refresh".to_owned(), vec!["one".to_owned(), "two".to_owned()])]);
}

#[test]
fn when_the_opener_has_gone_super_is_refused_for_reads_writes_and_calls() {
    let mut h = host();
    h.attach_remote_opener();
    h.remote_opener_state([("Status".to_owned(), "ready".to_owned())].into_iter().collect());
    h.remote_opener_closed();
    assert!(get(&mut h, 0).is_err(), "a read fails (the program's ON EXCEPTION)");
    assert!(matches!(h.cmd_set(&path(0, vec![]), "x"), Err(CmdError::Refused(m)) if m.contains("closed")));
    assert!(matches!(h.cmd_call(&path(1, vec![])), Err(CmdError::Refused(_))));
    assert!(h.effects.iter().all(|e| !matches!(e, Effect::SuperSet { .. } | Effect::SuperCall { .. })), "nothing is sent to a window that is gone");
    // A new state means the opener is back (a reload of the opener's page re-attaches).
    h.remote_opener_state([("Status".to_owned(), "again".to_owned())].into_iter().collect());
    assert_eq!(get(&mut h, 0).unwrap(), "again");
}

#[test]
fn the_opener_applies_what_its_child_asks_to_its_own_form_and_says_when_it_cannot() {
    let mut h = host();
    h.apply_super_set(0, "Caption", "From the child").unwrap();
    assert_eq!(h.trees[&0].get(0, "Caption"), "From the child");
    assert!(h.form_props(0).unwrap().iter().any(|(k, v)| k.eq_ignore_ascii_case("caption") && v == "From the child"), "and it is what the opener publishes next");
    h.apply_super_call(0, "Close", &[]).unwrap();
    assert!(h.close_after.contains(&0), "a window verb runs as it would for a local child");
    assert!(h.apply_super_set(9, "Caption", "x").unwrap_err().contains("closed"));
    assert!(h.apply_super_call(9, "Close", &[]).unwrap_err().contains("closed"));
}
