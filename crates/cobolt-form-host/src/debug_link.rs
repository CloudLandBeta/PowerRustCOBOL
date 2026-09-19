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

use cobolt_runtime::form_host::ROOT_HANDLE;
use cobolt_runtime::{
    new_breakpoints, new_user_scope, Breakpoints, DebugCmd, DebugEvent, DebugUserScope,
    RemoteDebugCmd,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex, OnceLock};

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

// ── Which debuggee? (spec 061) ───────────────────────────────────────────────
//
// Since spec 051 an application is SEVERAL forms, each running its own
// generated program in its own interpreter. Identity lives here, in the
// router, and deliberately not in the protocol: `DebugCmd` and `DebugEvent`
// are already per-interpreter, so what was missing was a second caller and
// someone to keep the names straight. `cobolt-runtime` is untouched by all of
// this — if a change here seems to need one there, the design has drifted.

/// One `@DBG` line, **outbound**. The router stamps the handle; the
/// interpreter that produced the event knows nothing about it.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DebugWire {
    /// A debuggee joined the session. `handle` is its supervisor handle
    /// (`W0` is the root form, `W1`… the forms it opens) and `form` its
    /// form-object name — which is what lets the IDE find the `.cfrm`, and
    /// through it the generated `.cbl` this debuggee's stops are reported
    /// against.
    Attached { handle: String, form: String },
    /// Its interpreter has finished: the window closed, or the program ended.
    Detached { handle: String },
    /// Anything the interpreter emitted.
    Event { handle: String, event: DebugEvent },
}

/// One `@DBG` line, **inbound**.
///
/// `target: None` means the root debuggee, and [`parse_inbound`] also still
/// accepts a bare [`RemoteDebugCmd`] — so a session driven by an IDE that
/// knows nothing of handles degrades to debugging the root form rather than
/// failing.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RemoteDebugMsg {
    #[serde(default)]
    pub target: Option<String>,
    pub cmd: RemoteDebugCmd,
}

/// Parse one inbound `@DBG` payload, newest form first.
///
/// A bare `RemoteDebugCmd` is externally tagged (`{"Cmd":{…}}`), so it can
/// never be mistaken for a [`RemoteDebugMsg`], which requires a lower-case
/// `cmd` field. Trying the envelope first is therefore unambiguous.
fn parse_inbound(json: &str) -> Option<RemoteDebugMsg> {
    if let Ok(msg) = serde_json::from_str::<RemoteDebugMsg>(json) {
        return Some(msg);
    }
    serde_json::from_str::<RemoteDebugCmd>(json)
        .ok()
        .map(|cmd| RemoteDebugMsg { target: None, cmd })
}

/// One debuggee's end of the session.
struct Debuggee {
    cmd_tx: Sender<DebugCmd>,
    breakpoints: Breakpoints,
    scope: DebugUserScope,
}

/// Fans one `@DBG` link out to every interpreter in the process.
///
/// Each debuggee gets its **own** command channel, breakpoint set and
/// "only my code" scope. Two consequences are the whole point:
///
/// - **A command cannot reach the wrong interpreter.** No `Receiver` is ever
///   shared, so this is true by construction rather than by care — the
///   alternative, one `Receiver` behind a mutex, would let whichever
///   interpreter happened to be blocked in `recv` first swallow a `Continue`
///   meant for another form, and nothing would report it.
/// - **A breakpoint belongs to a program.** Line 42 in two forms' generated
///   `.cbl` files is two breakpoints, because it is two sets.
///
/// [`PAUSED`] stays process-wide on purpose (operator ruling, 2026-09-19):
/// while any form is stopped, the whole application is stopped.
pub struct DebugRouter {
    debuggees: Mutex<HashMap<String, Debuggee>>,
    out: Sender<DebugWire>,
}

impl DebugRouter {
    /// A router that writes its wire lines into `out`. Tests drive this one
    /// directly; [`Self::stdio`] is the same thing with the process's real
    /// pipes attached.
    pub fn new(out: Sender<DebugWire>) -> Arc<Self> {
        Arc::new(Self {
            debuggees: Mutex::new(HashMap::new()),
            out,
        })
    }

    /// The `@DBG` link on this process's stdin/stdout.
    ///
    /// Spawns a reader that parses command lines off stdin and a pump that
    /// serialises wire lines onto stdout. **Idempotent** — stdin can only be
    /// claimed once, so every caller gets the one router, which is what lets
    /// the root form and each child ask for wiring independently.
    ///
    /// Call this only when a session was actually asked for: it takes stdin.
    pub fn stdio() -> Arc<Self> {
        static STDIO: OnceLock<Arc<DebugRouter>> = OnceLock::new();
        Arc::clone(STDIO.get_or_init(|| {
            let (out_tx, out_rx) = mpsc::channel::<DebugWire>();
            let router = DebugRouter::new(out_tx);

            // Wire lines → stdout (whole lines; `println!` locks stdout, so
            // interleaving with DISPLAY output stays line-atomic).
            std::thread::spawn(move || {
                use std::io::Write;
                for wire in out_rx.iter() {
                    match serde_json::to_string(&wire) {
                        Ok(json) => {
                            println!("@DBG {json}");
                            let _ = std::io::stdout().flush();
                        }
                        Err(e) => eprintln!("debug: cannot serialize DebugWire: {e}"),
                    }
                }
            });

            // stdin → the addressed debuggee.
            {
                let router = Arc::clone(&router);
                std::thread::spawn(move || {
                    use std::io::BufRead;
                    let stdin = std::io::stdin();
                    for line in stdin.lock().lines().map_while(Result::ok) {
                        let Some(json) = line.strip_prefix("@DBG ") else {
                            continue;
                        };
                        match parse_inbound(json) {
                            Some(msg) => router.dispatch(msg),
                            None => eprintln!("debug: bad @DBG command: {json}"),
                        }
                    }
                });
            }

            router
        }))
    }

    /// Enrol one interpreter and hand back what it needs to join the session.
    ///
    /// Announces `Attached` upstream, and spawns the small thread that stamps
    /// this debuggee's handle onto everything it emits. When that interpreter
    /// finishes, its event sender drops, the thread ends and `Detached`
    /// follows on its own — so a closing form needs no bookkeeping at the
    /// call site.
    pub fn register(self: &Arc<Self>, handle: &str, form: &str) -> DebugWiring {
        let (cmd_tx, cmd_rx) = mpsc::channel::<DebugCmd>();
        let (ev_tx, ev_rx) = mpsc::channel::<DebugEvent>();
        let breakpoints = new_breakpoints();
        let scope = new_user_scope();

        if let Ok(mut map) = self.debuggees.lock() {
            map.insert(
                handle.to_owned(),
                Debuggee {
                    cmd_tx,
                    breakpoints: Arc::clone(&breakpoints),
                    scope: Arc::clone(&scope),
                },
            );
        }
        let _ = self.out.send(DebugWire::Attached {
            handle: handle.to_owned(),
            form: form.to_owned(),
        });

        {
            let router = Arc::clone(self);
            let handle = handle.to_owned();
            std::thread::spawn(move || {
                for ev in ev_rx.iter() {
                    // The program has come to rest: from here until the next
                    // resuming command NO window takes input — the whole
                    // application is stopped, not just this form.
                    if matches!(ev, DebugEvent::Stopped { .. }) {
                        set_paused(true);
                    }
                    if router
                        .out
                        .send(DebugWire::Event {
                            handle: handle.clone(),
                            event: ev,
                        })
                        .is_err()
                    {
                        break;
                    }
                }
                router.unregister(&handle);
            });
        }

        (cmd_rx, ev_tx, breakpoints, scope)
    }

    /// Drop a debuggee and say so upstream. Idempotent: announcing a
    /// `Detached` twice would have the IDE forget a handle it has already
    /// forgotten, or worse, one that has been re-announced.
    pub fn unregister(&self, handle: &str) {
        let existed = self
            .debuggees
            .lock()
            .map(|mut map| map.remove(handle).is_some())
            .unwrap_or(false);
        if existed {
            let _ = self.out.send(DebugWire::Detached {
                handle: handle.to_owned(),
            });
        }
    }

    /// Deliver one inbound message to the debuggee it names.
    pub fn dispatch(&self, msg: RemoteDebugMsg) {
        let target = msg.target.as_deref().unwrap_or(ROOT_HANDLE);
        // Clone what is needed and release the map: nothing below should run
        // while every other debuggee's registration is blocked behind it.
        let found = self.debuggees.lock().ok().and_then(|map| {
            map.get(target).map(|d| {
                (
                    d.cmd_tx.clone(),
                    Arc::clone(&d.breakpoints),
                    Arc::clone(&d.scope),
                )
            })
        });
        let Some((cmd_tx, breakpoints, scope)) = found else {
            // A command for a form that has closed. Dropping it is right —
            // and saying so is what stops it looking like a lost command.
            eprintln!("debug: no debuggee {target} for {:?}", msg.cmd);
            return;
        };
        match msg.cmd {
            RemoteDebugCmd::Cmd(c) => {
                // Every command resumes the program except a Query, which
                // deliberately answers without moving it, and a Pause, which
                // asks it to STOP. Clearing the flag as the command goes in —
                // rather than waiting to be told the program moved — means the
                // window is live again on the same frame the developer pressed
                // Continue.
                if resumes(&c) {
                    set_paused(false);
                }
                let _ = cmd_tx.send(c);
            }
            RemoteDebugCmd::SetBreakpoints(lines) => {
                if let Ok(mut guard) = breakpoints.lock() {
                    *guard = lines.into_iter().collect();
                }
            }
            // "Only my code": the IDE hands over the generated `.cbl` lines
            // that hold the developer's own handler and procedure bodies, and
            // stepping crosses everything else without stopping. Shared, so
            // the toggle takes effect mid-session without restarting the form.
            RemoteDebugCmd::SetUserScope {
                user_only,
                user_lines,
            } => {
                if let Ok(mut guard) = scope.lock() {
                    guard.user_only = user_only;
                    guard.user_lines = user_lines.into_iter().collect();
                }
            }
        }
    }
}

/// Open the `@DBG` link for a session with ONE debuggee — the root form.
///
/// The single-form shorthand for [`DebugRouter::stdio`] +
/// [`DebugRouter::register`], kept so every caller that predates spec 061
/// compiles and behaves exactly as it did.
///
/// Call this only when a session was actually asked for — it takes stdin.
pub fn stdio_debug_wiring() -> DebugWiring {
    let router = DebugRouter::stdio();
    // No form name: the root form is the one the IDE launched, so it already
    // knows which file this debuggee's stops belong to.
    router.register(ROOT_HANDLE, "")
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
    ///
    /// ⚠️ `PAUSED` is process-wide, and `cargo test` runs this file's tests in
    /// ONE process, in parallel. No other test here may emit a
    /// `DebugEvent::Stopped` — the router sets the flag on one — or this
    /// assertion fails for a reason that has nothing to do with it. See the
    /// note in `router_tests`.
    #[test]
    fn a_form_with_no_debug_session_is_never_blocked() {
        assert!(
            !is_paused(),
            "the paused flag must start clear — a form run normally takes input"
        );
    }
}

#[cfg(test)]
mod router_tests {
    use super::*;
    use std::time::Duration;

    /// ⚠️ **No test below may emit `DebugEvent::Stopped`.** The router sets
    /// the process-wide `PAUSED` on one, and `a_form_with_no_debug_session_is_never_blocked`
    /// asserts that flag is clear — in the same process, in parallel. Use
    /// `Resumed`, which carries the same routing information and no state.
    const _: () = ();

    fn drain(out: &Receiver<DebugWire>) -> Vec<DebugWire> {
        let mut seen = Vec::new();
        while let Ok(w) = out.try_recv() {
            seen.push(w);
        }
        seen
    }

    /// A command addressed to one debuggee reaches it **and no other**.
    ///
    /// The absence is the point: with a shared `Receiver` — the design this
    /// one exists to avoid — whichever interpreter was blocked in `recv`
    /// first would swallow the other's `Continue`, silently.
    #[test]
    fn a_command_reaches_its_target_and_no_one_else() {
        // Held, not dropped: the router's forwarding threads send into this
        // end, and a closed receiver would make them give up early.
        let (out_tx, _out_rx) = mpsc::channel::<DebugWire>();
        let router = DebugRouter::new(out_tx);
        let (w0_cmd, _w0_ev, _w0_bps, _w0_scope) = router.register(ROOT_HANDLE, "MAIN");
        let (w1_cmd, _w1_ev, _w1_bps, _w1_scope) = router.register("W1", "CALLED");

        router.dispatch(RemoteDebugMsg {
            target: Some("W1".into()),
            cmd: RemoteDebugCmd::Cmd(DebugCmd::Continue),
        });

        assert!(
            matches!(
                w1_cmd.recv_timeout(Duration::from_secs(2)),
                Ok(DebugCmd::Continue)
            ),
            "the addressed debuggee must receive the command"
        );
        assert!(
            w0_cmd.try_recv().is_err(),
            "the OTHER debuggee must receive nothing — this is the whole \
             reason each one has its own channel"
        );
    }

    /// No target means the root, so an IDE that knows nothing of handles
    /// still drives the root form.
    #[test]
    fn an_unaddressed_command_goes_to_the_root() {
        let (out_tx, _out_rx) = mpsc::channel::<DebugWire>();
        let router = DebugRouter::new(out_tx);
        let (w0_cmd, _ev, _bps, _scope) = router.register(ROOT_HANDLE, "MAIN");
        let (w1_cmd, _ev1, _bps1, _scope1) = router.register("W1", "CALLED");

        router.dispatch(RemoteDebugMsg {
            target: None,
            cmd: RemoteDebugCmd::Cmd(DebugCmd::StepOver),
        });

        assert!(
            matches!(
                w0_cmd.recv_timeout(Duration::from_secs(2)),
                Ok(DebugCmd::StepOver)
            ),
            "an unaddressed command belongs to the root debuggee"
        );
        assert!(w1_cmd.try_recv().is_err(), "and to nobody else");
    }

    /// A bare `RemoteDebugCmd` — what an IDE predating spec 061 sends — is
    /// still understood, and means the root.
    #[test]
    fn a_bare_command_line_still_parses_as_the_root() {
        let json = serde_json::to_string(&RemoteDebugCmd::Cmd(DebugCmd::StepIn)).unwrap();
        let msg = parse_inbound(&json).expect("a bare command must still parse");
        assert!(msg.target.is_none(), "a bare command addresses the root");
        assert!(matches!(msg.cmd, RemoteDebugCmd::Cmd(DebugCmd::StepIn)));

        // And the envelope is preferred when it is the thing on the wire.
        let json = serde_json::to_string(&RemoteDebugMsg {
            target: Some("W2".into()),
            cmd: RemoteDebugCmd::Cmd(DebugCmd::StepOut),
        })
        .unwrap();
        let msg = parse_inbound(&json).expect("an envelope must parse");
        assert_eq!(msg.target.as_deref(), Some("W2"));
    }

    /// Every event carries the handle of the debuggee that produced it.
    #[test]
    fn events_come_out_stamped_with_their_debuggee() {
        let (out_tx, out_rx) = mpsc::channel::<DebugWire>();
        let router = DebugRouter::new(out_tx);
        let (_c0, w0_ev, _b0, _s0) = router.register(ROOT_HANDLE, "MAIN");
        let (_c1, w1_ev, _b1, _s1) = router.register("W1", "CALLED");

        // `Resumed`, never `Stopped` — see the note at the top of this module.
        w1_ev.send(DebugEvent::Resumed).unwrap();
        w0_ev.send(DebugEvent::Resumed).unwrap();
        std::thread::sleep(Duration::from_millis(100));

        let handles: Vec<String> = drain(&out_rx)
            .into_iter()
            .filter_map(|w| match w {
                DebugWire::Event { handle, .. } => Some(handle),
                _ => None,
            })
            .collect();
        assert!(
            handles.contains(&"W1".to_string()) && handles.contains(&ROOT_HANDLE.to_string()),
            "both debuggees' events must arrive, each under its own handle: {handles:?}"
        );
    }

    /// **A breakpoint belongs to a program.** Two forms compiled into one
    /// process can both carry a line 42, and it is two breakpoints.
    #[test]
    fn each_debuggee_gets_its_own_breakpoints_and_scope() {
        let (out_tx, _out_rx) = mpsc::channel::<DebugWire>();
        let router = DebugRouter::new(out_tx);
        let (_c0, _e0, w0_bps, w0_scope) = router.register(ROOT_HANDLE, "MAIN");
        let (_c1, _e1, w1_bps, _w1_scope) = router.register("W1", "CALLED");

        router.dispatch(RemoteDebugMsg {
            target: Some("W1".into()),
            cmd: RemoteDebugCmd::SetBreakpoints(vec![42]),
        });
        router.dispatch(RemoteDebugMsg {
            target: Some("W1".into()),
            cmd: RemoteDebugCmd::SetUserScope {
                user_only: true,
                user_lines: vec![7],
            },
        });

        assert!(
            w1_bps.lock().unwrap().contains(&42),
            "the addressed form's breakpoint is set"
        );
        assert!(
            w0_bps.lock().unwrap().is_empty(),
            "the OTHER form's line 42 is a different breakpoint, and was not set"
        );
        assert!(
            !w0_scope.lock().unwrap().user_only,
            "nor is the other form's scope touched"
        );
    }

    /// Joining and leaving are both announced — the IDE learns a debuggee
    /// exists, and learns when to forget it.
    #[test]
    fn attaching_and_detaching_are_announced() {
        let (out_tx, out_rx) = mpsc::channel::<DebugWire>();
        let router = DebugRouter::new(out_tx);
        let (_cmd, ev_tx, _bps, _scope) = router.register("W1", "CALLED");

        let attached = drain(&out_rx);
        assert!(
            attached.iter().any(|w| matches!(
                w,
                DebugWire::Attached { handle, form } if handle == "W1" && form == "CALLED"
            )),
            "register announces the debuggee and its form: {attached:?}"
        );

        // The interpreter finishes: its sender drops, and `Detached` follows
        // without anyone having to remember to say so.
        drop(ev_tx);
        std::thread::sleep(Duration::from_millis(100));
        let gone = drain(&out_rx);
        assert!(
            gone.iter()
                .any(|w| matches!(w, DebugWire::Detached { handle } if handle == "W1")),
            "a finished interpreter detaches itself: {gone:?}"
        );

        // And saying it twice is not a thing that happens.
        router.unregister("W1");
        assert!(
            !drain(&out_rx)
                .iter()
                .any(|w| matches!(w, DebugWire::Detached { .. })),
            "unregister is idempotent — a second Detached would have the IDE \
             forget a handle it may since have seen re-announced"
        );
    }
}
