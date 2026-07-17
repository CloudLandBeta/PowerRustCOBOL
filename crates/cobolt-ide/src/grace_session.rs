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
use std::sync::mpsc::{self, Receiver};

use cobolt_agents::grace::WorkflowRecord;

use crate::llm::LlmConfig;

enum GraceMsg {
    Progress(String),
    Done(Result<(WorkflowRecord, PathBuf), String>),
}

/// A running (or finished) Grace workflow, owned by the UI.
pub struct GraceSession {
    pub request: String,
    pub log: Vec<String>,
    rx: Option<Receiver<GraceMsg>>,
    finished: Option<Result<(WorkflowRecord, PathBuf), String>>,
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
                let mut on_progress =
                    move |line: String| {
                        let _ = tx2.send(GraceMsg::Progress(line));
                    };
                let result = crate::grace_host::run_grace_workflow(&dir, &llm, &req, &mut on_progress);
                let _ = tx.send(GraceMsg::Done(result));
            })
            .expect("failed to spawn grace-workflow thread");
        Self {
            request: request.to_string(),
            log: vec!["Grace received the request.".into()],
            rx: Some(rx),
            finished: None,
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

    /// The completed result, once available.
    pub fn finished(&self) -> Option<&Result<(WorkflowRecord, PathBuf), String>> {
        self.finished.as_ref()
    }
}
