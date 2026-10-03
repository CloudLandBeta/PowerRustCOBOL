// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! **The minimum of MCP's Streamable HTTP transport** (spec 080 plan §1.4),
//! over `std::net` — no HTTP stack, for the reason `cobolt-mcp` has none.
//!
//! | Request | Answer |
//! |---|---|
//! | `POST /mcp/<kit-id>`, a JSON-RPC request | `200`, `application/json`, the one reply. Never SSE. |
//! | `POST` of a notification or a response | `202`, empty body |
//! | `GET` / `DELETE` (or any other method) | `405` — no server stream, no session |
//! | `Origin` present and not this server | `403` (DNS-rebinding defence) |
//! | `Host` not `127.0.0.1:<port>` / `localhost:<port>` | `403` |
//! | `Content-Type` not `application/json` | `415` |
//! | chunked, or no `Content-Length` | `411`; a body over the MCP cap → `413` |
//! | any other path | `404` |
//!
//! Every response closes its connection. Each connection has its own thread,
//! so a `check` can answer while a `build` waits; the writing tools serialise
//! on the tool set's own lock.
//!
//! The URL carries the kit's id, and the request is served only when that id
//! is the open project's (R14): with no project open every tool answers "no
//! project open", with another project open every tool says so — and
//! `tools/list` always answers, so a client can connect first.

use std::io::{self, BufRead, BufReader, Read, Write};
use std::net::{Shutdown, TcpListener, TcpStream};
use std::sync::Arc;
use std::time::Duration;

use cobolt_mcp::{McpHandler, ServerInfo, Tool, ToolResult};
use serde_json::Value;

use crate::host::NoProject;

/// Longest request head accepted.
const MAX_HEAD_BYTES: usize = 64 * 1024;

/// What the server knows of the open project when a request arrives.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OpenKit {
    /// No project is open.
    NoProject,
    /// A project is open; `kit_id` is its kit's id, `None` without a kit.
    Project { kit_id: Option<String> },
}

/// Serve until the listener fails. `make_handler` builds the handler for one
/// request; `open_kit` says which project is open now.
pub fn serve_http<H, F, K>(listener: TcpListener, make_handler: F, open_kit: K) -> io::Result<()>
where
    H: McpHandler + 'static,
    F: Fn() -> H + Send + Sync + 'static,
    K: Fn() -> OpenKit + Send + Sync + 'static,
{
    let port = listener.local_addr()?.port();
    let make_handler = Arc::new(make_handler);
    let open_kit = Arc::new(open_kit);
    for stream in listener.incoming() {
        let Ok(stream) = stream else { continue };
        let make_handler = Arc::clone(&make_handler);
        let open_kit = Arc::clone(&open_kit);
        std::thread::spawn(move || {
            let mut stream = stream;
            let _ = handle(&mut stream, port, &*make_handler, &*open_kit);
            close_gently(&mut stream);
        });
    }
    Ok(())
}

struct Head {
    method: String,
    path: String,
    headers: Vec<(String, String)>,
}

impl Head {
    fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case(name))
            .map(|(_, v)| v.as_str())
    }
}

fn read_head(reader: &mut impl BufRead) -> io::Result<Option<Head>> {
    let mut lines = Vec::new();
    let mut total = 0usize;
    loop {
        let mut line = String::new();
        let n = reader.read_line(&mut line)?;
        if n == 0 {
            return Ok(None);
        }
        total += n;
        if total > MAX_HEAD_BYTES {
            return Ok(None);
        }
        let line = line.trim_end_matches(['\r', '\n']).to_owned();
        if line.is_empty() {
            break;
        }
        lines.push(line);
    }
    let mut it = lines.into_iter();
    let Some(request_line) = it.next() else {
        return Ok(None);
    };
    let mut parts = request_line.split_whitespace();
    let method = parts.next().unwrap_or_default().to_owned();
    let path = parts.next().unwrap_or_default().to_owned();
    let headers = it
        .filter_map(|l| l.split_once(':').map(|(k, v)| (k.trim().to_owned(), v.trim().to_owned())))
        .collect();
    Ok(Some(Head { method, path, headers }))
}

fn respond(stream: &mut TcpStream, status: &str, extra: &str, body: &[u8]) -> io::Result<()> {
    let ct = if body.is_empty() { "" } else { "Content-Type: application/json\r\n" };
    write!(
        stream,
        "HTTP/1.1 {status}\r\n{ct}{extra}Content-Length: {}\r\nConnection: close\r\n\r\n",
        body.len()
    )?;
    stream.write_all(body)?;
    stream.flush()
}

fn local_authority(value: &str, port: u16) -> bool {
    value == format!("127.0.0.1:{port}") || value.eq_ignore_ascii_case(&format!("localhost:{port}"))
}

/// End the connection without a reset: a refusal can leave a request body
/// unread, and closing over unread input makes the peer's stack drop the
/// answer it was about to read.
fn close_gently(stream: &mut TcpStream) {
    let _ = stream.shutdown(Shutdown::Write);
    let _ = stream.set_read_timeout(Some(Duration::from_millis(200)));
    let mut sink = [0u8; 8192];
    let mut left: usize = 1024 * 1024;
    while left > 0 {
        match stream.read(&mut sink) {
            Ok(0) | Err(_) => break,
            Ok(n) => left = left.saturating_sub(n),
        }
    }
}

fn handle<H: McpHandler>(
    stream: &mut TcpStream,
    port: u16,
    make_handler: &dyn Fn() -> H,
    open_kit: &dyn Fn() -> OpenKit,
) -> io::Result<()> {
    stream.set_read_timeout(Some(Duration::from_secs(30)))?;
    let mut reader = BufReader::new(stream.try_clone()?);
    let Some(head) = read_head(&mut reader)? else {
        return respond(stream, "400 Bad Request", "", b"");
    };

    // DNS-rebinding defence: a page in a browser names its own host and origin.
    if !head.header("Host").is_some_and(|h| local_authority(h, port)) {
        return respond(stream, "403 Forbidden", "", b"");
    }
    if let Some(origin) = head.header("Origin") {
        let ok = origin
            .strip_prefix("http://")
            .is_some_and(|rest| local_authority(rest.trim_end_matches('/'), port));
        if !ok {
            return respond(stream, "403 Forbidden", "", b"");
        }
    }
    let path = head.path.split('?').next().unwrap_or_default();
    let Some(kit_id) = path.strip_prefix("/mcp/").filter(|id| !id.is_empty() && !id.contains('/')) else {
        return respond(stream, "404 Not Found", "", b"");
    };
    if head.method != "POST" {
        return respond(stream, "405 Method Not Allowed", "Allow: POST\r\n", b"");
    }
    if head
        .header("Transfer-Encoding")
        .is_some_and(|t| t.to_ascii_lowercase().contains("chunked"))
    {
        return respond(stream, "411 Length Required", "", b"");
    }
    let Some(len) = head.header("Content-Length").and_then(|v| v.parse::<usize>().ok()) else {
        return respond(stream, "411 Length Required", "", b"");
    };
    if len > cobolt_mcp::transport::MAX_MESSAGE_BYTES {
        return respond(stream, "413 Payload Too Large", "", b"");
    }
    let json_type = head
        .header("Content-Type")
        .map(|c| c.split(';').next().unwrap_or("").trim().eq_ignore_ascii_case("application/json"))
        .unwrap_or(false);
    if !json_type {
        return respond(stream, "415 Unsupported Media Type", "", b"");
    }
    let mut body = vec![0u8; len];
    reader.read_exact(&mut body)?;

    // A response from the client (no method) owes nothing back.
    let is_response = serde_json::from_slice::<Value>(&body)
        .ok()
        .is_some_and(|v| v.is_object() && v.get("method").is_none());
    if is_response {
        return respond(stream, "202 Accepted", "", b"");
    }

    let refuse = match open_kit() {
        OpenKit::NoProject => Some(NoProject::NoneOpen),
        OpenKit::Project { kit_id: Some(open) } if open == kit_id => None,
        OpenKit::Project { .. } => Some(NoProject::Different),
    };
    let mut gated = Gated {
        inner: make_handler(),
        refuse,
    };
    match cobolt_mcp::dispatch(&body, &mut gated) {
        Some(reply) => {
            let bytes = serde_json::to_vec(&reply).map_err(io::Error::other)?;
            respond(stream, "200 OK", "", &bytes)
        }
        None => respond(stream, "202 Accepted", "", b""),
    }
}

/// The handler, or — for a request meant for a project that is not open —
/// the same tool list with every call answering why not.
struct Gated<H> {
    inner: H,
    refuse: Option<NoProject>,
}

impl<H: McpHandler> McpHandler for Gated<H> {
    fn server_info(&self) -> ServerInfo {
        self.inner.server_info()
    }
    fn list_tools(&mut self) -> Vec<Tool> {
        self.inner.list_tools()
    }
    fn call_tool(&mut self, name: &str, arguments: &Value) -> ToolResult {
        match &self.refuse {
            Some(why) => ToolResult::failed(why.message()),
            None => self.inner.call_tool(name, arguments),
        }
    }
    fn capabilities(&self) -> Value {
        self.inner.capabilities()
    }
    // Knowledge needs no open project: forwarded even when calls are refused.
    fn instructions(&self) -> Option<String> {
        self.inner.instructions()
    }
    fn list_resources(&mut self) -> Vec<cobolt_mcp::Resource> {
        self.inner.list_resources()
    }
    fn read_resource(&mut self, uri: &str) -> Option<cobolt_mcp::ResourceContents> {
        self.inner.read_resource(uri)
    }
}
