// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Navigation in the two application styles (WASM-011): `FormFormat` on the web (R3), what the host
//! does for an address that names no form (R6) and for a window the browser refused (R10).

use super::*;
use cobolt_forms::xml::load_form_from_str;
use cobolt_wasm_abi::manifest::{FormEntry, HandlerEntry, Limits, VariantInfo};

/// Every form binds `onRouteNotFound` (event 1) and `onPopupBlocked` (event 2), so where each is raised shows.
fn form_entry(id: u32, name: &str, main: bool) -> FormEntry {
    let handlers = vec![HandlerEntry { ctrl: ctrl::FORM, event: 1, entry: 2 * id }, HandlerEntry { ctrl: ctrl::FORM, event: 2, entry: 2 * id + 1 }];
    FormEntry { id, name: name.into(), main, instances: 4, region_size: 64, controls: vec![], handlers, layers: vec![] }
}

/// The events queued, as `(instance, event, arguments)`.
fn queued(h: &Host) -> Vec<(u32, u32, Vec<String>)> {
    h.work.iter().filter_map(|w| if let Work::Event(e) = w { Some((e.inst, e.event, e.args.clone())) } else { None }).collect()
}

/// A program of four forms: the main one, and one of each FormFormat. `with_formats` false gives every
/// description no `form-format` attribute at all (a `.cfrm` written before spec 049).
fn host_with(standalone: bool, with_formats: bool) -> Host {
    let forms = [("main-form", "Standalone"), ("embedded-form", "Embedded"), ("standalone-form", "Standalone"), ("both-form", "Both")];
    let manifest = Manifest {
        contract: None,
        remote: Vec::new(),
        abi: 1,
        generator: "test".into(),
        project: "demo".into(),
        variant: VariantInfo { multi_memory: true, conformance: false },
        limits: Limits::default(),
        names: vec![],
        events: vec!["onLoad".into(), ON_ROUTE_NOT_FOUND.into(), ON_POPUP_BLOCKED.into()],
        forms: forms.iter().enumerate().map(|(i, (n, _))| form_entry(i as u32, n, i == 0)).collect(),
        builtins: vec![],
        imports: vec![],
        sizes: Default::default(),
    };
    let mut h = Host::new(manifest, HostOptions { standalone, ..HostOptions::default() });
    for (name, format) in forms {
        let attr = if with_formats { format!(r#" form-format="{format}""#) } else { String::new() };
        let xml = format!(r#"<?xml version="1.0" encoding="UTF-8"?><Form name="{name}" title="{name}" width="320" height="200" index-base="1"{attr}></Form>"#);
        h.add_form(load_form_from_str(&xml).unwrap()).unwrap();
    }
    h
}

fn host(standalone: bool) -> Host {
    host_with(standalone, true)
}

fn queued_opens(h: &Host) -> Vec<(usize, OpenMode)> {
    h.work.iter().filter_map(|w| if let Work::Open { form, mode, .. } = w { Some((*form, *mode)) } else { None }).collect()
}

#[test]
fn nav_tests_an_embedded_form_held_in_a_data_item_is_refused_by_the_open_verbs() {
    for standalone in [false, true] {
        let mut h = host(standalone);
        h.create_tree(0, 0, None, Vec::new()).unwrap();
        for verb in ["OpenFormSync", "OpenFormAsync"] {
            let e = h.form_verb(0, verb, &["embedded-form"]).unwrap_err();
            assert!(matches!(&e, CmdError::Bad(m) if m.contains(verb) && m.contains("FormFormat Embedded")), "{e:?}");
        }
        assert!(h.work.is_empty(), "nothing was opened");
        assert!(!h.effects.iter().any(|e| matches!(e, Effect::OpenWindow { .. })), "no window was opened");
    }
}

#[test]
fn nav_tests_a_description_without_a_format_refuses_nothing() {
    // An old `.cfrm` has no `form-format`: it reads as Standalone (049 R3), which every open verb may open.
    let mut h = host_with(false, false);
    h.create_tree(0, 0, None, Vec::new()).unwrap();
    for name in ["embedded-form", "standalone-form", "both-form"] {
        assert!(h.form_verb(0, "OpenFormAsync", &[name]).is_ok(), "{name}");
    }
    // And a form whose description is not loaded yet is not judged at all.
    let bare = Host::new((*h.manifest).clone(), HostOptions::default());
    assert_eq!(bare.may_open_standalone(1), Ok(()));
    assert_eq!(bare.may_load_in_pane(2), Ok(()));
}

#[test]
fn nav_tests_a_both_or_standalone_form_opened_in_an_spa_is_a_dialog_or_a_panel_never_a_pane_load() {
    let mut h = host(false);
    h.create_tree(0, 0, None, Vec::new()).unwrap();
    h.form_verb(0, "OpenFormSync", &["both-form"]).unwrap();
    h.form_verb(0, "OpenFormAsync", &["standalone-form"]).unwrap();
    h.form_verb(0, "OpenFormAsync", &["both-form"]).unwrap();
    assert_eq!(queued_opens(&h), vec![(3, OpenMode::Modal), (2, OpenMode::Panel), (3, OpenMode::Panel)]);
}

#[test]
fn nav_tests_the_pane_takes_embedded_and_both_and_refuses_standalone() {
    let h = host(false);
    assert_eq!(h.may_load_in_pane(1), Ok(()));
    assert_eq!(h.may_load_in_pane(3), Ok(()));
    assert!(h.may_load_in_pane(2).unwrap_err().contains("FormFormat Standalone"));
    assert_eq!(h.may_open_standalone(2), Ok(()));
    assert_eq!(h.may_open_standalone(3), Ok(()));
    assert!(h.may_open_standalone(1).is_err());
}

#[test]
fn nav_tests_a_refused_menu_load_is_reported_as_a_run_time_error_under_the_form() {
    let mut h = host(false);
    h.refuse_open(2, "the menu item `customers`: no");
    let e: Vec<_> = h.effects.iter().filter_map(|e| if let Effect::RuntimeError(r) = e { Some(r.clone()) } else { None }).collect();
    assert_eq!(e.len(), 1);
    assert_eq!(e[0].code, err::HOST_REFUSED);
    assert_eq!(e[0].form.as_deref(), Some("standalone-form"));
}

#[test]
fn nav_tests_an_address_naming_no_form_raises_on_route_not_found_on_the_main_form_with_the_name() {
    let mut h = host(false);
    h.create_tree(4, 0, None, Vec::new()).unwrap();
    h.create_tree(5, 3, Some(4), Vec::new()).unwrap();
    assert!(h.route_not_found("no-such-form"));
    assert_eq!(queued(&h), vec![(4, 1, vec!["no-such-form".to_owned()])], "the main form's instance hears it, with the name the address gave");
    // No form open yet: nothing to raise it on.
    let mut empty = host(false);
    assert!(!empty.route_not_found("x"));
}

#[test]
fn nav_tests_a_refused_window_releases_the_waiting_caller_and_raises_on_popup_blocked_on_the_opener() {
    let mut h = host(true);
    h.create_tree(4, 0, None, Vec::new()).unwrap();
    h.create_tree(6, 3, None, Vec::new()).unwrap();
    // `OpenFormSync` in a standalone application: a window is asked for and the caller waits for it.
    h.inflight = Some(InFlight { inst: Some(6), opening: None, wait: None, task: None, event_desc: None });
    let handle = h.form_verb(6, "OpenFormSync", &["standalone-form"]).unwrap().flatten().expect("a handle");
    let wait = h.inflight.take().and_then(|f| f.wait).expect("the caller waits");
    assert_eq!(wait, Wait::Child { handle: handle.clone(), done: false });
    h.tasks.insert(9, Task { inst: 6, state: TaskState::Suspended(wait), closing: false, desc: None });
    assert!(!h.can_run(), "nothing to run while the window is awaited");

    // The browser refused it.
    assert!(h.popup_blocked(&handle, 6, 2));
    assert!(matches!(&h.tasks[&9].state, TaskState::Suspended(Wait::Child { done: true, .. })), "the caller is released");
    assert!(h.handles[&handle].closed, "the handle reads closed: the open failed");
    assert_eq!(queued(&h), vec![(6, 2, vec!["standalone-form".to_owned()])], "the opener hears it, with the form's name");
}

#[test]
fn nav_tests_set_unsaved_changes_marks_and_clears_the_instance_and_the_mark_goes_with_it() {
    let mut h = host(false);
    h.create_tree(4, 0, None, Vec::new()).unwrap();
    h.create_tree(5, 1, Some(4), Vec::new()).unwrap();
    assert!(!h.has_unsaved(None));
    assert_eq!(h.form_verb(5, "SetUnsavedChanges", &["1"]).unwrap(), Some(None));
    assert!(h.has_unsaved(None));
    assert!(h.has_unsaved(Some(&[5])));
    assert!(!h.has_unsaved(Some(&[4])), "only the form that said so");
    h.form_verb(5, "SETUNSAVEDCHANGES", &["0"]).unwrap();
    assert!(!h.has_unsaved(None), "0 clears it");
    h.form_verb(5, "SetUnsavedChanges", &[]).unwrap();
    assert!(h.has_unsaved(Some(&[5])), "no argument marks it");
    h.destroy(5);
    assert!(!h.has_unsaved(None), "a destroyed instance holds nothing");
}

#[test]
fn nav_tests_a_closing_window_cancels_its_waits_closes_every_form_and_abandons_a_closing_handler_that_waits() {
    // A standalone window going away (R12): what is waiting is cancelled first, then every form is closed.
    let mut h = host(true);
    h.create_tree(4, 0, None, Vec::new()).unwrap();
    h.create_tree(5, 3, Some(4), Vec::new()).unwrap();
    h.tasks.insert(9, Task { inst: 4, state: TaskState::Suspended(Wait::Timer { id: 1, done: false }), closing: false, desc: None });
    let calls = h.teardown();
    let cancel_at = calls.iter().position(|c| matches!(c, ModuleCall::Cancel { task: 9 })).expect("the waiting task is cancelled");
    let mut closed: Vec<u32> = calls.iter().filter_map(|c| if let ModuleCall::CloseForm { inst } = c { Some(*inst) } else { None }).collect();
    closed.sort();
    assert_eq!(closed, vec![4, 5], "every form's closing handlers run");
    assert!(calls.iter().position(|c| matches!(c, ModuleCall::CloseForm { .. })).unwrap() > cancel_at, "after the cancel");
    assert!(h.tasks.is_empty() && h.trees.is_empty(), "nothing is left to resume");

    // A closing handler that suspends or yields has no turn to come back in: its task is abandoned.
    for status in [Status::Suspended, Status::Yielded] {
        let r = CallResult::Status { status: status.code(), task: 12 };
        assert!(matches!(Host::abandon_on_hide(&r), Some(ModuleCall::Cancel { task: 12 })), "{status:?}");
    }
    for r in [CallResult::Status { status: Status::Done.code(), task: 0 }, CallResult::Status { status: -3, task: 0 }, CallResult::Unit, CallResult::Trapped("x".into())] {
        assert!(Host::abandon_on_hide(&r).is_none(), "finished, failed or trapped: nothing to cancel");
    }
}

#[test]
fn nav_tests_the_opener_sees_a_closed_window_as_a_closed_child() {
    // The opener's side of R12: the window's handle reads closed and an `OpenFormSync` caller is released, as for a
    // dialog closed in a single-page application — no event the desktop does not raise.
    let mut h = host(true);
    h.create_tree(4, 0, None, Vec::new()).unwrap();
    h.inflight = Some(InFlight { inst: Some(4), opening: None, wait: None, task: None, event_desc: None });
    let handle = h.form_verb(4, "OpenFormSync", &["standalone-form"]).unwrap().flatten().expect("a handle");
    let wait = h.inflight.take().and_then(|f| f.wait).expect("the caller waits");
    h.tasks.insert(9, Task { inst: 4, state: TaskState::Suspended(wait), closing: false, desc: None });
    h.child_closed(&handle);
    assert!(matches!(&h.tasks[&9].state, TaskState::Suspended(Wait::Child { done: true, .. })));
    assert!(h.handles[&handle].closed);
    assert!(queued(&h).is_empty(), "no event is invented for it");
}
