// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! WASM-014 T15 (R13, R14): what the host does with the connection's progress and stream frames. A form declares, before a call, the
//! controls that show its progress (D-Progress); each progress frame sets them without running any COBOL, and a click on the cancel
//! control asks for the call to be cancelled. A stream the form subscribed to raises `onStreamItem` / `onStreamEnd` on that form,
//! queued behind whatever the instance is running, and the subscriptions of an instance end with it.

use super::*;

/// The controls a call's progress is shown in, by index in the instance's tree.
#[derive(Debug, Clone, Default)]
pub(super) struct ProgressTargets {
    pub inst: u32,
    pub progress: Option<usize>,
    pub label: Option<usize>,
    pub cancel: Option<usize>,
}

impl Host {
    fn control_named(&self, inst: u32, name: &str) -> Option<usize> {
        let n = name.trim();
        if n.is_empty() {
            return None;
        }
        self.trees.get(&inst)?.ctls.iter().position(|c| c.name.eq_ignore_ascii_case(n))
    }

    /// `WS-SHOW-PROGRESS` before call `call`: where its progress is shown. A name the form does not have is refused (the call then
    /// fails `validation` in the module, naming it).
    pub fn show_progress(&mut self, call: u32, inst: u32, progress: &str, label: &str, cancel: &str) -> Result<(), String> {
        let mut t = ProgressTargets { inst, ..ProgressTargets::default() };
        for (what, name, slot) in [("progress control", progress, &mut t.progress), ("label control", label, &mut t.label), ("cancel control", cancel, &mut t.cancel)] {
            if name.trim().is_empty() {
                continue;
            }
            *slot = Some(self.control_named(inst, name).ok_or_else(|| format!("the {what} {name} is not a control of this form"))?);
        }
        self.progress_shown.insert(call, t);
        Ok(())
    }

    /// A progress frame for call `call`: the bar's `Value` and the label's `Caption` are set; no COBOL runs.
    pub fn call_progress(&mut self, call: u32, percent: u8, message: &str) {
        let Some(t) = self.progress_shown.get(&call).cloned() else { return };
        if let Some(c) = t.progress {
            self.ui_set(t.inst, c, "Value", &percent.to_string());
        }
        if let Some(c) = t.label {
            self.ui_set(t.inst, c, "Caption", message);
        }
    }

    /// Call `call` has been answered: its declaration is spent.
    pub fn call_finished(&mut self, call: u32) {
        self.progress_shown.remove(&call);
    }

    /// The user clicked control `ctl` of instance `inst`: the call whose declared cancel control it is, if any (the page then cancels it).
    pub fn cancel_clicked(&self, inst: u32, ctl: usize) -> Option<u32> {
        self.progress_shown.iter().find(|(_, t)| t.inst == inst && t.cancel == Some(ctl)).map(|(c, _)| *c)
    }

    /// Instance `inst` subscribed to stream `sub`.
    pub fn stream_owner(&mut self, sub: u32, inst: u32) {
        self.stream_owners.insert(sub, inst);
    }

    /// An item of stream `sub`: `onStreamItem` on the form that subscribed, with the subscription's id, the item's JSON and the
    /// restart flag (`Y` on the first item after a re-subscription). Answers whether it was queued (a handler is bound).
    pub fn stream_item(&mut self, sub: u32, json: &str, restart: bool) -> bool {
        let Some(inst) = self.stream_owners.get(&sub).copied() else { return false };
        self.stream_items.insert(sub, (json.to_owned(), restart));
        self.dom_event(inst, 0, ON_STREAM_ITEM, vec![sub.to_string(), json.to_owned(), if restart { "Y" } else { "N" }.to_owned()])
    }

    /// The latest item of stream `sub` and its restart flag, for `WS-STREAM-ITEM`.
    pub fn stream_latest(&self, sub: u32) -> Option<&(String, bool)> {
        self.stream_items.get(&sub)
    }

    /// Stream `sub` ended: `onStreamEnd` with `sub⇥reason`.
    pub fn stream_end(&mut self, sub: u32, reason: &str) -> bool {
        let Some(inst) = self.stream_owners.remove(&sub) else { return false };
        self.dom_event(inst, 0, ON_STREAM_END, vec![sub.to_string(), reason.to_owned()])
    }

    /// The subscriptions of instances that have closed since the last call: the page unsubscribes each, so the backend keeps nothing for them.
    pub fn take_closed_streams(&mut self) -> Vec<u32> {
        std::mem::take(&mut self.closed_streams)
    }

    /// Instance `inst` is going away: its subscriptions and progress declarations go with it.
    pub(super) fn drop_streams_of(&mut self, inst: u32) {
        let subs: Vec<u32> = self.stream_owners.iter().filter(|(_, i)| **i == inst).map(|(s, _)| *s).collect();
        for s in subs {
            self.stream_owners.remove(&s);
            self.stream_items.remove(&s);
            self.closed_streams.push(s);
        }
        self.progress_shown.retain(|_, t| t.inst != inst);
    }
}
