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

// ── Commands (IDE → interpreter) ──────────────────────────────────────────────

/// Command sent from the IDE to the running interpreter.
#[derive(Debug, Clone)]
pub enum DebugCmd {
    /// Resume execution until the next breakpoint.
    Continue,
    /// Execute the next statement, then pause again.
    StepOver,
    /// Request the interpreter to pause at the next statement (async).
    Pause,
}

// ── Events (interpreter → IDE) ────────────────────────────────────────────────

/// A snapshot of one data-item's current value.
#[derive(Debug, Clone)]
pub struct VarSnapshot {
    pub name:  String,
    pub value: String,
}

/// Event sent from the interpreter to the IDE.
#[derive(Debug, Clone)]
pub enum DebugEvent {
    /// The interpreter has paused before executing a statement.
    Paused {
        /// Source line that is about to execute (1-based).
        line:      u32,
        /// Source column (1-based).
        col:       u32,
        /// Name of the paragraph currently executing.
        paragraph: String,
        /// Snapshot of all data items at the moment of pause.
        vars:      Vec<VarSnapshot>,
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
