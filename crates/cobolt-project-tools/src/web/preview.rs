// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! **The web view of the Preview, as the IDE talks to it** (WASM-008 R1, R2,
//! R5, R8).
//!
//! The system web view (WKWebView, WebView2, WebKitGTK) lives in a **helper
//! process**, `prc-web-preview` (crate `cobolt-web-preview`), in a window of its
//! own — the way `rcrun run-form` holds a form. A web view that hangs or
//! crashes takes the helper down and nothing else (WASM-008 R2, AC6), and the
//! window is the operating system's, which no layout of ours can resize.
//!
//! The two talk in **JSON lines**: the IDE writes [`Command`]s to the helper's
//! standard input and reads [`Event`]s from its standard output. The helper is
//! built from the same definitions' *text* — it depends on `serde_json` and
//! nothing of ours — so `tests` here pin every line either side writes.
//!
//! When the helper is not installed (or does not start), [`PreviewSession`]
//! falls back to what is always there: the system browser, opened on the same
//! served bundle. The developer loses the console in the Output panel and the
//! device presets, never the Preview.

use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, Stdio};
use std::sync::mpsc::{self, Receiver};
use std::thread;

use serde::{Deserialize, Serialize};

/// The helper's file name.
#[cfg(windows)]
pub const HELPER_NAME: &str = "prc-web-preview.exe";
/// The helper's file name.
#[cfg(not(windows))]
pub const HELPER_NAME: &str = "prc-web-preview";

/// The environment variable that names the helper (a build tree, a test).
pub const HELPER_ENV: &str = "PRC_WEB_PREVIEW";

/// What the IDE asks of the helper.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "cmd", rename_all = "snake_case")]
pub enum Command {
    /// Open the window on `url`, `width` x `height` points. Only the origin
    /// of `url` may be navigated to afterwards (WASM-008 R8). The emulation the
    /// developer already chose rides along, so the page loads once, not once
    /// per setting.
    Open {
        url: String,
        title: String,
        width: u32,
        height: u32,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        dpr: Option<f32>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        dark: Option<bool>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        locale: Option<String>,
        /// top, right, bottom, left — CSS pixels.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        safe_area: Option<[u32; 4]>,
    },
    /// Resize the window — the developer picked a preset or an orientation.
    Size { width: u32, height: u32 },
    /// Load the page again.
    Reload,
    /// Prefer the dark or the light colour scheme.
    Scheme { dark: bool },
    /// The language the page sees (`navigator.language`).
    Locale { tag: String },
    /// The `devicePixelRatio` the page reads.
    Dpr { value: f32 },
    /// The insets `--prc-safe-area-*` carry, in CSS pixels.
    SafeArea { top: u32, right: u32, bottom: u32, left: u32 },
    /// Open the web view's developer tools.
    Devtools,
    /// Close the window and end.
    Close,
}

/// What the helper tells the IDE.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "event", rename_all = "snake_case")]
pub enum Event {
    /// The window is open and the page is loading.
    Ready,
    /// The page wrote to its console.
    Console { level: String, text: String },
    /// An uncaught error or an unhandled promise rejection.
    Error { message: String, source: String, line: u32 },
    /// The page tried to go to `url`, which is not its own origin; it did not.
    Blocked { url: String },
    /// The helper could not show a web view; `message` says why.
    Failed { message: String },
    /// The window was closed, or the helper ended.
    Closed,
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
    /// The command that opens the window.
    pub fn open(url: &str, title: &str, width: u32, height: u32, emulation: &Emulation) -> Command {
        Command::Open {
            url: url.to_owned(),
            title: title.to_owned(),
            width,
            height,
            dpr: emulation.dpr,
            dark: emulation.dark,
            locale: emulation.locale.clone(),
            safe_area: emulation.safe_area,
        }
    }

    /// One line of JSON, newline-terminated.
    pub fn to_line(&self) -> String {
        let mut s = serde_json::to_string(self).expect("a command serialises");
        s.push('\n');
        s
    }
}

impl Event {
    /// The event a line carries, or `None` for a line that is not one.
    pub fn parse(line: &str) -> Option<Event> {
        serde_json::from_str(line.trim()).ok()
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

/// How the Preview is shown.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    /// In the system web view of the helper process.
    WebView,
    /// In the system browser: the helper is not installed or did not start.
    SystemBrowser,
}

/// The Preview, running.
pub struct PreviewSession {
    mode: Mode,
    child: Option<Child>,
    stdin: Option<ChildStdin>,
    events: Option<Receiver<Event>>,
    closed: bool,
}

impl PreviewSession {
    /// Show `url`. With a `helper`, its window opens at `width` x `height`;
    /// without one, or if it will not start, the system browser opens the page.
    /// The mode that resulted is [`PreviewSession::mode`].
    pub fn start(
        url: &str,
        title: &str,
        width: u32,
        height: u32,
        emulation: &Emulation,
        helper: Option<&Path>,
    ) -> Result<Self, String> {
        if !is_preview_url(url) {
            return Err("the Preview shows only its own development server".to_owned());
        }
        if let Some(path) = helper {
            if let Ok(session) = Self::spawn(path, url, title, width, height, emulation) {
                return Ok(session);
            }
        }
        open_in_system_browser(url)?;
        Ok(Self { mode: Mode::SystemBrowser, child: None, stdin: None, events: None, closed: false })
    }

    fn spawn(
        path: &Path,
        url: &str,
        title: &str,
        width: u32,
        height: u32,
        emulation: &Emulation,
    ) -> Result<Self, String> {
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
        let mut session =
            Self { mode: Mode::WebView, child: Some(child), stdin, events: Some(rx), closed: false };
        let opened = session.send(&Command::open(url, title, width, height, emulation));
        if opened {
            Ok(session)
        } else {
            session.kill();
            Err("the helper did not take its first command".to_owned())
        }
    }

    pub fn mode(&self) -> Mode {
        self.mode
    }

    /// Send a command. `false` when the helper is gone (the pipe is broken) or
    /// there is none: in the system-browser mode a command has nowhere to go.
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
        let mut out = Vec::new();
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

    /// Is the web view still there? (The system-browser mode has no window of
    /// ours to ask about, so it reports `false`.)
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
                Command::open("http://127.0.0.1:8123/", "P", 390, 844, &Emulation::default()),
                r#"{"cmd":"open","url":"http://127.0.0.1:8123/","title":"P","width":390,"height":844}"#,
            ),
            (
                Command::open(
                    "http://127.0.0.1:8123/",
                    "P",
                    390,
                    844,
                    &Emulation { dpr: Some(3.0), dark: Some(true), locale: Some("pt-BR".into()), safe_area: Some([47, 0, 34, 0]) },
                ),
                r#"{"cmd":"open","url":"http://127.0.0.1:8123/","title":"P","width":390,"height":844,"dpr":3.0,"dark":true,"locale":"pt-BR","safe_area":[47,0,34,0]}"#,
            ),
            (Command::Size { width: 820, height: 1180 }, r#"{"cmd":"size","width":820,"height":1180}"#),
            (Command::Reload, r#"{"cmd":"reload"}"#),
            (Command::Scheme { dark: true }, r#"{"cmd":"scheme","dark":true}"#),
            (Command::Locale { tag: "pt-BR".into() }, r#"{"cmd":"locale","tag":"pt-BR"}"#),
            (Command::Dpr { value: 2.0 }, r#"{"cmd":"dpr","value":2.0}"#),
            (
                Command::SafeArea { top: 47, right: 0, bottom: 34, left: 0 },
                r#"{"cmd":"safe_area","top":47,"right":0,"bottom":34,"left":0}"#,
            ),
            (Command::Devtools, r#"{"cmd":"devtools"}"#),
            (Command::Close, r#"{"cmd":"close"}"#),
        ];
        for (command, text) in cases {
            assert_eq!(command.to_line(), format!("{text}\n"));
            assert_eq!(serde_json::from_str::<Command>(text).unwrap(), command);
        }
    }

    #[test]
    fn the_lines_the_helper_writes_are_understood() {
        let cases: Vec<(&str, Event)> = vec![
            (r#"{"event":"ready"}"#, Event::Ready),
            (r#"{"event":"console","level":"error","text":"boom"}"#, Event::Console { level: "error".into(), text: "boom".into() }),
            (
                r#"{"event":"error","message":"x is not defined","source":"app.js","line":12}"#,
                Event::Error { message: "x is not defined".into(), source: "app.js".into(), line: 12 },
            ),
            (r#"{"event":"blocked","url":"https://example.com/"}"#, Event::Blocked { url: "https://example.com/".into() }),
            (r#"{"event":"failed","message":"no web view"}"#, Event::Failed { message: "no web view".into() }),
            (r#"{"event":"closed"}"#, Event::Closed),
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
        assert!(PreviewSession::start("https://example.com/", "x", 1, 1, &Emulation::default(), None).is_err());
    }

    /// A helper that is not there, or will not start, leaves the system browser.
    #[test]
    fn a_missing_helper_means_the_system_browser_not_a_failure() {
        // `start` with a helper that cannot run falls through to the browser; to
        // keep the test from opening one, check the helper's own failure.
        let nothing = PathBuf::from("/definitely/not/a/helper");
        assert!(PreviewSession::spawn(&nothing, "http://127.0.0.1:1/", "t", 100, 100, &Emulation::default()).is_err());
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

        let mut s = PreviewSession::spawn(&helper, "http://127.0.0.1:9/", "T", 390, 844, &Emulation::default()).unwrap();
        assert_eq!(s.mode(), Mode::WebView);
        let mut got = Vec::new();
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        while got.len() < 3 && std::time::Instant::now() < deadline {
            got.extend(s.poll());
            thread::sleep(std::time::Duration::from_millis(10));
        }
        assert_eq!(got[0], Event::Ready);
        assert_eq!(got[1], Event::Console { level: "error".into(), text: "from the page".into() });
        assert_eq!(got[2], Event::Error { message: "uncaught".into(), source: "app.js".into(), line: 3 });
        assert!(s.alive());
        // A second command reaches the helper, which then ends (as a crash would).
        assert!(s.send(&Command::Reload));
        let mut closed = false;
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
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
