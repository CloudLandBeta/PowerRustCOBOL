// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! **The development server of the web Preview** (WASM-008, WASM-002 R20).
//!
//! A static file server for a Web bundle, on `127.0.0.1` only, built on
//! `std::net` the way the MCP endpoint of [`crate::http`] is. What a browser
//! needs of it, and nothing more:
//!
//! - the right **MIME type** for every file of a bundle — `application/wasm`
//!   above all, because streaming instantiation refuses a `.wasm` served as
//!   anything else;
//! - the **isolation headers** the project's setting asks for
//!   (`Cross-Origin-Opener-Policy`, `Cross-Origin-Embedder-Policy`), so the
//!   Preview runs with the same threads a deployed page does;
//! - **no path outside the bundle**, however it is spelt — `..`, encoded dots
//!   and separators, a backslash, a NUL, a symlink that points out;
//! - a **`Host` check**, so a page on another site cannot reach the server by
//!   pointing its own name at 127.0.0.1 (DNS rebinding);
//! - **live reload**: the server watches the bundle folder, counts the changes
//!   (`/__prc/version`) and serves the few lines of script that poll the count
//!   and reload the page. The script is added to `index.html` as it is sent;
//!   the file on disk is never touched.
//!
//! One connection, one request, one thread; `Connection: close`. A Preview
//! serves one developer.

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{Shutdown, SocketAddr, TcpListener, TcpStream};
use std::path::{Component, Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use cobolt_compiler::project_kind::Isolation;

/// The path the page polls for the change count.
pub const VERSION_PATH: &str = "/__prc/version";
/// The path of the live-reload script the server adds to `index.html`.
pub const RELOAD_SCRIPT_PATH: &str = "/__prc/livereload.js";
/// The largest request head the server reads (request line and headers).
const MAX_HEAD: usize = 16 * 1024;
/// How often the watcher looks at the bundle folder.
pub const WATCH_INTERVAL: Duration = Duration::from_millis(150);

/// How a server is started.
#[derive(Debug, Clone)]
pub struct ServeOptions {
    /// The port; 0 asks the system for a free one.
    pub port: u16,
    /// The isolation headers every response carries.
    pub isolation: Isolation,
    /// Add the live-reload script to `index.html` and watch the folder.
    pub live_reload: bool,
}

impl Default for ServeOptions {
    fn default() -> Self {
        Self { port: 0, isolation: Isolation::default(), live_reload: true }
    }
}

/// Why a server could not be started.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ServeError {
    /// The folder to serve is not there.
    NoFolder(PathBuf),
    /// The system refused the socket.
    Bind(String),
}

impl std::fmt::Display for ServeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ServeError::NoFolder(p) => write!(f, "there is no folder to serve at {}", p.display()),
            ServeError::Bind(e) => write!(f, "the development server could not listen: {e}"),
        }
    }
}

impl std::error::Error for ServeError {}

/// What the server shares with its handler threads.
struct State {
    /// The canonical folder being served.
    root: PathBuf,
    isolation: Isolation,
    live_reload: bool,
    /// Counts the changes the watcher has seen.
    version: AtomicU64,
    /// When the last change was seen, for the report.
    last_change: Mutex<Option<Instant>>,
    stop: AtomicBool,
    port: u16,
    /// Requests answered, for the tests and the Output panel.
    served: AtomicU64,
}

/// A running server. Dropping it stops it.
pub struct ServerHandle {
    state: Arc<State>,
    threads: Vec<thread::JoinHandle<()>>,
}

impl ServerHandle {
    /// The address a browser opens: `http://127.0.0.1:<port>/`.
    pub fn url(&self) -> String {
        format!("http://127.0.0.1:{}/", self.state.port)
    }

    pub fn port(&self) -> u16 {
        self.state.port
    }

    /// The folder being served.
    pub fn root(&self) -> &Path {
        &self.state.root
    }

    /// How many changes the watcher has seen; what `/__prc/version` answers.
    pub fn version(&self) -> u64 {
        self.state.version.load(Ordering::SeqCst)
    }

    /// Count a change now, without waiting for the watcher: a build that has
    /// just finished says so, and the page reloads at its next poll.
    pub fn notify_changed(&self) {
        self.state.version.fetch_add(1, Ordering::SeqCst);
        if let Ok(mut t) = self.state.last_change.lock() {
            *t = Some(Instant::now());
        }
    }

    /// When the last change was seen.
    pub fn last_change(&self) -> Option<Instant> {
        self.state.last_change.lock().ok().and_then(|t| *t)
    }

    /// How many requests have been answered.
    pub fn requests_served(&self) -> u64 {
        self.state.served.load(Ordering::SeqCst)
    }

    /// Stop the server and wait for its threads.
    pub fn stop(&mut self) {
        self.state.stop.store(true, Ordering::SeqCst);
        // Wake the accept that is waiting for a connection.
        let _ = TcpStream::connect_timeout(&SocketAddr::from(([127, 0, 0, 1], self.state.port)), Duration::from_millis(500));
        for t in self.threads.drain(..) {
            let _ = t.join();
        }
    }
}

impl Drop for ServerHandle {
    fn drop(&mut self) {
        self.stop();
    }
}

/// Serve `dir` on a loopback port.
pub fn start(dir: &Path, options: &ServeOptions) -> Result<ServerHandle, ServeError> {
    let root = std::fs::canonicalize(dir).map_err(|_| ServeError::NoFolder(dir.to_path_buf()))?;
    if !root.is_dir() {
        return Err(ServeError::NoFolder(dir.to_path_buf()));
    }
    // Loopback only, by construction: there is no way to ask for another address.
    let listener = TcpListener::bind(SocketAddr::from(([127, 0, 0, 1], options.port)))
        .map_err(|e| ServeError::Bind(e.to_string()))?;
    let port = listener.local_addr().map_err(|e| ServeError::Bind(e.to_string()))?.port();
    let state = Arc::new(State {
        root: root.clone(),
        isolation: options.isolation,
        live_reload: options.live_reload,
        version: AtomicU64::new(1),
        last_change: Mutex::new(None),
        stop: AtomicBool::new(false),
        port,
        served: AtomicU64::new(0),
    });
    let mut threads = Vec::new();
    {
        let state = state.clone();
        threads.push(thread::spawn(move || accept_loop(listener, state)));
    }
    if options.live_reload {
        let state = state.clone();
        threads.push(thread::spawn(move || watch_loop(state)));
    }
    Ok(ServerHandle { state, threads })
}

/// Accept until told to stop. The listener blocks (a non-blocking one would
/// poll, and hand every connection its sockets non-blocking on macOS, where a
/// large file then fails the first time the send buffer fills); [`ServerHandle::stop`]
/// wakes it with a connection of its own.
fn accept_loop(listener: TcpListener, state: Arc<State>) {
    while !state.stop.load(Ordering::SeqCst) {
        match listener.accept() {
            Ok((stream, _)) => {
                if state.stop.load(Ordering::SeqCst) {
                    break;
                }
                let state = state.clone();
                thread::spawn(move || {
                    let _ = handle(stream, &state);
                });
            }
            Err(_) => thread::sleep(Duration::from_millis(20)),
        }
    }
}

/// A fingerprint of the folder: every file's path, length and modification
/// time. Cheap enough to take several times a second for a bundle of a few
/// dozen files.
pub fn fingerprint(root: &Path) -> u64 {
    fn walk(dir: &Path, h: &mut DefaultHasher, depth: usize) {
        if depth > 8 {
            return;
        }
        let Ok(rd) = std::fs::read_dir(dir) else { return };
        let mut entries: Vec<_> = rd.flatten().collect();
        entries.sort_by_key(|e| e.file_name());
        for e in entries {
            let p = e.path();
            let Ok(m) = e.metadata() else { continue };
            if m.is_dir() {
                walk(&p, h, depth + 1);
            } else {
                p.hash(h);
                m.len().hash(h);
                if let Ok(t) = m.modified() {
                    if let Ok(d) = t.duration_since(std::time::UNIX_EPOCH) {
                        d.as_nanos().hash(h);
                    }
                }
            }
        }
    }
    let mut h = DefaultHasher::new();
    walk(root, &mut h, 0);
    h.finish()
}

fn watch_loop(state: Arc<State>) {
    let mut last = fingerprint(&state.root);
    while !state.stop.load(Ordering::SeqCst) {
        thread::sleep(WATCH_INTERVAL);
        let now = fingerprint(&state.root);
        if now != last {
            last = now;
            state.version.fetch_add(1, Ordering::SeqCst);
            if let Ok(mut t) = state.last_change.lock() {
                *t = Some(Instant::now());
            }
        }
    }
}

// ── MIME ─────────────────────────────────────────────────────────────────────

/// The `Content-Type` of a file, by extension. `.wasm` is `application/wasm`
/// (streaming instantiation requires it); anything unknown is a download.
pub fn mime_for(path: &Path) -> &'static str {
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("").to_ascii_lowercase();
    match ext.as_str() {
        "html" | "htm" => "text/html; charset=utf-8",
        "js" | "mjs" => "text/javascript; charset=utf-8",
        "css" => "text/css; charset=utf-8",
        "json" | "map" => "application/json; charset=utf-8",
        "webmanifest" => "application/manifest+json; charset=utf-8",
        "wasm" => "application/wasm",
        "txt" | "md" | "cfrm" | "cidx" | "yaml" | "yml" => "text/plain; charset=utf-8",
        "xml" => "application/xml; charset=utf-8",
        "svg" => "image/svg+xml",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "avif" => "image/avif",
        "ico" => "image/x-icon",
        "woff2" => "font/woff2",
        "woff" => "font/woff",
        "ttf" => "font/ttf",
        "otf" => "font/otf",
        "mp3" => "audio/mpeg",
        "ogg" => "audio/ogg",
        "wav" => "audio/wav",
        "mp4" => "video/mp4",
        "webm" => "video/webm",
        "pdf" => "application/pdf",
        _ => "application/octet-stream",
    }
}

// ── Paths ────────────────────────────────────────────────────────────────────

/// Why a request path was refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PathProblem {
    /// Not an absolute path of the form `/…`.
    NotAbsolute,
    /// A `..`, a NUL, a backslash or an encoded separator: an attempt to leave.
    Traversal,
    /// Percent-encoding that is not valid UTF-8.
    Encoding,
}

/// The request path (without its query) as a relative path inside the bundle.
///
/// The path is decoded **once**; a result that still contains a `%` is not
/// decoded again, so `%252e%252e` is the file name `%2e%2e`, never `..`.
/// Anything that could name a place outside the folder — a `..` segment, a
/// backslash, a NUL, an encoded `/` inside a segment — is refused outright
/// rather than cleaned up.
pub fn relative_path(request_path: &str) -> Result<PathBuf, PathProblem> {
    let raw = request_path.split(['?', '#']).next().unwrap_or("");
    if !raw.starts_with('/') {
        return Err(PathProblem::NotAbsolute);
    }
    let decoded = percent_decode(raw).ok_or(PathProblem::Encoding)?;
    if decoded.contains('\0') || decoded.contains('\\') {
        return Err(PathProblem::Traversal);
    }
    let mut out = PathBuf::new();
    for seg in decoded.split('/') {
        match seg {
            "" | "." => {}
            ".." => return Err(PathProblem::Traversal),
            s => {
                // A segment may not itself carry a separator or a drive.
                if s.contains(':') && cfg!(windows) {
                    return Err(PathProblem::Traversal);
                }
                out.push(s);
            }
        }
    }
    // Belt and braces: after joining, nothing but normal components remains.
    if out.components().any(|c| !matches!(c, Component::Normal(_))) {
        return Err(PathProblem::Traversal);
    }
    Ok(out)
}

/// Decode `%XX` once. `None` for a bad escape or a result that is not UTF-8.
fn percent_decode(s: &str) -> Option<String> {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%' {
            let hex = std::str::from_utf8(b.get(i + 1..i + 3)?).ok()?;
            out.push(u8::from_str_radix(hex, 16).ok()?);
            i += 3;
        } else {
            out.push(b[i]);
            i += 1;
        }
    }
    String::from_utf8(out).ok()
}

/// The file a request path names, or why it names none: the path is checked
/// ([`relative_path`]), joined to `root`, a directory becomes its
/// `index.html`, and the **canonical** result must still be inside `root` —
/// which is what stops a symlink inside the bundle from leading out of it.
pub fn resolve(root: &Path, request_path: &str) -> Result<PathBuf, ResolveError> {
    let rel = relative_path(request_path).map_err(ResolveError::Refused)?;
    let mut candidate = root.join(rel);
    if candidate.is_dir() {
        candidate.push("index.html");
    }
    let canonical = std::fs::canonicalize(&candidate).map_err(|_| ResolveError::NotFound)?;
    if !canonical.starts_with(root) {
        return Err(ResolveError::Refused(PathProblem::Traversal));
    }
    if !canonical.is_file() {
        return Err(ResolveError::NotFound);
    }
    Ok(canonical)
}

/// Why [`resolve`] found no file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResolveError {
    Refused(PathProblem),
    NotFound,
}

// ── One request ──────────────────────────────────────────────────────────────

struct Request {
    method: String,
    path: String,
    host: Option<String>,
}

fn read_request(stream: &TcpStream) -> Result<Request, (u16, &'static str)> {
    stream.set_read_timeout(Some(Duration::from_secs(5))).ok();
    let mut reader = BufReader::new(stream.take(MAX_HEAD as u64));
    let mut line = String::new();
    reader.read_line(&mut line).map_err(|_| (400, "bad request"))?;
    let mut parts = line.split_whitespace();
    let method = parts.next().ok_or((400, "bad request"))?.to_owned();
    let path = parts.next().ok_or((400, "bad request"))?.to_owned();
    if !parts.next().is_some_and(|v| v.starts_with("HTTP/1.")) {
        return Err((400, "bad request"));
    }
    let mut host = None;
    let mut read = line.len();
    loop {
        let mut h = String::new();
        let n = reader.read_line(&mut h).map_err(|_| (400, "bad request"))?;
        read += n;
        if n == 0 || read >= MAX_HEAD {
            return Err((431, "request head too large"));
        }
        let h = h.trim_end();
        if h.is_empty() {
            break;
        }
        if let Some((name, value)) = h.split_once(':') {
            if name.eq_ignore_ascii_case("host") {
                host = Some(value.trim().to_owned());
            }
        }
    }
    Ok(Request { method, path, host })
}

/// Is `host` (a `Host` header) one of this server's own names?
fn host_is_ours(host: Option<&str>, port: u16) -> bool {
    let Some(host) = host else { return false };
    let host = host.to_ascii_lowercase();
    host == format!("127.0.0.1:{port}") || host == format!("localhost:{port}") || host == format!("[::1]:{port}")
}

fn respond(
    stream: &mut TcpStream,
    status: u16,
    reason: &str,
    content_type: &str,
    extra: &[(&str, String)],
    body: &[u8],
    head_only: bool,
    state: &State,
) -> std::io::Result<()> {
    let mut head = format!(
        "HTTP/1.1 {status} {reason}\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\n\
         Cache-Control: no-store\r\nX-Content-Type-Options: nosniff\r\nConnection: close\r\n",
        body.len()
    );
    for (k, v) in state.isolation.headers() {
        head.push_str(&format!("{k}: {v}\r\n"));
    }
    // A page isolated by `require-corp` may load only what allows it; the
    // server's own files allow it.
    if !state.isolation.headers().is_empty() {
        head.push_str("Cross-Origin-Resource-Policy: same-origin\r\n");
    }
    for (k, v) in extra {
        head.push_str(&format!("{k}: {v}\r\n"));
    }
    head.push_str("\r\n");
    stream.write_all(head.as_bytes())?;
    if !head_only {
        stream.write_all(body)?;
    }
    stream.flush()
}

fn text_response(stream: &mut TcpStream, status: u16, reason: &str, body: &str, head_only: bool, state: &State) -> std::io::Result<()> {
    respond(stream, status, reason, "text/plain; charset=utf-8", &[], body.as_bytes(), head_only, state)
}

/// The script added to `index.html`: poll the change count, reload on a change.
fn reload_script() -> &'static str {
    "(function(){var v=null;function poll(){fetch('/__prc/version',{cache:'no-store'})\
     .then(function(r){return r.json();}).then(function(j){if(v===null){v=j.version;}else if(j.version!==v){location.reload();}})\
     .catch(function(){}).finally(function(){setTimeout(poll,400);});}poll();})();\n"
}

/// `html` with the live-reload script tag before `</body>` (or at the end).
pub fn with_reload_script(html: &[u8]) -> Vec<u8> {
    let tag = format!("<script src=\"{RELOAD_SCRIPT_PATH}\"></script>");
    let text = String::from_utf8_lossy(html);
    let at = text.to_ascii_lowercase().rfind("</body>").unwrap_or(text.len());
    let mut out = String::with_capacity(text.len() + tag.len());
    out.push_str(&text[..at]);
    out.push_str(&tag);
    out.push_str(&text[at..]);
    out.into_bytes()
}

fn handle(mut stream: TcpStream, state: &State) -> std::io::Result<()> {
    stream.set_nodelay(true).ok();
    let request = match read_request(&stream) {
        Ok(r) => r,
        Err((code, why)) => {
            let reason = if code == 431 { "Request Header Fields Too Large" } else { "Bad Request" };
            text_response(&mut stream, code, reason, why, false, state)?;
            return finish(stream, state);
        }
    };
    let head_only = request.method == "HEAD";
    if request.method != "GET" && !head_only {
        text_response(&mut stream, 405, "Method Not Allowed", "only GET and HEAD", false, state)?;
        return finish(stream, state);
    }
    // A name other than our own on the Host header is a page elsewhere reaching
    // in (DNS rebinding); it is refused before any file is looked at.
    if !host_is_ours(request.host.as_deref(), state.port) {
        text_response(&mut stream, 403, "Forbidden", "unknown host", head_only, state)?;
        return finish(stream, state);
    }
    let bare = request.path.split(['?', '#']).next().unwrap_or("");
    if state.live_reload && bare == VERSION_PATH {
        let body = format!("{{\"version\":{}}}", state.version.load(Ordering::SeqCst));
        respond(&mut stream, 200, "OK", "application/json; charset=utf-8", &[], body.as_bytes(), head_only, state)?;
        return finish(stream, state);
    }
    if state.live_reload && bare == RELOAD_SCRIPT_PATH {
        respond(&mut stream, 200, "OK", "text/javascript; charset=utf-8", &[], reload_script().as_bytes(), head_only, state)?;
        return finish(stream, state);
    }
    match resolve(&state.root, &request.path) {
        Ok(file) => match std::fs::read(&file) {
            Ok(mut body) => {
                let mime = mime_for(&file);
                if state.live_reload && file.file_name().is_some_and(|n| n == "index.html") {
                    body = with_reload_script(&body);
                }
                respond(&mut stream, 200, "OK", mime, &[], &body, head_only, state)?;
            }
            Err(_) => text_response(&mut stream, 404, "Not Found", "not found", head_only, state)?,
        },
        Err(ResolveError::Refused(_)) => text_response(&mut stream, 403, "Forbidden", "refused", head_only, state)?,
        Err(ResolveError::NotFound) => text_response(&mut stream, 404, "Not Found", "not found", head_only, state)?,
    }
    finish(stream, state)
}

fn finish(mut stream: TcpStream, state: &State) -> std::io::Result<()> {
    state.served.fetch_add(1, Ordering::SeqCst);
    let _ = stream.shutdown(Shutdown::Write);
    // Whatever the client is still sending (the rest of an over-long head) is
    // read and dropped before the socket closes: closing with unread input
    // makes the system reset the connection, and the client would lose the
    // answer it was owed.
    stream.set_read_timeout(Some(Duration::from_millis(200))).ok();
    let mut sink = [0u8; 4096];
    let mut drained = 0usize;
    while drained < 256 * 1024 {
        match stream.read(&mut sink) {
            Ok(0) | Err(_) => break,
            Ok(n) => drained += n,
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Write};

    fn scratch(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("prc-devserver-{tag}-{}-{}", std::process::id(), line!()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    /// A small bundle: a page, the two `.wasm` files, a script, a stylesheet
    /// and a file in a sub-folder; plus a secret beside the bundle.
    fn bundle(tag: &str) -> (PathBuf, PathBuf) {
        let outer = scratch(tag);
        let root = outer.join("bundle");
        std::fs::create_dir_all(root.join("assets")).unwrap();
        std::fs::write(root.join("index.html"), "<!doctype html><html><body><div id=app></div></body></html>").unwrap();
        std::fs::write(root.join("app.js"), "console.log('hi')").unwrap();
        std::fs::write(root.join("app.css"), "body{margin:0}").unwrap();
        std::fs::write(root.join("program.wasm"), b"\0asm\x01\0\0\0").unwrap();
        std::fs::write(root.join("player.wasm"), b"\0asm\x01\0\0\0").unwrap();
        std::fs::write(root.join("assets/logo.png"), b"\x89PNG").unwrap();
        std::fs::write(root.join("assets/font.woff2"), b"wOF2").unwrap();
        std::fs::write(outer.join("secret.txt"), "TOP SECRET").unwrap();
        (outer, root)
    }

    struct Reply {
        status: u16,
        headers: Vec<(String, String)>,
        body: Vec<u8>,
    }

    impl Reply {
        fn header(&self, name: &str) -> Option<&str> {
            self.headers.iter().find(|(k, _)| k.eq_ignore_ascii_case(name)).map(|(_, v)| v.as_str())
        }
    }

    /// Send raw bytes, read the whole answer.
    fn raw(port: u16, request: &str) -> Reply {
        let mut s = TcpStream::connect(("127.0.0.1", port)).unwrap();
        s.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
        s.write_all(request.as_bytes()).unwrap();
        let mut buf = Vec::new();
        let _ = s.read_to_end(&mut buf);
        let split = buf.windows(4).position(|w| w == b"\r\n\r\n").expect("a response head");
        let head = String::from_utf8_lossy(&buf[..split]).into_owned();
        let mut lines = head.lines();
        let status = lines.next().unwrap().split_whitespace().nth(1).unwrap().parse().unwrap();
        let headers = lines
            .filter_map(|l| l.split_once(':').map(|(k, v)| (k.trim().to_owned(), v.trim().to_owned())))
            .collect();
        Reply { status, headers, body: buf[split + 4..].to_vec() }
    }

    fn get(port: u16, path: &str) -> Reply {
        raw(port, &format!("GET {path} HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\n\r\n"))
    }

    #[test]
    fn every_file_of_a_bundle_is_served_with_its_mime_type() {
        let (outer, root) = bundle("mime");
        let server = start(&root, &ServeOptions { live_reload: false, ..Default::default() }).unwrap();
        let cases = [
            ("/", "text/html; charset=utf-8"),
            ("/index.html", "text/html; charset=utf-8"),
            ("/app.js", "text/javascript; charset=utf-8"),
            ("/app.css", "text/css; charset=utf-8"),
            ("/program.wasm", "application/wasm"),
            ("/player.wasm", "application/wasm"),
            ("/assets/logo.png", "image/png"),
            ("/assets/font.woff2", "font/woff2"),
        ];
        for (path, mime) in cases {
            let r = get(server.port(), path);
            assert_eq!(r.status, 200, "{path}");
            assert_eq!(r.header("content-type"), Some(mime), "{path}");
            assert_eq!(r.header("content-length").and_then(|v| v.parse::<usize>().ok()), Some(r.body.len()), "{path}");
            assert_eq!(r.header("x-content-type-options"), Some("nosniff"));
        }
        assert_eq!(get(server.port(), "/program.wasm").body, b"\0asm\x01\0\0\0");
        assert_eq!(get(server.port(), "/missing.js").status, 404);
        println!("devserver: {} files served with their MIME types; .wasm is application/wasm; an absent file is 404", cases.len());
        drop(server);
        let _ = std::fs::remove_dir_all(outer);
    }

    #[test]
    fn the_extension_table_knows_what_a_bundle_holds() {
        for (name, mime) in [
            ("a.wasm", "application/wasm"),
            ("a.WASM", "application/wasm"),
            ("a.mjs", "text/javascript; charset=utf-8"),
            ("a.json", "application/json; charset=utf-8"),
            ("a.svg", "image/svg+xml"),
            ("a.woff", "font/woff"),
            ("a.unknown", "application/octet-stream"),
            ("noextension", "application/octet-stream"),
        ] {
            assert_eq!(mime_for(Path::new(name)), mime, "{name}");
        }
    }

    /// WASM-002 R20: the isolation headers the project's setting asks for, on
    /// every response, and none when isolation is off.
    #[test]
    fn the_isolation_headers_follow_the_setting() {
        let (outer, root) = bundle("isolation");
        for (iso, coep) in [(Isolation::RequireCorp, Some("require-corp")), (Isolation::Credentialless, Some("credentialless")), (Isolation::Off, None)] {
            let server = start(&root, &ServeOptions { isolation: iso, live_reload: false, ..Default::default() }).unwrap();
            for path in ["/", "/program.wasm", "/nope"] {
                let r = get(server.port(), path);
                assert_eq!(r.header("cross-origin-embedder-policy"), coep, "{iso:?} {path}");
                assert_eq!(r.header("cross-origin-opener-policy"), coep.map(|_| "same-origin"), "{iso:?} {path}");
            }
        }
        println!("devserver: COOP/COEP on every response for require-corp and credentialless, on none for off");
        let _ = std::fs::remove_dir_all(outer);
    }

    /// The path-traversal refusals: every spelling of "outside the bundle".
    #[test]
    fn no_spelling_of_a_path_outside_the_bundle_is_served() {
        let (outer, root) = bundle("traversal");
        let server = start(&root, &ServeOptions { live_reload: false, ..Default::default() }).unwrap();
        let attempts = [
            "/../secret.txt",
            "/assets/../../secret.txt",
            "/%2e%2e/secret.txt",
            "/%2E%2E/secret.txt",
            "/assets/%2e%2e/%2e%2e/secret.txt",
            "/..%2fsecret.txt",
            "/%2e%2e%2fsecret.txt",
            "/..%5csecret.txt",
            "/..\\secret.txt",
            "/assets%5c..%5c..%5csecret.txt",
            "/%00",
            "/index.html%00.txt",
            "/assets/\0/../../secret.txt",
        ];
        for path in attempts {
            // A raw NUL cannot be sent in a request line the server accepts;
            // the encoded forms are what a browser can send.
            let r = raw(server.port(), &format!("GET {path} HTTP/1.1\r\nHost: 127.0.0.1:{}\r\n\r\n", server.port()));
            assert!(r.status == 403 || r.status == 404 || r.status == 400, "{path:?} -> {}", r.status);
            assert!(!String::from_utf8_lossy(&r.body).contains("TOP SECRET"), "{path:?} leaked the secret");
        }
        // A double-encoded dot is a file NAME with a percent sign, not a dot.
        let r = get(server.port(), "/%252e%252e/secret.txt");
        assert_eq!(r.status, 404);
        assert!(!String::from_utf8_lossy(&r.body).contains("TOP SECRET"));
        // An absolute-form or scheme-bearing target is not a path of ours.
        let r = raw(server.port(), &format!("GET http://evil.example/secret.txt HTTP/1.1\r\nHost: 127.0.0.1:{}\r\n\r\n", server.port()));
        assert!(r.status == 403 || r.status == 404, "{}", r.status);
        println!("devserver: {} spellings of a path outside the bundle refused, none leaked", attempts.len() + 2);
        drop(server);
        let _ = std::fs::remove_dir_all(outer);
    }

    /// A symlink inside the bundle that points out of it does not lead out.
    #[cfg(unix)]
    #[test]
    fn a_symlink_that_leaves_the_bundle_is_not_followed() {
        let (outer, root) = bundle("symlink");
        std::os::unix::fs::symlink(outer.join("secret.txt"), root.join("link.txt")).unwrap();
        std::os::unix::fs::symlink(&outer, root.join("up")).unwrap();
        let server = start(&root, &ServeOptions { live_reload: false, ..Default::default() }).unwrap();
        for path in ["/link.txt", "/up/secret.txt"] {
            let r = get(server.port(), path);
            assert_eq!(r.status, 403, "{path}");
            assert!(!String::from_utf8_lossy(&r.body).contains("TOP SECRET"));
        }
        drop(server);
        let _ = std::fs::remove_dir_all(outer);
    }

    #[test]
    fn the_path_parser_decodes_once_and_refuses_what_could_leave() {
        assert_eq!(relative_path("/").unwrap(), PathBuf::new());
        assert_eq!(relative_path("/a/b.js?x=1#y").unwrap(), PathBuf::from("a/b.js"));
        assert_eq!(relative_path("/a%20b/c.js").unwrap(), PathBuf::from("a b/c.js"));
        assert_eq!(relative_path("/a//b/./c").unwrap(), PathBuf::from("a/b/c"));
        assert_eq!(relative_path("/%252e%252e/x").unwrap(), PathBuf::from("%2e%2e/x"), "decoded once");
        assert_eq!(relative_path("/a/../b"), Err(PathProblem::Traversal));
        assert_eq!(relative_path("/%2e%2E/b"), Err(PathProblem::Traversal));
        assert_eq!(relative_path("/a%5cb"), Err(PathProblem::Traversal));
        assert_eq!(relative_path("/a%00b"), Err(PathProblem::Traversal));
        assert_eq!(relative_path("a/b"), Err(PathProblem::NotAbsolute));
        assert_eq!(relative_path("/%ff"), Err(PathProblem::Encoding));
        assert_eq!(relative_path("/%zz"), Err(PathProblem::Encoding));
        assert_eq!(relative_path("/%2"), Err(PathProblem::Encoding));
    }

    /// DNS rebinding: a request whose Host header is not the server's own is refused.
    #[test]
    fn a_foreign_host_header_is_refused() {
        let (outer, root) = bundle("host");
        let server = start(&root, &ServeOptions { live_reload: false, ..Default::default() }).unwrap();
        let p = server.port();
        assert_eq!(get(p, "/").status, 200);
        assert_eq!(raw(p, &format!("GET / HTTP/1.1\r\nHost: localhost:{p}\r\n\r\n")).status, 200);
        assert_eq!(raw(p, &format!("GET / HTTP/1.1\r\nHost: evil.example:{p}\r\n\r\n")).status, 403);
        assert_eq!(raw(p, "GET / HTTP/1.1\r\nHost: 127.0.0.1\r\n\r\n").status, 403, "the wrong port is not ours");
        assert_eq!(raw(p, "GET / HTTP/1.1\r\n\r\n").status, 403, "no Host at all");
        drop(server);
        let _ = std::fs::remove_dir_all(outer);
    }

    #[test]
    fn only_get_and_head_are_answered_and_head_has_no_body() {
        let (outer, root) = bundle("methods");
        let server = start(&root, &ServeOptions { live_reload: false, ..Default::default() }).unwrap();
        let p = server.port();
        let head = raw(p, &format!("HEAD /app.js HTTP/1.1\r\nHost: 127.0.0.1:{p}\r\n\r\n"));
        assert_eq!(head.status, 200);
        assert_eq!(head.header("content-length"), Some("17"));
        assert!(head.body.is_empty());
        for m in ["POST", "PUT", "DELETE"] {
            assert_eq!(raw(p, &format!("{m} /app.js HTTP/1.1\r\nHost: 127.0.0.1:{p}\r\nContent-Length: 0\r\n\r\n")).status, 405, "{m}");
        }
        assert_eq!(raw(p, "garbage\r\n\r\n").status, 400);
        let huge = format!("GET / HTTP/1.1\r\nHost: 127.0.0.1:{p}\r\nX-Pad: {}\r\n\r\n", "a".repeat(20_000));
        assert_eq!(raw(p, &huge).status, 431);
        drop(server);
        let _ = std::fs::remove_dir_all(outer);
    }

    #[test]
    fn it_listens_on_loopback_only() {
        let (outer, root) = bundle("loopback");
        let server = start(&root, &ServeOptions::default()).unwrap();
        assert!(server.url().starts_with("http://127.0.0.1:"), "{}", server.url());
        assert_ne!(server.port(), 0);
        let addr = TcpStream::connect(("127.0.0.1", server.port())).unwrap().peer_addr().unwrap();
        assert!(addr.ip().is_loopback());
        let e = start(&root.join("nope"), &ServeOptions::default()).err().unwrap();
        assert!(matches!(e, ServeError::NoFolder(_)), "{e}");
        drop(server);
        let _ = std::fs::remove_dir_all(outer);
    }

    /// Live reload: the script is added to `index.html` as it is sent (the file
    /// is untouched), and the change count moves when a file changes.
    #[test]
    fn live_reload_adds_its_script_and_counts_changes() {
        let (outer, root) = bundle("reload");
        let server = start(&root, &ServeOptions::default()).unwrap();
        let p = server.port();
        let page = get(p, "/");
        let text = String::from_utf8_lossy(&page.body).into_owned();
        assert!(text.contains("/__prc/livereload.js") && text.find("livereload").unwrap() < text.find("</body>").unwrap(), "{text}");
        assert!(!std::fs::read_to_string(root.join("index.html")).unwrap().contains("livereload"), "the file on disk is untouched");
        let script = get(p, RELOAD_SCRIPT_PATH);
        assert_eq!(script.status, 200);
        assert!(String::from_utf8_lossy(&script.body).contains("location.reload"));

        let before = server.version();
        let body = get(p, VERSION_PATH);
        assert_eq!(String::from_utf8_lossy(&body.body), format!("{{\"version\":{before}}}"));
        let started = Instant::now();
        std::fs::write(root.join("app.css"), "body{margin:1px;padding:2px}").unwrap();
        let mut waited = 0;
        while server.version() == before && waited < 4000 {
            thread::sleep(Duration::from_millis(10));
            waited += 10;
        }
        let latency = started.elapsed();
        assert!(server.version() > before, "the watcher saw the stylesheet change");
        assert!(server.last_change().is_some());
        println!("devserver: a changed stylesheet moved the change count in {} ms (watch interval {} ms)", latency.as_millis(), WATCH_INTERVAL.as_millis());
        assert!(latency < Duration::from_millis(2000));
        // A build that says it finished moves the count at once.
        let v = server.version();
        server.notify_changed();
        assert_eq!(server.version(), v + 1);
        drop(server);
        let _ = std::fs::remove_dir_all(outer);
    }

    #[test]
    fn with_live_reload_off_neither_the_script_nor_the_count_exist() {
        let (outer, root) = bundle("noreload");
        let server = start(&root, &ServeOptions { live_reload: false, ..Default::default() }).unwrap();
        let p = server.port();
        assert!(!String::from_utf8_lossy(&get(p, "/").body).contains("livereload"));
        assert_eq!(get(p, VERSION_PATH).status, 404);
        drop(server);
        let _ = std::fs::remove_dir_all(outer);
    }

    #[test]
    fn the_script_goes_before_the_closing_body_tag_or_at_the_end() {
        let a = String::from_utf8(with_reload_script(b"<html><body>x</body></html>")).unwrap();
        assert_eq!(a, "<html><body>x<script src=\"/__prc/livereload.js\"></script></body></html>");
        let b = String::from_utf8(with_reload_script(b"<p>no body tag")).unwrap();
        assert!(b.ends_with("<script src=\"/__prc/livereload.js\"></script>"));
        let c = String::from_utf8(with_reload_script(b"<HTML><BODY>x</BODY></HTML>")).unwrap();
        assert!(c.contains("</script></BODY>"), "{c}");
    }

    /// GOLDEN RULE #7: a measured account of the server.
    #[test]
    fn response_times_for_a_realistic_bundle() {
        let (outer, root) = bundle("perf");
        let big: Vec<u8> = (0..(3 * 1024 * 1024)).map(|i| (i % 251) as u8).collect();
        std::fs::write(root.join("program.wasm"), &big).unwrap();
        let server = start(&root, &ServeOptions { live_reload: false, ..Default::default() }).unwrap();
        let p = server.port();
        // Small files: 200 sequential requests.
        let t = Instant::now();
        for _ in 0..200 {
            assert_eq!(get(p, "/app.js").status, 200);
        }
        let small = t.elapsed();
        // The large module: 10 requests.
        let t = Instant::now();
        for _ in 0..10 {
            let r = get(p, "/program.wasm");
            assert_eq!(r.body.len(), big.len());
        }
        let large = t.elapsed();
        println!(
            "\n  devserver result\n    small file (app.js, 17 B): 200 requests in {} ms = {:.2} ms each, {:.0} req/s\n    program.wasm ({} MiB): 10 requests in {} ms = {:.1} ms each, {:.0} MiB/s",
            small.as_millis(),
            small.as_secs_f64() * 1000.0 / 200.0,
            200.0 / small.as_secs_f64(),
            big.len() / (1024 * 1024),
            large.as_millis(),
            large.as_secs_f64() * 1000.0 / 10.0,
            10.0 * (big.len() as f64 / (1024.0 * 1024.0)) / large.as_secs_f64()
        );
        assert!(small < Duration::from_secs(20) && large < Duration::from_secs(20));
        drop(server);
        let _ = std::fs::remove_dir_all(outer);
    }

    #[test]
    fn stopping_releases_the_port() {
        let (outer, root) = bundle("stop");
        let mut server = start(&root, &ServeOptions { live_reload: false, ..Default::default() }).unwrap();
        let port = server.port();
        server.stop();
        let again = start(&root, &ServeOptions { port, live_reload: false, ..Default::default() });
        assert!(again.is_ok(), "the port can be bound again after stop");
        drop(again);
        let _ = std::fs::remove_dir_all(outer);
    }
}
