// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Interactive Grace session (spec 029 Phase C).
//!
//! Runs one [`crate::grace_host::run_grace_workflow`] on a worker thread and
//! streams its progress lines back to the UI through a channel, so the egui
//! frame never blocks on the (network-bound, multi-round) orchestration. The
//! panel drains [`Self::poll`] each frame and renders [`Self::log`]; when the
//! run finishes, [`Self::finished`] carries the workflow status + record path.

use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, Sender};

use cobolt_agents::grace::WorkflowRecord;

use crate::git_exec::GitConfirmRequest;
use crate::llm::LlmConfig;
use crate::llm::{ChatTurn, LlmResponse};
use crate::target_select::{TargetChoice, TargetRequest};

enum GraceMsg {
    Progress(String),
    /// A typed live-action status entry (spec 036): what an agent is doing,
    /// never the content it produced or consumed.
    Action(crate::agent_actions::AgentAction),
    /// A gated git op is waiting for the operator (spec 030 R12). The worker
    /// blocks on `reply` until the UI answers (or drops it → deny).
    Confirm(GitConfirmRequest, Sender<bool>),
    /// A create/edit needs the developer to pick a target on the project tree
    /// (spec 034). The worker blocks on `reply` until the UI answers; a dropped
    /// channel (or a `None` reply) means the developer cancelled.
    SelectTarget(TargetRequest, Sender<Option<TargetChoice>>),
    Done(Result<(WorkflowRecord, PathBuf), String>),
}

/// A running (or finished) Grace workflow, owned by the UI.
pub struct GraceSession {
    pub request: String,
    pub log: Vec<String>,
    /// Complete, unthrottled live-action history (spec 036 R3/R6): the
    /// collapsed-history source. Display throttling happens in
    /// [`Self::current_action`], never here — no action is ever dropped.
    pub actions: Vec<crate::agent_actions::AgentAction>,
    /// Display-side throttle for the single live line (≤1 change/second).
    throttle: crate::agent_actions::ActionThrottle,
    rx: Option<Receiver<GraceMsg>>,
    finished: Option<Result<(WorkflowRecord, PathBuf), String>>,
    /// A gated git op awaiting the operator's Approve/Deny (spec 030 R12).
    pending_confirm: Option<(GitConfirmRequest, Sender<bool>)>,
    /// A create/edit awaiting the developer's target pick on the tree (spec 034).
    pending_select: Option<(TargetRequest, Sender<Option<TargetChoice>>)>,
    /// Live stop flag + token totals shared with the worker.
    control: crate::grace_host::WorkflowControl,
}

impl GraceSession {
    /// Spawn a workflow for `request` on a worker thread.
    pub fn spawn(project_dir: &Path, llm: &LlmConfig, request: &str) -> Self {
        Self::spawn_with_context(
            project_dir,
            llm,
            request,
            crate::grace_host::GraceRoutingContext::default(),
        )
    }

    /// Spawn a workflow carrying an advisory preference from its chatbot
    /// surface. Grace can still route to every enabled project specialist.
    pub fn spawn_with_context(
        project_dir: &Path,
        llm: &LlmConfig,
        request: &str,
        routing: crate::grace_host::GraceRoutingContext,
    ) -> Self {
        let (tx, rx) = mpsc::channel();
        let dir = project_dir.to_path_buf();
        let llm = llm.clone();
        let req = request.to_string();
        let control = crate::grace_host::WorkflowControl::default();
        let worker_control = control.clone();
        std::thread::Builder::new()
            .name("grace-workflow".into())
            .spawn(move || {
                let tx2 = tx.clone();
                let mut on_progress = move |line: String| {
                    // Spec 036 R4: the chat panes render only the typed
                    // action stream, so the full progress trace keeps its
                    // home in the IDE's AI log (Output panel) instead.
                    crate::llm::push_ai_log(crate::llm::AiLogKind::Detail, line.clone());
                    let _ = tx2.send(GraceMsg::Progress(line));
                };
                let action_tx = tx.clone();
                let mut on_action = move |action: crate::agent_actions::AgentAction| {
                    let _ = action_tx.send(GraceMsg::Action(action));
                };
                // Gated git ops block here until the UI answers; a dropped reply
                // channel (session dismissed) counts as a deny (spec 030 R12).
                let tx3 = tx.clone();
                let mut confirm = move |req: GitConfirmRequest| -> bool {
                    let (rtx, rrx) = mpsc::channel();
                    if tx3.send(GraceMsg::Confirm(req, rtx)).is_err() {
                        return false;
                    }
                    rrx.recv().unwrap_or(false)
                };
                // Target-selection handshake (spec 034): send the request to the
                // UI and block until the developer picks or cancels. A dropped
                // channel counts as a cancel.
                let tx4 = tx.clone();
                let mut select_target = move |req: TargetRequest| -> Option<TargetChoice> {
                    let (rtx, rrx) = mpsc::channel();
                    if tx4.send(GraceMsg::SelectTarget(req, rtx)).is_err() {
                        return None;
                    }
                    rrx.recv().unwrap_or(None)
                };
                let result = crate::grace_host::run_grace_workflow_with_control(
                    &dir,
                    &llm,
                    &req,
                    &routing,
                    &worker_control,
                    &mut on_progress,
                    &mut on_action,
                    &mut confirm,
                    &mut select_target,
                );
                let _ = tx.send(GraceMsg::Done(result));
            })
            .expect("failed to spawn grace-workflow thread");
        Self {
            request: request.to_string(),
            log: vec!["Grace received the request.".into()],
            actions: Vec::new(),
            throttle: crate::agent_actions::ActionThrottle::default(),
            rx: Some(rx),
            finished: None,
            pending_confirm: None,
            pending_select: None,
            control,
        }
    }

    /// Ask the running workflow to stop: every model call from now on refuses
    /// immediately, so the run winds down as soon as the in-flight provider
    /// request returns.
    pub fn stop(&self) {
        self.control.request_stop();
    }

    /// Whether the developer already pressed Stop.
    pub fn stop_requested(&self) -> bool {
        self.control.stop_requested()
    }

    /// Exact (input, output) token totals accumulated so far — updated the
    /// moment each model returns.
    pub fn token_totals(&self) -> (u64, u64) {
        self.control.token_totals()
    }

    /// Live Knowledge Base chunk-embedding progress — `(done, total,
    /// current_subject)` while records are being embedded, `None` otherwise.
    pub fn indexing_progress(&self) -> Option<(u64, u64, String)> {
        self.control.indexing_progress()
    }

    /// Drain progress + completion. Returns `true` if anything changed this
    /// frame (so the caller can request a repaint).
    pub fn poll(&mut self) -> bool {
        let mut changed = false;
        let Some(rx) = self.rx.as_ref() else {
            return false;
        };
        loop {
            match rx.try_recv() {
                Ok(GraceMsg::Progress(line)) => {
                    self.log.push(line);
                    changed = true;
                }
                Ok(GraceMsg::Action(action)) => {
                    self.actions.push(action);
                    changed = true;
                }
                Ok(GraceMsg::Confirm(req, reply)) => {
                    self.log
                        .push(format!("⏸ awaiting approval: {}", req.command));
                    self.pending_confirm = Some((req, reply));
                    changed = true;
                }
                Ok(GraceMsg::SelectTarget(req, reply)) => {
                    self.log
                        .push(format!("⏸ awaiting target selection: {}", req.name));
                    self.pending_select = Some((req, reply));
                    changed = true;
                }
                Ok(GraceMsg::Done(result)) => {
                    self.finished = Some(result);
                    self.rx = None;
                    changed = true;
                    break;
                }
                Err(mpsc::TryRecvError::Empty) => break,
                Err(mpsc::TryRecvError::Disconnected) => {
                    if self.finished.is_none() {
                        self.finished = Some(Err("Grace's worker stopped unexpectedly.".into()));
                    }
                    self.rx = None;
                    changed = true;
                    break;
                }
            }
        }
        changed
    }

    pub fn is_running(&self) -> bool {
        self.finished.is_none()
    }

    /// The throttled current-action line (spec 036 R1/R6): the visible line
    /// changes at most once per second, coalescing to the latest action; the
    /// full [`Self::actions`] history keeps every entry.
    pub fn current_action(&mut self) -> Option<&crate::agent_actions::AgentAction> {
        let now_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0);
        let idx = self.throttle.visible_index(self.actions.len(), now_ms)?;
        self.actions.get(idx)
    }

    /// The gated git op awaiting the operator's decision, if any (spec 030 R12).
    pub fn pending_confirm(&self) -> Option<&GitConfirmRequest> {
        self.pending_confirm.as_ref().map(|(r, _)| r)
    }

    /// Answer the pending git confirmation. Approve → the op runs; deny → it is
    /// skipped and the task fails with an evidenced "operator declined".
    pub fn respond_confirm(&mut self, approved: bool) {
        if let Some((_, reply)) = self.pending_confirm.take() {
            let _ = reply.send(approved);
        }
    }

    /// The create/edit target request awaiting the developer's pick, if any
    /// (spec 034).
    pub fn pending_select(&self) -> Option<&TargetRequest> {
        self.pending_select.as_ref().map(|(r, _)| r)
    }

    /// Answer the pending target selection. `Some(choice)` proceeds against the
    /// chosen target; `None` cancels the operation.
    pub fn respond_select(&mut self, choice: Option<TargetChoice>) {
        if let Some((_, reply)) = self.pending_select.take() {
            let _ = reply.send(choice);
        }
    }

    /// The completed result, once available.
    pub fn finished(&self) -> Option<&Result<(WorkflowRecord, PathBuf), String>> {
        self.finished.as_ref()
    }
}

#[cfg(test)]
mod handshake_tests {
    use super::*;
    use crate::project_model::FileKind;
    use crate::target_select::TargetOp;

    fn request() -> TargetRequest {
        TargetRequest {
            op: TargetOp::Create,
            kind: FileKind::Form,
            name: "order".into(),
            candidates: Vec::new(),
        }
    }

    /// The worker's `select_target` closure sends a request and blocks; the UI
    /// side answers, and the choice comes back (spec 034).
    #[test]
    fn select_target_roundtrip_returns_choice() {
        let (tx, rx) = mpsc::channel::<GraceMsg>();
        let mut select_target = move |req: TargetRequest| -> Option<TargetChoice> {
            let (rtx, rrx) = mpsc::channel();
            if tx.send(GraceMsg::SelectTarget(req, rtx)).is_err() {
                return None;
            }
            rrx.recv().unwrap_or(None)
        };
        let worker = std::thread::spawn(move || select_target(request()));
        // UI side: receive the request and respond with a chosen folder.
        match rx.recv().unwrap() {
            GraceMsg::SelectTarget(req, reply) => {
                assert_eq!(req.name, "order");
                reply
                    .send(Some(TargetChoice {
                        rel_path: "forms/customers".into(),
                    }))
                    .unwrap();
            }
            _ => panic!("expected SelectTarget"),
        }
        assert_eq!(
            worker.join().unwrap(),
            Some(TargetChoice {
                rel_path: "forms/customers".into()
            })
        );
    }

    /// A dropped UI channel (session dismissed) is a cancel → `None`.
    #[test]
    fn dropped_channel_cancels() {
        let (tx, rx) = mpsc::channel::<GraceMsg>();
        drop(rx);
        let mut select_target = move |req: TargetRequest| -> Option<TargetChoice> {
            let (rtx, rrx) = mpsc::channel();
            if tx.send(GraceMsg::SelectTarget(req, rtx)).is_err() {
                return None;
            }
            rrx.recv().unwrap_or(None)
        };
        assert_eq!(select_target(request()), None);
    }
}

/// Run Grace behind an existing chatbot's `LlmResponse` channel. This lets the
/// editor and Form Designer retain their transcript/change-set UI while Grace
/// performs the contextual multi-agent routing. Potentially destructive git
/// operations are denied because these compact chat surfaces have no approval
/// prompt; the full project Grace chat provides that prompt.
pub fn spawn_contextual_request(
    project_dir: &Path,
    llm: &LlmConfig,
    history: &[ChatTurn],
    request: &str,
    surface: &str,
    preferred_specialist: Option<&str>,
    context: &str,
    tr: crate::i18n::Tr,
) -> Receiver<LlmResponse> {
    let (tx, rx) = mpsc::channel();
    let dir = project_dir.to_path_buf();
    let llm = llm.clone();
    let request = request.to_string();
    let preferred = preferred_specialist.map(str::to_owned);
    let transcript_len = history
        .last()
        .filter(|turn| turn.role == "user" && turn.content == request)
        .map(|_| history.len().saturating_sub(1))
        .unwrap_or(history.len());
    let transcript = history[..transcript_len]
        .iter()
        .map(|turn| format!("{}: {}", turn.role, turn.content))
        .collect::<Vec<_>>()
        .join("\n\n");
    let context = if transcript.is_empty() {
        context.to_string()
    } else {
        format!("{context}\n\nCONVERSATION SO FAR:\n{transcript}")
    };
    let routing =
        crate::grace_host::GraceRoutingContext::new(surface, preferred.as_deref(), context);
    std::thread::Builder::new()
        .name("grace-chat-request".into())
        .spawn(move || {
            let progress_tx = tx.clone();
            let mut on_progress = move |line: String| {
                let _ = progress_tx.send(LlmResponse::Chunk(format!("{line}\n")));
            };
            let mut deny_unattended_git = |_req: GitConfirmRequest| false;
            // Compact chat surfaces have no modal host, so a create/edit that
            // needs a target pick cannot be disambiguated here: cancel it and let
            // Grace report that the full project Grace chat is required (spec 034).
            let mut no_target_picker = |_req: TargetRequest| None;
            match crate::grace_host::run_grace_workflow_with_context(
                &dir,
                &llm,
                &request,
                &routing,
                &mut on_progress,
                &mut deny_unattended_git,
                &mut no_target_picker,
            ) {
                Ok((record, _)) => {
                    // The contextual RAD-designer chat applies edits by PARSING
                    // the reply text (designer.rs `parse_change_set`). The readable
                    // workflow summary has the change-set operations stripped to
                    // prose, so nothing would apply. When the workflow approved a
                    // Form Designer change-set, send its RAW submission (which
                    // carries the `{"operations":…}` block) so the designer parses
                    // and applies it — this is what makes the agent's control
                    // moves actually take effect (and animate). Otherwise fall
                    // back to the readable summary. (The project-wide Grace chat
                    // applies via `approved_form_change_sets` on the record; the
                    // contextual path has no record on the main thread, so the
                    // change-set must ride the reply.)
                    let change_set_reply = crate::grace_host::approved_form_change_set_submission(
                        &record,
                        crate::agents_db::FORM_DESIGNER,
                    )
                    .map(|submission| {
                        // The designer's "Applied N changes." balloon is built
                        // from the change-set NOTE (everything outside the
                        // fenced JSON is discarded by the parser), so the
                        // run-statistics footer must ride inside the note to
                        // reach the final balloon.
                        match (
                            crate::agent::parse_change_set(&submission),
                            crate::grace_host::statistics_footer(&record, &tr),
                        ) {
                            (Ok(mut cs), Some(stats)) => {
                                cs.note = Some(match cs.note {
                                    Some(note) if !note.trim().is_empty() => {
                                        format!("{note}\n\n{stats}")
                                    }
                                    _ => stats,
                                });
                                serde_json::to_string_pretty(&cs)
                                    .map(|json| format!("```json\n{json}\n```"))
                                    .unwrap_or(submission)
                            }
                            _ => submission,
                        }
                    });
                    let reply = change_set_reply.unwrap_or_else(|| {
                        let mut summary = crate::grace_host::workflow_chat_reply(
                            &record,
                            preferred.as_deref(),
                            llm.verbose_log,
                            &tr,
                        );
                        // An approved task that produced no operation changes
                        // nothing. Say so: reporting it as a plain success is
                        // how a reviewed handler vanished silently, leaving the
                        // developer to discover the form was untouched.
                        let barren = crate::grace_host::approved_form_tasks_without_operations(
                            &record,
                            crate::agents_db::FORM_DESIGNER,
                        );
                        if !barren.is_empty() {
                            summary.push_str(&format!(
                                "\n\n\u{26a0} {} was approved but returned no change-set operation, so the form is unchanged. Nothing was applied.",
                                barren.join(", ")
                            ));
                        }
                        summary
                    });
                    let _ = tx.send(LlmResponse::Ok(reply));
                }
                Err(error) => {
                    let _ = tx.send(LlmResponse::Err(error));
                }
            }
        })
        .expect("failed to spawn grace-chat-request thread");
    rx
}
