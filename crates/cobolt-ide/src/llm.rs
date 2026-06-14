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
}

fn default_system_prompt() -> String { DEFAULT_SYSTEM_PROMPT.to_string() }
fn default_temperature() -> f32 { 0.2 }
fn default_max_tokens() -> u32 { 4096 }

impl Default for LlmConfig {
    fn default() -> Self {
        Self {
            endpoint: String::new(),
            api_key: String::new(),
            model: String::new(),
            system_prompt: default_system_prompt(),
            temperature: default_temperature(),
            max_tokens: default_max_tokens(),
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
        Self { role: "user".into(), content: content.into() }
    }
    pub fn assistant(content: impl Into<String>) -> Self {
        Self { role: "assistant".into(), content: content.into() }
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

const KEY_LEN: usize = 200;
const PAYLOAD_LEN: usize = 128 * 1024;
const RECORD_LEN: usize = KEY_LEN + PAYLOAD_LEN;

/// Build (but do not open) the conversations indexed file for a `data/` dir.
fn build_store(data_dir: &Path) -> IndexedFile {
    let path = data_dir.join(CONVERSATIONS_FILE);
    let primary = KeySpec { offset: 0, len: KEY_LEN, duplicates: false };
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
    let Some(rec) = rec else { return Vec::new(); };
    if rec.len() <= KEY_LEN {
        return Vec::new();
    }
    let payload = &rec[KEY_LEN..];
    let end = payload.iter()
        .rposition(|&b| b != b' ' && b != 0)
        .map(|i| i + 1)
        .unwrap_or(0);
    let text = String::from_utf8_lossy(&payload[..end]);
    serde_json::from_str(&text).unwrap_or_default()
}

/// Persist the conversation for one source file, replacing any previous turns.
/// An empty `turns` deletes the record.
pub fn save_history(data_dir: &Path, key: &str, turns: &[ChatTurn]) {
    let Some(mut f) = open_io_create(data_dir) else { return; };
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
) -> Receiver<LlmResponse> {
    let (tx, rx) = mpsc::channel();

    let endpoint = cfg.endpoint.trim().to_string();
    let api_key = cfg.api_key.trim().to_string();
    let model = cfg.model.trim().to_string();
    let temperature = cfg.temperature;
    let max_tokens = cfg.max_tokens;

    // Build the message list: system prompt, prior turns, then the new request
    // (prompt + current code) as the final user message.
    let mut messages: Vec<serde_json::Value> = Vec::new();
    messages.push(serde_json::json!({
        "role": "system",
        "content": cfg.system_prompt,
    }));
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

    let body = serde_json::json!({
        "model": model,
        "messages": messages,
        "temperature": temperature,
        "max_tokens": max_tokens,
    });

    std::thread::spawn(move || {
        let agent = ureq::AgentBuilder::new()
            .timeout(Duration::from_secs(120))
            .build();
        let mut req = agent.post(&endpoint)
            .set("Content-Type", "application/json");
        if !api_key.is_empty() {
            req = req.set("Authorization", &format!("Bearer {api_key}"));
        }
        let result = match req.send_json(body) {
            Ok(resp) => match resp.into_json::<serde_json::Value>() {
                Ok(json) => match extract_reply(&json) {
                    Some(text) => LlmResponse::Ok(text),
                    None => LlmResponse::Err(
                        "The model response did not contain any message content.".into(),
                    ),
                },
                Err(e) => LlmResponse::Err(format!("Could not read the model response: {e}")),
            },
            Err(ureq::Error::Status(code, resp)) => {
                let detail = resp.into_string().unwrap_or_default();
                LlmResponse::Err(format!("Model returned HTTP {code}. {}", detail.trim()))
            }
            Err(e) => LlmResponse::Err(format!("Could not reach the model: {e}")),
        };
        let _ = tx.send(result);
    });

    rx
}

/// Fire a tiny request just to validate connectivity, authentication, and the
/// model name — used by the settings dialog's **Test connection** button.
pub fn spawn_test(cfg: &LlmConfig) -> Receiver<LlmResponse> {
    spawn_request(cfg, &[], "Reply with the single word: OK.", "", "connection-test")
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
fn base_dir() -> PathBuf {
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
            .duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
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
}
