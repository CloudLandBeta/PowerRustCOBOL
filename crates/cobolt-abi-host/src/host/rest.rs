// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! **The RestClient, WebSearch and Maps controls on the desktop** (DESKTOP-WASM-001 T42b, ABI A46;
//! T47a, A48; T48).
//!
//! A method called on a RestClient goes to the runner's [`RestPort`], which carries it out by the
//! control's own rules (`cobolt_runtime::rest_door`, the interpreter's) with the window's HTTP client,
//! the one its HTTP built-ins use. The host carries out what the port answers, in its order: the
//! properties are written on the control, then each event is raised. A request of a control whose
//! `Mode` is not `Sync` runs on a thread of the runner, and its ending comes back later through
//! [`Host::rest_delivered`]. `Cancel` lets a running request go, as on every asynchronous control;
//! `IsBusy` reads `Busy`. A WebSearch takes the same road (T47a): its `Search` is a request the port
//! builds from the control's search properties, by the control's own rules
//! (`cobolt_runtime::search_door`), and its accessors read the last answer. So does a Maps control
//! (T48): its places, directions and road routes are requests on a thread of the runner, and its
//! markers, routes and regions are properties the port rewrites, by the control's own rules
//! (`cobolt_runtime::maps_door`). A browser module never reaches this, and a host with no port
//! answers as before.

use super::*;

/// What the host knows of the RestClient, WebSearch or Maps control a method was called on.
pub struct RestControl<'a> {
    /// The control's type: `RestClient`, `WebSearch` or `Maps`.
    pub class: &'a str,
    /// The control's name in the form.
    pub name: &'a str,
    /// One of its properties, as the program would read it.
    pub prop: &'a dyn Fn(&str) -> String,
}

/// What a method call, or a request's ending, does: the lines for the program's `DISPLAY` output
/// (a WebSearch's `Verbose` narration, already masked), then the properties written, in order, then
/// the events raised, in order, and the method's answer.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct RestEffects {
    pub lines: Vec<String>,
    pub props: Vec<(String, String)>,
    pub events: Vec<String>,
    pub answer: String,
}

/// The desktop's RestClient (DESKTOP-WASM-001 T42b), WebSearch (T47a) and Maps (T48), which the
/// runner gives the host in [`Host::rest`].
pub trait RestPort {
    /// A method (`method`, upper case) of the RestClient, WebSearch or Maps `c` of instance `inst`, with the
    /// argument texts `args`: what follows, or `None` when it is not one of the control's own methods,
    /// so the host's general dispatch answers it (`IsBusy`, a property).
    fn call(&mut self, inst: u32, c: &RestControl<'_>, method: &str, args: &[&str]) -> Option<RestEffects>;
    /// `Cancel`: whether a request of the control was running. Its ending, when it comes, is dropped.
    fn cancel(&mut self, inst: u32, ctl: &str) -> bool;
    /// The instance `inst` ended: endings still to come for it are dropped.
    fn release(&mut self, inst: u32);
}

impl Host {
    /// A method of the control `idx` when that control is a RestClient, a WebSearch or a Maps and the
    /// host has a port: `None` otherwise, and for a method that is not the control's own.
    pub(super) fn rest_call(&mut self, inst: u32, idx: usize, name: &str, args: &[&str]) -> Option<Result<Option<String>, CmdError>> {
        let tree = self.trees.get(&inst)?;
        let class = tree.ctls[idx].kind.type_name();
        if !matches!(class.as_str(), "RestClient" | "WebSearch" | "Maps") {
            return None;
        }
        let port = self.rest.as_mut()?;
        let m = name.trim().to_ascii_uppercase();
        let ctl = tree.ctl(idx).name.clone();
        if m == "CANCEL" {
            // The uniform asynchronous `Cancel` (spec 032): a request still running is let go, its
            // ending dropped when it comes; the control is no longer busy and hears `onCancelled`.
            if port.cancel(inst, &ctl) {
                let _ = self.trees.get_mut(&inst).expect("checked").set(idx, "Busy", "0");
                self.dom_event(inst, idx, "onCancelled", Vec::new());
            }
            return Some(Ok(None));
        }
        let prop = |p: &str| tree.get(idx, p);
        let fx = port.call(inst, &RestControl { class: &class, name: &ctl, prop: &prop }, &m, args)?;
        Some(Ok(Some(self.rest_apply(inst, idx, fx))))
    }

    /// Carry out `fx` on the control `idx` of instance `inst`: its lines go to the program's output
    /// first, then the properties and the events, as [`Host::service_apply`] carries them out.
    fn rest_apply(&mut self, inst: u32, idx: usize, fx: RestEffects) -> String {
        for line in fx.lines {
            self.effects.push(Effect::Display(line));
        }
        self.service_apply(inst, idx, &fx.props, &fx.events, fx.answer)
    }

    /// A request of the RestClient, WebSearch or Maps named `ctl` of instance `inst` that ran on its own
    /// thread ended, or ran out of time, and `fx` is what that does (the runner's port decided it).
    /// Nothing happens when the instance or the control is gone.
    pub fn rest_delivered(&mut self, inst: u32, ctl: &str, fx: RestEffects) {
        let Some(idx) = self.trees.get(&inst).and_then(|t| t.by_name(ctl)) else { return };
        self.rest_apply(inst, idx, fx);
    }
}
