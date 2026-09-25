// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! The model providers an application can offer its users — the same list,
//! the same rules and the same messages as the IDE's Model Providers Manager
//! (spec 048), so a model set up in a built application behaves exactly as it
//! does in the IDE.
//!
//! **Copied, not shared.** The IDE's version lives in `cobolt-ide`
//! (`llm.rs`: `PROVIDERS`, `provider_requires_key`, `spawn_list_models`,
//! `model_list_url`, `model_list_headers`, `is_openai_chat_model`,
//! `ollama_model_names`, `retired_model_message`, `heal_endpoint`, the
//! connection test and its error help) and in `cobolt-agents`
//! (`rig_transport::normalize_base`). A built application links neither, and
//! must not: the runtime is what ships. When the IDE's rules change, change
//! them here too — the tests on both sides name the same cases.
//!
//! Everything here is synchronous and runs on the interpreter's thread through
//! its own HTTP bridge, so a form keeps painting while a list or a test waits.

use crate::agent_runtime::{AskRequest, Protocol};
use crate::http_runtime::{HttpClient, RequestConfig};

/// One provider: its id (what a model entry's API names), the label a user
/// sees, and the endpoint to use when the user leaves it blank.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Provider {
    pub id: &'static str,
    pub label: &'static str,
    pub default_endpoint: &'static str,
}

/// The `anthropic-version` every Anthropic REST request must carry.
pub const ANTHROPIC_API_VERSION: &str = "2023-06-01";

/// The providers, in the order the IDE lists them.
pub const PROVIDERS: &[Provider] = &[
    Provider { id: "openai", label: "OpenAI", default_endpoint: "https://api.openai.com/v1" },
    Provider { id: "anthropic", label: "Anthropic", default_endpoint: "https://api.anthropic.com/v1" },
    Provider { id: "cohere", label: "Cohere", default_endpoint: "https://api.cohere.ai/v1" },
    Provider {
        id: "gemini",
        label: "Google Gemini",
        default_endpoint: "https://generativelanguage.googleapis.com/v1beta",
    },
    Provider { id: "perplexity", label: "Perplexity", default_endpoint: "https://api.perplexity.ai" },
    Provider { id: "mistral", label: "Mistral", default_endpoint: "https://api.mistral.ai/v1" },
    Provider { id: "groq", label: "Groq", default_endpoint: "https://api.groq.com/openai/v1" },
    Provider { id: "openrouter", label: "OpenRouter", default_endpoint: "https://openrouter.ai/api/v1" },
    Provider { id: "huggingface", label: "HuggingFace", default_endpoint: "https://router.huggingface.co/v1" },
    Provider { id: "together", label: "Together AI", default_endpoint: "https://api.together.xyz/v1" },
    Provider { id: "deepseek", label: "DeepSeek", default_endpoint: "https://api.deepseek.com/v1" },
    Provider {
        id: "alibaba",
        label: "Alibaba (Model Studio)",
        default_endpoint: "https://dashscope-intl.aliyuncs.com/compatible-mode/v1",
    },
    Provider { id: "xai", label: "xAI", default_endpoint: "https://api.x.ai/v1" },
    Provider { id: "voyageai", label: "Voyage AI", default_endpoint: "https://api.voyageai.com/v1" },
    Provider { id: "ollama", label: "Ollama (Local)", default_endpoint: "http://localhost:11434/api" },
    Provider { id: "ollama_cloud", label: "Ollama (Cloud)", default_endpoint: "https://ollama.com/api/chat" },
    Provider { id: "llamafile", label: "Llamafile (Local)", default_endpoint: "http://localhost:8080/v1" },
];

/// The provider an id names, ignoring case and spaces.
pub fn find(id: &str) -> Option<&'static Provider> {
    let id = id.trim();
    PROVIDERS.iter().find(|p| p.id.eq_ignore_ascii_case(id))
}

/// Whether this provider needs an API key. Local Ollama authenticates nothing,
/// so demanding a key would make a working setup look unconfigured; every
/// hosted provider — `ollama_cloud` included — needs one.
pub fn requires_key(id: &str) -> bool {
    !id.trim().eq_ignore_ascii_case("ollama")
}

/// Whether a request to this endpoint may go without a key: nothing on this
/// machine asks for one.
pub fn is_local(endpoint: &str) -> bool {
    let e = endpoint.trim().to_ascii_lowercase();
    e.contains("localhost") || e.contains("127.0.0.1")
}

/// Heal endpoints the IDE once shipped wrong: the Ollama Cloud host moved
/// (api.ollama.com → ollama.com), and HuggingFace's legacy Inference API host
/// was shut down, so its whole endpoint maps onto the router root.
pub fn heal_endpoint(ep: &str) -> String {
    let healed = ep.trim().replace("api.ollama.com", "ollama.com");
    if healed.contains("api-inference.huggingface.co") {
        return "https://router.huggingface.co/v1".to_string();
    }
    healed
}

/// The API root a request goes under. Stored endpoints may be full request
/// URLs; this strips them back to the root, maps Ollama's native `/api` onto
/// its OpenAI-compatible `/v1`, and — beyond the IDE's rule — gives a bare
/// origin the `/v1` root every provider here serves the chat under.
pub fn normalize_base(provider: &str, endpoint: &str) -> String {
    let mut base = heal_endpoint(endpoint).trim_end_matches('/').to_string();
    for suffix in ["/chat/completions", "/completions", "/messages"] {
        if let Some(stripped) = base.strip_suffix(suffix) {
            base = stripped.trim_end_matches('/').to_string();
        }
    }
    if let Some(host) = base.strip_suffix("/api/chat").or_else(|| base.strip_suffix("/api")) {
        base = format!("{}/v1", host.trim_end_matches('/'));
    }
    if base.is_empty() {
        return match provider {
            "anthropic" => "https://api.anthropic.com/v1".to_string(),
            "ollama" => "http://localhost:11434/v1".to_string(),
            _ => "https://api.openai.com/v1".to_string(),
        };
    }
    let after_scheme = base.split("://").nth(1).unwrap_or(&base);
    if !after_scheme.contains('/') {
        base.push_str("/v1");
    }
    base
}

/// Where a chat request for this provider goes, and in which shape: Anthropic
/// speaks its own `/messages`, the OpenAI-wire providers `/chat/completions`
/// under their API root — as the IDE sends them.
///
/// Ollama (local and cloud) keeps the runtime's native `/api/chat`, which the
/// AgentObject has always spoken with it, tool calling included (spec 072);
/// the IDE reaches the same models through Ollama's OpenAI-compatible root.
/// Only the provider's default endpoint, `…/api`, needed help: as a URL with a
/// path it was used as written, and `/api` alone is no chat endpoint.
pub fn chat_target(provider: &str, endpoint: &str) -> (Protocol, String) {
    let p = provider.trim().to_ascii_lowercase();
    if p == "ollama" || p == "ollama_cloud" {
        let healed = heal_endpoint(endpoint);
        let url = healed.trim_end_matches('/');
        let url = url.strip_suffix("/api").unwrap_or(url);
        let req = AskRequest { api: "ollama".into(), url: url.to_string(), ..Default::default() };
        let protocol = crate::agent_runtime::protocol_for("ollama", url);
        return (protocol, crate::agent_runtime::endpoint_for(&req, protocol));
    }
    let base = normalize_base(&p, endpoint);
    if p == "anthropic" {
        (Protocol::Anthropic, format!("{base}/messages"))
    } else {
        (Protocol::OpenAiChat, format!("{base}/chat/completions"))
    }
}

/// A model a provider has retired, with what to do about it.
pub fn retired_model_message(model: &str) -> Option<String> {
    if model.trim().eq_ignore_ascii_case("qwen3-coder-next") {
        Some(
            "`qwen3-coder-next` was retired by Ollama on 2026-07-15. \
             Refresh the model list and select a currently available model."
                .to_string(),
        )
    } else {
        None
    }
}

fn filter_retired_models(models: Vec<String>) -> Vec<String> {
    models.into_iter().filter(|m| retired_model_message(m).is_none()).collect()
}

/// The headers a model-list GET must carry: each provider names its own
/// credential header, and Anthropic needs its version on every request.
pub fn model_list_headers(provider: &str, api_key: &str) -> Vec<(String, String)> {
    let mut headers = Vec::new();
    let key = api_key.trim();
    if !key.is_empty() {
        match provider {
            "anthropic" => headers.push(("x-api-key".to_string(), key.to_string())),
            "gemini" => headers.push(("x-goog-api-key".to_string(), key.to_string())),
            _ => headers.push(("Authorization".to_string(), format!("Bearer {key}"))),
        }
    }
    if provider == "anthropic" {
        headers.push(("anthropic-version".to_string(), ANTHROPIC_API_VERSION.to_string()));
    }
    headers
}

/// Where a provider lists its models.
pub fn model_list_url(provider: &str, endpoint: &str) -> String {
    let ep = heal_endpoint(endpoint).trim_end_matches('/').to_string();
    if provider == "ollama" || provider == "ollama_cloud" {
        let root = ep
            .trim_end_matches("/api")
            .trim_end_matches("/api/chat")
            .trim_end_matches("/api/tags")
            .trim_end_matches("/v1")
            .trim_end_matches('/');
        return format!("{root}/api/tags");
    }
    if matches!(provider, "openai" | "huggingface" | "groq" | "alibaba") {
        for suffix in ["/chat/completions", "/responses"] {
            if let Some(root) = ep.strip_suffix(suffix) {
                return format!("{}/models", root.trim_end_matches('/'));
            }
        }
        return if ep.ends_with("/models") { ep } else { format!("{ep}/models") };
    }
    if provider == "anthropic" {
        let root = ep.strip_suffix("/messages").unwrap_or(&ep).trim_end_matches('/');
        return if root.ends_with("/models") { root.to_string() } else { format!("{root}/models") };
    }
    ep
}

/// OpenAI lists every model it serves; only the chat ones belong in a chat
/// application's list.
pub fn is_openai_chat_model(model: &str) -> bool {
    let m = model.trim().to_ascii_lowercase();
    if m.is_empty() {
        return false;
    }
    let excluded = [
        "embedding", "whisper", "tts", "audio", "realtime", "transcribe", "moderation", "image",
        "sora", "davinci", "babbage", "search-preview", "search-api",
    ];
    if excluded.iter().any(|needle| m.contains(needle)) {
        return false;
    }
    m.starts_with("gpt-") || m.starts_with("o1") || m.starts_with("o3") || m.starts_with("o4") || m == "chat-latest"
}

/// The model ids in an Ollama `/api/tags` body — tag included, never trimmed:
/// the untagged name is a different model.
pub fn ollama_model_names(body: &str) -> Result<Vec<String>, String> {
    let json: serde_json::Value = serde_json::from_str(body).map_err(|e| {
        format!("Ollama's model list was not JSON ({e}): {}", body.chars().take(300).collect::<String>())
    })?;
    let Some(models) = json.get("models").and_then(|m| m.as_array()) else {
        return Err(format!(
            "Ollama's model list carried no \"models\" array: {}",
            body.chars().take(300).collect::<String>()
        ));
    };
    Ok(models
        .iter()
        .filter_map(|m| {
            m.get("name")
                .and_then(|n| n.as_str())
                .or_else(|| m.get("model").and_then(|n| n.as_str()))
                .map(str::to_string)
        })
        .collect())
}

/// The model names in a provider's list reply (not Ollama's).
pub fn parse_model_list(provider: &str, body: &str) -> Result<Vec<String>, String> {
    if let Ok(json) = serde_json::from_str::<serde_json::Value>(body) {
        if let Some(data) = json.get("data").and_then(|d| d.as_array()) {
            let names = data
                .iter()
                .filter_map(|m| m.get("id").and_then(|n| n.as_str()))
                .filter(|id| provider != "openai" || is_openai_chat_model(id))
                .map(str::to_string)
                .collect();
            return Ok(filter_retired_models(names));
        }
        if let Some(models) = json.get("models").and_then(|d| d.as_array()) {
            let names = models
                .iter()
                .filter_map(|m| m.get("name").and_then(|n| n.as_str()))
                .map(|n| n.replace("models/", ""))
                .collect();
            return Ok(filter_retired_models(names));
        }
    }
    Err(format!(
        "Failed to parse model list from API response: {}",
        body.chars().take(500).collect::<String>()
    ))
}

/// Whether the IDE would fetch a list for this provider with this key: a known
/// provider, and a key when it needs one.
pub fn can_list_models(provider: &str, api_key: &str) -> bool {
    find(provider).is_some() && (!requires_key(provider) || !api_key.trim().is_empty())
}

/// Ask the provider which models it offers.
pub fn list_models(http: &HttpClient, provider: &str, endpoint: &str, api_key: &str) -> Result<Vec<String>, String> {
    let pid = provider.trim().to_ascii_lowercase();
    if find(&pid).is_none() {
        return Err(format!("\"{}\" is not a provider this application knows", provider.trim()));
    }
    if requires_key(&pid) && api_key.trim().is_empty() {
        return Err(format!("{} needs an API key before it can list its models.", label(&pid)));
    }
    let endpoint = if endpoint.trim().is_empty() {
        find(&pid).map(|p| p.default_endpoint).unwrap_or_default().to_string()
    } else {
        endpoint.to_string()
    };
    let url = model_list_url(&pid, &endpoint);
    let cfg = RequestConfig {
        timeout_ms: 30_000,
        follow_redirects: true,
        verify_tls: true,
        headers: model_list_headers(&pid, api_key),
    };
    let (body, status) = http.send_configured("GET", &url, None, &cfg);
    let ollama = pid == "ollama" || pid == "ollama_cloud";
    if status == 0 {
        return Err(if ollama {
            format!("Failed to reach Ollama at {url}: {body}")
        } else {
            format!("Failed to fetch models from API: {body}")
        });
    }
    if !(200..300).contains(&status) {
        let head: String = body.chars().take(500).collect();
        return Err(if ollama {
            format!("Failed to fetch models from Ollama ({status}): {head}")
        } else {
            format!("Failed to fetch models from API ({status}): {head}")
        });
    }
    if ollama {
        ollama_model_names(&body).map(filter_retired_models)
    } else {
        parse_model_list(&pid, &body)
    }
}

fn label(pid: &str) -> &'static str {
    find(pid).map(|p| p.label).unwrap_or("This provider")
}

/// Why a request must not be sent: a hosted provider with no key. An
/// unauthenticated call comes back as 401 and reads like a rejected key.
pub fn credential_gap(provider: &str, model: &str, endpoint: &str, api_key: &str) -> Option<String> {
    if !api_key.trim().is_empty() || endpoint.trim().is_empty() || is_local(endpoint) {
        return None;
    }
    if !requires_key(provider) {
        return None;
    }
    Some(format!(
        "no API key reached the request for provider \"{}\" model \"{}\" ({}). \
         The request was NOT sent, because an unauthenticated call comes back \
         as 401 and reads like a rejected key. Check that this model's key is \
         set — and if it is, the credential was lost on the way to this \
         particular call rather than missing.",
        provider.trim(),
        model.trim(),
        endpoint.trim()
    ))
}

/// Whether a provider error is an authorization rejection.
pub fn is_unauthorized(msg: &str) -> bool {
    let m = msg.to_ascii_lowercase();
    m.contains("unauthorized") || m.contains("status code 401") || m.contains("(401") || m.contains("http 401")
}

/// What to check when the provider answers 401, in the order that resolves it
/// fastest.
pub fn unauthorized_help(provider: &str, model: &str) -> String {
    format!(
        "the provider rejected the credential (401 Unauthorized). Check, in this order:\n\
         1. that a VALID API key for \"{provider}\" is set for this model;\n\
         2. whether that key has EXPIRED or been rotated at the provider — providers commonly expire keys on a schedule;\n\
         3. that the model \"{model}\" is still offered by \"{provider}\" — a retired or renamed model answers 401 as readily as a bad key.\n\
         The fix for an expired key is to renew it at the provider and store the new one."
    )
}

/// The connection test's request: one tiny question, the same one the IDE
/// asks. The token budget is capped at 16 — reachability is the question, and
/// any reply at all (reasoning included) answers it.
pub fn connection_test_request(provider: &str, endpoint: &str, model: &str, api_key: &str) -> AskRequest {
    AskRequest {
        api: provider.trim().to_string(),
        url: heal_endpoint(endpoint),
        endpoint: String::new(),
        model: model.trim().to_string(),
        api_key: api_key.trim().to_string(),
        system_prompt: "You are testing model access. Reply with the exact text OK and nothing else.".to_string(),
        prompt: "Reply with OK only.".to_string(),
        // 1.0: some models accept only their default temperature, and forcing
        // 0 fails a perfectly good model's test.
        temperature: 100,
        max_tokens: 16,
    }
}

/// Test that `model` answers through `provider` at `endpoint` with `api_key`.
/// `Ok(())`, or the message a user can act on — each IDE rule applied in the
/// IDE's order: a retired model, a missing key (nothing sent), then the reply.
pub fn test_connection(
    http: &HttpClient,
    provider: &str,
    endpoint: &str,
    model: &str,
    api_key: &str,
) -> Result<(), String> {
    let pid = provider.trim().to_ascii_lowercase();
    if find(&pid).is_none() {
        return Err(format!("\"{}\" is not a provider this application knows", provider.trim()));
    }
    let model = model.trim();
    if model.is_empty() {
        return Err("No model is chosen for this provider — press Refresh models first.".to_string());
    }
    if let Some(msg) = retired_model_message(model) {
        return Err(format!("{model}: Connection test failed: {msg}"));
    }
    let endpoint = if endpoint.trim().is_empty() {
        find(&pid).map(|p| p.default_endpoint).unwrap_or_default().to_string()
    } else {
        endpoint.to_string()
    };
    if let Some(gap) = credential_gap(&pid, model, &endpoint, api_key) {
        return Err(format!("{model}: Connection test failed: {gap}"));
    }
    let req = connection_test_request(&pid, &endpoint, model, api_key);
    let (protocol, url) = chat_target(&pid, &req.url);
    let cfg = RequestConfig {
        timeout_ms: 60_000,
        follow_redirects: true,
        verify_tls: true,
        headers: crate::agent_runtime::headers_for(&req, protocol),
    };
    let body = crate::agent_runtime::body_for(&req, protocol);
    let (reply, status) = http.send_configured("POST", &url, Some(&body), &cfg);
    let outcome = if status == 0 {
        Err(format!("could not reach {url}: {reply}"))
    } else {
        crate::agent_runtime::parse_reply(status, &reply).map(|_| ())
    };
    outcome.map_err(|e| {
        let e = if is_unauthorized(&e) || status == 401 {
            format!("{e}\n{}", unauthorized_help(&pid, model))
        } else {
            e
        };
        format!("{model}: Connection test failed: {e}")
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The same cases the IDE's own tests pin, so the two lists cannot drift
    /// apart unnoticed.
    #[test]
    fn the_catalogue_matches_the_ide() {
        assert_eq!(PROVIDERS.len(), 17);
        assert_eq!(find("OpenAI").unwrap().default_endpoint, "https://api.openai.com/v1");
        assert!(requires_key("ollama_cloud") && requires_key("llamafile") && !requires_key("Ollama"));
        assert_eq!(model_list_url("openai", "https://api.openai.com/v1/chat/completions"), "https://api.openai.com/v1/models");
        assert_eq!(model_list_url("anthropic", "https://api.anthropic.com/v1/messages"), "https://api.anthropic.com/v1/models");
        assert_eq!(model_list_url("ollama", "http://localhost:11434/api"), "http://localhost:11434/api/tags");
        assert_eq!(model_list_url("ollama_cloud", "https://api.ollama.com/api/chat"), "https://ollama.com/api/tags");
        assert_eq!(model_list_url("mistral", "https://api.mistral.ai/v1"), "https://api.mistral.ai/v1");
        assert_eq!(
            model_list_headers("anthropic", "k"),
            vec![("x-api-key".to_string(), "k".to_string()), ("anthropic-version".to_string(), ANTHROPIC_API_VERSION.to_string())]
        );
        assert_eq!(model_list_headers("gemini", "k")[0].0, "x-goog-api-key");
        assert!(is_openai_chat_model("gpt-4o") && !is_openai_chat_model("text-embedding-3-large"));
        assert_eq!(heal_endpoint("https://api-inference.huggingface.co/models/x"), "https://router.huggingface.co/v1");
    }

    #[test]
    fn every_provider_gets_the_chat_address_the_ide_sends_to() {
        let table: Vec<(String, Protocol, String)> = PROVIDERS
            .iter()
            .map(|p| {
                let (proto, url) = chat_target(p.id, p.default_endpoint);
                (p.id.to_string(), proto, url)
            })
            .collect();
        for (id, proto, url) in &table {
            if id == "anthropic" {
                assert_eq!((*proto, url.as_str()), (Protocol::Anthropic, "https://api.anthropic.com/v1/messages"));
            } else if id.starts_with("ollama") {
                assert_eq!(*proto, Protocol::OllamaChat, "{id}");
                assert!(url.ends_with("/api/chat"), "{id}: {url}");
            } else {
                assert_eq!(*proto, Protocol::OpenAiChat, "{id}");
                assert!(url.ends_with("/chat/completions"), "{id}: {url}");
            }
        }
        // Stored full URLs and bare origins land on the same address.
        assert_eq!(chat_target("openai", "https://api.openai.com/v1/chat/completions").1, "https://api.openai.com/v1/chat/completions");
        assert_eq!(chat_target("ollama", "http://localhost:11434").1, "http://localhost:11434/api/chat");
        assert_eq!(chat_target("ollama", "http://localhost:11434/api").1, "http://localhost:11434/api/chat");
        assert_eq!(chat_target("ollama_cloud", "https://api.ollama.com/api/chat").1, "https://ollama.com/api/chat");
        // A developer who wrote Ollama's OpenAI-compatible path keeps it.
        assert_eq!(chat_target("ollama", "http://localhost:11434/v1/chat/completions").0, Protocol::OpenAiChat);
    }

    #[test]
    fn the_test_refuses_before_sending_what_the_ide_refuses() {
        let http = HttpClient::new();
        let e = test_connection(&http, "openai", "", "gpt-4o", "").unwrap_err();
        assert!(e.contains("no API key reached the request") && e.contains("NOT sent"), "{e}");
        let e = test_connection(&http, "ollama", "", "qwen3-coder-next", "").unwrap_err();
        assert!(e.contains("was retired by Ollama"), "{e}");
        let e = test_connection(&http, "nobody", "", "m", "k").unwrap_err();
        assert!(e.contains("not a provider"), "{e}");
        let e = test_connection(&http, "openai", "", "", "k").unwrap_err();
        assert!(e.contains("Refresh models"), "{e}");
        assert!(unauthorized_help("openai", "gpt-4o").contains("EXPIRED"));
    }
}
