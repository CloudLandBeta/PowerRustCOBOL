// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! The desktop's Viewer (DESKTOP-WASM-001 T50).
//!
//! Every decision is the interpreter's own ([`viewer_door`]): what loading a source writes and raises, what
//! each method does to the conversation, the find bar, a save and a hand-off to the operating system, and
//! what the window's answers do. The port keeps what a Viewer holds that is not a property (the bytes
//! `LoadBytes` gave it, its conversation and its history), one per control, as each form's interpreter keeps
//! its own.

use std::collections::HashMap;

use cobolt_abi_host::host::{ViewerAccess, ViewerPort};
use cobolt_runtime::viewer_door::{self, ViewerSide, ViewerState};

/// The runner's Viewers: what each control keeps, by `(instance, CONTROL)`.
#[derive(Default)]
pub(crate) struct Viewers {
    kept: HashMap<(u32, String), ViewerState>,
}

impl Viewers {
    /// Run `f` on the Viewer `v` of instance `inst`, with what it keeps.
    fn with<R>(&mut self, inst: u32, v: &mut dyn ViewerAccess, loading: bool, f: impl FnOnce(&mut ViewerState, &mut Side<'_>) -> R) -> R {
        let key = (inst, v.name().trim().to_ascii_uppercase());
        let mut state = self.kept.remove(&key).unwrap_or_default();
        let r = f(&mut state, &mut Side { v, loading });
        self.kept.insert(key, state);
        r
    }
}

impl ViewerPort for Viewers {
    fn call(&mut self, inst: u32, v: &mut dyn ViewerAccess, method: &str, args: &[&str]) -> Option<Option<String>> {
        let m = method.trim().to_ascii_uppercase();
        let texts: Vec<String> = args.iter().map(|s| (*s).to_owned()).collect();
        self.with(inst, v, false, |state, side| viewer_door::method(state, side, &m, &texts))
    }

    fn written(&mut self, inst: u32, v: &mut dyn ViewerAccess, prop: &str, load: bool) {
        let value = v.get(prop);
        self.with(inst, v, true, |_, side| {
            if load {
                viewer_door::written(side, prop, &value);
            } else {
                viewer_door::mirror_alias(side, prop, &value);
            }
        });
    }

    fn input(&mut self, inst: u32, v: &mut dyn ViewerAccess, prop: &str, value: &str) -> bool {
        self.with(inst, v, false, |state, side| viewer_door::input(state, side, prop, value))
    }

    fn release(&mut self, inst: u32) {
        self.kept.retain(|(i, _), _| *i != inst);
    }
}

/// A Viewer as the rules see it in a built program. A write the rules make is a write as the
/// interpreter's `obj_set` makes it: while no load or mirror is running, a source written starts a load
/// and an aliased view property is mirrored, as the interpreter's write hook does.
struct Side<'a> {
    v: &'a mut dyn ViewerAccess,
    /// A load or a mirror is running: its own writes do not come back to the hook.
    loading: bool,
}

impl ViewerSide for Side<'_> {
    fn name(&self) -> &str {
        self.v.name()
    }
    fn get(&self, prop: &str) -> String {
        self.v.get(prop)
    }
    fn get_opt(&self, prop: &str) -> Option<String> {
        // A property the program never wrote reads as the control's seeded default, as the interpreter's
        // registry holds it.
        Some(self.v.get(prop))
    }
    fn set(&mut self, prop: &str, value: String) {
        self.v.set(prop, &value);
        if !self.loading {
            self.loading = true;
            let stored = self.v.get(prop);
            viewer_door::written(self, prop, &stored);
            self.loading = false;
        }
    }
    fn put_quiet(&mut self, prop: &str, value: String) {
        self.v.put_quiet(prop, &value);
    }
    fn event(&mut self, name: &str) {
        self.v.event(name);
    }
}
