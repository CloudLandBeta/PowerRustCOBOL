// SPDX-License-Identifier: Apache-2.0

//! Orchestrator — routes a developer request to a specialist and executes the
//! model call against either the Ollama-native chat API or any
//! OpenAI-compatible endpoint (OpenAI, OpenRouter, Ollama Cloud, LM Studio…).
//!
//! Observability: every step is reported through the `on_log` callback so the
//! host (the IDE) can stream it into the Agentic AI activity log — routing
//! decision, resolved URL, HTTP status, duration, payload sizes. A trace of
//! the full request/response is returned for the connection log (bodies only
//! included when `verbose` is set).

use crate::specialist::Specialist;

/// Everything one mesh request needs. The host composes the system prompt /
/// skills / context (spec 025 R14/R21/R2 contract) — the orchestrator never
/// invents or drops them.
pub struct MeshRequest {
    pub provider: String,
    pub model: String,
    pub api_key: String,
    pub endpoint: String,
    /// Full system prompt composed by the host. Empty ⇒ the routed
    /// specialist's built-in preamble is used instead.
    pub system_prompt: String,
    /// Reference material (skills) — sent as a second system message.
    pub skills: String,
    /// Per-request context (form snapshot, current code…) appended to the
    /// final user message.
    pub context: String,
    /// Prior conversation turns as (role, content).
    pub history: Vec<(String, String)>,
    pub user_prompt: String,
    pub temperature: f32,
    pub max_tokens: u32,
    /// Include full request/response bodies in the returned trace.
    pub verbose: bool,
}

pub struct Orchestrator {
    specialists: Vec<Specialist>,
}

impl Default for Orchestrator {
    fn default() -> Self {
        Self::new()
    }
}

impl Orchestrator {
    pub fn new() -> Self {
        Self {
            specialists: vec![
                Specialist::new("FormsDesigner"),
                Specialist::new("CodeGenerator"),
                Specialist::new("EventBinder"),
            ],
        }
    }

    /// Execute one request. `on_log` receives human-readable progress lines
    /// as they happen (routing, URL, status, timing) for the host's activity
    /// log. Returns `(reply_text, trace)` — the trace always carries at least
    /// endpoint/status/duration; full bodies only when `req.verbose`.
    pub async fn handle_request(
        &self,
        req: &MeshRequest,
        on_log: &(dyn Fn(String) + Send + Sync),
    ) -> Result<(String, String), String> {
        // ── Route ────────────────────────────────────────────────────────────
        let lower = req.user_prompt.to_lowercase();
        let target = if lower.contains("form") || lower.contains("ui") || lower.contains("screen")
        {
            "FormsDesigner"
        } else if lower.contains("event") || lower.contains("click") || lower.contains("bind") {
            "EventBinder"
        } else {
            "CodeGenerator"
        };
        let specialist = self
            .specialists
            .iter()
            .find(|s| s.name == target)
            .expect("built-in specialist");
        on_log(format!("routing → {} specialist", specialist.name));

        // ── Compose messages (host prompt wins; specialist is the fallback) ──
        let system = if req.system_prompt.trim().is_empty() {
            specialist.system_prompt.clone()
        } else {
            req.system_prompt.clone()
        };
        let mut messages = vec![serde_json::json!({ "role": "system", "content": system })];
        if !req.skills.trim().is_empty() {
            messages.push(serde_json::json!({
                "role": "system",
                "content": format!("Reference material (skills):\n\n{}", req.skills),
            }));
        }
        for (role, content) in &req.history {
            messages.push(serde_json::json!({
                "role": if role == "user" { "user" } else { "assistant" },
                "content": content,
            }));
        }
        let user = if req.context.trim().is_empty() {
            req.user_prompt.trim().to_string()
        } else {
            format!("{}\n\n{}", req.user_prompt.trim(), req.context)
        };
        messages.push(serde_json::json!({ "role": "user", "content": user }));

        // ── Resolve URL + wire shape ─────────────────────────────────────────
        // Heal the previously shipped wrong Ollama Cloud host: the service
        // lives at ollama.com (native `/api`, OpenAI-compatible `/v1`), NOT
        // api.ollama.com. Saved configs may still carry the old value.
        let base = req
            .endpoint
            .trim()
            .trim_end_matches('/')
            .replace("api.ollama.com", "ollama.com");
        // `native` = Ollama-native chat API (`…/api/chat`, response shape
        // `{"message":{"content":…}}`); otherwise OpenAI-compatible
        // (`…/chat/completions`, response shape `choices[0].message.content`).
        let (url, native) = if base.ends_with("/api/chat") {
            (base.clone(), true)
        } else if base.ends_with("/chat/completions") {
            (base.clone(), false)
        } else if base.ends_with("/api") {
            (format!("{base}/chat"), true)
        } else if base.ends_with("/v1") {
            (format!("{base}/chat/completions"), false)
        } else if req.provider == "ollama" {
            (format!("{base}/api/chat"), true)
        } else {
            (format!("{base}/v1/chat/completions"), false)
        };

        let body = if native {
            serde_json::json!({
                "model": req.model,
                "messages": messages,
                "stream": false,
                "options": {
                    "temperature": req.temperature,
                    "num_predict": req.max_tokens,
                },
            })
        } else {
            serde_json::json!({
                "model": req.model,
                "messages": messages,
                "temperature": req.temperature,
                "max_tokens": req.max_tokens,
                "stream": false,
            })
        };

        on_log(format!(
            "POST {url} · model {} · {} message(s) · {} wire format",
            req.model,
            messages.len(),
            if native { "ollama-native" } else { "openai" },
        ));

        let mut trace = format!(
            "=== API REQUEST ===\nEndpoint: {url}\nWire: {}\nModel: {}\nMessages: {}\n",
            if native { "ollama-native" } else { "openai" },
            req.model,
            messages.len(),
        );
        if req.verbose {
            trace.push_str(&format!(
                "Payload:\n{}\n",
                serde_json::to_string_pretty(&body).unwrap_or_default()
            ));
        }

        // ── Call ─────────────────────────────────────────────────────────────
        let client = reqwest::Client::new();
        let mut http = client.post(&url);
        if !req.api_key.trim().is_empty() {
            http = http.bearer_auth(req.api_key.trim());
        }

        let started = std::time::Instant::now();
        let resp = http.json(&body).send().await.map_err(|e| {
            let msg = format!("Could not reach the model: {e}");
            on_log(format!("network error after {:.1}s: {e}", started.elapsed().as_secs_f32()));
            msg
        })?;
        let status = resp.status();
        let raw = resp.text().await.unwrap_or_default();
        let secs = started.elapsed().as_secs_f32();
        on_log(format!("HTTP {status} in {secs:.1}s · {} bytes", raw.len()));

        trace.push_str(&format!(
            "=== API RESPONSE ===\nStatus: {status}\nDuration: {secs:.1}s\nBytes: {}\n",
            raw.len()
        ));
        if req.verbose {
            trace.push_str(&format!("Body:\n{raw}\n"));
        }

        if !status.is_success() {
            return Err(format!("Model returned HTTP {status}. {raw}"));
        }

        // ── Parse (accept both wire shapes regardless of what we guessed) ────
        let json: serde_json::Value = serde_json::from_str(&raw)
            .map_err(|e| format!("Could not read the model response: {e}"))?;
        let content = json
            // OpenAI-compatible: choices[0].message.content
            .get("choices")
            .and_then(|c| c.get(0))
            .and_then(|c| c.get("message"))
            .and_then(|m| m.get("content"))
            .and_then(|c| c.as_str())
            // Ollama-native: message.content
            .or_else(|| {
                json.get("message")
                    .and_then(|m| m.get("content"))
                    .and_then(|c| c.as_str())
            });
        // Token accounting when the provider reports it (both shapes).
        let tokens = json
            .get("usage") // openai
            .map(|u| {
                (
                    u.get("prompt_tokens").and_then(|v| v.as_u64()),
                    u.get("completion_tokens").and_then(|v| v.as_u64()),
                )
            })
            .or_else(|| {
                // ollama-native
                Some((
                    json.get("prompt_eval_count").and_then(|v| v.as_u64()),
                    json.get("eval_count").and_then(|v| v.as_u64()),
                ))
            });
        if let Some((Some(inp), Some(out))) = tokens {
            on_log(format!("tokens: {inp} in / {out} out"));
        }

        match content {
            Some(text) => Ok((text.to_string(), trace)),
            None => Err(format!(
                "The model response did not contain any message content. \
                 First 300 bytes: {}",
                &raw.chars().take(300).collect::<String>()
            )),
        }
    }
}
