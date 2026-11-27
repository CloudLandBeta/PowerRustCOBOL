// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! The Backend's verbs for the call a module is answering (ABI A60, spec 114 M5, T36).
//!
//! `COBOL::"WS-FAIL"`, `"WS-PROGRESS"`, `"WS-CANCELLED"`, `"WS-PAUSE"`, `"WS-EMIT"`, `"WS-CLAIM"`, `"WS-FILE-PATH"` and
//! `"WS-FILE-PUBLISH"` act on the call the program is serving: the failure it reports, the progress it shows, the items it
//! streams, the caller's claims and the files the caller may reach. The runner does not know the call; the **host** that
//! starts it does (the server, which owns the wire), and hands it over as a [`CallHost`]. This file is the runner's half: it
//! reads what the module passes, applies the interpreter's own rules (`cobolt_wire::verbs`, which the interpreter's built-ins
//! apply too) and answers.
//!
//! A call made with no host (a test calling the module directly) behaves as the interpreter does with no hooks:
//! `WS-FAIL` and `WS-EMIT` are run-time errors, `WS-PROGRESS` does nothing, `WS-CANCELLED` says `N`, `WS-CLAIM` says nothing,
//! and the file verbs say `NOBACKEND`.

use std::sync::Arc;
use std::time::{Duration, Instant};

use cobolt_wire::verbs;

/// What the host of a call gives the verbs (the interpreter's `BackendHooks`, as a trait).
pub trait CallHost: Send {
    /// `WS-PROGRESS`: a report, its percentage held to 0 to 100 and its message cut already.
    fn progress(&mut self, percent: u8, message: &str);
    /// `WS-EMIT`: the JSON of the next stream item. `Some(true)` it was sent, `Some(false)` nobody wants more, `None` this
    /// call is not a stream.
    fn emit(&mut self, item: &str) -> Option<bool>;
    /// `WS-CLAIM`: the caller's claim `name`, empty when there is none.
    fn claim(&self, name: &str) -> String;
    /// The caller has asked to cancel.
    fn cancel_requested(&self) -> bool;
    /// `WS-FILE-PATH`: where the file named `id` is on disk.
    fn file_path(&self, id: &str) -> FileReply;
    /// `WS-FILE-PUBLISH`: the id the file at `path` can be fetched by.
    fn publish(&self, path: &str) -> FileReply;
}

/// What a file verb answers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FileReply {
    /// The path (`WS-FILE-PATH`) or the id (`WS-FILE-PUBLISH`).
    Text(String),
    /// The caller may not reach that file, or the path cannot be published.
    NoFile,
    /// The call carries no file service.
    NoBackend,
}

/// An application failure the program reported with `WS-FAIL` (the interpreter's `AppFailure`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Failure {
    pub code: u32,
    pub message: String,
    pub detail: Option<String>,
}

/// Asked at every slice of a pause: `true` stops the call.
pub(crate) type Stop = Arc<dyn Fn() -> bool + Send + Sync>;

/// The verbs' state for one call.
#[derive(Default)]
pub(crate) struct Verbs {
    host: Option<Box<dyn CallHost>>,
    stop: Option<Stop>,
    /// What `WS-FAIL` reported.
    pub(crate) failure: Option<Failure>,
    /// A `WS-PROGRESS` or `WS-EMIT` ended the call because its caller asked to cancel.
    pub(crate) cancelled: bool,
}

/// What a verb's import answers when it ends the call in the interpreter's way: a run-time error, as the words of it.
pub(crate) type Refused = String;

impl Verbs {
    pub(crate) fn new(host: Option<Box<dyn CallHost>>, stop: Option<Stop>) -> Verbs {
        Verbs { host, stop, failure: None, cancelled: false }
    }

    /// `WS-FAIL`: record the failure. The module ends its run.
    pub(crate) fn fail(&mut self, code: f64, message: &str, detail: &str) -> Result<(), Refused> {
        if self.host.is_none() {
            return Err(verbs::not_serving("WS-FAIL"));
        }
        let code = verbs::fail_code(code)?;
        self.failure = Some(Failure { code, message: verbs::fail_message(message), detail: verbs::fail_detail(detail) });
        Ok(())
    }

    /// `WS-PROGRESS`: 1 when the call must end because its caller asked to cancel.
    pub(crate) fn progress(&mut self, percent: f64, message: &str) -> i32 {
        let Some(host) = self.host.as_mut() else { return 0 };
        host.progress(verbs::progress_percent(percent), &verbs::progress_message(message));
        if host.cancel_requested() {
            self.cancelled = true;
            1
        } else {
            0
        }
    }

    /// `WS-CANCELLED`: whether the caller asked to cancel.
    pub(crate) fn cancelled_asked(&self) -> bool {
        self.host.as_ref().is_some_and(|h| h.cancel_requested())
    }

    /// `WS-PAUSE`: wait in slices, ending early when the caller asks to cancel. `Err(())` when the server stops the call: the
    /// import traps, as the epoch deadline does.
    pub(crate) fn pause(&self, ms: f64) -> Result<(), ()> {
        let end = Instant::now() + Duration::from_millis(verbs::pause_ms(ms));
        loop {
            if self.stop.as_ref().is_some_and(|s| s()) {
                return Err(());
            }
            if self.cancelled_asked() {
                return Ok(());
            }
            let now = Instant::now();
            if now >= end {
                return Ok(());
            }
            std::thread::sleep((end - now).min(Duration::from_millis(verbs::PAUSE_SLICE_MS)));
        }
    }

    /// `WS-EMIT`: 1 when the call must end. `item` is the JSON of the group, or the mapping's refusal.
    pub(crate) fn emit(&mut self, item: Result<&str, i32>) -> Result<i32, Refused> {
        let Some(host) = self.host.as_mut() else {
            return Err(verbs::not_serving("WS-EMIT"));
        };
        let json = item.map_err(|code| verbs::emit_unwritable(&format!("the group cannot be written as JSON (mapping error {code})")))?;
        let sent = host.emit(json);
        let asked = host.cancel_requested();
        match sent {
            None => Err(verbs::EMIT_NOT_A_STREAM.to_owned()),
            Some(true) if !asked => Ok(0),
            Some(_) => {
                self.cancelled = true;
                Ok(1)
            }
        }
    }

    /// `WS-CLAIM`.
    pub(crate) fn claim(&self, name: &str) -> String {
        self.host.as_ref().map(|h| h.claim(name.trim())).unwrap_or_default()
    }

    /// `WS-FILE-PATH`: the path and the status word.
    pub(crate) fn file_path(&self, id: &str) -> (String, &'static str) {
        self.file_reply(|h| h.file_path(id.trim()))
    }

    /// `WS-FILE-PUBLISH`: the id and the status word.
    pub(crate) fn publish(&self, path: &str) -> (String, &'static str) {
        self.file_reply(|h| h.publish(path.trim()))
    }

    fn file_reply(&self, ask: impl Fn(&dyn CallHost) -> FileReply) -> (String, &'static str) {
        match self.host.as_deref().map(ask) {
            None | Some(FileReply::NoBackend) => (String::new(), verbs::FILE_NOBACKEND),
            Some(FileReply::Text(t)) => (t, verbs::FILE_OK),
            Some(FileReply::NoFile) => (String::new(), verbs::FILE_NOFILE),
        }
    }
}
