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
        contract: None,
        remote: Vec::new(),
        abi: 1,
        generator: "test".into(),
        project: "demo".into(),
        variant: VariantInfo { multi_memory: true, conformance: false },
        limits: Limits::default(),
        names: vec!["Status".into(), "Refresh".into(), "Lines".into()],
        events: vec!["onLoad".into()],
        forms: vec![FormEntry { id: 0, name: "main-form".into(), main: true, instances: 4, region_size: 64, controls: vec![], handlers: vec![], layers: vec![] }],
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

// ── reads that wait (ABI A24, WASM-011 T17) ─────────────────────────────────

/// A window whose opener answers by id, with the call of an event in flight on instance 0.
fn waiting_host() -> Host {
    let mut h = host();
    h.attach_remote_opener();
    h.remote_opener_waits(true);
    h.remote_opener_state([("Status".to_owned(), "stale".to_owned())].into_iter().collect());
    h.inflight = Some(InFlight { inst: Some(0), opening: None, wait: None, task: None, event_desc: None });
    h
}

fn get_wait(h: &mut Host, name: u32, args: Vec<&str>) -> Result<String, i32> {
    h.eval_query(&Command::GetWait(path(name, args)))
}

/// The call ends `SUSPENDED`: what it waits for becomes the task's wait, as `complete` does.
fn suspend(h: &mut Host, task: i32) {
    let wait = h.inflight.take().and_then(|f| f.wait).expect("the call waits");
    h.tasks.insert(task, Task { inst: 0, state: TaskState::Suspended(wait), closing: false, desc: None });
}

/// The task is resumed: the call in flight is its.
fn resume(h: &mut Host, task: i32) {
    h.inflight = Some(InFlight { inst: Some(0), opening: None, wait: None, task: Some(task), event_desc: None });
}

#[test]
fn a_waiting_read_asks_the_opener_suspends_and_is_answered_from_the_reply_not_the_replica() {
    let mut h = waiting_host();
    assert_eq!(get_wait(&mut h, 0, vec![]), Err(err::SUSPENDED_QUERY), "on its way, not the replica's `stale`");
    assert!(h.effects.contains(&Effect::SuperGet { id: 1, prop: "Status".into() }));
    assert_eq!(h.inflight.as_ref().unwrap().wait, Some(Wait::Super { ids: vec![1], done: false }));
    assert_eq!(h.shape_answer(Some(Err(err::SUSPENDED_QUERY))), Err(err::SUSPENDED_QUERY));
    assert_eq!(h.last_error, err::SUSPENDED_QUERY, "the page's glue gives the module `last_error()` when the query has no bytes: it has to say 'on its way'");
    // Asked again before the answer (a second look in the same pass): still waiting, nothing sent twice.
    assert_eq!(get_wait(&mut h, 0, vec![]), Err(err::SUSPENDED_QUERY));
    assert_eq!(h.effects.iter().filter(|e| matches!(e, Effect::SuperGet { .. })).count(), 1);
    suspend(&mut h, 9);
    assert!(!h.can_run(), "nothing to run while the opener has not answered");
    assert!(h.has_waiting());

    assert!(h.super_reply(1, Ok("fresh".into())));
    assert!(matches!(&h.tasks[&9].state, TaskState::Suspended(Wait::Super { done: true, .. })));
    assert!(h.can_run(), "the answer releases the task");
    resume(&mut h, 9);
    assert_eq!(get_wait(&mut h, 0, vec![]), Ok("fresh".into()), "the opener's answer");
    assert_eq!(get_wait(&mut h, 0, vec![]), Ok("fresh".into()), "the same ask in the same run is answered the same");
    assert_eq!(h.remote_opener.as_ref().unwrap().props["status"], "stale", "the replica stays what it was: a cache, not the answer");
    h.complete(&ModuleCall::Resume { task: 9, frame: None }, CallResult::Status { status: 0, task: 0 });
    assert!(h.remote_opener.as_ref().unwrap().reqs.is_empty(), "the run is over: its answers are not reused by the next");
    h.inflight = Some(InFlight { inst: Some(0), opening: None, wait: None, task: None, event_desc: None });
    assert_eq!(get_wait(&mut h, 0, vec![]), Err(err::SUSPENDED_QUERY), "the next read of it asks again");
    assert!(h.effects.contains(&Effect::SuperGet { id: 2, prop: "Status".into() }));
    eprintln!("waiting read: asked once, suspended, answered `fresh` (replica `stale` untouched), answered again within the run, forgotten when the run ended, asked afresh by the next");
}

#[test]
fn a_waiting_read_ends_in_the_error_path_when_the_opener_does_not_answer_in_time_or_goes_away() {
    // A timeout.
    let mut h = waiting_host();
    assert_eq!(get_wait(&mut h, 0, vec![]), Err(err::SUSPENDED_QUERY));
    suspend(&mut h, 9);
    assert!(h.super_timeout(1));
    assert!(h.can_run(), "a timeout releases the task");
    resume(&mut h, 9);
    assert_eq!(get_wait(&mut h, 0, vec![]), Err(err::HOST_REFUSED), "the read fails: the program's ON EXCEPTION");
    assert!(h.effects.iter().any(|e| matches!(e, Effect::RuntimeError(r) if r.message.contains("did not answer in time"))), "and the report says why");
    assert!(!h.super_timeout(1), "a request that is already settled is not settled twice");

    // The opener's window closes while the task waits.
    let mut h = waiting_host();
    assert_eq!(get_wait(&mut h, 0, vec![]), Err(err::SUSPENDED_QUERY));
    suspend(&mut h, 9);
    h.remote_opener_closed();
    assert!(h.can_run(), "`closed` releases the task");
    resume(&mut h, 9);
    assert_eq!(get_wait(&mut h, 0, vec![]), Err(err::HOST_REFUSED));
    assert!(h.effects.iter().any(|e| matches!(e, Effect::RuntimeError(r) if r.message.contains("closed"))));
    // And a read made after it closed is not even sent.
    h.effects.clear();
    assert_eq!(get_wait(&mut h, 0, vec![]), Err(err::HOST_REFUSED));
    assert!(h.effects.iter().all(|e| !matches!(e, Effect::SuperGet { .. })), "nothing is sent to a window that is gone");
}

#[test]
fn an_opener_that_does_not_answer_by_id_and_a_window_with_no_opener_are_read_as_before() {
    // An older opener: the replica answers at once, nothing is asked, nothing waits.
    let mut h = host();
    h.attach_remote_opener();
    h.remote_opener_state([("Status".to_owned(), "ready".to_owned())].into_iter().collect());
    h.inflight = Some(InFlight { inst: Some(0), opening: None, wait: None, task: None, event_desc: None });
    assert_eq!(get_wait(&mut h, 0, vec![]), Ok("ready".into()));
    assert!(h.effects.iter().all(|e| !matches!(e, Effect::SuperGet { .. })));
    assert_eq!(h.inflight.as_ref().unwrap().wait, None);
    // No opener window at all: the plain read's answer (a refusal), not a wait.
    let mut h = host();
    h.inflight = Some(InFlight { inst: Some(0), opening: None, wait: None, task: None, event_desc: None });
    let r = get_wait(&mut h, 0, vec![]);
    assert!(r.is_err() && r != Err(err::SUSPENDED_QUERY), "{r:?}");
}

#[test]
fn a_waiting_method_is_asked_with_its_arguments_and_two_reads_in_one_pass_wait_together() {
    let mut h = waiting_host();
    assert_eq!(h.eval_query(&Command::CallRWait(path(1, vec!["one", "two"]))), Err(err::SUSPENDED_QUERY));
    assert!(h.effects.contains(&Effect::SuperCallR { id: 1, name: "Refresh".into(), args: vec!["one".into(), "two".into()] }));
    assert_eq!(get_wait(&mut h, 2, vec!["2"]), Err(err::SUSPENDED_QUERY), "a subscripted read asks for the whole property");
    assert!(h.effects.contains(&Effect::SuperGet { id: 2, prop: "Lines".into() }));
    assert_eq!(h.inflight.as_ref().unwrap().wait, Some(Wait::Super { ids: vec![1, 2], done: false }), "one wait for both");
    suspend(&mut h, 9);
    assert!(h.super_reply(1, Ok("done".into())));
    assert!(!h.can_run(), "one answer is not enough");
    assert!(h.super_reply(2, Ok("a\nb\nc".into())));
    assert!(h.can_run());
    resume(&mut h, 9);
    assert_eq!(h.eval_query(&Command::CallRWait(path(1, vec!["one", "two"]))), Ok("done".into()));
    assert_eq!(get_wait(&mut h, 2, vec!["2"]), Ok("b".into()), "the subscript picks the second line of the answer");
    assert_eq!(h.eval_query(&Command::CallRWait(path(1, vec!["other"]))), Err(err::SUSPENDED_QUERY), "the same method with other arguments is another read");
}

#[test]
fn the_opener_answers_a_read_and_a_call_from_its_own_form_and_says_when_it_cannot() {
    let mut h = host();
    h.apply_super_set(0, "Caption", "From the child").unwrap();
    assert_eq!(h.apply_super_get(0, "Caption"), Ok("From the child".into()));
    assert_eq!(h.apply_super_get(0, "Instance"), Ok("0".into()), "the form's own properties, as the form reads them");
    assert_eq!(h.apply_super_call_result(0, "Close", &[]), Ok(String::new()));
    assert!(h.close_after.contains(&0));
    assert!(h.apply_super_get(9, "Caption").unwrap_err().contains("closed"));
    assert!(h.apply_super_call_result(9, "Close", &[]).unwrap_err().contains("closed"));
}

// ── a read made by a statement that has ON EXCEPTION (ABI A25, WASM-011 T17) ─

fn get_soft(h: &mut Host, name: u32) -> Result<String, i32> {
    h.eval_query(&Command::GetWaitSoft(path(name, vec![])))
}

fn run_errors(h: &Host) -> usize {
    h.effects.iter().filter(|e| matches!(e, Effect::RuntimeError(_))).count()
}

#[test]
fn a_guarded_read_waits_and_is_answered_like_a_plain_waiting_read() {
    let mut h = waiting_host();
    assert_eq!(get_soft(&mut h, 0), Err(err::SUSPENDED_QUERY), "the opener is asked by id and the handler waits, exactly as for the plain read");
    assert!(h.effects.contains(&Effect::SuperGet { id: 1, prop: "Status".into() }));
    suspend(&mut h, 9);
    assert!(h.super_reply(1, Ok("fresh".into())));
    resume(&mut h, 9);
    assert_eq!(get_soft(&mut h, 0), Ok("fresh".into()), "the opener's answer goes to NOT ON EXCEPTION");
    assert_eq!(run_errors(&h), 0);
    // The method form takes the same road.
    let mut h = waiting_host();
    assert_eq!(h.eval_query(&Command::CallRWaitSoft(path(1, vec!["a"]))), Err(err::SUSPENDED_QUERY));
    assert!(h.effects.contains(&Effect::SuperCallR { id: 1, name: "Refresh".into(), args: vec!["a".into()] }));
}

#[test]
fn a_guarded_read_that_fails_answers_the_code_and_reports_nothing_where_the_plain_one_reports() {
    for (how, end) in [("times out", 0u8), ("is closed under it", 1), ("is refused by the opener", 2)] {
        let mut soft = waiting_host();
        let mut hard = waiting_host();
        // The first ask is the guarded one for `soft` and the plain one for `hard`: the same wait either way.
        assert_eq!(get_soft(&mut soft, 0), Err(err::SUSPENDED_QUERY), "{how}");
        assert_eq!(get_wait(&mut hard, 0, vec![]), Err(err::SUSPENDED_QUERY), "{how}");
        suspend(&mut soft, 9);
        suspend(&mut hard, 9);
        for h in [&mut soft, &mut hard] {
            match end {
                0 => assert!(h.super_timeout(1)),
                1 => h.remote_opener_closed(),
                _ => assert!(h.super_reply(1, Err("no such method".into()))),
            }
            resume(h, 9);
        }
        assert_eq!(get_soft(&mut soft, 0), Err(err::HOST_REFUSED), "the code, for the clause to see ({how})");
        assert_eq!(run_errors(&soft), 0, "nothing reaches the page: the program handles it ({how})");
        assert_eq!(get_wait(&mut hard, 0, vec![]), Err(err::HOST_REFUSED), "the plain read fails the same way ({how})");
        assert_eq!(run_errors(&hard), 1, "but it is a run-time error with its reason ({how})");
    }
}

#[test]
fn a_guarded_read_after_the_opener_has_gone_is_not_sent_and_reports_nothing_and_quiet_mode_does_not_stick() {
    let mut h = waiting_host();
    h.remote_opener_closed();
    h.effects.clear();
    assert_eq!(get_soft(&mut h, 0), Err(err::HOST_REFUSED));
    assert_eq!(h.eval_query(&Command::CallRWaitSoft(path(1, vec![]))), Err(err::HOST_REFUSED));
    assert!(h.effects.iter().all(|e| !matches!(e, Effect::SuperGet { .. } | Effect::SuperCallR { .. })), "nothing is sent to a window that is gone");
    assert_eq!(run_errors(&h), 0);
    // A plain read right after still reports: the quiet mode belongs to the one query.
    assert_eq!(get_wait(&mut h, 0, vec![]), Err(err::HOST_REFUSED));
    assert_eq!(run_errors(&h), 1, "the next plain read is reported as always");
    // No opener window at all: the guarded read is a refusal for the clause, not a report.
    let mut h = host();
    h.inflight = Some(InFlight { inst: Some(0), opening: None, wait: None, task: None, event_desc: None });
    let r = get_soft(&mut h, 0);
    assert!(r.is_err() && r != Err(err::SUSPENDED_QUERY), "{r:?}");
    assert_eq!(run_errors(&h), 0);
}
