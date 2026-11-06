// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! The development backend and the page's origin (WASM-013 T9, R12): told the Preview's origin it accepts that
//! origin and no other port of the same machine; told nothing it keeps accepting any origin (and warns), so a
//! caller that has not been migrated still runs.

use cobolt_server::dev::{DevOptions, DevServer};
use cobolt_server::testkit::backend_dir;
use tokio_tungstenite::tungstenite::client::IntoClientRequest;

const EXPORTS: &str = "[contract]\nversion = \"1.0.0\"\n\n[[export]]\nprogram = \"PING\"\npublic = true\n";
const PING: &str = "IDENTIFICATION DIVISION.\nPROGRAM-ID. PING.\nDATA DIVISION.\nLINKAGE SECTION.\n01 LK-PONG.\n    05 REPLY PIC X(4).\nPROCEDURE DIVISION RETURNING LK-PONG.\nMAIN.\n    MOVE \"pong\" TO REPLY\n    GOBACK.\n";

/// `Ok(())` when the WebSocket handshake succeeded, else the HTTP status that refused it.
async fn handshake(server: &DevServer, origin: Option<&str>) -> Result<(), u16> {
    let mut req = format!("ws://{}/ws", server.addr()).into_client_request().unwrap();
    if let Some(o) = origin {
        req.headers_mut().insert("Origin", o.parse().unwrap());
    }
    let tcp = tokio::net::TcpStream::connect(server.addr()).await.unwrap();
    match tokio_tungstenite::client_async(req, tcp).await {
        Ok(_) => Ok(()),
        Err(tokio_tungstenite::tungstenite::Error::Http(r)) => Err(r.status().as_u16()),
        Err(e) => panic!("the handshake failed in another way: {e}"),
    }
}

fn start(tag: &str, page_origin: Option<&str>) -> DevServer {
    let dir = backend_dir(tag, EXPORTS, &[("ping.cbl", PING)]);
    DevServer::start(&DevOptions { backend_dir: dir, listen: None, page_origin: page_origin.map(str::to_owned), ..DevOptions::default() }).expect("the dev backend starts")
}

#[tokio::test(flavor = "multi_thread")]
async fn only_the_pages_origin_may_connect_and_not_another_port_of_the_same_host() {
    let server = start("origin", Some("http://127.0.0.1:5173"));
    assert_eq!(handshake(&server, Some("http://127.0.0.1:5173")).await, Ok(()), "the page's own origin");
    assert_eq!(handshake(&server, Some("http://127.0.0.1:5174")).await, Err(403), "another port of the same machine");
    assert_eq!(handshake(&server, Some("http://localhost:5173")).await, Err(403), "another host name for the same port");
    assert_eq!(handshake(&server, Some("https://evil.example.test")).await, Err(403));
    assert_eq!(handshake(&server, None).await, Ok(()), "a client that sends no Origin is not a browser (rcrun, curl)");
    println!("dev origin: the page's origin accepted; another port, another host and a foreign site get 403; no Origin accepted");
    server.stop();
}

#[tokio::test(flavor = "multi_thread")]
async fn without_a_page_origin_any_origin_is_still_accepted() {
    let server = start("any", None);
    assert_eq!(handshake(&server, Some("http://127.0.0.1:9999")).await, Ok(()));
    server.stop();
}
