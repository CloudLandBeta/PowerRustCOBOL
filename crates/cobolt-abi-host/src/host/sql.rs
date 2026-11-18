// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! **The SqlDatabase control on the desktop** (DESKTOP-WASM-001 T41c, ABI A43).
//!
//! A method called on a SqlDatabase goes to the runner's [`SqlPort`], which runs it with the
//! interpreter's own code (`cobolt_runtime::db_runtime::DbRegistry::control`) on the window's
//! connections, the ones its SQL built-ins use. The host carries out what the port answers, in its
//! order: the properties are written on the control, then each event is raised. A statement of a
//! control whose `Mode` is `Async` runs on a thread of the runner, and its result comes back later
//! through [`Host::sql_delivered`]. `Cancel` lets a running statement go, as on every asynchronous
//! control; `IsBusy` reads `Busy`. A browser module never reaches this: its database is the
//! backend's, and a host with no port answers as before.

use super::*;

/// What the host knows of the SqlDatabase a method was called on.
pub struct SqlControl<'a> {
    /// The control's name in the form.
    pub name: &'a str,
    /// One of its properties, as the program would read it.
    pub prop: &'a dyn Fn(&str) -> String,
}

/// What a method call, or a statement's result, does: the properties written, in order, then the
/// events raised, in order, and the method's answer.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct SqlEffects {
    pub props: Vec<(String, String)>,
    pub events: Vec<String>,
    pub answer: String,
}

/// The desktop's SqlDatabase (DESKTOP-WASM-001 T41c), which the runner gives the host in
/// [`Host::sql`].
pub trait SqlPort {
    /// A method (`method`, upper case) of the SqlDatabase `c` of instance `inst`, with the argument
    /// texts `args`: what follows, or `None` when it is not one of the database's own methods, so
    /// the host's general dispatch answers it (`IsBusy`, a property).
    fn call(&mut self, inst: u32, c: &SqlControl<'_>, method: &str, args: &[&str]) -> Option<SqlEffects>;
    /// `Cancel`: whether a statement of the control was running. Its result, when it comes, is
    /// dropped.
    fn cancel(&mut self, inst: u32, ctl: &str) -> bool;
    /// The instance `inst` ended: results still to come for it are dropped.
    fn release(&mut self, inst: u32);
}

impl Host {
    /// A method of the control `idx` when that control is a SqlDatabase and the host has a port:
    /// `None` otherwise, and for a method that is not the database's own.
    pub(super) fn sql_call(&mut self, inst: u32, idx: usize, name: &str, args: &[&str]) -> Option<Result<Option<String>, CmdError>> {
        let tree = self.trees.get(&inst)?;
        if tree.ctls[idx].kind.type_name() != "SqlDatabase" {
            return None;
        }
        let port = self.sql.as_mut()?;
        let m = name.trim().to_ascii_uppercase();
        let ctl = tree.ctl(idx).name.clone();
        if m == "CANCEL" {
            // The uniform asynchronous `Cancel` (spec 032): a statement still running is let go, its
            // result dropped when it comes; the control is no longer busy and hears `onCancelled`.
            if port.cancel(inst, &ctl) {
                let _ = self.trees.get_mut(&inst).expect("checked").set(idx, "Busy", "0");
                self.dom_event(inst, idx, "onCancelled", Vec::new());
            }
            return Some(Ok(None));
        }
        let prop = |p: &str| tree.get(idx, p);
        let fx = port.call(inst, &SqlControl { name: &ctl, prop: &prop }, &m, args)?;
        Some(Ok(Some(self.service_apply(inst, idx, &fx.props, &fx.events, fx.answer))))
    }

    /// Carry out what a service control's port decided on the control `idx` of instance `inst`: write
    /// `props`, in order, then raise each of `events`, in order; answer what the method answers
    /// (`answer`). The SqlDatabase's and the RestClient's (T42b) alike.
    pub(super) fn service_apply(&mut self, inst: u32, idx: usize, props: &[(String, String)], events: &[String], answer: String) -> String {
        if let Some(tree) = self.trees.get_mut(&inst) {
            for (k, v) in props {
                let _ = tree.set(idx, k, v);
            }
        }
        for e in events {
            self.dom_event(inst, idx, e, Vec::new());
        }
        answer
    }

    /// An asynchronous statement of the SqlDatabase named `ctl` of instance `inst` ended, or ran out
    /// of time, and `fx` is what that does (the runner's port decided it). Nothing happens when the
    /// instance or the control is gone.
    pub fn sql_delivered(&mut self, inst: u32, ctl: &str, fx: SqlEffects) {
        let Some(idx) = self.trees.get(&inst).and_then(|t| t.by_name(ctl)) else { return };
        self.service_apply(inst, idx, &fx.props, &fx.events, fx.answer);
    }
}
