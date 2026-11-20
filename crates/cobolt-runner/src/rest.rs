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
//!
//! A WebSearch's `Search` is such a request (T47a, ABI A48), built from the control's search
//! properties by its own rules (`cobolt_runtime::search_door`, which the interpreter follows too):
//! a control that is not configured sends nothing and raises `onError`; an answer raises
//! `onResultsReceived` before `onComplete`; `Verbose` narrates the request and its ending, every
//! credential masked; `ResultCount`, `TopTitle`, `TopSnippet`, `TopLink` and `GetResult(n)` read the
//! last answer.
//!
//! A Maps control's places, directions and road route are such requests too (T48), made by the
//! control's own rules (`cobolt_runtime::maps_door`, which the interpreter follows): always on a
//! thread of their own, whatever the `Mode`, with Google's client or OpenRouteService's in place of
//! the window's HTTP client, and a RestClient's endings; a second while one runs is ignored before
//! the key is looked at, as under the interpreter; one with no key sends nothing and raises
//! `onError`. Its markers, routes and regions are properties the port rewrites.

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::Arc;

use cobolt_abi_host::host::{RestControl, RestEffects, RestPort};
use cobolt_runtime::rest_door::{self, Effects, Request, Start};
use cobolt_runtime::{maps_door, search_door};

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
    /// generation → (the control's form instance, its name as the form writes it, whether it is a
    /// WebSearch's search).
    jobs: HashMap<u64, (u32, String, bool)>,
    /// The request each RestClient is running: (instance, CONTROL) → its generation. `Cancel` and a
    /// time-out take it away, so its ending is dropped.
    running: HashMap<(u32, String), u64>,
    generation: u64,
}

fn host_effects(fx: Effects) -> RestEffects {
    RestEffects { lines: Vec::new(), props: fx.props, events: fx.events, answer: fx.answer }
}

/// A WebSearch's effects, after the `lines` its request narrated.
fn search_effects(mut lines: Vec<String>, fx: search_door::Effects) -> RestEffects {
    lines.extend(fx.lines);
    RestEffects { lines, props: fx.props, events: fx.events, answer: fx.answer }
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
    /// request was cancelled or ran out of time, or its window has closed. `verbose` reads a
    /// WebSearch's `Verbose` as it is now, as the interpreter reads it when the answer comes.
    pub(crate) fn delivered(&self, news: RestNews, verbose: &dyn Fn(u32, &str) -> bool) -> Option<(u32, String, RestEffects)> {
        let mut r = self.requests.borrow_mut();
        match news {
            RestNews::Done { generation, inst, body, status } => {
                let (_, name, search) = r.jobs.remove(&generation)?;
                let running = (inst, name.trim().to_ascii_uppercase());
                if r.running.get(&running) != Some(&generation) {
                    return None;
                }
                r.running.remove(&running);
                drop(r);
                let fx = if search {
                    search_effects(Vec::new(), search_door::delivered(verbose(inst, &name), &name, &body, status))
                } else {
                    host_effects(rest_door::delivered(&body, status))
                };
                Some((inst, name, fx))
            }
            RestNews::TimedOut { generation, inst, ms } => {
                let running = r.running.iter().find(|(_, g)| **g == generation).map(|(k, _)| k.clone())?;
                r.running.remove(&running);
                let name = r.jobs.get(&generation).map(|(_, n, _)| n.clone())?;
                Some((inst, name, host_effects(rest_door::timed_out(ms))))
            }
        }
    }

    /// Send `request` for the control `c` of instance `inst`: now, when it is `Sync`, answering what
    /// `ended` makes of its answer; otherwise on a thread of its own, the control busy, its ending
    /// coming back as [`RestNews`]. A second request while one runs is ignored (spec 032 R6).
    fn send(&mut self, inst: u32, c: &RestControl<'_>, request: Request, search: bool, ended: &dyn Fn(&str, u16) -> RestEffects) -> RestEffects {
        let mut http = self.http.borrow_mut();
        let client = http.client();
        if request.sync {
            let (body, status) = client.send_configured(&request.verb, &request.url, request.body.as_deref(), &request.cfg);
            return ended(&body, status);
        }
        let client = client.clone();
        drop(http);
        let timeout_ms = request.timeout_ms;
        self.spawn(inst, c, search, timeout_ms, move || client.send_configured(&request.verb, &request.url, request.body.as_deref(), &request.cfg))
    }

    /// Run `work` for the control `c` of instance `inst` on a thread of its own, the control busy, its
    /// ending (the body and the status) coming back as [`RestNews`], and a second thread waiting out
    /// `timeout_ms` (0: none). A second request while one runs is ignored (spec 032 R6).
    fn spawn(&mut self, inst: u32, c: &RestControl<'_>, search: bool, timeout_ms: u64, work: impl FnOnce() -> (String, u16) + Send + 'static) -> RestEffects {
        let ctl = c.name.trim().to_ascii_uppercase();
        let generation = {
            let mut r = self.requests.borrow_mut();
            if r.running.contains_key(&(inst, ctl.clone())) {
                // One request in flight per control: the second start is ignored (spec 032 R6).
                return RestEffects::default();
            }
            r.generation += 1;
            let generation = r.generation;
            r.running.insert((inst, ctl), generation);
            r.jobs.insert(generation, (inst, c.name.to_owned(), search));
            generation
        };
        let send = Arc::clone(&self.send);
        std::thread::spawn(move || {
            let (body, status) = work();
            send(RestNews::Done { generation, inst, body, status });
        });
        if timeout_ms > 0 {
            let send = Arc::clone(&self.send);
            std::thread::spawn(move || {
                std::thread::sleep(std::time::Duration::from_millis(timeout_ms));
                send(RestNews::TimedOut { generation, inst, ms: timeout_ms });
            });
        }
        host_effects(rest_door::started())
    }

    /// A method of the WebSearch `c` (T47a), by the control's own rules.
    fn search(&mut self, inst: u32, c: &RestControl<'_>, method: &str, args: &[String]) -> Option<RestEffects> {
        match search_door::start(c.prop, c.name, method, args) {
            search_door::Start::NotOwn => None,
            search_door::Start::Answer(answer) => Some(RestEffects { answer, ..RestEffects::default() }),
            search_door::Start::NotConfigured(fx) => Some(search_effects(Vec::new(), fx)),
            search_door::Start::Request { request, lines } => {
                let verbose = search_door::verbose(c.prop);
                let fx = self.send(inst, c, request, true, &|body, status| search_effects(Vec::new(), search_door::finished(verbose, c.name, body, status)));
                let mut all = lines;
                all.extend(fx.lines);
                Some(RestEffects { lines: all, ..fx })
            }
        }
    }

    /// A method of the Maps control `c` (T48), by the control's own rules.
    fn maps(&mut self, inst: u32, c: &RestControl<'_>, method: &str, args: &[String]) -> Option<RestEffects> {
        if maps_door::is_service(method) && self.requests.borrow().running.contains_key(&(inst, c.name.trim().to_ascii_uppercase())) {
            // One request in flight per control, ignored before its key is looked at (spec 032 R6).
            return Some(RestEffects::default());
        }
        Some(match maps_door::start(c.prop, method, args) {
            maps_door::Start::NotOwn => return None,
            maps_door::Start::Set(prop, value) => RestEffects { props: vec![(prop.to_owned(), value)], ..RestEffects::default() },
            maps_door::Start::NotConfigured(fx) => host_effects(fx),
            maps_door::Start::Request(request) => {
                let timeout_ms = request.timeout_ms;
                self.spawn(inst, c, false, timeout_ms, move || maps_door::run(&request))
            }
        })
    }
}

impl RestPort for Port {
    fn call(&mut self, inst: u32, c: &RestControl<'_>, method: &str, args: &[&str]) -> Option<RestEffects> {
        let args: Vec<String> = args.iter().map(|a| a.trim().to_owned()).collect();
        if c.class == "WebSearch" {
            return self.search(inst, c, method, &args);
        }
        if c.class == "Maps" {
            return self.maps(inst, c, method, &args);
        }
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
        Some(self.send(inst, c, request, false, &|body, status| host_effects(rest_door::finished(body, status))))
    }

    fn cancel(&mut self, inst: u32, ctl: &str) -> bool {
        self.requests.borrow_mut().running.remove(&(inst, ctl.trim().to_ascii_uppercase())).is_some()
    }

    fn release(&mut self, inst: u32) {
        let mut r = self.requests.borrow_mut();
        r.running.retain(|(k, _), _| *k != inst);
        r.jobs.retain(|_, (k, _, _)| *k != inst);
    }
}
