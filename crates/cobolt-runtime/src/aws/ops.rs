// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! One AWS control operation, in two steps (spec 078 R19–R21, R25, R26).
//!
//! - [`prepare`] runs on the interpreter thread, synchronously: it resolves
//!   the route, expands the COBOL arguments into the tool's input, and REFUSES
//!   — an invalid JSON argument (R21), a write while `AllowWrite` is off (R25)
//!   — before anything is sent anywhere.
//! - [`execute`] runs the prepared call on a worker thread through the server
//!   pool and turns the answer into properties, rows and the control's own
//!   completion event.
//!
//! Nothing here names a service: the route table does (R15).

use std::collections::BTreeMap;

use cobolt_mcp::Mode;
use serde_json::{json, Value};

use super::connections::AwsConnection;
use super::pool::{self, Budget, CallError, ServerKey};
use super::process::{mask, Launch};
use super::routes::{self, Context, ExpandError, Mutating, Protocol, Routes};

/// What a prepared operation does.
#[derive(Debug, Clone, PartialEq)]
pub enum Action {
    Call { tool: String, input: Value },
    ListTools,
}

/// An operation ready to run.
#[derive(Debug, Clone)]
pub struct Prepared {
    pub key: ServerKey,
    pub launch: Launch,
    pub mode: Mode,
    pub action: Action,
    pub result: BTreeMap<String, String>,
    pub event: String,
    /// `AwsMcp.Call`: the tool's own read-only hint decides (R26).
    pub check_tool_hint: bool,
    pub allow_write: bool,
}

/// The outcome of [`execute`].
#[derive(Debug, Clone, PartialEq)]
pub enum Outcome {
    Done { props: Vec<(String, String)>, rows: Vec<Value>, event: String },
    Error(String),
    Timeout(String),
}

/// What [`prepare`] needs to know about the control.
pub struct ControlView<'a> {
    pub control_type: &'a str,
    pub method: &'a str,
    pub args: &'a [String],
    pub prop: &'a dyn Fn(&str) -> String,
    /// This control's `AllowWrite`.
    pub allow_write: bool,
    /// Whether ANY control on this connection allows writes — which decides
    /// whether the shared server starts read-only (amendment A4).
    pub connection_allows_write: bool,
}

/// Resolve, expand and check one operation (R21, R25) — before anything is
/// sent.
pub fn prepare(view: &ControlView, conn: &AwsConnection) -> Result<Prepared, String> {
    let table = Routes::shipped().with_override(&conn.routes_override)?;
    let op = table.op(view.control_type, view.method).ok_or_else(|| {
        format!("{}.{} has no route in the AWS route table", view.control_type, view.method)
    })?;
    if op.mutating == Mutating::Yes && !view.allow_write {
        return Err(format!(
            "{} changes AWS and AllowWrite is off: nothing was sent. Turn AllowWrite on for this control to allow it.",
            view.method
        ));
    }
    let conn_field = |f: &str| conn.field(f);
    let ctx = Context { args: view.args, prop: view.prop, connection: &conn_field };
    let expand_err = |e: ExpandError| match e {
        ExpandError::InvalidJson { arg, message } => {
            format!("argument {arg} of {} is not valid JSON ({message}): nothing was sent", view.method)
        }
        other => other.to_string(),
    };
    let server_id = routes::expand(&op.server, &ctx).map_err(expand_err)?;
    let server = table
        .servers
        .get(&server_id)
        .ok_or_else(|| format!("no AWS server \"{server_id}\" in the route table"))?;
    let launch = routes::launch_for(server, &ctx, view.connection_allows_write, &conn.profile, &conn.region)
        .map_err(expand_err)?;
    let action = if op.tool == "$list" {
        Action::ListTools
    } else {
        let tool = routes::expand(&op.tool, &ctx).map_err(expand_err)?;
        if tool.trim().is_empty() {
            return Err(format!("{}: no tool or function was named", view.method));
        }
        let input = match &op.input {
            Some(t) => routes::expand_input(t, &ctx).map_err(expand_err)?,
            None => json!({}),
        };
        Action::Call { tool, input }
    };
    Ok(Prepared {
        key: ServerKey { connection: conn.id.clone(), server: server_id },
        launch,
        mode: if server.protocol == Protocol::Stateless { Mode::Stateless } else { Mode::Handshake },
        action,
        result: op.result.clone(),
        event: op.event.clone(),
        check_tool_hint: op.mutating == Mutating::ByTool,
        allow_write: view.allow_write,
    })
}

/// The credential-chain failures an AWS server reports, in its own words.
const CREDENTIAL_FAILURES: [&str; 6] = [
    "ExpiredToken",
    "NoCredentialProviders",
    "Unable to locate credentials",
    "The SSO session",
    "Token has expired",
    "InvalidClientTokenId",
];

/// A server's raw failure in plain words for an end user (R23): a credential
/// problem names the profile and the fix, and never shows the server's text.
pub fn plain_error(raw: &str, profile: &str) -> String {
    if CREDENTIAL_FAILURES.iter().any(|f| raw.contains(f)) {
        let p = if profile.trim().is_empty() { "default" } else { profile.trim() };
        return format!("The AWS profile \"{p}\" is not signed in or has expired. Run: aws login --profile {p}");
    }
    mask(raw.trim())
}

/// Rows from an answer that is a JSON array, or holds one.
fn rows_of(v: &Value) -> Vec<Value> {
    match v {
        Value::Array(a) => a.clone(),
        _ => Vec::new(),
    }
}

/// Run a prepared operation through the pool.
pub fn execute(p: &Prepared, budget: Budget) -> Outcome {
    let profile = p.launch.profile.clone().unwrap_or_default();
    let fail = |e: CallError| match e {
        CallError::Timeout => Outcome::Timeout(format!(
            "No answer within {} ms: the AWS call was abandoned.",
            budget.call.as_millis()
        )),
        CallError::Start(m) => Outcome::Error(m),
        CallError::Failed(m) => {
            let stderr = pool::stderr_of(&p.key).unwrap_or_default();
            Outcome::Error(plain_error(&format!("{m}\n{stderr}"), &profile).lines().next().unwrap_or_default().to_owned())
        }
    };
    match &p.action {
        Action::ListTools => match pool::list_tools(&p.key, &p.launch, p.mode, budget) {
            Ok(tools) => {
                let rows: Vec<Value> = tools
                    .iter()
                    .map(|t| json!({"Name": t.name, "Description": t.description.clone().unwrap_or_default(), "ReadOnly": if t.is_read_only() { "1" } else { "0" }}))
                    .collect();
                let names: Vec<&str> = tools.iter().map(|t| t.name.as_str()).collect();
                Outcome::Done {
                    props: vec![
                        ("ResponseBody".into(), names.join("\n")),
                        ("ResultJson".into(), Value::Array(rows.clone()).to_string()),
                        ("RowCount".into(), rows.len().to_string()),
                    ],
                    rows,
                    event: p.event.clone(),
                }
            }
            Err(e) => fail(e),
        },
        Action::Call { tool, input } => {
            if p.check_tool_hint && !p.allow_write {
                let tools = match pool::list_tools(&p.key, &p.launch, p.mode, budget) {
                    Ok(t) => t,
                    Err(e) => return fail(e),
                };
                let read_only = tools.iter().find(|t| &t.name == tool).is_some_and(|t| t.is_read_only());
                if !read_only {
                    return Outcome::Error(format!(
                        "the tool \"{tool}\" is not marked read-only and AllowWrite is off: nothing was sent. Turn AllowWrite on for this control to allow it."
                    ));
                }
            }
            match pool::call_tool(&p.key, &p.launch, p.mode, tool, input.clone(), budget) {
                Ok(r) if r.is_error == Some(true) => {
                    let text = routes::result_text(&r);
                    Outcome::Error(plain_error(&text, &profile))
                }
                Ok(r) => {
                    let mut props: Vec<(String, String)> =
                        p.result.iter().map(|(k, expr)| (k.clone(), routes::extract(&r, expr))).collect();
                    // The whole answer as JSON, when the route did not map it.
                    let body = props.iter().find(|(k, _)| k == "ResponseBody").map(|(_, v)| v.clone());
                    let json_text = props
                        .iter()
                        .find(|(k, _)| k == "ResultJson")
                        .map(|(_, v)| v.clone())
                        .or_else(|| body.clone().filter(|b| serde_json::from_str::<Value>(b.trim()).is_ok()))
                        .unwrap_or_default();
                    let rows = serde_json::from_str::<Value>(json_text.trim()).map(|v| rows_of(&v)).unwrap_or_default();
                    props.retain(|(k, _)| k != "ResultJson");
                    props.push(("ResultJson".into(), json_text));
                    props.push(("RowCount".into(), rows.len().to_string()));
                    Outcome::Done { props, rows, event: p.event.clone() }
                }
                Err(e) => fail(e),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_credential_failure_names_the_profile_and_the_fix() {
        let raw = "botocore.exceptions.NoCredentialProviders: Unable to locate credentials AKIAABCDEFGHIJKLMNOP";
        let m = plain_error(raw, "sales");
        assert_eq!(m, "The AWS profile \"sales\" is not signed in or has expired. Run: aws login --profile sales");
        assert!(!plain_error("boom AKIAABCDEFGHIJKLMNOP", "x").contains("EFGHIJKLMNOP"));
    }

    fn view<'a>(ty: &'a str, method: &'a str, args: &'a [String], prop: &'a dyn Fn(&str) -> String, allow: bool) -> ControlView<'a> {
        ControlView { control_type: ty, method, args, prop, allow_write: allow, connection_allows_write: allow }
    }

    #[test]
    fn prepare_refuses_before_sending() {
        let conn = AwsConnection { id: "c1".into(), name: "dev".into(), profile: "dev".into(), region: "eu-west-1".into(), ..Default::default() };
        let none = |_: &str| String::new();
        let args = vec!["app-f".to_string(), "{\"id\":1}".to_string()];
        let e = prepare(&view("AwsLambda", "Invoke", &args, &none, false), &conn).unwrap_err();
        assert!(e.contains("AllowWrite is off"), "{e}");
        let bad = vec!["app-f".to_string(), "{nope".to_string()];
        let e = prepare(&view("AwsLambda", "Invoke", &bad, &none, true), &conn).unwrap_err();
        assert!(e.contains("argument 2") && e.contains("not valid JSON"), "{e}");
        let p = prepare(&view("AwsLambda", "Invoke", &args, &none, true), &conn).unwrap();
        assert_eq!(p.action, Action::Call { tool: "app_f".into(), input: json!({"parameters": {"id": 1}}) });
        assert_eq!(p.event, "onInvoked");
        assert_eq!(p.launch.profile.as_deref(), Some("dev"));
    }
}
