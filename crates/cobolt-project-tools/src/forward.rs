// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! **`rcrun mcp` hands the IDE's tools to the IDE** (spec 084 R66).
//!
//! Claude Code connects to both servers when it starts. When PowerRustCOBOL AI
//! was closed then, the IDE's server stays unreachable for that conversation,
//! even after the IDE opens; `rcrun mcp` is still connected. So a tool only the
//! IDE answers (the Form Designer, Project Settings, run, debug, its windows)
//! goes from `rcrun mcp` to the IDE over the same HTTP address and token Claude
//! Code would have used — and only when the IDE has this very project open, so
//! the two servers never act on one project at the same time from two places.
//!
//! The address and the token come from the plugin's own settings, which
//! Claude Code passes to `rcrun` in its environment; nothing is read from the
//! IDE's files and the token is never written anywhere.

use std::io::{Read, Write};
use std::net::{SocketAddr, TcpStream};
use std::path::Path;
use std::time::Duration;

use serde_json::{json, Value};

/// How long the IDE may take to answer one tool (a check of a large project).
const ANSWER_LIMIT: Duration = Duration::from_secs(300);
/// How long reaching the IDE may take before it counts as not running.
const CONNECT_LIMIT: Duration = Duration::from_millis(800);

/// Where the IDE serves its tools, and the token it admits.
#[derive(Clone, PartialEq, Eq)]
pub struct IdeAddress {
    pub port: u16,
    pub token: String,
}

impl std::fmt::Debug for IdeAddress {
    // The token never reaches a log.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "IdeAddress {{ port: {}, token: *** }}", self.port)
    }
}

impl IdeAddress {
    /// From the environment Claude Code gives `rcrun`: the plugin's `.mcp.json`
    /// sets `PRC_IDE_PORT` / `PRC_IDE_TOKEN` from its `port` and `token`
    /// options; Claude Code also exports every plugin option as
    /// `CLAUDE_PLUGIN_OPTION_<NAME>`. `None` when either is missing or empty.
    pub fn from_env() -> Option<Self> {
        let var = |names: &[&str]| {
            names
                .iter()
                .filter_map(|n| std::env::var(n).ok())
                .map(|v| v.trim().to_owned())
                .find(|v| !v.is_empty() && !v.starts_with("${"))
        };
        let port = var(&["PRC_IDE_PORT", "CLAUDE_PLUGIN_OPTION_PORT"])?.parse().ok()?;
        let token = var(&["PRC_IDE_TOKEN", "CLAUDE_PLUGIN_OPTION_TOKEN"])?;
        Some(Self { port, token })
    }
}

/// Why a call could not be forwarded.
#[derive(Debug)]
pub enum ForwardError {
    /// Nothing answers at the address: the IDE is not running.
    NotRunning,
    /// The IDE runs with another project open, or none; what to do about it.
    OtherProject(String),
    /// The IDE answered, but refused or failed the call; its words.
    Answer(String),
}

/// One JSON-RPC request to the IDE; its `result` or its error.
fn rpc(at: &IdeAddress, method: &str, params: Value) -> Result<Value, ForwardError> {
    let addr = SocketAddr::from(([127, 0, 0, 1], at.port));
    let mut stream = TcpStream::connect_timeout(&addr, CONNECT_LIMIT).map_err(|_| ForwardError::NotRunning)?;
    let _ = stream.set_read_timeout(Some(ANSWER_LIMIT));
    let _ = stream.set_write_timeout(Some(ANSWER_LIMIT));
    let body = json!({ "jsonrpc": "2.0", "id": 1, "method": method, "params": params }).to_string();
    let head = format!(
        "POST /mcp HTTP/1.1\r\nHost: 127.0.0.1:{}\r\nAuthorization: Bearer {}\r\nContent-Type: application/json\r\n\
         Accept: application/json, text/event-stream\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        at.port,
        at.token,
        body.len()
    );
    stream
        .write_all(head.as_bytes())
        .and_then(|_| stream.write_all(body.as_bytes()))
        .map_err(|_| ForwardError::NotRunning)?;
    let mut raw = Vec::new();
    stream
        .read_to_end(&mut raw)
        .map_err(|e| ForwardError::Answer(format!("PowerRustCOBOL AI did not answer ({e})")))?;
    let text = String::from_utf8_lossy(&raw);
    let (status_line, rest) = text.split_once("\r\n").unwrap_or((&text, ""));
    let status: u16 = status_line.split_whitespace().nth(1).and_then(|s| s.parse().ok()).unwrap_or(0);
    let body = rest.split_once("\r\n\r\n").map(|(_, b)| b).unwrap_or("");
    match status {
        200 => {}
        401 => {
            return Err(ForwardError::Answer(
                "PowerRustCOBOL AI refused the access token; choose Configure Claude Code in the IDE again".to_owned(),
            ))
        }
        other => return Err(ForwardError::Answer(format!("PowerRustCOBOL AI answered HTTP {other}"))),
    }
    let reply: Value =
        serde_json::from_str(body).map_err(|e| ForwardError::Answer(format!("PowerRustCOBOL AI's answer cannot be read ({e})")))?;
    if let Some(err) = reply.get("error") {
        let msg = err.get("message").and_then(Value::as_str).unwrap_or("error");
        return Err(ForwardError::Answer(msg.to_owned()));
    }
    Ok(reply.get("result").cloned().unwrap_or(Value::Null))
}

/// A `tools/call` result back into what a tool answers: the JSON of its text,
/// or its error; a picture comes back as `png_base64`, which the front turns
/// into image content again.
pub fn tool_answer(result: &Value) -> Result<Value, String> {
    let content = result.get("content").and_then(Value::as_array).cloned().unwrap_or_default();
    let mut text = String::new();
    let mut png = None;
    for c in &content {
        match c.get("type").and_then(Value::as_str) {
            Some("text") => text.push_str(c.get("text").and_then(Value::as_str).unwrap_or_default()),
            Some("image") => png = c.get("data").and_then(Value::as_str).map(str::to_owned),
            _ => {}
        }
    }
    if result.get("isError").and_then(Value::as_bool).unwrap_or(false) {
        return Err(text);
    }
    let mut answer = serde_json::from_str::<Value>(&text).unwrap_or_else(|_| json!({ "text": text }));
    if let (Some(png), Some(map)) = (png, answer.as_object_mut()) {
        map.insert("png_base64".into(), Value::String(png));
    }
    Ok(answer)
}

/// Call tool `name` in the IDE. A tool that needs a project is forwarded only
/// when the IDE has `manifest` open (`project_free` tools need none).
pub fn call_ide(at: &IdeAddress, name: &str, args: &Value, manifest: Option<&Path>, project_free: bool) -> Result<Value, ForwardError> {
    if !project_free {
        let state = tool_answer(&rpc(at, "tools/call", json!({ "name": "ide_state", "arguments": {} }))?)
            .map_err(ForwardError::Answer)?;
        let open = state.get("project").and_then(Value::as_str).map(Path::new);
        let same = match (open, manifest) {
            (Some(a), Some(b)) => same_file(a, b),
            _ => false,
        };
        if !same {
            return Err(ForwardError::OtherProject(match open {
                Some(other) => format!(
                    "PowerRustCOBOL AI has another project open ({}); call open_project with this project's folder first",
                    other.display()
                ),
                None => "PowerRustCOBOL AI has no project open; call open_project with this project's folder first".to_owned(),
            }));
        }
    }
    let result = rpc(at, "tools/call", json!({ "name": name, "arguments": args }))?;
    tool_answer(&result).map_err(ForwardError::Answer)
}

fn same_file(a: &Path, b: &Path) -> bool {
    match (a.canonicalize(), b.canonicalize()) {
        (Ok(x), Ok(y)) => x == y,
        _ => a == b,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_tool_result_comes_back_as_the_tools_answer() {
        let ok = json!({ "content": [{ "type": "text", "text": "{\"saved\":3}" }] });
        assert_eq!(tool_answer(&ok).unwrap(), json!({ "saved": 3 }));
        let failed = json!({ "content": [{ "type": "text", "text": "no project is open" }], "isError": true });
        assert_eq!(tool_answer(&failed).unwrap_err(), "no project is open");
        let picture = json!({ "content": [{ "type": "image", "data": "iVBO", "mimeType": "image/png" }, { "type": "text", "text": "{\"w\":10}" }] });
        assert_eq!(tool_answer(&picture).unwrap(), json!({ "w": 10, "png_base64": "iVBO" }));
        let debug = format!("{:?}", IdeAddress { port: 5720, token: "secret-token".into() });
        assert!(!debug.contains("secret-token"), "{debug}");
    }

    #[test]
    fn nothing_listening_is_not_running() {
        // A port nothing listens on: bind one, learn it, drop it.
        let port = std::net::TcpListener::bind("127.0.0.1:0").unwrap().local_addr().unwrap().port();
        let at = IdeAddress { port, token: "t".into() };
        assert!(matches!(call_ide(&at, "ide_settings", &json!({}), None, true), Err(ForwardError::NotRunning)));
    }
}
