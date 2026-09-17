// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! The `@DBG` stdio debug link — one implementation, two debuggees.
//!
//! A debug session between the IDE and a running form is a line protocol over
//! the child's own pipes: commands arrive on **stdin** as `@DBG <json
//! RemoteDebugCmd>`, events leave on **stdout** as `@DBG <json DebugEvent>`.
//! Plain lines on either stream stay ordinary DISPLAY output, so one pair of
//! pipes carries both without a second channel.
//!
//! It used to live inside `rcrun run-form`, which made `rcrun` the only program
//! the IDE could debug. That was the whole reason a program containing
//! `EXEC RUST` could not be debugged at all: a block is native code compiled
//! into a BUILT binary, the plain interpreter's [`ExecRustRegistry`] is empty,
//! and so the one process able to execute the block was the one process that
//! could not speak to the debugger. Moving the link here — the crate both
//! `rcrun run-form` and every compiled application already share (spec 042) —
//! is what lets the built binary be the debuggee.
//!
//! [`ExecRustRegistry`]: cobolt_runtime::exec_rust::ExecRustRegistry

use cobolt_runtime::{
    new_breakpoints, new_user_scope, Breakpoints, DebugCmd, DebugEvent, DebugUserScope,
    RemoteDebugCmd,
};
use std::sync::mpsc::{self, Receiver, Sender};

/// Everything an interpreter needs to join a debug session:
/// `(commands in, events out, shared breakpoints, shared "only my code" scope)`.
///
/// Hand the first three to [`attach_debug_channels`] and the fourth to
/// [`set_debug_user_scope`], in that order.
///
/// [`attach_debug_channels`]: cobolt_runtime::Interpreter::attach_debug_channels
/// [`set_debug_user_scope`]: cobolt_runtime::Interpreter::set_debug_user_scope
pub type DebugWiring = (
    Receiver<DebugCmd>,
    Sender<DebugEvent>,
    Breakpoints,
    DebugUserScope,
);

// The switch itself is part of the PROTOCOL, so it lives with the protocol —
// `cobolt_runtime::debugger` — where the IDE (which sets it) and this crate
// (which reads it) can both see one spelling of the name.
pub use cobolt_runtime::{debug_session_requested, DEBUG_SESSION_ENV};

/// Is the debuggee stopped right now — at a breakpoint, a step, or a pause?
///
/// Process-wide rather than threaded through [`DebugWiring`], because a debug
/// session IS process-wide: one `@DBG` link on this process's own stdio, one
/// interpreter, one answer. The form host reads it every frame to decide
/// whether the window takes input; no debug session means it is never set, so
/// a normally running form is untouched.
static PAUSED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// While the program is stopped, the form must not accept clicks.
///
/// A stopped program cannot run a handler — the interpreter is blocked inside
/// `debug_check` waiting for the next command — but the GUI thread keeps
/// painting and keeps collecting input, so every click the developer made while
/// reading the code was queued up and delivered in a burst the moment they
/// pressed Continue (operator, 2026-09-17). The window stays visible and legible
/// and simply takes no input, the same way it behaves under a modal child.
pub fn is_paused() -> bool {
    PAUSED.load(std::sync::atomic::Ordering::Relaxed)
}

fn set_paused(paused: bool) {
    PAUSED.store(paused, std::sync::atomic::Ordering::Relaxed);
}

/// Does this command set the program going again?
///
/// Everything does except the two that deliberately do not: `Query` answers a
/// question while the program stays exactly where it is — that is the whole
/// point of it, so the developer can open a group, then a table, then an
/// 88-level without it moving underneath them — and `Pause` is a request to
/// STOP, so treating it as a resume would unblock the window at the moment it
/// should be locking.
fn resumes(cmd: &DebugCmd) -> bool {
    !matches!(cmd, DebugCmd::Query { .. } | DebugCmd::Pause)
}

/// Open the `@DBG` link on this process's stdin/stdout.
///
/// Spawns two threads: a reader that parses `@DBG` command lines off stdin and
/// dispatches them, and a pump that serialises every [`DebugEvent`] the
/// interpreter emits onto stdout. Both end when their channel closes.
///
/// Call this only when a session was actually asked for — it takes stdin.
pub fn stdio_debug_wiring() -> DebugWiring {
    let (cmd_tx, cmd_rx) = mpsc::channel::<DebugCmd>();
    let (ev_tx, ev_rx) = mpsc::channel::<DebugEvent>();
    let breakpoints = new_breakpoints();
    let user_scope = new_user_scope();

    // stdin reader: parse and dispatch remote debug commands.
    {
        let bps = std::sync::Arc::clone(&breakpoints);
        let scope = std::sync::Arc::clone(&user_scope);
        std::thread::spawn(move || {
            use std::io::BufRead;
            let stdin = std::io::stdin();
            for line in stdin.lock().lines().map_while(Result::ok) {
                let Some(json) = line.strip_prefix("@DBG ") else {
                    continue;
                };
                match serde_json::from_str::<RemoteDebugCmd>(json) {
                    Ok(RemoteDebugCmd::Cmd(c)) => {
                        // Every command resumes the program except a Query,
                        // which deliberately answers without moving it, and a
                        // Pause, which asks it to STOP. Clearing the flag as the
                        // command goes in — rather than waiting to be told the
                        // program moved — means the window is live again on the
                        // same frame the developer pressed Continue.
                        if resumes(&c) {
                            set_paused(false);
                        }
                        if cmd_tx.send(c).is_err() {
                            break;
                        }
                    }
                    Ok(RemoteDebugCmd::SetBreakpoints(lines)) => {
                        if let Ok(mut guard) = bps.lock() {
                            *guard = lines.into_iter().collect();
                        }
                    }
                    // "Only my code": the IDE hands over the generated `.cbl`
                    // lines that hold the developer's own handler and procedure
                    // bodies, and stepping crosses everything else without
                    // stopping. Shared, so the toggle takes effect mid-session
                    // without restarting the form.
                    Ok(RemoteDebugCmd::SetUserScope {
                        user_only,
                        user_lines,
                    }) => {
                        if let Ok(mut guard) = scope.lock() {
                            guard.user_only = user_only;
                            guard.user_lines = user_lines.into_iter().collect();
                        }
                    }
                    Err(e) => eprintln!("debug: bad @DBG command: {e}"),
                }
            }
        });
    }

    // event pump: interpreter → stdout (whole lines; println! locks stdout, so
    // interleaving with DISPLAY output stays line-atomic).
    std::thread::spawn(move || {
        use std::io::Write;
        for ev in ev_rx.iter() {
            // The program has come to rest: from here until the next resuming
            // command the form takes no input.
            if matches!(ev, DebugEvent::Stopped { .. }) {
                set_paused(true);
            }
            match serde_json::to_string(&ev) {
                Ok(json) => {
                    println!("@DBG {json}");
                    let _ = std::io::stdout().flush();
                }
                Err(e) => eprintln!("debug: cannot serialize DebugEvent: {e}"),
            }
        }
    });

    (cmd_rx, ev_tx, breakpoints, user_scope)
}


#[cfg(test)]
mod paused_tests {
    use super::*;
    use cobolt_runtime::DebugQuery;

    /// Which commands let the form take input again.
    ///
    /// A stopped program cannot run a handler, so while it is stopped the
    /// window must refuse clicks rather than bank them for delivery on Continue
    /// (operator, 2026-09-17). Getting this list wrong in either direction is
    /// invisible until someone clicks: too eager and the window unlocks while
    /// the program is still stopped; too shy and it stays dead after Continue.
    #[test]
    fn every_command_resumes_except_query_and_pause() {
        for cmd in [
            DebugCmd::Continue,
            DebugCmd::StepOver,
            DebugCmd::StepIn,
            DebugCmd::StepOut,
            DebugCmd::RunToCursor { line: 42 },
            DebugCmd::Terminate,
        ] {
            assert!(resumes(&cmd), "{cmd:?} should resume the program");
        }

        // A Query answers without moving the program, so the window stays shut.
        assert!(
            !resumes(&DebugCmd::Query {
                id: 1,
                query: DebugQuery::Variables { reference: 7 },
            }),
            "a Query must not unblock the form: the program has not moved"
        );
        // And a Pause is the opposite of a resume.
        assert!(
            !resumes(&DebugCmd::Pause),
            "Pause asks the program to STOP; it cannot also unblock the form"
        );
    }

    /// Not in a debug session at all: the flag is never set, so an ordinary
    /// running form is untouched by any of this.
    #[test]
    fn a_form_with_no_debug_session_is_never_blocked() {
        assert!(
            !is_paused(),
            "the paused flag must start clear — a form run normally takes input"
        );
    }
}
