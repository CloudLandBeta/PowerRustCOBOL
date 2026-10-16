// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Spec 103 — the hosted AWS MCP Server over HTTPS, signed with SigV4, against
//! a loopback fake of the endpoint (AC1–AC5, AC10).
//!
//! No AWS account and no network: the fake (`support/http_fake.rs`) writes every
//! request down, and the signatures are checked by `support/sigv4.rs`, which is
//! the algorithm written out independently of the AWS crates the runtime signs
//! with. Each test prints a summary of what it measured (run with `--nocapture`).
//!
//! The credentials here are AWS's documented examples, not anyone's.

#![cfg(feature = "aws")]

mod support {
    #[path = "http_fake.rs"]
    pub mod http_fake;
    #[path = "sigv4.rs"]
    pub mod sigv4;
}

use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use cobolt_mcp::Mode;
use cobolt_runtime::aws::connections::AwsConnection;
use cobolt_runtime::aws::ops::{self, ControlView, Outcome};
use cobolt_runtime::aws::pool::{self, Budget, CallError, ServerKey};
use cobolt_runtime::aws::process::{Credentials, HttpsEndpoint, Launch};
use serde_json::{json, Value};
use support::http_fake::{HttpFake, Logged};
use support::sigv4;

/// The list of connections is process-wide: a test that publishes one holds this while it uses it.
static CONNECTIONS: Mutex<()> = Mutex::new(());

/// AWS's documented example credentials (their docs print this pair).
const KEY_ID: &str = "AKIDEXAMPLE";
const SECRET: &str = "wJalrXUtnFEMI/K7MDENG+bPxRfiCYEXAMPLEKEY";
const TOKEN: &str = "EXAMPLE-SESSION-TOKEN-NOT-REAL";

fn tools() -> Value {
    json!([
        {"name": "aws___run_script", "inputSchema": {"type": "object", "properties": {"code": {"type": "string"}}, "required": ["code"]}},
        {"name": "other_tool", "inputSchema": {"type": "object"}}
    ])
}

fn fake(extra: Value) -> HttpFake {
    let mut script = json!({"tools": tools()});
    for (k, v) in extra.as_object().unwrap() {
        script[k] = v.clone();
    }
    HttpFake::start(script)
}

fn launch_to(url: &str, region: &str, keys: bool) -> Launch {
    Launch {
        profile: Some("dev".into()),
        region: Some(region.into()),
        credentials: keys.then(|| Arc::new(Credentials::new(KEY_ID.into(), SECRET.into(), TOKEN.into()))),
        https: Some(HttpsEndpoint {
            url: url.to_owned(),
            service: "aws-mcp".into(),
            signing_region: "us-east-1".into(),
            metadata: vec![("AWS_REGION".into(), region.into())],
        }),
        ..Launch::default()
    }
}

fn launch(f: &HttpFake, region: &str) -> Launch {
    launch_to(&f.url, region, true)
}

fn key(tag: &str) -> ServerKey {
    ServerKey { connection: format!("https-{tag}-{}", std::process::id()), server: "hosted".into() }
}

fn budget(call_ms: u64) -> Budget {
    Budget { call: Duration::from_millis(call_ms), start: Duration::from_secs(10) }
}

fn call(k: &ServerKey, l: &Launch, b: Budget) -> Result<String, CallError> {
    pool::call_tool(k, l, Mode::Handshake, "other_tool", json!({"n": 1}), b)
        .map(|r| r.content[0].as_text().unwrap_or_default().to_owned())
}

// ── AC1: the signatures ───────────────────────────────────────────────────────

#[test]
fn ac1_every_signed_request_verifies_with_an_independent_implementation() {
    // The verifier must first reproduce AWS's own published example.
    let published = sigv4::reproduces_the_published_example().expect("the verifier matches AWS's published signature");

    let f = fake(json!({"page_size": 1}));
    let (k, l) = (key("sig"), launch(&f, "eu-west-1"));
    assert_eq!(call(&k, &l, budget(5000)).unwrap(), r#"{"n":1}"#);
    let listed = pool::list_tools(&k, &l, Mode::Handshake, budget(5000)).unwrap();
    assert_eq!(listed.len(), 2, "two tools across two pages");
    assert_eq!(call(&k, &l, budget(5000)).unwrap(), r#"{"n":1}"#);
    pool::stop(&k);
    assert!(f.wait_for(|f| f.count_http("DELETE") == 1, Duration::from_secs(3)), "the session is ended with a DELETE");

    let requests = f.requests();
    let mut verified = 0;
    let mut kinds = Vec::new();
    for r in &requests {
        let auth = sigv4::verify(r, SECRET, "aws-mcp", "us-east-1")
            .unwrap_or_else(|e| panic!("{} {} {:?} does not verify: {e}", r.method, r.path, r.rpc_method()));
        assert_eq!(auth.key_id, KEY_ID);
        verified += 1;
        kinds.push(r.rpc_method().unwrap_or_else(|| r.method.clone()));
    }
    // The token travels in its header, signed; the secret travels nowhere.
    for r in &requests {
        assert_eq!(r.header("x-amz-security-token"), Some(TOKEN));
        let wire = format!("{:?}{}", r.headers, String::from_utf8_lossy(&r.body));
        assert!(!wire.contains(SECRET), "the secret key is never sent");
    }
    // A request changed after signing no longer verifies.
    let call_req = requests.iter().find(|r| r.rpc_method().as_deref() == Some("tools/call")).unwrap();
    let mut forged: Logged = call_req.clone();
    *forged.body.last_mut().unwrap() ^= 1;
    assert!(sigv4::verify(&forged, SECRET, "aws-mcp", "us-east-1").is_err(), "an altered body is detected");
    let mut other_secret = call_req.clone();
    other_secret.headers.retain(|(k, _)| k != "x-amz-security-token");
    assert!(sigv4::verify(&other_secret, SECRET, "aws-mcp", "us-east-1").is_err(), "a dropped signed header is detected");
    assert!(sigv4::verify(call_req, "a-different-secret", "aws-mcp", "us-east-1").is_err(), "another secret is detected");

    println!(
        "\n  103 AC1 — independent SigV4 check: AWS's published example reproduced ({}…); \
         {verified} of {} logged requests verify ({}); an altered body, a dropped signed header and a wrong secret are each refused",
        &published[..8],
        requests.len(),
        kinds.join(", ")
    );
    assert_eq!(verified, requests.len());
    assert_eq!(requests.len(), 7, "initialize, initialized, tools/call, tools/list ×2, tools/call, DELETE");
}

// ── AC2: both answer forms, and the session ──────────────────────────────────────

#[test]
fn ac2_the_handshake_works_in_both_answer_forms() {
    let mut table = Vec::new();
    for (form, sse) in [("application/json", false), ("text/event-stream", true)] {
        let f = fake(json!({"sse": sse, "page_size": 1, "notify_before": true}));
        let (k, l) = (key(&format!("form{}", sse as u8)), launch(&f, "eu-west-1"));
        let t = Instant::now();
        assert_eq!(call(&k, &l, budget(5000)).unwrap(), r#"{"n":1}"#, "{form}");
        assert_eq!(pool::list_tools(&k, &l, Mode::Handshake, budget(5000)).unwrap().len(), 2, "{form}: two pages");
        let took = t.elapsed();
        let requests = f.requests();
        let init = requests.iter().find(|r| r.rpc_method().as_deref() == Some("initialize")).unwrap();
        assert!(init.header("mcp-session-id").is_none() && init.header("mcp-protocol-version").is_none(), "{form}: initialize names no session");
        assert_eq!(init.header("accept"), Some("application/json, text/event-stream"));
        assert_eq!(init.header("accept-encoding"), Some("identity"), "{form}: nothing is compressed under a signature");
        let later: Vec<&Logged> = requests.iter().filter(|r| r.rpc_method().as_deref() != Some("initialize") && r.method == "POST").collect();
        assert!(!later.is_empty());
        for r in &later {
            assert_eq!(r.header("mcp-session-id"), Some("session-1"), "{form}: {:?}", r.rpc_method());
            assert_eq!(r.header("mcp-protocol-version"), Some("2025-06-18"), "{form}");
        }
        // The server's own ping is answered, and the answer is not a request.
        let pings = requests.iter().filter(|r| r.rpc_method().is_none() && r.method == "POST").count();
        pool::stop(&k);
        assert!(f.wait_for(|f| f.count_http("DELETE") == 1, Duration::from_secs(3)), "{form}: the session is ended");
        table.push(format!("{form}: {} requests, {} server ping(s) answered, {} ms", requests.len(), pings, took.as_millis()));
        assert_eq!(pings, if sse { 1 } else { 0 }, "{form}: the server's ping travels in the event stream only, and is answered once");
    }
    println!("\n  103 AC2 — handshake, tools/list over 2 pages, tools/call, DELETE, in both answer forms:\n    {}", table.join("\n    "));
}

// ── AC3: _meta ────────────────────────────────────────────────────────────────

#[test]
fn ac3_the_metadata_is_on_every_request_of_every_connection() {
    let mut seen = 0;
    for region in ["eu-west-1", "ap-southeast-2"] {
        let f = fake(json!({}));
        let (k, l) = (key(&format!("meta-{region}")), launch(&f, region));
        call(&k, &l, budget(5000)).unwrap();
        pool::list_tools(&k, &l, Mode::Handshake, budget(5000)).unwrap();
        pool::stop(&k);
        f.wait_for(|f| f.count_http("DELETE") == 1, Duration::from_secs(3));
        for r in f.requests().iter().filter(|r| r.method == "POST") {
            let Some(msg) = r.json() else { continue };
            if msg.get("method").is_none() {
                continue;
            }
            assert_eq!(msg["params"]["_meta"]["AWS_REGION"], region, "{:?}", r.rpc_method());
            seen += 1;
        }
    }
    // A route with no metadata sends none.
    let f = fake(json!({}));
    let k = key("meta-none");
    let mut l = launch(&f, "eu-west-1");
    l.https.as_mut().unwrap().metadata.clear();
    call(&k, &l, budget(5000)).unwrap();
    pool::stop(&k);
    for r in f.requests().iter().filter(|r| r.method == "POST") {
        assert!(r.json().is_some_and(|m| m.get("params").and_then(|p| p.get("_meta")).is_none()), "{:?}", r.rpc_method());
    }
    println!("\n  103 AC3 — params._meta.AWS_REGION on {seen} of {seen} messages across two regions; none when the route declares none");
    assert!(seen >= 8, "initialize, initialized and tools/call for each of two connections, plus tools/list");
}

// ── AC4: expiry, silence, unreachability, refusals ────────────────────────────────

#[test]
fn ac4_an_expired_session_is_renewed_once_and_no_more() {
    // The second call meets a session the server forgot: one new session, the call repeated, and it works.
    let f = fake(json!({"expire_session_on_call": 2}));
    let (k, l) = (key("expiry"), launch(&f, "eu-west-1"));
    assert!(call(&k, &l, budget(5000)).is_ok());
    assert!(call(&k, &l, budget(5000)).is_ok(), "the second call is repeated on a new session");
    assert_eq!(f.sessions_opened(), 2, "one session renewed");
    assert_eq!(f.count_rpc("tools/call"), 3, "call 1, call 2 (refused), call 2 again");
    let last = f.requests().into_iter().rev().find(|r| r.rpc_method().as_deref() == Some("tools/call")).unwrap();
    assert_eq!(last.header("mcp-session-id"), Some("session-2"));
    pool::stop(&k);

    // A server that never knows its session: exactly one renewal, then an error.
    let f2 = fake(json!({"always_unknown_session": true}));
    let (k2, l2) = (key("expiry-always"), launch(&f2, "eu-west-1"));
    let e = call(&k2, &l2, budget(5000)).unwrap_err();
    assert!(matches!(e, CallError::Failed(_)), "{e:?}");
    assert_eq!(f2.sessions_opened(), 2, "the session was opened once, and renewed once");
    assert_eq!(f2.count_rpc("tools/call"), 2, "the call was repeated once, not forever");
    pool::stop(&k2);
    println!("\n  103 AC4 — expiry: session renewed once and the call repeated (2 sessions, 3 tools/call); a server that never knows it: 2 sessions, 2 tools/call, then an error");
}

#[test]
fn ac4_silence_unreachability_and_refusals_are_reported_plainly() {
    // A server that never answers a call: a timeout within the budget.
    let f = fake(json!({"silent": ["other_tool"]}));
    let (k, l) = (key("silent"), launch(&f, "eu-west-1"));
    pool::list_tools(&k, &l, Mode::Handshake, budget(5000)).unwrap();
    let t = Instant::now();
    let e = call(&k, &l, budget(400)).unwrap_err();
    let took = t.elapsed();
    assert_eq!(e, CallError::Timeout);
    assert!(took < Duration::from_millis(1400), "TimeoutMs 400 -> Timeout after {took:?}");
    pool::stop(&k);

    // Nothing is listening: the start fails in words, not in a socket error code.
    let port = {
        let l = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        l.local_addr().unwrap().port()
    };
    let k = key("refused");
    let e = call(&k, &launch_to(&format!("http://127.0.0.1:{port}/mcp"), "eu-west-1", true), budget(2000)).unwrap_err();
    let CallError::Start(m) = &e else { panic!("{e:?}") };
    assert!(m.contains("could not be reached"), "{m}");
    assert!(!m.contains(SECRET) && !m.contains(KEY_ID), "no key in an error");

    // An expired sign-in reads as the AWS CLI's fix, naming the profile.
    let f = fake(json!({"http_error": {"status": 403, "body": "{\"__type\":\"ExpiredTokenException\",\"message\":\"The security token included in the request is expired\"}"}}));
    let k = key("expired");
    let e = call(&k, &launch(&f, "eu-west-1"), budget(2000)).unwrap_err();
    let CallError::Start(m) = &e else { panic!("{e:?}") };
    assert!(m.contains("403"), "{m}");
    let plain = ops::plain_error(m, "dev");
    assert!(plain.contains("aws login --profile dev"), "{plain}");

    // No keys, and no profile chain yet (spec 103 P3): the same words.
    let f = fake(json!({}));
    let e = call(&key("nokeys"), &launch_to(&f.url, "eu-west-1", false), budget(2000)).unwrap_err();
    let CallError::Start(m) = &e else { panic!("{e:?}") };
    assert!(ops::plain_error(m, "dev").contains("aws login --profile dev"), "{m}");
    assert_eq!(f.requests().len(), 0, "nothing was sent without credentials");
    println!("\n  103 AC4 — a silent server: Timeout after {took:?} (TimeoutMs 400); an unreachable endpoint: \"the AWS service could not be reached\"; a 403 expired token and missing credentials: \"{plain}\"");
}

// ── AC5: the templates did not change ─────────────────────────────────────────────

/// Canonical text of a TOML value: compact JSON with the keys sorted.
fn canon(v: &toml::Value) -> String {
    match v {
        toml::Value::String(s) => serde_json::to_string(s).unwrap(),
        toml::Value::Integer(n) => n.to_string(),
        toml::Value::Float(x) => x.to_string(),
        toml::Value::Boolean(b) => b.to_string(),
        toml::Value::Datetime(d) => serde_json::to_string(&d.to_string()).unwrap(),
        toml::Value::Array(a) => format!("[{}]", a.iter().map(canon).collect::<Vec<_>>().join(",")),
        toml::Value::Table(t) => {
            let mut keys: Vec<&String> = t.keys().collect();
            keys.sort();
            let items: Vec<String> = keys.iter().map(|k| format!("{}:{}", serde_json::to_string(k).unwrap(), canon(&t[*k]))).collect();
            format!("{{{}}}", items.join(","))
        }
    }
}

fn sha256_hex(text: &str) -> String {
    use sha2::{Digest, Sha256};
    Sha256::digest(text.as_bytes()).iter().map(|b| format!("{b:02x}")).collect()
}

/// `name sha256` for every operation the table sends to the hosted server, and for the snippets.
fn hosted_hashes(table_text: &str) -> Vec<(String, String)> {
    let table: toml::Value = toml::from_str(table_text).expect("the route table parses");
    let mut out = Vec::new();
    for (name, op) in table["ops"].as_table().unwrap() {
        if op["server"].as_str() == Some("hosted") {
            out.push((name.clone(), sha256_hex(&canon(op))));
        }
    }
    out.push(("[snippets]".to_owned(), sha256_hex(&canon(&table["snippets"]))));
    out
}

const BASELINE: &str = include_str!("fixtures/aws-mcp/hosted-templates-1.91.179.sha256");

/// Run with `HOSTED_TOML=<an older routes.toml>` and `--ignored --nocapture` to print the fixture.
#[test]
#[ignore = "prints the baseline fixture from the file named by HOSTED_TOML"]
fn print_the_hosted_template_hashes() {
    let path = std::env::var("HOSTED_TOML").expect("HOSTED_TOML names a routes.toml");
    for (name, hash) in hosted_hashes(&std::fs::read_to_string(path).unwrap()) {
        println!("{name} {hash}");
    }
}

#[test]
fn ac5_the_33_hosted_templates_are_byte_identical_to_the_table_before_this_change() {
    let want: Vec<(String, String)> = BASELINE
        .lines()
        .filter(|l| !l.trim().is_empty() && !l.starts_with('#'))
        .map(|l| {
            let (n, h) = l.rsplit_once(' ').expect("name and hash");
            (n.to_owned(), h.to_owned())
        })
        .collect();
    let got = hosted_hashes(cobolt_runtime::aws::routes::SHIPPED);
    assert_eq!(want.len(), 34, "33 hosted operations and the snippets in the baseline");
    let mut same = 0;
    for (name, hash) in &want {
        match got.iter().find(|(n, _)| n == name) {
            Some((_, h)) if h == hash => same += 1,
            Some(_) => panic!("{name} changed since 1.91.179"),
            None => panic!("{name} is no longer in the table"),
        }
    }
    assert_eq!(got.len(), want.len(), "no hosted operation was added or moved either");
    println!("\n  103 AC5 — {same} of {} hashes equal main's (33 hosted operations + [snippets])", want.len());
    // The guard guards: one changed byte is a different hash.
    let shipped = cobolt_runtime::aws::routes::SHIPPED;
    let doctored = shipped.replacen("operation_name='GetItem'", "operation_name='GetItem '", 1);
    assert_ne!(doctored, shipped, "the mutation found DynamoDB's GetItem template to change");
    assert_ne!(hosted_hashes(&doctored), got, "a mutated template byte is detected");
}

// ── AC10: a refused operation sends nothing; an allowed one runs on real templates ──────

fn connection_to(f: &HttpFake, id: &str) -> AwsConnection {
    AwsConnection {
        id: id.to_owned(),
        name: id.to_owned(),
        profile: "dev".into(),
        region: "eu-west-1".into(),
        routes_override: format!(
            "[servers.hosted]\ntransport = 'https'\nurl = '{}'\nservice = 'aws-mcp'\nsigning_region = 'us-east-1'\nmetadata = {{ AWS_REGION = \"{{Connection.Region|or:us-east-1}}\" }}\n",
            f.url
        ),
        ..Default::default()
    }
}

#[test]
fn ac10_a_refused_operation_sends_nothing_and_an_allowed_one_runs_the_real_template() {
    let envelope = |v: Value| {
        let text = json!({"status": "success", "stdout": "", "stderr": "", "return_value": v}).to_string();
        json!({"content": [{"type": "text", "text": text}], "isError": false})
    };
    let _own = CONNECTIONS.lock().unwrap_or_else(|e| e.into_inner());
    let f = fake(json!({"answers": {"aws___run_script": envelope(json!({"Done": "1"}))}}));
    let conn = connection_to(&f, "ac10");
    let prop = |p: &str| if p == "TableName" { "orders".to_owned() } else { String::new() };
    let args = [r#"{"id":42,"name":"Ana"}"#.to_owned()];
    let view = |allow: bool| ControlView {
        control_type: "AwsDynamoDB",
        method: "PutItem",
        args: &args,
        prop: &prop,
        control: "DDB-1",
        allow_write: allow,
        connection_allows_write: allow,
    };

    let refused = ops::prepare(&view(false), &conn).unwrap_err();
    assert!(refused.contains("AllowWrite"), "{refused}");
    assert_eq!(f.requests().len(), 0, "a refused operation sent nothing: zero requests");

    // Keys the program set for the connection sign the call (spec 092 A6).
    cobolt_runtime::aws::connections::publish(vec![conn.clone()]);
    cobolt_runtime::aws::credentials::set("ac10", KEY_ID.into(), SECRET.into(), String::new()).unwrap();
    let prepared = ops::prepare(&view(true), &conn).expect("allowed once AllowWrite is on");
    assert!(prepared.launch.command.is_empty() && prepared.launch.https.is_some(), "no program is started for the hosted server");
    let outcome = ops::execute(&prepared, budget(5000));
    cobolt_runtime::aws::credentials::clear("ac10").unwrap();
    let Outcome::Done { props, event, .. } = &outcome else { panic!("{outcome:?}") };
    assert_eq!(event, "onItemPut");
    println!("\n  103 AC10 — AwsDynamoDB.PutItem with AllowWrite off: refused, 0 requests; on: {} requests, event {event}, properties {:?}", f.requests().len(), props);
    let script = f
        .requests()
        .iter()
        .filter_map(Logged::json)
        .find(|m| m["method"] == "tools/call")
        .map(|m| m["params"]["arguments"]["code"].as_str().unwrap_or_default().to_owned())
        .expect("a script was sent");
    assert!(script.contains("operation_name='PutItem'") && script.contains("region_name=\"eu-west-1\""), "{script}");
    for r in f.requests() {
        sigv4::verify(&r, SECRET, "aws-mcp", "us-east-1").unwrap();
    }
}

/// The write operations the demos only see refused (AllowWrite is off in them): with AllowWrite on, each
/// runs its real template against the endpoint — so all 33 hosted operations are shown to complete
/// (spec 103 AC9), the 25 the demos ask for being shown by the CLI suite.
#[test]
fn ac9_the_eight_write_operations_complete_when_allowed() {
    let _own = CONNECTIONS.lock().unwrap_or_else(|e| e.into_inner());
    let envelope = |v: Value| {
        let text = json!({"status": "success", "stdout": "", "stderr": "", "return_value": v}).to_string();
        json!({"content": [{"type": "text", "text": text}], "isError": false})
    };
    let f = fake(json!({"answers": {"aws___run_script": envelope(json!({"Done": "1"}))}}));
    let conn = connection_to(&f, "ac9w");
    cobolt_runtime::aws::connections::publish(vec![conn.clone()]);
    cobolt_runtime::aws::credentials::set("ac9w", KEY_ID.into(), SECRET.into(), String::new()).unwrap();

    let source = std::env::temp_dir().join(format!("prc-103-put-{}.txt", std::process::id()));
    std::fs::write(&source, b"hello").unwrap();
    let source_path = source.to_string_lossy().into_owned();

    // control, method, COBOL arguments, properties, the control's own event, the AWS operation it must send
    #[allow(clippy::type_complexity)]
    let cases: Vec<(&str, &str, Vec<String>, Vec<(&str, &str)>, &str, &str)> = vec![
        ("AwsCognito", "SignUp", vec!["ana".into(), "pw-Example-1".into(), "ana@example.com".into()], vec![("ClientId", "client")], "onSignedUp", "SignUp"),
        ("AwsCognito", "Confirm", vec!["ana".into(), "123456".into()], vec![("ClientId", "client")], "onConfirmed", "ConfirmSignUp"),
        ("AwsDynamoDB", "UpdateItem", vec![r#"{"id":42}"#.into(), "SET #n = :n".into(), r#"{":n":"Bo"}"#.into()], vec![("TableName", "orders")], "onItemUpdated", "UpdateItem"),
        ("AwsDynamoDB", "DeleteItem", vec![r#"{"id":42}"#.into()], vec![("TableName", "orders")], "onItemDeleted", "DeleteItem"),
        ("AwsEC2", "Stop", vec!["i-0123456789abcdef0".into()], vec![], "onInstancesStopped", "StopInstances"),
        ("AwsS3", "PutObject", vec!["a.txt".into(), "text/plain".into(), source_path], vec![("Bucket", "photos")], "onObjectPut", "PutObject"),
        ("AwsS3", "DeleteObject", vec!["a.txt".into()], vec![("Bucket", "photos")], "onObjectDeleted", "DeleteObject"),
        ("AwsS3Vectors", "PutVectors", vec![String::new(), r#"[{"key":"d1","data":[0.1,0.2]}]"#.into()], vec![("IndexName", "idx"), ("VectorBucketName", "vb")], "onVectorsPut", "PutVectors"),
    ];
    let started = Instant::now();
    for (control_type, method, args, props, event, aws_operation) in &cases {
        let prop = |p: &str| props.iter().find(|(k, _)| *k == p).map(|(_, v)| (*v).to_owned()).unwrap_or_default();
        let view = |allow: bool| ControlView {
            control_type,
            method,
            args,
            prop: &prop,
            control: "W-1",
            allow_write: allow,
            connection_allows_write: allow,
        };
        let before = f.requests().len();
        assert!(ops::prepare(&view(false), &conn).is_err(), "{control_type}.{method} is refused with AllowWrite off");
        assert_eq!(f.requests().len(), before, "{control_type}.{method}: a refused operation sends nothing");
        let prepared = ops::prepare(&view(true), &conn).unwrap_or_else(|e| panic!("{control_type}.{method}: {e}"));
        match ops::execute(&prepared, budget(10_000)) {
            Outcome::Done { event: got, .. } => assert_eq!(&got, event, "{control_type}.{method}"),
            other => panic!("{control_type}.{method}: {other:?}"),
        }
        let sent = f
            .requests()
            .iter()
            .filter_map(Logged::json)
            .filter(|m| m["method"] == "tools/call")
            .map(|m| m["params"]["arguments"]["code"].as_str().unwrap_or_default().to_owned())
            .last()
            .expect("a script was sent");
        assert!(sent.contains(&format!("operation_name='{aws_operation}'")), "{control_type}.{method} sent: {sent}");
    }
    cobolt_runtime::aws::credentials::clear("ac9w").unwrap();
    let _ = std::fs::remove_file(&source);
    for r in f.requests() {
        sigv4::verify(&r, SECRET, "aws-mcp", "us-east-1").unwrap();
    }
    println!(
        "\n  103 AC9 — the 8 write operations the demos see refused: each refused with AllowWrite off (0 requests), then completed with it on \
         with its own event and its AWS operation in the script; {} signed requests verified, {} ms",
        f.requests().len(),
        started.elapsed().as_millis()
    );
}
