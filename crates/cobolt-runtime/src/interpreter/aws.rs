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
            AsyncOutcome::AwsError { message } => self.aws_fail_event(obj, "onError", message, verbose),
            AsyncOutcome::AwsTimeout { message } => self.aws_fail_event(obj, "onTimeout", message, verbose),
            // Not an AWS outcome; never routed here.
            _ => {}
        }
    }

    fn aws_fail_event(&mut self, obj: &str, event: &str, message: String, verbose: bool) {
        if verbose {
            self.log_block("aws", obj, "error", &aws_mask(&message));
        }
        self.aws_raise(obj, event, vec![("LastError".into(), message), ("Busy".into(), "0".into())], None);
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
        if self.obj_get(obj, "Verbose").trim().eq_ignore_ascii_case("true") {
            self.log_block("aws", obj, "refused", &aws_mask(&message));
        }
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
            return Some(match ops::execute(&prepared, budget) {
                ops::Outcome::Done { props, rows, .. } => {
                    let body = props.iter().find(|(k, _)| k == "ResponseBody").map(|(_, v)| v.clone()).unwrap_or_default();
                    for (k, v) in props {
                        self.obj_set(obj, &k, v);
                    }
                    self.obj_set(obj, "LastError", String::new());
                    self.aws_rows.insert(obj.trim().to_ascii_uppercase(), rows);
                    body
                }
                ops::Outcome::Error(message) | ops::Outcome::Timeout(message) => {
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
                // for a first `uvx` download, then `TimeoutMs` — and it
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
