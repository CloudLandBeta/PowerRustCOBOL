// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! The Backend inspector's proxy (operator, 2026-11-08).
//!
//! During a Preview the page does not call the development Backend directly: it calls
//! this proxy, on a loopback port of its own, and the proxy calls the Backend. Between the
//! two it can
//!
//! * **record** every message: the HTTP requests and answers (`/auth/*`, `/health`) with
//!   their headers and body, the WebSocket handshakes with their headers, and every text frame
//!   (the JSON packets of the page's calls and the Backend's replies);
//! * **hold** the frames of one direction until the developer decides: forward as they are,
//!   forward **edited**, or drop them — before the Backend sees a request, or before the
//!   page sees a reply.
//!
//! Development only: the proxy listens on `127.0.0.1`, forwards to a loopback `ws://` Backend
//! and never touches a deployed one. HTTP exchanges are recorded, not held (they carry no
//! application data). A held frame blocks the connection it belongs to, as a debugger's
//! breakpoint does.

use std::collections::VecDeque;
use std::io::{ErrorKind, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::time::{Duration, Instant};

use tungstenite::client::IntoClientRequest;
use tungstenite::protocol::Message;
use tungstenite::{Error as WsError, WebSocket};

/// The most entries kept; the oldest go first.
pub const MAX_ENTRIES: usize = 1000;
/// The longest head or HTTP body the proxy reads.
const MAX_HEAD: usize = 64 * 1024;
const MAX_BODY: usize = 8 * 1024 * 1024;
/// How long a read waits before the loop looks at the other side.
const TICK: Duration = Duration::from_millis(15);

/// Which way a message travels.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    /// From the page to the Backend.
    ToBackend,
    /// From the Backend to the page.
    ToPage,
}

/// What a recorded entry is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// An HTTP request or its answer.
    Http,
    /// The WebSocket handshake (request headers, or the answer).
    Handshake,
    /// A text frame: JSON in this protocol.
    Text,
    /// A binary frame (body shown as its size).
    Binary,
    /// The connection closed.
    Close,
}

/// What happened to the entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum State {
    /// Forwarded as it came.
    Passed,
    /// Waiting for the developer.
    Held,
    /// Forwarded after the developer changed it.
    Edited,
    /// Never forwarded.
    Dropped,
}

/// One recorded message.
#[derive(Debug, Clone)]
pub struct Entry {
    pub id: u64,
    /// The connection it belongs to (1, 2, …).
    pub conn: u64,
    /// Milliseconds since the inspector was made.
    pub at_ms: u128,
    pub dir: Direction,
    pub kind: Kind,
    /// The request line or status line; empty for a frame.
    pub line: String,
    pub headers: Vec<(String, String)>,
    pub body: String,
    pub state: State,
    /// The body before the developer's edit.
    pub original: Option<String>,
}

/// One connection the page opened.
#[derive(Debug, Clone)]
pub struct Connection {
    pub conn: u64,
    pub path: String,
    pub websocket: bool,
    pub open: bool,
    pub at_ms: u128,
    pub frames: u64,
    pub bytes: u64,
}

/// What the developer decided for a held frame.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Decision {
    /// Forward this text (edited or not).
    Forward(String),
    /// Forward nothing.
    Drop,
}

#[derive(Default)]
struct Inner {
    entries: VecDeque<Entry>,
    connections: Vec<Connection>,
    decisions: Vec<(u64, Decision)>,
}

/// What the proxy records and what the developer asked it to hold; shared between the proxy's
/// threads and the IDE's window.
pub struct Inspector {
    inner: Mutex<Inner>,
    changed: Condvar,
    hold_requests: AtomicBool,
    hold_replies: AtomicBool,
    next_id: AtomicU64,
    next_conn: AtomicU64,
    started: Instant,
}

impl Default for Inspector {
    fn default() -> Self {
        Self {
            inner: Mutex::default(),
            changed: Condvar::new(),
            hold_requests: AtomicBool::new(false),
            hold_replies: AtomicBool::new(false),
            next_id: AtomicU64::new(1),
            next_conn: AtomicU64::new(1),
            started: Instant::now(),
        }
    }
}

impl Inspector {
    pub fn new() -> Arc<Self> {
        Arc::new(Self::default())
    }

    /// Hold the page's requests before the Backend sees them.
    pub fn set_hold_requests(&self, on: bool) {
        self.hold_requests.store(on, Ordering::SeqCst);
        if !on {
            self.release(Direction::ToBackend);
        }
    }

    /// Hold the Backend's replies before the page sees them.
    pub fn set_hold_replies(&self, on: bool) {
        self.hold_replies.store(on, Ordering::SeqCst);
        if !on {
            self.release(Direction::ToPage);
        }
    }

    pub fn holds_requests(&self) -> bool {
        self.hold_requests.load(Ordering::SeqCst)
    }

    pub fn holds_replies(&self) -> bool {
        self.hold_replies.load(Ordering::SeqCst)
    }

    /// Turning a hold off lets what it holds go, unchanged.
    fn release(&self, dir: Direction) {
        let held: Vec<(u64, String)> = self.lock().entries.iter().filter(|e| e.state == State::Held && e.dir == dir).map(|e| (e.id, e.body.clone())).collect();
        for (id, body) in held {
            self.decide(id, Decision::Forward(body));
        }
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Inner> {
        self.inner.lock().unwrap_or_else(|p| p.into_inner())
    }

    fn now_ms(&self) -> u128 {
        self.started.elapsed().as_millis()
    }

    fn push(&self, mut entry: Entry) -> u64 {
        let mut inner = self.lock();
        entry.id = self.next_id.fetch_add(1, Ordering::SeqCst);
        entry.at_ms = self.now_ms();
        let id = entry.id;
        if let Some(c) = inner.connections.iter_mut().find(|c| c.conn == entry.conn) {
            if matches!(entry.kind, Kind::Text | Kind::Binary) {
                c.frames += 1;
                c.bytes += entry.body.len() as u64;
            }
        }
        inner.entries.push_back(entry);
        while inner.entries.len() > MAX_ENTRIES {
            inner.entries.pop_front();
        }
        id
    }

    fn open_connection(&self, path: &str, websocket: bool) -> u64 {
        let conn = self.next_conn.fetch_add(1, Ordering::SeqCst);
        let at = self.now_ms();
        let mut inner = self.lock();
        inner.connections.push(Connection { conn, path: path.to_owned(), websocket, open: true, at_ms: at, frames: 0, bytes: 0 });
        if inner.connections.len() > 200 {
            inner.connections.remove(0);
        }
        conn
    }

    fn close_connection(&self, conn: u64) {
        if let Some(c) = self.lock().connections.iter_mut().find(|c| c.conn == conn) {
            c.open = false;
        }
    }

    /// The developer's answer for a held entry.
    pub fn decide(&self, id: u64, decision: Decision) {
        self.lock().decisions.push((id, decision));
        self.changed.notify_all();
    }

    /// Wait for the decision on `id`, or `None` when `stop` is raised first.
    fn wait_decision(&self, id: u64, stop: &AtomicBool) -> Option<Decision> {
        let mut inner = self.lock();
        loop {
            if let Some(at) = inner.decisions.iter().position(|(i, _)| *i == id) {
                return Some(inner.decisions.remove(at).1);
            }
            if stop.load(Ordering::SeqCst) {
                return None;
            }
            inner = self.changed.wait_timeout(inner, Duration::from_millis(100)).unwrap_or_else(|p| p.into_inner()).0;
        }
    }

    fn settle(&self, id: u64, state: State, body: Option<String>) {
        let mut inner = self.lock();
        if let Some(e) = inner.entries.iter_mut().find(|e| e.id == id) {
            e.state = state;
            if let Some(b) = body {
                if b != e.body {
                    e.original = Some(std::mem::replace(&mut e.body, b));
                }
            }
        }
    }

    /// A copy of what was recorded, oldest first.
    pub fn entries(&self) -> Vec<Entry> {
        self.lock().entries.iter().cloned().collect()
    }

    pub fn connections(&self) -> Vec<Connection> {
        self.lock().connections.clone()
    }

    /// The entries held now.
    pub fn held(&self) -> usize {
        self.lock().entries.iter().filter(|e| e.state == State::Held).count()
    }

    pub fn clear(&self) {
        let mut inner = self.lock();
        inner.entries.retain(|e| e.state == State::Held);
    }
}

/// The proxy: a loopback listener that forwards to one Backend.
pub struct Proxy {
    port: u16,
    path: String,
    target: String,
    stop: Arc<AtomicBool>,
}

impl Proxy {
    /// Listens on a free loopback port and forwards to `target` (`ws://127.0.0.1:PORT/ws`).
    /// `None` when `target` is not a loopback `ws://` address or no port could be taken.
    pub fn start(inspector: Arc<Inspector>, target: &str) -> Option<Proxy> {
        let (host, path) = split_ws(target)?;
        let listener = TcpListener::bind(("127.0.0.1", 0)).ok()?;
        listener.set_nonblocking(true).ok()?;
        let port = listener.local_addr().ok()?.port();
        let stop = Arc::new(AtomicBool::new(false));
        let flag = stop.clone();
        let upstream = host.clone();
        std::thread::spawn(move || {
            while !flag.load(Ordering::SeqCst) {
                match listener.accept() {
                    Ok((stream, _)) => {
                        let (insp, up, flag) = (inspector.clone(), upstream.clone(), flag.clone());
                        std::thread::spawn(move || {
                            let _ = serve(stream, &up, &insp, &flag);
                        });
                    }
                    Err(e) if e.kind() == ErrorKind::WouldBlock => std::thread::sleep(Duration::from_millis(20)),
                    Err(_) => break,
                }
            }
        });
        Some(Proxy { port, path, target: target.to_owned(), stop })
    }

    /// What the page connects to: the Backend's own path, on the proxy's port.
    pub fn url(&self) -> String {
        format!("ws://127.0.0.1:{}{}", self.port, self.path)
    }

    pub fn port(&self) -> u16 {
        self.port
    }

    /// The Backend address it forwards to.
    pub fn target(&self) -> &str {
        &self.target
    }
}

impl Drop for Proxy {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
    }
}

/// `ws://127.0.0.1:8443/ws` → (`127.0.0.1:8443`, `/ws`), for loopback addresses only.
fn split_ws(url: &str) -> Option<(String, String)> {
    let lower = url.to_ascii_lowercase();
    if !["ws://127.0.0.1:", "ws://localhost:", "ws://[::1]:"].iter().any(|p| lower.starts_with(p)) {
        return None;
    }
    let rest = &url["ws://".len()..];
    let at = rest.find(['/', '?', '#']).unwrap_or(rest.len());
    let path = if at == rest.len() { "/".to_owned() } else { rest[at..].to_owned() };
    Some((rest[..at].to_owned(), path))
}

/// A request head as read off the wire.
struct Head {
    line: String,
    headers: Vec<(String, String)>,
    /// Bytes of the head, including the blank line.
    len: usize,
}

impl Head {
    fn header(&self, name: &str) -> Option<&str> {
        self.headers.iter().find(|(k, _)| k.eq_ignore_ascii_case(name)).map(|(_, v)| v.as_str())
    }
}

fn parse_head(bytes: &[u8]) -> Option<Head> {
    let end = bytes.windows(4).position(|w| w == b"\r\n\r\n")? + 4;
    let text = String::from_utf8_lossy(&bytes[..end]).into_owned();
    let mut lines = text.split("\r\n");
    let line = lines.next()?.to_owned();
    let headers = lines.filter(|l| !l.is_empty()).filter_map(|l| l.split_once(':')).map(|(k, v)| (k.trim().to_owned(), v.trim().to_owned())).collect();
    Some(Head { line, headers, len: end })
}

/// Waits for the whole head with `peek`, so a WebSocket handshake can still be handed to the library untouched.
fn peek_head(stream: &TcpStream) -> std::io::Result<Head> {
    stream.set_read_timeout(Some(Duration::from_millis(50)))?;
    let mut buf = vec![0u8; MAX_HEAD];
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        match stream.peek(&mut buf) {
            Ok(0) => return Err(ErrorKind::UnexpectedEof.into()),
            Ok(n) => {
                if let Some(head) = parse_head(&buf[..n]) {
                    return Ok(head);
                }
                if n >= MAX_HEAD {
                    return Err(ErrorKind::InvalidData.into());
                }
                std::thread::sleep(Duration::from_millis(5));
            }
            Err(e) if matches!(e.kind(), ErrorKind::WouldBlock | ErrorKind::TimedOut) => {}
            Err(e) => return Err(e),
        }
        if Instant::now() > deadline {
            return Err(ErrorKind::TimedOut.into());
        }
    }
}

fn serve(stream: TcpStream, upstream: &str, insp: &Arc<Inspector>, stop: &Arc<AtomicBool>) -> std::io::Result<()> {
    stream.set_nodelay(true).ok();
    let head = peek_head(&stream)?;
    let websocket = head.header("upgrade").is_some_and(|u| u.eq_ignore_ascii_case("websocket"));
    let target = head.line.split(' ').nth(1).unwrap_or("/").to_owned();
    if websocket {
        serve_websocket(stream, upstream, &head, &target, insp, stop)
    } else {
        serve_http(stream, upstream, head, &target, insp)
    }
}

// ── HTTP ─────────────────────────────────────────────────────────────────────

fn serve_http(mut stream: TcpStream, upstream: &str, head: Head, target: &str, insp: &Arc<Inspector>) -> std::io::Result<()> {
    let conn = insp.open_connection(target, false);
    let result = http_exchange(&mut stream, upstream, head, conn, insp);
    insp.close_connection(conn);
    result
}

fn http_exchange(stream: &mut TcpStream, upstream: &str, head: Head, conn: u64, insp: &Arc<Inspector>) -> std::io::Result<()> {
    // Consume what was peeked: the head, then the body.
    let mut raw = vec![0u8; head.len];
    stream.read_exact(&mut raw)?;
    let wanted: usize = head.header("content-length").and_then(|v| v.parse().ok()).unwrap_or(0).min(MAX_BODY);
    let mut body = vec![0u8; wanted];
    stream.read_exact(&mut body)?;
    insp.push(Entry {
        id: 0,
        conn,
        at_ms: 0,
        dir: Direction::ToBackend,
        kind: Kind::Http,
        line: head.line.clone(),
        headers: head.headers.clone(),
        body: String::from_utf8_lossy(&body).into_owned(),
        state: State::Passed,
        original: None,
    });
    let mut out = TcpStream::connect(upstream)?;
    out.set_read_timeout(Some(Duration::from_secs(30)))?;
    // One exchange per connection: the answer ends where the Backend closes.
    let mut request = format!("{}\r\n", head.line);
    for (k, v) in head.headers.iter().filter(|(k, _)| !k.eq_ignore_ascii_case("connection")) {
        request.push_str(&format!("{k}: {v}\r\n"));
    }
    request.push_str("Connection: close\r\n\r\n");
    out.write_all(request.as_bytes())?;
    out.write_all(&body)?;
    let mut answer = Vec::new();
    let mut chunk = [0u8; 16 * 1024];
    loop {
        match out.read(&mut chunk) {
            Ok(0) => break,
            Ok(n) => {
                answer.extend_from_slice(&chunk[..n]);
                if answer.len() > MAX_BODY {
                    break;
                }
            }
            Err(e) if matches!(e.kind(), ErrorKind::WouldBlock | ErrorKind::TimedOut) => break,
            Err(e) => return Err(e),
        }
    }
    if let Some(reply) = parse_head(&answer) {
        insp.push(Entry {
            id: 0,
            conn,
            at_ms: 0,
            dir: Direction::ToPage,
            kind: Kind::Http,
            line: reply.line,
            headers: reply.headers,
            body: String::from_utf8_lossy(&answer[reply.len..]).into_owned(),
            state: State::Passed,
            original: None,
        });
    }
    stream.write_all(&answer)?;
    stream.flush()
}

// ── WebSocket ────────────────────────────────────────────────────────────────

type Socket = WebSocket<TcpStream>;

fn serve_websocket(stream: TcpStream, upstream: &str, head: &Head, target: &str, insp: &Arc<Inspector>, stop: &Arc<AtomicBool>) -> std::io::Result<()> {
    let conn = insp.open_connection(target, true);
    let result = websocket_session(stream, upstream, head, target, conn, insp, stop);
    insp.close_connection(conn);
    result
}

fn other<E: std::fmt::Display>(e: E) -> std::io::Error {
    std::io::Error::other(e.to_string())
}

fn websocket_session(stream: TcpStream, upstream: &str, head: &Head, target: &str, conn: u64, insp: &Arc<Inspector>, stop: &Arc<AtomicBool>) -> std::io::Result<()> {
    insp.push(Entry {
        id: 0,
        conn,
        at_ms: 0,
        dir: Direction::ToBackend,
        kind: Kind::Handshake,
        line: head.line.clone(),
        headers: head.headers.clone(),
        body: String::new(),
        state: State::Passed,
        original: None,
    });
    // The Backend first, with the page's own Origin (it checks it), so a refusal reaches the page as one.
    let mut request = format!("ws://{upstream}{target}").into_client_request().map_err(other)?;
    for name in ["origin", "sec-websocket-protocol", "cookie", "user-agent"] {
        if let Some(v) = head.header(name) {
            if let (Ok(n), Ok(v)) = (tungstenite::http::HeaderName::from_bytes(name.as_bytes()), tungstenite::http::HeaderValue::from_str(v)) {
                request.headers_mut().insert(n, v);
            }
        }
    }
    let out = TcpStream::connect(upstream)?;
    out.set_nodelay(true).ok();
    let (mut back, response) = tungstenite::client::client(request, out).map_err(other)?;
    let answer_headers: Vec<(String, String)> = response.headers().iter().map(|(k, v)| (k.to_string(), v.to_str().unwrap_or("").to_owned())).collect();
    insp.push(Entry {
        id: 0,
        conn,
        at_ms: 0,
        dir: Direction::ToPage,
        kind: Kind::Handshake,
        line: format!("HTTP/1.1 {}", response.status()),
        headers: answer_headers,
        body: String::new(),
        state: State::Passed,
        original: None,
    });
    let mut page: Socket = tungstenite::accept(stream).map_err(other)?;
    page.get_ref().set_read_timeout(Some(TICK))?;
    back.get_ref().set_read_timeout(Some(TICK))?;
    let result = pump(&mut page, &mut back, conn, insp, stop);
    insp.push(Entry { id: 0, conn, at_ms: 0, dir: Direction::ToPage, kind: Kind::Close, line: "closed".into(), headers: Vec::new(), body: String::new(), state: State::Passed, original: None });
    let _ = page.close(None);
    let _ = back.close(None);
    result
}

/// What one read gave.
enum Step {
    Frame(Message),
    Idle,
    Ended,
}

fn read_step(socket: &mut Socket) -> Step {
    match socket.read() {
        Ok(m) => Step::Frame(m),
        Err(WsError::Io(e)) if matches!(e.kind(), ErrorKind::WouldBlock | ErrorKind::TimedOut) => Step::Idle,
        Err(_) => Step::Ended,
    }
}

fn pump(page: &mut Socket, back: &mut Socket, conn: u64, insp: &Arc<Inspector>, stop: &Arc<AtomicBool>) -> std::io::Result<()> {
    while !stop.load(Ordering::SeqCst) {
        let mut idle = true;
        for dir in [Direction::ToBackend, Direction::ToPage] {
            let (from, to) = if dir == Direction::ToBackend { (&mut *page, &mut *back) } else { (&mut *back, &mut *page) };
            match read_step(from) {
                Step::Idle => {}
                Step::Ended => return Ok(()),
                Step::Frame(message) => {
                    idle = false;
                    if !relay(message, dir, to, conn, insp, stop) {
                        return Ok(());
                    }
                }
            }
            // Pongs the library queued while reading.
            let _ = from.flush();
        }
        if idle {
            std::thread::sleep(Duration::from_millis(1));
        }
    }
    Ok(())
}

/// Record, maybe hold, and forward one message. `false` = the connection is over.
fn relay(message: Message, dir: Direction, to: &mut Socket, conn: u64, insp: &Arc<Inspector>, stop: &Arc<AtomicBool>) -> bool {
    let entry = |kind, body: String, state| Entry { id: 0, conn, at_ms: 0, dir, kind, line: String::new(), headers: Vec::new(), body, state, original: None };
    match message {
        Message::Text(text) => {
            let text = text.as_str().to_owned();
            let hold = match dir {
                Direction::ToBackend => insp.holds_requests(),
                Direction::ToPage => insp.holds_replies(),
            };
            let id = insp.push(entry(Kind::Text, text.clone(), if hold { State::Held } else { State::Passed }));
            let send = if hold {
                match insp.wait_decision(id, stop) {
                    Some(Decision::Forward(edited)) => {
                        insp.settle(id, if edited == text { State::Passed } else { State::Edited }, Some(edited.clone()));
                        Some(edited)
                    }
                    Some(Decision::Drop) => {
                        insp.settle(id, State::Dropped, None);
                        None
                    }
                    None => return false,
                }
            } else {
                Some(text)
            };
            match send {
                Some(t) => to.send(Message::text(t)).is_ok(),
                None => true,
            }
        }
        Message::Binary(bytes) => {
            insp.push(entry(Kind::Binary, format!("{} bytes", bytes.len()), State::Passed));
            to.send(Message::Binary(bytes)).is_ok()
        }
        Message::Close(frame) => {
            let _ = to.send(Message::Close(frame));
            false
        }
        // Ping and pong are answered by the library on each side.
        _ => true,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A Backend that answers each text frame with `{"echo":<frame>}`, and an HTTP `GET /health`.
    fn fake_backend() -> (u16, Arc<AtomicBool>) {
        let listener = TcpListener::bind(("127.0.0.1", 0)).unwrap();
        let port = listener.local_addr().unwrap().port();
        let stop = Arc::new(AtomicBool::new(false));
        std::thread::spawn(move || {
            for stream in listener.incoming().flatten() {
                std::thread::spawn(move || {
                    let mut peek = [0u8; 16];
                    let n = stream.peek(&mut peek).unwrap_or(0);
                    if peek[..n].starts_with(b"GET /health") {
                        let mut s = stream;
                        let mut buf = [0u8; 2048];
                        let _ = s.read(&mut buf);
                        let _ = s.write_all(b"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: 11\r\nConnection: close\r\n\r\n{\"ok\":true}\n");
                        return;
                    }
                    let mut ws = tungstenite::accept(stream).unwrap();
                    while let Ok(Message::Text(t)) = ws.read() {
                        ws.send(Message::text(format!("{{\"echo\":{}}}", t.as_str()))).unwrap();
                    }
                });
            }
        });
        (port, stop)
    }

    fn client(port: u16) -> WebSocket<TcpStream> {
        let stream = TcpStream::connect(("127.0.0.1", port)).unwrap();
        let (ws, _) = tungstenite::client::client(format!("ws://127.0.0.1:{port}/ws"), stream).unwrap();
        ws
    }

    fn wait<T>(what: &str, mut f: impl FnMut() -> Option<T>) -> T {
        let end = Instant::now() + Duration::from_secs(10);
        loop {
            if let Some(v) = f() {
                return v;
            }
            assert!(Instant::now() < end, "timed out waiting for {what}");
            std::thread::sleep(Duration::from_millis(10));
        }
    }

    #[test]
    fn only_a_loopback_ws_address_is_proxied() {
        assert_eq!(split_ws("ws://127.0.0.1:8443/ws"), Some(("127.0.0.1:8443".into(), "/ws".into())));
        assert_eq!(split_ws("ws://localhost:9/"), Some(("localhost:9".into(), "/".into())));
        assert_eq!(split_ws("wss://127.0.0.1:8443/ws"), None);
        assert_eq!(split_ws("ws://203.0.113.9:8443/ws"), None);
    }

    #[test]
    fn frames_are_recorded_held_edited_and_dropped() {
        let (backend, _stop) = fake_backend();
        let insp = Inspector::new();
        let proxy = Proxy::start(insp.clone(), &format!("ws://127.0.0.1:{backend}/ws")).expect("a proxy");
        assert!(proxy.url().starts_with("ws://127.0.0.1:") && proxy.url().ends_with("/ws") && proxy.port() != backend);
        let mut page = client(proxy.port());

        // Passing through: recorded in both directions.
        page.send(Message::text("{\"t\":\"call\",\"id\":1}")).unwrap();
        let Message::Text(reply) = page.read().unwrap() else { panic!("a text reply") };
        assert_eq!(reply.as_str(), "{\"echo\":{\"t\":\"call\",\"id\":1}}");
        let entries = insp.entries();
        let texts: Vec<_> = entries.iter().filter(|e| e.kind == Kind::Text).map(|e| (e.dir, e.body.clone(), e.state)).collect();
        assert_eq!(texts.len(), 2, "{texts:?}");
        assert_eq!(texts[0], (Direction::ToBackend, "{\"t\":\"call\",\"id\":1}".into(), State::Passed));
        assert_eq!(texts[1].0, Direction::ToPage);
        // The handshakes carry their headers.
        assert!(entries.iter().any(|e| e.kind == Kind::Handshake && e.dir == Direction::ToBackend && e.headers.iter().any(|(k, _)| k.eq_ignore_ascii_case("sec-websocket-key"))));
        assert!(insp.connections().iter().any(|c| c.websocket && c.open && c.frames == 2));

        // Hold requests: the Backend does not see it until the developer edits and forwards it.
        insp.set_hold_requests(true);
        page.send(Message::text("\"original\"")).unwrap();
        let held = wait("the request to be held", || insp.entries().into_iter().find(|e| e.state == State::Held));
        assert_eq!((held.dir, held.body.as_str()), (Direction::ToBackend, "\"original\""));
        insp.decide(held.id, Decision::Forward("\"edited\"".into()));
        let Message::Text(reply) = page.read().unwrap() else { panic!("a text reply") };
        assert_eq!(reply.as_str(), "{\"echo\":\"edited\"}", "the Backend saw the edited packet");
        let edited = insp.entries().into_iter().find(|e| e.id == held.id).unwrap();
        assert_eq!((edited.state, edited.original.as_deref()), (State::Edited, Some("\"original\"")));
        insp.set_hold_requests(false);

        // Hold replies: the page gets the changed answer.
        insp.set_hold_replies(true);
        page.send(Message::text("1")).unwrap();
        let held = wait("the reply to be held", || insp.entries().into_iter().find(|e| e.state == State::Held && e.dir == Direction::ToPage));
        assert_eq!(held.body, "{\"echo\":1}");
        insp.decide(held.id, Decision::Forward("{\"echo\":\"changed on the way back\"}".into()));
        let Message::Text(reply) = page.read().unwrap() else { panic!("a text reply") };
        assert_eq!(reply.as_str(), "{\"echo\":\"changed on the way back\"}");
        insp.set_hold_replies(false);

        // Drop: nothing reaches the Backend, the next frame still goes through.
        insp.set_hold_requests(true);
        page.send(Message::text("\"never\"")).unwrap();
        let held = wait("the request to be held", || insp.entries().into_iter().find(|e| e.state == State::Held));
        insp.decide(held.id, Decision::Drop);
        wait("the drop", || insp.entries().iter().any(|e| e.id == held.id && e.state == State::Dropped).then_some(()));
        insp.set_hold_requests(false);
        page.send(Message::text("2")).unwrap();
        let Message::Text(reply) = page.read().unwrap() else { panic!("a text reply") };
        assert_eq!(reply.as_str(), "{\"echo\":2}");
        println!("recorded {} entries on {} connection(s); hold, edit and drop both ways", insp.entries().len(), insp.connections().len());
    }

    #[test]
    fn an_http_exchange_is_forwarded_and_recorded() {
        let (backend, _stop) = fake_backend();
        let insp = Inspector::new();
        let proxy = Proxy::start(insp.clone(), &format!("ws://127.0.0.1:{backend}/ws")).unwrap();
        let mut s = TcpStream::connect(("127.0.0.1", proxy.port())).unwrap();
        s.write_all(b"GET /health HTTP/1.1\r\nHost: x\r\nX-Probe: 1\r\n\r\n").unwrap();
        let mut got = String::new();
        s.read_to_string(&mut got).unwrap();
        assert!(got.starts_with("HTTP/1.1 200") && got.contains("{\"ok\":true}"), "{got}");
        let entries = wait("the exchange to be recorded", || {
            let e = insp.entries();
            (e.len() == 2).then_some(e)
        });
        assert_eq!(entries[0].line, "GET /health HTTP/1.1");
        assert!(entries[0].headers.iter().any(|(k, v)| k == "X-Probe" && v == "1"));
        assert_eq!(entries[1].dir, Direction::ToPage);
        assert!(entries[1].line.starts_with("HTTP/1.1 200") && entries[1].body.contains("\"ok\""));
    }
}
