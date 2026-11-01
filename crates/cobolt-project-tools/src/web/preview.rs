// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! **How the web Preview is opened, and the headless helper** (WASM-008 R12,
//! R9, R16, R2, R8).
//!
//! The Preview and Run open the served page in the developer's browser — the
//! system's default, or the one the project names ([`BrowserChoice`],
//! [`open_in_browser`]); nothing here is a window of the IDE's.
//!
//! The system web view (WKWebView, WebView2, WebKitGTK) is used **headless**
//! only, in a helper process, `prc-web-preview` (crate `cobolt-web-preview`),
//! whose window no one sees: `render_form`, `run_form` and the tests draw a
//! web form with it. A web view that hangs or crashes takes the helper down
//! and nothing else (R2).
//!
//! The two talk in **JSON lines**: [`PreviewSession`] writes [`Command`]s to
//! the helper's standard input and reads [`Event`]s from its standard output.
//! The helper is built from the same definitions' *text* — it depends on
//! `serde_json` and nothing of ours — so `tests` here pin every line either
//! side writes. The page's own events have the JSON of `/__prc/log` and are
//! read by the same type ([`PageEvent`]).

use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, Stdio};
use std::sync::mpsc::{self, Receiver};
use std::thread;

use serde::{Deserialize, Serialize};

use crate::web::devserver::PageEvent;

/// The helper's file name.
#[cfg(windows)]
pub const HELPER_NAME: &str = "prc-web-preview.exe";
/// The helper's file name.
#[cfg(not(windows))]
pub const HELPER_NAME: &str = "prc-web-preview";

/// The environment variable that names the helper (a build tree, a test).
pub const HELPER_ENV: &str = "PRC_WEB_PREVIEW";

/// When this variable is set, [`open_in_system_browser`] checks the address and
/// answers `Ok` without starting a browser — so a test that exercises the
/// fall-back does not open a window on the developer's screen.
pub const NO_BROWSER_ENV: &str = "PRC_PREVIEW_NO_BROWSER";

/// What the IDE asks of the helper (plan §3).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "cmd", rename_all = "snake_case")]
pub enum Command {
    /// Open the invisible web view on `url`, at a viewport of `width` x
    /// `height` CSS pixels. Only the origin of `url` may be navigated to
    /// afterwards (WASM-008 R8). The emulation rides along, so the page loads
    /// once, not once per setting.
    Open {
        url: String,
        width: u32,
        height: u32,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        dpr: Option<f32>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        dark: Option<bool>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        locale: Option<String>,
    },
    /// Load the page again.
    Reload,
    /// Prefer the dark or the light colour scheme.
    Scheme { dark: bool },
    /// The device the page is told it runs on — `devicePixelRatio`,
    /// `navigator.language`, the insets `--prc-safe-area-*` carry (top, right,
    /// bottom, left, in CSS pixels). Any of the three may be left out; the
    /// page loads again **once** for all of them.
    Emulate {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        dpr: Option<f32>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        locale: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        safe_area: Option<[u32; 4]>,
    },
    /// Write a PNG of the viewport to `path`, at `width × dpr` by `height × dpr`
    /// pixels, through the engine's own snapshot (WASM-008 T41).
    Render { path: String },
    /// Run a closed list of steps on the page (`click`, `type`, `wait`, `get`,
    /// `text`, `style`, `rect`, `focused`, `key`, `snapshot`, `page`; the helper
    /// checks them and refuses any other) and answer `scripted`. `click` and
    /// `type` fire real DOM events (the operator's decision of 2026-11-01);
    /// `page` reads a window property of a fixed list, of the page or of the
    /// `phone-frame` (WASM-008 T47).
    Script {
        steps: Vec<serde_json::Value>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        time_limit_s: Option<u32>,
    },
    /// Close the web view and end.
    Close,
}

/// What the helper tells the IDE: its own events, and the page's.
#[derive(Debug, Clone, PartialEq)]
pub enum Event {
    /// The web view is made and the page is loading; `visible` is what the
    /// system says of its window (`false`; `None` from an older helper).
    Ready { visible: Option<bool> },
    /// The page tried to go to `url`, which is not its own origin; it did not.
    Blocked { url: String },
    /// The helper could not do what it was asked; `message` says why.
    Failed { message: String },
    /// The helper ended.
    Closed,
    /// The page started loading `url`: its first load, and every reload after
    /// it (WASM-008 T47) — so a test reads "no page reload" instead of inferring it.
    Loading { url: String },
    /// What the page reported — the JSON of `/__prc/log`.
    Page(PageEvent),
    /// The answer to `render`: the PNG at `path`, of these pixels.
    Rendered { path: String, width: u32, height: u32 },
    /// The answer to `script`: the values the reading steps read, in order
    /// (`null` for a step that failed), what the page logged meanwhile, and the
    /// page's and the steps' errors.
    Scripted { values: Vec<serde_json::Value>, text: String, errors: Vec<String> },
}

/// The helper's own events, as their lines are written.
#[derive(Deserialize)]
#[serde(tag = "event", rename_all = "snake_case")]
enum HelperEvent {
    Ready {
        #[serde(default)]
        visible: Option<bool>,
    },
    Blocked {
        url: String,
    },
    Failed {
        message: String,
    },
    Closed,
    Loading {
        url: String,
    },
    Rendered {
        path: String,
        width: u32,
        height: u32,
    },
    Scripted {
        #[serde(default)]
        values: Vec<serde_json::Value>,
        #[serde(default)]
        text: String,
        #[serde(default)]
        errors: Vec<String>,
    },
}

/// What the page is told about the device it runs on (WASM-008 R5). Every
/// field is optional: what is `None` is left as the web view has it.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Emulation {
    /// The `devicePixelRatio` the page reads.
    pub dpr: Option<f32>,
    /// Prefer the dark (`true`) or the light (`false`) colour scheme.
    pub dark: Option<bool>,
    /// The language the page sees (`navigator.language`).
    pub locale: Option<String>,
    /// The insets `--prc-safe-area-*` carry: top, right, bottom, left, in CSS pixels.
    pub safe_area: Option<[u32; 4]>,
}

impl Command {
    /// The command that opens the web view (the insets go by `emulate`).
    pub fn open(url: &str, width: u32, height: u32, emulation: &Emulation) -> Command {
        Command::Open { url: url.to_owned(), width, height, dpr: emulation.dpr, dark: emulation.dark, locale: emulation.locale.clone() }
    }

    /// One line of JSON, newline-terminated.
    pub fn to_line(&self) -> String {
        let mut s = serde_json::to_string(self).expect("a command serialises");
        s.push('\n');
        s
    }
}

impl Event {
    /// The event a line carries, or `None` for a line that is not one (an
    /// event this IDE does not know is ignored; so is a field).
    pub fn parse(line: &str) -> Option<Event> {
        if let Ok(e) = serde_json::from_str::<HelperEvent>(line.trim()) {
            return Some(match e {
                HelperEvent::Ready { visible } => Event::Ready { visible },
                HelperEvent::Blocked { url } => Event::Blocked { url },
                HelperEvent::Failed { message } => Event::Failed { message },
                HelperEvent::Closed => Event::Closed,
                HelperEvent::Loading { url } => Event::Loading { url },
                HelperEvent::Rendered { path, width, height } => Event::Rendered { path, width, height },
                HelperEvent::Scripted { values, text, errors } => Event::Scripted { values, text, errors },
            });
        }
        PageEvent::parse(line).map(Event::Page)
    }
}

/// Where the helper is, if it is installed: `PRC_WEB_PREVIEW`, else beside the
/// running executable (the IDE and `rcrun` ship together).
pub fn helper_path() -> Option<PathBuf> {
    if let Some(p) = std::env::var_os(HELPER_ENV).map(PathBuf::from) {
        return p.is_file().then_some(p);
    }
    let exe = std::env::current_exe().ok()?;
    let beside = exe.parent()?.join(HELPER_NAME);
    beside.is_file().then_some(beside)
}

/// Open `url` in the system browser. Only a loopback `http` address is opened:
/// the Preview opens the development server and nothing else, whatever text
/// reaches this function.
pub fn open_in_system_browser(url: &str) -> Result<(), String> {
    if !is_preview_url(url) {
        return Err("only the Preview's own address is opened".to_owned());
    }
    // Tests and CI check the decision without a browser window appearing.
    if std::env::var_os(NO_BROWSER_ENV).is_some() {
        return Ok(());
    }
    #[cfg(target_os = "macos")]
    let mut cmd = std::process::Command::new("open");
    #[cfg(target_os = "windows")]
    let mut cmd = {
        let mut c = std::process::Command::new("cmd");
        c.args(["/C", "start", ""]);
        c
    };
    #[cfg(all(not(target_os = "macos"), not(target_os = "windows")))]
    let mut cmd = std::process::Command::new("xdg-open");
    cmd.arg(url)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map(|_| ())
        .map_err(|e| format!("the system browser could not be started: {e}"))
}

/// Is `url` the address of a development server on this machine?
pub fn is_preview_url(url: &str) -> bool {
    let Some(rest) = url.strip_prefix("http://") else { return false };
    let authority = rest.split(['/', '?', '#']).next().unwrap_or("");
    if authority.contains('@') {
        return false;
    }
    let (host, port) = match authority.rsplit_once(':') {
        Some((h, p)) => (h, p),
        None => return false,
    };
    (host == "127.0.0.1" || host == "localhost") && !port.is_empty() && port.chars().all(|c| c.is_ascii_digit())
}

/// A browser the project may name in `preview_browser` (WASM-008 R12, D11):
/// a closed list, each resolved the way its operating system finds it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KnownBrowser {
    Chrome,
    Chromium,
    Edge,
    Firefox,
    /// macOS only.
    Safari,
    Brave,
}

impl KnownBrowser {
    pub const ALL: [KnownBrowser; 6] = [
        KnownBrowser::Chrome,
        KnownBrowser::Chromium,
        KnownBrowser::Edge,
        KnownBrowser::Firefox,
        KnownBrowser::Safari,
        KnownBrowser::Brave,
    ];

    /// The word `preview_browser` carries.
    pub fn id(self) -> &'static str {
        match self {
            KnownBrowser::Chrome => "chrome",
            KnownBrowser::Chromium => "chromium",
            KnownBrowser::Edge => "edge",
            KnownBrowser::Firefox => "firefox",
            KnownBrowser::Safari => "safari",
            KnownBrowser::Brave => "brave",
        }
    }

    /// The browser's own name (a product name, the same in every language).
    pub fn name(self) -> &'static str {
        match self {
            KnownBrowser::Chrome => "Google Chrome",
            KnownBrowser::Chromium => "Chromium",
            KnownBrowser::Edge => "Microsoft Edge",
            KnownBrowser::Firefox => "Firefox",
            KnownBrowser::Safari => "Safari",
            KnownBrowser::Brave => "Brave Browser",
        }
    }

    pub fn parse(id: &str) -> Option<KnownBrowser> {
        let id = id.trim();
        Self::ALL.into_iter().find(|b| b.id().eq_ignore_ascii_case(id))
    }

    /// The macOS application bundle's name.
    fn mac_app(self) -> &'static str {
        match self {
            KnownBrowser::Edge => "Microsoft Edge",
            other => other.name(),
        }
    }

    /// The Windows executable the registry's App Paths names.
    fn windows_exe(self) -> Option<&'static str> {
        match self {
            KnownBrowser::Chrome | KnownBrowser::Chromium => Some("chrome.exe"),
            KnownBrowser::Edge => Some("msedge.exe"),
            KnownBrowser::Firefox => Some("firefox.exe"),
            KnownBrowser::Brave => Some("brave.exe"),
            KnownBrowser::Safari => None,
        }
    }

    /// The usual commands on a Linux `PATH`, first found wins.
    fn linux_commands(self) -> &'static [&'static str] {
        match self {
            KnownBrowser::Chrome => &["google-chrome", "google-chrome-stable"],
            KnownBrowser::Chromium => &["chromium", "chromium-browser"],
            KnownBrowser::Edge => &["microsoft-edge", "microsoft-edge-stable"],
            KnownBrowser::Firefox => &["firefox"],
            KnownBrowser::Brave => &["brave-browser", "brave"],
            KnownBrowser::Safari => &[],
        }
    }

    /// Is this browser on this machine? (The settings row offers only these.)
    pub fn is_present(self) -> bool {
        resolve_known(self).is_ok()
    }
}

/// Which browser the Preview opens (WASM-008 R12), parsed from the project's
/// `preview_browser`: empty = the system's default, a known id, else a path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BrowserChoice {
    System,
    Named(KnownBrowser),
    Path(PathBuf),
}

impl BrowserChoice {
    pub fn parse(setting: &str) -> BrowserChoice {
        let s = setting.trim();
        if s.is_empty() {
            BrowserChoice::System
        } else if let Some(b) = KnownBrowser::parse(s) {
            BrowserChoice::Named(b)
        } else {
            BrowserChoice::Path(PathBuf::from(s))
        }
    }

    /// The name an Output line gives this browser.
    pub fn label(&self) -> String {
        match self {
            BrowserChoice::System => String::new(),
            BrowserChoice::Named(b) => b.name().to_owned(),
            BrowserChoice::Path(p) => p.display().to_string(),
        }
    }
}

/// The program and arguments [`open_in_browser`] starts — a plan the tests read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LaunchPlan {
    pub program: PathBuf,
    pub args: Vec<String>,
}

impl LaunchPlan {
    fn new(program: impl Into<PathBuf>, args: &[&str]) -> Self {
        LaunchPlan { program: program.into(), args: args.iter().map(|a| (*a).to_owned()).collect() }
    }
}

/// How `choice` opens `url` on this system. A browser that is not here is an
/// error naming it; the caller falls back to [`BrowserChoice::System`]. Only a
/// loopback `http` address is ever planned (WASM-008 R8).
pub fn launch_plan(url: &str, choice: &BrowserChoice) -> Result<LaunchPlan, String> {
    if !is_preview_url(url) {
        return Err("only the Preview's own address is opened".to_owned());
    }
    match choice {
        BrowserChoice::System => Ok(system_plan(url)),
        BrowserChoice::Named(b) => {
            let mut plan = resolve_known(*b)?;
            plan.args.push(url.to_owned());
            Ok(plan)
        }
        BrowserChoice::Path(p) => {
            if !p.exists() {
                return Err(format!("{} was not found", p.display()));
            }
            if cfg!(target_os = "macos") && p.extension().is_some_and(|e| e.eq_ignore_ascii_case("app")) {
                let app = p.to_string_lossy();
                return Ok(LaunchPlan::new("open", &["-a", &app, url]));
            }
            Ok(LaunchPlan::new(p, &[url]))
        }
    }
}

fn system_plan(url: &str) -> LaunchPlan {
    if cfg!(target_os = "macos") {
        LaunchPlan::new("open", &[url])
    } else if cfg!(target_os = "windows") {
        LaunchPlan::new("cmd", &["/C", "start", "", url])
    } else {
        LaunchPlan::new("xdg-open", &[url])
    }
}

/// The program that starts `browser` here, without the address.
fn resolve_known(browser: KnownBrowser) -> Result<LaunchPlan, String> {
    let missing = || format!("{} was not found on this computer", browser.name());
    if cfg!(target_os = "macos") {
        let app = format!("{}.app", browser.mac_app());
        let mut roots = vec![PathBuf::from("/Applications"), PathBuf::from("/System/Applications")];
        if let Some(home) = std::env::var_os("HOME") {
            roots.push(PathBuf::from(home).join("Applications"));
        }
        let found = roots.iter().any(|r| r.join(&app).is_dir());
        return if found { Ok(LaunchPlan::new("open", &["-a", browser.mac_app()])) } else { Err(missing()) };
    }
    if browser == KnownBrowser::Safari {
        return Err("Safari is not available on this system".to_owned());
    }
    if cfg!(target_os = "windows") {
        let exe = browser.windows_exe().ok_or_else(missing)?;
        return if windows_app_path_exists(exe) {
            Ok(LaunchPlan::new("cmd", &["/C", "start", "", exe]))
        } else {
            Err(missing())
        };
    }
    browser
        .linux_commands()
        .iter()
        .find_map(|c| find_on_path(c))
        .map(|p| LaunchPlan::new(p, &[]))
        .ok_or_else(missing)
}

/// Is `exe` registered under App Paths (the per-user hive, then the machine's)?
fn windows_app_path_exists(exe: &str) -> bool {
    ["HKCU", "HKLM"].iter().any(|hive| {
        let key = format!(r"{hive}\SOFTWARE\Microsoft\Windows\CurrentVersion\App Paths\{exe}");
        std::process::Command::new("reg")
            .args(["query", &key, "/ve"])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .is_ok_and(|s| s.success())
    })
}

fn find_on_path(command: &str) -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    std::env::split_paths(&path).map(|d| d.join(command)).find(|p| p.is_file())
}

/// Open `url` in the browser `choice` names (WASM-008 R12). With
/// [`NO_BROWSER_ENV`] set the decision is made — the plan resolved, a missing
/// browser still an error — and nothing is started.
pub fn open_in_browser(url: &str, choice: &BrowserChoice) -> Result<(), String> {
    let plan = launch_plan(url, choice)?;
    if std::env::var_os(NO_BROWSER_ENV).is_some() {
        return Ok(());
    }
    let mut cmd = std::process::Command::new(&plan.program);
    cmd.args(&plan.args).stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null());
    let name = if matches!(choice, BrowserChoice::System) { "the system browser".to_owned() } else { choice.label() };
    cmd.spawn().map(|_| ()).map_err(|e| format!("{name} could not be started: {e}"))
}

/// The headless helper, running (R9, R16): nothing of it is on show, and no
/// browser is ever opened in its place.
pub struct PreviewSession {
    child: Option<Child>,
    stdin: Option<ChildStdin>,
    events: Option<Receiver<Event>>,
    closed: bool,
    /// Events that arrived while a headless call waited for its answer.
    kept: Vec<Event>,
}

impl PreviewSession {
    /// Load `url` in the `helper` at a viewport of `width` x `height`. A
    /// helper that is missing or will not start is an error saying so.
    pub fn start(url: &str, width: u32, height: u32, emulation: &Emulation, helper: &Path) -> Result<Self, String> {
        if !is_preview_url(url) {
            return Err("the Preview shows only its own development server".to_owned());
        }
        Self::spawn(helper, url, width, height, emulation).map_err(|e| format!("{}: {e}", helper.display()))
    }

    fn spawn(path: &Path, url: &str, width: u32, height: u32, emulation: &Emulation) -> Result<Self, String> {
        let mut child = std::process::Command::new(path)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| e.to_string())?;
        let stdin = child.stdin.take();
        let stdout = child.stdout.take().ok_or("no output pipe")?;
        let (tx, rx) = mpsc::channel();
        thread::spawn(move || {
            for line in BufReader::new(stdout).lines().map_while(Result::ok) {
                if let Some(event) = Event::parse(&line) {
                    if tx.send(event).is_err() {
                        return;
                    }
                }
            }
            let _ = tx.send(Event::Closed);
        });
        let mut session = Self { child: Some(child), stdin, events: Some(rx), closed: false, kept: Vec::new() };
        let opened = session.send(&Command::open(url, width, height, emulation));
        if opened {
            Ok(session)
        } else {
            session.kill();
            Err("the helper did not take its first command".to_owned())
        }
    }

    /// Send a command. `false` when the helper is gone (the pipe is broken).
    pub fn send(&mut self, command: &Command) -> bool {
        let Some(stdin) = self.stdin.as_mut() else { return false };
        let ok = stdin.write_all(command.to_line().as_bytes()).and_then(|_| stdin.flush()).is_ok();
        if !ok {
            self.stdin = None;
        }
        ok
    }

    /// The events that arrived since the last call. If the helper ended without
    /// saying so, `Closed` is among them: a web view that crashed is reported,
    /// not waited for (WASM-008 AC6).
    pub fn poll(&mut self) -> Vec<Event> {
        let mut out = std::mem::take(&mut self.kept);
        if let Some(rx) = &self.events {
            while let Ok(e) = rx.try_recv() {
                if e == Event::Closed {
                    self.closed = true;
                }
                out.push(e);
            }
        }
        if !self.closed {
            if let Some(child) = self.child.as_mut() {
                if matches!(child.try_wait(), Ok(Some(_))) {
                    self.closed = true;
                    out.push(Event::Closed);
                }
            }
        }
        out
    }

    /// Is the helper still there?
    pub fn alive(&mut self) -> bool {
        self.poll_state();
        self.child.is_some() && !self.closed
    }

    fn poll_state(&mut self) {
        if let Some(child) = self.child.as_mut() {
            if !self.closed && matches!(child.try_wait(), Ok(Some(_))) {
                self.closed = true;
            }
        }
    }

    /// Wait up to `limit` for the first event `pick` accepts. A `Failed` or
    /// `Closed` on the way is the error; every other event is kept for the
    /// next [`poll`](Self::poll).
    fn wait_for(&mut self, limit: std::time::Duration, pick: impl Fn(&Event) -> bool) -> Result<Event, String> {
        let deadline = std::time::Instant::now() + limit;
        loop {
            let left = deadline.saturating_duration_since(std::time::Instant::now());
            let Some(rx) = &self.events else { return Err("the helper is not running".to_owned()) };
            let event = match rx.recv_timeout(left.max(std::time::Duration::from_millis(1))) {
                Ok(e) => e,
                Err(mpsc::RecvTimeoutError::Timeout) => return Err(format!("the helper did not answer within {} s", limit.as_secs())),
                Err(mpsc::RecvTimeoutError::Disconnected) => Event::Closed,
            };
            match event {
                e if pick(&e) => return Ok(e),
                Event::Failed { message } => return Err(message),
                Event::Closed => {
                    self.closed = true;
                    return Err("the helper ended".to_owned());
                }
                e => self.kept.push(e),
            }
        }
    }

    /// Wait for `ready` (the web view is made): what the helper says of its
    /// window's visibility (`None` from an older helper). Headless runs only.
    pub fn ready(&mut self, limit: std::time::Duration) -> Result<Option<bool>, String> {
        match self.wait_for(limit, |e| matches!(e, Event::Ready { .. }))? {
            Event::Ready { visible } => Ok(visible),
            _ => unreachable!("only Ready is picked"),
        }
    }

    /// A PNG of the viewport at `path`; its pixels (WASM-008 T41). The helper
    /// waits for the page's load first.
    pub fn render(&mut self, path: &Path, limit: std::time::Duration) -> Result<(u32, u32), String> {
        if !self.send(&Command::Render { path: path.to_string_lossy().into_owned() }) {
            return Err("the helper is not running".to_owned());
        }
        match self.wait_for(limit, |e| matches!(e, Event::Rendered { .. }))? {
            Event::Rendered { width, height, .. } => Ok((width, height)),
            _ => unreachable!("only Rendered is picked"),
        }
    }

    /// Run `steps` on the page and wait for the answer: (values, text, errors).
    pub fn script(&mut self, steps: Vec<serde_json::Value>, time_limit_s: u32) -> Result<(Vec<serde_json::Value>, String, Vec<String>), String> {
        if !self.send(&Command::Script { steps, time_limit_s: Some(time_limit_s) }) {
            return Err("the helper is not running".to_owned());
        }
        let limit = std::time::Duration::from_secs(u64::from(time_limit_s) + 30);
        match self.wait_for(limit, |e| matches!(e, Event::Scripted { .. }))? {
            Event::Scripted { values, text, errors } => Ok((values, text, errors)),
            _ => unreachable!("only Scripted is picked"),
        }
    }

    /// End the helper now.
    pub fn kill(&mut self) {
        if let Some(mut child) = self.child.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
        self.stdin = None;
        self.closed = true;
    }

    /// Ask the helper to close, then make sure it has.
    pub fn close(&mut self) {
        let _ = self.send(&Command::Close);
        self.kill();
    }
}

impl Drop for PreviewSession {
    fn drop(&mut self) {
        self.kill();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The exact lines the helper reads. `cobolt-web-preview` pins the same
    /// strings on its side; a change to either side fails one of the two.
    #[test]
    fn the_lines_the_ide_writes_are_the_helpers_contract() {
        let cases: Vec<(Command, &str)> = vec![
            (
                Command::open("http://127.0.0.1:8123/", 390, 844, &Emulation::default()),
                r#"{"cmd":"open","url":"http://127.0.0.1:8123/","width":390,"height":844}"#,
            ),
            (
                Command::open(
                    "http://127.0.0.1:8123/",
                    390,
                    844,
                    &Emulation { dpr: Some(3.0), dark: Some(false), locale: Some("pt-BR".into()), safe_area: Some([47, 0, 34, 0]) },
                ),
                r#"{"cmd":"open","url":"http://127.0.0.1:8123/","width":390,"height":844,"dpr":3.0,"dark":false,"locale":"pt-BR"}"#,
            ),
            (Command::Reload, r#"{"cmd":"reload"}"#),
            (Command::Scheme { dark: true }, r#"{"cmd":"scheme","dark":true}"#),
            (
                Command::Emulate { dpr: Some(2.0), locale: Some("pt-BR".into()), safe_area: Some([47, 0, 34, 0]) },
                r#"{"cmd":"emulate","dpr":2.0,"locale":"pt-BR","safe_area":[47,0,34,0]}"#,
            ),
            (Command::Emulate { dpr: None, locale: None, safe_area: Some([0, 0, 0, 0]) }, r#"{"cmd":"emulate","safe_area":[0,0,0,0]}"#),
            (Command::Render { path: "/tmp/a.png".into() }, r#"{"cmd":"render","path":"/tmp/a.png"}"#),
            (
                Command::Script { steps: vec![serde_json::json!({ "click": { "control": "BTN-GO" } })], time_limit_s: Some(20) },
                r#"{"cmd":"script","steps":[{"click":{"control":"BTN-GO"}}],"time_limit_s":20}"#,
            ),
            (Command::Script { steps: vec![], time_limit_s: None }, r#"{"cmd":"script","steps":[]}"#),
            (Command::Close, r#"{"cmd":"close"}"#),
        ];
        for (command, text) in cases {
            assert_eq!(command.to_line(), format!("{text}\n"));
            assert_eq!(serde_json::from_str::<Command>(text).unwrap(), command);
        }
        // Gone with the visible window (WASM-008 T40); `title` on `open` too.
        for gone in [r#"{"cmd":"size","width":820,"height":1180}"#, r#"{"cmd":"devtools"}"#] {
            assert!(serde_json::from_str::<Command>(gone).is_err(), "{gone}");
        }
        let line = Command::open("http://127.0.0.1:8123/", 1, 1, &Emulation::default()).to_line();
        assert!(!line.contains("title"), "{line}");
    }

    #[test]
    fn the_lines_the_helper_writes_are_understood() {
        let console = |text: &str, form: &str| Event::Page(PageEvent::Console { level: "error".into(), text: text.into(), form: form.into() });
        let cases: Vec<(&str, Event)> = vec![
            (r#"{"event":"ready","visible":false}"#, Event::Ready { visible: Some(false) }),
            (r#"{"event":"ready"}"#, Event::Ready { visible: None }),
            (r#"{"event":"console","level":"error","text":"boom","form":"MAIN-FORM"}"#, console("boom", "MAIN-FORM")),
            // A line of an older helper, without `form`, still reads.
            (r#"{"event":"console","level":"error","text":"boom"}"#, console("boom", "")),
            (
                r#"{"event":"error","message":"x is not defined","source":"app.js","line":12,"form":"F"}"#,
                Event::Page(PageEvent::Error { message: "x is not defined".into(), source: "app.js".into(), line: 12, form: "F".into() }),
            ),
            (
                r#"{"event":"cobol-error","form":"F","unit":"BTN-CLICK","line":7,"code":"EC-SIZE","message":"m"}"#,
                Event::Page(PageEvent::CobolError { form: "F".into(), unit: "BTN-CLICK".into(), line: 7, code: "EC-SIZE".into(), message: "m".into() }),
            ),
            (r#"{"event":"updated","how":"stylesheet","ms":12.5}"#, Event::Page(PageEvent::Updated { how: "stylesheet".into(), ms: 12.5, form: String::new() })),
            (r#"{"event":"blocked","url":"https://example.com/"}"#, Event::Blocked { url: "https://example.com/".into() }),
            (r#"{"event":"failed","message":"no web view"}"#, Event::Failed { message: "no web view".into() }),
            (r#"{"event":"closed","extra":1}"#, Event::Closed),
            // `cobolt-web-preview` pins `loading` (WASM-008 T47).
            (r#"{"event":"loading","url":"http://127.0.0.1:8123/"}"#, Event::Loading { url: "http://127.0.0.1:8123/".into() }),
            // The helper's own text (`cobolt-web-preview` pins `rendered` and `scripted`).
            (
                r#"{"event":"rendered","height":2532,"path":"/tmp/a.png","width":1170}"#,
                Event::Rendered { path: "/tmp/a.png".into(), width: 1170, height: 2532 },
            ),
            (
                r#"{"errors":["step 2 (click): no `X` on the page"],"event":"scripted","text":"HELLO","values":["Ana",true]}"#,
                Event::Scripted { values: vec!["Ana".into(), true.into()], text: "HELLO".into(), errors: vec!["step 2 (click): no `X` on the page".into()] },
            ),
            (r#"{"event":"scripted"}"#, Event::Scripted { values: vec![], text: String::new(), errors: vec![] }),
        ];
        for (text, event) in cases {
            assert_eq!(Event::parse(text), Some(event), "{text}");
        }
        assert_eq!(Event::parse("not json"), None);
        assert_eq!(Event::parse(r#"{"event":"unknown"}"#), None);
        assert_eq!(Event::parse(""), None);
    }

    /// WASM-008 R8: the Preview opens its own development server and nothing
    /// else, from the system browser as from the web view.
    #[test]
    fn only_the_preview_s_own_address_is_opened() {
        for ok in ["http://127.0.0.1:8123/", "http://localhost:80/index.html", "http://127.0.0.1:1/?a=b"] {
            assert!(is_preview_url(ok), "{ok}");
        }
        for bad in [
            "https://127.0.0.1:8123/",
            "http://example.com:80/",
            "http://127.0.0.1/",
            "http://127.0.0.1.evil.com:80/",
            "http://user@127.0.0.1:80/",
            "http://127.0.0.1:abc/",
            "file:///etc/passwd",
            "javascript:alert(1)",
            "",
        ] {
            assert!(!is_preview_url(bad), "{bad}");
            assert!(open_in_system_browser(bad).is_err(), "{bad}");
        }
        assert!(PreviewSession::start("https://example.com/", 1, 1, &Emulation::default(), Path::new("/bin/true")).is_err());
    }

    /// WASM-008 T31 (R12, R8): the setting parses into a choice; each known
    /// browser resolves on this system to a program and arguments (or an error
    /// naming it); a non-loopback address is refused for every choice; with
    /// `PRC_PREVIEW_NO_BROWSER` nothing is started.
    #[test]
    fn the_launcher_resolves_each_choice_and_opens_only_the_preview_008() {
        assert_eq!(BrowserChoice::parse(""), BrowserChoice::System);
        assert_eq!(BrowserChoice::parse("  "), BrowserChoice::System);
        for b in KnownBrowser::ALL {
            assert_eq!(BrowserChoice::parse(b.id()), BrowserChoice::Named(b));
            assert_eq!(BrowserChoice::parse(&b.id().to_uppercase()), BrowserChoice::Named(b));
        }
        assert_eq!(BrowserChoice::parse("opera"), BrowserChoice::Path(PathBuf::from("opera")), "an unknown id is a path");
        assert_eq!(BrowserChoice::parse("/usr/bin/x-www"), BrowserChoice::Path(PathBuf::from("/usr/bin/x-www")));

        let url = "http://127.0.0.1:8123/";
        let system = launch_plan(url, &BrowserChoice::System).unwrap();
        assert_eq!(system.args.last().map(String::as_str), Some(url));
        println!("system default -> {} {:?}", system.program.display(), system.args);
        for b in KnownBrowser::ALL {
            match launch_plan(url, &BrowserChoice::Named(b)) {
                Ok(plan) => {
                    assert_eq!(plan.args.last().map(String::as_str), Some(url));
                    assert!(b.is_present());
                    println!("{:<8} -> {} {:?}", b.id(), plan.program.display(), plan.args);
                }
                Err(e) => {
                    assert!(e.contains(b.name()), "{e}");
                    assert!(!b.is_present());
                    println!("{:<8} -> not here: {e}", b.id());
                }
            }
        }
        if !cfg!(target_os = "macos") {
            let e = launch_plan(url, &BrowserChoice::Named(KnownBrowser::Safari)).unwrap_err();
            assert!(e.contains("not available"), "{e}");
        }
        let missing = BrowserChoice::Path(PathBuf::from("/definitely/not/a/browser"));
        let e = launch_plan(url, &missing).unwrap_err();
        assert!(e.contains("/definitely/not/a/browser"), "{e}");
        let exe = std::env::current_exe().unwrap();
        let here = launch_plan(url, &BrowserChoice::Path(exe.clone())).unwrap();
        assert_eq!((here.program, here.args), (exe, vec![url.to_owned()]));

        let choices = [
            BrowserChoice::System,
            BrowserChoice::Named(KnownBrowser::Firefox),
            BrowserChoice::Path(std::env::current_exe().unwrap()),
        ];
        for bad in ["https://127.0.0.1:8123/", "http://example.com:80/", "file:///etc/passwd", "javascript:alert(1)", ""] {
            for c in &choices {
                assert!(launch_plan(bad, c).is_err(), "{bad} with {c:?}");
                assert!(open_in_browser(bad, c).is_err(), "{bad} with {c:?}");
            }
        }

        // The switch decides without starting: the test's own executable as
        // the "browser" would run this test binary again if it were spawned.
        std::env::set_var(NO_BROWSER_ENV, "1");
        let before = std::time::Instant::now();
        assert!(open_in_browser(url, &BrowserChoice::Path(std::env::current_exe().unwrap())).is_ok());
        assert!(open_in_browser(url, &BrowserChoice::System).is_ok());
        assert!(open_in_browser(url, &missing).is_err(), "a missing browser is still an error with the switch set");
        println!("NO_BROWSER_ENV: decided in {:?}, nothing started", before.elapsed());
    }

    /// A helper that is not there is an error naming it — never a browser
    /// opened in its place (R16).
    #[test]
    fn a_missing_helper_is_an_error_not_a_browser() {
        let nothing = PathBuf::from("/definitely/not/a/helper");
        let e = PreviewSession::start("http://127.0.0.1:1/", 100, 100, &Emulation::default(), &nothing).err().expect("an error");
        assert!(e.contains("/definitely/not/a/helper"), "{e}");
    }

    /// A helper process that speaks the protocol: events arrive, commands
    /// reach it, and when it dies the session says so (WASM-008 AC4, AC6).
    #[cfg(unix)]
    #[test]
    fn a_session_relays_events_and_notices_a_helper_that_died() {
        use std::os::unix::fs::PermissionsExt;
        let dir = std::env::temp_dir().join(format!("prc-preview-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        // A stand-in for the helper: it says ready, echoes a console line for
        // the first command, then waits to be told to close.
        let helper = dir.join("fake-helper");
        std::fs::write(
            &helper,
            "#!/bin/sh\nread first\necho '{\"event\":\"ready\"}'\necho '{\"event\":\"console\",\"level\":\"error\",\"text\":\"from the page\"}'\n\
             echo '{\"event\":\"error\",\"message\":\"uncaught\",\"source\":\"app.js\",\"line\":3}'\nread second\nexit 7\n",
        )
        .unwrap();
        std::fs::set_permissions(&helper, std::fs::Permissions::from_mode(0o755)).unwrap();

        let mut s = PreviewSession::spawn(&helper, "http://127.0.0.1:9/", 390, 844, &Emulation::default()).unwrap();
        let mut got = Vec::new();
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(30);
        while got.len() < 3 && std::time::Instant::now() < deadline {
            got.extend(s.poll());
            thread::sleep(std::time::Duration::from_millis(10));
        }
        assert_eq!(got[0], Event::Ready { visible: None });
        assert_eq!(got[1], Event::Page(PageEvent::Console { level: "error".into(), text: "from the page".into(), form: String::new() }));
        assert_eq!(got[2], Event::Page(PageEvent::Error { message: "uncaught".into(), source: "app.js".into(), line: 3, form: String::new() }));
        assert!(s.alive());
        // A second command reaches the helper, which then ends (as a crash would).
        assert!(s.send(&Command::Reload));
        let mut closed = false;
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(30);
        while !closed && std::time::Instant::now() < deadline {
            closed = s.poll().contains(&Event::Closed);
            thread::sleep(std::time::Duration::from_millis(10));
        }
        assert!(closed, "a helper that ended is reported as closed");
        assert!(!s.alive());
        // Sending to a dead helper does not panic or hang; it says no.
        let mut any_failed = false;
        for _ in 0..5 {
            any_failed |= !s.send(&Command::Reload);
        }
        assert!(any_failed, "writes to a dead helper fail");
        let _ = std::fs::remove_dir_all(&dir);
        println!("preview: a stand-in helper relayed ready, a console line and an uncaught error; its end was reported as Closed");
    }
}
