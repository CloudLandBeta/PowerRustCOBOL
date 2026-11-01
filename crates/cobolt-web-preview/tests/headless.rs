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
    let listener = TcpListener::bind(("127.0.0.1", 0)).unwrap();
    let port = listener.local_addr().unwrap().port();
    std::thread::spawn(move || {
        for stream in listener.incoming().flatten() {
            let mut stream = stream;
            let mut buf = [0u8; 4096];
            let _ = stream.read(&mut buf);
            let body = "<!doctype html><html><body><p id=x>headless</p></body></html>";
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
