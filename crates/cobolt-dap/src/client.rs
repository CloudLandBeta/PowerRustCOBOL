// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! The client half: what the IDE debugger window is.
//!
//! Nothing here blocks. The UI calls [`DapClient::poll`] once a frame, gets
//! whatever arrived, and returns — so a debuggee that is busy, wedged or gone
//! costs the IDE nothing. That is the whole reason the client/adapter boundary
//! exists rather than the debugger calling an interpreter directly.

use std::collections::HashMap;
use std::io::{Read, Write};
use std::sync::mpsc::{Receiver, TryRecvError};

use serde_json::Value;

use crate::link::{spawn_reader, FrameWriter, LinkEvent};
use crate::protocol::{Event, ProtocolMessage, Response};
use crate::types::{Capabilities, InitializeArguments, PROTOCOL_VERSION};

/// Where a session is. The title strip renders these distinctly, and the
/// toolbar decides what is clickable from them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SessionState {
    /// No session.
    #[default]
    Disconnected,
    /// The debuggee is starting but has not answered `initialize`.
    Launching,
    /// Initialized; capabilities negotiated.
    Connected,
    /// The program is executing.
    Running,
    /// A pause was requested and the debuggee has not stopped yet. Distinct
    /// from Paused because the developer needs to see that the click landed
    /// even though nothing has stopped — a long-running statement can take a
    /// while to reach its next safepoint.
    PausePending,
    /// Stopped at a safepoint. The only state in which data may be edited.
    Paused,
    /// The program ended.
    Terminated,
}

impl SessionState {
    /// Is there a live link at all?
    pub fn is_live(self) -> bool {
        matches!(
            self,
            Self::Launching | Self::Connected | Self::Running | Self::PausePending | Self::Paused
        )
    }

    /// May a stepping command be issued right now?
    pub fn can_step(self) -> bool {
        self == Self::Paused
    }

    /// May execution be resumed?
    pub fn can_continue(self) -> bool {
        self == Self::Paused
    }

    /// May a pause be requested?
    pub fn can_pause(self) -> bool {
        self == Self::Running
    }
}

/// One thing that happened on the link, ready for the UI to act on.
#[derive(Debug, Clone)]
pub enum ClientUpdate {
    /// A response to a request this client sent, with the command it answers.
    Response { command: String, response: Response },
    /// An unsolicited event.
    Event(Event),
    /// A frame we could not read. Non-fatal; belongs in Problems.
    Malformed(String),
    /// The link is gone. Terminal.
    Closed(Option<String>),
}

/// A DAP client over any pair of streams: a child process's pipes, a socket, or
/// an in-process pair in a test.
pub struct DapClient {
    writer: FrameWriter,
    rx: Receiver<LinkEvent>,
    /// seq → command, so a response can say what it answers. DAP puts the
    /// command on the response too, but only this map proves the response
    /// belongs to a request *we* sent.
    pending: HashMap<i64, String>,
    capabilities: Capabilities,
    state: SessionState,
}

impl DapClient {
    /// Build a client over an already-connected pair of streams.
    pub fn new<R: Read + Send + 'static, W: Write + Send + 'static>(reader: R, writer: W) -> Self {
        Self {
            writer: FrameWriter::new(writer),
            rx: spawn_reader(reader),
            pending: HashMap::new(),
            capabilities: Capabilities::default(),
            state: SessionState::Launching,
        }
    }

    pub fn state(&self) -> SessionState {
        self.state
    }

    /// The UI sets this on `stopped` / `continued` — the client cannot infer
    /// which one an event meant without duplicating the UI's own bookkeeping.
    pub fn set_state(&mut self, state: SessionState) {
        self.state = state;
    }

    /// What the adapter said it can do. Every gated control reads this.
    pub fn capabilities(&self) -> &Capabilities {
        &self.capabilities
    }

    pub fn set_capabilities(&mut self, caps: Capabilities) {
        self.capabilities = caps;
    }

    /// Send `initialize`, announcing the COBOL protocol revision we speak.
    pub fn initialize(&mut self, client_name: &str) -> Option<i64> {
        let args = InitializeArguments {
            client_id: Some("powerrustcobol".into()),
            client_name: Some(client_name.to_owned()),
            adapter_id: "powerrustcobol".into(),
            lines_start_at1: Some(true),
            columns_start_at1: Some(true),
            cobol_protocol_version: Some(PROTOCOL_VERSION),
        };
        self.request("initialize", serde_json::to_value(args).ok())
    }

    /// Send a request; returns its `seq`, or `None` if the link is dead.
    pub fn request(&mut self, command: &str, arguments: Option<Value>) -> Option<i64> {
        let seq = self.writer.send_request(command, arguments)?;
        self.pending.insert(seq, command.to_owned());
        Some(seq)
    }

    /// Send a request built from a typed argument struct.
    pub fn request_typed<T: serde::Serialize>(&mut self, command: &str, args: &T) -> Option<i64> {
        self.request(command, serde_json::to_value(args).ok())
    }

    /// Drain everything that arrived since the last call. Never blocks.
    pub fn poll(&mut self) -> Vec<ClientUpdate> {
        let mut out = Vec::new();
        loop {
            match self.rx.try_recv() {
                Ok(LinkEvent::Message(ProtocolMessage::Response(resp))) => {
                    // Prefer what we recorded when sending; fall back to the
                    // response's own command so a reply to a request we have
                    // forgotten is still usable.
                    let command = self
                        .pending
                        .remove(&resp.request_seq)
                        .unwrap_or_else(|| resp.command.clone());
                    out.push(ClientUpdate::Response {
                        command,
                        response: resp,
                    });
                }
                Ok(LinkEvent::Message(ProtocolMessage::Event(ev))) => {
                    out.push(ClientUpdate::Event(ev));
                }
                Ok(LinkEvent::Message(ProtocolMessage::Request(req))) => {
                    // Reverse requests (`runInTerminal`, `startDebugging`) are
                    // the adapter asking the client to do something. We
                    // implement none, and DAP requires an answer either way —
                    // silence would wedge a conforming adapter.
                    let seq = self.writer.next_seq();
                    self.writer.send_response(Response::fail(
                        seq,
                        &req,
                        format!("{} is not supported by this client", req.command),
                    ));
                }
                Ok(LinkEvent::Malformed(text)) => out.push(ClientUpdate::Malformed(text)),
                Ok(LinkEvent::Closed(why)) => {
                    self.writer.close();
                    self.state = SessionState::Terminated;
                    out.push(ClientUpdate::Closed(why));
                    // Terminal: the reader thread is finished, and any later
                    // try_recv would only report an empty disconnected channel.
                    break;
                }
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => break,
            }
        }
        out
    }

    /// Stop talking. Does not kill the debuggee — that is `disconnect` with
    /// `terminateDebuggee`, a request the caller sends first if it wants it.
    pub fn close(&mut self) {
        self.writer.close();
        self.state = SessionState::Disconnected;
    }
}

#[cfg(test)]
mod client_tests {
    use super::*;
    use crate::transport::write_frame;

    fn frames(msgs: &[&str]) -> Vec<u8> {
        let mut raw = Vec::new();
        for m in msgs {
            write_frame(&mut raw, m.as_bytes()).unwrap();
        }
        raw
    }

    /// Drain until the channel yields the close, so a test never races the
    /// reader thread.
    fn poll_until_closed(client: &mut DapClient) -> Vec<ClientUpdate> {
        let mut all = Vec::new();
        for _ in 0..1000 {
            all.extend(client.poll());
            if all.iter().any(|u| matches!(u, ClientUpdate::Closed(_))) {
                break;
            }
            std::thread::yield_now();
        }
        all
    }

    #[test]
    fn a_response_is_matched_to_the_command_that_asked_for_it() {
        let incoming = frames(&[
            r#"{"seq":1,"type":"response","request_seq":1,"success":true,"command":"initialize","body":{"supportsConditionalBreakpoints":true}}"#,
        ]);
        let mut client = DapClient::new(std::io::Cursor::new(incoming), Vec::new());
        assert_eq!(client.initialize("test"), Some(1));

        let updates = poll_until_closed(&mut client);
        let ClientUpdate::Response { command, response } = &updates[0] else {
            panic!("expected a response, got {:?}", updates[0]);
        };
        assert_eq!(command, "initialize");
        let caps: Capabilities = response.body_as().unwrap();
        assert!(caps.supports_conditional_breakpoints);
    }

    /// The requirement: a debuggee that vanishes ends the session cleanly and
    /// leaves the client usable — no panic, no block, no repeated errors.
    #[test]
    fn a_lost_debuggee_terminates_the_session_without_freezing() {
        let mut client = DapClient::new(std::io::Cursor::new(Vec::new()), Vec::new());
        let updates = poll_until_closed(&mut client);
        assert!(matches!(updates.last(), Some(ClientUpdate::Closed(None))));
        assert_eq!(client.state(), SessionState::Terminated);
        // And the client still answers, rather than panicking on a dead link.
        assert_eq!(client.request("threads", None), None);
        assert!(client.poll().is_empty());
    }

    /// A reverse request must be answered, not ignored — an adapter waiting on
    /// a reply we never send would hang the session.
    #[test]
    fn an_unsupported_reverse_request_is_refused_rather_than_ignored() {
        let incoming =
            frames(&[r#"{"seq":1,"type":"request","command":"runInTerminal","arguments":{}}"#]);
        let sink = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        struct Tee(std::sync::Arc<std::sync::Mutex<Vec<u8>>>);
        impl std::io::Write for Tee {
            fn write(&mut self, b: &[u8]) -> std::io::Result<usize> {
                self.0.lock().unwrap().extend_from_slice(b);
                Ok(b.len())
            }
            fn flush(&mut self) -> std::io::Result<()> {
                Ok(())
            }
        }
        let mut client = DapClient::new(std::io::Cursor::new(incoming), Tee(sink.clone()));
        poll_until_closed(&mut client);

        let written = String::from_utf8(sink.lock().unwrap().clone()).unwrap();
        assert!(written.contains(r#""success":false"#), "{written}");
        assert!(written.contains("runInTerminal"), "{written}");
    }

    #[test]
    fn state_gates_match_the_toolbar_rules() {
        use SessionState::*;
        assert!(Running.can_pause() && !Running.can_step() && !Running.can_continue());
        assert!(Paused.can_step() && Paused.can_continue() && !Paused.can_pause());
        assert!(!PausePending.can_pause(), "a second pause is not a command");
        assert!(!Terminated.is_live() && !Disconnected.is_live());
        assert!(Paused.is_live() && Running.is_live());
    }
}
