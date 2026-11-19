// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! **The AWS controls on the desktop** (DESKTOP-WASM-001 T48, ABI A38).
//!
//! A method called on an `Aws…` control goes to the runner's [`AwsPort`], which decides it with the
//! interpreter's own rules (`cobolt_runtime::aws::door`) and runs the operation; the host carries
//! out what the port answers, in its order: the narration goes where the program's `DISPLAY` goes,
//! the properties are written on the control, the rows become its row set, and each event is raised
//! with the values it carries written as it leaves the queue, bound to a handler or not, as the
//! interpreter writes them. An asynchronous operation's result comes back later through
//! [`Host::aws_delivered`]. `Cancel` and `IsBusy` are the uniform ones every asynchronous control
//! has. A browser module never reaches this: its AWS controls are the backend's.

use serde_json::Value;

use super::*;

/// What the host knows of the AWS control a method was called on.
pub struct AwsControl<'a> {
    /// The control's type: `AwsS3`, `AwsLambda`, `AwsMcp`, …
    pub class: &'a str,
    /// The control's name in the form.
    pub name: &'a str,
    /// One of its properties, as the program would read it.
    pub prop: &'a dyn Fn(&str) -> String,
    /// The form's AWS controls, this one included: each one's `Connection`, and whether its
    /// `AllowWrite` is set (a connection's shared server starts read-only unless one of them is).
    pub peers: &'a [(String, bool)],
}

/// An event an operation raises, with the values it writes on the control as it leaves the queue.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct AwsEvent {
    pub name: String,
    pub props: Vec<(String, String)>,
    pub rows: Option<Vec<Value>>,
}

/// What a method call, or an operation's result, does — in this order (see the module's note).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct AwsEffects {
    /// Lines for the program's `DISPLAY` output, already masked.
    pub lines: Vec<String>,
    pub props: Vec<(String, String)>,
    pub rows: Option<Vec<Value>>,
    pub events: Vec<AwsEvent>,
    /// The method's answer.
    pub answer: String,
}

/// The desktop's AWS (DESKTOP-WASM-001 T48), which the runner gives the host in [`Host::aws`]. A
/// desktop host with none answers as an interpreter built without AWS: every operation raises
/// `onError` saying so.
pub trait AwsPort {
    /// A method (`method`, as the program wrote it) of the AWS control `c` of instance `inst`, with
    /// the argument texts `args`: what follows, or `None` when it is not one of the control's own
    /// methods, so the host's general dispatch answers it.
    fn call(&mut self, inst: u32, c: &AwsControl<'_>, method: &str, args: &[&str]) -> Option<AwsEffects>;
    /// `GetRow` (no `field`) and `GetField`: the control's row `i` (1-based) as JSON, or one of its
    /// fields; empty when there is no such row or field.
    fn row_value(&self, inst: u32, ctl: &str, i: &str, field: Option<&str>) -> String;
    /// The control's row set is now `rows`.
    fn set_rows(&mut self, inst: u32, ctl: &str, rows: Vec<Value>);
    /// `Cancel`: whether an operation of the control was running. Its result, when it comes, is
    /// dropped.
    fn cancel(&mut self, inst: u32, ctl: &str) -> bool;
    /// `AWS-SET-CREDENTIALS` (connection, key id, secret, token) and `AWS-CLEAR-CREDENTIALS`
    /// (connection): the status. `AWS-CREDENTIALS-SET` (connection): `Y` or `N`. The arguments are
    /// never logged.
    fn credentials(&mut self, verb: &str, args: &[&str]) -> String;
    /// The instance `inst` ended: its row sets go, and results still to come for it are dropped.
    fn release(&mut self, inst: u32);
}

/// An operation's values waiting in the queue behind their event (see [`Work::Apply`]).
#[derive(Debug, Clone)]
pub(super) struct AwsApply {
    pub inst: u32,
    pub ctl: usize,
    pub event: String,
    pub props: Vec<(String, String)>,
    pub rows: Option<Vec<Value>>,
}

/// What an interpreter built without AWS says.
const NO_AWS: &str = "this application was built without AWS support";

impl Host {
    /// A method of the control `idx` when that control is an AWS control: `None` for any other
    /// control, and for a method that is not the AWS control's own (`IsBusy` among them).
    pub(super) fn aws_call(&mut self, inst: u32, idx: usize, name: &str, args: &[&str]) -> Option<Result<Option<String>, CmdError>> {
        let tree = self.trees.get(&inst)?;
        let class = tree.ctls[idx].kind.type_name();
        if !class.starts_with("Aws") {
            return None;
        }
        let m = name.trim().to_ascii_uppercase();
        let ctl = tree.ctl(idx).name.clone();
        let arg = |i: usize| args.get(i).map(|a| a.trim().to_owned()).unwrap_or_default();
        let Some(port) = self.aws.as_mut() else {
            return match m.as_str() {
                "GETROW" | "GETFIELD" => Some(Ok(Some(String::new()))),
                "CANCEL" | "ISBUSY" => None,
                _ => {
                    let fx = AwsEffects {
                        events: vec![AwsEvent { name: "onError".into(), props: vec![("LastError".into(), NO_AWS.into())], rows: None }],
                        answer: "0".into(),
                        ..AwsEffects::default()
                    };
                    Some(Ok(Some(self.aws_apply(inst, idx, fx))))
                }
            };
        };
        match m.as_str() {
            "GETROW" => return Some(Ok(Some(port.row_value(inst, &ctl, &arg(0), None)))),
            "GETFIELD" => return Some(Ok(Some(port.row_value(inst, &ctl, &arg(0), Some(&arg(1)))))),
            _ => {}
        }
        let peers: Vec<(String, bool)> = tree
            .ctls
            .iter()
            .enumerate()
            .filter(|(_, c)| c.kind.type_name().starts_with("Aws"))
            .map(|(i, _)| (tree.get(i, "Connection"), tree.get(i, "AllowWrite").trim().eq_ignore_ascii_case("true")))
            .collect();
        let prop = |p: &str| tree.get(idx, p);
        let control = AwsControl { class: &class, name: &ctl, prop: &prop, peers: &peers };
        match port.call(inst, &control, &m, args) {
            Some(fx) => Some(Ok(Some(self.aws_apply(inst, idx, fx)))),
            // The uniform asynchronous `Cancel` (spec 032): an operation still running is let go, its
            // result dropped when it comes; the control is no longer busy and hears `onCancelled`.
            None if m == "CANCEL" => {
                if port.cancel(inst, &ctl) {
                    let _ = self.trees.get_mut(&inst).expect("checked").set(idx, "Busy", "0");
                    self.dom_event(inst, idx, "onCancelled", Vec::new());
                }
                Some(Ok(None))
            }
            None => None,
        }
    }

    /// Carry out `fx` on the control `idx` of instance `inst` (see the module's note), and answer
    /// what the method answers.
    fn aws_apply(&mut self, inst: u32, idx: usize, fx: AwsEffects) -> String {
        for line in fx.lines {
            self.effects.push(Effect::Display(line));
        }
        if let Some(tree) = self.trees.get_mut(&inst) {
            for (k, v) in &fx.props {
                let _ = tree.set(idx, k, v);
            }
        }
        if let Some(rows) = fx.rows {
            self.aws_rows(inst, idx, rows);
        }
        for e in fx.events {
            if e.props.is_empty() && e.rows.is_none() {
                // An event that carries no values (`onComplete`) is only raised.
                self.dom_event(inst, idx, &e.name, Vec::new());
            } else {
                self.work.push_back(Work::Apply(AwsApply { inst, ctl: idx, event: e.name, props: e.props, rows: e.rows }));
            }
        }
        fx.answer
    }

    /// The control `idx` now holds `rows`: an AWS control's row set, or a KnowledgeBase's search
    /// results (T47b), which travel the same way with their event.
    fn aws_rows(&mut self, inst: u32, idx: usize, rows: Vec<Value>) {
        let Some((name, kb)) = self.trees.get(&inst).map(|t| (t.ctl(idx).name.clone(), t.ctls[idx].kind.type_name() == "KnowledgeBase")) else { return };
        if kb {
            if let Some(port) = self.kb.as_mut() {
                port.set_results(inst, &name, rows);
            }
        } else if let Some(port) = self.aws.as_mut() {
            port.set_rows(inst, &name, rows);
        }
    }

    /// The queue reached an operation's values: write them, then the event they belong to follows
    /// when a handler is bound to it.
    pub(super) fn aws_applied(&mut self, a: AwsApply) -> Option<QueuedEvent> {
        let tree = self.trees.get_mut(&a.inst)?;
        for (k, v) in &a.props {
            let _ = tree.set(a.ctl, k, v);
        }
        if let Some(rows) = a.rows {
            self.aws_rows(a.inst, a.ctl, rows);
        }
        let c = self.trees.get(&a.inst)?.ctls.get(a.ctl)?;
        let abi = c.abi_id?;
        if !c.has_handler(&a.event) {
            return None;
        }
        let ev = self.manifest.event_index(&a.event)?;
        Some(QueuedEvent { inst: a.inst, ctrl: abi, event: ev, args: Vec::new() })
    }

    /// An asynchronous AWS operation of the control named `ctl` of instance `inst` ended, and `fx` is
    /// what its result does (the runner's port decided it, `Verbose` read with
    /// [`Host::control_property`]). Nothing happens when the instance or the control is gone.
    pub fn aws_delivered(&mut self, inst: u32, ctl: &str, fx: AwsEffects) {
        let Some(idx) = self.trees.get(&inst).and_then(|t| t.by_name(ctl)) else { return };
        self.aws_apply(inst, idx, fx);
    }

    /// The property `prop` of the control named `ctl` of instance `inst`, as the program reads it;
    /// `None` when there is no such control.
    pub fn control_property(&self, inst: u32, ctl: &str, prop: &str) -> Option<String> {
        let tree = self.trees.get(&inst)?;
        let idx = tree.by_name(ctl)?;
        Some(tree.get(idx, prop))
    }

    /// `COBOL::"AWS-SET-CREDENTIALS"`, `"AWS-CLEAR-CREDENTIALS"` and `"AWS-CREDENTIALS-SET"`: the
    /// answer goes to the application's `AwsStatus` or `AwsFlag`, which the generated code reads. A
    /// host with no AWS answers as an interpreter built without it.
    pub(super) fn aws_credentials(&mut self, verb: &str, args: &[&str]) {
        let answer = match self.aws.as_mut() {
            Some(port) => port.credentials(verb, args),
            None if verb == "AWS-CREDENTIALS-SET" => "N".to_owned(),
            None => format!("{NO_AWS}: nothing was stored"),
        };
        let key = if verb == "AWS-CREDENTIALS-SET" { "awsflag" } else { "awsstatus" };
        self.app.insert(key.into(), answer);
    }
}
