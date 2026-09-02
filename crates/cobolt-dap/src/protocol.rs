// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! The DAP message envelope: request, response, event.
//!
//! Bodies are [`serde_json::Value`] here and typed in [`crate::types`]. That
//! split is deliberate: an adapter must be able to *forward* or *reject* a
//! command it does not implement without failing to parse it, and a client must
//! survive an event a newer adapter invented. Everything we actually implement
//! has a typed body; everything else still round-trips.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::sync::atomic::{AtomicI64, Ordering};

/// Deserialize an optional DAP payload into a typed struct.
///
/// The fallback for an absent payload is an **empty object**, not `null`:
/// serde refuses `null` for a struct even when every field is optional, and DAP
/// omits `arguments` outright for commands that take none (`threads`,
/// `configurationDone`). Falling back to `null` rejected traffic that is
/// perfectly legal.
fn payload<T: serde::de::DeserializeOwned>(v: &Option<Value>) -> serde_json::Result<T> {
    serde_json::from_value(
        v.clone()
            .unwrap_or_else(|| Value::Object(serde_json::Map::new())),
    )
}

/// One message on the wire. Internally tagged by `type`, exactly as DAP
/// specifies.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum ProtocolMessage {
    Request(Request),
    Response(Response),
    Event(Event),
}

impl ProtocolMessage {
    /// Parse one frame body.
    pub fn from_slice(bytes: &[u8]) -> serde_json::Result<Self> {
        serde_json::from_slice(bytes)
    }

    /// Render for the wire.
    pub fn to_vec(&self) -> serde_json::Result<Vec<u8>> {
        serde_json::to_vec(self)
    }

    /// The sequence number this message carries, whatever its shape.
    pub fn seq(&self) -> i64 {
        match self {
            Self::Request(r) => r.seq,
            Self::Response(r) => r.seq,
            Self::Event(e) => e.seq,
        }
    }
}

/// A client → adapter command.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Request {
    pub seq: i64,
    pub command: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub arguments: Option<Value>,
}

impl Request {
    pub fn new(seq: i64, command: impl Into<String>, arguments: Option<Value>) -> Self {
        Self {
            seq,
            command: command.into(),
            arguments,
        }
    }

    /// Deserialize the arguments into a typed struct.
    ///
    /// Absent arguments parse as `null`, which is what serde needs to fill an
    /// all-optional struct — DAP omits `arguments` entirely for commands like
    /// `threads`, and treating that as an error would reject valid traffic.
    pub fn args<T: serde::de::DeserializeOwned>(&self) -> serde_json::Result<T> {
        payload(&self.arguments)
    }
}

/// An adapter → client reply. Always carries the `command` it answers and the
/// `request_seq` it answers, so a client that has several requests in flight
/// can match them without assuming ordering.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Response {
    pub seq: i64,
    pub request_seq: i64,
    pub success: bool,
    pub command: String,
    /// Present on failure: the short reason, shown to the developer.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub body: Option<Value>,
}

impl Response {
    /// A successful reply to `req`.
    pub fn ok(seq: i64, req: &Request, body: Option<Value>) -> Self {
        Self {
            seq,
            request_seq: req.seq,
            success: true,
            command: req.command.clone(),
            message: None,
            body,
        }
    }

    /// A refusal. The message is the developer-facing reason — "reverse
    /// execution needs snapshot recording", not "error 7".
    pub fn fail(seq: i64, req: &Request, message: impl Into<String>) -> Self {
        Self {
            seq,
            request_seq: req.seq,
            success: false,
            command: req.command.clone(),
            message: Some(message.into()),
            body: None,
        }
    }

    /// Deserialize the body into a typed struct.
    pub fn body_as<T: serde::de::DeserializeOwned>(&self) -> serde_json::Result<T> {
        payload(&self.body)
    }
}

/// An adapter → client notification, unsolicited.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Event {
    pub seq: i64,
    pub event: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub body: Option<Value>,
}

impl Event {
    pub fn new(seq: i64, event: impl Into<String>, body: Option<Value>) -> Self {
        Self {
            seq,
            event: event.into(),
            body,
        }
    }

    /// Build an event from a typed body.
    pub fn typed<T: Serialize>(seq: i64, event: impl Into<String>, body: &T) -> Self {
        Self::new(seq, event, serde_json::to_value(body).ok())
    }

    pub fn body_as<T: serde::de::DeserializeOwned>(&self) -> serde_json::Result<T> {
        payload(&self.body)
    }
}

/// A monotonic `seq` source, shared by every writer on one link.
///
/// DAP numbers messages per *sender*, and both ends of a session send. One
/// counter per link end keeps the numbering legal without coordination.
#[derive(Debug, Default)]
pub struct SeqCounter(AtomicI64);

impl SeqCounter {
    /// Start at 1 — DAP's first message is `seq: 1`, and 0 reads as "unset".
    pub fn new() -> Self {
        Self(AtomicI64::new(1))
    }

    pub fn next(&self) -> i64 {
        self.0.fetch_add(1, Ordering::Relaxed)
    }
}

#[cfg(test)]
mod envelope_tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn a_request_round_trips_through_the_wire_shape() {
        let msg = ProtocolMessage::Request(Request::new(
            1,
            "setBreakpoints",
            Some(json!({"source": {"path": "form.cbl"}})),
        ));
        let bytes = msg.to_vec().unwrap();
        let text = String::from_utf8(bytes.clone()).unwrap();
        assert!(text.contains(r#""type":"request""#), "{text}");
        assert_eq!(ProtocolMessage::from_slice(&bytes).unwrap(), msg);
    }

    #[test]
    fn a_response_and_an_event_round_trip_too() {
        for msg in [
            ProtocolMessage::Response(Response {
                seq: 2,
                request_seq: 1,
                success: true,
                command: "threads".into(),
                message: None,
                body: Some(json!({"threads": []})),
            }),
            ProtocolMessage::Event(Event::new(3, "stopped", Some(json!({"reason": "breakpoint"})))),
        ] {
            let bytes = msg.to_vec().unwrap();
            assert_eq!(ProtocolMessage::from_slice(&bytes).unwrap(), msg);
        }
    }

    /// A command with no arguments is legal DAP; `args()` must not treat the
    /// missing key as a parse failure.
    #[test]
    fn absent_arguments_parse_as_an_all_default_struct() {
        #[derive(Deserialize, Default, PartialEq, Debug)]
        struct Empty {
            #[serde(default)]
            all: Option<String>,
        }
        let req = Request::new(1, "threads", None);
        assert_eq!(req.args::<Empty>().unwrap(), Empty::default());
    }

    #[test]
    fn a_failed_response_carries_the_reason_and_no_body() {
        let req = Request::new(9, "readMemory", None);
        let resp = Response::fail(10, &req, "raw memory access is not exposed");
        assert!(!resp.success);
        assert_eq!(resp.request_seq, 9);
        assert_eq!(resp.command, "readMemory");
        assert!(resp.body.is_none());
        assert!(resp.message.unwrap().contains("raw memory"));
    }

    /// An adapter we have not taught a command must still be able to read the
    /// message — a hard parse error would take the whole link down.
    #[test]
    fn an_unknown_command_still_parses() {
        let raw = br#"{"seq":4,"type":"request","command":"someFutureThing","arguments":{"x":1}}"#;
        let ProtocolMessage::Request(req) = ProtocolMessage::from_slice(raw).unwrap() else {
            panic!("expected a request");
        };
        assert_eq!(req.command, "someFutureThing");
    }

    #[test]
    fn seq_numbers_start_at_one_and_advance() {
        let seq = SeqCounter::new();
        assert_eq!(seq.next(), 1);
        assert_eq!(seq.next(), 2);
        assert_eq!(seq.next(), 3);
    }
}
