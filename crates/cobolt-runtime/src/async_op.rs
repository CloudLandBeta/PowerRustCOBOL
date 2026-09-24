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
    /// An `AgentObject::Ask` came back. Carried RAW — status and body exactly
    /// as they arrived — rather than already parsed, so the reply is read and
    /// narrated on the interpreter thread by the same code the blocking call
    /// used. `Verbose` prints the status and the untouched body, and a
    /// provider that answers in an unexpected shape produces one diagnosis,
    /// not two that can drift apart.
    AgentReply { status: u16, body: String },
    /// Spec 068 — a KnowledgeBase operation moved on. Not final: the operation
    /// is still pending. Throttled by the worker.
    KbProgress {
        document: String,
        current: usize,
        total: usize,
    },
    /// Spec 068 — an update (add, update, delete, refresh, reindex, fetching
    /// the model) finished.
    KbIndexed {
        added: usize,
        updated: usize,
        removed: usize,
        /// `"<document>: <reason>"`, one per document not indexed.
        skipped: Vec<String>,
        /// Why documents were stored without vectors, when they were.
        note: String,
    },
    /// Spec 068 — a search finished.
    KbSearchDone {
        hits: Vec<KbHit>,
        mode: String,
        reason: String,
    },
    /// Spec 068 — another application held the collection's write lock past
    /// the wait.
    KbBusy { message: String },
    /// Spec 068 — the operation failed.
    KbError { message: String },
    /// Spec 068 — a model's search of a KnowledgeBase collection, run on a
    /// worker because it needed the embedding server. Delivered to the
    /// AGENT's tool loop, which it moves on; not the end of the agent's `Ask`.
    KbToolResult { call_id: String, text: String },
}

/// One passage a KnowledgeBase search found (spec 068 R33).
#[derive(Debug, Clone, PartialEq)]
pub struct KbHit {
    pub document: String,
    pub heading: String,
    pub passage: String,
    pub score: f32,
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
