// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! A small static file server for a Web bundle (WASM-002 R20: "the standalone server the IDE can
//! start"). It serves a folder with the right content types (`application/wasm` first among them —
//! a browser refuses streaming compilation without it), optionally with the cross-origin-isolation
//! headers, and records what it served so a test can prove what a page fetched and when.
//!
//! It is for development and for the tests, not for production: one thread per connection, no TLS,
//! `Connection: close`, no range requests. It refuses any path that leaves the folder.

use std::io::{self, Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::path::{Component, Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;

use crate::Isolation;

#[derive(Debug, Clone)]
pub struct ServeOptions {
    pub isolation: Isolation,
    /// Added to every response.
    pub extra_headers: Vec<(String, String)>,
    /// Serve index.html with `Cache-Control: no-cache` and everything else as given (default true).
    pub no_cache: bool,
}

impl Default for ServeOptions {
    fn default() -> Self {
        ServeOptions { isolation: Isolation::Off, extra_headers: Vec::new(), no_cache: true }
    }
}

pub struct Server {
    pub addr: SocketAddr,
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
    log: Arc<Mutex<Vec<String>>>,
}

impl Server {
    pub fn url(&self) -> String {
        format!("http://{}", self.addr)
    }

    /// The paths served so far, in order (`GET /index.html`).
    pub fn requests(&self) -> Vec<String> {
        self.log.lock().unwrap().clone()
    }

    pub fn clear_requests(&self) {
        self.log.lock().unwrap().clear();
    }

    pub fn stop(mut self) {
        self.stop_inner();
    }

    fn stop_inner(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        let _ = TcpStream::connect(self.addr);
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        self.stop_inner();
    }
}

/// The content type of a file by its extension.
pub fn mime(path: &str) -> &'static str {
    let ext = path.rsplit('.').next().unwrap_or("").to_ascii_lowercase();
    match ext.as_str() {
        "html" | "htm" => "text/html; charset=utf-8",
        "js" | "mjs" => "text/javascript; charset=utf-8",
        "css" => "text/css; charset=utf-8",
        "json" | "map" => "application/json; charset=utf-8",
        "wasm" => "application/wasm",
        "cfrm" | "wcfrm" | "mcfrm" | "xml" => "application/xml; charset=utf-8",
        "svg" => "image/svg+xml",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "ico" => "image/x-icon",
        "woff2" => "font/woff2",
        "woff" => "font/woff",
        "ttf" => "font/ttf",
        "txt" | "md" => "text/plain; charset=utf-8",
        "pdf" => "application/pdf",
        _ => "application/octet-stream",
    }
}

/// Resolve a request path below `root`, or `None` if it would leave it.
pub fn resolve(root: &Path, request_path: &str) -> Option<PathBuf> {
    let p = request_path.split(['?', '#']).next().unwrap_or("");
    let decoded = percent_decode(p)?;
    if decoded.contains('\\') || decoded.contains('\0') {
        return None;
    }
    let rel = decoded.trim_start_matches('/');
    let rel = if rel.is_empty() { "index.html" } else { rel };
    let mut out = root.to_path_buf();
    for c in Path::new(rel).components() {
        match c {
            Component::Normal(s) => out.push(s),
            Component::CurDir => {}
            _ => return None,
        }
    }
    Some(out)
}

fn percent_decode(s: &str) -> Option<String> {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%' {
            let h = std::str::from_utf8(b.get(i + 1..i + 3)?).ok()?;
            out.push(u8::from_str_radix(h, 16).ok()?);
            i += 3;
        } else {
            out.push(b[i]);
            i += 1;
        }
    }
    String::from_utf8(out).ok()
}

fn respond(stream: &mut TcpStream, status: &str, headers: &[(String, String)], body: &[u8], head_only: bool) {
    let mut head = format!("HTTP/1.1 {status}\r\n");
    for (k, v) in headers {
        head.push_str(&format!("{k}: {v}\r\n"));
    }
    head.push_str(&format!("Content-Length: {}\r\nConnection: close\r\n\r\n", body.len()));
    let _ = stream.write_all(head.as_bytes());
    if !head_only {
        let _ = stream.write_all(body);
    }
    let _ = stream.flush();
}

fn handle(mut stream: TcpStream, root: &Path, opts: &ServeOptions, log: &Mutex<Vec<String>>) {
    let mut buf = Vec::new();
    let mut chunk = [0u8; 2048];
    loop {
        match stream.read(&mut chunk) {
            Ok(0) => break,
            Ok(n) => {
                buf.extend_from_slice(&chunk[..n]);
                if buf.windows(4).any(|w| w == b"\r\n\r\n") || buf.len() > 16 * 1024 {
                    break;
                }
            }
            Err(_) => return,
        }
    }
    let head = String::from_utf8_lossy(&buf);
    let line = head.lines().next().unwrap_or("");
    let mut parts = line.split_whitespace();
    let (method, target) = (parts.next().unwrap_or(""), parts.next().unwrap_or("/"));
    let mut headers: Vec<(String, String)> = vec![("X-Content-Type-Options".into(), "nosniff".into())];
    match opts.isolation {
        Isolation::Off => {}
        Isolation::RequireCorp => {
            headers.push(("Cross-Origin-Opener-Policy".into(), "same-origin".into()));
            headers.push(("Cross-Origin-Embedder-Policy".into(), "require-corp".into()));
            headers.push(("Cross-Origin-Resource-Policy".into(), "same-origin".into()));
        }
        Isolation::Credentialless => {
            headers.push(("Cross-Origin-Opener-Policy".into(), "same-origin".into()));
            headers.push(("Cross-Origin-Embedder-Policy".into(), "credentialless".into()));
            headers.push(("Cross-Origin-Resource-Policy".into(), "same-origin".into()));
        }
    }
    headers.extend(opts.extra_headers.iter().cloned());
    if method != "GET" && method != "HEAD" {
        respond(&mut stream, "405 Method Not Allowed", &headers, b"method not allowed", false);
        return;
    }
    log.lock().unwrap().push(format!("{method} {}", target.split(['?', '#']).next().unwrap_or("")));
    let Some(path) = resolve(root, target) else {
        respond(&mut stream, "403 Forbidden", &headers, b"forbidden", method == "HEAD");
        return;
    };
    match std::fs::read(&path) {
        Ok(bytes) => {
            headers.push(("Content-Type".into(), mime(path.to_str().unwrap_or("")).into()));
            if opts.no_cache {
                headers.push(("Cache-Control".into(), "no-cache".into()));
            }
            respond(&mut stream, "200 OK", &headers, &bytes, method == "HEAD");
        }
        Err(_) => respond(&mut stream, "404 Not Found", &headers, b"not found", method == "HEAD"),
    }
}

/// Serve `root` on `addr` (`127.0.0.1:0` picks a free port) until the [`Server`] is stopped or dropped.
pub fn serve(root: impl Into<PathBuf>, addr: &str, opts: ServeOptions) -> io::Result<Server> {
    let root: PathBuf = root.into();
    let listener = TcpListener::bind(addr)?;
    let local = listener.local_addr()?;
    let stop = Arc::new(AtomicBool::new(false));
    let log = Arc::new(Mutex::new(Vec::new()));
    let (s2, l2) = (stop.clone(), log.clone());
    let opts = Arc::new(opts);
    let root = Arc::new(root);
    let thread = std::thread::spawn(move || {
        for conn in listener.incoming() {
            if s2.load(Ordering::SeqCst) {
                break;
            }
            let Ok(stream) = conn else { continue };
            let (root, opts, log) = (root.clone(), opts.clone(), l2.clone());
            std::thread::spawn(move || handle(stream, &root, &opts, &log));
        }
    });
    Ok(Server { addr: local, stop, thread: Some(thread), log })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn get(addr: SocketAddr, path: &str, method: &str) -> (String, String) {
        let mut s = TcpStream::connect(addr).unwrap();
        s.write_all(format!("{method} {path} HTTP/1.1\r\nHost: x\r\n\r\n").as_bytes()).unwrap();
        let mut out = Vec::new();
        s.read_to_end(&mut out).unwrap();
        let text = String::from_utf8_lossy(&out).into_owned();
        let (head, body) = text.split_once("\r\n\r\n").unwrap_or((&text, ""));
        (head.to_owned(), body.to_owned())
    }

    #[test]
    fn it_serves_files_with_the_right_types_and_refuses_to_leave_the_folder() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("index.html"), "<p>hi</p>").unwrap();
        std::fs::write(dir.path().join("a.wasm"), [0, 0x61, 0x73, 0x6d]).unwrap();
        std::fs::create_dir(dir.path().join("sub")).unwrap();
        std::fs::write(dir.path().join("sub/x.js"), "export {}").unwrap();
        let secret = dir.path().parent().unwrap().join("outside-secret.txt");
        std::fs::write(&secret, "no").unwrap();
        let srv = serve(dir.path(), "127.0.0.1:0", ServeOptions::default()).unwrap();
        let (h, b) = get(srv.addr, "/", "GET");
        assert!(h.starts_with("HTTP/1.1 200") && h.contains("text/html") && b == "<p>hi</p>");
        let (h, _) = get(srv.addr, "/a.wasm", "GET");
        assert!(h.contains("Content-Type: application/wasm"), "{h}");
        let (h, b) = get(srv.addr, "/a.wasm", "HEAD");
        assert!(h.contains("Content-Length: 4") && b.is_empty());
        for bad in ["/../outside-secret.txt", "/%2e%2e/outside-secret.txt", "/sub/../../outside-secret.txt", "/%5c..%5cx"] {
            let (h, b) = get(srv.addr, bad, "GET");
            assert!(h.starts_with("HTTP/1.1 403") || h.starts_with("HTTP/1.1 404"), "{bad}: {h}");
            assert!(!b.contains("no\n") && b != "no", "{bad} leaked");
        }
        let (h, _) = get(srv.addr, "/missing.js", "GET");
        assert!(h.starts_with("HTTP/1.1 404"));
        let (h, _) = get(srv.addr, "/", "POST");
        assert!(h.starts_with("HTTP/1.1 405"));
        assert_eq!(srv.requests()[0], "GET /");
        let _ = std::fs::remove_file(secret);
    }

    #[test]
    fn isolation_headers_are_added_on_request() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("index.html"), "x").unwrap();
        let plain = serve(dir.path(), "127.0.0.1:0", ServeOptions::default()).unwrap();
        assert!(!get(plain.addr, "/", "GET").0.contains("Cross-Origin-Opener-Policy"));
        let iso = serve(dir.path(), "127.0.0.1:0", ServeOptions { isolation: Isolation::RequireCorp, ..Default::default() }).unwrap();
        let h = get(iso.addr, "/", "GET").0;
        assert!(h.contains("Cross-Origin-Opener-Policy: same-origin") && h.contains("Cross-Origin-Embedder-Policy: require-corp"), "{h}");
    }

    #[test]
    fn resolve_maps_the_root_to_index_and_decodes_percent_escapes() {
        let root = Path::new("/srv/app");
        assert_eq!(resolve(root, "/").unwrap(), Path::new("/srv/app/index.html"));
        assert_eq!(resolve(root, "/forms/a%20b.cfrm?x=1").unwrap(), Path::new("/srv/app/forms/a b.cfrm"));
        assert!(resolve(root, "/a/../../b").is_none());
        assert!(resolve(root, "/%zz").is_none());
    }
}
