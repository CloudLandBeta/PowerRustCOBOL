// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! A desktop handler that fails (DESKTOP-WASM-001 R27, found by the AC19 pixel run): it stops at the
//! failure, as the interpreter's does, and once its run is over the failure is the form's unhandled
//! exception: `LastException`, then `onUnhandledException` when the form binds it, or else the
//! critical notice in the interpreter's words. The form goes on. A page keeps its own rule: the
//! failure is reported and the rest of the batch still applies.

use super::*;
use cobolt_forms::xml::load_form_from_str;
use cobolt_wasm_abi::batch::{BatchWriter, PathSpec};
use cobolt_wasm_abi::manifest::{ControlEntry, FormEntry, HandlerEntry, Limits, VariantInfo};

/// `names`: 0 SetProperty, 1 Caption, 2 Close.
const NAMES: [&str; 3] = ["SetProperty", "Caption", "Close"];
/// `events`: 0 onClick, 1 onLoad, 2 onUnhandledException.
const ON_CLICK: u32 = 0;
const ON_UNHANDLED: u32 = 2;

/// A host with one form, opened by nothing: `BTN-1` (id 3, `onClick` bound) and `LBL-1` (id 4). With
/// `catches` the form binds `onUnhandledException`.
fn host(desktop: bool, catches: bool) -> Host {
    let form = load_form_from_str(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<Form name="world-form" title="World" width="640" height="480" index-base="1">
  <Control id="BTN-1" type="Button" x="0" y="0" w="80" h="32"><Property name="Caption">Quit</Property></Control>
  <Control id="LBL-1" type="Label" x="0" y="40" w="80" h="32"><Property name="Caption">before</Property></Control>
</Form>"#,
    )
    .expect("the form loads");
    let mut handlers = vec![HandlerEntry { ctrl: 3, event: ON_CLICK, entry: 0 }];
    if catches {
        handlers.push(HandlerEntry { ctrl: ctrl::FORM, event: ON_UNHANDLED, entry: 1 });
    }
    let manifest = Manifest {
        contract: None,
        remote: Vec::new(),
        capabilities: Vec::new(),
        abi: 1,
        generator: "test".into(),
        project: "craft".into(),
        variant: VariantInfo { multi_memory: true, conformance: false },
        limits: Limits::default(),
        names: NAMES.iter().map(|n| n.to_string()).collect(),
        events: ["onClick", "onLoad", "onUnhandledException"].iter().map(|e| e.to_string()).collect(),
        forms: vec![FormEntry {
            id: 0,
            name: "world-form".into(),
            main: true,
            instances: 1,
            region_size: 64,
            controls: vec![
                ControlEntry { id: 3, name: "BTN-1".into(), kind: "Button".into() },
                ControlEntry { id: 4, name: "LBL-1".into(), kind: "Label".into() },
            ],
            handlers,
            layers: vec![],
        }],
        builtins: vec![],
        imports: vec![],
        sizes: Default::default(),
    };
    let mut h = Host::new(manifest, HostOptions { desktop, ..HostOptions::default() });
    h.add_form(form).unwrap();
    h
}

/// What BTN-QUIT's handler sends: `INVOKE super::SetProperty("WorldResult", "QUIT")`, then a write and a
/// `DISPLAY` that the interpreter never reaches when the first statement fails.
fn quit_batch() -> Vec<u8> {
    let mut w = BatchWriter::new();
    w.call(&PathSpec::call(0, ctrl::SUPER, 0, &["WorldResult", "QUIT"])).set(&PathSpec::prop(0, 4, 1), "after").display("after");
    w.as_bytes().to_vec()
}

fn caption(h: &Host) -> String {
    h.control_property(0, "LBL-1", "Caption").unwrap()
}

/// The form's `LastException`, as the program reads it (`me::LastException`).
fn last_exception(h: &Host) -> String {
    h.trees[&0].get(0, "LastException")
}

fn errors(h: &Host) -> Vec<String> {
    h.effects.iter().filter_map(|e| if let Effect::RuntimeError(r) = e { Some(r.message.clone()) } else { None }).collect()
}

fn displayed(h: &Host) -> usize {
    h.effects.iter().filter(|e| matches!(e, Effect::Display(_))).count()
}

/// The critical notices the window was sent: the form's ordered writes of `_CriticalException`.
fn notices(changes: &[(u32, Change)]) -> Vec<String> {
    changes
        .iter()
        .filter_map(|(_, c)| match c {
            Change::Command { ctl: 0, name, args } if name == crate::tree::ORDERED_WRITES && args.first().map(String::as_str) == Some(exceptions::CRITICAL_EXCEPTION_PROP) => {
                args.get(1).cloned()
            }
            _ => None,
        })
        .collect()
}

/// Click BTN-1 and run its handler as the module would, ending with `result`.
fn click(h: &mut Host, batch: &[u8], result: impl FnOnce(&Host) -> CallResult) -> ModuleCall {
    h.enqueue(0, 3, ON_CLICK, Vec::new());
    let call = h.next_call().expect("the click is delivered");
    assert!(matches!(call, ModuleCall::Event { event: ON_CLICK, .. }), "{call:?}");
    h.flush(batch);
    let r = result(h);
    h.complete(&call, r);
    call
}

fn trapped_if_halted(h: &Host) -> CallResult {
    if h.halted() {
        CallResult::Trapped("run-time error".into())
    } else {
        CallResult::Status { status: Status::Done.code(), task: 0 }
    }
}

#[test]
fn a_desktop_handler_stops_at_its_failure_and_the_form_shows_the_interpreters_notice() {
    let mut h = host(true, false);
    h.create_tree(0, 0, None, Vec::new()).unwrap();
    let _ = h.take_changes();
    let mut checks = 0;
    let mut check = |ok: bool, what: &str| {
        assert!(ok, "{what}");
        checks += 1;
    };
    h.enqueue(0, 3, ON_CLICK, Vec::new());
    let call = h.next_call().expect("the click is delivered");
    h.flush(&quit_batch());
    check(h.halted(), "the refused super:: halts the handler");
    check(caption(&h) == "before" && displayed(&h) == 0, "nothing after the failing statement is applied");
    h.complete(&call, CallResult::Trapped("run-time error".into()));
    check(errors(&h).is_empty(), "no run-time error line: the failure is the form's");
    check(last_exception(&h) == exceptions::SUPER_IS_NULL, "LastException holds the interpreter's words");
    let shown = notices(&h.take_changes());
    check(shown == [exceptions::critical_exception_text(exceptions::SUPER_IS_NULL)], &format!("one critical notice, the interpreter's: {shown:?}"));
    check(!h.halted(), "the next run starts clean");
    // The run that stopped is discarded, and the form goes on taking events.
    let cancel = h.next_call();
    check(matches!(cancel, Some(ModuleCall::Cancel { task: -1 })), "the cancel comes first");
    h.complete(&cancel.unwrap(), CallResult::Unit);
    let mut again = BatchWriter::new();
    again.set(&PathSpec::prop(0, 4, 1), "again");
    let call = click(&mut h, again.as_bytes(), |_| CallResult::Status { status: Status::Done.code(), task: 0 });
    check(matches!(call, ModuleCall::Event { .. }) && caption(&h) == "again", "the form still runs its handlers");
    println!(
        "── a_desktop_handler_stops_at_its_failure_and_the_form_shows_the_interpreters_notice ──\n  halt, nothing after, no error line, LastException, notice, clean next run, cancel, form goes on: {checks} of {checks} checks"
    );
}

#[test]
fn a_form_that_binds_on_unhandled_exception_runs_it_next_and_its_own_failure_is_reported() {
    let mut h = host(true, true);
    h.create_tree(0, 0, None, Vec::new()).unwrap();
    let _ = h.take_changes();
    let mut checks = 0;
    let mut check = |ok: bool, what: &str| {
        assert!(ok, "{what}");
        checks += 1;
    };
    // Something already waits behind the click: onUnhandledException goes before it.
    h.enqueue(0, 3, ON_CLICK, Vec::new());
    let call = h.next_call().expect("the click is delivered");
    h.enqueue(0, 3, ON_CLICK, Vec::new());
    h.flush(&quit_batch());
    h.complete(&call, CallResult::Trapped("run-time error".into()));
    let order: Vec<String> = h
        .work
        .iter()
        .map(|w| match w {
            Work::Cancel => "cancel".to_owned(),
            Work::Event(e) => h.manifest.events[e.event as usize].clone(),
            _ => "other".to_owned(),
        })
        .collect();
    check(order == ["cancel", "onUnhandledException", "onClick"], &format!("the handler runs next: {order:?}"));
    check(notices(&h.take_changes()).is_empty() && errors(&h).is_empty(), "no notice and no error line: the form catches it");
    check(last_exception(&h) == exceptions::SUPER_IS_NULL, "the handler reads LastException");
    // The cancel, then onUnhandledException, which fails in turn: reported, not handed to itself again.
    let cancel = h.next_call().unwrap();
    h.complete(&cancel, CallResult::Unit);
    let catch = h.next_call().unwrap();
    check(matches!(catch, ModuleCall::Event { ctrl: 0, event: ON_UNHANDLED, .. }), "onUnhandledException is the form's event");
    h.flush(&quit_batch());
    h.complete(&catch, CallResult::Trapped("run-time error".into()));
    check(errors(&h) == [exceptions::SUPER_IS_NULL], &format!("its own failure is a run-time error: {:?}", errors(&h)));
    check(!h.work.iter().any(|w| matches!(w, Work::Event(e) if e.event == ON_UNHANDLED)), "and does not run it again");
    println!(
        "── a_form_that_binds_on_unhandled_exception_runs_it_next_and_its_own_failure_is_reported ──\n  order, no notice, LastException, the form's event, its own failure reported once: {checks} of {checks} checks"
    );
}

#[test]
fn a_start_up_that_fails_shows_the_notice_on_the_form_it_opens() {
    let mut h = host(true, false);
    let mut checks = 0;
    let mut check = |ok: bool, what: &str| {
        assert!(ok, "{what}");
        checks += 1;
    };
    h.start();
    let open = h.next_call().expect("the main form opens");
    check(matches!(open, ModuleCall::OpenForm { .. }), "onLoad runs");
    // WORLD-FORM--ONLOAD: `INVOKE super::GetProperty("PlayerName") RETURNING …` in a form nothing opened.
    let mut w = BatchWriter::new();
    w.call_result(&PathSpec::call(0, ctrl::SUPER, 0, &["PlayerName"]));
    check(h.query(w.as_bytes()).is_err(), "the read fails");
    let mut w = BatchWriter::new();
    w.error(err::BAD_ARGUMENT.unsigned_abs(), "bad argument", 1, 7, 0, 1);
    h.abort(w.as_bytes());
    h.complete(&open, CallResult::Trapped("run-time error".into()));
    check(errors(&h).is_empty() && !h.halted(), "nothing yet: the instance is the cancel's to name");
    let cancel = h.next_call().unwrap();
    check(matches!(cancel, ModuleCall::Cancel { task: -1 }), "the cancel takes the form on to its event loop");
    h.complete(&cancel, CallResult::Status { status: Status::Done.code(), task: 0 });
    check(h.effects.iter().any(|e| matches!(e, Effect::Opened { inst: 0, .. })), "the form opens");
    check(errors(&h).is_empty(), "with no run-time error line");
    check(last_exception(&h) == exceptions::SUPER_IS_NULL, "the first failure is the exception, not the abort after it");
    check(notices(&h.take_changes()) == [exceptions::critical_exception_text(exceptions::SUPER_IS_NULL)], "and the notice is on its window");
    println!("── a_start_up_that_fails_shows_the_notice_on_the_form_it_opens ──\n  onLoad, failed read, abort, cancel, opened, LastException, notice: {checks} of {checks} checks");
}

#[test]
fn a_page_reports_the_failure_and_applies_the_rest_as_before() {
    let mut h = host(false, false);
    h.create_tree(0, 0, None, Vec::new()).unwrap();
    let _ = h.take_changes();
    click(&mut h, &quit_batch(), trapped_if_halted);
    let seen = (h.halted(), caption(&h), displayed(&h), errors(&h));
    assert_eq!(
        seen,
        (false, "after".to_owned(), 1, vec!["`super::` was used in a form that nothing opened (it is the first form)".to_owned()]),
        "a page's rule: reported, and the rest applies"
    );
    assert!(notices(&h.take_changes()).is_empty(), "no critical notice in a page");
    println!("── a_page_reports_the_failure_and_applies_the_rest_as_before ──\n  not halted, the write and the DISPLAY applied, one error line with the page's words, no notice: 4 of 4 checks");
}
