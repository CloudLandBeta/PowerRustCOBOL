// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! **The KnowledgeBase control on the desktop** (DESKTOP-WASM-001 T47b, ABI A49).
//!
//! A method called on a `KnowledgeBase` goes to the runner's [`KbPort`], which decides it with the
//! interpreter's own rules (`cobolt_runtime::kb_door`) and runs its background operation. The host
//! carries out what the port answers: the properties are written on the control, and each event a
//! running operation raises comes back through [`Host::kb_delivered`] with the values it carries,
//! written as it leaves the queue, bound to a handler or not, as the interpreter writes them (the
//! AWS controls' [`Work::Apply`]). A finished search's results travel with `onSearchComplete` and
//! become the control's results as that event leaves the queue, so its handler reads exactly
//! them. `Cancel` is the uniform one: a running operation is let go, the control is no longer busy
//! and hears `onCancelled`. A browser module never reaches this.

use serde_json::Value;

use super::*;

/// What the host knows of the KnowledgeBase a method was called on.
pub struct KbControl<'a> {
    /// The control's name in the form.
    pub name: &'a str,
    /// One of its properties, as the program would read it.
    pub prop: &'a dyn Fn(&str) -> String,
}

/// An event a running operation raises, with the values it writes on the control as it leaves the
/// queue: its properties and, for `onSearchComplete`, the results (one JSON object per result).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct KbEvent {
    pub name: String,
    pub props: Vec<(String, String)>,
    pub results: Option<Vec<Value>>,
}

/// What a method call does: the properties written, in order, and its answer.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct KbEffects {
    pub props: Vec<(String, String)>,
    pub answer: String,
}

/// The desktop's Knowledge Base (DESKTOP-WASM-001 T47b), which the runner gives the host in
/// [`Host::kb`]. A desktop host with none answers as an interpreter built without it.
pub trait KbPort {
    /// A method (`method`, upper case) of the KnowledgeBase `c` of instance `inst`, with the
    /// argument texts `args`: what follows, or `None` when it is not one of the control's own
    /// methods, so the host's general dispatch answers it. `Cancel` never reaches it.
    fn call(&mut self, inst: u32, c: &KbControl<'_>, method: &str, args: &[&str]) -> Option<KbEffects>;
    /// The control's results are now `results` (an `onSearchComplete` left the queue).
    fn set_results(&mut self, inst: u32, ctl: &str, results: Vec<Value>);
    /// `Cancel`: whether an operation of the control was running. Its reports, when they come, are
    /// dropped.
    fn cancel(&mut self, inst: u32, ctl: &str) -> bool;
    /// The instance `inst` ended: what its controls kept goes, and reports still to come for it are
    /// dropped.
    fn release(&mut self, inst: u32);
}

/// The methods of a KnowledgeBase, which a host with no Knowledge Base answers as an interpreter
/// built without one: `"0"`, with `LastError` saying why.
const KB_METHODS: [&str; 19] = [
    "CREATECOLLECTION",
    "REMOVECOLLECTION",
    "LISTCOLLECTIONS",
    "GETCOLLECTION",
    "LISTDOCUMENTS",
    "GETDOCUMENT",
    "GETRESULTDOCUMENT",
    "GETRESULTHEADING",
    "GETRESULTPASSAGE",
    "GETRESULTSCORE",
    "ADDDOCUMENT",
    "UPDATEDOCUMENT",
    "IMPORTDOCUMENT",
    "DELETEDOCUMENT",
    "REFRESH",
    "REINDEX",
    "SEARCH",
    "FETCHMODEL",
    "CANCEL",
];

/// What an interpreter built without the Knowledge Base says.
const NO_KB: &str = "the Knowledge Base is not linked into this program (runtime feature `kb`)";

impl Host {
    /// A method of the control `idx` when that control is a KnowledgeBase: `None` for any other
    /// control, and for a method that is not the KnowledgeBase's own.
    pub(super) fn kb_call(&mut self, inst: u32, idx: usize, name: &str, args: &[&str]) -> Option<Result<Option<String>, CmdError>> {
        let tree = self.trees.get(&inst)?;
        if tree.ctls[idx].kind.type_name() != "KnowledgeBase" {
            return None;
        }
        let m = name.trim().to_ascii_uppercase();
        let ctl = tree.ctl(idx).name.clone();
        let Some(port) = self.kb.as_mut() else {
            if !KB_METHODS.contains(&m.as_str()) {
                return None;
            }
            let _ = self.trees.get_mut(&inst).expect("checked").set(idx, "LastError", NO_KB);
            return Some(Ok(Some("0".into())));
        };
        if m == "CANCEL" {
            // The uniform asynchronous `Cancel` (spec 032): an operation still running is let go, its
            // reports dropped when they come; the control is no longer busy and hears `onCancelled`.
            if port.cancel(inst, &ctl) {
                let _ = self.trees.get_mut(&inst).expect("checked").set(idx, "Busy", "0");
                self.dom_event(inst, idx, "onCancelled", Vec::new());
            }
            return Some(Ok(Some("1".into())));
        }
        let prop = |p: &str| tree.get(idx, p);
        let control = KbControl { name: &ctl, prop: &prop };
        let fx = port.call(inst, &control, &m, args)?;
        if let Some(tree) = self.trees.get_mut(&inst) {
            for (k, v) in &fx.props {
                let _ = tree.set(idx, k, v);
            }
        }
        Some(Ok(Some(fx.answer)))
    }

    /// A running operation of the KnowledgeBase named `ctl` of instance `inst` raised `events` (the
    /// runner's port decided them): each is queued with the values it carries. Nothing happens when
    /// the instance or the control is gone.
    pub fn kb_delivered(&mut self, inst: u32, ctl: &str, events: Vec<KbEvent>) {
        let Some(idx) = self.trees.get(&inst).and_then(|t| t.by_name(ctl)) else { return };
        for e in events {
            self.work.push_back(Work::Apply(aws::AwsApply { inst, ctl: idx, event: e.name, props: e.props, rows: e.results }));
        }
    }
}
