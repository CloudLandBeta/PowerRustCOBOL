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

enum GraceMsg {
    Progress(String),
    /// A gated git op is waiting for the operator (spec 030 R12). The worker
    /// blocks on `reply` until the UI answers (or drops it → deny).
    Confirm(GitConfirmRequest, Sender<bool>),
    Done(Result<(WorkflowRecord, PathBuf), String>),
}

/// A running (or finished) Grace workflow, owned by the UI.
pub struct GraceSession {
    pub request: String,
    pub log: Vec<String>,
    rx: Option<Receiver<GraceMsg>>,
    finished: Option<Result<(WorkflowRecord, PathBuf), String>>,
    /// A gated git op awaiting the operator's Approve/Deny (spec 030 R12).
    pending_confirm: Option<(GitConfirmRequest, Sender<bool>)>,
}

impl GraceSession {
    /// Spawn a workflow for `request` on a worker thread.
    pub fn spawn(project_dir: &Path, llm: &LlmConfig, request: &str) -> Self {
        let (tx, rx) = mpsc::channel();
        let dir = project_dir.to_path_buf();
        let llm = llm.clone();
        let req = request.to_string();
        std::thread::Builder::new()
            .name("grace-workflow".into())
            .spawn(move || {
                let tx2 = tx.clone();
                let mut on_progress = move |line: String| {
                    let _ = tx2.send(GraceMsg::Progress(line));
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
                let result = crate::grace_host::run_grace_workflow(
                    &dir,
                    &llm,
                    &req,
                    &mut on_progress,
                    &mut confirm,
                );
                let _ = tx.send(GraceMsg::Done(result));
            })
            .expect("failed to spawn grace-workflow thread");
        Self {
            request: request.to_string(),
            log: vec!["Grace received the request.".into()],
            rx: Some(rx),
            finished: None,
            pending_confirm: None,
        }
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
                Ok(GraceMsg::Confirm(req, reply)) => {
                    self.log.push(format!("⏸ awaiting approval: {}", req.command));
                    self.pending_confirm = Some((req, reply));
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

    /// The completed result, once available.
    pub fn finished(&self) -> Option<&Result<(WorkflowRecord, PathBuf), String>> {
        self.finished.as_ref()
    }
}
