// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! The RestClient controls (DESKTOP-WASM-001 T42b, ABI A46): the host's [`RestPort`], by the
//! control's own rules (`cobolt_runtime::rest_door`, which the interpreter follows), with the HTTP
//! client of the window that calls, the one its HTTP built-ins use (T42), so `SetHeader` on a
//! RestClient and `COBOL::"HTTP-SET-HEADER"` set the same headers, as under the interpreter.
//!
//! A request of a control whose `Mode` is `Sync` runs now, on the program's thread, and answers the
//! body. Any other runs on a thread of its own with a copy of the window's client, as the
//! interpreter's worker runs with a copy of its client, and comes back to the program thread as
//! [`RestNews`], with a second thread waiting out its `TimeoutMs`; a second request while one runs is
//! ignored, `Cancel` and a time-out let the running one go and drop its ending, as under the
//! interpreter (spec 032).

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::Arc;

use cobolt_abi_host::host::{RestControl, RestEffects, RestPort};
use cobolt_runtime::rest_door::{self, Effects, Start};

use crate::http::Clients;

/// A RestClient request's news, on its way to the program thread (T42b).
pub(crate) enum RestNews {
    /// A request on its own thread ended: the body and the status (0: no answer came back, the body
    /// saying why).
    Done { generation: u64, inst: u32, body: String, status: u16 },
    /// A request's `TimeoutMs` (`ms`) passed.
    TimedOut { generation: u64, inst: u32, ms: u64 },
}

/// The requests running on their own threads.
#[derive(Default)]
struct Requests {
    /// generation → (the control's form instance, its name as the form writes it).
    jobs: HashMap<u64, (u32, String)>,
    /// The request each RestClient is running: (instance, CONTROL) → its generation. `Cancel` and a
    /// time-out take it away, so its ending is dropped.
    running: HashMap<(u32, String), u64>,
    generation: u64,
}

fn host_effects(fx: Effects) -> RestEffects {
    RestEffects { props: fx.props, events: fx.events, answer: fx.answer }
}

/// The runner's RestClient (T42b): the host's port, over the windows' HTTP clients.
#[derive(Clone)]
pub(crate) struct Port {
    http: Rc<RefCell<Clients>>,
    requests: Rc<RefCell<Requests>>,
    send: Arc<dyn Fn(RestNews) + Send + Sync>,
}

impl Port {
    pub(crate) fn new(http: Rc<RefCell<Clients>>, send: Arc<dyn Fn(RestNews) + Send + Sync>) -> Self {
        Port { http, requests: Rc::default(), send }
    }

    /// News of a request: `(the instance, the control's name, what the host does)`, or `None` when the
    /// request was cancelled or ran out of time, or its window has closed.
    pub(crate) fn delivered(&self, news: RestNews) -> Option<(u32, String, RestEffects)> {
        let mut r = self.requests.borrow_mut();
        match news {
            RestNews::Done { generation, inst, body, status } => {
                let (_, name) = r.jobs.remove(&generation)?;
                let running = (inst, name.trim().to_ascii_uppercase());
                if r.running.get(&running) != Some(&generation) {
                    return None;
                }
                r.running.remove(&running);
                Some((inst, name, host_effects(rest_door::delivered(&body, status))))
            }
            RestNews::TimedOut { generation, inst, ms } => {
                let running = r.running.iter().find(|(_, g)| **g == generation).map(|(k, _)| k.clone())?;
                r.running.remove(&running);
                let name = r.jobs.get(&generation).map(|(_, n)| n.clone())?;
                Some((inst, name, host_effects(rest_door::timed_out(ms))))
            }
        }
    }
}

impl RestPort for Port {
    fn call(&mut self, inst: u32, c: &RestControl<'_>, method: &str, args: &[&str]) -> Option<RestEffects> {
        let args: Vec<String> = args.iter().map(|a| a.trim().to_owned()).collect();
        // A module never holds the data item `RequestDataItem` names: the Build sends a RestClient that
        // names one down the Cargo path (T42b).
        let request = match rest_door::start(c.prop, method, &args, &|| None) {
            Start::NotOwn => return None,
            Start::SetTimeout(v) => return Some(RestEffects { props: vec![("Timeout".into(), v)], ..RestEffects::default() }),
            Start::SetHeader(name, value) => {
                self.http.borrow_mut().client().set_header(name, value);
                return Some(RestEffects::default());
            }
            Start::ClearHeaders => {
                self.http.borrow_mut().client().clear_headers();
                return Some(RestEffects::default());
            }
            Start::Request(r) => r,
        };
        let mut http = self.http.borrow_mut();
        let client = http.client();
        if request.sync {
            let (body, status) = client.send_configured(&request.verb, &request.url, request.body.as_deref(), &request.cfg);
            return Some(host_effects(rest_door::finished(&body, status)));
        }
        let client = client.clone();
        drop(http);
        let ctl = c.name.trim().to_ascii_uppercase();
        let generation = {
            let mut r = self.requests.borrow_mut();
            if r.running.contains_key(&(inst, ctl.clone())) {
                // One request in flight per control: the second start is ignored (spec 032 R6).
                return Some(RestEffects::default());
            }
            r.generation += 1;
            let generation = r.generation;
            r.running.insert((inst, ctl), generation);
            r.jobs.insert(generation, (inst, c.name.to_owned()));
            generation
        };
        let send = Arc::clone(&self.send);
        let timeout_ms = request.timeout_ms;
        std::thread::spawn(move || {
            let (body, status) = client.send_configured(&request.verb, &request.url, request.body.as_deref(), &request.cfg);
            send(RestNews::Done { generation, inst, body, status });
        });
        if timeout_ms > 0 {
            let send = Arc::clone(&self.send);
            std::thread::spawn(move || {
                std::thread::sleep(std::time::Duration::from_millis(timeout_ms));
                send(RestNews::TimedOut { generation, inst, ms: timeout_ms });
            });
        }
        Some(host_effects(rest_door::started()))
    }

    fn cancel(&mut self, inst: u32, ctl: &str) -> bool {
        self.requests.borrow_mut().running.remove(&(inst, ctl.trim().to_ascii_uppercase())).is_some()
    }

    fn release(&mut self, inst: u32) {
        let mut r = self.requests.borrow_mut();
        r.running.retain(|(k, _), _| *k != inst);
        r.jobs.retain(|_, (k, _)| *k != inst);
    }
}
