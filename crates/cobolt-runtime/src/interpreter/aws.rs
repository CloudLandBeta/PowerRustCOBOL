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
    /// drain; `obj` is the control's id as the operation was started.
    pub(crate) fn aws_delivered(&mut self, obj: &str, outcome: AsyncOutcome) {
        let verbose = self.obj_get(obj, "Verbose").trim().eq_ignore_ascii_case("true");
        match outcome {
            AsyncOutcome::Aws { mut props, rows, event } => {
                if verbose {
                    let body = props.iter().find(|(k, _)| k == "ResponseBody").map(|(_, v)| v.clone());
                    self.log_block("aws", obj, "response", &aws_mask(&body.unwrap_or_default()));
                }
                props.push(("LastError".into(), String::new()));
                props.push(("Busy".into(), "0".into()));
                self.aws_raise(obj, &event, props, Some(rows));
                let spelled = self.form_spelling(obj);
                self.async_dispatch_queue.push_back((spelled, "onComplete".to_string()));
            }
            AsyncOutcome::AwsError { message } => self.aws_fail_event(obj, "onError", "error", message),
            AsyncOutcome::AwsTimeout { message } => self.aws_fail_event(obj, "onTimeout", "timed out", message),
            // Not an AWS outcome; never routed here.
            _ => {}
        }
    }

    fn aws_fail_event(&mut self, obj: &str, event: &str, label: &str, message: String) {
        self.aws_log_failure(obj, label, &message);
        self.aws_raise(obj, event, vec![("LastError".into(), message), ("Busy".into(), "0".into())], None);
    }

    /// **A failed AWS operation is never silent** (operator, 2026-10-18: "PowerCloud failed
    /// to upload the photo … there is no visual log of the operation, so I cannot tell what
    /// went wrong"). Whatever `Verbose` says, the reason goes to wherever this program's
    /// DISPLAY goes — the IDE's Output panel under Run Form — as well as into `LastError`,
    /// where a program that shows it in a one-line label cuts it short. Masked like every
    /// AWS narration: a key, a secret or a token in the message never reaches the output.
    fn aws_log_failure(&mut self, obj: &str, label: &str, message: &str) {
        self.log_block("aws", obj, label, &aws_mask(message));
    }

    /// The last operation's row `i` (1-based): the row as JSON, or a field of
    /// it. Empty when there is no such row or field.
    fn aws_row_value(&self, obj: &str, i: &str, field: Option<&str>) -> String {
        let Some(row) = i
            .trim()
            .parse::<usize>()
            .ok()
            .filter(|n| *n >= 1)
            .and_then(|n| self.aws_rows.get(&obj.trim().to_ascii_uppercase())?.get(n - 1))
        else {
            return String::new();
        };
        let Some(field) = field else {
            return row.to_string();
        };
        let field = field.trim();
        let value = row.get(field).or_else(|| {
            row.as_object()?.iter().find(|(k, _)| k.eq_ignore_ascii_case(field)).map(|(_, v)| v)
        });
        match value {
            Some(Value::String(s)) => s.clone(),
            Some(Value::Null) | None => String::new(),
            Some(v) => v.to_string(),
        }
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

/// Mask what `Verbose` prints (R27). Without the feature there is nothing to
/// print.
#[cfg(feature = "aws")]
fn aws_mask(text: &str) -> String {
    crate::aws::process::mask(text)
}
#[cfg(not(feature = "aws"))]
fn aws_mask(text: &str) -> String {
    text.to_string()
}

#[cfg(feature = "aws")]
impl Interpreter {
    /// The route-table spelling of `m` on this control's class, when the
    /// table routes it.
    fn aws_route_method(&self, class: &str, m: &str, override_toml: &str) -> Option<String> {
        let table = crate::aws::routes::Routes::shipped().with_override(override_toml).ok()?;
        let prefix = format!("{class}.");
        table
            .ops
            .keys()
            .filter_map(|k| k.strip_prefix(&prefix))
            .find(|name| name.eq_ignore_ascii_case(m))
            .map(str::to_string)
    }

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

    /// Report a failure the way the control's `Mode` asks: a synchronous
    /// call answers empty with `LastError` set; an asynchronous one raises
    /// `onError` (R21, R25).
    fn aws_refuse(&mut self, obj: &str, sync: bool, message: String) -> String {
        self.aws_log_failure(obj, "refused", &message);
        if sync {
            self.obj_set(obj, "LastError", message);
            String::new()
        } else {
            self.aws_raise(obj, "onError", vec![("LastError".into(), message)], None);
            "0".into()
        }
    }

    /// A method called on an AWS control. `None` when the name is not one of
    /// its own, so the general dispatch handles it.
    pub(crate) fn aws_method(&mut self, obj: &str, m: &str, args: &[crate::value::CobolValue]) -> Option<String> {
        use crate::aws::{connections, ops, pool::Budget};
        use std::sync::atomic::{AtomicU64, Ordering};
        use std::sync::Arc;

        let arg = |i: usize| args.get(i).map(|v| v.as_display_string().trim().to_string()).unwrap_or_default();
        match m {
            "GETROW" => return Some(self.aws_row_value(obj, &arg(0), None)),
            "GETFIELD" => return Some(self.aws_row_value(obj, &arg(0), Some(&arg(1)))),
            _ => {}
        }
        let class = self.object_class(obj.trim())?;
        let conn_name = self.obj_get(obj, "Connection");
        let conn = connections::find(&conn_name);
        let override_toml = conn.as_ref().map(|c| c.routes_override.clone()).unwrap_or_default();
        let method = self.aws_route_method(&class, m, &override_toml)?;
        let sync = self.obj_get(obj, "Mode").trim().eq_ignore_ascii_case("sync");

        if !sync && self.async_pending.contains_key(obj) {
            return Some(self.aws_refuse(obj, sync, "another AWS operation is still running on this control".into()));
        }
        let Some(conn) = conn else {
            let known = connections::names();
            let message = if conn_name.trim().is_empty() {
                format!("this control names no AWS connection, and the project has {} — set Connection", known.len())
            } else {
                format!("no AWS connection named \"{}\" (the project has: {})", conn_name.trim(), known.join(", "))
            };
            return Some(self.aws_refuse(obj, sync, message));
        };

        // An argument not passed is empty; the route table says what stands
        // in for it — the property the designer set (`|or:{prop:…}`), the
        // empty object (`|json:{}`), or nothing (`|opt`).
        let texts: Vec<String> = args.iter().map(|v| v.as_display_string().trim().to_string()).collect();

        let allow_write = self.obj_get(obj, "AllowWrite").trim().eq_ignore_ascii_case("true");
        let connection_allows_write = allow_write || self.aws_connection_allows_write(&conn.id);
        let prepared = {
            let prop = |p: &str| self.obj_get(obj, p);
            let view = ops::ControlView {
                control_type: &class,
                method: &method,
                args: &texts,
                prop: &prop,
                control: obj,
                allow_write,
                connection_allows_write,
            };
            ops::prepare(&view, &conn)
        };
        let prepared = match prepared {
            Ok(p) => p,
            Err(message) => return Some(self.aws_refuse(obj, sync, message)),
        };
        if self.obj_get(obj, "Verbose").trim().eq_ignore_ascii_case("true") {
            let what = match &prepared.action {
                // A password or a token travels in this request.
                ops::Action::Call { tool, .. } if prepared.sensitive => format!("{tool} (arguments not shown: they carry a password or a token)"),
                ops::Action::Call { tool, input } => format!("{tool} {input}"),
                ops::Action::ListTools => "tools/list".into(),
            };
            self.log_block("aws", obj, "request", &aws_mask(&what));
        }
        let ms = |p: &str, default: u64| self.obj_get(obj, p).trim().parse::<u64>().unwrap_or(default);
        let budget = Budget {
            call: std::time::Duration::from_millis(ms("TimeoutMs", 30_000).max(1)),
            start: std::time::Duration::from_millis(ms("StartTimeoutMs", 120_000).max(1)),
        };

        if sync {
            // A Sync call is narrated as an Async one is: its answer under `Verbose`, and a
            // failure always — the Sync branch narrated nothing, so a failed `PutObject` in a
            // Sync form left no trace but `LastError`.
            let verbose = self.obj_get(obj, "Verbose").trim().eq_ignore_ascii_case("true");
            return Some(match ops::execute(&prepared, budget) {
                ops::Outcome::Done { props, rows, .. } => {
                    let body = props.iter().find(|(k, _)| k == "ResponseBody").map(|(_, v)| v.clone()).unwrap_or_default();
                    if verbose {
                        self.log_block("aws", obj, "response", &aws_mask(&body));
                    }
                    for (k, v) in props {
                        self.obj_set(obj, &k, v);
                    }
                    self.obj_set(obj, "LastError", String::new());
                    self.aws_rows.insert(obj.trim().to_ascii_uppercase(), rows);
                    body
                }
                ops::Outcome::Error(message) => {
                    self.aws_log_failure(obj, &format!("{method} failed"), &message);
                    self.obj_set(obj, "LastError", message);
                    String::new()
                }
                ops::Outcome::Timeout(message) => {
                    self.aws_log_failure(obj, &format!("{method} timed out"), &message);
                    self.obj_set(obj, "LastError", message);
                    String::new()
                }
            });
        }

        let generation = self
            .async_generations
            .entry(obj.to_string())
            .or_insert_with(|| Arc::new(AtomicU64::new(0)))
            .fetch_add(1, Ordering::Relaxed)
            + 1;
        self.obj_set(obj, "Busy", "1".into());
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
            let outcome = match ops::execute(&prepared, budget) {
                ops::Outcome::Done { props, rows, event } => AsyncOutcome::Aws { props, rows, event },
                ops::Outcome::Error(message) => AsyncOutcome::AwsError { message },
                ops::Outcome::Timeout(message) => AsyncOutcome::AwsTimeout { message },
            };
            let _ = tx.send(crate::async_op::AsyncOpResult { ctrl_id, generation, outcome });
        });
        Some("1".into())
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
