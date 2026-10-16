// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Test connection (spec 078 R22–R24, AC13, AC14; spec 103 R15, AC10).
//!
//! A connection reaches AWS in two ways, and each is tested on its own:
//!
//! - [`diagnose_hosted`] — the **hosted AWS MCP Server**, which needs nothing
//!   installed: it looks for the sign-in, opens a session at the endpoint,
//!   lists the tools and checks the route table against them. It reports
//!   whether the endpoint could not be reached, nothing is signed in (or the
//!   sign-in has expired), the endpoint refused the session, the tools do not
//!   match the route table, or all is well.
//! - [`diagnose`] — the **AWS Labs servers** (Lambda and the others), which are
//!   programs started with `uvx`: the program is missing, the profile is not
//!   signed in, the server does not match the route table, or all is well (with
//!   the Lambda functions it found).
//!
//! The IDE shows each outcome in its own language; the words for an end user,
//! which a control's `LastError` carries, come from the same checks.

use std::time::Duration;

use cobolt_mcp::{Mode, Tool};

use super::connections::AwsConnection;
use super::ops;
use super::pool::{self, Budget, CallError, ServerKey};
use super::routes::{self, Context, Protocol, Routes};

/// Which kind of server a report is about.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Server {
    /// The hosted AWS MCP Server, reached over HTTPS.
    Hosted,
    /// The AWS Labs servers, started as programs.
    Labs,
}

/// What Test connection found.
#[derive(Debug, Clone, PartialEq)]
pub enum Report {
    /// The program the server is started with is not installed; `message`
    /// names it and the fix.
    ProgramMissing { program: String, message: String },
    /// The AWS profile is missing, expired or not signed in.
    ProfileNotSignedIn { profile: String, message: String },
    /// The server started, but what it offers does not match the route table
    /// (or the server could not be reached for another reason, said plainly).
    RouteMismatch { detail: String },
    /// Everything works; the Lambda functions the connection reaches.
    AllGood { functions: Vec<String> },
    /// The hosted endpoint could not be reached (no network, a wrong address,
    /// a TLS failure) or did not answer in time.
    EndpointUnreachable { host: String, detail: String },
    /// The hosted endpoint answered, and refused the session: the sign-in is
    /// not allowed to use the service (`status` is the HTTP status).
    SessionRefused { status: String, detail: String },
    /// The hosted server works; how many tools it offers.
    HostedGood { tools: usize },
}

/// The Labs server Test connection starts: the one Lambda's operations use.
const DIAGNOSED_SERVER: &str = "lambda";

/// The route table's id of the hosted server.
const HOSTED_SERVER: &str = "hosted";

/// Test `conn`'s Labs servers: start the Lambda one, list its tools, check the routes.
///
/// `start` bounds the first start (which may download the server) and `call`
/// the listing.
pub fn diagnose(conn: &AwsConnection, start: Duration, call: Duration) -> Report {
    diagnose_server(conn, DIAGNOSED_SERVER, start, call)
}

/// Test `conn`'s hosted server: find the sign-in, open a session, list the
/// tools, check the routes (spec 103 R15). Nothing is installed or started.
pub fn diagnose_hosted(conn: &AwsConnection, start: Duration, call: Duration) -> Report {
    diagnose_server(conn, HOSTED_SERVER, start, call)
}

fn diagnose_server(conn: &AwsConnection, server_id: &str, start: Duration, call: Duration) -> Report {
    let table = match Routes::shipped().with_override(&conn.routes_override) {
        Ok(t) => t,
        Err(detail) => return Report::RouteMismatch { detail },
    };
    let Some(server) = table.servers.get(server_id) else {
        return Report::RouteMismatch { detail: format!("the route table has no \"{server_id}\" server") };
    };
    let args: [String; 0] = [];
    let no_prop = |_: &str| String::new();
    let conn_field = |f: &str| conn.field(f);
    let ctx = Context { args: &args, prop: &no_prop, connection: &conn_field };
    // Test connection only reads: the server starts read-only.
    let launch = match routes::launch_for(server, &ctx, false, &conn.profile, &conn.region) {
        Ok(l) => l,
        Err(e) => return Report::RouteMismatch { detail: e.to_string() },
    };
    let host = launch.https.as_ref().map(|h| host_of(&h.url));
    let key = ServerKey { connection: format!("diagnose:{}", conn.id), server: server_id.into() };
    let mode = if server.protocol == Protocol::Stateless { Mode::Stateless } else { Mode::Handshake };
    let result = pool::list_tools(&key, &launch, mode, Budget { call, start });
    let stderr = pool::stderr_of(&key).unwrap_or_default();
    // A diagnosis leaves nothing running behind it.
    pool::stop(&key);
    let tools = match result {
        Ok(tools) => tools,
        Err(CallError::Start(message))
            if host.is_none() && message == super::process::missing_program_message(&launch.command) =>
        {
            return Report::ProgramMissing { program: launch.command.clone(), message };
        }
        Err(CallError::Start(message)) | Err(CallError::Failed(message)) => {
            return classify_failure(&format!("{message}\n{stderr}"), &conn.profile, host.as_deref());
        }
        Err(CallError::Timeout) => {
            return classify_failure(
                &format!("The AWS server did not answer within {} seconds.\n{stderr}", call.as_secs()),
                &conn.profile,
                host.as_deref(),
            )
        }
    };
    if let Err(detail) = check_routes(&table.ops, server_id, &tools) {
        return Report::RouteMismatch { detail };
    }
    if host.is_some() {
        Report::HostedGood { tools: tools.len() }
    } else {
        Report::AllGood { functions: tools.into_iter().map(|t| t.name).collect() }
    }
}

/// Every operation routed to `server_id` finds its tool, with the inputs it fills.
fn check_routes(table_ops: &std::collections::BTreeMap<String, routes::OpDef>, server_id: &str, tools: &[Tool]) -> Result<(), String> {
    for (name, op) in table_ops {
        if op.server == server_id {
            routes::check_route(name, op, tools)?;
        }
    }
    Ok(())
}

/// The host of an address, for a message: `aws.example` of `https://aws.example/mcp`.
fn host_of(url: &str) -> String {
    let rest = url.split_once("://").map_or(url, |(_, r)| r);
    rest.split(['/', '?', '#']).next().unwrap_or(rest).to_owned()
}

/// A failure the server reported: a credential problem names the profile and
/// the fix; an endpoint that cannot be reached, or that refuses the session,
/// says so; anything else is said plainly, credentials masked.
fn classify_failure(raw: &str, profile: &str, host: Option<&str>) -> Report {
    let message = ops::plain_error(raw, profile);
    if message.starts_with("The AWS profile") {
        let profile = if profile.trim().is_empty() { "default" } else { profile.trim() };
        return Report::ProfileNotSignedIn { profile: profile.to_owned(), message };
    }
    let first = message.lines().next().unwrap_or_default().to_owned();
    if let Some(host) = host {
        // After the first line of a hosted failure: what the transport said.
        if raw.contains("could not be reached") || raw.contains("did not answer within") {
            let detail = first.split_once(": ").map_or(first.as_str(), |(_, d)| d).to_owned();
            return Report::EndpointUnreachable { host: host.to_owned(), detail };
        }
        if let Some(status) = raw.split("HTTP ").nth(1).and_then(|s| s.split_whitespace().next()).filter(|s| s.len() == 3) {
            let detail = first.split_once("AWS service: ").map_or("", |(_, d)| d).chars().take(160).collect();
            return Report::SessionRefused { status: status.to_owned(), detail };
        }
    }
    Report::RouteMismatch { detail: first }
}
