// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! A real Chrome driven through the DevTools protocol (WASM-016 T22, the operator's decision of
//! 2026-10-23): the browser is started headless with a profile of its own in a temporary folder,
//! one page is opened per run, and the test talks to it over the page's WebSocket — evaluate an
//! expression, press the mouse at a point, insert text, change the viewport — reading the console
//! as it goes. The client is a few calls over `tungstenite`, which the tests already use; no
//! browser-automation crate enters either workspace.
//!
//! Chrome is found at `CHROME` (when set) or where it installs on macOS, Linux and Windows. With
//! none, [`find_chrome`] answers `None` and the test says it was skipped and why.

use std::io::{Read, Write};
use std::net::TcpStream;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use serde_json::{json, Value};
use tungstenite::{Message, WebSocket};

/// The Chrome to run: `CHROME`, else the usual install places.
pub fn find_chrome() -> Option<PathBuf> {
    if let Some(p) = std::env::var_os("CHROME").map(PathBuf::from) {
        return p.is_file().then_some(p);
    }
    let mut candidates: Vec<PathBuf> = vec![
        "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome".into(),
        "/Applications/Chromium.app/Contents/MacOS/Chromium".into(),
    ];
    for var in ["ProgramFiles", "ProgramFiles(x86)", "LocalAppData"] {
        if let Some(base) = std::env::var_os(var) {
            candidates.push(Path::new(&base).join("Google/Chrome/Application/chrome.exe"));
        }
    }
    if let Some(path) = std::env::var_os("PATH") {
        for dir in std::env::split_paths(&path) {
            for name in ["google-chrome", "google-chrome-stable", "chromium", "chromium-browser"] {
                candidates.push(dir.join(name));
            }
        }
    }
    candidates.into_iter().find(|p| p.is_file())
}

/// A headless Chrome with a profile of its own; killed (this process only) when dropped.
pub struct Chrome {
    child: Child,
    port: u16,
    _profile: tempfile::TempDir,
}

impl Drop for Chrome {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

impl Chrome {
    pub fn launch(exe: &Path) -> Result<Chrome, String> {
        let profile = tempfile::tempdir().map_err(|e| e.to_string())?;
        let child = Command::new(exe)
            .args(["--headless=new", "--remote-debugging-port=0", "--no-first-run", "--no-default-browser-check", "--disable-extensions", "--mute-audio"])
            .arg(format!("--user-data-dir={}", profile.path().display()))
            .arg("about:blank")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| format!("cannot start {}: {e}", exe.display()))?;
        // Chrome writes the port it chose into the profile.
        let file = profile.path().join("DevToolsActivePort");
        let deadline = Instant::now() + Duration::from_secs(30);
        let port = loop {
            if let Some(p) = std::fs::read_to_string(&file).ok().and_then(|t| t.lines().next().and_then(|l| l.trim().parse::<u16>().ok())) {
                break p;
            }
            if Instant::now() > deadline {
                return Err("Chrome did not open its DevTools port within 30 s".into());
            }
            std::thread::sleep(Duration::from_millis(50));
        };
        Ok(Chrome { child, port, _profile: profile })
    }

    /// A new page (tab), with the console and the page domains on.
    pub fn new_page(&self) -> Result<Page, String> {
        let answer = http(self.port, "PUT", "/json/new?about:blank")?;
        let v: Value = serde_json::from_str(&answer).map_err(|e| format!("/json/new answered no JSON ({e}): {answer}"))?;
        let ws_url = v["webSocketDebuggerUrl"].as_str().ok_or_else(|| format!("no webSocketDebuggerUrl in {v}"))?.to_owned();
        let stream = TcpStream::connect(("127.0.0.1", self.port)).map_err(|e| e.to_string())?;
        stream.set_read_timeout(Some(Duration::from_secs(30))).map_err(|e| e.to_string())?;
        let (ws, _) = tungstenite::client::client(ws_url.as_str(), stream).map_err(|e| format!("DevTools handshake: {e}"))?;
        let mut page = Page { ws, next: 1, console: Vec::new(), exceptions: Vec::new() };
        page.call("Runtime.enable", json!({}))?;
        page.call("Page.enable", json!({}))?;
        Ok(page)
    }
}

/// One request to Chrome's small HTTP endpoint.
fn http(port: u16, method: &str, path: &str) -> Result<String, String> {
    let mut s = TcpStream::connect(("127.0.0.1", port)).map_err(|e| e.to_string())?;
    s.set_read_timeout(Some(Duration::from_secs(30))).map_err(|e| e.to_string())?;
    write!(s, "{method} {path} HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nContent-Length: 0\r\nConnection: close\r\n\r\n").map_err(|e| e.to_string())?;
    // Chrome may keep the connection open: read the head, then exactly `Content-Length` bytes.
    let mut buf = Vec::new();
    let mut chunk = [0u8; 4096];
    loop {
        let text = String::from_utf8_lossy(&buf).into_owned();
        if let Some((head, body)) = text.split_once("\r\n\r\n") {
            let len = head.lines().find_map(|l| l.split_once(':').filter(|(k, _)| k.eq_ignore_ascii_case("content-length")).and_then(|(_, v)| v.trim().parse::<usize>().ok()));
            if len.is_some_and(|n| body.len() >= n) {
                return Ok(body.to_owned());
            }
        }
        let n = s.read(&mut chunk).map_err(|e| format!("{method} {path}: {e}"))?;
        if n == 0 {
            let text = String::from_utf8_lossy(&buf).into_owned();
            return Ok(text.split_once("\r\n\r\n").map(|(_, b)| b.to_owned()).unwrap_or(text));
        }
        buf.extend_from_slice(&chunk[..n]);
    }
}

pub struct Page {
    ws: WebSocket<TcpStream>,
    next: u64,
    /// Every `console.*` call of the page, as text (`log: go`).
    pub console: Vec<String>,
    /// Uncaught exceptions the page reported.
    pub exceptions: Vec<String>,
}

impl Drop for Page {
    fn drop(&mut self) {
        let _ = self.call("Page.close", json!({}));
    }
}

impl Page {
    /// Send one DevTools command and wait for its answer, keeping the events that arrive meanwhile.
    pub fn call(&mut self, method: &str, params: Value) -> Result<Value, String> {
        let id = self.next;
        self.next += 1;
        self.ws.send(Message::text(json!({"id": id, "method": method, "params": params}).to_string())).map_err(|e| format!("{method}: {e}"))?;
        loop {
            let msg = self.ws.read().map_err(|e| format!("{method}: {e}"))?;
            let Message::Text(t) = msg else { continue };
            let v: Value = serde_json::from_str(t.as_str()).map_err(|e| e.to_string())?;
            match v["method"].as_str() {
                Some("Runtime.consoleAPICalled") => {
                    let args: Vec<String> = v["params"]["args"].as_array().map(|a| a.iter().map(|x| x["value"].as_str().map(str::to_owned).unwrap_or_else(|| x["description"].as_str().unwrap_or("").to_owned())).collect()).unwrap_or_default();
                    self.console.push(format!("{}: {}", v["params"]["type"].as_str().unwrap_or("?"), args.join(" ")));
                }
                Some("Runtime.exceptionThrown") => {
                    let d = &v["params"]["exceptionDetails"];
                    self.exceptions.push(d["exception"]["description"].as_str().or(d["text"].as_str()).unwrap_or("?").to_owned());
                }
                _ => {}
            }
            if v["id"].as_u64() == Some(id) {
                if let Some(e) = v.get("error") {
                    return Err(format!("{method}: {e}"));
                }
                return Ok(v["result"].clone());
            }
        }
    }

    /// Evaluate `expr` in the page (awaiting a promise) and answer its value.
    pub fn eval(&mut self, expr: &str) -> Result<Value, String> {
        let r = self.call("Runtime.evaluate", json!({"expression": expr, "awaitPromise": true, "returnByValue": true, "userGesture": true}))?;
        if let Some(d) = r.get("exceptionDetails") {
            let msg = d["exception"]["description"].as_str().or(d["text"].as_str()).unwrap_or("?");
            return Err(format!("`{}` threw: {msg}", short(expr)));
        }
        Ok(r["result"]["value"].clone())
    }

    /// Evaluate `expr` until it answers something truthy, at most `secs` seconds.
    pub fn wait_for(&mut self, expr: &str, secs: u64) -> Result<Value, String> {
        let deadline = Instant::now() + Duration::from_secs(secs);
        loop {
            let v = self.eval(expr)?;
            let truthy = !(v.is_null() || v == json!(false) || v == json!("") || v == json!(0));
            if truthy {
                return Ok(v);
            }
            if Instant::now() > deadline {
                return Err(format!("waited {secs} s for `{}`", short(expr)));
            }
            std::thread::sleep(Duration::from_millis(25));
        }
    }

    pub fn navigate(&mut self, url: &str) -> Result<(), String> {
        self.call("Page.navigate", json!({"url": url}))?;
        self.wait_for("document.readyState === 'complete'", 30)?;
        Ok(())
    }

    /// Press and release the left button at the centre of what `rect_expr` answers (`{x, y, width, height}`
    /// in the viewport), as a user's click: the browser hit-tests it.
    pub fn click_at(&mut self, rect_expr: &str) -> Result<(), String> {
        let r = self.eval(rect_expr)?;
        let (Some(x), Some(y), Some(w), Some(h)) = (r["x"].as_f64(), r["y"].as_f64(), r["width"].as_f64(), r["height"].as_f64()) else {
            return Err(format!("no rectangle from `{}`: {r}", short(rect_expr)));
        };
        let (cx, cy) = (x + w / 2.0, y + h / 2.0);
        self.call("Input.dispatchMouseEvent", json!({"type": "mouseMoved", "x": cx, "y": cy}))?;
        self.call("Input.dispatchMouseEvent", json!({"type": "mousePressed", "x": cx, "y": cy, "button": "left", "buttons": 1, "clickCount": 1}))?;
        self.call("Input.dispatchMouseEvent", json!({"type": "mouseReleased", "x": cx, "y": cy, "button": "left", "buttons": 0, "clickCount": 1}))?;
        Ok(())
    }

    /// Type `text` into what has the focus, as a keyboard would (an `input` event per insertion).
    pub fn type_text(&mut self, text: &str) -> Result<(), String> {
        self.call("Input.insertText", json!({"text": text}))?;
        Ok(())
    }

    /// The viewport's size, as a window resized by the user.
    pub fn resize(&mut self, width: u32, height: u32) -> Result<(), String> {
        self.call("Emulation.setDeviceMetricsOverride", json!({"width": width, "height": height, "deviceScaleFactor": 1, "mobile": false}))?;
        Ok(())
    }
}

fn short(s: &str) -> String {
    let s = s.split_whitespace().collect::<Vec<_>>().join(" ");
    if s.chars().count() > 120 {
        format!("{}…", s.chars().take(120).collect::<String>())
    } else {
        s
    }
}
