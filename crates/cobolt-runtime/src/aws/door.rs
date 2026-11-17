// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! **A method of an AWS control, for every host** (DESKTOP-WASM-001 T48, plan C5).
//!
//! What a call on an `Aws…` control decides before its operation runs, and what its result writes
//! after: whether the method is the control's own, which connection serves it, whether it is
//! refused, what the request is, how long it may take, and the properties, rows, events and
//! narration that follow. The interpreter and the desktop runner both call these functions, so
//! neither keeps a copy of the rules. Running the operation is [`ops::execute`], on whatever thread
//! the host chooses.
//!
//! A host applies an [`Effects`] in its order: the notes go to wherever the program's `DISPLAY`
//! goes ([`note_lines`]); the properties are written on the control; the rows, when there are any,
//! become the control's row set, which `GetRow` and `GetField` read ([`row_value`]); each event is
//! raised, its values written as it leaves the event queue; and the answer is the method's.

use std::time::Duration;

use serde_json::Value;

use super::{connections, ops, pool::Budget, routes::Routes};

/// What the host knows of the control a method was called on.
pub struct Control<'a> {
    /// The control's class: `AwsS3`, `AwsLambda`, `AwsMcp`, …
    pub class: &'a str,
    /// The control as the program named it.
    pub name: &'a str,
    /// One of its properties, as the program would read it.
    pub prop: &'a dyn Fn(&str) -> String,
    /// Whether an AWS control of the form that names the connection with this id (this one
    /// included) has `AllowWrite` set: it decides whether the connection's shared server starts
    /// read-only (spec 078 A4).
    pub any_allows_write: &'a dyn Fn(&str) -> bool,
    /// An asynchronous operation this control started has not reported yet.
    pub running: bool,
}

/// One narration for the program's `DISPLAY` output, already masked: a key, a secret or a token
/// never reaches it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Note {
    pub label: String,
    pub text: String,
}

impl Note {
    fn masked(label: impl Into<String>, text: &str) -> Note {
        Note { label: label.into(), text: super::process::mask(text) }
    }
}

/// The lines a note writes for the control `control`: `[aws CONTROL] label: text`.
pub fn note_lines(control: &str, note: &Note) -> Vec<String> {
    crate::narration::block("aws", control, &note.label, &note.text)
}

/// An event an operation raises, with the values it carries: they are written on the control as
/// the event leaves the queue, bound to a handler or not.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Event {
    pub name: String,
    pub props: Vec<(String, String)>,
    pub rows: Option<Vec<Value>>,
}

/// What the host does, in this order (see the module's note).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Effects {
    pub notes: Vec<Note>,
    pub props: Vec<(String, String)>,
    pub rows: Option<Vec<Value>>,
    pub events: Vec<Event>,
    pub answer: String,
}

/// What a method call becomes.
pub enum Start {
    /// Not one of the control's own methods: the host's general dispatch answers it.
    NotOwn,
    /// Refused before anything ran; [`refused`] says what follows.
    Refused { sync: bool, message: String },
    /// An operation to run.
    Run(Box<Run>),
}

/// An operation ready to run: at once when `sync` ([`finished`]), else on a worker, after
/// [`started`], whose outcome comes back through [`delivered`].
pub struct Run {
    pub sync: bool,
    /// The route table's spelling of the method (`PutObject`).
    pub method: String,
    pub prepared: ops::Prepared,
    pub budget: Budget,
    /// `Verbose` is on: the request is narrated (`request`), and so is the answer.
    pub verbose: bool,
    pub request: Option<Note>,
}

/// The route table's spelling of `m` on controls of `class`, when the table (the shipped one, with
/// the connection's override) routes it.
fn route(class: &str, m: &str, override_toml: &str) -> Option<String> {
    let table = Routes::shipped().with_override(override_toml).ok()?;
    let prefix = format!("{class}.");
    table.ops.keys().filter_map(|k| k.strip_prefix(&prefix)).find(|name| name.eq_ignore_ascii_case(m)).map(str::to_string)
}

/// A call of `m` (upper case, as the program's method name is matched) with the argument texts
/// `args` on the control `c`.
pub fn start(c: &Control<'_>, m: &str, args: &[String]) -> Start {
    let conn_name = (c.prop)("Connection");
    let conn = connections::find(&conn_name);
    let override_toml = conn.as_ref().map(|c| c.routes_override.clone()).unwrap_or_default();
    let Some(method) = route(c.class, m, &override_toml) else { return Start::NotOwn };
    let sync = (c.prop)("Mode").trim().eq_ignore_ascii_case("sync");

    if !sync && c.running {
        return Start::Refused { sync, message: "another AWS operation is still running on this control".into() };
    }
    let Some(conn) = conn else {
        let known = connections::names();
        let message = if conn_name.trim().is_empty() {
            format!("this control names no AWS connection, and the project has {} — set Connection", known.len())
        } else {
            format!("no AWS connection named \"{}\" (the project has: {})", conn_name.trim(), known.join(", "))
        };
        return Start::Refused { sync, message };
    };

    // An argument not passed is empty; the route table says what stands in for it — the property
    // the designer set (`|or:{prop:…}`), the empty object (`|json:{}`), or nothing (`|opt`).
    let texts: Vec<String> = args.iter().map(|a| a.trim().to_string()).collect();
    let allow_write = (c.prop)("AllowWrite").trim().eq_ignore_ascii_case("true");
    let connection_allows_write = allow_write || (c.any_allows_write)(&conn.id);
    let view = ops::ControlView {
        control_type: c.class,
        method: &method,
        args: &texts,
        prop: c.prop,
        control: c.name,
        allow_write,
        connection_allows_write,
    };
    let prepared = match ops::prepare(&view, &conn) {
        Ok(p) => p,
        Err(message) => return Start::Refused { sync, message },
    };
    let verbose = (c.prop)("Verbose").trim().eq_ignore_ascii_case("true");
    let request = verbose.then(|| {
        let what = match &prepared.action {
            // A password or a token travels in this request.
            ops::Action::Call { tool, .. } if prepared.sensitive => format!("{tool} (arguments not shown: they carry a password or a token)"),
            ops::Action::Call { tool, input } => format!("{tool} {input}"),
            ops::Action::ListTools => "tools/list".into(),
        };
        Note::masked("request", &what)
    });
    let ms = |p: &str, default: u64| (c.prop)(p).trim().parse::<u64>().unwrap_or(default);
    let budget = Budget {
        call: Duration::from_millis(ms("TimeoutMs", 30_000).max(1)),
        start: Duration::from_millis(ms("StartTimeoutMs", 120_000).max(1)),
    };
    Start::Run(Box::new(Run { sync, method, prepared, budget, verbose, request }))
}

/// A refusal, the way the control's `Mode` asks (spec 078 R21, R25): a synchronous call answers
/// empty with `LastError` set; an asynchronous one raises `onError` carrying it and answers `0`. A
/// refusal is never silent: whatever `Verbose` says, it is narrated.
pub fn refused(sync: bool, message: String) -> Effects {
    let notes = vec![Note::masked("refused", &message)];
    if sync {
        Effects { notes, props: vec![("LastError".into(), message)], ..Effects::default() }
    } else {
        Effects { notes, events: vec![Event { name: "onError".into(), props: vec![("LastError".into(), message)], rows: None }], answer: "0".into(), ..Effects::default() }
    }
}

/// A synchronous operation ended with `outcome`. A failure is narrated whatever `Verbose` says;
/// the answer is narrated under `Verbose`.
pub fn finished(method: &str, verbose: bool, outcome: ops::Outcome) -> Effects {
    match outcome {
        ops::Outcome::Done { mut props, rows, .. } => {
            let body = props.iter().find(|(k, _)| k == "ResponseBody").map(|(_, v)| v.clone()).unwrap_or_default();
            let notes = if verbose { vec![Note::masked("response", &body)] } else { Vec::new() };
            props.push(("LastError".into(), String::new()));
            Effects { notes, props, rows: Some(rows), answer: body, ..Effects::default() }
        }
        ops::Outcome::Error(message) => Effects { notes: vec![Note::masked(format!("{method} failed"), &message)], props: vec![("LastError".into(), message)], ..Effects::default() },
        ops::Outcome::Timeout(message) => Effects { notes: vec![Note::masked(format!("{method} timed out"), &message)], props: vec![("LastError".into(), message)], ..Effects::default() },
    }
}

/// An asynchronous operation was handed to a worker: the control is `Busy`, and the method answers
/// `1`.
pub fn started() -> Effects {
    Effects { props: vec![("Busy".into(), "1".into())], answer: "1".into(), ..Effects::default() }
}

/// An asynchronous operation's worker reported `outcome`; `verbose` is the control's `Verbose` now.
/// A success raises the operation's own event with its values, then `onComplete`; a failure raises
/// `onError` or `onTimeout`. Each clears `Busy`.
pub fn delivered(verbose: bool, outcome: ops::Outcome) -> Effects {
    let fail = |name: &str, label: &str, message: String| Effects {
        notes: vec![Note::masked(label, &message)],
        events: vec![Event { name: name.into(), props: vec![("LastError".into(), message), ("Busy".into(), "0".into())], rows: None }],
        ..Effects::default()
    };
    match outcome {
        ops::Outcome::Done { mut props, rows, event } => {
            let notes = if verbose {
                let body = props.iter().find(|(k, _)| k == "ResponseBody").map(|(_, v)| v.clone()).unwrap_or_default();
                vec![Note::masked("response", &body)]
            } else {
                Vec::new()
            };
            props.push(("LastError".into(), String::new()));
            props.push(("Busy".into(), "0".into()));
            Effects { notes, events: vec![Event { name: event, props, rows: Some(rows) }, Event { name: "onComplete".into(), ..Event::default() }], ..Effects::default() }
        }
        ops::Outcome::Error(message) => fail("onError", "error", message),
        ops::Outcome::Timeout(message) => fail("onTimeout", "timed out", message),
    }
}

pub use crate::aws_rows::row_value;
