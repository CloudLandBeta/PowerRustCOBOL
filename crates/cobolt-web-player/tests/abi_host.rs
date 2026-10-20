// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! The ABI host against a real module (WASM-015, WASM-017 R20): the fixture module of
//! `cobolt-wasm-abi` runs on `wasmi` with [`Host`] behind its imports, on **both** variants
//! (multi-memory and single-memory). Each test states which rule of the ABI or of WASM-017 it
//! proves.

mod common;

use common::Rig;
use cobolt_wasm_abi::fixture::standard::*;
use cobolt_web_player::host::{Effect, ModuleCall, OpenMode, Outbound};

fn both(f: impl Fn(&mut Rig)) {
    for multi in [true, false] {
        let mut rig = Rig::new(multi);
        f(&mut rig);
    }
}

fn errors(rig: &Rig) -> Vec<(i32, String)> {
    rig.host
        .borrow()
        .effects
        .iter()
        .filter_map(|e| match e {
            Effect::RuntimeError(r) => Some((r.code, r.message.clone())),
            _ => None,
        })
        .collect()
}

#[test]
fn the_main_form_opens_and_its_startup_handler_writes_to_it() {
    both(|rig| {
        rig.start();
        assert_eq!(rig.host.borrow().trees.len(), 1);
        assert_eq!(rig.get(0, LBL_OUT, "Caption"), "loaded", "onLoad ran on instance 0 (multi={})", rig.multi);
        let opened: Vec<_> = rig.host.borrow().effects.iter().filter(|e| matches!(e, Effect::Opened { .. })).cloned().collect();
        assert!(matches!(opened.as_slice(), [Effect::Opened { inst: 0, form: 0, mode: OpenMode::Root, .. }]));
        assert_eq!(rig.module_live(), 1);
    });
}

#[test]
fn a_click_reaches_the_handler_and_what_it_writes_is_in_the_tree() {
    both(|rig| {
        rig.start();
        assert!(rig.click(0, "BTN-GO"));
        // UTF-8 of one to four bytes survives the trip out of the module (WASM-017 R12).
        assert_eq!(rig.get(0, LBL_OUT, "Caption"), GREETING);
        assert_eq!(rig.get(0, LST_1, "Items"), "um", "CALL LST-1::AddItem reached the control");
        let displays: Vec<String> =
            rig.host.borrow().effects.iter().filter_map(|e| if let Effect::Display(t) = e { Some(t.clone()) } else { None }).collect();
        assert_eq!(displays, ["go"]);
        assert!(errors(rig).is_empty());
    });
}

#[test]
fn an_event_without_a_bound_handler_is_not_delivered() {
    both(|rig| {
        rig.start();
        // LBL-OUT has no onClick handler: the host does not even queue it.
        assert!(!rig.click(0, LBL_OUT));
        assert_eq!(rig.host.borrow().stats.events_queued, 0);
    });
}

#[test]
fn a_query_returns_the_text_and_reflects_the_handlers_own_earlier_write() {
    both(|rig| {
        rig.start();
        {
            // The user typed into the box.
            let mut h = rig.host.borrow_mut();
            let t = h.trees.get_mut(&0).unwrap();
            let c = t.by_name(TXT_IN).unwrap();
            t.set_from_ui(c, "Text", "digitado: ação 日本語");
        }
        rig.click(0, "BTN-QUERY");
        assert_eq!(rig.get(0, LBL_OUT, "Caption"), "digitado: ação 日本語", "the answer text crossed the arena intact, both ways");
        // SET then GET in one handler: the GET sees the SET (R11).
        rig.click(0, "BTN-RAW");
        assert_eq!(rig.get(0, LBL_OUT, "Caption"), "written first");
        let s = rig.host.borrow().stats.clone();
        assert_eq!(s.queries, 2);
    });
}

#[test]
fn event_arguments_cross_the_boundary_as_utf8() {
    both(|rig| {
        rig.start();
        let ctl = rig.control(0, "BTN-ECHO");
        let sent = "Ação, 日本語, 😀";
        rig.host.borrow_mut().dom_event(0, ctl, "onClick", vec![sent.to_owned()]);
        rig.drive();
        assert_eq!(rig.get(0, LBL_OUT, "Caption"), sent);
    });
}

#[test]
fn a_thousand_writes_make_one_host_call() {
    both(|rig| {
        rig.start();
        let before = rig.host.borrow().stats.clone();
        rig.click(0, "BTN-BURST");
        let after = rig.host.borrow().stats.clone();
        assert_eq!(after.sets - before.sets, BURST_COUNT as u64, "all {BURST_COUNT} commands were applied");
        assert_eq!(after.flushes - before.flushes, 1, "WASM-017 R11: one flush for the whole handler");
        assert_eq!(after.queries - before.queries, 0);
        // 1000 writes of one property are one change for the renderer.
        let changes = rig.host.borrow_mut().take_changes();
        let caption_changes = changes.iter().filter(|(_, c)| matches!(c, cobolt_web_player::tree::Change::Prop { key, .. } if key == "caption")).count();
        assert_eq!(caption_changes, 1);
        assert_eq!(rig.get(0, LBL_OUT, "Caption"), "x");
        println!("batching: {} commands, {} flush, {} bytes in", BURST_COUNT, after.flushes - before.flushes, after.bytes_in - before.bytes_in);
    });
}

#[test]
fn a_remote_request_suspends_the_task_and_events_raised_meanwhile_run_in_order_afterwards() {
    both(|rig| {
        rig.start();
        rig.click(0, "BTN-REMOTE");
        // Suspended: the request is in the outbox, the caption says "calling".
        let reqs = rig.take_requests();
        assert_eq!(reqs.len(), 1);
        assert_eq!(std::str::from_utf8(&reqs[0].1).unwrap(), r#"{"program":"PING","payload":{}}"#);
        assert_eq!(rig.get(0, LBL_OUT, "Caption"), "calling");
        assert_eq!(rig.host.borrow().live_tasks(), 1);
        // Two more events while it waits: queued, not run (WASM-005 R9).
        assert!(rig.click(0, "BTN-GO"));
        assert!(rig.click(0, "BTN-QUERY"));
        assert_eq!(rig.get(0, LBL_OUT, "Caption"), "calling", "nothing ran behind the waiting task");
        assert_eq!(rig.host.borrow().pending_work(), 2);
        // The answer arrives.
        let answer = br#"{"id":1,"result":"pong"}"#.to_vec();
        rig.host.borrow_mut().remote_answer(reqs[0].0, answer.clone());
        rig.drive();
        // The resumed handler echoed the answer frame; then BTN-GO and BTN-QUERY ran, in order.
        let kinds: Vec<&str> = rig
            .calls
            .iter()
            .skip_while(|c| !matches!(c, ModuleCall::Resume { .. }))
            .map(|c| match c {
                ModuleCall::Resume { .. } => "resume",
                ModuleCall::Event { .. } => "event",
                _ => "other",
            })
            .collect();
        assert_eq!(kinds, ["resume", "event", "event"], "resume first, then the two queued events");
        // BTN-QUERY ran last: it set the caption to the (empty) text box content after the others.
        assert_eq!(rig.get(0, LBL_OUT, "Caption"), "");
        assert_eq!(rig.host.borrow().live_tasks(), 0);
    });
}

#[test]
fn the_answer_frame_reaches_the_module_byte_for_byte() {
    both(|rig| {
        rig.start();
        rig.click(0, "BTN-REMOTE");
        let reqs = rig.take_requests();
        let frame = "pong ação 日本".as_bytes().to_vec();
        assert!(frame.len() < 128, "the fixture echoes one-byte lengths");
        rig.host.borrow_mut().remote_answer(reqs[0].0, frame.clone());
        rig.drive();
        assert_eq!(rig.get(0, LBL_OUT, "Caption").as_bytes(), frame.as_slice());
    });
}

#[test]
fn a_very_long_loop_yields_and_is_resumed_until_it_finishes() {
    both(|rig| {
        rig.start();
        let ctl = rig.control(0, "BTN-YIELD");
        rig.host.borrow_mut().dom_event(0, ctl, "onClick", Vec::new());
        // The first slice.
        assert!(rig.step_once());
        assert!(rig.host.borrow().has_yielded(), "the budget ran out at a loop back-edge");
        assert_eq!(rig.get(0, LBL_OUT, "Caption"), "spinning");
        // Each turn the host lets it go on; count the slices.
        let mut slices = 1;
        while rig.host.borrow().has_yielded() {
            rig.host.borrow_mut().allow_yielded();
            while rig.step_once() {
                slices += 1;
            }
        }
        assert_eq!(rig.get(0, LBL_OUT, "Caption"), "spun");
        let expected = (SPIN_ITERATIONS / SPIN_SLICE) as usize;
        let resumes = rig.calls.iter().filter(|c| matches!(c, ModuleCall::Resume { .. })).count();
        assert_eq!(resumes, expected, "{slices} slices of {SPIN_SLICE} iterations");
        assert_eq!(rig.host.borrow().stats.yields as usize, expected);
        println!("yield: {SPIN_ITERATIONS} iterations in {expected} resumes of {SPIN_SLICE}");
    });
}

#[test]
fn a_trap_is_reported_and_the_application_keeps_running() {
    both(|rig| {
        rig.start();
        rig.click(0, "BTN-TRAP");
        let e = errors(rig);
        assert_eq!(e.len(), 1, "{e:?}");
        assert_eq!(e[0].0, -10);
        assert!(e[0].1.contains("trapped"), "{e:?}");
        // The host discarded the run that trapped (pr_cancel(-1)): the next handler's batch is clean.
        rig.host.borrow_mut().effects.clear();
        rig.click(0, "BTN-GO");
        assert_eq!(rig.get(0, LBL_OUT, "Caption"), GREETING);
        assert!(errors(rig).is_empty(), "no stale command from the trapped run leaked into the next batch: {:?}", errors(rig));
        assert_eq!(rig.host.borrow().stats.traps, 1);
    });
}

#[test]
fn a_cobol_runtime_error_is_reported_once_with_its_number_and_text() {
    both(|rig| {
        rig.start();
        rig.click(0, "BTN-ERR");
        let e = errors(rig);
        assert_eq!(e.len(), 1, "the trap that follows abort is not reported a second time: {e:?}");
        assert_eq!(e[0], (1001, "subscript out of range".to_owned()));
        // And the application still answers.
        rig.click(0, "BTN-GO");
        assert_eq!(rig.get(0, LBL_OUT, "Caption"), GREETING);
    });
}

#[test]
fn stop_run_ends_the_application_and_later_events_are_ignored() {
    both(|rig| {
        rig.start();
        rig.click(0, "BTN-STOP");
        assert!(rig.host.borrow().is_stopped());
        assert!(rig.host.borrow().effects.iter().any(|e| matches!(e, Effect::Stopped)));
        assert!(!rig.click(0, "BTN-GO") || rig.get(0, LBL_OUT, "Caption") == "loaded", "nothing runs after STOP RUN");
        assert_eq!(rig.get(0, LBL_OUT, "Caption"), "loaded");
    });
}

#[test]
fn a_suspension_with_nothing_to_wait_for_resumes_on_the_next_turn() {
    both(|rig| {
        rig.start();
        rig.click(0, "BTN-SUSPEND");
        assert_eq!(rig.get(0, LBL_OUT, "Caption"), "resumed", "Wait::Turn: the module is never left waiting for nothing");
    });
}

#[test]
fn instances_are_created_and_released_and_the_count_returns_to_baseline() {
    both(|rig| {
        rig.start();
        let baseline_module = rig.module_live();
        let baseline_trees = rig.host.borrow().trees.len();
        // WASM-017 AC2: open and close a form 100 times.
        for i in 0..100 {
            rig.host.borrow_mut().open_form(1, Some(0), OpenMode::Panel);
            rig.drive();
            let inst = *rig.host.borrow().trees.keys().max().unwrap();
            assert_eq!(rig.get(inst, "LBL-CHILD", "Caption"), "child", "cycle {i}: the new instance's onLoad ran");
            rig.host.borrow_mut().close_instance(inst);
            rig.drive();
        }
        assert_eq!(rig.module_live(), baseline_module, "the module's own instance count");
        assert_eq!(rig.host.borrow().trees.len(), baseline_trees, "the host's trees");
        assert_eq!(rig.host.borrow().live_tasks(), 0);
        assert_eq!(rig.host.borrow().pending_work(), 0);
        println!("disposal: 100 open/close cycles, live instances {baseline_module} -> {}", rig.module_live());
    });
}

#[test]
fn opening_more_instances_than_the_form_allows_names_the_form() {
    both(|rig| {
        rig.start();
        for _ in 0..3 {
            rig.host.borrow_mut().open_form(1, Some(0), OpenMode::Panel);
        }
        rig.host.borrow_mut().open_form(1, Some(0), OpenMode::Panel);
        rig.drive();
        assert_eq!(rig.host.borrow().trees.len(), 1 + 3);
        let e = errors(rig);
        assert_eq!(e.len(), 1, "{e:?}");
        assert_eq!(e[0].0, -3);
        assert!(e[0].1.contains("child-form") && e[0].1.contains("3 instance"), "WASM-017 R2: the error names the form: {e:?}");
    });
}

#[test]
fn a_modal_child_runs_while_the_callers_handler_waits_and_the_caller_resumes_when_it_closes() {
    both(|rig| {
        rig.start();
        rig.click(0, "BTN-MODAL");
        // The caller is suspended on its child; the child opened and ran its onLoad.
        assert_eq!(rig.get(0, LBL_OUT, "Caption"), "modal");
        assert_eq!(rig.host.borrow().trees.len(), 2, "the child instance exists");
        assert_eq!(rig.get(1, "LBL-CHILD", "Caption"), "child");
        assert!(rig.host.borrow().effects.iter().any(|e| matches!(e, Effect::Opened { inst: 1, mode: OpenMode::Modal, .. })));
        // The caller's own events wait behind its suspended task (it is blocked, as a modal says).
        assert!(rig.click(0, "BTN-GO"));
        assert_eq!(rig.get(0, LBL_OUT, "Caption"), "modal");
        // The child's handlers run meanwhile: its Close button closes it.
        assert!(rig.click(1, "BTN-CLOSE"));
        assert_eq!(rig.host.borrow().trees.len(), 1, "the child instance was destroyed");
        // The caller resumed ("after modal"), then its queued BTN-GO ran.
        assert_eq!(rig.get(0, LBL_OUT, "Caption"), GREETING);
        let resumed_before_go = {
            let resume = rig.calls.iter().position(|c| matches!(c, ModuleCall::Resume { .. }));
            let go = rig.calls.iter().rposition(|c| matches!(c, ModuleCall::Event { inst: 0, .. }));
            resume < go
        };
        assert!(resumed_before_go);
        assert_eq!(rig.module_live(), 1);
        assert_eq!(rig.host.borrow().live_tasks(), 0);
    });
}

#[test]
fn super_reaches_the_form_that_opened_this_one() {
    both(|rig| {
        rig.start();
        rig.host.borrow_mut().open_form(1, Some(0), OpenMode::Panel);
        rig.drive();
        // A hand-made batch from instance 1, ctrl SUPER, setting the opener's caption (what `super::LBL-OUT`
        // would become is a path on the opener; the control id space of `super` is the opener's form).
        let lbl_abi = rig.host.borrow().trees[&0].ctls[rig.control(0, LBL_OUT)].abi_id.unwrap();
        let caption = rig.fixture.manifest.names.iter().position(|n| n == "Caption").unwrap() as u32;
        let _ = (lbl_abi, caption);
        // `super::` itself is the opener's form object (ctrl 1 = the opener form, then its properties).
        let mut w = cobolt_wasm_abi::BatchWriter::new();
        w.set(&cobolt_wasm_abi::PathSpec::prop(1, 1, caption), "from the child");
        rig.host.borrow_mut().flush(w.as_bytes());
        assert_eq!(rig.get(0, "main-form", "Caption"), "from the child");
    });
}

#[test]
fn a_hostile_batch_is_a_readable_error_never_a_panic() {
    both(|rig| {
        rig.start();
        rig.host.borrow_mut().flush(&[0x7e, 1, 2, 3]);
        rig.host.borrow_mut().flush(&[0x01, 0xff, 0xff, 0xff, 0xff, 0xff]);
        let r = rig.host.borrow_mut().query(&[0x10]);
        assert_eq!(r, Err(-2));
        // An unknown control id and a name id out of range.
        let mut w = cobolt_wasm_abi::BatchWriter::new();
        w.set(&cobolt_wasm_abi::PathSpec::prop(0, 77, 0), "x").set(&cobolt_wasm_abi::PathSpec::prop(0, 3, 9999), "x");
        rig.host.borrow_mut().flush(w.as_bytes());
        let e = errors(rig);
        assert!(e.len() >= 4, "{e:?}");
        assert!(e.iter().all(|(c, _)| *c < 0));
        // And the host still works.
        rig.click(0, "BTN-GO");
        assert_eq!(rig.get(0, LBL_OUT, "Caption"), GREETING);
    });
}

#[test]
fn an_answer_larger_than_the_answer_region_is_refused() {
    both(|rig| {
        rig.start();
        let big = "x".repeat(70_000);
        {
            let mut h = rig.host.borrow_mut();
            let t = h.trees.get_mut(&0).unwrap();
            let c = t.by_name(TXT_IN).unwrap();
            t.set_from_ui(c, "Text", &big);
        }
        let text = rig.fixture.manifest.names.iter().position(|n| n == "Text").unwrap() as u32;
        let txt_abi = rig.host.borrow().trees[&0].ctls[rig.control(0, TXT_IN)].abi_id.unwrap();
        let mut w = cobolt_wasm_abi::BatchWriter::new();
        w.get(&cobolt_wasm_abi::PathSpec::prop(0, txt_abi, text));
        assert_eq!(rig.host.borrow_mut().query(w.as_bytes()), Err(-5), "PAYLOAD_TOO_LARGE");
    });
}

#[test]
fn teardown_releases_every_instance_and_cancels_pending_requests() {
    both(|rig| {
        rig.start();
        rig.click(0, "BTN-REMOTE"); // suspended, one pending request
        rig.host.borrow_mut().open_form(1, Some(0), OpenMode::Panel);
        rig.drive();
        assert_eq!(rig.module_live(), 2);
        let calls = rig.host.borrow_mut().teardown();
        for c in &calls {
            rig.perform(c);
        }
        assert_eq!(rig.module_live(), 0, "WASM-017 R5: nothing of the application is left in the module");
        assert!(rig.host.borrow().trees.is_empty());
        let outbox = rig.host.borrow_mut().take_outbox();
        assert!(outbox.iter().any(|o| matches!(o, Outbound::Cancel { .. })), "the pending request was cancelled: {outbox:?}");
    });
}

#[test]
fn the_player_refuses_a_form_description_that_is_not_in_the_program() {
    let mut rig = Rig::new(true);
    let alien = cobolt_forms::xml::load_form_from_str("<Form name=\"alien\" title=\"x\" width=\"1\" height=\"1\"></Form>").unwrap();
    let e = rig.host.borrow_mut().add_form(alien).unwrap_err();
    assert!(e.contains("alien") && e.contains("rebuild"), "{e}");
    assert!(rig.host.borrow().missing_forms().is_empty());
}
