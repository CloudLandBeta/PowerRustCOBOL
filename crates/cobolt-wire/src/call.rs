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

/// What carries a remote call.
pub trait Transport: Send {
    /// Perform the call and return when it has finished or failed. Blocks; the
    /// interpreter runs on its own thread, so a form keeps painting while it
    /// waits.
    fn call(&mut self, request: &CallRequest) -> Result<CallReply, CallError>;
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
