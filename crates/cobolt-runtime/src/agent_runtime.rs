// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! The `AgentObject` control's LLM call — request shaping and reply parsing.
//!
//! Until 1.65.57 the control made **no network call at all**. `Ask` stored the
//! prompt, read a `LastReply` that nothing ever wrote, and returned the empty
//! string; `onResponse` is guarded on a non-empty reply, so it never fired
//! either. Every request-shaping property the designer offered —
//! `AgentAPIKey`, `AgentAPI`, `AgentEndpoint`, `Temperature`, `MaximumTokens` —
//! was recorded as *Unread* in `test_nonvisual_property_readers`, because there
//! was no request for them to reach.
//!
//! This module is the pure half: what to send, where, and how to read what
//! comes back. The transport is the interpreter's existing HTTP bridge, so a
//! build without the `http` feature degrades with that bridge's own message
//! rather than a second one.

/// The wire protocol a provider speaks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Protocol {
    /// `POST /v1/chat/completions` — OpenAI, LM Studio, and Ollama's
    /// OpenAI-compatible endpoint. Reply at `choices[0].message.content`.
    OpenAiChat,
    /// Ollama's native `POST /api/chat`. Reply at `message.content`.
    OllamaChat,
    /// Anthropic `POST /v1/messages`. Reply at `content[0].text`.
    Anthropic,
}

/// Everything one `Ask` needs, read off the control.
#[derive(Debug, Clone, Default)]
pub struct AskRequest {
    pub api: String,
    pub url: String,
    pub endpoint: String,
    pub model: String,
    pub api_key: String,
    pub system_prompt: String,
    pub prompt: String,
    /// As the designer stores it: 0–100, meaning 0.0–1.0.
    pub temperature: i64,
    pub max_tokens: i64,
}

/// Which protocol to speak.
///
/// `AgentAPI` decides it when it names a provider. The URL breaks the tie for
/// `Custom` and for an unset `AgentAPI`, because the path is the one honest
/// clue: a developer who typed `/api/chat` means Ollama's native shape, and one
/// who typed `/v1/chat/completions` means the OpenAI-compatible one. The
/// OpenAI-compatible shape is the default because three of the four providers
/// speak it.
pub fn protocol_for(api: &str, url: &str) -> Protocol {
    let path = url.to_ascii_lowercase();
    match api.trim().to_ascii_lowercase().as_str() {
        "anthropic" => Protocol::Anthropic,
        "openai" | "lmstudio" => Protocol::OpenAiChat,
        "ollama" => {
            // Ollama serves BOTH: `/api/chat` is its own, `/v1/...` is the
            // OpenAI-compatible one. Believe the path the developer wrote.
            if path.contains("/v1/") {
                Protocol::OpenAiChat
            } else {
                Protocol::OllamaChat
            }
        }
        _ => {
            if path.contains("/v1/messages") {
                Protocol::Anthropic
            } else if path.contains("/api/chat") || path.contains("/api/generate") {
                Protocol::OllamaChat
            } else {
                Protocol::OpenAiChat
            }
        }
    }
}

/// The address to POST to.
///
/// `AgentEndpoint` wins outright when set — that is what an override is for.
/// Otherwise `AgentURL` is used as written when it already names a path, and
/// only given the protocol's default path when it is a bare origin. A URL the
/// developer spelled out is never rewritten.
pub fn endpoint_for(req: &AskRequest, protocol: Protocol) -> String {
    let override_url = req.endpoint.trim();
    // A bare PATH (`/v1/chat`) is joined onto `AgentURL`'s origin: it was sent
    // verbatim as the whole URL, which no server can answer (property audit,
    // 2026-09-26). A full URL is still used as it stands.
    if let Some(path) = override_url.strip_prefix('/') {
        let base = req.url.trim();
        let (scheme, rest) = base.split_once("://").unwrap_or(("", base));
        let origin = rest.split('/').next().unwrap_or("");
        if !origin.is_empty() {
            let scheme = if scheme.is_empty() { "https" } else { scheme };
            return format!("{scheme}://{origin}/{path}");
        }
    }
    if !override_url.is_empty() {
        return override_url.to_owned();
    }
    let base = req.url.trim().trim_end_matches('/');
    if base.is_empty() {
        return String::new();
    }
    // A path beyond the origin means the developer already chose the endpoint.
    let after_scheme = base.split("://").nth(1).unwrap_or(base);
    if after_scheme.contains('/') {
        return base.to_owned();
    }
    let path = match protocol {
        Protocol::OpenAiChat => "/v1/chat/completions",
        Protocol::OllamaChat => "/api/chat",
        Protocol::Anthropic => "/v1/messages",
    };
    format!("{base}{path}")
}

/// The headers this request needs.
///
/// The key is sent only when there is one: a local Ollama or LM Studio wants no
/// Authorization header at all, and sending an empty bearer is worse than
/// sending none.
pub fn headers_for(req: &AskRequest, protocol: Protocol) -> Vec<(String, String)> {
    let mut out = vec![("Content-Type".to_owned(), "application/json".to_owned())];
    let key = req.api_key.trim();
    if key.is_empty() {
        return out;
    }
    match protocol {
        Protocol::Anthropic => {
            out.push(("x-api-key".to_owned(), key.to_owned()));
            out.push(("anthropic-version".to_owned(), "2023-06-01".to_owned()));
        }
        _ => out.push(("Authorization".to_owned(), format!("Bearer {key}"))),
    }
    out
}

/// The JSON body, in the protocol's own shape.
///
/// `stream` is always false: the reply is read once, whole. `Stream` on the
/// control stays unread until streaming is actually implemented — claiming it
/// here by asking the server to stream, and then parsing the first chunk as if
/// it were the whole answer, would truncate every reply.
pub fn body_for(req: &AskRequest, protocol: Protocol) -> String {
    let temperature = (req.temperature.clamp(0, 100) as f64) / 100.0;
    let max_tokens = req.max_tokens.max(1);
    let system = req.system_prompt.trim();
    let value = match protocol {
        Protocol::Anthropic => {
            let mut v = serde_json::json!({
                "model": req.model,
                "max_tokens": max_tokens,
                "temperature": temperature,
                "messages": [{"role": "user", "content": req.prompt}],
            });
            if !system.is_empty() {
                v["system"] = serde_json::Value::String(system.to_owned());
            }
            v
        }
        Protocol::OllamaChat => {
            let mut messages = Vec::new();
            if !system.is_empty() {
                messages.push(serde_json::json!({"role": "system", "content": system}));
            }
            messages.push(serde_json::json!({"role": "user", "content": req.prompt}));
            serde_json::json!({
                "model": req.model,
                "messages": messages,
                "stream": false,
                "options": {"temperature": temperature, "num_predict": max_tokens},
            })
        }
        Protocol::OpenAiChat => {
            let mut messages = Vec::new();
            if !system.is_empty() {
                messages.push(serde_json::json!({"role": "system", "content": system}));
            }
            messages.push(serde_json::json!({"role": "user", "content": req.prompt}));
            serde_json::json!({
                "model": req.model,
                "messages": messages,
                "stream": false,
                "temperature": temperature,
                "max_tokens": max_tokens,
            })
        }
    };
    value.to_string()
}

/// The assistant's text, or the reason there is none.
///
/// Every known shape is tried, whatever protocol was chosen: a `Custom`
/// endpoint may answer in any of them, and a provider that changes its path
/// should not turn a good reply into an error. A provider's own `error` field
/// is reported as the error even on a 200, which some of them do.
pub fn parse_reply(status: u16, body: &str) -> Result<String, String> {
    // Nothing came back at all. Saying "not JSON" about an empty string is
    // technically true and sends the reader to their provider's response
    // format, which is the one place the fault is not (operator, 2026-09-07).
    if body.trim().is_empty() {
        return Err(format!("HTTP {status}: the reply was empty"));
    }
    let json: serde_json::Value = match serde_json::from_str(body) {
        Ok(v) => v,
        Err(_) => {
            // Not JSON at all. On a good status that is still a failure, but the
            // body is the only evidence there is, so it goes in the message.
            return Err(format!(
                "HTTP {status}: the reply was not JSON — {}",
                clip(body)
            ));
        }
    };
    if let Some(text) = provider_error(&json) {
        return Err(format!("HTTP {status}: {text}"));
    }
    if !(200..300).contains(&status) {
        return Err(format!("HTTP {status}: {}", clip(body)));
    }
    // OpenAI-compatible, Ollama native, Anthropic — in that order.
    let candidates = [
        json.pointer("/choices/0/message/content"),
        json.pointer("/message/content"),
        json.pointer("/content/0/text"),
        // Ollama's /api/generate, which a developer may have pointed at.
        json.pointer("/response"),
    ];
    for found in candidates.into_iter().flatten() {
        if let Some(text) = found.as_str() {
            return Ok(text.to_owned());
        }
    }
    Err(format!(
        "HTTP {status}: no assistant message in the reply — {}",
        clip(body)
    ))
}

/// `body` (from [`body_for`]) asking for a STREAMED reply.
pub fn streaming_body(body: &str) -> String {
    match serde_json::from_str::<serde_json::Value>(body) {
        Ok(mut v) => {
            v["stream"] = serde_json::Value::Bool(true);
            v.to_string()
        }
        Err(_) => body.to_owned(),
    }
}

/// Reads a streamed reply line by line and assembles it.
///
/// The three shapes: OpenAI-compatible SSE (`data: {"choices":[{"delta":…}]}`
/// ending in `data: [DONE]`), Anthropic SSE (`content_block_delta` events,
/// usage in `message_start` / `message_delta`), and Ollama's native NDJSON
/// (`{"message":{"content":…},"done":false}`). Every line is tried against
/// every shape, as [`parse_reply`] does, so a `Custom` endpoint works too.
#[derive(Debug, Default)]
pub struct StreamAssembler {
    /// The reply so far.
    pub text: String,
    pub input_tokens: u64,
    pub output_tokens: u64,
    /// A provider error met inside the stream.
    pub error: Option<String>,
}

impl StreamAssembler {
    /// Take one line. True when it added text to the reply.
    pub fn feed(&mut self, line: &str) -> bool {
        let line = line.trim();
        let payload = match line.strip_prefix("data:") {
            Some(rest) => rest.trim(),
            // SSE comments, `event:` names and blank separators carry nothing
            // the `data:` line after them does not repeat.
            None if line.starts_with('{') => line,
            None => return false,
        };
        if payload.is_empty() || payload == "[DONE]" {
            return false;
        }
        let Ok(json) = serde_json::from_str::<serde_json::Value>(payload) else {
            return false;
        };
        if let Some(text) = provider_error(&json) {
            self.error.get_or_insert(text);
            return false;
        }
        let num = |p: &str| json.pointer(p).and_then(serde_json::Value::as_u64);
        if let Some(n) = num("/usage/prompt_tokens")
            .or_else(|| num("/usage/input_tokens"))
            .or_else(|| num("/message/usage/input_tokens"))
            .or_else(|| num("/prompt_eval_count"))
        {
            self.input_tokens = n;
        }
        if let Some(n) = num("/usage/completion_tokens")
            .or_else(|| num("/usage/output_tokens"))
            .or_else(|| num("/eval_count"))
        {
            self.output_tokens = n;
        }
        let piece = [
            json.pointer("/choices/0/delta/content"),
            json.pointer("/delta/text"),
            json.pointer("/message/content"),
            json.pointer("/response"),
        ]
        .into_iter()
        .flatten()
        .find_map(|v| v.as_str());
        match piece {
            Some(p) if !p.is_empty() => {
                self.text.push_str(p);
                true
            }
            _ => false,
        }
    }

    /// The finished stream as a whole, non-streamed reply document, so it is
    /// read by the same [`parse_reply`] and usage code as any other reply.
    pub fn into_body(self) -> String {
        match self.error {
            Some(e) => serde_json::json!({ "error": e }).to_string(),
            None => serde_json::json!({
                "message": { "content": self.text },
                "prompt_eval_count": self.input_tokens,
                "eval_count": self.output_tokens,
            })
            .to_string(),
        }
    }
}

/// A provider's own error text, wherever it puts it.
fn provider_error(json: &serde_json::Value) -> Option<String> {
    let node = json.get("error")?;
    if let Some(text) = node.as_str() {
        return Some(text.to_owned());
    }
    node.get("message")
        .and_then(|m| m.as_str())
        .map(|m| m.to_owned())
        .or_else(|| Some(node.to_string()))
}

/// A bounded, single-line rendering for an error message.
fn clip(text: &str) -> String {
    let flat = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if flat.chars().count() <= 200 {
        return flat;
    }
    let head: String = flat.chars().take(199).collect();
    format!("{head}…")
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Each provider's stream assembles into the reply and usage a
    /// non-streamed call would have returned.
    #[test]
    fn a_streamed_reply_assembles_in_every_provider_shape() {
        let openai = [
            r#"data: {"choices":[{"delta":{"role":"assistant"}}]}"#,
            "",
            r#"data: {"choices":[{"delta":{"content":"Hel"}}]}"#,
            r#"data: {"choices":[{"delta":{"content":"lo"}}],"usage":{"prompt_tokens":7,"completion_tokens":2}}"#,
            "data: [DONE]",
        ];
        let anthropic = [
            "event: message_start",
            r#"data: {"type":"message_start","message":{"usage":{"input_tokens":7}}}"#,
            "event: content_block_delta",
            r#"data: {"type":"content_block_delta","delta":{"type":"text_delta","text":"Hel"}}"#,
            r#"data: {"type":"content_block_delta","delta":{"type":"text_delta","text":"lo"}}"#,
            r#"data: {"type":"message_delta","usage":{"output_tokens":2}}"#,
            "data: {\"type\":\"message_stop\"}",
        ];
        let ollama = [
            r#"{"message":{"role":"assistant","content":"Hel"},"done":false}"#,
            r#"{"message":{"role":"assistant","content":"lo"},"done":false}"#,
            r#"{"message":{"role":"assistant","content":""},"done":true,"prompt_eval_count":7,"eval_count":2}"#,
        ];
        for lines in [&openai[..], &anthropic[..], &ollama[..]] {
            let mut a = StreamAssembler::default();
            let grew = lines.iter().filter(|l| a.feed(l)).count();
            assert_eq!(grew, 2, "{lines:?}");
            assert_eq!((a.text.as_str(), a.input_tokens, a.output_tokens), ("Hello", 7, 2));
            let body = a.into_body();
            assert_eq!(parse_reply(200, &body), Ok("Hello".to_string()));
            let u = crate::agent_tools::usage_from_body(&body);
            assert_eq!((u.input, u.output), (7, 2));
        }
    }

    /// An error inside a stream fails the Ask with the provider's own words.
    #[test]
    fn an_error_inside_a_stream_is_the_failure() {
        let mut a = StreamAssembler::default();
        a.feed(r#"data: {"choices":[{"delta":{"content":"Hi"}}]}"#);
        a.feed(r#"data: {"type":"error","error":{"type":"overloaded_error","message":"Overloaded"}}"#);
        assert_eq!(parse_reply(200, &a.into_body()), Err("HTTP 200: Overloaded".to_string()));
    }

    #[test]
    fn a_streaming_body_asks_for_a_stream() {
        let req = AskRequest { model: "m".into(), prompt: "q".into(), ..Default::default() };
        for p in [Protocol::OpenAiChat, Protocol::OllamaChat, Protocol::Anthropic] {
            let v: serde_json::Value = serde_json::from_str(&streaming_body(&body_for(&req, p))).unwrap();
            assert_eq!(v["stream"], serde_json::Value::Bool(true), "{p:?}");
        }
    }

    /// A path in `AgentEndpoint` joins `AgentURL`'s origin; a full URL stands
    /// (property audit, 2026-09-26).
    #[test]
    fn an_endpoint_path_joins_the_agent_url_origin() {
        let req = AskRequest {
            url: "http://localhost:11434/api".into(),
            endpoint: "/v1/chat/completions".into(),
            ..Default::default()
        };
        assert_eq!(endpoint_for(&req, Protocol::OpenAiChat), "http://localhost:11434/v1/chat/completions");
        let full = AskRequest { endpoint: "https://example.org/x".into(), ..req };
        assert_eq!(endpoint_for(&full, Protocol::OpenAiChat), "https://example.org/x");
    }
}
