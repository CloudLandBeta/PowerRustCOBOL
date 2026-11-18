// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! DESKTOP-WASM-001 T42 (R10, R11, ABI A45): the HTTP built-ins (`COBOL::"HTTP-GET"`, `"HTTP-POST"`,
//! `"HTTP-PUT"`, `"HTTP-DELETE"`, `"HTTP-SET-HEADER"`, `"HTTP-CLEAR-HEADERS"`, `"HTTP-DOWNLOAD"`, and
//! their `CALL "COBOL-…"` form) behave on the WebAssembly path as they do under the interpreter. Each
//! program runs twice against a stub server on this machine (no reference script calls the network):
//! packaged into a copy of the built `prc-runner`, and under `rcrun run`. The two runs must print the
//! same lines, send the server the same requests, and leave the same files. The interpreter's own
//! download test's cases (`cobolt-runtime/tests/test_http_download.rs`) must also answer what it asserts.

use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::sync::{Arc, Mutex};
use std::time::Instant;

use cobolt_program_package::Kind;

/// Every verb and every kind of receiver: a body into a short item, a group, a `JUSTIFIED RIGHT` item
/// (a body with spaces after it, and one longer than the item), a 40 000-byte body into a larger item
/// (longer than the module's scratch area), a number into `9(4)`, `S9(3)V99` and `ZZ9.99`, a text that
/// is no number, a UTF-8 body cut in the middle of a character; the status code into `9(3)`, `X(5)`,
/// `9(2)` and `ZZZ9`; a URL built with `STRING` and one with spaces around it; a redirect; a POST's body
/// with spaces after it and the default `Content-Type`; headers set (with spaces around the value), seen
/// by a program it calls (`CALL "COBOL-HTTP-GET"`), and cleared; downloads; a refused connection and a
/// URL of spaces.
const EDGES: &str = r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. HTTP-EDGES.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 WS-URL      PIC X(80)  VALUE SPACES.
       01 WS-BODY     PIC X(100) VALUE SPACES.
       01 WS-RESP     PIC X(70)  VALUE SPACES.
       01 WS-ST       PIC 9(3)   VALUE 0.
       01 WS-STX      PIC X(5)   VALUE SPACES.
       01 WS-ST2      PIC 9(2)   VALUE 0.
       01 WS-STE      PIC ZZZ9   VALUE ZERO.
       01 WS-BIG      PIC X(40010) VALUE SPACES.
       01 WS-RJ       PIC X(12)  JUSTIFIED RIGHT VALUE SPACES.
       01 WS-RJ8      PIC X(8)   JUSTIFIED RIGHT VALUE SPACES.
       01 WS-NUM      PIC 9(4)   VALUE 0.
       01 WS-AMT      PIC S9(3)V99 VALUE 0.
       01 WS-ED       PIC ZZ9.99 VALUE ZERO.
       01 WS-SHORT    PIC X(4)   VALUE SPACES.
       01 WS-GRP.
          05 WS-G1    PIC X(4).
          05 WS-G2    PIC X(4).
       01 WS-NONE     PIC X(10)  VALUE SPACES.
       01 WS-WORD     PIC X(10)  VALUE SPACES.
       PROCEDURE DIVISION.
       MAIN.
           COBOL::"HTTP-GET" ( "@BASE@/hello" WS-RESP WS-ST )
           DISPLAY "GET " WS-ST " [" FUNCTION TRIM(WS-RESP) "]"
           STRING "@BASE@" "/missing" DELIMITED BY SIZE INTO WS-URL
           COBOL::"HTTP-GET" ( WS-URL WS-RESP WS-STX )
           DISPLAY "MISSING [" WS-STX "] [" FUNCTION TRIM(WS-RESP) "]"
           COBOL::"HTTP-GET" ( "@BASE@/teapot" WS-RESP WS-ST2 )
           DISPLAY "TEAPOT " WS-ST2 " [" FUNCTION TRIM(WS-RESP) "]"
           COBOL::"HTTP-GET" ( "  @BASE@/redirect  " WS-RESP WS-STE )
           DISPLAY "REDIRECT [" WS-STE "] [" FUNCTION TRIM(WS-RESP) "]"
           COBOL::"HTTP-GET" ( "@BASE@/big" WS-BIG WS-ST )
           DISPLAY "BIG " WS-ST " [" WS-BIG(1:12) "] [" WS-BIG(39995:16) "]"
           COBOL::"HTTP-GET" ( "@BASE@/big" WS-RJ8 WS-ST )
           DISPLAY "BIG RJ [" WS-RJ8 "]"
           COBOL::"HTTP-GET" ( "@BASE@/short" WS-RJ WS-ST )
           DISPLAY "SHORT RJ [" WS-RJ "]"
           COBOL::"HTTP-GET" ( "@BASE@/short" WS-RESP WS-ST )
           DISPLAY "SHORT [" WS-RESP(1:8) "]"
           COBOL::"HTTP-GET" ( "@BASE@/hello" WS-GRP WS-ST )
           DISPLAY "GROUP [" WS-G1 "|" WS-G2 "]"
           COBOL::"HTTP-GET" ( "@BASE@/number" WS-NUM WS-ST )
           COBOL::"HTTP-GET" ( "@BASE@/number" WS-AMT WS-ST )
           COBOL::"HTTP-GET" ( "@BASE@/number" WS-ED WS-ST )
           DISPLAY "NUMBER " WS-NUM " " WS-AMT " [" WS-ED "]"
           COBOL::"HTTP-GET" ( "@BASE@/hello" WS-NUM WS-ST )
           DISPLAY "NOT A NUMBER " WS-NUM
           COBOL::"HTTP-GET" ( "@BASE@/utf8" WS-SHORT WS-ST )
           DISPLAY "UTF8 [" WS-SHORT "]"
           MOVE '{"name": "ana"}' TO WS-BODY
           COBOL::"HTTP-POST" ( "@BASE@/echo" WS-BODY WS-RESP WS-ST )
           DISPLAY "POST " WS-ST " " FUNCTION TRIM(WS-RESP)
           COBOL::"HTTP-SET-HEADER" ( "X-Test" "  abc  " )
           COBOL::"HTTP-SET-HEADER" ( "Content-Type" "text/plain" )
           COBOL::"HTTP-PUT" ( "@BASE@/echo" "put body" WS-RESP WS-ST )
           DISPLAY "PUT " WS-ST " " FUNCTION TRIM(WS-RESP)
           CALL "HEADERS" USING WS-RESP WS-ST
           DISPLAY "CALLED " WS-ST " " FUNCTION TRIM(WS-RESP)
           COBOL::"HTTP-DELETE" ( "@BASE@/echo" WS-RESP WS-ST )
           DISPLAY "DELETE " WS-ST " " FUNCTION TRIM(WS-RESP)
           COBOL::"HTTP-CLEAR-HEADERS" ( )
           COBOL::"HTTP-GET" ( "@BASE@/echo" WS-RESP WS-ST )
           DISPLAY "CLEARED " WS-ST " " FUNCTION TRIM(WS-RESP)
           COBOL::"HTTP-DOWNLOAD" ( "@BASE@/hello" "@DIR@/hello.txt" WS-STX WS-WORD )
           DISPLAY "DOWNLOAD [" WS-STX "] " WS-WORD
           COBOL::"HTTP-DOWNLOAD" ( "@BASE@/hello" WS-NONE WS-ST WS-WORD )
           DISPLAY "DOWNLOAD NO FILE " WS-ST " " WS-WORD
           COBOL::"HTTP-GET" ( "http://127.0.0.1:@CLOSED@/" WS-RESP WS-ST )
           DISPLAY "REFUSED " WS-ST " " WS-RESP(1:15)
           COBOL::"HTTP-GET" ( WS-NONE WS-RESP WS-ST )
           DISPLAY "NO URL " WS-ST " " WS-RESP(1:15)
           STOP RUN.
       IDENTIFICATION DIVISION.
       PROGRAM-ID. HEADERS.
       DATA DIVISION.
       LINKAGE SECTION.
       01 L-RESP      PIC X(70).
       01 L-ST        PIC 9(3).
       PROCEDURE DIVISION USING L-RESP L-ST.
           CALL "COBOL-HTTP-GET" USING "@BASE@/echo" L-RESP L-ST
           GOBACK.
       END PROGRAM HEADERS.
       END PROGRAM HTTP-EDGES.
"#;

/// The interpreter's download test's cases (`test_http_download.rs`), one program: a binary body into
/// a folder made on the way, a redirect, five redirects and a sixth refused, a 404 and a 500 that keep
/// an old file, the headers set (a credential not sent past a redirect) and cleared, addresses that
/// are no URL, a port nobody listens on, a file inside a file and no file name.
const DOWNLOADS: &str = r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. HTTP-DOWNLOADS.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 WS-CODE    PIC 9(3)   VALUE 999.
       01 WS-STATUS  PIC X(12)  VALUE "UNSET".
       01 WS-URL     PIC X(600) VALUE SPACES.
       01 WS-NONE    PIC X(400) VALUE SPACES.
       PROCEDURE DIVISION.
       MAIN.
           COBOL::"HTTP-DOWNLOAD" ( "@BASE@/bytes" "@DIR@/cache/photos/pet-1.bin" WS-CODE WS-STATUS )
           DISPLAY "BYTES " WS-CODE " " WS-STATUS
           COBOL::"HTTP-DOWNLOAD" ( "@BASE@/start" "@DIR@/one.bin" WS-CODE WS-STATUS )
           DISPLAY "START " WS-CODE " " WS-STATUS
           COBOL::"HTTP-DOWNLOAD" ( "@BASE@/hop5" "@DIR@/five.bin" WS-CODE WS-STATUS )
           DISPLAY "HOP5 " WS-CODE " " WS-STATUS
           COBOL::"HTTP-DOWNLOAD" ( "@BASE@/hop6" "@DIR@/six.bin" WS-CODE WS-STATUS )
           DISPLAY "HOP6 " WS-CODE " " WS-STATUS
           COBOL::"HTTP-DOWNLOAD" ( "@BASE@/gone" "@DIR@/missing.bin" WS-CODE WS-STATUS )
           DISPLAY "GONE " WS-CODE " " WS-STATUS
           COBOL::"HTTP-DOWNLOAD" ( "@BASE@/boom" "@DIR@/kept.bin" WS-CODE WS-STATUS )
           DISPLAY "BOOM " WS-CODE " " WS-STATUS
           COBOL::"HTTP-SET-HEADER" ( "X-Trace" "abc-123" )
           COBOL::"HTTP-SET-HEADER" ( "Authorization" "Bearer not-a-real-token" )
           COBOL::"HTTP-DOWNLOAD" ( "@BASE@/start" "@DIR@/h.bin" WS-CODE WS-STATUS )
           DISPLAY "HEADERS " WS-CODE " " WS-STATUS
           COBOL::"HTTP-CLEAR-HEADERS" ( )
           COBOL::"HTTP-DOWNLOAD" ( "@BASE@/plain" "@DIR@/c.bin" WS-CODE WS-STATUS )
           DISPLAY "CLEARED " WS-CODE " " WS-STATUS
           MOVE "ftp://example.com/a.bin" TO WS-URL
           PERFORM BAD-URL
           MOVE "not a url" TO WS-URL
           PERFORM BAD-URL
           MOVE SPACES TO WS-URL
           PERFORM BAD-URL
           MOVE "http://" TO WS-URL
           PERFORM BAD-URL
           MOVE "file:///etc/hosts" TO WS-URL
           PERFORM BAD-URL
           COBOL::"HTTP-DOWNLOAD" ( "http://127.0.0.1:@CLOSED@/a.bin" "@DIR@/x.bin" WS-CODE WS-STATUS )
           DISPLAY "CLOSED PORT " WS-CODE " " WS-STATUS
           COBOL::"HTTP-DOWNLOAD" ( "@BASE@/a.bin" "@DIR@/a-file/sub/x.bin" WS-CODE WS-STATUS )
           DISPLAY "INSIDE A FILE " WS-CODE " " WS-STATUS
           COBOL::"HTTP-DOWNLOAD" ( "@BASE@/a.bin" WS-NONE WS-CODE WS-STATUS )
           DISPLAY "NO NAME " WS-CODE " " WS-STATUS
           STOP RUN.
       BAD-URL.
           COBOL::"HTTP-DOWNLOAD" ( WS-URL "@DIR@/x.bin" WS-CODE WS-STATUS )
           DISPLAY "BAD URL " WS-CODE " " WS-STATUS.
"#;

/// What the download program prints: the codes and words the interpreter's test asserts.
const DOWNLOAD_LINES: [&str; 16] = [
    "BYTES 200 OK",
    "START 200 OK",
    "HOP5 200 OK",
    "HOP6 000 HTTP",
    "GONE 404 HTTP",
    "BOOM 500 HTTP",
    "HEADERS 200 OK",
    "CLEARED 200 OK",
    "BAD URL 000 BADURL",
    "BAD URL 000 BADURL",
    "BAD URL 000 BADURL",
    "BAD URL 000 BADURL",
    "BAD URL 000 BADURL",
    "CLOSED PORT 000 NET",
    "INSIDE A FILE 200 WRITE",
    "NO NAME 000 WRITE",
];

/// Every byte value 0–255, four times over and a little more (the interpreter's test's body).
fn every_byte() -> Vec<u8> {
    (0..1041u32).map(|i| (i % 256) as u8).collect()
}

// ── The stub server ─────────────────────────────────────────────────────────

struct Server {
    port: u16,
    requests: Arc<Mutex<Vec<String>>>,
}

impl Server {
    /// The requests served since the last call, each its head with the port taken out, in order.
    fn take(&self) -> Vec<String> {
        let port = format!(":{}", self.port);
        std::mem::take(&mut *self.requests.lock().unwrap()).into_iter().map(|r| r.replace(&port, ":PORT")).collect()
    }
}

fn reply(code: u16, reason: &str, extra: &str, body: &[u8]) -> Vec<u8> {
    let mut r = format!("HTTP/1.1 {code} {reason}\r\n{extra}Content-Length: {}\r\nConnection: close\r\n\r\n", body.len()).into_bytes();
    r.extend_from_slice(body);
    r
}

fn respond(method: &str, path: &str, head: &str, body: &[u8]) -> Vec<u8> {
    let header = |name: &str| {
        head.lines()
            .filter_map(|l| l.split_once(':'))
            .find(|(k, _)| k.trim().eq_ignore_ascii_case(name))
            .map(|(_, v)| v.trim().to_owned())
            .unwrap_or_else(|| "-".into())
    };
    let ok = |b: &[u8]| reply(200, "OK", "", b);
    let redirect = |to: &str| reply(302, "Found", &format!("Location: {to}\r\n"), b"");
    match path {
        "/hello" | "/plain" => ok(b"hello, world"),
        "/missing" | "/gone" => reply(404, "Not Found", "", b"404 Not Found"),
        "/boom" => reply(500, "Internal Server Error", "", b"500 Internal Server Error"),
        "/teapot" => reply(418, "I'm a teapot", "", b"short and stout"),
        "/redirect" | "/start" => redirect(if path == "/start" { "/final" } else { "/hello" }),
        "/final" => ok(b"the final body"),
        p if p.starts_with("/hop") => {
            let n: u32 = p[4..].parse().unwrap_or(1);
            if n > 1 { redirect(&format!("/hop{}", n - 1)) } else { redirect("/final") }
        }
        "/big" => ok("0123456789".repeat(4000).as_bytes()),
        "/short" => ok(b"abc   "),
        "/number" => ok(b"  42 "),
        "/utf8" => ok("café ñ".as_bytes()),
        "/bytes" => ok(&every_byte()),
        "/a.bin" => ok(b"data"),
        "/echo" => {
            let said = format!("{method} x={} ct={} [{}]", header("x-test"), header("content-type"), String::from_utf8_lossy(body));
            ok(said.as_bytes())
        }
        _ => reply(404, "Not Found", "", b"404 Not Found"),
    }
}

fn handle(mut stream: TcpStream, log: &Mutex<Vec<String>>) {
    let mut head = Vec::new();
    let mut byte = [0u8; 1];
    while !head.ends_with(b"\r\n\r\n") {
        match stream.read(&mut byte) {
            Ok(1) => head.push(byte[0]),
            _ => return,
        }
    }
    let head = String::from_utf8_lossy(&head).into_owned();
    let mut words = head.split_whitespace();
    let (method, path) = (words.next().unwrap_or("").to_owned(), words.next().unwrap_or("/").to_owned());
    let len = head
        .lines()
        .filter_map(|l| l.split_once(':'))
        .find(|(k, _)| k.trim().eq_ignore_ascii_case("content-length"))
        .and_then(|(_, v)| v.trim().parse::<usize>().ok())
        .unwrap_or(0);
    let mut body = vec![0u8; len];
    if stream.read_exact(&mut body).is_err() {
        return;
    }
    log.lock().unwrap().push(format!("{head}{}", String::from_utf8_lossy(&body)));
    let _ = stream.write_all(&respond(&method, &path, &head, &body));
    let _ = stream.flush();
}

fn serve() -> Server {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let requests = Arc::new(Mutex::new(Vec::new()));
    let log = Arc::clone(&requests);
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(stream) = stream else { break };
            let log = Arc::clone(&log);
            std::thread::spawn(move || handle(stream, &log));
        }
    });
    Server { port, requests }
}

// ── Running the two paths ───────────────────────────────────────────────────

struct Scratch(PathBuf);

impl Scratch {
    fn new(tag: &str) -> Self {
        let p = std::env::temp_dir().join(format!("prc-runner-t42-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&p);
        std::fs::create_dir_all(&p).unwrap();
        Scratch(p)
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        if std::env::var_os("PRC_RUNNER_KEEP").is_none() {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
}

fn parse(src: &str) -> cobolt_ast::program::Program {
    let parsed = cobolt_parser::parse(cobolt_lexer::tokenize(src, cobolt_lexer::SourceFormat::Free));
    let errors: Vec<String> = parsed.diagnostics.iter().filter(|d| d.is_error()).map(|d| format!("line {}: {}", d.span.line, d.message)).collect();
    assert!(errors.is_empty(), "the program does not parse: {errors:?}");
    parsed.program.expect("a program")
}

/// `rcrun`, built now from the current source (a stale binary would compare against old code).
fn rcrun() -> PathBuf {
    let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".into());
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let status = Command::new(cargo).current_dir(&root).args(["build", "-q", "-p", "cobolt-cli", "--bin", "rcrun"]).status().expect("run cargo");
    assert!(status.success(), "cargo build -p cobolt-cli --bin rcrun failed");
    let exe = std::env::current_exe().unwrap();
    let dir = exe.parent().and_then(Path::parent).unwrap();
    dir.join(format!("rcrun{}", std::env::consts::EXE_SUFFIX))
}

fn run(mut cmd: Command, cache: &Path) -> (Output, f64) {
    cmd.env("PRC_RUNNER_CACHE", cache).env("LC_ALL", "en_US.UTF-8");
    // The stub server is on this machine: no proxy may stand between.
    for v in ["HTTP_PROXY", "HTTPS_PROXY", "ALL_PROXY", "http_proxy", "https_proxy", "all_proxy"] {
        cmd.env_remove(v);
    }
    cmd.stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped());
    let t = Instant::now();
    let mut child = cmd.spawn().expect("start the program");
    let _ = child.stdin.take().unwrap().write_all(b"");
    let out = child.wait_with_output().unwrap();
    (out, t.elapsed().as_secs_f64() * 1000.0)
}

/// What a run printed, line by line, without the interpreter's log lines.
fn said(o: &Output) -> Vec<String> {
    String::from_utf8_lossy(&o.stdout)
        .lines()
        .filter(|l| !(l.starts_with("\u{1b}[2m") && (l.contains(" WARN") || l.contains(" INFO"))))
        .map(|l| l.trim_end().to_string())
        .collect()
}

/// The folder the programs download into, made afresh before each run: the old copy a failed refresh
/// must keep, and a file the next download tries to make a folder of.
fn fresh(dir: &Path) {
    let _ = std::fs::remove_dir_all(dir);
    std::fs::create_dir_all(dir).unwrap();
    std::fs::write(dir.join("kept.bin"), b"yesterday's copy").unwrap();
    std::fs::write(dir.join("a-file"), b"").unwrap();
}

/// What a run left in the folder: each file's name and size, and the checks the interpreter's test
/// makes on them. Returns how many checks passed.
fn files_left(name: &str, dir: &Path) -> (Vec<String>, usize) {
    let mut found = Vec::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        for e in std::fs::read_dir(&d).unwrap().flatten() {
            let p = e.path();
            if p.is_dir() {
                stack.push(p);
            } else {
                found.push(format!("{} {}", p.strip_prefix(dir).unwrap().display(), std::fs::metadata(&p).unwrap().len()));
            }
        }
    }
    found.sort();
    let mut checks = 0;
    assert!(found.iter().all(|f| !f.contains(".part")), "{name}: no .part file is left: {found:?}");
    checks += 1;
    if name == "downloads" {
        assert_eq!(std::fs::read(dir.join("cache/photos/pet-1.bin")).unwrap(), every_byte(), "byte for byte, the folder made on the way");
        assert_eq!(std::fs::read(dir.join("one.bin")).unwrap(), b"the final body");
        assert_eq!(std::fs::read(dir.join("five.bin")).unwrap(), b"the final body", "five redirects are followed");
        assert!(!dir.join("six.bin").exists() && !dir.join("missing.bin").exists() && !dir.join("x.bin").exists(), "a failed download writes nothing");
        assert_eq!(std::fs::read(dir.join("kept.bin")).unwrap(), b"yesterday's copy", "a failed refresh keeps the old file");
        checks += 5;
    } else {
        assert_eq!(std::fs::read(dir.join("hello.txt")).unwrap(), b"hello, world");
        checks += 1;
    }
    (found, checks)
}

/// The requests the server saw from the download program: what the interpreter's test asserts of them.
fn header_checks(reqs: &[String]) -> usize {
    let lower: Vec<String> = reqs.iter().map(|r| r.to_ascii_lowercase()).collect();
    let i = lower.iter().position(|r| r.contains("x-trace: abc-123")).expect("the header set is sent");
    assert!(lower[i].starts_with("get /start "), "{}", reqs[i]);
    assert!(lower[i].contains("authorization: bearer"), "…and the credential, to the first address");
    assert!(lower[i + 1].contains("x-trace: abc-123"), "a plain header follows the redirect:\n{}", reqs[i + 1]);
    assert!(!lower[i + 1].contains("authorization"), "a credential does not:\n{}", reqs[i + 1]);
    let plain = lower.iter().find(|r| r.starts_with("get /plain ")).expect("the download after HTTP-CLEAR-HEADERS");
    assert!(!plain.contains("x-trace"), "cleared headers are not sent:\n{plain}");
    assert!(lower.iter().all(|r| !r.contains("content-type")), "a download sends no body and no Content-Type");
    6
}

fn refusal(src: &str, opts: &cobolt_wasm::Options) -> Vec<String> {
    cobolt_wasm::compile_program(&parse(src), opts)
        .err()
        .map(|e| e.all().into_iter().map(|(line, m)| format!("line {line}: {m}")).collect())
        .unwrap_or_default()
}

#[test]
fn the_http_builtins_match_the_interpreter() {
    let root = Scratch::new("http");
    let cache = root.0.join("cache");
    let server = serve();
    let base = format!("http://127.0.0.1:{}", server.port);
    // A port nobody listens on: bind one, learn its number, close it.
    let closed = TcpListener::bind("127.0.0.1:0").unwrap().local_addr().unwrap().port().to_string();
    let t = Instant::now();
    let rcrun = rcrun();
    let rcrun_build_ms = t.elapsed().as_secs_f64() * 1000.0;
    let runner = std::fs::read(env!("CARGO_BIN_EXE_prc-runner")).expect("the built prc-runner");
    let native = cobolt_wasm::Options { native: true, ..cobolt_wasm::Options::default() };
    let mut checks = 0;

    // Fewer arguments than a built-in takes: the generator refuses, so the Build takes the Cargo path.
    let head = "       IDENTIFICATION DIVISION.\n       PROGRAM-ID. SHORT.\n       DATA DIVISION.\n       WORKING-STORAGE SECTION.\n       01 WS-R PIC X(9).\n       PROCEDURE DIVISION.\n";
    let short = format!("{head}           COBOL::\"HTTP-GET\" ( \"http://x\" WS-R )\n           STOP RUN.\n");
    let refused = refusal(&short, &native);
    assert!(refused.iter().any(|m| m.contains("takes the URL and the data items")), "HTTP-GET with two arguments takes the Cargo path: {refused:?}");
    let short = format!("{head}           COBOL::\"HTTP-SET-HEADER\" ( \"X-A\" )\n           STOP RUN.\n");
    let refused = refusal(&short, &native);
    assert!(refused.iter().any(|m| m.contains("takes the header's name and its value")), "HTTP-SET-HEADER with one argument: {refused:?}");
    // A browser module still sends HTTP to its Backend.
    let page = format!("{head}           COBOL::\"HTTP-GET\" ( \"http://x\" WS-R WS-R )\n           STOP RUN.\n");
    let refused = refusal(&page, &cobolt_wasm::Options::default());
    assert!(refused.iter().any(|m| m.contains("run on the backend")), "a browser module refuses it: {refused:?}");
    checks += 3;

    let project = root.0.join("project");
    std::fs::create_dir_all(project.join("src")).unwrap();
    std::fs::write(project.join("parity.project.toml"), "[project]\nname = \"Parity\"\nversion = \"1.0.0\"\nmain = \"src/edges.cbl\"\n").unwrap();
    let files = root.0.join("files");

    let programs: [(&str, &str, Option<&[&str]>); 2] = [("edges", EDGES, None), ("downloads", DOWNLOADS, Some(&DOWNLOAD_LINES))];
    let mut rows = Vec::new();
    let (mut total_compile, mut total_runner, mut total_rcrun, mut lines, mut requests) = (0.0, 0.0, 0.0, 0usize, 0usize);
    for (name, src, expected) in programs {
        let src = src.replace("@BASE@", &base).replace("@DIR@", &files.display().to_string()).replace("@CLOSED@", &closed);
        let t = Instant::now();
        let module = cobolt_wasm::compile_program(&parse(&src), &native).unwrap_or_else(|e| panic!("{name}: the generator refused it: {e}")).wasm;
        total_compile += t.elapsed().as_secs_f64() * 1000.0;
        let scan = cobolt_wasm_abi::scan::scan_module(&module).unwrap();
        let caps = scan.embedded_manifest().unwrap().capabilities;
        assert!(caps.iter().any(|c| c == "http"), "{name}: the manifest asks for `http`: {caps:?}");
        assert!(scan.imports.iter().any(|i| i.module == "prn" && i.name == "http_call"), "{name}: the module imports prn.http_call");
        checks += 2;

        let ours = root.0.join(name);
        std::fs::create_dir_all(&ours).unwrap();
        let exe = ours.join(format!("prog{}", std::env::consts::EXE_SUFFIX));
        let payload = cobolt_program_package::encode(&[(Kind::Module, &module)]);
        let packaged = cobolt_program_package::embed(&runner, &payload, "prc-t42-http").expect("the runner takes a payload");
        cobolt_program_package::write_executable(&exe, &packaged).expect("write the program");

        fresh(&files);
        server.take();
        let mut a = Command::new(&exe);
        a.current_dir(&ours);
        let (runner_out, runner_ms) = run(a, &cache);
        let runner_requests = server.take();
        assert!(runner_out.status.success(), "{name}: the runner failed: {}\n{}", String::from_utf8_lossy(&runner_out.stderr), said(&runner_out).join("\n"));
        let (runner_files, n) = files_left(name, &files);
        checks += n;

        fresh(&files);
        std::fs::write(project.join("src").join(format!("{name}.cbl")), &src).unwrap();
        let mut b = Command::new(&rcrun);
        b.args(["run", &format!("src/{name}.cbl"), "--source-format", "free"]).current_dir(&project);
        let (rcrun_out, rcrun_ms) = run(b, &cache);
        let rcrun_requests = server.take();
        assert!(rcrun_out.status.success(), "{name}: rcrun run failed: {}", String::from_utf8_lossy(&rcrun_out.stderr));
        let (rcrun_files, n) = files_left(name, &files);
        checks += n;

        let (ours_said, theirs_said) = (said(&runner_out), said(&rcrun_out));
        assert_eq!(ours_said, theirs_said, "{name}: the runner prints what rcrun run prints\nrunner stderr: {}", String::from_utf8_lossy(&runner_out.stderr));
        assert_eq!(runner_requests, rcrun_requests, "{name}: the server sees the same requests from both paths");
        assert_eq!(runner_files, rcrun_files, "{name}: both paths leave the same files");
        checks += 5;
        if let Some(expected) = expected {
            assert_eq!(ours_said, expected, "{name}: the codes and words the interpreter's test asserts");
            checks += 1 + header_checks(&runner_requests) + header_checks(&rcrun_requests);
        }
        if std::env::var_os("PRC_RUNNER_SHOW").is_some() {
            println!("{name}:\n  {}", ours_said.join("\n  "));
        }
        lines += ours_said.len();
        requests += runner_requests.len();
        total_runner += runner_ms;
        total_rcrun += rcrun_ms;
        rows.push(format!(
            "  {name:<10} {:>3} lines  {:>3} requests  {:>2} files  module {:>6} bytes  runner {runner_ms:>6.1} ms  rcrun {rcrun_ms:>6.1} ms",
            ours_said.len(),
            runner_requests.len(),
            runner_files.len(),
            module.len()
        ));
    }

    println!("── the_http_builtins_match_the_interpreter (DESKTOP-WASM-001 T42) ──");
    println!("  programs: every verb and receiver, and the interpreter's download test's cases, against a stub server on 127.0.0.1");
    println!("  forms:    GET (200, 404, 418, a redirect, a STRING-built URL, spaces around one), POST (default Content-Type), PUT, DELETE,");
    println!("            SET-HEADER (seen by a CALLed program's CALL \"COBOL-HTTP-GET\"), CLEAR-HEADERS; a body into X(70), a group,");
    println!("            JUSTIFIED X(12) and X(8), X(40010) for 40 000 bytes, 9(4), S9(3)V99, ZZ9.99, X(4) across a UTF-8 character;");
    println!("            the code into 9(3), X(5), 9(2), ZZZ9; DOWNLOAD: every byte, folders made, 1/5/6 redirects, 404, 500 keeping a");
    println!("            file, credentials stopping at a redirect, 5 addresses that are no URL, a closed port, a file in a file, no name;");
    println!("            refused: HTTP-GET with 2 arguments, HTTP-SET-HEADER with 1, HTTP-GET in a browser module");
    for r in &rows {
        println!("{r}");
    }
    println!("  timings:  rcrun built {rcrun_build_ms:.0} ms; compiled in {total_compile:.1} ms; runner {total_runner:.1} ms, rcrun run {total_rcrun:.1} ms");
    println!("  result:   {checks} of {checks} checks passed; {lines} lines and {requests} requests the same on both paths");
}
