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
    /// Explicit target specialist (FormsDesigner, EventBinder, CodeGenerator).
    /// If None, the orchestrator routes based on the user prompt.
    pub specialist: Option<String>,
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
        on_chunk: &(dyn Fn(&str) + Send + Sync),
    ) -> Result<(String, String), String> {
        // ── Route ────────────────────────────────────────────────────────────
        let target = if let Some(ref explicit) = req.specialist {
            explicit.clone()
        } else {
            route_specialist(&req.user_prompt).to_string()
        };
        let specialist = self
            .specialists
            .iter()
            .find(|s| s.name == target)
            .expect("built-in specialist");
        on_log(format!("routing → {} specialist", specialist.name));

        // ── Compose messages (host prompt wins; specialist is the fallback) ──
        // ── Compose messages (host prompt + specialist is combined) ──
        let system = if req.system_prompt.trim().is_empty() {
            specialist.system_prompt.clone()
        } else {
            format!(
                "{}\n\n{}",
                req.system_prompt.trim(),
                specialist.system_prompt
            )
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

        let mut full_trace = String::new();
        let mut final_content = String::new();
        let mut operations_acc: Vec<serde_json::Value> = Vec::new();
        let mut loop_count = 0;
        let max_loops = 10;

        loop {
            loop_count += 1;
            // ── Resolve URL + wire shape ─────────────────────────────────────────
            // Generic across providers: heal known-bad hosts (provider-keyed),
            // then let an EXPLICIT endpoint suffix decide the wire format so a
            // configured `/v1/chat/completions` gets the OpenAI wire even on an
            // Ollama-family provider (and vice-versa). `base` is kept for the
            // reasoning-hint check below.
            let base = heal_endpoint_host(&req.provider, req.endpoint.trim().trim_end_matches('/'));
            // `native` = Ollama-native chat API (`…/api/chat`, response shape
            // `{"message":{"content":…}}`); otherwise OpenAI-compatible
            // (`…/chat/completions`, response shape `choices[0].message.content`).
            let (url, native) = resolve_wire(&req.provider, &base);

            let mut body = if native {
                serde_json::json!({
                    "model": req.model,
                    "messages": messages,
                    "stream": true,
                    "think": false,
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
                    "stream": true,
                })
            };
            if !native && provider_uses_ollama_reasoning(&req.provider, &base) {
                // Ollama-family OpenAI-compatible endpoints accept this hint for
                // thinking models. Other providers can reject unknown fields, so
                // keep it scoped to endpoints we know may emit `delta.reasoning`.
                body["think"] = serde_json::Value::Bool(false);
            }

            on_log(format!(
                "POST {url} · model {} · {} message(s) · {} wire format (batch {loop_count})",
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
                let payload_str = serde_json::to_string_pretty(&body).unwrap_or_default();
                trace.push_str(&format!("Payload:\n{}\n", payload_str));
                if loop_count == 1 {
                    on_log(format!("Verbose: Loaded Skills:\n{}", req.skills));
                }
                on_log(format!("Verbose: Request URL: {}", url));
                on_log(format!("Verbose: Payload:\n{}", payload_str));
            }

            // ── Call ─────────────────────────────────────────────────────────────
            let client = reqwest::Client::new();
            let mut http = client.post(&url);
            if !req.api_key.trim().is_empty() {
                http = http.bearer_auth(req.api_key.trim());
            }

            let started = std::time::Instant::now();
            let mut resp = http.json(&body).send().await.map_err(|e| {
                let msg = format!("Could not reach the model: {e}");
                on_log(format!(
                    "network error after {:.1}s: {e}",
                    started.elapsed().as_secs_f32()
                ));
                msg
            })?;
            let status = resp.status();

            let mut raw = String::new();
            let mut full_content = String::new();
            let mut reasoning_chars: usize = 0;
            let mut line_buf = String::new();
            let mut inp_tokens: Option<u64> = None;
            let mut out_tokens: Option<u64> = None;

            while let match_result = resp.chunk().await {
                match match_result {
                    Ok(Some(chunk)) => {
                        let text = String::from_utf8_lossy(&chunk);
                        raw.push_str(&text);
                        line_buf.push_str(&text);

                        while let Some(idx) = line_buf.find('\n') {
                            let line = line_buf[..idx].trim_end().to_string();
                            line_buf = line_buf[idx + 1..].to_string();

                            if line.is_empty() {
                                continue;
                            }

                            if line.starts_with("data: ") {
                                let json_str = line.strip_prefix("data: ").unwrap();
                                if json_str == "[DONE]" {
                                    continue;
                                }
                                if let Ok(json) =
                                    serde_json::from_str::<serde_json::Value>(json_str)
                                {
                                    if let Some(content) = json
                                        .get("choices")
                                        .and_then(|c| c.get(0))
                                        .and_then(|c| c.get("delta"))
                                        .and_then(|m| m.get("content"))
                                        .and_then(|c| c.as_str())
                                    {
                                        full_content.push_str(content);
                                        on_chunk(content);
                                    }
                                    if let Some(reasoning) = json
                                        .get("choices")
                                        .and_then(|c| c.get(0))
                                        .and_then(|c| c.get("delta"))
                                        .and_then(|m| {
                                            m.get("reasoning")
                                                .or_else(|| m.get("reasoning_content"))
                                        })
                                        .and_then(|c| c.as_str())
                                    {
                                        reasoning_chars += reasoning.chars().count();
                                    }
                                    if let Some(usage) = json.get("usage") {
                                        if let Some(i) =
                                            usage.get("prompt_tokens").and_then(|v| v.as_u64())
                                        {
                                            inp_tokens = Some(i);
                                        }
                                        if let Some(o) =
                                            usage.get("completion_tokens").and_then(|v| v.as_u64())
                                        {
                                            out_tokens = Some(o);
                                        }
                                    }
                                }
                            } else {
                                if let Ok(json) = serde_json::from_str::<serde_json::Value>(&line) {
                                    if let Some(content) = json
                                        .get("message")
                                        .and_then(|m| m.get("content"))
                                        .and_then(|c| c.as_str())
                                    {
                                        full_content.push_str(content);
                                        on_chunk(content);
                                    }
                                    if let Some(reasoning) = json
                                        .get("message")
                                        .and_then(|m| {
                                            m.get("reasoning")
                                                .or_else(|| m.get("reasoning_content"))
                                        })
                                        .and_then(|c| c.as_str())
                                    {
                                        reasoning_chars += reasoning.chars().count();
                                    }
                                    if let Some(usage) =
                                        json.get("prompt_eval_count").and_then(|v| v.as_u64())
                                    {
                                        inp_tokens = Some(usage);
                                    }
                                    if let Some(usage) =
                                        json.get("eval_count").and_then(|v| v.as_u64())
                                    {
                                        out_tokens = Some(usage);
                                    }
                                }
                            }
                        }
                    }
                    Ok(None) => break,
                    Err(e) => return Err(format!("Stream error: {e}")),
                }
            }

            let secs = started.elapsed().as_secs_f32();
            on_log(format!("HTTP {status} in {secs:.1}s · {} bytes", raw.len()));

            trace.push_str(&format!(
                "=== API RESPONSE ===\nStatus: {status}\nDuration: {secs:.1}s\nBytes: {}\n",
                raw.len()
            ));
            if req.verbose {
                trace.push_str(&format!("Body:\n{raw}\n"));
            }

            full_trace.push_str(&trace);
            full_trace.push_str("\n");

            if !status.is_success() {
                return Err(format!("Model returned HTTP {status}. {raw}"));
            }

            if let (Some(inp), Some(out)) = (inp_tokens, out_tokens) {
                on_log(format!("tokens: {inp} in / {out} out"));
            } else if req.verbose {
                if let Some(inp) = inp_tokens {
                    on_log(format!("Verbose: tokens: {inp} in"));
                }
                if let Some(out) = out_tokens {
                    on_log(format!("Verbose: tokens: {out} out"));
                }
            }

            if full_content.is_empty() && reasoning_chars > 0 {
                return Err(format!(
                    "The model returned {reasoning_chars} reasoning characters but no assistant \
                     message content. PowerRustCOBOL cannot apply hidden reasoning as form \
                     operations. Use a non-reasoning/chat model or disable thinking/reasoning for \
                     this model. First 300 bytes: {}",
                    &raw.chars().take(300).collect::<String>()
                ));
            }

            if full_content.is_empty() {
                return Err(format!(
                    "The model response did not contain any message content. \
                     First 300 bytes: {}",
                    &raw.chars().take(300).collect::<String>()
                ));
            }

            // ── Pagination Loop Check ──
            if let Ok(json) = serde_json::from_str::<serde_json::Value>(&full_content) {
                if let Some(ops) = json.get("operations").and_then(|o| o.as_array()) {
                    operations_acc.extend(ops.clone());
                }

                let has_more = json
                    .get("has_more")
                    .and_then(|h| h.as_bool())
                    .unwrap_or(false);
                let response_id = json
                    .get("response_id")
                    .and_then(|r| r.as_str())
                    .unwrap_or("");
                let next_cursor = json.get("next_cursor").and_then(|c| c.as_str());

                if has_more && next_cursor.is_some() && loop_count < max_loops {
                    messages.push(serde_json::json!({
                        "role": "assistant",
                        "content": full_content
                    }));
                    messages.push(serde_json::json!({
                        "role": "user",
                        "content": format!("Continue response {} from cursor {}. Return only the next complete JSON batch.", response_id, next_cursor.unwrap())
                    }));
                    on_log(format!(
                        "Pagination detected! Fetching batch {}...",
                        loop_count + 1
                    ));
                    continue;
                }
            }

            // Loop break condition reached
            if !operations_acc.is_empty() {
                let synthesized = serde_json::json!({ "operations": operations_acc });
                final_content = serde_json::to_string_pretty(&synthesized).unwrap_or(full_content);
            } else {
                final_content = full_content;
            }
            break;
        }

        Ok((final_content, full_trace))
    }
}

pub fn route_specialist(prompt: &str) -> &'static str {
    let lower = prompt.to_lowercase();

    // PowerRustCOBOL UI languages: English, Spanish, Portuguese, Japanese,
    // Chinese, and French. Event intent wins over form/control nouns so
    // "bind the click event to my button" does not route to form layout.
    if contains_any(
        &lower,
        &[
            "event",
            "evento",
            "événement",
            "evenement",
            "イベント",
            "事件",
            "click",
            "clic",
            "clique",
            "クリック",
            "单击",
            "点击",
            "handler",
            "manejador",
            "manipulador",
            "gestionnaire",
            "ハンドラ",
            "处理",
            "bind",
            "wire",
            "wiring",
            "connect",
            "hook",
            "handle",
            "make the",
            "make my",
            "when pressed",
            "when clicked",
            "action",
            "enlazar",
            "vincular",
            "ligar",
            "conectar",
            "conecte",
            "conecta",
            "acionar",
            "acione",
            "quando clicar",
            "ao clicar",
            "acción",
            "acao",
            "ação",
            "associer",
            "connecter",
            "バインド",
            "绑定",
            "onload",
            "onclose",
            "save/update/delete",
            "save",
            "update",
            "delete",
            "navigate",
            "next",
            "previous",
            "first",
            "last",
            "indexed file",
            "indexed",
            "guardar",
            "salvar",
            "actualizar",
            "atualizar",
            "eliminar",
            "excluir",
            "borrar",
            "navegar",
            "siguiente",
            "próximo",
            "proximo",
            "anterior",
        ],
    ) {
        return "EventBinder";
    }

    if contains_any(
        &lower,
        &[
            "form",
            "formulario",
            "formulário",
            "formulaire",
            "フォーム",
            "表单",
            "窗体",
            "ui",
            "screen",
            "pantalla",
            "tela",
            "janela",
            "écran",
            "ecran",
            "画面",
            "屏幕",
            "control",
            "controle",
            "contrôle",
            "コントロール",
            "控件",
            "button",
            "boton",
            "botón",
            "botao",
            "botão",
            "bouton",
            "ボタン",
            "按钮",
            "tabcontrol",
            "tab control",
            "tab1",
            "aba",
            "pestaña",
            "onglet",
            "タブ",
            "选项卡",
            "add",
            "create",
            "insert",
            "place",
            "deploy",
            "añade",
            "agrega",
            "agregue",
            "adicion",
            "adicione",
            "coloca",
            "poner",
            "ponga",
            "ajouter",
            "créer",
            "creer",
            "insérer",
            "inserer",
            "追加",
            "作成",
            "添加",
            "新增",
            "创建",
        ],
    ) {
        return "FormsDesigner";
    }

    "CodeGenerator"
}

fn provider_uses_ollama_reasoning(provider: &str, base_url: &str) -> bool {
    let provider = provider.to_ascii_lowercase();
    let base_url = base_url.to_ascii_lowercase();
    provider.contains("ollama")
        || base_url.contains("ollama.com")
        || base_url.contains("localhost")
        || base_url.contains("127.0.0.1")
}

fn provider_is_ollama(provider: &str) -> bool {
    provider.to_ascii_lowercase().contains("ollama")
}

/// Correct known-bad hosts for a provider before a request is attempted.
///
/// Generic and data-driven: each row maps a `(provider-name fragment,
/// wrong host, canonical host)`. The provider NAME is part of the key so a
/// correction never fires for an unrelated provider that merely shares a
/// hostname fragment — adding a provider is one row. `base` should already be
/// trimmed and have no trailing slash.
fn heal_endpoint_host(provider: &str, base: &str) -> String {
    // (provider-name fragment, wrong host, canonical host)
    const HEALS: &[(&str, &str, &str)] = &[
        // Ollama Cloud shipped an early default pointing at the wrong host.
        ("ollama", "api.ollama.com", "ollama.com"),
    ];
    let p = provider.to_ascii_lowercase();
    let mut out = base.to_string();
    for (prov, wrong, right) in HEALS {
        if p.contains(prov) && out.contains(wrong) {
            out = out.replace(wrong, right);
        }
    }
    out
}

/// Resolve the chat URL and wire format from a (healed) endpoint, generically
/// across providers and models.
///
/// Priority — most specific wins, so a user's explicit choice is honoured:
///   1. an explicit wire suffix (`/api/chat` → native, `/chat/completions` →
///      OpenAI) is used verbatim, regardless of the provider name;
///   2. a version-only suffix (`/api` → native, `/v1` → OpenAI) implies its
///      conventional wire;
///   3. otherwise the provider default (Ollama-family → native `/api/chat`,
///      everything else → OpenAI-compatible `/v1/chat/completions`).
fn resolve_wire(provider: &str, base: &str) -> (String, bool) {
    if base.ends_with("/api/chat") {
        return (base.to_string(), true);
    }
    if base.ends_with("/chat/completions") {
        return (base.to_string(), false);
    }
    if base.ends_with("/api") {
        return (format!("{base}/chat"), true);
    }
    if base.ends_with("/v1") {
        return (format!("{base}/chat/completions"), false);
    }
    if provider_is_ollama(provider) {
        (ollama_native_chat_url(base), true)
    } else {
        (format!("{base}/v1/chat/completions"), false)
    }
}

fn ollama_native_chat_url(base: &str) -> String {
    for suffix in ["/v1/chat/completions", "/api/chat", "/chat/completions"] {
        if let Some(root) = base.strip_suffix(suffix) {
            return format!("{root}/api/chat");
        }
    }
    if let Some(root) = base.strip_suffix("/v1") {
        return format!("{root}/api/chat");
    }
    if base.ends_with("/api") {
        format!("{base}/chat")
    } else {
        format!("{base}/api/chat")
    }
}

fn contains_any(haystack: &str, needles: &[&str]) -> bool {
    needles.iter().any(|needle| haystack.contains(needle))
}

#[cfg(test)]
mod tests {
    use super::{ollama_native_chat_url, route_specialist};

    #[test]
    fn routes_crud_navigation_wiring_to_event_binder() {
        assert_eq!(
            route_specialist(
                "wire the Save, Update, Delete, Next and Previous buttons to the indexed file"
            ),
            "EventBinder"
        );
        assert_eq!(
            route_specialist("conecte guardar actualizar eliminar y navegar el archivo indexado"),
            "EventBinder"
        );
        assert_eq!(
            route_specialist("fazer os botões salvar atualizar excluir e navegar funcionarem"),
            "EventBinder"
        );
    }

    #[test]
    fn routes_plain_layout_request_to_forms_designer() {
        assert_eq!(
            route_specialist("add a button to Tab1 of TabControl-1"),
            "FormsDesigner"
        );
    }

    #[test]
    fn normalizes_ollama_openai_urls_to_native_chat() {
        assert_eq!(
            ollama_native_chat_url("https://ollama.com/v1"),
            "https://ollama.com/api/chat"
        );
        assert_eq!(
            ollama_native_chat_url("https://ollama.com/v1/chat/completions"),
            "https://ollama.com/api/chat"
        );
        assert_eq!(
            ollama_native_chat_url("http://localhost:11434/api"),
            "http://localhost:11434/api/chat"
        );
    }

    #[test]
    fn host_healing_is_provider_scoped() {
        use super::heal_endpoint_host;
        // Ollama-family: the early wrong host is corrected.
        assert_eq!(
            heal_endpoint_host("ollama_cloud", "https://api.ollama.com/v1/chat/completions"),
            "https://ollama.com/v1/chat/completions"
        );
        // A provider whose canonical host legitimately begins with `api.` is
        // left untouched — the correction is keyed by provider name.
        assert_eq!(
            heal_endpoint_host("openai", "https://api.openai.com/v1"),
            "https://api.openai.com/v1"
        );
        assert_eq!(
            heal_endpoint_host("anthropic", "https://api.anthropic.com/v1"),
            "https://api.anthropic.com/v1"
        );
    }

    #[test]
    fn wire_format_follows_the_explicit_endpoint_across_providers() {
        use super::resolve_wire;
        // Explicit OpenAI suffix wins even on an Ollama-family provider.
        assert_eq!(
            resolve_wire("ollama_cloud", "https://ollama.com/v1/chat/completions"),
            ("https://ollama.com/v1/chat/completions".into(), false)
        );
        // Explicit native suffix wins even on a non-Ollama provider.
        assert_eq!(
            resolve_wire("openai", "http://host/api/chat"),
            ("http://host/api/chat".into(), true)
        );
        // Version-only suffixes imply their conventional wire.
        assert_eq!(
            resolve_wire("ollama", "http://host/api"),
            ("http://host/api/chat".into(), true)
        );
        assert_eq!(
            resolve_wire("openai", "https://api.openai.com/v1"),
            ("https://api.openai.com/v1/chat/completions".into(), false)
        );
        // Bare host: provider default. Any non-Ollama provider (Anthropic,
        // xAI, Mistral, a generic OpenAI-compatible gateway…) gets the
        // OpenAI-compatible wire.
        assert_eq!(
            resolve_wire("mistral", "https://api.mistral.ai"),
            ("https://api.mistral.ai/v1/chat/completions".into(), false)
        );
        assert_eq!(
            resolve_wire("ollama", "http://localhost:11434"),
            ("http://localhost:11434/api/chat".into(), true)
        );
    }
}
