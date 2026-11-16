// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! What the connection carries besides calls (WASM-014 T15, R13-R15): the progress of a long call, streams, files in
//! (`put-begin` / `put-chunk` / `put-end`, resumable) and files out (`get-file`, answered with an address the browser
//! navigates to). The state machine is pure: it sees frames and says what to send and what the host should do next;
//! reading the file's pieces and hashing them is the browser's work (`upload.rs` has the incremental hash).

use cobolt_wire::proto::{parse_server, ServerFrame};
use serde_json::{json, Value};

use super::{class, code, Client, LinkState, Out, Socket};

/// The largest piece of a file sent in one `put-chunk` (before base64).
pub const CHUNK_BYTES: usize = 192 * 1024;

/// A stream the page subscribed to.
#[derive(Debug)]
pub(super) struct Sub {
    /// The program and arguments as the module produced them (a call's frame without `t`, `id`, `key`).
    frame: String,
    sent: bool,
    ever_sent: bool,
    /// The next item is the first after a re-subscription.
    restart: bool,
}

/// An upload in progress.
#[derive(Debug)]
pub(super) struct Put {
    slot: String,
    name: String,
    mime: String,
    size: u64,
    sha256: String,
    /// The backend's id for the upload, once it has answered `put-begin`: the key to resume with.
    upload: Option<String>,
    sent: bool,
}

/// Why a file name or slot is refused before any frame leaves: it must be a plain name, never a path.
pub fn plain_name(name: &str) -> Result<(), String> {
    if name.is_empty() {
        return Err("the name is empty".into());
    }
    if name.len() > 255 {
        return Err("the name is longer than 255 bytes".into());
    }
    if name.chars().any(|c| c == '/' || c == '\\' || c == '\0' || c.is_control()) {
        return Err("the name holds a path separator or a control character".into());
    }
    if name == "." || name == ".." {
        return Err("the name is a directory reference".into());
    }
    Ok(())
}

impl<S: Socket> Client<S> {
    // ── streams ──────────────────────────────────────────────────────────────

    /// Subscribe to a stream program. `frame` is the module's frame for it (`program` and `args`, as for a call). Answers the
    /// subscription's id; items arrive as [`Out::StreamItem`] and the end as [`Out::StreamEnd`]. After a lost connection the
    /// subscription is made again and the first item that follows carries `restart`.
    pub fn subscribe(&mut self, frame: &[u8], now: u64) -> u32 {
        self.clock = self.clock.max(now);
        let id = self.next_id;
        self.next_id += 1;
        let refuse = |s: &mut Self, why: &str| {
            s.out.push(Out::StreamEnd { sub: id, reason: format!("refused: {why}") });
            id
        };
        if frame.len() > self.cfg.max_frame {
            return refuse(self, "the request is larger than the connection allows");
        }
        let text = match std::str::from_utf8(frame) {
            Ok(t) => t.trim().to_owned(),
            Err(_) => return refuse(self, "the request frame is not UTF-8 text"),
        };
        match serde_json::from_str::<Value>(&text) {
            Ok(Value::Object(m)) if m.get("program").and_then(Value::as_str).is_some_and(|p| !p.trim().is_empty()) => {
                if ["t", "id", "key"].iter().any(|k| m.contains_key(*k)) {
                    return refuse(self, "the request frame carries a member the player adds (t, id or key)");
                }
            }
            _ => return refuse(self, "the request frame is not a JSON object naming a program"),
        }
        self.subs.insert(id, Sub { frame: text, sent: false, ever_sent: false, restart: false });
        match self.state {
            LinkState::Ready => self.transmit_sub(id),
            LinkState::Closed if self.wanted => self.start_attempt(1, now),
            _ => {}
        }
        id
    }

    /// End a subscription. The backend stops the program; [`Out::StreamEnd`] follows with the reason `unsubscribed`.
    pub fn unsubscribe(&mut self, sub: u32) {
        let Some(s) = self.subs.remove(&sub) else { return };
        if s.sent && self.state == LinkState::Ready {
            self.send_msg(&json!({"t": "unsub", "id": sub}).to_string());
        } else {
            self.out.push(Out::StreamEnd { sub, reason: "unsubscribed".into() });
        }
    }

    /// The subscriptions still open (for the host, to end an instance's when it closes).
    pub fn subscriptions(&self) -> Vec<u32> {
        self.subs.keys().copied().collect()
    }

    fn transmit_sub(&mut self, id: u32) {
        let Some(s) = self.subs.get_mut(&id) else { return };
        let inner = s.frame.strip_prefix('{').and_then(|b| b.strip_suffix('}')).unwrap_or("").trim().to_owned();
        if s.ever_sent {
            s.restart = true;
        }
        s.sent = true;
        s.ever_sent = true;
        let msg = format!("{{\"t\":\"sub\",\"id\":{id}{}{inner}}}", if inner.is_empty() { "" } else { "," });
        self.send_msg(&msg);
    }

    /// The user asked to cancel call `id` (WASM-014 R13): the backend is told, and the call stays pending until the backend answers it
    /// (`application 8000`), so the program sees the cancellation as the answer. Answers whether a cancel frame was sent.
    pub fn request_cancel(&mut self, id: u32) -> bool {
        if !self.inflight.get(&id).is_some_and(|f| f.sent) || self.state != LinkState::Ready {
            return false;
        }
        self.send_msg(&json!({"t": "cancel", "id": id}).to_string());
        true
    }

    // ── files in ─────────────────────────────────────────────────────────────

    /// Begin an upload into `slot`. `sha256` is the hex hash of the whole file, taken by the browser over its pieces. A name or slot
    /// that is a path is refused before any frame leaves ([`Out::TransferFailed`]). Answers the transfer's id; [`Out::PutReady`]
    /// says where to read the next piece from, and again after every reconnection (the backend knows how much it holds).
    pub fn put_begin(&mut self, slot: &str, name: &str, mime: &str, size: u64, sha256: &str, now: u64) -> u32 {
        self.clock = self.clock.max(now);
        let id = self.next_id;
        self.next_id += 1;
        for (what, v) in [("slot", slot), ("file name", name)] {
            if let Err(why) = plain_name(v) {
                self.out.push(Out::TransferFailed { id, class: class::VALIDATION.into(), code: code::BAD_MESSAGE, message: format!("the {what} {v:?} is refused: {why}") });
                return id;
            }
        }
        if sha256.len() != 64 || !sha256.bytes().all(|b| b.is_ascii_hexdigit()) {
            self.out.push(Out::TransferFailed { id, class: class::VALIDATION.into(), code: code::BAD_MESSAGE, message: "the file's SHA-256 is not 64 hex digits".into() });
            return id;
        }
        self.puts.insert(id, Put { slot: slot.into(), name: name.into(), mime: mime.into(), size, sha256: sha256.to_ascii_lowercase(), upload: None, sent: false });
        match self.state {
            LinkState::Ready => self.transmit_put(id),
            LinkState::Closed if self.wanted => self.start_attempt(1, now),
            _ => {}
        }
        id
    }

    fn transmit_put(&mut self, id: u32) {
        let Some(p) = self.puts.get_mut(&id) else { return };
        let mut m = json!({"t": "put-begin", "id": id, "slot": p.slot, "name": p.name, "size": p.size, "mime": p.mime, "sha256": p.sha256});
        if let Some(u) = &p.upload {
            m["resume"] = Value::String(u.clone());
        }
        p.sent = true;
        self.send_msg(&m.to_string());
    }

    /// Send the piece of upload `id` that starts at `offset`. Refused (false) when the upload is not ready for pieces, or the piece
    /// is larger than [`CHUNK_BYTES`].
    pub fn put_chunk(&mut self, id: u32, offset: u64, bytes: &[u8]) -> bool {
        let ready = self.puts.get(&id).is_some_and(|p| p.sent && p.upload.is_some());
        if !ready || bytes.len() > CHUNK_BYTES || self.state != LinkState::Ready {
            return false;
        }
        let data = cobolt_wire::seal::encode_b64(bytes);
        self.send_msg(&json!({"t": "put-chunk", "id": id, "offset": offset, "data": data}).to_string());
        true
    }

    /// All the pieces are sent: ask the backend to verify the hash and store the file.
    pub fn put_end(&mut self, id: u32) -> bool {
        if !self.puts.get(&id).is_some_and(|p| p.sent) || self.state != LinkState::Ready {
            return false;
        }
        self.send_msg(&json!({"t": "put-end", "id": id}).to_string());
        true
    }

    // ── files out ────────────────────────────────────────────────────────────

    /// Ask for a download address for `file`, the backend's own name for it. [`Out::FileUrl`] answers; the browser navigates to
    /// the address (a short-lived, per-request path under the page's own origin), and nothing is kept in browser storage.
    pub fn get_file(&mut self, file: &str, now: u64) -> u32 {
        self.clock = self.clock.max(now);
        let id = self.next_id;
        self.next_id += 1;
        if let Err(why) = plain_name(file) {
            self.out.push(Out::TransferFailed { id, class: class::VALIDATION.into(), code: code::BAD_MESSAGE, message: format!("the file name {file:?} is refused: {why}") });
            return id;
        }
        self.gets.insert(id, file.to_owned());
        if self.state == LinkState::Ready {
            self.transmit_get(id);
        } else if self.state == LinkState::Closed && self.wanted {
            self.start_attempt(1, now);
        }
        id
    }

    fn transmit_get(&mut self, id: u32) {
        if let Some(f) = self.gets.get(&id).cloned() {
            self.send_msg(&json!({"t": "get-file", "id": id, "file": f}).to_string());
        }
    }

    // ── from the connection ──────────────────────────────────────────────────

    /// The connection is ready: send what waited, and make the subscriptions and uploads again (an upload asks the backend how much
    /// it holds).
    pub(super) fn transfers_ready(&mut self) {
        for id in self.subs.iter().filter(|(_, s)| !s.sent).map(|(i, _)| *i).collect::<Vec<_>>() {
            self.transmit_sub(id);
        }
        for id in self.puts.iter().filter(|(_, p)| !p.sent).map(|(i, _)| *i).collect::<Vec<_>>() {
            self.transmit_put(id);
        }
        for id in self.gets.keys().copied().collect::<Vec<_>>() {
            self.transmit_get(id);
        }
    }

    /// The connection broke: what was sent on it is to be sent again on the next.
    pub(super) fn transfers_lost(&mut self) {
        for s in self.subs.values_mut() {
            s.sent = false;
        }
        for p in self.puts.values_mut() {
            p.sent = false;
        }
    }

    /// Handle a frame about progress, streams or files; false when it is none of those.
    pub(super) fn on_transfer_frame(&mut self, kind: &str, text: &str, v: &Value) -> bool {
        let id = v.get("id").and_then(Value::as_u64).filter(|n| *n <= u32::MAX as u64).map(|n| n as u32);
        match kind {
            "progress" => {
                if let Some(id) = id.filter(|i| self.inflight.contains_key(i)) {
                    let percent = v.get("percent").and_then(Value::as_u64).unwrap_or(0).min(100) as u8;
                    let message = v.get("message").and_then(Value::as_str).unwrap_or("").to_owned();
                    self.out.push(Out::Progress { id, percent, message });
                }
            }
            "subscribed" => {}
            "event" => {
                if let Ok(ServerFrame::Event(e)) = parse_server(text) {
                    let sub = e.sub as u32;
                    if let Some(s) = self.subs.get_mut(&sub) {
                        let restart = std::mem::take(&mut s.restart);
                        self.out.push(Out::StreamItem { sub, json: e.data.get().to_owned(), restart });
                    }
                }
            }
            "sub-end" => {
                let sub = v.get("sub").and_then(Value::as_u64).unwrap_or(0) as u32;
                if self.subs.remove(&sub).is_some() {
                    self.out.push(Out::StreamEnd { sub, reason: "ended".into() });
                }
            }
            "unsubscribed" => {
                if let Some(id) = id {
                    self.out.push(Out::StreamEnd { sub: id, reason: "unsubscribed".into() });
                }
            }
            "put-ready" => {
                if let Some((id, p)) = id.and_then(|i| self.puts.get_mut(&i).map(|p| (i, p))) {
                    p.upload = v.get("upload").and_then(Value::as_str).map(str::to_owned);
                    let have = v.get("have").and_then(Value::as_u64).unwrap_or(0);
                    self.out.push(Out::PutReady { id, have });
                }
            }
            "put-done" => {
                if let Some(id) = id.filter(|i| self.puts.remove(i).is_some()) {
                    let file = v.get("file").and_then(Value::as_str).unwrap_or("").to_owned();
                    self.out.push(Out::PutDone { id, file, size: v.get("size").and_then(Value::as_u64).unwrap_or(0) });
                }
            }
            "file-url" => {
                if let Some(id) = id.filter(|i| self.gets.remove(i).is_some()) {
                    self.out.push(Out::FileUrl {
                        id,
                        path: v.get("path").and_then(Value::as_str).unwrap_or("").to_owned(),
                        expires_in: v.get("expires_in").and_then(Value::as_u64).unwrap_or(0),
                        size: v.get("size").and_then(Value::as_u64).unwrap_or(0),
                        sha256: v.get("sha256").and_then(Value::as_str).unwrap_or("").to_owned(),
                    });
                }
            }
            _ => return false,
        }
        true
    }

    /// An `error` frame for a stream, an upload or a download: ends it and says why. False when `id` is none of those.
    pub(super) fn on_transfer_error(&mut self, id: u32, v: &Value) -> bool {
        let class_ = v.get("class").and_then(Value::as_str).unwrap_or("").to_owned();
        let code_ = v.get("code").and_then(Value::as_u64).unwrap_or(0) as u32;
        let message = v.get("message").and_then(Value::as_str).unwrap_or("").to_owned();
        if self.subs.remove(&id).is_some() {
            self.out.push(Out::StreamEnd { sub: id, reason: format!("{class_} {code_}: {message}") });
            true
        } else if self.puts.remove(&id).is_some() || self.gets.remove(&id).is_some() {
            self.out.push(Out::TransferFailed { id, class: class_, code: code_, message });
            true
        } else {
            false
        }
    }
}
