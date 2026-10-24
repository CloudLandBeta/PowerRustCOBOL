// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! `CALL … WITH WEBSOCKET LINKAGE` (WASM-005): the interpreter's half.
//!
//! The statement's arguments are groups. This module reads each one out of the
//! environment into a `cobolt_wire::Node` tree shaped like its declaration,
//! hands the tree to the mapping, passes the JSON to the run's
//! [`Transport`], and writes what comes back into the groups — all of it
//! decoded and validated before any of it is written, so a reply that does not
//! fit leaves every group as it was (WASM-005 R6).
//!
//! The layout of an argument comes from its **declaration in the program's
//! AST**, not from the environment: the environment upper-cases every name,
//! and the wire key is the data-name *as declared*. The environment is where
//! the values are.

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, OnceLock};

use cobolt_ast::data::DataDecl;
use cobolt_ast::expr::Expr;
use cobolt_ast::program::{DataSection, Program};
use cobolt_ast::stmt::{CallArg, Stmt};
use cobolt_lexer::Span;
use cobolt_wire::call::{ArgMode, ArgPayload, BackendFiles, CallObserver, CallReply, CallRequest, DownloadRequest, Transport, UploadRequest};
use cobolt_wire::error::{codes, CallError, ErrorClass};
use cobolt_wire::layout::{self, Item, Kind};
use cobolt_wire::mapping::{self, DEFAULT_MAX_BYTES};
use cobolt_wire::node::Node;
use cobolt_wire::number;

use super::{call_arg_expr, collect_quals, Interpreter};
use crate::environment::{base_name, subscript_key, CobolEnvironment};
use crate::error::RuntimeError;
use crate::value::{CobolNumeric, CobolValue};

/// What the host of an exported program gives the interpreter (the backend does
/// this per call): where progress and stream items go, the files the caller may
/// reach, and the flag a cancel request sets (WASM-014).
pub struct BackendHooks {
    pub progress: Option<Box<dyn Fn(u8, &str) + Send>>,
    /// Send one stream item (the JSON of a group). `false` means nobody wants more.
    pub emit: Option<Box<dyn Fn(&str) -> bool + Send>>,
    pub files: Option<Arc<dyn BackendFiles>>,
    /// Set when the caller asks to cancel; the program reaches it with
    /// `COBOL::"WS-CANCELLED"` and `COBOL::"WS-PROGRESS"`.
    pub cancel_requested: Arc<AtomicBool>,
}

/// An application failure a backend program reported with `COBOL::"WS-FAIL"`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppFailure {
    pub code: u32,
    pub message: String,
    pub detail: Option<String>,
}

/// Things that happen to a pending call, for a host that shows them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RemoteEvent {
    Progress { percent: u8, message: String },
    Warning(String),
}

type DefaultTransport = Box<dyn Fn() -> Option<Box<dyn Transport>> + Send + Sync>;
static DEFAULT_TRANSPORT: OnceLock<DefaultTransport> = OnceLock::new();

/// Register, once per process, how a run gets the transport for a remote call
/// when its host installed none: every interpreter of the process - the root
/// form's, the child forms', the backend's own - asks this the first time it
/// calls. Returns false if one was registered already.
pub fn set_default_transport_factory(f: impl Fn() -> Option<Box<dyn Transport>> + Send + Sync + 'static) -> bool {
    DEFAULT_TRANSPORT.set(Box::new(f)).is_ok()
}

type EventSink = Box<dyn FnMut(RemoteEvent) + Send>;

/// The run's remote-call state.
#[derive(Default)]
pub struct RemoteState {
    /// What carries a `WEBSOCKET` call. `None` until a host installs one.
    pub(crate) transport: Option<Box<dyn Transport>>,
    /// How the last remote call failed, for `COBOL::"WS-LAST-ERROR"`.
    pub(crate) last_error: Option<CallError>,
    /// Warnings the backend attached to the last successful call.
    pub(crate) warnings: Vec<String>,
    /// The caller's claims, on the backend (`COBOL::"WS-CLAIM"`); read-only to
    /// the program.
    pub(crate) claims: HashMap<String, String>,
    /// Where a host wants to see progress and warnings of a pending call.
    pub(crate) events: Option<EventSink>,
    /// A host sets this to cancel the pending call (the interpreter's thread is
    /// blocked in it, so a button's own handler cannot).
    pub(crate) cancel: Arc<AtomicBool>,
    /// Present when this interpreter runs a program the backend exports.
    pub(crate) hooks: Option<BackendHooks>,
    pub(crate) app_failure: Option<AppFailure>,
    /// The program was ended because the caller asked it to cancel.
    pub(crate) cancelled: bool,
    /// The last call that failed with the backend perhaps having run it, and the
    /// key it carried: the same call made again right after reuses the key, so the
    /// backend answers from its store instead of running it twice (WASM-014 R10).
    pub(crate) retry: Option<(u64, String)>,
}

/// Feeds a transport's reports to the host and to the interpreter's own state.
struct Observer<'a> {
    events: &'a mut Option<EventSink>,
    warnings: &'a mut Vec<String>,
    cancel: &'a Arc<AtomicBool>,
}

impl CallObserver for Observer<'_> {
    fn progress(&mut self, percent: u8, message: &str) {
        if let Some(f) = self.events.as_mut() {
            f(RemoteEvent::Progress { percent, message: message.to_string() });
        }
    }

    fn warning(&mut self, message: &str) {
        self.warnings.push(message.to_string());
        if let Some(f) = self.events.as_mut() {
            f(RemoteEvent::Warning(message.to_string()));
        }
    }

    fn cancel_requested(&mut self) -> bool {
        self.cancel.load(Ordering::SeqCst)
    }
}

/// A fingerprint of a request, to recognise the same call made again.
fn fingerprint(r: &CallRequest) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    r.program.hash(&mut h);
    for a in &r.args {
        (a.mode == ArgMode::Reference).hash(&mut h);
        a.json.hash(&mut h);
    }
    r.wants_returning.hash(&mut h);
    h.finish()
}

/// A fresh idempotency key: 128 random bits from the standard library's randomly seeded hasher.
fn fresh_key() -> String {
    use std::hash::{BuildHasher, Hasher};
    let mut out = String::with_capacity(32);
    for salt in 0u64..2 {
        let mut h = std::collections::hash_map::RandomState::new().build_hasher();
        h.write_u64(salt);
        h.write_u128(std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0));
        out.push_str(&format!("{:016x}", h.finish()));
    }
    out
}

impl Interpreter {
    /// Ask the process-wide factory for a transport if this run has none.
    fn ensure_transport(&mut self) {
        if self.remote.transport.is_none() {
            if let Some(f) = DEFAULT_TRANSPORT.get() {
                self.remote.transport = f();
            }
        }
    }

    /// Make this interpreter the run of an exported program (the backend does
    /// this for each call).
    pub fn set_backend_hooks(&mut self, hooks: BackendHooks) {
        self.remote.hooks = Some(hooks);
    }

    /// The failure the program reported with `WS-FAIL`, if it did.
    pub fn take_app_failure(&mut self) -> Option<AppFailure> {
        self.remote.app_failure.take()
    }

    /// Was the program ended because the caller asked it to cancel?
    pub fn remote_cancelled(&self) -> bool {
        self.remote.cancelled
    }

    /// Where progress and warnings of a pending remote call are reported.
    pub fn set_remote_events(&mut self, sink: Box<dyn FnMut(RemoteEvent) + Send>) {
        self.remote.events = Some(sink);
    }

    /// Setting this flag cancels the remote call that is pending.
    pub fn remote_cancel_handle(&self) -> Arc<AtomicBool> {
        self.remote.cancel.clone()
    }

    /// Install what carries `CALL … WITH WEBSOCKET LINKAGE` for this run: the
    /// real WebSocket client, a loopback double in a test.
    pub fn set_remote_transport(&mut self, transport: Box<dyn Transport>) {
        self.remote.transport = Some(transport);
    }

    /// Is a transport installed?
    pub fn has_remote_transport(&self) -> bool {
        self.remote.transport.is_some()
    }

    /// How the last remote call failed, if it did.
    pub fn remote_last_error(&self) -> Option<&CallError> {
        self.remote.last_error.as_ref()
    }

    /// Warnings the backend attached to the last remote call.
    pub fn remote_warnings(&self) -> &[String] {
        &self.remote.warnings
    }

    /// Give the program the caller's claims (the backend does this per call).
    pub fn set_remote_claims(&mut self, claims: HashMap<String, String>) {
        self.remote.claims = claims;
    }

    // ── groups as JSON, for a host that is not a CALL statement ─────────────────

    /// The layout and the current value of the data item `name` (declared in
    /// this program or one containing it), as the mapping sees them. The
    /// backend uses this to read what an exported program left in its
    /// parameters; the golden-vector runner uses it to compare states.
    pub fn read_group(&mut self, name: &str) -> Result<(Item, Node), CallError> {
        let expr = Expr::Identifier(name.to_string(), Span::new(0, 0, 0, 0));
        let b = self.bind_group(&expr, Span::new(0, 0, 0, 0))?;
        Ok((b.item, b.node))
    }

    /// The JSON text of the data item `name` (WASM-005 R5).
    pub fn group_to_json(&mut self, name: &str) -> Result<String, CallError> {
        let (item, node) = self.read_group(name)?;
        mapping::encode(&item, &node).map_err(|e| CallError::validation(e.to_string(), e.path))
    }

    /// Read `json` into the data item `name`, by name (WASM-005 R6). Either the
    /// whole text is accepted and the changed leaves are written, or the item is
    /// left exactly as it was and the error names the member.
    pub fn group_from_json(&mut self, name: &str, json: &str) -> Result<(), CallError> {
        let expr = Expr::Identifier(name.to_string(), Span::new(0, 0, 0, 0));
        let b = self.bind_group(&expr, Span::new(0, 0, 0, 0))?;
        let new = mapping::decode(&b.item, &b.node, json, DEFAULT_MAX_BYTES)
            .map_err(|e| CallError::validation(e.to_string(), e.path))?;
        self.write_group(&b, &new);
        Ok(())
    }

    /// Write one elementary leaf of the data item `name` from its stored text,
    /// where `path` is the dotted path the vectors use (`REC.LINES(2).QTY`).
    /// Test and tooling support: it is the inverse of what [`Self::read_group`]
    /// dumps.
    pub fn set_group_leaf(&mut self, name: &str, path: &str, text: &str) -> Result<(), CallError> {
        let (item, mut node) = self.read_group(name)?;
        cobolt_wire::node::set_by_path(&item, &mut node, path, text).map_err(|m| CallError::validation(m, path.to_string()))?;
        let expr = Expr::Identifier(name.to_string(), Span::new(0, 0, 0, 0));
        let b = self.bind_group(&expr, Span::new(0, 0, 0, 0))?;
        self.write_group(&b, &node);
        Ok(())
    }

    // ── the statement ────────────────────────────────────────────────────────

    pub(super) fn exec_remote_call(
        &mut self,
        program: &Expr,
        using: &[CallArg],
        returning: Option<&Expr>,
        on_exception: &[Stmt],
        not_on_exception: &[Stmt],
        span: Span,
    ) -> Result<(), RuntimeError> {
        let name = self.eval_expr(program, span)?.as_display_string().trim().to_ascii_uppercase();

        let outcome = self.remote_round_trip(&name, using, returning, span);
        // The second way to read the failure (Q7): `RETURN-CODE` holds the
        // same numeric code `WS-LAST-ERROR` reports, 0 after a success. Only
        // the code — the class and the message are the built-in's.
        let code = outcome.as_ref().err().map_or(0, |e| e.code as i64);
        self.env.set("RETURN-CODE", CobolValue::from_i64(code));
        match outcome {
            Ok(()) => {
                self.remote.last_error = None;
                if !not_on_exception.is_empty() {
                    self.exec_stmts(not_on_exception)?;
                }
                Ok(())
            }
            Err(err) => {
                tracing::warn!(target: "remote", "CALL \"{name}\" WITH WEBSOCKET LINKAGE failed: {err}");
                self.remote.last_error = Some(err.clone());
                if on_exception.is_empty() {
                    // Without ON EXCEPTION a failed call is a run-time error: a
                    // statement that silently did nothing would let the program
                    // carry on with data it never received.
                    return Err(RuntimeError::General {
                        message: format!("CALL \"{name}\" WITH WEBSOCKET LINKAGE failed: {err}"),
                    });
                }
                self.exec_stmts(on_exception)
            }
        }
    }

    /// Read the groups, call, decode and write back.
    fn remote_round_trip(
        &mut self,
        name: &str,
        using: &[CallArg],
        returning: Option<&Expr>,
        span: Span,
    ) -> Result<(), CallError> {
        // 1. The groups out of the environment, as JSON.
        let mut bound: Vec<Bound> = Vec::with_capacity(using.len());
        let mut request = CallRequest { program: name.to_string(), ..CallRequest::default() };
        for arg in using {
            let (expr, mode) = match arg {
                CallArg::ByReference(e) => (e, ArgMode::Reference),
                CallArg::ByValue(e) | CallArg::ByContent(e) => (e, ArgMode::Value),
            };
            let b = self.bind_group(expr, span)?;
            let json = mapping::encode(&b.item, &b.node).map_err(|e| CallError::validation(e.to_string(), e.path.clone()))?;
            request.args.push(ArgPayload { mode, json });
            bound.push(b);
        }
        let ret = match returning {
            Some(e) => {
                request.wants_returning = true;
                Some(self.bind_group(e, span)?)
            }
            None => None,
        };

        // 2. The call.
        let reply = self.send_request(&mut request)?;

        // 3. Decode everything first. Nothing is written until all of it fits.
        let mut staged: Vec<(usize, Node)> = Vec::new();
        for (i, (b, a)) in bound.iter().zip(request.args.iter()).enumerate() {
            if a.mode != ArgMode::Reference {
                continue;
            }
            if let Some(Some(json)) = reply.args.get(i) {
                let new = mapping::decode(&b.item, &b.node, json, DEFAULT_MAX_BYTES)
                    .map_err(|e| bad_reply(&format!("the answer for argument {} does not fit its group: {e}", i + 1), &e.path))?;
                staged.push((i, new));
            }
        }
        let mut staged_ret: Option<Node> = None;
        if let (Some(b), Some(json)) = (&ret, &reply.returning) {
            let new = mapping::decode(&b.item, &b.node, json, DEFAULT_MAX_BYTES)
                .map_err(|e| bad_reply(&format!("the RETURNING answer does not fit its group: {e}"), &e.path))?;
            staged_ret = Some(new);
        }

        // 4. Write.
        for (i, new) in staged {
            let b = &bound[i];
            self.write_group(b, &new);
        }
        if let (Some(b), Some(new)) = (&ret, staged_ret) {
            self.write_group(b, &new);
        }
        Ok(())
    }

    /// Send one request over the run's transport and answer its reply — the one
    /// door to the transport, which the `CALL` statement and a bound grid's reads
    /// and saves (WASM-005 R13, `remote_binding`) both go through.
    ///
    /// The same call made again right after one the backend may have run carries
    /// the same idempotency key, so the backend answers from its store (WASM-014
    /// R10); any other call gets a key of its own. The reply's warnings join the
    /// run's.
    pub(super) fn send_request(&mut self, request: &mut CallRequest) -> Result<CallReply, CallError> {
        self.ensure_transport();
        let fp = fingerprint(request);
        let key = match self.remote.retry.take() {
            Some((f, k)) if f == fp => k,
            _ => fresh_key(),
        };
        request.idempotency_key = Some(key.clone());
        self.remote.cancel.store(false, Ordering::SeqCst);
        self.remote.warnings.clear();
        let RemoteState { transport, events, warnings, cancel, .. } = &mut self.remote;
        let Some(transport) = transport.as_mut() else {
            return Err(CallError::transport(
                codes::NOT_CONFIGURED,
                "no backend is configured for this run (the host installs one when the project names a backend)",
            )
            .may_have_run(false));
        };
        let mut observer = Observer { events, warnings, cancel };
        let outcome = transport.call_observed(request, &mut observer);
        let reply = match outcome {
            Ok(r) => r,
            Err(e) => {
                if e.class == ErrorClass::Transport && e.may_have_run != Some(false) {
                    self.remote.retry = Some((fp, key));
                }
                return Err(e);
            }
        };
        for w in &reply.warnings {
            if !self.remote.warnings.contains(w) {
                self.remote.warnings.push(w.clone());
            }
        }
        Ok(reply)
    }

    // ── an argument: declaration, layout, value ──────────────────────────────

    fn bind_group(&mut self, expr: &Expr, _span: Span) -> Result<Bound, CallError> {
        let (leaf, quals): (String, Vec<String>) = match expr {
            Expr::Identifier(n, _) => (n.clone(), Vec::new()),
            Expr::Qualified { name, of, .. } => (name.clone(), collect_quals(of)),
            _ => {
                return Err(CallError::validation(
                    "an argument of a WEBSOCKET call must be a data item, written as its name (no subscript, reference modification, literal or expression)",
                    String::new(),
                ))
            }
        };
        let key = self.env.resolve_name(&leaf, &quals);
        let Some(sym) = self.env.symbol(&key).cloned() else {
            return Err(CallError::validation(format!("{leaf} is not a declared data item"), leaf));
        };
        let origin = sym.origin.clone();
        let Some(decl) = find_decl(&self.program, &origin, &sym.quals, &leaf) else {
            return Err(CallError::validation(
                format!("the declaration of {leaf} was not found in the program that declares it"),
                leaf,
            ));
        };
        let decl = decl.clone();
        let item = layout::from_decl(&decl).map_err(|e| CallError::validation(e.to_string(), e.path.clone()))?;
        let mut anc: Vec<String> = sym.quals.clone();
        let mut subs: Vec<i64> = Vec::new();
        let node = read_node(&self.env, &decl, &item, &mut anc, &mut subs)
            .map_err(|(p, m)| CallError::validation(m, p))?;
        Ok(Bound { decl, item, node, quals: sym.quals })
    }

    fn write_group(&mut self, b: &Bound, new: &Node) {
        let mut anc: Vec<String> = b.quals.clone();
        let mut subs: Vec<i64> = Vec::new();
        write_node(&mut self.env, &b.decl, &b.item, &b.node, new, &mut anc, &mut subs);
    }

    // ── the built-ins ────────────────────────────────────────────────────────

    /// `COBOL::"WS-LAST-ERROR"( class code message [detail] )`.
    pub(super) fn builtin_ws_last_error(&mut self, using: &[CallArg]) {
        let (class, code, message, detail) = match &self.remote.last_error {
            Some(e) => (e.class.word().to_string(), e.code as i64, e.message.clone(), e.detail.clone().unwrap_or_default()),
            None => (String::new(), 0, String::new(), String::new()),
        };
        if let Some(a) = using.first() {
            let n = self.expr_to_name(call_arg_expr(a));
            self.env.set_str(&n, &class);
        }
        if let Some(a) = using.get(1) {
            let n = self.expr_to_name(call_arg_expr(a));
            self.env.set(&n, CobolValue::from_i64(code));
        }
        if let Some(a) = using.get(2) {
            let n = self.expr_to_name(call_arg_expr(a));
            self.env.set_str(&n, &message);
        }
        if let Some(a) = using.get(3) {
            let n = self.expr_to_name(call_arg_expr(a));
            self.env.set_str(&n, &detail);
        }
    }

    /// `COBOL::"WS-CLAIM"( name value )`: a claim of the caller's validated
    /// token, spaces when it has none. Read-only; there is no way to set one.
    pub(super) fn builtin_ws_claim(&mut self, using: &[CallArg], span: Span) -> Result<(), RuntimeError> {
        if using.len() < 2 {
            return Ok(());
        }
        let wanted = self.eval_call_arg(&using[0], span)?.as_display_string().trim().to_string();
        let value = self.remote.claims.get(&wanted).cloned().unwrap_or_default();
        let n = self.expr_to_name(call_arg_expr(&using[1]));
        self.env.set_str(&n, &value);
        Ok(())
    }

    // ── the backend's side of the contract (WASM-014) ────────────────────────

    fn in_backend(&self, what: &str) -> Result<(), RuntimeError> {
        if self.remote.hooks.is_some() {
            Ok(())
        } else {
            Err(RuntimeError::General { message: format!("COBOL::\"{what}\" belongs to a program the backend exports; this run is not serving a call") })
        }
    }

    /// `COBOL::"WS-FAIL"( code message [detail] )`: end the call with the
    /// program's own failure. The code is 1 to 7999; the parameters the program
    /// changed are not sent back.
    pub(super) fn builtin_ws_fail(&mut self, using: &[CallArg], span: Span) -> Result<(), RuntimeError> {
        self.in_backend("WS-FAIL")?;
        if using.len() < 2 {
            return Err(RuntimeError::General { message: "COBOL::\"WS-FAIL\" needs a code and a message".into() });
        }
        let code = self.eval_call_arg(&using[0], span)?.as_f64().round();
        if !(1.0..=codes::APPLICATION_MAX as f64).contains(&code) {
            return Err(RuntimeError::General {
                message: format!("COBOL::\"WS-FAIL\": the code must be 1 to {}; {} and above belong to the platform", codes::APPLICATION_MAX, codes::APPLICATION_MAX + 1),
            });
        }
        let message: String = self.eval_call_arg(&using[1], span)?.as_display_string().trim_end().chars().take(200).collect();
        let detail = match using.get(2) {
            Some(a) => {
                let d: String = self.eval_call_arg(a, span)?.as_display_string().trim_end().chars().take(4096).collect();
                (!d.is_empty()).then_some(d)
            }
            None => None,
        };
        self.remote.app_failure = Some(AppFailure { code: code as u32, message, detail });
        Err(RuntimeError::StopRun)
    }

    /// `COBOL::"WS-PROGRESS"( percent message )`: report progress. A safe point:
    /// when the caller has asked to cancel, the call ends here.
    pub(super) fn builtin_ws_progress(&mut self, using: &[CallArg], span: Span) -> Result<(), RuntimeError> {
        if self.remote.hooks.is_none() || using.is_empty() {
            return Ok(());
        }
        let pct = self.eval_call_arg(&using[0], span)?.as_f64().clamp(0.0, 100.0).round() as u8;
        let message: String = match using.get(1) {
            Some(a) => self.eval_call_arg(a, span)?.as_display_string().trim_end().chars().take(120).collect(),
            None => String::new(),
        };
        if let Some(h) = &self.remote.hooks {
            if let Some(f) = &h.progress {
                f(pct, &message);
            }
            if h.cancel_requested.load(Ordering::SeqCst) {
                self.remote.cancelled = true;
                return Err(RuntimeError::StopRun);
            }
        }
        Ok(())
    }

    /// `COBOL::"WS-CANCELLED"( flag )`: `Y` when the caller has asked to cancel.
    pub(super) fn builtin_ws_cancelled(&mut self, using: &[CallArg]) {
        let asked = self.remote.hooks.as_ref().is_some_and(|h| h.cancel_requested.load(Ordering::SeqCst));
        if let Some(a) = using.first() {
            let n = self.expr_to_name(call_arg_expr(a));
            self.env.set_str(&n, if asked { "Y" } else { "N" });
        }
    }

    /// `COBOL::"WS-PAUSE"( milliseconds )`: wait, in slices, ending early when the
    /// caller asks to cancel and stopping at once when the call is cut off.
    pub(super) fn builtin_ws_pause(&mut self, using: &[CallArg], span: Span) -> Result<(), RuntimeError> {
        let Some(a) = using.first() else { return Ok(()) };
        let ms = self.eval_call_arg(a, span)?.as_f64().clamp(0.0, 600_000.0) as u64;
        let end = std::time::Instant::now() + std::time::Duration::from_millis(ms);
        loop {
            if self.cancel.as_ref().is_some_and(|c| c.load(Ordering::SeqCst)) {
                return Err(RuntimeError::Cancelled);
            }
            if self.remote.hooks.as_ref().is_some_and(|h| h.cancel_requested.load(Ordering::SeqCst)) {
                return Ok(());
            }
            let now = std::time::Instant::now();
            if now >= end {
                return Ok(());
            }
            std::thread::sleep((end - now).min(std::time::Duration::from_millis(10)));
        }
    }

    /// `COBOL::"WS-EMIT"( group )`: a stream program sends the group as its next item.
    pub(super) fn builtin_ws_emit(&mut self, using: &[CallArg]) -> Result<(), RuntimeError> {
        self.in_backend("WS-EMIT")?;
        let Some(a) = using.first() else { return Ok(()) };
        let name = self.expr_to_name(call_arg_expr(a));
        let json = self.group_to_json(&name).map_err(|e| RuntimeError::General { message: format!("COBOL::\"WS-EMIT\": {e}") })?;
        let (sent, asked) = match &self.remote.hooks {
            Some(h) => (h.emit.as_ref().map(|f| f(&json)), h.cancel_requested.load(Ordering::SeqCst)),
            None => (None, false),
        };
        match sent {
            None => Err(RuntimeError::General { message: "COBOL::\"WS-EMIT\" is for a program exported as a stream".into() }),
            Some(true) if !asked => Ok(()),
            Some(_) => {
                self.remote.cancelled = true;
                Err(RuntimeError::StopRun)
            }
        }
    }

    /// `COBOL::"WS-FILE-PATH"( file-id path status )`.
    pub(super) fn builtin_ws_file_path(&mut self, using: &[CallArg], span: Span) -> Result<(), RuntimeError> {
        if using.len() < 3 {
            return Ok(());
        }
        let id = self.eval_call_arg(&using[0], span)?.as_display_string().trim().to_string();
        let (path, status) = match self.remote.hooks.as_ref().and_then(|h| h.files.as_ref()) {
            None => (String::new(), "NOBACKEND"),
            Some(files) => match files.resolve(&id) {
                Some(p) => (p.display().to_string(), "OK"),
                None => (String::new(), "NOFILE"),
            },
        };
        let (np, ns) = (self.expr_to_name(call_arg_expr(&using[1])), self.expr_to_name(call_arg_expr(&using[2])));
        self.env.set_str(&np, &path);
        self.env.set_str(&ns, status);
        Ok(())
    }

    /// `COBOL::"WS-FILE-PUBLISH"( path file-id status )`.
    pub(super) fn builtin_ws_file_publish(&mut self, using: &[CallArg], span: Span) -> Result<(), RuntimeError> {
        if using.len() < 3 {
            return Ok(());
        }
        let path = self.eval_call_arg(&using[0], span)?.as_display_string().trim().to_string();
        let (id, status) = match self.remote.hooks.as_ref().and_then(|h| h.files.as_ref()) {
            None => (String::new(), "NOBACKEND"),
            Some(files) => match files.publish(std::path::Path::new(&path)) {
                Ok(id) => (id, "OK"),
                Err(_) => (String::new(), "NOFILE"),
            },
        };
        let (ni, ns) = (self.expr_to_name(call_arg_expr(&using[1])), self.expr_to_name(call_arg_expr(&using[2])));
        self.env.set_str(&ni, &id);
        self.env.set_str(&ns, status);
        Ok(())
    }

    // ── the frontend's side ──────────────────────────────────────────────────

    /// `COBOL::"WS-WARNING"( message )`: the most recent warning the backend attached.
    pub(super) fn builtin_ws_warning(&mut self, using: &[CallArg]) {
        let w = self.remote.warnings.last().cloned().unwrap_or_default();
        if let Some(a) = using.first() {
            let n = self.expr_to_name(call_arg_expr(a));
            self.env.set_str(&n, &w);
        }
    }

    /// The status word of a file transfer, and `WS-LAST-ERROR`'s record of why.
    fn transfer_status(&mut self, result: Result<(), CallError>) -> &'static str {
        match result {
            Ok(()) => "OK",
            Err(e) => {
                let word = if e.class == ErrorClass::Transport { "FAILED" } else { "REFUSED" };
                self.remote.last_error = Some(e);
                word
            }
        }
    }

    /// `COBOL::"WS-UPLOAD"( slot path mime file-id status )`.
    pub(super) fn builtin_ws_upload(&mut self, using: &[CallArg], span: Span) -> Result<(), RuntimeError> {
        if using.len() < 5 {
            return Ok(());
        }
        let slot = self.eval_call_arg(&using[0], span)?.as_display_string().trim().to_string();
        let path = self.eval_call_arg(&using[1], span)?.as_display_string().trim().to_string();
        let mime = self.eval_call_arg(&using[2], span)?.as_display_string().trim().to_string();
        let (id_name, st_name) = (self.expr_to_name(call_arg_expr(&using[3])), self.expr_to_name(call_arg_expr(&using[4])));
        let p = std::path::PathBuf::from(super::anchor_relative_path(&path));
        let (file, status) = if !p.is_file() {
            (String::new(), "NOFILE")
        } else {
            self.ensure_transport();
            self.remote.cancel.store(false, Ordering::SeqCst);
            let name = p.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| "file".into());
            let RemoteState { transport, events, warnings, cancel, .. } = &mut self.remote;
            match transport.as_mut() {
                None => (String::new(), "NOBACKEND"),
                Some(t) => {
                    let mut observer = Observer { events, warnings, cancel };
                    let r = t.upload(&UploadRequest { slot, path: p, name, mime }, &mut observer);
                    match r {
                        Ok(done) => (done.file, "OK"),
                        Err(e) => {
                            let word = if e.class == ErrorClass::Transport { "FAILED" } else { "REFUSED" };
                            self.remote.last_error = Some(e);
                            (String::new(), word)
                        }
                    }
                }
            }
        };
        self.env.set_str(&id_name, &file);
        self.env.set_str(&st_name, status);
        Ok(())
    }

    /// `COBOL::"WS-DOWNLOAD"( file-id path status )`.
    pub(super) fn builtin_ws_download(&mut self, using: &[CallArg], span: Span) -> Result<(), RuntimeError> {
        if using.len() < 3 {
            return Ok(());
        }
        let file = self.eval_call_arg(&using[0], span)?.as_display_string().trim().to_string();
        let path = self.eval_call_arg(&using[1], span)?.as_display_string().trim().to_string();
        let st_name = self.expr_to_name(call_arg_expr(&using[2]));
        let dest = std::path::PathBuf::from(super::anchor_relative_path(&path));
        self.ensure_transport();
        self.remote.cancel.store(false, Ordering::SeqCst);
        let RemoteState { transport, events, warnings, cancel, .. } = &mut self.remote;
        let outcome: Result<(), CallError> = match transport.as_mut() {
            None => {
                self.env.set_str(&st_name, "NOBACKEND");
                return Ok(());
            }
            Some(t) => {
                let mut observer = Observer { events, warnings, cancel };
                t.download(&DownloadRequest { file, dest }, &mut observer).map(|_| ())
            }
        };
        let word = self.transfer_status(outcome);
        self.env.set_str(&st_name, word);
        Ok(())
    }
}

/// An argument read out of the environment.
struct Bound {
    decl: DataDecl,
    item: Item,
    node: Node,
    /// The declaration's ancestors within its record, outermost first.
    quals: Vec<String>,
}

fn bad_reply(message: &str, path: &str) -> CallError {
    CallError::new(ErrorClass::Validation, codes::INVALID_PAYLOAD, message.to_string()).with_detail(path.to_string())
}

// ── finding the declaration ───────────────────────────────────────────────────

/// The declaration of the item named `leaf`, within the program `origin`
/// (anywhere in the tree), under the ancestors `quals` (outermost first).
fn find_decl<'a>(root: &'a Program, origin: &str, quals: &[String], leaf: &str) -> Option<&'a DataDecl> {
    fn programs<'a>(p: &'a Program, out: &mut Vec<&'a Program>) {
        out.push(p);
        for n in &p.nested_programs {
            programs(n, out);
        }
    }
    let mut all = Vec::new();
    programs(root, &mut all);
    let named = |decl: &DataDecl, n: &str| decl.name.as_deref().is_some_and(|d| d.eq_ignore_ascii_case(n));
    let search = |prog: &'a Program| -> Option<&'a DataDecl> {
        let data = prog.data.as_ref()?;
        let tops: Vec<&'a DataDecl> = data
            .sections
            .iter()
            .flat_map(|s| -> Vec<&'a DataDecl> {
                match s {
                    DataSection::WorkingStorage(v) | DataSection::LocalStorage(v) | DataSection::Linkage(v) => v.iter().collect(),
                    DataSection::FileSection(fds) => fds.iter().flat_map(|fd| fd.records.iter()).collect(),
                    DataSection::Screen(_) => Vec::new(),
                }
            })
            .collect();
        let mut path: Vec<&str> = quals.iter().map(String::as_str).collect();
        path.push(leaf);
        let mut level: Vec<&'a DataDecl> = tops;
        let mut found: Option<&'a DataDecl> = None;
        for (i, step) in path.iter().enumerate() {
            let hit = level.iter().copied().find(|d| named(d, step))?;
            if i + 1 == path.len() {
                found = Some(hit);
            } else {
                level = hit.children.iter().collect();
            }
        }
        found
    };
    // The program that declared it first; any other program that has it, after.
    all.iter()
        .copied()
        .filter(|p| p.identification.program_id.eq_ignore_ascii_case(origin))
        .find_map(search)
        .or_else(|| all.iter().copied().find_map(search))
}

// ── environment <-> Node ──────────────────────────────────────────────────────

fn data_children(decl: &DataDecl) -> Vec<&DataDecl> {
    decl.children.iter().filter(|c| c.level != 88 && c.level != 66).collect()
}

fn key_of(env: &CobolEnvironment, decl: &DataDecl, anc: &[String], subs: &[i64]) -> String {
    let name = decl.name.as_deref().unwrap_or("FILLER");
    let quals: Vec<String> = anc.iter().rev().cloned().collect();
    let key = env.resolve_name(name, &quals);
    if subs.is_empty() {
        key
    } else {
        subscript_key(&key, subs)
    }
}

/// Read the value of an item (and its table) out of the environment.
fn read_node(
    env: &CobolEnvironment,
    decl: &DataDecl,
    item: &Item,
    anc: &mut Vec<String>,
    subs: &mut Vec<i64>,
) -> Result<Node, (String, String)> {
    match &item.occurs {
        None => read_base(env, decl, item, anc, subs),
        Some(o) => {
            let mut elems = Vec::with_capacity(o.max);
            for i in 1..=o.max {
                subs.push(i as i64);
                let r = read_base(env, decl, item, anc, subs);
                subs.pop();
                elems.push(r?);
            }
            Ok(Node::Table(elems))
        }
    }
}

fn read_base(
    env: &CobolEnvironment,
    decl: &DataDecl,
    item: &Item,
    anc: &mut Vec<String>,
    subs: &mut Vec<i64>,
) -> Result<Node, (String, String)> {
    match &item.kind {
        Kind::Group { children } => {
            let decls = data_children(decl);
            let mut nodes = Vec::with_capacity(children.len());
            let own = decl.name.clone().unwrap_or_default().to_ascii_uppercase();
            anc.push(own);
            let mut result = Ok(());
            for (c, d) in children.iter().zip(decls.iter()) {
                if !c.is_member() {
                    nodes.push(Node::initial(c));
                    continue;
                }
                match read_node(env, d, c, anc, subs) {
                    Ok(n) => nodes.push(n),
                    Err(e) => {
                        result = Err(e);
                        break;
                    }
                }
            }
            anc.pop();
            result?;
            Ok(Node::Group(nodes))
        }
        kind => {
            let key = key_of(env, decl, anc, subs);
            let val = env.get(&key);
            match kind {
                Kind::Numeric { digits, scale, signed } => {
                    let mantissa = match val {
                        Some(CobolValue::Numeric(n)) => {
                            if n.decimals as u16 == *scale {
                                n.mantissa
                            } else {
                                n.truncate_to(*scale as u8).mantissa
                            }
                        }
                        Some(CobolValue::String { bytes, .. }) => {
                            if bytes.iter().all(|b| *b == b' ') {
                                0
                            } else {
                                match val.and_then(|v| v.as_exact()) {
                                    Some(n) => n.truncate_to(*scale as u8).mantissa,
                                    None => {
                                        return Err((
                                            display_path(anc, decl, subs),
                                            format!("{} does not hold a number", decl.name.as_deref().unwrap_or("?")),
                                        ))
                                    }
                                }
                            }
                        }
                        _ => 0,
                    };
                    let text = number::from_mantissa(mantissa, *scale);
                    if !number::is_canonical(&text, *digits, *scale, *signed) {
                        return Err((
                            display_path(anc, decl, subs),
                            format!("{} holds {text}, which does not fit its PICTURE", decl.name.as_deref().unwrap_or("?")),
                        ));
                    }
                    Ok(Node::Leaf(text.into_bytes()))
                }
                _ => {
                    let bytes = match val {
                        Some(CobolValue::String { bytes, .. }) => bytes.clone(),
                        Some(other) => other.as_display_string().into_bytes(),
                        None => Vec::new(),
                    };
                    Ok(Node::Leaf(layout_pad(kind, bytes)))
                }
            }
        }
    }
}

/// Fit stored bytes to the item's declared width (a slot normally already is).
fn layout_pad(kind: &Kind, mut bytes: Vec<u8>) -> Vec<u8> {
    let width = match kind {
        Kind::Alphanumeric { len, .. } | Kind::Edited { len } => Some(*len),
        _ => None,
    };
    if let Some(w) = width {
        bytes.truncate(w);
        bytes.resize(w, b' ');
        return bytes;
    }
    cobolt_wire::node::pad_text(kind, &bytes)
}

fn display_path(anc: &[String], decl: &DataDecl, subs: &[i64]) -> String {
    let mut p = anc.join(".");
    if !p.is_empty() {
        p.push('.');
    }
    p.push_str(decl.name.as_deref().unwrap_or("FILLER"));
    if !subs.is_empty() {
        p.push_str(&format!("({})", subs.iter().map(|i| i.to_string()).collect::<Vec<_>>().join(",")));
    }
    p
}

/// Write the leaves that differ between `old` and `new` into the environment.
fn write_node(
    env: &mut CobolEnvironment,
    decl: &DataDecl,
    item: &Item,
    old: &Node,
    new: &Node,
    anc: &mut Vec<String>,
    subs: &mut Vec<i64>,
) {
    if old == new {
        return;
    }
    match (&item.occurs, old, new) {
        (Some(_), Node::Table(o), Node::Table(n)) => {
            for (i, (oe, ne)) in o.iter().zip(n.iter()).enumerate() {
                subs.push(i as i64 + 1);
                write_base(env, decl, item, oe, ne, anc, subs);
                subs.pop();
            }
        }
        _ => write_base(env, decl, item, old, new, anc, subs),
    }
}

fn write_base(
    env: &mut CobolEnvironment,
    decl: &DataDecl,
    item: &Item,
    old: &Node,
    new: &Node,
    anc: &mut Vec<String>,
    subs: &mut Vec<i64>,
) {
    if old == new {
        return;
    }
    match (&item.kind, old, new) {
        (Kind::Group { children }, Node::Group(o), Node::Group(n)) => {
            let decls = data_children(decl);
            anc.push(decl.name.clone().unwrap_or_default().to_ascii_uppercase());
            for (((c, d), oc), nc) in children.iter().zip(decls.iter()).zip(o.iter()).zip(n.iter()) {
                if c.is_member() {
                    write_node(env, d, c, oc, nc, anc, subs);
                }
            }
            anc.pop();
        }
        (kind, _, Node::Leaf(bytes)) => {
            let key = key_of(env, decl, anc, subs);
            match kind {
                Kind::Numeric { scale, .. } => {
                    let text = String::from_utf8_lossy(bytes);
                    if let Some(m) = number::to_mantissa(&text) {
                        env.set(&key, CobolValue::Numeric(CobolNumeric::new(m, *scale as u8)));
                    }
                }
                Kind::National { .. } | Kind::Utf8 { .. } => {
                    // The slot of a national or UTF-8 item holds its text.
                    let text = String::from_utf8_lossy(bytes).into_owned();
                    env.set(&key, CobolValue::String { bytes: text.as_bytes().to_vec(), capacity: text.len() });
                }
                _ => env.set_verbatim_bytes(&key, bytes),
            }
        }
        _ => {}
    }
}

/// The root name of an item's table key, used by tests.
#[allow(dead_code)]
pub(crate) fn root_of(key: &str) -> &str {
    base_name(key)
}
