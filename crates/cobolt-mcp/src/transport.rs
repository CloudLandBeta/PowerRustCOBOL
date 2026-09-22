// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Message framing over a byte stream the caller owns.
//!
//! MCP frames each JSON-RPC message as one line: the message, then a newline,
//! and no embedded newlines inside it. `serde_json` never emits a bare newline
//! inside a compact document, so writing is simply "serialize, then terminate".
//!
//! Generic over [`BufRead`] and [`Write`] on purpose (spec 065 R4). stdio, a
//! socket and an HTTP body all satisfy those, so the host picks the transport
//! and this crate stays free of every dependency that choice would otherwise
//! imply.

use std::io::{self, BufRead, Write};

/// Refuse an absurd frame rather than allocating whatever a peer claims.
///
/// A tool result carrying a few hundred records is well under this; anything
/// past it is either a mistake or an attempt to exhaust memory, and neither
/// deserves the allocation.
pub const MAX_MESSAGE_BYTES: usize = 16 * 1024 * 1024;

/// Write one framed message.
///
/// Flushes. A server that buffers its reply is a server that hangs, and the
/// peer has no way to know it should keep waiting.
pub fn write_message<W: Write>(w: &mut W, body: &[u8]) -> io::Result<()> {
    w.write_all(body)?;
    w.write_all(b"\n")?;
    w.flush()
}

/// Read one framed message, or `Ok(None)` at a clean end of stream.
///
/// A clean EOF **between** messages is the peer closing the link, which is
/// normal. EOF part-way through a line is a truncated message and is an error:
/// returning the partial bytes would be indistinguishable from a valid short
/// message, and the caller would parse nonsense instead of reporting a broken
/// link.
///
/// Blank lines are skipped rather than treated as empty messages — some peers
/// pad, and an empty line is not a JSON-RPC document.
pub fn read_message<R: BufRead>(r: &mut R) -> io::Result<Option<Vec<u8>>> {
    let mut line = Vec::new();
    loop {
        line.clear();
        let mut taken = 0usize;
        // Read bounded, so a peer that never sends a newline cannot make us
        // grow without limit.
        loop {
            let available = r.fill_buf()?;
            if available.is_empty() {
                // EOF. Between messages this is a clean close; mid-line it is a
                // truncation.
                if line.is_empty() {
                    return Ok(None);
                }
                return Err(io::Error::new(
                    io::ErrorKind::UnexpectedEof,
                    "stream ended part-way through a message",
                ));
            }
            match available.iter().position(|b| *b == b'\n') {
                Some(idx) => {
                    line.extend_from_slice(&available[..idx]);
                    r.consume(idx + 1);
                    break;
                }
                None => {
                    taken += available.len();
                    if taken > MAX_MESSAGE_BYTES {
                        return Err(io::Error::new(
                            io::ErrorKind::InvalidData,
                            format!("message exceeds {MAX_MESSAGE_BYTES} bytes"),
                        ));
                    }
                    line.extend_from_slice(available);
                    let n = available.len();
                    r.consume(n);
                }
            }
        }
        // Tolerate CRLF peers and padding.
        while line.last() == Some(&b'\r') {
            line.pop();
        }
        if line.iter().any(|b| !b.is_ascii_whitespace()) {
            return Ok(Some(line));
        }
        // Blank line: keep reading.
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn a_message_round_trips() {
        let mut out = Vec::new();
        write_message(&mut out, br#"{"jsonrpc":"2.0"}"#).unwrap();
        assert_eq!(out, b"{\"jsonrpc\":\"2.0\"}\n");

        let mut cursor = io::Cursor::new(out);
        let got = read_message(&mut cursor).unwrap().unwrap();
        assert_eq!(got, br#"{"jsonrpc":"2.0"}"#);
    }

    #[test]
    fn several_messages_read_in_order() {
        let mut out = Vec::new();
        write_message(&mut out, b"one").unwrap();
        write_message(&mut out, b"two").unwrap();
        let mut cursor = io::Cursor::new(out);
        assert_eq!(read_message(&mut cursor).unwrap().unwrap(), b"one");
        assert_eq!(read_message(&mut cursor).unwrap().unwrap(), b"two");
        assert_eq!(read_message(&mut cursor).unwrap(), None, "clean EOF");
    }

    #[test]
    fn a_clean_eof_is_not_an_error() {
        let mut cursor = io::Cursor::new(Vec::new());
        assert_eq!(read_message(&mut cursor).unwrap(), None);
    }

    /// A truncated message must not look like a short valid one.
    #[test]
    fn a_truncated_message_is_an_error() {
        let mut cursor = io::Cursor::new(b"{\"partial\":".to_vec());
        let err = read_message(&mut cursor).expect_err("truncation must be reported");
        assert_eq!(err.kind(), io::ErrorKind::UnexpectedEof);
    }

    #[test]
    fn crlf_and_blank_lines_are_tolerated() {
        let mut cursor = io::Cursor::new(b"\r\n\r\n{\"a\":1}\r\n".to_vec());
        assert_eq!(read_message(&mut cursor).unwrap().unwrap(), br#"{"a":1}"#);
    }

    #[test]
    fn an_oversized_message_is_refused_rather_than_allocated() {
        // One byte past the cap, with no newline in sight.
        let flood = vec![b'x'; MAX_MESSAGE_BYTES + 1];
        let mut cursor = io::Cursor::new(flood);
        let err = read_message(&mut cursor).expect_err("the cap must hold");
        assert_eq!(err.kind(), io::ErrorKind::InvalidData);
    }
}
