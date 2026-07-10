// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Optional AI assistant for the COBOL code editor.
//!
//! The developer points PowerRustCOBOL at a cloud LLM (an OpenAI-compatible
//! *chat completions* endpoint) trained on the PowerRustCOBOL documentation.
//! Once configured, a prompt bar appears above the code editor; the model is
//! sent the standard system prompt, the per-file conversation history, the
//! current source, and the developer's request, and its reply updates the
//! editable buffer.
//!
//! ## Where things are stored
//!
//! * **Connection + system prompt** — a single *global* file in the user's
//!   config directory ([`config_path`]). Keeping it global (not in
//!   `cobolt.toml`) means the model is configured once per machine and the API
//!   key never lands in a project repository.
//! * **Conversation history** — per source file, in [`conversations_path`],
//!   keyed by the file's absolute path, so "the conversation for that
//!   particular code" survives restarts.
//!
//! The network call is blocking (`ureq`), so it runs on a worker thread and
//! reports back over an `mpsc` channel; the UI thread never blocks.

use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver};
use std::time::Duration;

use serde::{Deserialize, Serialize};

// ── Standard system prompt ──────────────────────────────────────────────────

/// The default "standard system prompt". The developer can replace it in the
/// settings dialog; it is what gets sent as the `system` role on every request.
pub const DEFAULT_SYSTEM_PROMPT: &str = "\
You are an expert pair programmer for PowerRustCOBOL, a modern COBOL-85 \
development environment with a RAD form designer. You help the developer write \
and modify COBOL source code.\n\
\n\
Rules:\n\
- Always reply with the COMPLETE updated COBOL source for the file, not a \
diff or a fragment.\n\
- Wrap the source in a single fenced code block tagged `cobol`.\n\
- Keep all COBOL identifiers and source text in English.\n\
- Preserve the developer's existing structure and comments unless they ask you \
to change them.\n\
- If the request is a question rather than an edit, answer briefly and, when \
appropriate, include the relevant COBOL in a `cobol` code block.";

// ── Configuration ───────────────────────────────────────────────────────────

/// Global AI-assistant configuration (connection details + system prompt).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmConfig {
    /// Full chat-completions endpoint URL (e.g.
    /// `https://api.openai.com/v1/chat/completions` or a private cloud URL).
    #[serde(default)]
    pub endpoint: String,
    /// Bearer token / API key. Sent as `Authorization: Bearer <key>` when set.
    #[serde(default)]
    pub api_key: String,
    /// Model identifier passed in the request body.
    #[serde(default)]
    pub model: String,
    /// The standard system prompt sent on every request.
    #[serde(default = "default_system_prompt")]
    pub system_prompt: String,
    /// Sampling temperature (0.0 … 2.0).
    #[serde(default = "default_temperature")]
    pub temperature: f32,
    /// Maximum tokens to generate in the reply.
    #[serde(default = "default_max_tokens")]
    pub max_tokens: u32,
    /// How long to wait for the model's reply, in seconds. Raise it for slow local
    /// models that take a while to load / generate.
    #[serde(default = "default_timeout_secs")]
    pub timeout_secs: u32,
    /// Selected provider id (see [`Provider::id`]); empty ⇒ none picked yet.
    #[serde(default)]
    pub provider: String,
    /// When true the activity log includes the verbose detail lines (model info,
    /// the full context sent to the model, connection timings). Info / reasoning /
    /// error lines are always logged regardless.
    #[serde(default)]
    pub verbose_log: bool,
}

fn default_system_prompt() -> String {
    DEFAULT_SYSTEM_PROMPT.to_string()
}
fn default_temperature() -> f32 {
    0.2
}
fn default_max_tokens() -> u32 {
    4096
}
fn default_timeout_secs() -> u32 {
    // Local models (Ollama, LM Studio) reply non-streamed: the whole completion
    // must be generated before any bytes arrive, and a cold model can spend a
    // minute+ just loading. Default generously so first-time local use doesn't
    // trip the read timeout; the user can lower it in Settings.
    300
}

impl Default for LlmConfig {
    fn default() -> Self {
        Self {
            endpoint: String::new(),
            api_key: String::new(),
            model: String::new(),
            system_prompt: default_system_prompt(),
            temperature: default_temperature(),
            max_tokens: default_max_tokens(),
            timeout_secs: default_timeout_secs(),
            provider: String::new(),
            verbose_log: false,
        }
    }
}

impl LlmConfig {
    /// The assistant is active only once an endpoint and a model are set. When
    /// this is `false` the editor hides the prompt bar entirely.
    pub fn is_configured(&self) -> bool {
        !self.endpoint.trim().is_empty() && !self.model.trim().is_empty()
    }

    /// Load the global config, falling back to defaults on any error.
    pub fn load() -> Self {
        let path = config_path();
        match std::fs::read_to_string(&path) {
            Ok(text) => toml::from_str(&text).unwrap_or_default(),
            Err(_) => Self::default(),
        }
    }

    /// Persist the global config to the user config directory.
    pub fn save(&self) -> std::io::Result<()> {
        let path = config_path();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let text = toml::to_string_pretty(self)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
        std::fs::write(&path, text)
    }
}

// ── Conversation history ────────────────────────────────────────────────────

/// One message in a conversation. `role` is `"user"` or `"assistant"`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatTurn {
    pub role: String,
    pub content: String,
}

impl ChatTurn {
    pub fn user(content: impl Into<String>) -> Self {
        Self {
            role: "user".into(),
            content: content.into(),
        }
    }
    pub fn assistant(content: impl Into<String>) -> Self {
        Self {
            role: "assistant".into(),
            content: content.into(),
        }
    }
}

// Conversations are persisted in PowerRustCOBOL's **own** indexed (ISAM) file
// format — the same engine COBOL programs use for `ORGANIZATION IS INDEXED` —
// living in the project's `data/` folder. (Dog-fooding our runtime.)
//
// Layout: one fixed-length record per source file.
//   bytes [0 .. KEY_LEN)            → relative source path (space-padded key)
//   bytes [KEY_LEN .. RECORD_LEN)   → JSON of the conversation, space-padded
use cobolt_runtime::indexed::{status, IndexedFile, KeySpec, OpenMode};

/// Indexed-file name inside the project's `data/` directory.
pub const CONVERSATIONS_FILE: &str = "conversations.dat";

/// Dedicated IDE-managed indexed file (in the project's `data/`) for general
/// per-project persistence of small state that must survive restarts.
/// Examples: which .cidx files have had their descriptor edited via the
/// embedded raw COBOL editor (once used, the property pane form is locked
/// and the editor remains the visible surface for that file's record layout,
/// exactly as agent conversation history uses the conversations store).
///
/// This follows the project rule that "persistence of any kind should always
/// go in indexed files managed by the IDE (just like the history of agent
/// conversations)" — dog-fooding the same `IndexedFile` runtime engine.
pub const IDE_STATE_FILE: &str = "ide_state.dat";

const KEY_LEN: usize = 200;
const PAYLOAD_LEN: usize = 128 * 1024;
const RECORD_LEN: usize = KEY_LEN + PAYLOAD_LEN;

/// Build (but do not open) the conversations indexed file for a `data/` dir.
fn build_store(data_dir: &Path) -> IndexedFile {
    let path = data_dir.join(CONVERSATIONS_FILE);
    let primary = KeySpec {
        offset: 0,
        len: KEY_LEN,
        duplicates: false,
    };
    let mut f = IndexedFile::new(path, RECORD_LEN, primary, Vec::new());
    f.set_strict_metadata(false);
    f.set_compressing(true);
    // The conversation store must survive CLOSE / reopen (the in-memory engine
    // is ephemeral by default since the WITH PERSISTENCE change).
    f.set_persist(true);
    f.set_key_names(vec![Some("SOURCE-PATH".to_string())]);
    f
}

/// Open the store for I-O, creating an empty file the first time.
fn open_io_create(data_dir: &Path) -> Option<IndexedFile> {
    let _ = std::fs::create_dir_all(data_dir);
    let mut f = build_store(data_dir);
    match f.open(OpenMode::Io) {
        status::OK => Some(f),
        status::FILE_NOT_FOUND => {
            let mut creator = build_store(data_dir);
            if creator.open(OpenMode::Output) != status::OK {
                return None;
            }
            creator.close();
            let mut reopened = build_store(data_dir);
            (reopened.open(OpenMode::Io) == status::OK).then_some(reopened)
        }
        _ => None,
    }
}

/// Space-pad (or truncate) a key string to the fixed key width.
fn key_bytes(key: &str) -> Vec<u8> {
    let mut k = vec![b' '; KEY_LEN];
    let src = key.as_bytes();
    let n = src.len().min(KEY_LEN);
    k[..n].copy_from_slice(&src[..n]);
    k
}

/// Assemble one fixed-length record from a key and a JSON payload.
fn make_record(key: &str, json: &str) -> Vec<u8> {
    let mut rec = vec![b' '; RECORD_LEN];
    let kb = key.as_bytes();
    let kn = kb.len().min(KEY_LEN);
    rec[..kn].copy_from_slice(&kb[..kn]);
    let pb = json.as_bytes();
    let pn = pb.len().min(PAYLOAD_LEN);
    rec[KEY_LEN..KEY_LEN + pn].copy_from_slice(&pb[..pn]);
    rec
}

/// Serialise the turns to JSON, dropping the oldest turns until they fit one
/// record. (A conversation grows unbounded otherwise; the recent turns matter
/// most for both the transcript and the model's context.)
fn fit_json(turns: &[ChatTurn]) -> String {
    let mut start = 0;
    loop {
        let json = serde_json::to_string(&turns[start..]).unwrap_or_default();
        if json.len() <= PAYLOAD_LEN || start + 1 >= turns.len() {
            return json;
        }
        start += 1;
    }
}

/// Load the saved conversation for one source file (empty if none / on error).
pub fn load_history(data_dir: &Path, key: &str) -> Vec<ChatTurn> {
    let mut f = build_store(data_dir);
    if f.open(OpenMode::Input) != status::OK {
        return Vec::new();
    }
    let (rec, st) = f.read_key(&key_bytes(key));
    f.close();
    if st != status::OK {
        return Vec::new();
    }
    let Some(rec) = rec else {
        return Vec::new();
    };
    if rec.len() <= KEY_LEN {
        return Vec::new();
    }
    let payload = &rec[KEY_LEN..];
    let end = payload
        .iter()
        .rposition(|&b| b != b' ' && b != 0)
        .map(|i| i + 1)
        .unwrap_or(0);
    let text = String::from_utf8_lossy(&payload[..end]);
    serde_json::from_str(&text).unwrap_or_default()
}

/// Persist the conversation for one source file, replacing any previous turns.
/// An empty `turns` deletes the record.
pub fn save_history(data_dir: &Path, key: &str, turns: &[ChatTurn]) {
    let Some(mut f) = open_io_create(data_dir) else {
        return;
    };
    let kb = key_bytes(key);

    if turns.is_empty() {
        let (existing, _) = f.read_key(&kb);
        if existing.is_some() {
            f.delete(Some(&kb));
        }
    } else {
        let json = fit_json(turns);
        let rec = make_record(key, &json);
        let (existing, _) = f.read_key(&kb);
        if existing.is_some() {
            f.rewrite(&rec, Some(&kb));
        } else {
            f.write(&rec);
        }
    }

    f.commit();
    f.close();
}

// ── General IDE-managed state persistence (dog-foods indexed runtime) ────────

fn build_ide_state_store(data_dir: &Path) -> IndexedFile {
    let path = data_dir.join(IDE_STATE_FILE);
    let primary = KeySpec {
        offset: 0,
        len: KEY_LEN,
        duplicates: false,
    };
    let mut f = IndexedFile::new(path, RECORD_LEN, primary, Vec::new());
    f.set_strict_metadata(false);
    f.set_compressing(true);
    f.set_persist(true);
    f.set_key_names(vec![Some("IDE-STATE-KEY".to_string())]);
    f
}

fn open_ide_state_create(data_dir: &Path) -> Option<IndexedFile> {
    let _ = std::fs::create_dir_all(data_dir);
    let mut f = build_ide_state_store(data_dir);
    match f.open(OpenMode::Io) {
        status::OK => Some(f),
        status::FILE_NOT_FOUND => {
            let mut creator = build_ide_state_store(data_dir);
            if creator.open(OpenMode::Output) != status::OK {
                return None;
            }
            creator.close();
            let mut reopened = build_ide_state_store(data_dir);
            (reopened.open(OpenMode::Io) == status::OK).then_some(reopened)
        }
        _ => None,
    }
}

/// Load the set of *relative* paths (to project root) of indexed files whose
/// record descriptor (file descriptor / data items layout) was edited via the
/// embedded raw COBOL editor. Once any such edit happens, the properties pane
/// form is no longer offered for that file's descriptor; the editor must
/// remain the visible/primary surface.
pub fn load_raw_preferred_indexed(data_dir: &Path) -> std::collections::HashSet<String> {
    let mut f = build_ide_state_store(data_dir);
    if f.open(OpenMode::Input) != status::OK {
        return std::collections::HashSet::new();
    }
    let kb = key_bytes("__RAW_PREFERRED_INDEXED");
    let (rec, st) = f.read_key(&kb);
    f.close();
    if st != status::OK || rec.is_none() {
        return std::collections::HashSet::new();
    }
    let rec = rec.unwrap();
    if rec.len() <= KEY_LEN {
        return std::collections::HashSet::new();
    }
    let payload = &rec[KEY_LEN..];
    let end = payload
        .iter()
        .rposition(|&b| b != b' ' && b != 0)
        .map(|i| i + 1)
        .unwrap_or(0);
    let text = String::from_utf8_lossy(&payload[..end]);
    serde_json::from_str(&text).unwrap_or_default()
}

/// Persist the set (as relative paths). Pass empty set to clear the record.
pub fn save_raw_preferred_indexed(data_dir: &Path, prefs: &std::collections::HashSet<String>) {
    let Some(mut f) = open_ide_state_create(data_dir) else {
        return;
    };
    let kb = key_bytes("__RAW_PREFERRED_INDEXED");

    if prefs.is_empty() {
        let (existing, _) = f.read_key(&kb);
        if existing.is_some() {
            f.delete(Some(&kb));
        }
    } else {
        let json =
            serde_json::to_string(&prefs.iter().cloned().collect::<Vec<_>>()).unwrap_or_default();
        let rec = make_record("__RAW_PREFERRED_INDEXED", &json);
        let (existing, _) = f.read_key(&kb);
        if existing.is_some() {
            f.rewrite(&rec, Some(&kb));
        } else {
            f.write(&rec);
        }
    }

    f.commit();
    f.close();
}

// ── Requests ────────────────────────────────────────────────────────────────

/// Result of a chat request, delivered over the channel from the worker thread.
pub enum LlmResponse {
    /// The assistant's raw reply text.
    Ok(String),
    /// A human-readable error (network, auth, malformed response, …).
    Err(String),
}

/// Spawn a worker thread that sends one chat request and returns the reply over
/// the receiver. The final user turn embeds the developer's prompt and the
/// current source so the model always sees the code it is editing.
pub fn spawn_request(
    cfg: &LlmConfig,
    history: &[ChatTurn],
    user_prompt: &str,
    code: &str,
    filename: &str,
    skills: &str,
) -> Receiver<LlmResponse> {
    // Build the message list: system prompt, the project's skills (RustCOBOL
    // reference material), prior turns, then the new request (prompt + current
    // code) as the final user message.
    let mut messages: Vec<serde_json::Value> = Vec::new();
    messages.push(serde_json::json!({
        "role": "system",
        "content": cfg.system_prompt,
    }));
    if !skills.trim().is_empty() {
        messages.push(serde_json::json!({
            "role": "system",
            "content": format!("Reference material (skills):\n\n{skills}"),
        }));
    }
    for turn in history {
        messages.push(serde_json::json!({
            "role": turn.role,
            "content": turn.content,
        }));
    }
    messages.push(serde_json::json!({
        "role": "user",
        "content": compose_user_message(user_prompt, code, filename),
    }));
    post_messages(cfg, messages)
}

/// POST a fully-composed OpenAI-style `messages` list on a worker thread and return
/// the reply over the channel. Shared by the code assistant and the dev agent — the
/// only difference is how `messages` is composed.
/// Result of probing a host for a known LLM API (spec 025).
#[derive(Debug, Clone)]
pub struct DetectedApi {
    /// Human-readable provider name (e.g. "Ollama", "OpenAI-compatible").
    pub provider: String,
    /// The full chat-completions endpoint to use.
    pub endpoint: String,
    /// Model identifiers the server advertises (may be empty).
    pub models: Vec<String>,
}

/// Reduce a URL to `scheme://host[:port]`, dropping any path/query — the base a
/// provider is probed against.
pub fn base_url(url: &str) -> String {
    let u = url.trim().trim_end_matches('/');
    match u.split_once("://") {
        Some((scheme, rest)) => {
            let host = rest.split('/').next().unwrap_or(rest);
            format!("{scheme}://{host}")
        }
        None => u.to_string(),
    }
}

/// Probe `url`'s host to auto-detect the LLM API and its models (spec 025). Tries
/// Ollama's native `/api/tags`, then the OpenAI-style `/v1/models`. Both resolve to
/// the same OpenAI-compatible chat endpoint (`/v1/chat/completions`), so detection
/// mainly identifies the provider and lists models. Runs on a worker thread via
/// [`spawn_detect`].
pub fn detect_api(url: &str) -> Result<DetectedApi, String> {
    let base = base_url(url);
    let agent = ureq::AgentBuilder::new()
        .timeout(Duration::from_secs(8))
        .build();
    let endpoint = format!("{base}/v1/chat/completions");

    // 1) Ollama native model list.
    if let Some(body) = logged_get(&agent, &format!("{base}/api/tags")) {
        if let Ok(json) = serde_json::from_str::<serde_json::Value>(&body) {
            let models = json
                .get("models")
                .and_then(|m| m.as_array())
                .map(|arr| {
                    arr.iter()
                        .filter_map(|m| m.get("name").and_then(|n| n.as_str()).map(String::from))
                        .collect()
                })
                .unwrap_or_default();
            return Ok(DetectedApi {
                provider: "Ollama".into(),
                endpoint,
                models,
            });
        }
    }
    // 2) OpenAI-style model list (LM Studio, OpenAI, vLLM, …).
    if let Some(body) = logged_get(&agent, &format!("{base}/v1/models")) {
        if let Ok(json) = serde_json::from_str::<serde_json::Value>(&body) {
            let models = json
                .get("data")
                .and_then(|d| d.as_array())
                .map(|arr| {
                    arr.iter()
                        .filter_map(|m| m.get("id").and_then(|n| n.as_str()).map(String::from))
                        .collect()
                })
                .unwrap_or_default();
            return Ok(DetectedApi {
                provider: "OpenAI-compatible".into(),
                endpoint,
                models,
            });
        }
    }
    Err(format!(
        "No known LLM API found at {base}. Tried Ollama (/api/tags) and OpenAI (/v1/models)."
    ))
}

/// GET `url`, record the exchange to the connection log, and return the body text
/// on a 2xx response.
fn logged_get(agent: &ureq::Agent, url: &str) -> Option<String> {
    let mut trace = HttpTrace {
        method: "GET".into(),
        url: url.to_string(),
        headers: Vec::new(),
        request_body: String::new(),
        outcome: String::new(),
        response_body: String::new(),
    };
    let out = match agent.get(url).call() {
        Ok(resp) => {
            trace.outcome = format!("{} {}", resp.status(), resp.status_text());
            let body = resp.into_string().unwrap_or_default();
            trace.response_body = body.clone();
            Some(body)
        }
        Err(ureq::Error::Status(code, resp)) => {
            trace.outcome = format!("HTTP {code}");
            trace.response_body = resp.into_string().unwrap_or_default();
            None
        }
        Err(e) => {
            trace.outcome = format!("connection error: {e}");
            None
        }
    };
    push_trace(trace);
    out
}

/// Probe for an LLM API on a worker thread; the result arrives over the channel.
pub fn spawn_detect(url: &str) -> Receiver<Result<DetectedApi, String>> {
    let (tx, rx) = mpsc::channel();
    let url = url.to_string();
    std::thread::spawn(move || {
        let _ = tx.send(detect_api(&url));
    });
    rx
}

// ── AI providers (spec: provider picker) ────────────────────────────────────
//
// A small registry of the cloud/local providers PowerRustCOBOL can drive. All of
// them are reached through the same OpenAI-style `/v1/chat/completions` transport
// (`Authorization: Bearer <key>`), except Amazon Bedrock which requires SigV4 and
// therefore only ships a curated fallback model list (no live listing / no direct
// Bearer chat). Selecting a provider fills in its default endpoint URL and the
// recommended system prompt, and (best-effort) fetches its current model list.

/// A supported AI provider.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Provider {
    OllamaLocal,
    OllamaCloud,
    Anthropic,
    OpenAI,
    Google,
    Amazon,
    Alibaba,
    Grok,
}

/// Every provider, in display order (used to build the picker).
pub const PROVIDERS: &[Provider] = &[
    Provider::OllamaLocal,
    Provider::OllamaCloud,
    Provider::Anthropic,
    Provider::OpenAI,
    Provider::Google,
    Provider::Amazon,
    Provider::Alibaba,
    Provider::Grok,
];

impl Provider {
    /// Stable identifier persisted in the config (not shown to the user).
    pub fn id(self) -> &'static str {
        match self {
            Provider::OllamaLocal => "ollama-local",
            Provider::OllamaCloud => "ollama-cloud",
            Provider::Anthropic => "anthropic",
            Provider::OpenAI => "openai",
            Provider::Google => "google",
            Provider::Amazon => "amazon",
            Provider::Alibaba => "alibaba",
            Provider::Grok => "grok",
        }
    }

    /// Resolve a persisted id back to a provider.
    pub fn from_id(s: &str) -> Option<Provider> {
        PROVIDERS.iter().copied().find(|p| p.id() == s)
    }

    /// Human-readable name shown in the picker (a brand — not translated).
    pub fn label(self) -> &'static str {
        match self {
            Provider::OllamaLocal => "Ollama (Local)",
            Provider::OllamaCloud => "Ollama (Cloud)",
            Provider::Anthropic => "Anthropic",
            Provider::OpenAI => "OpenAI",
            Provider::Google => "Google",
            Provider::Amazon => "Amazon",
            Provider::Alibaba => "Alibaba",
            Provider::Grok => "Grok",
        }
    }

    /// The default chat-completions endpoint filled in when the provider is picked.
    pub fn default_endpoint(self) -> &'static str {
        match self {
            Provider::OllamaLocal => "http://localhost:11434/v1/chat/completions",
            Provider::OllamaCloud => "https://ollama.com/v1/chat/completions",
            Provider::Anthropic => "https://api.anthropic.com/v1/chat/completions",
            Provider::OpenAI => "https://api.openai.com/v1/chat/completions",
            Provider::Google => {
                "https://generativelanguage.googleapis.com/v1beta/openai/chat/completions"
            }
            // Bedrock needs SigV4-signed requests; this base is informational only.
            Provider::Amazon => "https://bedrock-runtime.us-east-1.amazonaws.com",
            Provider::Alibaba => {
                "https://dashscope-intl.aliyuncs.com/compatible-mode/v1/chat/completions"
            }
            Provider::Grok => "https://api.x.ai/v1/chat/completions",
        }
    }

    /// Whether an API key is required to talk to the provider (local Ollama isn't).
    pub fn needs_key(self) -> bool {
        !matches!(self, Provider::OllamaLocal)
    }

    /// The recommended system prompt for this provider. They all perform the same
    /// COBOL authoring task, so the shared default prompt applies to each.
    pub fn default_prompt(self) -> &'static str {
        DEFAULT_SYSTEM_PROMPT
    }

    /// A curated, best-effort fallback model list used when a live fetch is not
    /// possible (no key yet, offline, or a provider without a listing endpoint).
    /// The live list from [`list_models`] is always preferred when available.
    pub fn default_models(self) -> &'static [&'static str] {
        match self {
            // Local Ollama has no fixed catalogue — it depends on what's pulled.
            Provider::OllamaLocal => &[],
            Provider::OllamaCloud => &[
                "gpt-oss:120b",
                "gpt-oss:20b",
                "deepseek-v3.1:671b",
                "qwen3-coder:480b",
            ],
            Provider::Anthropic => &[
                "claude-opus-4-8",
                "claude-sonnet-5",
                "claude-haiku-4-5-20251001",
                "claude-fable-5",
            ],
            Provider::OpenAI => &["gpt-4o", "gpt-4o-mini", "gpt-4.1", "o3", "o4-mini"],
            Provider::Google => &[
                "gemini-2.5-pro",
                "gemini-2.5-flash",
                "gemini-2.5-flash-lite",
            ],
            Provider::Amazon => &[
                "anthropic.claude-sonnet-4-20250514-v1:0",
                "amazon.nova-pro-v1:0",
                "amazon.nova-lite-v1:0",
            ],
            Provider::Alibaba => &["qwen-max", "qwen-plus", "qwen-turbo", "qwen3-coder-plus"],
            Provider::Grok => &["grok-4", "grok-3", "grok-3-mini", "grok-code-fast-1"],
        }
    }
}

/// How a model-listing endpoint is authenticated.
enum ListAuth<'a> {
    None,
    Bearer(&'a str),
    /// Anthropic's native `/v1/models` (`x-api-key` + `anthropic-version`).
    AnthropicKey(&'a str),
}

/// GET a JSON model-listing endpoint (auth-aware) and return the parsed body,
/// recording the exchange in the connection log. `None` on any non-2xx / error.
fn logged_get_auth(agent: &ureq::Agent, url: &str, auth: &ListAuth) -> Option<String> {
    let mut headers: Vec<(String, String)> = Vec::new();
    let mut req = agent.get(url);
    match auth {
        ListAuth::None => {}
        ListAuth::Bearer(key) if !key.is_empty() => {
            req = req.set("Authorization", &format!("Bearer {key}"));
            headers.push(("Authorization".into(), "Bearer ***".into()));
        }
        ListAuth::Bearer(_) => {}
        ListAuth::AnthropicKey(key) if !key.is_empty() => {
            req = req.set("x-api-key", key).set("anthropic-version", "2023-06-01");
            headers.push(("x-api-key".into(), "***".into()));
            headers.push(("anthropic-version".into(), "2023-06-01".into()));
        }
        ListAuth::AnthropicKey(_) => {}
    }
    let mut trace = HttpTrace {
        method: "GET".into(),
        url: url.to_string(),
        headers,
        request_body: String::new(),
        outcome: String::new(),
        response_body: String::new(),
    };
    let out = match req.call() {
        Ok(resp) => {
            trace.outcome = format!("{} {}", resp.status(), resp.status_text());
            let body = resp.into_string().unwrap_or_default();
            trace.response_body = body.clone();
            Some(body)
        }
        Err(ureq::Error::Status(code, resp)) => {
            trace.outcome = format!("HTTP {code}");
            trace.response_body = resp.into_string().unwrap_or_default();
            None
        }
        Err(e) => {
            trace.outcome = format!("connection error: {e}");
            None
        }
    };
    push_trace(trace);
    out
}

/// Pull model ids out of an OpenAI-style `{ "data": [ { "id": … } ] }` body.
fn parse_openai_models(body: &str) -> Vec<String> {
    serde_json::from_str::<serde_json::Value>(body)
        .ok()
        .and_then(|j| {
            j.get("data").and_then(|d| d.as_array()).map(|arr| {
                arr.iter()
                    .filter_map(|m| m.get("id").and_then(|n| n.as_str()).map(String::from))
                    .collect()
            })
        })
        .unwrap_or_default()
}

/// Pull model names out of an Ollama `{ "models": [ { "name": … } ] }` body.
fn parse_ollama_models(body: &str) -> Vec<String> {
    serde_json::from_str::<serde_json::Value>(body)
        .ok()
        .and_then(|j| {
            j.get("models").and_then(|d| d.as_array()).map(|arr| {
                arr.iter()
                    .filter_map(|m| m.get("name").and_then(|n| n.as_str()).map(String::from))
                    .collect()
            })
        })
        .unwrap_or_default()
}

/// Fetch the current model list for `provider`, using `endpoint` (for the Ollama
/// hosts) and `api_key` where auth is required. Falls back to the provider's
/// curated [`Provider::default_models`] when a live fetch yields nothing, so the
/// picker is never empty for a configured provider. Returns `Err` only when even
/// the fallback is empty (e.g. local Ollama with no server / no pulled models).
pub fn list_models(provider: Provider, endpoint: &str, api_key: &str) -> Result<Vec<String>, String> {
    let agent = ureq::AgentBuilder::new()
        .timeout(Duration::from_secs(10))
        .build();
    let key = api_key.trim();

    let live: Vec<String> = match provider {
        Provider::OllamaLocal | Provider::OllamaCloud => {
            let base = base_url(endpoint);
            let auth = if key.is_empty() {
                ListAuth::None
            } else {
                ListAuth::Bearer(key)
            };
            logged_get_auth(&agent, &format!("{base}/api/tags"), &auth)
                .map(|b| parse_ollama_models(&b))
                .unwrap_or_default()
        }
        Provider::Anthropic => logged_get_auth(
            &agent,
            "https://api.anthropic.com/v1/models",
            &ListAuth::AnthropicKey(key),
        )
        .map(|b| parse_openai_models(&b))
        .unwrap_or_default(),
        Provider::OpenAI => {
            logged_get_auth(&agent, "https://api.openai.com/v1/models", &ListAuth::Bearer(key))
                .map(|b| parse_openai_models(&b))
                .unwrap_or_default()
        }
        Provider::Google => logged_get_auth(
            &agent,
            "https://generativelanguage.googleapis.com/v1beta/openai/models",
            &ListAuth::Bearer(key),
        )
        .map(|b| parse_openai_models(&b))
        .unwrap_or_default(),
        Provider::Alibaba => logged_get_auth(
            &agent,
            "https://dashscope-intl.aliyuncs.com/compatible-mode/v1/models",
            &ListAuth::Bearer(key),
        )
        .map(|b| parse_openai_models(&b))
        .unwrap_or_default(),
        Provider::Grok => {
            logged_get_auth(&agent, "https://api.x.ai/v1/models", &ListAuth::Bearer(key))
                .map(|b| parse_openai_models(&b))
                .unwrap_or_default()
        }
        // Bedrock listing needs SigV4 — use the curated list only.
        Provider::Amazon => Vec::new(),
    };

    let mut models = if live.is_empty() {
        provider
            .default_models()
            .iter()
            .map(|s| s.to_string())
            .collect::<Vec<_>>()
    } else {
        live
    };
    models.sort();
    models.dedup();

    if models.is_empty() {
        Err(format!(
            "No models found for {}. Check the API key / endpoint, or that the server is running.",
            provider.label()
        ))
    } else {
        Ok(models)
    }
}

/// Fetch a provider's model list on a worker thread; the result arrives over the
/// channel so the UI never blocks.
pub fn spawn_list_models(
    provider: Provider,
    endpoint: &str,
    api_key: &str,
) -> Receiver<Result<Vec<String>, String>> {
    let (tx, rx) = mpsc::channel();
    let endpoint = endpoint.to_string();
    let api_key = api_key.to_string();
    std::thread::spawn(move || {
        let _ = tx.send(list_models(provider, &endpoint, &api_key));
    });
    rx
}

/// Normalise a chat endpoint URL. A **bare host** (no path, e.g. a local Ollama /
/// LM Studio server at `http://localhost:11434`) gets the OpenAI-compatible chat
/// path appended, since POSTing to the base returns HTTP 405. An endpoint that
/// already carries a path is left untouched.
fn normalize_endpoint(raw: &str) -> String {
    let e = raw.trim().trim_end_matches('/');
    if let Some((_, rest)) = e.split_once("://") {
        if !rest.contains('/') {
            // scheme://host[:port] with no path → append the standard chat path.
            return format!("{e}/v1/chat/completions");
        }
    }
    e.to_string()
}

fn post_messages(cfg: &LlmConfig, messages: Vec<serde_json::Value>) -> Receiver<LlmResponse> {
    let (tx, rx) = mpsc::channel();
    let endpoint = normalize_endpoint(&cfg.endpoint);
    let api_key = cfg.api_key.trim().to_string();
    let model = cfg.model.trim().to_string();
    let temperature = cfg.temperature;
    let max_tokens = cfg.max_tokens;
    let timeout = cfg.timeout_secs.max(1) as u64;
    let verbose = cfg.verbose_log;
    let msg_count = messages.len();
    // A one-line preview of the developer's request (the last user turn embeds the
    // prompt first, then the code context) for the activity log.
    let prompt_preview = messages
        .iter()
        .rev()
        .find(|m| m.get("role").and_then(|r| r.as_str()) == Some("user"))
        .and_then(|m| m.get("content").and_then(|c| c.as_str()))
        .map(|s| {
            let first = s.trim().lines().next().unwrap_or("").trim();
            let mut t: String = first.chars().take(140).collect();
            if first.chars().count() > 140 {
                t.push('…');
            }
            t
        })
        .unwrap_or_default();
    // Full context, captured before `messages` is moved into the body, so the
    // activity log can show the developer EXACTLY what was sent to the model
    // (system prompt, skills, replayed history, and the composed user turn with
    // its embedded source).
    let context_dump: Vec<(String, String)> = messages
        .iter()
        .map(|m| {
            let role = m
                .get("role")
                .and_then(|r| r.as_str())
                .unwrap_or("?")
                .to_string();
            let content = m
                .get("content")
                .and_then(|c| c.as_str())
                .unwrap_or("")
                .to_string();
            (role, content)
        })
        .collect();
    // Request a *streamed* (SSE) reply. The server then sends the status line and
    // tokens incrementally, so a slow local model succeeds as long as tokens keep
    // flowing — the read timeout below is per-read, not a deadline on the whole
    // (possibly minutes-long) generation.
    let body = serde_json::json!({
        "model": model,
        "messages": messages,
        "temperature": temperature,
        "max_tokens": max_tokens,
        "stream": true,
    });

    let req_body = serde_json::to_string_pretty(&body).unwrap_or_default();
    let mut headers = vec![
        ("Content-Type".to_string(), "application/json".to_string()),
        ("Accept".to_string(), "text/event-stream".to_string()),
    ];
    if !api_key.is_empty() {
        // Never log the real key.
        headers.push(("Authorization".to_string(), "Bearer ***".to_string()));
    }

    std::thread::spawn(move || {
        use std::io::BufRead;

        // Verbose detail lines are gated on the setting; concise info / reasoning /
        // error lines are always logged.
        let vlog = |t: String| {
            if verbose {
                ai_detail(t);
            }
        };

        let started = std::time::Instant::now();
        ai_info(format!("▶ Sending request to {endpoint}"));
        if !prompt_preview.is_empty() {
            vlog(format!("   prompt: {prompt_preview}"));
        }
        vlog(format!(
            "   model={} · {} message(s) · timeout={}s · streaming",
            if model.is_empty() { "(unset)" } else { &model },
            msg_count,
            timeout
        ));

        // Full context dump — every message role + content, line by line (verbose).
        if verbose {
            ai_detail("── context sent to the model ──".to_string());
            for (i, (role, content)) in context_dump.iter().enumerate() {
                ai_detail(format!("  [{}] {}:", i + 1, role));
                if content.is_empty() {
                    ai_detail("      (empty)".to_string());
                } else {
                    for line in content.lines() {
                        ai_detail(format!("      {line}"));
                    }
                }
            }
            ai_detail("── end of context ──".to_string());
        }

        // Per-read timeout (not an overall deadline): each socket read must return
        // within `timeout`, but the total streamed response may take far longer.
        let agent = ureq::AgentBuilder::new()
            .timeout_connect(Duration::from_secs(15))
            .timeout_read(Duration::from_secs(timeout))
            .build();
        vlog("   connecting…".to_string());
        let mut req = agent
            .post(&endpoint)
            .set("Content-Type", "application/json")
            .set("Accept", "text/event-stream");
        if !api_key.is_empty() {
            req = req.set("Authorization", &format!("Bearer {api_key}"));
        }
        let mut trace = HttpTrace {
            method: "POST".into(),
            url: endpoint.clone(),
            headers,
            request_body: req_body,
            outcome: String::new(),
            response_body: String::new(),
        };

        let result = match req.send_json(body) {
            Ok(resp) => {
                trace.outcome = format!("{} {}", resp.status(), resp.status_text());
                vlog(format!(
                    "◀ {} {} · {:.1}s",
                    resp.status(),
                    resp.status_text(),
                    started.elapsed().as_secs_f32()
                ));
                ai_info("⏳ Streaming response…".to_string());

                // Accumulators. `content_acc` is the assembled reply; reasoning is
                // flushed to the IDE log line-by-line as it streams in.
                let mut content_acc = String::new();
                let mut reasoning_buf = String::new();
                let mut reasoning_header_sent = false;
                let mut saw_sse = false;
                let mut first_token = true;
                let mut plain = String::new(); // fallback: server ignored `stream`

                // Flush any complete (newline-terminated) reasoning lines to the log.
                let flush_reasoning = |buf: &mut String, header: &mut bool| {
                    while let Some(idx) = buf.find('\n') {
                        let line: String = buf.drain(..=idx).collect();
                        let line = line.trim_end_matches(['\n', '\r']).to_string();
                        if !*header {
                            push_reasoning_line("💭 model reasoning:".to_string());
                            *header = true;
                        }
                        push_reasoning_line(line);
                    }
                };

                let mut reader = std::io::BufReader::new(resp.into_reader());
                let mut line = String::new();
                loop {
                    line.clear();
                    match reader.read_line(&mut line) {
                        Ok(0) => break, // EOF
                        Ok(_) => {}
                        Err(e) => {
                            trace.outcome = format!("stream read error: {e}");
                            break;
                        }
                    }
                    let trimmed = line.trim_end_matches(['\n', '\r']);
                    if trimmed.is_empty() {
                        continue;
                    }
                    if let Some(payload) = trimmed.strip_prefix("data:") {
                        let payload = payload.trim();
                        if payload == "[DONE]" {
                            break;
                        }
                        saw_sse = true;
                        if trace.response_body.len() < 20_000 {
                            trace.response_body.push_str(payload);
                            trace.response_body.push('\n');
                        }
                        if let Ok(json) = serde_json::from_str::<serde_json::Value>(payload) {
                            let (c, r) = extract_stream_delta(&json);
                            if first_token && (c.is_some() || r.is_some()) {
                                first_token = false;
                                vlog(format!(
                                    "   first token after {:.1}s",
                                    started.elapsed().as_secs_f32()
                                ));
                            }
                            if let Some(c) = c {
                                content_acc.push_str(&c);
                            }
                            if let Some(r) = r {
                                reasoning_buf.push_str(&r);
                                flush_reasoning(&mut reasoning_buf, &mut reasoning_header_sent);
                            }
                        }
                    } else {
                        // Not SSE — the server returned a plain JSON body despite the
                        // stream request; collect it for a one-shot parse below.
                        plain.push_str(trimmed);
                    }
                }

                if saw_sse {
                    // Any inline <think>…</think> in the streamed content → reasoning.
                    let (clean, inline) = split_think_tags(&content_acc);
                    if let Some(r) = inline {
                        reasoning_buf.push_str(&r);
                    }
                    if !reasoning_buf.trim().is_empty() {
                        if !reasoning_header_sent {
                            push_reasoning_line("💭 model reasoning:".to_string());
                        }
                        for l in reasoning_buf.trim_end().lines() {
                            push_reasoning_line(l.to_string());
                        }
                    }
                    if clean.trim().is_empty() {
                        ai_error("Completed but the response had no content (see Details).");
                        LlmResponse::Err(
                            "The streamed model response contained no message content \
                             (see Details)."
                                .into(),
                        )
                    } else {
                        ai_info(format!(
                            "✔ Completed · {} chars · {:.1}s",
                            clean.chars().count(),
                            started.elapsed().as_secs_f32()
                        ));
                        LlmResponse::Ok(clean)
                    }
                } else {
                    // Fallback: parse the collected body as a single OpenAI response.
                    vlog("   server did not stream; parsing single response".to_string());
                    trace.response_body = plain.clone();
                    match serde_json::from_str::<serde_json::Value>(&plain) {
                        Ok(json) => match extract_reply(&json) {
                            Some(text) => {
                                let (clean, inline) = split_think_tags(&text);
                                if let Some(r) = extract_reasoning(&json).or(inline) {
                                    push_reasoning(r);
                                }
                                ai_info(format!(
                                    "✔ Completed · {} chars · {:.1}s",
                                    clean.chars().count(),
                                    started.elapsed().as_secs_f32()
                                ));
                                LlmResponse::Ok(clean)
                            }
                            None => {
                                ai_error("Response had no message content (see Details).");
                                LlmResponse::Err(
                                    "The model response contained no message content (see \
                                     Details)."
                                        .into(),
                                )
                            }
                        },
                        Err(e) => {
                            ai_error(format!("Response was not valid JSON: {e} (see Details)."));
                            LlmResponse::Err(format!(
                                "Could not parse the model response as JSON: {e} (see Details)."
                            ))
                        }
                    }
                }
            }
            Err(ureq::Error::Status(code, resp)) => {
                trace.outcome = format!("HTTP {code}");
                trace.response_body = resp.into_string().unwrap_or_default();
                ai_error(format!("✖ HTTP {code} from the model (see Details)."));
                if code == 405 {
                    LlmResponse::Err(
                        "HTTP 405 (Method Not Allowed): the endpoint URL points at the wrong \
                         path. Use the full chat-completions URL, e.g. \
                         http://localhost:11434/v1/chat/completions for a local Ollama server."
                            .into(),
                    )
                } else {
                    LlmResponse::Err(format!("Model returned HTTP {code} (see Details)."))
                }
            }
            Err(e) => {
                trace.outcome = format!("connection error: {e}");
                let es = e.to_string();
                if es.to_lowercase().contains("timed out")
                    || es.to_lowercase().contains("timeout")
                {
                    ai_error(format!(
                        "✖ No output within {timeout}s — raise the AI request timeout in Settings."
                    ));
                    LlmResponse::Err(format!(
                        "The model did not send any output within {timeout}s. Local models can \
                         be slow to load before the first token — raise the AI request timeout \
                         in Settings (AI section) and try again. (A quick connection test can \
                         still pass because its reply is tiny.) Underlying error: {e}"
                    ))
                } else {
                    ai_error(format!("✖ Could not reach the model: {e}"));
                    LlmResponse::Err(format!("Could not reach the model: {e}"))
                }
            }
        };
        push_trace(trace);
        let _ = tx.send(result);
    });

    rx
}

// ── Connection trace log (spec 025 debug view) ───────────────────────────────

/// One captured HTTP exchange (request + response) for the connection-log modal.
#[derive(Clone)]
pub struct HttpTrace {
    pub method: String,
    pub url: String,
    pub headers: Vec<(String, String)>,
    pub request_body: String,
    /// Status line, or an error description.
    pub outcome: String,
    pub response_body: String,
}

fn trace_log() -> &'static std::sync::Mutex<Vec<HttpTrace>> {
    static LOG: std::sync::OnceLock<std::sync::Mutex<Vec<HttpTrace>>> = std::sync::OnceLock::new();
    LOG.get_or_init(|| std::sync::Mutex::new(Vec::new()))
}

/// Record one exchange, capping the retained history so it can't grow unbounded.
pub fn push_trace(t: HttpTrace) {
    if let Ok(mut l) = trace_log().lock() {
        l.push(t);
        let n = l.len();
        if n > 40 {
            l.drain(0..n - 40);
        }
    }
}

/// Whether any exchange has been logged (drives the "Details" button visibility).
pub fn has_connection_log() -> bool {
    trace_log().lock().map(|l| !l.is_empty()).unwrap_or(false)
}

// ── AI activity log side-channel ─────────────────────────────────────────────
//
// Everything a request worker does (compose → send → connect → stream → finish /
// error), plus the model's reasoning, is produced on the worker thread but shown
// in the IDE's output/log pane on the UI thread. Rather than thread it through
// every `LlmResponse` call site, workers push typed lines here and the app drains
// them each frame (same pattern as the connection trace log above).

/// The category of an AI log line — drives its styling in the output pane.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AiLogKind {
    /// A lifecycle milestone (sending, streaming, completed).
    Info,
    /// A secondary detail (status line, model, timings).
    Detail,
    /// A line of the model's chain-of-thought.
    Reasoning,
    /// A failure.
    Error,
}

/// One line queued for the IDE activity log.
#[derive(Clone)]
pub struct AiLogLine {
    pub kind: AiLogKind,
    pub text: String,
}

fn ai_log() -> &'static std::sync::Mutex<Vec<AiLogLine>> {
    static LOG: std::sync::OnceLock<std::sync::Mutex<Vec<AiLogLine>>> = std::sync::OnceLock::new();
    LOG.get_or_init(|| std::sync::Mutex::new(Vec::new()))
}

/// Queue one typed activity line (bounded so it can't grow without limit if the
/// UI never drains it).
pub fn ai_log_push(kind: AiLogKind, text: impl Into<String>) {
    if let Ok(mut l) = ai_log().lock() {
        l.push(AiLogLine {
            kind,
            text: text.into(),
        });
        let n = l.len();
        if n > 1000 {
            l.drain(0..n - 1000);
        }
    }
}

/// Log a lifecycle milestone.
pub fn ai_info(text: impl Into<String>) {
    ai_log_push(AiLogKind::Info, text);
}
/// Log a secondary detail.
pub fn ai_detail(text: impl Into<String>) {
    ai_log_push(AiLogKind::Detail, text);
}
/// Log a failure.
pub fn ai_error(text: impl Into<String>) {
    ai_log_push(AiLogKind::Error, text);
}

/// Queue one pre-formatted line of model reasoning.
pub fn push_reasoning_line(line: String) {
    ai_log_push(AiLogKind::Reasoning, line);
}

/// Queue a whole reasoning block: a header line followed by one line per text
/// line. Used for non-streamed replies (the streamed path flushes line-by-line).
pub fn push_reasoning(block: String) {
    push_reasoning_line("💭 model reasoning:".to_string());
    for line in block.lines() {
        push_reasoning_line(line.to_string());
    }
}

/// Take all queued activity lines, clearing the queue.
pub fn drain_ai_log() -> Vec<AiLogLine> {
    ai_log()
        .lock()
        .map(|mut l| std::mem::take(&mut *l))
        .unwrap_or_default()
}

/// Clear the connection log.
pub fn clear_connection_log() {
    if let Ok(mut l) = trace_log().lock() {
        l.clear();
    }
}

/// The full connection history rendered as human-readable text (newest last).
pub fn connection_log_text() -> String {
    let Ok(l) = trace_log().lock() else {
        return String::new();
    };
    let mut out = String::new();
    for (i, t) in l.iter().enumerate() {
        if i > 0 {
            out.push_str("\n\n──────────────────────────────────────\n\n");
        }
        out.push_str(&format!("{} {}\n", t.method, t.url));
        for (k, v) in &t.headers {
            out.push_str(&format!("{k}: {v}\n"));
        }
        if !t.request_body.is_empty() {
            out.push_str(&format!("\nRequest body:\n{}\n", t.request_body));
        }
        out.push_str(&format!("\n→ {}\n", t.outcome));
        if !t.response_body.is_empty() {
            out.push_str(&format!("Response body:\n{}\n", t.response_body));
        }
    }
    out
}

/// Compose the message list for the **dev agent** (spec 025 R14/R21/R2): the
/// effective system prompt, the skills as reference system content, the replayed
/// conversation `history`, then the new request with the fresh CONTEXT appended.
/// The prompt, skills, and context are applied here and are **never** part of the
/// stored history (R16).
pub fn compose_agent_messages(
    system_prompt: &str,
    skills: &str,
    history: &[ChatTurn],
    user_prompt: &str,
    context: &str,
) -> Vec<serde_json::Value> {
    let mut messages = Vec::new();
    messages.push(serde_json::json!({ "role": "system", "content": system_prompt }));
    if !skills.trim().is_empty() {
        messages.push(serde_json::json!({
            "role": "system",
            "content": format!("Reference material (skills):\n\n{skills}"),
        }));
    }
    for turn in history {
        messages.push(serde_json::json!({ "role": turn.role, "content": turn.content }));
    }
    messages.push(serde_json::json!({
        "role": "user",
        "content": format!("{}\n\n{}", user_prompt.trim(), context),
    }));
    messages
}

/// Send one dev-agent request (spec 025), reusing the shared transport.
pub fn spawn_agent_request(
    cfg: &LlmConfig,
    system_prompt: &str,
    skills: &str,
    history: &[ChatTurn],
    user_prompt: &str,
    context: &str,
) -> Receiver<LlmResponse> {
    post_messages(
        cfg,
        compose_agent_messages(system_prompt, skills, history, user_prompt, context),
    )
}

/// Fire a tiny request just to validate connectivity, authentication, and the
/// model name — used by the settings dialog's **Test connection** button.
pub fn spawn_test(cfg: &LlmConfig) -> Receiver<LlmResponse> {
    // Send a single, unambiguous user turn — no system prompt or COBOL-source
    // wrapper (which reads as a contradictory instruction to emit code). We only
    // care that the endpoint, auth, and model name resolve to a valid reply.
    let messages = vec![serde_json::json!({
        "role": "user",
        "content": "This is a connectivity check. Reply with only the word OK.",
    })];
    post_messages(cfg, messages)
}

/// Combine the developer's prompt with the current source into one user turn.
fn compose_user_message(user_prompt: &str, code: &str, filename: &str) -> String {
    format!(
        "{prompt}\n\nCurrent COBOL source ({filename}):\n```cobol\n{code}\n```",
        prompt = user_prompt.trim(),
        filename = filename,
        code = code,
    )
}

/// Pull `choices[0].message.content` out of an OpenAI-style response.
fn extract_reply(json: &serde_json::Value) -> Option<String> {
    json.get("choices")?
        .get(0)?
        .get("message")?
        .get("content")?
        .as_str()
        .map(|s| s.to_string())
}

/// Pull the model's *reasoning* (chain-of-thought) out of a response, when the
/// provider exposes it. Different OpenAI-compatible backends name this field
/// differently: `reasoning_content` (DeepSeek / Ollama compat), `reasoning`
/// (xAI / some OpenAI proxies), or Ollama-native `thinking`. Returns the trimmed
/// text, or `None` when the model didn't emit separate reasoning.
fn extract_reasoning(json: &serde_json::Value) -> Option<String> {
    let msg = json.get("choices")?.get(0)?.get("message")?;
    for key in ["reasoning_content", "reasoning", "thinking"] {
        if let Some(s) = msg.get(key).and_then(|v| v.as_str()) {
            let t = s.trim();
            if !t.is_empty() {
                return Some(t.to_string());
            }
        }
    }
    None
}

/// Pull the incremental `(content, reasoning)` out of one streamed SSE chunk
/// (`choices[0].delta.*`). Either part may be absent in a given chunk.
fn extract_stream_delta(json: &serde_json::Value) -> (Option<String>, Option<String>) {
    let delta = json
        .get("choices")
        .and_then(|c| c.get(0))
        .and_then(|c| c.get("delta"));
    let content = delta
        .and_then(|d| d.get("content"))
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .map(String::from);
    let mut reasoning = None;
    if let Some(d) = delta {
        for key in ["reasoning_content", "reasoning", "thinking"] {
            if let Some(s) = d.get(key).and_then(|v| v.as_str()) {
                if !s.is_empty() {
                    reasoning = Some(s.to_string());
                    break;
                }
            }
        }
    }
    (content, reasoning)
}

/// Split an inline `<think>…</think>` block (Qwen / DeepSeek native style) out of
/// the reply content. Returns `(clean_content, reasoning)`; when no block is
/// present the content is returned unchanged with `None`.
fn split_think_tags(content: &str) -> (String, Option<String>) {
    if let (Some(start), Some(end)) = (content.find("<think>"), content.find("</think>")) {
        if end >= start {
            let reasoning = content[start + "<think>".len()..end].trim().to_string();
            let mut clean = String::new();
            clean.push_str(&content[..start]);
            clean.push_str(&content[end + "</think>".len()..]);
            let reasoning = if reasoning.is_empty() {
                None
            } else {
                Some(reasoning)
            };
            return (clean.trim().to_string(), reasoning);
        }
    }
    (content.to_string(), None)
}

// ── Reply parsing ───────────────────────────────────────────────────────────

/// Extract COBOL source from an assistant reply.
///
/// Prefers a fenced block tagged `cobol`/`cob`/`cbl`; otherwise the first fenced
/// block of any kind; otherwise `None` (the reply is treated as prose).
pub fn extract_code(reply: &str) -> Option<String> {
    let blocks = fenced_blocks(reply);
    if blocks.is_empty() {
        return None;
    }
    blocks
        .iter()
        .find(|(lang, _)| {
            let l = lang.to_ascii_lowercase();
            l == "cobol" || l == "cob" || l == "cbl"
        })
        .or_else(|| blocks.first())
        .map(|(_, body)| body.clone())
}

/// Return all ```fenced``` blocks as `(language_tag, body)` pairs.
fn fenced_blocks(text: &str) -> Vec<(String, String)> {
    let mut blocks = Vec::new();
    let mut lines = text.lines();
    while let Some(line) = lines.next() {
        let trimmed = line.trim_start();
        if let Some(rest) = trimmed.strip_prefix("```") {
            let lang = rest.trim().to_string();
            let mut body = String::new();
            for inner in lines.by_ref() {
                if inner.trim_start().starts_with("```") {
                    break;
                }
                body.push_str(inner);
                body.push('\n');
            }
            // Drop the trailing newline we appended after the last content line.
            if body.ends_with('\n') {
                body.pop();
            }
            blocks.push((lang, body));
        }
    }
    blocks
}

// ── Paths ───────────────────────────────────────────────────────────────────

/// Base configuration directory for PowerRustCOBOL (created on demand).
pub(crate) fn base_dir() -> PathBuf {
    if let Ok(xdg) = std::env::var("XDG_CONFIG_HOME") {
        if !xdg.is_empty() {
            return PathBuf::from(xdg).join("powerrustcobol");
        }
    }
    if let Ok(appdata) = std::env::var("APPDATA") {
        if !appdata.is_empty() {
            return PathBuf::from(appdata).join("PowerRustCOBOL");
        }
    }
    if let Ok(home) = std::env::var("HOME") {
        if !home.is_empty() {
            return PathBuf::from(home).join(".config").join("powerrustcobol");
        }
    }
    PathBuf::from(".powerrustcobol")
}

/// Path to the global AI configuration file.
pub fn config_path() -> PathBuf {
    base_dir().join("llm.toml")
}

// ── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unconfigured_by_default() {
        assert!(!LlmConfig::default().is_configured());
    }

    #[test]
    fn configured_needs_endpoint_and_model() {
        let mut c = LlmConfig::default();
        c.endpoint = "https://example/v1/chat/completions".into();
        assert!(!c.is_configured(), "endpoint alone is not enough");
        c.model = "my-model".into();
        assert!(c.is_configured());
    }

    #[test]
    fn extracts_cobol_block_in_preference() {
        let reply = "Here you go:\n\n```text\nnot this\n```\n\n```cobol\n\
                     IDENTIFICATION DIVISION.\n           PROGRAM-ID. T.\n```\nDone.";
        let code = extract_code(reply).expect("should find a block");
        assert!(code.contains("PROGRAM-ID. T."));
        assert!(!code.contains("not this"));
    }

    #[test]
    fn falls_back_to_first_block_then_none() {
        let only = "```\nplain block\n```";
        assert_eq!(extract_code(only).as_deref(), Some("plain block"));
        assert!(extract_code("just prose, no code").is_none());
    }

    #[test]
    fn conversation_round_trip_via_indexed_file() {
        // Persist + reload a conversation through PowerRustCOBOL's own ISAM file.
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("prc-conv-{nanos}"));
        let _ = std::fs::remove_dir_all(&dir);

        let key = "src/main.cbl";
        assert!(load_history(&dir, key).is_empty(), "fresh store is empty");

        let turns = vec![
            ChatTurn::user("add a loop"),
            ChatTurn::assistant("```cobol\n           DISPLAY \"X\".\n```"),
        ];
        save_history(&dir, key, &turns);

        let got = load_history(&dir, key);
        assert_eq!(got.len(), 2, "two turns persisted");
        assert_eq!(got[0].role, "user");
        assert_eq!(got[0].content, "add a loop");
        assert!(got[1].content.contains("DISPLAY"));

        // Distinct keys are independent.
        assert!(load_history(&dir, "src/other.cbl").is_empty());

        // Clearing deletes the record.
        save_history(&dir, key, &[]);
        assert!(load_history(&dir, key).is_empty(), "cleared store is empty");

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn user_message_embeds_code_and_prompt() {
        let m = compose_user_message("add a loop", "PROCEDURE DIVISION.", "main.cbl");
        assert!(m.contains("add a loop"));
        assert!(m.contains("main.cbl"));
        assert!(m.contains("PROCEDURE DIVISION."));
        assert!(m.contains("```cobol"));
    }

    #[test]
    fn base_url_strips_path_to_host() {
        assert_eq!(
            base_url("http://localhost:11434/v1/chat/completions"),
            "http://localhost:11434"
        );
        assert_eq!(base_url("http://localhost:11434"), "http://localhost:11434");
        assert_eq!(base_url("http://localhost:11434/api/chat"), "http://localhost:11434");
        assert_eq!(base_url("https://api.openai.com/v1/"), "https://api.openai.com");
    }

    #[test]
    fn normalize_endpoint_appends_chat_path_only_to_bare_host() {
        // Bare local host → append the OpenAI-compatible chat path (fixes 405).
        assert_eq!(
            normalize_endpoint("http://localhost:11434"),
            "http://localhost:11434/v1/chat/completions"
        );
        assert_eq!(
            normalize_endpoint("http://localhost:11434/"),
            "http://localhost:11434/v1/chat/completions"
        );
        // Already has a path → untouched.
        assert_eq!(
            normalize_endpoint("http://localhost:11434/v1/chat/completions"),
            "http://localhost:11434/v1/chat/completions"
        );
        assert_eq!(
            normalize_endpoint("https://api.openai.com/v1/chat/completions"),
            "https://api.openai.com/v1/chat/completions"
        );
        // A deliberate non-OpenAI path is respected.
        assert_eq!(
            normalize_endpoint("http://localhost:11434/api/chat"),
            "http://localhost:11434/api/chat"
        );
    }

    #[test]
    fn agent_messages_layer_prompt_skills_history_then_request_context() {
        let history = vec![ChatTurn::user("prev req"), ChatTurn::assistant("prev reply")];
        let m = compose_agent_messages(
            "SYSPROMPT",
            "SKILLTEXT",
            &history,
            "  add a button  ",
            "CONTEXT-BLOCK",
        );
        // system prompt first, verbatim.
        assert_eq!(m[0]["role"], "system");
        assert_eq!(m[0]["content"], "SYSPROMPT");
        // skills injected as system reference material.
        assert_eq!(m[1]["role"], "system");
        assert!(m[1]["content"].as_str().unwrap().contains("SKILLTEXT"));
        // history replayed in order.
        assert_eq!(m[2]["content"], "prev req");
        assert_eq!(m[3]["content"], "prev reply");
        // final user turn = trimmed prompt + fresh CONTEXT.
        let last = m.last().unwrap();
        assert_eq!(last["role"], "user");
        let c = last["content"].as_str().unwrap();
        assert!(c.contains("add a button") && c.contains("CONTEXT-BLOCK"));
        // The prompt/skills/context are NOT ChatTurns — memory would store only the
        // developer/agent turns (R16), which this composer never writes.
        assert_eq!(m.len(), 5);
    }
}
