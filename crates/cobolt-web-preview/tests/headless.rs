// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! The built helper, driven as the tools drive it (WASM-008 T40, R16, AC15):
//! its web view opens with no window on show, the commands of the visible
//! window are refused, an unknown one is ignored, and `close` ends it.
//!
//! Without a web view on this system the helper says `failed`; the test then
//! prints why and passes, unless `PRC_REQUIRE_WEBVIEW=1` (the CI switch).

use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpListener;
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::time::Duration;

/// A loopback server answering every request with one small page.
fn serve_a_page() -> u16 {
    serve("<!doctype html><html><body><p id=x>headless</p></body></html>")
}

/// A loopback server answering every request with `body`.
fn serve(body: &'static str) -> u16 {
    let listener = TcpListener::bind(("127.0.0.1", 0)).unwrap();
    let port = listener.local_addr().unwrap().port();
    std::thread::spawn(move || {
        for stream in listener.incoming().flatten() {
            let mut stream = stream;
            let mut buf = [0u8; 4096];
            let _ = stream.read(&mut buf);
            let _ = write!(stream, "HTTP/1.1 200 OK\r\nContent-Type: text/html\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len());
        }
    });
    port
}

#[test]
fn the_helper_opens_with_no_window_on_show_and_refuses_the_window_commands() {
    let port = serve_a_page();
    let mut child = Command::new(env!("CARGO_BIN_EXE_prc-web-preview"))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("the helper starts");
    let mut stdin = child.stdin.take().unwrap();
    let stdout = child.stdout.take().unwrap();
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        for line in BufReader::new(stdout).lines().map_while(Result::ok) {
            if tx.send(line).is_err() {
                return;
            }
        }
    });
    let next = || {
        rx.recv_timeout(Duration::from_secs(30))
            .expect("a line from the helper within 30 s")
    };
    let mut send = |line: &str| {
        writeln!(stdin, "{line}")
            .and_then(|_| stdin.flush())
            .expect("the helper reads")
    };

    send(&format!(
        r#"{{"cmd":"open","url":"http://127.0.0.1:{port}/","width":390,"height":844,"dpr":3.0,"dark":false,"locale":"pt-BR"}}"#
    ));
    let first = next();
    if first.starts_with(r#"{"event":"failed""#) {
        let _ = child.kill();
        assert!(
            std::env::var("PRC_REQUIRE_WEBVIEW").as_deref() != Ok("1"),
            "PRC_REQUIRE_WEBVIEW=1 and no web view: {first}"
        );
        println!("skipped: no web view on this system ({first})");
        return;
    }
    assert_eq!(
        first, r#"{"event":"ready","visible":false}"#,
        "the helper's window is not on show"
    );
    send(r#"{"cmd":"size","width":820,"height":1180}"#);
    assert_eq!(
        next(),
        r#"{"event":"failed","message":"`size` is not a command of the headless helper"}"#
    );
    send(r#"{"cmd":"devtools"}"#);
    assert_eq!(
        next(),
        r#"{"event":"failed","message":"`devtools` is not a command of the headless helper"}"#
    );
    // An unknown command is ignored: the next line is the answer to `close`.
    send(r#"{"cmd":"something-new","x":1}"#);
    send(r#"{"cmd":"close"}"#);
    let mut last = next();
    while !last.starts_with(r#"{"event":"closed""#) {
        // Only the page's own events may come between (none from this page).
        assert!(!last.contains(r#""event":"failed""#), "{last}");
        last = next();
    }
    let status = child.wait().unwrap();
    println!("helper: ready with its window not visible; size and devtools refused; an unknown command ignored; closed ({status})");
}

/// A page shaped as the player draws a form (`data-name` on each control,
/// `data-layer` on a layer's controls, all in `#pr-app`'s shadow root, drawn after
/// the load): typing and a click change a label, the
/// click says something on the console, and a layer is hidden.
const FORM_PAGE: &str = r#"<!doctype html><html><head><style>
html, body { margin: 0; background: rgb(10, 120, 200); }
</style></head><body><div id="pr-app"></div><script>
// Drawn after the load, into an open shadow root of #pr-app, as the player draws a form.
setTimeout(function () {
  var root = document.getElementById('pr-app').attachShadow({ mode: 'open' });
  root.innerHTML = '<style>#LBL-OUT { color: rgb(255, 0, 0); }</style><section class="pr-form-root">'
    + '<div data-name="TXT-NAME" id="TXT-NAME"><input type="text"></div>'
    + '<button data-name="BTN-GO" id="BTN-GO" type="button">Go</button>'
    + '<span data-name="LBL-OUT" id="LBL-OUT">-</span>'
    + '<div data-name="PNL-L" data-layer="LAYER-1" hidden inert>layer</div></section>';
  var typed = '';
  root.querySelector('#TXT-NAME input').addEventListener('input', function (e) { typed = e.target.value; });
  root.getElementById('BTN-GO').addEventListener('click', function () {
    root.getElementById('LBL-OUT').textContent = 'Hello, ' + typed;
    console.log('CLICKED ' + typed);
  });
}, 300);
</script></body></html>"#;

/// The helper's own lines, a command at a time.
struct Helper {
    child: std::process::Child,
    stdin: std::process::ChildStdin,
    rx: mpsc::Receiver<String>,
}

impl Helper {
    /// The built helper, opened on `page`; `None` (and why) without a web view.
    fn open(page: &'static str, width: u32, height: u32, dpr: f64) -> Option<Helper> {
        let port = serve(page);
        let mut child = Command::new(env!("CARGO_BIN_EXE_prc-web-preview"))
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .expect("the helper starts");
        let stdin = child.stdin.take().unwrap();
        let stdout = child.stdout.take().unwrap();
        let (tx, rx) = mpsc::channel();
        std::thread::spawn(move || {
            for line in BufReader::new(stdout).lines().map_while(Result::ok) {
                if tx.send(line).is_err() {
                    return;
                }
            }
        });
        let mut h = Helper { child, stdin, rx };
        h.send(&format!(r#"{{"cmd":"open","url":"http://127.0.0.1:{port}/","width":{width},"height":{height},"dpr":{dpr}}}"#));
        let first = h.next();
        if first.starts_with(r#"{"event":"failed""#) {
            let _ = h.child.kill();
            assert!(std::env::var("PRC_REQUIRE_WEBVIEW").as_deref() != Ok("1"), "PRC_REQUIRE_WEBVIEW=1 and no web view: {first}");
            println!("skipped: no web view on this system ({first})");
            return None;
        }
        assert_eq!(first, r#"{"event":"ready","visible":false}"#);
        Some(h)
    }

    fn send(&mut self, line: &str) {
        writeln!(self.stdin, "{line}").and_then(|_| self.stdin.flush()).expect("the helper reads");
    }

    fn next(&self) -> String {
        self.rx.recv_timeout(Duration::from_secs(40)).expect("a line from the helper within 40 s")
    }

    /// The next line that is the answer `event` (or `failed`), as JSON.
    fn answer(&self, event: &str) -> serde_json::Value {
        loop {
            let v: serde_json::Value = serde_json::from_str(&self.next()).unwrap();
            if v["event"] == event || v["event"] == "failed" {
                return v;
            }
        }
    }

    fn close(mut self) {
        self.send(r#"{"cmd":"close"}"#);
        let _ = self.answer("closed");
        let _ = self.child.wait();
    }
}

#[test]
fn a_script_types_clicks_waits_and_reads_through_real_dom_events() {
    let Some(mut h) = Helper::open(FORM_PAGE, 390, 844, 2.0) else { return };
    let started = std::time::Instant::now();
    // Sent at once, before the page has loaded: the script waits for the load.
    h.send(&
        r#"{"cmd":"script","steps":[
        {"wait":{"control":"form"}},
        {"get":{"control":"layer:LAYER-1","property":"Visible"}},
        {"type":{"control":"TXT-NAME","text":"Ana"}},
        {"click":{"control":"BTN-GO"}},
        {"wait":{"ms":50}},
        {"get":{"control":"LBL-OUT","property":"Text"}},
        {"get":{"control":"TXT-NAME","property":"Text"}},
        {"style":{"control":"LBL-OUT","property":"color"}},
        {"rect":{"control":"BTN-GO"}},
        {"focused":{}},
        {"click":{"control":"NOT-THERE"}},
        {"snapshot":{}}]}"#
            .replace('\n', ""),
    );
    let v = h.answer("scripted");
    let ms = started.elapsed().as_millis();
    assert_eq!(v["event"], "scripted", "{v}");
    let values = v["values"].as_array().unwrap();
    assert_eq!(values[0], serde_json::json!(false), "the layer starts hidden: {v}");
    assert_eq!(values[1], "Hello, Ana", "the click saw what was typed: {v}");
    assert_eq!(values[2], "Ana");
    assert_eq!(values[3], "rgb(255, 0, 0)");
    assert!(values[4]["width"].as_f64().unwrap() > 0.0, "{v}");
    assert_eq!(values[5], "BTN-GO", "the click focused the button: {v}");
    assert_eq!(v["text"], "CLICKED Ana", "what the page said during the script: {v}");
    let errors: Vec<&str> = v["errors"].as_array().unwrap().iter().filter_map(|e| e.as_str()).collect();
    assert_eq!(errors.len(), 2, "{errors:?}");
    assert!(errors[0].starts_with("step 11 (click): no `NOT-THERE` on the page"), "{errors:?}");
    assert!(errors[1].starts_with("step 12 (snapshot): "), "{errors:?}");

    // A step that is not one is refused before anything runs.
    h.send(r#"{"cmd":"script","steps":[{"eval":"document.body.remove()"}]}"#);
    assert_eq!(h.answer("failed")["message"], "step 1: `eval` is not a step");
    h.close();
    println!("script: waited for the form, typed, clicked, waited, read 6 values and 2 step errors in {ms} ms (from open, page load included)");
}

#[test]
fn a_render_is_the_viewport_at_the_asked_pixel_ratio() {
    let Some(mut h) = Helper::open(FORM_PAGE, 390, 844, 3.0) else { return };
    let dir = std::env::temp_dir().join(format!("prc-web-preview-render-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("form.png");
    let started = std::time::Instant::now();
    h.send(&serde_json::json!({ "cmd": "render", "path": path.to_string_lossy() }).to_string());
    let v = h.answer("rendered");
    let ms = started.elapsed().as_millis();
    if v["event"] == "failed" {
        let _ = std::fs::remove_dir_all(&dir);
        panic!("the engine took no picture: {v}");
    }
    assert_eq!((v["width"].as_u64(), v["height"].as_u64()), (Some(390 * 3), Some(844 * 3)), "{v}");
    let bytes = std::fs::read(&path).unwrap();
    assert_eq!(&bytes[..8], b"\x89PNG\r\n\x1a\n");
    // Not a blank picture: the page's blue background is in it (the engine
    // drew the page); a PNG of one flat colour compresses to almost nothing.
    assert!(bytes.len() > 2_000, "{} bytes", bytes.len());
    h.close();
    let _ = std::fs::remove_dir_all(&dir);
    println!("render: {}x{} PNG, {} bytes, in {ms} ms (from open, page load included)", 390 * 3, 844 * 3, bytes.len());
}
