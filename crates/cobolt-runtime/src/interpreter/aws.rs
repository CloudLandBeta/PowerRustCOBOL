// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! The AWS controls' methods and events (spec 078 R19–R21, R25–R27).
//!
//! Methods are routed here **by the object's class**, before the general
//! dispatch — `Call`, `Invoke`, `Get` and `Query` mean something else on other
//! controls. Which methods a control has is not written here: it is the route
//! table's (`aws/routes.toml`), so a method is an AWS operation exactly when
//! the table routes `<ControlType>.<Method>`.
//!
//! `Cancel` and `IsBusy` fall through to the uniform async handling every
//! asynchronous control shares.
//!
//! **Events carry their own values**, as the KnowledgeBase's do: the async
//! drain applies every delivered result before any handler runs, so each
//! event queues its property values and rows beside it, and they are written
//! when that event is dispatched.
//!
//! What a call decides and what a result writes is [`crate::aws::door`]'s,
//! which the desktop runner calls too (DESKTOP-WASM-001 T48): this file only
//! carries it out on the interpreter's objects.

use serde_json::Value;

use super::Interpreter;
use crate::async_op::AsyncOutcome;

/// The values one AWS event carries, applied when it is dispatched.
#[derive(Debug, Clone, Default)]
pub(crate) struct AwsPayload {
    /// The control as the program named it (the key its properties use).
    pub obj: String,
    /// The event these values belong to.
    pub event: String,
    pub props: Vec<(String, String)>,
    /// A finished operation's row set, which `GetRow` / `GetField` read.
    pub rows: Option<Vec<Value>>,
}

impl Interpreter {
    pub(crate) fn is_aws(&self, obj: &str) -> bool {
        self.object_class(obj.trim()).is_some_and(|c| c.starts_with("Aws"))
    }

    /// Queue one AWS event with the values it carries.
    fn aws_raise(&mut self, obj: &str, event: &str, props: Vec<(String, String)>, rows: Option<Vec<Value>>) {
        let spelled = self.form_spelling(obj);
        self.aws_payloads
            .entry(spelled.to_ascii_uppercase())
            .or_default()
            .push_back(AwsPayload { obj: obj.to_string(), event: event.to_string(), props, rows });
        self.async_dispatch_queue.push_back((spelled, event.to_string()));
    }

    /// Called as an event leaves the queue: write the values its AWS event
    /// carried. An event that queued none — `onComplete`, `onCancelled` — is
    /// left alone, and so is a payload that belongs to a later event.
    pub(crate) fn aws_apply_payload(&mut self, ctrl: &str, event: &str) {
        let Some(queue) = self.aws_payloads.get_mut(&ctrl.to_ascii_uppercase()) else {
            return;
        };
        if queue.front().is_none_or(|p| p.event != event) {
            return;
        }
        let Some(payload) = queue.pop_front() else { return };
        for (prop, value) in payload.props {
            self.obj_set(&payload.obj, &prop, value);
        }
        if let Some(rows) = payload.rows {
            self.aws_rows.insert(payload.obj.trim().to_ascii_uppercase(), rows);
        }
    }

    /// An AWS worker reported. Runs on the interpreter thread from the async
    /// drain; `obj` is the control's id as the operation was started. A
    /// success raises the operation's own event with its values, then
    /// `onComplete`; a failure raises `onError` or `onTimeout`, and is
    /// narrated whatever `Verbose` says ([`crate::aws::door::delivered`]).
    #[cfg(feature = "aws")]
    pub(crate) fn aws_delivered(&mut self, obj: &str, outcome: AsyncOutcome) {
        use crate::aws::{door, ops};
        let outcome = match outcome {
            AsyncOutcome::Aws { props, rows, event } => ops::Outcome::Done { props, rows, event },
            AsyncOutcome::AwsError { message } => ops::Outcome::Error(message),
            AsyncOutcome::AwsTimeout { message } => ops::Outcome::Timeout(message),
            // Not an AWS outcome; never routed here.
            _ => return,
        };
        let verbose = self.obj_get(obj, "Verbose").trim().eq_ignore_ascii_case("true");
        self.aws_apply(obj, door::delivered(verbose, outcome));
    }

    /// A build without AWS never starts an operation, so nothing is ever delivered.
    #[cfg(not(feature = "aws"))]
    pub(crate) fn aws_delivered(&mut self, _obj: &str, _outcome: AsyncOutcome) {}

    /// The last operation's row `i` (1-based): the row as JSON, or a field of
    /// it. Empty when there is no such row or field.
    fn aws_row_value(&self, obj: &str, i: &str, field: Option<&str>) -> String {
        crate::aws_rows::row_value(self.aws_rows.get(&obj.trim().to_ascii_uppercase()), i, field)
    }
}

// ── Keys typed in at run time (spec 092 A6) ─────────────────────────────────
//
// The three built-ins' bodies. They log nothing and return no key: `status` is
// `OK` or why nothing was stored, the flag is Y or N.

/// `COBOL::"AWS-SET-CREDENTIALS"`: the status.
#[cfg(feature = "aws")]
pub(super) fn set_credentials(connection: &str, key_id: String, secret: String, token: String) -> String {
    match crate::aws::credentials::set(connection, key_id, secret, token) {
        Ok(()) => "OK".to_string(),
        Err(why) => why,
    }
}

/// `COBOL::"AWS-CLEAR-CREDENTIALS"`: the status.
#[cfg(feature = "aws")]
pub(super) fn clear_credentials(connection: &str) -> String {
    match crate::aws::credentials::clear(connection) {
        Ok(()) => "OK".to_string(),
        Err(why) => why,
    }
}

/// `COBOL::"AWS-CREDENTIALS-SET"`: whether the connection holds keys.
#[cfg(feature = "aws")]
pub(super) fn credentials_set(connection: &str) -> bool {
    crate::aws::credentials::is_set(connection)
}

#[cfg(not(feature = "aws"))]
const NO_AWS: &str = "this application was built without AWS support: nothing was stored";

#[cfg(not(feature = "aws"))]
pub(super) fn set_credentials(_connection: &str, _key_id: String, _secret: String, _token: String) -> String {
    NO_AWS.to_string()
}

#[cfg(not(feature = "aws"))]
pub(super) fn clear_credentials(_connection: &str) -> String {
    NO_AWS.to_string()
}

#[cfg(not(feature = "aws"))]
pub(super) fn credentials_set(_connection: &str) -> bool {
    false
}

#[cfg(feature = "aws")]
impl Interpreter {
    /// Whether any AWS control of this form, on the same connection, allows
    /// writes — which decides whether the shared server starts read-only
    /// (amendment A4).
    fn aws_connection_allows_write(&self, conn_id: &str) -> bool {
        self.objects.iter().any(|(id, o)| {
            o.class.starts_with("Aws")
                && crate::aws::connections::find(&self.obj_get(id, "Connection")).is_some_and(|c| c.id == conn_id)
                && self.obj_get(id, "AllowWrite").trim().eq_ignore_ascii_case("true")
        })
    }

    /// Carry out what a call or a result asks ([`crate::aws::door::Effects`], in its order) on
    /// the control `obj`, and answer what the method answers.
    fn aws_apply(&mut self, obj: &str, fx: crate::aws::door::Effects) -> String {
        for note in &fx.notes {
            for line in crate::aws::door::note_lines(obj, note) {
                self.agent_log(line);
            }
        }
        for (k, v) in fx.props {
            self.obj_set(obj, &k, v);
        }
        if let Some(rows) = fx.rows {
            self.aws_rows.insert(obj.trim().to_ascii_uppercase(), rows);
        }
        for e in fx.events {
            if e.props.is_empty() && e.rows.is_none() {
                // An event that carries no values (`onComplete`) is only presented.
                let spelled = self.form_spelling(obj);
                self.async_dispatch_queue.push_back((spelled, e.name));
            } else {
                self.aws_raise(obj, &e.name, e.props, e.rows);
            }
        }
        fx.answer
    }

    /// A method called on an AWS control. `None` when the name is not one of
    /// its own, so the general dispatch handles it. What it decides is
    /// [`crate::aws::door`]'s, which the desktop runner calls too.
    pub(crate) fn aws_method(&mut self, obj: &str, m: &str, args: &[crate::value::CobolValue]) -> Option<String> {
        use crate::aws::{door, ops};
        use std::sync::atomic::{AtomicU64, Ordering};
        use std::sync::Arc;

        let arg = |i: usize| args.get(i).map(|v| v.as_display_string().trim().to_string()).unwrap_or_default();
        match m {
            "GETROW" => return Some(self.aws_row_value(obj, &arg(0), None)),
            "GETFIELD" => return Some(self.aws_row_value(obj, &arg(0), Some(&arg(1)))),
            _ => {}
        }
        let class = self.object_class(obj.trim())?;
        let texts: Vec<String> = args.iter().map(|v| v.as_display_string()).collect();
        let start = {
            let prop = |p: &str| self.obj_get(obj, p);
            let any_allows_write = |id: &str| self.aws_connection_allows_write(id);
            let control = door::Control { class: &class, name: obj, prop: &prop, any_allows_write: &any_allows_write, running: self.async_pending.contains_key(obj) };
            door::start(&control, m, &texts)
        };
        let run = match start {
            door::Start::NotOwn => return None,
            door::Start::Refused { sync, message } => return Some(self.aws_apply(obj, door::refused(sync, message))),
            door::Start::Run(run) => run,
        };
        if let Some(request) = &run.request {
            for line in door::note_lines(obj, request) {
                self.agent_log(line);
            }
        }
        if run.sync {
            let outcome = ops::execute(&run.prepared, run.budget);
            return Some(self.aws_apply(obj, door::finished(&run.method, run.verbose, outcome)));
        }

        let generation = self
            .async_generations
            .entry(obj.to_string())
            .or_insert_with(|| Arc::new(AtomicU64::new(0)))
            .fetch_add(1, Ordering::Relaxed)
            + 1;
        let answer = self.aws_apply(obj, door::started());
        self.async_pending.insert(
            obj.to_string(),
            crate::async_op::PendingOp {
                generation,
                started_at: std::time::Instant::now(),
                // The worker's own budgets end every call — the start budget
                // for a first start (a `uvx` download, or the hosted session),
                // then `TimeoutMs` — and it
                // reports `onTimeout` itself; an interpreter-side limit would
                // cut a first start short.
                timeout_ms: 0,
                awaiting_start: None,
            },
        );
        let tx = self.async_result_tx.clone();
        let ctrl_id = obj.to_string();
        std::thread::spawn(move || {
            let outcome = match ops::execute(&run.prepared, run.budget) {
                ops::Outcome::Done { props, rows, event } => AsyncOutcome::Aws { props, rows, event },
                ops::Outcome::Error(message) => AsyncOutcome::AwsError { message },
                ops::Outcome::Timeout(message) => AsyncOutcome::AwsTimeout { message },
            };
            let _ = tx.send(crate::async_op::AsyncOpResult { ctrl_id, generation, outcome });
        });
        Some(answer)
    }
}

#[cfg(not(feature = "aws"))]
impl Interpreter {
    /// This build left AWS out — which a build never does when a form holds
    /// an AWS control (R29). Every operation answers `onError` and says why,
    /// instead of falling through to another control's method of the same
    /// name.
    pub(crate) fn aws_method(&mut self, obj: &str, m: &str, args: &[crate::value::CobolValue]) -> Option<String> {
        let arg = |i: usize| args.get(i).map(|v| v.as_display_string().trim().to_string()).unwrap_or_default();
        match m {
            "GETROW" => Some(self.aws_row_value(obj, &arg(0), None)),
            "GETFIELD" => Some(self.aws_row_value(obj, &arg(0), Some(&arg(1)))),
            "CANCEL" | "ISBUSY" => None,
            _ => {
                let message = "this application was built without AWS support".to_string();
                self.aws_raise(obj, "onError", vec![("LastError".into(), message)], None);
                Some("0".into())
            }
        }
    }
}

/// **A failed AWS operation is never silent** (operator, 2026-10-18: PowerCloud's photo
/// upload failed and "there is no visual log of the operation, so I cannot tell what went
/// wrong"). Its reason reaches the program's output — the IDE's Output panel under Run
/// Form — with `Verbose` off, Sync or Async, and masked.
#[cfg(all(test, feature = "aws"))]
mod failure_log_tests {
    use super::*;
    use crate::aws::connections::{self, AwsConnection};
    use cobolt_lexer::{tokenize, SourceFormat};
    use cobolt_parser::parse;

    const KEY_ID: &str = "AKIAIOSFODNN7EXAMPLE";

    /// An interpreter holding one Sync `AwsS3`, `S3-1`, on `connection`, whose program
    /// output is collected.
    fn s3(connection: &str) -> (Interpreter, std::sync::mpsc::Receiver<String>) {
        let source = "IDENTIFICATION DIVISION.\nPROGRAM-ID. T.\nPROCEDURE DIVISION.\n    STOP RUN.\n";
        let program = parse(tokenize(source, SourceFormat::Free)).program.expect("program should parse");
        let mut i = Interpreter::new(program);
        let props = vec![
            ("Connection".to_string(), connection.to_string()),
            ("Mode".to_string(), "Sync".to_string()),
            ("Bucket".to_string(), "powercloud-photos".to_string()),
            ("AllowWrite".to_string(), "true".to_string()),
            ("TimeoutMs".to_string(), "3000".to_string()),
            ("StartTimeoutMs".to_string(), "3000".to_string()),
            ("Verbose".to_string(), "false".to_string()),
        ];
        i.seed_objects(vec![("S3-1".to_string(), "AwsS3".to_string(), props)]);
        let (tx, rx) = std::sync::mpsc::channel();
        i.display_tx = Some(tx);
        (i, rx)
    }

    fn put(i: &mut Interpreter) {
        let args: Vec<crate::value::CobolValue> = ["photos/1.jpg", "hello", ""].iter().map(|a| crate::value::CobolValue::from_str(a, a.len().max(1))).collect();
        let _ = i.aws_method("S3-1", "PutObject", &args);
    }

    /// A Sync operation refused before it is sent: the reason is in the output with
    /// `Verbose` off.
    #[test]
    fn a_refused_operation_is_reported_with_verbose_off() {
        let (mut i, rx) = s3("No-Such-Connection");
        put(&mut i);
        let log: Vec<String> = rx.try_iter().collect();
        assert!(!i.obj_get("S3-1", "LastError").is_empty());
        assert!(
            log.iter().any(|l| l.starts_with("[aws S3-1] refused:") && l.contains("No-Such-Connection")),
            "{log:?}"
        );
    }

    /// A Sync `PutObject` that is sent and fails — here, to a server that is not there:
    /// "PutObject failed" and the reason, in the output.
    #[test]
    fn a_sync_put_that_fails_is_reported() {
        let conn = AwsConnection {
            id: "conn-failure-log".into(),
            name: "conn-failure-log".into(),
            profile: "test".into(),
            region: "eu-west-1".into(),
            routes_override: "[servers.hosted]\ntransport = \"https\"\nurl = \"http://127.0.0.1:9/mcp\"\nservice = \"aws-mcp\"\nsigning_region = \"us-east-1\"\nprotocol = \"handshake\"\n".into(),
            ..Default::default()
        };
        connections::publish(vec![conn]);
        let (mut i, rx) = s3("conn-failure-log");
        put(&mut i);
        let log: Vec<String> = rx.try_iter().collect();
        let error = i.obj_get("S3-1", "LastError");
        assert!(!error.is_empty(), "the operation failed");
        assert!(
            log.iter().any(|l| l.starts_with("[aws S3-1] PutObject failed") || l.starts_with("[aws S3-1] PutObject timed out") || l.starts_with("[aws S3-1] refused:")),
            "a line names the failure: {log:?}"
        );
        println!("Sync PutObject to a missing server: LastError {error:?}; output {log:?}");
    }

    /// An Async error is reported, and a key id inside AWS's message is masked.
    #[test]
    fn an_async_error_is_reported_and_masked() {
        let (mut i, rx) = s3("No-Such-Connection");
        i.obj_set("S3-1", "Mode", "Async".into());
        let message = format!("An error occurred (AccessDenied) when calling the PutObject operation: key {KEY_ID} is not allowed");
        i.aws_delivered("S3-1", AsyncOutcome::AwsError { message });
        let log = rx.try_iter().collect::<Vec<_>>().join("\n");
        assert!(log.contains("[aws S3-1] error: An error occurred (AccessDenied)"), "{log}");
        assert!(!log.contains(KEY_ID), "the key id never reaches the output: {log}");
    }
}
