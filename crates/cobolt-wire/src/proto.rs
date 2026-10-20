// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! The messages on the WebSocket (WASM-003 R6, WASM-014).
//!
//! One JSON text per frame. The first member of every message is `"t"`, its
//! kind; the rest are the kind's own. A payload — a group's JSON — travels
//! **embedded**, not as a string, and is carried as raw text so the exact
//! decimal digits of a number reach the mapping untouched.
//!
//! The first exchange (`hello`) is the one thing that never changes shape from
//! one protocol version to the next: a client older than the backend's window
//! reads a refusal it can understand, never a decoding error (WASM-014 R3).

use serde::{Deserialize, Serialize};
use serde_json::value::RawValue;

use crate::error::{CallError, ErrorClass};

/// A kind this build does not know is answered with an error, not a close.
pub const MAX_ID: u64 = u64::MAX;

// ── client -> server ──────────────────────────────────────────────────────────

/// The first message of a connection (WASM-014 R2).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Hello {
    #[serde(default)]
    pub t: String,
    /// The wire protocol the client speaks.
    pub protocol: u32,
    /// The contract version it was compiled against, `major.minor.patch`.
    pub contract: String,
    /// Who it is, for logs (`prc-wasm/2.0.0`, `rcrun/2.0.0`).
    #[serde(default)]
    pub client: String,
}

/// The bearer token of the connection (WASM-003 R9, R11). Sent as a message,
/// never in a URL, a header or a sub-protocol, where proxies and logs keep it.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Auth {
    pub t: String,
    pub token: String,
}

/// How an argument is passed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Mode {
    Value,
    Ref,
}

/// One argument: a group's JSON, embedded.
#[derive(Debug, Serialize, Deserialize)]
pub struct Arg {
    pub mode: Mode,
    pub json: Box<RawValue>,
}

/// A call to an exported program.
#[derive(Debug, Serialize, Deserialize)]
pub struct Call {
    pub t: String,
    pub id: u64,
    pub program: String,
    #[serde(default)]
    pub args: Vec<Arg>,
    #[serde(default)]
    pub returning: bool,
    /// An idempotency key (WASM-014 R10): a repeat with the same key and the
    /// same payload is answered from the stored result, not run again.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub key: Option<String>,
    /// A token for this call alone, instead of the connection's.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub token: Option<String>,
    /// The client's own timeout, so the backend can stop work nobody awaits.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timeout_ms: Option<u64>,
}

/// Cancel a long-running call (WASM-014 R13).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Cancel {
    pub t: String,
    pub id: u64,
}

/// Subscribe to a stream (WASM-014 R14).
#[derive(Debug, Serialize, Deserialize)]
pub struct Subscribe {
    pub t: String,
    /// The client's id for this subscription.
    pub id: u64,
    /// The stream program.
    pub program: String,
    #[serde(default)]
    pub args: Vec<Arg>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Unsubscribe {
    pub t: String,
    pub id: u64,
}

/// Begin an upload (WASM-014 R15).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PutBegin {
    pub t: String,
    pub id: u64,
    /// Which upload slot (a name the backend declared), never a path.
    pub slot: String,
    /// The browser's file name; the backend stores under its own name.
    pub name: String,
    pub size: u64,
    pub mime: String,
    /// Hex SHA-256 of the whole file.
    pub sha256: String,
    /// Resume the upload with this id after a lost connection.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resume: Option<String>,
}

/// One piece of an upload; `data` is base64.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PutChunk {
    pub t: String,
    pub id: u64,
    pub offset: u64,
    pub data: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PutEnd {
    pub t: String,
    pub id: u64,
}

/// Ask for a download address for a file the backend holds (WASM-014 R15).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetFile {
    pub t: String,
    pub id: u64,
    /// The backend's own name for the file (what an earlier reply gave).
    pub file: String,
}

// ── server -> client ──────────────────────────────────────────────────────────

/// What the backend decided about the connection.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Accepted {
    Ok,
    /// The client is older than the backend but compatible: connected, with a warning.
    Warn,
    Refused,
}

/// The backend's answer to [`Hello`] (WASM-014 R2).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerHello {
    pub t: String,
    pub protocol: u32,
    pub protocol_min: u32,
    pub contract: String,
    pub accepted: Accepted,
    pub message: String,
    /// For a refusal, the version the client must update to.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub update_to: Option<String>,
    pub server: String,
    /// The limits the client should respect, so it can fail early.
    #[serde(default)]
    pub max_frame_bytes: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthOk {
    pub t: String,
    /// The subject, for the application's display; no other claim.
    pub sub: String,
    /// Seconds until the token expires.
    pub expires_in: i64,
}

/// A call's answer.
#[derive(Debug, Serialize, Deserialize)]
pub struct Reply {
    pub t: String,
    pub id: u64,
    /// One per argument: the callee's final JSON for a by-reference one.
    pub args: Vec<Option<Box<RawValue>>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub returning: Option<Box<RawValue>>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub warnings: Vec<String>,
}

/// A failure, of a call (`id` set) or of the connection (`id` absent).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorMsg {
    pub t: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<u64>,
    pub class: String,
    pub code: u32,
    pub message: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub may_have_run: Option<bool>,
}

impl ErrorMsg {
    pub fn from_error(id: Option<u64>, e: &CallError) -> Self {
        ErrorMsg {
            t: "error".into(),
            id,
            class: e.class.wire().to_string(),
            code: e.code,
            message: e.message.clone(),
            detail: e.detail.clone(),
            may_have_run: e.may_have_run,
        }
    }

    pub fn into_error(self) -> CallError {
        CallError {
            class: ErrorClass::from_wire(&self.class),
            code: self.code,
            message: self.message,
            detail: self.detail,
            may_have_run: self.may_have_run,
        }
    }
}

/// Progress of a long-running call (WASM-014 R13).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Progress {
    pub t: String,
    pub id: u64,
    pub percent: u8,
    pub message: String,
}

/// One item of a stream (WASM-014 R14).
#[derive(Debug, Serialize, Deserialize)]
pub struct Event {
    pub t: String,
    pub sub: u64,
    pub seq: u64,
    pub data: Box<RawValue>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Subscribed {
    pub t: String,
    pub id: u64,
}

/// The backend accepted an upload begin: how much it already holds (resume).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PutReady {
    pub t: String,
    pub id: u64,
    /// The id to resume with after a lost connection.
    pub upload: String,
    /// Bytes already stored: the next chunk starts here.
    pub have: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PutDone {
    pub t: String,
    pub id: u64,
    /// The backend's own name for the stored file.
    pub file: String,
    pub size: u64,
}

/// A short-lived, per-request download address.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileUrl {
    pub t: String,
    pub id: u64,
    pub path: String,
    pub expires_in: u64,
    pub size: u64,
    pub sha256: String,
}

// ── parsing a frame ───────────────────────────────────────────────────────────

#[derive(Deserialize)]
struct Tag<'a> {
    #[serde(borrow)]
    t: &'a str,
}

/// A frame from the client, by kind.
#[derive(Debug)]
pub enum ClientFrame {
    Hello(Hello),
    Auth(Auth),
    Call(Call),
    Cancel(Cancel),
    Subscribe(Subscribe),
    Unsubscribe(Unsubscribe),
    PutBegin(PutBegin),
    PutChunk(PutChunk),
    PutEnd(PutEnd),
    GetFile(GetFile),
}

/// Parse one client frame. The `t` field is read first, then the whole message
/// as that kind — a payload never passes through a generic JSON value, so its
/// numbers keep every digit.
pub fn parse_client(text: &str) -> Result<ClientFrame, CallError> {
    let bad = |m: String| CallError::new(ErrorClass::Validation, crate::error::codes::BAD_MESSAGE, m);
    let tag: Tag<'_> = serde_json::from_str(text).map_err(|e| bad(format!("not a message: {e}")))?;
    macro_rules! kind {
        ($ty:ty, $variant:ident) => {{
            let mut m: $ty = serde_json::from_str(text).map_err(|e| bad(format!("a {} message does not fit: {e}", tag.t)))?;
            let _ = &mut m;
            Ok(ClientFrame::$variant(m))
        }};
    }
    match tag.t {
        "hello" => {
            let mut h: Hello = serde_json::from_str(text).map_err(|e| bad(format!("a hello does not fit: {e}")))?;
            h.t = "hello".into();
            Ok(ClientFrame::Hello(h))
        }
        "auth" => kind!(Auth, Auth),
        "call" => kind!(Call, Call),
        "cancel" => kind!(Cancel, Cancel),
        "sub" => kind!(Subscribe, Subscribe),
        "unsub" => kind!(Unsubscribe, Unsubscribe),
        "put-begin" => kind!(PutBegin, PutBegin),
        "put-chunk" => kind!(PutChunk, PutChunk),
        "put-end" => kind!(PutEnd, PutEnd),
        "get-file" => kind!(GetFile, GetFile),
        other => Err(bad(format!("unknown message kind \"{other}\""))),
    }
}

/// Serialise any message to its frame text.
pub fn to_text<T: Serialize>(m: &T) -> String {
    serde_json::to_string(m).unwrap_or_else(|_| "{\"t\":\"error\",\"class\":\"server\",\"code\":9000,\"message\":\"unserialisable message\"}".into())
}
