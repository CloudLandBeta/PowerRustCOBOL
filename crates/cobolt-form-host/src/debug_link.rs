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
