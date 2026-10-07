// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! The Delivery C AWS controls — the hosted AWS MCP Server's
//! `aws___run_script`, driven from COBOL against the fake MCP server (spec
//! 078 T-C1…T-C10; AC11, AC15).
//!
//! The fake answers the way AWS's code says the hosted server does: one text
//! item holding the envelope `{status, stdout, stderr, return_value}`. Each
//! case checks the script that was sent — the AWS service and operation, and
//! every COBOL value as a quoted literal, never as script text — what the
//! program reads back, the event order, and the refusals.
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
use cobolt_runtime::aws::routes::Routes;
use cobolt_runtime::{FormEvent, Interpreter};
use serde_json::{json, Value};

static TURN: Mutex<()> = Mutex::new(());

struct Fake {
    dir: PathBuf,
    conn: AwsConnection,
}

impl Drop for Fake {
    fn drop(&mut self) {
        pool::stop(&ServerKey { connection: self.conn.id.clone(), server: "hosted".into() });
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

impl Fake {
    /// The scripts sent, in order.
    fn scripts(&self) -> Vec<String> {
        std::fs::read_to_string(self.dir.join("calls.jsonl"))
            .unwrap_or_default()
            .lines()
            .filter_map(|l| serde_json::from_str::<Value>(l).ok())
            .filter(|c| c["method"] == "tools/call")
            .map(|c| {
                assert_eq!(c["params"]["name"], "aws___run_script");
                c["params"]["arguments"]["code"].as_str().unwrap_or_default().to_owned()
            })
            .collect()
    }
    fn started(&self) -> bool {
        self.dir.join("pids.txt").exists()
    }
}

/// The hosted server's answer: the envelope as JSON text.
fn envelope(return_value: Value) -> Value {
    let text = json!({"status": "success", "stdout": "", "stderr": "", "return_value": return_value}).to_string();
    json!({"content": [{"type": "text", "text": text}], "isError": false})
}

fn failed(error: &str) -> Value {
    let text = json!({"status": "error", "stdout": "", "stderr": "Traceback …", "error": error}).to_string();
    json!({"content": [{"type": "text", "text": text}], "isError": false})
}

/// A connection whose hosted server is the fake, answering `run_script`
/// with `answer`; `tag` names its own folder.
fn fake(tag: &str, answer: Value) -> Fake {
    fake_on(tag, &format!("conn-c-{tag}"), answer)
}

/// The same, on a connection of a given id — a control's secrets belong to
/// its connection and itself, so a sign-in and the calls after it share one.
fn fake_on(tag: &str, conn_id: &str, answer: Value) -> Fake {
    let dir = std::env::temp_dir().join(format!("prc-078c-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let script = json!({
        "pid_file": dir.join("pids.txt").to_string_lossy(),
        "call_log": dir.join("calls.jsonl").to_string_lossy(),
        "tools": [{"name": "aws___run_script", "inputSchema": {"type": "object", "properties": {"code": {"type": "string"}}, "required": ["code"]}}],
        "answers": {"aws___run_script": answer},
    });
    let script_path = dir.join("script.json");
    std::fs::write(&script_path, script.to_string()).unwrap();
    let routes_override = format!(
        "[servers.hosted]\ncommand = '{}'\nargs = ['{}']\n",
        env!("CARGO_BIN_EXE_fake_mcp"),
        script_path.display()
    );
    let conn = AwsConnection {
        id: conn_id.to_owned(),
        name: conn_id.to_owned(),
        profile: "test".into(),
        region: "eu-west-1".into(),
        routes_override,
        ..Default::default()
    };
    connections::publish(vec![conn.clone()]);
    Fake { dir, conn }
}

fn program(setup: &str, on_own: &str) -> cobolt_ast::program::Program {
    let src = format!(
        r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. AWSC.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 COBOL-EVENT-ID    PIC X(30).
       01 COBOL-CONTROL-ID  PIC X(30).
       01 COBOL-QUIT        PIC 9 VALUE 0.
       01 WS-A              PIC X(200).
       01 WS-B              PIC X(200).
       01 WS-C              PIC X(200).
       01 WS-D              PIC X(200).
       01 WS-RC             PIC X(2000).
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

fn field(label: &str, row: u8, name: &str) -> String {
    format!(
        "                       MOVE {row} TO WS-I\n                       MOVE \"{name}\" TO WS-F\n                       INVOKE AWS-1 \"GetField\" USING WS-I WS-F RETURNING WS-RC\n                       DISPLAY \"{label} \" WS-RC\n"
    )
}

fn prop(label: &str, name: &str) -> String {
    format!("                       MOVE AWS-1::{name} TO WS-RC\n                       DISPLAY \"{label} \" WS-RC\n")
}

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
    let mut v = vec![("Connection", f.conn.name.as_str()), ("Mode", "Async"), ("AllowWrite", allow), ("TimeoutMs", "5000"), ("StartTimeoutMs", "10000")];
    v.extend_from_slice(own);
    v
}

fn report(test: &str, cases: &[(String, String, u128)]) {
    eprintln!("\n  ══ {test} ══");
    for (case, outcome, ms) in cases {
        eprintln!("    {case:<52} {ms:>5} ms   {outcome}");
    }
    eprintln!("    {} cases, all passed", cases.len());
}

/// A mutating operation with AllowWrite off: refused, nothing sent.
fn refused(tag: &str, class: &str, own: &[(&str, &str)], setup: &str) -> (String, String, u128) {
    let f = fake(tag, envelope(json!({})));
    let r = run(program(setup, ""), class, &props(&f, "false", own));
    assert_eq!(r.events(), ["onError"], "{class} {tag}: {:?}", r.lines);
    assert!(r.line("ERROR").contains("AllowWrite is off"), "{}", r.line("ERROR"));
    assert!(!f.started(), "{class} {tag}: a refused write must not start the server");
    (format!("{class}: {tag} with AllowWrite off"), "refused, server not started".into(), r.millis)
}

/// The one script sent, checked for its AWS call and its quoted values.
fn sent(f: &Fake, service: &str, operation: &str, literals: &[&str]) -> String {
    let scripts = f.scripts();
    assert_eq!(scripts.len(), 1, "{scripts:?}");
    let code = scripts[0].clone();
    assert!(code.contains(&format!("service_name='{service}', operation_name='{operation}'")), "{code}");
    for l in literals {
        assert!(code.contains(l), "{l} not in:\n{code}");
    }
    assert!(code.contains("region_name=\"eu-west-1\""), "{code}");
    code
}

/// R25 / plan §4: every COBOL value a hosted template takes enters it as a
/// quoted literal, a number or a file's base64 — never as script text.
#[test]
fn no_op_accepts_script_text_from_cobol() {
    let routes = Routes::shipped();
    let mut checked = 0;
    for (name, op) in routes.ops.iter().filter(|(_, o)| o.server == "hosted") {
        let none = |_: &str| Some("token".to_string());
        let input = routes.paste(op.input.as_ref().expect("a hosted op has a script"), &none).unwrap();
        let code = input["code"].as_str().unwrap();
        let mut rest = code;
        while let Some(at) = rest.find('{') {
            let end = at + rest[at..].find('}').unwrap();
            // A nested `{prop:…}` inside `|or:` closes before the outer one.
            let mut close = end;
            let mut depth = 0;
            for (i, c) in rest[at..].char_indices() {
                match c {
                    '{' => depth += 1,
                    '}' => {
                        depth -= 1;
                        if depth == 0 {
                            close = at + i;
                            break;
                        }
                    }
                    _ => {}
                }
            }
            let body = &rest[at + 1..close];
            let last = body.rsplit('|').next().unwrap();
            let from_cobol = body.starts_with("arg:") || body.starts_with("prop:");
            assert!(
                !from_cobol || matches!(last, "quote" | "int"),
                "{name}: the placeholder {{{body}}} puts a COBOL value into the script as text"
            );
            checked += 1;
            rest = &rest[close + 1..];
        }
        assert!(!code.contains("{snippet:"), "{name}: a snippet was not pasted");
    }
    assert!(checked > 0);
    println!("plan §4: {checked} placeholders across the hosted templates, every COBOL value quoted or numeric");
}

#[test]
fn dynamodb_reads_and_writes_plain_json() {
    let _turn = TURN.lock().unwrap_or_else(|e| e.into_inner());
    let mut cases = Vec::new();
    let own = [("TableName", "orders")];
    cases.push(refused("put", "AwsDynamoDB", &own, &invoke("PutItem", &[r#"{"id": 1}"#])));

    // A value that tries to break out of its literal stays inside it.
    let f = fake("get", envelope(json!({"Items": [{"id": 42, "customer": "Ana", "total": 19.9}], "Found": "1"})));
    let r = run(
        program(&invoke("GetItem", &[r#"{"id": 42}"#]), &format!("{}{}{}", prop("FOUND", "Found"), field("CUSTOMER", 1, "customer"), field("TOTAL", 1, "total"))),
        "AwsDynamoDB",
        &props(&f, "false", &own),
    );
    assert_eq!(r.events(), ["onItem", "onComplete"], "{:?}", r.lines);
    assert_eq!((r.line("FOUND"), r.line("CUSTOMER"), r.line("TOTAL")), ("1".into(), "Ana".into(), "19.9".into()));
    sent(&f, "dynamodb", "GetItem", &[r#"TableName="orders""#, r#"json.loads("{\"id\":42}")"#]);
    cases.push(("AwsDynamoDB.GetItem(keyJson)".into(), "Found 1; the item as row 1, plain JSON".into(), r.millis));
    drop(f);

    // A return value the sandbox wrote as JSON TEXT is read just the same.
    let text_value = json!({"Items": [{"id": 1}, {"id": 2}], "Count": 2}).to_string();
    let f = fake("scan", envelope(Value::String(text_value)));
    let r = run(program(&invoke("Scan", &["7"]), &field("ID2", 2, "id")), "AwsDynamoDB", &props(&f, "false", &own));
    assert_eq!(r.events(), ["onScanned", "onComplete"], "{:?}", r.lines);
    assert_eq!((r.line("ROWS"), r.line("ID2")), ("2".into(), "2".into()));
    sent(&f, "dynamodb", "Scan", &["Limit=7"]);
    cases.push(("AwsDynamoDB.Scan(7), return_value as JSON text".into(), "2 rows".into(), r.millis));
    drop(f);

    let breakout = r#"x"); import os; os.remove("y"#;
    let f = fake("query", envelope(json!({"Items": [], "Count": 0})));
    let r = run(program(&invoke("Query", &["customer = :c", r#"{":c": "C-7"}"#, breakout]), ""), "AwsDynamoDB", &props(&f, "false", &own));
    assert_eq!(r.events(), ["onQueried", "onComplete"], "{:?}", r.lines);
    let code = sent(&f, "dynamodb", "Query", &[r#"KeyConditionExpression="customer = :c""#]);
    assert!(code.contains(r#""x\"); import os; os.remove(\"y""#), "the value must stay one quoted literal:\n{code}");
    cases.push(("AwsDynamoDB.Query, a value that tries to escape".into(), "kept inside its literal".into(), r.millis));
    drop(f);

    let f = fake("put-err", failed("ResourceNotFoundException: Requested resource not found"));
    let r = run(program(&invoke("PutItem", &[r#"{"id": 1}"#]), ""), "AwsDynamoDB", &props(&f, "true", &own));
    assert_eq!(r.events(), ["onError"], "{:?}", r.lines);
    assert!(r.line("ERROR").contains("Requested resource not found"), "{}", r.line("ERROR"));
    cases.push(("AwsDynamoDB.PutItem, the script fails".into(), "onError with AWS's message".into(), r.millis));
    report("AwsDynamoDB", &cases);
}

#[test]
fn s3_and_polly_write_bytes_to_files() {
    let _turn = TURN.lock().unwrap_or_else(|e| e.into_inner());
    let mut cases = Vec::new();
    let out_dir = std::env::temp_dir().join(format!("prc-078c-files-{}", std::process::id()));
    std::fs::create_dir_all(&out_dir).unwrap();
    let own = [("Bucket", "reports")];
    cases.push(refused("delete", "AwsS3", &own, &invoke("DeleteObject", &["a.txt"])));

    let f = fake("list", envelope(json!({"Contents": [{"Key": "a.txt", "Size": 5, "LastModified": "2026-10-07 10:00:00+00:00"}]})));
    let r = run(program(&invoke("List", &["2026/"]), &field("KEY", 1, "Key")), "AwsS3", &props(&f, "false", &own));
    assert_eq!((r.events(), r.line("KEY")), (vec!["onListed", "onComplete"], "a.txt".into()), "{:?}", r.lines);
    sent(&f, "s3", "ListObjectsV2", &[r#"Bucket="reports""#, r#"Prefix="2026/""#]);
    cases.push(("AwsS3.List(prefix)".into(), "1 row: Key, Size, LastModified".into(), r.millis));
    drop(f);

    let to_file = out_dir.join("copy.bin");
    let f = fake("get", envelope(json!({"Base64": "aGVsbG8=", "Text": "hello", "ContentType": "text/plain"})));
    let r = run(
        program(&invoke("GetObject", &["a.txt", to_file.to_str().unwrap()]), &prop("BODY", "ResponseBody")),
        "AwsS3",
        &props(&f, "false", &own),
    );
    assert_eq!(r.line("BODY"), "hello", "{:?}", r.lines);
    assert_eq!(std::fs::read(&to_file).unwrap(), b"hello");
    cases.push(("AwsS3.GetObject(key, toFile)".into(), "text in ResponseBody, bytes in the file".into(), r.millis));
    drop(f);

    let from_file = out_dir.join("upload.txt");
    std::fs::write(&from_file, b"bytes!").unwrap();
    let f = fake("put", envelope(json!({"ETag": "\"abc\""})));
    let r = run(program(&invoke("PutObject", &["b.txt", "", from_file.to_str().unwrap()]), ""), "AwsS3", &props(&f, "true", &own));
    assert_eq!(r.events(), ["onObjectPut", "onComplete"], "{:?}", r.lines);
    sent(&f, "s3", "PutObject", &[r#"f = "Ynl0ZXMh""#]);
    cases.push(("AwsS3.PutObject(key, \"\", fromFile)".into(), "the file's bytes sent as base64".into(), r.millis));
    drop(f);

    // Polly: the audio lands in OutputFile, which SavedFile then names.
    let audio = out_dir.join("hello.mp3");
    let f = fake("polly", envelope(json!({"Base64": "SUQzBAA=", "ContentType": "audio/mpeg", "Characters": 5})));
    let r = run(
        program(&invoke("Synthesize", &["Hello"]), &format!("{}{}", prop("SAVED", "SavedFile"), prop("CHARS", "Characters"))),
        "AwsPolly",
        &props(&f, "false", &[("VoiceId", "Camila"), ("OutputFormat", "mp3"), ("OutputFile", audio.to_str().unwrap())]),
    );
    assert_eq!(r.events(), ["onSynthesized", "onComplete"], "{:?}", r.lines);
    assert_eq!(r.line("SAVED"), audio.to_str().unwrap());
    assert_eq!(r.line("CHARS"), "5");
    assert_eq!(std::fs::read(&audio).unwrap(), b"ID3\x04\x00");
    sent(&f, "polly", "SynthesizeSpeech", &[r#"Text="Hello""#, r#"VoiceId="Camila""#]);
    cases.push(("AwsPolly.Synthesize(text)".into(), "5 bytes of audio in OutputFile".into(), r.millis));
    let _ = std::fs::remove_dir_all(&out_dir);
    report("AwsS3, AwsPolly", &cases);
}

#[test]
fn vision_and_language_controls_return_rows() {
    let _turn = TURN.lock().unwrap_or_else(|e| e.into_inner());
    let mut cases = Vec::new();
    let img = std::env::temp_dir().join(format!("prc-078c-img-{}.png", std::process::id()));
    std::fs::write(&img, [0x89, b'P', b'N', b'G']).unwrap();

    let f = fake("labels", envelope(json!({"Labels": [{"Name": "Dog", "Confidence": 98.1}, {"Name": "Pet", "Confidence": 97.0}]})));
    let r = run(program(&invoke("DetectLabels", &[img.to_str().unwrap()]), &field("LABEL1", 1, "Name")), "AwsRekognition", &props(&f, "false", &[("MinConfidence", "80")]));
    assert_eq!((r.events(), r.line("ROWS"), r.line("LABEL1")), (vec!["onLabels", "onComplete"], "2".into(), "Dog".into()), "{:?}", r.lines);
    sent(&f, "rekognition", "DetectLabels", &[r#"_source("b64:iVBORw==")"#, "MinConfidence=80"]);
    cases.push(("AwsRekognition.DetectLabels(local file)".into(), "2 labels; the file sent as base64".into(), r.millis));
    drop(f);
    let _ = std::fs::remove_file(&img);

    let f = fake("textract", envelope(json!({"Rows": [{"Kind": "FIELD", "Key": "Name:", "Value": "Ana", "Table": 0, "Row": 0, "Column": 0, "Confidence": 99}, {"Kind": "CELL", "Key": "", "Value": "42", "Table": 1, "Row": 2, "Column": 3, "Confidence": 95}]})));
    let r = run(
        program(&invoke("AnalyzeDocument", &["s3://inbox/form.pdf"]), &format!("{}{}", field("VALUE1", 1, "Value"), field("COL2", 2, "Column"))),
        "AwsTextract",
        &props(&f, "false", &[]),
    );
    assert_eq!((r.events(), r.line("VALUE1"), r.line("COL2")), (vec!["onDocumentAnalyzed", "onComplete"], "Ana".into(), "3".into()), "{:?}", r.lines);
    sent(&f, "textract", "AnalyzeDocument", &[r#"_source("s3://inbox/form.pdf")"#]);
    cases.push(("AwsTextract.AnalyzeDocument(s3://…)".into(), "a field and a cell as rows".into(), r.millis));
    drop(f);

    let f = fake("sentiment", envelope(json!({"Sentiment": "POSITIVE", "Scores": [{"Positive": 0.97, "Negative": 0.01, "Neutral": 0.02, "Mixed": 0.0}]})));
    let r = run(program(&invoke("DetectSentiment", &["I love it"]), &format!("{}{}", prop("SENTIMENT", "Sentiment"), field("POS", 1, "Positive"))), "AwsComprehend", &props(&f, "false", &[("LanguageCode", "en")]));
    assert_eq!((r.events(), r.line("SENTIMENT"), r.line("POS")), (vec!["onSentiment", "onComplete"], "POSITIVE".into(), "0.97".into()), "{:?}", r.lines);
    sent(&f, "comprehend", "DetectSentiment", &[r#"Text="I love it""#, r#"LanguageCode="en""#]);
    cases.push(("AwsComprehend.DetectSentiment(text)".into(), "POSITIVE, scores as row 1".into(), r.millis));
    drop(f);

    let f = fake("vectors", envelope(json!({"vectors": [{"key": "doc-7", "distance": 0.12, "metadata": {"title": "Refunds"}}]})));
    let r = run(program(&invoke("QueryVectors", &["", "[0.1, 0.2, 0.3]", "3"]), &field("KEY", 1, "Key")), "AwsS3Vectors", &props(&f, "false", &[("VectorBucketName", "kb-vectors"), ("IndexName", "docs")]));
    assert_eq!((r.events(), r.line("KEY")), (vec!["onVectorsQueried", "onComplete"], "doc-7".into()), "{:?}", r.lines);
    sent(&f, "s3vectors", "QueryVectors", &[r#"indexName="docs""#, "topK=3", r#"json.loads("[0.1,0.2,0.3]")"#]);
    cases.push(("AwsS3Vectors.QueryVectors(\"\", vector, 3)".into(), "1 row: Key, Distance, Metadata".into(), r.millis));
    drop(f);

    let f = fake("ec2", envelope(json!({"Instances": [{"InstanceId": "i-1", "Name": "web", "State": "running", "Type": "t3.micro", "PublicIp": "", "PrivateIp": "10.0.0.5"}]})));
    let r = run(program(&invoke("Describe", &[]), &field("STATE", 1, "State")), "AwsEC2", &props(&f, "false", &[("InstanceIds", "i-1, i-2")]));
    assert_eq!((r.events(), r.line("STATE")), (vec!["onDescribed", "onComplete"], "running".into()), "{:?}", r.lines);
    sent(&f, "ec2", "DescribeInstances", &[r#""i-1, i-2".split(',')"#]);
    cases.push(("AwsEC2.Describe() with InstanceIds".into(), "1 row: InstanceId, State, …".into(), r.millis));
    drop(f);
    cases.push(refused("stop", "AwsEC2", &[("InstanceIds", "i-1")], &invoke("Stop", &[])));
    report("AwsRekognition, AwsTextract, AwsComprehend, AwsS3Vectors, AwsEC2", &cases);
}

/// Q6: a sign-in's tokens never reach a property, a row, ResultJson, a
/// Verbose line or a file — and still reach AWS when a later call needs one.
#[test]
fn no_cognito_token_reaches_a_property_or_disk() {
    let _turn = TURN.lock().unwrap_or_else(|e| e.into_inner());
    let token = "eyJraWQiOiJ0ZXN0IiwiYWxnIjoiUlMyNTYifQ.PLANTED-ACCESS-TOKEN.sig";
    let id_token = "eyJ0eXAiOiJKV1QifQ.PLANTED-ID-TOKEN.sig";
    let own = [("ClientId", "app-client-1"), ("Verbose", "true")];

    // SignUp needs AllowWrite.
    let refused_case = refused("signup", "AwsCognito", &own, &invoke("SignUp", &["ana", "S3cret!pass", "ana@example.com"]));

    let f = fake_on(
        "signin",
        "conn-c-cognito",
        envelope(json!({"SignedIn": "1", "UserName": "ana", "Challenge": "", "Secret": {"AccessToken": token, "IdToken": id_token, "RefreshToken": "PLANTED-REFRESH"}})),
    );
    // Read back every property the control has, and every row, after the sign-in.
    let mut dump = String::new();
    for p in cobolt_forms::aws_catalog::by_name("AwsCognito").unwrap().runtime.iter().chain(["Connection", "ClientId"].iter()) {
        dump.push_str(&prop(&format!("PROP-{p}"), p));
    }
    dump.push_str("                       MOVE 1 TO WS-I\n                       INVOKE AWS-1 \"GetRow\" USING WS-I RETURNING WS-RC\n                       DISPLAY \"ROW1 \" WS-RC\n");
    let r = run(program(&invoke("SignIn", &["ana", "S3cret!pass"]), &dump), "AwsCognito", &props(&f, "false", &own));
    assert_eq!(r.events(), ["onSignedIn", "onComplete"], "{:?}", r.lines);
    assert_eq!((r.line("PROP-SignedIn"), r.line("PROP-UserName")), ("1".into(), "ana".into()));
    let everything = r.lines.join("\n");
    for planted in ["PLANTED-ACCESS-TOKEN", "PLANTED-ID-TOKEN", "PLANTED-REFRESH", "S3cret!pass"] {
        assert!(!everything.contains(planted), "{planted} reached the program or its output:\n{everything}");
    }
    assert!(everything.contains("arguments not shown"), "Verbose must say the request was withheld:\n{everything}");
    // …and no file the run could have written holds one: the working
    // directory and the temp directory, the fake's own log excepted.
    let mut leaked = Vec::new();
    for dir in [std::env::current_dir().unwrap(), std::env::temp_dir()] {
        for e in std::fs::read_dir(&dir).unwrap().flatten() {
            let p = e.path();
            if p.is_file() && !p.starts_with(&f.dir) {
                if let Ok(text) = std::fs::read(&p) {
                    if String::from_utf8_lossy(&text).contains("PLANTED-ACCESS-TOKEN") {
                        leaked.push(p.display().to_string());
                    }
                }
            }
        }
    }
    assert!(leaked.is_empty(), "a token was written to disk: {leaked:?}");
    let signin_ms = r.millis;
    drop(f);

    // The token still reaches AWS when a later call needs it — inside the script.
    let f = fake_on("attr", "conn-c-cognito", envelope(json!({"Value": "ana@example.com"})));
    let r = run(program(&invoke("GetAttribute", &["email"]), &prop("VALUE", "ResponseBody")), "AwsCognito", &props(&f, "false", &own));
    assert_eq!((r.events(), r.line("VALUE")), (vec!["onAttribute", "onComplete"], "ana@example.com".into()), "{:?}", r.lines);
    assert!(f.scripts()[0].contains(&format!("AccessToken=\"{token}\"")), "the access token must reach GetUser");
    assert!(!r.lines.join("\n").contains("PLANTED-ACCESS-TOKEN"), "Verbose printed the token");
    let attr_ms = r.millis;
    drop(f);

    // SignOut forgets the tokens here, whatever AWS answers.
    let f = fake_on("signout", "conn-c-cognito", failed("NotAuthorizedException: Access Token has been revoked"));
    let r = run(program(&invoke("SignOut", &[]), ""), "AwsCognito", &props(&f, "false", &own));
    assert_eq!(r.events(), ["onError"], "{:?}", r.lines);
    drop(f);
    let f = fake_on("after", "conn-c-cognito", envelope(json!({"Value": "x"})));
    let r = run(program(&invoke("GetAttribute", &["email"]), ""), "AwsCognito", &props(&f, "false", &own));
    assert_eq!(r.events(), ["onError"], "{:?}", r.lines);
    assert!(r.line("ERROR").contains("not signed in"), "{}", r.line("ERROR"));
    assert!(!f.started(), "nothing is sent for a control that holds no token");
    report(
        "AwsCognito",
        &[
            refused_case,
            ("AwsCognito.SignIn(user, password)".into(), "SignedIn 1; no token or password in props, rows, Verbose or disk".into(), signin_ms),
            ("AwsCognito.GetAttribute(email)".into(), "the kept token reaches AWS inside the script".into(), attr_ms),
            ("AwsCognito.SignOut, then GetAttribute".into(), "tokens forgotten; refused, nothing sent".into(), r.millis),
        ],
    );
}
