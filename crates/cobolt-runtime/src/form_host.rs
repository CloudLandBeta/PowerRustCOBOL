// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Spec 037 — the multi-form window supervisor.
//!
//! [`FormSupervisor`] owns every lifecycle rule of the multi-form runtime —
//! handles, the main-form singleton (R10), FormState close vetoes (R17/R19),
//! Sync/Async close cascades (R25–R27) and modal reply deferral (R28) — as
//! **pure state-machine logic**. It never touches a window: every decision
//! comes back as [`HostAction`]s that the GUI host (`rcrun run-form`)
//! executes against real viewports, and every COBOL-side call arrives as a
//! [`FormRequest`] from an interpreter thread. Keeping the rules here makes
//! the whole lifecycle testable headless (`cargo test -p cobolt-runtime`),
//! with the GUI shell reduced to plumbing.
//!
//! Handle ids are opaque strings (`"W1"`, `"W2"`, …; `ROOT_HANDLE` is the
//! form the process started with). A `windowHandler` data-item in COBOL
//! stores exactly this id; the interpreter nulls its variables when a
//! [`HostAction::NotifyClosed`] broadcast names their handle (R24).

use std::collections::HashMap;
use std::sync::mpsc::Sender;

/// The handle of the form the process was started with.
pub const ROOT_HANDLE: &str = "W0";

/// A request from an interpreter thread to the window supervisor.
#[derive(Debug)]
pub enum FormRequest {
    /// `me::"OpenFormSync"` / `me::"OpenFormAsync"` (R20).
    OpenForm {
        /// Handle of the CALLING form's window.
        caller: String,
        /// Target form id ("form id" argument, case-insensitive).
        form_id: String,
        sync: bool,
        /// `None` ⇒ default from the target form's RAD design (R21).
        window_state: Option<String>,
        x: Option<i64>,
        y: Option<i64>,
        width: Option<i64>,
        height: Option<i64>,
        /// Sync only; Async is never modal (R20). Comma-form default: true.
        modal: bool,
        /// Receives the handle id. For a MODAL Sync call the reply is held
        /// until the child closes (R28) and carries `None` — the handle is
        /// already NULL by then (R24).
        reply: Sender<Option<String>>,
    },
    /// A method invoked THROUGH a windowHandler (R23).
    HandleMethod {
        handle: String,
        method: String,
        args: Vec<String>,
        /// `Ok(value)` (empty for actions) or `Err(runtime error text)`.
        reply: Sender<Result<String, String>>,
    },
    /// The calling form asks to close ITSELF (`me::"Close"`).
    CloseSelf { caller: String },
}

/// One window-affecting decision for the GUI host to execute.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HostAction {
    /// Create the window + interpreter for `form_id` under `handle`.
    SpawnWindow {
        handle: String,
        form_id: String,
        window_state: Option<String>,
        x: Option<i64>,
        y: Option<i64>,
        width: Option<i64>,
        height: Option<i64>,
        modal: bool,
    },
    /// Destroy the window under `handle` (its veto checks already passed).
    CloseWindow { handle: String },
    /// Focus (and restore, spec Q3) the window under `handle`.
    FocusWindow { handle: String },
    SetWindowState { handle: String, state: String },
    SetFullScreen { handle: String, on: bool },
    SetTitleVisible { handle: String, on: bool },
    /// Queue `onCloseRejected` on the named form (R17/R19/R27).
    NotifyCloseRejected { handle: String },
    /// Broadcast to every interpreter: handles storing this id become NULL.
    NotifyClosed { handle: String },
    /// Every window is gone — the process may exit (R27).
    Exit,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Kind {
    Root,
    Sync,
    Async,
}

#[derive(Debug)]
struct HandleInfo {
    form_id: String,
    kind: Kind,
    /// Handle of the form that opened this one (`None` for the root).
    caller: Option<String>,
    modal: bool,
    /// Mirrored FormState (R16): true = Waiting, close is vetoed (R17).
    waiting: bool,
    /// Held reply of a MODAL Sync OpenForm, answered on close (R28).
    modal_reply: Option<Sender<Option<String>>>,
}

/// The pure multi-form lifecycle state machine. See the module docs.
pub struct FormSupervisor {
    /// The project's main form id (upper-cased), for the singleton rule.
    main_form_id: String,
    /// Whether the ROOT window is the main form (R27 close-all applies).
    root_is_main: bool,
    handles: HashMap<String, HandleInfo>,
    next_handle: u64,
}

impl FormSupervisor {
    /// `root_form_id` is the form the process started with; `main_form_id`
    /// the project's designated main form (they usually coincide — R5).
    pub fn new(root_form_id: &str, main_form_id: &str) -> Self {
        let mut handles = HashMap::new();
        handles.insert(
            ROOT_HANDLE.to_string(),
            HandleInfo {
                form_id: root_form_id.trim().to_ascii_uppercase(),
                kind: Kind::Root,
                caller: None,
                modal: false,
                waiting: false,
                modal_reply: None,
            },
        );
        Self {
            main_form_id: main_form_id.trim().to_ascii_uppercase(),
            root_is_main: root_form_id.trim().eq_ignore_ascii_case(main_form_id.trim()),
            handles,
            next_handle: 1,
        }
    }

    /// The live handle whose form is `form_id`, if any (singleton lookup).
    fn handle_of_form(&self, form_id: &str) -> Option<String> {
        let want = form_id.trim().to_ascii_uppercase();
        self.handles
            .iter()
            .find(|(_, i)| i.form_id == want)
            .map(|(h, _)| h.clone())
    }

    /// FormState mirror (host taps each interpreter's StateUpdate stream and
    /// forwards form-object `FormState` writes here). R16.
    pub fn note_form_state(&mut self, handle: &str, waiting: bool) {
        if let Some(info) = self.handles.get_mut(handle) {
            info.waiting = waiting;
        }
    }

    /// Is this handle currently live?
    pub fn is_open(&self, handle: &str) -> bool {
        self.handles.contains_key(handle)
    }

    /// The form id under a handle (host uses it to route events).
    pub fn form_id_of(&self, handle: &str) -> Option<&str> {
        self.handles.get(handle).map(|i| i.form_id.as_str())
    }

    /// Live Sync children of `handle`.
    fn sync_children(&self, handle: &str) -> Vec<String> {
        self.handles
            .iter()
            .filter(|(_, i)| i.kind == Kind::Sync && i.caller.as_deref() == Some(handle))
            .map(|(h, _)| h.clone())
            .collect()
    }

    /// The whole Sync subtree of `handle` (children, grandchildren, …).
    fn sync_subtree(&self, handle: &str) -> Vec<String> {
        let mut out = Vec::new();
        let mut stack = self.sync_children(handle);
        while let Some(h) = stack.pop() {
            stack.extend(self.sync_children(&h));
            out.push(h);
        }
        out
    }

    /// First Waiting form in `handles`, if any — the veto carrier.
    fn first_waiting(&self, handles: &[String]) -> Option<String> {
        handles
            .iter()
            .find(|h| self.handles.get(*h).map(|i| i.waiting).unwrap_or(false))
            .cloned()
    }

    /// An interpreter request → host actions. Replies are sent inline.
    pub fn handle_request(&mut self, req: FormRequest) -> Vec<HostAction> {
        match req {
            FormRequest::OpenForm {
                caller,
                form_id,
                sync,
                window_state,
                x,
                y,
                width,
                height,
                modal,
                reply,
            } => self.open_form(
                &caller,
                &form_id,
                sync,
                window_state,
                x,
                y,
                width,
                height,
                modal,
                reply,
            ),
            FormRequest::HandleMethod {
                handle,
                method,
                args,
                reply,
            } => self.handle_method(&handle, &method, &args, reply),
            FormRequest::CloseSelf { caller } => self.try_close(&caller),
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn open_form(
        &mut self,
        caller: &str,
        form_id: &str,
        sync: bool,
        window_state: Option<String>,
        x: Option<i64>,
        y: Option<i64>,
        width: Option<i64>,
        height: Option<i64>,
        modal: bool,
        reply: Sender<Option<String>>,
    ) -> Vec<HostAction> {
        let want_main = form_id.trim().eq_ignore_ascii_case(&self.main_form_id);
        // R10 — the main form is a singleton: focus the running instance and
        // return its existing handle instead of spawning a second one.
        if want_main {
            if let Some(existing) = self.handle_of_form(form_id) {
                let _ = reply.send(Some(existing.clone()));
                return vec![HostAction::FocusWindow { handle: existing }];
            }
        }
        let handle = format!("W{}", self.next_handle);
        self.next_handle += 1;
        let modal = sync && modal; // Async is never modal (R20)
        self.handles.insert(
            handle.clone(),
            HandleInfo {
                form_id: form_id.trim().to_ascii_uppercase(),
                kind: if sync { Kind::Sync } else { Kind::Async },
                caller: Some(caller.to_string()),
                modal,
                waiting: false,
                modal_reply: None,
            },
        );
        if modal {
            // R28 — the caller's COBOL flow resumes when the child closes;
            // hold the reply until then (it will carry None: the handle is
            // NULL once the child is gone, R24).
            if let Some(info) = self.handles.get_mut(&handle) {
                info.modal_reply = Some(reply);
            }
        } else {
            let _ = reply.send(Some(handle.clone()));
        }
        vec![HostAction::SpawnWindow {
            handle,
            form_id: form_id.trim().to_string(),
            window_state,
            x,
            y,
            width,
            height,
            modal,
        }]
    }

    fn handle_method(
        &mut self,
        handle: &str,
        method: &str,
        args: &[String],
        reply: Sender<Result<String, String>>,
    ) -> Vec<HostAction> {
        if !self.handles.contains_key(handle) {
            let _ = reply.send(Err(format!(
                "windowHandler {handle} refers to a closed form"
            )));
            return Vec::new();
        }
        let m = method.trim().to_ascii_uppercase();
        let actions = match m.as_str() {
            "CLOSE" => {
                let acts = self.try_close(handle);
                let _ = reply.send(Ok(String::new()));
                return acts;
            }
            // Spec Q3 — Focus restores a minimized window first.
            "FOCUS" | "SETFOCUS" => vec![
                HostAction::SetWindowState {
                    handle: handle.to_string(),
                    state: "Normal".into(),
                },
                HostAction::FocusWindow {
                    handle: handle.to_string(),
                },
            ],
            "SETWINDOWSTATE" => vec![HostAction::SetWindowState {
                handle: handle.to_string(),
                state: args.first().cloned().unwrap_or_else(|| "Normal".into()),
            }],
            "SETFULLSCREEN" => vec![HostAction::SetFullScreen {
                handle: handle.to_string(),
                on: truthy(args.first().map(String::as_str).unwrap_or("true")),
            }],
            "SETTITLEVISIBLE" => vec![HostAction::SetTitleVisible {
                handle: handle.to_string(),
                on: truthy(args.first().map(String::as_str).unwrap_or("true")),
            }],
            "GETFORMSTATE" | "FORMSTATE" => {
                let waiting = self.handles.get(handle).map(|i| i.waiting).unwrap_or(false);
                let _ = reply.send(Ok(if waiting { "Waiting" } else { "Ready" }.into()));
                return Vec::new();
            }
            other => {
                let _ = reply.send(Err(format!(
                    "windowHandler has no method \u{201c}{other}\u{201d}"
                )));
                return Vec::new();
            }
        };
        let _ = reply.send(Ok(String::new()));
        actions
    }

    /// R17/R19/R25/R27 — the ONE close path. Every close attempt (title-bar
    /// X, handle Close, self Close, cascades) funnels through here.
    pub fn try_close(&mut self, handle: &str) -> Vec<HostAction> {
        if !self.handles.contains_key(handle) {
            return Vec::new();
        }
        let is_root = handle == ROOT_HANDLE;
        let closes_everything = is_root
            && self
                .handles
                .get(handle)
                .map(|i| i.kind == Kind::Root && self.root_is_main)
                .unwrap_or(false);

        // Which windows would this close take down?
        let mut victims: Vec<String> = if closes_everything {
            // R27 — the main form takes every open form with it.
            self.handles.keys().cloned().collect()
        } else {
            // R25 — a caller takes its whole Sync subtree with it.
            let mut v = self.sync_subtree(handle);
            v.push(handle.to_string());
            v
        };

        // R17/R19 — any Waiting form in the victim set vetoes the WHOLE
        // close. The rejection event lands on the blocking form itself when
        // it is the one being closed, and ALSO on the initiating caller so
        // its logic can react (R19: the caller "refuses to close").
        if let Some(blocking) = self.first_waiting(&victims) {
            let mut acts = vec![HostAction::NotifyCloseRejected {
                handle: blocking.clone(),
            }];
            if blocking != handle {
                acts.push(HostAction::NotifyCloseRejected {
                    handle: handle.to_string(),
                });
            }
            return acts;
        }

        // Close order: deepest first (children before callers) so each
        // window's onClose runs while its caller still exists.
        victims.sort_by_key(|h| std::cmp::Reverse(self.depth_of(h)));
        let mut acts = Vec::new();
        for victim in &victims {
            if let Some(info) = self.handles.remove(victim) {
                // R28 — a held modal reply resolves (with NULL) on close.
                if let Some(tx) = info.modal_reply {
                    let _ = tx.send(None);
                }
                acts.push(HostAction::CloseWindow {
                    handle: victim.clone(),
                });
                acts.push(HostAction::NotifyClosed {
                    handle: victim.clone(),
                });
            }
        }
        // R26 — Async children of closed callers survive, detached.
        let gone: Vec<String> = victims;
        for info in self.handles.values_mut() {
            if let Some(c) = &info.caller {
                if gone.contains(c) {
                    info.caller = None;
                }
            }
        }
        if closes_everything || self.handles.is_empty() {
            acts.push(HostAction::Exit);
        }
        acts
    }

    /// A child's interpreter finished on its own (STOP RUN): its window is
    /// already closing — release the handle unconditionally (no veto: the
    /// program CHOSE to stop) and resolve any held modal reply.
    pub fn form_finished(&mut self, handle: &str) -> Vec<HostAction> {
        let Some(info) = self.handles.remove(handle) else {
            return Vec::new();
        };
        if let Some(tx) = info.modal_reply {
            let _ = tx.send(None);
        }
        let mut acts = vec![
            HostAction::CloseWindow {
                handle: handle.to_string(),
            },
            HostAction::NotifyClosed {
                handle: handle.to_string(),
            },
        ];
        // Its Sync subtree goes with it (R25); Async children detach (R26).
        for child in self.sync_subtree(handle) {
            acts.extend(self.try_close(&child));
        }
        for info in self.handles.values_mut() {
            if info.caller.as_deref() == Some(handle) {
                info.caller = None;
            }
        }
        if self.handles.is_empty() {
            acts.push(HostAction::Exit);
        }
        acts
    }

    /// Nesting depth (root = 0) — used to close children before callers.
    fn depth_of(&self, handle: &str) -> usize {
        let mut depth = 0;
        let mut cur = handle.to_string();
        while let Some(caller) = self.handles.get(&cur).and_then(|i| i.caller.clone()) {
            depth += 1;
            cur = caller;
            if depth > 64 {
                break; // cycle guard — cannot happen, but never hang
            }
        }
        depth
    }

    /// Live modal children of `handle` — the GUI blocks the caller's input
    /// while this is non-empty (R28).
    pub fn modal_children_of(&self, handle: &str) -> Vec<String> {
        self.handles
            .iter()
            .filter(|(_, i)| i.modal && i.caller.as_deref() == Some(handle))
            .map(|(h, _)| h.clone())
            .collect()
    }
}

fn truthy(s: &str) -> bool {
    let t = s.trim();
    t == "1" || t.eq_ignore_ascii_case("true") || t.eq_ignore_ascii_case("yes")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc::channel;

    fn open(
        sup: &mut FormSupervisor,
        caller: &str,
        form: &str,
        sync: bool,
        modal: bool,
    ) -> (Option<String>, Vec<HostAction>) {
        let (tx, rx) = channel();
        let acts = sup.handle_request(FormRequest::OpenForm {
            caller: caller.into(),
            form_id: form.into(),
            sync,
            window_state: None,
            x: None,
            y: None,
            width: None,
            height: None,
            modal,
            reply: tx,
        });
        (rx.try_recv().ok().flatten(), acts)
    }

    fn spawned(acts: &[HostAction]) -> Vec<String> {
        acts.iter()
            .filter_map(|a| match a {
                HostAction::SpawnWindow { handle, .. } => Some(handle.clone()),
                _ => None,
            })
            .collect()
    }

    fn closed(acts: &[HostAction]) -> Vec<String> {
        acts.iter()
            .filter_map(|a| match a {
                HostAction::CloseWindow { handle } => Some(handle.clone()),
                _ => None,
            })
            .collect()
    }

    /// R10/R11 — distinct handles per instance; the main form is a
    /// singleton returning its existing handle with a Focus action.
    #[test]
    fn open_form_registry_handles_and_singleton() {
        let mut sup = FormSupervisor::new("MAIN-FORM", "MAIN-FORM");
        let (h1, a1) = open(&mut sup, ROOT_HANDLE, "DETAIL", false, false);
        let (h2, a2) = open(&mut sup, ROOT_HANDLE, "DETAIL", false, false);
        let h1 = h1.expect("handle 1");
        let h2 = h2.expect("handle 2");
        assert_ne!(h1, h2, "two instances, two handles (R11)");
        assert_eq!(spawned(&a1), vec![h1.clone()]);
        assert_eq!(spawned(&a2), vec![h2.clone()]);

        // Opening the MAIN form focuses the running root instead (R10).
        let (hm, am) = open(&mut sup, &h1, "MAIN-FORM", false, false);
        assert_eq!(hm.as_deref(), Some(ROOT_HANDLE));
        assert!(spawned(&am).is_empty(), "no second main instance");
        assert!(
            am.contains(&HostAction::FocusWindow {
                handle: ROOT_HANDLE.into()
            }),
            "singleton focuses the running instance"
        );
        println!("handles: {h1}, {h2}; main-form reopen → {hm:?} + Focus (no spawn)");
    }

    /// R24 — closing a form broadcasts NotifyClosed for handle-NULLing.
    #[test]
    fn open_form_registry_close_broadcasts_null() {
        let mut sup = FormSupervisor::new("MAIN-FORM", "MAIN-FORM");
        let (h, _) = open(&mut sup, ROOT_HANDLE, "DETAIL", false, false);
        let h = h.unwrap();
        let acts = sup.try_close(&h);
        assert_eq!(closed(&acts), vec![h.clone()]);
        assert!(acts.contains(&HostAction::NotifyClosed { handle: h.clone() }));
        assert!(!sup.is_open(&h));
        println!("close {h} → CloseWindow + NotifyClosed broadcast");
    }

    /// R17 — Waiting vetoes every close path and fires onCloseRejected.
    #[test]
    fn form_state_lifecycle_waiting_vetoes_close() {
        let mut sup = FormSupervisor::new("MAIN-FORM", "MAIN-FORM");
        let (h, _) = open(&mut sup, ROOT_HANDLE, "EDITOR", true, false);
        let h = h.unwrap();
        sup.note_form_state(&h, true); // FormState = Waiting
        let acts = sup.try_close(&h);
        assert!(closed(&acts).is_empty(), "close refused");
        assert!(acts.contains(&HostAction::NotifyCloseRejected { handle: h.clone() }));
        assert!(sup.is_open(&h), "still open");
        // Ready again → the same close succeeds (R18).
        sup.note_form_state(&h, false);
        let acts = sup.try_close(&h);
        assert_eq!(closed(&acts), vec![h.clone()]);
        println!("Waiting → veto + onCloseRejected; Ready → closed");
    }

    /// R19 — a Sync caller with a Waiting Sync child refuses to close; the
    /// event lands on the blocking child AND the caller.
    #[test]
    fn form_state_lifecycle_sync_caller_vetoed_by_waiting_child() {
        let mut sup = FormSupervisor::new("MAIN-FORM", "MAIN-FORM");
        let (parent, _) = open(&mut sup, ROOT_HANDLE, "PARENT", true, false);
        let parent = parent.unwrap();
        let (child, _) = open(&mut sup, &parent, "CHILD", true, false);
        let child = child.unwrap();
        sup.note_form_state(&child, true);
        let acts = sup.try_close(&parent);
        assert!(closed(&acts).is_empty());
        assert!(acts.contains(&HostAction::NotifyCloseRejected {
            handle: child.clone()
        }));
        assert!(acts.contains(&HostAction::NotifyCloseRejected {
            handle: parent.clone()
        }));
        // Child Ready → caller close cascades: child closes first, then the
        // caller, and both broadcast NULL (R25 + R24).
        sup.note_form_state(&child, false);
        let acts = sup.try_close(&parent);
        assert_eq!(closed(&acts), vec![child.clone(), parent.clone()]);
        println!("waiting child vetoed caller; ready → cascade child-then-caller");
    }

    /// R26/R27 — Async children survive their caller; the main form's close
    /// takes everything and exits.
    #[test]
    fn form_cascades_async_survival_and_main_close_all() {
        let mut sup = FormSupervisor::new("MAIN-FORM", "MAIN-FORM");
        let (parent, _) = open(&mut sup, ROOT_HANDLE, "PARENT", false, false);
        let parent = parent.unwrap();
        let (orphan, _) = open(&mut sup, &parent, "FREE", false, false);
        let orphan = orphan.unwrap();
        let acts = sup.try_close(&parent);
        assert_eq!(closed(&acts), vec![parent.clone()]);
        assert!(sup.is_open(&orphan), "async child survives (R26)");

        // Main-form close: everything goes, Exit is emitted (R27).
        let acts = sup.try_close(ROOT_HANDLE);
        let mut victims = closed(&acts);
        victims.sort();
        let mut expect = vec![ROOT_HANDLE.to_string(), orphan.clone()];
        expect.sort();
        assert_eq!(victims, expect);
        assert!(acts.contains(&HostAction::Exit));
        println!("async survived caller close; main close took {expect:?} + Exit");
    }

    /// R27 + R17 — a Waiting form vetoes even the main form's close-all.
    #[test]
    fn form_cascades_waiting_form_vetoes_main_close() {
        let mut sup = FormSupervisor::new("MAIN-FORM", "MAIN-FORM");
        let (h, _) = open(&mut sup, ROOT_HANDLE, "EDITOR", false, false);
        let h = h.unwrap();
        sup.note_form_state(&h, true);
        let acts = sup.try_close(ROOT_HANDLE);
        assert!(closed(&acts).is_empty(), "main close refused");
        assert!(acts.contains(&HostAction::NotifyCloseRejected { handle: h.clone() }));
        assert!(sup.is_open(ROOT_HANDLE) && sup.is_open(&h));
        println!("main close vetoed by Waiting {h}; both still open");
    }

    /// R28 — a modal Sync OpenForm's reply is HELD until the child closes.
    #[test]
    fn form_cascades_modal_reply_deferred_until_close() {
        let mut sup = FormSupervisor::new("MAIN-FORM", "MAIN-FORM");
        let (tx, rx) = channel();
        let acts = sup.handle_request(FormRequest::OpenForm {
            caller: ROOT_HANDLE.into(),
            form_id: "DIALOG".into(),
            sync: true,
            window_state: None,
            x: None,
            y: None,
            width: None,
            height: None,
            modal: true,
            reply: tx,
        });
        let handle = spawned(&acts)[0].clone();
        assert!(
            rx.try_recv().is_err(),
            "modal reply must NOT arrive before the child closes"
        );
        assert_eq!(sup.modal_children_of(ROOT_HANDLE), vec![handle.clone()]);
        let _ = sup.try_close(&handle);
        assert_eq!(
            rx.try_recv().expect("reply after close"),
            None,
            "modal reply resolves to NULL on close (R24)"
        );
        println!("modal {handle}: reply held while open, NULL on close");
    }

    /// R23 — handle methods route to actions; unknown/closed handles error.
    #[test]
    fn window_commands_handle_methods() {
        let mut sup = FormSupervisor::new("MAIN-FORM", "MAIN-FORM");
        let (h, _) = open(&mut sup, ROOT_HANDLE, "EDITOR", false, false);
        let h = h.unwrap();

        let (tx, rx) = channel();
        let acts = sup.handle_request(FormRequest::HandleMethod {
            handle: h.clone(),
            method: "SetFullScreen".into(),
            args: vec!["true".into()],
            reply: tx,
        });
        assert!(rx.try_recv().unwrap().is_ok());
        assert!(acts.contains(&HostAction::SetFullScreen {
            handle: h.clone(),
            on: true
        }));

        // Focus restores first (spec Q3).
        let (tx, _rx) = channel();
        let acts = sup.handle_request(FormRequest::HandleMethod {
            handle: h.clone(),
            method: "Focus".into(),
            args: vec![],
            reply: tx,
        });
        assert_eq!(
            acts,
            vec![
                HostAction::SetWindowState {
                    handle: h.clone(),
                    state: "Normal".into()
                },
                HostAction::FocusWindow { handle: h.clone() },
            ]
        );

        // FormState read-through.
        sup.note_form_state(&h, true);
        let (tx, rx) = channel();
        sup.handle_request(FormRequest::HandleMethod {
            handle: h.clone(),
            method: "GetFormState".into(),
            args: vec![],
            reply: tx,
        });
        assert_eq!(rx.try_recv().unwrap().unwrap(), "Waiting");

        // A closed handle errors (the interpreter surfaces it).
        sup.note_form_state(&h, false);
        let _ = sup.try_close(&h);
        let (tx, rx) = channel();
        sup.handle_request(FormRequest::HandleMethod {
            handle: h.clone(),
            method: "Focus".into(),
            args: vec![],
            reply: tx,
        });
        assert!(rx.try_recv().unwrap().is_err());
        println!("handle methods: SetFullScreen/Focus(restore-first)/GetFormState ok; closed handle errors");
    }
}
