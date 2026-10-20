// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! The small HTTPS surface beside the WebSocket (WASM-003 Q3, WASM-013 R4):
//! the request head is read here, once, so a connection can be routed — to the
//! WebSocket upgrade, to the health probe, to a download — and the bytes read
//! are handed back to the WebSocket library untouched ([`Replay`]).

use std::io;
use std::pin::Pin;
use std::task::{Context, Poll};

use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt, ReadBuf};

/// The largest request head accepted.
pub const MAX_HEAD: usize = 16 * 1024;

/// A parsed request head.
#[derive(Debug, Clone)]
pub struct Head {
    pub method: String,
    pub path: String,
    pub query: String,
    pub headers: Vec<(String, String)>,
    /// The bytes of the head as read, to be replayed to the WebSocket library.
    pub raw: Vec<u8>,
}

impl Head {
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers.iter().find(|(k, _)| k.eq_ignore_ascii_case(name)).map(|(_, v)| v.as_str())
    }

    pub fn is_websocket_upgrade(&self) -> bool {
        self.method.eq_ignore_ascii_case("GET")
            && self.header("upgrade").is_some_and(|v| v.eq_ignore_ascii_case("websocket"))
            && self.header("connection").is_some_and(|v| v.to_ascii_lowercase().contains("upgrade"))
    }
}

/// What can go wrong reading a head.
#[derive(Debug)]
pub enum HeadError {
    Io(io::Error),
    TooLarge,
    Malformed,
    Closed,
}

/// Read bytes until a complete request head has arrived.
pub async fn read_head<S: AsyncRead + Unpin>(s: &mut S) -> Result<Head, HeadError> {
    let mut buf: Vec<u8> = Vec::with_capacity(1024);
    let mut chunk = [0u8; 1024];
    loop {
        let n = s.read(&mut chunk).await.map_err(HeadError::Io)?;
        if n == 0 {
            return Err(if buf.is_empty() { HeadError::Closed } else { HeadError::Malformed });
        }
        buf.extend_from_slice(&chunk[..n]);
        if buf.len() > MAX_HEAD {
            return Err(HeadError::TooLarge);
        }
        let mut headers = [httparse::EMPTY_HEADER; 64];
        let mut req = httparse::Request::new(&mut headers);
        match req.parse(&buf) {
            Ok(httparse::Status::Complete(_)) => {
                let target = req.path.unwrap_or("/");
                let (path, query) = match target.split_once('?') {
                    Some((p, q)) => (p.to_string(), q.to_string()),
                    None => (target.to_string(), String::new()),
                };
                let hs = req
                    .headers
                    .iter()
                    .map(|h| (h.name.to_string(), String::from_utf8_lossy(h.value).into_owned()))
                    .collect();
                return Ok(Head { method: req.method.unwrap_or("").to_string(), path, query, headers: hs, raw: buf });
            }
            Ok(httparse::Status::Partial) => continue,
            Err(_) => return Err(HeadError::Malformed),
        }
    }
}

/// A stream that first yields bytes already read, then the stream itself.
pub struct Replay<S> {
    prefix: std::io::Cursor<Vec<u8>>,
    inner: S,
}

impl<S> Replay<S> {
    pub fn new(prefix: Vec<u8>, inner: S) -> Self {
        Replay { prefix: std::io::Cursor::new(prefix), inner }
    }
}

impl<S: AsyncRead + Unpin> AsyncRead for Replay<S> {
    fn poll_read(mut self: Pin<&mut Self>, cx: &mut Context<'_>, buf: &mut ReadBuf<'_>) -> Poll<io::Result<()>> {
        let pos = self.prefix.position() as usize;
        let data = self.prefix.get_ref();
        if pos < data.len() {
            let n = (data.len() - pos).min(buf.remaining());
            buf.put_slice(&data[pos..pos + n]);
            self.prefix.set_position((pos + n) as u64);
            return Poll::Ready(Ok(()));
        }
        Pin::new(&mut self.inner).poll_read(cx, buf)
    }
}

impl<S: AsyncWrite + Unpin> AsyncWrite for Replay<S> {
    fn poll_write(mut self: Pin<&mut Self>, cx: &mut Context<'_>, buf: &[u8]) -> Poll<io::Result<usize>> {
        Pin::new(&mut self.inner).poll_write(cx, buf)
    }
    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.inner).poll_flush(cx)
    }
    fn poll_shutdown(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.inner).poll_shutdown(cx)
    }
}

/// The security headers every response of the HTTPS surface carries
/// (WASM-013 R4).
pub const SECURITY_HEADERS: &str = "Strict-Transport-Security: max-age=63072000; includeSubDomains\r\n\
X-Content-Type-Options: nosniff\r\n\
Referrer-Policy: no-referrer\r\n\
Permissions-Policy: geolocation=(), camera=(), microphone=()\r\n\
Cache-Control: no-store\r\n";

/// Write a complete response and close.
pub async fn respond<S: AsyncWrite + Unpin>(s: &mut S, status: u16, reason: &str, content_type: &str, body: &[u8], extra_headers: &str) {
    let head = format!(
        "HTTP/1.1 {status} {reason}\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nConnection: close\r\n{SECURITY_HEADERS}{extra_headers}\r\n",
        body.len()
    );
    let _ = s.write_all(head.as_bytes()).await;
    let _ = s.write_all(body).await;
    let _ = s.flush().await;
    let _ = s.shutdown().await;
}

/// Percent-decode a query component (enough for the signed-link and callback parameters).
pub fn query_param(query: &str, name: &str) -> Option<String> {
    for pair in query.split('&') {
        let (k, v) = pair.split_once('=').unwrap_or((pair, ""));
        if k == name {
            return Some(percent_decode(v));
        }
    }
    None
}

pub fn percent_decode(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        match b[i] {
            b'%' if i + 2 < b.len() => {
                let h = std::str::from_utf8(&b[i + 1..i + 3]).ok().and_then(|x| u8::from_str_radix(x, 16).ok());
                match h {
                    Some(v) => {
                        out.push(v);
                        i += 3;
                    }
                    None => {
                        out.push(b'%');
                        i += 1;
                    }
                }
            }
            b'+' => {
                out.push(b' ');
                i += 1;
            }
            c => {
                out.push(c);
                i += 1;
            }
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}
