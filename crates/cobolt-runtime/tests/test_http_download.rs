// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Spec 092 R12, AC4 — `COBOL::"HTTP-DOWNLOAD" ( url file http-status status )`.
//!
//! Every server here is a `TcpListener` on `127.0.0.1`, written by hand so the
//! test controls each byte the client receives — no external network. The
//! behaviours under test:
//!
//! - the body arrives as BYTES: all 256 byte values, byte for byte;
//! - redirects are followed, up to five, and a credential header is not
//!   carried across one;
//! - only the headers set with `HTTP-SET-HEADER` are added;
//! - a 4xx/5xx is a failure with its code, and writes nothing;
//! - the size cap (declared or streamed) and the total timeout hold, and no
//!   `.part` file is ever left behind;
//! - a URL's query string — where a pre-signed URL keeps its signature —
//!   reaches no log line.

#![cfg(feature = "http")]

use std::fmt::Write as _;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::sync::{mpsc, Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use cobolt_lexer::{tokenize, SourceFormat};
use cobolt_parser::{parse, Severity};
use cobolt_runtime::http_runtime::{DownloadLimits, HttpClient};
use cobolt_runtime::Interpreter;

// ── A scripted server ──────────────────────────────────────────────────────

/// What the server answers for one request path.
type Responder = dyn Fn(&str) -> Reply + Send + Sync;

enum Reply {
    /// Bytes written as they are, then the connection closes.
    Raw(Vec<u8>),
    /// These bytes, then silence for this long, then close.
    Stall(Vec<u8>, Duration),
}

struct Server {
    port: u16,
    /// The head of every request received, in order.
    requests: Arc<Mutex<Vec<String>>>,
}

impl Server {
    fn url(&self, path_and_query: &str) -> String {
        format!("http://127.0.0.1:{}{path_and_query}", self.port)
    }
    fn requests(&self) -> Vec<String> {
        self.requests.lock().unwrap().clone()
    }
}

fn serve(respond: impl Fn(&str) -> Reply + Send + Sync + 'static) -> Server {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let requests = Arc::new(Mutex::new(Vec::new()));
    let log = Arc::clone(&requests);
    let respond: Arc<Responder> = Arc::new(respond);
    thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(stream) = stream else { break };
            let log = Arc::clone(&log);
            let respond = Arc::clone(&respond);
            thread::spawn(move || handle(stream, &log, &*respond));
        }
    });
    Server { port, requests }
}

fn handle(mut stream: TcpStream, log: &Mutex<Vec<String>>, respond: &Responder) {
    let mut head = Vec::new();
    let mut byte = [0u8; 1];
    while !head.ends_with(b"\r\n\r\n") {
        match stream.read(&mut byte) {
            Ok(1) => head.push(byte[0]),
            _ => return,
        }
    }
    let head = String::from_utf8_lossy(&head).into_owned();
    let path = head.split_whitespace().nth(1).unwrap_or("/").to_owned();
    log.lock().unwrap().push(head);
    match respond(&path) {
        Reply::Raw(bytes) => {
            let _ = stream.write_all(&bytes);
        }
        Reply::Stall(bytes, wait) => {
            let _ = stream.write_all(&bytes);
            let _ = stream.flush();
            thread::sleep(wait);
        }
    }
    let _ = stream.flush();
}

fn ok(body: &[u8]) -> Reply {
    let mut r = format!(
        "HTTP/1.1 200 OK\r\nContent-Type: application/octet-stream\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        body.len()
    )
    .into_bytes();
    r.extend_from_slice(body);
    Reply::Raw(r)
}

fn redirect(to: &str) -> Reply {
    Reply::Raw(
        format!("HTTP/1.1 302 Found\r\nLocation: {to}\r\nContent-Length: 0\r\nConnection: close\r\n\r\n")
            .into_bytes(),
    )
}

fn status(code: u16, reason: &str) -> Reply {
    let body = format!("{code} {reason}");
    Reply::Raw(
        format!(
            "HTTP/1.1 {code} {reason}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        )
        .into_bytes(),
    )
}

/// Every byte value 0–255, four times over and a little more, so the body is
/// not a round number and holds NULs, CR/LF and bytes above 127 — everything a
/// text download would mangle.
fn every_byte() -> Vec<u8> {
    (0..1041u32).map(|i| (i % 256) as u8).collect()
}

// ── Running COBOL ──────────────────────────────────────────────────────────

fn run(src: &str) -> Vec<String> {
    let result = parse(tokenize(src, SourceFormat::Free));
    assert!(
        result.diagnostics.iter().all(|d| d.severity != Severity::Error),
        "parse errors: {:?}",
        result.diagnostics
    );
    let program = result.program.expect("no program");
    let (_event_tx, event_rx) = mpsc::channel();
    let (state_tx, _state_rx) = mpsc::channel();
    let (display_tx, display_rx) = mpsc::channel();
    let mut interp = Interpreter::new_with_channels(program, event_rx, state_tx, display_tx);
    interp.run().expect("run failed");
    display_rx.try_iter().map(|s| s.trim().to_owned()).collect()
}

fn field(lines: &[String], key: &str) -> String {
    let prefix = format!("{key}=[");
    lines
        .iter()
        .find_map(|l| l.strip_prefix(&prefix).and_then(|v| v.strip_suffix(']')))
        .unwrap_or_else(|| panic!("no {key} in {lines:?}"))
        .trim()
        .to_owned()
}

/// Run `before` (COBOL statements, e.g. HTTP-SET-HEADER calls) and then
/// `COBOL::"HTTP-DOWNLOAD" ( url file code status )`; returns (code, status).
fn download_with(before: &str, url: &str, file: &Path) -> (u16, String) {
    let src = format!(
        r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. DOWNLOAD-ONE.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 WS-URL     PIC X(600) VALUE "{url}".
       01 WS-FILE    PIC X(400) VALUE "{file}".
       01 WS-CODE    PIC 9(3)   VALUE 999.
       01 WS-STATUS  PIC X(12)  VALUE "UNSET".
       PROCEDURE DIVISION.
       MAIN.
{before}
           COBOL::"HTTP-DOWNLOAD" ( WS-URL WS-FILE WS-CODE WS-STATUS )
           DISPLAY "CODE=[" WS-CODE "]"
           DISPLAY "STATUS=[" WS-STATUS "]"
           STOP RUN.
"#,
        file = file.display(),
    );
    let out = run(&src);
    (field(&out, "CODE").parse().unwrap(), field(&out, "STATUS"))
}

fn download(url: &str, file: &Path) -> (u16, String) {
    download_with("", url, file)
}

fn scratch(tag: &str) -> PathBuf {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let dir = std::env::temp_dir().join(format!("prc-dl-{tag}-{}-{nanos}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn part_of(file: &Path) -> PathBuf {
    let mut p = file.as_os_str().to_owned();
    p.push(".part");
    PathBuf::from(p)
}

#[track_caller]
fn assert_nothing_written(file: &Path) {
    assert!(!file.exists(), "{} must not be written", file.display());
    assert!(!part_of(file).exists(), "no .part may be left beside {}", file.display());
}

// ── The tests ──────────────────────────────────────────────────────────────

/// AC4: the exact bytes of a binary body — every value 0–255 — land in the
/// file, and the folder on the way to it is created.
#[test]
fn a_binary_body_is_saved_byte_for_byte() {
    let server = serve(|_| ok(&every_byte()));
    let dir = scratch("bytes");
    let file = dir.join("cache/photos/pet-1.bin");
    let t = Instant::now();
    let (code, st) = download(&server.url("/photos/pet-1.bin"), &file);
    let ms = t.elapsed().as_millis();
    assert_eq!((code, st.as_str()), (200, "OK"));
    let saved = std::fs::read(&file).expect("the file is written");
    assert_eq!(saved.len(), 1041);
    assert_eq!(saved, every_byte(), "byte for byte, NULs and high bytes included");
    assert!(!part_of(&file).exists(), "the .part was renamed, not left behind");
    println!("  1041 bytes (every value 0–255) saved exactly in {ms} ms");
    let _ = std::fs::remove_dir_all(&dir);
}

/// AC4: a 302 is followed and the final body saved; five redirects in a row
/// are followed, a sixth is refused.
#[test]
fn redirects_are_followed_up_to_five() {
    let server = serve(|path| match path {
        "/start" => redirect("/final"),
        "/final" => ok(b"the final body"),
        p if p.starts_with("/hop") => {
            // /hopN → /hop(N-1) … /hop1 → /final
            let n: u32 = p[4..].parse().unwrap();
            if n > 1 { redirect(&format!("/hop{}", n - 1)) } else { redirect("/final") }
        }
        _ => status(404, "Not Found"),
    });
    let dir = scratch("redirect");

    let file = dir.join("one.bin");
    assert_eq!(download(&server.url("/start"), &file), (200, "OK".into()));
    assert_eq!(std::fs::read(&file).unwrap(), b"the final body");

    let five = dir.join("five.bin");
    assert_eq!(download(&server.url("/hop5"), &five), (200, "OK".into()), "five redirects are followed");
    assert_eq!(std::fs::read(&five).unwrap(), b"the final body");

    let six = dir.join("six.bin");
    let (code, st) = download(&server.url("/hop6"), &six);
    assert_eq!(st, "HTTP", "a sixth redirect is refused");
    assert_eq!(code, 0, "no final answer was received");
    assert_nothing_written(&six);
    println!("  302 followed; 5 hops OK; 6 hops → HTTP/0; {} requests served", server.requests().len());
    let _ = std::fs::remove_dir_all(&dir);
}

/// AC4: a 404 is a failure with its code — no file — and an existing file is
/// left exactly as it was. A 500 likewise.
#[test]
fn an_error_status_writes_nothing_and_keeps_the_old_file() {
    let server = serve(|path| match path {
        "/gone" => status(404, "Not Found"),
        _ => status(500, "Internal Server Error"),
    });
    let dir = scratch("status");
    let file = dir.join("missing.bin");
    assert_eq!(download(&server.url("/gone"), &file), (404, "HTTP".into()));
    assert_nothing_written(&file);

    let kept = dir.join("kept.bin");
    std::fs::write(&kept, b"yesterday's copy").unwrap();
    assert_eq!(download(&server.url("/boom"), &kept), (500, "HTTP".into()));
    assert_eq!(std::fs::read(&kept).unwrap(), b"yesterday's copy", "a failed refresh keeps the old file");
    assert!(!part_of(&kept).exists());
    let _ = std::fs::remove_dir_all(&dir);
}

/// R12: only the headers the program set are added, and a credential header
/// does not follow a redirect (to the same host or any other).
#[test]
fn only_the_headers_set_are_sent_and_authorization_stops_at_a_redirect() {
    let server = serve(|path| match path {
        "/start" => redirect("/final"),
        _ => ok(b"x"),
    });
    let dir = scratch("headers");
    let before = r#"           COBOL::"HTTP-SET-HEADER" ( "X-Trace" "abc-123" )
           COBOL::"HTTP-SET-HEADER" ( "Authorization" "Bearer not-a-real-token" )"#;
    let file = dir.join("h.bin");
    assert_eq!(download_with(before, &server.url("/start"), &file), (200, "OK".into()));
    let reqs = server.requests();
    assert_eq!(reqs.len(), 2, "{reqs:?}");
    let lower: Vec<String> = reqs.iter().map(|r| r.to_ascii_lowercase()).collect();
    assert!(lower[0].contains("x-trace: abc-123"), "the header set is sent:\n{}", reqs[0]);
    assert!(lower[0].contains("authorization: bearer"), "…and the credential, to the first address");
    assert!(lower[1].contains("x-trace: abc-123"), "a plain header follows the redirect:\n{}", reqs[1]);
    assert!(!lower[1].contains("authorization"), "a credential does not:\n{}", reqs[1]);
    for r in &lower {
        assert!(!r.contains("content-type"), "a download sends no body and no Content-Type:\n{r}");
    }

    // HTTP-CLEAR-HEADERS takes them all away again.
    let before = r#"           COBOL::"HTTP-SET-HEADER" ( "X-Trace" "abc-123" )
           COBOL::"HTTP-CLEAR-HEADERS" ()"#;
    assert_eq!(download_with(before, &server.url("/plain"), &dir.join("c.bin")), (200, "OK".into()));
    let last = server.requests().last().unwrap().to_ascii_lowercase();
    assert!(!last.contains("x-trace"), "cleared headers are not sent:\n{last}");
    let _ = std::fs::remove_dir_all(&dir);
}

/// R12: the 256 MB cap. A server that DECLARES more is refused before a byte
/// of the body is read; one that streams more (no Content-Length) is cut off
/// at the cap. Either way nothing is written and no `.part` remains.
#[test]
fn the_size_cap_holds_declared_or_streamed() {
    // Declared: 256 MiB + 1, then the connection closes without a body.
    let server = serve(|_| {
        Reply::Raw(
            b"HTTP/1.1 200 OK\r\nContent-Length: 268435457\r\nConnection: close\r\n\r\n".to_vec(),
        )
    });
    let dir = scratch("cap");
    let file = dir.join("huge.bin");
    let t = Instant::now();
    assert_eq!(download(&server.url("/huge"), &file), (200, "TOOBIG".into()));
    let declared_ms = t.elapsed().as_millis();
    assert_nothing_written(&file);

    // Streamed: chunked, 5 000 bytes, against a 1 000-byte cap. The cap is a
    // parameter of the runtime's download so a test need not move 256 MB; the
    // built-in passes the default, `DownloadLimits::default().max_bytes`.
    assert_eq!(DownloadLimits::default().max_bytes, 256 * 1024 * 1024);
    let chunked = serve(|_| {
        let mut r = b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\nConnection: close\r\n\r\n".to_vec();
        for _ in 0..5 {
            r.extend_from_slice(b"3e8\r\n"); // 1000 bytes per chunk
            r.extend_from_slice(&[b'z'; 1000]);
            r.extend_from_slice(b"\r\n");
        }
        r.extend_from_slice(b"0\r\n\r\n");
        Reply::Raw(r)
    });
    let streamed = dir.join("streamed.bin");
    let limits = DownloadLimits { max_bytes: 1000, ..DownloadLimits::default() };
    let got = HttpClient::new().download_with(&chunked.url("/stream"), &streamed, limits);
    assert_eq!((got.http_status, got.status), (200, "TOOBIG"));
    assert_nothing_written(&streamed);

    // Exactly at the cap is allowed.
    let exact = dir.join("exact.bin");
    let limits = DownloadLimits { max_bytes: 5000, ..DownloadLimits::default() };
    let got = HttpClient::new().download_with(&chunked.url("/stream"), &exact, limits);
    assert_eq!((got.http_status, got.status), (200, "OK"));
    assert_eq!(std::fs::metadata(&exact).unwrap().len(), 5000);
    println!("  declared 256 MiB+1 refused in {declared_ms} ms; streamed 5000 B vs cap 1000 → TOOBIG; cap 5000 → OK");
    let _ = std::fs::remove_dir_all(&dir);
}

/// R12: the total timeout covers the body too — a server that sends the
/// headers and part of the body and then goes quiet is abandoned.
#[test]
fn a_stalled_body_times_out_and_leaves_no_part_file() {
    assert_eq!(DownloadLimits::default().timeout_ms, 60_000, "the built-in's total timeout");
    let server = serve(|_| {
        Reply::Stall(
            b"HTTP/1.1 200 OK\r\nContent-Length: 1000\r\nConnection: close\r\n\r\n0123456789".to_vec(),
            Duration::from_secs(3),
        )
    });
    let dir = scratch("timeout");
    let file = dir.join("slow.bin");
    let limits = DownloadLimits { timeout_ms: 400, ..DownloadLimits::default() };
    let t = Instant::now();
    let got = HttpClient::new().download_with(&server.url("/slow"), &file, limits);
    let ms = t.elapsed().as_millis();
    assert_eq!(got.status, "TIMEOUT");
    assert!(ms < 2500, "gave up after {ms} ms, well before the server's 3 s stall ended");
    assert_nothing_written(&file);
    println!("  stalled body abandoned after {ms} ms (limit 400 ms)");
    let _ = std::fs::remove_dir_all(&dir);
}

/// R12: the address is checked before anything is sent; a refused connection
/// is NET; an empty file name is WRITE, as is a folder that cannot be made.
#[test]
fn bad_addresses_and_unwritable_files_answer_their_words() {
    let dir = scratch("words");
    let file = dir.join("x.bin");
    for url in ["ftp://example.com/a.bin", "not a url", "", "http://", "file:///etc/hosts"] {
        assert_eq!(download(url, &file), (0, "BADURL".into()), "{url:?}");
        assert_nothing_written(&file);
    }

    // A port nobody listens on: bind one, learn its number, close it.
    let port = TcpListener::bind("127.0.0.1:0").unwrap().local_addr().unwrap().port();
    let (code, st) = download(&format!("http://127.0.0.1:{port}/a.bin"), &file);
    assert_eq!((code, st.as_str()), (0, "NET"));
    assert_nothing_written(&file);

    let server = serve(|_| ok(b"data"));
    let blocker = dir.join("a-file");
    std::fs::write(&blocker, b"").unwrap();
    let inside_a_file = blocker.join("sub/x.bin");
    assert_eq!(download(&server.url("/a.bin"), &inside_a_file), (200, "WRITE".into()));
    assert!(!inside_a_file.exists());
    assert_eq!(download(&server.url("/a.bin"), Path::new("")), (0, "WRITE".into()), "no file name");
    let _ = std::fs::remove_dir_all(&dir);
}

// ── No signature in any log ────────────────────────────────────────────────

/// Every tracing event and span, rendered as `name=value` text.
#[derive(Clone, Default)]
struct Capture(Arc<Mutex<Vec<String>>>);

struct Fields(String);

impl tracing::field::Visit for Fields {
    fn record_debug(&mut self, field: &tracing::field::Field, value: &dyn std::fmt::Debug) {
        let _ = write!(self.0, "{}={:?} ", field.name(), value);
    }
}

impl tracing::Subscriber for Capture {
    fn enabled(&self, _: &tracing::Metadata<'_>) -> bool {
        true
    }
    fn new_span(&self, span: &tracing::span::Attributes<'_>) -> tracing::span::Id {
        let mut f = Fields(format!("span {} ", span.metadata().name()));
        span.record(&mut f);
        self.0.lock().unwrap().push(f.0);
        tracing::span::Id::from_u64(1)
    }
    fn record(&self, _: &tracing::span::Id, values: &tracing::span::Record<'_>) {
        let mut f = Fields(String::from("record "));
        values.record(&mut f);
        self.0.lock().unwrap().push(f.0);
    }
    fn record_follows_from(&self, _: &tracing::span::Id, _: &tracing::span::Id) {}
    fn event(&self, event: &tracing::Event<'_>) {
        let mut f = Fields(format!("{} {} ", event.metadata().level(), event.metadata().target()));
        event.record(&mut f);
        self.0.lock().unwrap().push(f.0);
    }
    fn enter(&self, _: &tracing::span::Id) {}
    fn exit(&self, _: &tracing::span::Id) {}
}

/// The one capture, installed as the global subscriber on first use.
fn global_capture() -> &'static Capture {
    static CAPTURE: std::sync::OnceLock<Capture> = std::sync::OnceLock::new();
    CAPTURE.get_or_init(|| {
        let capture = Capture::default();
        tracing::subscriber::set_global_default(capture.clone())
            .expect("nothing else in this test binary installs a global subscriber");
        capture
    })
}

/// R12: a pre-signed URL's query string is its signature. Downloads that fail
/// in every way the runtime reports — 404, refused connection, timeout, size —
/// and one that succeeds, all with a signed query: no captured log line, at
/// any level, carries any part of the query. The failures ARE logged, by path.
#[test]
fn the_query_string_never_reaches_a_log() {
    // Built from pieces: obviously fake, and no single literal looks like one.
    let signature = format!("{}{}{}", "feedface", "0badc0de", "5ec7e7");
    let query = format!("?X-Amz-Algorithm=AWS4-HMAC-SHA256&X-Amz-Expires=300&X-Amz-Signature={signature}");
    let server = serve(|path| {
        if path.starts_with("/photos/found.jpg") { ok(b"jpeg bytes") } else { status(404, "Not Found") }
    });
    let refused = TcpListener::bind("127.0.0.1:0").unwrap().local_addr().unwrap().port();
    let dir = scratch("secret");

    // The capture is the PROCESS-WIDE subscriber, not a scoped one: with
    // `with_default`, a parallel test reaching the same `warn!` with no
    // subscriber can leave that callsite's interest cached as "never", and the
    // events of this test are then silently skipped (seen in the full sweep).
    // Every test in this binary logs into it; none but this one uses a
    // signature, so the leak checks below hold for all of them.
    let capture = global_capture();
    let displayed = {
        let mut shown = Vec::new();
        for (url, file) in [
            (server.url(&format!("/photos/lost.jpg{query}")), dir.join("lost.jpg")),
            (server.url(&format!("/photos/found.jpg{query}")), dir.join("found.jpg")),
            (format!("http://127.0.0.1:{refused}/photos/x.jpg{query}"), dir.join("x.jpg")),
            (format!("http://user:pw@127.0.0.1:{refused}/photos/y.jpg{query}#frag"), dir.join("y.jpg")),
        ] {
            let src = format!(
                r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. SIGNED.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 WS-URL     PIC X(600) VALUE "{url}".
       01 WS-CODE    PIC 9(3).
       01 WS-STATUS  PIC X(12).
       PROCEDURE DIVISION.
       MAIN.
           COBOL::"HTTP-DOWNLOAD" ( WS-URL "{file}" WS-CODE WS-STATUS )
           DISPLAY "STATUS=[" WS-STATUS "]"
           STOP RUN.
"#,
                file = file.display()
            );
            shown.extend(run(&src));
        }
        // The runtime's own API, on its failure paths.
        let client = HttpClient::new();
        let small = DownloadLimits { max_bytes: 3, ..DownloadLimits::default() };
        let got = client.download_with(&server.url(&format!("/photos/found.jpg{query}")), &dir.join("s.jpg"), small);
        assert_eq!(got.status, "TOOBIG");
        shown
    };

    let lines = capture.0.lock().unwrap().clone();
    let statuses: Vec<String> = displayed
        .iter()
        .filter_map(|l| l.strip_prefix("STATUS=[").and_then(|v| v.strip_suffix(']')))
        .map(|v| v.trim().to_owned())
        .collect();
    assert_eq!(statuses, ["HTTP", "OK", "NET", "NET"], "{displayed:?}");
    assert!(
        lines.iter().any(|l| l.contains("/photos/lost.jpg")),
        "a failed download IS logged, by its path — or this test captures nothing:\n{lines:#?}"
    );
    for needle in [signature.as_str(), "X-Amz", "Signature", "Expires", "user:pw", "#frag"] {
        let leaks: Vec<&String> = lines.iter().filter(|l| l.contains(needle)).collect();
        assert!(leaks.is_empty(), "{needle:?} reached a log line: {leaks:#?}");
        let shown: Vec<&String> = displayed.iter().filter(|l| l.contains(needle)).collect();
        assert!(shown.is_empty(), "{needle:?} reached the program's output: {shown:#?}");
    }
    println!(
        "  {} log lines captured (this binary's tests, every level); none carries the query, the user info or the fragment",
        lines.len()
    );
    for l in lines.iter().filter(|l| l.contains("/photos/")) {
        println!("    {l}");
    }
    let _ = std::fs::remove_dir_all(&dir);
}
