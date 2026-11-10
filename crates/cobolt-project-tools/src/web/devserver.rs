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
//!   (`/__prc/version`) and serves the few lines of script that ask for the count
//!   and reload the page. The script is added to `index.html` as it is sent;
//!   the file on disk is never touched. While a build writes the folder
//!   ([`ServerHandle::build_started`]) the watcher counts nothing: the build's
//!   end ([`ServerHandle::notify_changed`]) is the one change the page sees, so
//!   the page never reads a bundle half written (WASM-008 T47). The page asks
//!   by **long poll** (`/__prc/version?since=<rev>&wait=<ms>`): the server holds
//!   the request until something the page must hear of changes (the count, or a
//!   failed build's errors) or `wait` runs out, so an idle page makes one request
//!   in 25 s and hears a change at once; without `since` the answer is immediate.
//!
//! - **page events** (WASM-008 R13, D13): `POST /__prc/log` is the one route
//!   that is not `GET`/`HEAD`. The served page reports its console lines,
//!   uncaught errors, COBOL run-time errors and in-place update times there;
//!   the server checks the `Host` and the `Origin`, the size, the type, a
//!   closed set of events and a rate, parses them into [`PageEvent`]s and
//!   queues them for [`ServerHandle::take_page_events`]. Another local process
//!   could still post — it can only add lines to the Output panel.
//!
//! One connection, one request, one thread; `Connection: close`. A Preview
//! serves one developer.

use std::collections::hash_map::DefaultHasher;
use std::collections::VecDeque;
use std::hash::{Hash, Hasher};
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{Shutdown, SocketAddr, TcpListener, TcpStream};
use std::path::{Component, Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use cobolt_compiler::project_kind::Isolation;
use serde::{Deserialize, Serialize};

/// The path the page polls for the change count.
pub const VERSION_PATH: &str = "/__prc/version";
/// The path of the live-reload script the server adds to `index.html`.
pub const RELOAD_SCRIPT_PATH: &str = "/__prc/livereload.js";
/// The path the page posts its events to (WASM-008 R13, D13).
pub const LOG_PATH: &str = "/__prc/log";
/// The page that shows the application at a phone's width, in a frame, when
/// no device or emulator can run it (WASM-008 R15, D12).
pub const PHONE_PATH: &str = "/__prc/phone";
/// The largest request head the server reads (request line and headers).
const MAX_HEAD: usize = 16 * 1024;
/// The largest body `POST /__prc/log` takes.
pub const MAX_LOG_BODY: usize = 16 * 1024;
/// The most events one `POST /__prc/log` may carry.
pub const MAX_EVENTS_PER_POST: usize = 40;
/// The most events taken in one second; a burst over it is refused.
pub const MAX_EVENTS_PER_SECOND: u32 = 200;
/// The most events kept for the IDE between two polls.
const MAX_QUEUED_EVENTS: usize = 1000;
/// How often the watcher looks at the bundle folder.
pub const WATCH_INTERVAL: Duration = Duration::from_millis(150);
/// The longest `GET /__prc/version?since=…&wait=…` is held: a larger `wait` is cut to this. The page asks for 25 s,
/// which every browser and every proxy on this machine's loopback holds without giving up.
pub const LONG_POLL_MAX: Duration = Duration::from_secs(30);

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
    /// The revision of everything `/__prc/version` tells the page: it moves with `version` and with the build
    /// errors, so a held request wakes for either. Starts at 1; a page that has not asked yet asks `since=0`.
    rev: Mutex<u64>,
    /// Woken at every change of `rev` and at the stop, for the held requests.
    rev_changed: Condvar,
    /// The requests for `/__prc/version` that have arrived, held ones too (the others are counted when answered).
    version_requests: AtomicU64,
    /// The folder's fingerprint as last counted; the watcher and a build's end
    /// both compare against it, under its lock.
    watched: Mutex<u64>,
    /// A build is writing the folder: the watcher waits for its end.
    building: AtomicBool,
    /// When the last change was seen, for the report.
    last_change: Mutex<Option<Instant>>,
    stop: AtomicBool,
    port: u16,
    /// Requests answered, for the tests and the Output panel.
    served: AtomicU64,
    /// Page events accepted and not yet taken by the IDE.
    page_events: Mutex<VecDeque<PageEvent>>,
    /// `POST /__prc/log` requests refused, for the tests and the report.
    log_refused: AtomicU64,
    /// The rate window: when it started and how many events it took.
    log_window: Mutex<(Instant, u32)>,
    /// The last build's error lines; `/__prc/version` carries them while any.
    build_errors: Mutex<Vec<String>>,
    /// The texts the page shows, in the developer's language.
    page_strings: Mutex<PageStrings>,
    /// The development Backend of this run: its address and token, put into
    /// the served `config.json` only (WASM-008 R6, D4).
    dev_backend: Mutex<Option<DevBackend>>,
    /// The size `/__prc/phone` frames the application at; `None`: no such page.
    phone: Mutex<Option<PhoneFrame>>,
}

impl State {
    /// Something `/__prc/version` tells the page has changed: move the revision and wake the held requests. Called
    /// after the state they read has been written, so a request that wakes reads the new state.
    fn bump(&self) {
        if let Ok(mut rev) = self.rev.lock() {
            *rev += 1;
        }
        self.rev_changed.notify_all();
    }

    /// Count one change of the bundle, for the page to see.
    fn count_change(&self) {
        self.version.fetch_add(1, Ordering::SeqCst);
        if let Ok(mut t) = self.last_change.lock() {
            *t = Some(Instant::now());
        }
        self.bump();
    }

    /// Hold a long poll that has seen revision `since` until the revision moves, the server stops or `wait` (cut to
    /// [`LONG_POLL_MAX`]) is over; answers the revision it ends on. A page that is behind, or ahead (a server
    /// started again), is answered at once.
    fn hold_for_change(&self, since: u64, wait: Duration) -> u64 {
        let deadline = Instant::now() + wait.min(LONG_POLL_MAX);
        let Ok(mut rev) = self.rev.lock() else { return since };
        while *rev == since && !self.stop.load(Ordering::SeqCst) {
            let left = deadline.saturating_duration_since(Instant::now());
            if left.is_zero() {
                break;
            }
            rev = match self.rev_changed.wait_timeout(rev, left) {
                Ok((guard, _)) => guard,
                Err(_) => return since,
            };
        }
        *rev
    }

    /// The body of `/__prc/version`: the change count, the failed build's errors while there are any and, for a long
    /// poll, the revision the answer is for (read before the rest, so what follows is at least that new).
    fn version_body(&self, rev: Option<u64>) -> String {
        let version = self.version.load(Ordering::SeqCst);
        let errors = self.build_errors.lock().map(|e| e.clone()).unwrap_or_default();
        match (rev, errors.is_empty()) {
            (None, true) => format!("{{\"version\":{version}}}"),
            (None, false) => serde_json::json!({ "version": version, "errors": errors }).to_string(),
            (Some(rev), true) => serde_json::json!({ "version": version, "rev": rev }).to_string(),
            (Some(rev), false) => serde_json::json!({ "version": version, "errors": errors, "rev": rev }).to_string(),
        }
    }
}

/// The long-poll parameters of a request for `/__prc/version`: `since` (the revision the page has seen) and `wait`
/// (how long it may be held, in milliseconds; none is none). `Ok(None)` when the request has no `since`: the plain,
/// immediate answer. `Err` names the parameter that is not a whole number.
fn long_poll_query(path: &str) -> Result<Option<(u64, Duration)>, &'static str> {
    let query = path.split('#').next().unwrap_or("").split_once('?').map_or("", |(_, q)| q);
    let (mut since, mut wait) = (None, 0u64);
    for pair in query.split('&') {
        match pair.split_once('=') {
            Some(("since", v)) => since = Some(v.parse::<u64>().map_err(|_| "bad since")?),
            Some(("wait", v)) => wait = v.parse::<u64>().map_err(|_| "bad wait")?,
            _ => {}
        }
    }
    Ok(since.map(|s| (s, Duration::from_millis(wait).min(LONG_POLL_MAX))))
}

/// The CSS size of the phone `/__prc/phone` shows the application in (a
/// preset in the project's orientation).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PhoneFrame {
    pub width: u32,
    pub height: u32,
}

fn html_text(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

/// `/__prc/phone`: the not-the-device banner, then the application in a
/// same-origin frame of the phone's CSS size. It is served with the bundle's
/// isolation headers, so the frame is isolated as the page alone would be.
pub fn phone_page(frame: PhoneFrame, banner: &str) -> String {
    let (w, h) = (frame.width, frame.height);
    format!(
        "<!doctype html>\n<html><head><meta charset=\"utf-8\"><title>{t}</title>\n\
         <style>html,body{{margin:0;background:#202124;font:13px system-ui,sans-serif;color:#e8eaed}}\
         #pr-phone-banner{{padding:8px 12px;background:#3c4043;text-align:center}}\
         #pr-phone{{display:block;margin:16px auto;width:{w}px;height:{h}px;border:0;background:#fff;\
         border-radius:12px;box-shadow:0 0 0 10px #000}}</style></head>\n\
         <body><div id=\"pr-phone-banner\" role=\"status\">{t}</div>\n\
         <iframe id=\"pr-phone\" src=\"/\" width=\"{w}\" height=\"{h}\" title=\"{t}\"></iframe></body></html>\n",
        t = html_text(banner)
    )
}

/// The Backend a Preview runs against, for this run only (WASM-008 R6, D4).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DevBackend {
    /// `ws://127.0.0.1:<port>/ws`.
    pub url: String,
    /// The development token `rcrun backend run` printed.
    pub token: String,
}

/// `config.json` as this run serves it: the development Backend's address
/// among `backend.origins`, `backend.unsafe_plain_ws` on and the token in
/// `backend.dev_token`. Only a loopback `ws://` address is ever put there; the
/// file on disk is never changed. `None` when `config` is not a JSON object.
pub fn with_dev_backend(config: &[u8], dev: &DevBackend) -> Option<Vec<u8>> {
    let lower = dev.url.to_ascii_lowercase();
    let loopback = ["ws://127.0.0.1:", "ws://localhost:", "ws://[::1]:"].iter().any(|p| lower.starts_with(p));
    if !loopback {
        return None;
    }
    let mut v: serde_json::Value = serde_json::from_slice(config).ok()?;
    let root = v.as_object_mut()?;
    let backend = root.entry("backend").or_insert_with(|| serde_json::json!({}));
    let backend = backend.as_object_mut()?;
    let origin = {
        let rest = &dev.url["ws://".len()..];
        format!("ws://{}", rest.split(['/', '?', '#']).next().unwrap_or(""))
    };
    let origins = backend.entry("origins").or_insert_with(|| serde_json::json!([]));
    if let Some(list) = origins.as_array_mut() {
        if !list.iter().any(|o| o.as_str() == Some(origin.as_str()) || o.as_str() == Some(dev.url.as_str())) {
            list.push(serde_json::Value::String(dev.url.clone()));
        }
    }
    backend.insert("unsafe_plain_ws".into(), serde_json::Value::Bool(true));
    backend.insert("dev_token".into(), serde_json::Value::String(dev.token.clone()));
    serde_json::to_vec(&v).ok()
}

/// The texts the Preview script shows in the page (WASM-008 §2.2), handed to
/// the server by the IDE in the developer's language and inserted with
/// `textContent` only. The defaults are English, for a server no IDE drives.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PageStrings {
    /// The banner of a page whose Preview has stopped (three failed polls).
    pub stopped: String,
    /// The title of the overlay of a failed build.
    pub errors_title: String,
    /// The overlay's dismiss button.
    pub dismiss: String,
    /// The banner of the phone page: this is the browser, not the device.
    pub not_the_device: String,
}

impl Default for PageStrings {
    fn default() -> Self {
        Self {
            stopped: "The Preview has stopped. Start it again from the IDE.".to_owned(),
            errors_title: "The build failed; this is the last good version.".to_owned(),
            dismiss: "Dismiss".to_owned(),
            not_the_device: "This is the browser at a phone's width, not the device.".to_owned(),
        }
    }
}

/// What the served page reports (WASM-008 R13, R5, plan §3): the same JSON
/// over `POST /__prc/log` and from the headless helper. Every event carries
/// the `form` it came from (empty when the page could not tell). A field the
/// page leaves out reads as empty; an unknown field is ignored.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "event", rename_all = "kebab-case")]
pub enum PageEvent {
    /// A `console.*` line.
    Console {
        #[serde(default)]
        level: String,
        #[serde(default)]
        text: String,
        #[serde(default)]
        form: String,
    },
    /// An uncaught error or an unhandled promise rejection.
    Error {
        #[serde(default)]
        message: String,
        #[serde(default)]
        source: String,
        #[serde(default)]
        line: u32,
        #[serde(default)]
        form: String,
    },
    /// A COBOL run-time error the player recorded (`errors()`): the program
    /// unit (the handler) and its line.
    CobolError {
        #[serde(default)]
        form: String,
        #[serde(default)]
        unit: String,
        #[serde(default)]
        line: u32,
        #[serde(default)]
        code: String,
        #[serde(default)]
        message: String,
    },
    /// An in-place update finished: what `app.update` answered and how long it took.
    Updated {
        #[serde(default)]
        how: String,
        #[serde(default)]
        ms: f64,
        #[serde(default)]
        form: String,
    },
}

impl PageEvent {
    /// The event a JSON line carries, or `None` for one outside the set.
    pub fn parse(line: &str) -> Option<PageEvent> {
        serde_json::from_str(line.trim()).ok()
    }

    /// One line of JSON.
    pub fn to_line(&self) -> String {
        serde_json::to_string(self).expect("an event serialises")
    }
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

    /// A build starts writing the folder: until it ends ([`Self::notify_changed`]
    /// or [`Self::build_failed`]) the watcher counts nothing, so the page is
    /// never told of a bundle half written (WASM-008 T47: the generator swaps
    /// the whole folder, and a page that read it in mid-swap reloaded).
    pub fn build_started(&self) {
        self.state.building.store(true, Ordering::SeqCst);
    }

    /// Count a change now, without waiting for the watcher: a build that has
    /// just finished says so, and the page takes it at its next poll. This is
    /// the one change counted for the build — the watcher starts again from
    /// the folder as the build left it.
    pub fn notify_changed(&self) {
        self.settle_watch();
        self.state.count_change();
    }

    /// A build ended without a new bundle (it failed, or wrote nothing): no
    /// change is counted, and the watcher starts again from the folder as it is.
    pub fn build_failed(&self) {
        self.settle_watch();
    }

    fn settle_watch(&self) {
        if let Ok(mut w) = self.state.watched.lock() {
            *w = fingerprint(&self.state.root);
            self.state.building.store(false, Ordering::SeqCst);
        }
    }

    /// The CSS-only fast path (WASM-007 R11, T23; an amendment to WASM-008's
    /// protocol): write form `form`'s new stylesheet into the served bundle —
    /// the sheet file, then its `config.json` entry, replaced whole so the page
    /// never reads half of it — and count the change now, without a Build. The
    /// page's next poll hands the new configuration to `app.update`, which calls
    /// `set_form_stylesheet` only. A sheet the Build would refuse is not written:
    /// `Err` with its `file:line: message` lines, and the Build reports it.
    pub fn apply_form_stylesheet(&self, form: &str, css: &str, layers: &[String]) -> Result<(), Vec<String>> {
        self.build_started();
        let written = self.write_form_stylesheet(form, css, layers);
        match &written {
            Ok(()) => self.notify_changed(),
            Err(_) => self.build_failed(),
        }
        written
    }

    fn write_form_stylesheet(&self, form: &str, css: &str, layers: &[String]) -> Result<(), Vec<String>> {
        let root = &self.state.root;
        let config = std::fs::read(root.join("config.json")).map_err(|e| vec![format!("config.json: {e}")])?;
        let (sheet, next) = cobolt_web_bundle::with_form_stylesheet(&config, form, css, layers)?;
        let io = |e: std::io::Error| vec![format!("{}: {e}", root.display())];
        let path = root.join(&sheet.path);
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir).map_err(io)?;
        }
        std::fs::write(&path, &sheet.bytes).map_err(io)?;
        let tmp = root.join(".config.json.tmp");
        std::fs::write(&tmp, &next).map_err(io)?;
        std::fs::rename(&tmp, root.join("config.json")).map_err(io)
    }

    /// When the last change was seen.
    pub fn last_change(&self) -> Option<Instant> {
        self.state.last_change.lock().ok().and_then(|t| *t)
    }

    /// How many requests have been answered.
    pub fn requests_served(&self) -> u64 {
        self.state.served.load(Ordering::SeqCst)
    }

    /// How many requests for `/__prc/version` have arrived, the one being held now included (an idle page makes one
    /// in 25 s; [`Self::requests_served`] counts a request only when its answer is sent).
    pub fn version_requests(&self) -> u64 {
        self.state.version_requests.load(Ordering::SeqCst)
    }

    /// The page events posted since the last call, oldest first.
    pub fn take_page_events(&self) -> Vec<PageEvent> {
        self.state.page_events.lock().map(|mut q| q.drain(..).collect()).unwrap_or_default()
    }

    /// How many `POST /__prc/log` requests were refused.
    pub fn page_events_refused(&self) -> u64 {
        self.state.log_refused.load(Ordering::SeqCst)
    }

    /// The last build's error lines (WASM-008 R7): while there are any,
    /// `/__prc/version` carries them and the page draws them over itself; an
    /// empty list (the next good build) removes them. The bundle on disk is
    /// whatever the last good build left.
    pub fn set_build_errors(&self, errors: Vec<String>) {
        let changed = self.state.build_errors.lock().map(|mut e| std::mem::replace(&mut *e, errors.clone()) != errors).unwrap_or(false);
        // The page holds a long poll for the errors too: wake it, but only for errors that are not the ones it has.
        if changed {
            self.state.bump();
        }
    }

    /// The development Backend of this run (WASM-008 R6): the served
    /// `config.json` names it and carries its token; `None` serves the file as
    /// it is on disk.
    pub fn set_dev_backend(&self, dev: Option<DevBackend>) {
        if let Ok(mut d) = self.state.dev_backend.lock() {
            *d = dev;
        }
    }

    /// Serve `/__prc/phone` with the application framed at `frame` (`None`: not served).
    pub fn set_phone(&self, frame: Option<PhoneFrame>) {
        if let Ok(mut f) = self.state.phone.lock() {
            *f = frame;
        }
    }

    /// The address of the phone page.
    pub fn phone_url(&self) -> String {
        format!("{}{}", self.url().trim_end_matches('/'), PHONE_PATH)
    }

    /// The texts the page shows, in the developer's language.
    pub fn set_page_strings(&self, strings: PageStrings) {
        if let Ok(mut s) = self.state.page_strings.lock() {
            *s = strings;
        }
    }

    /// Stop the server and wait for its threads.
    pub fn stop(&mut self) {
        // Set under the lock a held request waits on, so none misses the wake-up and sleeps out its 25 s.
        let held = self.state.rev.lock();
        self.state.stop.store(true, Ordering::SeqCst);
        drop(held);
        self.state.rev_changed.notify_all();
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
        rev: Mutex::new(1),
        rev_changed: Condvar::new(),
        version_requests: AtomicU64::new(0),
        watched: Mutex::new(fingerprint(&root)),
        building: AtomicBool::new(false),
        last_change: Mutex::new(None),
        stop: AtomicBool::new(false),
        port,
        served: AtomicU64::new(0),
        page_events: Mutex::new(VecDeque::new()),
        log_refused: AtomicU64::new(0),
        log_window: Mutex::new((Instant::now(), 0)),
        build_errors: Mutex::new(Vec::new()),
        page_strings: Mutex::new(PageStrings::default()),
        dev_backend: Mutex::new(None),
        phone: Mutex::new(None),
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
    while !state.stop.load(Ordering::SeqCst) {
        thread::sleep(WATCH_INTERVAL);
        // A build writing the folder is counted once, at its end, by the build.
        if state.building.load(Ordering::SeqCst) {
            continue;
        }
        let Ok(mut last) = state.watched.lock() else { return };
        if state.building.load(Ordering::SeqCst) {
            continue;
        }
        let now = fingerprint(&state.root);
        if now != *last {
            *last = now;
            state.count_change();
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
        "txt" | "md" | "cfrm" | "wcfrm" | "mcfrm" | "cidx" | "yaml" | "yml" => "text/plain; charset=utf-8",
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
    origin: Option<String>,
    content_type: Option<String>,
    content_length: Option<usize>,
    /// Read only for `POST /__prc/log` within [`MAX_LOG_BODY`].
    body: Vec<u8>,
}

fn read_request(stream: &TcpStream) -> Result<Request, (u16, &'static str)> {
    stream.set_read_timeout(Some(Duration::from_secs(5))).ok();
    let mut reader = BufReader::new(stream.take((MAX_HEAD + MAX_LOG_BODY) as u64));
    let mut line = String::new();
    reader.read_line(&mut line).map_err(|_| (400, "bad request"))?;
    let mut parts = line.split_whitespace();
    let method = parts.next().ok_or((400, "bad request"))?.to_owned();
    let path = parts.next().ok_or((400, "bad request"))?.to_owned();
    if !parts.next().is_some_and(|v| v.starts_with("HTTP/1.")) {
        return Err((400, "bad request"));
    }
    let mut host = None;
    let mut origin = None;
    let mut content_type = None;
    let mut content_length = None;
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
            let value = value.trim();
            if name.eq_ignore_ascii_case("host") {
                host = Some(value.to_owned());
            } else if name.eq_ignore_ascii_case("origin") {
                origin = Some(value.to_owned());
            } else if name.eq_ignore_ascii_case("content-type") {
                content_type = Some(value.to_owned());
            } else if name.eq_ignore_ascii_case("content-length") {
                content_length = Some(value.parse().map_err(|_| (400, "bad request"))?);
            }
        }
    }
    let mut body = Vec::new();
    let bare = path.split(['?', '#']).next().unwrap_or("");
    if method == "POST" && bare == LOG_PATH {
        if let Some(n) = content_length.filter(|n| *n <= MAX_LOG_BODY) {
            body.resize(n, 0);
            reader.read_exact(&mut body).map_err(|_| (400, "bad request"))?;
        }
    }
    Ok(Request { method, path, host, origin, content_type, content_length, body })
}

/// Is `host` (a `Host` header) one of this server's own names?
fn host_is_ours(host: Option<&str>, port: u16) -> bool {
    let Some(host) = host else { return false };
    let host = host.to_ascii_lowercase();
    host == format!("127.0.0.1:{port}") || host == format!("localhost:{port}") || host == format!("[::1]:{port}")
}

/// Is `origin` (an `Origin` header) this server's own?
fn origin_is_ours(origin: Option<&str>, port: u16) -> bool {
    let Some(origin) = origin else { return false };
    let origin = origin.to_ascii_lowercase();
    origin == format!("http://127.0.0.1:{port}") || origin == format!("http://localhost:{port}")
}

/// Take a `POST /__prc/log` (Host already checked). `Err` is the status and the
/// reason of a refusal; nothing of a refused request is kept.
fn take_log(request: &Request, state: &State) -> Result<usize, (u16, &'static str, &'static str)> {
    if !origin_is_ours(request.origin.as_deref(), state.port) {
        return Err((403, "Forbidden", "foreign origin"));
    }
    match request.content_length {
        None => return Err((411, "Length Required", "no length")),
        Some(n) if n > MAX_LOG_BODY => return Err((413, "Payload Too Large", "too large")),
        Some(_) => {}
    }
    let is_json = request
        .content_type
        .as_deref()
        .and_then(|t| t.split(';').next())
        .is_some_and(|t| t.trim().eq_ignore_ascii_case("application/json"));
    if !is_json {
        return Err((415, "Unsupported Media Type", "not json"));
    }
    let items: Vec<serde_json::Value> = serde_json::from_slice(&request.body).map_err(|_| (400, "Bad Request", "not a list of events"))?;
    if items.len() > MAX_EVENTS_PER_POST {
        return Err((413, "Payload Too Large", "too many events"));
    }
    let events: Vec<PageEvent> = items
        .into_iter()
        .map(serde_json::from_value)
        .collect::<Result<_, _>>()
        .map_err(|_| (400, "Bad Request", "unknown event"))?;
    {
        let mut w = state.log_window.lock().map_err(|_| (500, "Internal Server Error", "lock"))?;
        if w.0.elapsed() >= Duration::from_secs(1) {
            *w = (Instant::now(), 0);
        }
        let n = u32::try_from(events.len()).unwrap_or(u32::MAX);
        if w.1.saturating_add(n) > MAX_EVENTS_PER_SECOND {
            return Err((429, "Too Many Requests", "too many events this second"));
        }
        w.1 += n;
    }
    let count = events.len();
    if let Ok(mut q) = state.page_events.lock() {
        q.extend(events);
        while q.len() > MAX_QUEUED_EVENTS {
            q.pop_front();
        }
    }
    Ok(count)
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

/// The **Preview script** added to `index.html` (WASM-008 R3, R5, R7, R13, D6):
///
/// - asks for the change count by long poll (`/__prc/version?since=<rev>&wait=25000`: the server holds the request
///   until the count or the build errors change, or 25 s pass, so an idle page makes one request in 25 s and a
///   change reaches it at once; an answer that comes back at once with nothing new is asked again after 150 ms,
///   as is a failed one); on a change, hands the application its new configuration so it
///   takes the new module, descriptions and stylesheet in place, without reloading the page or the player
///   (`app.update`, WASM-016 R16), and reloads only when it answers that it cannot (or the page holds no
///   application); each update is timed with `performance.now()` and reported as `updated {how, ms}`;
/// - wraps `console.*`, `window.onerror` and `unhandledrejection` and reports them as `console` / `error`;
/// - reports the player's new run-time errors (`errors()`) as `cobol-error`, once the player has settled, from a
///   timer of its own (250 ms; 1 s while the page is hidden) that makes no request: a run-time error is the
///   page's, not the server's, and does not wait for a change;
/// - tags every event with its `form` (the route `#/form` of an SPA, `pr-form` of a standalone window) and
///   sends it over `window.ipc` inside the headless helper, else `POST /__prc/log`;
/// - draws a failed build's errors (`/__prc/version`'s `errors`) over the page in a dismissable overlay,
///   and a "the Preview has stopped" banner after three failed polls in a row.
///
/// Every text the page shows comes from [`PageStrings`] and goes in with `textContent`; nothing is parsed
/// as markup and nothing is evaluated.
const PREVIEW_SCRIPT: &str = r#"var ipc=window.ipc&&typeof window.ipc.postMessage==='function'?window.ipc:null;
var queue=[],timer=null,v=null,rev=0,pause=0,fails=0,banner=null,overlay=null,shown='',sent=0;
function form(){var m=/^#\/([^\/?#]+)/.exec(location.hash||'');if(m){try{return decodeURIComponent(m[1]);}catch(x){return m[1];}}
try{return new URLSearchParams(location.search).get('pr-form')||'';}catch(x){return '';}}
function flush(){timer=null;var batch=queue.splice(0,40);if(!batch.length){return;}
fetch('/__prc/log',{method:'POST',headers:{'Content-Type':'application/json'},body:JSON.stringify(batch),cache:'no-store',keepalive:true}).catch(function(){});
if(queue.length){timer=setTimeout(flush,100);}}
function send(e){if(!e.form){e.form=form();}
if(ipc){try{ipc.postMessage(JSON.stringify(e));}catch(x){}return;}
if(queue.length<200){queue.push(e);}if(!timer){timer=setTimeout(flush,100);}}
function text(a){try{return typeof a==='string'?a:(a instanceof Error?String(a.stack||a):JSON.stringify(a));}catch(x){return String(a);}}
['log','info','warn','error','debug'].forEach(function(level){var original=console[level];
console[level]=function(){send({event:'console',level:level,text:Array.prototype.map.call(arguments,text).join(' ').slice(0,4000)});
if(original){original.apply(console,arguments);}};});
window.addEventListener('error',function(e){send({event:'error',message:String(e.message||''),source:String(e.filename||''),line:e.lineno||0});});
window.addEventListener('unhandledrejection',function(e){send({event:'error',message:text(e.reason),source:'promise',line:0});});
function el(tag,cls,t){var n=document.createElement(tag);n.className=cls;if(t){n.textContent=t;}return n;}
function look(n,css){Object.keys(css).forEach(function(k){n.style.setProperty(k,css[k]);});}
var box={position:'fixed',left:'0',right:'0','z-index':'2147483647',font:'13px/1.4 system-ui,sans-serif',padding:'8px 12px'};
function showBanner(){if(banner||!document.body){return;}banner=el('div','prc-preview-stopped',S.stopped);
look(banner,box);look(banner,{top:'0',background:'#5a1a1a',color:'#fff'});document.body.appendChild(banner);}
function hideBanner(){if(banner){banner.remove();banner=null;}}
function showErrors(list){list=Array.isArray(list)?list:[];var key=JSON.stringify(list);if(key===shown){return;}shown=key;
if(overlay){overlay.remove();overlay=null;}if(!list.length||!document.body){return;}
overlay=el('div','prc-preview-errors');look(overlay,box);look(overlay,{bottom:'0','max-height':'50vh',overflow:'auto',background:'#2b0d0d',color:'#ffdede'});
var b=el('button','prc-preview-errors-dismiss',S.dismiss);b.type='button';look(b,{float:'right'});
b.addEventListener('click',function(){if(overlay){overlay.remove();overlay=null;}});
overlay.appendChild(b);overlay.appendChild(el('div','prc-preview-errors-title',S.errors_title));
list.forEach(function(line){var pre=el('pre','prc-preview-error',String(line));look(pre,{margin:'4px 0','white-space':'pre-wrap'});overlay.appendChild(pre);});
document.body.appendChild(overlay);}
function mounted(){var c=document.querySelector('[data-pr-config]');if(!c||!window.PowerRustCOBOL){return Promise.resolve(null);}
return PowerRustCOBOL.mount(c).then(function(app){return app&&typeof app.update==='function'?app:null;},function(){return null;});}
function cobolErrors(){return mounted().then(function(app){if(!app||!app.player||typeof app.player.errors!=='function'){return;}
return Promise.resolve(typeof app.settled==='function'?app.settled():null).then(function(){var list;
try{list=JSON.parse(app.player.errors());}catch(x){return;}if(!Array.isArray(list)){return;}if(list.length<sent){sent=0;}
list.slice(sent).forEach(function(e){send({event:'cobol-error',form:String(e.form||form()),unit:String(e.event||''),line:Number(e.line)||0,
code:e.code==null?'':String(e.code),message:String(e.message||e.text||'')});});sent=list.length;});}).catch(function(){});}
function inPlace(){var host=document.querySelector('[data-pr-config]');return mounted().then(function(app){if(!app){return 'reload';}
return fetch(host.getAttribute('data-pr-config')||'config.json',{cache:'no-store'}).then(function(r){return r.json();})
.then(function(c){var t0=performance.now();return Promise.resolve(app.update(c)).then(function(a){
send({event:'updated',how:String(a),ms:Math.round((performance.now()-t0)*10)/10});return a;});});});}
function reload(){flush();location.reload();}
function poll(){var t0=Date.now(),asked=rev;
fetch('/__prc/version?since='+rev+'&wait=25000',{cache:'no-store'}).then(function(r){return r.json();}).then(function(j){fails=0;hideBanner();showErrors(j.errors);
rev=typeof j.rev==='number'?j.rev:0;pause=Date.now()-t0<100&&rev===asked?150:0;
if(v===null){v=j.version;return;}if(j.version===v){return;}v=j.version;
return inPlace().then(function(a){if(a==='reload'||a==='unmounted'){reload();}},reload);},
function(){fails+=1;pause=150;if(fails>=3){showBanner();}}).catch(function(){}).finally(function(){setTimeout(poll,pause);});}
function watchCobol(){cobolErrors().finally(function(){setTimeout(watchCobol,document.hidden?1000:250);});}
poll();watchCobol();"#;

/// The Preview script with the page's texts: `(function(S){…})({…});`.
fn reload_script(strings: &PageStrings) -> String {
    let data = serde_json::to_string(strings).expect("strings serialise");
    format!("(function(S){{\n{PREVIEW_SCRIPT}\n}})({data});\n")
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
    let bare = request.path.split(['?', '#']).next().unwrap_or("");
    let is_log = request.method == "POST" && bare == LOG_PATH && state.live_reload;
    if request.method != "GET" && !head_only && !is_log {
        text_response(&mut stream, 405, "Method Not Allowed", "only GET and HEAD", false, state)?;
        return finish(stream, state);
    }
    // A name other than our own on the Host header is a page elsewhere reaching
    // in (DNS rebinding); it is refused before any file is looked at.
    if !host_is_ours(request.host.as_deref(), state.port) {
        if is_log {
            state.log_refused.fetch_add(1, Ordering::SeqCst);
        }
        text_response(&mut stream, 403, "Forbidden", "unknown host", head_only, state)?;
        return finish(stream, state);
    }
    if is_log {
        match take_log(&request, state) {
            Ok(_) => respond(&mut stream, 204, "No Content", "text/plain; charset=utf-8", &[], b"", false, state)?,
            Err((code, reason, why)) => {
                state.log_refused.fetch_add(1, Ordering::SeqCst);
                text_response(&mut stream, code, reason, why, false, state)?;
            }
        }
        return finish(stream, state);
    }
    if state.live_reload && bare == VERSION_PATH {
        state.version_requests.fetch_add(1, Ordering::SeqCst);
        let query = match long_poll_query(&request.path) {
            Ok(q) => q,
            Err(why) => {
                text_response(&mut stream, 400, "Bad Request", why, head_only, state)?;
                return finish(stream, state);
            }
        };
        // A long poll is held here, on this request's own thread, until the page has something to hear of.
        let rev = query.map(|(since, wait)| state.hold_for_change(since, wait));
        let body = state.version_body(rev);
        respond(&mut stream, 200, "OK", "application/json; charset=utf-8", &[], body.as_bytes(), head_only, state)?;
        return finish(stream, state);
    }
    if state.live_reload && bare == RELOAD_SCRIPT_PATH {
        let strings = state.page_strings.lock().map(|s| s.clone()).unwrap_or_default();
        respond(&mut stream, 200, "OK", "text/javascript; charset=utf-8", &[], reload_script(&strings).as_bytes(), head_only, state)?;
        return finish(stream, state);
    }
    if bare == PHONE_PATH {
        if let Some(frame) = state.phone.lock().ok().and_then(|f| *f) {
            let banner = state.page_strings.lock().map(|s| s.not_the_device.clone()).unwrap_or_default();
            respond(&mut stream, 200, "OK", "text/html; charset=utf-8", &[], phone_page(frame, &banner).as_bytes(), head_only, state)?;
            return finish(stream, state);
        }
    }
    match resolve(&state.root, &request.path) {
        Ok(file) => match std::fs::read(&file) {
            Ok(mut body) => {
                let mime = mime_for(&file);
                if state.live_reload && file.file_name().is_some_and(|n| n == "index.html") {
                    body = with_reload_script(&body);
                }
                // The development Backend goes into the response, never into the file (D4).
                if bare == "/config.json" {
                    let dev = state.dev_backend.lock().ok().and_then(|d| d.clone());
                    if let Some(next) = dev.and_then(|d| with_dev_backend(&body, &d)) {
                        body = next;
                    }
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

    fn post_log(port: u16, origin: Option<&str>, content_type: &str, body: &str) -> Reply {
        let origin = origin.map(|o| format!("Origin: {o}\r\n")).unwrap_or_default();
        raw(
            port,
            &format!(
                "POST {LOG_PATH} HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\n{origin}Content-Type: {content_type}\r\nContent-Length: {}\r\n\r\n{body}",
                body.len()
            ),
        )
    }

    /// WASM-008 T32 (R13, D13): `POST /__prc/log` takes the four page events
    /// from the server's own page, in order, typed; every other request to it
    /// is refused and counted; every other POST is still 405.
    #[test]
    fn page_events_are_taken_from_our_own_page_only_008() {
        let (outer, root) = bundle("log");
        let server = start(&root, &ServeOptions::default()).unwrap();
        let p = server.port();
        let ours = format!("http://127.0.0.1:{p}");
        let batch = r#"[{"event":"console","level":"error","text":"boom","form":"MAIN-FORM"},
            {"event":"error","message":"x is not defined","source":"app.js","line":12,"form":"MAIN-FORM"},
            {"event":"cobol-error","form":"MAIN-FORM","unit":"BTN-OK-CLICK","line":7,"code":"EC-SIZE","message":"size error","extra":1},
            {"event":"updated","how":"stylesheet","ms":38.5,"form":"MAIN-FORM"}]"#;
        assert_eq!(post_log(p, Some(&ours), "application/json", batch).status, 204);
        let got = server.take_page_events();
        assert_eq!(
            got,
            vec![
                PageEvent::Console { level: "error".into(), text: "boom".into(), form: "MAIN-FORM".into() },
                PageEvent::Error { message: "x is not defined".into(), source: "app.js".into(), line: 12, form: "MAIN-FORM".into() },
                PageEvent::CobolError {
                    form: "MAIN-FORM".into(),
                    unit: "BTN-OK-CLICK".into(),
                    line: 7,
                    code: "EC-SIZE".into(),
                    message: "size error".into()
                },
                PageEvent::Updated { how: "stylesheet".into(), ms: 38.5, form: "MAIN-FORM".into() },
            ]
        );
        assert!(server.take_page_events().is_empty(), "taken once");
        assert_eq!(server.page_events_refused(), 0);
        println!("page events: a batch of 4 kinds came out in order with its form");

        let one = r#"[{"event":"console","level":"log","text":"x"}]"#;
        let too_many = format!("[{}]", vec![r#"{"event":"console","level":"log","text":"x"}"#; MAX_EVENTS_PER_POST + 1].join(","));
        let oversize = format!(r#"[{{"event":"console","level":"log","text":"{}"}}]"#, "a".repeat(MAX_LOG_BODY));
        let localhost = format!("http://localhost:{p}");
        let cases: Vec<(&str, Reply)> = vec![
            ("wrong Host", raw(p, &format!("POST {LOG_PATH} HTTP/1.1\r\nHost: evil.example:{p}\r\nOrigin: {ours}\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{one}", one.len()))),
            ("foreign Origin", post_log(p, Some("http://evil.example"), "application/json", one)),
            ("another port's Origin", post_log(p, Some("http://127.0.0.1:1"), "application/json", one)),
            ("no Origin", post_log(p, None, "application/json", one)),
            ("text/plain", post_log(p, Some(&ours), "text/plain", one)),
            ("body over 16 KiB", post_log(p, Some(&ours), "application/json", &oversize)),
            ("41 events", post_log(p, Some(&ours), "application/json", &too_many)),
            ("unknown event", post_log(p, Some(&ours), "application/json", r#"[{"event":"keylog","text":"x"}]"#)),
            ("not a list", post_log(p, Some(&ours), "application/json", r#"{"event":"console"}"#)),
            ("not json", post_log(p, Some(&ours), "application/json", "hello")),
        ];
        let mut refused = 0;
        for (why, r) in &cases {
            assert!(r.status >= 400, "{why} -> {}", r.status);
            refused += 1;
            println!("  refused: {why:<22} -> {}", r.status);
        }
        assert_eq!(server.page_events_refused(), refused);
        assert!(server.take_page_events().is_empty(), "nothing of a refused request is kept");
        assert_eq!(post_log(p, Some(&localhost), "application/json; charset=utf-8", one).status, 204, "localhost is ours too");
        assert_eq!(server.take_page_events().len(), 1);

        // The rate: 200 events a second, then refused until the second is over.
        let forty = format!("[{}]", vec![r#"{"event":"console","level":"log","text":"x"}"#; MAX_EVENTS_PER_POST].join(","));
        let mut accepted = 1; // the localhost one above, in this window or the last
        let mut burst_refused = false;
        for _ in 0..8 {
            match post_log(p, Some(&ours), "application/json", &forty).status {
                204 => accepted += MAX_EVENTS_PER_POST,
                429 => burst_refused = true,
                other => panic!("unexpected {other}"),
            }
        }
        assert!(burst_refused, "a burst over {MAX_EVENTS_PER_SECOND}/s is refused");
        let taken = server.take_page_events().len();
        assert!(taken <= MAX_EVENTS_PER_SECOND as usize * 2, "{taken}");
        println!("  rate: 8 posts of 40 in a burst -> {taken} events taken, the rest refused with 429 ({accepted} counted)");
        println!("page events: {} refusals counted ({} cases + the burst)", server.page_events_refused(), cases.len());

        // Every other method on every other path is still 405; GET on the route is not a file.
        assert_eq!(raw(p, &format!("POST /app.js HTTP/1.1\r\nHost: 127.0.0.1:{p}\r\nContent-Length: 0\r\n\r\n")).status, 405);
        assert_eq!(raw(p, &format!("PUT {LOG_PATH} HTTP/1.1\r\nHost: 127.0.0.1:{p}\r\nContent-Length: 0\r\n\r\n")).status, 405);
        assert_eq!(get(p, LOG_PATH).status, 404);
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
        let text = String::from_utf8_lossy(&script.body);
        assert!(text.contains("location.reload"));
        // WASM-016 R16: a change is offered to the application first, in place; a reload is the fallback.
        assert!(text.contains("app.update(c)") && text.contains("a==='reload'"), "{text}");

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

    /// WASM-008 T47: a build is one change. While it writes the folder — the
    /// generator swaps the whole folder — the watcher counts nothing; its end
    /// counts once, and the watcher starts again from the folder it left. A
    /// failed build counts nothing, during or after.
    #[test]
    fn a_build_is_counted_once_at_its_end_never_in_mid_write_008() {
        let (outer, root) = bundle("generation");
        let server = start(&root, &ServeOptions::default()).unwrap();
        let settle = || thread::sleep(WATCH_INTERVAL * 4);
        let v = server.version();
        server.build_started();
        // The generator's swap: the folder moved away, then a new one in its place.
        let old = outer.join("bundle.old");
        std::fs::rename(&root, &old).unwrap();
        settle();
        std::fs::create_dir_all(&root).unwrap();
        for f in ["index.html", "app.js", "program.wasm", "player.wasm"] {
            std::fs::copy(old.join(f), root.join(f)).unwrap();
        }
        std::fs::write(root.join("app.css"), "body{margin:3px}").unwrap();
        settle();
        assert_eq!(server.version(), v, "nothing is counted while the build writes");
        server.notify_changed();
        assert_eq!(server.version(), v + 1, "the build's end is one change");
        settle();
        assert_eq!(server.version(), v + 1, "the watcher does not count the build again");
        assert_eq!(String::from_utf8_lossy(&get(server.port(), "/app.css").body), "body{margin:3px}");

        server.build_started();
        std::fs::write(root.join("app.js"), "console.log('half')").unwrap();
        settle();
        server.build_failed();
        settle();
        assert_eq!(server.version(), v + 1, "a failed build counts nothing, during or after");

        // Outside a build the watcher counts as before.
        std::fs::write(root.join("app.css"), "body{margin:4px}").unwrap();
        let mut waited = 0;
        while server.version() == v + 1 && waited < 4000 {
            thread::sleep(Duration::from_millis(10));
            waited += 10;
        }
        assert_eq!(server.version(), v + 2, "a hand edit is still seen");
        drop(server);
        let _ = std::fs::remove_dir_all(outer);
        println!("devserver: a whole-folder swap during a build counted 0 times, its end once; a failed build 0; a later edit once");
    }

    /// WASM-007 T23: a form's stylesheet is written into the served bundle and
    /// counted at once — no Build, no watcher wait; the page is served the new
    /// configuration naming the new sheet. A sheet with an error writes nothing.
    #[test]
    fn a_form_stylesheet_is_applied_in_place_and_counted_at_once() {
        let (outer, root) = bundle("fastcss");
        let config = r#"{"forms":[{"name":"main-form","url":"forms/main-form.1.json","integrity":"x","css":{"url":"forms/main-form.0.css","integrity":"y"}}],"player":{}}"#;
        std::fs::write(root.join("config.json"), config).unwrap();
        let server = start(&root, &ServeOptions::default()).unwrap();
        let v = server.version();
        let css = "#LBL-OUT { word-spacing: 7px }\r\n";
        server.apply_form_stylesheet("MAIN-FORM", css, &[]).unwrap();
        assert_eq!(server.version(), v + 1, "counted at once");
        let served: serde_json::Value = serde_json::from_slice(&get(server.port(), "/config.json").body).unwrap();
        let entry = &served["forms"][0];
        assert_eq!(entry["url"], "forms/main-form.1.json", "the description is not touched (no restart)");
        let sheet = entry["css"]["url"].as_str().unwrap();
        assert_ne!(sheet, "forms/main-form.0.css");
        assert_eq!(get(server.port(), &format!("/{sheet}")).body, css.as_bytes(), "the sheet byte for byte");
        assert!(!root.join(".config.json.tmp").exists());

        let before = std::fs::read(root.join("config.json")).unwrap();
        let v = server.version();
        let e = server.apply_form_stylesheet("main-form", ".a { background: url(https://evil.example/x) }", &[]).unwrap_err();
        assert!(e[0].starts_with("forms/main-form.css:1:"), "{e:?}");
        assert_eq!(std::fs::read(root.join("config.json")).unwrap(), before, "nothing written");
        assert_eq!(server.version(), v);
        println!("devserver fast path: {sheet} served, count moved at once; refused {e:?}");
        drop(server);
        let _ = std::fs::remove_dir_all(outer);
    }

    /// WASM-008 T33 (R13, R5, R3, D6): the Preview script relays the console,
    /// errors, COBOL errors and update times over `window.ipc` or `/__prc/log`,
    /// asks for changes by long poll, and inserts no markup and evaluates nothing; the page's
    /// texts are the ones the IDE handed over.
    #[test]
    fn the_preview_script_relays_and_never_parses_markup_008() {
        let script = reload_script(&PageStrings::default());
        for needle in [
            "app.update(c)",
            "a==='reload'",
            "errors()",
            "/__prc/log",
            "window.ipc",
            "performance.now",
            "/__prc/version?since='+rev+'&wait=25000",
            "setTimeout(poll,pause)",
            "setTimeout(watchCobol,",
            "unhandledrejection",
            "'cobol-error'",
            "'updated'",
            "pr-form",
            "location.hash",
            "fails>=3",
            "textContent",
        ] {
            assert!(script.contains(needle), "{needle}");
        }
        for banned in ["innerHTML", "outerHTML", "insertAdjacentHTML", "eval(", "new Function", "document.write", "setAttribute('style'"] {
            assert!(!script.contains(banned), "{banned}");
        }
        // The 150 ms poll of D6 is gone: the page asks by long poll, and waits 150 ms only after a failed or empty answer.
        assert!(!script.contains("setTimeout(poll,150)") && !script.contains("setTimeout(poll,400)"), "no fixed-interval poll");
        assert!(!script.contains("fetch('/__prc/version',"), "every ask carries its `since`");

        let (outer, root) = bundle("strings");
        let server = start(&root, &ServeOptions::default()).unwrap();
        let p = server.port();
        let pt = PageStrings {
            stopped: "A pré-visualização parou.".into(),
            errors_title: "A compilação falhou </script><b>".into(),
            dismiss: "Fechar".into(),
            not_the_device: "Não é o dispositivo.".into(),
        };
        server.set_page_strings(pt.clone());
        let served = String::from_utf8(get(p, RELOAD_SCRIPT_PATH).body).unwrap();
        let data = served.rsplit_once("})(").unwrap().1.trim_end().trim_end_matches(");");
        assert_eq!(serde_json::from_str::<PageStrings>(data).unwrap(), pt, "the texts arrive as data, in the IDE's language");
        println!("preview script: {} bytes, long poll (25 s hold), no markup sink, texts as data ({} bytes)", served.len(), data.len());
        drop(server);
        let _ = std::fs::remove_dir_all(outer);
    }

    /// WASM-008 T34 (R7, AC5): a failed build's errors ride on `/__prc/version`
    /// until the next good build; the overlay uses `textContent` only.
    #[test]
    fn a_failed_build_s_errors_ride_on_the_version_until_a_good_one_008() {
        let (outer, root) = bundle("errors");
        let server = start(&root, &ServeOptions::default()).unwrap();
        let p = server.port();
        let read = || serde_json::from_slice::<serde_json::Value>(&get(p, VERSION_PATH).body).unwrap();
        assert_eq!(read().get("errors"), None);
        let before = std::fs::read(root.join("index.html")).unwrap();
        let lines = vec!["webforms/main-form.wcfrm:12: BTN-OK has no Click handler".to_owned(), "src/main.cbl:3: <script>".to_owned()];
        server.set_build_errors(lines.clone());
        let j = read();
        assert_eq!(j["errors"], serde_json::json!(lines));
        assert!(j["version"].as_u64().is_some());
        assert_eq!(std::fs::read(root.join("index.html")).unwrap(), before, "the bundle is not touched");
        server.set_build_errors(Vec::new());
        assert_eq!(read().get("errors"), None, "a good build removes them");
        let script = reload_script(&PageStrings::default());
        let overlay = &script[script.find("function showErrors").unwrap()..script.find("function mounted").unwrap()];
        assert!(overlay.contains("textContent") || overlay.contains("el('pre'"), "{overlay}");
        assert!(!overlay.contains("innerHTML") && !overlay.contains("insertAdjacentHTML"));
        println!("build errors: {} lines carried by /__prc/version, removed by the next good build", lines.len());
        drop(server);
        let _ = std::fs::remove_dir_all(outer);
    }

    /// One long poll on its own thread: the answer, and the instant it arrived.
    fn long_poll(port: u16, since: u64, wait_ms: u64) -> thread::JoinHandle<(serde_json::Value, Instant)> {
        thread::spawn(move || {
            let r = get(port, &format!("{VERSION_PATH}?since={since}&wait={wait_ms}"));
            assert_eq!(r.status, 200);
            (serde_json::from_slice(&r.body).unwrap(), Instant::now())
        })
    }

    /// A long poll, asked and answered here: the answer and how long it took.
    fn ask(port: u16, since: u64, wait_ms: u64) -> (serde_json::Value, Duration) {
        let t = Instant::now();
        let r = get(port, &format!("{VERSION_PATH}?since={since}&wait={wait_ms}"));
        assert_eq!(r.status, 200);
        (serde_json::from_slice(&r.body).unwrap(), t.elapsed())
    }

    /// The Preview's page no longer asks every 150 ms: `/__prc/version?since=<rev>&wait=<ms>` is held until the
    /// count or the build errors change, or `wait` runs out, and answers a change at once. Without `since` the
    /// answer is the immediate one it always was.
    #[test]
    fn a_long_poll_is_held_until_there_is_something_to_hear_of_and_a_change_is_answered_at_once() {
        let (outer, root) = bundle("longpoll");
        let server = start(&root, &ServeOptions::default()).unwrap();
        let p = server.port();

        // The first ask of a page (`since=0`) is answered at once, with the revision it is to hold on.
        let (a, took) = ask(p, 0, 0);
        let rev = a["rev"].as_u64().expect("a long poll's answer names its revision");
        assert!(rev >= 1 && a["version"] == server.version() && took < Duration::from_millis(500), "{a} in {took:?}");
        // The plain request keeps its shape: no revision.
        assert_eq!(String::from_utf8_lossy(&get(p, VERSION_PATH).body), format!("{{\"version\":{}}}", server.version()));
        // Nothing changes: it is held for its `wait`, and the answer is the same revision and count.
        let (same, held) = ask(p, rev, 300);
        assert_eq!((&same["rev"], &same["version"]), (&a["rev"], &a["version"]));
        assert!(held >= Duration::from_millis(250) && held < Duration::from_secs(2), "held {held:?}");

        // A change while it is held ends the hold at once.
        let version = server.version();
        let waiting = long_poll(p, rev, 5000);
        thread::sleep(Duration::from_millis(200));
        let changed_at = Instant::now();
        server.notify_changed();
        let (b, answered_at) = waiting.join().unwrap();
        let by_notify = answered_at.duration_since(changed_at);
        assert_eq!((b["version"].as_u64(), b["rev"].as_u64()), (Some(version + 1), Some(rev + 1)), "{b}");
        assert!(by_notify < Duration::from_millis(250), "answered {by_notify:?} after the change");

        // A page that is behind (it holds an older revision) is answered at once, not held.
        let (c, took) = ask(p, rev, 5000);
        assert_eq!(c["rev"].as_u64(), Some(rev + 1));
        assert!(took < Duration::from_millis(500), "behind: {took:?}");
        // So is one that is ahead (a server started again counts from 1).
        let (d, took) = ask(p, rev + 100, 5000);
        assert_eq!(d["rev"].as_u64(), Some(rev + 1));
        assert!(took < Duration::from_millis(500), "ahead: {took:?}");

        // A failed build's errors wake it too, with the count unmoved ...
        let rev = rev + 1;
        let waiting = long_poll(p, rev, 5000);
        thread::sleep(Duration::from_millis(200));
        let errored_at = Instant::now();
        server.set_build_errors(vec!["webforms/main-form.wcfrm:3: broken".to_owned()]);
        let (e, answered_at) = waiting.join().unwrap();
        let by_errors = answered_at.duration_since(errored_at);
        assert_eq!(e["errors"], serde_json::json!(["webforms/main-form.wcfrm:3: broken"]));
        assert_eq!((e["version"].as_u64(), e["rev"].as_u64()), (Some(version + 1), Some(rev + 1)), "{e}");
        assert!(by_errors < Duration::from_millis(250), "answered {by_errors:?} after the errors");
        // ... but the same errors again are nothing new: the request is held to its wait.
        let rev = rev + 1;
        server.set_build_errors(vec!["webforms/main-form.wcfrm:3: broken".to_owned()]);
        let (same, held) = ask(p, rev, 300);
        assert_eq!(same["rev"].as_u64(), Some(rev));
        assert!(held >= Duration::from_millis(250), "the same errors woke it: {held:?}");
        // The next good build takes them away, and that is heard.
        let waiting = long_poll(p, rev, 5000);
        thread::sleep(Duration::from_millis(100));
        server.set_build_errors(Vec::new());
        let (f, _) = waiting.join().unwrap();
        assert_eq!((f.get("errors"), f["rev"].as_u64()), (None, Some(rev + 1)), "{f}");

        // A hand edit, seen by the watcher, is heard.
        let rev = rev + 1;
        let waiting = long_poll(p, rev, 5000);
        thread::sleep(Duration::from_millis(100));
        let edited_at = Instant::now();
        std::fs::write(root.join("app.css"), "body{margin:9px}").unwrap();
        let (g, answered_at) = waiting.join().unwrap();
        let by_watch = answered_at.duration_since(edited_at);
        assert_eq!((g["version"].as_u64(), g["rev"].as_u64()), (Some(version + 2), Some(rev + 1)), "{g}");
        assert!(by_watch < WATCH_INTERVAL * 6 + Duration::from_millis(500), "the watcher took {by_watch:?}");

        // Every ask is counted when it arrives (9 long polls above and 1 plain one), the held ones too.
        assert_eq!(server.version_requests(), 10, "asks counted on arrival");
        println!(
            "long poll: held {:.0} ms for nothing; a change answered {:.0} ms after it, build errors {:.0} ms, a hand edit {:.0} ms (watch interval {} ms); behind or ahead answered at once; same errors again woke nothing",
            held.as_secs_f64() * 1000.0,
            by_notify.as_secs_f64() * 1000.0,
            by_errors.as_secs_f64() * 1000.0,
            by_watch.as_secs_f64() * 1000.0,
            WATCH_INTERVAL.as_millis()
        );
        drop(server);
        let _ = std::fs::remove_dir_all(outer);
    }

    /// What the page may put on the long poll is whole numbers; anything else is refused, and a `wait` beyond what the
    /// server holds is cut.
    #[test]
    fn the_long_poll_parameters_are_whole_numbers_and_the_wait_is_cut() {
        assert_eq!(long_poll_query("/__prc/version"), Ok(None));
        assert_eq!(long_poll_query("/__prc/version?wait=500"), Ok(None), "no `since`: the plain, immediate answer");
        assert_eq!(long_poll_query("/__prc/version?since=7"), Ok(Some((7, Duration::ZERO))));
        assert_eq!(long_poll_query("/__prc/version?since=7&wait=25000"), Ok(Some((7, Duration::from_secs(25)))));
        assert_eq!(long_poll_query("/__prc/version?wait=25000&since=7&x=y#frag"), Ok(Some((7, Duration::from_secs(25)))));
        assert_eq!(long_poll_query("/__prc/version?since=7&wait=99999999"), Ok(Some((7, LONG_POLL_MAX))), "cut");
        for bad in ["?since=", "?since=-1", "?since=abc", "?since=1.5", "?since=7&wait=", "?since=7&wait=-5", "?since=7&wait=soon", "?since=18446744073709551616"] {
            assert!(long_poll_query(&format!("/__prc/version{bad}")).is_err(), "{bad}");
        }
        let (outer, root) = bundle("longpoll-bad");
        let server = start(&root, &ServeOptions::default()).unwrap();
        let p = server.port();
        for bad in ["?since=abc", "?since=1&wait=x"] {
            assert_eq!(get(p, &format!("{VERSION_PATH}{bad}")).status, 400, "{bad}");
        }
        println!("long poll parameters: 6 forms read, 8 refused as numbers, 2 refused by the server with 400, a wait beyond {} s cut", LONG_POLL_MAX.as_secs());
        drop(server);
        let _ = std::fs::remove_dir_all(outer);
    }

    /// Stopping the server does not leave a page's held request to sleep out its wait: it is answered at once, so the
    /// page sees the connection end and says the Preview has stopped.
    #[test]
    fn stopping_the_server_answers_a_held_long_poll_at_once() {
        let (outer, root) = bundle("longpoll-stop");
        let mut server = start(&root, &ServeOptions::default()).unwrap();
        let p = server.port();
        let (a, _) = ask(p, 0, 0);
        let rev = a["rev"].as_u64().unwrap();
        let waiting = long_poll(p, rev, 20_000);
        thread::sleep(Duration::from_millis(200));
        let stopped_at = Instant::now();
        server.stop();
        let stop_took = stopped_at.elapsed();
        let (answer, answered_at) = waiting.join().unwrap();
        let after = answered_at.duration_since(stopped_at);
        assert_eq!(answer["rev"].as_u64(), Some(rev), "nothing changed: the same revision");
        assert!(after < Duration::from_secs(1), "the held request slept on for {after:?}");
        println!("stop with a request held for 20 s: stop() took {} ms, the request was answered {} ms after the stop call", stop_took.as_millis(), after.as_millis());
        let _ = std::fs::remove_dir_all(outer);
    }

    #[test]
    fn the_development_backend_rides_in_the_served_config_only_008() {
        let (outer, root) = bundle("devbackend");
        let config = br#"{"backend":{"origins":["wss://api.example.com"],"unsafe_plain_ws":false,"call_timeout_ms":30000}}"#;
        std::fs::write(root.join("config.json"), config).unwrap();
        let server = start(&root, &ServeOptions::default()).unwrap();
        let p = server.port();
        let read = || serde_json::from_slice::<serde_json::Value>(&get(p, "/config.json").body).unwrap();
        assert_eq!(read()["backend"].get("dev_token"), None, "no Backend, the file as it is");
        let dev = DevBackend { url: "ws://127.0.0.1:47011/ws".into(), token: "eyJhbGciOiJIUzI1NiJ9.e30.sig".into() };
        server.set_dev_backend(Some(dev.clone()));
        let j = read();
        assert_eq!(j["backend"]["origins"], serde_json::json!(["wss://api.example.com", "ws://127.0.0.1:47011/ws"]));
        assert_eq!(j["backend"]["unsafe_plain_ws"], serde_json::json!(true));
        assert_eq!(j["backend"]["dev_token"], serde_json::json!(dev.token));
        assert_eq!(j["backend"]["call_timeout_ms"], serde_json::json!(30000), "the rest is kept");
        assert_eq!(std::fs::read(root.join("config.json")).unwrap(), config.to_vec(), "the bundle is not touched");
        // Only a loopback ws:// address is ever served.
        let far = DevBackend { url: "ws://203.0.113.9:47011/ws".into(), token: "t".into() };
        assert_eq!(with_dev_backend(config, &far), None);
        server.set_dev_backend(None);
        assert_eq!(read()["backend"].get("dev_token"), None, "the run ended, the token is gone");
        println!("config.json: dev Backend {} served in the response only; the file on disk byte-identical", dev.url);
        drop(server);
        let _ = std::fs::remove_dir_all(outer);
    }

    #[test]
    fn the_phone_page_frames_the_application_at_the_preset_width_isolated_008() {
        let (outer, root) = bundle("phone");
        let server = start(&root, &ServeOptions { isolation: Isolation::RequireCorp, ..Default::default() }).unwrap();
        let p = server.port();
        assert_eq!(get(p, PHONE_PATH).status, 404, "no phone page until the Preview asks for one");
        let phone = crate::web::presets::preset("phone").unwrap().viewport(false);
        server.set_page_strings(PageStrings { not_the_device: "Not the device <b>".into(), ..PageStrings::default() });
        server.set_phone(Some(PhoneFrame { width: phone.width, height: phone.height }));
        let r = get(p, PHONE_PATH);
        assert_eq!(r.status, 200);
        let body = String::from_utf8_lossy(&r.body).into_owned();
        assert!(body.contains("<iframe id=\"pr-phone\" src=\"/\" width=\"390\" height=\"844\""), "{body}");
        assert!(body.contains("width:390px;height:844px"), "{body}");
        assert!(body.contains("Not the device &lt;b&gt;") && !body.contains("<b>"), "the banner is text: {body}");
        assert_eq!(r.header("Cross-Origin-Opener-Policy"), Some("same-origin"));
        assert_eq!(r.header("Cross-Origin-Embedder-Policy"), Some("require-corp"));
        assert_eq!(server.phone_url(), format!("{}{PHONE_PATH}", server.url().trim_end_matches('/')));
        println!("/__prc/phone: a 390 x 844 frame of /, COOP/COEP on, the banner as text");
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
