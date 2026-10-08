// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Spec 078 T-C12 (AC20): the live smoke test — one READ-ONLY call per AWS
//! service, through the shipped route table and the real servers, with the
//! operator's own profile. Skipped unless asked for:
//!
//! ```text
//! COBOLT_AWS_LIVE=1 COBOLT_AWS_PROFILE=dev COBOLT_AWS_REGION=eu-west-1 \
//!   cargo test -p cobolt-runtime --features aws --test aws_live -- --ignored --nocapture
//! ```
//!
//! Optional: `COBOLT_AWS_FUNCTION_PREFIX` (Lambda), `COBOLT_AWS_S3_BUCKET`,
//! `COBOLT_AWS_DYNAMODB_TABLE`. A service whose input is not given is
//! listed as skipped.
//!
//! It prints a table of what answered and how fast, and saves each server's
//! real `tools/list` under `target/aws-live-tools/` — the input for
//! re-recording the fixtures in `tests/fixtures/aws-mcp/`, which until then
//! are derived from source and documentation (their provenance says so).

#![cfg(feature = "aws")]

use std::time::{Duration, Instant};

use cobolt_mcp::Mode;
use cobolt_runtime::aws::connections::AwsConnection;
use cobolt_runtime::aws::ops::{self, ControlView, Outcome};
use cobolt_runtime::aws::pool::{self, Budget, ServerKey};
use cobolt_runtime::aws::routes::{launch_for, Context, Routes};

fn env(k: &str) -> String {
    std::env::var(k).unwrap_or_default()
}

#[test]
#[ignore = "calls AWS with the operator's profile; run with COBOLT_AWS_LIVE=1 and --ignored"]
fn one_read_only_call_per_service() {
    if env("COBOLT_AWS_LIVE") != "1" {
        eprintln!("COBOLT_AWS_LIVE is not 1: nothing was called");
        return;
    }
    let conn = AwsConnection {
        id: "live".into(),
        name: "live".into(),
        profile: env("COBOLT_AWS_PROFILE"),
        region: env("COBOLT_AWS_REGION"),
        function_prefix: env("COBOLT_AWS_FUNCTION_PREFIX"),
        ..Default::default()
    };
    let budget = Budget { call: Duration::from_secs(60), start: Duration::from_secs(300) };
    let bucket = env("COBOLT_AWS_S3_BUCKET");
    let table = env("COBOLT_AWS_DYNAMODB_TABLE");
    let speech = std::env::temp_dir().join("prc-078-live.mp3");
    let speech = speech.to_string_lossy().into_owned();
    // (control, method, args, properties, needs)
    let cases: Vec<(&str, &str, Vec<String>, Vec<(&str, String)>, bool)> = vec![
        ("AwsLambda", "ListFunctions", vec![], vec![], true),
        ("AwsKnowledgeBase", "ListKnowledgeBases", vec![], vec![], true),
        ("AwsS3Tables", "ListTables", vec![], vec![], true),
        ("AwsComprehend", "DetectLanguage", vec!["Bom dia, tudo bem?".into()], vec![], true),
        ("AwsEC2", "Describe", vec![], vec![], true),
        ("AwsPolly", "Synthesize", vec!["Hello from PowerRustCOBOL".into(), String::new(), String::new(), speech.clone()], vec![], true),
        ("AwsS3", "List", vec![String::new()], vec![("Bucket", bucket.clone())], !bucket.is_empty()),
        ("AwsDynamoDB", "Scan", vec!["5".into()], vec![("TableName", table.clone())], !table.is_empty()),
    ];
    let mut rows = Vec::new();
    for (control, method, args, props, run) in cases {
        if !run {
            rows.push(format!("{control}.{method:<20} skipped (no input given)"));
            continue;
        }
        let prop = |p: &str| props.iter().find(|(k, _)| *k == p).map(|(_, v)| v.clone()).unwrap_or_default();
        let view = ControlView { control_type: control, method, args: &args, prop: &prop, control: "LIVE-1", allow_write: false, connection_allows_write: false };
        let t = Instant::now();
        let outcome = match ops::prepare(&view, &conn) {
            Ok(p) => ops::execute(&p, budget),
            Err(e) => Outcome::Error(format!("refused: {e}")),
        };
        let ms = t.elapsed().as_millis();
        rows.push(match outcome {
            Outcome::Done { rows: r, event, .. } => format!("{control}.{method:<20} {ms:>6} ms  {event}, {} rows", r.len()),
            Outcome::Error(e) => format!("{control}.{method:<20} {ms:>6} ms  ERROR {e}"),
            Outcome::Timeout(e) => format!("{control}.{method:<20} {ms:>6} ms  TIMEOUT {e}"),
        });
    }

    // Each server's real tools/list, for re-recording the fixtures.
    let out = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/aws-live-tools");
    let _ = std::fs::create_dir_all(&out);
    let routes = Routes::shipped();
    for id in routes.servers.keys() {
        let none = |_: &str| String::new();
        let field = |f: &str| conn.field(f);
        let ctx = Context { args: &[], prop: &none, connection: &field };
        let launch = launch_for(&routes.servers[id], &ctx, false, &conn.profile, &conn.region).unwrap();
        let key = ServerKey { connection: conn.id.clone(), server: id.clone() };
        let mode = if routes.servers[id].protocol == cobolt_runtime::aws::routes::Protocol::Stateless { Mode::Stateless } else { Mode::Handshake };
        match pool::list_tools(&key, &launch, mode, budget) {
            Ok(tools) => {
                let file = out.join(format!("{id}.tools.json"));
                let body = serde_json::json!({"_provenance": format!("LIVE tools/list of the shipped `{id}` server, recorded by aws_live.rs"), "tools": tools});
                std::fs::write(&file, serde_json::to_string_pretty(&body).unwrap()).unwrap();
                rows.push(format!("tools/list {id:<16} {} tools → {}", tools.len(), file.display()));
            }
            Err(e) => rows.push(format!("tools/list {id:<16} ERROR {e:?}")),
        }
        pool::stop(&key);
    }
    eprintln!("\n  ══ AWS live smoke test (spec 078 AC20) — profile {:?}, region {:?} ══", conn.profile, conn.region);
    for r in &rows {
        eprintln!("    {r}");
    }
}
