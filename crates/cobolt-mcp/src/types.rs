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
}

/// One piece of a tool's answer.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type")]
pub enum Content {
    #[serde(rename = "text")]
    Text { text: String },
    /// A picture, base64-encoded — a rendered form, for instance.
    #[serde(rename = "image")]
    Image {
        data: String,
        #[serde(rename = "mimeType")]
        mime_type: String,
    },
}

impl Content {
    pub fn text(s: impl Into<String>) -> Self {
        Content::Text { text: s.into() }
    }

    /// The text of a text block; `None` for an image.
    pub fn as_text(&self) -> Option<&str> {
        match self {
            Content::Text { text } => Some(text),
            Content::Image { .. } => None,
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
}

impl ToolResult {
    pub fn ok(content: Vec<Content>) -> Self {
        Self {
            content,
            is_error: None,
        }
    }

    pub fn failed(message: impl Into<String>) -> Self {
        Self {
            content: vec![Content::text(message)],
            is_error: Some(true),
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
}
