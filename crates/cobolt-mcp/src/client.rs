// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! The MCP **client** — the other half of [`crate::server`] (spec 078 R1–R6).
//!
//! Two layers, both pure:
//!
//! - [`Session`] builds request frames, numbers them, and decides what to do
//!   with every inbound frame ([`Session::route`]). It owns no stream, no
//!   thread and no clock, so a host can drive it from a reader thread with
//!   its own timeouts (the runtime's server pool does).
//! - [`Client`] drives a session over a `BufRead`/`Write` pair synchronously:
//!   send, then read until the answer to that request arrives, serving the
//!   server's own requests on the way. Tests and one-shot diagnostics use it.
//!
//! A server may interleave traffic of its own with the answer a client waits
//! for — progress and logging notifications, a `ping`, a sampling request.
//! None of it may stall a pending call (R4): notifications are dropped, a
//! `ping` is answered, and every other server request is refused with
//! `METHOD_NOT_FOUND`, which the protocol allows for a capability the client
//! never declared.

use std::io::{BufRead, Write};

use serde_json::{json, Value};

use crate::transport::{read_message, write_message};
use crate::types::{
    error_code, CallToolParams, ClientInfo, InitializeParams, InitializeResult, ListToolsParams,
    ListToolsResult, Message, Request, Response, Tool, ToolResult, CLIENT_ACCEPTED_VERSIONS,
    CLIENT_OFFERED_VERSION, STATELESS_META_KEY, STATELESS_REVISION,
};

/// What went wrong with a call, from the client's side.
#[derive(Debug, Clone, PartialEq)]
pub enum ClientError {
    /// The stream failed or closed before the answer came.
    Io(String),
    /// The server answered with a JSON-RPC error.
    Rpc { code: i64, message: String },
    /// The server's answer could not be understood.
    Protocol(String),
    /// The server answered `initialize` with a revision this client does not
    /// speak.
    UnsupportedVersion(String),
}

impl std::fmt::Display for ClientError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ClientError::Io(e) => write!(f, "the MCP server connection failed: {e}"),
            ClientError::Rpc { code, message } => write!(f, "the MCP server refused the request ({code}): {message}"),
            ClientError::Protocol(e) => write!(f, "the MCP server's answer was not understood: {e}"),
            ClientError::UnsupportedVersion(v) => {
                write!(f, "the MCP server speaks protocol revision {v}, which this client does not")
            }
        }
    }
}

impl std::error::Error for ClientError {}

/// How a session talks to its server.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    /// `initialize` / `notifications/initialized` first, then requests.
    Handshake,
    /// No handshake; every request names the stateless revision in `_meta`.
    Stateless,
}

/// What [`Session::route`] decided about one inbound frame.
#[derive(Debug, Clone, PartialEq)]
pub enum Inbound {
    /// An answer to one of our requests.
    Response(Response),
    /// A server request we must answer: write these bytes back.
    Reply(Vec<u8>),
    /// A notification, or anything else needing no action.
    Ignored,
    /// A frame that is not a JSON-RPC message at all.
    Malformed(String),
}

/// One client conversation with one server.
#[derive(Debug, Clone)]
pub struct Session {
    mode: Mode,
    next_id: u64,
    revision: Option<String>,
    client: ClientInfo,
}

impl Session {
    /// A session that opens with the handshake.
    pub fn handshake(client: ClientInfo) -> Self {
        Self { mode: Mode::Handshake, next_id: 1, revision: None, client }
    }

    /// A session for a server the route table marks stateless.
    pub fn stateless(client: ClientInfo) -> Self {
        Self { mode: Mode::Stateless, next_id: 1, revision: Some(STATELESS_REVISION.into()), client }
    }

    pub fn mode(&self) -> Mode {
        self.mode
    }

    /// The revision in force: the one the server answered `initialize` with,
    /// or the stateless revision. `None` before the handshake completes.
    pub fn revision(&self) -> Option<&str> {
        self.revision.as_deref()
    }

    /// A numbered request frame, and its id.
    pub fn request(&mut self, method: &str, params: Option<Value>) -> (u64, Vec<u8>) {
        let id = self.next_id;
        self.next_id += 1;
        let req = Request { jsonrpc: "2.0".into(), id: Some(json!(id)), method: method.into(), params };
        (id, serde_json::to_vec(&req).expect("a request serialises"))
    }

    /// A notification frame: no id, no answer owed.
    pub fn notification(method: &str, params: Option<Value>) -> Vec<u8> {
        let req = Request { jsonrpc: "2.0".into(), id: None, method: method.into(), params };
        serde_json::to_vec(&req).expect("a notification serialises")
    }

    /// The `initialize` request offering [`CLIENT_OFFERED_VERSION`].
    pub fn initialize_request(&mut self) -> (u64, Vec<u8>) {
        let params = InitializeParams {
            protocol_version: CLIENT_OFFERED_VERSION.into(),
            capabilities: json!({}),
            client_info: self.client.clone(),
        };
        self.request("initialize", Some(serde_json::to_value(params).expect("params serialise")))
    }

    /// The notification that completes the handshake.
    pub fn initialized_notification() -> Vec<u8> {
        Self::notification("notifications/initialized", None)
    }

    /// Accept the server's `initialize` answer: its revision must be one this
    /// client speaks.
    pub fn accept_initialize(&mut self, result: &Value) -> Result<InitializeResult, ClientError> {
        let init: InitializeResult =
            serde_json::from_value(result.clone()).map_err(|e| ClientError::Protocol(e.to_string()))?;
        if !CLIENT_ACCEPTED_VERSIONS.contains(&init.protocol_version.as_str()) {
            return Err(ClientError::UnsupportedVersion(init.protocol_version));
        }
        self.revision = Some(init.protocol_version.clone());
        Ok(init)
    }

    /// One page of `tools/list`.
    pub fn list_tools_request(&mut self, cursor: Option<&str>) -> (u64, Vec<u8>) {
        let params = ListToolsParams { cursor: cursor.map(str::to_owned) };
        let mut params = serde_json::to_value(params).expect("params serialise");
        self.stamp(&mut params);
        self.request("tools/list", Some(params))
    }

    /// `tools/call`.
    pub fn call_tool_request(&mut self, name: &str, arguments: Value) -> (u64, Vec<u8>) {
        let params = CallToolParams { name: name.into(), arguments, meta: None };
        let mut params = serde_json::to_value(params).expect("params serialise");
        self.stamp(&mut params);
        self.request("tools/call", Some(params))
    }

    /// A stateless request carries its revision in `_meta`.
    fn stamp(&self, params: &mut Value) {
        if self.mode == Mode::Stateless {
            if let Some(obj) = params.as_object_mut() {
                let meta = obj.entry("_meta").or_insert_with(|| json!({}));
                if let Some(m) = meta.as_object_mut() {
                    m.insert(STATELESS_META_KEY.into(), json!(STATELESS_REVISION));
                }
            }
        }
    }

    /// Decide what one inbound frame needs (R4).
    pub fn route(&self, raw: &[u8]) -> Inbound {
        match Message::parse(raw) {
            Ok(Message::Response(r)) => Inbound::Response(r),
            Ok(Message::Request(req)) if req.is_notification() => Inbound::Ignored,
            Ok(Message::Request(req)) => {
                let id = req.id.clone().unwrap_or(Value::Null);
                let reply = if req.method == "ping" {
                    Response::ok(id, json!({}))
                } else {
                    Response::err(
                        id,
                        error_code::METHOD_NOT_FOUND,
                        format!("this client does not offer {}", req.method),
                    )
                };
                Inbound::Reply(serde_json::to_vec(&reply).expect("a reply serialises"))
            }
            Err(e) => Inbound::Malformed(e),
        }
    }
}

/// The result of an answered request, or the server's error.
pub fn into_result(resp: Response) -> Result<Value, ClientError> {
    match (resp.result, resp.error) {
        (_, Some(e)) => Err(ClientError::Rpc { code: e.code, message: e.message }),
        (Some(v), None) => Ok(v),
        (None, None) => Err(ClientError::Protocol("an answer with neither result nor error".into())),
    }
}

/// A [`Session`] driven synchronously over a stream pair.
pub struct Client<R: BufRead, W: Write> {
    pub session: Session,
    reader: R,
    writer: W,
}

impl<R: BufRead, W: Write> Client<R, W> {
    pub fn new(session: Session, reader: R, writer: W) -> Self {
        Self { session, reader, writer }
    }

    /// The writer, for a test to read what was sent.
    pub fn writer(&self) -> &W {
        &self.writer
    }

    fn send(&mut self, bytes: &[u8]) -> Result<(), ClientError> {
        write_message(&mut self.writer, bytes).map_err(|e| ClientError::Io(e.to_string()))
    }

    /// Read until the answer to `id` arrives, serving the server on the way.
    fn await_answer(&mut self, id: u64) -> Result<Value, ClientError> {
        loop {
            let raw = read_message(&mut self.reader)
                .map_err(|e| ClientError::Io(e.to_string()))?
                .ok_or_else(|| ClientError::Io("the server closed the connection".into()))?;
            match self.session.route(&raw) {
                Inbound::Response(r) if r.id == json!(id) => return into_result(r),
                // An answer to something else (a request already given up on):
                // not ours to deliver.
                Inbound::Response(_) | Inbound::Ignored | Inbound::Malformed(_) => {}
                Inbound::Reply(bytes) => self.send(&bytes)?,
            }
        }
    }

    /// Send a request and wait for its answer.
    pub fn call(&mut self, method: &str, params: Option<Value>) -> Result<Value, ClientError> {
        let (id, bytes) = self.session.request(method, params);
        self.send(&bytes)?;
        self.await_answer(id)
    }

    /// The handshake. A stateless session has none and returns `None`.
    pub fn initialize(&mut self) -> Result<Option<InitializeResult>, ClientError> {
        if self.session.mode() == Mode::Stateless {
            return Ok(None);
        }
        let (id, bytes) = self.session.initialize_request();
        self.send(&bytes)?;
        let result = self.await_answer(id)?;
        let init = self.session.accept_initialize(&result)?;
        self.send(&Session::initialized_notification())?;
        Ok(Some(init))
    }

    /// Every tool, following `nextCursor` across pages.
    pub fn list_tools_all(&mut self) -> Result<Vec<Tool>, ClientError> {
        let mut tools = Vec::new();
        let mut cursor: Option<String> = None;
        loop {
            let (id, bytes) = self.session.list_tools_request(cursor.as_deref());
            self.send(&bytes)?;
            let page: ListToolsResult = serde_json::from_value(self.await_answer(id)?)
                .map_err(|e| ClientError::Protocol(e.to_string()))?;
            tools.extend(page.tools);
            match page.next_cursor {
                Some(c) if !c.is_empty() => cursor = Some(c),
                _ => return Ok(tools),
            }
        }
    }

    /// Call one tool. A tool-level failure is `Ok` with `is_error` set — the
    /// tool ran and said no — and is the caller's to report.
    pub fn call_tool(&mut self, name: &str, arguments: Value) -> Result<ToolResult, ClientError> {
        let (id, bytes) = self.session.call_tool_request(name, arguments);
        self.send(&bytes)?;
        serde_json::from_value(self.await_answer(id)?).map_err(|e| ClientError::Protocol(e.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::server::{dispatch, McpHandler};
    use crate::types::{Content, ServerInfo};
    use std::io::Cursor;

    fn me() -> ClientInfo {
        ClientInfo { name: "test-client".into(), version: "1".into() }
    }

    /// Scripted server output: one JSON line per frame.
    fn script(lines: &[Value]) -> Cursor<Vec<u8>> {
        let mut out = Vec::new();
        for l in lines {
            out.extend_from_slice(serde_json::to_string(l).unwrap().as_bytes());
            out.push(b'\n');
        }
        Cursor::new(out)
    }

    fn sent(w: &[u8]) -> Vec<Value> {
        w.split(|b| *b == b'\n').filter(|l| !l.is_empty()).map(|l| serde_json::from_slice(l).unwrap()).collect()
    }

    fn init_answer(id: u64, version: &str) -> Value {
        json!({"jsonrpc":"2.0","id":id,"result":{"protocolVersion":version,"capabilities":{"tools":{}},"serverInfo":{"name":"s","version":"1"}}})
    }

    #[test]
    fn the_handshake_accepts_2025_11_25_and_2025_06_18() {
        for version in ["2025-11-25", "2025-06-18", "2024-11-05"] {
            let mut c = Client::new(Session::handshake(me()), script(&[init_answer(1, version)]), Vec::new());
            let init = c.initialize().unwrap().unwrap();
            assert_eq!(init.protocol_version, version);
            assert_eq!(c.session.revision(), Some(version));
            let out = sent(c.writer());
            assert_eq!(out[0]["method"], "initialize");
            assert_eq!(out[0]["params"]["protocolVersion"], CLIENT_OFFERED_VERSION);
            assert_eq!(out[1]["method"], "notifications/initialized");
            assert!(out[1].get("id").is_none(), "the notification has no id");
        }
    }

    #[test]
    fn an_unsupported_answered_version_is_refused() {
        let mut c = Client::new(Session::handshake(me()), script(&[init_answer(1, "1999-01-01")]), Vec::new());
        assert_eq!(c.initialize().unwrap_err(), ClientError::UnsupportedVersion("1999-01-01".into()));
    }

    /// AC1 at the unit level: two pages of tools.
    #[test]
    fn tools_list_follows_next_cursor() {
        let page1 = json!({"jsonrpc":"2.0","id":1,"result":{"tools":[{"name":"a","inputSchema":{}}],"nextCursor":"p2"}});
        let page2 = json!({"jsonrpc":"2.0","id":2,"result":{"tools":[{"name":"b","inputSchema":{}},{"name":"c","inputSchema":{}}]}});
        let mut c = Client::new(Session::handshake(me()), script(&[page1, page2]), Vec::new());
        let tools = c.list_tools_all().unwrap();
        assert_eq!(tools.iter().map(|t| t.name.as_str()).collect::<Vec<_>>(), ["a", "b", "c"]);
        let out = sent(c.writer());
        assert!(out[0]["params"].get("cursor").is_none());
        assert_eq!(out[1]["params"]["cursor"], "p2");
    }

    /// AC3 at the unit level: progress, logging and a server `ping` arrive
    /// before the answer; the call still completes and the ping is answered.
    #[test]
    fn interleaved_server_traffic_does_not_stall_a_call() {
        let frames = [
            json!({"jsonrpc":"2.0","method":"notifications/progress","params":{"progress":1}}),
            json!({"jsonrpc":"2.0","method":"notifications/message","params":{"level":"info","data":"x"}}),
            json!({"jsonrpc":"2.0","id":"srv-1","method":"ping"}),
            json!({"jsonrpc":"2.0","id":"srv-2","method":"sampling/createMessage","params":{}}),
            json!({"jsonrpc":"2.0","id":1,"result":{"content":[{"type":"text","text":"done"}]}}),
        ];
        let mut c = Client::new(Session::handshake(me()), script(&frames), Vec::new());
        let r = c.call_tool("t", json!({})).unwrap();
        assert_eq!(r.content, vec![Content::text("done")]);
        let out = sent(c.writer());
        assert_eq!(out.len(), 3, "the call, the ping reply, the sampling refusal: {out:?}");
        assert_eq!(out[1]["id"], "srv-1");
        assert_eq!(out[1]["result"], json!({}));
        assert_eq!(out[2]["id"], "srv-2");
        assert_eq!(out[2]["error"]["code"], error_code::METHOD_NOT_FOUND);
    }

    #[test]
    fn a_tool_error_is_surfaced_not_raised() {
        let frames = [json!({"jsonrpc":"2.0","id":1,"result":{"content":[{"type":"text","text":"no such table"}],"isError":true}})];
        let mut c = Client::new(Session::handshake(me()), script(&frames), Vec::new());
        let r = c.call_tool("t", json!({})).unwrap();
        assert_eq!(r.is_error, Some(true));
        let rpc = [json!({"jsonrpc":"2.0","id":1,"error":{"code":-32602,"message":"bad"}})];
        let mut c = Client::new(Session::handshake(me()), script(&rpc), Vec::new());
        assert_eq!(c.call_tool("t", json!({})).unwrap_err(), ClientError::Rpc { code: -32602, message: "bad".into() });
    }

    #[test]
    fn a_stateless_call_carries_its_revision_and_sends_no_handshake() {
        let frames = [json!({"jsonrpc":"2.0","id":1,"result":{"content":[]}})];
        let mut c = Client::new(Session::stateless(me()), script(&frames), Vec::new());
        assert_eq!(c.initialize().unwrap(), None);
        c.call_tool("t", json!({"a":1})).unwrap();
        let out = sent(c.writer());
        assert_eq!(out.len(), 1, "no handshake: {out:?}");
        assert_eq!(out[0]["params"]["_meta"][STATELESS_META_KEY], STATELESS_REVISION);
        assert_eq!(out[0]["params"]["arguments"]["a"], 1);
    }

    #[test]
    fn a_closed_stream_is_an_io_error_not_a_hang() {
        let mut c = Client::new(Session::handshake(me()), script(&[]), Vec::new());
        assert!(matches!(c.call_tool("t", json!({})), Err(ClientError::Io(_))));
    }

    /// A handler that accepts whatever it is asked.
    struct Echo;
    impl McpHandler for Echo {
        fn server_info(&self) -> ServerInfo {
            ServerInfo { name: "echo".into(), version: "1".into() }
        }
        fn list_tools(&mut self) -> Vec<Tool> {
            vec![Tool { name: "t".into(), description: None, input_schema: json!({"type":"object"}), annotations: None }]
        }
        fn call_tool(&mut self, name: &str, arguments: &Value) -> ToolResult {
            ToolResult::ok(vec![Content::text(format!("{name} {arguments}"))])
        }
    }

    /// AC4 (R6): every request this client sends parses as a request in this
    /// crate's own server, and is answered — the two directions share one set
    /// of wire types and cannot drift.
    #[test]
    fn every_client_request_parses_as_a_server_request() {
        let mut s = Session::handshake(me());
        let mut st = Session::stateless(me());
        let frames: Vec<(&str, Vec<u8>)> = vec![
            ("initialize", s.initialize_request().1),
            ("notifications/initialized", Session::initialized_notification()),
            ("tools/list", s.list_tools_request(None).1),
            ("tools/list (cursor)", s.list_tools_request(Some("p2")).1),
            ("tools/call", s.call_tool_request("t", json!({"x":1})).1),
            ("tools/call (stateless)", st.call_tool_request("t", json!({"x":1})).1),
            ("tools/list (stateless)", st.list_tools_request(None).1),
        ];
        let mut h = Echo;
        for (what, bytes) in &frames {
            let reply = dispatch(bytes, &mut h);
            if *what == "notifications/initialized" {
                assert!(reply.is_none(), "a notification is not answered");
                continue;
            }
            let reply = reply.unwrap_or_else(|| panic!("{what}: no reply"));
            assert!(reply.error.is_none(), "{what}: the server refused it: {:?}", reply.error);
        }
        println!("wire parity: {} client frames parsed and served by the crate's own server", frames.len());
    }
}
