// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Async I/O operation primitive — spec 032.
//!
//! A generic facility that lets a control (today: `RestClient`) run a blocking
//! I/O call on a background `std::thread` while the interpreter's event loop
//! stays live. The worker reports its [`AsyncOpResult`] back over an
//! interpreter-owned `mpsc` channel; `COBOL-WAIT-EVENT` drains it, writes the
//! control's outputs, and dispatches a lifecycle event (`onComplete` /
//! `onError` / `onCancelled` / `onTimeout`).
//!
//! Race safety is by **generation**: each control has a monotonically-increasing
//! generation counter. A worker captures the generation live when it starts; a
//! delivered result is applied only if its generation still matches the
//! control's current generation. `Cancel()`, a timeout, or a superseding call
//! bumps the generation, so any late result from an abandoned worker is
//! discarded silently — the orphaned thread simply finishes on its own (none of
//! `ureq`/`rusqlite`/`redb` expose cooperative mid-call cancellation).

use std::time::Instant;

/// Outcome of a background I/O operation, delivered to the interpreter thread.
#[derive(Debug, Clone)]
pub enum AsyncOutcome {
    /// HTTP call returned a status (includes 4xx/5xx — `ureq` keeps the real code).
    HttpSuccess { body: String, status: u16 },
    /// Transport/network failure (no HTTP status; mirrors the sync convention
    /// where a network error yields status 0 and the error text as the body).
    HttpError { message: String },
}

/// A completed background operation, matched to a control by id + generation.
#[derive(Debug, Clone)]
pub struct AsyncOpResult {
    /// The control that started the operation.
    pub ctrl_id: String,
    /// The control generation live when the worker started (see module docs).
    pub generation: u64,
    /// What happened.
    pub outcome: AsyncOutcome,
}

/// Interpreter-side bookkeeping for one in-flight operation on a control.
///
/// At most one pending operation exists per control at a time — a second start
/// while `Busy` is rejected.
pub struct PendingOp {
    /// Generation this operation was started at.
    pub generation: u64,
    /// When the operation was spawned (for the timeout sweep).
    pub started_at: Instant,
    /// Effective timeout in milliseconds; `0` means "no interpreter-side timeout".
    pub timeout_ms: u64,
}
