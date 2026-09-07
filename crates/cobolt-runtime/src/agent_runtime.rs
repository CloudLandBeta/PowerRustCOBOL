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
