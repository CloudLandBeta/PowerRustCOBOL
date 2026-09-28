// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! PowerDemo3's "Passing Data to a Child Form" example: a parent and a child
//! form hand simple values and a whole record back and forth (operator,
//! 2026-09-28). Both forms compile; and each, run against a stand-in for the
//! other, moves the data both ways — the parent publishes it with
//! `ME::"SetProperty"` and reads the answer back, the child reads it with
//! `super::"GetProperty"` and answers with `super::"SetProperty"`. The order
//! record, a group item with a table inside, arrives field by field.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use cobolt_runtime::form_host::FormRequest;
use cobolt_runtime::{FormEvent, Interpreter, StateUpdate};

fn form_path(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/PowerDemo3/forms/General").join(name)
}

fn program(form: &cobolt_forms::Form) -> cobolt_ast::program::Program {
    let src = cobolt_codegen::generate(form);
    let parsed = cobolt_parser::parse(cobolt_lexer::tokenize(&src, cobolt_lexer::SourceFormat::Free));
    let errors: Vec<String> =
        parsed.diagnostics.iter().filter(|d| d.is_error()).map(|d| format!("line {}: {}", d.span.line, d.message)).collect();
    assert!(errors.is_empty(), "{} does not parse:\n{}", form.name, errors.join("\n"));
    let program = parsed.program.expect("a program");
    let sem: Vec<String> = cobolt_semantic::analyze(&program).errors().map(|d| d.message.clone()).collect();
    assert!(sem.is_empty(), "{} fails the semantic analyser: {sem:?}", form.name);
    program
}

/// The properties every form has published, by (handle, NAME).
type Surface = Arc<Mutex<HashMap<(String, String), String>>>;

struct Run {
    events: Sender<FormEvent>,
    input: Sender<StateUpdate>,
    state: Receiver<StateUpdate>,
    seen: Vec<StateUpdate>,
    handle: Option<JoinHandle<()>>,
    closed: Arc<Mutex<bool>>,
}

impl Run {
    /// Run `file` as the form at handle `own`, its `super` at `parent` when
    /// given; `open_child` stands in for a form it opens.
    fn start(
        file: &str,
        own: &str,
        parent: Option<&str>,
        surface: Surface,
        open_child: fn(&Surface, &Sender<StateUpdate>),
    ) -> Run {
        let form = cobolt_forms::load_form(&form_path(file)).unwrap();
        let program = program(&form);
        let seed = cobolt_form_host::seeding::build_object_seed(&form, &form.controls, None, None);
        let (events, event_rx) = mpsc::channel::<FormEvent>();
        let (input, input_rx) = mpsc::channel::<StateUpdate>();
        let to_self = input.clone();
        let (state_tx, state) = mpsc::channel::<StateUpdate>();
        let (display_tx, _display) = mpsc::channel::<String>();
        let closed = Arc::new(Mutex::new(false));
        let closed_flag = closed.clone();
        let (own, parent) = (own.to_string(), parent.map(str::to_string));
        let handle = thread::spawn(move || {
            let mut interp = Interpreter::new_with_channels(program, event_rx, state_tx, display_tx);
            interp.set_input_channel(input_rx);
            interp.seed_objects(seed);
            let (req_tx, req_rx) = mpsc::channel::<FormRequest>();
            let (_closed_tx, closed_rx) = mpsc::channel::<String>();
            interp.set_form_host(req_tx, &own, "FORM", closed_rx);
            if let Some(p) = &parent {
                interp.set_super_form(p);
            }
            thread::spawn(move || {
                while let Ok(req) = req_rx.recv() {
                    match req {
                        FormRequest::HandleMethod { handle, method, args, reply } => {
                            let key = args.first().map(|k| k.trim().to_ascii_uppercase()).unwrap_or_default();
                            let mut s = surface.lock().unwrap();
                            let out = match method.to_ascii_uppercase().as_str() {
                                "GETPROPERTY" => s.get(&(handle, key)).cloned().unwrap_or_default(),
                                "SETPROPERTY" => {
                                    s.insert((handle, key), args.get(1).cloned().unwrap_or_default());
                                    String::new()
                                }
                                _ => String::new(),
                            };
                            let _ = reply.send(Ok(out));
                        }
                        FormRequest::PublishFormProps { handle, props } => {
                            let mut s = surface.lock().unwrap();
                            for (k, v) in props {
                                s.insert((handle.clone(), k.to_ascii_uppercase()), v);
                            }
                        }
                        FormRequest::OpenForm { reply, .. } => {
                            open_child(&surface, &to_self);
                            let _ = reply.send(None);
                        }
                        FormRequest::CloseSelf { .. } => *closed_flag.lock().unwrap() = true,
                        _ => {}
                    }
                }
            });
            let _ = interp.run();
        });
        Run { events, input, state, seen: Vec::new(), handle: Some(handle), closed }
    }

    fn wait_for(&mut self, ctrl: &str, prop: &str, ok: impl Fn(&str) -> bool) -> String {
        let deadline = Instant::now() + Duration::from_secs(20);
        loop {
            if let Some(u) =
                self.seen.iter().rev().find(|u| u.ctrl_id.eq_ignore_ascii_case(ctrl) && u.prop.eq_ignore_ascii_case(prop) && ok(&u.value))
            {
                return u.value.clone();
            }
            let left = deadline.saturating_duration_since(Instant::now());
            assert!(!left.is_zero(), "timed out waiting for {ctrl}::{prop}; saw {:#?}", self.seen);
            if let Ok(u) = self.state.recv_timeout(left.min(Duration::from_millis(200))) {
                self.seen.push(u);
            }
        }
    }

    fn quit(mut self) {
        let _ = self.events.send(FormEvent::quit());
        if let Some(h) = self.handle.take() {
            let _ = h.join();
        }
    }
}

/// ORD-NUMBER 9(6), ORD-DATE X(10), then three lines of ITEM X(20),
/// QTY 9(3), PRICE 9(5)V99.
fn order(qty: [u32; 3]) -> String {
    let items = [("Office chair", 64990), ("Standing desk", 189000), ("Monitor arm", 21950)];
    let mut s = String::from("1042172026-09-28");
    for ((item, price), q) in items.iter().zip(qty) {
        s.push_str(&format!("{item:<20}{q:03}{price:07}"));
    }
    s
}

#[test]
fn the_child_reads_the_parent_and_sends_the_edited_record_back() {
    let t = Instant::now();
    let surface: Surface = Arc::default();
    {
        let mut s = surface.lock().unwrap();
        for (k, v) in [
            ("TITLE", "Passing Data to a Child Form".to_string()),
            ("CUSTOMERNAME", "Maria Silva".into()),
            ("CREDITLIMIT", "50000".into()),
            ("ISVIP", "Y".into()),
            ("ORDERRECORD", order([2, 1, 3])),
        ] {
            s.insert(("W0".into(), k.into()), v);
        }
    }
    let mut child = Run::start("props-child-form.cfrm", "W1", Some("W0"), surface.clone(), |_, _| {});
    let name = child.wait_for("Txt-Name", "Text", |v| !v.trim().is_empty());
    let from = child.wait_for("Lbl-From", "Caption", |v| !v.trim().is_empty());
    let limit = child.wait_for("Lbl-Limit-Value", "Caption", |v| !v.trim().is_empty());
    let item2 = child.wait_for("Lbl-Item-2", "Caption", |v| !v.trim().is_empty());
    let qty3 = child.wait_for("Txt-Qty-3", "Text", |v| !v.trim().is_empty());
    let total = child.wait_for("Lbl-Total", "Caption", |v| v.contains("Total"));
    assert_eq!(name.trim(), "Maria Silva", "a simple value, read from the parent");
    assert!(from.contains("Passing Data to a Child Form"), "super::Title reads bare: {from}");
    assert_eq!(limit.trim(), "50000");
    assert_eq!(item2.trim(), "Standing desk", "the record's table, field by field");
    assert_eq!(qty3.trim(), "3");
    assert!(total.contains("3,848.30"), "2 x 649.90 + 1 x 1,890.00 + 3 x 219.50: {total}");

    // The operator edits: a new name, 4 desks instead of 1; OK.
    child.input.send(StateUpdate::new("Txt-Name", "Text", "Maria S. Costa")).unwrap();
    child.input.send(StateUpdate::new("Txt-Qty-2", "Text", "4")).unwrap();
    child.events.send(FormEvent::click("Btn-Ok")).unwrap();
    let deadline = Instant::now() + Duration::from_secs(20);
    while !*child.closed.lock().unwrap() {
        assert!(Instant::now() < deadline, "the child closes itself after OK");
        thread::sleep(Duration::from_millis(20));
    }
    let closed = child.closed.clone();
    child.quit();
    let s = surface.lock().unwrap();
    let get = |k: &str| s.get(&("W0".to_string(), k.to_string())).cloned().unwrap_or_default();
    assert_eq!(get("CHILDRESULT"), "OK");
    assert_eq!(get("CUSTOMERNAME").trim(), "Maria S. Costa", "the edited simple value, back on the parent");
    assert_eq!(get("ORDERRECORD").trim_end(), order([2, 4, 3]).trim_end(), "the whole record back, quantity 2 edited");
    println!(
        "\n  ── PowerDemo3, child form ──\n  read: name, limit, VIP, a {}-byte order record; sent back: name, record, OK; closed {} — {:.0} ms\n",
        order([2, 1, 3]).len(),
        *closed.lock().unwrap(),
        t.elapsed().as_secs_f64() * 1000.0
    );
}

#[test]
fn the_parent_publishes_the_data_and_reads_the_answer() {
    let t = Instant::now();
    let surface: Surface = Arc::default();
    // What a child does while the parent waits in OpenFormSync: read the
    // record, change a quantity and the name, answer OK. Each
    // `super::"SetProperty"` reaches the parent's own form object, as the
    // host forwards it.
    fn child(surface: &Surface, parent: &Sender<StateUpdate>) {
        for (k, v) in [
            ("OrderRecord", order([5, 1, 3])),
            ("CustomerName", "Maria S. Costa".to_string()),
            ("ChildResult", "OK".to_string()),
        ] {
            parent.send(StateUpdate::new("FORM", k, &v)).unwrap();
        }
        let mut s = surface.lock().unwrap();
        let rec = s.get(&("W0".to_string(), "ORDERRECORD".to_string())).cloned().unwrap_or_default();
        assert_eq!(rec.trim_end(), order([2, 1, 3]).trim_end(), "the parent published the whole record");
        assert_eq!(s.get(&("W0".to_string(), "ISVIP".to_string())).map(String::as_str), Some("Y"));
        s.insert(("W0".into(), "ORDERRECORD".into()), order([5, 1, 3]));
        s.insert(("W0".into(), "CUSTOMERNAME".into()), "Maria S. Costa".into());
        s.insert(("W0".into(), "CHILDRESULT".into()), "OK".into());
    }
    let mut parent = Run::start("props-parent-form.cfrm", "W0", None, surface.clone(), child);
    let first = parent.wait_for("Lbl-Line-1", "Caption", |v| !v.trim().is_empty());
    assert!(first.contains("2 x Office chair"), "{first}");
    parent.events.send(FormEvent::click("Btn-Edit")).unwrap();
    let status = parent.wait_for("Lbl-Status", "Caption", |v| v.contains("sent"));
    let line1 = parent.wait_for("Lbl-Line-1", "Caption", |v| v.contains("5 x"));
    let name = parent.wait_for("Txt-Name", "Text", |v| v.contains("Costa"));
    let order_line = parent.wait_for("Lbl-Order", "Caption", |v| v.contains("total"));
    parent.quit();
    assert!(line1.contains("5 x Office chair") && line1.contains("3,249.50"), "{line1}");
    assert_eq!(name.trim(), "Maria S. Costa");
    assert!(order_line.contains("5,798.00"), "5 x 649.90 + 1,890.00 + 3 x 219.50: {order_line}");
    println!(
        "\n  ── PowerDemo3, parent form ──\n  published 5 properties; read back {status:?}; {order_line:?} — {:.0} ms\n",
        t.elapsed().as_secs_f64() * 1000.0
    );
}
