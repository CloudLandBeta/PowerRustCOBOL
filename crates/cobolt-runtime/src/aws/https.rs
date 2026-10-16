// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! A hosted AWS MCP server over HTTPS, signed with SigV4 (spec 103 R1–R5).
//!
//! The route table can name a server by an address instead of a program
//! (`transport = "https"`). This is how it is spoken to: MCP **streamable
//! HTTP**, where each JSON-RPC message is one `POST` and the answer is read from
//! that `POST`'s body — a JSON document, or `text/event-stream` events — so no
//! reader thread and no pending-call table are needed, unlike a child process.
//!
//! - **Every request is signed** with the SigV4 code of the official AWS crates,
//!   for the service and region the route names, with the credentials in force at
//!   that moment (R3). The credentials are the keys the program set for the
//!   connection (spec 092 A6), lent for the one call by
//!   [`super::process::Credentials::with_keys`] and kept nowhere.
//! - **`_meta` is added before signing** (R4): the values the route declares
//!   (today `AWS_REGION`) go into every message's `params._meta`, so the body the
//!   signature covers is the body that is sent and `cobolt-mcp` stays unaware.
//! - **The session** the server opens with `initialize` is named in every later
//!   request, and ended with a `DELETE` when the pool stops the link.
//! - **Nothing here names a service or an address** (R1): they come from the
//!   route table through [`HttpsEndpoint`].
//!
//! A failure is reported as the existing classes — a timeout, or a plain
//! sentence — and never carries a key.

use std::io::{BufRead, BufReader, Read};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime};

use aws_sigv4::http_request::{sign, SignableBody, SignableRequest, SigningParams, SigningSettings};
use aws_smithy_runtime_api::client::identity::Identity;
use cobolt_mcp::client::{Inbound, Session};
use cobolt_mcp::Response;
use serde_json::{json, Value};

use super::chain::{Chain, ChainError};
use super::process::{HttpsEndpoint, Launch};
use crate::http_runtime::{self, RawReply, SendError};

/// The largest answer read: far above any tool result, so a runaway body
/// cannot fill memory.
const MAX_BODY: u64 = 64 * 1024 * 1024;

/// What of an error body is shown.
const SNIPPET: usize = 400;

/// Why a request through a link produced no answer to read.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum LinkError {
    /// The time allowed ran out.
    Timeout,
    /// The server does not know the session any more (HTTP 404 on a request
    /// that named one): the caller opens a new one and repeats the call.
    SessionUnknown,
    /// Anything else, in words for the pool's `CallError::Failed`.
    Failed(String),
}

/// What the link has learned from the server.
#[derive(Default)]
struct Wire {
    session_id: Option<String>,
    protocol_version: Option<String>,
}

struct Inner {
    endpoint: HttpsEndpoint,
    /// `host[:port]` of the endpoint, as the `Host` header carries it.
    authority: String,
    agent: ureq::Agent,
    /// Only reads frames (`route` needs no counters), so it is a copy.
    router: Session,
    keys: Option<Arc<super::process::Credentials>>,
    /// The sign-in the connection's profile names; used when the program has set no keys.
    chain: Option<Chain>,
    wire: Mutex<Wire>,
    /// Counts the sessions this link has forgotten, so that several callers
    /// whose request met the same expired session open one new session between
    /// them, not one each.
    generation: AtomicU64,
    closed: AtomicBool,
}

/// One HTTPS connection to a hosted server. Cheap to clone; clones share the
/// session.
#[derive(Clone)]
pub(crate) struct Link(Arc<Inner>);

impl Link {
    /// A link to the endpoint `launch` carries. Nothing is sent yet.
    pub(crate) fn open(launch: &Launch, router: Session) -> Result<Link, String> {
        let endpoint = launch.https.clone().ok_or("the AWS service has no address")?;
        let authority = authority_of(&endpoint.url)?;
        Ok(Link(Arc::new(Inner {
            authority,
            agent: http_runtime::signed_agent(),
            router,
            keys: launch.credentials.clone(),
            chain: launch.credentials.is_none().then(|| {
                let region = launch.region.as_deref().unwrap_or(endpoint.signing_region.as_str());
                Chain::new(launch.profile.as_deref(), Some(region))
            }),
            endpoint,
            wire: Mutex::new(Wire::default()),
            generation: AtomicU64::new(0),
            closed: AtomicBool::new(false),
        })))
    }

    pub(crate) fn is_alive(&self) -> bool {
        !self.0.closed.load(Ordering::SeqCst)
    }

    /// How many sessions this link has dropped; see [`Inner::generation`].
    pub(crate) fn generation(&self) -> u64 {
        self.0.generation.load(Ordering::SeqCst)
    }

    /// Name the protocol revision the handshake agreed on, for the header that
    /// carries it on later requests.
    pub(crate) fn set_protocol_version(&self, version: &str) {
        lock(&self.0.wire).protocol_version = Some(version.to_owned());
    }

    /// Drop the session id: the next request opens a new session.
    pub(crate) fn forget_session(&self) {
        lock(&self.0.wire).session_id = None;
        self.0.generation.fetch_add(1, Ordering::SeqCst);
    }

    /// Send one request frame and wait for the answer with the same `id`.
    pub(crate) fn round_trip(&self, id: u64, bytes: &[u8], timeout: Duration) -> Result<Response, LinkError> {
        let reply = self.post(bytes, timeout)?;
        let class = classify(&reply);
        match class {
            Class::Answer => {}
            Class::SessionUnknown => {
                let had_session = lock(&self.0.wire).session_id.is_some();
                return Err(if had_session { LinkError::SessionUnknown } else { LinkError::Failed(error_text(reply)) });
            }
            Class::Refused => return Err(LinkError::Failed(error_text(reply))),
        }
        self.remember_session(&reply);
        let is_sse = reply
            .header("content-type")
            .is_some_and(|t| t.to_ascii_lowercase().starts_with("text/event-stream"));
        if is_sse {
            let mut found = None;
            for_each_event(reply.body, |data| {
                if let Some(resp) = self.take_response(data.as_bytes(), id, timeout) {
                    found = Some(resp);
                    return false;
                }
                true
            })?;
            found.ok_or_else(|| LinkError::Failed("the AWS service ended its answer without replying".into()))
        } else {
            let mut raw = Vec::new();
            reply
                .body
                .take(MAX_BODY)
                .read_to_end(&mut raw)
                .map_err(|e| read_error(&e))?;
            self.take_response(&raw, id, timeout)
                .ok_or_else(|| LinkError::Failed("the AWS service's answer was not a reply to the request".into()))
        }
    }

    /// Send a notification frame (no answer is owed).
    pub(crate) fn notify(&self, bytes: &[u8], timeout: Duration) -> Result<(), LinkError> {
        let reply = self.post(bytes, timeout)?;
        match classify(&reply) {
            Class::Answer => Ok(()),
            Class::SessionUnknown => Err(LinkError::SessionUnknown),
            Class::Refused => Err(LinkError::Failed(error_text(reply))),
        }
    }

    /// End the session at the server and stop using the link. Best effort: a
    /// session that cannot be ended expires there by itself.
    pub(crate) fn close(&self, timeout: Duration) {
        if self.0.closed.swap(true, Ordering::SeqCst) {
            return;
        }
        let session = lock(&self.0.wire).session_id.clone();
        if session.is_none() {
            return;
        }
        let _ = self.send("DELETE", &[], timeout);
    }

    /// [`Link::close`] on a thread of its own, for callers that hold a lock or
    /// stand on the interpreter's thread and must not wait on the network.
    pub(crate) fn close_in_background(&self) {
        let link = self.clone();
        let _ = std::thread::Builder::new()
            .name("aws-https-close".into())
            .spawn(move || link.close(Duration::from_secs(2)));
    }

    // ── one request ─────────────────────────────────────────────────────────

    /// POST a message, with `_meta` added and the request signed.
    fn post(&self, bytes: &[u8], timeout: Duration) -> Result<RawReply, LinkError> {
        let body = self.with_meta(bytes)?;
        self.send("POST", &body, timeout)
    }

    fn send(&self, method: &str, body: &[u8], timeout: Duration) -> Result<RawReply, LinkError> {
        if self.0.closed.load(Ordering::SeqCst) && method != "DELETE" {
            return Err(LinkError::Failed("the AWS service connection was closed".into()));
        }
        let mut headers = self.base_headers(method, !body.is_empty());
        self.sign(method, &mut headers, body, timeout)?;
        let reply = http_runtime::send_bytes(&self.0.agent, method, &self.0.endpoint.url, &headers, body, timeout).map_err(|e| match e {
            SendError::Timeout => LinkError::Timeout,
            SendError::Unreachable(why) => LinkError::Failed(format!("the AWS service could not be reached: {why}")),
        })?;
        // A refused sign-in is not kept: the next request asks the providers again,
        // so a sign-in the user has since renewed in a terminal is found.
        if matches!(reply.status, 401 | 403) {
            if let Some(chain) = &self.0.chain {
                chain.forget();
            }
        }
        Ok(reply)
    }

    fn base_headers(&self, method: &str, has_body: bool) -> Vec<(String, String)> {
        let mut h = vec![
            ("host".to_owned(), self.0.authority.clone()),
            ("accept".to_owned(), "application/json, text/event-stream".to_owned()),
            // ureq adds gzip on its own unless told otherwise, and the signature must cover what is sent.
            ("accept-encoding".to_owned(), "identity".to_owned()),
        ];
        if has_body || method == "POST" {
            h.push(("content-type".to_owned(), "application/json".to_owned()));
        }
        let wire = lock(&self.0.wire);
        if let Some(s) = &wire.session_id {
            h.push(("mcp-session-id".to_owned(), s.clone()));
        }
        if let Some(v) = &wire.protocol_version {
            h.push(("mcp-protocol-version".to_owned(), v.clone()));
        }
        h
    }

    /// Add the SigV4 headers for `method`, the endpoint and `body`: with the keys the
    /// program set, or else with what the connection's sign-in provides now.
    fn sign(&self, method: &str, headers: &mut Vec<(String, String)>, body: &[u8], timeout: Duration) -> Result<(), LinkError> {
        if let Some(keys) = self.0.keys.as_deref() {
            return keys.with_keys(|k| {
                let credentials = aws_credential_types::Credentials::new(
                    k.key_id,
                    k.secret,
                    k.token.map(str::to_owned),
                    None,
                    "AWS-SET-CREDENTIALS",
                );
                let identity: Identity = credentials.into();
                self.sign_with(&identity, method, headers, body)
            });
        }
        let chain = self.0.chain.as_ref().ok_or_else(|| LinkError::Failed("Unable to locate credentials".into()))?;
        let credentials = chain.credentials(timeout).map_err(|e| match e {
            ChainError::Timeout => LinkError::Timeout,
            ChainError::Failed(why) => LinkError::Failed(why),
        })?;
        let identity: Identity = credentials.into();
        self.sign_with(&identity, method, headers, body)
    }

    fn sign_with(&self, identity: &Identity, method: &str, headers: &mut Vec<(String, String)>, body: &[u8]) -> Result<(), LinkError> {
        let e = &self.0.endpoint;
        let params: SigningParams<'_> = aws_sigv4::sign::v4::SigningParams::builder()
            .identity(identity)
            .region(&e.signing_region)
            .name(&e.service)
            .time(SystemTime::now())
            .settings(SigningSettings::default())
            .build()
            .map_err(|_| LinkError::Failed("the AWS request could not be signed".into()))?
            .into();
        let signable = SignableRequest::new(
            method,
            e.url.as_str(),
            headers.iter().map(|(k, v)| (k.as_str(), v.as_str())),
            SignableBody::Bytes(body),
        )
        .map_err(|_| LinkError::Failed("the AWS request could not be signed".into()))?;
        let (instructions, _signature) = sign(signable, &params)
            .map_err(|_| LinkError::Failed("the AWS request could not be signed".into()))?
            .into_parts();
        for (name, value) in instructions.headers() {
            headers.push((name.to_owned(), value.to_owned()));
        }
        Ok(())
    }

    /// Put the route's metadata into `params._meta` of a message that has a method.
    fn with_meta(&self, bytes: &[u8]) -> Result<Vec<u8>, LinkError> {
        if self.0.endpoint.metadata.is_empty() {
            return Ok(bytes.to_vec());
        }
        let mut message: Value =
            serde_json::from_slice(bytes).map_err(|e| LinkError::Failed(format!("the AWS request was not JSON: {e}")))?;
        if let Some(obj) = message.as_object_mut().filter(|o| o.contains_key("method")) {
            let params = obj.entry("params").or_insert_with(|| json!({}));
            if let Some(p) = params.as_object_mut() {
                let meta = p.entry("_meta").or_insert_with(|| json!({}));
                if let Some(m) = meta.as_object_mut() {
                    for (k, v) in &self.0.endpoint.metadata {
                        m.insert(k.clone(), json!(v));
                    }
                }
            }
        }
        serde_json::to_vec(&message).map_err(|e| LinkError::Failed(format!("the AWS request could not be written: {e}")))
    }

    fn remember_session(&self, reply: &RawReply) {
        if let Some(id) = reply.header("mcp-session-id").map(str::trim).filter(|s| !s.is_empty()) {
            lock(&self.0.wire).session_id = Some(id.to_owned());
        }
    }

    /// One frame of an answer: the reply to `id`, or `None` (a notification, a
    /// server request answered here, or a reply to something else).
    fn take_response(&self, frame: &[u8], id: u64, timeout: Duration) -> Option<Response> {
        match self.0.router.route(frame) {
            Inbound::Response(r) if r.id.as_u64() == Some(id) => Some(r),
            Inbound::Reply(bytes) => {
                // A request from the server (`ping`): answer it, and keep waiting.
                let _ = self.post(&bytes, timeout);
                None
            }
            _ => None,
        }
    }
}

// ── reading an answer ──────────────────────────────────────────────────────────

enum Class {
    Answer,
    SessionUnknown,
    Refused,
}

fn classify(reply: &RawReply) -> Class {
    match reply.status {
        200..=299 => Class::Answer,
        404 => Class::SessionUnknown,
        _ => Class::Refused,
    }
}

/// The start of an error answer's body, as one line.
fn error_text(reply: RawReply) -> String {
    let status = reply.status;
    let mut raw = Vec::new();
    let _ = reply.body.take(SNIPPET as u64 * 4).read_to_end(&mut raw);
    let text = String::from_utf8_lossy(&raw);
    let one_line: String = text.split_whitespace().collect::<Vec<_>>().join(" ");
    let shown: String = one_line.chars().take(SNIPPET).collect();
    if shown.is_empty() {
        format!("HTTP {status} from the AWS service")
    } else {
        format!("HTTP {status} from the AWS service: {shown}")
    }
}

fn read_error(e: &std::io::Error) -> LinkError {
    if matches!(e.kind(), std::io::ErrorKind::TimedOut | std::io::ErrorKind::WouldBlock) {
        LinkError::Timeout
    } else {
        LinkError::Failed(format!("the AWS service's answer was cut short: {e}"))
    }
}

/// Call `each` with the `data` of every server-sent event, until it says stop or
/// the stream ends. An event is the `data:` lines before a blank line.
fn for_each_event(body: Box<dyn Read + Send + Sync + 'static>, mut each: impl FnMut(&str) -> bool) -> Result<(), LinkError> {
    let mut lines = BufReader::new(body.take(MAX_BODY));
    let mut data = String::new();
    let mut line = String::new();
    loop {
        line.clear();
        let n = lines.read_line(&mut line).map_err(|e| read_error(&e))?;
        let text = line.trim_end_matches(['\r', '\n']);
        if n == 0 || text.is_empty() {
            if !data.is_empty() {
                let event = std::mem::take(&mut data);
                if !each(&event) {
                    return Ok(());
                }
            }
            if n == 0 {
                return Ok(());
            }
        } else if let Some(rest) = text.strip_prefix("data:") {
            if !data.is_empty() {
                data.push('\n');
            }
            data.push_str(rest.strip_prefix(' ').unwrap_or(rest));
        }
        // `event:`, `id:`, `retry:` and `:` comment lines carry nothing we use.
    }
}

/// `host[:port]` of an address, as a `Host` header carries it.
fn authority_of(url: &str) -> Result<String, String> {
    let rest = url.split_once("://").map(|(_, r)| r).ok_or("the AWS service address is not a URL")?;
    let authority = rest.split(['/', '?', '#']).next().unwrap_or_default();
    if authority.is_empty() || authority.contains('@') {
        return Err("the AWS service address has no host".into());
    }
    Ok(authority.to_owned())
}

fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn events(text: &str) -> Vec<String> {
        let mut out = Vec::new();
        for_each_event(Box::new(std::io::Cursor::new(text.as_bytes().to_vec())), |d| {
            out.push(d.to_owned());
            true
        })
        .unwrap();
        out
    }

    #[test]
    fn server_sent_events_are_read_by_their_data_lines() {
        let text = ": comment\nevent: message\nid: 1\ndata: {\"a\":1}\n\ndata: {\"b\":\ndata: 2}\r\n\r\ndata:{\"c\":3}\n\n";
        assert_eq!(events(text), ["{\"a\":1}", "{\"b\":\n2}", "{\"c\":3}"]);
        assert_eq!(events("data: last-without-blank-line"), ["last-without-blank-line"], "a stream that ends mid-event still yields it");
        assert!(events("event: ping\n\n").is_empty());
    }

    #[test]
    fn the_stop_answer_ends_the_read() {
        let mut seen = 0;
        for_each_event(Box::new(std::io::Cursor::new(b"data: 1\n\ndata: 2\n\ndata: 3\n\n".to_vec())), |_| {
            seen += 1;
            seen < 2
        })
        .unwrap();
        assert_eq!(seen, 2);
    }

    #[test]
    fn the_host_header_is_the_authority() {
        assert_eq!(authority_of("https://hosted.example/mcp").unwrap(), "hosted.example");
        assert_eq!(authority_of("http://127.0.0.1:5123/mcp").unwrap(), "127.0.0.1:5123");
        assert!(authority_of("https://user@host/mcp").is_err());
        assert!(authority_of("not a url").is_err());
    }
}
