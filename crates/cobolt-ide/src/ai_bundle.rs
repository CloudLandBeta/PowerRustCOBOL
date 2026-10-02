// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Export and import of the AI configuration — model providers, the
//! project's agents and the model leaderboard — as one JSON file, so a
//! developer sets a machine up once and hands the result to the next one.
//!
//! # No credential leaves the machine
//!
//! The file is meant to be shared, so it carries **no secret**:
//!
//! - API keys are never read into it — the provider records it is built from
//!   have no key field, and agents name a provider, never a key;
//! - an endpoint's `user:password@` part is dropped;
//! - an agent's `mcp.json` keeps its servers but loses every `env` and
//!   `headers` value and every field whose name says key, token, secret,
//!   password, authorization or credential;
//! - and, as the last word, [`export`] refuses to write a file in which any
//!   key this IDE holds appears verbatim — whatever route it took to get there.
//!
//! Nor does it carry anything about the person who exported it ([`Personal`]):
//! their home folder, login name, and the name and e-mail git knows them by
//! are replaced by neutral placeholders wherever they appear — in an agent's
//! prompt, an `mcp.json` path, anywhere — and a leaderboard row's last error
//! (a provider's own text, which can name an account or organisation) is
//! dropped. The export is refused if any of them survives.
//!
//! The import therefore ends by asking for the key of every provider the file
//! names; that is what makes the file safe to pass around.
//!
//! # Import merges
//!
//! Nothing is deleted. A provider's endpoint is replaced; an agent with the
//! same name is overwritten (the local id is kept, so companion links survive);
//! a leaderboard row is replaced only by a more recent test of the same model.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Component, Path};

use serde::{Deserialize, Serialize};

use crate::agents_db::{AgentDef, AgentsDb};
use crate::leaderboard::Leaderboard;
use crate::llm::{LlmConfig, ProviderConfig};

/// What the `format` field of every file says, so an import can refuse a
/// file that is something else.
pub const FORMAT: &str = "powerrustcobol-ai-config";
/// The layout version this build writes and reads.
pub const FILE_VERSION: u32 = 1;
/// Agent files larger than this are left out of the bundle (and named in the
/// export summary): a knowledge base is not configuration.
pub const MAX_AGENT_FILE_BYTES: u64 = 1024 * 1024;
/// A stored key shorter than this is too short to search for without false
/// alarms; no real provider key is.
const MIN_KEY_LEN_TO_SCAN: usize = 8;

/// The file.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AiBundle {
    pub format: String,
    pub version: u32,
    #[serde(default)]
    pub ide_version: String,
    #[serde(default)]
    pub exported_at_unix: i64,
    #[serde(default)]
    pub providers: Vec<ProviderConfig>,
    #[serde(default)]
    pub provider_models: BTreeMap<String, Vec<String>>,
    #[serde(default)]
    pub agents: Vec<AgentEntry>,
    #[serde(default)]
    pub leaderboard: Option<Leaderboard>,
}

/// One agent: its `agent.json` plus the text files of its folder, keyed by
/// their path inside `agentic_ai/<name>/` (forward slashes).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentEntry {
    pub agent: AgentDef,
    #[serde(default)]
    pub files: BTreeMap<String, String>,
}

/// What an export wrote.
#[derive(Debug, Default)]
pub struct ExportSummary {
    pub providers: usize,
    pub agents: usize,
    pub leaderboard_rows: usize,
    /// Agent files left out — too large, or not text.
    pub skipped_files: Vec<String>,
}

/// What an import changed, and the providers whose keys must be asked for.
#[derive(Debug, Default)]
pub struct ImportSummary {
    pub providers: usize,
    pub agents: usize,
    pub leaderboard_rows: usize,
    /// Agents not imported, with the reason.
    pub skipped_agents: Vec<String>,
    /// Every provider id the file names (providers and agents alike), in
    /// order, without duplicates.
    pub providers_named: Vec<String>,
}

/// Build the bundle from this machine's configuration and, when a project is
/// open, its agents.
pub fn build(
    llm: &LlmConfig,
    agents: Option<&AgentsDb>,
    board: &Leaderboard,
    now_unix: i64,
) -> (AiBundle, ExportSummary) {
    let mut summary = ExportSummary::default();
    let providers: Vec<ProviderConfig> = llm
        .provider_configs
        .iter()
        .map(|p| ProviderConfig {
            endpoint: strip_userinfo(&p.endpoint),
            ..p.clone()
        })
        .collect();
    let provider_models = llm
        .provider_models
        .iter()
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect();
    let mut entries = Vec::new();
    if let Some(db) = agents {
        for a in &db.agents {
            let mut agent = a.clone();
            agent.endpoint = strip_userinfo(&agent.endpoint);
            let files = agent_files(&db.agent_dir(&a.name), &a.name, &mut summary.skipped_files);
            entries.push(AgentEntry { agent, files });
        }
    }
    let mut board = board.clone();
    for e in &mut board.entries {
        e.endpoint = strip_userinfo(&e.endpoint);
        e.last_error = None;
    }
    summary.providers = providers.len();
    summary.agents = entries.len();
    summary.leaderboard_rows = board.entries.len();
    let bundle = AiBundle {
        format: FORMAT.into(),
        version: FILE_VERSION,
        ide_version: crate::version::VERSION.into(),
        exported_at_unix: now_unix,
        providers,
        provider_models,
        agents: entries,
        leaderboard: Some(board),
    };
    (bundle, summary)
}

/// What identifies the person exporting: each value with the placeholder
/// that replaces it.
#[derive(Debug, Clone, Default)]
pub struct Personal {
    needles: Vec<(String, &'static str)>,
}

impl Personal {
    /// From this machine: home folder, login name, and git's global
    /// `user.name` / `user.email`.
    pub fn from_environment() -> Self {
        let git = |key: &str| {
            std::process::Command::new("git")
                .args(["config", "--global", "--get", key])
                .output()
                .ok()
                .filter(|o| o.status.success())
                .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        };
        let home = dirs::home_dir().map(|h| h.to_string_lossy().to_string());
        let login = std::env::var("USER").or_else(|_| std::env::var("USERNAME")).ok();
        Self::new(home, login, git("user.name"), git("user.email"))
    }

    pub fn new(home: Option<String>, login: Option<String>, name: Option<String>, email: Option<String>) -> Self {
        let mut needles = Vec::new();
        // Longest first, so the home folder goes before the login inside it.
        for (v, placeholder, min) in [
            (home, "~", 2),
            (email, "<e-mail removed>", 3),
            (name, "<name removed>", 4),
            (login, "<user>", 4),
        ] {
            if let Some(v) = v.map(|v| v.trim().trim_end_matches(['/', '\\']).to_string()) {
                if v.chars().count() >= min {
                    needles.push((v, placeholder));
                }
            }
        }
        Self { needles }
    }

    /// `text` with every personal detail replaced; and how many were.
    pub(crate) fn scrub(&self, text: &str) -> (String, usize) {
        let mut out = text.to_string();
        let mut n = 0;
        for (v, placeholder) in &self.needles {
            // Both as written and as JSON escapes it (a Windows path).
            let escaped = serde_json::to_string(v).unwrap_or_default();
            let escaped = escaped.trim_matches('"').to_string();
            for form in [v.clone(), escaped] {
                let (next, k) = replace_word_ci(&out, &form, placeholder);
                out = next;
                n += k;
            }
        }
        (out, n)
    }

    /// Which kind of personal detail `text` still contains, if any.
    pub(crate) fn find(&self, text: &str) -> Option<&'static str> {
        self.needles
            .iter()
            .find(|(v, _)| replace_word_ci(text, v, "").1 > 0)
            .map(|(_, p)| *p)
    }
}

/// Replace `needle` case-insensitively where it is not part of a longer word,
/// so a login of "dev" never mangles "developer".
fn replace_word_ci(text: &str, needle: &str, with: &str) -> (String, usize) {
    if needle.is_empty() {
        return (text.to_string(), 0);
    }
    let word = |c: Option<char>| c.is_some_and(|c| c.is_alphanumeric() || c == '_');
    let lower = text.to_lowercase();
    let pat = needle.to_lowercase();
    if lower.len() != text.len() {
        // Lower-casing changed byte offsets (rare scripts): match exactly.
        let n = text.matches(needle).count();
        return (text.replace(needle, with), n);
    }
    let mut out = String::with_capacity(text.len());
    let mut i = 0;
    let mut n = 0;
    while let Some(off) = lower[i..].find(&pat) {
        let at = i + off;
        let end = at + pat.len();
        let before = text[..at].chars().next_back();
        let after = text[end..].chars().next();
        out.push_str(&text[i..at]);
        if word(before) && word(needle.chars().next()) || word(after) && word(needle.chars().next_back()) {
            out.push_str(&text[at..end]);
        } else {
            out.push_str(with);
            n += 1;
        }
        i = end;
    }
    out.push_str(&text[i..]);
    (out, n)
}

/// Why [`export`] wrote nothing. The IDE words it in the developer's language
/// with [`ExportError::message`] (spec 080 F10).
#[derive(Debug, Clone, PartialEq)]
pub enum ExportError {
    /// A key the configuration holds is still in the text: where it was
    /// found (a key slot, never the key).
    KeyFound(String),
    /// A personal detail could not be replaced: its placeholder.
    PersonalDetail(&'static str),
    /// The scrubbed text no longer reads back as a bundle: why.
    Invalid(String),
    /// Serialising or writing the file failed: the system's reason.
    Write(String),
}

impl ExportError {
    /// The message the IDE shows, in the language of `tr`.
    pub fn message(&self, tr: &crate::i18n::Tr) -> String {
        let (template, detail) = match self {
            ExportError::KeyFound(w) => (tr.ai_export_refused_key, w.as_str()),
            ExportError::PersonalDetail(p) => (tr.ai_export_refused_personal, *p),
            ExportError::Invalid(e) => (tr.ai_export_refused_invalid, e.as_str()),
            ExportError::Write(e) => (tr.ai_export_failed, e.as_str()),
        };
        template.replacen("{}", detail, 1)
    }
}

/// Serialise `bundle`, replace every personal detail, and write it to `path` —
/// unless a key `llm` holds, or a personal detail, is still in the text, in
/// which case nothing is written. Returns how many details were replaced.
pub fn export(bundle: &AiBundle, llm: &LlmConfig, personal: &Personal, path: &Path) -> Result<usize, ExportError> {
    let json = serde_json::to_string_pretty(bundle).map_err(|e| ExportError::Write(e.to_string()))?;
    let (json, removed) = personal.scrub(&json);
    if let Some(where_) = find_key(&json, llm) {
        return Err(ExportError::KeyFound(where_));
    }
    if let Some(kind) = personal.find(&json) {
        return Err(ExportError::PersonalDetail(kind));
    }
    serde_json::from_str::<AiBundle>(&json).map_err(|e| ExportError::Invalid(e.to_string()))?;
    std::fs::write(path, json).map_err(|e| ExportError::Write(e.to_string()))?;
    Ok(removed)
}

/// Read and check a bundle.
pub fn read(path: &Path) -> Result<AiBundle, String> {
    let text = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
    let bundle: AiBundle = serde_json::from_str(&text).map_err(|e| format!("Not a configuration file: {e}"))?;
    if bundle.format != FORMAT {
        return Err("Not a PowerRustCOBOL AI configuration file.".into());
    }
    if bundle.version > FILE_VERSION {
        return Err(format!(
            "This file was written by a newer PowerRustCOBOL (format {}); this build reads format {FILE_VERSION}.",
            bundle.version
        ));
    }
    Ok(bundle)
}

/// Merge `bundle` into this machine and, when given, the project's agents.
/// The caller saves `llm` and `board` afterwards; agents are written here.
pub fn apply(
    bundle: &AiBundle,
    llm: &mut LlmConfig,
    agents: Option<&mut AgentsDb>,
    board: &mut Leaderboard,
) -> ImportSummary {
    let mut s = ImportSummary::default();
    let named = |id: &str, s: &mut ImportSummary| {
        let id = id.trim();
        if !id.is_empty() && !s.providers_named.iter().any(|p| p == id) {
            s.providers_named.push(id.to_string());
        }
    };

    for p in &bundle.providers {
        if crate::llm::Provider::from_id(&p.provider).is_none() {
            continue;
        }
        let cfg = llm.ensure_provider_config(&p.provider);
        cfg.endpoint = p.endpoint.clone();
        cfg.endpoint_user_edited = p.endpoint_user_edited;
        s.providers += 1;
        named(&p.provider, &mut s);
    }
    for (provider, models) in &bundle.provider_models {
        llm.provider_models.insert(provider.clone(), models.clone());
    }

    if let Some(db) = agents {
        // The local id wins for an agent that already exists, so links to it
        // stay valid; an imported companion link is mapped the same way.
        let mut id_map: BTreeMap<String, String> = BTreeMap::new();
        let mut taken: BTreeSet<String> = db.agents.iter().map(|a| a.id.clone()).collect();
        for e in &bundle.agents {
            let id = match db.by_name(&e.agent.name) {
                Some(local) => local.id.clone(),
                None if taken.contains(&e.agent.id) => crate::agents_db::new_uuid(),
                None => e.agent.id.clone(),
            };
            taken.insert(id.clone());
            id_map.insert(e.agent.id.clone(), id);
        }
        for e in &bundle.agents {
            let name = e.agent.name.trim();
            if !crate::agents_db::valid_name(name) {
                s.skipped_agents.push(format!("{name}: not a valid agent name"));
                continue;
            }
            let mut def = e.agent.clone();
            def.id = id_map[&e.agent.id].clone();
            def.companion = def.companion.as_ref().and_then(|c| id_map.get(c).cloned());
            let dir = db.agent_dir(name);
            let mut failed = None;
            for (rel, text) in &e.files {
                if !safe_relative(rel) {
                    failed = Some(format!("{name}: unsafe file path {rel}"));
                    break;
                }
                let target = dir.join(rel);
                if let Some(parent) = target.parent() {
                    let _ = std::fs::create_dir_all(parent);
                }
                if let Err(err) = std::fs::write(&target, text) {
                    failed = Some(format!("{name}: {rel}: {err}"));
                    break;
                }
            }
            if let Some(why) = failed {
                s.skipped_agents.push(why);
                continue;
            }
            // The prompt now belongs to whoever exported it: no upgrade may
            // replace it as an untouched default.
            if e.files.contains_key(&format!("{name}_prompt.md")) {
                let _ = std::fs::remove_file(dir.join(".prompt-revision"));
            }
            if let Err(err) = db.save_agent(&def) {
                s.skipped_agents.push(format!("{name}: {err}"));
                continue;
            }
            match db.agents.iter_mut().find(|a| a.name.eq_ignore_ascii_case(name)) {
                Some(local) => *local = def.clone(),
                None => db.agents.push(def.clone()),
            }
            s.agents += 1;
            if !def.no_model {
                named(&def.provider, &mut s);
            }
        }
        db.sort_rail();
    }

    if let Some(theirs) = &bundle.leaderboard {
        for e in &theirs.entries {
            let same = |x: &crate::leaderboard::Entry| {
                x.provider.eq_ignore_ascii_case(&e.provider) && x.model.eq_ignore_ascii_case(&e.model)
            };
            match board.entries.iter_mut().find(|x| same(x)) {
                Some(mine) if mine.tested_at_unix >= e.tested_at_unix => continue,
                Some(mine) => *mine = e.clone(),
                None => board.entries.push(e.clone()),
            }
            s.leaderboard_rows += 1;
        }
        for r in &theirs.retired {
            let known = board.retired.iter().any(|x| {
                x.provider.eq_ignore_ascii_case(&r.provider) && x.model.eq_ignore_ascii_case(&r.model)
            });
            if !known {
                board.retired.push(r.clone());
            }
        }
    }
    s
}

/// The providers among `named` that take a key, with whether one is already
/// stored — the rows of the "enter API keys" step.
pub fn providers_needing_keys(named: &[String], llm: &LlmConfig) -> Vec<(String, bool)> {
    named
        .iter()
        .filter(|p| crate::llm::provider_requires_key(p))
        .map(|p| {
            let stored = llm
                .api_keys
                .get(&crate::llm::provider_key_slot(p))
                .is_some_and(|k| !k.trim().is_empty());
            (p.clone(), stored)
        })
        .collect()
}

/// The text files of an agent folder, minus what is not configuration.
fn agent_files(dir: &Path, name: &str, skipped: &mut Vec<String>) -> BTreeMap<String, String> {
    let mut out = BTreeMap::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        let Ok(rd) = std::fs::read_dir(&d) else { continue };
        for e in rd.flatten() {
            let p = e.path();
            if p.is_dir() {
                stack.push(p);
                continue;
            }
            let Ok(rel) = p.strip_prefix(dir) else { continue };
            let rel = rel.to_string_lossy().replace('\\', "/");
            if rel == "agent.json" || rel == ".prompt-revision" {
                continue;
            }
            if e.metadata().map(|m| m.len()).unwrap_or(0) > MAX_AGENT_FILE_BYTES {
                skipped.push(format!("{name}/{rel}"));
                continue;
            }
            let Ok(text) = std::fs::read_to_string(&p) else {
                skipped.push(format!("{name}/{rel}"));
                continue;
            };
            let text = if rel == "mcp.json" { redact_mcp(&text) } else { text };
            out.insert(rel, text);
        }
    }
    out
}

/// `mcp.json` without its credentials: server definitions stay, every `env`
/// and `headers` value and every secret-named field is emptied. A file that is
/// not JSON is replaced by an empty object rather than shipped unread.
pub fn redact_mcp(text: &str) -> String {
    fn secret_name(k: &str) -> bool {
        let k = k.to_ascii_lowercase();
        ["key", "token", "secret", "password", "authorization", "credential"]
            .iter()
            .any(|w| k.contains(w))
    }
    fn blank_all(v: &mut serde_json::Value) {
        if let serde_json::Value::Object(m) = v {
            for x in m.values_mut() {
                if x.is_string() || x.is_number() {
                    *x = serde_json::Value::String(String::new());
                } else {
                    blank_all(x);
                }
            }
        }
    }
    fn walk(v: &mut serde_json::Value) {
        match v {
            serde_json::Value::Object(m) => {
                for (k, x) in m.iter_mut() {
                    let lk = k.to_ascii_lowercase();
                    if lk == "env" || lk == "headers" {
                        blank_all(x);
                    } else if secret_name(k) && x.is_string() {
                        *x = serde_json::Value::String(String::new());
                    } else if let serde_json::Value::String(s) = x {
                        *s = strip_userinfo(s);
                    } else {
                        walk(x);
                    }
                }
            }
            serde_json::Value::Array(a) => a.iter_mut().for_each(walk),
            _ => {}
        }
    }
    match serde_json::from_str::<serde_json::Value>(text) {
        Ok(mut v) => {
            walk(&mut v);
            serde_json::to_string_pretty(&v).unwrap_or_else(|_| "{}".into()) + "\n"
        }
        Err(_) => "{}\n".into(),
    }
}

/// A URL without its `user:password@` part.
pub fn strip_userinfo(url: &str) -> String {
    let Some(scheme_end) = url.find("://") else {
        return url.to_string();
    };
    let rest = &url[scheme_end + 3..];
    let host_end = rest.find(['/', '?', '#']).unwrap_or(rest.len());
    match rest[..host_end].rfind('@') {
        Some(at) => format!("{}{}", &url[..scheme_end + 3], &rest[at + 1..]),
        None => url.to_string(),
    }
}

/// Which stored credential `text` contains, if any — named by its slot, never
/// by its value.
pub(crate) fn find_key(text: &str, llm: &LlmConfig) -> Option<String> {
    let current = std::iter::once(("the active model", llm.api_key.as_str()));
    llm.api_keys
        .iter()
        .map(|(slot, k)| (slot.as_str(), k.as_str()))
        .chain(current)
        .find(|(_, k)| k.trim().len() >= MIN_KEY_LEN_TO_SCAN && text.contains(k.trim()))
        .map(|(slot, _)| slot.to_string())
}

/// A path that stays inside the agent folder.
fn safe_relative(rel: &str) -> bool {
    let p = Path::new(rel);
    !rel.is_empty() && p.components().all(|c| matches!(c, Component::Normal(_)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::llm::provider_key_slot;

    fn project() -> tempfile::TempDir {
        tempfile::tempdir().unwrap()
    }

    fn machine() -> LlmConfig {
        let mut llm = LlmConfig::load_defaults_for_test();
        llm.ensure_provider_config("anthropic").endpoint = "https://api.anthropic.com/v1".into();
        llm.ensure_provider_config("openai").endpoint = "https://me:hunter2pass@proxy.example/v1".into();
        llm.provider_models.insert("anthropic".into(), vec!["claude-opus-5-5".into()]);
        llm.store_api_key(provider_key_slot("anthropic"), "sk-ant-SECRET-0123456789");
        llm.store_api_key(provider_key_slot("openai"), "sk-openai-SECRET-987654");
        llm
    }

    #[test]
    fn an_export_carries_no_key_and_an_import_asks_for_each() {
        let src = project();
        let llm = machine();
        let mut db = AgentsDb::load(src.path());
        let id = db.create("Reporter", "Write reports.\n").unwrap();
        let mut def = db.by_id(&id).unwrap().clone();
        def.provider = "groq".into();
        def.model = "llama".into();
        db.save_agent(&def).unwrap();
        let db = AgentsDb::load(src.path());
        let dir = src.path().join("agentic_ai/Reporter");
        std::fs::write(
            dir.join("mcp.json"),
            r#"{"mcpServers":{"gh":{"command":"gh-mcp","env":{"GITHUB_TOKEN":"ghp_live_ABCDEFGHIJ"},"url":"https://u:pw@host/x","apiKey":"abc123456789"}}}"#,
        )
        .unwrap();
        std::fs::write(dir.join("knowledge/notes.md"), "Domain notes.\n").unwrap();

        let mut board = Leaderboard::default();
        board.ensure_models(&[("anthropic".into(), "claude-opus-5-5".into(), String::new())]);
        let (bundle, summary) = build(&llm, Some(&db), &board, 1);
        let out = src.path().join("ai.json");
        export(&bundle, &llm, &Personal::default(), &out).unwrap();
        let text = std::fs::read_to_string(&out).unwrap();
        for secret in ["sk-ant-SECRET", "sk-openai-SECRET", "hunter2pass", "ghp_live", "abc123456789", "u:pw@"] {
            assert!(!text.contains(secret), "{secret} leaked into the export");
        }
        assert!(text.contains("gh-mcp") && text.contains("Domain notes."), "configuration kept");
        assert_eq!(summary.agents, db.agents.len());

        // Another machine, another project: nothing configured, no key.
        let dst = project();
        let mut other = LlmConfig::load_defaults_for_test();
        let mut other_db = AgentsDb::load(dst.path());
        let mut other_board = Leaderboard::default();
        let back = read(&out).unwrap();
        let s = apply(&back, &mut other, Some(&mut other_db), &mut other_board);
        assert_eq!(other.provider_endpoint("openai"), "https://proxy.example/v1");
        assert_eq!(other_db.load_prompt("Reporter"), "Write reports.\n");
        assert!(dst.path().join("agentic_ai/Reporter/knowledge/notes.md").exists());
        assert_eq!(AgentsDb::load(dst.path()).by_name("Reporter").unwrap().provider, "groq");
        assert_eq!(other_board.entries.len(), 1);
        assert!(s.skipped_agents.is_empty(), "{:?}", s.skipped_agents);
        let ask = providers_needing_keys(&s.providers_named, &other);
        let ids: Vec<&str> = ask.iter().map(|(p, _)| p.as_str()).collect();
        for p in ["anthropic", "openai", "groq"] {
            assert!(ids.contains(&p), "asks for the {p} key: {ids:?}");
        }
        assert!(ask.iter().all(|(_, stored)| !stored), "no key came across");
        println!(
            "AI config bundle: {} providers, {} agents, {} leaderboard rows exported; {} keys asked for on import: {}",
            summary.providers,
            summary.agents,
            summary.leaderboard_rows,
            ask.len(),
            ids.join(", ")
        );
    }

    #[test]
    fn an_export_holding_a_key_is_refused_and_writes_nothing() {
        let dir = project();
        let llm = machine();
        let mut bundle = build(&llm, None, &Leaderboard::default(), 1).0;
        bundle.provider_models.insert("x".into(), vec!["sk-ant-SECRET-0123456789".into()]);
        let out = dir.path().join("ai.json");
        let err = export(&bundle, &llm, &Personal::default(), &out).unwrap_err();
        assert!(matches!(&err, ExportError::KeyFound(w) if w.contains("providerkey::anthropic")), "{err:?}");
        let msg = err.message(&crate::i18n::Language::English.tr());
        assert!(msg.contains("providerkey::anthropic") && !msg.contains("SECRET"), "{msg}");
        assert!(!out.exists());
    }

    /// Spec 080 F10 — the refusal is worded in the IDE's language: in each of
    /// the six it names where the key was found, keeps no placeholder, never
    /// shows the key, and every language but English says it in its own words.
    #[test]
    fn an_export_refusal_is_worded_in_every_language() {
        use crate::i18n::Language;
        let errors = [
            ExportError::KeyFound("providerkey::anthropic".into()),
            ExportError::PersonalDetail("<e-mail removed>"),
            ExportError::Invalid("expected value at line 1".into()),
            ExportError::Write("permission denied".into()),
        ];
        let english: Vec<String> = errors.iter().map(|e| e.message(&Language::English.tr())).collect();
        for &lang in Language::ALL {
            for (e, en) in errors.iter().zip(&english) {
                let msg = e.message(&lang.tr());
                let detail = match e {
                    ExportError::KeyFound(w) | ExportError::Invalid(w) | ExportError::Write(w) => w.as_str(),
                    ExportError::PersonalDetail(p) => p,
                };
                assert!(msg.contains(detail) && !msg.contains("{}"), "{lang:?}: {msg}");
                if lang != Language::English {
                    assert_ne!(&msg, en, "{lang:?} shows the English text");
                }
            }
        }
        println!("{}", Language::ALL.iter().map(|l| errors[0].message(&l.tr())).collect::<Vec<_>>().join("\n"));
    }

    #[test]
    fn an_import_never_writes_outside_the_agent_folder_and_keeps_local_ids() {
        let dst = project();
        let mut db = AgentsDb::load(dst.path());
        let local = db.create("Helper", "local\n").unwrap();
        let mut theirs = db.by_id(&local).unwrap().clone();
        theirs.id = "their-id".into();
        let mut evil = theirs.clone();
        evil.name = "Evil".into();
        evil.id = "evil-id".into();
        let bundle = AiBundle {
            format: FORMAT.into(),
            version: FILE_VERSION,
            agents: vec![
                AgentEntry { agent: theirs, files: [("Helper_prompt.md".to_string(), "theirs\n".to_string())].into() },
                AgentEntry { agent: evil, files: [("../../escape.txt".to_string(), "x".to_string())].into() },
            ],
            ..Default::default()
        };
        let s = apply(&bundle, &mut LlmConfig::load_defaults_for_test(), Some(&mut db), &mut Leaderboard::default());
        assert_eq!(db.by_name("Helper").unwrap().id, local, "local id kept");
        assert_eq!(db.load_prompt("Helper"), "theirs\n");
        assert!(!dst.path().join("escape.txt").exists());
        assert_eq!(s.skipped_agents.len(), 1, "{:?}", s.skipped_agents);
    }

    #[test]
    fn nothing_about_the_exporter_survives_an_export() {
        let src = project();
        let llm = machine();
        let mut db = AgentsDb::load(src.path());
        db.create(
            "Reporter",
            "Ask Emerson Lopes (emersonlopes@gmail.com, EmersonLopes@Gmail.com) or emersonlopes.\n\
             Notes in /Users/emersonlopes/Documents/notes.md. A developer, not dev.\n",
        )
        .unwrap();
        let dir = src.path().join("agentic_ai/Reporter");
        std::fs::write(
            dir.join("mcp.json"),
            r#"{"s":{"command":"/Users/emersonlopes/bin/srv","args":["C:\\Users\\emersonlopes\\x"]}}"#,
        )
        .unwrap();
        let db = AgentsDb::load(src.path());
        let mut board = Leaderboard::default();
        board.ensure_models(&[("openai".into(), "gpt".into(), String::new())]);
        board.entries[0].last_error = Some("org-emersonlopes-1234 quota exceeded".into());
        let me = Personal::new(
            Some("/Users/emersonlopes".into()),
            Some("emersonlopes".into()),
            Some("Emerson Lopes".into()),
            Some("emersonlopes@gmail.com".into()),
        );
        let (bundle, _) = build(&llm, Some(&db), &board, 1);
        let out = src.path().join("ai.json");
        let removed = export(&bundle, &llm, &me, &out).unwrap();
        let text = std::fs::read_to_string(&out).unwrap();
        let lower = text.to_lowercase();
        for detail in ["emersonlopes", "emerson lopes", "gmail", "org-"] {
            assert!(!lower.contains(detail), "{detail} leaked: {text}");
        }
        assert!(text.contains("~/Documents/notes.md") && text.contains("A developer, not dev."));
        assert!(read(&out).is_ok(), "still a valid file");
        println!("AI config export: {removed} personal details replaced; none survive.");
    }

    #[test]
    fn userinfo_is_stripped_and_a_foreign_file_is_refused() {
        assert_eq!(strip_userinfo("https://a:b@h.io/v1?q=1"), "https://h.io/v1?q=1");
        assert_eq!(strip_userinfo("http://localhost:11434"), "http://localhost:11434");
        let dir = project();
        let p = dir.path().join("x.json");
        std::fs::write(&p, r#"{"format":"other","version":1}"#).unwrap();
        assert!(read(&p).is_err());
    }
}
