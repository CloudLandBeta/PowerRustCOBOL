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

/// What AWS's `aws___run_script` sandbox refuses in a script, as it reported it on
/// 2026-10-17 to a connection with valid credentials: "Blocked import: base64",
/// "Blocked function: getattr()", "Blocked attribute access: __name__". One such name
/// anywhere refuses the WHOLE script, so every operation that pastes the prelude failed.
/// `None` when `code` keeps to the rules, else the first offence and its line.
fn sandbox_offence(code: &str) -> Option<String> {
    for (n, line) in code.lines().enumerate() {
        let t = line.trim();
        if t.starts_with('#') {
            continue;
        }
        let at = |what: &str| Some(format!("line {}: {what}: {t}", n + 1));
        if let Some(rest) = t.strip_prefix("import ").or_else(|| t.strip_prefix("from ")) {
            let modules: Vec<&str> = rest.split([',', ' ']).filter(|m| !m.is_empty()).collect();
            if t.starts_with("from ") || modules.iter().any(|m| *m != "json") {
                return at("an import other than json");
            }
        }
        for banned in ["getattr(", "setattr(", "delattr(", "eval(", "exec(", "compile(", "globals(", "locals(", "vars(", "open(", "__import__"] {
            if t.contains(banned) {
                return at(banned);
            }
        }
        // A dunder attribute: `.__name__`, `__class__`… (the tool's own name, written in
        // no script, is the only `__` that is not one).
        if t.split("aws___run_script").any(|piece| piece.contains("__")) {
            return at("a __dunder__");
        }
    }
    None
}

/// Every script the table sends to the hosted server, its snippets pasted, keeps to the
/// sandbox's rules — and the rules are seen to bite on the scripts as they were.
#[test]
fn every_hosted_script_keeps_to_the_sandboxs_rules() {
    let routes = Routes::shipped();
    let mut checked = 0;
    for (name, op) in &routes.ops {
        if op.tool != "aws___run_script" {
            continue;
        }
        let input = op.input.as_ref().unwrap_or_else(|| panic!("{name}: a script operation has an input"));
        let pasted = routes.paste(input, &|_| Some("TOKEN".to_owned())).unwrap_or_else(|e| panic!("{name}: {e}"));
        let code = pasted.get("code").and_then(toml::Value::as_str).unwrap_or_else(|| panic!("{name}: no code"));
        if let Some(offence) = sandbox_offence(code) {
            panic!("{name} would be refused by AWS's sandbox — {offence}");
        }
        checked += 1;
    }
    assert!(checked >= 30, "only {checked} hosted scripts found");
    // The guard guards: the three offences AWS reported, each alone, are caught.
    for (bad, what) in [
        ("import json, base64\nresult = 1", "import"),
        ("d = getattr(x, 'response', None)", "getattr("),
        ("c = type(x).__name__", "__dunder__"),
    ] {
        let found = sandbox_offence(bad).unwrap_or_else(|| panic!("{bad:?} passed the guard"));
        assert!(found.contains(what), "{found}");
    }
    println!("{checked} hosted scripts keep to the sandbox's rules (json only, no getattr, no dunders)");
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

/// Every route's completion event is one its control declares: otherwise the
/// designer cannot bind it, and a handler written for it is never generated.
#[test]
fn every_routed_event_is_one_its_control_raises() {
    let mut missing = Vec::new();
    for (key, op) in &Routes::shipped().ops {
        let control = key.split('.').next().unwrap_or_default();
        let declared = cobolt_forms::aws_catalog::by_name(control).map_or(&[][..], |c| c.events);
        if !declared.contains(&op.event.as_str()) {
            missing.push(format!("{key} raises {}", op.event));
        }
    }
    assert!(missing.is_empty(), "not in the control's aws_catalog events: {missing:?}");
    println!("{} routed operations, every event declared by its control", Routes::shipped().ops.len());
}

/// Every routed method is in the inline-call vocabulary: otherwise
/// `AWS-1::Query(…)` parses its parentheses as a subscript and calls nothing.
#[test]
fn every_routed_method_can_be_called_inline() {
    let missing: Vec<&String> = Routes::shipped()
        .ops
        .keys()
        .filter(|k| !k.split('.').nth(1).is_some_and(cobolt_ast::methods::is_known_method))
        .collect();
    assert!(missing.is_empty(), "not in is_known_method: {missing:?}");
    println!("{} routed methods, every one callable inline", Routes::shipped().ops.len());
}
