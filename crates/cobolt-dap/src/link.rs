// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! The shared write half of a DAP link, and the reader thread both ends use.
//!
//! Both ends of a session send: the client sends requests, the adapter sends
//! responses *and* unsolicited events from whichever thread produced them. So
//! the writer is shared and the `seq` counter with it. A lock is the whole
//! mechanism — frames are small and writes are rare next to the work that
//! produces them, and the alternative (a writer thread plus a queue) buys
//! nothing but a place for messages to be lost on shutdown.

use std::io::{BufRead, BufReader, Read, Write};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex};

use crate::protocol::{Event, ProtocolMessage, Request, Response, SeqCounter};
use crate::transport::{read_frame, write_frame};

/// A cloneable handle to the write half of a link.
#[derive(Clone)]
pub struct FrameWriter {
    inner: Arc<Mutex<Box<dyn Write + Send>>>,
    seq: Arc<SeqCounter>,
    /// Set once the peer is gone. Every later write becomes a cheap no-op
    /// instead of a fresh error on a dead pipe — a debuggee that exited is a
    /// normal end of session, and the IDE must not spend a frame per attempt
    /// rediscovering it.
    closed: Arc<AtomicBool>,
}

impl FrameWriter {
    pub fn new<W: Write + Send + 'static>(w: W) -> Self {
        Self {
            inner: Arc::new(Mutex::new(Box::new(w))),
            seq: Arc::new(SeqCounter::new()),
            closed: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Take the next sequence number for this end of the link.
    pub fn next_seq(&self) -> i64 {
        self.seq.next()
    }

    /// True once a write has failed or [`close`](Self::close) was called.
    pub fn is_closed(&self) -> bool {
        self.closed.load(Ordering::Relaxed)
    }

    /// Mark the link dead without writing anything.
    pub fn close(&self) {
        self.closed.store(true, Ordering::Relaxed);
    }

    /// Send one message. Returns false once the link is dead — callers treat
    /// that as "the session ended", never as a fault to report twice.
    pub fn send(&self, msg: &ProtocolMessage) -> bool {
        if self.is_closed() {
            return false;
        }
        let Ok(bytes) = msg.to_vec() else {
            // Our own message failed to serialise: a bug on this side, not a
            // dead peer. Do not kill the link for it.
            debug_assert!(false, "a DAP message we built would not serialise");
            return false;
        };
        let Ok(mut guard) = self.inner.lock() else {
            self.close();
            return false;
        };
        if write_frame(&mut *guard, &bytes).is_err() {
            drop(guard);
            self.close();
            return false;
        }
        true
    }

    /// Send a request and return the `seq` it was given, so the caller can
    /// match the response.
    pub fn send_request(&self, command: &str, arguments: Option<serde_json::Value>) -> Option<i64> {
        let seq = self.next_seq();
        self.send(&ProtocolMessage::Request(Request::new(seq, command, arguments)))
            .then_some(seq)
    }

    /// Reply to a request.
    pub fn send_response(&self, resp: Response) -> bool {
        self.send(&ProtocolMessage::Response(resp))
    }

    /// Emit an event with an already-built body.
    pub fn send_event(&self, event: &str, body: Option<serde_json::Value>) -> bool {
        let seq = self.next_seq();
        self.send(&ProtocolMessage::Event(Event::new(seq, event, body)))
    }

    /// Emit an event from a typed body.
    pub fn send_event_typed<T: serde::Serialize>(&self, event: &str, body: &T) -> bool {
        let seq = self.next_seq();
        self.send(&ProtocolMessage::Event(Event::typed(seq, event, body)))
    }
}

/// What a reader thread delivers.
#[derive(Debug, Clone, PartialEq)]
pub enum LinkEvent {
    /// A well-formed message arrived.
    Message(ProtocolMessage),
    /// A frame arrived that we could not parse. The link stays up — one bad
    /// message from a newer peer must not end the session — and the text is
    /// surfaced in the Problems tab.
    Malformed(String),
    /// The peer is gone. Terminal: nothing follows it on this channel.
    /// `None` is a clean end of stream, `Some` an I/O failure.
    Closed(Option<String>),
}

/// Spawn a reader that turns frames into [`LinkEvent`]s on a channel.
///
/// The channel is the reason the egui loop never blocks on the debuggee: the UI
/// drains what has arrived and returns, whatever the debuggee is doing.
pub fn spawn_reader<R: Read + Send + 'static>(reader: R) -> Receiver<LinkEvent> {
    let (tx, rx) = mpsc::channel();
    std::thread::Builder::new()
        .name("dap-reader".into())
        .spawn(move || read_loop(BufReader::new(reader), tx))
        .expect("spawning the DAP reader thread");
    rx
}

fn read_loop<R: BufRead>(mut reader: R, tx: Sender<LinkEvent>) {
    loop {
        match read_frame(&mut reader) {
            Ok(Some(body)) => {
                let ev = match ProtocolMessage::from_slice(&body) {
                    Ok(msg) => LinkEvent::Message(msg),
                    Err(e) => LinkEvent::Malformed(format!(
                        "unreadable DAP message ({e}): {}",
                        String::from_utf8_lossy(&body).chars().take(200).collect::<String>()
                    )),
                };
                if tx.send(ev).is_err() {
                    return; // the owner dropped the receiver; nothing to do
                }
            }
            Ok(None) => {
                let _ = tx.send(LinkEvent::Closed(None));
                return;
            }
            Err(e) => {
                let _ = tx.send(LinkEvent::Closed(Some(e.to_string())));
                return;
            }
        }
    }
}

#[cfg(test)]
mod link_tests {
    use super::*;
    use crate::protocol::Request;

    /// A `Vec<u8>` behind a shared lock, so a test can read what the writer
    /// produced while still holding the writer.
    #[derive(Clone, Default)]
    struct SharedSink(Arc<Mutex<Vec<u8>>>);

    impl Write for SharedSink {
        fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
            self.0.lock().unwrap().extend_from_slice(buf);
            Ok(buf.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    #[test]
    fn the_writer_frames_what_it_sends_and_numbers_it() {
        let sink = SharedSink::default();
        let w = FrameWriter::new(sink.clone());
        assert_eq!(w.send_request("initialize", None), Some(1));
        assert_eq!(w.send_request("threads", None), Some(2));

        let bytes = sink.0.lock().unwrap().clone();
        let mut cur = std::io::Cursor::new(bytes);
        for want_seq in [1, 2] {
            let body = read_frame(&mut cur).unwrap().unwrap();
            assert_eq!(ProtocolMessage::from_slice(&body).unwrap().seq(), want_seq);
        }
    }

    /// The requirement that keeps the IDE alive when a debuggee dies: a broken
    /// pipe closes the link once and every later send is a quiet false.
    #[test]
    fn a_failing_writer_closes_the_link_once_and_stays_closed() {
        struct Broken;
        impl Write for Broken {
            fn write(&mut self, _: &[u8]) -> std::io::Result<usize> {
                Err(std::io::Error::new(std::io::ErrorKind::BrokenPipe, "gone"))
            }
            fn flush(&mut self) -> std::io::Result<()> {
                Ok(())
            }
        }
        let w = FrameWriter::new(Broken);
        assert!(!w.is_closed());
        assert!(!w.send(&ProtocolMessage::Request(Request::new(1, "threads", None))));
        assert!(w.is_closed(), "one failure is enough to know");
        assert_eq!(w.send_request("threads", None), None);
    }

    #[test]
    fn the_reader_delivers_messages_then_exactly_one_close() {
        let mut raw = Vec::new();
        write_frame(&mut raw, br#"{"seq":1,"type":"event","event":"initialized"}"#).unwrap();
        write_frame(&mut raw, br#"{"seq":2,"type":"event","event":"terminated"}"#).unwrap();

        let rx = spawn_reader(std::io::Cursor::new(raw));
        let got: Vec<LinkEvent> = rx.iter().collect();
        assert_eq!(got.len(), 3, "two messages and a close: {got:?}");
        assert!(matches!(got[0], LinkEvent::Message(_)));
        assert!(matches!(got[1], LinkEvent::Message(_)));
        assert_eq!(got[2], LinkEvent::Closed(None));
    }

    /// One unreadable frame from a newer peer must not take the session down.
    #[test]
    fn a_malformed_frame_is_reported_and_the_link_survives() {
        let mut raw = Vec::new();
        write_frame(&mut raw, b"{not json").unwrap();
        write_frame(&mut raw, br#"{"seq":9,"type":"event","event":"terminated"}"#).unwrap();

        let rx = spawn_reader(std::io::Cursor::new(raw));
        let got: Vec<LinkEvent> = rx.iter().collect();
        assert!(matches!(got[0], LinkEvent::Malformed(_)), "{got:?}");
        assert!(matches!(got[1], LinkEvent::Message(_)), "the link kept reading");
        assert_eq!(got[2], LinkEvent::Closed(None));
    }
}
