// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Spec 072 — `AgentObject` tool calling, end to end.
//!
//! A real COBOL event loop asks a question of an `AgentObject` pointed at a
//! scripted local model server (a `TcpListener` on `127.0.0.1`, no network).
//! The server answers each round from a script and keeps every request body,
//! so each test can check both what the program saw and what went on the wire.
//!
//! Each test prints one summary block: the protocol, the rounds and tool calls
//! the loop made, and the wall time of the whole question.

use std::io::{Read, Write};
use std::net::TcpListener;
use std::path::Path;
use std::sync::{mpsc, Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use cobolt_lexer::{tokenize, SourceFormat};
use cobolt_parser::{parse, Severity};
use cobolt_runtime::agent_runtime::{body_for, AskRequest, Protocol};
use cobolt_runtime::indexed::{status, KeySpec, OpenMode};
use cobolt_runtime::Interpreter;

// ── The scripted model server ──────────────────────────────────────────────────

type Script = Box<dyn Fn(usize, &str) -> (u16, String) + Send>;

/// Serve every connection from `script(round, request_body)`. Returns the port
/// and the request bodies received, in order.
fn model_server(script: Script) -> (u16, Arc<Mutex<Vec<String>>>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let seen = Arc::new(Mutex::new(Vec::new()));
    let log = seen.clone();
    thread::spawn(move || {
        for (round, stream) in listener.incoming().enumerate() {
            let Ok(mut stream) = stream else { break };
            let body = read_request_body(&mut stream);
            log.lock().unwrap().push(body.clone());
            let (status, reply) = script(round, &body);
            let resp = format!(
                "HTTP/1.1 {status} X\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                reply.len(),
                reply
            );
            let _ = stream.write_all(resp.as_bytes());
            let _ = stream.flush();
        }
    });
    (port, seen)
}

fn read_request_body(stream: &mut std::net::TcpStream) -> String {
    let _ = stream.set_read_timeout(Some(Duration::from_secs(3)));
    let mut buf = Vec::new();
    let mut chunk = [0u8; 4096];
    let header_end = loop {
        match stream.read(&mut chunk) {
            Ok(0) | Err(_) => return String::new(),
            Ok(n) => buf.extend_from_slice(&chunk[..n]),
        }
        if let Some(i) = buf.windows(4).position(|w| w == b"\r\n\r\n") {
            break i + 4;
        }
    };
    let head = String::from_utf8_lossy(&buf[..header_end]).to_ascii_lowercase();
    let len = head
        .lines()
        .find_map(|l| l.strip_prefix("content-length:"))
        .and_then(|v| v.trim().parse::<usize>().ok())
        .unwrap_or(0);
    while buf.len() < header_end + len {
        match stream.read(&mut chunk) {
            Ok(0) | Err(_) => break,
            Ok(n) => buf.extend_from_slice(&chunk[..n]),
        }
    }
    String::from_utf8_lossy(&buf[header_end..]).into_owned()
}

// ── Replies in each provider's shape ───────────────────────────────────────────

fn openai_call(id: &str, name: &str, args: &str) -> String {
    serde_json::json!({
        "choices": [{"message": {"content": null, "tool_calls": [
            {"id": id, "type": "function", "function": {"name": name, "arguments": args}}
        ]}}],
        "usage": {"prompt_tokens": 50, "completion_tokens": 10}
    })
    .to_string()
}

fn openai_text(text: &str) -> String {
    serde_json::json!({
        "choices": [{"message": {"content": text}}],
        "usage": {"prompt_tokens": 80, "completion_tokens": 6}
    })
    .to_string()
}

// ── The COBOL side ─────────────────────────────────────────────────────────────

/// An event loop that asks one question and reports what happened. `setup`
/// runs before the `Ask`; `on_tool_call` is the body of the `onToolCall` arm,
/// with the call already read into `WS-CALL` / `WS-NAME` / `WS-ARGS`.
fn program(files: &str, setup: &str, on_tool_call: &str) -> String {
    format!(
        r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. TOOLS.
{files}
       WORKING-STORAGE SECTION.
       01 COBOL-EVENT-ID    PIC X(30).
       01 COBOL-CONTROL-ID  PIC X(30).
       01 COBOL-QUIT        PIC 9 VALUE 0.
       01 WS-OK             PIC X(4).
       01 WS-CALL           PIC X(40).
       01 WS-NAME           PIC X(40).
       01 WS-ARGS           PIC X(200).
       01 WS-RES            PIC X(200).
       01 WS-TEXT           PIC X(300).
       01 WS-N              PIC X(12).
       PROCEDURE DIVISION.
       MAIN.
{setup}
           MOVE AGT-1::Ask("How many actors earn 100000?") TO WS-TEXT
           PERFORM UNTIL COBOL-QUIT = 1
               CALL "COBOL-WAIT-EVENT"
                   USING COBOL-EVENT-ID COBOL-CONTROL-ID
               EVALUATE COBOL-EVENT-ID
                   WHEN "onToolCall"
                       MOVE AGT-1::ToolCallId TO WS-CALL
                       MOVE AGT-1::ToolName TO WS-NAME
                       MOVE AGT-1::ToolArguments TO WS-ARGS
                       DISPLAY "TOOL=" WS-NAME
                       DISPLAY "ARGS=" WS-ARGS
{on_tool_call}
                   WHEN "onResponse"
                       MOVE AGT-1::LastReply TO WS-TEXT
                       DISPLAY "REPLY=" WS-TEXT
                       PERFORM REPORT-USAGE
                       MOVE 1 TO COBOL-QUIT
                   WHEN "onError"
                       MOVE AGT-1::LastError TO WS-TEXT
                       DISPLAY "ERROR=" WS-TEXT
                       PERFORM REPORT-USAGE
                       MOVE 1 TO COBOL-QUIT
                   WHEN "onCancelled"
                       DISPLAY "CANCELLED"
                       MOVE 1 TO COBOL-QUIT
                   WHEN "onTimeout"
                       DISPLAY "TIMEOUT"
                       MOVE 1 TO COBOL-QUIT
               END-EVALUATE
           END-PERFORM.
           STOP RUN.
       REPORT-USAGE.
           MOVE AGT-1::LastInputTokens TO WS-N
           DISPLAY "IN=" WS-N
           MOVE AGT-1::LastOutputTokens TO WS-N
           DISPLAY "OUT=" WS-N
           MOVE AGT-1::LastToolCallCount TO WS-N
           DISPLAY "CALLS=" WS-N.
"#
    )
}

/// Run `src` with `AGT-1` seeded from `props`; the DISPLAY lines, trimmed.
fn run(src: &str, props: &[(&str, &str)]) -> Vec<String> {
    let result = parse(tokenize(src, SourceFormat::Free));
    assert!(
        result.diagnostics.iter().all(|d| d.severity != Severity::Error),
        "parse errors: {:?}",
        result.diagnostics
    );
    let program = result.program.expect("no program");
    let props: Vec<(String, String)> = props
        .iter()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect();
    let (done_tx, done_rx) = mpsc::channel();
    thread::spawn(move || {
        // The sender stays alive for the whole run: a dropped one reads as a
        // closed form and ends the wait before the model has answered.
        let (_event_tx, event_rx) = mpsc::channel();
        let (state_tx, _state_rx) = mpsc::channel();
        let (display_tx, display_rx) = mpsc::channel();
        let mut interp = Interpreter::new_with_channels(program, event_rx, state_tx, display_tx);
        interp.seed_objects(vec![("AGT-1".into(), "AgentObject".into(), props)]);
        let _ = interp.run();
        let lines: Vec<String> = display_rx.try_iter().map(|l| l.trim().to_owned()).collect();
        let _ = done_tx.send(lines);
    });
    done_rx
        .recv_timeout(Duration::from_secs(30))
        .expect("the program did not finish within 30 s")
}

fn agent_props<'a>(api: &'a str, url: &'a str) -> Vec<(&'a str, &'a str)> {
    vec![
        ("AgentAPI", api),
        ("AgentURL", url),
        ("AgentModel", "test-model"),
        ("SystemPrompt", "Be brief."),
        ("Temperature", "0"),
        ("MaximumTokens", "100"),
        ("TimeoutSeconds", "10"),
    ]
}

fn line<'a>(out: &'a [String], prefix: &str) -> Option<&'a str> {
    out.iter().find_map(|l| l.strip_prefix(prefix)).map(str::trim)
}

fn report(test: &str, protocol: &str, forms: &str, rounds: usize, calls: &str, t: Duration) {
    println!("\n  ── 072 {test} ─────────────────────────────");
    println!("  protocol: {protocol}");
    println!("  forms:    {forms}");
    println!("  rounds:   {rounds} request(s) on the wire · tool calls: {calls}");
    println!("  time:     {:.1} ms for the whole question", t.as_secs_f64() * 1000.0);
    println!("  ───────────────────────────────────────────────\n");
}

// ── A consultable indexed file ─────────────────────────────────────────────────

const RECORD_LEN: usize = 111;

fn temp(tag: &str) -> std::path::PathBuf {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let d = std::env::temp_dir().join(format!("prc-072-{tag}-{nanos}"));
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn build_actors(path: &Path) {
    let mut f = cobolt_runtime::indexed_disk::DiskIndexedFile::new(
        path,
        RECORD_LEN,
        KeySpec {
            offset: 0,
            len: 9,
            duplicates: false,
        },
        Vec::new(),
    );
    assert_eq!(f.open(OpenMode::Output), status::OK);
    for (id, salary) in [("1", "100000"), ("2", "250000"), ("3", "100000")] {
        let mut rec = vec![b' '; RECORD_LEN];
        rec[0..9].copy_from_slice(format!("{id:>9}").as_bytes());
        rec[99..110].copy_from_slice(format!("{salary:>11}").as_bytes());
        assert_eq!(f.write(&rec), status::OK);
    }
    f.close();
}

const ACTORS_CIDX: &str = r#"<?xml version="1.0" encoding="UTF-8"?><IndexedFile name="ACTORS-FILE" finalized="true" version="1.0"><assign-path>actors.idx</assign-path><access-mode>dynamic</access-mode><record-format fixed-length="111"/><storage mode="disk" compression="false" persistence="false"/><comment><![CDATA[One row per performer]]></comment><keys><primary duplicates="false" ordering="ascending"><part field="ACTOR-ID" offset="0" length="9" encoding="bytes"/></primary></keys><fields><Field level="1" name="ACTORS-RECORD" usage="display"><Field level="5" name="ACTOR-ID" pic="9(9)" usage="display" offset="0" length="9"><comment><![CDATA[The key]]></comment></Field><Field level="5" name="ACTOR-SALARY" pic="9(9)V99" usage="display" offset="99" length="11"><comment><![CDATA[Annual salary]]></comment></Field></Field></fields></IndexedFile>"#;

fn actors_fd(data: &Path) -> String {
    format!(
        r#"
       ENVIRONMENT DIVISION.
       INPUT-OUTPUT SECTION.
       FILE-CONTROL.
           SELECT ACTORS-FILE ASSIGN TO "{}"
               ORGANIZATION IS INDEXED
               ACCESS MODE IS DYNAMIC
               RECORD KEY IS ACTOR-ID.
       DATA DIVISION.
       FILE SECTION.
       FD ACTORS-FILE.
       01 ACTORS-RECORD.
          05 ACTOR-ID       PIC 9(9).
          05 FILLER         PIC X(90).
          05 ACTOR-SALARY   PIC 9(9)V99.
          05 FILLER         PIC X(1)."#,
        data.display()
    )
}

// ── Tests ──────────────────────────────────────────────────────────────────────

/// AC1 — OpenAI-compatible, native tools: `AllowFile` offers an indexed file,
/// the model searches it, the search runs, the result goes back, and the final
/// answer arrives through `onResponse` with the usage summed over both rounds.
#[test]
fn an_allowed_indexed_file_is_searched_for_the_model() {
    let dir = temp("allow");
    let data = dir.join("actors.idx");
    let cidx = dir.join("actors.cidx");
    build_actors(&data);
    std::fs::write(&cidx, ACTORS_CIDX).unwrap();

    let (port, seen) = model_server(Box::new(|round, _| match round {
        0 => (200, openai_call("call_1", "search_actors_file", r#"{"ACTOR-SALARY":"100000"}"#)),
        _ => (200, openai_text("Two actors earn 100000.")),
    }));
    let url = format!("http://127.0.0.1:{port}/v1/chat/completions");
    let setup = format!(
        r#"           MOVE AGT-1::AllowFile("ACTORS-FILE", "{}") TO WS-OK
           DISPLAY "ALLOW=" WS-OK"#,
        cidx.display()
    );
    let started = Instant::now();
    let out = run(
        &program(&actors_fd(&data), &setup, ""),
        &agent_props("OpenAI", &url),
    );
    let elapsed = started.elapsed();
    let seen = seen.lock().unwrap().clone();

    assert_eq!(line(&out, "ALLOW="), Some("1"), "{out:?}");
    assert_eq!(line(&out, "REPLY="), Some("Two actors earn 100000."), "{out:?}");
    assert!(line(&out, "TOOL=").is_none(), "an indexed tool never reaches the program");
    assert_eq!(seen.len(), 2, "two rounds on the wire");
    let first: serde_json::Value = serde_json::from_str(&seen[0]).unwrap();
    assert_eq!(first["tools"][0]["function"]["name"], "search_actors_file");
    let second: serde_json::Value = serde_json::from_str(&seen[1]).unwrap();
    let msgs = second["messages"].as_array().unwrap();
    let tool_msg = msgs.iter().find(|m| m["role"] == "tool").expect("a tool message");
    assert_eq!(tool_msg["tool_call_id"], "call_1");
    let result = tool_msg["content"].as_str().unwrap();
    for fragment in ["2 record(s)", "ACTOR-ID=1", "ACTOR-ID=3"] {
        assert!(result.contains(fragment), "search result missing {fragment}: {result}");
    }
    assert!(!result.contains("ACTOR-ID=2"), "{result}");
    assert_eq!(line(&out, "IN="), Some("130"), "50 + 80: {out:?}");
    assert_eq!(line(&out, "OUT="), Some("16"), "10 + 6: {out:?}");
    assert_eq!(line(&out, "CALLS="), Some("1"), "{out:?}");

    report(
        "indexed file tool",
        "OpenAiChat, native",
        "AllowFile(fd, cidx) · search_actors_file(ACTOR-SALARY)",
        seen.len(),
        "1 (indexed, answered in-process: 2 of 3 records)",
        elapsed,
    );
    let _ = std::fs::remove_dir_all(&dir);
}

/// AC2 + AC7 — a tool the program declared is answered by its own
/// `onToolCall` handler, in each native protocol's shape.
#[test]
fn a_cobol_tool_is_answered_by_on_tool_call_in_every_protocol() {
    struct Case {
        api: &'static str,
        path: &'static str,
        first: String,
        last: String,
        result_needle: &'static str,
    }
    let cases = [
        Case {
            api: "OpenAI",
            path: "/v1/chat/completions",
            first: openai_call("call_w", "get_weather", r#"{"CITY":"Lisbon"}"#),
            last: openai_text("It is sunny."),
            result_needle: r#""tool_call_id":"call_w""#,
        },
        Case {
            api: "Ollama",
            path: "/api/chat",
            first: serde_json::json!({
                "message": {"content": "", "tool_calls": [
                    {"function": {"name": "get_weather", "arguments": {"CITY": "Lisbon"}}}
                ]},
                "prompt_eval_count": 50, "eval_count": 10
            })
            .to_string(),
            last: serde_json::json!({
                "message": {"content": "It is sunny."},
                "prompt_eval_count": 80, "eval_count": 6
            })
            .to_string(),
            result_needle: r#""tool_name":"get_weather""#,
        },
        Case {
            api: "Anthropic",
            path: "/v1/messages",
            first: serde_json::json!({
                "content": [
                    {"type": "text", "text": "Checking."},
                    {"type": "tool_use", "id": "tu_1", "name": "get_weather", "input": {"CITY": "Lisbon"}}
                ],
                "usage": {"input_tokens": 50, "output_tokens": 10}
            })
            .to_string(),
            last: serde_json::json!({
                "content": [{"type": "text", "text": "It is sunny."}],
                "usage": {"input_tokens": 80, "output_tokens": 6}
            })
            .to_string(),
            result_needle: r#""tool_use_id":"tu_1""#,
        },
    ];
    let setup = r#"           MOVE AGT-1::AddTool("get_weather", "Today's weather in a city") TO WS-OK
           MOVE AGT-1::AddToolParameter("get_weather", "CITY", "The city name") TO WS-OK
           DISPLAY "PARAM=" WS-OK"#;
    let handler = r#"                       MOVE "Sunny, 25C" TO WS-RES
                       MOVE AGT-1::SetToolResult(WS-CALL, WS-RES) TO WS-OK"#;

    for case in cases {
        let (first, last) = (case.first.clone(), case.last.clone());
        let (port, seen) = model_server(Box::new(move |round, _| match round {
            0 => (200, first.clone()),
            _ => (200, last.clone()),
        }));
        let url = format!("http://127.0.0.1:{port}{}", case.path);
        let started = Instant::now();
        let out = run(&program("       DATA DIVISION.", setup, handler), &agent_props(case.api, &url));
        let elapsed = started.elapsed();
        let seen = seen.lock().unwrap().clone();

        assert_eq!(line(&out, "PARAM="), Some("1"), "{}: {out:?}", case.api);
        assert_eq!(line(&out, "TOOL="), Some("get_weather"), "{}: {out:?}", case.api);
        assert!(
            line(&out, "ARGS=").is_some_and(|a| a.contains("Lisbon")),
            "{}: {out:?}",
            case.api
        );
        assert_eq!(line(&out, "REPLY="), Some("It is sunny."), "{}: {out:?}", case.api);
        assert_eq!(seen.len(), 2, "{}", case.api);
        assert!(seen[0].contains("get_weather") && seen[0].contains("CITY"), "{}", case.api);
        assert!(seen[1].contains("Sunny, 25C"), "{}: {}", case.api, seen[1]);
        assert!(seen[1].contains(case.result_needle), "{}: {}", case.api, seen[1]);
        assert_eq!(line(&out, "IN="), Some("130"), "{}: {out:?}", case.api);
        assert_eq!(line(&out, "CALLS="), Some("1"), "{}: {out:?}", case.api);

        report(
            "COBOL tool via onToolCall",
            case.api,
            "AddTool · AddToolParameter · onToolCall → SetToolResult",
            seen.len(),
            "1 (answered by the program)",
            elapsed,
        );
    }
}

/// AC3 — an agent offering no tools sends exactly the bytes it always sent.
#[test]
fn no_tools_means_the_request_is_byte_identical() {
    let (port, seen) = model_server(Box::new(|_, _| (200, openai_text("Hello."))));
    let url = format!("http://127.0.0.1:{port}/v1/chat/completions");
    let started = Instant::now();
    let out = run(&program("       DATA DIVISION.", "", ""), &agent_props("OpenAI", &url));
    let elapsed = started.elapsed();
    let seen = seen.lock().unwrap().clone();

    let expected = body_for(
        &AskRequest {
            api: "OpenAI".into(),
            url: url.clone(),
            model: "test-model".into(),
            system_prompt: "Be brief.".into(),
            prompt: "How many actors earn 100000?".into(),
            temperature: 0,
            max_tokens: 100,
            ..Default::default()
        },
        Protocol::OpenAiChat,
    );
    assert_eq!(seen.len(), 1);
    assert_eq!(seen[0], expected, "the no-tools body changed");
    assert_eq!(line(&out, "REPLY="), Some("Hello."), "{out:?}");
    assert_eq!(line(&out, "IN="), Some("80"), "usage is read without tools too: {out:?}");
    assert_eq!(line(&out, "CALLS="), Some("0"), "{out:?}");
    report("no tools", "OpenAiChat", "Ask only", seen.len(), "0", elapsed);
}

/// AC4 — a model that never stops calling tools is stopped at
/// `MaximumToolRounds`, with `onError` — never an endless loop.
#[test]
fn a_model_that_keeps_calling_tools_is_stopped() {
    let (port, seen) = model_server(Box::new(|round, _| {
        (200, openai_call(&format!("c{round}"), "get_weather", r#"{"CITY":"X"}"#))
    }));
    let url = format!("http://127.0.0.1:{port}/v1/chat/completions");
    let mut props = agent_props("OpenAI", &url);
    props.push(("MaximumToolRounds", "3"));
    let setup = r#"           MOVE AGT-1::AddTool("get_weather", "Weather") TO WS-OK"#;
    let handler = r#"                       MOVE AGT-1::SetToolResult(WS-CALL, "rain") TO WS-OK"#;
    let started = Instant::now();
    let out = run(&program("       DATA DIVISION.", setup, handler), &props);
    let elapsed = started.elapsed();
    let seen = seen.lock().unwrap().clone();

    let error = line(&out, "ERROR=").unwrap_or_default();
    assert!(error.contains("MaximumToolRounds"), "{out:?}");
    assert_eq!(seen.len(), 3, "exactly MaximumToolRounds requests");
    assert!(line(&out, "REPLY=").is_none());
    assert_eq!(line(&out, "CALLS="), Some("2"), "the third round's calls were not run: {out:?}");
    report(
        "round limit",
        "OpenAiChat",
        "MaximumToolRounds = 3, a model that always calls",
        seen.len(),
        "2 answered, then onError",
        elapsed,
    );
}

/// AC5 — an unknown tool and arguments that are not JSON are told to the
/// model as error results; the question itself does not fail, and the
/// program's handler never sees either call.
#[test]
fn unknown_tools_and_bad_arguments_go_back_to_the_model() {
    let (port, seen) = model_server(Box::new(|round, _| match round {
        0 => (
            200,
            serde_json::json!({"choices": [{"message": {"content": null, "tool_calls": [
                {"id": "a", "type": "function", "function": {"name": "ghost", "arguments": "{}"}},
                {"id": "b", "type": "function", "function": {"name": "get_weather", "arguments": "{not json"}}
            ]}}]})
            .to_string(),
        ),
        _ => (200, openai_text("Sorry, I could not check.")),
    }));
    let url = format!("http://127.0.0.1:{port}/v1/chat/completions");
    let setup = r#"           MOVE AGT-1::AddTool("get_weather", "Weather") TO WS-OK"#;
    let started = Instant::now();
    let out = run(&program("       DATA DIVISION.", setup, ""), &agent_props("OpenAI", &url));
    let elapsed = started.elapsed();
    let seen = seen.lock().unwrap().clone();

    assert!(line(&out, "TOOL=").is_none(), "neither call reaches the program: {out:?}");
    assert_eq!(line(&out, "REPLY="), Some("Sorry, I could not check."), "{out:?}");
    assert_eq!(seen.len(), 2);
    assert!(seen[1].contains("no tool named 'ghost'"), "{}", seen[1]);
    assert!(seen[1].contains("not a JSON object"), "{}", seen[1]);
    report(
        "bad calls",
        "OpenAiChat",
        "unknown tool · arguments that are not JSON",
        seen.len(),
        "2 (both answered with an error result)",
        elapsed,
    );
}

/// AC6 — `ToolProtocol = Fenced` works against a server that rejects any
/// request carrying a `tools` field: the tools ride in the system prompt, the
/// call is a fenced JSON block in the text, the result is a user turn.
#[test]
fn the_fenced_protocol_works_where_native_tools_are_refused() {
    let (port, seen) = model_server(Box::new(|round, body| {
        let v: serde_json::Value = serde_json::from_str(body).unwrap_or_default();
        if v.get("tools").is_some() {
            return (400, r#"{"error":"this model does not support tools"}"#.to_owned());
        }
        match round {
            0 => (
                200,
                openai_text(
                    "```json\n{\"tool_calls\":[{\"tool\":\"get_weather\",\"args\":{\"CITY\":\"Porto\"}}]}\n```",
                ),
            ),
            _ => (200, openai_text("Cloudy in Porto.")),
        }
    }));
    let url = format!("http://127.0.0.1:{port}/v1/chat/completions");
    let mut props = agent_props("OpenAI", &url);
    props.push(("ToolProtocol", "Fenced"));
    let setup = r#"           MOVE AGT-1::AddTool("get_weather", "Weather") TO WS-OK
           MOVE AGT-1::AddToolParameter("get_weather", "CITY", "City") TO WS-OK"#;
    let handler = r#"                       MOVE AGT-1::SetToolResult(WS-CALL, "cloudy") TO WS-OK"#;
    let started = Instant::now();
    let out = run(&program("       DATA DIVISION.", setup, handler), &props);
    let elapsed = started.elapsed();
    let seen = seen.lock().unwrap().clone();

    assert_eq!(line(&out, "TOOL="), Some("get_weather"), "{out:?}");
    assert!(line(&out, "ARGS=").is_some_and(|a| a.contains("Porto")), "{out:?}");
    assert_eq!(line(&out, "REPLY="), Some("Cloudy in Porto."), "{out:?}");
    assert_eq!(seen.len(), 2);
    let first: serde_json::Value = serde_json::from_str(&seen[0]).unwrap();
    let system = first["messages"][0]["content"].as_str().unwrap();
    assert!(system.starts_with("Be brief.") && system.contains("get_weather"), "{system}");
    assert!(seen[1].contains("Tool results:") && seen[1].contains("cloudy"), "{}", seen[1]);
    report(
        "fenced protocol",
        "OpenAiChat, fenced (server refuses `tools`)",
        "ToolProtocol = Fenced · fenced JSON call · results as a user turn",
        seen.len(),
        "1 (answered by the program)",
        elapsed,
    );
}

/// AC8 — `Cancel` while the program is answering a tool: `onCancelled`, and
/// no further round is ever sent.
#[test]
fn cancel_during_a_tool_call_stops_the_loop() {
    let (port, seen) = model_server(Box::new(|round, _| match round {
        0 => (200, openai_call("c1", "get_weather", r#"{"CITY":"X"}"#)),
        _ => (200, openai_text("should never be asked")),
    }));
    let url = format!("http://127.0.0.1:{port}/v1/chat/completions");
    let setup = r#"           MOVE AGT-1::AddTool("get_weather", "Weather") TO WS-OK"#;
    let handler = r#"                       MOVE AGT-1::Cancel() TO WS-OK"#;
    let started = Instant::now();
    let out = run(&program("       DATA DIVISION.", setup, handler), &agent_props("OpenAI", &url));
    let elapsed = started.elapsed();
    // Give a stray round, if one were sent, time to arrive.
    thread::sleep(Duration::from_millis(300));
    let seen = seen.lock().unwrap().clone();

    assert!(out.iter().any(|l| l == "CANCELLED"), "{out:?}");
    assert!(line(&out, "REPLY=").is_none(), "{out:?}");
    assert_eq!(seen.len(), 1, "no round after the cancel");
    report(
        "cancel",
        "OpenAiChat",
        "Cancel() inside onToolCall",
        seen.len(),
        "1 (cancelled before its result was sent)",
        elapsed,
    );
}

/// Spec 071 — `ToolProtocol = None`: a file the program allowed is offered to
/// every agent, and this switch takes it away from one. The request goes out
/// as plain chat, with no tools field at all.
#[test]
fn tool_protocol_none_offers_this_agent_no_tools() {
    let dir = temp("none");
    let data = dir.join("actors.idx");
    let cidx = dir.join("actors.cidx");
    build_actors(&data);
    std::fs::write(&cidx, ACTORS_CIDX).unwrap();
    let (port, seen) = model_server(Box::new(|_, _| (200, openai_text("A plain answer."))));
    let url = format!("http://127.0.0.1:{port}/v1/chat/completions");
    let setup = format!(
        r#"           MOVE AGT-1::AllowFile("ACTORS-FILE", "{}") TO WS-OK
           DISPLAY "ALLOW=" WS-OK"#,
        cidx.display()
    );
    let mut props = agent_props("OpenAI", &url);
    props.push(("ToolProtocol", "None"));
    let started = Instant::now();
    let out = run(&program(&actors_fd(&data), &setup, ""), &props);
    let seen = seen.lock().unwrap().clone();
    assert_eq!(line(&out, "ALLOW="), Some("1"), "the file is allowed: {out:?}");
    assert_eq!(line(&out, "REPLY="), Some("A plain answer."), "{out:?}");
    assert_eq!(seen.len(), 1, "one plain round");
    let first: serde_json::Value = serde_json::from_str(&seen[0]).unwrap();
    assert!(first["tools"].is_null(), "no tools offered: {}", seen[0]);
    report("ToolProtocol None", "OpenAI", "AllowFile + ToolProtocol=None", seen.len(), "0", started.elapsed());
}
