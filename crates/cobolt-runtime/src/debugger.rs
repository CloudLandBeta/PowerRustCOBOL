// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Debugger channel types — Phase 7.
//!
//! Two channel pairs connect the IDE debugger UI to the interpreter thread:
//!
//! ```text
//! IDE thread (egui)                    Interpreter thread
//! ─────────────────────────────        ──────────────────────────────────────
//! DebuggerState.send_cmd()  ─────────► blocks in exec_stmts debug hook
//! DebuggerState.recv_event() ◄────────  sends DebugEvent when paused / done
//! ```
//!
//! Breakpoints are shared via `Arc<Mutex<HashSet<u32>>>` so the IDE can
//! toggle them while the program is running without an extra round-trip.

use std::collections::HashSet;
use std::sync::{Arc, Mutex};

use serde::{Deserialize, Serialize};

// ── Commands (IDE → interpreter) ──────────────────────────────────────────────

/// Command sent from the IDE to the running interpreter.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DebugCmd {
    /// Resume execution until the next breakpoint.
    Continue,
    /// Execute the next statement, then pause again.
    StepOver,
    /// Step into the next statement. Currently statement-level, matching StepOver.
    StepIn,
    /// Request the interpreter to pause at the next statement (async).
    Pause,
}

/// A debug command sent to a **remote** debuggee (an `rcrun run-form --debug`
/// process today; an Android/iOS runtime over adb/ssh tomorrow). Serialized as
/// one JSON line prefixed `@DBG ` on the debuggee's stdin; debug events travel
/// back the same way on stdout (plain lines remain DISPLAY output).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RemoteDebugCmd {
    /// Forward a pause/step/continue to the interpreter.
    Cmd(DebugCmd),
    /// Replace the debuggee's whole breakpoint set (idempotent).
    SetBreakpoints(Vec<u32>),
    /// Set the debug **scope**: when `user_only` is true, the interpreter pauses
    /// only on lines the developer authored (`user_lines`), stepping transparently
    /// through IDE-generated scaffolding. `user_lines` is the flat set of 1-based
    /// generated-`.cbl` line numbers that hold handler / user-procedure code.
    SetUserScope {
        user_only: bool,
        user_lines: Vec<u32>,
    },
}

// ── Events (interpreter → IDE) ────────────────────────────────────────────────

/// A snapshot of one data-item's current value.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VarSnapshot {
    pub name: String,
    pub scope: String,
    pub pic: String,
    pub origin: String,
    pub value: String,
}

/// Event sent from the interpreter to the IDE.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DebugEvent {
    /// The interpreter has paused before executing a statement.
    Paused {
        /// Source line that is about to execute (1-based).
        line: u32,
        /// Source column (1-based).
        col: u32,
        /// Name of the paragraph currently executing.
        paragraph: String,
        /// Snapshot of all data items at the moment of pause.
        vars: Vec<VarSnapshot>,
    },
    /// The interpreter resumed after a `Continue` or `StepOver`.
    Resumed,
    /// The program finished (STOP RUN / GOBACK).
    Finished,
}

// ── Shared breakpoint set ─────────────────────────────────────────────────────

/// A thread-safe set of source line numbers that are active breakpoints.
///
/// Wrap in `Arc::clone()` to share between the IDE thread and the interpreter.
pub type Breakpoints = Arc<Mutex<HashSet<u32>>>;

/// Create an empty, shared breakpoint set.
pub fn new_breakpoints() -> Breakpoints {
    Arc::new(Mutex::new(HashSet::new()))
}

// ── Shared debug scope (user-only stepping) ───────────────────────────────────

/// Which lines the debugger is allowed to stop on. When `user_only` is true and
/// `user_lines` is non-empty, the interpreter pauses only on those lines, running
/// transparently through IDE-generated scaffolding. Shared `Arc<Mutex<…>>` so the
/// IDE's "hide generated code" toggle takes effect live, mid-session.
#[derive(Debug, Default, Clone)]
pub struct UserScope {
    pub user_only: bool,
    pub user_lines: HashSet<u32>,
}

/// A thread-safe, shareable [`UserScope`].
pub type DebugUserScope = Arc<Mutex<UserScope>>;

/// Create a shared, empty scope (defaults to `user_only = false` = debug everything).
pub fn new_user_scope() -> DebugUserScope {
    Arc::new(Mutex::new(UserScope::default()))
}
