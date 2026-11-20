// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! **The IndexedFile control's PartiQL members on the desktop** (DESKTOP-WASM-001 T48, ABI A55).
//!
//! A method called on an IndexedFile goes to the runner's [`PartiqlPort`], which runs it by the
//! interpreter's own rules (`cobolt_runtime::partiql::door`) on the runner's file store, where the
//! program's own files are open too. The host writes the properties the port answers with, in its
//! order, and answers the method. Routed by the control's class, as `Get`, `Execute` and `Fetch`
//! mean something else on other controls. A browser module never reaches this: a page has no file,
//! and a host with no port answers as before.

use super::*;

/// What the host knows of the IndexedFile a method was called on.
pub struct PartiqlControl<'a> {
    /// The control's name in the form.
    pub name: &'a str,
    /// One of its properties, as the program would read it.
    pub prop: &'a dyn Fn(&str) -> String,
}

/// What a method call does: the properties written, in order, and the method's answer.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct PartiqlEffects {
    pub props: Vec<(String, String)>,
    pub answer: String,
}

/// The desktop's IndexedFile control (DESKTOP-WASM-001 T48), which the runner gives the host in
/// [`Host::partiql`].
pub trait PartiqlPort {
    /// A method (`method`, upper case) of the IndexedFile `c` of instance `inst`, with the argument
    /// texts `args`: what follows, or `None` when it is not one of the control's PartiQL members, so
    /// the host's general dispatch answers it.
    fn call(&mut self, inst: u32, c: &PartiqlControl<'_>, method: &str, args: &[&str]) -> Option<PartiqlEffects>;
    /// The instance `inst` ended: what its controls kept goes.
    fn release(&mut self, inst: u32);
}

impl Host {
    /// A method of the control `idx` when that control is an IndexedFile and the host has a port:
    /// `None` otherwise, and for a method that is not one of its PartiQL members.
    pub(super) fn partiql_call(&mut self, inst: u32, idx: usize, name: &str, args: &[&str]) -> Option<Result<Option<String>, CmdError>> {
        let tree = self.trees.get(&inst)?;
        if tree.ctls[idx].kind.type_name() != "IndexedFile" {
            return None;
        }
        let port = self.partiql.as_mut()?;
        let m = name.trim().to_ascii_uppercase();
        let ctl = tree.ctl(idx).name.clone();
        let prop = |p: &str| tree.get(idx, p);
        let fx = port.call(inst, &PartiqlControl { name: &ctl, prop: &prop }, &m, args)?;
        Some(Ok(Some(self.service_apply(inst, idx, &fx.props, &[], fx.answer))))
    }
}
