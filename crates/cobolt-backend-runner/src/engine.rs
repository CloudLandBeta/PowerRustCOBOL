// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! The Backend's WebAssembly module as the server's engine (spec 114 T21, R8 per-call, R9, R11, D1, D4).
//!
//! The server owns the wire and everything around a call: the tokens, the limits, the idempotency store, the log. What it
//! asks of an engine is one thing: run this exported program with these arguments (one JSON text per parameter) and answer
//! the final JSON of each parameter and of `RETURNING`. Here a call is one run of `pr_backend_call` on an instance of the
//! module of its own, with a provider state of its own: the data capabilities are the Desktop runner's functions
//! (`cobolt_runner`), and nothing is shared between two calls but what they write to disk or to a database.
//!
//! Every answer is the one the interpreter's engine gives: a refused request is a `validation` error naming the member
//! (`INVALID_PAYLOAD`), a run-time error is the program's fault (`PROGRAM_FAULT`, the cause in the log only), a call the
//! server stopped is `CALL_TIMEOUT` or `CANCELLED` by the same rule `exec.rs` applies, and the declared `status` group of
//! `RETURNING` is read with the very function the interpreter's engine uses (`engine::status_failure`).

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;

use cobolt_runner::backend::{BackendModule, CallControl, Outcome};
use cobolt_runner::verbs::{CallHost, FileReply};
use cobolt_server::backend::ExportedProgram;
use cobolt_server::engine::{check_arity, status_failure, Admission, Engine, EngineSession, ExecOptions, ExecResult};
use cobolt_wasm_abi::backend::Refusal;
use cobolt_wire::call::BackendFiles;
use cobolt_wire::error::{codes, CallError, ErrorClass};
use sha2::{Digest, Sha256};

use crate::epoch::Ticker;

/// The engine that runs a Backend built as a module.
pub struct WasmEngine {
    module: Arc<BackendModule>,
    /// The sha256 of the module, for the operator's log (never sent to a caller).
    pub module_sha256: String,
    /// How many calls are running on a thread right now: a call the server stopped is counted until its thread has
    /// returned, which is the point of stopping it (the interpreter's could only be abandoned).
    in_flight: Arc<AtomicUsize>,
    _ticker: Ticker,
}

impl WasmEngine {
    /// Check, load and link `module` against the capabilities `provided`, and start the clock that ends its runaway calls.
    /// The words of what is wrong when it cannot be: a missing capability is named, each one.
    pub fn start(module: &[u8], code: Option<&cobolt_runner_engine::MachineCode>, provided: &[&str]) -> Result<WasmEngine, String> {
        let loaded =
            BackendModule::load(module, code, provided, cobolt_runner::cache_dir()).map_err(|m| cobolt_runner::messages::text(cobolt_runner::Lang::En, &m))?;
        let module_sha256 = hex(&Sha256::digest(module));
        let ticker = Ticker::start(loaded.ticker());
        Ok(WasmEngine { module: Arc::new(loaded), module_sha256, in_flight: Arc::new(AtomicUsize::new(0)), _ticker: ticker })
    }

    /// The calls running on a thread at this moment.
    pub fn in_flight(&self) -> usize {
        self.in_flight.load(Ordering::SeqCst)
    }

    /// A handle that reads [`WasmEngine::in_flight`] from elsewhere (a test's, after the engine has been handed over).
    pub fn in_flight_counter(&self) -> Arc<AtomicUsize> {
        self.in_flight.clone()
    }

    /// The module this engine runs.
    pub fn module(&self) -> &BackendModule {
        &self.module
    }
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

impl Engine for WasmEngine {
    fn name(&self) -> &'static str {
        "wasm"
    }

    fn execute(&self, prog: &ExportedProgram, args: &[String], opts: ExecOptions) -> Result<ExecResult, CallError> {
        let _running = Running::start(&self.in_flight);
        run(&self.module, prog, args, opts)
    }

    fn session(&self) -> Box<dyn EngineSession> {
        Box::new(Session { module: self.module.clone(), in_flight: self.in_flight.clone(), admission: Admission::default() })
    }
}

/// One connection's calls. This runner keeps no run unit across calls (a Backend that asks for that is refused when it
/// loads, in words), so a session only holds who it is bound to.
struct Session {
    module: Arc<BackendModule>,
    in_flight: Arc<AtomicUsize>,
    admission: Admission,
}

/// One call counted while its thread is in the module.
struct Running<'a>(&'a AtomicUsize);

impl<'a> Running<'a> {
    fn start(counter: &'a AtomicUsize) -> Running<'a> {
        counter.fetch_add(1, Ordering::SeqCst);
        Running(counter)
    }
}

impl Drop for Running<'_> {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::SeqCst);
    }
}

impl EngineSession for Session {
    fn admit(&mut self, subject: Option<&str>) -> Result<(), CallError> {
        self.admission.admit(subject)
    }

    fn execute(&mut self, prog: &ExportedProgram, args: &[String], opts: ExecOptions) -> Result<ExecResult, CallError> {
        let _running = Running::start(&self.in_flight);
        run(&self.module, prog, args, opts)
    }

    fn programs(&self) -> usize {
        0
    }
}

fn fault(text: &str) -> CallError {
    CallError::new(ErrorClass::Server, codes::PROGRAM_FAULT, text)
}

/// What the module path does not serve yet: said as the program's fault, with the construct, so a caller and the log both
/// learn why. A Build sends a Backend that uses one to the Cargo path before it gets here.
fn unsupported(what: &str) -> CallError {
    fault(&format!("this Backend runs as a module, which does not serve {what} yet"))
}

/// The error of a request the mapping refused, as the interpreter words it: the code's name, the member, and the member
/// as the detail. (The interpreter adds the mapping's own sentence after the member; the module's routines answer the
/// code and the member only: a recorded difference, `specs/WASM-PARITY.md`.)
pub fn refusal_error(r: &Refusal) -> CallError {
    CallError::validation(format!("{} at {}", refusal_name(r.code), r.path), r.path.clone())
}

/// The word the rules use for a mapping error code (the interpreter's `ErrorKind::code`); a code the module does not
/// define is an internal error, as it is there.
pub fn refusal_name(code: i32) -> &'static str {
    match code {
        1 => "syntax",
        2 => "duplicate-key",
        3 => "unknown-key",
        4 => "wrong-type",
        5 => "overflow",
        6 => "too-long",
        7 => "too-many-elements",
        8 => "too-few-elements",
        9 => "not-utf8",
        10 => "odo-mismatch",
        11 => "odo-out-of-range",
        _ => "internal",
    }
}

/// What the server gives the program's verbs for one call (A60): the interpreter's `BackendHooks` and the caller's claims,
/// as the runner's [`CallHost`]. The server's own functions do the work (the progress and the items go to the caller, a
/// file is resolved or published for the caller's subject); this only hands them over.
struct Host {
    claims: HashMap<String, String>,
    progress: Option<Box<dyn Fn(u8, &str) + Send>>,
    emit: Option<Box<dyn Fn(&str) -> bool + Send>>,
    files: Option<Arc<dyn BackendFiles>>,
    requested: Arc<AtomicBool>,
}

impl CallHost for Host {
    fn progress(&mut self, percent: u8, message: &str) {
        if let Some(report) = &self.progress {
            report(percent, message);
        }
    }

    fn emit(&mut self, item: &str) -> Option<bool> {
        self.emit.as_ref().map(|send| send(item))
    }

    fn claim(&self, name: &str) -> String {
        self.claims.get(name).cloned().unwrap_or_default()
    }

    fn cancel_requested(&self) -> bool {
        self.requested.load(Ordering::SeqCst)
    }

    fn file_path(&self, id: &str) -> FileReply {
        match &self.files {
            None => FileReply::NoBackend,
            Some(files) => files.resolve(id).map_or(FileReply::NoFile, |p| FileReply::Text(p.display().to_string())),
        }
    }

    fn publish(&self, path: &str) -> FileReply {
        match &self.files {
            None => FileReply::NoBackend,
            Some(files) => files.publish(std::path::Path::new(path)).map_or(FileReply::NoFile, FileReply::Text),
        }
    }
}

fn run(module: &BackendModule, prog: &ExportedProgram, args: &[String], opts: ExecOptions) -> Result<ExecResult, CallError> {
    check_arity(prog, args)?;
    if prog.control.is_some() {
        return Err(unsupported("a program that seeds a control"));
    }
    if opts.identity.is_some() {
        return Err(unsupported("an identity program"));
    }
    let cancel = opts.cancel.clone();
    let requested = opts.cancel_requested.clone();
    let host = Host { claims: opts.claims, progress: opts.progress, emit: opts.emit, files: opts.files, requested: requested.clone() };
    let texts: Vec<&str> = args.iter().map(String::as_str).collect();
    let called = match module.call_with(&prog.sig.name, &texts, &CallControl::on(cancel.clone()), Some(Box::new(host))) {
        Ok(c) => c,
        Err(e) => {
            // The runner's own failure (the module could not be started), not the program's: the detail is for the log.
            tracing::error!(target: "exec", program = %prog.sig.name, "the module could not be run: {e}");
            return Err(fault("the program could not be run"));
        }
    };
    // The flags are what tell a cut-off from an answer, as in the interpreter's engine: a call that finished while the
    // server was stopping it is stopped.
    if cancel.load(Ordering::SeqCst) {
        return Err(if requested.load(Ordering::SeqCst) {
            CallError::new(ErrorClass::Application, codes::CANCELLED, "the call was cancelled")
        } else {
            CallError::new(ErrorClass::Limit, codes::CALL_TIMEOUT, "the call was cancelled or ran over its time limit")
        });
    }
    let display = called.display;
    // What ended the run, in the order the interpreter's engine reads it: a request the mapping refused or the module could
    // not take, a call the server stopped, a run-time error; then the program's own failure, which outranks everything
    // below (the parameters are not sent back); then a cancel the caller asked for; then the answer.
    let answered = match called.outcome {
        Outcome::Answered(answer) => Ok(answer),
        // A record the program holds cannot be written, or its answer is too large: said only if nothing above outranks it.
        Outcome::Unwritable(r) => Err(refusal_error(&r)),
        Outcome::TooLarge => Err(CallError::new(ErrorClass::Limit, codes::FRAME_TOO_LARGE, "the answer is larger than this Backend can send")),
        Outcome::Refused(r) => return Err(refusal_error(&r)),
        Outcome::RequestTooLarge => return Err(CallError::new(ErrorClass::Limit, codes::FRAME_TOO_LARGE, "the request is larger than this Backend can take")),
        Outcome::BadRequest => {
            tracing::error!(target: "exec", program = %prog.sig.name, "the module refused the request frame the runner wrote");
            return Err(fault("the program could not be run"));
        }
        Outcome::Interrupted => return Err(CallError::new(ErrorClass::Limit, codes::CALL_TIMEOUT, "the call was cancelled or ran over its time limit")),
        Outcome::Trapped(why) => {
            // The detail is for the operator's log; the caller learns only that it failed.
            tracing::error!(target: "exec", program = %prog.sig.name, "run-time error: {why}");
            return Err(fault("the program stopped with a run-time error"));
        }
    };
    if let Some(f) = called.failure {
        let mut e = CallError::new(ErrorClass::Application, f.code, f.message);
        e.detail = f.detail;
        return Err(e);
    }
    if called.cancelled || requested.load(Ordering::SeqCst) {
        return Err(CallError::new(ErrorClass::Application, codes::CANCELLED, "the call was cancelled"));
    }
    // A stream program has no answer to read back: it ends when it ends or is cancelled, whatever its parameters hold.
    if prog.sig.stream {
        return Ok(ExecResult { display, ..ExecResult::default() });
    }
    let answer = answered?;
    // The second way to report a business failure (WASM-014 R8): the declared status group of the RETURNING record.
    if let (Some(group), Some(_)) = (prog.sig.status.as_deref(), prog.returning.as_deref()) {
        if let Some(e) = status_failure(&prog.sig, group, || answer.returning.clone().ok_or_else(|| fault("the program's answer could not be read")))? {
            return Err(e);
        }
    }
    Ok(ExecResult { args: answer.args, returning: answer.returning, display })
}
