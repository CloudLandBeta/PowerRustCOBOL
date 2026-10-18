// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Test connection's four outcomes (spec 078 T-A13; AC13, AC14), against the
//! fake MCP server standing in for AWS's Lambda server.

#![cfg(feature = "aws")]

use std::path::PathBuf;
use std::time::Duration;

use cobolt_runtime::aws::connections::AwsConnection;
use cobolt_runtime::aws::diagnose::{diagnose, Report};
use serde_json::{json, Value};

struct Fake {
    dir: PathBuf,
    conn: AwsConnection,
}

impl Drop for Fake {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

/// A connection whose `lambda` server is `command` run with `script`.
fn connection(tag: &str, command: &str, script: Value) -> Fake {
    connection_on("lambda", tag, command, script)
}

/// A connection whose `server` is `command` run with `script`.
fn connection_on(server: &str, tag: &str, command: &str, script: Value) -> Fake {
    let dir = std::env::temp_dir().join(format!("prc-078-diag-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("script.json");
    std::fs::write(&path, script.to_string()).unwrap();
    let conn = AwsConnection {
        id: format!("diag-{tag}"),
        name: format!("diag-{tag}"),
        profile: "sales-prod".into(),
        region: "eu-west-1".into(),
        routes_override: format!("[servers.{server}]\ncommand = '{command}'\nargs = ['{}']\n", path.display()),
        ..Default::default()
    };
    Fake { dir, conn }
}

fn fake() -> &'static str {
    env!("CARGO_BIN_EXE_fake_mcp")
}

fn run(f: &Fake) -> Report {
    diagnose(&f.conn, Duration::from_secs(10), Duration::from_secs(5))
}

/// The Lambda server's per-function tool: its input is `parameters`.
fn lambda_tool(name: &str) -> Value {
    json!({"name": name, "description": "a function", "inputSchema": {
        "type": "object", "properties": {"parameters": {"type": "object"}}, "required": ["parameters"]}})
}

#[test]
fn path_without_uvx_reports_the_fix() {
    let f = connection("missing", "prc-surely-not-installed-uvx", json!({}));
    let r = run(&f);
    println!("  program missing -> {r:?}");
    let Report::ProgramMissing { program, message } = r else { panic!("{r:?}") };
    assert_eq!(program, "prc-surely-not-installed-uvx");
    assert!(message.contains("not installed"), "{message}");
}

/// AC13 — the server dies on a credential error: the report names the
/// profile and `aws login`, never the server's own text.
#[test]
fn a_rejected_profile_reports_aws_login() {
    let f = connection(
        "profile",
        fake(),
        json!({"exit_on_start": "botocore.exceptions.NoCredentialProviders: Unable to locate credentials"}),
    );
    let r = run(&f);
    println!("  profile not signed in -> {r:?}");
    let Report::ProfileNotSignedIn { profile, message } = r else { panic!("{r:?}") };
    assert_eq!(profile, "sales-prod");
    assert!(message.contains("aws login --profile sales-prod"), "{message}");
    assert!(!message.contains("botocore"), "the server's raw text is not shown: {message}");
}

/// AC14 — the four outcomes, the two remaining here: all good (with the
/// functions found) and a server that does not match the route table.
#[test]
fn diagnose_reports_each_outcome() {
    let good = connection("good", fake(), json!({"tools": [lambda_tool("sales_report"), lambda_tool("sales_close")]}));
    let r = run(&good);
    println!("  all good -> {r:?}");
    assert_eq!(r, Report::AllGood { functions: vec!["sales_report".into(), "sales_close".into()] });

    // Since 1.91.210 the routes are on the hosted server: one whose run_script tool
    // takes something else than `code` does not match them.
    let drifted = connection_on(
        "hosted",
        "drift",
        fake(),
        json!({"tools": [{"name": "aws___run_script", "inputSchema": {"type": "object", "properties": {"script": {"type": "string"}}, "required": ["script"]}}]}),
    );
    let r = cobolt_runtime::aws::diagnose::diagnose_hosted(&drifted.conn, Duration::from_secs(10), Duration::from_secs(5));
    println!("  route mismatch -> {r:?}");
    assert!(matches!(r, Report::RouteMismatch { .. }), "{r:?}");
}

/// Operator, 2026-10-18: no example needs uvx. No control operation reaches an AWS
/// Labs server, so Test connection does not start one.
#[test]
fn no_control_operation_needs_an_aws_labs_server() {
    let plain = AwsConnection { id: "labs-none".into(), name: "labs-none".into(), ..Default::default() };
    assert!(!cobolt_runtime::aws::diagnose::labs_in_use(&plain), "an operation still reaches a program started with uvx");
    // A connection that points the hosted server at a program for a test is one.
    let local = connection_on("hosted", "labs-local", fake(), json!({}));
    assert!(cobolt_runtime::aws::diagnose::labs_in_use(&local.conn));
}
