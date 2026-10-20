// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Running one exported program for one call (WASM-003 R3, R15, R18).
//!
//! Each call gets its **own interpreter** built from the program's AST: fresh
//! WORKING-STORAGE, no state shared with another caller, nothing left behind
//! (WASM-003 Q2: the model that scales and cannot leak between users). A
//! program that needs state keeps it in a file or a database.
//!
//! The arguments are read **into the program's LINKAGE items by the mapping**
//! before it starts, so a payload with an unknown field, a number where text
//! belongs, a string over its `PIC` or a table over its `OCCURS` is refused
//! and the program never runs (R15). After it ends, the final value of each
//! parameter (and of the `RETURNING` item) is written back out by the same
//! mapping.

use std::collections::HashMap;
use std::panic::AssertUnwindSafe;
use std::sync::atomic::AtomicBool;
use std::sync::{mpsc, Arc};

use cobolt_runtime::error::RuntimeError;
use cobolt_runtime::Interpreter;
use cobolt_wire::error::{codes, CallError, ErrorClass};

use crate::backend::ExportedProgram;

/// What a call needs besides its arguments.
pub struct ExecOptions {
    /// The caller's claims, readable by `COBOL::"WS-CLAIM"`.
    pub claims: HashMap<String, String>,
    /// Set to stop the program at its next statement (timeout, cancel, shutdown).
    pub cancel: Arc<AtomicBool>,
    /// How long an OPEN waits for a file another call holds (WASM-003 R19).
    pub file_lock_wait: std::time::Duration,
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

/// Run `prog` with `args` (one JSON text per parameter).
pub fn execute(prog: &ExportedProgram, args: &[String], opts: ExecOptions) -> Result<ExecResult, CallError> {
    if args.len() != prog.params.len() {
        return Err(CallError::new(
            ErrorClass::Validation,
            codes::INVALID_PAYLOAD,
            format!("{} takes {} argument(s), the call carries {}", prog.sig.name, prog.params.len(), args.len()),
        ));
    }
    let (_event_tx, event_rx) = mpsc::channel();
    let (state_tx, _state_rx) = mpsc::channel();
    let (display_tx, display_rx) = mpsc::channel();
    let mut interp = Interpreter::new_with_channels(prog.program.clone(), event_rx, state_tx, display_tx);
    let cancel = opts.cancel.clone();
    interp.set_cancel_flag(opts.cancel);
    interp.set_remote_claims(opts.claims);
    interp.set_file_lock_wait(Some(opts.file_lock_wait));

    // 1. Read the arguments into the parameters, all or nothing each, before anything runs.
    for (name, json) in prog.params.iter().zip(args.iter()) {
        interp.group_from_json(name, json)?;
    }

    // 2. Run.
    let outcome = std::panic::catch_unwind(AssertUnwindSafe(|| interp.run()));
    let display: Vec<String> = display_rx.try_iter().collect();
    // `run` treats a cancellation as a clean end, so the flag is what tells.
    if cancel.load(std::sync::atomic::Ordering::SeqCst) {
        return Err(CallError::new(ErrorClass::Limit, codes::CALL_TIMEOUT, "the call was cancelled or ran over its time limit"));
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

    // 3. Read the parameters back out.
    let mut out = ExecResult { display, ..ExecResult::default() };
    for name in &prog.params {
        out.args.push(interp.group_to_json(name)?);
    }
    if let Some(r) = &prog.returning {
        out.returning = Some(interp.group_to_json(r)?);
    }
    Ok(out)
}
