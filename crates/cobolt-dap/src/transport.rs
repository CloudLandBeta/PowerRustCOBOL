// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! DAP base framing: `Content-Length: N\r\n\r\n<utf-8 json>`.
//!
//! This is the one thing that makes the link **wire-compatible** rather than
//! merely DAP-shaped — a stock DAP client (VS Code's, say) reads exactly this
//! and nothing else. It replaces the older `@DBG <json>` line protocol, whose
//! framing was a newline: workable while both ends were ours, but it cannot
//! carry a payload containing a newline and no third-party client speaks it.
//!
//! Headers other than `Content-Length` are tolerated and ignored — adapters in
//! the wild still emit the long-deprecated `Content-Type` — and the header
//! block ends at the first empty line.

use std::io::{self, BufRead, Write};

/// The only header whose value we act on.
const CONTENT_LENGTH: &str = "content-length";

/// Refuse absurd frames rather than allocating whatever a peer claims. A
/// stack-trace response for a deeply nested PERFORM is a few hundred KB at
/// worst; 64 MB is far past any honest message and well short of a denial of
/// service.
const MAX_FRAME_BYTES: usize = 64 * 1024 * 1024;

/// Write one DAP frame: the header block, then the raw UTF-8 body.
///
/// Flushes — a debug adapter that buffers its response is a debugger that
/// hangs, and the peer has no way to know it should wait.
pub fn write_frame<W: Write>(w: &mut W, body: &[u8]) -> io::Result<()> {
    write!(w, "Content-Length: {}\r\n\r\n", body.len())?;
    w.write_all(body)?;
    w.flush()
}

/// Read one DAP frame, or `Ok(None)` at a clean end of stream.
///
/// A clean EOF **before any header** is the peer closing the link, which is
/// normal (the debuggee exited); EOF *mid-frame* is a truncated message and is
/// an error, because silently returning a short body would be indistinguishable
/// from a valid small one.
pub fn read_frame<R: BufRead>(r: &mut R) -> io::Result<Option<Vec<u8>>> {
    let mut len: Option<usize> = None;
    let mut saw_any_header = false;
    let mut line = String::new();

    loop {
        line.clear();
        if r.read_line(&mut line)? == 0 {
            return if saw_any_header {
                Err(io::Error::new(
                    io::ErrorKind::UnexpectedEof,
                    "stream ended inside a DAP header block",
                ))
            } else {
                Ok(None)
            };
        }
        let trimmed = line.trim_end_matches(['\r', '\n']);
        if trimmed.is_empty() {
            break; // end of the header block
        }
        saw_any_header = true;
        if let Some((name, value)) = trimmed.split_once(':') {
            if name.trim().eq_ignore_ascii_case(CONTENT_LENGTH) {
                len = Some(value.trim().parse::<usize>().map_err(|e| {
                    io::Error::new(
                        io::ErrorKind::InvalidData,
                        format!("bad Content-Length {:?}: {e}", value.trim()),
                    )
                })?);
            }
        }
    }

    let Some(len) = len else {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "DAP frame has no Content-Length header",
        ));
    };
    if len > MAX_FRAME_BYTES {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("DAP frame of {len} bytes exceeds the {MAX_FRAME_BYTES}-byte limit"),
        ));
    }

    let mut body = vec![0u8; len];
    r.read_exact(&mut body)?;
    Ok(Some(body))
}

#[cfg(test)]
mod framing_tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn a_frame_survives_a_round_trip() {
        let mut buf = Vec::new();
        write_frame(&mut buf, br#"{"seq":1,"type":"request"}"#).unwrap();
        assert_eq!(
            String::from_utf8_lossy(&buf),
            "Content-Length: 26\r\n\r\n{\"seq\":1,\"type\":\"request\"}"
        );
        let mut cur = Cursor::new(buf);
        assert_eq!(
            read_frame(&mut cur).unwrap().unwrap(),
            br#"{"seq":1,"type":"request"}"#
        );
    }

    /// The reason framing exists at all: a body containing a newline. The old
    /// `@DBG` line protocol could not carry one, which quietly capped what a
    /// debug event was allowed to say.
    #[test]
    fn a_body_may_contain_newlines() {
        let body = b"{\"a\":\"line one\nline two\"}";
        let mut buf = Vec::new();
        write_frame(&mut buf, body).unwrap();
        let mut cur = Cursor::new(buf);
        assert_eq!(read_frame(&mut cur).unwrap().unwrap(), body);
    }

    #[test]
    fn several_frames_read_back_in_order() {
        let mut buf = Vec::new();
        for n in 0..3 {
            write_frame(&mut buf, format!("{{\"seq\":{n}}}").as_bytes()).unwrap();
        }
        let mut cur = Cursor::new(buf);
        for n in 0..3 {
            let got = read_frame(&mut cur).unwrap().unwrap();
            assert_eq!(String::from_utf8_lossy(&got), format!("{{\"seq\":{n}}}"));
        }
        assert!(read_frame(&mut cur).unwrap().is_none(), "then a clean EOF");
    }

    #[test]
    fn unknown_headers_are_ignored_but_content_length_is_honoured() {
        let raw = "Content-Type: application/vnd.microsoft.dap\r\n\
                   Content-Length: 2\r\n\r\n{}";
        let mut cur = Cursor::new(raw.as_bytes().to_vec());
        assert_eq!(read_frame(&mut cur).unwrap().unwrap(), b"{}");
    }

    /// A closed link is not an error — the debuggee exiting is the normal end
    /// of a session, and the IDE must not report it as a protocol fault.
    #[test]
    fn a_clean_eof_is_none_not_an_error() {
        let mut cur = Cursor::new(Vec::new());
        assert!(read_frame(&mut cur).unwrap().is_none());
    }

    #[test]
    fn a_truncated_frame_is_an_error() {
        let raw = "Content-Length: 40\r\n\r\n{\"short\":true}";
        let mut cur = Cursor::new(raw.as_bytes().to_vec());
        assert!(read_frame(&mut cur).is_err(), "a short body must not pass");
    }

    #[test]
    fn a_header_block_without_content_length_is_an_error() {
        let raw = "Content-Type: whatever\r\n\r\n{}";
        let mut cur = Cursor::new(raw.as_bytes().to_vec());
        assert!(read_frame(&mut cur).is_err());
    }

    #[test]
    fn an_absurd_content_length_is_refused_without_allocating_it() {
        let raw = format!("Content-Length: {}\r\n\r\n", MAX_FRAME_BYTES + 1);
        let mut cur = Cursor::new(raw.into_bytes());
        assert!(read_frame(&mut cur).is_err());
    }
}
