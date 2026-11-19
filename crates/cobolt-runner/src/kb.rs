// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! The desktop's KnowledgeBase control (DESKTOP-WASM-001 T47b, ABI A49).
//!
//! Every decision is the interpreter's own ([`kb_door`]): which methods are the control's, what
//! each answers, what an operation is, and what each of its reports raises and writes. An
//! operation runs on a thread of its own and reports back to the program thread as messages, where
//! [`Kb::delivered`] turns each into an event the host queues. A report from an operation that was
//! cancelled, or whose form closed, is dropped, as the interpreter drops a stale one.

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use cobolt_abi_host::host::{KbControl, KbEffects, KbEvent, KbPort};
use cobolt_runtime::kb_door;

/// A running operation's report, on its way to the program thread.
pub(crate) struct KbNews {
    pub inst: u32,
    pub ctl: String,
    pub generation: u64,
    pub report: kb_door::Report,
}

/// What the program thread and the host share.
#[derive(Default)]
struct State {
    /// `(instance, CONTROL)` → the operation running on it, and its cancel flag.
    running: HashMap<(u32, String), (u64, Arc<AtomicBool>)>,
    generation: u64,
    /// What each control keeps between calls.
    kept: HashMap<(u32, String), kb_door::State>,
}

fn key(inst: u32, ctl: &str) -> (u32, String) {
    (inst, ctl.trim().to_ascii_uppercase())
}

/// The runner's Knowledge Base: the host's [`KbPort`], and (a clone) the program thread's handle on
/// the same state.
#[derive(Clone)]
pub(crate) struct Kb {
    state: Rc<RefCell<State>>,
    /// Where a report goes: the program thread's own channel, from the operation's thread.
    send: Arc<dyn Fn(KbNews) + Send + Sync>,
}

impl Kb {
    pub(crate) fn new(send: impl Fn(KbNews) + Send + Sync + 'static) -> Kb {
        Kb { state: Rc::default(), send: Arc::new(send) }
    }

    /// An operation of control `ctl` reported: the event the host queues, or `None` when the
    /// operation was cancelled or its form has closed. A report that ends the operation frees the
    /// control for the next one.
    pub(crate) fn delivered(&self, n: KbNews) -> Option<KbEvent> {
        let mut st = self.state.borrow_mut();
        let k = key(n.inst, &n.ctl);
        if st.running.get(&k).map(|(g, _)| *g) != Some(n.generation) {
            return None;
        }
        if n.report.is_final() {
            st.running.remove(&k);
        }
        drop(st);
        let e = kb_door::delivered(n.report)?;
        Some(KbEvent { name: e.name, props: e.props, results: e.results.as_deref().map(kb_door::result_rows) })
    }
}

impl KbPort for Kb {
    fn call(&mut self, inst: u32, c: &KbControl<'_>, method: &str, args: &[&str]) -> Option<KbEffects> {
        let k = key(inst, c.name);
        let texts: Vec<String> = args.iter().map(|a| (*a).to_owned()).collect();
        let (running, mut kept) = {
            let mut st = self.state.borrow_mut();
            (st.running.contains_key(&k), st.kept.remove(&k).unwrap_or_default())
        };
        let call = kb_door::call(c.prop, &mut kept, running, method, &texts);
        self.state.borrow_mut().kept.insert(k.clone(), kept);
        match call {
            kb_door::Call::NotOwn | kb_door::Call::Cancel => None,
            kb_door::Call::Done { props, answer } => Some(KbEffects { props, answer }),
            kb_door::Call::Start { job, props } => {
                let cancel = Arc::new(AtomicBool::new(false));
                let generation = {
                    let mut st = self.state.borrow_mut();
                    st.generation += 1;
                    let g = st.generation;
                    st.running.insert(k, (g, Arc::clone(&cancel)));
                    g
                };
                let send = Arc::clone(&self.send);
                let ctl = c.name.to_owned();
                std::thread::spawn(move || {
                    job.run(cancel, |report| send(KbNews { inst, ctl: ctl.clone(), generation, report }));
                });
                Some(KbEffects { props, answer: "1".into() })
            }
        }
    }

    fn set_results(&mut self, inst: u32, ctl: &str, results: Vec<serde_json::Value>) {
        self.state.borrow_mut().kept.entry(key(inst, ctl)).or_default().results = kb_door::results_of(&results);
    }

    fn cancel(&mut self, inst: u32, ctl: &str) -> bool {
        match self.state.borrow_mut().running.remove(&key(inst, ctl)) {
            Some((_, flag)) => {
                flag.store(true, Ordering::Relaxed);
                true
            }
            None => false,
        }
    }

    fn release(&mut self, inst: u32) {
        // An operation still running is not stopped, as the interpreter does not stop one when its
        // form closes: an index half written is worse than one finished for nobody.
        let mut st = self.state.borrow_mut();
        st.running.retain(|(i, _), _| *i != inst);
        st.kept.retain(|(i, _), _| *i != inst);
    }
}
