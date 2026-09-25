// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! The IDE's model-provider handling, from a running COBOL program:
//! `COBOL-PROVIDER-COUNT/GET`, `COBOL-MODEL-LIST/-GET` and `COBOL-MODEL-TEST`,
//! against a local stand-in server that answers like OpenAI and like Ollama —
//! and refuses a bad key with 401, so the IDE's help text is checked too.

#![cfg(feature = "http")]

use std::io::{Read, Write};
use std::net::TcpListener;
use std::sync::mpsc;
use std::sync::{Arc, Mutex};

use cobolt_lexer::{tokenize, SourceFormat};
use cobolt_parser::{parse, Severity};
use cobolt_runtime::Interpreter;

/// A server that answers each request by path, and remembers every request
/// line with its Authorization header.
fn server() -> (String, Arc<Mutex<Vec<String>>>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = format!("http://{}", listener.local_addr().unwrap());
    let seen = Arc::new(Mutex::new(Vec::new()));
    let log = seen.clone();
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut s) = stream else { continue };
            let mut buf = vec![0u8; 65536];
            let mut got = 0;
            // Read the head, then the body its Content-Length announces.
            loop {
                let n = s.read(&mut buf[got..]).unwrap_or(0);
                if n == 0 {
                    break;
                }
                got += n;
                let text = String::from_utf8_lossy(&buf[..got]).to_string();
                if let Some(head_end) = text.find("\r\n\r\n") {
                    let len = text
                        .lines()
                        .find_map(|l| l.to_ascii_lowercase().strip_prefix("content-length:").map(|v| v.trim().parse::<usize>().unwrap_or(0)))
                        .unwrap_or(0);
                    if got >= head_end + 4 + len {
                        break;
                    }
                }
            }
            let req = String::from_utf8_lossy(&buf[..got]).to_string();
            let line = req.lines().next().unwrap_or("").to_string();
            let auth = req
                .lines()
                .find(|l| l.to_ascii_lowercase().starts_with("authorization:"))
                .map(|l| l.split_once(':').unwrap().1.trim().to_string())
                .unwrap_or_default();
            log.lock().unwrap().push(format!("{line} | {auth}"));
            let (status, body) = if auth == "Bearer bad" {
                ("401 Unauthorized", r#"{"error":{"message":"Incorrect API key provided"}}"#.to_string())
            } else if line.starts_with("GET /v1/models") {
                ("200 OK", r#"{"data":[{"id":"gpt-4o"},{"id":"text-embedding-3-large"},{"id":"gpt-4o-mini"}]}"#.to_string())
            } else if line.starts_with("GET /api/tags") {
                ("200 OK", r#"{"models":[{"name":"llama3.2:3b"},{"name":"qwen3-coder-next"},{"name":"gpt-oss:20b"}]}"#.to_string())
            } else if line.starts_with("POST /v1/chat/completions") {
                ("200 OK", r#"{"choices":[{"message":{"content":"OK"}}]}"#.to_string())
            } else {
                ("404 Not Found", "{}".to_string())
            };
            let _ = write!(
                s,
                "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            );
        }
    });
    (addr, seen)
}

fn run(body: &str) -> Vec<String> {
    let src = format!(
        r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. T.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 WS-N       PIC 9(3).
       01 WS-I       PIC 9(3).
       01 WS-ID      PIC X(20).
       01 WS-LABEL   PIC X(40).
       01 WS-EP      PIC X(80).
       01 WS-NEEDS   PIC X.
       01 WS-MODEL   PIC X(60).
       01 WS-STATUS  PIC X(600).
       01 WS-URL     PIC X(80).
       PROCEDURE DIVISION.
{body}
           STOP RUN.
"#
    );
    let result = parse(tokenize(&src, SourceFormat::Free));
    assert!(result.diagnostics.iter().all(|d| d.severity != Severity::Error), "{:?}", result.diagnostics);
    let (_e, event_rx) = mpsc::channel();
    let (state_tx, _s) = mpsc::channel();
    let (display_tx, display_rx) = mpsc::channel();
    let mut interp = Interpreter::new_with_channels(result.program.unwrap(), event_rx, state_tx, display_tx);
    interp.run().expect("run failed");
    display_rx.try_iter().map(|s| s.trim_end().to_owned()).collect()
}

#[test]
fn a_program_lists_providers_fetches_models_and_tests_a_connection() {
    let t = std::time::Instant::now();
    let (url, seen) = server();
    let out = run(&format!(
        r#"
           CALL "COBOL-PROVIDER-COUNT" USING WS-N
           DISPLAY "COUNT=" WS-N
           CALL "COBOL-PROVIDER-GET" USING 2 WS-ID WS-LABEL WS-EP WS-NEEDS
           DISPLAY "P2=" FUNCTION TRIM(WS-ID) "|" FUNCTION TRIM(WS-LABEL) "|" FUNCTION TRIM(WS-EP) "|" WS-NEEDS
           CALL "COBOL-PROVIDER-GET" USING 15 WS-ID WS-LABEL WS-EP WS-NEEDS
           DISPLAY "P15=" FUNCTION TRIM(WS-ID) "|" WS-NEEDS

           MOVE "{url}/v1" TO WS-URL
           CALL "COBOL-MODEL-LIST" USING "openai" WS-URL "sk-good" WS-N WS-STATUS
           DISPLAY "OPENAI=" WS-N " " FUNCTION TRIM(WS-STATUS)
           PERFORM VARYING WS-I FROM 1 BY 1 UNTIL WS-I > WS-N
               CALL "COBOL-MODEL-LIST-GET" USING WS-I WS-MODEL
               DISPLAY "  M=" FUNCTION TRIM(WS-MODEL)
           END-PERFORM

           MOVE "{url}/api" TO WS-URL
           CALL "COBOL-MODEL-LIST" USING "ollama" WS-URL " " WS-N WS-STATUS
           DISPLAY "OLLAMA=" WS-N " " FUNCTION TRIM(WS-STATUS)
           CALL "COBOL-MODEL-LIST-GET" USING 2 WS-MODEL
           DISPLAY "  O2=" FUNCTION TRIM(WS-MODEL)

           MOVE "{url}/v1" TO WS-URL
           CALL "COBOL-MODEL-TEST" USING "groq" WS-URL "gpt-4o" "sk-good" WS-STATUS
           DISPLAY "TEST=" FUNCTION TRIM(WS-STATUS)
           CALL "COBOL-MODEL-TEST" USING "openai" WS-URL "gpt-4o" "bad" WS-STATUS
           DISPLAY "BAD=" WS-STATUS(1:120)
           CALL "COBOL-MODEL-TEST" USING "openai" "https://api.example.invalid/v1" "gpt-4o" " " WS-STATUS
           DISPLAY "NOKEY=" WS-STATUS(1:80)
           CALL "COBOL-MODEL-LIST" USING "openai" WS-URL "bad" WS-N WS-STATUS
           DISPLAY "LISTBAD=" WS-N " " WS-STATUS(1:60)
"#
    ));
    let joined = out.join("\n");
    for want in [
        "COUNT=017",
        "P2=anthropic|Anthropic|https://api.anthropic.com/v1|Y",
        "P15=ollama|N",
        "OPENAI=002 OK",
        "  M=gpt-4o",
        "  M=gpt-4o-mini",
        "OLLAMA=002 OK",
        "  O2=gpt-oss:20b",
        "TEST=OK",
        "BAD=gpt-4o: Connection test failed: HTTP 401: Incorrect API key provided",
        "NOKEY=gpt-4o: Connection test failed: no API key reached the request",
        "LISTBAD=000 Could not list models: Failed to fetch models from API (401",
    ] {
        assert!(joined.contains(want), "missing {want:?} in:\n{joined}");
    }
    assert!(joined.contains("EXPIRED") || out.iter().any(|l| l.contains("401")), "{joined}");
    let seen = seen.lock().unwrap().clone();
    assert!(seen.iter().any(|l| l == "GET /v1/models HTTP/1.1 | Bearer sk-good"), "{seen:?}");
    assert!(seen.iter().any(|l| l.starts_with("GET /api/tags")), "{seen:?}");
    assert!(seen.iter().any(|l| l == "POST /v1/chat/completions HTTP/1.1 | Bearer sk-good"), "{seen:?}");
    assert!(!seen.iter().any(|l| l.contains("example.invalid")), "a keyless hosted test sends nothing");
    println!("\n  ── model providers from COBOL ──");
    println!("  17 providers listed · openai list 2 of 3 (embedding filtered) · ollama list 2 of 3 (retired filtered)");
    println!("  test OK (groq at a /v1 root → /v1/chat/completions) · 401 explained · keyless refused unsent");
    println!("  {} requests reached the server — {:.0} ms\n", seen.len(), t.elapsed().as_secs_f64() * 1000.0);
}
