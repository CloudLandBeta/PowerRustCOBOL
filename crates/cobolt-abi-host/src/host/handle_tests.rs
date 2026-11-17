// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! A window handle as a receiver (ABI A40, spec 037 R23, DESKTOP-WASM-001 T39c): a call or a read through
//! the windowHandler an `OpenFormAsync … RETURNING WS-WORK` filled reaches the form of the window it names,
//! as `super::` reaches the opener's; once that window is closed, or for a handle the host never gave, it is
//! refused in the interpreter's words.

use super::*;
use cobolt_forms::xml::load_form_from_str;
use cobolt_wasm_abi::batch::{BatchWriter, Path, PathSpec, Seg};
use cobolt_wasm_abi::manifest::{FormEntry, Limits, VariantInfo};

/// `names`: 0 OpenFormAsync, 1 Close, 2 WS-WORK, 3 Title, 4 SetTitle.
const OPEN: u32 = 0;
const CLOSE: u32 = 1;
const WS_WORK: u32 = 2;
const TITLE: u32 = 3;
const SET_TITLE: u32 = 4;

/// Two forms in a page: the main form (instance 0, open) and `work-form`, which it opens.
fn host() -> Host {
    let form = |id: u32, name: &str, main: bool| FormEntry {
        id,
        name: name.into(),
        main,
        instances: 1,
        region_size: 64,
        controls: vec![],
        handlers: vec![],
        layers: vec![],
    };
    let manifest = Manifest {
        contract: None,
        remote: Vec::new(),
        capabilities: Vec::new(),
        abi: 1,
        generator: "test".into(),
        project: "handles".into(),
        variant: VariantInfo { multi_memory: true, conformance: false },
        limits: Limits::default(),
        names: ["OpenFormAsync", "Close", "WS-WORK", "Title", "SetTitle"].iter().map(|n| n.to_string()).collect(),
        events: vec!["onLoad".into()],
        forms: vec![form(0, "main-form", true), form(1, "work-form", false)],
        builtins: vec![],
        imports: vec![],
        sizes: Default::default(),
    };
    let mut h = Host::new(manifest, HostOptions::default());
    for (name, title) in [("main-form", "Main"), ("work-form", "Working")] {
        let xml = format!(r#"<?xml version="1.0" encoding="UTF-8"?><Form name="{name}" title="{title}" width="320" height="200" index-base="1"></Form>"#);
        h.add_form(load_form_from_str(&xml).unwrap()).unwrap();
    }
    h.create_tree(0, 0, None, Vec::new()).unwrap();
    h
}

/// `INVOKE ME::OpenFormAsync("work-form") RETURNING WS-WORK`, and the open carried out: the handle.
fn open_work(h: &mut Host) -> String {
    let handle = h.cmd_call(&Path { inst: 0, ctrl: ctrl::FORM, segs: vec![Seg { name: OPEN, args: vec!["work-form"] }] }).unwrap().unwrap();
    let call = h.next_call().expect("the open is the next call");
    assert!(matches!(call, ModuleCall::OpenForm { form: 1, .. }), "{call:?}");
    h.complete(&call, CallResult::Opened { inst: 1, status: Status::Done.code(), task: 0 });
    handle
}

/// The path of `WS-WORK::name(args…)`, the windowHandler holding `handle` (ABI A40).
fn through(handle: &str, name: u32, args: &[&str]) -> PathSpec {
    PathSpec::call(0, ctrl::HANDLE, WS_WORK, &[handle]).then(name, args)
}

fn read(h: &mut Host, path: &PathSpec) -> Result<String, i32> {
    let mut w = BatchWriter::new();
    w.get(path);
    h.query(w.as_bytes()).map(|b| String::from_utf8_lossy(&b).into_owned())
}

fn errors(h: &Host) -> Vec<String> {
    h.effects.iter().filter_map(|e| if let Effect::RuntimeError(r) = e { Some(r.message.clone()) } else { None }).collect()
}

#[test]
fn a_call_through_a_window_handle_reaches_the_form_of_that_window_and_a_closed_one_is_refused() {
    let mut checks = 0;
    let mut check = |ok: bool, what: &str| {
        assert!(ok, "{what}");
        checks += 1;
    };
    let mut h = host();
    let handle = open_work(&mut h);
    check(h.trees.contains_key(&1), "the opened form has its instance");
    check(read(&mut h, &through(&handle, TITLE, &[])).as_deref() == Ok("Working"), "a read through the handle is the opened form's");
    let mut w = BatchWriter::new();
    w.call(&through(&handle, SET_TITLE, &["Still working"]));
    h.flush(w.as_bytes());
    check(h.trees[&1].get(0, "Title") == "Still working", "a call through the handle reaches the opened form");
    check(h.trees[&0].get(0, "Title") == "Main", "and not the form that holds the handle");
    let mut w = BatchWriter::new();
    w.call(&through(&handle, CLOSE, &[]));
    h.flush(w.as_bytes());
    check(h.close_after.contains(&1) && !h.close_after.contains(&0), "Close closes the window the handle names");
    check(errors(&h).is_empty(), "no run-time error while the window is open");
    // The window has closed: the handle is NULL, as the interpreter's windowHandler becomes.
    h.child_closed(&handle);
    let mut w = BatchWriter::new();
    w.call(&through(&handle, CLOSE, &[]));
    h.flush(w.as_bytes());
    let want = exceptions::window_handle_is_null("WS-WORK");
    check(errors(&h) == [want.clone()], &format!("a call through a closed window's handle is refused in the interpreter's words: {:?}", errors(&h)));
    check(read(&mut h, &through(&handle, TITLE, &[])).is_err(), "so is a read");
    // A handle the host never gave (the item was never filled) is refused the same way.
    let mut w = BatchWriter::new();
    w.call(&through("", CLOSE, &[]));
    h.flush(w.as_bytes());
    check(errors(&h).last() == Some(&want), "an empty handle names no window");
    check(h.trees[&0].get(0, "Title") == "Main", "the holder's form is never the target");
    println!(
        "── a_call_through_a_window_handle_reaches_the_form_of_that_window_and_a_closed_one_is_refused (ABI A40) ──\n  \
         open, read, call, holder untouched, Close, no error, closed refused (call, read), empty refused, holder untouched: {checks} of {checks} checks"
    );
}
