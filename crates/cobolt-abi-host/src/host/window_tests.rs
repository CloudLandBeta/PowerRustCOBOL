// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! DESKTOP-WASM-001 T50: a form's window methods on the desktop. Every one goes to the window
//! supervisor, `SetFullScreen` too; `GetFormState` and `FormState` answer as the supervisor does
//! (`Ready`, or `Waiting` while the program has written `Waiting`); a form embedded in the ContentPane
//! refuses the methods that need a window of its own, in the interpreter's words, and keeps the others.
//! A page keeps its own answers.

use super::*;
use cobolt_forms::xml::load_form_from_str;
use cobolt_wasm_abi::batch::{BatchWriter, PathSpec};
use cobolt_wasm_abi::manifest::{ControlEntry, FormEntry, HandlerEntry, Limits, VariantInfo};

/// `names`: 0 SetFullScreen, 1 GetFormState, 2 FormState, 3 Focus, 4 SetBreadcrumbDetail, 5 SetTitleVisible, 6 Caption.
const NAMES: [&str; 7] = ["SetFullScreen", "GetFormState", "FormState", "Focus", "SetBreadcrumbDetail", "SetTitleVisible", "Caption"];
const ON_CLICK: u32 = 0;

/// A host with one form, `BTN-1` (id 3, `onClick` bound). On the desktop its one instance opens in
/// the window `window` names (the supervisor's handle, embedded or not).
fn host(desktop: bool, window: Option<(&str, bool)>) -> Host {
    let form = load_form_from_str(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<Form name="work-form" title="Work" width="640" height="480" index-base="1">
  <Control id="BTN-1" type="Button" x="0" y="0" w="80" h="32"><Property name="Caption">Go</Property></Control>
</Form>"#,
    )
    .expect("the form loads");
    let manifest = Manifest {
        contract: None,
        remote: Vec::new(),
        capabilities: Vec::new(),
        abi: 1,
        generator: "test".into(),
        project: "work".into(),
        variant: VariantInfo { multi_memory: true, conformance: false },
        limits: Limits::default(),
        names: NAMES.iter().map(|n| n.to_string()).collect(),
        events: ["onClick"].iter().map(|e| e.to_string()).collect(),
        forms: vec![FormEntry {
            id: 0,
            name: "work-form".into(),
            main: true,
            instances: 1,
            region_size: 64,
            controls: vec![ControlEntry { id: 3, name: "BTN-1".into(), kind: "Button".into() }],
            handlers: vec![HandlerEntry { ctrl: 3, event: ON_CLICK, entry: 0 }],
            layers: vec![],
        }],
        builtins: vec![],
        imports: vec![],
        sizes: Default::default(),
    };
    let mut h = Host::new(manifest, HostOptions { desktop, ..HostOptions::default() });
    h.add_form(form).unwrap();
    if let Some((handle, embedded)) = window {
        h.name_window(0, handle, embedded);
    }
    h.create_tree(0, 0, None, Vec::new()).expect("the tree builds");
    let _ = h.take_changes();
    h
}

/// `me::<name>(args)` answered, as `INVOKE me::GetFormState() RETURNING …` asks it.
fn answer(h: &mut Host, name: u32, args: &[&str]) -> String {
    let mut w = BatchWriter::new();
    w.call_result(&PathSpec::call(0, ctrl::FORM, name, args));
    String::from_utf8(h.query(w.as_bytes()).expect("the call answers")).unwrap()
}

/// `me::FormState`, read.
fn read_state(h: &mut Host) -> String {
    let mut w = BatchWriter::new();
    w.get(&PathSpec::prop(0, ctrl::FORM, 2));
    String::from_utf8(h.query(w.as_bytes()).expect("the read answers")).unwrap()
}

/// BTN-1's handler sends `batch`; the run ends as the module's would.
fn click(h: &mut Host, batch: &[u8]) {
    h.enqueue(0, 3, ON_CLICK, Vec::new());
    let call = h.next_call().expect("the click is delivered");
    h.flush(batch);
    let r = if h.halted() { CallResult::Trapped("run-time error".into()) } else { CallResult::Status { status: Status::Done.code(), task: 0 } };
    h.complete(&call, r);
    if let Some(c @ ModuleCall::Cancel { .. }) = h.next_call() {
        h.complete(&c, CallResult::Unit);
    }
}

/// `me::<name>(args)`, then `MOVE "after" TO BTN-1::Caption`, which the interpreter never reaches when
/// the call fails.
fn call_then_write(name: u32, args: &[&str]) -> Vec<u8> {
    let mut w = BatchWriter::new();
    w.call(&PathSpec::call(0, ctrl::FORM, name, args)).set(&PathSpec::prop(0, 3, 6), "after");
    w.as_bytes().to_vec()
}

/// The window commands the form's own window was sent.
fn window_commands(changes: &[(u32, Change)]) -> Vec<String> {
    changes.iter().filter_map(|(_, c)| if let Change::Command { ctl: 0, name, args } = c { Some(format!("{name}{args:?}")) } else { None }).collect()
}

fn fullscreen_effects(h: &Host) -> usize {
    h.effects.iter().filter(|e| matches!(e, Effect::Fullscreen(_))).count()
}

#[test]
fn window_methods_go_to_the_supervisor_and_an_embedded_form_refuses_the_ones_that_need_a_window() {
    let mut checks = 0;
    let mut check = |ok: bool, what: &str| {
        assert!(ok, "{what}");
        checks += 1;
    };

    // A form in a window of its own.
    let mut h = host(true, Some(("ROOT", false)));
    let mut w = BatchWriter::new();
    w.call(&PathSpec::call(0, ctrl::FORM, 0, &["1"]));
    h.flush(w.as_bytes());
    let sent = window_commands(&h.take_changes());
    check(sent == ["setfullscreen[\"1\"]"], &format!("SetFullScreen goes to the supervisor: {sent:?}"));
    check(fullscreen_effects(&h) == 0, "and is not a page's request to the browser");
    check(answer(&mut h, 1, &[]) == "Ready", "GetFormState answers Ready before any write");
    check(read_state(&mut h) == "Ready", "FormState reads Ready before any write");
    let mut w = BatchWriter::new();
    w.set(&PathSpec::prop(0, ctrl::FORM, 2), "Waiting");
    h.flush(w.as_bytes());
    check(answer(&mut h, 1, &[]) == "Waiting" && read_state(&mut h) == "Waiting", "Waiting once the program wrote it");
    let mut w = BatchWriter::new();
    w.set(&PathSpec::prop(0, ctrl::FORM, 2), "Ready");
    h.flush(w.as_bytes());
    check(answer(&mut h, 1, &[]) == "Ready", "and Ready again");

    // A form embedded in the ContentPane: the handler stops at the method, in the interpreter's words.
    let mut h = host(true, Some(("W1", true)));
    for (name, method) in [(0, "SETFULLSCREEN"), (3, "FOCUS"), (5, "SETTITLEVISIBLE")] {
        click(&mut h, &call_then_write(name, &["1"]));
        let words = exceptions::window_method_needs_a_window("W1", method);
        check(h.trees[&0].get(0, "LastException") == words, &format!("{method} is refused: {}", h.trees[&0].get(0, "LastException")));
        check(h.control_property(0, "BTN-1", "Caption").as_deref() == Some("Go"), &format!("nothing after the refused {method} runs"));
        check(window_commands(&h.take_changes()).iter().all(|c| !c.starts_with(&method.to_ascii_lowercase())), &format!("{method} never reaches the window"));
    }
    // The breadcrumb's detail is the embedded form's own.
    click(&mut h, &call_then_write(4, &["detail"]));
    let sent = window_commands(&h.take_changes());
    check(sent.iter().any(|c| c == "setbreadcrumbdetail[\"detail\"]"), &format!("SetBreadcrumbDetail goes to the supervisor: {sent:?}"));
    check(h.control_property(0, "BTN-1", "Caption").as_deref() == Some("after"), "and the handler goes on");
    // Released with the instance.
    h.destroy(0);
    check(h.windows.is_empty(), "the window's name goes with the instance");

    // A page: the browser's full screen, and FormState Normal.
    let mut h = host(false, None);
    let mut w = BatchWriter::new();
    w.call(&PathSpec::call(0, ctrl::FORM, 0, &["1"]));
    h.flush(w.as_bytes());
    check(fullscreen_effects(&h) == 1 && window_commands(&h.take_changes()).is_empty(), "a page asks the browser");
    check(answer(&mut h, 1, &[]) == "Normal" && read_state(&mut h) == "Normal", "a page answers Normal");
    println!(
        "── window_methods_go_to_the_supervisor_and_an_embedded_form_refuses_the_ones_that_need_a_window ──\n  SetFullScreen to the supervisor, GetFormState and FormState Ready/Waiting, SetFullScreen/Focus/SetTitleVisible refused when embedded, SetBreadcrumbDetail kept, release, the page's answers: {checks} of {checks} checks"
    );
}
