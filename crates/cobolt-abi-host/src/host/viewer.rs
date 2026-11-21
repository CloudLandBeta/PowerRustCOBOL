// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! **The Viewer on the desktop** (DESKTOP-WASM-001 T50).
//!
//! A program built on the WebAssembly path loads documents into a Viewer, writes its conversation,
//! finds in it, saves, prints and shares it by the interpreter's own rules (`cobolt_runtime::viewer_door`),
//! which the runner's [`ViewerPort`] applies to the control through a [`ViewerAccess`]. The host hands
//! the port what the interpreter hands its rules:
//! - a method of the Viewer itself;
//! - the program's `SetProperty` on a Viewer, which loads a written source, as the interpreter's
//!   own write does;
//! - a `MOVE` to a Viewer's member, which only keeps the other spelling of the property in step;
//! - what the window sends back from the toolbar and from the operating system.
//!
//! A browser module never reaches this: there is no port, and the page keeps the conversation
//! (`methods.rs`).

use super::*;

/// One Viewer of one form instance, as the runner's port reads and writes it.
pub trait ViewerAccess {
    /// The control's name in the form.
    fn name(&self) -> &str;
    /// A property, as the program would read it.
    fn get(&self, prop: &str) -> String;
    /// A program's write, which reaches the window.
    fn set(&mut self, prop: &str, value: &str);
    /// A write the window is not told of ([`FormTree::put_quiet`]).
    fn put_quiet(&mut self, prop: &str, value: &str);
    /// Raise one of the control's events, for its handler when one is bound.
    fn event(&mut self, name: &str);
}

/// The desktop's Viewer (DESKTOP-WASM-001 T50), which the runner gives the host in [`Host::viewer`]. With
/// none, a desktop host answers as a page does.
pub trait ViewerPort {
    /// A method (`method`, any case) of the Viewer `v` of instance `inst`, with the argument texts
    /// `args`: `Some` with its answer (`None` for a method that answers nothing), or `None` when it is
    /// not one of the Viewer's own methods, so the host's general dispatch answers it.
    fn call(&mut self, inst: u32, v: &mut dyn ViewerAccess, method: &str, args: &[&str]) -> Option<Option<String>>;
    /// The program wrote `prop` of the Viewer `v` (the value is already stored): through `SetProperty`
    /// (`load`), where a source starts a load, or through a `MOVE` to the control's member, where the
    /// other spelling of the property is only kept in step.
    fn written(&mut self, inst: u32, v: &mut dyn ViewerAccess, prop: &str, load: bool);
    /// The window wrote `prop` (`value`) of the Viewer `v` (already stored): `true` when it was one of
    /// the toolbar's or the operating system's answers, and has been carried out.
    fn input(&mut self, inst: u32, v: &mut dyn ViewerAccess, prop: &str, value: &str) -> bool;
    /// The instance `inst` ended: what its Viewers kept goes.
    fn release(&mut self, inst: u32);
}

/// A Viewer of a form the host holds.
struct On<'a> {
    host: &'a mut Host,
    inst: u32,
    idx: usize,
    name: String,
}

impl ViewerAccess for On<'_> {
    fn name(&self) -> &str {
        &self.name
    }
    fn get(&self, prop: &str) -> String {
        self.host.trees.get(&self.inst).map(|t| t.get(self.idx, prop)).unwrap_or_default()
    }
    fn set(&mut self, prop: &str, value: &str) {
        let Some(tree) = self.host.trees.get_mut(&self.inst) else { return };
        let events = tree.set(self.idx, prop, value);
        self.host.enqueue_requests(self.inst, events);
    }
    fn put_quiet(&mut self, prop: &str, value: &str) {
        if let Some(tree) = self.host.trees.get_mut(&self.inst) {
            tree.put_quiet(self.idx, prop, value);
        }
    }
    fn event(&mut self, name: &str) {
        self.host.dom_event(self.inst, self.idx, name, Vec::new());
    }
}

impl Host {
    /// Run `f` with the port on the Viewer `idx` of instance `inst`: `None` when there is no port, or
    /// the control is not a Viewer.
    fn with_viewer<R>(&mut self, inst: u32, idx: usize, f: impl FnOnce(&mut dyn ViewerPort, &mut On<'_>) -> R) -> Option<R> {
        let tree = self.trees.get(&inst)?;
        let c = tree.ctls.get(idx)?;
        if !methods::is_class(c, ControlType::Viewer) {
            return None;
        }
        let name = c.name.clone();
        let mut port = self.viewer.take()?;
        let r = f(port.as_mut(), &mut On { host: self, inst, idx, name });
        self.viewer = Some(port);
        Some(r)
    }

    /// A method of the control `idx` when it is a Viewer and the method is one of its own: the port's.
    /// `None` for any other control or method, and with no port.
    pub(super) fn viewer_call(&mut self, inst: u32, idx: usize, name: &str, args: &[&str]) -> Option<Result<Option<String>, CmdError>> {
        self.with_viewer(inst, idx, |port, v| port.call(inst, v, name, args)).flatten().map(Ok)
    }

    /// After the general dispatch carried out `name` on the control `idx`: a `SetProperty` that wrote a
    /// Viewer's property is a write as the interpreter's `obj_set` makes it, so a source starts a load.
    pub(super) fn viewer_set_property(&mut self, inst: u32, idx: usize, name: &str, args: &[&str]) {
        if self.viewer.is_none() || !name.trim().eq_ignore_ascii_case("SetProperty") {
            return;
        }
        let Some(tree) = self.trees.get(&inst) else { return };
        let (target, key) = methods::resolve_ref(tree, idx, args.first().map(|s| s.trim()).unwrap_or(""));
        self.with_viewer(inst, target, |port, v| port.written(inst, v, &key, true));
    }

    /// The program's `MOVE` to the member `prop` of the control `idx` (already stored): on a Viewer, the
    /// other spelling of the property is kept in step.
    pub(super) fn viewer_member_written(&mut self, inst: u32, idx: usize, prop: &str) {
        self.with_viewer(inst, idx, |port, v| port.written(inst, v, prop, false));
    }

    /// The window wrote `prop` (`value`) of the control `idx` of instance `inst`, already stored with
    /// [`Host::ui_set`]: when the control is a Viewer and the write is its toolbar's or the operating
    /// system's answer, it is carried out as the interpreter's input loop carries it out. The runner
    /// calls it after the window's writes are taken in, so what it writes reaches the window.
    pub fn viewer_input(&mut self, inst: u32, idx: usize, prop: &str, value: &str) -> bool {
        self.with_viewer(inst, idx, |port, v| port.input(inst, v, prop, value)).unwrap_or(false)
    }
}
