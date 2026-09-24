// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Spec 075 — `AgentObject::RegisterFile`, end to end.
//!
//! A COBOL program with **no FD** registers an indexed file by its path, and a
//! scripted local model server (no network) searches it. Every test checks the
//! registered file is byte-identical, with the same modification time,
//! afterwards (AC7). Each test prints what was registered, how, and how long
//! it took; the last one measures open and search throughput (AC14).

use std::io::{Read, Write};
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::sync::{mpsc, Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant, SystemTime};

use cobolt_lexer::{tokenize, SourceFormat};
use cobolt_parser::{parse, Severity};
use cobolt_runtime::indexed::{status, IndexedStore, KeySpec, OpenMode};
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

// ── Fixtures ───────────────────────────────────────────────────────────────────

const RECORD_LEN: usize = 111;

fn temp(tag: &str) -> PathBuf {
    let nanos = SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
    let d = std::env::temp_dir().join(format!("prc-075-{tag}-{nanos}"));
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn record(id: u64, salary: &str) -> Vec<u8> {
    let mut rec = vec![b' '; RECORD_LEN];
    rec[0..9].copy_from_slice(format!("{id:>9}").as_bytes());
    rec[99..110].copy_from_slice(format!("{salary:>11}").as_bytes());
    rec
}

fn primary() -> KeySpec {
    KeySpec { offset: 0, len: 9, duplicates: false }
}

/// `rows` actors in a DISK (`PRCIDXD1`) or MEMORY (`PRCIDX1`) container;
/// every other one earns 100000.
fn build_actors(path: &Path, rows: u64, disk: bool) {
    let mut f: Box<dyn IndexedStore> = if disk {
        Box::new(cobolt_runtime::indexed_disk::DiskIndexedFile::new(path, RECORD_LEN, primary(), Vec::new()))
    } else {
        // WITH PERSISTENCE, or a MEMORY file keeps its records to itself.
        let mut f = cobolt_runtime::indexed::IndexedFile::new(path, RECORD_LEN, primary(), Vec::new());
        f.set_persist(true);
        Box::new(f)
    };
    assert_eq!(f.open(OpenMode::Output), status::OK);
    for id in 1..=rows {
        let salary = if id % 2 == 1 { "100000" } else { "250000" };
        assert_eq!(f.write(&record(id, salary)), status::OK);
    }
    f.close();
}

fn cidx(length: usize, purpose: &str) -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?><IndexedFile name="ACTORS-FILE" finalized="true" version="1.0"><assign-path>actors.idx</assign-path><access-mode>dynamic</access-mode><record-format fixed-length="{length}"/><storage mode="disk" compression="false" persistence="false"/><comment><![CDATA[{purpose}]]></comment><keys><primary duplicates="false" ordering="ascending"><part field="ACTOR-ID" offset="0" length="9" encoding="bytes"/></primary></keys><fields><Field level="1" name="ACTORS-RECORD" usage="display"><Field level="5" name="ACTOR-ID" pic="9(9)" usage="display" offset="0" length="9"><comment><![CDATA[The key]]></comment></Field><Field level="5" name="ACTOR-SALARY" pic="9(9)V99" usage="display" offset="99" length="11"><comment><![CDATA[Annual salary]]></comment></Field></Field></fields></IndexedFile>"#
    )
}

/// A data file and its `.cidx` in a fresh folder.
fn fixture(tag: &str, rows: u64, disk: bool) -> (PathBuf, PathBuf, PathBuf) {
    let dir = temp(tag);
    let data = dir.join("actors.idx");
    let def = dir.join("actors.cidx");
    build_actors(&data, rows, disk);
    std::fs::write(&def, cidx(RECORD_LEN, "One row per performer")).unwrap();
    (dir, data, def)
}

/// Bytes and modification time — what AC7 compares before and after.
fn snapshot(p: &Path) -> Option<(Vec<u8>, SystemTime)> {
    Some((std::fs::read(p).ok()?, std::fs::metadata(p).ok()?.modified().ok()?))
}

// ── The COBOL side ─────────────────────────────────────────────────────────────

/// Register (and maybe withdraw) a file, show the outcome, ask one question.
fn program(setup: &str) -> String {
    format!(
        r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. REGISTERED.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 COBOL-EVENT-ID    PIC X(30).
       01 COBOL-CONTROL-ID  PIC X(30).
       01 COBOL-QUIT        PIC 9 VALUE 0.
       01 WS-OK             PIC X(4).
       01 WS-RES            PIC X(40).
       01 WS-NAME           PIC X(40).
       01 WS-N              PIC X(20).
       01 WS-TEXT           PIC X(300).
       PROCEDURE DIVISION.
       MAIN.
{setup}
           MOVE AGT-1::RegisterResult TO WS-RES
           DISPLAY "RESULT=" WS-RES
           MOVE AGT-1::RegisteredName TO WS-NAME
           DISPLAY "NAME=" WS-NAME
           MOVE AGT-1::RegisterFileBytes TO WS-N
           DISPLAY "BYTES=" WS-N
           MOVE AGT-1::RegisterLimitBytes TO WS-N
           DISPLAY "LIMIT=" WS-N
           MOVE AGT-1::RegisterMessage TO WS-TEXT
           DISPLAY "MSG=" WS-TEXT
           MOVE AGT-1::Ask("How many actors earn 100000?") TO WS-TEXT
           PERFORM UNTIL COBOL-QUIT = 1
               CALL "COBOL-WAIT-EVENT" USING COBOL-EVENT-ID COBOL-CONTROL-ID
               EVALUATE COBOL-EVENT-ID
                   WHEN "onResponse"
                       MOVE AGT-1::LastReply TO WS-TEXT
                       DISPLAY "REPLY=" WS-TEXT
                       MOVE 1 TO COBOL-QUIT
                   WHEN "onError"
                       MOVE AGT-1::LastError TO WS-TEXT
                       DISPLAY "ERROR=" WS-TEXT
                       MOVE 1 TO COBOL-QUIT
               END-EVALUATE
           END-PERFORM.
           STOP RUN.
"#
    )
}

fn register(data: &Path, def: &Path) -> String {
    format!(
        r#"           MOVE AGT-1::RegisterFile("{}", "{}") TO WS-OK
           DISPLAY "REG=" WS-OK"#,
        data.display(),
        def.display()
    )
}

struct Run {
    out: Vec<String>,
    requests: Vec<String>,
    took: Duration,
}

impl Run {
    fn line(&self, prefix: &str) -> &str {
        self.out.iter().find_map(|l| l.strip_prefix(prefix)).map(str::trim).unwrap_or("")
    }
    /// The first request's tools, by name.
    fn offered(&self) -> Vec<String> {
        let first: serde_json::Value = serde_json::from_str(&self.requests[0]).unwrap();
        first["tools"]
            .as_array()
            .map(|t| t.iter().filter_map(|x| x["function"]["name"].as_str().map(String::from)).collect())
            .unwrap_or_default()
    }
    /// What the file search answered, when the model called it.
    fn tool_result(&self) -> Option<String> {
        let last: serde_json::Value = serde_json::from_str(self.requests.last()?).ok()?;
        last["messages"]
            .as_array()?
            .iter()
            .find(|m| m["role"] == "tool")
            .and_then(|m| m["content"].as_str().map(String::from))
    }
}

/// Run `setup` then an `Ask`, against a model that searches the actors file
/// when offered it and answers in words otherwise.
fn run(setup: &str, limit: Option<u64>, free: Option<u64>) -> Run {
    let (port, seen) = model_server(Box::new(|round, body| {
        if round == 0 && body.contains("search_actors_file") {
            (200, openai_call("call_1", "search_actors_file", r#"{"ACTOR-SALARY":"100000"}"#))
        } else {
            (200, openai_text("Answered."))
        }
    }));
    let url = format!("http://127.0.0.1:{port}/v1/chat/completions");
    let src = program(setup);
    let result = parse(tokenize(&src, SourceFormat::Free));
    assert!(
        result.diagnostics.iter().all(|d| d.severity != Severity::Error),
        "parse errors: {:?}",
        result.diagnostics
    );
    let program = result.program.expect("no program");
    let props: Vec<(String, String)> = [
        ("AgentAPI", "OpenAI"),
        ("AgentURL", url.as_str()),
        ("AgentModel", "test-model"),
        ("SystemPrompt", "Be brief."),
        ("TimeoutSeconds", "10"),
    ]
    .iter()
    .map(|(k, v)| (k.to_string(), v.to_string()))
    .collect();
    let started = Instant::now();
    let (done_tx, done_rx) = mpsc::channel();
    thread::spawn(move || {
        let (_event_tx, event_rx) = mpsc::channel();
        let (state_tx, _state_rx) = mpsc::channel();
        let (display_tx, display_rx) = mpsc::channel();
        let mut interp = Interpreter::new_with_channels(program, event_rx, state_tx, display_tx);
        if let Some(bytes) = limit {
            interp.mcp_tools_mut().set_memory_limit(bytes);
        }
        interp.set_free_memory_probe(free);
        interp.seed_objects(vec![("AGT-1".into(), "AgentObject".into(), props)]);
        let _ = interp.run();
        let lines: Vec<String> = display_rx.try_iter().map(|l| l.trim().to_owned()).collect();
        let _ = done_tx.send(lines);
    });
    let out = done_rx.recv_timeout(Duration::from_secs(30)).expect("the program did not finish within 30 s");
    let requests = seen.lock().unwrap().clone();
    Run {
        out,
        requests,
        took: started.elapsed(),
    }
}

fn report(test: &str, lines: &[String], took: Duration) {
    println!("\n  ── 075 {test} ─────────────────────────────");
    for l in lines {
        println!("  {l}");
    }
    println!("  time:     {:.1} ms for the whole program", took.as_secs_f64() * 1000.0);
    println!("  ───────────────────────────────────────────────\n");
}

// ── Tests ──────────────────────────────────────────────────────────────────────

/// AC1, AC7 — a program with no FD registers a file by path; the model's
/// search answers from it; the file is untouched.
#[test]
fn a_file_registered_by_path_is_searched_for_the_model() {
    let (dir, data, def) = fixture("ac1", 3, true);
    let before = (snapshot(&data), snapshot(&def));
    let r = run(&register(&data, &def), None, None);
    assert_eq!(r.line("REG="), "1", "{:?}", r.out);
    assert_eq!(r.line("RESULT="), "MEMORY");
    assert_eq!(r.line("NAME="), "ACTORS-FILE");
    assert_eq!(r.offered(), ["search_actors_file"]);
    let result = r.tool_result().expect("the model searched the file");
    assert!(result.contains("ACTORS-FILE: 2 record(s)"), "{result}");
    assert_eq!(r.line("REPLY="), "Answered.");
    assert!(
        r.requests.iter().all(|b| !b.contains(&dir.display().to_string())),
        "no path reaches the model"
    );
    assert_eq!((snapshot(&data), snapshot(&def)), before, "AC7: byte-identical, same mtime");
    report(
        "no FD, RegisterFile → search",
        &[format!("registered: ACTORS-FILE, 3 records, held in memory ({} bytes)", r.line("BYTES=")), format!("search:     {}", result.lines().next().unwrap_or(""))],
        r.took,
    );
    let _ = std::fs::remove_dir_all(&dir);
}

/// AC2 — a .cidx that does not describe the file is refused, with its code,
/// before the model sees anything.
#[test]
fn a_cidx_that_does_not_describe_the_file_is_refused() {
    let (dir, data, _) = fixture("ac2", 3, true);
    let cases = [
        ("length.cidx", cidx(120, "One row per performer"), "RECORD-LENGTH-MISMATCH"),
        ("purpose.cidx", cidx(RECORD_LEN, ""), "NO-PURPOSE"),
        ("keys.cidx", cidx(RECORD_LEN, "x").replace(r#"offset="0" length="9" encoding"#, r#"offset="1" length="8" encoding"#), "KEY-MISMATCH"),
        ("empty.cidx", "<IndexedFile name=\"X\"><fields>".into(), "NO-FIELDS"),
    ];
    let before = snapshot(&data);
    let mut lines = Vec::new();
    for (file, text, want) in cases {
        let def = dir.join(file);
        std::fs::write(&def, text).unwrap();
        let r = run(&register(&data, &def), None, None);
        assert_eq!((r.line("REG="), r.line("RESULT=")), ("0", want), "{file}: {:?}", r.out);
        assert!(r.offered().is_empty(), "{file}: no tool offered");
        assert_eq!(r.line("REPLY="), "Answered.", "the model still answers");
        lines.push(format!("{file:<13} → {want}"));
    }
    assert_eq!(snapshot(&data), before);
    report("cidx refusals", &lines, Duration::ZERO);
    let _ = std::fs::remove_dir_all(&dir);
}

/// AC3 — a withdrawn registration is no longer offered.
#[test]
fn an_unregistered_file_is_no_longer_offered() {
    let (dir, data, def) = fixture("ac3", 3, true);
    let setup = format!(
        "{}\n           MOVE AGT-1::UnregisterFile(\"ACTORS-FILE\") TO WS-OK\n           DISPLAY \"UNREG=\" WS-OK",
        register(&data, &def)
    );
    let r = run(&setup, None, None);
    assert_eq!((r.line("REG="), r.line("UNREG=")), ("1", "1"));
    assert!(r.offered().is_empty());
    let _ = std::fs::remove_dir_all(&dir);
}

/// AC5 (local), AC8, AC9 — a missing file, a read-only file in a read-only
/// folder, and a file with a recovery journal beside it.
#[test]
#[cfg(unix)]
fn missing_read_only_and_journalled_files() {
    use std::os::unix::fs::PermissionsExt;
    let (dir, data, def) = fixture("ac589", 3, true);

    let r = run(&register(&dir.join("gone.idx"), &def), None, None);
    assert_eq!((r.line("REG="), r.line("RESULT=")), ("0", "NOT-FOUND"));
    assert!(r.line("MSG=").contains("gone.idx"), "named: {}", r.line("MSG="));
    assert_eq!(r.line("REPLY="), "Answered.", "the model still answers");

    std::fs::set_permissions(&data, std::fs::Permissions::from_mode(0o444)).unwrap();
    std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o555)).unwrap();
    let before = snapshot(&data);
    let r = run(&register(&data, &def), None, None);
    assert_eq!((r.line("REG="), r.line("RESULT=")), ("1", "MEMORY"), "{:?}", r.out);
    let limited = run(&register(&data, &def), Some(1024), None);
    assert_eq!(limited.line("RESULT="), "DISK", "read in place, read-only");
    assert!(limited.tool_result().unwrap().contains("2 record(s)"));
    assert_eq!(snapshot(&data), before);
    std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o755)).unwrap();

    let mut jrn = data.clone().into_os_string();
    jrn.push(".jrn");
    std::fs::write(&jrn, b"PRCJRN01 left by a crash").unwrap();
    let jrn_before = snapshot(Path::new(&jrn));
    let r = run(&register(&data, &def), None, None);
    assert_eq!((r.line("REG="), r.line("RESULT=")), ("0", "JOURNAL-PRESENT"));
    assert_eq!((snapshot(&data), snapshot(Path::new(&jrn))), (before, jrn_before), "AC9: both untouched");

    std::fs::set_permissions(&data, std::fs::Permissions::from_mode(0o644)).unwrap();
    report(
        "missing / read-only / journal",
        &[
            "gone.idx          → NOT-FOUND, model still answered".into(),
            "0444 file, 0555 dir → MEMORY, and DISK at a 1 KiB limit".into(),
            "actors.idx.jrn    → JOURNAL-PRESENT, file and journal unchanged".into(),
        ],
        r.took,
    );
    let _ = std::fs::remove_dir_all(&dir);
}

/// AC10, AC11, AC12 — the fit test: under the limits in memory; over the
/// project limit or over half the free memory, a DISK file is read in place;
/// a MEMORY file is refused with both numbers.
#[test]
fn the_fit_test_through_a_program() {
    let (dir, data, def) = fixture("fit", 500, true);
    let size = std::fs::metadata(&data).unwrap().len();
    let before = snapshot(&data);

    let under = run(&register(&data, &def), Some(size + 1), Some(4 * size));
    assert_eq!(under.line("RESULT="), "MEMORY");
    let over_limit = run(&register(&data, &def), Some(size - 1), Some(u64::MAX / 2));
    assert_eq!(over_limit.line("RESULT="), "DISK", "AC10");
    // 250 match; a search returns 100 unless asked for more.
    assert!(over_limit.tool_result().unwrap().contains("100 record(s) (truncated at 100"), "searched in place");
    let short_of_memory = run(&register(&data, &def), Some(u64::MAX / 2), Some(size));
    assert_eq!(short_of_memory.line("RESULT="), "DISK", "AC11: half the free memory decides");
    assert_eq!(snapshot(&data), before);

    let mem = dir.join("actors-mem.idx");
    build_actors(&mem, 500, false);
    let mem_size = std::fs::metadata(&mem).unwrap().len();
    let refused = run(&register(&mem, &def), Some(1024), None);
    assert_eq!((refused.line("REG="), refused.line("RESULT=")), ("0", "TOO-LARGE-FOR-LIMIT"), "AC12");
    assert_eq!(refused.line("BYTES="), mem_size.to_string());
    assert_eq!(refused.line("LIMIT="), "1024");
    let in_mem = run(&register(&mem, &def), None, None);
    assert_eq!(in_mem.line("RESULT="), "MEMORY");
    assert!(in_mem.tool_result().unwrap().contains("100 record(s) (truncated at 100"), "a PRCIDX1 file searched from memory");

    report(
        "fit test",
        &[
            format!("PRCIDXD1 {size} bytes: limit {} → MEMORY; limit {} → DISK; free {size} → DISK", size + 1, size - 1),
            format!("PRCIDX1 {mem_size} bytes: limit 1024 → TOO-LARGE-FOR-LIMIT ({mem_size} > 1024); default → MEMORY"),
        ],
        under.took,
    );
    let _ = std::fs::remove_dir_all(&dir);
}

/// AC14 — open and search throughput, in memory and from disk, on 20,000
/// records, through the same API the interpreter uses.
#[test]
fn registration_and_search_throughput() {
    use cobolt_runtime::mcp_tool::IndexedToolSet;
    use cobolt_runtime::registered_file as reg;
    const ROWS: u64 = 20_000;
    let (dir, data, def) = fixture("throughput", ROWS, true);
    let size = std::fs::metadata(&data).unwrap().len();
    let fetch = reg::Fetcher { smb: None };
    let query = serde_json::json!({ "ACTOR-SALARY": "250000", "limit": 1000 });
    let mut lines = vec![format!("file: {ROWS} records, {size} bytes (PRCIDXD1)")];
    for (label, limit) in [("memory", u64::MAX / 2), ("disk  ", 1024)] {
        let t = Instant::now();
        let r = reg::register(
            &data.display().to_string(),
            &def.display().to_string(),
            "",
            limit,
            &reg::FixedMemory(u64::MAX),
            &fetch,
        )
        .unwrap();
        let open_ms = t.elapsed().as_secs_f64() * 1000.0;
        let mut set = IndexedToolSet::new();
        set.register(r.description, r.access, r.source);
        const SEARCHES: u32 = 20;
        let t = Instant::now();
        for _ in 0..SEARCHES {
            let res = set.call("search_actors_file", &query);
            assert_ne!(res.is_error, Some(true));
        }
        let per_s = SEARCHES as f64 / t.elapsed().as_secs_f64();
        lines.push(format!(
            "{label}: register {open_ms:.1} ms · {per_s:.1} searches/s over {ROWS} records ({:.0} records/s)",
            per_s * ROWS as f64
        ));
    }
    report("throughput", &lines, Duration::ZERO);
    let _ = std::fs::remove_dir_all(&dir);
}

// ── smb:// (T10) ───────────────────────────────────────────────────────────────

use cobolt_runtime::registered_file::{self as reg, Fetch, Location, Refusal};

/// A share in memory: files by `share/path`, or one refusal for everything.
struct FakeShare {
    files: std::collections::HashMap<String, Vec<u8>>,
    refuse: Option<(&'static str, &'static str)>,
    asked: Mutex<Vec<String>>,
}

impl FakeShare {
    fn key(at: &Location) -> String {
        match at {
            Location::Smb(u) => format!("{}/{}", u.share, u.path),
            Location::Fs(_) => panic!("the share is asked only for smb://"),
        }
    }
}

impl Fetch for FakeShare {
    fn size(&self, at: &Location) -> Result<Option<u64>, Refusal> {
        self.asked.lock().unwrap().push(at.display());
        if let Some((code, msg)) = self.refuse {
            return Err(Refusal::new(code, format!("{}: {msg}", at.display())));
        }
        Ok(self.files.get(&Self::key(at)).map(|b| b.len() as u64))
    }
    fn read(&self, at: &Location, max: u64) -> Result<Vec<u8>, Refusal> {
        self.asked.lock().unwrap().push(at.display());
        let bytes = self.files.get(&Self::key(at)).cloned().ok_or_else(|| Refusal::new("NOT-FOUND", "gone"))?;
        assert!(bytes.len() as u64 <= max, "never asked for more than allowed");
        Ok(bytes)
    }
}

/// AC4, AC6, AC12 — the same file through a share gives the same search as
/// through its local path; the password shows up nowhere a program or a model
/// can see; a share file too large for memory is refused before download.
#[test]
fn a_share_file_searches_like_a_local_one_and_hides_its_password() {
    use cobolt_runtime::mcp_tool::IndexedToolSet;
    const SECRET: &str = "Tr0ub4dor&3";
    let mut summary = Vec::new();
    for disk in [true, false] {
        let (dir, data, def) = fixture(if disk { "smb-disk" } else { "smb-mem" }, 40, disk);
        let before = snapshot(&data);
        let share = FakeShare {
            files: [
                ("data/actors.idx".to_string(), std::fs::read(&data).unwrap()),
                ("data/actors.cidx".to_string(), std::fs::read(&def).unwrap()),
            ]
            .into(),
            refuse: None,
            asked: Mutex::new(Vec::new()),
        };
        let fetch = reg::Fetcher { smb: Some(&share) };
        let free = reg::FixedMemory(u64::MAX / 2);
        let search = |r: reg::Registered| {
            let mut set = IndexedToolSet::new();
            set.register(r.description, r.access, r.source);
            format!("{:?}", set.call("search_actors_file", &serde_json::json!({"ACTOR-SALARY": "100000"})).content)
        };
        let local = search(reg::register(&data.display().to_string(), &def.display().to_string(), "", 64 << 20, &free, &fetch).unwrap());
        let url = format!("smb://CORP;ana:{SECRET}@finance/data/actors.idx");
        let remote = reg::register(&url, &format!("smb://CORP;ana:{SECRET}@finance/data/actors.cidx"), "", 64 << 20, &free, &fetch).unwrap();
        assert_eq!(remote.fit, reg::Fit::Memory);
        let shown = format!("{remote:?}");
        assert!(!shown.contains(SECRET), "AC6: {shown}");
        assert_eq!(search(remote), local, "AC4: the same answer");
        let asked = share.asked.lock().unwrap().join(" ");
        assert!(!asked.contains(SECRET) && asked.contains("ana:****@"), "{asked}");
        let refused = reg::register(&url, &format!("smb://finance/data/actors.cidx"), "", 1024, &free, &fetch).unwrap_err();
        assert_eq!(refused.code, "TOO-LARGE-FOR-LIMIT", "AC12");
        assert!(refused.bound == 1024 && refused.size > 1024);
        assert!(!refused.message.contains(SECRET));
        assert_eq!(snapshot(&data), before);
        summary.push(format!(
            "{}: smb:// search == local search; 1 KiB limit → TOO-LARGE-FOR-LIMIT ({} > 1024)",
            if disk { "PRCIDXD1" } else { "PRCIDX1 " },
            refused.size
        ));
        let _ = std::fs::remove_dir_all(&dir);
    }

    // AC5 — an unreachable server and a refused login, by name and reason.
    for (code, what) in [("UNREACHABLE", "server down"), ("ACCESS-DENIED", "login refused")] {
        let share = FakeShare { files: Default::default(), refuse: Some((code, what)), asked: Mutex::new(Vec::new()) };
        let r = reg::register(
            &format!("smb://ana:{SECRET}@finance/data/actors.idx"),
            "smb://finance/data/actors.cidx",
            "",
            64 << 20,
            &reg::FixedMemory(u64::MAX / 2),
            &reg::Fetcher { smb: Some(&share) },
        )
        .unwrap_err();
        assert_eq!(r.code, code);
        assert!(r.message.contains("finance/data/actors.idx") && !r.message.contains(SECRET), "{}", r.message);
        summary.push(format!("{what:<14} → {code}"));
    }
    report("smb:// through a stand-in share", &summary, Duration::ZERO);
}

/// AC5 against the real client — a closed port answers UNREACHABLE quickly,
/// and a build without the feature says so.
#[test]
fn the_real_smb_client_reports_an_unreachable_server() {
    let t = Instant::now();
    let r = reg::register(
        "smb://ana:hunter2@127.0.0.1:1/share/actors.idx",
        "smb://127.0.0.1:1/share/actors.cidx",
        "",
        64 << 20,
        &reg::FixedMemory(u64::MAX / 2),
        &reg::Fetcher { smb: cobolt_runtime::smb_source::fetcher() },
    )
    .unwrap_err();
    if cfg!(feature = "smb") {
        assert_eq!(r.code, "UNREACHABLE", "{}", r.message);
    } else {
        assert_eq!(r.code, "SMB-UNAVAILABLE");
    }
    assert!(!r.message.contains("hunter2"), "{}", r.message);
    report("real smb2, closed port", &[format!("{} in {:.0} ms", r.code, t.elapsed().as_secs_f64() * 1000.0)], Duration::ZERO);
}

/// The operator's live check (T12): a real share with credentials, a guest
/// share and an OS network path, each read and compared with a local copy.
/// Set `COBOLT_TEST_SMB_URL` / `COBOLT_TEST_SMB_GUEST_URL` to the data file's
/// `smb://` address (its `.cidx` beside it, same stem) and
/// `COBOLT_TEST_NET_PATH` to the same file through the OS's network path.
#[test]
#[ignore = "needs a real SMB share — run by the operator"]
fn live_smb_share() {
    let fetch = reg::Fetcher { smb: cobolt_runtime::smb_source::fetcher() };
    let mut ran = 0;
    for var in ["COBOLT_TEST_SMB_URL", "COBOLT_TEST_SMB_GUEST_URL", "COBOLT_TEST_NET_PATH"] {
        let Ok(data) = std::env::var(var) else { continue };
        let def = match data.rsplit_once('.') {
            Some((stem, _)) => format!("{stem}.cidx"),
            None => format!("{data}.cidx"),
        };
        let t = Instant::now();
        let r = reg::register(&data, &def, "", 64 << 20, &reg::SystemMemory, &fetch);
        match &r {
            Ok(r) => println!("  {var}: {:?}, {} bytes in {:.0} ms", r.fit, r.size, t.elapsed().as_secs_f64() * 1000.0),
            Err(e) => println!("  {var}: {} — {}", e.code, e.message),
        }
        assert!(r.is_ok(), "{var}");
        ran += 1;
    }
    assert!(ran > 0, "set at least one of the COBOLT_TEST_* variables");
}
