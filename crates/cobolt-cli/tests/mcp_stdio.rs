// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! `rcrun mcp` end to end, through the real binary (spec 080 AC6a): the same
//! tool set and the same `check` answer as the in-process tools, every stdout
//! line a JSON-RPC message, and no network port.

use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use cobolt_mcp::McpHandler;
use cobolt_project_tools::{HeadlessHost, ProjectTools};
use serde_json::{json, Value};

const FIXTURE: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../cobolt-project-tools/tests/fixtures/check_project"
);

fn copy_dir(src: &Path, dst: &Path) {
    std::fs::create_dir_all(dst).unwrap();
    for e in std::fs::read_dir(src).unwrap().flatten() {
        let to = dst.join(e.file_name());
        if e.path().is_dir() {
            copy_dir(&e.path(), &to);
        } else {
            std::fs::copy(e.path(), to).unwrap();
        }
    }
}

fn fixture(name: &str) -> PathBuf {
    let dst = std::env::temp_dir().join(format!("prc-080-cli-mcp-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dst);
    copy_dir(Path::new(FIXTURE), &dst);
    dst
}

/// Spawn `rcrun mcp --project <p>`, send `requests` one per line, read one
/// reply per request that has an id; then (with the process still alive) run
/// `probe` on its pid, and close stdin. Returns the replies, every raw stdout
/// line, and the probe's result.
fn session<T>(project: &Path, requests: &[Value], probe: impl FnOnce(u32) -> T) -> (Vec<Value>, Vec<String>, T) {
    let mut child = Command::new(env!("CARGO_BIN_EXE_rcrun"))
        .args(["mcp", "--project"])
        .arg(project)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("rcrun starts");
    let mut stdin = child.stdin.take().unwrap();
    let mut stdout = BufReader::new(child.stdout.take().unwrap());
    let mut replies = Vec::new();
    let mut raw = Vec::new();
    for r in requests {
        writeln!(stdin, "{r}").unwrap();
        stdin.flush().unwrap();
        if r.get("id").is_none() {
            continue;
        }
        let mut line = String::new();
        stdout.read_line(&mut line).unwrap();
        raw.push(line.trim_end().to_owned());
        replies.push(serde_json::from_str(line.trim()).unwrap_or(Value::Null));
    }
    let probed = probe(child.id());
    drop(stdin);
    let mut rest = String::new();
    while stdout.read_line(&mut rest).unwrap_or(0) > 0 {
        raw.push(std::mem::take(&mut rest).trim_end().to_owned());
    }
    assert!(child.wait().unwrap().success(), "rcrun mcp exits cleanly at end of input");
    (replies, raw, probed)
}

/// Inet sockets the process holds, by `lsof`; `None` when `lsof` cannot run.
fn inet_sockets(pid: u32) -> Option<String> {
    let out = Command::new("lsof")
        .args(["-a", "-i", "-p", &pid.to_string()])
        .output()
        .ok()?;
    // lsof exits 1 when it lists nothing.
    Some(String::from_utf8_lossy(&out.stdout).into_owned())
}

#[test]
fn rcrun_mcp_serves_the_same_tools_and_answers_over_stdio() {
    let dir = fixture("main");
    let requests = [
        json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18","clientInfo":{"name":"t","version":"1"}}}),
        json!({"jsonrpc":"2.0","method":"notifications/initialized"}),
        json!({"jsonrpc":"2.0","id":2,"method":"tools/list"}),
        json!({"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"check","arguments":{}}}),
    ];
    let (replies, raw, sockets) = session(&dir, &requests, |pid| inet_sockets(pid));

    // Every stdout line is a JSON-RPC message.
    for line in &raw {
        let v: Value = serde_json::from_str(line).unwrap_or_else(|e| panic!("not JSON-RPC: {line:?} ({e})"));
        assert_eq!(v["jsonrpc"], "2.0", "{line}");
    }
    assert_eq!(replies.len(), 3);
    assert_eq!(replies[0]["result"]["protocolVersion"], "2025-06-18");

    // The same tool set — names and schemas — as the in-process tools.
    let mut local = ProjectTools::new(HeadlessHost::new(&dir, "x"));
    let expected = serde_json::to_value(local.list_tools()).unwrap();
    assert_eq!(replies[1]["result"]["tools"], expected);

    // The same check answer.
    let in_process = local.call_tool("check", &json!({}));
    let remote_text = replies[2]["result"]["content"][0]["text"].as_str().unwrap();
    let local_text = in_process.content[0].as_text().expect("a text answer").to_owned();
    assert_eq!(remote_text, local_text, "rcrun mcp check = in-process check");
    let check: Value = serde_json::from_str(remote_text).unwrap();
    assert!(check["errors"].as_u64().unwrap() >= 1);
    assert!(remote_text.contains("\"file\":\"forms/MAIN-FORM.cfrm\"") && remote_text.contains("\"line\":4"));

    // No network port.
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut listeners = 0;
    for e in std::fs::read_dir(&src).unwrap().flatten() {
        if std::fs::read_to_string(e.path()).unwrap_or_default().contains("TcpListener") {
            listeners += 1;
        }
    }
    assert_eq!(listeners, 0, "rcrun's own source opens no listener");
    match sockets {
        Some(listing) => {
            assert!(listing.trim().is_empty(), "rcrun mcp holds an inet socket:\n{listing}");
            println!("rcrun mcp: lsof -a -i -p <pid> lists no inet socket");
        }
        None => println!("rcrun mcp: inet-socket probe skipped — lsof is not available here"),
    }
    println!(
        "rcrun mcp: {} stdout lines, all JSON-RPC; tools/list = in-process ({} tools); \
         check = in-process ({} error(s)); 0 TcpListener in cobolt-cli/src",
        raw.len(),
        expected.as_array().unwrap().len(),
        check["errors"]
    );
}

#[test]
fn rcrun_mcp_with_an_unreadable_manifest_answers_no_project_on_every_tool() {
    let dir = fixture("unreadable");
    std::fs::write(dir.join("CheckDemo.project.toml"), "not = [valid").unwrap();
    let names = ["list_files", "check", "regenerate", "add_to_project", "build", "validate", "kb_lookup"];
    let requests: Vec<Value> = names
        .iter()
        .enumerate()
        .map(|(i, n)| {
            json!({"jsonrpc":"2.0","id":i+1,"method":"tools/call",
                   "params":{"name":n,"arguments":{"path":"forms/MAIN-FORM.cfrm","name":"Button"}}})
        })
        .collect();
    let (replies, _, _) = session(&dir.join("CheckDemo.project.toml"), &requests, |_| ());
    for (n, r) in names.iter().zip(&replies) {
        assert_eq!(r["result"]["isError"], true, "{n}: {r}");
        assert_eq!(r["result"]["content"][0]["text"], "no project open", "{n}");
    }
    println!("rcrun mcp: unreadable manifest → {}/7 tools answered \"no project open\"", replies.len());
}

mod product {
    include!("../../cobolt-ide/src/version.rs");
}

/// Spec 084 AC5 + AC7 over stdio: `initialize` carries the standing rules and
/// the PRODUCT version; `resources/list` lists the reference pack and
/// `resources/read` returns the very text embedded in the binary.
#[test]
fn rcrun_mcp_serves_instructions_and_the_reference_resources() {
    let project = fixture("resources");
    let requests = [
        json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18"}}),
        json!({"jsonrpc":"2.0","id":2,"method":"resources/list"}),
        json!({"jsonrpc":"2.0","id":3,"method":"resources/read","params":{"uri":"powerrustcobol://reference/developers-guide.md"}}),
        json!({"jsonrpc":"2.0","id":4,"method":"resources/read","params":{"uri":"powerrustcobol://reference/nope.md"}}),
    ];
    let (replies, _, _) = session(&project, &requests, |_| ());
    let init = &replies[0]["result"];
    assert_eq!(init["serverInfo"]["version"], product::VERSION, "rcrun reports the product version");
    assert!(init["capabilities"]["resources"].is_object(), "resources advertised: {init}");
    let instructions = init["instructions"].as_str().expect("instructions sent");
    assert!(instructions.contains(product::VERSION));
    let rules = cobolt_project_tools::content::rules();
    for rule in &rules {
        assert!(instructions.contains(&rule.text), "rule {} missing from the instructions", rule.id);
    }
    let listed = replies[1]["result"]["resources"].as_array().expect("a resource list");
    let names: Vec<&str> = listed.iter().filter_map(|r| r["name"].as_str()).collect();
    for want in ["README.md", "developers-guide.md", "cobol85-supported-syntax.md", "controls.md", "builtins.md", "cfrm-format.md", "cidx-format.md"] {
        assert!(names.contains(&want), "{want} not listed: {names:?}");
    }
    assert_eq!(
        replies[2]["result"]["contents"][0]["text"].as_str(),
        Some(cobolt_project_tools::reference::DEVELOPERS_GUIDE),
        "the guide is served byte for byte"
    );
    assert_eq!(replies[3]["error"]["code"], -32002, "an unknown resource is MCP's not-found");
    let _ = std::fs::remove_dir_all(project);
    println!(
        "rcrun mcp: version {}, {} rules in the instructions, {} resources listed, guide read ({} bytes), unknown URI -> -32002",
        product::VERSION,
        rules.len(),
        names.len(),
        cobolt_project_tools::reference::DEVELOPERS_GUIDE.len()
    );
}
