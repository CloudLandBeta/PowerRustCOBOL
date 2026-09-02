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
    /// Execute the next statement, then pause again. Crosses a PERFORM of a
    /// paragraph or section rather than entering it.
    StepOver,
    /// Step into the next statement, entering an out-of-line PERFORM or a CALL.
    StepIn,
    /// Request the interpreter to pause at the next statement (async).
    Pause,
    /// Run until the current PERFORM or CALL returns, then pause in the caller.
    StepOut,
    /// Run until a given 1-based source line, then pause. Gives up — and says
    /// so — if the frame it was issued from returns first.
    RunToCursor { line: u32 },
    /// End the program. Distinct from dropping the channel, which is the IDE
    /// going away rather than the developer asking to stop.
    Terminate,
    /// Ask a question while stopped.
    ///
    /// Answered with [`DebugEvent::Answer`] carrying the same `id`, and — unlike
    /// every other command — it does **not** resume: the program stays exactly
    /// where it is, so the developer can open a group, then a table, then an
    /// 88-level without the program moving underneath them.
    Query { id: u64, query: DebugQuery },
}

/// A question the IDE asks while the program is stopped.
///
/// Values are fetched on demand rather than pushed. The old `Paused` event
/// serialised **every data item in the program** at every stop — a large
/// message per single-step, nearly all of it rows nobody opened.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DebugQuery {
    /// The COBOL scopes visible in a frame: WORKING-STORAGE, LINKAGE, …
    Scopes { frame: usize },
    /// The rows under a handle — a scope, a group's children, a table's
    /// occurrences, an item's 88-levels, or its REDEFINES views.
    Variables { reference: i64 },
    /// Change a data item while stopped. Validated against the item's own
    /// PICTURE before anything is written; a rejected edit leaves the program
    /// exactly as it was.
    SetVariable { reference: i64, name: String, value: String },
    /// Evaluate a COBOL expression in a frame. Read-only.
    Evaluate { frame: usize, expression: String },
}

/// One collapsible scope in the data inspector.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScopeInfo {
    pub kind: crate::debug_session::ScopeKind,
    /// The COBOL section name — not translated: a COBOL developer looks for
    /// "WORKING-STORAGE SECTION", not a rendering of it.
    pub name: String,
    /// Handle to pass back as [`DebugQuery::Variables`].
    pub reference: i64,
    /// How many rows it holds, for the header count.
    pub count: u32,
    pub expensive: bool,
}

/// A value that is NOT an ordinary value, kept apart so the inspector can tell
/// seven different things apart instead of rendering them all as an empty cell.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SpecialValue {
    EmptyString,
    Spaces,
    LowValues,
    HighValues,
    /// A LINKAGE item with no argument, or an item never given a value.
    Unset,
    /// Reading it failed; the message is in the row's `value`.
    EvaluationError,
}

/// One row of the data inspector.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VarInfo {
    /// The item's own name, unqualified — the tree supplies the path.
    pub name: String,
    pub value: String,
    /// COBOL category: `group`, `alphanumeric`, `numeric`, `float`,
    /// `condition`, `index`.
    pub category: String,
    /// Non-zero when the row expands. Pass back as [`DebugQuery::Variables`].
    pub reference: i64,
    /// Raw PICTURE, empty for a group or an index.
    pub pic: String,
    /// Storage width in bytes, when known.
    pub length: Option<u32>,
    /// This item's own OCCURS count, if it is a table.
    pub occurs: Option<u32>,
    /// The controlling item of an OCCURS DEPENDING ON.
    pub depending_on: Option<String>,
    /// The item whose bytes this one re-reads.
    pub redefines: Option<String>,
    pub special: Option<SpecialValue>,
    /// May it be edited while stopped? False for a group, a table, an 88-level
    /// and anything the runtime cannot write back.
    pub editable: bool,
    /// The expression that re-reads this item — its qualified name. What "add
    /// to watches" would copy.
    pub evaluate_name: String,
    /// The item changed at this stop.
    pub changed: bool,
}

/// The reply to a [`DebugQuery`].
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DebugAnswer {
    Scopes(Vec<ScopeInfo>),
    Variables(Vec<VarInfo>),
    /// The value as it reads back AFTER the write — a `PIC 9(3)` given `7`
    /// answers `007`, so the developer sees what the program will see.
    Set { value: String },
    Evaluated { result: String, pic: String },
    /// The query failed. Carries the reason, which the UI shows verbatim.
    Error(String),
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
    /// The interpreter stopped at a safepoint.
    ///
    /// Carries **why** and the logical COBOL stack, and nothing else: scopes,
    /// variables and evaluations are fetched on demand. The older `Paused`
    /// below pushes every data item in the program with every stop, which is a
    /// large message per single-step for rows the developer never opens.
    Stopped {
        /// Source line about to execute (1-based).
        line: u32,
        /// Source column (1-based).
        col: u32,
        /// The paragraph the innermost frame is in.
        paragraph: String,
        reason: crate::debug_session::StopReason,
        /// The logical stack, innermost frame first.
        frames: Vec<crate::debug_session::DebugFrame>,
    },
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
    /// Output from the debugger itself: a logpoint, a file operation, an event.
    /// Goes to the investigation dock rather than the program's own DISPLAY.
    Output {
        text: String,
        channel: OutputChannel,
    },
    /// The reply to a [`DebugCmd::Query`], carrying the same `id`.
    Answer { id: u64, answer: DebugAnswer },
    /// The interpreter resumed after a `Continue` or `StepOver`.
    Resumed,
    /// The program finished (STOP RUN / GOBACK).
    Finished,
}

/// Which tab of the investigation dock a line of debugger output belongs in.
///
/// Without this everything piles into one console, and the question "what did
/// this program do to my files" becomes a grep through unrelated chatter.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum OutputChannel {
    /// Debugger messages and logpoint output.
    Console,
    /// Form events, program entry and exit.
    Events,
    /// OPEN / READ / WRITE / CLOSE and their FILE STATUS.
    FileIo,
    /// Warnings and refusals.
    Problems,
    /// Ordered record of what happened, including irreversible side effects.
    Timeline,
}

// ── Shared breakpoint set ─────────────────────────────────────────────────────

/// A thread-safe set of source line numbers that are active breakpoints.
///
/// Wrap in `Arc::clone()` to share between the IDE thread and the interpreter.
/// The environment variable a host sets to ask a COMPILED application to open
/// a debug session on its own stdio (`@DBG` lines, same protocol as
/// `rcrun run-form --debug`).
///
/// It lives here, with the rest of the protocol, because BOTH ends need the
/// name and they are in different crates: the IDE sets it when it launches a
/// built binary as a debuggee, and `cobolt-form-host` reads it inside that
/// binary. A second spelling in either place is a session that silently never
/// starts.
///
/// A compiled application is a normal desktop program and must not read stdin
/// uninvited, or a COBOL `ACCEPT` would find the debugger's reader thread
/// already holding it — so the link is off unless this says otherwise, and a
/// released binary behaves exactly as it did before it could be debugged.
pub const DEBUG_SESSION_ENV: &str = "COBOLT_DEBUG_SESSION";

/// Did the launching host ask this process for a debug session?
pub fn debug_session_requested() -> bool {
    std::env::var(DEBUG_SESSION_ENV)
        .map(|v| {
            let v = v.trim();
            v == "1" || v.eq_ignore_ascii_case("true") || v.eq_ignore_ascii_case("yes")
        })
        .unwrap_or(false)
}

pub type Breakpoints = Arc<Mutex<HashSet<u32>>>;

/// What a breakpoint does beyond "stop here".
///
/// Kept in a **separate** shared map rather than folded into [`Breakpoints`]:
/// that set is the fast path consulted at every statement, and a plain
/// breakpoint — the overwhelming majority — must not pay for a hash lookup into
/// a richer structure to learn it has nothing extra to say.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct BreakpointSpec {
    /// A COBOL condition. The breakpoint fires only when it is true; a
    /// condition that fails to parse or to evaluate is reported once and the
    /// breakpoint then behaves as a plain one, because silently never stopping
    /// is indistinguishable from a broken debugger.
    pub condition: Option<String>,
    /// `5`, `>= 5`, `% 3` — see the DAP hit-condition algebra.
    pub hit_condition: Option<String>,
    /// A logpoint: emit this, with `{expr}` interpolated, and do NOT stop.
    pub log_message: Option<String>,
    /// Remove the breakpoint once it has fired.
    pub temporary: bool,
}

impl BreakpointSpec {
    /// Does this carry anything beyond "stop here"?
    pub fn is_plain(&self) -> bool {
        self.condition.is_none()
            && self.hit_condition.is_none()
            && self.log_message.is_none()
            && !self.temporary
    }
}

/// Line → extra behaviour, shared with the IDE so an edit takes effect live.
pub type BreakpointSpecs = Arc<Mutex<std::collections::HashMap<u32, BreakpointSpec>>>;

/// An empty, shared spec map.
pub fn new_breakpoint_specs() -> BreakpointSpecs {
    Arc::new(Mutex::new(std::collections::HashMap::new()))
}

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

#[cfg(test)]
mod debug_session_env_tests {
    use super::*;

    /// The switch is OFF unless a host asks — the case that matters, because a
    /// released application must leave stdin to `ACCEPT`.
    ///
    /// One test, not several: they share the process environment, and cargo
    /// runs tests in a file on separate threads.
    #[test]
    fn the_debug_session_switch_is_off_unless_asked() {
        unsafe { std::env::remove_var(DEBUG_SESSION_ENV) };
        assert!(
            !debug_session_requested(),
            "absent must mean no session — a compiled app that grabs stdin \
             uninvited breaks ACCEPT"
        );
        for on in ["1", "true", "TRUE", "yes", " 1 "] {
            unsafe { std::env::set_var(DEBUG_SESSION_ENV, on) };
            assert!(debug_session_requested(), "{on:?} asks for a session");
        }
        for off in ["0", "", "no", "off", "please"] {
            unsafe { std::env::set_var(DEBUG_SESSION_ENV, off) };
            assert!(!debug_session_requested(), "{off:?} does not");
        }
        unsafe { std::env::remove_var(DEBUG_SESSION_ENV) };
    }
}
