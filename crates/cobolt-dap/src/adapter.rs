// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! The adapter half: what runs inside the debuggee.
//!
//! [`serve`] owns the request loop and nothing else — no COBOL, no interpreter.
//! A [`DapHandler`] supplies the semantics, which is what keeps the protocol
//! testable without a running program and lets the same loop serve an
//! interpreted form, a compiled application, or a fake in a test.

use std::io::Read;

use serde_json::Value;

use crate::link::{spawn_reader, FrameWriter, LinkEvent};
use crate::protocol::{ProtocolMessage, Request, Response};

/// What a handler decided about one request.
pub enum Reply {
    /// Answer now, with this body.
    Ok(Option<Value>),
    /// Refuse, with a reason the developer can read. The UI shows this text.
    Fail(String),
    /// The handler will send the response itself once it can — used when the
    /// answer needs the interpreter thread, which may be mid-statement. The
    /// loop moves on rather than blocking the link behind one slow request.
    Deferred,
}

impl Reply {
    /// A successful reply carrying a typed body.
    pub fn typed<T: serde::Serialize>(body: &T) -> Self {
        match serde_json::to_value(body) {
            Ok(v) => Self::Ok(Some(v)),
            Err(e) => Self::Fail(format!("could not serialise the response: {e}")),
        }
    }

    /// A successful reply with no body — DAP's `next`, `continue`, `pause`.
    pub fn empty() -> Self {
        Self::Ok(None)
    }
}

/// The semantics behind an adapter.
pub trait DapHandler: Send {
    /// Handle one request. `out` is the shared writer: use it to emit events,
    /// and to send the response yourself when returning [`Reply::Deferred`].
    fn handle(&mut self, req: &Request, out: &FrameWriter) -> Reply;

    /// The client went away. Release whatever the session held; the debuggee
    /// itself keeps running unless the handler decides otherwise.
    fn on_disconnect(&mut self, _out: &FrameWriter) {}

    /// A frame arrived that could not be parsed. The default reports it on the
    /// console and keeps the session up.
    fn on_malformed(&mut self, text: &str, out: &FrameWriter) {
        out.send_event(
            "output",
            Some(serde_json::json!({
                "category": "console",
                "output": format!("{text}\n"),
                "cobolChannel": "problems",
            })),
        );
    }
}

/// Run the request loop until the client disconnects.
///
/// Blocks the calling thread — the debuggee runs this on a dedicated thread, so
/// the interpreter is never inside it.
pub fn serve<R: Read + Send + 'static, H: DapHandler>(
    reader: R,
    out: FrameWriter,
    handler: &mut H,
) {
    let rx = spawn_reader(reader);
    for ev in rx {
        match ev {
            LinkEvent::Message(ProtocolMessage::Request(req)) => {
                let reply = handler.handle(&req, &out);
                let seq = out.next_seq();
                match reply {
                    Reply::Ok(body) => {
                        out.send_response(Response::ok(seq, &req, body));
                    }
                    Reply::Fail(message) => {
                        out.send_response(Response::fail(seq, &req, message));
                    }
                    Reply::Deferred => {}
                }
            }
            // A client's response to one of OUR reverse requests. We send none,
            // so there is nothing to correlate — but receiving one is not a
            // fault either.
            LinkEvent::Message(ProtocolMessage::Response(_))
            | LinkEvent::Message(ProtocolMessage::Event(_)) => {}
            LinkEvent::Malformed(text) => handler.on_malformed(&text, &out),
            LinkEvent::Closed(_) => break,
        }
    }
    handler.on_disconnect(&out);
    out.close();
}

#[cfg(test)]
mod adapter_tests {
    use super::*;
    use crate::transport::{read_frame, write_frame};
    use std::sync::{Arc, Mutex};

    #[derive(Clone, Default)]
    struct Sink(Arc<Mutex<Vec<u8>>>);
    impl std::io::Write for Sink {
        fn write(&mut self, b: &[u8]) -> std::io::Result<usize> {
            self.0.lock().unwrap().extend_from_slice(b);
            Ok(b.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    #[derive(Default)]
    struct Fake {
        seen: Vec<String>,
        disconnected: bool,
    }

    impl DapHandler for Fake {
        fn handle(&mut self, req: &Request, out: &FrameWriter) -> Reply {
            self.seen.push(req.command.clone());
            match req.command.as_str() {
                "initialize" => {
                    out.send_event("initialized", None);
                    Reply::typed(&crate::types::Capabilities {
                        supports_conditional_breakpoints: true,
                        ..Default::default()
                    })
                }
                "continue" => Reply::empty(),
                "readMemory" => Reply::Fail("raw memory access is not exposed".into()),
                "slow" => Reply::Deferred,
                other => Reply::Fail(format!("{other} is not implemented")),
            }
        }
        fn on_disconnect(&mut self, _out: &FrameWriter) {
            self.disconnected = true;
        }
    }

    fn run(requests: &[&str]) -> (Fake, Vec<ProtocolMessage>) {
        let mut raw = Vec::new();
        for r in requests {
            write_frame(&mut raw, r.as_bytes()).unwrap();
        }
        let sink = Sink::default();
        let out = FrameWriter::new(sink.clone());
        let mut handler = Fake::default();
        serve(std::io::Cursor::new(raw), out, &mut handler);

        let bytes = sink.0.lock().unwrap().clone();
        let mut cur = std::io::Cursor::new(bytes);
        let mut msgs = Vec::new();
        while let Some(body) = read_frame(&mut cur).unwrap() {
            msgs.push(ProtocolMessage::from_slice(&body).unwrap());
        }
        (handler, msgs)
    }

    #[test]
    fn a_request_is_answered_and_events_interleave() {
        let (handler, msgs) = run(&[
            r#"{"seq":1,"type":"request","command":"initialize"}"#,
            r#"{"seq":2,"type":"request","command":"continue"}"#,
        ]);
        assert_eq!(handler.seen, ["initialize", "continue"]);
        assert!(handler.disconnected, "EOF must tear the session down");

        assert!(matches!(&msgs[0], ProtocolMessage::Event(e) if e.event == "initialized"));
        let ProtocolMessage::Response(r) = &msgs[1] else {
            panic!("expected the initialize response, got {:?}", msgs[1]);
        };
        assert!(r.success && r.request_seq == 1 && r.command == "initialize");
        let ProtocolMessage::Response(r) = &msgs[2] else {
            panic!("expected the continue response");
        };
        assert!(r.success && r.request_seq == 2 && r.body.is_none());
    }

    /// The spec's rule at the protocol layer: an unavailable action is refused
    /// with a reason, never accepted silently.
    #[test]
    fn a_refusal_carries_its_reason() {
        let (_, msgs) = run(&[r#"{"seq":1,"type":"request","command":"readMemory"}"#]);
        let ProtocolMessage::Response(r) = &msgs[0] else {
            panic!("expected a response");
        };
        assert!(!r.success);
        assert_eq!(
            r.message.as_deref(),
            Some("raw memory access is not exposed")
        );
    }

    #[test]
    fn an_unknown_command_is_refused_rather_than_dropped() {
        let (_, msgs) = run(&[r#"{"seq":1,"type":"request","command":"whatIsThis"}"#]);
        let ProtocolMessage::Response(r) = &msgs[0] else {
            panic!("expected a response");
        };
        assert!(!r.success && r.request_seq == 1);
    }

    /// A deferred reply writes nothing now — the handler owns the answer. The
    /// loop must not invent an empty success in its place, or the client would
    /// see two responses to one request.
    #[test]
    fn a_deferred_reply_emits_nothing_from_the_loop() {
        let (handler, msgs) = run(&[r#"{"seq":1,"type":"request","command":"slow"}"#]);
        assert_eq!(handler.seen, ["slow"]);
        assert!(msgs.is_empty(), "the loop answered on the handler's behalf: {msgs:?}");
    }

    #[test]
    fn a_malformed_frame_is_reported_and_the_loop_continues() {
        let mut raw = Vec::new();
        write_frame(&mut raw, b"}{ nonsense").unwrap();
        write_frame(&mut raw, br#"{"seq":2,"type":"request","command":"continue"}"#).unwrap();

        let sink = Sink::default();
        let out = FrameWriter::new(sink.clone());
        let mut handler = Fake::default();
        serve(std::io::Cursor::new(raw), out, &mut handler);

        assert_eq!(handler.seen, ["continue"], "the good request still ran");
        let text = String::from_utf8(sink.0.lock().unwrap().clone()).unwrap();
        assert!(text.contains("problems"), "the bad frame was reported: {text}");
    }
}
