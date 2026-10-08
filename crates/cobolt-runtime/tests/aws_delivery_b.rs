// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! The Delivery B AWS controls — `AwsKnowledgeBase`, `AwsAgentCore`,
//! `AwsAgentMemory`, `AwsS3Tables`, `AwsGlue` — driven from COBOL against the
//! fake MCP server (spec 078 T-B2…T-B6; AC11, AC15).
//!
//! Each case checks what reaches the server (the tool and its exact
//! arguments), what the program reads back (properties, rows, fields), the
//! event order, and — for an operation that changes AWS — that `AllowWrite`
//! off refuses it before the server even starts. The answers the fake gives
//! are the shapes each real server's source produces (see the fixtures'
//! provenance).
//!
//! A summary of every case and its measured time prints at the end of each
//! test (run with `--nocapture`).

#![cfg(feature = "aws")]

use std::path::PathBuf;
use std::sync::{mpsc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use cobolt_lexer::{tokenize, SourceFormat};
use cobolt_parser::{parse, Severity};
use cobolt_runtime::aws::connections::{self, AwsConnection};
use cobolt_runtime::aws::pool::{self, ServerKey};
use cobolt_runtime::aws::routes::{launch_for, Context, Routes};
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
    fn sent(&self) -> Vec<Value> {
        std::fs::read_to_string(self.dir.join("calls.jsonl"))
            .unwrap_or_default()
            .lines()
            .filter_map(|l| serde_json::from_str::<Value>(l).ok())
            .filter(|c| c["method"] == "tools/call")
            .map(|c| c["params"].clone())
            .collect()
    }
    fn started(&self) -> bool {
        self.dir.join("pids.txt").exists()
    }
}

/// A connection whose route-table `server` is the fake, answering `tool`
/// with `answer`.
fn fake(tag: &str, server: &'static str, tool: &str, answer: Value) -> Fake {
    let dir = std::env::temp_dir().join(format!("prc-078b-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let script = json!({
        "pid_file": dir.join("pids.txt").to_string_lossy(),
        "call_log": dir.join("calls.jsonl").to_string_lossy(),
        "tools": [{"name": tool, "inputSchema": {"type": "object"}}],
        "answers": { tool: answer },
    });
    let script_path = dir.join("script.json");
    std::fs::write(&script_path, script.to_string()).unwrap();
    let routes_override = format!(
        "[servers.{server}]\ncommand = '{}'\nargs = ['{}']\n",
        env!("CARGO_BIN_EXE_fake_mcp"),
        script_path.display()
    );
    let conn = AwsConnection {
        id: format!("conn-b-{tag}"),
        name: format!("conn-b-{tag}"),
        profile: "test".into(),
        region: "eu-west-1".into(),
        routes_override,
        ..Default::default()
    };
    connections::publish(vec![conn.clone()]);
    Fake { dir, conn, server }
}

fn text(t: &str) -> Value {
    json!({"content": [{"type": "text", "text": t}], "isError": false})
}

/// A program that runs `setup`, then on the control's own event runs
/// `on_own`, and ends on onComplete, onError or onTimeout.
fn program(setup: &str, on_own: &str) -> cobolt_ast::program::Program {
    let src = format!(
        r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. AWSB.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 COBOL-EVENT-ID    PIC X(30).
       01 COBOL-CONTROL-ID  PIC X(30).
       01 COBOL-QUIT        PIC 9 VALUE 0.
       01 WS-A              PIC X(200).
       01 WS-B              PIC X(200).
       01 WS-C              PIC X(200).
       01 WS-D              PIC X(200).
       01 WS-RC             PIC X(300).
       01 WS-N              PIC X(10).
       01 WS-I              PIC 9 VALUE 1.
       01 WS-F              PIC X(20).
       PROCEDURE DIVISION.
       MAIN.
{setup}
           PERFORM UNTIL COBOL-QUIT = 1
               CALL "COBOL-WAIT-EVENT"
                   USING COBOL-EVENT-ID COBOL-CONTROL-ID
               DISPLAY "EVENT " COBOL-EVENT-ID
               EVALUATE COBOL-EVENT-ID
                   WHEN "onComplete"
                       MOVE 1 TO COBOL-QUIT
                   WHEN "onError"
                       MOVE AWS-1::LastError TO WS-RC
                       DISPLAY "ERROR " WS-RC
                       MOVE 1 TO COBOL-QUIT
                   WHEN "onTimeout"
                       MOVE 1 TO COBOL-QUIT
                   WHEN OTHER
                       MOVE AWS-1::RowCount TO WS-N
                       DISPLAY "ROWS " WS-N
{on_own}
               END-EVALUATE
           END-PERFORM.
           STOP RUN.
"#
    );
    let r = parse(tokenize(&src, SourceFormat::Free));
    assert!(r.diagnostics.iter().all(|d| d.severity != Severity::Error), "parse errors: {:?}", r.diagnostics);
    r.program.expect("no program")
}

/// `DISPLAY "<label> " GetField(row, field)`.
fn field(label: &str, row: u8, name: &str) -> String {
    format!(
        "                       MOVE {row} TO WS-I\n                       MOVE \"{name}\" TO WS-F\n                       INVOKE AWS-1 \"GetField\" USING WS-I WS-F RETURNING WS-RC\n                       DISPLAY \"{label} \" WS-RC\n"
    )
}

/// `DISPLAY "<label> " AWS-1::<prop>`.
fn prop(label: &str, name: &str) -> String {
    format!("                       MOVE AWS-1::{name} TO WS-RC\n                       DISPLAY \"{label} \" WS-RC\n")
}

/// `INVOKE AWS-1 "<method>" USING …` with the given argument values.
fn invoke(method: &str, args: &[&str]) -> String {
    let vars = ["WS-A", "WS-B", "WS-C", "WS-D"];
    let mut s = String::new();
    for (v, a) in vars.iter().zip(args) {
        s.push_str(&format!("           MOVE '{a}' TO {v}\n"));
    }
    let using = if args.is_empty() { String::new() } else { format!(" USING {}", vars[..args.len()].join(" ")) };
    s.push_str(&format!("           INVOKE AWS-1 \"{method}\"{using} RETURNING WS-RC\n"));
    s
}

struct Run {
    lines: Vec<String>,
    millis: u128,
}

impl Run {
    fn events(&self) -> Vec<&str> {
        self.lines.iter().filter_map(|l| l.strip_prefix("EVENT ")).map(str::trim).collect()
    }
    fn line(&self, label: &str) -> String {
        let p = format!("{label} ");
        self.lines.iter().find_map(|l| l.strip_prefix(&p)).map(|s| s.trim().to_string()).unwrap_or_default()
    }
}

fn run(prog: cobolt_ast::program::Program, class: &str, props: &[(&str, &str)]) -> Run {
    let (_event_tx, event_rx) = mpsc::channel::<FormEvent>();
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
    let deadline = Instant::now() + Duration::from_secs(20);
    while !handle.is_finished() {
        assert!(Instant::now() < deadline, "the program never finished");
        thread::sleep(Duration::from_millis(10));
    }
    let millis = started.elapsed().as_millis();
    Run { lines: display_rx.try_iter().map(|l| l.trim_end().to_string()).collect(), millis }
}

fn props<'a>(f: &'a Fake, allow: &'a str, own: &[(&'a str, &'a str)]) -> Vec<(&'a str, &'a str)> {
    let mut v = vec![
        ("Connection", f.conn.name.as_str()),
        ("Mode", "Async"),
        ("AllowWrite", allow),
        ("TimeoutMs", "5000"),
        ("StartTimeoutMs", "10000"),
    ];
    v.extend_from_slice(own);
    v
}

/// The end-of-test summary (GOLDEN RULE #7).
fn report(test: &str, cases: &[(String, String, u128)]) {
    eprintln!("\n  ══ {test} ══");
    for (case, outcome, ms) in cases {
        eprintln!("    {case:<46} {ms:>5} ms   {outcome}");
    }
    eprintln!("    {} cases, all passed", cases.len());
}

/// `method` on `class`, mutating, with AllowWrite off: refused, nothing sent,
/// the server never started.
fn refused(tag: &str, server: &'static str, tool: &str, class: &str, own: &[(&str, &str)], setup: &str) -> (String, String, u128) {
    let f = fake(tag, server, tool, text("{}"));
    let r = run(program(setup, ""), class, &props(&f, "false", own));
    assert_eq!(r.events(), ["onError"], "{class} {tag}: {:?}", r.lines);
    assert!(r.line("ERROR").contains("AllowWrite is off"), "{}", r.line("ERROR"));
    assert!(!f.started(), "{class} {tag}: a refused write must not start the server");
    (format!("{class}: {tag} with AllowWrite off"), "refused, server not started".into(), r.millis)
}

#[test]
fn knowledge_base_query_and_list() {
    let _turn = TURN.lock().unwrap_or_else(|e| e.into_inner());
    let mut cases = Vec::new();

    // Query: passages arrive as JSON objects one after another, not an array.
    let passages = concat!(
        r#"{"content": {"type": "TEXT", "text": "Refunds take 14 days."}, "location": {"type": "S3", "s3Location": {"uri": "s3://docs/refunds.pdf"}}, "score": 0.82}"#,
        "\n\n",
        r#"{"content": {"type": "TEXT", "text": "Keep the receipt."}, "location": {"type": "WEB", "webLocation": {"url": "https://example.com/faq"}}, "score": 0.41}"#
    );
    let f = fake("kbq", "bedrock-kb", "QueryKnowledgeBases", text(passages));
    let own = format!("{}{}{}", field("TEXT1", 1, "Text"), field("SOURCE2", 2, "Source"), field("SCORE1", 1, "Score"));
    let r = run(
        program(&invoke("Query", &["", "refund policy"]), &own),
        "AwsKnowledgeBase",
        &props(&f, "false", &[("KnowledgeBaseId", "KB12345678"), ("MaxResults", "5")]),
    );
    assert_eq!(r.events(), ["onQueried", "onComplete"], "{:?}", r.lines);
    assert_eq!(r.line("ROWS"), "2");
    assert_eq!(r.line("TEXT1"), "Refunds take 14 days.");
    assert_eq!(r.line("SOURCE2"), "https://example.com/faq");
    assert_eq!(r.line("SCORE1"), "0.82");
    let sent = f.sent();
    assert_eq!(sent[0]["name"], "QueryKnowledgeBases");
    assert_eq!(sent[0]["arguments"], json!({"knowledge_base_id": "KB12345678", "query": "refund policy", "number_of_results": 5}));
    cases.push(("AwsKnowledgeBase.Query(\"\", text)".to_string(), "2 passages: Text, Source (S3 and web), Score".to_string(), r.millis));
    drop(f);

    // ListKnowledgeBases: an object keyed by id becomes a row per entry.
    let listed = r#"{"KB12345678": {"name": "Support docs", "description": "FAQ and policies", "type": "VECTOR", "data_sources": []}}"#;
    let f = fake("kbl", "bedrock-kb", "ListKnowledgeBases", text(listed));
    let own = format!("{}{}", field("ID", 1, "Id"), field("NAME", 1, "Name"));
    let r = run(program(&invoke("ListKnowledgeBases", &[]), &own), "AwsKnowledgeBase", &props(&f, "false", &[]));
    assert_eq!(r.events(), ["onKnowledgeBasesListed", "onComplete"], "{:?}", r.lines);
    assert_eq!((r.line("ROWS"), r.line("ID"), r.line("NAME")), ("1".into(), "KB12345678".into(), "Support docs".into()));
    assert_eq!(f.sent()[0]["arguments"], json!({}));
    cases.push(("AwsKnowledgeBase.ListKnowledgeBases()".to_string(), "1 row: Id, Name".to_string(), r.millis));
    report("AwsKnowledgeBase", &cases);
}

#[test]
fn agentcore_invoke_keeps_the_session_and_reports_failures() {
    let _turn = TURN.lock().unwrap_or_else(|e| e.into_inner());
    let mut cases = Vec::new();
    let arn = [("RuntimeArn", "arn:aws:bedrock-agentcore:eu-west-1:111122223333:runtime/helper-abc")];
    cases.push(refused("invoke", "agentcore", "invoke_agent_runtime", "AwsAgentCore", &arn, &invoke("Invoke", &["", "Hello"])));

    let ok = r#"{"status": "success", "runtime_session_id": "sess-0123456789abcdef0123456789abcdef", "content_type": "application/json", "response_body": "Hi! How can I help?", "message": "Agent runtime invoked"}"#;
    let f = fake("invoke-ok", "agentcore", "invoke_agent_runtime", text(ok));
    let own = format!("{}{}", prop("BODY", "ResponseBody"), prop("SESSION", "SessionId"));
    let r = run(program(&invoke("Invoke", &["", "Hello \"there\""]), &own), "AwsAgentCore", &props(&f, "true", &arn));
    assert_eq!(r.events(), ["onInvoked", "onComplete"], "{:?}", r.lines);
    assert_eq!(r.line("BODY"), "Hi! How can I help?");
    assert_eq!(r.line("SESSION"), "sess-0123456789abcdef0123456789abcdef");
    let a = &f.sent()[0]["arguments"];
    assert_eq!(a["agent_runtime_arn"], arn[0].1);
    assert_eq!(a["payload"], r#"{"prompt":"Hello \"there\""}"#, "the prompt goes as JSON inside a string");
    assert!(a.get("runtime_session_id").is_none(), "no session yet: the key is left out");
    cases.push(("AwsAgentCore.Invoke(\"\", prompt)".to_string(), "reply in ResponseBody, SessionId captured".to_string(), r.millis));
    drop(f);

    // A failure the server reports as data, not as a tool error.
    let failed = r#"{"status": "error", "message": "Agent runtime not found", "error_type": "ResourceNotFoundException", "error_code": "404"}"#;
    let f = fake("invoke-err", "agentcore", "invoke_agent_runtime", text(failed));
    let r = run(program(&invoke("Invoke", &["", "Hello", "sess-0123456789abcdef0123456789abcdef"]), ""), "AwsAgentCore", &props(&f, "true", &arn));
    assert_eq!(r.events(), ["onError"], "{:?}", r.lines);
    assert_eq!(r.line("ERROR"), "Agent runtime not found");
    assert_eq!(f.sent()[0]["arguments"]["runtime_session_id"], "sess-0123456789abcdef0123456789abcdef");
    cases.push(("AwsAgentCore.Invoke, {status: error}".to_string(), "onError with the server's message".to_string(), r.millis));
    report("AwsAgentCore", &cases);
}

#[test]
fn agent_memory_records_and_retrieves() {
    let _turn = TURN.lock().unwrap_or_else(|e| e.into_inner());
    let mut cases = Vec::new();
    let own_props = [("MemoryId", "mem-1"), ("ActorId", "user-42"), ("Namespace", "/facts/user-42"), ("TopK", "3")];
    cases.push(refused("record", "agentcore", "memory_create_event", "AwsAgentMemory", &own_props, &invoke("RecordEvent", &["", "", "", "I like tea"])));

    let created = r#"{"status": "success", "message": "Event created", "event": {"eventId": "ev-77", "memoryId": "mem-1"}}"#;
    let f = fake("record-ok", "agentcore", "memory_create_event", text(created));
    let r = run(
        program(&invoke("RecordEvent", &["", "", "", "I like tea"]), &prop("EVENT-ID", "EventId")),
        "AwsAgentMemory",
        &props(&f, "true", &own_props),
    );
    assert_eq!(r.events(), ["onEventRecorded", "onComplete"], "{:?}", r.lines);
    assert_eq!(r.line("EVENT-ID"), "ev-77");
    assert_eq!(
        f.sent()[0]["arguments"],
        json!({"memory_id": "mem-1", "actor_id": "user-42", "payload": [{"conversational": {"content": {"text": "I like tea"}, "role": "USER"}}]})
    );
    cases.push(("AwsAgentMemory.RecordEvent(…, text)".to_string(), "EventId ev-77; role USER by default".to_string(), r.millis));
    drop(f);

    let records = r#"{"status": "success", "message": "2 records", "memory_records": [{"memoryRecordId": "r1", "content": {"text": "Likes tea"}, "score": 0.91}, {"memoryRecordId": "r2", "content": {"text": "Lives in Porto"}, "score": 0.33}], "next_token": null}"#;
    let f = fake("retrieve", "agentcore", "memory_retrieve_records", text(records));
    let r = run(
        program(&invoke("Retrieve", &["", "", "drinks"]), &format!("{}{}", field("TEXT1", 1, "Text"), field("ID2", 2, "Id"))),
        "AwsAgentMemory",
        &props(&f, "false", &own_props),
    );
    assert_eq!(r.events(), ["onRetrieved", "onComplete"], "{:?}", r.lines);
    assert_eq!((r.line("ROWS"), r.line("TEXT1"), r.line("ID2")), ("2".into(), "Likes tea".into(), "r2".into()));
    assert_eq!(
        f.sent()[0]["arguments"],
        json!({"memory_id": "mem-1", "namespace": "/facts/user-42", "search_query": "drinks", "top_k": 3})
    );
    cases.push(("AwsAgentMemory.Retrieve(\"\", \"\", query)".to_string(), "2 rows: Id, Text, Score".to_string(), r.millis));
    report("AwsAgentMemory", &cases);
}

#[test]
fn s3_tables_list_query_and_append() {
    let _turn = TURN.lock().unwrap_or_else(|e| e.into_inner());
    let mut cases = Vec::new();
    let bucket = "arn:aws:s3tables:eu-west-1:111122223333:bucket/analytics";
    let own_props = [("TableBucketArn", bucket), ("Namespace", "sales"), ("TableName", "orders")];
    cases.push(refused("append", "s3tables", "append_rows_to_table", "AwsS3Tables", &own_props, &invoke("AppendRows", &["", r#"[{"id": 3}]"#])));

    let listed = r#"{"tables": [{"namespace": ["sales"], "name": "orders", "type": "customer", "table_arn": "arn:t/1"}, {"namespace": ["ops"], "name": "events", "type": "customer", "table_arn": "arn:t/2"}, {"namespace": ["sales"], "name": "refunds", "type": "customer", "table_arn": "arn:t/3"}], "total_count": 3}"#;
    let f = fake("list", "s3tables", "list_tables", text(listed));
    let r = run(program(&invoke("ListTables", &[]), &field("NAME2", 2, "Name")), "AwsS3Tables", &props(&f, "false", &own_props));
    assert_eq!(r.events(), ["onTablesListed", "onComplete"], "{:?}", r.lines);
    assert_eq!((r.line("ROWS"), r.line("NAME2")), ("2".into(), "refunds".into()), "the Namespace property keeps sales only");
    assert_eq!(f.sent()[0]["arguments"], json!({"region_name": "eu-west-1"}));
    cases.push(("AwsS3Tables.ListTables() with Namespace sales".to_string(), "2 of 3 tables kept".to_string(), r.millis));
    drop(f);

    let result = r#"{"columns": ["order_id", "total"], "rows": [[1, 19.9], [2, 5.0]]}"#;
    let f = fake("query", "s3tables", "query_database", text(result));
    let r = run(
        program(&invoke("Query", &["SELECT order_id, total FROM orders"]), &format!("{}{}", field("ID2", 2, "order_id"), field("TOTAL1", 1, "total"))),
        "AwsS3Tables",
        &props(&f, "false", &own_props),
    );
    assert_eq!(r.events(), ["onQueried", "onComplete"], "{:?}", r.lines);
    assert_eq!((r.line("ROWS"), r.line("ID2"), r.line("TOTAL1")), ("2".into(), "2".into(), "19.9".into()));
    assert_eq!(
        f.sent()[0]["arguments"],
        json!({"warehouse": bucket, "region": "eu-west-1", "namespace": "sales", "uri": "https://s3tables.eu-west-1.amazonaws.com/iceberg", "query": "SELECT order_id, total FROM orders"})
    );
    cases.push(("AwsS3Tables.Query(sql)".to_string(), "2 rows named by the columns".to_string(), r.millis));
    drop(f);

    let f = fake("append-ok", "s3tables", "append_rows_to_table", text(r#"{"status": "success", "rows_appended": 2}"#));
    let r = run(
        program(&invoke("AppendRows", &["", r#"[{"order_id": 3}, {"order_id": 4}]"#]), &prop("APPENDED", "RowsAppended")),
        "AwsS3Tables",
        &props(&f, "true", &own_props),
    );
    assert_eq!(r.events(), ["onRowsAppended", "onComplete"], "{:?}", r.lines);
    assert_eq!(r.line("APPENDED"), "2");
    assert_eq!(f.sent()[0]["arguments"]["rows"], json!([{"order_id": 3}, {"order_id": 4}]));
    assert_eq!(f.sent()[0]["arguments"]["table_name"], "orders");
    cases.push(("AwsS3Tables.AppendRows(\"\", rowsJson)".to_string(), "RowsAppended 2".to_string(), r.millis));
    report("AwsS3Tables", &cases);
}

#[test]
fn glue_jobs_crawlers_and_schemas() {
    let _turn = TURN.lock().unwrap_or_else(|e| e.into_inner());
    let mut cases = Vec::new();
    let own_props = [("JobName", "nightly-etl"), ("DatabaseName", "sales"), ("TableName", "orders"), ("CrawlerName", "raw-crawler")];
    cases.push(refused("start", "dataprocessing", "manage_aws_glue_jobs", "AwsGlue", &own_props, &invoke("StartJobRun", &[])));
    cases.push(refused("crawler", "dataprocessing", "manage_aws_glue_crawlers", "AwsGlue", &own_props, &invoke("StartCrawler", &[])));

    // Each answer is two text blocks: a message, then the data.
    let two = |msg: &str, data: &str| json!({"content": [{"type": "text", "text": msg}, {"type": "text", "text": data}], "isError": false});
    let f = fake("start-ok", "dataprocessing", "manage_aws_glue_jobs", two("Successfully started job run", r#"{"job_name": "nightly-etl", "job_run_id": "jr_abc", "operation": "start-job-run"}"#));
    let r = run(program(&invoke("StartJobRun", &["", r#"{"--day": "2026-10-07"}"#]), &prop("RUN", "JobRunId")), "AwsGlue", &props(&f, "true", &own_props));
    assert_eq!(r.events(), ["onJobStarted", "onComplete"], "{:?}", r.lines);
    assert_eq!(r.line("RUN"), "jr_abc");
    assert_eq!(f.sent()[0]["arguments"], json!({"operation": "start-job-run", "job_name": "nightly-etl", "job_arguments": {"--day": "2026-10-07"}}));
    cases.push(("AwsGlue.StartJobRun(\"\", argsJson)".to_string(), "JobRunId jr_abc".to_string(), r.millis));
    drop(f);

    let f = fake("get-run", "dataprocessing", "manage_aws_glue_jobs", two("Successfully retrieved job run", r#"{"job_name": "nightly-etl", "job_run_id": "jr_abc", "job_run_details": {"Id": "jr_abc", "JobRunState": "SUCCEEDED"}, "operation": "get-job-run"}"#));
    let r = run(program(&invoke("GetJobRun", &["", "jr_abc"]), &prop("STATE", "State")), "AwsGlue", &props(&f, "false", &own_props));
    assert_eq!(r.events(), ["onJobRun", "onComplete"], "{:?}", r.lines);
    assert_eq!(r.line("STATE"), "SUCCEEDED");
    assert_eq!(f.sent()[0]["arguments"], json!({"operation": "get-job-run", "job_name": "nightly-etl", "job_run_id": "jr_abc"}));
    cases.push(("AwsGlue.GetJobRun(\"\", runId)".to_string(), "State SUCCEEDED".to_string(), r.millis));
    drop(f);

    let table = r#"{"database_name": "sales", "table_name": "orders", "storage_descriptor": {"Columns": [{"Name": "order_id", "Type": "bigint"}, {"Name": "total", "Type": "double", "Comment": "EUR"}]}, "operation": "get"}"#;
    let f = fake("schema", "dataprocessing", "manage_aws_glue_tables", two("Successfully retrieved table", table));
    let r = run(
        program(&invoke("GetTableSchema", &[]), &format!("{}{}", field("NAME2", 2, "Name"), field("TYPE1", 1, "Type"))),
        "AwsGlue",
        &props(&f, "false", &own_props),
    );
    assert_eq!(r.events(), ["onTableSchema", "onComplete"], "{:?}", r.lines);
    assert_eq!((r.line("ROWS"), r.line("NAME2"), r.line("TYPE1")), ("2".into(), "total".into(), "bigint".into()));
    assert_eq!(f.sent()[0]["arguments"], json!({"operation": "get-table", "database_name": "sales", "table_name": "orders"}));
    cases.push(("AwsGlue.GetTableSchema()".to_string(), "2 columns: Name, Type, Comment".to_string(), r.millis));
    drop(f);

    // The server's own read-only refusal is a tool error.
    let denied = json!({"content": [{"type": "text", "text": "Operation start-crawler is not allowed without write access"}], "isError": true});
    let f = fake("server-ro", "dataprocessing", "manage_aws_glue_crawlers", denied);
    let r = run(program(&invoke("StartCrawler", &[]), ""), "AwsGlue", &props(&f, "true", &own_props));
    assert_eq!(r.events(), ["onError"], "{:?}", r.lines);
    assert!(r.line("ERROR").contains("not allowed without write access"), "{}", r.line("ERROR"));
    cases.push(("AwsGlue.StartCrawler, server refuses".to_string(), "onError with its message".to_string(), r.millis));
    report("AwsGlue", &cases);
}

/// The servers that gate writes start read-only unless a control allows them
/// (R25), and S3 Tables asks uvx for the Python it needs.
#[test]
fn the_shipped_servers_start_read_only_unless_writes_are_allowed() {
    let routes = Routes::shipped();
    let none = |_: &str| String::new();
    let ctx = Context { args: &[], prop: &none, connection: &none };
    for id in ["s3tables", "dataprocessing"] {
        let s = &routes.servers[id];
        let ro = launch_for(s, &ctx, false, "dev", "eu-west-1").unwrap();
        let rw = launch_for(s, &ctx, true, "dev", "eu-west-1").unwrap();
        assert!(!ro.args.iter().any(|a| a == "--allow-write"), "{id}: {:?}", ro.args);
        assert_eq!(rw.args.last().map(String::as_str), Some("--allow-write"), "{id}");
    }
    let s3 = launch_for(&routes.servers["s3tables"], &ctx, false, "", "").unwrap();
    assert_eq!(&s3.args[..2], ["--python", ">=3.11"]);
    let ac = launch_for(&routes.servers["agentcore"], &ctx, false, "", "").unwrap();
    assert_eq!(ac.env, [("AGENTCORE_ENABLE_TOOLS".to_string(), "runtime,memory".to_string())]);
}
