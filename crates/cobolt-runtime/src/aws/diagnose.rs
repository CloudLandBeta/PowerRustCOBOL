// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Test connection (spec 078 R22–R24, AC13, AC14).
//!
//! [`diagnose`] starts a connection's AWS server the way a control would,
//! lists what it offers, checks the route table against it, and reports one
//! of four outcomes — the program the server needs is missing, the AWS
//! profile is not signed in, the server does not match the route table, or
//! all is well (with the Lambda functions it found). The IDE shows the
//! outcome in its own language; the words for an end user, which a control's
//! `LastError` carries, come from the same checks.

use std::time::Duration;

use cobolt_mcp::Mode;

use super::connections::AwsConnection;
use super::ops;
use super::pool::{self, Budget, CallError, ServerKey};
use super::routes::{self, Context, Protocol, Routes};

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
}

/// The server Test connection starts: the one Lambda's operations use.
const DIAGNOSED_SERVER: &str = "lambda";

/// Test `conn`: start its server, list its tools, check the routes.
///
/// `start` bounds the first start (which may download the server) and `call`
/// the listing.
pub fn diagnose(conn: &AwsConnection, start: Duration, call: Duration) -> Report {
    let table = match Routes::shipped().with_override(&conn.routes_override) {
        Ok(t) => t,
        Err(detail) => return Report::RouteMismatch { detail },
    };
    let Some(server) = table.servers.get(DIAGNOSED_SERVER) else {
        return Report::RouteMismatch { detail: format!("the route table has no \"{DIAGNOSED_SERVER}\" server") };
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
    let key = ServerKey { connection: format!("diagnose:{}", conn.id), server: DIAGNOSED_SERVER.into() };
    let mode = if server.protocol == Protocol::Stateless { Mode::Stateless } else { Mode::Handshake };
    let result = pool::list_tools(&key, &launch, mode, Budget { call, start });
    let stderr = pool::stderr_of(&key).unwrap_or_default();
    // A diagnosis leaves nothing running behind it.
    pool::stop(&key);
    let tools = match result {
        Ok(tools) => tools,
        Err(CallError::Start(message)) if message == super::process::missing_program_message(&launch.command) => {
            return Report::ProgramMissing { program: launch.command.clone(), message };
        }
        Err(CallError::Start(message)) | Err(CallError::Failed(message)) => {
            return classify_failure(&format!("{message}\n{stderr}"), &conn.profile);
        }
        Err(CallError::Timeout) => {
            return classify_failure(
                &format!("The AWS server did not answer within {} seconds.\n{stderr}", call.as_secs()),
                &conn.profile,
            )
        }
    };
    for (name, op) in &table.ops {
        if op.server != DIAGNOSED_SERVER {
            continue;
        }
        if let Err(detail) = routes::check_route(name, op, &tools) {
            return Report::RouteMismatch { detail };
        }
    }
    Report::AllGood { functions: tools.into_iter().map(|t| t.name).collect() }
}

/// A failure the server reported: a credential problem names the profile and
/// the fix; anything else is said plainly, credentials masked.
fn classify_failure(raw: &str, profile: &str) -> Report {
    let message = ops::plain_error(raw, profile);
    if message.starts_with("The AWS profile") {
        let profile = if profile.trim().is_empty() { "default" } else { profile.trim() };
        return Report::ProfileNotSignedIn { profile: profile.to_owned(), message };
    }
    Report::RouteMismatch { detail: message.lines().next().unwrap_or_default().to_owned() }
}
