// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! The seam between the server and what runs an exported program (spec 114 T5).
//!
//! The server owns the wire: the sockets, the tokens, the limits, the idempotency
//! store, the order of a connection's calls and the log. What it needs from an
//! engine is small — **run this program with these arguments and give back the
//! final JSON of each parameter** — and it is stated here as two traits so that a
//! second engine (a WebAssembly module in a prebuilt runner, spec 114) can stand
//! where the interpreter stands without the server knowing which one it holds.
//!
//! * [`Engine`] runs a call in its own run unit (`[backend] state = "per-call"`) and
//!   makes the per-connection session.
//! * [`EngineSession`] is one connection's run units in `per-connection` mode: it
//!   binds to the first caller with a subject, keeps one run unit per exported
//!   program, and takes the connection's calls in turn.
//! * A **stream** subscription is [`Engine::execute`] with [`ExecOptions::emit`] set:
//!   the program ends when it ends or is cancelled and has no answer to read back.
//!
//! The rules that are not the engine's — the arity of a call, what a program that
//! declares a `status` group of its `RETURNING` record reports, the binding of a
//! session to its first caller — live here, so both engines apply the same ones.
//!
//! This module links nothing of the interpreter: the headless server builds
//! without it (`--no-default-features --features server`).

use std::collections::HashMap;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;

use cobolt_wire::call::BackendFiles;
use cobolt_wire::error::{codes, CallError, ErrorClass};

use crate::backend::ExportedProgram;
use crate::config::Configurations;

/// What `COBOL::"AUTH-OP"` reaches: one identity operation, its `AUTH-REQUEST`
/// group as JSON in, its `AUTH-RESULT` group as JSON out (WASM-009 D4). The
/// backend's identity service implements it; an engine hands it to the program.
pub trait IdentityOps: Send + Sync {
    fn run(&self, operation: &str, request_json: &str) -> Result<String, CallError>;
}

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
    /// `backend.toml`'s `[[configuration]]` entries, for a declared control.
    pub configurations: Arc<Configurations>,
    /// The identity service, for a generated `AUTH-*` program run by the
    /// `/auth/*` routes (WASM-009 D4); never set for a WebSocket call.
    pub identity: Option<Arc<dyn IdentityOps>>,
}

impl ExecOptions {
    pub fn plain(cancel: Arc<AtomicBool>, file_lock_wait: std::time::Duration) -> Self {
        ExecOptions {
            claims: HashMap::new(),
            cancel,
            cancel_requested: Arc::new(AtomicBool::new(false)),
            file_lock_wait,
            progress: None,
            emit: None,
            files: None,
            configurations: Arc::default(),
            identity: None,
        }
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

/// Something that runs the project's exported programs for the server.
///
/// An engine is shared by every connection, so a call enters through `&self`; the
/// state of a `per-connection` run lives in the [`EngineSession`] it makes.
pub trait Engine: Send + Sync {
    /// Its name, for the log and for a test's result block: `interpreter`.
    fn name(&self) -> &'static str;

    /// Run `prog` with `args` (one JSON text per parameter) in a run unit of its own:
    /// fresh WORKING-STORAGE, nothing kept afterwards.
    fn execute(&self, prog: &ExportedProgram, args: &[String], opts: ExecOptions) -> Result<ExecResult, CallError>;

    /// One connection's session, for `[backend] state = "per-connection"`.
    fn session(&self) -> Box<dyn EngineSession>;
}

/// One connection's run units in `per-connection` mode (WASM-003 Q2).
///
/// Each exported program the connection calls keeps one run unit, so its
/// WORKING-STORAGE (and a file it left open) carries over from one call to the
/// next. A call that is cut off, cancelled or ends with a fault ends that
/// program's run unit: its next call starts fresh. The session is bound to the
/// first caller it serves and ends with the connection; nothing is resumed
/// across connections.
pub trait EngineSession: Send {
    /// Bind the session to the first caller with a subject; refuse another one.
    fn admit(&mut self, subject: Option<&str>) -> Result<(), CallError>;

    /// Run `prog` in this session's run unit for it.
    fn execute(&mut self, prog: &ExportedProgram, args: &[String], opts: ExecOptions) -> Result<ExecResult, CallError>;

    /// How many programs hold state in this session.
    fn programs(&self) -> usize;
}

/// The binding of a session to its first caller, the same for every engine.
#[derive(Debug, Default)]
pub struct Admission {
    subject: Option<String>,
}

impl Admission {
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
}

/// A call carries as many arguments as the program has parameters.
pub fn check_arity(prog: &ExportedProgram, args: &[String]) -> Result<(), CallError> {
    if args.len() != prog.params.len() {
        return Err(CallError::new(
            ErrorClass::Validation,
            codes::INVALID_PAYLOAD,
            format!("{} takes {} argument(s), the call carries {}", prog.sig.name, prog.params.len(), args.len()),
        ));
    }
    Ok(())
}

/// Read the declared status group of a program's RETURNING record (WASM-014 R8). A code of `0` is success (`None`); 1 to 7999 is the
/// program's own failure, answered exactly as `COBOL::"WS-FAIL"` would (class `application`, that code, the message and the detail);
/// any other code is the program's fault, logged, and the caller is told only `server 9001`.
///
/// `answer` reads the final JSON of the `RETURNING` record from the engine; it is called only when the program declares the group,
/// so an engine reads the record no more often than it did before the rule moved here.
pub fn status_failure(sig: &cobolt_wire::contract::ProgramSig, group: &str, answer: impl FnOnce() -> Result<String, CallError>) -> Result<Option<CallError>, CallError> {
    use cobolt_wire::layout::Kind;
    // The members are found by position in the declared layout (the shape was checked when the backend loaded).
    let names: Vec<String> = match sig.returning.as_ref().map(|r| &r.layout.kind) {
        Some(Kind::Group { children }) => match children.iter().find(|c| c.name.eq_ignore_ascii_case(group)).map(|g| &g.kind) {
            Some(Kind::Group { children }) => children.iter().map(|c| c.name.clone()).collect(),
            _ => Vec::new(),
        },
        _ => Vec::new(),
    };
    let [code_name, message_name, detail_name] = names.as_slice() else { return Ok(None) };
    let json: serde_json::Value = serde_json::from_str(&answer()?).map_err(|_| CallError::new(ErrorClass::Server, codes::PROGRAM_FAULT, "the program's answer could not be read"))?;
    let Some(g) = json.as_object().and_then(|o| o.iter().find(|(k, _)| k.eq_ignore_ascii_case(group))).map(|(_, v)| v) else { return Ok(None) };
    let field = |n: &str| g.as_object().and_then(|o| o.iter().find(|(k, _)| k.eq_ignore_ascii_case(n))).map(|(_, v)| v);
    let code = match field(code_name) {
        Some(serde_json::Value::Number(n)) => n.as_i64().unwrap_or(-1),
        Some(serde_json::Value::String(s)) => s.trim().parse().unwrap_or(-1),
        _ => 0,
    };
    if code == 0 {
        return Ok(None);
    }
    let text = |n: &str| field(n).and_then(serde_json::Value::as_str).map(|s| s.trim_end().to_string()).unwrap_or_default();
    if !(1..=7999).contains(&code) {
        tracing::error!(target: "exec", program = %sig.name, "the status code {code} is outside 1 to 7999");
        return Err(CallError::new(ErrorClass::Server, codes::PROGRAM_FAULT, "the program reported a status code outside the range it may use"));
    }
    let mut e = CallError::new(ErrorClass::Application, code as u32, text(message_name));
    let detail = text(detail_name);
    e.detail = (!detail.is_empty()).then_some(detail);
    Ok(Some(e))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_session_keeps_to_its_first_caller() {
        let mut a = Admission::default();
        assert!(a.admit(None).is_ok(), "a call with no subject binds nothing");
        assert!(a.admit(Some("ana")).is_ok(), "the first subject binds the session");
        assert!(a.admit(Some("ana")).is_ok(), "the same subject again");
        assert!(a.admit(None).is_ok(), "a call with no subject is not refused once bound");
        let e = a.admit(Some("bia")).expect_err("another subject");
        assert_eq!((e.class, e.code), (ErrorClass::Authorisation, codes::ROLE_REFUSED));
    }
}
