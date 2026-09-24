// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Spec 068 — the `KnowledgeBase` control, end to end through a real COBOL
//! event loop: every operation runs in the background and reports through
//! `onProgress`, `onIndexed`, `onSearchComplete`, `onBusy` and `onError`, each
//! handler reading the values its own event carried.
//!
//! Each test prints one summary block with what ran and how long it took.
#![cfg(feature = "kb")]

use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

use cobolt_lexer::{tokenize, SourceFormat};
use cobolt_parser::{parse, Severity};
use cobolt_runtime::Interpreter;

fn temp(tag: &str) -> PathBuf {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let d = std::env::temp_dir().join(format!("prc-068-{tag}-{nanos}"));
    std::fs::create_dir_all(&d).unwrap();
    d
}

/// Run `src` with `KB-1` seeded from `props`; the DISPLAY lines, trimmed.
fn run(src: &str, props: Vec<(String, String)>) -> Vec<String> {
    run_with(src, vec![("KB-1".into(), "KnowledgeBase".into(), props)])
}

type Seed = (String, String, Vec<(String, String)>);

fn run_with(src: &str, objects: Vec<Seed>) -> Vec<String> {
    let result = parse(tokenize(src, SourceFormat::Free));
    assert!(
        result.diagnostics.iter().all(|d| d.severity != Severity::Error),
        "parse errors: {:?}",
        result.diagnostics
    );
    let program = result.program.expect("no program");
    let (done_tx, done_rx) = mpsc::channel();
    thread::spawn(move || {
        let (_event_tx, event_rx) = mpsc::channel();
        let (state_tx, _state_rx) = mpsc::channel();
        let (display_tx, display_rx) = mpsc::channel();
        let mut interp = Interpreter::new_with_channels(program, event_rx, state_tx, display_tx);
        interp.seed_objects(objects);
        let _ = interp.run();
        let lines: Vec<String> = display_rx.try_iter().map(|l| l.trim().to_owned()).collect();
        let _ = done_tx.send(lines);
    });
    done_rx
        .recv_timeout(Duration::from_secs(60))
        .expect("the program did not finish within 60 s")
}

fn props(location: &Path, extra: &[(&str, &str)]) -> Vec<(String, String)> {
    let mut p = vec![
        ("Location".to_string(), location.display().to_string()),
        ("Collection".to_string(), "hr".to_string()),
        ("Embedder".to_string(), "Lexical".to_string()),
        ("MaximumResults".to_string(), "3".to_string()),
    ];
    for (k, v) in extra {
        p.retain(|(key, _)| key != k);
        p.push((k.to_string(), v.to_string()));
    }
    p
}

const HEADER: &str = r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. KBTEST.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 COBOL-EVENT-ID    PIC X(30).
       01 COBOL-CONTROL-ID  PIC X(30).
       01 COBOL-QUIT        PIC 9 VALUE 0.
       01 WS-STEP           PIC 99 VALUE 0.
       01 WS-OK             PIC X(4).
       01 WS-A              PIC X(80).
       01 WS-B              PIC X(80).
       01 WS-C              PIC X(80).
       01 WS-TEXT           PIC X(300).
"#;

fn line<'a>(out: &'a [String], prefix: &str) -> Vec<&'a str> {
    out.iter().filter_map(|l| l.strip_prefix(prefix)).map(str::trim).collect()
}

/// Import, add, refresh (with an outside file and an unreadable one), search,
/// delete, search again — every step a background operation reporting through
/// its events (AC8, AC9, AC10, AC14).
#[test]
fn a_program_indexes_refreshes_searches_and_deletes() {
    let root = temp("flow");
    let location = root.join("KB");
    let docs = location.join("hr").join("documents");
    std::fs::create_dir_all(&docs).unwrap();
    // Already in the folder, as if put there with the file manager.
    std::fs::write(docs.join("travel.md"), "# Travel\nBook flights through the travel desk.").unwrap();
    std::fs::write(docs.join("photo.jpg"), [1u8, 2, 3]).unwrap();
    for i in 0..30 {
        std::fs::write(docs.join(format!("memo{i:02}.md")), format!("# Memo {i}\nRoutine notice {i}.")).unwrap();
    }
    let import = root.join("leave.md");
    std::fs::write(&import, "# Leave\nAnnual leave is twenty working days.").unwrap();

    let src = format!(
        r#"{HEADER}
       PROCEDURE DIVISION.
       MAIN.
           MOVE KB-1::ImportDocument("{imp}") TO WS-OK
           DISPLAY "START=" WS-OK
           PERFORM UNTIL COBOL-QUIT = 1
               CALL "COBOL-WAIT-EVENT" USING COBOL-EVENT-ID COBOL-CONTROL-ID
               EVALUATE COBOL-EVENT-ID
                   WHEN "onProgress"
                       MOVE KB-1::ProgressCurrent TO WS-A
                       MOVE KB-1::ProgressTotal TO WS-B
                       DISPLAY "PROG=" WS-STEP ":" FUNCTION TRIM(WS-A) "/" FUNCTION TRIM(WS-B)
                   WHEN "onIndexed"
                       ADD 1 TO WS-STEP
                       MOVE KB-1::AddedCount TO WS-A
                       MOVE KB-1::UpdatedCount TO WS-B
                       MOVE KB-1::RemovedCount TO WS-C
                       DISPLAY "INDEXED=" WS-STEP ":" FUNCTION TRIM(WS-A) "," FUNCTION TRIM(WS-B) "," FUNCTION TRIM(WS-C)
                       MOVE KB-1::SkippedDocuments TO WS-TEXT
                       DISPLAY "SKIPPED=" WS-TEXT
                       EVALUATE WS-STEP
                           WHEN 1
                               MOVE KB-1::AddDocument("pay.md", "Salaries are paid monthly on the last working day.") TO WS-OK
                           WHEN 2
                               MOVE KB-1::Refresh() TO WS-OK
                           WHEN 3
                               MOVE KB-1::Search("salaries paid monthly") TO WS-OK
                           WHEN 4
                               MOVE KB-1::Search("salaries paid monthly") TO WS-OK
                       END-EVALUATE
                   WHEN "onSearchComplete"
                       MOVE KB-1::ResultCount TO WS-A
                       MOVE KB-1::GetResultDocument(1) TO WS-B
                       MOVE KB-1::SearchMode TO WS-C
                       DISPLAY "SEARCH=" WS-STEP ":" FUNCTION TRIM(WS-A) ":" FUNCTION TRIM(WS-B) ":" FUNCTION TRIM(WS-C)
                       MOVE KB-1::GetResultHeading(1) TO WS-TEXT
                       DISPLAY "HEADING=" WS-TEXT
                       IF WS-STEP = 3
                           ADD 1 TO WS-STEP
                           MOVE KB-1::DeleteDocument("pay.md") TO WS-OK
                           SUBTRACT 1 FROM WS-STEP
                       ELSE
                           MOVE 1 TO COBOL-QUIT
                       END-IF
                   WHEN "onError"
                       MOVE KB-1::LastError TO WS-TEXT
                       DISPLAY "ERROR=" WS-TEXT
                       MOVE 1 TO COBOL-QUIT
               END-EVALUATE
           END-PERFORM
           MOVE KB-1::ListDocuments() TO WS-A
           DISPLAY "DOCS=" WS-A
           STOP RUN.
"#,
        imp = import.display()
    );
    let t = Instant::now();
    let out = run(&src, props(&location, &[]));
    let took = t.elapsed();

    assert_eq!(line(&out, "START="), vec!["1"], "{out:#?}");
    assert!(line(&out, "ERROR=").is_empty(), "{out:#?}");
    let indexed = line(&out, "INDEXED=");
    // 1 import, 2 add, 3 refresh (travel + 30 memos; photo skipped), 4 delete.
    assert_eq!(indexed[0], "01:1,0,0", "{out:#?}");
    assert_eq!(indexed[1], "02:1,0,0");
    assert_eq!(indexed[2], "03:31,0,0", "only the outside files were indexed");
    assert_eq!(indexed[3], "04:0,0,1", "pay.md removed");
    let skipped = line(&out, "SKIPPED=");
    assert!(skipped[2].starts_with("photo.jpg:"), "{skipped:?}");
    let search = line(&out, "SEARCH=");
    assert!(search[0].starts_with("03:") && search[0].contains(":pay.md:Lexical"), "{search:?}");
    assert!(!search[1].contains("pay.md"), "deleted documents are gone: {search:?}");
    assert_eq!(line(&out, "HEADING=")[0], "pay", "a heading-less document is named by itself");
    // Progress during the refresh: strictly increasing, ending at the total.
    let refresh_progress: Vec<(usize, usize)> = line(&out, "PROG=")
        .into_iter()
        .filter(|p| p.starts_with("02:"))
        .map(|p| {
            let (cur, tot) = p[3..].split_once('/').unwrap();
            (cur.trim().parse().unwrap(), tot.trim().parse().unwrap())
        })
        .collect();
    assert!(!refresh_progress.is_empty(), "{out:#?}");
    assert!(refresh_progress.windows(2).all(|w| w[0].0 < w[1].0), "{refresh_progress:?}");
    let (last, total) = *refresh_progress.last().unwrap();
    assert_eq!(last, total);
    // travel, 30 memos, leave.md and photo.jpg are on disk; pay.md was deleted.
    assert_eq!(line(&out, "DOCS="), vec!["33"]);

    println!("\n  ── 068 KnowledgeBase end to end ─────────────────────");
    println!("  steps:    ImportDocument · AddDocument · Refresh · Search · DeleteDocument · Search");
    println!("  indexed:  1 + 1 + 31 documents; 1 skipped (photo.jpg); 1 removed");
    println!("  progress: {} onProgress events in the refresh, ending {last}/{total}", refresh_progress.len());
    println!("  time:     {:.1} ms for the whole program", took.as_secs_f64() * 1000.0);
    println!("  ──────────────────────────────────────────────────────\n");
    let _ = std::fs::remove_dir_all(&root);
}

/// An unreachable embedding server: documents are stored text-only and the
/// search runs lexically, each saying why (AC12).
#[test]
fn an_unreachable_embedder_falls_back_and_says_why() {
    let root = temp("down");
    let location = root.join("KB");
    let src = format!(
        r#"{HEADER}
       PROCEDURE DIVISION.
       MAIN.
           MOVE KB-1::AddDocument("pay.md", "Salaries are paid monthly.") TO WS-OK
           PERFORM UNTIL COBOL-QUIT = 1
               CALL "COBOL-WAIT-EVENT" USING COBOL-EVENT-ID COBOL-CONTROL-ID
               EVALUATE COBOL-EVENT-ID
                   WHEN "onIndexed"
                       MOVE KB-1::SearchModeReason TO WS-TEXT
                       DISPLAY "NOTE=" WS-TEXT
                       MOVE KB-1::Search("salaries") TO WS-OK
                   WHEN "onSearchComplete"
                       MOVE KB-1::SearchMode TO WS-A
                       MOVE KB-1::GetResultDocument(1) TO WS-B
                       DISPLAY "MODE=" WS-A
                       DISPLAY "DOC=" WS-B
                       MOVE KB-1::SearchModeReason TO WS-TEXT
                       DISPLAY "WHY=" WS-TEXT
                       MOVE 1 TO COBOL-QUIT
                   WHEN "onError"
                       MOVE KB-1::LastError TO WS-TEXT
                       DISPLAY "ERROR=" WS-TEXT
                       MOVE 1 TO COBOL-QUIT
               END-EVALUATE
           END-PERFORM
           STOP RUN.
"#
    );
    let out = run(
        &src,
        props(
            &location,
            &[("Embedder", "Endpoint"), ("EmbeddingURL", "http://127.0.0.1:9"), ("EmbeddingAPI", "Ollama")],
        ),
    );
    assert!(line(&out, "ERROR=").is_empty(), "{out:#?}");
    assert!(line(&out, "NOTE=")[0].starts_with("stored text-only for now"), "{out:#?}");
    assert_eq!(line(&out, "MODE="), vec!["Lexical"]);
    assert_eq!(line(&out, "DOC="), vec!["pay.md"], "still answers");
    assert!(line(&out, "WHY=")[0].contains("could not be reached"), "{out:#?}");
    println!("\n  ── 068 endpoint down: stored text-only, searched lexically, both explained\n");
    let _ = std::fs::remove_dir_all(&root);
}

/// Another application holding the collection's write lock: the operation
/// answers `onBusy` after the wait instead of hanging (AC5).
#[test]
fn a_held_write_lock_raises_on_busy() {
    let root = temp("busy");
    let location = root.join("KB");
    let collection = cobolt_kb::store::Collection::open(&location, "hr").unwrap();
    let lock = std::fs::File::options()
        .create(true)
        .truncate(false)
        .write(true)
        .open(PathBuf::from(format!("{}.lock", collection.index_path().display())))
        .unwrap();
    lock.try_lock().unwrap();
    let src = format!(
        r#"{HEADER}
       PROCEDURE DIVISION.
       MAIN.
           MOVE KB-1::AddDocument("pay.md", "Salaries.") TO WS-OK
           PERFORM UNTIL COBOL-QUIT = 1
               CALL "COBOL-WAIT-EVENT" USING COBOL-EVENT-ID COBOL-CONTROL-ID
               EVALUATE COBOL-EVENT-ID
                   WHEN "onBusy"
                       MOVE KB-1::LastError TO WS-TEXT
                       DISPLAY "BUSY=" WS-TEXT
                       MOVE 1 TO COBOL-QUIT
                   WHEN "onIndexed"
                       DISPLAY "INDEXED"
                       MOVE 1 TO COBOL-QUIT
               END-EVALUATE
           END-PERFORM
           STOP RUN.
"#
    );
    let t = Instant::now();
    let out = run(&src, props(&location, &[("WriteWaitMilliseconds", "300")]));
    let took = t.elapsed();
    drop(lock);
    assert!(line(&out, "BUSY=")[0].contains("busy"), "{out:#?}");
    assert!(took >= Duration::from_millis(300));
    println!("\n  ── 068 busy: onBusy after {:.0} ms (wait 300 ms)\n", took.as_secs_f64() * 1000.0);
    let _ = std::fs::remove_dir_all(&root);
}

// ── An agent that searches a collection (AC15) ────────────────────────────────

/// A scripted model server: answers each request from `script(round)`, keeps
/// the request bodies.
fn model_server(
    script: impl Fn(usize, &str) -> String + Send + 'static,
) -> (u16, std::sync::Arc<std::sync::Mutex<Vec<String>>>) {
    use std::io::{Read, Write};
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let seen = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    let log = seen.clone();
    thread::spawn(move || {
        for (round, stream) in listener.incoming().enumerate() {
            let Ok(mut stream) = stream else { break };
            let _ = stream.set_read_timeout(Some(Duration::from_secs(3)));
            let mut buf = Vec::new();
            let mut chunk = [0u8; 8192];
            let body = loop {
                match stream.read(&mut chunk) {
                    Ok(0) | Err(_) => break String::new(),
                    Ok(n) => buf.extend_from_slice(&chunk[..n]),
                }
                if let Some(i) = buf.windows(4).position(|w| w == b"\r\n\r\n") {
                    let head = String::from_utf8_lossy(&buf[..i]).to_ascii_lowercase();
                    let len = head
                        .lines()
                        .find_map(|l| l.strip_prefix("content-length:"))
                        .and_then(|v| v.trim().parse::<usize>().ok())
                        .unwrap_or(0);
                    while buf.len() < i + 4 + len {
                        match stream.read(&mut chunk) {
                            Ok(0) | Err(_) => break,
                            Ok(n) => buf.extend_from_slice(&chunk[..n]),
                        }
                    }
                    break String::from_utf8_lossy(&buf[i + 4..]).into_owned();
                }
            };
            let reply = script(round, &body);
            log.lock().unwrap().push(body);
            let resp = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                reply.len(),
                reply
            );
            let _ = stream.write_all(resp.as_bytes());
        }
    });
    (port, seen)
}

#[test]
fn an_agent_answers_from_a_collection_and_names_the_document() {
    let root = temp("agent");
    let location = root.join("KB");
    let (port, seen) = model_server(|round, _| match round {
        0 => serde_json::json!({
            "choices": [{"message": {"content": null, "tool_calls": [{
                "id": "call_1", "type": "function",
                "function": {"name": "kb_kb_1_hr", "arguments": "{\"query\":\"annual leave days\"}"}
            }]}}],
            "usage": {"prompt_tokens": 40, "completion_tokens": 8}
        })
        .to_string(),
        _ => serde_json::json!({
            "choices": [{"message": {"content": "Twenty working days, per leave.md."}}],
            "usage": {"prompt_tokens": 90, "completion_tokens": 9}
        })
        .to_string(),
    });
    let src = format!(
        r#"{HEADER}
       PROCEDURE DIVISION.
       MAIN.
           MOVE KB-1::AddDocument("leave.md", "Annual leave is twenty working days a year.") TO WS-OK
           PERFORM UNTIL COBOL-QUIT = 1
               CALL "COBOL-WAIT-EVENT" USING COBOL-EVENT-ID COBOL-CONTROL-ID
               EVALUATE COBOL-EVENT-ID
                   WHEN "onIndexed"
                       MOVE AGT-1::AllowKnowledgeBase("KB-1") TO WS-OK
                       DISPLAY "ALLOW=" WS-OK
                       MOVE AGT-1::Ask("How much annual leave do we get?") TO WS-TEXT
                   WHEN "onResponse"
                       MOVE AGT-1::LastReply TO WS-TEXT
                       DISPLAY "REPLY=" WS-TEXT
                       MOVE 1 TO COBOL-QUIT
                   WHEN "onError"
                       MOVE AGT-1::LastError TO WS-TEXT
                       DISPLAY "ERROR=" WS-TEXT
                       MOVE 1 TO COBOL-QUIT
               END-EVALUATE
           END-PERFORM
           STOP RUN.
"#
    );
    let agent: Vec<(String, String)> = [
        ("AgentAPI", "OpenAI"),
        ("AgentURL", &format!("http://127.0.0.1:{port}/v1/chat/completions")),
        ("AgentModel", "test-model"),
        ("TimeoutSeconds", "10"),
    ]
    .iter()
    .map(|(k, v)| (k.to_string(), v.to_string()))
    .collect();
    let t = Instant::now();
    let out = run_with(
        &src,
        vec![
            ("KB-1".into(), "KnowledgeBase".into(), props(&location, &[])),
            ("AGT-1".into(), "AgentObject".into(), agent),
        ],
    );
    let took = t.elapsed();
    assert!(line(&out, "ERROR=").is_empty(), "{out:#?}");
    assert_eq!(line(&out, "ALLOW="), vec!["1"]);
    assert_eq!(line(&out, "REPLY="), vec!["Twenty working days, per leave.md."]);
    let bodies = seen.lock().unwrap().clone();
    assert!(bodies[0].contains("kb_kb_1_hr"), "the tool is offered: {}", bodies[0]);
    assert!(
        bodies[1].contains("document: leave.md") && bodies[1].contains("twenty working days"),
        "the tool result names the document and carries the passage: {}",
        bodies[1]
    );
    println!("\n  ── 068 AgentObject + KnowledgeBase tool ────────────");
    println!("  rounds:   {} request(s) · tool kb_kb_1_hr answered from leave.md", bodies.len());
    println!("  time:     {:.1} ms for the whole question", took.as_secs_f64() * 1000.0);
    println!("  ─────────────────────────────────────────────────────\n");
    let _ = std::fs::remove_dir_all(&root);
}

/// The same question with the collection on an **endpoint** embedder: the
/// model's search needs the embedding server, so it runs on a worker and the
/// agent's tool loop resumes when it returns.
#[test]
fn an_agent_search_on_an_endpoint_embedder_resumes_the_tool_loop() {
    let root = temp("agent-endpoint");
    let location = root.join("KB");
    let chat_round = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let rounds = chat_round.clone();
    let (port, seen) = model_server(move |_, body| {
        if body.contains("\"input\"") {
            // An embedding request (document or query): one fixed vector each.
            let n = body.matches("\"input\"").count().max(1);
            let inputs: usize = serde_json::from_str::<serde_json::Value>(body)
                .ok()
                .and_then(|v| v.get("input").and_then(|i| i.as_array()).map(|a| a.len()))
                .unwrap_or(n);
            let vecs: Vec<Vec<f32>> = (0..inputs).map(|_| vec![1.0, 0.0, 0.0]).collect();
            return serde_json::json!({ "embeddings": vecs }).to_string();
        }
        match rounds.fetch_add(1, std::sync::atomic::Ordering::SeqCst) {
            0 => serde_json::json!({
                "choices": [{"message": {"content": null, "tool_calls": [{
                    "id": "call_1", "type": "function",
                    "function": {"name": "kb_kb_1_hr", "arguments": "{\"query\":\"leave\"}"}
                }]}}]
            })
            .to_string(),
            _ => serde_json::json!({ "choices": [{"message": {"content": "Twenty days."}}] }).to_string(),
        }
    });
    let src = format!(
        r#"{HEADER}
       PROCEDURE DIVISION.
       MAIN.
           MOVE KB-1::AddDocument("leave.md", "Annual leave is twenty working days a year.") TO WS-OK
           PERFORM UNTIL COBOL-QUIT = 1
               CALL "COBOL-WAIT-EVENT" USING COBOL-EVENT-ID COBOL-CONTROL-ID
               EVALUATE COBOL-EVENT-ID
                   WHEN "onIndexed"
                       MOVE KB-1::SearchModeReason TO WS-TEXT
                       DISPLAY "NOTE=" WS-TEXT
                       MOVE AGT-1::AllowKnowledgeBase("KB-1") TO WS-OK
                       MOVE AGT-1::Ask("How much leave?") TO WS-TEXT
                   WHEN "onResponse"
                       MOVE AGT-1::LastReply TO WS-TEXT
                       DISPLAY "REPLY=" WS-TEXT
                       MOVE 1 TO COBOL-QUIT
                   WHEN "onError"
                       MOVE AGT-1::LastError TO WS-TEXT
                       DISPLAY "ERROR=" WS-TEXT
                       MOVE 1 TO COBOL-QUIT
               END-EVALUATE
           END-PERFORM
           STOP RUN.
"#
    );
    let url = format!("http://127.0.0.1:{port}");
    let agent: Vec<(String, String)> = [
        ("AgentAPI", "OpenAI".to_string()),
        ("AgentURL", format!("{url}/v1/chat/completions")),
        ("AgentModel", "test-model".to_string()),
        ("TimeoutSeconds", "10".to_string()),
    ]
    .iter()
    .map(|(k, v)| (k.to_string(), v.clone()))
    .collect();
    let out = run_with(
        &src,
        vec![
            (
                "KB-1".into(),
                "KnowledgeBase".into(),
                props(&location, &[("Embedder", "Endpoint"), ("EmbeddingURL", &url), ("EmbeddingAPI", "Ollama")]),
            ),
            ("AGT-1".into(), "AgentObject".into(), agent),
        ],
    );
    assert!(line(&out, "ERROR=").is_empty(), "{out:#?}");
    assert_eq!(line(&out, "NOTE="), vec![""], "indexed with vectors, no fallback");
    assert_eq!(line(&out, "REPLY="), vec!["Twenty days."]);
    let bodies = seen.lock().unwrap().clone();
    let embeds = bodies.iter().filter(|b| b.contains("\"input\"")).count();
    let chats = bodies.len() - embeds;
    let last_chat = bodies.iter().rev().find(|b| !b.contains("\"input\"")).unwrap();
    assert!(last_chat.contains("document: leave.md") && last_chat.contains("Semantic search"), "{last_chat}");
    println!(
        "\n  ── 068 endpoint tool search: {embeds} embedding request(s), {chats} chat round(s), \
         result named leave.md\n"
    );
    let _ = std::fs::remove_dir_all(&root);
}
