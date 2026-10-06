// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Spec 078 AC9 (R17): the shipped route table checked against each routed
//! server's recorded `tools/list`, and the check shown to FAIL when a fixture
//! drifts — so a renamed tool fails the build, not the end user.

#![cfg(feature = "aws")]

use cobolt_mcp::Tool;
use cobolt_runtime::aws::routes::{check_route, OpDef, Routes};
use serde_json::Value;

fn fixture(server: &str) -> Option<Vec<Tool>> {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(format!("tests/fixtures/aws-mcp/{server}.tools.json"));
    let text = std::fs::read_to_string(path).ok()?;
    let v: Value = serde_json::from_str(&text).unwrap();
    assert!(v.get("_provenance").and_then(Value::as_str).is_some_and(|p| !p.is_empty()), "{server}: a fixture states where it came from");
    Some(serde_json::from_value(v["tools"].clone()).unwrap())
}

#[test]
fn every_route_names_a_real_tool_with_its_required_inputs() {
    let routes = Routes::shipped();
    let mut checked = 0;
    for (name, op) in &routes.ops {
        if op.server.starts_with('{') {
            continue; // AwsMcp: the control names its own server
        }
        let tools = fixture(&op.server).unwrap_or_else(|| panic!("{name}: no recorded tools/list for server {}", op.server));
        check_route(name, op, &tools).unwrap_or_else(|e| panic!("{e}"));
        checked += 1;
    }
    assert!(checked > 0);
    println!("AC9: {checked} routed operations checked against their recorded servers");
}

/// The check fails on drift: a renamed required input, and a renamed tool.
#[test]
fn a_drifted_fixture_fails_the_route_check() {
    let routes = Routes::shipped();
    let invoke = routes.op("AwsLambda", "Invoke").unwrap();
    let mut tools = fixture("lambda").unwrap();
    for t in &mut tools {
        t.input_schema["required"] = serde_json::json!(["payload"]);
    }
    let e = check_route("AwsLambda.Invoke", invoke, &tools).unwrap_err();
    assert!(e.contains("payload"), "{e}");

    // A route naming a fixed tool, against a fixture where it was renamed.
    let fixed: OpDef = toml::from_str("server = \"lambda\"\ntool = \"order_sync\"\nmutating = false\nevent = \"onInvoked\"\ninput = { parameters = \"{arg:1|json}\" }").unwrap();
    let ok = fixture("lambda").unwrap();
    assert!(check_route("t", &fixed, &ok).is_ok());
    let mut renamed = ok.clone();
    renamed[0].name = "order_sync_v2".into();
    let e = check_route("t", &fixed, &renamed).unwrap_err();
    assert!(e.contains("no tool named order_sync"), "{e}");
    println!("AC9 drift: a renamed input and a renamed tool both fail the check");
}
