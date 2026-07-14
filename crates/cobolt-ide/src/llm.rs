// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors

use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver};
use std::time::Duration;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

use cobolt_agents::Orchestrator;

pub const DEFAULT_SYSTEM_PROMPT: &str = "You are an expert pair programmer for PowerRustCOBOL...";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmConfig {
    #[serde(default)]
    pub endpoint: String,
    #[serde(default)]
    pub api_key: String,
    #[serde(default)]
    pub model: String,
    #[serde(default = "default_system_prompt")]
    pub system_prompt: String,
    #[serde(default = "default_temperature")]
    pub temperature: f32,
    #[serde(default = "default_max_tokens")]
    pub max_tokens: u32,
    #[serde(default = "default_timeout_secs")]
    pub timeout_secs: u32,
    #[serde(default)]
    pub provider: String,
    #[serde(default)]
    pub verbose_log: bool,
}

impl LlmConfig {
    pub fn load() -> Self {
        let path = base_dir().join("llm_config.json");
        if let Ok(data) = std::fs::read_to_string(&path) {
            if let Ok(cfg) = serde_json::from_str(&data) {
                return cfg;
            }
        }
        Self {
            endpoint: String::new(),
            api_key: String::new(),
            model: String::new(),
            system_prompt: DEFAULT_SYSTEM_PROMPT.to_string(),
            temperature: 0.7,
            max_tokens: 8192,
            timeout_secs: 30,
            provider: String::new(),
            verbose_log: false,
        }
    }
    
    pub fn is_configured(&self) -> bool {
        !self.provider.is_empty() && !self.model.is_empty()
    }
    pub fn save(&self) -> Result<(), String> {
        let path = base_dir().join("llm_config.json");
        let data = serde_json::to_string_pretty(self).map_err(|e| e.to_string())?;
        std::fs::write(&path, data).map_err(|e| e.to_string())?;
        Ok(())
    }
}

pub fn default_system_prompt() -> String { DEFAULT_SYSTEM_PROMPT.to_string() }
pub fn default_temperature() -> f32 { 0.7 }
pub fn default_max_tokens() -> u32 { 8192 }
pub fn default_timeout_secs() -> u32 { 30 }

pub enum LlmResponse {
    Ok(String),
    Chunk(String),
    Err(String),
}

#[derive(Clone, Serialize, Deserialize)]
pub struct ChatTurn {
    pub role: String,
    pub content: String,
}

impl ChatTurn {
    pub fn user(content: impl Into<String>) -> Self {
        Self { role: "user".into(), content: content.into() }
    }
    pub fn assistant(content: impl Into<String>) -> Self {
        Self { role: "assistant".into(), content: content.into() }
    }
}

pub enum AiLogKind {
    Info,
    Detail,
    Reasoning,
    Question,
    Error,
}

#[derive(Clone)]
pub struct Provider {
    pub id: &'static str,
    pub label: &'static str,
    pub default_endpoint: &'static str,
}

impl Provider {
    pub fn from_id(id: &str) -> Option<Self> {
        PROVIDERS.iter().find(|p| p.id == id).cloned()
    }
    pub fn id(&self) -> &str { self.id }
    pub fn label(&self) -> &str { self.label }
    pub fn default_endpoint(&self) -> &str { self.default_endpoint }
}

pub const PROVIDERS: &[Provider] = &[
    Provider {
        id: "openai",
        label: "OpenAI",
        default_endpoint: "https://api.openai.com/v1",
    },
    Provider {
        id: "anthropic",
        label: "Anthropic",
        default_endpoint: "https://api.anthropic.com/v1",
    },
    Provider {
        id: "cohere",
        label: "Cohere",
        default_endpoint: "https://api.cohere.ai/v1",
    },
    Provider {
        id: "gemini",
        label: "Google Gemini",
        default_endpoint: "https://generativelanguage.googleapis.com/v1beta",
    },
    Provider {
        id: "perplexity",
        label: "Perplexity",
        default_endpoint: "https://api.perplexity.ai",
    },
    Provider {
        id: "groq",
        label: "Groq",
        default_endpoint: "https://api.groq.com/openai/v1",
    },
    Provider {
        id: "mistral",
        label: "Mistral",
        default_endpoint: "https://api.mistral.ai/v1",
    },
    Provider {
        id: "openrouter",
        label: "OpenRouter",
        default_endpoint: "https://openrouter.ai/api/v1",
    },
    Provider {
        id: "huggingface",
        label: "HuggingFace",
        default_endpoint: "https://api-inference.huggingface.co/models",
    },
    Provider {
        id: "together",
        label: "Together AI",
        default_endpoint: "https://api.together.xyz/v1",
    },
    Provider {
        id: "deepseek",
        label: "DeepSeek",
        default_endpoint: "https://api.deepseek.com/v1",
    },
    Provider {
        id: "xai",
        label: "xAI",
        default_endpoint: "https://api.x.ai/v1",
    },
    Provider {
        id: "voyageai",
        label: "Voyage AI",
        default_endpoint: "https://api.voyageai.com/v1",
    },
    Provider {
        id: "ollama",
        label: "Ollama (Local)",
        default_endpoint: "http://localhost:11434/api",
    },
    Provider {
        id: "ollama_cloud",
        label: "Ollama (Cloud)",
        // ollama.com serves both the native API (/api) and an OpenAI-
        // compatible one (/v1). NOT api.ollama.com — that host is wrong.
        default_endpoint: "https://ollama.com/v1",
    },
    Provider {
        id: "llamafile",
        label: "Llamafile (Local)",
        default_endpoint: "http://localhost:8080/v1",
    },
];

pub struct DetectedApi {
    pub models: Vec<String>,
    pub endpoint: String,
    pub provider: String,
}

pub fn base_dir() -> PathBuf {
    let dir = dirs::data_dir().unwrap_or_else(|| PathBuf::from(".")).join("cobolt");
    if !dir.exists() {
        let _ = std::fs::create_dir_all(&dir);
    }
    dir
}

use std::sync::{Mutex, LazyLock};

static AI_LOG_QUEUE: LazyLock<Mutex<Vec<AiLogEntry>>> = LazyLock::new(|| Mutex::new(Vec::new()));
static CONNECTION_LOG: LazyLock<Mutex<String>> = LazyLock::new(|| Mutex::new(String::new()));

pub fn load_history(_dir: &Path, key: &str) -> Vec<ChatTurn> {
    let path = base_dir().join(format!("{}.json", key));
    if let Ok(data) = std::fs::read_to_string(&path) {
        if let Ok(history) = serde_json::from_str(&data) {
            return history;
        }
    }
    vec![]
}
pub fn save_history(_dir: &Path, key: &str, turns: &[ChatTurn]) {
    let path = base_dir().join(format!("{}.json", key));
    if let Ok(data) = serde_json::to_string_pretty(turns) {
        let _ = std::fs::write(&path, data);
    }
}

pub fn load_raw_preferred_indexed(_dir: &Path) -> HashSet<String> {
    let path = base_dir().join("preferred_indexed.json");
    if let Ok(data) = std::fs::read_to_string(&path) {
        if let Ok(rels) = serde_json::from_str(&data) {
            return rels;
        }
    }
    HashSet::new()
}
pub fn save_raw_preferred_indexed(_dir: &Path, rels: &HashSet<String>) {
    let path = base_dir().join("preferred_indexed.json");
    if let Ok(data) = serde_json::to_string_pretty(rels) {
        let _ = std::fs::write(&path, data);
    }
}

pub fn push_ai_log(kind: AiLogKind, text: impl Into<String>) {
    if let Ok(mut q) = AI_LOG_QUEUE.lock() {
        q.push(AiLogEntry { kind, text: text.into() });
    }
}

/// Run one mesh request on a worker thread, streaming progress into the
/// Agentic AI activity log and the connection log. Shared by the agent,
/// direct-editor, and compaction entry points.
fn run_mesh_request(req: cobolt_agents::MeshRequest, label: &'static str) -> Receiver<LlmResponse> {
    let (tx, rx) = mpsc::channel();
    push_ai_log(
        AiLogKind::Info,
        format!("{label}: \"{}\"", truncate_for_log(&req.user_prompt, 160)),
    );
    std::thread::spawn(move || {
        let rt = match tokio::runtime::Runtime::new() {
            Ok(rt) => rt,
            Err(e) => {
                let _ = tx.send(LlmResponse::Err(format!("Failed to start async runtime: {e}")));
                return;
            }
        };
        rt.block_on(async {
            let orch = Orchestrator::new();
            // Every orchestrator step lands in the activity log as it happens.
            let on_log = |line: String| push_ai_log(AiLogKind::Detail, format!("agent · {line}"));
            let tx_clone = tx.clone();
            let on_chunk = move |chunk: &str| {
                let _ = tx_clone.send(LlmResponse::Chunk(chunk.to_string()));
            };
            match orch.handle_request(&req, &on_log, &on_chunk).await {
                Ok((resp, trace)) => {
                    push_connection_log(&trace);
                    push_ai_log(
                        AiLogKind::Reasoning,
                        format!(
                            "reply: {} chars — {}",
                            resp.len(),
                            truncate_for_log(&resp, 200)
                        ),
                    );
                    let _ = tx.send(LlmResponse::Ok(resp));
                }
                Err(e) => {
                    push_connection_log(&format!("=== ERROR ===\n{e}\n"));
                    push_ai_log(AiLogKind::Error, e.clone());
                    let _ = tx.send(LlmResponse::Err(e));
                }
            }
        });
    });
    rx
}

fn truncate_for_log(s: &str, max: usize) -> String {
    let s = s.trim().replace('\n', " ");
    if s.chars().count() <= max {
        s
    } else {
        let cut: String = s.chars().take(max).collect();
        format!("{cut}…")
    }
}

fn mesh_request_base(cfg: &LlmConfig) -> cobolt_agents::MeshRequest {
    cobolt_agents::MeshRequest {
        provider: cfg.provider.clone(),
        model: cfg.model.clone(),
        api_key: cfg.api_key.clone(),
        endpoint: cfg.endpoint.clone(),
        specialist: None,
        system_prompt: String::new(),
        skills: String::new(),
        context: String::new(),
        history: Vec::new(),
        user_prompt: String::new(),
        temperature: cfg.temperature,
        max_tokens: cfg.max_tokens,
        verbose: cfg.verbose_log,
    }
}

pub fn spawn_agent_request(
    cfg: &LlmConfig,
    sys: &str,
    skills: &str,
    history: &[ChatTurn],
    sent: &str,
    context: &str,
    specialist: Option<String>,
) -> Receiver<LlmResponse> {
    let mut req = mesh_request_base(cfg);
    req.specialist = specialist;
    // The composed prompt/skills/context ARE the dev-agent contract (spec 025
    // R14/R21/R2) — they must reach the model on every request.
    req.system_prompt = sys.to_string();
    req.skills = skills.to_string();
    req.context = context.to_string();
    let mut h: Vec<_> = history.iter().map(|t| (t.role.clone(), t.content.clone())).collect();
    if let Some(last) = h.last() {
        if last.0 == "user" && last.1 == sent {
            h.pop();
        }
    }
    req.history = h;
    req.user_prompt = sent.to_string();
    run_mesh_request(req, "agent request")
}

pub fn spawn_request(
    cfg: &LlmConfig,
    history: &[ChatTurn],
    prompt: &str,
    code: &str,
    file: &str,
    skills: &str,
    specialist: Option<String>,
) -> Receiver<LlmResponse> {
    let mut req = mesh_request_base(cfg);
    req.specialist = specialist;
    req.system_prompt = cfg.system_prompt.clone();
    req.skills = skills.to_string();
    // The editor request acts on the current file: include it as context so
    // the model sees the code it is asked to work on.
    if !code.trim().is_empty() {
        req.context = format!("Current file `{file}`:\n```cobol\n{code}\n```");
    }
    
    let mut h: Vec<_> = history.iter().map(|t| (t.role.clone(), t.content.clone())).collect();
    if let Some(last) = h.last() {
        if last.0 == "user" && last.1 == prompt {
            h.pop();
        }
    }
    req.history = h;
    req.user_prompt = prompt.to_string();
    run_mesh_request(req, "editor request")
}

pub fn spawn_compaction(cfg: &LlmConfig, history: &[ChatTurn]) -> Receiver<LlmResponse> {
    let mut req = mesh_request_base(cfg);
    req.system_prompt =
        "You summarize a development-assistant conversation, preserving decisions, \
         names, and unresolved items."
            .to_string();
    req.history = history
        .iter()
        .map(|t| (t.role.clone(), t.content.clone()))
        .collect();
    req.user_prompt = "Summarize the preceding chat history concisely.".to_string();
    run_mesh_request(req, "history compaction")
}

/// Heal endpoints saved with the previously shipped wrong Ollama Cloud host
/// (api.ollama.com → ollama.com). Applied on every outbound use so existing
/// configs keep working without the user re-picking the provider.
fn heal_endpoint(ep: &str) -> String {
    ep.trim().replace("api.ollama.com", "ollama.com")
}

pub fn spawn_test(cfg: &LlmConfig) -> Receiver<LlmResponse> {
    let (tx, rx) = mpsc::channel();
    let ep = heal_endpoint(&cfg.endpoint);
    let key = cfg.api_key.clone();
    let pid = cfg.provider.clone();
    let verbose = cfg.verbose_log;

    std::thread::spawn(move || {
        let rt = match tokio::runtime::Runtime::new() {
            Ok(rt) => rt,
            Err(e) => {
                let _ = tx.send(LlmResponse::Err(format!("Failed to start async runtime: {}", e)));
                return;
            }
        };
        rt.block_on(async {
            let client = match reqwest::Client::builder().build() {
                Ok(c) => c,
                Err(e) => {
                    let _ = tx.send(LlmResponse::Err(format!("Failed to create HTTP client: {}", e)));
                    return;
                }
            };
            if pid == "ollama" {
                let url = format!("{}/api/tags", ep.trim_end_matches("/api").trim_end_matches("/v1").trim_end_matches('/'));
                
                if verbose {
                    let mut trace = String::new();
                    trace.push_str(&format!("=== API REQUEST (Test) ===\nEndpoint: {}\nMethod: GET\n\n", url));
                    match client.get(&url).send().await {
                        Ok(res) => {
                            let status = res.status();
                            let text = res.text().await.unwrap_or_default();
                            trace.push_str(&format!("=== API RESPONSE (Test) ===\nStatus: {}\nBody: {}\n\n", status, text));
                            push_connection_log(&trace);
                            if status.is_success() {
                                let _ = tx.send(LlmResponse::Ok("Connection successful! Ollama is reachable.".into()));
                                return;
                            }
                        }
                        Err(e) => {
                            trace.push_str(&format!("=== API ERROR (Test) ===\n{}\n\n", e));
                            push_connection_log(&trace);
                        }
                    }
                } else {
                    if let Ok(res) = client.get(&url).send().await {
                        if res.status().is_success() {
                            let _ = tx.send(LlmResponse::Ok("Connection successful! Ollama is reachable.".into()));
                            return;
                        }
                    }
                }
                let _ = tx.send(LlmResponse::Err("Failed to connect to Ollama endpoint.".into()));
            } else {
                let url = if ep.ends_with("/chat/completions") {
                    ep.replace("/chat/completions", "/models")
                } else if ep.ends_with("/v1") {
                    format!("{}/models", ep)
                } else if ep.ends_with('/') {
                    format!("{}v1/models", ep)
                } else {
                    format!("{}/v1/models", ep)
                };
                
                let mut req = client.get(&url);
                if !key.is_empty() {
                    req = req.header("Authorization", format!("Bearer {}", key));
                } else if pid == "gemini" {
                    req = req.header("x-goog-api-key", key.clone());
                }
                
                if verbose {
                    let mut trace = String::new();
                    trace.push_str(&format!("=== API REQUEST (Test) ===\nEndpoint: {}\nMethod: GET\n\n", url));
                    match req.try_clone().unwrap().build() {
                        Ok(built) => {
                            for (k, v) in built.headers() {
                                trace.push_str(&format!("{}: {:?}\n", k, v));
                            }
                        }
                        Err(_) => {}
                    }
                    trace.push_str("\n");
                    
                    match req.send().await {
                        Ok(res) => {
                            let status = res.status();
                            let text = res.text().await.unwrap_or_default();
                            trace.push_str(&format!("=== API RESPONSE (Test) ===\nStatus: {}\nBody: {}\n\n", status, text));
                            push_connection_log(&trace);
                            
                            if status.is_success() {
                                let _ = tx.send(LlmResponse::Ok("Connection successful! API key is valid.".into()));
                            } else {
                                let _ = tx.send(LlmResponse::Err(format!("API Error {}: {}", status, text)));
                            }
                        }
                        Err(e) => {
                            trace.push_str(&format!("=== API ERROR (Test) ===\n{}\n\n", e));
                            push_connection_log(&trace);
                            let _ = tx.send(LlmResponse::Err(format!("Network Error: {}", e)));
                        }
                    }
                } else {
                    match req.send().await {
                        Ok(res) => {
                            if res.status().is_success() {
                                let _ = tx.send(LlmResponse::Ok("Connection successful! API key is valid.".into()));
                            } else {
                                let status = res.status();
                                let text = res.text().await.unwrap_or_default();
                                let _ = tx.send(LlmResponse::Err(format!("API Error {}: {}", status, text)));
                            }
                        }
                        Err(e) => {
                            let _ = tx.send(LlmResponse::Err(format!("Network Error: {}", e)));
                        }
                    }
                }
            }
        });
    });
    rx
}

pub fn spawn_detect(endpoint: &str) -> Receiver<Result<DetectedApi, String>> {
    let (tx, rx) = mpsc::channel();
    let _ep = endpoint.to_string();
    std::thread::spawn(move || {
        let _ = tx.send(Err("Auto-detect not implemented. Please manually select a provider.".into()));
    });
    rx
}

pub fn spawn_list_models(provider: Provider, endpoint: &str, key: &str) -> Receiver<Result<Vec<String>, String>> {
    let (tx, rx) = mpsc::channel();
    let ep = heal_endpoint(endpoint);
    let key = key.to_string();
    let pid = provider.id().to_string();
    
    std::thread::spawn(move || {
        let rt = match tokio::runtime::Runtime::new() {
            Ok(rt) => rt,
            Err(e) => {
                let _ = tx.send(Err(format!("Failed to start async runtime: {}", e)));
                return;
            }
        };
        rt.block_on(async {
            let client = match reqwest::Client::builder().build() {
                Ok(c) => c,
                Err(e) => {
                    let _ = tx.send(Err(format!("Failed to create HTTP client: {}", e)));
                    return;
                }
            };
            if pid == "ollama" {
                let url = format!("{}/api/tags", ep.trim_end_matches("/api").trim_end_matches("/v1").trim_end_matches('/'));
                if let Ok(res) = client.get(&url).send().await {
                    if let Ok(json) = res.json::<serde_json::Value>().await {
                        if let Some(models) = json.get("models").and_then(|m| m.as_array()) {
                            let mut names = Vec::new();
                            for m in models {
                                if let Some(name) = m.get("name").and_then(|n| n.as_str()) {
                                    names.push(name.to_string());
                                }
                            }
                            let _ = tx.send(Ok(names));
                            return;
                        }
                    }
                }
                let _ = tx.send(Err("Failed to fetch models from Ollama".into()));
            } else {
                let url = if ep.ends_with("/chat/completions") {
                    ep.replace("/chat/completions", "/models")
                } else if ep.ends_with("/v1") {
                    format!("{}/models", ep)
                } else if ep.ends_with('/') {
                    format!("{}v1/models", ep)
                } else {
                    format!("{}/v1/models", ep)
                };
                
                let mut req = client.get(&url);
                if !key.is_empty() {
                    req = req.header("Authorization", format!("Bearer {}", key));
                } else if pid == "gemini" {
                    // Google Gemini uses x-goog-api-key or key= in query
                    req = req.header("x-goog-api-key", key.clone());
                }
                
                if let Ok(res) = req.send().await {
                    if let Ok(json) = res.json::<serde_json::Value>().await {
                        if let Some(data) = json.get("data").and_then(|d| d.as_array()) {
                            let mut names = Vec::new();
                            for m in data {
                                if let Some(id) = m.get("id").and_then(|n| n.as_str()) {
                                    names.push(id.to_string());
                                }
                            }
                            let _ = tx.send(Ok(names));
                            return;
                        }
                        if let Some(models) = json.get("models").and_then(|d| d.as_array()) { // Gemini might use this
                             let mut names = Vec::new();
                             for m in models {
                                 if let Some(name) = m.get("name").and_then(|n| n.as_str()) {
                                     names.push(name.to_string().replace("models/", ""));
                                 }
                             }
                             let _ = tx.send(Ok(names));
                             return;
                        }
                    }
                }
                let _ = tx.send(Err("Failed to fetch models from API".into()));
            }
        });
    });
    rx
}

pub fn has_connection_log() -> bool {
    true
}
pub fn connection_log_text() -> String {
    CONNECTION_LOG.lock().map(|l| l.clone()).unwrap_or_default()
}
pub fn clear_connection_log() {
    if let Ok(mut l) = CONNECTION_LOG.lock() {
        l.clear();
    }
}
pub fn push_connection_log(text: &str) {
    if let Ok(mut l) = CONNECTION_LOG.lock() {
        l.push_str(text);
        l.push('\n');
    }
}

pub struct AiLogEntry {
    pub kind: AiLogKind,
    pub text: String,
}

pub fn drain_ai_log() -> Vec<AiLogEntry> {
    if let Ok(mut q) = AI_LOG_QUEUE.lock() {
        std::mem::take(&mut *q)
    } else {
        vec![]
    }
}

pub fn normalize_comments(code: &str) -> String { code.to_string() }
pub fn extract_code(reply: &str) -> Option<String> {
    let lower = reply.to_lowercase();
    if let Some(start) = lower.find("```cobol") {
        if let Some(end) = reply[start + 8..].find("```") {
            return Some(reply[start + 8..start + 8 + end].trim().to_string());
        }
    }
    if let Some(start) = reply.find("```") {
        if let Some(nl) = reply[start..].find('\n') {
            let body_start = start + nl + 1;
            if let Some(end) = reply[body_start..].find("```") {
                return Some(reply[body_start..body_start + end].trim().to_string());
            }
        }
    }
    None
}
pub fn ai_question(_reply: &str) {}

pub struct MeshSession {
    orchestrator: Orchestrator,
}

impl MeshSession {
    pub fn new() -> Self {
        Self {
            orchestrator: Orchestrator::new(),
        }
    }

    pub fn execute_request(&self, _request: &str) {}
}
