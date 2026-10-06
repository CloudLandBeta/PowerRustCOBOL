// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! The JSON-RPC 2.0 envelope and the MCP bodies carried inside it.
//!
//! # What is modelled, and what deliberately is not
//!
//! The envelope is modelled exactly, because getting it wrong is what makes a
//! foreign client refuse to talk to us. The bodies are modelled only where this
//! server acts on them; everything else stays [`Value`], so a field a future
//! protocol revision adds travels through untouched rather than being dropped
//! on the floor. `cobolt-dap` made the same choice for the same reason.

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// The protocol revision this server implements.
///
/// Negotiation, not assertion: a client states the revision it wants, and
/// [`negotiate`] answers with that one when we can speak it and with this one
/// when we cannot. A server that simply announced its own version and ignored
/// the request would be refused by any client holding to an older revision.
pub const PROTOCOL_VERSION: &str = "2025-06-18";

/// Revisions this server is prepared to speak, newest first.
pub const SUPPORTED_VERSIONS: [&str; 2] = ["2025-06-18", "2024-11-05"];

/// The revision the CLIENT offers when it opens a handshake (spec 078 R3).
///
/// Kept apart from [`SUPPORTED_VERSIONS`]: that list drives what this crate's
/// SERVER echoes, and a server that suddenly answered a newer revision than it
/// was tested with would change behaviour for every existing client.
pub const CLIENT_OFFERED_VERSION: &str = "2025-11-25";

/// Revisions the client accepts in a server's `initialize` answer, newest
/// first — the one it offered, and the older handshake revisions AWS's
/// servers (the Python MCP SDK) may answer with.
pub const CLIENT_ACCEPTED_VERSIONS: [&str; 3] = ["2025-11-25", "2025-06-18", "2024-11-05"];

/// The stateless revision: no handshake, every request names its revision in
/// `_meta` (spec 078 R3, switched on per server by the route table).
pub const STATELESS_REVISION: &str = "2026-07-28";

/// The `_meta` key a stateless request carries its revision under.
pub const STATELESS_META_KEY: &str = "io.modelcontextprotocol/protocolVersion";

/// Answer a client's requested protocol revision.
///
/// Echoes the request when it is one we speak — the client then knows its own
/// revision was accepted — and otherwise offers ours, which is the protocol's
/// way of saying "not that one, but here is what I have".
pub fn negotiate(requested: Option<&str>) -> &'static str {
    match requested {
        Some(want) => SUPPORTED_VERSIONS
            .iter()
            .copied()
            .find(|v| *v == want)
            .unwrap_or(PROTOCOL_VERSION),
        None => PROTOCOL_VERSION,
    }
}

/// A JSON-RPC request or notification.
///
/// `id` absent means a notification: something the peer does not expect an
/// answer to, and answering one anyway is a protocol violation.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Request {
    pub jsonrpc: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<Value>,
    pub method: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub params: Option<Value>,
}

impl Request {
    /// Is this a notification — something expecting no reply?
    pub fn is_notification(&self) -> bool {
        self.id.is_none()
    }
}

/// Any one JSON-RPC message, as a peer's reader meets it (spec 078 R1).
///
/// A frame that names a `method` is a request (or, without an `id`, a
/// notification); one that carries a `result` or an `error` is a response.
/// The client needs this because a server may send requests of its own —
/// `ping`, `sampling/…` — interleaved with the answers it is waiting for.
#[derive(Debug, Clone, PartialEq)]
pub enum Message {
    Request(Request),
    Response(Response),
}

impl Message {
    /// Classify one raw frame.
    pub fn parse(raw: &[u8]) -> Result<Message, String> {
        let v: Value = serde_json::from_slice(raw).map_err(|e| format!("not JSON: {e}"))?;
        if v.get("method").is_some() {
            serde_json::from_value(v).map(Message::Request).map_err(|e| format!("not a request: {e}"))
        } else if v.get("result").is_some() || v.get("error").is_some() {
            serde_json::from_value(v).map(Message::Response).map_err(|e| format!("not a response: {e}"))
        } else {
            Err("neither a request nor a response".into())
        }
    }
}

/// A JSON-RPC response: exactly one of `result` or `error`, never both.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Response {
    pub jsonrpc: String,
    pub id: Value,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<RpcError>,
}

impl Response {
    pub fn ok(id: Value, result: Value) -> Self {
        Self {
            jsonrpc: "2.0".into(),
            id,
            result: Some(result),
            error: None,
        }
    }

    pub fn err(id: Value, code: i64, message: impl Into<String>) -> Self {
        Self {
            jsonrpc: "2.0".into(),
            id,
            result: None,
            error: Some(RpcError {
                code,
                message: message.into(),
                data: None,
            }),
        }
    }
}

/// A JSON-RPC error body.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RpcError {
    pub code: i64,
    pub message: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Value>,
}

/// The JSON-RPC 2.0 error codes this server produces.
pub mod error_code {
    /// The payload was not valid JSON.
    pub const PARSE_ERROR: i64 = -32700;
    /// Valid JSON, but not a valid request object.
    pub const INVALID_REQUEST: i64 = -32600;
    /// A method this server does not implement.
    pub const METHOD_NOT_FOUND: i64 = -32601;
    /// The method exists; the params do not fit it.
    pub const INVALID_PARAMS: i64 = -32602;
    /// `resources/read` named a URI this server does not have (MCP's code).
    pub const RESOURCE_NOT_FOUND: i64 = -32002;
    /// The handler failed for a reason of its own.
    pub const INTERNAL_ERROR: i64 = -32603;
}

/// Who is answering, and what revision they speak.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ServerInfo {
    pub name: String,
    pub version: String,
}

/// Who is asking — the client's half of `initialize`.
pub type ClientInfo = ServerInfo;

/// The client's `initialize` request body.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct InitializeParams {
    #[serde(rename = "protocolVersion")]
    pub protocol_version: String,
    pub capabilities: Value,
    #[serde(rename = "clientInfo")]
    pub client_info: ClientInfo,
}

/// `tools/list`'s request body: where to continue a paged listing.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct ListToolsParams {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cursor: Option<String>,
}

/// `tools/list`'s answer: one page of tools, and where the next one starts.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ListToolsResult {
    pub tools: Vec<Tool>,
    #[serde(rename = "nextCursor", default, skip_serializing_if = "Option::is_none")]
    pub next_cursor: Option<String>,
}

/// `tools/call`'s request body.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CallToolParams {
    pub name: String,
    #[serde(default)]
    pub arguments: Value,
    #[serde(rename = "_meta", default, skip_serializing_if = "Option::is_none")]
    pub meta: Option<Value>,
}

/// The reply to `initialize`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct InitializeResult {
    #[serde(rename = "protocolVersion")]
    pub protocol_version: String,
    pub capabilities: Value,
    #[serde(rename = "serverInfo")]
    pub server_info: ServerInfo,
    /// How to use this server, in prose the client hands to its model.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub instructions: Option<String>,
}

/// One document a server offers to be read, as `resources/list` lists it.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Resource {
    pub uri: String,
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(rename = "mimeType", default, skip_serializing_if = "Option::is_none")]
    pub mime_type: Option<String>,
}

/// The text of one resource, as `resources/read` returns it.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ResourceContents {
    pub uri: String,
    #[serde(rename = "mimeType", default, skip_serializing_if = "Option::is_none")]
    pub mime_type: Option<String>,
    pub text: String,
}

/// One tool, as `tools/list` advertises it.
///
/// `input_schema` is JSON Schema. It is a [`Value`] rather than a modelled type
/// because the schema is generated from whatever the developer described, and
/// narrowing it here would only constrain what they are allowed to say.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Tool {
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(rename = "inputSchema")]
    pub input_schema: Value,
    /// Hints about the tool's behaviour — `readOnlyHint`, `destructiveHint`.
    /// A client decides what it may call without asking from these (spec 078
    /// R26); a server that gives none leaves this `None`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub annotations: Option<Value>,
}

impl Tool {
    /// Whether the server marked this tool read-only (`readOnlyHint: true`).
    pub fn is_read_only(&self) -> bool {
        self.annotations
            .as_ref()
            .and_then(|a| a.get("readOnlyHint"))
            .and_then(Value::as_bool)
            .unwrap_or(false)
    }
}

/// One piece of a tool's answer.
///
/// Written by hand rather than derived: a server we did not write may answer
/// with content this crate does not model (an audio clip, an embedded
/// resource, a type a later revision adds), and a client that failed to decode
/// the whole result over one such block would lose the text beside it. Such a
/// block is kept, untouched, as [`Content::Other`] (spec 078). This crate's
/// own server only ever writes text and images.
#[derive(Debug, Clone, PartialEq)]
pub enum Content {
    Text { text: String },
    /// A picture, base64-encoded — a rendered form, for instance.
    Image { data: String, mime_type: String },
    /// Any other content block, exactly as it arrived.
    Other(Value),
}

impl Serialize for Content {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        let v = match self {
            Content::Text { text } => serde_json::json!({ "type": "text", "text": text }),
            Content::Image { data, mime_type } => {
                serde_json::json!({ "type": "image", "data": data, "mimeType": mime_type })
            }
            Content::Other(v) => v.clone(),
        };
        v.serialize(s)
    }
}

impl<'de> Deserialize<'de> for Content {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let v = Value::deserialize(d)?;
        let field = |k: &str| v.get(k).and_then(Value::as_str).map(str::to_owned);
        Ok(match v.get("type").and_then(Value::as_str) {
            Some("text") => match field("text") {
                Some(text) => Content::Text { text },
                None => Content::Other(v),
            },
            Some("image") => match (field("data"), field("mimeType")) {
                (Some(data), Some(mime_type)) => Content::Image { data, mime_type },
                _ => Content::Other(v),
            },
            _ => Content::Other(v),
        })
    }
}

impl Content {
    pub fn text(s: impl Into<String>) -> Self {
        Content::Text { text: s.into() }
    }

    /// The text of a text block; `None` for an image.
    pub fn as_text(&self) -> Option<&str> {
        match self {
            Content::Text { text } => Some(text),
            Content::Image { .. } | Content::Other(_) => None,
        }
    }

    /// A PNG, from its bytes.
    pub fn png(bytes: &[u8]) -> Self {
        Content::Image {
            data: base64(bytes),
            mime_type: "image/png".into(),
        }
    }
}

/// Standard base64 with padding — the encoding MCP's image content uses.
/// Written here rather than taken as a dependency: this crate's only
/// dependencies are the JSON ones (see `Cargo.toml`).
fn base64(bytes: &[u8]) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let b = [chunk[0], *chunk.get(1).unwrap_or(&0), *chunk.get(2).unwrap_or(&0)];
        let n = (u32::from(b[0]) << 16) | (u32::from(b[1]) << 8) | u32::from(b[2]);
        for i in 0..4 {
            if i <= chunk.len() {
                out.push(ALPHABET[((n >> (18 - 6 * i)) & 63) as usize] as char);
            } else {
                out.push('=');
            }
        }
    }
    out
}

/// What a tool call returns.
///
/// `is_error` carries a **tool-level** failure — the tool ran and could not do
/// the job — which is a different thing from a JSON-RPC error, where the call
/// itself was malformed. Collapsing the two would leave a caller unable to tell
/// "no such tool" from "that search found nothing it could use".
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ToolResult {
    pub content: Vec<Content>,
    #[serde(rename = "isError", default, skip_serializing_if = "Option::is_none")]
    pub is_error: Option<bool>,
    /// A structured answer, when the tool's schema declares one (MCP 2025-06-18).
    #[serde(rename = "structuredContent", default, skip_serializing_if = "Option::is_none")]
    pub structured_content: Option<Value>,
}

impl ToolResult {
    pub fn ok(content: Vec<Content>) -> Self {
        Self {
            content,
            is_error: None,
            structured_content: None,
        }
    }

    pub fn failed(message: impl Into<String>) -> Self {
        Self {
            content: vec![Content::text(message)],
            is_error: Some(true),
            structured_content: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn a_request_round_trips() {
        let src = r#"{"jsonrpc":"2.0","id":1,"method":"tools/list","params":{"cursor":"abc"}}"#;
        let req: Request = serde_json::from_str(src).unwrap();
        assert_eq!(req.method, "tools/list");
        assert!(!req.is_notification());
        let back = serde_json::to_string(&req).unwrap();
        let again: Request = serde_json::from_str(&back).unwrap();
        assert_eq!(req, again);
    }

    #[test]
    fn a_notification_has_no_id_and_expects_no_reply() {
        let src = r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#;
        let req: Request = serde_json::from_str(src).unwrap();
        assert!(req.is_notification());
        // Round-tripping must not invent an id.
        let back = serde_json::to_string(&req).unwrap();
        assert!(!back.contains("\"id\""), "an id was invented: {back}");
    }

    /// A body this crate does not model survives decode + encode unchanged.
    ///
    /// Without this a field from a newer protocol revision would be silently
    /// dropped, and the peer would see us answer a request we had quietly
    /// rewritten.
    #[test]
    fn an_unmodelled_body_survives_a_round_trip() {
        let src = r#"{"jsonrpc":"2.0","id":7,"method":"some/future","params":{"nested":{"kept":[1,2,{"deep":true}]},"extra":"yes"}}"#;
        let req: Request = serde_json::from_str(src).unwrap();
        let params = req.params.clone().unwrap();
        let back = serde_json::to_string(&req).unwrap();
        let again: Request = serde_json::from_str(&back).unwrap();
        assert_eq!(again.params.unwrap(), params, "params were not preserved");
        assert_eq!(again.method, "some/future");
    }

    #[test]
    fn a_response_carries_result_or_error_but_never_both() {
        let ok = Response::ok(serde_json::json!(1), serde_json::json!({"a":1}));
        let text = serde_json::to_string(&ok).unwrap();
        assert!(text.contains("result"), "{text}");
        assert!(!text.contains("error"), "{text}");

        let bad = Response::err(serde_json::json!(1), error_code::METHOD_NOT_FOUND, "nope");
        let text = serde_json::to_string(&bad).unwrap();
        assert!(text.contains("error"), "{text}");
        assert!(!text.contains("\"result\""), "{text}");
    }

    #[test]
    fn negotiation_echoes_a_revision_we_speak_and_offers_ours_otherwise() {
        assert_eq!(negotiate(Some("2024-11-05")), "2024-11-05", "echo the client");
        assert_eq!(negotiate(Some("1999-01-01")), PROTOCOL_VERSION, "offer ours");
        assert_eq!(negotiate(None), PROTOCOL_VERSION);
    }

    /// An image travels as MCP's `{"type":"image","data":…,"mimeType":…}`.
    #[test]
    fn an_image_content_has_the_mcp_shape() {
        let v = serde_json::to_value(Content::png(&[0x89, b'P', b'N', b'G'])).unwrap();
        assert_eq!(v["type"], "image");
        assert_eq!(v["mimeType"], "image/png");
        assert_eq!(v["data"], "iVBORw==");
        // RFC 4648 test vectors, every padding case.
        let vectors = [("", ""), ("f", "Zg=="), ("fo", "Zm8="), ("foo", "Zm9v"), ("foobar", "Zm9vYmFy")];
        for (plain, encoded) in vectors {
            assert_eq!(base64(plain.as_bytes()), encoded, "base64({plain:?})");
        }
        println!("image content: MCP shape checked; base64 matched {} RFC 4648 vectors", vectors.len());
    }

    #[test]
    fn a_tool_result_distinguishes_failure_from_emptiness() {
        let empty = ToolResult::ok(vec![Content::text("no matching records")]);
        let failed = ToolResult::failed("the file could not be opened");
        assert_eq!(empty.is_error, None, "an empty result is not a failure");
        assert_eq!(failed.is_error, Some(true));
    }

    /// Spec 078: content this crate does not model decodes, and travels back
    /// out untouched, instead of failing the whole result.
    #[test]
    fn an_unknown_content_decodes_as_other_and_round_trips() {
        let src = r#"[{"type":"text","text":"hi"},{"type":"audio","data":"AAA","mimeType":"audio/mpeg"},{"type":"resource","resource":{"uri":"s3://b/k"}}]"#;
        let blocks: Vec<Content> = serde_json::from_str(src).unwrap();
        assert_eq!(blocks[0], Content::text("hi"));
        assert!(matches!(&blocks[1], Content::Other(v) if v["type"] == "audio"));
        assert!(matches!(&blocks[2], Content::Other(v) if v["resource"]["uri"] == "s3://b/k"));
        let back: Value = serde_json::to_value(&blocks).unwrap();
        assert_eq!(back, serde_json::from_str::<Value>(src).unwrap());
        // An image still decodes as an image.
        let img: Content = serde_json::from_str(r#"{"type":"image","data":"QQ==","mimeType":"image/png"}"#).unwrap();
        assert_eq!(img, Content::Image { data: "QQ==".into(), mime_type: "image/png".into() });
        println!("content: text, image and 2 unmodelled block types decoded; the unmodelled ones round-tripped byte-equal");
    }

    #[test]
    fn message_classifies_request_notification_and_response() {
        let req = Message::parse(br#"{"jsonrpc":"2.0","id":3,"method":"ping"}"#).unwrap();
        assert!(matches!(&req, Message::Request(r) if !r.is_notification()));
        let note = Message::parse(br#"{"jsonrpc":"2.0","method":"notifications/progress","params":{}}"#).unwrap();
        assert!(matches!(&note, Message::Request(r) if r.is_notification()));
        let ok = Message::parse(br#"{"jsonrpc":"2.0","id":3,"result":{}}"#).unwrap();
        assert!(matches!(ok, Message::Response(_)));
        let err = Message::parse(br#"{"jsonrpc":"2.0","id":3,"error":{"code":-32601,"message":"no"}}"#).unwrap();
        assert!(matches!(err, Message::Response(r) if r.error.is_some()));
        assert!(Message::parse(br#"{"jsonrpc":"2.0","id":3}"#).is_err());
        assert!(Message::parse(b"not json").is_err());
    }

    #[test]
    fn a_tool_reports_its_read_only_hint() {
        let t: Tool = serde_json::from_str(r#"{"name":"q","inputSchema":{},"annotations":{"readOnlyHint":true}}"#).unwrap();
        assert!(t.is_read_only());
        let w: Tool = serde_json::from_str(r#"{"name":"w","inputSchema":{}}"#).unwrap();
        assert!(!w.is_read_only());
        let page: ListToolsResult = serde_json::from_str(r#"{"tools":[{"name":"q","inputSchema":{}}],"nextCursor":"p2"}"#).unwrap();
        assert_eq!(page.next_cursor.as_deref(), Some("p2"));
    }
}
