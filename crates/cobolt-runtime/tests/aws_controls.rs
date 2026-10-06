// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! The `AwsLambda` and `AwsMcp` controls, driven from COBOL against the fake
//! MCP server (spec 078 T-A11; AC11, AC12, AC15).
//!
//! Each test gives its control a connection of its own whose route override
//! points the server at `fake_mcp`, so nothing here needs AWS, a network or
//! Python. The connection registry is process-global, so the tests take turns.

#![cfg(feature = "aws")]

use std::path::PathBuf;
use std::sync::{mpsc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use cobolt_lexer::{tokenize, SourceFormat};
use cobolt_parser::{parse, Severity};
use cobolt_runtime::aws::connections::{self, AwsConnection};
use cobolt_runtime::aws::pool::{self, ServerKey};
use cobolt_runtime::{FormEvent, Interpreter};
use serde_json::{json, Value};

static TURN: Mutex<()> = Mutex::new(());

struct Fake {
    dir: PathBuf,
    conn: AwsConnection,
    server: &'static str,
}

impl Drop for Fake {
    fn drop(&mut self) {
        pool::stop(&ServerKey { connection: self.conn.id.clone(), server: self.server.into() });
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

impl Fake {
    fn calls(&self) -> Vec<Value> {
        std::fs::read_to_string(self.dir.join("calls.jsonl"))
            .unwrap_or_default()
            .lines()
            .filter_map(|l| serde_json::from_str(l).ok())
            .collect()
    }
    fn tool_calls(&self) -> usize {
        self.calls().iter().filter(|c| c["method"] == "tools/call").count()
    }
    fn started(&self) -> bool {
        self.dir.join("pids.txt").exists()
    }
}

/// A connection whose `server` (`lambda`, or `fake` for `AwsMcp`) is the fake
/// server, run with `script` merged into the defaults.
fn fake(tag: &str, server: &'static str, script: Value) -> Fake {
    let dir = std::env::temp_dir().join(format!("prc-078-ctl-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let mut s = json!({
        "pid_file": dir.join("pids.txt").to_string_lossy(),
        "call_log": dir.join("calls.jsonl").to_string_lossy(),
    });
    for (k, v) in script.as_object().unwrap() {
        s[k] = v.clone();
    }
    let script_path = dir.join("script.json");
    std::fs::write(&script_path, s.to_string()).unwrap();
    let routes_override = format!(
        "[servers.{server}]\ncommand = '{}'\nargs = ['{}']\n",
        env!("CARGO_BIN_EXE_fake_mcp"),
        script_path.display()
    );
    let conn = AwsConnection {
        id: format!("conn-{tag}"),
        name: format!("conn-{tag}"),
        profile: "test".into(),
        region: "eu-west-1".into(),
        routes_override,
        ..Default::default()
    };
    connections::publish(vec![conn.clone()]);
    Fake { dir, conn, server }
}

fn program(setup: &str, on_own: &str) -> cobolt_ast::program::Program {
    let src = format!(
        r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. AWSCTL.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 COBOL-EVENT-ID    PIC X(30).
       01 COBOL-CONTROL-ID  PIC X(30).
       01 COBOL-QUIT        PIC 9 VALUE 0.
       01 WS-NAME           PIC X(30).
       01 WS-PAY            PIC X(200).
       01 WS-RC             PIC X(200).
       01 WS-N              PIC X(10).
       01 WS-I              PIC 9 VALUE 2.
       01 WS-F              PIC X(10) VALUE "name".
       PROCEDURE DIVISION.
       MAIN.
{setup}
           PERFORM UNTIL COBOL-QUIT = 1
               CALL "COBOL-WAIT-EVENT"
                   USING COBOL-EVENT-ID COBOL-CONTROL-ID
               DISPLAY "EVENT " COBOL-EVENT-ID
               EVALUATE COBOL-EVENT-ID
                   WHEN "onInvoked"
{on_own}
                   WHEN "onToolResult"
{on_own}
                   WHEN "onComplete"
                       MOVE 1 TO COBOL-QUIT
                   WHEN "onError"
                       MOVE AWS-1::LastError TO WS-RC
                       DISPLAY "ERROR " WS-RC
                       MOVE 1 TO COBOL-QUIT
                   WHEN "onTimeout"
                       MOVE 1 TO COBOL-QUIT
                   WHEN "onClick"
                       MOVE 1 TO COBOL-QUIT
               END-EVALUATE
           END-PERFORM.
           STOP RUN.
"#
    );
    let r = parse(tokenize(&src, SourceFormat::Free));
    assert!(r.diagnostics.iter().all(|d| d.severity != Severity::Error), "parse errors: {:?}", r.diagnostics);
    r.program.expect("no program")
}

struct Run {
    lines: Vec<String>,
    millis: u128,
}

/// Run `prog` with AWS-1 seeded as `class` with `props`; `quit_after` sends
/// an `onClick` (which ends the loop) after that long, for a test whose
/// program would otherwise wait for an event that must never come.
fn run(prog: cobolt_ast::program::Program, class: &str, props: &[(&str, &str)], quit_after: Option<Duration>) -> Run {
    let (event_tx, event_rx) = mpsc::channel::<FormEvent>();
    let (state_tx, _state_rx) = mpsc::channel();
    let (display_tx, display_rx) = mpsc::channel::<String>();
    let seeded: Vec<(String, String)> = props.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect();
    let class = class.to_string();
    let started = Instant::now();
    let handle = thread::spawn(move || {
        let mut interp = Interpreter::new_with_channels(prog, event_rx, state_tx, display_tx);
        interp.seed_objects(vec![("AWS-1".to_string(), class, seeded)]);
        let _ = interp.run();
    });
    if let Some(d) = quit_after {
        thread::sleep(d);
        let _ = event_tx.send(FormEvent::new("BTN-1", "onClick"));
    }
    let deadline = Instant::now() + Duration::from_secs(20);
    while !handle.is_finished() {
        assert!(Instant::now() < deadline, "the program never finished");
        thread::sleep(Duration::from_millis(10));
    }
    drop(event_tx);
    let millis = started.elapsed().as_millis();
    let lines = display_rx.try_iter().map(|l| l.trim_end().to_string()).collect();
    Run { lines, millis }
}

fn lambda_script(delay_ms: u64) -> Value {
    let reply = r#"Function app-f returned: [{"id":1,"name":"Ana"},{"id":2,"name":"Bo"}]"#;
    json!({
        "tools": [{"name": "app_f", "inputSchema": {"type": "object", "properties": {"parameters": {"type": "object"}}}}],
        "answers": {"app_f": {"content": [{"type": "text", "text": reply}], "isError": false}},
        "delay_ms": {"app_f": delay_ms},
    })
}

const INVOKE: &str = r#"
           MOVE "app-f" TO WS-NAME
           MOVE '{"id": 7}' TO WS-PAY
           INVOKE AWS-1 "Invoke" USING WS-NAME WS-PAY RETURNING WS-RC
           DISPLAY "STARTED " WS-RC"#;

const READ_ROWS: &str = r#"
                       MOVE AWS-1::RowCount TO WS-N
                       DISPLAY "ROWS " WS-N
                       INVOKE AWS-1 "GetField" USING WS-I WS-F RETURNING WS-RC
                       DISPLAY "FIELD " WS-RC"#;

fn lambda_props<'a>(conn: &'a str, allow: &'a str, mode: &'a str) -> Vec<(&'a str, &'a str)> {
    vec![
        ("Connection", conn),
        ("Mode", mode),
        ("AllowWrite", allow),
        ("TimeoutMs", "5000"),
        ("StartTimeoutMs", "10000"),
        ("FunctionName", ""),
        ("Verbose", "false"),
    ]
}

fn report(test: &str, cases: &[(&str, String)], r: &Run) {
    eprintln!("\n  ── {test} ── {} ms", r.millis);
    for (case, outcome) in cases {
        eprintln!("    {case:<44} {outcome}");
    }
}

fn events(r: &Run) -> Vec<&str> {
    r.lines.iter().filter_map(|l| l.strip_prefix("EVENT ")).map(str::trim).collect()
}

#[test]
fn invoke_raises_oninvoked_then_oncomplete() {
    let _turn = TURN.lock().unwrap_or_else(|e| e.into_inner());
    let f = fake("invoke", "lambda", lambda_script(0));
    let r = run(program(INVOKE, READ_ROWS), "AwsLambda", &lambda_props(&f.conn.name, "true", "Async"), None);
    let ev = events(&r);
    report(
        "Invoke, async, AllowWrite on",
        &[
            ("events, in order", format!("{ev:?}")),
            ("RowCount / GetField(2, name)", format!("{:?}", r.lines.iter().filter(|l| l.starts_with("ROWS") || l.starts_with("FIELD")).collect::<Vec<_>>())),
            ("tools/call sent", f.tool_calls().to_string()),
        ],
        &r,
    );
    assert!(r.lines.iter().any(|l| l == "STARTED 1"), "{:?}", r.lines);
    assert_eq!(ev, ["onInvoked", "onComplete"], "own event first, then onComplete");
    assert!(r.lines.iter().any(|l| l == "ROWS 2"), "{:?}", r.lines);
    assert!(r.lines.iter().any(|l| l == "FIELD Bo"), "{:?}", r.lines);
    let call = f.calls().into_iter().find(|c| c["method"] == "tools/call").expect("no tools/call");
    assert_eq!(call["params"]["name"], "app_f");
    assert_eq!(call["params"]["arguments"], json!({"parameters": {"id": 7}}));
}

#[test]
fn invalid_json_names_the_argument_and_sends_nothing() {
    let _turn = TURN.lock().unwrap_or_else(|e| e.into_inner());
    let f = fake("badjson", "lambda", lambda_script(0));
    let setup = INVOKE.replace(r#"'{"id": 7}'"#, r#"'{nope'"#);
    let r = run(program(&setup, READ_ROWS), "AwsLambda", &lambda_props(&f.conn.name, "true", "Async"), None);
    let err = r.lines.iter().find(|l| l.starts_with("ERROR ")).cloned().unwrap_or_default();
    report("Invoke with invalid JSON", &[("LastError", err.clone()), ("server started", f.started().to_string())], &r);
    assert_eq!(events(&r), ["onError"]);
    assert!(err.contains("argument 2") && err.contains("not valid JSON"), "{err}");
    assert!(!f.started(), "nothing may be sent — the server must not even start");
}

#[test]
fn allowwrite_off_refuses_before_sending() {
    let _turn = TURN.lock().unwrap_or_else(|e| e.into_inner());
    // Invoke is mutating: refused before the pool is touched.
    let f = fake("nowrite", "lambda", lambda_script(0));
    let r = run(program(INVOKE, READ_ROWS), "AwsLambda", &lambda_props(&f.conn.name, "false", "Async"), None);
    let err = r.lines.iter().find(|l| l.starts_with("ERROR ")).cloned().unwrap_or_default();
    assert_eq!(events(&r), ["onError"]);
    assert!(err.contains("AllowWrite is off"), "{err}");
    assert!(!f.started(), "a refused Invoke must not start the server");
    let lambda_case = format!("onError, server started: {}", f.started());
    drop(f);

    // AwsMcp: a tool not marked read-only is refused; a read-only one runs.
    let g = fake(
        "mcpwrite",
        "fake",
        json!({"tools": [
            {"name": "read_thing", "inputSchema": {"type": "object"}, "annotations": {"readOnlyHint": true}},
            {"name": "write_thing", "inputSchema": {"type": "object"}}
        ]}),
    );
    let mcp = |tool: &str| {
        format!(
            r#"
           MOVE "{tool}" TO WS-NAME
           MOVE '{{"k": 1}}' TO WS-PAY
           INVOKE AWS-1 "Call" USING WS-NAME WS-PAY RETURNING WS-RC"#
        )
    };
    let props = vec![
        ("Connection", g.conn.name.as_str()),
        ("Mode", "Async"),
        ("AllowWrite", "false"),
        ("TimeoutMs", "5000"),
        ("StartTimeoutMs", "10000"),
        ("ServerId", "fake"),
        ("ToolName", ""),
    ];
    let w = run(program(&mcp("write_thing"), READ_ROWS), "AwsMcp", &props, None);
    let werr = w.lines.iter().find(|l| l.starts_with("ERROR ")).cloned().unwrap_or_default();
    assert_eq!(events(&w), ["onError"]);
    assert!(werr.contains("not marked read-only"), "{werr}");
    assert_eq!(g.tool_calls(), 0, "the write tool must never be called");
    let rd = run(program(&mcp("read_thing"), "                       CONTINUE"), "AwsMcp", &props, None);
    assert_eq!(events(&rd), ["onToolResult", "onComplete"]);
    assert_eq!(g.tool_calls(), 1);
    report(
        "AllowWrite off",
        &[
            ("AwsLambda.Invoke", lambda_case),
            ("AwsMcp.Call write_thing (no readOnlyHint)", format!("{:?}, tools/call sent 0", events(&w))),
            ("AwsMcp.Call read_thing (readOnlyHint)", format!("{:?}, tools/call sent 1", events(&rd))),
        ],
        &rd,
    );
}

#[test]
fn a_stale_result_after_cancel_is_discarded() {
    let _turn = TURN.lock().unwrap_or_else(|e| e.into_inner());
    let f = fake("cancel", "lambda", lambda_script(600));
    let setup = format!("{INVOKE}\n           INVOKE AWS-1 \"Cancel\"");
    // Wait well past the server's answer, then end the loop.
    let r = run(program(&setup, READ_ROWS), "AwsLambda", &lambda_props(&f.conn.name, "true", "Async"), Some(Duration::from_millis(1500)));
    let ev = events(&r);
    report("Cancel mid-flight", &[("events", format!("{ev:?}")), ("tools/call sent", f.tool_calls().to_string())], &r);
    assert_eq!(ev, ["onCancelled", "onClick"], "the late answer must raise nothing");
    assert!(!r.lines.iter().any(|l| l.starts_with("ROWS")));
}

#[test]
fn sync_mode_returns_the_body() {
    let _turn = TURN.lock().unwrap_or_else(|e| e.into_inner());
    let f = fake("sync", "lambda", lambda_script(0));
    let setup = format!(
        "{INVOKE}\n           DISPLAY \"BODY \" WS-RC\n           MOVE AWS-1::RowCount TO WS-N\n           DISPLAY \"ROWS \" WS-N\n           MOVE 1 TO COBOL-QUIT"
    );
    let r = run(program(&setup, READ_ROWS), "AwsLambda", &lambda_props(&f.conn.name, "true", "Sync"), None);
    let body = r.lines.iter().find(|l| l.starts_with("BODY ")).cloned().unwrap_or_default();
    report("Invoke, Mode = Sync", &[("returned", body.clone()), ("events", format!("{:?}", events(&r)))], &r);
    assert!(body.starts_with(r#"BODY [{"id":1,"name":"Ana"}"#), "{body}");
    assert!(r.lines.iter().any(|l| l == "ROWS 2"), "{:?}", r.lines);
    assert!(events(&r).is_empty(), "a synchronous call raises no event");
}
