// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! The desktop's AWS controls (DESKTOP-WASM-001 T48, ABI A38): a form's `AwsS3`, `AwsLambda`,
//! `AwsMcp` and the rest, and the keys `COBOL::"AWS-SET-CREDENTIALS"` gives a connection.
//!
//! Every decision is the interpreter's own: [`door`] says whether a method is the control's, which
//! connection serves it, whether it is refused, what the request is and what its result writes,
//! and [`ops::execute`] runs it, with the connections the application published at start. A
//! synchronous operation runs on the program thread, as it runs on the interpreter's; an
//! asynchronous one runs on a thread of its own and comes back to the program thread as a message,
//! where [`Aws::delivered`] turns it into what the host carries out. A result for an operation that
//! was cancelled, or whose form closed, is dropped, as the interpreter drops a late one.

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::Arc;

use cobolt_abi_host::host::{AwsControl, AwsEffects, AwsEvent, AwsPort};
use cobolt_runtime::aws::{connections, credentials, door, ops};
use serde_json::Value;

/// An asynchronous operation's result, on its way to the program thread.
pub(crate) struct AwsResult {
    pub inst: u32,
    pub ctl: String,
    pub generation: u64,
    pub outcome: ops::Outcome,
}

/// What the program thread and the host share: the operations running and the controls' row sets.
#[derive(Default)]
struct State {
    /// `(instance, CONTROL)` → the operation running on it.
    running: HashMap<(u32, String), u64>,
    generation: u64,
    rows: HashMap<(u32, String), Vec<Value>>,
}

fn key(inst: u32, ctl: &str) -> (u32, String) {
    (inst, ctl.trim().to_ascii_uppercase())
}

/// The runner's AWS: the host's [`AwsPort`], and (a clone) the program thread's handle on the same
/// state.
#[derive(Clone)]
pub(crate) struct Aws {
    state: Rc<RefCell<State>>,
    /// Where a finished operation's result goes: the program thread's own channel, from the
    /// operation's thread.
    send: Arc<dyn Fn(AwsResult) + Send + Sync>,
}

impl Aws {
    pub(crate) fn new(send: impl Fn(AwsResult) + Send + Sync + 'static) -> Aws {
        Aws { state: Rc::default(), send: Arc::new(send) }
    }

    /// An asynchronous operation of control `ctl` reported: what the host does with it, read with the
    /// control's `Verbose` now, or `None` when the operation was cancelled or its form has closed.
    pub(crate) fn delivered(&self, r: AwsResult, verbose: bool) -> Option<AwsEffects> {
        let mut st = self.state.borrow_mut();
        let k = key(r.inst, &r.ctl);
        if st.running.get(&k) != Some(&r.generation) {
            return None;
        }
        st.running.remove(&k);
        Some(effects(&r.ctl, door::delivered(verbose, r.outcome)))
    }
}

/// The door's effects as the host takes them: each note becomes its lines, under the control's name.
fn effects(ctl: &str, fx: door::Effects) -> AwsEffects {
    AwsEffects {
        lines: fx.notes.iter().flat_map(|n| door::note_lines(ctl, n)).collect(),
        props: fx.props,
        rows: fx.rows,
        events: fx.events.into_iter().map(|e| AwsEvent { name: e.name, props: e.props, rows: e.rows }).collect(),
        answer: fx.answer,
    }
}

/// `fx`, after the request's narration.
fn after(mut request: Vec<String>, mut fx: AwsEffects) -> AwsEffects {
    request.append(&mut fx.lines);
    fx.lines = request;
    fx
}

impl AwsPort for Aws {
    fn call(&mut self, inst: u32, c: &AwsControl<'_>, method: &str, args: &[&str]) -> Option<AwsEffects> {
        let k = key(inst, c.name);
        let running = self.state.borrow().running.contains_key(&k);
        // A connection's shared server starts writable when a control of the form that names it
        // allows writes (spec 078 A4).
        let any_allows_write = |id: &str| c.peers.iter().any(|(conn, write)| *write && connections::find(conn).is_some_and(|f| f.id == id));
        let control = door::Control { class: c.class, name: c.name, prop: c.prop, any_allows_write: &any_allows_write, running };
        let texts: Vec<String> = args.iter().map(|a| (*a).to_owned()).collect();
        let run = match door::start(&control, method, &texts) {
            door::Start::NotOwn => return None,
            door::Start::Refused { sync, message } => return Some(effects(c.name, door::refused(sync, message))),
            door::Start::Run(run) => run,
        };
        let request: Vec<String> = run.request.as_ref().map(|n| door::note_lines(c.name, n)).unwrap_or_default();
        if run.sync {
            let outcome = ops::execute(&run.prepared, run.budget);
            return Some(after(request, effects(c.name, door::finished(&run.method, run.verbose, outcome))));
        }
        let generation = {
            let mut st = self.state.borrow_mut();
            st.generation += 1;
            let g = st.generation;
            st.running.insert(k, g);
            g
        };
        let send = Arc::clone(&self.send);
        let ctl = c.name.to_owned();
        // The operation's own budgets end it (a first start's, then `TimeoutMs`), and it reports a
        // timeout itself, as under the interpreter.
        std::thread::spawn(move || {
            let outcome = ops::execute(&run.prepared, run.budget);
            send(AwsResult { inst, ctl, generation, outcome });
        });
        Some(after(request, effects(c.name, door::started())))
    }

    fn row_value(&self, inst: u32, ctl: &str, i: &str, field: Option<&str>) -> String {
        door::row_value(self.state.borrow().rows.get(&key(inst, ctl)), i, field)
    }

    fn set_rows(&mut self, inst: u32, ctl: &str, rows: Vec<Value>) {
        self.state.borrow_mut().rows.insert(key(inst, ctl), rows);
    }

    fn cancel(&mut self, inst: u32, ctl: &str) -> bool {
        self.state.borrow_mut().running.remove(&key(inst, ctl)).is_some()
    }

    fn credentials(&mut self, verb: &str, args: &[&str]) -> String {
        let text = |i: usize| args.get(i).map(|a| (*a).to_owned()).unwrap_or_default();
        let connection = text(0);
        let connection = connection.trim();
        let status = |r: Result<(), String>| match r {
            Ok(()) => "OK".to_owned(),
            Err(why) => why,
        };
        match verb {
            "AWS-SET-CREDENTIALS" => status(credentials::set(connection, text(1), text(2), text(3))),
            "AWS-CLEAR-CREDENTIALS" => status(credentials::clear(connection)),
            _ => if credentials::is_set(connection) { "Y" } else { "N" }.to_owned(),
        }
    }

    fn release(&mut self, inst: u32) {
        let mut st = self.state.borrow_mut();
        st.running.retain(|(i, _), _| *i != inst);
        st.rows.retain(|(i, _), _| *i != inst);
    }
}
