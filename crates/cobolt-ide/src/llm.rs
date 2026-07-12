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
        Self {
            endpoint: String::new(),
            api_key: String::new(),
            model: String::new(),
            system_prompt: DEFAULT_SYSTEM_PROMPT.to_string(),
            temperature: 0.7,
            max_tokens: 1024,
            timeout_secs: 30,
            provider: String::new(),
            verbose_log: false,
        }
    }
    
    pub fn is_configured(&self) -> bool { false }
    pub fn save(&self) -> Result<(), String> { Ok(()) }
}

pub fn default_system_prompt() -> String { DEFAULT_SYSTEM_PROMPT.to_string() }
pub fn default_temperature() -> f32 { 0.7 }
pub fn default_max_tokens() -> u32 { 1024 }
pub fn default_timeout_secs() -> u32 { 30 }

pub enum LlmResponse {
    Ok(String),
    Err(String),
}

#[derive(Clone)]
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
    pub id: String,
}

impl Provider {
    pub fn from_id(_id: &str) -> Option<Self> { None }
    pub fn id(&self) -> &str { &self.id }
    pub fn label(&self) -> &str { &self.id }
    pub fn default_endpoint(&self) -> &str { "" }
}

pub const PROVIDERS: &[Provider] = &[];

pub struct DetectedApi {
    pub models: Vec<String>,
    pub endpoint: String,
    pub provider: String,
}

pub fn base_dir() -> PathBuf {
    PathBuf::from(".gemini/antigravity/scratch") // dummy
}

pub fn load_history(_dir: &Path, _key: &str) -> Vec<ChatTurn> { vec![] }
pub fn save_history(_dir: &Path, _key: &str, _turns: &[ChatTurn]) {}

pub fn load_raw_preferred_indexed(_dir: &Path) -> HashSet<String> { HashSet::new() }
pub fn save_raw_preferred_indexed(_dir: &Path, _rels: &HashSet<String>) {}

pub fn spawn_agent_request(_cfg: &LlmConfig, _sys: &str, _skills: &str, _history: &[ChatTurn], _sent: &str, _context: &str) -> Receiver<LlmResponse> {
    let (_, rx) = mpsc::channel();
    rx
}

pub fn spawn_request(_cfg: &LlmConfig, _history: &[ChatTurn], _prompt: &str, _code: &str, _file: &str, _skills: &str) -> Receiver<LlmResponse> {
    let (_, rx) = mpsc::channel();
    rx
}

pub fn spawn_compaction(_cfg: &LlmConfig, _history: &[ChatTurn]) -> Receiver<LlmResponse> {
    let (_, rx) = mpsc::channel();
    rx
}

pub fn spawn_test(_cfg: &LlmConfig) -> Receiver<LlmResponse> {
    let (_, rx) = mpsc::channel();
    rx
}

pub fn spawn_detect(_endpoint: &str) -> Receiver<Result<DetectedApi, String>> {
    let (_, rx) = mpsc::channel();
    rx
}

pub fn spawn_list_models(_provider: Provider, _endpoint: &str, _key: &str) -> Receiver<Result<Vec<String>, String>> {
    let (_, rx) = mpsc::channel();
    rx
}

pub fn has_connection_log() -> bool { false }
pub fn connection_log_text() -> String { String::new() }
pub fn clear_connection_log() {}

pub struct AiLogEntry {
    pub kind: AiLogKind,
    pub text: String,
}

pub fn drain_ai_log() -> Vec<AiLogEntry> { vec![] }

pub fn normalize_comments(code: &str) -> String { code.to_string() }
pub fn extract_code(reply: &str) -> Option<String> { Some(reply.to_string()) }
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
