// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! The seam between a `CALL … WITH WEBSOCKET LINKAGE` and whatever carries it
//! (WASM-005 R8).
//!
//! The interpreter turns the statement's groups into JSON with the mapping,
//! hands a [`CallRequest`] to a [`Transport`], and writes the [`CallReply`] back
//! into the groups. Three things implement the trait: the real WebSocket client
//! (`cobolt-server`, feature `client`), a loopback double in tests, and — in the
//! browser, where there is no interpreter — the generated module's own
//! equivalent. The connection, the token, the framing, the correlation, the
//! keep-alive and the reconnection are the implementation's business and never
//! the developer's.

use crate::error::CallError;

/// How one argument is passed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArgMode {
    /// `BY VALUE`: sent; the caller's copy is never changed.
    Value,
    /// `BY REFERENCE`: sent; the callee's final values are written back.
    Reference,
}

/// One argument of a call: the JSON of a group, and how it travels.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArgPayload {
    pub mode: ArgMode,
    /// The group's JSON text (an elementary item's bare scalar).
    pub json: String,
}

/// A remote call.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct CallRequest {
    /// The exported program's name, upper-cased.
    pub program: String,
    pub args: Vec<ArgPayload>,
    /// The call named a `RETURNING` group.
    pub wants_returning: bool,
    /// An idempotency key the runtime generated for a non-idempotent call that
    /// may be repeated (WASM-014 R10); `None` to let the transport decide.
    pub idempotency_key: Option<String>,
}

/// The answer to a call.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct CallReply {
    /// One entry per argument, in order: the callee's final JSON for a
    /// `BY REFERENCE` argument, `None` for a `BY VALUE` one (or one the callee
    /// left alone).
    pub args: Vec<Option<String>>,
    /// The `RETURNING` group's JSON, when the call asked for one.
    pub returning: Option<String>,
    /// Deprecation and compatibility warnings the backend attached (WASM-014
    /// R6); the call still succeeded.
    pub warnings: Vec<String>,
}

/// What the caller wants to know while a call is pending, and the one thing it
/// can ask for: to cancel. A host that shows progress, warnings or a cancel
/// button implements it; the interpreter's own observer feeds
/// `Interpreter::set_remote_events` and `remote_cancel_handle`.
pub trait CallObserver {
    /// A long-running call reported progress (WASM-014 R13).
    fn progress(&mut self, _percent: u8, _message: &str) {}
    /// The backend attached a warning to a call or to the connection (a
    /// deprecated program, an older client; WASM-014 R2, R6).
    fn warning(&mut self, _message: &str) {}
    /// Asked while the call is pending; `true` asks the backend to cancel it.
    fn cancel_requested(&mut self) -> bool {
        false
    }
}

/// An observer that ignores everything.
pub struct NoObserver;

impl CallObserver for NoObserver {}

/// A file to put into one of the backend's upload slots (WASM-014 R15).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UploadRequest {
    /// The slot the backend declared; never a path.
    pub slot: String,
    /// The local file.
    pub path: std::path::PathBuf,
    /// The name to show for it; the backend names the stored file itself.
    pub name: String,
    pub mime: String,
}

/// The backend's name for an uploaded file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UploadResult {
    pub file: String,
    pub size: u64,
}

/// A file the backend offers, to fetch to a local path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DownloadRequest {
    pub file: String,
    pub dest: std::path::PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DownloadResult {
    pub size: u64,
}

/// The files a backend program may reach for the caller it is serving (WASM-014
/// R15): the ones that caller uploaded, and a way to offer one for download. The
/// backend gives each call one bound to that call's subject.
pub trait BackendFiles: Send + Sync {
    /// Where an uploaded file is on the backend's disk; `None` unless this caller uploaded it.
    fn resolve(&self, file_id: &str) -> Option<std::path::PathBuf>;
    /// Offer a file the program holds to this caller; returns the id to hand the client.
    fn publish(&self, path: &std::path::Path) -> Result<String, String>;
}

/// What carries a remote call.
pub trait Transport: Send {
    /// Perform the call and return when it has finished or failed. Blocks; the
    /// interpreter runs on its own thread, so a form keeps painting while it
    /// waits.
    fn call(&mut self, request: &CallRequest) -> Result<CallReply, CallError>;

    /// The same, telling `observer` what happens meanwhile and asking it whether to cancel.
    fn call_observed(&mut self, request: &CallRequest, observer: &mut dyn CallObserver) -> Result<CallReply, CallError> {
        let _ = observer;
        self.call(request)
    }

    /// Put a local file into an upload slot, in pieces, resuming after a lost connection.
    fn upload(&mut self, _request: &UploadRequest, _observer: &mut dyn CallObserver) -> Result<UploadResult, CallError> {
        Err(CallError::transport(crate::error::codes::NOT_CONFIGURED, "this transport does not move files"))
    }

    /// Fetch a file the backend offers.
    fn download(&mut self, _request: &DownloadRequest, _observer: &mut dyn CallObserver) -> Result<DownloadResult, CallError> {
        Err(CallError::transport(crate::error::codes::NOT_CONFIGURED, "this transport does not move files"))
    }
}

/// A transport that always fails: what a run has when no backend is configured.
pub struct NoBackend;

impl Transport for NoBackend {
    fn call(&mut self, _request: &CallRequest) -> Result<CallReply, CallError> {
        Err(CallError::transport(
            crate::error::codes::NOT_CONFIGURED,
            "no backend is configured for this run (set the backend address in the project settings, or COBOLT_BACKEND_URL)",
        )
        .may_have_run(false))
    }
}
