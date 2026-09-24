// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Spec 076 — the application's model list and key store, through COBOL.
//!
//! Programs hand entries over with `COBOL-MODEL-SET`, store keys with
//! `COBOL-KEY-SET`, and point an `AgentObject` (or a `KnowledgeBase`'s endpoint
//! embedder) at an entry with `ModelEntry`. Scripted local servers (no
//! network) record every request, headers included, so each test checks both
//! what the program saw and what went on the wire. The key store is a
//! `MemoryKeyStore` swapped in through the seam (AC5): the programs are
//! unchanged by it. Each test prints one summary block.

use std::io::{Read, Write};
use std::net::TcpListener;
use std::sync::{mpsc, Arc, Mutex, OnceLock};
use std::thread;
use std::time::{Duration, Instant};

use cobolt_lexer::{tokenize, SourceFormat};
use cobolt_parser::{parse, Severity};
use cobolt_runtime::Interpreter;

/// Every test shares one in-memory store, installed once (AC5).
fn memory_store() {
    static ONCE: OnceLock<()> = OnceLock::new();
    ONCE.get_or_init(|| {
        cobolt_runtime::key_store::set_key_store(Arc::new(cobolt_runtime::key_store::MemoryKeyStore::default()))
    });
}

type Script = Box<dyn Fn(usize, &str) -> String + Send>;

/// A server answering every request from `script(round, body)`; returns the
/// port and every request received, whole (headers and body).
fn server(script: Script) -> (u16, Arc<Mutex<Vec<String>>>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let seen = Arc::new(Mutex::new(Vec::new()));
    let log = seen.clone();
    thread::spawn(move || {
        for (round, stream) in listener.incoming().enumerate() {
            let Ok(mut stream) = stream else { break };
            let _ = stream.set_read_timeout(Some(Duration::from_secs(3)));
            let mut buf = Vec::new();
            let mut chunk = [0u8; 4096];
            let head_end = loop {
                match stream.read(&mut chunk) {
                    Ok(0) | Err(_) => break None,
                    Ok(n) => buf.extend_from_slice(&chunk[..n]),
                }
                if let Some(i) = buf.windows(4).position(|w| w == b"\r\n\r\n") {
                    break Some(i + 4);
                }
            };
            let Some(head_end) = head_end else { continue };
            let head = String::from_utf8_lossy(&buf[..head_end]).to_ascii_lowercase();
            let len = head
                .lines()
                .find_map(|l| l.strip_prefix("content-length:"))
                .and_then(|v| v.trim().parse::<usize>().ok())
                .unwrap_or(0);
            while buf.len() < head_end + len {
                match stream.read(&mut chunk) {
                    Ok(0) | Err(_) => break,
                    Ok(n) => buf.extend_from_slice(&chunk[..n]),
                }
            }
            let whole = String::from_utf8_lossy(&buf).into_owned();
            let body = String::from_utf8_lossy(&buf[head_end..]).into_owned();
            log.lock().unwrap().push(whole);
            let reply = script(round, &body);
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

fn chat(text: &str) -> String {
    serde_json::json!({
        "choices": [{"message": {"content": text}}],
        "usage": {"prompt_tokens": 10, "completion_tokens": 3}
    })
    .to_string()
}

fn chat_server(answer: &'static str) -> (String, Arc<Mutex<Vec<String>>>) {
    let (port, seen) = server(Box::new(move |_, _| chat(answer)));
    (format!("http://127.0.0.1:{port}/v1/chat/completions"), seen)
}

const HEADER: &str = r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. MODELS.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 COBOL-EVENT-ID    PIC X(30).
       01 COBOL-CONTROL-ID  PIC X(30).
       01 COBOL-QUIT        PIC 9 VALUE 0.
       01 WS-STEP           PIC 9 VALUE 0.
       01 WS-ST             PIC X(200).
       01 WS-FLAG           PIC X.
       01 WS-TEXT           PIC X(300).
       01 WS-OK             PIC X(4).
"#;

fn run(src: &str, objects: Vec<(&str, &str, Vec<(&str, &str)>)>) -> Vec<String> {
    let result = parse(tokenize(src, SourceFormat::Free));
    assert!(
        result.diagnostics.iter().all(|d| d.severity != Severity::Error),
        "parse errors: {:?}",
        result.diagnostics
    );
    let program = result.program.expect("no program");
    let objects: Vec<(String, String, Vec<(String, String)>)> = objects
        .into_iter()
        .map(|(id, ty, props)| {
            (
                id.to_string(),
                ty.to_string(),
                props.into_iter().map(|(k, v)| (k.to_string(), v.to_string())).collect(),
            )
        })
        .collect();
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
    done_rx.recv_timeout(Duration::from_secs(30)).expect("the program did not finish within 30 s")
}

fn lines<'a>(out: &'a [String], prefix: &str) -> Vec<&'a str> {
    out.iter().filter_map(|l| l.strip_prefix(prefix)).map(str::trim).collect()
}

fn agent(own_url: &str, verbose: bool) -> Vec<(&'static str, String)> {
    vec![
        ("AgentAPI", "OpenAI".into()),
        ("AgentURL", own_url.into()),
        ("AgentModel", "own-model".into()),
        ("AgentAPIKey", "".into()),
        ("TimeoutSeconds", "10".into()),
        ("Verbose", if verbose { "true" } else { "false" }.into()),
    ]
}

fn leak(props: Vec<(&'static str, String)>) -> Vec<(&'static str, &'static str)> {
    props.into_iter().map(|(k, v)| (k, &*Box::leak(v.into_boxed_str()))).collect()
}

fn report(test: &str, rows: &[String], t: Duration) {
    println!("\n  ── 076 {test} ─────────────────────────────");
    for r in rows {
        println!("  {r}");
    }
    println!("  time:     {:.1} ms", t.as_secs_f64() * 1000.0);
    println!("  ───────────────────────────────────────────────\n");
}

/// AC1, AC3, AC4, AC5, AC10 (missing key), AC11 — two entries handed over,
/// an agent repointed between questions, a key rotated and removed, a change
/// to the entry in use raising `onModelChanged`, and the key nowhere a
/// program, a log or an error can show it.
#[test]
fn entries_keys_repointing_and_change_notices() {
    memory_store();
    const KEY_A: &str = "sk-AAAA-company-secret";
    const KEY_B: &str = "sk-BBBB-backup-secret";
    const KEY_C: &str = "sk-CCCC-rotated-secret";
    let (url1, seen1) = chat_server("From the company model.");
    let (url2, seen2) = chat_server("From the backup model.");
    let (own, seen_own) = chat_server("From my own settings.");
    let src = format!(
        r#"{HEADER}
       PROCEDURE DIVISION.
       MAIN.
           CALL "COBOL-MODEL-SET" USING "company-t1" "OpenAI" "{url1}" "model-one" WS-ST
           DISPLAY "SET1=" WS-ST
           CALL "COBOL-MODEL-SET" USING "backup-t1" "OpenAI" "{url2}" " " WS-ST
           CALL "COBOL-KEY-SET" USING "company-t1" "{KEY_A}" WS-ST
           DISPLAY "KEY1=" WS-ST
           CALL "COBOL-KEY-SET" USING "backup-t1" "{KEY_B}"
           CALL "COBOL-KEY-IS-SET" USING "company-t1" WS-FLAG
           DISPLAY "ISSET=" WS-FLAG
           MOVE "company-t1" TO AGT-1::ModelEntry
           MOVE AGT-1::Ask("Question one") TO WS-TEXT
           PERFORM UNTIL COBOL-QUIT = 1
               CALL "COBOL-WAIT-EVENT" USING COBOL-EVENT-ID COBOL-CONTROL-ID
               EVALUATE COBOL-EVENT-ID
                   WHEN "onResponse"
                       ADD 1 TO WS-STEP
                       MOVE AGT-1::LastReply TO WS-TEXT
                       DISPLAY "REPLY=" WS-TEXT
                       EVALUATE WS-STEP
                           WHEN 1
                               MOVE "backup-t1" TO AGT-1::ModelEntry
                               CALL "COBOL-KEY-SET" USING "backup-t1" "{KEY_C}"
                               MOVE AGT-1::Ask("Question two") TO WS-TEXT
                           WHEN 2
                               CALL "COBOL-KEY-REMOVE" USING "backup-t1" WS-ST
                               CALL "COBOL-KEY-IS-SET" USING "backup-t1" WS-FLAG
                               DISPLAY "ISSET2=" WS-FLAG
                               CALL "COBOL-MODEL-SET" USING "backup-t1" "OpenAI" "{url2}" "model-two"
                           WHEN OTHER
                               MOVE 1 TO COBOL-QUIT
                       END-EVALUATE
                   WHEN "onModelChanged"
                       DISPLAY "CHANGED=" COBOL-CONTROL-ID
                       MOVE AGT-1::Ask("Question three") TO WS-TEXT
                   WHEN "onError"
                       MOVE AGT-1::LastError TO WS-TEXT
                       DISPLAY "ERROR=" WS-TEXT
                       MOVE " " TO AGT-1::ModelEntry
                       MOVE AGT-1::Ask("Question four") TO WS-TEXT
               END-EVALUATE
           END-PERFORM.
           STOP RUN.
"#
    );
    let t = Instant::now();
    let out = run(&src, vec![("AGT-1", "AgentObject", leak(agent(&own, true)))]);
    let took = t.elapsed();
    let (r1, r2, r3) = (seen1.lock().unwrap().clone(), seen2.lock().unwrap().clone(), seen_own.lock().unwrap().clone());

    assert_eq!(lines(&out, "SET1="), ["OK"]);
    assert_eq!(lines(&out, "KEY1="), ["OK"]);
    assert_eq!(lines(&out, "ISSET="), ["Y"], "AC3: only set / not set");
    assert_eq!(lines(&out, "ISSET2="), ["N"]);
    // AC1 — each question went where its entry pointed, with its key and model.
    assert_eq!(r1.len(), 1, "question one → company");
    assert!(r1[0].contains(&format!("Bearer {KEY_A}")) && r1[0].contains("\"model-one\""));
    assert_eq!(r2.len(), 1, "question two → backup; question three never sent");
    assert!(r2[0].contains(&format!("Bearer {KEY_C}")), "AC3: the rotated key, from the next request");
    assert!(r2[0].contains("\"own-model\""), "an entry naming no model keeps the agent's own");
    assert_eq!(lines(&out, "REPLY=")[..2], ["From the company model.", "From the backup model."]);
    // AC11 — changing the entry in use raised onModelChanged.
    assert_eq!(lines(&out, "CHANGED="), ["AGT-1"]);
    // AC10 — the changed entry has no key now, and its API needs one.
    let errors = lines(&out, "ERROR=");
    assert_eq!(errors.len(), 1, "{out:#?}");
    assert!(errors[0].contains("backup-t1") && errors[0].contains("no key"), "{}", errors[0]);
    // With no entry, the agent's own settings (AC9's last step).
    assert_eq!(r3.len(), 1);
    assert_eq!(lines(&out, "REPLY=").last().copied(), Some("From my own settings."));
    // AC4 — no key anywhere the program or its log shows (verbose is on).
    for secret in [KEY_A, KEY_B, KEY_C] {
        assert!(out.iter().all(|l| !l.contains(secret)), "{secret} leaked into: {out:#?}");
    }
    assert!(out.iter().any(|l| l.contains("header: Authorization: ****")), "the verbose log masks it");

    report(
        "list, keys, repointing, change notice",
        &[
            "entries: company-t1 (model-one), backup-t1 (no model → own-model)".into(),
            format!("requests: company 1 (key A), backup 1 (rotated key C), own settings {}", r3.len()),
            "key removed + entry changed → onModelChanged → onError (no key), nothing sent".into(),
            format!("{} display lines searched: no key; verbose headers masked", out.len()),
        ],
        took,
    );
}

/// AC10 — an unknown entry fails at once with onError naming it; nothing is
/// sent. AC9 — an entry wins over the agent's own settings.
#[test]
fn an_unknown_entry_fails_at_once_and_a_known_one_wins() {
    memory_store();
    let (own, seen_own) = chat_server("own");
    let (entry_url, seen_entry) = chat_server("entry");
    let src = format!(
        r#"{HEADER}
       PROCEDURE DIVISION.
       MAIN.
           MOVE "nobody-t2" TO AGT-1::ModelEntry
           MOVE AGT-1::Ask("Hello") TO WS-TEXT
           PERFORM UNTIL COBOL-QUIT = 1
               CALL "COBOL-WAIT-EVENT" USING COBOL-EVENT-ID COBOL-CONTROL-ID
               EVALUATE COBOL-EVENT-ID
                   WHEN "onError"
                       MOVE AGT-1::LastError TO WS-TEXT
                       DISPLAY "ERROR=" WS-TEXT
                       CALL "COBOL-MODEL-SET" USING "local-t2" "Ollama" "{entry_url}" "llama"
                       MOVE "local-t2" TO AGT-1::ModelEntry
                       MOVE AGT-1::Ask("Hello again") TO WS-TEXT
                   WHEN "onResponse"
                       MOVE AGT-1::LastReply TO WS-TEXT
                       DISPLAY "REPLY=" WS-TEXT
                       MOVE 1 TO COBOL-QUIT
               END-EVALUATE
           END-PERFORM.
           STOP RUN.
"#
    );
    let t = Instant::now();
    let out = run(&src, vec![("AGT-1", "AgentObject", leak(agent(&own, false)))]);
    let err = lines(&out, "ERROR=");
    assert_eq!(err.len(), 1);
    assert!(err[0].contains("nobody-t2") && err[0].contains("does not exist"), "{}", err[0]);
    assert!(seen_own.lock().unwrap().is_empty(), "nothing sent for the unknown entry");
    assert_eq!(lines(&out, "REPLY="), ["entry"], "the entry wins over the agent's own URL");
    let sent = seen_entry.lock().unwrap().clone();
    assert!(sent[0].contains("\"llama\"") && !sent[0].to_ascii_lowercase().contains("authorization: bearer"),
        "an Ollama entry needs no key and sends none");
    report(
        "unknown entry, precedence",
        &["nobody-t2 → onError at once, 0 requests".into(), "local-t2 (Ollama, no key) → answered; own settings untouched".into()],
        t.elapsed(),
    );
}

/// AC8 — a KnowledgeBase's endpoint embedder takes its endpoint, model and key
/// from a list entry.
#[test]
fn a_knowledge_base_embeds_through_an_entry() {
    memory_store();
    let (port, seen) = server(Box::new(|_, body| {
        let n = serde_json::from_str::<serde_json::Value>(body)
            .ok()
            .and_then(|v| v["input"].as_array().map(|a| a.len()))
            .unwrap_or(1);
        let vectors: Vec<Vec<f32>> = (0..n).map(|i| vec![1.0, i as f32]).collect();
        serde_json::json!({ "embeddings": vectors }).to_string()
    }));
    let dir = std::env::temp_dir().join(format!(
        "prc-076-kb-{}",
        std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()
    ));
    let src = format!(
        r#"{HEADER}
       PROCEDURE DIVISION.
       MAIN.
           CALL "COBOL-MODEL-SET" USING "embed-t3" "Ollama" "http://127.0.0.1:{port}" "embed-model-x"
           CALL "COBOL-KEY-SET" USING "embed-t3" "sk-EMBED-secret"
           MOVE KB-1::AddDocument("leave.md", "Leave policy. Twenty days a year.") TO WS-OK
           DISPLAY "START=" WS-OK
           PERFORM UNTIL COBOL-QUIT = 1
               CALL "COBOL-WAIT-EVENT" USING COBOL-EVENT-ID COBOL-CONTROL-ID
               EVALUATE COBOL-EVENT-ID
                   WHEN "onIndexed"
                       DISPLAY "INDEXED"
                       MOVE 1 TO COBOL-QUIT
                   WHEN "onError"
                       MOVE KB-1::LastError TO WS-TEXT
                       DISPLAY "ERROR=" WS-TEXT
                       MOVE 1 TO COBOL-QUIT
               END-EVALUATE
           END-PERFORM.
           STOP RUN.
"#
    );
    let t = Instant::now();
    let loc = dir.display().to_string();
    let out = run(
        &src,
        vec![(
            "KB-1",
            "KnowledgeBase",
            leak(vec![
                ("Location", loc),
                ("Collection", "hr".into()),
                ("Embedder", "Endpoint".into()),
                ("EmbeddingURL", "http://127.0.0.1:9".into()),
                ("ModelEntry", "embed-t3".into()),
            ]),
        )],
    );
    assert_eq!(lines(&out, "START="), ["1"], "{out:#?}");
    assert!(out.iter().any(|l| l == "INDEXED"), "{out:#?}");
    let sent = seen.lock().unwrap().clone();
    assert!(!sent.is_empty(), "the entry's endpoint was used, not EmbeddingURL");
    assert!(sent[0].contains("/api/embed") && sent[0].contains("embed-model-x"));
    assert!(out.iter().all(|l| !l.contains("sk-EMBED-secret")));
    report("KnowledgeBase via an entry", &[format!("{} embedding request(s) to the entry's endpoint, model embed-model-x", sent.len())], t.elapsed());
    let _ = std::fs::remove_dir_all(&dir);
}
