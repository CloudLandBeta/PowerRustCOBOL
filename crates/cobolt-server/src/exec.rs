// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Running one exported program for one call (WASM-003 R3, R15, R18; WASM-014).
//!
//! By default (`[backend] state = "per-call"`) each call gets its **own
//! interpreter** built from the program's AST: fresh WORKING-STORAGE, no state
//! shared with another caller, nothing left behind (WASM-003 Q2: the model that
//! scales and cannot leak between users). A program that needs state keeps it
//! in a file or a database — or the project says `per-connection`, and a
//! [`Session`] keeps one run unit per program for the life of a connection.
//!
//! The arguments are read **into the program's LINKAGE items by the mapping**
//! before it starts, so a payload with an unknown field, a number where text
//! belongs, a string over its `PIC` or a table over its `OCCURS` is refused
//! and the program never runs (R15). After it ends, the final value of each
//! parameter (and of the `RETURNING` item) is written back out by the same
//! mapping.
//!
//! What a program can say besides its answer (WASM-014): `WS-FAIL` ends the call
//! with its own failure and **discards the parameters' changes**; `WS-PROGRESS`
//! and `WS-EMIT` send things while it runs; a cancel request is seen through
//! `WS-CANCELLED` and at the safe points, and the call then ends as *cancelled*
//! (the program's effects are not undone).

use std::collections::HashMap;
use std::panic::AssertUnwindSafe;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc};

use cobolt_runtime::error::RuntimeError;
use cobolt_runtime::interpreter::remote::BackendHooks;
use cobolt_runtime::{ExternalStore, FormEvent, Interpreter, StateUpdate};
use cobolt_wire::call::BackendFiles;
use cobolt_wire::error::{codes, CallError, ErrorClass};

use crate::backend::ExportedProgram;

/// What a call needs besides its arguments.
pub struct ExecOptions {
    /// The caller's claims, readable by `COBOL::"WS-CLAIM"`.
    pub claims: HashMap<String, String>,
    /// Set to stop the program at its next statement (timeout, shutdown, the end
    /// of the grace period after a cancel).
    pub cancel: Arc<AtomicBool>,
    /// Set when the caller asks to cancel; the program sees it and ends at a safe point.
    pub cancel_requested: Arc<AtomicBool>,
    /// How long an OPEN waits for a file another call holds (WASM-003 R19).
    pub file_lock_wait: std::time::Duration,
    /// `WS-PROGRESS` reports go here.
    pub progress: Option<Box<dyn Fn(u8, &str) + Send>>,
    /// `WS-EMIT` items go here; `false` means nobody is listening.
    pub emit: Option<Box<dyn Fn(&str) -> bool + Send>>,
    /// The files the caller may reach.
    pub files: Option<Arc<dyn BackendFiles>>,
}

impl ExecOptions {
    pub fn plain(cancel: Arc<AtomicBool>, file_lock_wait: std::time::Duration) -> Self {
        ExecOptions { claims: HashMap::new(), cancel, cancel_requested: Arc::new(AtomicBool::new(false)), file_lock_wait, progress: None, emit: None, files: None }
    }
}

/// What a finished call produced.
#[derive(Debug, Default)]
pub struct ExecResult {
    /// The final JSON of each parameter, in order.
    pub args: Vec<String>,
    pub returning: Option<String>,
    /// Lines the program DISPLAYed (debug log only; never sent to the caller).
    pub display: Vec<String>,
}

fn cancelled() -> CallError {
    CallError::new(ErrorClass::Application, codes::CANCELLED, "the call was cancelled")
}

/// One interpreter and the ends of its channels, alive as long as it is.
struct Unit {
    interp: Interpreter,
    display_rx: mpsc::Receiver<String>,
    _event_tx: mpsc::Sender<FormEvent>,
    _state_rx: mpsc::Receiver<StateUpdate>,
}

impl Unit {
    fn new(prog: &ExportedProgram, external: ExternalStore) -> Unit {
        let (event_tx, event_rx) = mpsc::channel();
        let (state_tx, state_rx) = mpsc::channel();
        let (display_tx, display_rx) = mpsc::channel();
        let interp = Interpreter::new_with_channels_and_store(prog.program.clone(), event_rx, state_tx, display_tx, external);
        Unit { interp, display_rx, _event_tx: event_tx, _state_rx: state_rx }
    }
}

fn check_arity(prog: &ExportedProgram, args: &[String]) -> Result<(), CallError> {
    if args.len() != prog.params.len() {
        return Err(CallError::new(
            ErrorClass::Validation,
            codes::INVALID_PAYLOAD,
            format!("{} takes {} argument(s), the call carries {}", prog.sig.name, prog.params.len(), args.len()),
        ));
    }
    Ok(())
}

/// Run `prog` with `args` (one JSON text per parameter). A `stream` program has
/// no answer to read back: it ends when it ends or is cancelled.
pub fn execute(prog: &ExportedProgram, args: &[String], opts: ExecOptions) -> Result<ExecResult, CallError> {
    check_arity(prog, args)?;
    let mut unit = Unit::new(prog, cobolt_runtime::new_external_store());
    let mut sound = false;
    run_call(&mut unit, prog, args, opts, &mut sound)
}

/// One connection's run units, in `per-connection` mode (WASM-003 Q2, plan §2.6).
///
/// Each exported program the connection calls gets one interpreter, built at
/// its first call and entered again at every later one, so its
/// `WORKING-STORAGE` (and a file it left open) carries over as for a program
/// `CALL`ed again in one COBOL run unit; the session's interpreters share one
/// copy of every `EXTERNAL` item. A call that is cut off, cancelled or fails
/// with a fault ends that program's run unit: its next call starts fresh. The
/// session is bound to the first caller it serves and ends with the connection
/// — nothing is resumed across connections.
#[derive(Default)]
pub struct Session {
    units: HashMap<String, Unit>,
    external: Option<ExternalStore>,
    subject: Option<String>,
}

impl Session {
    pub fn new() -> Session {
        Session::default()
    }

    /// Bind the session to the first caller with a subject; refuse another one.
    pub fn admit(&mut self, subject: Option<&str>) -> Result<(), CallError> {
        match (subject, &self.subject) {
            (None, _) => Ok(()),
            (Some(s), None) => {
                self.subject = Some(s.to_owned());
                Ok(())
            }
            (Some(s), Some(bound)) if s == bound => Ok(()),
            (Some(_), Some(_)) => Err(CallError::new(
                ErrorClass::Authorisation,
                codes::ROLE_REFUSED,
                "this connection keeps another user's state: open a new connection to call as another user",
            )),
        }
    }

    /// Run `prog` in this session's run unit for it.
    pub fn execute(&mut self, prog: &ExportedProgram, args: &[String], opts: ExecOptions) -> Result<ExecResult, CallError> {
        check_arity(prog, args)?;
        let external = self.external.get_or_insert_with(cobolt_runtime::new_external_store).clone();
        let mut unit = self.units.remove(&prog.sig.name).unwrap_or_else(|| Unit::new(prog, external));
        let mut sound = false;
        let result = run_call(&mut unit, prog, args, opts, &mut sound);
        if sound {
            self.units.insert(prog.sig.name.clone(), unit);
        }
        result
    }

    /// How many programs hold state in this session.
    pub fn programs(&self) -> usize {
        self.units.len()
    }
}

/// One call on `unit`. `sound` is set when the run unit ended in a way that
/// leaves it fit to be entered again (an answer, a refused payload, the
/// program's own failure); a cut-off, a cancel or a fault leaves it false.
fn run_call(unit: &mut Unit, prog: &ExportedProgram, args: &[String], opts: ExecOptions, sound: &mut bool) -> Result<ExecResult, CallError> {
    let interp = &mut unit.interp;
    let cancel = opts.cancel.clone();
    let requested = opts.cancel_requested.clone();
    interp.set_cancel_flag(opts.cancel);
    interp.set_remote_claims(opts.claims);
    interp.set_file_lock_wait(Some(opts.file_lock_wait));
    interp.set_backend_hooks(BackendHooks { progress: opts.progress, emit: opts.emit, files: opts.files, cancel_requested: opts.cancel_requested });

    // 1. Read the arguments into the parameters, all or nothing each, before anything runs.
    *sound = true;
    for (name, json) in prog.params.iter().zip(args.iter()) {
        interp.group_from_json(name, json)?;
    }
    *sound = false;

    // 2. Run.
    let outcome = std::panic::catch_unwind(AssertUnwindSafe(|| interp.run()));
    let display: Vec<String> = unit.display_rx.try_iter().collect();
    // `run` treats a cancellation as a clean end, so the flags are what tell.
    if cancel.load(Ordering::SeqCst) {
        return Err(if requested.load(Ordering::SeqCst) {
            cancelled()
        } else {
            CallError::new(ErrorClass::Limit, codes::CALL_TIMEOUT, "the call was cancelled or ran over its time limit")
        });
    }
    match outcome {
        Err(_) => {
            return Err(CallError::new(ErrorClass::Server, codes::PROGRAM_FAULT, "the program failed unexpectedly"));
        }
        Ok(Ok(())) | Ok(Err(RuntimeError::GoBack)) | Ok(Err(RuntimeError::StopRun)) => {}
        Ok(Err(RuntimeError::Cancelled)) => {
            return Err(CallError::new(ErrorClass::Limit, codes::CALL_TIMEOUT, "the call was cancelled or ran over its time limit"));
        }
        Ok(Err(e)) => {
            // The detail is for the operator's log; the caller learns only that it failed.
            tracing::error!(target: "exec", program = %prog.sig.name, "run-time error: {e}");
            return Err(CallError::new(ErrorClass::Server, codes::PROGRAM_FAULT, "the program stopped with a run-time error"));
        }
    }
    // The program's own failure outranks everything but a cut-off: the parameters are not sent back.
    if let Some(f) = interp.take_app_failure() {
        *sound = true;
        let mut e = CallError::new(ErrorClass::Application, f.code, f.message);
        e.detail = f.detail;
        return Err(e);
    }
    if interp.remote_cancelled() || requested.load(Ordering::SeqCst) {
        return Err(cancelled());
    }
    *sound = true;

    // 3. Read the parameters back out.
    let mut out = ExecResult { display, ..ExecResult::default() };
    if prog.sig.stream {
        return Ok(out);
    }
    for name in &prog.params {
        out.args.push(interp.group_to_json(name)?);
    }
    if let Some(r) = &prog.returning {
        out.returning = Some(interp.group_to_json(r)?);
    }
    Ok(out)
}
